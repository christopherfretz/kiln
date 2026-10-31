//! Viable functions and the best viable function ([over.match.viable],
//! [over.match.best]; spec/sema-overload.md §3, §4).

use kiln_ast::DeclId;

use super::{
    Candidate, CandidateFailure, CandidateKind, Ics, IcsOptions, OverloadCandidateSet, OvlOutcome,
    Rewrite, SetKind,
};
use crate::Sema;
use crate::conv::{IcsCompare, RefBinding, compare_standard_conversions};

impl Sema<'_, '_> {
    /// Whether no function template specialization could be a better
    /// candidate than the non-template candidate `b` of `set`, so that
    /// skipping the set's templates (until WP-071) cannot change the
    /// result: every conversion sequence of `b` is an identity no other
    /// sequence beats ([over.ics.rank]/3.2.1, /3.2.3-3.2.6), and a
    /// template that ties loses to it ([over.match.best.general]/2.4).
    pub(crate) fn template_cannot_win(&self, set: &OverloadCandidateSet, b: u32) -> bool {
        let c = set.cand(b);
        if c.final_conv.is_some_and(|f| !f.is_identity()) {
            return false;
        }
        let r = c.ics.start as usize..c.ics.end as usize;
        set.ics.get(r).is_some_and(|xs| xs.iter().all(|x| self.unbeatable_ics(x)))
    }

    fn unbeatable_ics(&self, x: &Ics) -> bool {
        let Ics::Standard(s) = x else { return false };
        if s.rb.contains(RefBinding::IGNORED) {
            return true;
        }
        if !s.is_identity() {
            return false;
        }
        if !s.rb.contains(RefBinding::IS_REF) {
            return true;
        }
        let cv = |q| self.ast.types.effective_quals(q) & kiln_ast::types::Quals::CV;
        if cv(s.to_types[2]) != cv(s.from) {
            return false;
        }
        let lref = s.rb.contains(RefBinding::LVALUE_REF);
        if s.rb.contains(RefBinding::OBJECT_NO_REF_QUAL) {
            true
        } else if s.rb.contains(RefBinding::BINDS_FUNCTION_LVALUE) {
            lref
        } else {
            lref != s.rb.contains(RefBinding::BINDS_RVALUE)
        }
    }

    /// Adds function `f` (found as `found`) to `set` as a candidate of
    /// kind `kind` for the set's arguments (and object argument): checks
    /// arity ([over.match.viable]/2), then computes one conversion
    /// sequence per argument, stopping at the first bad one (/4).
    /// `first_opts` restricts the first argument's sequence
    /// ([over.best.ics]/4).
    pub(crate) fn add_candidate(
        &mut self,
        set: &mut OverloadCandidateSet,
        f: DeclId,
        found: DeclId,
        kind: CandidateKind,
        first_opts: IcsOptions,
    ) {
        self.ovl.stats.candidates += 1;
        let mut cand = Candidate::new(f, found, kind);
        let mut params = std::mem::take(&mut set.params);
        let variadic = self.fn_param_types(f, &mut params);
        // The object argument initializes the first parameter: an explicit
        // object parameter, or a non-member operator function's first
        // ([over.match.oper]/2).
        let oper_fn = set.oper && kind == CandidateKind::Function;
        let explicit_obj = set.object.is_some() && (oper_fn || self.has_explicit_object(f));
        let skip = usize::from(explicit_obj);
        let nparams = params.len().saturating_sub(skip);
        let nargs = set.args.len();
        let min = self.fn_min_args(f).saturating_sub(skip);
        let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
        cand.ics = start..start;
        if nargs > nparams && !variadic {
            cand.viable = false;
            cand.failure =
                CandidateFailure::TooManyArgs { max: u16::try_from(nparams).unwrap_or(u16::MAX) };
        } else if nargs < min {
            cand.viable = false;
            cand.failure =
                CandidateFailure::TooFewArgs { min: u16::try_from(min).unwrap_or(u16::MAX) };
        } else if kind == CandidateKind::Ctor
            && set.kind == SetKind::CopyInit
            && self.fn_spec(f).is_explicit()
            && !first_opts.contains(IcsOptions::ALLOW_EXPLICIT)
        {
            cand.viable = false;
            cand.failure = CandidateFailure::Explicit;
        } else {
            if let Some(o) = set.object {
                let x = if explicit_obj && params.is_empty() {
                    Ics::Ellipsis
                } else if explicit_obj {
                    let opts = if oper_fn {
                        first_opts.with(IcsOptions::CANDIDATE_ARG)
                    } else {
                        IcsOptions::NONE
                    };
                    self.try_implicit_conversion(&o, params[0], opts)
                } else if kind == CandidateKind::Function || self.is_static_method(f) {
                    super::ignored_object(o.ty)
                } else {
                    self.object_ics(&o, f)
                };
                set.ics.push(x);
                if x.is_bad() {
                    cand.viable = false;
                    cand.failure = CandidateFailure::BadConversion { arg: u16::MAX };
                }
            }
            if cand.viable {
                for i in 0..nargs {
                    let a = set.args[i];
                    let x = match params.get(skip + i) {
                        Some(&p) => {
                            let opts =
                                if i == 0 && !oper_fn { first_opts } else { IcsOptions::NONE };
                            self.try_implicit_conversion(
                                &a,
                                p,
                                opts.with(IcsOptions::CANDIDATE_ARG),
                            )
                        }
                        None => Ics::Ellipsis,
                    };
                    set.ics.push(x);
                    if x.is_bad() {
                        cand.viable = false;
                        cand.failure = CandidateFailure::BadConversion {
                            arg: u16::try_from(i).unwrap_or(u16::MAX),
                        };
                        break;
                    }
                }
            }
        }
        cand.ics.end = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
        set.params = params;
        set.cands.push(cand);
    }

    /// [over.match.best]: whether viable candidate `c1` is better than
    /// `c2` (C++26 order, spec/sema-overload.md §4.1).
    pub(crate) fn better(&self, set: &OverloadCandidateSet, c1: u32, c2: u32) -> bool {
        let (a, b) = (set.ics_of(c1), set.ics_of(c2));
        let (x, y) = (set.cand(c1), set.cand(c2));
        let mut has_better = false;
        let mut has_worse = false;
        // clang's workaround for C++20 reversed `operator==`: against the
        // reversed form of a function written the same way, a better
        // conversion for one operand and a worse one for the other are
        // treated as equally good (diagnosed as an extension).
        let tolerate = x.rewrite.is_reversed() != y.rewrite.is_reversed()
            && x.decl.is_some()
            && y.decl.is_some();
        // /2.1: no worse for every argument, better for one. In a
        // conversion set, a conversion function's object argument and a
        // constructor's argument occupy the same slot.
        for (p, q) in a.iter().zip(b.iter()) {
            if p.is_ignored() || q.is_ignored() {
                continue;
            }
            match self.compare_ics(p, q) {
                IcsCompare::Better => has_better = true,
                IcsCompare::Worse if tolerate && self.allow_ambiguity(x.func(), y.func()) => {
                    has_worse = true;
                }
                IcsCompare::Worse => return false,
                IcsCompare::Indistinguishable => {}
            }
        }
        if has_better && !has_worse {
            return true;
        }
        // clang's CWG2327 workaround: in an initialization by constructor,
        // a constructor over a conversion function.
        if matches!(set.kind, SetKind::Normal | SetKind::ListCtor { .. })
            && set.args.len() == 1
            && (x.kind == CandidateKind::Ctor) != (y.kind == CandidateKind::Ctor)
            && (x.kind == CandidateKind::ConvFn || y.kind == CandidateKind::ConvFn)
        {
            return x.kind == CandidateKind::Ctor;
        }
        // /2.2: in a user-defined conversion, the better second standard
        // conversion.
        if let (Some(f1), Some(f2)) = (&x.final_conv, &y.final_conv) {
            match compare_standard_conversions(self, f1, f2) {
                IcsCompare::Better => return true,
                IcsCompare::Worse => return false,
                IcsCompare::Indistinguishable => {}
            }
        }
        // /2.4: a non-template over a template specialization.
        match (x.template.is_some(), y.template.is_some()) {
            (false, true) => return true,
            (true, false) => return false,
            _ => {}
        }
        // /2.8: a non-rewritten candidate over a rewritten one; /2.9:
        // of two rewritten candidates, the one not reversed.
        if (x.rewrite == Rewrite::None) != (y.rewrite == Rewrite::None) {
            return x.rewrite == Rewrite::None;
        }
        if x.rewrite.is_reversed() != y.rewrite.is_reversed() {
            return !x.rewrite.is_reversed();
        }
        // /2.5 (more specialized template): WP-071. /2.6 (more
        // constrained): WP-073. /2.7 (constructor of a derived class over
        // an inherited constructor): inherited constructors are not
        // candidates yet. /2.8-2.10 (rewritten candidates, deduction
        // guides): WP-068b, S3T.
        false
    }

    /// [over.match.best]/2-3: the best viable function of `set` (clang
    /// `BestViableFunction`'s two passes); marks the candidates of an
    /// ambiguity.
    pub(crate) fn best_viable(&self, set: &mut OverloadCandidateSet) -> OvlOutcome {
        let mut best: Option<u32> = None;
        for c in set.viable() {
            best = match best {
                None => Some(c),
                Some(b) if self.better(set, c, b) => Some(c),
                keep => keep,
            };
        }
        let Some(b) = best else { return OvlOutcome::NoViable };
        // Every viable candidate the best is not better than is
        // ambiguous with it, transitively (clang's `PendingBest`).
        set.cands[b as usize].ambiguous_mark = true;
        let mut pending = vec![b];
        let mut ambiguous = false;
        while let Some(cur) = pending.pop() {
            for c in 0..u32::try_from(set.cands.len()).unwrap_or(u32::MAX) {
                let x = set.cand(c);
                if x.viable && !x.ambiguous_mark && !self.better(set, cur, c) {
                    set.cands[c as usize].ambiguous_mark = true;
                    pending.push(c);
                    ambiguous = true;
                }
            }
        }
        if ambiguous {
            return OvlOutcome::Ambiguous;
        }
        if set.cand(b).decl.is_some()
            && set.cand(b).kind != CandidateKind::Surrogate
            && self.ovl_deleted(set.cand(b).func())
        {
            return OvlOutcome::Deleted(b);
        }
        OvlOutcome::Success(b)
    }
}
