# Design note: sanitizers (`-fsanitize=`)

Status: WP-675, with the remaining UBSan checks from WP-871 (§1.5). The contract lives in spec/driver.md (§3, sanitizer
options), spec/codegen.md (sanitizer instrumentation) and spec/sema.md
(the `no_sanitize` attribute family). The landed summary, with the list
of checks not yet done, is spec/landed/codegen/WP-675.md. WP-871's is
spec/landed/codegen/WP-871.md.

Everything below was taken from clang 19 (`clang++-19 -###`, `-emit-llvm
-Xclang -disable-llvm-passes`, and `opt-19 -print-pipeline-passes`) on
aarch64 and x86_64 Linux. clang 19 is the oracle; where clang 22 differs
it says so.

## 1. What clang's frontend does, per sanitizer

There are two kinds of sanitizer:

- **Pass sanitizers** (address, hwaddress, thread, memory, leak,
  fuzzer-no-link). The frontend only marks functions and globals. An
  LLVM pass does all the instrumentation.
- **Frontend checks** (the `undefined` family). The frontend emits the
  check, the handler call and the handler's static data directly.

### 1.1 Function attributes

`CodeGenFunction::StartFunction` adds one attribute per enabled pass
sanitizer to every function that has a body. That includes the
compiler-made `__cxx_global_var_init*` and `_GLOBAL__sub_I_*` functions.

| sanitizer | function attribute |
|---|---|
| address, kernel-address | `sanitize_address` |
| hwaddress, kernel-hwaddress | `sanitize_hwaddress` |
| memtag-stack | `sanitize_memtag` |
| thread | `sanitize_thread` |
| memory, kernel-memory | `sanitize_memory` |
| fuzzer, fuzzer-no-link | `optforfuzzing` |
| leak | (none: LSan is runtime-only) |

The mask is first narrowed by the function's attributes:

- `no_sanitize("x", ...)` removes the named sanitizers (and their group
  members). `no_sanitize_address` and `no_address_safety_analysis` are
  `no_sanitize("address")`. `no_sanitize_thread` is
  `no_sanitize("thread")`, and `no_sanitize_memory` is
  `no_sanitize("memory")`.
- Removing address also removes kernel-address, and the other way
  round. hwaddress and kernel-hwaddress work the same way.
- `disable_sanitizer_instrumentation` gives that attribute and drops
  every `sanitize_*` attribute.
- If bounds was removed, `nosanitize_bounds` is added (and the
  function gets no array-bounds checks, §1.5.2).

The narrowing also applies to the frontend UBSan checks: a function with
`no_sanitize("undefined")` gets no checks.

Under thread, clang emits TBAA even at `-O0`, because TSan uses it. kiln
emits TBAA under thread at every level.

Under address, hwaddress and memory, clang emits `llvm.lifetime.start`
and `llvm.lifetime.end` at `-O0` too (`shouldEmitLifetimeMarkers`), so
the passes can find use-after-scope. It also keeps value names
(`-discard-value-names` is off for address and memory), so ASan and MSan
stack-frame descriptors carry variable names.

### 1.2 Global variables

clang 19 no longer writes `!llvm.asan.globals`. It sets
`GlobalValue::SanitizerMetadata`, which the textual IR prints after the
alignment:

- `, sanitize_address_dyninit` on a global with a dynamic initializer
  (under address only; it feeds `-fsanitize-address-initialization-order`).
- `, no_sanitize_address` on a global whose declaration has
  `no_sanitize("address")` (or `disable_sanitizer_instrumentation`).
  `no_sanitize_hwaddress` and `no_sanitize_memtag` follow the same rule.
- UBSan's own static data (`.src` strings, type descriptors, check data)
  is `no_sanitize_address` when address is also on.

### 1.3 Passes and their options

`BackendUtil.cpp addSanitizers` registers the instrumentation at the
`OptimizerLastEP` extension point. It does this at every optimization
level, so `-O0 -fsanitize=address` still runs a pipeline. The same
pipeline can be built with `opt`:

```
opt -passes='default<On>' --passes-ep-optimizer-last='<P>' <cl flags>
```

| sanitizer | `<P>` | cl flags clang sets |
|---|---|---|
| fuzzer-no-link | `sancov-module` (runs first) | `-sanitizer-coverage-level=3 -sanitizer-coverage-inline-8bit-counters -sanitizer-coverage-pc-table -sanitizer-coverage-trace-compares -sanitizer-coverage-stack-depth` |
| memory | `msan<eager-checks>`, `msan<recover;eager-checks>`, `msan<track-origins=N;eager-checks>`; at O1 and above followed by `function(early-cse<>,instcombine,jump-threading,gvn<>,instcombine)` | |
| thread | `tsan-module,function(tsan)` | |
| address | `asan<>` | `-asan-use-after-scope` when use-after-scope is on (the default; the cl default is off); `-asan-recover` with `-fsanitize-recover=address`; `-asan-use-after-return=<mode>` |
| hwaddress | `hwasan<>`, `hwasan<recover>` | `-hwasan-use-stack-safety=0` at O0 |

The order is sancov, msan, tsan, asan, hwasan, as in clang. For the
options kiln accepts, opt's defaults for globals dead-stripping, ODR
indicators and destructor kind already match clang on Linux, so nothing
else needs to be passed.

### 1.4 UBSan checks

Each check is an `i1` that is true when the operation is fine. Every
instruction the check creates carries `!nosanitize !{}`. Checks of the
same kind are and-ed together, then fed into one branch. What happens
on failure depends on the check kind's mode:

- **recover** (the default for most checks): the branch carries
  `!prof !{!"branch_weights", i32 1048575, i32 1}`. The failure block
  turns each operand into an `i64` (`zext`, `sext` or `ptrtoint`; a
  wider value goes through memory), calls `void
  @__ubsan_handle_<name>(ptr @data, i64...)` with call-site
  `nounwind`, and branches back to the continuation. The handler is
  declared `{ uwtable }`.
- **abort** (`-fno-sanitize-recover=<check>`, and always for `return`
  and `unreachable`): the same, but the handler is
  `@__ubsan_handle_<name>_abort`. The call site is `noreturn
  nounwind`, the declaration is `{ noreturn nounwind uwtable }`, and the
  call is followed by `unreachable`.
- **trap** (`-fsanitize-trap=<check>`): there is no `!prof`. The
  failure block is `call void @llvm.ubsantrap(i8 <kind>)` (call site
  `noreturn nounwind`) followed by `unreachable`. At O0 every check
  gets its own trap block. At O1 and above clang reuses one per kind,
  which only changes block layout after optimization.

The handler kind ids, which are also the trap codes:

```
AddOverflow 0, BuiltinUnreachable 1, CFICheckFail 2, DivremOverflow 3,
DynamicTypeCacheMiss 4, FloatCastOverflow 5, FunctionTypeMismatch 6,
ImplicitConversion 7, InvalidBuiltin 8, InvalidObjCCast 9,
LoadInvalidValue 10, MissingReturn 11, MulOverflow 12, NegateOverflow 13,
NullabilityArg 14, NullabilityReturn 15, NonnullArg 16, NonnullReturn 17,
OutOfBounds 18, PointerOverflow 19, ShiftOutOfBounds 20, SubOverflow 21,
TypeMismatch 22 (handler type_mismatch_v1), AlignmentAssumption 23,
VLABoundNotPositive 24
```

The static data are private, unnamed_addr globals:

- `@.src = private unnamed_addr constant [N x i8] c"<presumed file>\00",
  align 1`.
- A type descriptor is `{ i16 kind, i16 info, [N x i8] c"'<type>'\00" }`,
  `align 2`. An integer has kind 0 and info `log2(bits) << 1 | signed`,
  so `int` is 11. A float has kind 1 and info equal to its bit width.
  Anything else has kind `0xffff`.
- The check data is a `global` (not a constant: the runtime writes the
  "already reported" bit into the location) that starts with `{ ptr
  @.src, i32 line, i32 column }`, followed by the descriptors the handler
  wants:
  - overflow (add, sub, mul, divrem, negate): one type;
  - shift: left type, right type;
  - out_of_bounds: array type, index type;
  - vla_bound_not_positive: one type;
  - load_invalid_value: one type;
  - float_cast_overflow: from type, to type;
  - type_mismatch_v1: type, `i8 log2(alignment)`, `i8 type-check kind`;
  - missing_return and builtin_unreachable: only the location.

The location is the operator's location (the `+` of `a + b`, the `-` of
`-a`). For `return` it is the function's name in its definition
(clang's `CurCodeDecl` location), and for `__builtin_unreachable` it is
the call.

The check sequences (O0, `int` operands shown):

- signed-integer-overflow: `@llvm.{s}{add,sub,mul}.with.overflow.i32`,
  `extractvalue` 0 and 1, `xor i1 %ovf, true`. Unary minus is `0 - x`
  checked with `ssub` and reported with `negate_overflow`. Signed `/`
  and `%` check `a != INT_MIN || b != -1`.
- integer-divide-by-zero: `icmp ne %b, 0`. When both checks are on, the
  two are and-ed into one `divrem_overflow` check.
- shift-exponent: `icmp ule %b, 31` (unsigned compare against the
  width minus one).
- shift-base, for signed `<<` before C++20 only (in C++20 and later
  every signed left shift with an in-range exponent is defined, so clang
  emits no base check): if the exponent is in range, `lshr %a, (31 -
  %b)` (with `sub nuw nsw`), then `lshr` by 1 more, then `icmp eq 0`.
  The result is merged with a `phi`, so the exponent check guards the
  base check. Both feed one `shift_out_of_bounds` report.
- return: at the end of a value-returning function, `br i1 false`
  followed by `missing_return` (always abort), then `unreachable`.
- unreachable: `__builtin_unreachable()` becomes a call to
  `builtin_unreachable` (always abort) instead of a bare `unreachable`.

### 1.5 The remaining `undefined` checks (WP-871)

WP-871 added the checks that WP-675 accepted but did not emit. Each
one is a port of the named clang 19 function. A test cites the
subsection it pins.

| check (`-fsanitize=`) | handler (id) | clang 19 source | kiln | status |
|---|---|---|---|---|
| null, alignment, object-size | type_mismatch_v1 (22) | `EmitTypeCheck` | `ubsan.rs` `ub_type_check` | done (§1.5.1) |
| array-bounds | out_of_bounds (18) | `EmitBoundsCheck` | `ub_bounds` | done (§1.5.2) |
| local-bounds | none (an LLVM pass) | `BoundsChecking` | not run | follow-up |
| bool, enum | load_invalid_value (10) | `EmitScalarRangeCheck` | `ub_range_check` | done at scalar loads (§1.5.3) |
| float-cast-overflow | float_cast_overflow (5) | `EmitFloatConversionCheck` | `ub_float_cast_check` | done (§1.5.4) |
| pointer-overflow | pointer_overflow (19) | `EmitCheckedInBoundsGEP` | `ub_pointer_overflow` | done (§1.5.5) |
| nonnull-attribute | nonnull_arg (16) | `EmitNonNullArgCheck` | `ub_nonnull_arg` | done; not through member-pointer calls (§1.5.6) |
| returns-nonnull-attribute | nonnull_return_v1 (17) | `EmitReturnValueCheck` | `ub_ret_nonnull_*` | done (§1.5.6) |
| builtin | invalid_builtin (8) | `EmitCheckedArgForBuiltin` | `ub_builtin_zero` | done (§1.5.6) |
| implicit-{unsigned,signed}-integer-truncation, implicit-integer-sign-change | implicit_conversion (7) | `EmitIntegerTruncationCheck`, `EmitIntegerSignChangeCheck` | `ub_int_conv` | done (§1.5.7) |
| implicit-bitfield-conversion | implicit_conversion (7) | `EmitBitfieldConversionCheck` | `ub_bf_conv` | done (§1.5.7) |
| checks on `_Atomic` `++`/`--`/`op=` | as the operator | `EmitScalarPrePostIncDec`, `EmitCompoundAssignLValue` | `atomic.rs`, `ub_inc_dec` | done (§1.5.8) |
| `_BitInt` type descriptors | all | `EmitCheckTypeDescriptor` | `ub_type_keyed` | done (§1.5.8) |
| vptr | dynamic_type_cache_miss (4) | `EmitTypeCheck` | not emitted | follow-up |
| function | function_type_mismatch (6) | `EmitCall` prologue data | not emitted | follow-up |
| nullability-arg/-assign/-return | nullability_* (14, 15) | `EmitNullabilityCheck` | not emitted | follow-up |

Rules shared by all of them:

- **Type descriptors.** clang keys its descriptor map by the `QualType`
  object, sugar included, and prints the type with
  `getAsString`. kiln keys by the type and which view of it the
  check names (`UbDesc`): the written type, or the canonical one
  (`getRecordType`, `getThisType`, and the canonicalized types of
  `EmitScalarConversion`). Canonical class and enumeration types print
  with their full scope (`'n1::S'`) in clang 19 and 22. A written type
  prints as written. Sugar that kiln does not keep separately, such as
  `int (&)[4]`'s parenthesized `int[4]` or the deduced `auto` that
  `g(x) + g(x)` keeps as its common type, gets one descriptor in kiln
  where clang makes two (`ubsan-desc-identity-ir.cpp`, an xfail).
- **Locations** come from clang's `getExprLoc`. For an overloaded
  operator call that is its begin location for `()`, `[]`, `->`, `++`,
  `--`, `new`, `delete` and `co_await`, and its operator token
  otherwise.
- With no `-fsanitize`, none of this runs and the IR is unchanged.

#### 1.5.1 Type checks (null, alignment, object-size)

`EmitTypeCheck` runs on every glvalue access (`TCK_Load`, `TCK_Store`,
member access, member call, reference binding, constructor call,
downcast, upcast, `this` in a member function's prologue, `new`,
`delete`, a dynamic operation). It checks:

- null: `icmp ne ptr, null`, unless the kind allows null
  (`isNullPointerAllowed`: the pointer downcast and upcasts and
  `dynamic_cast`), or the pointer is a known non-null constant
  (an alloca or a global);
- object-size, at `-O1` and above: `@llvm.objectsize.i64.p0(ptr, false,
  false, false) >= sizeof(T)`;
- alignment: `(ptrtoint ptr) & (align - 1) == 0`, where the alignment
  is the access's, or the type's natural one. It is folded away when
  the pointer is a global or alloca that is aligned enough, as LLVM's
  constant folder does.

Kiln follows clang's skips: the `this` of an implicit member access
skips null, and a load through a reference skips null and alignment.
The descriptor names the written type, `getRecordType` for
constructor and member-call checks, and `getThisType` for the
prologue check of `this`. An implicit-move `return x;` is loaded
unchecked, as clang visits through the `NoOp` cast. The `memcpy`
family checks the alignment of its pointer arguments as clang 19 does
(`EmitArgCheck`).

#### 1.5.2 array-bounds

`EmitBoundsCheck`: an index into an array whose bound is known
(`getArrayIndexingBound`: a constant array, a VLA, or a vector) must
be `ule` the bound when the element is only addressed, and `ult` when
it is accessed. Flexible-array-like members are not checked. At the
default `-fstrict-flex-arrays=0` that is a trailing array member, a
zero-length array, and a one- or zero-element union member. The data
is the array's descriptor and the index's descriptor.

clang 19 compares in `size_t` and reports the raw index. clang 21 and
later compare in the wider of the index type and `size_t` and report
the converted index (only a 128-bit index differs). kiln follows
`-fclang-compat`.

#### 1.5.3 bool and enum

`EmitScalarRangeCheck` on a scalar load of `bool` (`icmp ule v, 1`
in its `i8` memory form) or of an enumeration without a fixed
underlying type (`[min, end)` of `EnumDecl::getValueRange`,
[dcl.enum]/8: one `icmp ule` when the range starts at 0, else two
compares and-ed). clang 19 converts the value with `EmitCheckValue`
before the branch, while clang 20 and later pass it raw. kiln checks
the loads that go through `load_lv_at` (scalar lvalue-to-rvalue
conversions). clang also checks a few rarer load sites, such as
atomic loads and loads of a `bool` bit-field, and kiln does not yet
(follow-up).

#### 1.5.4 float-cast-overflow

`EmitFloatConversionCheck`, for a floating to integer conversion:
`src > MinSrc && src < MaxSrc`. The bounds are the largest too-small
and the smallest too-large integer values, rounded into the source
format, or an infinity where they do not fit (`half`, `_BitInt(128)`).
The source format is the operand's before any promotion. The check
also covers complex parts, the operand of an atomic compound
assignment, and conversions to `_Atomic` integers, whose bounds are
signed as in clang. Both descriptors are canonical, so a typedef
prints as its canonical type.

#### 1.5.5 pointer-overflow

`EmitCheckedInBoundsGEP`, on pointer arithmetic, `p[i]` through a
pointer, and `++`/`--` of a pointer:

- The byte offset is computed with `@llvm.smul.with.overflow.i64` and
  `@llvm.sadd.with.overflow.i64` (a constant GEP is folded instead).
- Unless null pointers are valid, the base being null must match the
  result being null.
- If the offset is not constant, the result is compared with the base:
  `uge` (or `ule` for a subtraction) for an unsigned index, and a
  `select` on the offset's sign for a signed one.

Both checks are reported together with the base and the result.

#### 1.5.6 nonnull, returns-nonnull and builtin

- `nonnull-attribute`: each pointer argument named by the parameter's
  or the function's `nonnull` attribute is checked non-null before the
  call. The data is the argument's location, the attribute's location
  and the argument number.
- `returns-nonnull-attribute`: a `returns_nonnull` function keeps the
  location of the `return` statement that ran in `return.sloc.ptr`.
  The epilogue checks the value is non-null and reports that location.
- `builtin`: the argument of `__builtin_ctz`/`__builtin_clz` (and the
  `l`/`ll` forms) without a fallback must be non-zero. The last data
  byte is `BCK_CTZPassedZero` (0) or `BCK_CLZPassedZero` (1).

Gaps:

- clang also checks `nonnull` on the `memcpy`/`memset` builtins.
- A call through a member pointer passes no callee to the check.
- A lambda's trailing `returns_nonnull` lands on the wrong
  declaration in sema (`ubsan-lambda-returns-nonnull-ir.cpp`, an
  xfail).

#### 1.5.7 implicit-conversion

`EmitScalarConversion` with clang's implicit-conversion options checks
each implicit integral conversion: implicit casts, compound
assignment, and `++`/`--` of a promoted type. A promoted type is
widened, stepped with a plain `add` and narrowed back.

- truncation: `ext(trunc(src)) == src`, kind 1 (unsigned) or 2
  (signed);
- sign change: the source and result signs agree, kind 3, or 4 when
  the check is combined with a signed truncation.

The data is `{ loc, from, to, i8 kind }`. Neither check is UB, so
the converted value is kept.

`implicit-bitfield-conversion` (`EmitBitfieldConversionCheck`) checks
a store into a bit-field against the value read back from the
bit-field's width. A bit-field `++`/`--` is checked this way and not
by the promoted-step check above.

The `integer` group is the unsigned-overflow, unsigned-shift-base,
implicit integer truncation and sign-change checks. It does not
include implicit-bitfield-conversion, in clang 19 or 22. An explicit
`-fsanitize-trap=implicit-bitfield-conversion` is accepted, as in
clang 22 (clang 19 rejects it).

#### 1.5.8 Atomics and `_BitInt`

- **Atomics.** clang lowers `++`, `--`, `+=`, `-=`, `&=`, `|=` and `^=`
  on an `_Atomic` integer to one `atomicrmw`. That instruction gets no
  overflow check, so a signed operand stays unchecked there, as in
  clang. Two cases go through clang's compare-exchange loop instead,
  where the operation is checked like a plain one:
  - an unsigned operand under `unsigned-integer-overflow`;
  - any other operator (`*=`, `/=`, `%=`, shifts).

  The data names the `_Atomic` type, and the converted operand gets
  the conversion checks (§1.5.4, §1.5.7).
- **`_BitInt` descriptors.** clang 19 describes a `_BitInt(N)` as an
  integer (kind 0) of its storage width. clang 20 and later (#64100)
  give a signed `_BitInt` kind 2, with its exact width appended to the
  name as `\0`, a little-endian `i32` and `\0`. Unsigned `_BitInt`
  keeps kind 0. A `_BitInt` wider than 128 bits is not supported on
  aarch64 by either compiler.

## 2. The driver

The grammar is a comma-separated list for `-fsanitize=` and
`-fno-sanitize=`, read left to right. Groups expand when they are
parsed:

- `undefined` is alignment, array-bounds, bool, builtin, enum,
  float-cast-overflow, function, integer-divide-by-zero,
  nonnull-attribute, null, object-size, pointer-overflow, return,
  returns-nonnull-attribute, shift-base, shift-exponent,
  signed-integer-overflow, unreachable, vla-bound and vptr.
- `shift` is shift-base and shift-exponent. `bounds` is array-bounds and
  local-bounds. `integer` adds the unsigned and implicit-conversion
  checks.
- `all` stands for every sanitizer and is only valid with `-fno-` forms
  and the recover and trap lists.

Some members are dropped silently: vptr with `-fno-rtti`, and
object-size at `-O0`.

`-fsanitize-recover=` and `-fno-sanitize-recover=` take the same list.
The default recover set is the recoverable UB checks, which is everything
except return and unreachable, plus nothing else. Naming `unreachable`
or `return` is an error. Naming address, hwaddress, memory or thread is
accepted. `-fsanitize-trap=` takes only UB checks (and `undefined` and
`all`).

The diagnostics (clang 19 wording):

- An unknown name: `unsupported argument 'foo' to option
  '-fsanitize='`. The same message is used for a name a list may not
  take (`-fsanitize-trap=address`).
- An incompatible pair: `invalid argument '-fsanitize=address' not
  allowed with '-fsanitize=thread'`. The groups are checked in clang's
  order: address vs thread/memory, thread vs memory, leak vs
  thread/memory, kernel-address vs address/leak/thread/memory, hwaddress
  vs address/thread/memory/kernel-address. Each side is printed as the
  last argument that enabled part of it, filtered to the conflicting
  names. The later sanitizer is removed afterwards.
- Not supported on the target: `unsupported option '-fsanitize=<x>' for
  target '<triple>'`. Of the sanitizers kiln implements, all run on
  both x86_64 and aarch64 Linux.

Other options:

- `-fsanitize-ignorelist=` (and the old `-fsanitize-blacklist=`): the
  file must exist (`no such file or directory: '<f>'`). kiln accepts it
  but does not apply it yet (§5).
- `-fsanitize-address-use-after-scope` and its `-fno-` form.
- `-fsanitize-address-use-after-return=never|runtime|always`.
- `-fsanitize-memory-track-origins[=N]`.
- `-shared-libsan` and `-static-libsan`.

**Runtime and link.** kiln links by running `clang++` (spec/driver.md,
link step). Rather than rebuild clang's runtime-selection logic (the
`--whole-archive libclang_rt.asan_static.a libclang_rt.asan.a
libclang_rt.asan_cxx.a`, `--dynamic-list=*.syms`, `-lpthread -lrt
-ldl`, and the per-sanitizer rules), kiln passes the final sanitizer
set on to that link: `-fsanitize=<enabled list>`, plus `-shared-libsan`
or `-static-libsan` if given. The runtime then comes from the link
clang's resource directory. With Debian's packages that is
`/usr/lib/llvm-19/lib/clang/19/lib/linux/`, from `libclang-rt-19-dev`.
When every UB check is in trap mode, no runtime is needed and nothing
is passed on. kiln's own `-resource-dir` holds headers only and plays no
part in runtime lookup.

**Macros and `__has_feature`.** `__has_feature(address_sanitizer)`,
`thread_sanitizer`, `memory_sanitizer`, `hwaddress_sanitizer`,
`undefined_behavior_sanitizer` and friends come from the parsed
sanitizer set. clang 22, but not clang 19, also predefines
`__SANITIZE_ADDRESS__`, `__SANITIZE_THREAD__` and
`__SANITIZE_HWADDRESS__` (GCC's spellings). kiln follows the
`-fclang-compat` level.

## 3. How kiln hosts each part

| part | where |
|---|---|
| sanitizer names, bits, groups, masks | `kiln_ast::sanitize` (shared by driver, sema and codegen) |
| option parsing, groups, diagnostics, recover/trap sets, target check | `kiln_driver::sanitize` |
| `__has_feature` / `__SANITIZE_*__` | `kiln_pp::features` from the normalized argv, and the driver's predefines |
| opt pipeline (extension point plus cl flags; runs at O0 too) | `kiln_driver::codegen_cmd::ir_passes` / `opt_commands` |
| runtime link | `kiln_driver::codegen_cmd::link_command` (passes the set on to clang++) |
| `no_sanitize*` attributes (a mask in `AttrNode`) | `kiln_sema::class_b_attrs` |
| `sanitize_*` and `optforfuzzing` function attributes | `kiln_codegen::class_b::class_b_def_attrs` |
| global sanitizer metadata | `kiln_llvm` global printing, set by kiln_codegen |
| lifetime markers at O0, kept alloca names, TBAA under TSan | the existing kiln_codegen switches, keyed off `CodegenOptions::sanitize` |
| UBSan checks, handler calls, static data | `kiln_codegen::ubsan` |

kiln emits text IR and runs `opt` (spec/driver.md, codegen). Pass
sanitizers therefore need no in-tree LLVM work: kiln marks the IR the
way clang does and asks `opt` for the same extension-point pipeline.
UBSan is pure frontend IR, so it can be compared line by line against
`clang -Xclang -disable-llvm-passes`.

## 4. Tests

- Exact IR tests (`expect: ir`, before passes) for the UBSan checks in
  recover, trap, abort and mixed modes, at `-O0` and `-O2`, under C++17,
  C++20 and `-fwrapv`, and for the TSan and HWASan function attributes
  (`tests/conformance/driver.sanitize/`). They hold under both oracles.
- ASan and MSan cannot be exact IR tests: under them clang keeps local
  value names (and clang 22 adds alloca `!annotation`s), and kiln does
  not name values. A driver integration test
  (`crates/kiln_driver/tests/codegen.rs`,
  `sanitizer_frontend_attributes`) checks their attributes, global
  markers and lifetime markers instead.
- Driver unit tests for the grammar and its diagnostics.
- A trap-mode run test (no runtime). Recover-mode run tests need
  compiler-rt and a `requires: sanitizer-runtime` harness capability;
  that is a follow-up (§5).

## 5. Not covered

These are listed with BACKLOG rows in spec/landed/codegen/WP-675.md
and spec/landed/codegen/WP-871.md:

- UB checks: vptr, function, nullability-*, local-bounds, and the
  other gaps listed in §1.5. WP-871 emits the rest.
- Local value names under ASan/MSan (clang keeps them for the stack
  frame descriptors), and clang 22's alloca annotations there.
- `-ftrapv` and `-fsanitize-merge=`.
- The `requires: sanitizer-runtime` harness capability and
  recover-mode run tests.
- Ignorelist contents. Files are checked for existence but not applied.
- The memory sanitizer's use-after-dtor callbacks.
- dataflow, cfi, kcfi, safe-stack, shadow-call-stack, scudo, memtag,
  numerical and the kernel variants. These are rejected with `kiln does
  not support '-fsanitize=<x>'`.
