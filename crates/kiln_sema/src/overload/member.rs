//! Calls of overloaded member functions ([over.match.call]/2,
//! [over.match.funcs]/4-5; spec/sema-overload.md §2.3): `b.f(…)`,
//! `p->f(…)` and the implicit `this->f(…)` of a member name, where `f`
//! names more than one member function. The member access is a
//! `MemberExpr` of type [`QualType::OVERLOAD`] (kiln's
//! `UnresolvedMemberExpr`) whose set [`OvlMember`] records; the call
//! resolves it against its arguments and the object argument, and the
//! winner becomes the ordinary member access the call path binds.

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use super::{ArgInfo, CandidateKind, IcsOptions, OvlMember, SetKind};
use crate::Sema;
use crate::class::MemberRef;
use crate::init::InitList;

impl Sema<'_, '_> {
    /// Whether the members `decls` form a member function overload set
    /// (functions and function templates, at least one of them a member;
    /// a lone member function template is one too, resolved at the call).
    pub(crate) fn is_member_overload_set(&self, decls: &[DeclId]) -> bool {
        (decls.len() > 1 || decls.iter().any(|&d| self.kind_of(d) == DeclKind::FunctionTemplate))
            && decls.iter().all(|&d| {
                let k = self.kind_of(self.templated_or_self(d));
                k.is_function() && k != DeclKind::Function
            })
    }

    pub(crate) fn templated_or_self(&self, d: DeclId) -> DeclId {
        if self.kind_of(d) == DeclKind::FunctionTemplate {
            self.pattern_of(d).unwrap_or(d)
        } else {
            d
        }
    }

    /// The overloaded member access of `m` on the object `base` (a pointer
    /// for `m.arrow`) at `loc`: a `MemberExpr` of overloaded type naming
    /// the first member.
    pub(crate) fn build_member_overload(&mut self, m: OvlMember, loc: Loc) -> ExprId {
        let e = self.builder().member(m.base, m.decls[0], QualType::OVERLOAD, m.arrow, loc, None);
        self.ovl.members.insert(e, m);
        e
    }

    /// The overloaded member access `e` names (through parentheses).
    pub(crate) fn member_overload(&self, e: ExprId) -> Option<ExprId> {
        if self.ast.exprs.get(e).ty() != QualType::OVERLOAD {
            return None;
        }
        let x = self.ignore_parens(e);
        (self.ast.exprs.get(x).kind() == ExprKind::MemberExpr && self.ovl.members.contains_key(&x))
            .then_some(x)
    }

    /// Whether `e` is an overloaded member access whose set has a member
    /// function template (so `e.f<args>` is a template-id,
    /// [temp.names]/3).
    pub fn is_member_template_ref(&self, e: ExprId) -> bool {
        self.member_overload(e).is_some_and(|x| {
            self.member_overload_decls(x).iter().any(|&d| {
                matches!(self.kind_of(d), DeclKind::FunctionTemplate | DeclKind::VarTemplate)
            })
        })
    }

    /// The one non-template member the member access `e` names (clang's
    /// `MemberExpr`, not an overload set or a dependent access), which a
    /// `<` after may have been meant to follow a template name
    /// ([temp.names]/3; clang's `mightBeIntendedToBeTemplateName`).
    pub fn member_named_non_template(&self, e: ExprId) -> Option<DeclId> {
        let n = self.ast.exprs.get(e);
        if n.kind() != ExprKind::MemberExpr || self.ast.templates.expr_targs.get(e).is_some() {
            return None;
        }
        let d = if n.ty() == QualType::OVERLOAD {
            match self.member_overload_decls(e) {
                [d] => *d,
                _ => return None,
            }
        } else {
            self.ast.exprs.decl(e)
        };
        let d = self.ast.strip_shadow(d);
        (!matches!(
            self.kind_of(d),
            DeclKind::FunctionTemplate | DeclKind::VarTemplate | DeclKind::ClassTemplate
        ))
        .then_some(d)
    }

    /// Records the explicit template arguments `targs` of the member
    /// template-id `e` (see [`Self::is_member_template_ref`]); false
    /// (nothing recorded) for dependent arguments, which a member
    /// access of a non-dependent object does not take yet.
    pub fn set_member_template_args(&mut self, e: ExprId, targs: kiln_ast::ids::ArgListId) -> bool {
        if self
            .ast
            .types
            .arg_list_flags(targs)
            .intersects(kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION)
        {
            return false;
        }
        let Some(x) = self.member_overload(e) else { return false };
        match self.ovl.members.get_mut(&x) {
            Some(m) => {
                m.targs = Some(targs);
                true
            }
            None => false,
        }
    }

    /// The member functions of the overloaded member access `x`.
    pub(crate) fn member_overload_decls(&self, x: ExprId) -> &[DeclId] {
        self.ovl.members.get(&x).map_or(&[], |m| &m.decls)
    }

    /// The qualified overloaded member name `x` (an implicit `this->C::f`)
    /// as the `UnresolvedLookupExpr` `C::f` it was written as.
    pub(crate) fn member_overload_as_lookup(&mut self, x: ExprId) -> Option<ExprId> {
        let nns = self.ovl.members.get(&x)?.nns?;
        let m = self.ovl.members.remove(&x)?;
        let loc = self.ast.exprs.get(x).loc();
        Some(self.build_overload_ref(
            m.name,
            &m.decls,
            &m.found,
            Some(m.naming),
            false,
            Some(nns),
            loc,
        ))
    }

    /// The template-id `name<targs>` (qualified by `nns`, if any), whose
    /// lookup found the member functions and member function templates
    /// `cands` (with their found decls; `naming` the naming class if
    /// recorded), used as a callee where an implicit `this` may be used
    /// and its class is (derived from) their class ([class.mfct.non.static]/2,
    /// [expr.prim.id.general]/3): the overloaded member access
    /// `(*this).name<targs>`, whose call ranks the implicit object
    /// parameters against `*this` ([over.match.funcs]/4-5), as
    /// `this->name<targs>(…)` does. A type-dependent `this` (a member of
    /// a class template) or a dependent base on the way to the members'
    /// class gives the dependent `this->name<targs>`. `None` when no
    /// candidate is an instance member or no such `this` exists (the
    /// call is then resolved without an object argument).
    pub(crate) fn implicit_member_template_id(
        &mut self,
        name: kiln_ast::names::DeclName,
        cands: &[(DeclId, DeclId)],
        naming: Option<DeclId>,
        nns: Option<kiln_ast::NnsId>,
        targs: Option<kiln_ast::ids::ArgListId>,
        loc: Loc,
    ) -> Option<ExprId> {
        let decls: Vec<DeclId> = cands.iter().map(|&(d, _)| d).collect();
        if !decls.iter().all(|&d| {
            let k = self.kind_of(self.templated_or_self(d));
            k.is_function() && k != DeclKind::Function
        }) {
            return None;
        }
        let &first = decls.iter().find(|&&d| self.is_instance_member(self.templated_or_self(d)))?;
        let first = self.templated_or_self(first);
        // With a static candidate too, `this` is captured only once the
        // call names an instance member ([`Self::finish_member_winner`];
        // at instantiation for a dependent `this`): the static `f` of
        // `[](int s) { return f(s); }` needs none (clang's
        // `BuildPossibleImplicitMemberExpr`).
        let all_instance =
            decls.iter().all(|&d| self.is_instance_member(self.templated_or_self(d)));
        let Some(class) = self.implicit_object_class(first) else {
            let this_ty = self.dependent_implicit_this(first)?;
            if all_instance {
                self.note_this_use(loc);
            }
            let this = self.builder().this_expr(this_ty, true, loc);
            let b = crate::conv::decay_dependent(self, this);
            return Some(self.builder().dependent_member(
                Some(b),
                true,
                name,
                false,
                nns,
                targs,
                loc,
            ));
        };
        let (this_ty, _) = self.this_class()?;
        let obj_ty = self.ast.types.pointee(this_ty).unwrap_or(QualType::ERROR);
        if all_instance {
            self.note_this_use(loc);
        }
        let this = self.builder().this_expr(this_ty, true, loc);
        if self.ast.types.is_dependent(obj_ty) {
            // [temp.dep.expr]/3: `this` is type-dependent.
            let b = crate::conv::decay_dependent(self, this);
            return Some(self.builder().dependent_member(
                Some(b),
                true,
                name,
                false,
                nns,
                targs,
                loc,
            ));
        }
        // [class.access.base]/5: a qualified name is named in its
        // qualifier's class, an unqualified one in the class of `*this`.
        let qualifier_class = nns.and_then(|q| match self.ast.types.nns_view(q) {
            kiln_ast::types::Nns::Type { ty, .. } => {
                let r = self.ast.types.as_record(ty)?;
                Some(self.ast.definition(r).unwrap_or(r))
            }
            _ => None,
        });
        let set = OvlMember {
            name,
            decls,
            found: cands.iter().map(|&(_, f)| f).collect(),
            naming: naming.or(qualifier_class).unwrap_or(class),
            obj_ty,
            def: class,
            base: this,
            arrow: true,
            nns,
            implicit: true,
            targs,
        };
        Some(self.build_member_overload(set, loc))
    }

    /// Whether every candidate of `cands` is a member function (template)
    /// with an implicit object parameter ([dcl.fct]/6, not static, no
    /// explicit object parameter) and no implicit `this` can be their
    /// object ([class.mfct.non.static]/2): none may be used here, or its
    /// class is not (and cannot become, through a dependent base)
    /// derived from theirs.
    pub(crate) fn lacks_implicit_object(&self, cands: &[(DeclId, DeclId)]) -> bool {
        let Some(&(first, _)) = cands.first() else { return false };
        cands.iter().all(|&(d, _)| {
            let f = self.templated_or_self(d);
            self.kind_of(f) == DeclKind::CXXMethod
                && !self.is_static_method(f)
                && !self.has_explicit_object(f)
        }) && {
            let first = self.templated_or_self(first);
            self.implicit_object_class(first).is_none()
                && self.dependent_implicit_this(first).is_none()
        }
    }

    /// The dependent call of the name `callee` (`Get<decltype(v)>(0)`,
    /// an `UnresolvedLookupExpr`) whose candidates include member
    /// functions (templates) with an implicit object parameter of a class
    /// `*this` is (derived from): an enclosing lambda captures `this` now
    /// ([expr.prim.lambda.capture]/8), since its call operator may be
    /// instantiated only after the closure is complete. As clang: when
    /// every candidate is such a member the implicit `this->` is known
    /// and its capture required (`BuildImplicitMemberExpr` with
    /// `IsKnownInstance`); with a static candidate too `this` is captured
    /// only where no diagnostic is needed (a potential capture).
    ///
    /// An implicit member overload set (`f(s)` in a generic lambda, `s` a
    /// generic parameter, `f` static and instance members) whose call is
    /// dependent: when every enclosing lambda can capture `this` it does
    /// so now, since the closure is complete before the call is resolved;
    /// otherwise nothing is captured and an instance winner is diagnosed
    /// at instantiation (clang
    /// `tryImplicitlyCaptureThisIfImplicitMemberFunctionAccessWithDependentArgs`,
    /// [expr.prim.lambda.capture]/7-8). A set of instance members only
    /// captured `this` when it was formed.
    pub(crate) fn note_implicit_this_of_dependent_call(&mut self, callee: ExprId) {
        let c = self.ignore_parens(callee);
        if self.lambdas.is_empty() || self.unevaluated > 0 {
            return;
        }
        if let Some(x) = self.member_overload(c) {
            let implicit = self.ovl.members.get(&x).is_some_and(|m| m.implicit);
            if implicit {
                let loc = self.ast.exprs.get(x).loc();
                self.note_potential_this(loc);
            }
            return;
        }
        if self.ast.exprs.get(c).kind() != ExprKind::UnresolvedLookupExpr {
            return;
        }
        let instance: Vec<bool> = self
            .ast
            .unresolved_decls(c)
            .iter()
            .map(|&d| {
                let f = self.templated_or_self(self.ast.strip_shadow(d));
                self.kind_of(f) == DeclKind::CXXMethod
                    && !self.is_static_method(f)
                    && !self.has_explicit_object(f)
            })
            .collect();
        let decls = self.ast.unresolved_decls(c);
        let Some(i) = instance.iter().position(|&b| b) else { return };
        let first = self.templated_or_self(self.ast.strip_shadow(decls[i]));
        if self.implicit_object_class(first).is_none() {
            return;
        }
        let loc = self.ast.exprs.get(c).loc();
        if instance.iter().all(|&b| b) {
            self.note_this_use(loc);
        } else {
            self.note_potential_this(loc);
        }
    }

    /// A potential use of `this` at `loc` by a dependent call of an
    /// overload set with static and instance members: captured by the
    /// innermost capture-ready lambda and the lambdas enclosing it when
    /// every open lambda can capture it, else not at all.
    fn note_potential_this(&mut self, loc: Loc) {
        if self.this_capturable()
            && let Some(op) = self.capture_ready_call_op()
        {
            let saved = std::mem::replace(&mut self.ctx, op);
            self.note_this_use(loc);
            self.ctx = saved;
        }
    }

    /// The call operator of the innermost open lambda whose closure type
    /// is not in a dependent context (clang
    /// `getStackIndexOfNearestEnclosingCaptureReadyLambda`): a capture
    /// there is final, while a lambda in a template is formed again, with
    /// its captures, at instantiation. `None` outside a lambda.
    fn capture_ready_call_op(&self) -> Option<DeclId> {
        let mut c = self.ctx;
        for _ in 0..4096 {
            let Some(l) = self.ast.call_op_lambda(c) else {
                if self.kind_of(c) == DeclKind::RequiresExprBody
                    && let Some(p) = self.ast.decls.get(c).ctx()
                {
                    c = p;
                    continue;
                }
                return None;
            };
            let closure = self.ast.exprs.lambdas.get(l).closure;
            if !self.ast.decls.get(closure).has(DeclFlags::DEPENDENT_CTX) {
                return Some(c);
            }
            c = self.ast.decls.get(closure).ctx()?;
        }
        None
    }

    /// Resolves the call of the overloaded member access `callee`
    /// (possibly parenthesized) with `args`: the member access naming the
    /// best viable function, or `None` after diagnosing.
    pub(crate) fn resolve_member_call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        braced: &[Option<InitList>],
    ) -> Option<ExprId> {
        let x = self.member_overload(callee)?;
        let m = self.ovl.members.remove(&x)?;
        let loc = self.ast.exprs.get(x).loc();
        let name = m.name;
        let quals = self.ast.types.effective_quals(m.obj_ty);
        let vk = if m.arrow {
            ValueCategory::LValue
        } else {
            self.ast.exprs.get(m.base).value_category()
        };
        let mut set = self.begin_set(SetKind::Normal, loc);
        self.defer_templates(&mut set);
        let mark = self.push_call_args(&mut set, args, braced);
        let obj = self.ast.types.qualify(self.tag_type(m.def), quals);
        set.object = Some(ArgInfo::of_type(obj, vk));
        for (&d, &f) in m.decls.iter().zip(&m.found) {
            if self.ast.decls.get(d).has(DeclFlags::INVALID) {
                continue;
            }
            if self.ast.decls.cold.requires_clause.contains(d) {
                set.uncertain = true;
            }
            if self.kind_of(d) == DeclKind::FunctionTemplate {
                // A member function template: its specialization deduced
                // from the call's arguments ([temp.deduct.call]).
                if set.two_phase && m.targs.is_none() {
                    set.deferred.push(super::defer::Deferred::Template {
                        ft: d,
                        found: f,
                        kind: CandidateKind::Method,
                        first_opts: IcsOptions::NONE,
                        loc,
                        poi: self.templ.deduce_poi,
                        adl_only: false,
                    });
                    continue;
                }
                self.add_template_candidate(
                    &mut set,
                    d,
                    f,
                    m.targs,
                    CandidateKind::Method,
                    IcsOptions::NONE,
                    loc,
                );
                continue;
            }
            // A template-id names only the templates ([temp.arg.explicit]/4;
            // clang's `AddOverloadedCallCandidates`).
            if m.targs.is_some() {
                continue;
            }
            let n = set.cands.len();
            self.add_candidate(&mut set, d, f, CandidateKind::Method, IcsOptions::NONE);
            // `x.operator const char *()` names the specializations its
            // conversion templates deduce ([temp.mem]/6): they stay
            // template specializations for [over.match.best.general]/2.4-5
            // (clang `getPrimaryTemplate`), ordered by their conversion
            // types ([temp.deduct.partial]/3.2).
            if self.kind_of(d) == DeclKind::CXXConversion
                && let Some((_, info)) = self.ast.spec_info(d)
                && let Some(c) = set.cands.get_mut(n)
            {
                c.template = Some(info.template);
            }
        }
        let out = self.resolve_set(&mut set);
        self.ovl.lists.truncate(mark);
        let won = self.call_winner(&set, out, name, loc, loc, true);
        let won = won.map(|b| {
            let c = set.cand(b);
            (c.func(), c.found)
        });
        self.end_set(set);
        let (d, f) = won?;
        let e = self.finish_member_winner(&m, d, f, loc);
        self.ovl.resolved_member = Some(e);
        Some(e)
    }

    /// The member access naming `d` (found as `f`) that the overloaded
    /// member access `m` at `loc` resolves to, for a call or a target
    /// type ([over.over]): a `DeclRefExpr` for a static member of an
    /// implicit `this`, as clang.
    pub(crate) fn finish_member_winner(
        &mut self,
        m: &OvlMember,
        d: DeclId,
        f: DeclId,
        loc: Loc,
    ) -> ExprId {
        self.check_member_access(f, m.naming, Some(m.def), loc);
        let shadow = (f != d).then_some(f);
        if self.check_undeduced_use(d, loc) {
            return self.recover_err(&[m.base], loc);
        }
        if m.implicit && self.is_static_method(d) {
            let e = self.build_decl_ref_qualified(d, loc, shadow, m.nns);
            self.note_function_name_use(d, e, loc, m.nns.is_none());
            return e;
        }
        if m.implicit {
            // An implicit `this->` naming an instance member: the
            // enclosing lambdas capture `this` (already done when every
            // candidate was one).
            let mixed =
                !m.decls.iter().all(|&x| self.is_instance_member(self.templated_or_self(x)));
            if mixed
                && self.lambdas.is_empty()
                && self.unevaluated == 0
                && self.ast.call_op_lambda(self.ctx).is_some()
            {
                // A generic lambda's call operator being instantiated:
                // its closure is complete, so `this` must have been
                // captured when the lambda was formed (a potential
                // capture, [`Self::note_implicit_this_of_dependent_call`]);
                // without a capture-default it is an error here, as
                // clang's `CheckCXXThisCapture` from the instantiation.
                let _ = self.capture_this(loc, None);
            } else {
                self.note_this_use(loc);
            }
        }
        // [dcl.attr.deprecated]/4: a member access naming a member
        // function template warns for the template, then for the
        // specialization (clang `FixOverloadedFunctionReference` of the
        // member expression runs `DiagnoseUseOfDecl` on the found
        // declaration; a static member named through an implicit `this`
        // above becomes a `DeclRefExpr` and warns only for the
        // specialization).
        if self.kind_of(f) == DeclKind::FunctionTemplate {
            self.warn_deprecated_template_use(f, loc);
        }
        self.diagnose_deprecated(d, loc);
        let e = self.member_of_object(m.base, m.obj_ty, m.def, m.arrow, d, shadow, m.nns, loc);
        if let Some(targs) = m.targs
            && self.ast.exprs.get(e).kind() == ExprKind::MemberExpr
        {
            // Kept as written for printing (`s.get<0>()`, clang's
            // `StmtPrinter::VisitMemberExpr`).
            let _ = self.ast.exprs.set_member_template_args(e, targs);
        }
        if m.nns.is_some() && !self.is_err_expr(e) {
            self.member_refs
                .insert(e, MemberRef { decl: d, error: false, qualified: true, begin: None });
        }
        e
    }
}
