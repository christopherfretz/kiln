# libc++ under `-fsyntax-only`: first blockers

Each header is checked alone, as `#include <h>` in a one-line file:

```
kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0 h.cpp
```

The setup is libc++ 19.1.7 (Debian `libc++-19-dev`) on aarch64 Linux,
with kiln main after WP-072 (2026-10-31: the whole M3 template engine —
070a/b, 071, 072, 073, 074, 087 — plus classes 067a/b, overloading
068a/b, virtuals 077/078/080, builtins 088, `__int128` 107), updated
for WP-109 (concepts and alias follow-ups) on the `<type_traits>`,
`<compare>`, `<utility>`, `<array>`, `<span>` and `<string_view>` rows. The M3
gate headers (WP-081) are listed last. "First
blocker" is the first error kiln reports. The counts are all the errors
in the header's closure; most are follow-ons of the same unsupported
feature. Every header preprocesses identically to clang (see
`pp-parity.md`), so these are all parser or Sema gaps.

| header | exit | errors | was (WP-069) | first blocker | main error kinds |
|---|---:|---:|---:|---|---|
| `<cstddef>` | 0 | 0 | 45 | none (clean since WP-070b/071) | |
| `<cstdint>` | 0 | 0 | 0 | none | |
| `<climits>` | 0 | 0 | 0 | none | |
| `<cfloat>` | 0 | 0 | 0 | none | |
| `<cstring>` | 0 | 0 | 10 | none (clean since WP-068a) | |
| `<version>` | 0 | 0 | 0 | none | |
| `<initializer_list>` | 0 | 0 | 47 | none (clean since WP-070b/071) | |
| `<bit>` | 0 | 0 | — | none | |
| `<limits>` | 0 | 0 | — | none | |
| `<type_traits>` | 0 | 0 | 431 | none (clean since WP-109) | |
| `<compare>` | 1 | 48 | 399 | defaulted comparison operators (WP-082) | defaulted comparisons, follow-ons |
| `<utility>` | 1 | 81 | — | defaulted comparison operators (WP-082) | defaulted comparisons, follow-ons |
| `<array>` | 1 | 177 | — | `__utility/swap.h:33`: type alias template redefinition with different types (`__swap_result_t`; expression profiles, spec/types.md §7.2) | member exception specifications ×64, constructor selection, friend templates |
| `<span>` | 1 | 113 | — | defaulted comparison operators (WP-082) | member exception specifications ×28, constructor selection, friend templates |
| `<string_view>` | 1 | 329 | — | same as `<array>` (`swap.h:33`) | ambiguous overloaded operator ×84, member exception specifications ×63 |

The single-header counts rose a little: before WP-065 the parser gave
up on a class at its first constructor or operator (one "special member
functions" error, then the rest of the class skipped), so the templates
and operators later in those classes went unreported. Now every member
is parsed and each special member reports its own Sema stub
("constructors", "overloaded operators", ... "is not supported yet").
The libc++-wide count fell by a third (below).

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
3. **Special members and operators**: with WP-065 and WP-066b merged,
   no class *syntax* is rejected any more. Default member initializers
   (5 227 errors on main), member functions (214), member function
   qualifiers (204), access specifiers (168) and base classes (45) are
   gone, and so are the 37 `'x' is a private member of 'C'` follow-ons
   of the old `public:` rejection. Across `headers.cpp` the largest
   class-related kinds are now Sema stubs: overloaded operators 362
   (WP-068b), constructors 283 and destructors 63 (WP-067a), overloaded
   member functions 62, class-object initialization 53 (WP-067).
4. **Parameter recovery**: the 105 `expected ')'` errors come from
   parameters whose type names something undeclared, mostly a template
   kiln could not declare (`const string&` in `<stdexcept>`,
   `const chrono::nanoseconds&`, `days __d`): clang reports
   `unknown type name` there and carries on, while kiln's
   decl-specifier recovery does not yet take an unknown `T` in
   `const T &` as a type.

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
parser. `headers.cpp` as a whole exits 1, with 9 891 errors and no
warnings (15 198 on main at WP-066b, 15 492 before it, 15 534 before
WP-064, 15 632 at WP-062, 16 182 at WP-058, 13 826 before it: getting
past exception specifications exposed more follow-on errors;
using-declarations, asm labels, `__builtin_is_constant_evaluated`, the
WP-064 expressions, the WP-066b class members and the WP-065 class
syntax removed some). Its largest error kinds are templates (6 870),
overloaded operators (362), `a type specifier is required` (332, mostly
template follow-ons), undeclared identifiers (309), constructors (283)
and expected unqualified-id (272).
