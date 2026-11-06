//! The Oven's trial-fold fast path (spec/oven.md "Trial folds", WP-656):
//! for a generated set of integer expressions (every integer type, both
//! signs, every builtin unary and binary integer operator, boundary
//! values, overflow, `/ 0`, shifts by the width and beyond, nested
//! casts, `?:`, `&&`/`||`, `sizeof`/`alignof`, enumerators and casts to
//! enumerations), the fast path's answer is the full path's: the same
//! value, or `None` from both. A cast to an enumeration is outside the
//! domain (the full path checks the enumeration's range): the fast path
//! declines it.

#![allow(clippy::many_single_char_names, reason = "short names mirror the C++ under test")]

mod conv_common;

use conv_common::*;
use kiln_ast::consts::{IntBits, Scalar};
use kiln_ast::expr::{BinaryOp, TraitKind, UnaryOp};
use kiln_ast::types::BuiltinType as B;
use kiln_ast::{ConstValueId, DeclHead, ExprId, QualType};
use kiln_sema::Sema;
use kiln_sema::expr::CastStyle;

/// The integer types, with their widths and signedness on x86-64.
const INTS: [(B, u32, bool); 18] = [
    (B::Bool, 1, false),
    (B::Char, 8, true),
    (B::SChar, 8, true),
    (B::UChar, 8, false),
    (B::WChar, 32, true),
    (B::Char8, 8, false),
    (B::Char16, 16, false),
    (B::Char32, 32, false),
    (B::Short, 16, true),
    (B::UShort, 16, false),
    (B::Int, 32, true),
    (B::UInt, 32, false),
    (B::Long, 64, true),
    (B::ULong, 64, false),
    (B::LongLong, 64, true),
    (B::ULongLong, 64, false),
    (B::Int128, 128, true),
    (B::UInt128, 128, false),
];

/// Operand values: `(magnitude, negated)`.
const VALUES: [(u64, bool); 24] = [
    (0, false),
    (1, false),
    (2, false),
    (7, false),
    (8, false),
    (15, false),
    (16, false),
    (31, false),
    (32, false),
    (63, false),
    (64, false),
    (127, false),
    (128, false),
    (0x80, false),
    (0xFF, false),
    (0x7FFF, false),
    (0xFFFF, false),
    (0x7FFF_FFFF, false),
    (0x8000_0000, false),
    (0xFFFF_FFFF, false),
    (i64::MAX as u64, false),
    (u64::MAX, false),
    (1, true),
    (0x8000_0000, true),
];

/// Binary operators with an integer result.
const BINARY: [BinaryOp; 18] = [
    BinaryOp::Mul,
    BinaryOp::Div,
    BinaryOp::Rem,
    BinaryOp::Add,
    BinaryOp::Sub,
    BinaryOp::Shl,
    BinaryOp::Shr,
    BinaryOp::LT,
    BinaryOp::GT,
    BinaryOp::LE,
    BinaryOp::GE,
    BinaryOp::EQ,
    BinaryOp::NE,
    BinaryOp::And,
    BinaryOp::Xor,
    BinaryOp::Or,
    BinaryOp::LAnd,
    BinaryOp::LOr,
];

#[derive(Default)]
struct Tally {
    some: usize,
    none: usize,
}

fn shown(s: &Sema<'_, '_>, v: Option<ConstValueId>) -> Option<String> {
    v.map(|v| s.ast.consts.scalar(v).map_or_else(|| "<not a scalar>".into(), |x| format!("{x:?}")))
}

/// The fast path's answer for `e` is the full path's.
fn check(s: &mut Sema<'_, '_>, e: ExprId, what: &dyn Fn() -> String, t: &mut Tally) {
    let (fast, full) = s.trial_fold_paths(e);
    let (a, b) = (shown(s, fast), shown(s, full));
    assert_eq!(a, b, "fast vs full path for {}", what());
    tally(a.is_some(), t);
}

/// Where the fast path answers, its answer is the full path's (it may
/// decline what the full path folds).
fn check_sound(s: &mut Sema<'_, '_>, e: ExprId, what: &dyn Fn() -> String, t: &mut Tally) {
    let (fast, full) = s.trial_fold_paths(e);
    let (a, b) = (shown(s, fast), shown(s, full));
    if a.is_some() {
        assert_eq!(a, b, "fast vs full path for {}", what());
    }
    tally(a.is_some(), t);
}

fn tally(some: bool, t: &mut Tally) {
    if some {
        t.some += 1;
    } else {
        t.none += 1;
    }
}

/// `static_cast<T>(v)` of an `unsigned long long` literal, or of
/// `-(int)v` when negated.
fn operand(s: &mut Sema<'_, '_>, ty: B, (v, neg): (u64, bool)) -> ExprId {
    let lit = if neg {
        let l = int_lit(s, QualType::LONGLONG, v);
        s.act_on_unary(UnaryOp::Minus, l, L)
    } else {
        int_lit(s, QualType::ULONGLONG, v)
    };
    s.act_on_cast(CastStyle::Static, builtin(ty), lit, L, L)
}

fn name_of(ty: B, (v, neg): (u64, bool)) -> String {
    format!("({}){}{v:#x}", ty.name(), if neg { "-" } else { "" })
}

/// Literals of every integer type and their casts to every other: the
/// conversions of [conv.integral] and [conv.bool].
#[test]
fn casts_agree() {
    with_sema(|s, _| {
        let mut t = Tally::default();
        for &(from, ..) in &INTS {
            for &v in &VALUES {
                let x = operand(s, from, v);
                check(s, x, &|| name_of(from, v), &mut t);
                for &(to, ..) in &INTS {
                    let x = operand(s, from, v);
                    let c = s.act_on_cast(CastStyle::CStyle, builtin(to), x, L, L);
                    check(s, c, &|| format!("({}){}", to.name(), name_of(from, v)), &mut t);
                    // A nested round trip through the narrowest types.
                    let c = s.act_on_cast(CastStyle::Static, builtin(B::SChar), c, L, L);
                    let c = s.act_on_cast(CastStyle::Static, builtin(from), c, L, L);
                    check(s, c, &|| format!("round trip via {}", to.name()), &mut t);
                }
            }
        }
        assert_eq!(t.none, 0, "every literal conversion folds");
    });
}

/// `+x`, `-x` (overflow at the minimum), `~x`, `!x` of every operand.
#[test]
fn unary_agrees() {
    with_sema(|s, _| {
        let mut t = Tally::default();
        for &(ty, ..) in &INTS {
            for &v in &VALUES {
                for op in [UnaryOp::Plus, UnaryOp::Minus, UnaryOp::Not, UnaryOp::LNot] {
                    let x = operand(s, ty, v);
                    let e = s.act_on_unary(op, x, L);
                    check(s, e, &|| format!("{op:?} {}", name_of(ty, v)), &mut t);
                    let e = s.act_on_paren(e, L, L);
                    let e = s.act_on_unary(op, e, L);
                    check(s, e, &|| format!("{op:?} {op:?} {}", name_of(ty, v)), &mut t);
                }
            }
        }
        assert!(t.some > 0 && t.none > 0, "both outcomes: {} some, {} none", t.some, t.none);
    });
}

/// Every binary integer operator over pairs of operands of mixed types
/// (the usual arithmetic conversions): overflow, `/ 0`, `INT_MIN / -1`,
/// shifts by negative counts, by the width and beyond.
#[test]
fn binary_agrees() {
    const TYPES: [B; 11] = [
        B::Bool,
        B::Char,
        B::UChar,
        B::Short,
        B::UShort,
        B::Int,
        B::UInt,
        B::Long,
        B::ULong,
        B::Int128,
        B::UInt128,
    ];
    const VS: [(u64, bool); 8] = [
        (0, false),
        (1, false),
        (31, false),
        (64, false),
        (0x8000_0000, false),
        (u64::MAX, false),
        (1, true),
        (0x8000_0000, true),
    ];
    with_sema(|s, _| {
        let mut t = Tally::default();
        for &ta in &TYPES {
            for &va in &VS {
                for &tb in &TYPES {
                    for &vb in &VS {
                        for op in BINARY {
                            let a = operand(s, ta, va);
                            let b = operand(s, tb, vb);
                            let e = s.act_on_binary(op, a, b, L);
                            check(
                                s,
                                e,
                                &|| format!("{} {op:?} {}", name_of(ta, va), name_of(tb, vb)),
                                &mut t,
                            );
                        }
                    }
                }
            }
        }
        assert!(t.some > 0 && t.none > 0, "both outcomes: {} some, {} none", t.some, t.none);
    });
}

/// `c ? a : b` (the full path folds past a failing arm it does not
/// run; the fast path declines when either arm fails), `&&`/`||` over a
/// failing operand, and arithmetic spines deeper than the fast path
/// walks.
#[test]
fn conditional_and_depth_agree() {
    with_sema(|s, _| {
        let mut t = Tally::default();
        for c in [0, 1] {
            for (x, y) in [(0u64, 1u64), (5, 0), (u64::MAX, 3)] {
                let cond = int_lit(s, QualType::INT, c);
                let a = int_lit(s, QualType::INT, 10);
                let z = int_lit(s, QualType::INT, x);
                let a = s.act_on_binary(BinaryOp::Div, a, z, L);
                let b = int_lit(s, QualType::UINT, y);
                let e = s.act_on_conditional(cond, a, b, L);
                check_sound(s, e, &|| format!("{c} ? 10 / {x} : {y}u"), &mut t);
            }
        }
        for op in [BinaryOp::LAnd, BinaryOp::LOr] {
            for c in [0, 1] {
                let a = int_lit(s, QualType::INT, c);
                let one = int_lit(s, QualType::INT, 1);
                let z = int_lit(s, QualType::INT, 0);
                let b = s.act_on_binary(BinaryOp::Div, one, z, L);
                let e = s.act_on_binary(op, a, b, L);
                check_sound(s, e, &|| format!("{c} {op:?} 1 / 0"), &mut t);
            }
        }
        for n in [8, 63, 64, 65, 66, 200] {
            let mut e = int_lit(s, QualType::INT, 1);
            for _ in 0..n {
                let one = int_lit(s, QualType::INT, 1);
                e = s.act_on_binary(BinaryOp::Add, e, one, L);
            }
            check_sound(s, e, &|| format!("left spine of {n}"), &mut t);
            assert_eq!(n > 64, s.trial_fold_paths(e).0.is_none(), "left spine of {n}");
            let mut e = int_lit(s, QualType::INT, 1);
            for _ in 0..n {
                let one = int_lit(s, QualType::INT, 1);
                let p = s.act_on_paren(e, L, L);
                e = s.act_on_binary(BinaryOp::Add, one, p, L);
            }
            check_sound(s, e, &|| format!("right spine of {n}"), &mut t);
        }
        assert!(t.some > 0, "{} some", t.some);
    });
}

/// `sizeof`, `alignof` and `__alignof` of types, `sizeof` of an
/// expression, and arithmetic on them.
#[test]
fn traits_agree() {
    with_sema(|s, _| {
        let mut t = Tally::default();
        let mut types: Vec<QualType> = INTS.iter().map(|&(b, ..)| builtin(b)).collect();
        types.extend([QualType::FLOAT, QualType::DOUBLE, QualType::LONGDOUBLE]);
        let p = ptr(s, QualType::INT);
        let arr = array(s, QualType::SHORT, 7);
        let r = lref(s, QualType::DOUBLE);
        let td = typedef(s, "T", QualType::LONG);
        types.extend([p, arr, r, td]);
        for &q in &types {
            for k in [TraitKind::SizeOf, TraitKind::AlignOf, TraitKind::PreferredAlignOf] {
                let e = s.act_on_trait_type(k, q, L, L);
                check(s, e, &|| format!("{k:?}({q:?})"), &mut t);
                let e = s.act_on_trait_type(k, q, L, L);
                let three = int_lit(s, QualType::INT, 3);
                let e = s.act_on_binary(BinaryOp::Mul, e, three, L);
                let one = int_lit(s, QualType::INT, 1);
                let e = s.act_on_binary(BinaryOp::Sub, e, one, L);
                check(s, e, &|| format!("{k:?}({q:?}) * 3 - 1"), &mut t);
            }
        }
        let x = int_lit(s, QualType::SHORT, 3);
        let e = s.act_on_trait_expr(TraitKind::SizeOf, x, L);
        check(s, e, &|| "sizeof(short lit)".into(), &mut t);
        assert_eq!(t.none, 0, "every trait folds");
    });
}

/// Enumerators (with values at the underlying type's bounds) in
/// arithmetic, and casts to enumerations, which the fast path declines.
#[test]
fn enumerations_agree() {
    with_sema(|s, _| {
        let mut t = Tally::default();
        for (n, w, signed, fixed) in [("E", 8, true, true), ("F", 32, false, false)] {
            let under = if signed { QualType::SCHAR } else { QualType::UINT };
            let prom = if signed { QualType::INT } else { QualType::UINT };
            let h = DeclHead::new(s.ast.tu, name(s, n), L);
            let ed = s.ast.builder().enumeration(h, 0, fixed.then_some((under, prom)), None);
            s.ast.builder().begin_enum_definition(ed).expect("first definition");
            let q = s.ast.decls.get(ed).ty().expect("enum type");
            let mut refs = Vec::new();
            for (i, v) in [0i128, 1, 5, -1, 127, -128].into_iter().enumerate() {
                if !signed && v < 0 {
                    continue;
                }
                let h = DeclHead::new(ed, name(s, &format!("{n}{i}")), L);
                let d = s.ast.builder().enumerator(h, None);
                let bits = IntBits::from_u128(w, v as u128);
                let cv =
                    s.ast.consts.push_scalar(Scalar::Int { ty: q, signed, bits }).expect("intern");
                s.ast.decls.set_enumerator_value(d, cv).expect("first value");
                refs.push((d, v));
            }
            let comp = (!fixed).then_some((under, prom));
            s.ast.builder().complete_enum_definition(ed, comp).expect("types once");
            for &(d, v) in &refs {
                let e = s.build_decl_ref(d, L);
                check(s, e, &|| format!("enumerator {v}"), &mut t);
                for op in [BinaryOp::Add, BinaryOp::Mul, BinaryOp::Shl, BinaryOp::LT] {
                    let e = s.build_decl_ref(d, L);
                    let k = int_lit(s, QualType::INT, 3);
                    let e = s.act_on_binary(op, e, k, L);
                    check(s, e, &|| format!("enumerator {v} {op:?} 3"), &mut t);
                }
            }
            for v in [0u64, 1, 5, 200, 0xFFFF_FFFF] {
                let x = int_lit(s, QualType::INT, v);
                let e = s.act_on_cast(CastStyle::Static, q, x, L, L);
                assert_eq!(s.trial_fold_paths(e).0, None, "a cast to an enumeration declines");
            }
        }
        assert_eq!(t.none, 0, "every enumerator folds");
    });
}
