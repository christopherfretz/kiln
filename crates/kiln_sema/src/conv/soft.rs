//! Exact floating-point arithmetic for the conversions and literals:
//! correctly rounded (to nearest, ties to even) conversion of exact
//! rationals into each [`FloatKind`], decoding of encodings, and
//! clang's `%e`-style printing of values in diagnostics.
//!
//! Everything works on a small unsigned bignum; the quotients needed are
//! at most 115 bits wide, so division is a bit-serial loop over shifted
//! comparisons.

use std::cmp::Ordering;

use kiln_ast::consts::FloatKind;

/// An unsigned big integer, little-endian 32-bit limbs, no high zero limbs.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct Big(Vec<u32>);

impl Big {
    pub(crate) fn from_u128(mut v: u128) -> Big {
        let mut l = Vec::new();
        while v != 0 {
            #[allow(clippy::cast_possible_truncation, reason = "limb split")]
            l.push(v as u32);
            v >>= 32;
        }
        Big(l)
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.is_empty()
    }

    fn trim(&mut self) {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }

    pub(crate) fn bit_len(&self) -> u64 {
        match self.0.last() {
            None => 0,
            Some(&top) => (self.0.len() as u64 - 1) * 32 + u64::from(32 - top.leading_zeros()),
        }
    }

    pub(crate) fn mul_small(&mut self, m: u32) {
        let mut carry = 0u64;
        for l in &mut self.0 {
            let v = u64::from(*l) * u64::from(m) + carry;
            #[allow(clippy::cast_possible_truncation, reason = "low limb")]
            {
                *l = v as u32;
            }
            carry = v >> 32;
        }
        if carry != 0 {
            #[allow(clippy::cast_possible_truncation, reason = "carry fits a limb")]
            self.0.push(carry as u32);
        }
        self.trim();
    }

    pub(crate) fn add_small(&mut self, a: u32) {
        let mut carry = u64::from(a);
        for l in &mut self.0 {
            if carry == 0 {
                break;
            }
            let v = u64::from(*l) + carry;
            #[allow(clippy::cast_possible_truncation, reason = "low limb")]
            {
                *l = v as u32;
            }
            carry = v >> 32;
        }
        if carry != 0 {
            #[allow(clippy::cast_possible_truncation, reason = "carry fits a limb")]
            self.0.push(carry as u32);
        }
    }

    /// `self * 10^n`.
    pub(crate) fn mul_pow10(&mut self, mut n: u64) {
        while n >= 9 {
            self.mul_small(1_000_000_000);
            n -= 9;
        }
        if n > 0 {
            #[allow(clippy::cast_possible_truncation, reason = "n < 9")]
            self.mul_small(10u32.pow(n as u32));
        }
    }

    pub(crate) fn shl(&self, n: u64) -> Big {
        if self.is_zero() {
            return Big::default();
        }
        #[allow(clippy::cast_possible_truncation, reason = "bounded shift counts")]
        let (limbs, bits) = ((n / 32) as usize, (n % 32) as u32);
        let mut out = vec![0u32; limbs];
        if bits == 0 {
            out.extend_from_slice(&self.0);
        } else {
            let mut carry = 0u32;
            for &l in &self.0 {
                out.push((l << bits) | carry);
                carry = l >> (32 - bits);
            }
            if carry != 0 {
                out.push(carry);
            }
        }
        Big(out)
    }

    fn sub_assign(&mut self, o: &Big) {
        let mut borrow = 0i64;
        for i in 0..self.0.len() {
            let b = i64::from(o.0.get(i).copied().unwrap_or(0));
            let mut v = i64::from(self.0[i]) - b - borrow;
            borrow = 0;
            if v < 0 {
                v += 1 << 32;
                borrow = 1;
            }
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "0 <= v < 2^32"
            )]
            {
                self.0[i] = v as u32;
            }
        }
        self.trim();
    }

    pub(crate) fn cmp_big(&self, o: &Big) -> Ordering {
        self.0.len().cmp(&o.0.len()).then_with(|| self.0.iter().rev().cmp(o.0.iter().rev()))
    }

    /// Parses decimal digits (ASCII `0-9` only; others are skipped).
    pub(crate) fn from_decimal(digits: &str) -> Big {
        let mut b = Big::default();
        let mut chunk = 0u32;
        let mut n = 0u32;
        for c in digits.bytes().filter(u8::is_ascii_digit) {
            chunk = chunk * 10 + u32::from(c - b'0');
            n += 1;
            if n == 9 {
                b.mul_small(1_000_000_000);
                b.add_small(chunk);
                chunk = 0;
                n = 0;
            }
        }
        if n > 0 {
            b.mul_small(10u32.pow(n));
            b.add_small(chunk);
        }
        b
    }

    /// Parses hexadecimal digits (others are skipped).
    pub(crate) fn from_hex(digits: &str) -> Big {
        let mut b = Big::default();
        for c in digits.chars().filter_map(|c| c.to_digit(16)) {
            b.mul_small(16);
            b.add_small(c);
        }
        b
    }

    /// `floor(self / d)` when it is below `2^max_bits`, with how the
    /// remainder compares to half of `d`.
    fn div_bounded(&self, d: &Big, max_bits: u32) -> (u128, Ordering, bool) {
        let mut r = self.clone();
        let mut q = 0u128;
        for i in (0..max_bits).rev() {
            let t = d.shl(u64::from(i));
            if r.cmp_big(&t) != Ordering::Less {
                r.sub_assign(&t);
                q |= 1u128 << i;
            }
        }
        let inexact = !r.is_zero();
        let half = r.shl(1).cmp_big(d);
        (q, half, inexact)
    }
}

/// Precision (with the leading bit), maximum and minimum normal exponent
/// of `kind`.
pub(crate) fn params(kind: FloatKind) -> (u32, i64, i64) {
    match kind {
        FloatKind::Half => (11, 15, -14),
        FloatKind::BFloat16 => (8, 127, -126),
        FloatKind::Single => (24, 127, -126),
        FloatKind::Double => (53, 1023, -1022),
        FloatKind::X87Extended => (64, 16383, -16382),
        FloatKind::Quad => (113, 16383, -16382),
    }
}

/// Width of the exponent field.
fn exp_bits(kind: FloatKind) -> u32 {
    match kind {
        FloatKind::Half => 5,
        FloatKind::BFloat16 | FloatKind::Single => 8,
        FloatKind::Double => 11,
        FloatKind::X87Extended | FloatKind::Quad => 15,
    }
}

/// Total encoding width.
fn total_bits(kind: FloatKind) -> u32 {
    match kind {
        FloatKind::Half | FloatKind::BFloat16 => 16,
        FloatKind::Single => 32,
        FloatKind::Double => 64,
        FloatKind::X87Extended => 80,
        FloatKind::Quad => 128,
    }
}

/// A rounded result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Rounded {
    /// The encoding.
    pub bits: u128,
    /// The magnitude exceeded the largest finite value (`bits` is ±inf).
    pub overflow: bool,
    /// The result is not the exact value.
    pub inexact: bool,
    /// The exact value was nonzero but the result is zero.
    pub underflow_to_zero: bool,
}

/// A decoded value: `(-1)^neg * mant * 2^exp`, or a special.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Decoded {
    /// A finite value (`mant == 0` for zero).
    Finite { neg: bool, mant: u128, exp: i64 },
    /// An infinity.
    Inf { neg: bool },
    /// A NaN.
    NaN,
}

fn encode(kind: FloatKind, neg: bool, mant: u128, exp: i64) -> u128 {
    let (p, emax, emin) = params(kind);
    let sign = u128::from(neg) << (total_bits(kind) - 1);
    if mant == 0 {
        return sign;
    }
    let normal = mant >> (p - 1) != 0;
    let bias = emax;
    let field = if normal { exp + i64::from(p) - 1 + bias } else { 0 };
    let _ = emin;
    #[allow(clippy::cast_sign_loss, reason = "biased exponents are positive")]
    let field = field as u128;
    match kind {
        FloatKind::X87Extended => sign | (field << 64) | mant,
        _ => sign | (field << (p - 1)) | (mant & ((1u128 << (p - 1)) - 1)),
    }
}

/// The encoding of ±infinity in `kind`.
pub(crate) fn infinity(kind: FloatKind, neg: bool) -> u128 {
    let e = (1u128 << exp_bits(kind)) - 1;
    let sign = u128::from(neg) << (total_bits(kind) - 1);
    match kind {
        FloatKind::X87Extended => sign | (e << 64) | (1u128 << 63),
        _ => sign | (e << (params(kind).0 - 1)),
    }
}

/// Decodes encoding `bits` of `kind`.
pub(crate) fn decode(kind: FloatKind, bits: u128) -> Decoded {
    let (p, emax, emin) = params(kind);
    let tb = total_bits(kind);
    let neg = (bits >> (tb - 1)) & 1 == 1;
    let eb = exp_bits(kind);
    let (field, frac) = match kind {
        FloatKind::X87Extended => ((bits >> 64) & 0x7fff, bits & u128::from(u64::MAX)),
        _ => ((bits >> (p - 1)) & ((1u128 << eb) - 1), bits & ((1u128 << (p - 1)) - 1)),
    };
    let all_ones = (1u128 << eb) - 1;
    if field == all_ones {
        let payload = if kind == FloatKind::X87Extended { frac & !(1u128 << 63) } else { frac };
        return if payload == 0 { Decoded::Inf { neg } } else { Decoded::NaN };
    }
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation, reason = "15-bit field")]
    let f = field as i64;
    let lead = i64::from(p) - 1;
    if field == 0 {
        return Decoded::Finite { neg, mant: frac, exp: emin - lead };
    }
    let mant = match kind {
        FloatKind::X87Extended => frac,
        _ => frac | (1u128 << (p - 1)),
    };
    Decoded::Finite { neg, mant, exp: f - emax - lead }
}

/// Rounds `num / den * 2^scale` (with sign `neg`) to `kind`.
pub(crate) fn round_rational(
    kind: FloatKind,
    neg: bool,
    num: &Big,
    den: &Big,
    scale: i64,
) -> Rounded {
    let (p, emax, emin) = params(kind);
    if num.is_zero() {
        return Rounded {
            bits: encode(kind, neg, 0, 0),
            overflow: false,
            inexact: false,
            underflow_to_zero: false,
        };
    }
    #[allow(clippy::cast_possible_wrap, reason = "bit lengths are small")]
    let (ln, ld) = (num.bit_len() as i64, den.bit_len() as i64);
    // value in [2^(ln-ld-1+scale), 2^(ln-ld+1+scale)).
    let top = ln - ld + scale; // the leading bit's exponent is top - 1 or top
    if top - 1 > emax + 1 {
        return Rounded {
            bits: infinity(kind, neg),
            overflow: true,
            inexact: true,
            underflow_to_zero: false,
        };
    }
    let min_e = emin - (i64::from(p) - 1);
    if top < min_e - 2 {
        return Rounded {
            bits: encode(kind, neg, 0, 0),
            overflow: false,
            inexact: true,
            underflow_to_zero: true,
        };
    }
    // Choose e so the quotient has p or p + 1 bits.
    let mut e = (top - i64::from(p) - 1).max(min_e);
    let attempt = |e: i64| {
        // q = num * 2^(scale - e) / den
        let sh = scale - e;
        #[allow(clippy::cast_sign_loss, reason = "sign checked")]
        let (n, d) = if sh >= 0 {
            (num.shl(sh as u64), den.clone())
        } else {
            (num.clone(), den.shl((-sh) as u64))
        };
        n.div_bounded(&d, p + 3)
    };
    let (mut q, mut half, mut inexact) = attempt(e);
    while q >> p != 0 {
        e += 1;
        (q, half, inexact) = attempt(e);
    }
    let round_up = match half {
        Ordering::Greater => true,
        Ordering::Equal => q & 1 == 1,
        Ordering::Less => false,
    };
    if round_up {
        q += 1;
        if q >> p != 0 {
            q >>= 1;
            e += 1;
        }
    }
    let lead_exp = if q == 0 { min_e } else { e + 127 - i64::from(q.leading_zeros()) };
    if q != 0 && lead_exp > emax {
        return Rounded {
            bits: infinity(kind, neg),
            overflow: true,
            inexact: true,
            underflow_to_zero: false,
        };
    }
    if q == 0 {
        return Rounded {
            bits: encode(kind, neg, 0, 0),
            overflow: false,
            inexact: true,
            underflow_to_zero: true,
        };
    }
    // Normalise a normal result to exactly p bits at exponent e.
    let bits = encode(kind, neg, q, e);
    Rounded { bits, overflow: false, inexact, underflow_to_zero: false }
}

/// Rounds the integer `±v` to `kind`.
pub(crate) fn from_int(kind: FloatKind, neg: bool, v: u128) -> Rounded {
    round_rational(kind, neg, &Big::from_u128(v), &Big::from_u128(1), 0)
}

/// Converts encoding `bits` of `from` to `to` (rounding to nearest).
pub(crate) fn convert(from: FloatKind, to: FloatKind, bits: u128) -> Rounded {
    match decode(from, bits) {
        Decoded::NaN => {
            let q = infinity(to, false)
                | match to {
                    FloatKind::X87Extended => 1u128 << 62,
                    _ => 1u128 << (params(to).0 - 2),
                };
            Rounded { bits: q, overflow: false, inexact: false, underflow_to_zero: false }
        }
        Decoded::Inf { neg } => Rounded {
            bits: infinity(to, neg),
            overflow: false,
            inexact: false,
            underflow_to_zero: false,
        },
        Decoded::Finite { neg, mant, exp } => {
            round_rational(to, neg, &Big::from_u128(mant), &Big::from_u128(1), exp)
        }
    }
}

/// The value of encoding `bits` truncated toward zero to an integer, if
/// finite and below `2^128` in magnitude: `(neg, magnitude, exact)`.
pub(crate) fn to_int(kind: FloatKind, bits: u128) -> Option<(bool, u128, bool)> {
    match decode(kind, bits) {
        Decoded::Finite { neg, mant, exp } => {
            if exp >= 0 {
                if exp >= 128 || (mant != 0 && i64::from(mant.leading_zeros()) < exp) {
                    return None;
                }
                Some((neg, mant << exp, true))
            } else if exp <= -128 {
                Some((neg, 0, mant == 0))
            } else {
                let sh = -exp;
                let v = mant >> sh;
                Some((neg, v, (v << sh) == mant))
            }
        }
        _ => None,
    }
}

/// Formats encoding `bits` of `kind` as C's `%e` (six digits after the
/// point, at least two exponent digits): how clang prints a floating
/// value in a narrowing diagnostic (`1.000000e+300`).
pub(crate) fn format_e6(kind: FloatKind, bits: u128) -> String {
    let (neg, mant, exp) = match decode(kind, bits) {
        Decoded::NaN => return "nan".into(),
        Decoded::Inf { neg } => return if neg { "-inf".into() } else { "inf".into() },
        Decoded::Finite { neg, mant, exp } => (neg, mant, exp),
    };
    let sign = if neg { "-" } else { "" };
    if mant == 0 {
        return format!("{sign}0.000000e+00");
    }
    // Estimate the decimal exponent k = floor(log10(v)).
    let lead = exp + 127 - i64::from(mant.leading_zeros());
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, reason = "estimate")]
    let mut k = ((lead as f64) * std::f64::consts::LOG10_2).floor() as i64;
    for _ in 0..4 {
        // digits = round(v * 10^(6 - k)).
        let mut num = Big::from_u128(mant);
        let mut den = Big::from_u128(1);
        let t = 6 - k;
        #[allow(clippy::cast_sign_loss, reason = "sign checked")]
        if t >= 0 {
            num.mul_pow10(t as u64);
        } else {
            den.mul_pow10((-t) as u64);
        }
        #[allow(clippy::cast_sign_loss, reason = "sign checked")]
        let (num, den) =
            if exp >= 0 { (num.shl(exp as u64), den) } else { (num, den.shl((-exp) as u64)) };
        let (mut q, half, _) = num.div_bounded(&den, 30);
        if q < 1_000_000 {
            k -= 1;
            continue;
        }
        if q >= 10_000_000 {
            k += 1;
            continue;
        }
        if half == Ordering::Greater || (half == Ordering::Equal && q & 1 == 1) {
            q += 1;
        }
        if q == 10_000_000 {
            q = 1_000_000;
            k += 1;
        }
        let d = q.to_string();
        let es = if k < 0 { '-' } else { '+' };
        return format!("{sign}{}.{}e{es}{:02}", &d[..1], &d[1..], k.abs());
    }
    format!("{sign}0.000000e+00")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dbl(v: f64) -> u128 {
        u128::from(v.to_bits())
    }

    #[test]
    fn round_matches_std_for_double() {
        for s in [
            "0.1",
            "1",
            "3.14159",
            "1e300",
            "2.2250738585072014e-308",
            "4.9e-324",
            "1e-320",
            "123456789012345678901234567890",
        ] {
            let v: f64 = s.parse().unwrap();
            let (m, e) = split(s);
            let r = decimal(FloatKind::Double, &m, e);
            assert_eq!(r.bits, dbl(v), "{s}");
        }
    }

    fn split(s: &str) -> (String, i64) {
        let (mant, exp) = match s.split_once('e') {
            Some((m, e)) => (m, e.parse::<i64>().unwrap()),
            None => (s, 0),
        };
        match mant.split_once('.') {
            Some((i, f)) => (format!("{i}{f}"), exp - f.len() as i64),
            None => (mant.to_string(), exp),
        }
    }

    fn decimal(kind: FloatKind, digits: &str, e10: i64) -> Rounded {
        let mut n = Big::from_decimal(digits);
        let mut d = Big::from_u128(1);
        if e10 >= 0 {
            n.mul_pow10(e10 as u64);
        } else {
            d.mul_pow10((-e10) as u64);
        }
        round_rational(kind, false, &n, &d, 0)
    }

    #[test]
    fn single_and_half() {
        let r = decimal(FloatKind::Single, "1", 0);
        assert_eq!(r.bits, u128::from(1.0f32.to_bits()));
        let r = decimal(FloatKind::Single, "34028236", 31);
        assert!(r.overflow);
        let r = decimal(FloatKind::Half, "65504", 0);
        assert_eq!(r.bits, 0x7bff);
        let r = decimal(FloatKind::Half, "65520", 0);
        assert!(r.overflow);
        assert_eq!(decimal(FloatKind::BFloat16, "1", 0).bits, 0x3f80);
        assert_eq!(decimal(FloatKind::X87Extended, "1", 0).bits, (16383u128 << 64) | (1u128 << 63));
        assert_eq!(decimal(FloatKind::Quad, "1", 0).bits, 16383u128 << 112);
    }

    #[test]
    fn convert_and_print() {
        let r = convert(FloatKind::Double, FloatKind::Single, dbl(0.1));
        assert_eq!(r.bits, u128::from(0.1f32.to_bits()));
        assert!(convert(FloatKind::Double, FloatKind::Single, dbl(1e300)).overflow);
        assert_eq!(format_e6(FloatKind::Double, dbl(1e300)), "1.000000e+300");
        assert_eq!(format_e6(FloatKind::Double, dbl(3.402_823_57e38)), "3.402824e+38");
        assert_eq!(format_e6(FloatKind::Double, dbl(-0.5)), "-5.000000e-01");
        assert_eq!(format_e6(FloatKind::Double, dbl(9.999_999_9)), "1.000000e+01");
        assert_eq!(format_e6(FloatKind::Double, dbl(0.0)), "0.000000e+00");
        assert_eq!(to_int(FloatKind::Double, dbl(-2.5)), Some((true, 2, false)));
        assert!(from_int(FloatKind::Single, false, 16_777_217).inexact);
        assert!(!from_int(FloatKind::Single, false, 16_777_216).inexact);
    }
}
