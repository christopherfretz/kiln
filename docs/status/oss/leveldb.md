# leveldb with kiln++ (WP-720)

google/leveldb **1.23** (tag `1.23`, submodules `third_party/googletest`
`c27aceb` and `third_party/benchmark` `bf585a2`), built with kiln++ as
`CMAKE_CXX_COMPILER` through leveldb's own CMake build and its CTest
suite, against a clang++-19 control build of the same tree, with the M8
harness (`scripts/oss/`, spec/landed/harness/WP-666.md). leveldb compiles
itself with `-fno-exceptions -fno-rtti` and `-Werror`. Host: aarch64
Linux, libstdc++ 14, default `-fclang-compat`, C++17. Date: 2026-11-07.

**Result: parity on the first build.** Every TU compiles, every target
links and all 30 CTest tests pass under both compilers. One kiln
codegen bug was found anyway, by linking kiln objects against clang's
library: kiln ignored `-fno-rtti` in codegen (cluster 1, fixed).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh leveldb --compiler clang
scripts/oss/oss-build.sh leveldb
scripts/oss/oss-report.py leveldb
```

which runs, in `.oss/build/leveldb/<kiln|clang>/` (wrapper
`scripts/oss/kiln-cxx`: `ulimit -v 3000000`, `timeout 600`):

```sh
cmake --fresh -S .oss/src/leveldb -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DLEVELDB_BUILD_TESTS=ON -DLEVELDB_BUILD_BENCHMARKS=OFF \
  "-DCMAKE_CXX_FLAGS=-Wno-error=deprecated-copy -Wno-error=unused-but-set-variable"
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure             # 30 tests; TMPDIR, TEST_TMPDIR = test-tmp/
```

The project row needed the `CMAKE_CXX_FLAGS` line: the pinned googletest
and benchmark submodules are old, and under leveldb's `-Werror` clang 19
rejects them (12 `-Wdeprecated-copy` in gmock's matchers and actions, one
`-Wunused-but-set-variable` in `benchmark/src/complexity.cc`); the clang
control build failed 2 TUs and ran no tests without it. It is now
`verified = true`. The test phase now runs with a per-build
`TEST_TMPDIR` (below).

## Results

TUs are the harness's build-phase compiles (CMake probes excluded); links
are the 30 test executables and `leveldbutil` (31). Wall is
the whole `oss-build.sh` run (configure, build, test; the tests alone are
~120 s of it) from `report.json`; peak TU RSS is the wrapper's
`ru_maxrss` (the compiler and the backend or linker it waited for). One
run each, on a shared host with load average 15 to 18.

| Build | TUs ok / failed | Links | Tests (ctest) | Warnings | Wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `b5bf31981`) | 121 / 0 | 31 / 0 | 29 / 30 (*) | 1 | 96 s | 137 MB (`gtest-all.cc`) |
| clang++-19 (control) | 121 / 0 | 31 / 0 | 30 / 30 | 13 | 139 s, 166 s | 199 MB (`gtest-all.cc`), 198 MB (`db_test.cc`) |
| **kiln, final** (WP-720) | **121 / 0** | **31 / 0** | **30 / 30** | 1 | 161 s | **137 MB** (`gtest-all.cc`) |

(*) `db_test` failed 115 subtests (`TryReopen not ok`) only because the
clang control's ctest ran at the same time and both used
`/tmp/leveldbtest-0`; the same binary passes 57/57 with its own
`TEST_TMPDIR`. Fixed in the harness (2340999ac): the test phase now gets
`TMPDIR`/`TEST_TMPDIR` = `<build>/test-tmp`.

The run walls and summed per-TU walls swing with host load (clang's
summed compile time was 109 s in one run and 212 s in the next), so they
do not compare the compilers. Sequential timings of the three heaviest
TUs, one after another on the same host:

| TU | kiln | clang++-19 | kiln RSS | clang RSS |
|---|---|---|---|---|
| `db/db_impl.cc` | 1.07 s | 0.86 s | 72 MB | 142 MB |
| `db/db_test.cc` | 3.79 s | 3.12 s | 126 MB | 199 MB |
| `gtest-all.cc` | 3.96 s | 3.46 s | 137 MB | 200 MB |

kiln takes 1.15 to 1.25× clang's time and 0.5 to 0.7× its peak RSS
(LLVM codegen runs out of process in llc-19).

Warnings: kiln reports only the `-Wunused-but-set-variable` one; it does
not implement `-Wdeprecated-copy` (What remains).

## Clusters

| # | Signature | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | Mixed link: kiln objects (`db_test.cc.o`) against clang's `-fno-rtti` `libleveldb.a`: `undefined reference to 'typeinfo for leveldb::Comparator'` (also `Env`, `Logger`, …). kiln parsed `-fno-rtti` (sema rejects `typeid`/`dynamic_cast`) but codegen still filled every vtable's RTTI slot with `_ZTI…` and referred to the key-function TU's `type_info`, which a `-fno-rtti` TU never defines | every TU with a dynamic class (no failure in the pure kiln build: kiln's own objects defined and referenced it consistently) | fixed, commit `a5859ecc3` (codegen: RTTI slots null under `-fno-rtti`, EH `type_info` `linkonce_odr`; spec/codegen.md "RTTI", spec/landed/codegen/WP-720.md) | `tests/conformance/except.handle/ir-fno-rtti-type-info.cpp`, `ir-fno-rtti-type-info-19.cpp`, `fno-rtti-catch-by-base.cpp` |
| — | `db_test`: 115 subtests `TryReopen not ok` | 0 | not a kiln bug: concurrent test phases shared `/tmp/leveldbtest-0`; harness fixed, `2340999ac` | — |
| — | clang control: `-Werror` on gmock `-Wdeprecated-copy` and benchmark `-Wunused-but-set-variable` | 2 (clang) | project row flags (`-Wno-error=`) | — |

After the fix, mixed links in both directions pass: kiln's `db_test`
objects against clang's `libleveldb.a`/gtest/gmock/benchmark, and clang's
`db_test` objects against kiln's libraries; both binaries pass 57/57.
`nm` finds no `_ZTI` symbol, defined or undefined, in either
`libleveldb.a`.

No miscompiles: every leveldb test (db, table, log, version set, env,
cache, coding, filter, recovery, corruption, C API) passes under
the kiln build, `crc32c_test`, `env_posix_test` and `fault_injection_test` included.

## What remains

- `-Wdeprecated-copy` (and its `-dtor`/`-with-*` subgroups) is not
  diagnosed; only the group names exist
  (`crates/kiln_diag/src/clang_groups.rs:243`). Under leveldb's
  `-Werror` without the project row's `-Wno-error=` flags, clang rejects
  the gmock TUs and kiln accepts them.
- Variants not run: Debug, C++20/26, `LEVELDB_BUILD_BENCHMARKS=ON`,
  `BUILD_SHARED_LIBS=ON`, and snappy/crc32c/tcmalloc (CMake finds none
  of them on this host, so neither build uses them).
