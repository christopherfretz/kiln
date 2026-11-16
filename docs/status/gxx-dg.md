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

## Triage (2026-11-15, kiln `604edce44`, aarch64)

Hand-written from the run whose generated region follows. Each cluster
gives the case count (tests in parentheses), a few lines of my own that
show the pattern, and the part of kiln the fix probably lives in. A "case"
is one test at one standard (or one optimization level).

### Crash / hang (20 cases, all clusters)

WP-1456 fixed all six clusters below (re-run of the 20 cases: 17 pass, 3 freestanding `main` cases are rejected by kiln and clang alike because the runner drops `-ffreestanding`); the counts on this page predate it.

1. **Out of memory on a huge or recursive template** (6 cases, 2 tests).
   Expected a diagnostic, got the 2 GiB cap.
   `template<int N> struct A { int a[N][N][N][N][N][N][N][N][N][N]; }; A<66000> x;`
   expects "exceeds maximum object size". The other test is a member-template
   pointer-to-member used in a variable template. Area: sema type
   completion (array size overflow check before layout); template
   instantiation.
2. **LLVM crash** (5 cases, 2 tests). `asm("" : "=r"(big_struct))`, where
   the struct has 64 ints, is passed to LLVM instead of being rejected
   ("impossible constraint"). The other test converts `_Float128` and
   `__int128` extremes (`T(F(f))` in a `[[gnu::noipa]]` template). Area:
   sema inline-asm constraint checking; codegen for fp-to-int128
   conversion.
3. **Compile timeout: recursive concept satisfaction** (3 cases).
   `template<int N, class T> concept C = requires(T t) { f<N+1>(t); };`
   with `f` constrained on `C<N+1, T>` should stop at the instantiation
   depth limit. Area: sema constraint satisfaction (depth limit).
4. **SIGABRT on `this` in an NSDMI of a constexpr temporary** (3 cases).
   `struct X { const X* p = this; }; constexpr const X& r = X{};` Area:
   Oven (the address of a lifetime-extended temporary in a constant
   initializer).
5. **ICE `const_int on non-integer type ptr`** (3 cases).
   `const char *main() {}` under `-ffreestanding`. kiln reports "'main'
   must return 'int'" and then crashes in lowering. Area: IR lowering of
   `main`'s implicit `return 0` when the return type is not int.

The `compile timeout` of `g++.dg/other/pr117516.C` (deeply nested
structs, 1 case) is gone in the second run; it is on the 60 s edge.

### Wrong-code (272 cases, 107 tests)

WP-1460 fixed cluster 4 (`expr/cast4.C`, `init/ctor2.C`) and most of
cluster 9 (`bad_array_new1.C`, `bad_array_new2.C`, sized deallocation
before C++14 in `sized-dealloc1.C`); `abi/vbase13.C`, `init/new40.C`,
`aligned-new9.C` and `g++.jason/new3.C` remain (GCC layout and C++03
limit, `-faligned-new=N`, `-fcheck-new`).

WP-1484 fixed `eh/init-temp1.C` (elidable copies of a prvalue are elided, and a virtual base is destroyed when a temporary in its own initializer throws) and added `-f[no-]elide-constructors` from C++17; `init/copy3.C` (C++14 and earlier) remains.

WP-1457 fixed cluster 1 (dynamic exception specifications filter at run time in C++98 to C++14; C++98 destructors and class-specific `operator delete` may throw): its 14 tests went from 23 wrong cases to 0.

Clang fails 58 of these tests too. Many of those depend on GCC-specific
semantics (object-size builtins, `-flifetime-dse`, ICF addresses,
`__PRETTY_FUNCTION__` text). The clusters below come first from the 38
tests that clang passes, then from GCC's own behaviour.

WP-1458 fixed cluster 2 (its 42 tests: 11 → 36 of 106 cases pass; the C++98 runs wait for cluster 1, `init-temp1.C` is an overload-resolution bug).
WP-1461 fixed clusters 5 to 8 below except `__PRETTY_FUNCTION__` text: 21 of their 22 cases pass now (`init/copy3.C` needs `-fno-elide-constructors`, which kiln does not have).

WP-1459 fixed cluster 3, temporary lifetime (29 tests: 30 → 64 of 73 run cases pass; the rest are other bugs: `decomp6.C` and `paren-init7.C` pick an explicit copy constructor, `cast4.C` a cast's base offset, `ctor2.C` the vtable during base construction, `copy3.C` `-fno-elide-constructors`).

1. **Exception specifications and `std::unexpected`, C++98 to C++14**
   (about 20 cases, 9 tests). `void f() throw(int) { throw 'c'; }` must call
   the unexpected handler, which may rethrow `std::bad_exception`.
   Also: in C++98 a destructor is not implicitly `noexcept`, so
   `struct B { ~B() { throw 1; } };` must let the exception reach the
   caller (kiln calls `terminate`). Area: EH codegen (dynamic exception
   specification filters); sema (implicit exception spec by standard).
2. **Cleanups while an exception is in flight** (about 40 cases, 14
   tests). A destructor that throws (`noexcept(false)`) during aggregate
   init, array new, a temporary bound to a reference, or a statement
   expression must still destroy what was already constructed, in order.
   For example `struct S { ~S() noexcept(false) { throw 1; } }; A a[2] = { A(), (throw 0, A()) };`
   must destroy `a[0]` once. Area: codegen EH cleanup scopes (partial
   aggregate and array cleanup, temporaries' lifetime-extended cleanups).
3. **Temporary lifetime** (about 30 cases, 12 tests).
   `const B& b = A(1).*(&A::b);` extends the whole `A` (no early `~A`);
   the same through `?:` with array temporaries; range-for temporaries
   in C++23 (P2718); structured-binding temporaries (CWG2867). Area: sema
   lifetime extension (pointer-to-member and conditional operands);
   codegen temporary cleanups.
4. **Pointer conversions with an offset** (about 10 cases, 3 tests). A
   C-style cast `(Base*)constDerivedPtr`, where the Derived has a vptr and
   Base does not, must add the base offset (it is a static_cast plus a
   const_cast, not a reinterpret_cast). Converting `this` to a virtual base
   inside a constructor called from a more-derived class must use the
   construction vtable. Area: sema C-style cast decomposition; codegen
   VTT/construction vtables.
5. **Enumeration underlying type** (3 cases).
   `enum C { c1 = -1, c2 = 0x80000000 };` needs `sizeof(C) == 8` (no int
   holds both values). Area: sema enum underlying type selection.
6. **Template partial-specialization choice** (about 10 cases, 3 tests).
   A member partial specialization `template<bool B> struct inner<compare, B>`
   is not chosen. `X<T::template B, T>` does not match a template template
   argument. A constrained explicit specialization is skipped. Area: sema
   partial ordering and deduction of template template arguments.
7. **Mangling and RTTI names** (about 15 cases, 6 tests).
   `typeid(S volatile[3][4]).name()` must be `A3_A4_1S` (the cv-qualifiers
   go on the element). `__PRETTY_FUNCTION__` text differs from GCC's
   `X<T>::X() [with T = void]` (clang differs too). Area: mangler (array
   cv); the pretty-function printer (`-fclang-compat` vs GCC format).
8. **Array copy and initialization counts** (about 10 cases, 4 tests).
   `S s[1] = { S() };` must copy once in C++14 (no guaranteed elision
   before C++17). `auto [a, b] = arr;` must copy each element with the
   right constructor (explicit copy constructor vs converting template).
   Area: sema initialization (array element copy-init).
9. **Allocation** (about 10 cases, 4 tests). `new int[-1]` must throw
   `std::bad_array_new_length` (clang does not either). kiln does not have
   `-fcheck-new` (a class `operator new` that returns null must skip the
   constructor) or `-faligned-new=N` (it drops them, so the test runs
   without them). One sized-deallocation test fails at C++98 only (not
   reduced). Area: codegen new-expression (size overflow); driver flags.
10. **GCC extensions that clang also gets wrong** (about 90 cases):
    `__builtin_object_size` and `__builtin_dynamic_object_size` values,
    `__attribute__((cleanup))` with exceptions, `-flifetime-dse` padding,
    identical-code-folding address identity, `__builtin_clear_padding`,
    labels-as-values. Area: per builtin; low priority.

### Rejects-valid where clang accepts (997 cases, top 30 clusters)

WP-1468 fixed cluster 7 (an instantiated variable template binding its own temporary, a dependent array bound taken from the in-class declaration, `this` in a union member's default initializer, block-scope `extern` in a constexpr function (CWG2552), value-initialized vectors): its 7 tests went from 3 to 18 of 18 cases; cluster 29 already passed on main except four clang-matching wrong-line tests; the `g++.dg/cpp23` math builtins the Oven evaluates exactly no longer make a function "never constant" (`attr-assume5`/`6` pass; `attr-assume1`/`3` stop at the C++23-extension warning, as with clang).

WP-1465 fixed the deduction parts of clusters 1, 5, 11 and 23 (CWG2303 most-derived base, CTAD implicit-guide access, CWG2918 constraints in address selection, `auto` in array declarators, a type deduced from an array bound): re-run of their 46 tests, 13 → 45 of 133 cases pass; the rest are placement `new(0)`, C++98 null pointer constants, string-literal conversions and aggregate CTAD of member templates.

WP-1466 fixed the parser clusters 2, 3, 8, 9, 21, 22 and 30 (re-run of their 61 tests: 6 → 103 of 177 cases pass; most of the rest are access declarations in C++11 and later, which clang rejects too, compound literals that define an anonymous union, which clang rejects too, and six reduced repros left as `xfail(kiln): WP-1495`, two of them outside the parser: `#pragma GCC diagnostic ignored "-Wpedantic"` and access checks in partial specialization matching).

WP-1495 fixed the remaining parser and friend repros of those 61 tests (implicit `typename` before `operator`, out-of-line constructors of class templates with member-typedef parameters, `class N::template X<T>`, GNU `static`/`inline template`, friend no-match notes, dependent-qualifier friend functions) and four small rules (default base access from the defining class-key, C++11 keywords as identifiers in C++98, C++98 access declarations with template-id qualifiers, `-C` only with `-E`): 112 → 132 of 177 cases pass; of the 45 left, 40 are rejected by clang too and 5 (`template/access40*.C`) are access checks in partial specialization matching.

WP-1467 (after this run) fixed most of clusters 4, 6, 12, 13, 14, 18, 26, 27 and 28: 108 of their 167 cases now pass; left are SFINAE access and deletion checks, pre-C++17 elidable copies in `noexcept`, `V a{{}}` with an initializer-list constructor, explicit-instantiation ordering and two-phase operator lookup.

WP-1496 (after this run) fixed those: 115 → 159 of the same 168 cases pass; left are a local class's friend declared in the enclosing namespace (`friend2`), C++98 null pointer constants from non-literal integral constant expressions (`self1`, `Wnull-conversion-1`, `null3`) and taking the address of a member template's `operator()` in a requires-expression (`concepts-pr67825`).

WP-1472 added GCC's `__builtin_is_corresponding_member` / `__builtin_is_pointer_interconvertible_with_class` and fixed clusters 16 and 25 (`g++.dg/cpp2a` + `g++.dg/cpp23`: 3267 → 3296 of 4371 cases pass); clusters 19 and 24 and `lambda-scope10` have xfail repros (row WP-1497).

WP-1497 fixed clusters 19 and 24, `lambda-scope10`/`10a` and most of the concepts and single-rule rejects-valid cases (class NTTP lambda arguments in alias and member templates, eager instantiation in `decltype` and qualified calls, init-captures in requires-clauses, `typename T::type()` requirements, unions in defaulted comparisons, `static_cast` to an array, `source_location::current()` in libstdc++'s form): `g++.dg/cpp2a` + `g++.dg/cpp23` 3379 → 3433 of 4371 cases pass (on main after WP-1472 and later rows); the rest are CTAD, CWG2369, and concepts cases with pointers to member functions, out-of-line members of constrained partial specializations, template template parameters, using-declarations hidden by constrained members and a value-dependent member template call.

WP-1469 fixed the variable length array clusters 10, 17 and 20 and the "variable-sized object may not be initialized" row (re-run of their 29 tests: 0 → 45 of 87 cases pass, every case clang 22 accepts; the other 42 clang rejects too or puts on the same wrong line): VLA bounds captured by lambdas, explicit casts to variably modified types, range-based `for` over a VLA, VLAs of class objects, an initialized VLA whose bound folds, `-Wvla-cxx-extension` as an ExtWarn under `-pedantic-errors`, and `-Wpedantic` covering the off-by-default extensions (flexible array members under `-Wno-error=pedantic`).

WP-1499 added VLAs of arrays of class objects, lambda VLA bounds traced from the type (`typeof`, casts, locals), C's `-Wgnu-folding-constant` and the [stmt.expand] iterator and capture checks: the 87 VLA cases stay at 45 (the other 42 are VLA initializers and folded bounds clang 22 rejects too), and `g++.dg/cpp26/expansion-stmt*` goes from 66 to 69 of 107 cases (`expansion-stmt22` passes; no case went from pass to fail).

WP-1493 (after this run) fixed the rest of the deduction and overload rows those re-runs left: placement `new (0) X` through overload resolution, a template-id overload set as a call argument, C++98 null pointer constants and their warnings, aggregate CTAD for member templates of specializations and for nested braced arrays (with `#embed`), alias CTAD with a defaulted parameter, inherited-constructor guides through a typedef base, a defaulted `operator=`'s exception specification in address selection, and the access of a befriended function template's signature. Re-run of their 46 tests: 48 → 111 of 133 cases pass (11 → 36 tests); left are `variadic185`, `nsdmi-aggr12`, `two-stage4` (xfail repro: a non-dependent operator re-resolved at instantiation), `lambda-scope10`/`10a`, `evaluated1b`, `implicit-func-cast`, `constexpr-52892-1`, `fn9` (clang rejects too) and `class-deduction84` at C++17 (clang's extension warning where GCC errors).

1. **Deduction from a base class that is ambiguous** (60 cases, 22 tests).
   `struct C : A<int,int>, A<int> {}; template<class... T> void f(const A<T...>&); f(C{});`
   deduces `A<int,int>` (CWG2303: the most derived candidate wins). The
   cluster also holds `noexcept` and `-Wwrite-strings` overload sets. Area:
   sema template argument deduction (derived-to-base candidates).
2. **`X<T>()` as a constructor declarator** (39 cases, 23 tests).
   `template<class T> struct X { X<T>(); };` is a C++20 warning, not an
   error (CWG2237). Area: parser (declarator-id with template-id).
3. **`expected unqualified-id` in GNU syntax** (34 cases, 14 tests):
   `__attribute__((fallthrough));` in odd places, abi_tag attributes on
   namespaces, `X<T>::~X<T>()`. Area: parser (GNU attributes).
4. **Trait results** (32 cases, 12 tests): `is_trivially_constructible<B>`
   for `struct B : A { using A::A; B(int); };` must be true (an inherited
   default constructor does not suppress triviality). `noexcept` of NSDMIs
   and list-init. Area: sema type traits; implicit special members.
5. **CTAD with a protected alias in the constructor** (31 cases, 11
   tests). `template<class T> struct C : Base<T> { C(T, typename Base<T>::type); }; C c(1, 1);`
   Access is checked in the context of the class, not of the guide. Also
   `#embed` inside a nested braced list for CTAD. Area: sema CTAD (access
   context of implicit guides).
6. **`char* p = "lit";`** (25 cases, 9 tests). This is a warning in GCC,
   and in clang under `-fclang-compat`. kiln makes it a hard error. Area:
   sema string-literal conversion (deprecated-conversion extension).
7. **Constant initializers through `const` pointers** (19 cases, 7
   tests). `constexpr int const* const* q = &p; constexpr int const* r = *q;`
   Also constexpr union member access and delegating constructors. Area:
   Oven (reads through constexpr pointer chains).
8. **`friend X::X(T::t);`** (18 cases, 6 tests): "constructors is not
   supported yet". Area: parser/sema (friend constructor declarations).
9. **`__extension__ char a[0];` under `-pedantic-errors`** (17 cases):
   `__extension__` must silence the pedantic diagnostic. Area: parser
   (`__extension__` scope).
10. **Variably modified types in C++ (GNU)** (24 cases, 8 tests): VLA
    bounds in lambdas and `sizeof`, range-for over a VLA ("not supported
    yet", "bound not emitted"). Area: codegen VLA.
11. **`auto (*p)[1] = &v;`** (15 cases, 7 tests): deducing `auto` inside
    a declarator, and `auto x[2] = {1, 2}` (C++23 GNU). Area: sema auto
    deduction through declarators.
12. **`-fno-access-control`** (15 cases, 5 tests): the flag is not
    implemented (kiln drops it). Area: driver plus sema access checking.
13. **C-style cast between pointer-to-member types** (27 cases, 9
    tests): `(int A::*)(long A::*)0` and pointer-to-member-function casts
    are reinterpret_casts and must be allowed. Area: sema casts.
14. **`struct B<T>::C : B` and dependent bases** (15 cases, 5 tests): the
    injected-class-name of the enclosing template used as a base. Area:
    sema name lookup in base-specifiers.
15. **`vector_size` attribute on a friend's return type** (12 cases, 4
    tests). `friend __attribute__((vector_size(16))) float f();` and then
    the definition clash ("differ only in return type"). Area: sema
    attribute placement (GNU attribute applies to the type).
16. **`[outer]() { outer.f<int>(); }`** (9 cases): a member template
    called through a capture of the current instantiation. Area: sema
    lambda capture types in templates.
17. **Range-for over a VLA** (9 cases): see 10.
18. **`using B::operator=;` plus implicit `operator=`** (9 cases): the
    using-declaration must be hidden by the implicit one. Area: sema
    overload (using-declarations vs implicit members).
19. **Lambda in a template argument** (9 cases):
    `v<X{1}>(42).n` where `v` returns a lambda-dependent type. Area: sema
    NTTP class objects.
20. **Flexible array members in C++** (9 cases): mangling and
    substitution of `T[]` members under `-pedantic-errors` with
    `__extension__`. Area: sema extension diagnostics.
21. **Attributes mixed in a statement** (9 cases):
    `__attribute__(()) [[]] __attribute__((assume(true))) [[]];` Area: parser
    attribute sequences.
22. **`d.template D<T>::template B<T>::foo()`** (9 cases): a qualified
    member template name after `.template`. Area: parser/sema
    nested-name-specifier lookup.
23. **`auto x = &X<true>::f;` selecting by constraints** (8 cases,
    CWG2918). Area: sema overload set address with constraints.
24. **`T::nonexistent` in an uninstantiated default** (8 cases): a
    diagnostic too eager in an unused member or a constraint. Area: sema
    (eager checking of dependent members).
25. **`decltype((self))` in an explicit-object lambda** (8 cases): the
    constness of `self` for a `[*this]` capture. Area: sema deducing-this
    lambdas.
26. **IR lowering `BadCall`** (8 cases, 3 tests): a placement
    `operator delete` called with a different argument count than
    declared. `struct A { void* operator new(size_t, int); void operator delete(void*, int); };`
    Area: IR lowering of new-expression cleanup.
27. **`register` in C++17 under GCC's flags** (7 cases): kiln makes the
    `-Wregister` warning an error where GCC only warns. Area: diagnostics
    (default severity).
28. **Explicit instantiation by conversion-function name**
    (7 cases): `template void g<A>(S<&A::operator int>);`. Area: sema
    explicit instantiation matching.
29. **`static_assert` on a constexpr virtual call or `dynamic_cast`**
    (7 cases): Oven dynamic type during construction. Area: Oven.
30. **Anonymous structs and unions in templates** (`'(unnamed union at
    ...)' cannot be defined in a type specifier`, 6 cases). Area: parser.

### Accepts-invalid where clang rejects (922 cases, top 30 clusters)

WP-1468 fixed clusters 1, 12 and 18: a const member, a const subobject reached through a base or union, a const temporary and `new const T` are now const in the Oven (the 8 cluster-1 tests report the modification in a note at GCC's line, under an error at the use, as clang does), an aggregate reading a later element of itself is an error (`constexpr-array12` passes; `stmtexpr21` is accepted by clang too), and a constexpr local's own address in its value is rejected before C++26 (6 of 9 cluster-18 cases; at C++26 P2686R5 makes it valid, which kiln implements and GCC and clang do not yet).

WP-1494 (follow-ups of WP-1468) made the Oven keep a union's destroyed member as ended ("outside its lifetime"), tag the address of a union member that shares its type and offset with another so reads, placement new and destruction through it act on that member, make single-object placement into an element of an inactive array member end the old member, word reads of not-yet-initialized subobjects as clang ("outside its lifetime"), and reject a virtual call, `typeid` or `dynamic_cast` on a sibling base during construction or destruction ([class.cdtor]/4-6); re-run of clusters 7, 29, 1, 12, 18 and the `constexpr-union*`, `constexpr-new*` and `construct_at*` tests of `g++.dg/cpp2a` and `cpp26` (299 cases): 191 pass before and after, no case changed class (the runner matches lines, not wording).

WP-1470 fixed clusters 2 to 10 (an uninitialized reference member in aggregate init, `= default` with a mismatched signature (an error before C++20), an explicit specialization of a member whose constraints fail or of a constrained variable template, ambiguous class-scope `operator new`, access of the copy constructor for a vararg and of an elidable C++14 constructor, protected `&A::m`, explicit-object redeclarations, default arguments added to an out-of-line member template, a class incomplete inside instantiations it triggers): re-run of their 39 tests, 22 → 86 of 113 cases pass; the rest are pack constraint subsumption, an ambiguous conversion in direct-init, using-declaration hiding by requires-clause, P3074 unions, and cases clang also accepts.

The largest group (130 cases, 61 tests) expects any error at all
(`dg-error ""`). It is mixed: mostly old-deja programs that are invalid
in many small ways (missing `typename`, a bad friend, a nested class in an
anonymous union). The named clusters, by the expected message:

1. **Modifying a const object in constant evaluation** (24 cases, 8
   tests). `constexpr int f() { const A a{}; const_cast<A&>(a).x = 1; return 0; }`
   used in a constant expression. Area: Oven (const-object tracking).
2. **Explicit instantiation or specialization that matches nothing**
   (12 cases). `template void S<int>::g();` when `g` is constrained away.
   Area: sema explicit instantiation.
3. **Ambiguity not reported** (10 cases): overloads equal after
   constraints or conversions. Area: sema overload ranking.
4. **`= default` with a mismatched signature** (10 cases):
   `constexpr derived& operator=(const derived&) = default;` where the
   implicit one is not constexpr (C++11). Area: sema defaulted functions.
5. **Uninitialized reference member in aggregate init** (9 cases):
   `struct A { int& r; }; A a = {};`. Area: sema aggregate init.
6. **Access in an implicitly called function** (9 cases): copying a type
   with a private copy constructor through `f(a)`. Area: sema access
   checks for implicit calls.
7. **Incomplete type in a pack expansion** (9 cases):
   `T t(create<Args>()...)` with an incomplete `Args`. Area: sema
   completeness checks in instantiation.
8. **Default arguments on an out-of-line member template** (9 cases):
   `template<class U> C<int>::C(const C<U>&, bool = false) {}`. Area: sema
   redeclaration rules for default arguments.
9. **Deleted functions not diagnosed** (8 cases): a union with a
   non-trivial member and no constructor. Area: sema implicit deletion.
10. **Explicit-object redeclarations** (8 cases):
    `void f(); void f(this S&&);` cannot overload. Area: sema overloading
    (deducing this).
11. **Pack expansion into a non-pack alias parameter** (6 cases). Area:
    sema alias templates.
12. **Self-referencing constexpr aggregate** (6 cases):
    `constexpr A a = { a.ar[0] };`. Area: Oven (reads of an object under
    construction).
13. **Defaulted copy that is implicitly deleted** (6 cases, C++17 and
    earlier). Area: sema defaulted functions.
14. **Access of a private enumerator** (6 cases): `C::y` where `y` is a
    private scoped enumerator. Area: sema access for enumerators.
15. **Opaque enum redeclared with different access** (6 cases). Area: sema.
16. **`int&& j{{ i }};`** (6 cases): too many braces for a reference.
    Area: sema list-init of references.
17. **Explicit specialization redefinition through `= delete`** (6
    cases). Area: sema redeclarations.
18. **Constexpr address of a local** (6 cases):
    `constexpr A a = foo();` with a member pointing into `foo`'s frame.
    Area: Oven (escaping pointers into a dead frame).
19. **Lambda templates before C++20 under `-pedantic-errors`** (6 cases):
    `[]<class T>() {}` in C++14. Area: parser (pedantic extension
    diagnostics).
20. **`template<class T = void, typename T::type M>`** (6 cases): a
    `void` default used in a dependent type. Area: sema default template
    arguments.
21. **`static inline same_as<T> auto v = 0;`** (6 cases): the placeholder
    constraint is not checked. Area: sema constrained auto.
22. **`{ t } -> bool;`** (6 cases): a return-type requirement that is not
    a type-constraint. Area: parser (requires-expression).
23. **Unresolved operator in a generic lambda** (6 cases):
    `[](T x) { return !x; }(x)` with no `operator!`. Area: sema
    instantiation of lambda bodies.
24. **`} A::B;` typedef-name in an elaborated declarator** (6 cases).
    Area: parser.
25. **Template template mismatch** (6 cases): `A<A<TP>::Template>`.
    Area: sema template argument checking.
26. **A local redeclaring a parameter** (6 cases):
    `void f(int bar) { int bar = 0; }`. Area: sema scope rules.
27. **Default argument on a member of a class template partial
    specialization** (6 cases). Area: sema.
28. **Destructor template** (6 cases): `template<int i> ~Foo() {}`. Area:
    parser.
29. **Throwing a non-copyable rvalue before C++17** (6 cases):
    `throw A();` where `A`'s copy constructor is not accessible. Area: sema
    (copy-init check before guaranteed elision).
30. **Pedantic C++ extensions that kiln allows** (the remaining small
    clusters, for example `__int128` and zero-length arrays without
    `__extension__`). Area: diagnostics.

WP-1471 fixed clusters 11, 14 to 17, 19 to 22, 24 to 26 and 28 (its subset of 37 tests and 111 cases went from 9 to 81 cases passing). Still open: clusters 13 and 27 (wait for WP-1470), cluster 23 (an operator found only at instantiation; two-phase operator lookup, WP-1496 item 6), cluster 29 (the copy check for an elidable copy before C++17, WP-1496 item 7), and two GCC-only wrong-line expectations (nontype-auto17, crash11) where kiln matches clang.

### Priority directories

| directory | cases | pass | what fails |
|---|---:|---:|---|
| `g++.dg/reflect` | 565 | 306 | 181 rejects. 70 of them are one bug: `constexpr std::meta::info r;` (a default-initialized `info` is the null reflection, a constant) is "not a constant expression" and every later use fails. The rest are spread over metafunctions (`reflect_constant_array`, `members_of`, `extract`, `define_static_array`, `current_class` in a lambda, string-literal reflections that keep the terminator). 69 wrong-line, mostly missing "consteval-only" and "expected a reflection of" diagnostics. WP-1462 fixed the null-`info` bug and the metafunction-pointer, consteval-block scope, default-member-initializer evaluation point, data-member-description layout and literal-terminator clusters: 384/565 pass (107 rejects, 65 wrong-line, 9 accepts-invalid). WP-1482 iterated range arguments (`views::transform`/`filter`/fixed-extent `span`/input ranges), extract of automatic variables, the evaluation point in class template specializations, anonymous-union membership and trait-keyword names: 405/565 pass (86 rejects, 65 wrong-line, 9 accepts-invalid); the linkage and parent, `reflect_constant_array` `is_variable` and GNU extended-identifier rejects are deliberate deviations (docs/status/deviations.md). WP-1489 `vector<bool>` and array range arguments, member template and other splice forms, class splice operands, `^^C<args>::m`, `members_of` order and `is_structural_type`: 429/565 pass (62 rejects, 64 wrong-line: 50 missed and 14 extra errors, 10 accepts-invalid) |
| `g++.dg/contracts` | 123 | 52 | 28 rejects are tests GCC runs before C++26 with `-fcontracts`, a flag kiln does not have, so they fail to parse (`void f() pre(c);`) at `-std=c++23` and `-std=c++20`. 14 are link failures at C++26: GCC's driver links `libstdc++exp` for the default `handle_contract_violation`, and kiln's driver does not. 23 wrong-line. WP-1463 (`-f[no-]contracts`, `-lstdc++exp` as g++ links it, result-name and implicit-`this` rules): 96 pass, 5 rejects-valid (4 where kiln follows [dcl.contract.func]/4-5 and [temp.expl.spec] against GCC, 1 `__builtin_exit`), 2 wrong (they need `-fcontracts-client-check=pre` / `-fcontracts-definition-check=off`, which kiln does not implement), 0 accepts-invalid, 20 wrong-line. WP-1485 (both flags, `__builtin_exit`/`_Exit`, a deduced return type in a templated postcondition): 99 pass, 4 rejects-valid (the [dcl.contract.func]/4-5 and [temp.expl.spec] deviations), 0 wrong, 20 wrong-line (reviewed: different but defensible locations, spec/contracts.md) |
| `g++.dg/cpp26` | 435 | 270 | `constexpr auto [a, b] = ...;` (P2686 constexpr structured bindings, 15), one-argument `va_start(ap)` (7), constexpr `__range` of an expansion statement and constexpr placement new (6), structured bindings of vector types (4). 85 wrong-line, of which 16 are "reference to ... is ambiguous" WP-1464 fixed one-argument `va_start`, array placement new at an array's first element, a run-time `__range` of `std::array`/`span` (CWG3140) and vector/complex structured bindings: 283 pass, 46 compile-reject (the 15 `constexpr auto [a, b]` cases are GCC's pre-C++26 extension, which kiln rejects as clang 22 does). WP-1483 (iterating expansion statements per the working draft, union array placement new, class-type pack index, name-independent `_` after any declaration, `-ferror-limit=0`): 306 pass, 34 compile-reject, 9 accept-invalid, 83 wrong-line, 3 wrong. |
| `g++.dg/cpp2a` | 3784 | 2840 | GCC's `__builtin_is_corresponding_member` and `__builtin_is_pointer_interconvertible_with_class` (30, the builtins behind the P0466 traits), abbreviated templates in odd places (`auto` parameters, 13), constant evaluation (42), CTAD (15). 3 timeouts (crash cluster 3) |
| `g++.dg/cpp23` | 587 | 385 | a constexpr function that returns `__builtin_sqrtf(x)` (GCC folds the math builtins in constant evaluation; kiln rejects the function as "never produces a constant expression", 16), deduced `auto` arrays (6), CTAD (4), deducing `this` (redeclarations, `decltype((self))`), `_Float128`/`__int128` conversion (an LLVM crash) |
| `g++.dg/torture` | 1149 | 1066 | 72 rejects, mostly GCC-only builtins (`__builtin_shuffle`, `__builtin_add_overflow_p`, `__builtin_apply_args`; clang rejects them too); 9 wrong-code |
| `c-c++-common/torture` | 282 | 172 | the same GCC-only builtins (108 rejects); 2 wrong-code (`__builtin_clear_padding`) |

### What the run does not cover

- Message text is not compared, so a wrong diagnostic on the right line
  passes. `scan-*` and `dg-output` checks are not compared.
- The modules, gomp, lto, debug and sanitizer directories, and
  `coroutines/torture`, run under their own `.exp` and are not run here.
- Only `-O0` and `-O2` for the torture suites. Only the host target
  (aarch64). Tests for x86-only effective targets are skip rows.
- Oracles run only on the cases kiln fails, so the GCC and clang columns
  cover those cases only (`--oracles-on all` runs every case, in about
  three times the wall time).

<!-- gxx-dg:begin (generated by cargo xtask gxx-dg) -->

GCC C++ testsuite (`g++.dg`, `c-c++-common`, `g++.old-deja`, `releases/gcc-16.1.0`), host target `aarch64-unknown-linux-gnu`.

- tests: GCC `6afcc4f6da931eb93f3ab001a0dd9650ea71d1ea`; kiln `604edce44`
- command: `cargo xtask gxx-dg -j 4 --suite dg,torture,common,old-deja --opt -O0,-O2 --profile harness`
- kiln: kiln version 1.2.1 (C++26 frontend, LLVM IR backend) (-fclang-compat=22) (--gcc-toolchain=<GCC oracle's prefix>)
- gcc: g++ (GCC) 16.1.0 (-fdiagnostics-plain-output)
- clang: Debian clang version 22.1.8 (1~deb13u4) (--gcc-toolchain=<GCC oracle's prefix>)
- oracles: only the cases kiln does not pass (their counts below cover those cases)
- limits: 2048 MiB address space per step (plus kiln's stack reservation); compile/link timeout 60 s, run timeout 10 s; -j 4
- 20571 tests, 56766 cases run, 786 skip rows; wall 5925 s

### Results by class (cases)

| class | kiln | gcc | clang |
|---|---:|---:|---:|
| pass | 47950 | 8494 | 2636 |
| compile-reject | 3775 | 242 | 2795 |
| crash | 17 | 3 | 18 |
| wrong | 272 | 3 | 164 |
| timeout | 3 | 0 | 0 |
| accept-invalid | 1472 | 34 | 596 |
| wrong-line | 3277 | 40 | 2607 |
| total | 56766 | 8816 | 8816 |

### kiln's failures by triage bucket (cases)

| bucket | cases | GCC's testsuite marks xfail |
|---|---:|---:|
| kiln crash / hang | 20 | 0 |
| wrong-code | 272 | 2 |
| rejects-valid, clang accepts | 997 | 3 |
| rejects-valid, clang rejects too | 2778 | 3 |
| accepts-invalid, clang rejects | 922 | 0 |
| accepts-invalid, clang accepts too | 550 | 0 |
| wrong-line, clang matches | 786 | 0 |
| wrong-line, clang differs too | 2491 | 0 |

### Priority directories (kiln, cases)

| directory | cases | pass | compile-reject | accept-invalid | wrong-line | wrong | crash | timeout |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| g++.dg/reflect | 565 | 306 | 181 | 9 | 69 | 0 | 0 | 0 |
| g++.dg/contracts | 123 | 52 | 47 | 1 | 23 | 0 | 0 | 0 |
| g++.dg/cpp26 | 435 | 270 | 62 | 15 | 85 | 3 | 0 | 0 |
| g++.dg/cpp2a | 3784 | 2840 | 342 | 129 | 464 | 6 | 0 | 3 |
| g++.dg/cpp23 | 587 | 385 | 68 | 36 | 92 | 4 | 2 | 0 |
| g++.dg/torture | 1149 | 1066 | 72 | 2 | 0 | 9 | 0 | 0 |
| c-c++-common/torture | 282 | 172 | 108 | 0 | 0 | 2 | 0 | 0 |

### By directory (kiln, cases)

| directory | cases | pass | compile-reject | accept-invalid | wrong-line | wrong | crash | timeout |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| c-c++-common | 2866 | 2455 | 229 | 101 | 68 | 13 | 0 | 0 |
| c-c++-common/cpp | 420 | 298 | 63 | 18 | 41 | 0 | 0 | 0 |
| c-c++-common/torture | 282 | 172 | 108 | 0 | 0 | 2 | 0 | 0 |
| g++.dg | 942 | 849 | 49 | 8 | 33 | 0 | 3 | 0 |
| g++.dg/DRs | 203 | 114 | 24 | 8 | 33 | 24 | 0 | 0 |
| g++.dg/abi | 772 | 702 | 53 | 5 | 3 | 9 | 0 | 0 |
| g++.dg/concepts | 409 | 269 | 47 | 18 | 75 | 0 | 0 | 0 |
| g++.dg/contracts | 123 | 52 | 47 | 1 | 23 | 0 | 0 | 0 |
| g++.dg/conversion | 279 | 244 | 6 | 3 | 26 | 0 | 0 | 0 |
| g++.dg/coroutines | 533 | 416 | 33 | 15 | 69 | 0 | 0 | 0 |
| g++.dg/cpp | 213 | 174 | 13 | 7 | 18 | 1 | 0 | 0 |
| g++.dg/cpp0x | 9121 | 7685 | 538 | 234 | 641 | 20 | 3 | 0 |
| g++.dg/cpp1y | 2133 | 1750 | 123 | 73 | 183 | 4 | 0 | 0 |
| g++.dg/cpp1z | 1707 | 1438 | 104 | 57 | 99 | 9 | 0 | 0 |
| g++.dg/cpp23 | 587 | 385 | 68 | 36 | 92 | 4 | 2 | 0 |
| g++.dg/cpp26 | 435 | 270 | 62 | 15 | 85 | 3 | 0 | 0 |
| g++.dg/cpp2a | 3784 | 2840 | 342 | 129 | 464 | 6 | 0 | 3 |
| g++.dg/delayedfold | 21 | 15 | 3 | 0 | 0 | 3 | 0 | 0 |
| g++.dg/diagnostic | 579 | 437 | 24 | 49 | 66 | 3 | 0 | 0 |
| g++.dg/eh | 394 | 338 | 7 | 15 | 7 | 27 | 0 | 0 |
| g++.dg/expr | 406 | 374 | 18 | 6 | 5 | 3 | 0 | 0 |
| g++.dg/ext | 2901 | 2108 | 416 | 99 | 224 | 51 | 3 | 0 |
| g++.dg/inherit | 337 | 309 | 10 | 3 | 15 | 0 | 0 | 0 |
| g++.dg/init | 1101 | 952 | 43 | 27 | 59 | 20 | 0 | 0 |
| g++.dg/ipa | 686 | 668 | 18 | 0 | 0 | 0 | 0 | 0 |
| g++.dg/lookup | 1025 | 854 | 67 | 54 | 50 | 0 | 0 | 0 |
| g++.dg/opt | 1442 | 1362 | 62 | 3 | 0 | 15 | 0 | 0 |
| g++.dg/other | 1033 | 897 | 47 | 29 | 60 | 0 | 0 | 0 |
| g++.dg/overload | 294 | 250 | 18 | 11 | 15 | 0 | 0 | 0 |
| g++.dg/parse | 1851 | 1455 | 99 | 84 | 213 | 0 | 0 | 0 |
| g++.dg/reflect | 565 | 306 | 181 | 9 | 69 | 0 | 0 | 0 |
| g++.dg/rtti | 118 | 103 | 0 | 3 | 9 | 3 | 0 | 0 |
| g++.dg/tc1 | 105 | 75 | 15 | 6 | 9 | 0 | 0 | 0 |
| g++.dg/template | 4699 | 3894 | 344 | 199 | 247 | 9 | 6 | 0 |
| g++.dg/torture | 1149 | 1066 | 72 | 2 | 0 | 9 | 0 | 0 |
| g++.dg/tree-ssa | 860 | 838 | 19 | 0 | 3 | 0 | 0 | 0 |
| g++.dg/warn | 2840 | 2657 | 143 | 19 | 18 | 3 | 0 | 0 |
| g++.dg/wrappers | 30 | 23 | 7 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.abi | 111 | 108 | 3 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.benjamin | 126 | 105 | 9 | 0 | 12 | 0 | 0 | 0 |
| g++.old-deja/g++.bob | 54 | 51 | 0 | 0 | 3 | 0 | 0 | 0 |
| g++.old-deja/g++.brendan | 919 | 884 | 11 | 14 | 10 | 0 | 0 | 0 |
| g++.old-deja/g++.bugs | 286 | 262 | 2 | 5 | 17 | 0 | 0 | 0 |
| g++.old-deja/g++.eh | 207 | 197 | 0 | 2 | 1 | 7 | 0 | 0 |
| g++.old-deja/g++.ext | 117 | 70 | 31 | 4 | 6 | 6 | 0 | 0 |
| g++.old-deja/g++.gb | 39 | 38 | 0 | 0 | 1 | 0 | 0 | 0 |
| g++.old-deja/g++.jason | 1217 | 1142 | 21 | 16 | 32 | 6 | 0 | 0 |
| g++.old-deja/g++.law | 771 | 721 | 27 | 12 | 11 | 0 | 0 | 0 |
| g++.old-deja/g++.martin | 54 | 48 | 6 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.mike | 1040 | 984 | 37 | 2 | 9 | 8 | 0 | 0 |
| g++.old-deja/g++.niklas | 81 | 80 | 0 | 1 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.ns | 273 | 264 | 0 | 0 | 9 | 0 | 0 | 0 |
| g++.old-deja/g++.oliva | 114 | 90 | 12 | 8 | 3 | 1 | 0 | 0 |
| g++.old-deja/g++.other | 1509 | 1400 | 36 | 13 | 60 | 0 | 0 | 0 |
| g++.old-deja/g++.pt | 2177 | 2024 | 51 | 42 | 57 | 3 | 0 | 0 |
| g++.old-deja/g++.rfg | 6 | 6 | 0 | 0 | 0 | 0 | 0 | 0 |
| g++.old-deja/g++.robertl | 402 | 364 | 7 | 7 | 24 | 0 | 0 | 0 |
| g++.old-deja/g++.warn | 18 | 18 | 0 | 0 | 0 | 0 | 0 | 0 |
| total | 56766 | 47950 | 3775 | 1472 | 3277 | 272 | 17 | 3 |

### Clusters: kiln crash / hang (5 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 6 | crash: out of memory | g++.dg/template/array30.C -std=c++20, g++.dg/template/array30.C -std=c++26, g++.dg/template/array30.C -std=c++98 |
| 5 | crash: crash: LLVM crash | g++.dg/cpp23/ext-floating14.C -std=gnu++23, g++.dg/cpp23/ext-floating14.C -std=gnu++26, g++.dg/ext/asm16.C -std=c++20 |
| 3 | crash: compile: signal: N (SIGABRT) | g++.dg/cpp0x/constexpr-nsdmi2.C -std=c++11, g++.dg/cpp0x/constexpr-nsdmi2.C -std=c++20, g++.dg/cpp0x/constexpr-nsdmi2.C -std=c++26 |
| 3 | crash: internal compiler error: internal error: entered unreachable code: const_int on non-integer type ptr | g++.dg/freestanding-nonint-main.C -std=gnu++20, g++.dg/freestanding-nonint-main.C -std=gnu++26, g++.dg/freestanding-nonint-main.C -std=gnu++98 |
| 3 | timeout: compile timeout | g++.dg/cpp2a/concepts-recursive-sat1.C -std=c++20, g++.dg/cpp2a/concepts-recursive-sat1.C -std=c++23, g++.dg/cpp2a/concepts-recursive-sat1.C -std=c++26 |

### Clusters: wrong-code (3 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 198 | wrong: signal: N (SIGABRT) | c-c++-common/attr-aligned-1.c -std=c++98, c-c++-common/auto-init-padding-3.c -std=gnu++20, c-c++-common/auto-init-padding-3.c -std=gnu++26 |
| 64 | wrong: exit N | g++.dg/abi/bitfield1.C -std=gnu++20, g++.dg/abi/bitfield1.C -std=gnu++26, g++.dg/abi/bitfield1.C -std=gnu++98 |
| 10 | wrong: signal: N (SIGSEGV) | g++.dg/ext/label13a.C -std=gnu++20, g++.dg/ext/label13a.C -std=gnu++26, g++.dg/ext/label13a.C -std=gnu++98 |

### Clusters: rejects-valid, clang accepts (171 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 60 | compile-reject: error: no matching function for call to '…' | g++.dg/DRs/dr2303.C -std=c++11, g++.dg/DRs/dr2303.C -std=c++20, g++.dg/DRs/dr2303.C -std=c++26 |
| 39 | compile-reject: error: expected member name or '…' after declaration specifiers | g++.dg/DRs/dr2237-2.C -std=gnu++20, g++.dg/DRs/dr2237-2.C -std=gnu++26, g++.dg/DRs/dr2237-2.C -std=gnu++98 |
| 34 | compile-reject: error: expected unqualified-id | c-c++-common/Wimplicit-fallthrough-24.c -std=gnu++20, c-c++-common/Wimplicit-fallthrough-24.c -std=gnu++26, c-c++-common/Wimplicit-fallthrough-24.c -std=gnu++98 |
| 32 | compile-reject: error: static assertion failed due to requirement '…':  | g++.dg/cpp0x/inh-ctor38.C -std=c++11, g++.dg/cpp0x/inh-ctor38.C -std=c++20, g++.dg/cpp0x/inh-ctor38.C -std=c++26 |
| 31 | compile-reject: error: no viable constructor or deduction guide for deduction of template arguments of '…' | g++.dg/cpp/embed-21.C -std=gnu++20, g++.dg/cpp/embed-21.C -std=gnu++23, g++.dg/cpp/embed-21.C -std=gnu++26 |
| 27 | compile-reject: link: undefined reference to `… | c-c++-common/restrict-1.c -std=gnu++20, c-c++-common/restrict-1.c -std=gnu++26, c-c++-common/restrict-1.c -std=gnu++98 |
| 25 | compile-reject: error: cannot initialize a variable of type '…' with an lvalue of type '…' | g++.dg/warn/Wall-write-strings.C -std=gnu++20, g++.dg/warn/Wall-write-strings.C -std=gnu++26, g++.dg/warn/Wall-write-strings.C -std=gnu++98 |
| 19 | compile-reject: error: constexpr variable '…' must be initialized by a constant expression | g++.dg/cpp0x/constexpr-const2.C -std=c++11, g++.dg/cpp0x/constexpr-const2.C -std=c++20, g++.dg/cpp0x/constexpr-const2.C -std=c++26 |
| 18 | compile-reject: error: constructors is not supported yet | g++.dg/cpp2a/typename16.C -std=c++20, g++.dg/cpp2a/typename16.C -std=c++23, g++.dg/cpp2a/typename16.C -std=c++26 |
| 17 | compile-reject: error: zero size arrays are an extension [-Werror,-Wzero-length-array] | g++.dg/cpp2a/constexpr-init13.C -std=c++20, g++.dg/cpp2a/constexpr-init13.C -std=c++23, g++.dg/cpp2a/constexpr-init13.C -std=c++26 |
| 15 | compile-reject: error: '…' is a private member of '…' | g++.dg/other/access3.C -std=gnu++20, g++.dg/other/access3.C -std=gnu++26, g++.dg/other/access3.C -std=gnu++98 |
| 15 | compile-reject: error: C-style cast from '…' (aka '…') to '…' (aka '…') is not allowed | g++.dg/other/ptrmem3.C -std=c++20, g++.dg/other/ptrmem3.C -std=c++26, g++.dg/other/ptrmem3.C -std=c++98 |
| 15 | compile-reject: error: cannot initialize a variable of type '…' with an rvalue of type '…' | g++.dg/cpp0x/auto24.C -std=c++11, g++.dg/cpp0x/auto24.C -std=c++20, g++.dg/cpp0x/auto24.C -std=c++26 |
| 15 | compile-reject: error: expected class name | g++.dg/template/dependent-base4.C -std=c++20, g++.dg/template/dependent-base4.C -std=c++26, g++.dg/template/dependent-base4.C -std=c++98 |
| 15 | compile-reject: error: variable length array bound not emitted | c-c++-common/vla-1.c -std=gnu++20, c-c++-common/vla-1.c -std=gnu++26, c-c++-common/vla-1.c -std=gnu++98 |
| 12 | compile-reject: error: C-style cast from '…' to '…' (aka '…') is not allowed | g++.dg/Wcast-function-type.C -std=gnu++20, g++.dg/Wcast-function-type.C -std=gnu++26, g++.dg/Wcast-function-type.C -std=gnu++98 |
| 12 | compile-reject: error: functions that differ only in their return type cannot be overloaded | g++.dg/ext/attrib63.C -std=c++20, g++.dg/ext/attrib63.C -std=c++26, g++.dg/ext/attrib63.C -std=c++98 |
| 9 | compile-reject: error: '…' following the '…' keyword does not refer to a template | g++.dg/template/lookup12.C -std=c++20, g++.dg/template/lookup12.C -std=c++26, g++.dg/template/lookup12.C -std=c++98 |
| 9 | compile-reject: error: expected expression | g++.dg/ext/pr116545.C -std=gnu++14, g++.dg/ext/pr116545.C -std=gnu++20, g++.dg/ext/pr116545.C -std=gnu++98 |
| 9 | compile-reject: error: flexible array members are a CN feature [-Werror,-WcN-extensions] | g++.dg/ext/flexarray-mangle-2.C -std=c++11, g++.dg/ext/flexarray-mangle-2.C -std=c++20, g++.dg/ext/flexarray-mangle-2.C -std=c++26 |
| 9 | compile-reject: error: member reference base type '…' is not a structure or union | g++.dg/cpp2a/lambda-targ7.C -std=c++20, g++.dg/cpp2a/lambda-targ7.C -std=c++23, g++.dg/cpp2a/lambda-targ7.C -std=c++26 |
| 9 | compile-reject: error: no matching member function for call to '…' | g++.dg/cpp0x/lambda/lambda-current-inst1.C -std=c++11, g++.dg/cpp0x/lambda/lambda-current-inst1.C -std=c++20, g++.dg/cpp0x/lambda/lambda-current-inst1.C -std=c++26 |
| 9 | compile-reject: error: range-based for over a variable length array is not supported yet | g++.dg/cpp0x/pr70338.C -std=gnu++11, g++.dg/cpp0x/pr70338.C -std=gnu++20, g++.dg/cpp0x/pr70338.C -std=gnu++26 |
| 9 | compile-reject: error: use of overloaded operator '…' is ambiguous (with operand types '…' and '…') | g++.dg/cpp0x/pr85070.C -std=c++11, g++.dg/cpp0x/pr85070.C -std=c++20, g++.dg/cpp0x/pr85070.C -std=c++26 |
| 8 | compile-reject: error: IR lowering failed: IrError { kind: BadCall, message: "call with N arguments to a function taking N" } | g++.dg/tc1/dr127.C -std=c++20, g++.dg/tc1/dr127.C -std=c++26, g++.dg/tc1/dr127.C -std=c++98 |
| 8 | compile-reject: error: static assertion failed due to requirement '…' | g++.dg/cpp23/explicit-obj-lambda17.C -std=c++23, g++.dg/cpp23/explicit-obj-lambda17.C -std=c++26, g++.dg/cpp2a/concepts-pr67825.C -std=c++20 |
| 8 | compile-reject: error: type '…' cannot be used prior to '…' because it has no members | g++.dg/cpp0x/constexpr-decltype5.C -std=c++20, g++.dg/cpp0x/constexpr-decltype5.C -std=c++26, g++.dg/cpp2a/concepts-fn7.C -std=c++20 |
| 8 | compile-reject: error: variable '…' with type '…' has incompatible initializer of type '…' | g++.dg/DRs/dr2918.C -std=c++20, g++.dg/DRs/dr2918.C -std=c++23, g++.dg/DRs/dr2918.C -std=c++26 |
| 7 | compile-reject: error: ISO C++N does not allow '…' storage class specifier [-Wregister] | c-c++-common/vector-subscript-2.c -std=c++98, g++.dg/opt/pr82577.C -std=c++98, g++.dg/parse/linkage2.C -std=c++98 |
| 7 | compile-reject: error: explicit instantiation of '…' does not refer to a function template, variable template, member function, member class, or static data member | g++.dg/abi/mangle13.C -std=gnu++20, g++.dg/abi/mangle13.C -std=gnu++26, g++.dg/abi/mangle13.C -std=gnu++98 |

### Clusters: rejects-valid, clang rejects too (369 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 331 | compile-reject: error: use of undeclared identifier '…' | c-c++-common/Wsuggest-attribute-1.c -std=gnu++11, c-c++-common/Wsuggest-attribute-1.c -std=gnu++20, c-c++-common/Wsuggest-attribute-1.c -std=gnu++26 |
| 121 | compile-reject: error: constexpr variable '…' must be initialized by a constant expression | g++.dg/cpp0x/constexpr-92443.C -std=c++11, g++.dg/cpp0x/constexpr-92443.C -std=c++20, g++.dg/cpp0x/constexpr-92443.C -std=c++26 |
| 60 | compile-reject: error: static assertion expression is not an integral constant expression | g++.dg/cpp0x/constexpr-50060.C -std=c++14, g++.dg/cpp0x/constexpr-50060.C -std=c++20, g++.dg/cpp0x/constexpr-array-ptr6.C -std=c++11 |
| 55 | compile-reject: error: static assertion failed due to requirement '…' | g++.dg/abi/no_unique_address8a.C -std=gnu++11, g++.dg/abi/no_unique_address8a.C -std=gnu++20, g++.dg/abi/no_unique_address8a.C -std=gnu++26 |
| 53 | compile-reject: error: a type specifier is required for all declarations | g++.dg/abi/vbase8-10.C -std=gnu++20, g++.dg/abi/vbase8-10.C -std=gnu++26, g++.dg/abi/vbase8-10.C -std=gnu++98 |
| 53 | compile-reject: error: unknown type name '…' | g++.dg/concepts/abbrev11.C -std=c++14, g++.dg/concepts/class5.C -std=gnu++17, g++.dg/concepts/debug1.C -std=gnu++17 |
| 49 | compile-reject: link: undefined reference to `… | c-c++-common/pr19807-2.c -std=gnu++20, c-c++-common/pr19807-2.c -std=gnu++26, c-c++-common/pr19807-2.c -std=gnu++98 |
| 45 | compile-reject: error: '…' is a CN extension [-Werror,-WcN-extensions] | g++.dg/cpp0x/constexpr-71828.C -std=c++11, g++.dg/cpp0x/constexpr-71828.C -std=c++20, g++.dg/cpp0x/constexpr-71828.C -std=c++26 |
| 41 | compile-reject: error: expected member name or '…' after declaration specifiers | g++.dg/cpp0x/pr70538.C -std=c++11, g++.dg/cpp0x/pr70538.C -std=c++20, g++.dg/cpp0x/pr70538.C -std=c++26 |
| 40 | compile-reject: error: no matching function for call to '…' | g++.dg/abi/noreturn1.C -std=gnu++20, g++.dg/abi/noreturn1.C -std=gnu++26, g++.dg/abi/noreturn1.C -std=gnu++98 |
| 39 | compile-reject: error: expected '…' | c-c++-common/toplevel-asm-1.c -std=gnu++20, c-c++-common/toplevel-asm-1.c -std=gnu++26, c-c++-common/toplevel-asm-1.c -std=gnu++98 |
| 36 | compile-reject: error: invalid suffix on literal; C++N requires a space between literal and identifier [-Wreserved-user-defined-literal] | c-c++-common/cpp/dir-only-7.c -, g++.dg/cpp0x/Wliteral-suffix.C -std=c++11, g++.dg/cpp0x/Wliteral-suffix.C -std=c++20 |
| 33 | compile-reject: error: variable-sized object may not be initialized | g++.dg/cpp1y/lambda-vla1.C -std=c++14, g++.dg/cpp1y/lambda-vla1.C -std=c++20, g++.dg/cpp1y/lambda-vla1.C -std=c++26 |
| 31 | compile-reject: error: expected '…' after top level declarator | c-c++-common/diagnostic-format-sarif-file-bad-utf8-pr109098-2.c -std=gnu++20, c-c++-common/diagnostic-format-sarif-file-bad-utf8-pr109098-2.c -std=gnu++26, c-c++-common/diagnostic-format-sarif-file-bad-utf8-pr109098-2.c -std=gnu++98 |
| 30 | compile-reject: error: redefinition of '…' | c-c++-common/pr33763.c -std=gnu++20, c-c++-common/pr33763.c -std=gnu++26, c-c++-common/pr33763.c -std=gnu++98 |
| 30 | compile-reject: error: static assertion failed due to requirement '…':  | g++.dg/abi/base-defaulted1a.C -std=c++20, g++.dg/abi/base-defaulted1a.C -std=c++26, g++.dg/cpp0x/initlist-ref3.C -std=c++11 |
| 28 | compile-reject: error: '…' attribute cannot be applied to types | g++.dg/cpp0x/alias-decl-59.C -std=c++11, g++.dg/cpp0x/alias-decl-59.C -std=c++20, g++.dg/cpp0x/alias-decl-59.C -std=c++26 |
| 27 | compile-reject: error: use '…' keyword to treat '…' as a dependent template name | g++.dg/cpp0x/decltype34.C -std=c++11, g++.dg/cpp0x/decltype34.C -std=c++20, g++.dg/cpp0x/decltype34.C -std=c++26 |
| 26 | compile-reject: error: expected unqualified-id | c-c++-common/pr92833-4.c -std=c++20, c-c++-common/pr92833-4.c -std=c++26, c-c++-common/pr92833-4.c -std=c++98 |
| 25 | compile-reject: error: pack expansion does not contain any unexpanded parameter packs | g++.dg/cpp0x/constexpr-96241.C -std=c++11, g++.dg/cpp0x/constexpr-96241.C -std=c++20, g++.dg/cpp0x/constexpr-96241.C -std=c++26 |
| 22 | compile-reject: error: constexpr function never produces a constant expression [-Winvalid-constexpr] | g++.dg/cpp0x/constexpr-noreturn1.C -std=c++11, g++.dg/cpp0x/constexpr-noreturn1.C -std=c++20, g++.dg/cpp0x/constexpr-noreturn1.C -std=c++26 |
| 21 | compile-reject: error: cannot initialize a variable of type '…' with an rvalue of type '…' | c-c++-common/pr60101.c -std=gnu++20, c-c++-common/pr60101.c -std=gnu++26, c-c++-common/pr60101.c -std=gnu++98 |
| 21 | compile-reject: error: character <U+NA> not allowed in an identifier | c-c++-common/Wbidi-chars-10.c -std=gnu++20, c-c++-common/Wbidi-chars-10.c -std=gnu++26, c-c++-common/Wbidi-chars-10.c -std=gnu++98 |
| 20 | compile-reject: error: '…' not allowed in function prototype | g++.dg/concepts/abbrev1.C -std=c++14, g++.dg/concepts/abbrev10.C -std=c++14, g++.dg/concepts/abbrev2.C -std=c++14 |
| 18 | compile-reject: error: an attribute list cannot appear here | g++.dg/DRs/dr2262.C -std=gnu++11, g++.dg/DRs/dr2262.C -std=gnu++20, g++.dg/DRs/dr2262.C -std=gnu++26 |
| 17 | compile-reject: error: reference member '…' binds to a temporary object whose lifetime would be shorter than the lifetime of the constructed object | g++.dg/cpp0x/constexpr-ctor11.C -std=c++11, g++.dg/cpp0x/constexpr-ctor11.C -std=c++20, g++.dg/cpp0x/constexpr-ctor11.C -std=c++26 |
| 16 | compile-reject: error: '…' cannot be defined in a type specifier | g++.dg/ext/complit15.C -, g++.dg/opt/pr20931.C -std=gnu++20, g++.dg/opt/pr20931.C -std=gnu++26 |
| 16 | compile-reject: error: implicit instantiation of undefined template '…' | g++.dg/cpp0x/alias-decl-dr1286.C -std=c++11, g++.dg/cpp0x/alias-decl-dr1286.C -std=c++20, g++.dg/cpp0x/alias-decl-dr1286.C -std=c++26 |
| 16 | compile-reject: error: type '…' cannot be used prior to '…' because it has no members | g++.dg/cpp0x/constexpr-decltype5.C -std=c++11, g++.dg/cpp0x/fntmpdefarg5.C -std=c++11, g++.dg/cpp0x/fntmpdefarg5.C -std=c++20 |
| 15 | compile-reject: error: '…' is a private member of '…' | g++.dg/template/inherit9.C -std=c++20, g++.dg/template/inherit9.C -std=c++26, g++.dg/template/inherit9.C -std=c++98 |

### Clusters: accepts-invalid, clang rejects (267 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 130 | accept-invalid: no error (expected: ) | g++.dg/concepts/placeholder3.C -std=gnu++20, g++.dg/concepts/placeholder3.C -std=gnu++26, g++.dg/concepts/placeholder4.C -std=gnu++20 |
| 24 | accept-invalid: no error (expected: modifying a const object) | g++.dg/cpp1y/constexpr-tracking-const16.C -std=c++14, g++.dg/cpp1y/constexpr-tracking-const16.C -std=c++20, g++.dg/cpp1y/constexpr-tracking-const16.C -std=c++26 |
| 12 | accept-invalid: no error (expected: match) | g++.dg/cpp2a/concepts-explicit-inst4.C -std=c++20, g++.dg/cpp2a/concepts-explicit-inst4.C -std=c++23, g++.dg/cpp2a/concepts-explicit-inst4.C -std=c++26 |
| 10 | accept-invalid: no error (expected: ambiguous) | g++.dg/concepts/variadic2.C -std=gnu++20, g++.dg/cpp0x/overload-conv-3.C -std=c++11, g++.dg/cpp0x/overload-conv-3.C -std=c++14 |
| 10 | accept-invalid: no error (expected: defaulted) | g++.dg/cpp0x/constexpr-68754.C -std=c++11, g++.dg/cpp0x/defaulted51.C -std=c++11, g++.dg/cpp0x/defaulted51.C -std=c++17 |
| 9 | accept-invalid: no error (expected: default arguments) | g++.dg/other/default12.C -std=c++20, g++.dg/other/default12.C -std=c++26, g++.dg/other/default12.C -std=c++98 |
| 9 | accept-invalid: no error (expected: incomplete) | g++.dg/cpp0x/variadic103.C -std=c++11, g++.dg/cpp0x/variadic103.C -std=c++20, g++.dg/cpp0x/variadic103.C -std=c++26 |
| 9 | accept-invalid: no error (expected: this context) | g++.dg/cpp0x/defaulted28.C -std=c++11, g++.dg/cpp0x/defaulted28.C -std=c++20, g++.dg/cpp0x/defaulted28.C -std=c++26 |
| 9 | accept-invalid: no error (expected: uninitialized reference) | g++.dg/cpp0x/aggr2.C -std=c++11, g++.dg/cpp0x/aggr2.C -std=c++20, g++.dg/cpp0x/aggr2.C -std=c++26 |
| 8 | accept-invalid: no error (expected: cannot be overloaded with) | g++.dg/cpp23/explicit-obj-redecl.C -std=c++23, g++.dg/cpp23/explicit-obj-redecl.C -std=c++26, g++.dg/cpp23/explicit-obj-redecl2.C -std=gnu++23 |
| 8 | accept-invalid: no error (expected: deleted) | g++.dg/cpp0x/union4.C -std=c++26, g++.dg/cpp1z/elide6.C -std=c++11, g++.dg/cpp1z/elide6.C -std=c++14 |
| 6 | accept-invalid: no error (expected: .A..& a... is not a constant expression) | g++.dg/cpp1y/constexpr-nsdmi6a.C -std=c++14, g++.dg/cpp1y/constexpr-nsdmi6a.C -std=c++20, g++.dg/cpp1y/constexpr-nsdmi6a.C -std=c++26 |
| 6 | accept-invalid: no error (expected: N:destructor .Foo::~Foo\\\(\\\).) | g++.dg/template/dtor2.C -std=c++20, g++.dg/template/dtor2.C -std=c++26, g++.dg/template/dtor2.C -std=c++98 |
| 6 | accept-invalid: no error (expected: N:typedef) | g++.dg/other/pr84792-1.C -std=c++20, g++.dg/other/pr84792-1.C -std=c++26, g++.dg/other/pr84792-1.C -std=c++98 |
| 6 | accept-invalid: no error (expected: constraint) | g++.dg/cpp2a/concepts-class4.C -std=c++20, g++.dg/cpp2a/concepts-class4.C -std=c++23, g++.dg/cpp2a/concepts-class4.C -std=c++26 |
| 6 | accept-invalid: no error (expected: default argument) | g++.dg/template/defarg19.C -std=c++20, g++.dg/template/defarg19.C -std=c++26, g++.dg/template/defarg19.C -std=c++98 |
| 6 | accept-invalid: no error (expected: different access) | g++.dg/cpp0x/enum33.C -std=c++11, g++.dg/cpp0x/enum33.C -std=c++20, g++.dg/cpp0x/enum33.C -std=c++26 |
| 6 | accept-invalid: no error (expected: implicitly deleted) | g++.dg/cpp0x/defaulted52.C -std=c++11, g++.dg/cpp0x/defaulted52.C -std=c++17, g++.dg/cpp0x/defaulted59.C -std=c++11 |
| 6 | accept-invalid: no error (expected: lambda templates are only available with) | g++.dg/cpp1y/lambda-generic-77914.C -std=c++14, g++.dg/cpp1y/lambda-generic-77914.C -std=c++17, g++.dg/cpp2a/lambda-generic1.C -std=c++14 |
| 6 | accept-invalid: no error (expected: mismatch\|class template) | g++.dg/parse/crash11.C -std=c++20, g++.dg/parse/crash11.C -std=c++26, g++.dg/parse/crash11.C -std=c++98 |
| 6 | accept-invalid: no error (expected: no match for '…') | g++.dg/lookup/operator-1.C -std=c++11, g++.dg/lookup/operator-1.C -std=c++20, g++.dg/lookup/operator-1.C -std=c++26 |
| 6 | accept-invalid: no error (expected: pack expansion) | g++.dg/cpp0x/alias-decl-37.C -std=c++11, g++.dg/cpp0x/alias-decl-37.C -std=c++20, g++.dg/cpp0x/alias-decl-37.C -std=c++26 |
| 6 | accept-invalid: no error (expected: private) | g++.dg/cpp0x/enum32.C -std=c++11, g++.dg/cpp0x/enum32.C -std=c++20, g++.dg/cpp0x/enum32.C -std=c++26 |
| 6 | accept-invalid: no error (expected: redefinition) | g++.dg/cpp0x/pr79361-1.C -std=c++11, g++.dg/cpp0x/pr79361-1.C -std=c++20, g++.dg/cpp0x/pr79361-1.C -std=c++26 |
| 6 | accept-invalid: no error (expected: return-type-requirement is not a type-constraint) | g++.dg/cpp2a/concepts-pr69235.C -std=c++20, g++.dg/cpp2a/concepts-pr69235.C -std=c++23, g++.dg/cpp2a/concepts-pr69235.C -std=c++26 |
| 6 | accept-invalid: no error (expected: rvalue) | g++.old-deja/g++.eh/ctor1.C -std=c++14, g++.old-deja/g++.eh/ctor1.C -std=c++98, g++.old-deja/g++.mike/p2431.C -std=c++14 |
| 6 | accept-invalid: no error (expected: shadows a parameter) | g++.dg/parse/pr31952-1.C -std=c++20, g++.dg/parse/pr31952-1.C -std=c++26, g++.dg/parse/pr31952-1.C -std=c++98 |
| 6 | accept-invalid: no error (expected: too many braces) | g++.dg/cpp0x/initlist76.C -std=c++11, g++.dg/cpp0x/initlist76.C -std=c++20, g++.dg/cpp0x/initlist76.C -std=c++26 |
| 6 | accept-invalid: no error (expected: uninitialized) | g++.dg/cpp0x/constexpr-array12.C -std=c++11, g++.dg/cpp0x/constexpr-array12.C -std=c++20, g++.dg/cpp0x/constexpr-array12.C -std=c++26 |
| 6 | accept-invalid: no error (expected: void) | g++.dg/cpp1z/nontype-auto17.C -std=c++11, g++.dg/cpp1z/nontype-auto17.C -std=c++20, g++.dg/cpp1z/nontype-auto17.C -std=c++26 |

### Clusters: accepts-invalid, clang accepts too (153 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 38 | accept-invalid: no error (expected: ) | g++.dg/cpp0x/constexpr-ctor14a.C -std=gnu++11, g++.dg/cpp0x/constexpr-ctor14a.C -std=gnu++14, g++.dg/cpp1z/constexpr-lambda10.C -std=c++17 |
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

### Clusters: wrong-line, clang matches (216 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 62 | wrong-line: missed error at LN (expected: ) | g++.dg/conversion/ptrmem3.C -std=c++20, g++.dg/conversion/ptrmem3.C -std=c++26, g++.dg/conversion/ptrmem3.C -std=c++98 |
| 27 | wrong-line: extra error at LN: error: expected '…' | g++.dg/cpp0x/decltype63.C -std=c++11, g++.dg/cpp0x/decltype63.C -std=c++20, g++.dg/cpp0x/decltype63.C -std=c++26 |
| 18 | wrong-line: extra error at LN: error: expected unqualified-id | g++.dg/cpp0x/decltype13.C -std=c++20, g++.dg/cpp0x/decltype13.C -std=c++26, g++.dg/cpp0x/decltype13.C -std=c++98 |
| 15 | wrong-line: extra error at LN: error: static assertion expression is not an integral constant expression | g++.dg/cpp2a/constexpr-dynamic4.C -std=c++20, g++.dg/cpp2a/constexpr-dynamic4.C -std=c++23, g++.dg/cpp2a/constexpr-dynamic4.C -std=c++26 |
| 15 | wrong-line: missed error at LN (expected: ambiguous) | g++.dg/cpp0x/variadic-alias6.C -std=c++11, g++.dg/cpp0x/variadic-alias6.C -std=c++20, g++.dg/cpp0x/variadic-alias6.C -std=c++26 |
| 12 | wrong-line: extra error at LN: error: expected expression | g++.dg/cpp0x/range-for23.C -std=c++11, g++.dg/cpp0x/range-for23.C -std=c++20, g++.dg/cpp0x/range-for23.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: N:expected) | g++.dg/cpp1y/pr65202.C -std=c++14, g++.dg/cpp1y/pr65202.C -std=c++20, g++.dg/cpp1y/pr65202.C -std=c++26 |
| 9 | wrong-line: extra error at LN: error: C-style cast from '…' to '…' is not allowed | g++.dg/conversion/ptrmem4.C -std=c++20, g++.dg/conversion/ptrmem4.C -std=c++26, g++.dg/conversion/ptrmem4.C -std=c++98 |
| 9 | wrong-line: extra error at LN: error: no viable constructor or deduction guide for deduction of template arguments of '…' | g++.dg/cpp2a/class-deduction-aggr11.C -std=c++20, g++.dg/cpp2a/class-deduction-aggr11.C -std=c++23, g++.dg/cpp2a/class-deduction-aggr11.C -std=c++26 |
| 9 | wrong-line: extra error at LN: error: value of type '…' is not implicitly convertible to '…' | g++.dg/template/void3.C -std=c++20, g++.dg/template/void3.C -std=c++26, g++.dg/template/void3.C -std=c++98 |
| 9 | wrong-line: missed error at LN (expected: depth) | g++.dg/cpp0x/decltype26.C -std=c++11, g++.dg/cpp0x/decltype26.C -std=c++20, g++.dg/cpp0x/decltype26.C -std=c++26 |
| 8 | wrong-line: extra error at LN: error: use of undeclared identifier '…' | g++.dg/cpp2a/range-for7.C -, g++.dg/parse/error61.C -std=c++11, g++.dg/parse/error61.C -std=c++20 |
| 6 | wrong-line: extra error at LN: error: cannot initialize an array element of type '…' with an lvalue of type '…' | g++.dg/cpp2a/paren-init36.C -std=c++20, g++.dg/cpp2a/paren-init36.C -std=c++23, g++.dg/cpp2a/paren-init36.C -std=c++26 |
| 6 | wrong-line: extra error at LN: error: default member initializer for '…' needed within definition of enclosing class '…' outside of member functions | g++.dg/ext/flexary23.C -std=gnu++14, g++.dg/ext/flexary23.C -std=gnu++20, g++.dg/ext/flexary23.C -std=gnu++26 |
| 6 | wrong-line: extra error at LN: error: flexible array members are a CN feature [-Werror,-WcN-extensions] | g++.dg/pr69338.C -std=c++11, g++.dg/pr69338.C -std=c++20, g++.dg/pr69338.C -std=c++26 |
| 6 | wrong-line: extra error at LN: error: kiln: not yet implemented: pack expansion in this context | g++.dg/cpp1z/using-variadic1.C -std=c++17, g++.dg/cpp1z/using-variadic1.C -std=c++20, g++.dg/cpp1z/using-variadic1.C -std=c++26 |
| 6 | wrong-line: extra error at LN: error: no member named '…' in '…' | g++.dg/cpp0x/pr58080.C -std=c++11, g++.dg/cpp0x/pr58080.C -std=c++20, g++.dg/cpp0x/pr58080.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: N:expected '…' at end of input) | g++.dg/diagnostic/unclosed-function.C -std=c++20, g++.dg/diagnostic/unclosed-function.C -std=c++26, g++.dg/diagnostic/unclosed-function.C -std=c++98 |
| 6 | wrong-line: missed error at LN (expected: called in a constant expression before its definition is complete\|is not an inte…) | g++.dg/cpp1y/constexpr-recursion1.C -std=gnu++14, g++.dg/cpp1y/constexpr-recursion1.C -std=gnu++20, g++.dg/cpp1y/constexpr-recursion1.C -std=gnu++26 |
| 6 | wrong-line: missed error at LN (expected: default arguments) | g++.dg/other/default5.C -std=c++20, g++.dg/other/default5.C -std=c++26, g++.dg/other/default5.C -std=c++98 |
| 6 | wrong-line: missed error at LN (expected: depends on itself) | g++.dg/cpp2a/concepts-pr88395.C -std=c++20, g++.dg/cpp2a/concepts-pr88395.C -std=c++23, g++.dg/cpp2a/concepts-pr88395.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: different kind of entity) | g++.dg/cpp1y/var-templ66.C -std=c++14, g++.dg/cpp1y/var-templ66.C -std=c++20, g++.dg/cpp1y/var-templ66.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: expected unqualified-id) | g++.dg/parse/constructor4.C -std=c++20, g++.dg/parse/constructor4.C -std=c++26, g++.dg/parse/constructor4.C -std=c++98 |
| 6 | wrong-line: missed error at LN (expected: implicitly deleted) | g++.dg/cpp0x/defaulted15.C -std=c++11, g++.dg/cpp0x/defaulted15.C -std=c++17, g++.dg/cpp0x/defaulted53.C -std=c++11 |
| 6 | wrong-line: missed error at LN (expected: match) | g++.dg/cpp2a/concepts-var-templ1a.C -std=c++20, g++.dg/cpp2a/concepts-var-templ1a.C -std=c++23, g++.dg/cpp2a/concepts-var-templ1a.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: private) | g++.dg/parse/access14.C -std=c++20, g++.dg/parse/access14.C -std=c++26, g++.dg/parse/access14.C -std=c++98 |
| 6 | wrong-line: missed error at LN (expected: redefinition) | g++.dg/cpp2a/concepts.C -std=c++20, g++.dg/cpp2a/concepts.C -std=c++23, g++.dg/cpp2a/concepts.C -std=c++26 |
| 6 | wrong-line: missed error at LN (expected: reference to label '…' defined inside of '…' attribute expression from outs…) | c-c++-common/pr119537-1.c -std=gnu++20, c-c++-common/pr119537-1.c -std=gnu++26, c-c++-common/pr119537-1.c -std=gnu++98 |
| 6 | wrong-line: missed error at LN (expected: within this context) | g++.dg/tc1/dr166.C -std=c++20, g++.dg/tc1/dr166.C -std=c++26, g++.dg/tc1/dr166.C -std=c++98 |
| 5 | wrong-line: missed error at LN (expected: expected) | g++.dg/cpp23/init-stmt2.C -std=c++23, g++.dg/cpp23/init-stmt2.C -std=c++26, g++.dg/parse/attr4.C -std=c++20 |

### Clusters: wrong-line, clang differs too (602 clusters, top 30)

| cases | cluster | examples |
|---:|---|---|
| 148 | wrong-line: missed error at LN (expected: ) | c-c++-common/int128-types-1.c -std=gnu++20, c-c++-common/int128-types-1.c -std=gnu++26, c-c++-common/int128-types-1.c -std=gnu++98 |
| 40 | wrong-line: missed error at LN (expected: no match) | g++.dg/concepts/fn5.C -std=gnu++17, g++.dg/cpp0x/inh-ctor5.C -std=c++11, g++.dg/cpp0x/inh-ctor5.C -std=c++20 |
| 39 | wrong-line: extra error at LN: error: no matching function for call to '…' | g++.dg/concepts/auto7.C -std=c++14, g++.dg/concepts/auto7.C -std=c++20, g++.dg/concepts/auto7.C -std=c++26 |
| 39 | wrong-line: extra error at LN: error: use of undeclared identifier '…' | g++.dg/conversion/const3.C -std=c++20, g++.dg/conversion/const3.C -std=c++26, g++.dg/conversion/const3.C -std=c++98 |
| 37 | wrong-line: extra error at LN: error: unknown type name '…' | g++.dg/concepts/auto7c.C -std=c++17, g++.dg/concepts/class6.C -std=gnu++17, g++.dg/concepts/expression2.C -std=gnu++17 |
| 33 | wrong-line: extra error at LN: error: expected '…' | c-c++-common/toplevel-asm-5.c -std=gnu++20, c-c++-common/toplevel-asm-5.c -std=gnu++26, c-c++-common/toplevel-asm-5.c -std=gnu++98 |
| 29 | wrong-line: missed error at LN (expected: expected) | c-c++-common/asmgoto-2.c -std=gnu++20, c-c++-common/asmgoto-2.c -std=gnu++26, c-c++-common/asmgoto-2.c -std=gnu++98 |
| 24 | wrong-line: missed error at LN (expected: jump to label) | g++.dg/cpp1z/constexpr-if4.C -std=gnu++11, g++.dg/cpp1z/constexpr-if4.C -std=gnu++20, g++.dg/cpp1z/constexpr-if4.C -std=gnu++26 |
| 23 | wrong-line: missed error at LN (expected: call to non-.constexpr. function) | g++.dg/cpp1y/constexpr-diag1.C -std=gnu++14, g++.dg/cpp1y/constexpr-diag1.C -std=gnu++20, g++.dg/cpp1y/constexpr-diag1.C -std=gnu++26 |
| 22 | wrong-line: missed error at LN (expected: not declared) | g++.dg/contracts/cpp26/contracts-nested-class1.C -std=c++26, g++.dg/cpp0x/noexcept71.C -std=c++11, g++.dg/cpp0x/noexcept71.C -std=c++20 |
| 21 | wrong-line: missed error at LN (expected: is not valid in an identifier) | c-c++-common/cpp/Winvalid-utf8-3.c -std=gnu++20, c-c++-common/cpp/Winvalid-utf8-3.c -std=gnu++26, c-c++-common/cpp/Winvalid-utf8-3.c -std=gnu++98 |
| 21 | wrong-line: missed error at LN (expected: modifying a const object) | g++.dg/cpp1y/constexpr-tracking-const1.C -std=c++14, g++.dg/cpp1y/constexpr-tracking-const1.C -std=c++20, g++.dg/cpp1y/constexpr-tracking-const1.C -std=c++26 |
| 19 | wrong-line: extra error at LN: error: constexpr variable '…' must be initialized by a constant expression | g++.dg/cpp0x/constexpr-ref13.C -std=c++11, g++.dg/cpp0x/constexpr-ref13.C -std=c++20, g++.dg/cpp0x/constexpr-ref13.C -std=c++26 |
| 18 | wrong-line: missed error at LN (expected: uninitialized) | g++.dg/cpp26/constexpr-new6.C -std=c++26, g++.dg/init/const10.C -std=c++11, g++.dg/init/const10.C -std=c++20 |
| 17 | wrong-line: missed error at LN (expected: deleted) | g++.dg/cpp0x/defaulted40.C -std=c++11, g++.dg/cpp0x/defaulted40.C -std=c++20, g++.dg/cpp0x/defaulted40.C -std=c++26 |
| 16 | wrong-line: missed error at LN (expected: reference to '…' is ambiguous) | g++.dg/cpp26/name-independent-decl10.C -std=gnu++11, g++.dg/cpp26/name-independent-decl10.C -std=gnu++14, g++.dg/cpp26/name-independent-decl10.C -std=gnu++23 |
| 15 | wrong-line: extra error at LN: error: no member named '…' in '…' | g++.dg/coroutines/coro1-missing-await-method.C -std=c++20, g++.dg/coroutines/coro1-missing-await-method.C -std=c++23, g++.dg/coroutines/coro1-missing-await-method.C -std=c++26 |
| 15 | wrong-line: missed error at LN (expected: '…' outside class declaration) | g++.dg/cpp0x/range-for42.C -std=c++11, g++.dg/cpp0x/range-for42.C -std=c++14, g++.dg/cpp0x/range-for42.C -std=c++20 |
| 15 | wrong-line: missed error at LN (expected: ambiguous) | g++.dg/concepts/equiv.C -std=gnu++17, g++.dg/concepts/equiv.C -std=gnu++20, g++.dg/concepts/equiv.C -std=gnu++26 |
| 15 | wrong-line: missed error at LN (expected: instantiating erroneous template) | g++.dg/parse/defarg18a.C -std=c++11, g++.dg/parse/defarg18a.C -std=c++20, g++.dg/parse/defarg18a.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: '…' has incomplete type) | g++.dg/cpp1y/pr79435.C -std=c++14, g++.dg/cpp1y/pr79435.C -std=c++20, g++.dg/cpp1y/pr79435.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: '…' to '…') | g++.dg/cpp1y/pr94066-2.C -std=c++14, g++.dg/cpp1y/pr94066-2.C -std=c++20, g++.dg/cpp1y/pr94066-2.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: accessing uninitialized member) | g++.dg/cpp0x/constexpr-48089.C -std=c++11, g++.dg/cpp0x/constexpr-48089.C -std=c++20, g++.dg/cpp0x/constexpr-48089.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: incomplete type) | g++.dg/ext/is_invocable3.C -std=c++11, g++.dg/ext/is_invocable3.C -std=c++20, g++.dg/ext/is_invocable3.C -std=c++26 |
| 12 | wrong-line: missed error at LN (expected: structured bindings only available with) | g++.dg/cpp0x/attr-carries_dependency2.C -std=c++11, g++.dg/cpp0x/attr-carries_dependency2.C -std=c++14, g++.dg/cpp0x/attr-likely1.C -std=c++11 |
| 12 | wrong-line: missed error at LN (expected: template parameter) | g++.dg/cpp0x/vt-34314.C -std=c++11, g++.dg/cpp0x/vt-34314.C -std=c++20, g++.dg/cpp0x/vt-34314.C -std=c++26 |
| 11 | wrong-line: missed error at LN (expected: is not a constant expression) | g++.dg/cpp0x/constexpr-50060.C -std=c++11, g++.dg/cpp1y/constexpr-89074-1.C -std=c++14, g++.dg/cpp1y/constexpr-89074-1.C -std=c++20 |
| 11 | wrong-line: missed error at LN (expected: use of deleted function) | g++.dg/cpp0x/lambda/lambda-ice2.C -std=c++11, g++.dg/cpp0x/lambda/lambda-ice2.C -std=c++17, g++.dg/cpp0x/pr62101.C -std=c++11 |
| 9 | wrong-line: extra error at LN: error: extraneous template parameter list in template specialization or out-of-line template definition | g++.dg/cpp0x/alias-decl-42.C -std=c++11, g++.dg/cpp0x/alias-decl-42.C -std=c++20, g++.dg/cpp0x/alias-decl-42.C -std=c++26 |
| 9 | wrong-line: missed error at LN (expected: N:literal operator template .int operator\"\"_udl\\(\\). has invalid parameter l…) | g++.dg/cpp2a/udlit-class-nttp-ctad-neg2.C -std=c++20, g++.dg/cpp2a/udlit-class-nttp-ctad-neg2.C -std=c++23, g++.dg/cpp2a/udlit-class-nttp-ctad-neg2.C -std=c++26 |

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

- kiln: `--c++17`, `--embed-directory`, `--embed-directory=<path>`, `--param case-values-threshold=1`, `--param case-values-threshold=4`, `--param cxx-max-namespaces-for-diagnostic-help=5`, `--param early-inlining-insns=0`, `--param early-inlining-insns=1`, `--param early-inlining-insns=100`, `--param early-inlining-insns=14`, `--param early-inlining-insns=3`, `--param ggc-min-expand=0`, `--param ggc-min-heapsize=0`, `--param ggc-min-heapsize=1024`, `--param hardcfr-max-blocks=9`, `--param hardcfr-max-inline-blocks=5`, `--param hash-table-verification-limit=10000`, `--param ipa-cp-eval-threshold=1`, `--param ipa-cp-eval-threshold=100`, `--param ipa-cp-eval-threshold=125`, `--param large-function-growth=60`, `--param large-function-insns=10`, `--param logical-op-non-short-circuit=0`, `--param logical-op-non-short-circuit=1`, `--param max-cse-insns=0`, `--param max-devirt-targets=1`, `--param max-early-inliner-iterations=1`, `--param max-early-inliner-iterations=10`, `--param max-early-inliner-iterations=3`, `--param max-early-inliner-iterations=5`, `--param max-inline-insns-single=200`, `--param max-inline-insns-single=4`, `--param max-inline-insns-size=1`, `--param max-unroll-times=32`, `--param max-unswitch-insns=5`, `--param uninlined-thunk-insns=1000`, `--param unroll-jam-min-percent=0`, `--param vect-epilogues-nomask=1`, `--param=early-inlining-insns=111`, `--param=early-inlining-insns=176`, `--param=early-inlining-insns=192`, `--param=early-inlining-insns=196`, `--param=early-inlining-insns=82`, `--param=hash-table-verification-limit=1000`, `--param=hash-table-verification-limit=10000`, `--param=hash-table-verification-limit=10000000`, `--param=ipa-cp-eval-threshold=0`, `--param=ipa-cp-eval-threshold=1`, `--param=ipa-cp-max-recursive-depth=94`, `--param=large-stack-frame-growth=1`, `--param=logical-op-non-short-circuit=0`, `--param=max-cse-insns=0`, `--param=max-inline-insns-single=25`, `--param=max-stores-to-track=2`, `--param=modref-max-depth=1`, `--param=partial-inlining-entry-probability=100`, `--param=sccvn-max-alias-queries-per-access=0`, `-D`, `-H`, `-MQ` and 403 more
- gcc: `--c++17`, `--embed-directory`, `-D`, `-Wno-attributes=___::`, `-Wno-attributes=____::____`, `-Wno-attributes=c2::@tr`, `-Wno-attributes=c::____`, `-Wno-attributes=c@::attr`, `-fcf-protection`, `-fcf-protection=branch`, `-fcf-protection=full`, `-fcf-protection=return`, `-flifetime-dse=123456`, `-fpatchable-function-entry=1,65536`, `-fpatchable-function-entry=65536,1`, `-idirafter`, `-imacros`, `<path>`, `_GNU_SOURCE`
- clang: `--c++17`, `--embed-directory`, `-C`, `-D`, `-fabi-compat-version=10`, `-fabi-version=0`, `-fabi-version=10`, `-fabi-version=14`, `-fabi-version=15`, `-fabi-version=16`, `-fabi-version=18`, `-fabi-version=2`, `-fabi-version=20`, `-fabi-version=3`, `-fabi-version=4`, `-fabi-version=9`, `-fcf-protection`, `-fcf-protection=branch`, `-fcf-protection=full`, `-fcf-protection=return`, `-fchecking`, `-fchecking=2`, `-fcompare-debug`, `-fconcepts`, `-fconcepts-diagnostics-depth=2`, `-fconcepts-diagnostics-depth=3`, `-fconserve-stack`, `-fconstexpr-fp-except`, `-fconstexpr-loop-limit=5`, `-fconstexpr-loop-limit=98304`, `-fconstexpr-ops-limit=131072`, `-fcontract-checks-outlined`, `-fcontract-evaluation-semantic=enforce`, `-fcontract-evaluation-semantic=observe`, `-fcontracts`, `-fcontracts-client-check=all`, `-fcontracts-client-check=none`, `-fcontracts-client-check=pre`, `-fcontracts-definition-check=off`, `-fdelete-dead-exceptions`, `-fdiagnostics-format=sarif-file`, `-fdiagnostics-set-output=text:show-nesting=yes,show-nesting-locations=no`, `-fdiagnostics-show-caret`, `-fdump-ada-spec`, `-fdump-ipa-icf`, `-fdump-ipa-inline-all`, `-fdump-ipa-sra`, `-fdump-ipa-whole-program`, `-fdump-tree-dse1-details`, `-fdump-tree-eh`, `-fdump-tree-hardcfr`, `-fdump-tree-optimized`, `-fdump-tree-profile_estimate`, `-fdump-tree-unswitch-details`, `-fext-numeric-literals`, `-ffreestanding`, `-fhardcfr-check-noreturn-calls=always`, `-fhardcfr-check-noreturn-calls=never`, `-fhardcfr-check-noreturn-calls=no-xthrow`, `-fhardcfr-check-noreturn-calls=nothrow` and 38 more

<!-- gxx-dg:end -->
