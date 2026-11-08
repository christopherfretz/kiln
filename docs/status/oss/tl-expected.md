# tl_expected with kiln++ (WP-683)

This page covers tl_expected **v1.3.1** (tag `v1.3.1`). It was built with
kiln++ as `CMAKE_CXX_COMPILER`, through its own CMake build and test
suite. The control is a clang++-19 build of the same tree, and the harness
is M8's (`scripts/oss/`).

The tests fetch Catch2 **v2.13.10** with FetchContent. The same compiler
builds it.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.**
- All 13 TUs compile and the test executable links.
- The suite passes under both compilers: 38 Catch2 test cases and 454
  assertions.
- tl_expected needed no fix of its own. Its one failing TU on main was
  the Catch2 v2 `ResultBase` cluster that WP-680 fixed.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh tl_expected --compiler clang
scripts/oss/oss-build.sh tl_expected
scripts/oss/oss-report.py tl_expected
```

These run the following in `.oss/build/tl_expected/<kiln|clang>/`:

```sh
cmake --fresh -S .oss/src/tl_expected -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release -DEXPECTED_BUILD_TESTS=ON
nice -n 10 make -k
ctest --output-on-failure          # one test; Catch2 counts from ./tl-expected-tests
```

The row in `scripts/oss/projects.toml` worked unchanged and is now
`verified = true`.

## Results

The columns are measured the same way as in magic-enum.md.

| Build | TUs ok / failed | Links | Tests (ctest; Catch2) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `7c5998af2`) | 12 / 1 | 0 / 0 | 0/1 (not built) | 3 s | — | 117 MB |
| clang++-19 (control) | 13 / 0 | 1 / 1 | 1/1; 38 cases, 454 assertions | 14 s | 35.6 s | 337 MB (`tests/main.cpp`) |
| **kiln, final** | **13 / 0** | **1 / 1** | **1/1; 38 cases, 454 assertions** | 9 s | 24.5 s | **209 MB** (`tests/main.cpp`) |

Compared with clang, kiln takes 0.69× the summed TU time and 0.62× the
peak RSS.

Both compilers print the same two warnings:
`tests/assignment.cpp:72:42` and `:73:42`,
`unused variable 'e3'`/`'e4'` (`-Wunused-variable`).

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `'m_type' is a protected member of 'ResultBase'` (`catch.hpp:9037`) | 1 (`tests/main.cpp`, Catch2's main) | fixed by WP-680 (`062f38bf9`, with `e602261d4`, `030cd5639`), merged | `tests/conformance/class.protected/dependent-base-current-object.cpp` |

No miscompiles were found.

## What remains

- **Variants not run:** Debug, C++11/14 (`CMAKE_CXX_STANDARD`), and
  `-fno-exceptions` (`TL_EXPECTED_EXCEPTIONS_ENABLED` off).

## libc++ (WP-694)

Built and tested with `-stdlib=libc++` (the system libc++ 19) by both kiln++ and the clang++-19 control: `scripts/oss/oss-build.sh --stdlib libc++ <project>`, report via `scripts/oss/oss-report.py --stdlib libc++ --perf <project>`. The clang-control build tree was deleted after these numbers were recorded. Wall-time differences are dominated by file-system syscalls on the bind mount (see the perf finding in [../oss.md](../oss.md#libc-results-wp-694)).

<!-- oss:tl_expected:libcxx -->
### tl_expected (v1.3.1), libc++

| | kiln | clang-19 |
|---|---|---|
| configure | ok | ok |
| build | ok | ok |
| TUs ok / failed | 13 / 0 | 13 / 0 |
| links ok / failed | 1 / 0 | 1 / 0 |
| tests passed / total | 1 / 1 | 1 / 1 |
| wall | 29 s | 16 s |
| peak TU RSS | 202 MB | 371 MB |

Failure clusters (kiln): none.

Performance over the 13 TUs that compiled with both (CPU time, peak RSS of the compiler and its backend):
geomean kiln/clang-19 time **1.18x**, RSS **0.38x**; total 25 s vs 22 s; peak TU RSS 202 MB vs 371 MB.

| heaviest TUs (by clang-19 time) | clang-19 s | kiln s | time x | clang-19 MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `$KILN/.oss/src/tl_expected/tests/main.cpp` | 8.6 | 9.8 | 1.14 | 371 | 202 | 0.54 |
| `$KILN/.oss/src/tl_expected/tests/extensions.cpp` | 1.5 | 1.8 | 1.20 | 202 | 80 | 0.40 |
| `$KILN/.oss/src/tl_expected/tests/swap.cpp` | 1.2 | 1.4 | 1.16 | 195 | 74 | 0.38 |
| `$KILN/.oss/src/tl_expected/tests/assertions.cpp` | 1.2 | 1.1 | 0.93 | 189 | 69 | 0.36 |
| `$KILN/.oss/src/tl_expected/tests/issues.cpp` | 1.2 | 1.4 | 1.19 | 198 | 75 | 0.38 |

TUs over 1.2x on time or RSS: 6 of the 13 that take clang-19 at least 0.5 s.
- `$KILN/.oss/src/tl_expected/tests/observers.cpp`: time 1.2 / 0.9 s = 1.29x, RSS 69 / 190 MB = 0.37x
- `$KILN/.oss/src/tl_expected/tests/noexcept.cpp`: time 1.1 / 0.9 s = 1.27x, RSS 68 / 184 MB = 0.37x
- `$KILN/.oss/src/tl_expected/tests/constexpr.cpp`: time 1.2 / 0.9 s = 1.27x, RSS 69 / 185 MB = 0.37x
- `$KILN/.oss/src/tl_expected/tests/relops.cpp`: time 1.1 / 0.9 s = 1.25x, RSS 70 / 189 MB = 0.37x
- `$KILN/.oss/src/tl_expected/tests/constructors.cpp`: time 1.4 / 1.2 s = 1.23x, RSS 73 / 198 MB = 0.37x
<!-- /oss:tl_expected:libcxx -->
