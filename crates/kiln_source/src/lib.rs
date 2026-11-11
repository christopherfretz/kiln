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
//! - [`symbol`]: the shared identifier [`Interner`](symbol::Interner) and
//!   its 4-byte [`Symbol`](symbol::Symbol) handles; [`fx`]: the `FxHash`
//!   hasher used by every compiler-internal hash table.
//! - [`activity`]: the per-thread "current activity" breadcrumb that
//!   internal-compiler-error reports read.
//! - [`trace`]: the per-thread `-ftime-trace` recorder (Chrome trace
//!   events in clang's format).
#![forbid(unsafe_code)]

pub mod activity;
mod arena;
pub mod fx;
pub mod hash;
mod loc;
mod manager;
pub mod symbol;
pub mod trace;

pub use loc::{FileId, Loc, Span};
pub use manager::{
    ExpansionInfo, ExpansionKind, FileKind, LazyCopy, LazyFile, LazyOutcome, LineCol, LoadError,
    LoadReport, LocSpaceExhausted, PresumedLoc, Replacement, SourceManager, SourcePathStyle,
    read_open_file,
};
