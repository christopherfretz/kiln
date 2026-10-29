//! The preprocessor driver: the include / macro-expansion stack, the main
//! lexing loop ([cpp.pre], [cpp.rescan]) and the public API.

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use kiln_diag::{Diag, DiagEngine, DiagId, SCRATCH_BUFFER_NAME};
use kiln_lex::{Interner, LexOptions, Lexer, Punct, Symbol, TokFlags, TokKind, Token};
use kiln_source::{FileId, Loc, SourceManager, Span};

use crate::builtin::{FeatureQueries, NoFeatures};
use crate::callbacks::{FileChange, NoCallbacks, PpCallbacks, PragmaIntroducer, PragmaMessageKind};
use crate::cond::IfEvaluator;
use crate::ifeval::IfEval;
use crate::include::{IncludeResolver, NoIncludes};
use crate::macros::{MacroDef, MacroTable, SymMap};
use crate::options::PpOptions;
use crate::tok::PpTok;

/// One item of the preprocessed stream: a token, or an out-of-band event
/// at that point of the stream.
#[derive(Clone, Debug)]
pub enum PpItem {
    /// A fully macro-expanded token (phase 4 output).
    Token(Token),
    /// An event between tokens (pragma, file change, ...).
    Marker(Marker),
}

/// An out-of-band event in the token stream (what clang's `-E` printer
/// learns through callbacks and annotation tokens).
#[derive(Clone, Debug)]
pub enum Marker {
    /// A pragma the core did not consume (passed through to phase 7, or
    /// printed by `-E`).
    Pragma {
        /// How it was written.
        introducer: PragmaIntroducer,
        /// The `pragma` / `_Pragma` token.
        span: Span,
        /// The tokens after `pragma`, unexpanded, without the `Eod`.
        tokens: Vec<Token>,
    },
    /// `#pragma message` / `GCC warning` / `GCC error` (already diagnosed).
    PragmaMessage {
        /// The `message` / `warning` / `error` token.
        span: Span,
        /// `Some("GCC")` for the GCC forms.
        namespace: Option<String>,
        /// Which pragma.
        kind: PragmaMessageKind,
        /// The message, escapes resolved.
        text: String,
    },
    /// `#ident "text"` / `#sccs "text"`.
    Ident {
        /// The directive name.
        span: Span,
        /// The string literal as written.
        text: String,
    },
    /// Lexing moved to another file.
    FileChange(FileChange),
    /// A `module` / `import` directive ([cpp.module], [cpp.import]) starts
    /// here; its tokens (macro-expanded, starting with the `export` /
    /// `module` / `import` keyword) follow, ending with `;`.
    Module {
        /// The first token of the directive.
        span: Span,
        /// Starts with `export`.
        export: bool,
        /// An `import` (else a `module`) directive.
        import: bool,
    },
    /// An `#embed` directive ([cpp.embed]) produced the tokens that follow:
    /// its `prefix`, the bytes (see [`Preprocessor::is_embed_byte`]) and its
    /// `suffix` (or its `if_empty` tokens).
    Embed {
        /// The `embed` directive-name token.
        span: Span,
    },
}

/// One open `#if` group.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cond {
    /// The `#if` / `#ifdef` / `#ifndef` directive name.
    pub(crate) span: Span,
    /// Some group of this conditional was (or is being) included.
    pub(crate) found_non_skip: bool,
    /// `#else` was seen.
    pub(crate) found_else: bool,
    /// The whole conditional is inside a skipped group.
    pub(crate) was_skipping: bool,
}

/// The include-guard detector of one file (clang `MultipleIncludeOpt`):
/// a file whose only tokens are inside a top-level `#ifndef X` /
/// `#if !defined(X)` group is skipped when re-included while `X` is
/// defined.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MiOpt {
    /// Tokens were read outside the candidate guard.
    pub(crate) read_any: bool,
    /// The candidate controlling macro.
    pub(crate) the_macro: Option<Symbol>,
}

impl MiOpt {
    /// Any token (directive tokens included) was lexed.
    pub(crate) fn read_token(&mut self) {
        self.read_any = true;
    }

    /// A conditional other than the guard at the top level.
    pub(crate) fn invalidate(&mut self) {
        self.read_any = true;
        self.the_macro = None;
    }

    /// A top-level `#ifndef m` with nothing before it.
    pub(crate) fn enter_ifndef(&mut self, m: Symbol) {
        self.read_any = true;
        if self.the_macro.is_some() {
            self.invalidate();
        } else {
            self.the_macro = Some(m);
        }
    }

    /// The top-level `#endif`.
    pub(crate) fn exit_top_level(&mut self) {
        if self.the_macro.is_some() {
            self.read_any = false;
        } else {
            self.invalidate();
        }
    }

    /// The controlling macro, at the end of the file.
    pub(crate) fn controlling_macro(self) -> Option<Symbol> {
        if self.read_any { None } else { self.the_macro }
    }
}

/// A buffer being lexed.
pub(crate) struct FileFrame<'a> {
    pub(crate) lx: Lexer<'a>,
    pub(crate) file: FileId,
    /// Location of byte 0 of this inclusion of `file`: the buffer's own
    /// range for the main file and virtual buffers, a fresh
    /// `SourceManager::add_inclusion` range for an `#include`d file.
    pub(crate) base: Loc,
    /// Tokens lexed ahead (by the `(` lookahead), last = next.
    pub(crate) back: Vec<Token>,
    /// Start-of-line / leading-space flags propagated from an ended macro
    /// expansion, `ORed` into the next token.
    pub(crate) pend: TokFlags,
    pub(crate) conds: Vec<Cond>,
    /// Search-path index the file was found at (for `#include_next`).
    pub(crate) lookup: Option<u32>,
    pub(crate) system: bool,
    /// An `extern "C"` system header (implies `system`).
    pub(crate) extern_c: bool,
    /// Line markers with flag 1 not yet closed by flag 2 in this file
    /// (clang's presumed include stack, as far as it is checked).
    pub(crate) marker_depth: u32,
    /// Include-guard detection.
    pub(crate) mi: MiOpt,
}

/// A macro expansion or a token stream being read.
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct MacroFrame {
    pub(crate) toks: Vec<PpTok>,
    pub(crate) idx: usize,
    /// The macro (`None` for a token stream: pre-expanded argument, pushed
    /// back tokens).
    pub(crate) def: Option<Rc<MacroDef>>,
    pub(crate) at_sol: bool,
    pub(crate) has_ls: bool,
    /// The expansion ended with an empty argument preceded by whitespace.
    pub(crate) next_gets_space: bool,
    /// Base of the expansion entry of the replacement list.
    pub(crate) exp_base: Option<Loc>,
    /// The invocation range.
    pub(crate) exp: Span,
    /// The macro-name token of the invocation.
    pub(crate) name_span: Span,
    /// Identifiers from this stream are never macro-expanded.
    pub(crate) no_expand: bool,
}

pub(crate) enum Frame<'a> {
    File(Box<FileFrame<'a>>),
    Macro(MacroFrame),
}

/// Pre-interned identifiers.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Syms {
    pub(crate) defined: Symbol,
    pub(crate) va_args: Symbol,
    pub(crate) va_opt: Symbol,
    pub(crate) module: Symbol,
    pub(crate) import: Symbol,
    pub(crate) export: Symbol,
}

/// The preprocessor (translation phase 4, [cpp]).
///
/// Produces fully macro-expanded tokens with [`Preprocessor::next`]. Token
/// spans of expanded tokens are macro locations: the source manager maps
/// them to their spelling (in the `#define`, the argument, or the scratch
/// buffer) and to their expansion (the invocation).
#[allow(clippy::struct_excessive_bools)]
pub struct Preprocessor<'a, 'd> {
    pub(crate) sm: &'a SourceManager,
    pub(crate) diags: &'d DiagEngine<'a>,
    pub(crate) interner: &'a Interner,
    pub(crate) opts: PpOptions,
    pub(crate) lex_opts: LexOptions,
    pub(crate) macros: MacroTable,
    pub(crate) frames: Vec<Frame<'a>>,
    pub(crate) resolver: Box<dyn IncludeResolver + 'd>,
    pub(crate) evaluator: Option<Box<dyn IfEvaluator + 'd>>,
    pub(crate) features: Box<dyn FeatureQueries + 'd>,
    pub(crate) callbacks: Box<dyn PpCallbacks + 'd>,
    pub(crate) markers: VecDeque<Marker>,
    /// A token lexed while markers were queued (returned after them).
    pub(crate) pending: Option<Token>,
    /// Scratch buffers: one virtual buffer per distinct string.
    pub(crate) scratch: HashMap<Box<str>, FileId>,
    /// Recycled token buffers.
    pub(crate) pool: Vec<Vec<PpTok>>,
    /// Spare `bounds` / `pre` vectors of macro arguments (see `read_args`).
    pub(crate) arg_pool: Vec<crate::expand::ArgVecs>,
    pub(crate) syms: Syms,
    /// Directive-name classification by symbol.
    pub(crate) dirs: crate::directive::DirTable,
    pub(crate) counter: u32,
    pub(crate) once_files: HashSet<FileId>,
    /// Files with a detected include guard, and its macro.
    pub(crate) guards: HashMap<FileId, Symbol>,
    /// The `X` of a `#if !defined(X)` just evaluated by `eval_if`.
    pub(crate) if_ndef: Option<Symbol>,
    pub(crate) poisoned: SymMap<Span>,
    pub(crate) pushed: SymMap<Vec<Option<Rc<MacroDef>>>>,
    pub(crate) entered: HashSet<FileId>,
    pub(crate) include_depth: u32,
    pub(crate) main_file: Option<FileId>,
    /// Lex without macro expansion (clang `DisableMacroExpansion`).
    pub(crate) disable_expand: bool,
    /// Reading the arguments of this macro (name, name token).
    pub(crate) in_args: Option<(Symbol, Span)>,
    /// Pre-expanding a macro argument ([cpp.subst]/1).
    pub(crate) arg_preexpand: u32,
    /// Evaluating a `#if` / `#elif` expression.
    pub(crate) parsing_if: bool,
    /// `__VA_ARGS__` / `__VA_OPT__` are allowed (variadic `#define` body).
    pub(crate) va_scope: bool,
    pub(crate) date_time: Option<(String, String)>,
    /// Reused buffer for token pasting.
    pub(crate) paste_buf: String,
    /// A directive line is being read (the lexer yields `Eod`).
    pub(crate) in_directive: bool,
    /// The directive's `Eod`, once read: returned again for every
    /// further read until the directive is finished.
    pub(crate) dir_eod: Option<Token>,
    /// Lexer diagnostics are dropped (skipped groups).
    pub(crate) quiet_lex: bool,
    /// Depth of `#pragma GCC/clang diagnostic push` (for "cannot pop").
    pub(crate) diag_pushes: u32,
    /// Scratch buffers holding `#embed` byte spellings.
    pub(crate) embed_bufs: HashSet<FileId>,
}

impl std::fmt::Debug for Preprocessor<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preprocessor")
            .field("macros", &self.macros.len())
            .field("depth", &self.frames.len())
            .finish_non_exhaustive()
    }
}

impl<'a, 'd> Preprocessor<'a, 'd> {
    /// A preprocessor with no main file yet. Builtin macros are defined
    /// now; predefines and `-D` / `-U` are applied by
    /// [`enter_main_file`](Self::enter_main_file).
    pub fn new(
        sm: &'a SourceManager,
        diags: &'d DiagEngine<'a>,
        interner: &'a Interner,
        opts: PpOptions,
    ) -> Self {
        let syms = Syms {
            defined: interner.intern("defined"),
            va_args: interner.intern("__VA_ARGS__"),
            va_opt: interner.intern("__VA_OPT__"),
            module: interner.intern("module"),
            import: interner.intern("import"),
            export: interner.intern("export"),
        };
        let lex_opts = opts.lex_options();
        let mut pp = Preprocessor {
            sm,
            diags,
            interner,
            opts,
            lex_opts,
            macros: MacroTable::default(),
            frames: Vec::new(),
            resolver: Box::new(NoIncludes),
            evaluator: Some(Box::new(IfEval::default())),
            features: Box::new(NoFeatures),
            callbacks: Box::new(NoCallbacks),
            markers: VecDeque::new(),
            pending: None,
            scratch: HashMap::new(),
            pool: Vec::new(),
            arg_pool: Vec::new(),
            syms,
            dirs: crate::directive::DirTable::new(interner),
            counter: 0,
            once_files: HashSet::new(),
            guards: HashMap::new(),
            if_ndef: None,
            poisoned: SymMap::default(),
            pushed: SymMap::default(),
            entered: HashSet::new(),
            include_depth: 0,
            main_file: None,
            disable_expand: false,
            in_args: None,
            arg_preexpand: 0,
            parsing_if: false,
            va_scope: false,
            date_time: None,
            paste_buf: String::new(),
            in_directive: false,
            dir_eod: None,
            quiet_lex: false,
            diag_pushes: 0,
            embed_bufs: HashSet::new(),
        };
        pp.define_builtins();
        pp
    }

    /// Installs the `#include` / `#embed` / `__has_include` resolver
    /// (default: [`NoIncludes`]).
    pub fn set_include_resolver(&mut self, r: Box<dyn IncludeResolver + 'd>) {
        self.resolver = r;
    }

    /// Installs the `#if` evaluator (default: [`IfEval::default`], the
    /// x86-64 Linux character types).
    pub fn set_if_evaluator(&mut self, e: Box<dyn IfEvaluator + 'd>) {
        self.evaluator = Some(e);
    }

    /// Installs the feature-query hook (default: [`NoFeatures`]).
    pub fn set_feature_queries(&mut self, f: Box<dyn FeatureQueries + 'd>) {
        self.features = f;
    }

    /// Installs observer callbacks (default: [`NoCallbacks`]).
    pub fn set_callbacks(&mut self, c: Box<dyn PpCallbacks + 'd>) {
        self.callbacks = c;
    }

    /// The installed callbacks.
    pub fn callbacks(&mut self) -> &mut dyn PpCallbacks {
        &mut *self.callbacks
    }

    /// The macro table.
    pub fn macros(&self) -> &MacroTable {
        &self.macros
    }

    /// The source manager.
    pub fn source_manager(&self) -> &'a SourceManager {
        self.sm
    }

    /// The identifier interner.
    pub fn interner(&self) -> &'a Interner {
        self.interner
    }

    /// The options.
    pub fn options(&self) -> &PpOptions {
        &self.opts
    }

    /// The lexer options derived from the options.
    pub fn lex_options(&self) -> &LexOptions {
        &self.lex_opts
    }

    /// The main source file, once entered.
    pub fn main_file(&self) -> Option<FileId> {
        self.main_file
    }

    /// Enters `file` as the main source file: pushes it, then the
    /// predefines buffer (`<built-in>`: [`PpOptions::predefines`], then
    /// `-D` / `-U`) on top of it, so that the predefines are read first.
    pub fn enter_main_file(&mut self, file: FileId) {
        use std::fmt::Write as _;
        self.main_file = Some(file);
        self.push_file(file, None, None, None, false, false);
        let mut text = self.opts.predefines.clone();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        for (name, value) in &self.opts.defines {
            let _ = writeln!(text, "#define {name} {}", value.as_deref().unwrap_or("1"));
        }
        for name in &self.opts.undefines {
            let _ = writeln!(text, "#undef {name}");
        }
        if let Ok(b) = self.sm.add_virtual("<built-in>", text) {
            self.push_file(b, None, None, None, false, false);
        }
    }

    /// Pushes a file frame for `file` (reporting its invalid UTF-8 once).
    /// `inc` is the location of the header name of the `#include` that
    /// enters it (clang's include position); the frame then lexes in a
    /// fresh location range remembering it ([cpp.include]).
    pub(crate) fn push_file(
        &mut self,
        file: FileId,
        from: Option<Loc>,
        inc: Option<Loc>,
        lookup: Option<u32>,
        system: bool,
        extern_c: bool,
    ) {
        crate::profile::count!(files_entered);
        if self.entered.contains(&file) {
            crate::profile::count!(file_reentries);
        }
        // clang: the include position of a macro-expanded header name is
        // the end of its expansion. If location space is exhausted the
        // inclusion lexes in the buffer's own range (no include stack in
        // its diagnostics) rather than failing.
        let base = inc
            .and_then(|at| self.sm.add_inclusion(file, self.sm.expansion_loc(at)).ok())
            .unwrap_or_else(|| self.sm.loc(file, 0));
        if system {
            self.sm.set_file_kind(base, file_kind(system, extern_c));
        }
        if self.entered.insert(file) {
            for r in &self.sm.load_report(file).replacements {
                let at = base.with_offset(r.offset);
                Diag::new(DiagId::lex_invalid_utf8, Span::new(at, at.with_offset(3)))
                    .emit(self.diags);
            }
        }
        let lx = Lexer::new(self.sm, self.interner, file, base, self.lex_opts);
        self.frames.push(Frame::File(Box::new(FileFrame {
            lx,
            file,
            base,
            back: Vec::new(),
            pend: TokFlags::empty(),
            conds: Vec::new(),
            lookup,
            system,
            extern_c,
            marker_depth: 0,
            mi: MiOpt::default(),
        })));
        self.file_change(FileChange::Enter { file, from, system, extern_c });
    }

    /// Whether warnings located in system headers are dropped (no
    /// `-Wsystem-headers`): checks done only to warn can be skipped there.
    pub(crate) fn suppress_system_warnings(&self) -> bool {
        self.diags.suppress_system_warnings()
    }

    /// Records a file change (callback and marker).
    pub(crate) fn file_change(&mut self, c: FileChange) {
        self.callbacks.file_changed(c);
        self.markers.push_back(Marker::FileChange(c));
    }

    /// The next fully macro-expanded token; markers are dropped. Returns
    /// `Eof` (repeatedly) at the end of the main file.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Token {
        loop {
            if let PpItem::Token(t) = self.next_item() {
                return t;
            }
        }
    }

    /// The next token or marker. Markers come before the token that
    /// follows them in the source.
    pub fn next_item(&mut self) -> PpItem {
        if let Some(m) = self.markers.pop_front() {
            return PpItem::Marker(m);
        }
        if let Some(t) = self.pending.take() {
            return PpItem::Token(t);
        }
        let t = self.lex().tok;
        if let Some(m) = self.markers.pop_front() {
            self.pending = Some(t);
            return PpItem::Marker(m);
        }
        PpItem::Token(t)
    }

    pub(crate) fn emit(&self, d: Diag) {
        d.emit(self.diags);
    }

    pub(crate) fn enabled(&self, id: DiagId) -> bool {
        self.diags.is_enabled(id)
    }

    /// The top file frame.
    pub(crate) fn cur_file(&mut self) -> Option<&mut FileFrame<'a>> {
        self.frames.iter_mut().rev().find_map(|f| match f {
            Frame::File(f) => Some(&mut **f),
            Frame::Macro(_) => None,
        })
    }

    /// The top file frame (shared).
    pub(crate) fn cur_file_ref(&self) -> Option<&FileFrame<'a>> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::File(f) => Some(&**f),
            Frame::Macro(_) => None,
        })
    }

    /// Lexes one token from the top file frame (pushed-back tokens first),
    /// applying propagated flags and reporting lexer diagnostics. Comments
    /// are returned. Identifiers are checked for poisoning.
    ///
    /// Also returns whether the token physically starts its line (before
    /// propagated flags are applied): only such a `#` introduces a
    /// directive (clang `IsAtPhysicalStartOfLine`).
    pub(crate) fn lex_file_tok_phys(&mut self) -> (Token, bool) {
        let top = self.frames.len().wrapping_sub(1);
        if !matches!(self.frames.last(), Some(Frame::File(_))) {
            return (eof_tok(Span::INVALID), false);
        }
        let mut t = self.lex_raw_from(top);
        let phys = t.has(TokFlags::START_OF_LINE);
        let Some(Frame::File(f)) = self.frames.last_mut() else { unreachable!() };
        if !f.pend.is_empty() && !matches!(t.kind, TokKind::Eod | TokKind::Eof) {
            t.flags |= f.pend;
            f.pend = TokFlags::empty();
        }
        if let TokKind::Identifier(s) = t.kind {
            self.check_poison(s, t.span);
        }
        (t, phys)
    }

    /// The next token of file frame `i` (pushed-back tokens first),
    /// reporting lexer diagnostics; no flags are applied.
    pub(crate) fn lex_raw_from(&mut self, i: usize) -> Token {
        if self.in_directive
            && let Some(e) = self.dir_eod
        {
            return e;
        }
        let Frame::File(f) = &mut self.frames[i] else { unreachable!() };
        let t = if let Some(t) = f.back.pop() {
            t
        } else {
            crate::profile::count!(raw_tokens);
            let t = f.lx.next();
            if !f.lx.diagnostics().is_empty() {
                let ds = f.lx.take_diagnostics();
                if !self.quiet_lex {
                    for d in &ds {
                        if let Some(diag) = crate::diag_map::lex_diag(self.sm, d) {
                            self.emit(diag);
                        }
                    }
                }
            }
            t
        };
        if self.in_directive && t.kind == TokKind::Eod {
            self.dir_eod = Some(t);
        }
        t
    }

    /// Starts reading a directive line in the top file frame (after its
    /// `#`): the lexer yields `Eod` at the end of the line; comments are
    /// not returned.
    pub(crate) fn start_directive(&mut self) {
        if let Some(Frame::File(f)) = self.frames.last_mut() {
            f.lx.set_in_directive(true);
            f.lx.set_keep_comments(false);
        }
        self.in_directive = true;
        self.dir_eod = None;
    }

    /// Discards the rest of the directive line and ends the directive;
    /// returns its `Eod` (clang `DiscardUntilEndOfDirective`). A no-op
    /// returning `Eod` at an invalid span when no directive is active.
    pub(crate) fn finish_directive(&mut self) -> Token {
        if !self.in_directive {
            return Token { kind: TokKind::Eod, span: Span::INVALID, flags: TokFlags::empty() };
        }
        // In a skipped group (no diagnostics), a plain rest of the line is
        // discarded without forming its tokens.
        if self.quiet_lex
            && self.dir_eod.is_none()
            && let Some(Frame::File(f)) = self.frames.last_mut()
            && f.back.is_empty()
            && f.lx.skip_plain_line_rest()
        {
            f.mi.read_token();
        }
        let eod = loop {
            if let Some(e) = self.dir_eod {
                break e;
            }
            let t = self.lex_unexpanded();
            if t.tok.kind == TokKind::Eod {
                break t.tok;
            }
            if t.tok.kind == TokKind::Eof {
                break Token { kind: TokKind::Eod, span: t.tok.span, flags: TokFlags::empty() };
            }
        };
        // Macro frames pushed while reading the directive have ended.
        while matches!(self.frames.last(), Some(Frame::Macro(_))) {
            self.pop_macro();
        }
        self.in_directive = false;
        self.dir_eod = None;
        let keep = self.lex_opts.keep_comments;
        if let Some(Frame::File(f)) = self.frames.last_mut() {
            f.lx.set_in_directive(false);
            f.lx.set_mode(kiln_lex::LexMode::Normal);
            f.lx.set_keep_comments(keep);
            f.pend = TokFlags::empty();
        }
        eod
    }

    /// Pushes `t` back onto the top file frame.
    pub(crate) fn unlex_file_tok(&mut self, t: Token) {
        if let Some(Frame::File(f)) = self.frames.last_mut() {
            f.back.push(t);
        }
    }

    /// `#pragma poison` and `__VA_ARGS__` / `__VA_OPT__` outside a
    /// variadic definition (clang `HandlePoisonedIdentifier`).
    pub(crate) fn check_poison(&mut self, s: Symbol, span: Span) {
        if self.va_scope || self.quiet_lex {
            return;
        }
        if s == self.syms.va_args {
            self.emit(Diag::new(DiagId::pp_va_args_misuse, span));
        } else if s == self.syms.va_opt {
            self.emit(Diag::new(DiagId::pp_va_opt_misuse, span));
        } else if !self.poisoned.is_empty() && self.poisoned.contains_key(&s) {
            self.emit(Diag::new(DiagId::pp_poisoned_identifier, span));
        }
    }

    /// The main loop: the next token, macro-expanded unless
    /// `disable_expand`, with directives executed.
    pub(crate) fn lex(&mut self) -> PpTok {
        loop {
            if self.in_directive
                && let Some(e) = self.dir_eod
            {
                return PpTok::new(e);
            }
            match self.frames.last_mut() {
                None => return PpTok::new(eof_tok(Span::INVALID)),
                Some(Frame::Macro(m)) => {
                    if m.idx >= m.toks.len() {
                        self.pop_macro();
                        continue;
                    }
                    let t = self.macro_next();
                    if t.tok.kind == TokKind::Eod {
                        // The end of a `_Pragma` string (see `pragma.rs`).
                        if self.in_directive {
                            self.dir_eod = Some(t.tok);
                        }
                        return t;
                    }
                    if let Some(s) = t.ident() {
                        let no_expand = match self.frames.last() {
                            Some(Frame::Macro(m)) => m.no_expand,
                            _ => false,
                        };
                        if !self.disable_expand && !no_expand && self.macros.is_defined(s) {
                            match self.handle_identifier(t) {
                                Some(t) => return t,
                                None => continue,
                            }
                        }
                    }
                    return t;
                }
                Some(Frame::File(_)) => {
                    let (t, phys_sol) = self.lex_file_tok_phys();
                    if (!phys_sol || t.kind != TokKind::Punct(Punct::Hash))
                        && !matches!(t.kind, TokKind::Comment | TokKind::Eod | TokKind::Eof)
                        && let Some(Frame::File(f)) = self.frames.last_mut()
                    {
                        f.mi.read_token();
                    }
                    match t.kind {
                        TokKind::Punct(Punct::Hash) if phys_sol => {
                            self.handle_directive(t);
                        }
                        TokKind::Eof => {
                            if self.end_of_file(t) {
                                return PpTok::new(t);
                            }
                        }
                        TokKind::Identifier(s) => {
                            if phys_sol
                                && (s == self.syms.module
                                    || s == self.syms.import
                                    || s == self.syms.export)
                                && !self.disable_expand
                            {
                                self.maybe_module_directive(t, s);
                            }
                            if !self.disable_expand && self.macros.is_defined(s) {
                                match self.handle_identifier(PpTok::new(t)) {
                                    Some(t) => return t,
                                    None => continue,
                                }
                            }
                            return PpTok::new(t);
                        }
                        _ => return PpTok::new(t),
                    }
                }
            }
        }
    }

    /// Lexes with macro expansion disabled (clang `LexUnexpandedToken`).
    pub(crate) fn lex_unexpanded(&mut self) -> PpTok {
        let old = std::mem::replace(&mut self.disable_expand, true);
        let t = self.lex();
        self.disable_expand = old;
        t
    }

    /// The next token of the top macro frame (clang `TokenLexer::Lex`
    /// without the identifier handling): pastes, then applies the frame's
    /// start-of-line / leading-space flags.
    fn macro_next(&mut self) -> PpTok {
        let Some(Frame::Macro(m)) = self.frames.last_mut() else { unreachable!() };
        let first = m.idx == 0;
        let mut t = m.toks[m.idx];
        m.idx += 1;
        let paste = m.def.is_some() && m.toks.get(m.idx).is_some_and(PpTok::is_paste_op);
        if paste {
            t = self.paste_in_frame(t);
        }
        let Some(Frame::Macro(m)) = self.frames.last_mut() else { unreachable!() };
        if first {
            t.set_flag(TokFlags::START_OF_LINE, m.at_sol);
            t.set_flag(TokFlags::LEADING_SPACE, m.has_ls);
        } else {
            if m.at_sol {
                t.set_flag(TokFlags::START_OF_LINE, true);
            }
            if m.has_ls {
                t.set_flag(TokFlags::LEADING_SPACE, true);
            }
        }
        m.at_sol = false;
        m.has_ls = false;
        if paste && let Some(s) = t.ident() {
            self.check_poison(s, t.tok.span);
        }
        t
    }

    /// Pops the ended top macro frame, re-enabling its macro and
    /// propagating its line-start / leading-space state to what is below
    /// (clang `HandleEndOfTokenLexer` + `PropagateLineStartLeadingSpaceInfo`).
    pub(crate) fn pop_macro(&mut self) {
        let Some(Frame::Macro(m)) = self.frames.pop() else { unreachable!() };
        if let Some(d) = &m.def {
            d.enabled.set(true);
        }
        let sol = m.at_sol;
        let ls = m.has_ls || m.next_gets_space;
        self.give_buf(m.toks);
        self.propagate(sol, ls);
    }

    /// Gives the next token lexed from the top frame these flags.
    pub(crate) fn propagate(&mut self, sol: bool, ls: bool) {
        match self.frames.last_mut() {
            Some(Frame::Macro(m)) => {
                m.at_sol = sol;
                m.has_ls = ls;
            }
            Some(Frame::File(f)) => {
                if sol {
                    f.pend.insert(TokFlags::START_OF_LINE);
                }
                if ls {
                    f.pend.insert(TokFlags::LEADING_SPACE);
                }
            }
            None => {}
        }
    }

    /// Is the next token `(`? (clang `isNextPPTokenLParen`.) Looks through
    /// ended macro frames; a file frame is read one token ahead (the token
    /// is pushed back). The end of a file is never looked through.
    pub(crate) fn peek_lparen(&mut self) -> bool {
        for i in (0..self.frames.len()).rev() {
            match &self.frames[i] {
                Frame::Macro(m) => {
                    if let Some(t) = m.toks.get(m.idx) {
                        return t.is_punct(Punct::LParen);
                    }
                }
                Frame::File(_) => {
                    let first = self.lex_raw_from(i);
                    let mut t = first;
                    let mut seen: Vec<Token> = Vec::new();
                    while t.kind == TokKind::Comment && !self.opts.keep_macro_comments {
                        seen.push(t);
                        t = self.lex_raw_from(i);
                    }
                    let Frame::File(f) = &mut self.frames[i] else { unreachable!() };
                    if seen.is_empty() {
                        f.back.push(first);
                    } else {
                        f.back.push(t);
                        f.back.extend(seen.into_iter().rev());
                    }
                    return t.kind == TokKind::Punct(Punct::LParen);
                }
            }
        }
        false
    }

    /// The file frame on top reached `Eof`. Reports unterminated
    /// conditionals; returns true if this is the end of the main file
    /// (`t` is returned), else pops the file and returns false.
    fn end_of_file(&mut self, t: Token) -> bool {
        let conds = match self.frames.last_mut() {
            Some(Frame::File(f)) => std::mem::take(&mut f.conds),
            _ => Vec::new(),
        };
        for c in conds.iter().rev() {
            self.emit(Diag::new(DiagId::pp_unterminated_conditional, c.span));
        }
        if self.frames.len() <= 1 {
            let _ = t;
            return true;
        }
        let Some(Frame::File(f)) = self.frames.pop() else { unreachable!() };
        let exited = f.file;
        if conds.is_empty()
            && let Some(m) = f.mi.controlling_macro()
        {
            self.guards.insert(exited, m);
        }
        self.include_depth = self.include_depth.saturating_sub(1);
        if let Some(Frame::File(below)) = self.frames.last() {
            let (file, base, system, extern_c) =
                (below.file, below.base, below.system, below.extern_c);
            // The includer resumes after the newline ending the `#include`
            // (clang's lexer has consumed it with the `eod`).
            let mut off = below.lx.offset();
            let rest = self.sm.text(file).as_bytes().get(off as usize..).unwrap_or_default();
            if rest.starts_with(b"\r\n") {
                off += 2;
            } else if rest.starts_with(b"\n") || rest.starts_with(b"\r") {
                off += 1;
            }
            let resume = base.with_offset(off);
            self.file_change(FileChange::Exit { file, exited, resume, system, extern_c });
        }
        false
    }

    /// A pooled, empty token buffer.
    pub(crate) fn take_buf(&mut self) -> Vec<PpTok> {
        self.pool.pop().unwrap_or_else(|| {
            crate::profile::count!(buf_new);
            Vec::new()
        })
    }

    /// Returns a token buffer to the pool.
    pub(crate) fn give_buf(&mut self, mut v: Vec<PpTok>) {
        v.clear();
        if self.pool.len() < 64 {
            self.pool.push(v);
        } else {
            crate::profile::count!(buf_dropped);
        }
    }

    /// Pushes a token stream (no macro; clang `EnterTokenStream`).
    pub(crate) fn push_stream(&mut self, toks: Vec<PpTok>, no_expand: bool) {
        let (at_sol, has_ls) = match toks.first() {
            Some(t) => (t.has(TokFlags::START_OF_LINE), t.has(TokFlags::LEADING_SPACE)),
            None => (false, false),
        };
        self.frames.push(Frame::Macro(MacroFrame {
            toks,
            idx: 0,
            def: None,
            at_sol,
            has_ls,
            next_gets_space: false,
            exp_base: None,
            exp: Span::INVALID,
            name_span: Span::INVALID,
            no_expand,
        }));
    }

    /// The location of a copy of `text` in a scratch buffer (one buffer
    /// per distinct string, named `<scratch space>`).
    pub(crate) fn scratch_loc(&mut self, text: &str) -> Option<(FileId, Loc)> {
        if let Some(&f) = self.scratch.get(text) {
            return Some((f, self.sm.loc(f, 0)));
        }
        let mut buf = String::with_capacity(text.len() + 1);
        buf.push_str(text);
        buf.push('\n');
        let f = self.sm.add_virtual(SCRATCH_BUFFER_NAME, buf).ok()?;
        self.scratch.insert(text.into(), f);
        Some((f, self.sm.loc(f, 0)))
    }

    /// The base location of the source-manager entry containing `loc`.
    pub(crate) fn entry_of(&self, loc: Loc) -> Option<Loc> {
        crate::profile::count!(entry_of);
        self.sm.entry_bounds(loc).map(|(base, _)| base)
    }

    /// The distance between the spellings of `lo` and `hi` when their
    /// spelling chains end in the same file at the same depth (`None`
    /// otherwise). O(1) when both are in the same entry.
    pub(crate) fn loc_diff(&self, lo: Loc, hi: Loc) -> Option<u32> {
        crate::profile::count!(loc_diff);
        if let Some(d) = self.sm.same_entry_distance(lo, hi) {
            return Some(d);
        }
        crate::profile::count!(loc_diff_slow);
        self.loc_diff_slow(lo, hi)
    }

    /// [`loc_diff`](Self::loc_diff) without the same-entry fast path:
    /// walks both spelling chains in lockstep down to a file (linear in
    /// the expansion depth). Kept for locations in different entries and
    /// as the reference the fast path is tested against.
    pub(crate) fn loc_diff_slow(&self, lo: Loc, hi: Loc) -> Option<u32> {
        let (mut a, mut b) = (lo, hi);
        while a.is_macro() && b.is_macro() {
            crate::profile::count!(loc_diff_steps);
            a = self.sm.immediate_spelling_loc(a);
            b = self.sm.immediate_spelling_loc(b);
        }
        if a.is_macro() || b.is_macro() {
            return None;
        }
        let (fa, oa) = self.sm.offset_of(a)?;
        let (fb, ob) = self.sm.offset_of(b)?;
        (fa == fb && ob >= oa).then(|| ob - oa)
    }

    /// The spelling of a token (`""` for `Eod` / `Eof`).
    pub(crate) fn spelling(&self, t: &Token) -> std::borrow::Cow<'a, str> {
        if let TokKind::Identifier(s) = t.kind
            && !t.flags.contains(TokFlags::NEEDS_CLEANING)
            && !t.flags.contains(TokFlags::HAS_UCN)
        {
            return std::borrow::Cow::Borrowed(self.interner.as_str(s));
        }
        // A punctuator not spelled as a digraph and without splices or
        // trigraphs is spelled exactly as its primary spelling
        // ([lex.operators]): no source lookup needed.
        if let TokKind::Punct(p) = t.kind
            && !t.flags.contains(TokFlags::NEEDS_CLEANING)
            && !t.flags.contains(TokFlags::DIGRAPH)
        {
            #[cfg(debug_assertions)]
            if let Some(text) = self.sm.span_text(t.span)
                && !text.is_empty()
            {
                debug_assert_eq!(text, p.as_str(), "punctuator spelling");
            }
            return std::borrow::Cow::Borrowed(p.as_str());
        }
        crate::profile::count!(spelling_slow);
        kiln_lex::spelling(self.sm, t, &self.lex_opts)
    }

    /// True if `t` is one of the integer literals or commas an `#embed`
    /// directive produced for the resource's bytes (clang prints these as
    /// one `annot_embed` token).
    pub fn is_embed_byte(&self, t: &Token) -> bool {
        let at = self.sm.spelling_loc(t.span.lo);
        self.sm.offset_of(at).is_some_and(|(f, _)| self.embed_bufs.contains(&f))
    }

    /// True if the top frame is a file frame.
    pub(crate) fn in_file(&self) -> bool {
        matches!(self.frames.last(), Some(Frame::File(_)))
    }

    /// The directive-level lookahead: true if a `Marker` for a message
    /// pragma etc. should be queued (always).
    pub(crate) fn queue(&mut self, m: Marker) {
        self.markers.push_back(m);
    }
}

/// An `Eof` token at `span`.
pub(crate) fn eof_tok(span: Span) -> Token {
    Token { kind: TokKind::Eof, span, flags: TokFlags::empty() }
}

/// Re-export for the other modules.
pub(crate) type Def = Rc<MacroDef>;

/// The source manager's kind for a frame's `system` / `extern_c` flags.
pub(crate) fn file_kind(system: bool, extern_c: bool) -> kiln_source::FileKind {
    match (system, extern_c) {
        (_, true) => kiln_source::FileKind::ExternCSystem,
        (true, false) => kiln_source::FileKind::System,
        (false, false) => kiln_source::FileKind::User,
    }
}
