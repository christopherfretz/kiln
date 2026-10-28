//! Exact float printing: the shortest decimal that round-trips, in Rust's
//! `{:e}` notation (`1.5e0`, `-0e0`, `inf`, `nan`), for every
//! [`FloatKind`] (spec/ast.md §9.3).
//!
//! `float` and `double` use the standard library's formatter. The other
//! formats (binary16, bfloat16, x87 extended, binary128) use the
//! Burger–Dybvig free-format algorithm over a small bignum, which gives the
//! same digits (shortest; on a tie between two shortest candidates, the
//! upper one) — tests cross-check it against the standard library on
//! `f32`/`f64`.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "Burger–Dybvig notation: r/s value, mp/mm half-gaps, k exponent"
)]

use std::cmp::Ordering;

use super::value::FloatKind;

/// Appends the shortest round-trip form of the `kind` value with encoding
/// `bits` (zero-extended) to `out`: `[-]d[.ddd]e[-]x`, `inf`, `-inf` or
/// `nan` (any NaN, regardless of sign and payload).
pub fn format_float(kind: FloatKind, bits: u128, out: &mut String) {
    match kind {
        FloatKind::Single => push_std(&format!("{:e}", f32::from_bits(bits as u32)), out),
        FloatKind::Double => push_std(&format!("{:e}", f64::from_bits(bits as u64)), out),
        _ => format_generic(kind, bits, out),
    }
}

fn push_std(s: &str, out: &mut String) {
    out.push_str(if s == "NaN" { "nan" } else { s });
}

/// Encoding parameters of a binary interchange-like format.
struct Format {
    exp_bits: u32,
    /// Stored fraction bits (excluding an explicit integer bit).
    frac_bits: u32,
    /// x87: the integer bit is stored (bit `frac_bits`).
    explicit_int: bool,
}

fn format_of(kind: FloatKind) -> Format {
    let (exp_bits, frac_bits, explicit_int) = match kind {
        FloatKind::Half => (5, 10, false),
        FloatKind::BFloat16 => (8, 7, false),
        FloatKind::Single => (8, 23, false),
        FloatKind::Double => (11, 52, false),
        FloatKind::X87Extended => (15, 63, true),
        FloatKind::Quad => (15, 112, false),
    };
    Format { exp_bits, frac_bits, explicit_int }
}

/// The generic path; public to the crate so tests can compare it with the
/// standard library on `f32`/`f64`.
pub(crate) fn format_generic(kind: FloatKind, bits: u128, out: &mut String) {
    let f = format_of(kind);
    let int_bits = u32::from(f.explicit_int);
    let total = 1 + f.exp_bits + int_bits + f.frac_bits;
    let neg = bits >> (total - 1) & 1 != 0;
    let exp_field = (bits >> (f.frac_bits + int_bits)) as u32 & ((1 << f.exp_bits) - 1);
    let frac = bits & ((1u128 << f.frac_bits) - 1);
    let exp_max = (1u32 << f.exp_bits) - 1;
    if exp_field == exp_max {
        if frac == 0 {
            out.push_str(if neg { "-inf" } else { "inf" });
        } else {
            out.push_str("nan");
        }
        return;
    }
    if neg {
        out.push('-');
    }
    let int_bit = if f.explicit_int { bits >> f.frac_bits & 1 } else { u128::from(exp_field != 0) };
    let mant = (int_bit << f.frac_bits) | frac;
    if mant == 0 {
        out.push_str("0e0");
        return;
    }
    let bias = (1i64 << (f.exp_bits - 1)) - 1;
    let e = i64::from(exp_field.max(1)) - bias - i64::from(f.frac_bits);
    let lower_boundary = mant == 1u128 << f.frac_bits && exp_field > 1;
    let (digits, k) = shortest(mant, e, lower_boundary, mant & 1 == 0);
    out.push(char::from(digits[0]));
    if digits.len() > 1 {
        out.push('.');
        out.extend(digits[1..].iter().map(|&d| char::from(d)));
    }
    out.push('e');
    out.push_str(&(k - 1).to_string());
}

/// Burger & Dybvig free-format shortest digits of `mant * 2^e` (> 0).
/// Returns ASCII digits `d1 d2 ...` and `k` with value `0.d1d2... * 10^k`.
#[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    reason = "one algorithm; the exponent (|e| < 2^15) is exact in f64"
)]
fn shortest(mant: u128, e: i64, lower_boundary: bool, inclusive: bool) -> (Vec<u8>, i64) {
    // r / s is the value, m+ / s and m- / s the half-gaps to the neighbours.
    let (mut r, mut s, mut mp, mut mm);
    if e >= 0 {
        let be = Big::pow2(e as u32);
        if lower_boundary {
            r = Big::from(mant).shl((e + 2) as u32);
            s = Big::from(4);
            mp = be.clone().shl(1);
            mm = be;
        } else {
            r = Big::from(mant).shl((e + 1) as u32);
            s = Big::from(2);
            mp = be.clone();
            mm = be;
        }
    } else if lower_boundary {
        r = Big::from(mant).shl(2);
        s = Big::pow2((2 - e) as u32);
        mp = Big::from(2);
        mm = Big::from(1);
    } else {
        r = Big::from(mant).shl(1);
        s = Big::pow2((1 - e) as u32);
        mp = Big::from(1);
        mm = Big::from(1);
    }
    // Estimate k = ceil(log10(value)) and correct it.
    let bitlen = i64::from(128 - mant.leading_zeros());
    let mut k = ((bitlen - 1 + e) as f64 * std::f64::consts::LOG10_2).ceil() as i64;
    if k >= 0 {
        s.mul_pow10(k as u32);
    } else {
        let n = (-k) as u32;
        r.mul_pow10(n);
        mp.mul_pow10(n);
        mm.mul_pow10(n);
    }
    let too_low = |r: &Big, mp: &Big, s: &Big| {
        let hi = r.add(mp);
        if inclusive { hi.cmp(s) != Ordering::Less } else { hi.cmp(s) == Ordering::Greater }
    };
    while too_low(&r, &mp, &s) {
        s.mul_small(10);
        k += 1;
    }
    loop {
        let mut hi10 = r.add(&mp);
        hi10.mul_small(10);
        let ok = if inclusive {
            hi10.cmp(&s) != Ordering::Less
        } else {
            hi10.cmp(&s) == Ordering::Greater
        };
        if ok {
            break;
        }
        r.mul_small(10);
        mp.mul_small(10);
        mm.mul_small(10);
        k -= 1;
    }
    let mut digits = Vec::new();
    loop {
        r.mul_small(10);
        mp.mul_small(10);
        mm.mul_small(10);
        let mut d = 0u8;
        while r.cmp(&s) != Ordering::Less {
            r.sub_assign(&s);
            d += 1;
        }
        let low =
            if inclusive { r.cmp(&mm) != Ordering::Greater } else { r.cmp(&mm) == Ordering::Less };
        let hi = r.add(&mp);
        let high =
            if inclusive { hi.cmp(&s) != Ordering::Less } else { hi.cmp(&s) == Ordering::Greater };
        match (low, high) {
            (false, false) => digits.push(d),
            (true, false) => {
                digits.push(d);
                break;
            }
            (false, true) => {
                digits.push(d + 1);
                break;
            }
            (true, true) => {
                let twice = r.clone().shl(1);
                digits.push(if twice.cmp(&s) == Ordering::Less { d } else { d + 1 });
                break;
            }
        }
    }
    // A final digit of 10 carries (cannot happen with a correct k, but the
    // carry is cheap insurance).
    let mut i = digits.len() - 1;
    while digits[i] == 10 {
        digits[i] = 0;
        if i == 0 {
            digits.insert(0, 1);
            k += 1;
            break;
        }
        i -= 1;
        digits[i] += 1;
    }
    while digits.len() > 1 && digits[digits.len() - 1] == 0 {
        digits.pop();
    }
    (digits.into_iter().map(|d| b'0' + d).collect(), k)
}

/// A minimal unsigned bignum (little-endian `u32` limbs, no leading zero
/// limbs), enough for the digit loop above.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Big(Vec<u32>);

impl From<u128> for Big {
    fn from(mut v: u128) -> Self {
        let mut l = Vec::new();
        while v != 0 {
            l.push(v as u32);
            v >>= 32;
        }
        Big(l)
    }
}

impl Big {
    fn pow2(n: u32) -> Big {
        Big::from(1).shl(n)
    }

    fn trim(&mut self) {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }

    fn shl(mut self, n: u32) -> Big {
        if self.0.is_empty() {
            return self;
        }
        let (limbs, bits) = ((n / 32) as usize, n % 32);
        if bits != 0 {
            let mut carry = 0u32;
            for l in &mut self.0 {
                let v = (u64::from(*l) << bits) | u64::from(carry);
                *l = v as u32;
                carry = (v >> 32) as u32;
            }
            if carry != 0 {
                self.0.push(carry);
            }
        }
        if limbs != 0 {
            self.0.splice(0..0, std::iter::repeat_n(0, limbs));
        }
        self
    }

    fn mul_small(&mut self, m: u32) {
        let mut carry = 0u64;
        for l in &mut self.0 {
            let v = u64::from(*l) * u64::from(m) + carry;
            *l = v as u32;
            carry = v >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
        self.trim();
    }

    fn mul_pow10(&mut self, mut n: u32) {
        while n >= 9 {
            self.mul_small(1_000_000_000);
            n -= 9;
        }
        if n > 0 {
            self.mul_small(10u32.pow(n));
        }
    }

    fn add(&self, o: &Big) -> Big {
        let (a, b) = if self.0.len() >= o.0.len() { (self, o) } else { (o, self) };
        let mut out = Vec::with_capacity(a.0.len() + 1);
        let mut carry = 0u64;
        for (i, &x) in a.0.iter().enumerate() {
            let v = u64::from(x) + u64::from(b.0.get(i).copied().unwrap_or(0)) + carry;
            out.push(v as u32);
            carry = v >> 32;
        }
        if carry != 0 {
            out.push(carry as u32);
        }
        Big(out)
    }

    /// `self -= o`; requires `self >= o`.
    fn sub_assign(&mut self, o: &Big) {
        let mut borrow = 0i64;
        for (i, l) in self.0.iter_mut().enumerate() {
            let v = i64::from(*l) - i64::from(o.0.get(i).copied().unwrap_or(0)) - borrow;
            borrow = i64::from(v < 0);
            *l = v.rem_euclid(1 << 32) as u32;
        }
        debug_assert_eq!(borrow, 0, "Big::sub_assign underflow");
        self.trim();
    }

    fn cmp(&self, o: &Big) -> Ordering {
        self.0.len().cmp(&o.0.len()).then_with(|| self.0.iter().rev().cmp(o.0.iter().rev()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generic(kind: FloatKind, bits: u128) -> String {
        let mut s = String::new();
        format_generic(kind, bits, &mut s);
        s
    }

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    /// The bignum path agrees with the standard library's shortest
    /// formatting on `f64` (random bit patterns plus edge cases).
    #[test]
    fn generic_matches_std_f64() {
        let mut st = 0x9e37_79b9_7f4a_7c15u64;
        let mut cases: Vec<u64> = (0..20_000).map(|_| xorshift(&mut st)).collect();
        cases.extend([
            0,
            1,
            0x000f_ffff_ffff_ffff,
            0x0010_0000_0000_0000,
            0x7fef_ffff_ffff_ffff,
            1.5f64.to_bits(),
            0.1f64.to_bits(),
            1e23f64.to_bits(),
            5e-324f64.to_bits(),
            (-0.0f64).to_bits(),
            f64::INFINITY.to_bits(),
            f64::NEG_INFINITY.to_bits(),
            f64::NAN.to_bits(),
        ]);
        // Powers of two exercise the asymmetric lower boundary.
        cases.extend((1..2046u64).map(|e| e << 52));
        for b in cases {
            let mut want = String::new();
            format_float(FloatKind::Double, u128::from(b), &mut want);
            assert_eq!(generic(FloatKind::Double, u128::from(b)), want, "bits {b:#x}");
        }
    }

    #[test]
    fn generic_matches_std_f32() {
        let mut st = 0x1234_5678_9abc_def1u64;
        let mut cases: Vec<u32> = (0..20_000).map(|_| xorshift(&mut st) as u32).collect();
        cases.extend((1..254u32).map(|e| e << 23));
        cases.extend([0, 1, 0x007f_ffff, 0x0080_0000, 0x7f7f_ffff, 0.1f32.to_bits()]);
        for b in cases {
            let mut want = String::new();
            format_float(FloatKind::Single, u128::from(b), &mut want);
            assert_eq!(generic(FloatKind::Single, u128::from(b)), want, "bits {b:#x}");
        }
    }

    #[test]
    fn half_bfloat_x87_quad() {
        // binary16: 1.0, 0.1 (nearest 0.0999755859375, shortest "1e-1"),
        // max 65504, min subnormal 2^-24.
        assert_eq!(generic(FloatKind::Half, 0x3c00), "1e0");
        assert_eq!(generic(FloatKind::Half, 0x2e66), "1e-1");
        assert_eq!(generic(FloatKind::Half, 0x7bff), "6.55e4");
        assert_eq!(generic(FloatKind::Half, 0x0001), "6e-8");
        assert_eq!(generic(FloatKind::Half, 0xfc00), "-inf");
        assert_eq!(generic(FloatKind::Half, 0x7e00), "nan");
        assert_eq!(generic(FloatKind::BFloat16, 0x3fc0), "1.5e0");
        assert_eq!(generic(FloatKind::BFloat16, 0x8000), "-0e0");
        // x87: 1.0 = exp 0x3fff, integer bit set.
        assert_eq!(generic(FloatKind::X87Extended, (0x3fffu128 << 64) | (1u128 << 63)), "1e0");
        // x87 0.1L = 0x3ffb_cccc_cccc_cccc_cccd.
        assert_eq!(
            generic(FloatKind::X87Extended, (0x3ffbu128 << 64) | 0xcccc_cccc_cccc_cccd),
            "1e-1"
        );
        // x87 pi: 0x4000_c90f_daa2_2168_c235 = 3.14159265358979323851...
        assert_eq!(
            generic(FloatKind::X87Extended, (0x4000u128 << 64) | 0xc90f_daa2_2168_c235),
            "3.1415926535897932385e0"
        );
        // binary128 1.0 and 0.1 (0x3ffb_9999...9a).
        assert_eq!(generic(FloatKind::Quad, 0x3fffu128 << 112), "1e0");
        assert_eq!(generic(FloatKind::Quad, 0x3ffb_9999_9999_9999_9999_9999_9999_999a), "1e-1");
        assert_eq!(generic(FloatKind::Quad, 1u128 << 127), "-0e0");
        assert_eq!(generic(FloatKind::Quad, 0x7fffu128 << 112), "inf");
    }
}
