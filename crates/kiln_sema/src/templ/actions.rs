//! Parser actions for template-ids, dependent names and the dependent
//! expression forms (spec/sema-templates.md §2.2-§2.7, §11.2). Bodies
//! build nodes and check kinds only; everything that needs the engine
//! (WP-070..074) reports `sema_unimplemented_template_feature`.

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{BinaryOp, ExprDeps, ExprKind};
use kiln_ast::ids::{ArgListId, NnsId, TemplateNameId, TemplateParamListId};
use kiln_ast::names::DeclName;
use kiln_ast::templates::{Requirement, TemplateParmInfo};
use kiln_ast::types::{
    ArgView, Nns, TagKeyword, TemplateArg, TemplateName, TypeFlags, TypeKind, TypeView,
};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::decl::sym_str;
use crate::scope::{Scope, ScopeKind};

/// What a template-id names ([temp.names]/3-8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateIdRef {
    /// A class, alias or template template parameter specialization.
    Type(QualType),
    /// A concept-id (`ConceptSpecializationExpr`) or a variable template
    /// specialization (an `UnresolvedLookupExpr` while dependent).
    Value(ExprId),
    /// A function template-id: an `UnresolvedLookupExpr` naming the
    /// templates, resolved by the call (WP-071).
    Overload(ExprId),
}

impl Sema<'_, '_> {
    // -- dependent qualifiers --------------------------------------------------

    /// True if qualifier context `d` is a dependent marker
    /// (spec/sema-templates.md §11 "Landed"): a type template parameter,
    /// template template parameter, a class or alias template named with
    /// dependent arguments that are not the current instantiation, or a
    /// typedef of a dependent type. Qualified lookup into it finds nothing
    /// and names after it are dependent ([temp.dep.type]/6).
    pub fn is_dependent_ctx(&self, d: DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::TemplateTypeParm
            | DeclKind::TemplateTemplateParm
            | DeclKind::ClassTemplate
            | DeclKind::TypeAliasTemplate => true,
            DeclKind::Typedef | DeclKind::TypeAlias => {
                self.ast.decls.get(d).ty().is_some_and(|t| {
                    self.is_dependent_type(t) && self.ast.types.as_record(t).is_none()
                })
            }
            _ => false,
        }
    }

    /// True if `t` is a dependent type ([temp.dep.type]/7).
    pub fn is_dependent_type(&self, t: QualType) -> bool {
        self.ast.types.flags(t.ty()).intersects(TypeFlags::DEPENDENT_INSTANTIATION)
    }

    /// One component `name::` of a nested-name-specifier after `prefix`
    /// (its context and specifier): [`Sema::act_on_nested_name_spelled`]
    /// plus the dependent forms (`T::`, `T::X::`, [temp.dep.type]/6).
    pub fn act_on_nested_name_component(
        &mut self,
        prefix: Option<(DeclId, NnsId)>,
        name: Symbol,
        loc: Loc,
    ) -> Option<(DeclId, NnsId)> {
        if let Some((p, pn)) = prefix
            && self.is_dependent_ctx(p)
        {
            let n = self.ast.types.nns(Nns::Identifier { prefix: Some(pn), name });
            return Some((p, n));
        }
        let (ctx, spelled) = self.act_on_nested_name_spelled(prefix.map(|p| p.0), name, loc)?;
        let n = self.nns_component(prefix.map(|p| p.1), ctx, spelled);
        Some((ctx, n))
    }

    /// `prefix X<args>::` where the template-id named type `ty` of
    /// template `tmpl` ([temp.res.general]/2): the current instantiation
    /// (its pattern, [temp.dep.type]/1), a dependent marker, or the
    /// instantiated specialization ([temp.inst]/2).
    pub fn act_on_nested_name_template_id(
        &mut self,
        prefix: Option<NnsId>,
        tmpl: DeclId,
        ty: QualType,
        template_kw: bool,
        loc: Loc,
    ) -> Option<(DeclId, NnsId)> {
        let n = self.ast.types.nns(Nns::Type { prefix, ty, template_kw });
        if let Some(p) = self.current_instantiation(tmpl, ty) {
            return Some((p, n));
        }
        if !self.is_dependent_type(ty) {
            // [expr.prim.id.qual]/1: the class must be complete.
            if !self.require_complete_nns(ty, loc) {
                return None;
            }
            let r = self.ast.types.as_record(ty)?;
            return Some((self.ast.definition(r).unwrap_or(r), n));
        }
        Some((tmpl, n))
    }

    /// The pattern definition named by `X<args>` when `args` are `X`'s own
    /// parameters in order and `X`'s definition (or one of its members) is
    /// being processed ([temp.dep.type]/1.2).
    pub(crate) fn current_instantiation(&self, tmpl: DeclId, ty: QualType) -> Option<DeclId> {
        if self.kind_of(tmpl) != DeclKind::ClassTemplate {
            return None;
        }
        let TypeView::TemplateSpecialization { args, .. } = self.ast.types.view(ty.ty()) else {
            return None;
        };
        let info = self.ast.template_info(tmpl).copied()?;
        let list = *self.ast.templates.param_list(info.params);
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
        let args = self.ast.types.args(args).to_vec();
        if params.len() != args.len() {
            return None;
        }
        for (&p, &a) in params.iter().zip(&args) {
            if !self.arg_names_param(p, a) {
                return None;
            }
        }
        let pat = self.pattern_of(tmpl)?;
        if !self.templ.entering_context && !self.encloses(pat, self.ctx) {
            // `X<P...>` outside `X` (an alias template's or another
            // template's own parameters at the same positions) is an
            // ordinary dependent specialization.
            return None;
        }
        Some(self.ast.definition(pat).unwrap_or(pat))
    }

    /// Whether `d` is a template parameter of `p`'s kind at `p`'s depth
    /// and index (`info`).
    fn same_template_parm(&self, d: DeclId, p: DeclId, info: TemplateParmInfo) -> bool {
        self.kind_of(d) == self.kind_of(p)
            && self
                .ast
                .template_parm(d)
                .is_some_and(|i| i.depth == info.depth && i.index == info.index)
    }

    /// Whether template argument `a` names parameter `p` (at its depth
    /// and index; a pack expanded).
    pub(crate) fn arg_names_param(&self, p: DeclId, a: TemplateArg) -> bool {
        let Some(info) = self.ast.template_parm(p).copied() else { return false };
        match (self.kind_of(p), a.view()) {
            (DeclKind::TemplateTypeParm, ArgView::Type(t)) => {
                let t = match self.ast.types.view(t.ty()) {
                    TypeView::PackExpansion { pattern, .. } if info.pack => pattern,
                    _ if info.pack => return false,
                    _ => t,
                };
                let own = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                self.ast.types.canonical(t) == self.ast.types.canonical(own)
            }
            (DeclKind::NonTypeTemplateParm, ArgView::Expr { expr, .. }) => {
                let mut e = expr;
                if info.pack && self.ast.exprs.get(e).kind() == ExprKind::PackExpansionExpr {
                    e = self.ast.exprs.operand(e);
                }
                // The parameter at the same position: the out-of-line
                // head redeclares the class template's parameters.
                self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr
                    && self.same_template_parm(self.ast.exprs.decl(e), p, info)
            }
            (
                DeclKind::TemplateTemplateParm,
                ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. },
            ) => {
                matches!(self.ast.types.template_name_view(n), TemplateName::TemplateTemplateParm { decl, .. } if self.same_template_parm(decl, p, info))
            }
            _ => false,
        }
    }

    // -- template-ids -------------------------------------------------------------

    /// The function template-id `e` (an `UnresolvedLookupExpr`), written
    /// unqualified and followed by `(`: the call also uses
    /// argument-dependent lookup ([basic.lookup.argdep]/1).
    pub fn act_on_template_id_call_name(&mut self, e: ExprId) {
        if self.ast.exprs.get(e).kind() == ExprKind::UnresolvedLookupExpr {
            let _ = self.ast.exprs.set_adl(e);
        }
    }

    /// The template name of template (or template template parameter)
    /// `d` as written.
    pub fn template_name_for(&mut self, d: DeclId) -> TemplateNameId {
        if let Some(i) = self.ast.template_parm(d).copied()
            && self.kind_of(d) == DeclKind::TemplateTemplateParm
        {
            return self.ast.types.template_name(TemplateName::TemplateTemplateParm {
                decl: d,
                depth: u32::from(i.depth),
                index: u32::from(i.index),
                pack: i.pack,
            });
        }
        self.ast.types.template_name_of(self.ast.first(d))
    }

    /// `name < args >` ([temp.names]): what the template-id names, by the
    /// kind of the template. Dependent specializations are built as
    /// written; non-dependent class and alias template specializations
    /// are formed (§4.2, §4.10); variable templates need WP-071.
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub fn act_template_id(
        &mut self,
        name: TemplateNameId,
        args: &[TemplateArg],
        l: Loc,
        r: Loc,
    ) -> Result<TemplateIdRef, ()> {
        let _ = r;
        let list = self.ast.types.arg_list(args);
        let dep_args =
            self.ast.types.arg_list_flags(list).intersects(TypeFlags::DEPENDENT_INSTANTIATION);
        let Some(d) = self.ast.types.template_decl(name) else {
            return Ok(TemplateIdRef::Type(match self.ast.types.template_name_view(name) {
                // `TT<args>` for a template template parameter `TT`.
                TemplateName::TemplateTemplateParm { .. }
                | TemplateName::CanonTemplateTemplateParm { .. } => {
                    self.ast.types.template_specialization(name, list, None, false)
                }
                // `T::template X<args>` ([temp.names]/5).
                TemplateName::DependentIdentifier { nns, name: id } => self
                    .ast
                    .types
                    .dependent_template_specialization(TagKeyword::None, nns, id, list),
                _ => QualType::DEPENDENT,
            }));
        };
        match self.kind_of(d) {
            DeclKind::ClassTemplate | DeclKind::TemplateTemplateParm => {
                let ttp = self.kind_of(d) == DeclKind::TemplateTemplateParm;
                if !dep_args && !ttp {
                    let locs = std::mem::take(&mut self.templ.arg_locs);
                    let t = self.class_template_type(name, d, args, &locs, l)?;
                    return Ok(TemplateIdRef::Type(t));
                }
                Ok(TemplateIdRef::Type(
                    self.ast.types.template_specialization(name, list, None, false),
                ))
            }
            DeclKind::TypeAliasTemplate => {
                // [temp.alias]/2: the alias is substituted even with
                // dependent arguments; an argument that is a pack expansion
                // cannot be matched to parameters yet (WP-071), so that
                // specialization stands for itself.
                if !args.iter().any(|&a| self.is_pack_expansion_arg(a)) {
                    let locs = std::mem::take(&mut self.templ.arg_locs);
                    let t = self.alias_template_type(name, d, args, &locs, l)?;
                    return Ok(TemplateIdRef::Type(t));
                }
                Ok(TemplateIdRef::Type(
                    self.ast.types.template_specialization(name, list, None, true),
                ))
            }
            DeclKind::VarTemplate => {
                if !dep_args {
                    self.templ_unimplemented("variable template specialization", l);
                    return Err(());
                }
                let n = self.decl_symbol(d);
                let e = self.builder().unresolved_lookup(n, &[d], false, None, Some(list), l);
                Ok(TemplateIdRef::Value(e))
            }
            DeclKind::FunctionTemplate => {
                let n = self.decl_symbol(d);
                let e = self.builder().unresolved_lookup(n, &[d], false, None, Some(list), l);
                Ok(TemplateIdRef::Overload(e))
            }
            DeclKind::Concept => {
                let c = self.ast.first(d);
                let e = self.builder().concept_spec_expr(c, list, QualType::BOOL, l);
                Ok(TemplateIdRef::Value(e))
            }
            _ => {
                let msg = self
                    .diag(DiagId::sema_not_a_template, l)
                    .arg(format!("'{}'", self.decl_name(d)));
                self.emit(msg);
                Err(())
            }
        }
    }

    /// Whether `a` is a pack expansion (`T...`, `TT...`, `e...`).
    pub(crate) fn is_pack_expansion_arg(&self, a: TemplateArg) -> bool {
        match a.view() {
            ArgView::Type(q) => self.ast.types.kind(q.ty()) == TypeKind::PackExpansion,
            ArgView::TemplateExpansion { .. } => true,
            ArgView::Expr { expr, .. } => {
                self.ast.exprs.get(expr).kind() == ExprKind::PackExpansionExpr
            }
            _ => false,
        }
    }

    fn decl_symbol(&self, d: DeclId) -> Symbol {
        self.ast.decls.get(d).name().as_ident().unwrap_or(Symbol::from_index(0))
    }

    /// `[typename] nns::name` / `nns::[template] name<args>` with a
    /// dependent `nns` ([temp.res.general]/4): `DependentNameType` or
    /// `DependentTemplateSpecializationType`.
    pub fn act_dependent_name(
        &mut self,
        nns: NnsId,
        name: Symbol,
        targs: Option<ArgListId>,
        kw: TagKeyword,
    ) -> QualType {
        match targs {
            Some(a) => self.ast.types.dependent_template_specialization(kw, nns, name, a),
            None => self.ast.types.dependent_name(kw, nns, name),
        }
    }

    /// `nns::name` (with optional explicit arguments) in an expression,
    /// with a dependent `nns` ([temp.dep.expr]/3); `begin` is the
    /// qualifier's first token.
    pub fn act_dependent_scope_ref(
        &mut self,
        nns: NnsId,
        name: Symbol,
        targs: Option<ArgListId>,
        begin: Loc,
        loc: Loc,
    ) -> ExprId {
        self.builder().dependent_scope_decl_ref(nns, name, targs, begin, loc)
    }

    // -- concepts and requires ------------------------------------------------------

    /// `template<params> concept name = e;` ([temp.concept]/1); the head
    /// was consumed by [`Sema::take_template_head`].
    pub fn act_concept(
        &mut self,
        head: TemplateParamListId,
        name: Symbol,
        loc: Loc,
        e: ExprId,
    ) -> DeclId {
        let _ = self.take_template_head(None);
        self.templ.cur = None;
        if self.in_local_scope() || self.kind_of(self.ctx) == DeclKind::CXXRecord {
            self.error(DiagId::sema_template_outside_namespace_or_class, loc);
        }
        let h = DeclHead::new(self.ctx, DeclName::ident(name), loc);
        self.builder().concept(h, head, e)
    }

    /// `requires (params) {` ([expr.prim.req]/1): opens the body whose
    /// parameters the requirements see. Returns the `RequiresExprBody`.
    pub fn act_on_requires_expr_start(&mut self, loc: Loc) -> DeclId {
        let ctx = self.ctx;
        let body = self.builder().requires_body(ctx, loc);
        self.scopes.push(Scope::new(ScopeKind::RequiresExpr, Some(body)));
        self.templ.req_ctx.push(self.ctx);
        self.ctx = body;
        // The requirements are unevaluated operands ([expr.prim.req.general]/2).
        self.enter_unevaluated();
        body
    }

    /// `}` of a requires-expression: closes the scope opened by
    /// [`Sema::act_on_requires_expr_start`].
    pub fn act_requires_expr(
        &mut self,
        body: DeclId,
        params: &[DeclId],
        reqs: &[Requirement],
        loc: Loc,
        rbrace: Loc,
    ) -> ExprId {
        if let Some(i) = self.scopes.iter().rposition(|s| s.kind == ScopeKind::RequiresExpr) {
            self.scopes.truncate(i);
        }
        if let Some(c) = self.templ.req_ctx.pop() {
            self.ctx = c;
            self.exit_unevaluated();
        }
        if !self.in_template() {
            // A non-dependent requires-expression is evaluated at once
            // ([expr.prim.req.general]/5): satisfaction is WP-073.
            let dep = reqs.iter().any(|r| match *r {
                Requirement::Simple(e) | Requirement::Nested(e) => {
                    !self.ast.exprs.get(e).deps().is_none()
                }
                Requirement::Compound { expr, .. } => !self.ast.exprs.get(expr).deps().is_none(),
                Requirement::Type(t) => self.is_dependent_type(t),
            });
            if !dep && !reqs.is_empty() {
                self.templ_unimplemented("evaluation of a requires-expression", loc);
            }
        }
        self.builder().requires_expr(body, params, reqs, QualType::BOOL, loc, rbrace)
    }

    /// `{ e } -> C<args>` ([expr.prim.req.compound]/1): the immediately
    /// declared constraint `C<decltype((e)), args...>` written with an
    /// invented parameter, as clang builds it.
    pub fn act_on_return_type_requirement(
        &mut self,
        concept: DeclId,
        args: &[TemplateArg],
        loc: Loc,
    ) -> ExprId {
        let depth = self.template_depth();
        // clang names the invented parameter `expr-type` (not implicit).
        let name = self.intern("expr-type");
        let h = DeclHead::new(self.ctx, DeclName::ident(name), loc);
        let p = self.builder().template_type_parm(h, depth, 0, false, false);
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        self.immediately_declared_constraint(concept, ty, args, loc)
    }

    // -- packs and folds ----------------------------------------------------------------

    /// `pattern...` ([temp.variadic]/5): the pattern must name a pack.
    pub fn act_pack_expansion(&mut self, pattern: ExprId, ellipsis: Loc) -> ExprId {
        if self.is_err_expr(pattern) {
            return pattern;
        }
        if !self.ast.exprs.get(pattern).deps().contains(ExprDeps::UNEXPANDED_PACK) {
            self.error(DiagId::sema_pack_expansion_without_packs, ellipsis);
            return self.recover_err(&[pattern], ellipsis);
        }
        self.builder().pack_expansion_expr(pattern, ellipsis)
    }

    /// `T...` as a type ([temp.variadic]/5).
    pub fn act_pack_expansion_type(&mut self, pattern: QualType, ellipsis: Loc) -> QualType {
        if !self.ast.types.flags(pattern.ty()).contains(TypeFlags::UNEXPANDED_PACK) {
            self.error(DiagId::sema_pack_expansion_without_packs, ellipsis);
            return pattern;
        }
        self.ast.types.pack_expansion(pattern, None)
    }

    /// A template argument followed by `...`.
    pub fn act_pack_expansion_arg(&mut self, a: TemplateArg, ellipsis: Loc) -> TemplateArg {
        match a.view() {
            ArgView::Type(t) => TemplateArg::ty(self.act_pack_expansion_type(t, ellipsis)),
            ArgView::Expr { expr, .. } => {
                let e = self.act_pack_expansion(expr, ellipsis);
                self.builder().template_arg_expr(e)
            }
            ArgView::Template(n) => {
                if !self.ast.types.template_name_flags(n).contains(TypeFlags::UNEXPANDED_PACK) {
                    self.error(DiagId::sema_pack_expansion_without_packs, ellipsis);
                    return a;
                }
                TemplateArg::template_expansion(n, None)
            }
            _ => a,
        }
    }

    /// A fold-expression ([expr.prim.fold]): `( ... op e )`, `( e op ... )`
    /// or `( e1 op ... op e2 )`; exactly one operand of a binary fold has
    /// unexpanded packs ([temp.variadic]/11).
    pub fn act_fold(
        &mut self,
        lhs: Option<ExprId>,
        op: BinaryOp,
        rhs: Option<ExprId>,
        lparen: Loc,
        ellipsis: Loc,
        rparen: Loc,
    ) -> ExprId {
        let packs = |s: &Self, e: Option<ExprId>| {
            e.is_some_and(|e| s.ast.exprs.get(e).deps().contains(ExprDeps::UNEXPANDED_PACK))
        };
        let (l, r) = (packs(self, lhs), packs(self, rhs));
        if !l && !r {
            self.error(DiagId::sema_pack_expansion_without_packs, ellipsis);
            let kids: Vec<ExprId> = [lhs, rhs].into_iter().flatten().collect();
            return self.recover_err(&kids, lparen);
        }
        if l && r {
            self.error(DiagId::sema_fold_packs_both_sides, ellipsis);
        }
        self.builder().fold_expr(lhs, op, rhs, lparen, rparen)
    }

    /// `sizeof...(name)` ([expr.sizeof]/5): `name` must name a pack.
    pub fn act_on_sizeof_pack_name(
        &mut self,
        name: Symbol,
        name_loc: Loc,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let found = self.lookup_name(name, crate::lookup::LookupKind::Ordinary);
        let pack = found.decls.first().copied().filter(|&d| self.is_pack_decl(d));
        match pack {
            Some(d) => {
                self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
                let st = self.size_type();
                self.builder().size_of_pack(d, st, loc, rparen)
            }
            None => self.act_on_sizeof_pack(name, name_loc, loc),
        }
    }

    /// True for a template parameter pack or a function parameter pack.
    pub fn is_pack_decl(&self, d: DeclId) -> bool {
        if let Some(i) = self.ast.template_parm(d) {
            return i.pack;
        }
        self.kind_of(d) == DeclKind::ParmVar
            && self.ast.decls.get(d).ty().is_some_and(|t| {
                matches!(self.ast.types.view(t.ty()), TypeView::PackExpansion { .. })
            })
    }

    // -- specializations, instantiations, guides -------------------------------------

    /// `[extern] template declaration` ([temp.explicit]): explicit
    /// instantiation is WP-070a.
    pub fn act_explicit_instantiation(
        &mut self,
        extern_loc: Option<Loc>,
        loc: Loc,
    ) -> Option<DeclId> {
        let what = if extern_loc.is_some() {
            "explicit instantiation declaration"
        } else {
            "explicit instantiation"
        };
        self.templ_unimplemented(what, loc);
        None
    }

    /// `template<> declaration` ([temp.expl.spec]) or a partial
    /// specialization `template<params> class X<args>`
    /// ([temp.spec.partial]): WP-070a.
    pub fn act_explicit_specialization(&mut self, partial: bool, loc: Loc) -> Option<DeclId> {
        let what = if partial {
            "class template partial specialization"
        } else {
            "explicit specialization"
        };
        self.templ_unimplemented(what, loc);
        None
    }

    /// A deduction guide ([temp.deduct.guide]): CTAD is WP-074.
    pub fn act_deduction_guide(&mut self, name: Symbol, loc: Loc) -> Option<DeclId> {
        let _ = sym_str(self, name);
        self.templ_unimplemented("deduction guide", loc);
        None
    }

    // -- dependent expression forms ([temp.dep.expr]) -----------------------------

    /// True if `e` is type-dependent ([temp.dep.expr]/1).
    pub fn expr_is_type_dependent(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).deps().contains(ExprDeps::TYPE)
    }

    /// True if `e` is value-dependent ([temp.dep.constexpr]).
    pub fn expr_is_value_dependent(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).deps().intersects(ExprDeps::TYPE.with(ExprDeps::VALUE))
    }

    /// True if an operand of type `t` defers operator lookup to
    /// instantiation: a dependent type that may become a class or
    /// enumeration (clang's `isOverloadableType`; dependent pointers,
    /// arrays, functions and member pointers keep built-in semantics).
    /// The type of a subscript with a type-dependent operand (clang's
    /// `getDependentArraySubscriptType`): with an integral or unscoped
    /// enumeration index, the other operand's pointee or element type if
    /// that is dependent; else `<dependent type>`.
    pub(crate) fn dependent_subscript_type(&self, base: ExprId, idx: ExprId) -> QualType {
        let (bt, it) = (self.ety(base), self.ety(idx));
        let index_like = |t: QualType| {
            !self.ast.types.is_dependent(t)
                && (self.ast.types.is_integral(t) || crate::conv::is_unscoped_enum(self, t))
        };
        // The pointee or element type, keeping sugar (clang's
        // `getAs<PointerType>` / `getAsArrayTypeUnsafe`).
        let elem = |t: QualType| {
            let mut cur = t;
            for _ in 0..256 {
                match self.ast.types.view(cur.ty()) {
                    TypeView::Pointer(p) | TypeView::IncompleteArray(p) => return Some(p),
                    TypeView::ConstantArray(a) => return Some(a.elem),
                    TypeView::VariableArray { elem, .. }
                    | TypeView::DependentSizedArray { elem, .. } => return Some(elem),
                    _ => {
                        let next = self.ast.types.desugar_once(cur);
                        if next == cur {
                            return None;
                        }
                        cur = next;
                    }
                }
            }
            None
        };
        let r = if index_like(it) {
            elem(bt)
        } else if index_like(bt) {
            elem(it)
        } else {
            None
        };
        r.filter(|&t| self.ast.types.is_dependent(t)).unwrap_or(QualType::DEPENDENT)
    }

    pub(crate) fn is_dependent_overloadable(&self, t: QualType) -> bool {
        use kiln_ast::types::TypeKind;
        self.ast.types.is_dependent(t)
            && !matches!(
                self.ast.types.canonical_kind(t),
                TypeKind::Pointer | TypeKind::Function | TypeKind::MemberPointer
            )
            && !self.ast.types.is_array(t)
    }

    /// A cast to a dependent type or of a type-dependent operand
    /// ([temp.dep.expr]/3): kept with cast kind `Dependent` and the
    /// operand unconverted. `T(x)` is a `CXXUnresolvedConstructExpr`.
    pub(crate) fn dependent_cast(
        &mut self,
        style: crate::expr::CastStyle,
        ty: QualType,
        e: ExprId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        use kiln_ast::expr::{CastKind, ExprFlags, ValueCategory};
        if style == crate::expr::CastStyle::Functional {
            return self.builder().unresolved_construct(ty, &[e], false, loc, rparen);
        }
        let vk = match self.ast.types.view(ty.ty()) {
            TypeView::LValueRef(_) => ValueCategory::LValue,
            TypeView::RValueRef(_) => ValueCategory::XValue,
            _ => ValueCategory::PRValue,
        };
        let to = self.non_reference(ty);
        let kind = style.kind();
        // A cast to `void` is never dependent ([expr.static.cast]/6;
        // clang's `ToVoid`).
        let ck = if self.ast.types.is_void(to) { CastKind::ToVoid } else { CastKind::Dependent };
        self.builder().cast(kind, ck, e, to, vk, None, loc, ExprFlags::NONE)
    }

    /// A braced-init-list kept unanalyzed in a dependent context: an
    /// `InitListExpr` of type `void` ([temp.dep.expr]/3).
    pub(crate) fn dependent_init_list(&mut self, l: &crate::init::InitList) -> ExprId {
        let mut inits = Vec::with_capacity(l.items.len());
        for it in &l.items {
            if !it.designator.is_empty() {
                self.templ_unimplemented("designated initializers in a template", l.lbrace);
            }
            let e = match &it.value {
                crate::init::InitValue::Expr(e) => *e,
                crate::init::InitValue::List(n) => self.dependent_init_list(n),
            };
            inits.push(e);
        }
        self.builder().init_list(QualType::VOID, &inits, None, None, l.lbrace, l.rbrace)
    }

    /// True if any initializer of `l` (recursively) is type-dependent.
    pub(crate) fn init_list_type_dependent(&self, l: &crate::init::InitList) -> bool {
        l.items.iter().any(|it| match &it.value {
            crate::init::InitValue::Expr(e) => self.expr_is_type_dependent(*e),
            crate::init::InitValue::List(n) => self.init_list_type_dependent(n),
        })
    }

    /// A call with a type-dependent callee or argument ([temp.dep.expr]/3,
    /// [temp.dep.candidate]): a `<dependent type>` prvalue `CallExpr`. An
    /// unqualified callee naming non-member functions becomes an
    /// `UnresolvedLookupExpr` with ADL.
    pub(crate) fn dependent_call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        use kiln_ast::expr::{ExprFlags, ValueCategory};
        let mut f = callee;
        let n = *self.ast.exprs.get(callee);
        if n.kind() == ExprKind::DeclRefExpr {
            let d = self.ast.exprs.decl(callee);
            let qualified =
                self.ast.exprs.decl_ref_extra(callee).is_some_and(|x| x.qualifier.is_some());
            if self.kind_of(d) == DeclKind::Function && !qualified {
                let name = self.ast.decls.get(d).name().as_ident();
                if let Some(name) = name {
                    f = self.builder().unresolved_lookup(name, &[d], true, None, None, n.loc());
                }
            }
        }
        let ty = QualType::DEPENDENT;
        self.builder().call(f, args, ty, ValueCategory::PRValue, loc, rparen, ExprFlags::NONE)
    }

    /// An unqualified name followed by `(` that ordinary lookup does not
    /// find, inside a template: argument-dependent lookup at instantiation
    /// may find it ([temp.dep.candidate]), so it becomes an empty
    /// `UnresolvedLookupExpr` with ADL. The call diagnoses it when no
    /// argument is type-dependent.
    pub fn act_on_undeclared_call_name(&mut self, name: Symbol, loc: Loc) -> ExprId {
        self.builder().unresolved_lookup(name, &[], true, None, None, loc)
    }

    /// For a call whose callee is an unresolved name: the diagnostics when
    /// no argument is type-dependent, or `None` when the call stays
    /// dependent. An empty lookup is an undeclared identifier
    /// ([basic.lookup.argdep]); a function template needs deduction
    /// (WP-071).
    pub(crate) fn check_unresolved_callee(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
    ) -> Option<ExprId> {
        let n = *self.ast.exprs.get(callee);
        if n.kind() != ExprKind::UnresolvedLookupExpr
            || args.iter().any(|&a| self.expr_is_type_dependent(a))
            || self.expr_is_type_dependent(callee)
        {
            return None;
        }
        let mut kids = vec![callee];
        kids.extend_from_slice(args);
        let decls = self.ast.unresolved_decls(callee);
        if decls.is_empty() {
            let name = self.ast.unresolved_name(callee);
            let text = sym_str(self, name);
            let msg = self.diag(DiagId::sema_undeclared_var_use, n.loc()).arg(text);
            self.emit(msg);
        } else {
            self.templ_unimplemented("calls to function templates", n.loc());
        }
        Some(self.recover_err(&kids, n.loc()))
    }

    /// `base.name` / `base->name` on a type-dependent object
    /// ([temp.dep.expr]/5): a `CXXDependentScopeMemberExpr`; `->` on a
    /// pointer to a dependent type converts the pointer first. `None` if
    /// the object is not dependent.
    pub fn try_dependent_member(
        &mut self,
        base: ExprId,
        name: Symbol,
        targs: Option<ArgListId>,
        template_kw: bool,
        loc: Loc,
        arrow: bool,
    ) -> Option<ExprId> {
        let bt = self.ety(base);
        if !self.ast.types.is_dependent(bt) {
            return None;
        }
        // A member of the current instantiation found by lookup is an
        // ordinary member access ([temp.dep.expr]/5, clang's MemberExpr).
        if targs.is_none() && !template_kw {
            let obj = if arrow {
                self.pointee_sugared(bt).filter(|_| self.ast.types.is_pointer(bt))
            } else {
                Some(bt)
            };
            if let Some(rec) = obj.and_then(|o| self.ast.types.as_record(o))
                && self.lookup_in_class(rec, name, crate::lookup::LookupKind::Ordinary).decls.len()
                    == 1
            {
                return None;
            }
        }
        let b = if arrow && !self.is_dependent_overloadable(bt) {
            crate::conv::decay_dependent(self, base)
        } else {
            base
        };
        Some(self.builder().dependent_member(Some(b), arrow, name, template_kw, None, targs, loc))
    }
    /// Parameter `p` (the `idx`th) of the requires-expression whose body
    /// is `body` ([expr.prim.req.general]/4): a `ParmVarDecl` in the body,
    /// visible to the requirements.
    pub fn act_on_requires_param(
        &mut self,
        body: DeclId,
        p: &crate::declspec::ParamInfo,
        idx: u16,
    ) -> DeclId {
        let name = p.name.map_or(DeclName::EMPTY, |n| DeclName::ident(n.0));
        let d = self.builder().param(body, name, p.loc, p.ty, idx, 0);
        if p.invalid {
            self.ast.decls.set_flags(d, DeclFlags::INVALID);
        }
        if let Some((n, _)) = p.name {
            self.push_local(n, d);
        }
        d
    }

    /// True if `t` names a parameter pack that is not expanded
    /// ([temp.variadic]/7; clang's `containsUnexpandedParameterPack`).
    pub fn has_unexpanded_pack(&self, t: QualType) -> bool {
        self.ast.types.flags(t.ty()).contains(TypeFlags::UNEXPANDED_PACK)
    }

    /// The profile of a value-dependent expression `e` (for dependent
    /// array bounds and `noexcept` operands).
    pub(crate) fn dependent_profile(&mut self, e: ExprId) -> kiln_ast::ProfileId {
        if let ArgView::Expr { profile: Some(p), .. } = self.builder().template_arg_expr(e).view() {
            return p;
        }
        let f = TypeFlags::DEPENDENT_INSTANTIATION;
        self.ast.types.profile(&[e.index()], e, f)
    }

    /// True if class template `tmpl`'s name, used without arguments here,
    /// is its injected-class-name: inside its definition ([temp.local]/1).
    pub fn names_injected_class(&self, tmpl: DeclId) -> bool {
        self.kind_of(tmpl) == DeclKind::ClassTemplate
            && self.pattern_of(tmpl).is_some_and(|p| self.encloses(p, self.ctx))
    }

    /// The injected-class-name of class template `tmpl` used without
    /// arguments inside its own definition ([temp.local]/1): `X<P...>`
    /// for its parameters `P`, as an `InjectedClassNameType`. `None`
    /// outside the definition.
    pub fn injected_class_type(&mut self, tmpl: DeclId) -> Option<QualType> {
        if !self.names_injected_class(tmpl) {
            return None;
        }
        self.injected_type_of(tmpl)
    }

    /// The type of `this`'s pointee in a member of class `class`
    /// ([expr.prim.this]/2): for the pattern of a class template, its
    /// injected-class-name type `X<P...>` (as clang); else the class type.
    pub(crate) fn this_class_type(&mut self, class: DeclId) -> QualType {
        let first = self.ast.first(class);
        if self.kind_of(class) == DeclKind::CXXRecord
            && let Some(t) = self.ast.described_template(first)
            && let Some(q) = self.injected_type_of(t)
        {
            return q;
        }
        self.tag_type(class)
    }

    /// `X<P...>` for class template `tmpl`'s own parameters `P`, as an
    /// `InjectedClassNameType`.
    fn injected_type_of(&mut self, tmpl: DeclId) -> Option<QualType> {
        let pat = self.pattern_of(tmpl)?;
        let info = self.ast.template_info(tmpl).copied()?;
        let list = *self.ast.templates.param_list(info.params);
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
        let mut args = Vec::with_capacity(params.len());
        for p in params {
            let pack = self.ast.template_parm(p).is_some_and(|i| i.pack);
            let loc = self.ast.decls.get(p).loc();
            let a = match self.kind_of(p) {
                DeclKind::TemplateTypeParm => {
                    let t = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                    TemplateArg::ty(if pack { self.ast.types.pack_expansion(t, None) } else { t })
                }
                DeclKind::NonTypeTemplateParm => {
                    // Built directly: naming the parameter here is not a
                    // reference to it (clang leaves it unreferenced).
                    let mut t = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                    if let TypeView::PackExpansion { pattern, .. } = self.ast.types.view(t.ty()) {
                        t = pattern;
                    }
                    let t = self.non_reference(t);
                    let mut e = self.builder().decl_ref(
                        p,
                        t,
                        loc,
                        None,
                        kiln_ast::expr::NonOdrUse::None,
                        kiln_ast::expr::ExprFlags::NONE,
                    );
                    if pack {
                        e = self.act_pack_expansion(e, loc);
                    }
                    self.builder().template_arg_expr(e)
                }
                _ => {
                    let n = self.template_name_for(p);
                    if pack {
                        TemplateArg::template_expansion(n, None)
                    } else {
                        TemplateArg::template(n)
                    }
                }
            };
            args.push(a);
        }
        let name = self.template_name_for(tmpl);
        let list = self.ast.types.arg_list(&args);
        let spec = self.ast.types.template_specialization(name, list, None, false);
        let pat = self.ast.definition(pat).unwrap_or(pat);
        Some(self.ast.types.injected_class_name(pat, spec.ty()).into())
    }
}
