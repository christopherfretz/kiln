//! Side effects in unevaluated operands ([expr.context]/1): clang's
//! `-Wunevaluated-expression` ("expression with side effects has no
//! effect in an unevaluated context") on the operands of `sizeof`,
//! `alignof`, `decltype`, `noexcept` and a `typeid` that is not
//! evaluated.

use kiln_ast::ExprId;
use kiln_ast::expr::{ExprDeps, ExprKind};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// Warns when unevaluated operand `e` has side effects, unless it is
    /// instantiation-dependent or this is a template instantiation (as
    /// clang, which checks the pattern instead), at `e`'s location.
    pub(crate) fn check_unevaluated_side_effects(&mut self, e: ExprId) {
        if !self.inst.stack.is_empty() || self.is_err_expr(e) || !self.has_side_effects(e) {
            return;
        }
        let loc = self.expr_loc(e);
        let msg = self.diag(DiagId::sema_side_effects_unevaluated_context, loc);
        self.emit(msg);
    }

    /// clang's `Expr::getExprLoc`: an operator's location for unary,
    /// binary and member expressions; a subscript's base's; a member
    /// call's member name; where it begins otherwise.
    fn expr_loc(&self, mut e: ExprId) -> Loc {
        for _ in 0..256 {
            let exprs = &self.ast.exprs;
            match exprs.get(e).kind() {
                ExprKind::ArraySubscriptExpr | ExprKind::CXXMemberCallExpr => {
                    e = exprs.children(e, &self.ast.lists)[0];
                }
                ExprKind::UnaryOperator
                | ExprKind::BinaryOperator
                | ExprKind::CompoundAssignOperator
                | ExprKind::MemberExpr
                | ExprKind::CXXRewrittenBinaryOperator => return self.eloc(e),
                ExprKind::CXXOperatorCallExpr => {
                    use kiln_ast::names::OverloadedOperator as O;
                    let op = exprs.operator_call_op(e);
                    if matches!(
                        op,
                        O::PlusPlus
                            | O::MinusMinus
                            | O::Arrow
                            | O::Call
                            | O::Subscript
                            | O::New
                            | O::Delete
                            | O::ArrayNew
                            | O::ArrayDelete
                    ) {
                        return self.begin_loc(e);
                    }
                    return self.eloc(e);
                }
                _ => return self.begin_loc(e),
            }
        }
        self.eloc(e)
    }

    /// clang's `Expr::HasSideEffects(Ctx, /*IncludePossibleEffects=*/false)`:
    /// an increment or decrement, an assignment, a `new`, `delete`,
    /// `throw`, `co_await` or `co_yield`, or an evaluated `typeid`,
    /// anywhere in `e` outside nested unevaluated operands and lambda
    /// bodies. A call adds none of its own; an instantiation-dependent
    /// expression and one written in a macro expansion have none.
    pub(crate) fn has_side_effects(&self, e: ExprId) -> bool {
        let exprs = &self.ast.exprs;
        let dependent = ExprDeps::TYPE | ExprDeps::VALUE | ExprDeps::UNEXPANDED_PACK;
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let n = exprs.get(x);
            // Kiln marks every reference to a declaration in a dependent
            // context instantiation-dependent (spec/sema-templates.md
            // §2.4); clang does not, so only type and value dependence
            // count as dependence here.
            if n.deps().intersects(dependent) || n.loc().is_macro() {
                continue;
            }
            match n.kind() {
                ExprKind::UnaryOperator if exprs.unary_op(x).is_inc_dec() => return true,
                ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator
                    if exprs.binary_op(x).is_assignment() =>
                {
                    return true;
                }
                ExprKind::CXXNewExpr
                | ExprKind::CXXDeleteExpr
                | ExprKind::CXXThrowExpr
                | ExprKind::CoawaitExpr
                | ExprKind::DependentCoawaitExpr
                | ExprKind::CoyieldExpr => return true,
                ExprKind::CXXTypeidExpr => {
                    if exprs.typeid_potentially_evaluated(x) {
                        return true;
                    }
                }
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::LambdaExpr
                | ExprKind::RequiresExpr => {}
                _ => stack.extend(exprs.children(x, &self.ast.lists)),
            }
        }
        false
    }
}
