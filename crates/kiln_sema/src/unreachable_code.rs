//! `-Wunreachable-code` and its subgroups (clang `ReachableCode.cpp`;
//! spec/sema.md §7.10).
//!
//! The walk is [`Sema::dead_scan`]: it follows control through a body
//! the way clang's CFG does, with constant branch conditions pruning
//! their dead edge unless they are configuration values
//! ([`Sema::is_configuration_value`]). Each round reports one dead root,
//! the last dead element (in evaluation order) that no reached or dead
//! element flows into, and treats it as reached for the next round, so
//! the regions come out last first, as clang scans its blocks in reverse.

use std::collections::HashSet;

use kiln_ast::decl::{ConstexprKind, DeclKind};
use kiln_ast::expr::{BinaryOp, ExprKind, UnaryOp};
use kiln_ast::stmt::StmtKind;
use kiln_ast::{ExprId, StmtId};
use kiln_diag::{DiagId, FixIt};
use kiln_source::Loc;

use crate::Sema;
use crate::try_stmt::reach::{DeadScan, Unit};

/// clang `reachable_code::UnreachableKind`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Other,
    Break,
    Return,
}

impl Sema<'_, '_> {
    /// Reports the unreachable code of a function body (clang
    /// `CheckUnreachable`). Not in an instantiation (the caller already
    /// skips dependent contexts and bodies after an error).
    pub(crate) fn check_unreachable_code(&mut self, body: StmtId) {
        if !self.inst.stack.is_empty() {
            return;
        }
        let ids = [
            DiagId::sema_unreachable_code,
            DiagId::sema_unreachable_code_break,
            DiagId::sema_unreachable_code_return,
            DiagId::sema_unreachable_loop_increment,
        ];
        if !ids.iter().any(|&d| self.diags.is_enabled(d)) {
            return;
        }
        let mut roots: HashSet<Unit> = HashSet::new();
        let mut prev_note: Option<ExprId> = None;
        // Each round roots at least one more element.
        loop {
            let Some(scan) = self.dead_scan(body, roots.clone()) else { return };
            let dead: Vec<Unit> =
                scan.order.iter().copied().filter(|u| !scan.live.contains(u)).collect();
            let Some(&first) = dead.first() else { return };
            let mut all = roots.clone();
            all.extend(dead.iter().copied());
            let Some(fed) = self.dead_scan(body, all) else { return };
            let pick =
                dead.iter().rev().copied().find(|u| !fed.has_pred.contains(u)).unwrap_or(first);
            roots.insert(pick);
            self.report_dead(pick, &scan, &mut prev_note);
        }
    }

    /// clang `DeadCodeScan::reportDeadCode` and Sema's
    /// `UnreachableCodeHandler`.
    fn report_dead(&mut self, u: Unit, scan: &DeadScan, prev_note: &mut Option<ExprId>) {
        let loc = self.unreachable_loc(u);
        if loc.is_macro() || self.unit_begin(u).is_macro() {
            return;
        }
        let kind = if matches!(u, Unit::Stmt(s) if self.ast.stmts.get(s).kind() == StmtKind::BreakStmt)
        {
            Kind::Break
        } else if self.is_trivial_do_cond(u, scan) || self.is_builtin_unreachable_ref(u) {
            return;
        } else if self.is_dead_return(u, scan) {
            Kind::Return
        } else {
            Kind::Other
        };
        let fallthrough = matches!(u, Unit::Stmt(s)
            if self.ast.stmts.get(s).kind() == StmtKind::AttributedStmt && self.ift_is_fallthrough(s));
        let mut note = None;
        if kind == Kind::Other {
            if let Some(&l) = scan.inc.get(&u) {
                let at = match self.ast.stmts.get(l).kind() {
                    StmtKind::ForStmt => self.ast.stmts.loop_data(l).inc.map(|i| self.begin_loc(i)),
                    _ => None,
                }
                .unwrap_or_else(|| self.ast.stmts.get(l).loc());
                // clang passes the increment as the silencing range: a
                // repeat of the previous one is dropped.
                if fallthrough && self.diags.is_enabled(DiagId::sema_unreachable_fallthrough_attr) {
                    return;
                }
                let d = self.diag(DiagId::sema_unreachable_loop_increment, at);
                self.emit(d);
                return;
            }
            if let Some(&c) = scan.pruned.get(&u) {
                self.is_configuration_value(c, &mut note);
            }
        }
        if fallthrough && self.diags.is_enabled(DiagId::sema_unreachable_fallthrough_attr) {
            return;
        }
        if note.is_some() && *prev_note == note {
            return;
        }
        *prev_note = note;
        let id = match kind {
            Kind::Other => DiagId::sema_unreachable_code,
            Kind::Break => DiagId::sema_unreachable_code_break,
            Kind::Return => DiagId::sema_unreachable_code_return,
        };
        let mut d = self.diag(id, loc);
        if let Some(n) = note {
            let open = self.begin_loc(n);
            // The note range is a literal, possibly under `!` or `-`.
            let mut lit = n;
            while self.ast.exprs.get(lit).kind() == ExprKind::UnaryOperator {
                lit = self.ignore_implicit_casts(self.ast.exprs.operand(lit));
            }
            // clang emits the note only when the literal's end maps to the
            // file, and drops both fix-its when the opening one does not
            // (`macro_fixit.rs`).
            if let Some(close) = self.file_token_end(self.ast.exprs.get(lit).loc()) {
                let mut note = self.diag(DiagId::note_unreachable_silence, open);
                let fixits =
                    vec![FixIt::insert(open, "/* DISABLES CODE */ ("), FixIt::insert(close, ")")];
                for f in self.file_fixits(fixits) {
                    note = note.fixit(f);
                }
                d = d.with_note(note);
            }
        }
        self.emit(d);
    }

    /// Where element `u` begins.
    fn unit_begin(&self, u: Unit) -> Loc {
        match u {
            Unit::Stmt(s) => self.ast.stmts.get(s).loc(),
            Unit::Expr(e) => self.begin_loc(e),
        }
    }

    /// clang `GetUnreachableLoc`: an operator's token, a member's name,
    /// otherwise where the element begins.
    fn unreachable_loc(&self, u: Unit) -> Loc {
        let Unit::Expr(e) = u else { return self.unit_begin(u) };
        match self.ast.exprs.get(e).kind() {
            ExprKind::BinaryOperator
            | ExprKind::CompoundAssignOperator
            | ExprKind::UnaryOperator
            | ExprKind::ConditionalOperator
            | ExprKind::BinaryConditionalOperator
            | ExprKind::MemberExpr
            | ExprKind::ArraySubscriptExpr
            | ExprKind::CStyleCastExpr => self.ast.exprs.get(e).loc(),
            _ => self.begin_loc(e),
        }
    }

    /// clang `isTrivialDoWhile`: the root is a `do` loop's literal or
    /// enumerator condition (`do { ... } while (0)`).
    fn is_trivial_do_cond(&self, u: Unit, scan: &DeadScan) -> bool {
        let Unit::Expr(e) = u else { return false };
        let core = self.ignore_paren_imp_casts(e);
        if !scan.do_conds.contains(&core) && !scan.do_conds.contains(&e) {
            return false;
        }
        match self.ast.exprs.get(core).kind() {
            ExprKind::IntegerLiteral
            | ExprKind::StringLiteral
            | ExprKind::CXXBoolLiteralExpr
            | ExprKind::CharacterLiteral => true,
            ExprKind::DeclRefExpr => {
                self.kind_of(self.ast.exprs.decl(core)) == DeclKind::EnumConstant
            }
            _ => false,
        }
    }

    /// clang `isBuiltinUnreachable`: the root names `__builtin_unreachable`.
    fn is_builtin_unreachable_ref(&self, u: Unit) -> bool {
        let Unit::Expr(e) = u else { return false };
        self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr && {
            let d = self.ast.exprs.decl(e);
            self.kind_of(d) == DeclKind::Function && self.decl_name(d) == "__builtin_unreachable"
        }
    }

    /// clang `isDeadReturn`: the root is a `return` statement, or part of
    /// a returned value whose evaluation does not branch (the root's CFG
    /// block then ends in the `return`).
    fn is_dead_return(&self, u: Unit, scan: &DeadScan) -> bool {
        match u {
            Unit::Stmt(s) => {
                matches!(self.ast.stmts.get(s).kind(), StmtKind::ReturnStmt)
            }
            Unit::Expr(_) => {
                let Some(&rs) = scan.ret.get(&u) else { return false };
                self.ast.stmts.get(rs).kind() == StmtKind::ReturnStmt
                    && self.ast.stmts.return_value(rs).is_some_and(|v| !self.branches(v))
            }
        }
    }

    /// Whether evaluating `e` branches (`?:`, `&&`, `||`, a statement
    /// expression or a `throw`); lambda bodies do not count.
    fn branches(&self, e: ExprId) -> bool {
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            match self.ast.exprs.get(x).kind() {
                ExprKind::ConditionalOperator
                | ExprKind::BinaryConditionalOperator
                | ExprKind::StmtExpr
                | ExprKind::CXXThrowExpr => return true,
                ExprKind::BinaryOperator
                    if matches!(self.ast.exprs.binary_op(x), BinaryOp::LAnd | BinaryOp::LOr) =>
                {
                    return true;
                }
                ExprKind::LambdaExpr | ExprKind::UnaryExprOrTypeTraitExpr => {}
                _ => stack.extend(self.ast.exprs.children(x, &self.ast.lists)),
            }
        }
        false
    }

    /// clang `isConfigurationValue`: whether the constant condition `e`
    /// looks like configuration (a constexpr call, a global or `const`
    /// local, `sizeof`, a parenthesized or macro-expanded literal), whose
    /// dead branch is not reported. `note` receives the first bare
    /// literal met, where the silencing note goes.
    pub(crate) fn is_configuration_value(&self, e: ExprId, note: &mut Option<ExprId>) -> bool {
        self.config_value(e, note, true, false)
    }

    fn config_value(
        &self,
        e: ExprId,
        note: &mut Option<ExprId>,
        mut ints: bool,
        parens: bool,
    ) -> bool {
        let e = self.ignore_implicit_casts(e);
        let ex = &self.ast.exprs;
        let n = ex.get(e);
        match n.kind() {
            ExprKind::ParenExpr if !n.loc().is_macro() => {
                self.config_value(ex.operand(e), note, ints, true)
            }
            ExprKind::CallExpr => self.call_is_constexpr(e),
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => self.config_decl(ex.decl(e)),
            ExprKind::IntegerLiteral | ExprKind::CXXBoolLiteralExpr => {
                if !ints {
                    return false;
                }
                if note.is_none() {
                    *note = Some(e);
                }
                parens || n.loc().is_macro()
            }
            ExprKind::UnaryExprOrTypeTraitExpr => true,
            ExprKind::BinaryOperator => {
                let op = ex.binary_op(e);
                ints &= matches!(
                    op,
                    BinaryOp::LAnd
                        | BinaryOp::LOr
                        | BinaryOp::EQ
                        | BinaryOp::NE
                        | BinaryOp::LT
                        | BinaryOp::GT
                        | BinaryOp::LE
                        | BinaryOp::GE
                );
                let kids = ex.children(e, &self.ast.lists);
                self.config_value(kids[0], note, ints, false)
                    || self.config_value(kids[1], note, ints, false)
            }
            ExprKind::UnaryOperator if matches!(ex.unary_op(e), UnaryOp::LNot | UnaryOp::Minus) => {
                let sub = ex.operand(e);
                let before = *note;
                let r = self.config_value(sub, note, ints, false);
                if before.is_none() && note.is_some() {
                    // clang widens the range to the operator when it was
                    // just set to its operand.
                    if *note == Some(self.ignore_implicit_casts(sub)) {
                        *note = Some(e);
                    }
                }
                r
            }
            _ => false,
        }
    }

    /// clang `isConfigurationValue(const ValueDecl *)`.
    fn config_decl(&self, d: kiln_ast::DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::EnumConstant => self
                .ast
                .decls
                .enumerator(d)
                .and_then(|x| x.init)
                .is_some_and(|i| self.config_value(i, &mut None, true, false)),
            DeclKind::Var | DeclKind::ParmVar => {
                !self.has_local_storage(d)
                    || self.ast.decls.get(d).ty().is_some_and(kiln_ast::QualType::has_const_bit)
            }
            _ => false,
        }
    }

    /// The callee of call `c` is `constexpr` or `consteval`.
    fn call_is_constexpr(&self, c: ExprId) -> bool {
        let ex = &self.ast.exprs;
        let mut r = ex.callee(c);
        for _ in 0..64 {
            match ex.get(r).kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ParenExpr => r = ex.operand(r),
                ExprKind::DeclRefExpr | ExprKind::MemberExpr => {
                    let f = ex.decl(r);
                    return self
                        .ast
                        .decls
                        .function(f)
                        .is_some_and(|x| x.spec.constexpr_kind() != ConstexprKind::None);
                }
                _ => return false,
            }
        }
        false
    }

    /// clang `IgnoreImplicit` then `IgnoreCasts` (parentheses kept).
    fn ignore_implicit_casts(&self, mut e: ExprId) -> ExprId {
        let ex = &self.ast.exprs;
        for _ in 0..256 {
            e = match ex.get(e).kind() {
                ExprKind::ImplicitCastExpr
                | ExprKind::CStyleCastExpr
                | ExprKind::CXXFunctionalCastExpr
                | ExprKind::CXXStaticCastExpr
                | ExprKind::CXXConstCastExpr
                | ExprKind::CXXReinterpretCastExpr
                | ExprKind::CXXDynamicCastExpr
                | ExprKind::BuiltinBitCastExpr
                | ExprKind::ConstantExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr => match ex.children(e, &self.ast.lists).first() {
                    Some(&c) => c,
                    None => return e,
                },
                ExprKind::SubstNonTypeTemplateParmExpr => ex.subst_replacement(e),
                _ => return e,
            };
        }
        e
    }
}
