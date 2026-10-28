//! The preprocessor driver: the include / macro-expansion stack, the main
//! lexing loop ([cpp.pre], [cpp.rescan]) and the public API.

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use kiln_diag::{Diag, DiagEngine, DiagId, SCRATCH_BUFFER_NAME};
use kiln_lex::{Interner, LexOptions, Lexer, Punct, Symbol, TokFlags, TokKind, Token};
use kiln_source::{FileId, Loc, SourceManager, Span};

use crate::builtin::{FeatureQueries, NoFeatures};
use crate::callbacks::{FileChange, NoCallbacks, PpCallbacks, PragmaIntroducer, PragmaMessageKind};
use crate::cond::{IfEvaluator, StubIfEvaluator};
use crate::include::{IncludeResolver, NoIncludes};
use crate::macros::{MacroDef, MacroTable, SymMap};
use crate::options::PpOptions;
use crate::tok::{PpFlags, PpTok};

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
}

/// A buffer being lexed.
pub(crate) struct FileFrame<'a> {
    pub(crate) lx: Lexer<'a>,
    pub(crate) file: FileId,
    /// Tokens lexed ahead (by the `(` lookahead), last = next.
    pub(crate) back: Vec<Token>,
    /// Start-of-line / leading-space flags propagated from an ended macro
    /// expansion, ORed into the next token.
    pub(crate) pend: TokFlags,
    pub(crate) conds: Vec<Cond>,
    /// Search-path index the file was found at (for `#include_next`).
    pub(crate) lookup: Option<u32>,
    pub(crate) system: bool,
}

/// A macro expansion or a token stream being read.
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
    pub(crate) pragma: Symbol,
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
    pub(crate) syms: Syms,
    pub(crate) counter: u32,
    pub(crate) once_files: HashSet<FileId>,
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
            pragma: interner.intern("_Pragma"),
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
            evaluator: Some(Box::new(StubIfEvaluator)),
            features: Box::new(NoFeatures),
            callbacks: Box::new(NoCallbacks),
            markers: VecDeque::new(),
            pending: None,
            scratch: HashMap::new(),
            pool: Vec::new(),
            syms,
            counter: 0,
            once_files: HashSet::new(),
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
        };
        pp.define_builtins();
        pp
    }

    /// Installs the `#include` / `#embed` / `__has_include` resolver
    /// (default: [`NoIncludes`]).
    pub fn set_include_resolver(&mut self, r: Box<dyn IncludeResolver + 'd>) {
        self.resolver = r;
    }

    /// Installs the `#if` evaluator (default: [`StubIfEvaluator`]).
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

    /// Enters `file` as the main source file: pushes it, then the
    /// predefines buffer (`<built-in>`: [`PpOptions::predefines`], then
    /// `-D` / `-U`) on top of it, so that the predefines are read first.
    pub fn enter_main_file(&mut self, file: FileId) {
        use std::fmt::Write as _;
        self.main_file = Some(file);
        self.push_file(file, None, None, false);
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
            self.push_file(b, None, None, false);
        }
    }

    /// Pushes a file frame for `file` (reporting its invalid UTF-8 once).
    pub(crate) fn push_file(
        &mut self,
        file: FileId,
        from: Option<Loc>,
        lookup: Option<u32>,
        system: bool,
    ) {
        if self.entered.insert(file) {
            for r in &self.sm.load_report(file).replacements {
                let at = self.sm.loc(file, r.offset);
                Diag::new(DiagId::lex_invalid_utf8, Span::new(at, at.with_offset(3)))
                    .emit(self.diags);
            }
        }
        let lx = Lexer::new(self.sm, self.interner, file, self.sm.loc(file, 0), self.lex_opts);
        self.frames.push(Frame::File(Box::new(FileFrame {
            lx,
            file,
            back: Vec::new(),
            pend: TokFlags::empty(),
            conds: Vec::new(),
            lookup,
            system,
        })));
        self.file_change(FileChange::Enter { file, from });
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
    pub(crate) fn lex_file_tok(&mut self) -> Token {
        let top = self.frames.len().wrapping_sub(1);
        if !matches!(self.frames.last(), Some(Frame::File(_))) {
            return eof_tok(Span::INVALID);
        }
        let mut t = self.lex_raw_from(top);
        let Some(Frame::File(f)) = self.frames.last_mut() else { unreachable!() };
        if !f.pend.is_empty() && !matches!(t.kind, TokKind::Eod | TokKind::Eof) {
            t.flags = t.flags | f.pend;
            f.pend = TokFlags::empty();
        }
        if let TokKind::Identifier(s) = t.kind {
            self.check_poison(s, t.span);
        }
        t
    }

    /// The next token of file frame `i` (pushed-back tokens first),
    /// reporting lexer diagnostics; no flags are applied.
    fn lex_raw_from(&mut self, i: usize) -> Token {
        let Frame::File(f) = &mut self.frames[i] else { unreachable!() };
        if let Some(t) = f.back.pop() {
            return t;
        }
        let t = f.lx.next();
        if !f.lx.diagnostics().is_empty() {
            let ds = f.lx.take_diagnostics();
            for d in &ds {
                if let Some(diag) = crate::diag_map::lex_diag(self.sm, d) {
                    self.emit(diag);
                }
            }
        }
        t
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
        if self.va_scope {
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
            match self.frames.last_mut() {
                None => return PpTok::new(eof_tok(Span::INVALID)),
                Some(Frame::Macro(m)) => {
                    if m.idx >= m.toks.len() {
                        self.pop_macro();
                        continue;
                    }
                    let t = self.macro_next();
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
                    let t = self.lex_file_tok();
                    match t.kind {
                        TokKind::Punct(Punct::Hash) if t.has(TokFlags::START_OF_LINE) => {
                            self.handle_directive(t);
                        }
                        TokKind::Eof => {
                            if self.end_of_file(t) {
                                return PpTok::new(t);
                            }
                        }
                        TokKind::Identifier(s) => {
                            if t.has(TokFlags::START_OF_LINE)
                                && (s == self.syms.module || s == self.syms.import || s == self.syms.export)
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
        self.include_depth = self.include_depth.saturating_sub(1);
        if let Some(Frame::File(below)) = self.frames.last() {
            let file = below.file;
            let resume = self.sm.loc(file, below.lx.offset());
            self.file_change(FileChange::Exit { file, exited, resume });
        }
        false
    }

    /// A pooled, empty token buffer.
    pub(crate) fn take_buf(&mut self) -> Vec<PpTok> {
        self.pool.pop().unwrap_or_default()
    }

    /// Returns a token buffer to the pool.
    pub(crate) fn give_buf(&mut self, mut v: Vec<PpTok>) {
        v.clear();
        if self.pool.len() < 64 {
            self.pool.push(v);
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
        if loc.is_macro() {
            self.sm.expansion_info(loc).map(|e| e.base)
        } else {
            self.sm.offset_of(loc).map(|(f, _)| self.sm.loc(f, 0))
        }
    }

    /// `hi - lo` for two locations in the same entry, walking spelling
    /// chains down to a file (linear in the expansion depth).
    pub(crate) fn loc_diff(&self, lo: Loc, hi: Loc) -> Option<u32> {
        let (mut a, mut b) = (lo, hi);
        while a.is_macro() && b.is_macro() {
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
        kiln_lex::spelling(self.sm, t, &self.lex_opts)
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
