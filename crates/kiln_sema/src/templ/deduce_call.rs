//! Template argument deduction from a function call
//! ([temp.deduct.call]; spec/sema-templates.md §6.1): explicit
//! arguments first, then each `(P, A)` pair with the call adjustments,
//! then defaults, then the substitution of the deduced arguments into
//! the signature in a SFINAE context. A failure is a
//! [`DeductionFailure`] the overload candidate keeps for its note
//! (§10.4).

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::ids::ArgListId;
use kiln_ast::types::{ArgView, TemplateArg, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;
use crate::overload::{ArgInfo, CandidateKind, IcsOptions, OverloadCandidateSet, Rewrite};
use crate::templ::args::ConvertedArgs;
use crate::templ::deduce::{DeducedArgs, TdfFlags, Tdk};
use crate::templ::stack::{FrameKind, SfinaeState};
use crate::templ::subst::Substituter;

/// A failed deduction (clang's `DeductionFailureInfo`).
#[derive(Clone, Debug)]
pub struct DeductionFailure {
    /// Why.
    pub tdk: Tdk,
    /// The arguments deduced so far and the failure detail.
    pub deduced: DeducedArgs,
    /// `SubstitutionFailure`: the arguments substituted.
    pub args: Option<ArgListId>,
    /// `SubstitutionFailure`: the first error captured.
    pub diag: Option<Diag>,
    /// `ConstraintsNotSatisfied`: why (§8.7).
    pub sat: Option<super::concepts::SatId>,
}

/// What a deduction without call arguments matches the template
/// against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DeduceTarget {
    /// Nothing: every argument explicit or defaulted.
    None,
    /// The function type of the address target ([temp.deduct.funcaddr]).
    FuncAddr(QualType),
    /// The type a conversion function template converts to
    /// ([temp.deduct.conv]).
    Conversion(QualType),
}

impl DeductionFailure {
    fn new(tdk: Tdk, deduced: DeducedArgs) -> Box<DeductionFailure> {
        Box::new(DeductionFailure { tdk, deduced, args: None, diag: None, sat: None })
    }
}

impl Sema<'_, '_> {
    /// The explicitly given pack (index, length) that the trailing
    /// function parameter pack with pattern `pat` expands
    /// ([temp.arg.explicit]/9): only the
    /// first template parameter pack takes explicit arguments (it takes
    /// the rest of them), so none when the pattern (`U`, `U&&`, `U*`)
    /// expands another pack (`f<int>(tys<int>, 'a')` for
    /// `template<class... T, class... U> f(tys<T...>, U...)`). A pattern
    /// of another shape is taken to expand the first pack.
    fn trailing_pack_skip(
        &self,
        pat: QualType,
        params: &[DeclId],
        d: &DeducedArgs,
    ) -> Option<(u16, usize)> {
        let types = &self.ast.types;
        let mut q = types.canonical(pat);
        while let Some(p) = types.pointee(q) {
            q = types.canonical(p);
        }
        let first = params.iter().position(|&p| self.ast.template_parm(p).is_some_and(|x| x.pack));
        if let TypeView::CanonTemplateTypeParm { depth, index, .. } = types.view(q.ty())
            && depth == u32::from(d.depth)
            && first.is_some_and(|f| f != index as usize)
        {
            return None;
        }
        self.explicit_pack(params, d)
    }

    /// The function parameter packs whose pattern also expands the
    /// explicitly specified pack (the last explicit argument, when a
    /// pack: [temp.arg.explicit]/9) against the other packs of the
    /// pattern: one shorter than the explicit pack can never match
    /// (clang reports it substituting the explicit arguments, with the
    /// pack partially substituted). The failure, if any.
    fn check_partial_pack_lengths(
        &mut self,
        ft: DeclId,
        d: &DeducedArgs,
        levels: &crate::templ::args::MultiLevelArgs,
        tys: &[QualType],
        shown: ArgListId,
        loc: Loc,
    ) -> Option<Box<DeductionFailure>> {
        let n = self.ast.types.args(shown).len();
        let last = n.checked_sub(1)?;
        if !matches!(d.args.get(last).map(TemplateArg::view), Some(ArgView::Pack(_))) {
            return None;
        }
        let partial = (d.depth, u16::try_from(last).ok()?);
        for &t in tys {
            let Some(pat) = crate::templ::pack::type_pattern(self, t) else { continue };
            let r = self.in_sfinae(FrameKind::ExplicitArgSubstitution, ft, Some(shown), loc, |s| {
                let mut sub = Substituter::new(s, levels);
                sub.partial_pack = Some(partial);
                let _ = sub.expand_pattern_at(None, |sub| sub.ty(pat, loc));
            });
            if let Some(((), Err(Some(diag)))) = r
                && diag.id() == Some(DiagId::sema_pack_length_mismatch_partial)
            {
                let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d.clone());
                f.args = Some(shown);
                f.diag = Some(diag);
                return Some(f);
            }
        }
        None
    }

    /// Runs `f` in a SFINAE frame of `kind` for template `ft` showing
    /// `args`: its result, and the first error captured (`Some` when
    /// substitution failed). `None` when the instantiation depth limit
    /// stops it.
    pub(crate) fn in_sfinae<R>(
        &mut self,
        kind: FrameKind,
        ft: DeclId,
        args: Option<ArgListId>,
        loc: Loc,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<(R, Result<(), Option<Diag>>)> {
        if !self.push_inst_frame(kind, ft, ft, loc) {
            return None;
        }
        if let Some(fr) = self.inst.stack.frames.last_mut() {
            fr.sfinae = Some(true);
        }
        if let Some(a) = args {
            self.set_frame_args(a);
        }
        self.inst.sfinae.borrow_mut().push(SfinaeState::default());
        let r = f(self);
        let st = self.inst.sfinae.borrow_mut().pop().unwrap_or_default();
        self.pop_inst_frame();
        let failed = if st.errors > 0 { Err(st.first) } else { Ok(()) };
        Some((r, failed))
    }

    /// The function declaration a function template declares (its
    /// pattern, instantiated for a member template of a class template
    /// specialization).
    pub(crate) fn templated_function(&self, ft: DeclId) -> Option<DeclId> {
        self.pattern_of(ft).filter(|&f| self.ast.decls.get(f).kind().is_function())
    }

    /// Deduces the template arguments of function template `ft` for a
    /// call with arguments `args` (`explicit` the explicitly-specified
    /// template arguments, as written) and forms the specialization
    /// (§6.1). `object`: `args` has no slot for an object argument the
    /// template's implicit object parameter takes.
    pub(crate) fn deduce_call(
        &mut self,
        ft: DeclId,
        explicit: Option<ArgListId>,
        args: &[ArgInfo],
        loc: Loc,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        self.deduce_spec(ft, explicit, Some(args), DeduceTarget::None, loc)
    }

    /// The specialization of `ft` whose type matches the function type
    /// `target` (the address of an overload set, [temp.deduct.funcaddr]):
    /// P is the template's function type, A `target`.
    pub(crate) fn deduce_funcaddr(
        &mut self,
        ft: DeclId,
        explicit: Option<ArgListId>,
        target: QualType,
        loc: Loc,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        self.deduce_spec(ft, explicit, None, DeduceTarget::FuncAddr(target), loc)
    }

    /// The specialization of conversion function template `ft` that
    /// converts to `to` ([temp.deduct.conv]): P is the template's
    /// conversion type, A `to` (a reference type for a reference
    /// binding).
    pub(crate) fn deduce_conversion(
        &mut self,
        ft: DeclId,
        to: QualType,
        loc: Loc,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        let spec = self.deduce_spec(ft, None, None, DeduceTarget::Conversion(to), loc)?;
        if !self.specialize_lambda_conversion(ft, spec, loc) {
            let n = self.template_params(ft).len();
            let d = DeducedArgs::new(self.template_param_depth(ft), n);
            return Err(DeductionFailure::new(Tdk::NonDeducedMismatch, d));
        }
        Ok(spec)
    }

    /// The specialization of `ft` a template-id with the explicit
    /// arguments `explicit` names without a call or a target type
    /// ([over.over]/3, [temp.arg.explicit]/4): every argument explicit
    /// or defaulted.
    pub(crate) fn deduce_explicit_only(
        &mut self,
        ft: DeclId,
        explicit: Option<ArgListId>,
        loc: Loc,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        self.deduce_spec(ft, explicit, None, DeduceTarget::None, loc)
    }

    /// [`Self::deduce_call`]; `call` `None` for no call (no arity
    /// check, nothing deduced from arguments).
    #[allow(clippy::too_many_lines, reason = "the steps of §6.1 in order")]
    fn deduce_spec(
        &mut self,
        ft: DeclId,
        explicit: Option<ArgListId>,
        call: Option<&[ArgInfo]>,
        target: DeduceTarget,
        loc: Loc,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        let args = call.unwrap_or(&[]);
        let ft = self.ast.first(ft);
        let params = self.template_params(ft);
        let depth = self.template_param_depth(ft);
        let n = params.len();
        let mut d = DeducedArgs::new(depth, n);
        let Some(templated) = self.templated_function(ft) else {
            return Err(DeductionFailure::new(Tdk::MiscellaneousDeductionFailure, d));
        };
        let is_pack = |s: &Self, p: DeclId| s.ast.template_parm(p).is_some_and(|x| x.pack);
        // [temp.arg.explicit]/9: a template parameter pack anywhere in the
        // list (`template <class... A, class T>`) takes every remaining
        // explicit argument; only a list without one has a maximum arity.
        let has_pack = params.iter().any(|&p| is_pack(self, p));
        // §6.1 step 1: the explicitly-specified arguments.
        let (mut ptys, variadic) = self.proto_param_types(templated);
        // CWG1391: the parameters the explicit arguments make
        // non-dependent (published on success; nested deductions in
        // between overwrite the field).
        let mut nondep = 0u64;
        // [temp.deduct.funcaddr]: P is the function type, with the
        // explicit arguments substituted below.
        let mut pfn = self.ast.decls.get(templated).ty().unwrap_or(QualType::ERROR);
        // Expanded non-trailing function parameter packs.
        let mut retained = 0usize;
        let written: Vec<TemplateArg> =
            explicit.map(|l| self.ast.types.args(l).to_vec()).unwrap_or_default();
        if !written.is_empty() {
            if written.len() > n && !has_pack {
                // clang: a substitution failure with the arity message.
                let msg = self
                    .diag(DiagId::sema_template_arg_list_different_arity, loc)
                    .arg(1u8)
                    .arg(1u8)
                    .arg(self.decl_name(ft));
                let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                f.diag = Some(msg);
                return Err(f);
            }
            let mut bad = None;
            let r = self.in_sfinae(FrameKind::ExplicitArgSubstitution, ft, None, loc, |s| {
                for (i, &w) in written.iter().enumerate() {
                    let Some(&parm) = params.get(i) else { break };
                    let prior: Vec<TemplateArg> = d.args[..i].to_vec();
                    if is_pack(s, parm) {
                        // [temp.arg.explicit]/9: the rest form a pack,
                        // which deduction may extend.
                        let mut v = Vec::new();
                        for &w in &written[i..] {
                            let Ok(a) = s.check_template_arg(ft, depth, parm, w, loc, &prior)
                            else {
                                bad = Some(i);
                                return;
                            };
                            v.push(a);
                        }
                        let l = s.ast.types.arg_list(&v);
                        d.args[i] = TemplateArg::pack(l);
                        break;
                    }
                    let Ok(a) = s.check_template_arg(ft, depth, parm, w, loc, &prior) else {
                        bad = Some(i);
                        return;
                    };
                    d.args[i] = a;
                }
            });
            let Some(((), failed)) = r else {
                return Err(DeductionFailure::new(Tdk::InstantiationDepth, d));
            };
            if let Some(i) = bad.or_else(|| failed.is_err().then_some(written.len() - 1)) {
                d.info.param = u16::try_from(i).ok();
                return Err(DeductionFailure::new(Tdk::InvalidExplicitArguments, d));
            }
            // P with the explicit arguments substituted ([temp.deduct]/2).
            let levels = self.prior_levels(ft, depth, &d.args);
            let shown = self.ast.types.arg_list(&d.args[..written.len().min(n)]);
            let tys = ptys.clone();
            let fn_has_pack =
                tys.iter().any(|&t| crate::templ::pack::type_pattern(self, t).is_some());
            let whole = matches!(target, DeduceTarget::FuncAddr(_)) && !fn_has_pack;
            let r = self.in_sfinae(FrameKind::ExplicitArgSubstitution, ft, Some(shown), loc, |s| {
                let mut sub = Substituter::new(s, &levels);
                // A function parameter pack keeps its pattern: the
                // explicitly given prefix may be extended by deduction.
                // A non-trailing one is a non-deduced context
                // ([temp.deduct.type]/5.7): its explicit elements are
                // all of it (clang keeps the expansion, unexpanded, in
                // the parameter count).
                let mut v = Vec::with_capacity(tys.len());
                let mut kept = 0usize;
                for (j, &t) in tys.iter().enumerate() {
                    let Some(pat) = crate::templ::pack::type_pattern(sub.sema, t) else {
                        v.push(sub.ty(t, loc)?);
                        continue;
                    };
                    if j + 1 == tys.len() {
                        v.push(t);
                        continue;
                    }
                    match sub.expand_pattern_at(None, |sub| sub.ty(pat, loc))? {
                        crate::templ::pack::Expansion::Elements(e) => {
                            v.extend(e);
                            kept += 1;
                        }
                        crate::templ::pack::Expansion::Unexpanded(_) => v.push(t),
                    }
                }
                let f = if whole { Some(sub.ty(pfn, loc)?) } else { None };
                Ok::<_, crate::templ::subst::SubstFail>((v, f, kept))
            });
            match r {
                None => return Err(DeductionFailure::new(Tdk::InstantiationDepth, d)),
                Some((Ok((t, f, kept)), Ok(()))) => {
                    retained = kept;
                    if let Some(f) =
                        self.check_partial_pack_lengths(ft, &d, &levels, &tys, shown, loc)
                    {
                        return Err(f);
                    }
                    if t.len() == tys.len() {
                        nondep = t
                            .iter()
                            .take(64)
                            .enumerate()
                            .filter(|&(_, &x)| !self.is_dependent_type(x))
                            .fold(0, |m, (j, _)| m | 1 << j);
                    }
                    ptys = t;
                    if let Some(f) = f {
                        pfn = f;
                    }
                }
                Some((_, failed)) => {
                    let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                    f.args = Some(shown);
                    f.diag = failed.err().flatten();
                    return Err(f);
                }
            }
        }
        // Arity ([temp.deduct.call]/1: a parameter without an argument
        // is not deduced from).
        let nargs = args.len();
        let fn_pack = ptys.last().and_then(|&t| crate::templ::pack::type_pattern(self, t));
        if call.is_some() && nargs > ptys.len() + retained && !variadic && fn_pack.is_none() {
            return Err(DeductionFailure::new(Tdk::TooManyArguments, d));
        }
        if call.is_some() && nargs < self.fn_min_args(templated) {
            return Err(DeductionFailure::new(Tdk::TooFewArguments, d));
        }
        // [temp.deduct.funcaddr]: the function type against the target.
        if let DeduceTarget::FuncAddr(a) = target {
            let r = self.deduce(pfn, a, TdfFlags::NONE, &mut d);
            if r != Tdk::Success {
                self.resugar_fn_mismatch(pfn, &mut d);
                return Err(DeductionFailure::new(r, d));
            }
        }
        if let DeduceTarget::Conversion(to) = target {
            let ret =
                self.ast.types.proto(pfn).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
            let (p, a, tdf) = self.conversion_deduction_pair(ret, to);
            let p = self.lambda_conversion_p(ft, p, a);
            let r = self.deduce(p, a, tdf, &mut d);
            if r != Tdk::Success {
                // clang shows the written `cv T` of an underqualified
                // result and the canonical type of a mismatch.
                if r == Tdk::Underqualified
                    && let TypeView::Function(f) = self.ast.types.view(pfn.ty())
                {
                    let written = self.non_reference(f.ret());
                    self.resugar_mismatch(written, &mut d);
                }
                return Err(DeductionFailure::new(r, d));
            }
        }
        // §6.1 step 2: each (P, A). The explicitly given prefix of a
        // pack is fixed before any deduction: a pack deduced from an
        // earlier parameter (`tys<Tf...>`) is not a prefix of the
        // trailing function parameter pack.
        let skip = fn_pack.and_then(|pat| self.trailing_pack_skip(pat, &params, &d));
        // [temp.deduct.call]/4: the (argument index, P, adjusted A) of
        // each deduced-from argument, checked against the deduced A.
        let mut originals: Vec<(usize, QualType, QualType)> = Vec::new();
        let mut pi = 0;
        for (i, a) in args.iter().enumerate() {
            // [temp.deduct.type]/5.7: a non-trailing function parameter
            // pack is a non-deduced context and takes no argument.
            while pi + 1 < ptys.len() && crate::templ::pack::type_pattern(self, ptys[pi]).is_some()
            {
                pi += 1;
            }
            let Some(&p) = ptys.get(pi) else { break };
            pi += 1;
            if pi == ptys.len()
                && let Some(pat) = fn_pack
            {
                // [temp.deduct.call]/1: the trailing function parameter
                // pack's pattern against each remaining argument; the
                // elements given explicitly are not deduced.
                let rest = &args[i..];
                let r = self.deduce_pack_elements_from(skip, rest.len(), &mut d, |s, k, d| {
                    s.deduce_call_arg(pat, &rest[k], d)
                });
                if r != Tdk::Success {
                    self.resugar_mismatch(pat, &mut d);
                    return Err(DeductionFailure::new(r, d));
                }
                if self.has_deducible_params(pat, &d, true) {
                    // The explicit elements of a pack the pattern alone
                    // names are not deduced, so not checked.
                    let fixed = match skip {
                        Some((ep, m)) if self.pattern_packs(pat, &d).iter().all(|&x| x == ep) => m,
                        _ => 0,
                    };
                    for (k, a) in rest.iter().enumerate().skip(fixed) {
                        if self.is_original_arg(a) {
                            originals.push((i + k, pat, self.adjusted_call_arg(pat, a, d.depth)));
                        }
                    }
                }
                break;
            }
            if !self.is_dependent_type(p) {
                continue;
            }
            let r = self.deduce_call_arg(p, a, &mut d);
            // [temp.deduct.call]/1 (CWG1391): a P with no template
            // parameter in a deduced context takes no part in deduction
            // (`basic_format_string<char, type_identity_t<Args>...>`
            // against `const char*`); the argument is checked by the
            // conversion to the substituted P. Asked only on failure:
            // otherwise skipping and deducing agree.
            if r != Tdk::Success
                && (self.has_deducible_params(p, &d, false)
                    || self.has_deducible_pack_expansion(p, &d))
            {
                self.resugar_mismatch(p, &mut d);
                self.resugar_arg_mismatch(a.ty, &mut d);
                return Err(DeductionFailure::new(r, d));
            }
            if r == Tdk::Success
                && self.has_deducible_params(p, &d, false)
                && self.is_original_arg(a)
            {
                originals.push((i, p, self.adjusted_call_arg(p, a, d.depth)));
            }
        }
        // §6.1 step 3: defaults, then completeness.
        for (j, &p) in params.iter().enumerate() {
            if !matches!(d.args[j].view(), ArgView::Null) {
                continue;
            }
            if is_pack(self, p) {
                // [temp.arg.explicit]/4: a trailing pack not otherwise
                // deduced is deduced as empty.
                let l = self.ast.types.arg_list(&[]);
                d.args[j] = TemplateArg::pack(l);
                continue;
            }
            let Some(def) = self.template_default(p) else {
                d.info.param = u16::try_from(j).ok();
                return Err(DeductionFailure::new(Tdk::Incomplete, d));
            };
            let prior: Vec<TemplateArg> = d.args[..j].to_vec();
            let shown = self.ast.types.arg_list(&prior);
            let r = self.in_sfinae(FrameKind::DeducedArgSubstitution, ft, Some(shown), loc, |s| {
                s.default_template_arg(ft, depth, p, def, loc, &prior)
            });
            match r {
                None => return Err(DeductionFailure::new(Tdk::InstantiationDepth, d)),
                Some((Ok(a), Ok(()))) => d.args[j] = a,
                Some((_, failed)) => {
                    let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                    f.args = Some(shown);
                    f.diag = failed.err().flatten();
                    return Err(f);
                }
            }
        }
        let sugared = self.ast.types.arg_list(&d.args);
        // [temp.deduct.general]/5: the associated constraints
        // ([temp.constr.decl]/3) are checked before the function type is
        // substituted (§8.3).
        let constraints = self.associated_constraints(ft);
        if !constraints.is_empty() {
            let levels = self.prior_levels(ft, depth, &d.args);
            let sat = self.check_constraints(ft, &constraints, &levels, sugared, loc);
            if !self.sat_ok(sat) {
                let mut f = DeductionFailure::new(Tdk::ConstraintsNotSatisfied, d);
                f.args = Some(sugared);
                f.sat = Some(sat);
                return Err(f);
            }
        }
        // §6.1 step 4: the specialization, its signature substituted.
        let canonical = self.ast.types.canonical_args(sugared);
        let conv = ConvertedArgs { sugared, canonical };
        let r = self.in_sfinae(FrameKind::DeducedArgSubstitution, ft, Some(sugared), loc, |s| {
            s.get_function_spec(ft, &conv, loc)
        });
        match r {
            None => Err(DeductionFailure::new(Tdk::InstantiationDepth, d)),
            Some((Ok(spec), Ok(()))) => {
                if let Some((i, deduced_a, a)) = self.original_arg_mismatch(spec, &originals) {
                    d.info.param = u16::try_from(i).ok();
                    d.info.first = Some(TemplateArg::ty(deduced_a));
                    d.info.second = Some(TemplateArg::ty(a));
                    let mut f = DeductionFailure::new(Tdk::DeducedMismatch, d);
                    f.args = Some(sugared);
                    return Err(f);
                }
                // [over.match.class.deduct]/3: a guide formed for an alias
                // template is constrained on the alias template's
                // arguments being deducible from its return type.
                if let Some(&(alias, rhs)) = self.templ.ctad_alias.get(&ft) {
                    let ty = self.ast.decls.get(spec).ty().unwrap_or(QualType::ERROR);
                    let ret = self
                        .ast
                        .types
                        .proto(ty)
                        .map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
                    if !self.alias_deducible(alias, rhs, ret) {
                        let at = self.ast.decls.get(ft).loc();
                        let n = self.decl_name_str(self.ast.decls.get(alias).name());
                        let why = self
                            .diag(DiagId::note_alias_not_deducible, at)
                            .arg(n)
                            .arg(self.ty_arg(ret));
                        let mut f = DeductionFailure::new(Tdk::ConstraintsNotSatisfied, d);
                        f.args = (!self.ast.types.args(sugared).is_empty()).then_some(sugared);
                        f.diag = Some(why);
                        return Err(f);
                    }
                }
                self.templ.explicit_nondep = nondep;
                Ok(spec)
            }
            Some((_, failed)) => {
                let mut diag = failed.err().flatten();
                if let Some(spec) = self.ast.templates.find_spec(ft, canonical) {
                    match &diag {
                        Some(x) => {
                            self.templ.fn_spec_failures.entry(spec).or_insert_with(|| x.clone());
                        }
                        None => diag = self.templ.fn_spec_failures.get(&spec).cloned(),
                    }
                }
                let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                f.args = Some(sugared);
                f.diag = diag;
                Err(f)
            }
        }
    }

    /// Whether `p` names a template parameter of `d.depth` in a deduced
    /// context (clang's `hasDeducibleTemplateParameters`): only such a
    /// parameter's argument is an original call argument checked by
    /// [temp.deduct.call]/4. Probed by deducing `p` from itself.
    fn has_deducible_params(&mut self, p: QualType, d: &DeducedArgs, pattern: bool) -> bool {
        let mut probe = DeducedArgs::new(d.depth, d.args.len());
        // The packs a pack expansion's pattern names, element-wise.
        if pattern {
            probe.pack_cur = Some(Vec::new());
        }
        self.deduce(p, p, TdfFlags::NONE, &mut probe);
        probe.args.iter().any(|a| !matches!(a.view(), ArgView::Null))
            || probe.pack_cur.is_some_and(|c| !c.is_empty())
    }

    /// Whether `p` holds a template-argument pack expansion whose
    /// pattern is deducible (`Tup<int, Us...>`): deducing `p` against
    /// itself matches the expansion as a whole, so
    /// [`Self::has_deducible_params`] does not see its packs.
    fn has_deducible_pack_expansion(&mut self, p: QualType, d: &DeducedArgs) -> bool {
        let args = match self.ast.types.canonical_view(p) {
            TypeView::Pointer(i) | TypeView::LValueRef(i) | TypeView::RValueRef(i) => {
                return self.has_deducible_pack_expansion(i, d);
            }
            TypeView::TemplateSpecialization { args, .. } => args,
            _ => return false,
        };
        let list = self.ast.types.args(args).to_vec();
        list.into_iter().any(|a| match self.arg_expansion_pattern(a).map(|x| x.view()) {
            Some(ArgView::Type(q)) => {
                self.has_deducible_params(q, d, true) || self.has_deducible_pack_expansion(q, d)
            }
            Some(_) => true,
            None => matches!(a.view(), ArgView::Type(q) if self.has_deducible_pack_expansion(q, d)),
        })
    }

    /// The template parameter packs pack expansion pattern `p` names.
    fn pattern_packs(&mut self, p: QualType, d: &DeducedArgs) -> Vec<u16> {
        let mut probe = DeducedArgs::new(d.depth, d.args.len());
        probe.pack_cur = Some(Vec::new());
        self.deduce(p, p, TdfFlags::NONE, &mut probe);
        probe.pack_cur.unwrap_or_default().into_iter().map(|x| x.0).collect()
    }

    /// Whether call argument `a` is checked against its deduced `A`
    /// ([temp.deduct.call]/4): not a braced list or an overload set.
    fn is_original_arg(&self, a: &ArgInfo) -> bool {
        a.list.is_none()
            && a.ty != QualType::OVERLOAD
            && a.expr.is_none_or(|e| matches!(self.overload_set_arg(e), OverloadArg::NotASet))
    }

    /// The argument type `A` of `a` adjusted for deduction against
    /// parameter `p` ([temp.deduct.call]/2-3, as written): a forwarding
    /// reference's lvalue is `A&`; a non-reference `P` decays `A` and
    /// drops its top-level cv.
    fn adjusted_call_arg(&mut self, p: QualType, a: &ArgInfo, depth: u16) -> QualType {
        let t = &mut self.ast.types;
        match t.canonical_view(p) {
            TypeView::RValueRef(inner)
                if a.vk == ValueCategory::LValue
                    && inner.quals().is_empty()
                    && matches!(t.canonical_view(inner),
                        TypeView::CanonTemplateTypeParm { depth: x, .. } if x as u16 == depth) =>
            {
                t.lvalue_ref_to(a.ty)
            }
            TypeView::LValueRef(_) | TypeView::RValueRef(_) => a.ty,
            _ => {
                if t.is_array(a.ty) {
                    let e = t.element(a.ty).unwrap_or(QualType::ERROR);
                    t.pointer_to(e)
                } else if t.is_function(a.ty) {
                    t.pointer_to(a.ty)
                } else {
                    t.unqualified(a.ty)
                }
            }
        }
    }

    /// [temp.deduct.call]/4 (clang `CheckOriginalCallArgDeduction`): the
    /// first recorded argument whose deduced `A` (the parameter type of
    /// specialization `spec`) differs from its adjusted `A` other than
    /// by the allowed differences: `(index, deduced A, A)`, the deduced
    /// `A` as clang reports it (without the reference or pointer the
    /// comparison stripped).
    fn original_arg_mismatch(
        &mut self,
        spec: DeclId,
        originals: &[(usize, QualType, QualType)],
    ) -> Option<(usize, QualType, QualType)> {
        if originals.is_empty() {
            return None;
        }
        let (ptys, _) = self.proto_param_types(spec);
        for &(i, orig_p, orig_a) in originals {
            let Some(&deduced) = ptys.get(i) else { continue };
            if self.is_dependent_type(deduced)
                || self.ast.types.canonical(deduced) == QualType::ERROR
            {
                continue;
            }
            if let Some(deduced_a) = self.deduced_a_mismatch(orig_p, deduced, orig_a) {
                return Some((i, deduced_a, orig_a));
            }
        }
        None
    }

    /// The deduced `A` to report when `deduced` (for original `P`
    /// `orig_p`) does not match adjusted `A` `a`; `None` when it does.
    fn deduced_a_mismatch(
        &mut self,
        orig_p: QualType,
        deduced: QualType,
        a: QualType,
    ) -> Option<QualType> {
        let unq = |s: &mut Self, q: QualType| {
            let c = s.ast.types.canonical(q);
            s.ast.types.unqualified(c)
        };
        if unq(self, a) == unq(self, deduced) {
            return None;
        }
        let strip_ref =
            |s: &Self, q: QualType| match s.ast.types.view(s.ast.types.canonical(q).ty()) {
                TypeView::LValueRef(x) | TypeView::RValueRef(x) => s.ast.types.canonical(x),
                _ => s.ast.types.canonical(q),
            };
        let mut deduced_a = match self.ast.types.canonical_view(deduced) {
            TypeView::LValueRef(_) | TypeView::RValueRef(_) => self.non_reference(deduced),
            _ => deduced,
        };
        let mut ca = strip_ref(self, a);
        let mut cd = self.ast.types.canonical(deduced_a);
        let mut op = self.ast.types.canonical(orig_p);
        if let TypeView::LValueRef(x) | TypeView::RValueRef(x) = self.ast.types.canonical_view(op) {
            op = self.ast.types.canonical(x);
            // /4.1: the referred-to deduced A may be more cv-qualified.
            let (aq, dq) = (self.ast.types.effective_quals(ca), self.ast.types.effective_quals(cd));
            if aq != dq {
                if !dq.contains(aq) {
                    return Some(deduced_a);
                }
                let u = self.ast.types.unqualified(ca);
                ca = self.ast.types.qualify(u, dq);
            }
        }
        // /4.2: a qualification or function pointer conversion.
        let types = &self.ast.types;
        let ptr_like = matches!(
            types.canonical_view(ca),
            TypeView::Pointer(_) | TypeView::MemberPointer { .. }
        );
        if ptr_like
            && (crate::conv::is_qualification_convertible(self, ca, cd)
                || crate::conv::is_function_pointer_conversion(self, ca, cd))
        {
            return None;
        }
        // /4.3: a derived class (or pointer to one) of a simple-template-id.
        if let TypeView::Pointer(pp) = self.ast.types.canonical_view(op) {
            op = self.ast.types.canonical(pp);
            if let (TypeView::Pointer(x), TypeView::Pointer(y)) =
                (self.ast.types.canonical_view(ca), self.ast.types.canonical_view(cd))
            {
                ca = self.ast.types.canonical(x);
                cd = self.ast.types.canonical(y);
                deduced_a = self.pointee_sugared(deduced_a).unwrap_or(cd);
            }
        }
        if unq(self, ca) == unq(self, cd) {
            return None;
        }
        let simple_template_id =
            matches!(self.ast.types.canonical_view(op), TypeView::TemplateSpecialization { .. });
        if simple_template_id
            && let (Some(x), Some(y)) = (self.ast.types.as_record(ca), self.ast.types.as_record(cd))
            && crate::conv::is_derived_from(self, x, y)
        {
            return None;
        }
        Some(deduced_a)
    }

    /// [temp.deduct.conv]/2–6: the `(P, A)` pair and flags for a
    /// conversion function template returning `ret` converting to `to`
    /// (clang `DeduceTemplateArguments` for a conversion).
    fn conversion_deduction_pair(
        &mut self,
        ret: QualType,
        to: QualType,
    ) -> (QualType, QualType, TdfFlags) {
        let mut p = self.non_reference(ret);
        let a_ref = match self.ast.types.canonical_view(to) {
            TypeView::LValueRef(x) | TypeView::RValueRef(x) => Some(x),
            _ => None,
        };
        let a = if let Some(x) = a_ref {
            self.ast.types.unqualified(x)
        } else {
            // /3: A not a reference: decay P, else drop its top-level cv.
            let t = &mut self.ast.types;
            if t.is_array(p) {
                let e = t.element(p).unwrap_or(QualType::ERROR);
                p = t.pointer_to(e);
            } else if t.is_function(p) {
                p = t.pointer_to(p);
            } else {
                p = self.ast.types.unqualified(p);
            }
            self.ast.types.unqualified(to)
        };
        let mut tdf = TdfFlags::NONE;
        if a_ref.is_some() {
            tdf = tdf.union(TdfFlags::ARG_WITH_REFERENCE_TYPE);
        }
        let ptr =
            |s: &Self, t: QualType| matches!(s.ast.types.canonical_view(t), TypeView::Pointer(_));
        let mptr = |s: &Self, t: QualType| {
            matches!(s.ast.types.canonical_view(t), TypeView::MemberPointer { .. })
        };
        if (ptr(self, p) && ptr(self, a)) || (mptr(self, p) && mptr(self, a)) {
            tdf = tdf.union(TdfFlags::IGNORE_QUALIFIERS);
        }
        (p, a, tdf)
    }

    /// Deduces from one call argument `a` for parameter type `p`
    /// ([temp.deduct.call]/2–5).
    fn deduce_call_arg(&mut self, p: QualType, a: &ArgInfo, d: &mut DeducedArgs) -> Tdk {
        let t = &self.ast.types;
        let mut p = t.canonical(p);
        let mut flags = TdfFlags::NONE;
        // [temp.deduct.call]/1: a braced-init-list.
        if let Some(li) = a.list {
            let inner = match t.canonical_view(p) {
                TypeView::LValueRef(x) | TypeView::RValueRef(x) => t.canonical(x),
                _ => p,
            };
            let items: Vec<crate::init::InitValue> = self
                .ovl
                .lists
                .get(li as usize)
                .map(|l| l.items.iter().map(|x| x.value.clone()).collect())
                .unwrap_or_default();
            // `P'[N]` with a non-empty list: each element deduces `P'`,
            // and the list's length deduces `N` (WP-134: the aggregate
            // deduction candidate's `T(&&)[N]` parameters).
            let t = &self.ast.types;
            let array = match t.canonical_view(inner) {
                TypeView::ConstantArray(pa) if !items.is_empty() => Some((pa.elem, None)),
                TypeView::DependentSizedArray { elem, size } if !items.is_empty() => {
                    Some((elem, Some(size)))
                }
                _ => None,
            };
            let Some(elem) = array.map(|x| x.0).or_else(|| self.initializer_list_param(inner))
            else {
                // A non-deduced context.
                return Tdk::Success;
            };
            let n = u64::try_from(items.len()).unwrap_or(u64::MAX);
            // clang `DeduceFromInitializerList`: a non-dependent `P'`
            // (`to_array<long>({1, 2})`) deduces nothing from the elements.
            let items = if self.ast.types.is_dependent(elem) { items } else { Vec::new() };
            for v in items {
                let crate::init::InitValue::Expr(e) = v else { continue };
                let info = self.arg_info(e);
                let r = self.deduce_call_arg(elem, &info, d);
                if r != Tdk::Success {
                    return r;
                }
            }
            if let Some((_, Some(size))) = array {
                return self.deduce_array_bound(size, Some(Ok(n)), d);
            }
            return Tdk::Success;
        }
        let mut at = a.ty;
        // An overload set ([temp.deduct.call]/6): one non-template
        // function deduces from its type; otherwise non-deduced.
        if let Some(e) = a.expr {
            match self.overload_set_arg(e) {
                OverloadArg::Single(f) => {
                    let ft = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                    let addr =
                        self.ast.exprs.get(self.ignore_parens(e)).kind() == ExprKind::UnaryOperator;
                    at = if addr { self.ast.types.pointer_to(ft) } else { ft };
                }
                OverloadArg::NonDeduced => return Tdk::Success,
                OverloadArg::NotASet => {}
            }
        }
        let t = &mut self.ast.types;
        let mut at = t.canonical(at);
        match t.canonical_view(p) {
            TypeView::RValueRef(inner)
                if a.vk == ValueCategory::LValue
                    && inner.quals().is_empty()
                    && matches!(t.canonical_view(inner),
                        TypeView::CanonTemplateTypeParm { depth, .. } if depth as u16 == d.depth) =>
            {
                // A forwarding reference with an lvalue: `A&`.
                p = t.canonical(inner);
                at = t.lvalue_ref_to(at);
            }
            TypeView::LValueRef(inner) | TypeView::RValueRef(inner) => {
                p = t.canonical(inner);
                flags = flags.union(TdfFlags::PARAM_WITH_REFERENCE_TYPE);
            }
            _ => {
                if t.is_array(at) {
                    let e = t.element(at).unwrap_or(QualType::ERROR);
                    at = t.pointer_to(e);
                } else if t.is_function(at) {
                    at = t.pointer_to(at);
                } else {
                    at = t.unqualified(at);
                }
                at = t.canonical(at);
            }
        }
        if flags.contains(TdfFlags::PARAM_WITH_REFERENCE_TYPE) {
            // clang: P's top-level cv is intersected with A's.
            // clang: P's top-level cv is intersected with A's (an
            // array's are its element's).
            let aq = t.effective_quals(at).bits();
            p = QualType::new(p.ty(), p.quals_bits() & aq);
        }
        if matches!(t.canonical_view(at), TypeView::Pointer(_) | TypeView::MemberPointer { .. }) {
            flags = flags.union(TdfFlags::IGNORE_QUALIFIERS);
        }
        flags = flags.union(TdfFlags::DERIVED_CLASS);
        self.deduce(p, at, flags, d)
    }

    /// A mismatch in parameter type `p` is noted with the mismatched part
    /// as written (`T *`, not the canonical `type-parameter-0-0 *`): `p`,
    /// or what it refers or points to or the element of an array, at any
    /// depth (`S<T>` in `S<T> (&)[3]`).
    fn resugar_mismatch(&mut self, p: QualType, d: &mut DeducedArgs) {
        let t = &mut self.ast.types;
        let Some(first) = d.info.first else { return };
        let ArgView::Type(fq) = first.view() else { return };
        let want = t.canonical(fq);
        let mut cur = p;
        for _ in 0..64 {
            let bare = t.unqualified(cur);
            for cand in [cur, bare] {
                if t.canonical(cand) == want {
                    d.info.first = Some(TemplateArg::ty(cand));
                    return;
                }
            }
            cur = match t.view(cur.ty()) {
                TypeView::LValueRef(x) | TypeView::RValueRef(x) | TypeView::Pointer(x) => x,
                TypeView::ConstantArray(a) => a.elem,
                TypeView::IncompleteArray(e) => e,
                _ => {
                    let next = t.desugar_once(cur);
                    if next == cur {
                        return;
                    }
                    next
                }
            };
        }
    }

    /// The `A` of a mismatch as the argument's type spells it (`I`, not
    /// the canonical `int`, so the note adds `(aka 'int')`): the
    /// argument type, unqualified, or what it refers or points to or its
    /// element, at any depth, as [`Self::resugar_mismatch`] does for `P`.
    fn resugar_arg_mismatch(&mut self, a: QualType, d: &mut DeducedArgs) {
        let t = &mut self.ast.types;
        let Some(second) = d.info.second else { return };
        let ArgView::Type(sq) = second.view() else { return };
        let want = t.canonical(sq);
        let mut starts = vec![a];
        // An array argument decays for a non-reference `P` (`I *` from
        // `I[3]`).
        let mut x = t.unqualified(a);
        for _ in 0..64 {
            match t.view(x.ty()) {
                TypeView::ConstantArray(arr) => starts.push(arr.elem),
                TypeView::IncompleteArray(e) => starts.push(e),
                _ => {
                    let next = t.desugar_once(x);
                    if next != x {
                        x = next;
                        continue;
                    }
                }
            }
            break;
        }
        if let Some(&e) = starts.get(1) {
            starts[1] = t.pointer_to(e);
        }
        for mut cur in starts {
            for _ in 0..64 {
                let bare = t.unqualified(cur);
                for cand in [cur, bare] {
                    if t.canonical(cand) == want {
                        d.info.second = Some(TemplateArg::ty(cand));
                        return;
                    }
                }
                cur = match t.view(cur.ty()) {
                    TypeView::LValueRef(x) | TypeView::RValueRef(x) | TypeView::Pointer(x) => x,
                    TypeView::ConstantArray(a) => a.elem,
                    TypeView::IncompleteArray(e) => e,
                    _ => {
                        let next = t.desugar_once(cur);
                        if next == cur {
                            break;
                        }
                        next
                    }
                };
            }
        }
    }

    /// [`Self::resugar_mismatch`] for a function type P: the mismatched
    /// component as written in P's return or parameter types (`T *`, not
    /// `type-parameter-0-0 *`), or P itself.
    fn resugar_fn_mismatch(&mut self, p: QualType, d: &mut DeducedArgs) {
        let Some(first) = d.info.first else { return };
        let ArgView::Type(fq) = first.view() else { return };
        let t = &self.ast.types;
        let mut parts = vec![p];
        if let TypeView::Function(f) = t.view(p.ty()) {
            parts.push(f.ret());
            parts.extend(t.params(&f).iter().copied());
        }
        let want = t.canonical(fq);
        for cand in parts {
            let inner = match t.view(cand.ty()) {
                TypeView::LValueRef(x) | TypeView::RValueRef(x) => x,
                _ => cand,
            };
            for c in [cand, inner] {
                if t.canonical(c) == want {
                    d.info.first = Some(TemplateArg::ty(c));
                    return;
                }
            }
        }
    }

    /// `E` when `p` is `std::initializer_list<E>` (dependent or not).
    pub(crate) fn initializer_list_param(&mut self, p: QualType) -> Option<QualType> {
        if let TypeView::TemplateSpecialization { name, args, .. } =
            self.ast.types.canonical_view(p)
        {
            let tmpl = self.ast.types.template_decl(name)?;
            if Some(self.ast.first(tmpl)) != self.std_initializer_list().map(|x| self.ast.first(x))
            {
                return None;
            }
            return match self.ast.types.args(args).first()?.view() {
                ArgView::Type(t) => Some(t),
                _ => None,
            };
        }
        self.initializer_list_element(p)
    }

    /// For an argument naming an overload set (possibly `&set`): the one
    /// non-template function it names, or `Some(None)` when it names
    /// several or a template; `None` when it is not an overload set.
    fn overload_set_arg(&self, e: ExprId) -> OverloadArg {
        let mut e = self.ignore_parens(e);
        if self.ast.exprs.get(e).kind() == ExprKind::UnaryOperator {
            if self.ast.exprs.unary_op(e) != kiln_ast::expr::UnaryOp::AddrOf {
                return OverloadArg::NotASet;
            }
            e = self.ignore_parens(self.ast.exprs.operand(e));
        }
        if self.ast.exprs.get(e).kind() != ExprKind::UnresolvedLookupExpr {
            return OverloadArg::NotASet;
        }
        let decls = self.ast.unresolved_decls(e);
        let plain: Vec<DeclId> = decls
            .iter()
            .copied()
            .filter(|&d| self.kind_of(d) != DeclKind::FunctionTemplate)
            .collect();
        if plain.len() == 1 && plain.len() == decls.len() {
            return OverloadArg::Single(plain[0]);
        }
        OverloadArg::NonDeduced
    }

    /// The candidate note of a function template whose deduction failed
    /// (spec/sema-overload.md §10.4, clang `DiagnoseBadDeduction`).
    pub(crate) fn deduction_note(&self, ft: DeclId, f: &DeductionFailure, loc: Loc) -> Diag {
        let ft = self.ast.first(ft);
        if let Some(n) = self.deduction_failure_note(ft, f.tdk, &f.deduced, loc) {
            return n;
        }
        let param_name = |i: u16| {
            self.template_params(ft).get(i as usize).map(|&p| self.decl_name(p)).unwrap_or_default()
        };
        match f.tdk {
            Tdk::ConstraintsNotSatisfied => {
                let with = f.args.map(|l| format!(" {}", self.template_args_desc(ft, l)));
                let d = self
                    .diag(DiagId::note_ovl_candidate_unsatisfied_constraints, loc)
                    .arg(with.unwrap_or_default());
                match (f.sat, &f.diag) {
                    (Some(id), _) => self.with_sat_notes(d, id),
                    // An alias template guide's deducibility constraint.
                    (None, Some(why)) => d.with_note(why.clone()),
                    (None, None) => d,
                }
            }
            Tdk::SubstitutionFailure => {
                let with = f.args.map(|l| format!(" {}", self.template_args_desc(ft, l)));
                let msg = f.diag.as_ref().map(|x| format!(": {}", x.message()));
                self.diag(DiagId::note_ovl_candidate_substitution_failure, loc)
                    .arg(with.unwrap_or_default())
                    .arg(msg.unwrap_or_default())
            }
            Tdk::DeducedMismatch => {
                let with = f.args.map(|l| format!(" {}", self.template_args_desc(ft, l)));
                let ty = |x: Option<TemplateArg>| match x.map(|a| a.view()) {
                    Some(ArgView::Type(q)) => Some(q),
                    _ => None,
                };
                // A `%diff` pair in clang.
                let (da, aa) = match (ty(f.deduced.info.first), ty(f.deduced.info.second)) {
                    (Some(x), Some(y)) => self.ty_diff(x, y),
                    (x, y) => (
                        x.map(|q| self.ty_arg(q)).unwrap_or_default(),
                        y.map(|q| self.ty_arg(q)).unwrap_or_default(),
                    ),
                };
                self.diag(DiagId::note_ovl_candidate_deduced_mismatch, loc)
                    .arg(da)
                    .arg(ordinal(u32::from(f.deduced.info.param.unwrap_or(0)) + 1))
                    .arg(aa)
                    .arg(with.unwrap_or_default())
            }
            Tdk::InvalidExplicitArguments => {
                let i = f.deduced.info.param.unwrap_or(0);
                let name = param_name(i);
                if name.is_empty() {
                    self.diag(DiagId::note_ovl_candidate_explicit_arg_mismatch_unnamed, loc)
                        .arg(ordinal(u32::from(i) + 1))
                } else {
                    self.diag(DiagId::note_ovl_candidate_explicit_arg_mismatch_named, loc).arg(name)
                }
            }
            Tdk::InstantiationDepth => {
                self.diag(DiagId::note_ovl_candidate_instantiation_depth, loc)
            }
            Tdk::MiscellaneousDeductionFailure => {
                self.diag(DiagId::note_ovl_candidate_bad_deduction, loc)
            }
            _ => self.diag(DiagId::note_ovl_candidate, loc).arg("function template"),
        }
    }

    /// Whether the template of candidate `c1` of `set` is more
    /// specialized than that of `c2` ([over.match.best]/2.5) in a call
    /// with `nargs` arguments; for an operator, each candidate counts its
    /// own explicit arguments and reversal ([temp.func.order]/3).
    pub(crate) fn more_specialized_candidate(
        &mut self,
        set: &OverloadCandidateSet,
        c1: u32,
        c2: u32,
        nargs: usize,
    ) -> bool {
        let (x, y) = (set.cand(c1), set.cand(c2));
        let (Some(t1), Some(t2)) = (x.template, y.template) else { return false };
        // Conversion function templates order by their return types
        // ([temp.deduct.partial]/3.2); one never orders against a
        // constructor template.
        if x.kind == CandidateKind::ConvFn || y.kind == CandidateKind::ConvFn {
            return x.kind == y.kind && self.more_specialized_conversion(t1, t2);
        }
        if !set.oper {
            return self.more_specialized(t1, t2, nargs);
        }
        let n = |k: CandidateKind| {
            nargs + usize::from(k == CandidateKind::Function && set.object.is_some())
        };
        let reversed = x.rewrite.is_reversed() != y.rewrite.is_reversed();
        self.more_specialized_operator(t1, n(x.kind), t2, n(y.kind), reversed)
    }

    /// A rewritten or reversed operator function template candidate
    /// ([over.match.oper]/3.4, [temp.deduct.call]): deduction runs
    /// against the operands in the candidate's parameter order (swapped
    /// for a reversed `rewrite`); the hook `add_member_opers` and
    /// `add_nonmember_oper` call for templates (spec/sema-overload.md
    /// §10, WP-106).
    pub(crate) fn add_rewritten_template(
        &mut self,
        set: &mut OverloadCandidateSet,
        ft: DeclId,
        found: DeclId,
        kind: CandidateKind,
        rewrite: Rewrite,
        ops: &[ExprId],
    ) {
        if rewrite.is_reversed() {
            if ops.len() == 2 && self.should_add_reversed(ft, ops[1]) {
                self.add_reversed(set, ft, found, kind, rewrite);
            }
            return;
        }
        let loc = ops.first().map_or(set.loc, |&e| self.ast.exprs.get(e).loc());
        self.add_template_candidate(set, ft, found, None, kind, IcsOptions::NONE, loc);
        if let Some(c) = set.cands.last_mut() {
            c.rewrite = rewrite;
        }
    }
}

/// What a call argument naming an overload set deduces from
/// ([temp.deduct.call]/6).
enum OverloadArg {
    /// Not an overload set.
    NotASet,
    /// One non-template function: its type.
    Single(DeclId),
    /// Several, or a template: a non-deduced context.
    NonDeduced,
}

/// `1st`, `2nd`, `3rd`, `4th`, …
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}
