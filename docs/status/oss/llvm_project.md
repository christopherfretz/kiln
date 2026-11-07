# LLVM Support, ADT and IR with kiln++ (WP-730, WP-775)

LLVM **19.1.7** (tag `llvmorg-19.1.7`, `llvm/` only) built with kiln++ as
`CMAKE_CXX_COMPILER` through LLVM's own CMake build. The build covers
everything the `SupportTests`, `ADTTests` and `IRTests` unit-test binaries
link: lib/Demangle, lib/Support, lib/IR, lib/Analysis, lib/CodeGen,
lib/Transforms, lib/Passes and the rest of their dependencies, plus LLVM's
bundled googletest and gmock. It runs all three binaries. The control is
a clang++-19 build of the same tree. Both use the M8 harness
(`scripts/oss/`, spec/landed/harness/WP-666.md). Host: aarch64 Linux,
libstdc++ 14, C++17 (the project row), `-O3 -DNDEBUG` (LLVM's Release
flags), default `-fclang-compat`. Date: 2026-11-07.

**Result: parity.** kiln alone compiles all 1504 TUs and links all 4
binaries, and every test passes as it does with clang: SupportTests 1337
passed with 11 skipped, ADTTests 1576/1576, IRTests 732/732. There is no
mixed build any more: cluster D (WP-726) has landed.

WP-730 (Support and ADT) needed 11 fixes; two more clusters were fixed by
sibling work, and a NEON codegen gap by WP-724. WP-775 extended the build
to IRTests and needed 9 more: 8 for rejected valid code, 1 for a link
failure from a codegen bug (the WP-775 table below).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh llvm_project --compiler clang
scripts/oss/oss-build.sh llvm_project
scripts/oss/oss-report.py llvm_project
```

These run the following in `.oss/build/llvm_project/<kiln|clang>/`. The
wrapper `scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and
`timeout 600`.

```sh
cmake --fresh -S .oss/src/llvm_project/llvm -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DLLVM_TARGETS_TO_BUILD= -DLLVM_ENABLE_PROJECTS= -DLLVM_INCLUDE_TESTS=ON \
  -DLLVM_INCLUDE_BENCHMARKS=OFF -DLLVM_INCLUDE_EXAMPLES=OFF \
  -DLLVM_ENABLE_ZSTD=OFF -DLLVM_ENABLE_LIBXML2=OFF
nice -n 10 make -k SupportTests ADTTests IRTests   # draws from the shared jobserver
./unittests/Support/SupportTests --gtest_filter=-FileSystemTest.CreateDir:FileSystemTest.RespectUmask:raw_ostreamTest.filePermissions
./unittests/ADT/ADTTests
./unittests/IR/IRTests
```

The filter drops three permission tests that fail under both compilers
when run as root: the container runs as root, which ignores the mode
bits. The project row in `scripts/oss/projects.toml` adds this filter and
is now `verified = true`.

## Results

- **TUs** are the build-phase compiles, counted once each. `make -k`
  retries a failed TU, so a failure is logged twice; CMake probes are
  excluded.
- **Wall** is the whole `oss-build.sh` run (configure, build and test).
- **Σ TU wall** sums the wrapper's per-TU compile wall times; links are
  excluded.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

Each build ran once.

### WP-775: SupportTests, ADTTests and IRTests (1504 TUs)

Both builds are fresh `oss-build.sh` runs (`make clean` first). The kiln
build used the frozen harness binary at `b8f0572eb`.

| Build | TUs ok / failed | Links | SupportTests | ADTTests | IRTests | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|---|---|
| clang++-19 (control) | 1504 / 0 | 4 / 4 | 1337 + 11 skipped | 1576/1576 | 732/732 | 679 s | 2994.6 s | 914 MB (`PassBuilder.cpp`) |
| kiln, first (base `4ff9ed9d1`) | 149 / 1 (rest not attempted) | — | not built | not built | not built | — | — | — |
| **kiln, final** (`b8f0572eb`) | **1504 / 0** | **4 / 4** | **1337 + 11 skipped** | **1576/1576** | **732/732** | 841 s | 3735.1 s | **404 MB** (`PassBuilder.cpp`) |

The same figures for the TUs of each test binary alone:

| Subset | TUs | Σ TU wall, kiln / clang | Peak TU RSS, kiln / clang |
|---|---|---|---|
| all | 1504 | 3735.1 s / 2994.6 s (**1.25×**) | 404 MB / 914 MB (**0.44×**), both `lib/Passes/PassBuilder.cpp` |
| `unittests/ADT` | 85 | 223.4 s / 180.8 s (**1.24×**) | 355 MB / 409 MB (**0.87×**), both `SmallVectorTest.cpp` |
| `unittests/IR` | 44 | 169.0 s / 125.4 s (**1.35×**) | 258 MB / 601 MB (**0.43×**), both `PassBuilderCallbacksTest.cpp` |

No TU came near the 4 GB limit under either compiler. The slowest TU is
`lib/Transforms/Vectorize/SLPVectorizer.cpp` under both (23.1 s with
kiln, 24.1 s with clang).

### WP-730: SupportTests and ADTTests (373 TUs)

| Build | TUs ok / failed | Links | SupportTests | ADTTests | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|---|
| kiln, first (base `2ca3f6647`) | 7 / 1 (365 not attempted) | 0 / 0 | not built | not built | 153 s | — | 76 MB |
| clang++-19 (control) | 373 / 0 | 3 / 3 | 1337 + 11 skipped ‡ | 1576/1576 | 228 s | 569.6 s | 410 MB (`SmallVectorTest.cpp`) |
| kiln, run 2 (cluster 1) | 107 / 43 (223 not attempted) | 0 / 0 | not built | not built | 379 s | — | 98 MB |
| kiln, run 5 (clusters 1–8) | 369 / 4 | 1 / 1 | not built | not built | 106 s | — | 347 MB |
| kiln, run 6 (with origin/main) | 372 / 1 | 2 / 2 | 1336 + 11 skipped, 1 failed (`TypeNameTest.Names`) | not built (D) | 182 s | 705.3 s | 350 MB (`SmallVectorTest.cpp`) |
| kiln, final (`bfcac1284`) | 372 / 1 | 2 / 2 | 1337 + 11 skipped | not built (D) | 229 s | 726.3 s | 346 MB (`SmallVectorTest.cpp`) |

‡ The control ran unfiltered: 1351 tests, 1337 passed, 11 skipped and 3
failed (the root-only permission tests above). With the filter, those
three are not run.

In the first kiln build, `lib/Demangle/ItaniumDemangle.cpp` (cluster 1)
was rejected. LLVMSupport links LLVMDemangle, so `make -k` built nothing
that depends on it. Run 5 used a wrapper that hid
`__builtin_neon_vmull_v` from `xxhash.cpp`. WP-724 landed that builtin's
codegen (`b5ef6f5fe`), so run 6 and the final build use the plain harness
wrapper.

In WP-730, kiln took 1.28× clang's summed compile time, and its peak
RSS was 0.84× clang's.

## Clusters

WP-730's clusters 1–2 are from its first two builds. The rest appeared only once
those were fixed and more of the tree was built. TU counts are unique
TUs.

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `invalid application of 'sizeof' to an incomplete type 'const OperatorInfo[]'` (a static member array of unknown bound, ItaniumDemangle.h) | 1, which blocked the build | fixed, `8cac1c6e4` | `tests/conformance/temp.dep.expr/static-member-array-unknown-bound.cpp` |
| 2 | `argument to '__builtin_assume_aligned' must be a constant integer` (an enumerator alignment, Endian.h) | 39 | fixed, `bd8de33af` | `tests/conformance/ext.gnu.builtins/assume-aligned-enum-alignment.cpp` |
| 3 | `'deprecated' attribute takes one argument` (`LLVM_DEPRECATED(msg, fix)`) | 2 | fixed, `79f302311` | `tests/conformance/dcl.attr.deprecated/gnu-replacement-argument.cpp` |
| 4 | `use of overloaded operator '!=' is ambiguous` (rewritten `==` candidates in C++17) | 2 | fixed, `b4076f1b1` | `tests/conformance/over.match.oper/no-rewritten-before-cxx20.cpp` |
| 5 | `no matching function for call to 'operator new'` (an allocation function's default argument, YAMLParser.cpp) | 1 | fixed, `8b4c6c8d6` | `tests/conformance/expr.new/placement-default-arguments.cpp` |
| 6 | `no matching function for call to 'operator new'` (a global `operator new` template, AllocatorList.h) | same TU as 5 | fixed, `06b900445` | `tests/conformance/expr.new/global-operator-new-template.cpp` |
| 7 | `template argument for template template parameter must be a class template` (`Templates<Tail_...>`, gtest-type-util.h) | 2 | fixed, `9997ff8d6` | `tests/conformance/temp.local/injected-name-pack-expansion-args.cpp` |
| 8 | link: undefined `cl::opt<std::string>::~opt()` (an explicit instantiation definition did not mark the vtable used) | 2 links | fixed, `8171c6636` | `tests/conformance/temp.explicit/vtable-implicit-virtual-destructor-run.cpp` |
| A | `'type' does not refer to a value` (an alias naming a member template of a specialization, TrailingObjectsTest.cpp) | 1 | fixed, `17726c21b` | `tests/conformance/temp.alias/alias-names-member-template-of-specialization-run.cpp` |
| B | `template argument for template type parameter must be a type; did you forget 'typename'?` (`decltype(local)::iterator`, MappedIteratorTest.cpp) | 1 | fixed, `6a4f749ad` | `tests/conformance/dcl.type.decltype/template-local-nested-name-run.cpp` |
| C | `qualified member initializers is not supported yet` (`iterator_adaptor_base(It)`, IteratorTest.cpp) | 1 | fixed on main by `518b17f21` (WP-716), merged before run 6 | — |
| D | `member 'ilist_node' found in multiple base classes of different types` (SimpleIListTest.cpp) | 1 | fixed on WP-726 `d0a8f6bfd` ([temp.local]/4), not landed; not re-fixed here | that branch's test |
| E | wrong output: `TypeNameTest.Names`, `getTypeName<N1::S1>()` gave `N1::S1`, not `(anonymous namespace)::N1::S1` | 0 (1 test) | fixed, `bfcac1284` | `tests/conformance/dcl.fct.def.general/predefined-type-arg-canonical.cpp` |
| — | `code generation for __builtin_neon_vmull_v is not supported yet` (xxhash.cpp) | 1 | fixed by WP-724 `b5ef6f5fe` | — |

No cluster was a bug outside kiln. Cluster D landed with WP-726.

### WP-775 bugs (IRTests and its libraries)

Each fix is pinned by a run test compared with clang 19
(spec/landed/sema/WP-775.md). Fixes 6–8 appeared only once 1–5 let more
of the tree build; fix 9 was the one link failure. No IRTests test failed
once the binary linked.

| # | Signature (first diagnostic) | TUs | Fix | Test |
|---|---|---|---|---|
| 1 | `type 'int' cannot be used prior to '::'` (the [temp.deduct.partial]/12 probe substituted outside SFINAE, YAMLParser.cpp) | 1, which blocked LLVMSupport | `aa10884ec` | `tests/conformance/temp.func.order/p12-undeduced-probe-is-sfinae-run.cpp` |
| 2 | `no viable conversion from 'uint16_t' to 'RecordPrefix'` (`T(args)` with a non-dependent `T` was type-dependent, SimpleTypeSerializer.cpp:41) | 1 | `3c51ce368` | `tests/conformance/temp.dep.expr/functional-cast-not-type-dependent-run.cpp` |
| 3 | `kiln: not yet implemented: explicit specialization` (`template<> class P::R` through a typedef, LoopAnalysisManager.h:88, CGSCCPassManager.h:173) | 19 | `931b9a02a` | `tests/conformance/temp.expl.spec/member-class-via-typedef-run.cpp` |
| 4 | `type 'llvm::RegionNode *' cannot be used prior to '::'` (printing a specialization's name instantiated a default argument, GraphTraits.h:80) | 3 (Region*.cpp) | `e77145dff` | `tests/conformance/temp.expl.spec/after-template-id-with-written-defaults-run.cpp` |
| 5 | `redefinition of 'printStyle' with a different type` (`RegionT::` vs `Tr::RegionT::`, RegionInfoImpl.h:752) | same 3 | `ba8d190b7` | `tests/conformance/temp.dep.type/qualifier-through-member-alias-run.cpp` |
| 6 | `member access into incomplete type 'llvm::vfs::FileSystem'` (a default argument's conversion instantiated `~IntrusiveRefCntPtr` at the declaration, IntrusiveRefCntPtr.h:163) | 3 | `7c7e5a8f6` | `tests/conformance/dcl.fct.default/conversion-instantiates-only-when-used-run.cpp` |
| 7 | `'llvm::Function::isTargetIntrinsic' is not a member of class 'BasicTTIImplBase<…>'` (a qualified overload set with a static member through an unrelated implicit `this`, BasicTTIImpl.h:1557) | 1 | `a2a5e4967` | `tests/conformance/class.mfct.non.static/qualified-static-through-dependent-base-run.cpp` |
| 8 | `no matching member function for call to 'members_if'` (`o.f<K>` as a deduction argument, RDFLiveness.cpp:343) | 1 | `e2ee75f57` | `tests/conformance/temp.deduct.call/member-access-template-id-argument-run.cpp` |
| 9 | link: undefined `(anonymous namespace)::VarArgSystemZHelper::SystemZGpEndOffset` (codegen loaded a non-odr-used `static const` member through an lvalue conditional, MemorySanitizer.cpp:5921) | 1 link | `b8f0572eb` | `tests/conformance/basic.def.odr/conditional-arms-constant-not-odr-used-run.cpp` |

Fix 9 was a codegen bug that showed only at link time: clang emits each
arm of `c ? A : B` as its constant (`tryEmitAsConstant`), and kiln loaded an
`available_externally` global that an internal-linkage class never
defines. The other 8 rejected valid code.

## What remains

- Summed compile time is 1.25× clang's over the whole build, and
  1.35× over the IRTests TUs (M8 perf work).
- Gaps found while reducing the WP-775 bugs, not on this build's path:
  - `auto q = g.Is<3>;` (a member template-id naming one static
    specialization, used as a placeholder initializer) is still rejected;
    `resolve_single_template_id` handles only unresolved lookups
    (`crates/kiln_sema/src/overload/address.rs:401`).
  - Fix 7's diagnostic for a non-static winner without an object reads
    "call to non-static member function without an object argument";
    clang says "no matching member function".
- `scripts/oss/kiln-cxx:30`: `OOM_RE` also matches text in a quoted
  source snippet. YAMLParser.cpp's `operator new` rejection was logged as
  `oom` in run 3.
- Gaps found while reducing, not on this build's path:
  - A placement `operator delete` template is not matched
    (`crates/kiln_sema/src/new_delete.rs`, `matching_operator_delete`).
  - Overload resolution between template and non-template allocation
    functions does not follow clang's priority in every case
    (`crates/kiln_sema/src/overload/alloc.rs`).
  - `__PRETTY_FUNCTION__`'s class scope (`C<A, 3>::m`) still prints a
    class template specialization's arguments as written. clang prints
    them canonical (`C<(anonymous namespace)::A, 3>::m`), and a closure
    type argument prints `(lambda)` without its location
    (`crates/kiln_ast/src/dump/names.rs`, `write_scope_chain`).
  - `[[gsl::Pointer]]` and `[[gsl::Owner]]` warn as unknown attributes.
- The abseil page's gap "a global `operator new` template is not an
  allocation candidate" is fixed by cluster 6.
- Variants not run: Debug, assertions on, C++20, libc++, a non-empty
  `LLVM_TARGETS_TO_BUILD`, and the LLVM unit tests other than Support,
  ADT and IR. `docs/status/oss.md` was regenerated from the WP-775 final
  run (`scripts/oss/oss-report.py --update docs/status/oss.md
  llvm_project`).
