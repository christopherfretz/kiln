# libc++ under `-fsyntax-only`: first blockers

Each header is checked alone, as `#include <h>` in a one-line file:

```
kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0 h.cpp
```

The setup is libc++ 19.1.7 (Debian `libc++-19-dev`) on aarch64 Linux,
with kiln at WP-064 (expression and declarator parsing: tentative
parsing, `decltype`, functional casts, `throw`, `typeid`, GNU
statement expressions, `__builtin_offsetof`, compound literals;
previously WP-062). "First
blocker" is the first error kiln reports. The counts are all the errors
in the header's closure; most are follow-ons of the same unsupported
feature. Every header preprocesses identically to clang (see
`pp-parity.md`), so these are all parser or Sema gaps.

| header | exit | errors | was (WP-062) | first blocker | main error kinds |
|---|---:|---:|---:|---|---|
| `<cstddef>` | 1 | 42 | 43 | `__type_traits/enable_if.h:20:1`: templates | templates 38, special members 1, template-id follow-ons 3 |
| `<cstdint>` | 0 | 0 | 0 | none (clean since WP-062; one `__type_visibility__` warning, see below) | |
| `<climits>` | 0 | 0 | 0 | none (clean; was blocked by glibc's `__THROW`, `noexcept(true)`, before WP-058) | |
| `<cfloat>` | 0 | 0 | 0 | none (clean) | |
| `<cstring>` | 1 | 32 | 33 | `__type_traits/is_constant_evaluated.h:22:10`: `__builtin_is_constant_evaluated` undeclared | asm labels 20, overloading 10, `__builtin_is_constant_evaluated` undeclared 2 |
| `<version>` | 0 | 0 | 0 | none (clean) | |
| `<initializer_list>` | 1 | 45 | 46 | `__type_traits/enable_if.h:20:1`: templates | templates 41, special members 1 |
| `<compare>` | 1 | 418 | 419 | `__type_traits/enable_if.h:20:1`: templates | templates 284, undeclared identifiers 25, overloading 14, special members 12, static data members 11 |
| `<type_traits>` | 1 | 455 | 456 | `__fwd/functional.h:20:1`: templates | templates 436, special members 5 |

No header reports a using-declaration error any more (WP-062): the C
wrapper headers' `using ::name _LIBCPP_USING_IF_EXISTS;` lists resolve,
and a name glibc does not declare becomes an unresolved
`using_if_exists` declaration that is diagnosed only when used, as in
clang.

## What unblocks the most

1. **Templates** (M3): `<cstddef>`, `<initializer_list>`, `<compare>`,
   `<type_traits>`, and through them almost every other header.
2. **asm labels** (`__asm__("name")` on glibc declarations),
   `__builtin_is_constant_evaluated`, and overloading (glibc's
   C++ `const`/non-`const` pairs of `memchr`, `strchr`, ...):
   `<cstring>`.
3. **Class members** (special member functions, static data members,
   default member initializers): the largest non-template error kinds
   across `bench/pp/headers.cpp`.

using-declarations (WP-062) no longer block anything: `<cstdint>` is
clean, and the using-declaration errors of the other headers are gone.
Exception specifications (`noexcept`, `noexcept(true)`, `throw()`) no
longer block anything (WP-058). Declarations that used to stop there
now reach later gaps, so some counts rose (`<cstring>`'s overloading
errors went from 2 to 10).

## Noise kiln emits and clang does not

kiln does not yet suppress warnings in system headers. clang does, so
these warnings appear only with kiln:

- `unknown attribute '__type_visibility__' ignored` (100 in `<type_traits>`,
  the only diagnostic left in `<cstdint>`)
- `unknown attribute '__abi_tag__' ignored`
- `unknown attribute '__exclude_from_explicit_instantiation__' ignored`
- `'__visibility__' attribute ignored`

No header in `bench/pp/headers.cpp` (99 libc++ headers) crashes the
parser. `headers.cpp` as a whole exits 1, with 15 566 errors (15 632 at
WP-062, 16 182 at WP-058, 13 826 before it: getting past exception
specifications exposed more follow-on errors; WP-062's
using-declarations removed some).

WP-064 removed every `decltype` error (one per header above: libc++'s
`typedef decltype(nullptr) nullptr_t;`); none of its
constructs (statement expressions, `__builtin_offsetof`, `throw`,
`typeid`, compound literals) is reported unsupported in any header. The
remaining expression-level errors are follow-ons of unparsed
template-ids: `expected '(' for function-style cast or type
construction` (22, e.g. `numeric_limits<unsigned>::digits`) and
`unexpected type name 'T': expected expression` (8, a type name inside
a template argument list).
