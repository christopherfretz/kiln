# Backlog — work packages

Format per `docs/AGENTS.md`. Ids are stable; status moves between
`todo / in-progress(agent) / review / done`. The supervisor owns this file.

## M0 — bootstrap
| id | scope | status |
|---|---|---|
| WP-001 | `kiln_source`: implement `spec/source.md` with unit tests | done (2026-10-28) |
| WP-002 | `kiln_diag`: implement `spec/diag.md` (engine, ids table skeleton, text + JSON renderers) | todo |
| WP-003 | `xtask test`: conformance runner implementing the directive format in `docs/VALIDATION.md` (see `spec/harness.md`) | done (2026-10-28) |
| WP-006 | harness follow-ups: `run-args:`/`run-stdin:` directives; `xtask ci` runs `xtask test --compiler clang++-19` as a corpus self-check when clang is present; oracle hang → ERROR; `-fconstexpr-report=<file>` protocol for `oven:` (define in `spec/oven.md`) | todo |
| WP-004 | `xtask bench`: run `bench/` entries, record wall/RSS/oven stats, compare with baselines | todo |
| WP-005 | `resource/`: import clang 19 builtin headers + generated predefines for aarch64/x86_64 linux; script to regenerate | done (2026-10-28) |

## M1 — preprocessor parity (unlocked by WP-001..003)
| id | scope | status |
|---|---|---|
| WP-010 | `kiln_lex`: phases 1–2 + identifiers, punctuators, pp-numbers, comments, flags (`spec/lex.md`) | todo |
| WP-011 | `kiln_lex`: char/string/raw-string literals, UCNs, header-name mode, recovery, fuzz | todo (after WP-010 interface lands) |
| WP-012 | `kiln_pp`: directive parsing, conditional stack, `#define`/`#undef`, object-like expansion, hide sets, `Eod` protocol with lexer | todo |
| WP-013 | `kiln_pp`: function-like macros, `#`, `##`, `__VA_OPT__`, rescan, expansion locations | todo |
| WP-014 | `kiln_pp`: `#include` resolution, `#include_next`, `#pragma once`, include guards, `__has_include*`, include depth, callbacks | todo |
| WP-015 | `kiln_pp`: `#if` expression evaluator, `defined`, `__has_*` builtins table, `#line`, `#error/#warning`, pragmas, `_Pragma` | todo |
| WP-016 | `kiln_pp`: `#embed` | todo |
| WP-017 | `kiln_pp::print`: `-E` printer with clang line markers, `-P`, `-C/-CC`, `-dM/-dD` | todo |
| WP-018 | driver: `kiln -E` flag parsing subset + `-M*` dependency output | todo |
| WP-019 | differential corpus: all libc++ 19 headers + sysroot C headers + clang Preprocessor tests; `xtask test` pp-diff mode | todo |
| WP-020 | predefined macro audit: `__cpp_*` values kiln claims for M1; decide `__VERSION__`/`__clang_version__` strings, GNU-mode (`gnu++NN`) macro delta, which macros the driver supplies from flags (`__OPTIMIZE__`, `__PIE__`, `__EXCEPTIONS`, `__GXX_RTTI`, `__GCC_HAVE_DWARF2_CFI_ASM`) vs the static files — see `resource/README.md` | todo |
