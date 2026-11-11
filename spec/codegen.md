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
  true in it; a `constexpr` variable sema already evaluated
  (`VarEvalState::Value`) keeps that value instead, since only sema's
  bridge runs reflection intrinsics (`constexpr bool b = type_of(R) ==
  ^^int;` in an emitted body; WP-485); the other foldable initializers (local `const` arrays,
  records and references) use `try_fold`, where it is false (WP-063,
  matching clang's IR). A non-`const` automatic array or record is folded only
  when it is POD (clang `IsConstantAggregate`; a non-POD one's value is
  never used, and building it cost a copy of the whole object per
  variable; WP-633). When that constant initialization fails, a
  scalar initializer (arithmetic, enum, pointer or member pointer type)
  is re-folded with `Oven::fold_rvalue`, where it is false, and a value
  it yields becomes the static initializer (clang
  `ConstantEmitter::tryEmitPrivateForVarInit`, which emits a non-constant
  initializer it can fold; WP-415): `int g = __builtin_is_constant_evaluated()
  ? x : 7;` is the global `i32 7` with no dynamic initializer, and a
  re-folded initializer is never registered for constant destruction.
  A vector initializer is re-folded the same way (`tryEmitPrivate`
  evaluates it as an rvalue: `i4 gv = cv;` with `const i4 cv = {..}` is
  a static initializer; WP-453, §7w). An initializer list is re-folded
  the same way (clang's `ConstExprEmitter` emits each element with
  `tryEmitPrivateForMemory`, which falls back to `EvaluateAsRValue`;
  WP-492): `H h = {(void (*)())&f, 1};` and `A a = {
  __builtin_is_constant_evaluated() ? x : 7};` are static initializers.
  Other aggregate initializers (a constructor call, a list with
  temporaries) are not re-folded and keep their dynamic initializer. A
  cast to or from a function pointer continues in a fold (spec/oven.md
  "Pointers that point at nothing"), so `void (*p)() = (void (*)())&f;`
  is the global `ptr @f`.
  Uses of a const integral variable whose constant initialization failed
  fold to that same non-manifest value (spec/oven.md "Lax folds"), so
  `return cg;` emits `ret i32 9` and the unused internal `cg` is not
  emitted. Validation: `tests/codegen/static_init_refold.cpp`.
  Calls of builtins with an integral or floating
  result are candidates too (clang `EmitBuiltinExpr` tries
  `EvaluateAsRValue` first), except `__builtin_is_constant_evaluated()`
  (WP-088). A direct call of a C library function clang's evaluator
  evaluates (`builtin::lib_evaluated`: `strlen`, `strcmp`, `memcmp`,
  `wcslen`, ...; `builtins::lib_fold_callee`) is a candidate too, unless
  the caller is `no_builtin`: a folded integral or floating result is
  emitted as the constant and the arguments are not emitted, so
  `strlen("abc")` is `i32 3`/`i64 3` and `memcmp(a, b, 2)` with
  constant arrays folds (clang `EmitBuiltinExpr` folds the
  `BIstrlen`-style calls the same way; a pointer result such as
  `strchr` stays a call; WP-940,
  `ext.gnu.builtins/library-strlen-fold-ir.cpp`). Member access names
  a constant the same way a `DeclRefExpr` does (clang `tryEmitAsConstant`
  via `tryToConvertMemberExprToDeclRefExpr`, then `EvaluateAsInt`):
  `a.n`, `p->n` and `A::n` of a const non-volatile static data member
  with a constant initializer of scalar type (integral, enum, floating,
  pointer, member pointer, `nullptr_t`) fold, and so does an integral or
  enum non-static member whose whole access is a constant; the base
  object is still emitted when it has side effects (`get().n` calls
  `get`; WP-940, `expr.ref/ir-constexpr-static-member-fold.cpp`).
  Constant-reference loads look through a substituted non-type template
  parameter (clang `VisitSubstNonTypeTemplateParmExpr` visits the
  replacement): `template <const int &R> int ref() { return R; }` with
  `ref<cg>()` and `static const int cg = 3` is `ret i32 3` (WP-998,
  `temp.arg.nontype/reference-arg-const-fold-ir.cpp`); `*P` with
  `P = &cg` stays a load, as in clang. A `CXXDefaultArgExpr` has no children, so the pre-pass walks
  its parameter's default argument once, at the first use (WP-136).
  The pre-pass walks every statement codegen emits, including try blocks
  and their handlers (WP-214: a default argument naming a non-inline
  `static const` member, first used in a try block, loaded the
  `available_externally` member and failed to link; libc++'s
  `memory_resource::__max_align`). Validation:
  `tests/codegen/static_member_odr_use.cpp`. A default member
  initializer is walked once, at its field, so its candidates are folded
  for every constructor that uses it (`int m = c;` with
  `static const int c = 5;` stores `5`; WP-162,
  `tests/codegen/nsdmi_static_const_fold.cpp`).
  Non-initializer candidates (a load of a const variable, a builtin
  call) fold with `Oven::fold_rvalue` (lax mode, WP-165): like clang's
  `tryEmitAsConstant`, a read of any `const` non-volatile variable with an
  initializer folds, even one not usable in constant expressions (`const
  double`, `const _Atomic(int)`, a `const` array or class element), so a
  global whose every use folds is not emitted. A load of a const variable
  is recognized through parentheses and `NoOp` casts (the xvalue cast of
  an implicitly moved `return x;`). An arm of a glvalue conditional under
  a load (`c ? A : B`) is not a load candidate; a scalar non-reference
  variable it names without odr-use (sema's `non_odr_use_constant`) is
  emitted as its folded initializer (clang `VisitDeclRefExpr` ->
  `tryEmitAsConstant`; `expr.rs` `non_odr_use_constant`, WP-775), so no
  definition of it is needed. A read of a `mutable` member is not
  folded (the Oven's "read of mutable member" check, spec/oven.md); the
  variable's other members are.
  A reference variable whose referee type is `const` non-volatile folds
  the same way (clang `checkVarTypeForConstantEmission`:
  `CEK_AsValueOrReference`), so `a + b` for `const int& a = {}; const int
  b = 5;` is a constant (WP-231, WP-233; see also "Reference
  read-through").
  Validation: `tests/codegen/const_fold_lax.cpp`,
  `tests/codegen/ref_const_fold.cpp`.
  A fold whose value designates an automatic object (C++26 P2686R5
  constexpr references and pointers to locals,
  `kiln_oven::automatic_referent`) is not recorded: an LLVM constant
  cannot hold an alloca's address, so the initializer is emitted at run
  time and uses load the variable, as clang's `ConstantEmitter` falls back
  (WP-467, `tests/conformance/expr.const/constexpr-ref-automatic-run.cpp`).
  An array initializer list (or parenthesized list) of trivially
  copyable elements whose explicit elements are more than 16 bytes is a
  candidate too, folded with `Oven::fold_rvalue` (clang
  `AggExprEmitter::EmitArrayInit` trying `tryEmitForInitializer`; WP-1008,
  "Constant emission of initializers" in
  `spec/landed/codegen/WP-1008.md`). A list nested in an initializer
  that is already folded or tried as a whole, or in a list that folds,
  is not folded again.
- `builtins.rs`: builtin calls (§3, "Calls"; WP-088).
- `consts.rs`: turns Oven byte images into `kiln_llvm` constants. It has a
  memory form (bool is `i8`) and a scalar form (bool is `i1`), plus string
  literal globals. The relocation of a field and the active members of a
  union are looked up in the image by offset with a binary search, so
  lowering an image is O(F log R) in its F fields and R relocations, not
  O(F x R) (WP-665). This relies on the canonical form
  `ConstStore::push_aggregate` enforces for every image (`ByteImage`):
  relocations strictly increasing in offset, union members strictly
  increasing in (offset, field).
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
  `CodegenOptions::outline_atomics` is `Some(false)` (`-mno-outline-atomics`,
  WP-226; `tests/codegen/no_outline_atomics.cpp`), or
  `"+fp-armv8,+neon,+v8a,-fmv"` when it is `None` (no flag and no GCC
  9.3.1+ installation, spec/driver.md; WP-808); on x86-64
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
   emitted at its `HandleTopLevelDecl` (clang `HasSideEffects` with
   possible effects: a call, and a read of a volatile glvalue,
   [intro.execution]/7, so LLVM's `static int AnchorDest = AnchorSource;`
   with `extern volatile int AnchorSource;` is emitted; WP-801).
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

   A variable definition Sema instantiated at a reference while parsing
   (one usable in constant expressions, spec/sema-templates.md §4.6
   "Points of instantiation") is held the same way and replayed by the
   top-level walk at the declaration `TemplateTables::ref_var_insts`
   names (`Cg::ref_vars`, `emit_ref_vars`): before the namespace-scope
   declaration containing the reference, or after the anchor when the
   reference was in a non-member declaration (a `static_assert`); any
   the walk did not reach are replayed after it (WP-490). This is clang's
   `PassToConsumerRAII` handing the definition to `HandleTopLevelDecl`
   before the enclosing declaration: a must-emit specialization (dynamic
   initialization) gets its global, guard and `__cxx_global_var_init`
   before the function that names it, interleaved with the TU's own
   dynamic initializations in clang's order; a constant-initialized one
   is still deferred to its first use
   (`tests/codegen/var_template_point_of_instantiation.cpp`).
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
    integer to `fp128` (round to nearest even);
  - `__fp16` (clang `isHalfType`, never native in kiln) converts to and
    from integers through `float` (`fpext` then `fpto[su]i`; `[su]itofp`
    to `float` then `fptrunc`), and `++`/`--` on it extend to `float`,
    add ±1.0 and truncate; sema already promotes it to `float` in
    arithmetic (spec/sema-conv.md), `_Float16` increments stay in `half`.
- Half precision (`_Float16` and `__fp16` are `half`;
  `float16_arith.cpp`, `fp16_storage.cpp`):
  - x86_64 evaluates `_Float16` with excess precision (clang
    `ScalarExprEmitter::EmitPromoted`): `+ - * /` and unary `-`/`+`
    keep their `_Float16` operands in `float` through a whole expression
    tree and `fptrunc` the result once; a compound assignment computes in
    `float`. AArch64 operates on `half` directly. Complex `_Float16` is
    not promoted.
  - `__bf16` (`bfloat`; P1467 `std::bfloat16_t`, WP-361) is evaluated
    with excess precision on **both** targets, as clang 19/22
    `shouldEmitBFloat16WithExcessPrecision` with the default
    `-fbfloat16-excess-precision=standard`: the same `EmitPromoted`
    shape as x86_64 `_Float16` (`Cg::f16_excess`), with one `fptrunc`
    to `bfloat` at the end; comparisons compare `bfloat` operands
    directly. An explicit `__bf16` ↔ `_Float16` conversion (P1467
    [expr.static.cast]/11; clang has none) goes through `float`
    (`fpext` then `fptrunc`), exact one way and singly rounded the
    other. `_Float32`, `_Float64` and `_Float128` lower exactly as
    `float`, `double` and `fp128` (`__float128`). Test:
    `tests/codegen/bfloat16_arith.cpp`.
  - Constant operands of `half`, `bfloat`, `fp128` and `x86_fp80`
    arithmetic and casts fold as LLVM's `ConstantFolder` does, through
    `kiln_oven::apfloat` (the Oven's correctly rounded soft-float):
    `fadd/fsub/fmul/fdiv`, `fpext`/`fptrunc` with either side outside
    `float`/`double`, `[su]itofp` to `half`/`bfloat`, `fpto[su]i` from
    `half`, and unary `-` (a sign-bit flip). NaN results fold as APFloat
    computes them (WP-302, `tests/codegen/float_nan_fold.cpp`): a NaN
    operand is the result, quieted, sign and payload kept (the left one
    when both are NaN, signaling or not; `fsub` does not flip its sign);
    an invalid operation gives the positive default quiet NaN; a NaN
    `fpext`/`fptrunc` keeps its sign, shifts the significand by the
    precision difference (truncating) and sets the quiet bit (and x87's
    integer bit). The fold prepass evaluates with
    `Oven::set_fold_float_ub(true)`, so a global initialized by a NaN- or
    infinity-producing division (`0.0 / 0.0`, `1.0 / 0.0`) is a constant
    rather than a dynamic initializer, as clang's (WP-410,
    `tests/codegen/nan_global_init.cpp`). It also sets
    `Oven::set_fold_leaks(true)` (WP-1108): a fold outside a constant
    context may end with allocations still live, as long as its value
    does not point at one, since clang's `CheckMemoryLeaks` is only a
    `CCEDiag` there. So `(bool)new T` folds to `true` when clang's
    `VisitCXXNewExpr` evaluates it (a replaceable global allocation
    function, the nothrow form or the reserved placement form, and an
    initializer that evaluates): `if (new int(4))` emits no allocation,
    `?:`/`&&`/`||`/`!` drop the dead side, and `static bool b = new
    int;` is the constant `true` (`tests/codegen/new_fold.cpp`,
    `tests/conformance/expr.new/new-condition-fold.cpp`). A constant
    expression ([expr.const]/5.20) and sema's evaluations keep the leak
    check.
  - `fptosi`/`fptoui` of a constant (any format) fold toward zero; a NaN,
    an infinity or a value outside the destination's range is `poison`
    (`APFloat::convertToInteger` reporting `opInvalidOp`).
- Arithmetic:
  - signed add, sub and mul get `nsw`; shifts mask nothing at -O0;
  - a shift whose count has another integer type is `zext`/`trunc`ed
    (never sign-extended) to the left operand's type first, element-wise
    for a vector whose count is the splatted scalar (`v2u64 << int`:
    `<2 x i32>` to `<2 x i64>`; clang `EmitShl`/`EmitShr`, WP-811,
    BoringSSL's Keccak rotate;
    `tests/conformance/ext.vector/shift-scalar-count-other-width-run.cpp`);
  - pointer arithmetic: `gep inbounds` (`gep i8` for void and function
    pointers); pointer difference: `ptrtoint`, `sub`, then `sdiv exact`;
  - floating add/sub (also compound) contract a single-use `fmul`
    operand (looking through a single-use `fneg`) into
    `llvm.fmuladd.*`, as `tryEmitFMulAdd` does under `fp_contract(on)`
    (the default; §3.2 for `off` and `fast`):
    `b*c + a` / `b*c - a` are `fmuladd(b, c, a)` / `fmuladd(b, c, -a)`,
    `a + b*c` / `a - b*c` are `fmuladd(b, c, a)` / `fmuladd(-b, c, a)`
    (the addend of a right product is never negated), and a looked-through
    fneg flips the sign of `b` (tests/codegen/fmuladd_operand_order.cpp).
- Values:
  - an lvalue-to-rvalue conversion of a glvalue `?:` is emitted as a
    scalar `?:`: each arm loads in its own block and the result is a phi;
  - an lvalue-to-rvalue conversion of a reference-returning call, also
    through the lvalue `CK_UserDefinedConversion` of a conversion
    function returning a reference (`operator bool&()`), loads from the
    returned address (clang `EmitLoadOfLValue`), and so does one of a
    glvalue that clang's scalar visitor passes through to such a call:
    an implicit or explicit glvalue `CK_NoOp` cast
    (`static_cast<const T&>(f())`, `(const T&)f()`, `const_cast<T&>(f())`),
    the right operand of a glvalue comma and the chosen operand of
    `__builtin_choose_expr`; a glvalue `CK_LValueBitCast`
    (`reinterpret_cast<const U&>(e)`) loads its operand's address as `U`
    (spec/landed/codegen/WP-632.md);
  - `&&`/`||` produce an `i1` phi; `EmitBranchOnBoolExpr` splits
    `&&`/`||`/`!`/`?:` conditions into branches and folds constants;
  - bit-fields load the storage unit, then `shl`/`ashr` (signed) or
    `lshr`/`and` (unsigned); stores mask, clear and `or` into the unit;
  - a constructor's initialization of a bit-field member (a mem-init
    or a default member initializer `bool b : 1 {false};`,
    [class.base.init]/9) is stored through the storage unit the same
    way: `field_lvalue_init` defers a bit-field to `field_lvalue`, as
    clang `EmitLValueForFieldInitialization` does (WP-286;
    `tests/codegen/bitfield_member_init.cpp`,
    `tests/conformance/class.base.init/bitfield-default-member-init-run.cpp`);
  - `++`/`--` on bool, integers, floats and pointers, including bit-fields;
  - ext-vector element access (clang `EmitExtVectorElementExpr`, WP-312;
    `tests/codegen/ext_vector_element.cpp`): the base vector's lvalue
    (a prvalue base is stored to a temporary first; a glvalue access
    chain `v.even.y` composes its lanes onto the root vector). One lane
    is a vector-element lvalue (`VecLane::Index`, `extractelement`/
    `insertelement` with an `i64` index); several are a
    `VecLane::Swizzle` lvalue: a load is a one-operand `shufflevector`
    of the loaded vector, a store (`store_swizzle`, clang
    `EmitStoreThroughExtVectorComponentLValue`) shuffles the value into
    the loaded vector, widening it first when the vector is longer than
    the swizzle, and stores the whole vector back.
  - a braced initializer of a vector type (clang `VisitInitListExpr`):
    each scalar element an `insertelement` into `poison` (`i32` index),
    the remaining lanes zeros; an element that is itself a vector (the
    whole initializer `v16 b{a}` of a GNU vector of its type) is widened
    to the result's length by a one-operand `shufflevector`, then
    shuffled in after the lanes already set (WP-792;
    `tests/conformance/ext.vector/ir-init-list-vector-element-*.cpp`);
- Variable alignment (clang `getDeclAlign`, WP-312;
  `tests/codegen/alignas_variable.cpp`): `var_align` is
  `kiln_abi::layout::decl_align` (spec/abi.md §3.2), so an `alignas` or
  `aligned` attribute sets the `align` of a global, a static local, a
  local's or `catch` variable's `alloca`, and of the loads and stores
  through them.
- Class B attributes and their flags (WP-813, `class_b.rs`): the
  hardening, instrumentation, inlining and pointer-fact attributes and
  `CodegenOptions::hardening` add clang 19's function, parameter and
  call-site attributes, `!callback` and loop metadata, `llvm.assume`
  align bundles and module flags at the points clang adds them
  (`ConstructAttributeList`, `SetLLVMFunctionAttributesForDefinition`,
  `StartFunction`, `EmitFunctionProlog`, `EmitCall`); the per-attribute
  rules are spec/landed/codegen/WP-813.md
  (`tests/codegen/class_b_*.cpp`).
- Calls:
  - a call site gets `nounwind` only when the callee's prototype is
    nothrow, or the direct callee has a `NoThrow` attribute (a
    recognized C library function, spec/sema.md §5.4) or a `const` /
    `pure` attribute; the declaration and a definition get it too;
    a callee expression that folds to a function (a constant
    `int (&)(int)` naming one) is that function for these checks and
    for `TryMarkNoThrow`, as clang's `CalleePtr` (WP-432,
    `fn_ref_callee_nounwind.cpp`);
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
    functions recognized by name (spec/sema.md §5.4) stay ordinary calls,
    except the ones clang's `EmitBuiltinExpr` lowers through their
    library builtin id: `memcpy` / `memmove` / `memset`, `abs` / `labs` /
    `llabs`, and `fabs`, `ceil`, `floor`, `trunc`, `round`, `rint`,
    `nearbyint`, `copysign`, `fmax`, `fmin`, `fma` with their `f` / `l`
    forms (clang's `GenerateIntrinsics` set; `sqrt` stays a call under
    `-fmath-errno`, §3.2; WP-419's `roundeven` / `fmaximum_num` / `fminimum_num`
    below) are emitted as their `__builtin_` twin
    (`DeclArena::lib_builtin`, `builtins::lib_intrinsic`; WP-186,
    `tests/codegen/builtins_lib_intrinsic.cpp`). WP-1073 adds clang
    `EmitBuiltinExpr`'s inline lowering of: `alloca` (GNU; a dynamic
    `alloca i8` of the operand, aligned to `__BIGGEST_ALIGNMENT__`), `mempcpy`
    (GNU; `llvm.memcpy` then the `getelementptr` past the end), `bzero` /
    `bcopy` (GNU; `__builtin_bzero` / `__builtin_bcopy`: `llvm.memset` of
    zero, and `llvm.memmove` with the source and destination swapped
    back), `finite` / `__finite` `{,f,l}` (as `__builtin_isfinite`) and `creal` / `cimag` / `conj` `{,f,l}` (the
    operand's parts, `conj` an `fneg` of the imaginary part)
    (`tests/codegen/lib_gnu_inline.cpp`, `lib_gnu_inline_std.cpp`,
    `lib_complex_parts.cpp`, `lib_alloc_size*.cpp`). `abort` stays a
    `noreturn nounwind` call (WP-938; already clang's) and the
    `returns_twice` functions (`setjmp` ...) carry `returns_twice` on
    the declaration and the call (`builtins::returns_twice`, WP-807;
    already clang's, confirmed by WP-1073's
    `ext.gnu.returns_twice/setjmp-modified-local-run.cpp`). Not done (follow-ups):
    clang's -O0 fold of a library call the constant evaluator accepts
    (`strlen("abc")` is `i64 3`, `memcmp` of literals), clang 22's
    `modf` to `llvm.modf`; `-fno-builtin` / `-fno-builtin-<name>` and
    the `no_builtin` attribute are "no-builtin" below;
  - a variadic call prints its function type;
  - an indirect (sret) return writes into the destination slot when the
    call initializes an aggregate, otherwise into a temporary; an
    ignored aggregate call makes that temporary itself, after the
    arguments, and an empty-class (`Ignore`) result still gets an unused
    temporary (clang `GetUndefRValue`; WP-172). A discarded
    `ExprWithCleanups` around such a call (`M{}.e(E{}, 3);`) keeps the
    ignored slot inside a cleanup scope (clang
    `AggExprEmitter::VisitExprWithCleanups`), so there is no extra
    result temporary ahead of the object and argument temporaries
    (WP-777; `tests/codegen/discarded_member_call_on_temporary.cpp`,
    `tests/conformance/expr.call/run-discarded-member-call-on-temporary-order.cpp`).
    A discarded comma whose operands are such calls (`M{}.b(), M{}.b();`)
    ignores each operand the same way (clang `AggExprEmitter::VisitBinComma`),
    so each result temporary is made in order and none is made for the
    comma itself; chained calls, calls through returned references,
    temporary arguments and discarded conditionals are covered too (WP-774;
    `tests/codegen/discarded_member_call_chain.cpp`,
    `tests/conformance/expr.call/run-discarded-member-call-chain-order.cpp`);
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
      `abs` / `labs` / `llabs` are candidates only at
      `-fclang-compat=22` (clang 20 made them constant; WP-432,
      `builtins_abs_22.cpp`);
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
      prepass collects it); `__builtin_expect` and
      `__builtin_expect_with_probability` at -O0 are their first operand
      (the expected value emitted, the probability not), above -O0
      `llvm.expect.T(value, expected)` and
      `llvm.expect.with.probability.T(value, expected, double P)` with
      the probability folded (WP-492, `builtin_expect_o1.cpp`);
      `__builtin_unpredictable(x)` is `x`, and above -O0 (clang
      `EmitBranchOnBoolExpr` / `EmitSwitchStmt`) a conditional branch
      whose condition, parentheses stripped and then implicit conversions
      (`IgnoreImpCasts`), is the call carries `!unpredictable !{}`: an
      `if`, a `?:` emitted as branches, an `&&`/`||` operand and a `!`
      operand do, a parenthesized call under a conversion
      (`if ((__builtin_unpredictable(b)))`), a loop condition and a
      `select` do not; a `switch` does when its condition is the call
      itself (WP-779, `builtin_unpredictable_o1.cpp`); signbit (and WP-419's `signbitf` / `signbitl`, through
      `builtin::f128_twin`) bitcasts to an integer of the
      float's width and tests `slt 0`, and a constant argument folds
      (clang 19 and 22; `builtins_signbit_fl.cpp`); the isgreater family is one
      `fcmp` (`ogt oge olt ole one uno`) zero-extended;
    - memcpy/memmove are `llvm.memcpy`/`llvm.memmove` (`i64` length,
      not volatile) with each pointer's known alignment as a parameter
      `align` (clang `EmitPointerWithAlignment`, §3.3);
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
      call site too). The library function and a program's own
      declaration of it (`extern "C" double pow(double, double)`, or
      `strlen` after `__builtin_strlen`; separate declarations to sema)
      share one `@pow` / `@strlen` in either order, by the same-type reuse
      of a mangled name above, never a uniqued `@pow.1` / `@strlen.1` that
      no library defines (WP-288, WP-286;
      `tests/codegen/lib_builtin_shared_decl.cpp`,
      `tests/codegen/builtins_lib_redecl.cpp`,
      `tests/conformance/dcl.link/libcxx-pow-run.cpp`,
      `tests/conformance/expr.const/builtins-lib-redecl-run.cpp`).
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
      Deviation (WP-352): a signed `mul` whose encompassing width is
      above 64 bits is not `llvm.smul.with.overflow` (LLVM lowers that to
      compiler-rt's `__muloti4`, which libgcc, linked by the GNU driver,
      lacks on aarch64) but its inline expansion: each operand's sign
      (`slt 0`) and magnitude (`select` of `sub 0, x` and `x`), `umul`
      of the magnitudes, `neg = xor` of the signs, overflow `uo or
      (ur ugt INT_MAX + zext neg)`, value `select neg, (sub 0, ur), ur`
      (`checked.rs` `wide_signed_mul`; `crates/kiln_codegen/tests/lower.rs`
      `wide_signed_mul_overflow_is_inline`,
      `tests/conformance/ext.gnu.builtins/overflow-i128-smul-run.cpp`).
      The value is stored through `EmitToMemory` (an `i1` widened),
      volatile per the pointee. A `select` whose arms are not both
      constants is not folded (IRBuilder `FoldSelect`). The calls are not
      fold candidates, since they store
      (`tests/codegen/builtins_overflow.cpp`);
    - the carry builtins `__builtin_{add,sub}c{b,s,,l,ll}(x, y, ci, co)`
      (WP-557, clang `BI__builtin_addcb`..) emit `x`, `y`, `ci`, then
      the carry-out pointer, then two `llvm.u{add,sub}.with.overflow`
      calls (`x op y`, then that sum `op ci`); the `or` of the two flags,
      `zext` to the operand type, is stored through the pointer and the
      second sum is the value. Not fold candidates, as they store
      (`checked.rs` `emit_carry`; `tests/codegen/builtins_carry.cpp`);
    - `__builtin_alloca(n)` is `alloca i8, i64 n` in the current block
      (not the entry block, no stack save), aligned to
      `__BIGGEST_ALIGNMENT__` (16); `__builtin_alloca_with_align(n, a)`
      to `a / 8` (`tests/codegen/builtins_alloca.cpp`);
    - `__builtin_vsnprintf`, `__builtin_snprintf`, `__builtin_sprintf`
      are library builtins (`tests/codegen/builtins_printf_lib.cpp`), as
      are `__builtin_printf`, `__builtin_vprintf`, `__builtin_vsprintf`
      and the `scanf`, `sscanf`, `vscanf`, `vsscanf` forms (WP-841,
      `tests/conformance/ext.gnu.builtins/printf-scanf-family-run.cpp`).
    - the `__builtin_` C library aliases of WP-1104 (`builtin::is_libc_fwd`;
      spec/sema.md "Library functions"): `__builtin_malloc`, `calloc`,
      `realloc`, `free`, `fprintf`, `vfprintf`, `fscanf`, `vfscanf` and
      the string functions are calls of the library function without the
      prefix (`builtins::lib_name`), declared with the builtin's type, so
      the `NoThrow` ones are `nounwind` and a call under a pending
      cleanup is a `call`; no `allocsize` and no `noalias` return, as
      clang. `__builtin_alloca_uninitialized` and
      `__builtin_alloca_with_align_uninitialized` are emitted as
      `__builtin_alloca` / `__builtin_alloca_with_align` (`emit_alloca`),
      never initialized under `-ftrivial-auto-var-init` (clang's
      `memset` of `__builtin_alloca` under that flag is not modelled
      yet: WP-1104 follow-up). `tests/codegen/lib_builtin_fwd.cpp`,
      `ext.gnu.builtins/library-fwd-*-ir.cpp`,
      `alloca-uninitialized-ir.cpp`, `library-fwd-alloc-scanf-run.cpp`.
    - recognized C library functions (WP-419, clang `EmitBuiltinExpr`
      for a library builtin; spec/sema.md §5.4): a direct call of a
      declaration with a `lib_builtin` twin whose `__builtin_` form is an
      LLVM intrinsic (`ceil`, `floor`, `trunc`, `round`, `rint`,
      `nearbyint`, `roundeven`, `fabs`, `copysign`, `fma`, `fmax` /
      `fmin` -> `llvm.maxnum` / `llvm.minnum`, clang 22's `fmaximum_num`
      / `fminimum_num`, each `{,f,l}`) is emitted as the twin
      (`builtins::lib_intrinsic`) at every `-O` level, a definition of
      the function included; the call is never folded (clang does not
      constant-evaluate the library names). Under clang's default
      `-fmath-errno` the errno-setting ones (`sqrt`, `sin`, `exp`, ...)
      stay library calls; under `-fno-math-errno` they lower too (§3.2)
      (`tests/codegen/lib_float_intrinsics.cpp`, `_gnu.cpp`).
    - library builtins (WP-282, clang `EmitBuiltinExpr`):
      `__builtin_assume_aligned(p, a[, off])` returns `p` and emits
      `llvm.assume(i1 true) ["align"(ptr p, i64 a[, i64 off])]`
      (`emitAlignmentAssumption`; the call is folded first so a dependent
      alignment is a constant; `builtins_assume_aligned.cpp`).
      The alignment builtins (WP-416, clang `EmitBuiltinAlignTo`): the
      alignment is zero-extended or truncated to the source type (i64 for
      a pointer) and `mask = align - 1`; `__builtin_align_up` adds the mask
      (`getelementptr inbounds i8, p, mask` for a pointer), then both
      ands with `~mask` (`llvm.ptrmask.p0.i64` for a pointer);
      `__builtin_is_aligned` is `icmp eq (and src, mask), 0` (`ptrtoint`
      first for a pointer). Constant operands fold as IRBuilder folds them
      (`x + 0` is kept); an integer call is a fold candidate.
      `__builtin_debugtrap()` is `call void @llvm.debugtrap()`,
      `__builtin_frame_address(n)` `call ptr @llvm.frameaddress.p0(i32 n)`,
      `__builtin_return_address(n)` `call ptr @llvm.returnaddress(i32 n)`
      (WP-432, `builtins_return_address.cpp`).
      `__builtin_readcyclecounter()` is `call i64 @llvm.readcyclecounter()`
      on every target (WP-1057, `builtin_readcyclecounter{,-19}.cpp`).
      `__builtin_thread_pointer()` is `call ptr @llvm.thread.pointer()`,
      `@llvm.thread.pointer.p0()` when the consuming LLVM
      (`CodegenOptions::llvm_major`) is 20 or later, where the intrinsic
      became address-space overloaded (an LLVM 19 backend leaves the
      `.p0` name an undefined symbol), declared
      `memory(none)` (WP-782, `builtins_thread_pointer.cpp`).
      The `_FORTIFY_SOURCE` builtins `__builtin___X_chk` are calls of the
      library function `__X_chk` with the same arguments, except that
      `__builtin___memcpy_chk` / `memmove_chk` / `memset_chk` whose size
      and object size are integer constants with size <= object size
      (unsigned) are `__builtin_memcpy` / `memmove` / `memset` (clang
      `EmitBuiltinExpr`; WP-803, `ext.gnu.builtins/fortify-chk-run.cpp`).
      `__builtin_prefetch(p, rw, loc)` is `call void
      @llvm.prefetch.p0(ptr p, i32 rw, i32 loc, i32 1)` (defaults `rw` 0,
      `loc` 3; the trailing 1 selects the data cache), the intrinsic
      declared `nocallback nofree nosync nounwind willreturn
      memory(argmem: readwrite, inaccessiblemem: readwrite)` with a
      `nocapture readonly` (22: `readonly captures(none)`) pointer and
      three `immarg` operands (WP-610,
      `builtin_prefetch.cpp`); `__builtin_memcpy_inline(d, s, n)` is
      `call void @llvm.memcpy.inline.p0.p0.i64(ptr align A d, ptr align B
      s, i64 n, i1 false)` (declared like `llvm.memcpy`), and
      `__builtin_assume_separate_storage(a, b)` is `call void
      @llvm.assume(i1 true) [ "separate_storage"(ptr a, ptr b) ]` (WP-726,
      `builtin_memcpy_inline.cpp`); `__builtin_ia32_pause()` is `call void
      @llvm.x86.sse2.pause()`, declared `nounwind`
      (`target.x86/ia32-pause-ir{,-19}.cpp`); the x86 builtins in
      general are spec/landed/codegen/WP-630.md.
      `__builtin_elementwise_add_sat` / `_sub_sat` are `llvm.sadd.sat` /
      `uadd.sat` / `ssub.sat` / `usub.sat` by element signedness (a
      `bool` operand as unsigned), folded at 22 when the Oven folds them
      (WP-432, `builtins_elementwise_sat{,_22}.cpp`). The alignment
      builtins on a pointer to an object of known alignment fold in a
      global initializer (spec/oven.md "Builtins"; `builtins_align_ptr.cpp`).
      `__builtin_elementwise_abs` is `llvm.abs(x, i1 false)` for
      integers and `llvm.fabs` for floating point, `_max` / `_min`
      `llvm.smax` / `umax` / `smin` / `umin` by element signedness and
      `llvm.maxnum` / `minnum` for floating point, overloaded on the
      (vector) operand type; at 22 a call the Oven folds is folded
      (`builtins_align_elementwise.cpp`, `builtins_elementwise_22.cpp`).
      The driver sets the Oven's compatibility level before codegen's
      fold prepass, so a unit in which sema evaluated nothing folds at
      its level.
      `__builtin_object_size` / `__builtin_dynamic_object_size` of an
      object the frontend does not know is `llvm.objectsize(p, min =
      type & 2, nullunknown = true, dynamic)`, type 3 the constant 0;
      a constant-foldable call is its folded value (the Oven's
      speculative evaluation, spec/oven.md, so a local array folds), and
      an operand with side effects (a call included) that does not fold
      is the default (-1 for types 0/1, 0 for 2/3) without being emitted
      (clang `emitBuiltinObjectSize`; `builtins_object_size.cpp`).
      A `pass_object_size(t)` parameter (spec/sema.md §4) adds a hidden
      `size_t` argument right after it in the IR signature
      (`module::with_object_sizes`, applied by `decl_sig` and at direct
      calls, member, operator and constructor calls included); the caller
      passes, in order (clang `MaybeEmitImplicitObjectSize`): the Oven's
      fold of `__builtin_object_size(arg, t)` at the call site
      (`Oven::fold_object_size`, run by the fold prepass
      `Collect::ps_args` -> `Folds::obj_size`; no side-effect default, a
      failure is not an error), for `pass_dynamic_object_size` too; the
      caller's own hidden value when the argument is a compatible
      `pass_object_size` parameter (types equal, 0 serving 1, 3 serving
      2); 0 for type 3; else `llvm.objectsize(arg, t & 2, true, dynamic)`.
      In the callee
      `__builtin_object_size(param, t)` of a compatible type loads the
      hidden value (`Fx::object_sizes`; clang `emitBuiltinObjectSize` /
      `EmitCallArgs`; WP-803, WP-808,
      `ext.clang.pass.object.size/pass-object-size-run.cpp`,
      `ir-pass-object-size-fold.cpp`). Not yet (WP-808 follow-up,
      `pass-object-size-lenient-xfail.cpp`): clang's lenient designator
      (an out-of-bounds or negative offset gives 0, a reinterpreting
      cast keeps the byte offset, a member of an unknown base keeps its
      designator); those arguments take `llvm.objectsize`, which only
      weakens a check at -O0.
      `__builtin_powi*` is
      `llvm.powi.fT.i32`; `__builtin_copysignf16` truncates to `half`,
      calls `llvm.copysign.f16` and extends back;
      `__builtin_verbose_trap` is `llvm.trap` (clang uses the strings
      only for debug info) (`builtins_powi.cpp`).
      `__builtin_shufflevector` (a `ShuffleVectorExpr`) is one
      `shufflevector` with a constant mask, -1 a `poison` lane
      (`builtins_shufflevector.cpp`). The two-argument mask form is
      clang's `EmitShuffleVector` loop (`shuffle_by_mask`): the mask is
      ANDed with a splat of `NextPowerOf2(n - 1) - 1`, then each lane is
      an `extractelement` from the mask and from `v1`, then an
      `insertelement` into a `poison` result. A `*f128` builtin with a `long
      double` twin (`builtin::f128_twin`) is lowered as the twin on its
      `fp128` operands (intrinsic or constant); `roundevenf128` is
      `llvm.roundeven`; the others call the function of the builtin's
      own name (`builtins_f128.cpp`). `roundeven{,f,l}`,
      `canonicalize{,f,l}` and clang 22's `fmaximum_num{,f,l}` /
      `fminimum_num{,f,l}` are `llvm.roundeven`/`canonicalize`/
      `maximumnum`/`minimumnum` of the operand type; `exp10{,f,l}` and
      clang 22's `sincos{,f,l}` call the library function of that name
      (WP-410; `builtins_c_math.cpp`, `builtins_c_math_22.cpp`). WP-419:
      clang 22's `fmaximum_numf128` / `fminimum_numf128` lower as their
      `l` twins (`llvm.maximumnum` / `minimumnum` of `fp128`), and
      `sincosf128` and `sincospi{,f,l}` call the function of that name
      (`builtins_c_math_more_22.cpp`). A
      `*f16` builtin (WP-403) is
      lowered likewise through `builtin::f16_twin` on `half` operands:
      `ceil`/`floor`/`trunc`/`round`/`rint`/`fabs`/`fma`/`fmax`/`fmin`
      as their `llvm.*.f16` intrinsics, `roundevenf16`,
      `canonicalizef16`, `fmaximum_numf16`, `fminimum_numf16` as
      `llvm.roundeven`/`canonicalize`/`maximumnum`/`minimumnum`,
      `frexpf16` as `llvm.frexp.f16.i32`, the libm set as calls of the
      `half` function the builtin names (`sinf16`, `ldexpf16`, ...)
      (`builtins_f16_libm.cpp`; clang 22's `atan2f16`, `sincosf16`,
      `fmaximum_numf16`, `fminimum_numf16` in `builtins_f16_libm_22.cpp`).
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
    pointer to the string literal's global (the file spelled as
    `-fkiln-source-path-style=` selects, spec/driver.md);
    `__builtin_source_location()`
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
  linkage name (`kiln_abi::mangle::asm_label`); an instantiated
  function or variable, which has no recorded label, takes the
  `AsmLabel` attribute sema copied from its pattern (WP-449,
  `tests/codegen/asm_label_member.cpp`). **Known gap:** a block-scope
  `extern` function declaration in a template is instantiated as a
  local entity (mangled `_ZZ…E1qi`, its label dropped). clang prefixes LLVM's
  `\01` "no global prefix" marker only when the target's user-label
  prefix is non-empty (Mach-O); every kiln target is ELF, so the label
  is the symbol verbatim, and a `\01` the user wrote is kept (LLVM
  strips it when emitting).
- Local initialization (`EmitAutoVarInit`):
  - a constant aggregate goes through `emitStoresForConstant`. That means
    a single store, memset-0 plus a few stores, a bytewise memset, or a
    memcpy from a `@__const.<fn>.<var>` private global, in that order of
    preference. The global is clang's `createUnnamedGlobalFrom`
    (`Cg::unnamed_global_from`, WP-1144): `<fn>` is the declaring
    function's symbol, or its class name for a constructor or destructor
    (`~S`; clang `getNameAsString`, no template arguments); one global is
    cached per variable and reused while its constant is the same
    (raising its alignment), so a lambda or local class that names the
    variable without odr-using it ([basic.def.odr]/5,
    `Cg::constant_local_lvalue`; clang `EmitDeclRefLValue` when
    `canEmitSpuriousReferenceToVariable` fails) reads the same global, or
    creates it when the variable's own initialization did not. Test
    `basic.def.odr/potential-results-lambda-ir.cpp`. Constants are never promoted to globals at -O0. An
    aggregate is constant when it is constexpr, or when it is POD and
    its initializer folds. The exception is a trivial copy or move of
    an empty class, which reads nothing and so folds whatever its source
    is. Like clang's `isConstantInitializer`, it is constant only when
    its source names a constexpr variable, looking through casts and
    the std builtins that are their argument's lvalue (`std::move(ce)`,
    `forward`, `as_const`, `move_if_noexcept`; WP-289). `E e2 =
    std::move(e)` of a non-constexpr `e` emits nothing;
  - a non-constant aggregate goes through `EmitAggExpr`, with
    `CheckAggExprForMemSetUse` (size > 16 and nonzero*4 <= size gives a
    memset);
  - `EmitArrayInit`: an array list of trivially copyable elements whose
    explicit elements are more than 16 bytes and fold is one memcpy from
    a `private constant` `@constinit` global (WP-1008). Otherwise element
    0 is stored at the base, element i at `gep inbounds elem, base, i64
    i`, and a phi filler loop covers elements without an initializer.
    The full decision rules, from `EmitAutoVarInit` to
    `EmitCXXConstructExpr`, are in `spec/landed/codegen/WP-1008.md`.
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
- **GEP flags (WP-386).** Every address computation is
  `getelementptr inbounds`. At `-fclang-compat=22` (clang 20+, which
  emits `nuw` whenever the offset is known non-negative) `FnCx::gep_nuw`
  adds `nuw` to: member access (`struct_gep`: a field index, instruction
  or constant expression), an array subscript or pointer `+ n` whose
  index has unsigned type ([expr.sub]/2, [expr.add]/4; in either operand
  order), and `++p` / `p++` (a step of `+1`). A signed index, `p - n`,
  `--p` / `p--` keep plain `inbounds`, as clang's
  `EmitCheckedInBoundsGEP` (`isSigned` / `isSubtraction`). Below 22 no
  `nuw` (clang 19). LLVM 19 parses `nuw`, so this is keyed on the compat
  level only. The GEPs built outside these helpers match clang 22 too
  (checked against it, WP-402): array initialization loops (`add nuw`
  index, `inbounds nuw` element), `_Complex` parts and `va_list` fields
  (struct GEPs), and VLA steps (below).
- **Pointers to variable length arrays (WP-402).** clang
  `emitPointerArithmetic`: `p + n` / `p - n` on a pointer to a VLA type
  scales the `i64` index (negated first for `-`) by the VLA's element
  count (`mul nsw`, `Cg::vla_pointer_step`) and steps over its first
  non-variable element type; `++p` / `--p` step by that count (`sub nsw
  0, n` for `--`, `Cg::vla_inc_step`); `p - q` divides the byte
  difference by the VLA's size in bytes (`Cg::vla_bytes`, `mul nuw` of
  the element size and count, as `sizeof`). The GEP flags follow the
  rules above (an unsigned index or `++` gets `nuw` at compat 22).
- **Reference loads (WP-402).** clang 22 `EmitLoadOfReference`: at
  `-fclang-compat=22` the load of a reference's address from its slot
  (`Cg::load_reference`: a local, parameter, global or static reference
  variable, a reference member, so also a closure's by-reference capture,
  a range-for `__range` and a structured binding's reference) carries
  `!nonnull !{}` unless the referee is `void`, an array of unknown bound
  or an incomplete class, and then `!align !{i64 A}` when the referee is
  an object whose alignment (`pointee_layout`: the non-virtual alignment
  of a non-final class) exceeds 1. Below 22 neither (clang 19). Metadata
  nodes are numbered in first-use order after the module flags and
  ident, as clang prints them. A reference parameter forwarded by a
  delegating call (an inheriting constructor, a lambda's static invoker,
  a thunk: `Cg::delegate_call_arg`, clang `EmitDelegateCallArg`) is a
  plain `load ptr` of its slot with neither, at every compat level
  (WP-416).
- **Captured locals (WP-581).** A `DeclRefExpr` marked
  `REFERS_TO_ENCLOSING` names the closure's member for the capture
  (`captured_var_lvalue`: a by-copy capture is the field, a by-reference
  one, and a captured reference, is the field's loaded address), in every
  function emitted from the lambda: each specialization of a generic
  lambda's call operator, and the call operators of the closures formed
  again when an enclosing generic call operator is instantiated. Codegen
  never looks a captured entity up in the emitting function's local table;
  "reference to an unemitted local variable" for one is a sema bug (a
  capture that was never recorded), not a codegen case. Sema guarantees
  the member exists by capturing in the template definition every
  potential odr-use whose full-expression depends on a lambda parameter
  (spec/sema-lambda.md §4.2). Minimal repro (failed before WP-581):
  `bool f(const int& r) { const int& rr = r; return [&](const auto& x) {
  return x == *reinterpret_cast<const int*>(&rr); }(r); }`.
- **Pointee alignment of an array of unknown bound (WP-416).** clang
  `getTypeAlignInChars` gives `T[]` its element's alignment, so
  `pointee_layout` reports `(no size, align of T)`: an `int(&)[]` / `int
  (*)[]` parameter is `nonnull align 4` (no `dereferenceable`) and loads
  through it are `align 4`. A function type stays alignment 1.
- **Zeroed array fillers (WP-416).** clang `EmitArrayInit` passes
  `Dest.isZeroed()` to the filler's `EmitInitializationToLValue`: when
  the whole array was zeroed first (`Agg a[2] = {}`, one `memset` of the
  array), the filler loop skips the per-element zeroing of a
  value-initialized aggregate element, so no per-element `memset`.
- **Constant external declarations (WP-416).** clang
  `GetOrCreateLLVMGlobal` sets `isConstantStorage(Ctx, false, false)` on
  a variable's declaration (`Cg::get_gvar`, `constant_storage`): a
  reference (`extern int &r` is `external constant ptr`) or a `const`
  object whose base element type is not a class (`extern const int`,
  `extern int *const`, `extern const int[2]`) is `external constant`; a
  `const` class object stays `external global`. A definition later in the
  translation unit sets its own constness (§2).
- **Discarded volatile reads (WP-416).** A discarded volatile scalar
  glvalue carries sema's `LValueToRValue` cast (spec/sema.md §7,
  [expr.context]/2), lowered as a `load volatile` whose value is unused.

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
  one-based source index). The library allocation functions `malloc`,
  `calloc`, `realloc`, `aligned_alloc` and (GNU) `memalign` get clang's
  implicit `alloc_size` / `alloc_align` from sema (clang
  `AddKnownFunctionAttributes`; spec/sema.md "Library functions",
  WP-1073), so codegen reads them as written attributes: `allocsize`
  `(0)`, `(0,1)`, `(1)`, `(1)`, `(1)`. (Before WP-1073 codegen matched
  the first four by name itself; the IR was already clang's.)
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

### 3.2 Floating-point semantics (WP-806, `fp.rs`)
Normative detail: spec/landed/codegen/WP-806.md ([expr.pre]/6, C23
7.12.2, [cfenv.syn] / C23 7.6). In short:
- The FP state of a scope is the command line's `FpOptions`
  (`CodegenOptions::fp`, spec/driver.md "Floating-point flags") with the
  `FpOverride` sema recorded on the enclosing compound statement
  (`#pragma STDC FP_CONTRACT`, `#pragma clang fp`, `#pragma
  float_control`, `#pragma STDC CX_LIMITED_RANGE`).
- Contraction: `on` fuses within an expression as `llvm.fmuladd`, `fast`
  puts `contract` on every FP operation, `off` emits plain operations.
- The other fast-math flags go on FP binops, `fneg`, `fcmp`, FP `select`
  / `phi`, and calls of intrinsics and library builtins (the scope's);
  other calls with an FP result take the command line's. FMF on
  `fpext`/`fptrunc` and on FP-struct calls need LLVM 20 IR
  (`Module::ir_version`); at the default LLVM 19 they are omitted.
- Function attributes `no-infs-fp-math`, `no-nans-fp-math`,
  `approx-func-fp-math`, `unsafe-fp-math`, `no-signed-zeros-fp-math`,
  `denormal-fp-math` and `no-trapping-math` follow the command line; a
  pragma scope that turns one off makes it `"false"` on the definition.
  `nofpclass` marks FP parameters and returns under finite math.
- `-fno-math-errno` makes the errno-setting libm functions `memory(none)`
  and lowers them to their intrinsics (`ldexp`, `exp10` excepted).
- The floating-point environment is the default one; every flag and
  pragma that needs another (FENV_ACCESS ON, FENV_ROUND other than
  FE_TONEAREST, trapping, rounding, a wider eval method) is an error.

### 3.3 Pointer alignment (WP-819, WP-829, WP-847, `builtins.rs` `pointer_with_alignment`)
clang `CodeGenFunction::EmitPointerWithAlignment`, mirrored case by case,
gives a pointer operand's value, its known alignment, and whether that
alignment is a declaration's (`AlignmentSource::Decl`). Its users:
- the lvalue of `*p` (`EmitUnaryOpLValue`), of `p[i]` through a pointer
  (`EmitArraySubscriptExpr`, VLA subscripts included), of `p->m`
  (`EmitMemberExpr`, `this->m` included), of a vector element through
  `->` (`EmitExtVectorElementExpr`) and the object of `p->*m`
  (`EmitPointerToDataMemberBinaryExpr`, WP-829);
- the object of a member call through `->` (`p->f()`,
  `EmitCXXMemberOrOperatorMemberCallExpr`), the operand of a scalar
  derived-to-base conversion (whose virtual step loads the vtable
  pointer at that alignment) and of `dynamic_cast` (`VisitCastExpr`,
  WP-829);
- every `__atomic_*`/`__c11_atomic_*` pointer (`atomic.rs`), memcpy/
  memmove/memset-like builtins, `__builtin_clear_padding`, `va_arg`, NEON
  loads and stores, checked arithmetic results, and the placement
  argument of a new-expression (§3.1);
- the operand of a delete-expression (`EmitCXXDeleteExpr`, WP-847): the
  vtable pointer load of a virtual (deleting or destroying-delete)
  destructor call and of `::delete`'s offset-to-top read the object at
  that alignment, and `delete[]`'s array cookie load at
  `min_align(min_align(align, cookie size), cookie offset)`. The null
  test of a constant operand (`delete &g`) folds as LLVM's constant
  folder does: `false` for a global that is not `extern_weak`.

After skipping parentheses:
- **No-op/bitcast** from a non-`void` pointer keeps the operand's
  alignment and source; an *explicit* cast whose operand's source is not
  `Decl` takes the target type's natural pointee alignment.
- **Array-to-pointer decay**: the array lvalue's alignment and source.
- **Derived-to-base**: the operand's, then the base subobject's
  (`GetAddressOfBaseClass`, null-checked unless the operand is `this`).
- **`&lvalue`**, and a call of `__builtin_addressof`, `std::addressof` or
  `std::__addressof` (clang's library builtins; one argument, a pointer
  result): the operand lvalue's alignment (a member's `alignas`
  included) and source.
- **`p + i`, `i + p`, `p - i`** (clang 22 `emitPointerArithmetic`; only
  at `-fclang-compat=22`): the base pointer's source and its alignment
  at the byte offset `zext(i) * sizeof(*p)` for a constant `i`, or at
  any element (`MinAlign(align, sizeof(*p))`) otherwise; operands are
  emitted in source order. clang 19 takes the natural alignment.
- **Anything else**: the scalar value with the natural pointee alignment
  (`getNaturalPointeeTypeAlignment`): an `aligned` attribute on the first
  typedef in the pointee's sugar wins (libc++ `atomic_ref`'s
  `_Aligned_Tp*`); `void`, functions and incomplete types are 1; a class
  uses its pointer alignment (non-virtual alignment unless `final`).

An lvalue's source is `Decl` for a named non-reference variable or
parameter, a non-reference field of any base (`.` or `->`; clang
`getFieldAlignmentSource` is always `Decl`) or static member, and for
`*p` and `p[i]`/an array element when the pointer's (array's) source is
`Decl`; otherwise (reference variables, calls, ...) it is the type's.
So libstdc++'s `std::atomic<T>`, whose `alignas(sizeof(T)) T _M_i` is
passed as `std::__addressof(_M_i)`, gets a naturally aligned inline
atomic for a power-of-two size, as with clang, and needs no libatomic.

The alignment of an lvalue derived from another (WP-829; clang 19
`EmitLValueForField`, `emitArraySubscriptGEP`, `GetAddressOfBaseClass`):
- **Field** `b.m` / `p->m`: `alignmentAtOffset(base alignment, offset of
  m)` (`min_align`, the largest power of two dividing both), the offset
  being the field's IR struct element offset (a packed struct's,
  `__attribute__((packed))` or `#pragma pack`, is unpadded), or its byte
  offset for an empty field with no element; a bit-field's storage unit
  likewise; a union member keeps the base's. A reference member's
  referee is the natural pointee alignment of the referenced type.
- **Subscript** `a[i]` / `p[i]`: the base's alignment (the array lvalue's
  for a simple array-to-pointer decay, else the pointer's from this
  section) at the byte offset `i * sizeof(element)` for a constant `i`, or
  at one element (`MinAlign(align, sizeof(element))`) for a variable
  one (`getArrayElementAlign`).
- **Base subobject** (`GetAddressOfBaseClass`): a non-virtual path keeps
  the derived object's alignment at the non-virtual offset (unchanged
  for offset 0); a path through a virtual base starts from
  `getVBaseAlignment` (`vbase_align`): the virtual base's non-virtual
  alignment if the derived alignment is at least the derived class's
  non-virtual alignment, else the smaller of the two (an incomplete
  class takes the smaller), and then takes the non-virtual offset within
  the virtual base.
- **`this`** (`LoadCXXThisAddress`, `CXXThisAlignment`) and any class
  pointer that reaches the fallback: `getClassPointerAlignment`, the
  class's non-virtual alignment unless it is effectively `final` (`final`
  class or destructor), when it is the complete-object alignment. A
  class with a virtual base of stricter alignment is thus accessed through
  a pointer at its smaller non-virtual alignment: as a base subobject it
  need not be aligned as a complete object ([class.mi], [basic.align]),
  and the complete-object alignment there is an over-alignment that lets
  LLVM emit aligned vector accesses on a misaligned address.
- **Member pointer** `o.*m` / `p->*m` (`EmitCXXMemberDataPointerAddress`):
  `getDynamicOffsetAlignment(object alignment, class, natural alignment
  of the member type)`, the object's alignment from `->*`'s pointer as
  above.
- **References**: a reference's referee has the natural pointee
  alignment of its type (`EmitLoadOfReference`, `forPointeeType`).

So `*&x.m` keeps the `alignas` of `x` and `m`, `(char *)&x` keeps `x`'s
until an explicit cast to a non-character type whose operand's source is
not `Decl`, and `arr[2]` of an `alignas(64) int arr[]` is `align 8`.
Not mirrored: a `delete` expression's operand still has the natural
pointee alignment (clang `EmitCXXDeleteExpr` uses
`EmitPointerWithAlignment`); it only affects the array-cookie and vtable
loads.

### 3.4 Target builtins (`arm.rs`; WP-724, WP-846)

The `AArch64` ACLE builtins (`__builtin_arm_*`, `kiln_ast::decl::
builtin::arm`, declared per spec/landed/sema/WP-724.md) lower as clang 19
`EmitAArch64BuiltinExpr`; every one listed here is lowered (none stays in
`UNLOWERED_BUILTINS`). Intrinsic attribute shapes are `kiln_llvm::
intrinsics` (`target_nomem`, `target_intrinsic` with a `TargetMem`,
`aarch64_prefetch`, `aarch64_break`).
- **Hints, barriers, CRC32, bit counts** (WP-724): `llvm.aarch64.hint`
  with clang's `HintID`, `llvm.aarch64.{dmb,dsb,isb}`, `clrex`, the
  `crc32*` intrinsics (8/16-bit data zero-extended to `i32`; `crc32d` is
  `crc32x`), `llvm.bitreverse`, `llvm.ctlz` (`i1 false`), `llvm.aarch64.cls`.
- **Exclusive accesses** (`ldrex`/`ldaex`, `strex`/`stlex`): a 128-bit
  value uses `llvm.aarch64.ld[a]xp` / `st[l]xp` as two `i64` halves
  (the store goes through a temporary); anything else uses
  `llvm.aarch64.ld[a]xr.p0` / `st[l]xr.p0`, whose address operand carries
  `elementtype(iN)` (`N` the value's size), with the value truncated /
  zero-extended to `i64`, bit-cast for floating types and converted for
  pointers. A floating constant bit-cast to an integer folds
  (`func.rs` `fold_cast`, as LLVM's `ConstantFoldCastInstruction`). At
  `-O0` a load/store-exclusive retry loop can spin forever on real
  hardware, under clang too (the spills between them clear the
  reservation); the run test is `-O2`.
- **System registers** (`rsr`, `rsr64`, `rsrp`, `rsr128`, `wsr`, `wsr64`,
  `wsrp`, `wsr128`; clang `EmitSpecialRegisterBuiltin`): the string
  literal as written becomes `!{!"name"}`, read with
  `llvm.read_volatile_register.i64` (`.i128` for the 128-bit forms) or
  written with `llvm.write_register`; `rsr`/`wsr` truncate / zero-extend
  32 bits and `rsrp`/`wsrp` convert a pointer. The 128-bit forms need
  `d128` (`-march=armv9.4-a+d128`).
- **Conversions and random numbers**: `rint{32,64}{x,z}[f]` are
  `llvm.aarch64.frint{32,64}{x,z}.{f32,f64}`, `jcvt` is
  `llvm.aarch64.fjcvtzs`; `rndr`/`rndrrs` call the `{ i64, i1 }`
  intrinsic, store the value through the operand (evaluated after the
  call, as clang) and return the status zero-extended.
- **64-byte loads and stores** (`ld64b`, `st64b`, `st64bv`, `st64bv0`):
  the eight words go through `getelementptr i64, ptr v, i32 i`, stored
  after `llvm.aarch64.ld64b` or loaded before the store intrinsic.
- **GCS, `chkfeat`, `prefetch`, `trap`**: `llvm.aarch64.gcspopm` /
  `gcsss` (side effects; clang 19's `gcsss` takes and returns `ptr`),
  `llvm.aarch64.chkfeat` (no memory), `llvm.aarch64.prefetch` with the
  five operands, and `trap(n)` as `llvm.aarch64.break(i32 n)` with no
  `unreachable` after it.

**Operand checks** (`kiln_sema` `arm_builtins.rs`, clang 19
`SemaARM::CheckAArch64BuiltinFunctionCall`), after the arguments are
converted and skipped while value-dependent; every error is at the
callee's begin location:
- The register operand of the system-register builtins must be a string
  literal (through parentheses and implicit casts; "expression is not a
  string literal"). With colons it must be exactly five base-10 fields
  `o0:op1:CRn:CRm:op2` within `[0, max_o0]`, `[0, 7]`, `[0, 15]`,
  `[0, 15]`, `[0, 7]` ("invalid special register for builtin"); `max_o0`
  is 1 at `-fclang-compat=19` and 3 at 22 (clang 22 allows the full
  `op0`). clang 19 accepts `1:...` and encodes it as `op0` 1, a `sysl`
  that traps; kiln emits the same IR. A plain name is not checked,
  except that a two-operand write of a PSTATE field (`spsel`, `daifclr`,
  `daifset`, `pan`, `uao`, `dit`, `ssbs`, `tco`: `[0, 15]`; `allint`,
  `pm`: `[0, 1]`) checks the value's range; the 128-bit forms never do.
- `prefetch` operands 1–4 are in `[0, 1]`, `[0, 3]`, `[0, 1]`, `[0, 1]`
  and `dmb`/`dsb`/`isb` take `[0, 15]` ("argument value V is outside the
  valid range [lo, hi]"); a non-constant or non-integral one is
  "argument to '<builtin>' must be a constant integer".
- The exclusive accesses warn "passing T to parameter of type U discards
  qualifiers" (`-Wincompatible-pointer-types-discards-qualifiers`, an
  ExtWarn) when the pointee's qualifiers are dropped, before the pointee
  type check. clang's size check (wider than 16 bytes) cannot fire on
  `AArch64`: every integer, floating and pointer type is at most 16
  bytes.

Tests: `conformance/target.acle/` (`ir-*-19.cpp` exact under oracle 19,
`exclusive-sysreg-run.cpp`, `operand-checks.cpp`,
`special-register-o0{,-19}.cpp`), `kiln_sema` `arm_builtins::tests`.

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
- A declaration in unreachable code is still emitted. A local variable's
  alloca and NRVO flag, and a VLA's bound computation, first reopen a
  block with `ensure_insert_point` (clang `EnsureInsertPoint` in
  `EmitAutoVarAlloca` and `EmitVariablyModifiedType`). Every
  `fir.cur().expect(fir::OPEN_BLOCK)` depends on this (WP-971).
- Each loop gets a distinct `!llvm.loop` node with
  `llvm.loop.mustprogress`. A loop with a constant-true or absent
  condition and an empty body clears `mustprogress` on the function and
  gets no `mustprogress` entry; a range-based for loop never gets one.
- **C** (`CodegenOptions::c`, WP-1275): a function gets no
  `mustprogress` attribute and `main` no `norecurse` (clang: C++11 / C++
  only), and no return attribute is strict (`HasStrictReturn` is C++'s).
  A loop's `llvm.loop.mustprogress` follows [C11 6.8.5]/6: only from C11,
  and only when the controlling expression is not a constant expression
  (none in C89/C99). Test: `tests/c/linkage/symbol-names.c` (`expect: ir`).
- **Loop metadata at -O1** (WP-431; clang `LoopInfoStack::push`,
  [intro.progress]/1): at `CodegenOptions::opt_level == 1` exactly (clang
  `-O`/`-O1`/`-Og`, where `UnrollLoops` is off) every loop's node also
  carries `!{!"llvm.loop.unroll.disable"}` after the optional
  `mustprogress` entry: `!{self, mustprogress?, unroll.disable}`, the
  `unroll.disable` tuple cached per module. A loop with neither entry
  gets no `!llvm.loop`. `tests/codegen/loop_unroll_disable_o1.cpp`.
- **Loop hints** (WP-449; clang `CGLoopInfo`, `LoopInfoStack::push`/`pop`,
  `LoopInfo::createMetadata`; `loop_md.rs`): a loop whose statement is
  the sub-statement of an `AttributedStmt` with `LOOP_HINT` attributes
  (spec/parse.md §2.3) gets them as clang's `LoopAttributes`, the hint
  values folded as integer constants (`AttributedStmt` arms reach codegen
  only through `Cg::f.loop_hints`, keyed by the loop statement). Option
  and state map as in clang `LoopInfoStack::push` (`Unroll`/`UnrollCount`,
  `UnrollAndJam`/`UnrollAndJamCount`, `Vectorize`/`VectorizeWidth` with
  `fixed`/`scalable`, `Interleave`/`InterleaveCount`, `Distribute`,
  `PipelineDisabled`/`PipelineInitiationInterval`, `VectorizePredicate`);
  at -O1 an unspecified unroll with no count becomes `disable`. The
  node is a `distinct` placeholder made when the loop is pushed (clang's
  `TempLoopID`) and filled when it is popped, through
  `Module::md_set_operands`, so an inner loop of an `unroll_and_jam` parent
  is finished first and becomes the parent's
  `llvm.loop.unroll_and_jam.followup_inner` (listing its properties;
  at `-fclang-compat=19` one distinct self-referential node of them, as
  clang 19's loop-id `UnrollAndJamInnerFollowup`), the parent's own
  `followup_outer` carrying the before-jam attributes. The followup chain
  is clang's: full unroll → distribute → vectorize (`followup_all`,
  `followup_epilogue`) → unroll-and-jam → partial unroll → pipelining,
  each stage adding `llvm.loop.isvectorized` / `unroll.disable` /
  `unroll_and_jam.disable` / `distribute.enable` as clang does; the
  `mustprogress` entry comes first in the innermost node.
  `vectorize(assume_safety)` / `interleave(assume_safety)` set
  `vectorize.enable` and make the loop parallel (clang `IsParallel`,
  WP-462): pushing it makes a `distinct !{}` access group (before the
  loop id), its node lists `!{!"llvm.loop.parallel_accesses", group}`
  right after `mustprogress` (in the before- and after-jam nodes under
  an unroll-and-jam parent too), and while it is active every
  instruction `Fir::push` appends that may access memory (load, store,
  atomic, fence, call, `invoke`, `callbr`; LLVM `mayReadOrWriteMemory`)
  gets `!llvm.access.group` (clang `LoopInfoStack::InsertHelper`): the
  group of the one active parallel loop, or the uniqued list of every
  active parallel loop's group, outermost first. A call whose callee is
  a `memory(none)` function (a `const` function, `llvm.ctpop`) gets none;
  only the callee's attributes count, as clang tags the call before
  setting the call-site attributes. Instructions inserted at another
  position (`insert_before`/`insert_after`, clang `new X(.., InsertPt)`)
  get none either. The loop is active where clang pushes it: from
  before the condition of a `for` and a range-based `for`, from after
  the condition of a `while` (its cleanups included), and only around a
  `do` loop's back-edge.
  `tests/codegen/loop_hints.cpp`, `tests/codegen/loop_hints_o1.cpp`,
  `tests/codegen/loop_parallel.cpp`, `tests/codegen/loop_parallel_o1.cpp`,
  `tests/codegen/loop_parallel_jam.cpp`.
- **Forwarding blocks** (clang `SimplifyForwardingBlocks`): a loop
  condition block that only branches on is folded into its predecessor
  only while no cleanup (a destructor, a lifetime end, a `noexcept`
  terminate scope) is on the EH stack. `tests/codegen/forwarding_block_cleanup.cpp`.
- Case ranges (GNU) are an error. GNU computed goto: §7t.
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
  block (`noreturn_call`, `finish_noreturn_call`). A call of such a
  virtual function through the vtable has neither, as in clang
  (`CallBase::doesNotReturn` with no called function; WP-871). Under
  `-fsanitize=unreachable` see "Sanitizers".

- `[[assume(e)]]` (WP-160): when there is an insertion point, each
  assumption that is not `ASSUME_IGNORED` is emitted as a scalar and
  passed to `llvm.assume`, in source order and before the (null)
  sub-statement (`emit_assumptions`; `tests/codegen/assume_attr.cpp`).
  Calls to `const`/`pure` functions carry their call-site attributes
  (§3 "Calls", WP-182).

- Expansion statements (WP-447, `emit_expansion`): an expanded
  `CXXExpansionStmt` lowers its init-statement and prelude, then each
  element's compound statement in order, with no loop and no
  `!llvm.loop` metadata; `break` branches to a block after the last
  element and `continue` to a block ending the current one, through the
  element's cleanups. A pattern is never lowered. clang 22 has no
  expansion statements, so `tests/codegen/expansion_stmt.cpp` is
  `min-oracle: 23` (verify-only today); the order and the absence of
  back-edges are checked by `kiln_codegen` test
  `expansion_statement_lowers_in_order`.
  While optimizing, an expansion whose elements together exceed
  `-fkiln-outline-expansions=N` AST nodes (default 2048) emits each
  sizeable element as an internal `noinline` helper
  `<function>.expand.<n>` called in its place, with a status result
  for `break` and `return` (WP-793, `outline.rs`; the full rule is
  spec/landed/codegen/WP-793.md).

- Consteval-only variables (WP-534): a block-scope variable whose type
  is consteval-only (P2996R13 [basic.types.general]: `std::meta::info`,
  or a type that contains one through an array element, pointee,
  referee, member pointer, function return or parameter type, base or
  non-static data member; spec/reflection-repr.md §6) is never emitted:
  `emit_one_var_decl` returns before allocating storage or lowering the
  initializer (`Cg::is_consteval_only`, `module.rs`, a walk over the
  canonical type memoized per record). Such a variable can only be used
  in constant expressions, and every use of it in emitted code is
  already folded. In particular an expansion statement over a range of
  `info` (a `define_static_array` variable template or call, a
  `static constexpr info arr[]` local) emits no range object, no
  `__range`/`__begin`/`__end` and no structured-binding object: its
  prelude declarations are all consteval-only, and each element's
  constexpr `R` is substituted as a constant
  (`tests/conformance/stmt.expand/reflection-info-range-run.cpp`).
  A namespace-scope one is not emitted either, whatever its linkage
  (`extern constexpr info r = ^^S;`, an `export`ed one of a module
  interface, WP-1047): `emit_global_var` returns before it too
  (`tests/conformance/basic.types.general/consteval-only-extern-run.cpp`).

## 5. Driver (`kiln_driver::codegen_cmd`, `link`)
The normative description is spec/driver.md "Code generation". In short:
- `-S -emit-llvm` writes a `.ll` file; `-c -emit-llvm` pipes it to
  `llvm-as-19`.
- `-S` pipes the IR to `llc-19`; `-c` to `llc-19 -filetype=obj`.
- Linking compiles each source to a temporary object, then runs
  `clang++-19`, passing `-fuse-ld=`, `-l`, `-L`, `-Wl,`, `-static`,
  `-nostdlib`, `-shared`, `-rdynamic`, `-pie` and `-no-pie` through in
  command-line order.
- Tools come from `<llvm>/bin` of the LLVM installation spec/driver.md
  "LLVM discovery" selects (LLVM 22 by default, or 19).
- `-O<n>` selects `llc`'s level and is passed to the link; the IR is
  always the `-O0` IR. `-g` adds DWARF debug information (§5.1).
  `-###` and `-v` print the steps.
- Every job calls `ConstexprOpts::write_report`.

### 5.1 Debug information (WP-814; `debuginfo/`)
`CodegenOptions::debug` (`DebugOptions`, set from the flags in
spec/driver.md "Debug information flags") selects clang 19's
`codegenoptions::DebugInfoKind`. At `None` (the default) the IR is
unchanged, byte for byte. Otherwise kiln emits what clang 19's
`CGDebugInfo` emits for the same program, in clang's node creation
order, so `-S -emit-llvm -g` output equals clang's after `opt-19`
(producer string aside):
- a `DICompileUnit` (`!llvm.dbg.cu`) whose file carries the main
  buffer's MD5, the `"Dwarf Version"` and `"Debug Info Version"` module
  flags, and `DIFile`s split against the compilation directory after
  `-fdebug-prefix-map=`;
- under line tables only, a `DISubprogram` per definition with
  `DILocation`s on instructions;
- under `-g`, also types (basic, pointer/reference, arrays, enums,
  typedefs, classes with members, bases, methods, vtable holders and
  template parameters; class definitions homed as clang's
  `-fno-standalone-debug` does), namespaces, global and static member
  variables (`DIGlobalVariableExpression`), parameters and locals
  (`#dbg_declare`), and lexical blocks;
- statement and expression locations with clang's `EmitStopPoint`,
  `ApplyDebugLocation` and cleanup/landing-pad rules, so a debugger
  steps, breaks and finishes where it does in a clang build.

Debug information is total (WP-910, spec/landed/codegen/WP-910.md): `-g`
at any level compiles every program kiln compiles without it. A
construct it cannot describe degrades the way clang's does. A type with
no `DIType` leaves out the field or node that refers to it. A template
value argument with no IR constant keeps its parameter with `value:
null`. A template template argument with no template declaration leaves
its parameter out. `std::meta::info` is a pointer-sized `DIBasicType` of
that name. SVE types are clang's `VG`-scaled vector arrays. The
remaining differences from clang are listed in docs/status/debuginfo.md
"Gaps". The normative detail and the tests are in
spec/landed/codegen/WP-814.md and WP-910.md; the status, statistics and performance
are in docs/status/debuginfo.md. WP-817 (spec/landed/codegen/WP-817.md)
cut the front-end cost of `-g` without changing a byte of the IR.

### 5.2 Sanitizers (WP-675, WP-871; `ubsan.rs`, `class_b.rs`)
`CodegenOptions::sanitize` (`SanitizeOptions`, set from spec/driver.md
"Sanitizers") marks the IR as clang 19's frontend does
(docs/design/sanitizers.md §1); with no sanitizer the IR is unchanged.
- **Pass sanitizers.** `sanitize_address`, `sanitize_hwaddress`,
  `sanitize_thread`, `sanitize_memory` (and `optforfuzzing` under
  fuzzer) on every definition that does not opt out with
  `no_sanitize("...")`, `no_sanitize_address`/`_thread`/`_memory` or
  `disable_sanitizer_instrumentation`; global `no_sanitize_address`/
  `no_sanitize_hwaddress` and `sanitize_address_dyninit` markers;
  lifetime markers at `-O0` under ASan use-after-scope and HWASan;
  TBAA under TSan. The instrumentation itself is LLVM's: the driver
  runs clang's pass pipeline.
- **UBSan.** signed-integer-overflow (`+ - *`, unary `-`, `++`/`--`,
  compound assignment), unsigned-integer-overflow, integer-divide-by-zero
  and signed `/`/`%` overflow, shift-exponent, shift-base (signed only
  before C++20) and unsigned-shift-base, return, unreachable and
  vla-bound, as clang's `EmitCheck`: the checks of one operation are
  and-ed per mode (trap, recover, abort); a failing check branches (with
  clang's 1048575:1 weights) to `@llvm.ubsantrap(<handler id>)`, a
  `@__ubsan_handle_<name>` call or its `_abort` form, with the
  `.src`/type-descriptor/data globals clang emits, every inserted
  instruction under `!nosanitize !{}`. Operands promoted from a
  narrower type are not checked where clang elides the check. Trap
  blocks are shared per handler except at `-O0`/`optnone`. At
  `-fclang-compat=22` the trap branch also gets the weights, `nomerge`
  marks trap and handler calls at `-O0`/`optnone` and for checks outside
  the default merge set (the unsigned checks), and `@llvm.ubsantrap` is
  `memory(inaccessiblemem: write)` from LLVM 22.
- **`-ftrapv`, `-ftrapv-handler=`, `-fsanitize-merge=`** (WP-1032;
  `CodegenOptions::trapv`, `trapv_handler`, `SanitizeOptions::merge`;
  spec/driver.md). `-ftrapv` checks the signed operations the sanitizer
  leaves alone with the empty kind: `with.overflow` plus a bare trap,
  clang's `EmitTrapCheck`, whose trap block is shared like the
  sanitizer's. With `-ftrapv`, a handler name routes every overflow check through
  clang's `EmitOverflowCheckedBinOp` handler block; an operation wider
  than 64 bits (`__int128`) keeps the check it has without the handler
  (the handler's `(i64, i64, i8, i8, ...)` cannot carry its operands;
  clang 19 and 22 emit an invalid `sext i128` there and crash, WP-1044,
  `ubsan.rs` `ub_overflow`). The merge set
  splits trap blocks and adds `nomerge` as clang 20+.
- **Ignorelists** (WP-1032; `ignorelist.rs`, `class_b.rs`
  `ignored_sanitizers` / `global_sanitizer`): a function an entry
  names for some sanitizers is treated as `no_sanitize` of them (its
  `sanitize_*` attributes and UBSan checks go, including in its
  outlined and implicit helpers); a global loses its address/hwaddress
  instrumentation and, for `=init`, `sanitize_address_dyninit`; at
  `-fclang-compat=22` a `type:` entry elides overflow and implicit
  conversion checks of that type. A string-literal global (WP-1043,
  `consts.rs` `lit_string_global`, clang
  `GetAddrOfConstantStringFromLiteral`) is checked only when the
  literal creates it, against `global:` (its name: `.str.N`,
  `__func__.<mangled>`), `mainfile:` and `src:` (the file of the
  literal's expansion), with no `type:` lookup. An `await_suspend`
  wrapper (`coro.rs`), emitted by clang without a declaration, gets the
  module's sanitizers (attributes and checks) less the entries naming
  its own symbol by `fun:` or the main file by `src:`/`mainfile:`.
  Rules in spec/driver.md "Sanitizers".

- **UBSan type and value checks** (WP-871; docs/design/sanitizers.md
  §1.5 has the coverage table). Each is clang 19's function of the
  same name, with its handler, static data and location
  (`getExprLoc`; an overloaded `()`, `[]`, `->`, `++`, `--`, `new`,
  `delete` or `co_await` call is located at its begin):
  - null, alignment and object-size (`EmitTypeCheck`) on every
    glvalue access, reference binding, constructor and member call,
    up/downcast, `this` prologue, `new` and `delete`, with clang's
    skips. Alignment is folded for a global or alloca aligned enough.
    The `memcpy` family checks its pointer arguments' alignment.
  - array-bounds (`EmitBoundsCheck`), except on flexible-array-like
    members (`-fstrict-flex-arrays=0`).
  - bool and enum on scalar loads (`EmitScalarRangeCheck`).
  - float-cast-overflow (`EmitFloatConversionCheck`), including
    complex parts and `_Atomic` destinations (signed bounds).
  - pointer-overflow (`EmitCheckedInBoundsGEP`).
  - nonnull-attribute, returns-nonnull-attribute (through
    `return.sloc.ptr`) and builtin (`__builtin_ctz`/`clz` of zero).
    The `memcpy`/`memmove`/`mempcpy`/`memcpy_inline` builtins check
    both pointers (`memset` its destination) against the called
    declaration's `nonnull`, each before its alignment check.
  - unreachable after a call that does not return: the call site and
    the called function lose `noreturn`, and a `builtin_unreachable`
    check at the call's location precedes the `unreachable`; with ASan
    the frontend puts `__asan_handle_no_return` before the call.
  - the implicit integer truncation and sign-change checks
    (`EmitScalarConversion`) and implicit-bitfield-conversion. The
    `integer` group excludes the bit-field check.
  - `_Atomic` `++`/`--`/`op=`: an unsigned operand under
    unsigned-integer-overflow, and any operator `atomicrmw` cannot
    do, take the compare-exchange loop with the checked operation.
  - Type descriptors name the written type, or the canonical type
    (`getRecordType`, `getThisType`, the float-cast operands). A
    canonical tag prints with its full scope.
    The name is clang's `ConvertArgToString(ak_qualtype)`, the very
    formatting of a type argument of a diagnostic (WP-1158;
    `kiln_ast::diag_names::DiagNames::type_arg`, shared with Sema's
    `ty_arg`): `'T'` or `'T' (aka 'U')`, with the `-fclang-compat`
    level's spellings (for `ns::myE` naming `ns::E`, clang 19's aka is
    fully scoped, `(aka 'ns::E')`, and clang 22's keeps the written
    qualifier, `(aka 'E')`). The written type is the sugared one: a member
    access, `dynamic_cast` and `delete` check the pointee as written
    (`TypeContext::pointee_sugared`, clang `getPointeeType`), a
    downcast the written target (`static_cast<DT &>` is an lvalue of
    type `DT`). An implicit class template specialization prints
    without the trailing arguments equal to their defaults
    (`'ns::H<>'`); an explicit specialization or explicit
    instantiation prints the arguments written on it
    (`getTemplateArgsAsWritten`, `Templates::args_as_written`):
    `'std::coroutine_handle<void>'`, `'n::X<TA>'`. For a redeclared
    explicit specialization clang 19 keeps the definition's written
    arguments and clang 22 the first declaration's; an explicit
    instantiation keeps the first one's.

  At `-fclang-compat=22`:
  - a signed `_BitInt` descriptor is kind 2 with its width appended;
  - a range check passes the raw value;
  - a bounds check compares in the wider of the index type and
    `size_t`.

The checks kiln does not emit yet (vptr, function, nullability-*,
local-bounds) and the other gaps are listed with their BACKLOG rows in
spec/landed/codegen/WP-675.md and spec/landed/codegen/WP-871.md.

### 5.3 LTO (WP-873; `module.rs`, `vtable.rs`)
`CodegenOptions::lto` (`Lto::Full` / `Lto::Thin { split }`, set from
spec/driver.md "Codegen flags" `-flto[=thin]`) makes the module an LTO
unit as clang 19's `-flto` does (`CodeGenOpts.PrepareForLTO` /
`PrepareForThinLTO`):
- **Module flags** (clang `EmitAssemblyHelper::RunOptimizationPipeline`,
  after the frontend's, behaviour 1): full LTO adds `ThinLTO` 0 and
  `EnableSplitLTOUnit` 1; thin LTO adds `EnableSplitLTOUnit` 0 or 1
  (`-fsplit-lto-unit`). `CodegenOptions::lto_module_flags` is off under
  `-disable-llvm-passes` (clang runs no backend pipeline) and for a thin
  bitcode object, where `opt --thinlto-bc` adds the flag itself.
- **Vtable `!type` metadata** (clang `EmitVTableTypeMetadata`, both LTO
  kinds): on each vtable definition (not an `available_externally` one),
  for each address point of the group, sorted by the base's `_ZTS` name
  and then offset, `!type !{i64 <byte offset>, <id>}`; then, for every
  virtual function slot (destructor slots and unused slots aside),
  `!type !{i64 <slot offset>, <id of the member pointer type
  '<fn type> <base>::*'>.virtual}` (`mangle::mangle_member_pointer_rtti`,
  clang `CreateMetadataIdentifierForVirtualMemPtrType`). An id is the
  `_ZTS` string of an externally visible type, else one `distinct !{}`
  per type (`CodeGen::type_md_ids`, clang `MetadataIdMap`). A
  construction vtable carries its set twice, as clang 19 and 22 do.
- The module summary, the pre-link pipeline and the bitcode writer are
  the driver's (`opt`, spec/driver.md).
- Not yet: `-ffat-lto-objects` (rejected by the driver, `'-ffat-lto-objects'
  is not supported by kiln`; spec/driver.md "Codegen flags"), whole-program
  devirtualization options.
- Tests: `tests/codegen/lto_{full,full_o2,thin,thin_split}.cpp` (exact IR
  against clang 19, `ir_vs_clang`), `tests/conformance/lex.phases/
  lto-two-tu-*-run.cpp` (two TUs built, linked and run, with the default
  linker and `-fuse-ld=lld`).

### 5.4 Instrumentation (WP-941, WP-1003, WP-1082; `coverage/`)
`CodegenOptions::coverage` (`CoverageOptions`, set from spec/driver.md
"Coverage flags") instruments the module as clang 19's frontend does;
with no coverage flag the IR is unchanged, byte for byte.
- **gcov** (`--coverage`, `-fprofile-arcs`, `-ftest-coverage`): the
  module gets clang's `!llvm.gcov = !{!{!"<notes>.gcno", !"<data>.gcda",
  <cu>}}`; the counters and the notes are written by `opt`'s
  `insert-gcov-profiling` (clang `GCOVProfilerPass`), which finds lines
  through debug locations. Without `-g` kiln therefore emits clang's
  location-tracking-only debug information (`DebugInfoKind::
  LocTrackingOnly`: the compile unit with `emissionKind: NoDebug`,
  subprograms and statement/expression locations, no types or
  variables), so `llvm-cov gcov` reports the lines clang's build does.
- **InstrProf counters** (`-fprofile-instr-generate`, `coverage/pgo.rs`):
  clang `CodeGenPGO`. Each instrumented definition gets
  `@__profn_<name>` (`private`, the PGO function name: the mangled name,
  `<file>;<name>` for an internal one) and `llvm.instrprof.increment(
  name, hash, num_counters, index)` at clang's points: the function
  entry, and the `MapRegionCounters` regions (loops and their bodies,
  `if`/`else`, `case`/`default` and labels, `&&`/`||` right operands,
  `?:` arms, `catch` handlers, `co_await`/`co_yield` resume points), in
  the pre-order of clang's `RecursiveASTVisitor` with its quirks (a
  local class's member functions and a block-scope function declaration
  number into the enclosing function; a lambda's body and a defaulted
  function's body are not walked). The hash is clang 19's V3
  `PGOHash` (MD5 of the region kinds). An `&&`/`||` that is folded or
  branched to its parent's targets keeps clang's counter placement. A
  consteval function is never emitted, so it has no record (it has
  none under clang either); constexpr functions are instrumented where
  they are emitted. The counters are lowered by `opt`'s `instrprof`
  pass (clang `InstrProfilingLoweringPass`) into `__llvm_prf_cnts` /
  `__llvm_prf_data`; under `-disable-llvm-passes` the intrinsics stay.
  `-fprofile-instr-generate=<path>` also emits clang's
  `@__llvm_profile_filename` (`hidden`, `comdat`) unless
  `-disable-llvm-passes` is given (clang's lowering creates it, so
  without the pass there is none).
- **Coverage mapping** (`-fcoverage-mapping` with the counters,
  `coverage/{mapping,records,encode,skipped}.rs`, WP-1003): clang 19
  `CoverageMappingGen`, plus clang 20+'s rules under
  `-fclang-compat=20` or later (WP-1082, below).
  - *Regions.* `mapping.rs` ports `CounterCoverageMappingBuilder`
    statement for statement over the `pgo.rs` counters: code regions
    for bodies, statements, conditions and `?:`/`&&`/`||` operands; gap
    regions after `)` of a condition and between a statement and what
    follows a jump; branch regions for every instrumented condition
    (`if`/loops/`?:`/`&&`/`||`, with clang's folded-constant zero
    counters); expansion regions for macro expansions, nested as clang
    nests them; and clang's region order. Locations are clang's: an end
    is the start of the last token plus its length
    (`getPreciseTokenLocEnd`), a `= default` body is one region (in a
    class at the `default` keyword, out of class at the declarator's
    last token), and a constructor call ends at its `)`/`}` or its last
    written argument. A concatenated string literal ends at its last
    piece, a compound literal `(T){...}` at its `}`, and a qualified
    base initializer `N::B(x)` starts at `N` (clang's
    `ElaboratedTypeLoc`; the diagnostic location moves with it).
    Locations the AST does not keep (a condition's `)`, a `;`, a case's
    `:`) are re-lexed from the spelling. A `#x` stringizing in a macro
    body is a nested expansion: its token is mapped at the `#x` (clang
    `getNonScratchExpansionLoc`), so the literal ends there and gets a
    region of its own.
  - *clang 20+* (`-fclang-compat=20` or later, `Builder::v20`):
    - an `&&`/`||` right operand's count starts at the end of the left
      operand (a gap region between them when they are apart);
    - a branch whose condition folds to a constant keeps its counter on
      the side the constant takes and zeroes only the other side (clang
      19 zeroes both);
    - a `switch` without `default` has the cases' sum as its true side
      and the rest as its false side (clang 19 has them the other way);
    - an out count equal to the parent's after counter-expression
      simplification (`IsCounterEqual`) starts no region, and a
      `do ... while` body's termination survives the loop only when its
      exit starts one, which gives a `do { ... } while (0)` macro body
      its region;
    - a statement expression's `return` does not terminate the
      enclosing region.
  - *Skipped regions* (`skipped.rs`, clang `CoverageSourceInfo`): the
    groups the preprocessor skipped (`kiln_pp::PpCallbacks::
    source_range_skipped`), comments and empty-line runs, each with
    clang's previous/next watched token, so a range on a line with code
    is trimmed or dropped as clang's.
  - *Records* (`records.rs`): each emitted function's `__covrec_<NameHash
    hex>` in `__llvm_covfun` (`{ i64 NameHash, i32 size, i64 FuncHash,
    i64 FilenamesRef, <mapping> }`, `linkonce_odr hidden` with a comdat of
    its own name, the NameHash being MD5 of the PGO name, FuncHash the
    pgo.rs hash, FilenamesRef the TU hash); the TU's
    `__llvm_coverage_mapping` in `__llvm_covmap` (version 6 header and
    the filenames: the compilation directory, then each file, normalized
    as clang `normalizeFilename`). Filenames are written uncompressed;
    clang compresses them by default, so the TU hash (MD5 of the
    filenames blob) differs from clang's unless clang is run with
    `-mllvm -enable-name-compression=false`, when the section is the
    same, byte for byte. `llvm-cov` reads both forms.
  - *Unused functions* (clang `AddDeferredUnusedCoverageMapping`): a
    function defined in the TU and never emitted (an unused `inline` or
    `static` function, an unused member of a class or of an
    instantiated class template; templates themselves get none) gets a
    zero record
    (`FuncHash` 0, one zero code region over its body) and its PGO name
    in `__llvm_coverage_names`, so `llvm-cov` reports it at 0%. An
    instantiated function, member function or conversion function
    definition gets one too, as clang's `HandleTopLevelDecl` at the end
    of `InstantiateFunctionDefinition`, even when only constant
    evaluation used it (`kiln_ast::Decls::note_fn_def_inst` records the
    instantiation and the top-level declaration it happened in;
    `CovModule::cov_defer_insts` defers them in that order before each
    later top-level member). Constructors, destructors, immediate
    functions and the std builtins (`std::move`, ..., which their calls
    never reference) get none that way. A lambda's conversion function
    and static invoker are implicit, so a generic lambda's
    specializations of them have no record.
  - *Order* (WP-1107, spec/landed/codegen/WP-1107.md): the zero records
    and their names come in clang's `DeferredEmptyCoverageMappingDecls`
    order (a `MapVector`: first deferral wins, a later use clears the
    entry in place, `EmitDeferredUnusedCoverageMappings` walks it at the
    end of the TU). Entries are added where clang adds them: a top-level
    function definition (`EmitTopLevelDecl`), an inline member definition
    the parser finished (`HandleInlineFunctionDefinition`, at the end of
    its class: `cov_defer_class`), and an instantiated definition where
    its instantiation ends (`cov_defer_insts`). A member of a class
    template specialization (`extern template struct S<long>;`, an
    implicit instantiation) was never parsed as an inline definition, so
    `cov_defer_class` skips it (`Ast::instantiated_from`); its record
    comes from its own instantiation, as clang's.
  - *Folded `new` conditions* (WP-1108): a condition whose value is a
    new-expression (`if (new int)`, `c || new (std::nothrow) int(4)`,
    `!new T`) folds to `true` exactly when clang's `ConditionFoldsToBool`
    folds it (the codegen fold prepass, §3: a replaceable global
    allocation function, the nothrow form or the reserved placement
    form, and an initializer that evaluates), so its branch regions are
    those of a folded condition.
  - Without `-fcoverage-mapping` none of this is emitted, and the
    counters are WP-941's, byte for byte.
  - *Differences from clang*: none known in the records or the mapping
    (spec/landed/codegen/WP-1003.md, WP-1082.md; WP-1107 removed the
    last two, the zero-record order and the `(bool)new T` fold).
- **Runtime.** Both modes link `libclang_rt.profile-<arch>.a` from the
  LLVM 19 compiler-rt (`/usr/lib/llvm-19/lib/clang/19/lib/linux/` on
  Debian, package `libclang-rt-19-dev`); the linking `clang++` adds it.
- **Debug locations** met on the way (they affect `-g` too, and were
  gcov's line differences against clang): a coroutine's ramp wrapper
  (the `.resume`-less outer function clang 19 emits `private`, clang 22
  `internal`) has an artificial `DISubprogram` with only a linkage name
  whose file is the compile unit's (clang `getOrCreateFile` of an invalid
  location); the cleanup switch and the shared `unreachable` block have no
  location; `mem2reg` phis take LLVM `RenamePass`'s merged location; a
  `co_await`/`co_yield`'s end location is its operand's; and a default
  argument is emitted with debug locations off (clang `EmitCallArg`'s
  `DisableDebugLocationUpdates`), so its expressions keep the call's
  location instead of the parameter declaration's. A value `&&` (clang
  `VisitBinLAnd`) branches from its right-hand side into the merge block
  with no location and gives the phi an artificial line-0 location; a
  value `||` (`VisitBinLOr`) keeps the branch's location and leaves the
  phi without one (otherwise gcov counts a multi-line `&&`'s first line
  once more per right-hand evaluation).
- **clang 22** (`-fclang-compat=22`): coroutine `coro.id` takes the
  promise without the `bitcast` and the wrapper is `internal`; the
  comparison normalizes LLVM 19's `i1 @llvm.coro.end` to LLVM 22's
  `void` form plus `@llvm.coro.is_in_ramp` (`tests/ir_norm`).
- Tests: `tests/codegen/{gcov,instrprof,coverage}_*.cpp` (IR against
  clang 19 and 22, `ir_vs_clang`), `scripts/coverage-compare.py` (builds and runs
  `scripts/coverage/*.cpp` with both compilers and requires equal
  `llvm-profdata show --counts` records and `llvm-cov gcov` line counts,
  except the differences its `KNOWN` table names, which must stay
  different). Results and the compile-time overhead: spec/landed/codegen/WP-941.md.

## 6. Validation
- **Unit corpus** (`crates/kiln_codegen/tests/lower.rs`, WP-052): 90
  inline C++ programs, lowered for `aarch64-unknown-linux-gnu` straight
  from `parse_tu` (no driver). Every case must pass
  `opt-19 -S -passes=verify`. `Exact` cases must also equal
  `clang++-19 -S -emit-llvm -O0 -std=c++26 --target=aarch64-unknown-linux-gnu
  -moutline-atomics` after normalization (the flag for the same reason as
  in the IR-diff corpus below; kiln's default follows the GCC installation as clang's, WP-808). A case that differs but is marked `VerifyOnly` is
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
  `<opts>` to both command lines after it (WP-165). A
  `//@ module: <name>=<path>` line (path relative to the case, lines in
  dependency order) names a module interface unit the case imports: kiln
  gets `-fmodule-file=<name>=<path>` and builds it in-process
  (spec/modules.md §1); clang first precompiles it with `--precompile`
  and the same flags (plus the earlier lines' BMIs) into a scratch
  directory and gets `-fmodule-file=<name>=<bmi>` (WP-376). A
  `//@ min-oracle: <N>` line compares the case only under oracle release
  N or later; under an older oracle kiln's IR must still verify. The oracle is
  `clang++-N`/`opt-N` for `N` = `$KILN_ORACLE` (default 22; spec/harness.md
  "Oracle toolchain"); both texts go through that `opt-N`. Under oracle 22
  kiln's text first gets LLVM 22's lifetime-marker form
  (`ir_norm::lifetime_markers_llvm22`; a no-op on text already in that
  form). kiln writes its backend's form (spec/driver.md "IR optimization",
  WP-783), and with an LLVM 19 backend `opt-22` would upgrade the calls
  but leave the old declarations out of place. `KILN_CASE=<stem>`
  prints both texts (in the oracle's mode); without `opt-N`/`clang++-N` it
  is skipped. Each pair is compared in three ways (WP-355,
  `crates/kiln_codegen/tests/ir_norm/mod.rs`), and the summary line counts
  all three:
  - **Exact**: identical after the normalization above. Required under
    oracle 19 (kiln's codegen mirrors clang 19 `-O0`).
  - **Structural**: identical after also removing what clang/LLVM
    releases change without changing the program: comments, metadata
    lines and `!name !N` attachments, `attributes #N` groups and `#N`
    references, the `target datalayout` (LLVM 20+ added the
    `p270/p271/p272` address spaces on AArch64), `align N` (except a
    `byval` parameter's), GEP and arithmetic flags (`inbounds`, `nuw`,
    `nsw`, `nusw`, `exact`, `nneg`, `disjoint`, `samesign`,
    `inrange(..)`), and the non-ABI parameter/return/function attributes
    (`noundef`, `nonnull`, `noalias`, `nocapture`, `captures(..)`,
    `dead_on_return`, `dead_on_unwind`, `writable`,
    `readonly`/`readnone`/`writeonly`, `returned`, `nofree`, `noinline`,
    `immarg`, `dereferenceable(N)`, `dereferenceable_or_null(N)`,
    `initializes(..)`, `range(..)`, `nofpclass(..)`, `memory(..)`,
    `allocsize(..)`, `allocalign`, `allocptr`, `local_unnamed_addr`);
    each run of consecutive comdat or global lines is sorted (clang 22
    emits `_ZTI` before `_ZTS`). Instructions, types, constants, names,
    linkage and function order stay. Required under any other oracle
    (22).
  - **ABI slice**: the `target triple`, every named struct type
    (`%T = type ...`: the record layouts) and every `define`/`declare`
    signature except `@llvm.*` intrinsics, reduced to calling convention,
    return type, mangled name and parameter types with the ABI attributes
    (`byval(T)` and its `align`, `sret`, `inreg`, `signext`, `zeroext`,
    `byref`, `inalloca`, `preallocated`, `nest`, `swift*`), compared
    verbatim. Required under every oracle.
  A `(case, triple, oracle)` entry in the `DEVIATIONS` table allows
  `Structure` (the mode's comparison may fail; the ABI slice must match),
  `Abi` (both may fail; the IR must verify), `OracleInvalid` (the
  oracle's IR fails verification; only kiln's is verified) or
  `LoopCanon` (kiln's IR is first put through `opt
  -passes=loop-simplify,lcssa` and `ir_norm::unname_locals`, then must
  match exactly; the AArch64 deviation below, WP-189). An entry that
  starts matching is reported for removal. The
  corpus is 226 programs (ABI classification of 1- to 32-byte structs,
  HFAs, byval, varargs, `__int128`, `long double`, bit-fields, static
  locals, string literals, references, control flow, new/delete, and the
  WP-088 `builtins_{bits,mem,fp,misc,srcloc}` cases, the WP-079
  `class_*` cases of §7, the WP-137 `consteval_oversized` case, and later
  work packages' cases; 258 at WP-355): under oracle 19, 513/516 pairs
  exact, `DEVIATIONS` holding `global_init_array_eh`,
  `eh_array_destroy_throwing_global` and `array_temporary_global` on
  aarch64 (§7f, §7m); under oracle 22, 465/516 structural and 485/516 ABI
  slices equal, the rest listed (below, "clang 22 codegen"). kiln gets
  `-fclang-compat=N` for oracle N (WP-386, §6a): at WP-386, 546 pairs,
  oracle 19 531 exact; oracle 22 151 exact, 502 structural and 523 ABI
  slices equal (491 / 512 before kiln followed clang 22 at compat 22).
- **Exact IR goldens** (`expect: ir`, spec/harness.md "IR tests";
  WP-386): `tests/conformance/*/ir-clang22-*.cpp` pin kiln's
  `-fclang-compat=22` module to clang 22's byte for byte (after dropping
  comments and the producer lines) under `requires: clang>=22` and
  `requires: llvm>=22`, on the host target of each CI leg.
- **`run` tests:** 35 printf-based programs (`extern "C" int printf`, no
  headers) under `tests/conformance/<section>/`, each naming its section,
  are compiled, linked and run by kiln and by clang, and their stdout and
  exit status compared. The 5 header-dependent `run` tests
  (`basic.start.main` argv/environment/return-value/stdin and
  `stmt.ranged/constexpr-sum`) are `xfail(kiln)`: libstdc++'s
  `c++config.h` uses `decltype`, which kiln_parse does not support yet.

### Inlining attributes on definitions (WP-389, WP-397, WP-405)
- `Cg::def_inline_attrs` follows clang's
  `SetLLVMFunctionAttributesForDefinition`: an `optnone` attribute on
  any redeclaration, or `-O0` on a function without `minsize` and
  `always_inline`, gives `noinline optnone` (`optnone` overrides
  `always_inline` and `minsize`: neither is emitted, and
  `Cg::has_always_inline_attr` is false); otherwise `naked` gives
  `noinline`; `always_inline` gives
  `alwaysinline` at every level, or `noinline` alone when `noinline` is
  also present; otherwise `-O0` gives `noinline` (a `minsize`
  function); above `-O0`
  `noinline` (`[[gnu::noinline]]`, `[[clang::noinline]]`, `__attribute__`)
  stays `noinline`, and a function `inline`-specified on any of its
  declarations, or one instantiated from a pattern that is (not for an
  explicit specialization; lambda call operators included), gets
  `inlinehint` (`Cg::inline_specified`). `constexpr` and in-class
  definitions are implicitly inline but not inline-specified, so they
  get neither. Tests: `tests/codegen/noinline_matrix_o{0,1,2}.cpp`
  (exact under oracle 19 on both targets).
- `Cg::def_opt_attrs` adds `naked` for a `naked` function and, without
  an `optnone` attribute, `minsize` for `minsize` and `optsize` for
  `cold` unless the definition is `optnone` by `-O0` (`cold` itself
  comes from `cold_hot_attrs`, also on `optnone` definitions).
  Declarations and call sites get none of `optnone`, `minsize`,
  `naked`. A function with `disable_tail_calls` on any declaration has
  `"disable-tail-calls"="true"` on its declaration and its definition,
  not on call sites (`Cg::target_attrs_of`, clang
  `ConstructAttributeList`; WP-682,
  `tests/codegen/disable_tail_calls_o2.cpp`). A naked definition (`Fx::naked`) has no parameter prologue
  (no parameter or `this` slots: clang `EmitFunctionProlog` returns
  early) and its epilogue is `unreachable` instead of a return (clang
  `EmitFunctionEpilog`); falling off a value-returning naked function
  still traps first at `-O0`. Falling off the end of any value-returning
  function is `llvm.trap` + `unreachable` at `-O0` and `unreachable`
  alone above it (clang `GenerateCode`). Tests:
  `tests/codegen/optnone_minsize_naked_o{0,1}.cpp` (exact under oracle
  19, structural under 22, on both targets).
- A naked function returning indirectly keeps its `sret` parameter
  (`dead_on_unwind writable sret(T) align N`) but without `noalias`,
  which clang's `EmitFunctionProlog` adds only after the naked early
  return; nothing stores the `sret` pointer (WP-412). Register variables
  with an asm label in a naked body are ordinary `alloca`s whose asm
  operands are pinned to `{reg}` (§7c). Tests:
  `tests/codegen/naked_sret{,_o1}.cpp`, `tests/codegen/naked_asm_regvar.cpp`
  (per-target register names: AArch64 `x0`/`w1`, x86-64 `rdi`/`esi`).
- A template parameter object is emitted `linkonce_odr` with a comdat
  and `dso_local`, or `internal` when its type or value names an
  internal entity (spec/abi.md §8.22; clang 22 drops the comdat of an
  internal one, kiln keeps it below `-fclang-compat=22`); it carries no
  `align`, as clang's. A specialization taking such an object is
  `internal` with no comdat at both levels (`set_def_linkage`), so
  `linkage_internal_nttp` matches clang 22 with no deviation row
  (WP-413).

### Size optimization attributes (WP-412)
- `CodegenOptions::opt_size` is clang's `CodeGenOpts.OptimizeSize`: 1 at
  `-Os`, 2 at `-Oz` (the driver's `opt_size`). `Cg::size_attrs` gives
  `optsize`, and `minsize` too at 2 (clang
  `getTrivialDefaultFunctionAttributes`). They come with
  `Cg::target_attrs`, so every function that has the target attributes
  gets them: definitions (user functions, structor variants, thunks,
  initialization functions, TLS wrappers, `__clang_call_terminate`) and
  declarations (`Cg::declare_fn`, library builtins). Runtime functions
  (`rt_fn`: `__cxa_atexit`, `__cxa_allocate_exception`, `__cxa_throw`, …)
  and intrinsics have neither. An `optnone` function drops them: its
  definition (`DefInline::OptNone`, clang
  `SetLLVMFunctionAttributesForDefinition` removes them) and its
  declaration (`Cg::optnone_fns`).
- Call sites through clang `EmitCall` (`Fx::call_vals_to`: direct,
  indirect and virtual calls, structor calls; replaceable allocation
  calls) get `Cg::call_size_attrs`: the size attributes unless the
  callee is in `Cg::optnone_fns`. Runtime calls, inline asm, intrinsic
  calls and the plain calls of `__cxx_global_var_init` /
  `__clang_call_terminate` / `_ZTH` have none.
- Initialization functions (`__cxx_global_var_init`,
  `_GLOBAL__sub_I_*`) and the `thread_local` wrapper `_ZTW*` are
  `noinline` only at `-O0`.
- At `-Oz` a non-array delete-expression that does not deallocate
  through a virtual deleting destructor or a destroying `operator
  delete` calls `operator delete` unconditionally in the null check's
  end block (clang `EmitObjectDelete`: "When optimizing for size, call
  'operator delete' unconditionally").
- Tests: `tests/codegen/optsize_os.cpp`, `optsize_oz.cpp`,
  `delete_oz.cpp` (exact under oracle 19, structural under 22, both
  targets).

### clang `annotate` (WP-1113, `annotate.rs`)
clang's `[[clang::annotate("str", args...)]]` (sema: spec/sema.md §4.5)
has three lowerings, the same in clang 19 and 22, so `-fclang-compat=19`
changes nothing:
- **Globals and functions:** one `{ptr, ptr, ptr, i32, ptr}` entry per
  attribute in `@llvm.global.annotations` (appending, section
  `llvm.metadata`). It is emitted after `@llvm.global_dtors` and before the
  coverage globals and `@llvm.used` (clang `Release`).
  - A variable is annotated when it is defined, a static local after its
    initializer (clang `AddGlobalAnnotations`).
  - A function is recorded when it is first created, then annotated at the
    end of the module with its latest redeclaration (clang
    `DeferredAnnotations`). Structors are annotated as C2/D2; thunks are
    not.
  - Inherited attributes come before a declaration's own. An inherited one
    whose string the declaration already has is dropped (clang
    `mergeDeclAttributes`).
- **Locals and parameters:** `call void @llvm.var.annotation.p0.p0(ptr
  %addr, ptr str, ptr file, i32 line, ptr args)` after the alloca and its
  `dbg.declare`, before the initializer; for a parameter, after its store.
- **Fields:** each annotation of the member chains
  `@llvm.ptr.annotation.p0.p0` on the member's address. It comes after the
  GEP, after the load for a reference member, and on the base address for
  a union member. A bit-field and the binding of a reference member get
  none.
- **Operands, in clang's evaluation order (string, file, line, args):**
  - The strings and file names share one cache of private `unnamed_addr`
    `.str` constants in `llvm.metadata`, with no `align`.
  - The file and line are presumed (`#line` applies).
  - Args are `ptr null` when there are none. Otherwise they are a private
    `.args` constant struct of the arguments' constant values (clang
    `emitAbstract`: `bool` is `i1`), shared by arguments whose values have
    the same clang `APValue` profile. Integers are profiled in 32-bit
    chunks, so `1`, `1u` and `true` share one struct and `1L` does not; a
    string argument is an ordinary literal and so never shares.
- **Statements:** clang 22 accepts `annotate` on a statement and emits
  nothing for it, and neither does kiln.
- Details and deviations: spec/landed/codegen/WP-1113.md.

### TBAA (WP-412, `tbaa.rs`)
- **When.** `Cg::tbaa_on`: `CodegenOptions::optimize && strict_aliasing`
  (clang creates `CodeGenTBAA` iff `OptimizationLevel > 0 &&
  !RelaxedAliasing`), so `-O0` and `-fno-strict-aliasing` emit none.
  Bodies made `optnone` by the attribute are still tagged. The rule
  relied on is [basic.lval]/11.
- **Nodes** (clang `CodeGenTBAA`, built lazily and cached on
  `Cg::tbaa`): the root `!{!"Simple C++ TBAA"}`; scalar nodes
  `!{name, parent, i64 0}` under "omnipotent char" (the root's child).
  A builtin is named for its signed variant (`int`, `long`, `long long`,
  `__int128`, `short`, `float`, `double`, `long double`, `wchar_t`,
  `char8_t`, `char16_t`, `char32_t`, `bool`, `_Float16`, `__fp16`,
  `std::nullptr_t`); `char`, `signed char` and `unsigned char` are
  "omnipotent char"; `_BitInt(N)` is "_BitInt(N)". An externally
  visible enumeration is its `_ZTS` name, one in an unnamed namespace
  and `std::byte` are "omnipotent char". A pointer or reference is
  "any pointer" (child of char) below `-fclang-compat=22`; at 22 it is
  `"p<depth> <pointee>"` under `"any p<depth> pointer"` (pointer TBAA:
  depth counts pointer levels through arrays; the pointee is the
  builtin's name, "omnipotent char" for the char types, or a named
  class's `_ZTS` name; `void`, enumerations, functions, member pointers,
  `_BitInt`, complex, vectors, atomics and unnamed classes give the
  `any p<depth> pointer` node itself). Everything else (complex,
  vectors, member pointers, arrays, unions) is "omnipotent char". A
  vtable pointer is "vtable pointer" (child of the root).
- **`may_alias`** (WP-423; `Cg::tbaa_may_alias`, clang
  `TypeHasMayAlias`): a type whose class or enumeration carries GNU
  `may_alias` (on any declaration), or with a typedef-name carrying it
  anywhere in its sugar (`typedef int __attribute__((may_alias)) T;`,
  `using T [[gnu::may_alias]] = int;`, a typedef of such a typedef), is
  "omnipotent char" as an access type, whatever its canonical type (a
  pointer typedef too; checked before the canonical-type cache). A
  member of a may-alias class is a may-alias access (`{char, char, 0}`,
  clang `EmitLValueForField`); a may-alias class used as a member type
  keeps its struct node. In `!tbaa.struct` every scalar under a
  may-alias type is "omnipotent char" (`CollectFields`' `MayAlias`).
  Sema accepts the attribute on any declaration with no diagnostic
  (clang `MayAliasAttr` has no subject list); it takes no arguments,
  `[[gnu::may_alias]]` at the end of a decl-specifier-seq is no type
  attribute, and after a ptr-operator the GNU spelling applies to the
  declaration (`int *__attribute__((may_alias)) p`), as clang's
  `ProcessDeclAttributes` does for every GNU declaration attribute in a
  declarator chunk (`Parser::chunk_decl_attrs`).
- **Atomics** (WP-423; `atomic.rs`, clang CGAtomic): `_Atomic(T)` is
  "omnipotent char" as an access type. The `load atomic` / `store
  atomic` of an `_Atomic` lvalue (`EmitAtomicLoadOp`, `EmitAtomicStore`,
  scalar or aggregate) and the plain store of its initialization
  (`EmitAtomicInit` → `emitCopyIntoMemory`) carry the lvalue's tag
  (`{char, char, 0}`, or the struct path of an `_Atomic` member). The
  operation of an `__atomic_*` / `__c11_atomic_*` builtin (load, store,
  `atomicrmw`, `cmpxchg`, libcall) is untagged; its result is read back
  from the temporary with the value type's tag, and an aggregate result
  is copied out with the value type's `!tbaa.struct`.
  `tests/codegen/tbaa_atomic_o1.cpp`.
- **Struct nodes** (`getBaseTypeInfo`): a complete non-union class with
  no flexible array member is `!{!"_ZTS<name>", member, i64 off, ...}`
  listing its non-virtual non-empty bases by offset, then its fields:
  unnamed bit-fields and empty `[[no_unique_address]]` members are
  skipped, a bit-field is listed at its byte offset with its type node.
- **Access tags** (`Tb`, carried on `LV::tb` as clang's
  `LValue::getTBAAInfo`): `{T, T, 0}` for a plain access; a `.`/`->`
  member chain accumulates `{outermost class, member type, offset}`
  (`Cg::tb_field`); a member of a union, a vector-typed member and any
  access below a may-alias path are `{char, char, 0}`; a bit-field
  access is untagged. Array subscripts and derived-to-base conversions
  restart at the plain tag (`Tb::sub`, keeping may-alias), as does a
  member of a class that is no valid base. A conditional lvalue keeps
  its operands' common tag, else may-alias; `__builtin_bit_cast` reads
  may-alias. A reference member's slot load carries the member's path
  and its referee the plain tag; a reference's own slot (variables,
  members, captures) is accessed as the reference type, and a
  constructor's member initializers store with the member's path.
  Parameter spills (the `this` spill as the class pointer, the VTT spill
  as `void **`) are tagged; the `this` reload, the return-value slot of
  a direct return, complex parts, vector element read-modify-writes and
  `vec3`/bool-vector loads are not. `bool` loads also carry
  `!range !{i8 0, i8 2}` and `!noundef`. The scalar zero store of a
  value-initialization (`null_init_lvalue`: aggregate-init array filler
  loops, members an aggregate initializer omits, `new T()`) carries the
  lvalue's tag (clang `EmitStoreThroughLValue`, WP-431;
  `tests/codegen/tbaa_null_init_o1.cpp`).
- **Vtable pointers** (`Cg::tbaa_vptr`, clang `GetVTablePtr` /
  `InitializeVTablePointer`): the constructor and destructor vptr stores
  and every vptr load (virtual calls, `dynamic_cast` to `void*`,
  `typeid`, virtual base offsets, virtual member-function-pointer calls,
  `::delete` of a polymorphic object, virtual thunks).
- **`!tbaa.struct`** (`getTBAAStructInfo`, attached by the aggregate
  copies of `EmitAggregateCopy` only: trivial copy/move construction and
  assignment, aggregate-copy expressions): `(i64 off, i64 size, tag)`
  per scalar field, nested classes flattened, empty fields skipped; a
  union, array or vector field is one "omnipotent char" entry of its
  size, a run of bit-fields one at its first member's offset with the
  storage size; a class with a base or a flexible array member gets
  none. A copy constructor's memberwise field-range `memcpy` gets none.
- At `-fclang-compat=22` the module also gets `!llvm.errno.tbaa =
  !{int tag}` (clang 22 `EmitModuleMetadata`).
- Thunks get their method's definition attributes above `-O0`
  (`optnone`, `inlinehint`: clang `maybeEmitThunk` calls
  `SetLLVMFunctionAttributesForDefinition`); at `-O0` they are
  `noinline optnone`.
- Tests: `tests/codegen/tbaa_scalar_o1.cpp`, `tbaa_struct_path_o1.cpp`,
  `tbaa_vptr_o1.cpp`, `tbaa_copy_o1.cpp`, `tbaa_off_o1.cpp` (exact under
  oracle 19, which compares metadata, on both targets; oracle 22 strips
  metadata, so the clang 22 pointer nodes were checked by hand).

### Always-inliner (WP-389)
- clang's `-O0` pipeline (`buildO0DefaultPipeline`) runs
  `AlwaysInlinerPass` (without lifetime markers) before `-emit-llvm`
  writes the module, so its `-O0` IR has every call to an `always_inline`
  definition inlined and such a discardable definition deleted. kiln
  emits the definition the way clang's codegen does (`alwaysinline` in
  place of `noinline optnone`; with `noinline` also present, `noinline`
  alone; `Cg::def_inline_attrs`) and the driver runs LLVM's own
  pipeline, `opt -passes=default<O0>`, over a module whose attribute
  groups hold `alwaysinline` (spec/driver.md "IR optimization"). Writing an
  inliner over kiln's IR was the alternative; running LLVM's pass keeps
  `ir_vs_clang` exact by construction (alloca hoisting, block splitting
  and PHIs, `noalias` scope metadata for `sret`, invoke unwinding, the
  attribute merge and dead-function removal all come from the same
  code), and it only runs for modules that need it. Test:
  `tests/codegen/always_inline.cpp` (exact under oracle 19, structural
  under 22).
- **Cost (WP-397).** libstdc++'s and libc++'s headers define
  `always_inline` functions that almost every TU uses, so the pass runs
  for nearly every `-O0` compile. Measured on a TU including
  `<string>`, `<vector>` and `<algorithm>` (`-O0 -c`, harness build,
  aarch64, 20 interleaved runs, median wall clock; peak RSS ~64 MiB,
  the frontend's, in every configuration): libstdc++ 0.339 s without
  the pass, 0.362 s (+6.8%) with WP-389's two `opt` runs; libc++
  0.445 s / 0.478 s (+7.4%). The pass itself is not the cost: `opt
  -passes=always-inline` takes as long as `default<O0>` (process start
  ~10 ms, IR parse ~12 ms) and inserts lifetime markers clang's O0
  pipeline does not, so it is not used; skipping `opt` when no
  `always_inline` function is called would never fire, as kiln defines
  such a function only when it is used. The cost was the second `opt`
  (bitcode back to text, ~17 ms) and `llc` reparsing text, so for an
  object `llc` now reads the first run's bitcode directly (spec/driver.md
  "IR optimization"): libstdc++ 0.359 s (+5.9%), libc++ 0.462 s
  (+3.8%), a fixed ~20 ms per TU that shrinks relative to larger TUs.
  The objects are byte-identical at `-O0`; above `-O0` the bitcode
  keeps the in-memory use-list order, which makes `llc`'s output match
  clang's in-process codegen where the text round trip did not.

### Deviations
- **clang 22 codegen (WP-355, WP-386).** At `-fclang-compat=22` kiln
  follows clang 22 where §6a says; elsewhere its IR is clang 19's under
  every level. The oracle-22 `DEVIATIONS` entries record the rest of
  what clang 20-22 changed: AArch64 aggregates of pointers
  coerced to `ptr`/`[N x ptr]` (19: integers), `(unnamed struct)` in IR
  type names, `llvm.modf`, `llvm.thread.pointer.p0` (kiln names the
  intrinsic for the LLVM it drives, `llvm_major`), and `-O0` instruction
  choices (atomic temporaries, `select` for vector `?:`, constant-folded
  reductions, `undef` widening shuffles, `delete` block order). clang
  22's IR for a `bool` vector element store fails verification
  (`vector_ops`). At `-fclang-compat=22` (only), as clang 22 does, a
  padded `_Atomic` operand's atomic-sized temporary is zeroed before the
  operand is copied in (P0528), and a failed compare-exchange writes the
  old value back to `*expected` itself (C11 7.17.7.4p2) and not only to
  that temporary (`convertToAtomicIntPointer`, WP-411). clang 19 does
  neither, and nor does kiln below 22, so `atomic_padded` is exact under
  both oracles. Following the rest under
  `-fclang-compat=22` is a follow-up (docs/status/wp-184-oracle-switch.md,
  "Residual"). x86-64 `__int128` / `_BitInt(128)` as `i128` (19: two
  `i64`s) is followed at 22 since WP-458 (spec/abi.md §8.25).
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
  and LCSSA `phi`s (and the incoming order of existing `phi`s changes).
  kiln emits the plain loops, as clang 22 and clang 19 on x86_64 do: the
  reshaping is an LLVM 19 AArch64 pass-pipeline artifact, not frontend
  output, so kiln does not reproduce it (WP-189). `ir_vs_clang` marks
  the affected aarch64/oracle-19 cases `Allow::LoopCanon`
  (`global_init_array_eh`, `eh_array_destroy_throwing_global`,
  `array_temporary_global`): kiln's IR is run through `opt
  -passes=loop-simplify,lcssa` (which leaves `optnone` functions alone,
  as the pipeline does) and its local names are renumbered in definition
  order (`ir_norm::unname_locals`, so the pass-created `.preheader`/
  `.loopexit` blocks get the numbers clang's do), after which the case
  must match clang 19 byte for byte. x86_64 and oracle 22 are exact
  without it.
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
- **Record types** are `kiln_abi::IrTypes` lowerings (spec/abi.md §8.6),
  so a pointer field is `ptr` and no member is ever `i0`, even for a
  record nested in a member class declared before its definition
  (WP-582, spec/landed/codegen/WP-582.md).
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
  throwing one waits for EH. The placement argument's alignment is
  used only when its source is a declaration (§3.3, as clang
  `EmitCXXNewExpr`); otherwise the allocated type's. Corpus cases do not use `if (c && new T)` (clang folds the
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

## 6a. Landed (WP-386): clang 22 codegen at `-fclang-compat=22`
`CodegenOptions` carries `clang_compat` (the driver's `-fclang-compat`)
and `llvm_major` (the selected LLVM installation's major, spec/driver.md
"LLVM discovery"). What clang 20-22 changed is
keyed on the compat level, and additionally on the backend's major where
LLVM 19 rejects the syntax (checked with `llvm-as` of both toolchains):

| clang 22 | key | clang 19 / older backend |
|---|---|---|
| AArch64 data layout `e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32` (`kiln_llvm::AARCH64_LINUX_GNU_22`; LLVM 19 accepts it) | compat ≥ 22 | `e-m:e-i8:8:32-...` |
| AArch64 empty class by value as `i64` (spec/abi.md §8.21) | compat ≥ 22 | `i8` / ignored |
| x86-64 `__int128`, `_BitInt(128)` and a class starting with an `i128` by value and returned as `i128` (spec/abi.md §8.25) | compat ≥ 22 | `{ i64, i64 }` / two `i64`s |
| GEP `nuw` (§3 "GEP flags") | compat ≥ 22 | `inbounds` only |
| `dead_on_return` on an indirect, non-`byval` parameter whose type has a trivial destructor (AArch64 aggregates over 16 bytes, non-trivially-copyable classes, large vectors) or which the callee destroys (a `[[clang::trivial_abi]]` class with a non-trivial destructor, `kiln_abi::call::is_destroyed_in_callee`), on definitions, declarations and calls (a variadic argument too), before `noundef`; a class the caller destroys gets none (WP-774, `tests/conformance/expr.call/ir-clang22-dead-on-return-variadic.cpp`) | compat ≥ 22 and LLVM ≥ 21 | none (LLVM 19's `llvm-as` rejects it) |
| `"frame-pointer"="non-leaf-no-reserve"` and module flag `frame-pointer` 4 for `non-leaf` (AArch64's default; "Frame pointer") | compat ≥ 22 and LLVM ≥ 22 | `"non-leaf"`, 1 (LLVM 19 rejects the value) |
| no empty `!llvm.linker.options` | compat ≥ 22 | `!llvm.linker.options = !{}` in C++20 and later only (clang 19 emits it when the language has modules, `LangOptions::Modules` set by `CPlusPlusModules`; `CodegenOptions::cxx_modules`, WP-769) |
| a padded `_Atomic` operand's atomic-sized temporary zeroed before the copy, and a failed compare-exchange's old value copied back to `*expected` (`atomic.rs`, WP-411; `tests/codegen/atomic_padded.cpp`) | compat ≥ 22 | the temporary's padding left uninitialized; the old value stored only to the temporary |
| `captures(none)` for `nocapture` (intrinsic declarations such as `llvm.memcpy`/`llvm.memset`; `kiln_llvm` `Module::ir_version`, spec/llvm.md §5) | compat ≥ 22 and LLVM ≥ 21 | `nocapture` (LLVM 19 rejects `captures(..)`) |
| `!nonnull`/`!align` on reference loads (§3 "Reference loads") | compat ≥ 22 | none |
| each `_ZTI` global before its `_ZTS`, `__cxxabiv1` vtable and pointee/base descriptors (`Cg::reserve_rtti`) | compat ≥ 22 | `_ZTI` after them |

x86-64's data layout and `"frame-pointer"="all"` are the same in both.
The `xtask test` harness runs kiln at the oracle's compat level with the
oracle's LLVM (spec/harness.md "Oracle toolchain"), so the oracle-22 legs
see all eleven. The `module.rs` hunk is `Cg::new` (data layout, `IrTypes`
compat, `ir_version`), `sig_attrs`, `target_attrs` and `trailer`. Not
yet: LLVM 22's `nocreateundeforpoison` function attribute on many
intrinsic declarations (`*.with.overflow`, `ctpop`, `bswap`, `fabs`,
`fshl`, `smax`, ...; LLVM ≤ 21 rejects it). Tests:
`tests/conformance/expr.call/ir-clang22-argument-passing.cpp`,
`tests/conformance/expr.sub/ir-clang22-gep-flags.cpp`,
`tests/conformance/dcl.ref/ir-clang22-reference-loads.cpp`,
`tests/conformance/expr.typeid/ir-clang22-typeinfo-order.cpp`,
`tests/conformance/dcl.array/ir-clang22-vla-pointer-arithmetic.cpp`
(`expect: ir`), and `ir_vs_clang` under oracle 22.

## 7. Landed (WP-079): classes
Lowering of non-polymorphic classes (`class.rs`, `cleanup.rs`,
`memptr.rs`), mirroring clang 19 `-O0` on both targets. Polymorphic
classes and virtual bases are WP-080: every path that needs a vtable
pointer, a VTT or a virtual call reports a `CodegenError` naming WP-080.
- **Structors** (clang `ItaniumCXXABI::emitCXXStructor`): constructors
  and destructors are emitted as the base variant (C2/D2) and then the
  complete variant (C1/D1). A discardable C1 is replaced by C2 (RAUW);
  otherwise C1 is an `alias` to C2. A GNU `weak` structor is never
  discardable (its linkage is `weak`, inline or instantiated): C2/D2 and
  D0 are `weak` in the `C5`/`D5` COMDAT and C1/D1 are `weak` aliases
  (clang `getCodegenToUse` -> `COMDAT`); a `weak` written only on the
  out-of-line definition of a class template's member reaches its
  instantiations. clang's `applyReplacements` moves
  C2 to where the C1 declaration was created: when C2 already exists at
  the first C1 reference, `structor_ref` moves it to the end of the
  function list (`Module::move_function_to_end`, WP-113). Deleting destructors (D0) are WP-080.
  Structor references go through `structor_ref`. A cache hit on the base
  variant still queues it for definition, because clang's `EmitDeferred`
  is depth-first. The D1 alias has the destructor's visibility (§8).
- **Base-object destructor aliases** (WP-423; `Cg::try_base_dtor_alias`,
  clang `CodeGenModule::TryEmitBaseDestructorAsAlias`): at -O1+ only, and
  not for a `weak_odr` (explicit-instantiation, COMDAT) D2, a destructor whose body is an empty
  compound statement, in a class with no virtual bases and no field of a
  class type with a destructor, whose unique non-virtual base with a
  non-trivial destructor sits at offset 0, gets no D2 body: its D2 is
  the base's D2. The check runs before D2 is declared, and the base D2
  is declared first, as in clang. A discardable D2 (`linkonce_odr`,
  `available_externally`, `internal`) is replaced (RAUW,
  `Module::replace_function`) by the base D2, unless that is an
  `available_externally` `always_inline` one (WP-431;
  `tests/codegen/extern_template_dtor_alias_o1.cpp`). Otherwise D2 becomes an `alias`
  (`unnamed_addr`, external, `dso_local`, D2's visibility) of the base
  D2, unless that is only a declaration or weak for the linker
  (`linkonce_odr`, `weak_odr`). D1 then aliases (or is replaced by) the
  resulting D2 symbol; LLVM 19's `GlobalOpt` folds the alias chain.
  `tests/codegen/dtor_base_alias_o1.cpp`.
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
    ignored. This holds for a static member function used as a value,
    not called (`std::sort(b, e, arr.less)`, `int (*p)(int) = o.f;`):
    the member expression is the function's lvalue ([expr.ref]/6.3.1;
    WP-710, `tests/conformance/expr.ref/p6-static-member-function-value-run.cpp`).
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
    clang's `getMemberPointerPathAdjustment`; for a member of an
    anonymous union or struct (an `IndirectField`, [class.union.anon])
    the offset sums its access path (`ASTContext::getFieldOffset`,
    WP-716);
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
- A `CXXDefaultInitExpr` is emitted with the enclosing `this`, except
  inside an aggregate's initializer list, where `this` (and its class)
  is the aggregate being initialized ([class.mem.general]/9; clang's
  `FieldConstructionScope` and `CXXDefaultInitExprScope`):
  `record_init` sets `FuncState::default_init_this` for its fields and
  `enter_default_init` swaps it in while the default member initializer
  is emitted. Before WP-792 a `[this]` capture there panicked in a free
  function and captured the enclosing closure in a lambda. Test:
  `dcl.init.aggr/default-member-init-this-in-aggregate.cpp`.
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
  At `-O1`+ (WP-449; clang `EmitDeferredVTables` /
  `shouldEmitAvailableExternallyVTable`) a used vtable that is external
  for either reason is instead defined `available_externally` (no
  comdat, not `dso_local`) when it can be emitted speculatively
  (`Cg::can_speculatively_emit_vtable`, clang
  `canSpeculativelyEmitVTable`): no inline (or deleted) virtual function
  in a used slot is still unemitted (looked up by symbol: a function
  definition or an alias), and for a class with virtual bases every
  dynamic non-virtual base and every dynamic virtual base passes the same
  test, since the VTT and construction vtables are emitted with it under
  the same linkage. One that fails the test when first considered is
  retried after every deferred definition is emitted
  (`emit_vtables_opportunistically`). Its RTTI stays an external
  declaration. A thunk the vtable needs whose method has no definition
  here is defined `available_externally` with it (clang `maybeEmitThunk`
  with `ForVTable`; at `-O1`+ for any vtable), except for a variadic
  method or one with an incomplete class parameter or return type,
  which stay declarations. clang's `isVTableHidden` exclusion applies
  first: a vtable that would be hidden stays external (§8, WP-544).
  `tests/codegen/extern_template_vtable_o1.cpp`.
- **Explicit instantiation declarations** ([temp.explicit]/14, WP-158;
  `Cg::is_explicit_inst_declaration`): a function or variable that is
  an `extern template` specialization, or a member of a class
  specialization under one (the innermost explicitly instantiated
  enclosing class decides), gets no definition at `-O0`, even when inline
  (clang would make it `available_externally` only when optimizing);
  references name the external symbol, structor calls the complete
  variant (D1/C1). Implicitly declared special members are not
  instantiations and are still emitted.
  An inline function carrying `always_inline` on any redeclaration
  (`Cg::is_always_inline_fn`) is the exception (WP-373, WP-389): as clang
  (`GVA_AvailableExternally`, kept at `-O0` only for `AlwaysInlineAttr`),
  kiln emits its definition `available_externally` (no comdat, not
  `dso_local`; `Cg::is_available_externally_fn`) and the driver's O0
  pipeline inlines every call to it and deletes it (§6 "Always-inliner"),
  so the program never references the symbol. libstdc++'s
  `allocator<char>::allocate`/`deallocate` are such members under
  `extern template class allocator<char>`, and libstdc++.so does not
  export them. Tests:
  `tests/conformance/temp.explicit/extern-template-always-inline-run.cpp`,
  `tests/conformance/class.copy.ctor/libstdcxx-string-copy.cpp`.
  At `-O1`+ (WP-431; clang `shouldEmitFunction`, which then drops only
  `noinline` ones) every used function whose definition
  [temp.explicit]/12 instantiates under the declaration is emitted
  `available_externally` the same way (`Cg::keeps_available_externally`):
  one whose pattern is inline (declared `inline` or `constexpr`, or
  defined in its class; sema marks an instantiation inline as its
  pattern is, spec/sema-templates.md) or that has a deduced return type,
  and that carries no `noinline`. A non-inline member and a `noinline` one stay
  declarations; a static local of an emitted one is `linkonce_odr` in
  its own comdat; C1/D1 calls go to C2/D2 (the structor alias rule).
  `tests/codegen/extern_template_avail_ext_o1.cpp`.
  At every level (also for the `always_inline` exception at -O0) a
  trivially recursive function stays a declaration (WP-449, clang
  `isTriviallyRecursive`, `Cg::is_trivially_recursive` in
  `recursion.rs`). The function's name is its asm label, else its plain
  name when it is unmangled (`extern "C"`, `main`); a mangled function
  without a label never qualifies (WP-585: clang `isTriviallyRecursive`
  falls back to the identifier only for an unmangled one).
  Its body is walked as clang's `FunctionIsDirectlyRecursive` walks it:
  every statement and expression (a declaration statement's variable
  initializers included), but not the operands of a call. A call whose
  direct callee (through parentheses, implicit casts and substituted
  non-type template parameters) carries the same label, or is the
  builtin `__builtin_<label>` of a library function makes it recursive.
  The library builtins carry `builtin::attr::LIB_FUNCTION` (clang
  `isLibFunction`, the `FunctionWithBuiltinPrefix` flag, WP-462): the
  406 `__builtin_` rows that clang 19 and 22 treat so, found by probing
  each one in an `extern template` labelled with its library name. The
  generic classification and comparison builtins (`__builtin_isnan`,
  `__builtin_fpclassify`, ...), `__builtin_powi`, `__builtin_abort` and
  the `_Float16`/`__float128` math rows are among them; the bit
  operations (`__builtin_clz`, `__builtin_popcount`), `__builtin_expect`,
  `__builtin_trap`, the overflow, coroutine, elementwise and alignment
  builtins are not.
  `tests/codegen/trivially_recursive.cpp`,
  `tests/codegen/trivially_recursive_always_inline.cpp`,
  `tests/codegen/trivially_recursive_lib.cpp`.
- **GNU inline functions** (GNU extension, WP-585; clang
  `basicGVALinkageForFunction` -> `isInlineDefinitionExternallyVisible`,
  which is false in C++ for every `gnu_inline` function; sema rule in
  spec/sema.md "GNU inline functions"): an inline function with external
  linkage that carries `gnu_inline` (`AttrKind::GNU_INLINE`, on any of
  its declarations; templates, members and `constexpr` functions
  included) is defined in another translation unit, exactly like a
  function under an explicit instantiation declaration
  (`Cg::defined_elsewhere`, `Cg::is_gnu_inline_fn`): its body is
  emitted `available_externally` under the same conditions
  (`always_inline` at -O0; at -O1+ any not `noinline`; never when
  trivially recursive), otherwise it is only declared. It is never a
  `linkonce_odr` definition of its symbol. One with internal linkage
  (`static`, unnamed namespace) is defined as usual. The shape that
  matters is the C library's optimized-only C++ overload
  (`extern "C++" const char *strchr(const char *, int) __asm("strchr")`,
  `__extern_always_inline` = `extern __inline
  __attribute__((__always_inline__, __gnu_inline__))`, body
  `return __builtin_strchr(s, c);`): it is trivially recursive and
  stays a declaration. Emitted as a definition (kiln before WP-585) it
  replaced the C library's `strchr` in the program with a function
  that calls itself, an infinite loop at -O1 and undefined behaviour
  (`mustprogress`) at -O2. `tests/codegen/gnu_inline.cpp`,
  `tests/codegen/gnu_inline_o1.cpp`,
  `tests/conformance/ext.gnu.inline/available-externally-O1-run.cpp`,
  `tests/conformance/ext.gnu.inline/available-externally-O2-run.cpp`.
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
  `InstantiateClassMembers`). A dependent member (the partial
  specialization of a member class template) is not emitted, and a
  member of an implicit specialization of a member template
  (`X<int>::M<char>`) takes that specialization's kind, not the enclosing
  explicit instantiation's (`governing_explicit_inst_kind`; `linkonce_odr`,
  emitted only if used; WP-812, `explicit_inst_member_partial_spec.cpp`).
  A member explicit instantiation (`template void X<int>::f();`,
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
  refer to the runtime's `external constant` objects. Vector types (GNU and
  NEON) and `_Complex` types are not in the runtime: their
  `__fundamental_type_info` `{vt, name}` is emitted `linkonce_odr` where
  used (WP-709). RTTI of a type that
  involves an incomplete class has `internal` linkage. Linkage of class
  RTTI follows the vtable.
  - *Under `-fno-rtti`* (`CodegenOptions::rtti` false; WP-720): a
    vtable's (and construction vtable's) RTTI slot is `ptr null`, as is
    any `typeid`/`dynamic_cast` operand sema let through (clang
    `shouldEmitRTTI(ForEH=false)`, `Cg::rtti_descriptor`). Exception
    handling (`throw`, handler types) still builds its `type_info`
    objects, with their bases and pointees, but never refers to a
    key-function TU's (clang `ShouldUseExternalRTTIDescriptor` is false
    without RTTI: that TU may be `-fno-rtti` too): an external-linkage
    type's `type_info` is a `linkonce_odr` definition in every TU that
    uses it (clang `getTypeInfoLinkage`); fundamental types still refer
    to the runtime's. A kiln `-fno-rtti` object therefore links against
    a clang `-fno-rtti` library (leveldb, docs/status/oss/leveldb.md).
    Tests: `tests/conformance/except.handle/ir-fno-rtti-type-info.cpp`
    (and its `-19` sibling), `fno-rtti-catch-by-base.cpp` (run).
- **Constant objects of polymorphic classes** (`consts.rs`
  `record_elems`, WP-400): a constant initializer of a dynamic class
  gives each vptr field the vtable address point of its subobject in the
  complete object's vtable (the group `vtable_address_point(complete,
  subobject)` names; a base subobject never uses its own class's
  vtable), as clang's `ConstantEmitter` does. A class with virtual bases
  (constant only through P3533, kiln alone) emits its virtual-base
  members of the complete-object type the same way, at their layout
  offsets. A base laid out with a primary virtual base (nearly empty,
  Itanium C++ ABI 2.4: clang's `accumulateBases` makes it slot 0 of the
  base's type, with no separate vptr slot) holds just the vptr there: the
  address point of the base subobject, which the virtual base shares
  when it is the complete object's indirect primary base and which is
  the base's own otherwise (a shared virtual base reached through two
  bases, WP-902). clang 19 and 22 initialize such objects dynamically
  (`zeroinitializer` and a `__cxx_global_var_init`); kiln's constant is
  the object those constructors build. An array whose elements all have one constant type uses it as
  the element type (clang `CommonElementType`), so an array of a dynamic
  class is `[N x %struct.P]` and not a literal struct. Validation:
  `tests/codegen/polymorphic_const_global.cpp` (exact IR),
  `tests/conformance/dcl.constinit/constinit-virtual-base-run-cxx26.cpp`,
  `tests/conformance/class.mi/shared-virtual-base-constant-init-run.cpp`.
- **vptr stores**: constructors store the address point of every vptr
  after the base initializers and before the member initializers;
  destructors re-store them on entry unless clang's
  `CanSkipVTablePointerInitialization` holds (trivial body, no
  members needing destruction).
- **Virtual calls** (`CGCXXABI::getVirtualFunctionPointer`): the
  arguments are emitted first, then the vtable load (tagged "vtable pointer"
  above `-O0`, §TBAA), the slot GEP and load. Calls are devirtualized when the class
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
  A destroying `operator delete` changes both: see §7u (WP-457).
- **Thunks** (`CodeGenVTables::maybeEmitThunk`): a thunk is declared
  when the vtable that needs it is built and defined right after its
  target (method, D1 or D0) is emitted. `this`-adjusting (`_ZThn`) and
  return-adjusting (`_ZTch`, with a null check and phi for pointers)
  thunks spill and reload their parameters like any function, adjust
  `this`, and forward the arguments with a `tail call` (not `musttail`).
  Attributes follow clang's IsThunk rules: `this` keeps only `noundef`,
  and a reference return keeps only `noundef`. A thunk is `nounwind`
  only by its method's type or attributes: no `TryMarkNoThrow` over its
  body (clang `generateThunk`; WP-432, `thunk_attrs{,_noexc}.cpp`).
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
  `index` is the slot of the function the pointer names, so a covariant
  overrider's result is adjusted by the slot's entry: `&Base::f` reaches
  the return-adjusting thunk in the base's slot, `&Derived::f` the slot
  the overrider adds for its own return type (no adjustment), as clang
  does (`virtual_covariant_memptr`, WP-414).
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

**Vtables and VTTs of virtual lattices (WP-414).** Codegen asks for a
class's vtable group, construction groups, VTT and subobject table once
per structor and vptr store, so `Module.vts` (a `kiln_abi::VTableSession`,
spec/abi.md §8.5) caches them for the module; the IR is unchanged. The
virtual-base list of a class for the vptr stores (`class.rs`
`collect_vbases_in`) walks each class once, and the `__vmi_class_type_info`
flags (`vtable.rs` `VmiWalk`, clang `ComputeVMIClassTypeInfoFlags`) memoise
the (class, virtual?) edges already seen, so neither is exponential in the
number of paths through a lattice of virtual diamonds. Bench
`bench/frontend/virtual-lattice-emit.cpp` (D<N> : B<N>, C<N>; B<N>, C<N> :
virtual D<N-1>), `-emit-llvm`, release, depth 16: 1.37 s before, 0.40 s
after, clang 22 12.1 s; depth 20: 5.0 s before, 0.8 s after; depth 32:
4.3 s (clang over 120 s).

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
  `__cxa_begin_catch` then `std::terminate`). A destructor without a
  noexcept-specifier is non-throwing by [except.spec]/8 even when
  nothing in the TU uses it: sema resolves the exception specification
  of every definition (spec/sema.md §4.7, WP-431;
  `tests/codegen/dtor_implicit_noexcept.cpp`, `_o1.cpp`).
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
  `CXXBindTemporaryExpr` written in place). An ignored expression's slot
  never is (`emit_ignored` clears `dest_destructed`, clang
  `AggValueSlot::ignored()`): the left operand of a comma initializing a
  class variable (`P q = ((void)D{1}, P{2});`) still destroys its
  temporary (WP-1117; `class.temporary/discarded-not-extended-run.cpp`).
- **new-expressions** ([expr.new]/28): `CallDeleteDuringNew` as an EH
  cleanup around the initializer, with the pointer, size and placement
  arguments saved to `cond-cleanup.save` slots inside a conditional
  branch; class `operator new` keeps its declared result type. The
  reserved placement `operator new(std::size_t, void*)` is inlined, but
  its pointer is still the placement argument passed to a matching
  non-reserved `operator delete(void*, void*)` (a class-scope one).
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

**Landed (WP-757): observable checkpoints (P1494R5).**
`__builtin_observable_checkpoint()` is GCC's builtin behind libstdc++'s
`std::observable_checkpoint()` ([intro.abstract]/5,
[utility.undefined]): a `void` noexcept function builtin, reported by
`__has_builtin` (a GCC-only row, as `__builtin_is_string_literal`;
`__has_constexpr_builtin` answers 0, as clang has no row). clang has no
lowering to copy; GCC 16.1's keeps the stores before it and emits no
instruction (an opaque compiler barrier). kiln emits an
empty volatile asm statement with a memory clobber, as `asm volatile(""
::: "memory")` lowers (`emit_observable_checkpoint` in `asm.rs`):
`call void asm sideeffect "", "~{memory}"() #N` with the target's
machine clobbers appended (x86_64 `~{dirflag},~{fpsr},~{flags}`) and
`#N = { nounwind }`. LLVM treats the call as one that may read and
write all memory and may not return, so stores before it are kept and
undefined behaviour after it cannot be hoisted above it (an `unreachable`
after it does not erase them). No external call or declaration is
emitted, so LTO cannot see through it. The Oven evaluates the builtin as
a no-op, as GCC does (constant evaluation has no undefined behaviour to
bound); `std::observable_checkpoint` itself is not constexpr. Tests:
`kiln_codegen/tests/lower.rs` `observable_checkpoint_is_an_opaque_barrier`
(the IR and, with `opt-19`, the store kept at `-O2` where it is dropped
without the checkpoint); `tests/conformance/intro.abstract/`.

`__builtin_start_lifetime(p)` (WP-773, P3726R2 `std::start_lifetime`)
evaluates `p` for its side effects and emits nothing else, as GCC folds
it to `(void)p`: at run time an implicit-lifetime aggregate's lifetime
begins with its storage ([intro.object]/10), so there is nothing to do.
A constant whose union holds an array member with unconstructed elements
(spec/oven.md "C++26 union lifetime") is emitted with those elements as
zero bytes, like any byte not covered by the constant's image. Test:
`tests/conformance/obj.lifetime/start-lifetime-run.cpp`.

**Clearing padding (WP-774).** `__builtin_clear_padding(p)` (GCC; clang
has no such builtin, LLVM issue 64830) zeroes the padding bits of `*p`
and leaves every value bit alone (`clear_padding.rs`). The bits are
`kiln_abi::padding_mask` (spec/abi.md §3.3 "Padding bits"), which follows
GCC's `clear_padding_type`. `p` is evaluated once. Nested constant arrays
are flattened to their innermost element type `E` and a count `n`:
- no padding in `E`: nothing is emitted;
- `n == 1`, or the whole object is at most 64 bytes: straight-line code
  for each element;
- otherwise a loop over the elements (an `i64` byte-offset phi stepping by
  `sizeof(E)`, exit on `icmp eq` against the total, as `array_init_loop`).
Within an element, each run of whole padding bytes is one `llvm.memset`
to 0 (a lone byte is a `store i8 0`). A byte holding both value and
padding bits, as in a bit-field's storage unit, is loaded, `and`ed with
its value bits and stored back. Each access has the pointer's alignment
reduced by its offset. A volatile pointee makes every access volatile.
An element over 16 MiB is an internal error (its mask is not built). At
run time `std::atomic<T>` in libstdc++ clears padding through this builtin
(`__has_builtin` is 1, spec/pp.md). Tests:
`tests/conformance/ext.gnu.clear_padding/` (the run test's bytes are
compared with GCC 16.1's).

**Landed (WP-140): `PredefinedExpr`.** The name is a private
`unnamed_addr constant` array named `<spelling>.<mangled function>`
(`__func__._Z1fv`), as clang names it (`Module::string_global_named`);
in a constant initializer outside a function it is an ordinary `.str`
literal. `tests/codegen/predefined.cpp`.

## 7d. Landed (WP-154): atomics (`atomic.rs`)
clang 19 `CGAtomic` at -O0, mirrored:
- **`AtomicExpr`** (`emit_atomic_expr`): the pointer and its alignment
  (`EmitPointerWithAlignment`, §3.3); value operands are stored to `iN` temporaries of the atomic
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
- **Glvalue conditionals as prvalues (WP-418).** The lvalue-to-rvalue
  conversion of a glvalue `?:` of scalar type loads each arm in its own
  block and merges the loaded values in the `phi` (clang
  `VisitAbstractConditionalOperator` visits the arms as scalars): an arm
  that is a reference-returning call is loaded through its lvalue
  (`Cg::emit_arm`, `emit_glvalue_load`), never merged as its address
  (`tests/codegen/cond_glvalue_ref_call.cpp`).
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
  aarch64, 8 on x86_64) when the size exceeds it; clang's texts. At
  `-fclang-compat=19` the latter is `cg_atomic_oversized_19`, clang 19's
  text with its double space before "bytes" (clang 22 has one;
  `CodegenOptions::clang_compat`, WP-355).
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
    than the register span (`-fclang-compat` ≥ 20 only; clang 19 reads
    such a type in place), or an integer type is over-aligned
    (spec/landed/abi/WP-766.md).
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

### Constant destruction (WP-294)
A variable needs destruction (a cleanup at scope exit, a `__cxa_atexit`
registration, a guard for a static local, a non-`constant` global,
`DeclMustBeEmitted`) when its type is a class, or an array of classes,
with a non-trivial destructor, **unless it has constant destruction**
([expr.const]/7; clang `VarDecl::needsDestruction` is `DK_none` once
`Sema::FinalizeVarWithDestructor`'s `evaluateDestruction` succeeded).
Clang evaluates that for every variable (block-scope, static local,
namespace-scope or static member; not parameters, temporaries or
references) whose destructor is non-trivial and `constexpr`, implicit
ones included. "Empty body" is not the rule: a user-provided
non-`constexpr` `~U() {}` is always called, and a `constexpr` destructor
that only uses `this`, locals or constexpr helpers is not. The fold
prepass (§3, `fold.rs`) runs the destruction through the TU's Oven
(`Oven::has_constant_destruction`) and records such variables in
`Folds::const_destroyed`; `Cg::var_destructed_class` is
`destructed_class` for every other variable, and every site above uses
it. The object the destructor sees:
- a `const` variable (its type is `const`-qualified): readable and
  writable (clang `lifetimeStartedInEvaluation`), holding its
  initializer's folded value or, if that does not fold, indeterminate
  (`const A a{n};` with a destructor that only writes is elided); its
  `mutable` members are outside their lifetime, so reading or writing
  them fails ([expr.const]/5.9);
- any other variable: outside its lifetime ([expr.const]/5.16), so any
  read or write of a subobject fails (`S s;` with `~S() { x = 0; }` is
  called, with `~S() { int *p = &x; }` is not).

The destruction is manifestly constant-evaluated when the variable has
static storage and is constant-initialized
(`__builtin_is_constant_evaluated()` is then true). A leak or any
failure keeps the call. `tests/codegen/dtor_constant_destruction.cpp`
checks every case above against clang on both triples.

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
- **`init_priority(N)` (WP-189).** A namespace-scope variable carrying
  the GNU attribute (sema.md, `AttrKind::INIT_PRIORITY`, value in the
  node's `a`; on any redeclaration) is not partially-ordered or ordered:
  as clang's `EmitCXXGlobalVarDeclInitFunc`, after the thread-local
  branch (a `thread_local` ignores the priority) its init function joins
  `prioritized`. At the end of the TU the list is stable-sorted by
  priority and each priority gets one `_GLOBAL__I_%06u` function
  (internal, `.text.startup`, the `_GLOBAL__sub_I_` attributes) calling
  its init functions in definition order; the first chunk starts with
  the imported module initializers. `_GLOBAL__sub_I_<file>` is emitted
  whenever prioritized inits exist, even when empty. `@llvm.global_ctors`
  lists the partially-ordered entries (priority 65535) first, then the
  chunks with their priority, then `_GLOBAL__sub_I_`. In a module
  interface or implementation unit the prioritized inits instead run in
  the module initializer, after the imported module initializers and
  before the ordered ones (clang `EmitCXXModuleInitFunc`).
  `tests/codegen/init_priority.cpp`,
  `tests/conformance/basic.start.dynamic/init-priority-run.cpp`.
- **Local references (WP-189).** A local reference whose initializer
  folds stores the folded constant address (clang `EmitAutoVarInit`:
  `mightBeUsableInConstantExpressions`, `ConstantLValueEmitter`'s
  byte-offset GEP) instead of evaluating the glvalue; one that does not
  fold binds the evaluated glvalue. `tests/codegen/ref_local_constant_init.cpp`.
- Folding of reads of `const` class objects ([expr.const]/4,
  [basic.start.static]/3) matches clang (no corpus deviation found in
  the WP-189 triage).

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
| explicit specialization of a static data member (template) without an initializer | declaration only ([temp.expl.spec]/14; WP-572) | no definition; uses name an `external global` (`Ast::is_member_spec_declaration`) |
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

- A constructor emitted at an explicit instantiation definition creates
  its class's IR struct type before emitting member initializers (after
  the vptr stores; clang `EmitCtorPrologue`, whose
  `FieldConstructionScope` converts the record type), so the named
  struct types are declared in clang's order ([temp.explicit]/10,
  WP-189). Static data members of an explicitly instantiated class are
  `weak_odr` in their own COMDAT with their dynamic initializers in the
  ordered `_GLOBAL__sub_I_` sequence; `extern template` emits neither
  ([temp.explicit]/11).

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
- **Scalar, vector and complex compound literals** (`compound_literal_lvalue`,
  WP-302): in a function, `(int){x}`, `(v4){x, 2}` and `(_Complex double){x}`
  are clang's `EmitCompoundLiteralLValue`: the initializer is stored into a
  fresh `alloca` of the literal's type, which the scalar and complex
  emitters load (`VisitCompoundLiteralExpr` → `EmitLoadOfLValue`) and the
  lvalue emitter returns, so a materialized or subscripted literal gets a
  second temporary as in clang (`tests/codegen/compound_literal_scalar.cpp`).
  A non-constant file-scope literal is rejected by sema (`initializer
  element is not a compile-time constant`), so none reaches codegen.
- **C compound literals** (WP-1287): every C literal is an lvalue, so
  the lvalue emitter takes `compound_literal_lvalue` for every type in
  C (C++ keeps it for non-aggregates): an aggregate initializer is
  emitted into the fresh `alloca` by `emit_agg_into`. Like clang, the
  alloca gets no lifetime markers, so the object lives to the end of
  the function, which covers its block. A file-scope literal of any
  type is its `.compoundliteral` global (§7ab).
- **C VLAs** (WP-1287): a C-style cast to a variably modified type
  evaluates its bounds (`emit_vla_bounds`), and a function's parameters
  evaluate the bounds of their written (not star-decayed) types after
  the prologue (6.9.1p10), so `sizeof *a` of `int (*a)[n]` reads `n`.
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
- **Callee-destroyed arguments** (`expr.rs` `push_arg_cleanup` /
  `deactivate_arg_cleanups`, clang `CallArgList::addArgCleanupDeactivation`):
  a `trivial_abi` by-value argument gets a caller EH cleanup while the
  later arguments are evaluated; the cleanups are deactivated, innermost
  first, right before the call, and one with a later argument's cleanup
  still above it gets its `cleanup.isactive` flag (`eh.rs`
  `deactivate_cleanup_block`). Byte-identical to clang 19 and 22 for free,
  member, static, constructor and new-expression calls (WP-819); each
  object is destroyed once whether a later argument or the callee throws
  (`class.temporary/run-trivial-abi-cleanup-isactive.cpp`).
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
    §8.17, also as clang 22's `-ffp-model=fast` default, spec/driver.md
    "Floating-point flags").
  - `#pragma STDC CX_LIMITED_RANGE ON|OFF` selects `Basic` / `Full` in
    its scope (§3.2, WP-806).
- **Calls.** A complex argument or return goes through the target
  classification (spec/abi.md §8.17). A returned pair is unpacked from
  the coerced type (`cx_call_result`). Delegating calls (inheriting
  constructors, lambda static invokers) forward the parts unchanged.
- **Validation.** `tests/codegen/complex_{arith,calls,ops,types,delegate,
  imaginary,limited_range,improved_range}.cpp` and `va_arg_complex.cpp`
  are exact on aarch64 and x86_64; `tests/conformance/ext.complex/
  {limited,improved,full}-range.cpp` (run) observe each range.
- **Not implemented:** `_Complex _Float16` (parsed; not checked against clang),
  and `-fcomplex-arithmetic=promoted` (spec/abi.md §8.17 "Deviations").

## 7o. Landed (WP-351): coroutines (`coro.rs`)

A port of clang's `CGCoroutine.cpp` (switched-resume ABI). The function
whose body is a `CoroutineBodyStmt` (sema.md §7.13) is emitted
**pre-split**: it carries the `presplitcoroutine` attribute and the
`llvm.coro.*` intrinsics, and the driver's coroutine lowering
(driver.md "IR optimization") builds the frame and the `.resume`,
`.destroy` and `.cleanup` clones. `-emit-llvm` prints the split form, as
clang's does; `-Xclang -disable-llvm-passes` prints the pre-split one.
When the return type's class carries `[[clang::coro_only_destroy_when_complete]]`
(sema.md §7.13 attributes), the function also gets the
`coro_only_destroy_when_complete` attribute, which CoroSplit copies to
the clones (clang `EmitCoroutineBody`; `tests/codegen/
coro_only_destroy_when_complete.cpp`).

**Body layout (`emit_coroutine_body`, clang `EmitCoroutineBody`).**

1. `%id = llvm.coro.id(i32 <default new align>, ptr null, ptr null,
   ptr null)`; the promise's address is patched into the second
   argument once the promise is emitted.
2. `coro.alloc`: `llvm.coro.alloc(%id)` chooses between the allocation
   call (`operator new(llvm.coro.size.i64(), ...)`; under
   `-fcoro-aligned-allocation` with an `align_val_t` form,
   `operator new(llvm.coro.size.i64(), llvm.coro.align.i64(), ...)` and
   the matching aligned `operator delete`, sema.md §7.13;
   `tests/codegen/coro_aligned_alloc.cpp`) and no allocation;
   with `get_return_object_on_allocation_failure` a null result
   branches to `coro.ret.on.failure`, which returns that object.
   `coro.init`: `%frame = llvm.coro.begin(%id, phi(null, mem))`.
3. With a `__coro_gro` variable: its alloca (marked
   `!coro.outside.frame`), an `i1 gro.active` flag (false until the
   variable is initialized) guarding its destructor cleanup.
4. A `CoroDelete` cleanup: `if (llvm.coro.free(%id, %frame)) operator
   delete(<that pointer>[, llvm.coro.size])`; `__builtin_coro_free`
   inside the deallocation call is that `llvm.coro.free` result.
5. The parameter copies are emitted, and every use of a copied
   parameter in the body resolves to its copy (clang
   `ParamReferenceReplacerRAII`); then the promise.
6. The return object: initialized into the return slot (same type),
   into `__coro_gro` (then `gro.active = true`), or evaluated and
   discarded (`void`). Then a `CoroEnd` cleanup (below).
7. The initial suspend, then the body inside an implicit
   `try { body } catch (...) { __promise.unhandled_exception(); }` (the
   WP-100 lowering, `enter_catch_all`/`exit_catch_all`), flowing off the
   end into the implicit `co_return;`. When the initial `await_resume`
   can throw, it is emitted inside the same catch-all with an `i1
   resume.eh` flag set before it and cleared after; a set flag skips the
   body ([dcl.fct.def.coroutine]/5, CWG2451).
8. `coro.final`, reached by `co_return` (each `co_return` evaluates its
   promise call, then branches to it through the body's cleanups) or by
   falling off the end: the final suspend.
9. The scope's cleanups (`CoroEnd`, the promise, the copies,
   `CoroDelete`), then `coro.ret`: `llvm.coro.end(ptr null, i1 false,
   token none)` and the return to the caller: `return __coro_gro;`
   converted, the return slot, or `ret void`.

**Suspend points (`emit_suspend`, clang `emitSuspendExpression`).**
`await_ready()` branches to `<kind>.ready` when true, else to
`<kind>.suspend`, which does `%save = llvm.coro.save(ptr null)`, then
the suspend call, outlined as in clang 19 (`generateAwaitSuspendWrapper`):
a private `<coroutine>.__await_suspend_wrapper__<kind>(ptr awaiter, ptr
frame)` evaluates `await_suspend` (returning its `bool`, or the address
of its handle) and the coroutine calls `llvm.coro.await.suspend.void`,
`.bool` or `.handle` with the awaiter, the frame and the wrapper, so no
frame value is live across the call. `void` falls through; `bool`
branches back to `.ready` on `false`; for `.handle` CoroSplit resumes the
returned handle with a `musttail` call (symmetric transfer, [expr.await]/5.1;
`tests/codegen/coro_await_suspend.cpp`, the 1M-hop run test
`tests/conformance/expr.await/symmetric_transfer.cpp`). Then
`%s = llvm.coro.suspend(token %save, i1 <final>)` and `switch i8 %s,
label %coro.ret [i8 0, label %<kind>.ready; i8 1, label
%<kind>.cleanup]`. `.cleanup` leaves through the coroutine's cleanups
to `coro.ret` (a destroyed coroutine runs the destructors of the
suspended scope). `.ready` evaluates `await_resume()` in the wanted
form: ignored, scalar, lvalue, or into an aggregate slot. `<kind>` is
`init`, `await`, `yield` or `final`. The wrapper's function attributes are clang's
(`generateAwaitSuspendWrapper` + `FinishFunction`) at every level:
`alwaysinline mustprogress`, on x86 `"min-legal-vector-width"`, and at
`-fclang-compat=22` `"sample-profile-suffix-elision-policy"="selected"`
(clang 22); never `optnone`/`noinline`. Its parameters are `void *`
`ImplicitParamDecl`s, so at -O1+ their prolog stores carry "any pointer"
TBAA and the reloads none (`Cg::tbaa_void_ptr_store`; WP-423). After
the -O1 pipeline the wrappers are inlined away; the pre-pass text is
pinned against clang 19 by `kiln_driver/tests/codegen.rs`
`coro_wrappers_before_passes_match_clang19`, the optimized one by
`tests/codegen/coro_await_suspend_o1.cpp`.

**Full-expression temporaries across a suspension.** A temporary of the
full-expression holding a `co_await` lives until the full-expression
ends, after resumption ([class.temporary]/4). Since the cleanup blocks
of the full-expression are shared by the normal and the `.cleanup`
path, a value computed before the cleanups and used after them is
spilled to a `tmp.exprcleanup` alloca right after its definition and
reloaded after the cleanups (clang `RunCleanupsScope::ForceCleanup(
ValuesToReload)`; `end_scope_reload` / `pop_cleanup_blocks_reload`). A
reference bound to such a full-expression (`const auto& r = co_await
a;` with an lvalue-reference `await_resume`) reloads its bound address
the same way before the reference is stored (clang
`EmitExprWithCleanupsLValue`; `Cg::emit_ref_binding`; WP-726,
`expr.await/reference-result-bound-to-reference.cpp`). A prvalue use
of a glvalue `co_await` (`auto v = co_await a;` with a reference
`await_resume`, the operand of `CK_LValueToRValue`) loads the referee
through the resume call's lvalue, as for a reference-returning call, so
the value and not its address is spilled (clang `emitSuspendExpression`
visits the resume call; `Cg::emit_glvalue_load`; WP-789,
`expr.await/reference-result-loaded-by-value-run.cpp`).

**`CoroEnd` cleanup (EH only, clang `CallCoroEnd`).** On unwinding,
`llvm.coro.end(ptr null, i1 true, token none)`. For LLVM 19 it returns
`i1`: true (the ramp function) resumes the exception at once; otherwise
(the resume clone) unwinding continues through the cleanups.

**LLVM 22 `coro.end` (WP-355).** LLVM 22's `llvm.coro.end` returns
`void`; whether code runs in the ramp function is
`i1 @llvm.coro.is_in_ramp()`. The IR follows the backend's version
(`CodegenOptions::llvm_major`: the major of the driver's selected LLVM,
spec/driver.md "LLVM discovery"; not `-fclang-compat`), since each LLVM rejects the other's signature. With
LLVM 22 the `CoroEnd` cleanup is `call void @llvm.coro.end(...)`, then
`%r = call i1 @llvm.coro.is_in_ramp()`, and the branch is inverted as
clang 22's: the ramp function continues through the cleanups (freeing
the frame) and a clone resumes the exception. `__builtin_coro_end`'s
`bool` result is `is_in_ramp()` there. clang 22 stores the `void` call's
missing value instead, which is invalid IR.

**Builtins (`emit_coro_builtin`, clang `EmitCoroutineIntrinsic`).**
`__builtin_coro_resume/destroy/done/promise/noop/size/align/alloc/
begin/end/suspend/free/id` become the matching `llvm.coro.*` call;
`alloc`, `begin` and `free` take the function's `%id` first, `suspend`
a leading `token none`, `end` a trailing `token none`;
`__builtin_coro_frame` is the `llvm.coro.begin` value. A hand-written
`__builtin_coro_id` in a function that is not a C++ coroutine creates
the function's coroutine state (clang `createCoroData`, for coroutines
written by hand in C): its token feeds the later `alloc`/`begin`/`free`
calls, a `__builtin_coro_begin` becomes the `frame` value, and the
function gets no `presplitcoroutine`; a second one is a codegen error
"only one __builtin_coro_id can be used in a function", and one in a
C++ coroutine "__builtin_coro_id shall not be used in a C++ coroutine"
(tests/codegen/coro_builtins.cpp, `lower.rs` `hand_written_coro_id`).
Without either, `frame`/`alloc`/`begin`/`free` are a codegen error with
clang's wording ("this builtin expect that __builtin_coro_begin/id has
been used earlier in this function").

**IR parity details (WP-370, §7p).** A coroutine gets lifetime markers
at every level;
its cleanup destination slot is promoted to SSA; the `llvm.coro.free`
call is moved to guard the deallocation (clang `CallCoroDelete`
remembers the last one); the promise's address passes through a
`bitcast ptr %promise to ptr` before `coro.id`; an ignored empty-class
return gets clang's `retval` temporary, erased when unused.

### Frame pointer (WP-405)
`FramePointer::resolve` follows clang's driver (`getFramePointerKind`) for
Linux targets; `Cg::frame_pointer` applies it.
- The kind is `none` when `-fomit-frame-pointer` is in effect, or by
  default on x86-64 when optimizing (`-O1` and above, `-Os`, `-Oz`,
  `-Og`); AArch64 keeps the frame pointer at every level. Otherwise it
  is `non-leaf` when the leaf frame pointer is omitted, `all` when kept.
  The leaf default is to omit on AArch64 and keep on x86-64.
- `-fomit-frame-pointer` / `-fno-omit-frame-pointer` and
  `-momit-leaf-frame-pointer` / `-mno-omit-leaf-frame-pointer`: the last
  of each pair wins (`kiln_driver::args`, `CodegenOptions::
  omit_frame_pointer` / `omit_leaf_frame_pointer`); the leaf flags only
  matter when the frame pointer is kept.
- `all` and `non-leaf` give every function the `"frame-pointer"` string
  attribute and the module flag `!{i32 7, !"frame-pointer", i32 N}` with
  N = 2 / 1; `none` gives neither.
- clang 22 replaces `non-leaf` by `"non-leaf-no-reserve"` (flag 4) on
  both targets (AArch64's default, x86-64 with
  `-momit-leaf-frame-pointer`); kiln does so at compat 22 with an LLVM 22
  backend (§6a), and emits `"non-leaf"` / 1 otherwise.
- Tests: `kiln_codegen` unit `frame_pointer_matches_clang` (the matrix),
  `kiln_driver` `frame_pointer_flags_last_win`,
  `tests/codegen/frame_pointer_*.cpp` and `noinline_matrix_o{1,2}.cpp`
  (exact under oracle 19 on both targets).

### no-builtin (WP-808)
`-fno-builtin` / `-ffreestanding` (`CodegenOptions::no_builtin`, clang
`LangOpts.NoBuiltin`), `-fno-builtin-<name>`
(`CodegenOptions::no_builtin_fns`, `LangOpts.NoBuiltinFuncs`) and the
`no_builtin` attribute (spec/sema.md §4.5), as clang 19
(`addNoBuiltinAttributes`, `ConstructAttributeList`,
`EmitDirectCallee`):
- **Functions.** Every declaration and definition that goes through
  `Cg::target_attrs_of` (functions, global-init and TLS helpers,
  `__clang_call_terminate`; not the runtime declarations such
  as `__cxa_*`) gets `Cg::no_builtin_attrs`: `"no-builtins"` alone under
  `-fno-builtin`; else `"no-builtin-<name>"` per `-fno-builtin-<name>`,
  then for its `no_builtin` attribute `"no-builtins"` (wildcard) or
  `"no-builtin-<name>"` per name. Kiln reads the definition's attribute
  for every declaration of the function (`Cg::no_builtin_names`; clang
  reads the declaration it emits, and only definitions may carry one).
- **Call sites** (`Cg::call_size_attrs`: direct, indirect, virtual,
  constructor and destructor calls): `nobuiltin` under `-fno-builtin`
  (clang `!SimplifyLibCalls`) or when the callee's IR name is a
  `-fno-builtin-<name>`, with the same `"no-builtin*"` strings, the
  attribute part from the callee's `no_builtin` (`Module::nba_fns`,
  recorded when the function is declared; none on an indirect call, nor
  on a thunk's call of its target, `Fx::thunk`).
- **Lowering.** Sema recognizes no library builtin under the flags
  (spec/sema.md §5.4), so `memcpy` / `ceil` / ... are calls; inside a
  function whose `no_builtin` names the callee (or is the wildcard,
  `Cg::no_builtin_here`) a recognized library builtin is called, not
  lowered. `__builtin_*` always lower. `Cg::is_std_builtin` (`std::move`
  and kin) is off under `-fno-builtin` and for `-fno-builtin-std-<name>`;
  the attribute's `std-<name>` does not stop the fold (clang compares
  the current function's `"no-builtin-<name>"` with the callee's plain
  name, here `move`, which the attribute cannot name). **Deviation:**
  in a function with the wildcard `no_builtin`, clang 19 calls
  `std::move` without instantiating its definition (sema treated the
  call as a builtin), an undefined reference at link time; kiln keeps
  folding it.
- `-ffreestanding` also drops `uwtable` (`unwind_tables`) and makes
  `__STDC_HOSTED__` 0. Tests: `tests/conformance/dcl.attr.no-builtin/`
  (`ir-*.cpp` exact IR, `freestanding-memset-run.cpp`: a byte-loop
  `memset` at -O2 does not become a recursive `memset` call).

### Hardening and overflow options (WP-605)
The driver's "Codegen flags" (spec/driver.md) set these `CodegenOptions`
fields; each follows clang 19's IR.
- **`stack_protector: StackProtector`** (`Off`, the default, `On`,
  `Strong`, `All`): `ssp` / `sspstrong` / `sspreq` in the attribute group of
  every definition (`module.rs` `def_attrs`; clang
  `SetLLVMFunctionAttributesForDefinition`), thunks, global-init and TLS
  helpers included.
- **`unwind_tables: Option<UnwindTables>`** (`Async` by default): `uwtable`
  / `uwtable(sync)` on definitions and the `"uwtable"` module flag 2 / 1;
  `None` emits neither.
- **`no_plt`**: `nonlazybind` on every function declaration without a
  definition in the module (`set_decl_attrs`), the `"RtLibUseGOT"` module
  flag, and `visibility::assume_dso_local` no longer makes function
  declarations `dso_local` under the static model.
- **`wrapv`**: signed `add`/`sub`/`mul`/`shl`, increments, negation and
  VLA size products drop `nsw` (`Cg::nsw_if`, `vla_nsw`).
  **`wrapv_pointer`**: pointer arithmetic and subscripts are plain GEPs
  (`Cg::gep_arith`), and at compat 22 member access too (`Cg::field_gep`;
  clang 22 drops `inbounds nuw` there, clang 19 keeps `inbounds`).
- **`null_pointer_is_valid`**: the `null_pointer_is_valid` function
  attribute on definitions; `nonnull` is not emitted on reference
  parameters/returns, `this` (which gets `dereferenceable_or_null(N)`
  instead of `dereferenceable(N)`), `nonnull`/`returns_nonnull`
  attributes, the `operator new` result or `!nonnull` load metadata.
- **`target_cpu` / `tune_cpu`**: `"target-cpu"` / `"tune-cpu"`; `native`
  emits no `"target-cpu"` (the driver passes `llc -mcpu=native`), and on
  x86-64 the default `"tune-cpu"="generic"` is only emitted without a CPU.
- Tests: `tests/codegen/{stack_protector,stack_protector_strong,
  stack_protector_all,unwind_tables_sync,unwind_tables_none,no_plt,
  no_plt_static,wrapv,no_strict_overflow,no_delete_null_pointer_checks}.cpp`
  (exact under oracle 19 on both targets).

## 7p. Landed (WP-370): optimization-dependent IR

- **`CodegenOptions::opt_level`** (clang `CodeGenOpts.OptimizationLevel`:
  0 at `-O0`, 1 at `-O`/`-O1`/`-Og`, 2 at `-O2`/`-Os`/`-Oz`, else 3;
  `kiln_driver::codegen_cmd::opt_level`, WP-431) and its accessor
  `CodegenOptions::optimize()` (`opt_level > 0`): when optimizing, definitions are emitted without
  the `-O0` `noinline optnone` pair, so the driver's `opt` pipeline
  (driver.md "IR optimization") can inline and optimize them.
- **Lifetime markers (`lifetime.rs`, clang `EmitLifetimeStart` /
  `CallLifetimeEnd`, `ShouldEmitLifetimeMarkers`).** In every function
  at `-O1`+ (WP-423; also `optnone` ones, as clang) and in a coroutine
  at every level (CoroSplit uses them to keep short-lived locals out of
  the frame), each automatic variable gets `llvm.lifetime.start` right after
  its alloca (the end of clang `EmitAutoVarAlloca`) and each materialized
  temporary when it is created (before the outermost conditional for a
  full-expression temporary with no destructor), and `llvm.lifetime.end` from a
  `LifetimeEnd` cleanup (normal and EH, clang `NormalEHLifetimeMarker`).
  A variable a `goto` or a `switch` jumps past gets none (clang
  `VarBypassDetector`, initialized over the body as in clang
  `GenerateCode`; an indirect `goto` bypasses every variable). Thunks
  get none. The markers use LLVM 19's `(i64 size, ptr nocapture)`
  signature, and LLVM 22's `(ptr captures(none))` at `-fclang-compat=22`
  (WP-462, §7z): codegen emits the latter itself for an LLVM 22+ backend
  (`kiln_llvm` `Module::ir_version`, `intrinsics::lifetime_has_size`),
  and for an older one the sized form those tools verify (LLVM 21 spells
  its pointer `captures(none)` but keeps the size operand), in every
  output including `-S -emit-llvm` text (WP-783; driver.md "IR
  optimization").
- **Cleanup-destination promotion (`fir/mem2reg.rs`).** clang's
  `FinishFunction` replaces a coroutine's `cleanup.dest.slot` alloca by
  SSA values and phis; kiln ports LLVM 19's `PromoteMemToReg` for that
  one alloca (single-store and single-block fast paths, pruned phi
  placement at the iterated dominance frontier, the renaming walk whose
  successor order fixes each phi's incoming order, trivial-phi
  simplification), so the IR matches clang's.
- **Empty-record `va_arg`.** The aggregate copy of a `va_arg` result
  skips empty records, as clang's `EmitAggregateCopy`.
- **Invariant globals** (WP-431; `Cg::emit_decl_invariant`, clang
  `EmitDeclInvariant`, [dcl.type.cv]/4): when optimizing, the dynamic
  initialization of a non-reference variable (namespace scope, inline,
  `thread_local`, static local; guarded or not) whose type is
  const-qualified (an array of const elements included), has no
  `mutable` member and needs no destruction ends with
  `call ptr @llvm.invariant.start.p0(i64 <size>, ptr @var)`, naming the
  global itself also for a thread-local one. It is the last step of
  `global_var_decl_init`. `tests/codegen/invariant_start_o1.cpp`.

## 7p. Landed (WP-376): thread-local storage (`tls.rs`)

clang `CodeGenModule::setTLSMode`, `ItaniumCXXABI::EmitThreadLocalInitFuncs`
and `EmitThreadLocalVarDeclLValue` mirrored ([basic.stc.thread],
[basic.start.dynamic]/7, [basic.start.term]/1, Itanium ABI §3.4). A
`thread_local` variable is a C++ dynamic TLS variable (`TlsKind::Dynamic`).
A `__thread` or `_Thread_local` variable is clang `TLS_Static`
(`TlsKind::Static`, WP-396): it is a `thread_local` global with no
`_ZTW`/`_ZTH`, reached through `@llvm.threadlocal.address`. Sema
guarantees it a constant initializer and a trivial destructor
(spec/sema.md §4.10).

- **Globals.** A thread-local variable's global (namespace-scope, static
  data member, function-local, its `_ZGV` guard and the `_ZGR` of a
  temporary it extends) is `thread_local` (general dynamic). The address
  of a thread-local object is not a constant ([expr.const]/14; the Oven's
  result check rejects it without a note, as clang), so a reference bound
  to one is dynamically initialized.
- **Access.** A scalar load or store whose pointer is a thread-local
  global goes through `@llvm.threadlocal.address` (clang
  `EmitLoadOfScalar`/`EmitStoreOfScalar`); constructor calls and
  `__cxa_thread_atexit` take the raw global. A namespace-scope variable
  with dynamic initialization or a destructor, or one with no
  initializing declaration here (clang `usesThreadWrapperFunction`), is
  reached by calling its wrapper `_ZTW<name>`; otherwise, and for every
  function-local one, by `@llvm.threadlocal.address`. A reference's
  wrapper returns the referee.
- **Initialization.** Dynamic initializers are ordinary
  `__cxx_global_var_init.N` functions; an ordered variable's is called
  from `internal void @__tls_init()` behind `internal thread_local i8
  @__tls_guard` (branch weights 1:1023, guard stored before the calls;
  at -O1+ followed by `call ptr @llvm.invariant.start.p0(i64 1, ptr
  @__tls_guard)`, clang `EmitInvariantStart`, WP-423 — a module
  initializer's guard gets the same, through the shared
  `emit_guarded_calls`) and named by `_ZTH<name>`, an alias of `__tls_init` with the variable's
  linkage. A template instantiation's (unordered) `_ZTH` aliases its own
  guarded initializer. Destructors register with `__cxa_thread_atexit`
  (`nounwind`). A function-local `thread_local` is guarded by its
  `thread_local` `_ZGV` byte without `__cxa_guard_*` (weights 1:1023).
- **Wrappers.** Created at first use and, at the end of the TU, for each
  defined non-discardable thread-local variable in creation order:
  `internal` for an internal variable, else `weak_odr hidden` in a comdat
  (`linkonce_odr` when the variable is not defined here); `noinline`,
  `uwtable` (`nounwind` under `-fno-exceptions`), `noundef ptr` return.
  The body calls `_ZTH` (through `icmp ne ptr @_ZTH, null` and an
  `extern_weak` declaration when not defined here; omitted for a
  constant-initialized one), then returns `@llvm.threadlocal.address`
  (loaded through for a reference). Importers of a named module declare
  its variables and use the `linkonce_odr` form (spec/modules.md §10
  deviation 9).
- Tests: `tests/codegen/thread_local.cpp`,
  `tests/codegen/module_thread_local.cpp` (oracle 22),
  `tests/conformance/basic.stc.thread/module-tls-run.cpp`.
- The initializer, `_ZTH` and `_ZTW` of an implicitly instantiated
  thread-local variable template specialization or template static data
  member follow the TU's functions, as in clang (WP-466, §7ab;
  `tests/codegen/var_template_thread_local.cpp`, both oracles).

## 7q. Landed (WP-187): `-Wpsabi` and `bool`-vector conditions
- **Calls: `-Wpsabi`** (`psabi.rs`, clang
  `X86_64TargetCodeGenInfo::checkFunctionCallABI`). On x86-64 only, each
  direct call, member call and constructor call checks its arguments in
  order, then the return type. The first vector wider than 128 bits,
  with F = `avx` up to 256 bits and `avx512f` beyond, is checked against
  the caller's and the callee's feature maps (WP-1000: the command line's,
  or a `target("...")` attribute's, spec/driver.md "x86-64 target
  features"), and ends the check: neither has F warns `AVX vector
  argument|return of type T without 'F' enabled changes the ABI`
  (`cg_avx_calling_convention`, group `psabi`); exactly one has it is
  the same text as an error (`cg_avx_calling_convention_mismatch`, clang
  `err_avx_calling_convention`); both have it is silent.
  - T is the callee's declared (sugared) parameter type. For a call that
    passes an object argument, it is the parameter after that one (clang
    counts `this`). Through `...` it is the argument's own type, and for
    a return it is the declared return type. The type is printed with
    the vector `aka` rule (spec/sema.md, `vector_diag_arg`).
  - The location is clang's `getExprLoc`: the callee's begin for a plain
    call, the member name for a member call, the operator for most
    overloaded operators.
  - Calls through a pointer are not checked. At `-fclang-compat=22`, a
    callee that is not externally visible does not get the warning (the
    error still applies).
  - Tests: `ext.vector/ir-psabi-x86_64.cpp`, `ir-psabi-internal-x86_64*.cpp`,
    `crates/kiln_driver/tests/codegen.rs` `x86_avx_calling_convention`.
- **`bool` ext-vector `?:` conditions** (clang 20+) select on the
  condition's `<N x i1>` lanes with one `select`
  (`emit_vector_conditional`; `ext.vector/ir-bool-conditional*.cpp`,
  `bool-conditional-run.cpp`).
- At `ir_version` 20+, `kiln_llvm` prints uniform vector constants as
  `splat (T v)` (spec/llvm.md §5).

## 7r. Landed (WP-448): erroneous values (`auto_init.rs`)

C++26 [basic.indet] (P2795R5): an automatic variable without an
initializer has an *erroneous value*, an implementation-defined value
that reading yields (erroneous behaviour, no longer undefined), unless
it is declared `[[indeterminate]]` ([dcl.attr.indet]).

- **kiln's erroneous value is all-zero bytes**, padding included. It is
  the cheapest value to write (one `store <ty> 0` or one `llvm.memset`
  with a zero byte, which the backend folds into the stack setup) and
  the value GCC writes under `-ftrivial-auto-var-init=zero`; zero is
  also a valid value of every scalar (a null pointer, `false`, `+0.0`).
- **Where.** `emit_one_var_decl`, after the `alloca` and only when the
  declaration has no initializer (`Cg::auto_var_init`): at the
  declaration, so a loop body refills its variable on each iteration. A
  scalar (integer, pointer, floating point, or a vector of them) gets
  `store <ty> 0` (`volatile` for a volatile object); anything else
  (arrays, classes, complex) `call void @llvm.memset.p0.i64(ptr align A
  %v, i8 0, i64 N, i1 <volatile>)`. Both carry `!annotation
  !{!"auto-init"}`: the IR of clang's `-ftrivial-auto-var-init=zero`,
  which `tests/codegen/erroneous_init.cpp` compares exactly.
- **Not filled:** a `[[indeterminate]]` or `[[clang::uninitialized]]`
  variable (the attribute on any of its declarations) keeps its plain
  `alloca`; an object of size 0; a variable whose declaration a jump
  bypasses ([stmt.dcl]/2 makes that ill-formed for a variable with
  initialization, but a plain `int x;` may be bypassed and is then
  unwritten, as in clang). Variables with an initializer that may leave
  bytes unwritten (a non-trivial default constructor, a non-constant
  aggregate initializer) and VLAs are pre-filled since WP-452 (§7y).
- **Size limit: 4096 bytes** (`AUTO_VAR_INIT_MAX_SIZE`, the driver's
  default for `-ftrivial-auto-var-init-max-size`). A larger variable is
  not written (its value is still erroneous) and gets the kiln-only
  remark `erroneous value of variable 'b' not written: its size (4100
  bytes) exceeds -ftrivial-auto-var-init-max-size=4096
  [-Rauto-var-init]` (`cg_auto_var_init_too_large`, group
  `auto-var-init`), at the variable. A size equal to the limit is
  written; a limit of 0 means none, as for clang (whose default is 0).
  The limit keeps large stack buffers (`char buf[65536];`, filled later
  anyway) from paying a memset at every entry.
- **Standards.** The fill is on by default under `-std=c++26` and later
  only; before C++26 the read is undefined and nothing changes. The
  driver flags (spec/driver.md): `-ftrivial-auto-var-init=zero` turns it
  on under any standard (clang's meaning), `=uninitialized` off, and
  `=pattern` (§7y) fills with clang's pattern instead of zero.
  `CodegenOptions::auto_var_init` (`AutoVarInit::{Uninitialized, Zero,
  Pattern}`, default `Uninitialized` for API users) and
  `auto_var_init_max_size`. The clang oracles have no
  erroneous values, so the IR-diff corpus (`ir_vs_clang.rs`
  `pinned_flags`) and the conformance `expect: ir` tests at C++26 pin
  `-ftrivial-auto-var-init=uninitialized`.
- **Constant evaluation** is unchanged: an erroneous or indeterminate
  value is never read in a constant expression (spec/oven.md
  "Uninitialized reads").
- Tests: `tests/codegen/erroneous_init.cpp`,
  `crates/kiln_driver/tests/erroneous_init.rs`,
  `tests/conformance/basic.indet/` (`run-erroneous-zero.cpp`,
  `run-byte-read.cpp`, `run-threshold.cpp`, `indeterminate-compiles.cpp`,
  `ir-cxx23-unchanged.cpp`).
- **Remaining:** the run-time EB diagnosis P2795 recommends is today's
  `-Wuninitialized` (no sanitizer). A variable whose declaration a
  `goto` (or `switch` case label) jumps over stays unwritten, as in
  clang: the fill is at the declaration, which control never reaches.
  Reading it is still erroneous behaviour, but the value is whatever the
  stack held; covering it would mean filling at the jump or at function
  entry, which clang does not do either (not planned).
  `=pattern`, constructor and aggregate pre-fills and VLAs: §7y.
- **Note (WP-584).** The zero fill is correct: a formatting library's
  `"{:08b}"` failure (`argument not found`) seen only at C++26 `-O2` had a
  fill byte for byte clang's `-ftrivial-auto-var-init=zero`. The cause
  was the Oven folding `if consteval` to true in codegen's trial folds,
  fixed by WP-588 (spec/landed/oven/WP-588.md); the zero fill only made
  the resulting stray read deterministic. Tests pinning the shape:
  spec/landed/codegen/WP-584.md.

## 7s. Landed (WP-450): `bool` ext-vector constants
- A constant `bool` ext vector (the Oven's packed `BoolVec`,
  spec/oven.md "Bool vectors") is a `<N x i1>` constant: lane `i` is bit
  `i % 8` of byte `i / 8` of the folded image (`consts.rs`
  `image_const`). Its global, and a struct member or array element of
  that type, keeps the value type `<N x i1>` rather than the `iP` memory
  type (`const_ir_type`; clang's literal struct `{ i32, <8 x i1>, <3 x
  i1> }` and `[2 x <8 x i1>]`), and is laid out by its bit count rounded
  up to a power-of-two byte size (`func.rs` `ir_layout`).
- Constant folding as the IRBuilder's `ConstantFolder` does (`func.rs`
  `fold_shuffle`, `fold_vec_to_int`): a `shufflevector` of two constants
  folds (a poison mask lane is `undef`; an all-poison mask is `poison`),
  and a `bitcast` of a constant vector to an integer folds to `0` when the
  vector is null, to `-1` when every lane is all-ones at compat 22 (LLVM
  20+ `FoldBitCast`), and otherwise stays a `bitcast (...)` constant
  expression. So storing a constant `bool` vector to a local is `store iP
  bitcast (<P x i1> <...> to iP)`, as clang's.
- *Arrays* (WP-438), as clang's `ConstExprEmitter`
  `EmitArrayInitialization` / `EmitArrayConstant`: an array of `bool`
  vectors at any depth keeps `<N x i1>` elements (`[2 x [2 x <4 x
  i1>]]`), also as a class member (`record_elems`). The desired type is
  the memory form (`bool_vec_mem_form`: `<4 x i1>` is `i8`), so an
  all-zero array is `[3 x i8] zeroinitializer` and a trailing zero array
  without a common type is `[k x i8]`. A value-initialized element, filler
  or member (`ImplicitValueInitExpr`, clang `EmitNullConstant`) of a
  `bool` vector, array of them or class is the null of its memory type
  (`i8 0`, `[2 x i8] zeroinitializer`, `%struct.P zeroinitializer`), so a
  partly initialized array is a packed struct (`<{ <4 x i1>, <4 x i1>, i8,
  i8 }>`). `array_const` takes clang's `CommonElementType` over the
  explicit elements only: a trailing zero array of 8 or more is of their
  common type (`[9 x <{ <4 x i1>, i8 }>]`) even when the filler's differs.
- Test: `tests/codegen/bool_vector_const.cpp` (exact on both triples
  under oracle 19, structural under 22); arrays and members:
  `tests/codegen/bool_vector_array_const.cpp`; unions:
  `tests/codegen/bool_vector_union_const.cpp` (WP-438).

## 7t. Landed (WP-444): GNU computed goto (`stmt.rs`)
- `&&l` (clang `GetAddrOfLabel`, `addr_of_label`) is `blockaddress(@f,
  %bb)` for the label's block. The first label address or computed
  `goto` in a function makes the dispatch block (clang
  `GetIndirectGotoBlock`, `Fir::indirect_goto`): `%indirect.goto.dest =
  phi ptr` and `indirectbr ptr %indirect.goto.dest, [...]`, unplaced.
  Each `&&l` evaluation adds its label as a destination, so a label
  whose address is taken twice is listed twice, as clang's
  `addDestination`. The label's LLVM block is created ahead of lowering
  (`Fir::block_id`) so the `blockaddress` can name it; an address-taken
  block is a use of its block and is never pruned.
- `goto *e` (clang `EmitIndirectGotoStmt`): a target that is `&&l`
  under parentheses and implicit casts (`Ast::indirect_goto_constant_
  target`) is `goto l`, through the cleanups of the scopes it leaves;
  otherwise the target is emitted as a scalar, added to the `phi` from
  the current block, and the block branches to the dispatch block (no
  cleanups, which sema guarantees there are none to run).
- At the end of the function (`finish_indirect_goto`, before EH
  finishing) the dispatch block is placed after the return block. With
  no computed `goto` its `phi` has no incoming values and is replaced by
  `poison` (clang 19, `-fclang-compat=19`: `undef`).
- A label address in a constant (`static void *t[] = {&&a, &&b};`, a
  frozen Oven value `PtrBase::Label`, `consts.rs`) is the same
  `blockaddress`, so the global is a constant initializer exactly as
  clang's. The address of another function's label (only reachable
  through a constexpr value computed in that function) is a codegen
  error "address of a label of another function in a constant" (clang
  crashes there).
- Tests: `tests/codegen/computed_goto.cpp` (exact IR on both triples at
  both oracles: a static table, a conditional target, a duplicate
  address, an unused dispatch block, `goto *&&l` through a destructor, a
  template instantiated twice), `tests/conformance/stmt.goto/computed-
  goto-dispatch-run.cpp` (`expect: run`).

## 7w. Landed (WP-453): vector lane folds, int → `bool` vector conversion
- A subscript of an ext vector whose base names a `const` non-volatile
  vector variable (through parentheses and `NoOp` casts;
  `fold::refers_to_const_var`) is a fold candidate: `Collect::expr`
  pushes the base, and `emit_scalar_opt`'s vector `ArraySubscriptExpr`
  arm uses its folded value, so the lane is an `extractelement` of the
  constant (folded to the lane constant for a constant index, else
  `extractelement <const>, %i` with the index's side effects kept) and
  nothing is loaded. clang: `ScalarExprEmitter::VisitArraySubscriptExpr`
  visits a vector base as a scalar, `VisitDeclRefExpr`'s
  `tryEmitAsConstant` folds it. Int, float and `bool` lanes alike; a
  `const` global whose every use folds is not emitted. A volatile or
  non-const vector, a lane read through a pointer (`(*p)[2]`), a swizzle
  (`k.y`, `EmitLoadOfLValue`) and an `extern` vector without an
  initializer stay loads. As in clang, a write through a cast-away
  `const` (`((i4&)k)[1] = 9`, undefined) does not stop the fold.
- Otherwise a glvalue base of that arm is loaded from its address
  (`emit_lvalue` + `load_lv`): a call returning `V&` (`v[i][j]` on a
  `std::vector<V>`) has the address as its scalar, so `emit_scalar`
  of it gave `extractelement` a pointer and lowering failed (WP-726;
  `tests/conformance/ext.vector/subscript-of-reference-returning-call-run.cpp`).
- `__builtin_convertvector` of a `const` vector variable folds the same
  way (WP-490): `Collect::expr` pushes the `ConvertVectorExpr` operand
  (a bare vector DeclRef, no `LValueToRValue`), the
  `ConvertVectorExpr` arm of `emit_scalar_opt` uses its folded value,
  and `convert_vector`'s `icmp ne`/`fcmp une` against zero (to `bool`)
  or cast folds lane by lane as LLVM's `ConstantFoldCompareInstruction`
  / `ConstantFoldCastInstruction` do (`func.rs` `icmp`/`fcmp` over
  `cmp_lanes`, `fold_vec_cast`; a `half`/`bfloat` lane compares widened
  exactly to `double`). A lane that is `undef`/`poison` or that the
  scalar folder leaves (`fp128`, `x86_fp80` compares) is not folded.
  clang: `ScalarExprEmitter::VisitConvertVectorExpr` visits the
  operand as a scalar (`tryEmitAsConstant`) and the IRBuilder folds.
  So `b4 c = __builtin_convertvector(K, b4);` stores `i8 bitcast (<8 x
  i1> <...> to i8)` and the unused `K` is not emitted. The runtime fold
  compares a float lane with zero (`0.5f`, `2.0f`, NaN are `true`), as
  clang's codegen does, while the Oven converts it toward zero first as
  clang 22's evaluator does (`0.5f` is `false`; spec/oven.md
  "Vectors"). Tests: `tests/codegen/convertvector_const_fold.cpp`
  (exact under oracle 19, structural under 22),
  `tests/conformance/ext.vector/convertvector-const-fold-run.cpp`.
- A vector variable's non-constant initializer is re-folded like a
  scalar's (§2 `fold.rs`, `is_scalar`): `i4 gv = cv;`, `cv + 1`, a `const
  f4`/`b4`/`l3` source give a static initializer with no
  `__cxx_global_var_init` and no `@llvm.global_ctors`.
- clang 20+'s implicit integer → `bool` ext vector conversion
  (spec/sema.md §5.28) is a `CK_IntegralToBoolean`
  cast of vector type, lowered by `convert_vector`: `icmp ne <N x iK> v,
  zeroinitializer` giving `<N x i1>`, then stored on the packed layout
  (§7s).
- Tests: `tests/codegen/vector_const_fold.cpp` (both oracles),
  `tests/conformance/ext.vector/ir-bool-from-int-vector.cpp` (`expect: ir`,
  `requires: clang>=22`).

## 7u. Landed (WP-457): destroying `operator delete` through a virtual destructor

[expr.delete]/7, /10, [class.dtor]/13. (Numbered after WP-450's §7s and
WP-444's §7t; renumber at merge if those land differently.)

- **D0 body.** When the deallocation function Sema selected for a
  virtual destructor (`Ast::dtor_delete`) is a destroying `operator
  delete`, the deleting destructor calls only that function: no D1
  call and no `Cleanup::DtorDelete` (the destroying delete runs the
  destructor itself if it wants to). Its first argument is `this`
  converted to the function's first parameter type, the conversion
  Sema built (`Ast::dtor_delete_this`, a derived-to-base expression on
  `this`; a GEP for a non-primary base, as clang's
  `getOperatorDeleteThisArg`); the size and alignment arguments are the
  destructor's class's (`emit_delete_call`), so an inherited destroying
  delete receives the most-derived size.
- **The delete-expression.** `delete p` whose static type has a virtual
  destructor and a destroying `operator delete` makes the virtual D0 call
  after the null check (never devirtualized, as clang's
  `EmitDestroyingObjectDelete`), so the most-derived class's lookup
  decides: a derived class's own usual `operator delete` hides an
  inherited destroying one and its D0 is the usual D1-then-delete body.
- **Tag argument.** The empty `std::destroying_delete_t` argument is a
  temporary `alloca`; when the call's ABI ignores it (x86-64) the unused
  `alloca` is erased, as clang does, and kept where it is passed
  (AArch64).
- Tests: `tests/codegen/destroying_delete_virtual.cpp` (plain, sized,
  sized+aligned over-aligned, inherited through a non-primary base, a
  derived usual delete; both targets; exact under oracle 19, structural
  under 22), `tests/conformance/expr.delete/destroying-operator-delete-virtual.cpp`
  (run).

## 7ac. Landed (WP-460): callee-destroyed arguments (`[[clang::trivial_abi]]`)
A by-value parameter of a class the callee destroys
(`kiln_abi::call::is_destroyed_in_callee`, spec/abi.md §8.26: passed in
registers yet with a non-trivial destructor, which only `trivial_abi`
allows) is destroyed at the end of the callee, not at the end of the
caller's full-expression ([class.temporary]/3; clang
`isParamDestroyedInCallee`).
- *Caller* (`emit_call_args`, clang `EmitCallArg` /
  `CallArgList::addArgCleanupDeactivation`): the argument is emitted into
  its `agg.tmp` with `dest_destructed` set, so no normal full-expression
  destructor is pushed; `push_arg_cleanup` instead pushes an EH-only
  destructor cleanup (clang `DestroyUnpassedArg`) with an activation flag
  placeholder, recorded in `FuncState::arg_cleanups`. A later argument
  that throws destroys the already-built argument; the other arguments
  keep their normal cleanups (destroyed in reverse order after the
  call).
- *Deactivation* (`deactivate_arg_cleanups`, clang
  `deactivateArgCleanupsBeforeCall`): in `call_vals_to`, after the
  callee is computed and just before the call instruction, the pending
  cleanups of the call's aggregate arguments are deactivated innermost
  first and their placeholders erased, so once the call is made the
  callee owns the object (also on unwind out of the callee).
- *Callee* (`func.rs` prolog): each such parameter's address gets a
  normal-and-EH destructor cleanup (`Cleanup::Dtor`) pushed in parameter
  order, recorded in `FuncState::param_cleanups`; it runs at the
  function's end (after the return value is built) in reverse order.
  Thunks push none (the target destroys them).
- *Delegate calls* (`delegate_call_arg`, clang `EmitDelegateCallArg`):
  forwarding a callee-destroyed parameter (the lambda static invoker,
  inheriting constructors) deactivates this function's cleanup of it
  (`forward_param_cleanup`): the delegate destroys it.
- Return values are unchanged beyond register passing: a returned
  `trivial_abi` class is returned per its coerced type, not through
  `sret`; NRVO still applies.
- Tests: `tests/codegen/trivial_abi_calls.cpp`, `trivial_abi_subobjects.cpp`
  (both oracles),
  `tests/conformance/class.temporary/run-trivial-abi-callee-destroys.cpp`.
## 7x. Landed (WP-454): braced expansion elements, constexpr structured bindings

No lowering changed; the new forms reach codegen as existing AST.

- A braced element of an expansion-init-list (`template for (P p : {{1,
  2}, {.x = 3}})`, spec/sema.md §7.6a) is the per-element block `{ P p
  = {...}; body }`. The expansion variable's list-initialization, its
  temporaries and its destructor are emitted as for any block-scope
  `P p = {...};`: each element's temporaries die at the end of its
  declaration's full-expression and `p` at the end of its block, before
  the next element starts. Tests:
  `stmt.expand/braced-elements-run.cpp` (constructor and destructor
  order), `stmt.expand/braced-elements.cpp` (the Oven's order).
- A `constexpr` structured binding (P2686R5, spec/sema.md §4.9) emits
  `e` and its holding variables as `constexpr` variables: a namespace-
  scope `e` and its tuple-like holding references are constant globals
  (`@x = constant ptr @_ZGR1x_`, as clang); a block-scope non-`static`
  one calls `get` at run time, as clang does. Test:
  `dcl.struct.bind/p2686-run.cpp`.
- Left:
  - (Fixed by WP-467, `spec/landed/codegen/WP-467.md`.) A `static
    constexpr` block-scope tuple-like decomposition's holding references
    got a guarded dynamic initializer, where clang constant-initializes
    them.
  - (Fixed by WP-467.) An unreferenced namespace-scope `constexpr`
    decomposition's `e` (`@_ZDC1x1yE`, internal) was not emitted when
    its holding variables do not refer to it; clang emits it.

## 7aa. Landed (WP-459): explicit-object conversion functions, AArch64 target features
- A conversion function with an explicit object parameter
  ([class.conv.fct]/1, P0847) is lowered as an explicit-object member
  function (`is_explicit_object_method` covers `CXXConversion`): no
  `this`, the object an ordinary first parameter (a reference, or a value
  for `this H`), mangled with `H` (`_ZNH1ScviERKS_`), named like a free
  function (`decl_lvalue`), and called with the object as its first
  argument (no `this` adjustment, no temporary for an lvalue object). A
  `this auto&&` / `this Self&&` conversion template is instantiated per
  object type. Test `tests/codegen/explicit_object_conversion.cpp`.
- AArch64 `"target-features"` (`module.rs` `aarch64_features`): the default
  entries `+fp-armv8,+neon,+outline-atomics` (or `-outline-atomics`),
  `+v8a`, `-fmv` are overridden by `CodegenOptions::target_features` (the
  driver's `-march=`/`-mcpu=`/`-target-feature` result, spec/driver.md
  "Target features"; last setting wins) and printed as clang does, the
  sorted `+` features, then the sorted `-` ones
  (`+fp-armv8,+fullfp16,+neon,+outline-atomics,+sve,+v8a,-fmv` for
  `-march=armv8-a+sve`).
- AArch64 NEON builtins (`neon.rs`) lower as clang 19's
  `EmitAArch64BuiltinExpr`, from two tables: the bf16 intrinsics
  (spec/landed/codegen/WP-558.md "bf16 lowering") and the load/store,
  lane, splat, narrowing and permute builtins with the type-code and
  alignment rules (spec/landed/codegen/WP-628.md "NEON load/store
  builtins"; a load or store takes the pointer operand's known
  alignment, not always `align 1`), and the arithmetic builtins the
  reference workload reaches (WP-628.md "NEON arithmetic builtins":
  `vpadd`, `vqsub`, `vmax`, `vmin`, `vabs`, `vsqrt`, `vfma`, `vbsl`,
  `vtst` as `sext (icmp ne (and a, b), zeroinitializer)`, `vcvt`, the
  `v{max,min,add}v` reductions, `vmull_p64`; and every other builtin an
  `arm_neon.h` intrinsic reaches outside `vld2`..`vld4`/`vst2`..`vst4`
  and the `_x2`..`_x4` forms: the immediate shifts `vshl_n`, `vshrn_n`,
  `vsra_n`, `vsri_n`, `vmovl`, `vqmovn`, `vfma` by lane, the float width
  `vcvt`s, `vpmin`/`vpmax`, `vabd`, `vqadd`, `vminnm`/`vmaxnm`, the
  reciprocal estimates and steps, `vrnd*`, `vcnt`, `vclz`, `vcmla*`;
  the widening multiply `vmull_v`: spec/landed/sema/WP-724.md "NEON
  widening multiply"; the pairwise widening add `vpaddl[q]` as
  `{s,u}addlp.VTy.N` of the operand bitcast to twice the lanes at half the
  width, and `vqdmulh[q]`/`vqrdmulh[q]` as one `sq[r]dmulh` call: WP-792,
  `target.neon/ir-paddl-qdmulh-19.cpp`; the structure loads and stores
  `vld2`..`vld4`, `vst2`..`vst4` and the shift-and-insert `vsli_n`:
  spec/landed/sema/WP-799.md "NEON structure loads and stores"; the
  cryptographic extension `vaes{e,d,mc,imc}q_u8`, `vsha1{c,m,p}q_u32`,
  `vsha1h_u32`, `vsha1su{0,1}q_u32`, `vsha256h[2]q_u32` and
  `vsha256su{0,1}q_u32` as one call of the non-overloaded
  `llvm.aarch64.crypto.*` intrinsic of that name on `<16 x i8>`,
  `<4 x i32>` and `i32` operands, the AES type code dropped: WP-811,
  `target.neon/crypto-run.cpp`, `ir-crypto-19.cpp`). The
  AArch64 ACLE builtins (`__builtin_arm_*`,
  `arm.rs`) are spec/landed/sema/WP-724.md "ACLE builtins".
- With `CodegenOptions::sve` (AArch64 only), every function definition
  and the global-initialization functions (`__cxx_global_var_init`,
  `_GLOBAL__sub_I_*`, `__tls_init`: `init_fn_attrs`) carry
  `vscale_range(1,16)` (clang's `CodeGenFunction` for a target with SVE);
  the TLS wrapper and `__clang_call_terminate`, built outside it, do not.
  With `CodegenOptions::sve_vector_bits` (`-msve-vector-bits=N`, with or
  without `sve`) the range is `vscale_range(N/128,N/128)` instead (WP-469).
  Tests `ext.arm.sve/ir-target-features-sve.cpp`, `ir-sve-vector-bits.cpp`,
  `ir-target-features-march-levels.cpp`.
- SVE scalable vectors (WP-469; test `ext.arm.sve/ir-scalable-constants.cpp`):
  - A constant splat into a scalable vector (`v + 1`, the `-1` of `~v`,
    the zero of `-v`, a scalar `?:` arm) is `ConstData::Splat`
    (`Module::const_splat`; `ConstantVector::getSplat`): an all-zero lane
    folds to `zeroinitializer` (likewise undef/poison). It prints as
    `splat (T c)` for LLVM 20+ and as LLVM 19's
    `shufflevector (<vscale x N x T> insertelement (<vscale x N x T> poison,
    T c, i64 0), <vscale x N x T> poison, <vscale x N x i32>
    zeroinitializer)` before (`Module::ir_version`). A non-constant splat is
    `insertelement poison, x, i64 0` + a scalable `shufflevector` with a
    zero mask (`func.rs` `splat`, `shuffle`); no fixed-vector folding.
  - A vector `?:` on a sizeless condition is element-wise: an `svbool_t`
    condition selects directly at compat 22 and is compared
    `icmp ne … zeroinitializer` at compat 19, as every other SVE condition
    is; then `select`.
  - A comparison yields `<vscale x N x i1>` (`cmp_ty`), zero-extended to the
    result type unless that is `svbool_t` (`vec_cmp_result`); unsigned
    lanes use the unsigned predicates, floats `fcmp oeq` etc.
  - `v[i]` (spec/sema.md §5.19a) is a vector element lvalue as for a GNU
    vector (`vec_subscript`, `LValue::MakeVectorElt`): `extractelement`
    on a read, `insertelement` and a store back on a write (test
    `ext.arm.sve/ir-subscript.cpp`).
  - Allocas of a sizeless type are aligned as clang's `getTypeInfo`: 16
    for an SVE data vector, 2 for `svbool_t` and `__SVCount_t`
    (`kiln_abi` `target_types`; sizeof/alignof stay ill-formed in sema).
  - Not yet: a C++26 erroneous-value (`-ftrivial-auto-var-init`) store is
    emitted for an SVE local; clang emits none (`kiln_codegen`
    `auto_init.rs`, WP-489). The test passes
    `-ftrivial-auto-var-init=uninitialized`.
## 7v. Landed (WP-456): `PseudoObjectExpr`

The synthesized three-way comparison of a defaulted `operator<=>`
([class.spaceship]/1; spec/sema-class.md §4.12) is a `PseudoObjectExpr`
(ast.md §5.3) whose semantic expressions are `[OVE(a), OVE(b), chain]`.
As clang's `emitPseudoObjectExpr`, `Cg::bind_pseudo` emits the semantic
expressions before the result in order: an `OpaqueValueExpr` binds its
source once in `f.opaque` (an lvalue address for a glvalue source, else
the scalar), any other is emitted and ignored; the result is then
emitted in the requested form (scalar `emit_arm`, `emit_lvalue`,
`emit_agg`, complex `emit_cx`) and `unbind_pseudo` drops the bindings.
The syntactic form is never emitted. The chain is an lvalue of the
category's `const` static members, which the defaulted body copies into
its return slot (`static_cast<R>`), so the IR is clang's
(`tests/codegen/synth_three_way.cpp`, strong, weak and partial
categories, compared with clang 22 and clang 19 on both targets).

## 7z. Landed (WP-462): local externs, asm labels, placeholders, `-O1` details

- **Block-scope `extern` declarations** (spec/abi.md §8.27) name their
  namespace-scope entity also in an instantiation of a function
  template: `template<class T> void f() { extern void g(int); extern int
  ev; }` declares `@_Z1gi` and `@ev = external global i32`, not
  `_ZZ1fIiEvvE…` locals (`tests/codegen/local_extern_instantiation.cpp`,
  `tests/conformance/basic.link/local-extern-instantiation-run.cpp`).
- **Shared asm labels** (spec/abi.md §8.27): declarations of different
  types whose labels name one symbol are one function or global, as
  clang's `GetOrCreateLLVMFunction`/`GetOrCreateLLVMGlobal` return the
  existing entry whatever its type; each call keeps its own function
  type. A definition whose type differs from the existing declaration
  replaces it at the end of the module (`kiln_llvm`
  `Module::retype_function_to_end`, clang `Function::Create` + `takeName`
  + RAUW). **Labels on constructors, destructors and conversion
  functions**: every structor variant names the label, the first variant
  defined is the one definition, and the complete-object alias is not
  emitted (`tests/codegen/asm_label_shared.cpp`,
  `tests/codegen/asm_label_structors.cpp`,
  `tests/conformance/dcl.asm/asm-label-shared-and-structors.cpp`).
  A later variant of the same function type has its body appended to
  that definition as unreachable blocks (a labelled virtual destructor's
  `D0` after `D2`: `; No predecessors!`, declaring `_ZdlPvm`); one of
  another type — the complete-object variant of a class with virtual
  bases, without the base-object variant's VTT parameter — replaces the
  definition at the end of the module (`Module::redefine_function_to_end`,
  clang erasing the old definition), so the label is the complete-object
  function and the class's vtable and VTT are emitted (WP-492,
  `tests/codegen/asm_label_structors_vbases.cpp`,
  `tests/conformance/dcl.asm/asm-label-structors-vbases-run.cpp`).
- **Duplicate mangled names** (WP-492; clang `err_duplicate_mangled_name`
  in `GetOrCreateLLVMFunction` with `IsForDefinition`): each symbol name
  has a representative declaration, the first one codegen names it for
  (a reference, a definition, or a deferred labelled definition: clang's
  `Manglings`). A function definition into an already defined function
  whose representative is another entity is the error `definition with
  same mangled name 'N' as another definition` at the definition, with
  `note: previous definition is here` at the representative's
  definition; it is reported once (clang emits no later declaration
  after an error) and no output is written. Structor variants sharing a
  label are one entity. A function defined under the symbol of a defined
  variable (clang's error) is not detected
  (`crates/kiln_driver/tests/codegen.rs` `duplicate_mangled_name`).
- **Incomplete class in a signature**: a function first referenced while
  a class parameter or return type is incomplete (never defined, or
  defined later in the TU) is declared `void ()` with no attributes, as
  clang does for a type `isFuncTypeConvertible` rejects; the declaration
  stays when the class is completed, calls use their call-site function
  type, and a definition retypes it. A class is complete to the
  top-level walk from the end of the top-level declaration during which
  it was completed (clang `HandleTopLevelDecl`; a namespace or
  linkage-specification block is one declaration, so a class defined
  later in the same block is complete): the parser records the TU's
  last member and the class-completion epoch at the end of each
  (`DeclArena::end_top_level_decl`), a TU member takes the epoch of the
  first end at or after it in the member list (a class specialization
  first declared by an explicit instantiation is appended there), the
  explicit instantiation definitions that follow a member take the next
  member's, the walk compares the class's completion index against that
  epoch, and after the walk every class is complete. Class template specializations,
  member classes of them and explicit specializations count by where
  they are completed, local classes and closure types are complete with
  their enclosing declaration (WP-492,
  `tests/codegen/incomplete_class_inst.cpp`,
  `temp.inst/incomplete-specialization-signature-run.cpp`). A pointer
  or reference to an incomplete class has no `dereferenceable` size. A virtual method's placeholder is
  `unnamed_addr`, and so is its thunk's
  (`tests/codegen/incomplete_class_sig.cpp`,
  `tests/codegen/incomplete_class_thunk.cpp`). A braced list initializing
  a class through a type written before the class was defined now reads
  the definition's fields
  (`list-init-of-class-declared-incomplete-run.cpp`).
- **Lifetime markers at `-fclang-compat=22`** use LLVM 22's
  `(ptr captures(none))` form (§7p, WP-370, "Lifetime markers"; driver.md
  "IR optimization"); the `lifetime_o1`, `extern_template_vtable_o1` and
  `loop_hints_o1` deviations are gone.
- **Variadic thunks** are clang's `EmitMustTailThunk`: the thunk passes
  its own arguments with the adjusted `this` to a `musttail` call that
  forwards `...`, returns its result, and the generic epilogue lands in
  a dead block (x86-64 functions keep `align 2`). Before this a
  this-adjusting thunk dropped the variadic arguments. Above `-O0` the
  vtable's such thunk is `available_externally` when the vtable is; a
  return-adjusting variadic thunk of a method defined elsewhere is a
  declaration (`tests/codegen/variadic_thunk{,_o1,_o1_nopasses}.cpp`,
  `tests/conformance/class.virtual/variadic-thunk-run.cpp`).
- **Return-adjusting variadic thunks** (WP-492) are clang's
  `GenerateVarArgsThunk`, emitted after their method's definition
  (`emit_thunks`): `gen_code_or_thunk` keeps the finished `Fir` of every
  variadic virtual method (`Cg::vararg_bodies`), and the thunk is a copy
  of it (`Fir::clone_body`, LLVM `CloneFunction`): moved to the end of
  the module, the module's unique-name counter advanced once (the clone
  is first named after the method), with the method's attributes,
  `unnamed_addr`, `align`, personality and section, then the method's
  linkage and a comdat of its own (`setThunkProperties`). The this
  adjustment is inserted before the first `store` of the `this`
  argument in the entry block, which then stores the adjusted pointer;
  the first block in layout order ending in `ret` loses its `ret`, the
  returned value takes `PerformReturnAdjustment` (a pointer null-checked
  in blocks placed after the returning block, `Builder.CreateBr` keeping
  each insertion point; a constant pointer's check folded as LLVM's
  constant folder does: `null` is null, a global that is not
  `extern_weak` or an `inbounds` GEP of one is not) and a new `ret`
  returns it. A body that already made a block for a `blockaddress`, or
  a method never defined here, is diagnosed instead
  (`tests/codegen/variadic_thunk_ret{,_o1}.cpp`,
  `tests/conformance/class.virtual/variadic-covariant-thunk-run.cpp`).
- **`isLibFunction`** is builtin attribute bit `LIB_FUNCTION` (128,
  clang's `F`), set on the 406 `__builtin_` rows clang 19 and 22 treat as
  library functions (found by compiling each labelled `extern template`
  body at `-O1 -disable-llvm-passes`: a declaration means library); the
  trivially-recursive check (§7a) reads it
  (`tests/codegen/trivially_recursive_lib.cpp`).
- **`assume_safety` access groups** (§4, loop metadata):
  a distinct group per parallel loop, `llvm.loop.parallel_accesses` in
  its id, `!llvm.access.group` on each memory-accessing instruction
  `Fir::push` appends while it is active (the union under nested
  parallel loops; calls to `memory(none)` callees skipped), exact against
  clang 19 and 22 (`tests/codegen/loop_parallel{,_o1}.cpp`;
  `loop_parallel_jam.cpp`, where clang 19, and kiln at
  `-fclang-compat=19`, makes the `followup_inner` properties a distinct
  self-referential node; WP-492). An instruction kiln places away from the
  insertion point follows how clang makes it (WP-492): one made by
  `new X(.., InsertPt)` or appended directly (`Fir::insert_before`,
  `insert_after`, `insert_before_last`, `push_in`: the `cleanup.cond`
  flag stores of `setBeforeOutermostConditional` and
  `SetupCleanupBlockActivation`, the branch fixups' `cleanup.dest.slot`
  stores, the `cleanup.dest` load of a cleanup's exit switch, the
  `tmp.exprcleanup` spills of a `CGBuilderTy(CGM, ..)`) gets no access
  group; one made by the function's builder at a moved insertion point
  (`Fir::builder_insert_before_last`: the `llvm.lifetime.start` of a
  full-expression temporary in a conditional branch, put at the
  outermost conditional's start) gets the active one
  (`tests/codegen/loop_parallel_inserts{,_o1}.cpp`).
- **Not modelled**: `"; No predecessors!"` comments (vtable hiding,
  `isVTableHidden`, is modelled since WP-544: §8); the emission
  order of inline (deferred) labelled structors of a class with virtual
  bases.

## 7y. Landed (WP-452): `=pattern`, pre-fills and VLAs (`auto_init.rs`)

The rest of clang's `EmitAutoVarInit` (`CGDecl.cpp`) on top of §7r, so
`-ftrivial-auto-var-init=zero|pattern` and kiln's C++26 default give
clang's IR for every automatic variable. The fill mode is
`Cg::auto_init_mode`: none for `=uninitialized`, a `constexpr`, static,
thread-local or non-local variable, or one marked `[[indeterminate]]`
or `[[clang::uninitialized]]`.

- **Pre-fill** (`Cg::auto_init_prefill`, from `expr.rs`
  `emit_var_init`). A variable whose initializer is not a constant is
  filled first and then initialized, when its type is not a clang
  *scalar* (builtin, enumeration, pointer, member pointer, `_Complex`,
  `_BitInt`), or when it is but the initializer names the variable
  itself (`int x = x + 1;`, clang `isAccessedBy`). So a class with a
  non-trivial default constructor, an array of them, an aggregate or
  array initialized from a call, a structured binding's object, a
  reference (`int &r = f();`, the slot only), an `_Atomic` and a vector
  are filled; `int i = g();` is not. Clang's *scalar* and kiln's
  single-store type differ: a vector is not scalar but is one store.
- **Constant initializer.** One that lowers to a constant is stored as
  it was (§3); under a fill mode a non-null constant first has its
  `undef` parts replaced by the fill value and its padding made
  explicit as zero bytes (clang `replaceUndef`, then `constWithPadding`
  with `IsPattern::No`, even under `=pattern`), which changes the
  `__const` global and the store shape. A null constant is unchanged.
- **Store shape** (clang `emitStoresForConstant`, `Cg::stores_for_constant`).
  An integer, pointer, floating or vector type is one store. Otherwise:
  an all-zero value, or one larger than 32 bytes with at most 6
  non-zero stores, is `llvm.memset` 0 plus those stores; one larger
  than 32 bytes whose bytes are all one value (clang `isBytewiseValue`:
  `half`, `float` and `double` only among floating types, `inttoptr`
  of an integer looks through to it) is `llvm.memset` of that byte; anything
  else is an `llvm.memcpy` from `@__const.<mangled function>.<variable>`
  (`private unnamed_addr constant`) at the variable's alignment. Every
  store and call carries `!annotation !{!"auto-init"}`.
- **`=pattern`** (clang `initializationPatternFor`). An integer (any
  width, `bool` and `_BitInt` included) is all `0xAA` bytes; a pointer
  (and `nullptr_t`) is `inttoptr (i64 0xAAAAAAAAAAAAAAAA to ptr)`; a
  floating value is all-ones bits (a quiet NaN); a vector splats its
  element's pattern; arrays and classes are built element by element.
  Padding is explicit `[N x i8] c"\AA..."` in a literal struct
  (`constWithPadding`, `IsPattern::Yes`), packed when the members
  require it; an array is rebuilt only when its element type changed.
  So `int big[20]` is `memset i8 -86`, `float f[40]` is `memset i8 -1`,
  and a padded struct is a `memcpy`.
- **VLAs** (`Cg::auto_var_init_vla`, after `emit_vla_var`'s alloca).
  `-ftrivial-auto-var-init-max-size` does not apply. `=zero` is one
  `llvm.memset` of the run-time size (`mul nuw i64 %count, <element
  size>`, the `mul` omitted for a 1-byte element) at the alloca's
  alignment. `=pattern` is clang's loop: `icmp eq i64 %count, 0`
  branches past it; a setup block computes the size and the end
  pointer (`getelementptr inbounds i8`); the loop body's `phi` walks the
  elements, `memcpy`ing one element's pattern from a `@__const` global
  (element alignment) to each, and advances by the element size.
- **alloca** (WP-1118; clang `initializeAlloca` in `CGBuiltin.cpp`,
  `Cg::auto_init_alloca` from `checked.rs` `emit_alloca`). The memory of
  `__builtin_alloca` and `__builtin_alloca_with_align` (and the library
  `alloca`, a builtin in GNU modes such as `-std=gnu++26`) is filled right after its
  `alloca i8, i64 %n`: one `llvm.memset` of the same `%n` with byte 0
  (`=zero`) or `0xAA` (`=pattern`, `i8 -86`), at the alloca's alignment
  (`__BIGGEST_ALIGNMENT__` or the requested one), non-volatile, with
  `!annotation !{!"auto-init"}`. `__builtin_alloca_uninitialized` and
  `__builtin_alloca_with_align_uninitialized` are never filled.
  `-ftrivial-auto-var-init-max-size` does not apply (clang checks no
  size), and `[[clang::uninitialized]]` on a variable the pointer
  initializes does not stop it (the attribute is about the variable's
  own bytes). kiln's C++26 default (`zero`) fills alloca memory too:
  the default is `-ftrivial-auto-var-init=zero`, although
  [basic.indet] only covers automatic variables. clang's
  `-ftrivial-auto-var-init-stop-after=N` counter (`stopAutoInit`, which
  counts alloca fills with variable fills) is not implemented: the
  driver does not accept the flag.
- **Size limit.** The §7r limit and remark apply to every fill except
  a VLA's and an alloca's. A limit of 0 is none.
- **`[[clang::uninitialized]]`** (`AttrKind::UNINITIALIZED`, GNU
  `__attribute__((uninitialized))` too): on a local variable it turns
  the fill off, as `[[indeterminate]]` does (unlike that one it has no
  standard meaning, so the value is indeterminate only as far as kiln
  chooses; the constant evaluator ignores it). Elsewhere it is ignored
  with clang's warning `'clang::uninitialized' attribute only applies to
  local variables [-Wignored-attributes]`.
- **Not modelled:** the pattern of an SVE scalable
  vector, which kiln_llvm cannot spell as a constant, so it is filled
  with zero (clang: a splat); in the memset-plus-stores budget a vector
  constant counts as one store, where clang walks a
  `ConstantDataSequential`'s elements (no case found that differs).
- Tests: `tests/codegen/auto_var_init_prefill.cpp` (`=zero`),
  `auto_var_init_pattern.cpp`, `auto_var_init_vector.cpp` (`=pattern`,
  `min-oracle: 22`: clang 19 spells the constants without `splat`),
  `auto_var_init_max_size.cpp`, `auto_var_init_alloca.cpp` (`=zero`),
  `auto_var_init_alloca_pattern.cpp`; `crates/kiln_driver/tests/erroneous_init.rs`;
  `tests/conformance/basic.indet/` `run-erroneous-prefill.cpp` (the C++26
  default), `run-pattern.cpp` (compared with clang),
  `run-alloca-zero.cpp`, `run-alloca-pattern.cpp` (compared with clang),
  `run-alloca-default.cpp` (the C++26 default), `uninitialized-attr.cpp`.
## 7ab. Landed (WP-466): linkage, 3-lane stores, file-scope compound literals, end-of-TU variables
- **`const volatile` linkage** ([basic.link]/3.2; spec/sema.md): a
  namespace-scope `const volatile` variable that is not `static` has
  external linkage (`kiln_abi::mangle::internal_by_storage` tests
  `const` and not `volatile`, for an array through its element type), so
  it is a mangled `dso_local` definition emitted even when unused
  (`tests/codegen/const_volatile_linkage.cpp`).
- **3-lane ext-vector stores** (`func.rs` `vec_store`): a `<3 x T>` value
  is widened to its `<4 x T>` storage by `shufflevector <3 x T> %v, <3 x
  T> undef, <4 x i32> <0, 1, 2, 3>` at compat 22 (clang 22), by `... <3 x
  T> poison, <0, 1, 2, poison>` at compat 19 (clang 19). Loads are the
  same at both. The oracle-22 `Allow::Structure` rows of
  `builtins_shufflevector`, `ext_vector_element`, `vector_abi` and
  `vector_const` are gone (`tests/codegen/ext_vector3_store.cpp`).
- **File-scope array compound literals** (spec/sema.md §5.8; clang
  `tryEmitGlobalCompoundLiteral`): the literal's implicit static `Var`
  (`DeclTable::compound_literal_of`) is emitted by `get_gvar` at its
  first reference, its initializer constant first, as `@.compoundliteral`
  (uniqued `.compoundliteral.N` on the module's one name counter;
  `var_symbol`), `internal global`, or `internal constant` for a const
  element type. `emit_lvalue` names the global. Alignment: the variable
  rule at compat 22; the type alignment at compat 19, without x86-64's
  16-byte alignment of a large array (clang 19 `getTypeAlignInChars`).
  Clang 19 and 22 are otherwise identical
  (`tests/codegen/file_scope_compound_literal.cpp`, both oracles).
- **End-of-TU variable definitions** (WP-440; spec/sema-templates.md
  §4.6 "Points of instantiation"): an implicitly instantiated variable
  template specialization is recorded in `end_of_tu_defs` like a
  static data member, after the instantiations its initializer requires
  (post-order). Its global, its
  `__cxx_global_var_init.N` and its `@llvm.global_ctors` entry follow
  everything else in the TU, in clang's order, at both compat levels
  (`tests/codegen/var_template_init_order.cpp`,
  `var_template_thread_local.cpp`). `instantiated_var_def` treats any
  variable template specialization as its own definition (one without
  an initializer is default-initialized).

## 8. Symbol visibility and `dso_local` (WP-544; `visibility.rs`)

Every global value carries clang's ELF visibility and `dso_local` flag,
computed as clang's `CodeGenModule::setGlobalVisibility` /
`NamedDecl::getLinkageAndVisibility` (`getLVForDecl`) and
`shouldAssumeDSOLocal` do. Inputs: `CodegenOptions::visibility` (value
visibility, `-fvisibility=`), `type_visibility` (vtables and RTTI; equal to
`visibility` except under `-fvisibility-ms-compat`, where it is default),
`inlines_hidden` (`-fvisibility-inlines-hidden`), `reloc`
(`-fpic`/`-fpie`/`-fno-pic`), `semantic_interposition` and
`direct_access_external_data` (spec/driver.md "Options"); and sema's
`AttrKind::VISIBILITY` / `TYPE_VISIBILITY` attributes (`a`: 0 default,
1 hidden, 2 protected; spec/sema.md "Visibility attributes"), which
include those `#pragma GCC visibility push` gives a declaration.

**Where it applies.** A definition takes its entity's computed
visibility; a declaration (and an `available_externally` definition) only
an explicit one, so an entity defined elsewhere is `hidden` only when
known hidden (`Cg::decl_visibility(d, definition)`). Functions
(`set_decl_attrs`, `set_def_linkage`), variables (`get_gvar`,
`emit_var_def`), static locals of inline functions and their guard
variables, thunks (the method's), structor aliases (the destructor's)
follow `decl_visibility`. A class's vtable, VTT and construction vtables
take the class's type visibility (`Cg::class_visibility`; a construction
vtable is the most-derived class's, as clang's `setGVProperties(VTable,
RD)`); a `type_info` object and name take the type's
(`Cg::type_visibility`, clang `Type::getVisibility`: a class or
enumeration's, through pointers, references, arrays, member pointers and
function types); an external `type_info` declaration only an explicit one.
A template parameter object takes its type's visibility. A local-linkage
symbol is always default (`Cg::finish_dso_local`).

**The visibility of a declaration** (`Cg::lv`, memoised per `(decl,
computation)`; clang's `LVComputationKind`: value or type visibility, and
"ignore explicit" while computing a class for a member with its own
attribute). Visibilities merge by clang's `mergeVisibility`: the result is
never less restrictive (hidden < protected < default), and an equal one
only adds explicitness.
- *Explicit visibility* (clang `getExplicitVisibility`): the entity's own
  `visibility` (for type visibility `type_visibility` first) on any
  redeclaration; else that of the pattern it was instantiated from (a
  member of a class template specialization, a class, variable or
  function template specialization, the template). clang does not clone
  the attribute onto an instantiation, and neither does kiln's
  `copy_attrs`; an explicit instantiation's own attribute is the
  specialization's (spec/sema.md "Visibility attributes"; libc++ exports
  hidden function templates' `extern template` specializations this
  way). A replaceable global allocation or deallocation function
  is default (clang's `-fvisibility-global-new-delete=force-default`).
- *Namespace scope* (clang `getLVForNamespaceScopeDecl`): explicit
  visibility, else the nearest enclosing namespace's attribute (any of its
  definitions; it counts as explicit), else the global mode, then hidden
  for an inline function under `-fvisibility-inlines-hidden`
  (`use_inline_hidden`: a function defined inline, `constexpr` or
  implicitly inline, not an explicit instantiation). A non-`extern "C"`
  variable without explicit visibility merges its type's. A template
  specialization merges its arguments' (`args_lv`, clang
  `getLVForTemplateArgumentList`: a type's `type_lv`; a declaration,
  function or member named by a pointer or member pointer; a template
  template argument's template; a pack's elements; a class-type value's
  type, as its template parameter object), when clang's
  `shouldConsiderTemplateVisibility` holds: an implicit instantiation, or
  an explicit specialization or instantiation without its own attribute
  (for a class, also not merely ignoring explicit visibility).
- *Class members* (clang `getLVForClassMember`): the member's explicit
  visibility, else `-fvisibility-inlines-hidden`'s hidden; its template
  arguments; then its class's visibility (type visibility for a nested
  class), unless an explicit attribute on a member specialization
  suppresses that (clang's "suppressor" rule).
- *Local entities* (clang `getLVForLocalDecl`): a block-scope function
  declaration has only its own attribute; a static local, local class or
  closure type of an inline function or a template instantiation takes the
  function's visibility, except that the static local of a function hidden
  only by `-fvisibility-inlines-hidden` takes its class's explicit
  visibility, else the global mode (so such guard variables stay default).
- `vis_active`: with every mode default and no visibility attribute in the
  TU, the whole computation is skipped.

**`isVTableHidden`** (clang `CGVTables::isVTableHidden`, checked first by
`canSpeculativelyEmitVTableAsBase`; `Cg::is_vtable_hidden`): above `-O0` a
vtable defined in another translation unit is not emitted
`available_externally` (§7a) when its class's type visibility is hidden,
or a used slot's method (function or destructor) is hidden and not
defined here. `tests/codegen/visibility_vtable_hidden_o1.cpp`.

**`dso_local`** (`assume_dso_local`, clang 19's `shouldAssumeDSOLocal` for
ELF, applied by `finish_dso_local` after emission): implied (not printed)
for local linkage and for a hidden or protected symbol; never for an
intrinsic or the `extern_weak` `_ZTH` declaration. Under `-fpic`/`-fPIC`
only an external, non-COMDAT function definition is, and only with
`-fno-semantic-interposition` (`GlobalValue::canBenefitFromLocalAlias`).
Under `-fno-pic` every definition is; under `-fpie` a definition is as
emission chose (clang decides when it creates the global). A declaration
is `dso_local` under `-fpie`/`-fno-pic` with direct access to external
data (`-fdirect-access-external-data`, default only for `-fno-pic`) when it
is a non-TLS variable, or a function under `-fno-pic`. Module flags:
`PIC Level`, and `PIE Level` under `-fpie`; `SemanticInterposition` under
`-fPIC -fsemantic-interposition`; `direct-access-external-data` when its
value differs from the relocation model's default. COMDATs are
unaffected: a COMDAT member keeps its group whatever its visibility.

**Deviations from clang's model** (none shows in the corpus):
1. Template parameters' visibility (clang
   `getLVForTemplateParameterList`: the types of non-type parameters and
   template template parameters' lists) is not merged.
2. The entities a class-type or array non-type argument's value names are
   not merged (only the class type is).
3. No `getLVForClosure` special case: a closure type's visibility is
   computed as for a local class of its context (clang gives a lambda in a
   default argument or a namespace-scope variable initializer its owner's).
4. Linkage is not part of the computation: an entity with internal
   linkage (an unnamed namespace, `static`) is emitted with local linkage
   and so default visibility, which is what clang emits.
5. `-fvisibility-global-new-delete=` and `-fvisibility-from-dllstorageclass`
   are not accepted (only clang's defaults are modelled); `weak`, `dllexport`
   and `__declspec` do not affect visibility.

Tests: `tests/codegen/visibility_{hidden_tu,inlines_hidden,class_attr,
namespace,pragma,templates,vtable_hidden_o1}.cpp`,
`tests/codegen/dso_local_{nopic,pie,pic,pic_hidden,pic_nosi,pic_si_direct}.cpp`
(both oracles, both triples), `crates/kiln_driver/tests/codegen.rs`
`pic_visibility_and_objects` (`-fPIC -S -emit-llvm` and `-fPIC -c`).

**Sections** (WP-716; clang `SetFunctionAttributes`,
`setNonAliasAttributes`, `EmitStaticVarDecl`): a GNU `section("name")`
on any declaration of a function or of a variable with static storage
duration (`Ast::section_of`) gives its LLVM function or global
`section "name"`, whether it is defined here or only declared, a static
local and an instantiated template included, so the ELF linker's
`__start_name`/`__stop_name` bound the section. Section type conflicts
(clang `err_section_conflict`) are not diagnosed.
`tests/codegen/section_attr.cpp`, `tests/conformance/ext.gnu.section/`.

## 9. Aliases and ifuncs (WP-763; `alias.rs`)

GNU `alias("sym")` and `ifunc("resolver")` on a namespace-scope function
or variable (sema rules in spec/sema.md §4.5) make the declaration a
definition: an IR alias or ifunc, never a function body or a global with
an initializer (clang `CodeGenModule::EmitAliasDefinition`,
`emitIFuncDefinition`, `checkAliases`).

- **When.** At the first declaration carrying the attribute
  (`Ast::alias_definition`), in source order with the other top-level
  emission; an alias already emitted for the entity is not emitted again.
- **Target.** The string is the target's *mangled* symbol name, looked up
  as-is: an existing function or global of that name; else a deferred
  definition (an unused inline function, a static function, an internal
  variable) whose mangled name it is, which is emitted now; else a
  declaration created under that name, which a later definition
  completes in place (a function: the aliasing declaration's signature,
  retyped by the definition if they differ; when the types agree the
  definition keeps the return and parameter attributes the aliasing
  declaration gave it, as clang's `GetOrCreateLLVMFunction` sets them
  once, so an `extern "C"` alias of a C++ function leaves its
  definition without a `noundef` return (`AliasState::declared_for`,
  WP-777, `tests/codegen/gnu_alias_forward_extern_c.cpp`); a variable: a global of the
  aliasing declaration's type; an ifunc resolver: a `void ()` placeholder
  that the resolver's definition replaces and moves to the end of the
  function list, as clang). A later definition whose mangled name an
  alias named is emitted even when nothing else uses it.
- **IR.** `@name = <linkage> <visibility> alias <ty>, ptr @target` with
  `<ty>` the declaration's function or value type, or `@name = ifunc
  <fnty>, ptr @resolver`. An earlier declaration of `@name` (a use) is
  replaced by the alias (`replace_function_with_alias` /
  `_with_ifunc` / `replace_global_with_alias`, spec/llvm.md §2.5). An
  alias of a variable copies its `thread_local` mode (and gets no TLS
  wrapper, `tls.rs`).
- **Linkage and visibility.** An ifunc is `external`. An alias is
  `internal` when the declaration has internal linkage, `weak` when it is
  `weak`, else `external`. Visibility is the declaration's (`visibility`
  attribute, `-fvisibility`; §8); every non-internal alias or ifunc is
  `dso_local`.
- **`weakref`.** Not an alias: a use of the declaration is a use of the
  target symbol, declared `extern_weak` (a global variable target with no
  `align` and not `constant`, as clang's `GetOrCreateLLVMGlobal` without
  a declaration) unless the symbol is also referenced or defined
  normally (`Ast::weakref_target`, clang `GetWeakRefReference`). Test:
  `tests/conformance/ext.gnu.weak/weakref-run.cpp`.
- **Diagnostics at emission.** An ifunc (or an alias, through an asm
  label) whose name is already a definition is `definition with same
  mangled name 'x' as another definition` with `previous definition is
  here` (the alias is not emitted). An alias naming itself is `alias
  definition is part of a cycle`.
- **Checks at the end of the module** (`check_aliases`, after all
  emission, skipped once an error has been reported, as clang's
  `checkAliases` under `Diags.hasErrorOccurred()`), in alias order:
  - the chain of aliases from each one must end at a function or global
    *definition*: a cycle is `alias definition is part of a cycle`; an
    undefined end is `{alias must point to a defined variable or
    function|ifunc must point to a defined function}` with `the {function
    or variable|function} specified in an {alias|ifunc} must refer to its
    mangled name`, and then, for every function or variable of the TU
    whose unqualified name is the target string, `{function|variable} by
    that name is mangled as "sym"` with a fix-it replacing
    `alias("…")` / `ifunc("…")` (up to the attribute's `)`) by the
    mangled spelling, sorted by mangled name;
  - an ifunc's resolver must be a function returning a pointer: `ifunc
    resolver function must return a pointer`;
  - a `section("s")` on the alias that differs from the aliasee's is
    `{alias|ifunc} will not be in section 's' but in the same section as
    the {aliasee|resolver}` [-Wignored-attributes];
  - an alias whose aliasee is a weak (`weak`, `linkonce`, `extern_weak`,
    `common`) alias is `{alias|ifunc} will always resolve to t even if
    weak definition of w is overridden` [-Wignored-attributes] and is
    re-pointed at that alias's aliasee;
  - every ifunc resolver gets `disable_sanitizer_instrumentation`.
- These diagnostics are reported at code generation (not under
  `-fsyntax-only`), at the attribute, as clang.
- Not done: clang's diagnostics for targets without alias or ifunc
  support (kiln targets ELF only).

Tests: `tests/codegen/gnu_alias.cpp`, `tests/codegen/gnu_ifunc.cpp`,
`tests/codegen/gnu_weakref.cpp` (ir_vs_clang),
`crates/kiln_driver/tests/codegen.rs` `alias_ifunc_diagnostics`,
`tests/conformance/ext.gnu.alias/`.

## Landed sections after WP-488: `spec/landed/codegen/`

From WP-488 on, a work package's Landed section is not appended here: it
is its own file, `spec/landed/codegen/WP-NNN.md` (title line
`# WP-NNN — <title>`, the same content as a section above), so open pull
requests never conflict on the end of this file (spec/README.md "Landed
sections"). They are as normative as this file; read that directory
after the numbered Landed sections above.

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
   direct-referee use (deviations above).
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
