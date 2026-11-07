# googletest with kiln++ (WP-682)

googletest **v1.17.0** (tag `v1.17.0`) was built with kiln++ as
`CMAKE_CXX_COMPILER`, using googletest's own CMake build (gtest and gmock
with their tests) and its CTest suite. The control is a clang++-19 build
of the same tree. Host: aarch64 Linux, libstdc++ 14, C++20. Date:
2026-11-07.

**Result: parity.** All 85 TUs compile, all 67 links succeed and 63/63
tests pass. Neither compiler emits a warning. kiln needed four fixes:

- the driver: bare `-W`, and a silent exit 70
- template argument deduction
- `-Winconsistent-missing-override`
- the `disable_tail_calls` attribute

The sanitizer-attribute warnings were fixed by WP-667.

## Commands

From a kiln checkout, after `source scripts/env.sh`, build the harness
compiler and run the WP-666 harness (spec/landed/harness/WP-666.md):

```sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh googletest --compiler clang   # control build
scripts/oss/oss-build.sh googletest                    # kiln build
scripts/oss/oss-report.py --update docs/status/oss.md googletest
```

`oss-build.sh` clones `https://github.com/google/googletest` at
`v1.17.0` into `.oss/src/googletest`. It then runs these commands in
`.oss/build/googletest/<kiln|clang>`; `kiln-cxx` is the logging wrapper,
which applies `ulimit -v 3000000` and `timeout 600` to each compile:

```sh
nice -n 10 cmake --fresh -S .oss/src/googletest -B <build> -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -Dgtest_build_tests=ON -Dgmock_build_tests=ON -DPython3_EXECUTABLE=/usr/bin/python3
nice -n 10 make -k -C <build>          # jobs from the shared jobserver
cd <build> && timeout 1800 nice -n 10 ctest --output-on-failure
```

## Results

Wall time is the `/usr/bin/time -v` elapsed time of the whole
`oss-build.sh` run (configure, build and tests). Peak TU RSS is the
largest single compile. All figures are from one run each, on a shared
host.

| Build | TUs ok / failed | Links ok / failed | Tests | Warnings | Wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `7c5998af2`) | 15 / 1 | 7 / 0 | 2 / 63 | — | 32 s | 670 MiB (`gtest_all_test.cc`) |
| kiln, after clusters 1–2 | 85 / 0 | 67 / 0 | 63 / 63 | 117 | 59 s | 668 MiB |
| **kiln, final** (with WP-667 merged) | **85 / 0** | **67 / 0** | **63 / 63** | **0** | 53 s | **668 MiB** (`gtest_all_test.cc`) |
| **clang++-19** | **85 / 0** | **67 / 0** | **63 / 63** | **0** | 100 s | **794 MiB** (`gmock-matchers-containers_test.cc`) |

The first kiln build rejected `gtest-all.cc` (cluster 1), so `libgtest`
never built. `make -k` skipped every target that depends on it, which
is why only 16 TUs were attempted. The slowest single TU takes 28.4 s
under kiln and 28.6 s under clang. On this build, kiln's peak TU RSS is
0.84× clang's.

## Clusters

| # | Signature | Size | Status | Repro / test |
|---|---|---|---|---|
| 1 | `kiln++: error: unknown argument: '-W'` (from `cxx_strict_flags`, `googletest/cmake/internal_utils.cmake:97`) | 1 TU (`gtest-all.cc`); blocked 69 TUs and 61 tests | fixed in `3bd119e7d` | `tests/conformance/dcl.fct/unused-but-set-parameter-bare-w.cpp` |
| 1b | a silent exit 70 when the 1 GiB compile-thread stack cannot be reserved under the harness's `ulimit -v` | no TU in these runs (filed as WP-670 with cluster 1) | fixed in `3bd119e7d` (an error line; `kiln-cxx` files it as `oom`) | `crates/kiln_driver/tests/driver.rs` `compile_thread_spawn_failure_is_diagnosed` |
| 2 | `tuple:2452: no matching function for call to '__get_helper'` ("could not match '_Tuple_impl' against 'std::tuple'") | 1 TU (`gmock-actions_test.cc`) | fixed in `4a1c15231` | `tests/conformance/temp.deduct.call/libstdcxx-get-uninstantiated-tuple.cpp`, `derived-base-instantiates-argument-class.cpp` |
| 3 | `-Winconsistent-missing-override` on `MOCK_METHOD` members (gmock under `-isystem`) | 37 warnings | fixed in `06e0a5406` | `tests/conformance/class.virtual/inconsistent-missing-override-system-macro.cpp` |
| 4 | `unknown attribute 'disable_tail_calls' ignored` (`GTEST_NO_TAIL_CALL_`; `__has_attribute` already returned 1) | 14 warnings | fixed in `73e7176b2` | `tests/conformance/dcl.attr/gnu-disable-tail-calls.cpp`, `tests/codegen/disable_tail_calls_o2.cpp` |
| 5 | `unknown attribute 'no_sanitize_{address,memory,thread}' ignored` | 66 warnings | fixed by WP-667 (`e20c5e5ac`), merged from main | `tests/conformance/dcl.attr.grammar/sanitizer-lifetimebound-attributes.cpp` |

Notes on the clusters:

- **Cluster 2.** Before deducing a template-id P against A's bases, A
  must be instantiated ([temp.deduct.call]/4.3, [temp.inst]/2). An
  unqualified call worked because ADL had already instantiated A. The
  qualified `std::__get_helper<I>(t)` inside libstdc++'s `std::get` did
  not.
- **Cluster 3.** clang follows a macro-argument name to the parameter
  token in the macro body and skips the warning when that token is in a
  system header.
- **Cluster 4.** kiln now implements `disable_tail_calls` instead of
  ignoring it. The function's declaration and definition carry
  `"disable-tail-calls"="true"`, and the attribute is mutually exclusive
  with `naked`.
- **Not kiln bugs:** none found.

## What remains

- Nothing is needed for parity in this configuration: Release, C++20,
  libstdc++.
- Not tried:
  - a Debug or C++26 build
  - `-stdlib=libc++`
  - googletest's optional Python-driven tests beyond the CTest default set
