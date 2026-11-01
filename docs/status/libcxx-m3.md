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

- kiln: debug build of the WP-202 branch (commit in the generated "Run" table below), with main
  `c2fae060` (WP-183, WP-172 merged) merged in.
- Tests and headers: libc++ `llvmorg-19.1.7`. The tests come from the sparse checkout. The headers,
  `libc++.so`, `libc++experimental.a` and `libc++abi` come from the clang 19 packages in the
  container.
- Host: aarch64 Debian 13 container, with `-j 2` and an 8 GiB address-space cap per process.
- Driver: lit-format subset with a static feature set. `.verify.cpp` tests are UNSUPPORTED by the
  driver because kiln has no `-verify`. See `spec/libcxx.md`.

## Reading

**Overall: 497/585 (85.0%)** on the WP-202 branch (main `c2fae060`), up from 487. WP-202 sweeps
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

Per directory, WP-169 → main `e238afa8` → WP-178 → WP-176:

| directory | WP-169 | main | WP-178 | WP-176 |
|---|---:|---:|---:|---:|
| meta | 131/148 | 131/148 | 131/148 | 148/148 |
| utility | 83/145 | 84/145 | 102/145 | 104/145 |
| array | 32/37 | 32/37 | 32/37 | 33/37 |
| string.view | 54/88 | 56/88 | 56/88 | 65/88 |
| span | 18/35 | 18/35 | 20/35 | 20/35 |
| bit | 14/15 | 14/15 | 14/15 | 14/15 |
| support.limits | 83/112 | 83/112 | 83/112 | 93/112 |
| initlist | 5/5 | 5/5 | 5/5 | 5/5 |

**What blocks the rest** (first-error counts; see the generated cluster table):

1. `call to non-static member function without an object argument` (13 first), in
   support.limits (`numeric_limits` members).
2. `no member named '…' in '…'` (11 first). The example is `operator=` in
   `ArchetypeBases::TrivialValueBase<>`.
3. `static assertion expression is not an integral constant expression` (8 first, 29 any), mostly
   the span tests.
4. `member initializer '…' does not name a non-static data member or base class` (8 first), in
   mem.res.
5. `called object type '…' is not a function or function pointer` (8 first, 12 any).
6. `static assertion failed` (6 first), `test_iterators.h:1396`
   (`indirectly_writable<ProxyIterator<int*>, Proxy<int>>`), in span and string.view.
7. `static_cast from '…' to '…' is not allowed` (6 first), in mem.res.

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
| kiln | `kiln version 0.0.1 (C++26 frontend, LLVM IR backend)` at commit `d0c2274a` |
| libc++ tests | `llvmorg-19.1.7` (`libcxx/test`) |
| libc++ headers | `_LIBCPP_VERSION` 190107 |
| target | `aarch64-unknown-linux-gnu` |
| command | `cargo xtask libcxx --gate m3 -j 2` |
| compile flags | `-std=c++26 -stdlib=libc++ -resource-dir <repo>/resource -pthread -ferror-limit=0 -D_LIBCPP_ENABLE_EXPERIMENTAL -I <llvm-project>/libcxx/test/support <ADDITIONAL_COMPILE_FLAGS> <test>` |
| link flags | `t.o -o t.exe -stdlib=libc++ -pthread -lc++experimental -lc++abi` |
| features | `-fsized-deallocation`, `buildhost=linux`, `c++26`, `c++experimental`, `can-create-symlinks`, `has-64-bit-atomics`, `has-fconstexpr-steps`, `has-unix-headers`, `kiln`, `large_tests`, `libcpp-abi-version=1`, `libcpp-has-no-availability-markup`, `libcpp-has-thread-api-pthread`, `linux`, `long_tests`, `optimization=none`, `stdlib=libc++`, `stdlib=llvm-libc++`, `target=aarch64-unknown-linux-gnu` |
| limits | 8192 MiB address space per process (kiln: plus its stack reservation); compile/link timeout 120 s; run timeout 30 s; -j 2 |
| wall time | 450.5 s |
| peak RSS | 84 MiB (largest single process, `std/utilities/utility/mem.res/mem.poly.allocator.class/mem.poly.allocator.mem/construct_types.pass.cpp`; gnu-time) |

### Results by directory

Pass rate is PASS / (total − UNSUPPORTED). "driver" counts the UNSUPPORTED tests the driver cannot run (for example `.verify.cpp`), as opposed to those the configuration excludes (`UNSUPPORTED: c++03`).

| directory | total | pass | fail | xfail | xpass | unsupported (driver) | killed | timeout | error | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `std/utilities/meta` | 155 | 148 | 0 | 0 | 0 | 7 (6) | 0 | 0 | 0 | 100.0% |
| `std/utilities/utility` | 168 | 111 | 34 | 0 | 0 | 23 (19) | 0 | 0 | 0 | 76.6% |
| `std/containers/sequences/array` | 47 | 37 | 0 | 0 | 0 | 10 (10) | 0 | 0 | 0 | 100.0% |
| `std/strings/string.view` | 93 | 79 | 9 | 0 | 0 | 5 (5) | 0 | 0 | 0 | 89.8% |
| `std/containers/views/views.span` | 45 | 24 | 11 | 0 | 0 | 10 (9) | 0 | 0 | 0 | 68.6% |
| `std/numerics/bit` | 17 | 14 | 1 | 0 | 0 | 2 (2) | 0 | 0 | 0 | 93.3% |
| `std/language.support/support.limits` | 112 | 100 | 12 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 89.3% |
| `std/language.support/support.initlist` | 5 | 5 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 100.0% |
| **total** | 642 | 518 | 67 | 0 | 0 | 57 (51) | 0 | 0 | 0 | 88.5% |

### Failures by stage

| directory | compile | link | run | expected-fail-but-succeeded |
|---|---:|---:|---:|---:|
| `std/utilities/meta` | 0 | 0 | 0 | 0 |
| `std/utilities/utility` | 33 | 1 | 0 | 0 |
| `std/containers/sequences/array` | 0 | 0 | 0 | 0 |
| `std/strings/string.view` | 7 | 2 | 0 | 0 |
| `std/containers/views/views.span` | 11 | 0 | 0 | 0 |
| `std/numerics/bit` | 1 | 0 | 0 | 0 |
| `std/language.support/support.limits` | 12 | 0 | 0 | 0 |
| `std/language.support/support.initlist` | 0 | 0 | 0 | 0 |

### Dominant first errors per directory

The three most common first-error clusters among each directory's FAIL tests, with their test counts.

| directory | clusters |
|---|---|
| `std/utilities/meta` | — |
| `std/utilities/utility` | `compile: no member named '…' in '…'` (11); `compile: member initializer '…' does not name a non-static data member or base class` (9); `compile: static_cast from '…' (aka '…') to '…' (aka '…') is not allowed` (6) |
| `std/containers/sequences/array` | — |
| `std/strings/string.view` | `compile: static assertion expression is not an integral constant expression` (7); `link: linker command failed with exit code N (use -v to see invocation)` (2) |
| `std/containers/views/views.span` | `compile: no matching function for call to '…'` (4); `compile: no matching constructor for initialization of '…'` (3); `compile: constexpr if condition is not a constant expression` (2) |
| `std/numerics/bit` | `compile: static assertion failed due to requirement '…'` (1) |
| `std/language.support/support.limits` | `compile: invalid operands to binary expression ('…' (aka '…') and '…')` (7); `compile: use of undeclared identifier '…'` (2); `compile: constexpr if condition is not a constant expression` (1) |
| `std/language.support/support.initlist` | — |

### Error clusters

30 clusters over the FAIL tests (and compiler crashes). A cluster is the failing step's `error:` message with the location dropped, quoted names replaced by `'…'` and numbers by `N`. "first" counts tests whose first error is the cluster, "any" tests that report it anywhere. Top 30:

| # | first | any | cluster | example test | example message |
|---:|---:|---:|---|---|---|
| 1 | 11 | 11 | `compile: no member named '…' in '…'` | `std/utilities/utility/pairs/pairs.pair/assign_const_pair_U_V.pass.cpp` | `no member named 'operator=' in 'ArchetypeBases::TrivialValueBase<>'` |
| 2 | 9 | 13 | `compile: member initializer '…' does not name a non-static data member or base class` | `std/utilities/utility/mem.res/mem.poly.allocator.class/mem.poly.allocator.mem/construct_pair.pass.cpp` | `member initializer 'pair' does not name a non-static data member or base class` |
| 3 | 8 | 17 | `compile: static assertion expression is not an integral constant expression` | `std/containers/views/views.span/span.elem/at.pass.cpp` | `static assertion expression is not an integral constant expression` |
| 4 | 7 | 7 | `compile: invalid operands to binary expression ('…' (aka '…') and '…')` | `std/language.support/support.limits/support.limits.general/charconv.version.compile.pass.cpp` | `invalid operands to binary expression ('chars_format' (aka 'std::chars_format') and 'chars_format')` |
| 5 | 6 | 7 | `compile: static_cast from '…' (aka '…') to '…' (aka '…') is not allowed` | `std/utilities/utility/mem.res/mem.res.aliases/header_list_synop.pass.cpp` | `static_cast from '__base_pointer' (aka 'std::__list_node_base<int, void *> *') to '__node_pointer' (aka 'std::__list_node<int, void *> *') is not allowed` |
| 6 | 4 | 11 | `compile: no matching function for call to '…'` | `std/containers/views/views.span/span.cons/assign.pass.cpp` | `no matching function for call to 'size'` |
| 7 | 3 | 3 | `compile: constexpr if condition is not a constant expression` | `std/containers/views/views.span/span.objectrep/as_bytes.pass.cpp` | `constexpr if condition is not a constant expression` |
| 8 | 3 | 3 | `compile: no matching constructor for initialization of '…'` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `no matching constructor for initialization of 'std::span<const int>'` |
| 9 | 3 | 3 | `compile: static assertion failed due to requirement '…'` | `std/containers/views/views.span/span.cons/iterator_sentinel.pass.cpp` | `static assertion failed due to requirement 'is_constructible_v'` |
| 10 | 3 | 3 | `compile: static assertion failed due to requirement '…': tuple_element index out of range` | `std/utilities/utility/pairs/pairs.pair/assign.pair_like_rv.pass.cpp` | `static assertion failed due to requirement '1UL < 1UL': tuple_element index out of range` |
| 11 | 3 | 3 | `compile: use of undeclared identifier '…'` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `use of undeclared identifier '_Complex'` |
| 12 | 3 | 3 | `link: linker command failed with exit code N (use -v to see invocation)` | `std/strings/string.view/string.view.io/stream_insert.pass.cpp` | `linker command failed with exit code 1 (use -v to see invocation)` |
| 13 | 2 | 2 | `compile: braced-init-list operands of a functional cast is not supported yet` | `std/utilities/utility/pairs/pairs.pair/ctor.brace-init.pass.cpp` | `braced-init-list operands of a functional cast is not supported yet` |
| 14 | 1 | 1 | `compile: expected unqualified-id` | `std/language.support/support.limits/support.limits.general/random.version.compile.pass.cpp` | `expected unqualified-id` |
| 15 | 1 | 1 | `compile: redefinition of '…'` | `std/language.support/support.limits/support.limits.general/expected.version.compile.pass.cpp` | `redefinition of 'operator=='` |
| 16 | 0 | 6 | `compile: pack expansion does not contain any unexpanded parameter packs` | `std/language.support/support.limits/support.limits.general/chrono.version.compile.pass.cpp` | `pack expansion does not contain any unexpanded parameter packs` |
| 17 | 0 | 3 | `compile: a parameter pack may not be accessed at an out of bounds index` | `std/utilities/utility/pairs/pairs.pair/assign.pair_like_rv.pass.cpp` | `a parameter pack may not be accessed at an out of bounds index` |
| 18 | 0 | 3 | `compile: a type specifier is required for all declarations` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `a type specifier is required for all declarations` |
| 19 | 0 | 3 | `compile: expected '…'` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `expected '>'` |
| 20 | 0 | 3 | `compile: expected '…' at end of declaration list` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `expected ';' at end of declaration list` |
| 21 | 0 | 3 | `compile: expected parameter declarator` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `expected parameter declarator` |
| 22 | 0 | 3 | `compile: no type named '…' in '…'` | `std/utilities/utility/pairs/pairs.pair/assign.pair_like_rv.pass.cpp` | `no type named 'type' in 'std::tuple_element<1, std::__tuple_types<int>>'` |
| 23 | 0 | 3 | `compile: non-static data member cannot be constexpr; did you intend to make it static?` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `non-static data member cannot be constexpr; did you intend to make it static?` |
| 24 | 0 | 3 | `compile: static assertion failed due to requirement '…':` | `std/utilities/utility/pairs/pairs.pair/ctor.brace-init.pass.cpp` | `static assertion failed due to requirement '!can_construct_with_brace_init(0)':` |
| 25 | 0 | 2 | `compile: no matching constructor for initialization of '…' (aka '…')` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `no matching constructor for initialization of 'std::span<const std::string>' (aka 'std::span<const std::basic_string<char>>')` |
| 26 | 0 | 1 | `compile: expected a type` | `std/language.support/support.limits/support.limits.general/random.version.compile.pass.cpp` | `expected a type` |
| 27 | 0 | 1 | `compile: invalid operands to binary expression ('…' and '…' (aka '…'))` | `std/language.support/support.limits/support.limits.general/chrono.version.compile.pass.cpp` | `invalid operands to binary expression ('int' and '__flags' (aka 'std::__format_spec::__flags'))` |
| 28 | 0 | 1 | `compile: no viable conversion from '…' (aka '…') to '…' (aka '…')` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `no viable conversion from 'std::string[2]' (aka 'std::basic_string<char>[2]') to 'std::span<const std::string>' (aka 'std::span<const std::basic_string<char>>')` |
| 29 | 0 | 1 | `compile: no viable conversion from '…' to '…'` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `no viable conversion from 'int[2]' to 'std::span<const int>'` |
| 30 | 0 | 1 | `compile: unknown type name '…'` | `std/language.support/support.limits/support.limits.general/random.version.compile.pass.cpp` | `unknown type name '__calc_type'` |

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
