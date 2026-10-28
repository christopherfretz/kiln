//! In-memory model of LLVM IR (LLVM 19 dialect) with a textual printer.
//! No FFI.
//!
//! The model is a set of index arenas: types live in a [`TypeTable`] and are
//! named by [`TypeId`]; constants, globals, functions and metadata live in the
//! [`Module`] and are named by `u32` ids; instructions and blocks live in
//! their [`Function`]. A [`Value`] is a 4-byte tagged index into one of those
//! arenas. [`IrBuilder`] appends instructions with light verification, and
//! [`Module::print`] writes text that `llvm-as` and `opt -passes=verify`
//! accept.
//!
//! See `docs/ARCHITECTURE.md` and `spec/llvm.md` for the contract this crate
//! implements.
#![forbid(unsafe_code)]

mod attr;
mod builder;
mod constant;
mod error;
mod float;
mod function;
mod inst;
pub mod intrinsics;
mod metadata;
mod module;
mod print;
mod types;
mod value;
mod verify;

pub use attr::{Attr, AttrKind, AttrList};
pub use builder::{CallOpts, CmpXchgOpts, IrBuilder, MemOpts, SHUFFLE_POISON};
pub use constant::{ConstData, IntValue};
pub use error::{IrError, IrErrorKind, IrResult};
pub use function::Function;
pub use inst::{
    Align, BinOp, CallConv, CastOp, Clause, FastMath, Flags, FloatPredicate, GepFlags, Inst,
    IntPredicate, Opcode, OperandBundle, Ordering, RmwOp, SyncScope, TailKind,
};
pub use metadata::{MdField, MdNode};
pub use module::*;
pub use types::{TypeData, TypeId, TypeTable};
pub use value::{
    AliasId, AsmId, AttrSetId, BlockId, ConstId, FuncId, GlobalId, InstId, MdKindId, MetadataId,
    Value, ValueRef,
};
pub use verify::verify_function;
