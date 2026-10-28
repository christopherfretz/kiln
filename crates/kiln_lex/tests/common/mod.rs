//! Shared helpers for the kiln_lex integration tests.
#![allow(dead_code, reason = "each test binary uses a subset")]

use kiln_lex::{Interner, LexDiagKind, LexMode, LexOptions, Lexer, Std, TokKind, Token, dump};
use kiln_source::SourceManager;

/// Options for C++26 (the default).
pub fn cxx26() -> LexOptions {
    LexOptions::default()
}

/// Options for `std`.
pub fn std(std: Std) -> LexOptions {
    LexOptions::new(std)
}

/// The result of lexing one buffer.
pub struct Lexed {
    pub dump: String,
    pub diags: Vec<(LexDiagKind, u32, u32)>,
    pub tokens: Vec<Token>,
    pub spellings: Vec<String>,
    pub idents: Vec<String>,
}

/// Lexes `text` (whole file) to `Eof`, excluding the `Eof` token itself.
pub fn lex_with(text: &str, opts: LexOptions) -> Lexed {
    lex_driven(text, opts, |_, _| {})
}

/// Like [`lex_with`] but lets `drive` adjust the lexer before each token
/// (called with the lexer and the tokens so far).
pub fn lex_driven(text: &str, opts: LexOptions, mut drive: impl FnMut(&mut Lexer<'_>, &[Token])) -> Lexed {
    let sm = SourceManager::new();
    let file = sm.add_virtual("<test>", text.to_owned()).unwrap();
    let interner = Interner::new();
    let mut lx = Lexer::new(&sm, &interner, file, sm.loc(file, 0), opts);
    let mut tokens = Vec::new();
    for _ in 0..100_000 {
        drive(&mut lx, &tokens);
        let t = lx.next();
        if t.kind == TokKind::Eof {
            break;
        }
        tokens.push(t);
    }
    let diags = lx
        .diagnostics()
        .iter()
        .map(|d| {
            let lo = sm.offset_of(d.span.lo).map_or(u32::MAX, |x| x.1);
            let hi = sm.offset_of(d.span.hi).map_or(u32::MAX, |x| x.1);
            (d.kind, lo, hi)
        })
        .collect();
    let spellings = tokens.iter().map(|t| lx.spelling(t).into_owned()).collect();
    let idents = tokens
        .iter()
        .filter_map(|t| match t.kind {
            TokKind::Identifier(s) => Some(interner.as_str(s).to_owned()),
            _ => None,
        })
        .collect();
    Lexed { dump: dump(&sm, &opts, &tokens), diags, tokens, spellings, idents }
}

/// Lexes `text` with C++26 options.
pub fn lex(text: &str) -> Lexed {
    lex_with(text, cxx26())
}

/// The dump of `text` (C++26), without the `Eof` line.
pub fn dump_of(text: &str) -> String {
    lex(text).dump
}

/// Just the diagnostic kinds.
pub fn diag_kinds(text: &str, opts: LexOptions) -> Vec<LexDiagKind> {
    lex_with(text, opts).diags.into_iter().map(|d| d.0).collect()
}

/// The token kinds' dump names, space separated.
pub fn kinds(text: &str) -> String {
    kinds_with(text, cxx26())
}

/// [`kinds`] with options.
pub fn kinds_with(text: &str, opts: LexOptions) -> String {
    lex_with(text, opts).tokens.iter().map(|t| t.kind.name()).collect::<Vec<_>>().join(" ")
}

/// The spellings, space separated.
pub fn spellings(text: &str) -> String {
    lex(text).spellings.join(" ")
}

/// Lexes with header-name mode set before the first token, and before
/// the token following each `include` identifier.
pub fn lex_header_mode(text: &str) -> Lexed {
    lex_driven(text, cxx26(), |lx, toks| {
        if toks.is_empty() {
            lx.set_mode(LexMode::HeaderName);
        }
    })
}
