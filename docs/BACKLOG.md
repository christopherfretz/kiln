# Backlog — work packages

Format per `docs/AGENTS.md`. Ids are stable; status moves between
`todo / in-progress(agent) / review / done`. The supervisor owns this file.

## M0 — bootstrap
| id | scope | status |
|---|---|---|
| WP-001 | `kiln_source`: implement `spec/source.md` with unit tests | done (2026-10-28) |
| WP-002 | `kiln_diag`: implement `spec/diag.md` (engine, ids table skeleton, text + JSON renderers) | done (2026-10-28) |
| WP-003 | `xtask test`: conformance runner implementing the directive format in `docs/VALIDATION.md` (see `spec/harness.md`) | done (2026-10-28) |
| WP-006 | harness follow-ups: `run-args:`/`run-stdin:`/`env:`, ci corpus self-check, oracle hang=ERROR, `-fconstexpr-report` protocol (`spec/oven.md`), bench per-entry thresholds + oven counters, heavier pp entries. **Still open:** re-record all `bench/baselines/*.json` on a quiet host (load < 4; durable reminder set) | done except baseline re-record (2026-10-28) |
| WP-004 | `xtask bench`: run `bench/` entries, record wall/RSS/oven stats, compare with baselines (see `spec/bench.md`) | done (2026-10-28) |
| WP-005 | `resource/`: import clang 19 builtin headers + generated predefines for aarch64/x86_64 linux; script to regenerate | done (2026-10-28) |

## M1 — preprocessor parity (unlocked by WP-001..003)
| id | scope | status |
|---|---|---|
| WP-010 | `kiln_lex`: phases 1–2 + identifiers, punctuators, pp-numbers, comments, flags (`spec/lex.md`) | done (2026-10-28) |
| WP-011 | `kiln_lex`: literal escapes validation (simple/octal/hex/`\o{}`/`\x{}`/`\u`/`\N{}`), full Unicode name table + aliases + Hangul + loose matching for `\N{}`, header-name diagnostics, encoding/multichar checks, NFC_QC=Maybe, literal *value* decoder shared with sema/oven, C mode flag | todo |
| WP-012 | `kiln_pp` core: crate structure, lexer stack + `Eod` protocol, directive parsing, conditional stack (`#if` evaluator stubbed to a hook), `#define`/`#undef`/redefinition rules, the full macro engine (object-like + function-like, `#`, `##`, placemarkers, `__VA_OPT__`, rescan + hide sets, expansion locations + `<scratch space>`), builtin macros (`__LINE__`, `__FILE__`, `__COUNTER__`, `__DATE__`/`__TIME__`...), callbacks, `#error`/`#warning`/`#pragma` plumbing; include/`#if`-eval/`#embed`/printer left as module hooks | todo |
| WP-013 | (merged into WP-012) | — |
| WP-014 | `kiln_pp`: `#include` resolution, `#include_next`, `#pragma once`, include guards, `__has_include*`, include depth, callbacks | todo |
| WP-015 | `kiln_pp`: `#if` expression evaluator, `defined`, `__has_*` builtins table, `#line`, `#error/#warning`, pragmas, `_Pragma` | todo |
| WP-016 | `kiln_pp`: `#embed` | todo |
| WP-017 | `kiln_pp::print`: `-E` printer with clang line markers, `-P`, `-C/-CC`, `-dM/-dD` | todo |
| WP-018 | driver: `kiln -E` flag parsing subset + `-M*` dependency output | todo |
| WP-019 | differential corpus: all libc++ 19 headers + sysroot C headers + clang Preprocessor tests; `xtask test` pp-diff mode | todo |
| WP-020 | predefined macro audit: `__cpp_*` values kiln claims for M1; decide `__VERSION__`/`__clang_version__` strings, GNU-mode (`gnu++NN`) macro delta, which macros the driver supplies from flags (`__OPTIMIZE__`, `__PIE__`, `__EXCEPTIONS`, `__GXX_RTTI`, `__GCC_HAVE_DWARF2_CFI_ASM`) vs the static files — see `resource/README.md` | todo |

## Cross-cutting
| id | scope | status |
|---|---|---|
| WP-030 | perf/stress corpus from mirrorbind: `corpus/lib/synth/make_lib.py` generates synthetic reflection-heavy libraries (`corpus/runs/synth_*`, see mirrorbind `PERF.md`); wire a generated set into `bench/mirrorbind/` once kiln parses templates (M3) and reflection (M6); until then use their preprocessed/`-E` forms for pp benches (M1) | todo |
