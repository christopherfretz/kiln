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

- kiln: debug build of the WP-142 branch (commit in the generated "Run" table below), which
  includes main at `770fda2` (WP-114 merged).
- Tests and headers: libc++ `llvmorg-19.1.7`. The tests come from the sparse checkout. The headers,
  `libc++.so`, `libc++experimental.a` and `libc++abi` come from the clang 19 packages in the
  container.
- Host: aarch64 Debian 13 container, with `-j 2` and an 8 GiB address-space cap per process.
- Driver: lit-format subset with a static feature set. `.verify.cpp` tests are UNSUPPORTED by the
  driver because kiln has no `-verify`. See `spec/libcxx.md`.

## Reading

**Overall: 307/585 (52.5%)**, up from 254 after WP-141 (251 after WP-140, 20 at the WP-081
baseline). There are no crashes, and no KILLED, TIMEOUT or hang results. Every failure is at the
compile stage. Peak RSS was 72 MiB and wall time about 5 minutes.

WP-142 cleared the first five clusters of the WP-141 reading (`docs/status/wp-142-progress.md`):

- `>=` in template arguments, and `>` inside brackets in them;
- names scoped by specializations;
- deduced member return types;
- unnamed member classes;
- dependent paren and brace initializers;
- explicit instantiation of variables;
- partial specialization out-of-line members;
- class templates first declared by a friend.

`<string>` now compiles with no errors (87 at WP-141), `<memory>` with 6 and `<optional>` with 3, all
WP-138. Most tests that pass the `<tuple>` and `<string>` stage now fail on a later, smaller cluster.

Per directory, WP-141 → WP-142:

| directory | WP-141 | WP-142 |
|---|---:|---:|
| meta | 113 | 115/148 |
| utility | 25 | 28/145 |
| array | 23 | 25/37 |
| string.view | 21 | 49/88 |
| span | 3 | 15/35 |
| bit | 12 | 12/15 |
| support.limits | 52 | 58/112 |
| initlist | 5 | 5/5 |

**What blocks the rest.** One error dominates:

1. **`constructor cannot be redeclared` (124 first, 170 any)**, which is WP-138. Templates that
   differ only in an `enable_if` default non-type template argument are treated as the same
   declaration. For example `<optional>:685/695/705`, `<tuple>`, `shared_ptr.h:261`, and
   `vector:1218` (as "redefinition of"). Members overloaded the same way become ambiguous
   (`shared_ptr.h:286` `__on_zero_shared_impl`). Repro:
   `template <class A> struct S { template <class X = A, EI_t<sizeof(typename X::value_type) == 4, int> = 0> explicit S(A);`
   followed by the same declaration with `!= 4`.

Behind it:

2. `this kind of name in a using-declarator is not supported yet` (28 first, 40 any). This is
   `using B::operator=;`, which every `<variant>` user reaches through `_LIBCPP_VARIANT_DESTRUCTOR`.
   Repro: `struct B { B& operator=(int); }; struct D : B { using B::operator=; };`
3. `use of undeclared identifier` (19 first, 171 any). These are the builtins `__builtin_labs`,
   `__builtin_fabsf`, `__builtin_va_start`, `__builtin_trunc` and `__builtin_fpclassify` (WP-144).
4. NYI type traits (9 first, 77 any), for example `__is_trivially_assignable`.
5. NYI friend class template specialization (9 first, 42 any; WP-136).
6. `a destructor template-id is not supported yet` (8 first).

Among the clusters that are rarely first:

- `call to member function is ambiguous` (131 any; mostly item 1);
- local using-directives in templates (108 any; `convert_to_timespec.h:30`, every `<chrono>`
  user);
- `no member named` (92 any);
- `expected member name or ';'` (83 any). This is `_Atomic(T)` as a type specifier,
  `cxx_atomic_impl.h:274`;
- explicit object parameters (58 any; `format_arg.h:292`);
- `static assertion expression is not an integral constant expression` (53 any).

**Caveats.**
- The driver only checks that a `.compile.fail.cpp` test fails to compile without a crash, not
  that it fails with the diagnostic it targets.
- XFAILs written against compiler identity (`clang-*`, `gcc-*`) evaluate to false, because the
  feature set names only `kiln`.

**Candidate work packages, in order of gate impact:**
1. Templates that differ in a default template argument's `enable_if` are distinct (WP-138; 124
   first).
2. `using X::operator=;` and other operator-function-ids in using-declarators (`<variant>`, 28
   first).
3. Libm and atomic builtins (WP-144; 19 first, 171 any).
4. Type-trait builtins such as `__is_trivially_assignable` (9 first, 77 any).
5. Friend class template specializations (WP-136; 9 first, 42 any).
6. Destructor template-ids (`p->~X<T>()`, 8 first).
7. Local using-directives in templates (108 any).
8. `_Atomic(T)` type specifier (83 any).
9. Explicit object parameters (58 any).
10. Braced default arguments (`int g(P p = {})`, in `<vector>` and `<chrono>`).
11. `-verify` (51 driver-unsupported tests; WP-143).

<!-- libcxx:begin -->
<!-- generated by `cargo xtask libcxx --report`; edits inside the markers are overwritten -->

### Run

| | |
|---|---|
| kiln | `kiln version 0.0.1 (C++26 frontend, LLVM IR backend)` at commit `def1246` |
| libc++ tests | `llvmorg-19.1.7` (`libcxx/test`) |
| libc++ headers | `_LIBCPP_VERSION` 190107 |
| target | `aarch64-unknown-linux-gnu` |
| command | `cargo xtask libcxx --gate m3 -j 2` |
| compile flags | `-std=c++26 -stdlib=libc++ -resource-dir <repo>/resource -pthread -ferror-limit=0 -D_LIBCPP_ENABLE_EXPERIMENTAL -I <llvm-project>/libcxx/test/support <ADDITIONAL_COMPILE_FLAGS> <test>` |
| link flags | `t.o -o t.exe -stdlib=libc++ -pthread -lc++experimental -lc++abi` |
| features | `-fsized-deallocation`, `buildhost=linux`, `c++26`, `c++experimental`, `can-create-symlinks`, `has-64-bit-atomics`, `has-fconstexpr-steps`, `has-unix-headers`, `kiln`, `large_tests`, `libcpp-abi-version=1`, `libcpp-has-no-availability-markup`, `libcpp-has-thread-api-pthread`, `linux`, `long_tests`, `optimization=none`, `stdlib=libc++`, `stdlib=llvm-libc++`, `target=aarch64-unknown-linux-gnu` |
| limits | 8192 MiB address space per process (kiln: plus its stack reservation); compile/link timeout 120 s; run timeout 30 s; -j 2 |
| wall time | 315.9 s |
| peak RSS | 72 MiB (largest single process, `std/utilities/meta/meta.unary/meta.unary.cat/is_lvalue_reference.pass.cpp`; gnu-time) |

### Results by directory

Pass rate is PASS / (total − UNSUPPORTED). "driver" counts the UNSUPPORTED tests the driver cannot run (for example `.verify.cpp`), as opposed to those the configuration excludes (`UNSUPPORTED: c++03`).

| directory | total | pass | fail | xfail | xpass | unsupported (driver) | killed | timeout | error | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `std/utilities/meta` | 155 | 115 | 33 | 0 | 0 | 7 (6) | 0 | 0 | 0 | 77.7% |
| `std/utilities/utility` | 168 | 28 | 117 | 0 | 0 | 23 (19) | 0 | 0 | 0 | 19.3% |
| `std/containers/sequences/array` | 47 | 25 | 12 | 0 | 0 | 10 (10) | 0 | 0 | 0 | 67.6% |
| `std/strings/string.view` | 93 | 49 | 39 | 0 | 0 | 5 (5) | 0 | 0 | 0 | 55.7% |
| `std/containers/views/views.span` | 45 | 15 | 20 | 0 | 0 | 10 (9) | 0 | 0 | 0 | 42.9% |
| `std/numerics/bit` | 17 | 12 | 3 | 0 | 0 | 2 (2) | 0 | 0 | 0 | 80.0% |
| `std/language.support/support.limits` | 112 | 58 | 54 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 51.8% |
| `std/language.support/support.initlist` | 5 | 5 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 100.0% |
| **total** | 642 | 307 | 278 | 0 | 0 | 57 (51) | 0 | 0 | 0 | 52.5% |

### Failures by stage

| directory | compile | link | run | expected-fail-but-succeeded |
|---|---:|---:|---:|---:|
| `std/utilities/meta` | 33 | 0 | 0 | 0 |
| `std/utilities/utility` | 117 | 0 | 0 | 0 |
| `std/containers/sequences/array` | 12 | 0 | 0 | 0 |
| `std/strings/string.view` | 39 | 0 | 0 | 0 |
| `std/containers/views/views.span` | 20 | 0 | 0 | 0 |
| `std/numerics/bit` | 3 | 0 | 0 | 0 |
| `std/language.support/support.limits` | 54 | 0 | 0 | 0 |
| `std/language.support/support.initlist` | 0 | 0 | 0 | 0 |

### Dominant first errors per directory

The three most common first-error clusters among each directory's FAIL tests, with their test counts.

| directory | clusters |
|---|---|
| `std/utilities/meta` | `compile: kiln: not yet implemented: type trait '…'` (7); `compile: call to member function '…' is ambiguous` (5); `compile: static assertion failed due to requirement '…':` (5) |
| `std/utilities/utility` | `compile: constructor cannot be redeclared` (94); `compile: use of undeclared identifier '…'` (5); `compile: this kind of name in a using-declarator is not supported yet` (3) |
| `std/containers/sequences/array` | `compile: this kind of name in a using-declarator is not supported yet` (4); `compile: constructor cannot be redeclared` (3); `compile: a destructor template-id is not supported yet` (2) |
| `std/strings/string.view` | `compile: this kind of name in a using-declarator is not supported yet` (10); `compile: called object type '…' is not a function or function pointer` (6); `compile: static assertion expression is not an integral constant expression` (5) |
| `std/containers/views/views.span` | `compile: this kind of name in a using-declarator is not supported yet` (8); `compile: initialization by an initializer-list constructor is not supported yet` (4); `compile: constructor cannot be redeclared` (3) |
| `std/numerics/bit` | `compile: chosen constructor is explicit in copy-initialization` (1); `compile: static assertion failed due to requirement '…'` (1); `compile: use of undeclared identifier '…'` (1) |
| `std/language.support/support.limits` | `compile: constructor cannot be redeclared` (19); `compile: use of undeclared identifier '…'` (11); `compile: expected member name or '…' after declaration specifiers` (6) |
| `std/language.support/support.initlist` | — |

### Error clusters

100 clusters over the FAIL tests (and compiler crashes). A cluster is the failing step's `error:` message with the location dropped, quoted names replaced by `'…'` and numbers by `N`. "first" counts tests whose first error is the cluster, "any" tests that report it anywhere. Top 40:

| # | first | any | cluster | example test | example message |
|---:|---:|---:|---|---|---|
| 1 | 124 | 170 | `compile: constructor cannot be redeclared` | `std/containers/sequences/array/array.tuple/get_const_rv.pass.cpp` | `constructor cannot be redeclared` |
| 2 | 28 | 40 | `compile: this kind of name in a using-declarator is not supported yet` | `std/containers/sequences/array/iterator_concept_conformance.compile.pass.cpp` | `this kind of name in a using-declarator is not supported yet` |
| 3 | 19 | 171 | `compile: use of undeclared identifier '…'` | `std/containers/views/views.span/span.cons/iterator_sentinel.pass.cpp` | `use of undeclared identifier '__builtin_labs'` |
| 4 | 10 | 131 | `compile: call to member function '…' is ambiguous` | `std/containers/views/views.span/span.elem/at.pass.cpp` | `call to member function 'operator=' is ambiguous` |
| 5 | 9 | 77 | `compile: kiln: not yet implemented: type trait '…'` | `std/language.support/support.limits/support.limits.general/thread.version.compile.pass.cpp` | `kiln: not yet implemented: type trait '__is_trivially_assignable'` |
| 6 | 8 | 9 | `compile: a destructor template-id is not supported yet` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `a destructor template-id is not supported yet` |
| 7 | 6 | 83 | `compile: expected member name or '…' after declaration specifiers` | `std/language.support/support.limits/support.limits.general/atomic.version.compile.pass.cpp` | `expected member name or ';' after declaration specifiers` |
| 8 | 6 | 18 | `compile: initialization by an initializer-list constructor is not supported yet` | `std/containers/views/views.span/span.cons/array.pass.cpp` | `initialization by an initializer-list constructor is not supported yet` |
| 9 | 6 | 16 | `compile: static assertion failed due to requirement '…':` | `std/containers/sequences/array/size_and_alignment.compile.pass.cpp` | `static assertion failed due to requirement 'alignof(std::array<short, 0>) == alignof(short)':` |
| 10 | 6 | 12 | `compile: called object type '…' is not a function or function pointer` | `std/strings/string.view/string.view.comparison/equal.pass.cpp` | `called object type '<overloaded function type>' is not a function or function pointer` |
| 11 | 5 | 53 | `compile: static assertion expression is not an integral constant expression` | `std/strings/string.view/string.view.find/find_first_of_pointer_size.pass.cpp` | `static assertion expression is not an integral constant expression` |
| 12 | 5 | 15 | `compile: kiln: not yet implemented: substitution into CXXTryStmt` | `std/strings/string.view/string.view.access/at.pass.cpp` | `kiln: not yet implemented: substitution into CXXTryStmt` |
| 13 | 5 | 7 | `compile: template non-type parameter has a different type '…' (aka '…') in template redeclaration` | `std/language.support/support.limits/support.limits.general/deque.version.compile.pass.cpp` | `template non-type parameter has a different type '_Dp' (aka 'type-parameter-1-4') in template redeclaration` |
| 14 | 4 | 13 | `compile: static assertion failed due to requirement '…'` | `std/numerics/bit/bit.cast/bit_cast.compile.pass.cpp` | `static assertion failed due to requirement '!...'` |
| 15 | 4 | 4 | `compile: aggregate expression ConditionalOperator is not supported yet` | `std/strings/string.view/string.view.template/ends_with.ptr.pass.cpp` | `aggregate expression ConditionalOperator is not supported yet` |
| 16 | 4 | 4 | `compile: array has incomplete element type '…'` | `std/utilities/meta/meta.unary/meta.unary.cat/array.pass.cpp` | `array has incomplete element type 'Incomplete'` |
| 17 | 3 | 58 | `compile: explicit object parameters is not supported yet` | `std/language.support/support.limits/support.limits.general/chrono.version.compile.pass.cpp` | `explicit object parameters is not supported yet` |
| 18 | 3 | 14 | `compile: no viable conversion from '…' (aka '…') to '…' (aka '…')` | `std/containers/sequences/array/array.overview/nttp.equivalence.compile.pass.cpp` | `no viable conversion from 'std::array<int, 3>' (aka 'std::__1::array<int, 3>') to 'std::array' (aka 'std::__1::array')` |
| 19 | 2 | 108 | `compile: kiln: not yet implemented: local UsingDirective declarations in templates` | `std/language.support/support.limits/support.limits.general/shared_mutex.version.compile.pass.cpp` | `kiln: not yet implemented: local UsingDirective declarations in templates` |
| 20 | 2 | 92 | `compile: no member named '…' in '…'` | `std/containers/views/views.span/span.cons/initializer_list.pass.cpp` | `no member named '_SmallHandler' in '__any_imp'` |
| 21 | 2 | 25 | `compile: kiln: not yet implemented: partially substituted pack expansion` | `std/utilities/utility/pairs/pair.piecewise/piecewise_construct.pass.cpp` | `kiln: not yet implemented: partially substituted pack expansion` |
| 22 | 2 | 5 | `compile: chosen constructor is explicit in copy-initialization` | `std/numerics/bit/byteswap.pass.cpp` | `chosen constructor is explicit in copy-initialization` |
| 23 | 2 | 3 | `compile: constexpr if condition is not a constant expression` | `std/containers/views/views.span/span.objectrep/as_bytes.pass.cpp` | `constexpr if condition is not a constant expression` |
| 24 | 2 | 2 | `compile: braced-init-list operands of a functional cast is not supported yet` | `std/utilities/utility/pairs/pairs.pair/ctor.brace-init.pass.cpp` | `braced-init-list operands of a functional cast is not supported yet` |
| 25 | 1 | 26 | `compile: redefinition of '…'` | `std/language.support/support.limits/support.limits.general/expected.version.compile.pass.cpp` | `redefinition of 'operator=='` |
| 26 | 1 | 12 | `compile: no type named '…' in '…'` | `std/utilities/meta/meta.trans/meta.trans.other/common_reference.compile.pass.cpp` | `no type named 'type' in 'std::__1::common_reference<B &, D &>'` |
| 27 | 1 | 8 | `compile: ambiguous partial specializations of '…'` | `std/utilities/meta/meta.logical/conjunction.compile.pass.cpp` | `ambiguous partial specializations of 'conjunction<std::true_type>'` |
| 28 | 1 | 7 | `compile: invalid operands to binary expression ('…' (aka '…') and '…' (aka '…'))` | `std/language.support/support.limits/support.limits.general/charconv.version.compile.pass.cpp` | `invalid operands to binary expression ('chars_format' (aka 'std::__1::chars_format') and 'chars_format' (aka 'std::__1::chars_format'))` |
| 29 | 1 | 6 | `compile: static_cast from '…' (aka '…') to '…' (aka '…') is not allowed` | `std/utilities/utility/mem.res/mem.res.aliases/header_list_synop2.pass.cpp` | `static_cast from '__base_pointer' (aka 'std::__1::__list_node_base<std::__1::__list_imp<int, polymorphic_allocator<int>>::value_type, std::__1::__list_imp<int, …` |
| 30 | 1 | 3 | `compile: static assertion failed due to requirement '…': Types differ unexpectedly` | `std/utilities/meta/meta.trans/meta.trans.other/decay.pass.cpp` | `static assertion failed due to requirement 'test_macros_detail::is_same<int (int) const, std::decay<int (int) const>::type>::value': Types differ unexpectedly` |
| 31 | 1 | 1 | `compile: IR lowering failed: IrError { kind: BadCall, message: "call with N arguments to a function taking N" }` | `std/language.support/support.limits/limits/numeric.limits/default.pass.cpp` | `IR lowering failed: IrError { kind: BadCall, message: "call with 1 arguments to a function taking 2" }` |
| 32 | 1 | 1 | `compile: cast Dependent is not supported yet` | `std/strings/string.view/string.view.hash/string_view.pass.cpp` | `cast Dependent is not supported yet` |
| 33 | 1 | 1 | `compile: implicit assignment of an array of this type is not supported yet` | `std/containers/sequences/array/array.cons/implicit_copy.pass.cpp` | `implicit assignment of an array of this type is not supported yet` |
| 34 | 1 | 1 | `compile: static assertion failed due to requirement '…': Operation must be noexcept` | `std/utilities/utility/exchange/exchange.pass.cpp` | `static assertion failed due to requirement 'noexcept(std::exchange(x, std::move(x)))': Operation must be noexcept` |
| 35 | 1 | 1 | `compile: static assertion failed due to requirement '…': streaming operator declaration not present` | `std/strings/string.view/string.view.io/stream_insert_decl_present.compile.pass.cpp` | `static assertion failed due to requirement 'HasDecl<std::string_view>::value': streaming operator declaration not present` |
| 36 | 0 | 60 | `compile: expected expression` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `expected expression` |
| 37 | 0 | 58 | `compile: expected '…' for function-style cast or type construction` | `std/containers/sequences/array/range_concept_conformance.compile.pass.cpp` | `expected '(' for function-style cast or type construction` |
| 38 | 0 | 52 | `compile: unknown type name '…'` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `unknown type name '__SVFloat32_t'` |
| 39 | 0 | 44 | `compile: expected body of lambda expression` | `std/containers/sequences/array/range_concept_conformance.compile.pass.cpp` | `expected body of lambda expression` |
| 40 | 0 | 44 | `compile: static assertion failed due to requirement '…': unexpected required size for mask integer type` | `std/containers/sequences/array/range_concept_conformance.compile.pass.cpp` | `static assertion failed due to requirement 'sizeof(bool) == 0': unexpected required size for mask integer type` |

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
