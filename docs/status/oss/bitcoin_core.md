# Bitcoin Core with kiln++ (WP-811)

Bitcoin Core **v32.1** (tag `v32.1`), built with kiln++ as
`CMAKE_CXX_COMPILER` through its CMake build, with three test layers run:
`bench_bitcoin -sanity-check`, the Python functional suite, and ctest
(`test_bitcoin`, with one ctest test per Boost.Test suite, plus the
secp256k1 and univalue tests). The control is a clang++-19 build of the
same tree with the same flags. libsecp256k1 is C and is built with
clang-19 under both. Host: aarch64 Linux, libstdc++ 14, C++20 (the
project's own standard), Release, default `-fclang-compat`. Date:
2026-11-08.

**Result: parity.** All 477 TUs compile, all 9 targets link,
`bench_bitcoin -sanity-check` passes, and every ctest test passes. 380
functional tests pass under kiln. The two that failed in the full
parallel run, `feature_init.py` and `feature_assumeutxo.py`, pass when run
alone with the kiln build. The same failure, a LevelDB `NotFound` right
after a database directory is wiped and re-created, also happens to
`feature_assumeutxo.py` with the clang build when run alone. So the cause
is the shared bind-mounted filesystem, not kiln. kiln needed five fixes:
two for valid programs it rejected, two miscompiles, and one missing
builtin family.

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh bitcoin_core --compiler clang
scripts/oss/oss-build.sh bitcoin_core
scripts/oss/oss-report.py --perf bitcoin_core
```

These run in `.oss/build/bitcoin_core/<kiln|clang>/`. There is no
`depends/` build: the system Boost 1.83 headers (Boost.Test header-only)
and the system libevent are used.

```sh
cmake --fresh -S .oss/src/bitcoin_core -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_GUI=OFF -DENABLE_WALLET=OFF -DWITH_ZMQ=OFF -DENABLE_IPC=OFF \
  -DBUILD_BENCH=ON -DBUILD_TESTS=ON -DWITH_CCACHE=OFF -DINSTALL_MAN=OFF
nice -n 10 make -k
./bin/bench_bitcoin -sanity-check
python3 test/functional/test_runner.py -j 4 --tmpdirprefix="$TMPDIR" \
  --combinedlogslen=0 \
  --exclude feature_reindex_init --exclude feature_reindex_readonly
ctest --output-on-failure -j 4 --timeout 300 -E '^univalue_object_test$'
```

The wallet is off because the container has no sqlite3 headers, so the
wallet functional tests skip themselves (94 skipped under both builds).
Three tests are excluded under both compilers:

- `feature_reindex_init` and `feature_reindex_readonly` make the data
  directory read-only, which root ignores.
- `univalue_object_test` puts its checks inside `assert()`. The Release
  (`NDEBUG`) build compiles those out together with their side effects,
  and the control dies with SIGILL.

## Results

- **TUs** are the build-phase compiles. Configure probes are not
  counted; see below.
- **Tests** are the ctest results, which are what `report.json` reads.
  Functional is the Python suite, as passed / failed.
- **Wall** is the `oss-build.sh` run (configure, build and all three test
  layers), from `report.json`.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

| Build | TUs ok / failed | Links | ctest | Functional | Wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (`05e088464`) | 474 / 2 (1 not built) | 8 / 0 (`test_bitcoin` not attempted) | 221 / 358 | 243 / 144 | 2193 s | 792 MB (`init.cpp`) |
| clang++-19 (control) | 477 / 0 | 9 / 0 | 357 / 358 | 382 / 2 | 2002 s | 1225 MB (`init.cpp`) |
| **kiln, final** (`ef041a9f6`) | **477 / 0** | **9 / 0** | **357 / 357** | **380 / 2** | 2167 s | **801 MB** (`init.cpp`) |

The control ran before `univalue_object_test` and the two reindex tests
were excluded, so its 358th ctest test and its 2 functional failures are
those tests. The final kiln build's 2 functional failures are the
filesystem failures described above.

In the first kiln build:

- the two rejected TUs were both in `test_bitcoin`, so it was not linked
  and its 137 ctest tests could not run (`univalue_object_test`, the
  358th, passed);
- `crypto/sha256_arm_shani.cpp` was not built, because the configure
  probe for the ARM SHA-NI intrinsics failed (cluster 4);
- `bitcoind` aborted with a double free while loading a chain
  (cluster 3), which failed most of the functional suite.

**Configure probes.** The final kiln build has 104 ok and 20 rejected,
the same as the control. The two flags that only kiln rejects are
`-ftrapv` (which kiln rejects on purpose, spec/driver.md; Bitcoin adds it
only to Debug flags) and `-mbranch-protection=standard` (a follow-up). The
others are flags both compilers reject on aarch64, such as `-msse4.1` and
GCC-only warnings, with different wording.

### Performance

The comparison covers the 477 TUs that compiled under both, using CPU
time and the peak RSS of the compiler and its backend
(`oss-report.py --perf`).

- kiln/clang-19 geomean: time **1.14×**, RSS **0.42×**.
- Total: 1178 s vs 1076 s. Peak TU RSS: 801 MB vs 1225 MB.

| heaviest TUs (by clang-19 time) | clang-19 s | kiln s | time × | clang-19 MB | kiln MB | RSS × |
|---|---|---|---|---|---|---|
| `src/test/descriptor_tests.cpp` | 19.3 | 16.2 | 0.84 | 404 | 231 | 0.57 |
| `src/test/script_tests.cpp` | 11.3 | 13.8 | 1.22 | 434 | 214 | 0.49 |
| `src/rpc/net.cpp` | 11.1 | 20.9 | 1.87 | 1168 | 781 | 0.67 |
| `src/init.cpp` | 11.0 | 20.1 | 1.82 | 1225 | 801 | 0.65 |
| `src/rpc/blockchain.cpp` | 10.8 | 12.9 | 1.19 | 476 | 217 | 0.46 |

The slowest TUs relative to clang (about 1.6–1.9×) are the large
`rpc/`, `init.cpp` and `bench/` TUs, and their RSS stays at 0.65–0.7× of
clang's. These are the TUs with the largest code size, which suggests the
time is spent in IR emission or the backend rather than in the front end.
96 of the 397 TUs that take clang at least 0.5 s are over 1.2× on time.

## Clusters

| # | Signature (first diagnostic or symptom) | TUs / tests | Fix | Repro |
|---|---|---|---|---|
| 1 | `expected '(' for function-style cast` at `sizeof(decltype(e)::type)` | 1 (`test/disconnected_transactions.cpp`) | `489d2c1ea` | `tests/conformance/expr.sizeof/decltype-qualified-type-operand-run.cpp` |
| 2 | `ambiguous partial specializations of '_Parse_int<'0', 'x', ...>'` (libstdc++ `chrono.h`, the `0x..._s` literal) | 1 (`test/netbase_tests.cpp`) | `6db16e420` | `tests/conformance/temp.spec.partial.match/order-nontype-pack-leading-values-run.cpp` |
| 3 | miscompile: double free in `bitcoind` (`TranslatedLiteral::operator bilingual_str` in `S{tl}`; the conversion function's result was bound twice) | most of the 144 failed functional tests | `64c44c8db` | `tests/conformance/class.temporary/conversion-function-result-in-braced-cast-destroyed-once-run.cpp` |
| 4 | `vsha256hq_u32` and the other AArch64 crypto NEON builtins not lowered, so the `HAVE_ARM_SHANI` probe failed | 1 probe, `crypto/sha256_arm_shani.cpp` not built | `5ea2fefc9` | `tests/conformance/target.neon/crypto-run.cpp`, `target.neon/ir-crypto-19.cpp` |
| 5 | miscompile: `std::source_location::current()` as a constructor's default argument gave the default argument's location, not the construction's (every log line showed `[util/log.h:30]`; `logging_tests` failed, and the log rate limiter, keyed on the call site, merged all log sites) | `logging_tests`, 2 functional tests | `ef041a9f6` | `tests/conformance/expr.const/builtins-srcloc-constructor-default-arg-run.cpp` |

The spec sentences are in spec/parse.md (cluster 1), spec/sema-templates.md
(clusters 2 and 5), spec/sema-class.md (cluster 3) and spec/codegen.md
(cluster 4). None of the clusters was a bug outside kiln.

While reducing cluster 1, the same `decltype(e)::name` form in a parameter
declaration was found to be rejected too. It is not in Bitcoin, so it was
not fixed here: `tests/conformance/dcl.type.decltype/decltype-qualified-type-parameter-declaration.cpp`
is an xfail repro (`//@ xfail(kiln): WP-811 follow-up`).

## Disk

The source is 70 MB and the kiln build tree is 630 MB. The clang build
tree, 525 MB, was deleted once its numbers were recorded.

## What remains

- **`-mbranch-protection=standard`**: kiln does not accept the flag, so
  Bitcoin's hardening probe drops it and the kiln build has no PAC/BTI.
  This needs the driver option plus the `branch-target-enforcement` and
  `sign-return-address` module flags and function attributes.
- **Not built**: the wallet (no sqlite3 headers), the GUI, ZMQ and IPC
  (no capnp), and the `depends/` build.
- **Compile time** of the largest TUs (1.6–1.9× clang's).
