//! Overloaded operators in expressions ([over.match.oper], [over.sub],
//! [over.call.object]; spec/sema-overload.md §2.5, §5, §8): the candidate
//! set of an operator with a class or enumeration operand — member,
//! non-member and built-in candidates plus the C++20 rewritten
//! comparisons — and the `CXXOperatorCallExpr` of the selected function.

use kiln_ast::decl::{DeclFlags, DeclKind, Defaulted};
use kiln_ast::expr::{BinaryOp, DeclRefExtra, UnaryOp, ValueCategory};
use kiln_ast::names::{DeclName, OverloadedOperator as OO};
use kiln_ast::types::{Quals, RefQual, TypeKind, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use super::{CandidateFailure, CandidateKind, Ics, IcsOptions, OverloadCandidateSet, OvlOutcome};
use super::{Rewrite, SetKind};
use crate::Sema;
use crate::conv::IcsCompare;
use crate::expr::CallCallee;
use crate::init::InitList;
use crate::lookup::LookupKind;

/// The syntactic form of an operator use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum OperForm {
    /// `a @ b`.
    Binary(BinaryOp),
    /// `@a` or `a@`.
    Unary(UnaryOp),
    /// `a[b...]`.
    Subscript,
    /// `a(b...)`.
    Call,
}

/// What [`Sema::try_overloaded_operator`] decided.
#[derive(Debug)]
pub(crate) enum OperResult {
    /// No overloaded use: the built-in operator applies.
    Builtin,
    /// No viable function ([over.match.oper]/9 aside): the built-in
    /// operator, whose error carries these candidate notes.
    BuiltinNotes(Vec<Diag>),
    /// A built-in candidate won ([over.match.oper]/11): the built-in
    /// operator applies to these operands (a class operand converted by
    /// its user-defined conversion).
    Converted(Vec<ExprId>),
    /// The result (an error expression after diagnosing).
    Done(ExprId),
}

/// The operator function name of a binary operator.
pub(crate) fn binary_oper(op: BinaryOp) -> Option<OO> {
    use BinaryOp as B;
    Some(match op {
        B::PtrMemD => return None,
        B::PtrMemI => OO::ArrowStar,
        B::Mul => OO::Star,
        B::Div => OO::Slash,
        B::Rem => OO::Percent,
        B::Add => OO::Plus,
        B::Sub => OO::Minus,
        B::Shl => OO::LessLess,
        B::Shr => OO::GreaterGreater,
        B::Cmp => OO::Spaceship,
        B::LT => OO::Less,
        B::GT => OO::Greater,
        B::LE => OO::LessEqual,
        B::GE => OO::GreaterEqual,
        B::EQ => OO::EqualEqual,
        B::NE => OO::ExclaimEqual,
        B::And => OO::Amp,
        B::Xor => OO::Caret,
        B::Or => OO::Pipe,
        B::LAnd => OO::AmpAmp,
        B::LOr => OO::PipePipe,
        B::Assign => OO::Equal,
        B::MulAssign => OO::StarEqual,
        B::DivAssign => OO::SlashEqual,
        B::RemAssign => OO::PercentEqual,
        B::AddAssign => OO::PlusEqual,
        B::SubAssign => OO::MinusEqual,
        B::ShlAssign => OO::LessLessEqual,
        B::ShrAssign => OO::GreaterGreaterEqual,
        B::AndAssign => OO::AmpEqual,
        B::XorAssign => OO::CaretEqual,
        B::OrAssign => OO::PipeEqual,
        B::Comma => OO::Comma,
    })
}

/// The binary operator whose operator function is `op`.
pub(crate) fn binary_of(op: OO) -> Option<BinaryOp> {
    (0..=u8::MAX).map_while(BinaryOp::from_u8).find(|&b| binary_oper(b) == Some(op))
}

/// The prefix (`postfix`: postfix) unary operator whose operator
/// function is `op`.
pub(crate) fn unary_of(op: OO, postfix: bool) -> Option<UnaryOp> {
    use UnaryOp as U;
    Some(match (op, postfix) {
        (OO::PlusPlus, false) => U::PreInc,
        (OO::PlusPlus, true) => U::PostInc,
        (OO::MinusMinus, false) => U::PreDec,
        (OO::MinusMinus, true) => U::PostDec,
        (OO::Amp, false) => U::AddrOf,
        (OO::Star, false) => U::Deref,
        (OO::Plus, false) => U::Plus,
        (OO::Minus, false) => U::Minus,
        (OO::Tilde, false) => U::Not,
        (OO::Exclaim, false) => U::LNot,
        _ => return None,
    })
}

/// The operator function name of a unary operator.
pub(crate) fn unary_oper(op: UnaryOp) -> Option<OO> {
    use UnaryOp as U;
    Some(match op {
        U::PostInc | U::PreInc => OO::PlusPlus,
        U::PostDec | U::PreDec => OO::MinusMinus,
        U::AddrOf => OO::Amp,
        U::Deref => OO::Star,
        U::Plus => OO::Plus,
        U::Minus => OO::Minus,
        U::Not => OO::Tilde,
        U::LNot => OO::Exclaim,
        U::Real | U::Imag | U::Extension | U::Coawait => return None,
    })
}

/// [over.match.oper]/3.4: the operator whose functions are rewrite
/// targets of `op` besides `op` itself (clang
/// `getRewrittenOverloadedOperator`).
pub(crate) fn rewritten_of(op: OO) -> Option<OO> {
    match op {
        OO::ExclaimEqual => Some(OO::EqualEqual),
        OO::Less | OO::Greater | OO::LessEqual | OO::GreaterEqual => Some(OO::Spaceship),
        _ => None,
    }
}

/// Candidates named `op` have synthesized reversed forms.
pub(crate) fn allows_reversed(op: OO) -> bool {
    matches!(op, OO::EqualEqual | OO::Spaceship)
}

impl OperForm {
    fn oper(self) -> Option<OO> {
        match self {
            OperForm::Binary(b) => binary_oper(b),
            OperForm::Unary(u) => unary_oper(u),
            OperForm::Subscript => Some(OO::Subscript),
            OperForm::Call => Some(OO::Call),
        }
    }
}

/// A parameter type up to top-level cv-qualifiers, for
/// [`Sema::allow_ambiguity`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParamKey {
    /// A non-reference type.
    Ty(QualType),
    /// A reference to `quals base`.
    Ref { rvalue: bool, base: QualType, quals: Quals },
}

/// The parts of the selected candidate a result is built from.
#[derive(Clone, Copy)]
struct Chosen {
    f: DeclId,
    found: DeclId,
    rewrite: Rewrite,
}

impl Sema<'_, '_> {
    /// clang `allowAmbiguity`: a candidate and the reversed form of `f2`
    /// with the same parameter types (the implicit object parameter
    /// counting as the first) are not ambiguous by a better conversion
    /// for one operand and a worse one for the other.
    pub(crate) fn allow_ambiguity(&self, f1: DeclId, f2: DeclId) -> bool {
        if self.ast.first(f1) == self.ast.first(f2) {
            return true;
        }
        let p1 = self.ambiguity_params(f1);
        let p2 = self.ambiguity_params(f2);
        p1 == p2
    }

    /// The parameter types of `f` up to top-level cv, its implicit
    /// object parameter first.
    fn ambiguity_params(&self, f: DeclId) -> Vec<ParamKey> {
        let t = &self.ast.types;
        let mut params = Vec::new();
        self.fn_param_types(f, &mut params);
        let mut out: Vec<ParamKey> = params
            .iter()
            .map(|&p| match t.canonical_view(p) {
                TypeView::LValueRef(x) | TypeView::RValueRef(x) => {
                    let c = t.canonical(x);
                    ParamKey::Ref {
                        rvalue: matches!(t.canonical_view(p), TypeView::RValueRef(_)),
                        base: c.without_quals(),
                        quals: t.effective_quals(x) & Quals::CV,
                    }
                }
                _ => ParamKey::Ty(t.canonical(p).without_quals()),
            })
            .collect();
        if self.kind_of(f) == DeclKind::CXXMethod
            && !self.is_static_method(f)
            && !self.has_explicit_object(f)
            && let Some(c) = self.declaring_class(f)
        {
            let (q, rq) = self.method_object_quals(f);
            let ct = self.tag_type(c);
            out.insert(
                0,
                ParamKey::Ref {
                    rvalue: rq == RefQual::RValue,
                    base: t.canonical(ct).without_quals(),
                    quals: q & Quals::CV,
                },
            );
        }
        out
    }

    /// The class type of `e`, if any.
    fn operand_class(&self, e: ExprId) -> Option<DeclId> {
        self.ast.types.as_record(self.ety(e))
    }

    /// A class or enumeration operand ([over.match.oper]/1).
    fn overloadable_operand(&self, e: ExprId) -> bool {
        let t = self.ety(e);
        self.ast.types.as_record(t).is_some() || self.ast.types.as_enum(t).is_some()
    }

    /// Resolves the operator `form` applied to `args` (the first operand
    /// first; `braced` the braced-init-list operands): an overloaded
    /// operator if an operand has class or enumeration type
    /// ([over.match.oper]/1). `end` is the `)`/`]` of a call or subscript.
    /// `rewrites`: rewritten comparison candidates are considered.
    pub(crate) fn try_overloaded_operator(
        &mut self,
        form: OperForm,
        args: &[ExprId],
        braced: &[Option<InitList>],
        loc: Loc,
        end: Loc,
    ) -> OperResult {
        self.overloaded_operator(form, args, braced, loc, end, true)
    }

    #[allow(clippy::too_many_lines, reason = "clang's candidate order, then each outcome")]
    fn overloaded_operator(
        &mut self,
        form: OperForm,
        args: &[ExprId],
        braced: &[Option<InitList>],
        loc: Loc,
        end: Loc,
        rewrites: bool,
    ) -> OperResult {
        let Some(op) = form.oper() else { return OperResult::Builtin };
        let Some(&a0) = args.first() else { return OperResult::Builtin };
        let is_braced = |i: usize| braced.get(i).is_some_and(Option::is_some);
        let member_only = matches!(
            form,
            OperForm::Subscript | OperForm::Call | OperForm::Binary(BinaryOp::Assign)
        );
        let use_it = match form {
            OperForm::Call => self.operand_class(a0).is_some(),
            // clang `ActOnArraySubscriptExpr`: a class object or a class
            // index.
            OperForm::Subscript => {
                self.operand_class(a0).is_some()
                    || (args.len() == 2 && !is_braced(1) && self.operand_class(args[1]).is_some())
            }
            OperForm::Binary(BinaryOp::Assign) => self.overloadable_operand(a0),
            _ => (0..args.len()).any(|i| !is_braced(i) && self.overloadable_operand(args[i])),
        };
        if !use_it {
            return OperResult::Builtin;
        }
        // The operands as the candidates see them: a postfix `++`/`--`
        // has a second operand `0` ([over.match.oper]/1).
        let mut ops: Vec<ExprId> = args.to_vec();
        let mut brs: Vec<Option<InitList>> =
            (0..args.len()).map(|i| braced.get(i).cloned().flatten()).collect();
        if matches!(form, OperForm::Unary(u) if u.is_postfix()) {
            let zero = self.builder().int_literal(
                QualType::INT,
                0,
                kiln_ast::expr::Radix::Decimal,
                Loc::INVALID,
            );
            ops.push(zero);
            brs.push(None);
        }
        for &e in &ops {
            if let Some(c) = self.operand_class(e) {
                let t = self.ety(e);
                let _ = self.require_complete(t, loc);
                let _ = c;
            }
        }
        let extra =
            if rewrites && matches!(form, OperForm::Binary(_)) { rewritten_of(op) } else { None };
        let reversible = rewrites
            && matches!(form, OperForm::Binary(_))
            && (extra.is_some() || allows_reversed(op));
        let mut set = self.begin_set(SetKind::Normal, loc);
        set.oper = true;
        set.note_extra_args = usize::from(form != OperForm::Call);
        set.object = Some(self.arg_info(a0));
        let mark = self.push_call_args(&mut set, &ops[1..], &brs[1..]);
        let types: Vec<QualType> = ops.iter().map(|&e| self.ety(e)).collect();
        // [over.match.oper]/3.2: non-member candidates from unqualified
        // lookup, each with its reversed form (/3.4.4).
        let mut fns: Vec<(DeclId, DeclId)> = Vec::new();
        if !member_only {
            for name in [Some(op), extra].into_iter().flatten() {
                let found = self.lookup_operator_name(DeclName::operator(name));
                for (&d, &f) in found.decls.iter().zip(&found.found) {
                    let first = self.ast.first(d);
                    if !fns.iter().any(|&(o, _)| self.ast.first(o) == first) {
                        fns.push((d, f));
                    }
                }
            }
        }
        let n_unqual = fns.len();
        for &(d, f) in &fns {
            self.add_nonmember_oper(&mut set, d, f, op, extra, &ops, &types, false);
        }
        // /3.1: member candidates of the first operand's class, then
        // the reversed and rewritten ones.
        self.add_member_opers(&mut set, op, &ops, Rewrite::None, loc);
        // [over.call.object]/2: the surrogate call functions.
        if form == OperForm::Call {
            self.add_surrogates(&mut set, a0);
        }
        if reversible && allows_reversed(op) {
            self.add_member_opers(&mut set, op, &ops, Rewrite::Reversed, loc);
        }
        if let Some(x) = extra {
            self.add_member_opers(&mut set, x, &ops, Rewrite::Different, loc);
            if allows_reversed(x) {
                self.add_member_opers(&mut set, x, &ops, Rewrite::DifferentReversed, loc);
            }
        }
        // /3.2: argument-dependent lookup (not for `=`, `[]`, `()`).
        if !member_only {
            let adl_args: Vec<ExprId> =
                args.iter().enumerate().filter(|&(i, _)| !is_braced(i)).map(|(_, &e)| e).collect();
            for name in [Some(op), extra].into_iter().flatten() {
                self.adl_lookup_dn(DeclName::operator(name), &adl_args, &mut fns);
            }
            let adl: Vec<(DeclId, DeclId)> = fns[n_unqual..].to_vec();
            for (d, f) in adl {
                self.add_nonmember_oper(&mut set, d, f, op, extra, &ops, &types, true);
            }
        }
        // /3.3: built-in candidates.
        self.add_builtin_candidates(&mut set, op, form, &ops, &types);
        let out = self.best_viable(&mut set);
        self.ovl.lists.truncate(mark);
        let undecided = set.uncertain
            || (set.skipped_templates
                && !matches!(out, OvlOutcome::Success(b) | OvlOutcome::Deleted(b)
                    if self.template_cannot_win(&set, b)));
        if undecided {
            if set.skipped_templates {
                self.templ_unimplemented("calls to function templates", loc);
            } else {
                self.unsupported("overload resolution with constraints", loc);
            }
            self.end_set(set);
            return OperResult::Done(self.recover_err(args, loc));
        }
        let spelling = match form {
            OperForm::Binary(b) => b.spelling(),
            OperForm::Unary(u) => u.spelling(),
            OperForm::Subscript => "[]",
            OperForm::Call => "()",
        };
        let t0 = types[0];
        let r = match out {
            OvlOutcome::Success(b) => {
                if set.cand(b).decl.is_none() {
                    // A built-in candidate: the built-in operator, with
                    // the operands converted ([over.match.oper]/11).
                    let conv = self.builtin_operand_conversions(&set, b, &ops);
                    self.end_set(set);
                    return match conv {
                        Some(mut v) => {
                            v.truncate(args.len());
                            OperResult::Converted(v)
                        }
                        None => OperResult::Done(self.recover_err(args, loc)),
                    };
                }
                if set.cand(b).kind == CandidateKind::Surrogate {
                    let c = set.cand(b);
                    let (f, found) = (c.func(), c.found);
                    self.end_set(set);
                    return OperResult::Done(self.call_surrogate(a0, f, found, &args[1..], loc, end));
                }
                if reversible && !set.cand(b).rewrite.is_reversed() {
                    self.diagnose_reversed_ambiguity(&set, b, op, spelling, &types, loc);
                }
                let c = set.cand(b);
                let chosen = Chosen { f: c.func(), found: c.found, rewrite: c.rewrite };
                self.end_set(set);
                return self.finish_operator(form, op, chosen, &ops, &mut brs, args, loc, end);
            }
            OvlOutcome::Deleted(b) => {
                let f = set.cand(b).func();
                let implicit = self.ovl_deleted(f)
                    && (self.fn_spec(f).defaulted() != Defaulted::No
                        || self.fn_spec(self.ast.first(f)).defaulted() != Defaulted::No);
                let msg = match (form, implicit.then(|| self.special_kind_of(f)).flatten()) {
                    (OperForm::Call, _) => {
                        let m = self
                            .diag(DiagId::sema_ovl_deleted_object_call, self.begin_loc(a0))
                            .arg(self.ty_arg(t0))
                            .arg(self.delete_message_suffix(f));
                        self.with_ovl_notes(m, &set, false)
                    }
                    (_, Some(k)) => {
                        let mut m = self
                            .diag(DiagId::sema_ovl_deleted_special_oper, loc)
                            .arg(self.ty_arg(t0))
                            .arg(k as u8);
                        for n in self.deleted_function_notes(f, 0) {
                            m = m.with_note(n);
                        }
                        m
                    }
                    _ => {
                        let name = self
                            .ast
                            .decls
                            .get(f)
                            .name()
                            .as_operator()
                            .map_or(spelling, OO::spelling);
                        let m = self
                            .diag(DiagId::sema_ovl_deleted_oper, loc)
                            .arg(name)
                            .arg(self.delete_message_suffix(f));
                        self.with_ovl_notes(m, &set, false)
                    }
                };
                self.emit(msg);
                None
            }
            OvlOutcome::Ambiguous => {
                let msg = match form {
                    OperForm::Call => self
                        .diag(DiagId::sema_ovl_ambiguous_object_call, self.begin_loc(a0))
                        .arg(self.ty_arg(t0)),
                    OperForm::Subscript if ops.len() != 2 => self
                        .diag(DiagId::sema_ovl_ambiguous_subscript_call, loc)
                        .arg(self.ty_arg(t0)),
                    OperForm::Unary(_) => self
                        .diag(DiagId::sema_ovl_ambiguous_oper_unary, loc)
                        .arg(spelling)
                        .arg(self.ty_arg(t0)),
                    _ => self
                        .diag(DiagId::sema_ovl_ambiguous_oper_binary, loc)
                        .arg(spelling)
                        .arg(self.ty_arg(t0))
                        .arg(self.ty_arg(types[1])),
                };
                let msg = self.with_ovl_notes(msg, &set, true);
                self.emit(msg);
                None
            }
            OvlOutcome::NoViable => match form {
                OperForm::Binary(BinaryOp::Comma) | OperForm::Unary(_) => {
                    self.end_set(set);
                    return OperResult::Builtin;
                }
                OperForm::Binary(b)
                    if self.operand_class(a0).is_some()
                        && (b == BinaryOp::Assign || b.is_compound_assignment()) =>
                {
                    let msg = self.diag(DiagId::sema_ovl_no_viable_oper, loc).arg(spelling);
                    let msg = self.with_ovl_notes(msg, &set, false);
                    self.emit(msg);
                    None
                }
                OperForm::Binary(_) => {
                    let notes = self.ovl_notes(&set, false);
                    self.end_set(set);
                    return OperResult::BuiltinNotes(notes);
                }
                OperForm::Subscript => {
                    let id = if set.cands.is_empty() {
                        DiagId::sema_no_subscript_operator
                    } else {
                        DiagId::sema_ovl_no_viable_subscript
                    };
                    let msg = self.diag(id, loc).arg(self.ty_arg(t0));
                    let msg = self.with_ovl_notes(msg, &set, false);
                    self.emit(msg);
                    None
                }
                OperForm::Call => {
                    let id = if set.cands.is_empty() {
                        DiagId::sema_no_call_operator
                    } else {
                        DiagId::sema_ovl_no_viable_object_call
                    };
                    let msg = self.diag(id, self.begin_loc(a0)).arg(self.ty_arg(t0));
                    let msg = self.with_ovl_notes(msg, &set, false);
                    self.emit(msg);
                    None
                }
            },
        };
        let _: Option<()> = r;
        self.end_set(set);
        OperResult::Done(self.recover_err(args, loc))
    }

    /// Adds non-member operator function `d` (found as `found`) and its
    /// reversed form ([over.match.oper]/3.2, /3.4).
    #[allow(clippy::too_many_arguments, reason = "the operator's parts")]
    fn add_nonmember_oper(
        &mut self,
        set: &mut OverloadCandidateSet,
        d: DeclId,
        found: DeclId,
        op: OO,
        extra: Option<OO>,
        ops: &[ExprId],
        types: &[QualType],
        adl_only: bool,
    ) {
        if self.ast.decls.get(d).has(DeclFlags::INVALID) {
            return;
        }
        let name = self.ast.decls.get(d).name().as_operator();
        if name != Some(op) && (extra.is_none() || name != extra) {
            return;
        }
        let rewrite = if name == Some(op) { Rewrite::None } else { Rewrite::Different };
        let reversed_too = ops.len() == 2 && self.should_add_reversed(d, ops[1]);
        if self.kind_of(d) == DeclKind::FunctionTemplate {
            let t2 = types.get(1).copied();
            if self.template_may_match(d, types[0], t2)
                || (reversed_too
                    && t2.is_some_and(|t2| self.template_may_match(d, t2, Some(types[0]))))
            {
                set.skipped_templates = true;
            }
            return;
        }
        if self.ast.decls.function(d).is_none() {
            return;
        }
        if self.ast.decls.cold.requires_clause.contains(d) {
            set.uncertain = true;
        }
        let (t1, t2) = (types[0], types.get(1).copied());
        if self.acceptable_nonmember(d, t1, t2) {
            self.add_candidate(set, d, found, CandidateKind::Function, IcsOptions::NONE);
            if let Some(c) = set.cands.last_mut() {
                c.rewrite = rewrite;
                c.adl_only = adl_only;
            }
        }
        if reversed_too
            && let Some(t2) = t2
            && self.acceptable_nonmember(d, t2, Some(t1))
        {
            let rw = if rewrite == Rewrite::None {
                Rewrite::Reversed
            } else {
                Rewrite::DifferentReversed
            };
            self.add_reversed(set, d, found, CandidateKind::Function, rw);
            if let Some(c) = set.cands.last_mut() {
                c.adl_only = adl_only;
            }
        }
    }

    /// clang `IsAcceptableNonMemberOperatorCandidate` ([over.match.oper]/3.2):
    /// without a class operand, a non-member candidate must take an
    /// enumeration operand as its own type.
    fn acceptable_nonmember(&self, d: DeclId, t1: QualType, t2: Option<QualType>) -> bool {
        let rec = |t: QualType| self.ast.types.as_record(t).is_some();
        if rec(t1) || t2.is_some_and(rec) {
            return true;
        }
        let mut params = Vec::new();
        self.fn_param_types(d, &mut params);
        let matches = |p: QualType, t: QualType| {
            self.ast.types.as_enum(t).is_some()
                && self.ast.types.same_unqualified(t, self.non_reference(p))
        };
        if params.first().is_some_and(|&p| matches(p, t1)) {
            return true;
        }
        matches!((params.get(1), t2), (Some(&p), Some(t)) if matches(p, t))
    }

    /// Whether deduction for function template `d` against operands of
    /// types `t1`, `t2` could succeed — a conservative filter until
    /// WP-071 deduces: a parameter that is a dependent
    /// template-specialization type does not deduce from a non-class
    /// operand, and without a class operand a candidate must take an
    /// enumeration operand as its own type or a template parameter.
    fn template_may_match(&self, d: DeclId, t1: QualType, t2: Option<QualType>) -> bool {
        let Some(p) = self.pattern_of(d) else { return true };
        let mut params = Vec::new();
        self.fn_param_types(p, &mut params);
        let rec = |t: QualType| self.ast.types.as_record(t).is_some();
        let spec_param = |q: QualType| {
            let n = self.non_reference(q);
            self.ast.types.is_dependent(n)
                && self.ast.types.canonical_kind(n) == TypeKind::TemplateSpecialization
        };
        for (i, t) in [Some(t1), t2].into_iter().enumerate() {
            if let (Some(t), Some(&q)) = (t, params.get(i))
                && !rec(t)
                && spec_param(q)
            {
                return false;
            }
        }
        if rec(t1) || t2.is_some_and(rec) {
            return true;
        }
        let takes = |q: QualType, t: QualType| {
            let n = self.non_reference(q);
            self.ast.types.as_enum(t).is_some()
                && (self.ast.types.same_unqualified(t, n)
                    || matches!(
                        self.ast.types.canonical_kind(n),
                        TypeKind::CanonTemplateTypeParm | TypeKind::TemplateTypeParm
                    ))
        };
        params.first().is_some_and(|&q| takes(q, t1))
            || matches!((params.get(1), t2), (Some(&q), Some(t)) if takes(q, t))
    }

    /// Adds the member candidates named `name` of the class of the
    /// object operand: the first, or for a reversed `rewrite` the second
    /// ([over.match.oper]/3.1, /3.4.4).
    pub(crate) fn add_member_opers(
        &mut self,
        set: &mut OverloadCandidateSet,
        name: OO,
        ops: &[ExprId],
        rewrite: Rewrite,
        loc: Loc,
    ) {
        let reversed = rewrite.is_reversed();
        if reversed && ops.len() != 2 {
            return;
        }
        let obj = if reversed { ops[1] } else { ops[0] };
        let Some(c) = self.operand_class(obj) else { return };
        let t = self.ety(obj);
        if !self.require_complete(t, loc) && self.ast.definition(c).is_none() {
            return;
        }
        let Some(def) = self.ast.definition(c) else { return };
        if name == OO::Equal {
            self.ensure_implicit(def, crate::class::SpecialKind::CopyAssign);
            self.ensure_implicit(def, crate::class::SpecialKind::MoveAssign);
        }
        let found = self.lookup_in_class_dn(def, DeclName::operator(name), LookupKind::Ordinary);
        for (&d, &f) in found.decls.iter().zip(&found.found) {
            if self.ast.decls.get(d).has(DeclFlags::INVALID) {
                continue;
            }
            if self.kind_of(d) == DeclKind::FunctionTemplate {
                set.skipped_templates = true;
                continue;
            }
            if self.ast.decls.function(d).is_none() {
                continue;
            }
            if reversed && !self.should_add_reversed(d, ops[1]) {
                continue;
            }
            if self.ast.decls.cold.requires_clause.contains(d) {
                set.uncertain = true;
            }
            if reversed {
                self.add_reversed(set, d, f, CandidateKind::Method, rewrite);
            } else {
                self.add_candidate(set, d, f, CandidateKind::Method, IcsOptions::NONE);
                if let Some(x) = set.cands.last_mut() {
                    x.rewrite = rewrite;
                }
            }
        }
    }

    /// Adds `d` with the two operands swapped; keeps its conversion
    /// sequences in the written operand order.
    fn add_reversed(
        &mut self,
        set: &mut OverloadCandidateSet,
        d: DeclId,
        found: DeclId,
        kind: CandidateKind,
        rewrite: Rewrite,
    ) {
        let (Some(o), Some(&a)) = (set.object, set.args.first()) else { return };
        set.object = Some(a);
        set.args[0] = o;
        self.add_candidate(set, d, found, kind, IcsOptions::NONE);
        set.object = Some(o);
        set.args[0] = a;
        let n = set.cands.len() - 1;
        let r = set.cands[n].ics.clone();
        if r.len() == 1 {
            set.ics.push(Ics::Ellipsis);
            set.cands[n].ics.end += 1;
        }
        let r = set.cands[n].ics.clone();
        if r.len() == 2 {
            set.ics.swap(r.start as usize, r.start as usize + 1);
        }
        let c = &mut set.cands[n];
        c.rewrite = rewrite;
        if let CandidateFailure::BadConversion { arg } = c.failure {
            c.failure =
                CandidateFailure::BadConversion { arg: if arg == u16::MAX { 0 } else { u16::MAX } };
        }
    }

    /// clang `shouldAddReversed`: whether operator function `d` has a
    /// reversed candidate when `o` is its first operand in the reversed
    /// form ([over.match.oper]/3.4.4, P2468).
    fn should_add_reversed(&mut self, d: DeclId, o: ExprId) -> bool {
        let fd = if self.kind_of(d) == DeclKind::FunctionTemplate {
            match self.pattern_of(d) {
                Some(p) => p,
                None => return false,
            }
        } else {
            d
        };
        let Some(name) = self.ast.decls.get(fd).name().as_operator() else { return false };
        if !allows_reversed(name) {
            return false;
        }
        if name == OO::EqualEqual && !self.should_add_reversed_eqeq(fd, o) {
            return false;
        }
        let mut params = Vec::new();
        self.fn_param_types(fd, &mut params);
        let method = self.kind_of(fd) == DeclKind::CXXMethod;
        let skip = usize::from(method && self.has_explicit_object(fd));
        let nonobj = &params[skip.min(params.len())..];
        if method && skip == 0 {
            return true;
        }
        nonobj.len() != 2 || !self.ast.types.same_unqualified(nonobj[0], nonobj[1])
    }

    /// P2468 ([over.match.oper]/4): `operator==` `f` is no rewrite target
    /// if a search for `operator!=` finds a corresponding function — in
    /// the class of the first operand `o` for a member, else in `f`'s
    /// namespace.
    fn should_add_reversed_eqeq(&mut self, f: DeclId, o: ExprId) -> bool {
        let ne = DeclName::operator(OO::ExclaimEqual);
        if self.kind_of(f) == DeclKind::CXXMethod {
            let Some(c) = self.operand_class(o) else { return true };
            let found = self.lookup_in_class_dn(c, ne, LookupKind::Ordinary);
            return !found.decls.iter().any(|&x| self.functions_correspond(f, x));
        }
        let Some(ns) = self.innermost_namespace(f) else { return true };
        let ns1 = self.ast.first(ns);
        let raws: Vec<DeclId> = self.ast.lookup_local(ns, ne).iter().collect();
        !raws.iter().any(|&raw| {
            let x = self.ast.strip_shadow(raw);
            let lex = self.ast.decls.lexical_ctx(raw).or_else(|| self.ast.decls.get(raw).ctx());
            self.functions_correspond(f, x)
                && lex.is_some_and(|l| self.ast.first(self.ast.primary_context(l)) == ns1)
        })
    }

    /// [basic.scope.scope]/4: `a` and `b` would correspond — same
    /// parameter types and object parameter.
    fn functions_correspond(&self, a: DeclId, b: DeclId) -> bool {
        if self.ast.decls.function(b).is_none() || self.kind_of(b) == DeclKind::FunctionTemplate {
            return false;
        }
        let ma = self.kind_of(a) == DeclKind::CXXMethod;
        let mb = self.kind_of(b) == DeclKind::CXXMethod;
        if ma != mb {
            return false;
        }
        let ty = |d: DeclId| self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let (Some(pa), Some(pb)) = (self.ast.types.proto(ty(a)), self.ast.types.proto(ty(b)))
        else {
            return false;
        };
        let (xa, xb) = (self.ast.types.params(pa), self.ast.types.params(pb));
        xa.len() == xb.len()
            && xa.iter().zip(xb).all(|(&p, &q)| self.ast.types.same_type(p, q))
            && (!ma
                || (pa.info().this_quals() == pb.info().this_quals()
                    && pa.info().ref_qual() == pb.info().ref_qual()))
    }

    /// clang's `-Wambiguous-reversed-operator`: a viable reversed
    /// candidate with a better conversion for some operand than the
    /// selected non-reversed candidate `b`, tolerated by
    /// [`Sema::allow_ambiguity`].
    fn diagnose_reversed_ambiguity(
        &mut self,
        set: &OverloadCandidateSet,
        b: u32,
        op: OO,
        spelling: &str,
        types: &[QualType],
        loc: Loc,
    ) {
        let f = set.cand(b).func();
        let best: Vec<Ics> = set.ics_of(b).to_vec();
        let mut with: Vec<DeclId> = Vec::new();
        for c in 0..u32::try_from(set.cands.len()).unwrap_or(u32::MAX) {
            let x = set.cand(c);
            if !(x.viable && x.decl.is_some() && x.rewrite.is_reversed()) {
                continue;
            }
            let g = x.func();
            if !self.allow_ambiguity(g, f) {
                continue;
            }
            let xs = set.ics_of(c);
            if (0..2).any(|k| {
                matches!((xs.get(k), best.get(k)), (Some(p), Some(q))
                    if self.compare_ics(p, q) == IcsCompare::Better)
            }) {
                with.push(g);
            }
        }
        if with.is_empty() {
            return;
        }
        let same = with.len() == 1 && self.ast.first(with[0]) == self.ast.first(f);
        let floc = self.ast.decls.get(f).loc();
        let mut msg = self
            .diag(DiagId::sema_ovl_ambiguous_reversed_operator, loc)
            .arg(spelling)
            .arg(self.ty_arg(types[0]))
            .arg(self.ty_arg(types[1]))
            .arg(u8::from(same));
        if same {
            msg = msg
                .with_note(self.diag(DiagId::note_ovl_ambiguous_oper_binary_reversed_self, floc));
            if op == OO::EqualEqual && self.eqeq_non_const_self(f, types) {
                msg = msg.with_note(
                    self.diag(DiagId::note_ovl_ambiguous_eqeq_reversed_self_non_const, floc),
                );
            }
        } else {
            msg = msg.with_note(
                self.diag(DiagId::note_ovl_ambiguous_oper_binary_selected_candidate, floc),
            );
            for g in with {
                let gl = self.ast.decls.get(g).loc();
                msg = msg.with_note(
                    self.diag(DiagId::note_ovl_ambiguous_oper_binary_reversed_candidate, gl),
                );
            }
        }
        self.emit(msg);
    }

    /// clang's condition for "mark 'operator==' as const": a non-const
    /// member `operator==` of `C` taking a `C` with two `C` operands.
    fn eqeq_non_const_self(&mut self, f: DeclId, types: &[QualType]) -> bool {
        if self.kind_of(f) != DeclKind::CXXMethod
            || self.is_static_method(f)
            || self.has_explicit_object(f)
        {
            return false;
        }
        let (q, _) = self.method_object_quals(f);
        if q.is_const() {
            return false;
        }
        let Some(c) = self.declaring_class(f) else { return false };
        let ct = self.tag_type(c);
        let mut params = Vec::new();
        self.fn_param_types(f, &mut params);
        let Some(&p0) = params.first() else { return false };
        let same = |s: &Self, t: QualType| s.ast.types.same_unqualified(ct, t);
        same(self, self.non_reference(p0)) && same(self, types[0]) && same(self, types[1])
    }

    /// Builds the call of the selected operator function and the rest of
    /// a rewritten comparison ([over.match.oper]/10-11).
    #[allow(clippy::too_many_arguments, reason = "the operator's parts")]
    fn finish_operator(
        &mut self,
        form: OperForm,
        op: OO,
        chosen: Chosen,
        ops: &[ExprId],
        brs: &mut [Option<InitList>],
        written: &[ExprId],
        loc: Loc,
        end: Loc,
    ) -> OperResult {
        let Chosen { f, found, rewrite } = chosen;
        let mut order: Vec<ExprId> = ops.to_vec();
        if rewrite.is_reversed() {
            order.swap(0, 1);
            brs.swap(0, 1);
        }
        let fop = self.ast.decls.get(f).name().as_operator().unwrap_or(op);
        if rewrite != Rewrite::None && fop == OO::EqualEqual {
            let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            let ret = self.ast.types.proto(ty).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
            if !self.ast.types.same_unqualified(ret, QualType::BOOL) {
                let ext =
                    self.ast.types.is_integral(ret) || crate::conv::is_unscoped_enum(self, ret);
                let id = if ext {
                    DiagId::sema_ovl_rewrite_equalequal_not_bool_ext
                } else {
                    DiagId::sema_ovl_rewrite_equalequal_not_bool
                };
                let spelling = match form {
                    OperForm::Binary(b) => b.spelling(),
                    _ => op.spelling(),
                };
                let note = self.diag(DiagId::note_declared_at, self.ast.decls.get(f).loc());
                let msg = self.diag(id, loc).arg(self.ty_arg(ret)).arg(spelling).with_note(note);
                self.emit(msg);
                if !ext {
                    return OperResult::Done(self.recover_err(written, loc));
                }
            }
        }
        let Some(call) = self.build_operator_call(f, found, fop, &order, brs, loc, end) else {
            return OperResult::Done(self.recover_err(written, loc));
        };
        let OperForm::Binary(bop) = form else { return OperResult::Done(call) };
        if rewrite == Rewrite::None {
            return OperResult::Done(call);
        }
        let mut r = call;
        if op == OO::ExclaimEqual && fop == OO::EqualEqual {
            r = self.act_on_unary(UnaryOp::LNot, r, loc);
        } else if fop == OO::Spaceship && fop != op {
            let zero =
                self.builder().int_literal(QualType::INT, 0, kiln_ast::expr::Radix::Decimal, loc);
            r = if rewrite.is_reversed() {
                self.act_on_binary(bop, zero, r, loc)
            } else {
                self.act_on_binary(bop, r, zero, loc)
            };
        }
        if self.is_err_expr(r) {
            return OperResult::Done(r);
        }
        let e = self.builder().rewritten_binary(bop, written[0], written[1], r, loc);
        OperResult::Done(e)
    }

    /// The `CXXOperatorCallExpr` of operator function `f` (found as
    /// `found`, named `op`) with operands `ops` in parameter order,
    /// converting them ([over.match.oper]/10, [over.call.object]).
    #[allow(clippy::too_many_arguments, reason = "the call's parts")]
    pub(crate) fn build_operator_call(
        &mut self,
        f: DeclId,
        found: DeclId,
        op: OO,
        ops: &[ExprId],
        brs: &mut [Option<InitList>],
        loc: Loc,
        end: Loc,
    ) -> Option<ExprId> {
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let proto = self.sugared_proto(fty)?;
        let method = self.kind_of(f) == DeclKind::CXXMethod;
        let implicit_obj = method && !self.is_static_method(f) && !self.has_explicit_object(f);
        let extra = (found != f).then_some(DeclRefExtra {
            qualifier: None,
            template_args: None,
            found_decl: Some(found),
        });
        let callee = self.builder().operator_callee(f, fty, loc, extra);
        let named = Some(CallCallee { decl: f, overloadable: false });
        let rparen = if end.is_valid() { end } else { loc };
        let mut conv = Vec::with_capacity(ops.len());
        if method && !self.has_explicit_object(f) {
            let obj = if implicit_obj {
                self.operator_object(ops[0], f)
            } else {
                // A static operator's object is still evaluated; a class
                // prvalue is materialized (clang, as an lvalue).
                let n = *self.ast.exprs.get(ops[0]);
                if n.value_category() == ValueCategory::PRValue
                    && self.ast.types.as_record(n.ty()).is_some()
                {
                    self.materialize_temporary(ops[0], n.ty(), true)
                } else {
                    ops[0]
                }
            };
            conv.push(obj);
            let rest = self.convert_call_args(
                named,
                callee,
                &ops[1..],
                &mut brs[1..],
                &proto,
                loc,
                rparen,
            )?;
            conv.extend(rest);
        } else {
            conv = self.convert_call_args(named, callee, ops, brs, &proto, loc, rparen)?;
        }
        if method && let Some(c) = self.operand_class(ops[0]) {
            let def = self.ast.definition(c).unwrap_or(c);
            self.check_member_access(found, def, Some(def), loc);
        }
        if self.unevaluated == 0 {
            self.mark_function_referenced(f, loc);
        }
        let (ty, vk) = self.call_result_of(proto.ret(), loc, Some(f), loc);
        if ty == QualType::ERROR {
            return None;
        }
        let e = self.builder().operator_call(op, callee, &conv, ty, vk, loc, end);
        Some(self.maybe_bind_temporary(e))
    }
}
