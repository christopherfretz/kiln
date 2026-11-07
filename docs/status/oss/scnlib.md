# scnlib with kiln++ (WP-792)

This page covers scnlib **v4.0.1** (tag `v4.0.1`). It was built with
kiln++ as `CMAKE_CXX_COMPILER` through scnlib's own CMake build and test
suite. The control is a clang++-19 build of the same tree, and the
harness is M8's (`scripts/oss/`, spec/landed/harness/WP-666.md, WP-792).

scnlib is a `scanf` replacement in the style of `std::format`: format
strings are checked in constant evaluation (`consteval` format-string
constructors), and its readers are heavily templated. googletest and
fast_float are fetched at configure time and built with the same
compiler. The regex backend is `std`.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.** All 47 TUs compile, all 3 executables link and all
25 ctest tests pass under both compilers.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh scnlib --compiler clang
scripts/oss/oss-build.sh scnlib
scripts/oss/oss-report.py --perf scnlib
```

```sh
cmake --fresh -S .oss/src/scnlib -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DSCN_TESTS=ON -DSCN_BENCHMARKS=OFF -DSCN_BENCHMARKS_BUILDTIME=OFF \
  -DSCN_BENCHMARKS_BINARYSIZE=OFF -DSCN_EXAMPLES=OFF -DSCN_DOCS=OFF -DSCN_WERROR=OFF
nice -n 10 make -k
ctest --output-on-failure -j 4
```

## Results

Time is the wrapper's CPU time (user plus system) per TU. RSS is the
TU's `ru_maxrss`, which includes the backend. Each build was run once.

| Build | TUs ok / failed | Links | Tests | Σ TU CPU | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (WP-792 base) | 2 / 1 (`make -k` stopped at the library) | 0 | 0/25 | — | 266 MB |
| clang++-19 (control) | 47 / 0 | 3 / 3 | 25/25 | 190 s | 827 MB (`impl_tests/float_reader_test.cpp`) |
| **kiln, final** | **47 / 0** | **3 / 3** | **25/25** | **188 s** | **717 MB** (`impl_tests/float_reader_test.cpp`) |

Over the 43 TUs both logs time, kiln/clang has a geomean of **0.97x
time** and **0.43x RSS**. The five heaviest TUs:

| TU | clang s | kiln s | time x | clang MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `tests/unittests/impl_tests/float_reader_test.cpp` | 25.8 | 25.9 | 1.00 | 827 | 717 | 0.87 |
| `src/scn/impl.cpp` | 23.2 | 24.2 | 1.04 | 620 | 398 | 0.64 |
| `…/integer_reader_test.impl_wide_localized.cpp` | 18.6 | 18.9 | 1.01 | 626 | 530 | 0.85 |
| `…/integer_reader_test.impl_wide_classic.cpp` | 18.6 | 18.8 | 1.01 | 625 | 530 | 0.85 |
| `…/integer_reader_test.impl_narrow_localized.cpp` | 18.0 | 18.0 | 1.00 | 617 | 521 | 0.84 |

No TU is over 1.2x on time or RSS, among the 41 that take clang at least
0.5 s. The heavy TUs are dominated by the LLVM backend at `-O3` (both
compilers run the same LLVM passes), so their RSS ratio is closer to 1
than in the other wave 4 projects.

## Clusters

Correctness. The first build's failures, by first diagnostic. Later rows
were exposed by fixing earlier ones.

| # | Signature | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | a `return` in a discarded `if constexpr` branch of a non-template function took part in return type deduction (`get_file_tag`, in the library TU) | 1 (blocked every test) | fixed, `01c955f63` (sema) | `tests/conformance/dcl.spec.auto/discarded-return-not-deduced-run.cpp` |
| 2 | **wrong code**: `scn_tests` aborted with "free(): invalid pointer". `static thread_local std::string err_msg` was emitted as a constant `.tdata` initializer holding its own absolute small-buffer address (the linker warned "R_AARCH64_ABS64 used with TLS symbol") | 1 test | fixed, `2fa8713fc` (oven: a thread-local's own address is not a constant) | `tests/conformance/basic.stc.thread/self-address-dynamic-init-run.cpp` |

Row 2 is a miscompile.

## What remains

Nothing for scnlib itself.
