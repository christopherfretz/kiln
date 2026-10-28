# spec: kiln_source

Owns file contents, file identity, and source locations. Every other crate
refers to source positions only through this crate's types.

Standard: [lex.phases]/1 (physical source files, phase-1 decoding),
[cpp.include] (file identity), [cpp.replace]/[cpp.rescan] (expansion
locations), [cpp.line] and [cpp.predefined] (presumed locations for
`#line`, `__LINE__`, `__FILE__`).

## Sharing model
`SourceManager` is **append-only and every method takes `&self`**. The
lexer (`Lexer<'src>` holds `&'src SourceManager` and borrows `&'src str`
text), the preprocessor (which keeps loading files and creating expansion
entries while lexers are live) and the diagnostics engine all share one
`&SourceManager`. Text and names handed out never move or change. The type
is `Send + Sync`: appends are serialized by an internal mutex; lookups are
lock-free except `presumed_line_col` (a read lock). Storage is a safe
chunked append-only arena (`OnceLock` slots, doubling chunk sizes).

## Types
- `FileId(u32)`: dense index (`FileId::index()`) of a buffer: a loaded file
  or a virtual buffer (predefines, `-D` options, scratch text). Meaningful
  only for the manager that created it; methods taking a foreign `FileId`
  panic.
- `Loc(u32)`, 4 bytes, `Copy`, ordered by raw value. Value space:
  - `0` is `Loc::INVALID`;
  - `[1, 2^31)`: *file locations*. Each buffer owns `[start, start + len]`
    (the extra position is its end-of-file location), allocated in creation
    order, contiguous. `start + n` is byte offset `n`.
  - `[2^31, 2^32)`: *macro locations*. Each expansion entry owns
    `[base, base + len]` (again one extra position, so the half-open end of
    the last spelled byte stays inside the entry).
  - `Loc::is_valid()`, `is_macro()` (O(1), top bit), `is_file()`,
    `with_offset(n)` (same entry; caller keeps it in range).
  - `to_raw()` / `from_raw(u32)`: the raw value, for packing into `u32`
    slots of compact AST nodes; a never-allocated value is an unknown
    location (queries return `None`/`INVALID`).
- `Span { lo: Loc, hi: Loc }`, 8 bytes, `Copy`: half-open `[lo, hi)`.
  `Span::new`, `Span::empty(loc)`, `Span::INVALID`, `is_valid()`.
- `LineCol { file: FileId, line: u32 /*1-based*/, col: u32 /*1-based, bytes*/ }`.
- `PresumedLoc<'a> { file, file_name: &'a str, line, col }`.
- `LoadReport { replacements: Vec<Replacement> }`,
  `Replacement { offset: u32 /*of U+FFFD in decoded text*/, bytes: Box<[u8]> /*original*/ }`.
- `LoadError { Io(io::Error), LocSpaceExhausted }` (non-exhaustive);
  `LocSpaceExhausted` (unit error: clang's "ran out of source locations").
- `ExpansionKind { Macro { name: Span }, MacroArg }`,
  `ExpansionInfo { base: Loc, len: u32, spelling: Loc, expansion: Span, kind }`.

## SourceManager API
Buffers:
- `new()` / `Default`.
- `load_file(&Path) -> Result<FileId, LoadError>`: identity by canonical
  path (`fs::canonicalize`); loading the same file again through any path
  returns the same id without reading the disk. `file_name` is the path as
  given on first load. Invalid UTF-8 is replaced at load (below).
- `add_virtual(name: &str, contents: String) -> Result<FileId, LocSpaceExhausted>`:
  never deduplicated.
- `file_count()`, `text(FileId) -> &str` (always valid UTF-8),
  `file_name(FileId) -> &str`, `load_report(FileId) -> &LoadReport` (empty
  for virtual buffers).
- `loc(FileId, offset) -> Loc` (panics if `offset > len`; `offset == len`
  is the EOF location), `file_span(FileId) -> Span` (`[0, len)`),
  `offset_of(Loc) -> Option<(FileId, u32)>` (file locations only; `None`
  for invalid, unknown and macro locations).

Lines (tables built lazily, once per buffer, `OnceLock`):
- Line terminators are `\n`, `\r\n` and lone `\r` (clang's line table and
  lexer agree). The terminator belongs to the line it ends; the EOF location
  after a trailing terminator is column 1 of the empty line after it.
- `line_col(Loc) -> Option<LineCol>`: macro locations are first mapped
  through `expansion_loc` (the user-visible position; call `spelling_loc`
  first for the spelling position). `None` for invalid/unknown locations.
- `line_count(FileId) -> u32` (an empty buffer has 1 line),
  `line_text(FileId, line) -> Option<&str>` (without terminator; `None` for
  line 0 or past the end).
- `span_text(Span) -> Option<&str>`: both ends mapped through
  `spelling_loc`; `None` unless they land in one buffer, in order, on char
  boundaries.

Macro expansions (modeled on clang's expansion `SLocEntry`s, index-based):
- `add_expansion(spelling: Loc, len: u32 /*bytes*/, expansion: Span, kind) -> Result<Loc, LocSpaceExhausted>`
  returns `base`; `base + n` (`n <= len`) is spelled at `spelling + n`. The
  length is in **bytes of spelled text**, not tokens: one entry covers a
  whole macro replacement list (spelling = start of the replacement list in
  the `#define`), so a body token at definition offset `k` gets `base + k`;
  one `MacroArg` entry covers a contiguous run of argument tokens.
  `spelling` may itself be a macro location (an argument written inside
  another expansion). `Macro { name }`: `expansion` is the invocation range
  (macro name through closing `)`), `name` the macro-name token there; also
  used for builtin macros, `#` and `##` results (spelled in a virtual
  buffer). `MacroArg`: `expansion` is the parameter token in the replacement
  list (a macro location of the enclosing `Macro` entry).
  Panics if `spelling`, `expansion` or `name` endpoints are not already
  allocated locations: this makes every walk strictly decreasing, so walks
  terminate. Debug builds also check `[spelling, spelling + len]` stays in
  one entry.
- `is_macro_loc(Loc)` (O(1)), `expansion_info(Loc) -> Option<ExpansionInfo>`
  (the immediate entry).
- `immediate_spelling_loc(Loc)`: one step; `spelling_loc(Loc)`: to a file
  location. Identity for file locations.
- `expansion_loc(Loc)`: follows `expansion.lo` to the outermost
  invocation's file location. `expansion_span(Loc) -> Span`: the outermost
  invocation's file range (start walks `expansion.lo`, end walks
  `expansion.hi`); `Span::empty(loc)` for file locations.
- `immediate_macro_caller_loc(Loc)` (clang's `getImmediateMacroCallerLoc`):
  for `MacroArg`, the immediate spelling; else `expansion.lo`. Walking it
  while `expansion_info` is `Some`, and emitting a note for each `Macro`
  entry with `span_text(name)`, produces "expanded from macro" notes
  innermost first.
- Unknown macro locations map to `Loc::INVALID` / `Span::INVALID` / `None`;
  they never panic.

Presumed locations ([cpp.line]):
- `add_line_directive(FileId, at_offset, new_line: u32, new_file_name: Option<&str>)`:
  the line starting at `at_offset` (first byte *after* the directive's
  newline; `len` if the directive ends the file) has presumed line
  `new_line`; following lines count up. `None` keeps the presumed name in
  effect before `at_offset` (resolved when the directive is added).
  Directives may be added in any order; re-adding at an existing offset
  replaces (a header without guards re-entered re-adds its directives).
  Range checking of `new_line` ([cpp.line]/3) is kiln_pp's job. Panics if
  `at_offset > len`.
- `presumed_line_col(Loc) -> Option<PresumedLoc>`: macro locations use
  `expansion_loc`; the last directive at or before the offset applies;
  `col` is the physical byte column; the line saturates at `u32::MAX`.
  kiln_pp chooses which location `__LINE__` reports (clang: the expansion
  location of the `__LINE__` token).

## Phase-1 decoding
Files are decoded as UTF-8; each maximal invalid subsequence (as
`std::str::Utf8Chunks` defines it) becomes one U+FFFD, recorded in
`LoadReport` with its offset in the decoded text and the original bytes.
This crate never emits diagnostics; kiln_pp/kiln_diag report the
replacements (once per file, on first entry). A UTF-8 BOM is kept in the
text; skipping it is the lexer's job.

## Identifiers and hashing
- `kiln_source::symbol::{Symbol, Interner}`: the thread-safe identifier
  interner (moved here from kiln_lex, which re-exports it, so that
  kiln_ast stores `Symbol`s without linking the lexer; spec/types.md §17
  T2). `Symbol` is a 4-byte dense index; `Symbol::from_index` is the
  inverse of `index()` for packed encodings.
- `kiln_source::fx::{FxHasher, FxBuildHasher, FxHashMap, FxHashSet}`: the
  rustc `FxHash`, used by every compiler-internal hash table.

## Limits
Buffers share 2^31 positions (each buffer uses `len + 1`), expansion
entries another 2^31 (`len + 1` each). Exhaustion is reported as
`LoadError::LocSpaceExhausted` / `LocSpaceExhausted`, never a panic.

## Invariants
- `Loc` and `Span` are `Copy`, 4/8 bytes (static assertions).
- Line tables are computed lazily and once per buffer.
- No dependency on diagnostics; decode errors are data (`LoadReport`).
- One location range per buffer, allocated at first load. (See open
  question 1.)

## Tests (`crates/kiln_source/tests/unit.rs`, arena unit tests in `src/arena.rs`)
- Round-trip `line_col` on CRLF, LF, mixed (incl. lone CR), empty and
  trailing-newline-less files, against a reference model, at every offset
  including EOF; `line_text` strips terminators; byte columns.
- Invalid UTF-8 replacement positions and original bytes.
- Identity by canonical path (`..`, `.`, symlink), not-found and directory
  errors; virtual buffers never deduplicated.
- Location partitioning and `offset_of` round trip over many buffers.
- Expansion/spelling mapping for nested macro expansions (macro body inside
  macro body, macro argument spelled in another expansion, macro name
  token produced by an expansion, multi-line invocation range),
  "expanded from" walk, unknown macro locations.
- Presumed locations: name change and inheritance, out-of-order and
  replacing directives, macro locations, saturation.
- Text borrows survive concurrent appends; sharing across threads.

## Open questions
1. **Per-inclusion location ranges.** clang allocates a fresh `FileID`
   range per `#include` (with its include location), so any location
   encodes its include stack ("In file included from ...") and `#line`
   state per inclusion. Here a `FileId` is content identity and owns one
   range, so include stacks for post-pp diagnostics must come from
   elsewhere. Adding it later is additive: `add_inclusion(FileId,
   included_from: Loc) -> Result<Loc, _>` allocating another range over
   the same text, plus a lexer constructor taking a start `Loc`.
2. **Scratch buffer.** `##`/`#`/builtin-macro results need spelling text.
   `add_virtual` per result works but costs one entry per pasted token; a
   chunked scratch buffer (clang's `ScratchBuffer`) would be cheaper.
3. GNU line-marker flags (`# 33 "x.h" 1 3`: system header, enter/leave)
   are not stored; needed only for preprocessed input.
