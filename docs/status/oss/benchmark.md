# google/benchmark with kiln++ (WP-695)

google/benchmark **v1.9.5** (tag `v1.9.5`), built with kiln++ as
`CMAKE_CXX_COMPILER` through benchmark's own CMake build and its ctest
suite, against a clang++-19 control build of the same tree, with the M8
harness (`scripts/oss/`, spec/landed/harness/WP-666.md). With
`-DBENCHMARK_DOWNLOAD_DEPENDENCIES=ON` the configure step fetches
googletest **v1.15.2** (benchmark's `cmake/GoogleTest.cmake.in`); it is
built by the same compiler, so gtest/gmock are part of the build under
test. Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.** Every TU compiles, every executable links, and all 84
ctest tests pass under both compilers (72 benchmark and output tests,
the test binaries run with a short `--benchmark_min_time`, and 12 gtest
unit-test executables). kiln needed
one sema fix; it does not change generated code.

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh benchmark --compiler clang
scripts/oss/oss-build.sh benchmark
scripts/oss/oss-report.py benchmark
```

which runs, in `.oss/build/benchmark/<kiln|clang>/` (wrapper
`scripts/oss/kiln-cxx`: `ulimit -v 3000000`, `timeout 600`):

```sh
cmake --fresh -S .oss/src/benchmark -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DBENCHMARK_DOWNLOAD_DEPENDENCIES=ON -DBENCHMARK_ENABLE_TESTING=ON \
  -DBENCHMARK_ENABLE_WERROR=OFF -DBENCHMARK_ENABLE_LTO=OFF
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure
```

The project row in `scripts/oss/projects.toml` worked unchanged; it is now
`verified = true`. The build compiles with benchmark's own warning set
(`-Wall -Wextra -Wshadow -Wconversion -pedantic-errors -Wthread-safety …`);
neither compiler prints a warning.

## Results

TUs are the harness's build-phase compiles (benchmark library 20, tests
50, googletest 3; CMake probes excluded). Wall is the whole `oss-build.sh`
run (configure, build, test) from `report.json`; Σ TU wall sums the
wrapper's per-TU wall times; peak TU RSS is the wrapper's `ru_maxrss`
(the compiler and the backend or linker it waited for). One run each.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (base `2fac46d71`) | 72 / 1 | 48 / 0 | 83/84 (`benchmark` not built) | 38 s | — | 146 MB |
| clang++-19 (control) | 73 / 0 | 49 / 0 | 84/84 | 27 s | 65.1 s | 221 MB (`test/benchmark_gtest.cc`) |
| **kiln, final** (WP-695 fix) | **73 / 0** | **49 / 0** | **84/84** | 31 s | 92.3 s | **146 MB** (googletest's `gtest-all.cc`) |

The first control build's wall (47 s) included downloading googletest at
configure; the 27 s above is a rerun with the download present, like the
kiln runs. kiln takes 1.42× clang's summed TU time (longest TU 5.2 s vs
4.1 s) and 0.66× its peak RSS (LLVM codegen runs out of process in
llc-19). The one failed TU of the first build was `test/benchmark_test.cc`,
so the `benchmark` executable (ctest test 2) was not built.

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `too many arguments to function call, expected 1, have 2` at `BENCHMARK_CAPTURE((BM_template2_capture<void, char*>), foo, 42U)` (the macro calls `(F<A, B>)(st, args...)`), then `too few template arguments for class template 'is_same'` in the instantiation of `BM_template2_capture<void, char *>` with an empty pack | 1 | fixed, commit `70f03f3c6` (parse/sema: a parenthesized function template-id that is called stays an overload set; the call deduces the trailing pack, and the explicit-only specialization is not instantiated) | `tests/conformance/temp.arg.explicit/parenthesized-template-id-callee.cpp` |

No miscompiles: every benchmark output test (counters, reporters, user
counters, memory manager, setup/teardown, …) passes under the kiln build.
No sibling branch (WP-685's two CLI11 fixes, WP-686, WP-690, WP-691,
WP-692) had fixed this cluster.

## What remains

- `(operator()<A>)(x)` and `(N::operator+<A>)(x)` (an operator-function
  template-id in parentheses, `parse_operator_template_id` in
  crates/kiln_parse/src/expr.rs) still resolve to the explicit-only
  specialization before the call; not on benchmark's path. The fix is the
  same `template_id_is_callee` check there.
- Variants not run: Debug, C++20/26, `BENCHMARK_ENABLE_LIBPFM`,
  shared library (`BUILD_SHARED_LIBS=ON`), `-stdlib=libc++`.
