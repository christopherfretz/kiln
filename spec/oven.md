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
  keep their active member in a side map keyed by (allocation, offset,
  union type) (`mem::UKey`), so unions nested at one offset (`union {
  char c; union { int x; float y; } q; }`) each have their own entry
  (WP-224); two members of one union type at one offset share it.
  `mem::Memory::union_alts` holds, per entry, the other members that may
  equally be active after a placement new whose pointer names several
  (WP-233, "Classes").
  Any write into a granule clears its pointer bit, so a class holding no
  data (`Types::holds_no_data`: an empty class, or one of only such
  subobjects) is zero-initialized and copied as zero bytes: an empty base
  or `[[no_unique_address]]` member may share its address with a pointer
  (libc++'s `__compressed_pair` in `std::unique_ptr`).
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
  A member call prints its object before the callee name (clang
  `describeCall`): a `CXXMemberCallExpr` its implicit object expression as
  written, a member `CXXOperatorCallExpr` its first operand as written
  (`u.s.data[0].operator=(e)`, `*p.operator+=(0)`), anything else the
  `this` pointer's designator. A designator through a union names the
  member the pointer was formed through: the union's recorded active
  member when it holds the pointee at that offset, else the first member
  that does (WP-213).
- A closure call prints as clang's (WP-237;
  `tests/conformance/expr.prim.lambda/constexpr-call-notes.cpp`): the
  object expression, then `operator()` with a generic lambda's arguments
  (`f.operator()(0)`, `gl.operator()<int>(0)`); a static call operator has
  no object (`operator()(0)`); the static invoker of a conversion to a
  function pointer prints its first argument as the object
  (`(0->operator()()`). **Deviations:** an immediately invoked lambda
  prints `....operator()(0)` where clang pretty-prints the whole
  `LambdaExpr` (body included; kiln has no statement printer), and a
  closure type in a note's template arguments prints `(lambda)` where
  clang adds ` at F:L:C` (the Oven's notes have no source manager).
- A float in a note's message (`value V is outside the range of
  representable values of type 'T'`) is spelled as clang's
  `OptionalDiagnostic << APFloat`: `APFloat::toString` with
  `ceil(p * log10 2)` significant digits (`(p * 59 + 195) / 196` for a
  `p`-bit significand: 3 for `__bf16`, 4 `_Float16`, 8 `float`, 16
  `double`, 20 x87 and 35 binary128 `long double`), taken from the exact
  decimal value and rounded half up on the first dropped digit; plain
  notation unless that would pad more than three zeros or imply more
  digits than that precision, else `D.DDDE+X` (`1.0E+300`, `128.1`,
  `9.999999999999999E+22`, `6.55E+4`); `+Inf`, `-Inf`, `NaN`, `0`, `-0`
  (WP-237; `fltstr.rs`). A float argument in an "in call to" note is
  clang's `%e` (`f(1.000000e+10)`).
- Argument values print as clang's `APValue::printPretty` (WP-241): a
  class or array as a brace list of its bases and members, an array cut
  off after 10 elements with `...`, a member pointer as `&C::m` (`0` when
  null), a pointer to a temporary or to a lifetime-extended object as
  that object's expression (clang names a temporary by its
  `MaterializeTemporaryExpr`). A failure inside the inline trivial copy
  or move (`Op::Copy` aux bit 4) is reported inside the frame clang
  creates for the trivial constructor or assignment: the fault at the
  source argument, then "in call to 'T(t)'" (or "'t.operator=(s)'"), for
  memory faults, uninitialized subobjects and mutable reads alike
  (`tests/conformance/expr.const/call-note-argument-values.cpp`,
  `call-note-constructor.cpp`, `call-note-temporary-arguments.cpp`).
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
- `Oven::fold_rvalue(bridge, e) -> Option<ConstValueId>` (WP-165):
  `try_fold` in **lax mode** (`RunState::lax`), clang's codegen-time
  `EvaluateAsRValue`/`tryEmitAsConstant`. A `const`, non-volatile variable
  that has an initializer (or a known value) but is not usable in constant
  expressions (a `const double`, a `const _Atomic(int)`, a `const` class,
  array or pointer) is read as if it were (`VarInfo::lax`); its `mutable`
  members are not (below, WP-183). Lax mode
  applies only to an outermost evaluation (nested in a bridge callback it
  is plain `try_fold`). Variables resolved under lax mode are recorded in
  `lax_globals` and reset to unresolved when the next outermost
  evaluation runs in the other mode, and lax-mode calls are never
  memoized, so a lax read never leaks into a strict evaluation. Codegen
  uses it for its non-initializer fold candidates; sema uses it
  (`OvenBridge::fold_rvalue_int`) for clang's folded array bound
  (spec/sema.md §4.2).
- Freezing a pointer or reference into a constant rejects a pointer to an
  object of automatic storage duration (a static allocation whose origin
  is a local variable read as a global): "pointer to '…' is not a
  constant expression" (or "reference to …" when the frozen type is a
  reference; clang `note_constexpr_non_global`), with "declared here".
- Freezing a pointer or reference to a temporary lifetime-extended by
  the variable being initialized ([class.temporary]/6) rejects one whose
  type is destructed (`Ast::needs_destruction`: a non-trivial destructor
  of the type, an array element or a member; clang
  `CheckLValueConstantExpression`): "non-trivial destruction of
  lifetime-extended temporary with type 'const D' used in the result of a
  constant expression is not yet supported" at the
  `MaterializeTemporaryExpr`, the type as materialized (`const D`,
  `const D[2]`, `D` for `D&& r = D{3}`). `constexpr RD rd{{1}}` with a
  `const D&` member and `constexpr const D& d = {2}` fail; a temporary
  destroyed within the evaluation (`RD{{1}}.r.v`) does not. A
  non-constexpr variable so initialized is dynamically initialized and
  the temporary's destructor registered (WP-231, WP-233; see also
  "Temporaries" below;
  `tests/conformance/class.temporary/constexpr-nontrivial-dtor.cpp`,
  `tests/codegen/ref_temp_dtor_dynamic.cpp`,
  `tests/conformance/expr.const/constexpr-ref-temporary-nontrivial-dtor.cpp`).
- A **function parameter outside its call** (named in a template
  argument or a `static_assert` in the body, where it has no frame;
  WP-183) takes the same globals path as an external object that is never
  usable (`var_info` returns early for `ParmVar`; a default argument is
  not an initializer). So `X<&q>` and `Y<q>` (`const int&` parameter)
  fail in the freeze above, as clang's `VisitVarDecl` `Success(VD)` does,
  and only a read (`Z<q>`, `static_assert(r == 0)` through a reference)
  is clang's `note_constexpr_function_param_value_unknown` ("function
  parameter 'q' with unknown value cannot be used in a constant
  expression") with "declared here" (`MemFault::External`, `notes.rs`).
  This is a lowering change only; it is not on the interpreter hot path.

- **Variable length arrays** (WP-183, GNU extension): a VLA local lowers
  to a zero-sized opaque object, so declaring one in a constant
  evaluation succeeds, `sizeof` of a VLA (`is_vla`) throws `Invalid`
  ("subexpression not valid in a constant expression"), and an element
  access is clang's `note_constexpr_access_unsized_array` ("{read
  of,assignment to,...} element of array without known bound is not
  allowed in a constant expression"). Clang's designator lets the
  pointer arithmetic succeed and fails the access; kiln faults at the
  arithmetic for a nonzero index, where the cold note path
  (`indexes_vla`) reports it as a read whatever the access is.
- **Mutable members** (WP-183, [expr.const]/4, /5.9): a `mutable`
  subobject of an object usable in constant expressions is not usable, so
  a read of it (or of anything inside it) from an object whose lifetime
  began outside the evaluation is "read of mutable member 'm' is not
  allowed in a constant expression" with "declared here" at the member
  (clang `note_constexpr_access_mutable`), in strict and lax mode alike.
  `TyInfo::has_mutable` and `FieldInfo::mutable` record the members; a
  static object thawed from a value (`resolve_var`, a usable
  `resolve_let`, `thaw`) whose type has one gets the allocation flag
  `MUTABLE_OUTSIDE`. Loads from an allocation with that flag or
  `HAS_UNION` take the outlined slow path (`slow_access`; no cost for
  other allocations): `mutable_path` walks from the allocation root to
  the read object (a union through its active member) and faults at the
  outermost `mutable` member on the way; a read of a whole class object
  (the `Copy` of a trivial copy) faults at the first `mutable` member it
  reads (members before bases, recursively; not one of empty class type;
  any in a union; clang `diagnoseMutableFields`). The object being
  initialized (`RESOLVING`), its lifetime-extended temporaries, and
  frame and heap objects never carry the flag, so their mutable members
  are read and written freely. The read carries the trivial copy's "in
  call to 'S(s)'" frame (WP-241). Deviations: clang rejects every such read
  before C++14 and during constant destruction, kiln does not.

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

### Builtins — normative (WP-088)
The §9.5 builtin functions of spec/sema-templates.md, with clang 19's
results and notes. A builtin is one `Op::Builtin`
instruction (`aux` = the `bi` kind | sub-kind << 8; six operand words:
three argument registers, the destination, and two immediates such as
the element type), so a `memcpy` of 4096 `int`s or a `strlen` of a long
literal is one instruction and one step, not an interpreted loop.
- **Folded at compile time:** `__builtin_huge_val*`, `__builtin_nan*` and
  `__builtin_nans*` (the argument must be a string literal,
  parsed as clang's `StringRef::getAsInteger(0)`: decimal, `0` octal,
  `0x` hex, `0b` binary, `0o` octal; an empty string is payload 0; any
  other argument is not a constant), and the source-location builtins (below).
- **Atomics (WP-154):** `__atomic_always_lock_free`, `__atomic_is_lock_free`
  and `__c11_atomic_is_lock_free` fold through `bi::LOCK_FREE` as clang
  19's `IntExprEvaluator`: a power-of-two size within the target's inline
  width (`Target::max_atomic_inline_bytes`: aarch64 16, x86_64 8 bytes) is
  lock-free for the c11 form, a null object, a single byte, or an object
  whose pointee alignment is at least the size; otherwise
  `always_lock_free` is false and `is_lock_free` is not a constant.
  `NonAtomicToAtomic`/`AtomicToNonAtomic` casts are no-ops. Every other
  `AtomicExpr` is not a constant (clang's generic note).
- **`__builtin_va_arg` (WP-171):** a `VAArgExpr` is never a constant: the
  generic "subexpression not valid in a constant expression" note at the
  expression (clang has no `VisitVAArgExpr`), as for `va_start`/`va_end`/
  `va_copy`. It counts as a side effect for `__builtin_constant_p`-style
  folding (`has_side_effects`).
- **`DesignatedInitUpdateExpr` (WP-196):** never a constant (clang's
  evaluator has no visitor for one): the generic note at the expression,
  so a `constexpr` variable initialized by a partial override is
  ill-formed (spec/sema.md §6). Its `NoInitExpr` slots are reached only
  through it.
- **Not constant-evaluable** (a trap with "subexpression not valid in a
  constant expression", as clang 19): `__builtin_signbit`, the six
  ordered comparisons, the math functions; `__builtin_unreachable` when
  reached; `__builtin_operator_new`/`_delete` outside
  `std::allocator<T>::allocate`/`deallocate` (WP-061b; clang's
  untyped-memory note; inside them they allocate, "Transient
  allocation", WP-212).
- **Bits** (`bi::CLZ`/`CTZ`/`POPCOUNT`/`BSWAP`): any width up to 128; a
  zero operand to `clz`/`ctz` without the `g` forms' fallback is not a
  constant (no note, as clang).
- **Classification** (`bi::FP_CLASS`): `isfinite`/`isinf`/`isnan`/
  `isnormal` on every float format, x87 and binary128 through
  `softfloat.rs`; `__builtin_isfpclass` is the `MASK` class: the
  value's LLVM `FPClassTest` bit (sign and signaling distinguished) ANDed
  with the mask register, the mask evaluated first.
  `__builtin_expect_with_probability` is its first operand, as `expect`.
- **Strings and memory** (`bi::STRLEN`, `MEMCMP`, `MEMCHR`, `MEMCPY`, each
  with a `WIDE` or `MOVE` sub-kind): clang's typed semantics. `strlen`,
  `memcmp` and `memchr` read element by element and fail with clang's
  one-past-the-end / uninitialized notes; `memcmp` and `memchr` on
  non-character element types report `note_constexpr_memcmp_unsupported`
  / `memchr_unsupported`. `memcpy`/`memmove` need both pointers to
  designate arrays of the same trivially copyable type with enough
  elements left and a size that is a multiple of the element size
  (`note_constexpr_memcpy_*`: null, type pun, non-trivial, overlap for
  `memcpy`, unsupported); `memmove` handles overlap in either direction.
  The type check compares array types element-wise without qualifiers
  (`Types::same_unqualified`), and a pointer that designates a whole
  array (sema's implicit copy assignment of an array member passes
  `&this->m`) copies the array's elements (WP-201).
  The fast path compares, scans or moves bytes directly when the
  elements are one byte (or, for `memcpy`/`memmove`, scalars) and no
  union is involved; any irregularity falls back to the element-wise
  loop, which reports the exact fault.
  **Landed (WP-140):** `strcmp`/`strncmp`/`wcscmp`/`wcsncmp` are
  `MEMCMP` and `strchr`/`wcschr` are `MEMCHR` with the `STR` sub-kind
  (stop at the terminator, no character-element restriction: clang 19's
  `StopAtNull`) and, for the unbounded forms, `UNBOUNDED` (no count).
  Narrow elements compare as unsigned, wide ones in `wchar_t`'s
  signedness; `strchr` of a value not representable in the element type
  is null. `strrchr`, `strpbrk` and `strstr` trap ("subexpression not
  valid"), as clang 19, which does not constant-evaluate them.
- **`__builtin_bit_cast`** (`bi::BIT_CAST`; `compile/bitcast.rs`,
  `vm/bitcast.rs`): clang's `checkBitCastConstexprEligibilityType` runs at
  compile time (unions, pointers, member pointers, `volatile` and
  reference members are rejected with clang's subobject notes; so are a
  source bit-field and classes with bases). The VM then reads the source
  value representation into a byte buffer through the `kiln_abi` layout
  (padding and uninitialized scalars stay indeterminate) and builds the
  destination scalar by scalar: an indeterminate byte may only initialize
  `unsigned char`, unsigned plain `char` or `std::byte`, a `bool` byte
  must be 0 or 1. Dense, fully initialized types copy all bytes at once.
  `TO_MEM` builds an aggregate result in place; `READ_ONLY` only reads
  the source (clang reads before its type checks).
- **Temporaries:** a `MaterializeTemporaryExpr` with full-expression
  storage (a `bit_cast` or builtin operand bound to a reference
  parameter) is an lvalue in the Oven, its object living until the end of
  the full-expression (the general rule of "Classes", Temporaries; a
  temporary of const-qualified non-class type is a const object).
- **Source location** (`ExprKind::SourceLocExpr`): sema has computed the
  values (spec/ast.md §5.8). `LINE`/`COLUMN` compile to constants; the
  pointer kinds compile to one `Op::Builtin` with `bi::THAW_PTR`, whose
  immediates are the frozen pointer (`ConstValueId`) and its type; the VM
  thaws it (`thaw_ptr`, memoized per value), so every evaluation sees the
  same string and `__impl` objects. Inside a `CXXDefaultArgExpr` whose
  call site sema recorded, the outermost such default argument supplies
  the values.
- **Landed (WP-150): GCC integer and remaining memory builtins**
  (`builtin::INT_MEM`). Folding status, as clang 19:

  | builtin | Oven | evaluation |
  |---|---|---|
  | `ffs`, `ffsl`, `ffsll` | yes | `bi::INT_OP`/`FFS`: 0 for 0, else trailing zeros + 1 |
  | `parity`, `parityl`, `parityll` | yes | `INT_OP`/`PARITY`: popcount mod 2 |
  | `clrsb`, `clrsbl`, `clrsbll` | yes | `INT_OP`/`CLRSB`: redundant sign bits (width − significant bits) |
  | `bitreverse8`..`64` | yes | `INT_OP`/`BITREVERSE` |
  | `rotateleft8`..`64`, `rotateright8`..`64` | yes | `INT_OP`/`ROTL`/`ROTR`: amount (unsigned) modulo the width |
  | `bcmp` | yes | `MEMCMP` sub 0 (as `memcmp`; the unsupported note names `'__builtin_bcmp'`) |
  | `wmemcpy`, `wmemmove` | yes | `MEMCPY` with `WCHAR` (and `MOVE`): the count is in elements, no size-multiple check; notes say `'wmemcpy'`/`'wmemmove'` |
  | `memset`, `mempcpy` | no | trap "subexpression not valid in a constant expression" (clang 19 rejects both; WP-161 left them unevaluated, since an evaluating extension would need a `-fkiln-` opt-in) |
  | `abs`, `labs`, `llabs` (WP-144) | no | trap, as above |

  The notes take the builtin's name from the call's callee
  (`notes.rs` `called_builtin`). Codegen follows clang `EmitBuiltinExpr`:
  `ffs` is `cttz(x, true) + 1` selected against zero, `parity` is
  `ctpop(x) & 1`, `clrsb` is `ctlz(x < 0 ? ~x : x, false) - 1`,
  `bitreverse` is `llvm.bitreverse`, the rotates are `llvm.fshl`/`fshr`
  of `(x, x, n)`, `memset` is `llvm.memset` of the truncated byte with
  the destination's known alignment, `mempcpy` is `llvm.memcpy` plus an
  `i8` GEP by the count; `bcmp`, `wmemcpy` and `wmemmove` call the
  library functions.

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
("Classes"); `std::allocator<T>::allocate`/`deallocate`,
`__builtin_operator_new`/`_delete` inside them, `std::construct_at`/
`destroy_at` and explicit destructor calls landed with WP-212 ("`std::allocator`
and object lifetime" below).

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
The `operator new`/`operator delete` calls (or `__builtin_operator_new`/
`_delete`) inside `std::allocator<T>::allocate(n)` and `deallocate(p, n)`
lower to the same two ops: `New` with aux `ARRAY | STD` and `ty = T`
(clang's rule: the allocation is an array of `n` `T`), `Delete` with aux 8
(`STD`); see "`std::allocator` and object lifetime" below (WP-212).

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
    in user code (corrected: the earlier text planned it); it comes with
    the oracle that implements it. Inside a function of namespace `std`
    (`std::construct_at`) the reserved form is constant, as clang 19
    allows it there ([expr.const]/5.18; WP-212, below);
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

**`std::allocator` and object lifetime — landed (WP-212).** The
transient-allocation rules of [expr.const]/5.18–5.20 and
[allocator.members], with clang 19's notes, each verified against the
oracle (`tests/conformance/expr.new/constexpr-std-allocator.cpp`,
`tests/conformance/expr.delete/constexpr-std-deallocate.cpp`,
`tests/conformance/basic.life/constexpr-construct-destroy.cpp`, and with
libc++ `tests/conformance/expr.new/libcxx-constexpr-*.cpp`).

- *Which calls allocate.* A call of a replaceable global
  `operator new`/`operator new[]` (written `::operator new(n)` or as
  `__builtin_operator_new(n)`) is constant only when the function being
  evaluated is `allocate` of a specialization of the `std::allocator`
  primary template whose first template argument is a type (clang's
  `getStdAllocatorCaller`; `new_delete::std_allocator_elem`), and likewise
  `operator delete` only in `deallocate`. Anywhere else it is not
  constant: "cannot allocate untyped memory in a constant expression; use
  'std::allocator<T>::allocate' to allocate memory of type 'T'" for a
  new form, "subexpression not valid in a constant expression" for a
  delete form (both at the call). The caller test is made when the call
  is compiled, from the function being compiled, which is the function
  clang's frame walk finds for every call libc++ makes; a call through a
  function pointer whose target is `operator new` is not recognized (it
  is a non-constexpr function call either way).
- *Allocation.* Inside `allocate` the call lowers to `New` with aux
  `ARRAY | STD`, `ty = T` and `count` = the byte count argument: the
  allocation is a `T[n]` with `n = bytes / sizeof(T)`; a byte count that
  is not a multiple is "allocated size B is not a multiple of size S of
  element type 'T'"; the array-bound checks of `new[]` apply to `n`. The
  allocation is flagged `STD_ALLOC` (`mem::flags`), counted in `allocs`,
  and named `&{*new T[n]#k}` in notes as clang does. Its bytes are
  uninitialized and **outside any object's lifetime**: `Space::life` (a
  per-granule bitmask beside `init`, kept for the heap only once a
  `STD_ALLOC` allocation exists, so programs without the allocator pay
  nothing) records which bytes are within an object's lifetime, and only
  `STD_ALLOC` storage consults it. Reading, writing or calling a member
  function of an object there that has not been constructed (or has been
  destroyed) is "read of object outside its lifetime ..." / "assignment
  to ..." / "member call on object outside its lifetime is not allowed in
  a constant expression".
- *Construction.* A placement `new (p) T(args)` whose allocation function
  is the reserved placement form (`operator new(size_t, void*)`) is
  constant only inside a function of namespace `std` (clang's
  `isStdFunction`: `std::construct_at`, `std::allocator_traits::construct`
  and the library internals) and not as an array new; elsewhere it stays
  "call to placement 'operator new'" (C++26 P2747 is not in clang 19).
  It lowers to `Placement [p, T]`, which checks the storage with the
  access checks minus the lifetime check (null: "construction of
  dereferenced null pointer ..."; deleted, out of bounds) and that a
  `T` object can live at `p`: the most-derived object at that offset of
  the allocation's type must be `T` or contain a `T` subobject there, else
  "placement new would change type of storage from 'U' to 'T'". It then
  begins the lifetime of `sizeof(T)` bytes, marks them uninitialized, and
  the initializer runs as for any `new` (a constructor call for a class).
  Within `std::allocator<T>::allocate` a cast from `void*` to `T*` (as
  libc++ writes `static_cast<T*>(__builtin_operator_new(...))`) is a plain
  move, not the C++26 `void*` cast check.
- *Destruction.* An explicit destructor call `p->~T()` (including the one
  in `std::destroy_at`) lowers to `EndLife [p, T]` aux 1 before the call
  (the object must be within its lifetime: "destruction of object outside
  its lifetime ..."), then the destructor body, then `EndLife` aux 2,
  which ends the lifetime of its bytes and marks them uninitialized. A
  pseudo-destructor call (`p->~int()` on a scalar, through a template)
  is `EndLife` aux 3 (check, then end). Any other member call on an object
  in `STD_ALLOC` storage is preceded by `EndLife` aux 4, which only checks
  that the object is within its lifetime ("member call on ..."). Outside
  `STD_ALLOC` storage the lifetime is tracked from the first explicit
  destruction on ("Object lifetime" below). In an allocation with
  `HAS_UNION`, aux 4 also checks the active members on the path to the
  object (`union_path` with `ak::MEMBER_CALL`: "member call on member 'x'
  of union with active member 'c' ...", clang's `AK_MemberCall`, at the
  member name or the operator); only a union member that reaches the
  object is reported, not one that merely contains its offset, since an
  empty base is not laid out and may share an offset with a union (libc++'s
  `__compressed_pair`) (WP-224,
  `tests/conformance/class.union/constexpr-member-call-inactive.cpp`).
- *Deallocation.* Inside `deallocate`, `operator delete(p, ...)` lowers to
  `Delete` aux 8 (`STD`): `p` must be the start of a live heap allocation
  made by `allocate` ([allocator.members]/9). The checks and notes are
  `delete`'s, plus the allocation-kind check of clang's `DynAlloc::Kind`:
  "'delete' used to delete pointer to object allocated with
  'std::allocator<...>::allocate'" and "'std::allocator<...>::deallocate'
  used to delete pointer to object allocated with 'new'", each with
  "heap allocation performed here"; a null `p` is "'std::allocator<...>::
  deallocate' used to delete a null pointer". The size argument is
  evaluated but not compared with the allocation's (as clang 19).
  Objects still within their lifetime are not diagnosed at deallocation
  (as clang). The block goes back to the size-class lists at once, so
  reallocation-heavy code (`std::vector` growth) peaks at the last two
  buffers.
- *Leaks.* An allocation still live when the evaluation ends is "allocation
  performed here was not deallocated", as for `new` (the note points into
  the library's `allocate`, at the `__builtin_operator_new` call).
- *Libc++ shapes the evaluator relies on.* A trivial copy assignment of a
  union (`__rep = __str.__rep_` in `basic_string`) is a call of the
  union's implicit `operator=`; it copies the object representation of
  the source's active member and makes that member active in the
  destination (clang's `handleTrivialCopy`; `compile/class.rs`
  `method_call` lowers it to `Copy [this, src, U]`). A pointer into an
  array member that shares its offset with a preceding zero-size member
  (`char __padding_[0]` before the short string's `__data_`) designates
  that array (`Types::field_at` skips zero-size members, WP-201, "Classes")
  (`tests/conformance/class.union/constexpr-trivial-assign.cpp`,
  `tests/conformance/expr.add/constexpr-zero-size-member.cpp`).
- *Trivial copies.* A trivial copy/move constructor runs inline as
  `Copy [p, src, T]` aux 1, and a trivial copy/move assignment operator,
  called by either syntax, as `Copy [this, src, T]` (clang's
  `handleTrivialCopy`; no frame). For a class (not a union, whose object
  representation is copied) the instruction carries aux 2: the source is
  read whole, so every subobject of its value must be initialized
  (`first_uninit`, the walk freezing does: bases walked into, arrays by
  their member, unnamed and zero-width bit-fields, padding and inactive
  union members not read). A failure is `Fault::Uninit`: "subobject 'b' is
  not initialized" (or "subobject of type 'T' ..." unnamed) at the source
  argument, then the trivial call's frame note clang has ("in call to
  'S(s)'" at the construct expression, "in call to 't.operator=(s)'" at
  the call), the call stack, and "subobject declared here" (WP-224,
  `tests/conformance/expr.const/constexpr-trivial-copy-uninit.cpp`).
  `Copy` of a class writes its `dsize` bytes (`Types::copy_size`), not
  its `sizeof`: a derived class's members may live in a base's tail
  padding, and copying a base subobject must leave them alone; a
  `memcpy`/`memmove` of class elements copies each the same way (WP-276,
  `tests/conformance/class.copy.assign/trivial-copy-base-tail-padding.cpp`).

**Object lifetime outside `std::allocator` storage — landed (WP-222).**
`EndLife` aux 2 on a frame or `new` allocation not yet tracked makes it
`LIFE`-tracked (`mem::flags::LIFE`, `Memory::set_lifetime`): one
`trial_write` of the allocation, the flag, `Space::life` started for its
space if empty, every byte of the allocation marked within its lifetime,
then the destroyed object's bytes marked outside it. From then on
`Memory::check` consults the bytes as for `STD_ALLOC` storage (the
`flags::TRACKED` mask), so a second explicit destruction, a read, an
assignment or a member call of the destroyed object is diagnosed as there,
and a placement new (`std::construct_at`) re-begins the lifetime. The cost
is one flag test per access for untracked allocations and nothing per
object: the bits are the space's bitmask, released with the frame or the
heap block (static objects are not tracked). The implicit destructions
check the object (`vm/life.rs`), diagnosing "destroying object 'x' whose
lifetime has already ended" ([basic.life]/9) when no byte of it is within
its lifetime: a registered destruction before its destructor runs
(`run_cleanup`, the `Cleanup` records the object's type); the release of
a scope's trivially destructible objects at `ScopeExit` and of a
function's at `Ret` (only once `Memory::life_frame` is set; the callee's
locals in its frame, its parameters in the caller's context, as clang
destroys them there, [expr.call]/6); and `delete`/`delete[]` before the
destructors and the deallocation. Arrays are checked element by element
from the last, so the note names `a[1]` or `{*new S[2]#0}[0]`. A
variable's note is at its declaration, others' at the destroying
expression. A class containing a union is never diagnosed as destroyed
(ending its active member's lifetime, as `std::optional::reset` does,
leaves the union without an active member, which the bytes cannot tell
from a destroyed object), and assigning a union member (`SetActive`)
re-begins the lifetime of the whole union in tracked storage
([class.union.general]/6)
(`tests/conformance/basic.life/constexpr-lifetime-local.cpp`).

*Deviations from clang 19.* A union whose own lifetime ended (`v.~V();
v.b = 2;`) accepts the member assignment, which clang rejects. A class
whose subobjects' lifetimes all ended one by one is diagnosed at its
implicit destruction as if destroyed itself. An explicit call of a virtual destructor ends only the lifetime of
the static type's bytes. A placement new of the wrong type names the
most-derived type at the offset as the "from" type, where clang names the
designated subobject's type. "in call to" notes print every
element of a partly initialized array where clang prints the initialized
ones and its filler, a character as `'\x{3}'` where clang prints `'\x03'`,
and a floating literal in kiln's dump spelling (`3e0` where clang prints
`3.F`).

*Note locations.* A call-site note locates the callee at its begin, as
clang's `getBeginLoc`: a qualified name (`::operator new(n)`,
`std::construct_at(...)`, also when rebuilt by instantiation) at its
qualifier, which sema records in `expr_begin`. An element step out of
range is located at the begin of the subscripted base (`this->b[n]` at
`this`). `tests/conformance/expr.const/note-begin-location.cpp`.

*Memory benchmark.* `bench/oven/vector-alloc.cpp` builds a
`std::vector<int>` of 100 000 elements by `push_back` and sums it: 18
allocations, `peak_bytes` 786 600 (the 2^16- and 2^17-element buffers,
768 KiB, during the last reallocation, plus frames), `peak_arena`
1 048 744; the recorded release baseline (`bench/baselines/oven.json`)
takes 461 ms and 31.7 MB RSS on the 24-CPU aarch64 host.
`bench/oven/string.cpp` (std::string building) now runs on kiln too and
has a kiln baseline (25 allocations, `peak_bytes` 9 320). The gated kiln-mode copy at 10 000 elements is
`tests/conformance/expr.new/libcxx-constexpr-vector-churn.cpp`
(`allocs<=15 peak_bytes<=106496 peak_arena<=163840`).

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

### Trial evaluation — Landed (WP-135)
A *trial* is a part of an evaluation whose effects on the abstract memory
can be undone. `Memory` (`src/mem.rs`, "trial evaluation") keeps a stack of
open checkpoints and one undo log.
- `checkpoint(mode)` opens a checkpoint. It records the undo-log length, the
  frame mark (whose low half is the frame top), the static top, the
  evaluation context (`ctx`) and the heap allocation counter.
- `commit()` closes the innermost checkpoint and keeps its effects. An
  enclosing checkpoint can still undo them, because its log entries are
  older.
- `rollback(types)` closes the innermost checkpoint and undoes its effects.
  It replays the log entries made since the checkpoint, newest first, then
  releases the frame objects made since (`release(mark)`).
- When the last checkpoint closes, either way, the log and its index are
  cleared.

**Fresh objects.** An object is *fresh* for a checkpoint if it was created
after the checkpoint. Writes to fresh objects are never logged, because a
rollback discards those objects anyway.
- A frame object is fresh if its base is at or above the checkpoint's frame
  top.
- A static object is fresh if its base is at or above the checkpoint's
  static top.
- A heap object is fresh if it belongs to a deeper evaluation context, or if
  it belongs to the checkpoint's context and its `Origin::Heap` number is at
  least the checkpoint's allocation counter.

**What is logged.** All entries are made against the innermost checkpoint:
- `Gran`: the 8 bytes and the shadow (init and ptr bits) of each granule of
  a non-fresh object that is about to be written. A granule is logged at
  most once per checkpoint; `seen` maps the granule to the checkpoint's
  serial number.
- `Union`: an active-member entry of a non-fresh union that changes.
- `Flags`: changed allocation flags of a non-fresh object. This covers
  `MakeConst`, and the end of lifetime marked by cleanups.
- `HeapMeta`: on the first heap allocation or deallocation under a
  checkpoint, a snapshot of the heap structure, taken once per checkpoint.
  It holds the top, the live and count counters, the slot count, the
  size-class bins and the free-slot list.
- `HSlot`: a heap slot that is about to be reused or freed. Freeing a
  non-fresh block also logs that block's granules, since the allocator
  reuses its bytes.

A rollback therefore does three things. Objects that existed at the
checkpoint get back their values, shadow, active members, flags and
liveness. Heap blocks allocated since the checkpoint go back to the
allocator. Frame and static objects made since the checkpoint are released.

**The hooks.** Every VM path that modifies existing memory calls
`trial_write(ptr, len)` before writing. These paths are:
- stores and bit-field stores;
- object copies and `bulk_copy`;
- `ZeroInit` and `bit_cast` destinations;
- vptr writes;
- active-member changes.

Union entries and flags change only through `union_put` and `set_flags`.

**Speculative mode.** `TrialMode::Speculative` is clang's speculative
evaluation (`EvaluateBuiltinConstantP`: `SpeculativeEvaluationRAII`, plus
the evaluation-mode checks on side effects). Two kinds of effect fail with
`MemFault::Speculative`, which is a not-constant fault:
- writing a frame or static object that is not fresh for the innermost
  speculative checkpoint;
- `new` or `delete` in the evaluation context of any open speculative
  checkpoint.

Nested evaluations (`ensure_body`, constraints) run in deeper contexts and
may still allocate.

Writes to heap objects that existed before the trial are allowed and
logged. `Op::Speculate` (`__builtin_constant_p`) opens a speculative
checkpoint around its region:
- if the region completes, the checkpoint is committed;
- if the region fails, the checkpoint is rolled back. A not-constant
  failure then answers 0. A resource limit (steps, depth, memory) is still
  rolled back first, and then fails the evaluation.

**Divergence from clang 19.** A failed speculative evaluation that wrote a
heap object keeps that write in clang. In kiln it is rolled back
(`tests/conformance/expr.const/constant-p-trial-rollback.cpp`, which is
skipped for clang). Every other observable result matches clang
(`constant-p-trial.cpp`, which is strict-diags under both compilers).

`TrialMode::Transactional` allows every effect and undoes it on rollback.
It is the general mode for future evaluator trials. No evaluator path uses
it yet; the unit tests exercise it.

**Complexity.**

| Operation | Cost |
|---|---|
| Write outside any trial | One `Vec::is_empty` test. The logging path is out of line (`#[inline(never)]`). |
| `checkpoint`, `commit` | O(1). |
| Write inside a trial | O(granules written) the first time per checkpoint (a hash insert per granule), and a hash lookup after that. A fresh object costs one allocation lookup. |
| Heap op inside a trial | O(bins + free slots) once per checkpoint (`HeapMeta`), O(1) after that. |
| `rollback` | O(log entries since the checkpoint) plus the frame release. |

**Memory bounds.**
- The log holds at most one `Gran` per granule of a non-fresh object
  written per open checkpoint. It also holds one `HeapMeta` per checkpoint
  that touches the heap, and one `HSlot` (a boxed `Alloc`) per heap slot
  reused or freed.
- Writes to fresh objects are never logged. A trial that builds its own
  data costs nothing beyond that data.
- The log, the `seen` index and the checkpoint stack are freed when the
  outermost checkpoint closes, and by `Memory::reset`. They are counted in
  `Memory::footprint`.
- So the memory of N sequential failed trials is O(1) in N.
  `bench/oven/trial-rollback.cpp` runs 20000 failed trials over a helper
  that writes a pre-existing heap buffer, fills a local array and calls
  `new`. Before WP-135, every trial leaked its allocation: the measurement
  was `peak_arena` 20,480,320 and `allocs` 20001, and the evaluation failed
  the leak check. After WP-135 the measurement is `peak_arena` 320, `allocs`
  1 and `frozen_bytes` 32 for any N.

**Statistics.**
- Cumulative counters keep the trial's work: `allocated` bytes, `allocs`,
  steps and calls. The peaks keep their maxima: `peak_bytes`, `peak_arena`
  and depth.
- A rollback lowers the current arena and the live heap bytes. This keeps
  `peak_arena` honest: it is the most memory the Oven held at once,
  including inside trials.
- The diagnostics of a failed trial are discarded. `Op::Speculate` runs its
  region silent, and drops the pending records the region pushed, keeping
  only their peaks.

**Not rolled back.** These effects are all either independent of the trial
or idempotent:
- thawed globals and the provisional-resolution cache, which are reverted at
  the next outermost evaluation;
- the memo cache, which holds only pure calls with scalar parameters and no
  heap operations, so its entries do not depend on context;
- constants interned into the AST store, which are deduplicated by content;
- functions instantiated by `ensure_body`, which are sema effects, as they
  are in clang;
- frame slot generation numbers.

A frame release below an open checkpoint's frame top does not happen: the
trial's region returns to the checkpoint's frame before the checkpoint
closes. Rollback relies on this.

**kiln_sema's speculative sites (audit).** SFINAE `fold_int` in
`templ/args.rs`, constraint satisfaction (`templ/concepts`), noexcept
(`except.rs`), narrowing `try_fold` (`init.rs`), and every other sema call
of `try_fold`, `fold_int`, `eval_const`, `eval_int` or
`is_constant_initialized` share one property. Each is an *outermost* Oven
evaluation, or a nested one under `ensure_body`, so no memory outlives it:
- `eval_outermost` resets `Memory` at its end;
- provisional global resolutions are undone at the next outermost start;
- frozen results are interned.

None of these sites needs a checkpoint. A failed SFINAE or `requires` trial
leaves only an interned constant and the instantiations clang would also
perform. The WP-135 progress note has the site table.

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
it, also through an anonymous union; so does a call of a trivial copy or
move assignment operator on a member access chain, whose object operand
is lowered as a write, clang's `MaybeHandleUnionActiveMemberChange`,
WP-207; only the operator syntax does: `u.m.operator=(x)` is a member call
on `m`, checked as above, WP-224). Nested unions are tracked per union
subobject, so `n.q.x = 1` activates `q` in `n` and `x` in `q`
(`tests/conformance/class.union/constexpr-nested-same-offset.cpp`).
A placement new (`std::construct_at`, `Op::Placement`) walks the unions
on the path to the constructed object (`Vm::place_in_unions`, clang's
`findSubobject` with `AK_Construct`): an inactive member that is the
constructed object itself becomes the active member (the union's bytes
and nested entries reset as by an assignment, [class.union.general]/6);
an inactive member holding it is "construction of subobject of member
'p' of union with {active member 'a'|no active member} is not allowed in
a constant expression" at the new-expression. A pointer is an
allocation and an offset with no designator, so the constructed member
is identified by offset and type. When several members may be it (two
members of one type at one offset, or a member and a subobject of the
active member), the entry keeps one and `Memory::union_alts` the others;
a member access or assignment naming any of them is accepted, and an
assignment makes it the only active member. Deviation: after
`std::construct_at(&s.b, 7)` in `union S { int a; int b; }`, reading
`s.a` is accepted (clang rejects it; xfail
`tests/conformance/class.union/constexpr-placement-same-type.cpp`); a
frozen union records the one kept member
(`tests/conformance/class.union/constexpr-placement-activate.cpp`,
WP-233). The active-member check on the path to an access reaches a
bit-field's storage unit inside a member of class type
(`Types::has_bit_unit`; a bit-field is not a `has_subobject` target),
so libc++'s short-string `__s.__is_long_` reads and writes check against
`__s` (WP-207). `BeginLife`/`EndLife` per subobject
are not separate operations: a member's lifetime is its complete
object's, which is precise for constant evaluation because no constant
expression can end a subobject's lifetime independently except through a
union (tracked) or an explicit destructor call (tracked per byte,
"Transient allocation", WP-212, WP-222).
Looking a member up by offset (`Types::field_at`, `subobject_at`,
`has_member_at`) skips zero-size members (libc++'s `char __padding_[0]`
shares its offset with the next member). Within a union, extents are
computed per member, so a path through a bit-field member of a union
resolves (WP-201). Extents compare element types up to array-element
qualifiers (`Types::same_unqualified`): `p[1][2]` through a `const S&`
designates the member `P[2][3]` although the subscript's element type is
`const P[3]` (WP-201). A function named by a `DeclRefExpr` or a member access
(`t.f` with `f` a static member function) is an lvalue whose address is
`fn_ptr(func_index)`; references to functions evaluate to it (WP-201).

**Static members and function lvalues (WP-207).** `E.m`/`E->m` naming a
static data member evaluates `E` for its side effects only
([expr.ref]/7.1, clang's `VisitIgnoredBaseExpression`; a null `E->m` is
not an access) and designates the variable (`GlobalAddr`; a reference
member designates its referent). An lvalue of function type designates
a function in every lvalue position: a `DeclRefExpr` of a function
reference variable (its referent), `*fp`, and no-op casts and
`ConstantExpr` around either (`static_cast<F&&>(f)` in libc++'s
`__invoke`, [expr.static.cast]/4), whether called, bound to a reference
or under `&`.

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
(`Oven::cleanups`, entries `Cleanup { this, func, pc, depth }`, `depth`
the call-stack depth of the registering frame; kept there rather than as
a mark in `Frame`, which every call pushes and which stays at 32 bytes).
`ScopeExit` runs the cleanups above the height `ScopeEnter` recorded,
`Ret` those of its own depth; both run them last registered first, by entering
the destructor and re-executing themselves on its return, so destruction
is iterative and needs no native recursion. Locals are destroyed in
reverse declaration order at scope exit, `break`/`continue`/`return`
included; temporaries at the end of their full-expression
(`ExprWithCleanups`); members and bases after the destructor body, in
reverse construction order; array elements from the last. An anonymous
union member is not destroyed: its members are variant members
([class.dtor]/13, WP-276). A class
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
`SemaBridge::set_temporary_value(t, v)` (write-once). A pointer to one
whose type needs destruction (`Ast::needs_destruction`) fails the result
with clang's `non-trivial destruction of lifetime-extended temporary with
type 'T' used in the result of a constant expression is not yet
supported` at the temporary (`CheckLValueConstantExpression`; WP-231), so
`const D& r = D{1};` is dynamically initialized. Reading one later
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
one dispatches as `VCall` does, WP-077). `Downcast [dst, p, off, ty]` is
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
The class ops must not tax evaluations that do not use them: the VM
dispatch decodes an opcode by matching its discriminant (no table base
held in a register), `enter` and `load` stay inlined into the run loop,
and `Frame` stays at 32 bytes (a const assertion); together these kept
bench/oven/churn-tree.cpp within 1% of its pre-WP-076 instruction count
(it had grown 10%).

**Left for later (owners).**
- Virtual calls, virtual calls through member pointers, `dynamic_cast`,
  `typeid`, `delete` through a base with a virtual destructor: landed in
  WP-077 (below). Virtual bases still trap with a `kiln:` note.
- `std::construct_at`/`destroy_at`/`std::allocator`: landed in WP-212
  ("Transient allocation").
- Class assignment (`a = b` of class type) is a call of `operator=`
  (corrected: the earlier text said sema rejected it); a union's
  trivial one copies the source's active member (WP-212). Explicit
  destructor calls (`p->~T()`) and `::operator new`/`::operator delete`
  are evaluated (WP-212, "Transient allocation").
  An explicit call of a trivial destructor (WP-215) evaluates the
  object expression and runs nothing but the lifetime bookkeeping
  (WP-222: a later read, or the implicit destruction at scope end, is
  diagnosed as by clang). Inheriting constructors (WP-114) evaluate their
  `CXXInheritedCtorInitExpr` as a call of the base constructor with the
  inheriting constructor's own arguments (`compile/class.rs`
  `inherited_ctor`); notes name a non-constexpr inherited constructor
  (`non-constexpr constructor 'B' ...`) or the base for a constructor
  inherited through a virtual base, as clang.
- Constant destruction of a non-trivially-destructible temporary bound to
  a constexpr reference is not checked (clang 19 rejects it as "not yet
  supported"; kiln accepts it).
- Codegen of `PtrBase::Temporary` (a constexpr reference to a static
  temporary used at run time) is WP-079's ("pointers to lifetime-extended
  temporaries are not supported yet").
- Spec gap: `SubobjectId` (spec/sema-class.md §3.5) is not used by the
  Oven; offsets suffice until virtual bases (WP-077) make the path
  dynamic.

### Dynamic types — Landed (WP-077)

*Status:* implemented in `crates/kiln_oven/src/vm/virt.rs` (dynamic-type
words, dispatch, `dynamic_cast`, `typeid`), `compile/rtti.rs`,
`compile/class.rs` (`SetVptr`, `VCall`, `CovAdj`) and
`compile/new_delete.rs` (virtual destructor `delete`). Contract tests:
`tests/conformance/expr.const/virtual-*.cpp`; bench
`bench/oven/virtual-dispatch.cpp`.

**Dynamic-type word.** Each polymorphic subobject's vptr slot (offset 0
of a class with `TyInfo::has_vptr`; never a field) holds an Oven word:
low 32 bits the `OTy` of the class `C` whose constructor or destructor
last ran on the object, high 32 bits the subobject's byte offset in `C`.
`SetVptr [this, class]` writes the words of every polymorphic subobject
of `class` at `this`: in a constructor after the base initializers and
before the member initializers, in a destructor at entry
([class.cdtor]/4). The words are not part of a frozen value (frozen
records are structural); `Oven::thaw_vptrs` rewrites them for every
complete class object of a thawed value. A read of an uninitialized word
is CWG1517: `Fault::Invalid` ("subexpression not valid") if a
constructor of exactly that object and class is running (its bases
phase), else `Fault::NoDynamic(ak)` ("<verb> object outside its
lifetime", a subobject whose construction has not started). An object
not usable in constant expressions (`MemFault::External`/`Temporary`) is
`Fault::UnknownDynamic` ("... whose dynamic type is not constant").

**Dispatch.** `VCall [method, args, class, adj]` (aux 1: sret) reads the
word at `this`, locates the subobject (`class`, offset) in `C`'s
subobject list (`Virt::poly_info`, built from `ensure_layout` per class),
asks `SemaBridge::final_overrider`, adjusts `this` to the overrider's
subobject and enters it. Resolutions are cached per `(C, offset,
method)` (`Virt::dispatch`); `Virt::footprint` counts the tables into
`peak_bytes`. A pure overrider is `Fault::PureVirtual` (clang's "pure
virtual function 'P::f' called" + "declared here"). A qualified call
(`A::f()`, recorded by sema on the implicit `MemberExpr` as its
qualifier) and a call on an object of known dynamic type stay direct
calls. Covariant returns: `VCall` writes the overrider-to-method return
adjustment (a byte offset of the method's return class within the
overrider's) into register `adj`, and `CovAdj [p, adj]` moves the
returned pointer or reference (null stays null). `MpFn` of a virtual
member function dispatches the same way.

**`dynamic_cast`.** `DynCast [dst, p, static, target]` (target
`u32::MAX`: `void*`; aux 1: reference cast) follows clang's
`HandleDynamicCast`: a null pointer gives null; `void*` gives the most
derived object; otherwise phase 1 walks from the operand's subobject up
through public base edges to an object of the target class, phase 2
looks for a unique public base of the dynamic type. A failure gives null
for a pointer and `Fault::DynCast { kind, .. }` for a reference
(`note_constexpr_dynamic_cast_to_reference_failed`, kinds 0–3: the
static type is a non-public base, no base of the target type, ambiguous,
non-public target).

**`typeid`.** `TypeId [dst, q|p, type_info, class]`: aux 0 names the
canonical, unqualified, non-reference type `q` statically; aux 1 reads
the dynamic type of the polymorphic glvalue `p` (null:
`MemFault::Null` with access kind `TYPEID`). The result is the address
of one static, constant allocation of type `std::type_info` per type
(`Origin::TypeInfo(q)`, cached in `Virt::type_infos`), so `&typeid(T)`
is stable within the translation unit and libc++'s constant-evaluated
`operator==` (`this == &other`) works. Notes print it as `&typeid(T)`.

**`delete` through a base.** A single-object `Delete` whose operand
designates a base subobject (`has_base_subobject` in `ty.rs`, base fields
only) of a heap object of another type is `Fault::DeleteNonVirtual`
unless aux 4 (the static type has a virtual destructor) is set; with it,
the compiled code checks (`Delete` aux 2|4), calls the destructor
virtually (`VCall` on the static type) and frees the complete object
(aux 4). The implicit destructor of the dynamic type
is defined on demand by `ensure_body` (`Sema::define_virtual_dtor`:
clang defines it when the vtable is used).

**Notes.** A subobject destructor run by a destructor's epilogue
(`PushCleanup` aux 2) is noted at that destructor's call, as clang's
`HandleDestructionImpl` passes its call range down.

**Interface changes.** `Fault::{PureVirtual, UnknownDynamic,
NoDynamic, DeleteNonVirtual, DynCast}`; `Origin::TypeInfo`; ops
`SetVptr`, `VCall`, `CovAdj`, `DynCast`, `TypeId`; access kinds
`ak::{MEMBER_CALL, DYNAMIC_CAST, TYPEID, DESTROY}`. Sema (bridge
support only): `member_of_object`/`implicit_member_ref` record a
nested-name-specifier on implicit member accesses; list fillers of
arrays of class type are list-initialized (`Holder h{}` with a `C
cs[2]` member); `ensure_body` defines implicit virtual destructors.

**Left for later (owners).**
- Virtual bases: a class with a virtual base is not a literal type
  ([basic.types.general]/10), so constructing one is never a constant
  expression: `construct` traps first with clang's `CheckLiteralType` note
  `non-literal type 'X' cannot be used in a constant expression`, also
  for an inheriting constructor through a virtual base (WP-146; the
  evaluated variable itself is rejected by sema, sema-class.md §4.10).
  The remaining `VIRTUAL_UNSUPPORTED` traps (a `VirtualBase` ctor-init, a
  class cast through a virtual base) are therefore unreachable from
  valid code, so dispatch,
  `dynamic_cast` and `typeid` never meet one; the subobject list marks
  virtual subobjects and phase 2 treats one as `Fault::Invalid`. Owner:
  a follow-up WP (object layout of virtual bases in `ty.rs`).
- Covariant return adjustment through a pointer to member function:
  `Fault::Invalid` (stub, follow-up of WP-077).
- Freezing a pointer to a `typeid` object (`constexpr auto *p =
  &typeid(T);`) is `NotConstant`: needs a `PtrBase` for type_info objects
  shared with codegen (WP-080, with RTTI emission).
- `r.A::f()` (an explicit member access with a qualifier) is rejected by
  sema ("qualified member access is not supported yet"); only implicit
  `A::f()` is covered.
- A virtual call on a member subobject whose own base is still being
  constructed reports the lifetime note where clang reports CWG1517
  (`no_dynamic` keys on the running constructor's object and class).

### Vectors — Landed (WP-173)
GNU `vector_size` and clang `ext_vector_type` values are stored as an
array of their lanes (`TyKind::Array`, `Types::is_vector`). The array is
padded to the vector's own size and alignment, so an `ext_vector_type(3)`
occupies 4 lanes and the 4th is indeterminate. A vector of `bool` (bits)
stays `Opaque`: it has no value model yet. The operations are in
`compile/vector.rs`. The accept/reject boundary is clang 19's
`VectorExprEvaluator`:
- **Folded:** arithmetic, bitwise, shift and comparison operators, lane
  by lane, with scalars splatted. A comparison yields -1/0 lanes, but a
  true lane of vector `&&`/`||` folds to **1**. This is a clang 19 quirk:
  at run time the lane is -1. Also folded:
  - unary `+ - ~ !`;
  - casts between vectors and lax casts to and from scalars of the same
    size, as bit reinterpretations through `compile/bitcast.rs`;
  - `__builtin_convertvector`, lane by lane;
  - compound assignment;
  - `?:` with a scalar condition;
  - `__builtin_bit_cast` to and from vectors;
  - multi-element compound literals and list-initialization, including
    brace-elided members.
- **Diagnosed per lane:** signed overflow, division by zero and shift
  counts carry the usual notes, with the vector type in the operand text
  (`notes.rs` `vector_suffix`). An uninitialized vector read is
  diagnosed.
- **Rejected with "subexpression not valid in a constant expression"**
  at the expression's begin, as clang 19 does: a subscript `v[i]`,
  `__builtin_reduce_*`, and a bit cast from a vector to a scalar of a
  different class.
- **Rejected without a note** (clang 19 fails silently): a `?:` with a
  vector condition, and ext-vector element access `.x`.
- clang 20+ folds subscripts, reductions and vector-condition `?:`. The
  boundary moves with the oracle at the clang 22 switch (M3 to M4).
- Codegen emits folded vector values as `<N x T>` constants
  (`kiln_codegen/src/consts.rs`).

### Complex values — Landed (WP-211)
A GNU `_Complex T` object is stored as `T[2]` (`ty.rs`): the real part,
then the imaginary part. `compile/complex.rs` computes the operations
clang 19's `ComplexExprEvaluator` folds on the two parts in registers
(`cx_regs`), left operand first; `cx_init` stores a result into its
destination. A complex value from anything else (a call, a list, `?:`,
a statement expression) is materialized and its parts loaded.
- **Folded:** unary `+ - ~` (`~` is the conjugate) and `__extension__`;
  binary `+ - * /`; `==`/`!=` (both parts equal; a NaN part compares
  unequal); every complex cast (real-to-complex, complex-to-complex,
  integral/floating mixes, complex-to-real takes the real part,
  complex-to-bool is "either part non-zero"); `__real`/`__imag` as
  lvalues and rvalues; `__builtin_complex`; list initialization
  `{re, im}`; GNU imaginary constants (`2.0i`: a `+0` real part).
- **Arithmetic (`complex.rs`, `Op::Cx`):** floating `*` and `/` follow C11
  Annex G as clang's `HandleComplexComplexMul`/`Div` do, on `APFloat`
  operators that never fail (NaN and infinity are values). Integer
  operations wrap like `APSInt`; only division by a zero divisor fails.
  A real floating operand stays real (as in clang): it gets a `+0`
  imaginary part for `*`/`/`, and `+`/`-` compute only the parts it
  affects, with the `float` operators that fault on a NaN result or a
  division by zero (`handleFloatFloatBinOp`).
- **`++`/`--`** step the real part only (clang `IncDecSubobjectHandler`);
  overflow notes name the element type.
- **Rejected with the generic note**, as in clang 19: compound
  assignment to a complex object (`CompoundAssignSubobjectHandler`), after
  both operands are evaluated.
- Tests: `tests/conformance/ext.complex/constexpr.cpp`,
  `imaginary.cpp`.

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
    fn opaque_size_align(&mut self, ty: QualType) -> (u64, u32);  // via kiln_abi: size/align of a vector type, whose lanes the Oven lays out inside it (WP-151, WP-173); default (0, 1), so every bridge that folds vectors (sema's, codegen's prepass) must implement it
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
    pub fn fold_rvalue(&mut self, bridge: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId>;
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

Implemented (M3, WP-135): trial evaluation — checkpoint/commit/rollback
over an undo log, with `__builtin_constant_p` as a speculative trial whose
failure is rolled back; see "Trial evaluation — Landed (WP-135)".

Remaining (each traps with a `kiln:` note today):
- Lifetime of objects outside `std::allocator` storage (double
  destruction of a local, reads after an explicit destructor call), and
  the other WP-212 deviations listed under "Transient allocation".
- `delete` through `void*`, and P2747 placement new (see above).
- Front-end gaps that keep some forms from reaching the Oven (not
  Oven work): class-specific `operator new`,
  `delete[] (expr)` (parsed as a lambda) and
  `new char[n]{"hi"}`.
- Volatile objects: reads are diagnosed ([expr.const]/5.9) only where the
  front end puts the volatile qualifier on the read expression's type.

**Landed (WP-140): asm statements.** A `GCCAsmStmt` is never a core
constant expression ([dcl.asm]/1, [expr.const]/5.?): the Oven compiler
lowers it to the same trap as `goto`, giving clang's
`subexpression not valid in a constant expression` note at the `asm`.
A constexpr function that contains asm on a path not taken still
evaluates.

**Landed (WP-140): `PredefinedExpr`.** An lvalue of its `StringLiteral`
child: the same static string object as the literal
(`tests/conformance/dcl.fct.def.general/predefined-constexpr.cpp`).

**Landed (WP-160): `[[assume(e)]]`.** Each assumption that sema did not
mark `ASSUME_IGNORED` is evaluated when reached, inside its own
temporaries scope. If it is false, evaluation fails with clang's
`assumption evaluated to false` note at the assumption's expression
location ([dcl.attr.assume]/1, [expr.const]/5.8). An assumption that has
possible side effects is never evaluated, as in clang's `EvaluateStmt`
(`tests/conformance/expr.const/assume-false.cpp`).

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
  coalescing, top lowering, heap counters and per-context freeing; the
  trial undo log (WP-135): interleaved writes, once-per-checkpoint granule
  logging, allocations and frees rolled back, nested checkpoints, unions
  and flags, the speculative refusals, and a flat footprint over failed
  trials.
- `tests/conformance/expr.const/constant-p-trial*.cpp` (WP-135): a failed
  trial's callee writes, unions and allocations, nested trials, kept
  effects of a successful trial, and (kiln only, `oven:` budget) heap
  rollback and memory O(1) in the number of failed trials.
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
