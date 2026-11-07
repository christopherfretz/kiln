# Small header-only trio with kiln++ (WP-683)

WP-683 built three header-only projects with kiln++ through their own
CMake builds and test suites, using the M8 harness (`scripts/oss/`). The
clang++-19 control build was the parity target.

The three projects:
- magic_enum v0.9.7
- doctest v2.5.3
- tl_expected v1.3.1

All three reach parity. Each project has its own page with the exact
commands, the cluster table and the repro paths:
- magic-enum.md
- doctest.md
- tl-expected.md

Host: aarch64 Linux, libstdc++ 14. Date: 2026-11-07.

| Project | TUs first → final (clang) | Tests kiln / clang | Wall kiln / clang | Σ TU wall kiln / clang | Peak TU RSS kiln / clang |
|---|---|---|---|---|---|
| magic_enum | 7/22 → 22/22 (22/22) | 15/15 (190 cases, 3642 assertions) / 15/15 | 17 s / 41 s | 182.8 s / 199.5 s | 359 MB / 520 MB |
| doctest | 0/121 → 121/121 (121/121) | 193/193 / 193/193 | 12 s / 21 s | 135.3 s / 53.6 s | 99 MB / 165 MB |
| tl_expected | 12/13 → 13/13 (13/13) | 1/1 (38 cases, 454 assertions) / 1/1 | 9 s / 14 s | 24.5 s / 35.6 s | 209 MB / 337 MB |

"First" is kiln at main `7c5998af2`, the base of WP-683. For doctest,
the 37 TUs that depend on the 84 rejected ones were not attempted.

## Fixes

There are five WP-683 fixes, plus three clusters that WP-680 fixed (it
was merged into this branch).

| Cluster | Project (TUs) | Fix | Test |
|---|---|---|---|
| Catch2 v2 `ResultBase` protected `using` / inherited constructor / `R(A)` vs `R(A...)` ordering | magic_enum (15), tl_expected (1) | WP-680 (`062f38bf9`, `e602261d4`, `030cd5639`) | WP-680's tests, plus two run tests in `tests/conformance/class.protected/` |
| `__builtin_constant_p(classobj)` rebuilt in a template took the variadic path (`-Wnon-pod-varargs`) | magic_enum (6) | `f2be14a10` | `ext.gnu.builtins/constant-p-class-arg-template.cpp` |
| `bool`-based enumeration values printed as `1`, not as enumerators | magic_enum (3) | `994a9959b` | `dcl.fct.def.general/predefined-bool-enum-{arg,note}.cpp` |
| `-Wc++20-extensions` on a C++17 structured binding's implicit `get<i>` | magic_enum (1) | `5257fbaf5` | `dcl.struct.bind/tuple-get-adl-cxx17.cpp` |
| `-Qunused-arguments` unknown | doctest (84) | `1d54d6020` | `kiln_driver` `driver_errors_and_warnings` |
| Unqualified template-id call substituted outside its namespace found nothing | doctest (1) | `9825ed31c` | `temp.dep.candidate/template-id-definition-lookup.cpp` |

The fixes and their spec sentences are listed in
spec/landed/sema/WP-683.md.

## What remains

Each project page lists its own follow-ups. In short:
- **Destructor-access error.** kiln is missing the protected-destructor
  error after an inaccessible protected constructor (see
  docs/status/oss/spdlog.md).
- **`static_assert` note.** For a `bool`-based enumeration, kiln's
  "evaluates to" note prints `true == false` where clang prints
  `1 == 0`.
- **doctest compile time.** kiln takes 2.5× clang's summed TU time on
  doctest's small `-O3` TUs.
