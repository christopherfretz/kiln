# taskflow with kiln++ (WP-703)

taskflow **v3.10.0** (tag `v3.10.0`), built with kiln++ as
`CMAKE_CXX_COMPILER` through taskflow's own CMake build (the 27 unit-test
executables) and its CTest suite (2462 doctest cases, discovered per
executable), against a clang++-19 control build of the same tree, with
the M8 harness (`scripts/oss/`, spec/landed/harness/WP-666.md). doctest
is the bundled copy. Host: aarch64 Linux, libstdc++ 14, C++20, default
`-fclang-compat`. Date: 2026-11-07.

**Result: parity on the first build.** Every TU compiles, every test
executable links and all 2462 tests pass under both compilers, with kiln
main `cd5fd187e` unchanged. The one difference was a kiln-only
`-Wall` warning (taskflow compiles with `-Wall -Wextra -Wfatal-errors
-Wshadow`), fixed here; the final kiln build prints no warning, the same
as clang.

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh taskflow --compiler clang
scripts/oss/oss-build.sh taskflow
scripts/oss/oss-report.py taskflow
```

which runs, in `.oss/build/taskflow/<kiln|clang>/` (wrapper
`scripts/oss/kiln-cxx`: `ulimit -v 3000000`, `timeout 600`):

```sh
cmake --fresh -S .oss/src/taskflow -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DTF_BUILD_TESTS=ON -DTF_BUILD_EXAMPLES=OFF -DTF_BUILD_BENCHMARKS=OFF
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure
```

The project row in `scripts/oss/projects.toml` worked unchanged; it is
now `verified = true`. Each TU is compiled with
`-O3 -DNDEBUG -std=gnu++20 -Wall -Wextra -Wfatal-errors -Wshadow`.

## Results

TUs are the harness's build-phase compiles (CMake probes excluded). Wall
is the whole `oss-build.sh` run (configure, build, test; the test run is
~300 s of it under both compilers) from `report.json`; Σ TU wall sums the
wrapper's per-TU wall times; peak TU RSS is the wrapper's `ru_maxrss`
(the compiler and the backend or linker it waited for). One run each.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `cd5fd187e`) | 27 / 0 | 27 / 0 | 2462/2462 | 325 s | 221.4 s | 235 MB (`test_pipelines.cpp`) |
| clang++-19 (control) | 27 / 0 | 27 / 0 | 2462/2462 | 303 s | 227.4 s | 402 MB (`test_dependent_asyncs.cpp`) |
| **kiln, final** (WP-703 fix) | **27 / 0** | **27 / 0** | **2462/2462** | 272 s | 307.9 s ‡ | **239 MB** (`test_pipelines.cpp`) |

kiln's summed TU time is 0.97× clang's in the first build and its peak
RSS 0.58× (LLVM codegen runs out of process in llc-19). The slowest TU
takes 14.1 s with kiln and 15.9 s with clang. ‡ The final run shared the
host with several other harness builds; the fix touches only a warning,
so the first run's Σ TU wall is the comparable one.

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `warning: unused variable 'res' [-Wunused-variable]` on the init-capture of `[&res=results[i], &graph=graphs[i]](tf::Runtime& rt) mutable { rt.corun(graph); }` (`unittests/test_runtimes.cpp:73`; kiln only, not a failure) | 0 (1 warning) | fixed, commit `7f3ca6b64` (sema: an init-capture's variable is referenced by its capture, never -Wunused-variable; still -Wunused-but-set-variable) | `tests/conformance/expr.prim.lambda.capture/init-capture-not-unused-variable.cpp` |

No build failures and no miscompiles: every test (work stealing,
pipelines, algorithms, exceptions, cancellation, runtimes) passes under
the kiln build.

## Beyond the harness build

Probes outside the project row, with the same kiln++ (not part of the
harness numbers):

- **Examples** (`examples/*.cpp`, 41 programs, `-O2 -Wall -Wextra
  -Wshadow`): all compile, link and run under both compilers with the
  same output up to thread scheduling; `data_scalable_parallel_pipeline`
  fails to compile under both (the example names a type that no longer
  exists in v3.10.0).
- **-O0 -g** (the 27 unit-test sources, kiln only, then clang for the
  two failures): `test_asyncs` and `test_movable` abort in a test case
  under both compilers (assertions are live without `-DNDEBUG`), so
  not a kiln bug; the other 25 pass.
- **C++17 / C++23 / C++26** (`-fsyntax-only -Wall -Wextra`, the unit
  tests): C++23 and C++26 match clang exactly. Under C++17 kiln warned
  `use of function template name with no prior declaration in function
  call with explicit template arguments is a C++20 extension` on every
  tuple-like structured binding (`auto [p, f] = stack.top();`); that is
  fixed on main by WP-683 (`5257fbaf5`), so it was not re-fixed.
  `test_priorities.cpp` (not in taskflow's test list) fails under both
  compilers (it names a removed `tf::TaskPriority`).

## What remains

- kiln has no `-Wshadow`: `unittests/test_serializer.cpp` (disabled in
  taskflow's CMake) gets four `declaration shadows a local variable`
  warnings from clang and none from kiln, e.g. for
  ```cpp
  int a[4];
  void f() { for (int i = 0; i < 2; ++i) for (auto &i : a) i = 1; }
  ```
  taskflow's default build has no shadowing, so its build output is
  unaffected. A missing warning family, not a correctness gap.
- kiln has no `-Wunused-lambda-capture` (spec/diag.md lists it as a
  missing `unused` subgroup): clang warns `lambda capture 'r' is not used`
  on an unused init-capture with a side-effect-free initializer, kiln
  says nothing (since this WP; it said `unused variable` before).
- Variants not run through the harness: Debug, `TF_BUILD_EXAMPLES=ON`,
  `TF_BUILD_BENCHMARKS=ON`, sanitizers.
