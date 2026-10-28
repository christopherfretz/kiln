//! Translation phases 1-3: decoding, line splicing, preprocessing tokens ([lex]).
//!
//! The entry point is [`Lexer`], a pull lexer producing [`Token`]s. It
//! records problems as [`LexDiag`] data (the preprocessor maps them to
//! `kiln_diag` diagnostics), interns identifiers in a shared [`Interner`],
//! and never allocates per token. See `spec/lex.md` for the normative
//! contract this crate implements.
#![forbid(unsafe_code)]

mod diag;
mod dump;
#[cfg(test)]
mod golden_tests;
mod interner;
mod lexer;
pub mod literal;
mod options;
mod phase;
mod spelling;
mod token;
mod unicode;
#[allow(clippy::unreadable_literal, reason = "generated offsets")]
mod unicode_names;
mod unicode_tables;

pub use diag::{LexDiag, LexDiagKind, LexSeverity};
pub use dump::{dump, dump_source, dump_token};
pub use interner::{Interner, Symbol};
pub use lexer::Lexer;
pub use options::{LexMode, LexOptions, Std};
pub use spelling::{clean, source_offset, spelling};
pub use token::{Encoding, Punct, TokFlags, TokKind, Token};
pub use unicode::{NameMatch, lookup_char_name};

/// The Unicode version of the identifier tables ([lex.name] uses UAX #31
/// `XID_Start`/`XID_Continue`).
pub const UNICODE_VERSION: &str = unicode_tables::UNICODE_VERSION;
