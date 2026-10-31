//! Candidate notes (spec/sema-overload.md §9; clang `NoteCandidates`,
//! `CompareOverloadCandidatesForDisplay`): which candidates are noted, in
//! what order, and in which wording.

use std::cmp::Ordering;

use kiln_ast::decl::{DeclFlags, Defaulted};
use kiln_ast::types::Quals;
use kiln_diag::{Diag, DiagId};

use super::{BadKind, Candidate, CandidateFailure, CandidateKind, Ics, OverloadCandidateSet};
use crate::Sema;
use crate::class::SpecialKind;
use crate::conv::ConvRank;
use crate::expr::ordinal;

/// "const", "const and volatile", ... (clang `note_ovl_candidate_bad_cvr`).
fn lost_quals(q: Quals, joiner: &str) -> (String, bool) {
    let mut parts = Vec::new();
    if q.is_const() {
        parts.push("const");
    }
    if q.is_volatile() {
        parts.push("volatile");
    }
    if q.is_restrict() {
        parts.push("restrict");
    }
    let s = match parts.len() {
        0 | 1 => parts.concat(),
        2 => format!("{} {joiner} {}", parts[0], parts[1]),
        _ => format!("{}, {}, {joiner} {}", parts[0], parts[1], parts[2]),
    };
    (s, parts.len() > 1)
}

impl Sema<'_, '_> {
    /// The candidate kind phrase of clang's `select_ovl_candidate_kind`:
    /// "function", "constructor (the implicit copy constructor)", ...
    /// `oper_fn`: a non-member operator candidate, whose first operand
    /// is its first argument.
    pub(crate) fn ovl_kind_str(&self, c: &Candidate, oper_fn: bool) -> String {
        let f = c.func();
        let implicit = self.ast.decls.get(f).has(DeclFlags::IMPLICIT);
        let base = match c.kind {
            CandidateKind::Ctor if implicit => match self.special_kind_of(f) {
                Some(SpecialKind::DefaultCtor) => "constructor (the implicit default constructor)",
                Some(SpecialKind::MoveCtor) => "constructor (the implicit move constructor)",
                _ => "constructor (the implicit copy constructor)",
            },
            CandidateKind::Ctor => "constructor",
            _ if c.rewrite.is_reversed() => "function (with reversed parameter order)",
            CandidateKind::Method if implicit => match self.special_kind_of(f) {
                Some(SpecialKind::MoveAssign) => "function (the implicit move assignment operator)",
                Some(SpecialKind::CopyAssign) => "function (the implicit copy assignment operator)",
                _ => "function",
            },
            _ => "function",
        };
        if let Some(desc) = self.spec_desc(c, oper_fn) {
            format!("{base} {desc}")
        } else if c.template.is_some() {
            format!("{base} template")
        } else {
            base.to_owned()
        }
    }

    /// clang's `FnDesc` for a function template specialization
    /// candidate (`[with T = int]`). A conversion that failed for a
    /// parameter of non-dependent type was checked before the
    /// specialization was formed (CWG1391), so that note names the
    /// template.
    fn spec_desc(&self, c: &Candidate, oper_fn: bool) -> Option<String> {
        let f = c.func();
        if !self.is_function_spec(f) {
            return None;
        }
        let (_, info) = self.ast.spec_info(f)?;
        let (tmpl, args) = (info.template, info.args);
        if let CandidateFailure::BadConversion { arg } = c.failure
            && (arg != u16::MAX || oper_fn)
            && let Some(pattern) = c.template.and_then(|t| self.templated_function(t))
        {
            let (ptys, _) = self.proto_param_types(pattern);
            let param = if oper_fn {
                // The written operand, in the parameter order of a
                // reversed candidate.
                let slot = if arg == u16::MAX { 0 } else { usize::from(arg) + 1 };
                if c.rewrite.is_reversed() { 1 - slot.min(1) } else { slot }
            } else {
                usize::from(arg) + usize::from(self.has_explicit_object(f))
            };
            if ptys.get(param).is_some_and(|&p| !self.is_dependent_type(p)) {
                return None;
            }
        }
        Some(self.template_args_desc(tmpl, args))
    }

    /// The detail of candidate `c`'s bad conversion (`slot` in its
    /// sequences; `arg` the argument index, `None` for the object).
    fn bad_conversion_detail(&self, x: &Ics, arg: Option<usize>) -> String {
        let Ics::Bad(b) = x else { return String::new() };
        let which = match arg {
            Some(i) => format!("{} argument", ordinal(i + 1)),
            None => "object argument".to_owned(),
        };
        match b.kind {
            BadKind::NoConversion => {
                if let Some(s) = self.base_to_derived_pointer(b.from, b.to) {
                    return format!("{s} for {which}");
                }
                format!(
                    "no known conversion from {} to {} for {which}",
                    self.ty_arg(b.from),
                    self.ty_arg(b.to)
                )
            }
            BadKind::LvalueRequired => format!("expects an lvalue for {which}"),
            BadKind::RvalueRequired => format!("expects an rvalue for {which}"),
            BadKind::LosesQuals(q) => {
                let (s, plural) = lost_quals(q, "and");
                let s = if plural { format!("{s} qualifiers") } else { format!("{s} qualifier") };
                format!("{which} ({}) would lose {s}", self.ty_arg(b.from))
            }
            BadKind::ObjectQuals => {
                let have = self.ast.types.effective_quals(b.from);
                let want = self.ast.types.effective_quals(b.to);
                let (s, _) = lost_quals(have - want, "or");
                format!(
                    "'this' argument has type {}, but method is not marked {s}",
                    self.ty_arg(b.from)
                )
            }
            BadKind::InitList => {
                format!("cannot convert initializer list argument to {}", self.ty_arg(b.to))
            }
        }
    }

    /// clang's "cannot convert from base class pointer 'B *' to derived
    /// class pointer 'D *'".
    fn base_to_derived_pointer(
        &self,
        from: kiln_ast::QualType,
        to: kiln_ast::QualType,
    ) -> Option<String> {
        let t = &self.ast.types;
        let (pf, pt) = (t.pointee(from)?, t.pointee(to)?);
        let (b, d) = (t.as_record(pf)?, t.as_record(pt)?);
        if crate::conv::bases::complete_class(self, d).is_none()
            || !crate::conv::bases::is_derived_from(self, d, b)
        {
            return None;
        }
        Some(format!(
            "cannot convert from base class pointer {} to derived class pointer {}",
            self.ty_arg(from),
            self.ty_arg(to)
        ))
    }

    /// The note for candidate `c` of `set` (clang `NoteFunctionCandidate`).
    pub(crate) fn ovl_note(&mut self, set: &OverloadCandidateSet, c: u32) -> Diag {
        let cand = set.cand(c);
        let f = cand.func();
        let loc = self.ast.decls.get(f).loc();
        let oper_fn = set.oper && cand.kind == CandidateKind::Function && set.object.is_some();
        let kind = self.ovl_kind_str(cand, oper_fn);
        if cand.viable {
            if self.ovl_deleted(f) {
                let first = self.ast.first(f);
                let defaulted = self.fn_spec(first).defaulted() != Defaulted::No
                    || self.fn_spec(f).defaulted() != Defaulted::No;
                return self
                    .diag(DiagId::note_ovl_candidate_deleted, loc)
                    .arg(kind)
                    .arg(u8::from(defaulted));
            }
            return self.diag(DiagId::note_ovl_candidate, loc).arg(kind);
        }
        if cand.failure == CandidateFailure::BadDeduction
            && let (Some(ft), Some(fail)) = (cand.template, cand.deduction.as_deref())
        {
            return self.deduction_note(ft, fail, loc);
        }
        let detail = match cand.failure {
            CandidateFailure::TooManyArgs { .. } | CandidateFailure::TooFewArgs { .. } => {
                let mut params = Vec::new();
                let variadic = self.fn_param_types(f, &mut params);
                if oper_fn {
                    // The first operand is an argument too.
                    let (n, min) = (params.len(), self.fn_min_args(f));
                    return self
                        .diag(DiagId::note_ovl_candidate_kind_not_viable, loc)
                        .arg(kind)
                        .arg(self.arity_detail(f, n, min, set.args.len() + 1, variadic));
                }
                let skip = usize::from(set.object.is_some() && self.has_explicit_object(f));
                let n = params.len().saturating_sub(skip);
                let min = self.fn_min_args(f).saturating_sub(skip);
                self.arity_detail(f, n, min, set.args.len(), variadic)
            }
            CandidateFailure::BadConversion { arg } if oper_fn => {
                // The operands are the 1st, 2nd, ... arguments.
                let slot = if arg == u16::MAX { 0 } else { usize::from(arg) + 1 };
                let x = set.ics_of(c).get(slot).copied().unwrap_or(Ics::Ellipsis);
                self.bad_conversion_detail(&x, Some(slot))
            }
            CandidateFailure::BadConversion { arg } => {
                let (slot, which) = if arg == u16::MAX {
                    (0, None)
                } else {
                    let i = usize::from(arg);
                    (i + usize::from(set.object.is_some()), Some(i))
                };
                let x = set.ics_of(c).get(slot).copied().unwrap_or(Ics::Ellipsis);
                self.bad_conversion_detail(&x, which)
            }
            CandidateFailure::Explicit => {
                let which = u8::from(cand.kind == CandidateKind::ConvFn);
                return self.diag(DiagId::note_ovl_candidate_explicit, loc).arg(which);
            }
            CandidateFailure::None
            | CandidateFailure::BadFinalConversion
            | CandidateFailure::BadDeduction
            | CandidateFailure::ConstraintsNotSatisfied => {
                return self.diag(DiagId::note_ovl_candidate, loc).arg(kind);
            }
        };
        self.diag(DiagId::note_ovl_candidate_kind_not_viable, loc).arg(kind).arg(detail)
    }

    /// The notes for a failed resolution (spec/sema-overload.md §9.2):
    /// every candidate, or with `ambiguous` the ambiguous ones, sorted
    /// for display.
    pub(crate) fn ovl_notes(&mut self, set: &OverloadCandidateSet, ambiguous: bool) -> Vec<Diag> {
        let mut order: Vec<u32> = (0..u32::try_from(set.cands.len()).unwrap_or(u32::MAX))
            .filter(|&c| !ambiguous || set.cand(c).ambiguous_mark && set.cand(c).viable)
            // A non-viable built-in candidate is never listed.
            .filter(|&c| set.cand(c).kind != CandidateKind::Builtin || set.cand(c).viable)
            .collect();
        order.sort_by(|&a, &b| self.compare_for_display(set, a, b));
        let mut out = Vec::with_capacity(order.len());
        // The ambiguous conversions of the first built-in candidate's
        // operands (clang `NoteAmbiguousUserConversions`, once).
        let mut ambiguities = true;
        for c in order {
            if set.cand(c).kind == CandidateKind::Builtin {
                out.extend(self.builtin_notes(set, c, ambiguities));
                ambiguities = false;
            } else if set.cand(c).kind == CandidateKind::Surrogate {
                out.push(self.surrogate_note(set.cand(c).func()));
            } else {
                out.push(self.ovl_note(set, c));
            }
        }
        out
    }

    /// Attaches the notes of [`Sema::ovl_notes`] to `msg`.
    pub(crate) fn with_ovl_notes(
        &mut self,
        mut msg: Diag,
        set: &OverloadCandidateSet,
        ambiguous: bool,
    ) -> Diag {
        for n in self.ovl_notes(set, ambiguous) {
            msg = msg.with_note(n);
        }
        msg
    }

    /// clang `CompareConversions`: the first operand whose conversion
    /// kind (standard, user-defined, ellipsis, none) and rank differ
    /// decides (a total order, unlike [over.match.best]).
    fn compare_conversions(set: &OverloadCandidateSet, a: u32, b: u32) -> Ordering {
        // clang `ConversionSignals`: (kind rank, rank); an ignored object
        // argument counts as an exact match. A sequence not computed
        // after the first bad one counts as bad.
        let sig = |x: Option<&Ics>| -> (u8, ConvRank) {
            match x {
                Some(Ics::Standard(s)) => (0, s.rank()),
                Some(Ics::UserDefined(u)) => (1, u.after.rank()),
                Some(Ics::Ambiguous { .. }) => (1, ConvRank::ExactMatch),
                Some(Ics::Ellipsis) => (2, ConvRank::ExactMatch),
                Some(Ics::Bad(_)) | None => (3, ConvRank::ExactMatch),
            }
        };
        let (x, y) = (set.ics_of(a), set.ics_of(b));
        // clang tests the right candidate's `IgnoreObjectArgument` for
        // every operand, not just the first.
        let y_ignores = y.first().is_some_and(Ics::is_ignored);
        let exact = (0, ConvRank::ExactMatch);
        for i in 0..x.len().max(y.len()) {
            let p = if i == 0 && x.first().is_some_and(Ics::is_ignored) {
                exact
            } else {
                sig(x.get(i))
            };
            let q = if y_ignores { exact } else { sig(y.get(i)) };
            if p != q {
                return p.cmp(&q);
            }
        }
        Ordering::Equal
    }

    /// clang `CompareOverloadCandidatesForDisplay` as an ordering.
    fn compare_for_display(&self, set: &OverloadCandidateSet, a: u32, b: u32) -> Ordering {
        let (l, r) = (set.cand(a), set.cand(b));
        match (l.viable, r.viable) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (true, true) => {
                let o = Self::compare_conversions(set, a, b);
                if o != Ordering::Equal {
                    return o;
                }
            }
            (false, false) => {
                let nargs = set.args.len() + set.note_extra_args;
                // clang `EffectiveFailureKind`: a function whose
                // parameter count does not fit the argument count sorts as
                // an arity mismatch (in an operator set a member's
                // implicit object parameter counts as an argument).
                let arity = |c: &Candidate| match c.failure {
                    CandidateFailure::TooManyArgs { max } => Some((usize::from(max), true)),
                    CandidateFailure::TooFewArgs { min } if c.kind == CandidateKind::Surrogate => {
                        Some((usize::from(min), false))
                    }
                    CandidateFailure::TooFewArgs { .. } => {
                        let mut p = Vec::new();
                        let _ = self.fn_param_types(c.func(), &mut p);
                        Some((p.len(), false))
                    }
                    _ if set.note_extra_args > 0
                        && c.decl.is_some()
                        && c.kind != CandidateKind::Surrogate =>
                    {
                        let mut p = Vec::new();
                        let variadic = self.fn_param_types(c.func(), &mut p);
                        if nargs > p.len() && !variadic {
                            Some((p.len(), true))
                        } else if nargs < self.fn_min_args(c.func()) {
                            Some((p.len(), false))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                match (arity(l), arity(r)) {
                    (Some((ln, lmany)), Some((rn, rmany))) => {
                        let (ld, rd) = (ln.abs_diff(nargs), rn.abs_diff(nargs));
                        if ld != rd {
                            return ld.cmp(&rd);
                        }
                        if lmany != rmany {
                            return if lmany { Ordering::Less } else { Ordering::Greater };
                        }
                        return Ordering::Equal;
                    }
                    (Some(_), None) => return Ordering::Greater,
                    (None, Some(_)) => return Ordering::Less,
                    (None, None) => {}
                }
                let bad =
                    |c: &Candidate| matches!(c.failure, CandidateFailure::BadConversion { .. });
                match (bad(l), bad(r)) {
                    (true, false) => return Ordering::Less,
                    (false, true) => return Ordering::Greater,
                    (true, true) => {
                        let o = Self::compare_conversions(set, a, b);
                        if o != Ordering::Equal {
                            return o;
                        }
                    }
                    (false, false) => {}
                }
                let ded = |c: &Candidate| matches!(c.failure, CandidateFailure::BadDeduction);
                match (ded(l), ded(r)) {
                    (true, false) => return Ordering::Less,
                    (false, true) => return Ordering::Greater,
                    _ => {}
                }
            }
        }
        let loc = |c: &Candidate| {
            if c.kind == CandidateKind::Builtin {
                kiln_source::Loc::INVALID
            } else {
                self.ast.decls.get(c.func()).loc()
            }
        };
        let (ll, rl) = (loc(l), loc(r));
        self.loc_order(ll, rl)
    }
}

impl Sema<'_, '_> {
    /// Source order of two locations in the translation unit (clang
    /// `isBeforeInTranslationUnit`, through the include stack); invalid
    /// locations go last.
    pub(crate) fn loc_order(&self, a: kiln_source::Loc, b: kiln_source::Loc) -> Ordering {
        match (a.is_valid(), b.is_valid()) {
            (false, false) => return Ordering::Equal,
            (false, true) => return Ordering::Greater,
            (true, false) => return Ordering::Less,
            (true, true) => {}
        }
        let sm = self.diags.sm();
        let chain = |l: kiln_source::Loc| {
            let mut v = vec![sm.expansion_loc(l)];
            while v.len() < 256 {
                let up = sm.include_loc(v[v.len() - 1]);
                if !up.is_valid() {
                    break;
                }
                v.push(sm.expansion_loc(up));
            }
            v.reverse();
            v
        };
        let (x, y) = (chain(a), chain(b));
        for (p, q) in x.iter().zip(&y) {
            if p != q {
                return p.cmp(q);
            }
        }
        x.len().cmp(&y.len())
    }
}
