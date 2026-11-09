//! Statement and expression locations (clang `EmitStopPoint`,
//! `LexicalScope`, `ApplyDebugLocation(CGF, const Expr *)`).

use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{ExprId, StmtId};
use kiln_llvm::MetadataId;
use kiln_source::Loc;

use super::DbgSaved;
use crate::module::Cg;
use crate::stmt::Scope;

impl Cg<'_> {
    /// clang `Expr::getExprLoc` (`kiln_sema` `expr_loc`).
    pub(crate) fn di_expr_loc(&self, mut e: ExprId) -> Loc {
        use kiln_ast::names::OverloadedOperator as O;
        let (ast, exprs) = (self.ast, &self.ast.exprs);
        for _ in 0..256 {
            match exprs.get(e).kind() {
                ExprKind::ArraySubscriptExpr | ExprKind::CXXMemberCallExpr => {
                    e = exprs.children(e, &ast.lists)[0];
                }
                ExprKind::UnaryOperator
                | ExprKind::BinaryOperator
                | ExprKind::CompoundAssignOperator
                | ExprKind::MemberExpr
                | ExprKind::CXXRewrittenBinaryOperator => return exprs.get(e).loc(),
                ExprKind::CXXOperatorCallExpr
                    if !matches!(
                        exprs.operator_call_op(e),
                        O::PlusPlus
                            | O::MinusMinus
                            | O::Arrow
                            | O::Call
                            | O::Subscript
                            | O::New
                            | O::Delete
                            | O::ArrayNew
                            | O::ArrayDelete
                    ) =>
                {
                    return exprs.get(e).loc();
                }
                _ => return ast.begin_loc(e),
            }
        }
        exprs.get(e).loc()
    }

    /// clang `ApplyDebugLocation DL(CGF, E)` at an expression emitter's
    /// `Visit`; pass the result to [`Cg::dbg_restore`].
    #[inline]
    pub(crate) fn dbg_apply_expr(&mut self, e: ExprId) -> Option<DbgSaved> {
        if !self.dbg_on() {
            return None;
        }
        let loc = self.di_expr_loc(e);
        self.dbg_apply(loc)
    }

    /// clang `ApplyDebugLocation::CreateEmpty`.
    pub(crate) fn dbg_apply_empty(&mut self) -> Option<DbgSaved> {
        if !self.dbg_on() {
            return None;
        }
        let orig = self.f.fir.dbg.as_deref()?.loc;
        self.f.fir.set_dbg_loc(None);
        Some(DbgSaved(orig))
    }

    /// clang `Stmt::getBeginLoc`.
    pub(crate) fn di_stmt_begin(&self, s: StmtId) -> Loc {
        let ast = self.ast;
        match ast.stmts.get(s).kind() {
            StmtKind::ExprStmt => ast.begin_loc(ast.stmts.expr_stmt_expr(s)),
            // `CoroutineBodyStmt::getBeginLoc`: its body's.
            StmtKind::CoroutineBodyStmt => self.di_stmt_begin(ast.stmts.coroutine_body(s).body),
            _ => ast.stmts.get(s).loc(),
        }
    }

    /// clang `Stmt::getEndLoc` where the AST keeps it: a compound
    /// statement's `}`, through the statements ending in a
    /// substatement; invalid otherwise (WP-814 follow-up: the AST keeps
    /// no other end locations).
    pub(crate) fn di_stmt_end(&self, s: StmtId) -> Loc {
        let ast = self.ast;
        match ast.stmts.get(s).kind() {
            StmtKind::CompoundStmt => ast.stmts.rbrace(s),
            StmtKind::IfStmt => {
                let d = ast.stmts.if_data(s);
                d.els.or(d.then).map_or(Loc::INVALID, |b| self.di_stmt_end(b))
            }
            StmtKind::WhileStmt | StmtKind::ForStmt => {
                ast.stmts.loop_data(s).body.map_or(Loc::INVALID, |b| self.di_stmt_end(b))
            }
            StmtKind::DoStmt => ast.stmts.do_rparen(s),
            StmtKind::CXXForRangeStmt => self.di_stmt_end(ast.stmts.range_for_data(s).body),
            StmtKind::ExprStmt => self.di_expr_end(ast.stmts.expr_stmt_expr(s)),
            StmtKind::ReturnStmt => ast
                .stmts
                .return_value(s)
                .map_or_else(|| ast.stmts.get(s).loc(), |e| self.di_expr_end(e)),
            _ => Loc::INVALID,
        }
    }

    /// clang `Expr::getEndLoc`: the stored end location, else the end of
    /// the last operand, else the expression's own (single-token) location.
    pub(crate) fn di_expr_end(&self, mut e: ExprId) -> Loc {
        let (ast, exprs) = (self.ast, &self.ast.exprs);
        for _ in 0..256 {
            if let Some(l) = exprs.end_loc(e) {
                return l;
            }
            e = match exprs.get(e).kind() {
                ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator => {
                    exprs.children(e, &ast.lists)[1]
                }
                ExprKind::UnaryOperator if !exprs.unary_op(e).is_postfix() => {
                    exprs.children(e, &ast.lists)[0]
                }
                ExprKind::ImplicitCastExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr => exprs.children(e, &ast.lists)[0],
                // `CoroutineSuspendExpr::getEndLoc`: the operand's.
                ExprKind::CoawaitExpr | ExprKind::CoyieldExpr => {
                    exprs.coawait_operand(e, &ast.lists)
                }
                _ => return exprs.get(e).loc(),
            };
        }
        exprs.get(e).loc()
    }

    /// clang `EmitExplicitCastExprType`: an explicit cast's type is kept
    /// in the unit's retained types.
    pub(crate) fn di_explicit_cast(&mut self, e: ExprId) {
        if self.dbg_on() && self.di_reduced() && self.ast.exprs.get(e).kind().is_explicit_cast() {
            let q = self.ast.exprs.get(e).ty();
            if let Some(t) = self.di_type(q)
                && let Some(d) = self.di.as_deref_mut()
                && !d.retained.contains(&t)
            {
                d.retained.push(t);
            }
        }
    }

    /// clang `EmitStopPoint(S)` for statement `s`.
    pub(crate) fn dbg_stmt(&mut self, s: StmtId) {
        if self.dbg_on() {
            let loc = self.di_stmt_begin(s);
            self.dbg_stop_point(loc);
        }
    }

    /// `EmitStopPoint` of an expression emitted as a statement (a loop
    /// increment).
    pub(crate) fn dbg_expr_stmt(&mut self, e: ExprId) {
        if self.dbg_on() {
            let loc = self.ast.begin_loc(e);
            self.dbg_stop_point(loc);
        }
    }

    /// A `LexicalScope` over `[begin, end]`.
    pub(crate) fn begin_lexical(&mut self, begin: Loc, end: Loc) -> Scope {
        let mut sc = self.begin_scope(true);
        if self.dbg_block_start(begin) {
            sc.dbg_end = Some(end);
        }
        sc
    }

    /// The loop locations of clang `LoopStack.push` (`SourceLocToDebugLoc`
    /// of the statement's begin and end).
    pub(crate) fn di_loop_locs(&mut self, s: StmtId) -> Option<[MetadataId; 2]> {
        let scope = self.f.fir.dbg.as_deref()?.scope();
        let (b, e) = (self.ast.stmts.get(s).loc(), self.di_stmt_end(s));
        let (bl, bc) = self.di_line_col(b);
        let (el, ec) = self.di_line_col(e);
        Some([self.di_location_at(bl, bc, scope), self.di_location_at(el, ec, scope)])
    }

    /// clang `EmitReturnStmt`'s `NumReturnExprs` / `NumSimpleReturnExprs`.
    pub(crate) fn dbg_count_return(&mut self, rv: Option<ExprId>) {
        let simple = rv.is_none_or(|e| self.folds.get(e).is_some());
        if let Some(d) = self.f.fir.dbg.as_deref_mut() {
            d.returns += 1;
            d.simple_returns += u32::from(simple);
        }
    }
}
