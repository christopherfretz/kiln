//! The M2 constant folder behind [`ConstEvaluator`] ([expr.const]):
//! expressions are built through `Sema`'s `act_on_*` entry points, so the
//! implicit conversions are the ones the parser would get, then folded.

#![allow(clippy::many_single_char_names, reason = "short names mirror the C++ under test")]

mod conv_common;

use conv_common::*;
use kiln_ast::ExprId;
use kiln_ast::QualType;
use kiln_ast::expr::{BinaryOp, UnaryOp};
use kiln_sema::constfold::{evaluator, fold_int_expr};
use kiln_sema::{ConstEvaluator, ConstVal, Sema};

fn eval(s: &Sema<'_, '_>, e: ExprId) -> Result<ConstVal, kiln_sema::NotConstant> {
    evaluator(&s.abi).evaluate(&s.ast, e)
}

fn int(s: &mut Sema<'_, '_>, v: u64) -> ExprId {
    int_lit(s, QualType::INT, v)
}

/// [expr.add], [expr.mul]: `(7 + 5) * 3 / 2 % 5` folds to 3.
#[test]
fn expr_arith_folds() {
    with_sema(|s, _| {
        let (a, b, c, d, e) = (int(s, 7), int(s, 5), int(s, 3), int(s, 2), int(s, 5));
        let sum = s.act_on_binary(BinaryOp::Add, a, b, L);
        let sum = s.act_on_paren(sum, L, L);
        let prod = s.act_on_binary(BinaryOp::Mul, sum, c, L);
        let quot = s.act_on_binary(BinaryOp::Div, prod, d, L);
        let rem = s.act_on_binary(BinaryOp::Rem, quot, e, L);
        assert_eq!(eval(s, rem), Ok(ConstVal::Int(3)));
        assert_eq!(fold_int_expr(&s.ast, rem), Some(3));
    });
}

/// [expr.unary.op], [conv.integral]: `-1` converted to `unsigned` wraps.
#[test]
fn conv_integral_wraps() {
    with_sema(|s, _| {
        let one = int(s, 1);
        let neg = s.act_on_unary(UnaryOp::Minus, one, L);
        let z = int_lit(s, QualType::UINT, 0);
        let u = s.act_on_binary(BinaryOp::Add, neg, z, L);
        assert_eq!(eval(s, u), Ok(ConstVal::Int(i128::from(u32::MAX))));
    });
}

/// [expr.const]/5.8: division by zero is not a core constant expression.
#[test]
fn expr_const_div_by_zero() {
    with_sema(|s, _| {
        let (a, z) = (int(s, 1), int(s, 0));
        let q = s.act_on_binary(BinaryOp::Div, a, z, L);
        let err = eval(s, q).expect_err("1/0 must not fold");
        assert!(err.note.is_some(), "a note explains why");
    });
}

/// [expr.const]/5.7: signed overflow is not a core constant expression.
#[test]
fn expr_const_signed_overflow() {
    with_sema(|s, _| {
        let (a, b) = (int(s, 0x7fff_ffff), int(s, 1));
        let sum = s.act_on_binary(BinaryOp::Add, a, b, L);
        assert!(eval(s, sum).is_err());
    });
}

/// [expr.shift]: shifting by the type width is undefined, hence not constant.
#[test]
fn expr_shift_too_far() {
    with_sema(|s, _| {
        let (a, b) = (int(s, 1), int(s, 32));
        let sh = s.act_on_binary(BinaryOp::Shl, a, b, L);
        assert!(eval(s, sh).is_err());
        let (a, b) = (int(s, 1), int(s, 31));
        let sh = s.act_on_binary(BinaryOp::Shl, a, b, L);
        assert_eq!(eval(s, sh), Ok(ConstVal::Int(-0x8000_0000)));
    });
}

/// [expr.cond], [expr.log.and]: only the selected operand is evaluated.
#[test]
fn expr_cond_short_circuit() {
    with_sema(|s, _| {
        let (f, one, zero) = (bool_lit(s, false), int(s, 1), int(s, 0));
        let bad = s.act_on_binary(BinaryOp::Div, one, zero, L);
        let and = s.act_on_binary(BinaryOp::LAnd, f, bad, L);
        assert_eq!(eval(s, and), Ok(ConstVal::Int(0)));
        let (c, t) = (bool_lit(s, true), int(s, 9));
        let one = int(s, 1);
        let zero = int(s, 0);
        let bad = s.act_on_binary(BinaryOp::Div, one, zero, L);
        let sel = s.act_on_conditional(c, t, bad, L);
        assert_eq!(eval(s, sel), Ok(ConstVal::Int(9)));
    });
}

/// [expr.const]: a non-constexpr variable is not usable in a constant expression.
#[test]
fn expr_const_variable_not_usable() {
    with_sema(|s, _| {
        let x = lvalue(s, "x", QualType::INT);
        let one = int(s, 1);
        let sum = s.act_on_binary(BinaryOp::Add, x, one, L);
        assert!(eval(s, sum).is_err());
    });
}

/// [conv.fpint]: floating values fold and truncate on conversion.
#[test]
fn conv_fpint_truncates() {
    with_sema(|s, _| {
        let f = float_lit(s, QualType::DOUBLE, 2.5);
        let two = int(s, 2);
        let prod = s.act_on_binary(BinaryOp::Mul, f, two, L);
        assert_eq!(eval(s, prod), Ok(ConstVal::Float(5.0)));
    });
}
