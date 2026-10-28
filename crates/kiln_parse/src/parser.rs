//! The parser state, token access and error recovery (spec/parse.md §3).

use kiln_ast::{DeclId, ExprId};
use kiln_diag::{Diag, DiagId};
use kiln_lex::literal::LiteralOptions;
use kiln_lex::{LexOptions, Punct, Std, TokKind, Token};
use kiln_sema::{NameClass, Sema};
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, SourceManager};

use crate::keywords::{Keywords, Kw};

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

/// A recursive-descent parser over one token buffer, driving [`Sema`].
pub struct Parser<'a, 'sm> {
    /// Semantic analysis.
    pub(crate) s: Sema<'a, 'sm>,
    /// The sources (for literal spellings and diagnostics).
    pub(crate) sm: &'sm SourceManager,
    toks: Vec<Token>,
    /// Literal spellings, parallel to `toks`.
    pub(crate) spellings: Vec<Option<Box<str>>>,
    pos: usize,
    /// Open `(`, `[`, `{` consumed and not yet closed (clang's
    /// `ParenCount`/`BracketCount`/`BraceCount`).
    depth: [u32; 3],
    kw: Keywords,
    pub(crate) lex_opts: LexOptions,
    pub(crate) lit_opts: LiteralOptions,
    pub(crate) std: Std,
}

impl std::fmt::Debug for Parser<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parser").field("pos", &self.pos).finish_non_exhaustive()
    }
}

impl<'a, 'sm> Parser<'a, 'sm> {
    /// A parser over `toks` (which end with `Eof`).
    pub fn new(
        s: Sema<'a, 'sm>,
        sm: &'sm SourceManager,
        toks: Vec<Token>,
        spellings: Vec<Option<Box<str>>>,
        lex_opts: LexOptions,
        lit_opts: LiteralOptions,
    ) -> Self {
        let kw = Keywords::new(s.ast.symbols());
        let std = lex_opts.std;
        Parser { s, sm, toks, spellings, pos: 0, depth: [0; 3], kw, lex_opts, lit_opts, std }
    }

    /// Finishes parsing, returning the semantic state.
    pub fn into_sema(self) -> Sema<'a, 'sm> {
        self.s
    }

    // -- token access -----------------------------------------------------------

    /// The current token.
    pub(crate) fn tok(&self) -> Token {
        self.toks[self.pos]
    }

    /// The token `n` ahead of the current one (`Eof` past the end).
    pub(crate) fn peek(&self, n: usize) -> Token {
        self.toks[(self.pos + n).min(self.toks.len() - 1)]
    }

    /// The token at absolute index `i`.
    pub(crate) fn tok_at(&self, i: usize) -> Token {
        self.toks[i.min(self.toks.len() - 1)]
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
        if self.pos == 0 { self.loc() } else { self.toks[self.pos - 1].span.hi }
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
        t
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
        let mut qual: Option<DeclId> = None;
        if self.at(Punct::ColonColon) {
            self.bump();
            qual = Some(self.s.ast.tu);
        }
        while let Some(name) = self.ident() {
            if !self.peek_is(1, Punct::ColonColon) {
                break;
            }
            let loc = self.bump().span.lo;
            self.bump();
            match self.s.act_on_nested_name(qual, name, loc) {
                Some(d) => qual = Some(d),
                None => return Err(()),
            }
        }
        Ok(qual)
    }
}

/// `'p'` as clang quotes a token in "expected" diagnostics.
pub(crate) fn quote(p: Punct) -> String {
    format!("'{}'", p.as_str())
}
