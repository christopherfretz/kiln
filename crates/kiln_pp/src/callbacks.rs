//! Observer hooks ([`PpCallbacks`]): clang's `PPCallbacks`.
//!
//! Every method has a no-op default. Install with
//! [`crate::Preprocessor::set_callbacks`]. Callbacks must not assume they
//! can re-enter the preprocessor.

use kiln_lex::{Symbol, Token};
use kiln_source::{FileId, Loc, Span};

use crate::include::{IncludeRequest, ResolvedInclude};
use crate::macros::MacroDef;

/// A change of the file being lexed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileChange {
    /// Lexing enters `file` (the main file, the predefines buffer or an
    /// `#include`d file). `from` is the `#include` directive's hash, if any.
    Enter {
        /// The entered buffer.
        file: FileId,
        /// Where it was included from.
        from: Option<Loc>,
    },
    /// Lexing returns to `file` after the end of an included file.
    Exit {
        /// The buffer lexing continues in.
        file: FileId,
        /// The buffer that ended.
        exited: FileId,
    },
}

/// How a pragma was written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PragmaIntroducer {
    /// `#pragma` directive.
    Directive,
    /// `_Pragma("...")` operator ([cpp.pragma.op]).
    Operator,
}

/// A pragma the core does not handle itself.
#[derive(Clone, Copy, Debug)]
pub struct PragmaInfo<'t> {
    /// How it was written.
    pub introducer: PragmaIntroducer,
    /// The `pragma` / `_Pragma` token.
    pub span: Span,
    /// The tokens after `pragma` up to the end of the line, unexpanded.
    pub tokens: &'t [Token],
}

/// Observer hooks. All default to no-ops.
pub trait PpCallbacks {
    /// Lexing moved to another file.
    fn file_changed(&mut self, change: FileChange) {
        let _ = change;
    }

    /// An `#include`-like directive was processed (`result` is `None` if
    /// the file was not found or was skipped by `#pragma once`).
    fn inclusion_directive(&mut self, req: &IncludeRequest, result: Option<&ResolvedInclude>) {
        let _ = (req, result);
    }

    /// A macro was defined (also called for predefines and `-D`).
    fn macro_defined(&mut self, name: Symbol, def: &MacroDef) {
        let _ = (name, def);
    }

    /// A macro was undefined by `#undef` (or `-U`); `def` is the removed
    /// definition, if there was one.
    fn macro_undefined(&mut self, name: Symbol, span: Span, def: Option<&MacroDef>) {
        let _ = (name, span, def);
    }

    /// A macro is about to be expanded. `range` covers the invocation
    /// (name through `)` for function-like macros).
    fn macro_expands(&mut self, name: &Token, def: &MacroDef, range: Span) {
        let _ = (name, def, range);
    }

    /// A pragma the core does not consume itself (everything except
    /// `once`, `push_macro`, `pop_macro`, `poison`, `GCC poison` and
    /// `GCC system_header`). Return `true` to consume it; otherwise it is
    /// passed through to the token stream (see
    /// [`crate::Marker::Pragma`]).
    fn pragma(&mut self, info: &PragmaInfo<'_>) -> bool {
        let _ = info;
        false
    }

    /// `#ident` / `#sccs` with its string.
    fn ident(&mut self, span: Span, text: &str) {
        let _ = (span, text);
    }

    /// A group skipped by a conditional directive: from the end of the
    /// directive line that started skipping to the `#` of the directive
    /// that ended it.
    fn source_range_skipped(&mut self, range: Span) {
        let _ = range;
    }
}

/// The callbacks installed by default: none.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoCallbacks;

impl PpCallbacks for NoCallbacks {}
