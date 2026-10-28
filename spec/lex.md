# spec: kiln_lex — translation phases 1–3

Standard: [lex.phases] phases 1–3, [lex.pptoken], [lex.charset], [lex.comment],
[lex.header], [lex.ppnumber], [lex.name], [lex.operators], [lex.literal]
(spelling only; literal *values* are computed by kiln_sema/kiln_oven),
[lex.digraph], [lex.key] (classification happens in the parser, not here).

## Scope
Input: a `FileId` (UTF-8 text from kiln_source). Output: a stream of
**pp-tokens** with source spans and flags. The lexer never expands macros,
never processes directives, and never evaluates literals; those belong to
kiln_pp and later phases. It must be usable as a stand-alone, re-startable
component: kiln_pp drives it by pulling one token at a time and may ask it to
switch modes (see below).

## Interface
```rust
pub struct Lexer<'src> { /* private */ }
impl<'src> Lexer<'src> {
    pub fn new(sm: &'src SourceManager, file: FileId, opts: LexOptions) -> Self;
    pub fn next(&mut self) -> Token;             // returns Token { kind: Eof } at end, repeatedly
    pub fn set_mode(&mut self, m: LexMode);      // Normal | HeaderName (after #include / __has_include) | Module (module/import directive)
    pub fn diagnostics(&self) -> &[LexDiag];     // kiln_diag ids: lex_*
}
pub struct Token { pub kind: TokKind, pub span: Span, pub flags: TokFlags }
pub struct TokFlags: u8 { LEADING_SPACE, START_OF_LINE, /* internal: */ NEEDS_CLEANING /* had splices or UCNs in spelling */ }
pub enum TokKind {
    Identifier(Symbol),         // interned; UCNs and \u{...} already resolved to the spelled identifier
    PpNumber, CharLit, StringLit, RawStringLit, HeaderName,
    Punct(Punct),               // every token in [lex.operators], digraphs mapped to their primary spelling with flag DIGRAPH
    Other(char),                // any non-whitespace character that is not one of the above ([lex.pptoken]/2)
    Eod,                        // end of directive line; emitted only while `in_directive` is set by kiln_pp
    Eof,
}
```
`LexOptions`: `std` (C++17/20/23/26 affects: `<=>`, `u8` char literals,
`\N{}` and `\o{}`/`\x{}` escapes, `#embed` lexing in HeaderName mode,
digit separators, `__VA_OPT__` is pp's business), `dollar_in_identifiers`,
`trigraphs: false` (not in C++17+; diagnose their appearance as clang does
with `-Wtrigraphs` only when `-ftrigraphs`).

Spelling: `Lexer::spelling(&Token) -> Cow<str>` returns the token text with
line splices removed and UCNs *kept* (as clang's `getSpelling`); identifiers
compare by `Symbol` which is the UCN-normalized spelling ([lex.name]/1).

## Behavior
- Phase 1: input is UTF-8 (kiln_source guarantees validity). Characters
  outside the basic character set are legal in identifiers per [lex.name]
  (XID_Start/XID_Continue, with NFC check and the "no combining mark first"
  rule) and in literals/comments; elsewhere diagnose `lex_invalid_char`.
- Phase 2: backslash-newline splicing (`\` followed by `\n`, `\r\n`; also
  with trailing whitespace between them: diagnose `lex_backslash_newline_space`
  as a warning and splice, matching clang). A file ending without a newline
  behaves as if one were added. A splice may occur anywhere including inside
  tokens and `//` comments.
- Phase 3: maximal munch with the standard's exceptions: `<::` ([lex.pptoken]/3.2),
  header-name mode, raw string literals (delimiter up to 16 chars, no
  splice/UCN processing inside, and *reverting* phase 1/2 transformations is
  modeled by lexing the raw body directly from the source bytes), `//` and
  `/* */` comments become one whitespace, `' '` digit separators inside
  pp-numbers ([lex.ppnumber]: `'` followed by a digit or nondigit), user-defined
  literal suffixes are part of the literal token, encoding prefixes
  `u8 u U L` on char/string literals, `R`/`u8R`/... raw prefixes.
- `TokFlags::START_OF_LINE` is set on the first token after a newline that
  is not inside a comment; `LEADING_SPACE` when any whitespace or comment
  precedes the token on the same line. These two bits are what `-E` printing
  and the `#` / `##` operators depend on; they must be exact.
- Unterminated `/*`, `"`, `'`, raw string: diagnose and recover by ending the
  token at end of line (strings/chars) or end of file (comments/raw strings),
  as clang does.
- Mode `HeaderName`: `<...>` and `"..."` lex as `HeaderName`; set by kiln_pp
  only immediately after `include`, `embed`, `__has_include`, `__has_embed`,
  `import`.
- Mode `Module`: `module`/`import`/`export` directive recognition per
  [cpp.pre]/1 and [cpp.module]/[cpp.import]: the *preprocessor* decides;
  the lexer only reports `START_OF_LINE` reliably so it can.
- `Eod`: when kiln_pp sets `in_directive`, a newline yields `Eod` instead of
  being whitespace. kiln_pp clears it after consuming `Eod`.

## Performance obligations
- Zero allocation per token except identifier interning (first sight only).
- Lexing all libc++ 19 headers (`-E` corpus) in under 1.5x clang's lexer time;
  measured by `bench/pp/`.

## Tests (tests/conformance/lex/)
- One test per rule above, plus differential: `kiln -E -P` vs `clang -E -P`
  on `tests/differential/pp/` is also a lexer test once pp exists; before
  that, `xtask lex-dump` prints `kind spelling flags` per token and the
  goldens in `tests/conformance/lex/*.expected` are compared.
- Fuzz: `cargo test -p kiln_lex -- fuzz` runs a bounded random-byte fuzz
  checking no panics and that splicing is idempotent.
