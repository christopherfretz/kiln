# Backlog — work packages

Format per `docs/AGENTS.md`. Ids are stable; status moves between
`todo / in-progress(agent) / review / done`. The supervisor owns this file.

## M0 — bootstrap
| id | scope | status |
|---|---|---|
| WP-001 | `kiln_source`: implement `spec/source.md` with unit tests | done (2026-10-28) |
| WP-002 | `kiln_diag`: implement `spec/diag.md` (engine, ids table skeleton, text + JSON renderers) | done (2026-10-28) |
| WP-003 | `xtask test`: conformance runner implementing the directive format in `docs/VALIDATION.md` (see `spec/harness.md`) | done (2026-10-28) |
| WP-006b | bench: `--record` must MERGE rows into `docs/status/perf.md` instead of regenerating from the filtered run (a `--filter` record currently wipes other groups' rows); intern-cache TLS 32 KB could be inverted to leave initialised data | todo |
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
| WP-052 | L codegen M2 part 1: mangling + call ABI, module/function emission, constants, expr/stmt lowering — 73/74 constructs byte-identical to clang -O0 (aarch64) | done (2026-10-28) |
| WP-052b | L codegen M2 part 2: driver `-S -emit-llvm`/`-S`/`-c`/link (`codegen_cmd.rs`, `link.rs`, `write_report` on all paths, `-###`/`-v`), `tests/codegen/*.cpp` IR-diff corpus vs clang on BOTH targets (x86-64 call paths + align-16 local arrays untested), ~30 printf `run` tests, `xfail(kiln)` on header-dependent run tests; M2 exit = `run` tests pass through kiln vs oracle | todo — NEXT |
| WP-057 | sema: compound-assignment LHS promotion (bit-fields, `char32_t`, enums); `bitfields_compound` lowering case is Exact (74/74) | done (2026-10-28) |
| WP-058 | exception specifications (`noexcept`, `noexcept(e)`, `throw()`), `noexcept` operator, `if constexpr`/`if consteval` (+ init-statement); `<climits>` clean, `<cstring>` 137→56 errors | done (2026-10-28) |
| WP-054 | Oven↔sema integration (one Oven per TU via SemaBridge; folder/aggval deleted; `-fconstexpr-*` flags + report; references; string-literal identity; labels/goto; 99 expr.const source tests; oven baselines: kiln 0.02x clang time, 0.04x memory) | done (2026-10-28). Open: codegen path must call `write_report`; `constinit` not wired; sema rejects temporaries (M3) |
| WP-055 | parser consumes kiln_pp tokens (8-token lookahead window), pragmas, module/import stubs, shared driver pp construction; libc++ first blockers in `docs/status/libcxx-syntax.md` (templates, using-declarations, exception specs, asm labels, `__builtin_is_constant_evaluated`) | done (2026-10-28). Open: system-header warning suppression; sema diagnostics lack source ranges (no outer "expanded from macro" notes); `clang diagnostic` pragmas not applied |
| WP-056 | preprocessor performance: headers 3.56x→1.42x, repetition 171x→0.52x, xmacro 5.22x→0.99x of clang (`cargo xtask bench`, load ~8); per-inclusion source ranges + "In file included from" notes; release profile: `codegen-units=1` for kiln_lex/source/pp | done (2026-10-28). Follow-ups: `Lexer::next` ~185 instr/token (rare paths out of line, `check_poison`, double macro lookup, resolver allocations); unknown-directive warnings in skipped groups; backslash-space column off by one |

## M3 preparation
| id | scope | status |
|---|---|---|
| WP-060 | Oven M3 memory design: `spec/oven.md` "Transient allocation" — heap space with `HEAP_BIT` slots, free-on-delete with exact/half-step size classes + coalesced large regions, `peak_arena`/`frozen_bytes` counters, churn benchmark table with gates fixed from live-object counts | done (2026-10-28, design) |
| WP-061a | parse+sema `new`/`delete` expressions on non-class types ([expr.new]/[expr.delete]: `new T`, `new T[n]`, `new T(init)`/`{init}`, `delete`, `delete[]`, array-bound diagnostics as clang; `CXXNewExpr`/`CXXDeleteExpr` payload in `spec/ast.md`; `-fdump-ast` + codegen lowering to `operator new`/`operator delete` calls, Exact vs clang) | todo (after 052b lands) |
| WP-061b | Oven heap space per `spec/oven.md` "Transient allocation": `mem.rs` heap space + size-class free lists + coalescing (unit-testable without 061a), `New`/`Delete` KBC ops + checks with clang wording, `peak_arena`/`frozen_bytes` in report/harness/bench, `-fconstexpr-memory` over `peak_arena`, the five `churn-*` benchmarks + `tests/conformance/expr.new/` with the spec's gates, footprint-flat test extended | todo (mem.rs part can start now; glue after 061a) |

## M3 waves (dependency order; each row ≤5 deliverables, one agent-session)
Spec first (S-rows, supervisor reviews), then implementation. "needs" lists
the rows that must be MERGED first; rows with the same needs run in parallel.

| id | scope | needs | status |
|---|---|---|---|
| WP-S3C | spec: `spec/sema-class.md` — classes, members, `this`, access, bases (non-virtual then virtual), special members (implicit declaration/definition, triviality, deletedness), ctor-initializers, NSDMI, temporaries + `MaterializeTemporaryExpr` + lifetime extension, record layout (bases, vptr) vs clang `-fdump-record-layouts` | — | todo |
| WP-S3O | spec: `spec/sema-overload.md` — candidate sets ([over.match.*]), viability, user-defined conversion sequences (converting ctors, conversion functions), ranking + tie-breakers, operator overloading incl. builtin candidates and `<=>`/`==` rewrites, implicit object parameter, address of overloaded function, default arguments | — | todo |
| WP-S3T | spec: `spec/sema-templates.md` — parsing (template-ids, `typename`/`template`, packs, requires), dependent names/two-phase lookup, class template instantiation (lazy, member-wise), partial/explicit specialization matching, alias/variable templates, function template deduction ([temp.deduct]), partial ordering, SFINAE + instantiation stack, packs/folds, concepts/constraint satisfaction/subsumption, builtin traits table (`__is_same` …), instantiation memory accounting | — | todo |
| WP-062 | using-declarations (`using ::name;`, `__using_if_exists__`), using-directives, namespace aliases, `using enum`, `UsingShadow` lookup | — | todo |
| WP-063 | asm labels on declarations, `__builtin_is_constant_evaluated` (+ Oven/codegen), system-header warning suppression (clang `-isystem` rule), `__builtin_*` declaration table for the C-library builtins libc++ wrappers use | — | todo |
| WP-064 | parser expressions wave: tentative-parse disambiguation (`T(x)`, params, `(T)(x)`), `T()`/`T{}`/`T(a,b)` temporaries, braced-init-list args, default arguments, trailing return types, `throw`, `typeid`, `dynamic_cast`, compound literals, statement expressions | — | todo |
| WP-065 | parser classes: member functions (deferred in-class bodies, [class.mem]/7), access specifiers, base-clauses, ctor/dtor/conversion/operator names, ctor-initializers, static data members, NSDMI, friends, `= default`/`= delete`, `this` | — | todo |
| WP-066 | sema classes 1: member functions, `this`, implicit object param, access control, static members, NSDMI, complete-class contexts, record layout with non-virtual bases + layout-dump differential test | S3C, 065 | todo |
| WP-067 | sema classes 2: special members, ctor-initializers, member init order, aggregates with bases, temporaries/`MaterializeTemporaryExpr`/lifetime extension, copy elision as clang | 066 | todo |
| WP-068 | overload resolution per S3O; `CXXOperatorCallExpr`, `CXXMemberCallExpr`; user-defined conversions in ICS (extends WP-049 ranking) | S3O, 066 | todo |
| WP-069 | parser templates: template-declarations, template-ids, `typename`/`template`, packs, `sizeof...`, folds, concept/requires parsing, explicit specialization/instantiation syntax | S3T, 065 | todo |
| WP-070 | sema templates 1: template decls + dependent types/exprs, class template instantiation (lazy member-wise), partial/explicit specializations, alias + variable templates, static members of templates, instantiation stack diagnostics | 069 | todo |
| WP-071 | sema templates 2: function templates, deduction from calls/explicit args/defaults, partial ordering, SFINAE, integration with overload resolution | 070, 068 | todo |
| WP-072 | packs: variadic templates, pack expansion in all contexts, fold expressions, `sizeof...` | 071 | todo |
| WP-073 | concepts: constraint normalization, satisfaction, subsumption, `requires`-expressions, constrained `auto`, abbreviated function templates | 071 | todo |
| WP-074 | `auto`/`decltype`/`decltype(auto)`, `declval`, the builtin type-trait table (`__is_same`, `__is_base_of`, `__is_constructible`…, per S3T table), `__make_integer_seq`/`__type_pack_element` | 070 | todo |
| WP-075 | lambdas: closure types, captures (copy/ref/init/`this`), generic lambdas, constexpr lambdas | 067, 068 (generic: 071) | todo |
| WP-061b | Oven heap (see M3 preparation) | 061a | todo |
| WP-076 | Oven classes: ctors/dtors/`this`/member calls, `BeginLife`/`EndLife` subobjects, temporaries, unions with ctors, pointers to members, `std::construct_at`/allocator recognition | 067, 061b | todo |
| WP-077 | Oven virtual + RTTI: dynamic type in allocations, virtual calls via `SemaBridge::vtable`, `dynamic_cast`/`typeid` in constant evaluation | 076, 078 | todo |
| WP-078 | sema/abi virtual inheritance: vptr/vtable layout (Itanium), virtual bases, final/override, `dynamic_cast`/`typeid` semantics, layout-dump differential | 067 | todo |
| WP-079 | codegen classes: record types with bases, member functions/`this`, ctors/dtors (complete/base variants), `new`/`delete`, member pointers, temporaries; mangling for classes/templates in kiln_abi | 067, 061a | todo |
| WP-080 | codegen virtual: vtables, RTTI, virtual calls, thunks; template instantiation emission (linkonce_odr), `-fdump-record-layouts` | 078, 079 | todo |
| WP-081 | `cargo xtask libcxx` M3 gate: `<type_traits>`, `<utility>`, `<array>`, `<string_view>`, `<span>`, `<bit>`, `<limits>`, `<initializer_list>` compile; their libc++ tests run through kiln; status page `docs/status/libcxx-m3.md` | 072, 073, 074, 076, 079 | todo |

## Lessons
- Size packages so one agent finishes well within a session: codegen should have been ABI+mangling / lowering / driver+tests from the start (the pp engine ended up split the same way). Prefer 3 small briefs over 1 large one; a brief that lists >5 deliverables is too big.

## Cross-cutting
| id | scope | status |
|---|---|---|
| WP-030 | perf/stress corpus from mirrorbind: `corpus/lib/synth/make_lib.py` generates synthetic reflection-heavy libraries (`corpus/runs/synth_*`, see mirrorbind `PERF.md`); wire a generated set into `bench/mirrorbind/` once kiln parses templates (M3) and reflection (M6); until then use their preprocessed/`-E` forms for pp benches (M1) | todo |
