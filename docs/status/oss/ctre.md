# ctre with kiln++ (WP-792)

This page covers compile-time-regular-expressions **v3.11.0** (tag
`v3.11.0`). It was built with kiln++ as `CMAKE_CXX_COMPILER` through
ctre's own CMake build. The control is a clang++-19 build of the same
tree, and the harness is M8's (`scripts/oss/`,
spec/landed/harness/WP-666.md, WP-792).

ctre parses each regular expression at compile time into a type (an LL(1)
parser over a template string NTTP) and matches by instantiation. Its test
suite is compile-only: 25 TUs of `static_assert`s, each archived into a
static library by the `EXCLUDE_FROM_ALL` target `ctre-test`. The harness
test step checks that all 25 archives exist.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity on the first build.** All 25 TUs compile under both
compilers, so every `static_assert` holds under kiln.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh ctre --compiler clang
scripts/oss/oss-build.sh ctre
scripts/oss/oss-report.py --perf ctre
```

These run, in `.oss/build/ctre/<kiln|clang>/`:

```sh
cmake --fresh -S .oss/src/ctre -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DCTRE_BUILD_TESTS=ON -DCTRE_BUILD_PACKAGE=OFF
nice -n 10 make -k ctre-test
```

## Results

Time is the wrapper's CPU time (user plus system) per TU. RSS is the
TU's `ru_maxrss`, which includes the backend. Each build was run once.

| Build | TUs ok / failed | Test archives | Σ TU CPU | Peak TU RSS |
|---|---|---|---|---|
| clang++-19 (control) | 25 / 0 | 25/25 | 30 s | 414 MB (`tests/generating.cpp`) |
| **kiln, first = final (WP-792 base)** | **25 / 0** | **25/25** | **28 s** | **166 MB** (`tests/generating.cpp`) |

Over the 25 TUs, kiln/clang has a geomean of **1.06x time** and **0.45x
RSS**. The ratio is above 1 only on the small TUs, where process start-up
dominates. The five heaviest TUs all run faster under kiln, at 0.82x to
0.88x of clang's time and 0.39x to 0.46x of its RSS:

| TU | clang s | kiln s | time x | clang MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `tests/generating.cpp` | 5.1 | 4.2 | 0.82 | 414 | 166 | 0.40 |
| `tests/matching2.cpp` | 4.1 | 3.4 | 0.83 | 377 | 148 | 0.39 |
| `tests/matching2-msvc-greedy.cpp` | 3.7 | 3.1 | 0.85 | 354 | 137 | 0.39 |
| `tests/parsing.cpp` | 3.3 | 2.9 | 0.86 | 314 | 146 | 0.46 |
| `tests/matching3.cpp` | 3.1 | 2.8 | 0.88 | 325 | 137 | 0.42 |

No TU is over 1.2x on time or RSS, among the 13 that take clang at least
0.5 s.

## Clusters

None. No miscompiles were found.

## What remains

Nothing for ctre itself.
