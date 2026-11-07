# LLVM Support and ADT with kiln++ (WP-730)

LLVM **19.1.7** (tag `llvmorg-19.1.7`, `llvm/` only) built with kiln++ as
`CMAKE_CXX_COMPILER` through LLVM's own CMake build. The build covers
lib/Demangle, lib/Support, the ADT headers, LLVM's bundled googletest and
gmock, and the `SupportTests` and `ADTTests` unit-test binaries, and runs
both binaries. The control is a clang++-19 build of the same tree. Both
use the M8 harness (`scripts/oss/`, spec/landed/harness/WP-666.md). Host:
aarch64 Linux, libstdc++ 14, C++17 (the project row), `-O3 -DNDEBUG`
(LLVM's Release flags), default `-fclang-compat`. Date: 2026-11-07.

**Result: parity, except for one TU fixed on an unlanded branch.**
kiln compiles 372 of the 373 TUs, and every test that ran passes:
SupportTests 1337 passed with 11 skipped, the same as clang. The one
rejected TU, `unittests/ADT/SimpleIListTest.cpp` (cluster D), is fixed
on the WP-726 branch (`d0a8f6bfd`), which has not landed. Until it does,
`ADTTests` does not link in the kiln build. A mixed build compiled that
one TU with clang++-19 and linked it with kiln's other ADT objects; it
passed all 1576 tests, as the control does.

kiln needed 11 fixes in this WP. 10 were for rejected valid code or link
failures. The 11th was a wrong-output bug: a `__PRETTY_FUNCTION__`
spelling that `TypeNameTest.Names` checks. Two more clusters were fixed by
sibling work (C on main, D on WP-726), and a NEON codegen gap was fixed
by WP-724.

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
nice -n 10 make -k SupportTests ADTTests   # draws from the shared jobserver
./unittests/Support/SupportTests --gtest_filter=-FileSystemTest.CreateDir:FileSystemTest.RespectUmask:raw_ostreamTest.filePermissions
./unittests/ADT/ADTTests
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

| Build | TUs ok / failed | Links | SupportTests | ADTTests | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|---|
| kiln, first (base `2ca3f6647`) | 7 / 1 (365 not attempted) | 0 / 0 | not built | not built | 153 s | — | 76 MB |
| clang++-19 (control) | 373 / 0 | 3 / 3 | 1337 + 11 skipped ‡ | 1576/1576 | 228 s | 569.6 s | 410 MB (`SmallVectorTest.cpp`) |
| kiln, run 2 (cluster 1) | 107 / 43 (223 not attempted) | 0 / 0 | not built | not built | 379 s | — | 98 MB |
| kiln, run 5 (clusters 1–8) | 369 / 4 | 1 / 1 | not built | not built | 106 s | — | 347 MB |
| kiln, run 6 (with origin/main) | 372 / 1 | 2 / 2 | 1336 + 11 skipped, 1 failed (`TypeNameTest.Names`) | not built (D) | 182 s | 705.3 s | 350 MB (`SmallVectorTest.cpp`) |
| **kiln, final** (`bfcac1284`) | **372 / 1** | **2 / 2** | **1337 + 11 skipped** | not built (D); **1576/1576** mixed | 229 s | 726.3 s | **346 MB** (`SmallVectorTest.cpp`) |

‡ The control ran unfiltered: 1351 tests, 1337 passed, 11 skipped and 3
failed (the root-only permission tests above). With the filter, those
three are not run.

In the first kiln build, `lib/Demangle/ItaniumDemangle.cpp` (cluster 1)
was rejected. LLVMSupport links LLVMDemangle, so `make -k` built nothing
that depends on it. Run 5 used a wrapper that hid
`__builtin_neon_vmull_v` from `xxhash.cpp`. WP-724 landed that builtin's
codegen (`b5ef6f5fe`), so run 6 and the final build use the plain harness
wrapper.

Σ TU wall is the cost comparison: kiln takes **1.28×** clang's
summed compile time. The slowest TU is `SmallVectorTest.cpp` under both
compilers (16.2 s with kiln, 13.8 s with clang). kiln's peak RSS is
**0.84×** clang's.

## Clusters

Clusters 1–2 are from the first two builds. The rest appeared only once
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

No cluster was a bug outside kiln. Every TU that clang accepts, kiln
accepts as well once WP-726 lands.

## What remains

- Cluster D lands with WP-726. Until then `ADTTests` does not link in
  the kiln build, and this row's ADT result comes from the mixed build.
- Summed compile time is 1.28× clang's (M8 perf work).
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
- Variants not run: Debug, assertions on, C++20, libc++, IRTests and the
  rest of the LLVM unit tests. `docs/status/oss.md` is not updated here;
  `scripts/oss/oss-report.py --update docs/status/oss.md llvm_project`
  regenerates its row.
