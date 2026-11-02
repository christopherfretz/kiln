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
- `lib.rs`: `emit(ast, oven, target, opts) -> Result<Module, CodegenError>`;
  `emit_with_diags` also returns the warnings codegen reports itself
  (`Cg::diags`, `kiln_diag::Diag`s in emission order, as clang's
  `CGM.getDiags()`; WP-165), returned even when emission fails.
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
  (WP-088). A `CXXDefaultArgExpr` has no children, so the pre-pass walks
  its parameter's default argument once, at the first use (WP-136).
  The pre-pass walks every statement codegen emits, including try blocks
  and their handlers (WP-214: a default argument naming a non-inline
  `static const` member, first used in a try block, loaded the
  `available_externally` member and failed to link; libc++'s
  `memory_resource::__max_align`). Validation:
  `tests/codegen/static_member_odr_use.cpp`.
  Non-initializer candidates (a load of a const variable, a builtin
  call) fold with `Oven::fold_rvalue` (lax mode, WP-165): like clang's
  `tryEmitAsConstant`, a read of any `const` non-volatile variable with an
  initializer folds, even one not usable in constant expressions (`const
  double`, `const _Atomic(int)`, a `const` array or class element), so a
  global whose every use folds is not emitted. A load of a const variable
  is recognized through parentheses and `NoOp` casts (the xvalue cast of
  an implicitly moved `return x;`). A read of a `mutable` member is not
  folded (the Oven's "read of mutable member" check, spec/oven.md); the
  variable's other members are.
  A reference variable whose referee type is `const` non-volatile folds
  the same way (clang `checkVarTypeForConstantEmission`:
  `CEK_AsValueOrReference`), so `a + b` for `const int& a = {}; const int
  b = 5;` is a constant (WP-231, WP-233; see also "Reference
  read-through").
  Validation: `tests/codegen/const_fold_lax.cpp`,
  `tests/codegen/ref_const_fold.cpp`.
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
  attribute sets. `Cg::target_attrs` gives every function clang 19's
  target string attributes: on AArch64 `"target-cpu"="generic"` and
  `"target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"`, or
  `"+fp-armv8,+neon,+v8a,-fmv,-outline-atomics"` when
  `CodegenOptions::outline_atomics` is false (`-mno-outline-atomics`,
  WP-226; `tests/codegen/no_outline_atomics.cpp`); on x86-64
  `"target-cpu"="x86-64"`, the baseline features and
  `"tune-cpu"="generic"`.
- `func.rs`: per-function state (`Fx`) and the prolog and epilog: the
  return slot, the main `retval` store of 0, parameter spills, the
  fall-off-the-end trap/unreachable, mustprogress, and helpers for
  memcpy/memset/GEP/casts.
- `expr.rs`: scalar, lvalue and aggregate expression emission (§3).
- `stmt.rs`: statement emission (§4).

### 2a. Emission order (WP-172)
Function and global positions in the module are **creation order** (the
first `get_fn`/`get_gvar` of a name creates its declaration); a later
definition fills the existing declaration in place. A declaration whose
mangled name already names a function of the same type is that function
(clang `GetOrCreateLLVMFunction`): a runtime function codegen declared
first (`__cxa_allocate_exception` for a `throw`) and a user's
`extern "C"` redeclaration of it in another namespace ([dcl.link]/7)
share one symbol, never a renamed `name.1` copy. Emission order still
matters, because it decides where declarations created inside a body
land, the order in which comdats are created (at definition), and
nounwind inference: clang's `TryMarkNoThrow` marks a definition
`nounwind` when every call in it is to a `nounwind` callee, so a callee
defined earlier turns a caller's `invoke` into `call`. kiln mirrors
clang's `CodeGenModule` pipeline:
1. **Top-level walk** (`emit_members`, clang `HandleTopLevelDecl` per
   declaration, in parse order). Strong definitions are emitted now.
   Discardable ones (`linkonce_odr`: inline functions, implicit
   instantiations, ...) go to `deferred` (clang `DeferredDecls`) unless
   their name was already referenced, in which case they go straight to
   `to_emit` (`DeferredDeclsToEmit`). The first reference to a deferred
   name (`get_fn`/`get_gvar`) moves its definition to `to_emit`. A
   variable with a side-effecting dynamic initializer is must-emit and is
   emitted at its `HandleTopLevelDecl`.
2. **Key-function vtables** (`emit_key_function_vtables`, clang
   `DefineUsedVTables` -> `HandleVTable`).
3. **End-of-TU instantiations** (`emit_end_of_tu_instantiations`). Sema's
   end-of-TU drain (`kiln_sema::templ::pending`) records each definition
   it instantiates in `TemplateTables::end_of_tu_defs`, in clang's order:
   every instantiation is recursive (`GlobalEagerInstantiationScope`), so
   after a body is instantiated, the vtables and definitions it requires
   are instantiated before the rest of the enclosing queue (depth-first,
   pre-order; an already queued definition is never queued again, and
   constexpr and deduced-return functions are instantiated at their
   use, [temp.inst]/5). Codegen holds these definitions back from the
   top-level walk (`Cg::held`: `inline_def`, `instantiated_var_def` and
   the walk's `emit_global_*` skip them, including instantiated
   out-of-line static member definitions that sema appends to the TU)
   and replays them through `emit_global_*` in that order, which is
   clang's `HandleTopLevelDecl` of each instantiation (a static data
   member's replayed definition is its instantiated out-of-line
   redeclaration, initializer or not: `template<class T> T A<T>::s;`; a
   variable template specialization used before its template was
   defined has no redeclaration and is replayed itself, WP-238).
4. **`emit_deferred`** (clang `EmitDeferred`): deferred vtables first,
   then `to_emit`, recursing depth-first into the names each definition
   newly references.

No deviation from clang's function order is known for constexpr
function templates: one first instantiated inside a constant expression
(a `static constexpr` initializer, a `constexpr` function pointer, a
call from a dependent body) lands at the same module position as in
clang, so callers get the same `nounwind` inference (WP-117 reported a
difference that WP-172's pipeline already removes; checked again in
WP-204). Validation: `tests/codegen/constexpr_instantiation_order.cpp`.

Deviation: clang's `applyReplacements` moves a base-object structor
(`D2`/`C2`) that replaces a complete-object alias to the position of the
replaced declaration; `kiln_llvm` cannot reorder functions, so kiln keeps
it at its creation position (only visible when a complete-object
structor declaration is created before its base variant is defined, e.g.
a virtual destructor of a template base called from a cleanup in a
strong constructor). Validation: `tests/codegen/instantiation_order.cpp`,
`tests/codegen/virtual_template_out_of_line.cpp`.

## 3. Expressions (clang CGExprScalar/CGExprAgg/CGDecl rules mirrored)
`expr.rs` mirrors clang's `ScalarExprEmitter`, `EmitLValue`,
`AggExprEmitter`, `EmitBranchOnBoolExpr` and `EmitAutoVarDecl`.
A `CXXRewrittenBinaryOperator` emits its semantic form in every one of
them, the aggregate path included (a reversed `<=>` with a class result).
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
  - an lvalue-to-rvalue conversion of a reference-returning call, also
    through the lvalue `CK_UserDefinedConversion` of a conversion
    function returning a reference (`operator bool&()`), loads from the
    returned address (clang `EmitLoadOfLValue`);
  - `&&`/`||` produce an `i1` phi; `EmitBranchOnBoolExpr` splits
    `&&`/`||`/`!`/`?:` conditions into branches and folds constants;
  - bit-fields load the storage unit, then `shl`/`ashr` (signed) or
    `lshr`/`and` (unsigned); stores mask, clear and `or` into the unit;
  - `++`/`--` on bool, integers, floats and pointers, including bit-fields.
- Calls:
  - a call site gets `nounwind` only when the callee's prototype is
    nothrow, or the direct callee has a `NoThrow` attribute (a
    recognized C library function, spec/sema.md §5.4) or a `const` /
    `pure` attribute; the declaration and a definition get it too;
  - a written or implicit `const` / `pure` attribute (not on a builtin,
    whose row decides) adds `willreturn memory(none)` /
    `willreturn memory(read)` to the declaration, the definition and
    every call site: direct, member (`emit_method_call` /
    `call_extra`), and virtual (the named method's attributes) (clang
    `ConstructAttributeList`). When the return or some argument is
    passed by address (`sret`, `byval`, or a pointer to a temporary for
    a class that is not trivially copyable: `Sig::passes_indirect`),
    the access widens to `argmem: readwrite` (clang
    `AddPotentialArgAccess`: `memory(argmem: readwrite)` /
    `memory(read, argmem: readwrite)`; without it LLVM rejects an
    `sret` `writable` pointer). A template specialization carries the
    pattern's attributes (spec/sema-templates.md §5)
    (`tests/codegen/const_pure_call_sites.cpp`, WP-182);
  - a GNU `cold` / `hot` attribute on some declaration of the callee adds
    `cold` / `hot` to the declaration, the definition and direct call
    sites, member calls included (`builtins::cold_hot_attrs`; WP-172,
    `tests/codegen/cold_hot.cpp`; glibc's `abort` is `__COLD`). C library
    functions recognized by name stay ordinary calls: clang's lowering of
    a `BuiltinAttr` library call (`memcpy` to `llvm.memcpy`, a folded
    `strlen("abc")`) is not done (a follow-up);
  - a variadic call prints its function type;
  - an indirect (sret) return writes into the destination slot when the
    call initializes an aggregate, otherwise into a temporary; an
    ignored aggregate call makes that temporary itself, after the
    arguments, and an empty-class (`Ignore`) result still gets an unused
    temporary (clang `GetUndefRValue`; WP-172);
  - an assignment operator call (`CXXOperatorCallExpr` with `=`, `+=`,
    ... and a member callee) evaluates its right operand before the
    object (clang `EvaluationOrder::ForceRightToLeft`): a trivial
    copy/move assignment emits the source lvalue, then the object, then
    the `memcpy` (nothing for an empty class); any other assignment
    operator emits its argument, then the object, then names the callee.
    `a.operator=(b)` keeps object-first order (WP-172;
    `tests/codegen/assign_rhs_first.cpp`).
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
      516, 3, 264) zero-extended; `__builtin_isfpclass(x, m)` is
      `llvm.is.fpclass` with the folded mask `m` (not emitted; the fold
      prepass collects it); `__builtin_expect_with_probability` at -O0 is
      its first operand (the expected value emitted, the probability
      not); signbit bitcasts to an integer of the
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
      wcslen, wmemchr, wmemcmp, strcmp, strncmp, strchr, strrchr, strpbrk,
      strstr, wcscmp, wcsncmp, wcschr (WP-140; `tests/codegen/builtins_str.cpp`),
      nan*/nans* of a non-literal, and the math
      functions exp/exp2/expm1/ldexp/pow/scalbn/scalbln with their
      `f`/`l` forms) call the function of the unprefixed name, declared
      with the builtin's type, `nounwind`, and `willreturn
      memory(read)` / `memory(none)` for `Pure` / `Const` builtins (on the
      call site too).
    - checked arithmetic (WP-270, clang `EmitBuiltinExpr`;
      `checked.rs`): the typed forms call `llvm.{s,u}{op}.with.overflow`
      at the parameter type and store the value at the result pointer's
      known alignment. The generic forms take each type's width (`bool`
      1) and signedness; mixed-sign `mul` whose wider operand is at least
      as wide as the result is clang's `EmitCheckedMixedSignMultiply`
      (absolute value of the signed operand, `umul`, range checks against
      `INT_MAX + IsNegative` or `UINT_MAX`, a negating `select`), and
      unsigned × unsigned into an equally wide signed result is `umul`
      plus `ugt INT_MAX` (the result pointer emitted last); otherwise the
      operands are `int_cast` to the encompassing width (signed if any of
      the three is; each unsigned type one bit wider then), the
      intrinsic's flag is extracted before its value, and a narrower
      result is truncated, extended back and compared (`icmp ne`, `or`).
      The value is stored through `EmitToMemory` (an `i1` widened),
      volatile per the pointee. A `select` whose arms are not both
      constants is not folded (IRBuilder `FoldSelect`). The calls are not
      fold candidates, since they store
      (`tests/codegen/builtins_overflow.cpp`);
    - `__builtin_alloca(n)` is `alloca i8, i64 n` in the current block
      (not the entry block, no stack save), aligned to
      `__BIGGEST_ALIGNMENT__` (16); `__builtin_alloca_with_align(n, a)`
      to `a / 8` (`tests/codegen/builtins_alloca.cpp`);
    - `__builtin_vsnprintf`, `__builtin_snprintf`, `__builtin_sprintf`
      are library builtins (`tests/codegen/builtins_printf_lib.cpp`).
  - a call of one of the standard functions clang treats as builtins
    (clang `isStdBuiltin`: `std::move`, `move_if_noexcept`, `forward`,
    `forward_like`, `as_const`, `addressof`, `__addressof`, declared in
    namespace `std` through inline namespaces, one non-variadic parameter;
    `CheckBuiltinFunctionCall`: the parameter a reference, the return a
    reference or pointer to the same type up to qualifiers) is the address
    of its argument's lvalue at every optimization level, so the function
    is not referenced and not emitted (`Cg::is_std_builtin`, `emit_call`;
    landed in WP-172, `tests/codegen/std_builtin_fold.cpp`). The algorithm
    `std::move(first, last, out)` and a `move` outside `std` are ordinary
    calls. Not done: clang's `-Wunused-value` "ignoring return value of
    function declared with const attribute" on a discarded call of one.
  - a source-location builtin (`SourceLocExpr`, spec/ast.md §5.8) is
    the value sema recorded for it: `__builtin_LINE`/`_COLUMN` an
    integer constant; `__builtin_FILE`/`_FILE_NAME`/`_FUNCTION` a
    pointer to the string literal's global; `__builtin_source_location()`
    a pointer to a private `unnamed_addr constant` named `.constant`
    holding the frozen `__impl` object (clang
    `GetAddrOfUnnamedGlobalConstantDecl`), one per frozen value
    (`ModuleGen::const_object`), its initializer, and so its strings,
    emitted first. Inside a `CXXDefaultArgExpr` or a
    `CXXDefaultInitExpr` (and nested ones) the outermost use's recorded
    values are taken
    (`enter_default_arg`), so each call site gets its own location.
  - a conversion to bool of a `zext` from `i1` is the `i1`, and the
    `zext` is erased when unused (clang `EmitIntToBoolConversion`);
  - the IR constant folder propagates NaNs as APFloat does: a NaN
    operand's payload (quieted), the left one first; an invalid
    operation of non-NaN operands gives the positive default NaN.
- Names (via `kiln_abi::mangle`): namespace-scope variables with internal
  linkage are mangled (`_ZL1h`); static locals use 0-based
  discriminators per (function, name): none, then `_0` ... `_9`, then
  `__10_`, `__11_`, ...
- Private unnamed constants (`.str`, `.constant`) are uniqued by
  `kiln_llvm`'s one module-wide counter, as LLVM's
  `ValueSymbolTable::makeUniqueName`: a clashing name gets `.N` with the
  next counter value (`.str`, `.str.1`, `.constant`, `.str.2`), so names
  follow emission order exactly as clang's do.
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
    preference. Constants are never promoted to globals at -O0. An
    aggregate is constant when it is constexpr, or when it is POD and
    its initializer folds. The exception is a trivial copy or move of
    an empty class, which reads nothing and so folds whatever its source
    is. Like clang's `isConstantInitializer`, it is constant only when
    its source names a constexpr variable. `E e2 = std::move(e)` emits
    nothing;
  - a non-constant aggregate goes through `EmitAggExpr`, with
    `CheckAggExprForMemSetUse` (size > 16 and nonzero*4 <= size gives a
    memset);
  - `EmitArrayInit`: element 0 is stored at the base, element i at
    `gep inbounds elem, base, i64 i`, and a phi filler loop covers
    elements without an initializer.
- Constants (`consts.rs`): a union constant is its active member followed
  by `[N x i8] undef` padding, typed as the union when the element types
  match its IR type, else as a literal struct. The active member is the
  one the initializer list names, else the one the Oven recorded in the
  frozen image (`ByteImage::unions`; matched by offset and by the
  member's context, so nested unions at one offset resolve), so a union
  made active by an assignment in a constructor body or by a trivial copy
  assignment (libc++'s `std::string` constructors) is emitted, as clang
  emits its `APValue` (WP-212). An image with no recorded member is
  `zeroinitializer` when all its bytes are zero, else an error. When a
  global's definition has a different IR type from the declaration an
  earlier use created (a union, or a record holding one, initialized
  through a member other than its largest), `emit_var_def` retypes that
  declaration to the initializer's type and moves it to the end of the
  global list (`kiln_llvm` `retype_global_to_end`), as clang
  `GetOrCreateLLVMGlobal` does; a static local is retyped in place
  (`AddInitializerToStaticVarDecl`). Retyping a global that already has
  an initializer is an error (WP-213). A record with bit-fields
  is built like any other, each bit-field storage unit read from the
  image's bytes as its IR type (the Oven's object representation already
  packs the fields in target bit order; WP-207). Base classes are still
  M3 errors in constants.

### 3.1 New- and delete-expressions (`new_delete.rs`, WP-061a)
clang's `EmitCXXNewExpr`, `EmitCXXNewAllocSize`,
`EmitNewArrayInitializer`, `EmitCXXDeleteExpr` and `EmitDeleteCall` at
`-O0`; constructor loops, destructors and array cookies per §7b/§7g.
- **Allocation functions.** `alloc_fn` classifies a global `operator
  new`/`new[]`/`delete`/`delete[]` as clang does
  (`isReplaceableGlobalAllocationFunction`: `size_t` or `void*` first,
  then optionally `align_val_t`/`size_t`/`const std::nothrow_t&`;
  `isReservedGlobalPlacementOperator`: `(size_t, void*)` and `(void*,
  void*)`). A replaceable one's declaration or definition gets
  `nobuiltin`, and `operator new` also `allocsize(0)` and the
  `nonnull`/`noalias` return attributes clang adds implicitly
  (`alloc_fn_attrs`, applied in `module.rs::get_fn` and
  `func.rs::emit_fn_def`). `alloc_fn_attrs` also gives any function
  with GNU `malloc` and a pointer result a `noalias` result (clang
  `RestrictAttr`) and turns GNU `alloc_size(n[, m])` into
  `allocsize(n-1[,m-1])` (the LLVM index counts `this`, like the
  one-based source index). `extern "C"` namespace-scope `malloc`,
  `calloc`, `realloc` and `aligned_alloc` with a pointer result are the
  library builtins clang gives an implicit `allocsize` (`(0)`, `(0,1)`,
  `(1)`, `(1)`; `aligned_alloc` also an implicit `alloc_align(1)`) (clang
  `AddKnownFunctionAttributes`).
- **Direct calls** (`alloc_call_attrs`, from `emit_call` and the
  non-virtual member call): a direct call of a replaceable `operator
  new` gets `allocsize(0)`, `noalias` and `nonnull` unless nothrow;
  other callees their `allocsize` and, with GNU `malloc`, `noalias`
  (clang `ConstructAttributeList`). An `alloc_align` argument
  (GNU `alloc_align(n)`, `aligned_alloc`'s, or the `std::align_val_t` of
  a replaceable `operator new`) that is a constant power of two gives
  the result `align N`. *Deviation:* for any other alignment clang emits
  `llvm.assume(i1 true) ["align"(ptr %r, i64 %a)]` after the call; kiln
  emits nothing (an optimization hint only).
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
- **Null check.** (clang: nothrow and (non-POD or `hasInitializer`);
  class default-init always has a `CXXConstructExpr` initializer in
  clang's AST, so a nothrow `new` of any class is checked.) A `noexcept`
  allocation function that is neither the
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
  default, fall-through, GNU case ranges §7l), while, do, for, break, continue, return,
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
- A goto (forward, backward, out of nested scopes, loops and try blocks)
  branches through the cleanups of every scope it leaves (`cleanup.rs`
  branch fixups). A local default-initialized by a trivial constructor
  still pushes its destructor cleanup whenever there is an insertion
  point (clang `EmitAutoVarCleanups`). Sema rejects jumps into a
  protected scope, so codegen never sees one (WP-153).
- `noreturn` (WP-153): a function some declaration of which carries
  `[[noreturn]]` / `__attribute__((noreturn))`, or whose type is
  `noreturn`, gets the `noreturn` function attribute on its declaration
  and definition. Its calls get the `noreturn` call-site attribute —
  except a `[[noreturn]]` (C++11 spelling) virtual function's, whose
  overriders may return — and are followed by `unreachable` and a dummy
  block (`noreturn_call`, `finish_noreturn_call`).

- `[[assume(e)]]` (WP-160): when there is an insertion point, each
  assumption that is not `ASSUME_IGNORED` is emitted as a scalar and
  passed to `llvm.assume`, in source order and before the (null)
  sub-statement (`emit_assumptions`; `tests/codegen/assume_attr.cpp`).
  Calls to `const`/`pure` functions carry their call-site attributes
  (§3 "Calls", WP-182).

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
- **Unit corpus** (`crates/kiln_codegen/tests/lower.rs`, WP-052): 90
  inline C++ programs, lowered for `aarch64-unknown-linux-gnu` straight
  from `parse_tu` (no driver). Every case must pass
  `opt-19 -S -passes=verify`. `Exact` cases must also equal
  `clang++-19 -S -emit-llvm -O0 -std=c++26 --target=aarch64-unknown-linux-gnu
  -moutline-atomics` after normalization (the flag for the same reason as
  in the IR-diff corpus below; kiln's default is on, WP-228). A case that differs but is marked `VerifyOnly` is
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
  same normalization. On aarch64 both command lines also get
  `-moutline-atomics` (WP-226): clang 19 enables that feature by default
  only when it finds a GCC >= 9.3.1 installation for the target, which a
  native AArch64 host has and an x86-64 host cross-targeting aarch64 (the
  GitHub runner) does not, so the flag keeps the comparison independent of
  the host. A `//@ args: <opts>` line in a case appends
  `<opts>` to both command lines after it (WP-165). A `(case, triple)` pair in its `DEVIATIONS` table
  must only verify; every other pair must be identical. `KILN_CASE=<stem>`
  prints both texts; without `opt-19`/`clang++-19` it is skipped. The
  corpus is 226 programs (ABI classification of 1- to 32-byte structs,
  HFAs, byval, varargs, `__int128`, `long double`, bit-fields, static
  locals, string literals, references, control flow, new/delete, and the
  WP-088 `builtins_{bits,mem,fp,misc,srcloc}` cases, the WP-079
  `class_*` cases of §7, the WP-137 `consteval_oversized` case, and later
  work packages' cases): 449/452 pairs exact; `DEVIATIONS` holds
  `global_init_array_eh`, `eh_array_destroy_throwing_global` and
  `array_temporary_global` on aarch64 (§7f, §7m).
- **`run` tests:** 35 printf-based programs (`extern "C" int printf`, no
  headers) under `tests/conformance/<section>/`, each naming its section,
  are compiled, linked and run by kiln and by clang, and their stdout and
  exit status compared. The 5 header-dependent `run` tests
  (`basic.start.main` argv/environment/return-value/stdin and
  `stmt.ranged/constexpr-sum`) are `xfail(kiln)`: libstdc++'s
  `c++config.h` uses `decltype`, which kiln_parse does not support yet.

### Deviations
- **Uses of constant-initialized reference variables as lvalues.** A
  read through a reference to a `const` referee folds (above), but clang
  also emits a non-odr-use of a reference usable in constant expressions
  as a direct use of its referee's address (`EmitDeclRefLValue` for
  `NOUR_Constant`: `e.y`, `h[0]` with a non-constant index); kiln loads
  the reference's slot (a dead `load ptr @e`). Sema does not yet mark a
  reference bound to a static class or array temporary as a constant
  non-odr-use (`is_static_lvalue` is syntactic). No corpus case covers it.
- **Namespace-scope and static reference variables** are emitted as
  clang does (a constant initializer when the referee is a constant
  address, else dynamic initialization, §7e); only their uses differ, as
  above.
- **AArch64 loop canonicalization of initialization functions.**
  clang 19's AArch64 `-O0` pipeline runs `loop-idiom-vectorize`, whose
  LoopSimplify/LCSSA prerequisites rewrite loops in every function
  without `optnone` (the `__cxx_global_var_init` functions): array
  construction and EH-cleanup loops gain `.preheader`/`.loopexit` blocks
  and LCSSA `phi`s. kiln emits the plain loops. `ir_vs_clang` lists
  `global_init_array_eh` on aarch64 as a known deviation; x86_64 is exact.
- **GNU array range designators with side effects** (`int a[3] =
  {[0 ... 2] = f()}`, WP-193): sema gives each element of the range its
  own copy of the clause, so the clause is evaluated once per element.
  clang refuses it ("cannot compile this GNU array range designator
  extension yet", `sawArrayRangeDesignator`); GCC evaluates it once.
  Side-effect-free ranges match clang's IR
  (`tests/codegen/union_nsdmi_and_array_ranges.cpp`).
- **Reference members** (`struct S { int& r; }`) are diagnosed (`a
  reference member is not supported yet`).
- **`dereferenceable(N)` of a reference to a class** uses the class's
  non-virtual size (`nv_size`, at least 1). clang's
  `getMinimumObjectSize` uses the full size for a `final` class; with no
  virtual bases in M2 the two agree.
- **Call results stored through a coercion** (`CreateCoercedStore`) use
  the object size, where clang uses the type's data size when the slot
  may overlap (`AggValueSlot::getPreferredSize`). A base subobject
  initialized from a call (WP-158) goes through a temporary and copies
  the data size, as clang; an aggregate's base element is initialized by
  the base-object (C2) constructor variant (sema-class.md §5.6), and a
  zeroing construction of a base subobject memsets its non-virtual size
  through an `inbounds i8` GEP at offset 0 (clang
  `EmitNullBaseClassInitialization`). An immediate invocation
  (a valued `ConstantExpr`, WP-137) does use the data size of a
  `MayOverlap` slot (a materialized temporary): an oversized constant is
  stored to a temporary of its own type and the data size copied out,
  as clang 19.
- **Record constants** follow `ConstantAggregateBuilder::buildFrom`: the
  fields at their offsets with `undef` padding for gaps, unpacked when
  the natural layout fits the type's size (tail-padded with `undef`),
  packed otherwise; the named type only when layout-identical. A class
  that is not POD for layout (packed IR type with tail padding) is
  therefore a literal `{ i32, i8 }`, as clang.
- Sema gaps (kiln_sema) that keep `run` tests and corpus cases simple:
  casts to reference types (`static_cast<int&&>`), binding
  `const T(&)[N]` to a `T[N]` argument, copy-initializing a class object
  from an lvalue (so a function cannot return a named local or a
  parameter of class type), `S{...}` temporaries, and binding a
  reference to a temporary. In kiln_codegen itself, a local aggregate
  whose folded initializer the constant lowering cannot spell is
  initialized as written (`emit_agg_into`, WP-201).
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
  `bool` constant and `fold.rs` does not look inside it. A
  `ConceptSpecializationExpr` or `RequiresExpr` likewise lowers to the
  satisfaction Sema recorded (`i1` constant; an unrecorded one is an
  error), and `fold.rs` skips its operands; WP-136.)

## 7. Landed (WP-079): classes
Lowering of non-polymorphic classes (`class.rs`, `cleanup.rs`,
`memptr.rs`), mirroring clang 19 `-O0` on both targets. Polymorphic
classes and virtual bases are WP-080: every path that needs a vtable
pointer, a VTT or a virtual call reports a `CodegenError` naming WP-080.
- **Structors** (clang `ItaniumCXXABI::emitCXXStructor`): constructors
  and destructors are emitted as the base variant (C2/D2) and then the
  complete variant (C1/D1). A discardable C1 is replaced by C2 (RAUW);
  otherwise C1 is an `alias` to C2. clang's `applyReplacements` moves
  C2 to where the C1 declaration was created: when C2 already exists at
  the first C1 reference, `structor_ref` moves it to the end of the
  function list (`Module::move_function_to_end`, WP-113). Deleting destructors (D0) are WP-080.
  Structor references go through `structor_ref`. A cache hit on the base
  variant still queues it for definition, because clang's `EmitDeferred`
  is depth-first.
- **Constructor bodies** (`EmitConstructorBody`): base initializers at an
  `i8` GEP with the `min_align` alignment and `getOverlapForBaseInit`,
  then member initializers in declaration order. Delegating constructors
  are supported. Defaulted copy and move constructors use clang's
  `ConstructorMemcpyizer`, which merges runs of trivially copyable fields
  into one `llvm.memcpy`. A field that is empty for layout
  (`isEmptyFieldForLayout`, e.g. an empty `[[no_unique_address]]` member
  sharing offset 0 with a non-empty one) joins the run but copies no
  bytes. The memcpy starts at the lowest-offset non-empty field and ends
  at the end of the highest-offset one, and a run of only empty fields
  emits nothing.
- **Destructor bodies and cleanups** (`EmitDestructorBody`, `cleanup.rs`):
  a cleanup stack modeled on clang's `EHScopeStack` normal cleanups, with
  branch fixups, the `cleanup.dest.slot` switch, and the single-exit
  rewrite. Destructors run in this order: member destructors in reverse
  declaration order, then base destructors.
  - `ExprWithCleanups` opens a full-expression scope.
  - `CXXBindTemporaryExpr` and `MaterializeTemporaryExpr` push
    destructor cleanups. Lifetime-extended temporaries move to the
    enclosing automatic scope.
  - Locals of class type get destructor cleanups. NRVO uses an `nrvo`
    flag, as clang does.
- **Arrays of classes** (`EmitCXXAggrConstructorCall`): an
  `arrayctor.loop` with a `phi`, one element at a time, and an
  `arraydestroy` loop on cleanup. Variable-length arrays of classes
  report an error.
- **Aggregate initialization with bases** (C++17): base initializers come
  first in the init list. Each one is emitted into its base subobject.
- **Member calls** (`EmitCXXMemberCallExpr`): non-virtual calls through
  `.` or `->`. A trivial copy or move assignment becomes a `memcpy` of
  the data size. A static member called through an object evaluates the
  object for side effects.
- **Static data members:**
  - An in-class non-inline declaration is an `external global`.
  - An inline member is defined in its class.
  - An out-of-line definition takes the in-class initializer
    (`getAnyInitializer`).
  - A const member with an in-class initializer, odr-used but never
    defined out of line, is an `available_externally constant`.
  - Naming a static member through `.` evaluates the object and is
    ignored.
- **Member-expression folds** (`ScalarExprEmitter::VisitMemberExpr`): an
  integral or enumeration `MemberExpr` that the Oven folds becomes a
  constant, after the object is evaluated for side effects. A `const`
  non-volatile static data member named this way (`this->b.w`) folds to
  its variable's constant initializer even when the object does not fold
  (clang `tryToConvertMemberExprToDeclRefExpr`): the read is not an
  odr-use ([basic.def.odr]/5) and needs no out-of-line definition.
- **Pointers to members** (`memptr.rs`; Itanium §2.3, the ARM variant on
  AArch64; see spec/abi.md §8.6):
  - `&C::m` constants and Oven `Scalar::MemberPtr` constants, with
    clang's `getMemberPointerPathAdjustment`;
  - null member pointers;
  - base-to-derived and derived-to-base conversions (the
    `-1`-preserving `select` for data members; `adj` arithmetic for
    functions, folded on constants);
  - `.*` and `->*` lvalues, aligned with `getDynamicOffsetAlignment`;
  - calls through member function pointers, with the virtual/non-virtual
    dispatch diamond (`!nosanitize !{}` on the vtable slot GEP and
    load);
  - `==` and `!=` (`EmitMemberPointerComparison`);
  - conversion to `bool`.

  Pointers to virtual member functions are WP-080.
- **Tests:**
  - IR-diff corpus cases `class_structors`, `class_temporaries`,
    `class_copy`, `class_members`, `class_aggregate_bases`,
    `class_static_members` and `class_member_pointers`. All are exact on
    both triples, which makes the corpus 64 programs.
  - The `xfail(kiln)` markers that named WP-079 are gone:
    `class.friend/friends`, `class.mem/member-functions`,
    `class.static.data/static-members`, `class.derived/derived-to-base`,
    `expr.mptr.oper/ptr-to-member` and `conv.mem/base-to-derived`.

**Gaps (owned elsewhere or open):**
- No EH: there are no `invoke`s, landing pads or terminate scopes.
  Corpus cases call `noexcept` functions, so clang emits no EH either.
  A throwing call inside a cleanup scope is emitted as a plain `call`.
- WP-080 owns virtual functions, virtual bases, D0, and pointers to
  virtual member functions.
- A C1 that is not discardable is an alias. clang uses a COMDAT or a
  `weak_odr` C1 in some linkage combinations, which kiln has not
  covered yet.
- A `CXXDefaultInitExpr` is emitted with the enclosing `this`.
- Sema does not yet synthesize calls to non-trivial `operator=` for
  classes, so codegen sees only trivial assignments.
- Class `new[]`/`delete[]` with array cookies: see §7g (WP-118).
- The comment in `templates/builtins/builtin.bit-cast-run.cpp` defers
  class-template member calls. That test has no `xfail` and passes.

## 7a. Landed (WP-080): virtual functions
Lowering of polymorphic classes (`vtable.rs`, `dyncast.rs`, plus hooks in
`class.rs`, `func.rs`, `expr.rs`, `memptr.rs`, `new_delete.rs`), mirroring
clang 19 `-O0` on both targets. Layouts come from `kiln_abi::vtable`.
- **Vtables** (clang `CGVTables`, `ItaniumCXXABI::emitVTableDefinitions`):
  a vtable global is created on first use (vptr store, slot load) and
  defined at the end of the TU. Linkage follows the key function: a
  class whose key function is defined in this TU gets a strong
  `external` vtable; one whose key function is defined elsewhere gets an
  `external` declaration only; a class with no key function (or an
  inline one) gets a `linkonce_odr` `unnamed_addr` definition in its own
  COMDAT, as clang on ELF. Template patterns, dependent classes and
  classes instantiated from a template (any kind but an explicit
  specialization) have no key function (Itanium 5.2.6, clang
  `computeKeyFunction`). A class under an explicit instantiation
  declaration (`extern template`, itself or as a member class) gets an
  `external` vtable declaration (WP-158).
- **Explicit instantiation declarations** ([temp.explicit]/14, WP-158;
  `Cg::is_explicit_inst_declaration`): a function or variable that is
  an `extern template` specialization, or a member of a class
  specialization under one (the innermost explicitly instantiated
  enclosing class decides), gets no definition at `-O0`, even when inline
  (clang would make it `available_externally` only when optimizing);
  references name the external symbol, structor calls the complete
  variant (D1/C1). Implicitly declared special members are not
  instantiations and are still emitted.
- **Explicit instantiation definitions** ([temp.explicit], WP-170;
  `Cg::is_explicit_inst_definition`, clang `GetGVALinkageForFunction` ->
  `GVA_StrongODR`): a function or variable that is an explicit
  instantiation definition, or a member of a class under one (innermost
  explicitly instantiated enclosing class decides, as above), is
  `weak_odr dso_local` in its own COMDAT, inline or not, and is never
  deferred. Naming a class (`template struct X<int>;`) emits every
  non-template member, used or not, recursively through member classes,
  including out-of-line and inline static data members
  ([temp.explicit]/10; `emit_class_members`), plus a `weak_odr` vtable
  and RTTI (`VtLinkage::Weak`, emitted after the strong key-function
  vtables). A member class that was already defined when the enclosing
  class was explicitly instantiated keeps an implicit vtable (emitted
  `linkonce_odr` only if used; `Cg::vtable_explicit_inst_kind`), though
  its member functions are still `weak_odr` (clang
  `InstantiateClassMembers`). A member explicit instantiation (`template void X<int>::f();`,
  `template X<int>::X();`) is emitted right after the declaration that
  precedes it lexically (sema records it in
  `TemplateTables::explicit_member_insts`). Base-object structors of a
  class without virtual bases share a `C5`/`D5` COMDAT (with `D0`) and
  the complete-object symbol is a `weak_odr` alias (clang
  `getCodegenToUse` -> `COMDAT`). Static locals of such functions stay
  `linkonce_odr`. Linkage table, emission order and the remaining cases
  (WP-113): §7h.
  The initializer is
  `{ [N x ptr] ... }`, one array per primary/secondary group, with
  offset-to-top, RTTI, then the slots. Pure slots are
  `__cxa_pure_virtual`, deleted slots `__cxa_deleted_virtual`.
- **RTTI** (`ItaniumRTTIBuilder`): `_ZTI`/`_ZTS` for classes
  (`__class_type_info`, `__si_class_type_info`, `__vmi_class_type_info`
  with flags and base offset words), pointers (`__pointer_type_info`,
  qualifier flags plus 8 for an incomplete pointee), enums, functions and
  arrays (`{vt, name}`). Fundamental types and (const) pointers to them
  refer to the runtime's `external constant` objects. RTTI of a type that
  involves an incomplete class has `internal` linkage. Linkage of class
  RTTI follows the vtable.
- **vptr stores**: constructors store the address point of every vptr
  after the base initializers and before the member initializers;
  destructors re-store them on entry unless clang's
  `CanSkipVTablePointerInitialization` holds (trivial body, no
  members needing destruction).
- **Virtual calls** (`CGCXXABI::getVirtualFunctionPointer`): the
  arguments are emitted first, then the vtable load (`!tbaa` free at
  `-O0`), the slot GEP and load. Calls are devirtualized when the class
  or the method is `final`, when the object is a known complete object
  (a named non-reference variable or a temporary), and for qualified
  calls (`p->B::f()`).
- **Deleting destructors** (D0): the body calls D1 then the sized
  `operator delete` (or the class's own), the deallocation as a
  normal-and-EH `Cleanup::DtorDelete` around the D1 call (clang
  `EnterDtorCleanups` / `CallDtorDelete`, WP-219), so it also runs when
  a `noexcept(false)` destructor throws (`eh_deleting_dtor`,
  `expr.delete/run-deleting-dtor-throws`). An abstract class's D0 is
  `llvm.trap` + `unreachable`. `delete p` of a polymorphic class with a
  virtual destructor calls D0 through the vtable; otherwise D1 then
  `operator delete`. `::delete p` of such a class: see WP-108 below.
- **Thunks** (`CodeGenVTables::maybeEmitThunk`): a thunk is declared
  when the vtable that needs it is built and defined right after its
  target (method, D1 or D0) is emitted. `this`-adjusting (`_ZThn`) and
  return-adjusting (`_ZTch`, with a null check and phi for pointers)
  thunks spill and reload their parameters like any function, adjust
  `this`, and forward the arguments with a `tail call` (not `musttail`).
  Attributes follow clang's IsThunk rules: `this` keeps only `noundef`,
  and a reference return keeps only `noundef`.
- **`dynamic_cast`** (`EmitDynamicCast`): pointer sources are
  null-checked (blocks `dynamic_cast.notnull`/`.null`/`.end`, then a
  phi). The runtime call is `__dynamic_cast(p, srcRTTI, dstRTTI, hint)`
  with clang's `computeOffsetHint` (-1 virtual path, -2 not a public
  base, -3 multiple public paths, else the offset). A reference cast
  checks for null and calls `__cxa_bad_cast` (`noreturn`). `dynamic_cast
  <void*>` uses the offset-to-top word. Casts known to fail (final
  source, or final unrelated destination) fold to null / a bad_cast call.
- **`typeid`**: a type operand, or an expression that is not potentially
  evaluated or names a most-derived object, yields the static `_ZTI`.
  Otherwise the type_info pointer is loaded from vtable slot -1; a glvalue
  that comes from a pointer dereference is first null-checked with a
  call to `__cxa_bad_typeid`.
- **Pointers to virtual members**: `&C::vf` is `{index * 8 + 1, adj}` on
  x86-64 and `{index * 8, 2 * adj + 1}` on AArch64; calls through them
  take the virtual branch already present in WP-079's `memptr.rs`.
- **Template linkage policy**: vtables, RTTI and thunks of classes that
  are template specializations, or have no key function, are
  `linkonce_odr` and emitted only when used (deferred), as with inline
  functions. Explicit instantiation definitions are `weak_odr` (see
  above).

- **Virtual bases** (clang `GetAddressOfBaseClass`,
  `ItaniumCXXABI::{buildStructorSignature, getImplicitConstructorArgs,
  EmitDestructorCall}`, `CodeGenVTables::{EmitVTTDefinition,
  GenerateConstructionVTable}`):
  - *Conversions* start at the last virtual step of the cast's path
    (clang `BuildBasePathArray`); the non-virtual offset after it is
    static. The vbase offset is loaded from the vtable (non-`inbounds`
    `gep i8` by the vbase-offset offset, `load i64 align 8`), the
    non-virtual offset added, then one `gep inbounds i8`. A pointer
    operand is null-checked (branch + `phi`); `this` and lvalues are
    not. A `final` derived class uses its static vbase offset. The
    result alignment is clang's `getVBaseAlignment`.
  - *Structors* of a class with virtual bases are separate C1/C2 (D1/D2)
    functions, never aliased or replaced. The base-object variant takes
    `ptr noundef %vtt` after `this`; the prolog spills and reloads it
    after `this`. C1 constructs the virtual bases first (at their static
    offsets, `MayOverlap`), then everything C2 does; C2 skips them (and
    so does C1 of an abstract class). D1 calls D2 with the class's own
    `@_ZTT` and then destroys the virtual bases in reverse `vbases()`
    order with their D2; D1 of an abstract class is `llvm.trap`.
  - *VTT argument* (clang `GetVTTParameter`): in a function with a VTT
    parameter, `gep inbounds ptr, %vtt, i64 idx` (the parameter itself
    when delegating or idx = 0); otherwise the constant
    `getelementptr inbounds ([N x ptr], @_ZTT, i64 0, i64 idx)`, which
    folds to `@_ZTT` for idx = 0. `idx` is the sub-VTT index of the base
    subobject (the first VTT component of its construction vtable).
  - *Vptr stores in a base-object structor* load each address point from
    the VTT (secondary-vptr index; a non-zero index is a `gep inbounds
    ptr`); a vptr inside a virtual base is stored at an address computed
    with the vbase offset from the already-stored vtable.
  - *VTT and construction vtables* (`_ZTT<class>`, `_ZTC<class><off>_
    <base>`): `[N x ptr]` and vtable-group constants, `unnamed_addr`,
    `align 8`, with the vtable's linkage and COMDAT, emitted before the
    vtable's own initializer (clang `GenerateClassData` order;
    `GetAddrOfVTT` creates `_ZTV` first). VTT entries are address points
    with `inrange`; an address point at the end of a vtable (no virtual
    functions) belongs to that vtable, not the next one.
  - Thunks with a virtual adjustment (`_ZTv0_n<off>_`) load the
    vcall offset from the vtable.

**Landed (WP-108):**
- *Member-pointer RTTI* (clang `BuildPointerToMemberTypeInfo`):
  `{ vt, name, i32 flags, pointee, class }` with the
  `__pointer_to_member_type_info` vtable; flags are the pointee's
  qualifiers, 8 for an incomplete pointee class, 16
  (`PTI_ContainingClassIncomplete`) for an incomplete class, 64 for a
  noexcept function pointee. Linkage is `internal` when the class or the
  pointee involves an incomplete class or an internal-linkage class.
- *noexcept function types* (`extractPBaseFlags`): a pointer to (or
  member pointer to) a noexcept function type has flag 64 and refers to
  the `type_info` of the function type *without* `noexcept`
  (`_ZTIPDoFviE` -> `_ZTIFviE`); `typeid` of the function type itself
  is its own `_ZTIDoF...E` `__function_type_info`.
- *`::delete p` of a polymorphic class* (`emitVirtualObjectDelete`,
  `UseGlobalDelete`): after the null check, the complete object is
  `p + vtable[-2]` (offset-to-top), the complete destructor (D1) is
  called through the vtable, then the global `operator delete` with the
  complete-object pointer and the static type's size (even when the
  class declares its own `operator delete`). A final class is
  devirtualized as before.
- *Sema guarantee for `base_path_offset`*: a member-pointer conversion
  whose base path has a virtual step is rejected in every form (implicit,
  `static_cast` / C-style cast in either direction) with clang's
  `conversion from pointer to member of class 'D' to pointer to member
  of class 'B' via virtual base 'V' is not allowed`, so none reaches
  codegen (conformance `expr.static.cast/member-pointer-virtual-base`).
- Corpus: `virtual_rtti_memptr`, `virtual_global_delete`,
  `virtual_base_dyncast_hint` (the `-1` `__dynamic_cast` hint).

**Gaps (owned elsewhere or open):**
- Inheriting constructors (WP-114): `CI1`/`CI2` structors mangled with
  the base as a `<name>` (not a substitution candidate, clang
  `mangleCXXCtorType`), defined from the `CXXInheritedCtorInitExpr`
  ctor-initializer, which forwards the inheriting constructor's own
  parameters. When the class has virtual bases and the inherited
  constructor comes through one, the base variant takes no parameters
  beyond `this`/VTT (clang `inheritingCtorHasParams`), yet a call still
  passes the evaluated arguments, as clang does. Corpus
  `class_inheriting_ctors`, mangle `inheriting_ctors`. An inheriting
  constructor of a C-variadic base constructor cannot forward its
  variadic arguments, so, as clang does (`canEmitDelegateCallArgs` →
  `EmitInlinedInheritingCXXConstructorCall`), every call is inlined at
  the call site (`emit_inlined_inheriting_ctor`): `this` spilled and
  reloaded, then the ctor-initializers emitted with the call's evaluated
  arguments (`Fx::inh_args`) forwarded by the `CXXInheritedCtorInitExpr`,
  recursively when that base constructor is itself such an inheriting
  constructor; no `CI` symbol is emitted. Gap: inlining the base variant
  of a class with virtual bases (it needs the caller's VTT) is a
  "not supported yet" error. An inherited constructor template
  specialization is mangled `CI1`/`CI2` <base> <template-args> with the
  pattern's signature (clang `InheritedTemplateArgs`), and a constructor
  template specialization's base variant is `C2` (WP-146; corpus
  `class_inheriting_ctors_variadic`).
- (Resolved by WP-100: `__cxa_bad_cast` / `__cxa_bad_typeid` are
  invoked inside EH scopes, §7b.)
- RTTI of a pointer to member function with method qualifiers
  (`void (S::*)() const`, an abominable pointee) is not covered.

## 7b. Landed (WP-100): exceptions
Itanium EH as clang 19 emits it at -O0 (CGException.cpp, CGCleanup.cpp,
CGClass.cpp, CGExprCXX.cpp, CGExprAgg.cpp). `CodegenOptions::exceptions`
(`-fexceptions`, default on; `-fno-exceptions` turns every EH cleanup
off and sema rejects `try`/`throw`).

- **EH scope stack** (`cleanup.rs`, `eh.rs`): cleanups, catch scopes and
  terminate scopes share one stack (`EHScopeStack`), with clang's
  innermost-EH-scope links, `CurrentCleanupScopeDepth`, cached EH
  dispatch blocks and `IsUsedAsEHCleanup`. A call is an `invoke` when
  `getInvokeDest` finds an EH scope (`emit_call_op`,
  `runtime_call_or_invoke`, `noreturn_runtime_call_or_invoke`);
  nounwind callees stay `call`s. Landing pads (`EmitLandingPad`) take
  `cleanup` / `catch ptr @_ZTI…` / `catch ptr null` clauses in clang's
  order and spill to `exn.slot` / `ehselector.slot`; `eh.resume` loads
  them back into `resume`. The personality is `__gxx_personality_v0`,
  attached once the function has a landing pad. `FinishFunction` emits
  the EH blocks (resume, terminate landing pad, terminate handler)
  before the unreachable block; `TryMarkNoThrow` adds `nounwind` to a
  function without `invoke`s or `resume`.
- **Terminate scopes:** the body of a non-throwing function
  ([except.spec]/5) and EH cleanups that may throw are inside a
  terminate scope: its landing pad is `catch ptr null` and calls
  `__clang_call_terminate` (the linkonce_odr hidden helper calling
  `__cxa_begin_catch` then `std::terminate`).
- **throw** ([expr.throw]): `__cxa_allocate_exception(size)`, the
  operand initialized in place under a `FreeException` EH cleanup
  (deactivated after it), then `__cxa_throw(exn, @_ZTI…, dtor or null)`
  (invoked inside an EH scope) and `unreachable`; the insertion point
  stays on the terminated block (clang `EmitBlock` placement). A
  rethrow is `__cxa_rethrow`.
- **try/catch** (`try_catch.rs`): `enter_try` pushes a catch scope with
  one handler per `catch` (type-info, or catch-all); `exit_try` emits the
  dispatch chain (`llvm.eh.typeid.for` compares against the selector),
  each handler (`__cxa_begin_catch`, `InitCatchParam` for references,
  pointers, scalars, trivially and non-trivially copied classes, an
  `EndCatch` cleanup that calls `__cxa_end_catch`, invoked when the
  caught class's destructor may throw), and the `try.cont` block.
  Handler blocks are placed after their uses, as clang's.
- **Function-try-blocks** ([except.handle]/14-15): the try is entered
  before the constructor's member initializers / the body scope;
  falling off a constructor or destructor handler emits `__cxa_rethrow`;
  a destructor's try covers the member and base destruction (the
  complete destructor then emits base + virtual base cleanups itself,
  and an abstract class's complete destructor is a trap).
- **Partially constructed objects** ([except.ctor]/3-4): EH cleanups for
  constructed bases (`CallBaseDtor`), members (`DestroyField`), virtual
  bases, the target of a delegating constructor (`CallDtorDelegating`),
  the elements of an array being constructed
  (`RegularPartialArrayDestroy`), the elements of an array initialized
  from a braced list (`IrregularPartialArrayDestroy`, reading
  `arrayinit.endOfInit`; only with exceptions on, as clang's
  `needsEHCleanup`), and each base and member of a braced aggregate
  initialization (`pushDestroyAndDeferDeactivation`; all of these are
  deactivated, innermost first, once the object is complete, using a
  placeholder for clang's dominating instruction). An element slot of
  such a list is `IsDestructed` (no temporary cleanup for a
  `CXXBindTemporaryExpr` written in place).
- **new-expressions** ([expr.new]/28): `CallDeleteDuringNew` as an EH
  cleanup around the initializer, with the pointer, size and placement
  arguments saved to `cond-cleanup.save` slots inside a conditional
  branch; class `operator new` keeps its declared result type.
  Array new runs the constructor loop for a non-constant count
  (`arrayctor.cont` isempty check, as clang's
  `EmitCXXAggrConstructorCall`).
- **Destructor variants:** D1/D2 destroy members and bases through
  normal-and-EH cleanups, so a throwing destructor body still destroys
  them; D1 of a class with virtual bases adds the virtual base cleanups.
- **RTTI under EH:** `__cxa_bad_cast` / `__cxa_bad_typeid` are invoked
  inside an EH scope; the insertion point stays on the terminated block.
- **Oven:** a `CXXTryStmt` evaluates its compound statement only (a
  throw is not a core constant expression, so no handler is entered).
- **Interface change (flagged):** none in `kiln_codegen`'s public API;
  `kiln_ast` gained the try/catch builders and accessors (spec/ast.md
  §6) and `opaque_value_of`; `kiln_sema` gained `act_on_exception_decl`,
  `act_on_catch`, `act_on_try`, `push_try_scope` and
  `Scope::try_barrier` (spec/sema.md §7.5).
- **Behavior changes outside EH (flagged):** sema brace elision only
  into aggregates (spec/sema.md §7.5); `init_to_lvalue` marks aggregate
  element slots destructed, which fixed a double destruction of array
  elements initialized from bound temporaries.
- **Corpus:** `eh_throw`, `eh_throw_cleanups`, `eh_try_catch`,
  `eh_function_try`, `eh_partial_construction`, `eh_noexcept`, `eh_new`,
  `eh_new_array`, `eh_rtti`, `eh_aggregate_init`, exact on both triples.
  `run` tests: `except.handle/run-throw-catch`,
  `except.ctor/run-unwind-destroy`, `expr.new/run-delete-on-throw`.

(The D0 `CallDtorDelete` cleanup, left here by WP-100, landed with
WP-219; see "Deleting destructors" above.)

(Lifetime-extended temporaries, conditional temporaries at a computed
address, the delegating constructor's VTT and conditional `new` with a
class placement argument landed with WP-205, §7l.)

## 7c. Landed (WP-140): GNU inline asm (`asm.rs`)
`emit_asm` is clang's `CodeGenFunction::EmitAsmStmt`: outputs become
`=`+simplified constraint (register) or `=*` with an `elementtype` (an
indirect memory output); a register aggregate of size 1/2/4/8/16 is
passed as `iN`; a `+` output adds its input half (tied by number when
the constraint allows a register); a tied input narrower than its output
is zero/fp-extended, and a wider one widens the result; inputs are
prvalues, `i`/`n` constants, or `*m` pointers with `elementtype`. The
template is rewritten to LLVM syntax (`${N:mod}`, `$$`, `${:uid}`).
Clobbers: `memory` drops `memory(read)`/`memory(none)`, `cc` is kept,
others become `~{reg}` (normalized names); x86_64 appends
`~{dirflag},~{fpsr},~{flags}`. The call is `call ... asm sideeffect` for
volatile or output-less asm, is `nounwind`, and carries `!srcloc`.
Multiple outputs are a struct return taken apart with `extractvalue`
and stored with the operand's lvalue store (truncating back for
widened results). A `FileScopeAsmDecl` becomes `module asm` lines.
Validation: `tests/codegen/asm_stmt.cpp` matches clang 19 exactly on
aarch64 and x86_64. Not implemented: `asm goto` (callbr), the `unwind`
clobber inside a `try` (no `invoke`), `=@cc` outputs, register-variable
operands, MMX type adjustment.

**Landed (WP-140): `PredefinedExpr`.** The name is a private
`unnamed_addr constant` array named `<spelling>.<mangled function>`
(`__func__._Z1fv`), as clang names it (`Module::string_global_named`);
in a constant initializer outside a function it is an ordinary `.str`
literal. `tests/codegen/predefined.cpp`.

## 7d. Landed (WP-154): atomics (`atomic.rs`)
clang 19 `CGAtomic` at -O0, mirrored:
- **`AtomicExpr`** (`emit_atomic_expr`): the pointer and its alignment
  (the pointee's, or the operand's before an implicit cast for the generic
  forms); value operands are stored to `iN` temporaries of the atomic
  width (`atomic.tmp`/`.atomictmp`) and reloaded. A constant memory order
  picks one LLVM ordering (an order invalid for the op emits nothing, as
  clang); a non-constant one is a `switch` over the C ABI values
  (`monotonic`, `acquire`, `release`, `acqrel`, `seqcst` blocks, default
  monotonic). `cmpxchg` maps the failure order the same way (a nested
  switch when not constant; `weak` is a switch when not constant). Ops:
  `load atomic`, `store atomic`, `atomicrmw` (`xchg`, `add`, `sub`, `and`,
  `or`, `xor`, `nand`, `min`/`max`/`umin`/`umax`, `fadd`/`fsub`/`fmin`/
  `fmax` for floating types), the `*_fetch` forms recomputing the new
  value, and `cmpxchg` with the old value stored back on failure. The
  `__c11_*` forms use the `_Atomic` type's size; floating values travel
  through same-width integers except for `atomicrmw` float ops.
- **Builtins `CGBuiltin` lowers** (`emit_atomic_builtin`):
  `__atomic_test_and_set` (`atomicrmw xchg i8`, then `!= 0`),
  `__atomic_clear`, `__atomic_thread_fence`/`__c11_atomic_thread_fence`
  (`fence`), `__atomic_signal_fence`/`__c11_atomic_signal_fence`
  (`fence syncscope("singlethread")`), each with the order switch when
  not constant; the lock-free queries are folded by the Oven, and a
  non-constant `__atomic_is_lock_free` calls the libatomic function.
- **`_Atomic` lvalues** (`atomic_load_lv`/`atomic_store_lv`): reads are
  `load atomic seq_cst`, assignment is `store atomic seq_cst`;
  initialization is a plain store. `++`/`--` are `atomicrmw add/sub
  seq_cst` (pointer steps scaled by the pointee size; floats are
  `atomicrmw fadd/fsub`, or a `cmpxchg` loop for x87 `long double`), compound assignment is `atomicrmw` for `+ - & | ^` on
  integers and a `cmpxchg` loop (`atomic_cont`/`atomic_exit`) otherwise.
  On x86_64 a 16-byte `_Atomic` lvalue load/store is the
  `__atomic_load`/`__atomic_store` libcall with order 5, as clang.
- **Library calls (WP-165).** An `AtomicExpr` whose size is not a power
  of two or exceeds 16 bytes is the generic libatomic call
  (`__atomic_load(size, obj, ret, order)`, `__atomic_store(size, obj,
  val, order)`, `__atomic_exchange(size, obj, val, ret, order)`,
  `__atomic_compare_exchange(size, obj, expected, desired, success,
  failure)` returning `i1`, `noundef` and on x86_64 `zeroext`); every
  argument `noundef`, the declaration `nounwind willreturn`, orders passed
  as given (no switch). A `cmpxchg` loop on an `_Atomic` lvalue that needs
  a libcall (x86_64 16-byte x87 `long double` and `__int128`) materializes
  expected and desired in `_Atomic` temporaries (zeroed first when the
  value does not fill them) and reads the old value back from the
  expected buffer.
- **Padded and aggregate `_Atomic` (WP-165).** `_Atomic(T)` of at most 16
  bytes is laid out as `{ T, [n x i8] }` rounded to a power of two
  (`size_align`); operands whose memory size (x87: 80 bits) differs from
  the atomic width are copied into an `_Atomic`-typed temporary first
  (`convertToAtomicIntPointer`). Initialization is `EmitAtomicInit`
  (`atomic_init`, used for locals, `init_to_lvalue` and
  `__c11_atomic_init`): the object is zeroed when padded or not filled by
  its value, and the value goes into field 0. Aggregate `_Atomic` reads
  load the atomic integer straight into the slot (or `__atomic_load`
  into it); `NonAtomicToAtomic`/`AtomicToNonAtomic` aggregate casts
  follow `AggExprEmitter::VisitCastExpr` (peephole of a reverse pair;
  zero + field 0 for a padded destination; through an atomic temporary
  otherwise); assignment evaluates the right side into a temporary of the
  assignment's type (the result) and stores it with
  `store atomic`/`__atomic_store` (`atomic_agg_store`).
- **`++` of `_Atomic(bool)` (WP-165)**, accepted under
  `-Wno-increment-bool`: as clang's `EmitScalarPrePostIncDec`,
  pre-increment is `store atomic i8 1` (volatile when the object is)
  yielding `true`; post-increment is `atomicrmw xchg i8 1` (never
  volatile) yielding the old byte in its memory form (`i8`), which clang
  then stores or extends as if it were the `i1` value. Where the value
  must meet an `i1` clang 19 emits invalid IR; kiln deviates there
  (WP-183): `eval_bool` (a branch condition, `!`, `&&`, `||`, a `?:`
  condition) truncates an `i8` value of a `bool` expression to `i1`, and
  `emit_conditional` converts its second arm to the first arm's type
  when one is `i1` and the other `i8` (`trunc`, or `zext` that folds a
  constant), so the `phi` keeps clang's type (the first arm's). Two `i8`
  arms stay an `i8` `phi`, as in clang (valid IR, oracle-checked in
  `tests/codegen/atomic_bool_inc.cpp`); the invalid-in-clang cases are a
  run test with no oracle (`ext.c11.atomic/bool-postinc-run.cpp`).
- **`_Atomic` aggregates in calls (WP-183).** `_Atomic(T)` with `T` an
  aggregate is evaluated as an aggregate (`is_atomic_aggregate`, clang's
  `getEvaluationKind`) for a `return` operand, a parameter, an argument
  and a call result, and is an aggregate for the ABI (kiln_abi
  `is_aggregate`, clang's `isAggregateTypeForABI`): x86-64 classifies it
  `MEMORY` (`sret`/`byval`), AArch64 as an integer-coerced or indirect
  composite. An `EmitAtomicInit` from an `_Atomic` aggregate initializer
  (a call result) writes the whole atomic object, unprojected.
- **Floating `min`/`max_fetch` (WP-165), a deviation:** clang 19 compares
  the old value and the operand with an `icmp` (invalid IR); kiln yields
  `llvm.minnum` / `llvm.maxnum` of the two, the value `atomicrmw fmin` /
  `fmax` stored. `fetch_min`/`fetch_max` match clang.
- **`-Watomic-alignment` (WP-165)**, clang `EmitAtomicExpr`: for every
  atomic builtin except `__c11_atomic_init` (never for `_Atomic` lvalue
  accesses), at the expression's start, `cg_atomic_misaligned` (size,
  pointer alignment) when the pointer's alignment is not a multiple of
  the size, then `cg_atomic_oversized` (size, max inline width: 16 on
  aarch64, 8 on x86_64) when the size exceeds it; clang's texts, the
  latter with its double space before "bytes".
- **Validation:** `tests/codegen/atomic_{builtins,orders,lvalues,wide,
  qualifier,libcall,lv_libcall,padded,aggregate,bool_inc,alignment}.cpp` exact on
  aarch64 and x86_64; `run` tests under `tests/conformance/ext.c11.atomic/`
  (`fp-minmax-fetch-run.cpp` without an oracle).

## 7e. Landed (WP-171): `__builtin_va_arg` (`va_arg.rs`)
- **`emit_va_arg(e, dest)`** reads the address of the next argument
  (`va_arg_addr`) and loads a scalar, or `memcpy`s an aggregate into
  `dest`; an ignored aggregate read copies nothing (clang 19
  `Slot.asRValue()`), and an ignored `Ignore`-class type emits no code.
  The `va_list` base is the operand's lvalue, or its value when the
  operand is the decayed x86-64 array pointer.
- **Classification** comes from `kiln_abi::call::classify_va_arg`.
- **AAPCS64** (clang `AArch64ABIInfo::EmitAAPCSVAArg`), block order
  `vaarg.maybe_reg`, `vaarg.in_reg`, `vaarg.on_stack`, `vaarg.end`:
  - GPR types use `__gr_offs`/`__gr_top` (fields 3/1); FP and vector
    types use `__vr_offs`/`__vr_top` (fields 4/2).
  - The register size is `alignTo(size, 8)` for GPR (8 when passed
    indirectly) or 16 × the HFA member count for FPR. A GPR offset is
    realigned when the type's alignment exceeds 8.
  - An HFA/HVA with more than one member is copied from 16-byte-strided
    q-register slots into an `[N x base]` temporary.
  - On the stack: the pointer is rounded up with `llvm.ptrmask` when the
    alignment exceeds 8 and the argument is direct, then advanced by the
    slot size.
  - The `phi` address takes the smaller alignment. An indirect argument
    loads the pointer.
- **SysV x86-64** (clang `X86_64ABIInfo::EmitVAArg`):
  - `needed_int`/`needed_sse` gate `fits_in_gp` (`gp_offset ≤ 48-8n`) and
    `fits_in_fp` (`fp_offset ≤ 176-16n`).
  - In registers, the address comes from `reg_save_area`. Mixed int/SSE
    and two-SSE structs are reassembled into a temporary of the coerce
    type. A single register class is read in place, or through a
    temporary when the coerce type sits at offset 8, the type is larger
    than the register span, or an integer type is over-aligned.
  - Otherwise the address is `overflow_arg_area` (ptrmask-aligned above
    8), advanced by `alignTo(size, 8)`.
- **`min-legal-vector-width` of a definition** (`def_vector_width`) counts
  only IR vector parameters/returns, as clang's
  `CodeGenFunction::StartFunction` does, not vectors nested in coerced
  structs. Call sites keep the recursive `CallInfo::max_vector_width`.
- **`_Complex T`** (WP-237): clang 19's `ComplexExprEmitter::VisitVAArgExpr`
  takes the address from the same target walk (`va_arg_addr`; AAPCS64
  classifies a complex floating type as a two-element HFA and a complex
  integer as a general-register slot, x86-64 as for an argument, §8.17)
  and loads both parts whatever the consumer ignores.
- **Validation:** `tests/codegen/va_arg.cpp` and `va_arg_complex.cpp`
  exact on aarch64 and x86_64; `tests/conformance/cstdarg.syn/va-arg.cpp`
  and `va-arg-complex.cpp` (run).

## 7f. Landed (WP-178): dynamic initialization (`global_init.rs`)
Mirrors clang's CGDeclCXX ([basic.start.static], [basic.start.dynamic],
[basic.start.term], [stmt.dcl]/3).
- **Which variables.** `emit_var_def` folds the initializer first; a
  variable whose initializer does not fold gets a zero initializer and
  needs construction, one of class type with a non-trivial destructor
  (not a reference) needs destruction. The global is `constant` only
  when it is `const` or a reference and needs neither, and its type is
  not a class (or array of classes) with a `mutable` member, directly or
  in a subobject (clang `isTypeConstant`: `hasMutableFields`; WP-183,
  `module.rs` `has_mutable`, also for static locals and the static data
  members emitted with their class; `tests/codegen/const_mutable_fields.cpp`).
  Constant-initialized variables keep the constant path unchanged.
- **Init functions.** Each such variable gets an internal
  `__cxx_global_var_init[.N]` in `.text.startup` (`noinline`, `nounwind`
  under `-fno-exceptions`, `uwtable`, the target attributes, x86
  `min-legal-vector-width`) that runs the initializer into the global
  and registers the destructor with `__cxa_atexit(dtor, ptr, @__dso_handle)`
  (arrays: an internal `__cxx_global_array_dtor` helper). Ordered
  variables are called, in declaration order (deferred variables keep
  the slot of their first use), from `_GLOBAL__sub_I_<file>` (clang's
  file-name transformation), registered in `@llvm.global_ctors` with
  priority 65535.
- **Partially-ordered variables.** Template instantiations and
  non-internal inline variables have linkonce/weak linkage: their init
  function gets its own `@llvm.global_ctors` entry keyed on the variable
  (and `@llvm.used`, in the variable's comdat), guarded by an `i64` `_ZGV`
  (its first byte tested with a plain load for a template
  instantiation; the Itanium `__cxa_guard_acquire`/`release` protocol
  when thread-safe, i.e. a non-template inline variable; an `i8` guard
  only for a non-thread-safe internal variable).
- **Static locals.** A static local whose initializer does not fold is
  zero-initialized and guarded at its declaration: `_ZGV` (`i64`, in the
  local's comdat when it has one), an acquire-load of the guard byte
  (AArch64 tests the low bit), `!prof` branch weights `1, 1048575`,
  `__cxa_guard_acquire`, the initializer under a `__cxa_guard_abort`
  EH cleanup (`Cleanup::GuardAbort`), `__cxa_guard_release`, then the
  destructor registration.
- **Lifetime-extended temporaries.** A temporary bound to a reference
  with static storage becomes `_ZGR<var>_` (`let_global`): constant when
  the Oven folded it (the fold bridge now records
  `set_temporary_value`), else initialized in the variable's init
  function with its destructor registered via `__cxa_atexit`.
- **`constant` temporaries (WP-117, WP-204).** A `_ZGR` temporary is
  `constant` exactly as in clang's `GetAddrOfGlobalTemporary`: it has a
  constant initializer, and its *materialized type* is `const`, has no
  `mutable` member (`module::has_mutable`) and no destructor. The
  materialized type (`GlobalInit::materialized_type`) is found by
  locating the `MaterializeTemporaryExpr` whose
  `lifetime_extended_temporary` is the temporary in the extending
  variable's initializer and stripping what clang's
  `skipRValueSubobjectAdjustments` skips: parens, `NoOp` casts,
  derived-to-base casts of a record, and non-arrow member accesses of a
  non-bitfield, non-reference field. If anything was stripped it is the
  type of what remains, else the temporary's own type. A prvalue of
  class or scalar type is unqualified, so `const int& r = 5;`, `const P&
  r = P{1, 2};`, `const int& r = P{7, 8}.a;` and `int&& r = 6;` stay
  `global`; a temporary created by converting to the referenced type
  (`const double& r = 1;`, `const long& r = r1;`) or from a braced list
  (`const P& r = {3, 4};`) has the referenced `const` type and is
  `constant` unless it has a `mutable` member (`const M& r = {1};`). The
  backing array of a namespace-scope `std::initializer_list` is a
  `const` array, so it is `constant` when constant-initialized. A
  scalar braced list materializes directly (`const int& r = {};` is
  `constant`), but `const int& r = {7};` materializes the `int` operand
  of a `NoOp` cast inside the list and stays `global`. The rule is the
  same for a temporary first reached through a frozen constant pointer
  (`consts.rs` `ptr_base`, `PtrBase::Temporary`: a reference member of a
  constexpr aggregate, `constexpr R x{{}};`, or a constexpr pointer to a
  temporary), since `let_global` decides constness from the materialized
  type, not the type it is passed (WP-224).
  `tests/codegen/init_list_global.cpp`,
  `tests/codegen/ref_temporary_constant.cpp`,
  `tests/codegen/ref_temporary_braced.cpp`.
- Tests: `tests/codegen/global_init_*.cpp` (IR parity on both triples),
  `tests/conformance/basic.start.dynamic/order-run.cpp`,
  `tests/conformance/stmt.dcl/static-local-run.cpp`.
- **Destructed temporaries (WP-231).** A temporary of a class with a
  non-trivial destructor extended by a namespace-scope reference
  (`const D& r = D{1};`) is never constant: the Oven rejects the result
  (oven.md, clang `CheckLValueConstantExpression`), so `_ZGR` is a
  `zeroinitializer` `global`, built in `__cxx_global_var_init` and
  registered with `__cxa_atexit`, and the reference is `global ptr null`
  stored there. `tests/codegen/ref_temporary_dtor_global.cpp`,
  `tests/conformance/class.temporary/extended-temporary-destroyed-at-exit.cpp`.
- **Reference read-through (WP-231).** A `DeclRefExpr` to a reference
  marked `NonOdrUse::Constant` (sema: the reference is
  constant-initialized, [expr.const]/4, cached in `Sema::ref_usable`)
  emits its folded initializer (`expr.rs` `constant_ref_lvalue`, clang
  `EmitDeclRefLValue`'s `NOUR_Constant` path) instead of loading the
  reference; local reference initializers are fold candidates, and a
  read through a reference to a `const` non-volatile object folds to the
  value (`fold.rs` `refers_to_const_var`, clang `CEK_AsValueOrReference`).
  `tests/codegen/ref_constant_read.cpp`,
  `tests/conformance/basic.def.odr/reference-constant-init-not-odr-used.cpp`.
- **Not implemented:** `init_priority`, `thread_local` dynamic
  initialization (TLS wrappers), folding of reads of `const` class
  objects that clang permits ([basic.start.static]/3), and `constant` on
  an `extern` declaration of a reference or const scalar (clang
  `isTypeConstant`; `module.rs` `get_gvar` emits `external global`).

## 7g. Landed (WP-118): WP-100 follow-ups
- **Constructor loops.** Sema gives `new K[2][3]` / `new K[n][3]()` a
  `CXXConstructExpr` of the array type (`new_class_default` looks at the
  base element), and a mem-initializer `arr()` of an array of classes
  with a user-declared constructor one too (`ctorinit.rs`), so codegen
  runs the per-element constructor loop (with its partial-array EH
  cleanup) rather than nothing or a memset. `emit_agg` no longer
  memsets a braced array (`K y[2][3]{}`) whose base element class has a
  user-declared constructor (clang `CheckAggExprForMemSetUse`).
  Zeroing array new of a class with a non-trivial implicit constructor
  memsets each non-empty element before its constructor call
  (`EmitCXXAggrConstructorCall` with `zeroInitialize`).
- **Array cookies** (Itanium ABI 2.7, clang `ItaniumCXXABI::
  InitializeArrayCookie`/`ReadArrayCookie`). Needed when the element
  class has a non-trivial destructor or a class-scope usual
  `operator delete[]` takes the size, never for the reserved placement
  form. Size `max(8, alignof(elem))`; the `i64` count is stored in its
  last 8 bytes and the elements follow it. `EmitCXXNewAllocSize` adds it
  with `llvm.uadd.with.overflow` (constant-folded for a constant bound)
  and the initializer gets the size without it; the allocation is
  treated as at least 8-aligned. `delete[]` reads the count back,
  pushes the deallocation as a normal-and-EH `ArrayDelete` cleanup
  (`CallArrayDelete`), destroys the elements in reverse with a
  zero-length check and a partial-array EH cleanup around each
  destructor call (`emitArrayDestroy` with `useEHCleanup`), and passes a
  sized deallocation function `esize * n + cookie`. Sema now picks the
  global sized `operator delete[]` for a `delete[]` whose element type
  has a non-trivial destructor (clang `CanProvideSize`) and marks the
  element destructor of an array new referenced. A regular partial-array
  cleanup pushed inside a conditional branch (nothrow `new[]`) saves its
  ends (`SavedPartialArray`).
- **Throwing destructors in array destroy loops.** The normal-path
  destroy of a local array, of an array member (`DestroyField`) and of a
  static array (`__cxx_global_array_dtor`) pushes a partial-array EH
  cleanup around each destructor call (clang `isForNormalCleanup() &&
  useEHCleanupForArray`); the EH-path destroy does not. It only
  materializes when the destructor can throw. On aarch64 the static-array
  helper keeps the loop-canonicalization deviation above.
- **Aligned new cleanup.** Sema's `isPlacementNew` test counts any
  operator new other than `(size_t)`, so the cleanup of an aligned
  `new` calls the unsized aligned delete (`_ZdlPvSt11align_val_t`),
  as clang does.
- **Already done elsewhere:** `unreachable` after a user `[[noreturn]]`
  call; static-local guards (WP-178, §7f); the anonymous union member
  type is `%union.anon` (`anon_union_names.cpp`).
- **Corpus:** `tests/codegen/new_array_ctor_loops.cpp`,
  `tests/codegen/new_array_cookie.cpp`, `tests/codegen/eh_new_aligned.cpp`,
  `tests/codegen/anon_union_names.cpp`,
  `tests/codegen/eh_array_destroy_throwing.cpp` (exact on both triples);
  `run` tests `expr.new/run-array-cookie`,
  `except.ctor/run-array-dtor-throws`.
- **Deviation (mangling, follow-up):** a namespace-scope anonymous union
  variable (`static union { int ga; };`) mangles as `_ZLUt_`; clang
  uses the first named member, `_Z2ga` (Itanium 5.1.2).

## 7h. Landed (WP-113): explicit instantiation linkage and order
Linkage follows clang 19's `GetGVALinkageForFunction` /
`GetGVALinkageForVariable` (`-O0`, ELF). "Under" means the entity's own
kind, else that of the innermost explicitly instantiated or specialized
enclosing class (`Cg::explicit_inst_kind`):

| Entity | clang GVA | kiln IR |
|---|---|---|
| implicit instantiation (function, variable, member) | `GVA_DiscardableODR` | `linkonce_odr` in its own COMDAT, deferred until used |
| explicit specialization (`template<>`), non-inline | `GVA_StrongExternal` | strong `external` (`dso_local`); structors use C1/D1 aliases |
| explicit specialization, inline / constexpr | `GVA_DiscardableODR` | `linkonce_odr` |
| under an explicit instantiation definition | `GVA_StrongODR` | `weak_odr` in its own COMDAT, never deferred; vtable/RTTI `weak_odr`; C2/D2 in a C5/D5 COMDAT with `weak_odr` C1/D1 aliases |
| under an explicit instantiation declaration | `GVA_AvailableExternally` (only defined when optimizing) | no definition; references name the external symbol; vtable an `external` declaration |
| implicitly declared special member of either | `GVA_DiscardableODR` | `linkonce_odr` (not an instantiation) |
| member (or member of a member class) declared `exclude_from_explicit_instantiation` | `GVA_DiscardableODR` | `linkonce_odr`, deferred until used: the enclosing class's explicit instantiation does not govern it (`Ast::excluded_from_explicit_instantiation`; libc++'s `_LIBCPP_HIDE_FROM_ABI`, WP-214; `tests/codegen/exclude_from_explicit_inst.cpp`) |
| static local of a `weak_odr` function | `GVA_DiscardableODR` | `linkonce_odr` |

Where the definitions are emitted (clang hands them to
`HandleTopLevelDecl`, so the order also decides which callers are
`nounwind`, clang `TryMarkNoThrow`):
- `template struct X<int>;` and `template R f<int>(...)`: at the explicit
  instantiation; members are emitted in member order
  (`emit_class_members`). An implicit instantiation emitted earlier is
  re-emitted there `weak_odr` (its deferred `linkonce_odr` definition is
  never produced).
- An explicit instantiation definition following an explicit
  instantiation declaration of the same class: at the definition, not at
  the declaration where sema first listed the specialization
  (`TemplateTables::add_moved_class_inst`; codegen skips the class at its
  listed position and emits it from `explicit_member_insts`).
- A member explicit instantiation (`template void X<int>::f();`) or a
  member class (`[extern] template struct O<int>::I;`, [temp.explicit]/3):
  right after the declaration that precedes it lexically
  (`TemplateTables::explicit_member_insts`). The member class gets the
  explicit kind in its member-spec info, so its members and vtable follow
  the table above; member templates are not instantiated
  ([temp.explicit]/10).
- A function, member function, structor or variable template
  specialization explicitly instantiated before its template's
  definition is seen (for a variable template, after a non-defining
  `extern` declaration; sema `instantiate_late_var_spec` rebinds the
  specialization to the defining redeclaration): instantiated at the end
  of the TU and emitted after the last top-level declaration, in
  explicit instantiation order, before the key-function vtables and the
  deferred definitions (clang `PerformPendingInstantiations`;
  `TemplateTables::late_explicit_insts`, `Cg::emit_late_explicit_insts`).
  Members of an explicitly instantiated class that are defined after the
  class's explicit instantiation are not instantiated (as clang).
- Explicitly specialized members of an explicitly instantiated class
  (`template<> void X<int>::f() {}`, also static data members and
  structors) are ordinary strong definitions emitted where they appear;
  the class's explicit instantiation neither instantiates nor re-emits
  them (sema `instantiate_members_of` and codegen `emit_class_members`
  skip `SpecKind::ExplicitSpecialization`, clang
  `InstantiateClassMembers`). An out-of-line `template<>` redeclaration
  of a static data member (`Sema::act_on_explicit_member_var_spec`,
  WP-117) or structor (`Sema::mark_member_explicit_spec`) of an implicit
  specialization sets that kind, with or without an explicit
  instantiation.

Tests: `tests/codegen/explicit_instantiation*.cpp`,
`tests/codegen/structor_rauw_order.cpp` (IR parity on both triples).

## 7i. Landed (WP-183): variable length arrays (`vla.rs`)
Clang's `EmitAutoVarAlloca` / `EmitVariablyModifiedType` for the GNU
extension, byte-identical at -O0 on both triples (`tests/codegen/vla.cpp`):
- **Bounds.** `emit_vla_bounds(q)` walks a variably modified type
  (arrays, pointers, references) and evaluates each `VariableArray` bound
  once per function, zext/trunc to `i64` as unsigned, into
  `Fx::vla_sizes` (clang `VLASizeMap`). Variable declarations, and
  `typedef`/alias declarations in a `DeclStmt`, evaluate their bounds at
  the declaration. The fold prepass (`fold.rs` `vla_bounds`) visits the
  same bounds, so a `const _Atomic(int)` bound folds to a constant as in
  clang's `tryEmitAsConstant`.
- **Stack save.** The first VLA in a scope (`Fx::did_stack_save`, reset by
  `begin_scope` and restored by `end_scope`, clang `RunCleanupsScope`)
  allocates a `saved_stack` pointer slot, calls `@llvm.stacksave.p0`, and
  pushes a normal-only `Cleanup::StackRestore` (`@llvm.stackrestore.p0`).
- **Allocation.** After the save, `vla_size` multiplies the nested
  dimensions (`mul nuw`), and `Op::DynAlloca` emits `alloca elem, i64 n`
  aligned to the element alignment (at least 16 on x86-64, clang's large
  array alignment). Each non-constant dimension gets an `__vla_expr` `i64`
  slot and store (debug-info shadow, as clang). VLA elements of class type
  needing construction or destruction are an M3 "not supported yet" error.
- **Uses.** Array-to-pointer decay of a VLA emits no GEP; a subscript
  through a decayed VLA uses the pointer's alignment, and one whose result
  is itself a VLA scales the index by the element count (`mul nsw`).
  `sizeof` of a VLA operand or type emits the operand ignored (or its
  bounds) then `mul nuw` by the element size (skipped for size 1).
  `kiln_abi::convert_mem` lowers a `VariableArray` as its element type.

## 7j. Landed (WP-202): libc++ m3 "not supported yet" sweep
- **Alignment of an allocation function's result** (clang
  `AllocAlignAttrEmitter`, `new_delete.rs` `alloc_align_assume`): for a
  replaceable aligned `operator new` / `__builtin_operator_new` and a GNU
  `alloc_align(N)` function or member (`N` excludes `this`), a constant
  power-of-two alignment is the ret `align N` attribute; any other value
  (runtime, or a constant that is not a power of two) is zext'ed to `i64`
  and asserted after the call, `call void @llvm.assume(i1 true)
  [ "align"(ptr %r, i64 %a) ]` (FIR `Op::AssumeBundle`). None after a call
  that ended the block (`noreturn`).
- **Base-to-derived casts** (clang `GetAddressOfDerivedClass`,
  `class.rs` `derived_ptr` / `derived_cast_offset`): `static_cast` (and the
  C-style cast) of a pointer or reference to a base to the derived class
  subtracts the base's non-virtual offset with `gep inbounds i8, p, -off`;
  offset 0 is a no-op. A pointer cast is null-checked (`cast.notnull` /
  `cast.null` / `cast.end` and a `phi`) unless the operand is `this`; a
  reference (lvalue) cast never is. A virtual step is rejected by sema.
- **Implicit assignment of an array member** (sema `assign_array_loop`,
  clang `buildSingleCopyAssignRecursively`): when the element assignment is
  not trivial, the defaulted copy/move assignment holds one loop per
  dimension, `for (size_t __iD = 0; __iD != N; ++__iD)` assigning
  `to[__iD]` from `from[__iD]` (each element cast to an xvalue for a move).
  Codegen needs nothing new: the loop is an ordinary `ForStmt` (allocas in
  order, `icmp ne`, plain `add i64 1`, `!llvm.loop`).
- **Array compound literals** (sema `array_compound_literal`; `expr.rs`
  `agg_visit`): `(T[]){...}` is a prvalue of the written array type with
  the bound deduced from the list; it is materialized when bound to a
  reference or decayed (`conv::decay` materializes an array prvalue before
  `ArrayToPointerDecay`, clang `DefaultFunctionArrayConversion`). The
  aggregate emitter emits the literal as `EmitAggExpr` of its initializer
  (clang `VisitCompoundLiteralExpr`; C++ literals are not destroyed there).
- Oven fixes found on the way: constant array types of differently
  qualified elements (`const E[3]`, `E[3]`) share one `OTy`
  (`Types::const_arrays`, clang `getUnqualifiedArrayType`), and a pointer
  to a whole array object (`memcpy(&arr, ...)`) is a one-object extent
  (`Types::extent_in`).
- Sema deduction: a braced list for `P'[N]` with a non-dependent `P'`
  deduces only `N` (clang `DeduceFromInitializerList`;
  `std::to_array<long>({1, 2, 3})`).

Tests: `tests/codegen/alloc_align_runtime.cpp`,
`tests/codegen/cast_base_to_derived.cpp`,
`tests/codegen/implicit_array_assign.cpp`,
`tests/codegen/compound_literal_array.cpp` (IR parity on both triples);
`tests/conformance/class.copy.assign/array-member-loop.cpp` (constant
evaluation and run).

Not done: the codegen side of a braced-init-list functional cast
operand (`T({...})`; its sema is WP-200's).

## 7k. Landed (WP-190): WP-170 follow-ups
- **Zero-initialization with member pointers** (clang
  `isZeroInitializable` / `EmitNullConstant` / `EmitNullInitialization`;
  `kiln_abi` `irtype.rs` `zero_init`, `null_init.rs`): an Itanium data
  member pointer's null value is `-1`, so a type holding one (directly,
  as an array element, or in a base or member) is not zero-initializable.
  Its null constant is built field by field (`null_const`: `i64 -1` for
  each data member pointer, `zeroinitializer` elsewhere, flattened base
  subobjects, packed filler arrays for the tail of an array constant), and
  a runtime zero-initialization (`T()`, `T{}`, value-initialized `new T`,
  `new T[n]()`, globals, members of an aggregate) stores that constant
  instead of a `memset 0` (`null_init_array_n` loops over a runtime count).
  A function member pointer's null is `{0, 0}` and stays zero-initializable.
- **Parenthesized aggregate initialization** (`Agg a(3)`, P0960): already
  lowered through `CXXParenListInitExpr` as a braced list; WP-190 adds IR
  parity tests (default member initializers for trailing members, nested
  aggregates and bases, arrays, references, destructible temporaries,
  mem-initializers, static and dynamic globals, `new Agg(...)` and
  `new T[n](...)`).
- **Array cookies** (§7g) were re-verified for `new T[n]` with a
  non-trivial destructor. What was missing is the EH cleanup of a
  braced-list `new T[n]{...}` (clang `EmitNewArrayInitializer`): with
  exceptions on and a destructed element, an `array.init.end` slot (no
  initial store) holds the current element pointer, stored before each
  explicit element, before the constructor filler loop and right after the
  `phi` of the generic loop; an `IrregularPartialArrayDestroy` over the
  base element type (no extra zero GEPs for a multidimensional array)
  destroys `[begin, *slot)` on unwind and is deactivated on every exit.
  Inside a conditional (nothrow `new`), `begin` is saved to a slot first
  (`IrregularPartialArray::saved`).
- **Auxiliary allocas** (clang `AddAuxAllocas` / `AllocaTrackerRAII`;
  `cleanup.rs` `add_aux_allocas`, `fir.rs` `mark` / `allocas_from` /
  `erase_with_users`): allocas created while pushing an irregular partial
  array cleanup belong to it; when the cleanup is popped with neither a
  normal nor an EH entry, they are erased with every user, so a list that
  cannot throw has no `arrayinit.endOfInit` / `array.init.end` traffic.
- **Allocation attributes on virtual calls**: a virtual call takes the
  allocation attributes of the method it names (`allocsize`, `noalias` for
  `malloc`, ret `align N` for a constant `alloc_align`); a non-constant
  alignment is the `llvm.assume` "align" bundle of §7j.
- **Entity wording of `err_init_conversion_failed`** (sema
  `init_entity`): a functional cast `T{x}` says "a value" (clang
  `EK_Temporary`) and `new P(d)` / `new P{d}` says "a new value"
  (`EK_New`) instead of "a variable".

Tests: `tests/codegen/zero_init_member_ptr.cpp`,
`tests/codegen/const_array_filler.cpp`,
`tests/codegen/paren_aggregate_init_more.cpp`,
`tests/codegen/array_new_partial_eh.cpp`, virtual cases in
`tests/codegen/alloc_align_runtime.cpp` (IR parity on both triples);
`tests/conformance/dcl.init.general/zero-init-member-pointer-run.cpp`
(run); `tests/conformance/expr.type.conv/list-init-entity-diag.cpp`
(strict diagnostics).

Not done: "a member subobject" / "an array element" wording (WP-195);
a regular partial-array cleanup for the generic non-list `new T[n](...)`
loop; `internal constant` (vs `global`) for constant temporaries bound
by `const int& a = {};`; the `A{&d}` array temporary; parsing
`new (void (T::*)())()`.

## 7l. Landed (WP-205): WP-118 EH follow-ups and GNU case ranges
- **Lifetime-extended temporaries** (clang `pushLifetimeExtendedDestroy`,
  `class.rs` `push_lifetime_extended_destroy`): a temporary bound to a
  reference with automatic storage gets an EH-only destroy cleanup whose
  deactivation is deferred to the end of the enclosing full-expression
  (`pushDestroyAndDeferDeactivation`), plus a lifetime-extended
  normal-and-EH cleanup guarded by an active flag that is set at the end
  of the full-expression (`pushCleanupAfterFullExprWithActiveFlag`).
  Inside a conditional branch the address is saved
  (`saveValueInCond`), the deferred cleanup gets its own `cleanup.cond`
  flag, and both flags start false and are set in the branch.
- **Deferred deactivation** (clang `CleanupDeactivationScope`;
  `Cleanups::deferred`, `expr.rs` `push_deferred_destroy` /
  `deactivate_deferred`): one stack of (cleanup, dominating placeholder)
  pairs serves braced aggregate members, lambda captures, the
  lifetime-extended EH cleanups above and statement scopes; the end of a
  record/array init list, a lambda or a scope (`end_scope`) deactivates
  every entry pushed since it began, innermost first, so the EH cleanups
  of temporaries extended inside a braced aggregate are gone once the
  aggregate is complete. Deactivation allocates its `cleanup.isactive`
  flag as an auxiliary alloca of the enclosing scope, erased with it when
  the cleanup turns out unused.
- **Sema (flagged):** `temporaries_in_aggregate` also descends into both
  arms of a conditional operator, so `const A& a = c ? A{T()} : A{T()}`
  extends the `T` temporaries of each arm ([class.temporary]/6.10).
- **Conditional temporaries at a computed address**
  (`DominatingValue<Address>::save`): a complete-object destroy cleanup
  (`Cleanup::SavedObject`; an array too) pushed in a conditional branch
  at an address that is not in the entry block stores the address to a
  `cond-cleanup.save` slot and reloads it in the cleanup.
- **Delegating constructor with virtual bases** (clang
  `CallDelegatingCtorDtor`, `Cleanup::DelegatingDtor`,
  `class.rs` `call_dtor_delegating`): the base-object variant's EH
  cleanup passes the current function's VTT parameter unadjusted
  (`GetVTTParameter(Delegating)`), not a sub-VTT GEP.
- **Conditional `new` with class placement arguments**
  (`DominatingValue<RValue>::save` of an aggregate): the argument's
  address is saved like a pointer when it needs saving
  (`Saved::AggSlot`); an `agg.tmp` in the entry block is used as is.
- **GNU case ranges** (`stmt.rs` `emit_case_range`, clang
  `EmitCaseStmtRange`): the body block is emitted first; an empty range
  adds nothing; a range of fewer than 64 values adds one switch case per
  value; a wider one appends a `sw.caserange` block at the end of the
  function (`sub cond, lo`; `icmp ule diff, hi - lo`; branch to the body,
  else to the previous range block or the default), and the switch's
  default destination becomes the last range block
  (`SwitchCtx::range_head`). A switch with a case range is never folded
  as a constant switch. Sema is in spec/sema.md §7.5.
- **Deviation (documented):** namespace-scope and static local arrays
  with a throwing destructor (`__cxx_global_array_dtor`) keep the
  aarch64 loop-canonicalization deviation of §6;
  `eh_array_destroy_throwing_global` is exact on x86_64 and listed in
  `ir_vs_clang`'s deviations for aarch64.

Tests: `tests/codegen/eh_lifetime_extended.cpp`,
`eh_conditional_computed_addr.cpp`, `eh_delegating_vtt.cpp`,
`eh_new_class_placement.cpp`, `gnu_case_range.cpp` (exact on both
triples), `eh_array_destroy_throwing_global.cpp` (x86_64 exact);
`run` tests `except.ctor/run-lifetime-extended-unwind`,
`stmt.switch/run-gnu-case-range`; diagnostics
`stmt.switch/gnu-case-range`,
`except.terminate/throw-in-noexcept-reach-unevaluated`.

## 7m. Landed (WP-216): WP-190 deferred items
- **Array temporaries** (`A{...}` with `using A = T[n]`, and braced
  lists bound to a reference to array): the element-wise destroy of a
  bound array temporary is an `ArrayDtor` full-expression cleanup (clang
  `EmitCXXTemporary` → `pushDestroy` of the array type). A
  lifetime-extended array temporary at namespace scope registers a
  `__cxx_global_array_dtor` helper with `__cxa_atexit`
  (`register_temporary_dtor`, clang `generateDestroyHelper`); its aarch64
  loop shape is the §7f deviation (`array_temporary_global`). The Oven
  treats a lifetime-extended array of const elements as usable in
  constant expressions (`resolve_let`).
- **Default-argument temporaries:** the default argument is emitted
  without its definition's `ExprWithCleanups`, so its temporaries join
  the caller's full-expression (spec/sema.md §5.10).
- **`new T[n]{...}` generic loop:** the list's irregular partial-array
  cleanup is deactivated in the loop body after the first element's
  initializer (clang `EmitNewArrayInitializer`). clang's
  `pushRegularPartialArrayCleanup` for a non-list `new T[n](...)` /
  `new T[n]` loop is reachable only for an element initializer that is
  neither a `CXXConstructExpr` nor a memsettable value-initialization
  (in practice ObjC ARC types): sema always builds a `CXXConstructExpr`
  for a C++ class element, whose `construct_array_n` loop already pushes
  `Cleanup::PartialArray`.
- **Thunks of explicit instantiations:** a `weak_odr` thunk gets its own
  comdat (clang `setThunkLinkage`), not the `D5` comdat of its
  destructor. Base destructors of an explicitly instantiated class run in
  reverse declaration order and are emitted in clang's order.

Tests: `tests/codegen/array_temporary.cpp`, `array_temporary_global.cpp`
(x86_64 exact), `default_arg_temporary.cpp`,
`explicit_instantiation_base_dtors.cpp`; `run` tests
`class.temporary/run-array-temporary`,
`dcl.fct.default/run-default-arg-temporary`; constexpr tests
`class.temporary/constexpr-array`,
`dcl.fct.default/constexpr-default-arg-temporary`.

## 7n. Landed (WP-211): GNU `_Complex` (`complex.rs`)
Mirrors clang 19's `ComplexExprEmitter` at `-O0`, by default with the
full complex range (`CX_Full`; `CodegenOptions::complex_range`, below).
- **Representation.** A complex value is the pair `(real, imag)` of
  element values. In memory it is the IR struct `{ T, T }` (kiln_abi
  `irtype.rs`), with parts at struct GEP 0/1. A part is `None` when the
  consumer ignores it (clang `IgnoreReal`/`IgnoreImag`) or when the
  operand of an arithmetic operator is real.
- **Expressions.** These are lowered like clang's visitors:
  - loads, `__real`/`__imag` (lvalue and rvalue);
  - casts between real and complex;
  - unary `+ - ~` (conjugate), `++`/`--` (real part only);
  - `+ - * /`, assignment and compound assignment (the left value
    converted to the computation type and back);
  - `==`/`!=`, `?:` (phi of both parts), and conversion to `bool`;
  - list initialization, `__builtin_complex`;
  - folded constants (`extractvalue` of the constant pair);
  - `ImaginaryLiteral` (a null real part).
- **Multiplication.** For floating types it uses the four products, then
  the NaN check on both results (`fcmp uno`, branch weights 1:1048575),
  then the library call `__mul{s,d,x,t}c3`. Integer multiplication is
  inline. A real operand uses the shortened forms.
- **Division.** It calls the library routine `__div{s,d,x,t}c3` for
  complex floating divisors. A real divisor divides each part directly.
  Integer division uses clang's inline formula.
- **Complex range** (WP-237; `CodegenOptions::complex_range`, clang's
  `LangOptions::ComplexRange`, set by the driver, spec/driver.md). Only
  floating `*`/`/` with two complex operands (a real dividend gets a
  zero imaginary part) depend on it:
  - `Full`: the NaN-checked product and the library call above.
  - `Basic` (`-fcx-limited-range`, `-ffast-math`): the four products
    without the NaN check; `EmitAlgebraicDiv`, `((ac+bd) + i(bc-ad)) /
    (cc+dd)`, in clang's instruction order.
  - `Improved` (`-fcx-fortran-rules`, `-fcomplex-arithmetic=improved`):
    the plain product; `EmitRangeReductionDiv` (Smith): `fcmp ugt` of
    the `llvm.fabs` of `c` and `d`, a block scaling by `d/c`, one by
    `c/d`, and a `phi` per part.
  - `promoted` is not supported (the driver rejects it, spec/abi.md
    §8.17).
- **Calls.** A complex argument or return goes through the target
  classification (spec/abi.md §8.17). A returned pair is unpacked from
  the coerced type (`cx_call_result`). Delegating calls (inheriting
  constructors, lambda static invokers) forward the parts unchanged.
- **Validation.** `tests/codegen/complex_{arith,calls,ops,types,delegate,
  imaginary,limited_range,improved_range}.cpp` and `va_arg_complex.cpp`
  are exact on aarch64 and x86_64; `tests/conformance/ext.complex/
  {limited,improved,full}-range.cpp` (run) observe each range.
- **Not implemented:** `_Complex _Float16` (kiln has no `_Float16`),
  `-fcomplex-arithmetic=promoted`, and `#pragma STDC CX_LIMITED_RANGE`
  (spec/abi.md §8.17 "Deviations").

## Status and what remains
**Done:** WP-052 (lowering, §2–§4; unit corpus 79/79 exact on aarch64)
and WP-052b (driver §5; IR-diff corpus 92/92 exact on both triples; 33
`run` tests). WP-052b also fixed, additively: `kiln_llvm`
`Module::set_global_value_type` (a static local retyped by its
initializer keeps its position, as clang's), and kiln_sema's implicit
move, which no longer treats an lvalue-reference variable as movable.

**Done (WP-052 + WP-052b):** `fir.rs`, `module.rs`, `func.rs`, `consts.rs`,
`fold.rs`, `expr.rs` (§3), `stmt.rs` (§4), the driver path (§5) and the
IR-diff corpus (§6: then 46 programs, exact on both triples; see §6 for
the current count). The unit corpus
is 79/79 exact against clang on aarch64 (`bitfields_compound` became exact
with WP-057; WP-058 added five cases). Additive `kiln_abi` fixes came with
part 1: internal-linkage variable mangling, static-local discriminators,
and indirect returns with `byval` (clang's `getNaturalAlignIndirect`, so
no `result.ptr` spill).

**Remaining (M3):**
1. Oven folding of constant-initialized references, for the
   direct-referee use (deviations above); `init_priority` and
   `thread_local` dynamic initialization (§7e).
2. The Sema gaps above, then corpus cases for them: class copy-init,
   braced returns, reference casts, temporaries, range-for over arrays.
3. Bit-field and base-class constants, reference members.
4. Header-dependent `run` tests, once `decltype` parses (drop their
   `xfail(kiln)`).
5. WP-202 follow-ups: `p->m` through a base-to-derived (or derived-to-base)
   cast uses the pointee's natural alignment rather than clang's
   `EmitPointerWithAlignment` (e.g. `align 4` vs `align 8` for a base of an
   8-aligned class); the Itanium mangling of a dependent-sized array
   (`A <expression> _`, emitted as `u7unknown` for `T (&&)[N]`); class-type
   compound literals and list-initialized array temporaries (`A{1, 2}` with
   `using A = int[2]`) in sema.
