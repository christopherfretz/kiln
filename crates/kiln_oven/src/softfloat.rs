//! Software IEEE arithmetic for the formats Rust has no native type for
//! (binary16, bfloat16, x87 80-bit extended, binary128): unpack to a
//! 128-bit normalized significand, operate exactly enough (guard bits and
//! a sticky bit), round to nearest even on packing.

use std::cmp::Ordering;

use kiln_ast::consts::FloatKind;

use crate::kbc::Op;

#[derive(Clone, Copy)]
struct Fmt {
    /// Precision (significand bits including the leading one).
    p: u32,
    ebits: u32,
    /// x87: the integer bit is stored.
    explicit: bool,
}

impl Fmt {
    fn of(kind: FloatKind) -> Fmt {
        match kind {
            FloatKind::Half => Fmt { p: 11, ebits: 5, explicit: false },
            FloatKind::BFloat16 => Fmt { p: 8, ebits: 8, explicit: false },
            FloatKind::Single => Fmt { p: 24, ebits: 8, explicit: false },
            FloatKind::Double => Fmt { p: 53, ebits: 11, explicit: false },
            FloatKind::X87Extended => Fmt { p: 64, ebits: 15, explicit: true },
            FloatKind::Quad => Fmt { p: 113, ebits: 15, explicit: false },
        }
    }

    fn bias(self) -> i32 {
        (1 << (self.ebits - 1)) - 1
    }

    fn emin(self) -> i32 {
        1 - self.bias()
    }

    /// Bits of the stored significand field.
    fn mfield(self) -> u32 {
        if self.explicit { self.p } else { self.p - 1 }
    }

    fn total(self) -> u32 {
        1 + self.ebits + self.mfield()
    }

    fn emask(self) -> u128 {
        (1 << self.ebits) - 1
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Val {
    Zero(bool),
    Inf(bool),
    NaN,
    /// `m * 2^(e - 127)`, `m`'s top bit set.
    Fin(bool, i32, u128),
}

fn unpack(kind: FloatKind, a: u128) -> Val {
    let f = Fmt::of(kind);
    let sign = a >> (f.total() - 1) & 1 == 1;
    let ef = (a >> f.mfield()) & f.emask();
    let field = a & ((1u128 << f.mfield()) - 1);
    let frac = if f.explicit { field & ((1u128 << (f.p - 1)) - 1) } else { field };
    if ef == f.emask() {
        return if frac == 0 { Val::Inf(sign) } else { Val::NaN };
    }
    let (int, e0) = if ef == 0 {
        if field == 0 {
            return Val::Zero(sign);
        }
        (field, f.emin() - (f.p as i32 - 1))
    } else {
        let int = if f.explicit { field } else { field | 1 << (f.p - 1) };
        if int == 0 {
            return Val::Zero(sign);
        }
        (int, ef as i32 - f.bias() - (f.p as i32 - 1))
    };
    let lz = int.leading_zeros();
    Val::Fin(sign, e0 - lz as i32 + 127, int << lz)
}

fn pack_inf(f: Fmt, sign: bool) -> u128 {
    let s = u128::from(sign) << (f.total() - 1);
    let int = if f.explicit { 1 << (f.p - 1) } else { 0 };
    s | f.emask() << f.mfield() | int
}

fn pack_nan(f: Fmt) -> u128 {
    let q = 1u128 << (f.p - 2);
    let int = if f.explicit { 1 << (f.p - 1) } else { 0 };
    f.emask() << f.mfield() | int | q
}

fn pack_zero(f: Fmt, sign: bool) -> u128 {
    u128::from(sign) << (f.total() - 1)
}

fn shr_sticky(x: u128, d: u32) -> u128 {
    if d == 0 {
        x
    } else if d >= 128 {
        u128::from(x != 0)
    } else {
        x >> d | u128::from(x & ((1u128 << d) - 1) != 0)
    }
}

/// Rounds `m * 2^(e - 127)` to the format.
fn pack(kind: FloatKind, sign: bool, mut e: i32, mut m: u128) -> u128 {
    let f = Fmt::of(kind);
    if m == 0 {
        return pack_zero(f, sign);
    }
    let lz = m.leading_zeros();
    m <<= lz;
    e -= lz as i32;
    let mut shift = 128 - f.p;
    if e < f.emin() {
        shift += (f.emin() - e).min(200) as u32;
        e = f.emin();
    }
    let (mut kept, rem, half) = if shift >= 129 {
        (0u128, u128::from(m != 0), u128::MAX)
    } else if shift == 128 {
        (0u128, m, 1u128 << 127)
    } else {
        (m >> shift, m & ((1u128 << shift) - 1), 1u128 << (shift - 1))
    };
    if rem > half || (rem == half && kept & 1 == 1) {
        kept += 1;
        if kept >> f.p != 0 {
            kept >>= 1;
            e += 1;
        }
    }
    if e > f.bias() {
        return pack_inf(f, sign);
    }
    if kept == 0 {
        return pack_zero(f, sign);
    }
    let normal = kept >> (f.p - 1) != 0;
    let biased = if normal { (e + f.bias()) as u128 } else { 0 };
    let field = if f.explicit { kept } else { kept & ((1u128 << (f.p - 1)) - 1) };
    u128::from(sign) << (f.total() - 1) | biased << f.mfield() | field
}

fn repack(kind: FloatKind, v: Val) -> u128 {
    let f = Fmt::of(kind);
    match v {
        Val::Zero(s) => pack_zero(f, s),
        Val::Inf(s) => pack_inf(f, s),
        Val::NaN => pack_nan(f),
        Val::Fin(s, e, m) => pack(kind, s, e, m),
    }
}

pub(crate) fn is_nan(kind: FloatKind, a: u128) -> bool {
    unpack(kind, a) == Val::NaN
}

pub(crate) fn to_f64(kind: FloatKind, a: u128) -> f64 {
    f64::from_bits(repack(FloatKind::Double, unpack(kind, a)) as u64)
}

pub(crate) fn from_f64(kind: FloatKind, v: f64) -> u128 {
    repack(kind, unpack(FloatKind::Double, u128::from(v.to_bits())))
}

pub(crate) fn cast(from: FloatKind, to: FloatKind, a: u128) -> Option<u128> {
    Some(repack(to, unpack(from, a)))
}

pub(crate) fn cmp(kind: FloatKind, a: u128, b: u128) -> Option<Ordering> {
    let key = |v: Val| -> Option<(i8, i32, u128)> {
        Some(match v {
            Val::NaN => return None,
            Val::Zero(_) => (0, 0, 0),
            Val::Inf(s) => (if s { -2 } else { 2 }, 0, 0),
            Val::Fin(s, e, m) => (if s { -1 } else { 1 }, e, m),
        })
    };
    let (x, y) = (key(unpack(kind, a))?, key(unpack(kind, b))?);
    Some(match x.0.cmp(&y.0) {
        Ordering::Equal if x.0 == 1 => (x.1, x.2).cmp(&(y.1, y.2)),
        Ordering::Equal if x.0 == -1 => (y.1, y.2).cmp(&(x.1, x.2)),
        o => o,
    })
}

pub(crate) fn from_int(kind: FloatKind, v: u128, signed: bool) -> Option<u128> {
    let neg = signed && (v as i128) < 0;
    let mag = if neg { (v as i128).unsigned_abs() } else { v };
    if mag == 0 {
        return Some(pack_zero(Fmt::of(kind), false));
    }
    Some(pack(kind, neg, 127, mag))
}

/// Truncation toward zero: `(negative, magnitude)`; `None` for
/// non-finite values or magnitudes of 2^128 and more.
pub(crate) fn trunc(kind: FloatKind, a: u128) -> Option<(bool, u128)> {
    match unpack(kind, a) {
        Val::Zero(s) => Some((s, 0)),
        Val::Fin(s, e, m) => {
            if e < 0 {
                Some((s, 0))
            } else if e >= 128 {
                None
            } else {
                Some((s, m >> (127 - e)))
            }
        }
        _ => None,
    }
}

fn add(kind: FloatKind, x: Val, y: Val) -> Val {
    match (x, y) {
        (Val::NaN, _) | (_, Val::NaN) => Val::NaN,
        (Val::Inf(a), Val::Inf(b)) => {
            if a == b {
                Val::Inf(a)
            } else {
                Val::NaN
            }
        }
        (Val::Inf(a), _) | (_, Val::Inf(a)) => Val::Inf(a),
        (Val::Zero(a), Val::Zero(b)) => Val::Zero(a && b),
        (Val::Zero(_), v) | (v, Val::Zero(_)) => v,
        (Val::Fin(sa, ea, ma), Val::Fin(sb, eb, mb)) => {
            let ((sa, ea, ma), (sb, eb, mb)) =
                if (ea, ma) >= (eb, mb) { ((sa, ea, ma), (sb, eb, mb)) } else { ((sb, eb, mb), (sa, ea, ma)) };
            let a = ma >> 1;
            let b = shr_sticky(mb >> 1 | (mb & 1), (ea - eb).min(200) as u32);
            let (s, m) = if sa == sb { (sa, a + b) } else { (sa, a - b) };
            if m == 0 {
                return Val::Zero(false);
            }
            let lz = m.leading_zeros();
            let _ = kind;
            Val::Fin(s, ea + 1 - lz as i32, m << lz)
        }
    }
}

fn mul(kind: FloatKind, x: Val, y: Val) -> Val {
    let p = Fmt::of(kind).p;
    match (x, y) {
        (Val::NaN, _) | (_, Val::NaN) => Val::NaN,
        (Val::Inf(_), Val::Zero(_)) | (Val::Zero(_), Val::Inf(_)) => Val::NaN,
        (Val::Inf(a), Val::Inf(b) | Val::Fin(b, ..) | Val::Zero(b))
        | (Val::Fin(a, ..) | Val::Zero(a), Val::Inf(b)) => Val::Inf(a != b),
        (Val::Zero(a), Val::Zero(b) | Val::Fin(b, ..)) | (Val::Fin(a, ..), Val::Zero(b)) => Val::Zero(a != b),
        (Val::Fin(sa, ea, ma), Val::Fin(sb, eb, mb)) => {
            let (a, b) = (ma >> (128 - p), mb >> (128 - p));
            // 256-bit product.
            let (al, ah) = (a as u64 as u128, a >> 64);
            let (bl, bh) = (b as u64 as u128, b >> 64);
            let ll = al * bl;
            let mid = al * bh + ah * bl;
            let hh = ah * bh;
            let lo = ll.wrapping_add(mid << 64);
            let carry = u128::from(lo < ll);
            let hi = hh + (mid >> 64) + carry;
            let (t, m) = if hi != 0 {
                let lz = hi.leading_zeros();
                let m = if lz == 0 { hi | u128::from(lo != 0) } else { hi << lz | lo >> (128 - lz) | u128::from(lo << lz != 0) };
                (255 - lz as i32, m)
            } else {
                let lz = lo.leading_zeros();
                (127 - lz as i32, lo << lz)
            };
            Val::Fin(sa != sb, ea + eb - 2 * (p as i32 - 1) + t, m)
        }
    }
}

fn div(kind: FloatKind, x: Val, y: Val) -> Val {
    let p = Fmt::of(kind).p;
    match (x, y) {
        (Val::NaN, _) | (_, Val::NaN) | (Val::Inf(_), Val::Inf(_)) | (Val::Zero(_), Val::Zero(_)) => Val::NaN,
        (Val::Inf(a), Val::Fin(b, ..) | Val::Zero(b)) => Val::Inf(a != b),
        (Val::Fin(a, ..) | Val::Zero(a), Val::Inf(b)) => Val::Zero(a != b),
        (Val::Zero(a), Val::Fin(b, ..)) => Val::Zero(a != b),
        (Val::Fin(a, ..), Val::Zero(b)) => Val::Inf(a != b),
        (Val::Fin(sa, ea, ma), Val::Fin(sb, eb, mb)) => {
            let (n, d) = (ma >> (128 - p), mb >> (128 - p));
            let (mut r, mut q) = (n, 0u128);
            for _ in 0..127 {
                q <<= 1;
                if r >= d {
                    r -= d;
                    q |= 1;
                }
                r <<= 1;
            }
            let m = q | u128::from(r != 0);
            let lz = m.leading_zeros();
            Val::Fin(sa != sb, ea - eb + 1 - lz as i32, m << lz)
        }
    }
}

/// `a op b` in `kind`.
pub(crate) fn bin(op: Op, kind: FloatKind, a: u128, b: u128) -> Option<u128> {
    let (x, y) = (unpack(kind, a), unpack(kind, b));
    let v = match op {
        Op::FAdd => add(kind, x, y),
        Op::FSub => add(
            kind,
            x,
            match y {
                Val::Zero(s) => Val::Zero(!s),
                Val::Inf(s) => Val::Inf(!s),
                Val::Fin(s, e, m) => Val::Fin(!s, e, m),
                Val::NaN => Val::NaN,
            },
        ),
        Op::FMul => mul(kind, x, y),
        Op::FDiv => div(kind, x, y),
        _ => return None,
    };
    Some(repack(kind, v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(v: f64) -> u128 {
        u128::from(v.to_bits())
    }

    // [basic.fundamental]/12: the generic path agrees with native binary64.
    #[test]
    fn generic_matches_native_double() {
        let vals = [1.0, -2.5, 3.141_592_653_589_793, 1e-310, 1e300, 0.1, -0.0, 7.0e-20, 123_456_789.0];
        for &a in &vals {
            for &b in &vals {
                for (op, r) in [(Op::FAdd, a + b), (Op::FSub, a - b), (Op::FMul, a * b), (Op::FDiv, a / b)] {
                    let got = bin(op, FloatKind::Double, d(a), d(b)).unwrap();
                    let want = d(r);
                    if r.is_nan() {
                        assert!(is_nan(FloatKind::Double, got));
                    } else {
                        assert_eq!(got, want, "{op:?} {a} {b}");
                    }
                }
            }
        }
        assert_eq!(from_int(FloatKind::Double, (-5i128) as u128, true), Some(d(-5.0)));
        assert_eq!(cast(FloatKind::Double, FloatKind::Single, d(0.1)), Some(u128::from(0.1f32.to_bits())));
    }

    // x87 extended: 1/3 has a 64-bit significand.
    #[test]
    fn x87_basics() {
        let one = from_int(FloatKind::X87Extended, 1, true).unwrap();
        let three = from_int(FloatKind::X87Extended, 3, true).unwrap();
        assert_eq!(one, 0x3FFF_8000_0000_0000_0000);
        let third = bin(Op::FDiv, FloatKind::X87Extended, one, three).unwrap();
        assert_eq!(third, 0x3FFD_AAAA_AAAA_AAAA_AAAB);
        assert_eq!(to_f64(FloatKind::X87Extended, third), 1.0 / 3.0);
    }
}
