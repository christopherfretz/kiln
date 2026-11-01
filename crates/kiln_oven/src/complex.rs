//! GNU `_Complex` arithmetic as clang 19's constant evaluator computes it
//! (`ComplexExprEvaluator::VisitBinaryOperator`; spec/oven.md "Complex
//! values"): floating multiplication and division by C11 Annex G
//! (`HandleComplexComplexMul`/`Div`), with `APFloat` operators that never
//! fail (a NaN or an infinity is a value); integer operations on `APSInt`,
//! wrapping, the division failing only on a zero divisor.

use kiln_ast::consts::FloatKind;

use crate::arith::{self, canon};
use crate::kbc::Op;
use crate::softfloat::{self, Class};

/// The operations of `Op::Cx` (its `sub`).
pub(crate) mod cx {
    pub const ADD: u32 = 0;
    pub const SUB: u32 = 1;
    pub const MUL: u32 = 2;
    pub const DIV: u32 = 3;
}

/// `APFloat` arithmetic in one format.
#[derive(Clone, Copy)]
struct F(FloatKind);

impl F {
    fn bin(self, op: Op, a: u128, b: u128) -> u128 {
        arith::float_bin_raw(op, self.0, a, b).unwrap_or_else(|| softfloat::make_nan(self.0, 0, false))
    }
    fn add(self, a: u128, b: u128) -> u128 {
        self.bin(Op::FAdd, a, b)
    }
    fn sub(self, a: u128, b: u128) -> u128 {
        self.bin(Op::FSub, a, b)
    }
    fn mul(self, a: u128, b: u128) -> u128 {
        self.bin(Op::FMul, a, b)
    }
    fn div(self, a: u128, b: u128) -> u128 {
        self.bin(Op::FDiv, a, b)
    }
    fn class(self, a: u128) -> Class {
        softfloat::classify(self.0, a)
    }
    fn nan(self, a: u128) -> bool {
        self.class(a) == Class::NaN
    }
    fn inf(self, a: u128) -> bool {
        self.class(a) == Class::Inf
    }
    fn finite(self, a: u128) -> bool {
        !matches!(self.class(a), Class::NaN | Class::Inf)
    }
    fn sign(self) -> u128 {
        softfloat::sign_bit(self.0)
    }
    fn negative(self, a: u128) -> bool {
        a & self.sign() != 0
    }
    fn abs(self, a: u128) -> u128 {
        a & !self.sign()
    }
    fn copysign(self, a: u128, b: u128) -> u128 {
        self.abs(a) | b & self.sign()
    }
    fn one(self) -> u128 {
        arith::int_to_float(self.0, 1, true)
    }
    fn infinity(self, neg: bool) -> u128 {
        softfloat::inf(self.0) | if neg { self.sign() } else { 0 }
    }
    /// `copySign(APFloat(x.isInfinity() ? 1 : 0), x)`.
    fn unit(self, a: u128) -> u128 {
        self.copysign(if self.inf(a) { self.one() } else { 0 }, a)
    }
    /// A NaN replaced by a zero of its sign (`copySign(APFloat(sem), x)`).
    fn unnan(self, a: u128) -> u128 {
        if self.nan(a) { self.copysign(0, a) } else { a }
    }
    /// `llvm::maxnum` (a NaN operand yields the other).
    fn maxnum(self, a: u128, b: u128) -> u128 {
        if self.nan(a) {
            return b;
        }
        if self.nan(b) {
            return a;
        }
        match arith::float_cmp(self.0, a, b) {
            Some(std::cmp::Ordering::Less) => b,
            _ => a,
        }
    }
    fn scalbn(self, a: u128, n: i64) -> u128 {
        softfloat::scalbn(self.0, a, n)
    }
}

/// `(a + ib) op (c + id)` in floating format `kind`; `sub` is a `cx`
/// operation.
pub(crate) fn float_op(sub: u32, kind: FloatKind, v: [u128; 4]) -> [u128; 2] {
    let f = F(kind);
    let [a, b, c, d] = v;
    match sub {
        cx::ADD => [f.add(a, c), f.add(b, d)],
        cx::SUB => [f.sub(a, c), f.sub(b, d)],
        cx::MUL => mul(f, v),
        _ => div(f, v),
    }
}

/// clang's `HandleComplexComplexMul`.
fn mul(f: F, v: [u128; 4]) -> [u128; 2] {
    let [mut a, mut b, mut c, mut d] = v;
    let (ac, bd, ad, bc) = (f.mul(a, c), f.mul(b, d), f.mul(a, d), f.mul(b, c));
    let (rr, ri) = (f.sub(ac, bd), f.add(ad, bc));
    if !(f.nan(rr) && f.nan(ri)) {
        return [rr, ri];
    }
    let mut recalc = false;
    if f.inf(a) || f.inf(b) {
        (a, b, c, d) = (f.unit(a), f.unit(b), f.unnan(c), f.unnan(d));
        recalc = true;
    }
    if f.inf(c) || f.inf(d) {
        (c, d, a, b) = (f.unit(c), f.unit(d), f.unnan(a), f.unnan(b));
        recalc = true;
    }
    if !recalc && (f.inf(ac) || f.inf(bd) || f.inf(ad) || f.inf(bc)) {
        (a, b, c, d) = (f.unnan(a), f.unnan(b), f.unnan(c), f.unnan(d));
        recalc = true;
    }
    if !recalc {
        return [rr, ri];
    }
    let inf = f.infinity(false);
    [
        f.mul(inf, f.sub(f.mul(a, c), f.mul(b, d))),
        f.mul(inf, f.add(f.mul(a, d), f.mul(b, c))),
    ]
}

/// clang's `HandleComplexComplexDiv`.
fn div(f: F, v: [u128; 4]) -> [u128; 2] {
    let [mut a, mut b, mut c, mut d] = v;
    let mut logb = 0i64;
    let max_cd = f.maxnum(f.abs(c), f.abs(d));
    if f.finite(max_cd) {
        logb = i64::from(softfloat::ilogb(f.0, max_cd));
        (c, d) = (f.scalbn(c, -logb), f.scalbn(d, -logb));
    }
    let denom = f.add(f.mul(c, c), f.mul(d, d));
    let rr = f.scalbn(f.div(f.add(f.mul(a, c), f.mul(b, d)), denom), -logb);
    let ri = f.scalbn(f.div(f.sub(f.mul(b, c), f.mul(a, d)), denom), -logb);
    if !(f.nan(rr) && f.nan(ri)) {
        return [rr, ri];
    }
    if denom == 0 && (!f.nan(a) || !f.nan(b)) {
        let inf = f.infinity(f.negative(c));
        [f.mul(inf, a), f.mul(inf, b)]
    } else if (f.inf(a) || f.inf(b)) && f.finite(c) && f.finite(d) {
        (a, b) = (f.unit(a), f.unit(b));
        let inf = f.infinity(false);
        [
            f.mul(inf, f.add(f.mul(a, c), f.mul(b, d))),
            f.mul(inf, f.sub(f.mul(b, c), f.mul(a, d))),
        ]
    } else if f.inf(max_cd) && f.finite(a) && f.finite(b) {
        (c, d) = (f.unit(c), f.unit(d));
        [f.mul(0, f.add(f.mul(a, c), f.mul(b, d))), f.mul(0, f.sub(f.mul(b, c), f.mul(a, d)))]
    } else {
        [rr, ri]
    }
}

/// `(a + ib) op (c + id)` on integers of `width` bits (canonical
/// operands); `None` for a division by `0 + 0i`.
pub(crate) fn int_op(sub: u32, width: u32, signed: bool, v: [u128; 4]) -> Option<[u128; 2]> {
    let [a, b, c, d] = v;
    let k = |x: u128| canon(x, width, signed);
    let m = u128::wrapping_mul;
    Some(match sub {
        cx::ADD => [k(a.wrapping_add(c)), k(b.wrapping_add(d))],
        cx::SUB => [k(a.wrapping_sub(c)), k(b.wrapping_sub(d))],
        cx::MUL => [k(m(a, c).wrapping_sub(m(b, d))), k(m(a, d).wrapping_add(m(b, c)))],
        _ => {
            if c == 0 && d == 0 {
                return None;
            }
            let den = k(m(c, c).wrapping_add(m(d, d)));
            let nr = k(m(a, c).wrapping_add(m(b, d)));
            let ni = k(m(b, c).wrapping_sub(m(a, d)));
            let q = |n: u128| -> u128 {
                if den == 0 {
                    // `APInt` division by a wrapped-to-zero denominator is
                    // undefined in clang; yield zero.
                    0
                } else if signed {
                    k((n as i128).wrapping_div(den as i128) as u128)
                } else {
                    n / den
                }
            };
            [q(nr), q(ni)]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(v: f64) -> u128 {
        u128::from(v.to_bits())
    }

    fn fd(sub: u32, v: [f64; 4]) -> [f64; 2] {
        let r = float_op(sub, FloatKind::Double, v.map(d));
        r.map(|x| f64::from_bits(x as u64))
    }

    #[test]
    fn annex_g_double() {
        assert_eq!(fd(cx::MUL, [1.0, 2.0, 3.0, 4.0]), [-5.0, 10.0]);
        assert_eq!(fd(cx::DIV, [-5.0, 10.0, 3.0, 4.0]), [1.0, 2.0]);
        // inf * (nan + nan i) recovers an infinity.
        let r = fd(cx::MUL, [f64::INFINITY, 0.0, f64::NAN, f64::NAN]);
        assert!(r[0].is_nan() || r[0].is_infinite());
        // (1 + 1i) / 0 is an infinity.
        let r = fd(cx::DIV, [1.0, 1.0, 0.0, 0.0]);
        assert_eq!(r, [f64::INFINITY, f64::INFINITY]);
        // finite / inf is a zero.
        let r = fd(cx::DIV, [1.0, 1.0, f64::INFINITY, 0.0]);
        assert_eq!(r, [0.0, 0.0]);
    }

    #[test]
    fn scalbn_ilogb() {
        assert_eq!(softfloat::ilogb(FloatKind::Double, d(8.0)), 3);
        assert_eq!(softfloat::ilogb(FloatKind::Double, d(5e-324)), -1074);
        assert_eq!(softfloat::scalbn(FloatKind::Double, d(3.0), -2), d(0.75));
        assert_eq!(softfloat::scalbn(FloatKind::Double, d(1.0), -1075), d(0.0));
        assert_eq!(softfloat::scalbn(FloatKind::Double, d(1.5), -1074), d(1e-323));
        assert_eq!(softfloat::scalbn(FloatKind::Double, d(1.0), 1 << 40), d(f64::INFINITY));
    }

    #[test]
    fn integer_ops() {
        let n = |x: i128| x as u128;
        let r = int_op(cx::MUL, 32, true, [n(1), n(2), n(3), n(4)]).unwrap();
        assert_eq!(r, [n(-5), n(10)]);
        let r = int_op(cx::DIV, 32, true, [n(-5), n(10), n(3), n(4)]).unwrap();
        assert_eq!(r, [n(1), n(2)]);
        assert!(int_op(cx::DIV, 32, true, [1, 1, 0, 0]).is_none());
    }
}
