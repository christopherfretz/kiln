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

- kiln: debug build of the WP-141 branch (commit in the generated "Run" table below), which
  includes main at `7b55934`.
- Tests and headers: libc++ `llvmorg-19.1.7`. The tests come from the sparse checkout. The headers,
  `libc++.so`, `libc++experimental.a` and `libc++abi` come from the clang 19 packages in the
  container.
- Host: aarch64 Debian 13 container, with `-j 2` and an 8 GiB address-space cap per process.
- Driver: lit-format subset with a static feature set. `.verify.cpp` tests are UNSUPPORTED by the
  driver because kiln has no `-verify`. See `spec/libcxx.md`.

## Reading

**Overall: 254/585 (43.4%)**, up from 251 after WP-140 and 20 at the WP-081 baseline. There are no
crashes, and no KILLED, TIMEOUT or hang results. Nearly every failure is at the compile stage; 3 tests
reach link and fail there. Peak RSS was 72 MiB and wall time about 4.5 minutes.

WP-141 removed both panics, so every test now runs to a diagnosed compile error. The
`decl.rs:1907` panic ("decl has no Function payload") was hit by 214 tests through `<string>`. The
`FnSpec` panic was hit by `is_abstract.pass.cpp`. kiln now gets through `<string>` with 87 errors and
no crash (`tests/stress/no-crash-libcxx-headers.cpp`). Most of the failures it was masking stay
failures: the next error is in `<tuple>`, which every `<string>` user includes.

Per directory, WP-140 → WP-141:

| directory | WP-140 | WP-141 |
|---|---:|---:|
| meta | 112 | 113/148 |
| utility | 25 | 25/145 |
| array | 23 | 23/37 |
| string.view | 19 | 21/88 |
| span | 3 | 3/35 |
| bit | 12 | 12/15 |
| support.limits | 52 | 52/112 |
| initlist | 5 | 5/5 |

**What blocks the rest.** One error dominates:

1. **`expected '>'` at `<tuple>:575:101` (171 first, 257 any).** The default template argument is
   `__enable_if_t<_And<_BoolConstant<sizeof...(_Tp) >= 1>, ...>::value, int>`. The parser's
   template-argument lookahead (`scan_template_args`, `kiln_parse/src/parser.rs`) treats `>=` as
   closing `_BoolConstant<`. clang parses it as a relational operator, because the argument is an
   expression. So `_And<...>::value` is not seen as a qualified name. Repro:
   `template <bool B> struct BC { static constexpr bool value = B; };`
   `template <class... T> struct S { static constexpr bool a = BC<sizeof...(T) >= 1>::value; };`
   Before WP-141 the first error was one line earlier, at `<tuple>:471`
   (`Leaf<Is, Ts>(u)...`, a template-id mem-initializer), which WP-141 fixed.

Behind it:

2. `'…' does not refer to a value` (45 first, 153 any). For example `__atomic/memory_order.h:40`,
   `underlying_type<__legacy_memory_order>::type` in an expression.
3. `function '…' with deduced return type cannot be used before it is defined` (22 first, 63 any):
   `__get_iter_concept`.
4. `use of undeclared identifier` (19 first, 196 any). For example `__builtin_labs` and
   `__builtin_fabsf`.
5. `using declaration refers into '__base::', which is not a base class` (12 first, 120 any):
   `<optional>:351`.

Then come the NYI type traits (9 first, 77 any), the NYI friend class template specialization
(8 first, 42 any) and wrong `static_assert` values (8 first). Among the clusters that are rarely
first: `excess elements in scalar initializer` (246 any), `constructor cannot be redeclared`
(170 any), `no member named` (152 any, mostly anonymous member structs such as the
`basic_string::__short`/`__long` layout) and `call to member function is ambiguous` (131 any).

**Caveats.**
- The driver only checks that a `.compile.fail.cpp` test fails to compile without a crash, not
  that it fails with the diagnostic it targets.
- XFAILs written against compiler identity (`clang-*`, `gcc-*`) evaluate to false, because the
  feature set names only `kiln`.

**Candidate work packages, in order of gate impact:**
1. `>=` in a template argument in the parser's lookahead (`<tuple>:575`, 171 tests).
2. `'…' does not refer to a value` for a member type in an expression (`<atomic>`).
3. Deduced return type used before it is defined (`__get_iter_concept`).
4. Undeclared builtins (`__builtin_labs`, `__builtin_fabsf`, ...).
5. Using-declaration into a dependent base (`<optional>`).
6. `excess elements in scalar initializer` (246 any).
7. Anonymous member classes in class templates (`basic_string::__short`).
8. Explicit instantiation of a static data member (`basic_string::npos`).
9. Type-trait builtins such as `__is_trivially_assignable`.
10. `-verify` (51 driver-unsupported tests).

<!-- libcxx:begin -->
<!-- generated by `cargo xtask libcxx --report`; edits inside the markers are overwritten -->

### Run

| | |
|---|---|
| kiln | `kiln version 0.0.1 (C++26 frontend, LLVM IR backend)` at commit `0a9ac6b` |
| libc++ tests | `llvmorg-19.1.7` (`libcxx/test`) |
| libc++ headers | `_LIBCPP_VERSION` 190107 |
| target | `aarch64-unknown-linux-gnu` |
| command | `cargo xtask libcxx --gate m3 -j 2` |
| compile flags | `-std=c++26 -stdlib=libc++ -resource-dir <repo>/resource -pthread -ferror-limit=0 -D_LIBCPP_ENABLE_EXPERIMENTAL -I <llvm-project>/libcxx/test/support <ADDITIONAL_COMPILE_FLAGS> <test>` |
| link flags | `t.o -o t.exe -stdlib=libc++ -pthread -lc++experimental -lc++abi` |
| features | `-fsized-deallocation`, `buildhost=linux`, `c++26`, `c++experimental`, `can-create-symlinks`, `has-64-bit-atomics`, `has-fconstexpr-steps`, `has-unix-headers`, `kiln`, `large_tests`, `libcpp-abi-version=1`, `libcpp-has-no-availability-markup`, `libcpp-has-thread-api-pthread`, `linux`, `long_tests`, `optimization=none`, `stdlib=libc++`, `stdlib=llvm-libc++`, `target=aarch64-unknown-linux-gnu` |
| limits | 8192 MiB address space per process (kiln: plus its stack reservation); compile/link timeout 120 s; run timeout 30 s; -j 2 |
| wall time | 273.4 s |
| peak RSS | 72 MiB (largest single process, `std/language.support/support.limits/limits/numeric.limits.members/const_data_members.pass.cpp`; gnu-time) |

### Results by directory

Pass rate is PASS / (total − UNSUPPORTED). "driver" counts the UNSUPPORTED tests the driver cannot run (for example `.verify.cpp`), as opposed to those the configuration excludes (`UNSUPPORTED: c++03`).

| directory | total | pass | fail | xfail | xpass | unsupported (driver) | killed | timeout | error | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `std/utilities/meta` | 155 | 113 | 35 | 0 | 0 | 7 (6) | 0 | 0 | 0 | 76.4% |
| `std/utilities/utility` | 168 | 25 | 120 | 0 | 0 | 23 (19) | 0 | 0 | 0 | 17.2% |
| `std/containers/sequences/array` | 47 | 23 | 14 | 0 | 0 | 10 (10) | 0 | 0 | 0 | 62.2% |
| `std/strings/string.view` | 93 | 21 | 67 | 0 | 0 | 5 (5) | 0 | 0 | 0 | 23.9% |
| `std/containers/views/views.span` | 45 | 3 | 32 | 0 | 0 | 10 (9) | 0 | 0 | 0 | 8.6% |
| `std/numerics/bit` | 17 | 12 | 3 | 0 | 0 | 2 (2) | 0 | 0 | 0 | 80.0% |
| `std/language.support/support.limits` | 112 | 52 | 60 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 46.4% |
| `std/language.support/support.initlist` | 5 | 5 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 100.0% |
| **total** | 642 | 254 | 331 | 0 | 0 | 57 (51) | 0 | 0 | 0 | 43.4% |

### Failures by stage

| directory | compile | link | run | expected-fail-but-succeeded |
|---|---:|---:|---:|---:|
| `std/utilities/meta` | 35 | 0 | 0 | 0 |
| `std/utilities/utility` | 120 | 0 | 0 | 0 |
| `std/containers/sequences/array` | 14 | 0 | 0 | 0 |
| `std/strings/string.view` | 65 | 2 | 0 | 0 |
| `std/containers/views/views.span` | 32 | 0 | 0 | 0 |
| `std/numerics/bit` | 3 | 0 | 0 | 0 |
| `std/language.support/support.limits` | 59 | 1 | 0 | 0 |
| `std/language.support/support.initlist` | 0 | 0 | 0 | 0 |

### Dominant first errors per directory

The three most common first-error clusters among each directory's FAIL tests, with their test counts.

| directory | clusters |
|---|---|
| `std/utilities/meta` | `compile: kiln: not yet implemented: type trait '…'` (7); `compile: static assertion failed due to requirement '…':` (7); `compile: expected '…'` (6) |
| `std/utilities/utility` | `compile: expected '…'` (77); `compile: '…' does not refer to a value` (20); `compile: constructor cannot be redeclared` (4) |
| `std/containers/sequences/array` | `compile: expected '…'` (5); `compile: '…' does not refer to a value` (3); `compile: function '…' with deduced return type cannot be used before it is defined` (3) |
| `std/strings/string.view` | `compile: expected '…'` (49); `compile: function '…' with deduced return type cannot be used before it is defined` (7); `compile: excess elements in scalar initializer` (4) |
| `std/containers/views/views.span` | `compile: expected '…'` (21); `compile: function '…' with deduced return type cannot be used before it is defined` (6); `compile: using declaration refers into '…', which is not a base class of '…'` (3) |
| `std/numerics/bit` | `compile: chosen constructor is explicit in copy-initialization` (1); `compile: static assertion failed due to requirement '…'` (1); `compile: use of undeclared identifier '…'` (1) |
| `std/language.support/support.limits` | `compile: '…' does not refer to a value` (20); `compile: expected '…'` (13); `compile: use of undeclared identifier '…'` (12) |
| `std/language.support/support.initlist` | — |

### Error clusters

131 clusters over the FAIL tests (and compiler crashes). A cluster is the failing step's `error:` message with the location dropped, quoted names replaced by `'…'` and numbers by `N`. "first" counts tests whose first error is the cluster, "any" tests that report it anywhere. Top 40:

| # | first | any | cluster | example test | example message |
|---:|---:|---:|---|---|---|
| 1 | 171 | 257 | `compile: expected '…'` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `expected '>'` |
| 2 | 45 | 153 | `compile: '…' does not refer to a value` | `std/containers/sequences/array/array.tuple/get_const_rv.pass.cpp` | `'type' does not refer to a value` |
| 3 | 22 | 63 | `compile: function '…' with deduced return type cannot be used before it is defined` | `std/containers/sequences/array/iterator_concept_conformance.compile.pass.cpp` | `function '__get_iter_concept' with deduced return type cannot be used before it is defined` |
| 4 | 19 | 196 | `compile: use of undeclared identifier '…'` | `std/containers/views/views.span/span.cons/iterator_sentinel.pass.cpp` | `use of undeclared identifier '__builtin_labs'` |
| 5 | 12 | 120 | `compile: using declaration refers into '…', which is not a base class of '…'` | `std/containers/views/views.span/span.sub/first.pass.cpp` | `using declaration refers into '__base::', which is not a base class of '__optional_storage_base'` |
| 6 | 9 | 77 | `compile: kiln: not yet implemented: type trait '…'` | `std/language.support/support.limits/support.limits.general/thread.version.compile.pass.cpp` | `kiln: not yet implemented: type trait '__is_trivially_assignable'` |
| 7 | 8 | 42 | `compile: kiln: not yet implemented: friend class template specialization` | `std/language.support/support.limits/support.limits.general/bitset.version.compile.pass.cpp` | `kiln: not yet implemented: friend class template specialization` |
| 8 | 8 | 18 | `compile: static assertion failed due to requirement '…':` | `std/containers/sequences/array/size_and_alignment.compile.pass.cpp` | `static assertion failed due to requirement 'alignof(std::array<short, 0>) == alignof(short)':` |
| 9 | 5 | 246 | `compile: excess elements in scalar initializer` | `std/language.support/support.limits/support.limits.general/format.version.compile.pass.cpp` | `excess elements in scalar initializer` |
| 10 | 5 | 170 | `compile: constructor cannot be redeclared` | `std/language.support/support.limits/support.limits.general/mdspan.version.compile.pass.cpp` | `constructor cannot be redeclared` |
| 11 | 4 | 43 | `compile: static assertion failed due to requirement '…'` | `std/numerics/bit/bit.cast/bit_cast.compile.pass.cpp` | `static assertion failed due to requirement '!...'` |
| 12 | 4 | 4 | `compile: array has incomplete element type '…'` | `std/utilities/meta/meta.unary/meta.unary.cat/array.pass.cpp` | `array has incomplete element type 'Incomplete'` |
| 13 | 3 | 14 | `compile: no viable conversion from '…' (aka '…') to '…' (aka '…')` | `std/containers/sequences/array/array.overview/nttp.equivalence.compile.pass.cpp` | `no viable conversion from 'std::array<int, 3>' (aka 'std::__1::array<int, 3>') to 'std::array' (aka 'std::__1::array')` |
| 14 | 3 | 3 | `link: linker command failed with exit code N (use -v to see invocation)` | `std/language.support/support.limits/limits/numeric.limits.members/const_data_members.pass.cpp` | `linker command failed with exit code 1 (use -v to see invocation)` |
| 15 | 2 | 152 | `compile: no member named '…' in '…'` | `std/containers/views/views.span/span.cons/initializer_list.pass.cpp` | `no member named '_SmallHandler' in '__any_imp'` |
| 16 | 2 | 6 | `compile: chosen constructor is explicit in copy-initialization` | `std/numerics/bit/byteswap.pass.cpp` | `chosen constructor is explicit in copy-initialization` |
| 17 | 1 | 131 | `compile: call to member function '…' is ambiguous` | `std/utilities/meta/meta.rel/is_same.pass.cpp` | `call to member function 'fn' is ambiguous` |
| 18 | 1 | 26 | `compile: redefinition of '…'` | `std/language.support/support.limits/support.limits.general/expected.version.compile.pass.cpp` | `redefinition of 'operator=='` |
| 19 | 1 | 8 | `compile: ambiguous partial specializations of '…'` | `std/utilities/meta/meta.logical/conjunction.compile.pass.cpp` | `ambiguous partial specializations of 'conjunction<std::true_type>'` |
| 20 | 1 | 8 | `compile: invalid operands to binary expression ('…' (aka '…') and '…' (aka '…'))` | `std/language.support/support.limits/support.limits.general/charconv.version.compile.pass.cpp` | `invalid operands to binary expression ('chars_format' (aka 'std::__1::chars_format') and 'chars_format' (aka 'std::__1::chars_format'))` |
| 21 | 1 | 5 | `compile: static assertion failed due to requirement '…': Types differ unexpectedly` | `std/utilities/meta/meta.trans/meta.trans.other/decay.pass.cpp` | `static assertion failed due to requirement 'test_macros_detail::is_same<int (int) const, std::decay<int (int) const>::type>::value': Types differ unexpectedly` |
| 22 | 1 | 2 | `compile: braced-init-list operands of a functional cast is not supported yet` | `std/utilities/utility/pairs/pairs.pair/ctor.brace-init.pass.cpp` | `braced-init-list operands of a functional cast is not supported yet` |
| 23 | 1 | 1 | `compile: IR lowering failed: IrError { kind: BadCall, message: "call with N arguments to a function taking N" }` | `std/language.support/support.limits/limits/numeric.limits/default.pass.cpp` | `IR lowering failed: IrError { kind: BadCall, message: "call with 1 arguments to a function taking 2" }` |
| 24 | 1 | 1 | `compile: implicit assignment of an array of this type is not supported yet` | `std/containers/sequences/array/array.cons/implicit_copy.pass.cpp` | `implicit assignment of an array of this type is not supported yet` |
| 25 | 1 | 1 | `compile: static assertion failed due to requirement '…': streaming operator declaration not present` | `std/strings/string.view/string.view.io/stream_insert_decl_present.compile.pass.cpp` | `static assertion failed due to requirement 'HasDecl<std::string_view>::value': streaming operator declaration not present` |
| 26 | 0 | 255 | `compile: expected member name or '…' after declaration specifiers` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `expected member name or ';' after declaration specifiers` |
| 27 | 0 | 249 | `compile: expected '…' after base class list` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `expected '{' after base class list` |
| 28 | 0 | 249 | `compile: kiln: not yet implemented: partially substituted pack expansion` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `kiln: not yet implemented: partially substituted pack expansion` |
| 29 | 0 | 249 | `compile: pack expansion does not contain any unexpanded parameter packs` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `pack expansion does not contain any unexpanded parameter packs` |
| 30 | 0 | 193 | `compile: kiln: not yet implemented: anonymous member classes of class templates` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `kiln: not yet implemented: anonymous member classes of class templates` |
| 31 | 0 | 191 | `compile: kiln: not yet implemented: explicit instantiation of a variable` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `kiln: not yet implemented: explicit instantiation of a variable` |
| 32 | 0 | 164 | `compile: no member named '…' in '…' (aka '…')` | `std/containers/views/views.span/span.iterators/iterator.pass.cpp` | `no member named '__size_' in '__short' (aka 'std::__1::basic_string<char>::__short')` |
| 33 | 0 | 164 | `compile: out-of-line definition of '…' does not match any declaration in '…'` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `out-of-line definition of '__clone' does not match any declaration in '<error type>'` |
| 34 | 0 | 157 | `compile: static assertion failed due to requirement '…': __short has an unexpected size.` | `std/containers/views/views.span/span.iterators/iterator.pass.cpp` | `static assertion failed due to requirement 'sizeof(std::basic_string<char>::__short) == (sizeof(char) * (__min_cap + 1))': __short has an unexpected size.` |
| 35 | 0 | 144 | `compile: extraneous template parameter list in template specialization` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `extraneous template parameter list in template specialization` |
| 36 | 0 | 138 | `compile: constructors is not supported yet` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `constructors is not supported yet` |
| 37 | 0 | 134 | `compile: a type specifier is required for all declarations` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `a type specifier is required for all declarations` |
| 38 | 0 | 134 | `compile: function qualifiers on a non-member function is not supported yet` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `function qualifiers on a non-member function is not supported yet` |
| 39 | 0 | 134 | `compile: member reference base type '…' (aka '…') is not a structure or union` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `member reference base type 'function' (aka 'std::__1::function') is not a structure or union` |
| 40 | 0 | 133 | `compile: invalid use of '…' outside of a non-static member function` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `invalid use of 'this' outside of a non-static member function` |

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
