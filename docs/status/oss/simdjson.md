# simdjson with kiln++ (WP-691)

simdjson **v4.2.4** (tag `v4.2.4`, commit `980f2ad`), built with kiln++
through simdjson's own CMake build in developer mode and its CTest suite by
the `scripts/oss` harness. The control is a clang++-19 build of the same
tree. Host: aarch64 Linux, libstdc++ 14. Date: 2026-11-07.

**Result: one test short of parity.** All 135 TUs compile and all 94 links
succeed. 116 of 117 CTest cases pass; clang passes 117. The one failure
is a negative compile test that relies on `-Wuninitialized` under
`-Werror`, a flow analysis kiln does not have yet (cluster 4). Three kiln
fixes got the build here, all in the front end (two in the parser, one in
sema). None of them is a miscompile: the NEON kernels are built and
selected at run time (`builtin` and active implementation are both
`arm64`), and every runtime test passes.

## Commands

The harness entry is `scripts/oss/projects.toml` `[simdjson]`, now marked
`verified = true`.

```sh
scripts/oss/oss-build.sh simdjson --compiler clang   # control
scripts/oss/oss-build.sh simdjson                    # kiln++
scripts/oss/oss-report.py simdjson
```

`oss-build.sh` runs the following. `kxx` is the harness wrapper, which logs
each compile to `compile.jsonl`.

```sh
git clone --depth 1 --branch v4.2.4 https://github.com/simdjson/simdjson .oss/src/simdjson
nice -n 10 cmake --fresh -S .oss/src/simdjson -B .oss/build/simdjson/kiln -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=<kxx> -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DSIMDJSON_DEVELOPER_MODE=ON -DSIMDJSON_ENABLE_THREADS=ON -DSIMDJSON_EXCEPTIONS=ON
nice -n 10 make -k -C .oss/build/simdjson/kiln          # jobserver pool
cd .oss/build/simdjson/kiln && timeout 1800 nice -n 10 ctest --output-on-failure -E 'checkperf|fuzz'
```

Developer mode overrides the standard: `cmake/developer-options.cmake` sets
`CMAKE_CXX_STANDARD` from `SIMDJSON_CXX_STANDARD` (default 17), so 132 of
the 135 build TUs compile as `-std=c++17` and 3 as `-std=c++20`. It also
adds `-Werror -Wall -Wextra -Weffc++ -Wshadow …`. The CTest suite compiles
another 46 TUs at test time (C++11, C++14 and C++17 single-header and
negative compile tests).

## Results

How the columns are counted:

- **TUs** are the build-phase C++ compiles. CMake's configure probes and the
  CTest-time compiles are not counted. C sources (ujson4c, jsmn, cJSON …)
  are compiled by clang-19 in both builds.
- **Σ TU wall** is the sum of the per-TU wall times.
- **wall** is the harness's `wall_s`, which covers configure, build and test.

| Build | TUs ok / failed | Links ok / failed | Tests | C++ warnings | wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|---|
| kiln, first (chain tip `2fac46d71`) | 133 / 2 | 92 / 0 (2 not attempted) | 106 / 117 | 0 | 113 s | — | 203 MiB (`benchfeatures.cpp`) |
| **kiln, final** (`3a57abd30`) | **135 / 0** | **94 / 0** | **116 / 117** | **0** | 132 s | 282 s | **229 MiB** (`bench_ondemand.cpp`) |
| **clang++-19** | **135 / 0** | **94 / 0** | **117 / 117** | **0** | 125 s | 353 s | **331 MiB** (`bench_ondemand.cpp`) |

Notes on these numbers:

- **The build log has 11 warnings in both builds.** All are `'fastcall'
  calling convention is not supported for this target` from the C file
  `ultrajsondec.c`, which clang-19 compiles in both builds.
- **Time.** kiln's Σ TU wall is 0.80× clang's. The total wall is
  dominated by CTest and host load.
- **Memory.** kiln's peak TU RSS is 0.69× clang's, on the same TU
  (`benchmark/bench_ondemand.cpp`, which includes rapidjson, sajson,
  nlohmann::json, yyjson and Boost.JSON alongside simdjson).

## Clusters

| # | Signature (first error) | TUs | Status | Repro / test |
|---|---|---|---|---|
| 1 | `expected expression` at `__attribute__((__fallthrough__));` (`simdjson_fallthrough` in `common_defs.h:309`, the GNU spelling used below C++17 by the C++11/14 test builds) | 9 CTest-time compiles (9 tests) | fixed in `24458e99c` | `tests/conformance/dcl.attr.fallthrough/gnu-attribute-null-stmt-cxx11.cpp` |
| 2 | `'GenericValue' does not refer to a value` at `alignas(GenericValue)` (rapidjson `document.h:1237`; the injected-class-name of a class template, named inside its own template) | 1 (`benchmark/bench_ondemand.cpp`) | fixed in `691191240` | `tests/conformance/dcl.align/alignas-injected-class-name.cpp` |
| 3 | no matching `operator""_padded` for `u8"…"_padded` in C++17 (kiln typed `u8` literals as `char8_t` arrays before C++20) | 1 (`examples/quickstart/…/ondemand_readme_examples.cpp`, so 1 test Not Run) | fixed in `3a57abd30` | `tests/conformance/lex.string/utf8-literal-char-{pre-cxx20,cxx20}.cpp` |
| 4 | `iterate_char_star_should_not_compile` compiles: clang rejects `const char* json; parser.iterate(json, strlen(json));` with `-Werror,-Wuninitialized` ("variable 'json' is uninitialized when used here"); kiln has no uninitialized-use flow analysis | 1 test | open: follow-up to the uninitialized-values analysis (WP-443 UNINIT-a, `docs/design/uninitialized-values.md`) | `int g(int); int f() { int x; return g(x); }` with `-Wuninitialized`: clang warns, kiln is silent |

Behaviours and rules: `spec/landed/parse/WP-691.md`.

**WP-933 (2026-11-08):** the re-run after WP-862 was deferred for lack of
disk (the shared volume was below its free-space floor). Cluster 4 stays
open until a run confirms it. kiln now warns on the direct by-value use
(`int x; g(x);`), but the iterate test's own pattern was not checked.

## What remains

- **Cluster 4**, the `-Wuninitialized` flow analysis. It is a missing
  warning, not a kiln bug in what it accepts. When it lands, the build
  reaches 117 / 117.
- **`-fchar8_t` / `-fno-char8_t` are ignored.** The `u8` literal element
  type follows only `-std` (lexer `utf8_is_char8`, sema
  `string_literal_elem`). simdjson does not use the flags.
- **Not tried:** `SIMDJSON_CXX_STANDARD=20` or `26` (the project's C++20
  `std::ranges`/concepts paths and its C++26 static-reflection paths),
  the Debug build, `checkperf` and the fuzzers (excluded by the test
  filter), and the x86_64 kernels (AVX2/AVX-512 intrinsics headers; this
  host is aarch64).
