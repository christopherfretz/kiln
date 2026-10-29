//! Viable functions and the best viable function ([over.match.viable],
//! [over.match.best]; spec/sema-overload.md §3, §4).

use kiln_ast::DeclId;

use super::{
    Candidate, CandidateFailure, CandidateKind, Ics, IcsOptions, OverloadCandidateSet, OvlOutcome,
    SetKind,
};
use crate::Sema;
use crate::conv::{IcsCompare, compare_standard_conversions};

impl Sema<'_, '_> {
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
        let explicit_obj = set.object.is_some() && self.has_explicit_object(f);
        let skip = usize::from(explicit_obj);
        let nparams = params.len().saturating_sub(skip);
        let nargs = set.args.len();
        let min = self.fn_min_args(f).saturating_sub(skip);
        let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
        cand.ics = start..start;
        if nargs > nparams && !variadic {
            cand.viable = false;
            cand.failure = CandidateFailure::TooManyArgs { max: u16::try_from(nparams).unwrap_or(u16::MAX) };
        } else if nargs < min {
            cand.viable = false;
            cand.failure = CandidateFailure::TooFewArgs { min: u16::try_from(min).unwrap_or(u16::MAX) };
        } else if kind == CandidateKind::Ctor
            && set.kind == SetKind::CopyInit
            && self.fn_spec(f).is_explicit()
            && !first_opts.contains(IcsOptions::ALLOW_EXPLICIT)
        {
            cand.viable = false;
            cand.failure = CandidateFailure::Explicit;
        } else {
            if let Some(o) = set.object {
                let x = if explicit_obj {
                    self.try_implicit_conversion(&o, params[0], IcsOptions::NONE)
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
                            let opts = if i == 0 { first_opts } else { IcsOptions::NONE };
                            self.try_implicit_conversion(&a, p, opts)
                        }
                        None => Ics::Ellipsis,
                    };
                    set.ics.push(x);
                    if x.is_bad() {
                        cand.viable = false;
                        cand.failure =
                            CandidateFailure::BadConversion { arg: u16::try_from(i).unwrap_or(u16::MAX) };
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
        let mut has_better = false;
        // /2.1: no worse for every argument, better for one. In a
        // conversion set, a conversion function's object argument and a
        // constructor's argument occupy the same slot.
        for (x, y) in a.iter().zip(b.iter()) {
            if x.is_ignored() || y.is_ignored() {
                continue;
            }
            match self.compare_ics(x, y) {
                IcsCompare::Better => has_better = true,
                IcsCompare::Worse => return false,
                IcsCompare::Indistinguishable => {}
            }
        }
        if has_better {
            return true;
        }
        let (x, y) = (set.cand(c1), set.cand(c2));
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
        let ambiguous: Vec<u32> =
            set.viable().filter(|&c| c != b && !self.better(set, b, c)).collect();
        if !ambiguous.is_empty() {
            set.cands[b as usize].ambiguous_mark = true;
            for c in ambiguous {
                set.cands[c as usize].ambiguous_mark = true;
            }
            return OvlOutcome::Ambiguous;
        }
        if self.ovl_deleted(set.cand(b).func()) {
            return OvlOutcome::Deleted(b);
        }
        OvlOutcome::Success(b)
    }
}
