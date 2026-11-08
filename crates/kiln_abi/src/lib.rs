//! Itanium C++ ABI: mangling, record and vtable layout, target argument lowering.
//!
//! See `docs/ARCHITECTURE.md` and `spec/abi.md` for the contract
//! this crate implements. M2 provides the target description
//! ([`mod@target`]), type and record layout ([`layout`]) and the
//! `-fdump-record-layouts` printer ([`dump`]). M3 adds the Itanium vtable
//! interface ([`vtable`], implemented by WP-078b). [`dump_blocks`] splits
//! both dumps into the blocks the clang differentials compare.
#![forbid(unsafe_code)]

pub mod asm;
pub mod branch_protection;
pub mod call;
pub mod dump;
pub mod dump_blocks;
pub mod irtype;
pub mod layout;
pub mod mangle;
pub mod mangle_decomp;
pub mod target;
pub mod vtable;

pub use branch_protection::{BranchProtection, SignScope};
pub use dump::{
    DumpOptions, dump_all_record_layouts, dump_record_layout, record_definitions,
    write_record_layout, write_record_layout_with,
};
pub use layout::{
    LayoutCache, Layouter, MAX_SUBOBJECTS, TypeLayout, array_layout, ensure_layout,
    padding::{MAX_PADDING_MASK_BYTES, padding_mask},
    record_layout, subobjects, type_size_align,
};
pub use mangle::ReflectionMangling;
pub use target::{Arch, Target, Triple, TripleError, target};
