//! Template argument deduction from a function call
//! ([temp.deduct.call]; spec/sema-templates.md §6.1): explicit
//! arguments first, then each `(P, A)` pair with the call adjustments,
//! then defaults, then the substitution of the deduced arguments into
//! the signature in a SFINAE context. A failure is a
//! [`DeductionFailure`] the overload candidate keeps for its note
//! (§10.4).

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::ids::{ArgListId, NnsId, TemplateNameId};
use kiln_ast::types::{ArgView, ExcSpec, TemplateArg, TemplateName, TypeView};
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
    /// `NonDependentConversionFailure`: the call argument's index and
    /// its bad conversion sequence.
    pub(crate) bad: Option<(usize, crate::overload::Ics)>,
}

/// What a deduction without call arguments matches the template
/// against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DeduceTarget {
    /// Nothing: every argument explicit or defaulted.
    None,
    /// The function type of the address target ([temp.deduct.funcaddr]).
    FuncAddr(QualType),
    /// The type of an explicit instantiation or specialization
    /// ([temp.deduct.decl]): compared without either top-level exception
    /// specification, which is checked against the specialization's
    /// afterwards (clang `adjustCCAndNoReturn(..., AdjustExceptionSpec)`).
    Decl(QualType),
    /// The type a conversion function template converts to
    /// ([temp.deduct.conv]).
    Conversion(QualType),
}

impl DeductionFailure {
    fn new(tdk: Tdk, deduced: DeducedArgs) -> Box<DeductionFailure> {
        Box::new(DeductionFailure { tdk, deduced, args: None, diag: None, sat: None, bad: None })
    }
}

/// Marks the deduction step reached (feature `op-stats`).
macro_rules! stat_step {
    ($s:literal) => {
        #[cfg(feature = "op-stats")]
        crate::templ::deduce_stats::step($s);
    };
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("trailing_pack_skip");
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("check_partial_pack_lengths");
        let n = self.ast.types.args(shown).len();
        let last = n.checked_sub(1)?;
        if !matches!(d.args.get(last).map(TemplateArg::view), Some(ArgView::Pack(_))) {
            return None;
        }
        let partial = (d.depth, u16::try_from(last).ok()?);
        for &t in tys {
            let Some(pat) = crate::templ::pack::type_pattern(self, t) else { continue };
            let r = self.in_sfinae(FrameKind::ExplicitArgSubstitution, ft, Some(shown), loc, |s| {
                s.in_template_context(ft, |s| {
                    let mut sub = Substituter::new(s, levels);
                    sub.partial_pack = Some(partial);
                    let _ = sub.expand_pattern_at(None, |sub| sub.ty(pat, loc));
                });
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
        self.in_sfinae_held(kind, ft, args, loc, &mut Vec::new(), f)
    }

    /// [`Self::in_sfinae`], the warnings the frame held back (and nothing
    /// took) appended to `held`: one deduction's phases share them, as
    /// clang's `TemplateDeductionInfo` does (§5.7).
    pub(crate) fn in_sfinae_held<R>(
        &mut self,
        kind: FrameKind,
        ft: DeclId,
        args: Option<ArgListId>,
        loc: Loc,
        held: &mut Vec<Diag>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<(R, Result<(), Option<Diag>>)> {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("in_sfinae_held");
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
        let mut st = self.inst.sfinae.borrow_mut().pop().unwrap_or_default();
        self.pop_inst_frame();
        held.append(&mut st.warnings);
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
    /// template's implicit object parameter takes. `opts[i]`: the
    /// options of `args[i]`'s conversion sequence, for the
    /// non-dependent parameters checked before the function type is
    /// substituted ([temp.deduct.call]/10, CWG1391).
    pub(crate) fn deduce_call(
        &mut self,
        ft: DeclId,
        explicit: Option<ArgListId>,
        args: &[ArgInfo],
        opts: &[IcsOptions],
        loc: Loc,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("deduce_call");
        self.deduce_spec(ft, explicit, Some((args, opts)), DeduceTarget::None, loc)
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

    /// The specialization of `ft` an explicit instantiation or
    /// specialization of type `target` names ([temp.deduct.decl]): as
    /// [`Self::deduce_funcaddr`], but neither top-level exception
    /// specification takes part (nothing is deduced from it, and the
    /// template's is not substituted).
    pub(crate) fn deduce_decl(
        &mut self,
        ft: DeclId,
        explicit: Option<ArgListId>,
        target: QualType,
        loc: Loc,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        self.deduce_spec(ft, explicit, None, DeduceTarget::Decl(target), loc)
    }

    /// The specialization of conversion function template `ft` that
    /// converts to `to` ([temp.deduct.conv]): P is the template's
    /// conversion type, A `to` (a reference type for a reference
    /// binding).
    pub(crate) fn deduce_conversion(
        &mut self,
        ft: DeclId,
        to: QualType,
        obj: Option<ArgInfo>,
        loc: Loc,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        // An explicit object parameter is deduced from the object
        // argument too (clang's `DeduceTemplateArguments` for a
        // conversion function template, [temp.deduct.call]).
        let objs: Vec<ArgInfo> = obj.into_iter().collect();
        let opts = [IcsOptions::NONE];
        let call = (!objs.is_empty()).then_some((&objs[..], &opts[..]));
        let spec = self.deduce_spec(ft, None, call, DeduceTarget::Conversion(to), loc)?;
        if !self.specialize_lambda_conversion(ft, spec, loc) {
            let n = self.template_params(ft).len();
            let d = DeducedArgs::new(self.template_param_depth(ft), n);
            return Err(DeductionFailure::new(Tdk::NonDeducedMismatch, d));
        }
        Ok(spec)
    }

    /// Function type `f` returning `ret`.
    fn with_return_type(&mut self, f: QualType, ret: QualType) -> QualType {
        let Some(p) = self.ast.types.proto(f).copied() else { return f };
        let ps = self.ast.types.params(&p).to_vec();
        let exc = self.written_exc_spec(f);
        self.ast.types.function(ret, &ps, p.info(), exc)
    }

    /// The address of specialization `spec` of a function template with
    /// a deduced return type, taken for function type `target`: its
    /// return type is deduced from its definition (clang
    /// `DeduceReturnType` in `DeduceTemplateArguments`) and must be
    /// `target`'s. `Some(spec's type)` when it is not; `None` also while
    /// it stays undeduced (the use reports that).
    fn deduced_return_mismatch(
        &mut self,
        spec: DeclId,
        target: QualType,
        loc: Loc,
    ) -> Option<QualType> {
        let ret_of =
            |s: &Self, t: QualType| s.ast.types.proto(t).map(kiln_ast::types::FuncProto::ret);
        let sty = self.ast.decls.get(spec).ty()?;
        if ret_of(self, sty).is_some_and(|r| self.contains_undeduced_auto(r)) {
            self.instantiate_function_definition(spec, loc);
        }
        let sty = self.ast.decls.get(spec).ty()?;
        let (sret, tret) = (ret_of(self, sty)?, ret_of(self, target)?);
        if self.contains_undeduced_auto(sret) || self.ast.types.same_type(sret, tret) {
            return None;
        }
        Some(sty)
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
    fn deduce_spec(
        &mut self,
        ft: DeclId,
        explicit: Option<ArgListId>,
        call: Option<(&[ArgInfo], &[IcsOptions])>,
        target: DeduceTarget,
        loc: Loc,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        #[cfg(feature = "op-stats")]
        let (entry, key, outer) = {
            let entry = match target {
                DeduceTarget::None if call.is_some() => "call",
                DeduceTarget::None => "explicit",
                DeduceTarget::FuncAddr(_) => "funcaddr",
                DeduceTarget::Decl(_) => "decl",
                DeduceTarget::Conversion(_) => "conv",
            };
            let key = call.map(|(a, _)| {
                let v: Vec<_> = a.iter().map(|x| (x.ty, x.vk as u8, x.list.is_some())).collect();
                crate::templ::deduce_stats::key(
                    self.ast.first(ft).index(),
                    explicit.map(ArgListId::index),
                    &v,
                )
            });
            let outer = crate::templ::deduce_stats::begin(self.ast.first(ft).index());
            (entry, key, outer)
        };
        let mut params = self.deduce_scratch.pop().unwrap_or_default();
        let r = self.deduce_spec_steps(ft, explicit, call, target, loc, &mut params);
        self.deduce_scratch.push(params);
        #[cfg(feature = "op-stats")]
        crate::templ::deduce_stats::tally(
            "template",
            format!(
                "{} {:?} explicit={}",
                self.decl_name_str(self.ast.decls.get(ft).name()),
                r.as_ref().map_or_else(|f| f.tdk, |_| Tdk::Success),
                explicit.is_some()
            ),
        );
        #[cfg(feature = "op-stats")]
        crate::templ::deduce_stats::end(
            entry,
            r.as_ref().map_or_else(|f| f.tdk, |_| Tdk::Success),
            key,
            &outer,
        );
        r
    }

    /// The checks of §6.1 that need nothing substituted or allocated, in
    /// the full path's order: `ft` (its first declaration) declares a
    /// function (else `MiscellaneousDeductionFailure`), is valid (else
    /// `Invalid`), and a call of `nargs` arguments has at least the
    /// parameters without a default ([temp.deduct.call]/1; else
    /// `TooFewArguments`). `Ok` the template's function.
    fn deduce_head(&self, ft: DeclId, nargs: Option<usize>) -> Result<DeclId, Tdk> {
        stat_step!("invalid");
        let Some(templated) = self.templated_function(ft) else {
            return Err(Tdk::MiscellaneousDeductionFailure);
        };
        // clang `DeduceTemplateArguments`: an invalid template (its
        // function has an invalid parameter or return type, already
        // reported) deduces nothing.
        if (self.ast.decls.get(templated).has(DeclFlags::INVALID)
            && !self.templ.body_invalid.contains(&templated))
            || self.ast.decls.get(ft).has(DeclFlags::INVALID)
        {
            return Err(Tdk::Invalid);
        }
        // [temp.deduct.call]/1: clang checks the call's arity against
        // the written parameters before substituting the explicit
        // arguments (the maximum is checked once the parameter types are
        // known, below).
        stat_step!("arity-pre");
        if nargs.is_some_and(|k| k < self.fn_min_args(templated)) {
            return Err(Tdk::TooFewArguments);
        }
        Ok(templated)
    }

    /// §6.1 step 1, the substitution: `ft`'s function parameter types
    /// `tys` with the explicit arguments `levels` substituted (a function
    /// parameter pack keeps its pattern; a non-trailing one expands to its
    /// explicit elements, counted in the third result), and, for `whole`
    /// (`Some` the template's function type), the whole function type.
    /// Runs under the caller's `ExplicitArgSubstitution` frame.
    #[inline(never)]
    fn subst_explicit_param_types(
        &mut self,
        ft: DeclId,
        levels: &crate::templ::args::MultiLevelArgs,
        tys: &[QualType],
        whole: Option<QualType>,
        name_loc: Loc,
        loc: Loc,
    ) -> Result<(Vec<QualType>, Option<QualType>, usize), crate::templ::subst::SubstFail> {
        self.in_template_context(ft, |s| {
            let mut sub = Substituter::new(s, levels);
            sub.final_subst = true;
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
                    let t = sub.ty(t, loc)?;
                    v.push(sub.param_type(t, loc)?);
                    continue;
                };
                if j + 1 == tys.len() {
                    v.push(t);
                    continue;
                }
                match sub.expand_pattern_at(None, |sub| sub.ty(pat, loc))? {
                    crate::templ::pack::Expansion::Elements(e) => {
                        for t in e {
                            v.push(sub.param_type(t, loc)?);
                        }
                        kept += 1;
                    }
                    crate::templ::pack::Expansion::Unexpanded(_) => v.push(t),
                }
            }
            // clang `BuildFunctionType` reports at the function's
            // name (`Function->getLocation()`).
            let f = match whole {
                Some(pfn) => {
                    sub.type_begin = Some(name_loc);
                    let f = sub.ty(pfn, loc);
                    sub.type_begin = None;
                    Some(f?)
                }
                None => None,
            };
            Ok((v, f, kept))
        })
    }

    /// The failure [`Self::deduce_head`]'s `tdk` is: nothing deduced, at
    /// `loc`.
    pub(crate) fn deduce_head_failure(
        &self,
        ft: DeclId,
        tdk: Tdk,
        loc: Loc,
    ) -> Box<DeductionFailure> {
        let mut d = DeducedArgs::new(self.template_param_depth(ft), self.template_param_count(ft));
        d.loc = loc;
        DeductionFailure::new(tdk, d)
    }

    /// The pre-filter of a call candidate (WP-657; spec/sema-templates.md §6.1):
    /// the `Tdk` [`Self::deduce_call`] of `ft` with `nargs` arguments
    /// fails with before it substitutes or allocates anything, found
    /// without the argument lists; its failure is
    /// [`Self::deduce_head_failure`] of the first declaration. `None`
    /// when the full deduction is needed, and always under
    /// [`crate::Sema::full_deduction`].
    pub(crate) fn deduce_call_prefail(&self, ft: DeclId, nargs: usize) -> Option<Tdk> {
        if self.full_deduction {
            return None;
        }
        self.deduce_head(self.ast.first(ft), Some(nargs)).err()
    }

    #[allow(clippy::too_many_lines, reason = "the steps of §6.1 in order")]
    fn deduce_spec_steps(
        &mut self,
        ft: DeclId,
        explicit: Option<ArgListId>,
        call: Option<(&[ArgInfo], &[IcsOptions])>,
        target: DeduceTarget,
        loc: Loc,
        params_buf: &mut Vec<DeclId>,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        let (args, arg_opts) = call.unwrap_or((&[], &[]));
        let ft = self.ast.first(ft);
        let templated = match self.deduce_head(ft, call.map(|(a, _)| a.len())) {
            Ok(t) => t,
            Err(tdk) => return Err(self.deduce_head_failure(ft, tdk, loc)),
        };
        self.template_params_into(ft, params_buf);
        let params: &[DeclId] = params_buf;
        let depth = self.template_param_depth(ft);
        let n = params.len();
        let mut d = DeducedArgs::new(depth, n);
        d.loc = loc;
        let name_loc = self.ast.decls.get(templated).loc();
        // The warnings the phases before the specialization is formed
        // hold back (§5.7): kept with it on success, else dropped.
        let mut held: Vec<Diag> = Vec::new();
        let is_pack = |s: &Self, p: DeclId| s.ast.template_parm(p).is_some_and(|x| x.pack);
        // [temp.arg.explicit]/9: a template parameter pack anywhere in the
        // list (`template <class... A, class T>`) takes every remaining
        // explicit argument; only a list without one has a maximum arity.
        // An expanded parameter pack takes exactly its element count
        // ([temp.variadic]/5).
        let mut max_written = 0usize;
        let mut has_pack = false;
        for &p in params {
            match self.expanded_pack_types(p) {
                Some(t) => max_written += t.len(),
                None if is_pack(self, p) => has_pack = true,
                None => max_written += 1,
            }
        }
        // §6.1 step 1: the explicitly-specified arguments.
        let (mut ptys, variadic) = self.proto_param_types(templated);
        // CWG1391: the parameters the explicit arguments make
        // non-dependent (published on success; nested deductions in
        // between overwrite the field).
        let mut nondep = 0u64;
        // [temp.deduct.funcaddr]: P is the function type, with the
        // explicit arguments substituted below.
        let mut pfn = self.ast.decls.get(templated).ty().unwrap_or(QualType::ERROR);
        // [temp.deduct.decl], [temp.inst]/14: a declaration's match leaves
        // the template's exception specification alone (it is instantiated
        // when the specialization's is checked, not substituted here, so
        // `noexcept(T::value)` is no substitution failure); a non-dependent
        // one is kept for the mismatch note.
        let pexc = self.written_exc_spec(pfn);
        if matches!(target, DeduceTarget::Decl(_)) {
            pfn = self.with_exc_spec(pfn, ExcSpec::None);
        }
        // Expanded non-trailing function parameter packs.
        let mut retained = 0usize;
        let written: Vec<TemplateArg> =
            explicit.map(|l| self.ast.types.args(l).to_vec()).unwrap_or_default();
        // [temp.deduct.call]/1: clang checks the call's arity against
        // the written parameters before substituting the explicit
        // arguments, so `f<void>()` of `f(T&)` is "requires 1 argument"
        // rather than a substitution failure. A function parameter pack
        // anywhere leaves the maximum to the check after substitution.
        stat_step!("arity-pre");
        if let Some((a, _)) = call {
            let any_pack =
                ptys.iter().any(|&t| crate::templ::pack::type_pattern(self, t).is_some());
            if a.len() > ptys.len() && !variadic && !any_pack {
                return Err(DeductionFailure::new(Tdk::TooManyArguments, d));
            }
        }
        stat_step!("explicit");
        if !written.is_empty() {
            #[cfg(feature = "op-stats")]
            let _te = crate::templ::deduce_stats::timer("step-explicit");
            if written.len() > max_written && !has_pack {
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
            let r = self.in_sfinae_held(
                FrameKind::ExplicitArgSubstitution,
                ft,
                None,
                loc,
                &mut held,
                |s| {
                    let mut wi = 0usize;
                    for (i, &parm) in params.iter().enumerate() {
                        let Some(&w) = written.get(wi) else { break };
                        let prior: Vec<TemplateArg> = d.args[..i].to_vec();
                        // An expanded parameter pack takes one argument per
                        // element type ([temp.variadic]/5).
                        if let Some(tys) = s.expanded_pack_types(parm) {
                            let mut v = Vec::new();
                            while let Some(&w) = written.get(wi).filter(|_| v.len() < tys.len()) {
                                let ty = Some(tys[v.len()]);
                                let Ok(a) =
                                    s.check_expanded_pack_arg(ft, depth, parm, w, loc, &prior, ty)
                                else {
                                    bad = Some(i);
                                    return;
                                };
                                v.push(a);
                                wi += 1;
                            }
                            let l = s.ast.types.arg_list(&v);
                            d.args[i] = TemplateArg::pack(l);
                            continue;
                        }
                        wi += 1;
                        if is_pack(s, parm) {
                            // [temp.arg.explicit]/9: the rest form a pack,
                            // which deduction may extend.
                            let mut v = Vec::new();
                            for &w in &written[wi - 1..] {
                                let Ok(a) = s.check_template_arg(ft, depth, parm, w, loc, &prior)
                                else {
                                    bad = Some(i);
                                    return;
                                };
                                v.push(a);
                            }
                            let l = s.ast.types.arg_list(&v);
                            d.args[i] = TemplateArg::pack(l);
                            wi = written.len();
                            break;
                        }
                        let Ok(a) = s.check_template_arg(ft, depth, parm, w, loc, &prior) else {
                            bad = Some(i);
                            return;
                        };
                        d.args[i] = match a.view() {
                            ArgView::Type(q) => TemplateArg::ty(s.written_bare(q)),
                            _ => a,
                        };
                    }
                    // More arguments than an expanded pack and the parameters
                    // after it take.
                    if wi < written.len() {
                        bad = Some(params.len().saturating_sub(1));
                    }
                },
            );
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
            // clang substitutes the whole function type for a target type
            // and for a template-id naming one specialization without a
            // call (`ResolveSingleFunctionTemplateSpecialization` takes the
            // `ArgFunctionType`-less funcaddr deduction).
            let whole = match target {
                DeduceTarget::FuncAddr(_) | DeduceTarget::Decl(_) => true,
                DeduceTarget::None => call.is_none(),
                DeduceTarget::Conversion(_) => false,
            } && !fn_has_pack;
            // clang substitutes the explicit arguments as written
            // (`SubstituteExplicitTemplateArguments` is `Final`): a
            // parameter they make non-dependent keeps their sugar, so
            // `f<s::string>(1, 2)` of `f(W<T>, int)` notes
            // `'W<s::string>' (aka 'W<basic_string<char>>')`.
            let mut sugared = d.args.clone();
            for (a, &w) in sugared.iter_mut().zip(&written) {
                if let (ArgView::Type(c), ArgView::Type(wt)) = (a.view(), w.view())
                    && self.ast.types.canonical(c) == self.ast.types.canonical(wt)
                {
                    *a = TemplateArg::ty(wt);
                }
            }
            let sugared_levels = self.prior_levels(ft, depth, &sugared);
            #[cfg(feature = "op-stats")]
            let _te2 = crate::templ::deduce_stats::timer("step-explicit-subst");
            #[cfg(feature = "op-stats")]
            let stat_key = crate::templ::deduce_stats::key_of(&(
                ft,
                &sugared_levels,
                whole.then_some(pfn),
                held.len(),
            ));
            #[cfg(feature = "op-stats")]
            let stat_nodes = (self.ast.node_count(), self.ast.node_live_bytes(), held.len());
            // The memo (WP-661, `explicit_memo.rs`): a repeat of a
            // substitution that only computed types is answered in the
            // same frame.
            let memo_key = if self.full_deduction {
                None
            } else {
                let args = sugared_levels.levels.last().map_or(ArgListId::EMPTY, |l| l.args);
                crate::templ::explicit_memo::ExplicitKey::new(
                    ft,
                    depth,
                    args,
                    whole.then_some(pfn),
                    &tys,
                )
            };
            let before = (
                self.ast.node_count(),
                self.ast.node_live_bytes(),
                self.ast.instantiation_count(),
                held.len(),
                self.diags.error_count(),
                self.diags.warning_count(),
            );
            let mut answered = false;
            let r = self.in_sfinae_held(
                FrameKind::ExplicitArgSubstitution,
                ft,
                Some(shown),
                loc,
                &mut held,
                |s| {
                    if let Some(hit) = memo_key.as_ref().and_then(|k| s.explicit_memo.get(k)) {
                        answered = true;
                        return Ok(hit);
                    }
                    s.subst_explicit_param_types(
                        ft,
                        &sugared_levels,
                        &tys,
                        whole.then_some(pfn),
                        name_loc,
                        loc,
                    )
                },
            );
            if let (Some(k), false, Some((Ok((t, f, kept)), Ok(())))) = (memo_key, answered, &r)
                && before
                    == (
                        self.ast.node_count(),
                        self.ast.node_live_bytes(),
                        self.ast.instantiation_count() - 1,
                        held.len(),
                        self.diags.error_count(),
                        self.diags.warning_count(),
                    )
                && !self.decl_in_local_scope(ft)
            {
                self.explicit_memo.put(k, t, *f, *kept);
            }
            #[cfg(feature = "op-stats")]
            crate::templ::deduce_stats::explicit_answered(answered);
            #[cfg(feature = "op-stats")]
            crate::templ::deduce_stats::explicit_subst(
                stat_key,
                matches!(r, Some((Ok(_), Ok(())))) && held.len() == stat_nodes.2,
                self.ast.node_count() - stat_nodes.0
                    + u64::from(self.ast.node_live_bytes() != stat_nodes.1),
            );
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
        // is not deduced from). A function parameter pack anywhere (clang
        // `isTemplateVariadic`) leaves the check to the specialization's
        // viability: a non-trailing one is a non-deduced context, deduced
        // empty, so `f(1, 2, 3)` of `f(T..., U)` is `f<int>` with "requires
        // 1 argument".
        stat_step!("arity");
        let nargs = args.len();
        let fn_pack = ptys.last().and_then(|&t| crate::templ::pack::type_pattern(self, t));
        let any_pack = ptys.iter().any(|&t| crate::templ::pack::type_pattern(self, t).is_some());
        if call.is_some() && nargs > ptys.len() + retained && !variadic && !any_pack {
            return Err(DeductionFailure::new(Tdk::TooManyArguments, d));
        }
        if call.is_some() && nargs < self.fn_min_args(templated) {
            return Err(DeductionFailure::new(Tdk::TooFewArguments, d));
        }
        // [temp.deduct.funcaddr]: the function type against the target.
        // clang `DeduceTemplateArguments` (C++14): a deduced return type
        // is a non-deduced context (`SubstAutoTypeDependent`); a
        // declaration's must then be the same placeholder (the caller
        // compares the types), the address of a function deduces it from
        // the specialization's definition and compares it below.
        stat_step!("target");
        let placeholder_ret = matches!(target, DeduceTarget::FuncAddr(_) | DeduceTarget::Decl(_))
            && self.ast.types.proto(pfn).is_some_and(|p| self.contains_placeholder(p.ret()));
        if let DeduceTarget::FuncAddr(a) | DeduceTarget::Decl(a) = target {
            let decl = matches!(target, DeduceTarget::Decl(_));
            let pfn = if placeholder_ret
                && let Some(aret) = self.ast.types.proto(a).map(kiln_ast::types::FuncProto::ret)
            {
                self.with_return_type(pfn, aret)
            } else {
                pfn
            };
            let (p0, a0, tdf) = if decl {
                let p0 = self.with_exc_spec(pfn, ExcSpec::None);
                (p0, self.with_exc_spec(a, ExcSpec::None), TdfFlags::TOP_LEVEL_PARAMETER_TYPE_LIST)
            } else {
                let tdf = TdfFlags::IGNORE_NONDEDUCED_NOEXCEPT
                    .union(TdfFlags::TOP_LEVEL_PARAMETER_TYPE_LIST);
                (pfn, a, tdf)
            };
            let r = self.deduce(p0, a0, tdf, &mut d);
            if r != Tdk::Success {
                // clang shows the whole types with their specifications.
                let whole_p = d.info.first.is_some_and(|f| match f.view() {
                    ArgView::Type(t) => self.ast.types.same_type(t, p0),
                    _ => false,
                });
                if decl && whole_p {
                    let dependent = matches!(
                        pexc,
                        ExcSpec::DependentNoexcept(_)
                            | ExcSpec::Deferred(_)
                            | ExcSpec::NoexceptExpr {
                                value: kiln_ast::types::NoexceptValue::Dependent(_),
                                ..
                            }
                    );
                    let shown = if dependent { pfn } else { self.with_exc_spec(pfn, pexc) };
                    d.info.first = Some(TemplateArg::ty(shown));
                    d.info.second = Some(TemplateArg::ty(a));
                }
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
                // result; clang 19 the canonical type of a mismatch,
                // clang 22 the written one (`T *`, not
                // `type-parameter-0-0 *`).
                let mismatch = r == Tdk::NonDeducedMismatch && self.clang_compat >= 22;
                if (r == Tdk::Underqualified || mismatch)
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
        stat_step!("deduce");
        let skip = fn_pack.and_then(|pat| self.trailing_pack_skip(pat, params, &d));
        // [temp.deduct.call]/4: the (argument index, P, adjusted A) of
        // each deduced-from argument, checked against the deduced A.
        let mut originals: Vec<(usize, QualType, QualType)> = Vec::new();
        // The parameters still to be deduced (not given explicitly): a
        // deduction from a call argument takes the argument's sugar.
        let undeduced: Vec<bool> =
            d.args.iter().map(|x| matches!(x.view(), ArgView::Null)).collect();
        // [temp.deduct.call]/10: the (argument index, P) of each P that
        // takes no part in deduction, checked before substitution.
        let mut nondeduced: Vec<(usize, QualType)> = Vec::new();
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
                nondeduced.push((i, p));
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
            if r == Tdk::Success {
                self.resugar_call_deduction(p, a, &undeduced, &mut d);
            }
            if r == Tdk::Success
                && self.has_deducible_params(p, &d, false)
                && self.is_original_arg(a)
            {
                originals.push((i, p, self.adjusted_call_arg(p, a, d.depth)));
            }
        }
        stat_step!("defaults");
        // §6.1 step 3: defaults, then completeness.
        for (j, &p) in params.iter().enumerate() {
            // An expanded parameter pack needs one argument per element
            // type ([temp.variadic]/5; clang's `TDK_IncompletePack`).
            if let Some(tys) = self.expanded_pack_types(p) {
                let have = match d.args[j].view() {
                    ArgView::Pack(l) => self.ast.types.args(l).len(),
                    _ => 0,
                };
                if have < tys.len() {
                    if matches!(d.args[j].view(), ArgView::Null) {
                        d.args[j] = TemplateArg::pack(self.ast.types.arg_list(&[]));
                    }
                    d.info.param = u16::try_from(j).ok();
                    d.info.first = Some(d.args[j]);
                    return Err(DeductionFailure::new(Tdk::IncompletePack, d));
                }
            }
            if !matches!(d.args[j].view(), ArgView::Null) {
                // [temp.arg.template]/3: a deduced template template
                // argument must match its parameter (clang
                // `ConvertDeducedTemplateArgument`).
                if self.kind_of(p) == DeclKind::TemplateTemplateParm {
                    let prior: Vec<TemplateArg> = d.args[..=j].to_vec();
                    let shown = self.ast.types.arg_list(&prior);
                    let r = self.in_sfinae_held(
                        FrameKind::DeducedArgSubstitution,
                        ft,
                        Some(shown),
                        loc,
                        &mut held,
                        |s| s.check_deduced_template_template_arg(ft, depth, p, j, &prior, loc),
                    );
                    match r {
                        None => return Err(DeductionFailure::new(Tdk::InstantiationDepth, d)),
                        Some((true, Ok(()))) => {}
                        Some((_, failed)) => {
                            // Shown without the deduced arguments, as
                            // clang does.
                            let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                            f.diag = failed.err().flatten();
                            return Err(f);
                        }
                    }
                }
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
            #[cfg(feature = "op-stats")]
            let _td = crate::templ::deduce_stats::timer("step-defaults");
            let prior: Vec<TemplateArg> = d.args[..j].to_vec();
            let shown = self.ast.types.arg_list(&prior);
            let r = self.in_sfinae_held(
                FrameKind::DeducedArgSubstitution,
                ft,
                Some(shown),
                loc,
                &mut held,
                |s| s.default_template_arg(ft, depth, p, def, loc, &prior),
            );
            match r {
                None => return Err(DeductionFailure::new(Tdk::InstantiationDepth, d)),
                Some((Ok(a), Ok(()))) => d.args[j] = a,
                Some((formed, failed)) => {
                    // A default that was formed but diagnosed (an
                    // inaccessible `typename T::P`) is shown with the
                    // failure, as clang keeps it in the deduced list.
                    let shown = match formed {
                        Ok(a) => {
                            let mut with = prior.clone();
                            with.push(a);
                            self.ast.types.arg_list(&with)
                        }
                        Err(()) => shown,
                    };
                    let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                    f.args = Some(shown);
                    f.diag = failed.err().flatten();
                    return Err(f);
                }
            }
        }
        stat_step!("nondep");
        let sugared = self.ast.types.arg_list(&d.args);
        // [temp.deduct.call]/10 (CWG1391): an argument that does not
        // convert to its non-dependent P fails deduction before the
        // function type is substituted (and before the constraints are
        // checked, as clang 19 does), so a later parameter's
        // substitution (libc++ `shared_ptr(nullptr_t, D, enable_if_t<
        // __well_formed_deleter<D, nullptr_t>>)` for an `int*`
        // argument) is not instantiated (clang `CheckNonDependent`).
        for &(i, p) in &nondeduced {
            let o = arg_opts.get(i).copied().unwrap_or(IcsOptions::NONE);
            let x = self.try_implicit_conversion(&args[i], p, o);
            if x.is_bad() {
                let mut f = DeductionFailure::new(Tdk::NonDependentConversionFailure, d);
                f.args = Some(sugared);
                f.bad = Some((i, x));
                return Err(f);
            }
        }
        // clang 22 (CWG2369): the associated constraints are checked
        // before the deduced arguments are substituted into the function
        // type, so a candidate failing both reports the constraints
        // (spec/sema-templates.md §6.1).
        stat_step!("constraints");
        let early = self.clang_compat >= 22;
        if early {
            self.deduced_constraints(ft, depth, &d, sugared, loc)?;
        }
        stat_step!("signature");
        // §6.1 step 4: the specialization, its signature substituted.
        let canonical = self.ast.types.canonical_args(sugared);
        let conv = ConvertedArgs { sugared, canonical };
        // Whether this deduction forms the specialization (its own
        // held-back warnings then stored with it, `get_function_spec_signature`).
        let fresh = held.is_empty() || self.ast.templates.find_spec(ft, canonical).is_none();
        let r = self.in_sfinae(FrameKind::DeducedArgSubstitution, ft, Some(sugared), loc, |s| {
            s.get_function_spec_signature(ft, conv, loc)
        });
        match r {
            None => Err(DeductionFailure::new(Tdk::InstantiationDepth, d)),
            Some((Ok(_), Ok(()))) if self.templ.body_invalid.contains(&templated) => {
                // The specialization of a function whose body broke a
                // rule is invalid: a substitution failure without a
                // reason (clang `FinishTemplateArgumentDeduction`).
                let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                f.args = Some(sugared);
                Err(f)
            }
            Some((Ok(spec), Ok(()))) => {
                // [temp.deduct.general]/5: the associated constraints
                // ([temp.constr.decl]/3) are checked once the function
                // type has been substituted without failure, so a
                // candidate failing both reports the substitution failure
                // (clang 19 `FinishTemplateArgumentDeduction`; §6.1, §8.3).
                stat_step!("constraints");
                if !early {
                    self.deduced_constraints(ft, depth, &d, sugared, loc)?;
                }
                stat_step!("post");
                // The explicit-specifier, once the constraints are
                // satisfied (clang `instantiateExplicitSpecifierDeferred`).
                if self.templ.explicit_pending.contains(&spec) {
                    let r = self.in_sfinae(
                        FrameKind::DeducedArgSubstitution,
                        ft,
                        Some(sugared),
                        loc,
                        |s| s.finish_function_spec_explicit(spec),
                    );
                    match r {
                        None => return Err(DeductionFailure::new(Tdk::InstantiationDepth, d)),
                        Some((Ok(()), Ok(()))) => {}
                        Some((_, failed)) => {
                            let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                            f.args = Some(sugared);
                            f.diag = failed.err().flatten();
                            return Err(f);
                        }
                    }
                }
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
                if placeholder_ret
                    && let DeduceTarget::FuncAddr(a) = target
                    && let Some(m) = self.deduced_return_mismatch(spec, a, loc)
                {
                    d.info.first = Some(TemplateArg::ty(m));
                    d.info.second = Some(TemplateArg::ty(a));
                    let mut f = DeductionFailure::new(Tdk::NonDeducedMismatch, d);
                    f.args = Some(sugared);
                    return Err(f);
                }
                self.keep_deduction_warnings(spec, fresh, held);
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
    /// [temp.deduct.general]/5: the associated constraints of `ft`
    /// ([temp.constr.decl]/3) with the deduced arguments `sugared`.
    fn deduced_constraints(
        &mut self,
        ft: DeclId,
        depth: u16,
        d: &DeducedArgs,
        sugared: ArgListId,
        loc: Loc,
    ) -> Result<(), Box<DeductionFailure>> {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("deduced_constraints");
        let constraints = self.associated_constraints(ft);
        if constraints.is_empty() {
            return Ok(());
        }
        let levels = self.prior_levels(ft, depth, &d.args);
        let sat = self.check_constraints(ft, &constraints, &levels, sugared, loc);
        if self.sat_ok(sat) {
            return Ok(());
        }
        let mut f = DeductionFailure::new(Tdk::ConstraintsNotSatisfied, d.clone());
        f.args = Some(sugared);
        f.sat = Some(sat);
        Err(f)
    }

    fn has_deducible_params(&mut self, p: QualType, d: &DeducedArgs, pattern: bool) -> bool {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("has_deducible_params");
        let mut probe = DeducedArgs::new(d.depth, d.args.len());
        probe.loc = d.loc;
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("has_deducible_pack_expansion");
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
            // [temp.deduct.type]/5.3: a non-type pattern deduces only as
            // a bare parameter of the deduced depth (`N...`); any other
            // expression (`conv<Args>()...`, `sizeof(Args)...`) is a
            // non-deduced context.
            Some(ArgView::Expr { expr, .. }) => self.nttp_of_expr_at(Some(expr), d.depth),
            Some(ArgView::Profile(pr)) => {
                let e = self.ast.types.profile_expr(pr);
                self.nttp_of_expr_at(e, d.depth)
            }
            Some(_) => true,
            None => matches!(a.view(), ArgView::Type(q) if self.has_deducible_pack_expansion(q, d)),
        })
    }

    /// The template parameter packs pack expansion pattern `p` names.
    fn pattern_packs(&mut self, p: QualType, d: &DeducedArgs) -> Vec<u16> {
        let mut probe = DeducedArgs::new(d.depth, d.args.len());
        probe.loc = d.loc;
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("adjusted_call_arg");
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("original_arg_mismatch");
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
        // A function lvalue binds a reference to the function type without
        // `noexcept` ([dcl.init.ref]/4, reference-compatible).
        if self.ast.types.is_function(ca) {
            let (pa, pd) = (self.ast.types.pointer_to(ca), self.ast.types.pointer_to(cd));
            if crate::conv::is_function_pointer_conversion(self, pa, pd) {
                return None;
            }
        }
        // /4.3: a derived class (or pointer to one) of a simple-template-id.
        if let TypeView::Pointer(pp) = self.ast.types.canonical_view(op) {
            op = self.ast.types.canonical(pp);
            // clang strips the pointers only to a class (`A`'s pointee).
            if let (TypeView::Pointer(x), TypeView::Pointer(y)) =
                (self.ast.types.canonical_view(ca), self.ast.types.canonical_view(cd))
                && self.ast.types.as_record(x).is_some()
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("deduce_call_arg");
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
        // An overload set ([temp.deduct.call]/6): the type of the one
        // member it resolves to, else a non-deduced context.
        if let Some(e) = a.expr
            && let OverloadArg::Set(set) = self.overload_set_arg(e)
        {
            match self.resolve_overload_for_deduction(p, &set, d) {
                Some(t) => at = t,
                None => return Tdk::Success,
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
                // /2: P's top-level cv is ignored (`void f(const T t)`; the
                // written prototype keeps it, clang
                // `AdjustFunctionParmAndArgTypesForDeduction`; WP-231).
                p = t.unqualified(p);
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
        // /4.2: a (pointer to a, reference to a) function type deduces
        // regardless of `noexcept`, as clang's type match does; the
        // function pointer conversion is checked on the deduced `A`
        // (`deduced_a_mismatch`).
        flags =
            flags.union(TdfFlags::DERIVED_CLASS).union(TdfFlags::ALLOW_COMPATIBLE_FUNCTION_TYPE);
        self.deduce(p, at, flags, d)
    }

    /// [temp.deduct.call]/2-3 as clang deduces over sugared types: a `P`
    /// that is a template type parameter `cv T` (or a reference to one,
    /// or a forwarding reference) deduces `T` as the argument type as
    /// written (`N::H<int>`, not the canonical `H<int>` the canonical
    /// deduction recorded), so `[with T = ...]` bindings and the
    /// specialization's type print as clang prints them. Applied only to
    /// a parameter not given explicitly whose recorded deduction is the
    /// canonical form of the sugared candidate; the first argument's
    /// sugar is kept (clang takes the common sugar of all of them).
    fn resugar_call_deduction(
        &mut self,
        p: QualType,
        a: &ArgInfo,
        undeduced: &[bool],
        d: &mut DeducedArgs,
    ) {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("resugar_call_deduction");
        if a.list.is_some()
            || a.expr.is_some_and(|e| !matches!(self.overload_set_arg(e), OverloadArg::NotASet))
        {
            return;
        }
        let depth = u32::from(d.depth);
        let t = &mut self.ast.types;
        let parm = |t: &kiln_ast::types::TypeContext, q: QualType| match t.view(q.ty()) {
            TypeView::CanonTemplateTypeParm { depth: x, index, pack: false } if x == depth => {
                Some(index as usize)
            }
            _ => None,
        };
        let cp = t.canonical(p);
        let (index, cand) = match t.canonical_view(cp) {
            TypeView::RValueRef(inner)
                if a.vk == ValueCategory::LValue && inner.quals().is_empty() =>
            {
                let Some(i) = parm(t, t.canonical(inner)) else { return };
                (i, t.lvalue_ref_to(a.ty))
            }
            TypeView::LValueRef(inner) | TypeView::RValueRef(inner) => {
                let inner = t.canonical(inner);
                let Some(i) = parm(t, inner) else { return };
                if t.is_array(a.ty) && !inner.quals().is_empty() {
                    return;
                }
                let aq = t.effective_quals(a.ty).bits() & !inner.quals_bits();
                let u = t.unqualified(a.ty);
                (i, t.qualify(u, kiln_ast::types::Quals::from_bits(aq)))
            }
            _ => {
                let Some(i) = parm(t, cp.without_quals()) else { return };
                if t.is_array(a.ty) || t.is_function(a.ty) {
                    return;
                }
                (i, t.unqualified(a.ty))
            }
        };
        if !undeduced.get(index).copied().unwrap_or(false) {
            return;
        }
        let Some(&cur) = d.args.get(index) else { return };
        let ArgView::Type(cq) = cur.view() else { return };
        if t.canonical(cq) != cq || t.canonical(cand) != cq {
            return;
        }
        // A class or enumeration named bare is canonical in kiln
        // (spec/types.md R4); marked as written, so the binding prints it
        // unscoped (clang's `ElaboratedType` without a qualifier).
        let cand = if cand == cq { self.written_bare(cand) } else { cand };
        if cand != cq {
            d.args[index] = TemplateArg::ty(cand);
        }
    }

    /// `q` marked as written, if it is a bare (canonical) class or
    /// enumeration type, `cv`-qualified or not: an `Elaborated` node with
    /// neither keyword nor qualifier, which deduced bindings print
    /// unscoped where they print a canonical class scoped
    /// ([`Sema::deduced_args_desc`]).
    pub(crate) fn written_bare(&mut self, q: QualType) -> QualType {
        let t = &mut self.ast.types;
        let u = t.unqualified(q);
        if t.canonical(u) != u || !matches!(t.view(u.ty()), TypeView::Record(_) | TypeView::Enum(_))
        {
            return q;
        }
        let cv = t.effective_quals(q);
        let w = t.elaborated(kiln_ast::types::TagKeyword::None, None, u);
        t.qualify(w, cv)
    }

    /// A mismatch in parameter type `p` is noted with the mismatched part
    /// as written (`T *`, not the canonical `type-parameter-0-0 *`): `p`,
    /// or what it refers or points to or the element of an array, at any
    /// depth (`S<T>` in `S<T> (&)[3]`).
    fn resugar_mismatch(&mut self, p: QualType, d: &mut DeducedArgs) {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("resugar_mismatch");
        let Some(first) = d.info.first else { return };
        if let ArgView::Template(n) = first.view() {
            if let Some(w) = self.written_template_in(p, n) {
                d.info.first = Some(TemplateArg::template(w));
            }
            return;
        }
        let ArgView::Type(fq) = first.view() else { return };
        let want = self.ast.types.canonical(fq);
        let mut cur = p;
        for _ in 0..64 {
            let bare = self.ast.types.unqualified(cur);
            for cand in [cur, bare] {
                if self.ast.types.canonical(cand) == want {
                    d.info.first = Some(TemplateArg::ty(cand));
                    return;
                }
            }
            // The element of a `std::initializer_list<E>` parameter
            // ([temp.deduct.call]/1), as written (`value_type`).
            if let TypeView::TemplateSpecialization { args, .. } = self.ast.types.view(cur.ty())
                && self.initializer_list_param(cur).is_some()
                && let Some(ArgView::Type(e)) =
                    self.ast.types.args(args).first().map(TemplateArg::view)
            {
                cur = e;
                continue;
            }
            let t = &mut self.ast.types;
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("resugar_arg_mismatch");
        let Some(second) = d.info.second else { return };
        if let ArgView::Template(n) = second.view() {
            if let Some(w) = self.written_template_in(a, n) {
                d.info.second = Some(TemplateArg::template(w));
                d.info.second_from_decl = false;
            }
            return;
        }
        let t = &mut self.ast.types;
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

    /// The template name of `n`'s template as the first template-id
    /// naming it in `q`'s sugar spells it (clang deduces over sugared
    /// types and reports the `TemplateName`s as written): `N::A` for a
    /// template-id under an `N::` qualifier, else the template-id's own
    /// name. Searches pointees, elements, function parts and type
    /// template arguments, depth first, a bounded number of nodes.
    fn written_template_in(&mut self, q: QualType, n: TemplateNameId) -> Option<TemplateNameId> {
        let t = &self.ast.types;
        let want = t.template_decl(n).map(|x| self.ast.first(x))?;
        let mut stack = vec![(q, None::<NnsId>, false)];
        let mut found = None;
        for _ in 0..256 {
            let Some((cur, nns, aliased)) = stack.pop() else { break };
            let t = &self.ast.types;
            match t.view(cur.ty()) {
                TypeView::TemplateSpecialization { name, args, .. } => {
                    if t.template_decl(name).map(|x| self.ast.first(x)) == Some(want) {
                        found = Some((name, nns));
                        break;
                    }
                    for a in t.args(args).iter().rev() {
                        if let ArgView::Type(x) = a.view() {
                            stack.push((x, None, false));
                        }
                    }
                }
                // A deduced class template placeholder: clang takes the
                // specialized template, qualified only by the elaborated
                // qualifier around the placeholder (`s::pr` for
                // `s::pr{1, 2}`, `pr` for `pr{1, 2}`).
                TypeView::DeducedTemplateSpecialization { name, deduced: Some(_) }
                    if t.template_decl(name).map(|x| self.ast.first(x)) == Some(want) =>
                {
                    let d = t.template_decl(name)?;
                    // kiln keeps the placeholder's qualifier in its name.
                    let written = match t.template_name_view(name) {
                        TemplateName::Qualified { nns, .. } => Some(nns),
                        _ => None,
                    };
                    found = Some((self.template_name_for(d), written.or(nns)));
                    break;
                }
                // Below an alias the qualifier is the one written on the
                // alias's own name (clang's outermost `ElaboratedType`,
                // which has none for an unqualified name): `pair` for `P`
                // and `t::pair` for `t::Q`, both naming `std::pair<...>`.
                TypeView::Elaborated { nns: written, named, .. } => {
                    stack.push((named, if aliased { nns } else { written }, aliased));
                }
                TypeView::LValueRef(x) | TypeView::RValueRef(x) | TypeView::Pointer(x) => {
                    stack.push((x, None, false));
                }
                TypeView::ConstantArray(a) => stack.push((a.elem, None, false)),
                TypeView::IncompleteArray(e) => stack.push((e, None, false)),
                TypeView::Function(f) => {
                    for &x in t.params(&f).iter().rev() {
                        stack.push((x, None, false));
                    }
                    stack.push((f.ret(), None, false));
                }
                _ => {
                    let next = t.desugar_once(cur);
                    if next != cur {
                        stack.push((next, nns, true));
                    }
                }
            }
        }
        let (name, nns) = found?;
        let t = &mut self.ast.types;
        Some(match (nns, t.template_name_view(name)) {
            (Some(nns), TemplateName::Template(_)) => t.template_name(TemplateName::Qualified {
                nns,
                template_kw: false,
                underlying: name,
            }),
            _ => name,
        })
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

    /// For an argument naming an overload set (possibly `&set`, clang
    /// `OverloadExpr::find`): the set and its form; `NotASet` when it is
    /// not one.
    fn overload_set_arg(&self, e: ExprId) -> OverloadArg {
        let mut e = self.ignore_parens(e);
        let mut addr = false;
        let mut member_form = false;
        if self.ast.exprs.get(e).kind() == ExprKind::UnaryOperator {
            if self.ast.exprs.unary_op(e) != kiln_ast::expr::UnaryOp::AddrOf {
                return OverloadArg::NotASet;
            }
            let operand = self.ast.exprs.operand(e);
            e = self.ignore_parens(operand);
            addr = true;
            // [expr.unary.op]/4: only `&X::f`, unparenthesized, forms a
            // pointer to member.
            member_form = operand == e && self.ast.templates.expr_nns.get(e).is_some();
        }
        if self.ast.exprs.get(e).kind() != ExprKind::UnresolvedLookupExpr {
            return OverloadArg::NotASet;
        }
        OverloadArg::Set(OverloadSetArg { ule: e, addr, member_form })
    }

    /// The argument type `A` the overload set `set` gives call parameter
    /// `p` ([temp.deduct.call]/6, clang `ResolveOverloadForDeduction`),
    /// or `None` for a non-deduced context. For a `P` that is a function
    /// type, a pointer to function or a pointer to member function
    /// (after the reference and its top-level cv are dropped), deduction
    /// is tried against each member: a set with a function template is
    /// non-deduced (unless the set is a template-id, whose templates
    /// each contribute the specialization the explicit arguments name),
    /// and the type of the only member deduction succeeds for is `A`
    /// (several succeeding: non-deduced). Each trial starts from nothing
    /// deduced ([temp.deduct.type]/2: the pairs are deduced
    /// independently); the explicit template arguments are already
    /// substituted into `p`. For any other `P`, the one non-template
    /// function the set names, if it names one.
    fn resolve_overload_for_deduction(
        &mut self,
        p: QualType,
        set: &OverloadSetArg,
        d: &DeducedArgs,
    ) -> Option<QualType> {
        let t = &mut self.ast.types;
        let c = t.canonical(p);
        let mut p = t.unqualified(c);
        let mut by_ref = false;
        if let TypeView::LValueRef(x) | TypeView::RValueRef(x) = t.canonical_view(p) {
            p = t.canonical(x);
            by_ref = true;
        }
        let fn_like = match t.canonical_view(p) {
            TypeView::Function(_) => true,
            TypeView::Pointer(x) => t.is_function(x),
            TypeView::MemberPointer { pointee, .. } => t.is_function(pointee),
            _ => false,
        };
        let decls: Vec<DeclId> = self
            .ast
            .unresolved_decls(set.ule)
            .into_iter()
            .filter(|&x| !self.ast.decls.get(x).has(DeclFlags::INVALID))
            .collect();
        if !fn_like {
            // clang `resolveAddressOfSingleOverloadCandidate`.
            let [f] = decls[..] else { return None };
            if self.kind_of(f) == DeclKind::FunctionTemplate {
                return None;
            }
            return self.overload_member_type(f, set);
        }
        let explicit = self.ast.templates.expr_targs.get(set.ule).copied();
        if explicit.is_some_and(|l| {
            self.ast
                .types
                .arg_list_flags(l)
                .intersects(kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION)
        }) {
            return None;
        }
        let loc = self.ast.exprs.get(set.ule).loc();
        let is_ptr = matches!(self.ast.types.canonical_view(p), TypeView::Pointer(_));
        // clang's trial flags; `noexcept` is not compared (clang's type
        // match ignores a non-deduced mismatch).
        let mut flags = TdfFlags::ALLOW_COMPATIBLE_FUNCTION_TYPE;
        if by_ref {
            flags = flags.union(TdfFlags::PARAM_WITH_REFERENCE_TYPE);
        }
        if set.addr {
            flags = flags.union(TdfFlags::IGNORE_QUALIFIERS);
        }
        let mut found: Option<QualType> = None;
        for f in decls {
            let f = if self.kind_of(f) == DeclKind::FunctionTemplate {
                // /6.1: a function template makes the set non-deduced;
                // a template-id's names the specialization its explicit
                // arguments (and defaults) give.
                let l = explicit?;
                let Ok(spec) = self.deduce_explicit_only(f, Some(l), loc) else { continue };
                spec
            } else {
                f
            };
            let Some(mut at) = self.overload_member_type(f, set) else { continue };
            // Function-to-pointer conversion.
            if !by_ref && is_ptr && self.ast.types.is_function(at) {
                at = self.ast.types.pointer_to(at);
            }
            let at = self.ast.types.canonical(at);
            let mut trial = DeducedArgs::new(d.depth, d.args.len());
            trial.loc = d.loc;
            trial.expansions = d.expansions;
            trial.pack_elem = d.pack_elem;
            trial.pack_cur = d.pack_cur.as_ref().map(|_| Vec::new());
            if self.deduce(p, at, flags, &mut trial) != Tdk::Success {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some(at);
        }
        found
    }

    /// The type of overload set member `f` as the argument `set` names
    /// it (clang `GetTypeOfFunction`): a pointer to member function for a
    /// non-static member named `&X::f` (none for any other form), else
    /// the function type, or a pointer to it under `&`. `None` too while
    /// its return type stays undeduced.
    fn overload_member_type(&mut self, f: DeclId, set: &OverloadSetArg) -> Option<QualType> {
        let ret_of =
            |s: &Self, q: QualType| s.ast.types.proto(q).map(kiln_ast::types::FuncProto::ret);
        let mut ft = self.ast.decls.get(f).ty()?;
        if ret_of(self, ft).is_some_and(|r| self.contains_undeduced_auto(r)) {
            let loc = self.ast.exprs.get(set.ule).loc();
            self.instantiate_function_definition(f, loc);
            ft = self.ast.decls.get(f).ty()?;
            if ret_of(self, ft).is_some_and(|r| self.contains_undeduced_auto(r)) {
                return None;
            }
        }
        if self.is_implicit_object_member(f) {
            if !set.member_form {
                return None;
            }
            let class = self.declaring_class(f)?;
            let ct = self.tag_type(class).ty();
            return Some(self.ast.types.member_pointer(ft, ct));
        }
        Some(if set.addr { self.ast.types.pointer_to(ft) } else { ft })
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
                let with = f.args.map(|l| format!(" {}", self.deduced_args_desc(ft, l)));
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
                let with = f.args.map(|l| format!(" {}", self.deduced_args_desc(ft, l)));
                // clang names a failed `enable_if` instead of quoting the
                // error (spec/sema-templates.md §5.7.1): at the condition
                // when only the `enable_if` is known, at the template when
                // the failed requirement is.
                match f.diag.as_ref().and_then(|x| x.id().map(|id| (id, x))) {
                    Some((DiagId::sema_typename_nested_not_found_enable_if, x)) => {
                        return self
                            .diag(DiagId::note_ovl_candidate_disabled_by_enable_if, x.loc())
                            .arg("'enable_if'")
                            .arg(with.unwrap_or_default());
                    }
                    Some((DiagId::sema_typename_nested_not_found_requirement, x)) => {
                        return self
                            .diag(DiagId::note_ovl_candidate_disabled_by_requirement, loc)
                            .arg(x.arg_at(0).unwrap_or_default())
                            .arg(with.unwrap_or_default());
                    }
                    _ => {}
                }
                let level = self.diags.clang_compat();
                let msg = f.diag.as_ref().map(|x| format!(": {}", x.message_at(level)));
                self.diag(DiagId::note_ovl_candidate_substitution_failure, loc)
                    .arg(with.unwrap_or_default())
                    .arg(msg.unwrap_or_default())
            }
            Tdk::DeducedMismatch => {
                let with = f.args.map(|l| format!(" {}", self.deduced_args_desc(ft, l)));
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

/// Whether a call argument names an overload set ([temp.deduct.call]/6).
enum OverloadArg {
    /// Not an overload set.
    NotASet,
    /// An overload set.
    Set(OverloadSetArg),
}

/// A call argument naming an overload set (clang's
/// `OverloadExpr::FindResult`).
struct OverloadSetArg {
    /// The `UnresolvedLookupExpr`.
    ule: ExprId,
    /// It is the operand of `&`.
    addr: bool,
    /// It is `&X::f` (unparenthesized and qualified): the form of a
    /// pointer to member.
    member_form: bool,
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
