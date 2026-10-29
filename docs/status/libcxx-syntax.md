# libc++ under `-fsyntax-only`: first blockers

Each header is checked alone, as `#include <h>` in a one-line file:

```
kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0 h.cpp
```

The setup is libc++ 19.1.7 (Debian `libc++-19-dev`) on aarch64 Linux,
with kiln at WP-063 (asm labels, `__builtin_is_constant_evaluated`,
system-header diagnostic suppression; previously WP-058). "First
blocker" is the first error kiln reports. The counts are all the errors
in the header's closure; most are follow-ons of the same unsupported
feature. Every header preprocesses identically to clang (see
`pp-parity.md`), so these are all parser or Sema gaps.

| header | exit | errors | first blocker | main error kinds |
|---|---:|---:|---|---|
| `<cstddef>` | 1 | 47 | `__type_traits/enable_if.h:20:1`: templates | templates 38, using-declarations 4 |
| `<cstdint>` | 1 | 28 | `cstdint:161:1`: using-declarations (`using ::int8_t _LIBCPP_USING_IF_EXISTS;`) | using-declarations 28 |
| `<climits>` | 0 | 0 | none (clean; was blocked by glibc's `__THROW`, `noexcept(true)`, before WP-058) | |
| `<cfloat>` | 0 | 0 | none (clean) | |
| `<cstring>` | 1 | 34 | `stddef.h:41:9`: `decltype` (`typedef decltype(nullptr) nullptr_t;`) | using-declarations 23, overloading 10, `decltype` 1 |
| `<version>` | 0 | 0 | none (clean) | |
| `<initializer_list>` | 1 | 50 | `__type_traits/enable_if.h:20:1`: templates | templates 41, using-declarations 4 |
| `<compare>` | 1 | 451 | `__type_traits/enable_if.h:20:1`: templates | templates 284, using-declarations 32, undeclared identifiers 25, overloading 14, special members 12 |
| `<type_traits>` | 1 | 486 | `__fwd/functional.h:20:1`: templates | templates 436, using-declarations 32, special members 5 |

## What unblocks the most

1. **Templates** (M3): `<cstddef>`, `<initializer_list>`, `<compare>`,
   `<type_traits>`, and through them almost every other header.
2. **using-declarations** (`using ::name;`, with
   `__attribute__((__using_if_exists__))`): `<cstdint>`, `<cstring>`,
   and every C wrapper header.
3. **Overloading** (glibc's C++ `const`/non-`const` pairs of
   `memchr`, `strchr`, ...) and `decltype` (`decltype(nullptr)` in
   libc++'s `stddef.h`): `<cstring>`. asm labels and
   `__builtin_is_constant_evaluated` no longer block anything
   (WP-063): `<cstring>` went from 56 errors to 34.

Exception specifications (`noexcept`, `noexcept(true)`, `throw()`) no
longer block anything (WP-058). Declarations that used to stop there
now reach later gaps, so some counts rose (`<cstring>`'s overloading
errors went from 2 to 10).

kiln suppresses warnings in system headers the way clang does
(WP-063), so no header above produces a warning.

No header in `bench/pp/headers.cpp` (99 libc++ headers) crashes the
parser. `headers.cpp` as a whole exits 1, with 16 108 errors and no
warnings (16 182 errors before WP-063; 13 826 before WP-058, where getting
past exception specifications exposed more follow-on errors).
