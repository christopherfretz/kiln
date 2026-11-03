//! The "never produces a constant expression" check of `enable_if` and
//! `diagnose_if` conditions (WP-486; spec/sema.md "The `enable_if`
//! attribute"; clang `checkFunctionConditionAttr` /
//! `Expr::isPotentialConstantExprUnevaluated`): a condition that fails to
//! be a constant expression for a reason other than not knowing the
//! function's parameters is an error at the attribute, and the attribute
//! is dropped.
//!
//! clang evaluates the condition with the parameters unknown, going on
//! after a failure to read one; kiln walks the condition instead: a
//! subexpression that names no parameter (nor `this`) and is a scalar
//! prvalue is evaluated on its own, and one that does is taken apart
//! following clang's evaluation order. Anything kiln cannot take apart is
//! "maybe constant", so the check never rejects a condition clang accepts
//! at the price of missing some it rejects.

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::ids::RawId;
use kiln_ast::types::TypeView;
use kiln_ast::{ConstValueId, DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalFailure, EvalKind, FailKind, Note};
use kiln_source::Loc;

use crate::Sema;

/// The deepest condition subexpression the walk takes apart.
const MAX_DEPTH: u32 = 64;

/// A subexpression's outcome with the parameters unknown.
enum Potential {
    /// A constant (it names no parameter, or does so only where the value
    /// is not needed).
    Known(ConstValueId),
    /// It may be a constant for some parameter values.
    Unknown,
    /// It is never a constant; the notes say why.
    Fails(EvalFailure),
}

impl Sema<'_, '_> {
    /// clang `checkFunctionConditionAttr`'s never-constant check of the
    /// condition of `enable_if`/`diagnose_if` attribute `node` on function
    /// `f`: false (after [`Self::report_never_constant`]) drops it. A
    /// value-dependent condition is checked by each instantiation.
    pub(crate) fn check_attr_cond_potential(&mut self, f: DeclId, node: AttrNode) -> bool {
        match self.attr_cond_never_constant(f, node) {
            Some(fail) => {
                self.report_never_constant(node, fail);
                false
            }
            None => true,
        }
    }

    /// Why the condition of attribute `node` on function `f` is never a
    /// constant expression, if it is not.
    pub(crate) fn attr_cond_never_constant(
        &mut self,
        f: DeclId,
        node: AttrNode,
    ) -> Option<EvalFailure> {
        let cond = ExprId::from_raw(node.a)?;
        if !self.ast.decls.get(f).kind().is_function() || self.expr_is_value_dependent(cond) {
            return None;
        }
        match self.potential_constant(cond, node.loc, 0) {
            Potential::Fails(fail) => Some(fail),
            _ => None,
        }
    }

    /// "'x' attribute expression never produces a constant expression"
    /// at attribute `node`, with the evaluation's notes.
    pub(crate) fn report_never_constant(&mut self, node: AttrNode, fail: EvalFailure) {
        let name = if node.kind == AttrKind::DIAGNOSE_IF { "'diagnose_if'" } else { "'enable_if'" };
        let d = self.diag(DiagId::sema_attr_cond_never_constant_expr, node.loc).arg(name);
        let d = Self::with_failure_notes(d, fail);
        self.emit(d);
    }

    /// Whether `e` is a prvalue of arithmetic or enumeration type: what
    /// the walk evaluates on its own.
    fn potential_scalar(&self, e: ExprId) -> bool {
        let n = self.ast.exprs.get(e);
        let ty = n.ty();
        !n.value_category().is_glvalue()
            && (self.ast.types.is_arithmetic(ty)
                || matches!(self.ast.types.canonical_view(ty), TypeView::Enum(_)))
    }

    /// `e` evaluated with the parameters unknown (clang's evaluator in
    /// `checkingPotentialConstantExpression` mode, which goes on after a
    /// failure: every operand of an operator or call is evaluated, except
    /// past an unknown `&&`/`||` left operand or `?:` condition).
    fn potential_constant(&mut self, e: ExprId, loc: Loc, depth: u32) -> Potential {
        if depth > MAX_DEPTH {
            return Potential::Unknown;
        }
        let scalar = self.potential_scalar(e);
        if !self.names_param_or_this(e, 0) {
            if !scalar {
                return Potential::Unknown;
            }
            return match self.eval_const(e, EvalContext::new(EvalKind::Other, loc)) {
                Ok(v) => Potential::Known(v),
                Err(f) if f.notes.is_empty() => Potential::Unknown,
                Err(f) => Potential::Fails(f),
            };
        }
        if scalar {
            // A parameter in an unevaluated operand, or one whose value
            // is not needed.
            if let Ok(v) = self.eval_const(e, EvalContext::new(EvalKind::Other, loc)) {
                return Potential::Known(v);
            }
        }
        let ex = &self.ast.exprs;
        let kind = ex.get(e).kind();
        match kind {
            ExprKind::BinaryOperator
                if matches!(ex.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
            {
                let or = ex.binary_op(e) == BinaryOp::LOr;
                let [l, r] = ex.operands(e);
                match self.potential_constant(l, loc, depth + 1) {
                    Potential::Known(v) => match self.const_truth(v) {
                        Some(t) if t == or => Potential::Known(v),
                        Some(_) => self.potential_constant(r, loc, depth + 1),
                        None => Potential::Unknown,
                    },
                    p => p,
                }
            }
            ExprKind::ConditionalOperator => {
                let c = ex.children(e, &self.ast.lists)[0];
                let [t, f] = ex.cond_arms(e);
                match self.potential_constant(c, loc, depth + 1) {
                    Potential::Known(v) => match self.const_truth(v) {
                        Some(b) => self.potential_constant(if b { t } else { f }, loc, depth + 1),
                        None => Potential::Unknown,
                    },
                    Potential::Unknown => {
                        // clang `CheckPotentialConstantConditional`: never
                        // constant only if neither arm can be.
                        for arm in [f, t] {
                            if !matches!(
                                self.potential_constant(arm, loc, depth + 1),
                                Potential::Fails(_)
                            ) {
                                return Potential::Unknown;
                            }
                        }
                        let at = self.begin_loc(e);
                        Potential::Fails(EvalFailure {
                            kind: FailKind::NotConstant,
                            notes: vec![Note::new(
                                at,
                                "both arms of conditional operator are unable to produce a \
                                 constant expression",
                            )],
                        })
                    }
                    p @ Potential::Fails(_) => p,
                }
            }
            ExprKind::BinaryConditionalOperator => {
                let [common, ..] = ex.binary_conditional_operands(e, &self.ast.lists);
                match self.potential_constant(common, loc, depth + 1) {
                    p @ Potential::Fails(_) => p,
                    _ => Potential::Unknown,
                }
            }
            ExprKind::ParenExpr
            | ExprKind::ImplicitCastExpr
            | ExprKind::CStyleCastExpr
            | ExprKind::CXXFunctionalCastExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::UnaryOperator
            | ExprKind::BinaryOperator
            | ExprKind::ArraySubscriptExpr
            | ExprKind::CallExpr
            | ExprKind::CXXOperatorCallExpr
            | ExprKind::MemberExpr
            | ExprKind::ConstantExpr
            | ExprKind::ExprWithCleanups
            | ExprKind::MaterializeTemporaryExpr => {
                // Every operand, the first never-constant one deciding.
                for c in ex.children(e, &self.ast.lists) {
                    if let p @ Potential::Fails(_) = self.potential_constant(c, loc, depth + 1) {
                        return p;
                    }
                }
                Potential::Unknown
            }
            _ => Potential::Unknown,
        }
    }
}
