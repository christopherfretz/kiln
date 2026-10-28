//! Lowering of the typed AST to LLVM IR.
//!
//! See `docs/ARCHITECTURE.md` and `spec/codegen.md` for the contract
//! this crate implements.
#![forbid(unsafe_code)]
mod fold;
mod fir;
