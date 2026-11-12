//! The Oven's trial-fold fast path (spec/oven.md "Trial folds", WP-656):
//! for a generated set of integer expressions (every integer type, both
//! signs, every builtin unary and binary integer operator, boundary
//! values, overflow, `/ 0`, shifts by the width and beyond, nested
//! casts, `?:`, `&&`/`||`, `sizeof`/`alignof`, enumerators and casts to
//! enumerations, reads of variables of every kind), the fast path's
//! answer is the full path's: the same value, or `None` from both. A cast
//! to an enumeration is outside the domain (the full path checks the
//! enumeration's range): the fast path declines it; so does a read the
//! walk cannot answer without allocating (WP-663).

#![allow(clippy::many_single_char_names, reason = "short names mirror the C++ under test")]

mod conv_common;

use conv_common::*;
use kiln_ast::consts::{IntBits, Scalar};
use kiln_ast::decl::{DeclKind, DefinitionBits, FnSpec, TagKind, VarSpec};
use kiln_ast::expr::{BinaryOp, TraitKind, UnaryOp};
use kiln_ast::types::BuiltinType as B;
use kiln_ast::{ConstValueId, DeclHead, DeclId, ExprId, QualType};
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
    let (fast, full) = s.trial_fold_paths(e, false);
    let (a, b) = (shown(s, fast.flatten()), shown(s, full));
    assert_eq!(a, b, "fast vs full path for {}", what());
    tally(a.is_some(), t);
}

/// Where the fast path answers, its answer is the full path's (it may
/// decline what the full path folds).
fn check_sound(s: &mut Sema<'_, '_>, e: ExprId, what: &dyn Fn() -> String, t: &mut Tally) {
    let (fast, full) = s.trial_fold_paths(e, false);
    let (a, b) = (shown(s, fast.flatten()), shown(s, full));
    if fast.is_some() {
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
            assert_eq!(n > 64, s.trial_fold_paths(e, false).0.is_none(), "left spine of {n}");
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
                assert_eq!(
                    s.trial_fold_paths(e, false).0,
                    None,
                    "a cast to an enumeration declines"
                );
            }
        }
        assert_eq!(t.none, 0, "every enumerator folds");
    });
}

/// The Oven's state a skipped full path must not have changed: the
/// global entries (`global_index`) and the static space's top
/// (`static_bytes`), as `Oven::memory` counts them.
fn oven_state(s: &Sema<'_, '_>) -> (u64, u64, u64) {
    let r = s.oven().memory();
    let count = |n: &str| r.rows.iter().find(|x| x.name == n).map_or(u64::MAX, |x| x.count);
    (count("oven.globals"), count("oven.mem"), s.oven().stats().totals().static_bytes)
}

/// `v`'s initializer evaluated as sema records it (`VarEvalState`):
/// `Value` when the full path folds it, else `NotConstant`.
fn evaluate_init(s: &mut Sema<'_, '_>, v: DeclId, init: ExprId) {
    let cv = s.trial_fold_full(init, false);
    s.ast.decls.begin_eval(v).expect("not evaluated yet");
    s.ast.decls.finish_eval(v, cv);
}

/// Reads of variables in the walk (WP-663 B1): a parameter or a
/// non-`const` automatic variable outside its frame is a definite
/// failure, its global entry made and resolved as the full path does
/// (first reads included); a `const` integer or `constexpr` variable with
/// a known constant value reads that value once its object exists. A
/// `const` integer with a non-constant initializer, a reference, a `const
/// volatile` integer and a namespace-scope non-`const` variable are
/// declined. In every operand position (`&&`/`||`/`?:` arms included),
/// in strict and lax folds (`Oven::fold_rvalue`), the answers and the
/// Oven's state after each (global entries, the static space's top,
/// `static_bytes`) are those of a second Sema that runs the full path
/// only.
/// A two-operand expression over two variable reads.
type Pair = fn(&mut Sema<'_, '_>, ExprId, ExprId) -> ExprId;

#[test]
#[allow(clippy::too_many_lines, reason = "one table of read shapes")]
fn variable_reads_agree() {
    type Build = fn(&mut Sema<'_, '_>, ExprId) -> ExprId;
    type Trace = Vec<(String, Option<String>, (u64, u64, u64), String)>;
    let run = |fast: bool| -> (Trace, u32, u32) {
        with_sema(|s, _| {
            let ft = fn_type(s, QualType::INT, &[QualType::INT]);
            let h = DeclHead::new(s.ast.tu, name(s, "f"), L);
            let f = s.ast.builder().function(h, DeclKind::Function, ft, FnSpec::new(), None);
            let px = name(s, "x");
            let x = s.ast.builder().param(f, px, L, QualType::INT, 0, 0);
            s.ast.builder().set_params(f, &[x]).expect("params once");
            let ck = cnst(s, QualType::INT);
            let cvk = vol(s, ck);
            let rk = lref(s, ck);
            // A variable of `f` (`None`: at namespace scope) initialized by
            // `init`, evaluated as sema does for a potentially-constant one.
            let def = |s: &mut Sema<'_, '_>, at: Option<DeclId>, n: &str, ty, spec, init, eval| {
                let h = DeclHead::new(at.unwrap_or(s.ast.tu), name(s, n), L);
                let d = s.ast.builder().var(h, ty, spec, None);
                s.ast.builder().set_var_init(d, init).expect("init once");
                if eval {
                    evaluate_init(s, d, init);
                }
                d
            };
            let lit = |s: &mut Sema<'_, '_>, v| int_lit(s, QualType::INT, v);
            let plain = VarSpec::new();
            let i5 = lit(s, 5);
            let y = def(s, Some(f), "y", QualType::INT, plain, i5, false);
            let i4 = lit(s, 4);
            let k = def(s, Some(f), "k", ck, plain, i4, true);
            let rx = s.build_decl_ref(x, L);
            let kn = def(s, Some(f), "kn", ck, plain, rx, true);
            let i7 = lit(s, 7);
            let ce = def(s, None, "ce", ck, plain.with_constexpr(true), i7, true);
            let rkk = s.build_decl_ref(k, L);
            let r = def(s, Some(f), "r", rk, plain, rkk, true);
            let i3 = lit(s, 3);
            let cv = def(s, Some(f), "cv", cvk, plain, i3, true);
            let g = var(s, "g", QualType::INT);
            let shapes: [(&str, Build); 14] = [
                ("+v", |s, v| s.act_on_unary(UnaryOp::Plus, v, L)),
                ("-v", |s, v| s.act_on_unary(UnaryOp::Minus, v, L)),
                ("v + 1", |s, v| {
                    let one = int_lit(s, QualType::INT, 1);
                    s.act_on_binary(BinaryOp::Add, v, one, L)
                }),
                ("1 + v", |s, v| {
                    let one = int_lit(s, QualType::INT, 1);
                    s.act_on_binary(BinaryOp::Add, one, v, L)
                }),
                ("v / 0", |s, v| {
                    let z = int_lit(s, QualType::INT, 0);
                    s.act_on_binary(BinaryOp::Div, v, z, L)
                }),
                ("v == 3", |s, v| {
                    let k = int_lit(s, QualType::INT, 3);
                    s.act_on_binary(BinaryOp::EQ, v, k, L)
                }),
                ("(long)v", |s, v| s.act_on_cast(CastStyle::CStyle, QualType::LONG, v, L, L)),
                ("0 && v", |s, v| {
                    let z = int_lit(s, QualType::INT, 0);
                    s.act_on_binary(BinaryOp::LAnd, z, v, L)
                }),
                ("1 && v", |s, v| {
                    let one = int_lit(s, QualType::INT, 1);
                    s.act_on_binary(BinaryOp::LAnd, one, v, L)
                }),
                ("1 || v", |s, v| {
                    let one = int_lit(s, QualType::INT, 1);
                    s.act_on_binary(BinaryOp::LOr, one, v, L)
                }),
                ("0 || v", |s, v| {
                    let z = int_lit(s, QualType::INT, 0);
                    s.act_on_binary(BinaryOp::LOr, z, v, L)
                }),
                ("1 ? 2 : v", |s, v| {
                    let (c, two) = (int_lit(s, QualType::INT, 1), int_lit(s, QualType::INT, 2));
                    s.act_on_conditional(c, two, v, L)
                }),
                ("0 ? 2 : v", |s, v| {
                    let (c, two) = (int_lit(s, QualType::INT, 0), int_lit(s, QualType::INT, 2));
                    s.act_on_conditional(c, two, v, L)
                }),
                ("v ? 1 : 2", |s, v| {
                    let (one, two) = (int_lit(s, QualType::INT, 1), int_lit(s, QualType::INT, 2));
                    s.act_on_conditional(v, one, two, L)
                }),
            ];
            // (name, variable, the walk answers once the global exists).
            let vars = [
                ("x (parameter)", x, true),
                ("y (non-const local)", y, true),
                ("k (const int, constant init)", k, true),
                ("kn (const int, non-constant init)", kn, false),
                ("ce (constexpr)", ce, true),
                ("r (reference)", r, false),
                ("cv (const volatile int)", cv, false),
                ("g (namespace-scope int)", g, false),
            ];
            let (mut trace, mut failures, mut values) = (Trace::new(), 0, 0);
            for (vn, d, answers) in vars {
                for (what, build) in shapes {
                    for lax in [false, true] {
                        for round in 0..2 {
                            let rv = s.build_decl_ref(d, L);
                            let e = build(s, rv);
                            let w = format!("{what} (v = {vn}, lax {lax}, round {round})");
                            let quick = if fast { s.trial_fold_fast(e) } else { None };
                            let r = match quick {
                                Some(a) => a,
                                None => s.trial_fold_full(e, lax),
                            };
                            if let Some(a) = quick {
                                if a.is_some() { values += 1 } else { failures += 1 }
                            }
                            if fast {
                                // A parameter or a local answers from its first
                                // read; a readable variable once the first full
                                // path made its object. (Not `k / 0`: a failing
                                // operation declines.)
                                let local = d == x || d == y;
                                if answers && (local || round == 1 && what != "v / 0") {
                                    assert!(quick.is_some(), "fast path answers {w}");
                                }
                                // An arm outside the domain declines even untaken.
                                if !answers {
                                    assert_eq!(quick, None, "fast path declines {w}");
                                }
                            }
                            let st = (oven_state(s), s.oven().globals_digest());
                            trace.push((w, shown(s, r), st.0, st.1));
                        }
                    }
                }
            }
            // First reads of two variables in one expression, each pair fresh:
            // the entries are made in compile order and only the read that
            // runs first is resolved (`a`, `b`: non-`const` locals; `p`, `q`:
            // parameters, whose resolution is provisional).
            let ft2 = fn_type(s, QualType::INT, &[QualType::INT, QualType::INT]);
            let pairs: [(&str, Pair); 6] = [
                ("a + b", |s, a, b| s.act_on_binary(BinaryOp::Add, a, b, L)),
                ("1 ? +a : +b", |s, a, b| {
                    let c = int_lit(s, QualType::INT, 1);
                    let (a, b) =
                        (s.act_on_unary(UnaryOp::Plus, a, L), s.act_on_unary(UnaryOp::Plus, b, L));
                    s.act_on_conditional(c, a, b, L)
                }),
                ("0 ? +a : +b", |s, a, b| {
                    let c = int_lit(s, QualType::INT, 0);
                    let (a, b) =
                        (s.act_on_unary(UnaryOp::Plus, a, L), s.act_on_unary(UnaryOp::Plus, b, L));
                    s.act_on_conditional(c, a, b, L)
                }),
                ("0 && a || b", |s, a, b| {
                    let z = int_lit(s, QualType::INT, 0);
                    let l = s.act_on_binary(BinaryOp::LAnd, z, a, L);
                    s.act_on_binary(BinaryOp::LOr, l, b, L)
                }),
                ("a < (1 || b)", |s, a, b| {
                    let one = int_lit(s, QualType::INT, 1);
                    let r = s.act_on_binary(BinaryOp::LOr, one, b, L);
                    s.act_on_binary(BinaryOp::LT, a, r, L)
                }),
                ("-(b * a)", |s, a, b| {
                    let m = s.act_on_binary(BinaryOp::Mul, b, a, L);
                    s.act_on_unary(UnaryOp::Minus, m, L)
                }),
            ];
            for (i, (what, build)) in pairs.into_iter().enumerate() {
                for params in [false, true] {
                    let (a, b) = if params {
                        let h = DeclHead::new(s.ast.tu, name(s, &format!("g{i}")), L);
                        let g = s.ast.builder().function(
                            h,
                            DeclKind::Function,
                            ft2,
                            FnSpec::new(),
                            None,
                        );
                        let (np, nq) = (name(s, "p"), name(s, "q"));
                        let p = s.ast.builder().param(g, np, L, QualType::INT, 0, 0);
                        let q = s.ast.builder().param(g, nq, L, QualType::INT, 0, 1);
                        s.ast.builder().set_params(g, &[p, q]).expect("params once");
                        (p, q)
                    } else {
                        let (one, two) = (lit(s, 1), lit(s, 2));
                        let a = def(s, Some(f), &format!("a{i}"), QualType::INT, plain, one, false);
                        let b = def(s, Some(f), &format!("b{i}"), QualType::INT, plain, two, false);
                        (a, b)
                    };
                    for round in 0..2 {
                        let (ra, rb) = (s.build_decl_ref(a, L), s.build_decl_ref(b, L));
                        let e = build(s, ra, rb);
                        let w = format!("{what} (parameters {params}, round {round})");
                        let quick = if fast { s.trial_fold_fast(e) } else { None };
                        if fast {
                            assert!(quick.is_some(), "fast path answers {w}");
                            failures += 1;
                        }
                        let r = quick.unwrap_or_else(|| s.trial_fold_full(e, false));
                        let st = (oven_state(s), s.oven().globals_digest());
                        trace.push((w, shown(s, r), st.0, st.1));
                    }
                }
            }
            (trace, failures, values)
        })
    };
    let (fast, failures, values) = run(true);
    let (full, ..) = run(false);
    assert_eq!(fast.len(), full.len());
    for (a, b) in fast.iter().zip(&full) {
        assert_eq!(a, b, "fast vs full path");
    }
    assert!(failures > 0 && values > 0, "{failures} failures, {values} values answered");
}

/// The variables of [`constant_init_agrees`], in a fresh Sema: by name,
/// each `const int` unless named otherwise.
fn constant_init_vars(s: &mut Sema<'_, '_>) -> Vec<(&'static str, DeclId)> {
    let ft = fn_type(s, QualType::INT, &[QualType::INT]);
    let h = DeclHead::new(s.ast.tu, name(s, "f"), L);
    let f = s.ast.builder().function(h, DeclKind::Function, ft, FnSpec::new(), None);
    let px = name(s, "x");
    let x = s.ast.builder().param(f, px, L, QualType::INT, 0, 0);
    s.ast.builder().set_params(f, &[x]).expect("params once");
    let ck = cnst(s, QualType::INT);
    let rk = lref(s, ck);
    let plain = VarSpec::new();
    let def = |s: &mut Sema<'_, '_>, at: Option<DeclId>, n: &str, ty, spec, init| {
        let h = DeclHead::new(at.unwrap_or(s.ast.tu), name(s, n), L);
        let d = s.ast.builder().var(h, ty, spec, None);
        s.ast.builder().set_var_init(d, init).expect("init once");
        d
    };
    let bin = |s: &mut Sema<'_, '_>, op, l, r| {
        let (l, r) = (int_lit(s, QualType::INT, l), int_lit(s, QualType::INT, r));
        s.act_on_binary(op, l, r, L)
    };
    let mut out = Vec::new();
    let e = bin(s, BinaryOp::Mul, 3, 4);
    out.push(("n1 = 3 * 4 (namespace)", def(s, None, "n1", ck, plain, e)));
    let e = bin(s, BinaryOp::Div, 1, 0);
    out.push(("n2 = 1 / 0 (namespace)", def(s, None, "n2", ck, plain, e)));
    let e = bin(s, BinaryOp::Add, 0x7fff_ffff, 1);
    out.push(("n3 = INT_MAX + 1 (namespace)", def(s, None, "n3", ck, plain, e)));
    let i7 = int_lit(s, QualType::INT, 7);
    let ce = def(s, None, "ce", ck, plain.with_constexpr(true), i7);
    evaluate_init(s, ce, i7);
    let rce = s.build_decl_ref(ce, L);
    let one = int_lit(s, QualType::INT, 1);
    let e = s.act_on_binary(BinaryOp::Add, rce, one, L);
    out.push(("n4 = ce + 1 (namespace, ce constexpr)", def(s, None, "n4", ck, plain, e)));
    let g = var(s, "g", QualType::INT);
    let rg = s.build_decl_ref(g, L);
    let e = s.act_on_unary(UnaryOp::Plus, rg, L);
    out.push(("n5 = +g (namespace, g int)", def(s, None, "n5", ck, plain, e)));
    let rce = s.build_decl_ref(ce, L);
    out.push(("n6 = ce (namespace const int&)", def(s, None, "n6", rk, plain, rce)));
    let e = bin(s, BinaryOp::Shl, 1, 5);
    out.push(("l1 = 1 << 5 (local)", def(s, Some(f), "l1", ck, plain, e)));
    let rx = s.build_decl_ref(x, L);
    let one = int_lit(s, QualType::INT, 1);
    let e = s.act_on_binary(BinaryOp::Add, rx, one, L);
    out.push(("l2 = x + 1 (local, x parameter)", def(s, Some(f), "l2", ck, plain, e)));
    let e = bin(s, BinaryOp::Sub, 9, 2);
    out.push(("n7 = 9 - 2 (namespace int)", def(s, None, "n7", QualType::INT, plain, e)));
    out
}

/// `Oven::is_constant_initialized`'s fast path (WP-663 B2): over three
/// rounds of every variable of [`constant_init_vars`], the answers and
/// the Oven's state after each (global entries, the static space's top,
/// `static_bytes`: the full path's placeholder of a namespace-scope
/// variable included) are those of a second Sema that runs the full path
/// only, while the walk answers without compiling a thunk.
#[test]
fn constant_init_agrees() {
    type Trace = Vec<(String, bool, (u64, u64, u64), String)>;
    let thunks = |s: &Sema<'_, '_>| {
        let r = s.oven().memory();
        r.rows.iter().find(|x| x.name == "oven.thunks").map_or(0, |x| x.count)
    };
    let run = |fast: bool| -> (Trace, u64) {
        with_sema(|s, _| {
            let vars = constant_init_vars(s);
            let mut trace = Vec::new();
            for round in 0..3 {
                for &(n, v) in &vars {
                    let r = if fast {
                        s.is_constant_initialized(v)
                    } else {
                        s.is_constant_initialized_full(v)
                    };
                    let st = (oven_state(s), s.oven().globals_digest());
                    trace.push((format!("{n}, round {round}"), r, st.0, st.1));
                }
            }
            (trace, thunks(s))
        })
    };
    let (fast, fast_thunks) = run(true);
    let (full, full_thunks) = run(false);
    // `n1`'s full path places its placeholder: a global, static bytes.
    let (globals, _, static_bytes) = full[0].2;
    assert!(globals >= 1 && static_bytes > 0, "placeholder of n1: {:?}", full[0]);
    for (a, b) in fast.iter().zip(&full) {
        assert_eq!(a, b, "fast vs full path");
    }
    assert_eq!(fast.len(), full.len());
    let answer = |n: &str| fast.iter().find(|x| x.0 == format!("{n}, round 2")).map(|x| x.1);
    assert_eq!(answer("n1 = 3 * 4 (namespace)"), Some(true));
    assert_eq!(answer("n2 = 1 / 0 (namespace)"), Some(false));
    assert_eq!(answer("l2 = x + 1 (local, x parameter)"), Some(false));
    // A `leaf-check` build runs the full path after every walk too.
    if !kiln_oven::LEAF_CHECK {
        assert!(fast_thunks < full_thunks, "{fast_thunks} thunks vs {full_thunks}");
    }
}

/// `.` member reads in the walk (WP-663 B4): an integral member of a
/// parameter or a non-`const` automatic variable (nested `.` included)
/// is answered (a definite failure where it is read), as an operand and as
/// a glvalue fold of the member access itself (codegen's); `->`, a union member, a bit-field, a
/// reference member and a namespace-scope object are declined. In strict
/// and lax folds, the answers and the Oven's state after each are those
/// of a second Sema that runs the full path only.
/// A member access built on a variable, given a member-access builder.
type Access = fn(
    &mut Sema<'_, '_>,
    DeclId,
    &dyn Fn(&mut Sema<'_, '_>, ExprId, &str, bool) -> ExprId,
) -> ExprId;

#[test]
#[allow(clippy::too_many_lines, reason = "one table of member shapes")]
fn member_reads_agree() {
    type Build = fn(&mut Sema<'_, '_>, ExprId) -> ExprId;
    type Trace = Vec<(String, Option<String>, (u64, u64, u64), String)>;
    let run = |fast: bool| -> (Trace, usize) {
        with_sema(|s, _| {
            let record = |s: &mut Sema<'_, '_>, n: &str, tag, fields: &[(&str, QualType, u64)]| {
                let h = DeclHead::new(s.ast.tu, name(s, n), L);
                let r = s.ast.builder().record(h, tag, None);
                s.ast.builder().begin_record_definition(r).expect("first definition");
                for &(m, ty, width) in fields {
                    let w = (width != 0).then(|| {
                        let l = int_lit(s, QualType::INT, width);
                        s.ast.builder().constant_expr(l, None)
                    });
                    let h = DeclHead::new(r, name(s, m), L);
                    s.ast.builder().field(h, ty, w, 0);
                }
                s.ast.builder().complete_record_definition(r, DefinitionBits::empty());
                s.ast.decls.get(r).ty().expect("record type")
            };
            let uint = builtin(B::UInt);
            let p = record(
                s,
                "P",
                TagKind::Struct,
                &[("a", QualType::INT, 0), ("b", QualType::LONG, 0), ("c", uint, 3)],
            );
            let q = record(s, "Q", TagKind::Struct, &[("z", QualType::INT, 0), ("p", p, 0)]);
            let u = record(s, "U", TagKind::Union, &[("a", QualType::INT, 0)]);
            let ri = lref(s, QualType::INT);
            let rr = record(s, "R", TagKind::Struct, &[("r", ri, 0)]);
            let pp = ptr(s, p);
            let ft = fn_type(s, QualType::INT, &[p]);
            let h = DeclHead::new(s.ast.tu, name(s, "f"), L);
            let f = s.ast.builder().function(h, DeclKind::Function, ft, FnSpec::new(), None);
            let px = name(s, "px");
            let prm = s.ast.builder().param(f, px, L, p, 0, 0);
            s.ast.builder().set_params(f, &[prm]).expect("params once");
            let local = |s: &mut Sema<'_, '_>, n: &str, ty| {
                let h = DeclHead::new(f, name(s, n), L);
                s.ast.builder().var(h, ty, VarSpec::new(), None)
            };
            let (pl, ql, ul, rl, pt) = (
                local(s, "pl", p),
                local(s, "ql", q),
                local(s, "ul", u),
                local(s, "rl", rr),
                local(s, "pt", pp),
            );
            let gp = var(s, "gp", p);
            let member = |s: &mut Sema<'_, '_>, base: ExprId, n: &str, arrow: bool| {
                let sym = s.ast.symbols().intern(n);
                s.act_on_member(base, sym, L, L, arrow)
            };
            // (name, the access, the walk answers).
            let dot: Access = |s, d, m| {
                let r = s.build_decl_ref(d, L);
                m(s, r, "a", false)
            };
            let dot_b: Access = |s, d, m| {
                let r = s.build_decl_ref(d, L);
                m(s, r, "b", false)
            };
            let nested: Access = |s, d, m| {
                let r = s.build_decl_ref(d, L);
                let x = m(s, r, "p", false);
                m(s, x, "b", false)
            };
            let first: Access = |s, d, m| {
                let r = s.build_decl_ref(d, L);
                m(s, r, "z", false)
            };
            let bits: Access = |s, d, m| {
                let r = s.build_decl_ref(d, L);
                m(s, r, "c", false)
            };
            let refm: Access = |s, d, m| {
                let r = s.build_decl_ref(d, L);
                m(s, r, "r", false)
            };
            let arrow: Access = |s, d, m| {
                let r = s.build_decl_ref(d, L);
                m(s, r, "a", true)
            };
            let objects: [(&str, DeclId, Access, bool); 10] = [
                ("pl.a (non-const local)", pl, dot, true),
                ("pl.b (non-const local, long)", pl, dot_b, true),
                ("px.a (parameter)", prm, dot, true),
                ("ql.p.b (nested)", ql, nested, true),
                ("ql.z (offset 0)", ql, first, true),
                ("pl.c (bit-field)", pl, bits, false),
                ("ul.a (union)", ul, dot, false),
                ("rl.r (reference member)", rl, refm, false),
                ("pt->a (arrow)", pt, arrow, false),
                ("gp.a (namespace scope)", gp, dot, false),
            ];
            let shapes: [(&str, Build); 7] = [
                ("v (glvalue fold)", |_, v| v),
                ("+v", |s, v| s.act_on_unary(UnaryOp::Plus, v, L)),
                ("v + 1", |s, v| {
                    let one = int_lit(s, QualType::INT, 1);
                    s.act_on_binary(BinaryOp::Add, v, one, L)
                }),
                ("v == 3", |s, v| {
                    let k = int_lit(s, QualType::INT, 3);
                    s.act_on_binary(BinaryOp::EQ, v, k, L)
                }),
                ("0 && v", |s, v| {
                    let z = int_lit(s, QualType::INT, 0);
                    s.act_on_binary(BinaryOp::LAnd, z, v, L)
                }),
                ("1 ? 2 : v", |s, v| {
                    let (c, two) = (int_lit(s, QualType::INT, 1), int_lit(s, QualType::INT, 2));
                    s.act_on_conditional(c, two, v, L)
                }),
                ("v ? 1 : 2", |s, v| {
                    let (one, two) = (int_lit(s, QualType::INT, 1), int_lit(s, QualType::INT, 2));
                    s.act_on_conditional(v, one, two, L)
                }),
            ];
            let (mut trace, mut failures) = (Trace::new(), 0usize);
            for (on, d, access, answers) in objects {
                for (what, build) in shapes {
                    for lax in [false, true] {
                        for round in 0..2 {
                            let v = access(s, d, &member);
                            let e = build(s, v);
                            let w = format!("{what} (v = {on}, lax {lax}, round {round})");
                            let quick = if fast { s.trial_fold_fast(e) } else { None };
                            if fast {
                                if answers {
                                    assert!(quick.is_some(), "fast path answers {w}");
                                    failures += usize::from(quick == Some(None));
                                } else {
                                    assert_eq!(quick, None, "fast path declines {w}");
                                }
                            }
                            let r = quick.unwrap_or_else(|| s.trial_fold_full(e, lax));
                            let st = (oven_state(s), s.oven().globals_digest());
                            trace.push((w, shown(s, r), st.0, st.1));
                        }
                    }
                }
            }
            (trace, failures)
        })
    };
    let (fast, failures) = run(true);
    let (full, _) = run(false);
    assert_eq!(fast.len(), full.len());
    for (a, b) in fast.iter().zip(&full) {
        assert_eq!(a, b, "fast vs full path");
    }
    assert!(failures > 0);
}

/// `Oven::memory` (`-fkiln-memory-report`) splits `Oven::footprint`
/// into rows: their reserved bytes sum to it, before and after folds.
/// The two exact peaks (`oven.heap-peak`, `oven.static-peak`, WP-1315)
/// are extra rows, and the total counts them instead of the
/// `oven.mem.capacity` row.
#[test]
fn oven_memory_rows_sum_to_footprint() {
    with_sema(|s, _| {
        let sum = |s: &Sema<'_, '_>| {
            let r = s.oven().memory();
            assert!(r.rows.iter().all(|row| row.name.starts_with("oven.")));
            let row = |n: &str| r.row(n).map_or(0, |x| x.reserved_bytes);
            let held: u64 = r
                .rows
                .iter()
                .filter(|x| !x.name.ends_with("-peak"))
                .map(|x| x.reserved_bytes)
                .sum();
            let peaks = row("oven.heap-peak") + row("oven.static-peak");
            assert_eq!(r.reserved_bytes(), held - row("oven.mem.capacity") + peaks);
            (held, s.oven().footprint() as u64)
        };
        let (a, b) = sum(s);
        assert_eq!(a, b);
        for v in 0..40 {
            let (l, r) = (int_lit(s, QualType::INT, v), int_lit(s, QualType::INT, 3));
            let e = s.act_on_binary(BinaryOp::Mul, l, r, L);
            let _ = s.trial_fold_paths(e, false);
        }
        let (a, b) = sum(s);
        assert_eq!(a, b);
    });
}
