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

## WP-1278: attributes, reserved names and scopes (2026-11-11)

The same purview and command, at the WP-1278 branch (debug build): wall
17.7 s, peak RSS 114 MiB, the same 16 errors.

```
before (WP-1070): 25544 top-level declarations, 10938 carried, 14606 not
after (WP-1278):  26898 top-level declarations, 16775 carried, 10123 not
```

With WP-1279 merged as well: 26898 top-level declarations, 17680
carried, 9218 not. The rows below are WP-1278's alone.

There are more top-level declarations because the linkage
specifications and namespaces `std` that were refused whole are now
scopes, so their members count on their own. The rows WP-1278 targets:

| reason | before | after |
|---|---:|---:|
| a declaration with attributes | 4020 | 0 |
| a reserved identifier (`__x`, `_X`) | 2619 | 0 |
| an implicit declaration that is not a special member (LinkageSpec) | 263 | 0 |
| a linkage specification | 27 | 0 |
| a typedef naming an unnamed class | 18 | 0 |
| a declaration named `std` or `main` | 13 | 0 |
| an inline or unnamed namespace | 10 | 0 |
| cascade: a using-declaration naming a declaration outside the image | 848 | 303 |
| cascade: a type declared outside the image | 859 | 743 |

The rows that replace them are narrower:

- "an attribute the image does not carry yet" (41): `preferred_name`
  (18, which names a type), `diagnose_if` (12), `aligned` (6),
  `alignas` (3) and `enable_if` (2);
- "an exception specification the image does not carry" (63), "a
  declaration with side data the image does not carry" (37), "a
  parameter with an adjusted type or a pack" (36), "a declaration not
  named by an identifier" (30) and "a C linkage specification in a
  namespace" (2) are first reasons of declarations that the removed
  rows used to stop earlier.

The template rows grew (for example "a declaration in a template" 1595
to 1686, "cascade: a specialization of a template outside the image"
199 to 655, "a type the image does not carry" 38 to 368) because the
declarations now reach the template checks. They are WP-1279's. Of the
type kinds refused as "a type the image does not carry", all but about
17 are dependent (a dependent name 618, a pack expansion 215,
`auto` 167, `decltype` 142, a dependent-size array 61, a unary
transform 28; counted per attempt). The non-dependent ones are
`decltype` (6), `auto` (4), vector types (4) and member pointers (3).
All 116 of "a declaration the image does not carry yet" are the body
of a requires-expression (`RequiresExprBody`), also WP-1279's.

## Re-running

The purview file is generated as described in "Setup"; the census
prints on stderr after the compile's diagnostics. The image itself, and
the module file when the unit is valid, are the same with and without
`-fkiln-bmi-image-census` (`kind2_image_census` in
`crates/kiln_driver/tests/modules_bmi.rs`).

## WP-1279: templates, S4 third slice (2026-11-11)

Same setup and command as above, debug build of the WP-1279 branch;
wall 14.7 s, peak RSS 104 MiB. The slice carries template parameter
packs (and function parameter packs, pack expansions, `sizeof...`,
folds), default arguments of non-type parameters, dependent `auto`
return types, alias templates, variable templates and unresolved
names in patterns (spec/modules.md §11.11 "Templates as patterns",
"Third slice").

image census for 'allstd': 25544 top-level declarations, **11565
carried** (was 10938, +627), 13979 not

Rows that changed (before, after; reasons new in this slice are the
triage of "a declaration in a template" and the alias-template
cascade):

| before | after | first reason |
|---:|---:|---|
| 1595 | 0 | a declaration in a template |
| 0 | 929 | cascade: a member of a class outside the image |
| 70 | 736 | an explicit or partial specialization |
| 0 | 652 | cascade: an alias template outside the image |
| 584 | 0 | a template parameter pack |
| 384 | 0 | a default template argument of a non-type parameter |
| 353 | 0 | an alias template |
| 673 | 439 | a template |
| 414 | 589 | a member template or a template in a template |
| 217 | 86 | a default template argument |
| 0 | 99 | a template expression |
| 859 | 919 | cascade: a type declared outside the image |
| 38 | 71 | a type the image does not carry |
| 199 | 229 | cascade: a specialization of a template outside the image |
| 30 | 21 | a template parameter the image does not carry |
| 384 | 390 | a constrained template parameter |
| 0 | 2 | a requires-expression or concept |

Reading:

- "a declaration in a template" (1595) was mostly two things: partial
  specializations (666 of it) and members of class templates the image
  refused for another reason (929, now a cascade). The rest moved to
  their own rows once the template itself was carried.
- A row that grows is a declaration whose first refusal used to be a
  pack, a non-type default or an alias template and is now the next
  missing feature: member templates (+175), dependent unresolved names
  of forms not carried yet (99), dependent types holding an expression
  (+33).
- "a template" (439 left) is now deduction-guide templates (211, e.g.
  `valarray:947:1`), concepts (199, `__concepts/arithmetic.h:30:9`),
  template template parameters (25) and 4 others.
- Most of libc++'s templates are still refused first for their
  attributes (`_LIBCPP_HIDE_FROM_ABI`, `_LIBCPP_TEMPLATE_VIS`: WP-1278)
  or reserved names (the S5 name gate). To measure this slice with
  attributes and reserved names counted as "first reason elsewhere"
  (as the brief asks), a local, uncommitted build that skips those two
  checks gives, over the 25335 declarations it then counts: 12116
  carried at the base, 12966 carried with this slice (+850).

## WP-1283: class template specializations (2026-11-11)

Debug build of the WP-1283 branch, on top of WP-1279. Two purviews:

- `allstd` as in "Setup" (libc++ 19.1.7);
- `stdcxx`: `export module stdcxx; export { #include <bits/stdc++.h> }`
  against libstdc++ 14 (`/usr/include/c++/14`), command as above with
  `-stdlib=libstdc++` (20 errors; the purview is no more a valid unit
  than `allstd`).

```
allstd, before: 26898 top-level declarations, 17680 carried, 9218 not  (17.7 s, 114 MiB)
allstd, after:  26898 top-level declarations, 18007 carried, 8891 not  (18.8 s, 115 MiB)
stdcxx, before: 25559 top-level declarations, 17292 carried, 8267 not  (15.7 s,  99 MiB)
stdcxx, after:  25559 top-level declarations, 17535 carried, 8024 not  (19.2 s,  99 MiB)
```

+327 and +243 carried. The slice carries explicit and partial class
template specializations, specializations in reopened namespaces and by
qualified names, literal arguments, and declarator locations
(spec/modules.md §11.11 "Class template specializations"). Rows that
changed by more than 20 in either purview (allstd before, after;
stdcxx before, after):

| allstd | stdcxx | first reason |
|---|---|---|
| 933 → 0 | 1128 → 0 | an explicit or partial specialization |
| 589 → 730 | 486 → 735 | cascade: a specialization of a template outside the image |
| 750 → 631 | 595 → 625 | cascade: a type declared outside the image |
| 380 → 468 | 622 → 715 | a type the image does not carry |
| 449 → 521 | 474 → 570 | a template |
| 198 → 287 | 43 → 68 | a template parameter the image does not carry |
| 160 → 201 | 135 → 124 | a template argument the image does not carry |
| 28 → 70 | 22 → 71 | a constrained template |
| 92 → 114 | 86 → 131 | a member function template |
| 631 → 640 | 370 → 423 | a member template or a template in a template |
| 37 → 45 | 56 → 82 | a class whose implicit special members need overload resolution |
| 0 → 44 | 0 → 16 | an explicit instantiation |
| 0 → 40 | 0 → 25 | a static assertion in a template |
| 0 → 36 | 0 → 5 | an out-of-line member of a class nested in a template |
| 0 → 34 | 0 → 20 | an explicit specialization argument the image does not carry |
| 23 → 24 | 1 → 30 | an implicit member of a class template specialization |

Reading:

- Most of the 933 / 1128 now reach the next missing feature rather than
  being carried: the specialized template itself is refused (the
  specialization cascade, +141 / +249), or the specialization's body
  needs a type, template argument or member template the image does
  not carry. The largest roots are WP-1284's: dependent expressions as
  non-type arguments (`_BoolConstant = integral_constant<bool, _Val>`,
  `integral_constant.h:38`, roots most of the 923 alias-template
  cascade) and constrained templates.
- "a static assertion in a template" is new and deliberate: the image
  used to drop such assertions silently (spec/modules.md, "Static
  assertions in templates").
- "a template" is now mostly variable template specializations
  (`body.rs` refuses them), deduction guides and concepts.
- Not done in this slice and unchanged: member templates (640 / 423),
  friends (180 / 231), polymorphic class templates (45 / 44), implicit
  special members needing overload resolution (45 / 82).

## WP-1284: constraints, concepts, dependent names, deduction guides (2026-11-11)

Debug build of the WP-1284 branch with WP-1283 merged (`IMAGE_VERSION`
16), same two purviews and commands. "Before" is the WP-1283 "before"
(neither slice); "WP-1284 alone" is the branch before the merge.

```
allstd, before:        26898 top-level declarations, 17680 carried, 9218 not  (17.6 s, 114 MiB)
allstd, WP-1284 alone: 26765 top-level declarations, 18599 carried, 8166 not  (17.4 s, 115 MiB)
allstd, after:         26765 top-level declarations, 19044 carried, 7721 not  (18.2 s, 118 MiB)
stdcxx, before:        25559 top-level declarations, 17292 carried, 8267 not  (15.6 s,  98 MiB)
stdcxx, WP-1284 alone: 25333 top-level declarations, 18105 carried, 7228 not  (15.5 s, 101 MiB)
stdcxx, after:         25333 top-level declarations, 18381 carried, 6952 not  (16.7 s, 100 MiB)
```

+919 / +813 carried for WP-1284 alone, +1037 / +846 over WP-1283's
"after". The top-level totals drop by 133 / 226 because implicit
deduction guides (formed again by the importer) and requires-expression
bodies (carried with their expression) are no longer counted as
declarations. The slice carries concepts, type-constraints,
requires-clauses (template heads and trailing), requires-expressions,
dependent types (`DepTy`: dependent names, template specializations,
transforms, member pointers, vectors, deduced placeholders), the
`Identifier` qualifier step, `DependentScopeDeclRefExpr`, the
`template` keyword in dependent names, written locations of dependent
names, and explicit deduction guides (spec/modules.md §11.11
"Constraints", "Dependent types and names", "Deduction guides"). Rows
that changed by 20 or more in either purview, before to after (both
slices):

| allstd | stdcxx | first reason |
|---|---|---|
| 465 → 0 | 367 → 0 | a constrained template parameter |
| 449 → 111 | 474 → 209 | a template (concepts, guides) |
| 181 → 9 | 174 → 30 | a template expression |
| 116 → 0 | 177 → 1 | a declaration the image does not carry yet |
| 18 → 0 | 53 → 0 | an implicit declaration that is not a special member (guides) |
| 16 → 0 | 9 → 0 | a requires-expression or concept |
| 380 → 142 | 622 → 254 | a type the image does not carry |
| 160 → 326 | 135 → 286 | a template argument the image does not carry |
| 92 → 191 | 86 → 253 | a member function template |
| 28 → 42 | 22 → 45 | a constrained template |
| 889 → 891 | 657 → 622 | cascade: an alias template outside the image |

Reading:

- The largest remaining root is "a template argument the image does
  not carry": an expression as a non-type template argument
  (`integral_constant.h:38`, `_BoolConstant`), which also roots most of
  the alias-template cascade. Expression-bearing types (`decltype`,
  `noexcept(expr)`, dependent array and vector sizes) are the same gap.
  Neither is in this slice.
- "a member function template" and "a constrained template" grew
  because declarations that used to stop at a constraint or a
  dependent type now reach them; a reference to a constrained template
  from a carried body still falls back (spec/modules.md "Constraints").
- Member templates (674 / 446) and friends (181 / 231) are unchanged in
  kind.
