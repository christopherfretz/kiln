//! Itanium C++ ABI: mangling, record and vtable layout, target argument lowering.
//!
//! See `docs/ARCHITECTURE.md` and `spec/abi.md` for the contract
//! this crate implements. M2 provides the target description
//! ([`mod@target`]), type and record layout ([`layout`]) and the
//! `-fdump-record-layouts` printer ([`dump`]).
#![forbid(unsafe_code)]

pub mod dump;
pub mod layout;
pub mod target;

pub use dump::{dump_record_layout, write_record_layout};
pub use layout::{
    Layouter, TypeLayout, array_layout, ensure_layout, record_layout, type_size_align,
};
pub use target::{Arch, Target, Triple, TripleError, target};
