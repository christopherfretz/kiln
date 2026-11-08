//! Annotations (P3394R4 `[[=constant-expression]]`, [dcl.attr.annotation];
//! spec/annotations.md).
//!
//! The parser hands each annotation to [`Sema::act_on_annotation`], which
//! makes an `ANNOTATION` attribute node holding the operand. Once the
//! declaration it appertains to is known, [`Sema::take_annotations`]
//! checks the subject, evaluates the operand to its underlying constant
//! (frozen as a non-type template argument is) and moves it into
//! `ast.annots`; the attribute node is dropped, so no other attribute code
//! sees it. Annotations never change a declaration's meaning: nothing in
//! overload resolution, layout, codegen or mangling reads the table.

use kiln_ast::annot::{AnnId, Annotation};
use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::ids::RawId;
use kiln_ast::types::Quals;
use kiln_ast::{AttrId, ConstValueId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::init::Initializer;
use crate::templ::subst::Substituter;

impl Sema<'_, '_> {
    /// One parsed annotation `= e` (or `= e...`) at the `=` `loc`: an
    /// `ANNOTATION` attribute node whose `a` is the operand. An `...`
    /// makes the operand a pack expansion ([dcl.attr.grammar]/6); without
    /// one an unexpanded pack is an error ([temp.variadic]/12).
    pub fn act_on_annotation(
        &mut self,
        e: ExprId,
        ellipsis: Option<Loc>,
        loc: Loc,
    ) -> Option<AttrId> {
        use crate::templ::pack::PackOperand;
        let e = if let Some(l) = ellipsis {
            self.act_pack_expansion(e, l)
        } else if self.diagnose_unexpanded_pack(PackOperand::Expr(e), "expression", loc) {
            return None;
        } else {
            e
        };
        if self.is_err_expr(e) {
            return None;
        }
        let node = AttrNode {
            kind: AttrKind::ANNOTATION,
            syntax: AttrSyntax::Cxx11,
            flags: 0,
            loc,
            a: e.raw(),
            b: 0,
        };
        Some(self.ast.attrs.push(node))
    }

    /// Whether `attrs` holds an annotation.
    fn has_annotations(&self, attrs: &[AttrId]) -> bool {
        attrs.iter().any(|&a| self.ast.attrs.get(a).kind == AttrKind::ANNOTATION)
    }

    /// Drops the annotations of `attrs`, each an error: they appertain to
    /// `what`, which cannot carry one ([dcl.attr.annotation]/1), or (for
    /// `None`) to no declaration at all (a statement, a type).
    pub(crate) fn reject_annotations(&mut self, attrs: &mut Vec<AttrId>, what: Option<&str>) {
        if !self.has_annotations(attrs) {
            return;
        }
        attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            if node.kind != AttrKind::ANNOTATION {
                return true;
            }
            let d = match what {
                Some(w) => {
                    self.diag(DiagId::sema_annotation_not_allowed, node.loc).arg(w.to_owned())
                }
                None => self.diag(DiagId::sema_annotation_non_declaration, node.loc),
            };
            self.emit(d);
            false
        });
    }

    /// [`Sema::reject_annotations`] for a slice that is not kept: the
    /// parser's hook for the attributes of a label, a concept-definition
    /// or a namespace alias, which kiln keeps none of and which are not
    /// among the entities of [dcl.attr.annotation]/1.
    pub fn reject_annotations_in(&mut self, attrs: &[AttrId], what: Option<&str>) {
        if self.has_annotations(attrs) {
            let mut v = attrs.to_vec();
            self.reject_annotations(&mut v, what);
        }
    }

    /// Moves the annotations of `attrs` to the declaration `d` they
    /// appertain to ([dcl.attr.annotation]/1): each is checked against
    /// `d`'s kind and evaluated ([`Sema::annotation_value`]); a valid one
    /// is appended to `d`'s list in `ast.annots`, in source order. The
    /// attribute nodes are removed from `attrs`.
    pub(crate) fn take_annotations(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        if !self.has_annotations(attrs) {
            return;
        }
        if let Some(what) = self.annotation_subject_error(d) {
            self.reject_annotations(attrs, Some(what));
            return;
        }
        let mut out = Vec::new();
        attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            if node.kind != AttrKind::ANNOTATION {
                return true;
            }
            // One attribute shared by several declarators is one
            // annotation of each of their entities.
            if let Some(&id) = self.annot_of_attr.get(&a) {
                out.push(id);
            } else if let Some(e) = ExprId::from_raw(node.a)
                && let Some(id) = self.annotate(d, e, node.loc)
            {
                self.annot_of_attr.insert(a, id);
                out.push(id);
            }
            false
        });
        self.attach_annotations(d, &out);
    }

    fn attach_annotations(&mut self, d: DeclId, anns: &[AnnId]) {
        let l = self.ast.lists.push(anns);
        self.ast.annots.attach(d, l, &mut self.ast.lists);
    }

    /// Why `d` cannot carry an annotation ([dcl.attr.annotation]/1), as
    /// "an annotation cannot appertain to {what}"; `None` if it can.
    fn annotation_subject_error(&self, d: DeclId) -> Option<&'static str> {
        let n = self.ast.decls.get(d);
        // A block-scope function declaration or `extern` variable: its
        // host scope (the block) differs from its target scope
        // ([basic.scope.scope]/2, [dcl.meaning.general]/3.5).
        let local_extern =
            || {
                let in_block = self
                    .ast
                    .decls
                    .lexical_ctx(d)
                    .is_some_and(|c| self.ast.decls.get(c).kind().is_function());
                in_block
                    && match n.kind() {
                        DeclKind::Function => true,
                        DeclKind::Var => self.ast.decls.var(d).is_some_and(|v| {
                            v.spec.storage() == kiln_ast::decl::VarStorage::Extern
                        }),
                        _ => false,
                    }
            };
        Some(match n.kind() {
            DeclKind::Function | DeclKind::Var if local_extern() => {
                "a block-scope declaration whose target scope is a namespace"
            }
            DeclKind::ParmVar
                if self.ast.decls.get(d).ty().is_some_and(|t| self.ast.types.is_void(t)) =>
            {
                "a parameter of type 'void'"
            }
            DeclKind::Namespace
            | DeclKind::Typedef
            | DeclKind::TypeAlias
            | DeclKind::TypeAliasTemplate
            | DeclKind::Enum
            | DeclKind::EnumConstant
            | DeclKind::CXXRecord
            | DeclKind::ClassTemplate
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
            | DeclKind::Field
            | DeclKind::Function
            | DeclKind::FunctionTemplate
            | DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion
            | DeclKind::Var
            | DeclKind::VarTemplate
            | DeclKind::VarTemplateSpecialization
            | DeclKind::VarTemplatePartialSpecialization
            | DeclKind::ParmVar => return None,
            DeclKind::UsingDirective => "a using-directive",
            DeclKind::Using | DeclKind::UsingEnum => "a using-declaration",
            DeclKind::NamespaceAlias => "a namespace alias",
            DeclKind::Binding => "a structured binding",
            DeclKind::Concept => "a concept",
            DeclKind::CXXDeductionGuide => "a deduction guide",
            DeclKind::TemplateTypeParm
            | DeclKind::NonTypeTemplateParm
            | DeclKind::TemplateTemplateParm => "a template parameter",
            DeclKind::FileScopeAsm => "an asm-declaration",
            DeclKind::TranslationUnit | DeclKind::Empty => "an attribute-declaration",
            _ => "this declaration",
        })
    }

    /// One annotation of `d` with operand `e` at `loc`, appended to the
    /// table (not yet to `d`'s list).
    pub(crate) fn annotate(&mut self, d: DeclId, e: ExprId, loc: Loc) -> Option<AnnId> {
        let (value, ty, expr) = self.annotation_value(e, loc)?;
        Some(self.ast.annots.push(Annotation { decl: d, value, loc, written_type: ty, expr }))
    }

    /// The underlying constant of annotation `e` ([dcl.attr.annotation]/2):
    /// `std::meta::reflect_constant(e)` must be a constant expression, so
    /// `T` (`e`'s type, arrays and functions decayed, cv-qualifiers
    /// dropped; [meta.reflection.result]) is a structural type and the
    /// value a `const T` copy-initialized from `e`, evaluated as a
    /// template argument is and frozen ([temp.param]/8: a class value
    /// gets its template parameter object). A value-dependent operand is
    /// kept unevaluated (`None`). `None` overall after a diagnostic.
    pub(crate) fn annotation_value(
        &mut self,
        e: ExprId,
        loc: Loc,
    ) -> Option<(Option<ConstValueId>, QualType, ExprId)> {
        if self.is_err_expr(e) || self.has_errors_in(e) {
            return None;
        }
        let ety = self.ast.exprs.get(e).ty();
        if self.expr_is_value_dependent(e) || self.ast.types.is_dependent(ety) {
            return Some((None, ety, e));
        }
        let e = if self.ast.types.is_array(ety) || self.ast.types.is_function(ety) {
            crate::conv::decay(self, e)
        } else {
            e
        };
        if self.is_err_expr(e) {
            return None;
        }
        let ty = self.ast.exprs.get(e).ty();
        let ty = crate::conv::strip_cv(self, ty);
        if !self.check_annotation_type(ty, loc) {
            return None;
        }
        let canon = self.ast.types.canonical(ty);
        let canon = crate::conv::strip_cv(self, canon);
        let cty = self.ast.types.qualify(canon, Quals::CONST);
        let c = self.perform_init(cty, Initializer::Expr(e), loc, false);
        if self.is_err_expr(c) || self.has_errors_in(c) {
            return None;
        }
        let c = self.act_on_constant_full_expr(c);
        match self.eval_const(c, EvalContext::new(EvalKind::TemplateArg, loc)) {
            Ok(v) => {
                // [dcl.attr.annotation]/2: the annotation's value is
                // `reflect_constant(V)`, so V must be a valid constant
                // template argument: no pointer or reference into a string
                // literal (`[[="abc"]]`), a temporary, a `typeid` result or
                // a label ([temp.arg.nontype]/6). GCC 16.1 accepts
                // `[[="abc"]]`; kiln follows the draft (WP-830).
                if let Some(why) = self.bad_tparg_base(v) {
                    let d = self.diag(DiagId::sema_annotation_invalid_value, loc).arg(why);
                    self.emit(d);
                    return None;
                }
                if self.ast.types.as_record(canon).is_some() {
                    self.builder().template_param_object(cty, v);
                }
                Some((Some(v), ty, e))
            }
            Err(f) => {
                let d = self.diag(DiagId::sema_expr_not_cce, loc).arg("annotation");
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                None
            }
        }
    }

    /// New annotations with the values of `anns` ([dcl.attr.annotation]/3:
    /// each instantiation of an annotation is a distinct annotation).
    pub(crate) fn copy_annots(&mut self, anns: &[AnnId]) -> Vec<AnnId> {
        anns.iter()
            .map(|&a| {
                let n = *self.ast.annots.get(a);
                self.ast.annots.push(n)
            })
            .collect()
    }

    /// `T` of an annotation must be structural ([dcl.attr.annotation]/2,
    /// [meta.reflection.result]: `reflect_constant` requires it;
    /// [temp.param]/7): not `void`, and a class must be complete, literal
    /// and have only public, non-mutable subobjects of structural type.
    fn check_annotation_type(&mut self, ty: QualType, loc: Loc) -> bool {
        if self.is_error_type(ty) {
            return false;
        }
        let not_structural =
            |s: &mut Self| s.diag(DiagId::sema_annotation_not_structural, loc).arg(s.ty_arg(ty));
        if self.ast.types.is_void(ty) {
            let d = not_structural(self);
            self.emit(d);
            return false;
        }
        if let Some(r) = self.ast.types.as_record(ty) {
            if !self.require_complete(ty, loc) {
                let d = not_structural(self);
                let d = self.incomplete_note(d, ty);
                self.emit(d);
                return false;
            }
            let Some(def) = self.ast.definition(r) else { return true };
            if !self.record_bits(def).contains(DefinitionBits::LITERAL) {
                let d = not_structural(self);
                let d = self.with_non_literal_notes(d, ty);
                self.emit(d);
                return false;
            }
        }
        if self.is_structural(ty) {
            return true;
        }
        let mut d = not_structural(self);
        for n in self.not_structural_notes(ty) {
            d = d.with_note(n);
        }
        self.emit(d);
        false
    }

    /// The deferred annotations of function template specialization `f`
    /// (spec/annotations.md §5), substituted now that `f` is needed: at
    /// its odr-use or reference ([`Sema::mark_function_referenced`]) or
    /// when a query reads them. As a *noexcept-specifier*, an annotation
    /// is substituted only when needed, so a candidate that loses overload
    /// resolution never substitutes its own; a failure is a hard error,
    /// never a deduction failure ([dcl.attr.annotation]/4).
    pub(crate) fn require_annotations(&mut self, f: DeclId, poi: Loc) {
        if self.uninstantiated_annots.is_empty() || self.in_template() {
            return;
        }
        let first = self.ast.first(f);
        let Some(pattern) = self.uninstantiated_annots.remove(&first) else { return };
        self.instantiate_annotations(first, pattern, poi);
    }

    /// Substitutes the annotations of `pattern` (all its declarations')
    /// for function `f` in `f`'s instantiation context (as
    /// `substitute_exception_spec`).
    fn instantiate_annotations(&mut self, f: DeclId, pattern: DeclId, poi: Loc) {
        use crate::templ::stack::FrameKind;
        let anns = self.ast.annots.of_entity(&self.ast, pattern);
        let Some((tmpl, args)) = self.spec_args(f) else { return };
        let pairs = self.inst_context_pairs(self.spec_pairs_ctx(f, tmpl));
        let frame_args = self.ast.spec_info(f).map(|(_, i)| i.args);
        if !self.push_inst_frame(FrameKind::Annotation, f, pattern, poi) {
            return;
        }
        if let Some(l) = frame_args {
            self.set_frame_args(l);
        }
        let lex = self.templated_function(tmpl).unwrap_or(f);
        let ctx = self
            .ast
            .decls
            .lexical_ctx(lex)
            .filter(|&c| self.kind_of(c) == DeclKind::CXXRecord)
            .or_else(|| self.ast.decls.get(f).ctx())
            .unwrap_or(self.ast.tu);
        self.in_inst_context(ctx, |s| {
            let mut sub = Substituter::new(s, &args);
            sub.entity = Some(f);
            sub.map_contexts(&pairs);
            let params = sub.annotated_param_pairs(f, pattern);
            sub.subst_entity_annotations(f, &anns, &params);
        });
        self.pop_inst_frame();
    }
}

impl Substituter<'_, '_, '_> {
    /// The parameters of function `spec` paired with those of its pattern
    /// `def` that carry annotations (a parameter's own, not merged across
    /// redeclarations; spec/annotations.md §4). A pattern function
    /// parameter pack pairs with each parameter it expanded to
    /// ([temp.variadic]/5: each element is its own parameter, with its own
    /// annotations; WP-830). An annotation operand cannot name the pack
    /// (it is not a pack expansion pattern), so every element substitutes
    /// the same operands. With one pack its length is what the other
    /// parameters leave; with several (`L<A...>, A..., L<B...>, B...`),
    /// each takes the length of its own expansion under the current
    /// arguments, as [`Self::map_params`] does for the body (WP-866). A
    /// pack that stays a pack (a member template's own, substituted with
    /// its class) pairs one to one.
    pub(crate) fn annotated_param_pairs(
        &mut self,
        spec: DeclId,
        def: DeclId,
    ) -> Vec<(DeclId, DeclId)> {
        if self.sema.ast.decls.function(def).is_none() {
            return Vec::new();
        }
        let (ps, qs) = (self.sema.param_decls(spec), self.sema.param_decls(def));
        if !qs.iter().any(|&q| self.sema.ast.annots.has_list(q)) {
            return Vec::new();
        }
        let is_pack = |s: &Self, q: DeclId| {
            s.sema
                .ast
                .decls
                .param(q)
                .is_some_and(|x| x.flags & kiln_ast::decl::ParamData::PACK != 0)
        };
        let packs = qs.iter().filter(|&&q| is_pack(self, q)).count();
        let lens: Vec<usize> = match packs {
            0 => vec![1; qs.len()],
            1 => {
                let Some(n) = (ps.len() + 1).checked_sub(qs.len()) else { return Vec::new() };
                qs.iter().map(|&q| if is_pack(self, q) { n } else { 1 }).collect()
            }
            _ => {
                let mut v = Vec::with_capacity(qs.len());
                for &q in &qs {
                    v.push(if is_pack(self, q) { self.param_pack_len(q).unwrap_or(1) } else { 1 });
                }
                v
            }
        };
        if lens.iter().sum::<usize>() != ps.len() {
            return Vec::new();
        }
        let ast = &self.sema.ast;
        let wanted = |&(p, q): &(DeclId, DeclId)| {
            p != q && ast.annots.has_list(q) && !ast.annots.has_list(p)
        };
        let mut out = Vec::new();
        let mut it = ps.into_iter();
        for (q, n) in qs.into_iter().zip(lens) {
            out.extend(it.by_ref().take(n).map(|p| (p, q)).filter(wanted));
        }
        out
    }

    /// The annotations of pattern `def` (every declaration's, in order) on
    /// its instantiation `spec` ([dcl.attr.annotation]/3: each is a new
    /// annotation). A function template specialization's dependent ones
    /// wait until it is needed
    /// ([`Sema::require_annotations`]); others are substituted now, a
    /// pack expansion giving one annotation per element.
    pub(crate) fn copy_annotations(&mut self, spec: DeclId, def: DeclId) {
        if self.sema.ast.annots.is_empty() || spec == def {
            return;
        }
        let params = self.annotated_param_pairs(spec, def);
        let ast = &self.sema.ast;
        let anns =
            if ast.annots.has_list(spec) { Vec::new() } else { ast.annots.of_entity(ast, def) };
        if anns.is_empty() && params.is_empty() {
            return;
        }
        let param_anns =
            params.iter().flat_map(|&(_, q)| ast.lists.iter::<AnnId>(ast.annots.of_decl(q)));
        let dependent =
            anns.iter().copied().chain(param_anns).any(|a| ast.annots.get(a).value.is_none());
        // Only a function template specialization's wait (its signature
        // is substituted during deduction, where a failure here must not
        // be a deduction failure); a member of a class specialization,
        // member templates' patterns included, is substituted with it.
        if dependent && self.sema.is_function_spec(spec) {
            let first = ast.first(spec);
            self.sema.uninstantiated_annots.insert(first, def);
            return;
        }
        self.subst_entity_annotations(spec, &anns, &params);
    }

    /// `anns` substituted and attached to `spec`, then each pattern
    /// parameter's annotations to the paired parameter of `spec`.
    fn subst_entity_annotations(
        &mut self,
        spec: DeclId,
        anns: &[AnnId],
        params: &[(DeclId, DeclId)],
    ) {
        if !anns.is_empty() {
            let out = self.subst_annotations(spec, anns);
            self.sema.attach_annotations(spec, &out);
        }
        for &(p, q) in params {
            let ast = &self.sema.ast;
            let qa: Vec<AnnId> = ast.lists.iter::<AnnId>(ast.annots.of_decl(q)).collect();
            let out = self.subst_annotations(p, &qa);
            self.sema.attach_annotations(p, &out);
        }
    }

    /// `anns` substituted for `spec`: a frozen value is shared (a new
    /// annotation with the same underlying constant), a dependent operand
    /// is substituted and evaluated. A failure is diagnosed and drops the
    /// annotation ([dcl.attr.annotation]/4: not a deduction failure).
    pub(crate) fn subst_annotations(&mut self, spec: DeclId, anns: &[AnnId]) -> Vec<AnnId> {
        let mut out = Vec::with_capacity(anns.len());
        for &a in anns {
            let n = *self.sema.ast.annots.get(a);
            if n.value.is_some() {
                out.push(self.sema.ast.annots.push(Annotation { decl: spec, ..n }));
                continue;
            }
            let Ok(es) = self.expand_exprs(&[n.expr]) else { continue };
            for e in es {
                if let Some(id) = self.sema.annotate(spec, e, n.loc) {
                    out.push(id);
                }
            }
        }
        out
    }
}
