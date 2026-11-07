# folly::coro with kiln++ (WP-789)

folly **v2026.10.26.00** (tag `v2026.10.26.00`), Meta's C++ library,
with its coroutine library `folly/coro` as the subject. Built with
kiln++ as `CMAKE_CXX_COMPILER` through folly's own CMake build, against
a clang++-19 control build of the same tree, with the M8 harness
(`scripts/oss/`, spec/landed/harness/WP-666.md). Host: aarch64 Linux,
libstdc++ 14, C++20, default `-fclang-compat`. Date: 2026-11-07.

folly's CMake build defines no folly/coro tests; they exist only in its
BUCK files. The shim `scripts/oss/shims/folly-coro-tests.cmake` defines
one gtest executable for each of the 45 `folly/coro/test/*Test.cpp` and
4 `folly/coro/safe/test/*Test.cpp` files, and the aggregate target
`folly_coro_tests` that builds exactly them, with libfolly and
folly_test_support. It also applies the `[folly]` row's liburing shim.
The CTest run is restricted to their 618 cases (`-R '^coro_'`).

**Result: every TU compiles; 48 of 49 test executables link; 583 of 618
tests pass (clang: 616).** The one link failure,
`coro_safe_AsyncClosureTest`, accounts for all 33 kiln-only test
failures. Its cause, an immediate invocation in a `co_return` operand of
a coroutine template, is an open follow-up with an xfail repro. The two
tests that fail under kiln also fail under clang
(`AsyncStackMetadataTest.MetadataTraceHasNoAnnotationsWithoutScope` and
`FutureUtilTest.ToTaskInterruptOnCancelFutureWithCancellation`).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh folly_coro --compiler clang
scripts/oss/oss-build.sh folly_coro
scripts/oss/oss-report.py folly_coro
```

The `[folly_coro]` row (`scripts/oss/projects.toml`) configures with
`-DBUILD_TESTS=ON` and the shim, builds the target `folly_coro_tests`
and tests with `ctest --output-on-failure --timeout 300 -j 4 -R
'^coro_'`. The system dependencies are the same as `[folly]`
(docs/status/oss/folly.md). Each TU is compiled with folly's flags:
`-O3 -DNDEBUG -g -std=gnu++20 -Wall -Wextra`, plus folly's `-Wno-*`
list.

## Results

TUs are the harness's build-phase compiles (CMake probes excluded); wall
is the whole `oss-build.sh` run; Σ TU wall sums the wrapper's per-TU
wall times; peak TU RSS is the wrapper's `ru_maxrss`. One run each.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `8f9a969e4`) | 426 / 3 | 46 / 0 | 446/618 | 678 s | — | 220 MB |
| clang++-19 (control) | 429 / 0 | 49 / 0 | 616/618 | 723 s | 1043.7 s | 1372 MB (`CollectTest.cpp`) |
| **kiln, final** (WP-789) | **429 / 0** | **48 / 1** | **583/618** | 1030 s | 907.4 s | **408 MB** (`CollectTest.cpp`) |

kiln's summed TU time is 0.87× clang's and its peak TU RSS 0.30×. The
final run shared the host with other harness builds, which accounts for
the wall difference. In the first build, three TUs failed (so three test
executables were missing). The tests that ran included one kiln-only
failure, `BlockingWaitTest.WaitTaskInFiber`.

kiln prints 986 warnings that clang does not, and none of them is an
error. 968 of them are the `coro_await_elidable[_argument]` attribute
warnings below; the others are `disable_sanitizer_instrumentation` (12),
`-Wvla-cxx-extension` under a `-Wvla` pragma (9) and `gnu::retain` (3).
All of them have xfail repros.

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `AsyncClosureTest.cpp:320: class FOLLY_CORO_TASK_ATTRS value_or_fatal final` rejected (an explicitly given class-type template argument deduced against a base) | 1 | fixed, `69b6b4eee` | `temp.deduct.call/explicit-class-nttp-selects-base-run.cpp` |
| 2 | `CapturesTest.cpp:315: no matching constructor for initialization of 'capture<int &&>'` (a conversion function with an undeduced return type as a candidate) | 1 | fixed, `037caf4a1` | `class.conv.fct/operator-auto-of-class-template-candidate-run.cpp` |
| 3 | `Collect-inl.h:262: reference to local variable 'pipe' declared in enclosing lambda expression` (a local class's member body) | 1 | fixed, `3b2624f71` | `class.local/member-body-after-tuple-binding-run.cpp` |
| 4 | `BindAsyncClosure.h:422: non-static data member cannot be constexpr` (an in-class explicit specialization of a member variable template; then member access naming it) | 1 | fixed, `0d08f878e`, `6dd18f284` | `temp.expl.spec/in-class-member-variable-template-spec-run.cpp`, `temp.names/member-access-variable-template-run.cpp` |
| 5 | `BlockingWaitTest.WaitTaskInFiber` failed (miscompile: `co_await` of an awaiter whose `await_resume` returns `T&&`, used as a prvalue, did not load the referee) | 1 test | fixed, `b0e912d48` | `expr.await/reference-result-loaded-by-value-run.cpp` |
| 6 | `coro_safe_AsyncClosureTest`: `undefined reference to folly::bind::literals::string_literals::operator""_id<...>()` (the consteval literal operator, called in a `co_return` operand of a coroutine template, was not evaluated) | 1 link (33 tests) | open, xfail | `stmt.return.coroutine/immediate-invocation-in-template-operand-run.cpp` |
| 7 | `ViaIfAsync.h:886: warning: unknown attribute 'clang::coro_await_elidable' ignored` (and `coro_await_elidable_argument`; both claimed by `__has_cpp_attribute`) | 0 (968 warnings) | open, xfail | `dcl.attr.unknown/clang-coro-attributes-are-known.cpp` |
| 8 | `Keep.h`/`Subprocess.cpp: warning: unknown attribute 'gnu::retain'` / `'disable_sanitizer_instrumentation' ignored` (both claimed) | 0 (15 warnings) | open, xfail | `dcl.attr.unknown/claimed-retain-and-sanitizer-attributes-are-known.cpp` |
| 9 | `AsyncUDPSocket.cpp:982: warning: variable length arrays in C++ are a Clang extension` under `#pragma GCC diagnostic ignored "-Wvla"` | 0 (9 warnings) | open, xfail | `dcl.array/vla-warning-suppressed-by-wvla-pragma.cpp` |

All repro paths are under `tests/conformance/`. Cluster 4 was not in
the first build's list: it was hidden behind clusters 1-3 and showed
once they were fixed.

## What remains

- Cluster 6 (an immediate invocation in a template `co_return`
  operand). It is the only thing between kiln and clang's test count.
- Clusters 7-9: warnings only.
- Variants not run through the harness: Debug, C++23/26, the folly tests
  outside folly/coro, sanitizers.
