# gRPC with kiln++ (WP-811)

gRPC **v1.85.1** (tag `v1.85.1`, shallow clone with shallow submodules),
the C++ core built with kiln++ as `CMAKE_CXX_COMPILER` through gRPC's
CMake build. Every dependency comes from its submodule: abseil, protobuf,
re2, c-ares, BoringSSL, zlib, googletest and benchmark. `protoc` and
`grpc_cpp_plugin` are built by the compiler under test and then generate
the C++ code of the test protos, so a miscompile in either shows up as
broken generated code. The C sources (BoringSSL's C files, c-ares, zlib,
upb) go to clang-19 under both builds. The control is a clang++-19 build
of the same tree with the same flags. Host: aarch64 Linux, libstdc++ 14,
C++17 (the project row), Release, default `-fclang-compat`. Date:
2026-11-08.

**Result: parity.** All 5732 TUs compile, all 120 targets link, and 117
of the 118 test binaries pass under both compilers. The 118th,
`end2end_http2_test`, runs 1095 test cases. It is killed at the 300 s
per-test limit under both compilers, with no failed case under either
(see below). kiln needed five fixes, all for valid code it rejected. No
test failed at run time because of kiln (no miscompile).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh grpc --compiler clang
scripts/oss/oss-build.sh grpc
scripts/oss/oss-report.py --perf grpc
```

These run in `.oss/build/grpc/<kiln|clang>/`. The wrapper
`scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and `timeout 600`.

```sh
cmake --fresh -S .oss/src/grpc -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DgRPC_BUILD_TESTS=ON -DgRPC_DOWNLOAD_ARCHIVES=OFF \
  -DgRPC_{ABSL,CARES,PROTOBUF,RE2,SSL,ZLIB,BENCHMARK}_PROVIDER=module \
  -DgRPC_BUILD_GRPC_{CSHARP,NODE,OBJECTIVE_C,PHP,PYTHON,RUBY}_PLUGIN=OFF
nice -n 10 make -k protoc grpc_cpp_plugin <the binaries of scripts/oss/grpc-tests.txt>
bash scripts/oss/grpc-tests.sh scripts/oss/grpc-tests.txt ../../../src/grpc
```

gRPC's CMake build registers no ctest tests (upstream drives its tests
with `tools/run_tests`). So the `grpc` row builds the 118 binaries listed
in `scripts/oss/grpc-tests.txt`, and `scripts/oss/grpc-tests.sh` runs
them: at most 4 at a time, each under `timeout 300`, from the source
root. The list has two parts:

- the 103 unit tests that link no `grpc_test_util` library (promises,
  party and activity, slices, event-engine timers, flow control, ...);
- 15 tests that need generated code or localhost: the `codegen_test_*`
  and `proto_*` tests and the end2end tests (`end2end_test`,
  `async_end2end_test`, `client_callback_end2end_test`,
  `generic_end2end_test`, `raw_end2end_test`, `hybrid_end2end_test`,
  `health_service_end2end_test`, `proto_server_reflection_test`,
  `end2end_inproc_test` and `end2end_http2_test`).

The end2end tests take their ports from gRPC's port server
(`tools/run_tests/python_utils/port_server.py`, on localhost:32766),
which `grpc-tests.sh` starts first. Without it, 9 end2end tests failed
under the control too (its first run was 109/118).

## Results

- **TUs** are the build-phase compiles. Many sources are compiled once
  per test target, so 5732 compiles cover 1636 distinct sources.
- **Tests** are the test binaries (`grpc-tests.sh`), passed / total. A
  binary that was not built counts as a failure.
- **Wall** is the harness run (configure, build and tests), from
  `report.json`, on a host shared with other implementers' builds.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

| Build | TUs ok / failed | Links | Tests | Wall | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (`e3ef7f543`) | 4327 / 37 (1368 not attempted) | 85 / 0 | 83 / 118 (35 not built) | 10888 s | 279 MB (`http2_server_transport.cc`) |
| clang++-19 (control) | 5732 / 0 | 120 / 0 | 117 / 118 | 14754 s | 799 MB (`end2end_test_suites.cc`) |
| **kiln, final** (`ee25b11ca`) | **5732 / 0** | **120 / 0** | **117 / 118** | 17001 s | **327 MB** (`end2end_test_suites.cc`) |

In the first kiln build, every one of the 83 binaries that linked passed.
The 35 failures were binaries that were not built, because a TU they
need was rejected. The first build stopped before
`src/cpp/server/server_context.cc`, so cluster 5 only showed once the
other four were fixed.

The final kiln build ran in three passes, because the build was stopped
once by a host restart:

- a full `oss-build.sh` run on the merged code (`7521b9a4d`), stopped
  part-way by the restart;
- an incremental `make -k` over the same targets in the same tree, which
  found cluster 5 in `server_context.cc` (13 compiles, one per test
  target that builds `grpc++`);
- after the fix (`ee25b11ca`, merged with main `75d7f9612`), a second
  incremental `make -k` and the test run.

The wall time is the sum of the three passes, without the tests of the
middle one. The TU counts take the newest record of each compile. The
TUs compiled in the second pass after the fix, and all those of the
third pass, were compiled by the fixed kiln++.

**`end2end_http2_test`** runs 1095 test cases, which do not finish in
300 s under either compiler. At the kill, the clang build had run 1034
cases and the kiln build 878, with 0 failed under both. The kiln run
shared the host with more builds, so this is not a hang. A 300 s limit
is the harness rule, so the test is reported as a failure under both.

### Performance

The comparison covers the 1636 sources that compiled under both, using
CPU time and the peak RSS of the compiler and its backend
(`oss-report.py --perf`).

- kiln/clang-19 geomean: time **1.17×**, RSS **0.44×**.
- Total: 3857 s vs 3446 s. Peak TU RSS: 327 MB vs 799 MB.

| heaviest TUs (by clang-19 time) | clang-19 s | kiln s | time × | clang-19 MB | kiln MB | RSS × |
|---|---|---|---|---|---|---|
| `test/cpp/end2end/end2end_test.cc` | 12.9 | 13.0 | 1.01 | 662 | 283 | 0.43 |
| `src/core/ext/transport/chttp2/transport/http2_server_transport.cc` | 12.5 | 13.3 | 1.07 | 614 | 279 | 0.45 |
| `third_party/boringssl-with-bazel/crypto/fipsmodule/bcm.cc` | 12.1 | 8.1 | 0.67 | 413 | 208 | 0.50 |
| `src/core/ext/transport/chaotic_good_legacy/server_transport.cc` | 12.0 | 15.1 | 1.26 | 679 | 293 | 0.43 |
| `src/core/ext/transport/chttp2/transport/http2_client_transport.cc` | 11.8 | 12.1 | 1.02 | 597 | 270 | 0.45 |

The heavy TUs are close to parity on time, at under half of clang's RSS.
As in CMake and Bitcoin Core, the geomean is pulled up by the small TUs
(1–3 s), where kiln takes up to 2.6× clang's time. Examples are
`src/core/util/subprocess_posix.cc` (2.0 s against 0.8 s) and
`src/core/lib/surface/metadata_array.cc` (2.7 s against 1.1 s). 436 of
the 1170 sources that take clang at least 0.5 s are over 1.2× on time.
No TU came near the 3 GB address-space limit.

## Clusters

| # | Signature (first diagnostic) | TUs | Fix | Repro |
|---|---|---|---|---|
| 1 | `IR lowering failed: IrError { kind: TypeMismatch }` in BoringSSL's `bcm.cc` (a GNU vector `v2u64` shifted by an `int`, the Keccak rotate) | 24 | `a587b85cf` | `tests/conformance/ext.vector/shift-scalar-count-other-width-run.cpp` |
| 2 | `unknown warning option '-Wframe-larger-than=25344' [-Werror,-Wunknown-warning-option]` (BoringSSL builds `bcm.cc` with it) | 7 | `3cf7a3c85` | `crates/kiln_diag/tests/policy.rs` |
| 3 | `called object type 'T (Slice, bool, absl::FunctionRef<...>)' is not a function or function pointer` at `metadata_batch.h:794` (`ParseValue<F, G>::Parse<F f, G g>`: a non-type template parameter of substituted function type is not adjusted to a pointer) | 4 (`filter_stack_call.cc`, `call_utils.cc`) | `9e02a2ffc` | `tests/conformance/temp.param/nttp-substituted-function-array-type-adjusted-run.cpp` |
| 4 | `case value is not a constant expression` (`case response.INITIAL:`, an enumerator named by a class member access) | 2 (`grpclb.cc`) | `7e9c63bdb` | `tests/conformance/expr.ref/enumerator-member-access-constant-run.cpp`, `expr.ref/enumerator-member-access-unknown-base.cpp` |
| 5 | `IR lowering failed: IrError { kind: BadCall, message: "call with 1 arguments to a function taking 2" }` (`new (buf) CompletionOp(...)`: the class-scope placement `operator delete(void*, void*)` cleanup was called without the placement argument) | 13 (`server_context.cc`) | `ee25b11ca` | `tests/conformance/expr.new/p28-reserved-placement-class-delete-run.cpp` |

The spec sentences are in spec/codegen.md (clusters 1 and 5),
spec/diag.md §4 (cluster 2), spec/sema-templates.md (cluster 3)
and spec/oven.md (cluster 4). No cluster was a bug outside kiln. The
harness side needed one change: the port server in `grpc-tests.sh`
(`4550545f5`), which the control needs too.

## Disk

The source with its submodules is 1.1 GB and the kiln build tree is
about 900 MB. The clang build tree was deleted once its numbers were
recorded; its logs and `report.json` are kept.

## What remains

- **`end2end_http2_test`**: it needs more than 300 s on this host under
  both compilers. Running it with a longer limit, or split by
  `--gtest_filter`, would give a full pass/fail comparison.
- **Tests not run**: the tests that link `grpc_test_util` (most of
  `test/core`), the xDS and interop tests, and the other language
  plugins (off in the configure line).
- **Small-TU compile time**: the 1.2–2.6× time ratio on TUs of 1–3 s.
