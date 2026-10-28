# spec: kiln_oven — the constant evaluator

Standard: [expr.const] (the whole of it: core constant expressions, the list
of disallowed operations, constant-initialization, manifestly
constant-evaluated contexts, `std::is_constant_evaluated`,
`if consteval`), [basic.life], [basic.lval] (access rules), [class.union]
(active member), [expr.new]/[expr.delete] (transient allocation),
[dcl.constexpr], [dcl.constinit], [expr.prim.req], [meta.reflection] (P2996
and successors, M6), [intro.object], [basic.align], [conv.*]. This is a
spec is **normative for M2** (the C-like subset, implemented by WP-051):
every rule below not marked *(design, M3+)* describes what `kiln_oven` does
and is tested. Sections marked *(design)* fix the data model later
milestones build on. The reference implementation for diagnostics is
**clang 19** (`clang++-19 -std=c++2c --target=x86_64-unknown-linux-gnu`,
spec/diag.md); bullet numbers of [expr.const]/5 are those of N4950.

## Goals, in priority order
1. Conforming: every operation [expr.const] forbids is detected and diagnosed
   with a note chain to the originating expression, in the same cases clang
   diagnoses it (differential tests), including all undefined behavior that is
   reachable in a constant evaluation.
2. Lean: peak memory of an evaluation is proportional to the live abstract
   objects, not to the number of operations or the size of the AST.
3. Fast: a small constant expression (a `sizeof` fold, a `static_assert` of
   a trait) costs no allocation; a large one (table generation, string
   building, reflection loops) runs at interpreter speed with no per-value
   heap allocation.
4. Observable: every evaluation reports `steps`, `bytes`, `peak_bytes`,
   `calls`, `allocs`, and wall time; totals per TU are available from the
   driver (see "Reporting protocol").

## Architecture
```
typed AST (kiln_ast) ──compile──▶ KBC function (per FunctionDecl, cached for the TU)
                                     │
   SemaBridge (trait, implemented by kiln_sema): bodies on demand, layout, ConstStore
                                     │
                                  VM: frames + 128-bit registers + abstract memory ──▶ ConstValueId (frozen result)
```
Source map (`crates/kiln_oven/src`): `kbc.rs` instruction set, `compile.rs`
AST→KBC, `vm.rs` interpreter, `mem.rs` abstract memory, `arith.rs` checked
arithmetic, `softfloat.rs` binary16/bfloat16/x87/binary128, `value.rs` freeze/thaw,
`failure.rs` `EvalFailure`, `notes.rs`
clang-worded notes, `memo.rs` cache, `stats.rs`/`report.rs` metrics, `ty.rs`
the evaluator's flattened type table.

### Bytecode (KBC) — normative for M2
- A `Vec<u32>` stream per TU. Word 0 of an instruction is `opcode | aux << 8`
  (`aux`: a 24-bit immediate — width, signedness, comparison kind, access
  kind); the operand words follow, a fixed count per opcode, so decoding is
  one `match` with no table lookup per operand and no hashing.
- Register machine: each frame is a window of 128-bit registers in one
  growable `Vec<u128>`; every scalar fits one register (integers canonical:
  sign- or zero-extended from their width; floats as IEEE bit patterns,
  x87 in the low 80 bits; pointers as an encoded `Ptr`). Locals whose
  address is never taken and that are not aggregates live in registers;
  the others live in abstract memory.
- Typed, checked arithmetic per width (1..128 bits, `_BitInt`-ready): signed
  overflow, division by zero, `INT_MIN / -1`, out-of-range shifts, negative
  shifts and float results that are NaN fault; unsigned arithmetic wraps.
  binary32/binary64 use the host's IEEE arithmetic (round to nearest even);
  x87 `long double` and `__float128` use the bit-exact soft-float in
  `softfloat.rs`.
- Compilation is lazy, per function, on first call. A function that cannot
  be called in a constant expression (not constexpr, or no body) compiles
  to a one-instruction stub that traps with clang's note on call, so sema
  need not pre-check. A construct the compiler does not support compiles to
  a trap reading `kiln: constant evaluation of <Kind> is not supported yet`:
  an evaluation fails with a note, never a panic.
- Top-level expressions compile to cached thunks keyed by `ExprId`;
  re-evaluating an expression compiles nothing.

### Abstract memory — normative for M2
- **Pointers** are one `u64`: offset (32 bits) | past-the-end (1) | slot
  (22) | generation (9). Slot `FN_SLOT` encodes function pointers; slots
  with bit 21 (`STATIC_BIT`) set are in the static space. Null is 0.
- **Allocations** (`Alloc`): base and size in their space, `OTy` type,
  flags (`CONST`, `EXTERNAL` — a non-usable variable stood in for,
  `HAS_UNION`, `RESOLVING` — a global whose initializer is running), the
  `Origin` (local `Var`, `Temp`, `Global`, `External`, `Str(literal, expr)`,
  thawed `Const`) for notes, and the frame serial that created it.
- **Spaces**: the *frame* space is a bump arena of all frame objects of the
  current evaluation; the *static* space holds globals and thawed constants
  and persists for the TU. Each space keeps the bytes, a per-byte
  initialized bitmap and a per-8-byte-granule "holds a pointer" bit; unions
  keep their active member in a side map keyed by (allocation, offset).
- **Lifetime**: leaving a scope releases its objects (`release(mark)`); a
  released slot is reused first-in first-out with its generation bumped, so
  a dangling pointer is detected (generation mismatch) and still names its
  origin for as long as possible. Slots whose generation would wrap are
  retired.
- **Reset**: at the end of every top-level evaluation the frame space is
  `reset()` (top to 0, slots truncated); its buffers are kept, so steady
  state allocates nothing (`Oven::footprint` is flat; tested over 10k
  evaluations).
- **Checks on every access**, in this order (the first failing one is the
  note): null; function pointer; dead (scope ended in a running frame:
  "read of object outside its lifetime", else "read of variable whose
  lifetime has ended" + declared here); stale generation; past-the-end;
  out of bounds; a non-usable variable ([expr.const]/5.9); for a write or
  the read half of `++`/`--`/compound assignment: modifying an object whose
  lifetime began outside the evaluation ([expr.const]/5.16) before a string
  literal (clang: "whose value is not known") before a const object; the
  inactive union member path ([expr.const]/5.10); then, for reads,
  uninitialized bytes ([expr.const]/5.11) and representation compatibility
  (pointer bytes read as a pointer only).
- **Freeze/thaw**: a successful result is frozen into the AST's
  `ConstStore` (scalars inline; aggregates as a byte image with pointer
  relocations, padding and active union members; pointers as
  `(DeclId | StringLitId | ConstValueId, offset)`). Reading a `constexpr`
  variable thaws its value into the static space once (copy-on-read,
  cached by `ConstValueId`), marked `CONST`.
- **String literals**: each evaluated `StringLiteral` expression is its own
  object ([lex.string]/9: distinct evaluations need not share), so
  `"a" == "a"` is unspecified and diagnosed as clang does. Thawed literal
  pointers are keyed by contents (see Open questions).

### Diagnostics — normative for M2
- `EvalFailure { kind, notes }`: the primary note, then "in call to
  'f(args)'" for each active frame innermost first (clang's backtrace:
  arguments rendered as clang prints them; more than 10 frames elide the
  middle with clang's "(skipping N calls in backtrace; ...)" line), then
  any "declared here"/"temporary created here" notes. Wording is clang
  19's, byte for byte.
- Note locations follow clang: an lvalue-to-rvalue read is reported at the
  begin of the read expression (leftmost token); a float-to-integer
  overflow at the begin of the cast operand; `p->m` with `p` null at the
  member ("cannot access field of null pointer"), `(*p).m` at the begin
  ("read of dereferenced null pointer"); a compound assignment to an
  inactive union member at the operator; a step-limit note at the begin of
  the statement that would exceed it.
- `try_fold` and `is_constant_initialized` never render notes (`silent`).
- **Deviations from clang 19** (each documented on its test): an enum
  conversion out of the range of the enumeration's values
  ([expr.static.cast]/10) is diagnosed in every constant evaluation; clang 19
  only diagnoses it in a constexpr variable initializer.

### Evaluation entry points — normative for M2
- `Oven::evaluate(bridge, e, EvalContext) -> Result<ConstValueId,
  EvalFailure>`: a manifestly constant-evaluated context; recorded in the
  stats and the report. `EvalContext::init(var, loc)` marks a variable's
  initializer (its object is under construction, `RESOLVING`).
- `Oven::try_fold(bridge, e) -> Option<ConstValueId>`: silent, not
  recorded.
- `Oven::is_constant_initialized(bridge, var) -> bool`
  ([basic.start.static]/2): silent, not recorded; uses the variable's
  cached evaluation state when known.
- A `const` integral or enumeration variable is usable in constant
  expressions ([expr.const]/4) when its initializer folds; its value is
  evaluated on demand when first read (nested initializations are capped
  at 512 deep and then read as unknown).

### Limits and metrics — normative for M2
- `Budget { steps: 1048576, depth: 512, bytes: 256 MiB, memo_entries:
  4096 }` (defaults; driver flags `-fconstexpr-steps=`,
  `-fconstexpr-depth=`, `-fconstexpr-memory=` (kiln), and a memo size).
- **Steps** are exactly clang's: one per statement executed (a compound
  statement, a declaration statement, each loop-body execution, ...;
  conditions and loop increments are not statements). The smallest
  `-fconstexpr-steps=` clang accepts equals kiln's recorded `steps` on the
  differential set (tests/limits.rs, tests/bench.rs). An evaluation whose
  step count would exceed the budget fails with "constexpr evaluation hit
  maximum step limit; possible infinite loop?".
- **Depth** counts active calls as clang's `-fconstexpr-depth` does ("constexpr
  evaluation exceeded maximum depth of N calls").
- **Bytes** bounds live frame-space bytes ("constexpr evaluation exceeded
  the memory limit of N bytes", at the object's declaration).
- `EvalStats` holds one `EvalRecord` per `evaluate` call and TU totals.
  `Oven::footprint()` is the evaluator's own heap (code, memory, caches).

### Reporting protocol
`-fconstexpr-report=<path>` makes the driver write one report per
translation unit to `<path>` (created or truncated; written even when
compilation fails, covering the evaluations that ran). It is the interface
of `oven:` budgets in `cargo xtask test` and of the constexpr counters in
`cargo xtask bench` (`spec/harness.md`, `spec/bench.md`), and the consumer
is `xtask/src/util/oven_report.rs`.

The format is NDJSON (UTF-8, one JSON object per `\n`-terminated line), one
line per **top-level evaluation** in the order the evaluations finished,
then exactly one `total` line, which is the last line:
```
{"loc":"a.cpp:12:1","kind":"static_assert","steps":116,"bytes":40,"peak_bytes":32,"calls":3,"allocs":0,"wall_us":4}
{"total":{"steps":116,"bytes":40,"peak_bytes":32,"calls":3,"allocs":0,"wall_us":4,"evaluations":1}}
```
A top-level evaluation is one call of an evaluation entry point that the
language requires (a manifestly constant-evaluated context): it includes
everything it calls, so nested calls are never separate lines. `try_fold`
(opportunistic folding) is not reported: what it folds is an implementation
choice and must not move a budget.

| Key | Meaning |
|---|---|
| `loc` | `"file:line:col"` of the evaluated construct (1-based line and column; the file as the driver names it in diagnostics, so it may itself contain `:`), or `null` when there is none (a compiler-synthesized evaluation). For a template instantiation, the point of the construct in the template, not the point of instantiation. |
| `kind` | `static_assert`; `init` (the initializer of a `constexpr`/`constinit` variable or a constant initialization); `template_arg` (a template argument, including default arguments); `consteval` (an immediate invocation); `if_constexpr` (the condition); `other` (array bounds, enumerator values, `case` labels, `alignas`, `noexcept` and `explicit` specifiers, bit-field widths, splices, ...). |
| `steps` | Statements executed, counted exactly as clang counts `-fconstexpr-steps` (see "Limits and metrics"), including the recorded steps of memoized calls (as for the budget, so the count does not depend on cache state). |
| `bytes` | Abstract-memory bytes allocated over the evaluation (cumulative, each object rounded up to the 8-byte granule: frames, temporaries and dynamic allocations; a freed byte stays counted). |
| `peak_bytes` | The maximum number of abstract-memory bytes live at once. |
| `calls` | Function calls, memoized hits included. |
| `allocs` | Dynamic allocations (`new` expressions and `std::allocator<T>::allocate`). |
| `wall_us` | Wall-clock microseconds, rounded down. |

The `total` object has the same counters summed over the evaluations, except
`peak_bytes`, which is the **maximum** over them (evaluations do not overlap,
so the TU's peak is the largest single peak), plus `evaluations`, the number
of evaluation lines. All counters are integers in `[0, 2^53]`.

Every counter except `wall_us` is **deterministic**: the same compiler on the
same input reports the same values, on any host and under any load, which is
what lets budgets and the bench compare them exactly. A change in kiln that
moves them is a measured change in the evaluator, not noise.

Consumers must reject a report that is not this format: a line that is not
an object with the keys above (each of the right type), a `kind` outside the
list, a missing or repeated `total`, text after it, or a `total` that does not
match the evaluations (sums, maximum for `peak_bytes`, count for
`evaluations`; `wall_us` is not cross-checked, since rounding makes its sum
inexact). They ignore unknown keys (so the format can grow) and blank lines.

### Memoization — normative for M2
- A function is memoized when its parameters (at most 4) are integral,
  `bool` or floating and its result is such a scalar (not a pointer, not
  void, not an aggregate). Such a call cannot observe or modify mutable
  state: it receives no pointer, may not modify an object from outside the
  evaluation ([expr.const]/5.16), and reads of globals are either of
  immutable usable constants or fail the same way every time. (M3 widens
  this when pointers to transient allocations can be passed.)
- Calls are cached in a bounded LRU (`Budget::memo_entries`, 0 disables;
  table allocated once, least recently used evicted) keyed by `(function,
  argument bits)`. Only successful calls are cached.
- A hit charges everything the call cost when it ran: steps, calls,
  cumulative bytes, peak bytes above the live bytes at the call, and depth.
  Every counter and every budget decision is therefore identical with the
  cache on or off (tested).

### Performance — normative targets, recorded baselines
No heap allocation per evaluation once warm (no boxing of values, no hash
lookup per instruction; the frame arena is reset, not freed). Benchmarks:
`cargo test --release -p kiln_oven --test bench -- --ignored bench
--nocapture`. Baselines (aarch64 container, release, memo off, median of
repeated evaluations of the compiled expression; clang is
`clang++-19 -fsyntax-only` minus an empty TU, which includes its parse):

| bench | steps (= clang) | peak bytes | kiln | clang 19 |
|---|---|---|---|---|
| `fib(25)` (recursive) | 485570 | 0 | 7.4 ms | ~350 ms |
| `fib(25)`, memo on | 485570 | 0 | 0.17 µs (warm) / 55 µs (first) | — |
| sieve of 10000 | 49442 | 10008 | 1.1 ms | ~38 ms |
| heap sort of 1000 structs | 86468 | 16032 | 3.6 ms | ~69 ms |

### Design for M3+ *(design)*
- Transient allocation (`new`/`delete`, `std::allocator`): a third space
  with the same slot/generation scheme, `allocs` counted, leaks at the end
  of the evaluation diagnosed ([expr.const]/5.20).
- Classes: constructors/destructors as KBC functions, `BeginLife`/`EndLife`
  for subobjects, dynamic type in the allocation for virtual calls
  (`CallIndirect` through `SemaBridge::vtable`), `dynamic_cast`/`typeid`.
- Memoization widens to pure functions taking pointers to immutable
  objects; results may be small frozen aggregates.
- `std::bit_cast` uses the representation-compatibility shadow.

### Reflection runtime (M6)
- `std::meta::info` is a `ReflectionId(u32)` into sema's entity tables;
  it is a scalar in memory (8 bytes) with shadow kind `Reflection`.
- All `std::meta` functions are intrinsics dispatched by name from the
  `<meta>` header's declarations (`consteval` functions whose definitions are
  kiln-internal); they call `SemaBridge` methods. `define_aggregate` and
  friends mutate sema state and are therefore only allowed in the
  evaluation contexts the standard permits; the bridge enforces that.

## Interface — amended 2026-10-28 per spec/ast.md Q2
The Oven holds **no** borrow of the AST across calls: sema mutates the AST
(instantiation on demand, `define_aggregate`, interning constants) while
evaluations run. The Oven lives for the whole TU (its KBC cache, thaw cache
and memo cache persist across evaluations) and reaches the AST through the
bridge; it stores only ids, which are stable by spec/ast.md §2.4.
```rust
pub trait SemaBridge {
    fn ast(&self) -> &Ast;                                        // re-borrowed per use, never held
    fn ensure_body(&mut self, f: DeclId) -> Option<StmtId>;       // instantiates if needed
    fn ensure_layout(&mut self, record: DeclId) -> &RecordLayout; // via kiln_abi
    fn vtable(&mut self, record: DeclId) -> &VTableLayout;        // M3
    fn consts(&mut self) -> &mut ConstStore;                      // freeze results
    fn diag(&mut self, d: Diag);
    // reflection ops added in M6
}
impl Oven {
    pub fn new(budget: Budget) -> Self;
    pub fn budget(&self) -> Budget;
    pub fn evaluate(&mut self, bridge: &mut dyn SemaBridge, e: ExprId, ctx: EvalContext) -> Result<ConstValueId, EvalFailure>;
    pub fn try_fold(&mut self, bridge: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId>;
    pub fn is_constant_initialized(&mut self, bridge: &mut dyn SemaBridge, var: DeclId) -> bool;
    pub fn stats(&self) -> &EvalStats;
    pub fn footprint(&self) -> usize;
    pub fn disassemble(&self) -> String;                          // debugging aid
}
pub mod report { pub fn render(..) -> String; pub fn write(..) -> io::Result<()>; }
```

## Status (WP-051) and what remains
Implemented (M2): integers of every width incl. `__int128`, `bool`,
character types, binary32/64, x87 `long double`, binary128, enums;
pointers to objects, arrays and functions; arrays, structs, unions,
bit-fields; all C control flow (if, loops, switch incl. fallthrough,
break/continue, return) with goto diagnosed as clang does; calls and
recursion; constexpr variables and functions; `static_assert`; compound
assignment, comma, conditional, casts, `sizeof`/`alignof`; string literals,
`nullptr`, local aggregates and their initializers; `& * [] . ->` and
lvalue-to-rvalue; every [expr.const]/5 bullet reachable in M2 (5.2, 5.3,
5.4, 5.7, 5.8, 5.9, 5.10, 5.11, 5.14, 5.15, 5.16, 5.24).

Remaining for M2 consumers (each traps with a `kiln:` note today):
- Labels: `LabelStmt`/`AttributedStmt` payloads are not defined by
  kiln_ast (spec/ast.md §6), so labelled statements and `goto` are not
  exercised by tests (no AST builder exists).
- `MaterializeTemporaryExpr` lowering is implemented but untested: there is
  no builder in kiln_ast; to be tested when the parser (WP-048) emits it.
- Volatile objects: reads are diagnosed ([expr.const]/5.9) only where the
  front end puts the volatile qualifier on the read expression's type.

Open questions:
- Thawed string-literal identity: `PtrBase::StringLit` in kiln_ast's
  `ConstStore` carries no expression identity, so two constexpr pointers
  frozen from equal-content literals thaw into the same object, and
  `p + 1 != q` for `constexpr const char *p = "ab", *q = "ab";` is accepted
  where clang rejects it as unspecified. Fixing it needs an additive change
  to `PtrBase` (literal expression id) in kiln_ast.

## Tests
`crates/kiln_oven/tests/`, each test naming its section:
- `expr_const.rs`: one test per [expr.const]/5 bullet reachable in M2 with
  clang 19's exact notes, plus positives bounding each bullet.
- `ub.rs`: the core-UB catalogue ([expr.const]/5.8) with exact notes.
- `ops.rs`: positives for every M2 operation.
- `float.rs`: binary32/64 bit-compared with Rust's IEEE results; x87
  goldens from clang's IR.
- `limits.rs`: step and depth counts equal to clang's, limit notes, the
  memory budget, memo transparency and purity, the report golden, and the
  memory-reset test (flat footprint over 10k evaluations).
- `bench.rs` (ignored): the benchmarks above.
- `common/`: a small recursive-descent C++ subset parser building kiln_ast
  directly (test-only; replaced by the real front end as it lands).
Every negative golden was produced by kiln and compared to clang 19's
output for the same source; positives are all accepted by clang 19.
Planned *(design)*: `tests/conformance/expr.const/` through the driver,
`tests/differential/constexpr/` programs built and run with clang,
`xtask fuzz-constexpr`, and `bench/oven/` with vs-clang ratios.
