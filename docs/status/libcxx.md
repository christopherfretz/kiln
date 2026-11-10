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
| 2026-11-02 | wp/413 (51a1bd16) | stratum A: same 10% sample (790 tests) after WP-413, merged with main 201eb411 | 777 | 0 | 3 | 10 (0 driver) | **99.6%** | -j 3, harness kiln, 20 s timeout, 232 s. vs wp/394 (774/3/3/10): the 3 FAILs are fixed: `bitset.members/not_all` and `bitset.operators/op_not` (`vector<bitset<0>>`: an empty object owning its byte did not begin its lifetime at placement when that byte is an array element, vm.rs `placement`), `list.cons/from_range` (a concept-id over instantiation-dependent-only arguments such as `TC<N>*`, `N` a constant local, was treated as value-dependent and its satisfaction cached for the sugared arguments; [temp.constr.atomic]/3, spec/sema-templates.md §8.1). M3 gate on the same sha: 636/642 pass, 6 unsupported, 0 fail |
| 2026-11-02 | wp/394 (5f674d92) | stratum A: same 10% sample (790 tests) after WP-394 (kiln++ default `-fclang-compat=22`) | 774 | 3 | 3 | 10 (0 driver) | **99.2%** | -j 3, harness kiln, 20 s timeout. vs 88bbf786 (774/0/3/13): the 3 FAILs (`bitset.members/not_all`, `bitset.operators/op_not`: static assertion not an ICE; `list.cons/from_range`: NTTP kind not implemented) are the 3 formerly driver-unsupported tests that WP-386's `lit.local.cfg` parsing now runs; they fail identically under `KILN_ORACLE=19`, so the flip itself changes no outcome |
| 2026-11-02 | wp/386 (3b2dee81) | stratum A (790 tests) under **clang 22** (clang mode, `--compiler clang++-22`, the harness self-check; spec/libcxx.md "Clang mode") vs kiln on the same sha | clang 776 / kiln 777 | clang 1 / kiln 0 | 3 / 3 | 10 (0 driver) / 10 | clang **99.5%** / kiln **99.6%** | -j 3, 20 s timeout; clang 178.8 s, harness kiln 328.4 s. The one delta: `func.bind_front/bind_front.verify.cpp` fails under clang 22 only; re-run in WP-402 (oracle 22): kiln PASS, clang++-19 PASS, clang++-22 FAIL (`call to deleted constructor of 'NotMoveConst'`), which libc++ 22's copy of the test allows (`expected-error@*:* 0-1`) and libc++ 19's does not: a test-version difference, not a kiln or harness bug |
| 2026-11-02 | fe5406b9 | **full suite**, first run: `std` + `libcxx` = 8705 tests (`libcxx/`'s 807 are all driver-unsupported: `lit.local.cfg`); pass rate is over `std/` | 7469 | 92 | 29 | 1110 (830 driver) | **98.3%** | 38 min, -j 3, harness kiln, 20 s run timeout, box loaded (6 implementers); 5 TIMEOUT (rand.dist ×3, semaphore lost_wakeup, charconv.msvc) all pass at 240 s; failures clustered into WP-334…339 (coroutines ×10 wait for M4; ~150 UNSUPPORTED are missing locales, see WP-340) |
| 2026-11-02 | 88bbf786 | stratum A: same 10% sample (790 tests) after WP-365, 368, 370, 374, 378 (#114, #115, #118, #120, #121), both oracles | 774 | 0 | 3 | 13 (3 driver) | **99.6%** | -j 3, debug kiln, 20 s timeout; identical under `KILN_ORACLE=19` and `22` |
| 2026-11-02 | 93ee57ff | stratum A: same 10% sample (790 tests) after WP-303, 307, 308 (#70–#72) | 761 | 1 | 3 | 25 (3 driver) | **99.5%** | 27 min, -j 3, debug kiln, 20 s timeout, box loaded (7 implementers); the 1 fail is a coroutines test (`co_await`, M4) |
| 2026-11-02 | 0d51dec6 | stratum B: second 10% sample (790 tests, offset 5) after WP-294, 311, 313 (#73–#75) | 750 | 2 | 3 | 35 (4 driver) | **99.3%** | 15 min, -j 3, debug kiln, 20 s timeout, box loaded (6 implementers); both fails are coroutines tests (`co_await`, M4) |
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
  cargo xtask libcxx --no-fail -j 3 --report sample.json $(cat ../sample.list) | tee ../sample.log
```

The rows above were measured with a debug kiln and `--timeout 20`. Stratum
and full-suite runs now use the runner's defaults (`spec/libcxx.md` "Build
profiles", "Timeouts and retries"): the `harness` profile (optimized, no LTO,
incremental; built once per checkout), a 120 s limit per compile or link step
and 30 s per run (240 s for the long-running directories). A debug kiln
(`--profile debug`, 300 s compile limit) is several times slower on
template-heavy tests and is for debugging only; the M3 gate takes ~1 min with
the default profile and ~10 min with `--profile debug`. For kiln-vs-clang
compile time on heavy libc++ tests, use the bench's libc++ slice
(`cargo xtask bench --compare-with clang++-22 bench/libcxx-slice`,
`docs/status/perf.md` "libc++ slice").

Only failures are printed; cluster them with
`grep "FAIL (" sample.log | sed 's/.* — //' | sed "s/'[^']*'/'X'/g" | sort | uniq -c | sort -rn`.
The log elides symbols as `…`; reproduce a cluster with `kiln++ -fsyntax-only`
and the harness flags (`spec/libcxx.md`).

## M5: libc++ 22 full suite

ROADMAP M5: the whole libc++ 22.1.8 suite (`std/` + `libcxx/`, 9247 tests)
under `-std=c++26 -fclang-compat=22`, with the WP-418 harness (libc++'s
own `_warningFlags`). Failure clusters, fixes and proposed work packages:
[libcxx-m5.md](libcxx-m5.md).

| Date | kiln main | Scope | pass | fail | xfail | unsupported | pass rate | wall | peak RSS | environment |
|---|---|---|---|---|---|---|---|---|---|---|
| 2026-11-10 | final 1.0 RC `ae54fd2fa` (the release archive's `kiln++`) | **full suite** ([libcxx-1.0.md](libcxx-1.0.md)) | 8747 | 4 (+2 timeout, 1 xpass) | 47 | 446 | **99.4%** | — | — | as below; `lost_wakeup` passes alone (21.4 s) |
| 2026-11-10 | first 1.0 RC `e6199e693` (the release archive's `kiln++`) | **full suite** ([libcxx-1.0.md](libcxx-1.0.md)) | 8748 | 4 (+1 timeout, 1 xpass) | 47 | 446 (31 driver) | **99.4%** | 58 min (3462 s), -j 3, 20 s run timeout (240 s long), 3072 MiB cap. `std/` 99.5%, `libcxx/` 97.9% | 2049 MiB (the `pbump2gig.pass` test binary); largest kiln compile 620 MiB | as below; the 4 FAILs are permission tests that pass only when not root |
| 2026-11-03 | wp/446 (ced0e9ad, main b4e8dfb3 merged) | the 196 first-run FAIL/TIMEOUT tests re-run after the WP-446 fixes; full-suite counts with them substituted | 8708 | 48 | 36 | 455 (31 driver) | **99.0%** | -j 3, harness kiln, 20 s run timeout (240 s long), 3072 MiB cap. `std/` 99.4%, `libcxx/` 92.7% | — | as below |
| 2026-11-03 | wp/446 base (c77a092a)¹ | **full suite**, first run: `std` 8424 + `libcxx` 823 (libc++ 22's `libcxx/` tests run; under 19 they were all driver-unsupported) | 8562 | 195 (+1 timeout) | 34 | 455 (31 driver) | **97.4%** | 69 min (4121 s), -j 3, harness kiln, 20 s run timeout (240 s long), 3072 MiB cap. `std/` 97.9%, `libcxx/` 89.3% | 2434 MiB (the `gcount.pass` test binary); largest kiln++ compile 704 MiB (`simd_copy`) | `locale -a` has all 7 lit locales (`scripts/provision-locales.sh`); running as root (uid 0); aarch64, 24 cores, 31 GiB |

¹ The report stamps `c2e88d0a`, the head when the run finished (see
libcxx-m5.md). Pass rate = pass / (total − unsupported), as the
`--report` table computes it.

## M4 baseline: libc++ 22 (WP-355)

First run of the M4 gate (`cargo xtask libcxx --gate m4`, contract in
`spec/libcxx.md` "Gates" and "Toolchain") on the libc++ 22.1.8 tests
(`llvmorg-22.1.8`) and headers (`_LIBCPP_VERSION` 220108, the extracted
`.toolchain/libcxx-22-root`), `-std=c++26 -fclang-compat=22`, on WP-355's
`0d6ebb48`, harness kiln, `-j 3 --timeout 20 --mem-limit 3072`, 622 s wall,
peak RSS 384 MiB.

| Scope | total | pass | fail | unsupported (driver) | pass rate |
|---|---:|---:|---:|---:|---:|
| 21 M4 directories (`<array>`…`<flat_map>`, see the gate list) | 2661 | 290 | 2281 | 90 (7) | **11.3%** |

The number is low for two reasons, not hundreds: 1826 first failures (2080
in all) are libc++ 22's `pointer_traits` rebind `static_assert` ("Cannot
rebind pointer; did you forget to add a rebind member to your pointer?"),
reached from every container/allocator test; 332 first (2132 in all) are
`expected '…'` parse errors at libc++ 22's `_Nonnull`/`_Nullable`-annotated
declarations. The next 91 are a requires-expression pack expansion
("pack expansion does not contain any unexpanded parameter packs",
`tuple.rel`). These three clusters are the first M4 libc++ work packages;
`std/utilities/meta` (91.1%) and `std/numerics/bit` (93.8%) already pass.

**After WP-398** (nullability as type sugar, requires-expression pack
dependence, injected-class-name as a template template argument,
canonical template-id dependence, mutually recursive constexpr
instantiation), same flags at `-j 4`, 924 s wall:

| Scope | total | pass | fail | unsupported (driver) | pass rate |
|---|---:|---:|---:|---:|---:|
| 21 M4 directories | 2661 | 2448 | 130 | 83 (7) | **95.0%** |

From the WP-398 start (302 / 2276 / 83, 11.7%, on main 4603daf9). The
remaining clusters: 83 `expected '…'` at libc++ 22's `bitset:884`
`static_cast<unsigned _BitInt(_Size)>` (kiln_parse has no `_BitInt`; the
types exist in kiln_ast); 8 `no matching function` in `pstl.sort` /
`stable_sort` (the mutual-recursion fix landed after this run); 6 P2418
`no matching constructor` at `__format/buffer.h:362`, a class template's
static member function converted to a function pointer in a base braced
mem-initializer; then 4 `substitution into InitListExpr` and 4
`non-type template argument of this kind` (`not yet implemented`).

**After WP-404** (`_BitInt(N)` end to end, P2418 static member function
to function pointer in a base braced mem-initializer, substitution into
`InitListExpr` and float / bound-temporary NTTPs, nullability leftovers,
the definition-time template template argument check, class redefinition
recovery, a builtin type transform over an instantiation-dependent
operand), on wp/404 merged with main f4c0e909, same flags
(`-j 4 --mem-limit 3072`), 778.1 s wall, peak RSS 672 MiB
(`variant.visit/visit_return_type.pass.cpp`, the kiln compile step):

| Scope | total | pass | fail | unsupported (driver) | pass rate |
|---|---:|---:|---:|---:|---:|
| 21 M4 directories | 2661 | 2566 | 15 | 80 (0) | **99.4%** |

The `_BitInt`, `pstl.sort` / `stable_sort`, P2418 and both
`not yet implemented` clusters are gone, as is the subrange CTAD cluster
the clang 22 alias sugar exposed (9 range insertion tests,
`__remove_reference_t` over `TC<N>*`). The 15 remaining failures are
single tests: 1 `self-comparison always evaluates to false`
(`array/compare.verify`: clang 22 warns too, and libc++'s lit passes
`-Wno-tautological-compare` in `params.py` `_warningFlags`, which the
harness does not apply); 6 `static assertion expression is not an integral constant
expression` (`ranges.swap_ranges`, `ranges.ends_with`, `flat.map`
`initializer_list` and `iter_iter`, `range.single.view/assign`,
`is_implicit_lifetime.verify`); 2 `.verify` `invalid application of
'sizeof' to an incomplete type` (`allocator` `allocate` /
`allocate_at_least`); 1 IR lowering `phi incoming value: expected ptr,
found iN` (`ranges_nth_element`, a codegen bug); 1 `call to deleted
constructor` (`ranges_replace_copy_if`); 1 `static_assert` failure
(`iterator_robust_against_adl`); 1 `no member named … in namespace`
(`is_within_lifetime`); 1 missing `implicit instantiation of undefined
template` (`tuple_size_incomplete.verify`); 1 `not implicitly convertible`
(`tuple_size_structured_bindings`).

**After WP-416** (libc++ 22's `std-at-least-c++03`..`c++26` lit features,
`params.py` `std`), on wp/416 merged with main f8e1183d, same flags
(`-j 4 --mem-limit 3072`), 1501.6 s wall on a loaded host, peak RSS
676 MiB (`variant.visit/visit_return_type.pass.cpp`, the kiln compile
step):

| Scope | total | pass | fail | unsupported (driver) | pass rate |
|---|---:|---:|---:|---:|---:|
| 21 M4 directories | 2661 | 2614 | 20 | 27 (0) | **99.2%** |

The 53 tests gated on `std-at-least-c++NN` now run: 48 pass and 5 fail,
none a regression. The new failures: 1 more `static assertion expression
is not an integral constant expression` (`is_implicit_lifetime.pass`, the
`.verify` sibling's cause); 2 `'is_pod_v<int>'` / `'is_trivial_v<…>'
is deprecated` not warned (a deprecated variable template's use);
1 `returning reference to local temporary object` not diagnosed in
`<tuple>` (`make_from_tuple.verify`); 1 `no type named 'argument_type'`
reported as `expected a type` (`variant.hash/hash.depr.verify`).

**After WP-418** (the WP-404 follow-ups (a)–(d), and libc++ 22's
`params.py` `_warningFlags` applied to every compile: `-Werror -Wall
-Wextra …` plus the `-Wno-…` list and
`-D_LIBCPP_HAS_NO_PRAGMA_SYSTEM_HEADER`; `%{verify}` adds `-Wno-error`
and undefines the macro, as lit does), on wp/418 merged with main
d2c08217, same flags (`-j 4 --mem-limit 3072`), 1299.4 s wall on a
loaded host, peak RSS
677 MiB (`variant.visit/visit_return_type.pass.cpp`, the kiln compile
step):

| Scope | total | pass | fail | unsupported (driver) | pass rate |
|---|---:|---:|---:|---:|---:|
| 21 M4 directories | 2661 | 2632 | 2 | 27 (0) | **99.9%** |

All 15 WP-404 failures pass, and `is_implicit_lifetime.pass` with them
(an ineligible constrained constructor, [special]/6). The warning flags
surfaced 11 more failures, all fixed (a `-Werror` warning in a SFINAE
context is not a substitution failure; `-Wbraced-scalar-init` is silent
in an instantiation; a variable named by a reused non-dependent
expression is odr-used). The 2 remaining are WP-416's: 1
`returning reference to local temporary object` not diagnosed in
`<tuple>` (`make_from_tuple.verify`); 1 `no type named 'argument_type'`
reported as `expected a type` (`variant.hash/hash.depr.verify`).
WP-432 re-ran WP-416's 2 deprecated variable-template failures
(`meta.unary.prop/is_pod.deprecated.verify`,
`is_trivial.deprecated.verify`): both pass.

**After WP-445** (the WP-418 correctness follow-ups), on wp/445 78c5a251
merged with main b4e8dfb3, same flags (`-j 4 --mem-limit 3072`), 906.9 s
wall on a loaded host, peak RSS 683 MiB
(`variant.visit/visit_return_type.pass.cpp`, the kiln compile step):

| Scope | total | pass | fail | unsupported (driver) | pass rate |
|---|---:|---:|---:|---:|---:|
| 21 M4 directories | 2661 | 2634 | 0 | 27 (0) | **100.0%** |

`make_from_tuple.verify` passes (`-Wreturn-stack-address` sees the
temporary through a reference cast), and so does `hash.depr.verify` (a
qualified name in a type-only context that names no type gets the lookup
error). `-flax-vector-conversions=none` is now accepted, so the
`_warningFlags` probe passes it to every compile.

## Environment

What `cargo xtask libcxx` detects about the host, and what stays
UNSUPPORTED because of it (WP-340; contract: `spec/libcxx.md` "Feature
set", "Environment exclusions", "Timeouts and retries"):
- **Locales.** The `locale.*` features come from `locale -a`; run
  `scripts/provision-locales.sh` (as root, idempotent) once per container to
  generate the seven libc++ asks for. With them, `std/localization`,
  `std/utilities/format`, `std/re` and `std/input.output` are 1268 tests:
  1199 pass, 2 fail, 11 xfail, 56 unsupported (98.9%; harness kiln on
  71f03f99, -j 3, 315 s). Without them about 150 of those tests are
  UNSUPPORTED. The 2 failures are kiln bugs: `locale-specific_form.pass.cpp`
  (`basic_string<char, char_traits<char>, char[1]>` instantiated while
  resolving the `test` overload set) and `locale.stdcvt/remove.verify.cpp`
  (`std::codecvt_utf8<wchar_t>` for an absent member template gives
  `expected unqualified-id`, not `no template named …`).
- **Root.** The agent containers run as root, so 23 filesystem tests that
  expect `perms::none` to deny access stay UNSUPPORTED (`env: running as
  root: perms::none does not deny access`).
- **Host-shared scratch.** 2 filesystem tests stay UNSUPPORTED while the
  scratch directory is on a `fakeowner`/`virtiofs` bind mount.
- **glibc locale data.** glibc 2.41's `en_US.UTF-8` groups digits by `3`,
  not `3;3`: the 2 byname `grouping.pass.cpp` tests are UNSUPPORTED (clang
  19 fails them the same way).
- **Long tests.** `REQUIRES: long_tests` tests and those in
  `xtask/src/libcxx/slow_tests.txt` run under `--run-timeout-long` (240 s):
  the three `rand.dist` `eval.pass.cpp`, `lost_wakeup.pass.cpp`,
  `charconv.msvc/test.pass.cpp` and `buffered_writes.pass.cpp` (slow on the
  host-shared scratch mount) all pass in a harness run.
  `ALLOW_RETRIES: N` reruns a failing binary.

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
| 6 | `.verify.cpp` expectation mismatches | diag wording | WP-143/WP-253 (closed: all six pass since WP-367; gate m3 636 pass + 6 unsupported, re-checked by WP-391) |
| 42 | static assertions, no matching function, link failures, access/abstractness false positives, rewritten comparisons, … | long tail, reduced per test with cause, reproducer and status in [libcxx-longtail.md](libcxx-longtail.md) | WP-276, WP-289 |

Fixing the five largest clusters (129 of 210) should move the sample to about
90%; several tests will then surface a second failure, so the next sample run
decides the following batch.

## Per-directory sizes (full checkout, `std/`)

containers 1493, utilities 1366, numerics 889, input.output 636, strings 429,
ranges 423, time 417, thread 340, localization 332, iterators 309, algorithms
307, language.support 281, re 188, depr 154, atomics 118, experimental 101,
diagnostics 72, concepts 36.
