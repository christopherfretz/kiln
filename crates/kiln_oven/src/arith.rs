//! Checked integer and floating arithmetic on register values, and the
//! number formatting clang uses in notes.
//!
//! Integers travel as `u128` bit patterns, sign- or zero-extended from
//! their width ("canonical"); operations check [expr.pre]/4 overflow for
//! signed types and wrap for unsigned ones ([basic.fundamental]/2).

use kiln_ast::consts::FloatKind;

use crate::kbc::Op;
use crate::softfloat;

/// Why an integer operation has no value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum IntErr {
    /// The mathematical result of `a op b` is not representable.
    Overflow,
    DivZero,
    /// The (signed, canonical) shift count is negative.
    ShiftNeg,
    /// The shift count is at least the width.
    ShiftBig,
}

/// Sign- or zero-extends the low `width` bits of `v`.
#[inline]
pub(crate) fn canon(v: u128, width: u32, signed: bool) -> u128 {
    if width >= 128 {
        return v;
    }
    let sh = 128 - width;
    if signed { (((v << sh) as i128) >> sh) as u128 } else { (v << sh) >> sh }
}

#[inline]
fn fits(v: i128, width: u32) -> bool {
    width >= 128 || {
        let lim = 1i128 << (width - 1);
        v >= -lim && v < lim
    }
}

/// `a op b` for integer binary ops (`a`, `b` canonical); `cw`/`cs` are
/// the width and signedness of a shift count.
pub(crate) fn int_bin(
    op: Op,
    width: u32,
    signed: bool,
    a: u128,
    b: u128,
    cw: u32,
    cs: bool,
) -> Result<u128, IntErr> {
    let r = match op {
        Op::IAdd | Op::ISub | Op::IMul => {
            if signed {
                let (x, y) = (a as i128, b as i128);
                let v = match op {
                    Op::IAdd => x.checked_add(y),
                    Op::ISub => x.checked_sub(y),
                    _ => x.checked_mul(y),
                };
                match v {
                    Some(v) if fits(v, width) => v as u128,
                    _ => return Err(IntErr::Overflow),
                }
            } else {
                match op {
                    Op::IAdd => a.wrapping_add(b),
                    Op::ISub => a.wrapping_sub(b),
                    _ => a.wrapping_mul(b),
                }
            }
        }
        Op::IDiv | Op::IRem => {
            if b == 0 {
                return Err(IntErr::DivZero);
            }
            if signed {
                let (x, y) = (a as i128, b as i128);
                if y == -1 && (x == i128::MIN || !fits(x.wrapping_neg(), width)) {
                    return Err(IntErr::Overflow);
                }
                if op == Op::IDiv { (x / y) as u128 } else { (x % y) as u128 }
            } else if op == Op::IDiv {
                a / b
            } else {
                a % b
            }
        }
        Op::IShl | Op::IShr => {
            let c = canon(b, cw, cs);
            if cs && (c as i128) < 0 {
                return Err(IntErr::ShiftNeg);
            }
            if c >= u128::from(width) {
                return Err(IntErr::ShiftBig);
            }
            let c = c as u32;
            if op == Op::IShl {
                a << c
            } else if signed {
                ((a as i128) >> c) as u128
            } else {
                a >> c
            }
        }
        Op::IAnd => a & b,
        Op::IOr => a | b,
        Op::IXor => a ^ b,
        _ => unreachable!("not an integer binary op: {op:?}"),
    };
    Ok(canon(r, width, signed))
}

/// A 256-bit signed value (exact results of overflowing 128-bit ops, for
/// notes only).
#[derive(Clone, Copy, Debug)]
struct Big {
    neg: bool,
    /// Magnitude, little-endian 64-bit limbs.
    mag: [u64; 4],
}

impl Big {
    fn from_i(v: i128, signed: bool) -> Big {
        if signed && v < 0 {
            Big::from_u(v.unsigned_abs(), true)
        } else {
            Big::from_u(v as u128, false)
        }
    }

    fn from_u(m: u128, neg: bool) -> Big {
        Big { neg, mag: [m as u64, (m >> 64) as u64, 0, 0] }
    }

    fn cmp_mag(a: &[u64; 4], b: &[u64; 4]) -> std::cmp::Ordering {
        for i in (0..4).rev() {
            match a[i].cmp(&b[i]) {
                std::cmp::Ordering::Equal => {}
                o => return o,
            }
        }
        std::cmp::Ordering::Equal
    }

    fn add_mag(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
        let mut r = [0u64; 4];
        let mut c = 0u128;
        for i in 0..4 {
            let s = u128::from(a[i]) + u128::from(b[i]) + c;
            r[i] = s as u64;
            c = s >> 64;
        }
        r
    }

    fn sub_mag(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
        let mut r = [0u64; 4];
        let mut borrow = 0i128;
        for i in 0..4 {
            let mut d = i128::from(a[i]) - i128::from(b[i]) - borrow;
            borrow = 0;
            if d < 0 {
                d += 1 << 64;
                borrow = 1;
            }
            r[i] = d as u64;
        }
        r
    }

    fn add(self, o: Big) -> Big {
        if self.neg == o.neg {
            return Big { neg: self.neg, mag: Big::add_mag(&self.mag, &o.mag) };
        }
        match Big::cmp_mag(&self.mag, &o.mag) {
            std::cmp::Ordering::Less => Big { neg: o.neg, mag: Big::sub_mag(&o.mag, &self.mag) },
            _ => Big { neg: self.neg, mag: Big::sub_mag(&self.mag, &o.mag) },
        }
    }

    fn mul(self, o: Big) -> Big {
        let mut r = [0u64; 4];
        for i in 0..4 {
            let mut c = 0u128;
            for j in 0..4 - i {
                let t = u128::from(self.mag[i]) * u128::from(o.mag[j]) + u128::from(r[i + j]) + c;
                r[i + j] = t as u64;
                c = t >> 64;
            }
        }
        Big { neg: self.neg != o.neg, mag: r }
    }

    fn to_decimal(self) -> String {
        let mut m = self.mag;
        if m == [0; 4] {
            return "0".into();
        }
        let mut digits = Vec::new();
        while m != [0; 4] {
            let mut rem = 0u128;
            for i in (0..4).rev() {
                let cur = rem << 64 | u128::from(m[i]);
                m[i] = (cur / 10) as u64;
                rem = cur % 10;
            }
            digits.push(b'0' + rem as u8);
        }
        if self.neg {
            digits.push(b'-');
        }
        digits.reverse();
        String::from_utf8(digits).unwrap_or_default()
    }
}

/// The decimal spelling of the exact result of an overflowing `a op b`
/// (`op` one of add/sub/mul/div/neg, `b` ignored for negation).
pub(crate) fn exact_result(op: Op, signed: bool, a: u128, b: u128) -> String {
    let x = Big::from_i(a as i128, signed);
    let y = Big::from_i(b as i128, signed);
    let r = match op {
        Op::IAdd => x.add(y),
        Op::ISub => x.add(Big { neg: !y.neg, ..y }),
        Op::IMul => x.mul(y),
        // MIN / -1 and -MIN.
        Op::IDiv | Op::IRem | Op::INeg => Big { neg: !x.neg, ..x },
        _ => x,
    };
    r.to_decimal()
}

/// The decimal spelling of a canonical integer.
pub(crate) fn int_to_string(v: u128, signed: bool) -> String {
    if signed { (v as i128).to_string() } else { v.to_string() }
}

/// Why a floating operation has no value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FloatErr {
    NaN,
    DivZero,
    /// Float-to-integer conversion out of range.
    Range,
    /// The format's arithmetic is not implemented.
    Unsupported,
}

fn to_f64(kind: FloatKind, bits: u128) -> f64 {
    match kind {
        FloatKind::Single => f64::from(f32::from_bits(bits as u32)),
        FloatKind::Double => f64::from_bits(bits as u64),
        _ => softfloat::to_f64(kind, bits),
    }
}

fn from_f64(kind: FloatKind, v: f64) -> u128 {
    match kind {
        FloatKind::Single => u128::from((v as f32).to_bits()),
        FloatKind::Double => u128::from(v.to_bits()),
        _ => softfloat::from_f64(kind, v),
    }
}

/// Whether `bits` is a NaN of `kind`.
pub(crate) fn is_nan(kind: FloatKind, bits: u128) -> bool {
    match kind {
        FloatKind::Single => f32::from_bits(bits as u32).is_nan(),
        FloatKind::Double => f64::from_bits(bits as u64).is_nan(),
        _ => softfloat::is_nan(kind, bits),
    }
}

/// Whether `bits` is a (positive or negative) zero of `kind`.
pub(crate) fn is_zero(kind: FloatKind, bits: u128) -> bool {
    let sign = 1u128 << (kind.bits() - 1);
    bits & !sign == 0
}

/// `a op b` ([expr.mul], [expr.add]); a NaN result and division by zero
/// fail as clang's `handleFloatFloatBinOp` does.
pub(crate) fn float_bin(op: Op, kind: FloatKind, a: u128, b: u128) -> Result<u128, FloatErr> {
    if op == Op::FDiv && is_zero(kind, b) {
        return Err(FloatErr::DivZero);
    }
    let r = float_bin_raw(op, kind, a, b).ok_or(FloatErr::Unsupported)?;
    if is_nan(kind, r) {
        return Err(FloatErr::NaN);
    }
    Ok(r)
}

/// `a op b` rounded to nearest even, with no check of the result (an
/// `APFloat` operator: a NaN or an infinity is a value); `None` for a
/// format whose arithmetic is not implemented.
///
/// NaNs follow `APFloat`, not the host: a NaN operand yields the first
/// NaN operand, quieted, its sign and payload kept; an invalid operation
/// (`inf - inf`, `0 * inf`, `0 / 0`) the default NaN, positive and quiet.
pub(crate) fn float_bin_raw(op: Op, kind: FloatKind, a: u128, b: u128) -> Option<u128> {
    if matches!(kind, FloatKind::X87Extended | FloatKind::Quad) {
        return softfloat::bin(op, kind, a, b);
    }
    for x in [a, b] {
        if is_nan(kind, x) {
            return Some(softfloat::quiet(kind, x));
        }
    }
    let r = float_bin_native(op, kind, a, b);
    Some(if is_nan(kind, r) { softfloat::make_nan(kind, 0, false) } else { r })
}

/// [`float_bin_raw`] of non-NaN operands in the host's arithmetic.
fn float_bin_native(op: Op, kind: FloatKind, a: u128, b: u128) -> u128 {
    match kind {
        FloatKind::Single => {
            let (x, y) = (f32::from_bits(a as u32), f32::from_bits(b as u32));
            let v = match op {
                Op::FAdd => x + y,
                Op::FSub => x - y,
                Op::FMul => x * y,
                _ => x / y,
            };
            u128::from(v.to_bits())
        }
        FloatKind::Double => {
            let (x, y) = (f64::from_bits(a as u64), f64::from_bits(b as u64));
            let v = match op {
                Op::FAdd => x + y,
                Op::FSub => x - y,
                Op::FMul => x * y,
                _ => x / y,
            };
            u128::from(v.to_bits())
        }
        // binary16/bfloat16: the f64 result rounds correctly (53 >= 2p + 2).
        FloatKind::Half | FloatKind::BFloat16 => {
            let (x, y) = (to_f64(kind, a), to_f64(kind, b));
            let v = match op {
                Op::FAdd => x + y,
                Op::FSub => x - y,
                Op::FMul => x * y,
                _ => x / y,
            };
            from_f64(kind, v)
        }
        FloatKind::X87Extended | FloatKind::Quad => unreachable!("softfloat formats"),
    }
}

/// `-a`.
pub(crate) fn float_neg(kind: FloatKind, a: u128) -> u128 {
    a ^ 1u128 << (kind.bits() - 1)
}

/// Compares two floats; `None` if unordered.
pub(crate) fn float_cmp(kind: FloatKind, a: u128, b: u128) -> Option<std::cmp::Ordering> {
    match kind {
        FloatKind::Single => f32::from_bits(a as u32).partial_cmp(&f32::from_bits(b as u32)),
        FloatKind::Double => f64::from_bits(a as u64).partial_cmp(&f64::from_bits(b as u64)),
        FloatKind::Half | FloatKind::BFloat16 => to_f64(kind, a).partial_cmp(&to_f64(kind, b)),
        _ => softfloat::cmp(kind, a, b),
    }
}

/// Converts between floating formats (round to nearest even).
pub(crate) fn float_cast(from: FloatKind, to: FloatKind, a: u128) -> u128 {
    if from == to {
        return a;
    }
    // A NaN keeps its sign and its payload, shifted by the difference in
    // precision, and is quieted (`APFloat::convert`); the host's
    // conversions promise neither.
    if matches!(from, FloatKind::X87Extended | FloatKind::Quad)
        || matches!(to, FloatKind::X87Extended | FloatKind::Quad)
        || is_nan(from, a)
    {
        return softfloat::cast(from, to, a);
    }
    from_f64(to, to_f64(from, a))
}

/// Converts a canonical integer to a float (round to nearest even).
pub(crate) fn int_to_float(kind: FloatKind, v: u128, signed: bool) -> u128 {
    match kind {
        FloatKind::Single => {
            u128::from(if signed { (v as i128) as f32 } else { v as f32 }.to_bits())
        }
        FloatKind::Double => {
            u128::from(if signed { (v as i128) as f64 } else { v as f64 }.to_bits())
        }
        _ => softfloat::from_int(kind, v, signed),
    }
}

/// The value `APFloat::convertToInteger` gives an out-of-range float to
/// integer conversion of `a` (clang folds on with it past the undefined
/// behaviour): 0 for a NaN, else the bound of `width`/`signed` on the
/// value's side.
pub(crate) fn float_to_int_saturated(kind: FloatKind, a: u128, width: u32, signed: bool) -> u128 {
    if is_nan(kind, a) {
        return 0;
    }
    let neg = a & softfloat::sign_bit(kind) != 0;
    let ones = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
    match (signed, neg) {
        (true, false) => ones >> 1,
        (true, true) => canon(1u128 << (width.min(128) - 1), width, true),
        (false, false) => ones,
        (false, true) => 0,
    }
}

/// Truncates a float to an integer of `width`/`signed`
/// ([conv.fpint]/1: out of range is undefined).
pub(crate) fn float_to_int(
    kind: FloatKind,
    a: u128,
    width: u32,
    signed: bool,
) -> Result<u128, FloatErr> {
    let (neg, int) = match kind {
        FloatKind::X87Extended | FloatKind::Quad => {
            softfloat::trunc(kind, a).ok_or(FloatErr::Range)?
        }
        _ => {
            let v = to_f64(kind, a);
            if !v.is_finite() {
                return Err(FloatErr::Range);
            }
            let t = v.trunc();
            if t.abs() >= 3.402_823_669_209_385e38 {
                return Err(FloatErr::Range);
            }
            (t < 0.0, t.abs() as u128)
        }
    };
    if signed {
        let lim = if width >= 128 { 1u128 << 127 } else { 1u128 << (width - 1) };
        if (!neg && int >= lim) || (neg && int > lim) {
            return Err(FloatErr::Range);
        }
        let v = if neg { (int as i128).wrapping_neg() as u128 } else { int };
        Ok(canon(v, width, true))
    } else {
        if neg && int != 0 {
            return Err(FloatErr::Range);
        }
        if width < 128 && int >> width != 0 {
            return Err(FloatErr::Range);
        }
        Ok(int)
    }
}

/// Whether the float is nonzero (NaN is true).
pub(crate) fn float_truth(kind: FloatKind, a: u128) -> bool {
    !is_zero(kind, a)
}

/// How clang prints a float in a constant-evaluation note such as
/// `note_constexpr_overflow` (`OptionalDiagnostic << APFloat`); see
/// [`kiln_ast::consts::fltstr::note_string`].
pub(crate) fn apfloat_to_string(kind: FloatKind, a: u128) -> String {
    kiln_ast::consts::fltstr::note_string(kind, a)
}

/// `raw_ostream << double` (`%e`): how clang prints a float argument in
/// a call note.
pub(crate) fn float_arg_string(kind: FloatKind, a: u128) -> String {
    let v = to_f64(kind, a);
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf".into() } else { "inf".into() };
    }
    let s = format!("{v:.6e}");
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let e: i32 = e.parse().unwrap_or(0);
    format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.unsigned_abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    // [expr.pre]/4: signed overflow is undefined; [basic.fundamental]/2:
    // unsigned arithmetic is modular.
    #[test]
    fn int_ops() {
        assert_eq!(int_bin(Op::IAdd, 32, true, 1, 2, 32, true), Ok(3));
        let max = i32::MAX as u128;
        assert_eq!(int_bin(Op::IAdd, 32, true, max, 1, 32, true), Err(IntErr::Overflow));
        assert_eq!(int_bin(Op::IAdd, 32, false, u128::from(u32::MAX), 1, 32, false), Ok(0));
        let min = canon(1 << 31, 32, true);
        assert_eq!(int_bin(Op::IDiv, 32, true, min, u128::MAX, 32, true), Err(IntErr::Overflow));
        assert_eq!(int_bin(Op::IShl, 32, true, 1, 32, 32, true), Err(IntErr::ShiftBig));
        assert_eq!(exact_result(Op::IAdd, true, max, 1), "2147483648");
        assert_eq!(
            exact_result(Op::IMul, true, i128::MAX as u128, 2),
            "340282366920938463463374607431768211454"
        );
    }

    // [conv.fpint]/1 note formatting (APFloat::toString).
    #[test]
    fn apfloat_strings() {
        assert_eq!(apfloat_to_string(FloatKind::Double, u128::from(1e10f64.to_bits())), "1.0E+10");
        assert_eq!(apfloat_to_string(FloatKind::Double, u128::from(3e9f64.to_bits())), "3.0E+9");
        assert_eq!(apfloat_to_string(FloatKind::Double, u128::from(2.5f64.to_bits())), "2.5");
        assert_eq!(apfloat_to_string(FloatKind::Double, u128::from(300.0f64.to_bits())), "300");
        assert_eq!(apfloat_to_string(FloatKind::Single, u128::from(1e20f32.to_bits())), "1.0E+20");
        assert_eq!(
            float_arg_string(FloatKind::Double, u128::from(1.0f64.to_bits())),
            "1.000000e+00"
        );
    }
}
