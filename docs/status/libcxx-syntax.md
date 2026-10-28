# libc++ under `-fsyntax-only`: first blockers

Each header is checked alone, as `#include <h>` in a one-line file:

```
kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0 h.cpp
```

The setup is libc++ 19.1.7 (Debian `libc++-19-dev`) on aarch64 Linux,
with kiln at WP-055 (the parser reads `kiln_pp` tokens). "First
blocker" is the first error kiln reports. The counts are all the errors
in the header's closure; most are follow-ons of the same unsupported
feature. Every header preprocesses identically to clang (see
`pp-parity.md`), so these are all parser or Sema gaps.

| header | exit | errors | first blocker | main error kinds |
|---|---:|---:|---|---|
| `<cstddef>` | 1 | 47 | `__type_traits/enable_if.h:20:1`: templates | templates 38, using-declarations 4 |
| `<cstdint>` | 1 | 28 | `cstdint:161:1`: using-declarations (`using ::int8_t _LIBCPP_USING_IF_EXISTS;`) | using-declarations 28 |
| `<climits>` | 1 | 1 | glibc `bits/pthread_stack_min-dynamic.h:24:40`: exception specifications (`__THROW` is `noexcept(true)`) | exception specifications 1 |
| `<cfloat>` | 0 | 0 | none (clean) | |
| `<cstring>` | 1 | 137 | `__type_traits/is_constant_evaluated.h:21:69`: exception specifications (`_NOEXCEPT`) | exception specifications 81, using-declarations 23, asm labels 20, `__builtin_is_constant_evaluated` undeclared 2, overloading 2 |
| `<version>` | 0 | 0 | none (clean) | |
| `<initializer_list>` | 1 | 50 | `__type_traits/enable_if.h:20:1`: templates | templates 41, using-declarations 4 |
| `<compare>` | 1 | 485 | `__type_traits/enable_if.h:20:1`: templates | templates 284, exception specifications 34, using-declarations 32, special members 12, static data members 11 |
| `<type_traits>` | 1 | 490 | `__fwd/functional.h:20:1`: templates | templates 436, using-declarations 32, special members 5 |

## What unblocks the most

1. **Templates** (M3): `<cstddef>`, `<initializer_list>`, `<compare>`,
   `<type_traits>`, and through them almost every other header.
2. **using-declarations** (`using ::name;`, with
   `__attribute__((__using_if_exists__))`): `<cstdint>`, `<cstring>`,
   and every C wrapper header.
3. **Exception specifications** (`noexcept`, `noexcept(true)`, `throw()`):
   `<climits>` (through glibc's `__THROW`), `<cstring>`.
4. **asm labels** (`__asm__("name")` on glibc declarations) and
   `__builtin_is_constant_evaluated`: `<cstring>`.

## Noise kiln emits and clang does not

kiln does not yet suppress warnings in system headers. clang does, so
these warnings appear only with kiln:

- `unknown attribute '__type_visibility__' ignored` (100 in `<type_traits>`)
- `unknown attribute '__abi_tag__' ignored`
- `unknown attribute '__exclude_from_explicit_instantiation__' ignored`
- `'__visibility__' attribute ignored`

No header in `bench/pp/headers.cpp` (99 libc++ headers) crashes the
parser. `headers.cpp` as a whole exits 1, with 13 826 errors.
