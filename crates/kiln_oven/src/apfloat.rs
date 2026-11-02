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
#[must_use]
pub fn bin(op: BinOp, kind: FloatKind, a: u128, b: u128) -> Option<u128> {
    let op = match op {
        BinOp::Add => Op::FAdd,
        BinOp::Sub => Op::FSub,
        BinOp::Mul => Op::FMul,
        BinOp::Div => Op::FDiv,
    };
    arith::float_bin_raw(op, kind, a, b)
}

/// `fpext` / `fptrunc` from `from` to `to`.
#[must_use]
pub fn cast(from: FloatKind, to: FloatKind, a: u128) -> u128 {
    arith::float_cast(from, to, a)
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
