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

- kiln: debug build of the WP-081 branch at `683f8ca`, which includes main at `b66b6b9`.
- Tests and headers: libc++ `llvmorg-19.1.7`. The tests come from the sparse checkout. The headers,
  `libc++.so`, `libc++experimental.a` and `libc++abi` come from the clang 19 packages in the
  container.
- Host: aarch64 Debian 13 container, with `-j 2` and an 8 GiB address-space cap per process.
- Driver: lit-format subset with a static feature set. `.verify.cpp` tests are UNSUPPORTED by the
  driver because kiln has no `-verify`. See `spec/libcxx.md`.

## Reading

**Overall: 20/585 (3.4%).** Every failure is at the compile stage, so no test reaches link or run
except those that already pass. There are no KILLED, TIMEOUT or hang results. Peak RSS was 72 MiB
and wall time about 3 minutes. Two blockers cover most of the gate:

1. **GNU `asm` statements (273 first, 392 any).** `support/test_macros.h:294-303` (`DoNotOptimize`,
   `ClobberMemory`) uses `asm volatile("" ...)`. Every `.pass.cpp` that includes `test_macros.h`
   hits `'asm' statements is not supported yet`. This is a harness-wide parse blocker, not a
   library one.
2. **Panic at `kiln_ast/src/decl.rs:1907` "decl has no Function payload" (213 tests).** Any
   test that transitively includes `<string>` reaches it, because the panic happens while
   instantiating `basic_string<char>` from `_LIBCPP_STRING_V1_EXTERN_TEMPLATE_LIST`. Reproducer:
   `#include <string>` with `-ferror-limit=0`, which gives 31 errors and then exit 70. With the
   default error limit of 20, kiln stops before the panic.

Per gate header:

- **`<type_traits>` (meta, 6/148).** Dominated by `asm` (128 first). Once asm is removed, meta
  reaches 108/148 (73%). The remaining failures are the `<string>` panic (10), the NYI type
  trait `__is_trivially_assignable` (48 any), and a few wrong `static_assert` values. One test
  (`is_abstract.pass.cpp`) panics at `decl.rs:2234` with "FnSpec bit is not settable after
  creation".
- **`<utility>` (6/145).** Dominated by the `<string>` panic (91 first), then `asm` (26) and
  `__PRETTY_FUNCTION__` from `<cassert>` (15). `pair`/`tuple` tests also hit `<tuple>:471`
  `expected '…' or '…'` and the NYI `partially substituted pack expansion` (249 any across the
  gate).
- **`<array>` (2/37).** Dominated by `asm` (25), then the `<string>` panic (4) and
  `__PRETTY_FUNCTION__`. Without asm the next blocker is `__PRETTY_FUNCTION__` in `assert()`,
  plus `alignof(std::array<short, 0>)`.
- **`<string_view>` (3/88).** Dominated by the `<string>` panic (54 first), then `asm` (23) and
  `function '__get_iter_concept' with deduced return type cannot be used before it is defined`.
  `<string_view>` on its own compiles cleanly. The tests pull in `<string>` through test support.
- **`<span>` (2/35).** Dominated by the `<string>` panic (29 of 33 failures). `<span>` on its
  own compiles cleanly.
- **`<bit>` (0/15).** Dominated by `asm` (13). Without asm, all 15 still fail on
  `not yet implemented: substitution into RecoveryExpr` and `chosen constructor is explicit in
  copy-initialization` (byteswap, popcount and others).
- **`<version>`/`<limits>` (support.limits, 1/111).** These are mostly the
  `*.version.compile.pass.cpp` feature-test-macro tests, which include whole headers. The
  dominant blockers are `asm` (52) and the `<string>` panic (25), then `<tuple>`/`<memory>` errors
  and the chrono `typedef redefinition with different types ('int' vs 'long long')` (90 any).
  Without asm, the rate is 44/112.
- **`<initializer_list>` (support.initlist, 0/5).** All 5 fail first on `asm`. Without asm, 2/5
  pass and the rest fail on `__PRETTY_FUNCTION__`.

**Experiment without asm.** To see what lies behind blocker 1, I re-ran the gate with
`--libcxx-tests` pointing at a copy of `libcxx/test/support` with the `asm` lines deleted, and
`std/` symlinked. Result: **180/585 (30.8%)**.

| directory | pass |
|---|---:|
| meta | 108/148 (73.0%) |
| utility | 12 (8.3%) |
| array | 5 (13.5%) |
| string.view | 6 (6.8%) |
| span | 3 (8.6%) |
| bit | 0 |
| support.limits | 44 (39.3%) |
| initlist | 2 (40%) |

In that run, the first-error leaders were the `<string>` panic and `__PRETTY_FUNCTION__` (102
first). Only one test reached link and failed there (support.limits).

**Caveats.**
- 5 of the 20 passes are `.compile.fail.cpp` tests. They may be passing because of the asm error
  rather than the diagnostic they target. The driver only checks that the compile fails without
  a crash.
- XFAILs written against compiler identity (`clang-*`, `gcc-*`) evaluate to false, because the
  feature set names only `kiln`.

**Candidate work packages, in order of gate impact:**
1. GNU `asm` statement parsing.
2. The `decl.rs:1907` panic on `<string>`.
3. `__PRETTY_FUNCTION__`/`__FUNCTION__`.
4. Partially substituted pack expansion (`<tuple>`).
5. `excess elements in scalar initializer` (238 any).
6. Anonymous member classes in class templates (191 any).
7. basic_string `__short` layout (157 any).
8. RecoveryExpr substitution (`<bit>`).
9. The chrono `int` vs `long long` typedef.
10. `__is_trivially_assignable` and other type-trait builtins.
11. `__SVFloat32_t` and other SVE builtin types (47 any).
12. `__builtin_strcmp`.
13. `-verify` (51 driver-unsupported tests).
14. The `FnSpec` panic.

<!-- libcxx:begin -->
<!-- generated by `cargo xtask libcxx --report`; edits inside the markers are overwritten -->

### Run

| | |
|---|---|
| kiln | `kiln version 0.0.1 (C++26 frontend, LLVM IR backend)` at commit `f75b102` |
| libc++ tests | `llvmorg-19.1.7` (`libcxx/test`) |
| libc++ headers | `_LIBCPP_VERSION` 190107 |
| target | `aarch64-unknown-linux-gnu` |
| command | `cargo xtask libcxx --gate m3 -j 2` |
| compile flags | `-std=c++26 -stdlib=libc++ -resource-dir <repo>/resource -pthread -ferror-limit=0 -D_LIBCPP_ENABLE_EXPERIMENTAL -I <llvm-project>/libcxx/test/support <ADDITIONAL_COMPILE_FLAGS> <test>` |
| link flags | `t.o -o t.exe -stdlib=libc++ -pthread -lc++experimental -lc++abi` |
| features | `-fsized-deallocation`, `buildhost=linux`, `c++26`, `c++experimental`, `can-create-symlinks`, `has-64-bit-atomics`, `has-fconstexpr-steps`, `has-unix-headers`, `kiln`, `large_tests`, `libcpp-abi-version=1`, `libcpp-has-no-availability-markup`, `libcpp-has-thread-api-pthread`, `linux`, `long_tests`, `optimization=none`, `stdlib=libc++`, `stdlib=llvm-libc++`, `target=aarch64-unknown-linux-gnu` |
| limits | 8192 MiB address space per process (kiln: plus its stack reservation); compile/link timeout 120 s; run timeout 30 s; -j 2 |
| wall time | 184.0 s |
| peak RSS | 72 MiB (largest single process, `std/strings/string.view/string.view.find/find_last_of_string_view_size.pass.cpp`; gnu-time) |

### Results by directory

Pass rate is PASS / (total − UNSUPPORTED). "driver" counts the UNSUPPORTED tests the driver cannot run (for example `.verify.cpp`), as opposed to those the configuration excludes (`UNSUPPORTED: c++03`).

| directory | total | pass | fail | xfail | xpass | unsupported (driver) | killed | timeout | error | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `std/utilities/meta` | 155 | 110 | 38 | 0 | 0 | 7 (6) | 0 | 0 | 0 | 74.3% |
| `std/utilities/utility` | 168 | 25 | 120 | 0 | 0 | 23 (19) | 0 | 0 | 0 | 17.2% |
| `std/containers/sequences/array` | 47 | 19 | 18 | 0 | 0 | 10 (10) | 0 | 0 | 0 | 51.4% |
| `std/strings/string.view` | 93 | 19 | 69 | 0 | 0 | 5 (5) | 0 | 0 | 0 | 21.6% |
| `std/containers/views/views.span` | 45 | 3 | 32 | 0 | 0 | 10 (9) | 0 | 0 | 0 | 8.6% |
| `std/numerics/bit` | 17 | 12 | 3 | 0 | 0 | 2 (2) | 0 | 0 | 0 | 80.0% |
| `std/language.support/support.limits` | 112 | 51 | 61 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 45.5% |
| `std/language.support/support.initlist` | 5 | 5 | 0 | 0 | 0 | 0 (0) | 0 | 0 | 0 | 100.0% |
| **total** | 642 | 244 | 341 | 0 | 0 | 57 (51) | 0 | 0 | 0 | 41.7% |

### Failures by stage

| directory | compile | link | run | expected-fail-but-succeeded |
|---|---:|---:|---:|---:|
| `std/utilities/meta` | 38 | 0 | 0 | 0 |
| `std/utilities/utility` | 120 | 0 | 0 | 0 |
| `std/containers/sequences/array` | 18 | 0 | 0 | 0 |
| `std/strings/string.view` | 67 | 2 | 0 | 0 |
| `std/containers/views/views.span` | 32 | 0 | 0 | 0 |
| `std/numerics/bit` | 3 | 0 | 0 | 0 |
| `std/language.support/support.limits` | 60 | 1 | 0 | 0 |
| `std/language.support/support.initlist` | 0 | 0 | 0 | 0 |

### Dominant first errors per directory

The three most common first-error clusters among each directory's FAIL tests, with their test counts.

| directory | clusters |
|---|---|
| `std/utilities/meta` | `crash: panicked at crates/kiln_ast/src/decl.rs: decl has no Function payload` (10); `compile: kiln: not yet implemented: type trait '…'` (7); `compile: static assertion failed due to requirement '…':` (7) |
| `std/utilities/utility` | `crash: panicked at crates/kiln_ast/src/decl.rs: decl has no Function payload` (91); `compile: expected member name or '…' after declaration specifiers` (11); `compile: constructor cannot be redeclared` (4) |
| `std/containers/sequences/array` | `crash: panicked at crates/kiln_ast/src/decl.rs: decl has no Function payload` (4); `compile: expected member name or '…' after declaration specifiers` (3); `compile: '…' does not refer to a value` (2) |
| `std/strings/string.view` | `crash: panicked at crates/kiln_ast/src/decl.rs: decl has no Function payload` (54); `compile: excess elements in scalar initializer` (4); `compile: function '…' with deduced return type cannot be used before it is defined` (4) |
| `std/containers/views/views.span` | `crash: panicked at crates/kiln_ast/src/decl.rs: decl has no Function payload` (29); `compile: expected member name or '…' after declaration specifiers` (1); `compile: expected '…' or '…'` (1) |
| `std/numerics/bit` | `compile: chosen constructor is explicit in copy-initialization` (1); `compile: static assertion failed due to requirement '…'` (1); `compile: use of undeclared identifier '…'` (1) |
| `std/language.support/support.limits` | `crash: panicked at crates/kiln_ast/src/decl.rs: decl has no Function payload` (25); `compile: use of undeclared identifier '…'` (9); `compile: expected '…' or '…'` (8) |
| `std/language.support/support.initlist` | — |

### Error clusters

103 clusters over the FAIL tests (and compiler crashes). A cluster is the failing step's `error:` message with the location dropped, quoted names replaced by `'…'` and numbers by `N`. "first" counts tests whose first error is the cluster, "any" tests that report it anywhere. Top 40:

| # | first | any | cluster | example test | example message |
|---:|---:|---:|---|---|---|
| 1 | 213 | 213 | `crash: panicked at crates/kiln_ast/src/decl.rs: decl has no Function payload` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `panicked at crates/kiln_ast/src/decl.rs:1907:5: decl has no Function payload` |
| 2 | 18 | 259 | `compile: expected member name or '…' after declaration specifiers` | `std/containers/sequences/array/array.tuple/get_const_rv.pass.cpp` | `expected member name or ';' after declaration specifiers` |
| 3 | 14 | 249 | `compile: expected '…' or '…'` | `std/containers/sequences/array/range_concept_conformance.compile.pass.cpp` | `expected '(' or '{'` |
| 4 | 13 | 140 | `compile: use of undeclared identifier '…'` | `std/language.support/support.limits/limits/numeric.limits.members/quiet_NaN.pass.cpp` | `use of undeclared identifier '__builtin_fabsf'` |
| 5 | 11 | 146 | `compile: '…' does not refer to a value` | `std/containers/sequences/array/indexing.pass.cpp` | `'reference' does not refer to a value` |
| 6 | 10 | 37 | `compile: function '…' with deduced return type cannot be used before it is defined` | `std/containers/sequences/array/types.pass.cpp` | `function '__get_iter_concept' with deduced return type cannot be used before it is defined` |
| 7 | 8 | 48 | `compile: kiln: not yet implemented: type trait '…'` | `std/utilities/meta/meta.unary/meta.unary.prop/is_trivially_assignable.pass.cpp` | `kiln: not yet implemented: type trait '__is_trivially_assignable'` |
| 8 | 8 | 15 | `compile: static assertion failed due to requirement '…':` | `std/containers/sequences/array/size_and_alignment.compile.pass.cpp` | `static assertion failed due to requirement 'alignof(std::array<short, 0>) == alignof(short)':` |
| 9 | 6 | 87 | `compile: using declaration refers into '…', which is not a base class of '…'` | `std/language.support/support.limits/support.limits.general/algorithm.version.compile.pass.cpp` | `using declaration refers into '__base::', which is not a base class of '__optional_storage_base'` |
| 10 | 5 | 93 | `compile: constructor cannot be redeclared` | `std/language.support/support.limits/support.limits.general/mdspan.version.compile.pass.cpp` | `constructor cannot be redeclared` |
| 11 | 4 | 238 | `compile: excess elements in scalar initializer` | `std/strings/string.view/string.view.access/at.pass.cpp` | `excess elements in scalar initializer` |
| 12 | 4 | 6 | `compile: static assertion failed due to requirement '…'` | `std/numerics/bit/bit.cast/bit_cast.compile.pass.cpp` | `static assertion failed due to requirement '!...'` |
| 13 | 4 | 4 | `compile: array has incomplete element type '…'` | `std/utilities/meta/meta.unary/meta.unary.cat/array.pass.cpp` | `array has incomplete element type 'Incomplete'` |
| 14 | 3 | 257 | `compile: expected '…'` | `std/containers/sequences/array/compare.pass.cpp` | `expected ')'` |
| 15 | 3 | 6 | `compile: no viable conversion from '…' (aka '…') to '…' (aka '…')` | `std/containers/sequences/array/array.overview/nttp.equivalence.compile.pass.cpp` | `no viable conversion from 'std::array<int, 3>' (aka 'std::__1::array<int, 3>') to 'std::array' (aka 'std::__1::array')` |
| 16 | 3 | 3 | `link: linker command failed with exit code N (use -v to see invocation)` | `std/language.support/support.limits/limits/numeric.limits.members/const_data_members.pass.cpp` | `linker command failed with exit code 1 (use -v to see invocation)` |
| 17 | 2 | 78 | `compile: expected unqualified-id` | `std/containers/sequences/array/front_back.pass.cpp` | `expected unqualified-id` |
| 18 | 2 | 4 | `compile: chosen constructor is explicit in copy-initialization` | `std/numerics/bit/byteswap.pass.cpp` | `chosen constructor is explicit in copy-initialization` |
| 19 | 1 | 59 | `compile: call to member function '…' is ambiguous` | `std/utilities/meta/meta.rel/is_same.pass.cpp` | `call to member function 'fn' is ambiguous` |
| 20 | 1 | 2 | `compile: ambiguous partial specializations of '…'` | `std/utilities/meta/meta.logical/conjunction.compile.pass.cpp` | `ambiguous partial specializations of 'conjunction<std::true_type>'` |
| 21 | 1 | 2 | `compile: static assertion failed due to requirement '…': Types differ unexpectedly` | `std/utilities/meta/meta.trans/meta.trans.other/decay.pass.cpp` | `static assertion failed due to requirement 'test_macros_detail::is_same<int (int) const, std::decay<int (int) const>::type>::value': Types differ unexpectedly` |
| 22 | 1 | 1 | `compile: IR lowering failed: IrError { kind: BadCall, message: "call with N arguments to a function taking N" }` | `std/language.support/support.limits/limits/numeric.limits/default.pass.cpp` | `IR lowering failed: IrError { kind: BadCall, message: "call with 1 arguments to a function taking 2" }` |
| 23 | 1 | 1 | `compile: braced-init-list operands of a functional cast is not supported yet` | `std/utilities/utility/pairs/pairs.pair/ctor.brace-init.pass.cpp` | `braced-init-list operands of a functional cast is not supported yet` |
| 24 | 1 | 1 | `compile: implicit assignment of an array of this type is not supported yet` | `std/containers/sequences/array/array.cons/implicit_copy.pass.cpp` | `implicit assignment of an array of this type is not supported yet` |
| 25 | 1 | 1 | `compile: invalid operands to binary expression ('…' (aka '…') and '…' (aka '…'))` | `std/language.support/support.limits/support.limits.general/charconv.version.compile.pass.cpp` | `invalid operands to binary expression ('chars_format' (aka 'std::__1::chars_format') and 'chars_format' (aka 'std::__1::chars_format'))` |
| 26 | 1 | 1 | `compile: redefinition of '…'` | `std/language.support/support.limits/support.limits.general/expected.version.compile.pass.cpp` | `redefinition of 'operator=='` |
| 27 | 1 | 1 | `compile: static assertion failed due to requirement '…': streaming operator declaration not present` | `std/strings/string.view/string.view.io/stream_insert_decl_present.compile.pass.cpp` | `static assertion failed due to requirement 'HasDecl<std::string_view>::value': streaming operator declaration not present` |
| 28 | 1 | 1 | `crash: panicked at crates/kiln_ast/src/decl.rs: FnSpec bit is not settable after creation` | `std/utilities/meta/meta.unary/meta.unary.prop/is_abstract.pass.cpp` | `panicked at crates/kiln_ast/src/decl.rs:2234:9: FnSpec bit is not settable after creation` |
| 29 | 0 | 249 | `compile: expected '…' after base class list` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `expected '{' after base class list` |
| 30 | 0 | 249 | `compile: kiln: not yet implemented: partially substituted pack expansion` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `kiln: not yet implemented: partially substituted pack expansion` |
| 31 | 0 | 249 | `compile: pack expansion does not contain any unexpanded parameter packs` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `pack expansion does not contain any unexpanded parameter packs` |
| 32 | 0 | 191 | `compile: kiln: not yet implemented: anonymous member classes of class templates` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `kiln: not yet implemented: anonymous member classes of class templates` |
| 33 | 0 | 157 | `compile: static assertion failed due to requirement '…': __short has an unexpected size.` | `std/containers/views/views.span/span.iterators/iterator.pass.cpp` | `static assertion failed due to requirement 'sizeof(std::basic_string<char>::__short) == (sizeof(char) * (__min_cap + 1))': __short has an unexpected size.` |
| 34 | 0 | 130 | `compile: no member named '…' in '…'` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `no member named 'value' in 'std::__1::is_trivially_assignable<char &, const char &>'` |
| 35 | 0 | 99 | `compile: template argument for template type parameter must be a type` | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp` | `template argument for template type parameter must be a type` |
| 36 | 0 | 91 | `compile: a type specifier is required for all declarations` | `std/containers/views/views.span/span.sub/first.pass.cpp` | `a type specifier is required for all declarations` |
| 37 | 0 | 91 | `compile: expected a type` | `std/containers/views/views.span/span.sub/first.pass.cpp` | `expected a type` |
| 38 | 0 | 90 | `compile: expected '…' at end of declaration list` | `std/containers/views/views.span/span.sub/first.pass.cpp` | `expected ';' at end of declaration list` |
| 39 | 0 | 90 | `compile: incomplete type '…' (aka '…') named in nested name specifier` | `std/containers/views/views.span/span.sub/first.pass.cpp` | `incomplete type 'system_clock' (aka 'std::__1::chrono::system_clock') named in nested name specifier` |
| 40 | 0 | 90 | `compile: typedef redefinition with different types ('…' vs '…' (aka '…'))` | `std/containers/views/views.span/span.sub/first.pass.cpp` | `typedef redefinition with different types ('int' vs 'typename nano::type' (aka 'std::__1::ratio<1, 1000000000>'))` |

### Crashes

| crash | tests | examples |
|---|---:|---|
| `crash: panicked at crates/kiln_ast/src/decl.rs: decl has no Function payload` | 213 | `std/containers/sequences/array/array.cons/implicit_copy.addressof.compile.pass.cpp`, `std/containers/sequences/array/array.creation/to_array.pass.cpp`, `std/containers/sequences/array/iterator_concept_conformance.compile.pass.cpp` |
| `crash: panicked at crates/kiln_ast/src/decl.rs: FnSpec bit is not settable after creation` | 1 | `std/utilities/meta/meta.unary/meta.unary.prop/is_abstract.pass.cpp` |

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
