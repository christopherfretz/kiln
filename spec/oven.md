# spec: kiln_oven — the constant evaluator

Standard: [expr.const] (the whole of it: core constant expressions, the list
of disallowed operations, constant-initialization, manifestly
constant-evaluated contexts, `std::is_constant_evaluated`,
`if consteval`), [basic.life], [basic.lval] (access rules), [class.union]
(active member), [expr.new]/[expr.delete] (transient allocation),
[dcl.constexpr], [dcl.constinit], [expr.prim.req], [meta.reflection] (P2996
and successors, M6), [intro.object], [basic.align], [conv.*]. This is a
spec is **normative for M2** (the C-like subset, implemented by WP-051 and
wired into sema and the driver by WP-054):
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
- An "in call to" note is located at the begin of the call's callee
  expression (clang's call-site location), not at the `CallExpr`'s own
  location (its parenthesis). Arguments bound to reference parameters print
  as the referred-to object (`bad(a[1], 0)`), as clang prints them.
- `-fconstexpr-backtrace-limit=N` (`Oven::set_backtrace_limit`) sets the
  frame limit; 0 shows every frame.
- `try_fold`, `is_constant_initialized` and `fold_constant_init` never
  render notes or warnings (`silent`).
- An out-of-range conversion to an unscoped enumeration without a fixed
  underlying type ([expr.static.cast]/10) is accepted with the value, and
  reported only where clang 19 reports it ("Integration").

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
- `Oven::fold_constant_init(bridge, e, var) -> Option<ConstValueId>`
  (WP-063): folds `var`'s initializer `e` as a constant initializer
  ([basic.start.static]/2): manifestly constant-evaluated, but silent and
  not recorded, like `try_fold`. Codegen uses it for the initializers it
  emits as constants (spec/codegen.md).

### Manifestly constant-evaluated context — normative (WP-063)
`__builtin_is_constant_evaluated()` ([expr.const]/20, [meta.const.eval])
asks whether the evaluation is manifestly constant-evaluated.
- `RunState::const_context` holds the answer for one top-level
  evaluation: true for `evaluate`, `is_constant_initialized` and
  `fold_constant_init`, false for `try_fold` (a trial fold is not
  manifestly constant-evaluated). The nested initialization of a variable
  read during an evaluation (`resolve_var`) is a constant initializer:
  it runs with `const_context` true and restores the outer value after.
  A nested evaluation context (R-OVEN-1, a bridge callback's `evaluate`
  or `try_fold`) sets the flag by its own entry point and restores the
  enclosing context's value at its end, with the rest of the run state.
- A call to the builtin (a `CallExpr` whose callee is a
  `BuiltinFnToFnPtr` cast of a builtin's name) compiles to
  `IsConstEval [dst]`, which writes the flag as a `bool`. Its `aux` is 1
  when the enclosing function is `std::is_constant_evaluated` (a function
  of that name in `std`, through inline namespaces).
- **Warning** (`-Wconstant-evaluated`, clang's
  `warn_is_constant_evaluated_always_true_constexpr`): when the answer is
  true in a non-silent evaluation, and the builtin is called directly by
  the evaluated expression (frame depth 1), the warning names
  `__builtin_is_constant_evaluated` at the begin of the call's callee;
  when it is called from `std::is_constant_evaluated` at depth 2, it
  names `std::is_constant_evaluated` at that call's site. Deeper calls do
  not warn. The warning goes through `SemaBridge::diag`, so it obeys
  `-Wno-`/`-Werror=` and system-header suppression.
- **Memoization:** a function whose body uses `IsConstEval` is never
  memoized, and every memo key carries the context (bit 31 of the
  function word), so a call cached in one context is never replayed in
  the other.

### Limits and metrics — normative for M2
- `Budget { steps: 1048576, depth: 512, bytes: 256 MiB, memo_entries:
  4096 }` (defaults; driver flags `-fconstexpr-steps=`,
  `-fconstexpr-depth=`, `-fconstexpr-memory=` (kiln); the memo size has no
  flag yet).
- **Steps** are exactly clang's: one per statement executed (a compound
  statement, a declaration statement, each loop-body execution, ...;
  conditions and loop increments are not statements). The smallest
  `-fconstexpr-steps=` clang accepts equals kiln's recorded `steps` on the
  differential set (tests/limits.rs, tests/bench.rs). An evaluation whose
  step count would exceed the budget fails with "constexpr evaluation hit
  maximum step limit; possible infinite loop?".
- **Depth** counts active calls as clang's `-fconstexpr-depth` does ("constexpr
  evaluation exceeded maximum depth of N calls"); a nested evaluation counts
  from the depth of the call that started it, plus one (see "Nested
  evaluation").
- **Bytes** bounds the abstract-memory address space held,
  `frame.top + heap.top` ("constexpr evaluation exceeded the memory limit
  of N bytes", at the object's declaration); nested evaluations share it.
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
{"loc":"a.cpp:12:1","kind":"static_assert","steps":116,"bytes":40,"peak_bytes":32,"calls":3,"allocs":0,"peak_arena":32,"frozen_bytes":16,"wall_us":4}
{"total":{"steps":116,"bytes":40,"peak_bytes":32,"calls":3,"allocs":0,"peak_arena":32,"frozen_bytes":16,"wall_us":4,"evaluations":1}}
```
A top-level evaluation is one call of an evaluation entry point that the
language requires (a manifestly constant-evaluated context): it includes
everything it calls, so nested calls are never separate lines. A **nested
evaluation** (one that a bridge callback starts while another is running,
see "Nested evaluation") is its own line, written when it finishes (so
before the line of the evaluation that started it), and the line of the
enclosing evaluation counts only the enclosing evaluation's own work: every
counter of a line is exclusive of the nested evaluations it contains, so
the `total` sums never count work twice. `try_fold`
(opportunistic folding) is not reported: what it folds is an implementation
choice and must not move a budget.

| Key | Meaning |
|---|---|
| `loc` | `"file:line:col"` of the evaluated construct (1-based line and column; the file as the driver names it in diagnostics, so it may itself contain `:`), or `null` when there is none (a compiler-synthesized evaluation). For a template instantiation, the point of the construct in the template, not the point of instantiation. |
| `kind` | `static_assert`; `init` (the initializer of a `constexpr`/`constinit` variable or a constant initialization); `template_arg` (a template argument, including default arguments); `consteval` (an immediate invocation); `if_constexpr` (the condition); `constraint` (a constraint satisfaction check, M3); `other` (array bounds, enumerator values, `case` labels, `alignas`, `noexcept` and `explicit` specifiers, bit-field widths, splices, ...). |
| `steps` | Statements executed, counted exactly as clang counts `-fconstexpr-steps` (see "Limits and metrics"), including the recorded steps of memoized calls (as for the budget, so the count does not depend on cache state). |
| `bytes` | Abstract-memory bytes allocated over the evaluation (cumulative, each object rounded up to the 8-byte granule: frames, temporaries and dynamic allocations; a freed byte stays counted). |
| `peak_bytes` | The maximum number of abstract-memory bytes live at once (`frame.top + heap_live`, see "Transient allocation"). |
| `calls` | Function calls, memoized hits included. |
| `allocs` | Dynamic allocations (`new` expressions and `std::allocator<T>::allocate`). |
| `peak_arena` | The maximum abstract-memory address space held at once (`frame.top + heap.top`), fragmentation included; see "Transient allocation". `peak_arena >= peak_bytes`. |
| `frozen_bytes` | Bytes this evaluation added to the `ConstStore`: only newly interned bytes, so a result equal to one already frozen adds 0 (spec/ast.md §9). |
| `wall_us` | Wall-clock microseconds, rounded down. |

The `total` object has the same counters summed over the evaluations, except
`peak_bytes` and `peak_arena`, which are the **maximum** over them (top-level
evaluations do not overlap, so the TU's peak is the largest single peak;
a nested evaluation's peaks are exclusive, see "Nested evaluation"), plus
`evaluations`, the number
of evaluation lines. All counters are integers in `[0, 2^53]`.

Every counter except `wall_us` is **deterministic**: the same compiler on the
same input reports the same values, on any host and under any load, which is
what lets budgets and the bench compare them exactly. A change in kiln that
moves them is a measured change in the evaluator, not noise.

Consumers must reject a report that is not this format: a line that is not
an object with the keys above (each of the right type), a `kind` outside the
list, a missing or repeated `total`, text after it, or a `total` that does not
match the evaluations (sums, maximum for `peak_bytes` and `peak_arena`, count for
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
  argument bits)`, with the manifestly-constant-evaluated flag in bit 31
  of the function word (WP-063). Only successful calls are cached, and not a call that
  converted a value out of an enumeration's range (its diagnostic must
  recur on every evaluation that checks it), that started a nested
  evaluation (its side effects on sema and its separate report line must
  recur), or that allocated or freed heap memory.
- A hit charges everything the call cost when it ran: steps, calls,
  cumulative bytes, peak bytes above the live bytes at the call, arena
  above the arena at the call (checked against the memory limit), and
  depth.
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

Whole-driver baselines (`bench/baselines/oven.json`; release kiln and
clang 19, `-fsyntax-only`, median of 5 runs, aarch64, 24 CPUs, load
average 8-9 during the run):

| entry | kiln | clang 19 | kiln MB | clang MB |
|---|---|---|---|---|
| `oven/fib` | 1.9 ms | 611 ms | 3.4 | 83 |
| `oven/sieve-nohdr` | 8.2 ms | 267 ms | 3.2 | 89 |
| `oven/sort-nohdr` | 20.2 ms | 412 ms | 3.4 | 91 |
| `oven/table-nohdr` | 42.4 ms | 1369 ms | 4.2 | 90 |

### Design for M3+ *(design)*
- Transient allocation: see "Transient allocation" below (WP-060).
- **Nested evaluation (R-OVEN-1, spec/sema-templates.md §4.11):**
  implemented (WP-061b); normative in "Integration" → "Nested evaluation".
- **Hash-consed frozen values (R-OVEN-2):** implemented; `ConstStore`
  interns every frozen value by content (spec/ast.md §9), so class-type
  template arguments compare by `ConstValueId`, and `frozen_bytes` counts
  only newly interned bytes.
- Classes: constructors/destructors as KBC functions, `BeginLife`/`EndLife`
  for subobjects, dynamic type in the allocation for virtual calls
  (`CallIndirect` through `SemaBridge::vtable`), `dynamic_cast`/`typeid`.
- Memoization widens to pure functions taking pointers to immutable
  objects; results may be small frozen aggregates.
- `std::bit_cast` uses the representation-compatibility shadow.

### Transient allocation — normative for M3 (WP-060 design; WP-061a/b implement)

*Status:* implemented. The heap space, the allocator and the counters are
WP-061b part 1 (`crates/kiln_oven/src/mem.rs`); the `New`/`Delete`
operations, their checks, the leak check and the churn benchmarks are
WP-061b part 2 (`crates/kiln_oven/src/compile/new_delete.rs`, `vm.rs`,
`value.rs`). Classes with a constructor or destructor landed with WP-076
("Classes"); `std::allocator` and `__builtin_operator_new` (the templates
wave) are not yet constant; see "Status and what remains".

The operator's requirement: memory that a constant evaluation allocates and
frees **must be reclaimed while the evaluation is still running**, not at its
end. A loop that builds and destroys a 100-node list 1000 times must peak at
100 nodes, not 100 000. `peak_bytes` is the contract and the gate; the
benchmarks below fix the targets before the implementation exists.

**Heap space.** A third space, `heap`, with the same bytes/init/pointer
shadow as `frame` and `static`. Pointer encoding is unchanged except that
slot bit 20 (`HEAP_BIT`) marks a heap slot: frame slots are `< 2^20`, heap
slots `2^20..2^21`, static slots have bit 21 set as now. Heap slots have
their own `Alloc` table and free-slot queue with the frame space's
generation scheme: `delete` marks the slot dead and keeps its `Alloc` (so a
dangling pointer still names its origin in notes) until the slot is reused
with a bumped generation; a slot whose generation would wrap is retired.
`Origin::Heap(ExprId, u32)` names the `new` expression (or the
`std::allocator<T>::allocate` call) and the allocation's serial number in
its evaluation, for notes (clang prints a heap pointer as
`&{*new T#n}`, `n` counting from 0 per evaluation).

**Byte reuse.** Heap bytes are not a bump arena. Sizes are granule-rounded
(8 bytes), then served by size class:

| Request (rounded) | Classes | Block | Internal waste |
|---|---|---|---|
| 8 ..= 256 B | 32 exact classes, one per granule (8, 16, ..., 256) | the request | none |
| 257 B ..= 1 MiB | 24 half-step classes: for `2^p < n <= 2^(p+1)` (`p` = 8..=19), `3·2^(p-1)` if `n` fits, else `2^(p+1)` (384, 512, 768, 1024, ..., 786432, 1048576) | the class size | at most 1/3 of the block |
| > 1 MiB | none: a *large* block of exactly the request | the request | none |

A class request takes the last freed block of its class, else is carved
(like a large request) from a free large region, else grows the top. A
freed class block goes back on its class's list (it is never split or
merged, and only its own class reuses it) unless it ends at the top.
Large blocks are carved best-fit (the smallest fitting region, then the
lowest) from a sorted free list of large regions that are **coalesced**
with free neighbours on `delete`, and a freed block at the top of the heap
arena **lowers the top**, then again past any free large region that now
ends at the top (so a doubling buffer reclaims its address space). A free list is LIFO
(last freed, first reused). An allocation that finds no block grows the
arena top. The policy is observable only through the counters below and the
memory limit; an implementation may tune the class boundaries, but the two
gates (`peak_bytes`, `peak_arena`) in the churn benchmarks are the contract.

**Counters.** `bytes` and `allocs` are as before (cumulative). `peak_bytes`
is the maximum over the evaluation of `frame.top + heap_live`, where
`heap_live` is the sum of the granule-rounded sizes of the live heap
allocations. Two counters are added to the report and to the `oven:`
directive (`spec/harness.md`), both deterministic:

| Key | Meaning |
|---|---|
| `peak_arena` | The maximum over the evaluation of `frame.top + heap.top`: the abstract-memory address space held, fragmentation included. `peak_arena >= peak_bytes`; the ratio is the allocator's waste. |
| `frozen_bytes` | Bytes this evaluation added to the `ConstStore` (its frozen result, byte image plus relocations), so the cost of keeping results is visible apart from the cost of computing them. |

The `total` takes the maximum for `peak_arena` and the sum for
`frozen_bytes`. `-fconstexpr-memory=` bounds `frame.top + heap.top` (that is,
`peak_arena`), with clang's message.

**Reset.** At the end of a top-level evaluation every heap allocation is
either freed or diagnosed as a leak, so `reset()` truncates the heap slot
table and clears the free lists at no cost; buffers are kept and the
footprint stays flat (the 10k-evaluation test extends to a churning
evaluation). Each heap allocation records the evaluation context that made
it (`Memory::ctx`); a nested evaluation frees what it still holds when it
ends (`free_heap_of`), after its own leak check, so its blocks return to
the free lists for the enclosing evaluation.

**Operations (KBC).** `New [dst, ty, count, origin, k]` yields in `dst`
a pointer to a fresh allocation of `ty` (or, with aux `ARRAY`,
`ty[count]`), uninitialized unless aux `ZERO` (the initializer, if any, is
ordinary stores through the pointer; the lifetime of a
trivially-constructible object begins at allocation; classes with
constructors begin life in the constructor call, "Classes"). For an array,
`count` is the bound as written (before the `size_t` conversion; aux
`SIGNED` says whether it is signed) and `k` the number of explicit
initializers; the bound is checked in clang's order: negative, too large
for the address space (61 bits for a 64-bit `size_t`), beyond the step
limit (clang's `-fconstexpr-steps` allocation limit), smaller than `k`.
With aux `NOTHROW` (`new (std::nothrow)`) a failed check yields a null
pointer instead of a fault and the initializer is skipped. Elements past
the explicit initializers are filled by a runtime loop with the
`array_filler`, or by `ZERO` when the filler is zero-initialization.
`Delete [p, pointee]` (aux 1 for `delete[]`) frees the allocation; a null
`p` is a no-op.
`is_std_allocator_call` (`compile/new_delete.rs`) is the hook that will
recognize the calls below; it returns false until class templates exist,
and a call that reaches it traps with `kiln: std::allocator in constant
evaluation lands with the templates wave`. `std::allocator<T>::allocate(n)`
and `deallocate(p, n)` and the `__builtin_operator_new`/
`__builtin_operator_delete` calls libc++ makes inside them lower to the same
two ops with aux `ARRAY` and `ty = T` (clang's rule: the allocation is an
array of `n` `T`; `__builtin_operator_new` anywhere else is not a constant
expression). That recognition needs class templates and is part of the M3
Oven package; WP-061 covers `new`/`delete` expressions on the types the Oven
supports in M2 (scalars, arrays, aggregates without constructors).

*AST shapes the lowering consumes* (WP-061a; spec/ast.md §5.7).
- `CXXNewExpr` (`ty` = `T*`), `new_extra(e)`:
  - `allocated_ty` is `ty` for `New` (for an array new, the element
    type: `int[3]` for `new int[n][3]`, so `New`'s `ty` is the element
    type and `count` the first bound);
  - `array_size` (`ARRAY_FORM`) is the `count` operand, already
    converted to `size_t` (`IntegralCast`); a negative signed bound shows
    as a huge unsigned value, so the "negative array bound" note needs the
    operand under the cast;
  - `init` is, by `new_init_style(e)`: `None` with no initializer
    (default-initialization: the storage stays uninitialized, reads are
    "read of uninitialized object"); `Parens` with
    `ImplicitValueInitExpr` (`()`, zero) or the converted expression
    (`(x)`); `Braces` with an `InitListExpr` of type `T` (scalar `{x}`,
    an aggregate) or `T[k]` with `array_filler` for the elements beyond
    the explicit ones (array new; `k` is the number of explicit
    initializers, or the constant bound). Each is an ordinary store
    sequence through the new pointer, element by element for arrays;
  - `placement` holds the placement arguments. The only constant
    placement is a single `std::nothrow_t` argument (the non-throwing
    form). The allocation function must be a replaceable global one
    (clang's `isReplaceableGlobalAllocationFunction`, mirrored by
    `is_replaceable_global_alloc`); a user declaration that *replaces*
    one is still replaceable and the `new` is constant, because constant
    evaluation never calls it (corrected: the earlier text said a
    replaced `operator_new` is not constant; clang 19 accepts it). A
    non-replaceable one is not constant, with "call to placement
    'operator new'" (or "call to class-specific 'operator new'"). Any
    other placement form, including `std::align_val_t`, is "this
    placement new expression is not yet supported in constant
    expressions". P2747 (`constexpr` placement new, C++26) is not
    implemented by clang 19, so the reserved `new (p) T` is not constant
    either (corrected: the earlier text planned it); it comes with the
    oracle that implements it;
  - `PASS_ALIGN` and `operator_delete` are codegen facts and do not
    affect evaluation.
- `CXXDeleteExpr` (`ty` = `void`): `delete_operand(e)` is the pointer
  prvalue; `ARRAY_FORM` selects `Delete { array: true }` (also set for a
  `delete` of a pointer to an array, which clang treats as `delete[]`;
  `ARRAY_AS_WRITTEN` says what was written); `operator_delete(e)` must be
  a replaceable global deallocation function for the delete to be
  constant ([expr.const]); `delete_passes(e)` (size, alignment) is
  a codegen fact.

**Checks**, with clang 19's wording, each verified against the oracle
(WP-061b part 2; `tests/conformance/expr.new/constexpr-*.cpp`,
`tests/conformance/expr.delete/constexpr-*.cpp` and
`crates/kiln_oven/tests/new_delete.rs`). Pointers print as clang prints
them: `&x` for a variable, `&{*new T#n}` for allocation `n`, then
`[i]` / `.member` for subobjects.

| Fault | Note (location) | Extra note |
|---|---|---|
| Leak at the end of an evaluation ([expr.const]/5.20) | "allocation performed here was not deallocated" (at the lowest-numbered live `new`), plus " (along with N other memory leak(s))" | — |
| Result points into the heap | "pointer to heap-allocated object is not a constant expression" / "pointer to subobject of heap-allocated object ..." (at the variable for an initializer, as clang's `CheckConstantExpression`) | "heap allocation performed here" (the `new`) |
| Access after `delete` | "read of heap allocated object that has been deleted" / "assignment to ..." (the access kind as for every access note) | — (corrected: clang adds no allocation note) |
| `delete` of a non-heap pointer | "delete of pointer '&x' that does not point to a heap-allocated object" | "declared here" (the variable) / "temporary created here" |
| Double delete | "delete of pointer that has already been deleted" | — |
| `delete` of a subobject | "delete of pointer to subobject '&{*new S[2]#0}[1]' " (clang's trailing space) | — |
| `delete` past the end | "delete of pointer '&{*new int[3]#0}[3]' that does not point to complete object" | — |
| Wrong form | "non-array delete used to delete pointer to array object of type 'int[3]'" / "array delete used to delete pointer to non-array object of type 'int'" | "heap allocation performed here" |
| Array bound ([expr.new]/8) | "cannot allocate array; evaluated array bound N is negative" / "is too large" / "exceeds the limit (L); use '-fconstexpr-steps' to increase this limit" / "is too small to hold K explicitly initialized elements" (at the bound expression) | — |
| Non-replaceable allocation function | "call to placement 'operator new'" / "call to class-specific 'operator new'" (the `new`; `delete` likewise with 'operator delete') | — |
| Other placement forms | "this placement new expression is not yet supported in constant expressions" | — |
| Default-initialized scalar read | "read of uninitialized object is not allowed in a constant expression" | — |

`delete` of null (either form) is a no-op. A leak is reported as
`NotConstant`, like every other note here (clang reports it through a
CCEDiag; the observable result, an error on the evaluation, is the
same). The leak check runs at the end of every evaluation context,
outermost and nested.

**Churn benchmarks (written first, gated).** `bench/oven/churn-*.cpp` and
the same programs as kiln-specific tests under `tests/conformance/expr.new/`
with `oven:` budgets. Each uses only `new`/`delete` of scalars and
aggregates, so it runs before classes exist. The budgets are the design
targets, set from the live-object count, not from a measurement:

| Program | Pattern | Gate (per top-level evaluation) |
|---|---|---|
| `churn-list` | 1000 rounds of: build a 100-node singly linked list, sum it, destroy it | `allocs<=100000`, `peak_bytes<=4096`, `peak_arena<=8192` |
| `churn-vector` | 64 rounds of: grow an `int` buffer by doubling from 1 to 65536 elements (allocate new, copy, delete old), then free | `allocs<=1088`, `peak_bytes<=786432` (3×256 KiB: old+new during the last copy), `peak_arena<=1048576` |
| `churn-string` | build a 64 KiB `char` string by 4096 appends of 16 bytes with 1.5× growth, 16 rounds | `peak_bytes<=262144`, `peak_arena<=393216` |
| `churn-tree` | 16 rounds of: build a complete binary tree of 4095 nodes (24 bytes each), count it, destroy it in post-order | `allocs<=65520`, `peak_bytes<=131072`, `peak_arena<=196608` |
| `churn-mixed` | interleaved 8/24/40/200-byte allocations freed in a different order than allocated, 10 000 operations | `peak_arena<=2*peak_bytes` stated as an absolute: `peak_arena<=65536` with `peak_bytes<=32768` |

The bench entries take the default thresholds (`threshold-bytes` 2%), so
once recorded, any allocator change that raises `peak_bytes`, `peak_arena`
or `allocs` fails the bench. `steps` is gated at 0% as everywhere.

### Classes — Landed (WP-076)

*Status:* implemented in `crates/kiln_oven/src/compile/class.rs`
(construction, destruction, temporaries), `compile/mptr.rs` and
`memptr.rs` (member pointers), `compile/new_delete.rs` (class `new`/
`delete`), `vm.rs` (cleanups, downcasts) and `value.rs` (constant
destruction, static temporaries). Virtual dispatch, virtual bases,
`dynamic_cast` and `typeid` are WP-077's; builtins are WP-088's.

**Object model.** A class object is one allocation of its record type;
`ty.rs` lays a record out as its direct non-virtual base subobjects
(`Field::base`, in declaration order, before the members) and then its
fields, at the offsets of kiln_abi's record layout, so a subobject is a
byte offset plus a type, and a pointer to it is the allocation plus that
offset (no `SubobjectId` is materialized: the path is recovered from the
offset with `Types::has_subobject`/the most-derived-object walk when a
downcast or a derived member pointer needs it). Lifetime is per
allocation (the `live` bit and generation of `mem.rs`); a union's active
member is tracked per union subobject as in M2 ([class.union]/6: a
mem-initializer or an assignment to a member of non-class type activates
it, also through an anonymous union). `BeginLife`/`EndLife` per subobject
are not separate operations: a member's lifetime is its complete
object's, which is precise for constant evaluation because no constant
expression can end a subobject's lifetime independently except through a
union (tracked) or an explicit destructor call (not yet parsed, below).

**Construction.** `CXXConstructExpr` (complete, base and delegating; a
prvalue initializes its target object directly, [dcl.init.general]/17.6.1;
a trivial copy or move constructor copies the object representation,
[class.copy.ctor]/14), `CXXTemporaryObjectExpr`,
ctor-initializers in initialization order (bases, then members, with
`CXXDefaultInitExpr` for the rest), `CXXParenListInitExpr`,
`ArrayInitLoopExpr`/`ArrayInitIndexExpr` (implicit copies of array
members), default construction of arrays of class type (an element loop),
and the implicit special members obtained from `SemaBridge::ensure_body`.
A `const` complete object is writable while it is constructed and
destroyed: `MakeConst [p]` sets `CONST` once the constructor returns
([class.ctor.general]/5), `run_cleanup` clears it before the destructor
runs ([class.dtor]/5).

**Destruction (cleanups).** `PushCleanup [p, dtor, ty]` registers the
destruction of the object (or of each element of an array, aux 1: of a
heap array whose count is the allocation's) on the Oven's cleanup stack
(`Oven::cleanups`, entries `Cleanup { this, func, pc }`); each frame
records the stack height at entry (`Frame::clean`). `ScopeExit` and `Ret`
run the cleanups above their mark, last registered first, by entering
the destructor and re-executing themselves on its return, so destruction
is iterative and needs no native recursion. Locals are destroyed in
reverse declaration order at scope exit, `break`/`continue`/`return`
included; temporaries at the end of their full-expression
(`ExprWithCleanups`); members and bases after the destructor body, in
reverse construction order; array elements from the last. A class
without a user-provided destructor and with trivially destructible
members registers nothing.

**Temporaries.** `MaterializeTemporaryExpr` by storage duration:
full-expression temporaries are frame allocations released at the end of
the full-expression; temporaries extended by a local (automatic) are
allocated before the initializer runs and destroyed with the variable
(`ext_temps`); temporaries extended by a variable of static storage
duration whose initializer is being evaluated are allocated before the
initializer with `Origin::Let(t)` (`t` the `LifetimeExtendedTemporary`),
outlive the initializer's full-expression, and are frozen after the
result: a pointer to one freezes as `PtrBase::Temporary(t)` and its value
is written to `LetData.value` through the new
`SemaBridge::set_temporary_value(t, v)` (write-once). Reading one later
thaws it from `LetData.value` when it is usable in constant expressions
([expr.const]/4: const non-volatile type, extended by a variable usable
in constant expressions); otherwise it is an external object and a read
is clang's `read of temporary is not allowed in a constant expression
outside the expression that created the temporary` with `temporary
created here` (`MemFault::Temporary`). A reference variable counts as
potentially-constant ([expr.const]/3), so `const int &c = 6;` makes `c`
usable. `CXXBindTemporaryExpr` is transparent: a temporary's destruction
is registered where the prvalue is materialized.

**Constant destruction ([dcl.constexpr]/6, [expr.const]/7).** After a
constexpr variable of class (or class array) type is initialized and
frozen, `Oven::compile_destroy` builds a thunk that registers and runs its
destruction (`PushCleanup` + `Ret`); a fault there is
`FailKind::Destruction`, which sema reports as `constexpr variable 'x'
must have constant destruction` with the fault's note and `in call to
'x.~T()'` (`arr[1].~A()` for an element). Destructors may modify the
object (it is no longer const). Default-initialized constexpr class
variables are checked for const-default-constructibility and then
evaluated like any other initializer.

**Member pointers.** Values are interned per Oven as clang's
`APValue::MemberPointer` (member, `derived` flag, class path;
`memptr.rs`), so freeze/thaw maps 1:1 to `Scalar::MemberPtr`. Ops:
`MpCast [mp, class]` (castToDerived aux 0, castToBase aux 1, in place),
`MpAddr [dst, obj, mp]` (`obj.*mp`, checking derived-member containment;
`Fault::NullBase` for a base step through a null object), `MpActive
[p, mp]` (a union member reached through `.*` must be active), `MpFn
[dst, mp, trap]` (the callee of a pointer to member function; a virtual
one traps to WP-077). `Downcast [dst, p, off, ty]` is
`static_cast` from a base pointer or glvalue to a derived one
([expr.static.cast]/2, /11), checking that the base subobject is part of
an object of the derived type.

**Heap class objects.** `new T(args)`, `new T{...}`, `new T[n]` (with
explicit initializers and a default-constructed tail) construct in place;
`delete`/`delete[]` make the 061b checks first (`Delete` aux 2: checks
only), then run the destructors (array elements from the last), then
free. The leak check and the heap-result note are 061b's.

**Iterative walkers (Q16, R-WALK).** Left- and right-deep spines of
builtin binary operators, `&&`/`||` and comma are lowered with explicit
loops and worklists (`binary`, `logical`, `arith_spine`, `effect`, the
address-taken pre-scan); a 262144-term chain costs 34 MB end to end (was
168 MB), the rest being parse and sema.

**Counters and benchmarks.** No new counters: construction is `calls`
and `steps`, heap objects are `allocs`. Entries `bench/oven/class-churn`
(10000 heap objects per round with owning destructors, 8 rounds) and
`bench/oven/soa` (a 4096-row struct-of-arrays table, mirrorbind's
binding-table shape); gated copies `tests/conformance/class.dtor/
churn-objects.cpp` and `class.mem/soa-table.cpp` carry `oven:` budgets.

**Left for later (owners).**
- Virtual calls, virtual bases, virtual calls through member pointers,
  `dynamic_cast`, `typeid`, `delete` through a base with a virtual
  destructor: WP-077 (they trap with a `kiln:` note).
- `std::construct_at`/`destroy_at`/`std::allocator`: need function
  templates (WP-071) and the library; the `is_std_allocator_call` hook
  still returns false.
- Class assignment (`a = b` of class type) is rejected by sema before
  evaluation; explicit destructor calls (`p->~T()`) and
  `::operator new`/`::operator delete` are not parsed; inheriting
  constructors (`using B::B`) are not in sema: front-end gaps, not Oven
  work.
- Constant destruction of a non-trivially-destructible temporary bound to
  a constexpr reference is not checked (clang 19 rejects it as "not yet
  supported"; kiln accepts it).
- Codegen of `PtrBase::Temporary` (a constexpr reference to a static
  temporary used at run time) is WP-079's ("pointers to lifetime-extended
  temporaries are not supported yet").
- Spec gap: `SubobjectId` (spec/sema-class.md §3.5) is not used by the
  Oven; offsets suffice until virtual bases (WP-077) make the path
  dynamic.

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
    fn ensure_body(&mut self, f: DeclId, oven: &mut Oven) -> Option<StmtId>; // instantiates / defines implicit members if needed; may evaluate (nested)
    fn ensure_layout(&mut self, record: DeclId) -> &RecordLayout; // via kiln_abi
    fn final_overrider(&mut self, most_derived: DeclId, at: SubobjectId, method: DeclId)
        -> Option<(DeclId, SubobjectId)>;                         // spec/sema-class.md §3.5
    fn subobjects(&mut self, record: DeclId) -> &[Subobject];     // indexed by SubobjectId
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
    pub fn fold_constant_init(&mut self, bridge: &mut dyn SemaBridge, e: ExprId, var: DeclId) -> Option<ConstValueId>;
    pub fn stats(&self) -> &EvalStats;
    pub fn footprint(&self) -> usize;
    pub fn disassemble(&self) -> String;                          // debugging aid
}
pub mod report { pub fn render(..) -> String; pub fn write(..) -> io::Result<()>; }
```

## Integration — normative (WP-054)
`kiln_oven` is the only constant evaluator: sema calls it for every
constant it needs (spec/sema.md §8), and the driver reaches it through
sema.

- **One Oven per translation unit.** `Sema` owns it; `kiln_sema`'s
  `oven_bridge.rs` implements `SemaBridge` per call. `kiln_parse::parse_tu`
  and `parse_preprocessed` (the driver's path) return it beside the AST
  (`ParsedTu { ast, oven }`) for the report.
- **Classes (amended 2026-10-28, spec/sema-class.md §8.4).**
  `ensure_body(f)` also defines an implicit or defaulted special member
  on demand (sema's `define_implicit_*`, WP-067b), so the Oven lowers the
  same body codegen does. Virtual dispatch asks
  `final_overrider(most_derived, at, method)`; the Oven's base-class
  pointer paths use `SubobjectId` (`kiln_ast::class`, re-exported by
  `kiln_oven`), resolved through `subobjects(record)`. Both read
  `Ast::virtuals`, which sema fills (WP-078a); until then they answer
  `None` / empty. The Itanium vtable layout is codegen's
  (`kiln_abi::vtable`), never the Oven's: there is no
  `kiln_oven::VTableLayout`.
- **Contexts.** Every manifestly constant-evaluated site passes an
  `EvalContext`. Its `kind` is the report's `kind`; `loc` is the
  construct's location. The constructors match where clang 19 checks
  enumeration ranges:

  | constructor | kind | enum range checked | sites |
  |---|---|---|---|
  | `EvalContext::init(var, loc)` | `init` | yes | `constexpr` variable initializers (a `constinit` one that is not `constexpr` is to clear the check; sema does not evaluate those yet) |
  | `EvalContext::converted(loc)` | `other` | yes | `case` labels, enumerators of a fixed-type enumeration |
  | `EvalContext::new(StaticAssert, loc)` | `static_assert` | no | `static_assert` |
  | `EvalContext::new(Other, loc)` | `other` | no | array bounds, bit-field widths, other enumerators, `alignas`, array designators |
  | `EvalContext::new(TemplateArg / Consteval / IfConstexpr, loc)` | as named | yes | M3 |
  | `EvalContext::new(Constraint, loc)` | `constraint` | yes | M3: constraint satisfaction (an atomic constraint's value) |

- **Nested evaluation** (R-OVEN-1, spec/sema-templates.md §4.11). The
  Oven calls `SemaBridge::ensure_body(f, oven)` the first time it compiles
  a call to `f`, passing itself, and sema may evaluate through that `oven`
  while instantiating (a `static_assert`, a template argument, an
  `if constexpr` condition or a constraint in the instantiated definition).
  `evaluate`, `try_fold` and `is_constant_initialized` are re-entrant:
  - **Contexts form a stack.** The outermost evaluation (depth 0) starts
    from an empty frame space and resets it when it ends. A nested one
    saves the enclosing evaluation's state, takes a mark of the frame space
    and allocates its registers and frames above everything live, so the
    enclosing evaluation's objects are untouched; when it ends (success or
    failure) it frees the heap allocations it still holds, releases the
    frame space to its mark and restores the enclosing state.
  - **One budget.** The step budget is shared: a nested evaluation starts
    its step counter at the enclosing total, so the steps of every context
    together are bounded by `Budget::steps`; the memory limit bounds the
    combined address space. **Depth**: a nested evaluation starts at the
    enclosing evaluation's active call depth plus one (the nested context
    itself counts as a level); if that already reaches `Budget::depth` it
    fails at once with clang's depth note at the evaluated expression.
  - **Reporting.** A nested `evaluate` is recorded and reported as its own
    line with its own `kind` and `loc`, written when it finishes; the
    enclosing line excludes its cost (steps, bytes, calls, allocs,
    `frozen_bytes`, `wall_us`, and the memory it held: the enclosing peaks
    do not include it). The nested line's `peak_bytes` and `peak_arena` are
    measured above the live bytes and arena at its start, so they too are
    exclusive; the TU's true combined peak can exceed the `total` maximum
    (the memory limit still bounds the combined amount). Nested `try_fold`
    and `is_constant_initialized` are not reported, and their cost is
    excluded from the enclosing line too.
  - **Recursion through the body.** While `ensure_body(f)` runs, `f` is in
    the `Compiling` state: a call to `f` evaluated inside it fails as a
    call to an undefined function ("undefined function 'f' cannot be used
    in a constant expression" + declared here), which is clang's behavior
    for a function whose definition is not yet available.
  - **Memoization.** A call during which a nested evaluation ran is not
    memoized (see "Memoization").
- **Enumeration range.** Where the check is on, an out-of-range value
  gives the `-Wenum-constexpr-conversion` error ("integer value V is
  outside the valid range of values [MIN, MAX] for the enumeration type
  'E'") through `SemaBridge::diag`. The evaluation continues with the
  value.
  - Following clang's `HandleCast`, the operand is sign-extended if the
    enumeration has negative values, else zero-extended. `V` is printed
    in the operand's own signedness.
  - The error is reported at the cast each time such a cast executes in
    a checked evaluation, so it recurs on every evaluation that reaches it
    (`allow_duplicate`).
  - `try_fold` never reports it.
- **Driver flags** (`kiln_driver::constexpr_opts`, spec/driver.md):

  | flag | sets | default |
  |---|---|---|
  | `-fconstexpr-steps=N` | `Budget::steps` | 1048576 |
  | `-fconstexpr-depth=N` | `Budget::depth` | 512 |
  | `-fconstexpr-memory=N[k\|m\|g]` | `Budget::bytes` | 256 MiB |
  | `-fconstexpr-backtrace-limit=N` | the backtrace limit | 10 |
  | `-fconstexpr-report=<path>` | the report | none |

  An invalid value is a driver error, as clang's is.
- **The report** is written by `ConstexprOpts::write_report` at the end
  of each `-fsyntax-only` job, even one with errors. It is not yet written
  for:
  - codegen jobs (WP-052 calls the same function);
  - a job that stops before parsing (an invalid `-fdump-ast` value, an
    unsupported target).
- **Conformance.** `tests/conformance/expr.const/` runs through the driver
  and passes with `--compiler clang++-19`:
  - one directory per [expr.const]/5 bullet reachable in M2;
  - references and aggregate members;
  - `budget/`: step counts equal to clang's smallest accepted
    `-fconstexpr-steps`, pinned with `oven:` budgets, plus one-step-short
    twins with clang's notes.

## Status and what remains
Implemented (M2): integers of every width incl. `__int128`, `bool`,
character types, binary32/64, x87 `long double`, binary128, enums;
pointers to objects, arrays and functions; **references** (lowered as
pointers: local, parameter, member, global and return references, bound
to glvalues); arrays, structs, unions, bit-fields; all C control flow (if,
loops, switch incl. fallthrough, break/continue, return); labels and
`goto` (diagnosed as clang does); calls and recursion; constexpr variables
and functions; `static_assert`; compound assignment, comma, conditional,
casts, `sizeof`/`alignof` (of a reference type: the referred type's);
string literals (each evaluation of a `StringLiteral` is a distinct object;
frozen pointers keep it through `PtrBase::StringLit { lit, origin }`, so
`p + 1 != q` for `constexpr const char *p = "ab", *q = "ab";` is rejected
as unspecified, as by clang), `nullptr`, local aggregates and their
initializers; `& * [] . ->` and lvalue-to-rvalue; every [expr.const]/5
bullet reachable in M2 (5.2, 5.3, 5.4, 5.7, 5.8, 5.9, 5.10, 5.11, 5.14,
5.15, 5.16, 5.24).

Implemented (M3, WP-061b part 1): the heap space with its size-class
allocator and the `peak_arena`/`frozen_bytes` counters ("Transient
allocation"); re-entrant nested evaluation through `ensure_body` and
`EvalKind::Constraint` ("Nested evaluation"); content-interned frozen
values (R-OVEN-2).

Implemented (M3, WP-061b part 2): `new`/`delete` of scalars, arrays
(runtime bounds, multidimensional) and aggregates with trivial default
construction and destruction, with every initializer form (none, `()`,
`(x)`, `{...}` with an array filler, a string literal for `char[]`) and
the nothrow form; the clang-worded checks and the leak check above;
`allocs` counting `new`; memoization skipping calls that allocate or free;
the five churn benchmarks, all within their gates (measured: list
`allocs` 100000, `peak_bytes`/`peak_arena` 1600/1600; vector 1088,
393216/524288; string 336 allocations, 118048/257368; tree 65520,
98280/98280; mixed 5014 allocations, 5064/5416).

Implemented (M3, WP-076): classes in constant evaluation — construction,
destruction, temporaries (static ones frozen into `LetData.value`),
constant destruction, member pointers, class `new`/`delete` and the
iterative spine walkers; see "Classes — Landed (WP-076)".

Remaining (each traps with a `kiln:` note today):
- `std::allocator<T>::allocate`/`deallocate` and
  `__builtin_operator_new`/`__builtin_operator_delete`: need class
  templates; the `is_std_allocator_call` hook is in place and returns
  false ("kiln: std::allocator in constant evaluation lands with the
  templates wave").
- `delete` through `void*`, and P2747 placement new (see above).
- Front-end gaps that keep some forms from reaching the Oven (not
  Oven work): class assignment, explicit destructor calls,
  `::operator new`/`delete`, class-specific `operator new`, inheriting
  constructors, `delete[] (expr)` (parsed as a lambda) and
  `new char[n]{"hi"}`.
- Volatile objects: reads are diagnosed ([expr.const]/5.9) only where the
  front end puts the volatile qualifier on the read expression's type.

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
- `new_delete.rs`: `new`/`delete` round trips, the leak, heap-result,
  delete and bound notes, the `allocs`/peak counters, and memo purity
  (an allocating call is evaluated every time: two leaking calls report
  two leaks with the cache on).
- `crates/kiln_driver/tests/oven_classes.rs` (WP-076, through the real
  front end): member pointer freeze/thaw/cast round trips, 20000-operand
  spines, static temporaries, constant destruction, class `allocs`.
- `nested.rs`: nested evaluation from `ensure_body` (separate records with
  exclusive costs, shared step budget, depth from the starting call, a
  function whose body is being made available, no memoization through it)
  and content interning of frozen aggregates.
- `src/mem.rs` unit tests: the size classes, LIFO reuse, best-fit carving,
  coalescing, top lowering, heap counters and per-context freeing.
- `bench.rs` (ignored): the benchmarks above.
- `common/`: a small recursive-descent C++ subset parser building kiln_ast
  directly (test-only; replaced by the real front end as it lands).
Every negative golden was produced by kiln and compared to clang 19's
output for the same source; positives are all accepted by clang 19.
Through the driver: `tests/conformance/expr.const/` ("Integration"),
`tests/conformance/expr.new/` and `expr.delete/` (`constexpr-*.cpp`
faults with clang's notes; `churn-*.cpp` and `constexpr-counters.cpp`
with `oven:` gates) and
`bench/oven/` (`cargo xtask bench --filter oven`, with vs-clang ratios in
docs/status/perf.md). The `*-nohdr` entries are header-free twins of the
libc++-based ones, which kiln cannot compile until `#include` works.
Planned *(design)*: `tests/differential/constexpr/` programs built and run
with clang, and `xtask fuzz-constexpr`.
