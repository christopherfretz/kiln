//! Floating-point evaluation against the host's IEEE 754 arithmetic
//! ([basic.fundamental]/12, [expr.arith.conv], [conv.double],
//! [conv.fpint]): every binary32/binary64 result is bit-compared with
//! Rust's own (round-to-nearest-even) result for the same operation. The
//! x87 `long double` goldens are clang 19's (`-emit-llvm
//! --target=x86_64-unknown-linux-gnu`).

#![allow(
    clippy::format_push_string,
    clippy::float_cmp,
    reason = "test sources are built by appending; exact IEEE comparison is the point"
)]

mod common;
use common::*;

/// Operands chosen to exercise rounding, subnormals, overflow to infinity,
/// cancellation and exact cases.
const VALUES: &[&str] = &[
    "0.1",
    "0.2",
    "1.0",
    "3.0",
    "-2.5",
    "1e308",
    "-1.7976931348623157e308",
    "2.2250738585072014e-308",
    "5e-324",
    "16777217.0",
    "9007199254740993.0",
    "0.3333333333333333",
    "-0.0",
    "123456.789",
];

const OPS: &[&str] = &["+", "-", "*", "/"];

fn rust_op<T>(op: &str, a: T, b: T) -> T
where
    T: std::ops::Add<Output = T>
        + std::ops::Sub<Output = T>
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>,
{
    match op {
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        _ => a / b,
    }
}

/// Every pair of `VALUES` under every operator in `ty` (literal suffix
/// `suf`); `want` gives Rust's bits of the result, `None` to skip the case
/// (division by zero and NaN results are not constant: see tests/ub.rs).
fn grid(ty: &str, suf: &str, want: &dyn Fn(&str, &str, &str) -> Option<u128>) {
    let mut src = String::new();
    let mut expect = Vec::new();
    for (i, a) in VALUES.iter().enumerate() {
        for (j, b) in VALUES.iter().enumerate() {
            for (k, op) in OPS.iter().enumerate() {
                let Some(w) = want(a, op, b) else { continue };
                let name = format!("v{i}_{j}_{k}");
                src.push_str(&format!("constexpr {ty} {name} = ({a}{suf}) {op} ({b}{suf});\n"));
                expect.push((name, w, format!("{a} {op} {b}")));
            }
        }
    }
    let tu = ok(&src);
    for (name, w, what) in expect {
        assert_eq!(tu.float_bits(&name), w, "{ty}: {what}");
    }
}

/// [expr.add], [expr.mul]: binary64 arithmetic.
#[test]
fn basic_fundamental_double_arithmetic() {
    grid("double", "", &|a, op, b| {
        let (x, y): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
        let r = rust_op(op, x, y);
        ((op != "/" || y != 0.0) && !r.is_nan()).then(|| u128::from(r.to_bits()))
    });
}

/// [expr.add], [expr.mul]: binary32 arithmetic is not widened
/// (`FLT_EVAL_METHOD` 0); out-of-range literals become infinities.
#[test]
fn basic_fundamental_float_arithmetic() {
    grid("float", "f", &|a, op, b| {
        let (x, y): (f32, f32) = (a.parse().unwrap(), b.parse().unwrap());
        let r = rust_op(op, x, y);
        let lit_ok =
            |s: &str, v: f32| v.is_finite() && (v != 0.0 || s.parse::<f64>().unwrap() == 0.0);
        // A float literal out of range is ill-formed ([lex.fcon]/3).
        if !lit_ok(a, x) || !lit_ok(b, y) {
            return None;
        }
        ((op != "/" || y != 0.0) && !r.is_nan()).then(|| u128::from(r.to_bits()))
    });
}

/// [conv.fpint]/2: integer to floating conversion rounds to nearest.
#[test]
fn conv_fpint_integral_to_floating() {
    let ints: &[(&str, i128)] = &[
        ("0", 0),
        ("-1", -1),
        ("16777217", 16_777_217),
        ("9007199254740993LL", 9_007_199_254_740_993),
        ("-9223372036854775807LL - 1", i128::from(i64::MIN)),
        ("18446744073709551615ULL", i128::from(u64::MAX)),
        ("9223372036854775807LL", i128::from(i64::MAX)),
        ("123456789", 123_456_789),
    ];
    let mut src = String::new();
    for (i, (s, _)) in ints.iter().enumerate() {
        src.push_str(&format!("constexpr double d{i} = {s};\nconstexpr float f{i} = {s};\n"));
    }
    let tu = ok(&src);
    for (i, (s, v)) in ints.iter().enumerate() {
        #[allow(clippy::cast_precision_loss, reason = "the conversion under test")]
        let (d, f) = (*v as f64, *v as f32);
        assert_eq!(tu.float_bits(&format!("d{i}")), u128::from(d.to_bits()), "double {s}");
        assert_eq!(tu.float_bits(&format!("f{i}")), u128::from(f.to_bits()), "float {s}");
    }
}

/// [conv.fpint]/1: floating to integer conversion truncates toward zero.
#[test]
fn conv_fpint_floating_to_integral() {
    let vals: &[f64] =
        &[0.0, -0.0, 0.9, -0.9, 2.5, -2.5, 2_147_483_647.9, -2_147_483_648.9, 1e18, -1e18];
    let mut src = String::new();
    for (i, v) in vals.iter().enumerate() {
        src.push_str(&format!(
            "constexpr long long l{i} = {v:e};\nconstexpr int n{i} = (int)({v:e} / 1e9);\n"
        ));
    }
    src.push_str("constexpr unsigned long long u = 1.8e19;\nconstexpr bool b = 0.5;\n");
    let tu = ok(&src);
    for (i, v) in vals.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation, reason = "the conversion under test")]
        let (l, n) = (*v as i64, (*v / 1e9) as i32);
        assert_eq!(tu.int(&format!("l{i}")), i128::from(l), "{v}");
        assert_eq!(tu.int(&format!("n{i}")), i128::from(n), "{v} / 1e9");
    }
    assert_eq!(tu.int("u"), i128::from(18_000_000_000_000_000_000u64));
    assert_eq!(tu.int("b"), 1, "[conv.bool]: nonzero is true");
}

/// [conv.double], [conv.fpprom]: narrowing rounds to nearest (possibly to
/// infinity or a subnormal), widening is exact.
#[test]
fn conv_double_rounding() {
    let vals = ["0.1", "1e300", "-1e-40", "1e-50", "3.4028235677973366e38", "16777217.0", "-0.0"];
    let mut src = String::new();
    for (i, v) in vals.iter().enumerate() {
        src.push_str(&format!(
            "constexpr float f{i} = {v};\nconstexpr double w{i} = (float){v};\n"
        ));
    }
    let tu = ok(&src);
    for (i, v) in vals.iter().enumerate() {
        let d: f64 = v.parse().unwrap();
        #[allow(clippy::cast_possible_truncation, reason = "the conversion under test")]
        let f = d as f32;
        assert_eq!(tu.float_bits(&format!("f{i}")), u128::from(f.to_bits()), "float {v}");
        assert_eq!(
            tu.float_bits(&format!("w{i}")),
            u128::from(f64::from(f).to_bits()),
            "double(float {v})"
        );
    }
}

/// [expr.rel], [expr.eq]: comparisons, including signed zeros.
#[test]
fn expr_rel_floating_comparisons() {
    let mut src = String::new();
    let mut expect = Vec::new();
    for (i, a) in VALUES.iter().enumerate() {
        for (j, b) in VALUES.iter().enumerate() {
            let (x, y): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
            for (k, (op, r)) in
                [("<", x < y), ("<=", x <= y), ("==", x == y), ("!=", x != y)].iter().enumerate()
            {
                let name = format!("c{i}_{j}_{k}");
                src.push_str(&format!("constexpr bool {name} = ({a}) {op} ({b});\n"));
                expect.push((name, *r, format!("{a} {op} {b}")));
            }
        }
    }
    let tu = ok(&src);
    for (name, r, what) in expect {
        assert_eq!(tu.int(&name), i128::from(r), "{what}");
    }
}

/// [expr.unary.op]/8, [expr.ass], [expr.pre.incr]: negation flips the
/// sign bit; compound assignment and increment round once per operation.
#[test]
fn expr_unary_and_compound_floating() {
    let tu = ok(r"
constexpr double neg = -0.0;
constexpr double f(int n) {
  double s = 0;
  for (int i = 0; i < n; ++i)
    s += 0.1;
  return s;
}
constexpr double sum = f(10);
constexpr float g() {
  float x = 16777216.0f;
  ++x;
  x *= 3;
  x /= 7;
  x -= 0.5f;
  return x;
}
constexpr float inc = g();
");
    assert_eq!(tu.float_bits("neg"), u128::from((-0.0f64).to_bits()));
    let mut s = 0.0f64;
    for _ in 0..10 {
        s += 0.1;
    }
    assert_eq!(tu.float_bits("sum"), u128::from(s.to_bits()));
    let mut x = 16_777_216.0f32;
    x += 1.0;
    x *= 3.0;
    x /= 7.0;
    x -= 0.5;
    assert_eq!(tu.float_bits("inc"), u128::from(x.to_bits()));
}

/// [basic.fundamental]/12: `long double` is the x87 80-bit format on
/// x86-64 (64-bit significand, explicit integer bit, subnormals); goldens
/// are clang's. The test DSL reads `L` literals as binary64, so inexact
/// values are computed (`1.0L / 10` is the correctly rounded `0.1L`).
#[test]
fn basic_fundamental_long_double_x87() {
    let tu = ok(r"
constexpr long double l0 = 1.0L / 3;
constexpr long double l1 = 1.0L / 10 + 2.0L / 10;
constexpr long double l2 = (long double)18446744073709551615ull;
constexpr long double l3 = (long double)(1ull << 63) + 1;
constexpr long double big() {
  long double x = (long double)1e300;
  return x * x * x * x;
}
constexpr long double l4 = big();
constexpr long double l5 = (long double)0.1;
constexpr double l6 = (double)(1.0L / 3);
constexpr long double l7 = -2.5L * 3.0L - 1.0L;
constexpr long double tiny() {
  long double x = (long double)1e-300, y = x * x * x * x;
  y = y * y;
  return y * y * (long double)1e-145 / 3;
}
constexpr long double l8 = tiny();
constexpr long long l9 = (long long)-1e18L;
");
    let want: &[(&str, u128)] = &[
        ("l0", 0x3FFD_AAAA_AAAA_AAAA_AAAB),
        ("l1", 0x3FFD_9999_9999_9999_999A),
        ("l2", 0x403E_FFFF_FFFF_FFFF_FFFF),
        ("l3", 0x403E_8000_0000_0000_0001),
        ("l4", 0x4F91_9F17_732D_FAC7_6AE4),
        ("l5", 0x3FFB_CCCC_CCCC_CCCC_D000),
        ("l6", 0x3FD5_5555_5555_5555),
        ("l7", 0xC002_8800_0000_0000_0000),
        ("l8", 0x0000_0000_0000_0001_6534),
    ];
    for (n, w) in want {
        assert_eq!(tu.float_bits(n), *w, "{n}: {:#x}", tu.float_bits(n));
    }
    assert_eq!(tu.int("l9"), -1_000_000_000_000_000_000);
}
