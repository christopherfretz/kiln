# libc++ M3 gate status

This page tracks kiln against the libc++ tests the M3 exit criteria name (ROADMAP M3; the gate list
is `GATE_M3` in `xtask/src/libcxx/mod.rs`). The contract is `spec/libcxx.md`. The section between
the `libcxx` markers is generated; everything outside it is written by hand.

Regenerate with:

```sh
scripts/fetch-libcxx-tests.sh                 # once: sparse llvmorg-19.1.7 checkout under .toolchain/
cargo xtask libcxx --gate m3 -j 2 --report docs/status/libcxx-m3.md
```

## Setup

- kiln: debug build of main `4c3e8be9` (commit in the generated "Run" table below; WP-211 and
  everything through PR #15 merged).
- Tests and headers: libc++ `llvmorg-19.1.7`. The tests come from the sparse checkout. The headers,
  `libc++.so`, `libc++experimental.a` and `libc++abi` come from the clang 19 packages in the
  container.
- Host: aarch64 Debian 13 container, with `-j 2` and an 8 GiB address-space cap per process.
  The wall time was measured while six implementer builds shared the box; it is not a benchmark.
- Driver: lit-format subset with a static feature set. `.verify.cpp` tests are UNSUPPORTED by the
  driver because kiln has no `-verify`. See `spec/libcxx.md`.

## Reading

**Overall: 585/585 (100.0%)** on main `4c3e8be9` (the M3 exit; ROADMAP M3 is closed on this
run), confirming the 585/585 first reached on the WP-211 branch, up from 581. WP-211 clears the last four: GNU `_Complex` (support.limits
`is_specialized` and `complex.version`, utility `pairs.by.type`) and the `if constexpr` on a
static member function of the current instantiation in mdspan's `__rank_` (`mdspan.version`).

Before WP-211: **581/585 (99.3%)** on main `ae178da1` (WP-212/215/213/206 batch + WP-214), up from
576 (WP-212's branch run) and 571 (main after WP-214), with no test newly failing. Per directory:
utility 144/145, support.limits 109/112, string.view 86/88 → 88/88, span 35/35, array 37/37,
bit 15/15, meta 148/148, initlist 5/5. The four remaining failures are all in WP-211's scope
(GNU `_Complex`, mdspan.version `if constexpr`); link failures are 0 since WP-214.

| cluster (main `b7fc2cf7` → WP-207, first / any) | main | WP-207 | root cause |
|---|---:|---:|---|
| `static assertion failed` (`test_iterators.h:1396`) | 6 / 6 | 0 / 0 | a dependent unqualified call re-looked up its name from the instantiating scope; constraint satisfaction runs outside the instantiation context, so `swap(...)` in `ranges::__swap` found `std::swap`, not the deleted poison pill ([temp.dep.candidate]/1). Before WP-199 this also caused `tuple_element index out of range` (3) and the deleted `ranges::begin` call operator (1) |
| `static assertion expression is not an integral constant expression` | 8 / 27 | 1 / 16 | Oven: static data members through `.`/`->`, function lvalues (`static_cast<F&&>(f)` in `__invoke`), the union active-member check on bit-fields (libc++ short string `__is_long_`), trivial `operator=` activating a union member |
| `no matching function for call to '…'` (`size`) | 3 / 7 | 0 / 1 | `const T (&)[N]` deduction against a less-qualified array ([temp.deduct.call]/4.1) |
| `no matching constructor for initialization` (span of volatile) | 3 / 3 | 0 / 0 | an array is not a qualification level of its own ([conv.qual]/3) |
| `constexpr if condition is not a constant expression` | 3 / 3 | 1 / 1 | follows from the fixes above (span `as_bytes` tests; not reduced separately). The one left is mdspan |

Of the 13 tests whose first error was cluster #1 before WP-198's follow-ups, 6 pass (array, span
and string.view `range_concept_conformance`, span `iterator_sentinel` and `range`,
`ranges.version`). The other 7 now stop at errors other packages own: the 3 pairs `pair_like`
tests at `member initializer 'subrange' does not name...` (WP-200) and the 4 string.view
`from_iterator_sentinel`/`from_range`/deduction tests at `called object type '<overloaded
function type>'` (WP-201), which is why that cluster grew (8 → 12 first). The one remaining
integral-constant first error, `span.elem/at.pass.cpp`, needs `std::allocator` in constant
evaluation (`std::vector`; spec/oven.md "Transient allocation", still deferred).
Before WP-207: 517 on the WP-199 branch merged with main `f6eed8fa`, up from 497.
WP-199 (support.limits sweep) takes `std/language.support/support.limits` from 93/112 to 108/112.
Before the merge the branch alone (base `f5592a1c`) scored 495, up from 482 with no test newly
failing, all 13 in support.limits. The root causes were: implicit member access through a
dependent base (`numeric_limits` members), the type of a fixed-type enumerator before the closing
brace (`chars_format` operators), `__extension__` before `using` (random), friends of templated
classes redeclaring nothing before instantiation (expected's `operator==`), constraints checked in
the entity's context, lambdas in pack-expansion patterns, CWG1391 (a parameter with no deducible
template parameter takes no part in deduction: `format_string<type_identity_t<Args>...>`),
parameter type-constraints checked before the requires-clause (basic_format_context), instantiated
variables keeping direct-list-initialization, instantiated bodies using the definition's parameter
names, and completing the derived class before a pointer or `static_cast` derived/base check. The
last one also unblocks mem.res's `static_cast` downcasts, which WP-202's `BaseToDerived` codegen
then compiles, so part of the gain over 497 is in utility. The four left are GNU `_Complex`
(is_specialized, complex.version), a `constexpr if` on a member initializer (mdspan) and a
delegating `pair` constructor (syncstream, WP-200).
Before WP-199: 497 on the WP-202 branch (main `c2fae060`), up from 487. WP-202 sweeps
four codegen/sema "not supported yet" messages, which were the first failure in 10 tests: a
non-constant `std::align_val_t` allocation argument (4: `llvm.assume` align bundle), `cast
BaseToDerived` (4), compound literals of an array temporary (1: `std::to_array`), and implicit
assignment of an array member (1: `array.cons/implicit_copy`). Main's WP-183 and WP-172 also
landed since 487 was recorded and the report was not regenerated for them, so part of the delta
may be theirs. The fifth swept message (`braced-init-list operands of a functional cast`, 2) waits
on WP-200's sema.
Before WP-202: 487 on the WP-198 branch (main `db8c01bb`), up from 482. WP-198 clears
the largest cluster (`no struct named '__segmented_iterator_traits'`, 15 first) and the three that
followed it in the same tests (13 each): qualified friend class templates
(`template <class> friend struct std::__segmented_iterator_traits;`,
`template <bool> friend class take_view<_View>::__sentinel;`), dependent `x = {...}` assignments
(split_view), and a template template parameter as a CTAD placeholder (`ranges::to`'s
`_Container(...)`). Five of the 15 now pass; the rest stop at later errors (`static assertion
failed` in `test_iterators.h`'s `ProxyIterator` checks, `tuple_element index out of range`).
Before WP-198: 482 on the WP-176 branch merged with main `1fafd3f2`, up from 443 on
main (WP-178; 423 before it, 420 after WP-169, 415 after WP-166, 405 on WP-164, 364 at WP-167; 332
before WP-154, 319 after WP-146, 254 after WP-141, 20 at the WP-081 baseline). Part of the gain over
443 comes from WP-171 and WP-170 on main (lax overriding exception specifications, attributes), which
main's report has not been regenerated for.
WP-176 (follow-ups from WP-166) clears `std/utilities/meta`: 148/148 excluding UNSUPPORTED. The
root causes were: [expr.cond]/4 class-operand unification, calls through `F&&` being lvalues,
constructible traits not checking a temporary's destructor (clang `MaybeBindToTemporary`),
undeduced trailing packs not being empty in partial ordering (the `conjunction` ambiguity),
cv-qualified array arguments against `T const[]` partial specializations, noexcept of
pointer-to-member calls, `&L::m` in local classes, designated initializers in templates, and
aggregate `?:` / reference-member codegen. There are no link or run failures, no crashes, and no
KILLED, TIMEOUT or hang results. Every failure is at the compile stage except one link failure in
utility.

Per directory, WP-169 → main `e238afa8` → WP-178 → WP-176 → WP-199 (merged with main `f6eed8fa`) →
WP-207 (merged with main `b7fc2cf7`) → WP-201 (main `58cb7724`) → WP-212 (main `54936d99`) →
WP-211 (main `00061c4e`):

| directory | WP-169 | main | WP-178 | WP-176 | WP-199 | WP-207 | WP-201 | WP-212 | WP-211 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| meta | 131/148 | 131/148 | 131/148 | 148/148 | 148/148 | 148/148 | 148/148 | 148/148 | 148/148 |
| utility | 83/145 | 84/145 | 102/145 | 104/145 | 116/145 | 116/145 | 141/145 | 141/145 | 145/145 |
| array | 32/37 | 32/37 | 32/37 | 33/37 | 36/37 | 36/37 | 37/37 | 37/37 | 37/37 |
| string.view | 54/88 | 56/88 | 56/88 | 65/88 | 68/88 | 73/88 | 79/88 | 86/88 | 88/88 |
| span | 18/35 | 18/35 | 20/35 | 20/35 | 22/35 | 34/35 | 34/35 | 35/35 | 35/35 |
| bit | 14/15 | 14/15 | 14/15 | 14/15 | 14/15 | 14/15 | 15/15 | 15/15 | 15/15 |
| support.limits | 83/112 | 83/112 | 83/112 | 93/112 | 108/112 | 108/112 | 109/112 | 109/112 | 112/112 |
| initlist | 5/5 | 5/5 | 5/5 | 5/5 | 5/5 | 5/5 | 5/5 | 5/5 | 5/5 |

**What blocks the rest:** nothing in the gate list; every non-UNSUPPORTED test passes. The 51
driver-UNSUPPORTED tests (mostly `.verify.cpp`, which needs `-verify`) are the next frontier.

**Caveats.**
- The driver only checks that a `.compile.fail.cpp` test fails to compile without a crash, not
  that it fails with the diagnostic it targets.
- XFAILs written against compiler identity (`clang-*`, `gcc-*`) evaluate to false, because the
  feature set names only `kiln`.

<!-- libcxx:begin -->
<!-- generated by `cargo xtask libcxx --report`; edits inside the markers are overwritten -->

### Run

| | |
|---|---|
| kiln | `kiln version 0.0.1 (C++26 frontend, LLVM IR backend)` at commit `4c3e8be9` |
| libc++ tests | `llvmorg-19.1.7` (`libcxx/test`) |
| libc++ headers | `_LIBCPP_VERSION` 190107 |
| target | `aarch64-unknown-linux-gnu` |
| command | `cargo xtask libcxx --gate m3 -j 2` |
| compile flags | `-std=c++26 -stdlib=libc++ -resource-dir <repo>/resource -pthread -ferror-limit=0 -D_LIBCPP_ENABLE_EXPERIMENTAL -I <repo>/.toolchain/llvm-project/libcxx/test/support <ADDITIONAL_COMPILE_FLAGS> <test>` |
| link flags | `t.o -o t.exe -stdlib=libc++ -pthread -lc++experimental -lc++abi` |
| features | `-fsized-deallocation`, `buildhost=linux`, `c++26`, `c++experimental`, `can-create-symlinks`, `has-64-bit-atomics`, `has-fconstexpr-steps`, `has-unix-headers`, `kiln`, `large_tests`, `libcpp-abi-version=1`, `libcpp-has-no-availability-markup`, `libcpp-has-thread-api-pthread`, `linux`, `long_tests`, `optimization=none`, `stdlib=libc++`, `stdlib=llvm-libc++`, `target=aarch64-unknown-linux-gnu` |
| limits | 8192 MiB address space per process (kiln: plus its stack reservation); compile/link timeout 120 s; run timeout 30 s; -j 2 |
| wall time | 548.8 s |
| peak RSS | 78 MiB (largest single process, `std/utilities/utility/mem.res/mem.poly.allocator.class/mem.poly.allocator.mem/construct_types.pass.cpp`; gnu-time) |

### Results by directory

Pass rate is PASS / (total − UNSUPPORTED). "driver" counts the UNSUPPORTED tests the driver cannot run (for example `.verify.cpp`), as opposed to those the configuration excludes (`UNSUPPORTED: c++03`).

| directory | total | pass | fail | xfail | xpass | unsupported (driver) | killed | timeout | error | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `std/utilities/meta` | 155 | 148 | 0 | 0 | 0 | 7 (6) | 0 | 0 | 0 | 100.0% |
| `std/utilities/utility` | 168 | 145 | 0 | 0 | 0 | 23 (19) | 0 | 0 | 0 | 100.0% |
| `std/containers/sequences/array` | 47 | 37 | 0 | 0 | 0 | 10 (10) | 0 | 0 | 0 | 100.0% |
| `std/strings/string.view` | 93 | 88 | 0 | 0 | 0 | 5 (5) | 0 | 0 | 0 | 100.0% |
| `std/containers/views/views.span` | 45 | 35 | 0 | 0 | 0 | 10 (9) | 0 | 0 | 0 | 100.0% |
| `std/numerics/bit` | 17 | 15 | 0 | 0 | 0 | 2 (2) | 0 | 0 | 0 | 100.0% |
| `std/language.support/support.limits` | 112 | 112 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 100.0% |
| `std/language.support/support.initlist` | 5 | 5 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 100.0% |
| **total** | 642 | 585 | 0 | 0 | 0 | 57 (51) | 0 | 0 | 0 | 100.0% |

### Failures by stage

| directory | compile | link | run | expected-fail-but-succeeded |
|---|---:|---:|---:|---:|
| `std/utilities/meta` | 0 | 0 | 0 | 0 |
| `std/utilities/utility` | 0 | 0 | 0 | 0 |
| `std/containers/sequences/array` | 0 | 0 | 0 | 0 |
| `std/strings/string.view` | 0 | 0 | 0 | 0 |
| `std/containers/views/views.span` | 0 | 0 | 0 | 0 |
| `std/numerics/bit` | 0 | 0 | 0 | 0 |
| `std/language.support/support.limits` | 0 | 0 | 0 | 0 |
| `std/language.support/support.initlist` | 0 | 0 | 0 | 0 |

### Dominant first errors per directory

The three most common first-error clusters among each directory's FAIL tests, with their test counts.

| directory | clusters |
|---|---|
| `std/utilities/meta` | — |
| `std/utilities/utility` | — |
| `std/containers/sequences/array` | — |
| `std/strings/string.view` | — |
| `std/containers/views/views.span` | — |
| `std/numerics/bit` | — |
| `std/language.support/support.limits` | — |
| `std/language.support/support.initlist` | — |

### Error clusters

0 clusters over the FAIL tests (and compiler crashes). A cluster is the failing step's `error:` message with the location dropped, quoted names replaced by `'…'` and numbers by `N`. "first" counts tests whose first error is the cluster, "any" tests that report it anywhere. Top 0:

| # | first | any | cluster | example test | example message |
|---:|---:|---:|---|---|---|

### Crashes

None.

### KILLED, TIMEOUT, ERROR and XPASS

None.

### UNSUPPORTED reasons

| reason | tests |
|---|---:|
| `driver: -verify` | 51 |
| `REQUIRES: c++03` | 1 |
| `REQUIRES: c++11` | 1 |
| `REQUIRES: c++11 \|\| c++14 \|\| c++17 \|\| c++20` | 1 |
| `REQUIRES: libcpp-hardening-mode=debug` | 1 |
| `REQUIRES: libcpp-hardening-mode={{extensive\|debug}}` | 1 |
| `REQUIRES: objective-c++` | 1 |

<!-- libcxx:end -->
