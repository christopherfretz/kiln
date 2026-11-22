//! The end of one token, by lexing it (spec/lex.md "End of a token"): the
//! query tooling clients use to turn a token's start into its range.

use kiln_source::{Loc, SourceManager};

use crate::{Interner, LexMode, LexOptions, Lexer, TokKind};

/// The end (exclusive) of the preprocessing token that starts at `loc`,
/// found by lexing exactly one token there under `opts` ([lex.pptoken]:
/// the longest sequence of characters that forms a token, with phase 1
/// and 2 applied, so a token continued over a `\`-newline ends after its
/// last spliced character).
///
/// - A file location: the token is lexed from `loc` in normal mode (no
///   header-name; comments are skipped). `None` when `loc` is the end of
///   the buffer, not on a character boundary, or when the first token
///   lexed from there does not start at `loc` (whitespace or a comment is
///   there). The end is a location in the same buffer range as `loc` (an
///   inclusion range stays one).
/// - A macro location: the token is measured where it is spelled
///   (`SourceManager::spelling_loc`, a file location, also a scratch
///   buffer for a `##` or `#` result) and the end is that many bytes past
///   `loc` in `loc`'s own expansion entry, so that `spelling_loc` of the
///   end is the end of the spelled token. `None` when the end would leave
///   the entry. This is not clang's `Lexer::getLocForEndOfToken`, which
///   gives a macro location an end only at the end of its expansion;
///   compose with `SourceManager::file_loc_at_macro_end` for that.
/// - Invalid and unknown locations: `None`.
///
/// Identifiers are interned into `interner`; lexer diagnostics are
/// dropped.
pub fn token_end(
    sm: &SourceManager,
    interner: &Interner,
    loc: Loc,
    opts: LexOptions,
) -> Option<Loc> {
    if loc.is_macro() {
        let sp = sm.spelling_loc(loc);
        if sp.is_macro() {
            return None;
        }
        let len = sm.same_entry_distance(sp, token_end(sm, interner, sp, opts)?)?;
        let end = Loc::from_raw(loc.to_raw().checked_add(len)?);
        return sm.same_entry_distance(loc, end).map(|_| end);
    }
    let (file, pos) = sm.offset_of(loc)?;
    let text = sm.text(file);
    if pos as usize >= text.len() || !text.is_char_boundary(pos as usize) {
        return None;
    }
    let base = Loc::from_raw(loc.to_raw() - pos);
    let mut o = opts;
    o.keep_comments = false;
    let len = u32::try_from(text.len()).ok()?;
    let mut lx = Lexer::new_in_range(sm, interner, file, pos..len, base, o);
    lx.set_mode(LexMode::Normal);
    let t = lx.next();
    (t.kind != TokKind::Eof && t.span.lo == loc).then_some(t.span.hi)
}
