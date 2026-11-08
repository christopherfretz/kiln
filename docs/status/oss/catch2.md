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
| 1 | `'m_type' is a protected member of 'ResultBase'` (`catch_clara.hpp:285`; `using ResultBase::m_type;` in a class template reached through a dependent base) | 3 library TUs, plus the clara-using test TUs that were not attempted | fixed by WP-680 (same cluster; its code was taken at the chain merge, WP-681 keeps its test) | `tests/conformance/namespace.udecl/protected-member-through-dependent-base.cpp` |
| 2 | `calling a protected constructor of class 'ResultBase'` (clara `BasicResult::ok()`; an inherited protected base constructor used from a static member of the derived class) | the same 3, hidden behind #1 | fixed by WP-680 (same cluster; its code was taken at the chain merge, WP-681 keeps its test) | `tests/conformance/class.inhctor.init/protected-base-ctor-object-class.cpp` |
| 3 | `ambiguous partial specializations of 'UnaryLambdaTraits'` (`R (C::*)(A...) const` vs `R (C::*)(A) const`) | the same 3, hidden behind #1 | fixed by WP-680 (same cluster; its code was taken at the chain merge, WP-681 keeps its test) | `tests/conformance/temp.deduct.type/function-parameter-pack-vs-single.cpp` |
| 4 | `no viable overloaded operator[]` for `custom_index_op<int>{1,2,3}[0, 1, 2]` in C++20 (the parser read an expression-list before C++23) | 1 (`Message.tests.cpp`) | fixed in `80ec5cde1` | `tests/conformance/depr.comma.subscript/comma-index-cxx{17,20,23}.cpp` |
| 5 | `array initializer must be an initializer list` for `TestType with_null_terminator[10] = "abc";` in a template test case | 1 (`ToString.tests.cpp`) | fixed in `94af21839` | `tests/conformance/dcl.init.string/string-literal-dependent-element-{run,errors}.cpp` |
| 6 | `unknown attribute 'clang::lifetimebound' ignored` (1109 times) and `'no_sanitize'` (2 times), all warnings | every TU that includes `catch_stringref.hpp` | fixed by sibling WP-667 (`e20c5e5ac`), taken in by merging origin/main | — |

Behaviours and rules: `spec/landed/sema/WP-681.md`.

## What remains

- **Nothing for this configuration.** The build is at parity with no
  outstanding cluster.
- **Not tried:** the Debug build, C++26 (`-DCMAKE_CXX_STANDARD`),
  `-DCATCH_BUILD_EXAMPLES=ON` and the extra tests
  (`-DCATCH_BUILD_EXTRA_TESTS=ON`). C++23 and `-DCATCH_ENABLE_WERROR=ON`
  are below.

## C++23 and `-DCATCH_ENABLE_WERROR=ON` (WP-873, from WP-687)

Two more harness rows share the `catch2` checkout:
`[catch2_cxx23]` (`cxx_std = 23`, otherwise as `[catch2]`) and
`[catch2_werror]` (`-DCATCH_ENABLE_WERROR=ON`, C++20). Kiln is main
`a450c1a89` plus WP-873, date 2026-11-08, host and stdlib as above.

```sh
scripts/oss/oss-build.sh catch2_cxx23 [--compiler clang]
scripts/oss/oss-build.sh catch2_werror [--compiler clang]
```

| Build | TUs ok / failed | Link | Tests | Warnings | wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| **kiln, C++23** | **167 / 0** | **1 / 0** | **81 / 81** | **0** | 176 s | **119 MiB** (`MatchersRanges.tests.cpp`) |
| clang++-19, C++23 | 166 / 1 | — | 1 / 81 | 0 | — | — (report overwritten by the clang 22 run) |
| **clang++-22, C++23 (the control)** | **167 / 0** | **1 / 0** | **81 / 81** | **0** | 243 s | **456 MiB** (`InternalBenchmark.tests.cpp`) |
| **kiln, WERROR** | **167 / 0** | **1 / 0** | **81 / 81** | **0** | 119 s | **120 MiB** (`MatchersRanges.tests.cpp`) |
| **clang++-19, WERROR** | **167 / 0** | **1 / 0** | **81 / 81** | **0** | 99 s | **433 MiB** (`InternalBenchmark.tests.cpp`) |

- **Both configurations pass with no kiln change.** In C++23,
  `Message.tests.cpp` compiles its multidimensional `operator[]`
  `CAPTURE` case (`__cpp_multidimensional_subscript`; the parser fix is
  cluster 4 above). Under `-DCATCH_ENABLE_WERROR=ON` every TU gets
  `-Werror` with Catch2's warning set (`-Wall -Wextra -Wpedantic`,
  `-Wshadow`, `-Wold-style-cast`, `-Wunreachable-code-aggressive` and
  about thirty more), and kiln emits no warning. The C++20 build also
  runs the new definition-time `-Winvalid-constexpr` check (an error
  before C++23) on every constexpr function without a false positive.
- **clang 19 cannot build the C++23 configuration.** It rejects
  `tests/SelfTest/IntrospectiveTests/UniquePtr.tests.cpp:139`
  (`REQUIRE(*ptr == std::tuple<int, double, int>{1, 2., 3})`): libstdc++
  14's C++23 `tuple` `operator<=>` for tuple-like operands instantiates
  `std::tuple_size<Catch::Decomposer>`, a hard error in clang 19. clang 22,
  GCC 14 and GCC 16 accept it, and so does kiln. So the C++23 row's control
  is clang 22 (`control_cxx = "clang++-22"`). Not a kiln breakage.
- Wall time includes CTest (the ApprovalTests Python script) and host
  load from eight concurrent builds, as above.

## libc++ (WP-694)

Built and tested with `-stdlib=libc++` (the system libc++ 19) by both kiln++ and the clang++-19 control: `scripts/oss/oss-build.sh --stdlib libc++ <project>`, report via `scripts/oss/oss-report.py --stdlib libc++ --perf <project>`. The clang-control build tree was deleted after these numbers were recorded. Wall-time differences are dominated by file-system syscalls on the bind mount (see the perf finding in [../oss.md](../oss.md#libc-results-wp-694)).

<!-- oss:catch2:libcxx -->
### catch2 (v3.16.1), libc++

| | kiln | clang-19 |
|---|---|---|
| configure | ok | ok |
| build | ok | ok |
| TUs ok / failed | 167 / 0 | 167 / 0 |
| links ok / failed | 1 / 0 | 1 / 0 |
| tests passed / total | 81 / 81 | 81 / 81 |
| wall | 529 s | 466 s |
| peak TU RSS | 117 MB | 444 MB |

Failure clusters (kiln): none.

Performance over the 167 TUs that compiled with both (CPU time, peak RSS of the compiler and its backend):
geomean kiln/clang-19 time **1.34x**, RSS **0.39x**; total 204 s vs 156 s; peak TU RSS 117 MB vs 444 MB.

| heaviest TUs (by clang-19 time) | clang-19 s | kiln s | time x | clang-19 MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `$KILN/.oss/src/catch2/tests/SelfTest/IntrospectiveTests/InternalBenchmark.tests.cpp` | 3.5 | 1.9 | 0.53 | 444 | 77 | 0.17 |
| `$KILN/.oss/src/catch2/tests/SelfTest/UsageTests/MatchersRanges.tests.cpp` | 2.9 | 3.6 | 1.25 | 216 | 117 | 0.54 |
| `$KILN/.oss/src/catch2/tests/SelfTest/UsageTests/Matchers.tests.cpp` | 2.7 | 3.4 | 1.24 | 217 | 116 | 0.53 |
| `$KILN/.oss/src/catch2/tests/SelfTest/IntrospectiveTests/CmdLine.tests.cpp` | 2.4 | 2.9 | 1.22 | 219 | 95 | 0.43 |
| `$KILN/.oss/src/catch2/tests/SelfTest/IntrospectiveTests/GeneratorsImpl.tests.cpp` | 2.3 | 2.7 | 1.18 | 213 | 102 | 0.48 |

TUs over 1.2x on time or RSS: 141 of the 151 that take clang-19 at least 0.5 s.
- `$KILN/.oss/src/catch2/tests/SelfTest/UsageTests/ToStringByte.tests.cpp`: time 1.4 / 0.8 s = 1.70x, RSS 66 / 179 MB = 0.37x
- `$KILN/.oss/src/catch2/tests/SelfTest/UsageTests/ToStringOptional.tests.cpp`: time 1.5 / 0.9 s = 1.70x, RSS 67 / 183 MB = 0.37x
- `$KILN/.oss/src/catch2/tests/SelfTest/UsageTests/ToStringPair.tests.cpp`: time 1.4 / 0.9 s = 1.69x, RSS 67 / 184 MB = 0.36x
- `$KILN/.oss/src/catch2/tests/SelfTest/UsageTests/ToStringTuple.tests.cpp`: time 1.4 / 0.9 s = 1.64x, RSS 68 / 186 MB = 0.37x
- `$KILN/.oss/src/catch2/tests/SelfTest/UsageTests/ToStringChrono.tests.cpp`: time 1.4 / 0.9 s = 1.64x, RSS 67 / 183 MB = 0.37x
<!-- /oss:catch2:libcxx -->
