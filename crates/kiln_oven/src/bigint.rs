//! Fixed-width two's-complement integers over little-endian `u64` limbs:
//! the arithmetic of `_BitInt(N)` with `N > 128` (spec/oven.md
//! "`_BitInt`"), whose values live in memory (`TyKind::WideInt`).
//!
//! A value of width `w` held in `n` limbs (`n * 64 >= w`, the object's
//! size in words) is **canonical** when every bit at or above `w` equals
//! bit `w - 1` (signed) or zero (unsigned), the wide form of
//! `arith::canon`. Every function here takes canonical operands of equal
//! length; the checked operations of `vm/wide.rs` compute exact results
//! in a wider buffer and test them against the width.

use std::cmp::Ordering;

/// Sign- or zero-extends `w` from bit `width` in place.
pub(crate) fn canon(w: &mut [u64], width: u32, signed: bool) {
    let top = (width / 64) as usize;
    if top >= w.len() {
        return;
    }
    let neg = signed && width > 0 && bit(w, width - 1);
    let fill = if neg { u64::MAX } else { 0 };
    let rem = width % 64;
    let from = if rem == 0 {
        top
    } else {
        let mask = (1u64 << rem) - 1;
        w[top] = (w[top] & mask) | (fill & !mask);
        top + 1
    };
    w[from..].fill(fill);
}

fn bit(w: &[u64], i: u32) -> bool {
    w[(i / 64) as usize] >> (i % 64) & 1 != 0
}

/// Whether canonical `w` is negative as a signed value (its top bit).
pub(crate) fn is_neg(w: &[u64]) -> bool {
    w.last().is_some_and(|&x| (x as i64) < 0)
}

/// `w` sign- (`signed`) or zero-extended to `n >= w.len()` limbs.
pub(crate) fn extend(w: &[u64], n: usize, signed: bool) -> Vec<u64> {
    let fill = if signed && is_neg(w) { u64::MAX } else { 0 };
    let mut v = w.to_vec();
    v.resize(n, fill);
    v
}

/// Whether the exact value `w` (signed two's complement in its own
/// limbs) is representable in `width` bits of signedness `signed`.
pub(crate) fn fits(w: &[u64], width: u32, signed: bool) -> bool {
    if !signed && is_neg(w) {
        return false;
    }
    let mut c = w.to_vec();
    canon(&mut c, width, signed);
    c == w
}

/// `n` limbs holding the register value `v` (canonical for its own
/// width and signedness `signed`).
pub(crate) fn from_u128(v: u128, signed: bool, n: usize) -> Vec<u64> {
    let fill = if signed && (v as i128) < 0 { u64::MAX } else { 0 };
    let mut w = vec![fill; n.max(2)];
    w[0] = v as u64;
    w[1] = (v >> 64) as u64;
    w.truncate(n.max(1));
    w
}

/// The low 128 bits of `w`.
pub(crate) fn low_u128(w: &[u64]) -> u128 {
    let lo = u128::from(w.first().copied().unwrap_or(0));
    let hi = u128::from(w.get(1).copied().unwrap_or(0));
    lo | hi << 64
}

pub(crate) fn is_zero(w: &[u64]) -> bool {
    w.iter().all(|&x| x == 0)
}

/// `a + b`, wrapping.
pub(crate) fn add(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut c = 0u64;
    a.iter()
        .zip(b)
        .map(|(&x, &y)| {
            let (s, c1) = x.overflowing_add(y);
            let (s, c2) = s.overflowing_add(c);
            c = u64::from(c1 | c2);
            s
        })
        .collect()
}

/// `a - b`, wrapping.
pub(crate) fn sub(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut br = 0u64;
    a.iter()
        .zip(b)
        .map(|(&x, &y)| {
            let (d, b1) = x.overflowing_sub(y);
            let (d, b2) = d.overflowing_sub(br);
            br = u64::from(b1 | b2);
            d
        })
        .collect()
}

/// `-a`, wrapping.
pub(crate) fn neg(a: &[u64]) -> Vec<u64> {
    sub(&vec![0; a.len()], a)
}

/// `a * b` modulo `2^(64 * a.len())`.
pub(crate) fn mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    let n = a.len();
    let mut r = vec![0u64; n];
    for i in 0..n {
        if a[i] == 0 {
            continue;
        }
        let mut c = 0u128;
        for j in 0..n - i {
            let t = u128::from(a[i]) * u128::from(b[j]) + u128::from(r[i + j]) + c;
            r[i + j] = t as u64;
            c = t >> 64;
        }
    }
    r
}

/// Compares canonical `a` and `b`.
pub(crate) fn cmp(a: &[u64], b: &[u64], signed: bool) -> Ordering {
    if signed {
        match (is_neg(a), is_neg(b)) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
    }
    for i in (0..a.len()).rev() {
        match a[i].cmp(&b[i]) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    Ordering::Equal
}

/// Unsigned `a / b` and `a % b` (`b` nonzero), by shift and subtract.
pub(crate) fn divrem_u(a: &[u64], b: &[u64]) -> (Vec<u64>, Vec<u64>) {
    let n = a.len();
    let mut q = vec![0u64; n];
    let mut r = vec![0u64; n];
    let bits = n as u32 * 64;
    let top = (0..bits).rev().find(|&i| bit(a, i)).map_or(0, |i| i + 1);
    for i in (0..top).rev() {
        // r = r << 1 | bit i of a
        let mut carry = u64::from(bit(a, i));
        for x in &mut r {
            let nc = *x >> 63;
            *x = *x << 1 | carry;
            carry = nc;
        }
        if carry != 0 || cmp(&r, b, false) != Ordering::Less {
            r = sub(&r, b);
            q[(i / 64) as usize] |= 1 << (i % 64);
        }
    }
    (q, r)
}

/// `a << c` (`c` below the limbs' bit count), wrapping.
pub(crate) fn shl(a: &[u64], c: u32) -> Vec<u64> {
    let n = a.len();
    let (ws, bs) = ((c / 64) as usize, c % 64);
    let mut r = vec![0u64; n];
    for i in ws..n {
        let lo = a[i - ws];
        r[i] = lo << bs;
        if bs != 0 && i > ws {
            r[i] |= a[i - ws - 1] >> (64 - bs);
        }
    }
    r
}

/// `a >> c`: arithmetic when `signed`, else logical.
pub(crate) fn shr(a: &[u64], c: u32, signed: bool) -> Vec<u64> {
    let n = a.len();
    let fill = if signed && is_neg(a) { u64::MAX } else { 0 };
    let (ws, bs) = ((c / 64) as usize, c % 64);
    let at = |i: usize| a.get(i).copied().unwrap_or(fill);
    (0..n)
        .map(|i| {
            let lo = at(i + ws);
            if bs == 0 { lo } else { lo >> bs | at(i + ws + 1) << (64 - bs) }
        })
        .collect()
}

/// The decimal spelling of `w` (signed two's complement when `signed`).
pub(crate) fn to_decimal(w: &[u64], signed: bool) -> String {
    let negative = signed && is_neg(w);
    let mut m = if negative { neg(w) } else { w.to_vec() };
    if is_zero(&m) {
        return "0".into();
    }
    const CHUNK: u64 = 10_000_000_000_000_000_000;
    let mut parts = Vec::new();
    while !is_zero(&m) {
        let mut rem = 0u128;
        for x in m.iter_mut().rev() {
            let cur = rem << 64 | u128::from(*x);
            *x = (cur / u128::from(CHUNK)) as u64;
            rem = cur % u128::from(CHUNK);
        }
        parts.push(rem as u64);
    }
    let mut s = String::from(if negative { "-" } else { "" });
    let mut it = parts.iter().rev();
    if let Some(first) = it.next() {
        s.push_str(&first.to_string());
    }
    for p in it {
        s.push_str(&format!("{p:019}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(v: i128, n: usize) -> Vec<u64> {
        from_u128(v as u128, true, n)
    }

    #[test]
    fn arithmetic_matches_i128() {
        let vals = [0i128, 1, -1, 7, -7, 1 << 70, -(1 << 90), i128::from(i64::MAX), 12345678901234567];
        for &x in &vals {
            for &y in &vals {
                let (a, b) = (w(x, 4), w(y, 4));
                assert_eq!(add(&a, &b), w(x.wrapping_add(y), 4));
                assert_eq!(sub(&a, &b), w(x.wrapping_sub(y), 4));
                if let Some(p) = x.checked_mul(y) {
                    assert_eq!(mul(&a, &b), w(p, 4));
                }
                assert_eq!(cmp(&a, &b, true), x.cmp(&y));
                if y > 0 && x >= 0 {
                    let (q, r) = divrem_u(&a, &b);
                    assert_eq!((q, r), (w(x / y, 4), w(x % y, 4)));
                }
            }
        }
    }

    #[test]
    fn shifts_and_canon() {
        let one = w(1, 4);
        let big = shl(&one, 150);
        assert_eq!(shr(&big, 150, true), one);
        assert_eq!(shr(&w(-8, 4), 1, true), w(-4, 4));
        let mut x = shl(&one, 199);
        canon(&mut x, 200, true);
        assert!(is_neg(&x));
        assert!(!fits(&shl(&one, 199), 200, true));
        assert!(fits(&w(-1, 4), 200, true));
    }

    #[test]
    fn decimal() {
        assert_eq!(to_decimal(&w(-12345, 4), true), "-12345");
        assert_eq!(to_decimal(&shl(&w(1, 4), 199), false), "803469022129495137770981046170581301261101496891396417650688");
        assert_eq!(to_decimal(&w(0, 4), true), "0");
        assert_eq!(to_decimal(&w(30_000_000_000_000_000_005, 4), true), "30000000000000000005");
    }
}
