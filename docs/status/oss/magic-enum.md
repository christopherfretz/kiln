# magic_enum with kiln++ (WP-683)

This page covers magic_enum **v0.9.7** (tag `v0.9.7`). It was built with
kiln++ as `CMAKE_CXX_COMPILER`, through magic_enum's own CMake build and
test suite. The control is a clang++-19 build of the same tree, and the
harness is M8's (`scripts/oss/`, spec/landed/harness/WP-666.md).

magic_enum's tests use the bundled Catch2 v2 single header
(`test/3rdparty/Catch2`). They are compiled `-O3 -Werror` as C++17, C++20
and C++23. magic_enum reads enumerator names from `__PRETTY_FUNCTION__`
in constant evaluation.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.**
- All 22 TUs compile and all 22 test executables link, under both
  compilers.
- All 15 ctest tests pass under both compilers: 190 Catch2 test cases
  and 3642 assertions.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh magic_enum --compiler clang
scripts/oss/oss-build.sh magic_enum
scripts/oss/oss-report.py magic_enum
```

These run the following in `.oss/build/magic_enum/<kiln|clang>/`. The
wrapper is `scripts/oss/kiln-cxx`, which sets `ulimit -v 3000000` and
`timeout 600`.

```sh
cmake --fresh -S .oss/src/magic_enum -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DMAGIC_ENUM_OPT_BUILD_TESTS=ON -DMAGIC_ENUM_OPT_BUILD_EXAMPLES=ON
nice -n 10 make -k
ctest --output-on-failure
```

The project row was already `verified = true` (WP-666).

## Results

How each column was measured:
- **Wall** is the whole `oss-build.sh` run, from `report.json`.
- **Σ TU wall** is the sum of the wrapper's per-TU wall times.
- **Peak TU RSS** is the wrapper's `ru_maxrss`, which includes the
  backend.
- Each build was run once.

| Build | TUs ok / failed | Links | Tests (ctest; Catch2) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `7c5998af2`) | 7 / 15 | 7 / 7 | 0/15 | 7 s | — | 256 MB |
| clang++-19 (control) | 22 / 0 | 22 / 22 | 15/15; 190 cases, 3642 assertions | 41 s | 199.5 s | 520 MB (`test/test.cpp`) |
| **kiln, final** | **22 / 0** | **22 / 22** | **15/15; 190 cases, 3642 assertions** | 17 s | 182.8 s | **359 MB** (`test/test.cpp`) |

Compared with clang, kiln takes 0.92× the summed TU time and 0.69× the
peak RSS. The two builds' wall times were measured at different host
loads. Neither build prints any warnings.

## Clusters

The first build's 15 failures fall into the clusters below, by first
diagnostic. Later rows were exposed by fixing earlier ones.

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `'m_type' is a protected member of 'ResultBase'` (`catch.hpp`: `using ResultBase::m_type;` in a class template with a dependent base) | 15 | fixed by WP-680 (`062f38bf9`), merged | `tests/conformance/class.protected/dependent-base-current-object.cpp`, `using-decl-dependent-base-run.cpp` |
| 2 | `calling a protected constructor of class 'ResultBase'` (`BasicResult::ok()`, inherited constructor) | exposed by 1 | fixed by WP-680 (`e602261d4`), merged | `tests/conformance/namespace.udecl/inherited-protected-ctor-object.cpp`, `tests/conformance/class.protected/inherited-ctor-derived-object-run.cpp` |
| 3 | `ambiguous partial specializations of 'UnaryLambdaTraits<…>'` (`R (C::*)(A) const` vs `(A...)`) | exposed by 2 | fixed by WP-680 (`030cd5639`), merged | `tests/conformance/temp.deduct.type/function-param-pack-vs-single-ordering.cpp` |
| 4 | `cannot pass object of non-trivial type … through variadic function` (`-Wnon-pod-varargs` as an error) at Catch2 `REQUIRE`'s `__builtin_constant_p(expr)` in a template | 6 | fixed, `f2be14a10` (sema: rebuilt custom-checked builtin calls keep their own argument checks) | `tests/conformance/ext.gnu.builtins/constant-p-class-arg-template.cpp` |
| 5 | `static assertion failed due to requirement 'magic_enum::enum_count<BoolTest>() == 2'` (enum with underlying type `bool` printed as `1`, not `BoolTest::Nay`) | 3 | fixed, `994a9959b` (ast: bool-based enumeration values print as enumerators) | `tests/conformance/dcl.fct.def.general/predefined-bool-enum-arg.cpp`, `predefined-bool-enum-note.cpp` |
| 6 | `use of function template name with no prior declaration … is a C++20 extension [-Werror,-Wc++20-extensions]` at a `std::pair` structured binding (C++17) | 1 | fixed, `5257fbaf5` (sema: no C++20-extension warning on a structured binding's ADL get) | `tests/conformance/dcl.struct.bind/tuple-get-adl-cxx17.cpp` |

No miscompiles were found.

## What remains

These are pre-existing gaps found while reducing. magic_enum does not
hit any of them.

- **Missing destructor-access error.** After an inaccessible protected
  constructor, kiln omits clang's second error,
  `variable of type 'B' has protected destructor`. This is the same gap
  as the one in docs/status/oss/spdlog.md.
- **Different `static_assert` note.** For a comparison of values of a
  `bool`-based enumeration, the "expression evaluates to" note differs.
  kiln prints `'true == false'`, and clang prints `'1 == 0'`. In a
  template's requirement, kiln prints `(B)1` where clang prints
  `(B)true`. Repro (the note is at 2:33):

  ```cpp
  enum class B : bool { Nay, Yay };
  static_assert(static_cast<B>(1) == B::Nay, "");
  ```

- **Variants not run:** Debug, `MAGIC_ENUM_OPT_ENABLE_NONASCII`, and
  the C++26 reflection backend.
