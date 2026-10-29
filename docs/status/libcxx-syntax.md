# libc++ under `-fsyntax-only`: first blockers

Each header is checked alone, as `#include <h>` in a one-line file:

```
kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0 h.cpp
```

The setup is libc++ 19.1.7 (Debian `libc++-19-dev`) on aarch64 Linux,
with kiln at WP-065 (class syntax: member functions, special member
names, friends, access labels, bases, deferred member parsing) on top of
WP-063 and WP-062. "First blocker" is the first error kiln reports. The
counts are all the errors in the header's closure; most are follow-ons
of the same unsupported feature. Every header preprocesses identically
to clang (see `pp-parity.md`), so these are all parser or Sema gaps.

| header | exit | errors | was (WP-063) | first blocker | main error kinds |
|---|---:|---:|---:|---|---|
| `<cstddef>` | 1 | 54 | 43 | `__type_traits/enable_if.h:20:1`: templates | templates 43, overloaded operators 7, `decltype` 1 |
| `<cstdint>` | 0 | 0 | 0 | none (clean since WP-062; no warnings since WP-063) | |
| `<climits>` | 0 | 0 | 0 | none (clean; was blocked by glibc's `__THROW`, `noexcept(true)`, before WP-058) | |
| `<cfloat>` | 0 | 0 | 0 | none (clean) | |
| `<cstring>` | 1 | 11 | 11 | `stddef.h:41:9`: `decltype` (`typedef decltype(nullptr) nullptr_t;`) | overloading 10, `decltype` 1 |
| `<version>` | 0 | 0 | 0 | none (clean) | |
| `<initializer_list>` | 1 | 57 | 46 | `__type_traits/enable_if.h:20:1`: templates | templates 46, overloaded operators 7 |
| `<compare>` | 1 | 447 | 419 | `__type_traits/enable_if.h:20:1`: templates | templates 289, friend declarations 36, undeclared identifiers 25 (`__builtin_isnan`, ...), static data members 11, out-of-line member definitions 11, member functions 9 |
| `<type_traits>` | 1 | 465 | 454 | `__fwd/functional.h:20:1`: templates | templates 441, overloaded operators 7, member functions 4 |

No header reports a using-declaration error any more (WP-062): the C
wrapper headers' `using ::name _LIBCPP_USING_IF_EXISTS;` lists resolve,
and a name glibc does not declare becomes an unresolved
`using_if_exists` declaration that is diagnosed only when used, as in
clang. No header reports an asm-label or `__builtin_is_constant_evaluated`
error any more (WP-063).

WP-065 parses the class syntax that used to stop a declaration
(`operator|`, constructors, destructors, friends, defaulted members), so
errors now come from Sema's per-feature stubs ("member functions",
"overloaded operators", "friend declarations", ... "is not supported
yet") and parsing continues with the next declaration. That is why
counts rose: `<cstddef>`'s seven `std::byte` operators were one
`special member functions` error that skipped the declarations after
it, and are now seven `overloaded operators` errors followed by the
templates they used to hide. No parser error kind is new.

## What unblocks the most

1. **Templates** (M3): `<cstddef>`, `<initializer_list>`, `<compare>`,
   `<type_traits>`, and through them almost every other header.
2. **Overloading** (glibc's C++ `const`/non-`const` pairs of
   `memchr`, `strchr`, ...) and `decltype` (`decltype(nullptr)` in
   libc++'s `stddef.h`): all that is left in `<cstring>`.
3. **Class members** (default member initializers, special member
   functions, member functions, static data members): the largest
   non-template error kinds across `bench/pp/headers.cpp`.

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
parser. `headers.cpp` as a whole exits 1, with 15 731 errors and no
warnings (15 534 at WP-063, 15 632 at WP-062, 16 182 at WP-058, 13 826
before it). The largest kinds are templates (6 854), default member
initializers (5 140, almost all the one `Pass::level` line the file's
x-macro section repeats), member functions (884), overloading (195),
access specifiers (187) and overloaded operators (171).
