# spec: kiln_codegen — typed AST to LLVM IR (M2)

Status: **M2 subset implemented** (WP-052 lowering, WP-052b driver, IR-diff corpus and `run` tests). Deviations are listed in §6. This spec is normative for `crates/kiln_codegen`.
Reference: for every construct, the reference output is clang 19's
`-S -emit-llvm -O0 -std=c++26 --target=<triple>`. Targets are
`x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`.
Companions: `spec/llvm.md` (the IR model), `spec/abi.md` (signature
classification), `spec/oven.md` (constant evaluation).

## 1. Scope
The M2 subset covers the following:
- fundamental types, arithmetic and conversions;
- pointers, arrays and enums;
- structs without member functions;
- functions and all of C's control flow;
- constexpr functions and variables, and `static_assert`;
- string literals and `extern "C"`;
- lvalue and rvalue references as parameters, return types and locals
  (WP-052b), lowered as clang does: a `ptr` with `nonnull`,
  `dereferenceable(N)` and `align A` for a complete object referee, an
  `.addr` slot, and a load of the pointer at every use.

A construct outside M2 is a `CodegenError` ("not supported in M2"), never a
panic and never silently wrong IR.

## 2. Architecture
- `lib.rs`: `emit(ast, oven, target, opts) -> Result<Module, CodegenError>`.
- `fold.rs`: a pre-pass that asks the Oven to fold initializers and
  constant-reference loads (the `Folds` table). A const var is foldable
  only when it is non-volatile and either static storage, integral or enum
  (this mirrors clang's `isUsableInConstantExpressions` + `tryEmitAsConstant`).
  A constant initializer ([basic.start.static]/2: static storage,
  `constexpr`, or a `const` non-volatile integral/enum local) folds with
  `Oven::fold_constant_init`, so `__builtin_is_constant_evaluated()` is
  true in it; the other foldable initializers (local `const` arrays,
  records and references) use `try_fold`, where it is false (WP-063,
  matching clang's IR). Calls of builtins with an integral or floating
  result are candidates too (clang `EmitBuiltinExpr` tries
  `EvaluateAsRValue` first), except `__builtin_is_constant_evaluated()`
  (WP-088).
- `builtins.rs`: builtin calls (§3, "Calls"; WP-088).
- `consts.rs`: turns Oven byte images into `kiln_llvm` constants. It has a
  memory form (bool is `i8`) and a scalar form (bool is `i1`), plus string
  literal globals.
- `fir.rs`: `Fir`, a mutable per-function instruction buffer. It allows
  erasing, RAUW, block reordering and forwarding-block simplification,
  which clang does at -O0 but the append-only `kiln_llvm` builder cannot.
  `Fir::lower` writes the result into the module in one pass.
- `module.rs`: `Cg`, the module-level state. It owns type lowering,
  signatures (`kiln_abi::call`), mangling, globals, linkage and function
  attribute sets.
- `func.rs`: per-function state (`Fx`) and the prolog and epilog: the
  return slot, the main `retval` store of 0, parameter spills, the
  fall-off-the-end trap/unreachable, mustprogress, and helpers for
  memcpy/memset/GEP/casts.
- `expr.rs`: scalar, lvalue and aggregate expression emission (§3).
- `stmt.rs`: statement emission (§4).

## 3. Expressions (clang CGExprScalar/CGExprAgg/CGDecl rules mirrored)
`expr.rs` mirrors clang's `ScalarExprEmitter`, `EmitLValue`,
`AggExprEmitter`, `EmitBranchOnBoolExpr` and `EmitAutoVarDecl`.
- Conversions follow `EmitScalarConversion`:
  - to bool: `icmp ne 0`, `fcmp une 0.0` or `icmp ne null`;
  - integer to integer: cast by source signedness;
  - integer to float: `sitofp` or `uitofp`; float to integer: `fptosi` or
    `fptoui`, chosen by destination signedness;
  - float to float: `fptrunc` or `fpext`, chosen by rank;
  - pointer to and from integer: through `i64`;
  - casts of constants fold as LLVM's constant folder does, including
    integer to `fp128` (round to nearest even).
- Arithmetic:
  - signed add, sub and mul get `nsw`; shifts mask nothing at -O0;
  - pointer arithmetic: `gep inbounds` (`gep i8` for void and function
    pointers); pointer difference: `ptrtoint`, `sub`, then `sdiv exact`;
  - floating add/sub (also compound) contract a single-use `fmul`
    operand (looking through a single-use `fneg`) into
    `llvm.fmuladd.*`, as `tryEmitFMulAdd` does under `-ffp-contract=on`.
- Values:
  - an lvalue-to-rvalue conversion of a glvalue `?:` is emitted as a
    scalar `?:`: each arm loads in its own block and the result is a phi;
  - `&&`/`||` produce an `i1` phi; `EmitBranchOnBoolExpr` splits
    `&&`/`||`/`!`/`?:` conditions into branches and folds constants;
  - bit-fields load the storage unit, then `shl`/`ashr` (signed) or
    `lshr`/`and` (unsigned); stores mask, clear and `or` into the unit;
  - `++`/`--` on bool, integers, floats and pointers, including bit-fields.
- Calls:
  - a call site gets `nounwind` only when the callee's prototype is nothrow;
  - a variadic call prints its function type;
  - an indirect (sret) return writes into the destination slot when the
    call initializes an aggregate, otherwise into a temporary.
  - a call of a builtin (callee a `BuiltinFnToFnPtr` cast of a
    `kiln_ast::decl::builtin` function) is never a call of the builtin
    (`builtins.rs`, clang `EmitBuiltinExpr`; landed in WP-088):
    - a call with an integral or floating result is a fold candidate
      (§3, `fold.rs`); if the Oven folds it, it is that constant;
    - `__builtin_is_constant_evaluated()` is `i1 false`, since code that
      runs is not manifestly constant-evaluated (it is never a fold
      candidate);
    - `__builtin_addressof(x)` is `&x`, `__builtin_launder(p)` is `p`;
    - clz/ctz (`l`/`ll`/`g`) are `llvm.ctlz`/`llvm.cttz` with
      is-zero-poison true on x86_64 and false on AArch64
      (`isCLZForZeroUndef`), the result cast unsigned to the call type;
      the `g` forms with a fallback use true and `select (x == 0)`;
      popcount is `llvm.ctpop`, bswap `llvm.bswap`;
    - isfinite/isinf/isnan/isnormal are `llvm.is.fpclass` (tests 504,
      516, 3, 264) zero-extended; signbit bitcasts to an integer of the
      float's width and tests `slt 0`; the isgreater family is one
      `fcmp` (`ogt oge olt ole one uno`) zero-extended;
    - memcpy/memmove are `llvm.memcpy`/`llvm.memmove` (`i64` length,
      not volatile) with each pointer's known alignment as a parameter
      `align` (clang `EmitPointerWithAlignment`: implicit casts and
      casts of a declaration keep the inner alignment, other casts take
      the target's pointee alignment, `&x` and array decay the lvalue's,
      otherwise the pointee's natural alignment, 1 for `void`/incomplete);
      the value is the destination pointer;
    - frexp is `llvm.frexp`, the exponent stored at `int` alignment;
    - `__builtin_operator_new`/`_delete` call the global replaceable
      function sema chose, as a new-expression does (`builtin` call-site
      attribute, `allocsize`, `noalias nonnull`);
    - `__builtin_unreachable()` is `unreachable` followed by a fresh
      block; an expression statement that leaves a fresh block with no
      predecessors erases it (clang `EmitStmt`), and `FinishFunction`
      erases a return block without predecessors (and a then-unused
      `retval` slot);
    - the library builtins (memcmp, memchr, char_memchr→`memchr`, strlen,
      wcslen, wmemchr, wmemcmp, nan*/nans* of a non-literal, and the math
      functions exp/exp2/expm1/ldexp/pow/scalbn/scalbln with their
      `f`/`l` forms) call the function of the unprefixed name, declared
      with the builtin's type, `nounwind`, and `willreturn
      memory(read)` / `memory(none)` for `Pure` / `Const` builtins (on the
      call site too).
  - a conversion to bool of a `zext` from `i1` is the `i1`, and the
    `zext` is erased when unused (clang `EmitIntToBoolConversion`);
  - the IR constant folder propagates NaNs as APFloat does: a NaN
    operand's payload (quieted), the left one first; an invalid
    operation of non-NaN operands gives the positive default NaN.
- Names (via `kiln_abi::mangle`): namespace-scope variables with internal
  linkage are mangled (`_ZL1h`); static locals use 0-based
  discriminators per (function, name): none, then `_0` ... `_9`, then
  `__10_`, `__11_`, ...
- A GNU asm label (`asm("name")`, recorded on the entity's first
  declaration, spec/ast.md §4.2) replaces the function's or variable's
  linkage name (`kiln_abi::mangle::asm_label`). clang prefixes LLVM's
  `\01` "no global prefix" marker only when the target's user-label
  prefix is non-empty (Mach-O); every kiln target is ELF, so the label
  is the symbol verbatim, and a `\01` the user wrote is kept (LLVM
  strips it when emitting).
- Local initialization (`EmitAutoVarInit`):
  - a constant aggregate goes through `emitStoresForConstant`. That means
    a single store, memset-0 plus a few stores, a bytewise memset, or a
    memcpy from a `@__const.<fn>.<var>` private global, in that order of
    preference. Constants are never promoted to globals at -O0;
  - a non-constant aggregate goes through `EmitAggExpr`, with
    `CheckAggExprForMemSetUse` (size > 16 and nonzero*4 <= size gives a
    memset);
  - `EmitArrayInit`: element 0 is stored at the base, element i at
    `gep inbounds elem, base, i64 i`, and a phi filler loop covers
    elements without an initializer.
- Constants (`consts.rs`): a union constant is its active member followed
  by `[N x i8] undef` padding, typed as the union when the element types
  match its IR type, else as a literal struct. Records with bit-fields and
  base classes are still M3 errors in constants.

### 3.1 New- and delete-expressions (`new_delete.rs`, WP-061a)
clang's `EmitCXXNewExpr`, `EmitCXXNewAllocSize`,
`EmitNewArrayInitializer`, `EmitCXXDeleteExpr` and `EmitDeleteCall` at
`-O0`, for element types without constructors or destructors (so no
array cookies).
- **Allocation functions.** `alloc_fn` classifies a global `operator
  new`/`new[]`/`delete`/`delete[]` as clang does
  (`isReplaceableGlobalAllocationFunction`: `size_t` or `void*` first,
  then optionally `align_val_t`/`size_t`/`const std::nothrow_t&`;
  `isReservedGlobalPlacementOperator`: `(size_t, void*)` and `(void*,
  void*)`). A replaceable one's declaration or definition gets
  `nobuiltin`, and `operator new` also `allocsize(0)` and the
  `nonnull`/`noalias` return attributes clang adds implicitly
  (`alloc_fn_attrs`, applied in `module.rs::get_fn` and
  `func.rs::emit_fn_def`).
- **Calls** (`emit_new_delete_call`, clang's `EmitNewDeleteCall`): a
  replaceable callee called from a new- or delete-expression gets the
  `builtin` call-site attribute; an `operator new` call also
  `allocsize(0)` and the result attributes (`noalias`, `nonnull` unless
  nothrow, `align N` with `N` the allocation alignment:
  `default_new_align` (16) or the type's alignment when aligned, raised to
  `bit_floor(min(16, sizeof(T)))` for a replaceable, unaligned
  allocation, as clang's `EmitCXXNewExpr`). An
  `align_val_t` argument is the constant alignment (a non-constant one
  is unsupported).
- **Size** (`EmitCXXNewAllocSize`): the count is Sema's `size_t`
  conversion of the first bound. A constant count folds, to `-1` (all
  ones) on overflow or when the count is below the number of explicit
  initializers. A dynamic count compares `icmp ult` with that number
  (when there are initializers) and, unless the element size (times the
  constant inner bounds) is 1, is multiplied with
  `llvm.umul.with.overflow.i64`; the overflow bits are or-ed and a
  `select` gives `-1`, in clang's instruction order. The flattened
  element count of a multidimensional new is the count times the inner
  bounds.
- **Null check.** A `noexcept` allocation function that is neither the
  reserved placement form nor a replaceable non-nothrow `operator new`
  may return null: when there is an initializer, the result is compared
  with `null` and initialization runs in a not-null block, joined by a `phi`
  of the result and `null`.
- **Cleanup emulation.** clang pushes an EH cleanup that frees the memory
  if initialization throws. kiln has no EH yet: when the initializer
  cannot throw, the cleanup is never used, but clang's -O0 IR still shows
  its traces. Inside a conditional evaluation (an arm of `?:`, the right
  of `&&`/`||`; `FunctionCtx::conds`, pushed by `cond_eval`), the
  allocation's pointer and the placement arguments not defined in the
  entry block are saved to `alloca`s (preferred alignment:
  `pref_align`: at least 4 for integers of 2 to 16 bits on aarch64),
  and an `i1` active flag is stored `false` before the terminator of
  the outermost conditional's start block and `true` after the call. A
  may-throw initializer is unsupported.
- **Initialization.** A scalar or aggregate initializer is stored
  through the result (`EmitAggExpr` rules of §3). An array
  (`EmitNewArrayInitializer`) stores the explicit elements, moving a
  cursor with `gep i64 1`, then zeroes the tail with one `memset` when
  the filler is value-initialization (`()`, `{}`, an implicit value
  init, or a `struct` all of whose members are value-initialized:
  clang's `struct`-only rule), else loops over the remaining elements up
  to `gep base, count` with a `phi` cursor stepped by `gep i32 1`. A
  multidimensional array new is flattened to its innermost element.
- **Delete** (`EmitCXXDeleteExpr`): null check, a `gep inbounds` with `i32 0`
  indices down to the first scalar element for a pointer-to-array
  operand, then the call with the size (`i64`, the
  object size) and the alignment as the deallocation function's
  parameters require. The call is `nounwind` (the callee is `noexcept`).
- **Unsupported** (`CodegenError`): member-pointer element types, string
  literal array initializers, a placement argument of class type inside
  a conditional, non-constant `align_val_t` arguments, the deallocation
  functions clang does not treat as usual, and every construct Sema
  reports as unsupported (classes needing constructors or destructors).

## 4. Statements
- `stmt.rs` covers compound, declaration, expression, null, if (with
  init statement; constexpr and consteval if), switch (with case chains,
  default, fall-through), while, do, for, break, continue, return,
  goto and labels, attributed statements and `static_assert` (no code).
- A function's return goes through the `retval` slot and a single
  `return` block, as clang does; an sret return stores into the sret
  pointer.
- Block layout, forwarding-block removal and the dead-block cleanup
  follow clang -O0. A statement with no insertion point is skipped unless
  it contains a label (`ContainsLabel`).
- Each loop gets a distinct `!llvm.loop` node with
  `llvm.loop.mustprogress`. A loop with a constant-true or absent
  condition and an empty body clears `mustprogress` on the function and
  gets no loop metadata.
- Case ranges (GNU) are an error.

## 5. Driver (`kiln_driver::codegen_cmd`, `link`)
The normative description is spec/driver.md "Code generation". In short:
- `-S -emit-llvm` writes a `.ll` file; `-c -emit-llvm` pipes it to
  `llvm-as-19`.
- `-S` pipes the IR to `llc-19`; `-c` to `llc-19 -filetype=obj`.
- Linking compiles each source to a temporary object, then runs
  `clang++-19`, passing `-fuse-ld=`, `-l`, `-L`, `-Wl,`, `-static`,
  `-nostdlib`, `-shared`, `-rdynamic`, `-pie` and `-no-pie` through in
  command-line order.
- Tools come from `$KILN_LLVM_DIR/bin` (default `/usr/lib/llvm-19`), else
  the versioned name on `PATH`.
- `-O<n>` selects `llc`'s level and is passed to the link; the IR is
  always the `-O0` IR. `-g` is ignored. `-###` and `-v` print the steps.
- Every job calls `ConstexprOpts::write_report`.

## 6. Validation
- **Unit corpus** (`crates/kiln_codegen/tests/lower.rs`, WP-052): 74
  inline C++ programs, lowered for `aarch64-unknown-linux-gnu` straight
  from `parse_tu` (no driver). Every case must pass
  `opt-19 -S -passes=verify`. `Exact` cases must also equal
  `clang++-19 -S -emit-llvm -O0 -std=c++26 --target=aarch64-unknown-linux-gnu`
  after normalization. A case that differs but is marked `VerifyOnly` is
  a documented deviation (below); one that becomes exact is reported so
  it can be promoted. `KILN_CASE=<name>` prints both texts.
- **Normalization:** both texts go through `opt-19 -S -passes=verify`,
  which renumbers values and attribute groups canonically. Then the
  `; ModuleID` line, `source_filename`, `!llvm.ident` and its
  `clang version ...`/`kiln` string node are dropped. Nothing else is
  normalized: attributes, metadata, alignment, names and declaration
  order must match.
- **IR-diff corpus** (WP-052b): `crates/kiln_codegen/tests/ir_vs_clang.rs`
  compiles each `tests/codegen/*.cpp` in process through
  `kiln_driver::run` (`-S -emit-llvm -O0 -std=c++26 --target=<t> -o -`)
  and through `clang++-19` with the same flags, on
  `aarch64-unknown-linux-gnu` and `x86_64-unknown-linux-gnu`, under the
  same normalization. A `(case, triple)` pair in its `DEVIATIONS` table
  must only verify; every other pair must be identical. `KILN_CASE=<stem>`
  prints both texts; without `opt-19`/`clang++-19` it is skipped. The
  corpus is 56 programs (ABI classification of 1- to 32-byte structs,
  HFAs, byval, varargs, `__int128`, `long double`, bit-fields, static
  locals, string literals, references, control flow, new/delete, and the
  WP-088 `builtins_{bits,mem,fp,misc}` cases): 112/112 pairs exact,
  `DEVIATIONS` is empty.
- **`run` tests:** 33 printf-based programs (`extern "C" int printf`, no
  headers) under `tests/conformance/<section>/`, each naming its section,
  are compiled, linked and run by kiln and by clang, and their stdout and
  exit status compared. The 5 header-dependent `run` tests
  (`basic.start.main` argv/environment/return-value/stdin and
  `stmt.ranged/constexpr-sum`) are `xfail(kiln)`: libstdc++'s
  `c++config.h` uses `decltype`, which kiln_parse does not support yet.

### Deviations
- **Constant-initialized reference variables are not folded.** clang
  emits a use of a reference that is usable in constant expressions as a
  direct use of its referee (`load i32, ptr @g`); kiln loads the
  reference's slot. The Oven does not fold reference initializers yet,
  so `fold.rs` has nothing to substitute. No corpus case covers it.
- **Namespace-scope and static reference variables** are diagnosed
  (`dynamic initialization of a global is not supported yet (M3)`), even
  when clang would constant-initialize them, for the same reason.
- **Reference members** (`struct S { int& r; }`) are diagnosed (`a
  reference member is not supported yet`).
- **`dereferenceable(N)` of a reference to a class** uses the class's
  non-virtual size (`nv_size`, at least 1). clang's
  `getMinimumObjectSize` uses the full size for a `final` class; with no
  virtual bases in M2 the two agree.
- **Call results stored through a coercion** (`CreateCoercedStore`) use
  the object size, where clang uses the type's data size. They differ
  only for a class with tail padding reusable by a derived class, which
  M2 does not have.
- Sema gaps (kiln_sema) that keep `run` tests and corpus cases simple:
  casts to reference types (`static_cast<int&&>`), binding
  `const T(&)[N]` to a `T[N]` argument, copy-initializing a class object
  from an lvalue (so a function cannot return a named local or a
  parameter of class type), `S{...}` temporaries, and binding a
  reference to a temporary. In kiln_codegen itself, a constant
  initializer of an aggregate with bit-fields (`Flags f = {};`) reports
  `bit-field constants are not supported yet (M3)`, without a location.
- **New/delete (§3.1).** The EH cleanup is emulated only for
  initializers that cannot throw; clang's `invoke` + landing pad for a
  throwing one waits for EH. The placement-argument alignment is
  approximated by `pointer_with_align` (declaration alignment for named
  variables, array elements and members; otherwise the pointee type's
  alignment). Corpus cases do not use `alignas` on globals (ignored by
  codegen, a pre-existing gap) or `if (c && new T)` (clang folds the
  non-null `new` in `EmitBranchOnBoolExpr`; kiln emits the branch). The
  `class` vs `struct` `memset` distinction is implemented but not
  covered (access specifiers do not parse yet).
- Parser gaps (kiln_parse) that keep cases out of the corpus: returning
  a braced-init-list, `consteval`, range-based `for`. (`noexcept` and
  `if constexpr`/`if consteval` landed with WP-058: `calls_noexcept`,
  `noexcept_operator`, `if_constexpr_true`, `if_constexpr_false` and
  `if_consteval` are exact; a `CXXNoexceptExpr` lowers to its stored
  `bool` constant and `fold.rs` does not look inside it.)

## Status and what remains
**Done:** WP-052 (lowering, §2–§4; unit corpus 79/79 exact on aarch64)
and WP-052b (driver §5; IR-diff corpus 92/92 exact on both triples; 33
`run` tests). WP-052b also fixed, additively: `kiln_llvm`
`Module::set_global_value_type` (a static local retyped by its
initializer keeps its position, as clang's), and kiln_sema's implicit
move, which no longer treats an lvalue-reference variable as movable.

**Done (WP-052 + WP-052b):** `fir.rs`, `module.rs`, `func.rs`, `consts.rs`,
`fold.rs`, `expr.rs` (§3), `stmt.rs` (§4), the driver path (§5) and the
IR-diff corpus (§6: 46 programs, exact on both triples). The unit corpus
is 79/79 exact against clang on aarch64 (`bitfields_compound` became exact
with WP-057; WP-058 added five cases). Additive `kiln_abi` fixes came with
part 1: internal-linkage variable mangling, static-local discriminators,
and indirect returns with `byval` (clang's `getNaturalAlignIndirect`, so
no `result.ptr` spill).

**Remaining (M3):**
1. Oven folding of constant-initialized references; then global and
   static reference variables and the direct-referee use (deviations
   above).
2. The Sema gaps above, then corpus cases for them: class copy-init,
   braced returns, reference casts, temporaries, range-for over arrays.
3. Bit-field and base-class constants, case ranges, reference members.
4. Header-dependent `run` tests, once `decltype` parses (drop their
   `xfail(kiln)`).
