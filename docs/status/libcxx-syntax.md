# libc++ under `-fsyntax-only`: first blockers

Each header is checked alone, as `#include <h>` in a one-line file:

```
kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0 h.cpp
```

The setup is libc++ 19.1.7 (Debian `libc++-19-dev`) on aarch64 Linux,
with kiln at WP-058 (exception specifications, the `noexcept` operator,
`if constexpr`/`if consteval`; previously WP-055). "First
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
| `<cstring>` | 1 | 56 | `__type_traits/is_constant_evaluated.h:22:10`: `__builtin_is_constant_evaluated` undeclared | using-declarations 23, asm labels 20, overloading 10, `__builtin_is_constant_evaluated` undeclared 2, `decltype` 1 |
| `<version>` | 0 | 0 | none (clean) | |
| `<initializer_list>` | 1 | 50 | `__type_traits/enable_if.h:20:1`: templates | templates 41, using-declarations 4 |
| `<compare>` | 1 | 451 | `__type_traits/enable_if.h:20:1`: templates | templates 284, using-declarations 32, undeclared identifiers 25, overloading 14, special members 12 |
| `<type_traits>` | 1 | 488 | `__fwd/functional.h:20:1`: templates | templates 436, using-declarations 32, special members 5 |

## What unblocks the most

1. **Templates** (M3): `<cstddef>`, `<initializer_list>`, `<compare>`,
   `<type_traits>`, and through them almost every other header.
2. **using-declarations** (`using ::name;`, with
   `__attribute__((__using_if_exists__))`): `<cstdint>`, `<cstring>`,
   and every C wrapper header.
3. **asm labels** (`__asm__("name")` on glibc declarations),
   `__builtin_is_constant_evaluated`, and overloading (glibc's
   C++ `const`/non-`const` pairs of `memchr`, `strchr`, ...):
   `<cstring>`.

Exception specifications (`noexcept`, `noexcept(true)`, `throw()`) no
longer block anything (WP-058). Declarations that used to stop there
now reach later gaps, so some counts rose (`<cstring>`'s overloading
errors went from 2 to 10).

## Noise kiln emits and clang does not

kiln does not yet suppress warnings in system headers. clang does, so
these warnings appear only with kiln:

- `unknown attribute '__type_visibility__' ignored` (100 in `<type_traits>`)
- `unknown attribute '__abi_tag__' ignored`
- `unknown attribute '__exclude_from_explicit_instantiation__' ignored`
- `'__visibility__' attribute ignored`

No header in `bench/pp/headers.cpp` (99 libc++ headers) crashes the
parser. `headers.cpp` as a whole exits 1, with 16 182 errors (13 826
before WP-058: getting past exception specifications exposes more
follow-on errors).
