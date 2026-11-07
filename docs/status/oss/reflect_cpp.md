# reflect-cpp with kiln++ (WP-792)

This page covers reflect-cpp **v0.27.1** (tag `v0.27.1`). It was built
with kiln++ as `CMAKE_CXX_COMPILER` through reflect-cpp's own CMake build
and test suite. The control is a clang++-19 build of the same tree, and
the harness is M8's (`scripts/oss/`, spec/landed/harness/WP-666.md,
WP-792).

reflect-cpp is C++20 field reflection without P2996. Member names come
from `__PRETTY_FUNCTION__` of a function template whose template argument
is a pointer to the member of a fake object. Member counts come from
structured bindings and aggregate-initialization probes. Only the JSON
backend is built, using the bundled yyjson. googletest is a dependency
built with the same compiler. Precompiled headers are off for both
compilers. The tests' `-ggdb -Werror` are overridden by
`-g0 -Wno-error` in `CMAKE_CXX_FLAGS_RELEASE`.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.** 317 of 321 TUs compile, all 4 executables link and
219 of 220 ctest tests pass, under both compilers. Four test TUs fail
under both: `tests/json/test_json_schema{5,7,8}.cpp` and
`tests/json/test_json_schema_default_vals.cpp`. They use a name `json`
that the TU does not declare. Because of them,
`reflect-cpp-json-tests_NOT_BUILT` is the one failing test.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh reflect_cpp --compiler clang
scripts/oss/oss-build.sh reflect_cpp
scripts/oss/oss-report.py --perf reflect_cpp
```

```sh
cmake --fresh -S .oss/src/reflect_cpp -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DREFLECTCPP_BUILD_TESTS=ON -DREFLECTCPP_USE_VCPKG=OFF \
  -DCMAKE_DISABLE_PRECOMPILE_HEADERS=ON \
  "-DCMAKE_CXX_FLAGS_RELEASE=-O3 -DNDEBUG -g0 -Wno-error"
nice -n 10 make -k
ctest --output-on-failure -j 4 --timeout 300
```

## Results

Time is the wrapper's CPU time (user plus system) per TU. RSS is the
TU's `ru_maxrss`, which includes the backend. Each build was run once.

| Build | TUs ok / failed | Links | Tests | Σ TU CPU | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (WP-792 base) | 1 / 1 (`make -k` stopped at the library) | 0 | 0/6 | 12 s | 86 MB |
| clang++-19 (control) | 317 / 4 | 4 / 4 | 219/220 | 884 s | 549 MB (`src/reflectcpp_json.cpp`) |
| **kiln, final** | **317 / 4** | **4 / 4** | **219/220** | **804 s** | **250 MB** (`src/reflectcpp_json.cpp`) |

Over the 317 TUs that compile with both, kiln/clang has a geomean of
**0.91x time** and **0.39x RSS**. The five heaviest TUs:

| TU | clang s | kiln s | time x | clang MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `src/reflectcpp_json.cpp` | 10.6 | 8.8 | 0.83 | 549 | 250 | 0.46 |
| `tests/json/test_many_fields.cpp` | 4.9 | 4.2 | 0.84 | 460 | 194 | 0.42 |
| `tests/json/test_bitset.cpp` | 4.9 | 4.5 | 0.91 | 364 | 146 | 0.40 |
| `tests/json/test_fail_fast.cpp` | 4.8 | 3.9 | 0.81 | 330 | 127 | 0.38 |
| `tests/cli/test_wide_char.cpp` | 4.8 | 4.4 | 0.92 | 326 | 127 | 0.39 |

No TU is over 1.2x on time or RSS, among the 315 that take clang at
least 0.5 s.

## Clusters

Correctness. The failures of the first build and of the builds after
it, by first diagnostic. Later rows were exposed by fixing earlier ones.

| # | Signature | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `T::template contains<…>()` was rejected as not naming a template when the overload set of `contains` put a non-template overload before the function template (`rfl::Literal`) | 32 | fixed, `e511f3693` (sema) | `tests/conformance/temp.names/template-kw-mixed-overload-set.cpp` |
| 2 | `make_field<"name">(…)` was ambiguous. A pointer to a string literal was accepted as the argument of the `template <auto p>` overload, so that overload was not dropped beside the class-type NTTP one ([temp.arg.nontype]/6) | 19 | fixed, `b92557498` (sema) | `tests/conformance/temp.arg.nontype/string-literal-typeid-argument.cpp`, `…/string-literal-argument-sfinae-run.cpp` |
| 3 | **static_assert failure on valid code**: the field name of a 2-D C-array member came out as `c[0][0]`, because `__PRETTY_FUNCTION__` printed `&fake.c` of a const fake object as `&fake.c[0][0]` (`tests/json_c_arrays_and_inheritance/test_c_array_class5.cpp`) | 1 | fixed, `0819b92ef` (ast) | `tests/conformance/dcl.fct.def.general/predefined-pointer-to-array-member-arg-run.cpp` |
| 4 | `json` is not declared in the TU (an upstream test bug) | 4 | fails under clang too | — |

Row 3 is a miscompile of a constant expression. The program was valid
and kiln rejected it through the program's own `static_assert`.

## What remains

Nothing that is kiln's. Row 4 is upstream.
