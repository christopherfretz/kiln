# glog with kiln++ (WP-700)

glog **v0.7.1** (tag `v0.7.1`, the last release; the project is archived
upstream), built with kiln++ as `CMAKE_CXX_COMPILER` through glog's own
CMake build and its CTest suite, against a clang++-19 control build of the
same tree, with the M8 harness (`scripts/oss/`,
spec/landed/harness/WP-666.md). gflags, googletest and libunwind are off
(`-DWITH_GFLAGS=OFF -DWITH_GTEST=OFF -DWITH_UNWIND=OFF`), so glog uses its
own unit-test shim and its own stack unwinder. Host: aarch64 Linux,
libstdc++ 14, default `-fclang-compat`, C++17. Date: 2026-11-07.

**Result: parity on the first build.** Every TU compiles, every target
links, and the same 23 of 24 CTest tests pass under both compilers (the
one failure, `symbolize`, fails identically under clang: an environment
limit, see below). The only difference was one spurious warning from
kiln, `-Winvalid-noreturn` on `LogMessage::Fail()`, fixed here; it did
not change generated code.

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh glog --compiler clang
scripts/oss/oss-build.sh glog
scripts/oss/oss-report.py glog
```

which runs, in `.oss/build/glog/<kiln|clang>/` (wrapper
`scripts/oss/kiln-cxx`: `ulimit -v 3000000`, `timeout 600`):

```sh
cmake --fresh -S .oss/src/glog -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_TESTING=ON -DWITH_GFLAGS=OFF -DWITH_GTEST=OFF -DWITH_UNWIND=OFF
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure             # 26 tests, 2 disabled by glog
```

The project row in `scripts/oss/projects.toml` worked unchanged; it is now
`verified = true`. Several CTest tests (`cmake_package_config_*`,
`log_severity_*`, `includes_*`, `dcheck_off`) configure and build small
projects against the installed package at test time, with the same
compiler, so those compiles are kiln's too; `log_severity_constants` and
`log_severity_conversion` expect a compile to fail and match its
diagnostic against clang's wording (`no known conversion from 'int'`),
and pass.

## Results

TUs are the harness's build-phase compiles (CMake probes excluded); links
are the 14 executables and the shared library. Wall is the whole `oss-build.sh` run
(configure, build, test) from `report.json`; Σ wall sums the wrapper's
per-invocation wall times (23 compiles + 14 links); peak TU RSS is the
wrapper's `ru_maxrss` (the compiler and the backend or linker it waited
for). One run each.

| Build | TUs ok / failed | Links | Tests (ctest) | Warnings | Wall | Σ wall | Peak TU RSS |
|---|---|---|---|---|---|---|---|
| kiln, first (`2fac46d71`) | 23 / 0 | 14 / 0 | 23 / 24 | 1 | 35 s | — | 99 MB (`src/logging.cc`) |
| clang++-19 (control) | 23 / 0 | 14 / 0 | 23 / 24 | 0 | 37 s | 18.7 s | 169 MB (`src/logging.cc`) |
| **kiln, final** (WP-700 fix) | **23 / 0** | **14 / 0** | **23 / 24** | **0** | 20 s | 21.3 s | **99 MB** (`src/logging.cc`) |

kiln takes 1.14× clang's summed compile and link time and 0.59× its peak
RSS (LLVM codegen runs out of process in llc-19). The whole-run walls
(20–37 s) are dominated by configure, the test-time builds and host load
from other harness runs, not by the compiler; the slowest single TU is
`src/logging.cc` under both (2.9 s kiln, 2.8 s clang).

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `warning: function declared 'noreturn' should not return [-Winvalid-noreturn]` at `src/logging.cc:1953` `void LogMessage::Fail() { g_logging_fail_func(); }`, where `g_logging_fail_func` has type `typedef void (*logging_fail_func_t)() __attribute__((noreturn))` | 1 (warning only) | fixed, commit `e25bef16a` (sema: GNU `noreturn` is a function type attribute; spec/types.md §5.4, spec/landed/sema/WP-700.md) | `tests/conformance/dcl.attr.noreturn/gnu-noreturn-function-type.cpp`, `tests/conformance/dcl.attr.noreturn/gnu-noreturn-function-type-conv.cpp` |
| — | `symbolize`: `F… symbolize_unittest.cc:284] sigaltstack(&old_sigstk, nullptr): Invalid argument [22]` (abort) | 0 | not a kiln bug: fails the same way under clang++-19; the test installs an alternate signal stack, which this container refuses | — |

Cluster 1 had to be fixed as a whole: modelling only the call through
the pointer would have rejected glog's own uses, which only work because
the attribute is part of the type, such as
`InstallFailureFunction(&DumpStackTraceAndExit)` with
`__attribute__((noreturn)) static void DumpStackTraceAndExit()`
(`src/utilities.cc`) and a `[]() __attribute__((noreturn)) { … }` lambda
passed to it (`src/logging_unittest.cc`).

No miscompiles: every glog test that runs (logging, signal handler,
stack trace, STL logging, strip-log levels, log cleanup, package config)
passes under the kiln build.

## What remains

- GNU `noreturn` written in a pointer chunk of an abstract declarator
  (`using F = void (* __attribute__((noreturn)))();`) or inside grouping
  parentheses (`void (__attribute__((noreturn)) *p)();`) is not yet
  applied to the function type; clang applies it.
  `crates/kiln_parse/src/decl.rs:2313` (`skip_type_position_attrs`) drops
  those attributes. glog writes the attribute after the declarator only.
- `symbolize` cannot run in this container (`sigaltstack` EINVAL under
  both compilers).
- Tests glog disables itself: `demangle` and `dcheck_on`.
- Variants not run: Debug (`dcheck_on`), C++20/26, static library
  (glog defaults to `BUILD_SHARED_LIBS=ON`, which is what ran),
  `WITH_GFLAGS=ON`, `WITH_GTEST=ON`, `WITH_UNWIND=ON`.
