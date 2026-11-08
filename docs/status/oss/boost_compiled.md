# Boost compiled libraries with kiln++ (WP-804)

Boost **1.90.0** (tag `boost-1.90.0`, superproject with submodules) is built
through its CMake superproject with `BUILD_TESTING=ON` and static libraries
for 25 compiled libraries: system, filesystem, thread, chrono, atomic,
regex, program_options, serialization, date_time, container, json, url,
random, iostreams, log, locale, context, fiber, nowide, charconv,
stacktrace, timer, exception, wave and test. Each library's own `.a` is
built by the compiler under test as well as its tests. Tests run under
CTest. The control is a clang++-19 build of the same tree. Both use the M8
harness (`scripts/oss/`, spec/landed/harness/WP-666.md, WP-787.md and
WP-804.md). Host: aarch64 Linux, libstdc++ 14, C++20 (the project row),
default `-fclang-compat`. Date: 2026-11-08.

**Result:** in the final build (`536392274`) kiln++ compiles the same 1898
of 1917 TUs as the clang-19 control, links the same 1215 of 1227
executables, and passes 1326 of the 1327 ctest tests the row runs. The one
failure is an ordering flake of the harness, not kiln (see Tests). The
19 TUs and 12 links that fail fail the same way under the control. The
first build failed 6 TUs and 4 tests that the control passes. This WP
fixed 4 clusters, and WP-803 fixed the fifth. Kiln takes 1.83x clang's
summed TU CPU time, a perf finding, and about 0.45x its summed peak RSS.

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh boost_compiled --compiler clang
scripts/oss/oss-build.sh boost_compiled
scripts/oss/oss-report.py --perf --update docs/status/oss.md boost_compiled
```

These run the following in `.oss/build/boost_compiled/<kiln|clang>/`. The
wrapper `scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and
`timeout 600`. The full `-D` list is the `configure` array of the
`boost_compiled` row in `scripts/oss/projects.toml`.

```sh
cmake --fresh -S .oss/src/boost -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_TESTING=ON -DBUILD_SHARED_LIBS=OFF \
  "-DBOOST_INCLUDE_LIBRARIES=system;filesystem;...;wave;test" \
  -DCMAKE_PROJECT_Boost_INCLUDE=scripts/oss/shims/boost-jamfile-tests.cmake \
  "-DKILN_BOOST_JAMFILE_LIBRARIES=filesystem;thread;regex;serialization;date_time;iostreams;log;context;fiber;exception;test" \
  -DKILN_BOOST_JAMFILE_...   # per-library rules, variants, defines, links
nice -n 10 make -k all tests          # draws from the shared jobserver
ctest -j 4 --output-on-failure --timeout 300 -E '<41 tests, below>'
```

The shim registers the b2 tests of the 11 libraries that have none in a
superproject CMake build. Several of these Jamfiles wrap `run` in their own
rules; the shim extensions that read them are in
spec/landed/harness/WP-804.md. That gives 1368 ctest tests.

**Optional dependencies.** The configure found zlib, bzip2, LZMA and Zstd
(iostreams), and ICU 76.1 and iconv (locale). These were off: MPI and
Python (superproject defaults), charconv's quadmath (`__float128`
unavailable on aarch64), stacktrace's libbacktrace backend (not installed;
the `noop`, `basic` and `addr2line` backends are built), and the Windows
backends of locale and stacktrace.

**Excluded tests.** The row's ctest `-E` leaves out 41 tests. Each of them
also fails under the clang-19 control:

- 30 do not build (ctest "Not Run"); the shim does not read their b2
  target requirements:
  - exception's `/boost//thread` link and `<exception-handling>off`
    tests (8);
  - date_time's serialization tests and `testtime_formatters` (4);
  - iostreams' Windows and deprecated tests (2);
  - thread's `tss_null`, `test_9079_b`, `test_9192` and `test_13561` (4);
  - serialization's hashed containers (6, `BOOST_HAS_HASH` not defined);
  - Boost.Test's shared-library tests and `sync_access_test` (3), and its
    three `run-fail` tests.
- 11 fail at run time:
  - thread `test_5502` and the two executor examples (3);
  - date_time's facet and time-zone tests (3; data files and locales);
  - Boost.Test's output-pattern tests (4);
  - stacktrace's test (Release build, no debug info).

The control ran without the `-E` list (that run produced it): 1327 of 1368
pass, and the 41 failures are exactly the excluded tests.

**System Boost is not used.** The host has Boost 1.83 in
`/usr/include/boost`. A `-M` of `libs/filesystem/test/path_test.cpp` lists
494 dependencies: 160 in-tree Boost headers under `.oss/src/boost`, and 0
under `/usr/include/boost`.

## Results

- **TUs** are the build-phase compiles. CMake probes are excluded.
- **Wall** is the whole `oss-build.sh` run (configure, build and test),
  taken from `report.json`.
- **Σ TU wall** and **Σ TU CPU** sum the wrapper's per-TU compile
  times. Links are excluded.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

Each build ran once, from an empty build directory, on a host shared with
other harness builds.

| Build | TUs ok / failed | Links ok / failed | `.a` | Tests (ctest) | Wall | Σ TU wall | Σ TU CPU | Peak TU RSS |
|---|---|---|---|---|---|---|---|---|
| clang++-19 (control) | 1898 / 19 | 1215 / 12 | 29 | 1327/1368 (the 41 excluded fail) | 6201 s † | 5661 s | 3041 s | 1903 MB (`json/test/parse_into.cpp`) |
| kiln, first (`5273344dd`) | 1892 / 25 | 1213 / 12 | 28 | 1323/1368 | 12625 s | 15556 s | 4972 s | 1703 MB (`parse_into.cpp`) |
| kiln, final (`536392274`) | 1898 / 19 | 1215 / 12 | 29 | 1326/1327 (41 excluded) | 18267 s ‡ | 22080 s ‡ | 5573 s | 1700 MB (`parse_into.cpp`) |

† The control's wall includes the clone. The kiln runs reused the source.

‡ The final build ran at a host load average of about 15 (other agents'
builds), so its wall times are not comparable with the other two. CPU
time is the comparable measure.

### By library

First → final kiln build. A cell without an arrow is the same in both
builds and in the control. Tests count ctest results (the first build ran
all 1368; the final build ran 1327).

| Library | Library TUs ok/failed | Test TUs ok/failed | Links ok/failed | Tests pass/fail |
|---|---|---|---|---|
| atomic | 1/0 | 20/0 | 19/0 | 22/0 |
| charconv | 2/0 | 37/0 | 35/0 | 35/0 |
| chrono | 3/0 | 1/0 | 1/0 | 1/0 |
| container | 6/0 | 69/0 | 69/0 | 69/0 |
| context | 2/0 | 5/0 | 5/0 | 5/0 |
| date_time | 1/0 | 49/1 | 46/3 | 47/7 → 47/0 |
| exception | 0/0 | 46/4 | 22/4 | 25/9 → 26/0 |
| fiber | 19/0 | 32/0 | 32/0 | 32/0 |
| filesystem | 9/0 | 32/0 | 28/0 | 29/0 |
| iostreams | 7/0 | 39/2 | 39/0 | 39/2 → 39/0 |
| json | 3/0 | 52/0 | 4/0 | 3/1 → 4/0 |
| locale | 35/0 | 39/1 → 40/0 | 38/0 → 39/0 | 34/1 → 35/0 |
| log | 43/0 | 58/0 | 54/0 | 55/0 |
| nowide | 6/0 | 24/0 | 23/0 | 22/0 |
| program_options | 11/0 | 14/0 | 14/0 | 14/0 |
| random | 1/0 | 12/0 | 12/0 | 12/0 |
| regex | 0/0 | 28/1 | 22/0 | 20/0 |
| serialization | 48/0 | 207/6 | 132/0 | 132/6 → 131/1 |
| stacktrace | 3/0 | 5/0 | 3/0 | 2/1 → 2/0 |
| system | 0/0 | 182/0 | 160/0 | 161/0 |
| test | 42/0 | 60/2 | 56/4 | 53/10 → 53/0 |
| thread | 7/0 | 395/4 → 396/3 | 392/1 → 393/1 | 431/8 → 432/0 |
| timer | 2/0 | 5/0 | 4/0 | 4/0 |
| url | 132/0 | 90/0 | 3/0 | 76/0 |
| wave | 8/4 → 12/0 | 0/0 | 0/0 | 0/0 |

The remaining failed TUs and links are the 30 excluded tests that do not
build. Most "→ x/0" changes in the test column are the 41 exclusions. The
kiln-specific changes are json, locale, exception's `exception_fail`,
thread's `make_ready_future_pass` and wave's library (clusters 1 to 5).

### Tests

The one final failure is `run-serialization-test_class_info_load_xml_archive`.
It reads the archive that `test_class_info_save_xml_archive` writes, and
under `ctest -j 4` it started first and found no file. Run again in order
(`ctest -R 'class_info_(save|load)_xml'`), both pass. b2 runs a Jamfile's
tests in order, and the shim does not model that ordering, so this is a
harness flake that clang can hit too. It passed in the control and in the
first kiln build.

### Performance

Over the 1898 TUs that both compilers build (`oss-report.py --perf`, CPU
time):

| | kiln | clang++-19 | kiln / clang |
|---|---|---|---|
| Σ TU CPU, final | 5515 s | 3017 s | **1.83x** (median per TU 1.77x) |
| Σ TU CPU, first build | 4909 s | 3002 s | 1.64x (median 1.56x) |
| Σ TU peak RSS | | | 0.45x (median 0.42x) |
| Peak TU RSS | 1700 MB | 1903 MB | 0.89x (`json/test/parse_into.cpp`) |
| Max TU CPU | 122.1 s | 93.2 s | 1.31x (`parse_into.cpp`) |

`oss-report.py --perf` gives a geomean of 1.78x on time and 0.44x on RSS
over the 1546 distinct sources. Kiln uses less memory than clang on almost
every TU.

**Perf finding (recorded, not fixed):** the TU time ratio is above 1.2x.

- The ratio is highest where clang is fast. Boost.Thread's small tests
  take clang about 1.5 s and kiln 4.5 to 6 s. They spend that time in
  the frontend instantiating `<boost/thread.hpp>`:
  - `thread/test/test_5542_1.cpp` is 3.84x: 6.1 s against 1.6 s.
  - `thread/example/producer_consumer2.cpp` is 3.42x.
  - `thread/example/mutex.cpp` is 3.38x.
- The heavy TUs are near clang: `parse_into.cpp` is 1.31x and the container
  tests are 1.21x.
- Single-TU measurement on the first build, at
  `thread/test/sync/futures/promise/move_assign_pass.cpp`:
  - Full compile: kiln 28.4 s and 98 MB, clang++-19 7.6 s and 244 MB.
  - With `-fsyntax-only`: kiln 31.7 s, clang 7.0 s.
  - So the cost is in the frontend, not code generation.
  - Both compilers took that TU's `argv` from `compile.jsonl`, with
    `/usr/bin/time -f "%e s %M KB"`, the harness `kiln++` against
    `/usr/bin/clang++-19`.
  - The host was loaded, so re-measure on a quiet host before acting on
    the number.

### Heaviest TUs

Final kiln build against the control, by clang time. RSS is in MB and time
is CPU seconds.

| TU | clang s | kiln s | time x | clang MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `json/test/parse_into.cpp` | 93.2 | 122.1 | 1.31 | 1903 | 1700 | 0.89 |
| `json/test/basic_parser.cpp` | 40.9 | 44.5 | 1.09 | 596 | 516 | 0.87 |
| `container/test/flat_map_test.cpp` | 21.5 | 26.0 | 1.21 | 427 | 354 | 0.83 |
| `container/test/devector_test.cpp` | 18.2 | 22.1 | 1.21 | 488 | 415 | 0.85 |
| `container/test/flat_map_adaptor_test.cpp` | 15.1 | 18.5 | 1.22 | 371 | 302 | 0.81 |

## Clusters

The clusters come from the first build. They are counted where kiln fails
and clang passes: 6 TUs and 4 tests.

| # | Signature (first diagnostic) | Library | TUs / tests | Status | Repro |
|---|---|---|---|---|---|
| 1 | `use 'template' keyword to treat 'value' as a dependent template name` (spirit classic phoenix `operators.hpp:404`, `higher_rank`: `if_c<rank<T0>::value < rank<T1>::value, ...>`) | wave | 4 TUs (its library) | fixed, `cb5cac611` | `tests/conformance/temp.names/dependent-member-less-than-before-scope.cpp` |
| 2 | runtime: json `value_from` / `value_to` wrong (an init-list constructor's `{a, b}` element list was rebuilt as one element in a template; [dcl.init.list]/3.7) | json | 1 test | fixed, `3301cf89d` (WP-801 `4f7b87c3e` fixed the same cause independently; merged into one check) | `tests/conformance/dcl.init.list/init-list-ctor-element-list-in-template.cpp` |
| 3 | `static assertion failed due to requirement 'is_same_v<char, wchar_t>'` (explicit template arguments not substituted into a leading return type before the CWG1391 conversion check, so SFINAE came too late) | locale | 1 TU, 1 test | fixed, `e7bb95dde` | `tests/conformance/temp.deduct.call/explicit-args-return-sfinae-before-nondep-conversion.cpp` |
| 4 | compile-fail test compiles: throwing an abstract class, or one with a protected destructor, was accepted ([except.throw]/5, [class.abstract]/3) | exception | 1 test | fixed, `0c96f0962` (`c99e674ce` merges it with WP-799's destructor odr-use) | `tests/conformance/expr.throw/abstract-and-inaccessible-dtor.cpp` |
| 5 | `source file is not valid UTF-8` (`make_ready_future_pass.cpp:129`) | thread | 1 TU, 1 test | fixed by WP-803 `2e5a0fd97`, merged in | `tests/conformance/lex.phases/invalid-utf8-in-comment-accepted-run.cpp` |

## What remains

- **Trailing return types** (cluster 3's other half), with an xfail
  follow-up test:
  - With explicit template arguments, a trailing return type is
    substituted only with the deduced arguments. That happens after the
    CWG1391 conversion check has already instantiated the parameter's
    class.
  - The fix substitutes it with the explicit arguments first, as the
    leading form now does (`crates/kiln_sema/src/templ/deduce_call.rs`
    `subst_explicit_param_types`). The trailing form must be parsed in
    the parameters' scope before it can be substituted.
  - xfail test (`xfail(kiln): WP-804 follow-up`, passes under
    clang++-19):
    `tests/conformance/temp.deduct.call/explicit-args-trailing-return-sfinae-before-nondep-conversion.cpp`.
- **Harness:** the shim does not model b2's in-order test runs. That
  makes the `class_info` save/load pairs order-dependent under `-j 4`.
  It also leaves the 41 excluded tests out of the comparison.
- **Perf:** a TU CPU ratio of 1.83x, highest on small Boost.Thread TUs
  (Performance, above).

## Disk

- `.oss/`: 2.0 GB.
  - 882 MB of Boost source with submodules (shared with `boost_meta`).
  - 1.1 GB for the final kiln build directory.
  - The clang build directory was deleted after its numbers were
    recorded. Its `report.json` and `compile.jsonl` were restored for
    `oss-report.py`.
- `$CARGO_TARGET_DIR` (`.build/target-wp-804`): 16 GB.
- The volume had 150 GB free at the end.
