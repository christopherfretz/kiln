# duckdb with kiln++ (WP-799)

duckdb **v1.5.6** (tag `v1.5.6`), built with kiln++ as `CMAKE_CXX_COMPILER`
through duckdb's own CMake build (the `unittest` and `shell` targets, with
the default in-tree extensions `core_functions` and `parquet`), then
duckdb's `unittest` (Catch2 plus the default sqllogictest files). The
control is a clang++-19 build of the same tree with the same flags. Host:
aarch64 Linux, libstdc++ 14, C++17 (the project row), default
`-fclang-compat`, Release (`-O3`). Date: 2026-11-07.

**Result: parity.** All 421 TUs compile, both targets link, and `unittest`
gives the control's result: 4615 of 4628 test cases pass and the same 13
fail under both compilers, for reasons of the host (below). kiln needed
five fixes: two for valid code it rejected or could not lower (a
statement expression with a class lvalue result, the NEON structure
loads/stores), one for a valid TU it rejected (a `const` lost in a
conditional), and two **miscompiles** found by the tests (a double free
from a wrong NRVO decision, and the GNU `mode` attribute ignored, which
made the bundled mbedtls bignum arithmetic 32-bit).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh duckdb --compiler clang
scripts/oss/oss-build.sh duckdb
scripts/oss/oss-report.py --perf duckdb
```

In `.oss/build/duckdb/<kiln|clang>/` (the wrapper `scripts/oss/kiln-cxx`
applies `ulimit -v 3000000` and `timeout 600`):

```sh
cmake --fresh -S .oss/src/duckdb -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_UNITTESTS=ON -DBUILD_SHELL=ON
nice -n 10 make -k unittest shell     # draws from the shared jobserver
DUCKDB_TEST_MAX_THREADS=4 SUMMARIZE_FAILURES=1 test/unittest
```

duckdb's CMake builds most of the library as unity TUs (`ub_*.cpp`,
generated in the build tree), so a TU here is often a whole directory of
sources. `unittest` with no arguments runs every non-slow test case
(4628, of which 372 are skipped by their own `require` lines).

### The 13 failures of both builds

The same test cases fail under clang and kiln, for reasons of the host
and of the targets built, not of the compiler:

- 9 sqllogictest files under `test/extension/` (`load_extension.test`,
  `concurrent_load_extension.test`, `load_test_alias.test`, …) and the
  Catch2 case in `test/extension/test_remote_optimizer.cpp` load the demo
  loadable extensions (`loadable_extension_demo.duckdb_extension`, …),
  which are separate CMake targets not built here: `Extension "…" not
  found`.
- `test/sql/settings/reset/reset_threads.test` expects the default thread
  count to be 4; the host has 24 cores.
- "On Disk DB File Name Case Preserved"
  (`test/api/capi/test_starting_database.cpp`) and "Test the database
  instance cache with case-insensitive FS"
  (`test/api/capi/test_capi_instance_cache.cpp`) expect a case-sensitive
  file system on Linux; the build tree is on a case-insensitive host
  volume.

## Results

- **TUs** are duckdb's build-phase compiles (421, CMake probes excluded).
- **Wall** is the `oss-build.sh` run (configure, build and test), from
  `report.json`. Builds draw from a jobserver shared with other harness
  builds, so walls of different runs are not comparable; the summed TU
  times below are.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

| Build | TUs ok / failed | Links | Tests | Wall | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (`45ccf9433`) | 348 / 2 (71 not attempted) | 0 / 0 | not built | 1544 s | 485 MB |
| clang++-19 (control) | 421 / 0 | 2 / 0 | 4615 / 4628 (the 13 above) | 1880 s | 817 MB (`src/main/ub_duckdb_main.cpp`) |
| kiln, after clusters 1–3 (`8aedbe73a`) | 421 / 0 | 2 / 0 | aborted after 92 cases (`double free`) | 1468 s | 496 MB |
| kiln, after cluster 4 (`f1bcb5676`) | 421 / 0 | 2 / 0 | hung in "Test that we can verify a signature" (killed) | — | — |
| **kiln, final** (`579984270`) | **421 / 0** | **2 / 0** | **4615 / 4628, the same 13** | 1511 s | **496 MB** (`extension/core_functions/aggregate/distributive/ub_duckdb_core_functions_distributive.cpp`) |

In the first build the two rejected TUs (`third_party/zstd/compress/zstd_lazy.cpp`
and the core_functions `generic` unity TU) are in libraries that both
targets link, so `make -k` linked nothing and stopped attempting the TUs
behind them. With the NRVO fix and without the mbedtls test cases
(`~[mbedtls]`), `unittest` already gave the control's result; the
`mode` fix then made the mbedtls cases pass too.

### Performance

`oss-report.py --perf duckdb` matches TUs by source path, so it pairs only
the 236 TUs outside the unity files (the generated `ub_*.cpp` live in each
build tree; the tool's numbers for those 236: geomean time 1.31x, RSS
0.49x). Over all 421 TUs, pairing each unity file by its path in the
build tree (wall time, peak RSS of the compiler and its backend):
geomean kiln/clang-19 time **1.19x**, RSS **0.45x**; total 1849 s vs
1745 s; peak TU RSS 496 MB vs 817 MB.

| heaviest TUs (by clang-19 time) | clang-19 s | kiln s | time x | clang-19 MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `src/common/vector_operations/is_distinct_from.cpp` | 29.8 | 28.4 | 0.95 | 372 | 277 | 0.75 |
| `extension/core_functions/aggregate/distributive/ub_duckdb_core_functions_distributive.cpp` (build tree) | 27.3 | 30.6 | 1.12 | 669 | 496 | 0.74 |
| `extension/core_functions/scalar/date/ub_duckdb_core_functions_date.cpp` (build tree) | 26.7 | 28.2 | 1.05 | 530 | 322 | 0.61 |
| `extension/core_functions/aggregate/holistic/ub_duckdb_core_functions_holistic.cpp` (build tree) | 25.1 | 27.7 | 1.10 | 686 | 414 | 0.60 |
| `src/function/cast/ub_duckdb_func_cast.cpp` (build tree) | 23.0 | 23.8 | 1.04 | 629 | 481 | 0.77 |

No TU uses more memory under kiln than under clang (the highest
kiln/clang RSS ratio is 0.81, `src/execution/expression_executor/ub_duckdb_expression_executor.cpp`),
so there is no RSS finding. On time, 96 of the 335 TUs that take clang-19
at least 0.5 s are over 1.2x. The worst five:

- `third_party/brotli/enc/cluster.cpp`: time 1.3 / 0.6 s = 2.14x, RSS 71 / 137 MB = 0.52x
- `src/function/scalar/map/ub_duckdb_function_map.cpp` (build tree): time 4.1 / 2.1 s = 1.93x, RSS 87 / 237 MB = 0.37x
- `tools/shell/linenoise/terminal.cpp`: time 1.1 / 0.6 s = 1.88x, RSS 68 / 138 MB = 0.49x
- `third_party/snappy/snappy.cc`: time 1.4 / 0.7 s = 1.87x, RSS 71 / 141 MB = 0.51x
- `third_party/brotli/enc/compress_fragment_two_pass.cpp`: time 1.2 / 0.7 s = 1.75x, RSS 71 / 136 MB = 0.52x

Perf findings (time, logged, not reduced): small TUs take about 0.7 s
longer under kiln (as in z3); a few mid-size template-heavy unity TUs are
1.5–2x (the largest absolute gap is `test/api/udf_function/ub_test_api_udf_function.cpp`,
16.4 s vs 11.1 s). Reproduce with tag `v1.5.6`, from the worktree
(`$KILN`), with kiln++ at `579984270`:

```text
kiln: cd $KILN/.oss/build/duckdb/kiln/src/function/scalar/map && kiln++ -DDUCKDB -DDUCKDB_BUILD_LIBRARY -DDUCKDB_MAIN_LIBRARY -I$KILN/.oss/src/duckdb/src/include -I$KILN/.oss/src/duckdb/third_party/fsst -I$KILN/.oss/src/duckdb/third_party/fmt/include -I$KILN/.oss/src/duckdb/third_party/hyperloglog -I$KILN/.oss/src/duckdb/third_party/fastpforlib -I$KILN/.oss/src/duckdb/third_party/skiplist -I$KILN/.oss/src/duckdb/third_party/ska_sort -I$KILN/.oss/src/duckdb/third_party/fast_float -I$KILN/.oss/src/duckdb/third_party/re2 -I$KILN/.oss/src/duckdb/third_party/miniz -I$KILN/.oss/src/duckdb/third_party/utf8proc/include -I$KILN/.oss/src/duckdb/third_party/concurrentqueue -I$KILN/.oss/src/duckdb/third_party/pcg -I$KILN/.oss/src/duckdb/third_party/pdqsort -I$KILN/.oss/src/duckdb/third_party/tdigest -I$KILN/.oss/src/duckdb/third_party/mbedtls/include -I$KILN/.oss/src/duckdb/third_party/httplib -I$KILN/.oss/src/duckdb/third_party/jaro_winkler -I$KILN/.oss/src/duckdb/third_party/vergesort -I$KILN/.oss/src/duckdb/third_party/yyjson/include -I$KILN/.oss/src/duckdb/third_party/zstd/include -I$KILN/.oss/src/duckdb/third_party/jemalloc/include -DDUCKDB_ENABLE_JEMALLOC -O3 -DNDEBUG -O3 -DNDEBUG -std=c++17 -fPIC -MD -MT src/function/scalar/map/CMakeFiles/duckdb_function_map.dir/ub_duckdb_function_map.cpp.o -MF CMakeFiles/duckdb_function_map.dir/ub_duckdb_function_map.cpp.o.d -o CMakeFiles/duckdb_function_map.dir/ub_duckdb_function_map.cpp.o -c $KILN/.oss/build/duckdb/kiln/src/function/scalar/map/ub_duckdb_function_map.cpp
  wall 4.109 s, peak RSS 85 MB
clang: cd $KILN/.oss/build/duckdb/clang/src/function/scalar/map && clang++-19 -DDUCKDB -DDUCKDB_BUILD_LIBRARY -DDUCKDB_MAIN_LIBRARY -I$KILN/.oss/src/duckdb/src/include -I$KILN/.oss/src/duckdb/third_party/fsst -I$KILN/.oss/src/duckdb/third_party/fmt/include -I$KILN/.oss/src/duckdb/third_party/hyperloglog -I$KILN/.oss/src/duckdb/third_party/fastpforlib -I$KILN/.oss/src/duckdb/third_party/skiplist -I$KILN/.oss/src/duckdb/third_party/ska_sort -I$KILN/.oss/src/duckdb/third_party/fast_float -I$KILN/.oss/src/duckdb/third_party/re2 -I$KILN/.oss/src/duckdb/third_party/miniz -I$KILN/.oss/src/duckdb/third_party/utf8proc/include -I$KILN/.oss/src/duckdb/third_party/concurrentqueue -I$KILN/.oss/src/duckdb/third_party/pcg -I$KILN/.oss/src/duckdb/third_party/pdqsort -I$KILN/.oss/src/duckdb/third_party/tdigest -I$KILN/.oss/src/duckdb/third_party/mbedtls/include -I$KILN/.oss/src/duckdb/third_party/httplib -I$KILN/.oss/src/duckdb/third_party/jaro_winkler -I$KILN/.oss/src/duckdb/third_party/vergesort -I$KILN/.oss/src/duckdb/third_party/yyjson/include -I$KILN/.oss/src/duckdb/third_party/zstd/include -I$KILN/.oss/src/duckdb/third_party/jemalloc/include -DDUCKDB_ENABLE_JEMALLOC -O3 -DNDEBUG -O3 -DNDEBUG -std=c++17 -fPIC -MD -MT src/function/scalar/map/CMakeFiles/duckdb_function_map.dir/ub_duckdb_function_map.cpp.o -MF CMakeFiles/duckdb_function_map.dir/ub_duckdb_function_map.cpp.o.d -o CMakeFiles/duckdb_function_map.dir/ub_duckdb_function_map.cpp.o -c $KILN/.oss/build/duckdb/clang/src/function/scalar/map/ub_duckdb_function_map.cpp
  wall 2.128 s, peak RSS 231 MB
kiln: cd $KILN/.oss/build/duckdb/kiln/test/api/udf_function && kiln++ -DDUCKDB_BUILD_LIBRARY -DDUCKDB_MAJOR_VERSION=1 -DDUCKDB_MINOR_VERSION=5 -DDUCKDB_PATCH_VERSION=6 -DGENERATED_EXTENSION_HEADERS=1 -I$KILN/.oss/src/duckdb/src/include -I$KILN/.oss/src/duckdb/third_party/fsst -I$KILN/.oss/src/duckdb/third_party/fmt/include -I$KILN/.oss/src/duckdb/third_party/hyperloglog -I$KILN/.oss/src/duckdb/third_party/fastpforlib -I$KILN/.oss/src/duckdb/third_party/skiplist -I$KILN/.oss/src/duckdb/third_party/ska_sort -I$KILN/.oss/src/duckdb/third_party/fast_float -I$KILN/.oss/src/duckdb/third_party/re2 -I$KILN/.oss/src/duckdb/third_party/miniz -I$KILN/.oss/src/duckdb/third_party/utf8proc/include -I$KILN/.oss/src/duckdb/third_party/concurrentqueue -I$KILN/.oss/src/duckdb/third_party/pcg -I$KILN/.oss/src/duckdb/third_party/pdqsort -I$KILN/.oss/src/duckdb/third_party/tdigest -I$KILN/.oss/src/duckdb/third_party/mbedtls/include -I$KILN/.oss/src/duckdb/third_party/httplib -I$KILN/.oss/src/duckdb/third_party/jaro_winkler -I$KILN/.oss/src/duckdb/third_party/vergesort -I$KILN/.oss/src/duckdb/third_party/yyjson/include -I$KILN/.oss/src/duckdb/third_party/zstd/include -I$KILN/.oss/src/duckdb/third_party/jemalloc/include -I$KILN/.oss/src/duckdb/test/../third_party/catch -I$KILN/.oss/src/duckdb/test/../third_party/sqlite/include -I$KILN/.oss/src/duckdb/test/include -I$KILN/.oss/build/duckdb/kiln/codegen/include -DDUCKDB_ENABLE_JEMALLOC -O3 -DNDEBUG -O3 -DNDEBUG -std=c++17 -fPIC -MD -MT test/api/udf_function/CMakeFiles/test_api_udf_function.dir/ub_test_api_udf_function.cpp.o -MF CMakeFiles/test_api_udf_function.dir/ub_test_api_udf_function.cpp.o.d -o CMakeFiles/test_api_udf_function.dir/ub_test_api_udf_function.cpp.o -c $KILN/.oss/build/duckdb/kiln/test/api/udf_function/ub_test_api_udf_function.cpp
  wall 16.366 s, peak RSS 213 MB
clang: cd $KILN/.oss/build/duckdb/clang/test/api/udf_function && clang++-19 -DDUCKDB_BUILD_LIBRARY -DDUCKDB_MAJOR_VERSION=1 -DDUCKDB_MINOR_VERSION=5 -DDUCKDB_PATCH_VERSION=6 -DGENERATED_EXTENSION_HEADERS=1 -I$KILN/.oss/src/duckdb/src/include -I$KILN/.oss/src/duckdb/third_party/fsst -I$KILN/.oss/src/duckdb/third_party/fmt/include -I$KILN/.oss/src/duckdb/third_party/hyperloglog -I$KILN/.oss/src/duckdb/third_party/fastpforlib -I$KILN/.oss/src/duckdb/third_party/skiplist -I$KILN/.oss/src/duckdb/third_party/ska_sort -I$KILN/.oss/src/duckdb/third_party/fast_float -I$KILN/.oss/src/duckdb/third_party/re2 -I$KILN/.oss/src/duckdb/third_party/miniz -I$KILN/.oss/src/duckdb/third_party/utf8proc/include -I$KILN/.oss/src/duckdb/third_party/concurrentqueue -I$KILN/.oss/src/duckdb/third_party/pcg -I$KILN/.oss/src/duckdb/third_party/pdqsort -I$KILN/.oss/src/duckdb/third_party/tdigest -I$KILN/.oss/src/duckdb/third_party/mbedtls/include -I$KILN/.oss/src/duckdb/third_party/httplib -I$KILN/.oss/src/duckdb/third_party/jaro_winkler -I$KILN/.oss/src/duckdb/third_party/vergesort -I$KILN/.oss/src/duckdb/third_party/yyjson/include -I$KILN/.oss/src/duckdb/third_party/zstd/include -I$KILN/.oss/src/duckdb/third_party/jemalloc/include -I$KILN/.oss/src/duckdb/test/../third_party/catch -I$KILN/.oss/src/duckdb/test/../third_party/sqlite/include -I$KILN/.oss/src/duckdb/test/include -I$KILN/.oss/build/duckdb/clang/codegen/include -DDUCKDB_ENABLE_JEMALLOC -O3 -DNDEBUG -O3 -DNDEBUG -std=c++17 -fPIC -MD -MT test/api/udf_function/CMakeFiles/test_api_udf_function.dir/ub_test_api_udf_function.cpp.o -MF CMakeFiles/test_api_udf_function.dir/ub_test_api_udf_function.cpp.o.d -o CMakeFiles/test_api_udf_function.dir/ub_test_api_udf_function.cpp.o -c $KILN/.oss/build/duckdb/clang/test/api/udf_function/ub_test_api_udf_function.cpp
  wall 11.092 s, peak RSS 491 MB
kiln: cd $KILN/.oss/build/duckdb/kiln/extension/parquet && kiln++ -DDUCKDB_BUILD_LIBRARY '-DEXT_VERSION_PARQUET="v1.5.6"' -I$KILN/.oss/src/duckdb/src/include -I$KILN/.oss/src/duckdb/third_party/fsst -I$KILN/.oss/src/duckdb/third_party/fmt/include -I$KILN/.oss/src/duckdb/third_party/hyperloglog -I$KILN/.oss/src/duckdb/third_party/fastpforlib -I$KILN/.oss/src/duckdb/third_party/skiplist -I$KILN/.oss/src/duckdb/third_party/ska_sort -I$KILN/.oss/src/duckdb/third_party/fast_float -I$KILN/.oss/src/duckdb/third_party/re2 -I$KILN/.oss/src/duckdb/third_party/miniz -I$KILN/.oss/src/duckdb/third_party/utf8proc/include -I$KILN/.oss/src/duckdb/third_party/concurrentqueue -I$KILN/.oss/src/duckdb/third_party/pcg -I$KILN/.oss/src/duckdb/third_party/pdqsort -I$KILN/.oss/src/duckdb/third_party/tdigest -I$KILN/.oss/src/duckdb/third_party/mbedtls/include -I$KILN/.oss/src/duckdb/third_party/httplib -I$KILN/.oss/src/duckdb/third_party/jaro_winkler -I$KILN/.oss/src/duckdb/third_party/vergesort -I$KILN/.oss/src/duckdb/third_party/yyjson/include -I$KILN/.oss/src/duckdb/third_party/zstd/include -I$KILN/.oss/src/duckdb/extension/parquet/include -I$KILN/.oss/src/duckdb/extension/parquet/../../third_party/lz4 -I$KILN/.oss/src/duckdb/extension/parquet/../../third_party/parquet -I$KILN/.oss/src/duckdb/extension/parquet/../../third_party/thrift -I$KILN/.oss/src/duckdb/extension/parquet/../../third_party/snappy -I$KILN/.oss/src/duckdb/extension/parquet/../../third_party/brotli/include -O3 -DNDEBUG -O3 -DNDEBUG -std=c++17 -fPIC -MD -MT extension/parquet/CMakeFiles/parquet_extension.dir/__/__/third_party/brotli/enc/cluster.cpp.o -MF CMakeFiles/parquet_extension.dir/__/__/third_party/brotli/enc/cluster.cpp.o.d -o CMakeFiles/parquet_extension.dir/__/__/third_party/brotli/enc/cluster.cpp.o -c $KILN/.oss/src/duckdb/third_party/brotli/enc/cluster.cpp
  wall 1.295 s, peak RSS 70 MB
clang: cd $KILN/.oss/build/duckdb/clang/extension/parquet && clang++-19 -DDUCKDB_BUILD_LIBRARY '-DEXT_VERSION_PARQUET="v1.5.6"' -I$KILN/.oss/src/duckdb/src/include -I$KILN/.oss/src/duckdb/third_party/fsst -I$KILN/.oss/src/duckdb/third_party/fmt/include -I$KILN/.oss/src/duckdb/third_party/hyperloglog -I$KILN/.oss/src/duckdb/third_party/fastpforlib -I$KILN/.oss/src/duckdb/third_party/skiplist -I$KILN/.oss/src/duckdb/third_party/ska_sort -I$KILN/.oss/src/duckdb/third_party/fast_float -I$KILN/.oss/src/duckdb/third_party/re2 -I$KILN/.oss/src/duckdb/third_party/miniz -I$KILN/.oss/src/duckdb/third_party/utf8proc/include -I$KILN/.oss/src/duckdb/third_party/concurrentqueue -I$KILN/.oss/src/duckdb/third_party/pcg -I$KILN/.oss/src/duckdb/third_party/pdqsort -I$KILN/.oss/src/duckdb/third_party/tdigest -I$KILN/.oss/src/duckdb/third_party/mbedtls/include -I$KILN/.oss/src/duckdb/third_party/httplib -I$KILN/.oss/src/duckdb/third_party/jaro_winkler -I$KILN/.oss/src/duckdb/third_party/vergesort -I$KILN/.oss/src/duckdb/third_party/yyjson/include -I$KILN/.oss/src/duckdb/third_party/zstd/include -I$KILN/.oss/src/duckdb/extension/parquet/include -I$KILN/.oss/src/duckdb/extension/parquet/../../third_party/lz4 -I$KILN/.oss/src/duckdb/extension/parquet/../../third_party/parquet -I$KILN/.oss/src/duckdb/extension/parquet/../../third_party/thrift -I$KILN/.oss/src/duckdb/extension/parquet/../../third_party/snappy -I$KILN/.oss/src/duckdb/extension/parquet/../../third_party/brotli/include -O3 -DNDEBUG -O3 -DNDEBUG -std=c++17 -fPIC -MD -MT extension/parquet/CMakeFiles/parquet_extension.dir/__/__/third_party/brotli/enc/cluster.cpp.o -MF CMakeFiles/parquet_extension.dir/__/__/third_party/brotli/enc/cluster.cpp.o.d -o CMakeFiles/parquet_extension.dir/__/__/third_party/brotli/enc/cluster.cpp.o -c $KILN/.oss/src/duckdb/third_party/brotli/enc/cluster.cpp
  wall 0.604 s, peak RSS 134 MB
```

## Clusters

| # | Signature (first diagnostic) | TUs | Fix | Repro |
|---|---|---|---|---|
| 1 | `a statement expression with a class lvalue result is not supported yet` (`arm_neon.h`'s `vld2q_u16` macro) | 1 (`third_party/zstd/compress/zstd_lazy.cpp`) | `6ad563ead` | `tests/conformance/ext.gnu.stmt.expr/class-lvalue-result-run.cpp` |
| 2 | `code generation for '__builtin_neon_vld2q_v' is not supported yet` (and `vst2..4`, `vsli_n`), behind cluster 1 | the same TU | `b0b8aba45` | `tests/conformance/target.neon/ldn-stn-sli-run.cpp`, `ir-ldn-stn-sli-19.cpp` |
| 3 | `non-const lvalue reference to type 'vector<...>' cannot bind to a temporary` (`auto &r = b ? f() : cref;`: the conditional lost `const`) | 1 (core_functions `scalar/generic` unity TU, `system_functions.cpp`) | `8aedbe73a` | `tests/conformance/expr.cond/class-prvalue-unified-const-run.cpp` |
| 4 | **miscompile**: `unittest` aborted with `double free` in "Test errors in C API" (`Catalog::TryLookupEntry`: `return {nullptr, nullptr, ErrorData(e)};` beside `return result;` kept `result` as the NRVO variable) | `src/catalog/catalog.cpp` (in a unity TU) | `f1bcb5676` | `tests/conformance/class.copy.elision/nrvo-disabled-by-braced-return-run.cpp` |
| 5 | **miscompile**: `unittest` spun forever in "Test that we can verify a signature" (mbedtls's `typedef unsigned int mbedtls_t_udbl __attribute__((mode(TI)));` was 32 bits: `mode` was ignored, so `mbedtls_mpi_core_mla` truncated every product) | `third_party/mbedtls/library/bignum_core.cpp` (found by swapping object files between the builds, then functions within it) | `579984270` | `tests/conformance/ext.gnu.mode/mode-run.cpp`, `mode-errors.cpp` |

The spec sentences are in `spec/landed/sema/WP-799.md`. No cluster was a
bug outside kiln. Left as a follow-up: a `mode` attribute on a
declaration of dependent type is dropped
(`tests/conformance/ext.gnu.mode/mode-dependent-run.cpp`, xfail; duckdb
does not need it).

Diagnostics only (noted, not fixed): kiln warns once where clang does
not, `variable length arrays in C++ are a Clang extension
[-Wvla-cxx-extension]` on `asm volatile ("" : : "m" (*(char (*)[len])
buf) :)` in `third_party/mbedtls/library/platform_util.cpp:127` (clang 19
does not warn on that line).

## Disk

The kiln build tree is 455 MB (the control's, 435 MB, was deleted once its
numbers were recorded; its `report.json`, `compile.jsonl`, `build.log` and
`test.log` are kept). The source checkout is 402 MB.

## What remains

- **Variants not run**: Debug, the amalgamation build, the loadable demo
  extensions (which the 9 extension tests need), the other in-tree
  extensions (`json`, `icu`, `tpch`, …), the Python and Node packages,
  and the slow sqllogictest files (`unittest "*"`).
- **Perf**: the compile-time gap on small TUs and on a few unity TUs above
  (no RSS finding).
- **Tooling**: `oss-report.py` pairs TUs by source path, which differs
  between the two build trees for generated unity sources, so its
  `--perf` for duckdb covers only 236 of the 421 TUs.
