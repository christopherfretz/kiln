# nlohmann/json with kiln++ (WP-668)

nlohmann/json **v3.12.0** (`55f93686c`) built through its own CMake build
and tested with its own ctest suite (doctest), kiln++ as the C++
compiler, against a clang++-19 control build of the same tree. kiln base:
main `f3751eadd`; final build: branch `wp/668` with the four fixes below.

**Result: parity.** Every TU compiles, and 101/101 ctest tests pass
under kiln, as under clang++-19. No runtime (miscompile) failure was
seen at any point: each test whose TU compiled passed.

## Commands

Run from the scratch directory `D` (not committed). `kxx` is a wrapper
that runs `$KXX` under `ulimit -v 3000000` and `timeout 600` and logs one
line per invocation (source, rc, wall, max RSS from `/usr/bin/time -f
'%e %M'`, first `error:` line). Test data: `json_test_data` v3.1.0
(`https://github.com/nlohmann/json_test_data`).

```sh
git clone --depth 1 --branch v3.12.0 https://github.com/nlohmann/json src
git clone --depth 1 --branch v3.1.0 https://github.com/nlohmann/json_test_data
export KXX=/path/to/kiln++        # control: KXX=clang++-19
export KILN_RESOURCE_DIR=<kiln checkout>/resource
cmake -G "Unix Makefiles" -S src -B build-kiln -DCMAKE_CXX_COMPILER=$D/kxx \
  -DJSON_BuildTests=ON -DJSON_MultipleHeaders=ON -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_CXX_STANDARD=20 -DJSON_TestDataDirectory=$D/json_test_data
/usr/bin/time -f 'TOTAL %e %M' nice -n 10 make -k -C build-kiln
cd build-kiln && nice -n 10 ctest --output-on-failure -j 4 --timeout 1500
```

`make` draws its parallelism from the host's shared GNU-make jobserver
(no `-j`). cmake 3.31.6, GNU make 4.4.1, clang 19.1.7; aarch64 Linux.
The suite is configured for C++11/14/17/20/23 variants of the unit tests
(93 TUs, 92 targets).

## Numbers

| | clang++-19 | kiln++ first (main `f3751eadd`) | kiln++ final (`wp/668`) |
|---|---|---|---|
| TUs ok / failed | 93 / 0 | 4 / 86 | 93 / 0 |
| ctest | 101/101 pass | not run (86 targets missing) | 101/101 pass |
| sum of TU wall | 486.5 s | 140.0 s (failing TUs stop early) | 427.2 s |
| peak TU RSS | 573 MiB (`unit-regression1.cpp`) | 155 MiB | 297 MiB (`unit-deserialization.cpp`) |
| slowest TU | 17.8 s (`unit-element_access2.cpp`) | 3.2 s | 15.8 s (`unit-element_access2.cpp`) |
| `make` wall | 36.3 s | 42.6 s | 458.1 s |

The `make` wall clock depends on how many tokens the shared jobserver
had free (the final kiln build ran while three sibling builds drew from
it); the sum of per-TU wall is the comparable figure. No TU came near the
10-minute or 3 GiB caps. An intermediate kiln build (after fix 1) had
81 ok / 6 failed TUs; its 95 built tests all passed.

## Clusters

Grouped by first diagnostic. Repros are the conformance tests.

| # | signature (first diagnostic) | TUs | status | repro |
|---|---|---|---|---|
| 1 | `binary_writer.hpp:663: call to 'to_char_type' is ambiguous` (SFINAE static member templates of a class template resolved at the definition) | 86 | fixed in `32a56279c` | `tests/conformance/temp.dep.expr/member-template-overload-dependent-context.cpp` |
| 2 | `unit-constructor1.cpp:415: cannot initialize an array element of type 'const char' with an lvalue of type 'const char[12]'` (`char a[] = {"lit"}`) | 1 | fixed in `1e4832492` | `tests/conformance/dcl.init.string/braced-string-literal-run.cpp`, `braced-string-literal-excess.cpp` |
| 3 | `unit-regression2.cpp:376: out-of-line definition of 'for_3333' does not match any declaration` (`template<> P::P(const J&)` specializing a constructor template) | 3 | fixed in `4fb4e31da` | `tests/conformance/temp.expl.spec/special-member-template-spec-run.cpp`, `special-member-template-spec-no-match.cpp` |
| 4 | `unit-json_pointer.cpp:744: 'string_t' does not refer to a value` (`is_same<A, oj::json_pointer::string_t>`: a member typedef naming a class template specialization, in a template argument) | 2 | fixed in `e08a0ccd9` | `tests/conformance/temp.arg.type/member-typedef-of-specialization-qualifier.cpp` |
| 5 | `kiln++: error: unknown argument: '-print-resource-dir'` (CMake's compiler-identification probe; non-fatal, CMake only records no resource dir) | 0 | follow-up (driver) | `kiln++ -print-resource-dir` |

Cluster 1's first build also failed `json_v3_10_5.hpp:13931` (the ABI
test's bundled older header) with the same diagnostic; it is counted in
the 86. Clusters 2-4 were masked by cluster 1 in the first build.

## What remains

- `-print-resource-dir` (cluster 5): kiln++ rejects it, so CMake leaves
  `CMAKE_CXX_COMPILER_CLANG_RESOURCE_DIR` unset (it matters only for C++
  module dependency scanning). The driver should print its resource
  directory as clang does.
- Not measured: a `-stdlib=libc++` build (libstdc++ is the default),
  `-DCMAKE_BUILD_TYPE=Debug`, and per-TU timing medians (one run each).
  The expensive tests ran (`JSON_FastTests` defaults to OFF).

## libc++ (WP-694)

Built and tested with `-stdlib=libc++` (the system libc++ 19) by both kiln++ and the clang++-19 control: `scripts/oss/oss-build.sh --stdlib libc++ <project>`, report via `scripts/oss/oss-report.py --stdlib libc++ --perf <project>`. The clang-control build tree was deleted after these numbers were recorded. Wall-time differences are dominated by file-system syscalls on the bind mount (see the perf finding in [../oss.md](../oss.md#libc-results-wp-694)).

<!-- oss:nlohmann_json:libcxx -->
### nlohmann_json (v3.12.0), libc++

| | kiln | clang-19 |
|---|---|---|
| configure | ok | ok |
| build | ok | ok |
| TUs ok / failed | 93 / 0 | 93 / 0 |
| links ok / failed | 88 / 0 | 88 / 0 |
| tests passed / total | 97 / 97 | 97 / 97 |
| wall | 697 s | 534 s |
| peak TU RSS | 278 MB | 566 MB |

Failure clusters (kiln): none.

Performance over the 78 TUs that compiled with both (CPU time, peak RSS of the compiler and its backend):
geomean kiln/clang-19 time **1.23x**, RSS **0.43x**; total 326 s vs 280 s; peak TU RSS 278 MB vs 566 MB.

| heaviest TUs (by clang-19 time) | clang-19 s | kiln s | time x | clang-19 MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `$KILN/.oss/src/nlohmann_json/tests/src/unit-regression1.cpp` | 13.1 | 12.7 | 0.97 | 566 | 238 | 0.42 |
| `$KILN/.oss/src/nlohmann_json/tests/src/unit-element_access2.cpp` | 12.9 | 13.6 | 1.05 | 435 | 270 | 0.62 |
| `$KILN/.oss/src/nlohmann_json/tests/src/unit-regression2.cpp` | 12.5 | 14.2 | 1.13 | 516 | 242 | 0.47 |
| `$KILN/.oss/src/nlohmann_json/tests/src/unit-deserialization.cpp` | 12.5 | 14.9 | 1.19 | 399 | 269 | 0.67 |
| `$KILN/.oss/src/nlohmann_json/tests/src/unit-bjdata.cpp` | 11.2 | 12.3 | 1.10 | 387 | 278 | 0.72 |

TUs over 1.2x on time or RSS: 45 of the 78 that take clang-19 at least 0.5 s.
- `$KILN/.oss/src/nlohmann_json/tests/src/unit-type_traits.cpp`: time 1.1 / 0.5 s = 2.00x, RSS 65 / 158 MB = 0.41x
- `$KILN/.oss/src/nlohmann_json/tests/src/unit-large_json.cpp`: time 2.3 / 1.5 s = 1.46x, RSS 81 / 205 MB = 0.39x
- `$KILN/.oss/src/nlohmann_json/tests/src/unit-constructor2.cpp`: time 2.4 / 1.6 s = 1.45x, RSS 75 / 218 MB = 0.35x
- `$KILN/.oss/src/nlohmann_json/tests/src/unit-ordered_json.cpp`: time 3.6 / 2.5 s = 1.45x, RSS 90 / 257 MB = 0.35x
- `$KILN/.oss/src/nlohmann_json/tests/src/unit-concepts.cpp`: time 1.7 / 1.2 s = 1.44x, RSS 72 / 202 MB = 0.35x
<!-- /oss:nlohmann_json:libcxx -->
