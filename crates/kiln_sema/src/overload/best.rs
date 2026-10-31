//! Viable functions and the best viable function ([over.match.viable],
//! [over.match.best]; spec/sema-overload.md §3, §4).

use kiln_ast::decl::DeclKind;
use kiln_ast::{DeclId, ExprId};

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
        } else if let Some(sat) =
            self.check_function_constraints(f, set.loc).filter(|&s| !self.sat_ok(s))
        {
            // [over.match.viable]/3: the trailing requires-clause, after
            // the arity check (clang `AddOverloadCandidate`).
            cand.viable = false;
            cand.failure = CandidateFailure::ConstraintsNotSatisfied { sat };
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
            // /2.5: the more specialized template; /2.6: of non-template
            // functions with the same parameter-type-lists, the more
            // constrained.
            _ => {
                if set.tmpl_order.contains(&(c1, c2)) {
                    return true;
                }
                if set.tmpl_order.contains(&(c2, c1)) {
                    return false;
                }
            }
        }
        // /2.8: a non-rewritten candidate over a rewritten one; /2.9:
        // of two rewritten candidates, the one not reversed.
        if (x.rewrite == Rewrite::None) != (y.rewrite == Rewrite::None) {
            return x.rewrite == Rewrite::None;
        }
        if x.rewrite.is_reversed() != y.rewrite.is_reversed() {
            return !x.rewrite.is_reversed();
        }
        // /2.7 (constructor of a derived class over
        // an inherited constructor): inherited constructors are not
        // candidates yet. /2.8-2.10 (rewritten candidates, deduction
        // guides): WP-068b, S3T.
        false
    }

    /// Records in `set.tmpl_order` which viable function template
    /// specializations come from a more specialized template than which
    /// ([over.match.best.general]/2.5, [temp.func.order]), for `better`.
    pub(crate) fn order_template_candidates(&mut self, set: &mut OverloadCandidateSet) {
        set.tmpl_order.clear();
        self.order_constrained_candidates(set);
        if set.cands.iter().filter(|c| c.viable && c.template.is_some()).nth(1).is_none() {
            return;
        }
        let ts: Vec<(u32, DeclId)> = set
            .cands
            .iter()
            .enumerate()
            .filter(|(_, c)| c.viable)
            .filter_map(|(i, c)| Some((u32::try_from(i).ok()?, c.template?)))
            .collect();
        if ts.len() < 2 {
            return;
        }
        let nargs = set.args.len();
        for (i, &(c1, _)) in ts.iter().enumerate() {
            for &(c2, _) in &ts[i + 1..] {
                if self.more_specialized_candidate(set, c1, c2, nargs) {
                    set.tmpl_order.push((c1, c2));
                } else if self.more_specialized_candidate(set, c2, c1, nargs) {
                    set.tmpl_order.push((c2, c1));
                }
            }
        }
    }

    /// Records in `set.tmpl_order` which viable non-template candidates
    /// are more constrained than which ([over.match.best.general]/2.6,
    /// clang `getMoreConstrainedFunction`): both non-templates with the
    /// same non-object parameter-type-lists, the constraints those of the
    /// members they were instantiated from.
    fn order_constrained_candidates(&mut self, set: &mut OverloadCandidateSet) {
        // Fast path (no allocation): no viable non-template candidate
        // has a trailing requires-clause.
        let constrained = |c: &Candidate| {
            c.viable
                && c.template.is_none()
                && c.decl.is_some_and(|f| self.ast.templates.trailing_requires.get(f).is_some())
        };
        if !set.cands.iter().any(constrained) {
            return;
        }
        let fs: Vec<(u32, DeclId, Vec<ExprId>)> = set
            .cands
            .iter()
            .enumerate()
            .filter(|(_, c)| c.viable && c.template.is_none())
            .filter_map(|(i, c)| {
                let f = c.decl?;
                let r = self.ast.templates.trailing_requires.get(f).copied();
                Some((u32::try_from(i).ok()?, f, r.into_iter().collect()))
            })
            .collect();
        for (i, (c1, f1, r1)) in fs.iter().enumerate() {
            for (c2, f2, r2) in &fs[i + 1..] {
                if r1.is_empty() && r2.is_empty() || !self.same_param_type_lists(*f1, *f2) {
                    continue;
                }
                if self.more_constrained(*f1, r1, *f2, r2) {
                    set.tmpl_order.push((*c1, *c2));
                } else if self.more_constrained(*f2, r2, *f1, r1) {
                    set.tmpl_order.push((*c2, *c1));
                }
            }
        }
    }

    /// clang `sameFunctionParameterTypeLists`: the same variadic-ness and
    /// non-object parameter types, and for two implicit object member
    /// functions the same cv- and ref-qualifiers.
    fn same_param_type_lists(&mut self, f1: DeclId, f2: DeclId) -> bool {
        let (mut p1, mut p2) = (Vec::new(), Vec::new());
        let v1 = self.fn_param_types(f1, &mut p1);
        let v2 = self.fn_param_types(f2, &mut p2);
        let s1 = usize::from(self.has_explicit_object(f1));
        let s2 = usize::from(self.has_explicit_object(f2));
        if v1 != v2 || p1.len() - s1 != p2.len() - s2 {
            return false;
        }
        if !p1[s1..].iter().zip(&p2[s2..]).all(|(&a, &b)| self.ast.types.same_type(a, b)) {
            return false;
        }
        let quals = |s: &Self, f: DeclId| {
            let t = s.ast.decls.get(f).ty()?;
            match s.ast.types.view(t.ty()) {
                kiln_ast::types::TypeView::Function(p) => {
                    Some((p.info().this_quals(), p.info().ref_qual()))
                }
                _ => None,
            }
        };
        let method = |s: &Self, f: DeclId| {
            s.kind_of(f) != DeclKind::Function
                && !s.is_static_method(f)
                && !s.has_explicit_object(f)
        };
        if method(self, f1) && method(self, f2) {
            return quals(self, f1) == quals(self, f2);
        }
        true
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
