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
    let src = Src::new(text, trigraphs);
    let mut out = String::with_capacity(text.len());
    let mut raw = matches!(kind, TokKind::RawStringLit(_));
    let mut p = 0;
    while p < text.len() {
        let (c, s) = src.decode(p);
        let Some(c) = c else { break };
        out.push(c);
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
            out.push_str(&rest[..n]);
            p += n;
        }
    }
    out
}
