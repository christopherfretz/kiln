//! Matching a template template argument under clang 22's rules
//! ([temp.arg.template]/3-4, CWG2398, P2113; spec/sema-templates.md
//! §3.2.1). Used at `-fclang-compat=22` and above.
//!
//! clang 22's `isTemplateTemplateParameterAtLeastAsSpecializedAs`:
//! under a `PartialOrderingTTP` context (every diagnostic inside it
//! carries "template template argument has different template
//! parameters than its corresponding template template parameter" at
//! the argument and "previous template template parameter is here" at
//! `P`), `P`'s parameters, written as arguments at their own locations
//! (a pack as a pack expansion), are checked against `A`'s parameter
//! list as for a template-id `A<...>` named at the argument
//! (`CheckTemplateArgumentList` in `MatchingTTP` mode): an argument of
//! the wrong kind, a non-type argument whose (non-dependent) type does
//! not convert to the parameter's as a converted constant expression,
//! too few or too many arguments; a pack expansion takes every
//! remaining parameter. Then `A`'s parameters are deduced from those
//! arguments: a non-type parameter of `A` typed by `A`'s own type
//! parameter conflicts with a `P` argument of another type ("conflicting
//! deduction"). Last, a constrained `P` must be at least as constrained
//! as `A` ([temp.arg.template]/3, P2113).

use kiln_ast::decl::DeclKind;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, QualType, TemplateParamListId};
use kiln_diag::DiagId;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::templ::args::MultiLevelArgs;
use crate::templ::stack::FrameKind;

/// The prior arguments of the template that declares `P` (`p`) and of the
/// one that declares `A` (`a`); a nested template template parameter swaps
/// them, since its `A`-side parameter becomes the `P` of the inner check.
#[derive(Clone, Copy)]
pub(super) struct Priors<'m> {
    pub(super) p: &'m MultiLevelArgs,
    pub(super) a: &'m MultiLevelArgs,
}

impl Sema<'_, '_> {
    /// [temp.arg.template]/3 under clang 22: whether template `a`
    /// matches template template parameter `p` (lists `al`, `pl`),
    /// diagnosing a mismatch; `at` is the argument.
    pub(super) fn check_template_template_arg_22(
        &mut self,
        p: DeclId,
        a: DeclId,
        lists: (TemplateParamListId, TemplateParamListId),
        at: Loc,
        pr: Priors<'_>,
    ) -> bool {
        let (pl, al) = lists;
        if !self.push_inst_frame(FrameKind::PartialOrderingTtp, p, a, at) {
            return false;
        }
        let ok = self.ttp22_match_list(a, pl, al, at, pr) && self.ttp22_deduce(pl, al);
        self.pop_inst_frame();
        ok && self.ttp22_constraints(p, a, pl, al, at)
    }

    /// `P`'s parameters as arguments for `A`'s list
    /// (`CheckTemplateArgumentList`, `MatchingTTP`); stops at the first
    /// error, as clang does.
    fn ttp22_match_list(
        &mut self,
        a: DeclId,
        pl: TemplateParamListId,
        al: TemplateParamListId,
        at: Loc,
        pr: Priors<'_>,
    ) -> bool {
        let (xs, ys) = (self.ttp_list_params(pl), self.ttp_list_params(al));
        let mut i = 0usize;
        for (k, &y) in ys.iter().enumerate() {
            if self.ttp_is_pack(y) {
                // A parameter pack takes every remaining argument.
                while let Some(&x) = xs.get(i) {
                    if !self.ttp22_arg_fits(x, y, pr) {
                        return false;
                    }
                    i += 1;
                }
                continue;
            }
            let Some(&x) = xs.get(i) else {
                if self.template_default(y).is_some() {
                    continue;
                }
                self.diag_template_arity(a, false, at);
                return false;
            };
            if self.ttp_is_pack(x) {
                // A pack expansion's pattern fills this and every later
                // parameter (clang: `ArgIsExpansion && MatchingTTP`).
                return ys[k..].iter().all(|&y| self.ttp22_arg_fits(x, y, pr));
            }
            if !self.ttp22_arg_fits(x, y, pr) {
                return false;
            }
            i += 1;
        }
        // Trailing pack expansions may be empty (clang keeps them under
        // `PartialOrderingTTP`): `template<class, class...> class` takes
        // `template<class> class`.
        if xs[i.min(xs.len())..].iter().any(|&x| !self.ttp_is_pack(x)) {
            self.diag_template_arity(a, true, at);
            return false;
        }
        true
    }

    fn ttp_list_params(&self, l: TemplateParamListId) -> Vec<DeclId> {
        let l = *self.ast.templates.param_list(l);
        self.ast.lists.iter::<DeclId>(l.params).collect()
    }

    fn ttp_is_pack(&self, d: DeclId) -> bool {
        self.ast.template_parm(d).is_some_and(|x| x.pack)
    }

    /// clang `CheckTemplateArgument` for `P`'s parameter `x` (as written
    /// at its location) against `A`'s parameter `y`.
    fn ttp22_arg_fits(&mut self, x: DeclId, y: DeclId, pr: Priors<'_>) -> bool {
        let xl = self.ast.decls.get(x).loc();
        let yl = Span::empty(self.ast.decls.get(y).loc());
        match (self.kind_of(y), self.kind_of(x)) {
            (DeclKind::TemplateTypeParm, DeclKind::TemplateTypeParm)
            | (DeclKind::NonTypeTemplateParm, DeclKind::NonTypeTemplateParm)
            | (DeclKind::TemplateTemplateParm, DeclKind::TemplateTemplateParm) => {}
            (DeclKind::TemplateTypeParm, _) => {
                let d = self
                    .diag(DiagId::sema_template_arg_must_be_type, xl)
                    .note(DiagId::note_template_param_here, yl);
                self.emit(d);
                return false;
            }
            (DeclKind::NonTypeTemplateParm, _) => {
                let d = self
                    .diag(DiagId::sema_template_arg_must_be_expr, xl)
                    .note(DiagId::note_template_param_here, yl);
                self.emit(d);
                return false;
            }
            _ => {
                self.error(DiagId::sema_template_arg_must_be_template, xl);
                return false;
            }
        }
        match self.kind_of(y) {
            DeclKind::NonTypeTemplateParm => self.ttp22_nttp_fits(x, y, pr),
            DeclKind::TemplateTemplateParm => {
                // `y` is now the parameter and `P`'s `x` its argument, so
                // the sides' prior arguments swap.
                let yl = self.ast.template_parm(y).and_then(|t| t.params);
                let xll = self.ast.template_parm(x).and_then(|t| t.params);
                let (Some(yl), Some(xll)) = (yl, xll) else { return true };
                self.check_template_template_arg_22(
                    y,
                    x,
                    (yl, xll),
                    xl,
                    Priors { p: pr.a, a: pr.p },
                )
            }
            _ => true,
        }
    }

    /// A value-dependent reference to `P`'s non-type parameter `x` as a
    /// converted constant expression of `A`'s parameter `y`'s type
    /// (clang `CheckConvertedConstantExpression` on a non-type-dependent
    /// argument): only the conversion is checked. A dependent type on
    /// either side, or one clang would check by value, passes.
    fn ttp22_nttp_fits(&mut self, x: DeclId, y: DeclId, pr: Priors<'_>) -> bool {
        let tx = self.ttp22_nttp_type(x, pr.p);
        let ty = self.ttp22_nttp_type(y, pr.a);
        let (Some(tx), Some(ty)) = (tx, ty) else { return true };
        if self.is_dependent_type(tx) || self.contains_undeduced_auto(tx) {
            return true;
        }
        let xl = self.ast.decls.get(x).loc();
        let yl = Span::empty(self.ast.decls.get(y).loc());
        let t = &self.ast.types;
        if self.contains_undeduced_auto(ty) {
            // `auto*` deduced from a non-pointer ([temp.param]/6.3).
            if t.is_pointer(ty) && !t.is_pointer(tx) {
                let d = self
                    .diag(DiagId::sema_nttp_deduction_failed, xl)
                    .arg(self.decl_name(y))
                    .arg(self.ty_arg(ty))
                    .arg(self.ty_arg(tx))
                    .note(DiagId::note_template_param_here, yl);
                self.emit(d);
                return false;
            }
            return true;
        }
        if self.is_dependent_type(ty) || t.same_unqualified(tx, ty) {
            return true;
        }
        let int_like =
            |q| t.is_integral(q) || t.as_enum(q).is_some_and(|e| !self.is_scoped_enum(e));
        let id = if t.is_integral(ty) && int_like(tx) {
            return true;
        } else if t.is_pointer(ty) && t.is_integral(tx) {
            // A value-dependent integer may be a null pointer constant,
            // whose conversion a converted constant expression forbids.
            DiagId::sema_cce_conversion_not_allowed
        } else if (t.is_pointer(ty) && t.is_pointer(tx))
            || t.as_enum(ty).is_some()
            || (t.is_integral(ty) && (t.is_pointer(tx) || t.as_enum(tx).is_some()))
        {
            DiagId::sema_cce_not_implicitly_convertible
        } else {
            return true;
        };
        let d = self
            .diag(id, xl)
            .arg(self.ty_arg(tx))
            .arg(self.ty_arg(ty))
            .note(DiagId::note_template_param_here, yl);
        self.emit(d);
        false
    }

    /// `P`'s non-type parameter `x`'s type with the enclosing template's
    /// prior arguments substituted (clang matches the substituted `P`).
    fn ttp22_nttp_type(&mut self, x: DeclId, prior: &MultiLevelArgs) -> Option<QualType> {
        let ty = self.ast.decls.get(x).ty()?;
        if prior.substituted() == 0 || !self.is_instantiation_dependent_type(ty) {
            return Some(ty);
        }
        let at = self.ast.decls.get(x).loc();
        Some(crate::templ::subst::Substituter::new(self, prior).ty(ty, at).unwrap_or(ty))
    }

    /// The deduction of `A`'s parameters from `P`'s
    /// (`DeduceTemplateArguments` in partial ordering): a non-type
    /// parameter of `A` whose type is `A`'s type parameter `U` deduces
    /// `U` from the matching argument's type, which must agree with the
    /// argument `U` itself received. Lists with packs are not checked.
    fn ttp22_deduce(&mut self, pl: TemplateParamListId, al: TemplateParamListId) -> bool {
        let (xs, ys) = (self.ttp_list_params(pl), self.ttp_list_params(al));
        if xs.iter().chain(&ys).any(|&d| self.ttp_is_pack(d)) {
            return true;
        }
        for (&x, &y) in xs.iter().zip(&ys) {
            if self.kind_of(y) != DeclKind::NonTypeTemplateParm {
                continue;
            }
            let Some(ty) = self.ast.decls.get(y).ty() else { continue };
            let TypeView::TemplateTypeParm(u) = self.ast.types.view(ty.ty()) else { continue };
            let Some(k) = ys.iter().position(|&d| d == u) else { continue };
            let Some(&xu) = xs.get(k) else { continue };
            if self.kind_of(xu) != DeclKind::TemplateTypeParm {
                continue;
            }
            // `P`'s own parameters are compared unsubstituted.
            let (Some(deduced), Some(tx)) =
                (self.ast.decls.get(xu).ty(), self.ast.decls.get(x).ty())
            else {
                continue;
            };
            if self.ast.types.same_unqualified(deduced, tx)
                || matches!(self.ast.types.view(tx.ty()), TypeView::TemplateTypeParm(d) if d == xu)
            {
                continue;
            }
            let d = self
                .diag(DiagId::sema_inconsistent_deduction, self.ast.decls.get(u).loc())
                .arg(self.ty_arg(deduced))
                .arg(self.ty_arg(tx));
            self.emit(d);
            return false;
        }
        true
    }

    /// [temp.arg.template]/3 (P2113): a constrained `P` must be at least
    /// as constrained as `A`; an unconstrained one ignores `A`'s
    /// constraints.
    fn ttp22_constraints(
        &mut self,
        p: DeclId,
        a: DeclId,
        pl: TemplateParamListId,
        al: TemplateParamListId,
        at: Loc,
    ) -> bool {
        let mut pc = Vec::new();
        let l = *self.ast.templates.param_list(pl);
        self.head_constraints(l.params, l.requires, &mut pc);
        if pc.is_empty() {
            return true;
        }
        let ac = if self.kind_of(a) == DeclKind::TemplateTemplateParm {
            let mut ac = Vec::new();
            let l = *self.ast.templates.param_list(al);
            self.head_constraints(l.params, l.requires, &mut ac);
            ac
        } else {
            self.associated_constraints(a)
        };
        if self.at_least_as_constrained_positional(p, &pc, a, &ac) {
            return true;
        }
        let (pn, an) = (format!("'{}'", self.decl_name(p)), format!("'{}'", self.decl_name(a)));
        let d = self
            .diag(DiagId::sema_template_template_param_not_at_least_as_constrained, at)
            .arg(an.clone())
            .arg(pn.clone())
            .with_note(
                self.diag(DiagId::note_entity_declared_at, self.ast.decls.get(p).loc()).arg(pn),
            )
            .with_note(
                self.diag(DiagId::note_entity_declared_at, self.ast.decls.get(a).loc()).arg(an),
            );
        self.emit(d);
        false
    }
}
