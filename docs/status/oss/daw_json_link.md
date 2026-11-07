# daw_json_link with kiln++ (WP-792)

This page covers daw_json_link **v3.38.1** (tag `v3.38.1`). It was built
with kiln++ as `CMAKE_CXX_COMPILER` through the project's own CMake build
and test suite. The control is a clang++-19 build of the same tree, and
the harness is M8's (`scripts/oss/`, spec/landed/harness/WP-666.md,
WP-792).

daw_json_link maps JSON to C++ types through `constexpr` member
descriptions. Its parser and serializer run in constant evaluation in
many tests. `daw-header-libraries` and `daw-utf-range` are fetched at
configure time. A shim (`scripts/oss/shims/daw-pin-header-libraries.cmake`)
pins `header_libraries` to `v2.135.0`, because the upstream default is
its moving `v2` branch, which no longer builds v3.38.1 with either
compiler. The Boost tests are off: the system Boost 1.83 has no
Boost.Container CMake package.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.** All 159 TUs compile, all 158 executables link and all
147 ctest tests pass under both compilers.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh daw_json_link --compiler clang
scripts/oss/oss-build.sh daw_json_link
scripts/oss/oss-report.py --perf daw_json_link
```

```sh
cmake --fresh -S .oss/src/daw_json_link -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DDAW_ENABLE_TESTING=ON -DDAW_JSON_USE_BOOST=OFF \
  -DCMAKE_PROJECT_daw-json-link_INCLUDE=scripts/oss/shims/daw-pin-header-libraries.cmake
nice -n 10 make -k
ctest --output-on-failure -j 4 --timeout 300
```

## Results

Time is the wrapper's CPU time (user plus system) per TU. RSS is the
TU's `ru_maxrss`, which includes the backend. Each build was run once.
The kiln build's wall time (377 s vs 89 s) was taken at a much higher
host load than the control's; the per-TU CPU times are the comparison.

| Build | TUs ok / failed | Links | Tests | Σ TU CPU | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (WP-792 base) | 7 / 152 | 7 | — | — | — |
| clang++-19 (control) | 159 / 0 | 158 / 158 | 147/147 | 343 s | 304 MB |
| **kiln, final** | **159 / 0** | **158 / 158** | **147/147** | **351 s** | **137 MB** |

Over the 159 TUs, kiln/clang has a geomean of **1.06x time** and **0.41x
RSS**. The five heaviest TUs (all in `tests/src/`):

| TU | clang s | kiln s | time x | clang MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `apache_builds_hash_comments_test.cpp` | 7.3 | 6.5 | 0.89 | 304 | 135 | 0.44 |
| `apache_builds_cpp_comments_test.cpp` | 6.9 | 5.9 | 0.85 | 303 | 137 | 0.45 |
| `twitter_timeline_test.cpp` | 5.6 | 5.0 | 0.88 | 285 | 125 | 0.44 |
| `full_unicode_roundtrip_test.cpp` | 4.7 | 4.2 | 0.90 | 287 | 128 | 0.45 |
| `daw_json_simd_block_iterator_test.cpp` | 4.6 | 4.1 | 0.90 | 283 | 110 | 0.39 |

Three TUs are over 1.2x on time, among the 152 that take clang at least
0.5 s: `make_full24.cpp` (2.0 s vs 1.6 s, 1.31x), `cookbook_class1_test.cpp`
(2.7 s vs 2.2 s, 1.21x) and `cookbook_kv4_test.cpp` (2.2 s vs 1.8 s,
1.20x). Re-timed alone, they moved by about 0.1x between runs; they are
the perf cluster below.

## Clusters

Correctness. The first build's failures, by first diagnostic. Later rows
were exposed by fixing earlier ones.

| # | Signature | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `tbl.size()` rejected on the undeduced `const std::array` (`static constexpr std::array const tbl = f<T>();` in header_libraries' `daw_cxmath.h` `pow2_t`: a deduced-class-type static member's initializer was deferred) | 152 | fixed, `f270372df` (sema) | `tests/conformance/temp.inst/static-member-deduced-class-type.cpp` |
| 2 | `__builtin_neon_vpaddlq_v` and `__builtin_neon_vqdmulh_v` not implemented (UTF-8 validation through `arm_neon.h`) | 104 (and 3 in glaze) | fixed, `460ff45d3` (codegen) | `tests/conformance/target.neon/ir-paddl-qdmulh-19.cpp` |
| 3 | `invalid use of 'this'` in an instantiated `noexcept(…)` of a constructor (`json_writer_t`) | 5 | fixed, `39102e48d` (sema) | `except.spec/special-member-noexcept-this-instantiation.cpp` |

No miscompiles were found.

Performance (kept apart from correctness):

| # | Signature | TUs | Status |
|---|---|---|---|
| P1 | 1.20x to 1.31x clang's CPU time on three small cookbook/`make_full` TUs | 3 | filed, not fixed. The ratio is within run-to-run noise of the 1.2x bar, and the TUs are about 2 s. |

## What remains

- P1, if it reproduces on a quiet host.
