# yaml-cpp with kiln++ (WP-698)

yaml-cpp **0.9.0** (tag `yaml-cpp-0.9.0`), built with kiln++ as
`CMAKE_CXX_COMPILER` through yaml-cpp's own CMake build and its test
suite, against a clang++-19 control build of the same tree, with the M8
harness (`scripts/oss/`, spec/landed/harness/WP-666.md). The tests use the
googletest **1.13.0** that yaml-cpp vendors (`test/googletest-1.13.0`); it
is built by the same compiler, so googletest's and googlemock's sources
are part of the build under test. Host: aarch64 Linux, libstdc++ 14,
default `-fclang-compat`. Date: 2026-11-07.

**Result: parity on the first kiln build.** Every TU compiles, the test
executable links, and yaml-cpp's suite passes: 1011 googletest tests in
16 suites (8 disabled), under both compilers. kiln needed no fix. The
project row needed one configure flag (below), for both compilers.

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh yaml_cpp --compiler clang
scripts/oss/oss-build.sh yaml_cpp
scripts/oss/oss-report.py yaml_cpp
```

which runs, in `.oss/build/yaml_cpp/<kiln|clang>/` (wrapper
`scripts/oss/kiln-cxx`: `ulimit -v 3000000`, `timeout 600`):

```sh
cmake --fresh -S .oss/src/yaml_cpp -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DYAML_CPP_BUILD_TESTS=ON -DYAML_CPP_BUILD_TOOLS=OFF \
  -DCMAKE_CXX_FLAGS=-Wno-c++11-narrowing
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure             # one test: yaml-cpp::test = test/yaml-cpp-tests
```

The googletest summary comes from running `test/yaml-cpp-tests` directly
(ctest prints one test).

**The configure flag.** With the row as first written, the clang++-19
control build failed one TU, `test/binary_test.cpp`:
`std::string input{-58, -1, -99, 109};` is a narrowing conversion on a
target where `char` is unsigned (aarch64), and clang makes it an error
(`constant expression evaluates to -58 which cannot be narrowed to type
'char' [-Wc++11-narrowing]`, 3 errors). kiln reports the same three
errors at the same columns. yaml-cpp is only tested on targets with a
signed `char`. `-Wno-c++11-narrowing` (clang's own escape hatch; kiln
honours it too) lets both compilers accept the TU without changing what
`char` is; on x86_64 it changes nothing. The row in
`scripts/oss/projects.toml` now carries the flag and is `verified = true`.

## Results

TUs are the harness's build-phase compiles (yaml-cpp 32, googletest and
googlemock 4, yaml-cpp's tests 15; CMake probes excluded). Wall is the
whole `oss-build.sh` run (configure, build, test) from `report.json`; Σ TU
wall sums the wrapper's per-TU wall times; peak TU RSS is the wrapper's
`ru_maxrss` (the compiler and the backend or linker it waited for). One
run each.

| Build | TUs ok / failed | Links | Tests (ctest; googletest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| clang++-19, row as first written | 50 / 1 | 0 / 0 | 0/1 (not built) | 49 s † | — | 807 MB |
| clang++-19 (control) | 51 / 0 | 1 / 0 | 1/1; 1011 passed, 8 disabled | 32 s | 91.9 s | 807 MB (`test/integration/gen_emitter_test.cpp`) |
| **kiln, first = final** (base `2fac46d71`) | **51 / 0** | **1 / 0** | **1/1; 1011 passed, 8 disabled** | 34 s | 150.6 s ‡ | **721 MB** (`test/integration/gen_emitter_test.cpp`) |

† Includes the clone. ‡ The kiln build shared the host with four other
harness and CI runs. Its largest TUs track clang (`gen_emitter_test.cpp`
23.8 s vs 23.2 s, `handler_spec_test.cpp` 9.8 s vs 8.1 s), but each small
library TU logged ~2.2 s against clang's ~0.6 s; rerun alone,
`src/emitterstate.cpp -O3 -c` takes 0.46 s with kiln and 0.34 s with
clang, so most of that gap is host load, not compile cost. kiln's peak
RSS is 0.89× clang's (LLVM codegen runs out of process in llc-19).

Warnings: clang prints 14, kiln none. They are diagnostics kiln does not
implement yet, not failures (see "What remains").

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| — | `constant expression evaluates to -58 which cannot be narrowed to type 'char'` (`test/binary_test.cpp:11`) | 1, both compilers | not a kiln bug: yaml-cpp assumes a signed `char`; kiln matches clang; configure flag above | `std::string s{-58};` with unsigned `char` |

No kiln failure clusters and no miscompiles: every yaml-cpp test (parser,
emitter, node, spec examples, the generated emitter tests, base64,
`fptostring`) passes under the kiln build.

## What remains

- **`-Wshadow`** is accepted but not implemented. clang prints 12
  `declaration shadows a variable in namespace 'YAML' [-Wshadow]` in
  yaml-cpp's library TUs (`src/emitterstate.h:28`, `src/emitterutils.h:24`:
  an enumerator of a nested `enum value` shadows a namespace-scope
  enumerator of `EMITTER_MANIP`). Repro (`-Wshadow`; clang warns at 4:33
  and 5:12, kiln is silent):
  ```cpp
  namespace N {
  enum M { Flow, Block };
  int g;
  struct F { enum value { NoType, Flow }; };
  void f(int g) { int Block = 1; (void)Block; (void)g; }
  }
  ```
- **`-Wimplicit-int-float-conversion`** (in `-Wconversion`) is not
  implemented. clang warns twice in googletest's `gtest-printers.h:532`
  and `:544` (`AppropriateResolution<float>`, from `gtest-all.cc`).
  Repro (`-Wconversion`; clang: `implicit conversion from 'int32_t' (aka
  'int') to 'float' may lose precision`, kiln is silent):
  ```cpp
  #include <cstdint>
  bool f(float val, float d) { return static_cast<int32_t>(val / d + 0.5) * d == val; }
  ```
- Variants not run: Debug, C++20/23 (`CMAKE_CXX_STANDARD`), shared
  library (`YAML_CPP_BUILD_SHARED_LIBS=ON`), `YAML_CPP_BUILD_TOOLS=ON`
  (the `parse`/`read`/`sandbox` utilities), `-stdlib=libc++`.
