# fmt_module: {fmt} as a C++20 named module (WP-1148)

{fmt} **12.1.0** (tag `12.1.0`, commit `407c905`) built with
`-DFMT_MODULE=ON`: fmt's own CMake build makes `src/fmt.cc`
(`export module fmt;`) a `FILE_SET CXX_MODULES` library, so CMake drives
the build through Ninja and a dependency scanner. kiln++ is
`CMAKE_CXX_COMPILER` and kiln-scan-deps the scanner; the control is
clang++-19 with clang-scan-deps-19. Host: aarch64 Linux, CMake 3.31.6,
Ninja 1.12.1, libstdc++ 14 and libc++ 19. Date: 2026-11-09.

**Result: at parity under both standard libraries** (libstdc++ since
WP-1156, 2026-11-09: the importer was rejected, below). The scanner, the
module library and the importer build, and the test passes.

## Commands

```sh
cargo build --profile harness -p kiln_driver   # kiln++, kiln-scan-deps
for s in libstdc++ libc++; do
  scripts/oss/oss-build.sh fmt_module --compiler clang --stdlib $s
  scripts/oss/oss-build.sh fmt_module --stdlib $s
done
scripts/oss/oss-report.py --perf fmt_module
scripts/oss/oss-report.py --perf --stdlib libc++ fmt_module
```

The row (`scripts/oss/projects.toml`) is `modules = true`, so
`oss-build.sh` configures with `-G Ninja` and
`-DCMAKE_CXX_COMPILER_CLANG_SCAN_DEPS=<scanner>` (kiln-scan-deps beside
kiln++, clang-scan-deps-19 beside clang++-19), and with
`-DCMAKE_CXX_FLAGS=-stdlib=libc++` for libc++ (the scanner is called
without the kiln-cxx wrapper). Ninja is not a jobserver client, so the
harness caps it at 4 jobs.

fmt's test suite does not build in module mode
(`test/CMakeLists.txt`: "Testing is incompatible with build mode
'module'"), and upstream's `test/module-test.cc` no longer compiles
against 12.1.0 even with clang 19 (`make_format_args` of rvalues,
`fmt::is_char`, a visible `detail` namespace). The row therefore uses
`FMT_TEST=OFF` and adds an importer through
`-DCMAKE_PROJECT_FMT_INCLUDE=scripts/oss/shims/fmt-module-tests.cmake`:
`fmt-module-smoke` (`scripts/oss/shims/fmt-module-smoke.cc`) does
`import fmt;`, includes no {fmt} header, checks 14 formatted strings
(positional, specs, named args, `join`, ranges, chrono, a user
formatter, `memory_buffer`, `to_string`, `format_int`, `runtime`,
`formatted_size`, `styled`, a thrown `format_error`) and is the one CTest
case.

## Results

| build | TUs ok / failed | links | tests | wall | peak TU RSS |
|---|---|---|---|---|---|
| clang++-19, libstdc++ | 2 / 0 | 1 / 0 | 1/1 | 7 s | 249 MB |
| kiln, libstdc++ | 2 / 0 | 1 / 0 | 1/1 | 7 s | 98 MB |
| clang++-19, libc++ | 2 / 0 | 1 / 0 | 1/1 | 6 s | 240 MB |
| **kiln, libc++** | **2 / 0** | **1 / 0** | **1/1** | 7 s | **120 MB** |

Per TU (CPU time, peak RSS; `oss-report.py --perf`):

| TU | stdlib | clang-19 s | kiln s | time x | clang-19 MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|---|
| `src/fmt.cc` (the module) | libstdc++ | 3.5 | 3.0 | 0.83 | 249 | 98 | 0.39 |
| `src/fmt.cc` (the module) | libc++ | 3.0 | 2.7 | 0.92 | 240 | 100 | 0.42 |
| `fmt-module-smoke.cc` (importer) | libc++ | 1.4 | 2.2 | 1.53 | 225 | 120 | 0.53 |
| `fmt-module-smoke.cc` (importer) | libstdc++ | 1.4 | 2.2 | 1.61 | 203 | 93 | 0.46 |

The importer is the one TU over 1.2x (under both libraries): kiln's import replays the
module's declarations (spec/modules.md §6) where clang loads them lazily
from its PCM.

## The libstdc++ failure (fixed, WP-1156)

`fmt-module-smoke.cc` includes `<chrono>` and then does `import fmt;`.
kiln rejects it with `redefinition of 'numeric_limits<_Float32>'` (and
`_Float64`, `_Float128`) at `/usr/include/c++/14/limits:1989`, reached
from the importer's `<chrono>`. In fmt's GMF, glibc's `math.h` (with
`typedef float _Float32;`, which kiln skips in system headers,
spec/types.md §5.1) comes before libstdc++'s `<limits>`; in that order
the module's specializations are not merged with the importer's (with
`<limits>` first they are). libstdc++ emits these specializations only
when `__FLT32_DIG__` is predefined: kiln predefines it (the P1467 types,
spec/abi.md) and clang does not, which is why the control does not see
them.

Reduced: a GMF of `#include <math.h>` and an importer that includes
`<limits>` before the import,
`tests/conformance/module.global.frag/gmf-floatn-limits.cpp`.

Root cause (WP-1156): not the `_Float32` type, which is kiln's builtin
in both units (the typedef skip is order-independent). glibc's `math.h`
also defines `__builtin_nansf32(x)` as a macro, which changes the token
count of libstdc++'s `__glibcxx_float_n(32)` expansion, so the
position-based deduplication (spec/modules.md §6) could not match the
two units' text. The entity merge it falls back to did not cover a
class template's explicit or partial specializations, nor the
importer's own declarations before the `import` (the module unit kiln
builds in-process sees those as another translation unit). Both merge
now (`gmf-floatn-limits.cpp`, `gmf-floatn-limits-run.cpp`,
`gmf-spec-merge.cpp`). kiln's `__FLT32_DIG__` family matches GCC 16.1's
predefines (which libstdc++ is written for); only the `__FLT16_*`
spellings of the values differ in digits, not in value.
