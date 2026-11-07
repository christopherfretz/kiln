# abseil-cpp with kiln++ (WP-686)

abseil-cpp LTS **20260107.1** (tag `20260107.1`, commit `255c84d`), built
with kiln++ as `CMAKE_CXX_COMPILER` through abseil's own CMake build with
its tests enabled (`ABSL_BUILD_TESTING=ON`, googletest HEAD downloaded at
configure time and compiled by the compiler under test), and run with
CTest. The control is a clang++-19 build of the same tree. Both use the M8
harness (`scripts/oss/`, spec/landed/harness/WP-666.md). Host: aarch64
Linux, libstdc++ 14, C++17 (the project row), default `-fclang-compat`.
Date: 2026-11-07.

**Result: near parity.** Every TU compiles, every target links, and 231
of the 232 tests pass. The remaining failure, `absl_statusor_test`, is
fixed by WP-683 (`9825ed31c`, a dependent template-id call keeps its
definition's namespace-scope lookup), which has not landed on this
branch. To get here kiln needed 13 fixes: 11 rejected-valid code (one of
them a panic) and 2 miscompiles (one wrong-code mangling, one link
failure).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh abseil --compiler clang
scripts/oss/oss-build.sh abseil
scripts/oss/oss-report.py abseil
```

These run the following in `.oss/build/abseil/<kiln|clang>/`. The wrapper
`scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and `timeout 600`.

```sh
cmake --fresh -S .oss/src/abseil -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DABSL_BUILD_TESTING=ON -DABSL_USE_GOOGLETEST_HEAD=ON -DBUILD_TESTING=ON
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure             # 232 tests
```

The project row in `scripts/oss/projects.toml` worked unchanged and is
now `verified = true`.

## Results

- **TUs** are the build-phase compiles (413, CMake probes excluded).
- **Wall** is the whole `oss-build.sh` run (configure, build and test),
  taken from `report.json`.
- **Σ TU wall** sums the wrapper's per-TU compile wall times; links are
  excluded.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

Each build ran once.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (chain tip `65ef91c38`) | 126 / 4 (283 not attempted) | 51 / 0 | 50/232 | 78 s | — | 163 MB |
| clang++-19 (control) | 413 / 0 | 232 / 0 | 232/232 | 443 s † | 1230.9 s | 1853 MB (`any_invocable_test.cc`) |
| kiln, run 3 (8 fixes) | 397 / 16 | 216 / 0 | 214/232 | 324 s | — | 1665 MB (`any_invocable_test.cc`) |
| **kiln, final** (`1ac5ca05a`) | **413 / 0** | **232 / 0** | **231/232** | 356 s | 2288.3 s | **1665 MB** (`any_invocable_test.cc`) |

In the first kiln build, a rejected TU in `absl_strings` (cluster 1)
stopped `make -k` from building most dependent targets.

† The control's wall includes the clone. The kiln runs reused the source,
and all builds shared the host with other harness builds. Σ TU wall is
the cost comparison: kiln takes **1.86×** clang's summed compile time.
The slowest TU is close under both compilers (`any_invocable_test.cc`:
126 s kiln, 123 s clang), so the overhead is spread across the
mid-sized TUs. kiln's peak RSS is **0.90×** clang's.

## Clusters

The first four clusters are from the first build. The rest appeared only
once those were fixed and more of the tree was built.

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `missing 'template' keyword` (`charconv_bigint.cc`: `copy.DivMod<10>()` on an object of the current instantiation) | 1, which blocked most of the build | fixed, `8bf596420` | `tests/conformance/temp.dep.type/p1-current-instantiation-object-member-template-run.cpp` |
| 2 | `nullability specifier '_Nonnull' cannot be applied to non-pointer type` (`std::unique_ptr`) | 1 | fixed, `edb19d34c` | `tests/conformance/dcl.type.class/nullability-std-smart-pointers.cpp` |
| 3 | `no matching constructor` (`u8"…"` as `const char *` in C++17) | 1 | fixed, `30d954ea3` | `tests/conformance/lex.string/u8-literal-char-before-cxx20-run.cpp` |
| 4 | `&N::m` through a dependent base rejected (`any_invocable_test.cc`) | 1 | fixed, `d1b3ccfe7` | `tests/conformance/expr.unary.op/p3-qualified-member-unrelated-class-dependent-base-run.cpp` |
| 5 | panic: `CXXMemberCallExpr callee is ParenExpr` (`(g.min)()`, random tests) | 7 | fixed, `95eaaf4f9` | `tests/conformance/expr.call/p1-parenthesized-member-callee-run.cpp` |
| 6 | `no viable overloaded '='` (inherited constructors in a conversion; flat/node hash set/map tests) | 4 | fixed, `e36044e44` | `tests/conformance/namespace.udecl/p13-inherited-ctor-implicit-conversion-run.cpp` |
| 7 | `use of undeclared identifier '__builtin_infer_alloc_token'` (`__has_builtin` said yes) | 1 | fixed, `0af1661ef` | `tests/conformance/cpp.cond/has-builtin-undeclared-clang22-builtin.cpp` |
| 8 | `static_assert` failed: `__builtin_is_cpp_trivially_relocatable` with a user-provided move (`type_traits_test.cc`) | 1 | fixed, `993fbb18b` | `tests/conformance/ext.traits/is-cpp-trivially-relocatable-user-move.cpp` |
| 9 | `no type named 'raw_hash_set'` (a base specialization's injected-class-name) | 1 | fixed, `c9fd77fd8` | `tests/conformance/temp.local/p2-base-specialization-injected-name-run.cpp` |
| 10 | `expected class name` (`btree`: a member named like the base class template) | 1 | fixed, `dd0e27067` | `tests/conformance/class.derived/p2-base-class-name-type-only-lookup-run.cpp` |
| 11 | `no matching 'operator new'` (class-scope `operator new` template, `exception_safety_testing`) | 1 | fixed, `9f71f7e7f` | `tests/conformance/expr.new/class-operator-new-template-run.cpp` |
| 12 | link: undefined `ThrowingValue<…>::operator delete(void*)` (class template allocation functions never instantiated) | 1 link | fixed, `1ac5ca05a` | `tests/conformance/temp.inst/class-template-allocation-functions-instantiated-run.cpp` |
| 13 | miscompile: `absl_log_format_test` failures (`f(const T (&)[sizeof(T)])` for `char`/`wchar_t` mangled alike, so one definition replaced the other) | 0 (1 test) | fixed, `3b16fe7bb` | `tests/conformance/temp.over.link/dependent-array-bound-overloads-distinct-run.cpp`, `tests/mangle/dependent_array.cpp` |
| 14 | runtime: `absl_statusor_test` (2 cases) | 0 (1 test) | fixed by WP-683 `9825ed31c`, not merged here | WP-683's test |

No cluster was a bug outside kiln. Every TU that clang accepts, kiln
accepts as well.

## What remains

- `absl_statusor_test` passes once WP-683 lands.
- Compile time is 1.86× clang's Σ TU wall and needs profiling (M8 perf
  work).
- Gaps found while reducing, not on abseil's path:
  - A global `operator new` template is not an allocation candidate
    (`crates/kiln_sema/src/overload/alloc.rs`, `AllocFns` lookup).
  - A placement `operator delete` template is not matched
    (`new_delete.rs`, `matching_operator_delete`).
  - `D::B::m`, through a base's injected-class-name used as a
    nested-name-specifier component, is not handled
    (`crates/kiln_parse/src/parser.rs`).
  - `&N::f` naming an overload set through a dependent base is not handled
    (`crates/kiln_sema/src/class/member_expr.rs`).
- Variants not run: Debug, C++20/26, libc++, `BUILD_SHARED_LIBS=ON`, and
  the `absl` install tree consumed by re2. `docs/status/oss.md` is not
  updated here; `scripts/oss/oss-report.py --update docs/status/oss.md
  abseil` regenerates its row.
