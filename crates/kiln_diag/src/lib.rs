//! Diagnostics for kiln: ids, warning groups and policy, the `Diag`
//! builder, the thread-safe `DiagEngine`, and clang-compatible text and
//! JSON rendering.
//!
//! See `spec/diag.md` for the normative contract this crate implements.
//!
//! ```ignore
//! let engine = DiagEngine::new(&sm, &opts, TextSink::new(std::io::stderr(), &opts));
//! Diag::new(DiagId::pp_macro_redefined, name)
//!     .arg("FOO")
//!     .note(DiagId::note_previous_definition, prev)
//!     .emit(&engine);
//! if let Some(s) = engine.summary() { eprintln!("{s}"); }
//! ```
#![forbid(unsafe_code)]

mod clang_groups;
mod columns;
mod diag;
mod engine;
mod groups;
mod ids;
mod json;
mod layout;
mod options;
mod policy;
pub mod template;
mod text;

pub use diag::{Diag, Diagnostic, FixIt, Label, Level, NoteMsg};
pub use engine::{DiagEngine, DiagSink, MemorySink, SharedBuffer};
pub use groups::Group;
pub use ids::{DiagClass, DiagId, DiagKind};
pub use json::{JsonSink, escape_json};
pub use layout::SCRATCH_BUFFER_NAME;
pub use options::{DEFAULT_TABSTOP, DiagOptions, Format, OptionDiag};
pub use policy::Severity;
pub use text::TextSink;
