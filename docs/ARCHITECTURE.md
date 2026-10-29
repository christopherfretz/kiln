# Kiln Architecture

Kiln is a C++26 compiler frontend written in safe Rust. It consumes C++ source
and emits LLVM IR; LLVM (invoked as an external tool) does optimization, code
generation and linking. The defining feature is **the Oven**: a constant
evaluator designed from the start to be the fastest and leanest constexpr /
consteval / reflection engine available, with explicit tracking of wall-clock
time and memory for every evaluation.

This document is the map. Each crate has a contract in `spec/` and tests that
are tied to that contract; see `docs/VALIDATION.md` for how correctness is
established and `docs/ROADMAP.md` for sequencing.

## Non-negotiable principles

1. **Safe Rust end to end.** Every crate carries `#![forbid(unsafe_code)]`.
   No LLVM FFI: kiln writes LLVM IR itself (text now, bitcode later) and shells
   out to `llc`/`clang`/`lld`. Third-party dependencies are kept minimal and
   must themselves be safe or widely audited.
   *Exception policy (operator decision, 2026-10-28):* `unsafe` may be
   admitted only where a **measured** performance need dictates it, inside a
   **hermetic, narrowly bounded, separately reviewed component** (e.g. a
   future JIT for the Oven), with supervisor sign-off recorded in the spec.
   It is never the first tool reached for; diversions require justification.
2. **Index-based arenas, no pointer graphs.** Every compiler entity (token,
   declaration, type, expression, bytecode function, abstract allocation) is a
   `u32` id into a typed arena. This gives cheap copies, deterministic memory
   accounting, trivial serialization, and no `Rc` cycles.
3. **Conformance first, with clang as the oracle.** Where the standard is
   silent or implementation-defined, kiln matches clang on the same target so
   that libc++ builds and the Itanium ABI is honored. Kiln predefines the clang
   feature macros it actually implements (`__clang__`, `__clang_major__`, ...)
   plus `__KILN__`, and implements the clang builtins libc++ relies on
   (`__is_same`, `__type_pack_element`, `__make_integer_seq`, `__builtin_*`).
4. **Metrics are part of correctness.** Every constant evaluation reports steps,
   peak bytes, and time. The test harness records both and fails on regression.
5. **Specs over folklore.** Behavior lives in `spec/*.md` with references to
   the standard's stable section names ([expr.const], [temp.deduct], ...), and
   each spec has behavioral tests. A re-implementation of any crate from its
   spec plus its tests must be possible.

## Pipeline and crate layout

```
source text
  │  kiln_source   SourceManager, FileId, Span, SourceLoc (incl. macro expansion locs)
  │  kiln_diag     diagnostics, severities, rendering, -Werror/-Wno-* policy
  ▼
  kiln_lex        translation phases 1–3: UTF-8 decode, line splicing, pp-tokens
  kiln_pp         translation phase 4: directives, macro expansion, #include/#embed,
                  __has_include/__has_cpp_attribute, module/import preamble, -E output
  ▼
  kiln_ast        the typed AST: Types (interned, canonical), Decls, Exprs, Stmts,
                  templates, attributes. Stable textual dump format (ast-dump).
  kiln_parse      phases 5–7 syntax: a recursive-descent parser that calls Sema
                  as it goes (clang model: C++ syntax is not context-free).
  kiln_sema       name lookup, scopes, types, declarations, templates and
                  deduction, concepts, overload resolution, implicit conversions,
                  initialization, lambdas, coroutines, modules, reflection.
  ▼
  kiln_oven       THE constant evaluator: bytecode compiler + VM + abstract
                  memory model + std::meta runtime. Called by sema for every
                  constant expression, template argument, static_assert, if
                  constexpr, consteval call, and reflection query.
  ▼
  kiln_abi        Itanium C++ ABI: name mangling, record/vtable layout, EH tables
                  shape, target-specific argument lowering (AAPCS64, SysV x86-64).
  kiln_llvm       Rust model of LLVM IR (types, values, instructions, metadata,
                  attributes) with a textual printer. No FFI.
  kiln_codegen    typed AST → kiln_llvm module: expressions, control flow,
                  exceptions (landingpads), RTTI, vtables, thread_local, globals,
                  static init, debug info (later).
  ▼
  kiln_driver     the `kiln` binary: clang-compatible command line, -E/-fsyntax-only/
                  -S -emit-llvm/-c/link, resource dir discovery, invokes llc/lld.
  kiln_harness    test + benchmark runner (`cargo xtask ...`): conformance tests,
                  differential tests against clang, libc++ lit driver, perf baselines.
```

Dependency direction is strictly top-down in that list. `kiln_oven` depends on
`kiln_ast` and talks to sema only through a `SemaBridge` trait defined in
`kiln_oven` (for instantiation-on-demand and reflection); sema implements it.
This keeps the Oven testable in isolation with a mock bridge.

### Parser ↔ Sema coupling

C++ parsing needs name classification (type? template? namespace? value?),
which needs declarations processed so far, which can require instantiating
class templates (`std::vector<int>::iterator it;`). So the parser calls
`kiln_sema::Sema` directly, as clang does. The stable, testable contract is
not the parser/sema boundary; it is the **AST** and its dump format. Parser+sema
are validated together by AST-dump goldens and diagnostics tests; codegen and
the Oven are validated from AST inputs independently.

### Types

Types are interned in a `TypeContext`: one `TypeId` per canonical type and one
per sugared spelling (typedefs, template-ids), with `canonical(TypeId)` O(1).
Type equality for the language is canonical-id equality. Qualifiers are bit
flags on the type reference (`QualType = TypeId + Qualifiers`), as in clang.

### Declarations and scopes

`DeclId` arena with parent links; `DeclContext` lookup tables are hash maps
from interned `Name` to a small-vector of decl ids (overload sets). Templates
store their pattern AST plus an instantiation cache keyed by canonical
template-argument lists. Instantiation is lazy and recorded with a
`InstantiationStack` for diagnostics ("in instantiation of ...").

## The Oven (kiln_oven)

The Oven is a register-based bytecode VM with a byte-addressed abstract memory
model. It is where kiln must beat every existing implementation, so the design
is spelled out in detail in `spec/oven.md`; the summary:

- **Lazy bytecode compilation.** When a function is first needed in constant
  evaluation, its typed AST is compiled to `KBC` (kiln bytecode), cached for
  the TU. Each template instantiation compiles once. Compilation is a thin,
  mostly-linear pass; no optimization beyond constant folding and
  register allocation into a frame slot array.
- **Flat memory with shadow metadata.** Every abstract allocation (a complete
  object, a `new`ed block, a temporary, a string literal) is a contiguous
  byte buffer in target layout plus a shadow side-table recording, per byte
  range: initialized?, pointer provenance (which allocation + offset a stored
  pointer refers to), lifetime state, and the active member of unions.
  Scalars are raw bytes; aggregates are their layout. There is no per-value
  tree (clang's `APValue`) to allocate, walk or free. Bulk operations
  (`memcpy`, array copies, `std::array` initialization) are memcpy-speed.
- **Pointers are (AllocId, offset) with provenance.** Dereference is a bounds
  + lifetime + initialization check against the shadow, which is exactly what
  [expr.const] requires for rejecting undefined behavior. Invalid pointer
  arithmetic, out-of-bounds, use-after-lifetime, uninitialized reads, strict
  aliasing violations and overflow are all diagnosed, with the full call stack.
- **Stack-disciplined frame memory, real allocator for transients.** Frame
  objects live in a bump arena with mark/release at scope and call exit
  (released slots are reused FIFO with generation bumps, so dangling
  pointers are still detected); the arena is reset, not freed, between
  top-level evaluations, so steady-state cost is zero. Transient
  allocations (`new`/`delete`, `std::allocator`, M3) MUST be reclaimed on
  `delete` by a size-class allocator inside the arena — reclaiming only at
  the end of an evaluation is the g++ failure mode that motivates kiln and
  is forbidden. `peak_bytes`/`peak_arena`/`allocs` are reported per
  evaluation and gated by the bench on allocation-churn workloads. Results
  that escape are frozen into compact `ConstValue`s interned in the AST by
  content (reported separately as `frozen_bytes`).
- **Deterministic budgets.** Steps and bytes are counted exactly; limits are
  user-configurable (`-fconstexpr-steps`, `-fconstexpr-memory`) and every
  evaluation can report its cost (`-fconstexpr-report`). The harness uses the
  same counters for regression tracking.
- **Reflection is a VM intrinsic.** `std::meta::info` is an opaque handle into
  sema's entity tables; `std::meta` functions are intrinsics that call back
  through `SemaBridge` (lookup, instantiation, `define_aggregate`, splicing).
  Reflection values are first-class in the memory model from day one.
- **Memoization.** Calls to functions with no side effects and small, hashable
  arguments are memoized within a TU, bounded by a configurable cache size.
  This is what turns the typical "compute a table in a constexpr function
  called from 10,000 instantiations" pattern from quadratic to linear.
- **No global state.** The VM reads the AST and talks to sema only via the
  bridge, so multiple evaluators can eventually run on independent constant
  expressions in parallel.

## Codegen

Codegen is deliberately conventional. It mirrors clang's lowering (so that
`clang -S -emit-llvm` can be used as a reference in differential IR tests) and
targets `aarch64-unknown-linux-gnu` first (the development container) with
`x86_64-unknown-linux-gnu` second. ABI lowering of by-value aggregates is done
by `kiln_abi`, because LLVM does not do it for us.

## Driver

`kiln` accepts the clang subset of flags that build systems and lit use:
`-std=`, `-I`, `-isystem`, `-D`, `-U`, `-E`, `-fsyntax-only`, `-S`,
`-emit-llvm`, `-c`, `-o`, `-O`, `-g`, `-W...`, `-f...`, `-stdlib=libc++`,
`-nostdinc++`, `-x c++`, `-MMD`/`-MF`, `--target=`. Anything else is
diagnosed. For `-c` and linking it invokes `llc`/`clang` from the configured
LLVM install.

## Source tree

```
Cargo.toml            workspace
crates/<crate>/       one crate per pipeline stage (see above)
spec/                 one contract per crate/module; standard section refs
tests/conformance/    behavioral tests by standard section (see VALIDATION.md)
tests/differential/   inputs compared against clang
bench/                constexpr-heavy corpus + recorded baselines
docs/                 this file, ROADMAP, VALIDATION, AGENTS, status reports
scripts/              provisioning and helper scripts
```
