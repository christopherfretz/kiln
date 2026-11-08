# CLI11 with kiln++ (WP-685)

CLI11 **v2.6.1** (tag `v2.6.1`, commit `bfffd37e1`), built with kiln++ as
`CMAKE_CXX_COMPILER` through CLI11's own CMake build and its CTest suite,
against a clang++-19 control build of the same tree, with the M8 harness
(`scripts/oss/`, spec/landed/harness/WP-666.md). The build covers the
library, the 86-test suite (Catch2 **v2.13.10** single header, which CMake
downloads when no Catch2 is installed; it is compiled by the compiler
under test), the examples and the fuzz-corpus test. Host: aarch64 Linux,
libstdc++ 14, default `-fclang-compat`. Date: 2026-11-07.

**Result: parity**, with libstdc++ and with libc++. Every TU compiles,
every target links and all 86 tests pass under both compilers. kiln
needed two sema fixes: one rejected-valid (2 TUs) and one miscompile
found only in the libc++ build (every binary crashed).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh cli11 --compiler clang
scripts/oss/oss-build.sh cli11
scripts/oss/oss-report.py cli11
```

which runs, in `.oss/build/cli11/<kiln|clang>/` (wrapper
`scripts/oss/kiln-cxx`: `ulimit -v 3000000`, `timeout 600`):

```sh
cmake --fresh -S .oss/src/cli11 -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DCLI11_BUILD_TESTS=ON -DCLI11_BUILD_EXAMPLES=ON \
  -DCLI11_WARNINGS_AS_ERRORS=OFF
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure             # 86 tests
```

The libc++ variant is the same with `KILN_OSS_ROOT=.oss/libcxx
CXXFLAGS=-stdlib=libc++ LDFLAGS=-stdlib=libc++` in the environment (CMake
reads both). The project row in `scripts/oss/projects.toml` worked
unchanged; it is now `verified = true`.

## Results

TUs are the harness's build-phase compiles (65; CMake probes excluded).
Wall is the whole `oss-build.sh` run (configure, build, test) from
`report.json`; Σ TU wall sums the wrapper's per-TU wall times (compiles
and links); peak TU RSS is the wrapper's `ru_maxrss` (the compiler and the
backend or linker it waited for). One run each.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (chain tip `65ef91c38`) | 63 / 2 | 58 / 0 (2 not attempted) | 85/86 (`FuzzFailTest` not built) | 88 s | — | 338 MB (`tests/ConfigFileTest.cpp`) |
| clang++-19 (control) | 65 / 0 | 60 / 0 | 86/86 | 29 s | 560.7 s | 443 MB (`tests/ConfigFileTest.cpp`) |
| **kiln, fix 1** (`e9bbdb3ed`) | **65 / 0** | **60 / 0** | **86/86** | 33 s | 593.6 s | **338 MB** (`tests/ConfigFileTest.cpp`) |
| kiln, final (`c32e3a004`) | 65 / 0 | 60 / 0 | 86/86 | 54 s ‡ | 589.7 s | 338 MB (`tests/ConfigFileTest.cpp`) |
| clang++-19, libc++ | 65 / 0 | 60 / 0 | 86/86 | 38 s | 519.9 s | 967 MB (`tests/OptionTypeTest.cpp`) |
| kiln, libc++, `e9bbdb3ed` | 65 / 0 | 60 / 0 | 0/86 (every binary segfaults) | 41 s | — | 450 MB (`tests/OptionTypeTest.cpp`) |
| **kiln, libc++, final** (`c32e3a004`) | **65 / 0** | **60 / 0** | **86/86** | 55 s ‡ | 606.8 s | **441 MB** (`tests/OptionTypeTest.cpp`) |

The first control build's wall (106 s) included the clone; the 29 s above
is a rerun with the source present, like the kiln runs. kiln takes
1.05–1.17× clang's summed TU time and 0.46–0.76× its peak RSS (LLVM
codegen runs out of process in llc-19). ‡ Those two runs shared the host
with four other harness builds (load average 25); their Σ TU wall shows
the wall is host load, not compile cost. No warnings from either
compiler in any build.

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `type Q cannot be narrowed to Q in initializer list [-Wc++11-narrowing]` (the harness's normalized signature; a `double` narrowed to the `int64_t` constructor parameter) at `TypeTools.hpp:1311` `output = T{dval}` (`T` = `CLI::intWrapper64`, in `fuzz/fuzzApp.cpp` and `tests/FuzzFailTest.cpp`) | 2 | fixed, commit `e9bbdb3ed` (sema: an ignored narrowing warning still fails a substitution). CLI11's `is_direct_constructible` probes `TT{std::declval<CC>()}` under `#pragma GCC diagnostic ignored "-Wnarrowing"`; clang's narrowing warnings are `SFINAEFailure`, kiln's failed the substitution only while mapped to an error | `tests/conformance/temp.deduct.general/narrowing-ignored-still-substitution-failure.cpp` |
| 2 | miscompile, libc++ only: every binary segfaults in static initialization (`std::function::operator=` → `__value_func::swap`, from `CLI::detail::ExistingFileValidator`) | 0 (all 60 binaries) | fixed, commit `c32e3a004` (sema: substitute a default after an instantiation-dependent argument with its value). `aligned_storage<sizeof(__buf_)>::type` named the primary template (a one-byte union) because the default `_Align` was substituted with the written `sizeof(__buf_)`, which is instantiation-dependent through `this` | `tests/conformance/temp.arg.general/default-after-instantiation-dependent-argument.cpp` |

## What remains

- kiln does not declare `__builtin_printf` (`use of undeclared identifier`;
  clang and GCC accept it). Found while reducing cluster 2, not on CLI11's
  path.
- Variants not run: Debug, C++26, `CLI11_PRECOMPILED=ON`,
  `CLI11_SINGLE_FILE=ON`, shared library. `docs/status/oss.md` is not
  updated here (`scripts/oss/oss-report.py --update docs/status/oss.md
  cli11` regenerates its row).

## libc++ (WP-694)

Built and tested with `-stdlib=libc++` (the system libc++ 19) by both kiln++ and the clang++-19 control: `scripts/oss/oss-build.sh --stdlib libc++ <project>`, report via `scripts/oss/oss-report.py --stdlib libc++ --perf <project>`. The clang-control build tree was deleted after these numbers were recorded. Wall-time differences are dominated by file-system syscalls on the bind mount (see the perf finding in [../oss.md](../oss.md#libc-results-wp-694)).

The known-good check: the WP-685 `std::function` swap miscompile (cluster 2 above) stays fixed under libc++.

<!-- oss:cli11:libcxx -->
### cli11 (v2.6.1), libc++

| | kiln | clang-19 |
|---|---|---|
| configure | ok | ok |
| build | ok | ok |
| TUs ok / failed | 65 / 0 | 65 / 0 |
| links ok / failed | 60 / 0 | 60 / 0 |
| tests passed / total | 86 / 86 | 86 / 86 |
| wall | 246 s | 216 s |
| peak TU RSS | 438 MB | 949 MB |

Failure clusters (kiln): none.

Performance over the 64 TUs that compiled with both (CPU time, peak RSS of the compiler and its backend):
geomean kiln/clang-19 time **1.03x**, RSS **0.43x**; total 510 s vs 491 s; peak TU RSS 438 MB vs 949 MB.

| heaviest TUs (by clang-19 time) | clang-19 s | kiln s | time x | clang-19 MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `$KILN/.oss/src/cli11/tests/OptionTypeTest.cpp` | 21.0 | 22.2 | 1.06 | 949 | 438 | 0.46 |
| `$KILN/.oss/src/cli11/tests/AppTest.cpp` | 16.6 | 17.7 | 1.06 | 685 | 317 | 0.46 |
| `$KILN/.oss/src/cli11/tests/TransformTest.cpp` | 16.0 | 16.9 | 1.05 | 733 | 326 | 0.45 |
| `$KILN/.oss/src/cli11/tests/ConfigFileTest.cpp` | 15.6 | 16.5 | 1.06 | 598 | 320 | 0.54 |
| `$KILN/.oss/src/cli11/tests/SetTest.cpp` | 15.1 | 15.8 | 1.04 | 715 | 319 | 0.45 |

TUs over 1.2x on time or RSS: 1 of the 64 that take clang-19 at least 0.5 s.
- `$KILN/.oss/src/cli11/tests/TimerTest.cpp`: time 1.6 / 1.1 s = 1.43x, RSS 73 / 196 MB = 0.37x
<!-- /oss:cli11:libcxx -->
