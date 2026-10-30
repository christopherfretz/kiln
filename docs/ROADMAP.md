# Kiln Roadmap

Milestones are ordered so that each one produces a *measurable* artifact and
unlocks parallel work for the next. Exit criteria are tests in the repo, not
opinions. Status is tracked in `docs/BACKLOG.md`; this file is the plan.

Targets: `aarch64-unknown-linux-gnu` (dev container) first, `x86_64` second.
Oracle/backend: clang/LLVM 19. Reference library:
the libc++ shipped with that LLVM.

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
- `kiln -E` on every header of libc++ 19, every C header in the sysroot that
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
Status: done. `kiln -fsyntax-only -fdump-ast` matches clang on 47 golden
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

## M3 — Classes, overloading, templates
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

## M4 — The rest of the language
Namespaces/ADL corner cases, exceptions, RTTI, coroutines, modules (at least header units and `import std;` consumption),
`consteval`, `constinit`, structured bindings,
three-way comparison, deducing this, attributes, `thread_local`,
static initialization order, inline variables.
- Exit: `<vector>`, `<string>`, `<memory>`, `<optional>`, `<variant>`,
  `<tuple>`, `<algorithm>`, `<ranges>`, `<format>` compile; their libc++
  tests pass.

## M5 — libc++ test suite
Run the full libc++ lit suite with kiln as `%{cxx}` against the
clang-built libc++ runtime. Track pass rate in `docs/status/libcxx.md`.
- Exit: 100% of tests pass or are triaged as `UNSUPPORTED`/`XFAIL` with a
  written reason that is not "kiln bug".
- Stretch: build libc++ itself with kiln and run the suite against it.

## M6 — C++26 reflection and mirrorbind
P2996 reflection, `std::meta` runtime in the Oven, splicing, annotations
(P3394), `define_aggregate`, expansion statements (P1306), plus whichever of
token injection makes it into the IS. Reference compiler: **GCC 16.1.0**
(it builds mirrorbind; the Bloomberg p2996 clang fork does not and is used
only as an advisory oracle for AST/diagnostic spot checks — see BACKLOG
WP-102). Reflection tests default to `oracle: none` with goldens written
from the P2996 text.
- Exit: mirrorbind (`git@github.com:christopherfretz/mirrorbind.git`) builds
  and its tests pass; its compile time and peak memory with kiln are recorded
  against GCC 16.1 (whose constexpr memory growth is the problem kiln exists
  to fix).

## M7 — Performance campaign
With correctness pinned by M5/M6, optimize the Oven and the frontend against
the `bench/` corpus: template-heavy code, reflection-heavy mirrorbind, and
synthetic constexpr workloads. Publish the numbers.

## Continuous
- Every milestone adds to `tests/conformance/` and `bench/`.
- Diagnostics quality is tracked from M1 (locations, notes, fix-its later).
- `x86_64` is kept building from M2 on via cross-compiled IR tests.
