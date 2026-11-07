# Kiln Roadmap

Milestones are ordered so that each one produces a *measurable* artifact and
unlocks parallel work for the next. Exit criteria are tests in the repo, not
opinions. Status is tracked in `docs/BACKLOG.md`; this file is the plan.

Targets: `aarch64-unknown-linux-gnu` (dev container) first, `x86_64` second.
Oracle/backend: clang/LLVM 22 (since WP-355; clang/LLVM 19 stays installed as
the second oracle and the frozen M3 gate). Reference library: the libc++
shipped with that LLVM.

## M0 — Bootstrap — DONE 2026-10-28
- Cargo workspace with every crate stubbed, `forbid(unsafe_code)`, clippy +
  rustfmt clean, `cargo test` green.
- `cargo xtask` harness skeleton: conformance test runner with the directive
  format in `docs/VALIDATION.md`; perf recorder (wall clock + max RSS).
- Provisioning script; CI workflow (GitHub Actions) running fmt/clippy/test.
- Specs: `spec/source.md`, `spec/lex.md`, `spec/pp.md`, `spec/oven.md` drafted
  enough to brief implementers.
- Exit: `cargo xtask test` runs and passes an empty/sanity suite; first
  work packages for M1 issued.

## M1 — Preprocessor parity with clang — DONE 2026-10-28
Result: 291/291 token-wise vs `clang -E` (all libc++ 19 public headers, wrapped
C headers, sysroot headers, torture corpus); `bench/pp` 0.52x–1.42x of clang
time at ≤ clang memory. See `docs/status/pp-parity.md`, `docs/status/perf.md`.
The first fully-conformant, independently valuable component, and highly
parallelizable (lexer, macro expander, directives, include resolution, `-E`
printer, diagnostics are separate work packages).
- `kiln++ -E` on every header of libc++ 19, every C header in the sysroot that
  libc++ includes, and the conformance corpus produces token streams equal to
  `clang -E` (compared token-wise, ignoring whitespace/line markers; and
  exactly, with `-P`, for the printer test).
- Full [lex] and [cpp]: UCNs, raw strings, user-defined literals, `#embed`,
  `__VA_OPT__`, `_Pragma`, `#pragma once`, `__has_include`,
  `__has_cpp_attribute`, `__has_builtin` (clang's list), `__COUNTER__`,
  digit separators, alternative tokens, `import`/`module` directive tokens.
- Diagnostics for every error in [cpp] with locations matching clang's.
- Perf: preprocessing all libc++ headers within 1.5x of clang's `-E` time
  and lower peak RSS. Recorded in `bench/baselines/pp.json`.

## M2 — The C-ish subset, end to end, with the Oven — done (2026-10-28)
Status: done. `kiln++ -fsyntax-only -fdump-ast` matches clang on 47 golden
directories; 99 [expr.const] source tests; Oven 0.02x clang time / 0.04x
memory on `bench/oven`; codegen byte-identical to clang -O0 on the IR-diff
corpus (46 programs × 2 triples) and the 79-case unit corpus; 33 printf
`run` tests pass vs the oracle through `kiln` driving `llc-19`/`clang++-19`.
A vertical slice that exercises every crate: parse, sema, Oven, ABI, codegen,
driver. Scope: fundamental types, arithmetic and conversions, pointers,
arrays, enums, structs (no members functions yet), functions, all C control
flow, `constexpr` functions and variables, `static_assert`, string literals,
`extern "C"`.
- Programs in `tests/conformance/run/` compile with kiln, link against libc,
  run, and produce the same output as when built with clang.
- `static_assert` over constexpr functions evaluates in the Oven; every
  [expr.const] undefined-behavior category reachable in this subset is
  diagnosed (overflow, OOB, uninitialized read, lifetime).
- Oven metrics are reported and recorded; first perf comparisons vs clang on
  a C-subset constexpr corpus.
- IR differential tests: kiln's `-S -emit-llvm` is semantically compared with
  clang's via `opt`-normalized FileCheck goldens where structure matters.

## M3 — Classes, overloading, templates — done (2026-11-01)
Status: done. The M3 libc++ gate (`cargo xtask libcxx --gate m3`) is 585/585
on main `4c3e8be9` (`docs/status/libcxx-m3.md`; since WP-143 the gate is 642 tests incl. `.verify.cpp`:
636 pass and 6 unsupported since WP-367/WP-371, WP-253's six `.verify.cpp` failures closed). The clang 22
oracle switch (WP-184 C–E) is WP-355 in M4.
Member functions, constructors/destructors, references, overload resolution,
implicit conversion sequences, class templates, function templates, deduction,
partial/explicit specialization, `auto`, variadic templates, fold expressions,
`decltype`, `if constexpr`, concepts and constraints, pack indexing, lambdas,
inheritance (non-virtual first, then virtual), operator overloading,
RAII/`new`/`delete`, Itanium mangling and
record layout matching clang (checked by layout-dump differential tests).
- Oven: objects with lifetimes, constructors in constant evaluation,
  `std::construct_at`-style, unions' active member, pointers-to-members,
  virtual calls in constexpr, transient allocation, memoization.
- Exit: `<type_traits>`, `<utility>`, `<array>`, `<string_view>`, `<span>`,
  `<bit>`, `<limits>`, `<initializer_list>` from libc++ compile and their
  libc++ tests pass.

## M3b — libc++ 19 end to end — done (2026-11-02)
Status: closed by operator decision at 98.3% of `std/` (7469 pass / 92 fail / 29 xfail / 1110 unsupported /
5 timeout on fe5406b9; `docs/status/libcxx.md`). The remainder is coroutines (WP-351, M4), environment
exclusions (WP-340) and small sema clusters tracked as follow-up rows; the ≥ 99% target was a goal, not a
gate, and M4 started the same day (WP-351, WP-355…359).
Before any C++23/26 feature work or the clang 22 oracle switch, the C++20-level
base must be fully functional, and the measure is libc++ 19 driven end to end
with the clang 19 oracle still in place: every libc++ 19 header compiles, the
full test suite (8695 `.cpp`, 7898 under `std/`) runs under
`cargo xtask libcxx`, and the pass rate is the headline number
(`docs/status/libcxx.md`).
- Method: a 10% stratified sample (every 10th `std/` test, 790 files,
  `xtask libcxx` over the list) is cheap (~20 min at -j 3) and is rerun after
  each batch of fixes; failures are clustered by diagnostic and the biggest
  clusters become work packages (WP-268…276 are the first batch). Full-suite
  runs baseline the status page between batches.
- libstdc++ is secondary: `xtask libstdcxx` (`docs/status/libstdcxx.md`) is run
  for overlap, and clusters that also block libc++ ride along.
- Exit: full libc++ 19 suite ≥ 99% pass, remainder triaged as `UNSUPPORTED`/
  `XFAIL` with a written reason that is not "kiln bug"; the per-directory gates
  in `xtask libcxx` cover the whole of `std/`. Then WP-184 steps C–E (oracle
  switch) run and M4 begins.

## M4 — Complete C++23, and C++26 as shipped by clang 22
Namespaces/ADL corner cases, exceptions, RTTI, coroutines, modules (at least header units and `import std;` consumption),
`consteval`, `constinit`, structured bindings,
three-way comparison, deducing this, attributes, `thread_local`,
static initialization order, inline variables.
- **Feature completeness is the exit, not a header list.** `docs/STANDARD_SUPPORT.md`
  holds one row per C++23 language feature (P0847 deducing `this`, P1938
  `if consteval`, P1169 static `operator()`, P2128 multidimensional `operator[]`,
  P0849 `auto(x)`, P1774 `[[assume]]`, P2334 `#elifdef`/`#elifndef`, P2437 `#warning`,
  P1467 extended floating-point types, P2718 range-for lifetime extension,
  P2448/P2242/P2280 constexpr relaxations, P2582 CTAD from inherited constructors,
  P2324 labels at the end of blocks, P0330 `size_t` literals, P2360 alias declarations in init-statements, P2290 delimited escapes,
  P2071 named universal character escapes, P2295 UTF-8 source, P2362 wide multichar
  literals, P2156 duplicate attributes, P2266 simpler implicit move, P2468 equality
  operator rewrites, P2493 `__cpp_*` macros, DR-level items) and per non-reflection
  C++26 feature that clang 22 implements (P2662 pack indexing, P2169 placeholder
  `_`, P2573 `= delete("reason")`, P2741 `static_assert` user messages, P2893
  variadic friends, P2747 `constexpr` placement new, P3068 `constexpr` exceptions,
  P2738 `constexpr` `void*` casts, P1967 `#embed`, P0963/P1061 structured bindings
  in conditions and with packs, P2795 erroneous behavior, P2786 trivial relocation,
  P2300-adjacent library hooks, expansion statements where clang ships them).
  Each row names its corpus/libc++ tests; status is `done`/`partial`/`todo`.
  Seeded by WP-185; `cargo xtask features` checks it (done rows have passing tests and clang 22's macro value).
- **Oracle switch (WP-184):** at M3b exit the differential oracle, libc++ headers and
  libc++ test checkout move from clang/llvm 19 to clang/llvm 22 (both are in the
  sandbox). The M3 gate is frozen on 19 so its numbers stay comparable; M4+ gates run
  on 22 in `-std=c++26`. Steps A/B (the `KILN_ORACLE` and `-fclang-compat`
  knobs) landed early (WP-262/263); C–E landed as WP-355 (2026-11-02): `KILN_ORACLE`
  defaults to 22, CI also runs `KILN_ORACLE=19 cargo xtask ci --quick`, and the
  M4 libc++ baseline is in `docs/status/libcxx.md` (`docs/status/wp-184-oracle-switch.md`); WP-398 took the M4 gate from 11.7% to 95.0% (2448/2661); 99.9% (2632/2661, 2 fail) after WP-416+WP-418, WP-445 owns the 2.
- Exit: every C++23 row is `done`; every clang-22 C++26 row is `done` or has a
  written reason; `<vector>`, `<string>`, `<memory>`, `<optional>`, `<variant>`,
  `<tuple>`, `<algorithm>`, `<ranges>`, `<format>`, `<expected>`, `<mdspan>`,
  `<print>`, `<flat_map>` from libc++ 22 compile and their libc++ tests pass.

## M5 — libc++ test suite
Run the full libc++ 22 lit suite in `-std=c++26` with kiln as `%{cxx}` against the
clang-built libc++ runtime. This is also the end-to-end proof of the C++23/26
library surface (M4's feature table covers the language). Track pass rate in `docs/status/libcxx.md`.
- Exit: 100% of tests pass or are triaged as `UNSUPPORTED`/`XFAIL` with a
  written reason that is not "kiln bug". A `.verify` test that fails only because
  kiln's diagnostic wording or location differs from clang's is triaged as
  not-a-kiln-bug and does not block M5 (operator decision, 2026-11-03).
- Stretch: build libc++ itself with kiln and run the suite against it.

## M6 — C++26 reflection and the reference workload
*The reference workload* is a private, reflection-heavy downstream C++26
codebase that motivates kiln; it is referred to only by that name here.
P2996 reflection, `std::meta` runtime in the Oven, splicing, annotations
(P3394), `define_aggregate`, expansion statements (P1306), plus whichever of
token injection makes it into the IS. Reference compiler: **GCC 16.1.0**
(it builds the reference workload; the Bloomberg p2996 clang fork does not and is used
only as an advisory oracle for AST/diagnostic spot checks — see BACKLOG
WP-102). Reflection tests default to `oracle: none` with goldens written
from the P2996 text.
- **Standard libraries.** `<meta>` is mostly compiler-intrinsic glue, so the
  library picks the intrinsic surface: kiln keeps ONE reflection core in the
  Oven (`std::meta::info` as a native value, splices, annotations,
  `define_aggregate`) behind two thin intrinsic shims, the p2996 fork's
  `__metafunction(...)` for its `<experimental/meta>` (to get off the ground and
  run the fork's tests and the reference workload early) and GCC 16.1's for libstdc++'s
  production `<meta>`. The same reflection programs run through both headers as
  a differential check. A kiln-owned `<meta>` is a fallback only. The maturity
  bar for the compiler is consuming BOTH libc++ (M4/M5 gates) and libstdc++
  (consumption track from M4, WP-188; the reference workload under GCC uses it).
  libstdc++ header status: `docs/status/libstdcxx.md` (`cargo xtask libstdcxx`).
- Plan and decisions: `docs/plans/m6-reflection.md` (16 work packages,
  WP-470..485; GCC 16.1.0 provisioned on the dev host 2026-11-03; libstdc++
  16.1 `<meta>` is the production header, the test header until then).
- Exit: the reference workload builds
  and its tests pass; its compile time and peak memory with kiln are recorded
  against GCC 16.1 (whose constexpr memory growth is the problem kiln exists
  to fix). Exit recorded 2026-11-05 (docs/plans/m6-reflection.md §5); the
  one open perf item (one wide binding TU 3.5x slower than GCC) is carried
  into M7 as WP-641/WP-642.

## M7 — Performance campaign
With correctness pinned by M5/M6, optimize the Oven and the frontend against
the `bench/` corpus: template-heavy code, the reflection-heavy reference workload, and
synthetic constexpr workloads. Publish the numbers.

## M8 — Robustness: real-world open-source builds
Build real open-source C++ projects that already build with clang, with
kiln as the C++ compiler, through each project's OWN build system and test
suite, and turn every failure into a reduced `tests/conformance` case plus
a fix. Harness: `scripts/oss/` (spec/landed/harness/WP-666.md); project
list and pinned tags: `scripts/oss/projects.toml`; status and failure
clusters: `docs/status/oss.md`. Control build: the same harness with
clang-19.
- Wave 1 (fmt, nlohmann/json, spdlog, Catch2, googletest, doctest,
  magic_enum, CLI11, tl-expected, range-v3) and wave 2 (abseil, RE2,
  benchmark, glog, yaml-cpp, simdjson, Eigen, EnTT, Taskflow, cereal):
  every project configures, builds and passes its test suite at parity
  with clang-19; zero kiln panics; zero false rejects; no TU over the
  harness caps (3 GB address space, 600 s).
- Wave 3 (protobuf, LevelDB, RocksDB, LLVM lib/Support + lib/ADT unit
  tests, folly as a stretch): a tracked pass rate per project.
- Exit: the above, and every open failure cluster in `docs/status/oss.md`
  is "not a kiln bug" with a written reason.

## Continuous
- Every milestone adds to `tests/conformance/` and `bench/`.
- Diagnostics quality is tracked from M1 (locations, notes, fix-its later).
- `x86_64` is kept building from M2 on via cross-compiled IR tests.
