//! Constant evaluation through `Sema` ([expr.const]): expressions are
//! built through `Sema`'s `act_on_*` entry points, so the implicit
//! conversions are the ones the parser would get, then evaluated by the
//! TU's Oven ([`Sema::eval_const`], [`Sema::try_fold`]).

#![allow(clippy::many_single_char_names, reason = "short names mirror the C++ under test")]

mod conv_common;

use conv_common::*;
use kiln_ast::ExprId;
use kiln_ast::QualType;
use kiln_ast::consts::Scalar;
use kiln_ast::expr::{BinaryOp, UnaryOp};
use kiln_sema::{EvalContext, EvalKind, Sema};

/// The integer value of `e`, or `Err` with the failure's notes.
fn eval(s: &mut Sema<'_, '_>, e: ExprId) -> Result<i128, Vec<String>> {
    match s.eval_const(e, EvalContext::new(EvalKind::Other, L)) {
        Ok(v) => Ok(s.const_int(v).expect("an integer")),
        Err(f) => Err(f.notes.into_iter().map(|n| n.msg).collect()),
    }
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
        assert_eq!(eval(s, rem), Ok(3));
        assert_eq!(s.fold_int(rem), Some(3));
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
        assert_eq!(eval(s, u), Ok(i128::from(u32::MAX)));
    });
}

/// [expr.const]/5.8: division by zero is not a core constant expression.
#[test]
fn expr_const_div_by_zero() {
    with_sema(|s, _| {
        let (a, z) = (int(s, 1), int(s, 0));
        let q = s.act_on_binary(BinaryOp::Div, a, z, L);
        let err = eval(s, q).expect_err("1/0 must not fold");
        assert_eq!(err, ["division by zero"]);
        assert_eq!(s.try_fold(q), None, "a silent fold fails too");
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
        assert_eq!(eval(s, sh), Ok(-0x8000_0000));
    });
}

/// [expr.cond], [expr.log.and]: only the selected operand is evaluated.
#[test]
fn expr_cond_short_circuit() {
    with_sema(|s, _| {
        let (f, one, zero) = (bool_lit(s, false), int(s, 1), int(s, 0));
        let bad = s.act_on_binary(BinaryOp::Div, one, zero, L);
        let and = s.act_on_binary(BinaryOp::LAnd, f, bad, L);
        assert_eq!(eval(s, and), Ok(0));
        let (c, t) = (bool_lit(s, true), int(s, 9));
        let one = int(s, 1);
        let zero = int(s, 0);
        let bad = s.act_on_binary(BinaryOp::Div, one, zero, L);
        let sel = s.act_on_conditional(c, t, bad, L);
        assert_eq!(eval(s, sel), Ok(9));
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
        let v = s.try_fold(prod).expect("2.5 * 2 folds");
        assert!(matches!(
            s.ast.consts.scalar(v),
            Some(Scalar::Float { bits, .. }) if bits == u128::from(5.0f64.to_bits())
        ));
    });
}
