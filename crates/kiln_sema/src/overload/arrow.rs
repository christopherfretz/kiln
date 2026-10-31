//! `->` on a class object ([over.ref]; spec/sema-overload.md §5): the
//! `operator->` calls applied until the result is not a class (clang
//! `ActOnStartCXXMemberReference`, `BuildOverloadedArrowExpr`), with
//! cycle detection and clang's default depth limit.

use kiln_ast::names::{DeclName, OverloadedOperator as OO};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::fx::FxHashSet;

use super::{OvlOutcome, Rewrite, SetKind};
use crate::Sema;
use crate::lookup::LookupKind;

/// clang's default `-foperator-arrow-depth` ([implimits]); kiln has no
/// option to change it yet.
pub(crate) const ARROW_DEPTH: usize = 256;

/// clang `noteOperatorArrows`: at most this many notes, one of them a
/// "skipping" note.
const NOTE_LIMIT: usize = 9;

/// What [`Sema::arrow_base`] decided for `e->m`.
pub(crate) enum ArrowBase {
    /// The base for the built-in `->`: `e` itself if not a class, else
    /// the last `operator->` call.
    Arrow(ExprId),
    /// The class `e` has no `operator->`: diagnosed with a suggestion;
    /// recover as `e.m`.
    Dot,
    /// Diagnosed.
    Err,
}

/// The `operator->` outcome for one class object.
enum Step {
    Call(ExprId, DeclId),
    /// The class declares no `operator->`.
    NotFound,
    Err,
}

impl Sema<'_, '_> {
    /// The base of `base->m` ([over.ref]/1): while it has class type,
    /// `x->m` is `(x.operator->())->m`.
    pub(crate) fn arrow_base(&mut self, base: ExprId, op_loc: Loc) -> ArrowBase {
        let start = self.ety(base);
        let mut seen: FxHashSet<QualType> = FxHashSet::default();
        seen.insert(self.ast.types.canonical(start));
        let mut calls: Vec<(DeclId, QualType)> = Vec::new();
        let mut cur = base;
        loop {
            let t = self.ety(cur);
            if self.ast.types.as_record(t).is_none() {
                return ArrowBase::Arrow(cur);
            }
            if calls.len() >= ARROW_DEPTH {
                let depth = ARROW_DEPTH.to_string();
                let mut msg = self
                    .diag(DiagId::sema_operator_arrow_depth_exceeded, op_loc)
                    .arg(self.ty_arg(start))
                    .arg(depth);
                for n in self.operator_arrow_notes(&calls) {
                    msg = msg.with_note(n);
                }
                let n = self.diag(DiagId::note_operator_arrow_depth, op_loc);
                self.emit(msg.with_note(n));
                return ArrowBase::Err;
            }
            match self.overloaded_arrow(cur, op_loc) {
                Step::Call(e, f) => {
                    let rt = self.ety(e);
                    calls.push((f, rt));
                    cur = e;
                    if !seen.insert(self.ast.types.canonical(rt)) {
                        let mut msg = self.diag(DiagId::sema_circular_arrow, op_loc);
                        for n in self.operator_arrow_notes(&calls) {
                            msg = msg.with_note(n);
                        }
                        self.emit(msg);
                        return ArrowBase::Err;
                    }
                }
                Step::NotFound if calls.is_empty() => {
                    let msg = self
                        .diag(DiagId::sema_member_reference_suggestion, op_loc)
                        .arg(self.ty_arg(t))
                        .arg("1");
                    self.emit(msg);
                    return ArrowBase::Dot;
                }
                Step::NotFound => {
                    let mut msg = self
                        .diag(DiagId::sema_operator_arrow_not_pointer, op_loc)
                        .arg(self.ty_arg(t));
                    if let Some(&(f, _)) = calls.last() {
                        let at = self
                            .ast
                            .decls
                            .cold
                            .begin_loc
                            .get(f)
                            .copied()
                            .unwrap_or(self.ast.decls.get(f).loc());
                        msg = msg.with_note(
                            self.diag(DiagId::note_member_reference_arrow_from_operator_arrow, at),
                        );
                    }
                    self.emit(msg);
                    return ArrowBase::Err;
                }
                Step::Err => return ArrowBase::Err,
            }
        }
    }

    /// clang `noteOperatorArrows`.
    fn operator_arrow_notes(&self, calls: &[(DeclId, QualType)]) -> Vec<Diag> {
        let (skip_start, skip) = if calls.len() > NOTE_LIMIT {
            ((NOTE_LIMIT - 1).div_ceil(2), calls.len() - (NOTE_LIMIT - 1))
        } else {
            (calls.len(), 0)
        };
        let mut out = Vec::new();
        let mut i = 0;
        while i < calls.len() {
            let (f, rt) = calls[i];
            let at = self.ast.decls.get(f).loc();
            if i == skip_start {
                out.push(
                    self.diag(DiagId::note_operator_arrows_suppressed, at)
                        .arg(skip.to_string())
                        .arg(if skip == 1 { "0" } else { "1" }),
                );
                i += skip;
            } else {
                out.push(self.diag(DiagId::note_operator_arrow_here, at).arg(self.ty_arg(rt)));
                i += 1;
            }
        }
        out
    }

    /// `base.operator->()` (clang `BuildOverloadedArrowExpr`): the member
    /// candidates only ([over.ref]/1, [over.match.oper]/3.1).
    fn overloaded_arrow(&mut self, base: ExprId, loc: Loc) -> Step {
        let mut set = self.begin_set(SetKind::Normal, loc);
        set.oper = true;
        set.note_extra_args = 1;
        set.object = Some(self.arg_info(base));
        let mark = self.push_call_args(&mut set, &[], &[]);
        self.add_member_opers(&mut set, OO::Arrow, &[base], Rewrite::None, loc);
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
            return Step::Err;
        }
        let t = self.ety(base);
        let msg = match out {
            OvlOutcome::Success(b) => {
                let c = set.cand(b);
                let (f, found) = (c.func(), c.found);
                self.end_set(set);
                return match self.build_operator_call(
                    f,
                    found,
                    OO::Arrow,
                    &[base],
                    &mut [None],
                    loc,
                    Loc::INVALID,
                ) {
                    Some(e) => Step::Call(e, f),
                    None => Step::Err,
                };
            }
            OvlOutcome::NoViable if set.cands.is_empty() && !self.declares_arrow(t) => {
                self.end_set(set);
                return Step::NotFound;
            }
            OvlOutcome::NoViable => {
                let m = self.diag(DiagId::sema_ovl_no_viable_oper, loc).arg("operator->");
                self.with_ovl_notes(m, &set, false)
            }
            OvlOutcome::Ambiguous => {
                let m = self
                    .diag(DiagId::sema_ovl_ambiguous_oper_unary, loc)
                    .arg("->")
                    .arg(self.ty_arg(t));
                self.with_ovl_notes(m, &set, true)
            }
            OvlOutcome::Deleted(b) => {
                let f = set.cand(b).func();
                let m = self
                    .diag(DiagId::sema_ovl_deleted_oper, loc)
                    .arg("->")
                    .arg(self.delete_message_suffix(f));
                self.with_ovl_notes(m, &set, false)
            }
        };
        self.emit(msg);
        self.end_set(set);
        Step::Err
    }

    /// Whether class type `t` declares an `operator->` at all (clang
    /// adds invalid declarations as candidates, so a class whose only
    /// `operator->` is ill-formed reports no viable operator rather than
    /// suggesting `.`).
    fn declares_arrow(&mut self, t: QualType) -> bool {
        let Some(c) = self.ast.types.as_record(t) else { return false };
        let Some(def) = self.ast.definition(c) else { return false };
        !self
            .lookup_in_class_dn(def, DeclName::operator(OO::Arrow), LookupKind::Ordinary)
            .decls
            .is_empty()
    }
}
