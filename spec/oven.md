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
  x87 in the low 80 bits; pointers as an encoded `Ptr` in the low 64
  bits, a `void *` with its designator tag, `kbc::VOID_TAG`, the
  designated type + 1, in bits 64..96, kept through memory by the pointer
  mark, "Abstract memory" and "Construction"). Locals whose
  address is never taken and that are not aggregates live in registers;
  the others live in abstract memory.
- Typed, checked arithmetic per width (1..128 bits, `_BitInt`-ready): signed
  overflow, division by zero, `INT_MIN / -1`, out-of-range shifts, negative
  shifts and float results that are NaN fault; unsigned arithmetic wraps.
  `++`/`--` on a signed integer narrower than `int` (`signed char`,
  `short`, a signed plain `char`) adds in `int` and wraps on the
  conversion back, as clang's `UnaryOperator::canOverflow` is false for
  it ([expr.pre.incr]/1 is `x += 1`, [conv.integral]/3): `compile.rs`
  `inc_dec` emits the add at 32 bits and an `ICast` to the narrow width
  (WP-980; `tests/conformance/expr.pre.incr/narrow-signed-wrap-*.cpp`).
  `int`/`long` `++` at the maximum still faults
  (`int-overflow-constexpr.cpp`).
  NaNs that are values (a `__builtin_nan` operand, a quiet library
  result) follow `APFloat`, not the host (WP-410): an operation with a
  NaN operand yields the first NaN operand quieted, sign and payload
  kept; an invalid operation the positive default quiet NaN; a float
  conversion keeps the sign and shifts the payload by the precision
  difference (`softfloat::cast`). `Oven::set_fold_float_ub(true)`
  (codegen's fold prepass only; sema never sets it) lets arithmetic that
  yields a NaN or divides by zero go on with the IEEE result, as clang's
  `ConstantEmitter` folds such an initializer; a call evaluated so is
  never memoized (`tests/conformance/expr.const/nan-payload.cpp`,
  `tests/codegen/nan_global_init.cpp`). `Oven::set_fold_leaks(true)`
  (codegen's fold prepass only, WP-1108) lets a fold outside a constant
  context (`fold_rvalue`, not a constant expression) end with
  allocations still live, as long as its value does not point at one:
  clang's `CheckMemoryLeaks` is only a `CCEDiag` there, so
  `(bool)new int(4)` folds to `true`. Only the outermost evaluation's
  check is relaxed (a call's memoized result still needs every
  allocation freed); [expr.const]/5.20 is unchanged for constant
  expressions and for every evaluation sema runs.
  binary32/binary64 use the host's IEEE arithmetic (round to nearest even);
  x87 `long double`, `__float128` / `_Float128`, `_Float16` and `__bf16`
  use the bit-exact soft-float in `softfloat.rs` (each operation rounded
  once, never through `double`). The P1467 types (WP-361) map to the
  `FloatKind` of their format in `ty.rs`: `_Float32` Single, `_Float64`
  Double, `_Float128` Quad, so they evaluate exactly as `float`,
  `double` and `__float128` (test:
  `tests/conformance/basic.extended.fp/p1467-constexpr.cpp`).
- Compilation is lazy, per function, on first call. A function that cannot
  be called in a constant expression (not constexpr, or no body) compiles
  to a one-instruction stub that traps with clang's note on call, so sema
  need not pre-check. A construct the compiler does not support compiles to
  a trap reading `kiln: constant evaluation of <Kind> is not supported yet`:
  an evaluation fails with a note, never a panic.
- Top-level expressions compile to cached thunks keyed by `ExprId`;
  re-evaluating an expression compiles nothing, except after a failed
  trial fold, whose thunk is reclaimed ("Evaluation entry points").

#### Dispatch — Landed (WP-648)
`Vm::run_loop` (`vm.rs`) executes the stream in safe Rust; every rule here
is an implementation choice that leaves results, diagnostics, step and
fault counts and trap ids unchanged.

- **Fetch window.** Each iteration reads the instruction as one fixed
  array of `kbc::WINDOW` (8) words: word 0 and up to seven operand words,
  in one checked slice read (`first_chunk`), so the operands are then
  indexed with no further bounds check. `run_loop` borrows the window in
  place (`kbc::window_ref(code, pc, &mut tail) -> &[u32; WINDOW]`) rather
  than copying it: a copied window is spilled to the stack and every
  operand read goes through a store-to-load forward, which cost several
  percent of wall time on loop-heavy programs although it lowers the
  instruction count. A `const` assertion keeps every opcode's operand
  count (`Op::OPERANDS`) below `WINDOW`.
- **Padding.** The stream ends with `WINDOW - 1` `Nop` words (`kbc::pad`,
  called by `Compiler::end_unit` and `Vm::unwind_pc`), so a window starting
  at any instruction lies inside the stream. Only the last unit carries
  them: the next unit's first word (`Compiler::here`, `Vm::unwind_pc`)
  first truncates them (`kbc::unpad`, keyed on `Oven::padded`, the stream
  length the last `pad` left), because a unit's own last instruction then
  has the next unit's words after it. The padding is never executed (a
  unit ends in `Ret`, `Halt`, a trap or `Unwind`) and never a jump target;
  it costs 28 bytes in all. A window that would still run past the end
  goes to the cold `kbc::tail_window`, which copies the instruction's own
  words into `tail` and zero-fills the rest (a truncated instruction
  panics there, as an out-of-range read did before).
- **Hot and cold arms.** Opcodes that are rare in measured programs
  (allocation, unions, member pointers, RTTI, exceptions, `Wide`, `Cx`,
  `VoidCast`, ...) run in `Vm::run_cold`, an `#[inline(never)]` function
  taking the opcode, `pc` and the frame base and returning the next `pc`
  and frame base (or the fault and its `pc`). It fetches the window again
  itself, so `run_loop`'s window never has its address taken. `run_loop`
  names those opcodes in one arm rather than a wildcard, so its `match` is
  a single jump table, and keeps the common opcodes in a body small enough
  to stay in registers.
- **In-place fusion.** A hot arm whose window also holds the next
  instruction runs that instruction too when it is the usual successor,
  skipping one dispatch: `Mov`→`Mov`, `Step`→`Mov`, `Step`→`Const`,
  `JmpIfNot`→`Step` (fall-through) and `ICmp`→`JmpIf`/`JmpIfNot`. The
  encoding is unchanged: the second instruction is still in the stream,
  may still be a jump target, and is executed with its own semantics; a
  step fault in a fused `Step` is reported at that `Step`'s own `pc`. The
  `op-stats` feature counts a fused instruction as executed, so its
  histograms match an unfused run.
- **Pointer steps.** `PtrAdd`'s aux carries, besides the index's integer
  aux and the negate bit 9, an element-step field (bits 10..=15,
  `ptr_add_aux`) the compiler fills from the element type: `k + 1` when
  the element's size is `1 << k` and the type is not `Opaque` (an
  incomplete class, whose size may still change), `0` otherwise and for
  an unsigned 128-bit index (its value may exceed `i128`). With a
  nonzero field the run loop first tries, inline, a step inside an
  array allocation of exactly the operand's element type: the current
  index is the offset shifted right by `k`, and a new index in `0..=n`
  gives the pointer (past-the-end at `n`); a zero index returns the
  pointer unchanged. Otherwise, with a nonzero field, `ptr_add_member`
  (out of line, not cold) tries `ptr_add_front`: a step inside an array
  member the type table's whole-array extent table already holds
  (`Types::extent_hit`, "Path cache" below; an `inplace_vector`'s
  `union { T data[N]; }`), the new index in `0..=n` (WP-760).
  Everything else runs `ptr_add`, which tries the same two steps with
  the size read from the type table and otherwise steps within the
  extent of the subobject the pointer lies in, as before; every result
  and every fault is the one that general path gives.

### Abstract memory — normative for M2
- **Pointers** are one `u64`: offset (32 bits) | past-the-end (1) | slot
  (22) | generation (9). Slot `FN_SLOT` encodes function pointers; slots
  with bit 21 (`STATIC_BIT`) set are in the static space. Null is 0.
- **Allocations** (`Alloc`): base and size in their space, `OTy` type,
  flags (`CONST`, `EXTERNAL` — a non-usable variable stood in for,
  `HAS_UNION`, `RESOLVING` — a global whose initializer is running), the
  `Origin` (local `Var`, `Temp`, `Global`, `External`, `Str(literal, expr)`,
  thawed `Const`) for notes, and the frame serial that created it.
  An `External` object (other than a reference's pointer slot) and an
  `Unknown` one (P2280, "Unknown references") are *unbacked*: a static
  slot with a size, for address arithmetic and bounds, and the base
  `mem::UNBACKED`, holding no bytes, since every access of them is
  diagnosed before the base is used (`Memory::alloc_unbacked`, WP-375).
  Naming an `extern` array or a reference to an array of any size costs
  one slot. Their size is clamped to 4 GiB (pointer offsets are 32 bits).
- **Spaces**: the *frame* space is a bump arena of all frame objects of the
  current evaluation; the *static* space holds globals and thawed constants
  and persists for the TU. Each space keeps the bytes, a per-byte
  initialized bitmap and a per-8-byte-granule pointer mark byte (0: no
  pointer; 1: a pointer; `mem::PTR_TAGGED` (2) and up: a `void *` and
  its designator tag, below and "Construction"); unions
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
  Constructing such a subobject touches none of its bytes; a complete
  object or an array element of such a type owns its bytes, so its
  construction begins their lifetime and its destruction ends it (a second
  destruction is "destruction of object outside its lifetime";
  `vm.rs` `placement`, `basic.life/empty-object-lifetime.cpp`; WP-404).
  Placement of such an object revives the lifetime of all of its type's
  bytes, so an empty member destroyed and re-created with
  `std::construct_at` is alive again
  (`basic.life/empty-member-recreated.cpp`; WP-418).
- **Lifetime**: leaving a scope releases its objects (`release(mark)`); a
  released slot is reused first-in first-out with its generation bumped, so
  a dangling pointer is detected (generation mismatch) and still names its
  origin for as long as possible. A dead slot is reused only once more
  than `FRAME_QUARANTINE` (16) dead slots wait, so a pointer into a frame
  just returned from or unwound (whose slot the next frame object would
  otherwise take) still reads as "read of variable whose lifetime has
  ended" + declared here, as clang reports it, not as a stale slot.
  Slots whose generation would reach the last one (`mem::NOWHERE`,
  never issued) are retired.
- **Static reclamation** (WP-322, WP-060): a static object that dies
  during compilation (the scratch copy `Oven::has_constant_destruction`
  destroys) is freed by `Memory::free_static`, wherever it lies, not only
  when it is the most recent static: its slot is dead and is reused with
  its generation bumped (a dangling pointer faults as stale, and memo keys
  holding it never match the new object), and its range joins a sorted,
  coalesced free list that `Memory::alloc_static` allocates from first-fit
  before growing the space; a free range that reaches the top lowers it.
  So the static space held does not grow with the number of dead statics
  (`static_bytes`, "Reporting protocol"). Inside a trial ("Trial
  evaluation") free ranges and slots are not reused, and the top is never
  lowered below the top the innermost checkpoint saw, so objects above it
  stay fresh to the trial. Thawed globals, placeholders of running
  initializers and external stand-ins are kept for the TU (memo entries
  and resolved globals may hold pointers to them).
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
  A local is kept in a register only when the compile-time pre-scan
  (`Scan`) sees no address of it; a glvalue cast that drops `const`
  (`const_cast<int&>(c) = 2`, `(int&)c`) counts as one, so the write
  reaches the const allocation and faults (WP-465;
  `expr.const/5.8/modify-const-cast-away.cpp`). Const is a flag of the
  allocation: a const member of a non-const object written through a
  cast (`*(int*)&s.m = 3`) is not detected yet.
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
  19's, byte for byte, or clang 22's at `-fclang-compat=22`: the level is
  an Oven option (`Oven::set_clang_compat`, set by sema from its own
  level before every evaluation), never a global (WP-377). The level-22
  rewordings are listed in the "Compat-22 note wording" rows below.
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
  (WP-237; `kiln_ast::consts::fltstr`, shared with sema's expression
  printing since WP-410). A float argument in an "in call to" note is
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
- A pointer argument's path through an unnamed member (an anonymous
  union or struct) names that member as clang does:
  `&v.V::(anonymous union at FILE:L:C).data[1]`, the class qualified and
  the location the member's type's (`notes::ptr_string`,
  `write_unnamed_member`). The Oven knows only raw locations, so the
  location is written as a marked raw `Loc` and resolved to its presumed
  `file:line:col` when the note is rendered (`SemaBridge::sources`; with
  no source manager the mark is dropped). Before WP-773 the member was an
  empty name (`&v..data[1]`)
  (`tests/conformance/expr.const/oven-note-unnamed-member-path.cpp`).
  Deviation: for an anonymous union inside a member of unnamed struct
  type (`v.s.data` with `struct { union { int data[4]; }; } s;`) clang
  19 prints `v.s.union (anonymous union at ...)`; kiln prints the
  qualified form there too.
- Compat-22 note wording (WP-377; each 19 row stays in force below 22):
  | level-19 note | level-22 note |
  |---|---|
  | "read of dereferenced null pointer ..." at the access, for `*p` with `p` null | "dereferencing a null pointer is not allowed in a constant expression" at the `*` (the lvalue `*p` is checked when formed, `Fault::NullDeref`) |
  | `(*p).m` with `p` null: "read of dereferenced null pointer" | "dereferencing a null pointer" at the `*` |
  | `p->*pm` with `p` null and a data member: "read of/assignment to dereferenced null pointer" | "cannot access field of null pointer" at the operator |
  | `p - q` across objects: "subexpression not valid in a constant expression" | "arithmetic involving unrelated objects 'P' and 'Q' has unspecified value" (`Fault::PtrArith`) |
  | compare across objects: "comparison between 'P' and 'Q' has unspecified value" | "comparison between pointers to unrelated objects 'P' and 'Q' has unspecified value" |
  | literal-address equality: "comparison of addresses of literals has unspecified value" | "comparison of addresses of potentially overlapping literals has unspecified value" |
  Corpus: `tests/conformance/expr.const/5.8/*null*`, `ptr-sub-*`,
  `5.24/relational-unrelated.cpp`, `5.24/string-literals.cpp`,
  `expr.mptr.oper/constexpr-faults.cpp`.
- `-fconstexpr-backtrace-limit=N` (`Oven::set_backtrace_limit`) sets the
  frame limit; 0 shows every frame.
- `try_fold`, `is_constant_initialized` and `fold_constant_init` never
  render notes or warnings (`silent`).
- An out-of-range conversion to an unscoped enumeration without a fixed
  underlying type ([expr.static.cast]/10) is accepted with the value, and
  reported only where clang 19 reports it ("Integration"). At
  `-fclang-compat=22` it is a fault in every non-silent evaluation
  (`Fault::EnumRange`, clang 22's CCEDiag): "integer value V is outside
  the valid range of values [MIN, MAX] for the enumeration type 'E'", so
  the expression is not a constant expression (WP-394;
  `expr.const/5.8/enum-range-clang22.cpp`).
- A local variable of non-literal type in a constexpr function
  ([dcl.constexpr]/3 before P2448R2) is a fault at level 19 in every mode;
  at level 22 in C++23 and later the check is skipped, as clang 22
  (P2448R2), and evaluation fails later on the non-constexpr constructor
  or destructor call instead (WP-394; `expr.const/non-literal-local-clang22.cpp`).

### Evaluation entry points — normative for M2
- `Oven::evaluate(bridge, e, EvalContext) -> Result<ConstValueId,
  EvalFailure>`: a manifestly constant-evaluated context; recorded in the
  stats and the report. `EvalContext::init(var, loc)` marks a variable's
  initializer (its object is under construction, `RESOLVING`).
  - **A variable's own name in its initializer** (WP-1100): while a class
    or array variable's initializer runs into its object (`eval_at` with
    an object: a static, constexpr or constexpr-local variable, through
    `eval_var_init`), the name designates that object under construction
    ([basic.life]/7, [class.cdtor]/2; clang's `EvaluatingDecl`), not the
    static placeholder. The thunk emits `Op::BindObject [p, global]`
    before it initializes its frame allocation `p` (`Origin::Var(d)`):
    the global's state becomes `GState::Bound(p)`, and `Op::GlobalAddr`
    of a bound global yields `p` and bumps `run.impure`, so no call that
    names the object is memoized across its changes. `eval_at` restores
    the previous state when the run ends (`Oven::unbind_object`), and a
    nested evaluation (`run_job`, another variable's initializer) lifts
    the binding while it runs (`Oven::unbound`), so only the initializer's
    own run sees it. Freezing maps `p` to `PtrBase::Decl` as before;
    thawing a `Decl` of a bound global reads `p`. An element not yet
    initialized has no value, so reading it through the name is "read of
    object outside its lifetime" (`notes.rs` `unbegun_init_object`:
    uninitialized bytes of `run.object`'s allocation while no constructor
    or destructor of an object in it is on the stack; inside one, a
    default-initialized member stays "uninitialized"), as clang, where
    the subobject is absent. A scalar variable's self-reference still
    reads the `RESOLVING` placeholder. Corpus:
    `expr.const/initializer-reads-own-member{,-local,-calls}.cpp`,
    `expr.const/initializer-reads-unbegun-member.cpp`.
- `Oven::try_fold(bridge, e) -> Option<ConstValueId>`: silent, not
  recorded.
  - **Trial-fold fast path** (WP-656, `leaf.rs`): an outermost
    `try_fold` or `fold_rvalue` first walks `e` directly, with no thunk
    compile, no VM run and no context set-up. Its domain is a prvalue
    tree whose every node has `bool` or integer type of at most 128 bits
    and is an integer, character or `bool` literal, `noexcept`, a type
    or expression trait, an enumerator, `sizeof` of a type or an
    expression (not a VLA), `alignof` of a type (not through a typedef
    with attributes), a parenthesis, a substituted non-type template
    parameter, a `ConstantExpr`, an integral, boolean or no-op cast (not to an
    enumeration), a unary `+ - ~ !`, a binary arithmetic, shift,
    comparison or logical operator, `?:`, or a variable read (below),
    at most 64 deep. It
    declines (runs the full path) outside that domain, on any failing
    operation (overflow, `/ 0`, a bad shift), when either arm of `?:`,
    `&&` or `||` does not walk, inside a running evaluation, and when a
    one-shot setting is pending (`-Wconstant-evaluated`, the overflow
    check, a potential constant expression, a plain, `void` or address
    result, a `source_location` root).
  - **Variable reads** (WP-663): an lvalue-to-rvalue conversion of a
    `DeclRefExpr` (through parentheses) to a variable or parameter whose
    full-path `GlobalAddr`, `Load` either reads a known value or
    certainly faults. A provisional resolution counts as unresolved
    (`begin_outermost` forgets it first). The walk answers in two cases
    and declines otherwise:
    - *a value*: the variable is usable in constant expressions
      (`value::var_info`), not a reference, not `volatile`, of the
      conversion's type kind, its initializer's value is known
      (`VarEvalState::Value`) and its global is resolved to a `CONST`,
      `Origin::Global` object of its type that is not `EXTERNAL`,
      `RESOLVING`, `TRACKED` or lax-only (`LAX_READ`). The `Load` reads
      that value in a strict and a lax fold alike.
    - *a definite failure* (the fold's answer is "not a constant"): a
      function parameter or automatic variable (the walk runs only
      outermost, so outside its frame) that is neither usable nor laxly
      readable, whose global is resolved to an `EXTERNAL`,
      `Origin::External` object that is not lax-only, or is unresolved
      (or has no entry yet) and is not a reference: `resolve_global`
      then resolves it to such an object (`resolve_var_in` makes an
      external one for a variable neither usable nor lax, or reuses the
      provisional one it made before). The `Load` faults
      (`MemFault::External`) in both modes. A failure propagates
      through every strict operator; `&&`, `||` and `?:` take it only
      from the operand they run (the other arm must still walk, since
      the full path compiles it), and the first failure in run order
      (left to right) is reported before the operator's own checks, as
      the full path's run faults on that `Load` first.
    - *the full path's leftovers*, redone when the walk answers
      (`Oven::leaf_effects`), in its order: provisional resolutions are
      forgotten (`begin_outermost`); the thunk compile's global entries
      are created for the walked variables that have none, in compile
      order, which is the walk's (every operand, run or not, left to
      right; `?:` its condition and then both arms); the global of the
      read that faults first is resolved (`resolve_global`, as
      `Op::GlobalAddr` does); and the static peak is noted
      (`end_outermost`).
    - *member reads*: the read may instead be of a `.` member access
      (nested, through parentheses) whose innermost base is such a
      `DeclRefExpr`, where every member is a non-static, non-reference,
      non-bit-field data member of a non-union class (each compiles to
      `Op::Field`, which faults only on a null or past-the-end address,
      neither of which an object's address is). Only the definite
      failure answers: the base variable's external object faults at
      the member's `Load` as at its own; a usable base declines. `->`,
      a union, a bit-field and a reference member decline.
    - *scalar glvalue roots*: a fold of a glvalue `DeclRefExpr` or
      `MemberExpr` of `bool` or integer type (codegen's fold of a member
      access, `compile_thunk`'s `lv` then `load`) is the read its
      lvalue-to-rvalue conversion would be, and is walked as one, except
      for `is_constant_initialized`'s walk (an object is placed).
  - **Equivalence rule:** where the walk answers, its value is the one
    the full path would freeze (it computes each node with the same
    `crate::arith` operation on the same register value and freezes with
    `freeze_scalar`), and skipping the full path changes nothing else
    observable: diagnostics, statistics (`static_bytes` included),
    `-fkiln-cfg-stats` and the emitted IR stay byte-identical. A definite
    failure is the full path's `None`, and the global entries, their
    resolutions and the static space are left as the full path leaves
    them. The full path stays callable (`try_fold_full`, test-only) and
    kiln_sema's `tests/trial_fold.rs` compares the two, answers and
    state (`Oven::globals_digest`), against a second Sema that runs the
    full path only; the `leaf-check` feature asserts the equivalence of
    the answers on every walk of a build.
  - **Failed trial folds give their thunk back** (WP-798,
    `reclaim.rs`): when an outermost `try_fold`, `fold_rvalue` or
    `fold_for_overflow` that took the full path fails, and its run
    compiled a fresh thunk and nothing else (no callee stub, no other
    unit) and appended nothing at run time (no unwind block, no
    reflection trap), the code, site, step-location, origin, trap,
    function and reflection-site tables are cut back to their lengths
    before the fold (the padding of the unit before it restored) and the
    thunk cache forgets the expression; a later fold of it compiles it
    again. `fold_for_overflow` maps its overflows to sites first. A
    successful fold, an evaluation and a nested fold keep their thunks.
    Answers, diagnostics and statistics are unchanged; `oven.code`,
    `oven.sites`, `oven.funcs` and `oven.thunks` shrink.
- `Oven::evaluate_chars(bridge, size, data, EvalContext) -> (Vec<u8>,
  Result<(), EvalFailure>)` (WP-356, [dcl.pre]/11-12, P2741; clang
  `Expr::EvaluateCharRangeAsString`): the characters of a `static_assert`
  message that is not a string literal. `size` (converted to
  `std::size_t`) and `data` (converted to `const char *`) are evaluated
  in that order in one manifestly constant-evaluated context, then the
  builtin `bi::MSG_CHARS` reads the `size` characters `data` points to
  into the Oven's message buffer; temporaries of both calls live until
  the last read. Recorded in the stats like `evaluate`, never memoized.
  On failure the characters read before the failing one are returned
  with the failure and its notes (clang prints them). spec/sema.md §4.6
  renders the text.
- `Oven::evaluate_with_params(bridge, cond, binds, at) ->
  Option<ConstValueId>` (WP-460, clang `Expr::EvaluateWithSubstitution`,
  for the `enable_if` attribute, spec/sema.md §4.5): evaluates `cond`
  (`None`: only the bindings, giving `true`) in a fresh function whose
  locals are the parameters `binds` names, each initialized from its
  argument expression (a reference parameter binds to it, an object
  parameter is a local variable copy-initialized from it). A parameter
  not bound is not a constant expression when read. Manifestly
  constant-evaluated, silent, not recorded, never cached; fails if the
  evaluation leaks an allocation. `at` locates the evaluation for the
  depth check.
- `Oven::evaluate_call(bridge, f, at) -> Result<(), EvalFailure>`
  (WP-468, clang `Expr::isPotentialConstantExpr` for a function without
  parameters, spec/sema.md §5.28 "Never constant"): evaluates a call of
  `f` without arguments in a stub (`compile_call`, the call's site at
  `at`), manifestly constant-evaluated, with notes; the result, if any,
  is frozen and dropped. The failure's last note is the call's `in call
  to`. A function with parameters, a variadic one, or one returning a
  reference or an aggregate is not evaluated (`Ok`), nor is anything
  inside another evaluation. Not recorded in the stats; a success is
  memoized as any call (`Func::memo`), so a later use of the function
  costs a lookup.
- Potential-constant frames (WP-1150, WP-1163, `frame.rs`, clang
  `Expr::isPotentialConstantExpr`'s call frame, spec/sema.md "Never
  constant"): the never-constant walk keeps the values of a body's local
  variables while its parameters stay unknown.
  - `Oven::frame_local(bridge, d, init, frame) ->
    Result<Option<FrameObject>, EvalFailure>` allocates local `d`'s
    object as a writable static (`Origin::Var(d)`, so it survives the
    per-evaluation reset) and initializes it from `init` (`None`: left
    uninitialized). `Ok(None)` means `d` is not tracked: a reference, a
    volatile object, a non-literal type or one needing destruction, a
    pointer to member, a type with a virtual pointer, union, `mutable` or
    volatile subobject, or past 64 KiB of tracked objects. A pointer (or
    a class or array holding one) is tracked (WP-1163). A failed
    initialization frees the object and returns its failure.
  - `Oven::evaluate_in_frame(bridge, e, frame, FrameEval) ->
    Result<ConstValueId, EvalFailure>` evaluates the full-expression `e`
    for its scalar value (`Value`) or its side effects (`Discard`, giving
    `true`), and a modification of a tracked object stays.
  - `Oven::evaluate_loop_in_frame(bridge, s, at, frame, steps) ->
    Result<u64, EvalFailure>` (WP-1163) runs the loop statement `s` once
    to its end (`at` its location), with `steps` (capped at
    `-fconstexpr-steps`) as its step budget, and gives the steps it used
    (one per statement, as clang's `nextStep`); a failure (the step
    limit's included) restores every tracked object's bytes,
    initialization and pointer marks as they were before it.
  - After each evaluation (`frame_escapes`), a tracked object holding a
    pointer that would dangle once the evaluation ends (one into the
    evaluation's own frame or heap space; null, a function pointer and a
    pointer to a static object or another tracked object keep) fails the
    evaluation quietly; `frame_local` frees the object.
  - In both, each `(DeclId, FrameObject)` of `frame` designates that
    object when its variable is named. Reading any other variable (a
    parameter, an untracked local) or `this`, or an object whose
    lifetime ended in an earlier evaluation (a freed tracked object, a
    temporary), fails quietly (clang's potential mode knows no value
    there, or does not report it). The evaluation is manifestly
    constant-evaluated, with notes, and calls fail quietly as in a
    potential evaluation; its result is not checked as a constant result
    (clang does not in potential mode, so a pointer to the heap or to a
    temporary as a value is not a failure). It is not recorded, and fails
    if it leaks an allocation. Inside another evaluation, nothing is tracked or
    evaluated.
  - `Oven::frame_free(o)` frees a tracked object (its scope ended),
    `Oven::frame_snapshot(frame)` gives its objects' bytes and
    initialization (the walk's repeating-iteration test), and `Oven::frame_end()`
    forgets the stubs compiled for the walk, which are cached by
    expression, evaluation and frame variables (`compile_frame`).
- `Oven::is_constant_initialized(bridge, var) -> bool`
  ([basic.start.static]/2): silent, not recorded; uses the variable's
  cached evaluation state when known.
  - **Fast path** (WP-663, `Oven::leaf_init`): an initializer of a
    non-reference variable that the trial-fold walk answers (above) is
    answered by it: the full path compiles it as the variable's
    initializer, which for a scalar prvalue in the walk's domain is the
    fold's code (no extended temporary is in the domain), and no walked
    node depends on the manifest context. What the full path leaves
    behind is redone in its order: provisional resolutions are
    forgotten, the placeholder of a non-local variable is placed
    (`eval_inner`, which `static_bytes` counts), the walked reads' global
    entries are made and the first faulting read's global resolved, the
    placeholder's global is left unresolved again, and the static peak
    is noted; so the globals, the static space and `static_bytes` are
    the full path's. The full path stays callable
    (`is_constant_initialized_full`, test-only); `tests/trial_fold.rs`
    compares answers and state against a full-only Sema.
- A `const` integral or enumeration variable is usable in constant
  expressions ([expr.const]/4) when its initializer folds; its value is
  evaluated on demand when first read (nested initializations are capped
  at 512 deep and then read as unknown).
- `Oven::fold_constant_init(bridge, e, var) -> Option<ConstValueId>`
  (WP-063): folds `var`'s initializer `e` as a constant initializer
  ([basic.start.static]/2): manifestly constant-evaluated, but silent and
  not recorded, like `try_fold`. Codegen uses it for the initializers it
  emits as constants (spec/codegen.md).
- `Oven::has_constant_destruction(bridge, var, value, const_context) ->
  bool` (WP-294, [expr.const]/7, clang `Sema::FinalizeVarWithDestructor`
  + `VarDecl::evaluateDestruction`): true when destroying `var` (a
  non-reference variable whose type, through arrays, has a non-trivial
  destructor) is a constant expression, so codegen emits no destructor
  call, `atexit` registration or guard for it. `value` is the frozen
  initializer value (`None`: not constant). A `const` variable's object
  is a writable static allocation (`Origin::Var`) holding `value`, or
  indeterminate without one; its `mutable` members are outside their
  lifetime, so reading or writing them faults. Any other variable's
  object is external: every access faults. `const_context` is clang's
  "is constant-initialized static" flag (lax evaluation, `RunState`).
  Silent, not recorded; an outermost evaluation (false when nested); the
  temporary object is freed (`Memory::free_static`, "Abstract memory")
  and memory reset.
- `Oven::fold_for_overflow(bridge, e) -> (Option<ConstValueId>,
  Vec<Overflow>)` (WP-468): `fold_rvalue` checking for undefined
  behaviour (clang `EvaluateForOverflow`, for `-Winteger-overflow`,
  spec/sema.md §5.28). `RunState::ub_check` is set for the outermost run
  only (`Oven::ub_check_next`, taken by `begin_outermost`; cleared in a
  nested evaluation, a global's initialization and speculation): a signed
  `IAdd`/`ISub`/`IMul`/`INeg` whose result is outside its width yields
  the result wrapped to the width and records `(pc, wrapped)`
  (`vm.rs` `Oven::wrap_overflow`, cold) instead of failing. The records come back
  in order as `Overflow { loc, e, value }` (the instruction's site); the
  evaluation's failure (`None`) does not drop them. Silent; lax, so not
  memoized.
- `Oven::fold_rvalue(bridge, e) -> Option<ConstValueId>` (WP-165):
  `try_fold` in **lax mode** (`RunState::lax`), clang's codegen-time
  `EvaluateAsRValue`/`tryEmitAsConstant`. A `const`, non-volatile variable
  that has an initializer (or a known value) but is not usable in constant
  expressions (a `const double`, a `const _Atomic(int)`, a `const` class,
  array or pointer) is read as if it were (`VarInfo::lax`); its `mutable`
  members are not (below, WP-183). Lax mode
  applies only to an outermost evaluation (nested in a bridge callback it
  is plain `try_fold`). Lax-mode calls are never memoized, so a lax
  read never leaks into a strict evaluation. A lax global keeps one
  static object for the whole translation unit (WP-407): it is resolved
  once, in whichever mode first names it, and is never reset by a mode
  switch, so a strict/lax alternation costs no static memory and a
  pointer to the global taken in one mode designates the same object,
  holding the same value, in the other (test
  `limits.rs::lax_globals_do_not_grow`). **Lax folds** through
  a usable variable (WP-395): a constexpr pointer, reference or class
  member pointing at a lax global (`constexpr const double *p = &d;
  int a[(int)*p];`) is resolved strictly, before or outside any lax fold,
  so the pointer it holds must designate an object a later lax fold can
  read. A lax global therefore gets one static object for both modes,
  holding its value but flagged external
  (`Origin::External`, `Alloc::frame == LAX_READ`): a strict read still
  faults "read of non-constexpr variable" exactly as before, and only a
  lax fold (`Memory::lax`, set by `fold_rvalue` for its outermost run)
  may read it; writes fault in either mode. Its value is the one sema
  froze, or else its initializer evaluated **laxly** whatever the mode
  of the evaluation that resolves it (`Oven::resolve_var` sets
  `RunState::lax` and `Memory::lax` around it): clang's
  `VarDecl::evaluateValue` only notes (`CCEDiag`) a read of a
  non-constexpr variable, so `const double d1 = d0 * 2;` keeps the value
  5.0 for a later fold however it is reached (`constexpr const double
  *p = &d1; int a[(int)*p];` folds, WP-407). A potentially-constant variable
  (a const integral, a reference) whose constant initialization failed
  (sema's `VarEvalState::NotConstant`, or a failed nested resolution) is
  read the same way (`Oven::refold_var_init`, WP-415): its initializer is
  evaluated once more, non-manifestly (`const_context` false,
  `__builtin_is_constant_evaluated()` false) and laxly, into a `LAX_READ`
  object, as clang's `VarDecl::evaluateValue` keeps a non-manifest value
  after noting "initializer of 'cx' is not a constant expression". So
  `const int cx = __builtin_is_constant_evaluated() ? x : 9;` reads 9 in
  a fold (`int a[cx]` is folded to a constant array, `__builtin_constant_p(cx)`
  is true) while a strict read still faults with that note; an
  initializer that does not fold either (`const int cy = x;`) leaves an
  external object. `__builtin_constant_p`'s speculation is itself a lax
  fold (`RunState::lax` and `Memory::lax` set for its region, as clang's
  `EvaluateBuiltinConstantP` folds): `__builtin_constant_p(d)` of
  `const double d = 2.5;` is true. Validation:
  `tests/conformance/expr.const/failed-constant-init-folds-non-manifestly.cpp`. Before WP-407 such a global
  was resolved per mode and reset at each mode switch, so a pointer
  resolved strictly did not fold and, worse, a pointer thawed in a lax
  fold kept reading the lax contents in a later strict evaluation that
  never re-resolved the global (`constexpr double v = *p;` accepted). An
  initializer that does not fold even laxly leaves the object unbacked,
  so every read faults in either mode; a global met mid-initialization
  (`VarEvalState::Evaluating`) is resolved provisionally, for that
  evaluation only.
  A genuinely non-constant bound (a pointer to a non-`const` variable, or
  any function-scope bound, which clang does not fold) stays a variable
  length array (tests `expr.const/lax-fold-through-pointer.cpp`,
  `expr.const/lax-fold-through-pointer-chain.cpp`,
  `dcl.array/vla-through-constexpr-pointer.cpp`). Codegen
  uses it for its non-initializer fold candidates; sema uses it
  (`OvenBridge::fold_rvalue_int`) for clang's folded array bound
  (spec/sema.md §4.2).
- Freezing a pointer or reference into a constant rejects a pointer to an
  object of automatic storage duration (a static allocation whose origin
  is a local variable read as a global): "pointer to '…' is not a
  constant expression" (or "reference to …" when the frozen type is a
  reference; clang `note_constexpr_non_global`), with "declared here".
  At C++26 (P2686R5 [expr.const.init]/2, WP-467) the pointer is frozen as
  `PtrBase::Decl(local)` instead when it is constexpr-referenceable from
  the variable being initialized (`Oven::referenceable_from_object`:
  `run.object` is a non-static local declared `constexpr`, of the same
  function as the referent's variable; a `static` one is not a local, and
  a non-`constexpr` one is deliberately excluded, spec/sema.md §4.9). Globals are
  resolved once per Oven, so the check is made at the freeze, against
  `run.object`, not at resolve time. `automatic_referent(ast, value)`
  finds such a pointer anywhere in a frozen value (scalars, relocations,
  pointer-to-constant chains), which sema uses for usability
  (spec/sema.md §4.9) and codegen to skip the fold. A temporary
  lifetime-extended by such a `constexpr` local reference (WP-1122) is an
  automatic object too: `begin_static_extended` allocates the variable's
  `StorageDuration::Automatic` extended temporaries as `Origin::Let`
  objects as well as its static ones, the pointer freezes as
  `PtrBase::Temporary(t)`, and `automatic_referent` reports a temporary
  whose extending variable is a local. Left: a lambda body
  reading through such a pointer in its own `static_assert` is accepted
  (the evaluation point is not known to the Oven).
- Freezing a pointer or reference to an object with thread storage
  duration fails silently (`FailKind::NotConstant`, clang
  `CheckLValueConstantExpression`): [expr.const]/14 permits static
  storage duration only. This covers the `thread_local` variable being
  initialized itself (`run.object`, e.g. `S *self = this;` in a default
  member initializer, or a small-buffer pointer into the object), so
  such a variable is dynamically initialized per thread rather than
  emitted with its own address in the `.tdata` image (WP-792; test
  `basic.stc.thread/self-address-dynamic-init-run.cpp`).
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
  The "declared here" of a `MemFault::External` read names the
  parameter's or usable variable's first declaration ("initializer of
  'c' is not a constant expression"/"is unknown"), and for a variable
  that is not usable ("read of non-const(expr) variable") its definition
  once seen, else the redeclaration the faulting `DeclRefExpr` names
  (clang `findCompleteObject` replaces the `DeclRefExpr`'s declaration
  by `getDefinition()`; `notes.rs` `used_redecl`, WP-415,
  `tests/conformance/expr.const/declared-here-names-definition.cpp`).

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
  A VLA type's size never reaches evaluation (WP-1287): any other
  expression of variably modified type -- pointer arithmetic on a pointer
  to a VLA (`p + 1`, `++p`, `p += 1`), a subscript or dereference yielding
  a VLA -- throws `Invalid` at its location (a subscript's is its base's)
  before anything reads the zero-size layout (`variably_modified` in
  `compile.rs`). A name, a cast (its own rules apply) and a parenthesis
  carry a pointer value and need no size, so `int (*p)[n] = nullptr;
  p == nullptr` stays constant, as in clang
  (`dcl.array/vla-size-never-constant.cpp`).
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
  `HAS_UNION` take the outlined slow path (`slow_access`; out of line
  but not `#[cold]`, since union-backed storage takes it on every
  access; no cost for other allocations; the union check is
  `union_path_in`, "Union-path memo" below): `mutable_path` walks from the allocation root to
  the read object (a union through its active member) and faults at the
  outermost `mutable` member on the way; a read of a whole class object
  (the `Copy` of a trivial copy) faults at the first `mutable` member it
  reads (members before bases, recursively; not one of empty class type;
  any in a union; clang `diagnoseMutableFields`). The object being
  initialized (`RESOLVING`), its lifetime-extended temporaries, and
  frame and heap objects never carry the flag, so their mutable members
  are read and written freely (on a frame or heap allocation the same bit
  is `flags::VOLATILE`, and `mutable_path` hands such an access to
  `volatile_path`; WP-1019, "Undefined behaviour checks"). The read carries the trivial copy's "in
  call to 'S(s)'" frame (WP-241). A *write* (`store`, `store_bits`) to a
  `mutable` member of a `MUTABLE_OUTSIDE` allocation that is not `CONST`
  faults the same way (`mutable_path` with the store's access kind, whose
  verb opens the note): this is the writable `const` object of
  `has_constant_destruction` (WP-294), whose mutable members are outside
  their lifetime; initialization stores are exempt. Deviation: clang
  rejects every such read before C++14, kiln does not.

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
  `Oven::warn_constant_evaluated(bridge, e, var)` (WP-415) reruns the
  constant initializer `e` of a variable whose silent constant
  initialization failed with `RunState::cev_warn` set, so a silent run
  still reports this warning (and nothing else): clang warns while
  checking the constant initializer even when it then fails (spec/sema.md
  "Constant initialization"). `cev_warn` is cleared in speculation
  (`__builtin_constant_p`) and in the nested initialization of a variable
  it reads.
- **`if consteval`** ([stmt.if]/4; WP-588) reads the same flag: a
  `consteval` / `!consteval` if statement compiles both substatements and
  branches on `IsConstEval [dst]` with `aux` 16 (the flag alone, never
  `-Wconstant-evaluated`). It is a run-time test, not a compile-time
  choice, because one compiled body serves every evaluation: a trial fold
  (`try_fold`: codegen folding a plain `if` condition, an operand of `&&`
  or `?:`, a dynamic initializer) runs the else branch, as clang's
  `ExprConstant` does when `InConstantContext` is false and GCC's
  `cp_fold` with `manifestly_const_eval` false; every manifestly
  constant-evaluated evaluation (above) runs the compound statement.
  Codegen lowers the run-time `if consteval` to its else branch
  (spec/codegen.md). Before WP-588 the Oven compiled only the compound
  statement, so a trial fold of a libstdc++ `char_traits::length` took
  the constant-evaluation loop and the folded result replaced the run-time
  call. Tests: `tests/conformance/stmt.if.consteval/`.
- **P4143R0 "Constant evaluation fixes"** (Croydon DR, every `-std=`;
  WP-757). The paper rewords [lex.phases]/1.7 and [expr.const] without
  changing what kiln does, so it is pinned by tests only:
  - *Each* manifestly constant-evaluated expression is evaluated during
    translation ([lex.phases]/1.7), and its side effects (a consteval
    block's `define_aggregate`) are complete from every point from which
    the evaluation is reachable: a complete-class context of the class
    holding the consteval block sees the completed type, a nested class's
    static member initializer before the block does not (spec/sema.md
    "Other complete-class contexts"; spec/consteval-blocks.md).
  - A manifestly constant-evaluated expression is evaluated even in an
    unevaluated operand ([expr.const]/27 footnote): a template argument
    in `decltype`, `sizeof` or `noexcept` is evaluated and its failure
    diagnosed, while a call to an immediate function in an unevaluated
    operand is not an immediate invocation and is not evaluated. The
    *static_assert-message* of a holding assertion is the exception (an
    unevaluated operand, spec/sema.md §4.6).
  - `std::is_constant_evaluated()` is true while determining whether an
    initializer is constant-initializable ([expr.const]/5.1 notes; the
    `const_context` rule above), and a `constexpr` local of a recursive
    function is a distinct object per invocation, created at run time.
  - Tests: `tests/conformance/lex.phases/p4143-*.cpp`,
    `tests/conformance/expr.const/p4143-*.cpp`.
- **Memoization:** a function whose body uses `IsConstEval` for the
  builtin is never memoized (the warning is a side effect); every memo
  key carries the context (bit 31 of the function word), so a call cached
  in one context is never replayed in the other (an `if consteval` body
  is memoized under that key).

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
  A direct call with one argument to `std::move`, `std::move_if_noexcept`,
  `std::forward`, `std::forward_like`, `std::as_const`, `std::addressof`
  or `std::__addressof` is not a call: it costs no step, no frame and no
  depth (clang's `isStdBuiltin` set; `compile.rs` `is_std_builtin`;
  `expr.const/budget/std-builtins*.cpp`; WP-418).
- **Depth** counts active calls as clang's `-fconstexpr-depth` does ("constexpr
  evaluation exceeded maximum depth of N calls"); a nested evaluation counts
  from the depth of the call that started it, plus one (see "Nested
  evaluation").
- **Bytes** bounds the abstract-memory address space held,
  `frame.top + heap.top` ("constexpr evaluation exceeded the memory limit
  of N bytes", at the object's declaration); nested evaluations share it.
- **Reflection bytes** (`refl_bytes`; WP-479, spec/reflection.md §2.4,
  §3.8, spec/reflection-values.md §5). A reflection intrinsic that grows
  the translation unit's interned tables (a new reflection table entry,
  new constant-store content for `reflect_constant`,
  `reflect_constant_string`/`_array` or `define_static_*`) charges the
  growth to the running evaluation: content bytes as the constant store
  counts them, plus `REFL_ENTRY_BYTES` (`meta/intern.rs`) per table entry.
  The charge is `Memory::refl`, which is live for the rest of the
  evaluation, so it counts into `peak_bytes`, `peak_arena` and the
  **Bytes** limit above (the same "exceeded the memory limit" error at
  the call). Content interning makes it bounded: a call whose result is
  already interned charges 0, so `n` calls over `k` distinct values
  charge `O(k)` (`meta.reflection.result/reflect-constant-budget.cpp`,
  `stress/reflect-constant-1e5.cpp`). Every op charges its new table
  entries; only the value and name families charge constant-store
  content, so what `substitute`'s instantiation or `members_of`'s class
  completion interns is not reflection storage. Each byte is charged
  once, by the call that interned it: a call subtracts what the
  intrinsic calls nested inside it (in evaluations its bridge callback
  ran on the lent Oven) already attributed (`Oven::refl_seen`), so a
  1,000-deep `can_substitute` chain charges linearly
  (`stress/substitute-nested-1000.cpp`). The tables are not reclaimed
  during the evaluation (an `info` may sit anywhere in Oven memory); a
  per-evaluation overlay table is an M7 item.
- `EvalStats` holds one `EvalRecord` per `evaluate` call and TU totals.
  `Oven::footprint()` is the evaluator's own heap (code, memory, caches,
  and the type table with its offset index, WP-642). `Oven::memory()`
  (WP-663, `-fkiln-memory-report`) splits the same total into rows
  `oven.code`, `oven.sites`, `oven.funcs`, `oven.thunks` (count = cached
  thunks), `oven.globals`, `oven.mem.capacity` (abstract memory's
  buffers and tables as held; count = the static top), `oven.memo` and
  `oven.other`; their reserved bytes sum to `footprint()` (tested).
- **Memory report: the Oven's peaks** (WP-1315). Two more rows count
  abstract memory exactly, and the report's total counts them instead of
  `oven.mem.capacity` (a `.capacity` row is shown for information and is
  outside the total, spec/driver.md):
  - `oven.heap-peak` (count, live and reserved = the bytes): the
    translation unit's evaluation-memory high-water mark,
    `Memory::heap_peak`, the most address space any evaluation held at
    once, as `peak_arena` counts it (`frame.top + heap.top` plus the
    reflection bytes), nested evaluations included. Memory a constant
    evaluation frees again, or that is dropped when it ends, still
    counts: it is the peak, not what is held when the report is taken.
    It is kept as `Memory::arena_peak` is, and folded into a
    translation-unit maximum when an evaluation context ends (the end of
    the outermost one, `Memory::reset`; the end of a nested one, whose
    peak is otherwise restored away; and the start of an outermost one),
    so the VM's paths do nothing new. It never decreases.
  - `oven.static-peak`: the static space's high-water mark
    (`Memory::static_peak`, at least the static top).

  Why not the buffers' capacity: `Space::grow` at least doubles a space,
  so capacity moves in steps unrelated to the peak. In
  `bench/oven/class-churn.cpp` scaled to N elements, the capacity row was
  2.18 MB at N = 10000, 10200 and 10500 (peak +5%), 3.16 MB at 11000 and
  2.51 MB at 15000 (not monotone), while `oven.heap-peak` is 680 472,
  686 872, 696 472, 843 544 and 1 004 312 bytes: monotone and equal to
  the evaluation's `peak_arena`. Tests: `mem.rs`
  `heap_peak_is_the_tu_high_water_mark` (grows with what is held at once,
  not with churn; not lowered by a free or a reset; deterministic) and
  `kiln_driver/tests/driver.rs` `memory_report_oven_heap_peak` (the rows
  and the total from the driver).

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
{"loc":"a.cpp:12:1","kind":"static_assert","steps":116,"bytes":40,"peak_bytes":32,"calls":3,"allocs":0,"peak_arena":32,"frozen_bytes":16,"static_bytes":0,"memo_hits":0,"refl_bytes":0,"reclaimed_bytes":24,"wall_us":4}
{"total":{"steps":116,"bytes":40,"peak_bytes":32,"calls":3,"allocs":0,"peak_arena":32,"frozen_bytes":16,"static_bytes":0,"memo_hits":0,"refl_bytes":0,"reclaimed_bytes":24,"wall_us":4,"evaluations":1,"splice_fast":0}}
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
| `kind` | `static_assert`; `init` (the initializer of a `constexpr`/`constinit` variable or a constant initialization); `template_arg` (a template argument, including default arguments); `consteval` (an immediate invocation); `consteval_block` (a `consteval` block, P2996); `if_constexpr` (the condition); `constraint` (a constraint satisfaction check, M3); `other` (array bounds, enumerator values, `case` labels, `alignas`, `noexcept` and `explicit` specifiers, bit-field widths, splices, ...). |
| `steps` | Statements executed, counted exactly as clang counts `-fconstexpr-steps` (see "Limits and metrics"), including the recorded steps of memoized calls (as for the budget, so the count does not depend on cache state). |
| `bytes` | Abstract-memory bytes allocated over the evaluation (cumulative, each object rounded up to the 8-byte granule: frames, temporaries and dynamic allocations; a freed byte stays counted). |
| `peak_bytes` | The maximum number of abstract-memory bytes live at once (`frame.top + heap_live`, see "Transient allocation"). |
| `calls` | Function calls, memoized hits included. |
| `allocs` | Dynamic allocations (`new` expressions and `std::allocator<T>::allocate`). |
| `peak_arena` | The maximum abstract-memory address space held at once (`frame.top + heap.top`), fragmentation included; see "Transient allocation". `peak_arena >= peak_bytes`. |
| `frozen_bytes` | Bytes this evaluation added to the `ConstStore`: only newly interned bytes, so a result equal to one already frozen adds 0 (spec/ast.md §9). |
| `static_bytes` | Bytes of the static space held when the evaluation ended (`Memory::static_top`: live static objects plus the free ranges below the top; "Abstract memory", static reclamation). The space persists across evaluations, so this is the TU's so far. Optional for consumers (absent in reports before WP-322: read as 0). |
| `memo_hits` | Calls answered from the memo table ("Memoization"); a nested evaluation's hits are its own line's. The one counter that differs with the cache on or off: it observes the cache, it is not a cost. Optional for consumers (absent in reports before WP-470: read as 0). |
| `refl_bytes` | Bytes this evaluation added to the translation unit's reflection table and content-interned constants ("Reflection bytes" below). Already part of `peak_bytes` and `peak_arena`; reported separately so reflection growth is visible. Optional for consumers (absent in reports before WP-479: read as 0). |
| `reclaimed_bytes` | Of `bytes`, those freed before the evaluation ended (`delete`, `std::allocator<T>::deallocate`, a returning call's frame objects) and so reusable within it, rather than released with the evaluation ("Transient allocation"; `EvalRecord::reclaimed_bytes`, the `-fconstexpr-profile` `reclaimed`). `reclaimed_bytes <= bytes`. Optional for consumers (absent in reports before WP-1147: read as 0). |
| `wall_us` | Wall-clock microseconds, rounded down. |

The `total` object has the same counters summed over the evaluations, except
`peak_bytes` and `peak_arena`, which are the **maximum** over them (top-level
evaluations do not overlap, so the TU's peak is the largest single peak;
a nested evaluation's peaks are exclusive, see "Nested evaluation"), and
`static_bytes`, the static space's **high-water mark** over the whole TU
(`Memory::static_peak`), which also covers work no line records
(`has_constant_destruction`, `try_fold`) and so is at least every line's,
plus `evaluations`, the number
of evaluation lines, and `splice_fast`, the splice operands sema read
from a frozen reflection without starting an evaluation (spec/reflection.md
"Fast path"; WP-869). `splice_fast` is a total-only key: no evaluation
line carries it and it is not cross-checked against them; it is optional
for consumers (absent in reports before WP-884: read as 0). All counters
are integers in `[0, 2^53]`.

Every counter except `wall_us` is **deterministic**: the same compiler on the
same input reports the same values, on any host and under any load, which is
what lets budgets and the bench compare them exactly. A change in kiln that
moves them is a measured change in the evaluator, not noise.

Consumers must reject a report that is not this format: a line that is not
an object with the keys above (each of the right type), a `kind` outside the
list, a missing or repeated `total`, text after it, or a `total` that does not
match the evaluations (sums, maximum for `peak_bytes` and `peak_arena`, count for
`evaluations`, at least the maximum for `static_bytes`; `wall_us` is not cross-checked, since rounding makes its sum
inexact). They ignore unknown keys (so the format can grow) and blank lines.

### Trace hook — Landed (WP-1115)
Every Oven event of `-ftime-trace` (spec/driver.md "Time trace") goes
through the `hook_*` methods of `crates/kiln_oven/src/hooks.rs`; the rest
of the Oven only calls them. `-fconstexpr-stats` / `-fconstexpr-profile`
(WP-1116) aggregate the same begin/end pairs per call site by adding a
sink inside these methods, not a second set of call sites.

**Events** (all on the compiling thread, nested as the evaluation runs):

| Event | When | Detail | `-ftime-trace-verbose` args |
|---|---|---|---|
| `EvaluateAsInitializer` | a top-level evaluation with `EvalKind::Init` and an object | the variable's qualified name | `kind`, `steps` |
| `EvaluateAsRValue` | a top-level `EvalKind::StaticAssert` evaluation (clang's `VerifyIntegerConstantExpression`) | the condition's range | `kind`, `steps` |
| `EvaluateAsConstantExpr` | any other top-level evaluation (an immediate invocation, a template argument, `if constexpr`, ...; `evaluate_chars`) | the expression's range, `<file:line:col[, col:c]>` (`evaluate_chars`: `ctx.loc`) | `kind`, `steps` |
| `OvenCompile` | a function compiled to VM code (first call) | its qualified name | — |
| `ConstevalCall` / `ConstexprCall` | verbose only: a VM call of an immediate / other function, until its frame is popped | the callee's qualified name | — |
| `OvenMetaOp` | a reflection meta operation (`SemaBridge::meta`) | the operation (`members_of`) | — |

Execution is not a separate event: the VM compiles lazily, so compile and
execute interleave; an evaluation's self time minus its `OvenCompile`
children is its execution. Nested evaluations (a bridge callback that
evaluates, `ctx_depth > 0`) are events of their own, inside the outer one.

**Call sites** (the only Oven lines that know of the hook; WP-1100 and
WP-455 edit the Oven in parallel, so they stay thin):
- `lib.rs` `Oven::evaluate` and `Oven::evaluate_chars`: `let hook =
  self.hook_eval_begin(&ctx);` first, and `if let Some(m) = hook {
  self.hook_eval_end(bridge, m, e, &ctx, &c, r.is_ok()) }` after the
  evaluation (the cost and the outcome: WP-1116);
- `vm.rs` `enter`: `self.hook_compile(b, callee)` instead of
  `compile_func`, and `if self.hooks.calls { self.hook_call_enter(b,
  callee) }` after the frame is pushed;
- `vm.rs` `Op::Ret`: `if self.hooks.calls { self.hook_call_ret(b) }` after
  the frame is popped;
- (WP-1116) `vm.rs` `enter`, a memo hit: `if self.hooks.calls {
  self.hook_memo_hit(callee, v.steps, v.peak) }` before the hit is
  charged (no frame is pushed, so no other hook sees the call);
- (WP-1116) `vm.rs` `exec` and `trial`: `if self.hooks.calls {
  self.hook_call_unwind(b, entry) }` right after `run_loop`, before the
  memo table's pending peaks are restored;
- (WP-1116) `vm/exc.rs` `unwind`: `if self.hooks.calls {
  self.hook_call_ret(b) }` after each frame an exception pops;
- `vm/meta.rs`: `hook_meta_begin()` / `Oven::hook_meta_end(t, op)` around
  `SemaBridge::meta`.

A frame unwound by an exception ends where `vm/exc.rs` pops it; one
abandoned by a failure or the end of a trial ends where `exec` / `trial`
return (`hook_call_unwind`). These three sites (WP-1116) make the
profile's per-call steps and peaks exact: a call that ended lazily, at
the next call entered at its depth (`hook_call_enter`, still the
fallback) or at `hook_eval_end`, would be charged the caller's steps
that ran in between, and its peak would be read after the memo table
had restored the enclosing call's. Each is one `hooks.calls` test, the
same as `Op::Ret`'s.

**API for a sink** (`pub(crate)`; WP-1116 extends it):
- `Hooks { on, calls, trace_calls, marks, prof }`, the field
  `Oven::hooks`: `on` is read from the recorder
  (`kiln_source::trace::enabled`) and the profile's level at each
  top-level evaluation (`ctx_depth == 0`) and gates every hook; `calls`
  (`trace && verbose`, or `-fconstexpr-profile`) gates the per-call ones;
  `trace_calls` is the trace's half of it. `Hooks::refresh` ORs the
  sinks' switches.
- `hook_eval_begin(&mut self, &EvalContext) -> Option<EvalMark>` /
  `hook_eval_end(&mut self, &dyn SemaBridge, EvalMark, Option<ExprId>,
  &EvalContext, &Cost, ok)`: one pair per evaluation; the end closes the
  calls the evaluation left open.
- `hook_call_enter(&mut self, &dyn SemaBridge, func)` /
  `hook_call_ret(&mut self, &dyn SemaBridge)`: `marks` is the stack of
  open calls (`CallMark { depth, func, span }`, `depth` the VM frame
  depth with the callee's frame pushed); a sink that aggregates per call
  site reads `func` (`Oven::funcs[func].decl`) and the caller's
  `Frame::call_pc` (`Oven::site`) there.
- `hook_memo_hit(&mut self, func, steps, peak)`,
  `hook_call_unwind(&mut self, &dyn SemaBridge, depth)` (WP-1116).
- `hook_compile(&mut self, &mut dyn SemaBridge, func)`,
  `hook_meta_begin(&mut self) -> Option<Span>` / `hook_meta_end(Span,
  MetaOp)`.

**Off cost.** With no sink, `hook_eval_begin` reads one thread-local bool
per top-level evaluation and the VM tests the plain bool `hooks.calls`
once per call and per return (the hook bodies are `#[cold]`, out of
line); no clock read, no allocation. `Frame` does not grow. A/B on
`cargo xtask bench --filter oven/` against the parent commit: within
noise (WP-1115 landing note). The bench entry `oven/time-trace` measures
the traced cost.

### Profile counters — Landed (WP-1116)
`-fconstexpr-stats` and `-fconstexpr-profile` (spec/driver.md "Constexpr
profile") read a sink inside the trace hook methods
(`crates/kiln_oven/src/profile.rs`, `Hooks::prof`) plus the Oven's
existing counters. `Oven::set_profile(Level)` arms it before the first
evaluation (`kiln_parse::ConstexprLimits::profile`):

- `Level::Off` (default): no sink; the hooks behave as WP-1115's.
- `Level::Stats` (`-fconstexpr-stats` alone): `Hooks::on` is set, so
  every evaluation takes `hook_eval_begin`'s out-of-line half (one
  thread-local read), each compile two clock reads and each meta
  operation one increment. `Hooks::calls` stays off: the VM's per-call
  cost is WP-1115's off cost.
- `Level::Calls` (`-fconstexpr-profile`): `Hooks::calls` is set too, and
  every VM call takes `hook_call_enter` / a return hook (two clock
  reads, a mark push and pop, a row update).

**The summary's counters**, by line:

| Line | Key | Meaning |
|---|---|---|
| `evaluations` | `count`, `failed` | recorded top-level evaluations (`EvalStats::totals`) and those that were not constant |
| | `immediate` | those with `EvalKind::Consteval` (immediate invocations) |
| | `wall-us` | their wall time, each context's own (a nested one is not counted twice) |
| `steps` | `count` | their VM steps (memo hits charged their recorded steps: the step budget's count) |
| `calls` | `count` | their calls, a memo hit counting the calls it stands for (`-fconstexpr-report`'s `calls`) |
| | `memo-hits`, `memo-misses`, `memo-hit-rate` | the memo table's lookups in the TU, trials included (`Memo::hits` / `misses`; the rate to one decimal, `-` with no lookup) |
| `memory` | `allocs`, `bytes` | constexpr `new` / `std::allocator` allocations and their bytes (recorded evaluations) |
| | `peak-live` | the most bytes live at once in one evaluation (`Memory::peak`: frames, heap, reflection) |
| | `reclaimed` | bytes allocated and freed before the evaluation ended: `allocated` minus the frame and heap bytes still held (WP-060) |
| | `peak-arena` | the largest arena high-water mark |
| `interned` | `frozen-bytes`, `refl-bytes`, `static-bytes` | constant pool bytes interned, reflection data made, static storage materialized |
| `meta-ops` | `count` | reflection meta operations (`SemaBridge::meta` calls through `hook_meta_begin`) |
| | `splice-fast` | splices answered without a meta operation (`EvalTotals::splice_fast`) |
| `trials` | `count`, `failed`, `steps`, `wall-us` | unrecorded evaluation contexts (`try_fold`, `fold_*`, constant-initialization checks, `evaluate_potential`, ...): every `eval_job` (`EvalStats::jobs`, `JobTotals`) minus the recorded ones |
| `compile` | `functions`, `wall-us` | functions compiled to VM code and the time spent compiling them |

**The profile's rows.** `Profile::funcs` is indexed by `Oven::funcs`
(one `Row` per VM function, merged per declaration when reported);
`Profile::sites` is keyed by (`EvalContext::loc`, `EvalKind`). Each row
keeps `count`, `memo_hits`, `failed`, inclusive / exclusive steps and
nanoseconds, and `peak`. The accounting, on the `Hooks::marks` stack:

- An evaluation pushes a mark of depth 0 (`EVAL`, never closed by a
  call's end) at `hook_eval_begin` and pops it at `hook_eval_end`,
  charging its site. A call pushes one at `hook_call_enter` and pops it
  when its frame is gone (`Op::Ret`, an exception's unwind, the end of
  `exec` / `trial`, or the next call at its depth).
- A mark's inclusive steps are the step counter's advance across it;
  its exclusive ones subtract its children's (calls, memo hits, nested
  evaluations). The step counter restarts for each outermost evaluation
  and for a variable's initialization evaluated inside another
  (`RunState::nesting`): a child at another nesting adds its time to the
  parent's children but not its steps, which the counter never charged
  to the parent.
- A recursive function's inclusive steps and time are charged once, when
  its outermost activation ends (`Row::active` counts open
  activations); its exclusive ones and its count, at every activation.
- A memo hit is a call (`count`, `memo_hits`) whose inclusive and
  exclusive steps are the steps it stands for, with no time.
- A call's peak is the most bytes live during it above those live at its
  entry: `hook_call_enter` sets `Memory::peak` to the bytes live then,
  and its end raises it back to the saved peak (a maximum), so the
  evaluation's own `peak_bytes` is unchanged. A site's peak is the
  evaluation's `peak_bytes`.
- Trials push no site mark; their calls are charged to the function rows
  (the `total` line counts them separately).

Ranking and rendering are `ProfileRows::render` (spec/driver.md
"Constexpr profile"). Step counts are exact: the profile reads the same
counter as the step budget, at every call boundary.

**Where it touches the Oven** (for WP-1100 / WP-455 rebases):
`hooks.rs` (the sink, all hook bodies), `profile.rs` (new), the three
unwind sites and the memo-hit site above, `lib.rs` (`hook_eval_end`'s
arguments, `Cost::reclaimed`, `EvalRecord::reclaimed_bytes`,
`EvalStats::note_job` in `eval_job`, `reclaimed` in `end_outermost` and
`eval_nested`), `mem.rs` (`Memory::held`), `stats.rs` (`JobTotals`,
`reclaimed_bytes`, `failed`).

**Overhead.** Off: no sink; the VM's cost is WP-1115's off cost (one
predictable `hooks.calls` test per call and return, now also per memo
hit, per `exec` / `trial` return and per exception-popped frame). On:
the WP-1116 landing note records `cargo xtask bench --filter oven/` with
each flag.

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
- **GCC's implicit library builtins (WP-380):** a direct call of a
  function with no builtin id but a `gnu_lib_twin` (spec/sema.md §5.4:
  the C library `ceil`/`floor`/`trunc`/`round`/`fabs`/`fmin`/`fmax`/
  `copysign`, and since WP-393 `sqrt`, with their `f`/`l` variants under
  `-stdlib=libstdc++`) is
  lowered as a call of the twin `__builtin_` function, so it folds with
  the twin's rules; under `-stdlib=libc++` such a call is a call of a
  non-constexpr function, as in clang.
- **C library functions clang evaluates (WP-940):** a direct call of a
  function whose recorded library twin (`Decls::lib_builtin`, spec/sema.md
  §5.4) is one of `builtin::lib_evaluated` (`strlen`, `wcslen`, `strcmp`,
  `wcscmp`, `strncmp`, `wcsncmp`, `memcmp`, `bcmp`, `wmemcmp`, `strchr`,
  `wcschr`, `memchr`, `wmemchr`, `memcpy`, `memmove`, `wmemcpy`,
  `wmemmove`: clang `VisitBuiltinCallExpr`'s `BIstrlen`-style cases) is an
  `Op::Cce` check with clang's note "non-constexpr function 'strlen'
  cannot be used in a constant expression" (no "declared here"), then the
  twin `__builtin_` call. A constant expression fails at the note; a fold
  (`__builtin_constant_p`, a GNU-folded array bound, codegen's
  `fold_rvalue` and constant initializers) continues, so `strlen("abc")`
  folds to 3 as in clang 19 and 22.
- **`__builtin_` library aliases (WP-1104):** `__builtin_malloc`,
  `calloc`, `realloc`, `free`, `fprintf`, `vfprintf`, `fscanf`,
  `vfscanf`, the string aliases (`__builtin_strcpy`, ...) and the
  `_uninitialized` allocas (`builtin::is_libc_fwd`) are never
  evaluated: a call traps with clang's note "subexpression not valid
  in a constant expression" (plus "in call to" inside a function), so
  `constexpr void *p = __builtin_malloc(4);` is "constexpr variable 'p'
  must be initialized by a constant expression", as in clang 19 and 22
  (`ext.gnu.builtins/library-fwd-constexpr.cpp`).
- **Atomics (WP-154):** `__atomic_always_lock_free`, `__atomic_is_lock_free`
  and `__c11_atomic_is_lock_free` fold through `bi::LOCK_FREE` as clang
  19's `IntExprEvaluator`: a power-of-two size within the target's inline
  width (`Target::max_atomic_inline_bytes`: aarch64 16, x86_64 8 bytes) is
  lock-free for the c11 form, a null object, a single byte, or an object
  whose pointee alignment is at least the size (the pointee as written:
  an `aligned` typedef's alignment counts, `typedef_align`; WP-335), or
  (WP-726) an object argument that is an integer cast to a pointer whose
  value is a multiple of the size (clang 19 folds the argument with
  `EvaluateAsRValue`: libstdc++'s `reinterpret_cast<void *>(-_S_alignment)`
  in `std::atomic<T>::is_lock_free`); otherwise
  `always_lock_free` is false and `is_lock_free` is not a constant.
  `NonAtomicToAtomic`/`AtomicToNonAtomic` casts are no-ops, also when
  they initialize an object in place: `_Atomic(S)` of class type `S` is
  initialized as its `S` operand (libc++'s `__cxx_atomic_base_impl(T v)
  : __a_value(v)`, so `constexpr std::atomic<S> a(s)`); a size-promoted
  `_Atomic(S)` keeps its tail as padding (WP-334,
  `tests/conformance/ext.c11.atomic/constexpr-class-init.cpp`). Every other
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
  constant expression", as clang 19): the math functions outside the
  P0533 set below (`sin`, `exp`, `pow`, `cbrt`, `hypot`, ...:
  `builtin::is_libm` minus `p0533_base`); `__builtin_unreachable` when
  reached; `__builtin_operator_new`/`_delete` outside
  `std::allocator<T>::allocate`/`deallocate` (WP-061b; clang's
  untyped-memory note; inside them they allocate, "Transient
  allocation", WP-212); `__builtin_debugtrap`, `__builtin_frame_address`
  and `__builtin_return_address`, noted at the call's begin (WP-432),
  and `__builtin_thread_pointer` likewise (WP-782);
  `__builtin_extract_return_addr`, `__builtin_frob_return_addr` and
  `__builtin___clear_cache` likewise (WP-1288);
  every `__builtin___X_chk` likewise (WP-803);
  `__builtin_prefetch` likewise (WP-610); every x86 builtin but the
  scalar bit ones below likewise, `__builtin_ia32_pause` (WP-610)
  included, and `__builtin_nontemporal_store` / `_load` and
  `__builtin_nondeterministic_value` (WP-630).
- **x86 builtins** (WP-630; clang 19 `IntExprEvaluator`): `lzcnt_u16` /
  `_u32` / `_u64` and `tzcnt_*` are `bi::CLZ` / `bi::CTZ` with the width
  as the zero fallback; `bextr_u32` / `_u64` / `bextri_*`, `bzhi_si` /
  `_di`, `pdep_si` / `_di` and `pext_si` / `_di` are `bi::INT_OP`'s
  `BEXTR` (start in control bits 0-7, length in 8-15, clamped to the
  width; zero for a zero length or a start past the width), `BZHI`,
  `PDEP` and `PEXT`. At `-fclang-compat=22` (clang 20's addition)
  `addcarryx_u32` / `_u64` and `subborrow_u32` / `_u64` are `bi::CARRY`
  with `CARRY_BOOL_IN` (the carry-in counts as 1 when nonzero): the
  operands, then the pointer, are evaluated, the wrapped result is
  stored and the carry-out is the value (`target.x86/constexpr-*.cpp`).
- **Floating-point math, P0533** (WP-365; `bi::FP_MATH`, sub = float
  code | op << 4 | `SECOND` for the pointer output; `softfloat::math`):
  `ceil`/`floor`/`trunc`/`round`/`roundeven`/`rint`/`nearbyint`,
  `lround`/`llround`/`lrint`/`llrint`, `fmod`/`remainder`/`remquo`,
  `fdim`, `fma`, `frexp`/`ldexp`/`scalbn`/`scalbln`/`ilogb`/`logb`/`modf`,
  `nextafter`/`nexttoward`, `nextafterf16`/`f16b`, `signbit` (WP-419:
  `signbitf` / `signbitl` through `builtin::f128_twin`), the six
  comparison macros (`isgreater` ... `isunordered`, quiet: false on NaN)
  and integer `abs`/`labs`/`llabs` (`bi::IABS`), and P1383's `sqrt`
  (WP-393, `bi::SQRT`), each in its plain, `f`, `l` and `f128` spellings
  (`kiln_ast` `builtin::p0533_base`; `sqrt` also clang's `__fp16`
  `__builtin_sqrtf16`; WP-403: `ceil`/`floor`/`trunc`/`round`/`rint`/
  `fma`/`frexp` also as `__builtin_<fn>f16`, which `builtin::f16_twin`
  maps to the `f` form, the `__fp16` operand picking binary16), evaluate
  exactly in the operand's format: binary16, bfloat16, binary32,
  binary64, x87 80-bit and binary128 all go through the integer
  significand code in `softfloat` (no host libm, no host float
  arithmetic; one rounding, nearest-even, the only mode a constant
  expression sees). `_Float16`/`__bf16`/`_Float32`/`_Float64` reach the
  `float`/`double` forms by conversion, as libstdc++'s overloads do.
  Policy (C23 Annex F, as GCC folds): a call is not a constant
  (`Fault::Invalid`; "subexpression not valid in a constant expression"
  inside a function, the bare initializer error at top level) when it
  raises an IEEE exception other than inexact: invalid (a signaling NaN
  operand, an out-of-range or NaN `lround`/`lrint` source, `fmod`/
  `remainder`/`remquo` by zero or of an infinity, inf * 0 or inf - inf in
  `fma`, `ilogb` of zero/NaN/infinity), overflow (a finite result
  rounding to infinity in `fma`, `fdim`, `ldexp`, `nextafter`),
  divide-by-zero (`logb(0)`), or underflow (an inexact subnormal `ldexp`/
  `fma` result, and every subnormal or zero `nextafter` result, F.10.8.3);
  `abs` of the most negative value is undefined (a note-less failure,
  `Fault::Quiet`, so the error lands at the declaration as clang 22's). Exact infinite and NaN
  results from infinite or quiet-NaN operands are constants. They are
  evaluated whatever `-fclang-compat` says; codegen's constant-folding
  prepass leaves them as calls (`clang19_unevaluated`) so the IR still
  matches clang 19, which evaluates none of them. **Divergences:** GCC
  14 folds no form of `rint`/`nearbyint`/`lrint` (kiln rounds
  nearest-even), rejects `fmod(x, inf)` (= x here) and any infinite
  operand to its MPFR folders (`fma`, `fdim`, `ldexp`), and does not fold
  the pointer-output `*f128` calls.
  **`sqrt`** (WP-393, IEEE 754-2019 §5.4.1 squareRoot, C23 F.10.4.5) is
  correctly rounded in the operand's format: binary32 and binary64 use
  Rust's `f32::sqrt`/`f64::sqrt` (IEEE squareRoot, correctly rounded on
  every host; the one place the P0533 code uses host float arithmetic),
  the other formats `softfloat::math::sqrt_fin`, a digit-by-digit integer
  root of the significand to 125 bits plus a sticky bit, rounded once by
  `pack` (unit tests check it against the host for binary32/binary64).
  `sqrt(-0)` is -0; a negative finite operand, -inf or a signaling NaN
  raises invalid and is not a constant; `sqrt(+inf)` is +inf and a quiet
  NaN is returned (no exception, so constants by [library.c]/3).
  **Divergences:** g++ 14 also rejects `sqrt(+inf)` and `sqrt(qNaN)`;
  clang 19 and 22 evaluate no `sqrt` form (`__has_constexpr_builtin`
  answers 0 at both levels; codegen keeps the call, as for the rest of
  the set). Tests: `tests/conformance/c.math/constexpr-sqrt*.cpp`,
  `gnu-lib-sqrt.cpp`.
- **Sign, min and max** (`bi::FP_OP`; WP-403 for the `f16` and
  `_num` forms): `fabs`, `copysign`, `fmax`/`fmin` (each spelling,
  `__builtin_fabsf16`/`fmaxf16`/`fminf16` through `f16_twin`) and the
  clang 22 `fmaximum_num`/`fminimum_num` in every spelling (`f16`, plain,
  `f`, `l`, `f128`; WP-410 for plain/`f`/`l`, WP-419 for `f128`), bit-exact as clang folds
  them: `fmax`/`fmin` below `-fclang-compat=22` (clang 19 `APFloat::
  maxnum`) yield the other operand for a NaN (`-0 < +0`); at 22
  (`FMAX_22`/`FMIN_22`, LLVM 22 `maxnum`) a signaling NaN `a`, else a
  signaling NaN `b`, yields itself quieted (`softfloat::quiet`) first;
  `maximumnum`/`minimumnum` yield the other operand for a NaN, quieted
  when both are NaN. `__builtin_canonicalize{,f,l,f16}` and the
  `F16_LIBM` set (`sin`, `pow`, `atan2`, `sincos`, ...) are not
  constants (the generic note), as in clang
  (`tests/conformance/c.math/f16-builtins*.cpp`, `constexpr-f16*.cpp`).
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
- **`__builtin_start_lifetime(p)`** (WP-773, `bi::START_LIFETIME`;
  P3726R2, GCC's builtin behind libstdc++'s `std::start_lifetime`): one
  instruction; `t1` is `p`'s designator type (the pointee; `DYN`, a
  `void *` with no tag, is invalid, which sema already rejects). It
  checks `p`'s storage as a placement new would, makes the member path
  to it active without activating an array member it is an element of,
  and revives ended tracked storage with uninitialized bytes; see "C++26
  union lifetime" below. It has no value.
- **`__builtin_clear_padding(p)`** (WP-774, `bi::CLEAR_PADDING`,
  `Vm::clear_padding`; GCC's builtin): one instruction, with `t1` taken
  from `p`'s designator as for `START_LIFETIME`. GCC rejects the call in
  a constant expression; kiln accepts it as an extension, so that
  libstdc++'s `constexpr atomic(T)` (which clears `T`'s padding when
  `__has_builtin` says it can) still initializes constants. The Oven
  does not model padding bits: they stay indeterminate, so a later
  `bit_cast` of them is still non-constant. The call checks that `p`
  designates storage the evaluation may write (`check_storage` for
  `sizeof(T)` bytes, then the write rules of an assignment, with access
  kind `ASSIGN`), and otherwise does nothing. A null, one-past-the-end,
  freed or out-of-evaluation object (a global not under construction)
  gives the matching "assignment to ..." / "cannot modify an object
  that is visible outside" note. It has no value. Tests:
  `tests/conformance/ext.gnu.clear_padding/clear-padding-constexpr*.cpp`.
- **`__builtin_is_within_lifetime(p)`** (WP-418, `bi::LIFETIME`;
  P2641R4, clang 20 `IsWithinLifetime`): a null, function or
  one-past-the-end `p` fails with `'__builtin_is_within_lifetime' cannot
  be called with a {null|function|one-past-the-end} pointer` at the call
  (`'std::is_within_lifetime' ...` at the `std::` call when the builtin's
  frame is that function's); then `p`'s storage is checked as a read of
  its designator type (static, or a `void *`'s conversion tag), so a
  destroyed, deleted or out-of-scope object faults as a read outside its
  lifetime, as clang. Otherwise the result is true unless a union on the
  path from the allocation to `p` has no active member reaching it
  (`union_path`). An Oven pointer carries no designator, so two members of
  one union at the same offset and type are told apart statically: when
  the argument is `&a.b.c` (through pointer casts) rooted at a variable or
  `this`, each union member access in the chain emits `bi::UNION_LIVE`
  (sub = member index; `mem.may_be_active`), ANDed into the result
  (`union_designators`). A call of `std::is_within_lifetime` with such an
  argument ANDs its own result with the same chain (its parameter loses
  the designator). Known gap: a pointer to an inactive member stored in a
  variable first (`int *p = &u.a; is_within_lifetime(p)`) reads as within
  its lifetime when the active member has the same offset and type
  (clang: false): the Oven pointer would need a member designator that
  survives loads, stores and arithmetic (follow-up). A subobject of an
  object under construction whose lifetime has not yet begun is not
  within it (clang: the subobject has no value yet; WP-445): each
  compiled constructor that initializes subobjects records `(pc, i)`
  steps in `Oven::ctor_steps` (from `pc` on, entries `..i` of the
  class's non-virtual `fields` have begun, entry `i` is being
  initialized, `u32::MAX`: all), and `not_yet_begun` walks the
  constructor frames on the stack whose `this` holds `p` (the frame's
  pc is its callee's `call_pc`): a subobject after entry `i`, or entry
  `i` itself when it is a non-class, non-array member, answers false. A
  base or class member being initialized is left to the frames above it;
  a member the mem-initializer list skips has begun (clang
  default-initializes it). Nothing is emitted, so the check costs no
  instruction (`meta.const.eval/is-within-lifetime-construction.cpp`).
  A complete-object constructor (P3533R2) first records
  `(pc, CTOR_STEP_VBASE | j)` before each virtual-base initializer: while
  virtual base `j` is initialized, later virtual bases and all
  non-virtual subobjects have not begun; from the first non-virtual step
  on, the virtual bases have. A base-subobject constructor initializes
  none and records no such step (WP-465;
  `meta.const.eval/is-within-lifetime-virtual-base-cxx26.cpp`, kiln only).
  A pointer into the variable whose initializer is being evaluated
  (`RunState::object`, clang's `EvaluatingDecl`) fails with `... cannot be
  called with a pointer to an object whose lifetime has not yet begun`
  (`BiFault::Lifetime` kind 3;
  `meta.const.eval/is-within-lifetime-not-begun-errors.cpp`). That
  includes the RESOLVING placeholder a self-reference such as `constexpr
  C c(&c)` reads while `c` is evaluated (`Origin::Global` of the same
  first declaration; a local constexpr variable mid-initialization
  resolves to such an object too, kept in `provisional` for that
  evaluation only), so a consteval constructor's immediate invocation
  handed `&c` fails the same way (WP-465;
  `meta.const.eval/is-within-lifetime-consteval-var.cpp`).
  A member list-initialized by its mem-initializer (an `InitListExpr` or
  parenthesized list of a non-union class or array) records a step per
  element, recursively for an element that is such a list itself
  (`CTOR_STEP_ELEM | i`, the element's offset from `this` and size in
  `CtorStep::off`/`len`; `CTOR_STEP_RECORD` for an element of class
  type): while an element is initialized, a subobject ending before it
  and an aggregate holding it have begun; the element itself, everything
  after it (explicit, omitted or array-filler elements: clang initializes
  them in order) has not; inside an element of class type the frames
  above decide (WP-465;
  `meta.const.eval/is-within-lifetime-member-list-init.cpp`).
  Known gaps: `this` inside a consteval constructor whose immediate invocation
  initializes a variable, member, parameter or local temporary is the
  thunk's temporary, so it is answered where clang reports kind 3
  (clang accepts it when the invocation is nested in a temporary, member
  access or comma of a static-storage initializer).
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

- **Landed (WP-270): checked arithmetic** (`builtin::CHECKED`). The
  generic and typed `__builtin_{add,sub,mul}_overflow` forms are
  evaluated, as clang 19's `ExprConstant`: the compiler evaluates both
  operands, then the result pointer, then emits `bi::OVERFLOW` twice,
  once with the `VALUE` flag for the wrapped result and once for the
  flag, and stores the value through the pointer with the ordinary
  assignment checks (a null pointer is "assignment to dereferenced null
  pointer", an object from outside the evaluation "cannot modify an
  object that is visible outside that expression"). The sub-operand
  packs the operation (0 add, 1 sub, 2 mul), each operand's signedness,
  the result's signedness and width; operands arrive canonical (sign- or
  zero-extended to 128 bits). The VM computes in sign-magnitude with a
  checked `u128` magnitude, so any `__int128`/`unsigned __int128` mix is
  exact: overflow is a magnitude past `u128` or a value outside the
  result type; the stored value is the wrapping operation truncated to
  the result width (`arith::canon`). **Deliberate divergence:** for a
  `bool` result clang 19's evaluator truncates to 8 bits (it stores 2 for
  `1 + 1`), while its codegen and GCC use 1 bit; kiln uses 1 bit in both
  (value `0`, overflow `true`).
  WP-557: the carry builtins `__builtin_{add,sub}c{b,s,,l,ll}`
  (`builtin::carry_op`; clang 19 `ExprConstant` `BI__builtin_addcb`..)
  evaluate the three operands, then the carry-out pointer, then emit
  `bi::CARRY` (kbc 34) twice: once for the carry-out, stored through the
  pointer with the ordinary assignment checks as above, and once with
  `CARRY_VALUE` for the call's value. The sub-operand is the width |
  subtract << 8; the VM does two wrapping steps in the width, `x op y`
  then `op carryin`, and the carry-out is 1 when either wrapped (the
  carry-in is any value of the type, not only 0 or 1;
  `tests/conformance/builtin.carry/carry-constexpr.cpp`).
  `__builtin_alloca`, `__builtin_alloca_with_align`, `__builtin_vsnprintf`,
  `__builtin_snprintf` and `__builtin_sprintf` trap "subexpression not
  valid in a constant expression", as clang 19, and so do the other
  `printf`/`scanf` family builtins (WP-841).

- **Landed (WP-282): library builtins.** `__builtin_object_size` and
  `__builtin_dynamic_object_size` are `bi::OBJECT_SIZE` (kbc 22, `SIDE`
  1: the operand is evaluated only for its pointer value, clang's
  `tryEvaluateBuiltinObjectSize`): for a pointer into a known complete
  object the bytes from the pointer to the end of the whole object
  (types 0/2) or of the closest subobject (types 1/3), 0 past the end;
  a null or function pointer or an object of incomplete type is not a
  constant (clang 19's `ConstantExpression` mode). WP-292: the operand
  is looked through parentheses and no-op / bit casts of pointer
  prvalues (clang `ignorePointerCastsAndParens`, so `(const char *)&s`
  designates `s`) and evaluated speculatively (`Op::Speculate`). clang
  evaluates it ignoring side effects: a comma's left operand is an
  ignored value whose failure is noted, after which any lvalue-to-rvalue
  read fails (and a call with arguments). So the left operands of commas at any depth (WP-302:
  `(x, (y, arr))`, `&a[(f(), 1)]`, `b + (f(), 2)`) with no read after
  them in evaluation order (reads in skipped left operands not counted)
  are skipped (`skipped_comma_lhs`; the compiler's `skip_effects`), and
  the others are evaluated with the operand; an operand with a
  remaining side effect gives the unknown value (-1 for types 0/1, 0 for
  2/3) without being evaluated, as clang fails the modification of an
  object from before the trial (register-promoted locals are not rolled
  back by `Speculate`, so this is decided statically); otherwise a
  failed trial gives the unknown value when the operand has side effects
  in clang's `HasSideEffects` sense (a call to a function that is not
  `const`/`pure` included, `SIDE` set) and the note "subexpression not
  valid in a constant expression" at the call otherwise.
  WP-808: a string literal has no whole-object type (clang
  `getObjectType`: a variable, a compound literal or a dynamic
  allocation), so types 0/2 of a pointer into one are not a constant;
  types 1/3 use its array designator. `Oven::fold_object_size(p, t)` is
  codegen's `tryEvaluateObjectSize` of a `pass_object_size` argument
  (`Job::ObjectSize`, compiled by `compile_object_size` as the same
  speculative region without the side-effect default): the size, or
  `None` for any failure, silently and with the trial's effects
  reclaimed.
  WP-1073: an operand based on the result of a call to a function with
  `alloc_size` (written, or sema's implicit one on `malloc` / `calloc` /
  ...; clang `tryEvaluateBuiltinObjectSize`'s `InvalidBase` path,
  `getBytesReturnedByAllocSizeCall`) is decided statically by the
  compiler (`alloc_size_object`) before the trial: the base is the call
  itself or a local `const` pointer variable initialized by it (through
  one cast); the size is the product of the constant size arguments
  (integer literals, `constexpr` variables, `sizeof`, `+ - *`; any
  overflow of `size_t` gives up), and `+` / `-` of a constant moves the
  offset by that many pointees. The result is size minus offset, 0 when
  the offset is negative or past the end. A cast to a non-`void`
  pointer type inside the operand (below the outer casts stripped
  above) invalidates the designator, so type 3 then falls back to the
  trial, as in clang. A non-constant size falls back to the trial.
  The alignment builtins on integers (kbc 26, `ALIGN`, WP-416; clang
  `ExprConstant` `BI__builtin_align_up` / `_down` / `__builtin_is_aligned`):
  the alignment is checked first, at its own location: one that is not a
  positive power of 2 fails with "requested alignment V is not a positive
  power of two", one above 2^(N-1) for an N-bit source with "requested
  alignment must be M or less for type 'T'; V is invalid"; then
  `(x + mask) & ~mask`, `x & ~mask` or `(x & mask) == 0`, wrapped to the
  source type. On a pointer source (kbc 28, `PTR_ALIGN`, WP-432; clang
  `PointerExprEvaluator` / `IntExprEvaluator`, `getBaseAlignment`) the
  known alignment is the complete object's (a variable's
  `SemaBridge::decl_align`, else its type's, a heap or temporary object
  its type's) at the pointer's byte offset (`MinAlign`).
  `__builtin_is_aligned` checks the alignment first, then evaluates the
  pointer: 1 when the known alignment is at least the requested one, 0
  when the object's is but the offset's is not; `align_up` / `align_down`
  evaluate the pointer first: the pointer itself when its known alignment
  suffices, else (the object's suffices) the pointer with its byte offset
  rounded, by element steps when the change is a multiple of the element
  size (one past the end included); clang does not bound the new offset.
  Otherwise (the object's alignment is less, a null, integral or function
  pointer) the call fails with "cannot constant evaluate whether
  run-time alignment is at least A" / "cannot constant evaluate the
  result of adjusting alignment to A" at the pointer
  (`BiFault::AlignRun`). Deliberate divergence: clang moves only the
  offset and keeps the old designator, so reading through an adjusted
  pointer reads the old element in clang (`*__builtin_align_up(&a[3], 8)`
  is `a[3]`); the Oven reads the element at the new offset, as the
  program does at run time. Test
  `ext.gnu.builtins/align-pointer-constexpr.cpp`.
  `__builtin_assume_aligned` (kbc 23, `ASSUME_ALIGNED`, sub 1 with an
  offset) evaluates to its pointer operand after clang's check: the
  complete object's alignment (`SemaBridge::decl_align`, clang
  `getDeclAlign`: the declared `alignas`/`aligned`, else the type's, a
  reference as a pointer, an x86-64 array of 16 bytes or more at least
  16; a heap or temporary object its type's) must be at least the
  asserted one ("alignment of the base pointee object (N bytes) is less
  than the asserted M bytes"), and the byte offset minus `off` a
  multiple of it ("offset of the aligned pointer from the base pointee
  object (N bytes) is not a multiple of the asserted M bytes"; without
  an object, "value of the aligned pointer (N) is not a multiple ...");
  both notes are at the pointer operand. `__builtin_shufflevector` (`vec_shuffle`)
  concatenates the two operands' lanes and picks by index; a -1 lane is
  the note `index for __builtin_shufflevector not within the bounds of
  the input vectors; index of -1 found at position N is not permitted in
  a constexpr context`, which clang emits as an error-level note
  (`Note::is_error`, spec/diag.md §6). The two-argument mask form is not
  a constant (generic note; clang 19 crashes on it). The `*f128` builtins with a `long double` twin
  evaluate as the twin (quad-precision `softfloat` arithmetic), and so
  do the GCC `_FloatN` constants libstdc++ `<limits>` names
  (`__builtin_{huge_val,inf,nan,nans}f{16,32,64}`, `__builtin_nansf16b`;
  WP-361, `kiln_ast` `builtin_ids::FLOATN`), converted to the result
  type; `__builtin_nextafterf16`/`f16b` evaluate as the P0533 set above
  (and lower to an inline bit-level sequence, spec/types.md);
  `powi`, `verbose_trap` and the non-P0533 `*f128` names trap
  "subexpression not valid", as clang 19. **Divergences:** for the
  object-size comma rule a call with arguments counts as a read (its
  callee reads them) and one without as none, whatever the callee reads.

#### `_Float32x`/`_Float64x` math builtins (WP-393)

GCC spells its TS 18661-3 math builtins for `_Float32x` (double's
format) and `_Float64x` (long double's) with `f32x`/`f64x` suffixes.
kiln provides the set clang 22 provides plus those g++ 14 provides that
libstdc++'s headers reference under kiln's flags. Both are empty:
`clang++-22` and `clang++-19` answer `__has_builtin` 0 for every name
below (and have no `_Float32x` type), libstdc++ 14 names none of them,
and glibc's `<math.h>` (`bits/floatn-common.h`), which names the
constant forms, maps them to the `double`/`long double` builtins because
kiln reports clang's `__GNUC__` 4. So kiln declares none, and code
reaches the formats through the `double`/`long double` forms by
conversion, as for `_Float32`/`_Float64` above.

| Family (each `__builtin_<f>f32x` and `__builtin_<f>f64x`) | kiln |
|---|---|
| `fabs`, `copysign`, `fmax`, `fmin` | not provided (clang 22 doesn't either) |
| `ceil`, `floor`, `trunc`, `round`, `rint`, `nearbyint` | not provided (clang 22 doesn't either) |
| `sqrt` | not provided (clang 22 doesn't either) |
| `nan`, `nans`, `inf`, `huge_val` | not provided (clang 22 doesn't either); glibc's `HUGE_VAL_F32X`, `SNANF64X`, ... macros use the `double`/`long double` forms |
| `fma`, `fmod`, `frexp`, `ldexp`, `nextafter` and the rest of GCC's set | not provided (clang 22 doesn't either) |

`__builtin_isnan`/`isinf`/`isfinite` and the other type-generic builtins
take `_Float32x`/`_Float64x` operands directly. Test:
`tests/conformance/basic.extended.fp/float-nx-builtins.cpp`.

### Memoization — normative for M2
- A function is memoized when its parameters (at most 4) are integral,
  `bool`, floating or `std::meta::info` (an id's bits: spec/reflection.md
  §3.6, WP-470) and its result is such a scalar (not a pointer, not
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
  cache on or off (tested), except `memo_hits`, which counts the hits
  ("Reporting protocol").
- A call that ran a reflection intrinsic whose purity class is not
  `Stable` is not cached (spec/reflection.md §3.3: the intrinsic bumps
  `run.impure`).

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
with a bumped generation; a slot whose generation would reach the last
one (never issued: `mem::NOWHERE`) is retired.
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
    placement new expression is not supported in constant expressions "
    (clang 22's wording, trailing space included). The reserved form
    `new (p) T` / `new (p) T[n]` is constant in C++26 (P2747,
    `__cpp_constexpr` 202406L, clang 22's `VisitCXXNewExpr`) and,
    before C++26, inside a function of namespace `std`
    (`std::construct_at`, [expr.const]/5.18; WP-212, below); elsewhere
    before C++26 it is "this placement new expression is not supported
    in constant expressions before C++2c" (WP-357, superseding the
    clang 19 rule that it was never constant in user code);
  - `PASS_ALIGN` and `operator_delete` are codegen facts and do not
    affect evaluation.
- `CXXDeleteExpr` (`ty` = `void`): `delete_operand(e)` is the pointer
  prvalue; `ARRAY_FORM` selects `Delete { array: true }` (also set for a
  `delete` of a pointer to an array, which clang treats as `delete[]`;
  `ARRAY_AS_WRITTEN` says what was written); `operator_delete(e)` must be
  a replaceable global deallocation function for the delete to be
  constant ([expr.const]); `delete_passes(e)` (size, alignment) is
  a codegen fact. A class's destroying `operator delete`
  ([expr.delete]/10) is class-specific, so the delete is not constant
  ("call to class-specific 'operator delete'" at the `delete`), directly
  or through a virtual destructor, as clang 22 and 19 do (WP-457;
  `tests/conformance/expr.const/constexpr-destroying-delete.cpp`).

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
| Other placement forms | "this placement new expression is not supported in constant expressions " (clang 22) | — |
| Reserved placement form before C++26, outside namespace std | "this placement new expression is not supported in constant expressions before C++2c" | — |
| Default-initialized scalar read | "read of uninitialized object is not allowed in a constant expression" | — |

**Uninitialized reads (C++26 erroneous values, WP-448).** P2795R5 gives
a default-initialized automatic object an *erroneous* value at run time
([basic.indet]), but in constant evaluation the object is still
uninitialized: reading an erroneous or an indeterminate value is not a
core constant expression ([expr.const], the lvalue-to-rvalue bullet),
so the Oven keeps the row above, with clang's note, for both. The
[basic.indet]/2 exemption that lets an `unsigned char` or `std::byte`
glvalue read such a value at run time does not reach constant
evaluation: copying an uninitialized `unsigned char` or `std::byte` is
the same note (as clang). A literal reading of the C++26 wording could
let such a copy produce an erroneous value without an operation "that
would have erroneous behavior"; kiln follows clang and rejects it. A
`[[indeterminate]]` variable is read the same way. Test:
`tests/conformance/basic.indet/constexpr-erroneous.cpp`.

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
  `__builtin_operator_new(n)`), or of a global type-aware one usable in
  constant evaluation (P2719, `::operator new(std::type_identity<T>{}, n,
  al)`, whose `type_identity` argument is evaluated for effect and whose
  size or pointer is the next argument; WP-1112), is constant only when the function being
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
- *Construction.* A placement `new (p) T(args)` or `new (p) T[n]{...}`
  whose allocation function is the reserved placement form
  (`operator new(size_t, void*)` / `operator new[](size_t, void*)`) is
  constant in C++26 (P2747, WP-357; clang 22) and, in earlier modes,
  inside a function of namespace `std` (clang's `isStdFunction`:
  `std::construct_at`, `std::allocator_traits::construct` and the
  library internals). It lowers to `Placement [p, T, stat, n, k]`, which
  checks the storage with the access checks minus the lifetime check
  (null: "construction of dereferenced null pointer ..."; deleted: "...
  heap allocated object that has been deleted"; one past the end; a
  `const` object: "modification of object of const-qualified type ...";
  an object outside the evaluation: "a constant expression cannot modify
  an object that is visible outside that expression") and that a `T`
  object can live at `p`: the most-derived object at that offset of the
  allocation's type must be `T` or contain a `T` subobject there, else
  "placement new would change type of storage from 'U' to 'T'". `stat`
  is the static pointee type of the placement argument before its
  conversion to `void *` (`bi::DYN` when it was `void *` already). As in
  clang 22's `HandlePlacementNew`, the designated object is the `stat`
  subobject at `p` (for `bi::DYN`, the subobject the `void *` was made
  from, below, else the outermost object there; inside
  `std::construct_at` and libc++'s `std::__construct_at` the compiler
  passes `T` instead of `bi::DYN`, since [specialized.construct]/2
  constructs `*location` and libc++ hands the placement a `void *` from
  `__voidify`, so a first member or `optional`/`variant` storage at
  offset 0 is the designated object, `construct-at-member.cpp`), and it
  may hold the new object when its array extent is at least the new
  type's and their element types are the same: `new (a) int[3]` with
  `int a[3]` decayed designates one `int` ("... from 'int' to
  'int[3]'"; `new (&a) int[3]` is the valid spelling), and `new (&a)
  int[n]` into `int a[2]` with `n == 3` is "... from 'int[2]' to
  'int[3]'". An array placement (`new_aux::ARRAY`) first checks its
  bound `n` as an array `new` does, against `k` explicit initializers
  (`new_aux::BOUND`). It then
  begins the lifetime of `sizeof(T)` bytes, marks them uninitialized, and
  the initializer runs as for any `new` (a constructor call for a class).
  The designator survives a `void *` that a user function returns
  (WP-378): a conversion of a non-null `T *` (`T` not `void`) to `void
  *` lowers to `VoidCast [dst, p, T]` aux `kbc::VOID_TAG`, which records
  `T` (+ 1) in bits 64..96 of the pointer register, and `Placement`
  with `bi::DYN` designates the `T` subobject at `p` when there is one.
  `constexpr void *f(S &s) { return &s.first; }` then makes `new (f(s))
  int(5)` construct `s.first`, a base `static_cast<B *>(&d)` a `B`, and
  `new (f(s)) S{}` is "... from 'int' to 'S'", as in clang 22, whose
  `void *` keeps the full lvalue designator; a `void *` made from `&s`
  designates `s` (`expr.new/p2747-placement-voidified-subobject*.cpp`).
  Copies, calls, returns and register locals keep the tag; every pointer
  operation reads the low 64 bits. A store of a tagged pointer to memory
  (a `void *` member or array element, a `void *` local whose address is
  taken, an object copy) keeps it with the granule's pointer mark (WP-387,
  WP-395), and a load of a pointer whose granule bears a tagged mark puts
  the tag back in bits 64..96 (`Memory::set_void_tag`,
  `Memory::void_tag`). The first 253 distinct tag types are interned
  TU-wide (append-only, so a rolled-back or copied mark stays valid):
  mark `PTR_TAGGED + i` names the `i`-th, and a `void *` costs what it
  did before, 8 bytes plus the granule's existing mark byte. A pointer of
  any further type is marked `PTR_SPILL` and its tag kept in a map keyed
  by space and granule (`Memory::vspill`), which a trial's rollback
  restores (`Undo::Gran` carries the spilled tag) and `Memory::copy`
  carries with the marks. There is no cap and no fallback: the 300th
  distinct type is checked as the first is
  (`expr.static.cast/p2738-void-cast-300-types.cpp`). A map for every
  tag (no table) measured 3.5% slower on a `void *` store/load loop (15M
  tagged stores and loads: 1.10 s table, 1.17 s map, 1.09 s table plus
  spill, medians of 11) and within noise on `bench/oven`; the table
  keeps the common case to a byte. A frozen `void *` carries its tag as
  the pointer's designated type (`Scalar::Pointer::designated`,
  spec/ast.md §9; carried through the frozen value rather than rebuilt
  from the declaration and offset), and thawing puts it back in the register or the
  granule's mark: a `constexpr const void *g = &s.x;` read in a later
  evaluation designates `s.x`, not `s`, with no allocation on the thaw
  path (the type is interned once). An untagged `void *` (a builtin's
  result) designates the outermost object at its address.
  The tag is a type, not a designator path. With the pointer's offset
  and past-the-end bit that names one subobject in every case the
  standard distinguishes, but a `void *` loaded from memory cannot tell
  which of two same-typed members of a union it came from, and an
  untagged `void *` falls back to the outermost object at its address
  (`expr.static.cast/p2738-void-cast-thawed.cpp` covers thawed
  constants: the complete object, a member, an array element, an empty
  base at offset 0 against the first member and a `[[no_unique_address]]`
  member sharing its address).
  The tag is the designated subobject's type; with the pointer's offset
  and past-the-end bit it names one subobject ([intro.object]/9: two
  distinct objects of one type have distinct addresses), except members
  of one type of a union, which the Oven's typed pointers do not tell
  apart either (`Memory::union_alts`). An empty base is a subobject of
  its class at its offset (`Types::empty_bases`, kept apart from
  `Types::fields` as it holds no storage), so it and an empty
  `[[no_unique_address]]` member at the same address are different
  designated objects, and constructing an object that holds no data
  (`Types::holds_no_data`) by placement touches no byte (the member
  sharing its address keeps its value)
  (`expr.new/p2747-placement-voidified-through-memory*.cpp`). The
  exception is an empty complete object or array element (libc++'s
  `vector<bitset<0>>`; see "Architecture" above): it owns its byte
  (`basic.life/constexpr-empty-allocator-storage.cpp`).
- *Casts from `void *`* (C++26, P2738R1, [expr.static.cast]/14): `VoidCast
  [dst, p, T]` aux 0 (a `static_cast` from `cv void *` to `cv T *`
  outside `std::allocator<T>::allocate`) succeeds for null and when the
  type the `void *`'s tag records is similar to `T` ([conv.qual]/2,
  `Types::similar`: equal but for the cv-qualifiers at each pointer or
  array level), as clang's check of `Designator.getType`; otherwise
  "cast from 'void *' is not allowed in a constant expression because
  the pointed object type 'U' is not similar to the target type 'T'",
  clang's wording: the operand type as written (`'CVP' (aka 'const void
  *')`), `U` with the qualifiers of the declaration naming the
  subobject (the variable through array elements, or the member; none
  for a base: `'const D'`, `'int'`), and `T` as written. So
  a `void *` made from a base subobject, a first member or an array is
  not the derived object, the enclosing object or the first element. An
  untagged `void *` succeeds when the allocation has a `T` subobject at
  its address (`expr.static.cast/p2738-void-cast-designator*.cpp`).
  A placement new allocates nothing: constructing and destroying objects
  in one storage in a loop holds that storage only
  (`expr.new/p2747-placement-churn.cpp`, 100000 objects, one heap
  allocation; WP-375).
  Within `std::allocator<T>::allocate` a cast from `void*` to `T*` (as
  libc++ writes `static_cast<T*>(__builtin_operator_new(...))`) is a plain
  move, not the C++26 `void*` cast check.
  Before C++26 ([expr.const]/5.14 of C++11 to C++23) every other cast
  from `cv void *` to an object pointer (`static_cast`, C-style,
  functional, in any function including a template instantiation) is
  not a core constant expression: the cast compiles to a null test and
  two traps at its begin location, "cast from 'void *' is not allowed in
  a constant expression in C++ standards before C++2c" for a non-null
  operand and "cast from 'void *' is not allowed in a constant
  expression" for null (the operand type as written, with its aka), as
  clang 19 and 22 in every mode. clang reports this as a CCE note and
  keeps evaluating; kiln stops at it in every evaluation but a fold,
  which gives the same diagnostics wherever clang needs a constant
  (`expr.const/p2738-void-cast-pre26-cxx*.cpp`).
  Probed (WP-407, clang 19 and 22, `-std=c++23`): a required constant
  expression keeps the first note (clang `EM_ConstantExpression` never
  replaces it), so a later fault (a read past the end through the cast
  pointer, in a callee or not) adds no second note; `static_assert`,
  `constexpr` variables, enumerators and array bounds print the same as
  kiln. In a *fold* clang continues with the cast's result designating
  nothing (`Designator.setInvalid()`), so a null test or comparison of
  that pointer folds while any access through it fails silently: a
  file-scope `int a[(const int *)vp ? 1 : 2]` is "folded to constant
  array", `__builtin_constant_p((const int *)vp == &n)` is true, and
  `int g = (const int *)vp != nullptr;` is a static initializer. Kiln
  models this with pointers that point at nothing (below):
  `expr.const/p2738-void-cast-pre26-fold-continues.cpp`.
- *Pointers that point at nothing* (WP-415): clang's other casts that
  only a fold may perform ([expr.const]/5.15: a `reinterpret_cast`
  between object pointer types other than the same type, a C-style cast
  doing the same, an integer-to-pointer and a pointer-to-integer
  conversion) note "reinterpret_cast is not allowed in a constant
  expression" (a `reinterpret_cast` naming one of those conversions;
  any other is that note and stops) or "cast that performs the
  conversions of a reinterpret_cast is not allowed in a constant
  expression" and continue in a fold the same way. They compile to
  `Cce [dst, src, trap]` (aux `kbc::cce::CHECK`, `PTR`, `INT_TO_PTR`,
  `PTR_TO_INT`), and the pre-C++26 `void *` cast's two traps are `Cce`
  too. Outside a fold `Cce` traps with its note, exactly the old `Trap`
  (for an integer/pointer conversion the note comes before the operand
  is evaluated, as in clang). In a fold (`RunState::fold`: a silent
  evaluation outside a constant context, i.e. `Oven::try_fold`,
  `Oven::fold_rvalue` and `__builtin_constant_p`'s speculation; never
  a constant initializer or a constant-destruction check, where clang
  requires no notes) `PTR` keeps null and marks any other pointer as
  pointing at nothing: its generation becomes the never-issued last one
  (`mem::NOWHERE`, which is why `GEN_MAX` is 510), so every access
  through it fails the generation check every access already makes, and
  the fast path costs nothing. `INT_TO_PTR` makes a nowhere pointer of
  slot 0 whose offset is the integer (0 stays null; one that does not
  fit 32 bits fails); `PTR_TO_INT` gives back that integer, or fails for
  a pointer to an object (clang keeps a symbolic value there; kiln's
  fold stops). Pointer arithmetic on a nowhere pointer moves its offset
  in element units; subtraction of two nowhere pointers of one slot is
  the offset difference over the element size; comparisons of two
  pointers of which one points at nothing compare offsets within one
  slot, and across slots fold only for `==`/`!=` (false/true) unless a
  side is an integer address other than null. The generation is lost,
  so pointers compare by slot. `__builtin_constant_p` of a nowhere
  pointer is true for an integer address and for a string literal at
  offset 0, as clang's base check. A fold that executed a `Cce` is
  impure (never memoized). `__builtin_constant_p((int *)8 + 1 == (int
  *)12)` is true, `int f[(long)(int *)8]` folds to 8 elements and
  `int i[*reinterpret_cast<const char *>(&n)]` is an error:
  `expr.const/5.15/reinterpret-cast-fold-points-nowhere.cpp`. A cast to
  or from a function pointer is `PTR` too (WP-492): the function pointer
  keeps its function (a nowhere pointer of the function slot), so it
  freezes to that function (`void (*p)() = (void (*)())&f;` is a static
  initializer) and compares by function; an indirect call through it
  calls the function only if the call's function type is the
  function's, ignoring the exception specification (clang
  `EvaluateCall`'s `hasSameFunctionTypeIgnoringExceptionSpec`;
  `Oven::cast_callee_matches`), so a pointer cast back to its type
  calls in a fold, and any other fails as clang's `Error`. A lax
  variable (spec "Lax folds") whose constant initialization failed is
  read as its initializer re-folded non-manifestly, as a
  potentially-constant one is (`const auto p = (int (*)(int))(void
  (*)())&f;` folds `p(1)` to a direct call): `VarDecl::evaluateValue`
  re-evaluates any variable without constant initialization in
  `EM_ConstantFold`.
- *Address constants* (WP-1295; C23 6.6p9, [basic.start.static]/2
  where clang's `ConstantEmitter` folds): a static's initializer that
  is an address constant only through casts is static data, never a
  guarded dynamic initializer. **Design choice:** the Oven, not a
  structural constant emitter in codegen, folds these. Clang's emitter
  is its evaluator's `LValue`/`APValue` (base plus offset) read
  non-manifestly, and aggregates, member initializers and
  address-plus-offset already go through the Oven's freezing; a second
  emitter in codegen would duplicate the evaluator for one shape and
  drift from it. The cost is two cold paths and no per-value state.
  Freezing a nowhere pointer (`value.rs` `nowhere_of`, reached only
  when the generation is `mem::NOWHERE`) recovers the pointee: a slot
  that is a static's storage gets its live generation back and freezes
  to `PtrBase::Decl` plus the byte offset, so `(char *)&arr[3]`,
  `(T *)&x + 1` and `(void *)fn` are relocations; slot 0 (an
  integer-to-pointer cast) freezes to `PtrBase::Absolute` (raw 0),
  which codegen emits as an integer constant or `inttoptr`. A pointer
  into a temporary or the heap still fails. A pointer-to-integer
  conversion keeps the address only at a *direct position*, the whole
  value of a variable's initializer or a scalar member's `InitStore`
  (`static intptr_t i = (intptr_t)&g;`, `struct { uintptr_t u; } s =
  {(uintptr_t)&g};`), only while `Oven::set_fold_addr_int` is on, and
  only for a pointer to a static or a function of the integer's width:
  `Cce` aux `kbc::cce::ADDR_INT` yields the raw pointer tagged with
  `kbc::ADDR_INT_TAG` (bit 64, never a canonical integer register), and
  `InitStore` with aux 1 stores it into memory as a pointer (pointer
  mark 1) of integer type, which freezes as a `Scalar::Pointer` of
  integer type (`walk_addr_int`). Any other use of the integer, such
  as arithmetic or a load as an integer, faults on the mark, so no
  integer the program computes with holds an address and the hot path
  is unchanged. Codegen turns the switch on around its lax
  variable-initializer refold; sema turns it on only for the C static
  initializer check, which reads success alone.
  `c/run/static-address-constant-forms.c`,
  `codegen/ptr_cast_static_init.cpp`,
  `conformance/expr.const.cast/reinterpret-address-not-constant.cpp`
  (a constant initializer still rejects every such cast).
- *C static initializers* (WP-1304; C23 6.6p5-p10, 6.7.11p4): every C
  file-scope or `static` variable's initializer is folded with
  `Oven::set_fold_c_static` on, by codegen (`fold::prepass`, the
  `Collect::c_static` set) and by sema's `check_c_static_init` (which
  also turns on `set_fold_float_ub` and `set_fold_leaks`, as codegen's
  fold has them). C++ never sets it. The flag folds what clang's
  initializer evaluation folds, each on a cold path, so ordinary
  evaluation is unchanged:
  - floating division by zero and invalid operations give the IEEE
    result (`fold_float_ub`): `1.0 / 0.0`, `0.0 / 0.0`, `HUGE_VAL`,
    `NAN` (the C++ rejection, [expr.const]/10.7, is unchanged);
  - `__builtin_constant_p` of a non-constant is 0 (a constant context,
    clang `hasConstantInitialization`);
  - a C compound literal of scalar, structure or vector type is read as
    its initializer whatever its qualifiers (`value.rs` `var_info`); a
    non-`const` array literal is not;
  - the member of a null pointer goes on with its offset (`(size_t)&((T
    *)0)->m`, `Op::NullField` aux 0), a pointer that points at nothing;
  - the difference of two pointers into one complete object that are
    not elements of one array is their byte-offset difference over the
    element size (`vm.rs` `c_static_sub`: `s.f - &s.b`);
  - a pointer-to-integer address may sit under integral conversions of
    the same width and additions and subtractions of integers
    (`compile.rs` `addr_int_inner` records them; `Op::AddrInt` with
    `kbc::addr_int::{CAST, ADD, SUB}` moves the displacement, bits 66..
    of the tagged register, and the freeze adds it to the offset:
    `(unsigned long)&_text - 0x10000000L - 1` is `ptrtoint (gep i8
    @_text, -0x10000001)`); a displacement stored to memory is kept in
    `Oven::addr_int_disps` by space and byte;
  - an address the pointer encoding cannot hold, an integer cast to a
    pointer that does not fit 32 bits (`(void *)-1`), a function plus an
    offset (`(char *)f + 1`) and a static's address past its end
    (`data + 128`), is a *synthetic address*: a nowhere pointer of
    `mem::SYNTH_SLOT` whose offset indexes `Oven::synth` (base pointer,
    byte displacement), frozen as the base plus the offset
    (`Function` bases may then carry an offset, spec/ast.md);
  - an array member without a bound decays (`&c.sc[0]`): clang's note is
    a CCEDiag, so the decay is a `Cce` check that only a fold passes;
  - a label address difference (Label addresses, below).

  Still not constant, as clang: a call, a read of a non-`const`
  variable, an automatic object's address, an address cast to a
  narrower integer, an address multiplied or subtracted from, two
  objects' addresses subtracted, and at `-fclang-compat=19` a vector
  lane read (`c/decl/static-init-not-constant{,-forms,-19,-22}.c`).
  Tests: `c/run/static-init-{float-ieee,builtin-constant-p,
  compound-literal-values,pointer-difference,address-integer-arith,
  address-offset}.c`.
  A conditional whose condition (through parentheses and casts) is a
  `__builtin_constant_p` call is a fold in every evaluation, a constant
  initializer's included (clang `VisitConditionalOperator`'s
  `FoldConstant`, a GNU extension, GCC PR38377; WP-802): `Op::Fold`
  (aux 1) saves `RunState::fold` in a register and sets it before the
  condition, and `Op::Fold` (aux 0) restores it after the chosen arm,
  in the rvalue, glvalue and initialization lowerings. So
  `__builtin_constant_p(E) ? E : E` is a constant expression where `E`
  is a glvalue `reinterpret_cast` (which outside a fold is
  `Cce`'s "reinterpret_cast is not allowed in a constant expression":
  `expr.const/5.15/glvalue-reinterpret-cast-bcp-conditional.cpp`). A
  fold failure inside such a conditional reports the failing access's
  note, where clang keeps the first `Cce` note (a wording difference
  only).
  Codegen's folds
  share the TU's bytecode, so they see the TU's language mode
  (`Oven::set_cxx_year`, recorded by sema; before WP-407 they compiled
  with C++26 rules and constant-initialized `int g = *(const int *)vp;`
  in C++23: `expr.const/p2738-void-cast-pre26-static-init.cpp`,
  `p2738-void-cast-static-init-cxx26.cpp`).
  A `void *` argument in an "in call to" note prints the subobject its
  tag designates (`f(&s)`, not the first member at its address), an
  untagged one the whole object.
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
  A class holding no data (`Types::holds_no_data`) copies no byte and
  reads nothing from its source, which is addressed, not read (clang's
  `isReadByLvalueToRvalueConversion`): copying an `extern const` empty
  object such as libc++'s `std::placeholders::_1` is a constant (WP-276,
  `tests/conformance/expr.const/trivial-copy-empty-class-no-read.cpp`).

**Object lifetime outside `std::allocator` storage — landed (WP-222).**
`EndLife` aux 2 on a frame or `new` allocation not yet tracked makes it
`LIFE`-tracked (`mem::flags::LIFE`, `Memory::set_lifetime`): one
`trial_write` of the allocation, the flag, `Space::life` started for its
space if empty, every byte of the allocation marked within its lifetime,
then the destroyed object's bytes marked outside it. From then on
`Memory::check` consults the bytes as for `STD_ALLOC` storage (the
`flags::TRACKED` mask): an access that passes the inlined plain checks
(`Memory::check_plain`) in tracked storage tests `Memory::live_at` (the
space's lifetime bits, out of line, a `bool` result) and only on a dead
byte runs `Memory::check_slow`, which reports the fault in the usual
order (WP-760: an `inplace_vector`'s frame object is tracked after its
first `pop_back`, so all its later accesses pay this test), so a second explicit destruction, a read, an
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
destroys them there, [expr.call]/6; `Memory::life_hi` is one past the
greatest `Memory::live` index of a `LIFE`-tracked frame object, raised
by `set_lifetime` and lowered by the frame release, and a scope whose
mark is at or above it releases no tracked object, so the callers of
`Oven::scope_life` skip it inline, `Memory::life_above`, WP-760); and
`delete`/`delete[]` before the
destructors and the deallocation. Arrays are checked element by element
from the last, so the note names `a[1]` or `{*new S[2]#0}[0]`. A
variable's note is at its declaration, others' at the destroying
expression. A class containing a union is never diagnosed as destroyed
at its implicit destruction (ending its active member's lifetime, as
`std::optional::reset` does, leaves the union without an active member,
which the bytes cannot tell from a destroyed object)
(`tests/conformance/basic.life/constexpr-lifetime-local.cpp`).
The union's side-map entry tells the two apart for the union itself
(WP-334): destroying the active member (`u.t.~T()`, `std::destroy_at`)
ends the member's bytes but keeps the union's entry, so the union is
still within its lifetime ([class.union.general]/6) and may be destroyed
explicitly (`EndLife` aux 1 accepts dead bytes of a union with an entry;
libc++'s `expected::__destroy_union` destroys the member, then the union,
then constructs it anew); destroying a union, or an object containing
unions (`EndLife` aux 2, one `has_union` test per destruction plus a
walk of `upos` when set), drops their entries, so a member access of a
destroyed union reports the lifetime fault, not "no active member"
(`active_check`), a second destruction is "destruction of object outside
its lifetime", and assigning a member (`SetActive`, `revive_union`)
re-begins the union's lifetime in tracked storage only while the entry
exists; otherwise the assignment is "assignment to object outside its
lifetime" ([basic.life]/7)
(`tests/conformance/class.union/constexpr-destroy-member-then-union.cpp`,
`tests/conformance/basic.life/constexpr-lifetime-union-ended.cpp`).
No new memory state: the entry already existed and is removed with the
destruction. Clang 19 accepts destroying a union with a user-provided
destructor twice and reading a member after it; kiln rejects both.

**C++26 union lifetime (P3726R2, WP-743; `Oven::cxx_year >= 2026`).**
A union whose trivial default constructor ran (P3074R7) is within its
lifetime with no active member: its bytes are uninitialized and it has
no side-map entry, so reading any member is "read of member 'i' of union
with no active member". Four rules differ from earlier modes. Earlier
modes keep the WP-334 behaviour above.
- **Destroying the active member** (`std::destroy_at(&u.s)`,
  `u.s.~S()`, `EndLife` aux 2, `Vm::union_of_member`) leaves the union
  alive with no active member, as its trivial default constructor leaves
  it. The union's bytes become uninitialized (not dead), and its entry
  and nested entries are removed. The union may then be destroyed,
  assigned, or given a new member by construction. A member read is "no
  active member", and destroying the member again is "destruction of
  member 's' of union with no active member" (`EndLife` aux 1 checks the
  path as a `DESTROY` access).
- **Constructing an element of an inactive array member**
  (`std::construct_at(&u.arr[i], ...)`, `Vm::place_in_unions`,
  `Vm::array_element_at`) activates the array member. Its other elements
  stay uninitialized, and reading one is "read of uninitialized object".
  Each element's lifetime is its bytes', so `std::inplace_vector`-style
  storage (`union { T a[N]; }`) constructs and destroys elements one by
  one. Destroying an element (`EndLife` aux 2 on a non-member object)
  ends only that element's bytes. It costs nothing for ordinary arrays:
  the test runs only on the path through a union, once per placement
  new. Deviation from the DIS wording, which leaves the array object
  inactive until `std::start_lifetime(u.arr)` ([obj.lifetime]); kiln
  follows GCC 17, which activates it implicitly (decided in WP-773: code
  written for GCC 17 and libstdc++'s `inplace_vector` relies on it, and
  rejecting it buys no safety, since the element's bytes are still
  uninitialized).
- **`std::start_lifetime(r)`** (P3726R2, [obj.lifetime]; GCC's
  `__builtin_start_lifetime(&r)`, `bi::START_LIFETIME`,
  `Vm::start_lifetime`) begins the lifetime of the implicit-lifetime
  aggregate `r` and initializes nothing. Through a union it makes `r`'s
  member the active member (`place_in_unions` with `implicit = false`):
  an inactive member becomes active, and the previously active member
  ends without a destructor call, as for placement new. Unlike placement
  new it does not activate an array member when `r` is an element of
  one (or lies inside an inactive member at a deeper level): that is
  "construction of subobject of member 's' of union with active member
  'i'", GCC's "containing object not within lifetime". If `r` is already
  within its lifetime it is left as it is (constructed elements keep
  their values); otherwise tracked storage whose lifetime ended
  (`std::allocator<T>::allocate`, a destroyed object) is revived with
  its bytes uninitialized. Storage that is not writable (a constant, a
  pointer past the end) is the same fault as a placement new there.
  libc++ has no `std::start_lifetime` yet, so tests declare it over the
  builtin (`tests/conformance/obj.lifetime/`).
- **Unconstructed elements are not part of a value.** An element of a
  union's active array member that was never constructed is an inactive
  union elemental subobject ([class.union]/2, [expr.const]); so is an
  element of such an element (nested arrays recurse). Freezing a constant
  (`Oven::walk_elements`) and the trivial-copy initialization check
  (`Oven::first_uninit_elements`) skip an element none of whose bytes is
  initialized (`Space::any_init`); a partly initialized element is still
  "subobject 'b' is not initialized". The skipped element is not covered
  in the image, codegen emits it as zero bytes, and reading it (in the
  constant, or a copy of it) is "read of uninitialized object". The
  extra work is one bitmap scan per element, and only for a union whose
  array member is not wholly initialized. Deviation: an element that was
  constructed but holds only indeterminate bytes (`::new (p) int`) is
  indistinguishable from an unconstructed one and is accepted as a hole;
  GCC rejects it. Template-argument equivalence ([temp.type]) compares
  the image's cover, so two constants that differ only in which elements
  are holes compare by their constructed elements.
- **Element or member at offset 0.** A pointer has no designator, so
  `&u.s` and `&u.arr[0]` of one type are both candidates. The entry
  keeps the member and `union_alts` the array (WP-233). Constructing
  through the pointer again with the array possibly active collapses the
  union to the array. Destroying the member at the entry ends the
  union's member (above). Pointer arithmetic on such a pointer uses the
  array (`Types::extent_in` prefers an array member's extent inside a
  union, so `&u.arr[0] + 1` is `&u.arr[1]`).
- **Constructing over an active member of another type** does not end
  the old member's lifetime by running its destructor ([basic.life]/5:
  reusing storage ends a lifetime without a destructor call). Both
  members stay possibly active (`union_alts`). An allocation the old
  member owned is never freed, so a leak is reported as "allocation
  performed here was not deallocated" at the end of evaluation, as GCC 17
  reports it.

No new memory state: entries are removed when the member is destroyed,
and element lifetimes are the existing per-byte bitmap
(`tests/conformance/class.union/constexpr-trivial-union-lifetime-cxx26.cpp`,
`tests/conformance/expr.const/constexpr-trivial-union-invalid-cxx26.cpp`,
`tests/conformance/basic.life/union-member-destroyed-cxx26.cpp`,
`tests/conformance/obj.lifetime/start-lifetime-*.cpp`,
`tests/conformance/expr.const/constexpr-union-partial-array-*.cpp`).

An explicit destruction of a non-union class object whose union members'
bytes alone are outside their lifetime is accepted too (`EndLife` aux 1,
`live_but_unions`: every byte outside its `upos` unions must be live, and
there must be such a byte; a member constructed by `std::construct_at`
through a plain pointer has no union entry to consult): libc++ 22's list
nodes destroy the value of their anonymous union, then the node (WP-446,
`tests/conformance/class.union.anon/constexpr-destroy-member-then-object.cpp`).
A class all of whose bytes are union bytes keeps the stricter check.

An explicit destructor call written with a dependent destructor name
(`t.~T()`, `u.t.~T()`, `p->Base<T>::~Base()`) is rebuilt at
instantiation (spec/sema-templates.md §3.4) into the same
`CXXMemberCallExpr` / `CXXPseudoDestructorExpr` a non-dependent call
gives, so it ends the lifetime exactly as above: the scalar
`T t{}; t.~T();` with `T = int` is "destroying object 't' whose lifetime
has already ended" at scope exit, as clang does (WP-457;
`tests/conformance/expr.prim.id.dtor/dependent-member-destructor-name*.cpp`).

*Unconstructed bases (WP-390).* Sema leaves a direct non-virtual base
out of a constructor's ctor-initializers only after diagnosing that it
has no usable default constructor ([class.base.init]/9, clang's
`err_missing_default_ctor`); clang's evaluator still runs the
constructor, leaving the base subobject without a value. A
non-delegating constructor's prologue (`member_prologue`) emits
`EndLife` aux 8 for each such direct non-virtual base that holds data (a
virtual base is the most derived constructor's alone, "Virtual bases"): `Memory::set_unconstructed`
tracks the allocation in any space (a `constexpr` variable's static
storage too), marks the base's `copy_size` bytes uninitialized and
outside their lifetime, sets `life_frame` and records the base in
`Memory::unconstructed` (cleared by `reset`; entries a rolled-back
trial made stay, an error path only). Reading, assigning or calling a
member function of the base or its members is then "... object outside
its lifetime"; a destructor of the base is "destroying object 'd' whose
lifetime has already ended"; and freezing the object (`walk`) is
"constructor of base class 'B' is not called" at the base-specifier,
before its fields are visited. The derived object itself is not
destroyed: `life_ended` ignores dead bytes that belong to an
unconstructed base of the object (`Memory::holds_unconstructed`), so a
class whose only data is that base is destroyed silently, as by clang
(`tests/conformance/class.base.init/unconstructed-base-constexpr.cpp`,
`base-initialized-by-some-constructors-run.cpp`). An empty base
(`Types::empty_bases`, not in `fields`) has no bytes to mark: the
prologue emits `EndLife` aux 8|16 and `set_unconstructed` records it by
identity (slot, generation, offset, type; `Memory::empty_unconstructed`)
and tracks its allocation, so a member call on it is "member call on
object outside its lifetime", `life_ended` reports its destruction (a
constexpr destructor of the base), and the freeze notes the first
unconstructed base in base-specifier order, empty or not (WP-401;
`tests/conformance/expr.const/unconstructed-empty-base.cpp`). Such a
destruction names the derived object (the running destructor's `this`),
as clang prints the base's lvalue. A fault found while a destructor's
epilogue destroys a subobject (`PushCleanup` aux 2) is reported at the
outermost such destructor's call (`Oven::cleanup_fault_pc`): the
`delete`, the variable or the temporary, as clang's
`HandleDestructionImpl` passes its call range down
(`expr.const/unconstructed-base-destroy-location.cpp`). A destructor
with a body destroys a trivially destructible base too: its epilogue
registers a `PushCleanup` aux 2 with the `exc::LIFE_CHECK` sentinel
function (registered only while `Memory::unconstructed` is non-empty),
whose `run_cleanup` runs no function and checks the base's lifetime
(`cleanup_life`), so an unconstructed one is "destroying object 'd'
whose lifetime has already ended" in the destructor's frame, as
clang's `HandleDestructionImpl` checks every base for a value; a
trivial destructor destroys no base. A trivial copy (`Op::Copy` aux 4)
failing on the bytes of an unconstructed base of the source, directly
or in a member, is "constructor of base class 'B' is not called" at
that base-specifier (`Oven::unconstructed_base_of`; clang copies the
absent base and `CheckFullyInitialized` notes it), with the copy's
frame note (WP-409; `expr.const/unconstructed-trivial-base.cpp`).

*A virtual base without a mem-initializer (WP-400).* When the most
derived constructor names no mem-initializer for a virtual base, Sema
default-initializes it ([class.base.init]/9); a base whose default
constructor is trivial has its lifetime begun with indeterminate
contents, so it is *not* an unconstructed base: reading a member is
"read of uninitialized object is not allowed in a constant expression",
writing it first is fine, and a default member initializer of the base
runs. This is clang's behaviour for the non-virtual analog, so
`unconstructed_bases` deliberately leaves virtual bases out (test
`tests/conformance/class.base.init/vbase-default-initialized-cxx26.cpp`).

*Deviations from clang 19.* A class
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
  failure then answers 0, except in a lax fold outside a constant context
  (`Oven::fold_rvalue`, clang `EvaluateAsInt`/`EvaluateAsRValue`), where
  it fails the fold as clang's `EvaluateBuiltinConstantP` caller does
  (WP-403: `x && __builtin_constant_p(x)` gets no
  `-Wconstant-logical-operand`,
  `tests/conformance/expr.log.and/constant-operand.cpp`). A resource limit (steps, depth, memory) is still
  rolled back first, and then fails the evaluation.

**Divergence from clang 19.** A failed speculative evaluation that wrote a
heap object keeps that write in clang. In kiln it is rolled back
(`tests/conformance/expr.const/constant-p-trial-rollback.cpp`, which is
skipped for clang). Every other observable result matches clang
(`constant-p-trial.cpp`, which is strict-diags under both compilers).

`TrialMode::Transactional` allows every effect and undoes it on rollback.
It is the general mode for evaluator trials. Its user is
`std::meta::substitute`/`can_substitute` (WP-482): the reflection bridge
opens one with `Oven::trial_begin` around each uncached substitution and
closes it with `Oven::trial_end(keep)`, so a failed substitution's nested
constant evaluations leave no bytes behind
(`tests/conformance/reflection/substitute-trial-rollback.cpp`).
`Memory::in_trial` is true under it, but it is not speculative: a
metafunction call's `MetaCtx::trial` is `run.fold ||
Memory::in_speculative_trial()` (spec/reflection.md §4.3, WP-538), so the
nested evaluations it wraps (a variable template's initializer) run
their traits and instantiating queries; only `Mutating` ops are refused
under any checkpoint.

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
  the next outermost evaluation (the provisional objects themselves are
  kept and reused, "Unknown references");
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
- `eval_outermost` (`begin_outermost` / `end_outermost`, shared with
  `has_constant_destruction`) resets `Memory` at its end;
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
destruction, static temporaries). Virtual dispatch, `dynamic_cast` and
`typeid` are WP-077's, virtual bases WP-388's ("Virtual bases" below);
builtins are WP-088's.

**Object model.** A class object is one allocation of its record type;
`ty.rs` lays a record out as its direct non-virtual base subobjects
(`Field::base`, in declaration order, before the members), then its
fields, and then, for a class with virtual bases, one base field per
non-empty virtual base ("Virtual bases"), at the offsets of kiln_abi's
record layout, so a subobject is a
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
shares its offset with the next member). Outside a union, of two members
holding the byte the one at the greater offset wins: it lives in the
other's tail padding (a `[[no_unique_address]]` member or a non-POD base,
Itanium dsize), where the other holds no object (WP-289). Walking the
path to an object (`union_path`, `place_in_unions`) instead takes the
member that holds the object (`Types::member_toward`: the greater-offset
member if it does, else the first that does): a zero-dsize union of
`[[no_unique_address]]` empty members shares offset 0 with a
`[[no_unique_address]] bool` in libc++'s `expected<void, E>`, and a write
to the bool is not an access to the union ([intro.object]/9, WP-334,
`tests/conformance/dcl.attr/no-unique-address-union-overlap-constexpr.cpp`).
Within a union, extents are
computed per member, so a path through a bit-field member of a union
resolves (WP-201). A class's members may overlap too: an empty
`[[no_unique_address]]` member shares its offset with the next one
([intro.object]/9; libc++ `layout_stride::mapping`'s empty `extents`
before its `strides` array), so `Types::extent_in` tries every
non-bit-field member containing the byte, union or not, and takes the
first that resolves (WP-282). Extents compare element types up to array-element
qualifiers (`Types::same_unqualified`): `p[1][2]` through a `const S&`
designates the member `P[2][3]` although the subscript's element type is
`const P[3]` (WP-201). A function named by a `DeclRefExpr` or a member access
(`t.f` with `f` a static member function) is an lvalue whose address is
`fn_ptr(func_index)`; references to functions evaluate to it (WP-201).

**Offset index (WP-642).** Every by-offset lookup above (`field_at`,
`member_toward`, `extent_in`, `has_subobject`, `has_base_subobject`,
`has_bit_unit`, and the path walks `union_path`, `mutable_path` and
`place_in_unions`) visits its candidates through `Types::near(t, off)`:
the fields of `t` whose extent `[offset, offset + max(size, 1))` contains
`off`, a superset of every caller's predicate, in declaration order, so
each caller's first-match and greatest-offset rules are unchanged. A
record with at least 16 fields (and each base-subobject view of it) gets,
once at layout time, its field indices sorted by offset plus a prefix
maximum of their ends; `near` binary-searches it and walks back while an
earlier field can still reach `off`. When that yields more than 8
candidates or needs more than 32 steps back (many members at one offset,
or one large member spanning many), `near` returns every field, which is
the linear scan it replaces. Smaller records always take the scan. A
member access looks its field up by declaration
(`Types::field_index`, a map for indexed records). The index is part of
`Types::footprint`, which `Oven::footprint` includes
(`bench/oven/wide-struct-store.cpp`, `wide-struct-read.cpp`).

**Path cache (WP-644).** The walks from an allocation's root to the
object an access designates (`union_path`, `mutable_path`,
`place_in_unions`) take their memory-independent steps from
`Types::hop(walk, t, off, p)`: from record `t` toward the object of type
`p` at byte `off` of it, the member each level descends into (by that
walk's rule above), up to the first array (`Hop::Array`, whose index the
walk computes), the first union (`Hop::Union`, with a mask of the members,
among the first 64, that may hold the object by the walk's rule), the
object (`Reached`), a `mutable` member (`mutable_path` only) or the end of
the path (`Stop`). The walk still reads the union's active member and
possibly-active members from memory at every access and picks among the
mask's members exactly as before, so the path cache holds no
active-member answer (the union-path memo below remembers whole walks,
keyed by the state of the active members); the diagnostics of a member
that no possibly-active one holds are composed uncached. `Types::extent`
and `Types::has_subobject` strip leading arrays by arithmetic and cache
their answer per (record, offset, type). The cache lives in `Types`;
each of its three maps holds at most `PATH_CACHE_MAX`
(`crates/kiln_oven/src/ty/path.rs`, 16,384) entries and is cleared by
the insert past that, and all three are cleared when a provisional class
slot is rebuilt in place (`Types::refresh`, which also counts the
rebuilds in `Types::rebuilt`). It is part of `Types::footprint`, which
`Oven::footprint` includes.

The *extent front table* (WP-760, `ExtentFront` in `ty/path.rs`) holds
whole-array answers of `Types::extent`: 256 entries, two-way per (root
type, element type), direct-mapped, so a collision overwrites and the
table never grows (part of the footprint, cleared with the maps). An
entry `(root, p, lo, hi, n)` says every byte in `[lo, hi)` of a `root`
object lies in the array of `n` elements of `p` (up to qualifiers)
starting at `lo`. It is filled when `Types::extent` answers an array
whose bytes no other member overlaps on the way down from the root
(`Types::extent_unique`: a single member covers them at every record
level; records of more than 64 fields are not examined), so the walk
would take the same steps for every byte in it. `Types::extent_hit`
reads it without walking; `ptr_add_front` steps with it, and
`Types::has_subobject` answers true for an offset at an element boundary
of such an array of `p` (an `inplace_vector`'s `construct_at(&data[n])`)
before its per-offset cache.

**Union-path memo (WP-760).** `union_path` (an access) and
`place_in_unions` (a construction) remember their `Ok` answers in the
Oven's `UnionMemo` (`crates/kiln_oven/src/vm/umemo.rs`): 512 entries,
direct-mapped and fixed-size (a collision overwrites; its memory does
not grow with the evaluation), keyed by (walk, allocation key, allocation
type, accessed type, offset). An entry is stamped with
`Memory::union_epoch`, which every change to `Memory::unions` or
`Memory::union_alts` increments (a set, a removal by destruction,
release or reuse, a trial rollback, a thaw), and with `Types::rebuilt`;
it answers only while both are what they were. The walk's `Ok` answer
is a pure function of the allocation's type, the offset, the accessed
type, the type table and those maps; only a fault depends on the access
kind, so faults, and a walk that let the access through on a fault path
(`union_fault` returning `Ok`), are never remembered, and every
diagnostic is the walk's own. An entry covers one offset, or a *region*:
when the walk resolved every union by its active member and reached an
array without unions holding the accessed object, every element at the
same position (`lo + i * stride + phase` below `lo + len`) is answered
by the same walk ([intro.object]/9: no two subobjects of one type share
an address, so each record on the way has one member toward any of
them), so a `union { T data[N]; }` costs one walk, not one per element.
`place_in_unions` remembers only a walk that succeeded and changed no
active member (a hit is a construction that would do nothing); its
region is the elements of the active array member of the only union on
the way.

Before the memo, `union_path_in` tests `union_overlaps`: the accessed
object's bytes against `TyInfo::uhull`, the bytes `[lo, hi)` spanning
every union subobject of the allocation's type (`(0, 0)` without one;
`Types::union_hull`, computed at layout), and the access to the whole
object (offset 0, the allocation's own type), the walk's first step;
an access outside the hull crosses no union and checks nothing.
`Memory::may_be_active` (a member access's `bi::UNION_LIVE` and the
other active-member questions) remembers its last `true` answer with the
epoch it holds at, in a `Cell` on `Memory`. `Types::holds_no_data`
answers false without walking the members of a class with a vptr,
pointer or union subobject.

**Static members and function lvalues (WP-207).** `E.m`/`E->m` naming a
static data member evaluates `E` for its side effects only
([expr.ref]/7.1, clang's `VisitIgnoredBaseExpression`; a null `E->m` is
not an access) and designates the variable (`GlobalAddr`; a reference
member designates its referent). `E.e`/`E->e` naming an enumerator is that
enumerator's value, after evaluating `E` as a glvalue for `.` (nothing is
read, so a non-constexpr local or by-value parameter is fine) or as a
pointer for `->` ([expr.ref]/7.5; WP-811, gRPC's `case response.INITIAL:`;
`expr.ref/enumerator-member-access-constant-run.cpp`,
`expr.ref/enumerator-member-access-unknown-base.cpp`). An lvalue of function type designates
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
members registers nothing. An anonymous union member is never destroyed
(`dtor_of` is `None` for it; WP-287): a class that would destroy one
with a non-trivial variant member has a user-provided destructor that
destroys the active member itself, and the union's own implicit
destructor is deleted ([class.dtor]/7, [class.union.anon]; clang
`HandleDestructionImpl`; libc++ `__optional_destruct_base`;
`tests/conformance/class.union.anon/anonymous-union-destruction-constexpr.cpp`).

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
`memptr.rs`), so freeze/thaw maps 1:1 to `Scalar::MemberPtr`. The
member of `&C::m` for a member of an anonymous union or struct is its
`IndirectField` ([class.union.anon]/1): the entry's offset sums the
access path, and the union checked by `MpActive` is the anonymous union
that directly holds the member (WP-716). `&C::m` while `C` (or a class
of the path) is still being defined, as in a static data member
initializer of `C` or of a class nested in it, interns a *pending* entry
(member and path only; it compares equal as usual) that
`Oven::mp_refresh` lays out after `Types::refresh` once the classes are
complete; a pending entry designates no subobject (`MpAddr` faults,
WP-792). Ops:
`MpCast [mp, class]` (castToDerived aux 0, castToBase aux 1, in place),
`MpAddr [dst, obj, mp]` (`obj.*mp`, checking derived-member containment;
`Fault::NullBase` for a base step through a null object), `MpActive
[p, mp]` (a union member reached through `.*` must be active), `MpFn
[dst, mp, this, adj]` (the callee of a pointer to member function; a
virtual one dispatches as `VCall` does, WP-077, and writes its covariant
return adjustment into `adj`). `Downcast [dst, p, off, ty]` is
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
`bench/oven/soa` (a 4096-row struct-of-arrays table, the shape of real
binding tables); gated copies `tests/conformance/class.dtor/
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
  WP-077 (below). Virtual bases: WP-388 ("Virtual bases").
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
  Oven; offsets suffice for non-virtual paths, and a path through a
  virtual base is an index into `Virt`'s subobject list (`Op::VBase`,
  "Virtual bases").

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
member function dispatches the same way, and a call through a pointer to
member function whose type returns a class pointer or reference gets
the same `adj` register and `CovAdj` after the call (0 for a non-virtual
member): the result has the return type of the function the pointer
names, as for a direct call ([class.virtual]/8, [expr.mptr.oper]/6;
`class.virtual/covariant-return-member-pointer.cpp`, WP-414).

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
- Virtual bases: landed in WP-388 ("Virtual bases"). Before C++26 a
  class with a virtual base is not a literal type
  ([basic.types.general]/10), so constructing one is never a constant
  expression: `construct` traps first with clang's `CheckLiteralType` note
  `non-literal type 'X' cannot be used in a constant expression`, also
  for an inheriting constructor through a virtual base (WP-146; the
  evaluated variable itself is rejected by sema, sema-class.md §4.10).
  `call_frame` makes the same check for every by-value argument of
  class type, to a declared or a variadic parameter (clang
  `EvaluateCallArg` → `EvaluateInPlace`): a non-literal one traps at the
  argument before it is initialized (WP-320, sema.md §5.20).
  At `-fclang-compat=22` from C++23 none of these `CheckLiteralType`
  traps is made (clang 22 dropped it with P2448): a non-literal object
  fails only at the non-constexpr constructor its creation calls
  (`literal_check_relaxed`, WP-400). Constructing a class with a virtual
  base through a constexpr constructor (before C++26) instead traps
  "cannot construct object of type 'X' with virtual base class in a
  constant expression" at the construct expression, with no frame for
  the constructor (clang `HandleConstructorCall`; test
  `expr.const/constexpr-vbase-construct-cxx23.cpp`).
  A by-value class argument is the callee's parameter object: allocated
  with `Origin::Var` of the parameter in the callee's first declaration
  (clang's canonicalized `APValue` base), so its destructor's "in call
  to 'p.~X()'" note names that parameter (`'.~X()'` when unnamed) and
  sits at its location, at both levels. A class argument passed through
  the ellipsis with a non-trivial copy or move constructor or destructor
  (clang `isValidVarArgType` = `VAK_Undefined`, which sema rewrites to
  `(__builtin_trap(), arg)`) traps "subexpression not valid" at the
  argument's begin, after the literal-type check (WP-413; tests
  `expr.call/param-object-destruction-const.cpp`,
  `expr.const/non-literal-argument.cpp` and its `-clang19` split).
- A local variable that sema marked invalid (its initialization was
  already diagnosed) traps silently when its declaration is evaluated,
  so the enclosing constant expression reports only its own error
  (clang's invalid-declaration evaluation failure; WP-400).
- Freezing a pointer to a `typeid` object (`constexpr auto *p =
  &typeid(T);`) and an explicitly qualified call on a member access
  (`r.A::f()`, `p->A::f()`, non-virtual per [class.virtual]/16) landed
  (WP-176 and later; tests `class.virtual/qualified-call-constexpr.cpp`,
  `codegen/typeid_constant.cpp`).
- A virtual call on a member subobject whose own base is still being
  constructed reports the lifetime note where clang reports CWG1517
  (`no_dynamic` keys on the running constructor's object and class).

### Virtual bases — Landed (WP-388)

*Status:* P3533R2 (C++26): objects with virtual base classes are
constructed, converted, dispatched on and destroyed in constant
evaluation. Implemented in `crates/kiln_oven/src/ty.rs` (layout),
`compile/class.rs` (construction variants, `Op::VBase`),
`compile.rs`/`lib.rs` (base-object entry points) and `vm/virt.rs`
(addresses, dynamic-type words, dispatch, `dynamic_cast`, `base_owner`).
Sema: before C++26 a class with a virtual base is not literal and its
constexpr constructors/destructors and constexpr defaulted special
members are rejected (`vbases_forbid_constexpr`, sema-class.md §4.10);
from C++26 they are constexpr as any other.

**Object model.** A class `C` with virtual bases has two record types:
the complete object (`C` itself) and its base-subobject view
(`Types::nv_view(C)`, `nv_size`/`nv_align`, mapped back by
`Types::class_of`/`same_class`). The view's fields are the non-virtual
bases then the members; the complete type has the same fields followed by
one `base: true` field per non-empty virtual base, in `layout.vbases`
order at kiln_abi's offsets, each typed as that virtual base's own view
(`Types::vbase_start(C)` is the index of the first). A base class
subobject is laid out with the view of its class, so it holds no virtual
base: every virtual base of the most derived object is stored once,
in the most derived object, and shared by every path to it
([class.mi]/4).

**Construction and destruction.** A constructor or destructor of a class
with virtual bases compiles to two functions, as the Itanium C1/C2 and
D1/D2: the complete-object one (`Oven::func_index`) and the base-object
one (`Oven::func_index_base`, `Func::base_obj`, built from
`Oven::base_funcs`; a base-class initializer or destructor call is
`Callee::Base`). The complete-object constructor initializes the virtual
bases first, in `vbases()` order (sema lists their `VirtualBase`
initializers first), and registers each for destruction on an exception
([except.ctor]/3); the base-object constructor skips them
([class.base.init]/7). The complete-object destructor alone destroys the
virtual bases, after everything else, in the reverse order
([class.dtor]/13). A class without virtual bases has one function, used
for both.

**Dynamic-type words.** `SetVptr` with aux 1 (a base-object constructor
or destructor) writes, into a slot inside one of the class's virtual
bases, a *relative* word (`REL` bit set): the high word is the signed
distance from the class's own start to the slot, as the virtual base is
the most derived object's and at no fixed offset in the class. `locate`
resolves a relative word by trying each virtual subobject of that class
against the most derived object found once from the object's start
(`locate_rel`). Plain words are as in
"Dynamic types".

**Addresses.** `Virt::subs` lists every subobject of a polymorphic or
vbase-bearing class (offset, class, `virt` when in or below a virtual
base, `top` = the virtual base above it). `sub_addr(raw, cls, k)` adds
the offset for a non-virtual subobject; for a virtual one it finds the
most derived object (`owner`: `base_owner` from the allocation's root,
cached per object and per virtual-base top) and adds that object's
offset of the virtual base (`PolyInfo::vbase_at`). A derived-to-base
conversion whose path crosses a virtual base compiles to `Op::VBase
[dst, p, class, sub]` (`path_sub`; aux 1 = null stays null), which is
`sub_addr`; `Oven::vbase_ptr` is the same for a known base class.
`static_cast` (and the implicit conversions) from a virtual base to a
derived class is ill-formed and rejected by sema ("cannot cast ... via
virtual base"); `dynamic_cast` reaches it through the complete object.

**Dispatch and `dynamic_cast`.** A virtual call through a virtual base
subobject reads its word and dispatches on the complete object's dynamic
type (`locate`, then the final overrider's subobject through
`sub_addr`); `dynamic_cast` and `typeid` find the most derived object and
the target the same way (`cast_target`, `PolyInfo::vvptrs`/`at`).
A covariant return whose method return class is (in) a virtual base of
the overrider's return class (WP-400) is `CovAdj::VBase(class, k)` in the
`adj` register (`cov_word`: high half 1; a plain offset is sign-extended),
and `CovAdj` converts the returned pointer as `Op::VBase` does, through
the returned object's most derived object (`sub_addr`), so a more derived
dynamic type of the returned object is honoured
(`expr.const/constexpr-covariant-virtual-base-cxx26.cpp`).

**`base_owner`.** The search for the most derived object (used by
`sub_addr` and by the exception handler match of a thrown pointer to a
subobject, "Exceptions") descends through array elements, members and
base fields, the virtual-base fields included, so a subobject inside an
enclosing object's virtual base is found. In a union it searches only the
active member (`Memory::unions` at the union's key; every member while no
single active one is recorded), so a member whose class has the searched
class at the same offset is not mistaken for the most derived object
(WP-400, `class.union/constexpr-virtual-base-in-union-cxx26.cpp`).

**Tests.** `tests/conformance/class.mi/constexpr-virtual-*`
(C++23 rejection with clang's notes, C++26 acceptance, diamond and
nested `_run` tests with a construction/destruction log),
`class.derived/virtual-base-downcast.cpp`,
`class.derived/constexpr-virtual-base-conversions*.cpp`,
`class.inhctor.init/constexpr-vbase-run.cpp`,
`except.handle/constexpr-catch-in-virtual-base-storage.cpp`; stress
`tests/stress/constexpr-virtual-deep-diamond.cpp` (a diamond over a
64-deep chain of virtual bases, 66 shared virtual base subobjects,
allocated, converted, dispatched on and deleted 256 times: 1072 bytes
peak, 1536 arena, no growth per round).

**Interface changes.** `Op::VBase`; `SetVptr` aux 1;
`Func::base_obj`, `Oven::func_index_base`, `Oven::base_funcs`,
`Callee::Base`; `Virt::subs`; `Types::{nv_view, class_of, same_class,
vbase_start}`; `covariance` returns `Option<u128>` (a `cov_word`,
WP-400). Sema: a rejected
constexpr defaulted special member (C++23) is marked invalid, as clang's
`setInvalidDecl`.

**Call caches (WP-414).** A plain word's call is cached in
`Virt::dispatch` by (dynamic type, slot offset, method). A word written
by a complete-object constructor or destructor, or by thawing, carries
the `CMP` flag (bit 30 of the class half; Oven types stay below 2^30): its
object is the most derived object, so an overrider in a virtual base is at
its complete-object offset and the cached `delta` gives `this` without a
walk of the object model. (A plain word without `CMP`, written on a
non-virtual slot by a base-object constructor, only reaches overriders in
non-virtual subobjects, since a final overrider's subobject contains the
called one; it keeps the `sub_addr` walk as a guard.) A relative word's
call (`vcall_rel`) walks the object model once (`owner` of the running
class's object) and is cached in `Virt::rel_dispatch` by (running class,
relative slot offset, method, called class, most derived class, the
running class's offset in it), which fix the called subobject; `this` is
then `sub_in` of the cached overrider subobject. `locate` likewise walks
once for all candidates of a relative word (`locate_rel`). Probe (release,
1M calls through virtual bases on complete objects, 2M through relative
words in base-object constructors): 170 → 143 ms and 465 → 288 ms, same
steps, peak RSS 9 MB either way; `bench/oven/virtual-dispatch.cpp` (no
virtual bases) is unchanged. Test:
`tests/stress/constexpr-vcall-virtual-base-cache.cpp`.

### Vectors — Landed (WP-173)
GNU `vector_size` and clang `ext_vector_type` values are stored as an
array of their lanes (`TyKind::Array`, `Types::is_vector`). The array is
padded to the vector's own size and alignment, so an `ext_vector_type(3)`
occupies 4 lanes and the 4th is indeterminate. A vector of `bool` is the
exception (see "Bool vectors" below). The operations are in
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
- **Element-wise builtins** (WP-416): at compatibility level 22
  `__builtin_elementwise_abs` folds (integer: the most negative value
  wraps; floating: `fabs`) as do the integer `_max` / `_min` and
  `_add_sat` / `_sub_sat` (`INT_OP` `SADD_SAT` / `UADD_SAT` / `SSUB_SAT` /
  `USUB_SAT`, saturating in the element type; WP-432), scalar and
  lane-wise (`vec_elementwise`); the floating `_max` / `_min` fail without
  a note, as clang 22's. At 19 none folds: "subexpression not valid in a
  constant expression" at the call's begin (shown inside a function
  body, not at top level, as clang 19).
- **Ext-vector element access** (`ext_elt_regs`, WP-312): on a prvalue
  vector (a compound literal, an operator result, a call) the selected
  lanes are taken from the operand's registers, for one lane (a scalar)
  and for a swizzle (a vector), as clang 19 folds them.
- **Rejected without a note** (clang 19 fails silently): a `?:` with a
  vector condition, and ext-vector element access on a vector object
  (`v.x` for a variable `v`, the error at the expression's begin).
- At `-fclang-compat=22` subscripts (read and written) and
  `__builtin_reduce_*` fold, as clang 20+ does (WP-394;
  `ext.vector/constexpr-diagnostics-clang22.cpp`). A `?:` with a vector
  condition fails silently in clang 22 as well, so it stays rejected.
  Writing a lane (`v[i] = x`, `v.y = x`, any vector element type) of a
  union member does not make that member active: only member access and
  array subscript do ([class.union.general]/6), so the vector operand is
  accessed as a read-modify-write (`compile.rs` `lane_base_access`) and
  an inactive member is "assignment to member 'v' of union with active
  member 'c'", as clang (WP-438;
  `ext.vector/bool-constexpr-union-diagnostics-clang22.cpp`).
  The compat-19 boundary above holds until the oracle switch (WP-184,
  M3 to M4).
- Codegen emits folded vector values as `<N x T>` constants
  (`kiln_codegen/src/consts.rs`).

*Bool vectors* (WP-450). An `ext_vector_type(N)` of `bool` is
`TyKind::BoolVec { elem, n }` (`ty.rs`): packed bits in `ceil(N/8)`
bytes, lane `i` at bit `i % 8` of byte `i / 8`, padded to the vector's
size and alignment (`b3` is 1 byte, `b24` is 4), as clang lays out
`<N x i1>` in memory. A value is never one allocation per lane: list
initialization, the operators and `__builtin_convertvector` move whole
registers lane by lane (`load_lanes` / `store_lanes` use `LoadBits` /
`StoreBits` of width 1 at the lane's byte and bit), and a lane place is
`Place::Lane`, read and written by the KBC ops `LoadLane` / `StoreLane`
`[.., p, idx, vector type]` with a dynamic index. A lane index above `N`
faults with "cannot refer to element I of array of N elements", index
`N` with "read of / assignment to dereferenced one-past-the-end
pointer", as clang 22's. The packed bytes are what `value.rs` freezes
(the last byte's padding bits masked) and thaws.
- *Folded* at either level: list and empty initialization, a scalar
  splat, `!`, `~`, unary `+`/`-`, `& | ^` (vector or scalar operand),
  `==`/`!=` and the other comparisons, `__builtin_convertvector` both
  ways (to `bool`: nonzero is true; a float lane converts toward zero
  through a 1-bit unsigned conversion and out of range gives "value V is
  outside the range of representable values of type 'bool'"), and
  `__builtin_bit_cast` both ways. At compat 22 also subscripts (through
  pointers, members, array elements, compound assignment) and
  `__builtin_reduce_*` (`add`/`mul` wrap in one bit, `max`/`min` are
  `or`/`and`).
- *Bit casts* use the packed bytes (`vm/bitcast.rs`); a bool vector
  whose `N` is not a multiple of 8 is rejected in both directions with
  clang's "element size 1 * element count N is not a multiple of the
  byte size 8" note (`compile/bitcast.rs`). A source with an
  indeterminate lane byte is "indeterminate value".
- *Shuffles* (WP-438): `__builtin_shufflevector` folds lane by lane at
  either level. At compat 19 a shuffle whose lane count differs from its
  operands' has clang 19's GNU-vector-of-`bool` type, `CK_BitCast` to
  the ext vector (sema); kiln stores that GNU vector packed like a
  `BoolVec`, but clang 19's `BitCastBuffer` writes it a byte per lane and
  reads the smaller packed destination from its first bytes, so
  `vm/bitcast.rs` `gnu_bool_vec_cast` gives lane `k` = source lane `k/8`
  when `k % 8 == 0`, else false (`b8 w = shuffle(b4, b4, 0..7)` is
  `0x01`), the value clang 19 folds and emits. At compat 22 the shuffle
  has the ext vector type and no cast.
- *Union members* (WP-438): a lane is one bit, so a lane store into a
  `bool` vector member of a union goes through the union like a
  bit-field's (`ty.rs` `has_bit_unit` treats the lanes of a `BoolVec` as
  bit units): `union U { b8 v; unsigned char c; }; constexpr U u =
  {(b8)true};` constructs `v`, at either level.
- *Notes* print a bool vector argument lane by lane (`{true, false,
  ...}`, every lane, as clang).
- *Codegen*: a constexpr or constant-initialized bool-vector variable
  is a `<N x i1>` constant global, also as a struct member and an array
  element (spec/codegen.md), with no `__cxx_global_var_init`
  (`tests/codegen/bool_vector_const.cpp`). A runtime
  `__builtin_convertvector` of a `const` vector variable is folded by
  codegen instead (WP-490, spec/codegen.md §7w), with clang's codegen
  rule for a float lane (`x != 0`, so `0.5f` is `true`) where the Oven
  keeps clang 22's evaluator rule (toward zero, `0.5f` is `false`, a
  lane outside `(-1, 2)` is not a constant); kiln follows clang on both
  sides (`ext.vector/convertvector-const-fold-run.cpp`).
- *Not folded*: clang 22's implicit integer → `bool` ext vector
  conversion (spec/sema.md §5.28), a `CK_IntegralToBoolean` cast of
  vector type, which clang 22's `VectorExprEvaluator` does not evaluate:
  `compile.rs` `unsupported` throws the generic "subexpression not valid
  in a constant expression" for it (WP-453; `__builtin_convertvector` to
  a bool vector is the constant form). Every form is rejected, as by
  clang 22 (WP-466 checked them; no extension): a converted comparison
  result (`b4 L = I < 2`), a C-style or `static_cast`, an assignment or a
  `return` in a constexpr function. A `vector_size` or floating source
  has no conversion at all (sema).
Tests: `tests/conformance/ext.vector/bool-constexpr-*.cpp` (shuffles:
`bool-constexpr-shuffle-run.cpp`, `-19.cpp`;
`tests/codegen/bool_vector_shuffle_const.cpp`; unions:
`bool-constexpr-union-run.cpp`, `tests/codegen/bool_vector_union_const.cpp`),
`ext.vector/bool-from-int-vector-diagnostics-clang22.cpp`,
`ext.vector/bool-from-int-vector-constexpr-clang22.cpp`;
budget (packed bits, no per-lane allocation): `tests/stress/bool-vector-sieve.cpp`.

### `_BitInt` — Landed (WP-404, wide WP-445)
Type rules: spec/types.md §21.
- *Representation.* `_BitInt(N)` with `N <= 128` is `TyKind::Int { width:
  N, signed }` in its ABI-sized slot (`ty.rs`): a register value is the
  u128 register at width N, so every KBC integer opcode (arithmetic,
  comparisons, shifts, bitwise, conversions, overflow and shift-count
  checks) runs on it unchanged, with no allocation. Unsigned arithmetic
  wraps to N bits; signed overflow, `MIN / -1` and a shift count `>= N`
  fault with clang's notes ("value 64 is outside the range of representable
  values of type '_BitInt(7)'", "shift count 7 >= width of type
  '_BitInt(7)' (7 bits)"). Memory stores the value in the slot's bytes
  sign- or zero-extended, as codegen does.
- *`__builtin_bit_cast`* with a `_BitInt` source, destination or
  subobject is rejected at any width, as clang ("constexpr bit cast
  involving type 'unsigned _BitInt(16)' is not yet supported";
  `compile/bitcast.rs`).
- *Wider than 128 bits* (WP-445; x86-64 only, AArch64's `BITINT_MAXWIDTH`
  is 128) is `TyKind::WideInt { width, signed }` (`ty.rs`), memory-class
  like an aggregate: a register holds the object's address, and every
  operation is one `Op::Wide` instruction (`kbc.rs` `wide`; operands
  `[d, a, b, type, x]`, aux = sub-op | `INIT` | `RMW` | access kind) that
  reads its operand objects and writes the destination object
  (`vm/wide.rs`). The memory form is canonical: `ceil(size/8)` little-endian
  `u64` limbs, sign- or zero-extended past bit N, as codegen stores it.
  Limb arithmetic is `kiln_oven/src/bigint.rs` (safe Rust, no crates):
  each operation computes exactly in one more limb (`2n+1` for `*`) and
  then checks the result fits N bits, so signed overflow, `MIN / -1`,
  `MIN % -1` and negation of MIN fault with clang's note carrying the
  exact decimal value ("value 2^199 is outside the range of representable
  values of type 's200' (aka '_BitInt(200)')", printed in decimal);
  unsigned arithmetic wraps. Division or remainder by zero, a negative
  shift count and a count `>= N` fault as for narrow integers; C++20
  `<<` of a negative or overflowing signed value is defined. Reads check
  initialization and constness exactly as scalar loads do.
  `compile/wide.rs` lowers literals, value-init, list-init, `+ - * / % &
  | ^ << >> ~ !`, unary `-`/`+`, `++`/`--`, (compound) assignment,
  comparisons, wide/narrow/bool conversions (`FROM`/`TO`/`TO_BOOL`/`CAST`;
  a wide shift count saturates through `TO`), and an operand lvalue read
  is copied at its own source location so a note points where clang's
  does. Freeze writes `Scalar::Int` with exactly `IntBits::words_for(N)`
  words; thaw re-extends them over the slot.
- *Floating conversions, bit-fields, `switch`, `<=>`, literals* (WP-465).
  `TO_FLOAT` converts the exact value to a float kind
  (`softfloat::from_limbs`: the top 128 significant bits plus a sticky
  bit, rounded to nearest even; past the format's range it is infinity,
  as clang folds it); `FROM_FLOAT` truncates toward zero
  (`softfloat::trunc_limbs`) and faults `FloatRange` when the result does
  not fit N bits (a negative value for an unsigned type, NaN, infinity),
  noted at the operand's begin with the destination type as written and its
  `aka` (the note names every typedef so, narrow ones too). A compound
  assignment with a floating computation type converts the wide left
  operand to it and the result back. A wide bit-field is a
  `Place::Bits` read by `LOAD_BITS` into a temporary (sign- or
  zero-extended from its width) and written by `STORE_BITS` (truncated to
  the width), for initialization, assignment, compound assignment and
  `++`/`--`; freezing copies its bytes one at a time under a per-byte mask.
  `switch` on a wide condition compares it with each case value converted
  to the condition's type (`wide_case_cmp`: `CMP` EQ, or GE/LE for a GNU
  range); sema keeps a case value outside `i128` as its exact constant
  (spec/sema.md §7). `<=>` of two wide operands of the same type is two
  `CMP`s giving the ordering. An `IntegerLiteral` wider than 128 bits
  (only a substituted non-type template argument: C++ has no `wb` suffix)
  is built from its 128-bit chunks, most significant first (`FROM`, `SHL`
  by 128, `OR`). The generic wide-`_BitInt` trap in `compile.rs`
  `rv_inner` remains as a safety net for any other expression form.

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
- **Compound assignment** (`+= -= *= /=`, WP-380) with a complex left
  or right operand folds as in GCC; clang 19 and 22 fail with the generic
  note (`CompoundAssignSubobjectHandler`), and kiln deliberately accepts
  more (its constexpr reference is GCC; nothing valid is rejected). The
  right operand is evaluated first ([expr.assign]/1), then the left
  object's parts are read, `E1 op E2` is computed in `ComputeResultTy`
  exactly as the binary operator (Annex G for `*`/`/`), and the parts are
  converted back to the left operand's element type and stored. Other
  compound operators on complex operands are rejected by Sema.
- **`_Complex __bf16`** (WP-380): each operation is computed in
  `_Complex float` (the parts converted up, the float rules above) and
  each part is rounded to bf16 once at its end, as GCC's run-time
  `__mulsc3`/`__divsc3` and kiln's codegen compute it. This matches
  GCC 14's folding of a single operation (it folds `*`/`/` correctly
  rounded with MPC, the binary32 result for bf16 operands but in rare
  cancellation cases). Two documented differences: GCC keeps bf16
  intermediates of one expression in float (excess precision, for scalar
  `__bf16` too) where kiln rounds every operation, as clang does for
  scalars; and clang's evaluator, which reaches bf16 complex values only
  through `__builtin_complex`, rounds each partial product. For `float`
  and wider elements kiln keeps clang's Annex G formula, where GCC
  rounds correctly with MPC (an open question for M6).
- Tests: `tests/conformance/ext.complex/constexpr.cpp`,
  `imaginary.cpp`, `compound-assign.cpp`, `complex-bf16-rounding.cpp`.

### Exceptions — Landed (WP-357, WP-368)
P3068 (C++26, `__cpp_constexpr_exceptions` 202411L): a throw-expression,
try-blocks and handlers are evaluated in a constant evaluation. Before
C++26 (`SemaBridge::cxx26()` false) the lowering is the C++23 rule:
an evaluated throw-expression is not constant ("subexpression not valid
in a constant expression") and a try-block is its compound statement
(the handlers are never reached). No oracle
implements P3068 (clang 22 has no macro and rejects every evaluated
`throw`): the tests are `skip(clang)` with the reason, and the notes
follow clang trunk's P3068 branch.

*Model* (`compile/except.rs`, `vm/exc.rs`):
- `throw e` allocates the exception object in the heap as `new T(e)`
  does (`Op::New`, origin the throw-expression; `T` is the operand's
  type with references and top-level cv dropped, arrays and functions
  decayed), initializes it, and executes `Throw [p, T, dtor]`. Between
  the allocation and the `Throw`, an unwind-only `EXC_FREE` cleanup
  (`PushCleanup` aux 4: run by unwinding, dropped by a normal exit) frees
  the object if its initialization throws: the object never existed
  ([expr.throw]/5), and that exception propagates instead
  (`expr.throw/constexpr-throw-ctor-throws.cpp`, a 10000-round loop
  under `oven:` budgets).
  `throw;` is `Throw` aux 1 (the innermost handled exception; none:
  "there is no active exception to rethrow").
- A try-block executes `TryEnter [handler]`, which pushes a handler
  marker onto the cleanup stack (`Cleanup::func == TRY_MARK`, `this` =
  the handler pc), and pops it with the destructions registered inside
  it on every exit, so `break`/`return`/`goto` out of it need nothing
  special.
- A throw finds the innermost marker of the running evaluation (markers
  below `RunState::entry` belong to an enclosing evaluation and are
  never used: a nested evaluation cannot catch its caller's exception
  nor be caught by it). Each evaluation also has its own exceptions:
  `exec` records the in-flight and handled stack lengths as
  `Excs::base` for its duration, so a nested evaluation (a
  `static_assert` in a member instantiated by `ensure_body` while the
  outer evaluation unwinds or is in a handler) has nothing to rethrow
  ("there is no active exception to rethrow"), counts no uncaught
  exception, and may throw without "exception thrown out of an object
  destructor while unrolling another exception"; an exception escaping it is uncaught in it
  (`except.handle/constexpr-nested-evaluation.cpp`). Every frame it leaves is checked first: a
  non-throwing function ([except.spec], `SemaBridge::is_nothrow`,
  resolving a deferred specification) ends the evaluation with
  "uncaught exception in noexcept function" at the call leaving it
  ([except.terminate]/1.4, std::terminate). It then unwinds: the
  destructions registered above the marker run innermost first
  ([except.ctor]/2), each returning to the Oven's single `Op::Unwind`
  (emitted on first use, never truncated), and the frames above the
  marker's are popped as by a return (their registered cleanups run).
  Popping a frame checks its scope-lifetime as a return does: an object
  whose lifetime ended before the unwind (`x.~T()` of a trivially
  destructible `x`) is "destroying object 'x' whose lifetime has already
  ended", and the frame's slots are released, so a pointer into an
  unwound frame or block dangles in the handler
  (`except.handle/constexpr-unwound-lifetime.cpp`).
- The handler code matches the exception against each handler in order
  with `Catch [dst, T, next]` ([except.handle]/3): aux 0 is `...`; aux 1
  an object or reference handler of the same class or of an unambiguous
  public base (`dst` = the (sub)object's address; a by-value handler
  copies it); aux 2 a pointer handler: `nullptr_t` to any pointer, the
  same type, a qualification conversion, a conversion to `void *`
  (object pointers only), or to an unambiguous public base. A base
  that is (in) a virtual base is reached through the most derived object
  of the thrown pointer's (sub)object ([class.mi]/4, `Oven::vbase_ptr`):
  the object model locates it (`base_owner`: the complete object, member
  or array element one of whose subobjects of the pointer's class the
  pointer designates), and the base's offset is that object's virtual
  base's plus the base's non-virtual offset in it
  (`except.handle/constexpr-catch-virtual-base-ptr.cpp`,
  `constexpr-catch-virtual-base-ptr-subobject.cpp`). `base_owner` also
  searches the virtual-base storage of each candidate object, so a
  subobject inside an enclosing object's virtual base (P3533R2) is placed
  too (`constexpr-catch-in-virtual-base-storage.cpp`, WP-388); a pointer
  the model still cannot place is "subexpression not valid in a constant
  expression".
  A `const T&` handler
  with `T` a pointer type matches as aux 2 and binds to a temporary
  holding the converted pointer; any other reference handler of pointer
  type matches only the same type. The matching handler executes
  `CatchBegin`: the exception moves from the in-flight to the handled
  stack and an `EXC_END` sentinel registered in the handler's scope ends
  that when the handler is left by any path. When nothing matches,
  `Throw` aux 2 resumes unwinding.
- The exception object is destroyed (its destructor runs as a cleanup
  above an `EXC_FREE` sentinel) and freed when the last handler using it
  exits and it was not rethrown ([except.throw]/4), so it is never a
  leak and a loop that throws and catches keeps a flat heap (the WP-060
  reclaim rule; `expr.throw/constexpr-throw-churn.cpp` runs a throwing
  loop under the harness `mem<=512` limit).
- An exception escaping a destructor run by unwinding is "exception
  thrown out of an object destructor while unrolling another exception"
  (the P3068 branch's wording), with "currently active exception thrown
  here" at the first throw (kiln's addition).
- An exception that leaves the evaluation (no marker of this evaluation)
  fails it, worded as clang's P3068 branch (`notes::uncaught_note`):
  "unhandled exception: W" at the throw-expression, `W` the `what()`
  string of a class derived from `std::exception` (evaluated silently
  through a virtual call), printed whole up to `WHAT_CAP` (64 KiB)
  bytes and past it as its first `WHAT_CAP` bytes, backed off to a UTF-8
  character boundary (at most 3 bytes, when the cap would split a
  multi-byte character), followed by "… [N more bytes]", `N` the bytes
  not printed, those of a split character included (GCC 16.1 prints any
  length; the cap keeps a pathological string from flooding the
  terminal; WP-1245, WP-1251). The scan reads `char`s until the null
  character and keeps no more than the cap; when a read faults after
  at least one character (no null character within the string's array,
  an uninitialized or dead character), the note prints the characters
  read, then "… [end of what() string not reached: R]" (after the cap,
  "… [N more bytes; end of what() string not reached: R]"), `R` the
  note that read carries elsewhere ("read of dereferenced one-past-the-end
  pointer is not allowed in a constant expression"), as a `static_assert`
  message prints the characters read before its failure (WP-1251;
  `expr.throw/constexpr-uncaught-what-{unterminated,utf8-cap,fails}.cpp`).
  The bytes kept are escaped as all program-provided text a diagnostic
  prints (`kiln_diag::escape_diag_text`, spec/diag.md §6.2; the cap
  counts the bytes before escaping): a newline or a tab prints as is, a
  control character as `<U+XXXX>`, a byte that is not valid UTF-8 as
  `<XX>`, as clang prints a `static_assert` message (GCC 16.1 prints
  C escapes, `\x0a`, and keeps every note on one line; WP-1259,
  `expr.throw/constexpr-uncaught-what-escape.cpp`). Else, or
  when that call is not constant or the first read faults,
  "unhandled exception of type 'T' thrown from here" at the operand when
  `T` has size 1 (an empty class, a `char`, a class of one `char`: the
  branch's `getTypeSizeInChars(T).isOne()`), and "unhandled exception of type 'T' with
  content V thrown from here" otherwise, `V` the value as every other
  note prints it (`12`, `{4}`, `1.500000e+00`). The libc++ 22 `std::exception` constructors and `what()` are not
  `constexpr`, so with the library headers only user exception classes
  are usable today.
- *Function-try-blocks* ([except.handle]/13-15): a function body that is
  a `CXXTryStmt` compiles as a try-block around the body; the handlers of
  a constructor or destructor end with an implicit `throw;`
  ([except.handle]/15). A constructor's try-block also covers its
  mem-initializers and default member initializers: every base and
  member constructed registers an unwind-only cleanup (`PushCleanup`
  aux 2|4) that destroys it, in reverse order, when a later initializer
  or the body throws ([except.ctor]/3), before the handler is entered; a
  delegating constructor registers only the whole object's destructor,
  once its target returns ([except.ctor]/4). A destructor's members and
  bases are destroyed before its handler is entered
  (`except.ctor/constexpr-ctor-try-block-subobjects.cpp`).
- *`std::uncaught_exceptions`* ([except.uncaught]/1): a call of
  `std::uncaught_exceptions()` whose declaration is `constexpr` is
  `Builtin UNCAUGHT`, the number of exceptions of the running evaluation
  in flight (thrown or rethrown and not yet caught). P3842R2 (C++26)
  withdrew the `constexpr` P3068R6 had added; libc++ 19/22 and libstdc++
  declare it without `constexpr`, so with the library headers the call
  is not constant ("non-constexpr function", as clang) and the builtin
  serves a `constexpr` declaration only (`except.uncaught/*.cpp`).
- *`__builtin_constant_p` trials*: a throw allocates its exception
  object, which a speculative trial refuses ("Speculative mode"), so a
  trial whose operand throws, caught inside it or not, answers 0 and the
  evaluation around it continues with its own in-flight and handled
  exceptions unchanged (`except.handle/constexpr-bcp-trials.cpp`).
- A failed evaluation restores the lengths of the in-flight and handled
  stacks (`Excs::lens`/`truncate`), and `Oven::reset` clears them.

*Deliberate divergences from clang trunk's P3068 branch* (each follows
the standard): `nullptr` is not caught by a `const int *&` handler (a
reference handler of pointer type requires the same type, [except.handle]/3.1);
a function pointer is not caught by `void *` ([conv.ptr]/2 converts
object pointers only); a constructor's handler rethrows at its end;
an uncaught exception's content is printed as every Oven note prints a
value (a class as `{4}`).

### Unknown references — Landed (WP-357)
P2280 (C++23 [expr.const]/10, a DR applied from C++23 on, as clang 20+;
`SemaBridge::cxx23()`): naming a reference variable that is not usable
in constant expressions (a parameter of a function outside its call, an
`extern` reference, a reference whose initializer is not constant) is
constant; the reference binds to "an unspecified object of the referenced
type". On first use in an Oven the reference's external object is
rebound to a static object of origin `Origin::Unknown(d)` (one per
variable, kept in `Oven::unknowns`, `EXTERNAL | CONST`; unbacked, so it
holds no bytes whatever the referenced type's size, "Abstract memory",
WP-375: `expr.const/p2280-unknown-reference-mem.cpp` names a reference
to a 2 GB array), so its address is a constant and passing it on, binding another reference to it
(`std::size(a)` of an array reference parameter), and non-virtual member
calls that do not read it are constant
(`expr.const/p2280-unknown-reference.cpp`). Diagnosed, with clang 22's notes:
- reading, assigning or incrementing it: "read of variable 'x' whose
  value is not known" / "assignment to ..." / "increment of ...", plus
  "declared here" at the variable;
- a virtual call or `dynamic_cast`/`typeid` needing its dynamic type:
  "virtual function called on object 'v' whose dynamic type is not
  constant";
- an equality comparison of its address with another object's or null:
  "comparison between pointers to unrelated objects '&a[4]' and
  'nullptr' has unspecified value";
- a constant result referring to it (a namespace-scope reference's
  initializer): the clang 22 "reference to ... is not a constant
  expression" form.
Before C++23 the reference stays external: naming it is "function
parameter 'x' with unknown value cannot be used in a constant expression"
as before (`expr.const/p2280-unknown-reference-cxx20.cpp`).
As in clang 22, only a *variable* of reference type binds to an unknown
object: a reference member of an unknown object is read from that object
("read of variable 'h' whose value is not known"), and `this` outside
the evaluation of a call to a constexpr member function is not constant
("[implicit] use of 'this' pointer is only allowed within the evaluation
of a call to a 'constexpr' member function"); inside a member function
called on an unknown object, `this` is its address, so comparing it with
the object's own address is constant and with null is not
(`expr.const/p2280-ok.cpp`, `p2280-errors.cpp`). *Deviation:* a
reference to an object of 4 GiB or more stays external (naming it is
diagnosed as before C++23); clang accepts it.

**Provisional globals (WP-378).** A variable with no initializer yet
(`extern const int &r;` before its definition, `extern const int n;`)
may get one later in the translation unit, so its resolution
(`GState::Resolved`) holds for one outermost evaluation only: it is
listed in `Oven::provisional` and reset to `Unresolved` by the next
`begin_outermost`. Its *object* is owned by the translation unit:
`Oven::provisional_objs` keeps, per global, the object it resolved to and
its `OTy`, and the next resolution reuses that object while the variable
still has no initializer, is `NotEvaluated` or `NotConstant`, its type
interns to the same `OTy` and the object is live. So a provisional global
costs one static slot (an unbacked one, or a reference's pointer slot
bound to its `Origin::Unknown` object) per translation unit however many
evaluations name it, and its address is the same in each
(`tests/stress/p2280-provisional-globals.cpp`: 20 000 evaluations,
`static_bytes<=16`; `kiln_oven` test `provisional_globals_do_not_grow`:
footprint flat over 10 000 evaluations). Once a definition gives it an
initializer, the next resolution is a fresh one (the old object stays, at
most one per variable, since a frozen result may still point to it)
(`expr.const/p2280-provisional-global.cpp`).

### Label addresses — Landed (WP-444)
GNU `&&l` (clang's `AddrLabelExpr` lvalue base: an object of unknown
value, distinct per expression) is a constant `void *`.
- `compile.rs` lowers an `AddrLabelExpr` to `Op::LabelAddr [dst, expr]`
  (KBC, appended after `Cce`). The VM's `label_addr` returns the pointer
  cached for that expression in `VirtState::labels` (`ExprId` → pointer,
  12 bytes per entry in the footprint), allocating on first use an
  unbacked `CONST | EXTERNAL` object of origin `Origin::Label(expr)`.
  Nothing is allocated unless an `&&l` is evaluated, and a second
  evaluation of the same expression yields an equal pointer.
- Two label addresses compare equal only when they come from the same
  expression; a label address is never null. Reading through one is
  `MemFault::Literal`, "read of object '&&l' whose value is not known"
  plus "temporary created here" at the expression. Offsetting past the
  object is "cannot refer to element N of non-array object"; a cast to
  an object pointer gets the `void *` similarity note, a cast to an
  integer the reinterpret_cast note; a relational comparison of two
  label addresses is "comparison between pointers to unrelated objects
  '&&&l' and '&&&m' has unspecified value" (the pointer is printed as
  clang's `&` + the expression). `__builtin_constant_p(&&l)` is false
  (`is_literal` is not true of label bases).
- clang 19 (`-fclang-compat=19`): any equality comparison with a label
  or string literal base is "comparison of addresses of literals has
  unspecified value" (`is_literal_or_label`), and a relational one is
  "comparison between '&&&l' and '&&&m' has unspecified value".
- Freezing gives `PtrBase::Label { label, origin }` (spec/ast.md, code
  6, printed `&&l` with `+ N` for a non-zero offset; never mangled into
  a template argument); thawing goes back through `label_addr(origin)`.
  Codegen lowers the frozen value to a `blockaddress` (spec/codegen.md
  §7t).
- *Label address differences* (WP-1304, GNU C; clang's
  `AddrLabelDiff`). In a C static initializer, `&&a - &&b` is a
  constant. `compile.rs` `addr_int_inner` recognizes the subtraction
  (`label_diff`) under the same integer operations as an address,
  plus a narrowing conversion directly above it (to 32 bits or more
  at `-fclang-compat=22`, any width at 19), and lowers it to
  `Op::AddrInt` `addr_int::LABEL_DIFF`. Under `set_fold_c_static`
  the VM records the two `AddrLabelExpr`s in `Oven::label_diffs` and
  yields the address `mem::label_diff_ptr(idx)` (a nowhere pointer of
  `LABEL_DIFF_SLOT`) tagged as an integer; elsewhere it fails as
  `PtrSub`. Stored into an integer object, it leaves the bytes
  uninitialized (no evaluation can read it) and records the object in
  `Oven::label_diff_cells` (space, byte → index, displacement; cleared
  by `set_fold_c_static`), which the freeze's `walk` relocates; as a
  whole value it freezes directly. Either way it is a
  `PtrBase::LabelDiff` of the integer's size, and thawing one leaves
  the object uninitialized. Codegen emits `trunc (sub (ptrtoint
  (blockaddress), ptrtoint (blockaddress)))` (`consts.rs`, also as an
  aggregate member through `member_ptr_reloc`). Tests:
  `c/run/static-local-label-difference{,-short-19}.c`,
  `c/decl/static-init-not-constant-22.c`.
- Evaluating `goto *p` is "subexpression not valid in a constant
  expression"; sema already rejects the statement in a constexpr body
  (spec/sema.md). Tests: `tests/conformance/stmt.goto/computed-goto-
  constexpr{,-19}.cpp`.

### Pseudo-object expressions — Landed (WP-456)
A `PseudoObjectExpr` (ast.md §5.3; today only the synthesized three-way
comparison of a defaulted `operator<=>`, spec/sema-class.md §4.12) is
compiled from its semantic expressions (`compile.rs` `bind_pseudo` /
`unbind_pseudo`): those before the result are compiled in order, an
`OpaqueValueExpr` binding its source once in the compiler's opaque map
(an lvalue for a glvalue source, else a register), any other compiled
for effect; the result is then compiled as an rvalue, lvalue,
initialization or effect, as the context asks, and the bindings are
dropped. Each operand is therefore evaluated exactly once, and the
bytecode is the same as for a hand-written `a == b ? ... : a < b ? ...`
over two locals: the evaluation fast path allocates nothing new. The
syntactic form `a <=> b` is never compiled; it is the location of notes
(`expr_loc`). Tests:
`tests/conformance/class.spaceship/synth-three-way.cpp`
(`static_assert`s over strong, weak and partial categories, a base, an
array member, and a call counter that checks the single evaluation).

### Nested immediate invocations — Landed (WP-537)
A `ConstantExpr` that is a nested immediate invocation (ast.md
`CONSTANT_ELIDED`: P2564R3 made its enclosing constexpr function
template specialization or lambda immediate-escalating) carries no
cached value; it is evaluated in place, as part of the enclosing
evaluation ([expr.const]/16-18), in every value category: `rv_inner`,
`lv`, `init_inner`, `tracked_list`, `fn_lv` and `effect` all compile its
operand as they would compile a `ParenExpr`'s. Its side effects (a void
`f(x)` that increments the caller's local `x`, a comma fold of such
calls) are therefore seen by the enclosing evaluation, and a failure in
it is diagnosed with the enclosing call stack (`in call to 'h(x)'` /
`in call to 'g2<1>()'`, as clang). For effect, a `ConstantExpr` that
does carry a value contributes nothing (its operand's side effects were
those of its own evaluation), as for a `void` one.

A top-level immediate invocation of a `void` consteval function (`h();`
in `main`, `(void)h()`, `h(), h()`) is a constant expression with no
value. `Oven::evaluate` with `EvalKind::Consteval` arms `void_next`, so
`eval_at` freezes a stand-in (`bool` 1) for a `void` result instead of
"expression of type 'void' has no value"; every other `EvalKind` keeps
that diagnostic. Nothing reads the stand-in: the AST dump prints no
value for a `void` `ConstantExpr` and codegen emits no code for it
(clang `VisitConstantExpr`). Tests:
`tests/conformance/expr.const/immediate-invocation-nested-void{-run,-neg,-reflection}.cpp`,
`tests/conformance/expr.const/immediate-invocation-void-run.cpp`.

### Valued immediate invocations — Landed (WP-1140)
An immediate invocation is evaluated exactly once. Sema stores each
outermost invocation's value (`set_constant_value`, sema.md §5.10)
as soon as it is evaluated, before any fold of the enclosing
initializer, and a value is stored only for a successful evaluation,
so a stored value is final. The compiler then treats a `ConstantExpr`
with a value as a leaf wherever it is compiled, instead of compiling
its operand again: the whole-initializer fold of a constant-initialized
variable (`try_fold`), the variable's own constant-initialization check
(`check_constexpr_init`, `check_constinit`), `fold_constant_init` for
codegen, and every enclosing evaluation that reaches the wrapper.
- `rv_inner`: a scalar with bits (`scalar_bits`) is a constant; a
  pointer is the address of its thawed object (`THAW_PTR`).
- `init_inner` (`valued_leaf`): a scalar (not a vector, complex or wide
  integer) is stored as `rv_inner` reads it; a class object is copied
  (`Op::Copy`, aux 1) from the static object its value thaws to
  (`bi::THAW_OBJ`, cached per `ConstValueId` in `Oven::thawed`, so the
  bytes and relocations are written once per compilation). The copy is
  the guaranteed-elision initialization the call itself would perform:
  no constructor runs.
- Not leaves (the call is evaluated again): a glvalue (`lv`; the value
  is the address it designates), a vector, complex or wide integer, a
  class with a `mutable` member (its thawed object is not readable,
  [expr.const]/5.9) or a virtual table pointer, a nested invocation
  (`CONSTANT_ELIDED`, never valued), and an invocation in a default
  argument or default member initializer: those are elided where they
  are written and evaluated at each use (sema.md §5.10), because the
  use site (`source_location::current()`) can change the value.

The trial folds that run before the full-expression ends (the
implicit-conversion and overflow warnings, `impcast.rs`,
`int_overflow.rs`) run after sema has evaluated the outermost
invocations (`evaluate_immediates_early`, sema.md §5.10, WP-1129), so
they read the stored values: a valued wrapper is a leaf (0 steps), and
a scalar prvalue call reached by stripping the wrapper is answered by
the bridge without an evaluation (clang's `EvaluateForOverflow`
evaluates the call a second time). A default argument's invocation is
still elided where it is written, but codegen's fold prepass and the
compiler (`Compiler::stored_value`: `rv_inner`, `valued_leaf`,
`cx_const` and the effect arm, outside a use with source-location
values) reuse the value sema's check of a use stored
(`default_immediate_value`) instead of evaluating the call again
(WP-1171). The shift-count check of `<<`/`>>` (sema
`diagnose_shift_count`) evaluates a count that is an invocation once
and hands the result to the full-expression's finish (WP-1171). Tests:
`tests/conformance/expr.const/immediate-invocation-evaluated-once-run.cpp`,
`tests/conformance/expr.const/budget/immediate-invocation-once*.cpp`,
`crates/kiln_driver/tests/time_trace.rs` (`immediate_invocation_evaluated_once`,
`immediate_invocation_trial_folds_once`),
`crates/kiln_driver/tests/constexpr_profile.rs`
(`immediate_invocations_are_not_folded_again`).

### Reflection runtime (M6; `-freflection`)
Specified in **`spec/reflection.md`** §3:
- the `info` value (`TyKind::Reflection`, bits `index - 1`) and the
  typed-copy rule (§2.1, §2.5);
- the `bi::META` instruction and the `SemaBridge::meta` entry point
  (§3.2);
- `bi::META_EXC` (31; operands `[dst, what, at, site + 1]`): run only in
  the block after a `bi::META` call whose failure is thrown (§8.2). It
  writes the reflection of the failing intrinsic to `dst` and builds the
  message (`Str` recipe) at `what` and the call's `source_location` at
  `at` from the pending failure, which the block then passes to the
  `std::meta::exception` constructor and throws as a throw-expression
  ("Exceptions" above). `SemaBridge::meta_exception` names the
  constructor when the call is compiled, `SemaBridge::meta_throw` makes
  the values when the call fails;
- the purity classes and the per-op table (§3.3);
- `MetaCtx` (§3.4) and materialized results (§3.5);
- memoization (§3.6);
- provisional class slots in the type table (§3.7);
- accounting (§3.8).

The VM half is `vm/meta.rs`; the op tables are in `meta/`.

### Undefined behaviour checks — Landed (WP-948, WP-1018)
[expr.const]/10 makes every core undefined behaviour disqualify an
expression, so each class below is one structural check, never a
per-syntax patch; each is a note under the caller's "is not a constant
expression" / "must be initialized by a constant expression", at the
location clang 22 gives it.
| class | check | note |
|---|---|---|
| member access through a past-the-end pointer (`.`, `->`, a base or member of an element past the end) | the `Field` op's access check (`MemFault::PastEnd`); a frozen subobject past-end pointer keeps its `past` bit (`Scalar::Pointer::past`, `value::ptr_of`/`thaw_ptr_raw`) | "cannot access field of pointer past the end of object" |
| member call through a past-the-end pointer | `EndLife` aux 4 (`ak::MEMBER_CALL`): a past-end, dead or stale `this` faults as a member-call access | "member call on dereferenced one-past-the-end pointer is not allowed in a constant expression" |
| member call on an object outside its lifetime (after an explicit destructor call, after its scope ended) | same check | "member call on object outside its lifetime is not allowed in a constant expression" |
| read of a volatile object through a non-volatile glvalue (`const_cast`) | `value::var_info`: a volatile variable is never usable, so its object is `EXTERNAL`; a volatile-qualified glvalue read stays a compile-time trap (`compile.rs` `LValueToRValue`) | "read of volatile object 'v' is not allowed in a constant expression" + "volatile object declared here" |
| access to a volatile local, temporary, heap object or volatile member, or a subobject of one (WP-1019) | a frame or heap allocation whose type is volatile or holds a volatile member gets `flags::VOLATILE` (the static-only `MUTABLE_OUTSIDE` bit, so loads and stores already take `slow_access`); `vm::volatile_path` walks from the root to the accessed object (a union through its active member): only the accessed object's volatility counts, the last volatile member on the path names it, and an object whose constructor or destructor is running is not volatile ([class.ctor.general]/5, [class.dtor]/16): a structor frame's `this` in the same allocation names it by offset and class (`Func::this_ty`), and the walk exempts the record at that offset of that class only, so a base subobject at offset 0 does not exempt its complete object (no two live subobjects of one class share an address, [intro.object]/9; WP-1101). An empty base is not in `Types::fields`, so the walk never enters one and its structor exempts nothing else. The complete object's own volatility is `Oven::volatile_decls`/`volatile_exprs` (interned types are cv-stripped); a trivial copy of a class with a volatile member is not an access to it (clang) | "{read of\|assignment to\|increment of\|decrement of} volatile {object 'v'\|member 'v'\|temporary} is not allowed in a constant expression" + "volatile {object\|member} declared here" / "volatile temporary created here" (none for a heap object) |
| a write, compound assignment, increment or decrement through a volatile-qualified glvalue | compile-time trap (`Compiler::volatile_glvalue`, clang `findCompleteObject`), at the operator, or at the left operand when its member path names a union member (clang `HandleUnionActiveMemberChange`; parentheses stop the walk) | "assignment to volatile-qualified type 'volatile int' is not allowed in a constant expression" |
| a parameter object used after its call returned (WP-1018) | the caller keeps each by-value class argument's address below the call window and runs `Op::EndParam` after the call: a registered destructor runs then (last constructed first), and `Memory::kill` ends the allocation's lifetime (`Undo::Kill` on rollback) while its storage stays until the full-expression's scope releases it ([expr.call]/7 is implementation-defined; clang's `CallScope`) | "read of object outside its lifetime is not allowed in a constant expression" (no declared-here note) |
| subtraction of pointers into a zero-size array | `PtrSub` aux 1 (set when the pointee array type has size 0): same allocation faults `Fault::ZeroSizeSub` | "subtraction of pointers to type 'int[0]' of zero size" |
| equality comparison involving a pointer to a virtual member function ([expr.eq]/4.3) | `Op::MpCmp` before a member-function-pointer `ICmp`; a null operand is specified ([expr.eq]/4.1-2) and passes | "comparison of pointer to virtual member function 'g' has unspecified value" (`Fault::MpVirtCmp`) |
| a reference variable or reference data member bound past the end, at `-fclang-compat=22` | `Op::RefChk` (`Compiler::ref_chk`) after a local reference's binding and after a reference member's (`init_field`: aggregate and designated initialization, a new-expression, a temporary, a default member initializer, a mem-initializer, a base's aggregate initialization; WP-1021): a pointer with `past` set or at the end of its object faults at the initializer's start (clang 22 `EvaluateVarDecl`/`EvaluateInPlace`). Not checked: clang 19, reference parameters, a lambda's init-captures | "read of dereferenced one-past-the-end pointer is not allowed in a constant expression" |
| a reference constant result (a variable's, or a reference member's of a class result) past the end, in both modes | `value::check_ref_past_end` while freezing a reference to a static object or an extended temporary (clang `CheckLValueConstantExpression`); "subobject of" when the pointee type is not the allocation's or the pointer is past a subobject's end. A pointer result may point past the end | "dereferenced pointer past the end of {subobject of }'a' is not a constant expression" + "declared here" (a temporary: "temporary" + "temporary created here") |
| an initializer that is not yet visible ([expr.const]/2) | sema `fix_const_int_init` (kiln_sema `decl.rs`): a namespace-scope `const` non-volatile integral or enumeration variable is evaluated where its initializer is, silently, and the result (a value or `NotConstant`) is cached, as clang's `CheckCompleteVariableDeclaration`; a later read of a `NotConstant` usable variable faults `External` | "initializer of 'b' is not a constant expression" + "declared here" |
| `extern constexpr` without an initializer ([dcl.constexpr]/1) | sema `default_init_var` | error "constexpr variable declaration must be a definition" |
Cost on valid code: `MpCmp` and `RefChk` are cold ops emitted only for
member-function-pointer comparisons and (at level 22) local references
and reference members; the zero-size test is a compile-time aux bit;
the `past` bit costs no store bytes when false. The volatile check reuses
the `MUTABLE_OUTSIDE` bit that every load and store fast path already
tests, so it costs nothing on an allocation without a volatile object;
`EndParam` runs once per by-value class argument.

Known gaps: a pointer to a parameter object that escapes two call levels
may be noted "outside its lifetime" where clang says "variable whose
lifetime has ended"; at `-fclang-compat=19` a
new-expression of a class with a volatile member is not rejected as
non-literal (clang 19 `CheckLiteralType`), it fails at the member access
instead. Corpus:
`tests/conformance/expr.const/{member-of-past-end-base,call-on-past-end,
read-volatile-via-cast,zero-size-pointer-subtraction,
virtual-member-pointer-compare,reference-past-end-clang22,
initializer-not-yet-seen,call-out-of-lifetime,
read-volatile-local-and-heap,volatile-object-access{,-ok},
volatile-member-access{-clang22,-ok-clang22,-19},
ref-member-past-end{-clang22,-19,-ok},ref-past-end-result}.cpp`,
`expr.call/param-object-{lifetime-ends,destroyed}-at-return.cpp`,
`class.ctor/constexpr-structor-volatile-{subobject{,-ok},empty-base}.cpp`,
their `*-siblings*.cpp`,
`ub-checks-valid-near-misses.cpp`,
`dcl.constexpr/extern-constexpr-declaration.cpp`.

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
    // reflection (spec/reflection.md §3; defaults: not an intrinsic / Err(MetaError::silent())):
    fn meta_intrinsic(&self, f: DeclId) -> Option<MetaOp>;
    fn meta(&mut self, op: MetaOp, args: &[u128], out: &mut MetaOut, ctx: &MetaCtx, oven: &mut Oven) -> Result<(), MetaError>;
}
impl Oven {
    pub fn new(budget: Budget) -> Self;
    pub fn budget(&self) -> Budget;
    pub fn evaluate(&mut self, bridge: &mut dyn SemaBridge, e: ExprId, ctx: EvalContext) -> Result<ConstValueId, EvalFailure>;
    pub fn try_fold(&mut self, bridge: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId>;
    pub fn is_constant_initialized(&mut self, bridge: &mut dyn SemaBridge, var: DeclId) -> bool;
    pub fn fold_constant_init(&mut self, bridge: &mut dyn SemaBridge, e: ExprId, var: DeclId) -> Option<ConstValueId>;
    pub fn has_constant_destruction(&mut self, bridge: &mut dyn SemaBridge, var: DeclId, value: Option<ConstValueId>, const_context: bool) -> bool;
    pub fn fold_rvalue(&mut self, bridge: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId>;
    pub fn stats(&self) -> &EvalStats;
    pub fn footprint(&self) -> usize;
    pub fn memory(&self) -> kiln_ast::mem::MemReport;             // WP-663
    pub fn disassemble(&self) -> String;                          // debugging aid
    // The evaluation paused in a bridge callback (WP-1248): a token for it,
    // and its call stack as "in call to 'f(args)'" notes, innermost first,
    // elided as a failure's backtrace (-fconstexpr-backtrace-limit). The
    // notes are those of the evaluation `mark` names, while this Oven still
    // holds it paused (the mark matches, or a nested evaluation runs above
    // it on the same stack): its own frames only. Empty for a mark taken in
    // a potential-constant evaluation (`evaluate_call`, `evaluate_potential`,
    // a frame job: sema's never-constant check), whose frames are the
    // check's own (WP-1258).
    pub fn frame_mark(&self) -> FrameMark;
    pub fn paused_call_notes(&self, mark: FrameMark, ast: &Ast, sources: Option<&SourceManager>) -> Vec<Note>;
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
| `EvalContext::consteval_block(loc)` | `consteval_block` | yes | a consteval-block-declaration (WP-473); the only constructor that sets `plain` |

  `EvalContext::plain` (WP-473, `spec/consteval-blocks.md` §3) is the
  plainly constant-evaluated bit: true only for a consteval block's
  evaluation ([expr.const.core]/2.32), the one context in which injection
  may mutate. It becomes `run.plain` for that evaluation (a nested
  evaluation starts from its own context's bit and restores the
  enclosing one); `Oven::plain()` is `run.plain && !run.fold &&
  !mem.in_trial()`, so `try_fold`, `__builtin_constant_p` operands and
  every other trial read false. `Op::IsConstEval` aux 2 reads it
  (`__kiln_plainly_constant_evaluated()`); aux 4 / aux 8 save-and-clear
  / restore it around a constexpr local's initializer, which is its own
  manifestly constant-evaluated expression. The call memo key carries
  `Oven::plain()` at bit 30 next to `const_context` at bit 31.

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

### Expansion statements — Landed (WP-447)
An expanded `CXXExpansionStmt` (spec/sema.md §7.6a) compiles to straight
line code with no loop (`compile.rs` `expansion_stmt`): the init-statement
and the prelude (`__range`/`__begin` or the structured binding) in a scope
of the statement, then each element's `CompoundStmt` in order, each with a
`Breakable` of its own. Each element block is an ordinary block scope, so
its element variable, and anything its constructor allocated, is destroyed
and released at the end of that element: the peak is one element's, not
N's. `break` jumps past the last element; `continue` jumps to the end of
the current element (the next one follows). A pattern (not expanded) is
never compiled. Test: `tests/conformance/stmt.expand/oven-memory.cpp`
(48 elements, each copy allocating 256 bytes: `peak_bytes<=512`).

### Switch dispatch — Landed (WP-1144)
A `switch` (`compile.rs` `switch_stmt`) compiles its condition, then one
compare-and-branch per case value (a GNU range: two compares) to an
out-of-line stub per label, then the default (or the end). Each stub
charges the steps clang's label hunt would charge (`EvaluateStmt` with a
`Case`: one per statement visited before the label, stepping into a
`for`/`if` init-statement) and jumps to the label's code. A label that is
a direct child of the body (or of a label chain there) is an ordinary
jump target: that path, which is every switch without nested labels,
emits exactly the code it did before WP-1144.

A label may also be anywhere in a sub-statement of the body
([stmt.switch]/2; Duff's device): inside a `do`, `while` or `for` body,
a nested block, or an `if` branch. Jumping straight to it would skip the
`ScopeEnter`/`Alloc` of every block between the body and the label, so
the jump descends instead. Such a switch has a selector register `sel`
(0 at entry); label k's stub sets `sel = k + 1` and jumps toward the
first statement on its path. Each statement on a nested label's path
that opens a scope with code (a block, an `if`, a loop's init or
per-iteration scope) is a *hunt site*: the jump lands before the scope
entry, and right after it `JmpIfNot sel` skips to the ordinary code,
else a jump (one label below) or a compare per label (several) goes on
toward the next site. At the label the jump lands on a pad that clears
`sel` and enters the label's code, so the ordinary path, and every later
iteration of the enclosing loops (increment, condition, `continue`,
`break`), runs with `sel == 0`: a site costs one not-taken branch then.
A site whose scope emits nothing is no site; a statement off every path
is compiled as before. The jump bypasses only declarations sema allows
([stmt.dcl]/3: vacuous initialization), and block locals are allocated
at scope entry, so a bypassed variable exists, uninitialized, at the
label. A label the descent cannot reach (none known) traps at the label
with "a 'case' label in this statement is not supported yet".

Where clang's evaluator diverges from the program's run-time result,
kiln follows the run time: a bypassed class object with a trivial
default constructor is usable after the label (clang starts only
variables with no initializer), and a `for` entered by the jump goes on
with its increment (clang re-runs the loop from its init-statement).
Tests: `tests/conformance/stmt.switch/nested-labels{,-run,
-clang-divergence}.cpp`, `tests/conformance/expr.const/duffs-device.cpp`;
bench `bench/oven/switch-dense.cpp` (the ordinary path).

### Coroutines — rejected (WP-351)
A coroutine is never a constexpr function ([dcl.constexpr]/3; Sema
rejects `constexpr`/`consteval` coroutines and drops the implicit
constexpr of a coroutine lambda, sema.md §7.13), so a constant
evaluation reaches one only through a call Sema already reports as
"non-constexpr function ... cannot be used in a constant expression",
or through a `__builtin_coro_*` builtin, which is not a constexpr
builtin (no note, as clang). Defensively, the compiler lowers a
`CoawaitExpr`, `CoyieldExpr` or `DependentCoawaitExpr` and a
`CoroutineBodyStmt` or `CoreturnStmt` to a trap reporting clang's
generic "subexpression not valid in a constant expression" at the
expression or statement; no coroutine frame is ever built at compile
time. Test: `tests/conformance/dcl.fct.def.coroutine/
constant_evaluation.cpp`.

## Status and what remains
Implemented (M2): integers of every width incl. `__int128`, `bool`,
character types, binary32/64, x87 `long double`, binary128, enums;
pointers to objects, arrays and functions; **references** (lowered as
pointers: local, parameter, member, global and return references, bound
to glvalues); arrays, structs, unions, bit-fields; all C control flow (if,
loops, switch incl. fallthrough, break/continue, return); labels and
`goto` (diagnosed as clang does); calls and recursion; constexpr variables
and functions; `static_assert`; compound assignment, comma, conditional,
casts, `sizeof`/`alignof` (of a reference type: the referred type's;
`alignof e` naming a variable or data member is its declared alignment,
`SemaBridge::alignof_decl`, clang `GetAlignOfExpr`, spec/abi.md §3.2;
`alignof(T)` where `T`'s sugar has a typedef with `aligned` is that
typedef's alignment via `SemaBridge::opaque_size_align`, as the Oven's
canonical type model drops it, WP-335);
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

Implemented (C++26, WP-357): P2747 placement new (single objects and
arrays, "Transient allocation"), P3068 exceptions ("Exceptions — Landed
(WP-357)") and P2280 unknown references ("Unknown references — Landed
(WP-357)"); WP-368 closed the WP-357 leftovers, WP-383 the pointer
handler converting a pointer to a base-class subobject to a virtual base
of its most derived object. P3533 virtual inheritance: WP-388 ("Virtual
bases").

Remaining (each traps with a `kiln:` note today):
- Lifetime of objects outside `std::allocator` storage (double
  destruction of a local, reads after an explicit destructor call), and
  the other WP-212 deviations listed under "Transient allocation".
- `delete` through `void*`.
- Front-end gaps that keep some forms from reaching the Oven (not
  Oven work): class-specific `operator new`,
  `delete[] (expr)` when `expr` starts with two identifiers (clang's
  lambda lookahead) and
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

**Landed (WP-356): lambdas with an explicit object parameter, condition
bindings.** In a lambda call operator the closure object is the implicit
object parameter or, with an explicit object parameter
([expr.prim.lambda.closure]/5, P0847), that parameter (or its referent)
converted to its closure-type base when its type is derived from the
closure type (non-virtual bases only, as sema requires; clang
`EmitLValueForLambdaField`); the prologue (`compile/lambda.rs`) keeps it
in `closure_reg`, and captures are read through it while `this` in the
body is the captured one. An overloaded operator call whose callee is an
explicit object member function passes the converted object as the
first argument, as a plain call does ([over.call.func]/3). A structured
binding declared as the condition of `if`, `while`, `for` or `switch`
([stmt.pre]/4, P0963) initializes its tuple-like bindings (the `get`
calls) after the condition is evaluated (`cond_bindings`); in a loop
their registers are kept to the end of the loop statement
(`tests/conformance/dcl.struct.bind/p0963-{binding,switch}-condition.cpp`,
`tests/conformance/dcl.fct/p0847-lambda-explicit-object*.cpp`).

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
- `crates/kiln_oven/src/profile.rs` unit tests and
  `crates/kiln_driver/tests/constexpr_profile.rs` (WP-1116, "Profile
  counters"; [expr.const]/22): exact steps and calls of a recursion, a
  loop (equal to `-fconstexpr-report`'s), a known constexpr heap peak
  and reclaimed bytes, failed evaluations and trials, consteval memo
  hits, the deterministic ranking with its tie-breaks, the limit, NDJSON
  well-formedness, nothing printed without the flags, and an importer
  reporting only its own evaluations (also `module_cache.rs`).
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
  Static reclamation (WP-322): every dead static is reclaimed wherever it
  lies, slot reuse makes old pointers stale, top ranges coalesce and are
  given back, and inside a trial nothing is reused nor lowered below the
  checkpoint's top.
- `tests/conformance/expr.const/oven-dead-statics.cpp` (WP-322, kiln only,
  `oven: static_bytes`): the static space does not keep the scratch copies
  of 32 constant-destruction checks.
- `tests/conformance/expr.const/constant-p-trial*.cpp` (WP-135): a failed
  trial's callee writes, unions and allocations, nested trials, kept
  effects of a successful trial, and (kiln only, `oven:` budget) heap
  rollback and memory O(1) in the number of failed trials.
- `tests/conformance/expr.new/p2747-placement-new*.cpp` (WP-357,
  `requires: clang>=22`): placement new of objects and arrays, the misuse
  notes, the pre-C++26 rule; `expr.const/p2280-unknown-reference*.cpp`
  likewise for P2280. WP-375: `expr.new/constexpr-placement-ok.cpp`
  (a run test: `std::allocator` storage, heap elements, union members,
  `std::construct_at`), `constexpr-placement.cpp` (the forms still not
  constant), `p2747-placement-churn.cpp` (`oven:` budget, `mem<=512`);
  `expr.const/p2280-ok.cpp` (`std::array` through a reference),
  `p2280-errors.cpp`, `p2280-unknown-reference-mem.cpp` (`oven:`
  budget). `expr.throw/constexpr-exceptions*.cpp`,
  `constexpr-uncaught.cpp`, `constexpr-throw-churn.cpp` (`mem<=512`),
  `constexpr-throw-ctor-throws.cpp` (`oven:` budget),
  `except.handle/constexpr-*.cpp`, `except.ctor/constexpr-*.cpp` and
  `except.uncaught/constexpr-uncaught-exceptions.cpp` (WP-357, WP-368,
  `skip(clang)`: no released clang implements P3068) for exceptions;
  `except.uncaught/uncaught-exceptions-libcxx-not-constexpr.cpp`
  (oracle-compared) for libc++'s non-`constexpr` declaration.
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
