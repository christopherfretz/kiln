//! Candidate notes (spec/sema-overload.md §9; clang `NoteCandidates`,
//! `CompareOverloadCandidatesForDisplay`): which candidates are noted, in
//! what order, and in which wording.

use std::cmp::Ordering;

use kiln_ast::decl::{DeclFlags, DeclKind, Defaulted};
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
            CandidateKind::Ctor
                if c.found != f && self.kind_of(c.found) == DeclKind::ConstructorUsingShadow =>
            {
                "inherited constructor"
            }
            CandidateKind::Ctor => "constructor",
            // clang `oc_implicit_equality_comparison`, ahead of the
            // reversed form ([class.compare.default]/4).
            _ if implicit
                && self.ast.decls.get(f).name()
                    == kiln_ast::names::DeclName::operator(
                        kiln_ast::names::OverloadedOperator::EqualEqual,
                    ) =>
            {
                "function (the implicit 'operator==' for this 'operator<=>)'"
            }
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
    /// specialization was formed (CWG1391), as was one made
    /// non-dependent by the explicit template arguments, so that note
    /// names the template.
    fn spec_desc(&self, c: &Candidate, oper_fn: bool) -> Option<String> {
        let f = c.func();
        if !self.is_function_spec(f) {
            return None;
        }
        let (_, info) = self.ast.spec_info(f)?;
        let (tmpl, args) = (info.template, info.args);
        // The implicit object argument's conversion is one of those
        // checked first (clang `CheckNonDependentConversions` "Check the
        // 'this' conversion first"; WP-823).
        if let CandidateFailure::BadConversion { arg: u16::MAX } = c.failure
            && !oper_fn
            && !self.has_explicit_object(f)
        {
            return None;
        }
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
            let explicit = !oper_fn && param < 64 && c.explicit_nondep >> param & 1 != 0;
            if explicit || ptys.get(param).is_some_and(|&p| !self.is_dependent_type(p)) {
                return None;
            }
        }
        // The specialization's arguments are canonical in clang (its
        // `TemplateArgumentList` is the canonical builder's), whatever
        // sugar the deduction or the explicit arguments carried: `f<s::pl>`
        // is noted `[with T = s::plain]` (WP-373).
        Some(self.deduced_args_desc(tmpl, self.ast.types.canonical_args(args)))
    }

    /// The detail of candidate `c`'s bad conversion (`slot` in its
    /// sequences; `arg` the argument index, `None` for the object;
    /// `expr` the argument expression when known).
    pub(crate) fn bad_conversion_detail(
        &self,
        x: &Ics,
        arg: Option<usize>,
        expr: Option<kiln_ast::ExprId>,
    ) -> String {
        let Ics::Bad(b) = x else { return String::new() };
        let which = match arg {
            Some(i) => format!("{} argument", ordinal(i + 1)),
            None => "object argument".to_owned(),
        };
        match b.kind {
            BadKind::NoConversion if let Some(n) = expr.and_then(|e| self.overload_arg_name(e)) => {
                // clang `note_ovl_candidate_bad_overload`: an overload set
                // argument (a template-id or `&` included) with no
                // selection that converts ([over.over]).
                format!("no overload of '{n}' matching {} for {which}", self.ty_arg(b.to))
            }
            BadKind::NoConversion => {
                if let Some(s) = self.base_to_derived_pointer(b.from, b.to) {
                    return format!("{s} for {which}");
                }
                let (from, to) = self.ty_diff(b.from, b.to);
                format!("no known conversion from {from} to {to} for {which}")
            }
            BadKind::LvalueRequired => format!("expects an lvalue for {which}"),
            BadKind::RvalueRequired => format!("expects an rvalue for {which}"),
            BadKind::LosesQuals(_) if !self.same_unqualified_referee(b.from, b.to) => {
                // clang names the lost qualifiers only for the same type
                // (`DiagnoseBadConversion`); a derived class is no known
                // conversion.
                let (from, to) = self.ty_diff(b.from, b.to);
                format!("no known conversion from {from} to {to} for {which}")
            }
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
                // A reference parameter names the referenced type (clang
                // reports the temporary's list-initialization failure).
                let to = if self.ast.types.is_reference(b.to) {
                    self.pointee_sugared(b.to).unwrap_or(b.to)
                } else {
                    b.to
                };
                format!("cannot convert initializer list argument to {}", self.ty_arg(to))
            }
        }
    }

    /// Whether `from` is, cv aside, the type `to` refers or points to.
    fn same_unqualified_referee(&self, from: kiln_ast::QualType, to: kiln_ast::QualType) -> bool {
        let t = &self.ast.types;
        let (from, to) = if t.is_reference(to) {
            (from, t.pointee(to).unwrap_or(to))
        } else if let (Some(f), Some(p)) = (t.pointee(from), t.pointee(to)) {
            (f, p)
        } else {
            (from, to)
        };
        t.canonical(from).without_quals() == t.canonical(to).without_quals()
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

    /// `x` with its target replaced by parameter `param` of `f` as
    /// written, when that is the same type: the candidate's conversions
    /// run against the canonical prototype, but clang's
    /// `DiagnoseBadConversion` prints the declared parameter type with
    /// its sugar (`'WI' (aka 'W<int>')`, WP-227).
    fn sugared_bad_param(&self, f: kiln_ast::DeclId, x: Ics, param: Option<usize>) -> Ics {
        let (Ics::Bad(mut b), Some(i)) = (x, param) else { return x };
        let (ptys, _) = self.proto_param_types(f);
        // The canonical prototype drops a parameter's top-level cv
        // ([dcl.fct]/5); clang prints it as declared (`'const CI *const'`).
        let t = &self.ast.types;
        if let Some(&p) = ptys.get(i)
            && p != b.to
            && t.canonical(p).without_quals() == t.canonical(b.to).without_quals()
        {
            b.to = p;
        }
        Ics::Bad(b)
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
                let (at, spec) = self.deleted_note_decl(f);
                let defaulted = spec
                    || self.fn_spec(first).defaulted() != Defaulted::No
                    || self.fn_spec(f).defaulted() != Defaulted::No;
                let loc = self.ast.decls.get(at).loc();
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
            // clang `DiagnoseFailedEnableIfAttr`: at the function.
            CandidateFailure::EnableIf { attr } => return self.enable_if_note(attr, loc),
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
                let x = self.sugared_bad_param(f, x, Some(slot));
                self.bad_conversion_detail(&x, Some(slot), None)
            }
            CandidateFailure::BadConversion { arg } => {
                let (slot, which) = if arg == u16::MAX {
                    (0, None)
                } else {
                    let i = usize::from(arg);
                    (i + usize::from(set.object.is_some()), Some(i))
                };
                let x = set.ics_of(c).get(slot).copied().unwrap_or(Ics::Ellipsis);
                // The parameter the argument initializes: an explicit
                // object parameter takes the object argument.
                let param = if self.has_explicit_object(f) && set.object.is_some() {
                    Some(which.map_or(0, |i| i + 1))
                } else {
                    which
                };
                let x = self.sugared_bad_param(f, x, param);
                let e = which.and_then(|i| set.args.get(i)).and_then(|a| a.expr);
                self.bad_conversion_detail(&x, which, e)
            }
            CandidateFailure::ConstraintsNotSatisfied { sat } => {
                let d = self
                    .diag(DiagId::note_ovl_candidate_kind_not_viable, loc)
                    .arg(kind)
                    .arg("constraints not satisfied");
                return self.with_sat_notes(d, sat);
            }
            CandidateFailure::Explicit => {
                let which = match self.ctad_guide_kind(f, cand.template) {
                    Some(None) => 2,
                    _ => u8::from(cand.kind == CandidateKind::ConvFn),
                };
                // At the first declaration, which carries the
                // explicit-specifier (clang `DiagnoseFailedExplicitSpec`).
                let loc = self.ast.decls.get(self.ast.first(f)).loc();
                return self.diag(DiagId::note_ovl_candidate_explicit, loc).arg(which);
            }
            CandidateFailure::IllegalCtor => {
                let which = u8::from(cand.template.is_some());
                return self.diag(DiagId::note_ovl_candidate_illegal_constructor, loc).arg(which);
            }
            CandidateFailure::InhCtorSlice { moving } => {
                let which = u8::from(cand.template.is_some());
                return self
                    .diag(DiagId::note_ovl_candidate_inherited_constructor_slice, loc)
                    .arg(which)
                    .arg(u8::from(moving));
            }
            CandidateFailure::None
            | CandidateFailure::BadFinalConversion
            | CandidateFailure::TrivialConversion
            | CandidateFailure::BadDeduction => {
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
            // An inherited copy/move constructor excluded by
            // [over.match.funcs]/8 is not listed (clang
            // `NoteFunctionCandidate`, `ovl_fail_inhctor_slice`).
            .filter(|&c| {
                let cand = set.cand(c);
                !matches!(cand.failure, CandidateFailure::InhCtorSlice { .. })
                    || cand.template.is_some()
                    || !matches!(
                        self.special_kind_of(cand.func()),
                        Some(SpecialKind::CopyCtor | SpecialKind::MoveCtor)
                    )
            })
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
                // A surrogate whose conversion function's constraints are
                // not satisfied (a templated lambda's) is not shown, as in
                // clang 19.
                if !matches!(
                    set.cand(c).failure,
                    CandidateFailure::ConstraintsNotSatisfied { .. }
                ) {
                    out.push(self.surrogate_note(set.cand(c).func()));
                }
            } else {
                out.push(self.ovl_note(set, c));
                // clang `MaybeEmitInheritedConstructorNote` (not after a
                // deduction failure: `DiagnoseBadDeduction` omits it).
                let found = set.cand(c).found;
                if self.kind_of(found) == DeclKind::ConstructorUsingShadow
                    && !matches!(set.cand(c).failure, CandidateFailure::BadDeduction)
                {
                    let nominated = self.ast.decls.ctor_shadow_nominated(found);
                    let base = nominated.and_then(|n| self.ast.decls.get(n).ctx()).or_else(|| {
                        self.ast.using_target(found).and_then(|t| self.ast.decls.get(t).ctx())
                    });
                    if let Some(b) = base {
                        let ty = self.class_type(b);
                        let loc = self.ast.decls.get(found).loc();
                        let d = self.diag(DiagId::note_ovl_candidate_inherited_constructor, loc);
                        out.push(d.arg(self.ty_arg(ty)));
                    }
                }
                // clang `NoteImplicitDeductionGuide`: an implicit guide's
                // notes say what it was synthesized as, as do those of
                // every alias template guide (even one formed from a
                // user-declared guide).
                let cand = set.cand(c);
                let synthesized = match self.ctad_guide_kind(cand.func(), cand.template) {
                    Some(Some(_)) => true,
                    Some(None) => cand.template.is_some_and(|t| {
                        self.templ.ctad_alias.contains_key(&t) || self.is_inherited_guide(t)
                    }),
                    None => false,
                };
                if synthesized
                    && let Some(t) = cand.template
                    && let Some(sig) = self.implicit_guide_signature(t)
                {
                    let loc = self.ast.decls.get(cand.func()).loc();
                    out.push(self.diag(DiagId::note_implicit_deduction_guide, loc).arg(sig));
                }
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
        if ambiguous {
            let cands: Vec<_> = set
                .cands
                .iter()
                .filter(|c| c.decl.is_some())
                .map(|c| (c.func(), c.template))
                .collect();
            for n in self.ambiguous_constraint_notes(&cands) {
                msg = msg.with_note(n);
            }
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
                    // A conversion function (no parameters, or only its
                    // explicit object one) noted for the one argument of
                    // its set sorts by that count too (clang's
                    // `getNumParams`): after the other failures.
                    _ if (set.note_extra_args > 0 || c.kind == CandidateKind::ConvFn)
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
                    (true, true) => {
                        // clang `RankDeductionFailure`.
                        let (lr, rr) = (deduction_rank(l), deduction_rank(r));
                        if lr != rr {
                            return lr.cmp(&rr);
                        }
                    }
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

/// clang's `RankDeductionFailure`: the display rank of a candidate whose
/// template argument deduction failed (lower first).
fn deduction_rank(c: &Candidate) -> u8 {
    use crate::templ::deduce::Tdk;
    match c.deduction.as_ref().map(|d| d.tdk) {
        None | Some(Tdk::Success | Tdk::NonDependentConversionFailure) => 0,
        Some(Tdk::Incomplete | Tdk::IncompletePack) => 1,
        Some(Tdk::Underqualified | Tdk::Inconsistent) => 2,
        Some(
            Tdk::SubstitutionFailure
            | Tdk::DeducedMismatch
            | Tdk::ConstraintsNotSatisfied
            | Tdk::NonDeducedMismatch
            | Tdk::MiscellaneousDeductionFailure
            | Tdk::Invalid,
        ) => 3,
        Some(Tdk::InstantiationDepth) => 4,
        Some(Tdk::InvalidExplicitArguments) => 5,
        Some(Tdk::TooManyArguments | Tdk::TooFewArguments) => 6,
    }
}
