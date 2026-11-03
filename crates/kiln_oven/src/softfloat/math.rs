//! The exact `<cmath>` functions of P0533 ([c.math], C23 7.12, IEEE
//! 754-2019 §5) in every format, for the Oven's `bi::FP_MATH` builtins
//! (spec/oven.md "Floating-point math builtins"): nextafter, frexp,
//! ldexp, ilogb/logb, modf, the rounding functions, fmod/remainder/remquo,
//! fma and fdim; and P1383's `sqrt` (WP-393).
//!
//! Each result is the correctly rounded IEEE result under the default
//! environment (round to nearest even), computed on unpacked values, never
//! through the host libm (`sqrt` of binary32/binary64 is the host's IEEE
//! `squareRoot`, which Rust guarantees correctly rounded). A call that would raise an IEEE exception other
//! than inexact (invalid, division by zero, overflow, or underflow: a tiny
//! inexact result, and any subnormal or zero `nextafter` result that moves)
//! is not a constant ([library.c]/3, C23 F.8.4 as GCC applies it), and so
//! is any operation on a signaling NaN: those return `None`.

use std::cmp::Ordering;

use kiln_ast::consts::FloatKind;

use super::{Class, Fmt, Val, add, classify, cmp, is_signaling, pack, pack_zero, repack, unpack};

/// The rounding of [`round_int`] and [`to_int`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Rounding {
    /// `ceil`: toward +infinity.
    Up,
    /// `floor`: toward -infinity.
    Down,
    /// `trunc`: toward zero.
    Zero,
    /// `round`/`lround`: to nearest, ties away from zero.
    Away,
    /// `rint`/`nearbyint`/`lrint`/`roundeven`: to nearest, ties to even
    /// (the default rounding mode).
    Even,
}

/// `a` with its quiet bit set (and, x87, its integer bit).
fn quiet(kind: FloatKind, a: u128) -> u128 {
    let f = Fmt::of(kind);
    let int = if f.explicit { 1 << (f.p - 1) } else { 0 };
    a | 1 << (f.p - 2) | int
}

/// Whether an operation's operands include a NaN, and if so its result.
enum NanIn {
    /// No operand is a NaN.
    No,
    /// Some operand is a NaN: `None` (not a constant) if one is signaling,
    /// else the first NaN, quieted, in the result kind.
    Yes(Option<u128>),
}

/// The NaN result of an operation with operands `xs` (each with its kind).
fn nan_result(kind: FloatKind, xs: &[(FloatKind, u128)]) -> NanIn {
    if xs.iter().any(|&(k, a)| is_signaling(k, a)) {
        return NanIn::Yes(None);
    }
    match xs.iter().find(|&&(k, a)| matches!(unpack(k, a), Val::NaN(..))) {
        None => NanIn::No,
        Some(&(k, a)) => {
            NanIn::Yes(Some(if k == kind { quiet(kind, a) } else { repack(kind, unpack(k, a)) }))
        }
    }
}

fn tiny(kind: FloatKind, r: u128) -> bool {
    matches!(classify(kind, r), Class::Subnormal | Class::Zero)
}

/// The largest finite value of `kind` with sign `s`.
fn max_finite(kind: FloatKind, s: bool) -> Val {
    let f = Fmt::of(kind);
    Val::Fin(s, f.bias(), ((1u128 << f.p) - 1) << (128 - f.p))
}

/// `nextafter(x, y)` (`nexttoward` when `ykind` is wider than `kind`): the
/// neighbour of `x` in `kind` in the direction of `y`; `y` (converted)
/// when they are equal.
pub(crate) fn nextafter(kind: FloatKind, x: u128, ykind: FloatKind, y: u128) -> Option<u128> {
    if let NanIn::Yes(r) = nan_result(kind, &[(kind, x), (ykind, y)]) {
        return r;
    }
    let up = match cmp(ykind, super::cast(kind, ykind, x), y)? {
        Ordering::Equal => return Some(super::cast(ykind, kind, y)),
        o => o == Ordering::Less,
    };
    let f = Fmt::of(kind);
    let r = match unpack(kind, x) {
        // The smallest subnormal: an underflow.
        Val::Zero(_) | Val::NaN(..) => return None,
        Val::Inf(s) => max_finite(kind, s),
        v @ Val::Fin(s, e, m) => {
            let lsb = e.max(f.emin()) - (f.p as i32 - 1);
            let away = up != s;
            let step = if !away && m == 1 << 127 && e > f.emin() { lsb - 1 } else { lsb };
            add(kind, v, Val::Fin(if away { s } else { !s }, step, 1 << 127))
        }
    };
    let bits = repack(kind, r);
    (classify(kind, bits) == Class::Normal).then_some(bits)
}

/// `frexp(x)`: the fraction in [0.5, 1) with `x`'s sign and the exponent;
/// zero, infinity and NaN are returned with exponent 0 (GCC).
pub(crate) fn frexp(kind: FloatKind, x: u128) -> Option<(u128, i32)> {
    match unpack(kind, x) {
        Val::Fin(s, e, m) => Some((pack(kind, s, -1, m), e + 1)),
        Val::NaN(..) if is_signaling(kind, x) => None,
        Val::NaN(..) => Some((quiet(kind, x), 0)),
        _ => Some((x, 0)),
    }
}

/// `ldexp`/`scalbn`/`scalbln(x, n)`: exact unless it overflows or an
/// underflow loses bits.
pub(crate) fn ldexp(kind: FloatKind, x: u128, n: i128) -> Option<u128> {
    match unpack(kind, x) {
        Val::Fin(s, e, m) => {
            // Beyond +-2^17 every format has over- or underflowed already.
            let ne = (i128::from(e) + n.clamp(-1 << 17, 1 << 17)) as i32;
            let r = pack(kind, s, ne, m);
            (unpack(kind, r) == Val::Fin(s, ne, m)).then_some(r)
        }
        Val::NaN(..) if is_signaling(kind, x) => None,
        Val::NaN(..) => Some(quiet(kind, x)),
        _ => Some(x),
    }
}

/// `ilogb(x)`: the unbiased exponent; zero, infinity and NaN are invalid.
pub(crate) fn ilogb(kind: FloatKind, x: u128) -> Option<i32> {
    match unpack(kind, x) {
        Val::Fin(_, e, _) => Some(e),
        _ => None,
    }
}

/// `logb(x)`: the unbiased exponent as a value of `kind`; `logb(0)`
/// divides by zero; `logb(+-inf)` is +inf.
pub(crate) fn logb(kind: FloatKind, x: u128) -> Option<u128> {
    match unpack(kind, x) {
        Val::Fin(_, e, _) => Some(super::from_int(kind, e as i128 as u128, true)),
        Val::Zero(_) => None,
        Val::Inf(_) => Some(super::inf(kind)),
        Val::NaN(..) if is_signaling(kind, x) => None,
        Val::NaN(..) => Some(quiet(kind, x)),
    }
}

/// The integral part of `m * 2^(e - 127)` and its fraction scaled by
/// 2^128 (inexact low bits folded into bit 0).
fn split(e: i32, m: u128) -> (u128, u128) {
    if e >= 127 {
        (u128::MAX, 0)
    } else if e >= 0 {
        let d = 127 - e as u32;
        (m >> d, if d == 0 { 0 } else { m << (128 - d) })
    } else {
        (0, super::shr_sticky(m, (-e - 1) as u32))
    }
}

fn bump(int: u128, frac: u128, s: bool, mode: Rounding) -> bool {
    const HALF: u128 = 1 << 127;
    match mode {
        Rounding::Zero => false,
        Rounding::Up => !s && frac != 0,
        Rounding::Down => s && frac != 0,
        Rounding::Away => frac >= HALF,
        Rounding::Even => frac > HALF || (frac == HALF && int & 1 == 1),
    }
}

/// `ceil`, `floor`, `trunc`, `round`, `roundeven`, `rint`, `nearbyint`:
/// `x` rounded to an integral value of `kind` (the sign of a zero result
/// is `x`'s).
pub(crate) fn round_int(kind: FloatKind, x: u128, mode: Rounding) -> Option<u128> {
    match unpack(kind, x) {
        Val::Fin(s, e, m) if e < Fmt::of(kind).p as i32 - 1 => {
            let (int, frac) = split(e, m);
            let mag = int + u128::from(bump(int, frac, s, mode));
            Some(if mag == 0 { pack_zero(Fmt::of(kind), s) } else { pack(kind, s, 127, mag) })
        }
        Val::NaN(..) if is_signaling(kind, x) => None,
        Val::NaN(..) => Some(quiet(kind, x)),
        _ => Some(x),
    }
}

/// `lround`/`llround`/`lrint`/`llrint`: `x` rounded to a signed integer of
/// `width` bits (two's complement in 128 bits); NaN, infinity and a value
/// out of range are invalid.
pub(crate) fn to_int(kind: FloatKind, x: u128, mode: Rounding, width: u32) -> Option<u128> {
    let (s, mag) = match unpack(kind, x) {
        Val::Zero(_) => return Some(0),
        Val::Fin(s, e, m) if e < 127 => {
            let (int, frac) = split(e, m);
            (s, int + u128::from(bump(int, frac, s, mode)))
        }
        _ => return None,
    };
    let limit = 1u128 << (width.clamp(1, 128) - 1);
    if mag > limit || (mag == limit && !s) {
        return None;
    }
    Some(if s { mag.wrapping_neg() } else { mag })
}

/// `modf(x)`: the fraction and the integral part, each with `x`'s sign.
pub(crate) fn modf(kind: FloatKind, x: u128) -> Option<(u128, u128)> {
    let f = Fmt::of(kind);
    match unpack(kind, x) {
        Val::Inf(s) => Some((pack_zero(f, s), x)),
        Val::Fin(s, ..) => {
            let int = round_int(kind, x, Rounding::Zero)?;
            let frac = super::bin(crate::kbc::Op::FSub, kind, x, int)?;
            let frac = if classify(kind, frac) == Class::Zero { pack_zero(f, s) } else { frac };
            Some((frac, int))
        }
        Val::NaN(..) if is_signaling(kind, x) => None,
        Val::NaN(..) => Some((quiet(kind, x), quiet(kind, x))),
        Val::Zero(_) => Some((x, x)),
    }
}

/// `fmod(x, y)` (`nearest` false) or `remainder`/`remquo(x, y)`: the
/// exact remainder and the low 31 bits of the quotient's magnitude with
/// its sign (GCC's `remquo` fold). An infinite `x` or a zero `y` is
/// invalid.
pub(crate) fn rem(kind: FloatKind, x: u128, y: u128, nearest: bool) -> Option<(u128, i32)> {
    if let NanIn::Yes(r) = nan_result(kind, &[(kind, x), (kind, y)]) {
        return r.map(|n| (n, 0));
    }
    let p = Fmt::of(kind).p;
    let (sx, ex, mx, sy, ey, my) = match (unpack(kind, x), unpack(kind, y)) {
        (Val::Inf(_) | Val::NaN(..), _) | (_, Val::Zero(_) | Val::NaN(..)) => return None,
        (Val::Zero(_), _) | (_, Val::Inf(_)) => return Some((x, 0)),
        (Val::Fin(sx, ex, mx), Val::Fin(sy, ey, my)) => (sx, ex, mx, sy, ey, my),
    };
    let (xi, yi) = (mx >> (128 - p), my >> (128 - p));
    let d = ex - ey;
    if d < -1 {
        // |x| < |y| / 2.
        return Some((x, 0));
    }
    // r = (|x| mod |y|) in units of 2^scale; q the quotient.
    let (mut r, mut q, ydiv, scale) = if d == -1 {
        (xi, 0u128, yi << 1, ex - (p as i32 - 1))
    } else {
        let mut r = xi % yi;
        let mut q = xi / yi;
        let mut left = d as u32;
        while left > 0 {
            let k = left.min(yi.leading_zeros());
            r <<= k;
            q = (q << k) | (r / yi);
            r %= yi;
            left -= k;
        }
        (r, q, yi, ey - (p as i32 - 1))
    };
    let mut neg = sx;
    if nearest {
        let twice = r << 1;
        if twice > ydiv || (twice == ydiv && q & 1 == 1) {
            r = ydiv - r;
            neg = !neg;
            q = q.wrapping_add(1);
        }
    }
    let v = if r == 0 { pack_zero(Fmt::of(kind), sx) } else { pack(kind, neg, scale + 127, r) };
    let qm = (q & 0x7FFF_FFFF) as i32;
    Some((v, if sx == sy { qm } else { -qm }))
}

/// `fdim(x, y)`: `x - y` when `x > y`, else +0; an overflow is not a
/// constant.
pub(crate) fn fdim(kind: FloatKind, x: u128, y: u128) -> Option<u128> {
    if let NanIn::Yes(r) = nan_result(kind, &[(kind, x), (kind, y)]) {
        return r;
    }
    if cmp(kind, x, y) != Some(Ordering::Greater) {
        return Some(pack_zero(Fmt::of(kind), false));
    }
    let r = super::bin(crate::kbc::Op::FSub, kind, x, y)?;
    let finite = |a| !matches!(unpack(kind, a), Val::Inf(_));
    (finite(r) || !finite(x) || !finite(y)).then_some(r)
}

/// A 256-bit unsigned integer `(high, low)`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct U256(u128, u128);

impl U256 {
    fn mul(a: u128, b: u128) -> U256 {
        let (al, ah) = (a as u64 as u128, a >> 64);
        let (bl, bh) = (b as u64 as u128, b >> 64);
        let ll = al * bl;
        let (mid, c1) = (al * bh).overflowing_add(ah * bl);
        let lo = ll.wrapping_add(mid << 64);
        let carry = u128::from(lo < ll);
        let hi = ah * bh + (mid >> 64) + (u128::from(c1) << 64) + carry;
        U256(hi, lo)
    }

    fn leading_zeros(self) -> u32 {
        if self.0 != 0 { self.0.leading_zeros() } else { 128 + self.1.leading_zeros() }
    }

    fn shl(self, d: u32) -> U256 {
        match d {
            0 => self,
            1..128 => U256(self.0 << d | self.1 >> (128 - d), self.1 << d),
            128..256 => U256(self.1 << (d - 128), 0),
            _ => U256(0, 0),
        }
    }

    /// Shifted right, the lost bits `OR`ed into bit 0.
    fn shr_sticky(self, d: u32) -> U256 {
        let r = match d {
            0 => return self,
            1..128 => U256(self.0 >> d, self.1 >> d | self.0 << (128 - d)),
            128..256 => U256(0, self.0 >> (d - 128)),
            _ => U256(0, 0),
        };
        U256(r.0, r.1 | u128::from(self != r.shl(d)))
    }

    fn add(self, o: U256) -> U256 {
        let (lo, c) = self.1.overflowing_add(o.1);
        U256(self.0 + o.0 + u128::from(c), lo)
    }

    fn sub(self, o: U256) -> U256 {
        let (lo, b) = self.1.overflowing_sub(o.1);
        U256(self.0 - o.0 - u128::from(b), lo)
    }
}

/// `fma(x, y, z)`: `x * y + z` with a single rounding.
pub(crate) fn fma(kind: FloatKind, x: u128, y: u128, z: u128) -> Option<u128> {
    let f = Fmt::of(kind);
    let (a, b, c) = (unpack(kind, x), unpack(kind, y), unpack(kind, z));
    if matches!((a, b), (Val::Inf(_), Val::Zero(_)) | (Val::Zero(_), Val::Inf(_))) {
        return None;
    }
    if let NanIn::Yes(r) = nan_result(kind, &[(kind, x), (kind, y), (kind, z)]) {
        return r;
    }
    let sign = |v| matches!(v, Val::Zero(true) | Val::Inf(true) | Val::Fin(true, ..));
    let sp = sign(a) != sign(b);
    let (ea, ma, eb, mb) = match (a, b) {
        (Val::Inf(_), _) | (_, Val::Inf(_)) => {
            return match c {
                Val::Inf(sc) if sc != sp => None,
                _ => Some(super::pack_inf(f, sp)),
            };
        }
        _ if matches!(c, Val::Inf(_)) => return Some(z),
        (Val::Zero(_), _) | (_, Val::Zero(_)) => {
            return Some(match c {
                Val::Zero(sc) => pack_zero(f, sp && sc),
                _ => z,
            });
        }
        (Val::Fin(_, ea, ma), Val::Fin(_, eb, mb)) => (ea, ma, eb, mb),
        _ => return None,
    };
    let lsb = |e: i32| e - (f.p as i32 - 1);
    // Each term as (sign, 256-bit integer with bit 254 its top, lsb exponent).
    let norm = |m: U256, e: i32| {
        let lz = m.leading_zeros();
        (m.shl(lz).shr_sticky(1), e - lz as i32 + 1)
    };
    let (pm, pe) = norm(U256::mul(ma >> (128 - f.p), mb >> (128 - f.p)), lsb(ea) + lsb(eb));
    let (s, m, e) = match c {
        Val::Fin(sc, ec, mc) => {
            let (cm, ce) = norm(U256(0, mc >> (128 - f.p)), lsb(ec));
            let ((s1, m1, e1), (s2, m2, e2)) = if (pe, pm) >= (ce, cm) {
                ((sp, pm, pe), (sc, cm, ce))
            } else {
                ((sc, cm, ce), (sp, pm, pe))
            };
            let m2 = m2.shr_sticky((e1 - e2).min(300) as u32);
            if s1 == s2 {
                (s1, m1.add(m2), e1)
            } else if m1 == m2 {
                return Some(pack_zero(f, false));
            } else {
                (s1, m1.sub(m2), e1)
            }
        }
        _ => (sp, pm, pe),
    };
    // To a 128-bit significand with a sticky bit.
    let lz = m.leading_zeros();
    let m = m.shl(lz);
    let (m128, lost) = (m.0 | u128::from(m.1 != 0), m.1 != 0);
    let e = e - lz as i32 + 255;
    let r = pack(kind, s, e, m128);
    if matches!(unpack(kind, r), Val::Inf(_)) {
        return None;
    }
    let exact = !lost && unpack(kind, r) == Val::Fin(s, e, m128);
    (!tiny(kind, r) || exact).then_some(r)
}

/// `sqrt(x)` correctly rounded (IEEE 754-2019 §5.4.1, C23 F.10.4.5):
/// `sqrt(+-0)` is `+-0`, `sqrt(+inf)` is `+inf`, a quiet NaN is returned
/// quiet; a negative operand (`-inf` included) or a signaling NaN is
/// invalid. The result never overflows or underflows.
pub(crate) fn sqrt(kind: FloatKind, x: u128) -> Option<u128> {
    match unpack(kind, x) {
        Val::NaN(..) if is_signaling(kind, x) => None,
        Val::NaN(..) => Some(quiet(kind, x)),
        Val::Zero(_) | Val::Inf(false) => Some(x),
        Val::Inf(true) | Val::Fin(true, ..) => None,
        Val::Fin(false, e, m) => Some(match kind {
            FloatKind::Single => u128::from(f32::from_bits(x as u32).sqrt().to_bits()),
            FloatKind::Double => u128::from(f64::from_bits(x as u64).sqrt().to_bits()),
            _ => sqrt_fin(kind, e, m),
        }),
    }
}

/// The square root of the positive finite `m * 2^(e - 127)` rounded to
/// `kind`: a digit-by-digit (restoring) integer square root of 125 bits,
/// at least 12 beyond binary128's 113, with the remainder as a sticky
/// bit, rounded once by [`pack`].
fn sqrt_fin(kind: FloatKind, e: i32, m: u128) -> u128 {
    // Result bits: the remainder stays below 2 * root < 2^126, so
    // `rem << 2` fits.
    const N: u32 = 125;
    // `unpack` leaves at least 15 low zero bits (no format has more than
    // 113 significand bits), so `x = mm * 2^ee` exactly, `mm < 2^115`,
    // with `ee` made even.
    let (mut mm, mut ee) = (m >> 14, e - 113);
    if ee & 1 != 0 {
        mm <<= 1;
        ee -= 1;
    }
    // sqrt(mm * 4^t) has exactly N bits.
    let t = N - (128 - mm.leading_zeros()).div_ceil(2);
    let (mut root, mut rem) = (0u128, 0u128);
    for j in (0..N).rev() {
        let pair = if j >= t { mm >> (2 * (j - t)) & 3 } else { 0 };
        rem = rem << 2 | pair;
        let trial = root << 2 | 1;
        root <<= 1;
        if rem >= trial {
            rem -= trial;
            root |= 1;
        }
    }
    // sqrt(x) = (root + f) * 2^(ee/2 - t), 0 <= f < 1, f = 0 iff rem = 0.
    pack(kind, false, ee / 2 - t as i32 + 127, root | u128::from(rem != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: FloatKind = FloatKind::Double;
    const S: FloatKind = FloatKind::Single;
    const H: FloatKind = FloatKind::Half;
    const B: FloatKind = FloatKind::BFloat16;
    const Q: FloatKind = FloatKind::Quad;
    const X: FloatKind = FloatKind::X87Extended;

    fn d(v: f64) -> u128 {
        u128::from(v.to_bits())
    }

    fn s(v: f32) -> u128 {
        u128::from(v.to_bits())
    }

    const VALS: [f64; 16] = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        2.5,
        -2.5,
        3.5,
        1.75,
        -0.3,
        1e300,
        -1e-300,
        123.456,
        4503599627370497.0,
        7.0,
        1e-310,
    ];

    // [c.math.nearbyint], [c.math.round], C23 7.12.9: native agreement.
    #[test]
    fn rounding_matches_native() {
        for &v in &VALS {
            assert_eq!(round_int(D, d(v), Rounding::Up), Some(d(v.ceil())), "ceil {v}");
            assert_eq!(round_int(D, d(v), Rounding::Down), Some(d(v.floor())), "floor {v}");
            assert_eq!(round_int(D, d(v), Rounding::Zero), Some(d(v.trunc())), "trunc {v}");
            assert_eq!(round_int(D, d(v), Rounding::Away), Some(d(v.round())), "round {v}");
            assert_eq!(
                round_int(D, d(v), Rounding::Even),
                Some(d(v.round_ties_even())),
                "rint {v}"
            );
            let w = v as f32;
            assert_eq!(round_int(S, s(w), Rounding::Up), Some(s(w.ceil())), "ceilf {w}");
        }
        assert_eq!(to_int(D, d(-2.5), Rounding::Away, 64), Some((-3i128) as u128));
        assert_eq!(to_int(D, d(2.5), Rounding::Even, 64), Some(2));
        assert_eq!(to_int(D, d(9.3e18), Rounding::Even, 64), None);
        assert_eq!(
            to_int(D, d(-9223372036854775808.0), Rounding::Even, 64),
            Some((i64::MIN as i128) as u128)
        );
    }

    // C23 7.12.10: fmod and remainder agree with the exact definitions.
    #[test]
    fn remainders() {
        for &a in &VALS {
            for &b in &VALS {
                let r = rem(D, d(a), d(b), false);
                if b == 0.0 || a.is_infinite() {
                    assert_eq!(r, None);
                    continue;
                }
                assert_eq!(r.map(|r| r.0), Some(d(a % b)), "fmod {a} {b}");
            }
        }
        assert_eq!(rem(D, d(5.0), d(3.0), true), Some((d(-1.0), 2)));
        assert_eq!(rem(D, d(7.5), d(2.0), true), Some((d(-0.5), 4)));
        assert_eq!(rem(D, d(-7.0), d(2.0), true), Some((d(1.0), -4)));
        assert_eq!(rem(D, d(1e300), d(3.0), false).map(|r| r.0), Some(d(1e300 % 3.0)));
        assert_eq!(rem(D, d(1.0), d(3.0), true), Some((d(1.0), 0)));
        assert_eq!(rem(D, d(2.0), d(3.0), true), Some((d(-1.0), 1)));
    }

    // C23 7.12.11.3: one ulp steps, signed zero, the binade edges.
    #[test]
    fn next() {
        let n = |k, x, y| nextafter(k, x, k, y);
        assert_eq!(n(D, d(1.0), d(2.0)), Some(d(1.0 + f64::EPSILON)));
        assert_eq!(n(D, d(1.0), d(0.0)), Some(d(1.0 - f64::EPSILON / 2.0)));
        assert_eq!(n(D, d(-1.0), d(0.0)), Some(d(-1.0 + f64::EPSILON / 2.0)));
        assert_eq!(n(D, d(f64::INFINITY), d(0.0)), Some(d(f64::MAX)));
        assert_eq!(n(D, d(f64::MAX), d(f64::INFINITY)), None);
        assert_eq!(n(D, d(0.0), d(1.0)), None);
        assert_eq!(n(D, d(0.0), d(-0.0)), Some(d(-0.0)));
        assert_eq!(n(D, d(f64::MIN_POSITIVE), d(0.0)), None);
        assert_eq!(
            n(D, d(f64::MIN_POSITIVE), d(1.0)),
            Some(d(f64::from_bits(0x0010_0000_0000_0001)))
        );
        // binary16 and bfloat16: 1 + 2^-10, 1 - 2^-11; 1 + 2^-7.
        assert_eq!(n(H, 0x3C00, 0x4000), Some(0x3C01));
        assert_eq!(n(H, 0x3C00, 0), Some(0x3BFF));
        assert_eq!(n(H, 0x7BFF, 0x7C00), None);
        assert_eq!(n(H, 0xFC00, 0), Some(0xFBFF));
        assert_eq!(n(B, 0x3F80, 0x4000), Some(0x3F81));
        assert_eq!(n(B, 0x0080, 0x4000), Some(0x0081));
        assert_eq!(n(B, 0x0080, 0), None);
        // x87: 1 -> 1 + 2^-63; binary128: 1 - 2^-113.
        assert_eq!(
            n(X, 0x3FFF_8000_0000_0000_0000, 0x4000_8000_0000_0000_0000),
            Some(0x3FFF_8000_0000_0000_0001)
        );
        assert_eq!(n(X, 0x3FFF_8000_0000_0000_0000, 0), Some(0x3FFE_FFFF_FFFF_FFFF_FFFF));
        assert_eq!(n(Q, 0x3FFF << 112, 0), Some(0x3FFE_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF));
        // nexttoward(1.0f, 1.0L + tiny) steps up.
        let one_x = 0x3FFF_8000_0000_0000_0000;
        assert_eq!(nextafter(S, s(1.0), X, one_x + 1), Some(s(1.0 + f32::EPSILON)));
        assert_eq!(nextafter(S, s(1.0), X, one_x), Some(s(1.0)));
    }

    // C23 7.12.13.1: fma rounds once.
    #[test]
    fn fused() {
        let e = f64::EPSILON;
        let (a, b, c) = (1.0 + e, 1.0 - e, -1.0);
        assert_eq!(fma(D, d(a), d(b), d(c)), Some(d(a.mul_add(b, c))));
        assert_eq!(fma(D, d(a), d(b), d(c)), Some(d(-e * e)));
        for &x in &VALS[..14] {
            for &y in &VALS[..14] {
                for &z in &[0.0, 1.0, -1e-300, 3.25, -7.0] {
                    let want = x.mul_add(y, z);
                    if want.is_infinite() {
                        continue;
                    }
                    let got = fma(D, d(x), d(y), d(z));
                    if let Some(g) = got {
                        assert_eq!(g, d(want), "fma {x} {y} {z}");
                    }
                }
            }
        }
        assert_eq!(fma(D, d(1e300), d(1e300), d(0.0)), None);
        assert_eq!(fma(D, d(f64::INFINITY), d(0.0), d(1.0)), None);
        assert_eq!(fma(D, d(2.0), d(3.0), d(-6.0)), Some(d(0.0)));
        assert_eq!(fma(D, d(-0.0), d(1.0), d(-0.0)), Some(d(-0.0)));
    }

    // C23 7.12.6: frexp, ldexp, ilogb, logb, modf.
    #[test]
    fn exponents() {
        assert_eq!(frexp(D, d(8.0)), Some((d(0.5), 4)));
        assert_eq!(frexp(D, d(-0.75)), Some((d(-0.75), 0)));
        assert_eq!(frexp(D, d(1e-310)).map(|r| r.1), Some(-1029));
        assert_eq!(ldexp(D, d(1.0), 1023), Some(d(f64::from_bits(0x7FE0_0000_0000_0000))));
        assert_eq!(ldexp(D, d(1.0), 1024), None);
        assert_eq!(ldexp(D, d(1.0), -1074), Some(d(5e-324)));
        assert_eq!(ldexp(D, d(1.5), -1074), None);
        assert_eq!(ilogb(D, d(0.0)), None);
        assert_eq!(ilogb(D, d(10.0)), Some(3));
        assert_eq!(logb(D, d(10.0)), Some(d(3.0)));
        assert_eq!(logb(D, d(f64::NEG_INFINITY)), Some(d(f64::INFINITY)));
        assert_eq!(modf(D, d(-3.75)), Some((d(-0.75), d(-3.0))));
        assert_eq!(modf(D, d(-4.0)), Some((d(-0.0), d(-4.0))));
        assert_eq!(fdim(D, d(1.0), d(3.0)), Some(d(0.0)));
        assert_eq!(fdim(D, d(5.0), d(3.0)), Some(d(2.0)));
        assert_eq!(fdim(D, d(f64::MAX), d(-f64::MAX)), None);
    }

    /// IEEE 754-2019 §5.4.1, C23 F.10.4.5: the integer square root agrees
    /// with the host's correctly rounded binary32/binary64 `sqrt` (normal
    /// and subnormal operands, both exponent parities), and is correctly
    /// rounded for x87 and binary128 `sqrt(2)`.
    #[test]
    fn square_root() {
        let mut bits = 0x3FF0_0000_0000_0001u64;
        let mut vals = vec![2.0, 3.0, 0.5, 1e-310, 5e-324, 1.7976931348623157e308, 0.1, 1e22];
        for _ in 0..4000 {
            // xorshift over positive finite doubles.
            bits ^= bits << 13;
            bits ^= bits >> 7;
            bits ^= bits << 17;
            let v = f64::from_bits(bits & 0x7FEF_FFFF_FFFF_FFFF);
            vals.push(v);
        }
        for &v in &vals {
            let Val::Fin(false, e, m) = unpack(D, d(v)) else { continue };
            assert_eq!(sqrt_fin(D, e, m), d(v.sqrt()), "sqrt({v:e})");
            let f = v as f32;
            if let Val::Fin(false, e, m) = unpack(S, s(f)) {
                assert_eq!(sqrt_fin(S, e, m), s(f.sqrt()), "sqrtf({f:e})");
            }
        }
        assert_eq!(sqrt(D, d(2.0)), Some(d(std::f64::consts::SQRT_2)));
        assert_eq!(sqrt(D, d(-0.0)), Some(d(-0.0)));
        assert_eq!(sqrt(D, d(f64::INFINITY)), Some(d(f64::INFINITY)));
        assert_eq!(sqrt(D, d(-1.0)), None);
        assert_eq!(sqrt(D, d(f64::NEG_INFINITY)), None);
        assert_eq!(sqrt(D, 0x7FF0_0000_0000_0001), None);
        assert!(is_nan_bits(sqrt(D, d(f64::NAN)).unwrap()));
        // 2.0 in binary128, x87 extended, binary16 and bfloat16.
        let (q2, x2) = (0x4000u128 << 112, 0x4000_8000_0000_0000_0000u128);
        assert_eq!(sqrt(Q, q2), Some(0x3FFF_6A09_E667_F3BC_C908_B2FB_1366_EA95));
        assert_eq!(sqrt(X, x2), Some(0x3FFF_B504_F333_F9DE_6484));
        // sqrt(2) = 1.0110101000001001111...: rounds down in binary16.
        assert_eq!(sqrt(H, 0x4000), Some(0x3DA8));
        assert_eq!(sqrt(B, 0x4000), Some(0x3FB5));
        // Exact squares and the smallest subnormals: 2^-16494 -> 2^-8247,
        // 2^-16445 -> sqrt(2) * 2^-8223.
        assert_eq!(sqrt(Q, 0x4001u128 << 112), Some(q2));
        assert_eq!(sqrt(Q, 1), Some(0x1FC8u128 << 112));
        assert_eq!(sqrt(X, 1), Some(0x1FE0_B504_F333_F9DE_6484));
        assert_eq!(sqrt(X, 0x7FFF_8000_0000_0000_0001), None);
    }

    fn is_nan_bits(a: u128) -> bool {
        matches!(unpack(D, a), Val::NaN(..))
    }
}
