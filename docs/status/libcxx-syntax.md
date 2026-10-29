# libc++ under `-fsyntax-only`: first blockers

Each header is checked alone, as `#include <h>` in a one-line file:

```
kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0 h.cpp
```

The setup is libc++ 19.1.7 (Debian `libc++-19-dev`) on aarch64 Linux,
with kiln at WP-066b (class members in Sema: member lookup through
bases, access control, `this`, member access and calls, pointers to
members, static data members; the parser side of classes is WP-065,
not yet merged). "First
blocker" is the first error kiln reports. The counts are all the errors
in the header's closure; most are follow-ons of the same unsupported
feature. Every header preprocesses identically to clang (see
`pp-parity.md`), so these are all parser or Sema gaps.

| header | exit | errors | was (WP-063) | first blocker | main error kinds |
|---|---:|---:|---:|---|---|
| `<cstddef>` | 1 | 43 | 43 | `__type_traits/enable_if.h:20:1`: templates | templates 38, special members 1, `decltype` 1 |
| `<cstdint>` | 0 | 0 | 0 | none (clean since WP-062; no warnings since WP-063) | |
| `<climits>` | 0 | 0 | 0 | none (clean; was blocked by glibc's `__THROW`, `noexcept(true)`, before WP-058) | |
| `<cfloat>` | 0 | 0 | 0 | none (clean) | |
| `<cstring>` | 1 | 11 | 11 | `stddef.h:41:9`: `decltype` (`typedef decltype(nullptr) nullptr_t;`) | overloading 10, `decltype` 1 |
| `<version>` | 0 | 0 | 0 | none (clean) | |
| `<initializer_list>` | 1 | 46 | 46 | `__type_traits/enable_if.h:20:1`: templates | templates 41, special members 1 |
| `<compare>` | 1 | 386 | 419 | `__type_traits/enable_if.h:20:1`: templates | templates 284, undeclared identifiers 25, overloading 14, special members 12, class-object initialization 11 |
| `<type_traits>` | 1 | 454 | 454 | `__fwd/functional.h:20:1`: templates | templates 436, special members 5 |

No header reports a using-declaration error any more (WP-062): the C
wrapper headers' `using ::name _LIBCPP_USING_IF_EXISTS;` lists resolve,
and a name glibc does not declare becomes an unresolved
`using_if_exists` declaration that is diagnosed only when used, as in
clang. No header reports an asm-label or `__builtin_is_constant_evaluated`
error any more (WP-063).

## What unblocks the most

1. **Templates** (M3): `<cstddef>`, `<initializer_list>`, `<compare>`,
   `<type_traits>`, and through them almost every other header.
2. **Overloading** (glibc's C++ `const`/non-`const` pairs of
   `memchr`, `strchr`, ...) and `decltype` (`decltype(nullptr)` in
   libc++'s `stddef.h`): all that is left in `<cstring>`.
3. **Class members**: the largest non-template error kinds across
   `bench/pp/headers.cpp` are now parser rejections (default member
   initializers 5 227, member functions 183, member function qualifiers
   175, access specifiers 167, base classes 45) that WP-065 removes,
   then special member functions (493, WP-067). WP-066b implemented the
   Sema side (member lookup through bases, access control, `this`,
   implicit `this->`, member calls, `.*`/`->*`, `&C::m`, static data
   members, friends), so in-class static data members no longer fail
   (`<compare>`'s 11 static-data-member errors are gone; the
   declarations they guarded now reach class-object initialization,
   WP-067). The 36 `'x' is a private member of 'C'` errors in
   `headers.cpp` are follow-ons of the parser rejecting `public:`: the
   members stay implicitly private until WP-065 lands.

using-declarations (WP-062), asm labels and
`__builtin_is_constant_evaluated` (WP-063) no longer block anything:
`<cstdint>` is clean, and `<cstring>` went from 56 errors (WP-058) to 11.
Exception specifications (`noexcept`, `noexcept(true)`, `throw()`) no
longer block anything (WP-058). Declarations that used to stop there
now reach later gaps, so some counts rose (`<cstring>`'s overloading
errors went from 2 to 10).

kiln suppresses warnings in system headers the way clang does
(WP-063), so no header above produces a warning (the
`unknown attribute '__type_visibility__'`-style noise is gone).

No header in `bench/pp/headers.cpp` (99 libc++ headers) crashes the
parser. `headers.cpp` as a whole exits 1, with 15 248 errors and no
warnings (15 534 at WP-063, 15 632 at WP-062, 16 182 at WP-058, 13 826 before it: getting
past exception specifications exposed more follow-on errors;
using-declarations, asm labels and `__builtin_is_constant_evaluated`
removed some).
