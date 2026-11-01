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

- kiln: debug build of the WP-199 branch (commit in the generated "Run" table below), based on
  main `f5592a1c`.
- Tests and headers: libc++ `llvmorg-19.1.7`. The tests come from the sparse checkout. The headers,
  `libc++.so`, `libc++experimental.a` and `libc++abi` come from the clang 19 packages in the
  container.
- Host: aarch64 Debian 13 container, with `-j 2` and an 8 GiB address-space cap per process.
- Driver: lit-format subset with a static feature set. `.verify.cpp` tests are UNSUPPORTED by the
  driver because kiln has no `-verify`. See `spec/libcxx.md`.

## Reading

**Overall: 495/585 (84.6%)** on the WP-199 branch (base main `f5592a1c`), up from 482 on
WP-176 merged with main `1fafd3f2`, and from 443 on main (WP-178; 423 before it, 420 after WP-169, 415 after WP-166, 405 on WP-164, 364 at WP-167; 332
before WP-154, 319 after WP-146, 254 after WP-141, 20 at the WP-081 baseline). Part of the gain over
443 comes from WP-171 and WP-170 on main (lax overriding exception specifications, attributes), which
main's report has not been regenerated for.
WP-199 (support.limits sweep) takes `std/language.support/support.limits` from 93/112 to 106/112
with no test newly failing. The root causes were: implicit member access through a dependent
base (`numeric_limits` members), the type of a fixed-type enumerator before the closing brace
(`chars_format` operators), `__extension__` before `using` (random), friends of templated
classes redeclaring nothing before instantiation (expected's `operator==`), constraints
checked in the entity's context, lambdas in pack-expansion patterns, CWG1391 (a parameter with
no deducible template parameter takes no part in deduction:
`format_string<type_identity_t<Args>...>`), parameter type-constraints checked before the
requires-clause, instantiated variables keeping direct-list-initialization, instantiated bodies
using the definition's parameter names, and completing the derived class before a pointer or
`static_cast` derived/base check. The last one moves the mem.res `static_cast` failures on to
codegen (`cast BaseToDerived is not supported yet`). The six left are GNU `_Complex`
(is_specialized, complex.version), `__any_imp::_SmallHandler` (any), a `constexpr if` on a
member initializer (mdspan), `variant<>` (ranges) and a delegating `pair` constructor
(syncstream, WP-200).
WP-176 (follow-ups from WP-166) clears `std/utilities/meta`: 148/148 excluding UNSUPPORTED. The
root causes were: [expr.cond]/4 class-operand unification, calls through `F&&` being lvalues,
constructible traits not checking a temporary's destructor (clang `MaybeBindToTemporary`),
undeduced trailing packs not being empty in partial ordering (the `conjunction` ambiguity),
cv-qualified array arguments against `T const[]` partial specializations, noexcept of
pointer-to-member calls, `&L::m` in local classes, designated initializers in templates, and
aggregate `?:` / reference-member codegen. There are no link or run failures, no crashes, and no
KILLED, TIMEOUT or hang results. Every failure is at the compile stage except one link failure in
utility.

Per directory, WP-169 → main `e238afa8` → WP-178 → WP-176 → WP-199:

| directory | WP-169 | main | WP-178 | WP-176 | WP-199 |
|---|---:|---:|---:|---:|---:|
| meta | 131/148 | 131/148 | 131/148 | 148/148 | 148/148 |
| utility | 83/145 | 84/145 | 102/145 | 104/145 | 104/145 |
| array | 32/37 | 32/37 | 32/37 | 33/37 | 33/37 |
| string.view | 54/88 | 56/88 | 56/88 | 65/88 | 65/88 |
| span | 18/35 | 18/35 | 20/35 | 20/35 | 20/35 |
| bit | 14/15 | 14/15 | 14/15 | 14/15 | 14/15 |
| support.limits | 83/112 | 83/112 | 83/112 | 93/112 | 106/112 |
| initlist | 5/5 | 5/5 | 5/5 | 5/5 | 5/5 |

**What blocks the rest** (first-error counts; see the generated cluster table):

1. `no struct named '…' in '…'` (15 first). This is `__ranges/to.h:181`,
   `decltype(_Container(std::declval<...>()...))` ("this kind of name is not supported yet").
2. `cast BaseToDerived is not supported yet` (10 first), in mem.res: codegen of a
   `static_cast` downcast (sema accepted it in WP-199).
3. `no member named '…' in '…'` (11 first). The example is `operator=` in
   `ArchetypeBases::TrivialValueBase<>`.
4. `member initializer '…' does not name a non-static data member or base class` (8 first), in
   mem.res.
5. `called object type '…' is not a function or function pointer` (8 first, 12 any).
6. `static assertion expression is not an integral constant expression` (7 first, 29 any), mostly
   the span tests.
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
| kiln | `kiln version 0.0.1 (C++26 frontend, LLVM IR backend)` at commit `4e71042b` |
| libc++ tests | `llvmorg-19.1.7` (`libcxx/test`) |
| libc++ headers | `_LIBCPP_VERSION` 190107 |
| target | `aarch64-unknown-linux-gnu` |
| command | `cargo xtask libcxx --gate m3 -j 2` |
| compile flags | `-std=c++26 -stdlib=libc++ -resource-dir <repo>/resource -pthread -ferror-limit=0 -D_LIBCPP_ENABLE_EXPERIMENTAL -I <llvm-project>/libcxx/test/support <ADDITIONAL_COMPILE_FLAGS> <test>` |
| link flags | `t.o -o t.exe -stdlib=libc++ -pthread -lc++experimental -lc++abi` |
| features | `-fsized-deallocation`, `buildhost=linux`, `c++26`, `c++experimental`, `can-create-symlinks`, `has-64-bit-atomics`, `has-fconstexpr-steps`, `has-unix-headers`, `kiln`, `large_tests`, `libcpp-abi-version=1`, `libcpp-has-no-availability-markup`, `libcpp-has-thread-api-pthread`, `linux`, `long_tests`, `optimization=none`, `stdlib=libc++`, `stdlib=llvm-libc++`, `target=aarch64-unknown-linux-gnu` |
| limits | 8192 MiB address space per process (kiln: plus its stack reservation); compile/link timeout 120 s; run timeout 30 s; -j 2 |
| wall time | 396.6 s |
| peak RSS | 84 MiB (largest single process, `std/utilities/utility/mem.res/mem.poly.allocator.class/mem.poly.allocator.mem/construct_types.pass.cpp`; gnu-time) |

### Results by directory

Pass rate is PASS / (total − UNSUPPORTED). "driver" counts the UNSUPPORTED tests the driver cannot run (for example `.verify.cpp`), as opposed to those the configuration excludes (`UNSUPPORTED: c++03`).

| directory | total | pass | fail | xfail | xpass | unsupported (driver) | killed | timeout | error | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `std/utilities/meta` | 155 | 148 | 0 | 0 | 0 | 7 (6) | 0 | 0 | 0 | 100.0% |
| `std/utilities/utility` | 168 | 104 | 41 | 0 | 0 | 23 (19) | 0 | 0 | 0 | 71.7% |
| `std/containers/sequences/array` | 47 | 33 | 4 | 0 | 0 | 10 (10) | 0 | 0 | 0 | 89.2% |
| `std/strings/string.view` | 93 | 65 | 23 | 0 | 0 | 5 (5) | 0 | 0 | 0 | 73.9% |
| `std/containers/views/views.span` | 45 | 20 | 15 | 0 | 0 | 10 (9) | 0 | 0 | 0 | 57.1% |
| `std/numerics/bit` | 17 | 14 | 1 | 0 | 0 | 2 (2) | 0 | 0 | 0 | 93.3% |
| `std/language.support/support.limits` | 112 | 106 | 6 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 94.6% |
| `std/language.support/support.initlist` | 5 | 5 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 100.0% |
| **total** | 642 | 495 | 90 | 0 | 0 | 57 (51) | 0 | 0 | 0 | 84.6% |

### Failures by stage

| directory | compile | link | run | expected-fail-but-succeeded |
|---|---:|---:|---:|---:|
| `std/utilities/meta` | 0 | 0 | 0 | 0 |
| `std/utilities/utility` | 40 | 1 | 0 | 0 |
| `std/containers/sequences/array` | 4 | 0 | 0 | 0 |
| `std/strings/string.view` | 23 | 0 | 0 | 0 |
| `std/containers/views/views.span` | 15 | 0 | 0 | 0 |
| `std/numerics/bit` | 1 | 0 | 0 | 0 |
| `std/language.support/support.limits` | 6 | 0 | 0 | 0 |
| `std/language.support/support.initlist` | 0 | 0 | 0 | 0 |

### Dominant first errors per directory

The three most common first-error clusters among each directory's FAIL tests, with their test counts.

| directory | clusters |
|---|---|
| `std/utilities/meta` | — |
| `std/utilities/utility` | `compile: no member named '…' in '…'` (11); `compile: cast BaseToDerived is not supported yet` (10); `compile: member initializer '…' does not name a non-static data member or base class` (8) |
| `std/containers/sequences/array` | `compile: compound literals of an array temporary is not supported yet` (1); `compile: implicit assignment of an array of this type is not supported yet` (1); `compile: no struct named '…' in '…'` (1) |
| `std/strings/string.view` | `compile: called object type '…' is not a function or function pointer` (8); `compile: static assertion expression is not an integral constant expression` (5); `compile: no struct named '…' in '…'` (5) |
| `std/containers/views/views.span` | `compile: no struct named '…' in '…'` (4); `compile: static assertion expression is not an integral constant expression` (3); `compile: no matching function for call to '…'` (3) |
| `std/numerics/bit` | `compile: static assertion failed due to requirement '…'` (1) |
| `std/language.support/support.limits` | `compile: no struct named '…' in '…'` (2); `compile: use of undeclared identifier '…'` (2); `compile: constexpr if condition is not a constant expression` (1) |
| `std/language.support/support.initlist` | — |

### Error clusters

33 clusters over the FAIL tests (and compiler crashes). A cluster is the failing step's `error:` message with the location dropped, quoted names replaced by `'…'` and numbers by `N`. "first" counts tests whose first error is the cluster, "any" tests that report it anywhere. Top 33:

| # | first | any | cluster | example test | example message |
|---:|---:|---:|---|---|---|
| 1 | 15 | 15 | `compile: no struct named '…' in '…'` | `std/containers/sequences/array/range_concept_conformance.compile.pass.cpp` | `no struct named '__segmented_iterator_traits' in 'std'` |
| 2 | 11 | 11 | `compile: no member named '…' in '…'` | `std/utilities/utility/pairs/pairs.pair/assign_const_pair_U_V.pass.cpp` | `no member named 'operator=' in 'ArchetypeBases::TrivialValueBase<>'` |
| 3 | 10 | 10 | `compile: cast BaseToDerived is not supported yet` | `std/utilities/utility/mem.res/mem.res.aliases/header_list_synop.pass.cpp` | `cast BaseToDerived is not supported yet` |
| 4 | 9 | 13 | `compile: member initializer '…' does not name a non-static data member or base class` | `std/language.support/support.limits/support.limits.general/syncstream.version.compile.pass.cpp` | `member initializer 'pair' does not name a non-static data member or base class` |
| 5 | 8 | 29 | `compile: static assertion expression is not an integral constant expression` | `std/containers/views/views.span/span.cons/copy.pass.cpp` | `static assertion expression is not an integral constant expression` |
| 6 | 8 | 12 | `compile: called object type '…' is not a function or function pointer` | `std/strings/string.view/string.view.comparison/comparison.pass.cpp` | `called object type '<overloaded function type>' is not a function or function pointer` |
| 7 | 5 | 5 | `compile: a non-constant std::align_val_t allocation argument is not supported yet` | `std/strings/string.view/string.view.cons/from_string.pass.cpp` | `a non-constant std::align_val_t allocation argument is not supported yet` |
| 8 | 4 | 8 | `compile: no matching function for call to '…'` | `std/containers/views/views.span/span.cons/assign.pass.cpp` | `no matching function for call to 'size'` |
| 9 | 3 | 3 | `compile: constexpr if condition is not a constant expression` | `std/containers/views/views.span/span.objectrep/as_bytes.pass.cpp` | `constexpr if condition is not a constant expression` |
| 10 | 3 | 3 | `compile: no matching constructor for initialization of '…'` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `no matching constructor for initialization of 'std::span<const int>'` |
| 11 | 3 | 3 | `compile: use of undeclared identifier '…'` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `use of undeclared identifier '_Complex'` |
| 12 | 2 | 3 | `compile: no viable conversion from '…' to '…'` | `std/containers/sequences/array/array.overview/nttp.equivalence.compile.pass.cpp` | `no viable conversion from 'std::array<int, 3>' to 'std::array'` |
| 13 | 2 | 3 | `compile: static assertion failed due to requirement '…'` | `std/numerics/bit/bit.cast/bit_cast.compile.pass.cpp` | `static assertion failed due to requirement '!...'` |
| 14 | 2 | 2 | `compile: braced-init-list operands of a functional cast is not supported yet` | `std/utilities/utility/pairs/pairs.pair/ctor.brace-init.pass.cpp` | `braced-init-list operands of a functional cast is not supported yet` |
| 15 | 1 | 1 | `compile: compound literals of an array temporary is not supported yet` | `std/containers/sequences/array/array.creation/to_array.pass.cpp` | `compound literals of an array temporary is not supported yet` |
| 16 | 1 | 1 | `compile: implicit assignment of an array of this type is not supported yet` | `std/containers/sequences/array/array.cons/implicit_copy.pass.cpp` | `implicit assignment of an array of this type is not supported yet` |
| 17 | 1 | 1 | `compile: lvalue cast BaseToDerived is not supported yet` | `std/strings/string.view/string.view.nonmem/quoted.pass.cpp` | `lvalue cast BaseToDerived is not supported yet` |
| 18 | 1 | 1 | `compile: static assertion failed due to requirement '…': streaming operator declaration not present` | `std/strings/string.view/string.view.io/stream_insert_decl_present.compile.pass.cpp` | `static assertion failed due to requirement 'HasDecl<std::string_view>::value': streaming operator declaration not present` |
| 19 | 1 | 1 | `link: linker command failed with exit code N (use -v to see invocation)` | `std/utilities/utility/mem.res/mem.res.global/null_memory_resource.pass.cpp` | `linker command failed with exit code 1 (use -v to see invocation)` |
| 20 | 0 | 13 | `compile: excess elements in scalar initializer` | `std/containers/sequences/array/range_concept_conformance.compile.pass.cpp` | `excess elements in scalar initializer` |
| 21 | 0 | 13 | `compile: no class named '…' in '…'` | `std/containers/sequences/array/range_concept_conformance.compile.pass.cpp` | `no class named '__sentinel' in 'take_view<_View>'` |
| 22 | 0 | 13 | `compile: this kind of name is not supported yet` | `std/containers/sequences/array/range_concept_conformance.compile.pass.cpp` | `this kind of name is not supported yet` |
| 23 | 0 | 6 | `compile: static assertion failed` | `std/containers/views/views.span/span.cons/iterator_sentinel.pass.cpp` | `static assertion failed` |
| 24 | 0 | 3 | `compile: a type specifier is required for all declarations` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `a type specifier is required for all declarations` |
| 25 | 0 | 3 | `compile: expected '…'` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `expected '>'` |
| 26 | 0 | 3 | `compile: expected '…' at end of declaration list` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `expected ';' at end of declaration list` |
| 27 | 0 | 3 | `compile: expected parameter declarator` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `expected parameter declarator` |
| 28 | 0 | 3 | `compile: non-static data member cannot be constexpr; did you intend to make it static?` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `non-static data member cannot be constexpr; did you intend to make it static?` |
| 29 | 0 | 3 | `compile: static assertion failed due to requirement '…':` | `std/utilities/utility/pairs/pairs.pair/ctor.brace-init.pass.cpp` | `static assertion failed due to requirement '!can_construct_with_brace_init(0)':` |
| 30 | 0 | 2 | `compile: '…' is a private member of '…' (aka '…')` | `std/containers/views/views.span/span.cons/initializer_list.pass.cpp` | `'__s_' is a private member of 'any' (aka 'std::any')` |
| 31 | 0 | 2 | `compile: no matching constructor for initialization of '…' (aka '…')` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `no matching constructor for initialization of 'std::span<const std::string>' (aka 'std::span<const std::basic_string<char>>')` |
| 32 | 0 | 1 | `compile: no viable conversion from '…' (aka '…') to '…' (aka '…')` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `no viable conversion from 'std::string[2]' (aka 'std::basic_string<char>[2]') to 'std::span<const std::string>' (aka 'std::span<const std::basic_string<char>>')` |
| 33 | 0 | 1 | `compile: static assertion failed due to requirement '…': Types differ unexpectedly` | `std/containers/sequences/array/array.creation/to_array.pass.cpp` | `static assertion failed due to requirement 'test_macros_detail::is_same<<error type>, std::array<int, 2>>::value': Types differ unexpectedly` |

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
