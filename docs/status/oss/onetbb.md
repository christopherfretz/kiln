# oneTBB with kiln++ (WP-803)

This page covers oneTBB **v2022.3.0** (tag `v2022.3.0`). It was built with
kiln++ as `CMAKE_CXX_COMPILER` through oneTBB's own CMake build and CTest
suite. The control is a clang++-19 build of the same tree, and the
harness is M8's (`scripts/oss/`, spec/landed/harness/WP-666.md, WP-803).

oneTBB is Intel's task-parallel runtime: `tbb` (the scheduler, arenas,
flow graph, concurrent containers), `tbbmalloc` (the scalable allocator)
and `tbbmalloc_proxy`. It is dense in atomics, memory orders, lock-free
structures, aligned allocation and thread-local storage. Its build adds
`-D_FORTIFY_SOURCE=2`, so glibc's clang fortify headers
(`pass_object_size`, `overloadable`, `__builtin___*_chk`) are on every
TU's path. The test suite is `-DTBB_TEST=ON`: 139 CTest tests, the
`test/tbb` unit tests and the `test/conformance` tests.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.** All 184 TUs compile, all 145 targets link and all
139 tests pass, under both compilers.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh onetbb --compiler clang
scripts/oss/oss-build.sh onetbb
scripts/oss/oss-report.py --perf onetbb
```

```sh
cmake --fresh -S .oss/src/onetbb -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DTBB_TEST=ON -DTBB_STRICT=OFF -DTBBMALLOC_BUILD=ON -DTBB_EXAMPLES=OFF
nice -n 10 make -k
ctest --output-on-failure -j 4 --timeout 300
```

`test_concurrent_map` sometimes aborts with "terminate called without an
active exception" under the clang-19 control as well. In a loop of five
runs of that test alone it failed four times. It is flaky in this
container and is not a kiln finding. It passed in both final runs below.

## Results

Time is the wrapper's CPU time (user plus system) per TU. RSS is the
TU's `ru_maxrss`, which includes the backend. The final kiln build and
the control were run at the same time, so they saw the same host load.

| Build | TUs ok / failed | Links | Tests | Wall | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (WP-803 base) | 5 / 38 (43 attempted) | 2 / 2 | 0/139 | 24.7 s | — |
| clang++-19 (control) | 184 / 0 | 145 / 145 | 139/139 | 1011 s | 915 MB (`test/tbb/test_eh_flow_graph.cpp`) |
| **kiln, final** | **184 / 0** | **145 / 145** | **139/139** | **1077 s** | **835 MB** (`test/tbb/test_eh_flow_graph.cpp`) |

Over the 179 TUs both logs time, kiln/clang has a geomean of **1.10x
time** and **0.54x RSS**. The CPU totals are 956 s and 876 s. The five
heaviest TUs (by clang time):

| TU | clang s | kiln s | time x | clang MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `test/tbb/test_concurrent_unordered_map.cpp` | 29.1 | 28.2 | 0.97 | 744 | 625 | 0.84 |
| `test/tbb/test_eh_flow_graph.cpp` | 28.7 | 34.4 | 1.20 | 915 | 835 | 0.91 |
| `test/tbb/test_concurrent_unordered_set.cpp` | 25.1 | 27.4 | 1.09 | 706 | 597 | 0.85 |
| `test/tbb/test_join_node.cpp` | 24.8 | 29.5 | 1.19 | 852 | 732 | 0.86 |
| `test/tbb/test_join_node_msg_key_matching_n_args.cpp` | 21.9 | 24.5 | 1.12 | 729 | 623 | 0.85 |

No TU is over 1.2x on RSS. 13 of the 159 TUs that take clang at least
0.5 s are over 1.2x on time (P1 below).

## Clusters

Correctness. The first build's failures, by first diagnostic. Later rows
were exposed by fixing earlier ones. No test failed or hung once its TUs
compiled, so there is no miscompile cluster.

| # | Signature | TUs | Status | Repro |
|---|---|---|---|---|
| 1a | `use of undeclared identifier '__builtin___memmove_chk'` (glibc `_FORTIFY_SOURCE` with clang) | 38 | fixed, `c125de8ba` (sema, codegen, oven) | `ext.gnu.builtins/fortify-chk-run.cpp` |
| 1b | `static declaration of 'sprintf' follows non-static declaration` (glibc's clang fortify overloads: `pass_object_size`, `overloadable`) | exposed by 1a, the same 38 | fixed, `7c8f2d81e` (sema, overload, codegen, abi) | `ext.clang.pass.object.size/pass-object-size-{run,errors}.cpp` |
| 1c | a using-declaration of a `pass_object_size` overload conflicts with it | exposed by 1b | fixed, `77d9534ff` (sema) | `namespace.udecl/using-pass-object-size-overload.cpp` |
| 2 | a template-template parameter pack expansion used as an argument | exposed by 1 | fixed on main, `432771e70`; WP-803 keeps its test | `temp.spec.partial.general/template-template-pack-expansion-arg-run.cpp` |
| 3 | `unknown argument: '-ffp-model=precise'` | exposed by 1 (oneTBB's common compile flags) | fixed, `0ebcb0421` (driver) | `crates/kiln_driver/tests/codegen.rs` |
| 4 | `#pragma weak` not applied (`tbbmalloc_proxy`) | exposed by 1 | fixed, `5e5f7b9f1` (parse, sema) | `cpp.pragma/weak-{run,syntax}.cpp` |
| 5 | `new (T().f()) U` parsed as a parenthesized type-id | exposed by 1 | fixed, `680617088` (parse) | `expr.new/placement-temporary-member-call-run.cpp` |
| 6 | a C-style cast to an inaccessible base rejected ([expr.cast]/4) | exposed by 1 | fixed on main, `70a876edf`; WP-803 keeps its test | `expr.cast/cstyle-inaccessible-base-run.cpp` |
| 7 | a base-specifier naming a member of a dependent base of the current instantiation | exposed by 1 | fixed, `ee553ac2b` (sema) | `temp.dep.type/base-member-of-dependent-base-run.cpp` |
| 8 | `no template named 'convert'` (`Value::template convert<U>(x)`, `convert` in a dependent base) | exposed by 1 (the associative-container tests, `test/common/concurrent_associative_common.h`) | fixed, `ac05d7bdb`, `6c4ade5f1` (parse, sema) | `temp.dep.type/template-kw-member-of-dependent-base-run.cpp` |
| 9 | `member initializer 'allocator_type' does not name a non-static data member or base class` (libstdc++'s `_Alloc_hider : allocator_type`; `tbb_allocator` has a static member function `allocator_type`) | 5 (exposed by 8) | fixed, `bf57835ba` (sema) | `class.base.init/typedef-bound-at-definition-run.cpp` |

The spec sentences are listed in `spec/landed/harness/WP-803.md`.

Performance (kept apart from correctness):

| # | Signature | TUs | Status |
|---|---|---|---|
| P1 | 1.20x to 1.39x clang's CPU time | 13 of 159 | filed, not profiled |

P1 has two groups. The small library TUs, such as
`src/tbbmalloc/frontend.cpp` (0.8 s vs 0.6 s, 1.39x), use under half of
clang's RSS. The template-heavy tests are the second group. The largest
of them is reproduced by:

```sh
# project onetbb, tag v2022.3.0, from .oss/build/onetbb/<compiler>/test
F="-Dconformance_concurrent_unordered_map_EXPORTS -I$KILN/.oss/src/onetbb/test \
  -I$KILN/.oss/src/onetbb/include -O3 -DNDEBUG -std=c++20 -fvisibility=hidden \
  -fvisibility-inlines-hidden -Wall -Wextra -ffp-model=precise -fPIC \
  -fstack-protector-strong -D_FORTIFY_SOURCE=2 \
  -c $KILN/.oss/src/onetbb/test/conformance/conformance_concurrent_unordered_map.cpp -o /dev/null"
kiln++ $F       # 20.6 s wall, 423 MB peak RSS
clang++-19 $F   # 16.0 s wall, 535 MB peak RSS
```

`test/tbb/test_join_node_key_matching_n_args.cpp` is similar: 25.3 s
and 577 MB under kiln, against 20.0 s and 696 MB under clang.

## What remains

- P1.
- Only simple forms of a `pass_object_size` argument fold to a constant
  size. Other forms call `llvm.objectsize`, which is what clang does at
  -O0. The fortify checks keep their meaning either way.
