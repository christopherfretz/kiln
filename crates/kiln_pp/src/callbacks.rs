//! Observer hooks ([`PpCallbacks`]): clang's `PPCallbacks`.
//!
//! Every method has a no-op default. Install one observer with
//! [`crate::Preprocessor::set_callbacks`], or add any number with
//! [`crate::Preprocessor::add_callbacks`]: every observer sees every
//! event, in registration order. Callbacks must not assume they can
//! re-enter the preprocessor.

use kiln_lex::{Symbol, Token};
use kiln_source::{FileId, Loc, Span};

use crate::include::{IncludeRequest, ResolvedInclude};
use crate::macros::MacroDef;

/// A change of the file being lexed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileChange {
    /// Lexing enters `file` (the main file, the predefines buffer or an
    /// `#include`d file).
    Enter {
        /// The entered buffer.
        file: FileId,
        /// The header-name token of the `#include` that entered it, if any
        /// (clang's include location).
        from: Option<Loc>,
        /// The entered buffer is a system header.
        system: bool,
        /// The entered buffer is an `extern "C"` system header (found in a
        /// `-internal-externc-isystem` directory; line-marker flag 4).
        extern_c: bool,
    },
    /// Lexing returns to `file` after the end of an included file.
    Exit {
        /// The buffer lexing continues in.
        file: FileId,
        /// The buffer that ended.
        exited: FileId,
        /// Where lexing resumes in `file`: the start of the line after the
        /// `#include`.
        resume: Loc,
        /// `file` is a system header.
        system: bool,
        /// `file` is an `extern "C"` system header.
        extern_c: bool,
    },
    /// A `#line` directive or line marker ([cpp.line]) changed the presumed
    /// line (and maybe name) from `at` on.
    Rename {
        /// The current buffer.
        file: FileId,
        /// The start of the line after the directive.
        at: Loc,
        /// `file` is (now) a system header.
        system: bool,
        /// `file` is (now) an `extern "C"` system header (flag 4).
        extern_c: bool,
        /// A line marker with flag 1 (entering an include) or 2 (returning
        /// from one): clang reports these as `EnterFile` / `ExitFile`.
        flag: Option<u8>,
    },
    /// `#pragma GCC system_header` made the rest of `file` a system header.
    SystemHeader {
        /// The current buffer.
        file: FileId,
        /// The `system_header` token.
        at: Loc,
    },
}

/// The kind of a message pragma.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PragmaMessageKind {
    /// `#pragma message(...)`.
    Message,
    /// `#pragma GCC warning "..."`.
    Warning,
    /// `#pragma GCC error "..."`.
    Error,
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

/// A module file read for an import ([`PpCallbacks::module_file_read`]).
#[derive(Clone, Copy, Debug)]
pub struct ModuleFileRead<'a> {
    /// The module file as clang names it in a dependency file: as given
    /// by `-fmodule-file=<name>=<path>` (or recorded by the module file
    /// that imports it), absolute when found in a
    /// `-fprebuilt-module-path` or the toolchain's module cache.
    pub path: &'a str,
    /// The module (`M`, `M:P`).
    pub module: &'a str,
    /// For a module file from the toolchain's `std` module cache
    /// (spec/modules.md §11.12): the files its build read (its libc++
    /// source first), the same on a hit and on a miss. `None` otherwise.
    pub cache_inputs: Option<&'a [String]>,
}

/// Observer hooks. All default to no-ops.
pub trait PpCallbacks {
    /// Lexing moved to another file.
    fn file_changed(&mut self, change: FileChange) {
        let _ = change;
    }

    /// An `#include`-like directive was processed (`result` is `None` if
    /// the file was not found). [`PpCallbacks::file_changed`] follows when
    /// the file is entered, [`PpCallbacks::file_skipped`] when it is not.
    fn inclusion_directive(&mut self, req: &IncludeRequest, result: Option<&ResolvedInclude>) {
        let _ = (req, result);
    }

    /// An `#include` found `result` but did not enter it: `#pragma once`,
    /// `#import`, or the multiple-include optimization (clang
    /// `FileSkipped`). `system` is the characteristic the file would have
    /// been entered with (its directory's or its includer's).
    fn file_skipped(&mut self, result: &ResolvedInclude, system: bool) {
        let _ = (result, system);
    }

    /// A module file was read for an import, before its unit is read
    /// (clang's `visitModuleFile`; spec/driver.md "Dependency output").
    fn module_file_read(&mut self, read: &ModuleFileRead<'_>) {
        let _ = read;
    }

    /// An imported unit begins (`begin`) or ended. `from_module_file`:
    /// its text comes from a module file's record (spec/modules.md
    /// §11.2), so the files entered, skipped or not found until it ends
    /// are the module file's inputs, not this compile's (clang lists no
    /// input file of an explicitly loaded module). Units nest.
    fn module_unit(&mut self, begin: bool, from_module_file: bool) {
        let _ = (begin, from_module_file);
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

    /// A pragma the core does not consume itself. The core consumes
    /// `once`, `push_macro`, `pop_macro`, `mark`, `region`, `endregion`,
    /// `GCC`/`clang` `poison`, `system_header` and `dependency`, and turns
    /// `message`, `GCC warning` and `GCC error` into
    /// [`crate::Marker::PragmaMessage`] (see
    /// [`pragma_message`](Self::pragma_message)); `GCC`/`clang`
    /// `diagnostic` is validated, then handed here like any other pragma.
    /// Return `true` to consume it; otherwise it is passed through to the
    /// token stream (see [`crate::Marker::Pragma`]). With several
    /// observers, each one is offered the pragma, and it is consumed when
    /// any of them returns `true`.
    fn pragma(&mut self, info: &PragmaInfo<'_>) -> bool {
        let _ = info;
        false
    }

    /// `#pragma message`, `#pragma GCC warning` or `#pragma GCC error`
    /// (already diagnosed by the core). `namespace` is `Some("GCC")` for
    /// the GCC forms; `text` is the message with escapes resolved.
    fn pragma_message(
        &mut self,
        span: Span,
        namespace: Option<&str>,
        kind: PragmaMessageKind,
        text: &str,
    ) {
        let _ = (span, namespace, kind, text);
    }

    /// `#ident` / `#sccs` with its string.
    fn ident(&mut self, span: Span, text: &str) {
        let _ = (span, text);
    }

    /// A group skipped by a conditional directive (clang
    /// `SourceRangeSkipped`): from the `#` of the directive that started
    /// skipping to the end of the directive line that ended it (just
    /// past its last character, before the newline; for a group ended
    /// by an entered `#elif`, wherever lexing resumed after it).
    fn source_range_skipped(&mut self, range: Span) {
        let _ = range;
    }
}

/// The callbacks installed by default: none.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoCallbacks;

impl PpCallbacks for NoCallbacks {}

/// Two observers called in order: how
/// [`crate::Preprocessor::add_callbacks`] chains a second and later
/// observer onto those installed (a third makes `Chain(Chain(a, b), c)`),
/// so that with none or one observer the preprocessor makes exactly the
/// calls it made before there could be several.
pub(crate) struct Chain<'d>(pub Box<dyn PpCallbacks + 'd>, pub Box<dyn PpCallbacks + 'd>);

impl PpCallbacks for Chain<'_> {
    fn file_changed(&mut self, change: FileChange) {
        self.0.file_changed(change);
        self.1.file_changed(change);
    }

    fn inclusion_directive(&mut self, req: &IncludeRequest, result: Option<&ResolvedInclude>) {
        self.0.inclusion_directive(req, result);
        self.1.inclusion_directive(req, result);
    }

    fn file_skipped(&mut self, result: &ResolvedInclude, system: bool) {
        self.0.file_skipped(result, system);
        self.1.file_skipped(result, system);
    }

    fn module_file_read(&mut self, read: &ModuleFileRead<'_>) {
        self.0.module_file_read(read);
        self.1.module_file_read(read);
    }

    fn module_unit(&mut self, begin: bool, from_module_file: bool) {
        self.0.module_unit(begin, from_module_file);
        self.1.module_unit(begin, from_module_file);
    }

    fn macro_defined(&mut self, name: Symbol, def: &MacroDef) {
        self.0.macro_defined(name, def);
        self.1.macro_defined(name, def);
    }

    fn macro_undefined(&mut self, name: Symbol, span: Span, def: Option<&MacroDef>) {
        self.0.macro_undefined(name, span, def);
        self.1.macro_undefined(name, span, def);
    }

    fn macro_expands(&mut self, name: &Token, def: &MacroDef, range: Span) {
        self.0.macro_expands(name, def, range);
        self.1.macro_expands(name, def, range);
    }

    fn pragma(&mut self, info: &PragmaInfo<'_>) -> bool {
        // Both see it, whatever the first one answers.
        let first = self.0.pragma(info);
        self.1.pragma(info) | first
    }

    fn pragma_message(
        &mut self,
        span: Span,
        namespace: Option<&str>,
        kind: PragmaMessageKind,
        text: &str,
    ) {
        self.0.pragma_message(span, namespace, kind, text);
        self.1.pragma_message(span, namespace, kind, text);
    }

    fn ident(&mut self, span: Span, text: &str) {
        self.0.ident(span, text);
        self.1.ident(span, text);
    }

    fn source_range_skipped(&mut self, range: Span) {
        self.0.source_range_skipped(range);
        self.1.source_range_skipped(range);
    }
}
