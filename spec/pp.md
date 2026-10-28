# spec: kiln_pp — translation phase 4, the preprocessor

Standard: [cpp] entire clause (cpp.pre, cpp.cond, cpp.include, cpp.embed,
cpp.replace incl. __VA_OPT__ [cpp.replace.general]/[cpp.stringize]/
[cpp.concat], cpp.rescan, cpp.scope, cpp.line, cpp.error, cpp.warning,
cpp.pragma, cpp.null, cpp.predefined, cpp.pragma.op, cpp.module, cpp.import),
plus clang extensions libc++ needs: `__has_include`, `__has_include_next`,
`__has_cpp_attribute`, `__has_c_attribute`, `__has_builtin`, `__has_feature`,
`__has_extension`, `__has_attribute`, `__has_warning`, `#include_next`,
`#pragma once`, `#pragma clang ...` (diagnostic push/pop/ignored, system_header,
module import no-ops), `#pragma GCC system_header`, `#pragma push_macro`/
`pop_macro`, `__COUNTER__`, `__INCLUDE_LEVEL__`, `__BASE_FILE__`,
`__FILE_NAME__`, `__TIMESTAMP__`, `__DATE__`/`__TIME__` (overridable for
reproducible tests).

## Interface
```rust
pub struct Preprocessor<'a> { /* owns a stack of Lexers and macro-expansion token buffers */ }
impl Preprocessor {
    pub fn new(sm: &SourceManager, diags: &DiagEngine, opts: PpOptions) -> Self;
    pub fn enter_main_file(&mut self, file: FileId);
    pub fn next(&mut self) -> Token;        // fully expanded token stream; Token::span is the *expansion* loc; spelling loc reachable via SourceManager
    pub fn macros(&self) -> &MacroTable;    // for -dM / -dD
    pub fn callbacks(&mut self) -> &mut PpCallbacks; // include enter/exit, macro define/undef/expand, pragma — used by -MMD, -E line markers, the parser (for `import` handling) and tests
}
pub struct PpOptions {
    pub std: Std, pub include_dirs: IncludeDirs /* -I, -isystem, -iquote, -idirafter, -isysroot, -resource-dir, -nostdinc, -nostdinc++ */,
    pub defines: Vec<(String, Option<String>)>, pub undefines: Vec<String>, pub predefines: Predefines /* target + clang-compat macro set */,
    pub max_include_depth: u32 /* 200 */, pub reproducible_time: Option<(String,String)> /* __DATE__, __TIME__ */,
}
```
A separate `kiln_pp::print` module implements `-E` (with clang's line-marker
rules: `# <line> "<file>" <flags>`), `-P`, `-C`/`-CC` (comments preserved —
requires the lexer's comment tokens via `LexOptions::keep_comments`),
`-dM`, `-dD`, `-dI`. Output must be byte-identical to clang 19's for the
differential corpus.

## Behavior (highlights; the standard is normative)
- Macro expansion follows [cpp.rescan] exactly, including the "already
  expanded" (hide-set / disabled macro) rule, the `##` placemarker rules,
  `#` stringification spacing rules (spacing derived from `LEADING_SPACE`
  flags, not from source text), `__VA_OPT__` incl. the placemarker and the
  "stringify `__VA_OPT__`" corner cases from P1042, and the function-like
  macro name followed by `(` across lines/files rule.
- Expansion locations: each expanded token carries a `Loc` in a macro
  expansion range; kiln_source maps it to (spelling, expansion) so that
  diagnostics can print "expanded from macro 'X'" notes identical to clang's.
- `#include` resolution order and `#include_next` semantics match clang
  (quote dirs, then `-I`, then `-isystem`, then sysroot C++ then C includes;
  `-idirafter` last). The resource dir contains kiln's own builtin headers
  (`stddef.h`, `stdarg.h`, `stdint.h`, ... — a copy of clang 19's, under
  `resource/include/`, since libc++ and glibc headers depend on their exact
  contents).
- `#pragma once` and include guards: detect the classic guard pattern
  (`#ifndef X / #define X ... #endif` spanning the whole file) and skip
  re-opening the file, as clang's `MultipleIncludeOpt` does.
- Conditional expressions ([cpp.cond]): evaluated in `intmax_t`/`uintmax_t`
  with the usual arithmetic conversions, `defined`, `__has_*`, character
  literals with the target's encoding rules, `true`/`false`, division by zero
  diagnosed; unevaluated branches of `?:`/`&&`/`||` do not diagnose.
- `#embed` ([cpp.embed]): `limit`, `prefix`, `suffix`, `if_empty`; bytes
  become pp-numbers; large embeds must not expand to a token per byte in
  memory eagerly — stream them.
- `#line`, `__LINE__`/`__FILE__` presumed locations tracked by kiln_source.
- Modules: `module;`, `export module x;`, `import x;`, `import <h>;`,
  `import "h";` are recognized in the preprocessor per [cpp.module]/[cpp.import]
  and emitted as directive-marker tokens for the parser; header-unit import
  is M4 work and may initially diagnose "unsupported".
- Predefined macros ([cpp.predefined] + target + clang compatibility): the
  exact set is generated from `clang -E -dM -x c++ -std=<std> /dev/null` for
  the target and checked in as `resource/predefines/<triple>.<std>.txt`
  (std in c++17/20/23/26, strict dialect; see `resource/README.md` for the
  GNU-dialect and option-dependent deltas the driver must apply); kiln
  adds `__KILN__`, `__KILN_MAJOR__`, etc., and removes nothing. The
  `__cpp_*` feature macros reflect what kiln actually implements and are
  reviewed at each milestone.

## Error recovery
Every diagnostic in [cpp] has a `DiagId` (`pp_*`) and matches clang's
message wording where a test depends on it. After an error in a directive,
skip to end of line and continue.

## Performance obligations
- Token buffers for expansion results are reused (no per-expansion `Vec`).
- `-E` on all libc++ 19 headers: ≤ 1.5x clang's time, ≤ clang's peak RSS.
  Baseline in `bench/baselines/pp.json`.

## Tests
- `tests/conformance/cpp/<section>/` per [cpp] subsection, directive format
  per docs/VALIDATION.md with `expect: pp`.
- `tests/differential/pp/`: every libc++ 19 header, the C headers it pulls
  in, and the torture cases from clang's `test/Preprocessor` that are not
  clang-internal; compared to `clang -E` token-wise and `-E -P` textually.

## Decisions recorded from kiln_source (WP-001) — binding for pp
- **One location range per inclusion.** `kiln_source` currently gives one
  `Loc` range per `FileId`; "In file included from ..." notes need a range per
  inclusion. WP-014 adds `SourceManager::add_inclusion(FileId, included_from: Loc)
  -> Loc` (a new range aliasing the same text) and a `Lexer` constructor that
  starts from a base `Loc`; the main file uses the file's own range. Spec
  `source.md` must be updated with that API when it lands.
- **Scratch buffer.** `#`/`##` results and builtin-macro expansions
  (`__LINE__`, `__COUNTER__`, ...) go into a chunked scratch buffer
  (one `add_virtual` per chunk, e.g. 4 KiB), not one virtual buffer per token.
- **Line endings / BOM.** `\n`, `\r\n` and lone `\r` end a line (matches
  kiln_source line tables); a UTF-8 BOM is kept in the text and skipped by
  the lexer at offset 0 of a file.
- **`#line`.** pp calls `add_line_directive` with `at_offset` = first byte
  after the directive's newline and range-checks `new_line` ([cpp.line]/3)
  itself. `__LINE__` reports the presumed line of the expansion location of
  the `__LINE__` token (clang behavior).
- **Invalid UTF-8.** pp reports `load_report(FileId)` replacements once per
  file via kiln_diag (`lex_invalid_utf8`) when it first enters the file.
