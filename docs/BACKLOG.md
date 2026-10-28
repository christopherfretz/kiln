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
| WP-011 | `kiln_lex`: literal escapes validation, `\N{}` full name table + loose matching, header-name diagnostics, literal value decoder (`kiln_lex::literal`), number classifier, full NFC | done (2026-10-28). Open: invalid-UCN-in-identifier recovery vs clang; C mode; GNU/MS suffixes | 
| WP-012 | `kiln_pp` core + macro engine + builtins + pragmas + `-E` printer + include-guard optimization (226 tests vs clang) | done (2026-10-28). Open: `#embed` not streamed; per-inclusion source ranges; `-dD/-dI` placement |
| WP-013 | (merged into WP-012) | — |
| WP-014 | include resolver, predefines, `kiln -E` driver, `-M*`, hooks installed; **pp parity 275/276 token-wise (libc++ 106/106, sysroot 92/92, torture 48/48)** | done (2026-10-28) |
| WP-015 | `IfEval` (clang-port `#if` evaluator, 0/1228 differential diffs) + `ClangFeatures` tables (2309 builtins, 128 features, 310 C++ attrs, 35 flag overlays; generated/verified by `scripts/gen-features.sh`) | done (2026-10-28). Decisions: unsupported triples → driver error (not silent NoFeatures); `-march` ignored for now (M7); `#embed` `defined` rule follows the standard |
| WP-016 | `#embed` resolver (part of WP-014's resolver) + streaming (WP-012 open item) | folded into WP-014 / later |
| WP-017 | `-E` printer | done inside WP-012 (`-dD/-dI` placement open) |
| WP-018 | driver `-E` | folded into WP-014; `-fsyntax-only -fdump-ast` wiring after WP-048 |
| WP-019 | differential corpus: 223 generated header tests (libc++ 19 public + wrapped C + sysroot) + 48 torture cases, token-wise `-E` comparison via kiln_lex, `%{resource_dir}` placeholders, `--report-parity docs/status/pp-parity.md` | done (2026-10-28) |
| WP-020 | **Decision (M1):** kiln predefines exactly clang 19's set for the target/std (differential `-E` parity requires it) plus `__KILN__`/`__KILN_MAJOR__`/`__KILN_MINOR__`; `__VERSION__`/`__clang_version__` stay clang's strings for now. Audit of `__cpp_*` claims deferred to M2+. predefined macro audit: `__cpp_*` values kiln claims for M1; decide `__VERSION__`/`__clang_version__` strings, GNU-mode (`gnu++NN`) macro delta, which macros the driver supplies from flags (`__OPTIMIZE__`, `__PIE__`, `__EXCEPTIONS`, `__GXX_RTTI`, `__GCC_HAVE_DWARF2_CFI_ASM`) vs the static files — see `resource/README.md` | todo |

## M2 preparation
| id | scope | status |
|---|---|---|
| WP-040 | `spec/ast.md` + `spec/types.md` + `spec/ast-dump.md` design | done, approved with decisions (types.md §17, ast.md §15) (2026-10-28) |
| WP-041 | A ast-core (ids, arenas, headers, lookup/redecl, MemReport, AstBuilder; Symbol/Interner → kiln_source) | done (2026-10-28). Follow-up: id exhaustion panics instead of a fatal diagnostic |
| WP-042 | B types (TypeContext, interning, canonical, M2 kinds, template-arg tables, layout hooks, clang-port printer) | done (2026-10-28) |
| WP-043 | C const-store (ConstStore, StringPool, float formatting, printing) | done (2026-10-28). Sema obligation: `&s` vs `&s.x` at equal offsets intern identically — [temp.type] equivalence must compare designators in sema, not ConstValueId |
| WP-044 | D dump printer (clang format, 14 goldens byte-exact) | done (2026-10-28). Sema obligations: propagate bit-field lvalue marking through operators; `TypedefNameForAnonDecl` storage for `typedef struct {..} T` (dcl.typedef/aliases golden) | |
| WP-045 | E dump loader (text → Ast) | deferred — parser (WP-048) is in progress; Oven/codegen test on parsed sources instead. Revive only if H slips |
| WP-046 | F+G: `xtask clang-ast` normalizer + harness `expect: ast` + 32 AST goldens; ast-dump spec reconciled with clang | done (2026-10-28). Decisions: second type string = clang's top-level desugar (types.md §12 note); ParenExpr kept; values clang can't give (floats in ConstantExpr, pointers, long double) are checked by kiln-only `expect: ast` tests later |
| WP-047 | (merged into WP-046) | — |
| WP-048 | H parser M2 + Sema core (full M2 subset; `kiln -fsyntax-only -fdump-ast` end to end; 47 AST golden dirs, 74 diag tests) | done (2026-10-28). Open: parser lexes the file itself — must take kiln_pp tokens (WP-055); `aggval` unions/bit-fields; [dcl.align]/6; jumps past init | |
| WP-049 | I sema conversions ([conv], [expr.arith.conv], [over.ics] ranking, narrowing, literal typing; 104 tests, clang wording) | done (2026-10-28). M3: user-defined conversions, temporaries, access checks | |
| WP-050 | J abi layout (targets, Itanium record layout w/ bit-fields/packed/aligned/no_unique_address, clang-exact on 640 random records per run) | done (2026-10-28). Open: `#pragma pack` needs sema state; bases/vptr are M3 (spec/abi.md §8) | |
| WP-051 | K Oven M2 (KBC, VM, abstract memory, 56-case UB catalogue, exact clang step parity, memo, report; fib/sieve/sort 19–47x faster than clang) | done (2026-10-28). Open: string-literal identity needs `PtrBase::StringLit` to carry the literal ExprId; labels/goto (builders now exist from WP-048); MaterializeTemporaryExpr untested; enum range check stricter than clang |
| WP-053 | `kiln_llvm` IR model + builder + verifier + printer (40 corpus modules verified; 16 C programs match clang -O0 through `opt -S`) | done (2026-10-28) |
| WP-052 | L codegen M2 (AST → kiln_llvm, `-S -emit-llvm`/`-c`/link via clang; `run` tests vs clang oracle) | in-progress |
| WP-054 | Oven↔sema integration: replace sema's `ConstEvaluator` Folder with the Oven via `SemaBridge`; driver `-fconstexpr-steps/-depth/-memory`, `-fconstexpr-report`; `oven:` conformance tests; string-literal identity; labels/goto; tests/conformance/expr.const via parsed sources; `bench/oven` through kiln | in-progress |
| WP-055 | parser consumes kiln_pp tokens (`TokenSource` over `Preprocessor::next`), lexdiag.rs stopgap removed, `kiln -fsyntax-only` on files with `#include`; `lex.digraph/alternative-tokens.cpp` passes; `-fsyntax-only` default std = clang's gnu++17 | in-progress |
| WP-056 | macro-engine performance: `bench/pp/repetition` 147x and `xmacro` 5x slower than clang (suspect `expand.rs relocate()`/`loc_diff` walking location chains per token); target ≤1.5x clang on all pp entries; plus `SourceManager::add_inclusion` per-inclusion ranges + "In file included from" notes | in-progress |

## Cross-cutting
| id | scope | status |
|---|---|---|
| WP-030 | perf/stress corpus from mirrorbind: `corpus/lib/synth/make_lib.py` generates synthetic reflection-heavy libraries (`corpus/runs/synth_*`, see mirrorbind `PERF.md`); wire a generated set into `bench/mirrorbind/` once kiln parses templates (M3) and reflection (M6); until then use their preprocessed/`-E` forms for pp benches (M1) | todo |
