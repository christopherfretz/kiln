//! Register locals (WP-1908; spec/oven.md "Register locals"): the parts of
//! the compile-time escape test that look past one expression.
//!
//! The pre-scan (`Scan` in `compile.rs`) keeps a local in a register unless
//! it sees its address. Two refinements live here:
//!
//! - [`std_cast_arg`]: the operand of a call of `std::move`, `forward`,
//!   `forward_like`, `move_if_noexcept` or `as_const` is the operand's own
//!   lvalue (clang's `isStdBuiltin`, no call), so the call is transparent
//!   to the scan, as parentheses are: `b = std::move(t)` reads `t`, it does
//!   not bind a reference to it. `addressof` and `__addressof` are not in
//!   the set: their result is the address.
//! - [`assigned_first`]: a scalar declared without an initializer
//!   (`size_type n;`) is in a register only when it is certainly assigned
//!   before any other use and no jump can enter its scope past the
//!   declaration. Otherwise it is in memory, whose per-byte initialized
//!   bitmap and scope release diagnose a read of it before its assignment,
//!   in a later iteration of a loop or after a jump past it
//!   ([expr.const]/5.11, [stmt.dcl]/3), as before.
//!
//! Both are conservative: a construct they do not know means memory.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::stmt::StmtKind;
use kiln_ast::{Ast, DeclId, ExprId, StmtId};

use super::{holding_vars, is_std_builtin, is_std_function, skip_parens};

/// The direct callee of call `e`, as `Compiler::call_inner` finds it
/// (through parentheses, function-to-pointer decay and no-op casts).
pub(super) fn direct_callee(ast: &Ast, e: ExprId) -> Option<DeclId> {
    let mut c = skip_parens(ast, ast.exprs.callee(e));
    while ast.exprs.get(c).kind().is_cast()
        && matches!(
            ast.exprs.cast_kind(c),
            CastKind::FunctionToPointerDecay | CastKind::BuiltinFnToFnPtr | CastKind::NoOp
        )
    {
        c = skip_parens(ast, ast.exprs.operand(c));
    }
    (ast.exprs.get(c).kind() == ExprKind::DeclRefExpr
        && ast.decls.get(ast.exprs.decl(c)).kind().is_function())
    .then(|| ast.exprs.decl(c))
}

/// The operand of `e` when `e` is a glvalue call of one of clang's
/// standard-library value-category casts (`std::move`, `forward`,
/// `forward_like`, `move_if_noexcept`, `as_const`; `is_std_builtin`
/// without `addressof`) on an object, not a function: the call
/// designates the operand's object and runs no code.
pub(super) fn std_cast_arg(ast: &Ast, e: ExprId) -> Option<ExprId> {
    let n = ast.exprs.get(e);
    if n.kind() != ExprKind::CallExpr || !n.value_category().is_glvalue() {
        return None;
    }
    let d = direct_callee(ast, e)?;
    let [x] = ast.lists.get(ast.exprs.call_args(e))[..] else { return None };
    let x = ExprId::new(x);
    if !is_std_builtin(ast, d)
        || is_std_function(ast, d, "addressof")
        || is_std_function(ast, d, "__addressof")
        || ast.types.is_function(ast.exprs.get(skip_parens(ast, x)).ty())
    {
        return None;
    }
    Some(x)
}

/// Whether uninitialized local `d`, declared by a statement of a
/// compound statement, is assigned before any other use: `later` are the
/// declarations after it in its declaration statement, `rest` the
/// statements after that one in the compound statement. A label, `case`
/// or `default` anywhere in `rest` refuses (a jump to it would enter the
/// scope past the declaration, [stmt.dcl]/3), as does any use before the
/// assignment on some path.
pub(super) fn assigned_first(ast: &Ast, d: DeclId, later: &[DeclId], rest: &[StmtId]) -> bool {
    let w = Walk { ast, d };
    if later.iter().any(|&v| w.decl_mentions(v)) || rest.iter().any(|&s| w.has_label(s)) {
        return false;
    }
    w.seq(rest) != Da::Used
}

/// What a statement does to the variable on every path through it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Da {
    /// Assigned before any use, on every path that completes it.
    Assigned,
    /// Not named.
    Untouched,
    /// Named (or maybe named, or assigned on some paths only) before an
    /// assignment.
    Used,
}

struct Walk<'a> {
    ast: &'a Ast,
    d: DeclId,
}

impl Walk<'_> {
    fn seq(&self, stmts: &[StmtId]) -> Da {
        for &s in stmts {
            match self.stmt(s) {
                Da::Untouched => {}
                r => return r,
            }
        }
        Da::Untouched
    }

    fn stmt(&self, s: StmtId) -> Da {
        let ast = self.ast;
        match ast.stmts.get(s).kind() {
            StmtKind::CompoundStmt => {
                let body: Vec<StmtId> =
                    ast.lists.iter::<StmtId>(ast.stmts.compound_body(s)).collect();
                self.seq(&body)
            }
            StmtKind::ExprStmt => {
                let e = ast.stmts.expr_stmt_expr(s);
                if self.is_assignment(e) {
                    Da::Assigned
                } else if self.expr_mentions(e) {
                    Da::Used
                } else {
                    Da::Untouched
                }
            }
            StmtKind::IfStmt => {
                let x = *ast.stmts.if_data(s);
                if x.init.is_some() || x.cond_var.is_some() {
                    return self.generic(s);
                }
                if x.cond.is_some_and(|c| self.expr_mentions(c)) {
                    return Da::Used;
                }
                let a = x.then.map_or(Da::Untouched, |t| self.stmt(t));
                let b = x.els.map_or(Da::Untouched, |t| self.stmt(t));
                if a == b { a } else { Da::Used }
            }
            _ => self.generic(s),
        }
    }

    /// A statement that assigns on no path the walk can prove.
    fn generic(&self, s: StmtId) -> Da {
        if self.stmt_mentions(s) { Da::Used } else { Da::Untouched }
    }

    /// Whether full-expression `e` is `d = rhs` with `rhs` not naming `d`
    /// (the right operand is sequenced first, [expr.assign]/1).
    fn is_assignment(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let mut e = skip_parens(ast, e);
        if ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups {
            e = skip_parens(ast, ast.exprs.operand(e));
        }
        if ast.exprs.get(e).kind() != ExprKind::BinaryOperator
            || ast.exprs.binary_op(e) != kiln_ast::expr::BinaryOp::Assign
        {
            return false;
        }
        let [l, r] = ast.exprs.operands(e);
        let l = skip_parens(ast, l);
        ast.exprs.get(l).kind() == ExprKind::DeclRefExpr
            && ast.exprs.decl(l) == self.d
            && !self.expr_mentions(r)
    }

    /// Whether `e` names `d`, or may (a lambda, an unknown construct).
    fn expr_mentions(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let mut work = vec![e];
        while let Some(x) = work.pop() {
            let n = ast.exprs.get(x);
            match n.kind() {
                ExprKind::DeclRefExpr if ast.exprs.decl(x) == self.d => return true,
                // Its captures, implicit ones included, are not walked.
                ExprKind::LambdaExpr => return true,
                ExprKind::StmtExpr => {
                    if self.stmt_mentions(ast.exprs.stmt_expr_body(x)) {
                        return true;
                    }
                }
                ExprKind::OpaqueValueExpr => work.extend(ast.exprs.opaque_source(x)),
                _ => ast.exprs.children_into(x, &ast.lists, &mut work),
            }
        }
        false
    }

    fn decl_mentions(&self, v: DeclId) -> bool {
        let ast = self.ast;
        if ast.decls.get(v).kind() != DeclKind::Var {
            return false;
        }
        std::iter::once(v)
            .chain(holding_vars(ast, v))
            .filter_map(|v| ast.decls.var(v).and_then(|x| x.init))
            .any(|i| self.expr_mentions(i))
    }

    /// Whether statement `s` names `d`, or may (a statement kind the walk
    /// does not take apart).
    fn stmt_mentions(&self, s: StmtId) -> bool {
        let ast = self.ast;
        let e = |x: Option<ExprId>| x.is_some_and(|x| self.expr_mentions(x));
        let st = |x: Option<StmtId>| x.is_some_and(|x| self.stmt_mentions(x));
        let dv = |x: Option<DeclId>| x.is_some_and(|x| self.decl_mentions(x));
        match ast.stmts.get(s).kind() {
            StmtKind::CompoundStmt => {
                ast.lists.iter::<StmtId>(ast.stmts.compound_body(s)).any(|c| self.stmt_mentions(c))
            }
            StmtKind::DeclStmt => ast
                .lists
                .iter::<DeclId>(ast.stmts.decl_stmt_decls(s))
                .any(|v| self.decl_mentions(v)),
            StmtKind::ExprStmt => self.expr_mentions(ast.stmts.expr_stmt_expr(s)),
            StmtKind::NullStmt
            | StmtKind::BreakStmt
            | StmtKind::ContinueStmt
            | StmtKind::GotoStmt => false,
            StmtKind::ReturnStmt => e(ast.stmts.return_value(s)),
            StmtKind::IfStmt => {
                let x = *ast.stmts.if_data(s);
                st(x.init) || dv(x.cond_var) || e(x.cond) || st(x.then) || st(x.els)
            }
            StmtKind::SwitchStmt => {
                let x = *ast.stmts.switch_data(s);
                st(x.init) || dv(x.cond_var) || e(Some(x.cond)) || st(Some(x.body))
            }
            StmtKind::WhileStmt | StmtKind::DoStmt | StmtKind::ForStmt => {
                let x = *ast.stmts.loop_data(s);
                st(x.init) || dv(x.cond_var) || e(x.cond) || e(x.inc) || st(x.body)
            }
            StmtKind::CXXForRangeStmt => {
                let x = *ast.stmts.range_for_data(s);
                st(x.init)
                    || dv(Some(x.range))
                    || dv(x.begin)
                    || dv(x.end)
                    || e(x.cond)
                    || e(x.inc)
                    || dv(Some(x.loop_var))
                    || st(Some(x.body))
            }
            StmtKind::CXXTryStmt => {
                self.stmt_mentions(ast.stmts.try_block(s))
                    || ast.lists.iter::<StmtId>(ast.stmts.try_handlers(s)).any(|h| {
                        dv(ast.stmts.catch_var(h)) || self.stmt_mentions(ast.stmts.catch_block(h))
                    })
            }
            StmtKind::ContractAssertStmt => self.expr_mentions(ast.stmts.contract_cond(s)),
            // Labels (refused before the walk), attributes (an assumption
            // names expressions), expansion statements, asm, coroutines.
            _ => true,
        }
    }

    /// Whether `s` holds a label, `case` or `default` (or may: a
    /// statement kind the walk does not take apart).
    fn has_label(&self, s: StmtId) -> bool {
        let ast = self.ast;
        let st = |x: Option<StmtId>| x.is_some_and(|x| self.has_label(x));
        match ast.stmts.get(s).kind() {
            StmtKind::CompoundStmt => {
                ast.lists.iter::<StmtId>(ast.stmts.compound_body(s)).any(|c| self.has_label(c))
            }
            StmtKind::DeclStmt
            | StmtKind::ExprStmt
            | StmtKind::NullStmt
            | StmtKind::BreakStmt
            | StmtKind::ContinueStmt
            | StmtKind::GotoStmt
            | StmtKind::ReturnStmt
            | StmtKind::ContractAssertStmt => false,
            StmtKind::IfStmt => {
                let x = *ast.stmts.if_data(s);
                st(x.init) || st(x.then) || st(x.els)
            }
            StmtKind::SwitchStmt => {
                let x = *ast.stmts.switch_data(s);
                st(x.init) || st(Some(x.body))
            }
            StmtKind::WhileStmt | StmtKind::DoStmt | StmtKind::ForStmt => {
                let x = *ast.stmts.loop_data(s);
                st(x.init) || st(x.body)
            }
            StmtKind::CXXForRangeStmt => {
                let x = *ast.stmts.range_for_data(s);
                st(x.init) || st(Some(x.body))
            }
            StmtKind::CXXTryStmt => {
                self.has_label(ast.stmts.try_block(s))
                    || ast
                        .lists
                        .iter::<StmtId>(ast.stmts.try_handlers(s))
                        .any(|h| self.has_label(ast.stmts.catch_block(h)))
            }
            _ => true,
        }
    }
}
