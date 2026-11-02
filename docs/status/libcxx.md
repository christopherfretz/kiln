# libc++ 19 end to end — status (ROADMAP M3b)

The headline metric for M3b: how much of libc++ 19.1.7's own test suite
(`.toolchain/llvm-project/libcxx/test`, 8695 `.cpp`, 7898 under `std/`) kiln
passes when run as the compiler under `cargo xtask libcxx` (contract:
`spec/libcxx.md`). The M3 gate (`docs/status/libcxx-m3.md`) is the frozen
8-directory subset; this page tracks the whole suite.

## Measurements

| Date | kiln main | Scope | pass | fail | xfail | unsupported | pass rate | wall |
|---|---|---|---|---|---|---|---|---|
| 2026-11-02 | 68822097 | same 10% sample (790 tests) after WP-268…276, 279, 280, 282, 287 | 724 | 41 | 3 | 22 (3 driver) | **94.3%** | 24 min, -j 3, debug kiln, 20 s timeout |
| 2026-11-01 | 1b340ab5 | 10% stratified sample, 790 `std/` tests (every 10th in sorted order) | 555 | 210 | 3 | 22 (3 driver) | 72.3% | 21 min, -j 3, debug kiln, 20 s timeout |

Pass rate = pass / total; `unsupported` are tests the driver skips (`.verify.cpp`
needs `-verify`, WP-143) or lit marks unsupported for this configuration.

How to rerun the sample (from the helper checkout; adapt paths):

```
T=kiln/.toolchain/llvm-project/libcxx/test
find $T/std -name '*.cpp' | sort | awk 'NR%10==3' > sample.list
cd kiln && source scripts/env.sh && \
  cargo xtask libcxx --no-fail -j 3 --timeout 20 --report sample.json $(cat ../sample.list) | tee ../sample.log
```

Only failures are printed; cluster them with
`grep "FAIL (" sample.log | sed 's/.* — //' | sed "s/'[^']*'/'X'/g" | sort | uniq -c | sort -rn`.
The log elides symbols as `…`; reproduce a cluster with `kiln++ -fsyntax-only`
and the harness flags (`spec/libcxx.md`).

## Failure clusters, 2026-11-02 sample (41 failures)

| Count | Diagnostic | Where | WP |
|---|---|---|---|
| 7 | `no matching function for call to '…'` in `std::format`/`print` | format.functions, format.range, container.adaptors.format, print.fun, time.zone.exception | WP-286 |
| 6 | `static assertion expression is not an integral constant expression` / failed | `<expected>` ctor/observers/dtor | PR #53 (WP-289) |
| 5 | `no matching constructor for initialization of '…'` | allocator-extended ctors: tuple alloc_copy/alloc_const_Types/member_swap_const, unord.map from_range, unord.multiset init_size_allocator | WP-295 |
| 3 | `assertion failed` at run time | filesystem status, is_block_file, directory_entry replace_filename | WP-297 |
| 3 | `.verify`: `warning expected but not seen: '…' is deprecated` | codecvt char16_t, unary_negate, storage.iterator | WP-297 |
| 3 | `not yet implemented: substitution into CXXParenListInitExpr` / `CXXStdInitializerListExpr` | time.zone zonedtime ×2, charconv.msvc | WP-296 |
| 3 | `not yet implemented: pack expansion in this context`; mdspan extents.verify | mdspan layout_stride, inout_ptr | WP-292 |
| 1 | coroutine expressions not supported | support.coroutines | M4 |
| 10 | singletons: redefinition with a different type (forwardlist), not marked const (uninitialized.construct), reinterpret_cast (formatter.pointer), cannot delete (shared_ptr pointer_deleter), [over.over] (variant visit.member, strstream), type specifier required (lognormal param_types), relops_bool_conv verify, optional U.pass destructor | — | WP-295/296/297, WP-291 |

## Failure clusters, 2026-11-01 sample (210 failures)

| Count | Diagnostic | Root cause | WP |
|---|---|---|---|
| 53 | `'lgamma_r' is missing exception specification 'noexcept(true)'` | redeclaration without exception spec of a glibc `__THROW` function (libc++ `__random/binomial_distribution.h:101`); clang: extension warning + inherit | WP-268 |
| 42 | `no member named '__current_' in 'move_iterator<…>'` | noexcept-specifier of a friend is a complete-class context, kiln evaluates it eagerly (`__iterator/move_iterator.h:233`) | WP-269 |
| 12 | `not yet implemented: reference to a member of a dependent context` | hidden friends of nested class templates under constraint satisfaction (join/filter/elements views) | WP-272 |
| 11 | `use of undeclared identifier '__builtin_mul_overflow'` | mdspan layouts, gcd/lcm | WP-270 |
| 11 | `field has incomplete type 'unsigned char[]'` | flexible array member in glibc `bits/socket.h` (every `<filesystem>` test) | WP-271 |
| 6 | `no type named 'type' in 'enable_if<false, int>'` | SFINAE through `__enable_if_t` alias reported as hard error | WP-274 |
| 4 | `friend declaration of 'get_underlying' does not match any declaration` | friend function templates in class templates (`experimental/propagate_const`) | WP-273 |
| 6 | `.verify.cpp` expectation mismatches | diag wording | WP-143/WP-253 |
| 42 | static assertions, no matching function, link failures, access/abstractness false positives, rewritten comparisons, … | long tail, reduced per test with cause, reproducer and status in [libcxx-longtail.md](libcxx-longtail.md) | WP-276, WP-289 |

Fixing the five largest clusters (129 of 210) should move the sample to about
90%; several tests will then surface a second failure, so the next sample run
decides the following batch.

## Per-directory sizes (full checkout, `std/`)

containers 1493, utilities 1366, numerics 889, input.output 636, strings 429,
ranges 423, time 417, thread 340, localization 332, iterators 309, algorithms
307, language.support 281, re 188, depr 154, atomics 118, experimental 101,
diagnostics 72, concepts 36.
