# GCC C++ testsuite (`g++.dg`)

GCC 16.1's C++ tests (`g++.dg` with `g++.dg/torture`, `c-c++-common` with
its torture directory, `g++.old-deja`) through kiln++, with GCC 16.1 and
clang 22 as oracles. Informational, never in `ci`. Run it with

```
scripts/fetch-gcc-tests.sh            # once; about 160 MB under .oss/gcc-tests
cargo xtask gxx-dg -j 4 --report docs/status/gxx-dg.md
```

The rules (suites, standards, directives, how a diagnostic is matched, the
outcome classes and triage buckets) are in `spec/harness.md`, "The g++.dg
runner". In short:

- **Standards** follow `g++-dg.exp`'s `g++-std-flags`: a test that names a
  `-std=` runs once as written; otherwise it runs at up to three of
  20, 98, 26 (then 17, 11, 14, 23), raised to the lowest standard its
  `dg-do` / `dg-require-effective-target` selectors and `c++NN` directives
  allow, as `c++NN` without `dg-options` and `gnu++NN` with them. Torture
  tests run at `-O0` and `-O2` with the default standard (`gnu++20`).
- **Diagnostics** are compared by line and severity only, never by
  message text: an expected error missed, or an error on a line nothing
  expects, is `wrong-line`.
- **libstdc++** is GCC 16.1's for all three compilers (kiln and clang get
  `--gcc-toolchain=` of the GCC oracle), since the tests were written
  against it.
- **Not applicable**: GCC-internal checks (`scan-assembler`, tree and RTL
  dumps, `dg-output`) are never compared; tests in directories their own
  `.exp` drives (modules, gomp, lto, debug, the sanitizers, ...) and tests
  whose `dg-do` target or effective target does not hold here are skip
  rows. The counts are in the generated region below.

## Triage (2026-11-16, kiln `9eb066486`, aarch64 and x86-64)

Hand-written from the run whose generated region follows (the second
full run, WP-1473), compared case by case with the first (2026-11-15,
kiln `604edce44`, WP-1443) by `scripts/gxx-dg-compare.py`. A "case" is
one test at one standard (or one optimization level). Each cluster
gives its case count (tests in parentheses), a few lines of my own that
show the pattern, and the part of kiln the fix probably lives in.

**49496 of 56766 applicable cases pass (87.2%)** on aarch64, against
47950 of 56766 (84.5%) in the first run; on x86-64 49708 of 57070
(87.1%, see "x86-64" below). Same 20571 tests, same 56766 cases and
786 skip rows on aarch64.

| class | first run | second run | x86-64 |
|---|---:|---:|---:|
| pass | 47950 | 49496 | 49708 |
| compile-reject | 3775 | 2875 | 2943 |
| accept-invalid | 1472 | 1196 | 1205 |
| wrong-line | 3277 | 3049 | 3052 |
| wrong | 272 | 144 | 153 |
| crash | 17 | 3 | 6 |
| timeout | 3 | 3 | 3 |

| bucket | first run | second run | x86-64 |
|---|---:|---:|---:|
| kiln crash / hang | 20 | 6 | 9 |
| wrong-code | 272 | 144 | 153 |
| rejects-valid, clang accepts | 997 | 355 | 358 |
| rejects-valid, clang rejects too | 2778 | 2520 | 2585 |
| accepts-invalid, clang rejects | 922 | 638 | 644 |
| accepts-invalid, clang accepts too | 550 | 558 | 561 |
| wrong-line, clang matches | 786 | 686 | 686 |
| wrong-line, clang differs too | 2491 | 2363 | 2366 |

By case, first run against second (aarch64): 1645 cases went from a
failing class to pass, 99 from pass to a failing class, 77 between two
failing classes; 47851 passed in both.

### What the fix rows closed

One line per row that landed between the two runs (each re-ran only its
own subset; their BACKLOG rows have the detail).

- WP-1452: triviality traits from eligible special members.
- WP-1455: an out-of-line constructor whose parameter names an enclosing class.
- WP-1456: all six crash/hang clusters (array size overflow, inline-asm operands, `_Float128`/`__int128` conversions, recursive satisfaction depth, `this` in a constexpr temporary's NSDMI, non-`int` `main`).
- WP-1457: dynamic exception specifications at run time before C++17; throwing C++98 destructors.
- WP-1458: cleanups while an exception is in flight (aggregates, arrays, temporaries, `delete`).
- WP-1459: lifetime extension through `.*`, casts and `?:`; CWG2867; `-f[no-]range-for-ext-temps`.
- WP-1460: C-style casts to a base, construction vtables in base initializers, `new T[n]` bound checks.
- WP-1461: member partial specializations with non-type arguments, `T::template B`, enum underlying types, array RTTI names, array copy counts.
- WP-1462, WP-1482, WP-1489, WP-1490, WP-1505, WP-1515: `g++.dg/reflect` 306 → 447 of 565.
- WP-1463, WP-1485: `g++.dg/contracts` 52 → 100 of 123 (`-fcontracts`, `-lstdc++exp`, `__builtin_exit`, client checks).
- WP-1464, WP-1483: `g++.dg/cpp26` 270 → 309 of 435 (one-argument `va_start`, placement new, expansion statements, vector bindings).
- WP-1465, WP-1493, WP-1506: the deduction and CTAD clusters (CWG2303, CWG2369, CWG2918, guide access, aggregate and alias CTAD): 132 of their 133 cases pass.
- WP-1466, WP-1495, WP-1507: the parser and friend clusters (CWG2237, GNU attribute positions, `inline template`, dependent friends): 138 of 177; the 39 left are rejected by clang too.
- WP-1467, WP-1496: string literals to `char*`, pointer-to-member casts, access contexts, two-phase operator lookup, elidable copies before C++17.
- WP-1468: Oven const subobjects and temporaries, unions, delegating constructors.
- WP-1469, WP-1499, WP-1512: variable length arrays in C++ and expansion statements (every VLA case clang 22 accepts passes).
- WP-1470, WP-1500, WP-1501, WP-1513: the accepts-invalid batches (explicit specializations matching nothing, ambiguities, `= default` checks, default arguments, special-member friend templates).
- WP-1471: accepts-invalid batch 2 (13 of 17 clusters).
- WP-1472, WP-1497, WP-1509, WP-1510: `g++.dg/cpp2a` + `g++.dg/cpp23` 3225 → 3514 of 4371.
- WP-1484, WP-1503: elidable copies in codegen and the Oven; `-f[no-]elide-constructors`.
- WP-1494, WP-1508: union member designators in the Oven.
- WP-1517: constexpr static data members before C++17 are not inline.
- WP-1526..WP-1529: P2280 in every mode from C++11 and `this` as a member-access postfix (`constexpr-ref14`, `constexpr-array-ptr6`, `lambda-generic-const10`); placement new into an inactive `T[1]` member (`constexpr-new23`/`28`); `constexpr-eh7`'s `catch (const char **&)` of a `nullptr` stays uncaught ([except.handle]/3.4 needs `const T&`); `__builtin_bit_cast` of classes with bases and bit-fields (`bit-cast3`, `bit-cast7`, `bit-cast16`).
- WP-1473 (this run): the regressions below.
- WP-1547, WP-1542, WP-1545 (after this run): `label13a`, `fam-in-union-alone-in-struct-2`, `delete5`, `udlit-char-template-vs-std-literal-operator`, `ellipsis3` and `pr53011` from the crash, wrong-code and regression lists; `attr-aligned-1` is left (an Oven typedef-alignment bug).
- WP-1530 (after this run): `p->N::f<U>()` with a dependent `p` and a non-dependent class `N`, and an overloaded member template named through a qualifier of the current instantiation or a non-dependent base, take a template argument list without `template` (`decltype34`, 3 cases; `member3`-`member5` need member calls through `this` to be non-dependent, left as a remainder); `nested3` (WP-1546) is right as it is: its member declaration is a type-only context where `<` starts a list ([temp.names]/7.3.4), and its L14 error is WP-1470's incomplete-class rule.
- WP-1541, WP-1536 (after this run): function specifiers on structured bindings and for-range/expansion declarations (`range-for42`/`43`, `decomp3`, `expansion-stmt9`), attributes on asm-declarations (`dr2262`, CWG2262) and keyword attribute-tokens (`gen-attrs-61`): 20 cases; `inline template` (`wrong-inline1`, `error11`, 6 cases) keeps clang's warning, a documented difference.
- WP-1587, WP-1549, WP-1551, WP-1550 (after this run): `__builtin_{add,sub,mul}_overflow_p` (`builtin-arith-overflow-2`, `builtin-arith-overflow-p-*`), GNU `noreturn` added by a function pointer conversion (`noreturn1`/`2`, `canon-33194`): 56 cases to pass; `__builtin_va_arg_pack` and the C++ `gnu_inline` redefinitions stay rejected, as in clang (C: the cast to a union type, a regression).
- WP-1574, WP-1568, WP-1575, WP-1569, WP-1567 (after this run): `attr-aligned-1` (`alignof` of a cv-qualified aligned array typedef, also of an array of one); the Oven now runs the destructors of empty virtual bases, so `delete5`'s codegen guard is gone; an assignment through an inactive union member of class type with a non-trivial default constructor is rejected (`constexpr-union8` stays passing: anonymous union members are exempt); a union's flexible array member elements of class type are left to the static initializer. Filtered before/after run (1503 cases): no case from pass to a failing class.
- WP-1532, WP-1533, WP-1534, WP-1535 (after this run): accepts-invalid fixes for `overload-conv-5`, `inh-ctor30`, `explicit77`, `fn-ptr3a`, `fn-ptr5`, `discarded1a` (from C++11), `using-enum-3`, `using30`; `constexpr-lambda10` is left (an Oven capture read).
- WP-1593, WP-1595, WP-1572, WP-1588, WP-1591 (after this run): nested anonymous union members with default member initializers (`constexpr-union8`'s exemption for anonymous union members is gone), arrays of over-aligned typedef elements, `this` in a static member function's declarator (`constexpr-this1`, 3 cases: accepts-invalid to wrong-line, now clang 22's diagnostics), structured bindings in a condition with an array and at class scope, a function named as a template argument in a lambda body in a template. Filtered before/after run (1475 cases): no case from pass to a failing class.
- WP-1590, WP-1594, WP-1573 (after this run): an odr-use of a by-copy capture in a constant expression in the lambda body names the closure member (`constexpr-lambda10`, 3 cases); `delete` deallocates when the destructor throws; an indeterminate bit-field sharing a byte with a determinate one after a bit-cast is not readable. Filtered before/after run (3119 cases): no case from pass to a failing class.
- WP-1570, WP-1578, WP-1598, WP-1599 (after this run): a member call through `this` naming one non-template member function with a non-dependent type, of a class with no dependent base, is not type-dependent (`member3`, `member4`: 6 cases; `member5`'s call of a member template stays dependent, a remainder); the cleanups of a dependent template argument end with it before C++17; `sat_concept_id_partial` with a non-type parameter; requires-expression deferral under pack expansions. Filtered before/after run (12500 cases of template, lookup, cpp2a, concepts, cpp1z, cpp26 and others): no case from pass to a failing class.
- WP-1548, WP-1552 (after this run): GCC's `__builtin_shuffle` and mixed-length `__builtin_shufflevector` (`builtin-shuffle-1`, `builtin-shufflevector-1`/`-2`, `builtin-convertvector-2`, every `vshuf-*`), `__builtin_speculation_safe_value` (`spec-barrier-*`, `builtin-speculation-overloads1`-`4`), `__builtin_assoc_barrier`, `__builtin_ffsg`/`clrsbg`/`parityg` (`pr111309-1`, `pr118868-1`), and deprecation/unavailability of conversion functions and instantiated enumerators; `builtin-assoc-barrier-1` still fails on its `optimize` attribute; `__builtin_has_attribute`, `__integer_pack` and `__is_invocable` are left out.
- WP-1554, WP-1556, WP-1600 (after this run): the runner reads `dg-message` and `dg-regexp` lines that name an error as expected errors and line-0 `dg-error`/`dg-message` as any line; a case whose errors only `dg-message ""`, `dg-prune-output` or `dg-excess-errors` could account for is `expect-unknown`, counted apart (filtered run with one release kiln, 7292 cases: 27 to pass, 135 to expect-unknown, none from pass to a failing class); the citations of the verdict tables were checked against the working draft; every CI job has a timeout.
- WP-1604, WP-1606, WP-1592 (after this run): an anonymous union or struct defines only public non-static data members (`anon-union5`, 3 cases), `alignof` of an enumeration with an aligned attribute and arrays of it, an odr-use of a by-copy capture in a case value outside a template, and `B b(u)` through `operator B() const` beside a better `operator B&()` (clang's CWG2327 result). Filtered before/after run (3741 cases): no case from pass to a failing class.
- WP-1609, WP-1610, WP-1612 (after this run): a call through `this` of the current instantiation naming an overload set or a member template resolves at definition when instantiation cannot choose differently (`member5`, 3 cases); a type-constraint's argument list is checked as the concept-id (pack expansion onto a non-pack parameter, arity); `__builtin_parityg` rejects `bool` and enumerations; an invalid specialization adds no `redefinition` error (`builtin-speculation-overloads5`, 3 cases); `var-concept5` (2 cases) passes too. Filtered before/after run (7447 cases of template, lookup, concepts, builtins): no case from pass to a failing class, 8 to pass.
- WP-1539 (first half), WP-1543 (after this run): pointer-to-member conversions through an inaccessible or ambiguous base (`ptrmem2`-`7`), decl-specifier and declarator rules (`inline` on a parameter or data member before C++17, free-standing tag specifiers, `void` parameters, conversion-function cv, template typedef, typedef-name destructors), friend forms (`crash42`, `crash43`, `crash44`, `friend45`), `C::C` as a type, `typename` naming a non-type template, ambiguous qualified injected-class-names, repeated `using E::x;` in a class (`using-enum-5`), and `'+f'` asm outputs on x86-64. Filtered before/after run (9064 cases): 81 to pass, none from pass to a failing class.
- WP-1621, WP-1622, WP-1624, WP-1625, WP-1616, WP-1615 (after this run): `<stddef.h>` declares `::nullptr_t` from C++11 (`cpp0x/stddef`, 3 cases); an in-class initializer of a floating or complex floating `static const` member at C++98 is the GNU extension clang accepts (`ext/complex7`); a by-copy capture odr-used in a local class's or enumeration's scope is diagnosed; a template argument's narrowing check folds after its full-expression; `using N::X;` then `struct X` in one scope conflicts; an rvalue reference bound through a conversion function yielding a derived class as clang and GCC bind it.
- WP-1630, WP-1631 (after this run): an overload set without a target (`for1`, `cond7`), `static_cast` casting away constness (`cast5`), closure special members by language mode (`lambda-ctor-neg`, `lambda1`), explicit specialization headers (`spec15`), CTAD class parameters as forwarding references (`class-deduction26`, `35`, `96`), inaccessible move constructors (`implicit-move1`) and template template argument access (`access18`), later-standard rules in earlier modes (`typename3`, `dependent-name5`, `constexpr-lambda29`, `eb129`, `900210_10`, `subscript11`), anonymous union member access (`union2`), injected-class-name ambiguity at instantiation (`injected1`). Filtered before/after runs (15820 + 20785 cases, overlapping): 73 to pass; from pass: `using-enum-3` and `using-enum-5` (a class's using-declarator of its own enumerator is accepted, as clang 22; deviations.md), `nested5` and `crash-9` at C++98 (implicit `typename` is a C++20 extension, an error under `-pedantic-errors`, as clang 22).
- WP-1525, WP-1523, WP-1520 (after this run): `sizeof` of a variable length array type in an instantiated function template (`template/vla1`, 3 cases to pass); caller-side contract checks of declared-only template specializations; `expansion-stmt16` stays wrong-line by decision (its second error is not required) and `eh/anon1` stays wrong pending the `type_info` name decision. Filtered before/after run (2424 cases of vla, contracts, expansion-stmt, lambda, rtti, inherit, abi): 3 to pass, none from pass to a failing class.

### Cases that went from pass to a failing class (99)

All 99 are explained; 37 of them were real regressions, fixed by
WP-1473 (re-run of the 50 tests with the fixes: 37 cases back to pass,
none newly failing):

- by-copy capture of a reference (`int (&f)() = g; [f]{}`), from WP-1426 (6 cases);
- `explicit` on a friend declaration not diagnosed, exposed by WP-1507 (5);
- CWG2918 address selection between templates with different parameter lists, from WP-1465 (6);
- converted constant expressions allowing a floating-integral conversion, exposed by WP-1506 (1);
- the explicit instantiation scope check applied in C++98, from WP-1448 (1);
- `T a[N](t...)` with an empty pack, from WP-1470 (1);
- a template-id's value deducing `T` of `template<class T, T N>` before C++17, from WP-1465 (3);
- an in-class `constexpr` static data member without initializer before C++17, from WP-1517 (1);
- the second step of a copy-initialization before C++17 copy-initialized instead of direct-initialized, from WP-1496/WP-1513 (2);
- a friend destructor template accepted, from WP-1513 (6);
- C++11 implicit `const` not seen through a member function template, from WP-1506 (1);
- `Other::m` naming a member of a sibling class inside a member function taken as `this->m` (1);
- `__is_pod` in C++98 judged by C++11 rules, from WP-1452 (1).

One regression is an xfail repro (`over.built/string-literal-vs-converted-char-pointer.cpp`,
`WP-1473 remainder`): `x != "lit"` with a class converting to `char*`
is reported ambiguous since WP-1467 made the string literal convert to
`char*` (1 torture case at `-O0`/`-O2`, 2 cases; WP-1545).

The other 60 changed class because kiln now follows the standard or
clang 22 where the test expects GCC's answer: clang fails each of them
the same way (`pure-virtual1` link, contracts before C++26, `cond2`,
`constexpr-self1`, `defaulted67`/`68`, `inh-ctor37`, `noexcept36`,
`nsdmi-dr1397`, `pr85437-2`, `override5`, `flexary34`, `pr86159`,
`constexpr-nsdmi3`/`8`/`9`/`10`, `lambda-this5`, `concepts-return-req2`,
`crash-11`, `type_trait5`, `p7180`, `explicit70`, `elide2`), or:

- `inline template<class T> void f();` is accepted with clang's warning since WP-1495; [temp.pre]/1 has no decl-specifier before `template`, GCC errors (`wrong-inline1`, `error11`; 6 cases; WP-1536);
- a missing `template` keyword before a dependent member template's `<` is no longer diagnosed where clang still reports it (`nested3`, 3 cases; WP-1546);
- `reflect/crash8` runs at C++26 without `-freflection`; kiln enables reflection at C++26 and the splice destructor it uses is valid (1);
- `other/pr117516` (deeply nested structs) is on the 60 s compile limit (1).

### Crash / hang (6 cases)

1. **Stack overflow copying a class through its own ellipsis
   constructor** (3 cases, 1 test, `overload/ellipsis3`; clang 22
   overflows too). Direct-initializing a `volatile S` from another one, where `S` has
   only a variadic constructor `S(...)`, picks `S(...)`, whose argument
   must itself be copied into the ellipsis by `S(...)`, without end. Area: sema overload (passing a
   class object through `...`: [expr.call]/13 makes it a copy, and a
   volatile glvalue has no usable copy constructor). WP-1542.
2. **Compile timeout** (3 cases, `other/pr117516`): about 60 s on a
   chain of thousands of nested structs. Area: sema record layout and
   implicit members (perf).

### Wrong-code (144 cases, 54 tests)

Clang passes six of these tests; the rest either fail under clang too
or test GCC behaviour clang does not have.

1. **Clang passes** (14 cases, 6 tests): `attr-aligned-1` at C++98
   (`__attribute__((aligned))` on a type's maximum alignment),
   `fam-in-union-alone-in-struct-2` (a flexible array member alone in a
   union), `udlit-char-template-vs-std-literal-operator` (a character
   literal operator template next to the standard one), `label13a`
   (SIGSEGV: labels as values across functions), `delete5` at C++98, and
   `is_pod_98` (fixed by WP-1473). Area: per test; none reduced yet (WP-1547).
2. **GCC builtins and options kiln has with different results** (about
   70 cases): `__builtin_object_size` and `__builtin_dynamic_object_size`
   (4 tests), `__attribute__((cleanup))` with exceptions (4),
   `-flifetime-dse` (3), identical-code-folding addresses (2),
   `__builtin_clear_padding` (3), `-ftrivial-auto-var-init` padding (1),
   `__sync` builtins (1), `__builtin_constant_p` folding (1). Clang fails
   or rejects them too. Area: per builtin; low priority.
3. **`__PRETTY_FUNCTION__` text** (12 cases, 4 tests: `pretty1`,
   `pretty2`, `pretty3`, `memtemp77`): GCC's `[with T = int]` form; clang
   differs too. Area: the pretty-function printer.
4. **Layout and ABI** (about 15 cases): `abi/bitfield1`, `abi/vbase13`,
   `packed4`, `abi/local1` (GCC's layout of oversized bit-fields, packed
   non-PODs and local class names). Clang differs too.
5. **Accepted where clang rejects, then wrong at run time** (about 20
   cases): `nsdmi-union5`, `explicit-obj-constraints`, `expansion-stmt17`,
   `flexary39`/`40`, `pr37146-1`. The C++ ones are worth a look: clang 22
   rejects (or lacks the feature) so there is no oracle; GCC passes.
   Area: per test.

### Rejects-valid where clang accepts (355 cases, 104 clusters, top 15)

A long tail: no cluster is larger than 12 cases.

1. **Overloads that differ in a vector return type** (12 cases, 4 tests).
   `vector float f(); vector int f(int);` under GCC's `vector` keyword
   (AltiVec syntax as a GNU extension) is taken as two declarations of
   one function. Area: sema redeclaration (vector types compared by
   element type).
2. **Link failure: undefined reference** (9 cases, 4 tests): `restrict`
   member functions and inline definitions kiln does not emit. Area:
   codegen deferred emission.
3. **Call to a deleted function in SFINAE** (7 cases, 5 tests;
   `sfinae70`/`71` at C++23 and later). An implicitly movable entity is
   a prvalue argument (`f(Ptr(), 42)` with `f` taking `Ptr&`) since
   P2266; GCC and clang accept at C++26. Area: sema overload.
4. **GNU `?:` with a class-type operand** (7 cases, 3 tests):
   `bool b = X() ?: false;` says "not supported yet". Area: sema
   conditional operator (binary form).
5. **`decltype` of an unresolved call in a lambda's mangled name** (6
   cases): "cannot yet mangle expression type UnresolvedLookupExpr".
   Area: mangler.
6. **Pointer-to-member comparison after a `static_cast`** (6 cases):
   `ap != bp` with `const int A::*` and `int B::*` where `B` derives from
   `A` needs the composite pointer type. Area: sema composite pointer
   type for pointers to members.
7. **`using namespace N __attribute__((strong));`** (6 cases): GCC warns
   that `strong` is no longer supported; kiln stops at the attribute.
   Area: parser (GNU attributes on using-directives).
8. **`using namespace std;` with no `std` declared** (6 cases, `-w`):
   GCC predeclares `std`. Area: sema (implicit namespace std).
9. **Out-of-line member template of a class template's friend operator**
   (6 cases): "extraneous template parameter list". Area: sema templates
   (template headers of out-of-line member templates).
10. **`[[gnu::vector_size]]` on a dependent pointer alias** (6 cases):
    the attribute is applied before substitution. Area: sema attributes.
11. **Not yet implemented in kiln** (18 cases, 6 tests): deduction guides
    as members of a class template's member template, friend
    declarations with a dependent nested-name-specifier
    (`friend void* A<T>::operator new(size_t)`), friend member templates
    of a class template (`friend struct Outer<V>::Inner`). Area: sema
    templates.
12. **Ambiguous member through bases of different specializations** (6
    cases): `D().v<int>::size()` with bases `v<double>` and `v<int>`
    (the injected-class-name names one base). Area: sema member lookup.

### Accepts-invalid where clang rejects (638 cases, 225 clusters)

The largest cluster is "an error expected anywhere, kiln gives none"
(106 cases, 49 tests: the test's `dg-error ""` has no message
pattern); with the same cases in the other buckets (338 in all, 147 of
them with no error at all) it is classified in "Any error expected, and
the largest wrong-line clusters" below. The rest is a tail of small
clusters:

- C++98 pedantic extensions not diagnosed under `-pedantic-errors`
  (`enum E { a, b, };`, a lambda with a template parameter list in
  `gnu++98`; 9 cases). Area: parser extension warnings.
- an implicitly moved `return x;` whose first overload resolution selects a deleted converting constructor `R(X&&)`
  ([expr.prim.id.unqual]/15; C++20 [class.copy.elision]/3; 5 cases). Area: sema return.
- redeclarations through an alias template that ignores its parameter
  (`void f(void_t<typename T::type>*)` against `void f(void*)`, CWG1980;
  4 cases). Area: sema template redeclaration matching.
- `requires b;` on a parameter in a requires-expression is not a
  constant expression ([expr.prim.req.nested]/1; 4 cases). Area: sema
  constraint satisfaction.
- `parse/dtor16` (a destructor template; 6 cases) passes after WP-1473.

### Where clang agrees with kiln: verdicts by the standard

Verdicts: **a** = kiln and clang are right by the standard (GCC accepts an extension, or GCC differs);
**b** = the test needs a GCC-only flag, builtin, attribute, pragma or target feature; **c** = kiln is wrong
along with clang. Paragraph numbers are the C++26 working draft of 2026-11 (cplusplus/draft of 2026-11-16) unless marked otherwise, checked against the paragraph text (WP-1556). A cluster is the first kiln
diagnostic with names and numbers stripped. Mixed clusters give the dominant verdict and the split.

Two caveats:
- 75 of the 1093 cases in the 30 largest rejects-valid clusters have GCC rejecting the test as well. Those
  tests expect their error through `dg-message ""` or `dg-regexp`, which the runner does not read as an
  expected error. That is a runner gap, not a kiln finding; it is marked "GCC-rej" below. Since WP-1554
  the runner reads them (see the WP-1554 line above).
- clang 22 has no reflection, so for the 61 `reflect/` cases in this bucket "clang rejects too" tells us
  nothing. They are marked c? and need checking one by one (WP-1537).

The c verdicts are rows WP-1526 to WP-1538; the b verdicts worth adding
for GCC compatibility are WP-1548 to WP-1552 and WP-1553.

WP-1531 fixed, after this run: the generic-lambda requirement order (#30's c 3, `concepts-lambda13`) and a concept's invalid default argument outside the atoms' parameter mappings (#8's c 3, `concepts-defarg2`); both are clang deviations (docs/status/deviations.md). `concepts-return-req1` (accepts-invalid #6) is satisfaction-correct in kiln; the expected ambiguity is GCC's partial ordering of `f()` against `f(...)`, which clang and kiln resolve by constraints. WP-1553 accepts `-ftemplate-depth-N`.

#### Rejects-valid, clang rejects too (2520 cases): 30 largest clusters (1093 cases)

| # | cluster (short pattern) | cases | verdict | reason |
|---|---|---|---|---|
| 1 | use of undeclared identifier | 265 | b | GCC builtins: `__builtin_shuffle` 72, `__builtin_add_overflow_p` 38, `__builtin_apply_args` 18, `__builtin_va_arg_pack`(`_len`) 18, `__builtin_mul_overflow_p` 12, `__integer_pack` 10, `__builtin_assoc_barrier` 9, `__builtin_puts` 8, `__builtin_speculation_safe_value` 6, 7 smaller ones 16; `__builtin_has_attribute` operands 22; `__CHAR8_TYPE__` 4. a: `-fpermissive` lookup 10, `::nullptr_t` 3 (kiln's: [support.c.headers.other]/1 requires `<stddef.h>` to declare it; fixed by WP-1615). c: ADL of a non-hidden friend 1 ([basic.lookup.argdep]/4, P1787). GCC-rej 27 |
| 2 | static assertion failed (no message) | 68 | mixed | c? reflection 25 (clang has no reflection); a 21: `source_location` column and function_name are implementation-defined ([support.srcloc.cons]) 18; `__is_trivial` with a deleted, more-constrained default ctor 3 ([special]/6, [depr.meta.types]/3); b 21: GCC 16 makes misuse of `__atomic_*` and other builtins a SFINAE failure 12, `-D` taken from a target selector 6, `-fabi-version` 3; c? constexpr virtual inheritance 1 |
| 3 | constexpr variable must be initialized by a constant expression | 54 | a | GCC differs on UB during constant evaluation 38: writing a member before its lifetime starts ([basic.life]/7) 18, member pointer cast to an unrelated class ([expr.static.cast]/11) 6, delete through a base without a virtual dtor ([expr.delete]/3) 3, reading an object in its own initializer 3, pointer parameter read in `p->static_member` ([expr.ref]) 3, local address 2, implementation-defined function_name 3. c 5: bit_cast with bit-fields 3, P2280 only applied from C++23 2. b 7: `-fconstexpr-fp-except`, vector `==`, zero-size array. Plus reflection 1, GCC-rej 3 |
| 4 | a type specifier is required | 53 | a | implicit int (`main()`, `-fpermissive`, system-header line markers); [dcl.type.general]/3 |
| 5 | unknown type name | 53 | b | Concepts TS `concept bool` (`-fconcepts`) 35; GCC trait `__remove_reference` 11; `__constinit` keyword 4; GNU attribute inside a parenthesized declarator 3 |
| 6 | '_Complex' is a C99 extension (-Werror) | 42 | a | `_Complex` is a reserved identifier ([lex.name]/4) used as an extension; it is an error only under the suite's `-pedantic-errors` |
| 7 | expected 'token' | 40 | b | GCC 15 extended toplevel `asm` with operands 18; a: `__builtin_offsetof` with a qualified member designator 6 ([support.types.layout]/1), `_Alignas` used as an identifier 2 ([lex.name]/4); GCC-rej 14 |
| 8 | no matching function for call | 37 | mixed | a 11: CWG1286 alias template equivalence is still open 3, `-fpermissive` pointer conversions 6, exception-spec relaxed in system headers 2; b 9: GNU `noreturn` on a function-pointer parameter is ignored for conversions; c 9: P2280 `this->m.f()` in a template argument 3 (rejected even at C++23), explicit args plus deduction extending a pack ([temp.arg.explicit]/9) 3, default template argument of a concept outside the atomic constraint's parameter mapping ([temp.constr.normal]/1.4) 3; reflection 2; GCC-rej 6 |
| 9 | link: undefined reference | 37 | b | `link_error` folded away only by GCC's optimizer 9, `#pragma interface/implementation` 9, `-nostdlib++` plus weak `__cxa_pure_virtual` 3, alias attribute on typeinfo 3; c 4: C++11 implicit move picks `X(const T&)` (C++11 [class.copy]/32) 1, friend template defined by a later implicit instantiation not emitted ([temp.point]/7) 3; GCC-rej 6 |
| 10 | invalid suffix on literal (reserved UDL) | 36 | a | ud-suffix without `_` is reserved, IFNDR ([over.literal]/8, [usrlit.suffix]); `"x"MACRO` lexes as one UDL token ([lex.pptoken]/5); GCC-rej 2 |
| 11 | ISO C++11 does not allow access declarations | 34 | a | removed in C++11 (C++03 [class.access.dcl]); kiln still accepts them at C++98 |
| 12 | static assertion expression is not an ICE | 34 | c | c 18: bit_cast of classes with bases or bit-fields 6, placement new/construct_at changing a union's active member 6, P2280 2, a thrown `nullptr` not caught by a pointer handler ([except.handle]/3) 1, ...; a 10: subtracting pointers into different subobjects ([expr.add]/5), arithmetic on a pointer to a non-similar type ([expr.add]/6), `&*nullptr` ([expr.unary.op]/1), `frexp` before C++23, `void*` cast to a non-similar type ([expr.const.core]/2.15, P2738); b 3: `__builtin_strstr`/`index`/`rindex` in constant expressions; reflection 6 |
| 13 | redefinition | 30 | b | `gnu_inline` extern-inline redefinition semantics 24; a: a namespace alias reusing the name of the namespace it aliases 3 ([basic.link]/8, [basic.link]/11.3); alias specializations that ignore their argument 3 (CWG1980 open) |
| 14 | variable-sized object may not be initialized | 30 | a | VLAs and initialized VLAs are GNU extensions ([dcl.array]/1); GCC-rej 3 |
| 15 | attribute cannot be applied to types | 28 | a | attribute or `alignas` in a position that appertains to a type ([dcl.attr.grammar]/7, [dcl.align]/1); GCC ignores it with a warning |
| 16 | use 'template' keyword for dependent template name | 27 | mixed | c 12: member of the current instantiation (`this->f()`, implicit `this`, non-dependent base) treated as type-dependent, so `.bfn<int>` is not parsed as a template ([temp.dep.type]/4, [temp.dep.expr]/5); a 12: a dependent `X::Y<int>::Z` with no `template` before Y (`<` in a non-terminal nested-name-specifier) ([temp.names]/7), GCC applies the CWG1710 direction; GCC-rej 3 |
| 17 | pack expansion contains no unexpanded packs | 25 | b | GCC builtin `__integer_pack` 21; GCC-rej 4 |
| 18 | static assertion failed (with message) | 25 | a | `noexcept(T())` with a trivial noexcept(false) default ctor 6 (CWG2820); nothrow array new with a runtime bound treated as throwing 3 ([except.spec]/6); list-init of a reference through a conversion function 3 ([dcl.init.list]/3.10, CWG1996); `__func__` length 3 ([dcl.fct.def.general]/8); closure size 3 ([expr.prim.lambda.closure]/4). b 5: `-fabi-version`, `_Complex` trait. c 2: `new T[](args)` (P1009/P0960, [expr.new]/7) |
| 19 | expected unqualified-id | 23 | a | explicit template args on a qualified constructor name 6 ([class.qual]/1, [temp.arg.explicit]/2); `unsigned` with a typedef-name 3 ([dcl.type.simple]); b: `__is_invocable` trait 3, `-fconcepts` 1; c 3: keywords and alternative tokens in an attribute-token, `[[using, ...]]` ([dcl.attr.grammar]/4); GCC-rej 7 |
| 20 | cannot initialize a variable of type | 21 | a | `-fpermissive` int/pointer conversions, PMF to `void*` (`-Wno-pmf-conversions`), `_Complex` |
| 21 | character not allowed in an identifier | 21 | a | bidi controls are not XID_Continue ([lex.name]/1); delimited and named escapes are C++23 ([lex.universal.char]) |
| 22 | 'auto' not allowed in function prototype | 20 | b | Concepts TS `-fconcepts` at C++14 18; a: abbreviated function templates before C++20 2 ([dcl.fct]/23) |
| 23 | an attribute list cannot appear here | 18 | a | attributes on a non-defining friend ([dcl.attr.grammar]/7), after an elaborated class name ([dcl.type.elab]), after a namespace name ([namespace.def.general]); c 3: attribute on an asm-declaration (CWG2262, [dcl.asm]/1) |
| 24 | reference member binds to a temporary | 17 | a | GCC differs: a mem-initializer binding a temporary to a reference member is ill-formed ([class.base.init]/8); GCC only warns |
| 25 | cannot be defined in a type specifier | 16 | a | GNU compound literal that defines a union (ISO C++ has no compound literals, and types cannot be defined in a cast); GCC-rej 3 |
| 26 | implicit instantiation of undefined template | 16 | a | CWG1286 (open) 6; `gnu::aligned` after an array declarator applies to the array type 3; non-dependent use of an incomplete type in a template, IFNDR ([temp.res.general]/6) 3; default template arg of a member template substituted with the class (underspecified, [temp.inst]/3) 3; 1 not checked |
| 27 | is a private member | 15 | a | non-dependent access violation in a template that is never instantiated, IFNDR ([temp.res.general]/6) |
| 28 | first two args to `__builtin_shufflevector` must have the same type | 15 | b | GCC's `__builtin_shufflevector` takes vectors of different lengths |
| 29 | structured binding cannot be declared X | 15 | a | `constexpr`/`static`/pack structured bindings are C++26/C++20 ([dcl.pre]/7); GCC pedwarns in earlier modes |
| 30 | type cannot be used prior to '::' | 15 | a | unspecified whether the instantiation happens ([temp.inst]/10) 9; c 3: a requires-expression checked out of order after partial instantiation of a generic lambda ([expr.prim.req.general]/5); GCC-rej 3 |

Dominant verdicts: a 19, b 8, c 3 (counting #2 as c? because of reflection, and #16 as c).

#### Accepts-invalid, clang accepts too (558 cases): 30 largest clusters (247 cases)

| # | cluster (expected GCC message) | cases | verdict | reason |
|---|---|---|---|---|
| 1 | (empty pattern) | 41 | a | CWG727/CWG393 DRs (class-scope explicit specialization, reference to an array of unknown bound) 5; partial specialization after use, IFNDR ([temp.spec.partial.general]/1) 3; a floating-point new-array bound is valid since C++14 ([expr.new]/8) 2; GNU asm label on a member 3; out-of-namespace explicit specialization 3 ([temp.expl.spec]/3); deducing explicit object param of a captureless lambda 2; ... c 11: address-of a by-default-captured constant in a constexpr init in a lambda 3, `inline template<...>` 3 ([temp.pre]/1), deduction from ambiguous bases must fail 3 ([temp.deduct.call]/5), C++98 ICE rules 2; b 8: `-fno-elide-constructors`, `-fno-new-inheriting-ctors`, packed return |
| 2 | changes meaning | 27 | a | a name whose meaning changes in class scope is IFNDR ([class.member.lookup]/6; C++20 [basic.scope.class]/2) |
| 3 | is not supported for this target | 18 | b | `-fcf-protection` is an x86 CET flag; GCC also accepts on this host (target selector) |
| 4 | conflicting | 12 | a | redeclaring `__cxa_*` with different types: reserved names ([lex.name]/4) 9; `__typeof__` conflict in a template that is never instantiated, IFNDR 3 |
| 5 | not a member of coroutine_handle | 9 | a | the test supplies its own `std::coroutine_handle` without `address`; UB ([namespace.std]/1) |
| 6 | ambiguous | 9 | c | inherited ctor vs derived ctor with different parameter types is ambiguous ([over.match.best.general]/2.7) 3; atomic constraint with an empty mapping is satisfied, so both overloads are viable ([temp.constr.normal]/1.4) 3; two different conversion functions leave the copy/move ctor candidates ambiguous ([over.ics.rank]/3.3, CWG2327 open) 2; b `-fconcepts` 1 |
| 7 | arithmetic on pointer to an empty aggregate | 9 | a | zero-length arrays are GNU; kiln flags the declaration under `-pedantic-errors` and warns about the arithmetic |
| 8 | narrowing (in system header) | 9 | c | narrowing is ill-formed ([dcl.init.list]/3.9, [intro.compliance.general]/2), but kiln exempts system headers on purpose; needs a decision, not a fix |
| 9 | redeclared | 9 | a | `__cxa_*` reserved names ([lex.name]/4) |
| 10 | does not declare anything | 8 | a | kiln issues a warning, which is a diagnostic ([dcl.pre]/6, [intro.compliance.general]/2); a lone GNU attribute declaration |
| 11 | anonymous struct with base | 6 | a | anonymous structs are a GNU extension |
| 12 | cannot bind packed field | 6 | a | `packed` is GNU; GCC's ban on binding a reference to a packed field is GCC policy |
| 13 | cannot tail-call | 6 | a | `gnu::musttail` feasibility is implementation-specific and checked at codegen, not under `-fsyntax-only` |
| 14 | conflicting declaration | 6 | a | `__dso_handle`, `__dynamic_cast` are reserved ([lex.name]/4) |
| 15 | depth | 6 | a | class-scope explicit specialization is valid (CWG727) 3; b: `-ftemplate-depth-50` spelling 3 |
| 16 | fail | 6 | c | a function template named as a template argument is odr-used and must be instantiated ([basic.def.odr], [temp.inst]) |
| 17 | incomplete | 6 | c | discarded volatile glvalue needs an lvalue-to-rvalue conversion, so the type must be complete ([expr.context]/2) 3; a: member of incomplete self type in a template that is never instantiated 3 |
| 18 | invalid arguments for -fpatchable-function-entry | 6 | b | flag value check; GCC also accepts on this host |
| 19 | match | 6 | b | `-fno-new-ttp-matching` (pre-P0522 behavior) |
| 20 | no match | 6 | b | `-fno-new-inheriting-ctors` 3; a: call with no viable overload in a template that is never instantiated, IFNDR 3 |
| 21 | conflicts | 5 | c | a using-declaration that redeclares an existing class member ([class.mem.general]/8, [namespace.udecl]/10 example; C++20 [basic.scope.declarative]/4) |
| 22 | indeterminate attr not on first declaration | 4 | a | `[[indeterminate]]` is C++26; earlier modes ignore it with a warning; kiln diagnoses it at C++26 |
| 23 | deleted | 4 | a | GCC differs: a prvalue initializes a `[[no_unique_address]]` member directly ([dcl.init.general]/15.6.1; CWG2403 open) |
| 24 | designator used multiple times | 4 | a | kiln warns (`-Winitializer-overrides`); a warning is a diagnostic |
| 25 | mixed designated and non-designated | 4 | a | kiln warns (`-Wc99-designator`) under the test's `dg-options ""` |
| 26 | attribute may not be specified (strict_flex_array) | 3 | b | GCC attribute `strict_flex_array` |
| 27 | expanded inside directive with -fdirectives-only | 3 | b | `-fdirectives-only` |
| 28 | -fdirectives-only is incompatible | 3 | b | `-fdirectives-only` |
| 29 | is not in NFC (-Werror=normalized) | 3 | b | `-Wnormalized` |
| 30 | is unavailable | 3 | b | `[[gnu::unavailable]]` spelling (kiln: unknown attribute) |

Dominant verdicts: a 17, b 8, c 5.

### Any error expected, and the largest wrong-line clusters

Source: the WP-1473 full g++.dg run (aarch64), per-case results. Each test was
re-run with kiln and clang 22 using the runner's flags (`-pedantic-errors
-Wno-long-long` for g++.dg and old-deja unless `dg-options` replaces them).
R = kiln misses a diagnostic the standard requires (row WP-1539; WP-1553 for the `-ftemplate-depth-N` spelling). G = stays (a GCC
extension or flag, a mode-specific GCC rule, no diagnostic required, a location
convention, or GCC differs from the text). The clang column counts clang 22's
result on the same cases.

#### "Any error expected" (`dg-error ""`): 338 cases, 144 tests

147 accepts-invalid (no error at all) and 191 wrong-line ("missed error at
LN") cases.

| cluster | cases | tests | clang | verdict | reason with paragraph |
|---|---:|---:|---|---|---|
| Location only: kiln reports on a nearby line, at the use, or at the instantiation, where clang does (`initlist-ref-2267`, `nsdmi-dr1397`, `pr77655`, `parse/crash2`, `other/goto1`, `other/goto3`, `other/init9`, `eh/spec6`, `pt/crash36`, `robertl/eb93`, `ns/alias4`, `law/access4`, ...) | 103 | 38 | 98 wrong-line, 3 accepts, 2 pass | G | The error is there, on a line next to GCC's (jump: at the `goto`, not the label; parse errors: at the next token; deleted or private special members: at the use). `init/enum1`: kiln flags the enumerator, GCC the closing brace. Only `init/synth2` and `bob/inherit2` at C++98 (1 case each) match clang and not kiln |
| Pre-C++11 rules not enforced at `-std=c++98 -pedantic-errors` (non-static member initializers, `long long`, trailing enum comma, braced default argument, empty member declaration, `friend T;` forms, local types as template arguments, `>>`, variable templates, internal linkage and pointer arithmetic in NTTPs, comma in a constant expression, explicit specialization outside the template's namespace, NTTP type that depends on a partial specialization's parameter) | 23 | 23 | 18 pass, 3 accepts, 2 wrong-line | R | C++03 [class.mem]/4, [basic.fundamental]/2, [dcl.enum]/1, [class.friend]/2, [temp.arg.type]/2, [temp.names]/3, [temp.arg.nontype]/1, [expr.const]/1, [temp.expl.spec]/2, [temp.class.spec]/9 |
| Decl-specifier and declarator rules: `inline` on a non-static data member, a parameter, or a C++14 variable; `const` on a conversion function's decl-specifiers; `inline template<...>`; `template<...> typedef`; cv on a class declaration; a qualified parameter name; a declaration with no type; `(void, ...)`; `~Typedef()` | 27 | 9 | 24 pass, 3 accepts | R | [dcl.inline]/1, [class.conv.fct]/2, [temp.pre]/1, [temp.pre]/2, [dcl.type.cv]/1, [dcl.meaning.general]/3.1, [dcl.type.general]/3, [dcl.fct]/3, [class.dtor]/1. kiln reads `inline template` as an explicit instantiation and only warns |
| Later-standard rules applied in earlier modes: CTAD before C++17 (`binder2nd(...)`), missing `typename` before C++20, static `operator[]` before C++23, NTTP subobject or cast forms before C++20, C++11 implicit copy assignment in a constant expression, `goto`/`static` in a lambda used in a constant expression before C++23 | 19 | 10 | 14 pass, 5 wrong-line | R (18), G (1) | C++14 [dcl.type.simple]/1 (template-name needs arguments), C++17 [temp.res]/3, C++20 [over.oper]/7, C++17 [temp.arg.nontype]/2, C++11 [expr.const]/2.21, C++20 [dcl.constexpr]/3. `pt/ptrmem6` at C++20 is valid ([expr.static.cast]/11): GCC differs |
| Friend declarations: `explicit` on a friend, explicit specialization in a friend, `template<class U> friend S<U>;` and `friend typename S<U>::X` | 18 | 6 | 18 pass | R | [dcl.fct.spec]/3, [temp.expl.spec]/18, [temp.friend]/7 |
| Pointer-to-member conversion through a private or ambiguous base (implicit, `static_cast`, C-style cast, base-to-derived direction) | 15 | 5 | 15 pass | R | [conv.mem]/2, [expr.static.cast]/11 |
| Anonymous unions: private member, nested class, and access of members of an anonymous union in a private or protected section | 15 | 5 | 12 pass, 3 wrong-line | R | [class.union.anon]/1, [class.access.general]/1 |
| Naming: `typename A::f<0>` where `f` is a function template, an injected-class-name found in two bases, `X::X` naming a constructor, a repeated `using E::x` in class scope | 12 | 4 | 9 pass, 3 wrong-line | R | [temp.res.general]/3, [temp.local]/4, [class.qual]/1, [namespace.udecl]/10 |
| Non-static member function or overload set without a target (`for(;;s.f)`, `a ? f : g;`, `auto p = &f<int>;` with two templates) | 9 | 3 | 9 pass | R | [expr.ref]/8.3, [over.over]/1, [temp.deduct.call]/6. kiln only warns "expression result unused" |
| VLA (GNU extension): `typeid` of a VLA type, deduction from a VLA | 9 | 3 | 9 pass | G | extension restriction; kiln warns on the VLA |
| GNU named return value (`int f() return r {...}`): GCC also flags the body | 9 | 3 | 9 wrong-line | G | removed GCC extension; kiln errors at the declaration, as clang |
| GCC-only flags: `-fno-elide-constructors`, `-fno-new-inheriting-ctors`, `-fno-new-ttp-matching`, `-ftemplate-depth-10` | 11 | 4 | 5 accepts, 3 pass, 3 wrong-line | G | GCC dialect flags; kiln drops `-ftemplate-depth-10` (clang accepts that spelling) |
| GCC differs from the text | 11 | 5 | 11 accepts | G | each case checked against the paragraph the test relies on |
| GCC extensions: `packed` on a non-default target, `asm` label on a data member, anonymous struct with a constructor in a union | 8 | 3 | 8 accepts | G | GNU extension rules |
| No diagnostic required: partial specialization declared after an instantiation it would match, C++98 non-ICE in a dependent template argument, an internal function used and not defined | 8 | 4 | 5 accepts, 3 pass | G | [temp.spec.partial.general]/1, [temp.res.general]/6, [basic.def.odr]/12 (kiln warns `-Wundefined-internal`) |
| CTAD: `T&&` of a class template parameter treated as a forwarding reference | 6 | 2 | 6 pass | R | [temp.deduct.call]/3 |
| Explicit specialization: one `template<>` for a member template of `A<int>`, specialization inside an unspecialized enclosing template | 6 | 2 | 3 pass, 3 wrong-line | R | [temp.expl.spec]/16-17 |
| Lambdas: copy assignment of a capturing closure, `*&n` of a capture in a constant expression | 6 | 2 | 3 pass, 3 accepts | R | [expr.prim.lambda.closure]/17, [expr.const.core]/2.14 (`this` of the closure) |
| Concepts TS (`-fconcepts` at C++17) | 4 | 4 | 4 wrong-line | G | GCC dialect |
| Access: private member template as a template template argument, private move constructor picked by implicit move in `return` | 4 | 2 | 4 pass | R | [class.access.general]/4, [expr.prim.id.unqual]/15 |
| `static_cast<void*>` of a const array casts away constness | 3 | 1 | 3 pass | R | [expr.static.cast]/6 |
| Deduction from two candidate bases yielding different `A` | 3 | 1 | 3 accepts | R | [temp.deduct.call]/5 (clang accepts too) |
| Derived-to-base pointer conversion while the derived class is incomplete (CWG2310) | 3 | 1 | 3 accepts | R | [conv.ptr]/3; kiln reports a VLA error on the next line instead |
| `__int128 signed signed` | 3 | 1 | 3 wrong-line | G | kiln warns (`-Wduplicate-decl-specifier`), a diagnostic per [intro.compliance.general]/2.3 |
| P2686R5 (C++26): constexpr reference and `initializer_list` bound to a constexpr local | 2 | 2 | 2 pass | G | kiln implements P2686R5, GCC and clang do not yet |
| A body after an `export` diagnostic is not checked (undeclared name) | 1 | 1 | 1 pass | R | error recovery ([basic.lookup.unqual]) |

Total: 338 cases. R: 16 clusters, 169 cases. G: 10 clusters, 169 cases
(including the 1 ptrmem6 case at C++20).

#### Twenty largest wrong-line clusters (excluding `(expected: )`)

| # | cluster | bucket | cases | verdict | reason |
|---:|---|---|---:|---|---|
| 1 | missed "modifying a const object" | clang differs | 45 | stays | error at the use with a note at GCC's line, as clang (WP-1468) |
| 2 | extra "unknown type name" | clang differs | 42 | stays | 33 are Concepts TS `concept bool` at C++17, 5 GCC's `__constinit`; the rest as clang |
| 3 | extra "use of undeclared identifier" | clang differs | 36 | stays | GCC builtins (`__integer_pack`, `__builtin_va_arg_pack`, `__builtin_current_exception`) and recovery errors clang also reports |
| 4 | extra "expected '…'" | clang differs | 33 | WP-1540 | 8 of 11 tests: kiln puts "expected '}'" at end of input on the line after the last one (column 1); clang uses the end of the last token |
| 5 | extra "no matching function for call" | clang differs | 33 | stays | same errors and lines as clang (Concepts TS, GCC accepts) |
| 6 | missed "expected" | clang differs | 28 | stays | kiln reports what clang reports (GCC's parser stops elsewhere) |
| 7 | extra "expected '…'" | clang matches | 27 | WP-1540 | same end-of-input location as #4; `cpp1z/using6` also cascades after a conflicting using-declaration |
| 8 | missed "no match" | clang differs | 25 | stays | error at the call or the deleted inherited constructor, as clang |
| 9 | missed "jump to label" | clang differs | 24 | stays | error at the `goto`, as clang |
| 10 | missed "call to non-constexpr function" | clang differs | 23 | stays | error at the constant expression with a note at the call, as clang |
| 11 | missed "not declared" | clang differs | 23 | stays | undeclared name reported where clang reports it |
| 12 | extra "constexpr variable must be initialized by a constant expression" | clang differs | 19 | stays | clang rejects the same variables |
| 13 | missed "uninitialized" | clang differs | 18 | stays | GCC errors on the class; kiln and clang warn there and error at the use |
| 14 | extra "expected unqualified-id" | clang matches | 16 | WP-1540 | same end-of-input location: an extra error at line N+1, column 1 |
| 15 | missed "reference to '_' is ambiguous" | clang differs | 16 | stays | before C++26 `_` is an extension; kiln reports as clang |
| 16 | extra "no member named" | clang differs | 15 | stays | coroutine promise and member errors where clang reports them |
| 17 | missed "ambiguous" | clang differs | 15 | stays | Concepts TS and redeclarations reported as clang |
| 18 | missed "instantiating erroneous template" | clang differs | 15 | stays | GCC's `-fpermissive` follow-up error; kiln reports the template's own error |
| 19 | missed "is not valid in an identifier" | clang differs | 15 | stays | kiln's errors match clang's line for line |
| 20 | missed "'virtual' outside class declaration" | clang matches | 14 | WP-1541 | `virtual`/`explicit` on a structured binding or range-for declaration is not rejected |

Stays: 16 clusters. Rows: 4 clusters, WP-1540 (#4, #7, #14, one cause) and WP-1541 (#20).

### Priority directories

| directory | cases | first run | second run | x86-64 | what is left |
|---|---:|---:|---:|---:|---|
| `g++.dg/reflect` | 565 | 306 | 447 | 447 | 61 compile-reject, 49 wrong-line (table below), 8 accepts-invalid |
| `g++.dg/contracts` | 123 | 52 | 100 | 100 | 4 rejects-valid (the [dcl.contract.func]/4-5 and [temp.expl.spec] deviations), 19 wrong-line (defensible locations, spec/contracts.md) |
| `g++.dg/cpp26` | 435 | 270 | 309 | 309 | 34 compile-reject (15 are GCC's pre-C++26 constexpr structured bindings, rejected by clang too), 83 wrong-line, 6 accepts-invalid, 3 wrong |
| `g++.dg/cpp2a` | 3784 | 2840 | 3074 | 3074 | 204 compile-reject, 412 wrong-line, 94 accepts-invalid |
| `g++.dg/cpp23` | 587 | 385 | 440 | 440 | 39 compile-reject, 78 wrong-line, 28 accepts-invalid, 2 wrong |
| `g++.dg/torture` | 1149 | 1066 | 1077 | 1075 of 1155 | 62 rejects, mostly GCC-only builtins (`__builtin_shuffle`, `__builtin_add_overflow_p`; clang rejects them too); 8 wrong |
| `c-c++-common/torture` | 282 | 172 | 184 | 180 of 278 | the same GCC-only builtins (96 rejects); 2 wrong (`__builtin_clear_padding`) |

### g++.dg/reflect wrong-line cases (WP-1515)

Hand-written from the WP-1515 re-run (49 cases). Classes: (a) kiln
reports where clang reports the non-reflection analogue (at the use, the
initialized variable, the call or the instantiation, with a note at the
line GCC expects): stays; (b) a wrong or useless location: none found;
(c) an error GCC reports and kiln misses or a different error; (d) GCC
16.1 follows P2996R13's consteval-only-type rules, which the working
draft replaced (P4101R1, docs/status/deviations.md): stays; (e) the text
decides against GCC: stays.

| class | cases | tests |
|---|---:|---|
| (a) error at the use, call or instantiation | 21 | `bitfield1`, `complete1`, `crash13`, `crash19`, `crash20`, `crash21`, `crash22`, `crash23`, `current_class2`, `define_aggregate3`, `define_aggregate7`, `error11`, `error8`, `extract8`, `init2`, `p2996-21`, `p3491-2`, `reflect_constant6`, `reflect_constant_array11`, `reflect_constant_array2`, `type7` |
| (a) error inside the libstdc++ header (`tuple`, `variant` static_assert) | 2 | `tuple2`, `variant2` |
| (d) P4101R1 consteval-only values instead of types (functions and variables of consteval-only type, override) | 10 | `crash12`, `init10`, `init11`, `init6`, `init7`, `init9`, `override1`, `parm3`, `parm4`, `type2` |
| (e) text against GCC | 2 | `qrn1` (`^^N::M::template NMCC` where `NMCC` is an alias, not a template: [expr.reflect]/2; clang rejects too), `annotations1` (annotation on a lambda call operator) |
| (c) missed or different error, singletons | 14 | `annotations17` (annotation on a parameter of a block-scope extern function), `annotations4` (non-structural annotation on a function, L41), `annotations7` (annotation type not copy constructible, prvalue operand), `crash5` (cascade after `info` unknown), `current_function2` (reference return of a bad array type), `expr2` (`^^` of a requires-expression parameter), `mangle4` (`default_accessor` conversion; GCC's test says it is unclear), `ns4` (second `NOTHERE` lookup after the first error), `odr1` (splice of an enclosing function's local in a local class), `pack-index1` (`^^Ts...[0]`), `parse3` (L16), `pr124689` (a user-declared `std::meta` with wrong return types: QoI, kiln never misreads), `splice1` (splice of an unnamed bit-field), `splicing-base4` (base splice with an unprivileged context) |

No (b) or (c) cluster reaches three cases. The missed errors of WP-1505
item (4), recounted (39 missed in wrong-line cases plus 7
accepts-invalid): consteval-only checks 15, all (d) (9 wrong-line, plus
`crash18`, `init12`, `init4`, `init5`, `p2996-15`, `pr124012`);
"expected a reflection of" argument checks 3 (`crash19`, `crash22`,
`crash23`: reported at the call, (a)); `define_aggregate` checks 3 (a);
annotation checks 3 (c); constant-evaluation errors reported at the use
7 (a); others 14. `bit_cast` (accepts-invalid) is GCC's eager error;
kiln follows [bit.cast]/2 (deviations.md).

### g++.dg/reflect compile-reject cases (WP-1537)

Hand-written from the WP-1537 triage of the 60 compile-reject cases of the
base re-run (448 of 565 then): (K) kiln wrong, (G) GCC 16.1 wrong or on
older wording (rows in docs/status/deviations.md "GCC 16.1 reflection
divergences"), (U) unspecified, (T) outside reflection. After WP-1537:
462 of 565 pass (14 compile-reject cases now pass, none regressed; 46
compile-reject left). WP-1537 fixed: the
identifier of a base relationship and of a parameter from a pack
(`bases_of1`, `has_identifier1`), `data_member_spec` annotations (`data_member_spec5`,
`mangle1`, `display_string_of1` L122), defaulted comparisons by value with an
explicit object parameter (`members_of6` L18), `>=` after a splice in a
template argument (`parse1`), `type_of` of a structured binding and of an
enumerator inside its enum-specifier (`type_of1`), `is_final` of a class being
defined (`is_final1`), variant members in `is_accessible` and `^^A::f` of a
protected member (`is_accessible2`, `member-visibility2`), `extract` of a
pointer variable and of a closure as a function pointer (`extract1`,
`extract4`), extern "C" default arguments per namespace (`p2996-9`),
`access_context::current()` in an inherited constructor's default member
initializer and in a trailing requires-clause (`p2996-18`), a base
splice through the designated direct base (`splicing-base1`), a constructor
parameter's splice-type-specifier (`type3`), the built-in `==` for
`std::meta::info` in a defaulted comparison (`cwg3153`), a dependent member
template-id reflected in a class template (`dep13`), and CWG3149 xvalue
elements (`cwg3149`, now G only).

| class | cases |
|---|---|
| K left (xfail repros under rows WP-1613 and WP-1614) | `member15`, `member17` (member splice rebuilt by name in templates), `enumerators_of1`, `is_enumerable_type1` (local opaque enum in a function template), `type_trait6` L781-783 (abominable function type in `is_assignable`), `reflect_constant9` L33, `reflect_function2` L74, `p2996-7` (dependent namespace alias), `variable_of2`, `define_aggregate10` (CWG3162), `visibility1` (substituted template arguments not constant-evaluated), `parse2` (constexpr expansion over a prvalue in a template), `json-parser` (`define_aggregate` refused under `substitute`; also `#embed` pedantic) |
| G | `members_of6` L30/34, `is_accessible2` L454-465, `mangle3` (alias template named as its parameter, [temp.local]/6; splice of a variable template without `template`), `type_trait6` L988 and `is_constructible_type1` L606 (P3074), `type_rels1` L41-42, `reflect_constant9` L29/32, `expansion-stmt1` / `cwg3149` / `parse2` (body not a compound-statement), `eh9` (IFNDR, diagnosed), `eh5` `qux`, `expr7` (`using enum`), `extract11`, `is_constructible_type1` L230-234 and L667-671; existing rows: `has_linkage1`, `has_external_linkage1`, `has_c_language_linkage1`, `has_parent1`, `parent_of1`, `constant_of7`, `reflect_constant_array1`, `reflect_constant_string1`, `data_member_spec4`, `type_trait5` |
| U | display strings (`display_string_of1`), `eh5` `what()` texts, `source_location_of1`, `source_location_of2`, `reflect_constant7` |
| T | `::nullptr_t` without `<cstddef>` (`extract1` L9-11, `reflect_constant5`, `type_trait1`), `variable_of3` (`__integer_pack`), `members_of11` (`__builtin_constexpr_diag`), `init17` and `has_identifier1` L223 (statement expressions) |

After WP-1613 and WP-1614: 468 of 565 g++.dg/reflect cases pass (8 compile-reject cases now pass: `member15`, `member17`, `enumerators_of1`, `is_enumerable_type1`, `p2996-7`, `reflect_function2`, `variable_of2`, `visibility1`; none regressed in g++.dg/reflect, g++.dg/template or g++.dg/ext/is_*). Of the K row, `define_aggregate10` (CWG3162 injected declarations) is left; `type_trait6`, `reflect_constant9` and `parse2` now fail only on their G lines, and `json-parser` only on `#embed` pedantic.

### x86-64

The first x86-64 run: the opt-in `gxx-dg` jobs of `.github/workflows/sweep.yml`
(docs/VALIDATION.md, "x86_64 sweep") at the same kiln `9eb066486`,
hosted run 43964183097, four shards of 4:51 to 7:18 each, peak RSS 1.93
GB per shard. The CI reference container has the same oracles as this
host (GCC 16.1.0, clang 22.1.8) and the same GCC 16.1 libstdc++.

**49708 of 57070 applicable cases pass (87.1%)**; 57749 rows against
57552 on aarch64. The 197 extra rows are tests whose `dg-do` target or
effective target selects one architecture: on the other host such a
test is a single skip row (`*`), on its own host one row per standard
or optimization level. 117 tests are skip rows on aarch64 and run on
x86-64, 10 the other way; per-target `dg-options` change a few more.

Of the 57408 cases both hosts ran, 9 pass on aarch64 and fail on
x86-64, 3 the other way; no case moves between two failing classes:

- target-specific by the test's own directives: `abi/pr87137`
  (`ms_struct` layout, 3), `cpp1z/register1`/`register3` (global
  register variable in `ebx`, 3), with clang failing the same way;
  `cpp0x/pr77948-1`/`3`/`4` pass on x86-64 only (the `q` literal suffix
  GCC expects to be unsupported on aarch64, 3);
- the target ABI: `torture/pr35526` (plain `char` is signed on x86-64,
  so a constant above 127 narrows; clang the same, 2);
- kiln alone: `g++.old-deja/g++.eh/unwind1` at C++26 exits 1 on x86-64
  (GCC and clang pass). C++26 zero-initializes the `__builtin_alloca`
  memory the test reads its stack pointer from, through an
  `llvm.memset` that becomes a `memset` call at `-O0`; not reduced
  further, since x86-64 code cannot run on the aarch64 host (WP-1544).

Of the cases only x86-64 runs, kiln alone fails 9: `ext/pr106877`,
`ext/pr84683` (accepted) and `ext/pr84828` (llc aborts with
`-mno-sse2`) are one cause, an x87 read-write output (`"+f"`) that
clang 22 rejects ("invalid output constraint"); xfail repro
`dcl.asm/x87-read-write-output-x86_64.cpp` (WP-1543). The other x86-64-only
failures fail under clang too: GCC's `__builtin_ia32_*` builtins,
`__float80`, `target` multiversioning, global register variables, and
the `abi/empty*` tests that link a companion file the runner does not
compile (GCC fails them too). The aarch64-only failures are the
`sve-sizeless` diagnostics (6, clang the same).

`x86_avx_abi_interop` (WP-1492, WP-1512 item 6) is a `cargo test` of
`crates/kiln_driver/tests/codegen.rs`, which the `gxx-dg` jobs do not
build; CI's x86_64 lane does, and it passed there (`x86_64 / unit
rest`, main `414c95191`).

### What the run does not cover

- Message text is not compared, so a wrong diagnostic on the right line
  passes. `scan-*` and `dg-output` checks are not compared.
- The modules, gomp, lto, debug and sanitizer directories, and
  `coroutines/torture`, run under their own `.exp` and are not run here.
- Only `-O0` and `-O2` for the torture suites. Two targets: aarch64 here
  and x86-64 in the opt-in sweep jobs; tests for other targets are skip
  rows.
- Oracles run only on the cases kiln fails, so the GCC and clang columns
  cover those cases only (`--oracles-on all` runs every case, in about
  three times the wall time).

<!-- gxx-dg:begin (generated by cargo xtask gxx-dg) -->

GCC C++ testsuite (`g++.dg`, `c-c++-common`, `g++.old-deja`, `releases/gcc-16.1.0`), host target `aarch64-unknown-linux-gnu`.

- tests: GCC `6afcc4f6da931eb93f3ab001a0dd9650ea71d1ea`; kiln `9eb066486`
- command: `cargo xtask gxx-dg -j 8 --suite dg,torture,common,old-deja --opt -O0,-O2 --profile release`
- kiln: kiln version 1.2.1 (C++26 frontend, LLVM IR backend) (-ferror-limit=0) (-fclang-compat=22) (--gcc-toolchain=<GCC oracle's prefix>)
- gcc: g++ (GCC) 16.1.0 (-fdiagnostics-plain-output)
- clang: Debian clang version 22.1.8 (1~deb13u4) (-ferror-limit=0) (--gcc-toolchain=<GCC oracle's prefix>)
- oracles: only the cases kiln does not pass (their counts below cover those cases)
- limits: 2048 MiB address space per step (plus kiln's stack reservation); compile/link timeout 60 s, run timeout 10 s; -j 8
- 20571 tests, 56766 cases run, 786 skip rows; wall 1526 s

### Results by class (cases)

| class | kiln | gcc | clang |
|---|---:|---:|---:|
| pass | 49496 | 6981 | 1591 |
| compile-reject | 2875 | 210 | 2534 |
| crash | 3 | 3 | 14 |
| wrong | 144 | 2 | 109 |
| timeout | 3 | 0 | 0 |
| accept-invalid | 1196 | 34 | 600 |
| wrong-line | 3049 | 40 | 2422 |
| total | 56766 | 7270 | 7270 |

### kiln's failures by triage bucket (cases)

| bucket | cases | GCC's testsuite marks xfail |
|---|---:|---:|
| kiln crash / hang | 6 | 0 |
| wrong-code | 144 | 2 |
| rejects-valid, clang accepts | 355 | 3 |
| rejects-valid, clang rejects too | 2520 | 0 |
| accepts-invalid, clang rejects | 638 | 0 |
| accepts-invalid, clang accepts too | 558 | 0 |
| wrong-line, clang matches | 686 | 0 |
| wrong-line, clang differs too | 2363 | 0 |

### Priority directories (kiln, cases)

| directory | cases | pass | compile-reject | accept-invalid | wrong-line | wrong | crash | timeout |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| g++.dg/reflect | 565 | 447 | 61 | 8 | 49 | 0 | 0 | 0 |
| g++.dg/contracts | 123 | 100 | 4 | 0 | 19 | 0 | 0 | 0 |
| g++.dg/cpp26 | 435 | 309 | 34 | 6 | 83 | 3 | 0 | 0 |
| g++.dg/cpp2a | 3784 | 3074 | 204 | 94 | 412 | 0 | 0 | 0 |
| g++.dg/cpp23 | 587 | 440 | 39 | 28 | 78 | 2 | 0 | 0 |
| g++.dg/torture | 1149 | 1077 | 62 | 2 | 0 | 8 | 0 | 0 |
| c-c++-common/torture | 282 | 184 | 96 | 0 | 0 | 2 | 0 | 0 |

### By directory (kiln, cases)

| directory | cases | pass | compile-reject | accept-invalid | wrong-line | wrong | crash | timeout |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| c-c++-common | 2866 | 2464 | 221 | 101 | 67 | 13 | 0 | 0 |
| c-c++-common/cpp | 420 | 306 | 60 | 18 | 36 | 0 | 0 | 0 |
| c-c++-common/torture | 282 | 184 | 96 | 0 | 0 | 2 | 0 | 0 |
| g++.dg | 942 | 873 | 37 | 8 | 24 | 0 | 0 | 0 |
| g++.dg/DRs | 203 | 158 | 6 | 8 | 31 | 0 | 0 | 0 |
| g++.dg/abi | 772 | 711 | 44 | 5 | 3 | 9 | 0 | 0 |
| g++.dg/concepts | 409 | 279 | 45 | 8 | 77 | 0 | 0 | 0 |
| g++.dg/contracts | 123 | 100 | 4 | 0 | 19 | 0 | 0 | 0 |
| g++.dg/conversion | 279 | 246 | 4 | 3 | 26 | 0 | 0 | 0 |
| g++.dg/coroutines | 533 | 417 | 32 | 15 | 69 | 0 | 0 | 0 |
| g++.dg/cpp | 213 | 177 | 10 | 7 | 18 | 1 | 0 | 0 |
| g++.dg/cpp0x | 9121 | 7876 | 478 | 168 | 587 | 12 | 0 | 0 |
| g++.dg/cpp1y | 2133 | 1779 | 111 | 37 | 203 | 3 | 0 | 0 |
| g++.dg/cpp1z | 1707 | 1473 | 84 | 50 | 96 | 4 | 0 | 0 |
| g++.dg/cpp23 | 587 | 440 | 39 | 28 | 78 | 2 | 0 | 0 |
| g++.dg/cpp26 | 435 | 309 | 34 | 6 | 83 | 3 | 0 | 0 |
| g++.dg/cpp2a | 3784 | 3074 | 204 | 94 | 412 | 0 | 0 | 0 |
| g++.dg/delayedfold | 21 | 15 | 3 | 0 | 0 | 3 | 0 | 0 |
| g++.dg/diagnostic | 579 | 458 | 24 | 49 | 45 | 3 | 0 | 0 |
| g++.dg/eh | 394 | 362 | 7 | 15 | 7 | 3 | 0 | 0 |
| g++.dg/expr | 406 | 380 | 15 | 6 | 5 | 0 | 0 | 0 |
| g++.dg/ext | 2901 | 2182 | 379 | 105 | 192 | 43 | 0 | 0 |
| g++.dg/inherit | 337 | 311 | 8 | 3 | 15 | 0 | 0 | 0 |
| g++.dg/init | 1101 | 992 | 32 | 20 | 53 | 4 | 0 | 0 |
| g++.dg/ipa | 686 | 673 | 13 | 0 | 0 | 0 | 0 | 0 |
| g++.dg/lookup | 1025 | 887 | 54 | 41 | 43 | 0 | 0 | 0 |
| g++.dg/opt | 1442 | 1381 | 43 | 3 | 0 | 15 | 0 | 0 |
| g++.dg/other | 1033 | 922 | 34 | 11 | 63 | 0 | 0 | 3 |
| g++.dg/overload | 294 | 252 | 18 | 6 | 15 | 0 | 3 | 0 |
| g++.dg/parse | 1851 | 1495 | 73 | 72 | 211 | 0 | 0 | 0 |
| g++.dg/reflect | 565 | 447 | 61 | 8 | 49 | 0 | 0 | 0 |
| g++.dg/rtti | 118 | 106 | 0 | 3 | 9 | 0 | 0 | 0 |
| g++.dg/tc1 | 105 | 84 | 12 | 0 | 9 | 0 | 0 | 0 |
| g++.dg/template | 4699 | 4053 | 220 | 173 | 250 | 3 | 0 | 0 |
| g++.dg/torture | 1149 | 1077 | 62 | 2 | 0 | 8 | 0 | 0 |
| g++.dg/tree-ssa | 860 | 841 | 16 | 0 | 3 | 0 | 0 | 0 |
| g++.dg/warn | 2840 | 2712 | 91 | 16 | 18 | 3 | 0 | 0 |
| g++.dg/wrappers | 30 | 23 | 7 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.abi | 111 | 108 | 3 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.benjamin | 126 | 108 | 9 | 0 | 9 | 0 | 0 | 0 |
| g++.old-deja/g++.bob | 54 | 50 | 0 | 1 | 3 | 0 | 0 | 0 |
| g++.old-deja/g++.brendan | 919 | 885 | 9 | 16 | 9 | 0 | 0 | 0 |
| g++.old-deja/g++.bugs | 286 | 270 | 0 | 5 | 11 | 0 | 0 | 0 |
| g++.old-deja/g++.eh | 207 | 206 | 0 | 0 | 1 | 0 | 0 | 0 |
| g++.old-deja/g++.ext | 117 | 75 | 28 | 2 | 6 | 6 | 0 | 0 |
| g++.old-deja/g++.gb | 39 | 38 | 0 | 0 | 1 | 0 | 0 | 0 |
| g++.old-deja/g++.jason | 1217 | 1168 | 11 | 10 | 28 | 0 | 0 | 0 |
| g++.old-deja/g++.law | 771 | 729 | 21 | 10 | 11 | 0 | 0 | 0 |
| g++.old-deja/g++.martin | 54 | 49 | 5 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.mike | 1040 | 1004 | 33 | 0 | 3 | 0 | 0 | 0 |
| g++.old-deja/g++.niklas | 81 | 81 | 0 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.ns | 273 | 264 | 0 | 0 | 9 | 0 | 0 | 0 |
| g++.old-deja/g++.oliva | 114 | 96 | 6 | 8 | 3 | 1 | 0 | 0 |
| g++.old-deja/g++.other | 1509 | 1420 | 23 | 12 | 54 | 0 | 0 | 0 |
| g++.old-deja/g++.pt | 2177 | 2034 | 42 | 39 | 59 | 3 | 0 | 0 |
| g++.old-deja/g++.rfg | 6 | 6 | 0 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.robertl | 402 | 368 | 4 | 4 | 26 | 0 | 0 | 0 |
| g++.old-deja/g++.warn | 18 | 18 | 0 | 0 | 0 | 0 | 0 | 0 |
| total | 56766 | 49496 | 2875 | 1196 | 3049 | 144 | 3 | 3 |

### Clusters: kiln crash / hang (2 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 3 | crash: compile: signal: N (SIGABRT) | g++.dg/overload/ellipsis3.C -std=c++20, g++.dg/overload/ellipsis3.C -std=c++26, g++.dg/overload/ellipsis3.C -std=c++98 |
| 3 | timeout: compile timeout | g++.dg/other/pr117516.C -std=c++20, g++.dg/other/pr117516.C -std=c++26, g++.dg/other/pr117516.C -std=c++98 |

### Clusters: wrong-code (3 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 101 | wrong: signal: N (SIGABRT) | c-c++-common/attr-aligned-1.c -std=c++98, c-c++-common/auto-init-padding-3.c -std=gnu++20, c-c++-common/auto-init-padding-3.c -std=gnu++26 |
| 40 | wrong: exit N | g++.dg/abi/bitfield1.C -std=gnu++20, g++.dg/abi/bitfield1.C -std=gnu++26, g++.dg/abi/bitfield1.C -std=gnu++98 |
| 3 | wrong: signal: N (SIGSEGV) | g++.dg/ext/label13a.C -std=gnu++20, g++.dg/ext/label13a.C -std=gnu++26, g++.dg/ext/label13a.C -std=gnu++98 |

### Clusters: rejects-valid, clang accepts (104 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 12 | compile-reject: error: functions that differ only in their return type cannot be overloaded | g++.dg/ext/attrib63.C -std=c++20, g++.dg/ext/attrib63.C -std=c++26, g++.dg/ext/attrib63.C -std=c++98 |
| 9 | compile-reject: link: undefined reference to `… | c-c++-common/restrict-1.c -std=gnu++20, c-c++-common/restrict-1.c -std=gnu++26, c-c++-common/restrict-1.c -std=gnu++98 |
| 7 | compile-reject: error: call to deleted function '…' | g++.dg/cpp0x/sfinae70.C -std=c++26, g++.dg/cpp0x/sfinae71.C -std=c++26, g++.dg/cpp0x/sfinae72.C -std=c++26 |
| 7 | compile-reject: error: the GNU '…' operator on class types is not supported yet | g++.dg/ext/cond3.C -std=gnu++20, g++.dg/ext/cond3.C -std=gnu++26, g++.dg/ext/cond3.C -std=gnu++98 |
| 6 | compile-reject: error: by-copy capture of variable '…' with incomplete type '…' | g++.dg/cpp0x/lambda/lambda-70383.C -std=c++11, g++.dg/cpp0x/lambda/lambda-70383.C -std=c++20, g++.dg/cpp0x/lambda/lambda-70383.C -std=c++26 |
| 6 | compile-reject: error: cannot yet mangle expression type UnresolvedLookupExpr | g++.dg/cpp1y/lambda-ice3.C -std=c++14, g++.dg/cpp1y/lambda-ice3.C -std=c++20, g++.dg/cpp1y/lambda-ice3.C -std=c++26 |
| 6 | compile-reject: error: comparison of distinct pointer types ('…' and '…') | g++.old-deja/g++.other/cast7.C -std=c++20, g++.old-deja/g++.other/cast7.C -std=c++26, g++.old-deja/g++.other/cast7.C -std=c++98 |
| 6 | compile-reject: error: expected '…' after namespace name | g++.dg/lookup/strong-using.C -std=c++11, g++.dg/lookup/strong-using.C -std=c++20, g++.dg/lookup/strong-using.C -std=c++26 |
| 6 | compile-reject: error: expected namespace name | g++.dg/cpp1y/pr59867.C -std=gnu++14, g++.dg/cpp1y/pr59867.C -std=gnu++20, g++.dg/cpp1y/pr59867.C -std=gnu++26 |
| 6 | compile-reject: error: extraneous template parameter list in template specialization or out-of-line template definition | g++.dg/template/friend17.C -std=c++20, g++.dg/template/friend17.C -std=c++26, g++.dg/template/friend17.C -std=c++98 |
| 6 | compile-reject: error: invalid vector element type '…' | g++.dg/cpp0x/alias-decl-79.C -std=c++11, g++.dg/cpp0x/alias-decl-79.C -std=c++20, g++.dg/cpp0x/alias-decl-79.C -std=c++26 |
| 6 | compile-reject: error: kiln: not yet implemented: CXXDeductionGuide members of class templates | g++.dg/cpp1z/class-deduction106.C -std=c++17, g++.dg/cpp1z/class-deduction106.C -std=c++20, g++.dg/cpp1z/class-deduction106.C -std=c++26 |
| 6 | compile-reject: error: kiln: not yet implemented: dependent nested name specifier | g++.dg/template/friend46.C -std=c++20, g++.dg/template/friend46.C -std=c++26, g++.dg/template/friend46.C -std=c++98 |
| 6 | compile-reject: error: kiln: not yet implemented: member templates | g++.dg/template/friend61.C -std=c++20, g++.dg/template/friend61.C -std=c++26, g++.dg/template/friend61.C -std=c++98 |
| 6 | compile-reject: error: member '…' found in multiple base classes of different types | g++.dg/tc1/dr176-2.C -std=c++20, g++.dg/tc1/dr176-2.C -std=c++26, g++.dg/tc1/dr176-2.C -std=c++98 |
| 6 | compile-reject: error: no function template matches function template specialization '…' | g++.dg/Wmissing-attributes-1.C -std=gnu++20, g++.dg/Wmissing-attributes-1.C -std=gnu++26, g++.dg/Wmissing-attributes-1.C -std=gnu++98 |
| 6 | compile-reject: error: no member named '…' in '…' | g++.dg/template/operator2.C -std=c++20, g++.dg/template/operator2.C -std=c++26, g++.dg/template/operator2.C -std=c++98 |
| 6 | compile-reject: error: no template named '…' | g++.dg/template/crash133.C -std=c++20, g++.dg/template/crash133.C -std=c++26, g++.dg/template/crash133.C -std=c++98 |
| 6 | compile-reject: error: non-type template parameter type contains unexpanded parameter pack '…' | g++.dg/cpp0x/variadic152.C -std=c++11, g++.dg/cpp0x/variadic152.C -std=c++20, g++.dg/cpp0x/variadic152.C -std=c++26 |
| 6 | compile-reject: error: overload resolution selected deleted operator '…' | g++.dg/lookup/operator-8.C -std=c++20, g++.dg/lookup/operator-8.C -std=c++23, g++.dg/lookup/operator-8.C -std=c++26 |
| 6 | compile-reject: error: partial ordering for explicit instantiation of '…' is ambiguous | g++.dg/template/partial9.C -std=c++20, g++.dg/template/partial9.C -std=c++26, g++.dg/template/partial9.C -std=c++98 |
| 6 | compile-reject: error: size of array has non-integer type '…' | g++.dg/cpp0x/constexpr-ex4.C -std=c++11, g++.dg/cpp0x/constexpr-ex4.C -std=c++20, g++.dg/cpp0x/constexpr-ex4.C -std=c++26 |
| 6 | compile-reject: error: unexpected type name '…': expected expression | g++.dg/ext/builtin-structured-binding-size4.C -std=c++20, g++.dg/ext/builtin-structured-binding-size4.C -std=c++23, g++.dg/ext/builtin-structured-binding-size4.C -std=c++26 |
| 5 | compile-reject: error: expression is not an integral constant expression | g++.dg/template/conv21.C -std=c++11, g++.dg/template/conv21.C -std=c++20, g++.dg/template/conv21.C -std=c++26 |
| 4 | compile-reject: error: call to '…' is ambiguous | g++.dg/abi/mangle79.C -std=gnu++26, g++.dg/cpp0x/sfinae68.C -std=c++11, g++.dg/cpp0x/sfinae68.C -std=c++20 |
| 3 | compile-reject: error: "-fstack-protector not enabled" | c-c++-common/fhardened-2.c -std=gnu++20, c-c++-common/fhardened-2.c -std=gnu++26, c-c++-common/fhardened-2.c -std=gnu++98 |
| 3 | compile-reject: error: "Printed"  // { dg-warning "\"Printed\" .-Wcpp." } [-Werror,-W#warnings] | c-c++-common/cpp/warning-directive-3.c -std=gnu++20, c-c++-common/cpp/warning-directive-3.c -std=gnu++26, c-c++-common/cpp/warning-directive-3.c -std=gnu++98 |
| 3 | compile-reject: error: '…' attribute requires parameter N to be an integer constant | g++.dg/ext/attr-format4.C -std=gnu++11, g++.dg/ext/attr-format4.C -std=gnu++20, g++.dg/ext/attr-format4.C -std=gnu++26 |
| 3 | compile-reject: error: '…' does not refer to a value | g++.dg/template/using17.C -std=c++20, g++.dg/template/using17.C -std=c++26, g++.dg/template/using17.C -std=c++98 |
| 3 | compile-reject: error: '…' is missing exception specification '…' | g++.dg/ext/attr-nothrow.C -std=gnu++20, g++.dg/ext/attr-nothrow.C -std=gnu++26, g++.dg/ext/attr-nothrow.C -std=gnu++98 |

### Clusters: rejects-valid, clang rejects too (361 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 265 | compile-reject: error: use of undeclared identifier '…' | c-c++-common/Wsuggest-attribute-1.c -std=gnu++11, c-c++-common/Wsuggest-attribute-1.c -std=gnu++20, c-c++-common/Wsuggest-attribute-1.c -std=gnu++26 |
| 68 | compile-reject: error: static assertion failed due to requirement '…' | g++.dg/abi/no_unique_address8a.C -std=gnu++11, g++.dg/abi/no_unique_address8a.C -std=gnu++20, g++.dg/abi/no_unique_address8a.C -std=gnu++26 |
| 54 | compile-reject: error: constexpr variable '…' must be initialized by a constant expression | g++.dg/cpp0x/constexpr-92443.C -std=c++11, g++.dg/cpp0x/constexpr-92443.C -std=c++20, g++.dg/cpp0x/constexpr-92443.C -std=c++26 |
| 53 | compile-reject: error: a type specifier is required for all declarations | g++.dg/abi/vbase8-10.C -std=gnu++20, g++.dg/abi/vbase8-10.C -std=gnu++26, g++.dg/abi/vbase8-10.C -std=gnu++98 |
| 53 | compile-reject: error: unknown type name '…' | g++.dg/concepts/abbrev11.C -std=c++14, g++.dg/concepts/class5.C -std=gnu++17, g++.dg/concepts/debug1.C -std=gnu++17 |
| 42 | compile-reject: error: '…' is a CN extension [-Werror,-WcN-extensions] | g++.dg/cpp0x/constexpr-71828.C -std=c++11, g++.dg/cpp0x/constexpr-71828.C -std=c++20, g++.dg/cpp0x/constexpr-71828.C -std=c++26 |
| 40 | compile-reject: error: expected '…' | c-c++-common/toplevel-asm-1.c -std=gnu++20, c-c++-common/toplevel-asm-1.c -std=gnu++26, c-c++-common/toplevel-asm-1.c -std=gnu++98 |
| 37 | compile-reject: error: no matching function for call to '…' | g++.dg/abi/noreturn1.C -std=gnu++20, g++.dg/abi/noreturn1.C -std=gnu++26, g++.dg/abi/noreturn1.C -std=gnu++98 |
| 37 | compile-reject: link: undefined reference to `… | c-c++-common/pr19807-2.c -std=gnu++20, c-c++-common/pr19807-2.c -std=gnu++26, c-c++-common/pr19807-2.c -std=gnu++98 |
| 36 | compile-reject: error: invalid suffix on literal; C++N requires a space between literal and identifier [-Wreserved-user-defined-literal] | c-c++-common/cpp/dir-only-7.c -, g++.dg/cpp0x/Wliteral-suffix.C -std=c++11, g++.dg/cpp0x/Wliteral-suffix.C -std=c++20 |
| 34 | compile-reject: error: ISO C++N does not allow access declarations; use using declarations instead | g++.dg/cpp0x/pr70538.C -std=c++11, g++.dg/cpp0x/pr70538.C -std=c++20, g++.dg/cpp0x/pr70538.C -std=c++26 |
| 34 | compile-reject: error: static assertion expression is not an integral constant expression | g++.dg/cpp0x/constexpr-50060.C -std=c++14, g++.dg/cpp0x/constexpr-50060.C -std=c++20, g++.dg/cpp0x/constexpr-array-ptr6.C -std=c++11 |
| 30 | compile-reject: error: redefinition of '…' | c-c++-common/pr33763.c -std=gnu++20, c-c++-common/pr33763.c -std=gnu++26, c-c++-common/pr33763.c -std=gnu++98 |
| 30 | compile-reject: error: variable-sized object may not be initialized | g++.dg/cpp1y/lambda-vla1.C -std=c++14, g++.dg/cpp1y/lambda-vla1.C -std=c++20, g++.dg/cpp1y/lambda-vla1.C -std=c++26 |
| 28 | compile-reject: error: '…' attribute cannot be applied to types | g++.dg/cpp0x/alias-decl-59.C -std=c++11, g++.dg/cpp0x/alias-decl-59.C -std=c++20, g++.dg/cpp0x/alias-decl-59.C -std=c++26 |
| 27 | compile-reject: error: use '…' keyword to treat '…' as a dependent template name | g++.dg/cpp0x/decltype34.C -std=c++11, g++.dg/cpp0x/decltype34.C -std=c++20, g++.dg/cpp0x/decltype34.C -std=c++26 |
| 25 | compile-reject: error: pack expansion does not contain any unexpanded parameter packs | g++.dg/cpp0x/constexpr-96241.C -std=c++11, g++.dg/cpp0x/constexpr-96241.C -std=c++20, g++.dg/cpp0x/constexpr-96241.C -std=c++26 |
| 25 | compile-reject: error: static assertion failed due to requirement '…':  | g++.dg/abi/base-defaulted1a.C -std=c++20, g++.dg/abi/base-defaulted1a.C -std=c++26, g++.dg/cpp0x/initlist-ref3.C -std=c++11 |
| 23 | compile-reject: error: expected unqualified-id | c-c++-common/pr92833-4.c -std=c++20, c-c++-common/pr92833-4.c -std=c++26, c-c++-common/pr92833-4.c -std=c++98 |
| 21 | compile-reject: error: cannot initialize a variable of type '…' with an rvalue of type '…' | c-c++-common/pr60101.c -std=gnu++20, c-c++-common/pr60101.c -std=gnu++26, c-c++-common/pr60101.c -std=gnu++98 |
| 21 | compile-reject: error: character <U+NA> not allowed in an identifier | c-c++-common/Wbidi-chars-10.c -std=gnu++20, c-c++-common/Wbidi-chars-10.c -std=gnu++26, c-c++-common/Wbidi-chars-10.c -std=gnu++98 |
| 20 | compile-reject: error: '…' not allowed in function prototype | g++.dg/concepts/abbrev1.C -std=c++14, g++.dg/concepts/abbrev10.C -std=c++14, g++.dg/concepts/abbrev2.C -std=c++14 |
| 18 | compile-reject: error: an attribute list cannot appear here | g++.dg/DRs/dr2262.C -std=gnu++11, g++.dg/DRs/dr2262.C -std=gnu++20, g++.dg/DRs/dr2262.C -std=gnu++26 |
| 17 | compile-reject: error: reference member '…' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object | g++.dg/cpp0x/constexpr-ctor11.C -std=c++11, g++.dg/cpp0x/constexpr-ctor11.C -std=c++20, g++.dg/cpp0x/constexpr-ctor11.C -std=c++26 |
| 16 | compile-reject: error: '…' cannot be defined in a type specifier | g++.dg/ext/complit15.C -, g++.dg/opt/pr20931.C -std=gnu++20, g++.dg/opt/pr20931.C -std=gnu++26 |
| 16 | compile-reject: error: implicit instantiation of undefined template '…' | g++.dg/cpp0x/alias-decl-dr1286.C -std=c++11, g++.dg/cpp0x/alias-decl-dr1286.C -std=c++20, g++.dg/cpp0x/alias-decl-dr1286.C -std=c++26 |
| 15 | compile-reject: error: '…' is a private member of '…' | g++.dg/template/inherit9.C -std=c++20, g++.dg/template/inherit9.C -std=c++26, g++.dg/template/inherit9.C -std=c++98 |
| 15 | compile-reject: error: first two arguments to '…' must have the same type | c-c++-common/torture/builtin-convertvector-2.c -O0, c-c++-common/torture/builtin-convertvector-2.c -O2, c-c++-common/torture/builtin-shufflevector-1.c -O0 |
| 15 | compile-reject: error: structured binding declaration cannot be declared '…' | g++.dg/cpp26/decomp22.C -std=gnu++11, g++.dg/cpp26/decomp22.C -std=gnu++14, g++.dg/cpp26/decomp22.C -std=gnu++20 |
| 15 | compile-reject: error: type '…' cannot be used prior to '…' because it has no members | g++.dg/cpp0x/fntmpdefarg5.C -std=c++11, g++.dg/cpp0x/fntmpdefarg5.C -std=c++20, g++.dg/cpp0x/fntmpdefarg5.C -std=c++26 |

### Clusters: accepts-invalid, clang rejects (225 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 106 | accept-invalid: no error (expected: ) | g++.dg/cpp0x/alias-decl-58.C -std=c++11, g++.dg/cpp0x/alias-decl-58.C -std=c++20, g++.dg/cpp0x/alias-decl-58.C -std=c++26 |
| 6 | accept-invalid: no error (expected: member template) | g++.dg/parse/dtor16.C -std=c++20, g++.dg/parse/dtor16.C -std=c++26, g++.dg/parse/dtor16.C -std=c++98 |
| 5 | accept-invalid: no error (expected: invalid use of '…') | g++.dg/cpp2a/lambda-generic3.C -std=gnu++98, g++.dg/cpp2a/lambda-generic4.C -std=gnu++98, g++.dg/lookup/this2.C -std=c++11 |
| 5 | accept-invalid: no error (expected: use of deleted function) | g++.dg/cpp0x/move-return5.C -std=c++11, g++.dg/cpp0x/move-return5.C -std=c++20, g++.dg/cpp1z/inline-var11.C -std=c++17 |
| 4 | accept-invalid: no error (expected: comma at end) | g++.old-deja/g++.brendan/enum11.C -std=c++98, g++.old-deja/g++.brendan/enum8.C -std=c++98, g++.old-deja/g++.brendan/enum9.C -std=c++98 |
| 4 | accept-invalid: no error (expected: not a class) | g++.dg/cpp0x/alias-decl-80.C -std=c++11, g++.dg/cpp0x/alias-decl-80.C -std=c++20, g++.dg/cpp0x/alias-decl-80.C -std=c++26 |
| 4 | accept-invalid: no error (expected: not a constant expression) | g++.dg/cpp2a/concepts-pr67697.C -std=c++20, g++.dg/cpp2a/concepts-pr67697.C -std=c++23, g++.dg/cpp2a/concepts-pr67697.C -std=c++26 |
| 3 | accept-invalid: no error (expected: '…' has no member named '…') | g++.dg/coroutines/pr112341-2.C -std=c++20, g++.dg/coroutines/pr112341-2.C -std=c++23, g++.dg/coroutines/pr112341-2.C -std=c++26 |
| 3 | accept-invalid: no error (expected: '…' is an inaccessible base of '…'\|conversion) | g++.dg/overload/pmf1.C -std=c++20, g++.dg/overload/pmf1.C -std=c++26, g++.dg/overload/pmf1.C -std=c++98 |
| 3 | accept-invalid: no error (expected: '…' is unavailable) | g++.dg/ext/attr-unavailable-3.C -std=gnu++20, g++.dg/ext/attr-unavailable-3.C -std=gnu++26, g++.dg/ext/attr-unavailable-3.C -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' names the constructor) | g++.dg/diagnostic/pr105483.C -std=c++20, g++.dg/diagnostic/pr105483.C -std=c++26, g++.dg/diagnostic/pr105483.C -std=c++98 |
| 3 | accept-invalid: no error (expected: '…' qualifiers cannot) | g++.dg/template/qualttp20.C -std=gnu++20, g++.dg/template/qualttp20.C -std=gnu++26, g++.dg/template/qualttp20.C -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' should be '…' to terminate a template argument list) | g++.dg/parse/template30.C -std=gnu++11, g++.dg/parse/template30.C -std=gnu++20, g++.dg/parse/template30.C -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…') | g++.dg/cpp0x/this2.C -std=c++11, g++.dg/cpp0x/this2.C -std=c++20, g++.dg/cpp0x/this2.C -std=c++26 |
| 3 | accept-invalid: no error (expected: (access)\|(context)) | g++.dg/lookup/scoped1.C -std=c++20, g++.dg/lookup/scoped1.C -std=c++26, g++.dg/lookup/scoped1.C -std=c++98 |
| 3 | accept-invalid: no error (expected: .co_await. cannot be used in default arguments) | g++.dg/coroutines/pr115906.C -std=c++20, g++.dg/coroutines/pr115906.C -std=c++23, g++.dg/coroutines/pr115906.C -std=c++26 |
| 3 | accept-invalid: no error (expected: .constinit. on function return type is not allowed) | g++.dg/cpp2a/constinit18.C -std=c++20, g++.dg/cpp2a/constinit18.C -std=c++23, g++.dg/cpp2a/constinit18.C -std=c++26 |
| 3 | accept-invalid: no error (expected: .constinit. specifier invalid for function) | g++.dg/cpp2a/constinit19.C -std=c++20, g++.dg/cpp2a/constinit19.C -std=c++23, g++.dg/cpp2a/constinit19.C -std=c++26 |
| 3 | accept-invalid: no error (expected: .operator delete. must return type .void'… | g++.dg/init/delete5.C -std=c++14, g++.dg/init/delete5.C -std=c++20, g++.dg/init/delete5.C -std=c++26 |
| 3 | accept-invalid: no error (expected: .template. keyword not permitted in destructor name) | g++.dg/template/template-keyword2.C -std=c++20, g++.dg/template/template-keyword2.C -std=c++26, g++.dg/template/template-keyword2.C -std=c++98 |
| 3 | accept-invalid: no error (expected: N:.void operator\\+\\(T, T\\) \\\[with T = int\\\]. must have an argument of cla…) | g++.dg/template/explicit-args3.C -std=c++20, g++.dg/template/explicit-args3.C -std=c++26, g++.dg/template/explicit-args3.C -std=c++98 |
| 3 | accept-invalid: no error (expected: N:attempt to use poisoned) | c-c++-common/cpp/pragma-diagnostic-loc.c -std=c++20, c-c++-common/cpp/pragma-diagnostic-loc.c -std=c++26, c-c++-common/cpp/pragma-diagnostic-loc.c -std=c++98 |
| 3 | accept-invalid: no error (expected: N:both .__thread. and .thread_local. specified) | g++.dg/diagnostic/thread-thread_local.C -std=gnu++11, g++.dg/diagnostic/thread-thread_local.C -std=gnu++20, g++.dg/diagnostic/thread-thread_local.C -std=gnu++26 |
| 3 | accept-invalid: no error (expected: N:cannot define member function '…' within '…') | g++.dg/parse/error29.C -std=gnu++20, g++.dg/parse/error29.C -std=gnu++26, g++.dg/parse/error29.C -std=gnu++98 |
| 3 | accept-invalid: no error (expected: N:data member may not have variably modified type) | g++.dg/diagnostic/variably-modified-type-1.C -std=gnu++20, g++.dg/diagnostic/variably-modified-type-1.C -std=gnu++26, g++.dg/diagnostic/variably-modified-type-1.C -std=gnu++98 |
| 3 | accept-invalid: no error (expected: N:decl-specifier) | g++.dg/cpp1z/class-deduction11.C -std=c++17, g++.dg/cpp1z/class-deduction11.C -std=c++20, g++.dg/cpp1z/class-deduction11.C -std=c++26 |
| 3 | accept-invalid: no error (expected: N:declaration of .operator\"\"_baddecl. as non-function) | g++.dg/cpp0x/udlit-nofunc-neg.C -std=c++11, g++.dg/cpp0x/udlit-nofunc-neg.C -std=c++20, g++.dg/cpp0x/udlit-nofunc-neg.C -std=c++26 |
| 3 | accept-invalid: no error (expected: N:declaration of .void K::f\\(int\\). outside of class) | g++.dg/diagnostic/out-of-class-redeclaration.C -std=c++20, g++.dg/diagnostic/out-of-class-redeclaration.C -std=c++26, g++.dg/diagnostic/out-of-class-redeclaration.C -std=c++98 |
| 3 | accept-invalid: no error (expected: N:declaration) | g++.dg/parse/error39.C -std=c++20, g++.dg/parse/error39.C -std=c++26, g++.dg/parse/error39.C -std=c++98 |
| 3 | accept-invalid: no error (expected: N:default arguments are not allowed in declaration of friend) | g++.dg/template/friend42.C -std=c++20, g++.dg/template/friend42.C -std=c++26, g++.dg/template/friend42.C -std=c++98 |

### Clusters: accepts-invalid, clang accepts too (155 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 41 | accept-invalid: no error (expected: ) | g++.dg/cpp0x/constexpr-ctor14a.C -std=gnu++11, g++.dg/cpp0x/constexpr-ctor14a.C -std=gnu++14, g++.dg/cpp1z/constexpr-lambda10.C -std=c++17 |
| 27 | accept-invalid: no error (expected: changes meaning) | g++.dg/ext/uow-3.C -std=gnu++20, g++.dg/ext/uow-3.C -std=gnu++26, g++.dg/ext/uow-3.C -std=gnu++98 |
| 18 | accept-invalid: no error (expected: '…' is not supported for this target) | c-c++-common/fcf-protection-1.c -std=gnu++20, c-c++-common/fcf-protection-1.c -std=gnu++26, c-c++-common/fcf-protection-1.c -std=gnu++98 |
| 12 | accept-invalid: no error (expected: conflicting) | g++.dg/eh/builtin10.C -std=c++20, g++.dg/eh/builtin10.C -std=c++26, g++.dg/eh/builtin10.C -std=c++98 |
| 9 | accept-invalid: no error (expected: '…' is not a member of '…') | g++.dg/coroutines/pr105475-1.C -std=c++20, g++.dg/coroutines/pr105475-1.C -std=c++23, g++.dg/coroutines/pr105475-1.C -std=c++26 |
| 9 | accept-invalid: no error (expected: ambiguous) | g++.dg/concepts/variadic2.C -std=gnu++26, g++.dg/cpp0x/overload-conv-5.C -std=c++20, g++.dg/cpp0x/overload-conv-5.C -std=c++26 |
| 9 | accept-invalid: no error (expected: arithmetic on pointer to an empty aggregate) | c-c++-common/pr58346-1.c -std=gnu++20, c-c++-common/pr58346-1.c -std=gnu++26, c-c++-common/pr58346-1.c -std=gnu++98 |
| 9 | accept-invalid: no error (expected: narrowing) | g++.dg/diagnostic/sys-narrow1.C -std=c++11, g++.dg/diagnostic/sys-narrow1.C -std=c++20, g++.dg/diagnostic/sys-narrow1.C -std=c++26 |
| 9 | accept-invalid: no error (expected: redeclared) | g++.dg/eh/builtin6.C -std=c++20, g++.dg/eh/builtin6.C -std=c++26, g++.dg/eh/builtin6.C -std=c++98 |
| 8 | accept-invalid: no error (expected: does not declare anything) | g++.dg/ext/anon-struct2.C -std=gnu++20, g++.dg/ext/anon-struct2.C -std=gnu++26, g++.dg/ext/anon-struct2.C -std=gnu++98 |
| 6 | accept-invalid: no error (expected: anonymous struct with base) | g++.dg/ext/anon-struct8.C -std=gnu++20, g++.dg/ext/anon-struct8.C -std=gnu++26, g++.dg/ext/anon-struct8.C -std=gnu++98 |
| 6 | accept-invalid: no error (expected: cannot bind packed field) | g++.dg/ext/packed3.C -std=c++20, g++.dg/ext/packed3.C -std=c++26, g++.dg/ext/packed3.C -std=c++98 |
| 6 | accept-invalid: no error (expected: cannot tail-call:) | c-c++-common/musttail2.c -std=c++11, c-c++-common/musttail2.c -std=c++20, c-c++-common/musttail2.c -std=c++26 |
| 6 | accept-invalid: no error (expected: conflicting declaration) | g++.dg/init/dso_handle2.C -std=gnu++20, g++.dg/init/dso_handle2.C -std=gnu++26, g++.dg/init/dso_handle2.C -std=gnu++98 |
| 6 | accept-invalid: no error (expected: depth) | g++.dg/template/pr39425.C -std=c++20, g++.dg/template/pr39425.C -std=c++26, g++.dg/template/pr39425.C -std=c++98 |
| 6 | accept-invalid: no error (expected: fail) | g++.dg/template/fn-ptr3a.C -std=c++20, g++.dg/template/fn-ptr3a.C -std=c++26, g++.dg/template/fn-ptr3a.C -std=c++98 |
| 6 | accept-invalid: no error (expected: incomplete) | g++.dg/expr/discarded1a.C -std=c++20, g++.dg/expr/discarded1a.C -std=c++26, g++.dg/expr/discarded1a.C -std=c++98 |
| 6 | accept-invalid: no error (expected: invalid arguments for '…') | c-c++-common/patchable_function_entry-error-1.c -std=gnu++20, c-c++-common/patchable_function_entry-error-1.c -std=gnu++26, c-c++-common/patchable_function_entry-error-1.c -std=gnu++98 |
| 6 | accept-invalid: no error (expected: match) | g++.dg/template/ttp16.C -std=gnu++20, g++.dg/template/ttp16.C -std=gnu++26, g++.dg/template/ttp16.C -std=gnu++98 |
| 6 | accept-invalid: no error (expected: no match) | g++.dg/cpp0x/inh-ctor11.C -std=gnu++11, g++.dg/cpp0x/inh-ctor11.C -std=gnu++20, g++.dg/cpp0x/inh-ctor11.C -std=gnu++26 |
| 5 | accept-invalid: no error (expected: conflicts) | g++.dg/cpp0x/using-enum-3.C -std=c++20, g++.dg/cpp0x/using-enum-3.C -std=c++26, g++.dg/lookup/using30.C -std=c++20 |
| 4 | accept-invalid: no error (expected: '…' attribute not specified for parameter '…' on the first declarati…) | g++.dg/cpp26/attr-indeterminate3.C -std=c++11, g++.dg/cpp26/attr-indeterminate3.C -std=c++20, g++.dg/cpp26/attr-indeterminate5.C -std=c++11 |
| 4 | accept-invalid: no error (expected: deleted) | g++.dg/cpp2a/no_unique_address7.C -std=c++20, g++.dg/cpp2a/no_unique_address7.C -std=c++26, g++.dg/cpp2a/no_unique_address7a.C -std=c++20 |
| 4 | accept-invalid: no error (expected: designator used multiple times in the same initializer list) | g++.dg/cpp0x/desig5.C -std=gnu++11, g++.dg/cpp0x/desig5.C -std=gnu++20, g++.dg/cpp0x/desig5.C -std=gnu++26 |
| 4 | accept-invalid: no error (expected: either all initializer clauses should be designated or none of them should be) | g++.dg/cpp/embed-14.C -std=gnu++20, g++.dg/cpp/embed-14.C -std=gnu++26, g++.dg/cpp2a/desig2.C -std=gnu++20 |
| 3 | accept-invalid: no error (expected: '…' attribute may not be specified for '…') | g++.dg/strict-flex-array-1.C -std=gnu++20, g++.dg/strict-flex-array-1.C -std=gnu++26, g++.dg/strict-flex-array-1.C -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' expanded inside directive with '…') | c-c++-common/cpp/counter-2.c -std=gnu++20, c-c++-common/cpp/counter-2.c -std=gnu++26, c-c++-common/cpp/counter-2.c -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' is incompatible with '…'\n) | c-c++-common/cpp/dir-only-4.c -std=gnu++20, c-c++-common/cpp/dir-only-4.c -std=gnu++26, c-c++-common/cpp/dir-only-4.c -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' is not in NFC .-Werror=normalized=.) | c-c++-common/pr68833-3.c -std=gnu++20, c-c++-common/pr68833-3.c -std=gnu++26, c-c++-common/pr68833-3.c -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' is unavailable) | g++.dg/warn/deprecated-19.C -std=gnu++20, g++.dg/warn/deprecated-19.C -std=gnu++26, g++.dg/warn/deprecated-19.C -std=gnu++98 |

### Clusters: wrong-line, clang matches (186 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 63 | wrong-line: missed error at LN (expected: ) | g++.dg/conversion/ptrmem3.C -std=c++20, g++.dg/conversion/ptrmem3.C -std=c++26, g++.dg/conversion/ptrmem3.C -std=c++98 |
| 27 | wrong-line: extra error at LN: error: expected '…' | g++.dg/cpp0x/decltype63.C -std=c++11, g++.dg/cpp0x/decltype63.C -std=c++20, g++.dg/cpp0x/decltype63.C -std=c++26 |
| 16 | wrong-line: extra error at LN: error: expected unqualified-id | g++.dg/cpp0x/decltype13.C -std=c++20, g++.dg/cpp0x/decltype13.C -std=c++26, g++.dg/cpp0x/decltype13.C -std=c++98 |
| 14 | wrong-line: missed error at LN (expected: '…' outside class declaration) | g++.dg/cpp0x/range-for42.C -std=c++11, g++.dg/cpp0x/range-for42.C -std=c++14, g++.dg/cpp0x/range-for42.C -std=c++17 |
| 12 | wrong-line: extra error at LN: error: expected expression | g++.dg/cpp0x/range-for23.C -std=c++11, g++.dg/cpp0x/range-for23.C -std=c++20, g++.dg/cpp0x/range-for23.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: N:expected) | g++.dg/cpp1y/pr65202.C -std=c++14, g++.dg/cpp1y/pr65202.C -std=c++20, g++.dg/cpp1y/pr65202.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: ambiguous) | g++.dg/cpp0x/variadic-alias6.C -std=c++11, g++.dg/cpp0x/variadic-alias6.C -std=c++20, g++.dg/cpp0x/variadic-alias6.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: no match) | g++.dg/lookup/operator-4.C -std=c++17, g++.dg/lookup/operator-4.C -std=c++20, g++.dg/lookup/operator-4.C -std=c++26 |
| 9 | wrong-line: extra error at LN: error: value of type '…' is not implicitly convertible to '…' | g++.dg/template/void3.C -std=c++20, g++.dg/template/void3.C -std=c++26, g++.dg/template/void3.C -std=c++98 |
| 7 | wrong-line: extra error at LN: error: use of undeclared identifier '…' | g++.dg/cpp2a/range-for7.C -, g++.dg/parse/error61.C -std=c++11, g++.dg/parse/error61.C -std=c++20 |
| 6 | wrong-line: extra error at LN: error: default member initializer for '…' needed within definition of enclosing class '…' outside of member functions | g++.dg/ext/flexary23.C -std=gnu++14, g++.dg/ext/flexary23.C -std=gnu++20, g++.dg/ext/flexary23.C -std=gnu++26 |
| 6 | wrong-line: extra error at LN: error: kiln: not yet implemented: pack expansion in this context | g++.dg/cpp1z/using-variadic1.C -std=c++17, g++.dg/cpp1z/using-variadic1.C -std=c++20, g++.dg/cpp1z/using-variadic1.C -std=c++26 |
| 6 | wrong-line: extra error at LN: error: no member named '…' in '…' | g++.dg/cpp0x/pr58080.C -std=c++11, g++.dg/cpp0x/pr58080.C -std=c++20, g++.dg/cpp0x/pr58080.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: N:) | g++.dg/cpp2a/lambda-pack-init7.C -std=c++20, g++.dg/cpp2a/lambda-pack-init7.C -std=c++23, g++.dg/cpp2a/lambda-pack-init7.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: N:expected '…' at end of input) | g++.dg/diagnostic/unclosed-function.C -std=c++20, g++.dg/diagnostic/unclosed-function.C -std=c++26, g++.dg/diagnostic/unclosed-function.C -std=c++98 |
| 6 | wrong-line: missed error at LN (expected: called in a constant expression before its definition is complete\|is not an inte…) | g++.dg/cpp1y/constexpr-recursion1.C -std=gnu++14, g++.dg/cpp1y/constexpr-recursion1.C -std=gnu++20, g++.dg/cpp1y/constexpr-recursion1.C -std=gnu++26 |
| 6 | wrong-line: missed error at LN (expected: depends on itself) | g++.dg/cpp2a/concepts-pr88395.C -std=c++20, g++.dg/cpp2a/concepts-pr88395.C -std=c++23, g++.dg/cpp2a/concepts-pr88395.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: depth) | g++.dg/cpp0x/decltype26.C -std=c++11, g++.dg/cpp0x/decltype26.C -std=c++20, g++.dg/cpp0x/decltype26.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: different kind of entity) | g++.dg/cpp1y/var-templ66.C -std=c++14, g++.dg/cpp1y/var-templ66.C -std=c++20, g++.dg/cpp1y/var-templ66.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: expected unqualified-id) | g++.dg/parse/constructor4.C -std=c++20, g++.dg/parse/constructor4.C -std=c++26, g++.dg/parse/constructor4.C -std=c++98 |
| 6 | wrong-line: missed error at LN (expected: private) | g++.dg/parse/access14.C -std=c++20, g++.dg/parse/access14.C -std=c++26, g++.dg/parse/access14.C -std=c++98 |
| 6 | wrong-line: missed error at LN (expected: redefinition) | g++.dg/cpp2a/concepts.C -std=c++20, g++.dg/cpp2a/concepts.C -std=c++23, g++.dg/cpp2a/concepts.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: reference to label '…' defined inside of '…' attribute expression from outs…) | c-c++-common/pr119537-1.c -std=gnu++20, c-c++-common/pr119537-1.c -std=gnu++26, c-c++-common/pr119537-1.c -std=gnu++98 |
| 6 | wrong-line: missed error at LN (expected: within this context) | g++.dg/tc1/dr166.C -std=c++20, g++.dg/tc1/dr166.C -std=c++26, g++.dg/tc1/dr166.C -std=c++98 |
| 5 | wrong-line: extra error at LN: error: ISO C++ requires the body of an expansion statement to be a compound statement [-Werror,-Wexpansion-stmt-missing-braces] | g++.dg/cpp26/expansion-stmt8.C -std=c++11, g++.dg/cpp26/expansion-stmt8.C -std=c++14, g++.dg/cpp26/expansion-stmt8.C -std=c++17 |
| 5 | wrong-line: missed error at LN (expected: expected) | g++.dg/cpp23/init-stmt2.C -std=c++23, g++.dg/cpp23/init-stmt2.C -std=c++26, g++.dg/parse/attr4.C -std=c++20 |
| 5 | wrong-line: missed error at LN (expected: use of deleted function) | g++.dg/cpp1y/defaulted2.C -std=c++14, g++.dg/cpp1y/defaulted2.C -std=c++17, g++.dg/cpp2a/lambda1.C -std=c++11 |
| 4 | wrong-line: extra error at LN: error: no matching function for call to '…' | g++.dg/cpp26/pack-indexing11.C -std=c++26, g++.dg/template/dependent-expr5.C -std=c++20, g++.dg/template/dependent-expr5.C -std=c++26 |
| 4 | wrong-line: missed error at LN (expected: this context) | g++.dg/parse/access2.C -std=c++20, g++.dg/parse/access2.C -std=c++26, g++.dg/parse/access2.C -std=c++98 |
| 3 | wrong-line: extra error at LN: error: called object type '…' is not a function or function pointer | g++.dg/parse/defarg17.C -std=c++20, g++.dg/parse/defarg17.C -std=c++26, g++.dg/parse/defarg17.C -std=c++98 |

### Clusters: wrong-line, clang differs too (556 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 128 | wrong-line: missed error at LN (expected: ) | c-c++-common/int128-types-1.c -std=gnu++20, c-c++-common/int128-types-1.c -std=gnu++26, c-c++-common/int128-types-1.c -std=gnu++98 |
| 45 | wrong-line: missed error at LN (expected: modifying a const object) | g++.dg/cpp1y/constexpr-tracking-const1.C -std=c++14, g++.dg/cpp1y/constexpr-tracking-const1.C -std=c++20, g++.dg/cpp1y/constexpr-tracking-const1.C -std=c++26 |
| 42 | wrong-line: extra error at LN: error: unknown type name '…' | g++.dg/concepts/auto7c.C -std=c++17, g++.dg/concepts/class6.C -std=gnu++17, g++.dg/concepts/expression2.C -std=gnu++17 |
| 36 | wrong-line: extra error at LN: error: use of undeclared identifier '…' | g++.dg/conversion/const3.C -std=c++20, g++.dg/conversion/const3.C -std=c++26, g++.dg/conversion/const3.C -std=c++98 |
| 33 | wrong-line: extra error at LN: error: expected '…' | c-c++-common/toplevel-asm-5.c -std=gnu++20, c-c++-common/toplevel-asm-5.c -std=gnu++26, c-c++-common/toplevel-asm-5.c -std=gnu++98 |
| 33 | wrong-line: extra error at LN: error: no matching function for call to '…' | g++.dg/concepts/auto7.C -std=c++14, g++.dg/concepts/auto7.C -std=c++20, g++.dg/concepts/auto7.C -std=c++26 |
| 28 | wrong-line: missed error at LN (expected: expected) | c-c++-common/asmgoto-2.c -std=gnu++20, c-c++-common/asmgoto-2.c -std=gnu++26, c-c++-common/asmgoto-2.c -std=gnu++98 |
| 25 | wrong-line: missed error at LN (expected: no match) | g++.dg/concepts/fn5.C -std=gnu++17, g++.dg/concepts/placeholder5.C -std=gnu++20, g++.dg/concepts/placeholder5.C -std=gnu++26 |
| 24 | wrong-line: missed error at LN (expected: jump to label) | g++.dg/cpp1z/constexpr-if4.C -std=gnu++11, g++.dg/cpp1z/constexpr-if4.C -std=gnu++20, g++.dg/cpp1z/constexpr-if4.C -std=gnu++26 |
| 23 | wrong-line: missed error at LN (expected: call to non-.constexpr. function) | g++.dg/cpp1y/constexpr-diag1.C -std=gnu++14, g++.dg/cpp1y/constexpr-diag1.C -std=gnu++20, g++.dg/cpp1y/constexpr-diag1.C -std=gnu++26 |
| 23 | wrong-line: missed error at LN (expected: not declared) | g++.dg/contracts/cpp26/contracts-nested-class1.C -std=c++23, g++.dg/contracts/cpp26/contracts-nested-class1.C -std=c++26, g++.dg/cpp0x/noexcept71.C -std=c++11 |
| 19 | wrong-line: extra error at LN: error: constexpr variable '…' must be initialized by a constant expression | g++.dg/cpp0x/constexpr-nullptr-2.C -std=c++11, g++.dg/cpp0x/constexpr-nullptr-2.C -std=c++20, g++.dg/cpp0x/constexpr-nullptr-2.C -std=c++26 |
| 18 | wrong-line: missed error at LN (expected: uninitialized) | g++.dg/cpp26/constexpr-new6.C -std=c++26, g++.dg/init/const10.C -std=c++11, g++.dg/init/const10.C -std=c++20 |
| 16 | wrong-line: missed error at LN (expected: reference to '…' is ambiguous) | g++.dg/cpp26/name-independent-decl10.C -std=gnu++11, g++.dg/cpp26/name-independent-decl10.C -std=gnu++14, g++.dg/cpp26/name-independent-decl10.C -std=gnu++23 |
| 15 | wrong-line: extra error at LN: error: no member named '…' in '…' | g++.dg/coroutines/coro1-missing-await-method.C -std=c++20, g++.dg/coroutines/coro1-missing-await-method.C -std=c++23, g++.dg/coroutines/coro1-missing-await-method.C -std=c++26 |
| 15 | wrong-line: missed error at LN (expected: ambiguous) | g++.dg/concepts/equiv.C -std=gnu++17, g++.dg/concepts/equiv.C -std=gnu++20, g++.dg/concepts/equiv.C -std=gnu++26 |
| 15 | wrong-line: missed error at LN (expected: instantiating erroneous template) | g++.dg/parse/defarg18a.C -std=c++11, g++.dg/parse/defarg18a.C -std=c++20, g++.dg/parse/defarg18a.C -std=c++26 |
| 15 | wrong-line: missed error at LN (expected: is not valid in an identifier) | c-c++-common/cpp/Winvalid-utf8-3.c -std=gnu++20, c-c++-common/cpp/Winvalid-utf8-3.c -std=gnu++26, c-c++-common/cpp/Winvalid-utf8-3.c -std=gnu++98 |
| 13 | wrong-line: missed error at LN (expected: deleted) | g++.dg/cpp0x/defaulted40.C -std=c++11, g++.dg/cpp0x/defaulted40.C -std=c++20, g++.dg/cpp0x/defaulted40.C -std=c++26 |
| 12 | wrong-line: extra error at LN: error: array is too large (N'…'N'…'N'…'N elements) | c-c++-common/array-5.c -std=gnu++20, c-c++-common/array-5.c -std=gnu++26, c-c++-common/array-5.c -std=gnu++98 |
| 12 | wrong-line: missed error at LN (expected: '…' has incomplete type) | g++.dg/cpp1y/pr79435.C -std=c++14, g++.dg/cpp1y/pr79435.C -std=c++20, g++.dg/cpp1y/pr79435.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: '…' to '…') | g++.dg/cpp1y/pr94066-2.C -std=c++14, g++.dg/cpp1y/pr94066-2.C -std=c++20, g++.dg/cpp1y/pr94066-2.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: accessing uninitialized member) | g++.dg/cpp0x/constexpr-48089.C -std=c++11, g++.dg/cpp0x/constexpr-48089.C -std=c++20, g++.dg/cpp0x/constexpr-48089.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: template parameter) | g++.dg/cpp0x/vt-34314.C -std=c++11, g++.dg/cpp0x/vt-34314.C -std=c++20, g++.dg/cpp0x/vt-34314.C -std=c++26 |
| 11 | wrong-line: missed error at LN (expected: is not a constant expression) | g++.dg/cpp0x/constexpr-50060.C -std=c++11, g++.dg/cpp1y/constexpr-89074-1.C -std=c++14, g++.dg/cpp1y/constexpr-89074-1.C -std=c++20 |
| 11 | wrong-line: missed error at LN (expected: use of deleted function) | g++.dg/cpp0x/lambda/lambda-ice2.C -std=c++11, g++.dg/cpp0x/lambda/lambda-ice2.C -std=c++17, g++.dg/cpp0x/pr62101.C -std=c++11 |
| 9 | wrong-line: extra error at LN: error: '…' is a CN extension [-Werror,-WcN-extensions] | g++.dg/ext/builtin-structured-binding-size3.C -std=c++11, g++.dg/ext/builtin-structured-binding-size3.C -std=c++20, g++.dg/ext/builtin-structured-binding-size3.C -std=c++26 |
| 9 | wrong-line: extra error at LN: error: extraneous template parameter list in template specialization or out-of-line template definition | g++.dg/cpp0x/alias-decl-42.C -std=c++11, g++.dg/cpp0x/alias-decl-42.C -std=c++20, g++.dg/cpp0x/alias-decl-42.C -std=c++26 |
| 9 | wrong-line: missed error at LN (expected: N:literal operator template .int operator\"\"_udl\\(\\). has invalid parameter l…) | g++.dg/cpp2a/udlit-class-nttp-ctad-neg2.C -std=c++20, g++.dg/cpp2a/udlit-class-nttp-ctad-neg2.C -std=c++23, g++.dg/cpp2a/udlit-class-nttp-ctad-neg2.C -std=c++26 |
| 9 | wrong-line: missed error at LN (expected: cannot convert) | g++.dg/conversion/err-recover1.C -std=c++20, g++.dg/conversion/err-recover1.C -std=c++26, g++.dg/conversion/err-recover1.C -std=c++98 |

### Not applicable

Skipped cases (rows; `*` rows cover every case of the test):

| rows | reason |
|---:|---|
| 546 | dg-do target does not match |
| 76 | requires effective target strub |
| 19 | dg-require-iconv |
| 12 | requires effective target fopenmp |
| 11 | dg-require-profiling |
| 8 | dg-do: unknown effective target using_sjlj_exceptions |
| 8 | dg-skip-if: part |
| 7 | dg-require-dll |
| 7 | requires effective target ia32 |
| 6 | dg-add-options float32 |
| 5 | requires effective target c++14_down |
| 3 | dg-require-stack-check |
| 3 | requires effective target dfp |
| 3 | requires effective target fopenacc |
| 3 | requires effective target freorder |
| 3 | requires effective target ilp32 |
| 3 | requires effective target store_merge |
| 2 | dg-skip-if: char8_t |
| 2 | requires effective target arm_neon_ok |
| 2 | requires effective target naked_functions |
| 2 | requires effective target persistent |
| 2 | requires effective target powerpc_altivec |
| 2 | unknown effective target arm32 |
| 1 | dg-additional-options: unknown effective target arm_hard_vfp_ok |
| 1 | dg-additional-options: unknown effective target arm_thumb2_ok |
| 1 | dg-additional-options: unknown effective target bmi2 |
| 1 | dg-additional-options: unknown effective target pow10 |
| 1 | dg-additional-options: unknown effective target powerpc_elfv2 |
| 1 | dg-additional-options: unknown effective target s390_vxe |
| 1 | dg-additional-options: unknown effective target sse4_runtime |
| 1 | dg-additional-options: unknown effective target supports_stack_clash_protection |
| 1 | dg-do: unknown effective target __float128 |
| 1 | dg-do: unknown effective target cas_int |
| 1 | dg-do: unknown effective target correct_iso_cpp_string_wchar_protos |
| 1 | dg-do: unknown effective target float128 |
| 1 | dg-do: unknown effective target float128_runtime |
| 1 | dg-do: unknown effective target float16_runtime |
| 1 | dg-do: unknown effective target float32_runtime |
| 1 | dg-do: unknown effective target float64_runtime |
| 1 | dg-do: unknown effective target mips64 |
| 34 | (34 other reasons) |

GCC-internal checks of the tests run, never compared (tests; the case's class comes from its compile and run):

| tests | check |
|---:|---|
| 366 | `scan-assembler` |
| 252 | `scan-tree-dump-times` |
| 191 | `scan-tree-dump-not` |
| 159 | `scan-assembler-not` |
| 155 | `scan-tree-dump` |
| 58 | `scan-ipa-dump` |
| 32 | `scan-hidden` |
| 25 | `cleanup-ada-spec` |
| 23 | `scan-not-hidden` |
| 18 | `dg-output` |
| 18 | `scan-ipa-dump-times` |
| 13 | `scan-file` |
| 13 | `scan-sarif-file` |
| 13 | `verify-sarif-file` |
| 12 | `scan-assembler-times` |
| 11 | `scan-ipa-dump-not` |
| 10 | `scan-ada-spec-not` |
| 5 | `scan-ada-spec` |
| 5 | `scan-file-not` |
| 5 | `scan-rtl-dump` |
| 2 | `scan-assembler-symbol-section` |
| 2 | `scan-rtl-dump-times` |
| 2 | `scan-sarif-file-not` |
| 1 | `cleanup-coverage-files` |
| 1 | `cleanup-final-insns-dump` |

Directories their own `.exp` drives (not run; files):

`g++.dg/analyzer` 87, `g++.dg/asan` 63, `g++.dg/bprob` 2, `g++.dg/charset` 12, `g++.dg/compat` 127, `g++.dg/coroutines/torture` 137, `g++.dg/debug` 245, `g++.dg/dfp` 15, `g++.dg/gcov` 38, `g++.dg/goacc` 22, `g++.dg/goacc-gomp` 0, `g++.dg/gomp` 464, `g++.dg/graphite` 8, `g++.dg/guality` 9, `g++.dg/hwasan` 1, `g++.dg/lto` 314, `g++.dg/modules` 1401, `g++.dg/pch` 27, `g++.dg/plugin` 27, `g++.dg/sarif-output` 3, `g++.dg/simulate-thread` 4, `g++.dg/special` 9, `g++.dg/tls` 71, `g++.dg/tm` 81, `g++.dg/tree-prof` 18, `g++.dg/tsan` 14, `g++.dg/ubsan` 127, `g++.dg/vect` 131, `g++.dg/torture/stackalign` 20

### Flags a compiler rejected (dropped from its cases)

- kiln: `--c++17`, `--embed-directory <path>`, `--embed-directory=<path>`, `--param case-values-threshold=1`, `--param case-values-threshold=4`, `--param cxx-max-namespaces-for-diagnostic-help=5`, `--param early-inlining-insns=0`, `--param early-inlining-insns=1`, `--param early-inlining-insns=100`, `--param early-inlining-insns=14`, `--param early-inlining-insns=3`, `--param ggc-min-expand=0`, `--param ggc-min-heapsize=0`, `--param ggc-min-heapsize=1024`, `--param hardcfr-max-blocks=9`, `--param hardcfr-max-inline-blocks=5`, `--param hash-table-verification-limit=10000`, `--param ipa-cp-eval-threshold=1`, `--param ipa-cp-eval-threshold=100`, `--param ipa-cp-eval-threshold=125`, `--param large-function-growth=60`, `--param large-function-insns=10`, `--param logical-op-non-short-circuit=0`, `--param logical-op-non-short-circuit=1`, `--param max-cse-insns=0`, `--param max-devirt-targets=1`, `--param max-early-inliner-iterations=1`, `--param max-early-inliner-iterations=10`, `--param max-early-inliner-iterations=3`, `--param max-early-inliner-iterations=5`, `--param max-inline-insns-single=200`, `--param max-inline-insns-single=4`, `--param max-inline-insns-size=1`, `--param max-unroll-times=32`, `--param max-unswitch-insns=5`, `--param uninlined-thunk-insns=1000`, `--param unroll-jam-min-percent=0`, `--param vect-epilogues-nomask=1`, `--param=early-inlining-insns=111`, `--param=early-inlining-insns=176`, `--param=early-inlining-insns=192`, `--param=early-inlining-insns=196`, `--param=early-inlining-insns=82`, `--param=hash-table-verification-limit=1000`, `--param=hash-table-verification-limit=10000`, `--param=hash-table-verification-limit=10000000`, `--param=ipa-cp-eval-threshold=0`, `--param=ipa-cp-eval-threshold=1`, `--param=ipa-cp-max-recursive-depth=94`, `--param=large-stack-frame-growth=1`, `--param=logical-op-non-short-circuit=0`, `--param=max-cse-insns=0`, `--param=max-inline-insns-single=25`, `--param=max-stores-to-track=2`, `--param=modref-max-depth=1`, `--param=partial-inlining-entry-probability=100`, `--param=sccvn-max-alias-queries-per-access=0`, `-C`, `-H`, `-MQ` and 386 more
- gcc: `--c++17`, `-Wno-attributes=___::`, `-Wno-attributes=____::____`, `-Wno-attributes=c2::@tr`, `-Wno-attributes=c::____`, `-Wno-attributes=c@::attr`, `-fcf-protection`, `-fcf-protection=branch`, `-fcf-protection=full`, `-fcf-protection=return`, `-flifetime-dse=123456`, `-fpatchable-function-entry=1,65536`, `-fpatchable-function-entry=65536,1`, `<path>`
- clang: `--c++17`, `--embed-directory <path>`, `-fabi-version=0`, `-fabi-version=14`, `-fabi-version=15`, `-fabi-version=16`, `-fabi-version=18`, `-fabi-version=2`, `-fabi-version=20`, `-fabi-version=3`, `-fabi-version=4`, `-fcf-protection`, `-fcf-protection=branch`, `-fcf-protection=full`, `-fcf-protection=return`, `-fchecking`, `-fchecking=2`, `-fcompare-debug`, `-fconcepts`, `-fconcepts-diagnostics-depth=2`, `-fconcepts-diagnostics-depth=3`, `-fconserve-stack`, `-fconstexpr-fp-except`, `-fconstexpr-loop-limit=5`, `-fconstexpr-loop-limit=98304`, `-fconstexpr-ops-limit=131072`, `-fcontract-evaluation-semantic=enforce`, `-fcontract-evaluation-semantic=observe`, `-fcontracts`, `-fdiagnostics-format=sarif-file`, `-fdiagnostics-set-output=text:show-nesting=yes,show-nesting-locations=no`, `-fdiagnostics-show-caret`, `-fdump-ada-spec`, `-fdump-ipa-icf`, `-fdump-ipa-inline-all`, `-fdump-ipa-sra`, `-fdump-ipa-whole-program`, `-fdump-tree-dse1-details`, `-fdump-tree-eh`, `-fdump-tree-hardcfr`, `-fdump-tree-optimized`, `-fdump-tree-profile_estimate`, `-fext-numeric-literals`, `-ffreestanding`, `-fhardcfr-check-noreturn-calls=always`, `-fhardcfr-check-noreturn-calls=never`, `-fhardcfr-check-noreturn-calls=no-xthrow`, `-fhardcfr-check-noreturn-calls=nothrow`, `-fharden-control-flow-redundancy`, `-fhardened`, `-fimplicit-constexpr`, `-fipa-icf`, `-fira-algorithm=priority`, `-flifetime-dse`, `-flifetime-dse=1`, `-flifetime-dse=123456`, `-fmax-include-depth=1`, `-fnew-inheriting-ctors`, `-fnew-ttp-matching`, `-fno-checking` and 18 more

<!-- gxx-dg:end -->
