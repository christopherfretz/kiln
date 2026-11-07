# rocksdb with kiln++ (WP-724)

rocksdb **v10.7.5** (tag `v10.7.5`), built with kiln++ as
`CMAKE_CXX_COMPILER` through rocksdb's own CMake build with its tests
enabled, and a bounded selection of its tests run with CTest. The control
is a clang++-19 build of the same tree with the same flags and the same
test filter. Host: aarch64 Linux, libstdc++ 14, C++20 (the project row),
default `-fclang-compat`. Date: 2026-11-07.

**Result: parity.** All 573 TUs compile, all 213 targets link, and the
2158 selected tests that are not disabled pass under both compilers. kiln
needed seven fixes, all for valid programs it rejected or could not
lower. No test failed at run time once everything built (no miscompile).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh rocksdb --compiler clang
scripts/oss/oss-build.sh rocksdb
scripts/oss/oss-report.py rocksdb
```

In `.oss/build/rocksdb/<kiln|clang>/` (the wrapper `scripts/oss/kiln-cxx`
applies `ulimit -v 3000000` and `timeout 600`):

```sh
cmake --fresh -S .oss/src/rocksdb -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Debug \
  -DCMAKE_CXX_FLAGS_DEBUG=-O1 -DCMAKE_C_FLAGS_DEBUG=-O1 -DWITH_TESTS=ON \
  -DWITH_GFLAGS=OFF -DFAIL_ON_WARNINGS=OFF -DWITH_BENCHMARK_TOOLS=OFF \
  -DWITH_TOOLS=OFF -DROCKSDB_BUILD_SHARED=OFF -DPORTABLE=1
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure -R "<filter>"   # 2163 tests, below
```

The configuration is Debug because rocksdb builds its tests only in a
Debug build (`WITH_TESTS` is a `CMAKE_DEPENDENT_OPTION` on it). It uses
`-O1` rather than `-O0` so the tests run fast. The shared library is off,
since the tests link the static one. `PORTABLE=1` keeps `-march=native`
out. On aarch64, rocksdb still adds `-march=armv8-a+crc+crypto`, so the
CRC32C (`util/crc32c_arm64.cc`) and NEON (xxhash) paths are built.

### Test filter

rocksdb's CMake registers every gtest case as a separate CTest test
(`gtest_discover_tests`). That is 17,437 tests in 208 binaries, and
several binaries run for longer than 3 minutes under clang. Others
crash (`SIGSEGV`) or fail under clang on this host as well (for example
`backup_engine_test`, `checkpoint_test`, `table_test`, `db_basic_test`,
`db_secondary_test` and `flush_job_test`). So the tracked run is bounded to 62 representative
binaries. Each one passes under clang and finishes in seconds when run
whole:

- **cache/**: `cache_reservation_manager_test`, `cache_test`,
  `compressed_secondary_cache_test`, `lru_cache_test`.
- **memtable/**: `skiplist_test`, `write_buffer_manager_test`.
- **options/**: `configurable_test`, `customizable_test`, `options_test`.
- **table/**: `block_fetcher_test`, `block_test`, `cleanable_test`,
  `cuckoo_table_builder_test`, `data_block_hash_index_test`,
  `full_filter_block_test`, `merger_test`,
  `partitioned_filter_block_test`.
- **util/**: `autovector_test`, `coding_test`, `compression_test`,
  `crc32c_test`, `defer_test`, `hash_test`, `heap_test`, `random_test`,
  `repeatable_thread_test`, `ribbon_test`, `slice_test`,
  `slice_transform_test`, `string_util_test`, `thread_list_test`,
  `thread_local_test`, `timer_queue_test`, `timer_test`,
  `udt_util_test`, `work_queue_test`.
- **monitoring/ and memory/**: `histogram_test`, `iostats_context_test`,
  `memory_test`, `statistics_test`.
- **db/**: `compaction_iterator_test`, `compaction_job_test`,
  `compaction_picker_test`, `db_blob_index_test`, `db_encryption_test`,
  `db_logical_block_size_cache_test`, `db_memtable_test`,
  `db_merge_operand_test`, `db_statistics_test`, `db_wide_basic_test`,
  `db_with_timestamp_compaction_test`, `db_write_test`, `dbformat_test`,
  `filename_test`, `log_test`,
  `merge_helper_test`, `range_del_aggregator_test`,
  `range_tombstone_fragmenter_test`, `version_builder_test`,
  `version_edit_test`, `wal_edit_test`, `write_batch_test`.

`crc32c_test` and `hash_test` run the code behind clusters 3 and 4 below,
and `db_write_test` contains cluster 7.

CTest names are gtest `Suite.Case` names with no binary in them, so the
filter is an anchored `-R` alternation of the 132 suite heads of those
binaries: `^(AutoVectorTest|Basic/HeapTest|…|bool/RetriableLogTest)\.`.
A parameterized head keeps its instantiation prefix. The full regex is the
`test` field of the `rocksdb` row in `scripts/oss/projects.toml`. A head
is spelled with its prefix because a bare head can be shared:
`db_iter_test`'s `DBIteratorTest` is also a suite of the slow
`db_iterator_test`, so `db_iter_test` is left out. Matched against the
`*_tests.cmake` lists of all 208 binaries, the filter selects exactly the
2163 cases of the 62 binaries. Five of them are disabled in the source
(`DISABLED_`), and CTest reports those as not run, which leaves 2158.
The test phase takes about 57 s under each compiler.

## Results

- **TUs** are rocksdb's build-phase compiles (573, CMake probes excluded).
- **Wall** is the `oss-build.sh` run (configure, build and test), from
  `report.json`. The first kiln build and the first clang build ran
  without tests.
- **Σ TU wall** sums the wrapper's per-TU compile wall times.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

Each build ran once, on a host shared with other harness builds.

| Build | TUs ok / failed | Links | Tests (ctest -R) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (`f415f77ee`) | 342 / 13 (218 not attempted) | 0 / 0 | — | 249 s | — | 233 MB |
| clang++-19 (control, no tests) | 573 / 0 | 213 / 0 | — | 181 s | — | 675 MB |
| clang++-19 (control) | 573 / 0 | 213 / 0 | 2158/2158 | 254 s | 1982 s | 678 MB (`db/db_compaction_test.cc`) |
| **kiln, final** (`c6161897f`) | **573 / 0** | **213 / 0** | **2158/2158** | 320 s | 2365 s | **490 MB** (`db/db_compaction_test.cc`) |

In the first kiln build, two of the rejected TUs (`util/crc32c.cc` and
`util/hash.cc`) are in `librocksdb.a`, so `make -k` linked nothing. Every
later TU of every test binary was not attempted.

kiln's summed compile time is **1.19×** clang's. Its peak RSS is
**0.72×** clang's. The slowest TU (`db/db_compaction_test.cc`) takes
28.3 s under kiln and 26.0 s under clang. No TU came near 2 GiB under
either compiler. The summed link wall (through the kiln++ driver: 630 s
vs 412 s) is not a compile measure.

## Clusters

| # | Signature (first diagnostic) | TUs | Fix | Repro |
|---|---|---|---|---|
| 1 | `no member named 'capacity' in 'struct LRUCacheOptions'` (`offsetof(LRUCacheOptions, capacity)`, a member of a base) | 1 (`cache/cache.cc`) | `8d8f86361` | `tests/conformance/ext.gnu.offsetof/base-class-member-run.cpp`, `base-class-member-diagnostics.cpp` |
| 2 | `redeclaration of using declaration` (`using B::f;` and `using D::f;` naming one member) | 6 (`utilities/transactions/*`, via `write_prepared_txn_db.h`) | `7f8318334` | `tests/conformance/namespace.udecl/p10-same-member-through-other-class-run.cpp`, `p10-class-redeclaration-same-class.cpp` |
| 3 | `use of undeclared identifier '__builtin_arm_chkfeat'` (clang 19's `arm_acle.h`) | 2 (`util/crc32c.cc`, `util/crc32c_arm64.cc`) | `2076cd055` | `tests/conformance/target.acle/{header-only,data-processing-run,ir-data-processing-19,exclusive-address-argument}.cpp` |
| 4 | `code generation for __builtin_neon_vmull_v is not supported yet` (xxhash's `vmull_u32`) | 2 (`util/hash.cc`, `util/xxhash.cc`) | `2e7f5b8c5` | `tests/conformance/target.neon/{mull-run,ir-mull-19}.cpp` |
| 5 | `no member named 'basic_regex' in 'std::basic_regex<char>'` (`using std::regex::basic_regex;`) | 1 (`test_util/testharness.cc`) | `b8889a8fc` | `tests/conformance/class.qual/p2-injected-class-name-after-typedef-run.cpp` |
| 6 | `'value_type' does not refer to a value` (`sizeof(std::remove_reference<…>::type::value_type)`) | 1 (`memtable/vectorrep.cc`) | `a8ed31ff0` | `tests/conformance/dcl.ambig.res/sizeof-member-typedef-of-specialization-run.cpp` |
| 7 | `no matching function for call to 'GetLiveFilesStorageInfo'` (`f({opts}, …)` with `opts` of the aggregate parameter type) | 1 (`db/db_write_test.cc`, not attempted in the first build) | `c6161897f` | `tests/conformance/over.ics.list/aggregate-single-element-of-class-{run,errors}.cpp` |

The spec sentences are in `spec/landed/sema/WP-724.md`. No cluster was a
bug outside kiln. kiln now declares all of the ACLE builtins but lowers
only some of them: the CRC32 checksums, `rbit`, `clz`, `cls`, the hints
and the barriers. The others are a "not supported yet" error if code
generation reaches them. rocksdb reaches none of them.

## What remains

- **Tests not run**: the other 146 test binaries. Some are slow (over
  3 minutes under clang, e.g. `db_test`, `db_compaction_test`,
  `db_bloom_filter_test`, `inlineskiplist_test`) or crash under clang on
  this host. A full run takes hours.
- **Variants not run**: Release (it builds no tests), `-O0`, the shared
  library, gflags, the benchmark tools and `ldb` and the other tools,
  compression libraries (none were found, so all are off), jemalloc,
  liburing and folly.
- **ACLE builtins not lowered**: the exclusive loads and stores
  (`__builtin_arm_ldrex`/`strex`/`ldaex`/`stlex`, which need the
  `elementtype` call-site attribute), `rsr`/`wsr`, `rint*`, `jcvt`,
  `rndr`, `ld64b`/`st64b`, `gcs`, `chkfeat`, `prefetch`, `trap` and
  `clrex`.
