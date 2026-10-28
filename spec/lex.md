# spec: kiln_lex — translation phases 1–3

Standard: [lex.phases] phases 1–3, [lex.charset], [lex.pptoken], [lex.comment],
[lex.header], [lex.ppnumber], [lex.name], [lex.universal.char],
[lex.operators], [lex.digraph], [lex.ccon]/[lex.string]/[lex.ext] (token
*extent* only — literal contents and values are WP-011 / kiln_sema /
kiln_oven). Keyword classification ([lex.key]) is the parser's job.

Status: implemented by WP-010 (crate `crates/kiln_lex`). Items marked
**WP-011** are stubs owned by that work package.

## Scope
Input: one buffer (a `FileId` of kiln_source, whole or a byte range).
Output: a pull stream of **pp-tokens** with spans and flags, plus
diagnostics **as data**. The lexer never expands macros, never processes
directives and never evaluates literals. kiln_pp drives it one token at a
time and steers it with two controls: `in_directive` and `LexMode`.

`kiln_lex` depends only on `kiln_source`. It does **not** depend on
`kiln_diag`: it records `LexDiag { kind, span }` and kiln_pp maps each
`LexDiagKind` to a `kiln_diag` id (and drops them in skipped groups).

## Interface
```rust
pub struct Lexer<'a> { /* private */ }
impl<'a> Lexer<'a> {
    // `base` = Loc of byte 0 of the file text (sm.loc(file, 0), or the
    // per-inclusion range base); token Loc = base.with_offset(byte offset).
    pub fn new(sm: &'a SourceManager, interner: &'a Interner, file: FileId,
               base: Loc, opts: LexOptions) -> Self;
    // Sub-range (scratch buffers, _Pragma): panics if the range is not on
    // char boundaries. BOM skipped only if range starts at 0. No NoNewlineAtEof.
    pub fn new_in_range(sm, interner, file, range: Range<u32>, base, opts) -> Self;
    pub fn next(&mut self) -> Token;            // Eof forever after the end
    pub fn set_mode(&mut self, LexMode);  pub fn mode(&self) -> LexMode;
    pub fn set_in_directive(&mut self, bool); pub fn in_directive(&self) -> bool;
    pub fn set_keep_comments(&mut self, bool);
    pub fn spelling(&self, &Token) -> Cow<'a, str>;
    pub fn diagnostics(&self) -> &[LexDiag]; pub fn take_diagnostics(&mut self) -> Vec<LexDiag>;
    pub fn offset(&self) -> u32; // + options(), file(), interner(), source_manager()
}
pub struct Token { pub kind: TokKind, pub span: Span, pub flags: TokFlags } // Copy, <= 20 bytes
pub enum TokKind {
    Identifier(Symbol),     // interned, UCNs resolved ([lex.name]/1: same identifier)
    PpNumber,
    CharLit(Encoding), StringLit(Encoding), RawStringLit(Encoding), // incl. ud-suffix
    HeaderName,             // only in LexMode::HeaderName
    Punct(Punct),           // digraphs map to the primary Punct + DIGRAPH flag
    Other(char),            // [lex.pptoken]/2 single character (ill-formed if non-ASCII)
    Unknown,                // recovery token: unterminated/empty literal, bad UCN
    Comment,                // only with keep_comments
    Eod, Eof,
}
pub enum Encoding { Ordinary, Wide, Utf8, Utf16, Utf32 }
pub struct TokFlags(u8); // LEADING_SPACE START_OF_LINE NEEDS_CLEANING DIGRAPH HAS_UCN UD_SUFFIX
pub struct LexOptions { std: Std, trigraphs: bool /*false*/, dollar_in_identifiers: bool /*true*/,
                        keep_comments: bool /*false*/ }   // Default: Std::Cxx26
pub enum LexMode { Normal, HeaderName }
pub struct Interner;  // thread-safe (&self), Symbol(u32); shared by all lexers of a TU
pub fn spelling(sm, &Token, &LexOptions) -> Cow<str>; pub fn clean(text, TokKind, trigraphs) -> String;
pub fn dump(sm, &LexOptions, &[Token]) -> String; pub fn dump_source(text, LexOptions) -> String;
```
`Punct` covers every [lex.operators] token plus `^^`, `[:`, `:]` (C++26,
P2996). `Punct::as_str()` is the primary spelling, `name()` clang's kind name.

### Allocation
`next()` does not allocate except: interning a new identifier spelling;
identifiers containing UCNs, `$` or splices (built in a reused scratch
buffer); recording a diagnostic. `spelling()` borrows unless
`NEEDS_CLEANING`.

## Behavior
### Phases 1–2 ([lex.phases]/1.1–1.2)
- Input is UTF-8 (kiln_source guarantees it). A BOM at offset 0 is skipped.
- Lines end in `\n`, `\r\n` or lone `\r` (kiln_source's rule).
- Trigraphs are translated only with `trigraphs: true` (pre-C++17 mode):
  `TrigraphConverted`. With them off, a trigraph inside a literal or a
  `??x` punctuator sequence records `TrigraphIgnored`. A trigraph is never
  formed across a splice (phase 1 precedes phase 2).
- Line splice: `\` (or `??/` with trigraphs), optional horizontal whitespace
  (`BackslashNewlineSpace`, still spliced, as clang), newline. Splices may
  occur anywhere, including inside tokens and `//` comments. A splice at end
  of file records `BackslashNewlineEof`. A splice is not whitespace.
- Splicing is not applied inside a raw string body ([lex.pptoken]/3.1): the
  body is lexed from the raw bytes.
- Phase-2 work is not done during decoding of the common case: tokens are
  scanned after lexing and `NEEDS_CLEANING` is set iff a splice (or a
  translated trigraph) occurs in the token's bytes outside a raw body.
- A file not ending in newline records `NoNewlineAtEof` (whole-file lexers
  only).
- NUL is whitespace (`NullCharacter`). Non-ASCII whitespace (U+0085, U+00A0,
  U+1680, U+180E, U+2000–200A, U+2028/9, U+202F, U+205F, U+3000) is
  whitespace with `UnicodeWhitespace` (clang's compile-mode extension).

### Phase 3 ([lex.pptoken])
- Maximal munch, with the standard's exceptions:
  - `<::` not followed by `:` or `>` is `<` `::` (C++11+).
  - C++26: `[::` not followed by `:` is `[` `::`; `[:>` is `[` `:>`; `[:`
    and `:]` are splice brackets; `^^` is one token. Before C++26 these
    lex as their components.
  - `<=>` is one token from C++20; before, `<=` `>` with `SpaceshipCompat`.
  - Raw strings: prefix `R`, `LR`, `u8R`, `uR`, `UR` (C++11+); delimiter of
    ≤16 d-chars (C++26 adds `$ @ \``) — too long or invalid char: diagnose,
    skip to the next `"`, produce `Unknown`. No `)delim"` before end of input:
    `UnterminatedRawString`, `Unknown` to end of file.
- Literal prefixes: `L` always; `u`/`U`/`u8` strings and raw strings C++11+;
  `u8` characters C++17+. A prefix not available is an identifier.
- Quoted literals end at the matching quote; `\` skips the next character.
  A newline or end of file first: `Unterminated{String,Char}` and an `Unknown`
  token to end of line. `''`: `EmptyCharLiteral`, `Unknown`.
- ud-suffix (C++11+, [lex.ext]): an identifier immediately after a literal
  is part of it if it starts with `_`, or is a standard suffix (`s`, `sv`
  after strings; clang's rule); otherwise `ReservedUdSuffix` and the
  identifier is a separate token. Sets `UD_SUFFIX`.
- pp-number ([lex.ppnumber]): `digit` or `. digit`, then identifier-continue
  characters (incl. UCNs), `.`, `e±`/`E±`, `p±`/`P±` (C++17+; before, only
  when the number is a hexadecimal float, the clang extension), `'` followed
  by a digit or nondigit (C++14+).
- Identifiers ([lex.name]): `XID_Start`/`_` then `XID_Continue` (Unicode
  15.1 tables in `unicode_tables.rs`, generated by
  `scripts/gen-unicode-tables.py`), UCNs (`\uXXXX`, `\UXXXXXXXX`, `\u{…}`,
  `\N{…}`), and `$` (if enabled, `DollarInIdentifier`). A non-ASCII
  `XID_Continue`-only character at token start is a lone token
  (`CharNotAllowedInIdentifier`; UTF-8 → `Other`, UCN → `Unknown`). Inside
  an identifier, a disallowed character is kept with
  `CharNotAllowedInIdentifier` (clang's recovery). NFC_QC=No characters
  record `IdentifierNotNfc`.
- UCNs outside literals ([lex.universal.char]): surrogates, values above
  U+10FFFF and non-identifier characters → `UcnInvalidCodePoint`; basic or
  control characters other than `$ @ \`` → `UcnBasicOrControl`; delimited/named
  forms before C++23 record `Ucn{Delimited,Named}Extension`. An invalid UCN
  inside an identifier stays in it (diagnosed); at token start it is
  `Unknown`. A `\` that does not start a UCN is `Other('\\')`.
- Any other character is `Other(c)`; non-ASCII ones record `InvalidChar`.

### Comments ([lex.comment])
`//` to end of line (splices continue it; `MultiLineLineComment` once
unless the next line also starts with `//`). `/* … */`: first `*/` ends it,
even when split by a splice (`EscapedNewlineBlockCommentEnd`); `/*/` does
not close; `/*` inside records `NestedBlockComment`; unterminated →
`UnterminatedBlockComment`, runs to end of file. A comment is whitespace
(sets `LEADING_SPACE`) unless `keep_comments`, which returns `Comment`
tokens with the usual flags.

### Flags
- `START_OF_LINE`: first token of the buffer or after a newline that is not
  inside a comment or spliced. A multi-line block comment does not start a
  line.
- `LEADING_SPACE`: whitespace, NUL, Unicode whitespace or a comment precedes
  the token on its line.
- `NEEDS_CLEANING`, `DIGRAPH`, `HAS_UCN` (spelled with a UCN), `UD_SUFFIX`.
  These and the two above are exact; `-E` printing and `#`/`##` use them.

### Directive and header-name protocol
- `in_directive`: the next newline (not consumed) or end of input yields
  `Eod`; returning `Eod` clears `in_directive`. kiln_pp sets it after a
  `START_OF_LINE` `#`. Do not set it again after an `Eod` at end of input
  (that would yield `Eod` forever).
- `LexMode::HeaderName` applies to exactly the next token (`Comment` tokens
  do not count): `<…>` or `"…"` on one line becomes `HeaderName` (inside
  `"…"`, `\` is an ordinary character, [lex.header]); otherwise the token
  lexes normally. kiln_pp sets it after `include`, `embed`,
  `__has_include(`, `__has_embed(`, `import`. Module directives need nothing
  more from the lexer than exact `START_OF_LINE`.

### Spelling
`spelling` = source text with splices removed and (if enabled) trigraphs
translated, UCNs kept as written; the raw body of a raw string is verbatim.
`clean(spelling(t), t.kind, false) == spelling(t)`. Identifiers compare by
`Symbol`, which interns the UCN-resolved text.

### Diagnostics
`LexDiag { kind: LexDiagKind, span }` (`#[non_exhaustive]` enum; each
variant's doc names the clang diagnostic it mirrors).
`LexDiagKind::severity()` gives clang's default class (`Error`, `Warning`,
`Extension` = pedantic/off by default). Recovery never panics on any input;
every token consumes at least one byte except `Eod`/`Eof`.

### Dump format (`dump`, `dump_source`; used by the goldens)
One line per token: `kind 'spelling' [StartOfLine] [LeadingSpace] [UnClean]
[Digraph] [UCN] [UDSuffix]`, with `\n`, `\r`, `\t` escaped. Kind names
follow clang's `-dump-tokens` (`identifier` for keywords too) except the
kiln-only `raw_string_literal` (`wide_raw_string_literal`, …), `other`,
`unknown`, `header_name`, `comment`, `eod`, `eof`.

## Deviations from clang (intentional; all in error or extension territory)
- `p±` in a pp-number from C++17 regardless of hex prefix (standard; clang
  splits `1p+2`).
- C++26 reflection tokens (`^^`, `[:`, `:]`) — clang 19 lacks them.
- Unicode whitespace is always whitespace (clang keeps it as an unknown
  token in `-E` output).
- Stray non-ASCII → `Other(c)` + `InvalidChar` (clang: `unknown`).
- ` `-style UCN for whitespace → `Unknown` (clang treats as whitespace).
- In a `"…"` header name `\` is literal (clang treats it as an escape).
- NFC: only NFC_QC=No is checked (Maybe needs full normalization; TODO).
- No `FormFeedInDirective`/`VerticalTabInDirective`; no clang mathematical
  notation identifier extension; no C mode.

## Performance obligations
- Zero allocation per token (see Allocation).
- Lexing all libc++ headers in under 1.5× clang's raw lexer time; measured
  by the bench harness once it carries a lexer benchmark.

## Tests
- Per-rule tests: `crates/kiln_lex/tests/*.rs` (one file per area; each
  test names its standard section).
- Goldens: in-crate (`src/golden_tests.rs`), `dump_source` output
  cross-checked against `clang-19 -cc1 -std=c++26 -dump-tokens` modulo the
  deviations above.
- Fuzz: `cargo test -p kiln_lex -- fuzz` — bounded random input (raw bytes
  and token-piece soup) under several option sets: no panic, progress,
  ordered spans, `spelling == clean(raw) == clean(spelling)`. Longer runs:
  `KILN_LEX_FUZZ_ITERS`, `KILN_LEX_FUZZ_SEED`, `KILN_LEX_FUZZ_LEN`.
- Later: `kiln -E -P` vs `clang -E -P` differential (tests/differential/pp/).

## WP-011 (literals) — what is stubbed and where (`crates/kiln_lex/src`)
- `lexer.rs::lex_quoted`: escapes are only *skipped* for termination; no
  validation of simple/octal/hex/`\o{}`/`\x{}`/`\u`/`\N` escapes, no
  multichar or encoding-range checks, no NUL-in-literal policy beyond a
  diagnostic.
- `lexer.rs::lex_raw_string`: body not inspected (fine per standard);
  delimiter rules implemented.
- `lexer.rs::scan_header_name`: no diagnostics for `'`, `\`, `//`, `/*` in
  header names (conditionally-supported, [lex.header]/2).
- `unicode.rs::lookup_char_name`: `\N{}` resolves only `CJK UNIFIED
  IDEOGRAPH-XXXX` / `CJK COMPATIBILITY IDEOGRAPH-XXXX`; needs the full UCD
  name table (+ aliases, Hangul syllables, loose matching rules of
  [lex.universal.char]).
- Literal values / encodings: not computed here (kiln_sema/kiln_oven),
  but the escape decoder WP-011 writes should live in `kiln_lex` so both can
  share it.
- `#embed` resource lexing is kiln_pp's; HeaderName mode already serves it.
