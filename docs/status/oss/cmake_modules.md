# cmake_modules: CMake C++20 modules and `import std` (WP-1148)

`scripts/oss/fixtures/modules` (the `selftest.sh` fixture of WP-982 and
WP-984, a `local` row in `scripts/oss/projects.toml`): a primary
interface, an interface partition, an implementation unit and a consumer
`app`, built through `FILE_SET CXX_MODULES`; under libc++ also `stdapp`,
which does `import std;` through CMake's `CXX_MODULE_STD` (CMake builds
libc++'s `std.cppm` and `std.compat.cppm` with the compiler under test).
kiln++ with kiln-scan-deps against clang++-19 with clang-scan-deps-19.
Host: aarch64 Linux, CMake 3.31.6, Ninja 1.12.1, libstdc++ 14 and
libc++ 19. Date: 2026-11-09.

**Result: parity under both standard libraries.** The test checks
`app` prints `hello 42` and, under libc++, `stdapp` prints
`import std 6`. Under libstdc++ CMake offers no `import std`, so there
is no `stdapp` under either compiler.

## Commands

```sh
cargo build --profile harness -p kiln_driver
for s in libstdc++ libc++; do
  scripts/oss/oss-build.sh cmake_modules --compiler clang --stdlib $s
  scripts/oss/oss-build.sh cmake_modules --stdlib $s
done
scripts/oss/oss-report.py --perf cmake_modules
scripts/oss/oss-report.py --perf --stdlib libc++ cmake_modules
```

The fixture adds `-fmodules-reduced-bmi` only for compilers reporting
version 20 or later (kiln accepts it as a no-op; clang 19 does not know
it).

## Results

| build | TUs ok / failed | links | test | wall | peak TU RSS |
|---|---|---|---|---|---|
| clang++-19, libstdc++ | 4 / 0 | 1 / 0 | ok | 1 s | 93 MB |
| kiln, libstdc++ | 4 / 0 | 1 / 0 | ok | 1 s | 55 MB |
| clang++-19, libc++ | 7 / 0 | 2 / 0 | ok | 4 s | 261 MB |
| kiln, libc++ | 7 / 0 | 2 / 0 | ok | 5 s | 102 MB |

Per TU under libc++ (CPU time, peak RSS; `oss-report.py --perf --stdlib
libc++ cmake_modules`):

| TU | clang-19 s | kiln s | time x | clang-19 MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| libc++ `std.cppm` | 1.7 | 1.4 | 0.82 | 261 | 102 | 0.39 |
| `stdmain.cpp` (`import std;`) | 0.9 | 2.3 | 2.55 | 150 | 96 | 0.64 |
| libc++ `std.compat.cppm` (imports `std`) | 0.3 | 1.4 | 5.17 | 143 | 94 | 0.66 |

Building `std` is faster and leaner than clang's; importing it is
slower, because kiln replays the imported module's declarations
(spec/modules.md §6) where clang loads them lazily. That is the
lazy-loading module-file work (spec/modules.md §11), not this harness.
