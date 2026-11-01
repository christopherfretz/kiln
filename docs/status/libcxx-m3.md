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

- kiln: debug build of the WP-176 branch (commit in the generated "Run" table below), with main
  `1fafd3f2` (WP-171, WP-173, WP-178, WP-170 merged) merged in.
- Tests and headers: libc++ `llvmorg-19.1.7`. The tests come from the sparse checkout. The headers,
  `libc++.so`, `libc++experimental.a` and `libc++abi` come from the clang 19 packages in the
  container.
- Host: aarch64 Debian 13 container, with `-j 2` and an 8 GiB address-space cap per process.
- Driver: lit-format subset with a static feature set. `.verify.cpp` tests are UNSUPPORTED by the
  driver because kiln has no `-verify`. See `spec/libcxx.md`.

## Reading

**Overall: 519/585 (88.7%)** on the WP-200 branch merged with main `ea42778a` (`ec1d8c34`), up
from 487 on main. The WP-200 branch alone, before the merge, was 513. WP-200 (the
`std/utilities/utility` sweep) takes utility from 104/145 to 136/145 and clears four clusters:
`no member named 'operator='` (11 first), `member initializer … does not name …` (8),
`static_cast from … is not allowed` (6), and braced functional-cast operands (2). The root causes
were these:
- Implicit `operator=` and destructors are now declared before lookup by a qualified name, a member
  access or a using-declaration.
- Mem-initializers can name an injected-class-name or a member typedef, including in instantiated
  constructors.
- A class template specialization is now instantiated before a base/derived check (static_cast,
  pointer conversions, reference binding).
- Downcasts are now code-generated.
- Type-constraints are now checked before the requires-clause (clang's order).
- Functional casts now accept braced-init-list operands.
- A deleted defaulted move member is now ignored by overload resolution (CWG1402).

Before WP-200: 487 on the WP-198 branch (main `db8c01bb`), up from 482. WP-198 clears
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

Per directory, WP-169 → main `e238afa8` → WP-178 → WP-176 → WP-200+main `ea42778a`:

| directory | WP-169 | main | WP-178 | WP-176 | WP-200 |
|---|---:|---:|---:|---:|---:|
| meta | 131/148 | 131/148 | 131/148 | 148/148 | 148/148 |
| utility | 83/145 | 84/145 | 102/145 | 104/145 | 136/145 |
| array | 32/37 | 32/37 | 32/37 | 33/37 | 34/37 |
| string.view | 54/88 | 56/88 | 56/88 | 65/88 | 66/88 |
| span | 18/35 | 18/35 | 20/35 | 20/35 | 21/35 |
| bit | 14/15 | 14/15 | 14/15 | 14/15 | 14/15 |
| support.limits | 83/112 | 83/112 | 83/112 | 93/112 | 95/112 |
| initlist | 5/5 | 5/5 | 5/5 | 5/5 | 5/5 |

**What blocks the rest** (first-error counts; see the generated cluster table):

1. `call to non-static member function without an object argument` (13 first), in
   support.limits (`numeric_limits` members, WP-199).
2. `static assertion expression is not an integral constant expression` (10 first, 26 any), mostly
   the span tests.
3. `called object type '…' is not a function or function pointer` (8 first, 12 any).
4. `static assertion failed` (6 first), `test_iterators.h:1396`
   (`indirectly_writable<ProxyIterator<int*>, Proxy<int>>`), in span and string.view.
5. `no matching function for call to '…'` (4 first, 14 any).
6. `a non-constant std::align_val_t allocation argument is not supported yet` (4 first), in mem.res.

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
| kiln | `kiln version 0.0.1 (C++26 frontend, LLVM IR backend)` at commit `ec1d8c34` |
| libc++ tests | `llvmorg-19.1.7` (`libcxx/test`) |
| libc++ headers | `_LIBCPP_VERSION` 190107 |
| target | `aarch64-unknown-linux-gnu` |
| command | `cargo xtask libcxx --gate m3 -j 2` |
| compile flags | `-std=c++26 -stdlib=libc++ -resource-dir <repo>/resource -pthread -ferror-limit=0 -D_LIBCPP_ENABLE_EXPERIMENTAL -I <llvm-project>/libcxx/test/support <ADDITIONAL_COMPILE_FLAGS> <test>` |
| link flags | `t.o -o t.exe -stdlib=libc++ -pthread -lc++experimental -lc++abi` |
| features | `-fsized-deallocation`, `buildhost=linux`, `c++26`, `c++experimental`, `can-create-symlinks`, `has-64-bit-atomics`, `has-fconstexpr-steps`, `has-unix-headers`, `kiln`, `large_tests`, `libcpp-abi-version=1`, `libcpp-has-no-availability-markup`, `libcpp-has-thread-api-pthread`, `linux`, `long_tests`, `optimization=none`, `stdlib=libc++`, `stdlib=llvm-libc++`, `target=aarch64-unknown-linux-gnu` |
| limits | 8192 MiB address space per process (kiln: plus its stack reservation); compile/link timeout 120 s; run timeout 30 s; -j 2 |
| wall time | 475.2 s |
| peak RSS | 84 MiB (largest single process, `std/utilities/utility/mem.res/mem.poly.allocator.class/mem.poly.allocator.mem/construct_types.pass.cpp`; gnu-time) |

### Results by directory

Pass rate is PASS / (total − UNSUPPORTED). "driver" counts the UNSUPPORTED tests the driver cannot run (for example `.verify.cpp`), as opposed to those the configuration excludes (`UNSUPPORTED: c++03`).

| directory | total | pass | fail | xfail | xpass | unsupported (driver) | killed | timeout | error | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `std/utilities/meta` | 155 | 148 | 0 | 0 | 0 | 7 (6) | 0 | 0 | 0 | 100.0% |
| `std/utilities/utility` | 168 | 136 | 9 | 0 | 0 | 23 (19) | 0 | 0 | 0 | 93.8% |
| `std/containers/sequences/array` | 47 | 34 | 3 | 0 | 0 | 10 (10) | 0 | 0 | 0 | 91.9% |
| `std/strings/string.view` | 93 | 66 | 22 | 0 | 0 | 5 (5) | 0 | 0 | 0 | 75.0% |
| `std/containers/views/views.span` | 45 | 21 | 14 | 0 | 0 | 10 (9) | 0 | 0 | 0 | 60.0% |
| `std/numerics/bit` | 17 | 14 | 1 | 0 | 0 | 2 (2) | 0 | 0 | 0 | 93.3% |
| `std/language.support/support.limits` | 112 | 95 | 17 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 84.8% |
| `std/language.support/support.initlist` | 5 | 5 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 100.0% |
| **total** | 642 | 519 | 66 | 0 | 0 | 57 (51) | 0 | 0 | 0 | 88.7% |

### Failures by stage

| directory | compile | link | run | expected-fail-but-succeeded |
|---|---:|---:|---:|---:|
| `std/utilities/meta` | 0 | 0 | 0 | 0 |
| `std/utilities/utility` | 8 | 1 | 0 | 0 |
| `std/containers/sequences/array` | 3 | 0 | 0 | 0 |
| `std/strings/string.view` | 22 | 0 | 0 | 0 |
| `std/containers/views/views.span` | 14 | 0 | 0 | 0 |
| `std/numerics/bit` | 1 | 0 | 0 | 0 |
| `std/language.support/support.limits` | 17 | 0 | 0 | 0 |
| `std/language.support/support.initlist` | 0 | 0 | 0 | 0 |

### Dominant first errors per directory

The three most common first-error clusters among each directory's FAIL tests, with their test counts.

| directory | clusters |
|---|---|
| `std/utilities/meta` | — |
| `std/utilities/utility` | `compile: a non-constant std::align_val_t allocation argument is not supported yet` (2); `compile: static assertion expression is not an integral constant expression` (2); `compile: call to non-static member function without an object argument` (1) |
| `std/containers/sequences/array` | `compile: compound literals of an array temporary is not supported yet` (1); `compile: implicit assignment of an array of this type is not supported yet` (1); `compile: no viable conversion from '…' to '…'` (1) |
| `std/strings/string.view` | `compile: called object type '…' is not a function or function pointer` (8); `compile: static assertion expression is not an integral constant expression` (5); `compile: static assertion failed` (4) |
| `std/containers/views/views.span` | `compile: static assertion expression is not an integral constant expression` (3); `compile: no matching function for call to '…'` (3); `compile: no matching constructor for initialization of '…'` (3) |
| `std/numerics/bit` | `compile: static assertion failed due to requirement '…'` (1) |
| `std/language.support/support.limits` | `compile: call to non-static member function without an object argument` (10); `compile: constraints not satisfied for class template '…' [with _OutIt = back_insert_iterator<__format::__output_buffer<char>>, _CharT = char]` (3); `compile: invalid operands to binary expression ('…' (aka '…') and '…')` (1) |
| `std/language.support/support.initlist` | — |

### Error clusters

34 clusters over the FAIL tests (and compiler crashes). A cluster is the failing step's `error:` message with the location dropped, quoted names replaced by `'…'` and numbers by `N`. "first" counts tests whose first error is the cluster, "any" tests that report it anywhere. Top 34:

| # | first | any | cluster | example test | example message |
|---:|---:|---:|---|---|---|
| 1 | 13 | 14 | `compile: call to non-static member function without an object argument` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `call to non-static member function without an object argument` |
| 2 | 10 | 26 | `compile: static assertion expression is not an integral constant expression` | `std/containers/views/views.span/span.cons/copy.pass.cpp` | `static assertion expression is not an integral constant expression` |
| 3 | 8 | 12 | `compile: called object type '…' is not a function or function pointer` | `std/strings/string.view/string.view.comparison/comparison.pass.cpp` | `called object type '<overloaded function type>' is not a function or function pointer` |
| 4 | 6 | 7 | `compile: static assertion failed` | `std/containers/views/views.span/span.cons/iterator_sentinel.pass.cpp` | `static assertion failed` |
| 5 | 4 | 14 | `compile: no matching function for call to '…'` | `std/containers/views/views.span/span.cons/assign.pass.cpp` | `no matching function for call to 'size'` |
| 6 | 4 | 4 | `compile: a non-constant std::align_val_t allocation argument is not supported yet` | `std/strings/string.view/string.view.cons/from_string.pass.cpp` | `a non-constant std::align_val_t allocation argument is not supported yet` |
| 7 | 3 | 6 | `compile: constraints not satisfied for class template '…' [with _OutIt = back_insert_iterator<__format::__output_buffer<char>>, _CharT = char]` | `std/language.support/support.limits/support.limits.general/chrono.version.compile.pass.cpp` | `constraints not satisfied for class template 'std::basic_format_context' [with _OutIt = back_insert_iterator<__format::__output_buffer<char>>, _CharT = char]` |
| 8 | 3 | 3 | `compile: constexpr if condition is not a constant expression` | `std/containers/views/views.span/span.objectrep/as_bytes.pass.cpp` | `constexpr if condition is not a constant expression` |
| 9 | 3 | 3 | `compile: no matching constructor for initialization of '…'` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `no matching constructor for initialization of 'std::span<const int>'` |
| 10 | 2 | 6 | `compile: static assertion failed due to requirement '…'` | `std/numerics/bit/bit.cast/bit_cast.compile.pass.cpp` | `static assertion failed due to requirement '!...'` |
| 11 | 2 | 3 | `compile: no viable conversion from '…' to '…'` | `std/containers/sequences/array/array.overview/nttp.equivalence.compile.pass.cpp` | `no viable conversion from 'std::array<int, 3>' to 'std::array'` |
| 12 | 1 | 7 | `compile: invalid operands to binary expression ('…' (aka '…') and '…')` | `std/language.support/support.limits/support.limits.general/charconv.version.compile.pass.cpp` | `invalid operands to binary expression ('chars_format' (aka 'std::chars_format') and 'chars_format')` |
| 13 | 1 | 1 | `compile: call to deleted function call operator in type '…' (aka '…')` | `std/containers/views/views.span/range_concept_conformance.compile.pass.cpp` | `call to deleted function call operator in type 'const __begin::__fn' (aka 'const std::ranges::__begin::__fn')` |
| 14 | 1 | 1 | `compile: compound literals of an array temporary is not supported yet` | `std/containers/sequences/array/array.creation/to_array.pass.cpp` | `compound literals of an array temporary is not supported yet` |
| 15 | 1 | 1 | `compile: expected unqualified-id` | `std/language.support/support.limits/support.limits.general/random.version.compile.pass.cpp` | `expected unqualified-id` |
| 16 | 1 | 1 | `compile: implicit assignment of an array of this type is not supported yet` | `std/containers/sequences/array/array.cons/implicit_copy.pass.cpp` | `implicit assignment of an array of this type is not supported yet` |
| 17 | 1 | 1 | `compile: redefinition of '…'` | `std/language.support/support.limits/support.limits.general/expected.version.compile.pass.cpp` | `redefinition of 'operator=='` |
| 18 | 1 | 1 | `compile: static assertion failed due to requirement '…': streaming operator declaration not present` | `std/strings/string.view/string.view.io/stream_insert_decl_present.compile.pass.cpp` | `static assertion failed due to requirement 'HasDecl<std::string_view>::value': streaming operator declaration not present` |
| 19 | 1 | 1 | `link: linker command failed with exit code N (use -v to see invocation)` | `std/utilities/utility/mem.res/mem.res.global/null_memory_resource.pass.cpp` | `linker command failed with exit code 1 (use -v to see invocation)` |
| 20 | 0 | 6 | `compile: constraints not satisfied for class template '…' [with _OutIt = back_insert_iterator<__format::__output_buffer<wchar_t>>, _CharT = wchar_t]` | `std/language.support/support.limits/support.limits.general/chrono.version.compile.pass.cpp` | `constraints not satisfied for class template 'std::basic_format_context' [with _OutIt = back_insert_iterator<__format::__output_buffer<wchar_t>>, _CharT = wchar…` |
| 21 | 0 | 6 | `compile: pack expansion does not contain any unexpanded parameter packs` | `std/language.support/support.limits/support.limits.general/chrono.version.compile.pass.cpp` | `pack expansion does not contain any unexpanded parameter packs` |
| 22 | 0 | 3 | `compile: a type specifier is required for all declarations` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `a type specifier is required for all declarations` |
| 23 | 0 | 3 | `compile: expected '…'` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `expected '>'` |
| 24 | 0 | 3 | `compile: expected '…' at end of declaration list` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `expected ';' at end of declaration list` |
| 25 | 0 | 3 | `compile: expected parameter declarator` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `expected parameter declarator` |
| 26 | 0 | 3 | `compile: non-static data member cannot be constexpr; did you intend to make it static?` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `non-static data member cannot be constexpr; did you intend to make it static?` |
| 27 | 0 | 3 | `compile: use of undeclared identifier '…'` | `std/language.support/support.limits/limits/is_specialized.pass.cpp` | `use of undeclared identifier '_Complex'` |
| 28 | 0 | 2 | `compile: no matching constructor for initialization of '…' (aka '…')` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `no matching constructor for initialization of 'std::span<const std::string>' (aka 'std::span<const std::basic_string<char>>')` |
| 29 | 0 | 1 | `compile: expected a type` | `std/language.support/support.limits/support.limits.general/random.version.compile.pass.cpp` | `expected a type` |
| 30 | 0 | 1 | `compile: invalid operands to binary expression ('…' and '…' (aka '…'))` | `std/language.support/support.limits/support.limits.general/chrono.version.compile.pass.cpp` | `invalid operands to binary expression ('int' and '__flags' (aka 'std::__format_spec::__flags'))` |
| 31 | 0 | 1 | `compile: no matching function for call to object of type '…' (aka '…')` | `std/language.support/support.limits/support.limits.general/chrono.version.compile.pass.cpp` | `no matching function for call to object of type 'const __lower_bound::__fn' (aka 'const std::ranges::__lower_bound::__fn')` |
| 32 | 0 | 1 | `compile: no viable conversion from '…' (aka '…') to '…' (aka '…')` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `no viable conversion from 'std::string[2]' (aka 'std::basic_string<char>[2]') to 'std::span<const std::string>' (aka 'std::span<const std::basic_string<char>>')` |
| 33 | 0 | 1 | `compile: static assertion failed due to requirement '…': Types differ unexpectedly` | `std/containers/sequences/array/array.creation/to_array.pass.cpp` | `static assertion failed due to requirement 'test_macros_detail::is_same<<error type>, std::array<int, 2>>::value': Types differ unexpectedly` |
| 34 | 0 | 1 | `compile: unknown type name '…'` | `std/language.support/support.limits/support.limits.general/random.version.compile.pass.cpp` | `unknown type name '__calc_type'` |

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
