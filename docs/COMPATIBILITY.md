# Compatibility

This page lists every standard library, library and application that kiln
has been explicitly validated against, with the result. It is a summary.
The per-project detail, including build logs, failure clusters, fixes and
the exact commands to reproduce each run, is in
[status/oss.md](status/oss.md) and the per-project pages under
[status/oss/](status/oss/).

**Language support** is tracked separately. The standards support matrix
is in [STANDARD_SUPPORT.md](STANDARD_SUPPORT.md): one row per paper, from
C++98 to C++26 plus reflection, in the style of clang's `cxx_status`. Its
summary table counts 257 rows done, 3 partial (all three in C++26: P2686,
P2786 and P3394) and 0 todo or untested, over C++98/03, C++11, C++14,
C++17, C++20, C++23, C++26, C++26 reflection, the DR-level papers and the
kiln extensions.

## Method

Each project is built with kiln (`kiln++`) as the C++ compiler, through
the project's own build system (CMake), at a pinned tag
([scripts/oss/projects.toml](../scripts/oss/projects.toml)). Then the
project's own test suite runs. A control build of the same tree, with the
same flags and test filter, uses clang++-19 (g++-16 for mp-units, which
clang 19 cannot build). The harness (`scripts/oss/`,
[spec/landed/harness/WP-666.md](../spec/landed/harness/WP-666.md)) then
compares the two builds: TUs compiled, targets linked, and tests passed.
Each kiln-only failure is reduced to a `tests/conformance` case and fixed
under a work package (WP).

A **kiln-only failure** is a test (or a TU, link or example the tests
depend on) that passes with the control compiler and fails with kiln. A
test that fails under both compilers (for example, a container limit or a
test that clang also rejects) does not count against kiln. Host: aarch64
Linux (Debian), in a container.

## Ratings

Every result below carries one rating. For an open-source project the
pass rate is kiln's passing tests over the control's passing tests; for a
standard library's own test suite it is passing tests over the tests that
apply to the platform.

| Rating | Meaning |
|---|---|
| **Flawless** | No kiln-only failures: everything the control builds, kiln builds, and every test that passes with the control passes with kiln (pass rate 100%). |
| **Near-perfect** | A handful of kiln-only failures: fewer than 10, or a pass rate of at least 99%. |
| **Solid** | Pass rate of at least 95%. |
| **Partial** | Builds and runs, pass rate below 95%. |
| **Broken** | Does not build, or its tests cannot run. |
| **Not run** | Not attempted yet. |

A rating reflects the last measured run. When a fix has landed since, the
notes say so, and the rating moves up at the next run.

## Standard libraries

kiln does not ship a standard library. It compiles the platform's
library. libstdc++ is the default (`-stdlib=libstdc++`); libc++ is
selected with `-stdlib=libc++` ([spec/driver.md](../spec/driver.md)).

| Standard library | Test suite | Pass rate | Rating |
|---|---|---|---|
| libc++ 22.1.8 | libc++'s own test suite (9247 tests), `-std=c++26` | 99.2% | **Near-perfect** |
| libc++ 19.1.7 | libc++'s own test suite (`std/`) | 98.3% | **Solid** |
| libc++ 19 | The 10 wave-1 open-source projects' test suites, built with `-stdlib=libc++` | 100% of clang's passing tests | **Flawless** |
| libstdc++ (GCC 14, the default) | Every public header (109) compiled at `-std=c++20` and `-std=c++26` | 100% | **Flawless** |
| libstdc++ (GCC 14, the default) | Every open-source project below | See the table below | — |
| libstdc++ (GCC 14) | libstdc++'s own test suite | — | **Not run** (Debian does not ship its sources) |

Per-run detail: [status/libcxx.md](status/libcxx.md),
[status/libcxx-m5.md](status/libcxx-m5.md),
[status/libstdcxx.md](status/libstdcxx.md).

## Libraries and applications

47 projects: 43 **Flawless**, 3 **Near-perfect** (simdjson, folly, the Boost metaprogramming libraries), 1 **Partial** (folly::coro: its known failures are fixed (WP-802, WP-933), and the re-run is deferred for lack of disk). The libc++ column is the WP-694 run against
libc++ 19. Every other build uses the host's libstdc++ 14. Test counts are
"kiln passed / total"; the clang count is the same unless it is given.

### Wave 1: header-only and small libraries

| project | version / tag | scope validated | std lib(s) | rating and result vs clang | notes / WP |
|---|---|---|---|---|---|
| [Catch2](status/oss/catch2.md) | `v3.16.1` | full CTest suite (167 TUs) | libstdc++ 14, libc++ 19 | **Flawless**: 81/81; libc++ 81/81 | WP-681 |
| [CLI11](status/oss/cli11.md) | `v2.6.1` | full test suite | libstdc++ 14, libc++ 19 | **Flawless**: 86/86; libc++ 86/86 | WP-685 (one miscompile found only under libc++, fixed) |
| [doctest](status/oss/doctest.md) | `v2.5.3` | full ctest suite (121 TUs) | libstdc++ 14, libc++ 19 | **Flawless**: 193/193; libc++ 193/193 | WP-683 |
| [fmt](status/oss/fmt.md) | `12.1.0` | full test suite in the default, `FMT_PEDANTIC`, Debug and C++26 configurations | libstdc++ 14, libc++ 19 | **Flawless**: 21/21 (`FMT_PEDANTIC` 25/25); libc++ 21/21 | WP-667, WP-694 |
| [googletest](status/oss/googletest.md) | `v1.17.0` | full suite, googletest and googlemock | libstdc++ 14, libc++ 19 | **Flawless**: 63/63; libc++ 63/63 | WP-682, WP-694 |
| [magic_enum](status/oss/magic-enum.md) | `v0.9.7` | full suite (190 Catch2 cases, 3642 assertions) | libstdc++ 14, libc++ 19 | **Flawless**: 15/15; libc++ 15/15 | WP-683 ([small-trio.md](status/oss/small-trio.md)) |
| [nlohmann/json](status/oss/nlohmann-json.md) | `v3.12.0` | full ctest suite | libstdc++ 14, libc++ 19 | **Flawless**: 101/101; libc++ 97/97 | WP-668 |
| [range-v3](status/oss/range-v3.md) | `0.12.0` | full ctest suite | libstdc++ 14, libc++ 19 | **Flawless**: 238/239; libc++ 254/255. The one failure (`tokenize`, plus the `calendar` example under libc++) fails under clang too | WP-669 |
| [spdlog](status/oss/spdlog.md) | `v1.17.0` | full suite (179 Catch2 cases, 588 assertions) | libstdc++ 14, libc++ 19 | **Flawless**: 1/1; libc++ 1/1 | WP-680 |
| [tl::expected](status/oss/tl-expected.md) | `v1.3.1` | full suite (38 Catch2 cases, 454 assertions) | libstdc++ 14, libc++ 19 | **Flawless**: 1/1; libc++ 1/1 | WP-683 ([small-trio.md](status/oss/small-trio.md)) |

### Wave 2: compiled libraries

| project | version / tag | scope validated | std lib(s) | rating and result vs clang | notes / WP |
|---|---|---|---|---|---|
| [Abseil](status/oss/abseil.md) | `20260107.1` | full CTest suite, googletest built by the same compiler | libstdc++ 14 | **Flawless**: 232/232 | WP-686 |
| [Google Benchmark](status/oss/benchmark.md) | `v1.9.5` | full ctest suite (benchmark, output and gtest unit tests) | libstdc++ 14 | **Flawless**: 84/84 | WP-695 |
| [cereal](status/oss/cereal.md) | `v1.3.2` | full ctest suite | libstdc++ 14 | **Flawless**: 40/40 | WP-704 |
| [Eigen](status/oss/eigen.md) | `3.4.1` | subset: 132 test targets (539 TUs, 539 ctest tests) | libstdc++ 14 | **Flawless**: 539/539 | WP-709, with WP-686's allocation fixes |
| [EnTT](status/oss/entt.md) | `v3.15.0` | full ctest suite | libstdc++ 14 | **Flawless**: 68/68 | WP-690 |
| [glog](status/oss/glog.md) | `v0.7.1` | full CTest suite | libstdc++ 14 | **Flawless**: 23/24 under both (`symbolize` fails under both: environment) | WP-700 |
| [RE2](status/oss/re2.md) | `2025-11-05` | full test suite; Abseil, googletest and Benchmark built by the same compiler | libstdc++ 14 | **Flawless**: 20/20 | WP-710 |
| [simdjson](status/oss/simdjson.md) | `v4.2.4` | developer-mode CTest suite (without `checkperf` and `fuzz`) | libstdc++ 14 | **Near-perfect**: 116/117 (clang 117/117) | the failing test is a negative compile test that needs `-Wuninitialized` under `-Werror`; **WP-696** (open). WP-933's re-run after WP-862 was deferred for lack of disk. WP-691 |
| [Taskflow](status/oss/taskflow.md) | `v3.10.0` | full test suite | libstdc++ 14 | **Flawless**: 2462/2462 | WP-703 |
| [yaml-cpp](status/oss/yaml_cpp.md) | `yaml-cpp-0.9.0` | full suite (1011 googletest tests), vendored googletest built by the same compiler | libstdc++ 14 | **Flawless**: 1/1 | WP-698 |

### Wave 3: large libraries

| project | version / tag | scope validated | std lib(s) | rating and result vs clang | notes / WP |
|---|---|---|---|---|---|
| [folly](status/oss/folly.md) | `v2026.10.26.00` | CTest suite minus three exclusions, Release | libstdc++ 14 | **Near-perfect**: full run 3767/3773 (clang 3773/3773). The 6 were `memcpy_test`, which did not link (no `[[gnu::ifunc]]`/`alias` codegen). After WP-763 a targeted re-run passes `memcpy_test` 6/6, which makes 3773; the full suite was not re-run | WP-726, WP-763 |
| [leveldb](status/oss/leveldb.md) | `1.23` | full CTest suite | libstdc++ 14 | **Flawless**: 30/30 | WP-720 |
| [LLVM Support, ADT, IR](status/oss/llvm_project.md) | `llvmorg-19.1.7` | everything `SupportTests`, `ADTTests` and `IRTests` link (1504 TUs); all three binaries run | libstdc++ 14 | **Flawless**: SupportTests 1337 passed (11 skipped), ADTTests 1576/1576, IRTests 732/732 | WP-730, WP-775 |
| [protobuf](status/oss/protobuf.md) | `v33.1` | CTest suite, including the kiln-built `protoc` that generates the test protos | libstdc++ 14 | **Flawless**: 4/4 | WP-716 |
| [RocksDB](status/oss/rocksdb.md) | `v10.7.5` | a bounded test selection | libstdc++ 14 | **Flawless**: 2158/2158 selected tests | WP-724 |

### Wave 4: constexpr-heavy libraries, coroutines, Boost, LLVM and applications

| project | version / tag | scope validated | std lib(s) | rating and result vs clang | notes / WP |
|---|---|---|---|---|---|
| [Bitcoin Core](status/oss/bitcoin_core.md) | `v32.1` | ctest suite, `bench_bitcoin -sanity-check`, functional tests | libstdc++ 14 | **Flawless**: ctest 357/357 (clang 357/358); 380 functional tests pass | the 2 functional tests that failed in the parallel run pass when run alone. The same filesystem failure also happens under clang. WP-811 |
| [Boost, compiled libraries](status/oss/boost_compiled.md) (25: system, filesystem, thread, regex, serialization, json, log, locale, context, fiber, wave, test, ...) | `boost-1.90.0` | CMake superproject, the libraries and their CTest tests | libstdc++ 14 | **Flawless**: same 1898/1917 TUs and 1215/1227 links as the control; 1326/1327 tests | the one failure is a test-ordering flake of the harness. 41 tests that fail under the control are excluded. WP-804 |
| [Boost, heavy libraries](status/oss/boost_heavy.md) (Asio, Beast, Cobalt, Graph, Geometry, Math) | `boost-1.90.0` | CMake superproject, the six libraries' CTest tests | libstdc++ 14 | **Flawless**: 602/602 | WP-812 |
| [Boost, metaprogramming libraries](status/oss/boost_meta.md) (20: mp11, hana, fusion, mpl, pfr, describe, variant2, spirit, yap, hof, ...) | `boost-1.90.0` | CMake superproject, CTest tests | libstdc++ 14 | **Near-perfect**: tests 2079/2087 (clang 2087/2099), 99.6% of clang's passes; 2230/2253 TUs (clang 2240) | of the 23 failed TUs, 13 also fail under the control. 5 diverge from clang 19 but match clang 22 (not a bug), and 5 compile after later fixes. WP-787, WP-802 |
| [CMake](status/oss/cmake_app.md) | `v4.4.5` | whole CTest suite | libstdc++ 14 | **Flawless**: 689/714 (clang 690/714) | the one difference passes on rerun. WP-811 |
| [ctre](status/oss/ctre.md) | `v3.11.0` | all 25 test TUs (`static_assert` tests) | libstdc++ 14 | **Flawless**: 25/25 TUs | WP-792 |
| [daw_json_link](status/oss/daw_json_link.md) | `v3.38.1` | full ctest suite | libstdc++ 14 | **Flawless**: 147/147 | WP-792 |
| [DuckDB](status/oss/duckdb.md) | `v1.5.6` | `unittest` (Catch2 and the default sqllogictest files) and the shell | libstdc++ 14 | **Flawless**: 4615/4628 under both | the same 13 fail under both because of the host. WP-799 |
| [folly::coro](status/oss/folly_coro.md) | `v2026.10.26.00` | folly's coroutine tests | libstdc++ 14 | **Partial**: 583/618 (clang 616/618), 94.6% of clang's passes | one link failure (a consteval call in a `co_return` operand of a coroutine template) accounts for all 33 kiln-only failures. WP-802 fixed it (`cea9ab532`). WP-933's partial re-run found two more kiln-only compile failures (an init-list mangling and an empty `auto...` pack); both are fixed, and the full re-run is deferred for lack of disk. WP-789, WP-933 |
| [Glaze](status/oss/glaze.md) | `v9.1.0` | full ctest suite | libstdc++ 14 | **Flawless**: 100/102 under both | the same 2 TUs fail under both with libstdc++ 14. WP-792 |
| [gRPC](status/oss/grpc.md) | `v1.85.1` | the C++ core and its submodule dependencies; kiln-built `protoc` and `grpc_cpp_plugin` | libstdc++ 14 | **Flawless**: 117/118 under both | `end2end_http2_test` hits the 300 s limit under both. WP-811 |
| [libunifex](status/oss/libunifex.md) | `v0.4.0` | full test suite | libstdc++ 14 | **Flawless**: 91/94 under both | the 3 io_uring tests are not permitted in the container. WP-789 |
| [LLVM (all of `llvm/`)](status/oss/llvm_full.md) | `llvmorg-19.1.7` | every library, the AArch64 and X86 backends, every tool, `check-llvm` | libstdc++ 14 | **Flawless**: run 9, 38434 passed, 7 failed (clang: 38432 passed, 9 failed) | 6 failures are the control's own environmental failures. The seventh was fixed after run 9 and verified under lit. WP-782 |
| [clang and lld](status/oss/llvm_clang.md) | `llvmorg-19.1.7` | 3565 TUs, 157 targets, `check-clang`, `check-lld` | libstdc++ 14 | **Flawless**: `check-lld` 2193 passed; `check-clang` 35229 passed, the control's count | `check-clang` reached parity after one more miscompile fix, verified on the affected binary. WP-801 |
| [LLVM self-host](status/oss/llvm_selfhost.md) | `llvmorg-19.1.7` | the kiln-built clang (stage 1) builds clang and lld again (stage 2); stage 2 runs `check-clang` and `check-lld`; stage-2/stage-3 IR comparison | libstdc++ 14 | **Flawless**: 35229 and 2193 passed, with the control's 7 environmental failures | stage-2/stage-3 IR is byte-identical after two documented normalizations. No miscompile found. WP-815 |
| [mp-units](status/oss/mp_units.md) | `v2.5.0` | full ctest suite | libstdc++ 14 | **Flawless** (against **g++-16**): 25/25 | clang 19 cannot build mp-units. WP-792 |
| [oneTBB](status/oss/onetbb.md) | `v2022.3.0` | full CTest suite | libstdc++ 14 | **Flawless**: 139/139 | WP-803 |
| [OpenCV](status/oss/opencv.md) | `4.12.0` | modules core, imgproc, imgcodecs, features2d, flann, calib3d, ts | libstdc++ 14 | **Flawless**: 7/7 | WP-803 |
| [reflect-cpp](status/oss/reflect_cpp.md) | `v0.27.1` | full ctest suite | libstdc++ 14 | **Flawless**: 219/220 under both | the same 4 test TUs fail under both. WP-792 |
| [scnlib](status/oss/scnlib.md) | `v4.0.1` | full ctest suite | libstdc++ 14 | **Flawless**: 25/25 | WP-792 |
| [stdexec](status/oss/stdexec.md) | `nvhpc-26.05` | full test suite, including the `*_fail` tests that match error text | libstdc++ 14 | **Flawless**: 907/907 | WP-789 |
| [Z3](status/oss/z3.md) | `z3-4.15.8` | `test-z3 /a`, and the shell on the SMT-LIB2 examples | libstdc++ 14 | **Flawless**: every test and example gives the control's output, apart from timings | WP-799 |

## Rejecting invalid code

The tables above measure valid code. How often kiln accepts code that it
should reject is measured on clang's own negative tests: the `-verify`
tests of `clang/test/CXX` and `clang/test/SemaCXX` at `llvmorg-22.1.8`, run
through kiln by `cargo xtask clang-neg` (WP-912,
[spec/clang-neg.md](../spec/clang-neg.md)). Only lines are compared, never
wording: an `expected-error` counts as reported when kiln has an error on
its line. Run of 2026-11-08 on kiln `6c742784e`:

| measure | result |
|---|---|
| Test files where clang expects an error and kiln reports none (in some configuration) | 7.2% (90 of 1249) |
| `expected-error` lines with no kiln error on them | 17.9% (5015 of 28043) |
| Configurations of `expected-no-diagnostics` tests where kiln reports an error (rejects valid) | 11.7% (49 of 418) |
| Crashes, timeouts, out of memory | 7 crashes (2 root causes), 0, 0 of 2850 cases |

Most of the missed errors are declaration rules: types defined where the
grammar forbids them, members named after their class, union and static
member rules, redeclarations, and clang attribute checks. The breakdown,
the triage clusters with their reduced tests, and the rejects-valid cases
are in [status/accepts-invalid.md](status/accepts-invalid.md).

## Constexpr and compile-time performance

The wave-4 constexpr-heavy projects were built once with kiln and once
with the control, on the same host, and compared per TU over the TUs
that compile under both (WP-792, [status/oss.md](status/oss.md)
"Constexpr performance"). Time is the TU's CPU time (user plus system).
RSS is its peak, including the LLVM backend. A ratio below 1 means kiln
is faster or smaller.

| project | control | TUs | time (geomean) | RSS (geomean) | peak TU RSS kiln / control |
|---|---|---|---|---|---|
| ctre | clang-19 | 25 | 1.06x | 0.45x | 166 / 414 MB |
| mp-units | g++-16 | 64 | 0.92x | 0.39x | 573 / 1539 MB |
| daw_json_link | clang-19 | 159 | 1.06x | 0.41x | 137 / 304 MB |
| Glaze | clang-19 | 87 | 0.97x | 0.47x | 1658 / 2171 MB |
| scnlib | clang-19 | 47 | 0.97x | 0.43x | 717 / 827 MB |
| reflect-cpp | clang-19 | 317 | 0.91x | 0.39x | 250 / 549 MB |

The five heaviest TUs of each clang-controlled project, re-run alone with
kiln, clang++-19 and g++-16 under the harness's 3 GB address-space cap.
Geomean kiln / g++-16:

| project | time | RSS | note |
|---|---|---|---|
| ctre | 1.21x | 0.29x | g++-16 is the fastest of the three on ctre |
| daw_json_link | 0.49x | 0.20x | |
| scnlib | 0.66x | 0.57x | |
| Glaze | 0.56x | 0.28x | over 2 TUs: g++-16 runs out of the 3 GB cap on `json_test`, `yaml_test` and `beve_test`, where kiln peaks at 1244, 1658 and 759 MB |
| reflect-cpp | 0.68x | 0.21x | |

Outside the constexpr-heavy projects, the compile-time ratios are on each
project's page. For example, all of LLVM takes 0.94x clang's summed TU
time and 0.63x its peak TU RSS ([status/oss/llvm_full.md](status/oss/llvm_full.md)).

---

Last updated 2026-11-08, reflecting kiln `main` at `6b8b36f03`.
