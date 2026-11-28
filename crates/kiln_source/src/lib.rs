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
//!   [`SourceManager::add_expansion`] entries;
//!   [`SourceManager::expansion_entries`] lists them.
//! - Tooling-client queries: [`SourceManager::editable_range`] (the file
//!   characters a range written through macros stands for, when they can
//!   be edited as one file range).
//! - `#line` presumed locations: [`SourceManager::add_line_directive`] and
//!   [`SourceManager::presumed_line_col`].
//! - [`symbol`]: the shared identifier [`Interner`](symbol::Interner) and
//!   its 4-byte [`Symbol`](symbol::Symbol) handles; [`fx`]: the `FxHash`
//!   hasher used by every compiler-internal hash table.
//! - [`activity`]: the per-thread "current activity" breadcrumb that
//!   internal-compiler-error reports read.
//! - [`Overlay`]: the files of a run that differ from the disk (replaced,
//!   created, removed; [`SourceManager::set_overlay`]; tooling clients).
//! - [`WorkDir`]: the working directory relative paths open against
//!   ([`SourceManager::set_work_dir`]; clang's `-working-directory`).
//! - [`trace`]: the per-thread `-ftime-trace` recorder (Chrome trace
//!   events in clang's format).
#![forbid(unsafe_code)]

pub mod activity;
mod arena;
mod editable;
pub mod fx;
pub mod hash;
pub mod image_locs;
mod loc;
mod manager;
mod overlay;
pub mod symbol;
pub mod trace;
mod workdir;

pub use loc::{FileId, Loc, Span};
pub use manager::{
    ExpansionEntries, ExpansionEntry, ExpansionInfo, ExpansionKind, FileKind, FileOrigin, LazyCopy,
    LazyFile, LazyOutcome, LineCol, LoadError, LoadReport, LocSpaceExhausted, PresumedLoc,
    Replacement, SourceManager, SourcePathStyle, read_open_file, read_open_file_shared,
};
pub use overlay::{
    Content, CreatedDir, CreatedKey, DirCache, EntryState, LoadFailures, Loader, Overlay, Resolved,
};
pub use workdir::WorkDir;
