//! The parser state, token access and error recovery (spec/parse.md §3).

use std::cell::RefCell;

use kiln_ast::{DeclId, ExprId, NnsId};
use kiln_diag::{Diag, DiagId};
use kiln_lex::literal::LiteralOptions;
use kiln_lex::{LexOptions, Punct, Std, TokFlags, TokKind, Token};
use kiln_sema::using::Qualifier;
use kiln_sema::{NameClass, Sema};
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, SourceManager, Span};

use crate::class::LateClass;
use crate::keywords::{Keywords, Kw};
use crate::tokens::TokenSource;

/// The result of parsing an expression: `None` when a syntax error was
/// reported (clang's invalid `ExprResult`); the caller recovers by
/// skipping. Semantic errors produce an error expression instead.
pub type PExpr = Option<ExprId>;

/// The bracket kinds whose nesting [`Parser::bump`] tracks.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Bracket {
    Paren = 0,
    Square = 1,
    Brace = 2,
}

/// A saved token buffer being re-parsed (spec/parse.md §2.4): while one
/// is active, [`Parser::tok_at`] and [`Parser::bump`] read its tokens
/// (indices from 0) and then an `Eof` sentinel, and the state of the
/// interrupted stream is restored by [`Parser::end_replay`].
///
/// An `inject` frame instead holds tokens inserted before the current
/// one (clang's `EnterToken`, [`Parser::inject_semi`]): after them the
/// interrupted stream continues, and the frame pops itself once they are
/// consumed.
struct Replay {
    toks: Vec<Token>,
    eof: Token,
    pos: usize,
    cur: Token,
    prev_hi: Loc,
    depth: [u32; 3],
    inject: bool,
}

/// A recursive-descent parser over the preprocessor's token stream,
/// driving [`Sema`].
pub struct Parser<'a, 'sm> {
    /// Semantic analysis.
    pub(crate) s: Sema<'a, 'sm>,
    /// The sources (for literal spellings and diagnostics).
    pub(crate) sm: &'sm SourceManager,
    /// The token stream; lookahead (`&self`) fills its window lazily.
    src: RefCell<TokenSource<'a, 'sm>>,
    /// The token at `pos`.
    cur: Token,
    /// The end of the token before `pos`.
    prev_hi: Loc,
    pos: usize,
    /// Open `#pragma GCC visibility push`es.
    pub(crate) visibility_depth: u32,
    /// Open `(`, `[`, `{` consumed and not yet closed (clang's
    /// `ParenCount`/`BracketCount`/`BraceCount`).
    depth: [u32; 3],
    kw: Keywords,
    pub(crate) lex_opts: LexOptions,
    pub(crate) lit_opts: LiteralOptions,
    pub(crate) std: Std,
    /// Active token replays, innermost last.
    replay: Vec<Replay>,
    /// The classes being defined whose complete-class contexts are saved
    /// for replay (clang's `ParsingClassStack`), innermost last.
    pub(crate) late: Vec<LateClass>,
    /// The saved default arguments and *noexcept-specifier* of the member
    /// function declarator being parsed, taken by the member-declaration.
    pub(crate) late_fn: crate::class::LateFnParts,
    /// The `act_on_*` calls the class parser made, in order (for the
    /// sequence tests of spec/sema-class.md §2.1).
    #[cfg(test)]
    pub(crate) trace: Vec<String>,
    /// The next statement parsed is directly in a statement expression's
    /// compound statement (clang's `ParsedStmtContext::InStmtExpr`); taken
    /// by `parse_statement`, passed on through labels.
    pub(crate) in_stmt_expr: bool,
}

impl std::fmt::Debug for Parser<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parser").field("pos", &self.pos).finish_non_exhaustive()
    }
}

impl<'a, 'sm> Parser<'a, 'sm> {
    /// A parser over `src`. `s`'s AST must use the preprocessor's
    /// interner.
    pub fn new(s: Sema<'a, 'sm>, mut src: TokenSource<'a, 'sm>, lit_opts: LiteralOptions) -> Self {
        let kw = Keywords::new(s.ast.symbols());
        let lex_opts = src.lex_options();
        let std = lex_opts.std;
        let sm = src.source_manager();
        let cur = src.get(0);
        let mut p = Parser {
            s,
            sm,
            src: RefCell::new(src),
            cur,
            prev_hi: Loc::INVALID,
            pos: 0,
            visibility_depth: 0,
            depth: [0; 3],
            kw,
            lex_opts,
            lit_opts,
            std,
            replay: Vec::new(),
            late: Vec::new(),
            late_fn: crate::class::LateFnParts::default(),
            #[cfg(test)]
            trace: Vec::new(),
            in_stmt_expr: false,
        };
        p.handle_events(0);
        p
    }

    /// Finishes parsing (handling the events after the last token),
    /// returning the semantic state and the token source.
    pub fn finish(mut self) -> (Sema<'a, 'sm>, TokenSource<'a, 'sm>) {
        self.handle_events(usize::MAX);
        (self.s, self.src.into_inner())
    }

    /// Finishes parsing, returning the semantic state.
    pub fn into_sema(self) -> Sema<'a, 'sm> {
        self.finish().0
    }

    /// Handles the preprocessor events before token `pos`.
    fn handle_events(&mut self, pos: usize) {
        while let Some(e) = self.src.get_mut().take_event(pos) {
            self.on_event(e);
        }
    }

    // -- token access -----------------------------------------------------------

    /// The current token.
    pub(crate) fn tok(&self) -> Token {
        self.cur
    }

    /// The token `n` ahead of the current one (`Eof` past the end).
    pub(crate) fn peek(&self, n: usize) -> Token {
        self.tok_at(self.pos + n)
    }

    /// The token at absolute index `i` (`Eof` past the end; at most
    /// [`crate::tokens::KEEP_BEHIND`] tokens before the current one).
    pub(crate) fn tok_at(&self, i: usize) -> Token {
        if i == self.pos { self.cur } else { self.tok_in(self.replay.len(), i) }
    }

    /// The token at index `i` of the stream `level` replays deep (0 is the
    /// token source).
    fn tok_in(&self, level: usize, i: usize) -> Token {
        let Some(r) = level.checked_sub(1).and_then(|l| self.replay.get(l)) else {
            return self.src.borrow_mut().get(i);
        };
        match r.toks.get(i) {
            Some(t) => *t,
            None if r.inject => self.tok_in(level - 1, r.pos + (i - r.toks.len())),
            None => r.eof,
        }
    }

    /// The current index (for lookahead scans).
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// The current token's location.
    pub(crate) fn loc(&self) -> Loc {
        self.tok().span.lo
    }

    /// The end of the previous token (where clang's `ExpectAndConsume`
    /// reports a missing token).
    pub(crate) fn prev_end(&self) -> Loc {
        if self.prev_hi == Loc::INVALID { self.loc() } else { self.prev_hi }
    }

    /// True at the end of input.
    pub(crate) fn at_eof(&self) -> bool {
        self.tok().kind == TokKind::Eof
    }

    /// Consumes the current token (never `Eof`) and returns it.
    pub(crate) fn bump(&mut self) -> Token {
        let t = self.tok();
        if t.kind == TokKind::Eof {
            return t;
        }
        if let TokKind::Punct(p) = t.kind {
            match p {
                Punct::LParen => self.depth[Bracket::Paren as usize] += 1,
                Punct::LSquare => self.depth[Bracket::Square as usize] += 1,
                Punct::LBrace => self.depth[Bracket::Brace as usize] += 1,
                Punct::RParen => self.close(Bracket::Paren),
                Punct::RSquare => self.close(Bracket::Square),
                Punct::RBrace => self.close(Bracket::Brace),
                _ => {}
            }
        }
        self.pos += 1;
        self.prev_hi = t.span.hi;
        if let Some(r) = self.replay.last() {
            if r.inject && self.pos >= r.toks.len() {
                // The inserted tokens are consumed: resume the stream.
                if let Some(r) = self.replay.pop() {
                    self.pos = r.pos;
                    self.cur = r.cur;
                }
                return t;
            }
            self.cur = r.toks.get(self.pos).copied().unwrap_or(r.eof);
            return t;
        }
        let src = self.src.get_mut();
        src.release(self.pos);
        self.cur = src.get(self.pos);
        if src.has_event(self.pos) {
            self.handle_events(self.pos);
        }
        t
    }

    /// Starts re-parsing `toks` (spec/parse.md §2.4): the current token
    /// becomes `toks[0]`, and after the last one an `Eof` sentinel at
    /// `end` (the token that ended the saved range) stops every rule.
    /// Paired with [`Parser::end_replay`].
    pub(crate) fn begin_replay(&mut self, toks: Vec<Token>, end: Loc) {
        let eof = Token { kind: TokKind::Eof, span: Span::new(end, end), flags: TokFlags::empty() };
        let cur = toks.first().copied().unwrap_or(eof);
        let saved = Replay {
            toks,
            eof,
            pos: self.pos,
            cur: self.cur,
            prev_hi: self.prev_hi,
            depth: self.depth,
            inject: false,
        };
        self.replay.push(saved);
        self.pos = 0;
        self.cur = cur;
        self.prev_hi = Loc::INVALID;
        self.depth = [0; 3];
    }

    /// Inserts a `;` before the current token (at its location), as clang
    /// does to recover from a missing `;` after a class or enum definition
    /// (`PP.EnterToken` + `Tok.setKind(tok::semi)`).
    pub(crate) fn inject_semi(&mut self) {
        let at = self.loc();
        let semi = Token {
            kind: TokKind::Punct(Punct::Semi),
            span: Span::new(at, at),
            flags: TokFlags::empty(),
        };
        let saved = Replay {
            toks: vec![semi],
            eof: semi,
            pos: self.pos,
            cur: self.cur,
            prev_hi: self.prev_hi,
            depth: self.depth,
            inject: true,
        };
        self.replay.push(saved);
        self.pos = 0;
        self.cur = semi;
    }

    /// Ends the innermost replay, restoring the interrupted stream, and
    /// returns true if the saved tokens were all consumed.
    pub(crate) fn end_replay(&mut self) -> bool {
        while self.replay.last().is_some_and(|r| r.inject) {
            self.replay.pop();
        }
        let done = self.at_eof();
        if let Some(r) = self.replay.pop() {
            self.pos = r.pos;
            self.cur = r.cur;
            self.prev_hi = r.prev_hi;
            self.depth = r.depth;
        }
        done
    }

    /// Records an `act_on_*` call for the sequence tests (a no-op
    /// outside tests).
    #[cfg_attr(not(test), allow(clippy::unused_self))]
    pub(crate) fn trace(&mut self, what: impl FnOnce(&Self) -> String) {
        #[cfg(test)]
        {
            let s = what(self);
            self.trace.push(s);
        }
        #[cfg(not(test))]
        let _ = what;
    }

    /// How many brackets of the kind `close` closes the parser has open
    /// (0 for a non-closer).
    pub(crate) fn depth_of(&self, close: Punct) -> u32 {
        let b = match close {
            Punct::RParen => Bracket::Paren,
            Punct::RSquare => Bracket::Square,
            Punct::RBrace => Bracket::Brace,
            _ => return 0,
        };
        self.depth[b as usize]
    }

    fn close(&mut self, b: Bracket) {
        let d = &mut self.depth[b as usize];
        *d = d.saturating_sub(1);
    }

    /// The punctuator `t` is.
    pub(crate) fn punct_of(t: Token) -> Option<Punct> {
        match t.kind {
            TokKind::Punct(p) => Some(p),
            _ => None,
        }
    }

    /// True if the current token is `p`.
    pub(crate) fn at(&self, p: Punct) -> bool {
        self.tok().kind == TokKind::Punct(p)
    }

    /// True if the token `n` ahead is `p`.
    pub(crate) fn peek_is(&self, n: usize, p: Punct) -> bool {
        self.peek(n).kind == TokKind::Punct(p)
    }

    /// The keyword `t` is.
    pub(crate) fn kw_of(&self, t: Token) -> Option<Kw> {
        match t.kind {
            TokKind::Identifier(s) => self.kw.get(s),
            _ => None,
        }
    }

    /// The current token's keyword.
    pub(crate) fn kw(&self) -> Option<Kw> {
        self.kw_of(self.tok())
    }

    /// True if the current token is keyword `k`.
    pub(crate) fn at_kw(&self, k: Kw) -> bool {
        self.kw() == Some(k)
    }

    /// The identifier (not a keyword) `t` is.
    pub(crate) fn ident_of(&self, t: Token) -> Option<Symbol> {
        match t.kind {
            TokKind::Identifier(s) if self.kw.get(s).is_none() => Some(s),
            _ => None,
        }
    }

    /// True if `t` is the contextual keyword `final` ([class.pre]/1,
    /// [class.mem.general]: an identifier with special meaning).
    pub(crate) fn is_final_tok(&self, t: Token) -> bool {
        t.kind == TokKind::Identifier(self.kw.final_)
    }

    /// True if `t` is the contextual keyword `override`.
    pub(crate) fn is_override_tok(&self, t: Token) -> bool {
        t.kind == TokKind::Identifier(self.kw.override_)
    }

    /// The current token as an identifier.
    pub(crate) fn ident(&self) -> Option<Symbol> {
        self.ident_of(self.tok())
    }

    /// Consumes `p` if it is next.
    pub(crate) fn eat(&mut self, p: Punct) -> Option<Loc> {
        if self.at(p) { Some(self.bump().span.lo) } else { None }
    }

    /// Consumes keyword `k` if it is next.
    pub(crate) fn eat_kw(&mut self, k: Kw) -> Option<Loc> {
        if self.at_kw(k) { Some(self.bump().span.lo) } else { None }
    }

    /// Consumes `p` or reports `expected 'p'` after the previous token
    /// (clang's `ExpectAndConsume`).
    pub(crate) fn expect(&mut self, p: Punct) -> Option<Loc> {
        if let Some(l) = self.eat(p) {
            return Some(l);
        }
        let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg(quote(p));
        self.s.emit(d);
        None
    }

    /// Where a missing token is reported: after the previous token, or at
    /// the current one when that is the first token or at end of file
    /// (clang reports at the end of the previous token when it is valid).
    pub(crate) fn expected_loc(&self) -> Loc {
        self.prev_end()
    }

    /// Consumes the closing `close` of `open` at `open_loc`, or reports
    /// `expected 'close'` with `to match this 'open'`.
    pub(crate) fn expect_closing(&mut self, close: Punct, open: Punct, open_loc: Loc) -> Loc {
        if let Some(l) = self.eat(close) {
            return l;
        }
        // A missing closer at a token the parser already stopped at is
        // reported there (clang's `BalancedDelimiterTracker::diagnoseMissingClose`
        // reports at the current token).
        let at = self.loc();
        let note = self.s.diag(DiagId::note_matching, open_loc).arg(quote(open));
        let d = self.s.diag(DiagId::parse_expected, at).arg(quote(close)).with_note(note);
        self.s.emit(d);
        // Unless already at a closing bracket, skip to the closer and
        // take it (clang's `diagnoseMissingClose`: `SkipUntil(close,
        // StopAtSemi | StopBeforeMatch)`).
        if !matches!(
            Self::punct_of(self.tok()),
            Some(Punct::RParen | Punct::RBrace | Punct::RSquare)
        ) && self.skip_until(&[close], true, false)
            && let Some(l) = self.eat(close)
        {
            return l;
        }
        at
    }

    /// Reports `id` at the current token.
    pub(crate) fn error_here(&self, id: DiagId) {
        self.s.error(id, self.loc());
    }

    /// Reports a diagnostic.
    pub(crate) fn emit(&self, d: Diag) {
        self.s.emit(d);
    }

    /// Reports `{what} is not supported yet` at `loc`.
    pub(crate) fn unsupported(&self, what: &str, loc: Loc) {
        self.s.unsupported(what, loc);
    }

    // -- recovery ---------------------------------------------------------------

    /// clang's `SkipUntil`: skips tokens until one of `stops` (consumed
    /// when `consume`), balancing brackets. Stops before `;` when
    /// `stop_at_semi`, and before a closing bracket that closes one the
    /// parser opened earlier. Returns true if a stop token was found.
    pub(crate) fn skip_until(
        &mut self,
        stops: &[Punct],
        stop_at_semi: bool,
        consume: bool,
    ) -> bool {
        let mut first = true;
        loop {
            let t = self.tok();
            if t.kind == TokKind::Eof {
                return false;
            }
            if let Some(p) = Self::punct_of(t)
                && stops.contains(&p)
            {
                if consume {
                    self.bump();
                }
                return true;
            }
            match Self::punct_of(t) {
                Some(Punct::LParen) => {
                    self.bump();
                    self.skip_until(&[Punct::RParen], false, true);
                }
                Some(Punct::LSquare) => {
                    self.bump();
                    self.skip_until(&[Punct::RSquare], false, true);
                }
                Some(Punct::LBrace) => {
                    self.bump();
                    self.skip_until(&[Punct::RBrace], false, true);
                }
                Some(p @ (Punct::RParen | Punct::RSquare | Punct::RBrace)) => {
                    let b = match p {
                        Punct::RParen => Bracket::Paren,
                        Punct::RSquare => Bracket::Square,
                        _ => Bracket::Brace,
                    };
                    if self.depth[b as usize] > 0 && !first {
                        return false;
                    }
                    self.bump();
                }
                Some(Punct::Semi) if stop_at_semi => return false,
                _ => {
                    self.bump();
                }
            }
            first = false;
        }
    }

    /// Skips to the end of the current declaration or statement: up to and
    /// including the next `;` at this nesting level, or up to (not
    /// including) a `}` that closes an enclosing brace.
    pub(crate) fn skip_to_semi(&mut self) {
        self.skip_until(&[Punct::RBrace], true, false);
        if self.at(Punct::RBrace) && self.depth[Bracket::Brace as usize] == 0 {
            // A stray `}` at file scope is reported by the caller.
            return;
        }
        self.eat(Punct::Semi);
    }

    /// The index of the first of `targets` at bracket depth 0, scanning
    /// from the current token and stopping at an unbalanced closer, `{`,
    /// `;` (unless a target) or end of file.
    pub(crate) fn scan_for(&self, targets: &[Punct]) -> Option<(usize, Punct)> {
        let mut depth = 0u32;
        let mut i = self.pos;
        loop {
            let t = self.tok_at(i);
            let p = match t.kind {
                TokKind::Eof => return None,
                TokKind::Punct(p) => p,
                _ => {
                    i += 1;
                    continue;
                }
            };
            if depth == 0 && targets.contains(&p) {
                return Some((i, p));
            }
            match p {
                Punct::LParen | Punct::LSquare => depth += 1,
                Punct::RParen | Punct::RSquare => {
                    if depth == 0 {
                        return None;
                    }
                    depth -= 1;
                }
                Punct::LBrace | Punct::RBrace | Punct::Semi if depth == 0 => return None,
                _ => {}
            }
            i += 1;
        }
    }

    // -- names --------------------------------------------------------------------

    /// Classifies the (possibly qualified) name starting at token index
    /// `i`, without consuming or diagnosing: returns the index after it
    /// and the classification of its last component. `None` if there is
    /// no name at `i` or a qualifier does not name a scope.
    pub(crate) fn scan_name(&self, mut i: usize) -> Option<(usize, NameClass, Symbol)> {
        let mut qual: Option<DeclId> = None;
        if self.tok_at(i).kind == TokKind::Punct(Punct::ColonColon) {
            qual = Some(self.s.ast.tu);
            i += 1;
        }
        loop {
            let name = self.ident_of(self.tok_at(i))?;
            let class = self.s.classify_name(name, qual);
            if self.tok_at(i + 1).kind == TokKind::Punct(Punct::ColonColon) {
                qual = Some(match class {
                    NameClass::Namespace(d) | NameClass::Type(_, d) => d,
                    _ => return None,
                });
                i += 2;
                continue;
            }
            return Some((i + 1, class, name));
        }
    }

    /// Parses an optional nested-name-specifier (`::`, `N::`, `N::M::`),
    /// diagnosing unknown names. Returns `Ok(None)` when there is none,
    /// `Err(())` after an error.
    pub(crate) fn parse_nested_name(&mut self) -> Result<Option<DeclId>, ()> {
        Ok(self.parse_nested_name_nns()?.map(|q| q.ctx))
    }

    /// [`Parser::parse_nested_name`], also building the specifier as
    /// written (namespace aliases and typedef names kept).
    pub(crate) fn parse_nested_name_nns(&mut self) -> Result<Option<Qualifier>, ()> {
        let start = self.loc();
        let mut qual: Option<(DeclId, NnsId)> = None;
        if self.at(Punct::ColonColon) {
            self.bump();
            qual = Some((self.s.ast.tu, self.s.nns_global()));
        }
        while let Some(name) = self.ident() {
            if !self.peek_is(1, Punct::ColonColon) {
                break;
            }
            let loc = self.bump().span.lo;
            self.bump();
            let Some((ctx, spelled)) =
                self.s.act_on_nested_name_spelled(qual.map(|q| q.0), name, loc)
            else {
                return Err(());
            };
            let nns = self.s.nns_component(qual.map(|q| q.1), ctx, spelled);
            qual = Some((ctx, nns));
        }
        Ok(qual.map(|(ctx, nns)| Qualifier { ctx, nns, loc: start }))
    }
}

/// `'p'` as clang quotes a token in "expected" diagnostics.
pub(crate) fn quote(p: Punct) -> String {
    format!("'{}'", p.as_str())
}
