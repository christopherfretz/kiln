# stdexec with kiln++ (WP-789)

stdexec **nvhpc-26.05** (tag `nvhpc-26.05`, `6d7ad68`), NVIDIA's P2300
`std::execution` reference implementation, built with kiln++ as
`CMAKE_CXX_COMPILER` through its own CMake build (tests and examples on,
its defaults) and its CTest suite (907 Catch2 3.14 cases, fetched by
rapids-cmake/CPM at configure time), against a clang++-19 control build
of the same tree, with the M8 harness (`scripts/oss/`,
spec/landed/harness/WP-666.md). Host: aarch64 Linux, libstdc++ 14,
C++20, default `-fclang-compat`. Date: 2026-11-07.

**Result: parity.** Every TU compiles, every executable links and all
907 tests pass under both compilers, including the `*_fail` tests that
match kiln's error text. The first build compiled 112 of 231 TUs, and
none of its 7 tests passed.

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh stdexec --compiler clang
scripts/oss/oss-build.sh stdexec
scripts/oss/oss-report.py stdexec
```

The `[stdexec]` row (`scripts/oss/projects.toml`) configures with no
extra options and tests with `ctest --output-on-failure --timeout 300
-j 4`. Each TU is compiled with `-O3 -DNDEBUG -std=gnu++20
-Wno-gnu-line-marker -Wall -Werror=unused-parameter`. stdexec's build-failure tests (`*_fail`
TUs) are ctest cases that compile a TU and match the compiler's error
text against the `// build error:` regex in the source; they count as
tests, not as TU failures.

## Results

TUs are the harness's build-phase compiles (CMake probes excluded); wall
is the whole `oss-build.sh` run; Σ TU wall sums the wrapper's per-TU
wall times; peak TU RSS is the wrapper's `ru_maxrss`. One run each.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `8f9a969e4`) | 112 / 119 | 1 / 0 | 0/7 | 196 s | — | 209 MB |
| clang++-19 (control) | 231 / 0 | 20 / 0 | 907/907 | 209 s | 602.6 s | 1064 MB (`test_merge_each_threaded.cpp`) |
| **kiln, final** (WP-789) | **231 / 0** | **20 / 0** | **907/907** | 69 s | 575.7 s | **541 MB** (`test_merge_each_threaded.cpp`) |

kiln's summed TU time is 0.96× clang's and its peak TU RSS 0.51×. The
final run's wall (60 s build, 6 s ctest) is from a quiet host; the
control run shared the host with other harness builds, so only the Σ TU
wall is comparable. The kiln build prints no warning, the same as
clang.

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `__typeinfo.hpp:256: function '__typeid_lookup' with deduced return type cannot be used before it is defined` (a friend defined in another specialization) | 117 | fixed, `cd8a0b00a` | `temp.friend/deduced-friend-defined-by-other-specialization.cpp` |
| 2 | `intrusive_heap.hpp:82: class template partial specialization does not specialize any template argument` and `implicit instantiation of undefined template '__intrusive_mpsc_queue<&test_node::next_>'` (partial specializations over an `auto` primary) | 2 | fixed, `05ad3b78d` | `temp.spec.partial.general/auto-primary-*.cpp` |
| 3 | `__manual_lifetime.hpp:63: invalid use of non-static data member '__buffer_'` (a typed declarator taken for a deduction guide) | 106 | fixed, `64be77a24` | `temp.deduct.guide/typed-function-named-like-class-template-run.cpp` |
| 4 | `__let.hpp:287` (a name not found in a current instantiation with a dependent base) | 6 | fixed, `fe75c597b` | `temp.dep.type/current-instantiation-qualifier-unknown-member-alias-run.cpp` |
| 5 | `__any.hpp:576: template argument for template template parameter must be a class template or type alias template`, `__any_allocator.hpp:71` | 4 | fixed, `432771e70`, `07adfd56d` | `temp.variadic/template-template-pack-expansion-partial-spec-run.cpp`, `temp.local/injected-class-name-concept-template-template-arg-run.cpp` |
| 6 | `__any_static_cast_t` (`test_cpo_connect_awaitable`: base deduction after a template template mismatch) | 1 | fixed, `5b84e8dfd` | `temp.deduct.call/base-after-template-template-mismatch-run.cpp` |
| 7 | `any_sender_of.hpp:715` (a member function template taken as an override) | 3 | fixed, `6b9fb1203` | `temp.mem/member-template-does-not-override-run.cpp` |
| 8 | `unknown attribute '__artificial__' ignored [-Werror]` | 1 | fixed, `d4c170598` | `dcl.attr/gnu-artificial.cpp` |
| 9 | `test_just.cpp:207: reference to an unemitted local variable` (a local class member in a generic lambda) | 1 | fixed, `7cfb60df4` | `class.local/generic-lambda-local-class-member-then-local-run.cpp` |
| 10 | `__tuple.hpp:74: '__value' following the 'template' keyword does not refer to a template` (`e.template B<T>::m`) | 2 | fixed, `c4d346e15` | `temp.names/template-kw-before-member-qualifier-run.cpp` |
| 11 | `test_repeat_n_fail`: the `// build error:` regex did not match (a failed `static_assert` printed its concept-id requirement as `...`) | 1 test | fixed, `e0b920269` | `dcl.pre/static-assert-concept-id-requirement.cpp` |
| 12 | `test_merge_each.cpp:76`, `merge_each.hpp:1153: deduced type ... does not satisfy 'next_sender'` (a placeholder's type-constraint without the concept's default argument) | 2 | fixed, `fcbdcf055` | `dcl.spec.auto/type-constraint-uses-concept-default-args.cpp` |
| 13 | `test task with allocator`: `CHECK( bytes > 0 )` (miscompile: the promise's `operator new` template was not used for the frame) | 1 test | fixed, `4d193e1d9` | `dcl.fct.def.coroutine/promise_alloc_template.cpp` |

All repro paths are under `tests/conformance/`.

## What remains

- No open kiln issue remains in this project.
- Variants not run through the harness: Debug, C++23/26, the CUDA and
  TBB backends, sanitizers.
