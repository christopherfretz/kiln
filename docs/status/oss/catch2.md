# Catch2 with kiln++ (WP-681)

Catch2 **v3.16.1** (tag `v3.16.1`, commit `0809213`), built with kiln++
through Catch2's own CMake build and its CTest suite by the `scripts/oss`
harness. The control is a clang++-19 build of the same tree. Host: aarch64
Linux, libstdc++ 14. Date: 2026-11-07.

**Result: parity.** All 167 TUs compile, the one link succeeds, all 81 CTest
cases pass, and the build log has no warnings under either compiler. Getting
there took five kiln fixes, all in the front end (four in sema, one in the
parser). None of them is a miscompile.

## Commands

The harness entry is `scripts/oss/projects.toml` `[catch2]`, now marked
`verified = true`.

```sh
scripts/oss/oss-build.sh catch2 --compiler clang   # control
scripts/oss/oss-build.sh catch2                    # kiln++
scripts/oss/oss-report.py catch2
```

`oss-build.sh` runs the following. `kxx` is the harness wrapper, which logs
each compile to `compile.jsonl`.

```sh
git clone --depth 1 --branch v3.16.1 https://github.com/catchorg/Catch2 .oss/src/catch2
nice -n 10 cmake --fresh -S .oss/src/catch2 -B .oss/build/catch2/kiln -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=<kxx> -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DCATCH_DEVELOPMENT_BUILD=ON -DCATCH_BUILD_TESTING=ON \
  -DCATCH_ENABLE_WERROR=OFF -DCATCH_BUILD_EXAMPLES=OFF
nice -n 10 make -k -C .oss/build/catch2/kiln          # jobserver pool
cd .oss/build/catch2/kiln && timeout 1800 nice -n 10 ctest --output-on-failure
```

## Results

How the columns are counted:

- **TUs** are the build-phase compiles. CMake's configure probes are not counted.
- **Σ TU wall** is the sum of the per-TU wall times.
- **wall** is the harness's `wall_s`, which covers configure, build and test.

| Build | TUs ok / failed | Link | Tests | Warnings | wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|---|
| kiln, first (main `7c5998af2`) | 104 / 3 (60 not attempted) | — | 1 / 81 | — | 20 s | — | 90 MiB |
| kiln, after fixes, before merging WP-667 | 167 / 0 | 1 / 0 | 81 / 81 | 1111 (`-Wunknown-attributes`) | 110 s | — | 118 MiB |
| **kiln, final** | **167 / 0** | **1 / 0** | **81 / 81** | **0** | 212 s | 205 s | **118 MiB** (`MatchersRanges.tests.cpp`) |
| **clang++-19** | **167 / 0** | **1 / 0** | **81 / 81** | **0** | 133 s | 184 s | **432 MiB** (`InternalBenchmark.tests.cpp`) |

Notes on these numbers:

- **The first build stopped early.** Three library TUs (`catch_clara.cpp`,
  `catch_session.cpp`, `catch_commandline.cpp`) failed. That left
  `libCatch2` unbuilt, so the test TUs that depend on it were never
  compiled and the SelfTest binary did not link. One CTest case passed.
- **Wall time is dominated by CTest and host load.** `ApprovalTests` runs
  SelfTest under every reporter through a Python script. It took 122 s in
  the final kiln run and 27.5 s in the clang run. Rerunning it back to back
  on the same host gave 47 s for kiln and 68 s for clang, so the gap is host
  load. Running SelfTest directly takes 0.18 s with either compiler's build.
  The Σ TU wall is the comparable number: kiln uses 1.11× clang's time.
- **Memory.** kiln's peak TU RSS is 0.27× clang's.

## Clusters

| # | Signature (first error) | TUs | Status | Repro / test |
|---|---|---|---|---|
| 1 | `'m_type' is a protected member of 'ResultBase'` (`catch_clara.hpp:285`; `using ResultBase::m_type;` in a class template reached through a dependent base) | 3 library TUs, plus the clara-using test TUs that were not attempted | fixed in `6d39fb6f0` | `tests/conformance/namespace.udecl/protected-member-through-dependent-base.cpp` |
| 2 | `calling a protected constructor of class 'ResultBase'` (clara `BasicResult::ok()`; an inherited protected base constructor used from a static member of the derived class) | the same 3, hidden behind #1 | fixed in `2f9055123` | `tests/conformance/class.inhctor.init/protected-base-ctor-object-class.cpp` |
| 3 | `ambiguous partial specializations of 'UnaryLambdaTraits'` (`R (C::*)(A...) const` vs `R (C::*)(A) const`) | the same 3, hidden behind #1 | fixed in `fd45c150e` | `tests/conformance/temp.deduct.type/function-parameter-pack-vs-single.cpp` |
| 4 | `no viable overloaded operator[]` for `custom_index_op<int>{1,2,3}[0, 1, 2]` in C++20 (the parser read an expression-list before C++23) | 1 (`Message.tests.cpp`) | fixed in `80ec5cde1` | `tests/conformance/depr.comma.subscript/comma-index-cxx{17,20,23}.cpp` |
| 5 | `array initializer must be an initializer list` for `TestType with_null_terminator[10] = "abc";` in a template test case | 1 (`ToString.tests.cpp`) | fixed in `94af21839` | `tests/conformance/dcl.init.string/string-literal-dependent-element-{run,errors}.cpp` |
| 6 | `unknown attribute 'clang::lifetimebound' ignored` (1109 times) and `'no_sanitize'` (2 times), all warnings | every TU that includes `catch_stringref.hpp` | fixed by sibling WP-667 (`e20c5e5ac`), taken in by merging origin/main | — |

Behaviours and rules: `spec/landed/sema/WP-681.md`.

## What remains

- **Nothing for this configuration.** The build is at parity with no
  outstanding cluster.
- **Not tried:** the Debug build, C++23 or C++26
  (`-DCMAKE_CXX_STANDARD`), `-DCATCH_BUILD_EXAMPLES=ON`, the extra tests
  (`-DCATCH_BUILD_EXTRA_TESTS=ON`) and `-DCATCH_ENABLE_WERROR=ON`.
  - The C++23 build is the one that exercises the multidimensional
    `operator[]` overload in `Message.tests.cpp`.
  - kiln is warning-free here, so `CATCH_ENABLE_WERROR=ON` is expected to
    pass. It was not run.
