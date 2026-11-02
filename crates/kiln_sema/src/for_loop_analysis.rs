//! `-Wfor-loop-analysis` ([stmt.for]): clang's
//! `CheckForLoopConditionalStatement` and `CheckForRedundantIteration`
//! (SemaStmt.cpp), run as each `for` statement is built — once for the
//! template definition and again for every instantiation.

use kiln_ast::expr::{CastKind, ExprKind, TraitKind, UnaryOp};
use kiln_ast::names::OverloadedOperator;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_diag::DiagId;

use crate::Sema;

/// Recursion limit for the walks below; deeper trees are not analyzed.
const MAX_DEPTH: u32 = 512;

/// The variables a simple loop condition reads (clang `DeclExtractor`).
#[derive(Default)]
struct CondVars {
    decls: Vec<DeclId>,
    refs: Vec<ExprId>,
}

impl Sema<'_, '_> {
    /// The two `-Wfor-loop-analysis` checks for `for (...; cond; inc)
    /// body`. A condition that declares a variable is not analyzed.
    pub(crate) fn check_for_loop_analysis(
        &mut self,
        cond_var: Option<DeclId>,
        cond: Option<ExprId>,
        inc: Option<ExprId>,
        body: StmtId,
    ) {
        if cond_var.is_none()
            && let Some(c) = cond
        {
            self.check_loop_condition(c, inc, body);
        }
        if let Some(i) = inc {
            self.check_redundant_iteration(i, body);
        }
    }

    /// Warns when no variable of a simple condition — built only from
    /// literals, local non-`volatile` variables and built-in operators —
    /// can change in the loop: none is written, has its address taken or
    /// is bound to a reference in the condition, the increment or the
    /// body, and the body has no `return`, `break` or `goto`.
    fn check_loop_condition(&mut self, cond: ExprId, inc: Option<ExprId>, body: StmtId) {
        let begin = self.expr_range(cond).lo;
        if !self.diags.is_enabled_at(DiagId::warn_variables_not_in_loop_body, begin) {
            return;
        }
        let mut vars = CondVars::default();
        if !self.fla_extract(cond, &mut vars, 0) || vars.decls.is_empty() {
            return;
        }
        let excluded = vars.decls.iter().any(|&d| {
            self.ast
                .decls
                .get(d)
                .ty()
                .is_some_and(|t| self.ast.types.effective_quals(t).is_volatile())
                || !self.has_local_storage(d)
        });
        if excluded
            || self.fla_match(&vars.decls, cond, 0)
            || inc.is_some_and(|i| self.fla_match(&vars.decls, i, 0))
            || self.fla_match_stmt(&vars.decls, body, 0)
        {
            return;
        }
        let n = vars.decls.len();
        let mut d = self
            .diag(DiagId::warn_variables_not_in_loop_body, self.ast.exprs.get(vars.refs[0]).loc());
        d = d.arg(if n > 4 { 0 } else { n });
        for i in 0..4 {
            let name = match vars.decls.get(i) {
                Some(&v) if n <= 4 => self.decl_arg(v),
                _ => String::new(),
            };
            d = d.arg(name);
        }
        for &r in &vars.refs {
            let span = self.expr_range(r);
            d = d.range(span);
        }
        // clang checks every instantiation again, repeating the warning.
        self.emit(d.allow_duplicate());
    }

    /// clang `DeclExtractor`: false if `e` is not a simple condition.
    fn fla_extract(&self, e: ExprId, vars: &mut CondVars, depth: u32) -> bool {
        if depth > MAX_DEPTH {
            return false;
        }
        let ex = &self.ast.exprs;
        let kind = ex.get(e).kind();
        match kind {
            ExprKind::IntegerLiteral
            | ExprKind::FloatingLiteral
            | ExprKind::CXXBoolLiteralExpr
            | ExprKind::CharacterLiteral
            | ExprKind::GNUNullExpr
            | ExprKind::ImaginaryLiteral
            // Unevaluated operands ([expr.context]/1).
            | ExprKind::CXXNoexceptExpr
            | ExprKind::ExpressionTraitExpr
            | ExprKind::OffsetOfExpr => true,
            ExprKind::UnaryExprOrTypeTraitExpr => {
                ex.trait_kind(e) != TraitKind::VecStep
                    || ex.children(e, &self.ast.lists).into_iter().all(|c| self.fla_extract(c, vars, depth + 1))
            }
            ExprKind::CXXTypeidExpr => {
                !ex.typeid_potentially_evaluated(e)
                    || ex.children(e, &self.ast.lists).into_iter().all(|c| self.fla_extract(c, vars, depth + 1))
            }
            ExprKind::UnaryOperator if ex.unary_op(e) == UnaryOp::Deref => false,
            ExprKind::BinaryOperator
            | ExprKind::CompoundAssignOperator
            | ExprKind::UnaryOperator
            | ExprKind::ParenExpr
            | ExprKind::ConditionalOperator
            | ExprKind::LambdaExpr => {
                ex.children(e, &self.ast.lists).into_iter().all(|c| self.fla_extract(c, vars, depth + 1))
            }
            k if k.is_cast() => self.fla_extract(ex.operand(e), vars, depth + 1),
            ExprKind::BinaryConditionalOperator => {
                let [common, _, _, f] = ex.binary_conditional_operands(e, &self.ast.lists);
                self.fla_extract(common, vars, depth + 1) && self.fla_extract(f, vars, depth + 1)
            }
            ExprKind::DeclRefExpr => {
                let d = ex.decl(e);
                if !matches!(self.kind_of(d), kiln_ast::decl::DeclKind::Var | kiln_ast::decl::DeclKind::ParmVar) {
                    return false;
                }
                vars.refs.push(e);
                if !vars.decls.contains(&d) {
                    vars.decls.push(d);
                }
                true
            }
            _ => false,
        }
    }

    /// clang `DeclMatcher` over an expression: true if one of `decls` is
    /// used other than by reading its value.
    fn fla_match(&self, decls: &[DeclId], e: ExprId, depth: u32) -> bool {
        if depth > MAX_DEPTH {
            return true;
        }
        let ex = &self.ast.exprs;
        match ex.get(e).kind() {
            ExprKind::ImplicitCastExpr if ex.cast_kind(e) == CastKind::LValueToRValue => {
                self.fla_match_value(decls, ex.operand(e), depth + 1)
            }
            ExprKind::DeclRefExpr => decls.contains(&ex.decl(e)),
            // An opaque value is visited through its source; unevaluated
            // operands ([expr.context]/1) are not visited.
            ExprKind::OpaqueValueExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::ExpressionTraitExpr
            | ExprKind::OffsetOfExpr => false,
            ExprKind::UnaryExprOrTypeTraitExpr if ex.trait_kind(e) != TraitKind::VecStep => false,
            ExprKind::CXXTypeidExpr if !ex.typeid_potentially_evaluated(e) => false,
            ExprKind::DesignatedInitExpr => self.fla_match(decls, ex.designated_init(e), depth + 1),
            ExprKind::StmtExpr => self.fla_match_stmt(decls, ex.stmt_expr_body(e), depth + 1),
            _ => ex
                .children(e, &self.ast.lists)
                .into_iter()
                .any(|c| self.fla_match(decls, c, depth + 1)),
        }
    }

    /// clang `DeclMatcher::CheckLValueToRValueCast`: `e` is read.
    fn fla_match_value(&self, decls: &[DeclId], e: ExprId, depth: u32) -> bool {
        if depth > MAX_DEPTH {
            return true;
        }
        let e = self.ignore_paren_imp_casts(e);
        let ex = &self.ast.exprs;
        match ex.get(e).kind() {
            ExprKind::DeclRefExpr => false,
            ExprKind::ConditionalOperator => {
                let c = ex.children(e, &self.ast.lists)[0];
                let [t, f] = ex.cond_arms(e);
                self.fla_match(decls, c, depth + 1)
                    || self.fla_match_value(decls, t, depth + 1)
                    || self.fla_match_value(decls, f, depth + 1)
            }
            ExprKind::BinaryConditionalOperator => {
                let [common, _, _, f] = ex.binary_conditional_operands(e, &self.ast.lists);
                self.fla_match_value(decls, common, depth + 1)
                    || self.fla_match_value(decls, f, depth + 1)
            }
            _ => self.fla_match(decls, e, depth + 1),
        }
    }

    /// clang `DeclMatcher` over a statement: any `return`, `break` or
    /// `goto` counts as a use.
    fn fla_match_stmt(&self, decls: &[DeclId], s: StmtId, depth: u32) -> bool {
        if depth > MAX_DEPTH {
            return true;
        }
        let st = &self.ast.stmts;
        let lists = &self.ast.lists;
        let e = |x: Option<ExprId>| x.is_some_and(|x| self.fla_match(decls, x, depth + 1));
        let s_ = |x: Option<StmtId>| x.is_some_and(|x| self.fla_match_stmt(decls, x, depth + 1));
        let var = |d: Option<DeclId>| e(d.and_then(|d| self.ast.decls.var(d)).and_then(|v| v.init));
        match st.get(s).kind() {
            StmtKind::ReturnStmt
            | StmtKind::CoreturnStmt
            | StmtKind::BreakStmt
            | StmtKind::GotoStmt => true,
            StmtKind::CompoundStmt => {
                lists.iter::<StmtId>(st.compound_body(s)).any(|c| s_(Some(c)))
            }
            StmtKind::ExprStmt => e(Some(st.expr_stmt_expr(s))),
            StmtKind::DeclStmt => lists.iter::<DeclId>(st.decl_stmt_decls(s)).any(|d| var(Some(d))),
            StmtKind::IfStmt => {
                let i = *st.if_data(s);
                s_(i.init) || var(i.cond_var) || e(i.cond) || s_(i.then) || s_(i.els)
            }
            StmtKind::SwitchStmt => {
                let w = *st.switch_data(s);
                s_(w.init) || var(w.cond_var) || e(Some(w.cond)) || s_(Some(w.body))
            }
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                let l = *st.loop_data(s);
                s_(l.init) || var(l.cond_var) || e(l.cond) || e(l.inc) || s_(l.body)
            }
            StmtKind::CXXForRangeStmt => {
                let r = *st.range_for_data(s);
                s_(r.init)
                    || var(Some(r.range))
                    || var(r.begin)
                    || var(r.end)
                    || e(r.cond)
                    || e(r.inc)
                    || var(Some(r.loop_var))
                    || s_(Some(r.body))
            }
            StmtKind::CaseStmt => {
                let c = *st.case_data(s);
                e(Some(c.lhs)) || e(c.rhs) || s_(Some(c.sub))
            }
            StmtKind::DefaultStmt => s_(Some(st.default_sub(s))),
            StmtKind::LabelStmt | StmtKind::AttributedStmt => s_(Some(st.sub_stmt(s))),
            StmtKind::CXXTryStmt => {
                s_(Some(st.try_block(s)))
                    || lists.iter::<StmtId>(st.try_handlers(s)).any(|h| s_(Some(h)))
            }
            StmtKind::CXXCatchStmt => s_(Some(st.catch_block(s))),
            StmtKind::GCCAsmStmt => st.asm_operands(s).iter().any(|op| e(Some(op.expr))),
            _ => false,
        }
    }

    /// Warns when the last statement of a braced body increments
    /// (decrements) the same variable as the increment expression and the
    /// body has no `continue` of this loop.
    fn check_redundant_iteration(&mut self, inc: ExprId, body: StmtId) {
        let begin = self.expr_range(inc).lo;
        if !self.diags.is_enabled_at(DiagId::warn_redundant_loop_iteration, begin) {
            return;
        }
        let st = &self.ast.stmts;
        if st.get(body).kind() != StmtKind::CompoundStmt {
            return;
        }
        let Some(last) = self.ast.lists.iter::<StmtId>(st.compound_body(body)).last() else {
            return;
        };
        if st.get(last).kind() != StmtKind::ExprStmt {
            return;
        }
        let Some((loop_inc, loop_ref)) = self.fla_iteration(inc) else { return };
        let Some((last_inc, last_ref)) = self.fla_iteration(st.expr_stmt_expr(last)) else {
            return;
        };
        let ex = &self.ast.exprs;
        if loop_inc != last_inc
            || ex.decl(loop_ref) != ex.decl(last_ref)
            || self.fla_continue(body, 0)
        {
            return;
        }
        let which = u8::from(last_inc);
        let note = self
            .diag(DiagId::note_loop_iteration_here, ex.get(loop_ref).loc())
            .arg(u8::from(loop_inc));
        let d = self
            .diag(DiagId::warn_redundant_loop_iteration, ex.get(last_ref).loc())
            .arg(self.decl_arg(ex.decl(last_ref)))
            .arg(which)
            .with_note(note);
        self.emit(d.allow_duplicate());
    }

    /// clang `ProcessIterationStmt`: `++x`, `x--`, ... applied directly
    /// to a name, built-in or overloaded. Returns whether it increments
    /// and the name.
    fn fla_iteration(&self, e: ExprId) -> Option<(bool, ExprId)> {
        let ex = &self.ast.exprs;
        let mut e = e;
        if ex.get(e).kind() == ExprKind::ExprWithCleanups && !ex.cleanups_have_side_effects(e) {
            e = ex.operand(e);
        }
        let (inc, arg) = match ex.get(e).kind() {
            ExprKind::UnaryOperator => {
                let inc = match ex.unary_op(e) {
                    UnaryOp::PreInc | UnaryOp::PostInc => true,
                    UnaryOp::PreDec | UnaryOp::PostDec => false,
                    _ => return None,
                };
                (inc, ex.operand(e))
            }
            ExprKind::CXXOperatorCallExpr => {
                let inc = match ex.operator_call_op(e) {
                    OverloadedOperator::PlusPlus => true,
                    OverloadedOperator::MinusMinus => false,
                    _ => return None,
                };
                (inc, self.ast.lists.iter::<ExprId>(ex.call_args(e)).next()?)
            }
            _ => return None,
        };
        (ex.get(arg).kind() == ExprKind::DeclRefExpr).then_some((inc, arg))
    }

    /// clang `BreakContinueFinder::ContinueFound`: a `continue` that
    /// belongs to the loop whose body is `s` (nested loops other than
    /// their init-statements are not searched).
    fn fla_continue(&self, s: StmtId, depth: u32) -> bool {
        if depth > MAX_DEPTH {
            return true;
        }
        let st = &self.ast.stmts;
        let sub = |x: Option<StmtId>| x.is_some_and(|x| self.fla_continue(x, depth + 1));
        match st.get(s).kind() {
            StmtKind::ContinueStmt => true,
            StmtKind::ForStmt => sub(st.loop_data(s).init),
            StmtKind::CXXForRangeStmt => sub(st.range_for_data(s).init),
            StmtKind::WhileStmt | StmtKind::DoStmt => false,
            StmtKind::IfStmt => {
                let i = *st.if_data(s);
                sub(i.init) || sub(i.then) || sub(i.els)
            }
            StmtKind::SwitchStmt => {
                let w = *st.switch_data(s);
                sub(w.init) || sub(Some(w.body))
            }
            _ => self.children(s).into_iter().any(|c| self.fla_continue(c, depth + 1)),
        }
    }
}
