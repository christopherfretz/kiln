//! The end of one token, by lexing it (spec/lex.md "End of a token"): the
//! query tooling clients use to turn a token's start into its range.

use std::collections::HashSet;
use std::hash::BuildHasher;

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

/// [`token_end`] aware of the parser's split `>` tokens (spec/lex.md
/// "Split `>` tokens"): a location in `split_greaters` (the unit's
/// `kiln_ast::Ast::split_greaters`, where the parser took the first `>`
/// of a `>>`, `>=`, `>>=` or `>>>` as the end of a template argument
/// list, [temp.names]/4) is a one-character `>` token, as sema and clang
/// measure it; every other location is [`token_end`]'s. `None` as for
/// [`token_end`], and for a split location that is not a `>`.
pub fn token_end_split<S: BuildHasher>(
    sm: &SourceManager,
    interner: &Interner,
    loc: Loc,
    opts: LexOptions,
    split_greaters: &HashSet<Loc, S>,
) -> Option<Loc> {
    if split_greaters.contains(&loc) {
        let sp = sm.spelling_loc(loc);
        let (file, pos) = sm.offset_of(sp)?;
        if sm.text(file).as_bytes().get(pos as usize) != Some(&b'>') {
            return None;
        }
        let end = Loc::from_raw(loc.to_raw().checked_add(1)?);
        return sm.same_entry_distance(loc, end).map(|_| end);
    }
    token_end(sm, interner, loc, opts)
}

/// The bytes of one file that the tokens from the one at `first` through
/// the one at `last` are written as: `(file, begin, end)` with `end`
/// exclusive, or `None` (spec/lex.md "Token ranges", spec/source.md
/// "Editable file ranges").
///
/// `last`'s token is measured by [`token_end`], and the range
/// `[first, end)` is mapped by `SourceManager::editable_file_range`, the
/// rule fix-its use: two file locations give themselves; ends in one use
/// of one macro argument give where the argument is written; a range
/// that begins (ends) every expansion it begins (ends) in gives the
/// invocations, from the macro name through the closing parenthesis (the
/// name alone for an object-like macro). Anything else (an end inside a
/// replacement list, ends in two arguments, a token made by `#` or `##`
/// that is not the whole expansion, a range across files) is `None`, as
/// are invalid locations and a `last` that starts no token. A range one
/// token long is `token_range(sm, interner, at, at, opts)`.
pub fn token_range(
    sm: &SourceManager,
    interner: &Interner,
    first: Loc,
    last: Loc,
    opts: LexOptions,
) -> Option<(kiln_source::FileId, u32, u32)> {
    let end = token_end(sm, interner, last, opts)?;
    file_range(sm, first, end)
}

/// [`token_range`] with `last`'s token measured by [`token_end_split`]:
/// a range that ends at a split `>` (`vt<1>` in `B<vt<1>> y;`) ends
/// after its first character.
pub fn token_range_split<S: BuildHasher>(
    sm: &SourceManager,
    interner: &Interner,
    first: Loc,
    last: Loc,
    opts: LexOptions,
    split_greaters: &HashSet<Loc, S>,
) -> Option<(kiln_source::FileId, u32, u32)> {
    let end = token_end_split(sm, interner, last, opts, split_greaters)?;
    file_range(sm, first, end)
}

fn file_range(sm: &SourceManager, first: Loc, end: Loc) -> Option<(kiln_source::FileId, u32, u32)> {
    let (file, begin, end) = sm.editable_file_range(kiln_source::Span::new(first, end))?;
    (begin < end).then_some((file, begin, end))
}
