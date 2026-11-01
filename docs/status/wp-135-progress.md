# WP-135 progress (Oven transactional trial evaluation)

Status: complete, ready for review. Branch `wp/135-oven-rollback`.
Design, complexity and memory bounds: spec/oven.md "Trial evaluation —
Landed (WP-135)".

## Design

`Memory` keeps a stack of checkpoints plus one undo log. Each checkpoint
records the log length, the frame mark, the static top, `ctx` and the heap
allocation counter. A write to an object that existed at the innermost
checkpoint logs its old 8-byte granules once per checkpoint. Union entries,
flags, and the heap structure (snapshotted once per checkpoint) are logged
as well. Objects created after the checkpoint are never logged.

- Rollback replays the log in reverse, then releases the frame objects
  made since the checkpoint.
- Commit pops the checkpoint.
- Outside a trial, each write pays for one `is_empty` test.

`__builtin_constant_p` (`Op::Speculate`) runs its region as a
`Speculative` trial, with clang's rules:
- it may not modify frame or static objects from outside the trial;
- it may not `new` or `delete` in its own context;
- it may modify heap objects from outside the trial, and those writes are
  undone when the trial fails.

| deliverable | status |
|---|---|
| 1. checkpoint/rollback in the Oven | done: `src/mem.rs` "trial evaluation" (`checkpoint`, `commit`, `rollback`, `trial_write`, `union_put`, `set_flags`); hooked into every VM write path (store, bit-field store, copy, `bulk_copy`, ZeroInit, bit_cast, vptr, active member, MakeConst, cleanups) |
| 2. `__builtin_constant_p` and sema audit | done: `speculate()` checkpoints, commits or rolls back; sema audit below; no kiln_sema edit was needed |
| 3. tests | 7 `mem.rs` unit tests, 2 conformance tests (below) |
| 4. bench | `bench/oven/trial-rollback.cpp`, baseline recorded (below) |
| 5. docs | spec/oven.md section, Status and Tests; this file; kbc `Speculate` comment; wp-130 note resolved |

## Tests

| file | covers |
|---|---|
| `crates/kiln_oven/src/mem.rs` `rollback_restores_interleaved_writes` | interleaved writes to frame, static and heap objects; a fresh object's writes are not logged |
| `granules_are_logged_once_per_checkpoint` | log deduplication, and re-logging under a nested checkpoint |
| `rollback_frees_allocations_and_revives_frees` | allocations since the checkpoint are freed, frees are undone, heap counters and slots are restored, blocks are reused |
| `nested_checkpoints` | an inner commit is undone by an outer rollback; an inner rollback leaves the outer trial's effects |
| `unions_and_flags_are_restored` | active-member entries and flags |
| `speculative_trials_refuse_outside_effects` | frame and static refusals, `new`/`delete` in the trial's own context, heap writes allowed |
| `failed_trials_hold_no_memory` | flat `footprint` and arena over many failed trials |
| `tests/conformance/expr.const/constant-p-trial.cpp` (clang + kiln, strict diags) | (a) a callee write to a caller local or global is refused and the object is unchanged; arrays, unions; own locals and lambdas give 1; (b) allocating trials give 0; (c) nested trials; (d) a successful trial keeps its heap write |
| `tests/conformance/expr.const/constant-p-trial-rollback.cpp` (kiln only, `oven: peak_arena<=512 allocs<=3`) | a failed trial's heap writes are rolled back, nested or not; 10000 failed trials in O(1) memory |

## Bench (`cargo xtask bench --filter oven`)

`oven/trial-rollback` runs 20000 failed trials. Each trial writes a
pre-existing heap buffer, fills `int[16]`, calls `new int[256]`, and
divides by zero.

| | `peak_arena` | `allocs` | `frozen_bytes` | result |
|---|---|---|---|---|
| main (ece8aaf), N=1000 | 1,024,320 | 1001 | 32 | error: leaked allocations |
| main (ece8aaf), N=20000 | 20,480,320 | 20001 | 32 | error: leaked allocations |
| WP-135, N=1000 or 20000 | 320 | 1 | 32 | ok (steps 460007 for N=20000, 39 ms release) |

Hot path, A/B of release binaries at the merge base and at WP-135 (best of
15 runs each, interleaved, host load about 9):

| bench | merge base | WP-135 |
|---|---|---|
| fib | 6.3 ms | 6.6 ms |
| churn-tree | 26.4 ms | 26.9 ms |
| virtual-dispatch | 19.1 ms | 18.8 ms |
| churn-vector | 577 ms | 581 ms |

Steps and the oven counters are identical, and max RSS is within 1%. The
full `--filter oven` run reports "REGRESSED" against `bench/baselines/oven.json`,
but those baselines date from WP-077/088. Both binaries show the same drift,
so it predates this WP. `sort`, `string` and `table` fail in the front end,
on main as well.

## kiln_sema speculative sites (audit)

Each of these sites is an outermost Oven evaluation, or a nested one through
`ensure_body` with a lent Oven. Afterwards:
- `Memory` is reset (`eval_outermost`);
- provisional global resolutions are reverted at the next outermost start;
- the frozen result is interned (deduplicated).

So a failed or discarded evaluation leaves no Oven state behind, and no site
needs a checkpoint. Its residue is an interned constant, and any function
instantiation `ensure_body` performed, which clang also performs.

| site | kind |
|---|---|
| `templ/args.rs:418` | SFINAE `fold_int` (deduction/substitution) |
| `templ/args.rs:437`, `templ/nttp.rs:248/320`, `templ/pack.rs:390` | NTTP/pack `eval_const`/`fold_int` |
| `templ/concepts/requires.rs:170`, `templ/concepts/sat.rs:506/599` | `requires` / constraint satisfaction (`EvalKind::Constraint`) |
| `templ/builtins/traits.rs:130`, `templ/builtins/builtin_templates.rs:202/221` | trait and builtin-template arguments |
| `except.rs:59` (`fold_int`), `except.rs:98` (`eval_const`) | noexcept specifiers |
| `init.rs:830` | narrowing `try_fold` |
| `init.rs:1044`, `struct_bind.rs:464`, `new_delete.rs:711/767` | array initializer bounds, `tuple_size`, `new` bounds |
| `stmt.rs:253/435/846/860` | `if constexpr` condition, case values, fall-off analysis |
| `consteval.rs:242/250` | immediate invocations |
| `decl.rs:726/2558/2961/3747/3772/4135/4217/4250/4730/4823` | array bounds, bit-field widths, constexpr initializers, enumerators, `static_assert`, alignment attributes |
| `expr.rs:470/1929/1948`, `gnu.rs:258` | usable-in-constant-expressions, division-by-zero and shift-count warnings, `offsetof` |
| `class/members.rs:343`, `class/special/declare.rs:123`, `class/compare/mod.rs:204/378` | in-class static initializers, `explicit(bool)`, comparison categories |

## Deferred and open

- `TrialMode::Transactional` has no evaluator user yet. Candidates are
  speculative constant folding in codegen and reflection's
  `define_aggregate` retries.
- Rollback assumes the trial region never releases frames below the
  checkpoint's frame top. This holds for `Op::Speculate`.
- Deliberate divergence: clang 19 keeps the heap writes of a failed
  speculative evaluation, and kiln rolls them back.
