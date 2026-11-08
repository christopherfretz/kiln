# Design note: sanitizers (`-fsanitize=`)

Status: WP-675. The contract lives in spec/driver.md (§3, sanitizer
options), spec/codegen.md (sanitizer instrumentation) and spec/sema.md
(the `no_sanitize` attribute family). The landed summary, with the list
of checks not yet done, is spec/landed/codegen/WP-675.md.

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
- If bounds was removed, `nosanitize_bounds` is added. kiln does not
  emit bounds checks yet (§5), so this does not arise.

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
`-a`). For `return` it is the closing brace of the function, and for
`__builtin_unreachable` it is the call.

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

- Exact IR tests against clang 19 for UBSan checks in all three modes,
  and for the function attributes and global metadata, both before
  passes.
- Driver unit tests for the grammar and its diagnostics, and strict-diag
  corpus tests for the error wording.
- Run tests: a run test that needs compiler-rt says `//@ requires:
  sanitizer-runtime`. The harness probes for it once, at startup, by
  linking an empty `-fsanitize=address,undefined` program with the
  oracle clang. Machines without the runtime skip these tests instead of
  failing. CI installs `libclang-rt-19-dev` (and the 22 package for the
  clang 22 leg), so CI does run them. Trap-mode run tests need no
  runtime and are always on.

## 5. Not covered by WP-675

These are listed with BACKLOG rows in spec/landed/codegen/WP-675.md:

- UB checks: null, alignment and object-size (type_mismatch), array
  bounds, bool and enum loads, float-cast-overflow, pointer-overflow,
  vptr, function, nonnull-attribute, returns-nonnull-attribute, builtin,
  and the unsigned and implicit-conversion `integer` checks.
- Ignorelist contents. Files are checked for existence but not applied.
- The memory sanitizer's use-after-dtor callbacks.
- dataflow, cfi, kcfi, safe-stack, shadow-call-stack, scudo, memtag,
  numerical and the kernel variants. These are rejected with `kiln does
  not support '-fsanitize=<x>'`.
