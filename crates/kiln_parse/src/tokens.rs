//! The parser's token buffer (spec/parse.md §2).
//!
//! Until the preprocessor (WP-012) feeds the parser, the buffer lexes the
//! main file directly: comments are dropped, alternative tokens become
//! their punctuators, and preprocessing directives are skipped (a
//! `#pragma pack` and every directive other than `#pragma` are reported as
//! unsupported). The whole file is lexed up front, so lookahead is an
//! index.

use kiln_lex::{LexDiag, LexOptions, Lexer, Punct, TokFlags, TokKind, Token};
use kiln_source::symbol::Interner;
use kiln_source::{FileId, Loc, SourceManager};

use crate::keywords::alternative_token;

/// A directive the buffer skipped that the parser must report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skipped {
    /// `#pragma <name>` that kiln does not implement but must not ignore
    /// silently (`pack`).
    UnsupportedPragma(String, Loc),
    /// Any other directive (`#include`, `#define`, ...).
    Directive(String, Loc),
}

/// The lexed tokens of one file.
#[derive(Debug)]
pub struct TokenBuffer {
    /// The tokens, ending with exactly one `Eof`.
    pub toks: Vec<Token>,
    /// The spelling of each literal token (numbers, characters, strings);
    /// `None` for the others.
    pub spellings: Vec<Option<Box<str>>>,
    /// The lexer's diagnostics.
    pub diags: Vec<LexDiag>,
    /// Directives to report.
    pub skipped: Vec<Skipped>,
}

impl TokenBuffer {
    /// Lexes `file`.
    pub fn lex(sm: &SourceManager, interner: &Interner, file: FileId, opts: LexOptions) -> Self {
        let mut lx = Lexer::new(sm, interner, file, sm.loc(file, 0), opts);
        let mut toks = Vec::new();
        let mut spellings = Vec::new();
        let mut skipped = Vec::new();
        let mut pending: Option<Token> = None;
        loop {
            let t = pending.take().unwrap_or_else(|| lx.next());
            match t.kind {
                TokKind::Comment => continue,
                TokKind::Punct(Punct::Hash) if t.has(TokFlags::START_OF_LINE) => {
                    pending = Some(skip_directive(&mut lx, t, &mut skipped));
                    continue;
                }
                _ => {}
            }
            let mut t = t;
            let spelling = match t.kind {
                TokKind::PpNumber
                | TokKind::CharLit(_)
                | TokKind::StringLit(_)
                | TokKind::RawStringLit(_) => Some(lx.spelling(&t).into_owned().into_boxed_str()),
                TokKind::Identifier(sym) => {
                    if let Some(p) = alternative_token(interner.as_str(sym)) {
                        t.kind = TokKind::Punct(p);
                    }
                    None
                }
                _ => None,
            };
            toks.push(t);
            spellings.push(spelling);
            if t.kind == TokKind::Eof {
                break;
            }
        }
        let diags = lx.take_diagnostics();
        TokenBuffer { toks, spellings, diags, skipped }
    }
}

/// Skips the directive introduced by `hash`; returns the first token
/// after it.
fn skip_directive(lx: &mut Lexer<'_>, hash: Token, skipped: &mut Vec<Skipped>) -> Token {
    let mut t = lx.next();
    let at_new_line = |t: &Token| t.has(TokFlags::START_OF_LINE) || t.kind == TokKind::Eof;
    if at_new_line(&t) {
        return t; // the null directive
    }
    let name = lx.spelling(&t).into_owned();
    let mut words = Vec::new();
    t = lx.next();
    while !at_new_line(&t) {
        if words.len() < 2 && t.kind != TokKind::Comment {
            words.push(lx.spelling(&t).into_owned());
        }
        t = lx.next();
    }
    if name == "pragma" {
        if words.first().is_some_and(|w| w == "pack") {
            skipped.push(Skipped::UnsupportedPragma("pack".into(), hash.span.lo));
        }
    } else {
        skipped.push(Skipped::Directive(name, hash.span.lo));
    }
    t
}
