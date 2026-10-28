# spec: kiln_pp — translation phase 4, the preprocessor

Standard: [cpp], the whole clause: cpp.pre, cpp.cond, cpp.include,
cpp.embed, cpp.module, cpp.import, cpp.replace (including `__VA_OPT__`,
cpp.stringize, cpp.concat, cpp.rescan and cpp.scope), cpp.line, cpp.error,
cpp.pragma, cpp.null, cpp.predefined and cpp.pragma.op.

It also covers the clang extensions that libc++ needs:

- the feature tests `__has_include`, `__has_include_next`,
  `__has_cpp_attribute`, `__has_builtin`, `__has_constexpr_builtin`,
  `__has_feature`, `__has_extension`, `__has_attribute`,
  `__has_declspec_attribute`, `__has_warning`, `__has_embed`,
  `__is_identifier` and `__is_target_*` (clang 19 does not define
  `__has_c_attribute` in C++);
- the directives `#include_next`, `#import`, `#warning`, `#ident` and GNU
  line markers;
- the pragmas `#pragma once`, `#pragma clang|GCC ...`, `push_macro` and
  `pop_macro`;
- the builtin macros `__COUNTER__`, `__INCLUDE_LEVEL__`, `__BASE_FILE__`,
  `__FILE_NAME__` and `__TIMESTAMP__`, plus `__DATE__` and `__TIME__`
  (both overridable so that tests are reproducible).

Reference behaviour is clang 19 (`clang++-19 -std=c++26`). Output must
match it byte for byte unless a deviation is listed below.

## Architecture: the core and its hooks

The crate root is `crates/kiln_pp/src`. Its modules are:

| Module | Contents |
|---|---|
| `preprocessor.rs` | `Preprocessor`: the frame stack (file lexers and macro token buffers), `next` / `next_item`, `PpItem` / `Marker`, file enter/exit, the scratch buffers and the include-guard detector (`MiOpt`). |
| `directive.rs` | Directive dispatch ([cpp.pre]); `#define` / `#undef`; the structure and skipping of `#if` groups; `#include` / `#embed` requests; `#line` and GNU line markers; `#error` / `#warning`, `#ident` and the null directive; the module / import directive markers. |
| `expand.rs` | Macro invocation (a port of clang's `TokenLexer`, `MacroArgs` and `PPMacroExpansion`): argument collection, substitution, `#`, `##`, `__VA_OPT__`, rescan and the hide set, and the builtin macro values. |
| `pragma.rs` | `#pragma` and `_Pragma` (destringize, then re-lex), and the pragmas the core handles itself. |
| `cond.rs` | The `IfEvaluator` hook, `IfContext`, and `StubIfEvaluator`. |
| `include.rs` | The `IncludeResolver` hook, the request types, and `MemoryResolver` (for tests). |
| `builtin.rs` | The `Builtin` macros, the `FeatureQueries` hook, and the C++26 keyword list. |
| `callbacks.rs` | The `PpCallbacks` observer and pragma hook, and `FileChange`. |
| `macros.rs` | `MacroDef` and `MacroTable`. |
| `options.rs` | `PpOptions`. |
| `print.rs` | The `-E` printer (`print_preprocessed`) and `-dM` (`print_macros`). |
| `tok.rs` | `PpTok`: a token plus its preprocessor-only flags. |
| `diag_map.rs` | Maps lexer diagnostics to `kiln_diag` ids. |

The core is complete for everything that does not depend on the outside
world. Each of the following is a trait object installed on the
`Preprocessor`, with a default that is enough for the tests. Later work
packages replace the defaults without changing the core.

### `IncludeResolver`: to be implemented by WP-014 (and WP-016 for `#embed`)

```rust
pub trait IncludeResolver {
    fn resolve(&mut self, sm: &SourceManager, req: &IncludeRequest) -> Option<ResolvedInclude>;
    fn resolve_embed(&mut self, sm: &SourceManager, req: &EmbedRequest) -> Option<EmbedData> { None }
}
pub struct IncludeRequest { kind: IncludeKind, name: String, angled: bool,
    includer: Option<FileId>, lookup_from: Option<u32>, name_span: Span, directive_span: Span }
pub enum IncludeKind { Include, IncludeNext, Import, HasInclude, HasIncludeNext }
pub struct ResolvedInclude { file: FileId, lookup_index: Option<u32>, system: bool }
pub struct EmbedRequest { name: String, angled: bool, includer: Option<FileId>,
    limit: Option<u64>, offset: Option<u64>, name_span: Span, query_only: bool }
pub struct EmbedData { bytes: Vec<u8> }
```

The core does the following itself:

- parses the header name, whether it is a `header-name` token, a
  macro-expanded string, or `<` pieces `>` ([cpp.include]/7);
- passes the `lookup_index` of the includer as `lookup_from` for
  `#include_next`;
- enforces `max_include_depth`, `#pragma once`, `#import` and the
  include-guard optimization (below);
- enters the file and reports `FileChange`.

WP-014 supplies `-I`, `-isystem`, `-iquote`, `-idirafter`, the sysroot,
the resource directory (`resource/include/`, a copy of clang 19's builtin
headers), the file cache, and the `system` flag. The clang search order is
quote directories, then `-I`, then `-isystem`, then the sysroot C++ and C
includes, then `-idirafter`.

For `#embed` the core parses `limit`, `prefix`, `suffix`, `if_empty` and
their `clang::` / `__x__` spellings. It evaluates `limit` and
`clang::offset` with `IfEvaluator::eval_value`. The resolver returns the
bytes, already cut to size.

The default is `NoIncludes`, which finds nothing.

### `IfEvaluator`: to be implemented by WP-015

```rust
pub trait IfEvaluator {
    fn eval(&mut self, cx: &mut dyn IfContext) -> bool;
    fn eval_value(&mut self, cx: &mut dyn IfContext) -> Option<i64> { /* stub grammar */ }
}
pub trait IfContext {
    fn next(&mut self) -> Token;            // macro-expanded; ends with Eod
    fn next_unexpanded(&mut self) -> Token; // the operand of `defined`
    fn is_defined(&mut self, name: Symbol) -> bool;
    fn spelling(&self, tok: &Token) -> String;
    fn interner(&self) -> &Interner;
    fn source_manager(&self) -> &SourceManager;
    fn emit(&mut self, diag: Diag);
    fn is_enabled(&self, id: DiagId) -> bool;
    fn std(&self) -> Std;
}
```

The core keeps the structure of conditional groups: nesting, skipping
(clang's `SkipExcludedConditionalBlock`), `#elifdef` and `#elifndef`, and
the unterminated-conditional and stray-`#else` diagnostics. Only the
value of an expression comes from the evaluator. The `__has_*` operators
expand as builtin macros before the evaluator sees them, as in clang.

`StubIfEvaluator` handles integer literals, `true` and `false`,
identifiers (which are 0), `defined`, parentheses, the unary and binary
operators, and `?:`, in wrapping `intmax_t` arithmetic. It warns when a
macro expansion produces `defined`. Anything else reports
`pp_expr_unsupported`.

WP-015 adds the rest:

- the usual arithmetic conversions to `uintmax_t`;
- character literals;
- division by zero;
- the clang wording of every error;
- no diagnostics in unevaluated operands.

### `FeatureQueries`: to be implemented by WP-015 (feature tables)

```rust
pub trait FeatureQueries {
    fn has_feature(&self, name: &str) -> bool { false }
    fn has_extension(&self, name: &str) -> bool { self.has_feature(name) }
    fn has_builtin(&self, name: &str) -> u32 { 0 }
    fn has_constexpr_builtin(&self, name: &str) -> bool { false }
    fn has_attribute(&self, name: &str) -> u32 { 0 }
    fn has_cpp_attribute(&self, scope: Option<&str>, name: &str) -> u32 { /* [cpp.cond] Table 23 */ }
    fn has_declspec_attribute(&self, name: &str) -> bool { false }
    fn is_keyword(&self, name: &str) -> bool { /* C++26 keywords */ }
    fn is_target(&self, query: TargetQuery, name: &str) -> bool { false }
    fn flt_eval_method(&self) -> i32 { 0 }
}
```

The core parses the operand of each query, including scoped attribute
names and the `__x__` normalisation. It diagnoses malformed operands with
clang's wording and spells the result. Values other than 0 and 1 get an
`L` suffix, as in clang.

`__has_warning` is answered from `kiln_diag`'s group table.

WP-015 generates the tables from clang 19 for the configured target. The
default is `NoFeatures`, which uses the trait defaults.

### `PpCallbacks`: consumers are WP-014 (`-M`), WP-017 (`-dD`, `-dI`) and phase 7

```rust
pub trait PpCallbacks {
    fn file_changed(&mut self, change: FileChange) {}
    fn inclusion_directive(&mut self, req: &IncludeRequest, result: Option<&ResolvedInclude>) {}
    fn macro_defined(&mut self, name: Symbol, def: &MacroDef) {}
    fn macro_undefined(&mut self, name: Symbol, span: Span, def: Option<&MacroDef>) {}
    fn macro_expands(&mut self, name: &Token, def: &MacroDef, range: Span) {}
    fn pragma(&mut self, info: &PragmaInfo<'_>) -> bool { false }  // true = consumed
    fn pragma_message(&mut self, span: Span, namespace: Option<&str>, kind: PragmaMessageKind, text: &str) {}
    fn ident(&mut self, span: Span, text: &str) {}
    fn source_range_skipped(&mut self, range: Span) {}
}
pub enum FileChange { Enter { file, from: Option<Loc>, system },
    Exit { file, exited, resume: Loc, system },
    Rename { file, at: Loc, system, flag: Option<u8> },  // #line / line marker
    SystemHeader { file, at: Loc } }
```

As in clang, `macro_expands` is also called for builtin macros such as
`_Pragma`.

The core consumes these pragmas:

- `once`, `push_macro`, `pop_macro`, `mark`, `region` and `endregion`;
- `GCC` / `clang` `poison`, `system_header` and `dependency`.

`message`, `GCC warning` and `GCC error` are diagnosed by the core and
become `Marker::PragmaMessage`. `GCC` / `clang` `diagnostic` is validated
by the core, then offered to `pragma`, as is every other pragma. A pragma
that `pragma` does not consume becomes `Marker::Pragma`.

The default is `NoCallbacks`.

## Interface

```rust
impl<'a, 'd> Preprocessor<'a, 'd> {
    pub fn new(sm: &'a SourceManager, diags: &'d DiagEngine<'a>, interner: &'a Interner, opts: PpOptions) -> Self;
    pub fn set_include_resolver(&mut self, r: Box<dyn IncludeResolver + 'd>);
    pub fn set_if_evaluator(&mut self, e: Box<dyn IfEvaluator + 'd>);
    pub fn set_feature_queries(&mut self, f: Box<dyn FeatureQueries + 'd>);
    pub fn set_callbacks(&mut self, c: Box<dyn PpCallbacks + 'd>);
    pub fn enter_main_file(&mut self, file: FileId);  // then <built-in>: predefines, -D, -U
    pub fn next(&mut self) -> Token;                  // phase-4 tokens; Eof repeats
    pub fn next_item(&mut self) -> PpItem;            // tokens and Markers
    pub fn is_embed_byte(&self, t: &Token) -> bool;
    pub fn macros(&self) -> &MacroTable;
    pub fn callbacks(&mut self) -> &mut dyn PpCallbacks;
    pub fn source_manager(&self) -> &'a SourceManager;
    pub fn interner(&self) -> &'a Interner;
    pub fn options(&self) -> &PpOptions;
    pub fn lex_options(&self) -> &LexOptions;
}
pub enum PpItem { Token(Token), Marker(Marker) }
pub enum Marker { Pragma{..}, PragmaMessage{..}, Ident{..}, FileChange(FileChange),
                  Module{span, export, import}, Embed{span} }
pub struct PpOptions { std, trigraphs, dollar_in_identifiers, keep_comments,
    keep_macro_comments, predefines: String, defines, undefines,
    max_include_depth /* 200 */, reproducible_time, timestamp }
```

`Token::span` is the expansion location. The spelling and the macro
backtrace are reachable through `SourceManager::expansion_info`.

### Module and import directives

A `module`, `import` or `export module|import` that starts a line and is
followed by a name, `:`, `;` (for `module`), or a header name (for
`import`) is a directive ([cpp.module], [cpp.import]).

The core queues a `Marker::Module` and returns the directive's tokens,
macro-expanded, as ordinary tokens for the parser. Header-unit imports are
diagnosed as unsupported; they are M4 work.

### `kiln_pp::print`

`print_preprocessed(pp, PrintOptions { line_markers, line_directives }, out)`
is a port of clang's `PrintPPOutputPPCallbacks`. It covers:

- **`-E`** with line markers `# <line> "<file>" <flags>`, where flag 1 is
  enter, 2 is exit and 3 is a system header (`line_markers: true`).
- **`#line` spelling** of the markers (`-fuse-line-directives`).
- **`-P`** (`line_markers: false`), with clang's blank-line and
  column-indentation rules.
- **`AvoidConcat` spacing**, including `[:`, `:]` and `^^` (see the
  deviations).
- **`#embed` output** as `b0, b1, ...`.
- **Pass-through pragmas.**
- **`-C`** through `PpOptions::keep_comments`, using the lexer's comment
  tokens.

`print_macros` implements `-dM`. `-dD` and `-dI` are not implemented: they
belong to WP-017, which builds them on `macro_defined` and
`inclusion_directive` (see the open questions).

## Behavior (the standard is normative; clang 19 decides what it leaves open)

- **Directives.** Only a `#` that is physically at the start of a line
  starts a directive (clang `IsAtPhysicalStartOfLine`). `EMPTY # define`
  is text. Directives inside macro arguments are processed, with the
  clang extension diagnostic.
- **Macro expansion.** Expansion follows [cpp.rescan] as clang implements
  it. This covers:
  - the hide set (disabled macros, and `NO_EXPAND` tokens that stay
    unexpandable);
  - placemarkers for `##`;
  - the spacing of `#` results, taken from `LEADING_SPACE` flags rather
    than the source text;
  - `__VA_OPT__`, including P1042's stringify and placemarker cases;
  - a function-like macro name followed by `(` on a later line, or after
    the end of a file.
- **Locations.** Each expanded token has a `Loc` in a macro expansion
  range (`ExpansionKind::Macro` or `MacroArg`), so diagnostics can print
  "expanded from macro" notes like clang's.
- **`__LINE__`.** The presumed line at the end of the outermost expansion
  range of the `__LINE__` token. A `__LINE__` spelled after a line splice
  is on the line after the splice. Both follow clang
  (`AdvanceToTokenCharacter`).
- **`#pragma once` and include guards.** The core ports clang's
  `MultipleIncludeOpt`:
  - Every token lexed in a file counts as read, including directive
    tokens, but not the directive's `#`, comments, `Eod` or `Eof`.
  - A top-level `#ifndef X` with `X` undefined, or a top-level
    `#if !defined(X)` / `#if !(defined X)` that is true and has no tokens
    from a macro expansion, with nothing read before it, is a candidate
    guard.
  - Any other top-level conditional, `#else` or `#elif` invalidates the
    candidate, as does anything read after the matching `#endif`.
  - A file with a guard is not re-entered while the guard macro is
    defined. It produces no `FileChange` events, which changes the output
    of `-P`.
- **`#embed`.** The core produces `prefix`, then the byte tokens
  (`is_embed_byte`: a pp-number per byte and the commas between them,
  spelled in one scratch buffer per embed), then `suffix`. An empty resource produces `if_empty`
  instead. Each embed is preceded by `Marker::Embed`.
- **`#line` and line markers.** `#line` operands are macro-expanded; line
  markers are not. Marker flag 2 with no matching flag-1 marker in the
  file is an error. Presumed locations are kept by kiln_source
  (`add_line_directive`).
- **Predefined macros.** The macros of [cpp.predefined], plus those of the
  target and clang compatibility, come from `PpOptions::predefines`. That
  text is normally `resource/predefines/<triple>.<std>.txt`, generated
  with `clang -E -dM`; see `resource/README.md`.

  The `<built-in>` buffer holds the predefines, then each `-D` and `-U`.
  The builtin macros are entries in the macro table with
  `MacroDef::is_builtin` set, so `#ifdef __has_include` is true.
  `__DATE__`, `__TIME__` and `__TIMESTAMP__` can be fixed through
  `reproducible_time` and `timestamp`.
- **Diagnostics.** Every [cpp] diagnostic has a `pp_*` id in
  `kiln_diag/src/ids.rs`, with clang 19's wording, level and group. After
  an error in a directive, the rest of the line is skipped.

## Deviations from clang 19 (each covered by a test or a test note)

1. **The `<built-in>` buffer in `-E` mode.** kiln prints only
   `# 1 "<built-in>" 1` and `# 1 "<file>" 2`. clang also prints
   `# 1 "<built-in>" 3`, `# 493 "<built-in>" 3` and the
   `<command line>` buffer, because it splits the buffer into the
   predefines and the command-line macros. `-P` output is unaffected.
2. **`#pragma clang|GCC diagnostic` is only validated.** kiln checks the
   syntax and the group names. Changing diagnostic state is phase-7 work
   for the consumer of `PpCallbacks::pragma`.
3. **`#pragma clang deprecated`, `final` and `restrict_expansion` are
   passed through.** kiln does not implement their effects on macros.
4. **`#pragma GCC dependency` is consumed without checking the file's
   date.**
5. **`push_macro` does not suppress redefinition warnings.** It also does
   not record the "macro was pushed" state.
6. **Poisoned identifiers are not checked inside `_Pragma` strings.**
7. **`AvoidConcat` also separates `[:`, `:]` and `^^`.** This is for C++26
   reflection splicers. It always uses the C++11 and later rules, such as
   the ud-suffix and raw-string prefix cases.
8. **The spacing of `#embed` output is empirical.** clang prints it from
   an annotation token. kiln reproduces the observed spacing, including
   the separator before the token that follows the embed. The
   `__has_embed` operand parameters are evaluated with a nested
   `IfEvaluator::eval_value`.
9. **Line marker flag 2 does not check the filename.** clang also checks
   that the name matches the includer.
10. **Module directives check only that the first name is not a macro.**
    kiln checks the first name component, not every component. The check
    follows [cpp.module]/2; clang 19 does not diagnose this case at all.
11. **Header-unit `import` reports "importing a header unit is not
    supported yet".**
12. **`PpCallbacks::macro_expands` is not delayed.** clang delays the
    callback for function-like macros until the arguments have been
    collected; kiln calls it when the expansion begins.
13. **`contract_assert` is a keyword in `__is_identifier`.** This follows
    C++26 (P2900). clang 19 treats it as an identifier.
14. **The include-guard detector's `DidMacroExpansion` is approximated.**
    kiln uses "no token from a macro location" instead.
15. **`-CC` does not retain comments in `#define` bodies.** Comments in
    macro arguments are retained.
16. **`__FILE__` names a header by its include spelling.** For example,
    `"b.h"` comes from `MemoryResolver`, not `"inc/b.h"`. The spelling
    comes from the resolver, so WP-014 decides it.

## Error recovery

After an error in a directive, the rest of the line is skipped and
preprocessing continues.

`pp_file_not_found` is a fatal error, as in clang. The driver stops after
it through kiln_diag's fatal handling; the core itself carries on.

## Performance obligations

- **Token buffers are recycled.** Expansion token buffers come from
  `Preprocessor::pool`, so there is no fresh `Vec` per expansion in the
  steady state.

  An allocation-count test would need a counting `#[global_allocator]`,
  which needs `unsafe impl GlobalAlloc`. That is impossible under
  `#![forbid(unsafe_code)]` in this crate and its tests. The obligation is
  therefore checked by the bench harness (peak RSS, wall time), not by a
  unit test.
- **Speed and memory against clang.** `-E` on all libc++ 19 headers must
  take at most 1.5x clang's time and use no more than clang's peak RSS.
  The baseline is in `bench/baselines/pp.json` (M1 gate).

## Tests

- **Per-rule tests** live in `crates/kiln_pp/tests/`, one file per rule
  group. Each test carries a `///` tag naming its [cpp] section, and a
  note when it records a deviation. The files are `cpp_replace`,
  `cpp_stringize`, `cpp_rescan`, `cpp_cond`, `cpp_line`, `cpp_pragma`,
  `cpp_predefined`, `cpp_include`, `cpp_embed`, `cpp_module` and
  `cpp_options`.

  Their goldens are clang 19's `-E -P` output and diagnostics, run with
  the aarch64 Linux C++26 predefines in
  `resource/predefines/aarch64-unknown-linux-gnu.c++26.txt`.

  They are generated by `crates/kiln_pp/tests/goldens/gen.py`, which runs
  clang and needs `/usr/bin/clang++-19`. Edit the `cpp_*.py` cases, then
  rerun `python3 gen.py <module>`.
- **Hand-written tests.**
  - `print.rs` covers line-marker mode, checked by hand against clang
    without `-P`, and `-dM`.
  - `hooks.rs` shows that custom `IncludeResolver`, `IfEvaluator`,
    `FeatureQueries` and `PpCallbacks` implementations plug in, and checks
    the `Marker` stream.
- **Later.**
  - `tests/conformance/cpp/<section>/`, in the directive format of
    docs/VALIDATION.md (`expect: pp`).
  - `tests/differential/pp/`, which covers every libc++ 19 header and
    clang's `test/Preprocessor` torture cases. These are compared to
    `clang -E` token by token and to `-E -P` as text, once WP-014's
    header search exists.

## Open questions

- **`-dD` and `-dI`.** Should these be WP-017 printer modes driven by
  `macro_defined` and `inclusion_directive`, or should they be markers in
  the item stream? clang interleaves them with the output text, which
  needs the printer to see them in stream order. Today callbacks run
  synchronously, before queued markers are printed.
- **The `<built-in>` / `<command line>` split** (deviation 1). Should
  `<built-in>` become two buffers so that `-E` output is byte-identical?
- **One location range per inclusion** (below). This is needed before
  "In file included from" notes can distinguish repeated inclusions.
- **Comments in `#define` bodies under `-CC`** (deviation 15).
- **Streaming `#embed`.** The byte tokens are built eagerly, as a `Vec`
  of about two tokens per byte. A large resource should instead be a
  lazily expanded frame. The printer and the parser only need the byte
  values (clang's `annot_embed`), so a dedicated `PpItem` carrying the
  bytes may be the better interface. This must be decided before M3
  embeds of a megabyte or more.

## Decisions recorded from kiln_source (WP-001): binding for pp

- **One location range per inclusion.** `kiln_source` currently gives one
  `Loc` range per `FileId`. "In file included from ..." notes need a range
  per inclusion.

  WP-014 adds `SourceManager::add_inclusion(FileId, included_from: Loc) ->
  Loc`, a new range that aliases the same text. It also adds a `Lexer`
  constructor that starts from a base `Loc`. The main file uses the file's
  own range. Spec `source.md` must be updated with that API when it lands.

  Until then, kiln_pp enters every inclusion of a file at
  `sm.loc(file, 0)`.
- **Scratch buffers.** The results of `#` and `##`, the values of builtin
  macros such as `__LINE__` and `__COUNTER__`, and `_Pragma` strings are
  spelled in one virtual buffer per distinct string. These buffers are
  interned in `Preprocessor::scratch`, so a string spelled twice reuses
  its buffer.

  Each `#embed` gets its own buffer.
- **Line endings and BOM.** `\n`, `\r\n` and a lone `\r` each end a line,
  matching kiln_source's line tables. A UTF-8 BOM is kept in the text; the
  lexer skips it at offset 0 of a file.
- **`#line`.** pp calls `add_line_directive` with `at_offset` set to the
  first byte after the directive's newline. pp range-checks `new_line`
  itself ([cpp.line]/3).

  `__LINE__` reports the presumed line at the end of the outermost
  expansion range of the `__LINE__` token. This is the expansion location
  for a direct use. For a use in a macro argument, it is the line of the
  invocation's closing `)` (clang behaviour).
- **Invalid UTF-8.** pp reports the `load_report(FileId)` replacements
  once per file through kiln_diag (`lex_invalid_utf8`), when it first
  enters the file.
