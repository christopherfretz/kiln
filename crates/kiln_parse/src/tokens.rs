//! The parser's token source (spec/parse.md §2): the preprocessor's
//! phase-4 output behind a sliding lookahead window.
//!
//! [`TokenSource`] pulls tokens from [`Preprocessor::next_item`] on demand.
//! Lookahead is by absolute token index (the parser's `tok_at(i)`): the
//! window holds every token from a little behind the parser's position to
//! the furthest token looked at, and consumed tokens are dropped as the
//! parser advances ([`TokenSource::release`]). Alternative tokens
//! ([lex.digraph]/2: `and`, `bitor`, ...) become their punctuators here;
//! other keywords stay identifiers for the parser to classify.
//!
//! Preprocessor markers become [`Event`]s keyed by the index of the token
//! they precede; the parser handles them when it reaches that token
//! (pragmas, spec/parse.md §2.3). A `module` / `import` directive is
//! reported and its tokens through the `;` dropped. After a fatal error
//! (`fatal error: 'x' file not found`) the source ends: clang stops
//! parsing there too.

use std::collections::VecDeque;

use kiln_diag::DiagEngine;
use kiln_lex::{LexOptions, Punct, TokKind, Token};
use kiln_pp::{Marker, PpItem, Preprocessor};
use kiln_source::Span;
use kiln_source::symbol::Symbol;

use crate::keywords::ALTERNATIVE_TOKENS;

/// A preprocessor event the parser handles when it reaches the token
/// after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A pragma the preprocessor passed through: the `pragma` / `_Pragma`
    /// token and the tokens after it (unexpanded, without the end of line).
    Pragma {
        /// The `pragma` / `_Pragma` token.
        span: Span,
        /// The pragma's tokens.
        tokens: Vec<Token>,
    },
    /// A `module` / `import` directive ([cpp.module], [cpp.import]),
    /// already removed from the token stream.
    Module {
        /// The first token of the directive.
        span: Span,
        /// An `import` (else a `module`) directive.
        import: bool,
    },
}

/// How many consumed tokens the window keeps behind the parser's position
/// (the parser reads at most one token back: `prev_end`, the string of a
/// linkage specification).
pub const KEEP_BEHIND: usize = 8;

/// Consumed tokens accumulate up to this many before the window drops
/// them (in one `drain`, so the cost is amortized).
const RELEASE_BATCH: usize = 256;

/// The parser's view of the preprocessed token stream.
pub struct TokenSource<'a, 'sm> {
    pp: Preprocessor<'sm, 'a>,
    diags: &'a DiagEngine<'sm>,
    /// Tokens `base..base + toks.len()`.
    toks: Vec<Token>,
    base: usize,
    /// The index of the `Eof` token, once read.
    eof: Option<usize>,
    /// Pending events, by the index of the token they precede.
    events: VecDeque<(usize, Event)>,
    /// The alternative-token spellings' symbols and their punctuators.
    alternatives: Vec<(Symbol, Punct)>,
    /// Never drop consumed tokens (`-fparse-window=whole`, for measuring).
    keep_all: bool,
}

impl std::fmt::Debug for TokenSource<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenSource")
            .field("base", &self.base)
            .field("window", &self.toks.len())
            .field("eof", &self.eof)
            .finish_non_exhaustive()
    }
}

impl<'a, 'sm> TokenSource<'a, 'sm> {
    /// A source reading `pp` (whose main file is entered), whose
    /// diagnostics go to `diags`.
    pub fn new(pp: Preprocessor<'sm, 'a>, diags: &'a DiagEngine<'sm>) -> Self {
        let interner = pp.interner();
        let alternatives =
            ALTERNATIVE_TOKENS.iter().map(|&(s, p)| (interner.intern(s), p)).collect();
        TokenSource {
            pp,
            diags,
            toks: Vec::with_capacity(RELEASE_BATCH + 64),
            base: 0,
            eof: None,
            events: VecDeque::new(),
            alternatives,
            keep_all: false,
        }
    }

    /// Keeps every token (the whole-file buffer the parser used before
    /// WP-055), for measurements.
    pub fn keep_all(&mut self, keep: bool) {
        self.keep_all = keep;
    }

    /// The lexer options of the stream (for spellings).
    pub fn lex_options(&self) -> LexOptions {
        *self.pp.lex_options()
    }

    /// The source manager.
    pub fn source_manager(&self) -> &'sm kiln_source::SourceManager {
        self.pp.source_manager()
    }

    /// The preprocessor (for its macro table and callbacks after parsing).
    pub fn preprocessor(&mut self) -> &mut Preprocessor<'sm, 'a> {
        &mut self.pp
    }

    /// Returns the preprocessor.
    pub fn into_preprocessor(self) -> Preprocessor<'sm, 'a> {
        self.pp
    }

    /// The token at absolute index `i` (`Eof` past the end). An index
    /// already released reads as the oldest token kept.
    pub fn get(&mut self, i: usize) -> Token {
        if let Some(e) = self.eof
            && i >= e
        {
            return self.toks[self.toks.len() - 1];
        }
        while i >= self.base + self.toks.len() {
            self.read();
            if let Some(e) = self.eof
                && i >= e
            {
                return self.toks[self.toks.len() - 1];
            }
        }
        // `i < base` only if the parser looks further back than
        // [`KEEP_BEHIND`], which it never does.
        debug_assert!(i >= self.base, "token {i} was released");
        self.toks[i.saturating_sub(self.base)]
    }

    /// Drops the tokens before `pos - KEEP_BEHIND` (the parser is at
    /// `pos` and never rewinds).
    pub fn release(&mut self, pos: usize) {
        if self.keep_all {
            return;
        }
        let keep_from = pos.saturating_sub(KEEP_BEHIND);
        let n = keep_from.saturating_sub(self.base);
        if n >= RELEASE_BATCH && n < self.toks.len() {
            self.toks.drain(..n);
            self.base += n;
        }
    }

    /// The next pending event if it precedes token `pos` or an earlier one.
    pub fn take_event(&mut self, pos: usize) -> Option<Event> {
        match self.events.front() {
            Some((at, _)) if *at <= pos => self.events.pop_front().map(|(_, e)| e),
            _ => None,
        }
    }

    /// True if an event precedes token `pos` or an earlier one.
    pub fn has_event(&self, pos: usize) -> bool {
        self.events.front().is_some_and(|(at, _)| *at <= pos)
    }

    /// Reads one token from the preprocessor into the window.
    fn read(&mut self) {
        let next = self.base + self.toks.len();
        loop {
            if self.diags.has_fatal() {
                self.push_eof();
                return;
            }
            match self.pp.next_item() {
                PpItem::Token(mut t) => {
                    if let TokKind::Identifier(s) = t.kind
                        && let Some(&(_, p)) = self.alternatives.iter().find(|(a, _)| *a == s)
                    {
                        t.kind = TokKind::Punct(p);
                    }
                    if t.kind == TokKind::Eof {
                        self.eof = Some(next);
                    }
                    self.toks.push(t);
                    return;
                }
                PpItem::Marker(m) => self.marker(m, next),
            }
        }
    }

    fn marker(&mut self, m: Marker, next: usize) {
        match m {
            Marker::Pragma { span, tokens, .. } => {
                self.events.push_back((next, Event::Pragma { span, tokens }));
            }
            Marker::Module { span, import, .. } => {
                // The directive's tokens end with `;` ([cpp.module],
                // [cpp.import]); the parser never sees them.
                loop {
                    let t = self.pp.next();
                    // `Eof` repeats: the caller reads it next.
                    if t.kind == TokKind::Punct(Punct::Semi) || t.kind == TokKind::Eof {
                        break;
                    }
                }
                self.events.push_back((next, Event::Module { span, import }));
            }
            // Already reported by the preprocessor, or no effect on
            // phase 7 (the `#embed` bytes are ordinary literals).
            Marker::PragmaMessage { .. }
            | Marker::Ident { .. }
            | Marker::FileChange(_)
            | Marker::Embed { .. } => {}
        }
    }

    /// Ends the stream (after a fatal error).
    fn push_eof(&mut self) {
        let next = self.base + self.toks.len();
        let span = self.toks.last().map_or(Span::INVALID, |t| Span::new(t.span.hi, t.span.hi));
        let mut t = self.toks.last().copied().unwrap_or_else(|| self.pp.next());
        t.kind = TokKind::Eof;
        t.span = span;
        self.toks.push(t);
        self.eof = Some(next);
    }
}
