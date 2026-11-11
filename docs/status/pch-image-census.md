# Declaration image fallback census: every libc++ header in a purview (WP-1070, 2026-11-10)

How much of the standard library the declaration image (spec/modules.md
§11.11 "Part 2a") carries once locations in other files and in macro
expansions are carried (stage S7, WP-1070), counted with
`-fkiln-bmi-image-census` (spec/modules.md §11.11 "Fallback census").
This is the first measurement for the real PCH: a header that a PCH
or a header unit holds is a purview like this one.

## Setup

- kiln at the WP-1070 branch, debug build (`cargo build -p kiln_driver`).
- libc++ 19.1.7 (`/usr/include/c++/v1`), aarch64 Linux, `-std=c++26`.
- `allstd.cppm` is `export module allstd; export { ... }` with one
  `#include <h>` per header, in alphabetical order: the 105 public
  libc++ 19 headers without an extension (`ls /usr/include/c++/v1 |
  grep -v '^__\|\.'`, less `experimental` and `ext`) and
  `<stdatomic.h>`, 106 in all.

```
kiln++ -std=c++26 -stdlib=libc++ -fkiln-bmi-image -fkiln-bmi-image-census \
  --precompile allstd.cppm -o allstd.pcm
```

Wall 11.7 s, peak RSS 107 MiB (debug build, census included).

The purview is not a valid module unit, and the census is not a claim
that it should compile: the C library headers it pulls in declare
entities with internal linkage that the `export` block exports, and
redeclare global-module entities (`operator new`, `_IO_FILE`) in the
module. kiln reports 16 errors and writes no module file; clang 19.1.7
and clang 22.1.8 reject the same command (without the kiln flags) with
20 errors (0.59 s / 165 MiB and 0.69 s / 161 MiB). The census is
computed from the parsed purview regardless, which is what it is for:
each top-level declaration is counted under the first reason the image
refuses it.

## Result

Reasons in the order of the census output (most frequent first). The
example is the first refused declaration of that reason in source
order; paths are relative to `/usr/include/c++/v1` unless absolute, and
`<resource>` is kiln's resource directory. A `cascade:` reason is the
consequence of an earlier refusal (the declaration names something not
in the image).

image census for 'allstd': 25544 top-level declarations, 10938 carried, 14606 not

| count | first reason | first example |
|---:|---|---|
| 4020 | a declaration with attributes | `__type_traits/desugars_to.h:18:1` |
| 2619 | a reserved identifier (`__x`, `_X`) | `__type_traits/desugars_to.h:18:1` |
| 1595 | a declaration in a template | `__algorithm/comp.h:29:19` |
| 859 | cascade: a type declared outside the image | `<resource>/include/__stddef_max_align_t.h:24:3` |
| 848 | cascade: a using-declaration naming a declaration outside the image | `cstddef:58:9` |
| 673 | a template | `__type_traits/integral_constant.h:42:7` |
| 584 | a template parameter pack | `__type_traits/desugars_to.h:35:59` |
| 414 | a member template or a template in a template | `__type_traits/common_reference.h:149:40` |
| 384 | a default template argument of a non-type parameter | `cstddef:104:76` |
| 384 | a constrained template parameter | `__iterator/incrementable_traits.h:47:40` |
| 381 | a declaration in an unsupported context | `__compare/ordering.h:114:53` |
| 353 | an alias template | `__type_traits/is_same.h:22:29` |
| 263 | an implicit declaration that is not a special member (LinkageSpec) | `__memory/addressof.h:23:10` |
| 217 | a default template argument | `__utility/swap.h:47:76` |
| 199 | cascade: a specialization of a template outside the image | `__type_traits/is_floating_point.h:30:29` |
| 177 | a friend declaration | `__compare/ordering.h:43:7` |
| 115 | a declaration the image does not carry yet | `__concepts/convertible_to.h:27:58` |
| 89 | cascade: a redeclaration of an entity outside the image | `__utility/swap.h:33:7` |
| 70 | an explicit or partial specialization | `__type_traits/common_type.h:73:29` |
| 50 | a template argument the image does not carry | `__type_traits/integral_constant.h:34:39` |
| 38 | a type the image does not carry | `stddef.h:41:27` |
| 37 | an explicit specialization or instantiation | `__fwd/string.h:45:7` |
| 30 | a template parameter the image does not carry | `__type_traits/common_type.h:59:29` |
| 27 | a linkage specification | `/usr/include/ctype.h:28:1` |
| 22 | an implicit declaration that is not a special member (TemplateTypeParm) | `__ranges/dangling.h:29:44` |
| 20 | cascade: a body referring to a declaration outside the image | `optional:274:28` |
| 18 | a typedef naming an unnamed class | `<resource>/include/__stddef_max_align_t.h:19:9` |
| 18 | a redeclared template | `__type_traits/common_reference.h:187:8` |
| 18 | an implicit declaration that is not a special member (CXXDeductionGuide) | `__utility/pair.h:81:25` |
| 13 | a declaration named `std` or `main` | `cstddef:69:11` |
| 13 | a template argument list | `numbers:139:25` |
| 12 | a class whose implicit special members need overload resolution | `__utility/pair.h:65:29` |
| 10 | an inline or unnamed namespace | `__chrono/duration.h:482:18` |
| 10 | a polymorphic class template | `__locale:174:28` |
| 10 | a specialization type the image does not carry | `locale:593:31` |
| 6 | an implicit declaration that is not a special member (CXXRecord) | `__compare/synth_three_way.h:28:65` |
| 4 | a constrained template | `__ranges/enable_view.h:34:7` |
| 2 | a location in a predefined buffer (<built-in>, <command line>) | `new:348:25` |
| 1 | a template named through a qualifier or parameter | `__type_traits/is_valid_expansion.h:21:67` |
| 1 | an implicit declaration that is not a special member (TypeAlias) | `TypeAlias ''` |
| 1 | an enumerator value the image does not carry | `__fwd/subrange.h:27:35` |
| 1 | cascade: an implicit special member of a class outside the image | `CXXConstructor 'std::__va_list::__va_list'` |

| count | attribute |
|---:|---|
| 3031 | `visibility` |
| 2684 | `abi_tag` |
| 2678 | `exclude_from_explicit_instantiation` |
| 869 | `type_visibility` |
| 230 | `nodiscard` |
| 38 | `deprecated` |
| 37 | `nodebug` |
| 32 | `noreturn` |
| 25 | `always_inline` |
| 18 | `(unnamed)` |
| 12 | `diagnose_if` |
| 11 | `no_sanitize` |
| 8 | `no_unique_address` |
| 5 | `noinline` |
| 4 | `const` |
| 4 | `format` |
| 4 | `malloc` |
| 4 | `returns_nonnull` |
| 4 | `standalone_debug` |
| 2 | `aligned` |
| 2 | `enable_if` |
| 2 | `packed` |
| 1 | `alignas` |

The second table counts, for the declarations refused for their
attributes, how many carry each attribute (a declaration counts once
per attribute name; `(unnamed)` is an attribute kiln keeps without a
spelled name).

## Reading

- Locations no longer block anything material: the only location
  fallback left is 2 declarations whose location is in `<built-in>`
  (`hardware_destructive_interference_size`, through
  `__GCC_DESTRUCTIVE_SIZE`). Before S7 the whole unit fell back at its
  first location outside the main file.
- The largest single cause is attributes, nearly all of them from
  `_LIBCPP_HIDE_FROM_ABI` (`visibility`, `abi_tag`,
  `exclude_from_explicit_instantiation`) and `_LIBCPP_TEMPLATE_VIS`
  (`type_visibility`). Carrying these four attributes is the next step
  with the most effect.
- Reserved identifiers (`__x`, `_X`) are refused by the name gate,
  which guards the importer's preprocessor against clashes; it goes
  when S5 replaces the gate with ODR merging.
- Templates (the template, pack, constraint, default argument, alias
  template and member template rows and "a declaration in a template",
  about 4.6 thousand together) are the S4 follow-ups.
- The cascade rows (about 2 thousand) mostly disappear with their
  causes: `max_align_t`'s unnamed-class typedef, and the
  using-declarations of `cstddef`, `cstdio` and the other C wrappers,
  which name C library declarations the image refused first.

## Re-running

The purview file is generated as described in "Setup"; the census
prints on stderr after the compile's diagnostics. The image itself, and
the module file when the unit is valid, are the same with and without
`-fkiln-bmi-image-census` (`kind2_image_census` in
`crates/kiln_driver/tests/modules_bmi.rs`).
