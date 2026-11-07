# All of LLVM with kiln++ (WP-782)

LLVM **19.1.7** (tag `llvmorg-19.1.7`, `llvm/` only) built with kiln++ as
`CMAKE_CXX_COMPILER` through LLVM's own CMake build: every library, the
AArch64 and X86 backends, every tool, the unit tests and `check-llvm`
(lit plus the unit tests). The control is a clang++-19 build of the same
tree. Both use the M8 harness (`scripts/oss/`,
spec/landed/harness/WP-666.md). Host: aarch64 Linux, libstdc++ 14, C++17
(the project row), Release with assertions on, `-O3`, no debug info,
default `-fclang-compat`. Date: 2026-11-07.

**Result:** _(filled in at the final run)_

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh llvm_full --compiler clang
scripts/oss/oss-build.sh llvm_full
scripts/oss/oss-report.py llvm_full
```

These run the following in `.oss/build/llvm_full/<kiln|clang>/`. The
wrapper `scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and
`timeout 600`; the row sets no memory-cap override.

```sh
cmake --fresh -S .oss/src/llvm_full/llvm -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DLLVM_TARGETS_TO_BUILD="AArch64;X86" -DLLVM_ENABLE_PROJECTS= \
  -DLLVM_ENABLE_ASSERTIONS=ON -DLLVM_INCLUDE_TESTS=ON \
  -DLLVM_INCLUDE_BENCHMARKS=OFF -DLLVM_INCLUDE_EXAMPLES=OFF \
  -DLLVM_ENABLE_ZSTD=OFF -DLLVM_ENABLE_LIBXML2=OFF \
  -DLLVM_LIT_ARGS="-sv -j 4"
nice -n 10 make clean
nice -n 10 make -k all llvm-test-depends UnitTests   # shared jobserver
timeout 10800 nice -n 10 make check-llvm
```

The project row adds `test_timeout = 10800` and a `lit_summary` reader
for lit's result block (`scripts/oss/projects.py`).

## Results

TUs are the build-phase C++ compiles, counted once each; CMake probes
are excluded. Wall is the whole `oss-build.sh` run (configure, build and
`check-llvm`). Peak TU RSS is the wrapper's `ru_maxrss` for the compiler
plus the backend it waited for.

| Build | TUs ok / failed | Links | check-llvm (lit + unit) | Wall | Peak TU RSS |
|---|---|---|---|---|---|
| clang++-19 (control) | 2780 / 0 | 165 / 165 | 38432 passed, 9 failed ‡, 22121 unsupported, 78 skipped, 65 xfail (60705 discovered) | 3675 s (lit 2029 s) | 932 MB (`lib/Passes/PassBuilder.cpp`) |

‡ The control's nine failures are environmental and are expected under
kiln too: three Support permission tests that root does not fail
(`FileSystemTest.CreateDir`, `FileSystemTest.RespectUmask`,
`raw_ostreamTest.filePermissions`), four `raw_socket_streamTest` tests
(the container's socket sandbox) and two `llvm-lipo` executable tests.

The control build tree was deleted once these numbers were recorded
(4.8 GB); its `compile.jsonl`, logs and the 112 clang-built static
libraries (247 MB) were kept under `.oss/runs/clang/` for per-library
object swaps.

## Clusters

## What remains
