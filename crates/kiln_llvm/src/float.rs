//! Floating-point encodings: conversion from `f64` to the narrower and
//! wider formats LLVM supports, and LLVM's textual float syntax.

use std::fmt;

/// Decompose a finite non-zero `f64` into `(mantissa, exponent)` with
/// `value = mantissa * 2^exponent` (mantissa without trailing structure).
fn decompose(v: f64) -> (u64, i32) {
    let bits = v.to_bits();
    let exp = ((bits >> 52) & 0x7FF) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    if exp == 0 { (frac, -1074) } else { (frac | (1u64 << 52), exp - 1075) }
}

/// Right shift with round-to-nearest-even.
fn round_shift(m: u64, shift: u32) -> u64 {
    if shift == 0 {
        return m;
    }
    if shift > 64 {
        return 0;
    }
    let m = u128::from(m);
    let q = m >> shift;
    let rem = m & ((1u128 << shift) - 1);
    let half = 1u128 << (shift - 1);
    let up = rem > half || (rem == half && q & 1 == 1);
    (q + u128::from(up)) as u64
}

/// Convert to an IEEE binary format with `eb` exponent bits and `mb`
/// stored mantissa bits (`mb` < 52), rounding to nearest-even.
pub(crate) fn f64_to_minifloat(v: f64, eb: u32, mb: u32) -> u64 {
    let sign = u64::from(v.is_sign_negative()) << (eb + mb);
    let exp_all = (1u64 << eb) - 1;
    if v.is_nan() {
        let payload = (v.to_bits() & ((1u64 << 52) - 1)) >> (52 - mb);
        return sign | (exp_all << mb) | (1u64 << (mb - 1)) | payload;
    }
    if v.is_infinite() {
        return sign | (exp_all << mb);
    }
    if v == 0.0 {
        return sign;
    }
    let bias = (1i32 << (eb - 1)) - 1;
    let (m, e) = decompose(v);
    let p = 63 - m.leading_zeros() as i32;
    let ue = e + p;
    let field = ue + bias;
    let enc = if field >= 1 {
        let mant = round_shift(m, (p - mb as i32) as u32);
        ((field as u64 - 1) << mb) + mant
    } else {
        let et = 1 - bias - mb as i32;
        round_shift(m, (et - e) as u32)
    };
    if enc >= exp_all << mb { sign | (exp_all << mb) } else { sign | enc }
}

/// Exact conversion to IEEE binary128.
pub(crate) fn f64_to_fp128(v: f64) -> u128 {
    let sign = u128::from(v.is_sign_negative()) << 127;
    let exp_all = 0x7FFFu128;
    if v.is_nan() {
        let payload = u128::from(v.to_bits() & ((1u64 << 52) - 1)) << 60;
        return sign | (exp_all << 112) | (1u128 << 111) | payload;
    }
    if v.is_infinite() {
        return sign | (exp_all << 112);
    }
    if v == 0.0 {
        return sign;
    }
    let (m, e) = decompose(v);
    let p = 63 - m.leading_zeros() as i32;
    let field = (e + p + 16383) as u128;
    let mant = (u128::from(m) << (112 - p)) & ((1u128 << 112) - 1);
    sign | (field << 112) | mant
}

/// Exact conversion to x87 80-bit extended (in the low 80 bits).
pub(crate) fn f64_to_x87(v: f64) -> u128 {
    let sign = u128::from(v.is_sign_negative()) << 79;
    if v.is_nan() {
        let payload = u128::from(v.to_bits() & ((1u64 << 52) - 1)) << 11;
        return sign | (0x7FFFu128 << 64) | (0xC000_0000_0000_0000u128) | payload;
    }
    if v.is_infinite() {
        return sign | (0x7FFFu128 << 64) | 0x8000_0000_0000_0000u128;
    }
    if v == 0.0 {
        return sign;
    }
    let (m, e) = decompose(v);
    let p = 63 - m.leading_zeros() as i32;
    let field = (e + p + 16383) as u128;
    sign | (field << 64) | u128::from(m << (63 - p))
}

/// Write a `double`-valued constant the way LLVM's `AsmWriter` does:
/// `%.6e`-style decimal when it reparses exactly, else `0x` + 16 hex digits.
pub(crate) fn write_double<W: fmt::Write + ?Sized>(w: &mut W, v: f64) -> fmt::Result {
    if v.is_finite() {
        // Rust's `{:.5e}` gives six significant digits like `1.00000e-1`;
        // LLVM's APFloat::toString(6) prints `1.000000e-01`.
        let s = format!("{v:.5e}");
        #[allow(clippy::float_cmp)] // exact round trip is the point
        if s.parse::<f64>().is_ok_and(|r| r == v) {
            let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
            let (esign, edigits) = match exp.strip_prefix('-') {
                Some(d) => ('-', d),
                None => ('+', exp),
            };
            w.write_str(mant)?;
            w.write_str("0e")?;
            w.write_char(esign)?;
            if edigits.len() < 2 {
                w.write_char('0')?;
            }
            return w.write_str(edigits);
        }
    }
    write!(w, "0x{:X}", v.to_bits())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(v: f64) -> String {
        let mut s = String::new();
        write_double(&mut s, v).unwrap();
        s
    }

    #[test]
    fn double_syntax() {
        assert_eq!(d(0.1), "1.000000e-01");
        assert_eq!(d(1.0), "1.000000e+00");
        assert_eq!(d(-0.0), "-0.000000e+00");
        assert_eq!(d(5e-324), "4.940660e-324");
        assert_eq!(d(123_456.7), "0x40FE240B33333333");
        assert_eq!(d(f64::from(0.1f32)), "0x3FB99999A0000000");
        assert_eq!(d(f64::INFINITY), "0x7FF0000000000000");
        assert_eq!(d(1e100), "1.000000e+100");
    }

    #[test]
    fn conversions() {
        assert_eq!(f64_to_minifloat(1.0, 5, 10), 0x3C00);
        assert_eq!(f64_to_minifloat(-2.0, 5, 10), 0xC000);
        assert_eq!(f64_to_minifloat(65504.0, 5, 10), 0x7BFF);
        assert_eq!(f64_to_minifloat(1e6, 5, 10), 0x7C00);
        assert_eq!(f64_to_minifloat(5.960_464_477_539_063e-8, 5, 10), 1);
        assert_eq!(f64_to_minifloat(1.0, 8, 7), 0x3F80);
        assert_eq!(f64_to_minifloat(f64::from(f32::MAX), 8, 23), u64::from(f32::MAX.to_bits()));
        assert_eq!(f64_to_minifloat(1e-45, 8, 23), 1);
        assert_eq!(f64_to_fp128(1.5), 0x3FFF_8000_0000_0000u128 << 64);
        assert_eq!(f64_to_x87(1.0), (0x3FFFu128 << 64) | 0x8000_0000_0000_0000);
    }
}
