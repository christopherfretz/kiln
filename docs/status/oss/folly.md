# folly with kiln++ (WP-726)

folly **v2026.10.26.00** (tag `v2026.10.26.00`), built with kiln++ as
`CMAKE_CXX_COMPILER` through folly's own CMake build with its tests
enabled (`BUILD_TESTS=ON`), and its CTest suite run minus three
exclusions. The control is a clang++-19 build of the same tree with the
same flags and the same test filter. Host: aarch64 Linux, libstdc++ 14,
boost 1.88, fmt 10, C++20 (the project row), Release, default
`-fclang-compat`. Date: 2026-11-07.

**Result: 725 of 726 TUs compile (clang 19: 724), 328 of 331 links succeed and 3767 of 3773 tests pass (clang: 3773). The 6 failing tests are `memcpy_test`, whose binary does not link: kiln has no `[[gnu::ifunc]]` / `alias` codegen. The one rejected TU, `RangeTest.cpp`, is rejected by clang 19 too.**

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh folly --compiler clang
scripts/oss/oss-build.sh folly
scripts/oss/oss-report.py folly
```

In `.oss/build/folly/<kiln|clang>/` (the wrapper `scripts/oss/kiln-cxx`
applies `ulimit -v 3000000` and `timeout 600`):

```sh
cmake --fresh -S .oss/src/folly -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTS=ON \
  -DCMAKE_PROJECT_folly_INCLUDE=scripts/oss/shims/folly-no-liburing.cmake
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure --timeout 300 \
  -E 'WriteFileAtomic.directoryPermissions|^range_test[.]|^container_map_util_test[.]'
```

folly needs boost, double-conversion, gflags, glog, libevent, fmt and
fast_float installed; googletest 1.17 is fetched at configure time.
folly's CMake skips its own slow, broken and hanging tests unless asked
(19 binaries, listed in `configure.log`); they stay skipped.

**liburing.** folly compiles its io_uring code whenever `<liburing.h>`
exists (`folly/io/async/Liburing.h` tests `__has_include(<liburing.h>)`,
not the CMake `LibUring` result). That code uses the io_uring zero-copy
receive API of liburing 2.10 (`IORING_REGISTER_QUERY`,
`struct io_uring_zcrx_*`), and Debian 13's liburing-dev is 2.9. So
`IoUringZeroCopyBufferPool.cpp` fails under every compiler, the
monolithic `libfolly` does not link and no test builds. Every folly tag
back to v2025.10.06.00 uses zcrx. The shim
`scripts/oss/shims/folly-no-liburing.cmake` (included after
`project(folly)`) puts a replacement `folly/io/async/Liburing.h` saying
`FOLLY_HAS_LIBURING 0` first on the include path, for both compilers, so
folly builds as on a host without liburing.

### Test filter

folly registers every gtest case as a CTest test (3862). Three are
excluded, for both compilers:

- `WriteFileAtomic.directoryPermissions` expects a permission error that
  root (the container user) does not get. It fails under clang.
- `range_test` (`folly/test/RangeTest.cpp`) and `container_map_util_test`
  (`folly/container/test/MapUtilTest.cpp`) do not compile under clang 19,
  so the control has no executable for their cases. kiln rejects
  `RangeTest.cpp` with the same first error as clang ("no matching
  function for call to 'format'"). kiln (like GCC 14) accepts
  `MapUtilTest.cpp`; see what remains.

That leaves 3773 tests. The test phase takes about 4 to 5 minutes (clang 221 s, kiln 272 s).

## Results

- **TUs** are folly's build-phase compiles (CMake probes excluded),
  googletest's included.
- **Wall** is the `oss-build.sh` run (configure, build and test), from
  `report.json`. The intermediate kiln builds ran their tests against
  binaries that did not exist (no test passed in any of them).
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

Each build ran once, on a host shared with other harness builds.

| Build | TUs ok / failed | Links | Tests (ctest -E) | Wall | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (`504e3ad96`) | 0 / 57 (the rest not attempted) | 0 / 0 | 0 | 197 s | 87 MB |
| kiln, interim (`98cccac46`) | 365 / 4 | 1 / 1 | 0 | 174 s | 181 MB |
| kiln, interim (`11810373f`) | 719 / 7 | 2 / 325 | 0 | 619 s | 666 MB (`container/test/F14MapTest.cpp`) |
| clang++-19 (control) | 724 / 2 | 330 / 0 | 3773/3773 | 856 s | 2202 MB (`algorithm/simd/test/FindFixedTest.cpp`) |
| **kiln, final** (`88d2fffb8`) | **725 / 1** | **328 / 3** | **3767/3773** | 748 s | **686 MB (`container/test/F14MapTest.cpp`)** |

In the first kiln build every TU failed on the unknown
`-finput-charset=UTF-8` flag folly passes, so `make -k` stopped after
the first target of each directory. In the `11810373f` build, 323 links
failed on undefined `std::_Vector_base<…>::_Vector_impl` constructors
and `std::allocator` destructors (cluster 22).

**TUs over 2 GiB RSS:** under clang, `algorithm/simd/test/FindFixedTest.cpp`
(2202 MB) and `container/test/F14MapTest.cpp` (2179 MB). None under kiln: its peak is 686 MB, and its slowest TU is `F14MapTest.cpp` at 57 s.

## Clusters

Found by the builds and by `-fsyntax-only` runs of the remaining TUs
between builds. TUs counts the TUs whose first error it was, in the build
that found it.

| # | Signature (first diagnostic) | TUs | Fix | Repro (`tests/conformance/` unless noted) |
|---|---|---|---|---|
| 1 | `unknown argument: '-finput-charset=UTF-8'` | all | `dffabf9c5` | `crates/kiln_driver/tests/driver.rs` |
| 2 | `'formatter<…>' does not refer into a class` (fmt 10 `formatter<T, Char, enable_if_t<…>>::format` out of line) | ≥1 | `8542aec42` | `temp.spec.partial.member/out-of-line-member-dependent-name-arg.cpp` |
| 3 | `use of undeclared identifier '__builtin_memcpy_inline'` / `'__builtin_assume_separate_storage'` | ≥1 | `3e62ba880` | `ext.gnu.builtins/memcpy-inline-{args,run}.cpp`, `tests/codegen/builtin_memcpy_inline.cpp` |
| 4 | `.*` through a base member pointer on an uninstantiated specialization (`Memory.h` weak_ptr accessor) | ≥1 | `9939722b8` | `expr.mptr.oper/lhs-{incomplete,instantiates-derived}.cpp` |
| 5 | ambiguous conversion for `Q::m` with two subobjects of `m`'s class (boost multi_index, `TimeoutQueue`) | ≥1 | `d0d5da36d` | `class.member.lookup/p8-qualifier-selects-subobject.cpp`, `tests/codegen/qualified_member_subobject.cpp` |
| 6 | injected-class-name `tramp<1>` found in several bases taken as ambiguous | ≥1 | `d0a8f6bfd` | `temp.local/p4-injected-names-same-template.cpp` |
| 7 | constexpr defaulted special member used in a template's constinit initializer not defined (`FastStaticBool`) | ≥1 | `46480a082` | `dcl.constexpr/defaulted-special-in-template.cpp` |
| 8 | `constinit` lost on an instantiated static member definition | ≥1 | `3ef004a92` | `dcl.constinit/static-member-template-definition.cpp` |
| 9 | `using T::C` through a typedef qualifier (`relaxed_atomic<bool>`) | ≥1 | `5e6932a80` | `namespace.udecl/inheriting-via-typedef-qualifier.cpp` |
| 10 | googletest `Templates<Tail_...>` expansion reported as a non-template argument | 1 (`gtest-all.cc`) | `8be985bdc` | `temp.variadic/template-template-expansion-own-template.cpp` |
| 11 | overload set to a class type (`dynamic::object` default arguments) | ≥1 | `9d8b9fd85` | `over.over/class-target-{converting-ctor,no-constructor}.cpp` |
| 12 | `'this' cannot be implicitly captured in this context` | 1 (`CancellationToken.cpp`) | `363a0753e` | `expr.prim.lambda.capture/implicit-this-{instance,static}-overload-winner.cpp` |
| 13 | `'operator unsigned int' is a private member of 'relaxed_atomic_base<…>'` | 3 (`SharedMutex.cpp`, …) | `54514a440` | `namespace.udecl/conversion-function-via-using{,-access}.cpp` |
| 14 | `call to 'copy_table' is ambiguous` | 1 (`Fingerprint.cpp`) | `007d09144` | `temp.func.order/p3-pack-expansion-other-index.cpp` |
| 15 | `extraneous template parameter list` (`Format-inl.h:300`) | 1 (`Format.cpp`) | `a50d25c72` | `temp.spec.partial.member/out-of-line-member-pack-arguments.cpp` |
| 16 | `use of undeclared identifier '__builtin_unpredictable'` | 1 (`stats/TDigest.cpp`) | `1cfb64e98` | `ext.gnu.builtins/unpredictable.cpp` |
| 17 | `no member named 'get' in 'QuotientMultiSet::Block'`; `no type named 'iterator' in 'EvictingCacheMap::NodeList'` | 2 | `063b97f7b` | `temp.dep.type/incomplete-member-class-qualifier{,-missing}.cpp` |
| 18 | `call to 'to_pointer' is ambiguous` (fmt 10) | 1 (`json/json.cpp`) | `6d3fd0e05` | `temp.func.order/p12-undeduced-used-in-non-deduced-context.cpp` |
| 19 | `non-type template argument is not a constant expression` (`&C::m` while `C` is defined) | 1 (`TimeoutManager.cpp`) | `98cccac46` | `temp.arg.nontype/member-pointer-of-class-being-defined.cpp` |
| 20 | `template parameter redefines default argument` (`ViaIfAsync.h:892`, hidden friend `operator co_await`) | 2 | `a27b4e28e` | `temp.friend/p9-constraint-on-enclosing-distinct.cpp` |
| 21 | **miscompile**: `opt: input module is broken` (reference bound to a `co_await` result) | 2 (`fibers/Semaphore*.cpp`) | `8df3c5aa9` | `expr.await/reference-result-bound-to-reference.cpp` |
| 22 | link: undefined `_Vector_impl()` / `~allocator()` (members of a defaulted special defined from a template) | 323 links | `bf9193ff2` | `dcl.constexpr/defaulted-special-in-template-odr-uses-members.cpp` |
| 23 | `variable of non-literal type 'const AssertionResult' cannot be defined in a constexpr function` (if condition) | 1 | `34bec5073` | `dcl.constexpr/p3-if-condition-variable-not-checked.cpp` |
| 24 | `'fine_clock' does not refer to a value` | 1 (`ChronoTest.cpp`) | `9ea44be9c` | `temp.expl.spec/member-typedef-through-specialization-in-template-arg.cpp` |
| 25 | variable template `auto&` / `const auto*` "depends on itself" | 1 (`ConstexprMathTest.cpp`) | `76ef7269e` | `dcl.spec.auto/variable-template-reference-placeholder.cpp` |
| 26 | `'writeChain' is not virtual and cannot be declared pure` (dependent base) | 1 | `56d351ed8` | `class.abstract/pure-{override-of-dependent-base,not-virtual-at-instantiation}.cpp` |
| 27 | `static assertion failed … is_destructible<neon_vector_type …>` | 1 (`MemcpyBenchmark.cpp`) | `934755876` | `expr.prim.id.dtor/pseudo-destructor-vector-type-run.cpp` |
| 28 | IR lowering: `extractelement on a non-vector` (`v[i][j]` on `std::vector<V>`) | 1 (same TU, after 27) | `6f8febe85` | `ext.vector/subscript-of-reference-returning-call-run.cpp` |
| 29 | `call to 'iterator_arrow_result' is ambiguous` (boost `reverse_iterator`, `(f)(t)` used ADL) | 1 (`EvictingCacheMapTest.cpp`) | `e9596e8f6` | `basic.lookup.argdep/parenthesized-function-template-no-adl.cpp` |
| 30 | link: undefined `__atomic_is_lock_free` (folded only for a null pointer, not an aligned integer object pointer) | 1 link | `6ecc52a6a` | `ext.c11.atomic/is-lock-free-integer-pointer-run.cpp` |
| 31 | **miscompile**: ~790 test crashes, `size > base` assertion in `atomic_grow_array` (`decltype(__builtin_operator_new(…))` signature stayed dependent; under-aligned `new`) | run time | `88d2fffb8` | `dcl.type.decltype/builtin-call-return-type-run.cpp` |
| — | `no matching function for call to 'format'` (`RangeTest.cpp`) | 1 | clang 19 rejects it too; excluded | — |

The spec sentences are in `spec/landed/sema/WP-726.md`. Cluster 9 is
the same change as WP-724's `b8889a8fc` (rocksdb cluster 5). Clusters 21
and 28 were invalid IR, caught before run time; cluster 31 was a run-time miscompile. A builtin first named inside a template was declared in a dependent context, so the signature `decltype(__builtin_operator_new(...))` of folly's `operator_new` invoker stayed dependent, and the aligned `new` it chose returned under-aligned storage. About 790 tests crashed (hazard pointers, thread locals, futures, `fbstring`), many on the `size > base` assertion in `atomic_grow_array`.

## What remains

- **liburing**: folly's io_uring backends are not built (shim above).
  Building them needs liburing 2.10 or newer on the host.
- **Tests not run**: the 19 slow, broken and hanging binaries folly's
  CMake skips by default, and `range_test` / `container_map_util_test`,
  which clang 19 does not compile.
- **Variants not run**: Debug, `-O0`, shared libraries, the benchmarks
  (`BUILD_BENCHMARKS`), jemalloc, libiberty, the extended random
  (`FOLLY_HAVE_EXTRANDOM_SFMT19937`).
- **`__builtin_unpredictable`** is lowered as its operand: clang's
  `!unpredictable` branch hint is dropped
  (`crates/kiln_codegen/src/builtins.rs:1101`).
- **`[[gnu::ifunc]]` / `[[gnu::alias]]`**: not lowered, so `__folly_memcpy`
  and `__folly_memset` (`folly/memcpy_select_aarch64.cpp:133`) are
  undefined and `memcpy_test`, `memcpy_benchmark` and `memset_benchmark`
  do not link (the 6 failing tests).
- **`MapUtilTest.cpp`**: clang 19 rejects it ("type 'const std::map<int,
  int> *' cannot be used prior to '::'", from `NestedMapType` instantiated
  while substituting `get_default`, `folly/container/MapUtil.h:337`); kiln
  treats the error in the member typedef as a substitution failure.
  `spec/sema-templates.md` Q12 says follow clang 19.
- Narrow fixes worth generalizing: `crates/kiln_sema/src/conv/mod.rs:507`,
  `crates/kiln_sema/src/templ/order.rs:691`,
  `crates/kiln_sema/src/templ/member.rs:251`,
  `crates/kiln_sema/src/overload/ics.rs:1622`,
  `crates/kiln_sema/src/overload/member.rs:187`.
- **Partial ordering** (`crates/kiln_sema/src/templ/order.rs:691`): with
  a pack in the other template, or an undeduced non-type or template
  parameter, undeduced parameters count as unused, a narrower rule than
  clang's [temp.deduct.partial]/12 handling.
