//! `-Wempty-body` (WP-191): a null statement `;` written on the same line
//! as the `)` of an `if`, `switch`, `for`, range-based `for` or `while`
//! (clang `DiagnoseEmptyStmtBody`, `DiagnoseEmptyLoopBody`;
//! spec/sema.md §7.9).
//!
//! An `if` without `else` (not `if consteval`), a `switch` and a
//! range-based `for` are diagnosed when the statement is finished. Since `for (...);` and
//! `while (...);` are common idioms, a loop is diagnosed only when its
//! enclosing compound statement is finished and the statement following
//! it there is a compound statement or begins in a later column than the
//! loop. Nothing is diagnosed in a template instantiation.

use kiln_ast::stmt::StmtKind;
use kiln_ast::{ExprId, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

/// The kind of statement whose body is checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub enum EmptyBodyStmt {
    /// `if (c) ;` without `else`.
    If,
    /// `switch (c) ;`.
    Switch,
    /// `for (init; cond; inc) ;`.
    For,
    /// `for (decl : range) ;`.
    RangeFor,
    /// `while (c) ;`.
    While,
}

impl EmptyBodyStmt {
    const fn diag(self) -> DiagId {
        match self {
            EmptyBodyStmt::If => DiagId::sema_empty_if_body,
            EmptyBodyStmt::Switch => DiagId::sema_empty_switch_body,
            EmptyBodyStmt::For => DiagId::sema_empty_for_body,
            EmptyBodyStmt::RangeFor => DiagId::sema_empty_range_for_body,
            EmptyBodyStmt::While => DiagId::sema_empty_while_body,
        }
    }
}

impl Sema<'_, '_> {
    /// clang `ShouldDiagnoseEmptyStmtBody`: `body` is a null statement
    /// whose `;` is spelled on the presumed line of `stmt_loc` and is not
    /// preceded by a macro expanding to nothing (`if (c) EMPTY;`). `after`
    /// is the end of the `)` before the body. Returns the `;`.
    fn empty_body_semi(&self, stmt_loc: Loc, after: Loc, body: StmtId) -> Option<Loc> {
        if !self.inst.stack.is_empty() {
            return None;
        }
        let n = self.ast.stmts.get(body);
        if n.kind() != StmtKind::NullStmt {
            return None;
        }
        let semi = n.loc();
        let sm = self.diags.sm();
        let stmt_line = sm.presumed_line_col(stmt_loc)?.line;
        let body_line = sm.line_col(sm.spelling_loc(semi))?.line;
        (stmt_line == body_line && !self.leading_empty_macro(after, semi)).then_some(semi)
    }

    /// clang `NullStmt::hasLeadingEmptyMacro`, read from the source: the
    /// text between the `)` ending at `after` and the `;` holds more than
    /// whitespace and comments, which can only be macro invocations that
    /// expanded to no tokens.
    fn leading_empty_macro(&self, after: Loc, semi: Loc) -> bool {
        let sm = self.diags.sm();
        if sm.is_macro_loc(after) || sm.is_macro_loc(semi) {
            return false;
        }
        let Some(text) = sm.span_text(kiln_source::Span::new(after, semi)) else {
            return false;
        };
        let mut rest = text;
        loop {
            rest = rest.trim_start();
            if let Some(r) = rest.strip_prefix("/*") {
                rest = r.split_once("*/").map_or("", |(_, r)| r);
            } else {
                return !rest.is_empty();
            }
        }
    }

    fn warn_empty_body(&mut self, kind: EmptyBodyStmt, semi: Loc) {
        let note = self.diag(DiagId::note_empty_body_on_separate_line, semi);
        let msg = self.diag(kind.diag(), semi).with_note(note);
        self.emit(msg);
    }

    /// An `if` without `else` or a range-based `for` whose `)` ends at
    /// `rparen`.
    pub fn check_empty_body(&mut self, kind: EmptyBodyStmt, rparen: Loc, body: StmtId) {
        if let Some(semi) = self.empty_body_semi(rparen, rparen, body) {
            self.warn_empty_body(kind, semi);
        }
    }

    /// A `switch` on `cond` whose `)` ends at `rparen` (clang checks the
    /// line of the condition's end).
    pub fn check_empty_switch_body(&mut self, cond: ExprId, rparen: Loc, body: StmtId) {
        let end = self.expr_range(cond).hi;
        if let Some(semi) = self.empty_body_semi(end, rparen, body) {
            self.warn_empty_body(EmptyBodyStmt::Switch, semi);
        }
    }

    /// Records loop `stmt` (`)` at `rparen`) for
    /// [`check_empty_loop_bodies`](Self::check_empty_loop_bodies) when
    /// its body is a null statement on the same line.
    pub fn note_loop_body(&mut self, kind: EmptyBodyStmt, stmt: StmtId, rparen: Loc, body: StmtId) {
        if let Some(semi) = self.empty_body_semi(rparen, rparen, body) {
            self.empty_loop_bodies.insert(stmt, (kind, semi));
        }
    }

    /// clang `DiagnoseEmptyLoopBody` for the statements of a finished
    /// compound statement: each recorded loop followed by a compound
    /// statement or by a statement beginning in a later column.
    pub fn check_empty_loop_bodies(&mut self, stmts: &[StmtId]) {
        if self.empty_loop_bodies.is_empty() {
            return;
        }
        for w in stmts.windows(2) {
            let Some((kind, semi)) = self.empty_loop_bodies.remove(&w[0]) else { continue };
            let next = self.ast.stmts.get(w[1]);
            let probable = next.kind() == StmtKind::CompoundStmt
                || {
                    let sm = self.diags.sm();
                    let col = |l: Loc| sm.presumed_line_col(l).map(|p| p.col);
                    let loop_loc = self.ast.stmts.get(w[0]).loc();
                    matches!((col(self.stmt_begin_loc(w[1])), col(loop_loc)), (Some(b), Some(s)) if b > s)
                };
            if probable {
                self.warn_empty_body(kind, semi);
            }
        }
        if let Some(last) = stmts.last() {
            self.empty_loop_bodies.remove(last);
        }
    }

    /// Where statement `s` begins (clang `Stmt::getBeginLoc`).
    pub(crate) fn stmt_begin_loc(&self, s: StmtId) -> Loc {
        let n = self.ast.stmts.get(s);
        if n.kind() == StmtKind::ExprStmt {
            self.begin_loc(self.ast.stmts.expr_stmt_expr(s))
        } else {
            n.loc()
        }
    }
}
