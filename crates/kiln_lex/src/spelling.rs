//! Token spellings.

use std::borrow::Cow;

use kiln_source::SourceManager;

use crate::options::LexOptions;
use crate::phase::Src;
use crate::token::{TokFlags, TokKind, Token};

/// The spelling of `tok`: its source text with line splices removed (and
/// trigraphs replaced, if `opts.trigraphs`); universal-character-names
/// are kept as written (clang's `getSpelling`). Inside a raw string
/// literal's body nothing is removed ([lex.pptoken]/3.1).
///
/// Borrows the source unless the token has [`TokFlags::NEEDS_CLEANING`].
/// Works for tokens at macro locations too (the span is mapped through
/// spelling locations). Returns `""` for `Eod`/`Eof` and for spans the
/// source manager cannot map.
pub fn spelling<'s>(sm: &'s SourceManager, tok: &Token, opts: &LexOptions) -> Cow<'s, str> {
    let Some(text) = sm.span_text(tok.span) else {
        return Cow::Borrowed("");
    };
    if tok.flags.contains(TokFlags::NEEDS_CLEANING) {
        Cow::Owned(clean(text, tok.kind, opts.trigraphs))
    } else {
        Cow::Borrowed(text)
    }
}

/// Removes line splices (and translates trigraphs if `trigraphs`) from the
/// source text of a token of kind `kind`.
pub fn clean(text: &str, kind: TokKind, trigraphs: bool) -> String {
    let mut out = String::with_capacity(text.len());
    walk(text, kind, trigraphs, |piece, _, _| {
        out.push_str(piece);
        true
    });
    out
}

/// Maps an offset into the [`clean`]ed spelling of a token back to an
/// offset into its source text `text` (as [`spelling`] would have cleaned
/// it): the offset of the first source byte of the character there, after
/// any line splices before it. An offset at or past the end of the
/// spelling maps to the end of `text`. Used to place
/// [`crate::literal::LitDiag`]s.
pub fn source_offset(text: &str, kind: TokKind, trigraphs: bool, spelling_offset: usize) -> usize {
    let src = Src::new(text, trigraphs);
    let mut out = 0usize;
    let mut found = text.len();
    walk(text, kind, trigraphs, |piece, p, verbatim| {
        if spelling_offset < out + piece.len() {
            // Verbatim runs map byte for byte; a decoded character maps to
            // its source position after splices.
            found = if verbatim { p + (spelling_offset - out) } else { src.skip_splices(p) };
            return false;
        }
        out += piece.len();
        true
    });
    found
}

/// Calls `f(piece, source_offset, verbatim)` for each piece of the cleaned spelling
/// of `text`: one decoded character at a time, except that a raw string's
/// delimiter and body are passed as one verbatim piece. Stops early when
/// `f` returns `false`.
#[allow(clippy::many_single_char_names, reason = "short-lived scanner state")]
fn walk(text: &str, kind: TokKind, trigraphs: bool, mut f: impl FnMut(&str, usize, bool) -> bool) {
    let src = Src::new(text, trigraphs);
    let mut raw = matches!(kind, TokKind::RawStringLit(_));
    let mut p = 0;
    let mut buf = [0u8; 4];
    while p < text.len() {
        let (c, s) = src.decode(p);
        let Some(c) = c else { break };
        if !f(c.encode_utf8(&mut buf), p, false) {
            return;
        }
        p += s.max(1);
        if raw && c == '"' {
            raw = false;
            // The delimiter and body are copied verbatim.
            let rest = text.get(p..).unwrap_or("");
            let body_len = rest.find('(').and_then(|open| {
                let delim = &rest[..open];
                let close = format!("){delim}\"");
                rest[open..].find(&close).map(|k| open + k + close.len())
            });
            let n = body_len.unwrap_or(rest.len());
            if n > 0 && !f(&rest[..n], p, true) {
                return;
            }
            p += n;
        }
    }
}
