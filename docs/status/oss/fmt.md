# fmt with kiln++ (WP-667)

{fmt} **12.2.0** (tag `12.2.0`, commit `1be298e`), built with kiln++ as
`CMAKE_CXX_COMPILER` through fmt's own CMake build and its CTest suite,
against a clang++-19 control build of the same tree. Host: aarch64 Linux,
libstdc++ 14. Date: 2026-11-06.

**Result: parity.** Every fmt TU compiles, every test passes, and the build
is warning-free under both compilers, in the default, `FMT_PEDANTIC`, Debug
and C++26 configurations. kiln needed two fixes (one sema, one driver), and
neither changes generated code.

## Commands

The compiler is a wrapper, `kxx`. It runs `$KXX` (kiln++ or clang++-19) under
`ulimit -v 3000000` and `timeout 600`, and uses `/usr/bin/time -f '%e %M'` to
log one line per invocation: source, rc, wall, max RSS and the first error.
For kiln++, the environment sets `KILN_RESOURCE_DIR=<kiln checkout>/resource`.

```sh
git clone --depth 1 --branch 12.2.0 https://github.com/fmtlib/fmt src
mkdir build && cd build
KXX=kiln++ cmake -G "Unix Makefiles" -DCMAKE_CXX_COMPILER=$PWD/../kxx \
  -DCMAKE_C_COMPILER=clang-19 -DFMT_TEST=ON -DFMT_DOC=OFF \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_STANDARD=20 ../src
KXX=kiln++ nice -n 10 make -k
timeout 1800 ctest --output-on-failure
```

The control build is the same with `KXX=clang++-19`. The variant builds add:

- `-DFMT_PEDANTIC=ON`, which adds `compile-error-test`, `find-package-test`, `add-subdirectory-test` and `static-export-test`.
- `-DCMAKE_BUILD_TYPE=Debug`.
- `-DCMAKE_CXX_STANDARD=26`.

## Results

"TUs" counts fmt's own 50 compiled sources (library, gtest, tests), not
CMake's probes. CMake's `detect-stdfs.cc` probe fails by design under both
compilers (it `#error`s to report which stdfs library to link) and is not
counted. Wall is `make`'s; the TU sum is the sum of the per-TU wall times
from the wrapper.

| Build | TUs ok / failed | Tests | Warnings | make wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `f3751eadd`) | 50 / 0 | 21/21 | 48 (`-Wunknown-attributes`) | 23 s | 151.2 s | 310 MiB (`printf-test.cc`) |
| clang++-19, first | 50 / 0 | 21/21 | 0 | 25 s | 138.5 s | 387 MiB (`printf-test.cc`) |
| **kiln, final** | **50 / 0** | **21/21** | **0** | 29 s | 165.8 s | **310 MiB** (`printf-test.cc`) |
| **clang++-19, final** | **50 / 0** | **21/21** | **0** | 20 s | 140.7 s | **387 MiB** (`printf-test.cc`) |
| kiln, `FMT_PEDANTIC` | 50 / 0 | 25/25 | 0 | 21 s | — | — |
| clang++-19, `FMT_PEDANTIC` | 50 / 0 | 25/25 | 0 | 19 s | — | — |
| kiln, Debug | 50 / 0 | 21/21 | 0 | 12 s | 93.0 s | 162 MiB (`format-test.cc`) |
| clang++-19, Debug | 50 / 0 | 21/21 | 0 | 13 s | 78.9 s | 357 MiB (`chrono-test.cc`) |
| kiln, C++26 | 50 / 0 | 21/21 | 0 | 27 s | 168.9 s | 312 MiB (`printf-test.cc`) |
| clang++-19, C++26 | 50 / 0 | 21/21 | 0 | 34 s | 157.4 s | 388 MiB (`printf-test.cc`) |

Notes on these numbers:

- **CPU vs memory.** kiln uses about 1.18× clang's CPU on the Release build and 0.80× its peak RSS. In Debug the gap is 1.18× CPU and 0.45× RSS.
- **Wall times are noisy.** `make` wall is serial on a shared host, so it varies by several seconds between runs.
- **kiln++ is a debug build of the driver.** Front-end time dominates, and LLVM codegen runs out of process in llc-19.
- **`compile-error-test` is identical.** In the PEDANTIC build, kiln and clang report the same first error on all 13 of fmt's expected-failure snippets.
  - Several of these stop at the snippets' `#ifdef FMT_HAS_CONSTEVAL ... #else #error` guard under both compilers.
  - fmt's consteval format-string checks were therefore also compared directly. The primary errors are identical, and only the wording of the "in call to" notes differs (see below).

## Clusters

| # | Cluster | Hits (first build) | Kind | Resolution |
|---|---|---|---|---|
| 1 | `__attribute__((no_sanitize("undefined")))` and `[[clang::lifetimebound]]` warned `unknown attribute` | 40 + 8 warnings | kiln bug: attributes kiln reports via `__has_attribute` but did not know in sema | fixed, commit "sema: accept clang's sanitizer and lifetimebound attributes silently" — `spec/landed/sema/WP-667.md`, `tests/conformance/dcl.attr.grammar/{sanitizer-*,gnu-no-sanitize-clang{19,22}}.cpp` |
| 2 | CMake compiler probes: `-print-resource-dir` rejected; `-v` link printed no linker line, so no linker id, implicit link libs/dirs or library architecture | configure-time | kiln bug: driver | fixed, commit "driver: -print-resource-dir, and -v reaches the linking clang++" — `spec/landed/driver/WP-667.md`, `crates/kiln_driver/tests/{driver,codegen}.rs` |
| 3 | CMake's IPO probe (`check_ipo_supported`) fails: `-flto=thin` unknown argument | PEDANTIC configure only | kiln gap: driver | follow-up (below) |
| 4 | consteval "in call to" note wording differs from clang | diagnostics only | wording long tail | follow-up (below) |
| 5 | missing `-Winvalid-constexpr` ("constexpr function never produces a constant expression") | diagnostics only | known gap, spec/sema.md | follow-up (below) |

After fix 2, the `CMakeCXXCompiler.cmake` CMake writes for kiln++ differs
from clang++-19's only in the reported version (22.1.8, kiln's default
`-fclang-compat`), in the version-matched `ar`/`ranlib`/`clang-scan-deps`
CMake then picks, and in the include and resource directories.

## What remains

- **`-flto` / `-flto=thin`** (driver). kiln++ rejects these, so `check_ipo_supported()` reports no IPO, and any project that sets `INTERPROCEDURAL_OPTIMIZATION` fails to build.
  - Fixing it means emitting LLVM bitcode objects under `-c -flto` and passing `-flto` through to the `clang++` link.
  - Needs its own WP.
- **consteval call-note wording** (sema / Oven note printer). Repro: put a `fail()` call (a non-constexpr function) under a consteval constructor that takes `const char (&)[N]`. The differences from clang are:
  - Template arguments in the notes keep alias sugar: `parse<char, checker>` where clang prints `parse<char, S<5>>`.
  - NTTP values have no integer suffix: `F<5>` where clang prints `F<5UL>`.
  - String literals print `\0`, where clang prints `"a\000{}"`.
  - Calls to a qualified member (`return B::next();`) have the note column on the qualifier (`B`), where clang puts it on the member name.
  - In fmt's own `FMT_STRING` errors, a local class is printed as `struct id_adapter &` where clang prints `id_adapter &`. This is not reproduced outside fmt yet.

  Primary errors, locations and note order already match clang. This is the deferred wording long tail.
- **`-Winvalid-constexpr`.** clang diagnoses at definition time that a constexpr function can never be constant (for example, `constexpr int next() { fail(); return 0; }`). kiln does not; this is already listed as a gap in spec/sema.md.
- **No subject or argument checks** for the attributes from fix 1. For example, `no_sanitize` on a variable, or a non-string argument, is accepted silently. clang diagnoses both.
- **Not exercised:**
  - fmt's C++20 module build (`FMT_MODULE=ON`). It needs CMake's Ninja generator, which this run may not use.
  - `FMT_FUZZ`.
  - `FMT_OS=OFF` and the header-only (`fmt::fmt-header-only`) consumer, apart from what `FMT_PEDANTIC`'s `add-subdirectory-test` / `find-package-test` cover.

## libc++ (WP-694)

Built and tested with `-stdlib=libc++` (the system libc++ 19) by both kiln++ and the clang++-19 control: `scripts/oss/oss-build.sh --stdlib libc++ <project>`, report via `scripts/oss/oss-report.py --stdlib libc++ --perf <project>`. The clang-control build tree was deleted after these numbers were recorded. Wall-time differences are dominated by file-system syscalls on the bind mount (see the perf finding in [../oss.md](../oss.md#libc-results-wp-694)).

Before the fix kiln++ failed 2 of 50 TUs (both built from `test/ranges-test.cc`) with "call to consteval function `fmt::fstring<tuple_join_view<…>>` is not a constant expression": under `-std=gnu++20` the implicit default constructor of a class with a libc++ `std::tuple` member was not constexpr, because kiln did not count the tuple's constexpr default constructor template as a constexpr default constructor ([class.default.ctor]/1). Fixed in `has_constexpr_default_ctor` (crates/kiln_sema/src/class/special/implicit.rs), test `tests/conformance/class.default.ctor/constexpr-ctor-template.cpp`.

<!-- oss:fmt:libcxx -->
### fmt (12.1.0), libc++

| | kiln | clang-19 |
|---|---|---|
| configure | ok | ok |
| build | ok | ok |
| TUs ok / failed | 50 / 0 | 50 / 0 |
| links ok / failed | 22 / 0 | 22 / 0 |
| tests passed / total | 21 / 21 | 21 / 21 |
| wall | 95 s | 45 s |
| peak TU RSS | 351 MB | 434 MB |

Failure clusters (kiln): none.

Performance over the 30 TUs that compiled with both (CPU time, peak RSS of the compiler and its backend):
geomean kiln/clang-19 time **1.42x**, RSS **0.44x**; total 140 s vs 105 s; peak TU RSS 351 MB vs 434 MB.

| heaviest TUs (by clang-19 time) | clang-19 s | kiln s | time x | clang-19 MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `$KILN/.oss/src/fmt/test/printf-test.cc` | 13.7 | 16.4 | 1.19 | 434 | 351 | 0.81 |
| `$KILN/.oss/src/fmt/test/chrono-test.cc` | 11.0 | 14.7 | 1.34 | 373 | 280 | 0.75 |
| `$KILN/.oss/src/fmt/test/format-test.cc` | 9.4 | 12.5 | 1.32 | 362 | 261 | 0.72 |
| `$KILN/.oss/src/fmt/test/base-test.cc` | 7.0 | 9.5 | 1.36 | 415 | 190 | 0.46 |
| `$KILN/.oss/src/fmt/test/gtest/gmock-gtest-all.cc` | 5.4 | 6.5 | 1.19 | 289 | 154 | 0.53 |

TUs over 1.2x on time or RSS: 26 of the 30 that take clang-19 at least 0.5 s.
- `$KILN/.oss/src/fmt/test/test-main.cc`: time 1.9 / 1.0 s = 1.97x, RSS 66 / 194 MB = 0.34x
- `$KILN/.oss/src/fmt/test/util.cc`: time 1.0 / 0.6 s = 1.77x, RSS 64 / 157 MB = 0.41x
- `$KILN/.oss/src/fmt/test/color-test.cc`: time 3.1 / 1.9 s = 1.68x, RSS 84 / 217 MB = 0.39x
- `$KILN/.oss/src/fmt/test/gtest-extra-test.cc`: time 3.2 / 2.0 s = 1.61x, RSS 93 / 215 MB = 0.43x
- `$KILN/.oss/src/fmt/test/args-test.cc`: time 2.5 / 1.5 s = 1.61x, RSS 76 / 212 MB = 0.36x
<!-- /oss:fmt:libcxx -->
