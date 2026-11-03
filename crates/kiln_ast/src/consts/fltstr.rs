//! Exact decimal printing of a float encoding as `llvm::APFloat::toString`
//! does it, for every [`FloatKind`] (spec/oven.md, "Diagnostics"; spec/ast.md §9.3).
//!
//! The value is expanded to its exact decimal digits (`mant * 2^e` as
//! `N * 10^-k` with `N = mant * 5^k`, over a small bignum), rounded half
//! up on the first dropped digit to the requested number of significant
//! digits (LLVM's `AdjustToPrecision`), and laid out in plain or `E`
//! notation with LLVM's padding rule.

use super::value::FloatKind;

/// The significand precision in bits of a format (LLVM `semanticsPrecision`).
pub(crate) fn precision(kind: FloatKind) -> u32 {
    match kind {
        FloatKind::Half => 11,
        FloatKind::BFloat16 => 8,
        FloatKind::Single => 24,
        FloatKind::Double => 53,
        FloatKind::X87Extended => 64,
        FloatKind::Quad => 113,
    }
}

/// How clang streams an `APFloat` into a constant-evaluation note
/// (`OptionalDiagnostic::operator<<(const APFloat &)`):
/// `toString(Buf, (precision * 59 + 195) / 196)`, i.e. `ceil(precision *
/// log10(2))` significant digits, `1.0E+300`, `128.1`, `+Inf`, `NaN`.
pub fn note_string(kind: FloatKind, bits: u128) -> String {
    to_string(kind, bits, (precision(kind) * 59).div_ceil(196))
}

/// `llvm::APFloat::toString(Str, format_precision, /*FormatMaxPadding=*/3,
/// /*TruncateZero=*/true)`. A `format_precision` of 0 means LLVM's
/// round-trip default, `2 + precision * 59 / 196` digits (the JSON AST
/// dump's `value`).
pub fn to_string(kind: FloatKind, bits: u128, format_precision: u32) -> String {
    const MAX_PADDING: i64 = 3;
    let fp = if format_precision == 0 { 2 + precision(kind) * 59 / 196 } else { format_precision }
        as usize;
    let mut out = String::new();
    let (neg, class) = decode(kind, bits);
    let (mant, e2) = match class {
        Class::Nan => return "NaN".into(),
        Class::Inf => return if neg { "-Inf".into() } else { "+Inf".into() },
        Class::Zero => return if neg { "-0".into() } else { "0".into() },
        Class::Finite(m, e) => (m, e),
    };
    if neg {
        out.push('-');
    }
    // Most significant digit first; `exp` is the power of ten of the last.
    let (mut digits, mut exp) = exact_digits(mant, e2);
    adjust_to_precision(&mut digits, &mut exp, fp);
    let nd = digits.len() as i64;
    let scientific = if exp >= 0 {
        // `765e3` is `765000`, unless that looks more precise than it is.
        exp > MAX_PADDING || nd + exp > fp as i64
    } else {
        // The power of the most significant digit.
        let msd = exp + nd - 1;
        msd < 0 && -msd > MAX_PADDING
    };
    let push = |out: &mut String, ds: &[u8]| out.extend(ds.iter().map(|&d| char::from(b'0' + d)));
    if scientific {
        let e10 = exp + nd - 1;
        push(&mut out, &digits[..1]);
        out.push('.');
        if nd == 1 {
            out.push('0');
        } else {
            push(&mut out, &digits[1..]);
        }
        out.push('E');
        out.push(if e10 >= 0 { '+' } else { '-' });
        out.push_str(&e10.unsigned_abs().to_string());
    } else if exp >= 0 {
        push(&mut out, &digits);
        out.extend(std::iter::repeat_n('0', exp as usize));
    } else {
        let whole = nd + exp;
        if whole > 0 {
            push(&mut out, &digits[..whole as usize]);
            out.push('.');
            push(&mut out, &digits[whole as usize..]);
        } else {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-whole) as usize));
            push(&mut out, &digits);
        }
    }
    out
}

enum Class {
    Nan,
    Inf,
    Zero,
    /// `mant * 2^e`, `mant > 0`.
    Finite(u128, i64),
}

/// The sign and class of an encoding (`bits` zero-extended).
fn decode(kind: FloatKind, bits: u128) -> (bool, Class) {
    let (exp_bits, frac_bits, explicit_int) = match kind {
        FloatKind::Half => (5u32, 10u32, false),
        FloatKind::BFloat16 => (8, 7, false),
        FloatKind::Single => (8, 23, false),
        FloatKind::Double => (11, 52, false),
        FloatKind::X87Extended => (15, 63, true),
        FloatKind::Quad => (15, 112, false),
    };
    let int_bits = u32::from(explicit_int);
    let neg = bits >> (1 + exp_bits + int_bits + frac_bits - 1) & 1 == 1;
    let exp_field = (bits >> (frac_bits + int_bits)) as u32 & ((1 << exp_bits) - 1);
    let frac = bits & ((1u128 << frac_bits) - 1);
    let exp_max = (1u32 << exp_bits) - 1;
    if exp_field == exp_max {
        // x87: an all-ones exponent with only the integer bit set is
        // infinity; anything else there is a NaN (or pseudo-NaN).
        return (neg, if frac == 0 { Class::Inf } else { Class::Nan });
    }
    let int_bit = if explicit_int { bits >> frac_bits & 1 } else { u128::from(exp_field != 0) };
    let mant = (int_bit << frac_bits) | frac;
    if mant == 0 {
        return (neg, Class::Zero);
    }
    let bias = (1i64 << (exp_bits - 1)) - 1;
    let e = i64::from(exp_field.max(1)) - bias - i64::from(frac_bits);
    (neg, Class::Finite(mant, e))
}

/// The exact decimal digits of `mant * 2^e` (most significant first, no
/// trailing zeros) and the power of ten of the last digit.
fn exact_digits(mant: u128, e: i64) -> (Vec<u8>, i64) {
    let tz = mant.trailing_zeros();
    let (mant, e) = (mant >> tz, e + i64::from(tz));
    // Little-endian base-2^32 limbs.
    let mut n: Vec<u32> = (0..4).map(|i| (mant >> (32 * i)) as u32).collect();
    let mut exp = 0i64;
    if e >= 0 {
        shl(&mut n, e as u64);
    } else {
        // mant * 2^e == mant * 5^-e * 10^e.
        let mut k = e.unsigned_abs();
        while k > 0 {
            let step = k.min(13); // 5^13 < 2^32
            mul_small(&mut n, 5u32.pow(step as u32));
            k -= step;
        }
        exp = e;
    }
    // Base-10^9 chunks, least significant first.
    let mut chunks = Vec::new();
    trim(&mut n);
    while !n.is_empty() {
        chunks.push(div_small(&mut n, 1_000_000_000));
        trim(&mut n);
    }
    let mut digits = Vec::with_capacity(chunks.len() * 9);
    for (i, &c) in chunks.iter().rev().enumerate() {
        let s = if i == 0 { c.to_string() } else { format!("{c:09}") };
        digits.extend(s.bytes().map(|b| b - b'0'));
    }
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
        exp += 1;
    }
    (digits, exp)
}

/// LLVM `AdjustToPrecision`: keep `fp` significant digits, rounding half
/// up on the first dropped digit, and drop the trailing zeros that leaves.
fn adjust_to_precision(digits: &mut Vec<u8>, exp: &mut i64, fp: usize) {
    let n = digits.len();
    if n <= fp {
        return;
    }
    let round_up = digits[fp] >= 5;
    *exp += (n - fp) as i64;
    digits.truncate(fp);
    if round_up {
        while let Some(&9) = digits.last() {
            digits.pop();
            *exp += 1;
        }
        match digits.last_mut() {
            Some(d) => *d += 1,
            None => digits.push(1),
        }
    }
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
        *exp += 1;
    }
}

fn trim(n: &mut Vec<u32>) {
    while n.last() == Some(&0) {
        n.pop();
    }
}

fn shl(n: &mut Vec<u32>, by: u64) {
    let words = (by / 32) as usize;
    let bits = (by % 32) as u32;
    if bits > 0 {
        let mut carry = 0u32;
        for w in n.iter_mut() {
            let v = (u64::from(*w) << bits) | u64::from(carry);
            *w = v as u32;
            carry = (v >> 32) as u32;
        }
        n.push(carry);
    }
    n.splice(0..0, std::iter::repeat_n(0, words));
}

fn mul_small(n: &mut Vec<u32>, m: u32) {
    let mut carry = 0u64;
    for w in n.iter_mut() {
        let v = u64::from(*w) * u64::from(m) + carry;
        *w = v as u32;
        carry = v >> 32;
    }
    if carry > 0 {
        n.push(carry as u32);
    }
}

/// Divides in place; returns the remainder.
fn div_small(n: &mut [u32], d: u32) -> u32 {
    let mut rem = 0u64;
    for w in n.iter_mut().rev() {
        let v = (rem << 32) | u64::from(*w);
        *w = (v / u64::from(d)) as u32;
        rem = v % u64::from(d);
    }
    rem as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(v: f64) -> u128 {
        u128::from(v.to_bits())
    }
    fn s(v: f32) -> u128 {
        u128::from(v.to_bits())
    }

    // [conv.fpint]/1: clang's `note_constexpr_overflow` spelling of the
    // source value; every expectation is clang 19's output.
    #[test]
    fn note_double() {
        let n = |v| note_string(FloatKind::Double, d(v));
        assert_eq!(n(1e300), "1.0E+300");
        assert_eq!(n(1e10), "1.0E+10");
        assert_eq!(n(2.5e9), "2.5E+9");
        assert_eq!(n(128.1), "128.1");
        assert_eq!(n(1e23), "9.999999999999999E+22");
        assert_eq!(n(f64::MAX), "1.797693134862316E+308");
        assert_eq!(n(-f64::MAX), "-1.797693134862316E+308");
        assert_eq!(n(300.300_000_000_000_04), "300.3000000000001");
        assert_eq!(n(9_007_199_254_740_993.0), "9007199254740992");
        assert_eq!(n(1e15 + 0.5), "1000000000000001");
        assert_eq!(n(223.232_630_059_894_35), "223.2326300598943");
        assert_eq!(n(11000.0), "11000");
        assert_eq!(n(1e4), "1.0E+4");
        assert_eq!(n(123_456_789_012.5), "123456789012.5");
        assert_eq!(n(99_999_999_999_999_999.0), "1.0E+17");
        assert_eq!(n(0.001), "0.001");
        assert_eq!(n(0.0001), "1.0E-4");
        assert_eq!(n(f64::MIN_POSITIVE), "2.225073858507201E-308");
        assert_eq!(n(5e-324), "4.940656458412465E-324");
        assert_eq!(n(0.0), "0");
        assert_eq!(n(-0.0), "-0");
        assert_eq!(n(f64::INFINITY), "+Inf");
        assert_eq!(n(f64::NEG_INFINITY), "-Inf");
        assert_eq!(n(f64::NAN), "NaN");
    }

    #[test]
    fn note_other_kinds() {
        assert_eq!(note_string(FloatKind::Single, s(1e20)), "1.0E+20");
        assert_eq!(note_string(FloatKind::Single, s(128.1)), "128.10001");
        assert_eq!(note_string(FloatKind::Single, s(-f32::MAX)), "-3.4028235E+38");
        assert_eq!(note_string(FloatKind::Single, s(1000.0 / 3.0)), "333.33334");
        // _Float16 333.25 (1000.0/3 rounded) and 65504.
        assert_eq!(note_string(FloatKind::Half, 0x5D35), "333.3");
        assert_eq!(note_string(FloatKind::Half, 0x7BFF), "6.55E+4");
        // __bf16 334 (1000.0/3 rounded).
        assert_eq!(note_string(FloatKind::BFloat16, 0x43A7), "334");
        // binary128 1000.0L/3 and LDBL_MAX (aarch64 long double).
        let third = 0x4007_4D55_5555_5555_5555_5555_5555_5555_u128;
        assert_eq!(note_string(FloatKind::Quad, third), "333.33333333333333333333333333333332");
        let qmax = 0x7FFE_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_u128;
        assert_eq!(note_string(FloatKind::Quad, qmax), "1.189731495357231765085759326628007E+4932");
        // x87 1000.0L/3 and LDBL_MAX (x86_64 long double).
        let x3 = 0x4007_A6AA_AAAA_AAAA_AAAB_u128;
        assert_eq!(note_string(FloatKind::X87Extended, x3), "333.33333333333333334");
        let xmax = 0x7FFE_FFFF_FFFF_FFFF_FFFF_u128;
        assert_eq!(note_string(FloatKind::X87Extended, xmax), "1.189731495357231765E+4932");
        assert_eq!(note_string(FloatKind::X87Extended, 0x7FFF_8000_0000_0000_0000), "+Inf");
    }

    // The round-trip default (`toString()` with no precision): the JSON
    // AST dump's `value` for a `FloatingLiteral`.
    #[test]
    fn round_trip_default() {
        let t = |v| to_string(FloatKind::Double, d(v), 0);
        assert_eq!(t(1e300), "1.0000000000000001E+300");
        assert_eq!(t(128.1), "128.09999999999999");
        assert_eq!(t(1e15 + 0.5), "1000000000000000.5");
        assert_eq!(t(5e-324), "4.9406564584124654E-324");
        assert_eq!(t(123.456e-10), "1.23456E-8");
        assert_eq!(to_string(FloatKind::Single, s(128.1), 0), "128.100006");
        assert_eq!(to_string(FloatKind::Single, s(1e-45), 0), "1.40129846E-45");
    }
}
