//! Unnamed member classes of class templates and the attributes of an
//! instantiated class (spec/sema-templates.md §4.6).
//!
//! An unnamed member class has no name to be found by, so it is defined
//! with its enclosing class: an anonymous struct or union
//! ([class.union.anon]) declares the unnamed member object and injects
//! its members' names as `IndirectField`s, as the parser does; an unnamed
//! class naming a member's type (`struct { int a; } s;`) is defined
//! before that member (clang `TemplateDeclInstantiator::VisitCXXRecordDecl`
//! for an anonymous record, `InstantiateClass` then
//! `BuildAnonymousStructOrUnion`).

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, TagKind, VarStorage};
use kiln_ast::ids::RawId;
use kiln_ast::types::TagKeyword;
use kiln_ast::{AttrId, DeclId, ExprId, QualType};

use super::{SubstFail, SubstResult, Substituter};
use crate::decl::TagUse;
use crate::declspec::{DeclSpec, StorageSpec};

impl Substituter<'_, '_, '_> {
    /// True if `m` is an unnamed member class of the pattern.
    pub(crate) fn is_unnamed_member_class(&self, m: DeclId) -> bool {
        let n = self.sema.ast.decls.get(m);
        n.kind() == DeclKind::CXXRecord
            && n.name().is_empty()
            && self.sema.ast.definition(m) == Some(m)
    }

    /// Whether record `m` of the pattern is an anonymous struct or union.
    pub(crate) fn is_anonymous_record(&self, m: DeclId) -> bool {
        self.sema.ast.decls.record(m).is_some_and(|r| r.defn.contains(DefinitionBits::ANONYMOUS))
    }

    /// Unnamed member class `m` of the pattern: defined now with its
    /// members; an anonymous struct or union then declares its member
    /// object and injects its members' names into the class being
    /// instantiated. The instantiations are linked to their patterns
    /// here (the caller links nothing).
    pub(crate) fn unnamed_member_class(&mut self, m: DeclId) -> Result<(), SubstFail> {
        let loc = self.sema.ast.decls.get(m).loc();
        let Some(rec) = self.sema.ast.decls.record(m).copied() else { return Err(SubstFail) };
        let kw = match rec.tag {
            TagKind::Class => TagKeyword::Class,
            TagKind::Union => TagKeyword::Union,
            TagKind::Struct => TagKeyword::Struct,
        };
        let anonymous = rec.defn.contains(DefinitionBits::ANONYMOUS);
        let parent = self.entity;
        let x = self.sema.act_on_tag(kw, None, None, TagUse::Definition, 0, None, loc);
        if let Some(spec) = parent {
            self.sema.ast.templates.record_member(spec, m, x);
        }
        self.class_body_with(x, m, None, anonymous);
        self.entity = parent;
        if !anonymous {
            return Ok(());
        }
        let mut ds = DeclSpec::new(loc);
        ds.keyword = kw;
        ds.tag = Some(x);
        ds.tag_declares = true;
        let Some(obj) = self.sema.act_on_decl_spec_only(&ds) else { return Err(SubstFail) };
        let Some(spec) = parent else { return Ok(()) };
        // The pattern's member object (implicit, of type `m`) and the
        // names it injected.
        let Some(pctx) = self.sema.ast.decls.get(m).ctx() else { return Ok(()) };
        let ast = &self.sema.ast;
        let pobj = ast.decls.members(pctx).find(|&f| {
            ast.decls.get(f).kind() == DeclKind::Field
                && ast.decls.get(f).ty().and_then(|t| ast.types.as_record(t)) == Some(m)
        });
        let heads = |ctx: DeclId, head: DeclId| -> Vec<DeclId> {
            ast.decls
                .members(ctx)
                .filter(|&f| {
                    ast.decls.get(f).kind() == DeclKind::IndirectField
                        && ast.lists.iter::<DeclId>(ast.indirect_field_chain(f)).next()
                            == Some(head)
                })
                .collect()
        };
        let mut links = Vec::new();
        if let Some(pobj) = pobj {
            links.push((pobj, obj));
            let new = heads(spec, obj);
            for p in heads(pctx, pobj) {
                let name = ast.decls.get(p).name();
                if let Some(&i) = new.iter().find(|&&i| ast.decls.get(i).name() == name) {
                    links.push((p, i));
                }
            }
        }
        for (p, i) in links {
            self.sema.ast.templates.record_member(spec, p, i);
        }
        Ok(())
    }

    /// Whether pattern variable `v` is the unnamed object of a
    /// block-scope anonymous union ([class.union.anon]/1).
    pub(crate) fn is_anonymous_object(&self, v: DeclId) -> bool {
        let n = self.sema.ast.decls.get(v);
        n.has(DeclFlags::IMPLICIT)
            && n.name().is_empty()
            && n.ty()
                .and_then(|t| self.sema.ast.types.as_record(t))
                .is_some_and(|r| self.is_anonymous_record(r))
    }

    /// The unnamed object `v` of a block-scope anonymous union of the
    /// pattern: declared again for the instantiated union (already
    /// defined by the declaration statement's preceding class), which
    /// injects its members' names into the block scope. The pattern
    /// names a member as `v.member`, so `v` alone is linked.
    pub(crate) fn local_anonymous_object(&mut self, v: DeclId) -> SubstResult<DeclId> {
        let n = *self.sema.ast.decls.get(v);
        let prec = n.ty().and_then(|t| self.sema.ast.types.as_record(t)).ok_or(SubstFail)?;
        let rec = self.find_instantiated_decl(prec, n.loc())?;
        let mut ds = DeclSpec::new(n.loc());
        ds.keyword = TagKeyword::Union;
        ds.tag = Some(rec);
        ds.tag_declares = true;
        if self.sema.ast.decls.var(v).is_some_and(|x| x.spec.storage() == VarStorage::Static) {
            ds.storage = Some((StorageSpec::Static, n.loc()));
        }
        let obj = self.sema.act_on_decl_spec_only(&ds).ok_or(SubstFail)?;
        self.locals.insert(v, super::DeclOrPack::Decl(obj));
        Ok(obj)
    }

    /// The attributes of pattern class definition `def` on `spec`
    /// ([dcl.attr.grammar]/5 appertain to the class; clang
    /// `InstantiateAttrs`): copied, with a dependent `alignas` operand
    /// substituted. A failed operand drops the attribute (diagnosed).
    /// Also used for a data member's attributes (`field_decl`) and a
    /// function's (`function_decl`, `function_spec_signature`).
    pub(crate) fn class_attrs(&mut self, spec: DeclId, def: DeclId) {
        self.copy_attrs(spec, def, false);
    }

    /// [`Self::class_attrs`] for function template specialization `spec`
    /// of pattern `pat`, plus every attribute written only on another
    /// declaration of the template seen so far: clang substitutes the
    /// specialization from the latest redeclaration, onto which
    /// `mergeDeclAttributes` carried every earlier one's, so an attribute
    /// added by a later declaration (libc++'s `get` and `any_cast`,
    /// forward-declared without `nodiscard`; a `cold` definition) or
    /// present only on the first applies to calls through any of them
    /// ([dcl.attr.nodiscard]/2). A specialization formed before such a
    /// declaration keeps what it had, as clang's does.
    ///
    /// For a member template instantiated with its class, clang
    /// substitutes from the member declaration made when the class
    /// specialization was instantiated, which carries the attributes of
    /// the declarations seen by then: no attribute written only on an
    /// out-of-line definition seen later applies (libstdc++'s
    /// `basic_string::__resize_and_overwrite` and its `always_inline`,
    /// WP-818; `nodiscard`, `pure`, `const`, `cold`, ..., WP-837).
    /// Declaration ids are allocated in creation order, so the
    /// declarations compare with the instantiated member template by id.
    /// spec/sema-templates.md "Attributes of a specialization".
    pub(crate) fn function_spec_attrs(&mut self, spec: DeclId, pat: DeclId) {
        let member = self
            .sema
            .ast
            .spec_info(spec)
            .map(|(_, s)| s.template)
            .filter(|&t| self.sema.inst_template(t) != t);
        let late = |r: DeclId| member.is_some_and(|m| r > m);
        if !late(pat) {
            self.copy_attrs(spec, pat, false);
        }
        let ast = &self.sema.ast;
        let mut have: Vec<AttrKind> = ast
            .lists
            .iter::<AttrId>(ast.attrs.of_decl(spec))
            .map(|a| ast.attrs.get(a).kind)
            .collect();
        let mut extra = Vec::new();
        for r in ast.redecls_of(pat).take(4096) {
            if r == pat || late(r) || !ast.decls.get(r).has(DeclFlags::HAS_ATTRS) {
                continue;
            }
            for a in ast.lists.iter::<AttrId>(ast.attrs.of_decl(r)) {
                let n = ast.attrs.get(a);
                if !have.contains(&n.kind) && Self::merges_unsubstituted(n) {
                    have.push(n.kind);
                    extra.push(a);
                }
            }
        }
        self.sema.builder().append_attrs(spec, &extra);
    }

    /// Whether attribute `n` of another declaration of a function
    /// template carries onto a specialization as written
    /// ([`Self::function_spec_attrs`]): not one [`Self::copy_attrs`]
    /// substitutes (its operands name the template's parameters) or
    /// never clones.
    fn merges_unsubstituted(n: &AttrNode) -> bool {
        let substituted = matches!(
            n.kind,
            AttrKind::ALIGNED
                | AttrKind::ENABLE_IF
                | AttrKind::DIAGNOSE_IF
                | AttrKind::VISIBILITY
                | AttrKind::TYPE_VISIBILITY
                | AttrKind::MODE
        ) || (n.kind == AttrKind::ANNOTATE && n.b != 0);
        let dependent_priority = n.flags & AttrNode::PRIORITY_DEPENDENT != 0
            && matches!(n.kind, AttrKind::CONSTRUCTOR | AttrKind::DESTRUCTOR);
        !substituted && !dependent_priority
    }

    /// The `alignas` attributes of pattern variable `def` on its
    /// instantiation `spec` (clang `InstantiateAttrs` for a local
    /// variable; [dcl.align]/1).
    pub(crate) fn var_align_attrs(&mut self, spec: DeclId, def: DeclId) {
        self.copy_attrs(spec, def, true);
    }

    /// Copies `def`'s attributes onto `spec`: only `aligned`/`cleanup`
    /// when `only_aligned`.
    fn copy_attrs(&mut self, spec: DeclId, def: DeclId, only_aligned: bool) {
        self.copy_annotations(spec, def);
        let ast = &self.sema.ast;
        let list = ast.attrs.of_decl(def);
        let nodes: Vec<AttrNode> = ast
            .lists
            .iter::<AttrId>(list)
            .map(|a| *ast.attrs.get(a))
            // A local variable's `cleanup` names a function, not a dependent
            // entity (WP-807).
            .filter(|n| !only_aligned || matches!(n.kind, AttrKind::ALIGNED | AttrKind::CLEANUP))
            // clang does not clone a visibility attribute (`Clone = 0`):
            // codegen reads the pattern's (spec/codegen.md "Symbol
            // visibility"); a `mode` is applied to the substituted type.
            .filter(|n| {
                !matches!(n.kind, AttrKind::VISIBILITY | AttrKind::TYPE_VISIBILITY | AttrKind::MODE)
            })
            .collect();
        if nodes.is_empty() || ast.attrs.of_decl(spec) != kiln_ast::ids::ListId::EMPTY {
            return;
        }
        let mut out = Vec::new();
        for node in nodes {
            if node.kind == AttrKind::ALIGNED {
                // `alignas(Ts...)` is one attribute per element
                // ([dcl.align]/1, [temp.variadic]/5; clang
                // `instantiateDependentAlignedAttr`); the strictest wins.
                if let Some(t) = QualType::from_raw(node.b) {
                    let Ok(ts) = self.expand_types(&[t], node.loc) else { continue };
                    for t in ts {
                        // [dcl.align]/3: each substituted type-id is an
                        // `alignof` operand, so it is complete; reported
                        // at the attribute (clang `CheckAlignasTypeArgument`
                        // from `instantiateDependentAlignedAttr`).
                        if !self.sema.check_trait_operand_named(
                            kiln_ast::expr::TraitKind::AlignOf,
                            "alignas",
                            t,
                            node.loc,
                            None,
                        ) {
                            continue;
                        }
                        let n = AttrNode { b: t.to_raw(), ..node };
                        out.push(self.sema.ast.attrs.push(n));
                    }
                    continue;
                } else if let Some(e) = ExprId::from_raw(node.a) {
                    let Ok(es) = self.expand_exprs(&[e]) else { continue };
                    for e in es {
                        let Some(e) = self.sema.check_alignment_expr(e, node.loc) else {
                            continue;
                        };
                        let n = AttrNode { a: e.raw(), ..node };
                        out.push(self.sema.ast.attrs.push(n));
                    }
                    continue;
                }
            }
            if node.flags & AttrNode::PRIORITY_DEPENDENT != 0
                && matches!(node.kind, AttrKind::CONSTRUCTOR | AttrKind::DESTRUCTOR)
            {
                // clang 22 `instantiateDependentConstructorAttr` (WP-807).
                let Some(e) = ExprId::from_raw(node.a) else { continue };
                let Ok(es) = self.expand_exprs(&[e]) else { continue };
                if let [e] = es[..]
                    && let Some(n) = self.sema.instantiate_ctor_dtor_priority(node, e)
                {
                    out.push(self.sema.ast.attrs.push(n));
                }
                continue;
            }
            if matches!(node.kind, AttrKind::ENABLE_IF | AttrKind::DIAGNOSE_IF) {
                // Its condition names the instantiation's parameters.
                if let Some(n) = self.enable_if_cond(spec, def, node) {
                    out.push(self.sema.ast.attrs.push(n));
                }
                continue;
            }
            if node.kind == AttrKind::ANNOTATE && node.b != 0 {
                // clang `instantiateDependentAnnotationAttr`: the other
                // arguments are substituted, then constant-folded (WP-954).
                let args = self.sema.annotate_args(&node);
                if args.iter().any(|&e| self.sema.expr_is_value_dependent(e)) {
                    let Ok(es) = self.expand_exprs(&args) else { continue };
                    let shown = self.sema.attr_shown(&node, "annotate");
                    let Some(es) = self.sema.check_annotate_args(&shown, &es) else { continue };
                    let b = self.sema.ast.lists.push(&es).raw();
                    out.push(self.sema.ast.attrs.push(AttrNode { b, ..node }));
                    continue;
                }
            }
            out.push(self.sema.ast.attrs.push(node));
        }
        let _ = self.sema.builder().attach_attrs(spec, &out);
    }
}
