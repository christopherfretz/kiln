//! Source files, spans and locations (including macro-expansion locations).
//!
//! See `docs/ARCHITECTURE.md` and `spec/source.md` for the contract
//! this crate implements.
//!
//! - [`SourceManager`] owns every buffer of a translation unit. It is
//!   append-only and shared by `&` reference between the lexer, the
//!   preprocessor and the diagnostics engine.
//! - [`Loc`] (4 bytes) is a position in a single global offset space that
//!   is partitioned into per-buffer ranges and per-expansion ranges, as in
//!   clang's `SourceLocation`. [`Span`] (8 bytes) is a half-open range.
//! - Macro-expansion locations map to (spelling, expansion) pairs through
//!   [`SourceManager::add_expansion`] entries.
//! - `#line` presumed locations: [`SourceManager::add_line_directive`] and
//!   [`SourceManager::presumed_line_col`].
#![forbid(unsafe_code)]

mod arena;
mod loc;
mod manager;

pub use loc::{FileId, Loc, Span};
pub use manager::{
    ExpansionInfo, ExpansionKind, LineCol, LoadError, LoadReport, LocSpaceExhausted, PresumedLoc,
    Replacement, SourceManager,
};
