# Kitware CMake with kiln++ (WP-811)

Kitware CMake **v4.4.5** (tag `v4.4.5`), built with kiln++ as
`CMAKE_CXX_COMPILER` through CMake's own CMake build (bootstrapped with the
system cmake 3.31), with its whole CTest suite run. The control is a
clang++-19 build of the same tree with the same flags. The bundled
third-party libraries (libuv, curl, zlib, libarchive, ...) are C and built
with clang-19 under both. Host: aarch64 Linux, libstdc++ 14, C++17 (the
project row), Release, default `-fclang-compat`. Date: 2026-11-08.

**Result: parity.** All 587 TUs compile and all 17 targets link. The test
results match the control's, except for tests that also fail under clang
or fail under both when rerun. kiln needed one fix, for a valid program
it rejected. No test failed at run time because of kiln (no miscompile).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh cmake_app --compiler clang
scripts/oss/oss-build.sh cmake_app
scripts/oss/oss-report.py --perf cmake_app
```

These run in `.oss/build/cmake_app/<kiln|clang>/`. The wrapper
`scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and `timeout 600`.

```sh
cmake --fresh -S .oss/src/cmake_app -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_USE_SYSTEM_LIBRARIES=OFF -DBUILD_TESTING=ON \
  -DBUILD_CursesDialog=OFF -DBUILD_QtDialog=OFF
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure -j 4 --timeout 300
```

The suite has 714 tests. It runs the built `cmake`, `ctest` and `cpack`
on test projects, and those projects use the system compilers (gcc). So
the suite tests the CMake that kiln built. It does not test kiln on the
test projects. `ccmake` is off because the container has no curses
headers.

## Results

- **TUs** are CMake's build-phase compiles. The 39 configure probes are
  not counted, and they give the same 37 ok / 2 rejected under both
  compilers.
- **Wall** is the `oss-build.sh` run (configure, build and test), from
  `report.json`. Most of it is the test suite.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

Each build ran once, on a host shared with other harness builds.

| Build | TUs ok / failed | Links | Tests | Wall | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (`05e088464`) | 452 / 1 (134 not attempted) | 3 / 0 | 22 / 714 | 950 s | 164 MB (`cmGeneratorTarget.cxx`) |
| clang++-19 (control) | 587 / 0 | 17 / 0 | 690 / 714 | 5916 s | 388 MB (`cmake.cxx`) |
| **kiln, final** (`1780a54c2`) | **587 / 0** | **17 / 0** | **689 / 714** | 5830 s | **165 MB** (`cmGeneratorTarget.cxx`) |

In the first kiln build, the rejected TU (`Source/cmExportCommand.cxx`)
is in `CMakeLib`. So `make -k` linked only the bootstrap tools, and 692
tests could not find `cmake`.

The failures under both compilers (22) are CPack DEB, GoogleTest,
FetchContent, try_compile/try_run, find_package-CPS, BundleUtilities,
PrecompileHeaders, CMakePresets, cmake_language and CommandLine. They
need tools or network the container does not have, or they check
root-only behaviour. RunCMake.Policy and RunCMake.Diagnostics failed only
under clang. Three tests failed only under kiln in the `-j 4` run:
RunCMake.ExcludeFromAll, RunCMake.ctest_fixtures and
RunCMake.target_sources. When the three were rerun one at a time
(`ctest -R`) in both build trees, ExcludeFromAll passed under both, and
ctest_fixtures and target_sources failed the same way under both. So
those differences come from the parallel run, not from kiln. Both logs
also show "Current working directory cannot be established" 17 times.

### Performance

The comparison covers the 580 TUs that compiled under both, using CPU
time and the peak RSS of the compiler and its backend
(`oss-report.py --perf`).

- kiln/clang-19 geomean: time **1.24×**, RSS **0.42×**.
- Total: 745 s vs 640 s. Peak TU RSS: 165 MB vs 388 MB.

| heaviest TUs (by clang-19 time) | clang-19 s | kiln s | time × | clang-19 MB | kiln MB | RSS × |
|---|---|---|---|---|---|---|
| `Source/cmGeneratorTarget.cxx` | 6.9 | 7.4 | 1.07 | 362 | 165 | 0.45 |
| `Source/cmake.cxx` | 6.1 | 6.9 | 1.14 | 388 | 147 | 0.38 |
| `Source/cmGeneratorExpressionNode.cxx` | 6.0 | 6.4 | 1.06 | 359 | 153 | 0.43 |
| `Source/cmGlobalGenerator.cxx` | 5.7 | 6.1 | 1.07 | 355 | 137 | 0.39 |
| `Source/cmLocalGenerator.cxx` | 5.6 | 6.6 | 1.18 | 328 | 140 | 0.43 |

The heavy TUs are close to parity on time. The geomean is pulled up by
the small TUs (around 1 s), where kiln takes up to 2× clang's time, for
example `cmGetFilenameComponentCommand.cxx` at 1.4 s against 0.7 s. Its
RSS stays at 0.4× clang's throughout.

## Clusters

| # | Signature (first diagnostic) | TUs | Fix | Repro |
|---|---|---|---|---|
| 1 | `target of using declaration conflicts with declaration already in scope` (`using A::Check; using ParseResult::Check;` in a class with the dependent base `A`) | 1 (`Source/cmExportCommand.cxx`) | `1780a54c2` | `tests/conformance/namespace.udecl/dependent-then-nondependent-same-name-run.cpp` |

The spec sentence is in `spec/sema.md` (using-declarations; WP-811). No
cluster was a bug outside kiln.

## Disk

The source is 180 MB and the kiln build tree is 2.1 GB. The clang build
tree, also 2.1 GB, was deleted once its numbers were recorded.

## What remains

- **Test projects under kiln**: the suite compiles its test projects with
  gcc. Running it with `CC`/`CXX` set to kiln++ would also test kiln on
  several hundred small CMake projects.
- **Variants not run**: Debug, the curses and Qt dialogs, and system
  libraries (`CMAKE_USE_SYSTEM_LIBRARIES=ON`).
- **Small-TU compile time**: the 1.2–2× time ratio on TUs of about 1 s.
