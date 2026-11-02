//! Floating-point arithmetic on raw encodings, rounded to nearest even
//! (LLVM `APFloat`), shared with codegen's constant folder (LLVM's
//! `ConstantFolder`, which folds `fadd half`, `fpext`, `sitofp ... to
//! fp128` and the like that clang's IR builder never emits). The formats
//! are the Oven's (spec/oven.md "Floating point").

pub use kiln_ast::consts::FloatKind;

use crate::arith;
use crate::kbc::Op;

/// A binary floating-point operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    /// `fadd`.
    Add,
    /// `fsub`.
    Sub,
    /// `fmul`.
    Mul,
    /// `fdiv`.
    Div,
}

/// `a op b` in `kind` (a NaN or an infinity is a value); `None` for a
/// format whose arithmetic is not implemented.
///
/// NaN results are `APFloat`'s, independent of the host: a NaN operand
/// is the result, quieted, sign and payload kept (the left one when both
/// are NaN, whichever is signaling; `fsub` does not flip it), and an
/// invalid operation (`inf - inf`, `0 * inf`, `0 / 0`) gives the default
/// positive quiet NaN.
#[must_use]
pub fn bin(op: BinOp, kind: FloatKind, a: u128, b: u128) -> Option<u128> {
    if is_nan(kind, a) {
        return Some(quiet(kind, a));
    }
    if is_nan(kind, b) {
        return Some(quiet(kind, b));
    }
    let op = match op {
        BinOp::Add => Op::FAdd,
        BinOp::Sub => Op::FSub,
        BinOp::Mul => Op::FMul,
        BinOp::Div => Op::FDiv,
    };
    let x = arith::float_bin_raw(op, kind, a, b)?;
    Some(if is_nan(kind, x) { default_nan(kind) } else { x })
}

/// `fpext` / `fptrunc` from `from` to `to`. A NaN keeps its sign, its
/// significand is shifted by the precision difference (truncated when
/// narrowing) and the result is quiet (`APFloat::convert`).
#[must_use]
pub fn cast(from: FloatKind, to: FloatKind, a: u128) -> u128 {
    if from == to || !is_nan(from, a) {
        return arith::float_cast(from, to, a);
    }
    let (pf, pt) = (precision(from), precision(to));
    let sig = a & mask(frac_bits(from));
    let sig = if pt >= pf { sig << (pt - pf) } else { sig >> (pf - pt) };
    let sign = a >> (from.bits() - 1) & 1;
    let x = sign << (to.bits() - 1) | (sig & mask(frac_bits(to))) | exp_field(to);
    quiet(to, x)
}

/// The significand precision `p` (integer bit included).
fn precision(kind: FloatKind) -> u32 {
    match kind {
        FloatKind::Half => 11,
        FloatKind::BFloat16 => 8,
        FloatKind::Single => 24,
        FloatKind::Double => 53,
        FloatKind::X87Extended => 64,
        FloatKind::Quad => 113,
    }
}

/// The stored significand width: `p - 1`, or all 64 bits of x87's
/// explicit-integer-bit significand.
fn frac_bits(kind: FloatKind) -> u32 {
    if kind == FloatKind::X87Extended { 64 } else { precision(kind) - 1 }
}

fn mask(n: u32) -> u128 {
    (1u128 << n) - 1
}

/// The all-ones exponent field in place.
fn exp_field(kind: FloatKind) -> u128 {
    mask(kind.bits() - 1) & !mask(frac_bits(kind))
}

/// `a` with the quiet bit (`p - 2`) set, and x87's integer bit.
fn quiet(kind: FloatKind, a: u128) -> u128 {
    let int = if kind == FloatKind::X87Extended { 1 << 63 } else { 0 };
    a | 1 << (precision(kind) - 2) | int
}

/// `APFloat`'s default NaN: positive, quiet, zero payload.
fn default_nan(kind: FloatKind) -> u128 {
    quiet(kind, exp_field(kind))
}

/// `sitofp` / `uitofp` of the integer `v` (sign- or zero-extended to 128
/// bits) to `kind`.
#[must_use]
pub fn from_int(kind: FloatKind, v: u128, signed: bool) -> u128 {
    arith::int_to_float(kind, v, signed)
}

/// Whether `bits` is a NaN of `kind`.
#[must_use]
pub fn is_nan(kind: FloatKind, bits: u128) -> bool {
    arith::is_nan(kind, bits)
}
