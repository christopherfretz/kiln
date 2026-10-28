# spec: kiln_lex — translation phases 1–3

Standard: [lex.phases] phases 1–3, [lex.charset], [lex.pptoken], [lex.comment],
[lex.header], [lex.ppnumber], [lex.name], [lex.universal.char],
[lex.operators], [lex.digraph], [lex.ccon]/[lex.string]/[lex.ext],
[lex.icon]/[lex.fcon] (classification and values, module
`kiln_lex::literal`; types and exact floating values are kiln_sema's /
kiln_oven's). Keyword classification ([lex.key]) is the parser's job.

Status: implemented by WP-010 (pp-tokens) and WP-011 (literals, `\N{}`
names, header-name diagnostics, full NFC check) in `crates/kiln_lex`.

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
    pub fn skip_plain_line_rest(&mut self) -> bool; // skipped-group fast path (WP-056)
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
pub fn source_offset(text, TokKind, trigraphs, spelling_offset: usize) -> usize; // spelling -> source byte
pub enum NameMatch { Exact(char), Loose(char) }        // + .char()
pub fn lookup_char_name(name: &str) -> Option<NameMatch>; // \N{} names; never allocates
pub mod literal;                                         // see "Literals" below
pub fn dump(sm, &LexOptions, &[Token]) -> String; pub fn dump_source(text, LexOptions) -> String;
```
`Punct` covers every [lex.operators] token plus `^^`, `[:`, `:]` (C++26,
P2996). `Punct::as_str()` is the primary spelling, `name()` clang's kind name.

### Allocation
`next()` does not allocate except: interning a new identifier spelling;
identifiers containing UCNs, `$` or splices (built in a reused scratch
buffer); the NFC check of an identifier with a non-ASCII character that is
not NFC_QC=Yes (full normalization); recording a diagnostic. `spelling()` borrows unless
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
  `CharNotAllowedInIdentifier` (clang's recovery). An identifier that is not in NFC records
  `IdentifierNotNfc`: NFC_QC=Yes-only text is accepted without work,
  otherwise the identifier is normalized (UAX #15, tables generated from
  UCD 15.1) and compared.
- `\N{name}` ([lex.universal.char]/3): the name or any alias of
  NameAliases.txt (all five types), Hangul syllable names, and the NR2
  ranges (`CJK UNIFIED IDEOGRAPH-XXXX`, `TANGUT IDEOGRAPH-XXXX`, …),
  matched exactly.
  A name that matches only under UAX #44-LM2 loose matching (case,
  spaces, `_`, medial `-`) is the named character with the error
  `UcnNameLooseMatch` (clang's recovery); no match is `UcnUnknownName`.
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
- `skip_plain_line_rest` (for skipped groups, [cpp.cond]/13): when the
  lexer is in `LexMode::Normal` and not at the start of a line, and the
  logical line provably ends at the next `\n`/`\r`, it moves to that
  terminator and returns true; otherwise it changes nothing and returns
  false. Only a block comment, a raw string literal or a line splice can
  carry a logical line past a terminator, so the rest of the line (`R`)
  qualifies when it is non-empty and holds no `R"`, no `??/` (trigraphs
  on), no `\` as its last non-blank byte, and every `/*` in it is
  followed on the line by a `*/` whose preceding byte is not a `/` that
  could itself open a comment (`/*/`) -- rules that hold whether a given
  `/*` is a comment or sits inside a literal. Afterwards `next()` yields
  exactly the tokens (kinds, spans, flags, `Eod` placement) it would have
  yielded after lexing the skipped tokens (the next token is an `Eod`, a
  line-initial token or `Eof`; only `Eof`'s `LEADING_SPACE` may differ).
  No diagnostics or interning happen for the skipped bytes. kiln_pp uses
  it only where the skipped tokens are discarded unseen (excluded groups;
  the rest of a directive line in a skipped group).
- [lex.header]/2: each `'`, `\`, `//`, `/*` in a header name (and `"` in
  `<…>`) is conditionally-supported and accepted with
  `HeaderNameSpecialChars { seq }` (Extension) on that sequence.

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
- In a `"…"` header name `\` is literal (clang treats it as an escape);
  kiln diagnoses it with `HeaderNameSpecialChars` (clang has no such
  diagnostic; it is Extension, off by default).
- `\N{}` accepts all five alias types (clang 19: correction, control,
  alternate only); a loose-matched name of a basic or control character
  records only `UcnBasicOrControl` (clang reports both).
- Literal-module deviations are listed under "Literals".
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
- Literals: `tests/literals_escapes.rs`, `literals_values.rs`,
  `literals_numbers.rs`, each test tagged with its section; values
  checked against clang-19 (`static_assert`/`#if`). In-crate goldens
  `golden_lex_literals_tokens` / `golden_lex_literals_values`.
- Unicode: every table name and alias round-trips (exact and loose), all
  11,172 Hangul syllables, NR2 ranges. The full UAX #15 conformance file
  runs with `KILN_UCD_NORMALIZATION_TEST=<path to NormalizationTest.txt>
  cargo test -p kiln_lex -- --ignored nfc`.
- Fuzz: `cargo test -p kiln_lex -- fuzz` — bounded random input (raw bytes
  and token-piece soup) under several option sets: no panic, progress,
  ordered spans, `spelling == clean(raw) == clean(spelling)`; every
  literal and pp-number token is also evaluated (diag ranges ordered and
  inside the spelling, `has_error` consistent, and an error-free unsuffixed
  string re-encoded with `encode_string` decodes to the same units).
  Longer runs: `KILN_LEX_FUZZ_ITERS`, `KILN_LEX_FUZZ_SEED`,
  `KILN_LEX_FUZZ_LEN`. Every fuzzed token also checks
  `NEEDS_CLEANING == (clean(raw) != raw)` (guards the clean-run cursor);
  `lex_phases_sparse_splices_fuzz` covers long clean runs with rare
  splices; `cpp_cond_skip_plain_line_rest_preserves_line_starts` checks
  that skipping never moves a line-initial token or an `Eod`.
- Later: `kiln -E -P` vs `clang -E -P` differential (tests/differential/pp/).

## Literals (`kiln_lex::literal`, [lex.ccon] [lex.string] [lex.icon] [lex.fcon] [lex.ext])
The lexer only finds where a literal ends (`\` skips the next character).
Its contents are evaluated on demand from the token's *spelling* by the
`literal` module, as clang's `LiteralSupport` does: a literal that is never
evaluated (skipped group, unused macro) is never diagnosed. kiln_pp
evaluates `#if` numbers and characters and `#line` operands; sema
evaluates the rest. Nothing panics on any string.

```rust
pub struct EncodingParams { pub char_signed: bool, pub wchar_bits: u8 /*16|32*/, pub wchar_signed: bool }
//   consts LINUX_X86_64 (Default), LINUX_AARCH64, WINDOWS
pub struct LiteralOptions { pub std: Std, pub params: EncodingParams } // new(); Default = C++26 + default params
pub struct LitDiag { pub kind: LexDiagKind, pub range: Range<u32> /* into the spelling */ }
impl LitDiag { pub fn locate(&self, tok: &Token, text: &str, opts: &LexOptions) -> LexDiag; }
pub enum CodeUnits { U8(Vec<u8>), U16(Vec<u16>), U32(Vec<u32>) } // len, is_empty, get, unit_bits, iter
pub struct StringLiteral<'s> { encoding, raw, units: CodeUnits, ud_suffix: Option<&'s str>, diags, has_error }
pub enum CharType { Char, Int, WChar, Char8, Char16, Char32 }
pub struct CharLiteral<'s> { encoding, ty, chars: u32, value: i64, pp_value: i64, pp_unsigned: bool,
                             ud_suffix, diags, has_error }
pub fn decode_string(spelling: &str, LiteralOptions) -> StringLiteral;           // normal + raw
pub fn decode_unevaluated_string(spelling: &str, LiteralOptions) -> StringLiteral; // [lex.string.uneval]
pub fn decode_char(spelling: &str, LiteralOptions) -> CharLiteral;
pub fn encode_string(&CodeUnits, Encoding) -> String; // a literal spelling with that value (round-trip)
pub fn classify_number(spelling: &str, Std) -> Number;
pub struct Number<'s> { class: NumberClass<'s>, diags, has_error }
pub enum NumberClass<'s> { Integer(IntegerLiteral<'s>), Floating(FloatLiteral<'s>), Invalid }
pub struct IntegerLiteral<'s> { radix: Radix, digits: Cow<'s, str>, suffix: IntSuffix, ud_suffix }
//   value() -> Option<u128>  (the #if value; None = IntegerTooLarge)
pub struct FloatLiteral<'s> { radix, int_digits, frac_digits, exponent: i64, suffix: FloatSuffix, ud_suffix }
//   parse_f64() / parse_f32(): correctly rounded (ties to even), inf on overflow
pub enum Radix { Binary, Octal, Decimal, Hex }  pub struct IntSuffix { unsigned, size: IntSize }
pub enum IntSize { None, Long, LongLong, Size }
pub enum FloatSuffix { None, F, L, F16, F32, F64, F128, BF16 }
```
All of this is additive to the WP-010 interface; the new `LexDiagKind`
variants are additions to a `#[non_exhaustive]` enum.

### Behavior
- Escapes ([lex.ccon]/3, [lex.string]): simple escapes; octal (1–3 digits)
  and `\o{…}`; hex `\x…` and `\x{…}` (delimited forms: C++23, earlier
  `DelimitedEscapeExtension`); `\u`/`\U`/`\u{}`/`\N{}` with
  [lex.universal.char] rules (inside literals any scalar value is allowed
  from C++11; before, basic and control characters are
  `UcnBasicOrControl`); `\e \E \( \{ \[ \%` (GNU, `NonstandardEscape`); anything else `UnknownEscape` (the character
  itself is the value). A numeric escape's value is truncated to the code
  unit width (`HexEscapeTooLarge`/`OctalEscapeTooLarge`).
- Encoding: ordinary and `u8` → UTF-8; `u` → UTF-16; `U` → UTF-32; `L`
  → UTF-16/32 per `wchar_bits`. Numeric escapes produce one code unit
  verbatim.
- Characters: one c-char that is one code unit → its value as `ty`. An
  ordinary literal with several c-chars is `int`, packs 8 bits per c-char
  (last in the low byte, low 32 bits kept; `MultiCharLiteral`,
  `FourCharLiteral` for exactly four, `CharConstantTooLong` beyond four);
  other encodings: `MultiCharNotAllowed`. A c-char needing several code
  units: `CharTooLarge`. `pp_value`/`pp_unsigned` give the `#if` value
  ([cpp.cond]/12) exactly as clang computes it, including x86's
  sign-extended `L` value.
- clang's per-literal `HadError` gating is mirrored exactly (some
  follow-up diagnostics are suppressed after certain errors, e.g. a second
  `\x100`; an empty `\x{}` in a character literal leaves its `}` as a
  c-char), so diagnostic counts match clang on the cross-check corpus.
- ud-suffix ([lex.ext]): split off and returned as spelled; numbers
  accept `_x` (not `__x`) and from C++14 the standard-library suffixes
  (`h min s ms us ns il i if`, C++20 `d y`) as clang's `isValidUDSuffix`.
- pp-numbers: binary (C++14; earlier `BinaryLiteralExtension`), octal,
  decimal, hex; hex floats (C++17; `HexFloatExtension`); digit separators
  (C++14; `DigitSeparatorPosition`); integer suffixes `u`, `l`, `ll`, `z`
  (C++23; `SizeSuffixExtension`) in any legal order and case; float
  suffixes `f l f16 f32 f64 f128 bf16`. Diagnostics follow clang's
  `NumericLiteralParser` (`InvalidDigit`, `InvalidSuffix`,
  `ExponentNoDigits`, `HexFloatNoExponent`, …).
- Unevaluated strings: an encoding prefix, a ud-suffix, or a numeric /
  unknown escape is an error (`UnevaluatedString*`).
- Raw strings: body verbatim; CR LF in the body becomes LF.

### Tables
`scripts/gen-unicode-tables.py [UCD_VERSION]` (default 15.1.0, SHA-256 of
each downloaded UCD file pinned) generates `unicode_tables.rs` (XID,
whitespace, NFC_QC, canonical decomposition/composition, CCC; ~138 KB of
source) and `unicode_names.rs` (33,421 names and aliases, front-coded in
blocks of 32 and sorted by loose-match key, plus 17 NR2 ranges and the
algorithmic Hangul syllables; ~468 KB of source). Name lookup is a binary
search over blocks with a fixed 128-byte key buffer: no allocation.

### Deviations from clang
- `f32 f64 f128 bf16` accepted from C++23 (clang 19 rejects them); `f16`
  in every mode (clang: targets with `_Float16`); GNU `q`/`w`, MS `i64`,
  fixed-point and `_BitInt` (`wb`) suffixes are not supported (invalid
  suffix).
- `i`/`if`/`il` on a number are a ud-suffix (the standard library's
  `operator""i`), not the GNU imaginary suffix.
- `FourCharLiteral` is Extension (clang's `-Wfour-char-constants` is off by
  default), as is `HeaderNameSpecialChars`.
- `StringLiteral::units` excludes the terminating null; ud-suffixes are
  not UCN-expanded or NFC-checked.

### What remains (not WP-011)
- C mode (`'a'` is `int`, no `u8` characters, …): no C mode anywhere yet.
- Did-you-mean notes for unknown/loose `\N{}` names.
- GNU `q`/`i`/`j` suffixes, MS `i64`, `_BitInt` `wb`/`uwb`.
- Types of literals, whether an integer fits its suffix's type, and exact
  floating values in every format: kiln_sema / kiln_oven (they get the
  digits and `parse_f64`).
- UCN expansion and NFC of ud-suffixes (clang does both).
- Open question from WP-010: an invalid UCN inside an identifier
  (`a\N{ABC}`) stays in the identifier; clang splits it into an `unknown`
  token (clang does keep an out-of-range `\U00110000`). Error territory
  either way.
- `#embed` resource lexing is kiln_pp's; HeaderName mode already serves it.
