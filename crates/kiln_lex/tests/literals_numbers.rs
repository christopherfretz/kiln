//! pp-number classification: [lex.icon], [lex.fcon], [lex.ext].
//! Diagnostics match clang 19 (`-fsyntax-only -pedantic`) unless noted.
#![allow(clippy::float_cmp)] // exact literals: the lexer must round-trip them bit-for-bit
use kiln_lex::literal::{
    FloatSuffix, IntSize, IntSuffix, NumberClass, Radix, classify_number, classify_number_in,
};
use kiln_lex::{CStd, LexDiagKind as D, Std};

fn int(sp: &str) -> (Radix, String, IntSuffix, Option<u128>) {
    let n = classify_number(sp, Std::Cxx26);
    assert!(n.diags.is_empty(), "{sp}: {:?}", n.diags);
    match n.class {
        NumberClass::Integer(i) => (i.radix, i.digits.to_string(), i.suffix, i.value()),
        c => panic!("{sp}: {c:?}"),
    }
}

fn float(sp: &str) -> f64 {
    let n = classify_number(sp, Std::Cxx26);
    assert!(n.diags.is_empty(), "{sp}: {:?}", n.diags);
    match n.class {
        NumberClass::Floating(f) => f.parse_f64(),
        c => panic!("{sp}: {c:?}"),
    }
}

fn diags_in(sp: &str, std: Std) -> Vec<D> {
    classify_number(sp, std).diags.iter().map(|d| d.kind).collect()
}

fn diags(sp: &str) -> Vec<D> {
    diags_in(sp, Std::Cxx26)
}

const NONE: IntSuffix = IntSuffix { unsigned: false, size: IntSize::None };

/// [lex.icon]/1: decimal, octal, hexadecimal and binary literals, with
/// digit separators removed.
#[test]
fn lex_icon_bases() {
    assert_eq!(int("0"), (Radix::Octal, "0".into(), NONE, Some(0)));
    assert_eq!(int("123"), (Radix::Decimal, "123".into(), NONE, Some(123)));
    assert_eq!(int("0777"), (Radix::Octal, "0777".into(), NONE, Some(0o777)));
    assert_eq!(int("0xFf"), (Radix::Hex, "Ff".into(), NONE, Some(255)));
    assert_eq!(int("0B101"), (Radix::Binary, "101".into(), NONE, Some(5)));
    assert_eq!(int("1'000'000").3, Some(1_000_000));
}

/// [lex.icon]/1: integer-suffix combinations.
#[test]
fn lex_icon_suffixes() {
    let s = |sp| int(sp).2;
    let us = |unsigned, size| IntSuffix { unsigned, size };
    assert_eq!(s("1u"), us(true, IntSize::None));
    assert_eq!(s("1L"), us(false, IntSize::Long));
    assert_eq!(s("1ull"), us(true, IntSize::LongLong));
    assert_eq!(s("1LLU"), us(true, IntSize::LongLong));
    assert_eq!(s("1lu"), us(true, IntSize::Long));
    assert_eq!(s("1z"), us(false, IntSize::Size));
    assert_eq!(s("1Uz"), us(true, IntSize::Size));
    assert_eq!(s("1zu"), us(true, IntSize::Size));
    assert_eq!(diags("1Ll"), [D::InvalidSuffix { floating: false }]);
    assert_eq!(diags("1lz"), [D::InvalidSuffix { floating: false }]);
    assert_eq!(diags("1uu"), [D::InvalidSuffix { floating: false }]);
    assert_eq!(diags("1x"), [D::InvalidSuffix { floating: false }]);
    assert_eq!(diags("1u_x"), [D::InvalidSuffix { floating: false }]);
}

/// [lex.icon], [lex.fcon]: diagnosed malformed literals (clang's
/// `NumericLiteralParser`).
#[test]
fn lex_icon_errors() {
    assert_eq!(diags("0x"), [D::InvalidSuffix { floating: false }]);
    assert_eq!(diags("0x'1"), [D::InvalidSuffix { floating: false }]);
    assert_eq!(diags("0b"), [D::InvalidDigit { ch: 'b', radix: 8 }]);
    assert_eq!(diags("0b'1"), [D::InvalidDigit { ch: 'b', radix: 8 }]);
    assert_eq!(diags("09"), [D::InvalidDigit { ch: '9', radix: 8 }]);
    assert_eq!(diags("0b12"), [D::InvalidDigit { ch: '2', radix: 2 }]);
    assert_eq!(diags("0b1e"), [D::InvalidDigit { ch: 'e', radix: 2 }]);
    assert_eq!(diags("1f"), [D::InvalidDigit { ch: 'f', radix: 10 }]);
    assert_eq!(diags("1fu"), [D::InvalidDigit { ch: 'f', radix: 10 }]);
    assert_eq!(diags("0x1f"), []);
    let n = classify_number("100000000000000000000000000000000000000000", Std::Cxx26);
    match n.class {
        NumberClass::Integer(i) => assert_eq!(i.value(), None),
        c => panic!("{c:?}"),
    }
}

/// [lex.icon]/1 (C++14): digit separators go between digits.
#[test]
fn lex_icon_digit_separators() {
    assert_eq!(diags("0x1'p1"), [D::DigitSeparatorPosition { at_end: true }]);
    assert_eq!(diags("1.'5"), [D::DigitSeparatorPosition { at_end: false }]);
    assert_eq!(diags("1e'5"), [D::DigitSeparatorPosition { at_end: false }]);
    assert_eq!(diags("1'.5"), [D::DigitSeparatorPosition { at_end: true }]);
    assert_eq!(diags("1'u"), [D::DigitSeparatorPosition { at_end: true }]);
    assert_eq!(int("0'1").3, Some(1));
}

/// [lex.fcon]/1: decimal and hexadecimal floating literals.
#[test]
fn lex_fcon_values() {
    assert_eq!(float("1.5"), 1.5);
    assert_eq!(float(".5"), 0.5);
    assert_eq!(float("1."), 1.0);
    assert_eq!(float("1.e5"), 1e5);
    assert_eq!(float("1e-5"), 1e-5);
    assert_eq!(float("09.5"), 9.5);
    assert_eq!(float("08e1"), 80.0);
    assert_eq!(float("0e1"), 0.0);
    assert_eq!(float("1'0.2'5e1'0"), 10.25e10);
    assert_eq!(float("0x1p1"), 2.0);
    assert_eq!(float("0x1.8p+3"), 12.0);
    assert_eq!(float("0x.8p0"), 0.5);
    assert_eq!(float("0X1P-1074"), f64::from_bits(1));
    assert_eq!(float("0x1p-1075"), 0.0); // ties to even
    assert_eq!(float("0x1.8p-1074"), f64::from_bits(2)); // ties to even, up
    assert_eq!(float("0x1.fffffffffffff8p0"), 2.0); // rounds to the next binade
    assert_eq!(float("0x1.fffffffffffffp1023"), f64::MAX);
    assert_eq!(float("0x1p1024"), f64::INFINITY);
    assert_eq!(float("0x0.0000000000001p-1022"), f64::from_bits(1));
    assert_eq!(float("0x1.0000000000000800000000001p0"), 1.0 + f64::EPSILON);
    assert_eq!(float("1e400"), f64::INFINITY);
    assert_eq!(float("0.1"), 0.1);
}

/// [lex.fcon]: `parse_f32` rounds directly (no double rounding).
#[test]
fn lex_fcon_f32() {
    let f = |sp: &str| match classify_number(sp, Std::Cxx26).class {
        NumberClass::Floating(f) => f.parse_f32(),
        c => panic!("{c:?}"),
    };
    assert_eq!(f("0x1p-149"), f32::from_bits(1));
    assert_eq!(f("0x1.000001p0"), 1.0); // tie to even
    assert_eq!(f("0x1.000001000001p0"), 1.0 + f32::EPSILON);
    assert_eq!(f("0x1p128"), f32::INFINITY);
    assert_eq!(f("1.1f"), 1.1f32);
    assert_eq!(f("0x1.fffffep127"), f32::MAX);
}

/// [lex.fcon]: floating-point-suffixes and malformed floating literals.
#[test]
fn lex_fcon_suffixes_and_errors() {
    let suffix = |sp: &str, std| match classify_number(sp, std).class {
        NumberClass::Floating(f) => f.suffix,
        c => panic!("{sp}: {c:?}"),
    };
    assert_eq!(suffix("1.0f", Std::Cxx26), FloatSuffix::F);
    assert_eq!(suffix("1.0L", Std::Cxx26), FloatSuffix::L);
    assert_eq!(suffix("1.0F16", Std::Cxx26), FloatSuffix::F16);
    assert_eq!(suffix("1.0f32", Std::Cxx23), FloatSuffix::F32);
    assert_eq!(suffix("1.0F64", Std::Cxx23), FloatSuffix::F64);
    assert_eq!(suffix("1.0f128", Std::Cxx23), FloatSuffix::F128);
    assert_eq!(suffix("1.0BF16", Std::Cxx23), FloatSuffix::BF16);
    assert_eq!(suffix("0X1P1F", Std::Cxx26), FloatSuffix::F);
    // WP-361: before C++23 they are a pedantic extension, as in GCC 14
    // (clang 19 rejects them in every mode: a documented deviation).
    assert_eq!(suffix("1.0f32", Std::Cxx11), FloatSuffix::F32);
    assert_eq!(diags_in("1.0f32", Std::Cxx20), [D::FloatSuffixExtension]);
    assert_eq!(diags_in("1.0bf16", Std::Cxx17), [D::FloatSuffixExtension]);
    assert_eq!(diags_in("1.0F128", Std::Cxx23), []);
    assert_eq!(diags_in("1.0f16", Std::Cxx17), []);
    assert_eq!(diags_in("1.0Bf16", Std::Cxx23), [D::InvalidSuffix { floating: true }]);
    // WP-380: GCC's `_FloatNx` suffixes (lower-case `x` only), a pedantic
    // extension in every mode.
    assert_eq!(suffix("1.0f32x", Std::Cxx23), FloatSuffix::F32x);
    assert_eq!(suffix("0x1p3F64x", Std::Cxx11), FloatSuffix::F64x);
    assert_eq!(diags_in("1.0f64x", Std::Cxx26), [D::FloatSuffixGnu]);
    assert_eq!(diags_in("1.0f32X", Std::Cxx23), [D::InvalidSuffix { floating: true }]);
    assert_eq!(diags("1.0ll"), [D::InvalidSuffix { floating: true }]);
    assert_eq!(diags("1.0z"), [D::InvalidSuffix { floating: true }]);
    assert_eq!(diags("1.0u"), [D::InvalidSuffix { floating: true }]);
    // GNU `q`/`Q` (`__float128`), as clang's `isFloat128`: libstdc++'s
    // `<numbers>` spells its `__float128` constants with it.
    assert_eq!(suffix("1.0q", Std::Cxx20), FloatSuffix::Q);
    assert_eq!(suffix("2.5Q", Std::Cxx26), FloatSuffix::Q);
    assert_eq!(diags("1.0Q"), []);
    assert_eq!(diags("1.0fq"), [D::InvalidSuffix { floating: true }]);
    assert_eq!(diags("1.0lq"), [D::InvalidSuffix { floating: true }]);
    assert_eq!(diags("1e"), [D::ExponentNoDigits]);
    assert_eq!(diags("1e+"), [D::ExponentNoDigits]);
    assert_eq!(diags("07e"), [D::ExponentNoDigits]);
    assert_eq!(diags("0x1p"), [D::ExponentNoDigits]);
    assert_eq!(diags("0x1pf"), [D::ExponentNoDigits]);
    assert_eq!(diags("0x1."), [D::HexFloatNoExponent]);
    assert_eq!(diags("0x.p1"), [D::HexFloatNoSignificand]);
}

/// [lex.ext]/1: ud-suffixes; [usrlit.suffix]: standard library suffixes
/// (no leading `_`) are valid from C++14, `d`/`y` from C++20; `__` is
/// reserved and rejected as clang.
#[test]
fn lex_ext_numeric_ud_suffix() {
    let ud = |sp: &str, std| match classify_number(sp, std).class {
        NumberClass::Integer(i) => i.ud_suffix.map(str::to_owned),
        NumberClass::Floating(f) => f.ud_suffix.map(str::to_owned),
        NumberClass::Invalid => None,
    };
    assert_eq!(ud("1_x", Std::Cxx11).as_deref(), Some("_x"));
    assert_eq!(ud("1.5_km", Std::Cxx11).as_deref(), Some("_km"));
    assert_eq!(ud("0x1_x", Std::Cxx11).as_deref(), Some("_x"));
    assert_eq!(ud("1ms", Std::Cxx14).as_deref(), Some("ms"));
    assert_eq!(ud("1.5if", Std::Cxx14).as_deref(), Some("if"));
    assert_eq!(ud("2d", Std::Cxx20).as_deref(), Some("d"));
    assert_eq!(ud("0xffms", Std::Cxx14).as_deref(), Some("ms")); // `ff` are digits
    assert_eq!(ud("1ms", Std::Cxx11).as_deref(), None);
    assert_eq!(ud("2d", Std::Cxx17).as_deref(), None);
    assert_eq!(ud("1_x", Std::Cxx03).as_deref(), None);
    assert_eq!(diags("1__x"), [D::InvalidSuffix { floating: false }]);
    // A ud-suffix replaces any standard suffix.
    match classify_number("1u_x", Std::Cxx26).class {
        NumberClass::Invalid => {}
        c => panic!("{c:?}"),
    }
}

/// Extensions diagnosed before the standard that adopted them.
#[test]
fn lex_icon_fcon_extensions() {
    assert_eq!(diags_in("0b1", Std::Cxx11), [D::BinaryLiteralExtension]);
    assert_eq!(diags_in("0b1", Std::Cxx14), []);
    assert_eq!(diags_in("0x1p1", Std::Cxx14), [D::HexFloatExtension]);
    assert_eq!(diags_in("0x1p1", Std::Cxx17), []);
    assert_eq!(diags_in("1z", Std::Cxx20), [D::SizeSuffixExtension]);
    assert_eq!(diags_in("1uz", Std::Cxx20), [D::SizeSuffixExtension]);
}

/// WP-1331, C23 6.4.4.1: `wb` / `WB` with `u` / `U` in either order, in
/// C only; an extension before C23, a compat warning from C23 on, at the
/// literal's start.
#[test]
fn bit_int_suffixes() {
    let c = |sp: &str, c: CStd| {
        let n = classify_number_in(sp, Std::Cxx17, Some(c));
        let kinds: Vec<D> = n.diags.iter().map(|d| d.kind).collect();
        let suffix = match n.class {
            NumberClass::Integer(l) => Some((l.suffix, l.value())),
            _ => None,
        };
        (suffix, kinds)
    };
    let bit = |unsigned| IntSuffix { unsigned, size: IntSize::BitInt };
    for sp in ["1wb", "1WB"] {
        assert_eq!(c(sp, CStd::C23), (Some((bit(false), Some(1))), vec![D::BitIntSuffixCompat]));
    }
    for sp in ["1uwb", "1Uwb", "1wbu", "1WBU", "1uWB"] {
        assert_eq!(c(sp, CStd::C17).0, Some((bit(true), Some(1))), "{sp}");
    }
    assert_eq!(c("0x7fwb", CStd::C11).1, [D::BitIntSuffixExtension]);
    let n = classify_number_in("0xffwb", Std::Cxx17, Some(CStd::C23));
    assert_eq!(n.diags[0].range, 0..6);
    for sp in ["1wB", "1Wb", "1wbl", "1lwb", "1wbwb", "1uwbu", "1.0wb", "1zwb"] {
        assert_eq!(c(sp, CStd::C23).0, None, "{sp}");
    }
    // C++ has no such suffix.
    assert_eq!(diags_in("1wb", Std::Cxx26), [D::InvalidSuffix { floating: false }]);
}

/// Diagnostic ranges point into the spelling.
#[test]
fn lex_icon_diag_ranges() {
    let n = classify_number("0b12", Std::Cxx26);
    assert_eq!(n.diags[0].range, 3..4);
    let n = classify_number("12xyz", Std::Cxx26);
    assert_eq!(n.diags[0].range, 2..5);
}

/// GNU imaginary constants (clang `NumericLiteralParser`): one `i`/`j`
/// among the standard suffixes; a valid ud-suffix such as `if` or `il`
/// is kept beside the flag so sema can fall back from a failed literal
/// operator lookup to the imaginary constant.
#[test]
fn gnu_imaginary_suffix() {
    let im = |sp: &str| {
        let n = classify_number(sp, Std::Cxx26);
        assert!(n.diags.is_empty(), "{sp}: {:?}", n.diags);
        match n.class {
            NumberClass::Integer(i) => (i.imaginary, i.ud_suffix.map(str::to_owned)),
            NumberClass::Floating(f) => (f.imaginary, f.ud_suffix.map(str::to_owned)),
            NumberClass::Invalid => panic!("{sp}"),
        }
    };
    assert_eq!(im("1i"), (true, Some("i".into())));
    assert_eq!(im("1il"), (true, Some("il".into())));
    assert_eq!(im("1.5if"), (true, Some("if".into())));
    assert_eq!(im("7ui"), (true, None));
    assert_eq!(im("2.0j"), (true, None));
    assert_eq!(im("1.5fi"), (true, None));
    assert_eq!(im("3LLJ"), (true, None));
    assert_eq!(im("1"), (false, None));
    assert_eq!(im("1_i"), (false, Some("_i".into())));
    // A second `i` is not a suffix.
    assert_eq!(diags("1ii"), [D::InvalidSuffix { floating: false }]);
}
