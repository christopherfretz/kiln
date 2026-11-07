//! Matching a template template argument against its template template
//! parameter ([temp.arg.template]/3-4; spec/sema-templates.md §3.2,
//! WP-195).
//!
//! clang's `CheckTemplateTemplateArgument` with relaxed matching (P0522,
//! on by default in clang 19): the argument `A` matches parameter `P`
//! when `P` is at least as specialized as `A`
//! ([`Sema::ttp_at_least_as_specialized`], a structural approximation of
//! the function-template rewrite of [temp.arg.template]/4). Otherwise
//! the parameter lists are compared exactly
//! (`TemplateParameterListsAreEqual` in `TPL_TemplateTemplateArgumentMatch`
//! mode, [`Sema::ttp_lists_equal`]), whose first difference is the
//! diagnostic: "template template argument has different template
//! parameters than its corresponding template template parameter" at
//! the argument, with a note at the differing parameters.
//!
//! A non-type parameter of `P` whose type names an earlier parameter of
//! the enclosing template (`template<class T, template<T> class P>`) is
//! compared with the prior arguments substituted, as clang substitutes
//! them into `P` before matching (`SubstDecl` of the template template
//! parameter); `A`'s parameters are never substituted.

use kiln_ast::decl::DeclKind;
use kiln_ast::types::{ArgView, TemplateArg};
use kiln_ast::{DeclId, QualType, TemplateParamListId};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;
use crate::templ::args::MultiLevelArgs;
use crate::templ::subst::Substituter;

/// The substitutions for the two parameter lists being matched: `P`'s
/// (the enclosing template's prior arguments) and `A`'s (none). Swapped
/// when a nested template template parameter of `P` becomes the
/// argument.
#[derive(Clone, Copy)]
struct Sides<'m> {
    p: Option<&'m MultiLevelArgs>,
    a: Option<&'m MultiLevelArgs>,
}

impl Sides<'_> {
    fn swap(self) -> Self {
        Sides { p: self.a, a: self.p }
    }
}

impl Sema<'_, '_> {
    /// [temp.arg.template]/3: whether template `a` (a class or alias
    /// template, or a template template parameter) matches template
    /// template parameter `p`; diagnoses a mismatch at `at`. `prior`
    /// substitutes the enclosing template's earlier arguments into `p`'s
    /// dependent non-type parameter types.
    pub(crate) fn check_template_template_arg(
        &mut self,
        p: DeclId,
        a: DeclId,
        at: Loc,
        prior: &MultiLevelArgs,
    ) -> bool {
        if self.templ.keyless_ttps.contains(&p) {
            self.reject_keyless_ttp_arg(a, at);
            return false;
        }
        let pl = self.ast.template_parm(p).and_then(|x| x.params);
        let al = match self.kind_of(a) {
            DeclKind::TemplateTemplateParm => self.ast.template_parm(a).and_then(|x| x.params),
            _ => self.ast.template_info(a).map(|x| x.params),
        };
        let (Some(pl), Some(al)) = (pl, al) else { return true };
        if self.clang_compat >= 22 {
            let none = MultiLevelArgs::new();
            let pr = super::ttarg22::Priors { p: prior, a: &none };
            return self.check_template_template_arg_22(p, a, (pl, al), at, pr);
        }
        let sides = Sides { p: Some(prior), a: None };
        self.ttp_at_least_as_specialized(al, pl, sides) || self.ttp_lists_equal(al, pl, at, sides)
    }

    /// [temp.arg.template]/3 for deduced arguments: whether each
    /// deduced template template argument in `args` (`tmpl`'s parameters
    /// `params`, at `depth`) matches its parameter, as clang's
    /// `ConvertDeducedTemplateArgument` checks it before substitution. A
    /// mismatch is diagnosed at `at`, a substitution failure in the
    /// deduction's SFINAE frame (`T<Ts...>` deduced from
    /// `integer_sequence<int>` for `template<class...> class T`).
    pub(crate) fn check_deduced_template_template_args(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        params: &[DeclId],
        args: &[TemplateArg],
        at: Loc,
    ) -> bool {
        (0..params.len().min(args.len()))
            .all(|j| self.check_deduced_template_template_arg(tmpl, depth, params[j], j, args, at))
    }

    /// [`Self::check_deduced_template_template_args`] for parameter `p`
    /// (index `j` of `args`) alone.
    pub(crate) fn check_deduced_template_template_arg(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        j: usize,
        args: &[TemplateArg],
        at: Loc,
    ) -> bool {
        if self.kind_of(p) != DeclKind::TemplateTemplateParm {
            return true;
        }
        let Some(&a) = args.get(j) else { return true };
        let elems: Vec<TemplateArg> = match a.view() {
            ArgView::Pack(l) => self.ast.types.args(l).to_vec(),
            _ => vec![a],
        };
        for x in elems {
            let ArgView::Template(n) = x.view() else { continue };
            let Some(d) = self.arg_template_decl(n) else { continue };
            if !matches!(
                self.kind_of(d),
                DeclKind::ClassTemplate
                    | DeclKind::TypeAliasTemplate
                    | DeclKind::TemplateTemplateParm
            ) {
                continue;
            }
            let prior = self.prior_levels(tmpl, depth, &args[..j]);
            if !self.check_template_template_arg(p, d, at, &prior) {
                return false;
            }
        }
        true
    }

    /// The type of non-type parameter `d`, with `subst` applied when it
    /// is dependent (unchanged if substitution fails).
    fn ttp_nttp_type(&mut self, d: DeclId, subst: Option<&MultiLevelArgs>) -> Option<QualType> {
        let ty = self.ast.decls.get(d).ty()?;
        let Some(m) = subst.filter(|m| m.substituted() > 0) else { return Some(ty) };
        if !self.is_instantiation_dependent_type(ty) {
            return Some(ty);
        }
        let at = self.ast.decls.get(d).loc();
        Some(Substituter::new(self, m).ty(ty, at).unwrap_or(ty))
    }

    fn list_params(&self, l: TemplateParamListId) -> Vec<DeclId> {
        let l = *self.ast.templates.param_list(l);
        self.ast.lists.iter::<DeclId>(l.params).collect()
    }

    fn is_pack_parm(&self, d: DeclId) -> bool {
        self.ast.template_parm(d).is_some_and(|x| x.pack)
    }

    /// [temp.arg.template]/4, structurally: `P`'s parameters, as
    /// arguments, fit `A`'s list (a pack on either side takes the rest
    /// of the other's of the same kind; `A`'s parameters left over have
    /// defaults or are packs), each parameter matching in kind, a
    /// non-type parameter's type the same or both integral (or either
    /// dependent), a template template parameter's list matched in the
    /// reverse direction (`P`'s parameter is then the argument).
    fn ttp_at_least_as_specialized(
        &mut self,
        a: TemplateParamListId,
        p: TemplateParamListId,
        sides: Sides<'_>,
    ) -> bool {
        let (pa, pp) = (self.list_params(a), self.list_params(p));
        let mut j = 0;
        for &x in &pp {
            if self.is_pack_parm(x) {
                // An expansion `X...` fills every remaining parameter.
                return pa[j..].iter().all(|&y| self.ttp_param_fits(x, y, sides));
            }
            let Some(&y) = pa.get(j) else { return false };
            if !self.ttp_param_fits(x, y, sides) {
                return false;
            }
            if !self.is_pack_parm(y) {
                j += 1;
            }
        }
        pa[j..].iter().all(|&y| self.is_pack_parm(y) || self.template_default(y).is_some())
    }

    /// Whether `P`'s parameter `x`, as an argument, fits `A`'s `y`.
    fn ttp_param_fits(&mut self, x: DeclId, y: DeclId, sides: Sides<'_>) -> bool {
        let (kx, ky) = (self.kind_of(x), self.kind_of(y));
        if kx != ky {
            return false;
        }
        match kx {
            DeclKind::NonTypeTemplateParm => {
                let (Some(tx), Some(ty)) =
                    (self.ttp_nttp_type(x, sides.p), self.ttp_nttp_type(y, sides.a))
                else {
                    return true;
                };
                // A placeholder type takes any argument's type.
                self.is_dependent_type(tx)
                    || self.is_dependent_type(ty)
                    || self.contains_undeduced_auto(tx)
                    || self.contains_undeduced_auto(ty)
                    || self.ast.types.same_type(tx, ty)
                    || self.ast.types.is_integral(tx) && self.ast.types.is_integral(ty)
            }
            DeclKind::TemplateTemplateParm => {
                let lx = self.ast.template_parm(x).and_then(|i| i.params);
                let ly = self.ast.template_parm(y).and_then(|i| i.params);
                match (lx, ly) {
                    (Some(lx), Some(ly)) => self.ttp_at_least_as_specialized(lx, ly, sides.swap()),
                    _ => true,
                }
            }
            _ => true,
        }
    }

    /// clang's `TemplateParameterListsAreEqual(A, P, Complain,
    /// TPL_TemplateTemplateArgumentMatch)`: `P`'s pack matches any number
    /// of `A`'s parameters of its kind; diagnoses the first difference.
    fn ttp_lists_equal(
        &mut self,
        a: TemplateParamListId,
        p: TemplateParamListId,
        at: Loc,
        sides: Sides<'_>,
    ) -> bool {
        let (pa, pp) = (self.list_params(a), self.list_params(p));
        let mut j = 0;
        for &x in &pp {
            if !self.is_pack_parm(x) {
                let Some(&y) = pa.get(j) else {
                    self.ttp_arity_mismatch(a, p, pa.len() > pp.len(), at);
                    return false;
                };
                if !self.ttp_kinds_match(y, x, at, sides) {
                    return false;
                }
                j += 1;
                continue;
            }
            while let Some(&y) = pa.get(j) {
                if !self.ttp_kinds_match(y, x, at, sides) {
                    return false;
                }
                j += 1;
            }
        }
        if j != pa.len() {
            self.ttp_arity_mismatch(a, p, pa.len() > pp.len(), at);
            return false;
        }
        true
    }

    fn ttp_mismatch(&mut self, at: Loc, note: Diag, prev: Diag) {
        let d = self.diag(DiagId::sema_template_arg_template_params_mismatch, at);
        self.emit(d.with_note(note).with_note(prev));
    }

    fn ttp_arity_mismatch(
        &mut self,
        a: TemplateParamListId,
        p: TemplateParamListId,
        many: bool,
        at: Loc,
    ) {
        let (la, lp) = (
            self.ast.templates.param_list(a).template_loc,
            self.ast.templates.param_list(p).template_loc,
        );
        let note =
            self.diag(DiagId::note_template_param_list_different_arity, la).arg(u8::from(many));
        let prev = self.diag(DiagId::note_template_prev_declaration, lp).arg(1u8);
        self.ttp_mismatch(at, note, prev);
    }

    /// clang's `MatchTemplateParameterKind` (`TPL_TemplateTemplateArgumentMatch`)
    /// for `A`'s parameter `y` against `P`'s `x`.
    fn ttp_kinds_match(&mut self, y: DeclId, x: DeclId, at: Loc, sides: Sides<'_>) -> bool {
        let (ky, kx) = (self.kind_of(y), self.kind_of(x));
        let (ly, lx) = (self.ast.decls.get(y).loc(), self.ast.decls.get(x).loc());
        if ky != kx {
            let note = self.diag(DiagId::note_template_param_different_kind, ly);
            let prev = self.diag(DiagId::note_template_prev_declaration, lx).arg(1u8);
            self.ttp_mismatch(at, note, prev);
            return false;
        }
        let (py, px) = (self.is_pack_parm(y), self.is_pack_parm(x));
        if py != px && !px {
            let k = match ky {
                DeclKind::TemplateTypeParm => 0u8,
                DeclKind::NonTypeTemplateParm => 1,
                _ => 2,
            };
            let note = self
                .diag(DiagId::note_template_parameter_pack_non_pack, ly)
                .arg(k)
                .arg(u8::from(py));
            let prev =
                self.diag(DiagId::note_template_parameter_pack_here, lx).arg(k).arg(u8::from(px));
            self.ttp_mismatch(at, note, prev);
            return false;
        }
        match ky {
            DeclKind::NonTypeTemplateParm => {
                if let (Some(ty), Some(tx)) =
                    (self.ttp_nttp_type(y, sides.a), self.ttp_nttp_type(x, sides.p))
                    && !self.is_dependent_type(ty)
                    && !self.is_dependent_type(tx)
                    && !self.ast.types.same_type(ty, tx)
                {
                    let note = self
                        .diag(DiagId::note_template_nontype_parm_different_type, ly)
                        .arg(self.ty_arg(ty));
                    let prev = self
                        .diag(DiagId::note_template_nontype_parm_prev_declaration, lx)
                        .arg(self.ty_arg(tx));
                    self.ttp_mismatch(at, note, prev);
                    return false;
                }
                true
            }
            DeclKind::TemplateTemplateParm => {
                let ly = self.ast.template_parm(y).and_then(|i| i.params);
                let lx = self.ast.template_parm(x).and_then(|i| i.params);
                match (ly, lx) {
                    (Some(ly), Some(lx)) => self.ttp_lists_equal(ly, lx, at, sides),
                    _ => true,
                }
            }
            _ => true,
        }
    }
}
