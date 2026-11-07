# glaze with kiln++ (WP-792)

This page covers glaze **v9.1.0** (tag `v9.1.0`). It was built with
kiln++ as `CMAKE_CXX_COMPILER` through glaze's own CMake build and test
suite. The control is a clang++-19 build of the same tree, and the
harness is M8's (`scripts/oss/`, spec/landed/harness/WP-666.md, WP-792).

glaze is a C++23 JSON/BEVE/CBOR/TOML/YAML library that reflects
aggregates at compile time (member names from `__PRETTY_FUNCTION__` and
structured bindings) and builds its perfect-hash key tables in constant
evaluation. The developer-mode test suite is used, with sanitizers,
networking tests (OpenSSL) and performance tests off. Fuzzing is off: it
needs `-fsanitize=fuzzer`, which neither build links. Standalone Asio is
fetched at configure time.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.** 87 of 89 TUs compile, all 79 executables link and
100 of 102 ctest tests pass, under both compilers. The same two TUs,
`tests/cbor_test/cbor_test.cpp` and `tests/msgpack_test/msgpack_test.cpp`,
fail under both. libstdc++ 14 reports "incomplete type
'cbor_ambiguous::nest_b' used in type trait expression", so their two
tests are "Not Run".

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh glaze --compiler clang
scripts/oss/oss-build.sh glaze
scripts/oss/oss-report.py --perf glaze
```

```sh
cmake --fresh -S .oss/src/glaze -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=23 -DCMAKE_BUILD_TYPE=Release \
  -Dglaze_DEVELOPER_MODE=ON -DBUILD_TESTING=ON -Dglaze_ENABLE_SANITIZERS=OFF \
  -Dglaze_BUILD_NETWORKING_TESTS=OFF -Dglaze_BUILD_PERFORMANCE_TESTS=OFF \
  -Dglaze_ENABLE_FUZZING=OFF -DCMAKE_DISABLE_PRECOMPILE_HEADERS=ON
nice -n 10 make -k
ctest --output-on-failure -j 4 --timeout 300
```

## Results

Time is the wrapper's CPU time (user plus system) per TU. RSS is the
TU's `ru_maxrss`, which includes the backend. Each build was run once.
The kiln build's wall time (1008 s vs 265 s) was taken at a much higher
host load than the control's; the per-TU CPU times are the comparison.

| Build | TUs ok / failed | Links | Tests | Σ TU CPU | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (WP-792 base, fuzzing off) | 1 / 88 | — | 26/102 | — | — |
| clang++-19 (control) | 87 / 2 | 79 / 79 | 100/102 | 970 s | 2171 MB (`tests/json_test/json_test.cpp`) |
| **kiln, final** | **87 / 2** | **79 / 79** | **100/102** | **869 s** | **1658 MB** (`tests/yaml_test/yaml_test.cpp`) |

Over the 82 TUs both logs time, kiln/clang has a geomean of **0.97x
time** and **0.47x RSS**. The five heaviest TUs (all in `tests/`):

| TU | clang s | kiln s | time x | clang MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `json_test/json_test.cpp` | 143.5 | 102.9 | 0.72 | 2171 | 1244 | 0.57 |
| `yaml_test/yaml_test.cpp` | 133.6 | 120.2 | 0.90 | 2049 | 1658 | 0.81 |
| `beve_test/beve_test.cpp` | 68.0 | 61.3 | 0.90 | 1532 | 759 | 0.50 |
| `toml_test/toml_test.cpp` | 44.6 | 38.2 | 0.86 | 857 | 506 | 0.59 |
| `json_test/json_wide_variant_test.cpp` | 36.1 | 29.9 | 0.83 | 1256 | 613 | 0.49 |

Two small TUs are over 1.2x on time, among the 81 that take clang at
least 0.5 s: `cli_menu_test.cpp` (3.4 s vs 2.7 s, 1.25x) and
`float_write_test.cpp` (2.9 s vs 2.3 s, 1.23x).

## Clusters

Correctness. The first build's failures, by first diagnostic. Later rows
were exposed by fixing earlier ones.

| # | Signature | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `unknown argument: '-ftime-trace'` (glaze's test CMake adds it for clang) | 81 | fixed, `f40ab9a4a` (driver: accepted with an unused-argument warning) | `crates/kiln_driver/tests/driver.rs` |
| 2 | `__builtin_neon_vpaddlq_v` / `vqdmulh` not implemented (SIMD UTF-8 validation) | 3 (and 104 in daw_json_link) | fixed, `460ff45d3` (codegen) | `tests/conformance/target.neon/ir-paddl-qdmulh-19.cpp` |
| 3 | a vector initializer list with a vector element (`v16 b{a}`) lowered as one `insertelement` (SIMD validator default member initializers) | exposed by 2 | fixed, `4d6b62ac6` (codegen) | `ext.vector/ir-init-list-vector-element-{19,clang22}.cpp` |
| 4 | `&C::m` in a static data member initializer of `C` needs the layout of incomplete `C` (`exceptions_test` field metadata) | exposed by 1 | fixed, `5d561c6a0` (oven) | `expr.unary.op/member-pointer-in-incomplete-class.cpp` |
| 5 | `sys_seconds{sys_days{ymd}}` loses its conversion at instantiation (the chrono reader) | exposed by 1 | fixed, `9ae83de73` (sema) | `expr.type.conv/list-cast-conversion-function-in-template.cpp` |
| 6 | **wrong code**: `[this]` in a default member initializer of an aggregate captured the enclosing closure (the TOML `std::function` members test crashed; a codegen panic in a free function) | 1 test | fixed, `053776b9f` (codegen) | `dcl.init.aggr/default-member-init-this-in-aggregate.cpp` |

Row 6 is a miscompile. The other glaze test failures in the first build
were TUs that did not compile.

Performance (kept apart from correctness):

| # | Signature | TUs | Status |
|---|---|---|---|
| P1 | 1.23x to 1.25x clang's CPU time on two TUs of about 3 s | 2 | filed, not profiled further: the rest of the suite is 0.72x to 0.90x on the heavy TUs, and these two are near the 1.2x bar. |

## What remains

- P1.
- kiln warns "unknown attribute 'flatten' ignored" on glaze's
  `[[gnu::flatten]]`, which clang knows (no effect on the result).
