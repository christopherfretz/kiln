# Boost metaprogramming libraries with kiln++ (WP-787)

Boost **1.90.0** (tag `boost-1.90.0`, superproject with submodules) is built
through its CMake superproject with `BUILD_TESTING=ON` for 20 header-only
metaprogramming libraries: mp11, hana, fusion, mpl, pfr, describe,
variant2, variant, spirit, yap, type_traits, tuple, optional, outcome,
static_string, callable_traits, type_index, function_types, lambda2 and
hof. The libraries they depend on (preprocessor, core, config, thread and
others) are configured too, and the TUs of their test dependencies are
counted. Tests run under CTest. The control is a clang++-19 build of the
same tree. Both use the M8 harness (`scripts/oss/`,
spec/landed/harness/WP-666.md and WP-787.md). Host: aarch64 Linux,
libstdc++ 14, C++20 (the project row), default `-fclang-compat`.
Date: 2026-11-07.

**Result:** in the final build (`50a2647c8`) kiln++ compiles
2211 of the 2253 TUs (the clang-19 control compiles 2240). It links all
1986 executables it builds, and 2059 of the 2087 tests that pass under the
control also pass under kiln. The first build managed 1118 TUs, 919 links
and 989 tests. 13 of the 42 failed TUs also fail under the control. The
other 29 fall in 10 follow-up clusters and one divergence that matches
clang 22, and one test fails at run time (fusion `tuple_traits`). This
WP fixed 12 clusters. Summed over the 2211 TUs both compilers build, kiln
takes 1.36x clang's wall time, which is a perf finding, and about half its
peak RSS (0.51x).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh boost_meta --compiler clang
scripts/oss/oss-build.sh boost_meta
scripts/oss/oss-report.py boost_meta
```

These run the following in `.oss/build/boost_meta/<kiln|clang>/`. The
wrapper `scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and
`timeout 600`.

```sh
cmake --fresh -S .oss/src/boost -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON \
  "-DBOOST_INCLUDE_LIBRARIES=mp11;hana;fusion;mpl;pfr;describe;variant2;variant;spirit;yap;type_traits;tuple;optional;outcome;static_string;callable_traits;type_index;function_types;lambda2;hof" \
  "-DKILN_BOOST_JAMFILE_LIBRARIES=hana;fusion;variant;spirit;yap;type_traits;callable_traits;hof" \
  -DCMAKE_PROJECT_Boost_INCLUDE=scripts/oss/shims/boost-jamfile-tests.cmake
nice -n 10 make -k tests              # draws from the shared jobserver
ctest -j 4 --output-on-failure --timeout 300 -E '<12 tests, below>'
```

The shim `scripts/oss/shims/boost-jamfile-tests.cmake` registers the b2
tests of the 8 libraries that have none in a superproject CMake build
(spec/landed/harness/WP-787.md). That gives 2099 ctest tests.

**Excluded tests.** The row's ctest `-E` leaves out 12 tests. Each of them
also fails to build under the clang-19 control, so it is not a kiln
result:

- yap `user_expression_transform_1`, `user_expression_transform_2`,
  `call_expr`, `reference_returns`, `if_else`: "call to constructor of
  `term<T>` is ambiguous" (yap 1.90 against clang 19).
- variant `overload_selection`: `variant.hpp:1615` "no matching member".
- spirit `karma_format_manip_attr`, `karma_generate_attr`,
  `qi_match_manip_attr`, `qi_parse_attr`: `BOOST_PP_ITERATE` includes
  `test_manip_attr.hpp`, which b2 finds through a project `<include>`
  requirement that the shim does not read.
- spirit `lex_regression_matlib_static`, `lex_regression_matlib_switch`:
  `matlib_static.h` is written when another test runs.

In the control, all 2087 other tests pass.

**System Boost is not used.** The host has Boost 1.83 in
`/usr/include/boost`. A `-M` of `libs/outcome/test/tests/comparison.cpp`
lists 510 in-tree Boost headers under clang++-19, 505 under kiln++, and 0
under `/usr/include/boost` for either.

## Results

- **TUs** are the build-phase compiles. CMake probes are excluded.
- **Wall** is the whole `oss-build.sh` run (configure, build and test),
  taken from `report.json`.
- **Σ TU wall** sums the wrapper's per-TU compile wall times. Links are
  excluded.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

Each build ran once, from an empty build directory, on a host shared with
other harness builds.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| clang++-19 (control) | 2240 / 13 | 2013 / 0 | 2087/2099 (the 12 excluded) | 3153 s † | 8404 s | 1087 MB (`preprocessing_mode.cpp`) |
| kiln, first (`4718aa54e`) | 1118 / 918 (1 panic; 217 not attempted) | 919 / 0 | 989/2099 | 3681 s | 14985 s | 1177 MB (`preprocessing_mode.cpp`) |
| kiln, final (`50a2647c8`) | 2211 / 42 (13 also fail in the control) | 1986 / 0 | 2059/2099 (27 not built, 1 failed; the 12 excluded) | 6509 s ‡ | 11444 s | 1176 MB (`preprocessing_mode.cpp`) |

† The control's wall includes the clone. The kiln runs reused the source.

The clang control's 13 failed TUs are the sources of the 12 excluded
tests. In the first kiln build, all 217 TUs never attempted are
spirit's: `make -k` skips a target once one of its TUs fails.

‡ The final build ran while the host was also running a full LLVM build
(load average 9 to 13), so its wall time is not comparable with the other
two.

### Performance

Over the 2211 TUs that both compilers build:

| | kiln | clang++-19 | kiln / clang |
|---|---|---|---|
| Σ TU wall | 11164 s | 8195 s | **1.36x** |
| Σ TU peak RSS | | | 0.51x |
| Peak TU RSS | 1176 MB | 1087 MB | 1.08x (`preprocessing_mode.cpp`) |
| Max TU wall | 32.2 s | 46.8 s | 0.69x |

**Perf finding (recorded, not fixed):** the TU wall ratio is above 1.2x.
By library:

- yap: 1.75x.
- variant: 1.68x.
- fusion: 1.57x.
- hana: 1.40x.
- spirit: 1.39x.
- mp11, mpl, variant2 and outcome: 0.85x to 1.08x.

The worst single TUs are spirit `qi/symbols1.cpp`, `qi/symbols2.cpp` and
`lex/regression_matlib_generate.cpp`, at 2.4x clang (28 to 30 s against
12 s). The ratio may be inflated by host load: the clang control ran
under different load. The finding should be re-measured on a quiet host
before it is acted on. Peak memory is at or below clang everywhere except
`preprocessing_mode.cpp`, whose peak is the preprocessor (1.08x).

### Heaviest TUs

Final kiln build against the control. RSS is in MB; wall is in seconds.

| By kiln peak RSS | kiln RSS | clang RSS | kiln wall | clang wall |
|---|---|---|---|---|
| `function_types/test/reconfiguration/preprocessing_mode.cpp` | 1176 | 1087 | 6.8 | 4.7 |
| `hana/test/tuple/laws.functor.cpp` | 555 | 938 | 32.2 | 27.3 |
| `function_types/test/reconfiguration/partial_arity_preprocessing.cpp` | 537 | 572 | 3.5 | 2.5 |
| `hana/test/ext/std/tuple/laws.functor.cpp` | 383 | 763 | 16.0 | 19.8 |
| `hana/test/detail/canonical_constant/laws.cpp` | 355 | 746 | 13.3 | 13.0 |

| By kiln wall | kiln RSS | clang RSS | kiln wall | clang wall |
|---|---|---|---|---|
| `hana/test/tuple/laws.functor.cpp` | 555 | 938 | 32.2 | 27.3 |
| `spirit/test/qi/sequence.cpp` | 319 | 567 | 30.1 | 16.6 |
| `spirit/test/lex/regression_matlib_generate.cpp` | 271 | 443 | 30.0 | 12.3 |
| `spirit/test/qi/symbols1.cpp` | 259 | 416 | 28.4 | 11.9 |
| `spirit/test/qi/symbols2.cpp` | 257 | 410 | 28.0 | 11.7 |

The control's slowest TU, `static_string/test/static_string.cpp`
(46.8 s), takes kiln 8.0 s.

## Clusters

Clusters are taken from the first build. TUs are counted where kiln
rejects and clang accepts (912 TUs). The runtime clusters appeared in
the ctest results once the build got further.

| # | Signature (first diagnostic) | Libraries | TUs | Status | Repro |
|---|---|---|---|---|---|
| 1 | `ambiguous partial specializations` (non-deduced contexts ignored in partial ordering) | hana, yap | 347 | fixed, `3c11f1995` | `tests/conformance/temp.spec.partial.match/order-non-deduced-context-must-match.cpp` |
| 2 | `template argument for template type parameter must be a type` (fusion `vector.hpp:233`: injected-class-name of a variadic partial specialization) | fusion, hana, variant | 243 | fixed, `95474d204` | `tests/conformance/temp.local/partial-pack-injected-name-in-alias-template.cpp` |
| 3 | `function with deduced return type cannot be used before it is defined` (member template in a class template) | hana | 133 | fixed, `c9073f3a2` | `tests/conformance/dcl.spec.auto/deduced-member-template-used-in-class-template.cpp` |
| 4 | `chosen constructor is explicit in copy-initialization` (braced NSDMI, hana `laws/base.hpp:248`) | hana | 130 | fixed, `c8b0b9098` | `tests/conformance/class.mem.general/default-member-initializer-direct-list-dependent.cpp` |
| 5 | `no viable conversion` (Boost.Exception `exception_ptr.hpp:185`, type-dependent functional cast) | outcome, thread | 35 | fixed, `c9b85fc64` | `tests/conformance/temp.dep.expr/direct-init-from-dependent-functional-cast-explicit-ctor.cpp` |
| 6 | panic: `class properties are already set` (recursive request while a partial specialization is selected) | static_string | 1 | fixed, `c43fd01a3` | `tests/conformance/temp.inst/partial-match-recursive-request-incomplete.cpp` |
| 7 | `excess elements in scalar initializer` (`T x(a...)` with `a` empty) | variant2 | 2 | fixed, `c341de3cb` | `tests/conformance/temp.variadic/empty-expansion-paren-init-value-initializes.cpp` |
| 8 | runtime: `is_virtual_base_of` false for a non-public virtual base (C-style cast checked base access) | type_traits, describe | 0 (3 tests) | fixed, `70a876edf` | `tests/conformance/expr.cast/c-style-cast-inaccessible-base.cpp` |
| 9 | runtime: `has_plus_assign<bool, int*>` false (`bool += pointer` rejected) | type_traits | 0 (1 test) | fixed, `72d41ec3d` | `tests/conformance/expr.ass/bool-add-assign-pointer-run.cpp` |
| 10 | `constexpr variable must be initialized by a constant expression` (`BOOST_HOF_CONST_FOLD`: `reinterpret_cast`) | hof | 6 | follow-up, xfail | `hof/detail/constexpr_deduce.hpp:51`; `tests/conformance/expr.const/builtin-constant-p-conditional-folds-reinterpret-cast.cpp` |
| 11 | `pack expansion contains parameter pack that has a different length` | hof | 5 | follow-up, xfail (same cause as row 22) | `hof/pack.hpp:252`; `tests/conformance/temp.variadic/expansion-empty-first-pack-checks-other-packs-run.cpp` |
| 12 | `static_assert` failed: `noexcept fold` / `noexcept reverse_fold` | hof | 2 | follow-up, xfail | hof `test/fold.cpp`, `test/reverse_fold.cpp`; `tests/conformance/except.spec/nested-member-template-noexcept-specifier-run.cpp` |
| 13 | `invalid use of 'this' outside of a non-static member function` | hof | 1 | fixed by main (passes after merging `origin/main` at `5644b9de6`) | `hof/implicit.hpp:120` |
| 14 | `in-class initializer for static data member is not a constant expression` (enumeration out of range) | type_traits, mpl, spirit | 3 (5 final) | matches clang 22, not a bug | type_traits `make_signed_test`, `make_unsigned_test`; mpl `issue_69` |
| 15 | `address of overloaded function does not match required type` | function_types | 1 | follow-up, xfail | `libs/function_types/example/interface_example.cpp:16`; `tests/conformance/over.over/template-id-overload-set-arity-mismatch-run.cpp` |
| 16 | `expression UnresolvedLookupExpr is not supported yet` | mp11 | 1 | follow-up, xfail | `libs/mp11/test/mp_valid.cpp:49`; `tests/conformance/temp.arg.template/member-alias-template-through-typedef-call-run.cpp` |
| 17 | `not yet implemented: pack expansion in this context` (pack into a non-pack alias template parameter) | mp11 | 1 | follow-up, xfail | `libs/mp11/test/mp_defer.cpp:38`; `tests/conformance/temp.arg.template/member-alias-template-through-typedef-pack-run.cpp` |
| 18 | `expected expression` (parser lookahead: `H<int>::type` inherited from a base, in a function type) | mpl, fusion | 1 (4 final) | follow-up, xfail | `libs/mpl/test/map.cpp:51`, `libs/fusion/test/sequence/filter_view.cpp:117`; `tests/conformance/dcl.ambig.res/function-type-param-inherited-member-type-run.cpp` |

Cluster 14 is a divergence from the clang-19 control, not a kiln bug. The
tests give an enumeration with a fixed range a value outside it, in a
static data member's constant initializer. clang 22 rejects that too
([expr.const]/10, [dcl.enum]/8), and kiln follows clang 22.

Rows 19 to 23 come from an interim build (`b031819cc`), which got
further than the first build. In the first build, clusters 1 to 4 hid
row 20, and spirit's TUs were never attempted (row 21). Row 19 was a
regression from main.

| # | Signature (first diagnostic) | Libraries | TUs (interim) | Status | Repro |
|---|---|---|---|---|---|
| 19 | `call to 'group_helper' is ambiguous`, `call to 'mp_for_each_impl' is ambiguous` (an expansion that matched nothing left its pack undeduced in partial ordering) | hana, mp11, describe | 22 | fixed, `757997b83` | `tests/conformance/temp.func.order/p12-empty-expansion-deduces-empty-pack-run.cpp` |
| 20 | `incompatible operand types ('integral_constant<[...], true>' and 'integral_constant<[...], false>')` (no [expr.cond]/6 built-in candidates; hana `eval_if.hpp:59`, `support/numeric.hpp:75`) | hana | 79 | fixed, `0bfbb7d0e` | `tests/conformance/expr.cond/p6-class-arms-builtin-overload-run.cpp`, `p6-class-arms-builtin-overload-errors.cpp` |
| 21 | `no type named 'type' in 'boost::proto::_state'` (derived-class deduction through `F<T>**` instantiated `mpl::and_<_state, ...>`; proto `template_arity`) | spirit | 95 | fixed, `50a2647c8` | `tests/conformance/temp.deduct.call/pointer-to-pointer-no-derived-class-deduction.cpp` |
| 22 | `static_assert` failed in `integer_sequence` `equal`, and at run time `is_constructible<fusion::tuple<>, int>` is true (an empty first pack hides the expansion's other packs) | hana, fusion | 2 (+1 test) | follow-up, xfail | `crates/kiln_sema/src/templ/pack.rs:286`; `tests/conformance/temp.variadic/expansion-empty-first-pack-checks-other-packs-run.cpp` |
| 23 | `source file is not valid UTF-8` (byte `0xA8` in a comment; clang accepts it) | spirit | 1 | follow-up, xfail | `crates/kiln_pp/src/preprocessor.rs:541`; `tests/conformance/lex.phases/invalid-utf8-in-comment-accepted-run.cpp` |

Row 18 now also covers fusion `filter_view.cpp:117`,
`joint_view.cpp:162` and `iterator_range.cpp:98` (4 TUs in all). Row 14
now also covers 2 spirit TUs (5 in all).

## What remains

In the final build, 29 TUs fail that the control compiles, and one test
fails at run time:

- **hof, 14 TUs** (rows 10 to 13):
  - `reinterpret_cast` in `BOOST_HOF_CONST_FOLD` constant folding:
    `crates/kiln_oven/src/compile.rs:3863`.
  - The pack-length mismatch at `hof/pack.hpp:252` has row 22's cause
    (`crates/kiln_sema/src/templ/pack.rs:286`).
  - The `noexcept` of a fold: a member template's exception
    specification, first instantiated inside another function's
    noexcept-specifier, comes out potentially-throwing.
  - `this` in `hof/implicit.hpp:120` compiles after merging `origin/main`
    (`5644b9de6`), so 13 hof TUs remain.
- **Parser lookahead, 4 TUs** (row 18): `X<A>::type`, where `type` is
  inherited from a base of the specialization, is not seen as a type when
  it is the parameter type of a function type in a cast or declarator.
  - `crates/kiln_sema/src/decl.rs:9015` `classify_template_member`
    returns `Dependent` without instantiating, and
    `crates/kiln_parse/src/tentative.rs:396` `after_dependent_name` then
    rejects the type-id.
  - The fix needs lookahead to annotate (instantiate) a template-id scope,
    as clang's `TryAnnotateCXXScopeToken` does. That is a design-level
    change.
  - Repro: `template<class T> struct B { typedef int type; }; template<class S> struct H : B<S> {}; void (*p)(H<int>::type) = 0;`.
- **Pack expansion checks only the first pack, 2 TUs and 1 test** (row 22):
  - `crates/kiln_sema/src/templ/pack.rs:286` `expand_pattern_impl` takes
    the expansion's length from the first pack the probe reaches. When
    that pack is empty the pattern is never substituted, so a pack of
    another length, or one of a retained level, goes unnoticed.
  - clang checks all of the pattern's packs first
    (`TryExpandParameterPacks`).
  - Repro: `template<class... T> struct tup { template<class... U, class = std::enable_if_t<all<std::is_convertible_v<U, T>...>::value>> explicit tup(U&&...); };` makes `is_constructible_v<tup<>, int>` true.
- **Divergence from the control, 5 TUs** (row 14): kiln matches clang 22.
- **Invalid UTF-8 in a comment, 1 TU** (row 23):
  `crates/kiln_pp/src/preprocessor.rs:541` reports every replaced
  sequence in the file. clang reports one only where a token is lexed,
  never in a comment.
- **Singles:**
  - function_types `interface_example.cpp:16` (row 15).
  - mp11 `mp_valid.cpp:49`, `UnresolvedLookupExpr` in codegen:
    `crates/kiln_codegen/src/expr.rs:517`.
  - mp11 `mp_defer.cpp:38`: `crates/kiln_sema/src/templ/pack.rs:136`.
- **Perf:** the TU wall ratio of 1.36x (Performance, above).

Each follow-up has a hand-reduced `xfail(kiln): WP-787 follow-up` test,
checked to pass under clang++-19:

- `tests/conformance/temp.variadic/expansion-empty-first-pack-checks-other-packs-run.cpp` (rows 11, 22)
- `tests/conformance/expr.const/builtin-constant-p-conditional-folds-reinterpret-cast.cpp` (row 10)
- `tests/conformance/except.spec/nested-member-template-noexcept-specifier-run.cpp` (row 12)
- `tests/conformance/over.over/template-id-overload-set-arity-mismatch-run.cpp` (row 15)
- `tests/conformance/temp.arg.template/member-alias-template-through-typedef-call-run.cpp` (row 16)
- `tests/conformance/temp.arg.template/member-alias-template-through-typedef-pack-run.cpp` (row 17)
- `tests/conformance/dcl.ambig.res/function-type-param-inherited-member-type-run.cpp` (row 18)
- `tests/conformance/lex.phases/invalid-utf8-in-comment-accepted-run.cpp` (row 23)

## Disk

- `.oss/`: 1.7 GB. That is 879 MB of Boost source with submodules, plus
  774 MB for the final kiln build directory. The clang build directory was
  deleted after its numbers were recorded.
- `$CARGO_TARGET_DIR` (`.build/target-wp-787`): 16 GB.
- The volume had 203 GB free at the end.
