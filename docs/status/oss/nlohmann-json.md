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
