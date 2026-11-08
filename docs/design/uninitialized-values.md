# Design note: flow-based uninitialized-value analysis for locals

Status: implemented (WP-862, `crates/kiln_sema/src/uninit.rs`;
spec/sema.md §7.14 is the contract). It was proposed as WP-186 item 11
and depends on the per-function CFG of [reachability.md](reachability.md).
The implementation differs from this note in three ways:
- it tracks non-empty class types too, as clang 19 does;
- it has no memory budget or skip remark, because the scratch is two
  bits per variable per block and is reused;
- the tests are in `tests/conformance/basic.indet/uninit-*.cpp`.

## What clang computes

`clang/lib/Analysis/UninitializedValues.cpp` runs over the body's CFG
when any of these is enabled:

- `-Wuninitialized`;
- `-Wsometimes-uninitialized`, in `-Wuninitialized`, which is in `-Wall`;
- `-Wconditional-uninitialized`, off by default;
- `-Wuninitialized-const-reference`.

Tracked variables (`isTrackedVar`) are the function's own non-static,
non-exception local variables of scalar or vector type. Class types and
references are left to the syntactic `SelfReferenceChecker`, which kiln
already ports in `self_ref.rs`. Each tracked variable's value per
program point is one of `Unknown`, `Initialized`, `Uninitialized` or
`MayUninitialized`, two bits per variable.

1. `ClassifyRefs` labels each `DeclRefExpr` of a tracked variable as
   `Init`, `Use`, `SelfInit` (`int x = x;`), `ConstRefUse` or `Ignore`.
   An `Ignore` is an address taken, a non-const reference bound, or an
   unevaluated operand.
2. A worklist runs over blocks in reverse post-order and merges the
   predecessors' vectors until a fixed point. An `Init` sets
   `Initialized`. A declaration without an initializer sets
   `Uninitialized`.
3. A final pass visits each use with its block's incoming vector and
   reports the first offending use per variable:
   - `Uninitialized` → `variable 'x' is uninitialized when used here`,
     with the note `initialize the variable 'x' to silence this warning`
     and a ` = 0` / `= nullptr` / `= false` fix-it.
   - `MayUninitialized` with a predecessor branch that decides it →
     `-Wsometimes-uninitialized`:
     `variable 'x' is used uninitialized whenever 'if' condition is false`,
     plus the `uninitialized use occurs here` and `remove the 'if' if its
     condition is always true` notes. clang finds the deciding edge by
     scanning the use's predecessors for one whose other successor
     initializes.
   - Otherwise → `-Wconditional-uninitialized`
     `variable 'x' may be uninitialized when used here`.

## Fit in kiln

- Module `kiln_sema::uninit`, run from the same end-of-body hook as the
  other flow warnings, after the CFG is built. It is skipped in
  dependent contexts, after errors, and when all four groups are off.
- Variables are indexed densely: `tracked: Vec<DeclId>` plus an
  open-addressed `DeclId → u32` map in the per-`Sema` scratch.
- Block state is one `Vec<u64>` of `2 × tracked × blocks` bits in the
  scratch, cleared per function. Memory is
  `blocks × ceil(2·vars / 64) × 8` bytes, for example
  4 KiB blocks × 512 vars = 512 KiB. Above a budget (default 64 MiB,
  flag-tunable), the analysis is skipped with the
  `-Rkiln-analysis-skipped` remark. It never guesses, so there are no
  false positives from truncation.
- Use classification reuses the value-use rules already written for
  `self_ref.rs`: lvalue-to-rvalue, increment, compound assignment,
  `std::move` and copy-constructor arguments. Both checkers then agree
  by construction.
- The analysis is safe Rust over indices only. The fixed point is
  bounded by `blocks × lattice height (3) × vars`, so it terminates by
  construction. A step counter enforces the same bound defensively.
- Interaction with the Oven: none. Constant evaluation reports reads of
  uninitialized objects itself ([expr.const]/5); this analysis is a
  warning only.

## Validation

- Corpus in `tests/conformance/dcl.init/uninit-*.cpp` (strict-diags,
  both oracles). Cover straight-line code, `if`/`switch`/loops, `goto`,
  `&&`/`||`/`?:`, the fix-it spellings per type, self-init, the
  const-reference argument, and the `-Wsometimes` branch kinds (`if`,
  `switch` case/default, `for`/`while` loop entered or not, `&&`/`||`
  operand).
- Perf: add generated bodies with many locals and blocks to the perf
  harness. Track peak scratch bytes and time per function. A regression
  gate applies at the same thresholds as Sema's other passes.

## BACKLOG split (proposed)

- CFG-a: `kiln_sema::cfg` builder, `-fkiln-dump-cfg`,
  `tests/cfg/` against clang's `debug.DumpCFG`, plus perf budgets.
- CFG-b: port the runtime-diag filter, fall-off and -Wexceptions to the
  CFG, with no corpus changes.
- CFG-c: port implicit fallthrough and -Wunreachable-code. Delete
  `try_stmt/reach.rs`'s walk modes.
- CFG-d: `-Winfinite-recursion`, `-Wmissing-noreturn`, `noreturn`
  destructors.
- UNINIT-a: the tracked-variable dataflow,
  `-Wuninitialized` for scalar locals (`int x = x;` included) and
  `-Wconditional-uninitialized`.
- UNINIT-b: `-Wsometimes-uninitialized` edge attribution and notes,
  `-Wuninitialized-const-reference`, and the fix-it spellings.
