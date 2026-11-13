# Design note: reachability analysis for flow-sensitive diagnostics

Status: implemented in WP-509 (CFG-a..d: the builder and dump, the
runtime filter, fall-off and -Wexceptions, fallthrough and unreachable
code, -Winfinite-recursion and -Wmissing-noreturn; spec/sema.md
§7.5.1-§7.5.3). The uninitialized-values consumer is WP-510. Proposed in
WP-186 item 11. Companion note:
[uninitialized-values.md](uninitialized-values.md).

## What clang computes

At the end of each non-dependent function body, clang's
`AnalysisBasedWarnings::IssueWarnings` builds one `CFG` for the body.
The build options are `PruneTriviallyFalseEdges`, `AddInitializers`,
`AddImplicitDtors`, `AddTemporaryDtors` and
`AddCXXDefaultInitExprInCtors`, with no EH edges. Blocks hold
statements, expressions, member initializers and implicit destructor
calls in evaluation order. An edge whose condition `tryEvaluateBool`
folds keeps the dead successor as an "unreachable alternate", so
analyses can still see it. A `noreturn` call or destructor ends its
block with an edge to the exit. A `try` gets a dispatch block whose
edges go only to the handlers. The CFG is built lazily, and only when
an enabled warning needs it. Its consumers are:

| diagnostic | clang entry point |
|---|---|
| `DiagRuntimeBehavior` filter (division by zero, null dereference, `-Wunused-value`, array bounds, ...) | `flushDiagnostics` against reachable blocks |
| `-Wreturn-type`, `-Winvalid-noreturn`, `-Wmissing-noreturn`, coroutine/lambda fall-off | `CheckFallThrough` |
| `-Wunreachable-code{,-break,-return,-loop-increment,-fallthrough}` | `reachable_code::FindUnreachableCode` |
| `-Wimplicit-fallthrough` | `FallthroughMapper` + predecessor scan |
| `-Wexceptions` (throw in a non-throwing function) | `checkThrowInNonThrowingFunc` |
| `-Winfinite-recursion` | `checkRecursiveFunction` |
| `-Wuninitialized` family | `runUninitializedVariablesAnalysis` (companion note) |
| `-Wthread-safety`, `-Wconsumed`, `-Wcalled-once-parameter` | out of scope for kiln today |

## Where kiln was (before WP-509)

kiln had no CFG. `try_stmt/reach.rs` (about 1k lines) is a structured
walk over the AST that mirrors the CFG's edges, as described in
spec/sema.md §7.5 and §7.10. Four entry points re-walk the body, each
to its own fixed point:

- `unreachable_exprs`, the runtime-diag filter (WP-203);
- `fall_off_kind` (WP-219);
- `dead_scan`, for -Wunreachable-code (WP-234; O(regions × body));
- `stmt_flow`, for implicit fallthrough.

The walk matches the corpus. Its known gaps are:

- `noreturn` implicit and temporary destructors;
- `-Winfinite-recursion` and `-Wmissing-noreturn`;
- clang 22's CFG for bodies with `if consteval`.

Each gap means extending every walk mode. Each new consumer pays for
another traversal.

## Proposal: one lazily built, per-function CFG

Status: implemented in WP-509 (CFG-a). As built, edges live in one
`edges` vector with per-edge bits (`LIVE`, `SCAN`, `NULL`, `RETURN`,
`FAKE`, `ABNORMAL`) and blocks index `succs`/`preds` adjacency ranges;
the builder is one forward walk; temporaries' destructors come from the
`CXXBindTemporaryExpr`s under each `ExprWithCleanups` (spec/sema.md
§7.5.1). Measured sizes: `spec/landed/sema/WP-509.md`.

- Module `kiln_sema::cfg`, all safe Rust and index-based (the AST
  arena style).
  - `Cfg { blocks: Vec<Block>, elems: Vec<Elem>, succs: Vec<Succ> }`,
    where `Block` holds `Range<u32>` views into `elems` and `succs`.
  - `Elem` is one of `Stmt(StmtId)`, `Expr(ExprId)`,
    `Init(MemberInitId)`, `AutoDtor(DeclId)`, `TempDtor(ExprId)` or
    `Scope*`.
  - `Succ { to: BlockId, pruned: bool }` models clang's
    `AdjacentBlock` unreachable alternate.
  - The block terminator records the branching statement, so
    diagnostics can name `'if' condition is false`.
  - There is no `Rc`, no pointer graph and no `unsafe`.
- Builder: one post-order AST visit (clang `CFGBuilder`). It reuses
  `known_truth` (the `tryEvaluateBool` port) for pruning and the
  existing `noreturn` / `isAllEnumCasesCovered` logic. Labels and
  `goto` are resolved through a label → block map. Temporaries'
  destructors come from the materialization records codegen already
  reads.
- Lifetime: build at the end of the body in `IssueWarnings` order, only
  when an enabled consumer needs it (`Diags::is_enabled` per group, as
  clang). Drop it before the next function. The three `Vec`s live in a
  per-`Sema` scratch that is `clear()`ed, not freed. Steady-state memory
  is then bounded by the largest body, not by the TU, and template
  instantiation (the hot path of reflection-heavy code) never allocates
  per function.
- Budgets: block and element counts are O(AST nodes in the body) with
  small constants (roughly 16 B/elem, 24 B/block). A hard cap
  (`-fkiln-cfg-max-elems`, default sized so a 3 GiB process cannot be
  approached) skips flow warnings for an oversized body with a
  `-Rkiln-analysis-skipped` remark instead of degrading. Build time and
  peak bytes go into the validation harness's perf tables
  (docs/VALIDATION.md), measured on generated large-switch and
  deep-nesting bodies.
- Constant evaluation is untouched. The Oven never needs a CFG, and
  `consteval` / `constexpr` bodies are analysed like any other body
  only when a warning asks.

## Migration and validation

1. (Implemented in WP-509, CFG-a; the corpus is
   `tests/conformance/stmt.*/cfg-*.cpp`, `expect: cfg`.) Add the builder
   and a `-fkiln-dump-cfg` debug printer in clang's
   `debug.DumpCFG` layout. Compare it against
   `clang -cc1 -analyze -analyzer-checker=debug.DumpCFG` on a CFG corpus
   (`tests/cfg/*.cpp`, normalized block numbering, both oracles).
2. (Implemented in WP-509: CFG-b the runtime filter, fall-off and
   -Wexceptions; CFG-c implicit fallthrough and unreachable code;
   `try_stmt/reach.rs` deleted.) Port the consumers one at a time onto
   the CFG: the runtime filter,
   fall-off, -Wexceptions, implicit fallthrough, then unreachable code.
   The existing conformance corpus is the regression oracle: zero
   expectation changes per port. Delete each walk mode once it is ported.
3. New diagnostics on top: `-Winfinite-recursion`,
   `-Wmissing-noreturn`, `noreturn` destructors (implemented in
   WP-509, CFG-d; `noreturn` destructors in CFG-a), then the
   uninitialized-values analysis (WP-510).
