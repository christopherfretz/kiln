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

## WP-1292: member templates, friends, implicit members (2026-11-11)

Debug build of the WP-1292 branch, on top of WP-1283 (#610); same two
purviews and commands as the WP-1283 section. "before" is that
section's "after".

```
allstd, before: 26898 top-level declarations, 18007 carried, 8891 not  (18.8 s, 115 MiB)
allstd, after:  26898 top-level declarations, 18802 carried, 8096 not  (16.5 s, 118 MiB)
stdcxx, before: 25559 top-level declarations, 17535 carried, 8024 not  (19.2 s,  99 MiB)
stdcxx, after:  25559 top-level declarations, 18468 carried, 7091 not  (14.8 s, 101 MiB)
```

With WP-1284 (#617) merged (`IMAGE_VERSION` 17), against WP-1284's
"after":

```
allstd, WP-1284:      26765 top-level declarations, 19044 carried, 7721 not  (18.2 s, 118 MiB)
allstd, both slices:  26765 top-level declarations, 19915 carried, 6850 not  (17.3 s, 119 MiB)
stdcxx, WP-1284:      25333 top-level declarations, 18381 carried, 6952 not  (16.7 s, 100 MiB)
stdcxx, both slices:  25333 top-level declarations, 19452 carried, 5881 not  (15.4 s, 105 MiB)
```

+871 / +1071 over WP-1284. WP-1284's "a member function template"
row (191 / 253, declarations that reached a member template once their
constraints were carried) is gone; what is left of this slice's rows
is "a member template or a template in a template" (1 / 11, partial
specializations of member class templates, `classtempl.rs:144`:
`__function::__callable<_Fp, true>`, `function.h:839`), "a friend template" (15 / 15) and "a
friend declaration in a class template" (5 / 16). The tables below are
from the branch before the merge.

+795 and +933 carried. The slice carries member templates and
templates in templates, redeclared templates with inherited default
arguments, pack expansions in base specifiers, polymorphic class
templates, friends of classes that are not templates (hidden friends
found by ADL among them), and classes whose implicit special members
need overload resolution (spec/modules.md §11.11). It went in three
steps; the middle columns are the census after the first (member
templates) and second (friends, polymorphic class templates, pack
bases). Rows that changed by more than 20 in either purview; "—" is a
row the WP-1283 table does not list:

| allstd | stdcxx | first reason |
|---|---|---|
| 640 → 1 → 1 → 1 | 423 → 9 → 9 → 9 | a member template or a template in a template |
| 180 → 181 → 0 → 0 | 231 → 233 → 0 → 0 | a friend declaration |
| — → 421 → 0 → 0 | — → 253 → 0 → 0 | a declaration in an unsupported context |
| 45 → 45 → 0 → 0 | 44 → 44 → 0 → 0 | a polymorphic class template |
| 45 → 45 → 68 → 0 | 82 → 82 → 118 → 0 | a class whose implicit special members need overload resolution |
| — → 969 → 1206 → 1083 | — → 417 → 469 → 186 | cascade: a member of a class outside the image |
| — → 262 → 443 → 558 | — → 233 → 425 → 640 | cascade: a redeclaration of an entity outside the image |
| 730 → 736 → 678 → 581 | 735 → 749 → 700 → 637 | cascade: a specialization of a template outside the image |
| 631 → 634 → 585 → 475 | 625 → 608 → 547 → 551 | cascade: a type declared outside the image |
| — → 280 → 276 → 339 | — → 315 → 334 → 384 | cascade: a body referring to a declaration outside the image |
| — → 303 → 303 → 297 | — → 206 → 173 → 126 | cascade: a using-declaration naming a declaration outside the image |
| — → 1069 → 1079 → 1091 | — → 706 → 703 → 713 | cascade: an alias template outside the image |
| 40 → 42 → 67 → 73 | 25 → 25 → 49 → 71 | a static assertion in a template |
| 36 → 36 → 61 → 64 | 5 → 5 → 5 → 14 | an out-of-line member of a class nested in a template |
| — → 52 → 91 → 99 | — → 68 → 88 → 104 | mem-initializers of a constructor in a class template |
| — → 0 → 10 → 14 | — → 0 → 7 → 10 | a friend template |
| — → 0 → 3 → 3 | — → 0 → 6 → 13 | a friend declaration in a class template |

Reading:

- "a declaration in an unsupported context" (421 / 253 after the first
  step) was a cascade of friends: out-of-line definitions of members of
  classes refused for their friends (`partial_ordering::less`,
  `__compare/ordering.h:114`, whose class is refused at
  `ordering.h:43`). With friends carried both rows are gone; what
  remains of friends is the friend templates and the friends of class
  templates (14 + 3 / 10 + 13).
- The two largest cascades have concrete roots:
  - "a member of a class outside the image" (1083 / 186): the member
    is carried but its class is not. In `allstd`, 842 of them are
    members of `basic_string` (`string:752`), refused as "a
    redeclaration of an entity outside the image": the pattern's first
    declaration is `template <class _CharT, class _Traits, class
    _Alloc> friend class basic_string;` in class template
    `__wrap_iter` (`__iterator/wrap_iter.h:101`), a friend template.
    Skipping friend redeclarations alone gains only 17, because the
    members then stop on the next missing feature. The other roots
    are attributes (92), a template argument (48, `limits:462`), a
    redeclared partial specialization (48, `vector:1848`) and a
    specialization outside the image (30). `char_traits<char>`, declared
    (`__fwd/string.h:25`) before its definition, is refused as "a class
    member outside its class definition".
  - "an alias template outside the image" (1091 / 713): rooted in
    WP-1284's dependent non-type arguments (`_BoolConstant =
    integral_constant<bool, _Val>`, `integral_constant.h:38`), as in
    the WP-1283 section. It grows a little as more of its users are
    reached.
- "a static assertion in a template", "an out-of-line member of a
  class nested in a template" and "mem-initializers of a constructor in
  a class template" grow because member templates and friends no longer
  stop the walk first; they are WP-1292 follow-ups (see the brief's
  remainder) or WP-1284's.

WP-1036 path tally on this branch, with WP-1284 merged
(docs/status/pch.md "Which path the differential takes" and "Re-run
with a small loop"):

- `cargo xtask test --pch-diff check` over the conformance PCH and
  modules directories, `tests/differential` and `tests/header-guard`:
  141 compared, all equal; 19 took the image and 122 replayed (17 and
  124 before either slice; 60 of the replays are for a template
  argument, 25 for a type, 13 for "a template").
- One header per PCH: libc++ 19, 9 of 106 take the image (`<cassert>`,
  `<cerrno>`, `<cfenv>`, `<cfloat>`, `<ciso646>`, `<climits>`,
  `<cstdbool>`, `<cstdint>`, `<version>`; `<cfenv>` is new); libstdc++
  14, 0 of 109 (73 for the block-scope `terminate` declaration,
  WP-1293, and 36 for a type). The libc++ first refusals moved from "a
  member function template" (13), "a default template argument" (2)
  and "a member template" (1) to the next feature: a template argument
  (53, WP-1300's non-type argument expressions among them), a type
  (16), "a template" (13; variable template specializations) and a
  specialization outside the image (7).

## WP-1307: friend templates, explicit instantiations, variable template specializations, static assertions (2026-11-11)

Debug build of the WP-1307 branch merged with main at WP-1304 (#627;
WP-1300, #626, `IMAGE_VERSION` 20, included); same two purviews and
commands as the WP-1283 section. "before" is the merged main's own
binary (`2f9bf7eae`) on the same purviews, so the deltas are this
slice's alone.

```
allstd, before: 26765 top-level declarations, 21191 carried, 5574 not
allstd, after:  26761 top-level declarations, 21441 carried, 5320 not  (16.5 s, 125 MiB)
stdcxx, before: 25333 top-level declarations, 20650 carried, 4683 not
stdcxx, after:  25287 top-level declarations, 21083 carried, 4204 not  (15.8 s, 108 MiB)
```

+250 / +433 carried (the top-level counts drop a little: explicit
instantiations and the specializations they name are no longer
counted as declarations of their own). The slice carries friend
templates (the `__wrap_iter` shape, a friend class template as the
template's first declaration) and friends declared in class templates,
enumerators of enumerations with a typedef or `bool` underlying type,
explicit instantiation declarations and definitions of class templates
(a section of their own, rerun by the reader), partial and explicit
specializations of variable templates, static assertions in class
templates, out-of-line members of class template partial
specializations, late-parsed default arguments, and specializations
named before their template's definition (spec/modules.md §11.11).
Rows of this slice (before → after; "—" is a row not listed):

| allstd | stdcxx | first reason |
|---|---|---|
| 16 → — | 18 → — | a friend template |
| 5 → — | 17 → — | a friend declaration in a class template |
| 7 → — | 4 → — | an enumerator value the image does not carry |
| 44 → — | 18 → — | an explicit instantiation |
| 16 → — | 50 → — | an unsupported lexical context |
| — → — | — → 38 | cascade: an explicit instantiation in a namespace outside the image |
| 41 → 29 | 105 → 73 | an explicit specialization or instantiation |
| 113 → 27 | 209 → 38 | a template (left: template template parameters) |
| 88 → — | 106 → — | a static assertion in a template |
| 119 → 32 | 110 → 32 | a parameter with an adjusted type or a pack |
| 64 → 17 | 14 → 13 | an out-of-line member of a class nested in a template |
| 148 → 197 | 148 → 206 | mem-initializers of a constructor in a class template |
| 60 → 121 | — → — | an attribute the image does not carry yet |
| 27 → 43 | 77 → 82 | a template expression |
| 1083 → 1040 | 183 → 138 | cascade: a member of a class outside the image |
| 522 → 467 | 288 → 213 | cascade: a body referring to a declaration outside the image |

Rows that grow are declarations newly reached behind a carried one
(their first reason was a cascade before).

Reading:

- `basic_string` (`string:752`, 800-odd member rows of "a member of a
  class outside the image") no longer stops on its friend template: the
  friend `basic_string` in `__wrap_iter` is carried. It now cascades
  from `__wrap_iter` itself, refused for "mem-initializers of a
  constructor in a class template" (`wrap_iter.h:46`), so the next root
  is mem-initializers in class templates (the row grows as more class
  templates are reached).
- "a template" is now only template template parameters
  (`common_reference.h:149`, `is_valid_expansion.h:21`).
- stdcxx's 38 explicit instantiations (`extern template class
  basic_ios<char>;`, `basic_ios.tcc:167`) are in `namespace std`
  reopenings that have no record in the image when the directive is
  written. Why the reopening has none is not established (a reopening
  with the same `__visibility__` attribute and only an `extern
  template` is carried in a reduced test); a follow-up
  (`explinst.rs`, the `lexical` match).
- "a template specialization" (20 / 156) is explicit instantiations of
  function templates (`extern template ostream& __ostream_insert(...)`)
  and explicit function specializations; not carried.
- Uses of a variable template specialization in a non-dependent body
  (`kind<char **>`), and function template calls with explicit
  arguments, are "a template argument list" (15 / 89 declarations).

Bench headers (`bench/pch/{gnu,mid,lcx}.h`, `-fkiln-pch-image -v`),
first refusal, merged main → branch: `gnu` and `mid` "an invalid or
attributed local declaration" → the same; `lcx` "a template" → "a
template expression". All three still replay.

`cargo xtask test --pch-diff check -v` over `cpp.include`, the
`module.*` and `cpp.module` conformance directories,
`tests/differential/pp` and `tests/header-guard/libcxx{,-c}`: 155
compared, all equal, on both binaries; 36 took the image on both, 119
replayed. The headers' first refusals move inward rather than flip:
"a template" 13 → 0 (template template parameters only now), and the
headers that passed it stop on "mem-initializers of a constructor in a
class template" (3 → 9), "a declaration in a template" (30 → 34) or "a
template expression" (16 → 19).

## WP-1310: whole standard headers (2026-11-11)

Debug build of the WP-1310 branch (main at `876dfe167`, IMAGE_VERSION 22);
same two purviews and commands as the WP-1283 section. "before" is the
WP-1307 section's "after" (the same main).

```
allstd, before: 26761 top-level declarations, 21441 carried, 5320 not
allstd, after:  26761 top-level declarations, 21894 carried, 4867 not  (16.7 s, 128 MiB)
stdcxx, before: 25287 top-level declarations, 21083 carried, 4204 not
stdcxx, after:  25287 top-level declarations, 21462 carried, 3825 not  (16.9 s, 111 MiB)
```

+453 / +379 carried. The wave is judged by the units that flip from
replay to the image rather than by declarations, so it took the
cascade roots that stop whole headers: attributed block-scope
declarations, constructor mem-initializers in class templates,
enumerations in class templates, parameters of an adjusted type, the
`__int128_t` / `__uint128_t` / `__builtin_va_list` typedefs, `aligned`
attributes, locations in the predefined buffers, builtin records that
fall inside a class (an "internal image inconsistency" before), asm
labels given on a later redeclaration (glibc's `__REDIRECT`), C
linkage specifications in a namespace (a PCH only) and the
source-location builtins (spec/modules.md §11.11).

Whole headers carried, each built alone as a PCH under
`-fkiln-pch-image -v` (the `pch.md` method):

- libc++ 19 (118 extension-less headers): 17 → 32. New: `<cctype>`,
  `<cinttypes>`, `<clocale>`, `<csetjmp>`, `<csignal>`, `<cstdarg>`,
  `<cstddef>`, `<cstdio>`, `<cstdlib>`, `<cstring>`, `<ctime>`,
  `<cwctype>`, `<initializer_list>`, `<source_location>`, `<typeinfo>`.
- libstdc++ 14 (109 headers): 0 → 29 (`<cassert>` through `<version>`,
  including `<limits>`, `<typeinfo>`, `<initializer_list>`,
  `<source_location>`, `<cstdio>`, `<cwchar>`).
- libc++ header-guard units (`tests/header-guard/libcxx`, 106) with no
  refusal: 11 → 27 (`<cxxabi.h>` among them).

First refusals of the headers still replayed: libc++ "a template
expression" 34, "a defaulted comparison or an out-of-line defaulted
function" 16, "a specialization of a template outside the image" 7;
libstdc++ "a specialization type the image does not carry" 39, "an
explicit specialization using a non-type default argument" 16, "a
redeclaration of an entity outside the image" 12, "an explicit
instantiation in a namespace outside the image" 10.

Bench headers (`bench/pch/{gnu,mid,lcx}.h`), first refusal before →
after: `gnu` "an invalid or attributed local declaration" → "an
explicit specialization using a non-type default argument"; `mid` the
same → the same as `gnu`; `lcx` "a template expression" → the same. All
three still replay; `gnu` still has 3648 of 24591 declarations
refused, so the next waves are needed before a bench case flips.


## WP-1320: PCH image wave 6a, libstdc++ roots (2026-11-11)

Debug builds: "before" is main at `fcd83a424` (IMAGE_VERSION 22), "after"
the WP-1320 branch; same two purviews and commands as the WP-1283
section, both binaries in one session.

```
allstd, before: 26761 top-level declarations, 21894 carried, 4867 not  (18.5 s, 127 MiB)
allstd, after:  26761 top-level declarations, 23358 carried, 3403 not  (22.4 s, 124 MiB)
stdcxx, before: 25287 top-level declarations, 21462 carried, 3825 not  (16.6 s, 112 MiB)
stdcxx, after:  25233 top-level declarations, 22680 carried, 2553 not  (18.0 s, 110 MiB)
```

+1464 / +1218 carried. stdcxx counts 54 fewer top-level declarations
because explicit class instantiations are now listed after the walk
rather than where the walk first meets their specialization. Forms
carried (spec/modules.md §11.11 "Forms carried by WP-1320"): explicit
specializations taking a non-type default, dependent specializations
completed with defaults (and `typename X<T>::type` bases), explicit
instantiations in reopened namespaces, new-expressions and the implicit
global allocation functions (`Implicit` kind 7), pseudo-destructor
calls, explicit template arguments of a declaration reference,
block-scope `static_assert`s, typedefs and aliases, GNU complex types
(type tag 23) and canonical template parameter types (type tag 24).
Rows that changed by 15 or more (before → after):

| allstd | stdcxx | first reason |
|---|---|---|
| 473 → 37 | 358 → 58 | a local declaration the image does not carry |
| 334 → 0 | 206 → 0 | a specialization type the image does not carry |
| 16 → 0 | 89 → 0 | a template argument list |
| — | 159 → 0 | a type the image does not carry |
| 27 → 9 | 33 → 17 | a new or delete expression |
| 107 → 32 | 63 → 21 | a template parameter the image does not carry |
| 547 → 220 | 471 → 280 | cascade: a redeclaration of an entity outside the image |
| 601 → 288 | 604 → 459 | cascade: a specialization of a template outside the image |
| — | 38 → 0 | cascade: an explicit instantiation in a namespace outside the image |
| 522 → 733 | 265 → 189 | cascade: a body referring to a declaration outside the image |
| 59 → 75 | 54 → 72 | an expression in a type naming a parameter or local declaration |
| — | 32 → 70 | a try block |
| — | 87 → 111 | a template expression |

Per header (109 libstdc++ 14 headers, each `#include <h>` alone as a
PCH, `-fkiln-pch-image -fkiln-bmi-image-census`), first refusal per
declaration: declarations in `bench/pch/gnu.h` / headers where it occurs.

| reason | before | after |
|---|---|---|
| a specialization type the image does not carry | 222 / 78 | 0 |
| a local declaration the image does not carry | 383 / 78 | 59 / 64 |
| a type the image does not carry | 159 / 27 | 0 |
| a template argument list | 90 / 68 | 0 |
| a pseudo-destructor call | 7 / 78 | 0 |
| a new or delete expression | 33 / 78 | 17 / 28 |
| an explicit specialization using a non-type default argument | 8 / 67 | 0 |
| a redeclaration of an entity outside the image | 0 / 12 | 0 |
| an explicit instantiation in a namespace outside the image | 0 / 10 | 0 |
| a template parameter the image does not carry | 63 / 78 | 21 / 78 |

Census-clean headers: 29 → 31 (`<new>`, `<iosfwd>`). Every other
header includes `<type_traits>`, whose remaining roots are "a template
expression" (16, WP-1314's), template template parameters ("a
template" / "a template parameter", type_traits:2742, 2750, 2782, 3900),
constrained partial specializations (2752, 3451), a member-template
partial specialization (1856) and a specialization of a partial
specialization (1878); so no further header flips until those land.
`-fkiln-bmi-image-census=all` (new) lists every refused declaration.

## WP-1314: libc++ wave 6b (2026-11-11)

Debug build of the WP-1314 branch alone (base main `2059be16b`); the
tree with WP-1320 merged in is measured last, under "Merged with
WP-1320". IMAGE_VERSION 24 in the last commit (WP-1320 took 23). The
libc++ family is the 106 headers
`bench/pch/lcx.h` includes, each built alone as a PCH with
`-fkiln-pch-image -fkiln-bmi-image-census=all -v`, plus `lcx.h` whole.
`-fkiln-bmi-image-census=all` (taken from WP-1320, byte for byte) lists
every refused declaration, so the table counts every refusal reason,
not only each header's first.

```
lcx.h, before: 26762 top-level declarations, 21884 carried, 4878 not
lcx.h, after:  26762 top-level declarations, 22397 carried, 4365 not
```

+513 carried. Headers through the image: 26 of 106 before and after.
No header flipped: each replayed header has 25 to 45 root reasons, so
carrying the brief's three buckets moves first refusals but no unit.
First refusals of the 80 replayed headers before: a template
expression 31, a defaulted comparison or an out-of-line defaulted
function 15, a specialization of a template outside the image 7, an
expression in a type naming a parameter or local declaration 5, a
type declared outside the image 5, a specialization type 4, 3 or fewer
for the rest. After: a specialization type the image does not carry
52, a specialization of a template outside the image 8 (a cascade: its
roots are the class members of `char_traits<char>`, below), an
expression in a type naming a parameter or local 5, a type declared
outside the image 5, 3 or fewer for the rest. `lcx.h` stopped at "a
template expression" (`is_referenceable.h:34`), then at "an implicit,
invalid or attributed parameter" (`move.h:30`), and now stops at "a
specialization type the image does not carry" (`common_type.h:41`).

Every reason, as declarations in `lcx.h` / headers it appears in,
before and after ("-": absent):

| reason | before | after | example (`/usr/include/c++/v1/`) |
|---|---:|---:|---|
| a specialization type the image does not carry | 334 / 75 | 364 / 75 | __type_traits/common_type.h:41:8 |
| a local declaration the image does not carry | 473 / 75 | - / - | __utility/declval.h:30:56 |
| a template parameter the image does not carry | 107 / 72 | 107 / 72 | __type_traits/common_reference.h:150:8 |
| a template expression | 48 / 72 | - / - | __type_traits/is_referenceable.h:34:8 |
| an implicit, invalid or attributed parameter | 19 / 72 | - / - | __utility/move.h:30:1 |
| a constrained template | 46 / 71 | 46 / 71 | __type_traits/common_reference.h:63:8 |
| a template | 27 / 71 | 27 / 71 | __type_traits/common_reference.h:149:47 |
| an expression in a type naming a parameter or local declaration | 59 / 60 | 116 / 68 | __iterator/iter_move.h:72:54 |
| a defaulted comparison or an out-of-line defaulted function | 11 / 67 | - / - | __compare/ordering.h:62:47 |
| an expression the image does not carry yet | 21 / 66 | 29 / 66 | __type_traits/extent.h:25:29 |
| an explicit specialization argument the image does not carry | 29 / 65 | 29 / 65 | __utility/pair.h:602:8 |
| an attribute the image does not carry yet | 98 / 64 | 98 / 64 | __math/traits.h:88:1 |
| a new or delete expression | 27 / 63 | 38 / 63 | __memory/construct_at.h:38:44 |
| an implicit declaration that is not a special member (CXXRecord) | 6 / 62 | 6 / 62 | __compare/synth_three_way.h:28:65 |
| a template named through a qualifier or parameter | 3 / 57 | 3 / 57 | __type_traits/is_valid_expansion.h:21:67 |
| a type the image does not carry | 6 / 57 | 6 / 57 | __ranges/size.h:82:69 |
| a pseudo-destructor call | 5 / 57 | 5 / 57 | __memory/construct_at.h:65:58 |
| a declaration not named by an identifier | 39 / 56 | 39 / 56 | __ranges/view_interface.h:76:44 |
| a template argument the image does not carry | 46 / 43 | 49 / 54 | __format/range_default_formatter.h:96:29 |
| a partial specialization argument the image does not carry | 2 / 52 | 2 / 52 | __iterator/segmented_iterator.h:72:8 |
| a class member outside its class definition | 6 / 52 | 6 / 52 | __functional/operations.h:51:19 |
| an implicit member of a class template specialization | 11 / 21 | 14 / 52 | __memory/uses_allocator_construction.h:148:76 |
| an implicit declaration that is not a special member (TemplateTypeParm) | 28 / 51 | 28 / 51 | __ranges/access.h:85:19 |
| a local using-declaration or using-directive | - / - | 29 / 51 | __chrono/convert_to_timespec.h:29:40 |
| an expression annotation the image does not carry | 7 / 49 | 7 / 49 | __algorithm/sort.h:938:7 |
| a local class or enumeration | - / - | 6 / 49 | __functional/hash.h:244:32 |
| an out-of-line defaulted function | - / - | 4 / 47 | __bit_reference:62:55 |
| a lambda expression | 17 / 26 | 28 / 43 | optional:565:9 |
| an anonymous union or struct member | 10 / 40 | 10 / 40 | optional:285:3 |
| a template specialization | 20 / 35 | 20 / 35 | __algorithm/sort.h:891:6 |
| an explicit specialization or instantiation | 29 / 34 | 30 / 34 | __fwd/string.h:48:7 |
| a specialization of a partial specialization | 6 / 32 | 7 / 32 | __chrono/hh_mm_ss.h:91:46 |
| a typeid or dynamic_cast expression | 4 / 3 | 7 / 32 | any:177:35 |
| a specialization of a constrained template | 10 / 16 | 12 / 32 | __format/format_context.h:69:7 |
| a try block | 42 / 31 | 43 / 31 | __memory/uninitialized_algorithms.h:59:69 |
| a template argument list | 17 / 30 | - / - | __thread/support/pthread.h:196:35 |
| a member template or a template in a template | 2 / 29 | 2 / 29 | __chrono/duration.h:183:12 |
| an explicitly specialized member | 1 / 26 | 1 / 26 | __locale:1422:32 |
| a redeclared partial specialization | 3 / 21 | 3 / 21 | bitset:607:8 |
| an inherited constructor or assignment operator | 13 / 18 | 13 / 18 | optional:348:8 |
| a coroutine, contract-checked or implicitly defaulted function | 1 / 15 | 1 / 15 | __format/format_arg.h:292:40 |
| a specialization of a template outside the image | 0 / 7 | 0 / 8 |  |
| a union initializer | 2 / 7 | 2 / 7 | __mutex/mutex.h:25:20 |
| a type declared outside the image | 0 / 5 | 0 / 5 |  |
| a specialization named while its user is recorded | 4 / 5 | 4 / 5 | deque:2588:33 |
| a dependent using-declaration | 1 / 3 | 1 / 3 | variant:1132:17 |
| a redeclaration of an entity outside the image | 0 / 1 | 0 / 1 |  |
| a declaration with side data the image does not carry | 4 / 1 | 4 / 1 | __mdspan/extents.h:330:39 |
| a variable template specialization | - / - | 13 / 1 | numbers:139:25 |
| a template expression (a name found through a using-declaration) | - / - | 0 / 1 |  |
| an out-of-line member of a class nested in a template | 17 / 1 | 17 / 1 | __random/binomial_distribution.h:114:46 |

Cascades in `lcx.h`: 3217 before, 3089 after.

Carried (spec/modules.md §11.11, "Forms carried by WP-1314" and
"Defaulted comparisons"): the "a template expression" bucket split by
form; explicit template arguments of a resolved name; block-scope
static assertions; block-scope typedefs and aliases; unresolved names
that found class members, with their naming class (cold annotation
1024); dependent `decltype` bases; parameter attributes of plain-word
kinds (`[[clang::lifetimebound]]`); defaulted comparisons (the
`defcmps` section); a class kept open across a reopened namespace.

Still blocking, by headers it appears in: a specialization type the
image does not carry (75; WP-1320 completes specialization types with
defaults), a template parameter the image does not carry (72), a
constrained template (71), a template (71), an expression in a type
naming a parameter or local (68), an expression the image does not
carry yet (66), an explicit specialization argument (65), an attribute
(64; `preferred_name`, `diagnose_if`, `enable_if`), a new or delete
expression (63; WP-1320 has the global ones), an implicit `CXXRecord`
(62). The out-of-line defaulted rows (47 headers: `tuple:367`,
`tuple:422`, `__bit_reference:62`) are in-class defaulted constructors
the writer sees as not a first declaration or not a special member; a
reduction outside libc++ does not reproduce it.

### Merged with WP-1320

The final tree (WP-1320 merged in, then this branch), same census:

```
lcx.h, merged: 26762 top-level declarations, 23832 carried, 2930 not
```

+1948 carried over the "before" run, +1435 over WP-1314 alone.
Headers through the image: 28 of 106 (`cwchar` and `limits` flip; no
other header changes). `lcx.h` stops at "a constrained template"
(`__type_traits/common_reference.h:63`). First refusals of the 78
replayed headers: a constrained template 44, a specialization of a
template outside the image 9, an expression in a type naming a
parameter or local 6, a type declared outside the image 6, a template
4, 2 or fewer for the rest. Cascades in `lcx.h`: 2073.

Every reason on the merged tree, as declarations in `lcx.h` / headers it
appears in:

| reason | merged | example (`/usr/include/c++/v1/`) |
|---|---:|---|
| a constrained template | 46 / 71 | __type_traits/common_reference.h:63:8 |
| a template | 28 / 71 | __type_traits/common_reference.h:149:47 |
| a template parameter the image does not carry | 32 / 70 | __type_traits/common_reference.h:150:8 |
| an expression in a type naming a parameter or local declaration | 126 / 68 | __iterator/iter_move.h:72:54 |
| an attribute the image does not carry yet | 100 / 67 | __math/traits.h:88:1 |
| an expression the image does not carry yet | 15 / 66 | __type_traits/extent.h:25:29 |
| an explicit specialization argument the image does not carry | 29 / 65 | __utility/pair.h:602:8 |
| an implicit declaration that is not a special member (CXXRecord) | 6 / 62 | __compare/synth_three_way.h:28:65 |
| a template argument the image does not carry | 52 / 59 | __format/range_default_formatter.h:96:29 |
| an expression annotation the image does not carry | 8 / 58 | __iterator/iterator_traits.h:451:8 |
| a type the image does not carry | 5 / 58 | __ranges/size.h:82:69 |
| a declaration with side data the image does not carry | 6 / 57 | __utility/pair.h:171:97 |
| a template named through a qualifier or parameter | 3 / 57 | __type_traits/is_valid_expansion.h:21:67 |
| a declaration not named by an identifier | 39 / 56 | __ranges/view_interface.h:76:44 |
| a local using-declaration or using-directive | 35 / 54 | __memory/compressed_pair.h:152:60 |
| an implicit member of a class template specialization | 23 / 54 | __memory/uses_allocator_construction.h:148:76 |
| a class member outside its class definition | 6 / 52 | __functional/operations.h:51:19 |
| a partial specialization argument the image does not carry | 2 / 52 | __iterator/segmented_iterator.h:72:8 |
| an implicit declaration that is not a special member (TemplateTypeParm) | 28 / 51 | __ranges/access.h:85:19 |
| a local class or enumeration | 6 / 49 | __functional/hash.h:244:32 |
| an out-of-line defaulted function | 5 / 47 | __bit_reference:62:55 |
| a member template or a template in a template | 3 / 45 | __memory/unique_ptr.h:323:10 |
| a lambda expression | 39 / 43 | optional:565:9 |
| an anonymous union or struct member | 11 / 41 | optional:285:3 |
| a new or delete expression | 9 / 39 | __memory/shared_ptr.h:384:8 |
| a try block | 54 / 37 | __memory/uninitialized_algorithms.h:59:69 |
| a template specialization | 20 / 35 | __algorithm/sort.h:891:6 |
| an explicit specialization or instantiation | 30 / 34 | __fwd/string.h:48:7 |
| a specialization of a constrained template | 12 / 32 | __format/format_context.h:69:7 |
| a typeid or dynamic_cast expression | 9 / 32 | any:177:35 |
| an explicitly specialized member | 4 / 26 | __locale:1422:32 |
| a redeclared partial specialization | 3 / 21 | bitset:607:8 |
| a dependent using-declaration | 2 / 19 | __charconv/traits.h:156:29 |
| an inherited constructor or assignment operator | 13 / 18 | optional:348:8 |
| a specialization of a partial specialization | 9 / 15 | __chrono/hh_mm_ss.h:91:46 |
| a coroutine, contract-checked or implicitly defaulted function | 1 / 15 | __format/format_arg.h:292:40 |
| a specialization of a template outside the image | 0 / 9 |  |
| a union initializer | 2 / 7 | __mutex/mutex.h:25:20 |
| a deduced template named through a qualifier or parameter | 0 / 7 |  |
| a specialization named while its user is recorded | 5 / 6 | deque:2588:33 |
| a type declared outside the image | 0 / 6 |  |
| a local structured binding | 1 / 2 | __algorithm/ranges_find.h:43:3 |
| an out-of-line member of a class nested in a template | 17 / 1 | __random/binomial_distribution.h:114:46 |
| a variable template specialization | 13 / 1 | numbers:139:25 |
| a template expression (a name found through a using-declaration) | 0 / 1 |  |

## WP-1342: PCH image wave 7a, template declarations (2026-11-12)

Debug build of the WP-1342 branch (base main `bdf052a17`, IMAGE_VERSION
24 there). Method as in the WP-1314 section: `bench/pch/lcx.h` (libc++
19) and `bench/pch/gnu.h` (libstdc++ 14) whole, plus each of their
headers alone (106 libc++, 109 libstdc++), every one with
`-fkiln-pch-image -fkiln-bmi-image-census=all -v`. The bucket: template
declarations. "a template parameter the image does not carry" was
mostly a cascade (a parameter refused for another reason, its template
then refused too); it is now "a template parameter outside the image",
counted as a cascade, and the template template parameters it also
counted are carried.

My reasons, as declarations in the whole header / headers they appear
in, before and after ("-": absent):

| reason | lcx before | lcx after | gnu before | gnu after |
|---|---:|---:|---:|---:|
| a constrained template | 46 / 71 | - | 52 / 78 | - |
| a template | 28 / 71 | - | 39 / 78 | - |
| a template parameter the image does not carry | 32 / 70 | - | 21 / 78 | - |
| a member template or a template in a template | 3 / 45 | - | 18 / 78 | - |
| a template named through a qualifier or parameter | 3 / 57 | - | 1 / 51 | - |
| a deduced template named through a qualifier or parameter | 0 / 7 | - | 15 / 54 | - |
| a template expression (a name found through a using-declaration) | 0 / 1 | - | 37 / 27 | - |
| cascade: a template parameter outside the image (new name) | - | 17 / 52 | - | 3 / 52 |

Carried forms (spec/modules.md §11.11, "Forms carried by WP-1342"):
template template parameters (record kind 27) and template-name
arguments, dependent template names as arguments (`__xref<_Tp>::template
__apply`, `common_reference.h:153`), constrained class and variable
templates and constrained partial specializations, partial
specializations of member templates (of classes and of class templates,
`type_traits:1856`, `alloc_traits.h:69`, `stl_iterator.h:2283`),
qualified and template-template-parameter class template deduction
(`ranges_util.h:749`, `__ranges/to.h:161`, `ranges:9470`), and names
found through using-declarations in template bodies (the tr1 special
functions, `cmath:3762`, `format:1397`).

```
lcx.h, before: 26762 top-level declarations, 23832 carried, 2930 not
lcx.h, after:  26762 top-level declarations, 23837 carried, 2925 not
gnu.h, before: 26396 top-level declarations, 23756 carried, 2640 not
gnu.h, after:  26396 top-level declarations, 24296 carried, 2100 not
```

Headers through the image: libc++ 28 of 106 before and after,
libstdc++ 31 of 109 before and after. No header flipped: every
replayed header still has reasons of the sibling buckets. The libc++
count moves little because carrying a constrained template exposes
each of its specializations ("a specialization of a constrained
template", 12 / 32 before, 66 / 36 after; WP-1344) where the template's
own refusal had hidden them. First refusal of `lcx.h`: "a constrained
template" (`common_reference.h:63`) before, "an expression the image
does not carry yet" (`__type_traits/extent.h:25`) after. `gnu.h`: "a
dependent using-declaration" (`ext/numeric_traits.h:54`) before and
after. Per libstdc++ header, the first refusal "a member template or a
template in a template" (58 headers, `type_traits:1856`) became "a
template argument the image does not carry" (58, `type_traits:1878`: a
specialization `_List<unsigned char, ...>` with a pack argument, WP-1344).

What remains is other buckets' (and the cascade above, whose roots are
defaults and types refused for their own reasons, e.g.
`__algorithm/unwrap_iter.h:77`): "a specialization of a constrained
template" (66 / 36 lcx, 10 / 38 gnu; `system_error:167`,
`ranges:8828`), the pack-argument specialization at `type_traits:1878`,
"a partial specialization argument the image does not carry"
(`aligned_union.h:32`).
