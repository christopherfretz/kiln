//! Constants, interned per module (`spec/llvm.md` §3.2).
//!
//! Constants are canonicalized the way LLVM's constant folding of
//! aggregate creation does: an aggregate whose elements are all
//! zero/undef/poison becomes `zeroinitializer`/`undef`/`poison`; an array of
//! `i8` integer constants becomes a byte string (`c"..."`); scalars "zero"
//! are integer/float zero or `null`. Interning then makes equal constants
//! equal [`Value`]s.

use std::collections::HashMap;

use crate::inst::{BinOp, CastOp, Flags, GepFlags};
use crate::types::TypeId;
use crate::value::{BlockId, ConstId, FuncId, Value};

/// An integer constant's bits, masked to its type's width.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum IntValue {
    /// Width ≤ 128.
    Small(u128),
    /// Width > 128: little-endian 64-bit words, `ceil(width / 64)` of them.
    Big(Box<[u64]>),
}

/// A constant's structure.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ConstData {
    /// Integer of type `ty`.
    Int {
        /// Integer type.
        ty: TypeId,
        /// Bits (zero-extended, masked to width).
        value: IntValue,
    },
    /// Floating-point of type `ty`; `bits` is the IEEE (or x87) encoding.
    Float {
        /// Floating-point type.
        ty: TypeId,
        /// Raw encoding in the low bits.
        bits: u128,
    },
    /// `null` of a pointer type.
    Null(TypeId),
    /// `none` (the `token` constant).
    NoneToken,
    /// `zeroinitializer` of an aggregate or vector type.
    Zero(TypeId),
    /// `undef`.
    Undef(TypeId),
    /// `poison`.
    Poison(TypeId),
    /// `[ty elem, ...]`.
    Array {
        /// Array type.
        ty: TypeId,
        /// Elements (constants).
        elems: Box<[Value]>,
    },
    /// `{ ty elem, ... }` (literal or identified struct type).
    Struct {
        /// Struct type.
        ty: TypeId,
        /// Fields (constants).
        elems: Box<[Value]>,
    },
    /// `<ty elem, ...>`.
    Vector {
        /// Vector type.
        ty: TypeId,
        /// Elements (constants).
        elems: Box<[Value]>,
    },
    /// `c"..."`, an `[N x i8]` array.
    Bytes {
        /// Array type.
        ty: TypeId,
        /// Contents.
        bytes: Box<[u8]>,
    },
    /// `getelementptr [flags] [inrange(a, b)] (source, ptr base, idx...)`.
    Gep {
        /// Result type (`ptr`).
        ty: TypeId,
        /// Source element type.
        source: TypeId,
        /// Flags.
        flags: GepFlags,
        /// `inrange(lo, hi)`.
        inrange: Option<(i64, i64)>,
        /// Base then indices.
        ops: Box<[Value]>,
    },
    /// `op (ty val to ty)`; LLVM 19 allows trunc, ptrtoint, inttoptr,
    /// bitcast, addrspacecast.
    Cast {
        /// Conversion.
        op: CastOp,
        /// Operand.
        val: Value,
        /// Destination type.
        ty: TypeId,
    },
    /// `op [flags] (ty lhs, ty rhs)`; LLVM 19 allows add, sub, mul, xor.
    Binary {
        /// Operator.
        op: BinOp,
        /// `nuw`/`nsw`.
        flags: Flags,
        /// Left operand.
        lhs: Value,
        /// Right operand.
        rhs: Value,
        /// Type.
        ty: TypeId,
    },
    /// `blockaddress(@f, %bb)`.
    BlockAddress {
        /// Function.
        func: FuncId,
        /// Block in that function.
        block: BlockId,
        /// Pointer type.
        ty: TypeId,
    },
    /// `dso_local_equivalent @f`.
    DsoLocalEquivalent(Value),
    /// `no_cfi @f`.
    NoCfi(Value),
}

impl ConstData {
    /// The constant's type.
    pub fn ty(&self) -> TypeId {
        match self {
            ConstData::Int { ty, .. }
            | ConstData::Float { ty, .. }
            | ConstData::Null(ty)
            | ConstData::Zero(ty)
            | ConstData::Undef(ty)
            | ConstData::Poison(ty)
            | ConstData::Array { ty, .. }
            | ConstData::Struct { ty, .. }
            | ConstData::Vector { ty, .. }
            | ConstData::Bytes { ty, .. }
            | ConstData::Gep { ty, .. }
            | ConstData::Cast { ty, .. }
            | ConstData::Binary { ty, .. }
            | ConstData::BlockAddress { ty, .. } => *ty,
            ConstData::NoneToken => TypeId::TOKEN,
            ConstData::DsoLocalEquivalent(_) | ConstData::NoCfi(_) => TypeId::PTR,
        }
    }

    /// True for integer zero, float +0.0, `null`, `zeroinitializer`.
    pub fn is_zero(&self) -> bool {
        match self {
            ConstData::Int { value: IntValue::Small(v), .. } => *v == 0,
            ConstData::Int { value: IntValue::Big(w), .. } => w.iter().all(|&x| x == 0),
            ConstData::Float { bits, .. } => *bits == 0,
            ConstData::Null(_) | ConstData::Zero(_) => true,
            _ => false,
        }
    }
}

/// Interned constants.
#[derive(Debug, Default)]
pub(crate) struct ConstTable {
    data: Vec<ConstData>,
    map: HashMap<ConstData, ConstId>,
}

impl ConstTable {
    pub(crate) fn intern(&mut self, d: ConstData) -> Value {
        if let Some(&id) = self.map.get(&d) {
            return Value::constant(id);
        }
        let id = ConstId(self.data.len() as u32);
        self.data.push(d.clone());
        self.map.insert(d, id);
        Value::constant(id)
    }

    pub(crate) fn get(&self, id: ConstId) -> &ConstData {
        &self.data[id.index()]
    }

    pub(crate) fn len(&self) -> usize {
        self.data.len()
    }
}

/// Mask `v` to `bits` (≤ 128).
pub(crate) fn mask128(v: u128, bits: u32) -> u128 {
    if bits >= 128 { v } else { v & ((1u128 << bits) - 1) }
}

/// Write the signed decimal value of a `bits`-wide two's-complement
/// integer stored in little-endian words.
pub(crate) fn write_signed_words<W: std::fmt::Write + ?Sized>(
    w: &mut W,
    words: &[u64],
    bits: u32,
) -> std::fmt::Result {
    const CHUNK: u64 = 10_000_000_000_000_000_000;
    let top = bits - 1;
    let neg = (words[(top / 64) as usize] >> (top % 64)) & 1 == 1;
    let mut mag: Vec<u64> = words.to_vec();
    if neg {
        // Two's complement negate within `bits`.
        let mut carry = 1u64;
        for x in &mut mag {
            let (s, c) = (!*x).overflowing_add(carry);
            *x = s;
            carry = u64::from(c);
        }
        let rem = bits % 64;
        if rem != 0 {
            let last = mag.len() - 1;
            mag[last] &= (1u64 << rem) - 1;
        }
        w.write_char('-')?;
    }
    // Repeated division by 10^19.
    let mut parts: Vec<u64> = Vec::new();
    while mag.iter().any(|&x| x != 0) {
        let mut rem: u128 = 0;
        for x in mag.iter_mut().rev() {
            let cur = (rem << 64) | u128::from(*x);
            *x = (cur / u128::from(CHUNK)) as u64;
            rem = cur % u128::from(CHUNK);
        }
        parts.push(rem as u64);
    }
    match parts.split_last() {
        None => w.write_char('0'),
        Some((first, rest)) => {
            write!(w, "{first}")?;
            for p in rest.iter().rev() {
                write!(w, "{p:019}")?;
            }
            Ok(())
        }
    }
}
