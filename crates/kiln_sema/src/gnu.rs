//! GNU extensions kiln accepts in C++ (spec/sema.md §6.10): statement
//! expressions `({ ... })`, `__builtin_offsetof` and compound literals
//! `( T ) { ... }`. Each matches clang's behaviour for C++.

use kiln_ast::stmt::StmtKind;
use kiln_ast::{ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::{self, ConvCtx};

/// The state saved across a statement expression
/// ([`Sema::act_on_start_stmt_expr`]): the enclosing full-expression's
/// pending potential odr-uses, which the statements inside must not
/// resolve.
#[derive(Debug)]
pub struct StmtExprScope {
    outer_odr: Vec<ExprId>,
}

impl Sema<'_, '_> {
    /// Whether a statement expression may appear here: inside a function
    /// body (clang's `getFnParent()`; `err_stmtexpr_file_scope`
    /// otherwise, reported at the `(`, which is then the caller's to
    /// recover from). Returns `false` after diagnosing.
    pub fn check_stmt_expr_allowed(&self, lparen: Loc) -> bool {
        if self.func.is_some() {
            return true;
        }
        self.error(DiagId::sema_stmtexpr_file_scope, lparen);
        false
    }

    /// The `({` of a statement expression: its statements are separate
    /// full-expressions, so the enclosing one's pending odr-uses are set
    /// aside until [`Sema::act_on_stmt_expr`].
    pub fn act_on_start_stmt_expr(&mut self) -> StmtExprScope {
        StmtExprScope { outer_odr: std::mem::take(&mut self.pending_odr) }
    }

    /// The expression statement that ends a statement expression (clang's
    /// `ActOnStmtExprResult`): its value is the statement expression's.
    /// The usual array/function decay applies; a non-`void` result is
    /// then copy-initialized into a prvalue of its cv-unqualified type
    /// (an lvalue-to-rvalue conversion for a scalar). It is not
    /// discarded, so no unused-value warning is given.
    pub fn act_on_stmt_expr_result(&mut self, e: ExprId) -> StmtId {
        let e = self.stmt_expr_result_value(e);
        let e = self.act_on_full_expr(e);
        self.builder().expr_stmt(e)
    }

    fn stmt_expr_result_value(&mut self, e: ExprId) -> ExprId {
        if self.is_err_expr(e) {
            return e;
        }
        let e = conv::decay(self, e);
        let n = *self.ast.exprs.get(e);
        let ty = n.ty();
        if self.ast.types.is_void(ty) || self.ast.types.is_dependent(ty) {
            return e;
        }
        if self.ast.types.as_record(ty).is_some() {
            if n.value_category().is_glvalue() {
                // Copying a class glvalue needs its copy constructor.
                self.unsupported("a statement expression with a class lvalue result", n.loc());
                return self.recover(ty, &[e], n.loc());
            }
            return e;
        }
        let to = conv::strip_cv(self, ty);
        self.convert_for_init(e, to, ConvCtx::Initialization)
    }

    /// `( compound-statement )` (a GNU statement expression; clang's
    /// `ActOnStmtExpr`). Its type is that of the last statement that is
    /// not a null statement, looking through labels, when that is an
    /// expression statement, else `void`; it is always a prvalue. `lparen`
    /// and `rparen` are the parentheses.
    pub fn act_on_stmt_expr(
        &mut self,
        scope: StmtExprScope,
        body: StmtId,
        lparen: Loc,
        rparen: Loc,
    ) -> ExprId {
        let inner = std::mem::replace(&mut self.pending_odr, scope.outer_odr);
        self.pending_odr.extend(inner);
        let ty = self.stmt_expr_result(body).map_or(QualType::VOID, |e| self.ast.exprs.get(e).ty());
        self.builder().stmt_expr(body, ty, lparen, rparen)
    }

    /// The expression whose value a statement expression with body `body`
    /// yields (clang's `getStmtExprResult` + `ValueStmt::getExprStmt`).
    pub(crate) fn stmt_expr_result(&self, body: StmtId) -> Option<ExprId> {
        let stmts = &self.ast.stmts;
        let children = self.ast.lists.get(stmts.compound_body(body));
        let last = children
            .iter()
            .rev()
            .map(|&s| StmtId::new(s))
            .find(|&s| stmts.get(s).kind() != StmtKind::NullStmt)
            .or_else(|| children.last().map(|&s| StmtId::new(s)))?;
        self.value_stmt_expr(last)
    }

    /// The expression of an expression statement, looking through labels
    /// and attributes.
    fn value_stmt_expr(&self, s: StmtId) -> Option<ExprId> {
        let stmts = &self.ast.stmts;
        let mut cur = s;
        for _ in 0..1024 {
            let n = stmts.get(cur);
            match n.kind() {
                StmtKind::ExprStmt => return Some(stmts.expr_stmt_expr(cur)),
                StmtKind::LabelStmt | StmtKind::AttributedStmt => cur = StmtId::new(n.b()),
                _ => return None,
            }
        }
        None
    }

    /// clang's `isUnusedResultAWarning` for a `StmtExpr` at `loc` (its
    /// `(`): the warning for its last statement when that is an expression
    /// (directly or under one label), else a warning unless it is `void`.
    pub(crate) fn unused_stmt_expr(
        &self,
        e: ExprId,
        loc: Loc,
    ) -> Option<(DiagId, Loc, Vec<String>)> {
        let stmts = &self.ast.stmts;
        let body = self.ast.exprs.stmt_expr_body(e);
        let children = self.ast.lists.get(stmts.compound_body(body));
        if let Some(&last) = children.last() {
            let mut s = StmtId::new(last);
            if stmts.get(s).kind() == StmtKind::LabelStmt {
                s = StmtId::new(stmts.get(s).b());
            }
            if stmts.get(s).kind() == StmtKind::ExprStmt {
                return self.unused_warning(stmts.expr_stmt_expr(s));
            }
        }
        (!self.ast.types.is_void(self.ast.exprs.get(e).ty())).then_some((
            DiagId::sema_unused_expr,
            loc,
            Vec::new(),
        ))
    }
}
