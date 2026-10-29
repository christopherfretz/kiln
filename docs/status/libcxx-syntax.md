# libc++ under `-fsyntax-only`: first blockers

Each header is checked alone, as `#include <h>` in a one-line file:

```
kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0 h.cpp
```

The setup is libc++ 19.1.7 (Debian `libc++-19-dev`) on aarch64 Linux,
with kiln at WP-066b (class members in Sema: member lookup through
bases, access control, `this`, member access and calls, pointers to
members, static data members, friends) merged with main (WP-064,
WP-061a, WP-061b). The parser side of classes is WP-065, not yet
merged. "First
blocker" is the first error kiln reports. The counts are all the errors
in the header's closure; most are follow-ons of the same unsupported
feature. Every header preprocesses identically to clang (see
`pp-parity.md`), so these are all parser or Sema gaps.

| header | exit | errors | was (main before WP-066b) | first blocker | main error kinds |
|---|---:|---:|---:|---|---|
| `<cstddef>` | 1 | 42 | 42 | `__type_traits/enable_if.h:20:1`: templates | templates 38, special members 1 |
| `<cstdint>` | 0 | 0 | 0 | none (clean since WP-062; no warnings since WP-063) | |
| `<climits>` | 0 | 0 | 0 | none (clean; was blocked by glibc's `__THROW`, `noexcept(true)`, before WP-058) | |
| `<cfloat>` | 0 | 0 | 0 | none (clean) | |
| `<cstring>` | 1 | 10 | 10 | `string.h:89:20`: overloading `memchr` | overloading 10 |
| `<version>` | 0 | 0 | 0 | none (clean) | |
| `<initializer_list>` | 1 | 45 | 45 | `__type_traits/enable_if.h:20:1`: templates | templates 41, special members 1 |
| `<compare>` | 1 | 385 | 418 | `__type_traits/enable_if.h:20:1`: templates | templates 284, undeclared identifiers 25, overloading 14, special members 12, class-object initialization 11 |
| `<type_traits>` | 1 | 453 | 453 | `__fwd/functional.h:20:1`: templates | templates 436, special members 5 |

No header reports a `decltype` error any more (WP-064): libc++'s
`stddef.h` `typedef decltype(nullptr) nullptr_t;` now declares
`std::nullptr_t`, which removes one error from each header that
includes it.

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
   `memchr`, `strchr`, ...): all that is left in `<cstring>`.
3. **Class members**: the largest non-template error kinds across
   `bench/pp/headers.cpp` are now parser rejections (default member
   initializers 5 227, member functions 214, member function qualifiers
   204, access specifiers 168) that WP-065 removes, then special member
   functions (473, WP-067). WP-066b implemented the Sema side, so
   in-class static data members no longer fail: `<compare>`'s 11
   static-data-member errors are gone, and the declarations they guarded
   now reach class-object initialization (WP-067). The
   `'x' is a private member of 'C'` errors in `headers.cpp` are
   follow-ons of the parser rejecting `public:`: those members stay
   implicitly private until WP-065 lands.

using-declarations (WP-062), asm labels and
`__builtin_is_constant_evaluated` (WP-063) no longer block anything:
`<cstdint>` is clean, and `<cstring>` went from 56 errors (WP-058) to 11 (10 after WP-064).
Exception specifications (`noexcept`, `noexcept(true)`, `throw()`) no
longer block anything (WP-058). Declarations that used to stop there
now reach later gaps, so some counts rose (`<cstring>`'s overloading
errors went from 2 to 10).

kiln suppresses warnings in system headers the way clang does
(WP-063), so no header above produces a warning (the
`unknown attribute '__type_visibility__'`-style noise is gone).

No header in `bench/pp/headers.cpp` (99 libc++ headers) crashes the
parser. `headers.cpp` as a whole exits 1, with 15 198 errors and no
warnings (15 492 on main before WP-066b, 15 534 before WP-064, 15 632 at WP-062, 16 182 at WP-058, 13 826 before it: getting
past exception specifications exposed more follow-on errors;
using-declarations, asm labels, `__builtin_is_constant_evaluated` and
the WP-064 expressions and the WP-066b class members removed some). Its
largest error kinds are templates (6 845), default member initializers
(5 227), special member functions (473), member functions (214) and
member function qualifiers (204).
