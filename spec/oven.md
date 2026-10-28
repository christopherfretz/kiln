# spec: kiln_oven — the constant evaluator

Standard: [expr.const] (the whole of it: core constant expressions, the list
of disallowed operations, constant-initialization, manifestly
constant-evaluated contexts, `std::is_constant_evaluated`,
`if consteval`), [basic.life], [basic.lval] (access rules), [class.union]
(active member), [expr.new]/[expr.delete] (transient allocation),
[dcl.constexpr], [dcl.constinit], [expr.prim.req], [meta.reflection] (P2996
and successors, M6), [intro.object], [basic.align], [conv.*]. This is a
*design* spec: the implementation starts in M2 on a C-like subset and grows
with each milestone, but the data model below is fixed now because every
later feature depends on it.

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
4. Observable: every evaluation reports `steps`, `peak_bytes`, `calls`,
   `allocs`, and wall time; totals per TU are available from the driver.

## Architecture
```
typed AST (kiln_ast) ──compile──▶ KBC function (per FunctionDecl instantiation, cached)
                                     │
   SemaBridge (trait, implemented by kiln_sema): instantiate-on-demand, lookup, reflection ops, layout
                                     │
                                  VM: frames + registers + abstract memory (arena) ──▶ ConstValue (frozen result)
```

### Bytecode (KBC)
- Register machine. Each function has a fixed frame layout: `n_regs` slots
  of 8 bytes for scalars/pointers plus a per-frame *local memory block* for
  objects with addresses (locals whose address is taken or which are
  aggregates), laid out by kiln_abi record layout.
- Instructions are a compact `enum` (`#[repr(u8)]` tag + fixed operands),
  stored in a `Vec<u32>`-style stream; decoding is a match, no dynamic dispatch.
- Typed arithmetic ops per `IntWidth`/`Float` kind with the overflow checks
  [expr.const] requires (signed overflow is a hard error; unsigned wraps;
  floating evaluation uses the target's IEEE semantics, with `-ffast-math`
  irrelevant at compile time).
- Calls: `Call(FuncId, args...)`; `CallIndirect` for pointer-to-function and
  virtual dispatch (vtable lookup through the dynamic type recorded in the
  allocation's shadow); `CallIntrinsic(IntrinsicId)` for builtins and the
  `std::meta` runtime; `CallBridge` for anything needing sema.
- Control flow: `Jmp`, `JmpIf`, `Switch` (table or binary search).
- Lifetime ops: `BeginLife(alloc, offset, type)`, `EndLife`, `SetActiveMember`,
  `Alloc(type, count) -> ptr` (transient `new`), `Dealloc(ptr)`.
- Compilation is lazy, on first call; recursion compiles the callee when
  reached. Uncompilable functions (non-constexpr, no body) compile to a stub
  that diagnoses on call, so sema need not pre-check.

### Abstract memory
- `AllocId(u32)` → `Allocation { bytes: Box<[u8]>-like slice in the arena,
  shadow: Shadow, kind: Static|Frame|Transient|Temporary|StringLiteral,
  type: TypeId, dynamic_type: TypeId, life: Live|Dead|NotStarted, owner }`.
- `Shadow` tracks, per 8-byte granule (pointer-sized), one of:
  `Uninit`, `Scalar` (bytes are the value), `Pointer(Provenance)`,
  `MemberPointer(..)`, `Reflection(info)`, `Padding`; plus a bitmap at byte
  granularity for initialized-ness of sub-granule writes (for `char` arrays
  and bitfields). Unions keep the active member index per union subobject in
  a side map keyed by (alloc, offset).
- `Pointer { alloc: AllocId, offset: u32, provenance: Provenance }` where
  provenance records the *designator* path only when needed for diagnostics
  (clang's `LValue` designator is replaced by (type, offset) recovery via
  kiln_abi layout, which is cheaper and exact).
- Reads check: alloc live, offset+size in bounds, initialized, the access
  type is compatible with the stored kind ([basic.lval]/11 and the
  [expr.const] "read of an object outside its lifetime / of a volatile /
  of an unrelated type" rules), and that the object is not a
  non-`constexpr` variable from outside the evaluation.
- Writes mark granules and check `const` and lifetime.
- The arena is `reset()` at the end of each top-level evaluation; the result
  is frozen into `ConstValue`, an immutable, interned, compact representation
  (scalars inline; aggregates as byte image + shadow summary; pointers as
  `(DeclId, offset)` or `(StringLiteralId, offset)`), stored in the AST.
  Frozen values can be *thawed* back into an allocation when a later
  evaluation reads a `constexpr` variable (copy-on-read, cached).

### Evaluation entry points (called by sema)
- `evaluate_as_constant(expr) -> Result<ConstValue, EvalFailure>` for
  constant expressions, template arguments, `static_assert`, array bounds,
  enumerator values, `constinit`, `consteval` calls (immediate invocation),
  `if constexpr`, reflection splices.
- `try_fold(expr) -> Option<ConstValue>` for codegen/sema folding where
  failure is not an error (must not emit diagnostics; records them in a
  scratch buffer that is discarded).
- `is_constant_initialized(var)`.
- `EvalFailure` carries the diagnostic chain (primary error + notes "in call
  to ..." with argument values rendered on demand, not eagerly).

### Limits and metrics
- `Budget { steps, bytes, call_depth, alloc_count }` from driver flags
  `-fconstexpr-steps=`, `-fconstexpr-depth=`, `-fconstexpr-memory=`
  (kiln extension), with clang-compatible defaults where clang has one.
- `EvalStats` per evaluation and accumulated per TU; `-fconstexpr-report`
  prints the top-N evaluations by time/memory with locations.

### Memoization
- A function is `pure` for memoization if its KBC has no bridge calls, no
  writes through pointers that escape the frame, and no reads of mutable
  globals; computed at compile time of the function. Calls to pure functions
  whose arguments are all scalar/reflection values and whose result is a
  scalar or small frozen aggregate are cached in a bounded LRU keyed by
  `(FuncId, args)`. Cache hits still count their recorded `steps` toward the
  budget so limits remain deterministic.

### Reflection runtime (M6)
- `std::meta::info` is a `ReflectionId(u32)` into sema's entity tables;
  it is a scalar in memory (8 bytes) with shadow kind `Reflection`.
- All `std::meta` functions are intrinsics dispatched by name from the
  `<meta>` header's declarations (`consteval` functions whose definitions are
  kiln-internal); they call `SemaBridge` methods. `define_aggregate` and
  friends mutate sema state and are therefore only allowed in the
  evaluation contexts the standard permits; the bridge enforces that.

## Interface (M2 shape; grows)
```rust
pub trait SemaBridge {
    fn function_body(&mut self, f: FunctionId) -> Option<&Stmt>;      // instantiates if needed
    fn layout(&self, t: TypeId) -> &RecordLayout;
    fn vtable(&self, t: TypeId) -> &VTableLayout;
    fn diag(&mut self, d: Diag);
    // reflection ops added in M6
}
pub struct Oven<'a> { /* arena, caches, budget, stats */ }
impl Oven<'_> {
    pub fn new(ast: &Ast, bridge: &mut dyn SemaBridge, budget: Budget) -> Self;
    pub fn evaluate(&mut self, e: ExprId, ctx: EvalContext) -> Result<ConstValue, EvalFailure>;
    pub fn try_fold(&mut self, e: ExprId) -> Option<ConstValue>;
    pub fn stats(&self) -> &EvalStats;
}
```

## Tests
- `tests/conformance/expr.const/`: one test per bullet of [expr.const]/5
  (the disallowed-operation list) with the diagnostic and note chain, plus
  positive tests for each allowed operation.
- Differential: `tests/differential/constexpr/` programs are built with clang
  and run; their `static_assert`s and runtime prints must agree with kiln.
- Fuzz: `xtask fuzz-constexpr` generates random well-defined constexpr
  programs (integer arithmetic, arrays, structs, loops, recursion) and
  cross-checks results with clang.
- Perf: `bench/oven/` has microbenchmarks (constexpr fib, sieve, string
  building, sort, big `std::array` tables, reflection loops) with recorded
  steps/bytes/time baselines and vs-clang ratios.
