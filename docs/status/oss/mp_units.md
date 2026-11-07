# mp-units with kiln++ (WP-792)

This page covers mp-units **v2.5.0** (tag `v2.5.0`). It was built with
kiln++ as `CMAKE_CXX_COMPILER` through mp-units' own CMake build and test
suite. The harness is M8's (`scripts/oss/`, spec/landed/harness/WP-666.md,
WP-792).

The control is **g++-16** (GCC 16.1), not clang. clang 19 cannot build
mp-units: the project documents an unfixable clang 19 bug (llvm-project
PR 118288, "type constraint differs in template redeclaration"), and the
first clang 19 build compiled 7 of 32 TUs. clang 22 also rejects most of
v2.5.0 ("no viable deduction guide" for `quantity` in
`quantity_spec.h`). `control_cxx = "g++-16"` in `projects.toml` selects
GCC. The control build directory is still `.oss/build/mp_units/clang/`.

mp-units is concept- and NTTP-heavy: units, dimensions and quantity
specs are class-type NTTPs, and unit magnitudes are computed in constant
evaluation. `test/static` is `static_assert`-only; `test/runtime` is
Catch2 (the `catch2` row, built with clang 19). Contracts are off (no
gsl-lite) and C++ modules are off.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity with GCC 16.** All 64 TUs compile, all 23 executables
link and all 25 ctest tests pass under both compilers.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh mp_units --compiler clang   # control: g++-16 on PATH
scripts/oss/oss-build.sh mp_units
scripts/oss/oss-report.py --perf mp_units
```

```sh
cmake --fresh -S .oss/src/mp_units -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DMP_UNITS_API_CONTRACTS=NONE -DMP_UNITS_BUILD_CXX_MODULES=OFF
nice -n 10 make -k
ctest --output-on-failure -j 4
```

## Results

Time is the wrapper's CPU time (user plus system) per TU. RSS is the
TU's `ru_maxrss`, which includes the backend. Each build was run once.

| Build | TUs ok / failed | Links | Tests | Σ TU CPU | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (WP-792 base) | 1 / 31 (`make -k` stopped at the library) | 0 | 0/1 | — | 77 MB |
| g++-16 (control) | 64 / 0 | 23 / 23 | 25/25 | 267 s | 1539 MB (`test/static/isq_test.cpp`) |
| **kiln, final** | **64 / 0** | **23 / 23** | **25/25** | **235 s** | **573 MB** (`test/static/isq_test.cpp`) |

Over the 64 TUs, kiln/gcc has a geomean of **0.92x time** and **0.39x
RSS**. The five heaviest TUs:

| TU | gcc s | kiln s | time x | gcc MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `test/runtime/fmt_test.cpp` | 21.3 | 14.6 | 0.69 | 899 | 277 | 0.31 |
| `example/spectroscopy_units.cpp` | 15.1 | 10.4 | 0.69 | 910 | 306 | 0.34 |
| `example/foot_pound_second.cpp` | 11.2 | 8.3 | 0.74 | 701 | 238 | 0.34 |
| `test/static/isq_test.cpp` | 11.0 | 12.0 | 1.10 | 1539 | 573 | 0.37 |
| `example/si_constants.cpp` | 8.6 | 5.9 | 0.68 | 518 | 170 | 0.33 |

Three TUs are over 1.2x on time, among the 61 that take gcc at least
0.5 s. Each is about 1.45x when re-timed alone:
`test/static/angular_test.cpp` (2.8 s vs 2.2 s), `isq_angle_test.cpp`
(3.8 s vs 3.1 s) and `quantity_spec_test.cpp` (2.3 s vs 1.9 s). They are
the perf cluster below.

## Clusters

Correctness. The first build's failures, by first diagnostic. Later rows
were exposed by fixing earlier ones.

| # | Signature | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | rejected at `quantity.h:205`: a qualified class template name (`::mp_units::quantity{t, 1}`) in its own definition was taken as the injected class name, not a CTAD placeholder | 31 | fixed, `83da0bd16` (parse) | `tests/conformance/temp.local/qualified-name-in-own-definition-ctad.cpp` |
| 2 | partial specialization matching ignores the constraint of a `C<A…> auto V` NTTP ([temp.param]/5) | exposed by 1 | fixed, `ebbe34022` (sema) | `temp.param/nttp-constrained-auto.cpp` |
| 3 | `requires(M != mag<1>)` repeated on a redeclaration of `scaled_unit` taken as a different requires-clause | exposed by 1 | fixed, `107ab118c` (ast) | `temp.over.link/requires-clause-specialization-reference.cpp` |
| 4 | **`static_assert` that GCC and clang accept fails**: `template<int N, C<N> auto V>` left `V` unevaluated, so `quantity_point<R, default_point_origin(R)>` named two specializations | exposed by 2 | fixed, `472048d98` (sema) | `temp.param/nttp-constrained-auto-dependent-constraint.cpp` |
| 5 | `-Werror,-Wreturn-type` in `are_ingredients_convertible`: an `if constexpr` with a condition variable left a path off the end | exposed by 1 | fixed, `a03cdb1aa` (sema) | `stmt.if/constexpr-condition-variable-falloff.cpp` |
| 6 | `function with deduced return type cannot be used before it is defined` (`pow<-1>(magnitude<>)`, a friend template whose body names its own class's specialization) | exposed by 1 | fixed, `6e42f8236` (sema) | `temp.friend/friend-template-body-names-own-specialization.cpp` |

Row 4 is a wrong constant-evaluation result, which the brief ranks top
priority. No miscompile shows in the 25 tests.

Performance (kept apart from correctness):

| # | Signature | TUs | Status |
|---|---|---|---|
| P1 | 1.22x to 1.27x gcc's CPU time in the build, about 1.45x re-timed alone, on `angular_test`, `isq_angle_test` and `quantity_spec_test` | 3 | filed. `-fkiln-instantiation-report` shows the cost spread over instantiation and overload resolution, not one hot spot: `are_ingredients_convertible` is 1.69 s inclusive in `angular_test`. No fix of 150 lines or less. |

## What remains

- P1 (instantiation and overload-resolution cost on the unit-conversion
  checks).
- Diagnostic wording differences seen while reducing (not failures):
  printing of a `C auto&` constraint, of pack notes, of `<1, 2L>`
  arguments and of `decltype(…)` types.
