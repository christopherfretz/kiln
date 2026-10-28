//! The standard conversions ([conv]) as performed by `implicit_convert`
//! in copy-initialization: the cast nodes built, their kinds, types and
//! value categories, and the standard conversion sequence classified
//! ([over.ics.scs]). Each expectation is clang 19's `-ast-dump` for the
//! C++ in the test's comment (x86_64-linux-gnu, -std=c++26).

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "short names mirror the C++ under test"
)]

mod conv_common;

use conv_common::*;
use kiln_ast::QualType;
use kiln_ast::expr::{CastKind, ExprKind, ValueCategory};
use kiln_sema::Sema;
use kiln_sema::conv::{
    ConvCtx, ConvRank, FirstStep, SecondStep, StandardConversion, ThirdStep, implicit_convert,
    standard_conversion,
};

/// `T x = e;`: the casts `implicit_convert` wraps around `e`, as dump
/// fragments, outermost first.
fn init(s: &mut Sema<'_, '_>, e: kiln_ast::ExprId, to: QualType) -> Vec<String> {
    let r = implicit_convert(s, e, to, ConvCtx::Initialization).expect("convertible");
    let out = dump_casts(s, r);
    // The innermost node is `e` itself.
    assert_eq!(under_casts(s, r), under_casts(s, e));
    out
}

fn scs(s: &mut Sema<'_, '_>, e: kiln_ast::ExprId, to: QualType) -> StandardConversion {
    standard_conversion(s, e, to, ConvCtx::Initialization).expect("a standard conversion")
}

// ---------------------------------------------------------------------------
// [conv.lval]
// ---------------------------------------------------------------------------

/// [conv.lval]/1: `int x; int a = x;` — the result is a prvalue of the
/// cv-unqualified type.
/// `ImplicitCastExpr 'int' <LValueToRValue>`
#[test]
fn lvalue_to_rvalue_conv_lval_1() {
    with_sema(|s, _| {
        let x = lvalue(s, "x", QualType::INT);
        assert_eq!(init(s, x, QualType::INT), ["'int' <LValueToRValue>"]);
        let x = lvalue(s, "x2", QualType::INT);
        let c = scs(s, x, QualType::INT);
        assert_eq!(
            (c.first, c.second, c.third),
            (FirstStep::LValueToRValue, SecondStep::Identity, ThirdStep::Identity)
        );
        assert_eq!(c.rank(), ConvRank::ExactMatch);
        assert!(c.is_identity());
    });
}

/// [conv.lval]/1: `const int cx; volatile int vx; int b = cx, c = vx;` —
/// cv-qualifiers are dropped by the lvalue-to-rvalue conversion.
/// `ImplicitCastExpr 'int' <LValueToRValue>`
#[test]
fn lvalue_to_rvalue_drops_cv_conv_lval_1() {
    with_sema(|s, _| {
        let ci = cnst(s, QualType::INT);
        let cx = lvalue(s, "cx", ci);
        assert_eq!(init(s, cx, QualType::INT), ["'int' <LValueToRValue>"]);
        let vi = vol(s, QualType::INT);
        let vx = lvalue(s, "vx", vi);
        assert_eq!(init(s, vx, QualType::INT), ["'int' <LValueToRValue>"]);
        // An xvalue converts the same way.
        let xv = xvalue(s, "xv", QualType::INT);
        assert_eq!(init(s, xv, QualType::INT), ["'int' <LValueToRValue>"]);
    });
}

/// [conv.lval]/1: a prvalue needs no lvalue-to-rvalue conversion
/// (`int a = g();`: no cast).
#[test]
fn prvalue_needs_no_conversion_conv_lval_1() {
    with_sema(|s, _| {
        let g = prvalue(s, "g", QualType::INT);
        assert_eq!(init(s, g, QualType::INT), Vec::<String>::new());
        let c = scs(s, g, QualType::INT);
        assert_eq!(c.first, FirstStep::Identity);
        let one = int_lit(s, QualType::INT, 1);
        assert_eq!(init(s, one, QualType::INT), Vec::<String>::new());
    });
}

// ---------------------------------------------------------------------------
// [conv.array]
// ---------------------------------------------------------------------------

/// [conv.array]/1: `int a[3]; int* p = a;`
/// `ImplicitCastExpr 'int *' <ArrayToPointerDecay>`
#[test]
fn array_to_pointer_conv_array_1() {
    with_sema(|s, _| {
        let at = array(s, QualType::INT, 3);
        let a = lvalue(s, "a", at);
        let p = ptr(s, QualType::INT);
        assert_eq!(init(s, a, p), ["'int *' <ArrayToPointerDecay>"]);
        let a = lvalue(s, "a2", at);
        let c = scs(s, a, p);
        assert_eq!(
            (c.first, c.second, c.rank()),
            (FirstStep::ArrayToPointer, SecondStep::Identity, ConvRank::ExactMatch)
        );
    });
}

/// [conv.array]/1: `const int ca[2]; const int* q = ca;` — the element's
/// cv-qualification is kept.
/// `ImplicitCastExpr 'const int *' <ArrayToPointerDecay>`
#[test]
fn array_of_const_conv_array_1() {
    with_sema(|s, _| {
        let ci = cnst(s, QualType::INT);
        let at = array(s, ci, 2);
        let a = lvalue(s, "ca", at);
        let p = ptr(s, ci);
        assert_eq!(init(s, a, p), ["'const int *' <ArrayToPointerDecay>"]);
    });
}

/// [conv.array]/1 with [conv.qual]: `int a[3]; const int* q = a;` — decay,
/// then a qualification conversion.
/// `ImplicitCastExpr 'const int *' <NoOp>`
/// `ImplicitCastExpr 'int *' <ArrayToPointerDecay>`
#[test]
fn array_decay_then_qualification_conv_array_1() {
    with_sema(|s, _| {
        let at = array(s, QualType::INT, 3);
        let a = lvalue(s, "a", at);
        let ci = cnst(s, QualType::INT);
        let p = ptr(s, ci);
        assert_eq!(init(s, a, p), ["'const int *' <NoOp>", "'int *' <ArrayToPointerDecay>"]);
        let a = lvalue(s, "a2", at);
        let c = scs(s, a, p);
        assert_eq!((c.first, c.third), (FirstStep::ArrayToPointer, ThirdStep::Qualification));
        assert_eq!(c.rank(), ConvRank::ExactMatch);
    });
}

/// [conv.array]/1 to `bool`: `int a[3]; bool b = a;`
/// `ImplicitCastExpr 'bool' <PointerToBoolean>`
/// `ImplicitCastExpr 'int *' <ArrayToPointerDecay>`
#[test]
fn array_to_bool_conv_array_1_conv_bool() {
    with_sema(|s, _| {
        let at = array(s, QualType::INT, 3);
        let a = lvalue(s, "a", at);
        assert_eq!(
            init(s, a, QualType::BOOL),
            ["'bool' <PointerToBoolean>", "'int *' <ArrayToPointerDecay>"]
        );
    });
}

// ---------------------------------------------------------------------------
// [conv.func]
// ---------------------------------------------------------------------------

/// [conv.func]/1: `void f(); void (*p)() = f;`
/// `ImplicitCastExpr 'void (*)()' <FunctionToPointerDecay>`
#[test]
fn function_to_pointer_conv_func_1() {
    with_sema(|s, _| {
        let ft = fn_type(s, QualType::VOID, &[]);
        let f = function_ref(s, "f", ft);
        let fp = ptr(s, ft);
        assert_eq!(init(s, f, fp), ["'void (*)()' <FunctionToPointerDecay>"]);
        let f = function_ref(s, "f2", ft);
        let c = scs(s, f, fp);
        assert_eq!(
            (c.first, c.second, c.third),
            (FirstStep::FunctionToPointer, SecondStep::Identity, ThirdStep::Identity)
        );
    });
}

/// [conv.func]/1 with [conv.fctptr]: `void fn() noexcept; void (*r)() = fn;`
/// `ImplicitCastExpr 'void (*)()' <NoOp>`
/// `ImplicitCastExpr 'void (*)() noexcept' <FunctionToPointerDecay>`
#[test]
fn function_to_pointer_then_fctptr_conv_func_1() {
    with_sema(|s, _| {
        let nft = noexcept_fn_type(s, QualType::VOID, &[]);
        let ft = fn_type(s, QualType::VOID, &[]);
        let f = function_ref(s, "fn", nft);
        let fp = ptr(s, ft);
        assert_eq!(
            init(s, f, fp),
            ["'void (*)()' <NoOp>", "'void (*)() noexcept' <FunctionToPointerDecay>"]
        );
        let f = function_ref(s, "fn2", nft);
        let c = scs(s, f, fp);
        assert_eq!((c.first, c.third), (FirstStep::FunctionToPointer, ThirdStep::FunctionPointer));
        assert_eq!(c.rank(), ConvRank::ExactMatch);
    });
}

// ---------------------------------------------------------------------------
// [conv.qual]
// ---------------------------------------------------------------------------

/// [conv.qual]/3: `int* p; const int* a = p; const volatile int* c = p;`
/// `ImplicitCastExpr 'const int *' <NoOp>`
/// `ImplicitCastExpr 'int *' <LValueToRValue>`
#[test]
fn qualification_one_level_conv_qual_3() {
    with_sema(|s, _| {
        let ip = ptr(s, QualType::INT);
        let ci = cnst(s, QualType::INT);
        let cip = ptr(s, ci);
        let p = lvalue(s, "p", ip);
        assert_eq!(init(s, p, cip), ["'const int *' <NoOp>", "'int *' <LValueToRValue>"]);
        let cvi = vol(s, ci);
        let cvip = ptr(s, cvi);
        let p = lvalue(s, "p2", ip);
        assert_eq!(init(s, p, cvip), ["'const volatile int *' <NoOp>", "'int *' <LValueToRValue>"]);
    });
}

/// [conv.qual]/3: `int** pp; const int* const* b = pp;` and
/// `int*** ppp; const int* const* const* d = ppp;` — `const` must be added
/// at every level above the first changed one.
/// `ImplicitCastExpr 'const int *const *' <NoOp>`
#[test]
fn qualification_multi_level_conv_qual_3() {
    with_sema(|s, _| {
        let ip = ptr(s, QualType::INT);
        let ipp = ptr(s, ip);
        let ci = cnst(s, QualType::INT);
        let cip = ptr(s, ci);
        let cipc = cnst(s, cip);
        let to = ptr(s, cipc);
        let pp = lvalue(s, "pp", ipp);
        assert_eq!(init(s, pp, to), ["'const int *const *' <NoOp>", "'int **' <LValueToRValue>"]);

        let ippp = ptr(s, ipp);
        let to2c = cnst(s, to);
        let to3 = ptr(s, to2c);
        let ppp = lvalue(s, "ppp", ippp);
        assert_eq!(
            init(s, ppp, to3),
            ["'const int *const *const *' <NoOp>", "'int ***' <LValueToRValue>"]
        );
    });
}

/// [conv.qual]/3.3: `int** pp; const int** q = pp;` is ill-formed — `const`
/// is not added at the intermediate level.
#[test]
fn qualification_needs_const_above_conv_qual_3() {
    with_sema(|s, _| {
        let ip = ptr(s, QualType::INT);
        let ipp = ptr(s, ip);
        let ci = cnst(s, QualType::INT);
        let cip = ptr(s, ci);
        let to = ptr(s, cip);
        let pp = lvalue(s, "pp", ipp);
        assert!(standard_conversion(s, pp, to, ConvCtx::Initialization).is_none());
        assert!(implicit_convert(s, pp, to, ConvCtx::Initialization).is_err());
    });
}

/// [conv.qual]/3 (P0388): arrays of known bound: `int (*ap)[3];
/// const int (*e)[3] = ap;`
/// `ImplicitCastExpr 'const int (*)[3]' <NoOp>`
#[test]
fn qualification_through_array_conv_qual_3() {
    with_sema(|s, _| {
        let a3 = array(s, QualType::INT, 3);
        let ap = ptr(s, a3);
        let ci = cnst(s, QualType::INT);
        let ca3 = array(s, ci, 3);
        let cap = ptr(s, ca3);
        let e = lvalue(s, "ap", ap);
        assert_eq!(init(s, e, cap), ["'const int (*)[3]' <NoOp>", "'int (*)[3]' <LValueToRValue>"]);
    });
}

/// [conv.qual]: removing a qualifier is not a conversion: `const int* p;
/// int* q = p;` is ill-formed.
#[test]
fn qualification_cannot_drop_const_conv_qual() {
    with_sema(|s, _| {
        let ci = cnst(s, QualType::INT);
        let cip = ptr(s, ci);
        let ip = ptr(s, QualType::INT);
        let p = lvalue(s, "p", cip);
        assert!(standard_conversion(s, p, ip, ConvCtx::Initialization).is_none());
    });
}

/// [conv.qual]/3 for member pointers: `int B::* m; const int B::* c = m;`
/// `ImplicitCastExpr 'const int B::*' <NoOp>`
#[test]
fn qualification_member_pointer_conv_qual_3() {
    with_sema(|s, _| {
        let b = class(s, "B", &[]);
        let m = mem_ptr(s, QualType::INT, b);
        let ci = cnst(s, QualType::INT);
        let cm = mem_ptr(s, ci, b);
        let e = lvalue(s, "m", m);
        assert_eq!(init(s, e, cm), ["'const int B::*' <NoOp>", "'int B::*' <LValueToRValue>"]);
    });
}

// ---------------------------------------------------------------------------
// [conv.prom] in initialization (the promotions as a second step)
// ---------------------------------------------------------------------------

/// [conv.prom]/1, [over.ics.scs]: `short s; int x = s;` is an integral
/// promotion of rank Promotion.
/// `ImplicitCastExpr 'int' <IntegralCast>`
/// `ImplicitCastExpr 'short' <LValueToRValue>`
#[test]
fn promotion_rank_conv_prom_1() {
    with_sema(|s, _| {
        for (n, t, name) in [
            ("s", QualType::SHORT, "short"),
            ("c", QualType::CHAR, "char"),
            ("uc", QualType::UCHAR, "unsigned char"),
            ("us", QualType::USHORT, "unsigned short"),
            ("b", QualType::BOOL, "bool"),
            ("w", QualType::WCHAR, "wchar_t"),
            ("c8", QualType::CHAR8, "char8_t"),
            ("c16", QualType::CHAR16, "char16_t"),
        ] {
            let e = lvalue(s, n, t);
            let c = scs(s, e, QualType::INT);
            assert_eq!(c.second, SecondStep::IntegralPromotion, "{name}");
            assert_eq!(c.rank(), ConvRank::Promotion, "{name}");
            assert_eq!(
                init(s, e, QualType::INT),
                ["'int' <IntegralCast>".to_string(), format!("'{name}' <LValueToRValue>")]
            );
        }
        // char32_t promotes to unsigned int, so to int it is a conversion.
        let e = lvalue(s, "c32", QualType::CHAR32);
        assert_eq!(scs(s, e, QualType::UINT).second, SecondStep::IntegralPromotion);
        assert_eq!(scs(s, e, QualType::INT).second, SecondStep::IntegralConversion);
        // int to long is a conversion, not a promotion ([conv.prom]/1).
        let e = lvalue(s, "i", QualType::INT);
        assert_eq!(scs(s, e, QualType::LONG).second, SecondStep::IntegralConversion);
    });
}

/// [conv.fpprom]/1: `float f; double a = f;` is a floating-point promotion.
/// `ImplicitCastExpr 'double' <FloatingCast>`
/// `ImplicitCastExpr 'float' <LValueToRValue>`
#[test]
fn float_promotion_conv_fpprom_1() {
    with_sema(|s, _| {
        let f = lvalue(s, "f", QualType::FLOAT);
        let c = scs(s, f, QualType::DOUBLE);
        assert_eq!((c.second, c.rank()), (SecondStep::FloatingPromotion, ConvRank::Promotion));
        assert_eq!(
            init(s, f, QualType::DOUBLE),
            ["'double' <FloatingCast>", "'float' <LValueToRValue>"]
        );
        // float -> long double and double -> long double are conversions.
        assert_eq!(scs(s, f, QualType::LONGDOUBLE).second, SecondStep::FloatingConversion);
        let d = lvalue(s, "d", QualType::DOUBLE);
        let c = scs(s, d, QualType::LONGDOUBLE);
        assert_eq!((c.second, c.rank()), (SecondStep::FloatingConversion, ConvRank::Conversion));
    });
}

// ---------------------------------------------------------------------------
// [conv.integral]
// ---------------------------------------------------------------------------

/// [conv.integral]/1: `int i; long l; unsigned u;` with `unsigned char a = i;
/// long c = i; int d = l; unsigned e = i; int g = u;`
/// `ImplicitCastExpr 'unsigned char' <IntegralCast>`
#[test]
fn integral_conversions_conv_integral_1() {
    with_sema(|s, _| {
        for (from, fname, to, tname) in [
            (QualType::INT, "int", QualType::UCHAR, "unsigned char"),
            (QualType::INT, "int", QualType::LONG, "long"),
            (QualType::LONG, "long", QualType::INT, "int"),
            (QualType::INT, "int", QualType::UINT, "unsigned int"),
            (QualType::UINT, "unsigned int", QualType::INT, "int"),
            (QualType::LONGLONG, "long long", QualType::LONG, "long"),
            (QualType::INT128, "__int128", QualType::INT, "int"),
        ] {
            let e = lvalue(s, "v", from);
            let c = scs(s, e, to);
            assert_eq!(
                (c.second, c.rank()),
                (SecondStep::IntegralConversion, ConvRank::Conversion)
            );
            assert_eq!(
                init(s, e, to),
                [format!("'{tname}' <IntegralCast>"), format!("'{fname}' <LValueToRValue>")]
            );
        }
    });
}

/// [conv.integral]/1, [dcl.enum]/8: an unscoped enumeration converts to
/// any integer type: `enum E { e0 }; E e; long l = e;`
/// `ImplicitCastExpr 'long' <IntegralCast>`
/// `ImplicitCastExpr 'E' <LValueToRValue>`
#[test]
fn unscoped_enum_to_integer_conv_integral_1() {
    with_sema(|s, _| {
        let et = enumeration(s, "E", false, None, (QualType::UINT, QualType::INT));
        let e = lvalue(s, "e", et);
        assert_eq!(init(s, e, QualType::LONG), ["'long' <IntegralCast>", "'E' <LValueToRValue>"]);
        // To its promoted type it is a promotion ([conv.prom]/3).
        let c = scs(s, e, QualType::INT);
        assert_eq!(c.second, SecondStep::IntegralPromotion);
    });
}

/// [conv.integral]/1: no implicit conversion to an enumeration, nor from a
/// scoped one ([dcl.enum]/10): `int i; E e = i;` and `S s; int j = s;` are
/// ill-formed.
#[test]
fn no_conversion_to_enum_or_from_scoped_conv_integral() {
    with_sema(|s, _| {
        let et = enumeration(s, "E", false, None, (QualType::UINT, QualType::INT));
        let st = enumeration(s, "S", true, Some(QualType::INT), (QualType::INT, QualType::INT));
        let i = lvalue(s, "i", QualType::INT);
        assert!(standard_conversion(s, i, et, ConvCtx::Initialization).is_none());
        let sv = lvalue(s, "sv", st);
        assert!(standard_conversion(s, sv, QualType::INT, ConvCtx::Initialization).is_none());
        assert!(standard_conversion(s, sv, QualType::BOOL, ConvCtx::Initialization).is_none());
        // Same enumeration: identity.
        let e = lvalue(s, "e", et);
        assert_eq!(init(s, e, et), ["'E' <LValueToRValue>"]);
    });
}

// ---------------------------------------------------------------------------
// [conv.double], [conv.fpint]
// ---------------------------------------------------------------------------

/// [conv.double]/1: `double d; float b = d; long double c = d;` and
/// `long double ld; double e = ld;`
/// `ImplicitCastExpr 'float' <FloatingCast>`
#[test]
fn floating_conversions_conv_double_1() {
    with_sema(|s, _| {
        for (from, fname, to, tname) in [
            (QualType::DOUBLE, "double", QualType::FLOAT, "float"),
            (QualType::DOUBLE, "double", QualType::LONGDOUBLE, "long double"),
            (QualType::LONGDOUBLE, "long double", QualType::DOUBLE, "double"),
            (QualType::FLOAT, "float", QualType::FLOAT16, "_Float16"),
            (QualType::FLOAT16, "_Float16", QualType::FLOAT, "float"),
        ] {
            let e = lvalue(s, "v", from);
            let c = scs(s, e, to);
            assert_eq!(c.second, SecondStep::FloatingConversion, "{fname} -> {tname}");
            assert_eq!(
                init(s, e, to),
                [format!("'{tname}' <FloatingCast>"), format!("'{fname}' <LValueToRValue>")]
            );
        }
    });
}

/// [conv.fpint]/1-2: `double d; int i = d;` and `int i; double g = i;`;
/// `float h = true;` ([conv.fpint]/2: `bool` converts like an integer).
/// `ImplicitCastExpr 'int' <FloatingToIntegral>`
/// `ImplicitCastExpr 'double' <IntegralToFloating>`
#[test]
fn floating_integral_conv_fpint() {
    with_sema(|s, _| {
        let d = lvalue(s, "d", QualType::DOUBLE);
        let c = scs(s, d, QualType::INT);
        assert_eq!((c.second, c.rank()), (SecondStep::FloatingIntegral, ConvRank::Conversion));
        assert_eq!(
            init(s, d, QualType::INT),
            ["'int' <FloatingToIntegral>", "'double' <LValueToRValue>"]
        );
        let i = lvalue(s, "i", QualType::INT);
        assert_eq!(
            init(s, i, QualType::DOUBLE),
            ["'double' <IntegralToFloating>", "'int' <LValueToRValue>"]
        );
        let t = bool_lit(s, true);
        assert_eq!(init(s, t, QualType::FLOAT), ["'float' <IntegralToFloating>"]);
        // An unscoped enumeration converts to floating ([conv.fpint]/2).
        let et = enumeration(s, "E", false, None, (QualType::UINT, QualType::INT));
        let e = lvalue(s, "e", et);
        assert_eq!(
            init(s, e, QualType::DOUBLE),
            ["'double' <IntegralToFloating>", "'E' <LValueToRValue>"]
        );
    });
}

// ---------------------------------------------------------------------------
// [conv.bool]
// ---------------------------------------------------------------------------

/// [conv.bool]/1: arithmetic, unscoped enumeration, pointer and member
/// pointer to `bool`.
/// `ImplicitCastExpr 'bool' <IntegralToBoolean>` / `<FloatingToBoolean>` /
/// `<PointerToBoolean>` / `<MemberPointerToBoolean>`
#[test]
fn boolean_conversions_conv_bool_1() {
    with_sema(|s, _| {
        let b = class(s, "B", &[]);
        let ip = ptr(s, QualType::INT);
        let ft = fn_type(s, QualType::VOID, &[]);
        let fp = ptr(s, ft);
        let mp = mem_ptr(s, QualType::INT, b);
        let et = enumeration(s, "E", false, None, (QualType::UINT, QualType::INT));
        for (t, tn, ck) in [
            (QualType::INT, "int", "IntegralToBoolean"),
            (QualType::LONG, "long", "IntegralToBoolean"),
            (et, "E", "IntegralToBoolean"),
            (QualType::DOUBLE, "double", "FloatingToBoolean"),
            (ip, "int *", "PointerToBoolean"),
            (fp, "void (*)()", "PointerToBoolean"),
            (mp, "int B::*", "MemberPointerToBoolean"),
        ] {
            let e = lvalue(s, "v", t);
            let c = scs(s, e, QualType::BOOL);
            assert_eq!(
                (c.second, c.rank()),
                (SecondStep::BooleanConversion, ConvRank::Conversion),
                "{tn}"
            );
            assert_eq!(
                init(s, e, QualType::BOOL),
                [format!("'bool' <{ck}>"), format!("'{tn}' <LValueToRValue>")]
            );
        }
    });
}

/// [conv.bool]/1: `std::nullptr_t` converts to `bool` only in
/// direct-initialization: `bool t4(nullptr);` is OK, `bool t = nullptr;`
/// is not.
/// `ImplicitCastExpr 'bool' <PointerToBoolean>`
#[test]
fn nullptr_to_bool_direct_only_conv_bool_1() {
    with_sema(|s, _| {
        let n = nullptr(s);
        assert!(standard_conversion(s, n, QualType::BOOL, ConvCtx::Initialization).is_none());
        let r =
            implicit_convert(s, n, QualType::BOOL, ConvCtx::DirectInitialization).expect("direct");
        assert_eq!(dump_casts(s, r), ["'bool' <PointerToBoolean>"]);
        let c =
            standard_conversion(s, n, QualType::BOOL, ConvCtx::DirectInitialization).expect("scs");
        assert_eq!(c.second, SecondStep::BooleanConversion);
    });
}

/// [conv.bool]: a scoped enumeration does not convert to `bool`
/// implicitly, even contextually ([dcl.enum]/10).
#[test]
fn scoped_enum_not_to_bool_conv_bool() {
    with_sema(|s, _| {
        let st = enumeration(s, "S", true, Some(QualType::INT), (QualType::INT, QualType::INT));
        let e = lvalue(s, "e", st);
        assert!(kiln_sema::conv::contextually_convert_to_bool(s, e).is_err());
    });
}

// ---------------------------------------------------------------------------
// [conv.ptr]
// ---------------------------------------------------------------------------

/// [conv.ptr]/1: `int* p0 = 0; int* p1 = nullptr;` — null pointer
/// conversions.
/// `ImplicitCastExpr 'int *' <NullToPointer>`
#[test]
fn null_pointer_conversion_conv_ptr_1() {
    with_sema(|s, _| {
        let ip = ptr(s, QualType::INT);
        let z = int_lit(s, QualType::INT, 0);
        let c = scs(s, z, ip);
        assert!(c.null_pointer);
        assert_eq!((c.second, c.rank()), (SecondStep::PointerConversion, ConvRank::Conversion));
        assert_eq!(init(s, z, ip), ["'int *' <NullToPointer>"]);
        let n = nullptr(s);
        assert_eq!(init(s, n, ip), ["'int *' <NullToPointer>"]);
        // `0L` and `(0)` are null pointer constants too.
        let zl = int_lit(s, QualType::LONG, 0);
        assert_eq!(init(s, zl, ip), ["'int *' <NullToPointer>"]);
        let z2 = int_lit(s, QualType::INT, 0);
        let pz = s.ast.builder().paren(z2, L, L);
        assert_eq!(init(s, pz, ip), ["'int *' <NullToPointer>"]);
        // `1` and a zero-valued variable are not.
        let one = int_lit(s, QualType::INT, 1);
        assert!(standard_conversion(s, one, ip, ConvCtx::Initialization).is_none());
        let zv = lvalue(s, "zero", QualType::INT);
        assert!(standard_conversion(s, zv, ip, ConvCtx::Initialization).is_none());
        // `false` is not a null pointer constant since C++11 (CWG903).
        let f = bool_lit(s, false);
        assert!(standard_conversion(s, f, ip, ConvCtx::Initialization).is_none());
    });
}

/// [conv.ptr]/2: `int* ip; void* v = ip; const void* cv = ip;`
/// `ImplicitCastExpr 'void *' <BitCast>`
#[test]
fn pointer_to_void_conv_ptr_2() {
    with_sema(|s, _| {
        let ip = ptr(s, QualType::INT);
        let vp = ptr(s, QualType::VOID);
        let e = lvalue(s, "ip", ip);
        let c = scs(s, e, vp);
        assert_eq!((c.second, c.third), (SecondStep::PointerConversion, ThirdStep::Identity));
        assert_eq!(init(s, e, vp), ["'void *' <BitCast>", "'int *' <LValueToRValue>"]);
        let cvoid = cnst(s, QualType::VOID);
        let cvp = ptr(s, cvoid);
        assert_eq!(init(s, e, cvp), ["'const void *' <BitCast>", "'int *' <LValueToRValue>"]);
        // const int* -> void* drops const: ill-formed.
        let ci = cnst(s, QualType::INT);
        let cip = ptr(s, ci);
        let ce = lvalue(s, "cip", cip);
        assert!(standard_conversion(s, ce, vp, ConvCtx::Initialization).is_none());
        // Function pointers do not convert to void* ([conv.ptr]/2: object
        // types only).
        let ft = fn_type(s, QualType::VOID, &[]);
        let fp = ptr(s, ft);
        let fe = lvalue(s, "fp", fp);
        assert!(standard_conversion(s, fe, vp, ConvCtx::Initialization).is_none());
    });
}

/// [conv.ptr]/3: `struct B {}; struct D : B {}; D* d; B* b = d;`
/// `ImplicitCastExpr 'B *' <DerivedToBase (B)>`
#[test]
fn derived_to_base_pointer_conv_ptr_3() {
    with_sema(|s, _| {
        let b = class(s, "B", &[]);
        let d = class(s, "D", &[(b, false)]);
        let dp = ptr(s, d);
        let bp = ptr(s, b);
        let e = lvalue(s, "d", dp);
        let c = scs(s, e, bp);
        assert_eq!(c.second, SecondStep::PointerConversion);
        assert_eq!(init(s, e, bp), ["'B *' <DerivedToBase (B)>", "'D *' <LValueToRValue>"]);
        // const D* -> const B*.
        let cd = cnst(s, d);
        let cdp = ptr(s, cd);
        let cb = cnst(s, b);
        let cbp = ptr(s, cb);
        let e = lvalue(s, "cd", cdp);
        assert_eq!(
            init(s, e, cbp),
            ["'const B *' <DerivedToBase (B)>", "'const D *' <LValueToRValue>"]
        );
        // Base-to-derived is not implicit.
        let e = lvalue(s, "b", bp);
        assert!(standard_conversion(s, e, dp, ConvCtx::Initialization).is_none());
    });
}

/// [conv.ptr]/3: `D* d; const void* cv2 = d;` — to `cv void*` is a
/// `BitCast` even for class pointers.
#[test]
fn class_pointer_to_void_conv_ptr_2() {
    with_sema(|s, _| {
        let b = class(s, "B", &[]);
        let d = class(s, "D", &[(b, false)]);
        let dp = ptr(s, d);
        let cvoid = cnst(s, QualType::VOID);
        let cvp = ptr(s, cvoid);
        let e = lvalue(s, "d", dp);
        assert_eq!(init(s, e, cvp), ["'const void *' <BitCast>", "'D *' <LValueToRValue>"]);
    });
}

// ---------------------------------------------------------------------------
// [conv.mem]
// ---------------------------------------------------------------------------

/// [conv.mem]/1: `int D::* m0 = 0; int D::* m1 = nullptr;`
/// `ImplicitCastExpr 'int D::*' <NullToMemberPointer>`
#[test]
fn null_member_pointer_conv_mem_1() {
    with_sema(|s, _| {
        let d = class(s, "D", &[]);
        let mp = mem_ptr(s, QualType::INT, d);
        let z = int_lit(s, QualType::INT, 0);
        let c = scs(s, z, mp);
        assert!(c.null_pointer);
        assert_eq!(c.second, SecondStep::MemberPointerConversion);
        assert_eq!(init(s, z, mp), ["'int D::*' <NullToMemberPointer>"]);
        let n = nullptr(s);
        assert_eq!(init(s, n, mp), ["'int D::*' <NullToMemberPointer>"]);
    });
}

/// [conv.mem]/2: `int B::* bm; int D::* m2 = bm;`
/// `ImplicitCastExpr 'int D::*' <BaseToDerivedMemberPointer (B)>`
#[test]
fn base_to_derived_member_pointer_conv_mem_2() {
    with_sema(|s, _| {
        let b = class(s, "B", &[]);
        let d = class(s, "D", &[(b, false)]);
        let bm = mem_ptr(s, QualType::INT, b);
        let dm = mem_ptr(s, QualType::INT, d);
        let e = lvalue(s, "bm", bm);
        let c = scs(s, e, dm);
        assert_eq!(
            (c.second, c.rank()),
            (SecondStep::MemberPointerConversion, ConvRank::Conversion)
        );
        assert_eq!(
            init(s, e, dm),
            ["'int D::*' <BaseToDerivedMemberPointer (B)>", "'int B::*' <LValueToRValue>"]
        );
        // The reverse direction is not implicit.
        let e = lvalue(s, "dm", dm);
        assert!(standard_conversion(s, e, bm, ConvCtx::Initialization).is_none());
        // Different member types do not convert.
        let lm = mem_ptr(s, QualType::LONG, d);
        let e = lvalue(s, "bm2", bm);
        assert!(standard_conversion(s, e, lm, ConvCtx::Initialization).is_none());
    });
}

// ---------------------------------------------------------------------------
// [conv.fctptr]
// ---------------------------------------------------------------------------

/// [conv.fctptr]/1: `void (*p)() noexcept; void (*q)() = p;` and
/// `void (B::*mp)() noexcept; void (B::*mq)() = mp;`
/// `ImplicitCastExpr 'void (*)()' <NoOp>`
#[test]
fn function_pointer_conversion_conv_fctptr_1() {
    with_sema(|s, _| {
        let nft = noexcept_fn_type(s, QualType::VOID, &[]);
        let ft = fn_type(s, QualType::VOID, &[]);
        let np = ptr(s, nft);
        let fp = ptr(s, ft);
        let e = lvalue(s, "p", np);
        let c = scs(s, e, fp);
        assert_eq!(
            (c.second, c.third, c.rank()),
            (SecondStep::Identity, ThirdStep::FunctionPointer, ConvRank::ExactMatch)
        );
        assert_eq!(
            init(s, e, fp),
            ["'void (*)()' <NoOp>", "'void (*)() noexcept' <LValueToRValue>"]
        );
        // The reverse adds noexcept: ill-formed.
        let e2 = lvalue(s, "q", fp);
        assert!(standard_conversion(s, e2, np, ConvCtx::Initialization).is_none());

        let b = class(s, "B", &[]);
        let nmp = mem_ptr(s, nft, b);
        let mp = mem_ptr(s, ft, b);
        let e = lvalue(s, "mp", nmp);
        assert_eq!(
            init(s, e, mp),
            ["'void (B::*)()' <NoOp>", "'void (B::*)() noexcept' <LValueToRValue>"]
        );
    });
}

// ---------------------------------------------------------------------------
// Value categories of the results.
// ---------------------------------------------------------------------------

/// [conv.general]/6: every standard conversion yields a prvalue.
#[test]
fn results_are_prvalues_conv_general_6() {
    with_sema(|s, _| {
        let x = lvalue(s, "x", QualType::SHORT);
        let r = implicit_convert(s, x, QualType::LONG, ConvCtx::Initialization).unwrap();
        let (k, ck, t, vk) = shape(s, r);
        assert_eq!(
            (k, ck, t, vk),
            (
                ExprKind::ImplicitCastExpr,
                Some(CastKind::IntegralCast),
                QualType::LONG,
                ValueCategory::PRValue
            )
        );
    });
}
