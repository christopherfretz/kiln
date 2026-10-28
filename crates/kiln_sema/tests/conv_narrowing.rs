//! Narrowing conversions in list-initialization ([dcl.init.list]/7) and
//! clang's `-Wc++11-narrowing` errors for them. Each case is the
//! initializer `T x{e};` in the comment; the expected messages are
//! clang 19's (x86_64-linux-gnu, -std=c++26).

#![allow(clippy::many_single_char_names, reason = "short names mirror the C++ under test")]

mod conv_common;

use conv_common::*;
use kiln_ast::{ExprId, QualType};
use kiln_diag::MemorySink;
use kiln_sema::Sema;
use kiln_sema::conv::{ConvCtx, Narrowing, narrowing_diag, narrowing_kind, standard_conversion};

fn kind(s: &mut Sema<'_, '_>, e: ExprId, to: QualType) -> Narrowing {
    let c = standard_conversion(s, e, to, ConvCtx::Initialization).expect("convertible");
    narrowing_kind(s, &c, e, None)
}

/// `T x{e};`: clang's error, or `None` if not narrowing.
fn diag(s: &mut Sema<'_, '_>, sink: &MemorySink, e: ExprId, to: QualType) -> Option<String> {
    let n = kind(s, e, to);
    let d = narrowing_diag(s, &n, e, to)?;
    let lines = emitted(s, sink, d);
    assert_eq!(
        lines.get(1).map(String::as_str),
        Some("note: insert an explicit cast to silence this issue")
    );
    Some(lines[0].clone())
}

// ---------------------------------------------------------------------------
// [dcl.init.list]/7.1: floating to integer
// ---------------------------------------------------------------------------

/// [dcl.init.list]/7.1: from a floating-point type to an integer type,
/// always (even a constant): `int a1{d}; int a5{1.0};`
/// clang: `error: type 'double' cannot be narrowed to 'int' in initializer
/// list`
#[test]
fn floating_to_integer_dcl_init_list_7_1() {
    with_sema(|s, sink| {
        let d = lvalue(s, "d", QualType::DOUBLE);
        assert_eq!(kind(s, d, QualType::INT), Narrowing::Type);
        assert_eq!(
            diag(s, sink, d, QualType::INT).as_deref(),
            Some("error: type 'double' cannot be narrowed to 'int' in initializer list")
        );
        let one = float_lit(s, QualType::DOUBLE, 1.0);
        assert_eq!(
            diag(s, sink, one, QualType::INT).as_deref(),
            Some("error: type 'double' cannot be narrowed to 'int' in initializer list")
        );
        let f = lvalue(s, "f", QualType::FLOAT);
        assert_eq!(kind(s, f, QualType::BOOL), Narrowing::Type);
    });
}

// ---------------------------------------------------------------------------
// [dcl.init.list]/7.2: floating to lower-rank floating
// ---------------------------------------------------------------------------

/// [dcl.init.list]/7.2: `double` to `float` from a non-constant:
/// `float a2{d}; double a20{ld};`
/// clang: `error: non-constant-expression cannot be narrowed from type
/// 'double' to 'float' in initializer list`
#[test]
fn floating_lower_rank_variable_dcl_init_list_7_2() {
    with_sema(|s, sink| {
        let d = lvalue(s, "d", QualType::DOUBLE);
        assert_eq!(kind(s, d, QualType::FLOAT), Narrowing::Variable);
        assert_eq!(
            diag(s, sink, d, QualType::FLOAT).as_deref(),
            Some(
                "error: non-constant-expression cannot be narrowed from type 'double' to 'float' in initializer list"
            )
        );
        let ld = lvalue(s, "ld", QualType::LONGDOUBLE);
        assert_eq!(
            diag(s, sink, ld, QualType::DOUBLE).as_deref(),
            Some(
                "error: non-constant-expression cannot be narrowed from type 'long double' to 'double' in initializer list"
            )
        );
        // Widening is never narrowing: `double a19{f};`
        let f = lvalue(s, "f", QualType::FLOAT);
        assert_eq!(kind(s, f, QualType::DOUBLE), Narrowing::NotNarrowing);
    });
}

/// [dcl.init.list]/7.2: a constant within range is not narrowing even when
/// inexact: `float a3{1.0}; float a28{0.1}; float a21{3.4028235e38};`; out
/// of range it is: `float a4{1e300};`
/// clang: `error: constant expression evaluates to 1.000000e+300 which
/// cannot be narrowed to type 'float'`
#[test]
fn floating_lower_rank_constant_dcl_init_list_7_2() {
    with_sema(|s, sink| {
        for v in [1.0, 0.1, 3.402_823_5e38, -3.402_823_5e38, 1e-300, f64::NAN, f64::INFINITY] {
            let e = float_lit(s, QualType::DOUBLE, v);
            assert_eq!(kind(s, e, QualType::FLOAT), Narrowing::NotNarrowing, "{v}");
        }
        let big = float_lit(s, QualType::DOUBLE, 1e300);
        assert_eq!(
            diag(s, sink, big, QualType::FLOAT).as_deref(),
            Some(
                "error: constant expression evaluates to 1.000000e+300 which cannot be narrowed to type 'float'"
            )
        );
        // Through parentheses: `float x{(1e300)};`
        let big = float_lit(s, QualType::DOUBLE, 1e300);
        let p = s.ast.builder().paren(big, L, L);
        assert!(matches!(kind(s, p, QualType::FLOAT), Narrowing::Constant { .. }));
    });
}

// ---------------------------------------------------------------------------
// [dcl.init.list]/7.3: integer to floating
// ---------------------------------------------------------------------------

/// [dcl.init.list]/7.3: integer to floating from a non-constant, or a
/// constant whose value does not round-trip: `double a6{i};
/// float a7{16777217};` but not `float a8{16777216};`
/// clang: `error: constant expression evaluates to 16777217 which cannot
/// be narrowed to type 'float'`
#[test]
fn integer_to_floating_dcl_init_list_7_3() {
    with_sema(|s, sink| {
        let i = lvalue(s, "i", QualType::INT);
        assert_eq!(
            diag(s, sink, i, QualType::DOUBLE).as_deref(),
            Some(
                "error: non-constant-expression cannot be narrowed from type 'int' to 'double' in initializer list"
            )
        );
        let n = int_lit(s, QualType::INT, 16_777_217);
        assert_eq!(
            diag(s, sink, n, QualType::FLOAT).as_deref(),
            Some(
                "error: constant expression evaluates to 16777217 which cannot be narrowed to type 'float'"
            )
        );
        let n = int_lit(s, QualType::INT, 16_777_216);
        assert_eq!(kind(s, n, QualType::FLOAT), Narrowing::NotNarrowing);
        let t = bool_lit(s, true);
        assert_eq!(kind(s, t, QualType::FLOAT), Narrowing::NotNarrowing);
        let m = neg_int(s, 5);
        assert_eq!(kind(s, m, QualType::DOUBLE), Narrowing::NotNarrowing);
    });
}

// ---------------------------------------------------------------------------
// [dcl.init.list]/7.4: integer to narrower integer
// ---------------------------------------------------------------------------

/// [dcl.init.list]/7.4: to an integer type that cannot represent every
/// source value, unless a constant that fits: `char a9{300};
/// unsigned a11{-1}; int a14{2147483648L}; short a24{-32769};
/// unsigned long long a25{-1LL};`
/// clang: `error: constant expression evaluates to 300 which cannot be
/// narrowed to type 'char'`
#[test]
fn integer_constant_out_of_range_dcl_init_list_7_4() {
    with_sema(|s, sink| {
        let e = int_lit(s, QualType::INT, 300);
        assert_eq!(
            diag(s, sink, e, QualType::CHAR).as_deref(),
            Some(
                "error: constant expression evaluates to 300 which cannot be narrowed to type 'char'"
            )
        );
        let e = neg_int(s, 1);
        assert_eq!(
            diag(s, sink, e, QualType::UINT).as_deref(),
            Some(
                "error: constant expression evaluates to -1 which cannot be narrowed to type 'unsigned int'"
            )
        );
        let e = int_lit(s, QualType::LONG, 2_147_483_648);
        assert_eq!(
            diag(s, sink, e, QualType::INT).as_deref(),
            Some(
                "error: constant expression evaluates to 2147483648 which cannot be narrowed to type 'int'"
            )
        );
        let e = neg_int(s, 32769);
        assert_eq!(
            diag(s, sink, e, QualType::SHORT).as_deref(),
            Some(
                "error: constant expression evaluates to -32769 which cannot be narrowed to type 'short'"
            )
        );
        let one = int_lit(s, QualType::LONGLONG, 1);
        let e = s.ast.builder().unary(
            kiln_ast::expr::UnaryOp::Minus,
            one,
            QualType::LONGLONG,
            L,
            kiln_ast::expr::ExprFlags::NONE,
        );
        assert_eq!(
            diag(s, sink, e, QualType::ULONGLONG).as_deref(),
            Some(
                "error: constant expression evaluates to -1 which cannot be narrowed to type 'unsigned long long'"
            )
        );
    });
}

/// [dcl.init.list]/7.4: constants that fit are fine: `char a10{100};
/// bool a16{1}; int a27{true}; short{-32768}`.
#[test]
fn integer_constant_in_range_dcl_init_list_7_4() {
    with_sema(|s, _| {
        let e = int_lit(s, QualType::INT, 100);
        assert_eq!(kind(s, e, QualType::CHAR), Narrowing::NotNarrowing);
        let e = int_lit(s, QualType::INT, 1);
        assert_eq!(kind(s, e, QualType::BOOL), Narrowing::NotNarrowing);
        let e = bool_lit(s, true);
        assert_eq!(kind(s, e, QualType::INT), Narrowing::NotNarrowing);
        let e = neg_int(s, 32768);
        assert_eq!(kind(s, e, QualType::SHORT), Narrowing::NotNarrowing);
        let e = int_lit(s, QualType::INT, 255);
        assert_eq!(kind(s, e, QualType::UCHAR), Narrowing::NotNarrowing);
        let e = int_lit(s, QualType::INT, 256);
        assert!(matches!(kind(s, e, QualType::UCHAR), Narrowing::Constant { .. }));
        // `bool{2}` is narrowing.
        let e = int_lit(s, QualType::INT, 2);
        assert!(matches!(kind(s, e, QualType::BOOL), Narrowing::Constant { .. }));
    });
}

/// [dcl.init.list]/7.4: from non-constants: `unsigned a12{i}; int a13{l};
/// unsigned char a15{u}; bool a17{i};`, but `long a18{i};` widens.
/// clang: `error: non-constant-expression cannot be narrowed from type
/// 'int' to 'unsigned int' in initializer list`
#[test]
fn integer_variable_dcl_init_list_7_4() {
    with_sema(|s, sink| {
        for (from, fname, to, tname) in [
            (QualType::INT, "int", QualType::UINT, "unsigned int"),
            (QualType::LONG, "long", QualType::INT, "int"),
            (QualType::UINT, "unsigned int", QualType::UCHAR, "unsigned char"),
            (QualType::INT, "int", QualType::BOOL, "bool"),
        ] {
            let e = lvalue(s, "v", from);
            assert_eq!(
                diag(s, sink, e, to),
                Some(format!(
                    "error: non-constant-expression cannot be narrowed from type '{fname}' to '{tname}' in initializer list"
                ))
            );
        }
        for (from, to) in [
            (QualType::INT, QualType::LONG),
            (QualType::UINT, QualType::LONG),
            (QualType::USHORT, QualType::INT),
            (QualType::BOOL, QualType::INT),
            (QualType::CHAR, QualType::INT),
        ] {
            let e = lvalue(s, "w", from);
            assert_eq!(kind(s, e, to), Narrowing::NotNarrowing);
        }
    });
}

/// [dcl.init.list]/7.4 with a fixed-underlying enumeration target: `enum
/// E : unsigned char {}; E a22{256};` narrows to the underlying type.
/// clang: `error: constant expression evaluates to 256 which cannot be
/// narrowed to type 'E'`
#[test]
fn enum_target_uses_underlying_dcl_init_list_7_4() {
    with_sema(|s, sink| {
        let et =
            enumeration(s, "E", false, Some(QualType::UCHAR), (QualType::UCHAR, QualType::INT));
        let e = int_lit(s, QualType::INT, 256);
        let c = standard_conversion(s, e, QualType::UCHAR, ConvCtx::Initialization).unwrap();
        // `E{256}`: direct-list-init of a fixed enum from an integer
        // ([dcl.init.list]/3.8); the conversion checked is to the
        // underlying type, reported against `E`.
        let mut c2 = c;
        c2.to_types = [QualType::INT, et, et];
        let n = narrowing_kind(s, &c2, e, None);
        let d = narrowing_diag(s, &n, e, et).unwrap();
        assert_eq!(
            emitted(s, sink, d)[0],
            "error: constant expression evaluates to 256 which cannot be narrowed to type 'E'"
        );
    });
}

/// [dcl.init.list]/7: an evaluated constant can be supplied by the caller
/// (`const int k = 7; char c{k};`): a constant that fits is fine.
#[test]
fn caller_supplied_value_dcl_init_list_7() {
    with_sema(|s, _| {
        use kiln_ast::consts::{IntBits, Scalar};
        let int = |v: i128| Scalar::Int {
            ty: QualType::INT,
            signed: true,
            bits: IntBits::from_i128(32, v),
        };
        let k = lvalue(s, "k", QualType::INT);
        let c = standard_conversion(s, k, QualType::CHAR, ConvCtx::Initialization).unwrap();
        assert_eq!(narrowing_kind(s, &c, k, None), Narrowing::Variable);
        let seven = s.ast.consts.push_scalar(int(7)).unwrap();
        assert_eq!(narrowing_kind(s, &c, k, Some(seven)), Narrowing::NotNarrowing);
        let big = s.ast.consts.push_scalar(int(1000)).unwrap();
        assert_eq!(
            narrowing_kind(s, &c, k, Some(big)),
            Narrowing::Constant { value: "1000".into() }
        );
    });
}
