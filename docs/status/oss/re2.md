# re2 with kiln++ (WP-710)

re2 **2025-11-05** (tag `2025-11-05`, commit `927f5d5`), built with
kiln++ as `CMAKE_CXX_COMPILER` through re2's own CMake build with its
tests and benchmarks enabled (`RE2_BUILD_TESTING=ON`), and run with
CTest. re2 needs abseil, googletest and benchmark installed
(`find_package`); the harness builds and installs them with the same
compiler first (`deps` in `scripts/oss/projects.toml`,
spec/landed/harness/WP-666.md "Dependencies"), so the kiln build links a
kiln-built abseil 20260107.1, googletest v1.17.0 and benchmark v1.9.5, and
the control links clang-built ones. The control is a clang++-19 build of
the same tree. Host: aarch64 Linux, libstdc++ 14, C++17 (the project row),
default `-fclang-compat`. Date: 2026-11-07.

**Result: parity.** Every TU compiles, every target links and all 20
tests pass. kiln needed one fix (a rejected valid program).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh re2 --compiler clang
scripts/oss/oss-build.sh re2
scripts/oss/oss-report.py re2
```

Before configuring re2, `oss-build.sh` runs, once per compiler,
`oss-build.sh <dep> --compiler <c> --install .oss/<dep>-install-<c>`
for abseil (`-DABSL_BUILD_TESTING=OFF -DABSL_ENABLE_INSTALL=ON
-DABSL_PROPAGATE_CXX_STD=ON`), googletest (`-DBUILD_GMOCK=ON
-DINSTALL_GTEST=ON`) and benchmark (`-DBENCHMARK_ENABLE_TESTING=OFF
-DBENCHMARK_ENABLE_INSTALL=ON -DBENCHMARK_ENABLE_WERROR=OFF
-DBENCHMARK_ENABLE_LTO=OFF`), each in `.oss/build/<dep>/<c>-dep/`
with `cmake --install` at the end. Then, in `.oss/build/re2/<kiln|clang>/`
(the wrapper `scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and
`timeout 600`):

```sh
cmake --fresh -S .oss/src/re2 -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release -DRE2_BUILD_TESTING=ON \
  "-DCMAKE_PREFIX_PATH=.oss/abseil-install-<c>;.oss/googletest-install-<c>;.oss/benchmark-install-<c>"
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure             # 20 tests
```

The project row had `verified = false` and a note asking for abseil to
be installed by hand; with `deps` it ran end to end and is now
`verified = true`.

## Results

- **TUs** are re2's build-phase compiles (51, CMake probes and the
  dependencies' TUs excluded).
- **Wall** is the `oss-build.sh` run of re2 (configure, build and test,
  dependencies excluded), from `report.json`. About 296 s of it is
  CTest, dominated by the exhaustive tests (two of 115 s each), so it is
  no compile-time measure.
- **Σ TU wall** sums the wrapper's per-TU compile wall times.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

Each build ran once, on a host shared with other harness builds.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (`fcb3fd82d` + harness deps) | 21 / 1 (29 not attempted) | 0 / 0 | 0/20 | 4 s | — | 84 MB |
| clang++-19 (control) | 51 / 0 | 21 / 0 | 20/20 | 314 s | 85.9 s | 229 MB (`re2_test.cc`) |
| **kiln, final** (`b5a6a869f`) | **51 / 0** | **21 / 0** | **20/20** | 308 s | 78.5 s | **159 MB** (`re2_test.cc`) |

In the first kiln build the rejected TU (`prog.cc`) is in the `re2`
library itself, so `make -k` built nothing that links it. The
dependencies built with kiln at parity with their clang builds (TUs ok /
failed, kiln and clang alike): abseil 156 / 0, googletest 4 / 0,
benchmark 20 / 0.

kiln's summed compile time is **0.91×** clang's and its peak RSS
**0.69×** clang's; the slowest TU (`re2_test.cc`) takes 5.1 s under
both.

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `member is not a field (M3)` (`prog.cc`: `std::sort(b, e, sorted.less)`, a static member function of `SparseArray<int>` named with `.` and not called) | 1, the `re2` library, which blocked every link | fixed, `b5a6a869f` | `tests/conformance/expr.ref/p6-static-member-function-value-run.cpp` |

No cluster was a bug outside kiln, and no test failed at run time once
everything built (no miscompile). None of the open sibling branches
(wp/703, wp/704, wp/706, wp/709) touches this code path.

## What remains

- Variants not run: Debug, C++20/26, libc++, `BUILD_SHARED_LIBS=ON`,
  `RE2_USE_ICU`, the benchmark binary (`regexp_benchmark` builds and
  links; ctest does not run it), and the Python binding.
