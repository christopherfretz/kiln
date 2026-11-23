//! The token table (spec/parse.md §2.6, WP-2825): with
//! [`crate::ParseOptions::token_table`], every token the preprocessor
//! delivers to the parser, in delivery order, for tooling clients that
//! ask "does a token start here" without lexing a buffer a second time.
//!
//! The token source records one [`TokenRow`] per delivered token (the
//! kind the parser sees, the location, the length of the spelling, the
//! macro flag), at one predictable branch per token when the option is
//! off. At the end of the parse [`TokenTable::build`] replaces each token
//! the parser split ([`kiln_ast::Ast::split_greaters`]) by the pieces it
//! consumed, marks the tokens of macro arguments, and indexes the tokens
//! written in each file for [`TokenTable::starts_token`] and
//! [`TokenTable::token_at`].

use kiln_lex::{Punct, TokKind, Token};
use kiln_source::fx::{FxHashMap, FxHashSet};
use kiln_source::{ExpansionKind, FileId, Loc, SourceManager};

/// The length bits of [`TokenRow::bits`].
const LEN_MASK: u32 = (1 << 30) - 1;
/// The token comes from a macro expansion (its location is a macro
/// location).
const MACRO: u32 = 1 << 31;
/// The token comes from a macro argument (its location's own expansion
/// entry is a macro argument's).
const MACRO_ARG: u32 = 1 << 30;

/// One token as the parser received it: 16 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenRow {
    kind: TokKind,
    loc: Loc,
    /// The length (low 30 bits) and the [`MACRO`] and [`MACRO_ARG`]
    /// flags.
    bits: u32,
}

const _: () = assert!(size_of::<TokenRow>() == 16);

impl TokenRow {
    /// The row of the delivered token `t`: the macro-argument flag is
    /// set later ([`TokenTable::build`]).
    #[inline]
    pub(crate) fn of(t: &Token) -> TokenRow {
        let len = t.span.hi.to_raw().saturating_sub(t.span.lo.to_raw()).min(LEN_MASK);
        let flag = if t.span.lo.is_macro() { MACRO } else { 0 };
        TokenRow { kind: t.kind, loc: t.span.lo, bits: len | flag }
    }

    fn piece(kind: Punct, loc: Loc, len: u32, from: TokenRow) -> TokenRow {
        TokenRow { kind: TokKind::Punct(kind), loc, bits: len | (from.bits & !LEN_MASK) }
    }

    /// The kind the parser saw: an alternative token (`and`, `bitor`,
    /// ...) is its punctuator, a keyword an identifier, a digraph its
    /// primary punctuator; a piece of a split token is the piece's
    /// punctuator.
    pub fn kind(&self) -> TokKind {
        self.kind
    }

    /// Where the token starts: a file location, or a macro location for
    /// a token from a macro expansion (as the parser's tokens and the
    /// AST's locations are).
    pub fn loc(&self) -> Loc {
        self.loc
    }

    /// The length in bytes of the token's spelling (line splices inside
    /// it included), measured in its location's space; 1 for the first
    /// `>` of a split token.
    pub fn len(&self) -> u32 {
        self.bits & LEN_MASK
    }

    /// True for a zero-length token (none is delivered; for symmetry
    /// with [`Self::len`]).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The token comes from a macro expansion ([`Self::loc`] is a macro
    /// location): a token of a replacement list, of a macro argument, or
    /// made by `#`, `##` or a builtin macro.
    pub fn from_macro(&self) -> bool {
        self.bits & MACRO != 0
    }

    /// The token was substituted for a macro parameter: its location's
    /// own expansion entry is a macro argument's
    /// ([`ExpansionKind::MacroArg`]).
    pub fn from_macro_arg(&self) -> bool {
        self.bits & MACRO_ARG != 0
    }
}

/// The tokens of a parsed unit ([`crate::ParsedTu::token_table`]).
#[derive(Clone, Debug, Default)]
pub struct TokenTable {
    rows: Vec<TokenRow>,
    /// Tokens the preprocessor delivered ([`crate::ParsedTu::tokens`]).
    delivered: u64,
    /// Per file, the file tokens' `(offset, row)`, sorted.
    files: FxHashMap<FileId, Vec<(u32, u32)>>,
}

impl TokenTable {
    /// The table of the delivered tokens `rows` (in delivery order): the
    /// tokens split at a location of `splits` replaced by their pieces,
    /// macro-argument tokens marked, the file tokens indexed.
    pub(crate) fn build(
        mut rows: Vec<TokenRow>,
        splits: &FxHashSet<Loc>,
        sm: &SourceManager,
    ) -> TokenTable {
        let delivered = u64::try_from(rows.len()).unwrap_or(u64::MAX);
        if !splits.is_empty() && rows.iter().any(|r| is_split(r, splits)) {
            rows = expand_splits(&rows, splits);
        } else {
            rows.shrink_to_fit();
        }
        let mut files: FxHashMap<FileId, Vec<(u32, u32)>> = FxHashMap::default();
        for (i, r) in rows.iter_mut().enumerate() {
            let Ok(i) = u32::try_from(i) else { break };
            let at = if r.loc.is_macro() {
                let Some(e) = sm.expansion_info(r.loc) else { continue };
                if e.kind != ExpansionKind::MacroArg {
                    continue;
                }
                r.bits |= MACRO_ARG;
                argument_spelling(sm, r.loc)
            } else {
                Some(r.loc)
            };
            if let Some((f, off)) = at.and_then(|l| sm.offset_of(l)) {
                files.entry(f).or_default().push((off, i));
            }
        }
        for v in files.values_mut() {
            v.sort_unstable();
            v.shrink_to_fit();
        }
        TokenTable { rows, delivered, files }
    }

    /// The rows, in the order the parser consumed them.
    pub fn rows(&self) -> &[TokenRow] {
        &self.rows
    }

    /// The number of rows: the delivered tokens plus one for each split
    /// (a split `>>`, `>=` or `>>=` is two rows; a `>>=` split twice is
    /// three).
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// True when no token was delivered.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Row `i`.
    pub fn get(&self, i: usize) -> Option<&TokenRow> {
        self.rows.get(i)
    }

    /// The tokens the preprocessor delivered ([`crate::ParsedTu::tokens`]),
    /// before the splits.
    pub fn delivered(&self) -> u64 {
        self.delivered
    }

    /// The file tokens of `file`, by offset: `(offset, row)`. A file
    /// token is a token whose location is in `file`, or a macro-argument
    /// token written in `file` as the argument (every link of its
    /// spelling chain a macro argument's), not a token of a replacement
    /// list. An offset repeats when the file was entered twice or an
    /// argument expanded twice.
    pub fn file_tokens(&self, file: FileId) -> impl Iterator<Item = (u32, usize)> + '_ {
        self.files.get(&file).into_iter().flatten().map(|&(o, r)| (o, r as usize))
    }

    /// True if a file token of `file` starts at byte `offset`
    /// (O(log n)).
    pub fn starts_token(&self, file: FileId, offset: u32) -> bool {
        self.files.get(&file).is_some_and(|v| {
            let i = v.partition_point(|&(o, _)| o < offset);
            v.get(i).is_some_and(|&(o, _)| o == offset)
        })
    }

    /// The row of the file token of `file` whose spelling holds byte
    /// `offset` (the first one in delivery order when several were
    /// spelled there); `None` in whitespace, comments, directives,
    /// skipped groups and replacement lists (O(log n)).
    pub fn token_at(&self, file: FileId, offset: u32) -> Option<usize> {
        let v = self.files.get(&file)?;
        let end = v.partition_point(|&(o, _)| o <= offset);
        let &(start, _) = v.get(end.checked_sub(1)?)?;
        let first = v.partition_point(|&(o, _)| o < start);
        let &(o, row) = v.get(first)?;
        let len = self.rows.get(row as usize)?.len();
        (offset - o < len).then_some(row as usize)
    }

    /// Bytes held: the rows and the file index (`-fkiln-memory-report`'s
    /// accounting; spec/parse.md §2.6 "Cost").
    pub fn heap_bytes(&self) -> u64 {
        let rows = self.rows.capacity() * size_of::<TokenRow>();
        let index: usize = self
            .files
            .values()
            .map(|v| {
                v.capacity() * size_of::<(u32, u32)>() + size_of::<(FileId, Vec<(u32, u32)>)>()
            })
            .sum();
        u64::try_from(rows + index).unwrap_or(u64::MAX)
    }
}

/// The punctuator left after the parser takes the first `>` of `p`
/// ([temp.names]/4), as `Parser::split_greater` makes it.
fn rest_of(p: Punct) -> Option<Punct> {
    match p {
        Punct::GreaterGreater => Some(Punct::Greater),
        Punct::GreaterEqual => Some(Punct::Equal),
        Punct::GreaterGreaterEqual => Some(Punct::GreaterEqual),
        _ => None,
    }
}

fn is_split(r: &TokenRow, splits: &FxHashSet<Loc>) -> bool {
    matches!(r.kind, TokKind::Punct(p) if rest_of(p).is_some()) && splits.contains(&r.loc)
}

/// `rows` with each split token replaced by the pieces the parser
/// consumed: the `>` it took (length 1) and the rest, itself split again
/// when the rest is a `>=` whose `>` was taken too.
fn expand_splits(rows: &[TokenRow], splits: &FxHashSet<Loc>) -> Vec<TokenRow> {
    let extra = rows.iter().filter(|r| is_split(r, splits)).count();
    let mut out = Vec::with_capacity(rows.len() + 2 * extra);
    for &r in rows {
        let mut cur = r;
        while let TokKind::Punct(p) = cur.kind
            && let Some(rest) = rest_of(p)
            && splits.contains(&cur.loc)
        {
            out.push(TokenRow::piece(Punct::Greater, cur.loc, 1, r));
            cur = TokenRow::piece(rest, cur.loc.with_offset(1), cur.len().saturating_sub(1), r);
        }
        out.push(cur);
    }
    out.shrink_to_fit();
    out
}

/// Where the macro-argument token at `loc` was written as an argument:
/// the end of its spelling chain when every link is a macro argument's
/// (a file location), `None` when a link is a replacement list's, a
/// `#`/`##` result's or an `#embed`'s.
fn argument_spelling(sm: &SourceManager, mut loc: Loc) -> Option<Loc> {
    while loc.is_macro() {
        if sm.expansion_info(loc)?.kind != ExpansionKind::MacroArg {
            return None;
        }
        loc = sm.immediate_spelling_loc(loc);
    }
    loc.is_valid().then_some(loc)
}
