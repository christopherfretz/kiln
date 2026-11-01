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
use kiln_ast::decl::{DeclKind, DefinitionBits, TagKind};
use kiln_ast::ids::RawId;
use kiln_ast::types::TagKeyword;
use kiln_ast::{AttrId, DeclId, ExprId, QualType};

use super::{SubstFail, Substituter};
use crate::decl::TagUse;
use crate::declspec::DeclSpec;

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
        self.sema
            .ast
            .decls
            .record(m)
            .is_some_and(|r| r.defn.contains(DefinitionBits::ANONYMOUS))
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

    /// The attributes of pattern class definition `def` on `spec`
    /// ([dcl.attr.grammar]/5 appertain to the class; clang
    /// `InstantiateAttrs`): copied, with a dependent `alignas` operand
    /// substituted. A failed operand drops the attribute (diagnosed).
    /// Also used for a data member's attributes (`field_decl`).
    pub(crate) fn class_attrs(&mut self, spec: DeclId, def: DeclId) {
        let ast = &self.sema.ast;
        let list = ast.attrs.of_decl(def);
        let nodes: Vec<AttrNode> =
            ast.lists.iter::<AttrId>(list).map(|a| *ast.attrs.get(a)).collect();
        if nodes.is_empty() || ast.attrs.of_decl(spec) != kiln_ast::ids::ListId::EMPTY {
            return;
        }
        let mut out = Vec::new();
        for mut node in nodes {
            if node.kind == AttrKind::ALIGNED {
                if let Some(t) = QualType::from_raw(node.b) {
                    let Ok(t) = self.ty(t, node.loc) else { continue };
                    node.b = t.to_raw();
                } else if let Some(e) = ExprId::from_raw(node.a) {
                    let Ok(e) = self.expr(e) else { continue };
                    let Some(e) = self.sema.check_alignment_expr(e) else { continue };
                    node.a = e.raw();
                }
            }
            out.push(self.sema.ast.attrs.push(node));
        }
        let _ = self.sema.builder().attach_attrs(spec, &out);
    }
}
