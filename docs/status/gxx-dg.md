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

## Triage (2026-11-18, kiln `01c4bb2f5`, aarch64 and x86-64)

Hand-written from the third full run (WP-1691), whose generated region
follows, compared case by case with the second (2026-11-16, kiln
`9eb066486`, WP-1473) by `scripts/gxx-dg-compare.py`. A "case" is one
test at one standard (or one optimization level). Same 20571 tests and
56766 applicable cases on aarch64 in all three runs (57070 on x86-64).

Fixed since this run: WP-1692 (stores through a glvalue conditional of bit-fields, `torture/pr37146-1`), WP-1693 (the explicit instantiation scope check at C++98, `template/explicit8`) and WP-1689 (`new` and `delete` in a constant evaluation before C++20).

| run | kiln | aarch64: pass of decided | x86-64: pass of decided | expect-unknown (aarch64 / x86-64) |
|---|---|---:|---:|---:|
| first (2026-11-15, WP-1443) | `604edce44` | 47950 of 56766 (84.5%) | not run | 0 / - |
| second (2026-11-16, WP-1473) | `9eb066486` | 49496 of 56766 (87.2%) | 49708 of 57070 (87.1%) | 0 / 0 |
| third (2026-11-18, WP-1691) | `01c4bb2f5` | 50256 of 56568 (88.8%) | 50475 of 56870 (88.8%) | 198 / 200 |

**Reading the two headlines side by side.** Since WP-1554 (`c656ea266`)
the runner has an `expect-unknown` class (spec/harness.md, "The g++.dg
runner"): kiln reported an error the test does not require, and the test
file cannot say whether GCC's output holds one too (an error that only a
`dg-message ""`, `dg-excess-errors`, `dg-prune-output` or a `dg-message`
whose pattern leaves the kind open could account for, in a test that does
not link). Such a case is neither a pass nor a failure: the headline's
percentage is over the decided cases only, so the third run's denominator
is smaller by 198 (aarch64) and 200 (x86-64). The pass counts compare
directly (+760 on aarch64, +767 on x86-64); on the old denominator the
third run is 50256 of 56766 (88.5%) on aarch64 and 50475 of 57070
(88.4%) on x86-64. In the second run 197 of the 198 aarch64 cases were
`compile-reject` and one passed (`cpp2a/lambda-uneval10` at C++98, where
kiln now gives clang's C++11-extension error); GCC 16.1 rejects 137 of
them as well, and clang 22 194.

| class (cases) | first | second | third | x86-64 second | x86-64 third |
|---|---:|---:|---:|---:|---:|
| pass | 47950 | 49496 | 50256 | 49708 | 50475 |
| compile-reject | 3775 | 2875 | 2290 | 2943 | 2356 |
| accept-invalid | 1472 | 1196 | 995 | 1205 | 998 |
| wrong-line | 3277 | 3049 | 2905 | 3052 | 2908 |
| wrong | 272 | 144 | 121 | 153 | 130 |
| crash | 17 | 3 | 0 | 6 | 0 |
| timeout | 3 | 3 | 1 | 3 | 3 |
| expect-unknown | - | - | 198 | - | 200 |

Class matrix, second run against third (aarch64; rows: second, columns:
third). x86-64 has the same shape: 801 cases to pass, the same 33 from
pass, 27 between failing classes, 200 into `expect-unknown`.

| second \ third | pass | compile-reject | accept-invalid | wrong-line | wrong | timeout | expect-unknown |
|---|---:|---:|---:|---:|---:|---:|---:|
| pass | 49462 | 15 | 13 | 5 | 0 | 0 | 1 |
| compile-reject | 399 | 2273 | 0 | 3 | 3 | 0 | 197 |
| accept-invalid | 207 | 0 | 976 | 13 | 0 | 0 | 0 |
| wrong-line | 159 | 0 | 6 | 2884 | 0 | 0 | 0 |
| wrong | 24 | 2 | 0 | 0 | 118 | 0 | 0 |
| crash | 3 | 0 | 0 | 0 | 0 | 0 | 0 |
| timeout | 2 | 0 | 0 | 0 | 0 | 1 | 0 |

794 cases went from a failing class to pass (x86-64: 801), counted as
one number: each fix row below re-ran only its own subset.

### What is left: the four-way split

Every failing decided case, one category each (first match wins):

- **kiln bug, open row**: the test is named in the scope of an open or
  partial BACKLOG row that is not a wording row.
- **kiln bug, new row**: the cases of the rows this triage opened
  (WP-1692 to WP-1698), by bucket: rejects-valid where clang 22 accepts,
  accepts-invalid where clang 22 rejects, wrong code on a program clang
  22 does not compile, the crash/hang bucket, and wrong-line cases where
  clang 22 reports GCC's lines and kiln misses an expected error or says
  "not yet implemented". A sample of 8 such wrong-line tests
  (`variadic-alias6`, `lookup/operator-4`, `parse/access14`,
  `cpp1z/using-variadic1`, `cpp1y/pr65202`, `template/void3`,
  `range-for23`, `decltype26`) found 5 missed or unimplemented, 1 location
  only and 2 extra follow-on errors: an upper bound.
- **GCC-specific or test expectation** (will not pass with a kiln that
  follows the text and clang 22): rejects-valid and accepts-invalid where
  clang 22 does the same (the second run's verdict tables below: a or b
  in 25 of the 30 largest clusters of each), wrong code where clang 22
  is wrong too, and GCC's "sorry, unimplemented" expectations.
- **wording or location only**: the other wrong-line cases (an extra
  error clang 22 does not give at that line, or clang 22 differing from
  GCC's lines as well; the second run's 20 largest of these were 16
  "stays"), and the cases of the location and wording rows (WP-1540,
  WP-1632).

| category | aarch64 | x86-64 |
|---|---:|---:|
| kiln bug, open row | 25 | 26 |
| kiln bug, new row (WP-1692 to WP-1698; an upper bound) | 1160 | 1165 |
| GCC-specific or test expectation | 2614 | 2688 |
| wording or location only | 2513 | 2516 |
| failing decided cases | 6312 | 6395 |
| expect-unknown (not decided) | 198 | 200 |

The open-row cases: WP-1683 (`#pragma interface`, 12), WP-1661 (C++98
`-pedantic-errors`, 5), WP-1675 (`__builtin_has_attribute`, 3), WP-1662
(2), WP-1646 (2), WP-1660 (1), and on x86-64 WP-1544 (`unwind1`, 1).
The "new row" cases by kind (aarch64): accepts-invalid where clang
rejects 443, wrong-line with a missed or unimplemented error where clang
matches 385, rejects-valid where clang accepts 309, wrong code clang
cannot compile 21, `explicit8` 1, the compile timeout 1.

### Cases that went from pass to a failing class (33)

The same 33 cases (15 tests) on both hosts. None is flaky. One is a
regression:

- `template/explicit8` at C++98 (1): an explicit instantiation in a
  namespace that does not enclose the template is accepted. WP-1473's
  fix for a C++98 instantiation in an *enclosing* namespace (`46d321c9d`)
  skips the whole scope check before C++11, so this ill-formed case (C++03
  [temp.explicit]/5, and the C++11 rule too) lost its error; GCC rejects,
  clang 22 only warns under `-Wc++11-compat`. xfail repro
  `temp.explicit/scope-not-enclosing-cxx98.cpp` (WP-1693);
  the fix applies the C++11 rule in C++98 (`crates/kiln_sema/src/templ/spec.rs`,
  the scope check).

kiln made stricter or more correct than the test expects:

- `g++.old-deja/g++.bugs/900428_01` at C++20 and C++26 (2): a discarded
  `volatile` glvalue of an incomplete class (an indirection, a
  reference or a variable as an expression statement or cast to `void`)
  is an error since WP-1533 (`05b588ed8`): from C++11 [expr.context]/2
  applies the lvalue-to-rvalue conversion to it and [conv.lval]/1 makes a
  program that needs it for an incomplete type ill-formed. GCC 16.1 and
  clang 22 only warn (the test expects GCC's warnings); kiln follows the
  text (spec/sema.md), and C++98 still passes.
- `cpp2a/new-array5` at C++20 and C++26 (2): an array new with no bound and
  an empty `()` initializer supplies no initial element, so no bound can be determined ([expr.new]/7,
  [dcl.init.general]/16.4); kiln reports it as clang 22 does since
  WP-1518 (`968fedd96`); GCC allocates an array of no elements.
- `ext/deprecate-1` (3): a member function in a GNU anonymous struct is an
  error, as in clang 22, since WP-1604 (GCC accepts it with a deprecation
  warning under `-fpermissive`).
- `warn/ignored_typedef` (3): `typedef struct Tag { int m; };` declares no
  typedef-name; clang 22's `-Wmissing-declarations` extension warning, an
  error under the suite's `-pedantic-errors`, since WP-1539 (GCC warns
  that the typedef is ignored).
- `other/crash-9` and `template/nested5` at C++98 (2): implicit
  `typename` is a C++20 extension, an error under `-pedantic-errors`, as
  in clang 22 (WP-1630).
- `template/pr84694` (3): an unqualified friend `b<0>` naming a member
  template, rejected with clang 22's text (WP-1662).
- `reflect/constant_of1` (1) and `reflect/substitute3` (1): the working
  draft decides against GCC 16.1 (WP-1642, WP-1673; docs/status/deviations.md).

The runner, not kiln (all three compilers fail each the same way or the
expectation is GCC's own limitation):

- `c-c++-common/cpp/eof-3` and `cpp/line-2` (6): since WP-1554 a
  `dg-regexp` that names an error is an expected error; the regexp needs
  `$srcdir`-relative include output no compiler here prints (GCC fails
  them too).
- `cpp0x/inh-ctor32` and `ext/typeof12` (6): the expected "error" is
  GCC's `sorry, unimplemented` (passing arguments through an inherited
  ellipsis constructor; mangling `__typeof__` of a parameter); kiln and
  clang compile the valid code. WP-1699: the runner should not read a
  `sorry, unimplemented` `dg-message` as an expected error.
- `cpp1y/udlit-char-template-sfinae-neg` (3): WP-1617 (its L20
  `dg-error` is not read by DejaGnu; GCC is wrong-line too).

Cases that moved between two failing classes: 27 (aarch64). Into
`expect-unknown`: 198 (above).

### Rows that landed between the second and third run

One line per row (each re-ran only its own subset; the BACKLOG rows have
the detail). Before the second run: WP-1452 to WP-1517 (first to second
run: 47950 to 49496 cases on aarch64).

- WP-1473 (after its run): 37 cases its own run found lost to earlier fix rows, by 13 one-rule changes (by-copy capture of a reference to a function, `explicit` and destructor templates in friend declarations, CWG2918 for `&f<X>`, C++98 explicit instantiation in an enclosing namespace, and nine more); its C++98 change lost `template/explicit8` (above).
- WP-1526..WP-1529: P2280 in every mode from C++11 and `this` as a member-access postfix (`constexpr-ref14`, `constexpr-array-ptr6`, `lambda-generic-const10`); placement new into an inactive `T[1]` member (`constexpr-new23`/`28`); `constexpr-eh7`'s `catch (const char **&)` of a `nullptr` stays uncaught ([except.handle]/3.4 needs `const T&`); `__builtin_bit_cast` of classes with bases and bit-fields (`bit-cast3`, `bit-cast7`, `bit-cast16`).
- WP-1547, WP-1542, WP-1545: `label13a`, `fam-in-union-alone-in-struct-2`, `delete5`, `udlit-char-template-vs-std-literal-operator`, `ellipsis3` and `pr53011` from the crash, wrong-code and regression lists; `attr-aligned-1` is left (an Oven typedef-alignment bug).
- WP-1530: `p->N::f<U>()` with a dependent `p` and a non-dependent class `N`, and an overloaded member template named through a qualifier of the current instantiation or a non-dependent base, take a template argument list without `template` (`decltype34`, 3 cases; `member3`-`member5` need member calls through `this` to be non-dependent, left as a remainder); `nested3` (WP-1546) is right as it is: its member declaration is a type-only context where `<` starts a list ([temp.names]/7.3.4), and its L14 error is WP-1470's incomplete-class rule.
- WP-1541, WP-1536: function specifiers on structured bindings and for-range/expansion declarations (`range-for42`/`43`, `decomp3`, `expansion-stmt9`), attributes on asm-declarations (`dr2262`, CWG2262) and keyword attribute-tokens (`gen-attrs-61`): 20 cases; `inline template` (`wrong-inline1`, `error11`, 6 cases) keeps clang's warning, a documented difference.
- WP-1587, WP-1549, WP-1551, WP-1550: `__builtin_{add,sub,mul}_overflow_p` (`builtin-arith-overflow-2`, `builtin-arith-overflow-p-*`), GNU `noreturn` added by a function pointer conversion (`noreturn1`/`2`, `canon-33194`): 56 cases to pass; `__builtin_va_arg_pack` and the C++ `gnu_inline` redefinitions stay rejected, as in clang (C: the cast to a union type, a regression).
- WP-1574, WP-1568, WP-1575, WP-1569, WP-1567: `attr-aligned-1` (`alignof` of a cv-qualified aligned array typedef, also of an array of one); the Oven now runs the destructors of empty virtual bases, so `delete5`'s codegen guard is gone; an assignment through an inactive union member of class type with a non-trivial default constructor is rejected (`constexpr-union8` stays passing: anonymous union members are exempt); a union's flexible array member elements of class type are left to the static initializer. Filtered before/after run (1503 cases): no case from pass to a failing class.
- WP-1532, WP-1533, WP-1534, WP-1535: accepts-invalid fixes for `overload-conv-5`, `inh-ctor30`, `explicit77`, `fn-ptr3a`, `fn-ptr5`, `discarded1a` (from C++11), `using-enum-3`, `using30`; `constexpr-lambda10` is left (an Oven capture read).
- WP-1593, WP-1595, WP-1572, WP-1588, WP-1591: nested anonymous union members with default member initializers (`constexpr-union8`'s exemption for anonymous union members is gone), arrays of over-aligned typedef elements, `this` in a static member function's declarator (`constexpr-this1`, 3 cases: accepts-invalid to wrong-line, now clang 22's diagnostics), structured bindings in a condition with an array and at class scope, a function named as a template argument in a lambda body in a template. Filtered before/after run (1475 cases): no case from pass to a failing class.
- WP-1590, WP-1594, WP-1573: an odr-use of a by-copy capture in a constant expression in the lambda body names the closure member (`constexpr-lambda10`, 3 cases); `delete` deallocates when the destructor throws; an indeterminate bit-field sharing a byte with a determinate one after a bit-cast is not readable. Filtered before/after run (3119 cases): no case from pass to a failing class.
- WP-1570, WP-1578, WP-1598, WP-1599: a member call through `this` naming one non-template member function with a non-dependent type, of a class with no dependent base, is not type-dependent (`member3`, `member4`: 6 cases; `member5`'s call of a member template stays dependent, a remainder); the cleanups of a dependent template argument end with it before C++17; `sat_concept_id_partial` with a non-type parameter; requires-expression deferral under pack expansions. Filtered before/after run (12500 cases of template, lookup, cpp2a, concepts, cpp1z, cpp26 and others): no case from pass to a failing class.
- WP-1548, WP-1552: GCC's `__builtin_shuffle` and mixed-length `__builtin_shufflevector` (`builtin-shuffle-1`, `builtin-shufflevector-1`/`-2`, `builtin-convertvector-2`, every `vshuf-*`), `__builtin_speculation_safe_value` (`spec-barrier-*`, `builtin-speculation-overloads1`-`4`), `__builtin_assoc_barrier`, `__builtin_ffsg`/`clrsbg`/`parityg` (`pr111309-1`, `pr118868-1`), and deprecation/unavailability of conversion functions and instantiated enumerators; `builtin-assoc-barrier-1` still fails on its `optimize` attribute; `__builtin_has_attribute`, `__integer_pack` and `__is_invocable` are left out.
- WP-1554, WP-1556, WP-1600: the runner reads `dg-message` and `dg-regexp` lines that name an error as expected errors and line-0 `dg-error`/`dg-message` as any line; a case whose errors only `dg-message ""`, `dg-prune-output` or `dg-excess-errors` could account for is `expect-unknown`, counted apart (filtered run with one release kiln, 7292 cases: 27 to pass, 135 to expect-unknown, none from pass to a failing class); the citations of the verdict tables were checked against the working draft; every CI job has a timeout.
- WP-1604, WP-1606, WP-1592: an anonymous union or struct defines only public non-static data members (`anon-union5`, 3 cases), `alignof` of an enumeration with an aligned attribute and arrays of it, an odr-use of a by-copy capture in a case value outside a template, and `B b(u)` through `operator B() const` beside a better `operator B&()` (clang's CWG2327 result). Filtered before/after run (3741 cases): no case from pass to a failing class.
- WP-1609, WP-1610, WP-1612: a call through `this` of the current instantiation naming an overload set or a member template resolves at definition when instantiation cannot choose differently (`member5`, 3 cases); a type-constraint's argument list is checked as the concept-id (pack expansion onto a non-pack parameter, arity); `__builtin_parityg` rejects `bool` and enumerations; an invalid specialization adds no `redefinition` error (`builtin-speculation-overloads5`, 3 cases); `var-concept5` (2 cases) passes too. Filtered before/after run (7447 cases of template, lookup, concepts, builtins): no case from pass to a failing class, 8 to pass.
- WP-1539 (first half), WP-1543: pointer-to-member conversions through an inaccessible or ambiguous base (`ptrmem2`-`7`), decl-specifier and declarator rules (`inline` on a parameter or data member before C++17, free-standing tag specifiers, `void` parameters, conversion-function cv, template typedef, typedef-name destructors), friend forms (`crash42`, `crash43`, `crash44`, `friend45`), `C::C` as a type, `typename` naming a non-type template, ambiguous qualified injected-class-names, repeated `using E::x;` in a class (`using-enum-5`), and `'+f'` asm outputs on x86-64. Filtered before/after run (9064 cases): 81 to pass, none from pass to a failing class.
- WP-1621, WP-1622, WP-1624, WP-1625, WP-1616, WP-1615: `<stddef.h>` declares `::nullptr_t` from C++11 (`cpp0x/stddef`, 3 cases); an in-class initializer of a floating or complex floating `static const` member at C++98 is the GNU extension clang accepts (`ext/complex7`); a by-copy capture odr-used in a local class's or enumeration's scope is diagnosed; a template argument's narrowing check folds after its full-expression; `using N::X;` then `struct X` in one scope conflicts; an rvalue reference bound through a conversion function yielding a derived class as clang and GCC bind it.
- WP-1628, WP-1629, WP-1640, WP-1533, WP-1535, WP-1547, WP-1603: siblings and remainders of rows whose GCC cases already passed: `alignof` through a typedef of an aligned array; `(void)ca` of a constexpr class object does not capture it; keyword `alignas(*&i)` of a by-copy capture where clang and GCC accept it; a function template specialization named as a template argument in a generic lambda's body is instantiated; a dependent variable placeholder's type-constraint list is checked at definition; unqualified calls of a static-only member set or a member template of the current instantiation resolve at definition; three clang-over-GCC decisions recorded in `deviations.md`. Filtered before/after run with release binaries (11674 cases of template, lookup, concepts, lambda, constexpr, decomp, alignment, narrowing): no case changed class.
- WP-1630, WP-1631: an overload set without a target (`for1`, `cond7`), `static_cast` casting away constness (`cast5`), closure special members by language mode (`lambda-ctor-neg`, `lambda1`), explicit specialization headers (`spec15`), CTAD class parameters as forwarding references (`class-deduction26`, `35`, `96`), inaccessible move constructors (`implicit-move1`) and template template argument access (`access18`), later-standard rules in earlier modes (`typename3`, `dependent-name5`, `constexpr-lambda29`, `eb129`, `900210_10`, `subscript11`), anonymous union member access (`union2`), injected-class-name ambiguity at instantiation (`injected1`). Filtered before/after runs (15820 + 20785 cases, overlapping): 73 to pass; from pass: `using-enum-3` and `using-enum-5` (a class's using-declarator of its own enumerator is accepted, as clang 22; deviations.md), `nested5` and `crash-9` at C++98 (implicit `typename` is a C++20 extension, an error under `-pedantic-errors`, as clang 22).
- WP-1525, WP-1523, WP-1520: `sizeof` of a variable length array type in an instantiated function template (`template/vla1`, 3 cases to pass); caller-side contract checks of declared-only template specializations; `expansion-stmt16` stays wrong-line by decision (its second error is not required) and `eh/anon1` stays wrong pending the `type_info` name decision. Filtered before/after run (2424 cases of vla, contracts, expansion-stmt, lambda, rtti, inherit, abi): 3 to pass, none from pass to a failing class.
- WP-1522, WP-1646, WP-1647, WP-1648, WP-1634: address template arguments before C++20 (`nontype8`; `nontype25` keeps its dependent form at L10), a trivial assignment is not constexpr in C++11 (`constexpr-trivial2`), constexpr mismatch of a befriended special member (`decl6`), out-of-line member definitions in a module purview of an `extern "C++"` class; CWG2310 (`constexpr-base6`) and `constexpr-ctor14a` stay accepted as clang 22 does (docs/status/deviations.md). Before and after over the friend, nontype, ttp, constexpr, init, modules and old-deja directories: 0 pass to failing, 3 failing to pass.
- WP-1642, WP-1644, WP-1524: `g++.dg/reflect` 476 → 481 of 565 (`annotations17`, `annotations7`, `define_aggregate10`, `expr2`, `odr1`, `splice1` pass); `constant_of1` now rejects the draft example's escaped local (deviations), `pack-index1` accepts `^^Ts...[0]` of a type pack (deviations); `ns4`, `qrn1`, `splicing-base4`, `pr124689` and the rest of `annotations4` differ only in wording, location or follow-on errors.
- WP-1645, WP-1652, WP-1519, WP-1518: C++98 extensions under `-pedantic-errors` as clang 22 reports them (`parse11`, `cvt11`, `init1`, `enum6`, `var1`, `friend38`, enumerator-list commas, extra `;`, `>>`; GCC's `-Wno-long-long` silences `long long`), storage classes on anonymous unions (`anon-union3`), a non-local lambda with a capture-default (`lambda-non-local`, `lambda-uneval21`), `lambda-targ20`, and two link failures (`tem06`, `explicit74`): 37 cases to pass over the brief's directories.
- WP-1655, WP-1656: a constant expression in a lambda body reads a by-copy capture that a call receives (`p.get()`, `get(p)`, a copy) as clang 22 and GCC 16.1 do (deviations.md); a friend redeclaring a member checks its exception specification against the member's, and a C++11 friend's constexpr member function is implicitly `const`, as clang 22. Before and after over the lambda, constexpr-lambda, friend, nontype, eh and g++.pt directories (4960 cases): no case changed class (the fixed forms were siblings of passing cases).
- WP-1611, WP-1633: `__builtin_has_attribute` (`builtin-has-attribute-5`/`-6`/`-7`, `ext/consteval1`; a class member access operand in a template definition is refused as GCC does, so `builtin-has-attribute2` stays pass), GCC's `__remove_reference(T)` spelling (`ext/remove_reference`, `cpp1z/pr122185`, `coroutines/pr113457`, `torture/pr116460`), `__builtin_assoc_barrier` on a class operand. Filtered before/after run (10079 cases): 21 to pass, none from pass to a failing class.
- WP-1663, WP-1653: a C-linkage function named like a mangled C++ function forwards to one LLVM global (`pr77550`), `noalias` on `__restrict` parameters (`c-c++-common/restrict-1.c`, 3 cases); on the abi, inherit, modules, opt, torture, ext and restrict subset 5851 → 5855 of 6719 with no case leaving pass (`g++.dg/modules` stays skipped by the runner, so WP-1653 is covered by kiln run tests)
- WP-1657, WP-1666, WP-1665, WP-1662: pre-C++17 address and member-pointer template argument forms, dependent forms included, plus the C++98 parentheses extension (`ptrmem6`); a lambda call is not constexpr before C++17 (`lambda-conv15`, `auto-fn33`); a non-dependent friend specialization in a template is matched at the definition (`friend89`). Before/after run (20268 decided cases): 8 to pass, 3 from pass to compile-reject, all `template/pr84694` (an unqualified friend `b<0>` naming a member template: rejected with clang 22's text, GCC 16.1 accepts).
- WP-1660: `g++.dg/reflect` stays 481 of 565, no case changes class: `^^decltype(l)::operator()` and the other special names after a computed scope are reflected (`extract11` now fails later, at `substitute` of a call operator template whose return type is undeduced), an annotation before a lambda-declarator appertains to the call operator (`annotations1` keeps only its annotation on a concept, which the text does not allow: deviations.md), and `decltype` of an object splice keeps the template parameter object's `const` (`mangle3` L63).
- WP-1667: GCC's `__integer_pack(N)` (`ext/integer-pack1`, `3`, `4`, `6`-`8`, `cpp0x/variadic-alias7`, `cpp2a/lambda-generic12`, `reflect/variable_of3`), `__is_invocable` and `__is_nothrow_invocable` (`ext/is_invocable1`-`7`, `is_nothrow_invocable`; `__has_builtin` stays 0, so `has-builtin-1` still fails), `__builtin_has_attribute` on type-id attributes and internal visibility (`builtin-has-attribute-2`/`-3`/`-4` fail on fewer lines). Filtered before/after run (11131 cases): 46 to pass, none from pass to a failing class; `integer-pack3` (3 cases) passes after the mangling commit.
- WP-1651, WP-1669: under libstdc++ the `type_info` name of a unit-local type starts with GCC's `*`, so two units' twins compare unequal and a handler or `dynamic_cast` does not match the other's (`abi/local1`, `eh/anon1`: 6 cases to pass); module initializers for instantiated static data members, a constexpr exposure of a TU-local value; on rtti, eh, abi, inherit, torture, ext and old-deja eh/abi/other no case left pass.
- WP-1673, WP-1674: `g++.dg/reflect` stays 481 of 565 (release binaries, base and branch): `extract11` passes (`substitute` instantiates a function template's definition to deduce its placeholder return type, [meta.reflection.substitute]/2's example, also at namespace scope); `substitute3` goes from pass to compile-reject by the same rule, against GCC 16.1 (deviations.md). A constexpr local's recorded value is materialized, so a nested evaluation has the local's point (CWG3162); `^^typename S::T` (non-dependent) reflects the type denoted.
- WP-1664, WP-1617 (runner only): an input file in a test's options (`dg-options "-O3 ${srcdir}/.../pr64049-1.C"`) is compiled and linked with it instead of probed as a flag (`pr64049-2`, 3 cases to pass); a line-0 `dg-error`/`dg-message` in a `dg-do link` test is an expected link error (`g++.pt/const2`, 3 cases to pass); `dg-additional-sources` takes selectors and `linkonly`, `$subdir` is substituted, a missing source skips; a `dg-prune-output` that can only match a warning or note line no longer lifts the extra-error check (`cpp1y/udlit-char-template-sfinae-neg`, 3 cases from pass to wrong-line: its L20 `dg-error` has no blank before `}`, so DejaGnu does not read it and GCC 16.1 is wrong-line too). `interface2`, `interface3a`, `interface3b`, `opt/interface1` stay compile-reject: GCC's `#pragma interface`/`implementation`, which clang does not implement either. Before/after with one kiln binary (19540 rows): no other case changed class.
- WP-1677, WP-1678, WP-1676: an uninitialized variable in a constexpr function or constructor before C++20 is clang's C++20-extension warning (`cpp1z/constexpr-lambda12` now passes); in a module interface a declaration that names a TU-local entity (a signature, a type, a base, a default argument, an inline function's body) is an error, on the subset GCC 16.1 rejects; `ext/integer-pack2` stays a difference by design (clang 22 keeps the narrowing error under `-fpermissive`) and `cpp2a/lambda-generic-variadic21` too (it names the variable template's own specialization in its lambda: ill-formed, clang 22 rejects). Over `modules`, `cpp0x`-`cpp2a/constexpr*`, `cpp1y/var-templ*`, `cpp2a/lambda*` and `ext`: 5040 -> 5041 of 6197 decided cases, no case from pass to a failing class.
- WP-1680, WP-1675: no case changed class (10252 cases, base and branch harness); `c-c++-common/builtin-has-attribute-2.c` and `-4.c` assert fewer failing lines (nonstring, nocommon, nonnull on function typedefs, format with partial arguments).
- WP-1650, WP-1668, WP-1679: construction vtables of deep single-inheritance chains below a virtual base in quadratic time (a 300-deep chain 7.2 s → 0.85 s, IR and vtable dumps byte-identical), C `static` array parameters get `align`/`dereferenceable` as clang's, the `type_info` `*` marker on closures of internal variables, unnamed namespace-scope classes and unnamed-namespace incomplete classes; on abi, inherit, rtti, eh, init, torture, old-deja abi/mike/other and c-c++-common (10099 cases, release binaries base and branch) and the C torture suite no case changed class.
- WP-1685, WP-1684: before C++20, default-initializing a class whose implicit default constructor leaves a member uninitialized is not constant, and a constexpr constructor leaving one uninitialized gets clang's C++20-extension warning (`cpp0x/constexpr-ctor`, `constexpr-diag4`, `constexpr-array6`, `pr79118`, `cpp1y/constexpr-83921-3` now pass); in a module interface a default member initializer, a member function's default argument, an odr-use of a TU-local constant and a closure-typed variable's lambda are checked for exposures (`modules/internal-20_c` gets its expected error at L10; the runner skips `g++.dg/modules`, so all 782 module files there were compiled with both binaries: no other diagnostic changed). Over `modules`, `cpp0x`, `cpp1y`, `cpp1z/constexpr*`, `cpp2a/constexpr*`, `init` and `old-deja/g++.other`: 12757 -> 12767 of 14528 decided cases, no case from pass to a failing class.

### New clusters

Cluster keys of the two runs compared per bucket (`clusters` on both
TSVs): no new cluster of 5 cases or more. The one new key of that size,
"qualified reference to '…' is a constructor name rather than a type"
(6 cases: `cpp0x/deleted3`, `template/ctor9`), is the second run's "expected
unqualified-id" cluster (#19 below) with clang 22's newer wording; its
verdict stays a ([class.qual]/1, [temp.arg.explicit]/2). Below 5 cases,
new keys: rejects-valid where clang rejects too 12 clusters (35 cases),
wrong-line where clang differs too 15 (35), accepts-invalid where clang
accepts too 6 (16), wrong-line where clang matches 3 (6), rejects-valid
where clang accepts 2 (3), accepts-invalid where clang rejects 1 (1).

### Crash / hang (1 case; 3 on x86-64)

**Compile timeout** on `other/pr117516` (about thirty classes, each
with two members of the one before, so the last has 2^27 `char`
subobjects): 50 to 60 s per standard against the 60 s step limit, so
which standard times out depends on the load (C++26 on aarch64, all
three on x86-64). Flaky: re-run alone three times, 3 of 3 cases passed
each time. Perf log (GCC 16.1's testsuite,
`gcc/testsuite/g++.dg/other/pr117516.C`, this aarch64 host at a load of
about 7 of 24 cores, kiln `01c4bb2f5` release):

- `kiln++ -std=c++26 -c -o pr117516.o pr117516.C`: 53.6 s, peak RSS 443 MB;
- `clang++-19 -std=c++2c -c -o pr117516.o pr117516.C`: 6.1 s, peak RSS 94 MB;
- `kiln++ -std=c++26 -fsyntax-only pr117516.C`: 0.01 s;
  `kiln++ -std=c++26 -S -emit-llvm pr117516.C`: 55.0 s for a 54-line
  module (one `llvm.memset` of 134217728 bytes).

About nine times clang's time, all of it in kiln's IR generation (not
sema, not LLVM): probably a walk over every subobject instead of every
class. WP-1695.

### Wrong code (121 cases, 44 tests; 130 on x86-64)

1. **A glvalue conditional of bit-fields** (2 cases, `torture/pr37146-1`):
   `(c ? s.p : s.q) = v` stores a whole `int` into the storage unit, so
   the neighbouring bit-fields are overwritten; compound assignment and
   `++` too, reads are right ([expr.cond]/5: the result is a bit-field).
   clang 22 cannot compile it ("cannot compile this conditional operator
   yet"); GCC 16.1 is right. xfail repro
   `expr.cond/p5-bit-field-glvalue-store-run.cpp`; WP-1692 (codegen,
   `cond_lvalue` in `crates/kiln_codegen/src/expr.rs`).
2. **Wrong at run time on a program clang 22 rejects** (19 cases, 7
   tests): `cpp26/expansion-stmt17` (an expansion statement over a range
   of temporaries with `-frange-for-ext-temps`), `ext/flexary39`,
   `flexary40` (flexible array members), `auto-init-padding-3`
   (`-ftrivial-auto-var-init` and padding), `torture/builtin-clear-padding-2`,
   `c-c++-common/torture/builtin-clear-padding-4`, `builtin-assoc-barrier-1`.
   No oracle but GCC; WP-1694: reduce each and fix or document.
3. **Clang 22 is wrong too** (100 cases): GCC builtins and options
   (`__builtin_object_size` and `__builtin_dynamic_object_size`,
   `cleanup` with exceptions, `-flifetime-dse`, identical-code-folding
   addresses, `__sync`, `__builtin_constant_p`), `__PRETTY_FUNCTION__`
   text (`pretty1`-`3`, `memtemp77`), GCC layouts (`abi/bitfield1`,
   `abi/vbase13`, `packed4`), and unspecified results the test pins to
   GCC's choice (`constexpr-static4`: static initialization of a
   dynamically initialized array; `init/array57`: order inside an
   aggregate initializer; `explicit5`: GCC's extension of CWG899).
   GCC-specific.

### Rejects-valid where clang accepts (309 cases, 90 clusters)

WP-1696: one batch row, largest clusters first.
The 20 largest (this run's counts):

| cases | cluster | pattern |
|---:|---|---|
| 12 | functions that differ only in their return type | a `vector_size` attribute written before the declaration's type (a friend declaration, then the definition): kiln does not apply it to the return type of every declaration, so they differ |
| 7 | GNU `?:` on class types is not supported yet | `bool b = X() ?: false;` (binary conditional with a class operand) |
| 6 | call to deleted function | `sfinae70`/`71` at C++23 and later: an implicitly movable entity as an argument (P2266) |
| 6 | cannot yet mangle `UnresolvedLookupExpr` | `decltype(f(x))` of an unresolved call in a lambda's signature |
| 6 | comparison of distinct pointer types | `ap != bp` for `const int A::*` and `int B::*` with `B` derived from `A` (composite pointer type) |
| 6 | expected `;` after namespace name | `using namespace N __attribute__((strong));` |
| 6 | expected namespace name | `using namespace std;` with no `std` declared (`-w`) |
| 6 | extraneous template parameter list | an out-of-line member template of a class template's friend |
| 6 | invalid vector element type | `[[gnu::vector_size]]` on a dependent pointer alias, applied before substitution |
| 6 | not yet implemented: deduction guides as members | a deduction guide in a class template's member template |
| 6 | not yet implemented: dependent nested name specifier | `friend void* A<T>::operator new(size_t)` |
| 6 | not yet implemented: member templates | `friend struct Outer<V>::Inner` |
| 6 | member found in multiple base classes | `D().v<int>::size()` with bases `v<double>` and `v<int>` |
| 6 | no function template matches | a specialization with attributes the primary lacks (`Wmissing-attributes-1`) |
| 6 | no member named | `template/operator2`: a conversion-function template named through a dependent base |
| 6 | NTTP type contains unexpanded pack | `template<class... T> struct S { template<T... v> ... }` (`variadic152`) |
| 6 | overload resolution selected deleted operator | `lookup/operator-8`: a deleted operator found by the first lookup only |
| 6 | partial ordering for explicit instantiation is ambiguous | `template/partial9` |
| 6 | size of array has non-integer type | `constexpr-ex4`: a literal class converting to an integer as a bound |
| 6 | unexpected type name: expected expression | `__builtin_structured_binding_size(T)` with a type operand |

### Accepts-invalid where clang rejects (446 cases, 183 clusters)

A tail: the largest cluster has 14 cases. WP-1697: one batch
row (the accepts-invalid batches WP-1470 and WP-1471 are done). The
largest:

| cases | expected | pattern |
|---:|---|---|
| 14 | any error (`dg-error ""`) | `constexpr-initlist11a` at C++26, `parse/template7`, `template/varmod1` and 5 more |
| 5 | invalid use of `this` | a generic lambda in `gnu++98`, `lookup/this2` |
| 5 | use of deleted function | `move-return5`: an implicitly moved `return x;` selecting a deleted constructor |
| 4 | not a class | `alias-decl-80` |
| 4 | not a constant expression | `concepts-pr67697` |
| 3 each | (singletons) | `constinit` on a function or its return type (`constinit18`, `19`), `co_await` in a default argument, `template` in a destructor name, `operator delete` returning non-`void`, both `__thread` and `thread_local`, `operator+(T, T)` with no class parameter after explicit arguments, scoped access in `lookup/scoped1`, a poisoned identifier through `_Pragma` |

### Where clang agrees with kiln: verdicts by the standard

Verdicts: **a** = kiln and clang are right by the standard (GCC accepts
an extension, or GCC differs); **b** = the test needs a GCC-only flag,
builtin, attribute, pragma or target feature; **c** = kiln is wrong along
with clang. Paragraph numbers are the C++26 working draft of 2026-11
(cplusplus/draft of 2026-11-16), checked against the paragraph text
(WP-1556). A cluster is the first kiln diagnostic with names and numbers
stripped; the verdicts are the second run's (WP-1473), the counts this
run's. Clusters with no case left are dropped.

#### Rejects-valid, clang rejects too (1981 cases; 2044 on x86-64)

| # | cluster (short pattern) | second | third | verdict | reason |
|---|---|---:|---:|---|---|
| 1 | use of undeclared identifier | 265 | 64 | b | GCC builtins left: `__builtin_apply_args`, `__builtin_va_arg_pack`(`_len`), `__builtin_puts` and smaller ones; `__CHAR8_TYPE__`. a: `-fpermissive` lookup. (`__builtin_shuffle`, the `_overflow_p` builtins, `__integer_pack`, `__builtin_assoc_barrier`, `__builtin_speculation_safe_value`, `__builtin_has_attribute` and `::nullptr_t` were fixed: WP-1548, WP-1587, WP-1611, WP-1615, WP-1667) |
| 2 | static assertion failed (no message) | 68 | 59 | mixed | reflection (WP-1537's table below); a: `source_location` column and function name are implementation-defined ([support.srcloc.cons]); `__is_trivial` with a deleted more-constrained default constructor ([special]/6); b: GCC 16 SFINAE on `__atomic_*` misuse, `-D` from a target selector, `-fabi-version` |
| 3 | constexpr variable must be initialized by a constant expression | 54 | 50 | a | GCC differs on UB in constant evaluation: a member written before its lifetime starts ([basic.life]/7), member pointer cast to an unrelated class ([expr.static.cast]/11), `delete` through a base without a virtual destructor ([expr.delete]/3), an object read in its own initializer; b: `-fconstexpr-fp-except`, vector `==`, zero-size arrays |
| 4 | a type specifier is required | 53 | 53 | a | implicit `int` ([dcl.type.general]/3) |
| 5 | unknown type name | 53 | 42 | b | Concepts TS `concept bool` (`-fconcepts`), `__constinit`, GNU attribute in a parenthesized declarator (`__remove_reference` fixed by WP-1633) |
| 6 | `_Complex` is a C99 extension | 42 | 42 | a | a reserved identifier used as an extension ([lex.name]/4); an error only under `-pedantic-errors` |
| 7 | expected 'token' | 40 | 26 | b | GCC 15 toplevel `asm` with operands; a: `__builtin_offsetof` with a qualified member ([support.types.layout]/1), `_Alignas` as an identifier ([lex.name]/4) |
| 8 | no matching function for call | 37 | 14 | a | CWG1286 (open), `-fpermissive` pointer conversions, relaxed exception specifications in system headers |
| 9 | link: undefined reference | 37 | 31 | b | `link_error` folded only by GCC's optimizer, `#pragma interface` (WP-1683), `-nostdlib++` with weak `__cxa_pure_virtual`, alias attribute on a typeinfo |
| 10 | invalid suffix on literal (reserved UDL) | 36 | 34 | a | a ud-suffix without `_` is reserved ([over.literal]/8, [usrlit.suffix]); `"x"MACRO` is one token ([lex.pptoken]/5) |
| 11 | ISO C++11 does not allow access declarations | 34 | 34 | a | removed in C++11 (C++03 [class.access.dcl]) |
| 12 | static assertion expression is not an ICE | 34 | 14 | a | subtracting pointers into different subobjects ([expr.add]/5), arithmetic on a pointer to a non-similar type ([expr.add]/6), `&*nullptr`, `frexp` before C++23, `void*` cast to a non-similar type ([expr.const.core]/2.15); b: `__builtin_strstr`/`index`/`rindex` |
| 13 | redefinition | 30 | 27 | b | `gnu_inline` extern-inline redefinitions (clang the same; WP-1550 left them); a: a namespace alias reusing its namespace's name ([basic.link]/8) |
| 14 | variable-sized object may not be initialized | 30 | 27 | a | initialized VLAs are a GNU extension ([dcl.array]/1) |
| 15 | attribute cannot be applied to types | 28 | 25 | a | an attribute or `alignas` that appertains to a type ([dcl.attr.grammar]/7, [dcl.align]/1); GCC ignores it with a warning |
| 16 | use 'template' keyword for dependent template name | 27 | 12 | a | a dependent `X::Y<int>::Z` without `template` ([temp.names]/7) (the current-instantiation part was fixed by WP-1530, WP-1570, WP-1609) |
| 17 | pack expansion contains no unexpanded packs | 25 | 4 | b | `__integer_pack` uses left in GCC-rejected forms (WP-1667 fixed the rest) |
| 18 | static assertion failed (with message) | 25 | 25 | a | `noexcept(T())` with a trivial `noexcept(false)` constructor (CWG2820), nothrow array new with a run-time bound ([except.spec]/6), list-init of a reference through a conversion function ([dcl.init.list]/3.10), `__func__` length, closure size; b: `-fabi-version`; c 2: `new T[](args)` ([expr.new]/7) |
| 19 | expected unqualified-id / constructor name | 23 | 10 | a | explicit template arguments on a qualified constructor name ([class.qual]/1, [temp.arg.explicit]/2) |
| 20 | cannot initialize a variable of type | 21 | 21 | a | `-fpermissive` conversions, PMF to `void*` |
| 21 | character not allowed in an identifier | 21 | 21 | a | bidi controls are not XID_Continue ([lex.name]/1) |
| 22 | 'auto' not allowed in function prototype | 20 | 20 | b | Concepts TS `-fconcepts` at C++14 |
| 23 | an attribute list cannot appear here | 18 | 9 | a | attributes on a non-defining friend ([dcl.attr.grammar]/7), after an elaborated class name, after a namespace name (CWG2262 fixed by WP-1536) |
| 24 | reference member binds to a temporary | 17 | 14 | a | ill-formed ([class.base.init]/8); GCC only warns |
| 25 | cannot be defined in a type specifier | 16 | 13 | a | a GNU compound literal that defines a union |
| 26 | implicit instantiation of undefined template | 16 | 16 | a | CWG1286 (open), `gnu::aligned` after an array declarator, IFNDR ([temp.res.general]/6) |
| 27 | is a private member | 15 | 15 | a | IFNDR in a template never instantiated ([temp.res.general]/6) |
| 29 | structured binding cannot be declared X | 15 | 15 | a | `constexpr`/`static` structured bindings before C++26/C++20 ([dcl.pre]/7) |
| 30 | type cannot be used prior to '::' | 15 | 9 | a | unspecified whether the instantiation happens ([temp.inst]/10) |

Dropped: #28 (`__builtin_shufflevector` of different lengths, WP-1548).

#### Accepts-invalid, clang accepts too (549 cases; 552 on x86-64)

| # | cluster (expected GCC message) | second | third | verdict | reason |
|---|---|---:|---:|---|---|
| 1 | (empty pattern) | 41 | 35 | a | CWG727/CWG393, partial specialization after use (IFNDR), floating new-array bound since C++14, GNU asm label on a member, deducing an explicit object parameter of a captureless lambda; b: `-fno-elide-constructors` forms, packed return |
| 2 | changes meaning | 27 | 27 | a | IFNDR ([class.member.lookup]/6) |
| 3 | is not supported for this target | 18 | 18 | b | `-fcf-protection` (x86 CET) |
| 4 | conflicting | 12 | 12 | a | redeclared `__cxa_*` (reserved, [lex.name]/4); IFNDR |
| 5 | not a member of `coroutine_handle` | 9 | 9 | a | a user `std::coroutine_handle` ([namespace.std]/1) |
| 6 | ambiguous | 9 | 4 | a | the inherited-constructor and constraint cases were fixed (WP-1531, WP-1532); left: CWG2327 conversion functions (clang's result, deviations.md) |
| 7 | arithmetic on pointer to an empty aggregate | 9 | 9 | a | zero-length arrays are GNU |
| 8 | narrowing (in system header) | 9 | 9 | a | decided: kept with clang 22 (WP-1538, deviations.md) |
| 9 | redeclared | 9 | 9 | a | `__cxa_*` reserved names |
| 10 | does not declare anything | 8 | 8 | a | kiln warns, a diagnostic ([intro.compliance.general]/2) |
| 11 | anonymous struct with base | 6 | 6 | a | GNU extension |
| 12 | cannot bind packed field | 6 | 6 | a | GCC policy for `packed` |
| 13 | cannot tail-call | 6 | 6 | a | `musttail` feasibility is checked in codegen |
| 14 | conflicting declaration | 6 | 6 | a | `__dso_handle`, `__dynamic_cast` reserved |
| 15 | depth | 6 | 3 | b | `-ftemplate-depth-50` spelling (the class-scope specialization part is valid, CWG727) |
| 17 | incomplete | 6 | 4 | a | a member of an incomplete self type in a template never instantiated |
| 18 | invalid arguments for `-fpatchable-function-entry` | 6 | 6 | b | flag value check |
| 19 | match | 6 | 6 | b | `-fno-new-ttp-matching` |
| 20 | no match | 6 | 6 | b | `-fno-new-inheriting-ctors`; a: IFNDR |
| 21 | conflicts | 5 | 2 | a | what WP-1534 left: a using-declarator of the class's own enumerator, accepted as clang 22 (deviations.md) |
| 22 | indeterminate attr not on first declaration | 4 | 4 | a | `[[indeterminate]]` is C++26 |
| 23 | deleted | 4 | 4 | a | a prvalue initializes a `[[no_unique_address]]` member directly (CWG2403 open) |
| 24 | designator used multiple times | 4 | 4 | a | kiln warns (`-Winitializer-overrides`) |
| 25 | mixed designated and non-designated | 4 | 4 | a | kiln warns (`-Wc99-designator`) |
| 26-30 | `strict_flex_array`, `-fdirectives-only` (2), `-Wnormalized`, `[[gnu::unavailable]]` | 15 | 15 | b | GCC attributes and flags |

Dropped: #16 (a function template named as a template argument is
instantiated, WP-1533).

### Wrong-line

**Clang matches (567 cases, 167 clusters).** WP-1540 owns the
end-of-input location (34 cases: "expected '}'" and "expected
unqualified-id" at line N+1). Of the rest, 373 cases miss an error clang
22 reports on GCC's line and 12 stop at "not yet implemented"
(`cpp1z/using-variadic1`: a pack expansion in a using-declaration of a
dependent base); 150 have an extra error clang does not give (follow-on
errors after a parse error, `range-for23`, `template/void3`). The
largest: missed "expected" (12, `pr65202`: kiln reports on the next
line), missed "ambiguous" (12, `variadic-alias6` L19), missed "no match"
(12, `lookup/operator-4`: kiln misses the operator lookup errors clang
reports on 19 lines), missed "private" (6, `parse/access14` L18), missed
"depends on itself" (6, `concepts-pr88395`). WP-1698: the missed errors
where clang matches, largest clusters first.

**Clang differs too (2338 cases, 543 clusters).** The second run's
verdicts hold; counts are this run's:

| # | cluster | cases | verdict | reason |
|---:|---|---:|---|---|
| 1 | missed (any error expected) | 133 | stays | locations next to GCC's (the "any error expected" tables of WP-1473; its R clusters were fixed by WP-1539, WP-1630, WP-1645) |
| 2 | missed "modifying a const object" | 45 | stays | error at the use with a note at GCC's line, as clang |
| 3 | extra "unknown type name" | 45 | stays | Concepts TS `concept bool`, `__constinit` |
| 4 | extra "no matching function for call" | 34 | stays | same errors and lines as clang (Concepts TS) |
| 5 | extra "expected '…'" | 33 | WP-1540 | end-of-input location (8 of 11 tests) |
| 6 | extra "use of undeclared identifier" | 33 | stays | GCC builtins and recovery errors clang reports too |
| 7 | missed "expected" | 28 | stays | GCC's parser stops elsewhere |
| 8 | missed "no match" | 25 | stays | at the call or the deleted inherited constructor, as clang |
| 9 | missed "jump to label" | 24 | stays | at the `goto`, as clang |
| 10 | missed "call to non-constexpr function" | 23 | stays | at the constant expression with a note at the call |
| 11 | missed "not declared" | 23 | stays | where clang reports it |
| 12 | missed "uninitialized" | 18 | stays | kiln and clang warn on the class and error at the use |
| 13 | missed "reference to '_' is ambiguous" | 16 | stays | `_` before C++26, as clang |
| 14 | extra "constexpr variable must be initialized" | 15 | stays | clang rejects the same variables |
| 15 | extra "no member named" | 15 | stays | as clang |
| 16 | missed "ambiguous" | 15 | stays | Concepts TS and redeclarations, as clang |
| 17 | missed "instantiating erroneous template" | 15 | stays | GCC's `-fpermissive` follow-up error |
| 18 | missed "is not valid in an identifier" | 15 | stays | line for line as clang |
| 19 | missed "deleted" | 13 | stays | at the use, as clang |
| 20 | extra "array is too large" | 12 | stays | as clang (`array-5`) |

### Priority directories

| directory | cases | first | second | third | x86-64 third | what is left (third, aarch64) |
|---|---:|---:|---:|---:|---:|---|
| `g++.dg/reflect` | 565 | 306 | 447 | 482 | 482 | 34 compile-reject, 40 wrong-line, 9 accepts-invalid (tables below) |
| `g++.dg/contracts` | 123 | 52 | 100 | 100 | 100 | 4 rejects-valid (the [dcl.contract.func]/4-5 and [temp.expl.spec] deviations), 19 wrong-line (spec/contracts.md) |
| `g++.dg/cpp26` | 435 | 270 | 309 | 314 | 314 | 34 compile-reject (15 GCC's pre-C++26 constexpr structured bindings), 78 wrong-line, 6 accepts-invalid, 3 wrong (`expansion-stmt17`) |
| `g++.dg/cpp2a` | 3784 | 2840 | 3074 | 3113 | 3113 | 170 compile-reject, 403 wrong-line, 91 accepts-invalid, 7 expect-unknown |
| `g++.dg/cpp23` | 587 | 385 | 440 | 447 | 447 | 36 compile-reject, 77 wrong-line, 24 accepts-invalid, 3 expect-unknown |
| `g++.dg/torture` | 1149 | 1066 | 1077 | 1107 | 1105 of 1155 | 30 compile-reject (GCC-only builtins and options), 2 accepts-invalid, 8 wrong, 2 expect-unknown |
| `c-c++-common/torture` | 282 | 172 | 184 | 256 | 252 of 278 | 24 rejects, 2 wrong (`builtin-clear-padding-4`) |

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

After WP-1670 and WP-1671 (partial ordering of implicit object against explicit object member function templates; lambdas in unevaluated operands before C++20 and in template arguments at C++17): over `g++.dg/cpp23`, `cpp2a/lambda*`, `cpp2a/concepts*`, `cpp0x/lambda`, `cpp1y/lambda*`, `cpp1z/lambda*`, `cpp1z/constexpr-lambda*`, `template`, `overload` and `g++.old-deja/g++.pt`, 9442 to 9445 of 10576 decided cases pass (`lambda-uneval` at C++11 and C++17 and `lambda-conv15` at C++17, all accept-invalid before), none regressed; `cpp23/explicit-obj-constraints` reports 16 ambiguities, clang 22's static-member ones, instead of 48.

### x86-64

The third run's x86-64 rows come from the opt-in `gxx-dg` jobs of
`.github/workflows/sweep.yml` (docs/VALIDATION.md, "x86_64 sweep") at the
same kiln `01c4bb2f5`: four shards of 4:37 to 6:02, peak RSS 1.93 GB per
shard, merged with `gxx-dg-compare.py merge --expect 4` (identical to the
job's own summary rows). Same oracles as this host (GCC 16.1.0, clang
22.1.8) and the same libstdc++.

**50475 of 56870 decided cases pass (88.8%)**, 200 `expect-unknown`
(second run: 49708 of 57070, 87.1%); 57749 rows against 57552 on
aarch64 (tests whose `dg-do` target or effective target selects one
architecture are one skip row on the other host). Against the second
x86-64 run: 801 cases to pass, the same 33 cases from pass as on aarch64
(above), 27 between failing classes, 200 into `expect-unknown`.

Of the 57408 cases both hosts ran, 9 pass on aarch64 and fail on
x86-64, 3 the other way, 2 are `expect-unknown` on x86-64 only; none
moves between two failing classes:

- target-specific by the test's own directives: `abi/pr87137`
  (`ms_struct` layout, 3), `cpp1z/register1`/`register3` (global
  register variable in `ebx`, 3), with clang failing the same way;
  `cpp0x/pr77948-1`/`3`/`4` pass on x86-64 only (the `q` literal suffix
  GCC expects to be unsupported on aarch64, 3);
- the target ABI: `torture/pr35526` (plain `char` is signed on x86-64,
  so a constant above 127 narrows; the test prunes "narrowing" output,
  so the case is `expect-unknown`, 2);
- the step limit: `other/pr117516` times out at C++98 and C++20 on
  x86-64 and at C++26 on both hosts (the crash / hang bucket above): a
  compile time near the 60 s limit, not a host difference;
- kiln alone: `g++.old-deja/g++.eh/unwind1` at C++26 exits 1 on x86-64,
  unchanged since the second run (WP-1544).

No new kiln difference between the hosts. Of the cases only x86-64
runs, every failure fails under clang 22 the same way (GCC's
`__builtin_ia32_*` builtins, `__float80`, `target` multiversioning,
global register variables, the `abi/empty*` tests that link a
companion file the runner does not compile); `ext/pr106877`,
`ext/pr84683` and `ext/pr84828` pass since WP-1543. The aarch64-only
failures are the `sve-sizeless` diagnostics (6, clang the same).

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

- tests: GCC `6afcc4f6da931eb93f3ab001a0dd9650ea71d1ea`; kiln `01c4bb2f5`
- command: `cargo xtask gxx-dg -j 8 --suite dg,torture,common,old-deja --opt -O0,-O2 --profile release`
- kiln: kiln version 1.2.1 (C++26 frontend, LLVM IR backend) (-ferror-limit=0) (-fclang-compat=22) (--gcc-toolchain=<GCC oracle's prefix>)
- gcc: g++ (GCC) 16.1.0 (-fdiagnostics-plain-output)
- clang: Debian clang version 22.1.8 (1~deb13u4) (-ferror-limit=0) (--gcc-toolchain=<GCC oracle's prefix>)
- oracles: only the cases kiln does not pass (their counts below cover those cases)
- limits: 2048 MiB address space per step (plus kiln's stack reservation); compile/link timeout 60 s, run timeout 10 s; -j 8
- 20571 tests, 56766 cases run, 786 skip rows; wall 1043 s

### Results by class (cases)

| class | kiln | gcc | clang |
|---|---:|---:|---:|
| pass | 50256 | 6269 | 1235 |
| compile-reject | 2290 | 32 | 1995 |
| crash | 0 | 3 | 11 |
| wrong | 121 | 2 | 100 |
| timeout | 1 | 0 | 0 |
| accept-invalid | 995 | 40 | 591 |
| wrong-line | 2905 | 21 | 2382 |
| expect-unknown | 198 | 143 | 196 |
| total | 56766 | 6510 | 6510 |

### kiln's failures by triage bucket (cases)

| bucket | cases | GCC's testsuite marks xfail |
|---|---:|---:|
| kiln crash / hang | 1 | 0 |
| wrong-code | 121 | 2 |
| rejects-valid, clang accepts | 309 | 3 |
| rejects-valid, clang rejects too | 1981 | 0 |
| accepts-invalid, clang rejects | 446 | 0 |
| accepts-invalid, clang accepts too | 549 | 0 |
| wrong-line, clang matches | 567 | 0 |
| wrong-line, clang differs too | 2338 | 0 |

### Priority directories (kiln, cases)

| directory | cases | pass | compile-reject | accept-invalid | wrong-line | wrong | crash | timeout | expect-unknown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| g++.dg/reflect | 565 | 482 | 34 | 9 | 40 | 0 | 0 | 0 | 0 |
| g++.dg/contracts | 123 | 100 | 4 | 0 | 19 | 0 | 0 | 0 | 0 |
| g++.dg/cpp26 | 435 | 314 | 34 | 6 | 78 | 3 | 0 | 0 | 0 |
| g++.dg/cpp2a | 3784 | 3113 | 170 | 91 | 403 | 0 | 0 | 0 | 7 |
| g++.dg/cpp23 | 587 | 447 | 36 | 24 | 77 | 0 | 0 | 0 | 3 |
| g++.dg/torture | 1149 | 1107 | 30 | 2 | 0 | 8 | 0 | 0 | 2 |
| c-c++-common/torture | 282 | 256 | 24 | 0 | 0 | 2 | 0 | 0 | 0 |

### By directory (kiln, cases)

| directory | cases | pass | compile-reject | accept-invalid | wrong-line | wrong | crash | timeout | expect-unknown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| c-c++-common | 2866 | 2506 | 156 | 95 | 70 | 9 | 0 | 0 | 30 |
| c-c++-common/cpp | 420 | 306 | 51 | 24 | 36 | 0 | 0 | 0 | 3 |
| c-c++-common/torture | 282 | 256 | 24 | 0 | 0 | 2 | 0 | 0 | 0 |
| g++.dg | 942 | 879 | 31 | 8 | 24 | 0 | 0 | 0 | 0 |
| g++.dg/DRs | 203 | 165 | 3 | 7 | 28 | 0 | 0 | 0 | 0 |
| g++.dg/abi | 772 | 720 | 38 | 5 | 3 | 6 | 0 | 0 | 0 |
| g++.dg/concepts | 409 | 281 | 45 | 6 | 77 | 0 | 0 | 0 | 0 |
| g++.dg/contracts | 123 | 100 | 4 | 0 | 19 | 0 | 0 | 0 | 0 |
| g++.dg/conversion | 279 | 264 | 4 | 3 | 8 | 0 | 0 | 0 | 0 |
| g++.dg/coroutines | 533 | 426 | 20 | 15 | 72 | 0 | 0 | 0 | 0 |
| g++.dg/cpp | 213 | 177 | 10 | 7 | 18 | 1 | 0 | 0 | 0 |
| g++.dg/cpp0x | 9121 | 7973 | 402 | 143 | 552 | 9 | 0 | 0 | 42 |
| g++.dg/cpp1y | 2133 | 1789 | 102 | 35 | 204 | 0 | 0 | 0 | 3 |
| g++.dg/cpp1z | 1707 | 1506 | 72 | 29 | 93 | 4 | 0 | 0 | 3 |
| g++.dg/cpp23 | 587 | 447 | 36 | 24 | 77 | 0 | 0 | 0 | 3 |
| g++.dg/cpp26 | 435 | 314 | 34 | 6 | 78 | 3 | 0 | 0 | 0 |
| g++.dg/cpp2a | 3784 | 3113 | 170 | 91 | 403 | 0 | 0 | 0 | 7 |
| g++.dg/delayedfold | 21 | 15 | 3 | 0 | 0 | 3 | 0 | 0 | 0 |
| g++.dg/diagnostic | 579 | 465 | 12 | 43 | 44 | 3 | 0 | 0 | 12 |
| g++.dg/eh | 394 | 368 | 4 | 15 | 7 | 0 | 0 | 0 | 0 |
| g++.dg/expr | 406 | 385 | 12 | 1 | 5 | 0 | 0 | 0 | 3 |
| g++.dg/ext | 2901 | 2263 | 309 | 105 | 177 | 39 | 0 | 0 | 8 |
| g++.dg/inherit | 337 | 311 | 8 | 3 | 15 | 0 | 0 | 0 | 0 |
| g++.dg/init | 1101 | 996 | 30 | 20 | 51 | 4 | 0 | 0 | 0 |
| g++.dg/ipa | 686 | 682 | 4 | 0 | 0 | 0 | 0 | 0 | 0 |
| g++.dg/lookup | 1025 | 905 | 39 | 38 | 43 | 0 | 0 | 0 | 0 |
| g++.dg/opt | 1442 | 1381 | 43 | 3 | 0 | 15 | 0 | 0 | 0 |
| g++.dg/other | 1033 | 933 | 19 | 10 | 57 | 0 | 0 | 1 | 13 |
| g++.dg/overload | 294 | 261 | 12 | 3 | 12 | 0 | 0 | 0 | 6 |
| g++.dg/parse | 1851 | 1514 | 58 | 57 | 207 | 0 | 0 | 0 | 15 |
| g++.dg/reflect | 565 | 482 | 34 | 9 | 40 | 0 | 0 | 0 | 0 |
| g++.dg/rtti | 118 | 106 | 0 | 3 | 9 | 0 | 0 | 0 | 0 |
| g++.dg/tc1 | 105 | 87 | 6 | 0 | 9 | 0 | 0 | 0 | 3 |
| g++.dg/template | 4699 | 4123 | 190 | 125 | 240 | 3 | 0 | 0 | 18 |
| g++.dg/torture | 1149 | 1107 | 30 | 2 | 0 | 8 | 0 | 0 | 2 |
| g++.dg/tree-ssa | 860 | 841 | 16 | 0 | 3 | 0 | 0 | 0 | 0 |
| g++.dg/warn | 2840 | 2710 | 83 | 15 | 18 | 3 | 0 | 0 | 11 |
| g++.dg/wrappers | 30 | 23 | 4 | 0 | 0 | 0 | 0 | 0 | 3 |
| g++.old-deja/g++.abi | 111 | 108 | 3 | 0 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.benjamin | 126 | 111 | 6 | 0 | 9 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.bob | 54 | 51 | 0 | 0 | 3 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.brendan | 919 | 900 | 9 | 2 | 8 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.bugs | 286 | 272 | 2 | 2 | 10 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.eh | 207 | 206 | 0 | 0 | 1 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.ext | 117 | 75 | 28 | 2 | 6 | 6 | 0 | 0 | 0 |
| g++.old-deja/g++.gb | 39 | 39 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.jason | 1217 | 1173 | 10 | 9 | 24 | 0 | 0 | 0 | 1 |
| g++.old-deja/g++.law | 771 | 738 | 21 | 6 | 6 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.martin | 54 | 49 | 5 | 0 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.mike | 1040 | 1004 | 33 | 0 | 3 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.niklas | 81 | 81 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.ns | 273 | 267 | 0 | 0 | 6 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.oliva | 114 | 99 | 6 | 6 | 3 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.other | 1509 | 1437 | 23 | 4 | 45 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.pt | 2177 | 2070 | 25 | 14 | 56 | 3 | 0 | 0 | 9 |
| g++.old-deja/g++.rfg | 6 | 6 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.robertl | 402 | 372 | 1 | 0 | 26 | 0 | 0 | 0 | 3 |
| g++.old-deja/g++.warn | 18 | 18 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| total | 56766 | 50256 | 2290 | 995 | 2905 | 121 | 0 | 1 | 198 |

### Clusters: kiln crash / hang (1 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 1 | timeout: compile timeout | g++.dg/other/pr117516.C -std=c++26 |

### Clusters: wrong-code (2 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 84 | wrong: signal: N (SIGABRT) | c-c++-common/auto-init-padding-3.c -std=gnu++20, c-c++-common/auto-init-padding-3.c -std=gnu++26, c-c++-common/auto-init-padding-3.c -std=gnu++98 |
| 37 | wrong: exit N | g++.dg/abi/bitfield1.C -std=gnu++20, g++.dg/abi/bitfield1.C -std=gnu++26, g++.dg/abi/bitfield1.C -std=gnu++98 |

### Clusters: rejects-valid, clang accepts (90 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 12 | compile-reject: error: functions that differ only in their return type cannot be overloaded | g++.dg/ext/attrib63.C -std=c++20, g++.dg/ext/attrib63.C -std=c++26, g++.dg/ext/attrib63.C -std=c++98 |
| 7 | compile-reject: error: the GNU '…' operator on class types is not supported yet | g++.dg/ext/cond3.C -std=gnu++20, g++.dg/ext/cond3.C -std=gnu++26, g++.dg/ext/cond3.C -std=gnu++98 |
| 6 | compile-reject: error: call to deleted function '…' | g++.dg/cpp0x/sfinae70.C -std=c++26, g++.dg/cpp0x/sfinae71.C -std=c++26, g++.dg/cpp0x/sfinae72.C -std=c++26 |
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
| 3 | compile-reject: error: ISO C++N does not allow incrementing expression of type bool [-Wincrement-bool] | g++.dg/expr/bitfield4.C -std=c++98, g++.dg/expr/bitfield6.C -std=c++98, g++.dg/expr/lval4.C -std=c++98 |
| 3 | compile-reject: error: address of overloaded function '…' does not match required type '…' | g++.dg/cpp1y/auto-96647.C -std=c++14, g++.dg/cpp1y/auto-96647.C -std=c++20, g++.dg/cpp1y/auto-96647.C -std=c++26 |
| 3 | compile-reject: error: ambiguous partial specializations of '…' | g++.dg/template/partial12.C -std=c++20, g++.dg/template/partial12.C -std=c++26, g++.dg/template/partial12.C -std=c++98 |

### Clusters: rejects-valid, clang rejects too (331 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 64 | compile-reject: error: use of undeclared identifier '…' | c-c++-common/Wsuggest-attribute-1.c -std=gnu++11, c-c++-common/Wsuggest-attribute-1.c -std=gnu++20, c-c++-common/Wsuggest-attribute-1.c -std=gnu++26 |
| 59 | compile-reject: error: static assertion failed due to requirement '…' | g++.dg/abi/no_unique_address8a.C -std=gnu++11, g++.dg/abi/no_unique_address8a.C -std=gnu++20, g++.dg/abi/no_unique_address8a.C -std=gnu++26 |
| 53 | compile-reject: error: a type specifier is required for all declarations | g++.dg/abi/vbase8-10.C -std=gnu++20, g++.dg/abi/vbase8-10.C -std=gnu++26, g++.dg/abi/vbase8-10.C -std=gnu++98 |
| 50 | compile-reject: error: constexpr variable '…' must be initialized by a constant expression | g++.dg/cpp0x/constexpr-92443.C -std=c++11, g++.dg/cpp0x/constexpr-92443.C -std=c++20, g++.dg/cpp0x/constexpr-92443.C -std=c++26 |
| 42 | compile-reject: error: '…' is a CN extension [-Werror,-WcN-extensions] | g++.dg/cpp0x/constexpr-71828.C -std=c++11, g++.dg/cpp0x/constexpr-71828.C -std=c++20, g++.dg/cpp0x/constexpr-71828.C -std=c++26 |
| 42 | compile-reject: error: unknown type name '…' | g++.dg/concepts/abbrev11.C -std=c++14, g++.dg/concepts/class5.C -std=gnu++17, g++.dg/concepts/debug1.C -std=gnu++17 |
| 34 | compile-reject: error: ISO C++N does not allow access declarations; use using declarations instead | g++.dg/cpp0x/pr70538.C -std=c++11, g++.dg/cpp0x/pr70538.C -std=c++20, g++.dg/cpp0x/pr70538.C -std=c++26 |
| 34 | compile-reject: error: invalid suffix on literal; C++N requires a space between literal and identifier [-Wreserved-user-defined-literal] | c-c++-common/cpp/dir-only-7.c -, g++.dg/cpp0x/Wliteral-suffix.C -std=c++11, g++.dg/cpp0x/Wliteral-suffix.C -std=c++20 |
| 31 | compile-reject: link: undefined reference to `… | c-c++-common/pr19807-2.c -std=gnu++20, c-c++-common/pr19807-2.c -std=gnu++26, c-c++-common/pr19807-2.c -std=gnu++98 |
| 27 | compile-reject: error: redefinition of '…' | g++.dg/cpp0x/alias-decl-targ1.C -std=c++11, g++.dg/cpp0x/alias-decl-targ1.C -std=c++20, g++.dg/cpp0x/alias-decl-targ1.C -std=c++26 |
| 27 | compile-reject: error: variable-sized object may not be initialized | g++.dg/cpp1y/lambda-vla1.C -std=c++14, g++.dg/cpp1y/lambda-vla1.C -std=c++20, g++.dg/cpp1y/lambda-vla1.C -std=c++26 |
| 26 | compile-reject: error: expected '…' | c-c++-common/toplevel-asm-1.c -std=gnu++20, c-c++-common/toplevel-asm-1.c -std=gnu++26, c-c++-common/toplevel-asm-1.c -std=gnu++98 |
| 25 | compile-reject: error: '…' attribute cannot be applied to types | g++.dg/cpp0x/alias-decl-59.C -std=c++11, g++.dg/cpp0x/alias-decl-59.C -std=c++20, g++.dg/cpp0x/alias-decl-59.C -std=c++26 |
| 25 | compile-reject: error: static assertion failed due to requirement '…':  | g++.dg/abi/base-defaulted1a.C -std=c++20, g++.dg/abi/base-defaulted1a.C -std=c++26, g++.dg/cpp0x/initlist-ref3.C -std=c++11 |
| 21 | compile-reject: error: cannot initialize a variable of type '…' with an rvalue of type '…' | c-c++-common/pr60101.c -std=gnu++20, c-c++-common/pr60101.c -std=gnu++26, c-c++-common/pr60101.c -std=gnu++98 |
| 21 | compile-reject: error: character <U+NA> not allowed in an identifier | c-c++-common/Wbidi-chars-10.c -std=gnu++20, c-c++-common/Wbidi-chars-10.c -std=gnu++26, c-c++-common/Wbidi-chars-10.c -std=gnu++98 |
| 20 | compile-reject: error: '…' not allowed in function prototype | g++.dg/concepts/abbrev1.C -std=c++14, g++.dg/concepts/abbrev10.C -std=c++14, g++.dg/concepts/abbrev2.C -std=c++14 |
| 16 | compile-reject: error: implicit instantiation of undefined template '…' | g++.dg/cpp0x/alias-decl-dr1286.C -std=c++11, g++.dg/cpp0x/alias-decl-dr1286.C -std=c++20, g++.dg/cpp0x/alias-decl-dr1286.C -std=c++26 |
| 15 | compile-reject: error: '…' is a private member of '…' | g++.dg/template/inherit9.C -std=c++20, g++.dg/template/inherit9.C -std=c++26, g++.dg/template/inherit9.C -std=c++98 |
| 15 | compile-reject: error: structured binding declaration cannot be declared '…' | g++.dg/cpp26/decomp22.C -std=gnu++11, g++.dg/cpp26/decomp22.C -std=gnu++14, g++.dg/cpp26/decomp22.C -std=gnu++20 |
| 14 | compile-reject: error: constexpr function never produces a constant expression [-Winvalid-constexpr] | g++.dg/cpp0x/constexpr-noreturn1.C -std=c++11, g++.dg/cpp0x/constexpr-noreturn1.C -std=c++20, g++.dg/cpp0x/constexpr-noreturn1.C -std=c++26 |
| 14 | compile-reject: error: invalid vector element type '…' | c-c++-common/pr123437.c -std=c++20, c-c++-common/pr123437.c -std=c++26, c-c++-common/pr123437.c -std=c++98 |
| 14 | compile-reject: error: no matching function for call to '…' | g++.dg/cpp0x/alias-decl-0.C -std=c++11, g++.dg/cpp0x/alias-decl-0.C -std=c++20, g++.dg/cpp0x/alias-decl-0.C -std=c++26 |
| 14 | compile-reject: error: reference member '…' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object | g++.dg/cpp0x/constexpr-ctor11.C -std=c++11, g++.dg/cpp0x/constexpr-ctor11.C -std=c++20, g++.dg/cpp0x/constexpr-ctor11.C -std=c++26 |
| 14 | compile-reject: error: static assertion expression is not an integral constant expression | g++.dg/cpp0x/constexpr-50060.C -std=c++14, g++.dg/cpp0x/constexpr-50060.C -std=c++20, g++.dg/cpp0x/constexpr-nullptr-1.C -std=gnu++11 |
| 13 | compile-reject: error: '…' cannot be defined in a type specifier | g++.dg/ext/complit15.C -, g++.dg/opt/pr20931.C -std=gnu++20, g++.dg/opt/pr20931.C -std=gnu++26 |
| 12 | compile-reject: error: arithmetic on a pointer to void | c-c++-common/Wpointer-arith-1.c -std=gnu++20, c-c++-common/Wpointer-arith-1.c -std=gnu++26, c-c++-common/Wpointer-arith-1.c -std=gnu++98 |
| 12 | compile-reject: error: call to '…' is ambiguous | g++.dg/cpp2a/array-conv3.C -std=c++20, g++.dg/cpp2a/array-conv3.C -std=c++23, g++.dg/cpp2a/array-conv3.C -std=c++26 |
| 12 | compile-reject: error: misplaced attributes; expected attributes here | g++.dg/cpp0x/gen-attrs-18.C -std=c++11, g++.dg/cpp0x/gen-attrs-18.C -std=c++20, g++.dg/cpp0x/gen-attrs-18.C -std=c++26 |
| 12 | compile-reject: error: reinterpret_cast from '…' to '…' (aka '…') is not allowed | g++.dg/ext/attr-ifunc-1.C -std=gnu++20, g++.dg/ext/attr-ifunc-1.C -std=gnu++26, g++.dg/ext/attr-ifunc-1.C -std=gnu++98 |

### Clusters: accepts-invalid, clang rejects (183 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 14 | accept-invalid: no error (expected: ) | g++.dg/cpp0x/constexpr-initlist11a.C -std=c++26, g++.dg/parse/template7.C -, g++.dg/template/varmod1.C -std=gnu++20 |
| 5 | accept-invalid: no error (expected: invalid use of '…') | g++.dg/cpp2a/lambda-generic3.C -std=gnu++98, g++.dg/cpp2a/lambda-generic4.C -std=gnu++98, g++.dg/lookup/this2.C -std=c++11 |
| 5 | accept-invalid: no error (expected: use of deleted function) | g++.dg/cpp0x/move-return5.C -std=c++11, g++.dg/cpp0x/move-return5.C -std=c++20, g++.dg/cpp1z/inline-var11.C -std=c++17 |
| 4 | accept-invalid: no error (expected: not a class) | g++.dg/cpp0x/alias-decl-80.C -std=c++11, g++.dg/cpp0x/alias-decl-80.C -std=c++20, g++.dg/cpp0x/alias-decl-80.C -std=c++26 |
| 4 | accept-invalid: no error (expected: not a constant expression) | g++.dg/cpp2a/concepts-pr67697.C -std=c++20, g++.dg/cpp2a/concepts-pr67697.C -std=c++23, g++.dg/cpp2a/concepts-pr67697.C -std=c++26 |
| 3 | accept-invalid: no error (expected: '…' has no member named '…') | g++.dg/coroutines/pr112341-2.C -std=c++20, g++.dg/coroutines/pr112341-2.C -std=c++23, g++.dg/coroutines/pr112341-2.C -std=c++26 |
| 3 | accept-invalid: no error (expected: '…' qualifiers cannot) | g++.dg/template/qualttp20.C -std=gnu++20, g++.dg/template/qualttp20.C -std=gnu++26, g++.dg/template/qualttp20.C -std=gnu++98 |
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
| 3 | accept-invalid: no error (expected: N:defining explicit specialization) | g++.dg/template/friend64.C -std=c++20, g++.dg/template/friend64.C -std=c++26, g++.dg/template/friend64.C -std=c++98 |
| 3 | accept-invalid: no error (expected: N:friend function definition '…' cannot have a name qualified with '…') | g++.dg/diagnostic/friend2.C -std=c++20, g++.dg/diagnostic/friend2.C -std=c++26, g++.dg/diagnostic/friend2.C -std=c++98 |
| 3 | accept-invalid: no error (expected: N:function body) | g++.dg/cpp1z/class-deduction10.C -std=c++17, g++.dg/cpp1z/class-deduction10.C -std=c++20, g++.dg/cpp1z/class-deduction10.C -std=c++26 |
| 3 | accept-invalid: no error (expected: N:invalid application of .__alignof. to a bit-field) | g++.dg/diagnostic/alignof3.C -std=c++20, g++.dg/diagnostic/alignof3.C -std=c++26, g++.dg/diagnostic/alignof3.C -std=c++98 |
| 3 | accept-invalid: no error (expected: N:qualifiers) | g++.dg/other/pr61753.C -std=c++20, g++.dg/other/pr61753.C -std=c++26, g++.dg/other/pr61753.C -std=c++98 |
| 3 | accept-invalid: no error (expected: N:template-id .foo<N>. in declaration\|explicit specialization) | g++.dg/template/spec33.C -std=c++20, g++.dg/template/spec33.C -std=c++26, g++.dg/template/spec33.C -std=c++98 |
| 3 | accept-invalid: no error (expected: N:too many ..... in lambda capture) | g++.dg/cpp2a/lambda-pack-init2.C -std=c++20, g++.dg/cpp2a/lambda-pack-init2.C -std=c++23, g++.dg/cpp2a/lambda-pack-init2.C -std=c++26 |

### Clusters: accepts-invalid, clang accepts too (160 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 35 | accept-invalid: no error (expected: ) | g++.dg/cpp0x/constexpr-ctor14a.C -std=gnu++11, g++.dg/cpp0x/constexpr-ctor14a.C -std=gnu++14, g++.dg/cpp1z/inh-ctor35.C -std=gnu++11 |
| 27 | accept-invalid: no error (expected: changes meaning) | g++.dg/ext/uow-3.C -std=gnu++20, g++.dg/ext/uow-3.C -std=gnu++26, g++.dg/ext/uow-3.C -std=gnu++98 |
| 18 | accept-invalid: no error (expected: '…' is not supported for this target) | c-c++-common/fcf-protection-1.c -std=gnu++20, c-c++-common/fcf-protection-1.c -std=gnu++26, c-c++-common/fcf-protection-1.c -std=gnu++98 |
| 12 | accept-invalid: no error (expected: conflicting) | g++.dg/eh/builtin10.C -std=c++20, g++.dg/eh/builtin10.C -std=c++26, g++.dg/eh/builtin10.C -std=c++98 |
| 9 | accept-invalid: no error (expected: '…' is not a member of '…') | g++.dg/coroutines/pr105475-1.C -std=c++20, g++.dg/coroutines/pr105475-1.C -std=c++23, g++.dg/coroutines/pr105475-1.C -std=c++26 |
| 9 | accept-invalid: no error (expected: arithmetic on pointer to an empty aggregate) | c-c++-common/pr58346-1.c -std=gnu++20, c-c++-common/pr58346-1.c -std=gnu++26, c-c++-common/pr58346-1.c -std=gnu++98 |
| 9 | accept-invalid: no error (expected: narrowing) | g++.dg/diagnostic/sys-narrow1.C -std=c++11, g++.dg/diagnostic/sys-narrow1.C -std=c++20, g++.dg/diagnostic/sys-narrow1.C -std=c++26 |
| 9 | accept-invalid: no error (expected: redeclared) | g++.dg/eh/builtin6.C -std=c++20, g++.dg/eh/builtin6.C -std=c++26, g++.dg/eh/builtin6.C -std=c++98 |
| 8 | accept-invalid: no error (expected: does not declare anything) | g++.dg/ext/anon-struct2.C -std=gnu++20, g++.dg/ext/anon-struct2.C -std=gnu++26, g++.dg/ext/anon-struct2.C -std=gnu++98 |
| 6 | accept-invalid: no error (expected: anonymous struct with base) | g++.dg/ext/anon-struct8.C -std=gnu++20, g++.dg/ext/anon-struct8.C -std=gnu++26, g++.dg/ext/anon-struct8.C -std=gnu++98 |
| 6 | accept-invalid: no error (expected: cannot bind packed field) | g++.dg/ext/packed3.C -std=c++20, g++.dg/ext/packed3.C -std=c++26, g++.dg/ext/packed3.C -std=c++98 |
| 6 | accept-invalid: no error (expected: cannot tail-call:) | c-c++-common/musttail2.c -std=c++11, c-c++-common/musttail2.c -std=c++20, c-c++-common/musttail2.c -std=c++26 |
| 6 | accept-invalid: no error (expected: conflicting declaration) | g++.dg/init/dso_handle2.C -std=gnu++20, g++.dg/init/dso_handle2.C -std=gnu++26, g++.dg/init/dso_handle2.C -std=gnu++98 |
| 6 | accept-invalid: no error (expected: invalid arguments for '…') | c-c++-common/patchable_function_entry-error-1.c -std=gnu++20, c-c++-common/patchable_function_entry-error-1.c -std=gnu++26, c-c++-common/patchable_function_entry-error-1.c -std=gnu++98 |
| 6 | accept-invalid: no error (expected: match) | g++.dg/template/ttp16.C -std=gnu++20, g++.dg/template/ttp16.C -std=gnu++26, g++.dg/template/ttp16.C -std=gnu++98 |
| 6 | accept-invalid: no error (expected: no match) | g++.dg/cpp0x/inh-ctor11.C -std=gnu++11, g++.dg/cpp0x/inh-ctor11.C -std=gnu++20, g++.dg/cpp0x/inh-ctor11.C -std=gnu++26 |
| 4 | accept-invalid: no error (expected: '…' attribute not specified for parameter '…' on the first declarati…) | g++.dg/cpp26/attr-indeterminate3.C -std=c++11, g++.dg/cpp26/attr-indeterminate3.C -std=c++20, g++.dg/cpp26/attr-indeterminate5.C -std=c++11 |
| 4 | accept-invalid: no error (expected: ambiguous) | g++.dg/concepts/variadic2.C -std=gnu++26, g++.dg/cpp2a/concepts-return-req1.C -std=c++20, g++.dg/cpp2a/concepts-return-req1.C -std=c++23 |
| 4 | accept-invalid: no error (expected: deleted) | g++.dg/cpp2a/no_unique_address7.C -std=c++20, g++.dg/cpp2a/no_unique_address7.C -std=c++26, g++.dg/cpp2a/no_unique_address7a.C -std=c++20 |
| 4 | accept-invalid: no error (expected: designator used multiple times in the same initializer list) | g++.dg/cpp0x/desig5.C -std=gnu++11, g++.dg/cpp0x/desig5.C -std=gnu++20, g++.dg/cpp0x/desig5.C -std=gnu++26 |
| 4 | accept-invalid: no error (expected: either all initializer clauses should be designated or none of them should be) | g++.dg/cpp/embed-14.C -std=gnu++20, g++.dg/cpp/embed-14.C -std=gnu++26, g++.dg/cpp2a/desig2.C -std=gnu++20 |
| 4 | accept-invalid: no error (expected: incomplete) | g++.dg/expr/discarded1a.C -std=c++98, g++.dg/template/pr71710.C -std=c++11, g++.dg/template/pr71710.C -std=c++20 |
| 3 | accept-invalid: no error (expected: '…' attribute may not be specified for '…') | g++.dg/strict-flex-array-1.C -std=gnu++20, g++.dg/strict-flex-array-1.C -std=gnu++26, g++.dg/strict-flex-array-1.C -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' expanded inside directive with '…') | c-c++-common/cpp/counter-2.c -std=gnu++20, c-c++-common/cpp/counter-2.c -std=gnu++26, c-c++-common/cpp/counter-2.c -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' is incompatible with '…'\n) | c-c++-common/cpp/dir-only-4.c -std=gnu++20, c-c++-common/cpp/dir-only-4.c -std=gnu++26, c-c++-common/cpp/dir-only-4.c -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' is not in NFC .-Werror=normalized=.) | c-c++-common/pr68833-3.c -std=gnu++20, c-c++-common/pr68833-3.c -std=gnu++26, c-c++-common/pr68833-3.c -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' is unavailable) | g++.dg/warn/deprecated-19.C -std=gnu++20, g++.dg/warn/deprecated-19.C -std=gnu++26, g++.dg/warn/deprecated-19.C -std=gnu++98 |
| 3 | accept-invalid: no error (expected: '…' must be first) | c-c++-common/pr79428-3.c -std=c++20, c-c++-common/pr79428-3.c -std=c++26, c-c++-common/pr79428-3.c -std=c++98 |
| 3 | accept-invalid: no error (expected: '…' nested depth N exceeds maximum of N \\\(use '… | c-c++-common/cpp/fmax-include-depth.c -std=gnu++20, c-c++-common/cpp/fmax-include-depth.c -std=gnu++26, c-c++-common/cpp/fmax-include-depth.c -std=gnu++98 |
| 3 | accept-invalid: no error (expected: -Wchanges-meaning) | g++.dg/warn/changes-meaning2.C -std=gnu++20, g++.dg/warn/changes-meaning2.C -std=gnu++26, g++.dg/warn/changes-meaning2.C -std=gnu++98 |

### Clusters: wrong-line, clang matches (167 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 27 | wrong-line: extra error at LN: error: expected '…' | g++.dg/cpp0x/decltype63.C -std=c++11, g++.dg/cpp0x/decltype63.C -std=c++20, g++.dg/cpp0x/decltype63.C -std=c++26 |
| 13 | wrong-line: extra error at LN: error: expected unqualified-id | g++.dg/cpp0x/decltype13.C -std=c++20, g++.dg/cpp0x/decltype13.C -std=c++26, g++.dg/cpp0x/decltype13.C -std=c++98 |
| 12 | wrong-line: missed error at LN (expected: N:expected) | g++.dg/cpp1y/pr65202.C -std=c++14, g++.dg/cpp1y/pr65202.C -std=c++20, g++.dg/cpp1y/pr65202.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: ambiguous) | g++.dg/cpp0x/variadic-alias6.C -std=c++11, g++.dg/cpp0x/variadic-alias6.C -std=c++20, g++.dg/cpp0x/variadic-alias6.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: no match) | g++.dg/lookup/operator-4.C -std=c++17, g++.dg/lookup/operator-4.C -std=c++20, g++.dg/lookup/operator-4.C -std=c++26 |
| 11 | wrong-line: missed error at LN (expected: ) | g++.dg/cpp0x/constexpr-ref2.C -std=c++26, g++.dg/init/synth2.C -std=c++98, g++.old-deja/g++.benjamin/tem05.C -std=c++98 |
| 9 | wrong-line: extra error at LN: error: expected expression | g++.dg/cpp0x/range-for23.C -std=c++11, g++.dg/cpp0x/range-for23.C -std=c++20, g++.dg/cpp0x/range-for23.C -std=c++26 |
| 9 | wrong-line: extra error at LN: error: value of type '…' is not implicitly convertible to '…' | g++.dg/template/void3.C -std=c++20, g++.dg/template/void3.C -std=c++26, g++.dg/template/void3.C -std=c++98 |
| 7 | wrong-line: extra error at LN: error: use of undeclared identifier '…' | g++.dg/cpp2a/range-for7.C -, g++.dg/parse/error61.C -std=c++11, g++.dg/parse/error61.C -std=c++20 |
| 6 | wrong-line: extra error at LN: error: default member initializer for '…' needed within definition of enclosing class '…' outside of member functions | g++.dg/ext/flexary23.C -std=gnu++14, g++.dg/ext/flexary23.C -std=gnu++20, g++.dg/ext/flexary23.C -std=gnu++26 |
| 6 | wrong-line: extra error at LN: error: kiln: not yet implemented: pack expansion in this context | g++.dg/cpp1z/using-variadic1.C -std=c++17, g++.dg/cpp1z/using-variadic1.C -std=c++20, g++.dg/cpp1z/using-variadic1.C -std=c++26 |
| 6 | wrong-line: extra error at LN: error: no member named '…' in '…' | g++.dg/cpp0x/pr58080.C -std=c++11, g++.dg/cpp0x/pr58080.C -std=c++20, g++.dg/cpp0x/pr58080.C -std=c++26 |
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
| 4 | wrong-line: extra error at LN: error: no matching function for call to '…' | g++.dg/cpp26/pack-indexing11.C -std=c++26, g++.dg/template/dependent-expr5.C -std=c++20, g++.dg/template/dependent-expr5.C -std=c++26 |
| 4 | wrong-line: missed error at LN (expected: this context) | g++.dg/parse/access2.C -std=c++20, g++.dg/parse/access2.C -std=c++26, g++.dg/parse/access2.C -std=c++98 |
| 3 | wrong-line: extra error at LN: error: called object type '…' is not a function or function pointer | g++.dg/parse/defarg17.C -std=c++20, g++.dg/parse/defarg17.C -std=c++26, g++.dg/parse/defarg17.C -std=c++98 |
| 3 | wrong-line: extra error at LN: error: cannot initialize a member subobject of type '…' (aka '…') with an rvalue of type '…' | g++.old-deja/g++.benjamin/13478.C -std=c++20, g++.old-deja/g++.benjamin/13478.C -std=c++26, g++.old-deja/g++.benjamin/13478.C -std=c++98 |
| 3 | wrong-line: extra error at LN: error: chosen constructor is explicit in copy-initialization | g++.dg/cpp2a/desig30.C -std=c++20, g++.dg/cpp2a/desig30.C -std=c++23, g++.dg/cpp2a/desig30.C -std=c++26 |
| 3 | wrong-line: extra error at LN: error: constructor inherited by '…' from base class '…' is implicitly deleted | g++.dg/cpp1z/inh-ctor24.C -std=gnu++11, g++.dg/cpp1z/inh-ctor24.C -std=gnu++20, g++.dg/cpp1z/inh-ctor24.C -std=gnu++26 |

### Clusters: wrong-line, clang differs too (543 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 133 | wrong-line: missed error at LN (expected: ) | c-c++-common/int128-types-1.c -std=gnu++20, c-c++-common/int128-types-1.c -std=gnu++26, c-c++-common/int128-types-1.c -std=gnu++98 |
| 45 | wrong-line: extra error at LN: error: unknown type name '…' | g++.dg/concepts/auto7c.C -std=c++17, g++.dg/concepts/class6.C -std=gnu++17, g++.dg/concepts/expression2.C -std=gnu++17 |
| 45 | wrong-line: missed error at LN (expected: modifying a const object) | g++.dg/cpp1y/constexpr-tracking-const1.C -std=c++14, g++.dg/cpp1y/constexpr-tracking-const1.C -std=c++20, g++.dg/cpp1y/constexpr-tracking-const1.C -std=c++26 |
| 34 | wrong-line: extra error at LN: error: no matching function for call to '…' | g++.dg/concepts/auto7.C -std=c++14, g++.dg/concepts/auto7.C -std=c++20, g++.dg/concepts/auto7.C -std=c++26 |
| 33 | wrong-line: extra error at LN: error: expected '…' | c-c++-common/toplevel-asm-5.c -std=gnu++20, c-c++-common/toplevel-asm-5.c -std=gnu++26, c-c++-common/toplevel-asm-5.c -std=gnu++98 |
| 33 | wrong-line: extra error at LN: error: use of undeclared identifier '…' | g++.dg/conversion/const3.C -std=c++20, g++.dg/conversion/const3.C -std=c++26, g++.dg/conversion/const3.C -std=c++98 |
| 28 | wrong-line: missed error at LN (expected: expected) | c-c++-common/asmgoto-2.c -std=gnu++20, c-c++-common/asmgoto-2.c -std=gnu++26, c-c++-common/asmgoto-2.c -std=gnu++98 |
| 25 | wrong-line: missed error at LN (expected: no match) | g++.dg/concepts/fn5.C -std=gnu++17, g++.dg/concepts/placeholder5.C -std=gnu++20, g++.dg/concepts/placeholder5.C -std=gnu++26 |
| 24 | wrong-line: missed error at LN (expected: jump to label) | g++.dg/cpp1z/constexpr-if4.C -std=gnu++11, g++.dg/cpp1z/constexpr-if4.C -std=gnu++20, g++.dg/cpp1z/constexpr-if4.C -std=gnu++26 |
| 23 | wrong-line: missed error at LN (expected: call to non-.constexpr. function) | g++.dg/cpp1y/constexpr-diag1.C -std=gnu++14, g++.dg/cpp1y/constexpr-diag1.C -std=gnu++20, g++.dg/cpp1y/constexpr-diag1.C -std=gnu++26 |
| 23 | wrong-line: missed error at LN (expected: not declared) | g++.dg/contracts/cpp26/contracts-nested-class1.C -std=c++23, g++.dg/contracts/cpp26/contracts-nested-class1.C -std=c++26, g++.dg/cpp0x/noexcept71.C -std=c++11 |
| 18 | wrong-line: missed error at LN (expected: uninitialized) | g++.dg/cpp26/constexpr-new6.C -std=c++26, g++.dg/init/const10.C -std=c++11, g++.dg/init/const10.C -std=c++20 |
| 16 | wrong-line: missed error at LN (expected: reference to '…' is ambiguous) | g++.dg/cpp26/name-independent-decl10.C -std=gnu++11, g++.dg/cpp26/name-independent-decl10.C -std=gnu++14, g++.dg/cpp26/name-independent-decl10.C -std=gnu++23 |
| 15 | wrong-line: extra error at LN: error: constexpr variable '…' must be initialized by a constant expression | g++.dg/cpp0x/constexpr-nullptr-2.C -std=c++11, g++.dg/cpp0x/constexpr-nullptr-2.C -std=c++20, g++.dg/cpp0x/constexpr-nullptr-2.C -std=c++26 |
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
| 9 | wrong-line: extra error at LN: error: declaration does not declare anything [-Werror,-Wmissing-declarations] | g++.dg/cpp0x/decltype7.C -std=c++20, g++.dg/cpp0x/decltype7.C -std=c++26, g++.dg/cpp0x/decltype7.C -std=c++98 |
| 9 | wrong-line: extra error at LN: error: extraneous template parameter list in template specialization or out-of-line template definition | g++.dg/cpp0x/alias-decl-42.C -std=c++11, g++.dg/cpp0x/alias-decl-42.C -std=c++20, g++.dg/cpp0x/alias-decl-42.C -std=c++26 |
| 9 | wrong-line: missed error at LN (expected: N:literal operator template .int operator\"\"_udl\\(\\). has invalid parameter l…) | g++.dg/cpp2a/udlit-class-nttp-ctad-neg2.C -std=c++20, g++.dg/cpp2a/udlit-class-nttp-ctad-neg2.C -std=c++23, g++.dg/cpp2a/udlit-class-nttp-ctad-neg2.C -std=c++26 |

### Expectation unknown (198 cases, neither pass nor failure)

kiln reported an error the test does not require, where the test file does not decide whether GCC's output holds one (spec/harness.md "The g++.dg runner"). By the directive that leaves it open:

| cases | directive | examples |
|---:|---|---|
| 33 | `dg-message ""` | c-c++-common/cpp/warning-zero-location-2.c -std=gnu++20, c-c++-common/cpp/warning-zero-location-2.c -std=gnu++26, c-c++-common/cpp/warning-zero-location-2.c -std=gnu++98 |
| 27 | `dg-excess-errors ""` | g++.dg/cpp0x/pr104668.C -std=c++11, g++.dg/cpp0x/pr104668.C -std=c++20, g++.dg/cpp0x/pr104668.C -std=c++26 |
| 9 | `dg-excess-errors "The error is sent to the SARIF file, rat…"` | c-c++-common/diagnostic-format-sarif-file-3.c -std=gnu++20, c-c++-common/diagnostic-format-sarif-file-3.c -std=gnu++26, c-c++-common/diagnostic-format-sarif-file-3.c -std=gnu++98 |
| 6 | `dg-message "an attribute that appertains to a type-s…"` | g++.dg/cpp0x/gen-attrs-85.C -std=c++11, g++.dg/cpp0x/gen-attrs-85.C -std=c++20, g++.dg/cpp0x/gen-attrs-85.C -std=c++26 |
| 6 | `dg-message "pass"` | g++.dg/overload/ellipsis2.C -std=gnu++20, g++.dg/overload/ellipsis2.C -std=gnu++26, g++.dg/overload/ellipsis2.C -std=gnu++98 |
| 6 | `dg-prune-output "In function"` | g++.dg/diagnostic/bad-binary-ops-highlight-colors.C -std=gnu++20, g++.dg/diagnostic/bad-binary-ops-highlight-colors.C -std=gnu++26, g++.dg/diagnostic/bad-binary-ops-highlight-colors.C -std=gnu++98 |
| 6 | `dg-prune-output "error"` | g++.dg/cpp0x/fntmpdefarg5.C -std=c++11, g++.dg/cpp0x/fntmpdefarg5.C -std=c++20, g++.dg/cpp0x/fntmpdefarg5.C -std=c++26 |
| 6 | `dg-prune-output "specifies less restrictive attribute"` | c-c++-common/builtin-has-attribute-3.c -std=gnu++20, c-c++-common/builtin-has-attribute-3.c -std=gnu++26, c-c++-common/builtin-has-attribute-3.c -std=gnu++98 |
| 5 | `dg-message "computed goto"` | g++.dg/ext/label15.C -std=gnu++20, g++.dg/ext/label15.C -std=gnu++26, g++.dg/ext/label15.C -std=gnu++98 |
| 3 | `dg-excess-errors "expected to be full of errors, but not a…"` | g++.dg/wrappers/cp-stdlib.C -std=c++20, g++.dg/wrappers/cp-stdlib.C -std=c++26, g++.dg/wrappers/cp-stdlib.C -std=c++98 |
| 3 | `dg-excess-errors "incomplete type\|not a member"` | g++.dg/cpp0x/noexcept15.C -std=c++11, g++.dg/cpp0x/noexcept15.C -std=c++20, g++.dg/cpp0x/noexcept15.C -std=c++26 |
| 3 | `dg-message "35:non-standard-layout"` | g++.dg/other/offsetof3.C -std=c++20, g++.dg/other/offsetof3.C -std=c++26, g++.dg/other/offsetof3.C -std=c++98 |
| 3 | `dg-message "ambiguous\|declared here"` | g++.dg/parse/crash22.C -std=c++20, g++.dg/parse/crash22.C -std=c++26, g++.dg/parse/crash22.C -std=c++98 |
| 3 | `dg-message "an attribute that appertains to a friend…"` | g++.dg/warn/Wno-attributes-1.C -std=gnu++11, g++.dg/warn/Wno-attributes-1.C -std=gnu++20, g++.dg/warn/Wno-attributes-1.C -std=gnu++26 |
| 3 | `dg-message "copy"` | g++.dg/overload/ellipsis1.C -std=gnu++20, g++.dg/overload/ellipsis1.C -std=gnu++26, g++.dg/overload/ellipsis1.C -std=gnu++98 |
| 3 | `dg-message "declar"` | g++.dg/template/error43.C -std=c++20, g++.dg/template/error43.C -std=c++26, g++.dg/template/error43.C -std=c++98 |
| 3 | `dg-message "expected"` | c-c++-common/pr92833-4.c -std=c++20, c-c++-common/pr92833-4.c -std=c++26, c-c++-common/pr92833-4.c -std=c++98 |
| 3 | `dg-message "foo"` | g++.dg/template/lookup9.C -std=gnu++20, g++.dg/template/lookup9.C -std=gnu++26, g++.dg/template/lookup9.C -std=gnu++98 |
| 3 | `dg-message "nonexistent.h"` | c-c++-common/missing-header-5.c -std=gnu++20, c-c++-common/missing-header-5.c -std=gnu++26, c-c++-common/missing-header-5.c -std=gnu++98 |
| 3 | `dg-message "not first declaration"` | g++.dg/cpp0x/deleted15.C -std=gnu++11, g++.dg/cpp0x/deleted15.C -std=gnu++20, g++.dg/cpp0x/deleted15.C -std=gnu++26 |
| 3 | `dg-message "offsetof"` | g++.dg/other/offsetof5.C -std=c++20, g++.dg/other/offsetof5.C -std=c++26, g++.dg/other/offsetof5.C -std=c++98 |
| 3 | `dg-message "previous declaration"` | g++.dg/cpp0x/Wattributes1.C -std=c++11, g++.dg/cpp0x/Wattributes1.C -std=c++20, g++.dg/cpp0x/Wattributes1.C -std=c++26 |
| 3 | `dg-message "synthesized\|deleted"` | g++.old-deja/g++.pt/crash20.C -std=c++20, g++.old-deja/g++.pt/crash20.C -std=c++26, g++.old-deja/g++.pt/crash20.C -std=c++98 |
| 3 | `dg-message "three-way comparison of vector"` | g++.dg/cpp2a/spaceship-vec1.C -std=c++20, g++.dg/cpp2a/spaceship-vec1.C -std=c++23, g++.dg/cpp2a/spaceship-vec1.C -std=c++26 |
| 3 | `dg-message "variable-length array of .auto."` | g++.dg/cpp23/auto-array4.C -std=gnu++11, g++.dg/cpp23/auto-array4.C -std=gnu++20, g++.dg/cpp23/auto-array4.C -std=gnu++26 |
| 3 | `dg-message "will be removed"` | g++.dg/ext/deprecate-2.C -std=gnu++20, g++.dg/ext/deprecate-2.C -std=gnu++26, g++.dg/ext/deprecate-2.C -std=gnu++98 |
| 3 | `dg-prune-output "-Wdangling-pointer"` | g++.dg/warn/ref-temp1.C -std=gnu++20, g++.dg/warn/ref-temp1.C -std=gnu++26, g++.dg/warn/ref-temp1.C -std=gnu++98 |
| 3 | `dg-prune-output "\[^\n\r\]*first argument to .va_arg. not…"` | c-c++-common/pr70651.c -std=c++20, c-c++-common/pr70651.c -std=c++26, c-c++-common/pr70651.c -std=c++98 |
| 3 | `dg-prune-output "declared but never defined"` | c-c++-common/attributes-3.c -std=c++20, c-c++-common/attributes-3.c -std=c++26, c-c++-common/attributes-3.c -std=c++98 |
| 3 | `dg-prune-output "diagnostic/function-color1.C"` | g++.dg/diagnostic/function-color1.C -std=gnu++20, g++.dg/diagnostic/function-color1.C -std=gnu++26, g++.dg/diagnostic/function-color1.C -std=gnu++98 |

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

- kiln: `--c++17`, `--embed-directory <path>`, `--embed-directory=<path>`, `--param case-values-threshold=1`, `--param case-values-threshold=4`, `--param cxx-max-namespaces-for-diagnostic-help=5`, `--param early-inlining-insns=0`, `--param early-inlining-insns=1`, `--param early-inlining-insns=100`, `--param early-inlining-insns=14`, `--param early-inlining-insns=3`, `--param ggc-min-expand=0`, `--param ggc-min-heapsize=0`, `--param ggc-min-heapsize=1024`, `--param hardcfr-max-blocks=9`, `--param hardcfr-max-inline-blocks=5`, `--param hash-table-verification-limit=10000`, `--param ipa-cp-eval-threshold=1`, `--param ipa-cp-eval-threshold=100`, `--param ipa-cp-eval-threshold=125`, `--param large-function-growth=60`, `--param large-function-insns=10`, `--param logical-op-non-short-circuit=0`, `--param logical-op-non-short-circuit=1`, `--param max-cse-insns=0`, `--param max-devirt-targets=1`, `--param max-early-inliner-iterations=1`, `--param max-early-inliner-iterations=10`, `--param max-early-inliner-iterations=3`, `--param max-early-inliner-iterations=5`, `--param max-inline-insns-single=200`, `--param max-inline-insns-single=4`, `--param max-inline-insns-size=1`, `--param max-unroll-times=32`, `--param max-unswitch-insns=5`, `--param uninlined-thunk-insns=1000`, `--param unroll-jam-min-percent=0`, `--param vect-epilogues-nomask=1`, `--param=early-inlining-insns=111`, `--param=early-inlining-insns=176`, `--param=early-inlining-insns=192`, `--param=early-inlining-insns=196`, `--param=early-inlining-insns=82`, `--param=hash-table-verification-limit=1000`, `--param=hash-table-verification-limit=10000`, `--param=hash-table-verification-limit=10000000`, `--param=ipa-cp-eval-threshold=0`, `--param=ipa-cp-eval-threshold=1`, `--param=ipa-cp-max-recursive-depth=94`, `--param=large-stack-frame-growth=1`, `--param=logical-op-non-short-circuit=0`, `--param=max-cse-insns=0`, `--param=max-inline-insns-single=25`, `--param=max-stores-to-track=2`, `--param=modref-max-depth=1`, `--param=partial-inlining-entry-probability=100`, `--param=sccvn-max-alias-queries-per-access=0`, `-C`, `-H`, `-MQ` and 379 more
- gcc: `--c++17`, `-Wno-attributes=___::`, `-Wno-attributes=____::____`, `-Wno-attributes=c2::@tr`, `-Wno-attributes=c::____`, `-Wno-attributes=c@::attr`, `-fcf-protection`, `-fcf-protection=branch`, `-fcf-protection=full`, `-fcf-protection=return`, `-flifetime-dse=123456`, `-fpatchable-function-entry=1,65536`, `-fpatchable-function-entry=65536,1`, `-include <path>`
- clang: `--c++17`, `--embed-directory <path>`, `-fabi-version=0`, `-fabi-version=14`, `-fabi-version=15`, `-fabi-version=16`, `-fabi-version=18`, `-fabi-version=2`, `-fabi-version=20`, `-fabi-version=3`, `-fcf-protection`, `-fcf-protection=branch`, `-fcf-protection=full`, `-fcf-protection=return`, `-fchecking`, `-fchecking=2`, `-fcompare-debug`, `-fconcepts`, `-fconcepts-diagnostics-depth=2`, `-fconcepts-diagnostics-depth=3`, `-fconserve-stack`, `-fconstexpr-fp-except`, `-fconstexpr-loop-limit=5`, `-fcontract-evaluation-semantic=enforce`, `-fcontract-evaluation-semantic=observe`, `-fcontracts`, `-fdiagnostics-format=sarif-file`, `-fdiagnostics-set-output=text:show-nesting=yes,show-nesting-locations=no`, `-fdiagnostics-show-caret`, `-fdump-ada-spec`, `-fdump-ipa-sra`, `-fdump-ipa-whole-program`, `-fdump-tree-dse1-details`, `-fdump-tree-eh`, `-fdump-tree-hardcfr`, `-fdump-tree-optimized`, `-fext-numeric-literals`, `-ffreestanding`, `-fhardcfr-check-noreturn-calls=always`, `-fhardcfr-check-noreturn-calls=never`, `-fhardcfr-check-noreturn-calls=no-xthrow`, `-fhardcfr-check-noreturn-calls=nothrow`, `-fharden-control-flow-redundancy`, `-fhardened`, `-fimplicit-constexpr`, `-fipa-icf`, `-fira-algorithm=priority`, `-flifetime-dse`, `-flifetime-dse=1`, `-flifetime-dse=123456`, `-fmax-include-depth=1`, `-fnew-inheriting-ctors`, `-fnew-ttp-matching`, `-fno-checking`, `-fno-delete-dead-exceptions`, `-fno-diagnostics-show-highlight-colors`, `-fno-ext-numeric-literals`, `-fno-hardcfr-check-returning-calls`, `-fno-immediate-escalation`, `-fno-implicit-constexpr` and 12 more

<!-- gxx-dg:end -->
