//! Integral and floating-point promotions as the usual unary conversions
//! ([conv.prom], [conv.fpprom]) and the usual arithmetic conversions
//! ([expr.arith.conv]), including the C++26 enumeration rules. Each
//! expectation is clang 19's `-ast-dump` (x86_64-linux-gnu, -std=c++26)
//! for the C++ in the test's comment.

#![allow(clippy::many_single_char_names, reason = "short names mirror the C++ under test")]

mod conv_common;

use conv_common::*;
use kiln_ast::ExprId;
use kiln_ast::QualType;
use kiln_diag::DiagId as ids;
use kiln_sema::Sema;
use kiln_sema::conv::{
    ArithConvKind, integral_promotion, promoted_type, usual_arithmetic_conversions,
    usual_arithmetic_conversions_kind, usual_unary_conversions,
};

/// `+e`: the casts the usual unary conversions put on `e`.
fn unary_plus(s: &mut Sema<'_, '_>, e: ExprId) -> Vec<String> {
    let r = usual_unary_conversions(s, e);
    dump_casts(s, r)
}

/// `l + r`: the common type and the casts on each operand.
fn binary(s: &mut Sema<'_, '_>, l: ExprId, r: ExprId) -> (String, Vec<String>, Vec<String>) {
    let (l, r, t) = usual_arithmetic_conversions(s, l, r).expect("arithmetic operands");
    (s.ty_str(t), dump_casts(s, l), dump_casts(s, r))
}

// ---------------------------------------------------------------------------
// [conv.prom]
// ---------------------------------------------------------------------------

/// [conv.prom]/1: a prvalue of an integer type other than `bool`,
/// `char8_t`, `char16_t`, `char32_t`, `wchar_t` whose rank is less than
/// `int`'s promotes to `int`: `short s; +s;` etc.
/// `ImplicitCastExpr 'int' <IntegralCast>`
/// `ImplicitCastExpr 'short' <LValueToRValue>`
#[test]
fn small_integers_promote_to_int_conv_prom_1() {
    with_sema(|s, _| {
        for (t, n) in [
            (QualType::SHORT, "short"),
            (QualType::USHORT, "unsigned short"),
            (QualType::CHAR, "char"),
            (QualType::SCHAR, "signed char"),
            (QualType::UCHAR, "unsigned char"),
        ] {
            let e = lvalue(s, "v", t);
            assert_eq!(
                unary_plus(s, e),
                ["'int' <IntegralCast>".to_string(), format!("'{n}' <LValueToRValue>")]
            );
        }
        // int and wider: only the lvalue-to-rvalue conversion.
        for (t, n) in [
            (QualType::INT, "int"),
            (QualType::UINT, "unsigned int"),
            (QualType::LONG, "long"),
            (QualType::ULONGLONG, "unsigned long long"),
            (QualType::INT128, "__int128"),
        ] {
            let e = lvalue(s, "v", t);
            assert_eq!(unary_plus(s, e), [format!("'{n}' <LValueToRValue>")]);
        }
    });
}

/// [conv.prom]/2: `char8_t`, `char16_t`, `char32_t`, `wchar_t` promote to
/// the first of `int`, `unsigned int`, `long`, ... that can represent all
/// their values: `char32_t c; +c;` is `unsigned int`.
/// `ImplicitCastExpr 'unsigned int' <IntegralCast>`
#[test]
fn character_types_promote_by_range_conv_prom_2() {
    with_sema(|s, _| {
        for (t, n, to) in [
            (QualType::CHAR8, "char8_t", "int"),
            (QualType::CHAR16, "char16_t", "int"),
            (QualType::WCHAR, "wchar_t", "int"),
            (QualType::CHAR32, "char32_t", "unsigned int"),
        ] {
            let e = lvalue(s, "v", t);
            assert_eq!(
                unary_plus(s, e),
                [format!("'{to}' <IntegralCast>"), format!("'{n}' <LValueToRValue>")],
                "{n}"
            );
        }
    });
    // aarch64: wchar_t is unsigned 32-bit, so it promotes to unsigned int.
    with_target(aarch64(), |s, _| {
        let e = lvalue(s, "w", QualType::WCHAR);
        assert_eq!(
            unary_plus(s, e),
            ["'unsigned int' <IntegralCast>", "'wchar_t' <LValueToRValue>"]
        );
    });
}

/// [conv.prom]/3: an unscoped enumeration without a fixed underlying type
/// promotes to the first of `int`, `unsigned int`, ... that represents its
/// values: `enum E { e0 }; E e; +e;`
/// `ImplicitCastExpr 'int' <IntegralCast>`
/// `ImplicitCastExpr 'E' <LValueToRValue>`
#[test]
fn unscoped_enum_promotes_conv_prom_3() {
    with_sema(|s, _| {
        let et = enumeration(s, "E", false, None, (QualType::UINT, QualType::INT));
        let e = lvalue(s, "e", et);
        assert_eq!(unary_plus(s, e), ["'int' <IntegralCast>", "'E' <LValueToRValue>"]);
        assert_eq!(promoted_type(s, et, None), Some(QualType::INT));
        // `enum Big { b = 0xffffffff };` promotes to unsigned int.
        let big = enumeration(s, "Big", false, None, (QualType::UINT, QualType::UINT));
        let e = lvalue(s, "b", big);
        assert_eq!(unary_plus(s, e), ["'unsigned int' <IntegralCast>", "'Big' <LValueToRValue>"]);
    });
}

/// [conv.prom]/4: an unscoped enumeration with a fixed underlying type
/// promotes as its underlying type: `enum EC : unsigned char {}; +ec;` is
/// `int`, `enum EL : long {}; +el;` is `long`.
/// `ImplicitCastExpr 'long' <IntegralCast>`
/// `ImplicitCastExpr 'EL' <LValueToRValue>`
#[test]
fn fixed_enum_promotes_as_underlying_conv_prom_4() {
    with_sema(|s, _| {
        let ec =
            enumeration(s, "EC", false, Some(QualType::UCHAR), (QualType::UCHAR, QualType::INT));
        let e = lvalue(s, "ec", ec);
        assert_eq!(unary_plus(s, e), ["'int' <IntegralCast>", "'EC' <LValueToRValue>"]);
        let el =
            enumeration(s, "EL", false, Some(QualType::LONG), (QualType::LONG, QualType::LONG));
        let e = lvalue(s, "el", el);
        assert_eq!(unary_plus(s, e), ["'long' <IntegralCast>", "'EL' <LValueToRValue>"]);
    });
}

/// [conv.prom]: a scoped enumeration is not promoted ([dcl.enum]/10).
/// `enum class S : char {}; S sc; (void)+sc;` is ill-formed in clang; the
/// usual unary conversions leave it as `S`.
#[test]
fn scoped_enum_not_promoted_conv_prom() {
    with_sema(|s, _| {
        let st = enumeration(s, "S", true, Some(QualType::CHAR), (QualType::CHAR, QualType::INT));
        let e = lvalue(s, "sc", st);
        assert_eq!(unary_plus(s, e), ["'S' <LValueToRValue>"]);
        assert_eq!(promoted_type(s, st, None), None);
    });
}

/// [conv.prom]/5: a bit-field promotes to `int` if `int` represents all
/// its values, else to `unsigned int` if that does; otherwise as its
/// declared type.
/// `struct { unsigned u31:31; unsigned u32:32; long l40:40; int i3:3; } x;`
/// `+x.u31` → `ImplicitCastExpr 'int' <IntegralCast>` over
/// `ImplicitCastExpr 'unsigned int' <LValueToRValue>`; `+x.u32`, `+x.l40`,
/// `+x.i3`: no promotion.
#[test]
fn bit_fields_promote_by_width_conv_prom_5() {
    with_sema(|s, _| {
        let e = bit_field(s, "u31", QualType::UINT, 31);
        assert_eq!(unary_plus(s, e), ["'int' <IntegralCast>", "'unsigned int' <LValueToRValue>"]);
        let e = bit_field(s, "u32", QualType::UINT, 32);
        assert_eq!(unary_plus(s, e), ["'unsigned int' <LValueToRValue>"]);
        let e = bit_field(s, "l40", QualType::LONG, 40);
        assert_eq!(unary_plus(s, e), ["'long' <LValueToRValue>"]);
        let e = bit_field(s, "i3", QualType::INT, 3);
        assert_eq!(unary_plus(s, e), ["'int' <LValueToRValue>"]);
        // `unsigned long ul20 : 20` fits in int.
        let e = bit_field(s, "ul20", QualType::ULONG, 20);
        assert_eq!(unary_plus(s, e), ["'int' <IntegralCast>", "'unsigned long' <LValueToRValue>"]);
        // `unsigned long long ull32 : 32` fits in unsigned int.
        let e = bit_field(s, "ull32", QualType::ULONGLONG, 32);
        assert_eq!(
            unary_plus(s, e),
            ["'unsigned int' <IntegralCast>", "'unsigned long long' <LValueToRValue>"]
        );
        // Through parentheses: `+(x.u31)`.
        let e = bit_field(s, "p31", QualType::UINT, 31);
        let p = s.ast.builder().paren(e, L, L);
        assert_eq!(unary_plus(s, p), ["'int' <IntegralCast>", "'unsigned int' <LValueToRValue>"]);
    });
}

/// [conv.prom]/6-7: `bool` promotes to `int`, also as a bit-field
/// (`bool b1 : 1; +x.b1;`).
/// `ImplicitCastExpr 'int' <IntegralCast>`
/// `ImplicitCastExpr 'bool' <LValueToRValue>`
#[test]
fn bool_promotes_to_int_conv_prom_7() {
    with_sema(|s, _| {
        let e = lvalue(s, "b", QualType::BOOL);
        assert_eq!(unary_plus(s, e), ["'int' <IntegralCast>", "'bool' <LValueToRValue>"]);
        let e = bit_field(s, "b1", QualType::BOOL, 1);
        assert_eq!(unary_plus(s, e), ["'int' <IntegralCast>", "'bool' <LValueToRValue>"]);
        let t = bool_lit(s, true);
        assert_eq!(unary_plus(s, t), ["'int' <IntegralCast>"]);
    });
}

/// [conv.prom]: `integral_promotion` decays arrays first and leaves
/// floating types alone (`float` is not promoted by the usual unary
/// conversions: [conv.fpprom] applies only in conversion sequences).
#[test]
fn integral_promotion_leaves_floats_conv_prom() {
    with_sema(|s, _| {
        let f = lvalue(s, "f", QualType::FLOAT);
        let r = integral_promotion(s, f);
        assert_eq!(dump_casts(s, r), ["'float' <LValueToRValue>"]);
        let f = lvalue(s, "f2", QualType::FLOAT);
        assert_eq!(unary_plus(s, f), ["'float' <LValueToRValue>"]);
        let at = array(s, QualType::INT, 4);
        let a = lvalue(s, "a", at);
        let r = integral_promotion(s, a);
        assert_eq!(dump_casts(s, r), ["'int *' <ArrayToPointerDecay>"]);
    });
}

// ---------------------------------------------------------------------------
// [expr.arith.conv]
// ---------------------------------------------------------------------------

/// [expr.arith.conv]/1.5.1: after promotion, same type: `short s; s + s`
/// is `int`.
/// `BinaryOperator 'int' '+'` with each operand
/// `ImplicitCastExpr 'int' <IntegralCast>` over `'short' <LValueToRValue>`
#[test]
fn promoted_same_type_expr_arith_conv_1_5_1() {
    with_sema(|s, _| {
        let a = lvalue(s, "s", QualType::SHORT);
        let b = lvalue(s, "s", QualType::SHORT);
        let (t, l, r) = binary(s, a, b);
        assert_eq!(t, "int");
        assert_eq!(l, ["'int' <IntegralCast>", "'short' <LValueToRValue>"]);
        assert_eq!(r, ["'int' <IntegralCast>", "'short' <LValueToRValue>"]);
        // `c + b` (char, bool): both promote to int.
        let c = lvalue(s, "c", QualType::CHAR);
        let b = lvalue(s, "b", QualType::BOOL);
        let (t, l, r) = binary(s, c, b);
        assert_eq!(t, "int");
        assert_eq!(l, ["'int' <IntegralCast>", "'char' <LValueToRValue>"]);
        assert_eq!(r, ["'int' <IntegralCast>", "'bool' <LValueToRValue>"]);
        // `us + i`.
        let u = lvalue(s, "us", QualType::USHORT);
        let i = lvalue(s, "i", QualType::INT);
        let (t, l, r) = binary(s, u, i);
        assert_eq!(t, "int");
        assert_eq!(l, ["'int' <IntegralCast>", "'unsigned short' <LValueToRValue>"]);
        assert_eq!(r, ["'int' <LValueToRValue>"]);
    });
}

/// [expr.arith.conv]/1.5.3: unsigned of rank >= the signed one's: `i + u`
/// is `unsigned int`.
/// `ImplicitCastExpr 'unsigned int' <IntegralCast>` over `'int' <LValueToRValue>`
#[test]
fn unsigned_wins_at_equal_rank_expr_arith_conv_1_5_3() {
    with_sema(|s, _| {
        let i = lvalue(s, "i", QualType::INT);
        let u = lvalue(s, "u", QualType::UINT);
        let (t, l, r) = binary(s, i, u);
        assert_eq!(t, "unsigned int");
        assert_eq!(l, ["'unsigned int' <IntegralCast>", "'int' <LValueToRValue>"]);
        assert_eq!(r, ["'unsigned int' <LValueToRValue>"]);
    });
}

/// [expr.arith.conv]/1.5.4: the signed type represents every unsigned
/// value: `l + u` is `long`; `i128 + ull` is `__int128`.
/// `ImplicitCastExpr 'long' <IntegralCast>` over `'unsigned int' <LValueToRValue>`
#[test]
fn signed_represents_unsigned_expr_arith_conv_1_5_4() {
    with_sema(|s, _| {
        let l = lvalue(s, "l", QualType::LONG);
        let u = lvalue(s, "u", QualType::UINT);
        let (t, a, b) = binary(s, l, u);
        assert_eq!(t, "long");
        assert_eq!(a, ["'long' <LValueToRValue>"]);
        assert_eq!(b, ["'long' <IntegralCast>", "'unsigned int' <LValueToRValue>"]);
        let x = lvalue(s, "i128", QualType::INT128);
        let y = lvalue(s, "ull", QualType::ULONGLONG);
        let (t, a, b) = binary(s, x, y);
        assert_eq!(t, "__int128");
        assert_eq!(a, ["'__int128' <LValueToRValue>"]);
        assert_eq!(b, ["'__int128' <IntegralCast>", "'unsigned long long' <LValueToRValue>"]);
    });
}

/// [expr.arith.conv]/1.5.5: otherwise both become the unsigned counterpart
/// of the signed type: `ll + ul` is `unsigned long long` (LP64: `long
/// long` cannot represent every `unsigned long`).
/// `ImplicitCastExpr 'unsigned long long' <IntegralCast>` on both sides
#[test]
fn unsigned_counterpart_expr_arith_conv_1_5_5() {
    with_sema(|s, _| {
        let a = lvalue(s, "ll", QualType::LONGLONG);
        let b = lvalue(s, "ul", QualType::ULONG);
        let (t, l, r) = binary(s, a, b);
        assert_eq!(t, "unsigned long long");
        assert_eq!(l, ["'unsigned long long' <IntegralCast>", "'long long' <LValueToRValue>"]);
        assert_eq!(r, ["'unsigned long long' <IntegralCast>", "'unsigned long' <LValueToRValue>"]);
    });
}

/// [expr.arith.conv]/1.4: one floating operand: `i + d` is `double`,
/// `ld + i` is `long double`, `f + l` is `float`.
/// `ImplicitCastExpr 'double' <IntegralToFloating>`
#[test]
fn integer_to_floating_operand_expr_arith_conv_1_4() {
    with_sema(|s, _| {
        let i = lvalue(s, "i", QualType::INT);
        let d = lvalue(s, "d", QualType::DOUBLE);
        let (t, l, r) = binary(s, i, d);
        assert_eq!(t, "double");
        assert_eq!(l, ["'double' <IntegralToFloating>", "'int' <LValueToRValue>"]);
        assert_eq!(r, ["'double' <LValueToRValue>"]);
        let ld = lvalue(s, "ld", QualType::LONGDOUBLE);
        let i = lvalue(s, "i", QualType::INT);
        let (t, l, r) = binary(s, ld, i);
        assert_eq!(t, "long double");
        assert_eq!(l, ["'long double' <LValueToRValue>"]);
        assert_eq!(r, ["'long double' <IntegralToFloating>", "'int' <LValueToRValue>"]);
        let f = lvalue(s, "f", QualType::FLOAT);
        let lv = lvalue(s, "l", QualType::LONG);
        let (t, l, r) = binary(s, f, lv);
        assert_eq!(t, "float");
        assert_eq!(l, ["'float' <LValueToRValue>"]);
        assert_eq!(r, ["'float' <IntegralToFloating>", "'long' <LValueToRValue>"]);
    });
}

/// [expr.arith.conv]/1.4.2: two floating operands convert to the one of
/// greater conversion rank: `f + d` is `double`, `h + f` (`_Float16`) is
/// `float`.
/// `ImplicitCastExpr 'double' <FloatingCast>` over `'float' <LValueToRValue>`
#[test]
fn floating_rank_expr_arith_conv_1_4_2() {
    with_sema(|s, _| {
        let f = lvalue(s, "f", QualType::FLOAT);
        let d = lvalue(s, "d", QualType::DOUBLE);
        let (t, l, r) = binary(s, f, d);
        assert_eq!(t, "double");
        assert_eq!(l, ["'double' <FloatingCast>", "'float' <LValueToRValue>"]);
        assert_eq!(r, ["'double' <LValueToRValue>"]);
        let h = lvalue(s, "h", QualType::FLOAT16);
        let f = lvalue(s, "f", QualType::FLOAT);
        let (t, l, r) = binary(s, h, f);
        assert_eq!(t, "float");
        assert_eq!(l, ["'float' <FloatingCast>", "'_Float16' <LValueToRValue>"]);
        assert_eq!(r, ["'float' <LValueToRValue>"]);
        let d = lvalue(s, "d", QualType::DOUBLE);
        let ld = lvalue(s, "ld", QualType::LONGDOUBLE);
        let (t, l, _) = binary(s, d, ld);
        assert_eq!(t, "long double");
        assert_eq!(l, ["'long double' <FloatingCast>", "'double' <LValueToRValue>"]);
    });
}

/// [expr.arith.conv]/1.5: an unscoped enumeration and an integer: `e + i`
/// is `int`.
/// `ImplicitCastExpr 'int' <IntegralCast>` over `'E' <LValueToRValue>`
#[test]
fn enum_and_integer_expr_arith_conv_1_5() {
    with_sema(|s, _| {
        let et = enumeration(s, "E", false, None, (QualType::UINT, QualType::INT));
        let e = lvalue(s, "e", et);
        let i = lvalue(s, "i", QualType::INT);
        let (t, l, r) = binary(s, e, i);
        assert_eq!(t, "int");
        assert_eq!(l, ["'int' <IntegralCast>", "'E' <LValueToRValue>"]);
        assert_eq!(r, ["'int' <LValueToRValue>"]);
    });
}

/// [expr.arith.conv]/1.1: two operands of the same scoped enumeration type
/// keep it: `S sc; sc == sc` compares as `S`.
/// `ImplicitCastExpr 'S' <LValueToRValue>` on both sides
#[test]
fn same_scoped_enum_expr_arith_conv_1_1() {
    with_sema(|s, _| {
        let st = enumeration(s, "S", true, Some(QualType::INT), (QualType::INT, QualType::INT));
        let a = lvalue(s, "sc", st);
        let b = lvalue(s, "sc", st);
        let (l, r, t) =
            usual_arithmetic_conversions_kind(s, a, b, ArithConvKind::Comparison).unwrap();
        assert_eq!(s.ty_str(t), "S");
        assert_eq!(dump_casts(s, l), ["'S' <LValueToRValue>"]);
        assert_eq!(dump_casts(s, r), ["'S' <LValueToRValue>"]);
    });
}

/// [expr.arith.conv]/1.2 (C++26, P2864): an enumeration with a
/// floating-point operand is ill-formed: `e + d`.
/// clang: `error: invalid arithmetic between enumeration type 'E' and
/// floating-point type 'double'`
#[test]
fn enum_with_floating_ill_formed_expr_arith_conv_1_2() {
    with_sema(|s, sink| {
        let et = enumeration(s, "E", false, None, (QualType::UINT, QualType::INT));
        let e = lvalue(s, "e", et);
        let d = lvalue(s, "d", QualType::DOUBLE);
        let err = usual_arithmetic_conversions(s, e, d).unwrap_err();
        assert_eq!(err.id, ids::sema_arith_enum_float);
        assert_eq!(
            message(s, sink, &err, e),
            "error: invalid arithmetic between enumeration type 'E' and floating-point type 'double'"
        );
        // Floating first: the order of the message follows the operands.
        let e = lvalue(s, "e", et);
        let d = lvalue(s, "d", QualType::DOUBLE);
        let err = usual_arithmetic_conversions(s, d, e).unwrap_err();
        assert_eq!(
            message(s, sink, &err, d),
            "error: invalid arithmetic between floating-point type 'double' and enumeration type 'E'"
        );
    });
}

/// [expr.arith.conv]/1.3 (C++26, P2864): two different enumeration types
/// are ill-formed: `e + f` and `e == f`.
/// clang: `error: invalid arithmetic between different enumeration types
/// ('E' and 'F')`, `error: invalid comparison of different enumeration
/// types ('E' and 'F')`
#[test]
fn different_enums_ill_formed_expr_arith_conv_1_3() {
    with_sema(|s, sink| {
        let et = enumeration(s, "E", false, None, (QualType::UINT, QualType::INT));
        let ft = enumeration(s, "F", false, None, (QualType::UINT, QualType::INT));
        let e = lvalue(s, "e", et);
        let f = lvalue(s, "f", ft);
        let err = usual_arithmetic_conversions(s, e, f).unwrap_err();
        assert_eq!(
            message(s, sink, &err, e),
            "error: invalid arithmetic between different enumeration types ('E' and 'F')"
        );
        let e = lvalue(s, "e", et);
        let f = lvalue(s, "f", ft);
        let err =
            usual_arithmetic_conversions_kind(s, e, f, ArithConvKind::Comparison).unwrap_err();
        assert_eq!(
            message(s, sink, &err, e),
            "error: invalid comparison of different enumeration types ('E' and 'F')"
        );
        let e = lvalue(s, "e", et);
        let f = lvalue(s, "f", ft);
        let err =
            usual_arithmetic_conversions_kind(s, e, f, ArithConvKind::Conditional).unwrap_err();
        assert_eq!(
            message(s, sink, &err, e),
            "error: invalid conditional expression between different enumeration types ('E' and 'F')"
        );
    });
}

/// [expr.arith.conv]/1.1: a scoped enumeration with another type has no
/// common type: `S sc; sc + 1` is ill-formed.
#[test]
fn scoped_enum_with_other_ill_formed_expr_arith_conv_1_1() {
    with_sema(|s, _| {
        let st = enumeration(s, "S", true, Some(QualType::INT), (QualType::INT, QualType::INT));
        let a = lvalue(s, "sc", st);
        let one = int_lit(s, QualType::INT, 1);
        assert!(usual_arithmetic_conversions(s, a, one).is_err());
    });
}

/// [expr.compound.assign]/7 via clang's `CompoundAssign` mode: the left
/// operand of `i += c` is not converted; the right is converted to the
/// common type.
#[test]
fn compound_assign_left_untouched_expr_ass_7() {
    with_sema(|s, _| {
        let i = lvalue(s, "i", QualType::INT);
        let c = lvalue(s, "c", QualType::CHAR);
        let (l, r, t) =
            usual_arithmetic_conversions_kind(s, i, c, ArithConvKind::CompoundAssign).unwrap();
        assert_eq!(s.ty_str(t), "int");
        assert_eq!(l, i);
        assert_eq!(dump_casts(s, r), ["'int' <IntegralCast>", "'char' <LValueToRValue>"]);
    });
}

/// `l op= r` in clang's `CompoundAssign` mode: the computation type
/// (`ComputeLHSTy`) and the casts on the right operand. The left operand is
/// never converted.
fn compound(s: &mut Sema<'_, '_>, l: ExprId, r: ExprId) -> (String, Vec<String>) {
    let (l2, r2, t) = usual_arithmetic_conversions_kind(s, l, r, ArithConvKind::CompoundAssign)
        .expect("arithmetic operands");
    assert_eq!(l2, l, "the left operand of a compound assignment is not converted");
    (s.ty_str(t), dump_casts(s, r2))
}

/// [expr.ass]/6, [conv.prom]/5: `E1 op= E2` computes `E1 op E2`, so a
/// bit-field left operand takes part promoted by its width, in type only.
/// clang 19, `struct { unsigned c:9; long l3:3; unsigned w:32;
/// unsigned long ul:40; bool b:1; unsigned short us:5; char32_t c32:5; } x;`
/// - `x.c += 2`: `ComputeLHSTy='int'`, `2` unconverted;
/// - `x.c += 2u`: `'unsigned int'` (same unpromoted type, kept);
/// - `x.l3 += 1`: `'int'`; `x.l3 += 1L`: `'long'`;
/// - `x.w += 1`: `'unsigned int'`, `1` `IntegralCast` to it;
/// - `x.ul += 1`: `'unsigned long'` (wider than int: no promotion);
/// - `x.b += 1`, `x.us += 1`, `x.c32 += 1`: `'int'`.
#[test]
fn compound_assign_promotes_bit_field_lhs_expr_ass_6() {
    with_sema(|s, _| {
        let c = bit_field(s, "c", QualType::UINT, 9);
        let two = int_lit(s, QualType::INT, 2);
        assert_eq!(compound(s, c, two), ("int".into(), vec![]));
        let c = bit_field(s, "c", QualType::UINT, 9);
        let two_u = int_lit(s, QualType::UINT, 2);
        assert_eq!(compound(s, c, two_u), ("unsigned int".into(), vec![]));
        let u = lvalue(s, "u", QualType::UINT);
        let c = bit_field(s, "c", QualType::UINT, 9);
        assert_eq!(
            compound(s, c, u),
            ("unsigned int".into(), vec!["'unsigned int' <LValueToRValue>".into()])
        );
        let l3 = bit_field(s, "l3", QualType::LONG, 3);
        let one = int_lit(s, QualType::INT, 1);
        assert_eq!(compound(s, l3, one), ("int".into(), vec![]));
        let l3 = bit_field(s, "l3", QualType::LONG, 3);
        let one_l = int_lit(s, QualType::LONG, 1);
        assert_eq!(compound(s, l3, one_l), ("long".into(), vec![]));
        let w = bit_field(s, "w", QualType::UINT, 32);
        let one = int_lit(s, QualType::INT, 1);
        assert_eq!(
            compound(s, w, one),
            ("unsigned int".into(), vec!["'unsigned int' <IntegralCast>".into()])
        );
        let ul = bit_field(s, "ul", QualType::ULONG, 40);
        let one = int_lit(s, QualType::INT, 1);
        assert_eq!(
            compound(s, ul, one),
            ("unsigned long".into(), vec!["'unsigned long' <IntegralCast>".into()])
        );
        for (t, n, w) in
            [(QualType::BOOL, "b", 1), (QualType::USHORT, "us", 5), (QualType::CHAR32, "c32", 5)]
        {
            let e = bit_field(s, n, t, w);
            let one = int_lit(s, QualType::INT, 1);
            assert_eq!(compound(s, e, one), ("int".into(), vec![]), "{n}");
        }
        // A floating right operand wins whatever the promotion:
        // `x.c *= 2.0` computes in double.
        let c = bit_field(s, "c", QualType::UINT, 9);
        let d = float_lit(s, QualType::DOUBLE, 2.0);
        assert_eq!(compound(s, c, d), ("double".into(), vec![]));
    });
}

/// [expr.ass]/6, [conv.prom]/1-3: a promotable left operand that is not a
/// bit-field is promoted in type too. clang 19:
/// - `char32_t x; x += 1`: `ComputeLHSTy='unsigned int'`, `1`
///   `IntegralCast` to it (`char32_t` promotes to `unsigned int`);
/// - `short sh; sh += sh`: `'int'`; `bool bb; bb += bb`: `'int'`;
/// - `enum E { e0 } e; e += 1`: `'int'` (the assignment back is then
///   ill-formed, which `compound_assign` diagnoses).
#[test]
fn compound_assign_promotes_lhs_type_expr_ass_6() {
    with_sema(|s, _| {
        let x = lvalue(s, "x", QualType::CHAR32);
        let one = int_lit(s, QualType::INT, 1);
        assert_eq!(
            compound(s, x, one),
            ("unsigned int".into(), vec!["'unsigned int' <IntegralCast>".into()])
        );
        let sh = lvalue(s, "sh", QualType::SHORT);
        let sh2 = lvalue(s, "sh", QualType::SHORT);
        assert_eq!(
            compound(s, sh, sh2),
            ("int".into(), vec!["'int' <IntegralCast>".into(), "'short' <LValueToRValue>".into()]),
            "the right operand is promoted, so the left one is too"
        );
        let sh = lvalue(s, "sh", QualType::SHORT);
        let one = int_lit(s, QualType::INT, 1);
        assert_eq!(compound(s, sh, one), ("int".into(), vec![]));
        let et = enumeration(s, "E", false, None, (QualType::UINT, QualType::INT));
        let e = lvalue(s, "e", et);
        let one = int_lit(s, QualType::INT, 1);
        assert_eq!(compound(s, e, one), ("int".into(), vec![]));
    });
    // aarch64: wchar_t is unsigned 32-bit and promotes to unsigned int.
    with_target(aarch64(), |s, _| {
        let w = lvalue(s, "w", QualType::WCHAR);
        let one = int_lit(s, QualType::INT, 1);
        assert_eq!(
            compound(s, w, one),
            ("unsigned int".into(), vec!["'unsigned int' <IntegralCast>".into()])
        );
    });
}

/// [expr.arith.conv]: non-arithmetic operands: `p + d` on an `int*` is not
/// an arithmetic conversion.
#[test]
fn non_arithmetic_operands_expr_arith_conv() {
    with_sema(|s, _| {
        let ip = ptr(s, QualType::INT);
        let p = lvalue(s, "p", ip);
        let d = lvalue(s, "d", QualType::DOUBLE);
        let err = usual_arithmetic_conversions(s, p, d).unwrap_err();
        assert_eq!(err.id, ids::sema_invalid_operands);
    });
}
