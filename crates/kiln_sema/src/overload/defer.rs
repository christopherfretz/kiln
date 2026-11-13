//! clang 21's deferred template candidates and perfect-match resolution
//! (llvm-project `OverloadCandidateSet::BestViableFunction`'s
//! `TwoPhaseResolution`, `PerfectViableFunction`,
//! `InjectNonDeducedTemplateCandidates`; spec/sema-overload.md §4.4).
//!
//! In a set that resolves in two phases (`OverloadCandidateSet::two_phase`,
//! at `clang_compat >= 22`), a function template named without explicit
//! template arguments is not deduced when it is added: it is recorded as
//! a [`Deferred`] candidate. Resolution first looks for a perfect match
//! among the candidates added so far (non-templates, built-in operators,
//! templates with explicit arguments): when there is exactly one, or one
//! more constrained than every other, it is the result and no deferred
//! template is ever deduced, so nothing its signature names is
//! instantiated. Otherwise the deferred templates are deduced, appended
//! in the order they were added, and the set is resolved as usual.
//!
//! clang defers in every candidate set except initialization by
//! user-defined conversion ([over.match.copy], [over.match.conv],
//! [over.match.ref]) and code completion: calls, member calls, operators,
//! initialization by constructor, class template argument deduction,
//! range-based `for`'s `begin`/`end`, special member lookup.

use kiln_ast::decl::DeclKind;
use kiln_ast::names::OverloadedOperator as OO;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use super::{CandidateKind, Ics, IcsOptions, OverloadCandidateSet, OvlOutcome, Rewrite};
use crate::Sema;
use crate::conv::{RefBinding, StandardConversion};

/// A function template candidate whose deduction is deferred: what the
/// adding context needs to add it later, exactly as it would have.
#[derive(Clone, Debug, kiln_snap::Snap)]
pub(crate) enum Deferred {
    /// `add_template_candidate(ft, found, None, kind, first_opts, loc)`
    /// with the deduction's point of instantiation `poi`; `adl_only`
    /// marks the candidate found only by argument-dependent lookup.
    Template {
        ft: DeclId,
        found: DeclId,
        kind: CandidateKind,
        first_opts: IcsOptions,
        loc: Loc,
        poi: Option<Loc>,
        adl_only: bool,
    },
    /// A non-member operator function template and its rewritten and
    /// reversed forms ([over.match.oper]/3.2, /3.4).
    NonmemberOper {
        d: DeclId,
        found: DeclId,
        op: OO,
        extra: Option<OO>,
        ops: Vec<ExprId>,
        types: Vec<QualType>,
        adl_only: bool,
    },
    /// A member operator function template in the form `rewrite`
    /// ([over.match.oper]/3.1, /3.4).
    MemberOper { d: DeclId, found: DeclId, rewrite: Rewrite, ops: Vec<ExprId>, loc: Loc },
    /// A constructor or member function template found in a class
    /// (`add_found_candidates`), `found` its declaration as found (an
    /// inherited constructor's shadow).
    Found { ft: DeclId, found: DeclId, kind: CandidateKind, first_opts: IcsOptions },
    /// A deduction guide template ([over.match.class.deduct]).
    Guide { g: DeclId, how: crate::templ::ctad::CtadInit, loc: Loc },
}

impl Sema<'_, '_> {
    /// Makes `set` resolve in two phases when the compatibility level has
    /// clang 21's deferred template candidates.
    pub(crate) fn defer_templates(&self, set: &mut OverloadCandidateSet) {
        set.two_phase = self.clang_compat >= 22;
    }

    /// Resolves `set` (clang `BestViableFunction`): a perfect match among
    /// the candidates added so far when templates were deferred, else
    /// every candidate after deducing the deferred templates.
    pub(crate) fn resolve_set(&mut self, set: &mut OverloadCandidateSet) -> OvlOutcome {
        if !set.deferred.is_empty()
            && !set.no_perfect
            && let Some(b) = self.perfect_viable(set)
        {
            let c = set.cand(b);
            if c.decl.is_some() && c.kind != CandidateKind::Surrogate && self.ovl_deleted(c.func())
            {
                // The deleted call's notes list every candidate, the
                // deferred templates deduced for them (clang
                // `CompleteCandidates` injects them).
                self.inject_deferred(set);
                return OvlOutcome::Deleted(b);
            }
            return OvlOutcome::Success(b);
        }
        self.inject_deferred(set);
        self.order_template_candidates(set);
        self.best_viable(set)
    }

    /// Deduces the deferred template candidates of `set`, appending them
    /// in the order they were added.
    pub(crate) fn inject_deferred(&mut self, set: &mut OverloadCandidateSet) {
        if set.deferred.is_empty() {
            return;
        }
        let ds = std::mem::take(&mut set.deferred);
        let two_phase = std::mem::replace(&mut set.two_phase, false);
        for d in ds {
            match d {
                Deferred::Template { ft, found, kind, first_opts, loc, poi, adl_only } => {
                    let n = set.cands.len();
                    let saved = std::mem::replace(&mut self.templ.deduce_poi, poi);
                    self.add_template_candidate(set, ft, found, None, kind, first_opts, loc);
                    self.templ.deduce_poi = saved;
                    if adl_only {
                        for c in &mut set.cands[n..] {
                            c.adl_only = true;
                        }
                    }
                }
                Deferred::NonmemberOper { d, found, op, extra, ops, types, adl_only } => {
                    self.add_nonmember_oper(set, d, found, op, extra, &ops, &types, adl_only);
                }
                Deferred::MemberOper { d, found, rewrite, ops, loc } => {
                    self.add_member_oper_template(set, d, found, rewrite, &ops, loc);
                }
                Deferred::Found { ft, found, kind, first_opts } => {
                    let n = set.cands.len();
                    self.add_found_template_now(set, ft, found, kind, first_opts);
                    if found != ft {
                        self.exclude_inhctor_slice(set, n, found);
                    }
                }
                Deferred::Guide { g, how, loc } => self.add_ctad_candidate(set, g, how, loc),
            }
        }
        set.two_phase = two_phase;
    }

    /// clang `PerfectViableFunction`: the one candidate of `set` that is
    /// a perfect match, or of several the one more constrained than each
    /// other ([over.match.best.general]/2.6 by clang's
    /// `getMoreConstrainedFunction`, whatever the parameter types). `None`
    /// when there is none, when two are equally constrained (one function
    /// and its reversed form among them) or one is a built-in operator,
    /// and for a perfect conversion function while constructor templates
    /// are deferred (a constructor might be preferred,
    /// [over.match.best.general]/2.2's CWG2327 tie-breaker).
    fn perfect_viable(&mut self, set: &OverloadCandidateSet) -> Option<u32> {
        let ctor_templates = set
            .deferred
            .iter()
            .any(|d| matches!(d, Deferred::Found { kind: CandidateKind::Ctor, .. }));
        let mut best: Option<u32> = None;
        for c in 0..u32::try_from(set.cands.len()).unwrap_or(u32::MAX) {
            if !self.perfect_candidate(set, c) {
                continue;
            }
            if ctor_templates && set.cand(c).kind == CandidateKind::ConvFn {
                return None;
            }
            let Some(b) = best else {
                best = Some(c);
                continue;
            };
            let (x, y) = (set.cand(b), set.cand(c));
            let (Some(f1), Some(f2)) = (x.decl, y.decl) else { return None };
            if x.kind == CandidateKind::Surrogate || y.kind == CandidateKind::Surrogate {
                return None;
            }
            let requires = |s: &Self, f: DeclId| -> Vec<ExprId> {
                s.ast.templates.trailing_requires.get(f).copied().into_iter().collect()
            };
            let (r1, r2) = (requires(self, f1), requires(self, f2));
            if self.more_constrained(f2, &r2, f1, &r1) {
                best = Some(c);
            } else if !self.more_constrained(f1, &r1, f2, &r2) {
                return None;
            }
        }
        best
    }

    /// clang `OverloadCandidate::isPerfectMatch`: candidate `c` is viable
    /// and every conversion sequence, the object argument's and a
    /// conversion function's second standard conversion included, is
    /// perfect ([`Self::perfect_conversion`]). A member function's
    /// object argument must be one: a static member function's, or that
    /// of a member function named without an object, never is.
    fn perfect_candidate(&self, set: &OverloadCandidateSet, c: u32) -> bool {
        let x = set.cand(c);
        if !x.viable {
            return false;
        }
        if x.kind == CandidateKind::Function
            && set.object.is_none()
            && x.decl.is_some_and(|f| self.kind_of(f) == DeclKind::CXXMethod)
        {
            return false;
        }
        let object = usize::from(set.object.is_some());
        let perfect_arg = |i: usize, ics: &Ics| {
            // A list-initialization sequence is never perfect (an
            // initializer-list constructor might be preferred).
            let braced = i
                .checked_sub(object)
                .and_then(|j| set.args.get(j))
                .is_some_and(|a| a.list.is_some());
            !braced && matches!(ics, Ics::Standard(s) if self.perfect_conversion(s))
        };
        set.ics_of(c).iter().enumerate().all(|(i, ics)| perfect_arg(i, ics))
            && x.final_conv.as_ref().is_none_or(|f| self.perfect_conversion(f))
    }

    /// clang `StandardConversionSequence::isPerfect`: an identity
    /// conversion (lvalue transformations aside); binding a reference,
    /// to the argument's own type, cv-qualification included, and not an
    /// lvalue reference (an implicit object parameter without a
    /// ref-qualifier included) to an rvalue.
    pub(crate) fn perfect_conversion(&self, s: &StandardConversion) -> bool {
        if s.rb.contains(RefBinding::IGNORED) || !s.is_identity() {
            return false;
        }
        if !s.rb.contains(RefBinding::IS_REF) {
            return true;
        }
        self.ast.types.same_type(s.from, s.to_types[2])
            && !(s.rb.contains(RefBinding::BINDS_RVALUE) && s.rb.contains(RefBinding::LVALUE_REF))
    }

    /// clang `hasDependentExplicit`: the *explicit-specifier* of function
    /// `f` is value-dependent.
    pub(crate) fn dependently_explicit(&self, f: DeclId) -> bool {
        self.fn_spec(f).is_explicit() && !self.nondependently_explicit(f)
    }
}
