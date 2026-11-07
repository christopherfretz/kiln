# doctest with kiln++ (WP-683)

This page covers doctest **v2.5.3** (tag `v2.5.3`). It was built with
kiln++ as `CMAKE_CXX_COMPILER`, through doctest's own CMake build and
test suite. The control is a clang++-19 build of the same tree, and the
harness is M8's (`scripts/oss/`).

doctest's test CMake compiles every TU with:
- `-std=gnu++11 -O3`
- `-Weverything -pedantic -pedantic-errors`
- `-Qunused-arguments`

That flag set applies to any clang-identified compiler, which includes
kiln++. Because of `-Weverything`, any kiln-only warning counts as a
reject.

The tests (`ctest`) compare each example's output with a checked-in
reference.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.**
- All 121 TUs compile and all 43 executables link.
- All 193 ctest tests pass under both compilers.
- Neither compiler prints a warning.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh doctest --compiler clang
scripts/oss/oss-build.sh doctest
scripts/oss/oss-report.py doctest
```

These run the following in `.oss/build/doctest/<kiln|clang>/`:

```sh
cmake --fresh -S .oss/src/doctest -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DDOCTEST_WITH_TESTS=ON -DDOCTEST_WITH_MAIN_IN_STATIC_LIB=ON
nice -n 10 make -k
ctest --output-on-failure --timeout 300 -E test_debugger
```

The project row needed one change, which is now `verified = true`: the
test command excludes `test_debugger`. On aarch64, that test's SIGTRAP
handler returns to the `brk` of `__builtin_debugtrap()` and loops
forever. The clang-19 build loops the same way, so this is not a kiln
bug. The other 193 of doctest's 194 tests run.

## Results

The columns are measured the same way as in magic-enum.md.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `7c5998af2`) | 0 / 84 (37 not attempted) | 0 / 0 | 0/194 (not built) | 11 s | — | 9 MB |
| clang++-19 (control) | 121 / 0 | 43 / 43 | 193/193 | 21 s | 53.6 s | 165 MB (`test_color.none.cpp`) |
| **kiln, final** | **121 / 0** | **43 / 43** | **193/193** | 12 s | 135.3 s | **99 MB** (`test_color.ansi.cpp`) |

Compared with clang, kiln uses 0.60× the peak RSS but takes 2.5× the
summed TU time. The slowest TU takes 3.7 s with kiln and 1.9 s with
clang.

doctest's TUs are small `-O3` C++11 files, so per-TU fixed cost
dominates: kiln's frontend, then `opt` and `llc` as separate processes.
Fixed cost is a perf topic and was not investigated here.

The two builds' wall times were measured at different host loads.

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `kiln++: error: unknown argument: '-Qunused-arguments'` | 84 (the other 37 depend on them) | fixed, `1d54d6020` (driver: accept `-Qunused-arguments`, which silences the unused-argument warnings as in clang) | `crates/kiln_driver/tests/codegen.rs` `driver_errors_and_warnings` |
| 2 | `no matching conversion for functional-style cast from 'std::string' to 'String'` (`tests/doctest/test_string.cpp:153`: doctest's `is_std_string` partial specialization did not match) | 1 (exposed by 1) | fixed, `9825ed31c` (sema: a dependent template-id call keeps its definition's namespace-scope lookup) | `tests/conformance/temp.dep.candidate/template-id-definition-lookup.cpp` |
| — | `test_debugger` loops forever on aarch64 | 0 | not a kiln bug (clang-19 too); excluded in `projects.toml` | — |

Cluster 2 in detail ([temp.dep.candidate]/1):
- `declval<const T &>().c_str()` appears in the arguments of a partial
  specialization in namespace `doctest::detail`.
- Those arguments are substituted where the specialization is named,
  which is outside that namespace.
- kiln looked up an unqualified template-id call again from that scope,
  and found nothing.
- It now keeps the declarations found at the definition, as it already
  did for calls without explicit template arguments.

No miscompiles were found.

## What remains

- **TU compile time.** kiln takes 2.5× clang's summed TU time on these
  small `-O3` TUs. This was not investigated; it is a candidate for the
  perf campaign.
- **Variants not run:** Debug, `DOCTEST_CONFIG_USE_STD_HEADERS`, C++17
  and later (doctest pins `gnu++11` for its tests), and
  `-DDOCTEST_INTERNAL_WERROR=ON`. The last should behave the same,
  since there are no warnings.
