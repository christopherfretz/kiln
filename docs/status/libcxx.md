# libc++ 19 end to end — status (ROADMAP M3b)

The headline metric for M3b: how much of libc++ 19.1.7's own test suite
(`.toolchain/llvm-project/libcxx/test`, 8695 `.cpp`, 7898 under `std/`) kiln
passes when run as the compiler under `cargo xtask libcxx` (contract:
`spec/libcxx.md`). The M3 gate (`docs/status/libcxx-m3.md`) is the frozen
8-directory subset; this page tracks the whole suite.

## Measurements

Rows from WP-319 on name the kiln++ build profile in the wall column
(`harness` by default, `spec/libcxx.md` "Build profiles"); earlier rows say
"debug kiln".

| Date | kiln main | Scope | pass | fail | xfail | unsupported | pass rate | wall |
|---|---|---|---|---|---|---|---|---|
| 2026-11-02 | 4d071f81 | stratum B: same disjoint 10% sample (790 tests) after WP-297, 300, 301, 304, 306 (#61–#65) | 739 | 13 | 3 | 34 (4 driver) | **97.8%** | 26 min, -j 3, debug kiln, 20 s timeout, 1 timeout counted as fail (simd_copy, WP-313), box loaded (7 implementers). All 14 non-passes are covered: 6 by WP-310 (landed in 7a8b29bf, after this sha), 3 by WP-309 (PR #69), 1 each by WP-307, WP-303, WP-313, and 2 coroutine tests (M4) |
| 2026-11-02 | 94249795 | stratum A: same 10% sample (790 tests) after WP-297, 300, 301, 304 (#61–#63, #65) | 760 | 1 | 3 | 25 (3 driver) | **99.3%** | 29 min, -j 3, debug kiln, 20 s timeout, box loaded (6 implementers); the 1 fail is coroutines (`co_await`, M4), the 1 timeout is `charconv.msvc/test.pass.cpp` (run) |
| 2026-11-02 | a812270a | stratum B: disjoint 10% sample (790 tests, offset 5 in sorted order) | 730 | 25 | 3 | 31 (4 driver) | **96.2%** | 28 min, -j 3, debug kiln, 20 s timeout, 1 timeout counted as fail, box loaded (6 implementers) |
| 2026-11-02 | 68822097 | same 10% sample (790 tests) after WP-268…276, 279, 280, 282, 287 | 724 | 41 | 3 | 22 (3 driver) | **94.3%** | 24 min, -j 3, debug kiln, 20 s timeout |
| 2026-11-02 | c74c7353 | same 10% sample (790 tests) after WP-286, 288, 289, 291, 295 (#52–#56) | 748 | 17 | 3 | 22 (3 driver) | **97.4%** | 26 min, -j 3, debug kiln, 20 s timeout, box loaded (6 implementers) |
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

## Failure clusters, 2026-11-02 stratum-B sample on a812270a (25 failures)

Of the 25, 13 were already covered by open work at the time: tuple convert_move
(#60), mdspan ×2 (#58 + WP-302), `-verify`/run-environment ×7 (WP-297: filesystem
run assertions, deprecated/`codecvt_byname` verify), value-dependent atomic
constraint after substitution ×2 (WP-307), variant.visit.member ambiguity
(WP-303), coroutines ×2 (M4). The remaining 12 were rerun on main 37814e13 with
`-v`: 2 now pass (counted.iterator conformance, range.take/end), 10 remain:

| Cluster | Tests | WP |
|---|---|---|
| ranges/iterator concept satisfaction: `!projectable<…>` static_asserts fail; `<::` digraph not split (`enable_borrowed_range<::BorrowedRange>`); `ranges::to` deduces `void`; `invoke_result` hard error inside a concept check | projected.compile, subrange/ctor.range_size, to_std_containers, ranges_remove_copy | WP-309 |
| Sema: out-of-class static-member definition makes a second lookup result ("reference to 'oa2' is ambiguous"); `construct_at<Holder<Incomplete>*>` completes `Holder` ("field has incomplete type"); unsigned-`abs` warning missing | random.sample, allocator.traits construct, c.math/abs.verify | WP-310 |
| Run-time: multimap merge `gConstructed == 8`; any emplace throw path `small::count == 1`; `setp.assert` child did not die; `simd_copy` timeout | merge.pass, emplace.pass, setp.assert, simd_copy | WP-310 |

## Failure clusters, 2026-11-02 sample on c74c7353 (17 failures)

| Count | Diagnostic | Where | WP |
|---|---|---|---|
| 3 | run: assertion failed (filesystem perms as root) | filesystems status / is_block_file / replace_filename | WP-297 (env UNSUPPORTED; clang fails identically) |
| 3 | verify: deprecated warning expected but not seen | codecvt_char16_t_char, unary_negate, storage.iterator | WP-297 |
| 1 | verify: static_assert expected (relops_bool_conv) | variant.relops | WP-297 / WP-296 |
| 3 | NYI: substitution into CXXParenListInitExpr / CXXStdInitializerListExpr | time.zone zonedtime ×2, charconv.msvc | WP-296 (PR #57) |
| 1 | reinterpret_cast nullptr_t→integral not allowed | formatter.pointer | WP-296 (PR #57) |
| 1 | cannot delete expression (full-expression cleanups) | pointer_deleter | WP-296 (PR #57) |
| 1 | address of overloaded function does not match ([over.over]/3) | variant robust_against_adl | WP-296 (PR #57) |
| 1 | NYI: atomic constraint value-dependent after substitution | container.adaptors.format | WP-298 |
| 1 | no matching function (format.range formatter) | format.range.formatter | WP-298 |
| 1 | no matching function for call (ranges.remove_if) | alg.remove/ranges.remove_if | NEW → WP-304 |
| 1 | coroutine expressions not supported | support.coroutines | M4 |

Expected after PR #57, WP-297 and WP-298 land: 16 of 17 fixed (one coroutine test deferred to M4).

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
