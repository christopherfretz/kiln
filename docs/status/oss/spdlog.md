# spdlog with kiln++ (WP-680)

spdlog **v1.17.0** (tag `v1.17.0`), built with kiln++ as
`CMAKE_CXX_COMPILER` through spdlog's own CMake build and its test suite,
against a clang++-19 control build of the same tree, with the M8 harness
(`scripts/oss/`, spec/landed/harness/WP-666.md). The tests fetch Catch2
**v3.5.0** with FetchContent (spdlog pins commit `53d0d91`); it is built by
the same compiler, so Catch2's sources are part of the build under test.
fmt is the bundled copy. Host: aarch64 Linux, libstdc++ 14, default
`-fclang-compat`. Date: 2026-11-07.

**Result: parity.** Every TU compiles, the test executable links, and
spdlog's suite passes: 179 Catch2 test cases, 588 assertions, under both
compilers. kiln needed three sema fixes (all found in Catch2's clara
command-line parser); none changes generated code.

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh spdlog --compiler clang
scripts/oss/oss-build.sh spdlog
scripts/oss/oss-report.py spdlog
```

which runs, in `.oss/build/spdlog/<kiln|clang>/` (wrapper
`scripts/oss/kiln-cxx`: `ulimit -v 3000000`, `timeout 600`):

```sh
cmake --fresh -S .oss/src/spdlog -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DSPDLOG_BUILD_TESTS=ON -DSPDLOG_BUILD_EXAMPLE=OFF
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure             # one test: spdlog-utests --order decl
```

The project row in `scripts/oss/projects.toml` worked unchanged; it is now
`verified = true`. The Catch2 summary line comes from running
`tests/spdlog-utests --order decl` directly (ctest prints one test).

## Results

TUs are the harness's build-phase compiles (spdlog 33, Catch2 107; CMake
probes excluded). Wall is the whole `oss-build.sh` run (configure, build,
test) from `report.json`; Σ TU wall sums the wrapper's per-TU wall times;
peak TU RSS is the wrapper's `ru_maxrss` (the compiler and the backend or
linker it waited for). One run each.

| Build | TUs ok / failed | Links | Tests (ctest; Catch2 cases) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `7c5998af2`) | 109 / 4 (27 not attempted) | 0 / 0 | 0/1 (not built) | 22 s | — | 113 MB |
| clang++-19 (control) | 140 / 0 | 1 / 0 | 1/1; 179 cases, 588 assertions | 25 s | 173.5 s | 296 MB (`src/spdlog.cpp`) |
| **kiln, final** (WP-680 fixes) | **140 / 0** | **1 / 0** | **1/1; 179 cases, 588 assertions** | 29 s | 218.8 s | **117 MB** (`tests/test_pattern_formatter.cpp`) |
| kiln, final + main with WP-667 | 140 / 0 | 1 / 0 | 1/1; 179 cases, 588 assertions | 64 s ‡ | 191.3 s | 119 MB (`src/spdlog.cpp`) |

The first kiln build stopped at Catch2's library (4 failed TUs), so the
27 TUs that depend on it were not compiled. The first control build's
wall (97 s) included cloning Catch2 at configure; the 25 s above is a
rerun with the clone present, like the kiln runs. kiln takes 1.1–1.26×
clang's summed TU time and 0.40× its peak RSS (LLVM codegen runs out of
process in llc-19). ‡ That run shared the host with three other harness
builds; its Σ TU wall (lower than the 29 s run's) shows the 64 s is
host load, not compile cost. `docs/status/oss.md` carries this last run.

Warnings: before WP-667 reached main, kiln printed 31
`unknown attribute 'no_sanitize' ignored` (bundled fmt's
`FMT_NO_SANITIZE`) and a configure probe hit
`unknown argument: '-print-resource-dir'`. WP-667 (fmt) fixed both
(`e20c5e5ac`, `57e56944d`), so they were not re-fixed here; with main
merged, the kiln build prints no warnings, the same as clang.

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `'m_type' is a protected member of 'ResultBase'` at clara's `using ResultBase::m_type;` in `BasicResult<T> : ResultValueBase<T>` | 4 | fixed, commit `062f38bf9` (sema: replay a dependent protected-access check with the instantiated object class) | `tests/conformance/class.protected/dependent-base-current-object.cpp` |
| 2 | `calling a protected constructor of class 'ResultBase'` at `BasicResult::ok()`'s `return { ResultType::Ok };` (inherited through two `using X::X;`) | 4 (exposed by 1) | fixed, commit `e602261d4` (sema: an inherited protected constructor's object class is the constructed class; also clang's ctor/dtor note) | `tests/conformance/namespace.udecl/inherited-protected-ctor-object.cpp`, `tests/conformance/class.protected/ctor-restricted-note.cpp` |
| 3 | `ambiguous partial specializations of 'UnaryLambdaTraits<…>'` (`R (C::*)(A) const` vs `R (C::*)(A...) const`) | 2 (exposed by 2) | fixed, commit `030cd5639` (sema: a function parameter pack does not deduce a non-pack parameter in partial ordering) | `tests/conformance/temp.deduct.type/function-param-pack-vs-single-ordering.cpp` |
| — | configure: `kiln++: error: unknown argument: '-print-resource-dir'` (CMake probe; configure still succeeds) | 0 | fixed by WP-667 in `57e56944d`, not re-fixed | — |
| — | `-Wunknown-attributes` on `no_sanitize` (31 warnings) | 0 | fixed by WP-667 in `e20c5e5ac`, not re-fixed | — |

No miscompiles: every spdlog test (async, file sinks, pattern formatter,
backtrace, …) passes under the kiln build.

## What remains

- Found while reducing cluster 2, not on spdlog's path: kiln does not
  diagnose a protected destructor used for a complete object from a
  derived class's member. clang:
  `error: variable of type 'B' has protected destructor` +
  `note: protected destructor can only be used to destroy a base class subobject`
  for
  ```cpp
  class B { protected: B(int) {} ~B() {} };
  struct Q : B { Q() : B(0) {} void f() { B b(3); } };
  ```
  (kiln reports only the constructor). The note text already exists
  (`note_access_protected_restricted_ctordtor`); the missing piece is the
  destructor access check of a local variable whose naming context is a
  derived class.
- Variants not run: Debug, C++26, `SPDLOG_USE_STD_FORMAT=ON`,
  `SPDLOG_BUILD_EXAMPLE=ON`, shared library.
