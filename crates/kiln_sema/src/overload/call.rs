//! Calls of overloaded names ([over.match.call]; spec/sema-overload.md
//! §2.1–§2.2): an `UnresolvedLookupExpr` callee is resolved against the
//! call's arguments, and the winner becomes a `DeclRefExpr` that the
//! direct call path then converts the arguments for.

use std::ops::ControlFlow;

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::names::DeclName;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::{
    ArgInfo, CandidateKind, Ics, IcsOptions, OverloadCandidateSet, OvlOutcome, SetKind, UleFound,
};
use crate::Sema;
use crate::init::InitList;

impl Sema<'_, '_> {
    /// An `UnresolvedLookupExpr` for the overload set `decls` (found as
    /// `found`, parallel) named `name`: `adl` for an unqualified callee
    /// whose lookup does not suppress argument-dependent lookup.
    #[allow(clippy::too_many_arguments, reason = "the lookup result's parts")]
    pub(crate) fn build_overload_ref(
        &mut self,
        name: DeclName,
        decls: &[DeclId],
        found: &[DeclId],
        naming: Option<DeclId>,
        adl: bool,
        nns: Option<kiln_ast::NnsId>,
        loc: Loc,
    ) -> ExprId {
        // [temp.dep.expr]/3: a set with a function of dependent type (a
        // static member `key(const node<K>*)` of a class template beside
        // `key(const nb*)`, libstdc++ `_Rb_tree::_S_key`) is type-dependent,
        // and the call is resolved at instantiation (clang's
        // `OverloadExpr` of decls in a dependent context; WP-366). So is
        // a set with a member of a dependent class, templates included
        // (clang's `computeDependence(OverloadExpr)`: a decl whose context
        // is dependent): the SFINAE-selected static member templates
        // `to_char_type<C = CharType>(std::uint8_t)` of a class template
        // are resolved per instantiation (WP-668).
        let dependent = self.overload_set_is_dependent(decls);
        let e = self.builder().unresolved_lookup_in(name, decls, adl, nns, None, dependent, loc);
        if naming.is_some() || found.iter().zip(decls).any(|(f, d)| f != d) {
            self.ovl.ule_found.insert(e, UleFound { found: found.to_vec(), naming });
        }
        e
    }

    /// Whether an overload set naming `decls` is type-dependent
    /// ([`Self::build_overload_ref`]; clang's
    /// `computeDependence(OverloadExpr)`), with or without explicit
    /// template arguments: `F::ov<long>` naming the static member
    /// templates of class template `F` is resolved per instantiation
    /// (WP-833).
    pub(crate) fn overload_set_is_dependent(&self, decls: &[DeclId]) -> bool {
        decls.iter().any(|&d| {
            let n = self.ast.decls.get(d);
            (self.kind_of(d) != DeclKind::FunctionTemplate
                && n.ty().is_some_and(|t| self.ast.types.is_dependent(t)))
                || n.ctx().is_some_and(|c| self.ast.decls.get(c).has(DeclFlags::DEPENDENT_CTX))
        })
    }

    /// Resolves the non-dependent call of the overloaded name `callee` (an
    /// `UnresolvedLookupExpr`, possibly parenthesized) with `args`
    /// (`braced` the braced-init-list ones): the `DeclRefExpr` naming the
    /// best viable function, or `None` after diagnosing.
    /// "use of undeclared identifier 'f'" for the callee `name` at `loc`
    /// that no lookup found; clang 22 adds the range of its (first) token.
    pub(crate) fn diagnose_undeclared_call_name(&mut self, name: DeclName, loc: Loc) {
        let mut msg = self.diag(Self::undeclared_use_id(name), loc).arg(self.decl_name_str(name));
        if self.clang_compat >= 22 {
            let end = self.token_end(loc).unwrap_or(loc);
            msg = msg.range(kiln_source::Span::new(loc, end));
        }
        self.emit(msg);
    }

    #[allow(
        clippy::too_many_lines,
        reason = "lookup, ADL, the one-candidate path and the candidate set meet here"
    )]
    pub(crate) fn resolve_unresolved_call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        braced: &[Option<InitList>],
    ) -> Option<ExprId> {
        let ule = self.ignore_parens(callee);
        let n = *self.ast.exprs.get(ule);
        let name = self.ast.unresolved_name(ule);
        let decls = self.ast.unresolved_decls(ule);
        // C has no argument-dependent lookup.
        let adl = n.aux() != 0 && !self.is_c();
        let extra = self.ovl.ule_found.remove(&ule);
        let naming = extra.as_ref().and_then(|x| x.naming);
        let mut cands: Vec<(DeclId, DeclId)> = decls
            .iter()
            .enumerate()
            .map(|(i, &d)| (d, extra.as_ref().and_then(|x| x.found.get(i).copied()).unwrap_or(d)))
            .collect();
        if adl {
            let at = self.begin_loc(callee);
            self.adl_lookup_dn(name, args, at, &mut cands);
        }
        let last = cands.last().copied();
        cands.retain(|&(d, _)| !self.ast.decls.get(d).has(DeclFlags::INVALID));
        // Only invalid functions (already diagnosed) were found: clang's
        // `resolveKind` keeps the last one, and the call is resolved
        // against it (a redeclaration of an invalid function is invalid,
        // WP-288).
        let only_invalid = cands.is_empty() && last.is_some();
        cands.extend(last.filter(|_| only_invalid));
        let at = self.begin_loc(callee);
        if cands.is_empty()
            && adl
            && let Some(f) =
                self.module_hidden_lookup(name, None, crate::lookup::LookupKind::Ordinary)
        {
            // A reachable but not visible function: clang's missing-import
            // error, and the call goes on with it ([module.reach]).
            self.module_diagnose_hidden(&f, name, n.loc());
            cands.extend(f.decls.iter().copied().zip(f.found.iter().copied()));
        }
        // A dependent call whose name a dependent base of an enclosing
        // class declares, which the definition's unqualified lookup did not
        // search ([temp.dep.general]/3): clang's `DiagnoseEmptyLookup`
        // "explicit qualification required to use member 'z' from
        // dependent base class" (spec/sema-templates.md §3.1.1, WP-1084).
        if cands.is_empty()
            && adl
            && !self.inst.stack.is_empty()
            && self.diagnose_member_found_in_class(name, n.loc(), false).is_some()
        {
            return None;
        }
        if cands.is_empty() {
            self.diagnose_undeclared_call_name(name, n.loc());
            return None;
        }
        // `f<args>(…)`: only the templates are candidates
        // ([over.match.call.general]/1 via [temp.arg.explicit]).
        let explicit = self.ast.templates.expr_targs.get(ule).copied();
        if explicit.is_some() {
            cands.retain(|&(d, _)| self.kind_of(d) == DeclKind::FunctionTemplate);
            if cands.is_empty() {
                self.diagnose_args_for_non_template(name, &decls, n.loc());
                return None;
            }
        }
        let templates = cands.iter().any(|&(d, _)| self.kind_of(d) == DeclKind::FunctionTemplate);
        let plain =
            cands.iter().filter(|&&(d, _)| self.kind_of(d) != DeclKind::FunctionTemplate).count();
        let nns = self.ast.templates.expr_nns.get(ule).copied();
        // A constrained function found by argument-dependent lookup goes
        // through overload resolution, as clang's `BuildOverloadedCallExpr`
        // for an ADL lookup: unsatisfied, it is a non-viable candidate
        // ([over.match.viable]/3), not an invalid reference.
        let adl_constrained = adl && self.ast.templates.trailing_requires.get(cands[0].0).is_some();
        if plain == 1 && !templates && !adl_constrained && !only_invalid {
            // One candidate: the direct path's conversions and
            // diagnostics are clang's for a one-candidate set.
            let (d, f) = cands[0];
            let e = self.finish_overload_ref(d, f, naming, n.loc(), nns, ule, true);
            if !adl
                && decls.len() == 1
                && self.ast.decls.get(d).has(DeclFlags::INVALID)
                && !self.invalid_by_param(d)
            {
                // A name clang resolves without an overload set (a
                // `DeclRefExpr` built directly) whose function became
                // invalid when its definition was instantiated by that
                // reference: the call is erroneous, undiagnosed.
                return Some(self.recover_err(&[e], n.loc()));
            }
            self.ovl.adl_single = adl.then_some(e);
            return Some(e);
        }
        if let ControlFlow::Break(r) = self.implicit_object_call(ule, &cands, naming, args, braced)
        {
            return r;
        }
        let mut set = self.begin_set(SetKind::Normal, at);
        self.defer_templates(&mut set);
        let base = self.push_call_args(&mut set, args, braced);
        for (i, &(d, f)) in cands.iter().enumerate() {
            if self.kind_of(d) == DeclKind::FunctionTemplate {
                if self.ast.decls.cold.requires_clause.contains(d) {
                    set.uncertain = true;
                }
                if set.two_phase && explicit.is_none() {
                    set.deferred.push(super::defer::Deferred::Template {
                        ft: d,
                        found: f,
                        kind: CandidateKind::Function,
                        first_opts: IcsOptions::NONE,
                        loc: at,
                        poi: Some(n.loc()),
                        adl_only: i >= decls.len(),
                    });
                    continue;
                }
                // clang's `TemplateDeductionInfo` takes the candidate set's
                // location, the callee's begin with its nested-name-specifier
                // (`Fn->getExprLoc()`); the specialization is referenced at
                // the name.
                self.templ.deduce_poi = Some(n.loc());
                self.add_template_candidate(
                    &mut set,
                    d,
                    f,
                    explicit,
                    CandidateKind::Function,
                    IcsOptions::NONE,
                    at,
                );
                self.templ.deduce_poi = None;
            } else {
                if self.ast.decls.cold.requires_clause.contains(d) {
                    set.uncertain = true;
                }
                self.add_candidate(&mut set, d, f, CandidateKind::Function, IcsOptions::NONE);
            }
            if i >= decls.len()
                && let Some(c) = set.cands.last_mut()
            {
                c.adl_only = true;
            }
        }
        let out = self.resolve_set(&mut set);
        self.ovl.lists.truncate(base);
        let r = self.finish_overloaded_call(&set, out, name, at, n.loc(), naming, nns, ule);
        self.end_set(set);
        if explicit.is_some() && decls.is_empty() && self.ovl.binding_get != Some(ule) {
            self.diagnose_adl_only_template_id(n.loc());
        }
        r
    }

    /// The call of the overloaded name `ule` (its candidates `cands`,
    /// `naming` the recorded naming class) with `args` when its object
    /// argument is the implicit `*this`, or when every candidate needs one
    /// and there is none (`Break` with the callee, `None` after
    /// diagnosing); `Continue` for a call without an object argument.
    fn implicit_object_call(
        &mut self,
        ule: ExprId,
        cands: &[(DeclId, DeclId)],
        naming: Option<DeclId>,
        args: &[ExprId],
        braced: &[Option<InitList>],
    ) -> ControlFlow<Option<ExprId>> {
        let (name, loc) = (self.ast.unresolved_name(ule), self.ast.exprs.get(ule).loc());
        // `f<args>(…)` / `C::f<args>(…)` naming member function templates
        // where `this` may be used: the object argument is the implicit
        // `*this` ([class.mfct.non.static]/2, [over.match.funcs]/4), as
        // for the non-template name (`implicit_member_overload`), so the
        // implicit object parameters rank the candidates
        // (spec/sema-overload.md §2.3).
        let explicit = self.ast.templates.expr_targs.get(ule).copied();
        let nns = self.ast.templates.expr_nns.get(ule).copied();
        if explicit.is_some()
            && let Some(e) =
                self.implicit_member_template_id(name, cands, naming, nns, explicit, loc)
        {
            if self.member_overload(e).is_none() {
                // `this` is type-dependent: resolved at instantiation.
                return ControlFlow::Break(Some(e));
            }
            return ControlFlow::Break(self.resolve_member_call(e, args, braced));
        }
        if self.lacks_implicit_object(cands) {
            // [expr.prim.id.general]/3, [over.match.call.general]/2.2:
            // every candidate needs an object and no `this` may be used
            // (a static member function, an unrelated class) — clang's
            // `IMA_Error_StaticContext` / `IMA_Error_Unrelated`, diagnosed
            // before overload resolution, also in an unevaluated operand.
            self.error(DiagId::sema_member_call_without_object, loc);
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// A call `f<args>(…)` whose `f` ordinary lookup did not find (only
    /// argument-dependent lookup found its templates, [temp.names]/3) is
    /// a C++20 feature: before C++20 clang's `ext_adl_only_template_id`
    /// at the name, after the call's own diagnostics (`ActOnCallExpr`);
    /// from C++20 its `-Wpre-c++20-compat` counterpart
    /// (`warn_cxx17_compat_adl_only_template_id`, off by default).
    pub(crate) fn diagnose_adl_only_template_id(&mut self, loc: Loc) {
        let id = if self.std < kiln_lex::Std::Cxx20 {
            DiagId::sema_ext_adl_only_template_id
        } else {
            DiagId::sema_cxx17_compat_adl_only_template_id
        };
        let w = self.diag(id, loc).per_instantiation();
        self.emit(w);
    }

    /// Adds function template `ft` (found as `found`) to `set`: its
    /// specialization deduced from the set's arguments (§6.1,
    /// spec/sema-overload.md §2.3), else a non-viable candidate that
    /// keeps the deduction failure for its note (§10.4).
    #[allow(clippy::too_many_arguments, reason = "add_candidate's parts and the template's")]
    pub(crate) fn add_template_candidate(
        &mut self,
        set: &mut OverloadCandidateSet,
        ft: DeclId,
        found: DeclId,
        explicit: Option<kiln_ast::ids::ArgListId>,
        kind: CandidateKind,
        first_opts: IcsOptions,
        loc: Loc,
    ) {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("add_template_candidate");
        // A non-member operator template's first parameter takes the
        // object operand ([over.match.oper]/2), as does an explicit
        // object parameter ([over.match.funcs.general]/4).
        let pattern = self.templated_function(ft).unwrap_or(ft);
        let oper_fn = set.oper && kind == CandidateKind::Function;
        let lead = (oper_fn || self.has_explicit_object(pattern)) && set.object.is_some();
        // The deduction's failure before it substitutes anything (an
        // invalid template, too few arguments): the same failure, without
        // the argument lists (WP-657).
        #[cfg(feature = "op-stats")]
        let frame = crate::templ::deduce_stats::cand_begin();
        if let Some(tdk) = self.deduce_call_prefail(ft, set.args.len() + usize::from(lead)) {
            #[cfg(feature = "op-stats")]
            crate::templ::deduce_stats::prefiltered(tdk);
            match tdk {
                // No candidate (as `add_template_candidate_with`).
                crate::templ::deduce::Tdk::Invalid => {}
                // The failure is formed only if the arity check of
                // `add_candidate` leaves the candidate viable.
                crate::templ::deduce::Tdk::TooFewArguments => self.add_arity_failed_candidate(
                    set,
                    ft,
                    found,
                    kind,
                    first_opts,
                    pattern,
                    |s| s.deduce_head_failure(s.ast.first(ft), tdk, loc),
                ),
                _ => {
                    let fail = self.deduce_head_failure(self.ast.first(ft), tdk, loc);
                    self.add_template_candidate_with(
                        set,
                        ft,
                        found,
                        kind,
                        first_opts,
                        pattern,
                        lead,
                        Err(fail),
                    );
                }
            }
            #[cfg(feature = "op-stats")]
            crate::templ::deduce_stats::cand_end(&frame, tdk);
            return;
        }
        // Each argument's conversion options, as `add_candidate` gives
        // them, for the non-dependent parameters deduction checks. The
        // buffers are reused across candidates (a deduction may resolve
        // calls of its own, so one pair per nesting level).
        let (mut args, mut opts) = self.cand_scratch.pop().unwrap_or_default();
        args.clear();
        opts.clear();
        if lead && let Some(o) = set.object {
            args.push(o);
            opts.push(if oper_fn {
                first_opts.with(IcsOptions::CANDIDATE_ARG)
            } else {
                IcsOptions::NONE
            });
        }
        args.extend_from_slice(&set.args);
        for i in 0..set.args.len() {
            let o = if i == 0 && !oper_fn { first_opts } else { IcsOptions::NONE };
            opts.push(o.with(IcsOptions::CANDIDATE_ARG));
        }
        let mark = self.ovl.nondep_ics.len();
        let r = self.deduce_call(ft, explicit, &args, &opts, loc);
        self.cand_scratch.push((args, opts));
        #[cfg(feature = "op-stats")]
        let tdk = r.as_ref().map_or_else(|f| f.tdk, |_| crate::templ::deduce::Tdk::Success);
        self.add_template_candidate_with(set, ft, found, kind, first_opts, pattern, lead, r);
        self.ovl.nondep_ics.truncate(mark);
        #[cfg(feature = "op-stats")]
        crate::templ::deduce_stats::cand_end(&frame, tdk);
    }

    /// [`Self::add_template_candidate`] with the deduction's result `r`.
    #[allow(clippy::too_many_arguments, reason = "add_template_candidate's state")]
    fn add_template_candidate_with(
        &mut self,
        set: &mut OverloadCandidateSet,
        ft: DeclId,
        found: DeclId,
        kind: CandidateKind,
        first_opts: IcsOptions,
        pattern: DeclId,
        lead: bool,
        r: Result<DeclId, Box<crate::templ::deduce_call::DeductionFailure>>,
    ) {
        match r {
            Ok(spec) => {
                let nondep = self.templ.explicit_nondep;
                self.add_candidate(set, spec, found, kind, first_opts);
                if let Some(c) = set.cands.last_mut() {
                    c.template = Some(ft);
                    c.explicit_nondep = nondep;
                }
            }
            Err(fail) => {
                use crate::templ::deduce::Tdk;
                if fail.tdk == Tdk::Invalid {
                    // An invalid template is no candidate at all: the
                    // call finds no match and the template is not noted
                    // (clang `TemplateDeductionResult::Invalid`).
                    return;
                }
                if let Some((i, x)) = fail.bad {
                    // A non-dependent parameter's bad conversion, found
                    // before the specialization was formed: reported as
                    // the template's bad conversion (clang
                    // `ovl_fail_bad_conversion`), in `add_candidate`'s
                    // sequence slots.
                    self.ovl.stats.candidates += 1;
                    let mut c = super::Candidate::new(pattern, found, kind);
                    let (slot, arg) = if lead {
                        (
                            i,
                            i.checked_sub(1)
                                .map_or(u16::MAX, |a| u16::try_from(a).unwrap_or(u16::MAX)),
                        )
                    } else {
                        (
                            i + usize::from(set.object.is_some()),
                            u16::try_from(i).unwrap_or(u16::MAX),
                        )
                    };
                    let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
                    set.ics.extend(std::iter::repeat_n(Ics::Ellipsis, slot));
                    set.ics.push(x);
                    c.ics = start..u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
                    c.template = Some(ft);
                    c.viable = false;
                    c.failure = super::CandidateFailure::BadConversion { arg };
                    set.cands.push(c);
                    return;
                }
                if matches!(fail.tdk, Tdk::TooManyArguments | Tdk::TooFewArguments) {
                    self.add_arity_failed_candidate(
                        set,
                        ft,
                        found,
                        kind,
                        first_opts,
                        pattern,
                        |_| fail,
                    );
                    return;
                }
                self.ovl.stats.candidates += 1;
                let mut c = super::Candidate::new(pattern, found, kind);
                let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
                c.ics = start..start;
                c.template = Some(ft);
                c.viable = false;
                c.failure = super::CandidateFailure::BadDeduction;
                c.deduction = Some(fail);
                set.cands.push(c);
            }
        }
    }

    /// The candidate of a template whose deduction failed on the call's
    /// arity: the pattern as a candidate, so that its own arity check
    /// gives the note; the deduction's failure (`fail`) only when that
    /// check passes.
    #[allow(clippy::too_many_arguments, reason = "add_template_candidate's state")]
    fn add_arity_failed_candidate(
        &mut self,
        set: &mut OverloadCandidateSet,
        ft: DeclId,
        found: DeclId,
        kind: CandidateKind,
        first_opts: IcsOptions,
        pattern: DeclId,
        fail: impl FnOnce(&Self) -> Box<crate::templ::deduce_call::DeductionFailure>,
    ) {
        self.add_candidate(set, pattern, found, kind, first_opts);
        if let Some(c) = set.cands.last_mut() {
            c.template = Some(ft);
            if c.viable {
                c.viable = false;
                c.failure = super::CandidateFailure::BadDeduction;
                c.deduction = Some(fail(self));
            }
        }
    }

    /// Adds the call arguments `args` (`braced` the braced-init-list ones)
    /// to `set`; returns the mark to truncate `ovl.lists` back to.
    pub(crate) fn push_call_args(
        &mut self,
        set: &mut OverloadCandidateSet,
        args: &[ExprId],
        braced: &[Option<InitList>],
    ) -> usize {
        let base = self.ovl.lists.len();
        for (i, &a) in args.iter().enumerate() {
            match braced.get(i).and_then(Option::as_ref) {
                Some(l) => {
                    let idx = u32::try_from(self.ovl.lists.len()).unwrap_or(u32::MAX);
                    self.ovl.lists.push(l.clone());
                    set.args.push(ArgInfo {
                        list: Some(idx),
                        ..ArgInfo::of_type(QualType::VOID, ValueCategory::PRValue)
                    });
                }
                None => set.args.push(self.arg_info(a)),
            }
        }
        base
    }

    #[allow(clippy::too_many_arguments, reason = "the call's parts")]
    fn finish_overloaded_call(
        &mut self,
        set: &OverloadCandidateSet,
        out: OvlOutcome,
        name: DeclName,
        at: Loc,
        loc: Loc,
        naming: Option<DeclId>,
        nns: Option<kiln_ast::NnsId>,
        ule: ExprId,
    ) -> Option<ExprId> {
        let b = self.call_winner(set, out, name, at, loc, false)?;
        let c = set.cand(b);
        let (d, f) = (c.func(), c.found);
        Some(self.finish_overload_ref(d, f, naming, loc, nns, ule, false))
    }

    /// The best viable candidate of the call set `set` of the function
    /// named `name` (`member`: a member function call), or `None` after
    /// diagnosing the failure at `at` (§9.1; `loc` the name, for the
    /// unimplemented-feature reports).
    pub(crate) fn call_winner(
        &mut self,
        set: &OverloadCandidateSet,
        out: OvlOutcome,
        name: DeclName,
        at: Loc,
        loc: Loc,
        member: bool,
    ) -> Option<u32> {
        let text = self.decl_name_str(name);
        let undecided = set.uncertain
            || (set.skipped_templates
                && !matches!(out, OvlOutcome::Success(b) | OvlOutcome::Deleted(b)
                    if self.template_cannot_win(set, b)));
        if undecided {
            if set.skipped_templates {
                self.templ_unimplemented("calls to function templates", loc);
            } else {
                self.unsupported("overload resolution with constraints", loc);
            }
            return None;
        }
        if !matches!(out, OvlOutcome::Success(_)) {
            let ty = self.recovery_type(set, out);
            self.note_call_recovery(ty);
        }
        let (id, ambiguous) = match out {
            OvlOutcome::Success(b) => return Some(b),
            OvlOutcome::Deleted(b) => {
                let d = set.cand(b).func();
                // clang names the lookup name (`ULE->getName()`), not the
                // selected specialization: 'show', not 'show<int>'.
                let (id, name) = if member {
                    (DiagId::sema_ovl_deleted_member_call, text)
                } else {
                    (DiagId::sema_call_deleted_function, format!("'{text}'"))
                };
                let msg = self.diag(id, at).arg(name).arg(self.delete_message_suffix(d));
                let msg = self.with_ovl_notes(msg, set, false);
                self.emit(msg);
                return None;
            }
            OvlOutcome::NoViable if member => {
                (DiagId::sema_ovl_no_viable_member_function_in_call, false)
            }
            OvlOutcome::NoViable => (DiagId::sema_ovl_no_viable_function_in_call, false),
            OvlOutcome::Ambiguous if member => (DiagId::sema_ovl_ambiguous_member_call, true),
            OvlOutcome::Ambiguous => (DiagId::sema_ovl_ambiguous_call, true),
        };
        // clang `<< Fn->getSourceRange()`: the callee's name.
        let range = kiln_source::Span::new(at, self.token_end(loc).unwrap_or(loc));
        let msg = self.diag(id, at).arg(text).range(range);
        let msg = self.with_ovl_notes(msg, set, ambiguous);
        self.emit(msg);
        None
    }

    /// The type of clang's `RecoveryExpr` for the failed call of `set`
    /// (`chooseRecoveryType`): the return type of the best candidate,
    /// else the one return type of the viable candidates, else of all
    /// candidates; `None` when they differ (a type-dependent recovery).
    fn recovery_type(&self, set: &OverloadCandidateSet, out: OvlOutcome) -> Option<QualType> {
        let ret = |c: &super::Candidate| -> Option<QualType> {
            let d = c.decl?;
            if self.ast.decls.get(d).has(DeclFlags::INVALID) {
                return None;
            }
            let t = self.ast.decls.get(d).ty()?;
            self.ast
                .types
                .proto_of(self.ast.types.canonical(t).ty())
                .map(kiln_ast::types::FuncProto::ret)
        };
        // `None`: no candidate seen; `Some(None)`: they differ.
        let pick = |cands: &mut dyn Iterator<Item = &super::Candidate>| {
            let mut r: Option<Option<QualType>> = None;
            for t in cands.filter_map(&ret) {
                let t = self.ast.types.canonical(t);
                r = match r {
                    None => Some(Some(t)),
                    Some(Some(u)) if u == t => Some(Some(t)),
                    Some(_) => Some(None),
                };
            }
            r
        };
        let best = match out {
            OvlOutcome::Deleted(b) => ret(set.cand(b)).map(|t| Some(self.ast.types.canonical(t))),
            _ => None,
        };
        let r = best
            .or_else(|| pick(&mut set.cands.iter().filter(|c| c.viable)))
            .or_else(|| pick(&mut set.cands.iter()))
            .flatten()?;
        (!self.is_dependent_type(r) && self.contained_placeholder(r).is_none()).then_some(r)
    }

    /// The `DeclRefExpr` naming the selected function `d` (found as
    /// `found`) at `loc`, with the access check of a class member found by
    /// lookup in `naming`. `single`: the only candidate, whose use as a
    /// deleted function the call diagnoses.
    #[allow(clippy::too_many_arguments, reason = "the reference's parts")]
    pub(crate) fn finish_overload_ref(
        &mut self,
        d: DeclId,
        found: DeclId,
        naming: Option<DeclId>,
        loc: Loc,
        nns: Option<kiln_ast::NnsId>,
        ule: ExprId,
        single: bool,
    ) -> ExprId {
        let instance = self.is_class_member_ref(d);
        if let Some(naming) = naming {
            // [class.access.base]/5: the object is `*this` for an
            // implicit member access.
            let object = if instance { self.implicit_object_class(d) } else { None };
            self.check_member_access(found, naming, object, loc);
        }
        let shadow = (found != d).then_some(found);
        if instance {
            // A non-static member without an object: `&C::f`, or "call to
            // non-static member function without an object argument".
            let e = self.implicit_member_ref(d, loc, shadow, nns);
            // The explicit template arguments written on the name
            // (`pick<X>()` through an implicit `this`) stay on the access,
            // so an instantiation names the specialization again with
            // them ([temp.arg.explicit]/1).
            if let Some(targs) = self.ast.templates.expr_targs.get(ule).copied()
                && self.ast.exprs.get(e).kind() == ExprKind::MemberExpr
            {
                let _ = self.ast.exprs.set_member_template_args(e, targs);
            }
            return e;
        }
        // The only candidate's constraints first ([expr.prim.id.general]/4,
        // clang `DiagnoseUseOfDecl` before `BuildDeclRefExpr`): a function
        // whose constraints are not satisfied is never needed, so its
        // deferred *noexcept-specifier* is not instantiated
        // ([temp.inst]/14).
        if single && self.diagnose_unsatisfied_use(d, loc) {
            return self.recover_err(&[], loc);
        }
        // clang `FixOverloadedFunctionReference`: the explicit template
        // arguments written on the name (`ok<int>(0)`) stay on the
        // reference, so a printed requirement shows them.
        let targs = self.ast.templates.expr_targs.get(ule).copied();
        let e = if targs.is_some() && self.kind_of(d).is_function() {
            let extra = kiln_ast::expr::DeclRefExtra {
                qualifier: nns,
                template_args: targs,
                found_decl: shadow,
            };
            self.build_decl_ref_extra(d, loc, Some(extra))
        } else {
            self.build_decl_ref_qualified(d, loc, shadow, nns)
        };
        if let Some(&b) = self.ast.templates.expr_begin.get(ule)
            && b != loc
            && self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr
        {
            let _ = self.ast.templates.expr_begin.insert(e, b);
        }
        if single {
            self.note_function_name_use(d, e, loc, nns.is_none());
            if nns.is_none() {
                self.act_on_deleted_callee(e, false, false);
            }
        }
        e
    }
}
