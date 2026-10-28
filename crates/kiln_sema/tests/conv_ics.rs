//! Standard conversion sequences ([over.ics.scs]) and their ranking
//! ([over.ics.rank]/3.2, /4). Each comparison names the overload set it
//! decides; clang 19 picks the overload the expectation says is better.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "short names mirror the C++ under test"
)]

mod conv_common;

use conv_common::*;
use kiln_ast::{ExprId, QualType};
use kiln_sema::Sema;
use kiln_sema::conv::{
    ConvCtx, ConvRank, FirstStep, FixedEnumPromotion, IcsCompare, SecondStep, StandardConversion,
    ThirdStep, compare_standard_conversions, standard_conversion,
};

fn scs(s: &mut Sema<'_, '_>, e: ExprId, to: QualType) -> StandardConversion {
    standard_conversion(s, e, to, ConvCtx::Argument).expect("a standard conversion")
}

/// Compares `e -> a` with `e -> b`, checking antisymmetry.
fn cmp(s: &mut Sema<'_, '_>, e: ExprId, a: QualType, b: QualType) -> IcsCompare {
    let x = scs(s, e, a);
    let y = scs(s, e, b);
    let r = compare_standard_conversions(s, &x, &y);
    let back = compare_standard_conversions(s, &y, &x);
    let expect_back = match r {
        IcsCompare::Better => IcsCompare::Worse,
        IcsCompare::Worse => IcsCompare::Better,
        IcsCompare::Indistinguishable => IcsCompare::Indistinguishable,
    };
    assert_eq!(back, expect_back, "antisymmetry");
    r
}

/// Compares `e1 -> t1` with `e2 -> t2`.
fn cmp2(s: &mut Sema<'_, '_>, e1: ExprId, t1: QualType, e2: ExprId, t2: QualType) -> IcsCompare {
    let x = scs(s, e1, t1);
    let y = scs(s, e2, t2);
    compare_standard_conversions(s, &x, &y)
}

// ---------------------------------------------------------------------------
// [over.ics.scs]
// ---------------------------------------------------------------------------

/// [over.ics.scs]/3, Table 19: categories and ranks of each step.
#[test]
fn table_of_ranks_over_ics_scs_3() {
    with_sema(|s, _| {
        let b = class(s, "B", &[]);
        let d = class(s, "D", &[(b, false)]);
        let dp = ptr(s, d);
        let bp = ptr(s, b);
        let ip = ptr(s, QualType::INT);
        let ci = cnst(s, QualType::INT);
        let cip = ptr(s, ci);
        let bm = mem_ptr(s, QualType::INT, b);
        let dm = mem_ptr(s, QualType::INT, d);
        let i = lvalue(s, "i", QualType::INT);
        let sh = lvalue(s, "sh", QualType::SHORT);
        let f = lvalue(s, "f", QualType::FLOAT);
        let dv = lvalue(s, "dv", QualType::DOUBLE);
        let pv = lvalue(s, "p", ip);
        let dpv = lvalue(s, "dp", dp);
        let bmv = lvalue(s, "bm", bm);
        let cases: Vec<(ExprId, QualType, SecondStep, ThirdStep, ConvRank)> = vec![
            (i, QualType::INT, SecondStep::Identity, ThirdStep::Identity, ConvRank::ExactMatch),
            (pv, cip, SecondStep::Identity, ThirdStep::Qualification, ConvRank::ExactMatch),
            (
                sh,
                QualType::INT,
                SecondStep::IntegralPromotion,
                ThirdStep::Identity,
                ConvRank::Promotion,
            ),
            (
                f,
                QualType::DOUBLE,
                SecondStep::FloatingPromotion,
                ThirdStep::Identity,
                ConvRank::Promotion,
            ),
            (
                i,
                QualType::LONG,
                SecondStep::IntegralConversion,
                ThirdStep::Identity,
                ConvRank::Conversion,
            ),
            (
                dv,
                QualType::FLOAT,
                SecondStep::FloatingConversion,
                ThirdStep::Identity,
                ConvRank::Conversion,
            ),
            (
                dv,
                QualType::INT,
                SecondStep::FloatingIntegral,
                ThirdStep::Identity,
                ConvRank::Conversion,
            ),
            (dpv, bp, SecondStep::PointerConversion, ThirdStep::Identity, ConvRank::Conversion),
            (
                bmv,
                dm,
                SecondStep::MemberPointerConversion,
                ThirdStep::Identity,
                ConvRank::Conversion,
            ),
            (
                i,
                QualType::BOOL,
                SecondStep::BooleanConversion,
                ThirdStep::Identity,
                ConvRank::Conversion,
            ),
        ];
        for (k, (e, to, second, third, rank)) in cases.into_iter().enumerate() {
            let c = scs(s, e, to);
            assert_eq!(
                (c.first, c.second, c.third, c.rank()),
                (FirstStep::LValueToRValue, second, third, rank),
                "case {k}"
            );
            assert_eq!(c.from, s.ast.exprs.get(e).ty(), "case {k}");
        }
    });
}

/// [over.ics.scs]: `to_types` records the type after each step: `short s;`
/// to `const long*`... e.g. `int a[2]` to `const int*` is
/// `int*` (after decay), `int*` (no second step), `const int*`.
#[test]
fn to_types_after_each_step_over_ics_scs() {
    with_sema(|s, _| {
        let at = array(s, QualType::INT, 2);
        let a = lvalue(s, "a", at);
        let ip = ptr(s, QualType::INT);
        let ci = cnst(s, QualType::INT);
        let cip = ptr(s, ci);
        let c = scs(s, a, cip);
        assert!(s.ast.types.same_type(c.to_types[0], ip));
        assert!(s.ast.types.same_type(c.to_types[1], ip));
        assert!(s.ast.types.same_type(c.to_types[2], cip));
        let sh = lvalue(s, "sh", QualType::SHORT);
        let c = scs(s, sh, QualType::LONG);
        assert_eq!(c.to_types, [QualType::SHORT, QualType::LONG, QualType::LONG]);
    });
}

// ---------------------------------------------------------------------------
// [over.ics.rank]/3.2.1: proper subsequence
// ---------------------------------------------------------------------------

/// [over.ics.rank]/3.2.1: identity is a proper subsequence of any
/// non-identity sequence: `void f(int*); void f(const int*); int* p;
/// f(p);` calls `f(int*)`.
#[test]
fn identity_is_subsequence_over_ics_rank_3_2_1() {
    with_sema(|s, _| {
        let ip = ptr(s, QualType::INT);
        let ci = cnst(s, QualType::INT);
        let cip = ptr(s, ci);
        let p = lvalue(s, "p", ip);
        assert_eq!(cmp(s, p, ip, cip), IcsCompare::Better);
        // Lvalue transformations do not count: `int a[2]` to `int*` vs
        // `const int*`.
        let at = array(s, QualType::INT, 2);
        let a = lvalue(s, "a", at);
        assert_eq!(cmp(s, a, ip, cip), IcsCompare::Better);
    });
}

// ---------------------------------------------------------------------------
// [over.ics.rank]/3.2.2: rank
// ---------------------------------------------------------------------------

/// [over.ics.rank]/3.2.2: Exact Match beats Promotion beats Conversion:
/// `void f(int); void f(long); short s; f(s);` calls `f(int)`;
/// `void g(double); void g(int); float x; g(x);` calls `g(double)`.
#[test]
fn rank_orders_over_ics_rank_3_2_2() {
    with_sema(|s, _| {
        let sh = lvalue(s, "sh", QualType::SHORT);
        assert_eq!(cmp(s, sh, QualType::INT, QualType::LONG), IcsCompare::Better);
        assert_eq!(cmp(s, sh, QualType::SHORT, QualType::INT), IcsCompare::Better);
        let f = lvalue(s, "f", QualType::FLOAT);
        assert_eq!(cmp(s, f, QualType::DOUBLE, QualType::INT), IcsCompare::Better);
        assert_eq!(cmp(s, f, QualType::DOUBLE, QualType::LONGDOUBLE), IcsCompare::Better);
        // Two conversions of equal rank: `void h(long); void h(unsigned);
        // h(1);` is ambiguous.
        let i = lvalue(s, "i", QualType::INT);
        assert_eq!(cmp(s, i, QualType::LONG, QualType::UINT), IcsCompare::Indistinguishable);
        assert_eq!(cmp(s, i, QualType::DOUBLE, QualType::LONG), IcsCompare::Indistinguishable);
    });
}

// ---------------------------------------------------------------------------
// [over.ics.rank]/3.2.5: qualification
// ---------------------------------------------------------------------------

/// [over.ics.rank]/3.2.5: `void f(const int*); void f(const volatile int*);
/// int* p; f(p);` calls `f(const int*)`.
#[test]
fn fewer_qualifiers_better_over_ics_rank_3_2_5() {
    with_sema(|s, _| {
        let ip = ptr(s, QualType::INT);
        let ci = cnst(s, QualType::INT);
        let cvi = vol(s, ci);
        let cip = ptr(s, ci);
        let cvip = ptr(s, cvi);
        let p = lvalue(s, "p", ip);
        assert_eq!(cmp(s, p, cip, cvip), IcsCompare::Better);
        // `const int*` vs `volatile int*`: neither is convertible to the
        // other: ambiguous.
        let vi = vol(s, QualType::INT);
        let vip = ptr(s, vi);
        assert_eq!(cmp(s, p, cip, vip), IcsCompare::Indistinguishable);
    });
}

// ---------------------------------------------------------------------------
// [over.ics.rank]/4.1: pointer to bool
// ---------------------------------------------------------------------------

/// [over.ics.rank]/4.1: a conversion not converting a pointer, pointer to
/// member or `std::nullptr_t` to `bool` is better than one that does:
/// `void f(bool); void f(void*); int* p; f(p);` calls `f(void*)`.
#[test]
fn pointer_to_bool_worse_over_ics_rank_4_1() {
    with_sema(|s, _| {
        let ip = ptr(s, QualType::INT);
        let vp = ptr(s, QualType::VOID);
        let p = lvalue(s, "p", ip);
        assert_eq!(cmp(s, p, vp, QualType::BOOL), IcsCompare::Better);
        // Through array decay: `int a[2]; f(a);`.
        let at = array(s, QualType::INT, 2);
        let a = lvalue(s, "a", at);
        assert_eq!(cmp(s, a, vp, QualType::BOOL), IcsCompare::Better);
        // Member pointer to bool vs ... `void g(bool); void g(int D::*);`
        let b = class(s, "B", &[]);
        let d = class(s, "D", &[(b, false)]);
        let bm = mem_ptr(s, QualType::INT, b);
        let dm = mem_ptr(s, QualType::INT, d);
        let m = lvalue(s, "m", bm);
        assert_eq!(cmp(s, m, dm, QualType::BOOL), IcsCompare::Better);
        // Integer to bool is not pointer-to-bool: `void h(bool); void
        // h(long); h(1);` is ambiguous.
        let i = lvalue(s, "i", QualType::INT);
        assert_eq!(cmp(s, i, QualType::BOOL, QualType::LONG), IcsCompare::Indistinguishable);
    });
}

// ---------------------------------------------------------------------------
// [over.ics.rank]/4.2: fixed-underlying enumeration promotion
// ---------------------------------------------------------------------------

/// [over.ics.rank]/4.2: `enum E : short {}; void f(short); void f(int);
/// E e; f(e);` calls `f(short)`: promotion to the underlying type beats
/// promotion to its promoted type.
#[test]
fn fixed_enum_to_underlying_over_ics_rank_4_2() {
    with_sema(|s, _| {
        let et =
            enumeration(s, "E", false, Some(QualType::SHORT), (QualType::SHORT, QualType::INT));
        let e = lvalue(s, "e", et);
        let a = scs(s, e, QualType::SHORT);
        let b = scs(s, e, QualType::INT);
        assert_eq!(
            (a.second, a.fixed_enum),
            (SecondStep::IntegralPromotion, FixedEnumPromotion::ToUnderlying)
        );
        assert_eq!(
            (b.second, b.fixed_enum),
            (SecondStep::IntegralPromotion, FixedEnumPromotion::ToPromotedUnderlying)
        );
        assert_eq!(cmp(s, e, QualType::SHORT, QualType::INT), IcsCompare::Better);
        // An enum without a fixed type: `enum G { g0 }; f(g)` to int is a
        // promotion, to short a conversion.
        let gt = enumeration(s, "G", false, None, (QualType::UINT, QualType::INT));
        let g = lvalue(s, "g", gt);
        assert_eq!(scs(s, g, QualType::INT).fixed_enum, FixedEnumPromotion::None);
        assert_eq!(cmp(s, g, QualType::INT, QualType::SHORT), IcsCompare::Better);
    });
}

// ---------------------------------------------------------------------------
// [over.ics.rank]/4.3: floating-point types of equal rank (C++23)
// ---------------------------------------------------------------------------

/// [over.ics.rank]/4.3: on aarch64 `long double` and `__float128` share a
/// format, hence a conversion rank: `void f(__float128); void f(int);
/// long double x; f(x);` calls `f(__float128)`.
#[test]
fn equal_rank_floating_over_ics_rank_4_3() {
    with_target(aarch64(), |s, _| {
        let x = lvalue(s, "x", QualType::LONGDOUBLE);
        assert_eq!(cmp(s, x, QualType::FLOAT128, QualType::INT), IcsCompare::Better);
        assert_eq!(cmp(s, x, QualType::FLOAT128, QualType::DOUBLE), IcsCompare::Better);
    });
    // On x86-64 they differ in rank: both are plain conversions.
    with_sema(|s, _| {
        let x = lvalue(s, "x", QualType::LONGDOUBLE);
        assert_eq!(cmp(s, x, QualType::FLOAT128, QualType::INT), IcsCompare::Indistinguishable);
    });
}

// ---------------------------------------------------------------------------
// [over.ics.rank]/4.4: derivation
// ---------------------------------------------------------------------------

/// [over.ics.rank]/4.4.1-2: `struct A {}; struct B : A {}; struct C : B {};`
/// with `void f(A*); void f(void*); B* b; f(b);` calling `f(A*)`, and
/// `void g(A*); void g(B*); C* c; g(c);` calling `g(B*)`.
#[test]
fn derived_pointer_conversions_over_ics_rank_4_4() {
    with_sema(|s, _| {
        let a = class(s, "A", &[]);
        let b = class(s, "B", &[(a, false)]);
        let c = class(s, "C", &[(b, false)]);
        let (ap, bp, cp) = (ptr(s, a), ptr(s, b), ptr(s, c));
        let vp = ptr(s, QualType::VOID);
        let bv = lvalue(s, "b", bp);
        // /4.4.1: B* -> A* better than B* -> void*.
        assert_eq!(cmp(s, bv, ap, vp), IcsCompare::Better);
        // /4.4.3: C* -> B* better than C* -> A*.
        let cv = lvalue(s, "c", cp);
        assert_eq!(cmp(s, cv, bp, ap), IcsCompare::Better);
        // /4.4.2: A* -> void* better than B* -> void* (different sources:
        // [over.ics.rank]/4.4.2 compares ICSs of different arguments).
        let av = lvalue(s, "a", ap);
        let bv2 = lvalue(s, "b2", bp);
        assert_eq!(cmp2(s, bv2, vp, av, vp), IcsCompare::Better);
        // /4.4.6: B* -> A* better than C* -> A*.
        let bv3 = lvalue(s, "b3", bp);
        let cv3 = lvalue(s, "c3", cp);
        assert_eq!(cmp2(s, bv3, ap, cv3, ap), IcsCompare::Better);
        assert_eq!(cmp2(s, cv3, ap, bv3, ap), IcsCompare::Worse);
    });
}

/// [over.ics.rank]/4.4.4, 4.4.7: member pointers: `int A::*` to `int B::*`
/// beats `int A::*` to `int C::*`; `int B::*` to `int C::*` beats
/// `int A::*` to `int C::*`.
#[test]
fn derived_member_pointer_conversions_over_ics_rank_4_4() {
    with_sema(|s, _| {
        let a = class(s, "A", &[]);
        let b = class(s, "B", &[(a, false)]);
        let c = class(s, "C", &[(b, false)]);
        let am = mem_ptr(s, QualType::INT, a);
        let bm = mem_ptr(s, QualType::INT, b);
        let cm = mem_ptr(s, QualType::INT, c);
        let m = lvalue(s, "m", am);
        assert_eq!(cmp(s, m, bm, cm), IcsCompare::Better);
        let mb = lvalue(s, "mb", bm);
        let ma = lvalue(s, "ma", am);
        assert_eq!(cmp2(s, mb, cm, ma, cm), IcsCompare::Better);
    });
}

/// [over.ics.rank]: null pointer conversions are not ordered by derivation:
/// `void f(A*); void f(B*); f(0);` is ambiguous (neither is better).
#[test]
fn null_pointer_not_ordered_over_ics_rank_4_4() {
    with_sema(|s, _| {
        let a = class(s, "A", &[]);
        let b = class(s, "B", &[(a, false)]);
        let (ap, bp) = (ptr(s, a), ptr(s, b));
        let z = int_lit(s, QualType::INT, 0);
        assert_eq!(cmp(s, z, ap, bp), IcsCompare::Indistinguishable);
        // `f(void*)` vs `f(A*)` with `0`: still ambiguous ([over.ics.rank]
        // /4.4.1 is about pointers to class types being converted).
        let vp = ptr(s, QualType::VOID);
        assert_eq!(cmp(s, z, ap, vp), IcsCompare::Indistinguishable);
    });
}
