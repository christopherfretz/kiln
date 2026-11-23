# spec/reflection-queries.md — the query family (WP-476)

The contract for `[meta.reflection.queries]` (P2996R13) and the P3096
parameter queries. It covers only the `QueryOp` rows tagged `476` in
`crates/kiln_oven/src/meta/query.rs`, plus `is_annotation` and
`is_data_member_spec`. Purity classes, memoization and `MetaError` are
spec/reflection.md §3.3, §3.6 and §8.1. The `Reflectee` set is
spec/reflection-repr.md. The implementation is
`crates/kiln_sema/src/meta/query/`. The tests are
`tests/conformance/meta.reflection.queries/`, and the test-side
declarations are in `tests/support/kiln_meta_queries.h`.

## 1. Results
- **Predicates** (`is_*`, `has_*`) answer `false` for every reflection
  they do not apply to, the null reflection included. None of them fails.
- **`info` results** are interned reflections. An `info` argument that is
  not a reflection id is `MetaError::invalid`.
- **`object_of(r)`** gives `Object(cv)`. Here `cv` is the interned
  pointer `Scalar::Pointer { base: Decl(first declaration), offset 0 }`,
  so `object_of(^^v) == object_of(^^v)` for every redeclaration of `v`.
  The pointer type is to the cv-unqualified object type (one reflection
  per object; `type_of` restores `const`). For a reference `R` usable in
  constant expressions, `cv` is the pointer value of `R`'s initializer,
  re-interned the same way: a variable, a template parameter object, a
  string literal object, a `std::type_info` object or a lifetime-extended
  temporary. A reference that is not usable in constant expressions fails
  with `reference 'R' is not usable in constant expressions` (WP-879).
- **`constant_of(r)`** ([meta.reflection.queries]; WP-503). With `R`
  the splice of `r`, it is `reflect_constant_array(R)` for an array,
  `reflect_function(R)` for a function and `reflect_constant(R)` for
  anything else ([meta.reflection.result], P3491R3). An annotation gives
  its underlying constant. All of these go through one builder,
  `Sema::constant_reflection` (`kiln_sema/src/meta/value.rs`), which
  `reflect_constant` and annotations share:
  - A scalar (integral, enumeration, floating-point, pointer, pointer
    to member, `info`) gives `Value(cv)`.
  - A class value gives the `Object` of its template parameter object
    (`const T`).
  - An array value gives the `Object` of the `reflect_constant_array`
    object, `PtrBase::Const` of the elements' image typed
    `E[N]` (cv-unqualified `E`).
  - Before interning, an aggregate image is retyped cv-unqualified, so
    one value is one object whatever spelling of its type froze it:
    `constant_of(^^arr) == reflect_constant_array(arr)`.
  - The value must be structural (`'T' is not a structural type`), and
    a valid constant template argument (`check_tparg`: no pointer to a
    string literal, temporary or subobject that is not allowed).

  The value is found from the reflectee:
  - A **`Value`** is its own constant.
  - An **enumerator** gives its value.
  - A **variable usable in constant expressions** (`constexpr`, or a
    constant-initialized `const` non-volatile integral) gives its
    initializer value, evaluated in a lent Oven if not yet evaluated.
    A reference variable gives the constant of the object it refers to.
  - A **structured binding** gives the constant of its tuple-like
    variable, or else of its binding expression folded over a usable
    decomposition.
  - A **function**, and a static member function, gives `r` itself
    (`reflect_function`).
  - An **`Object`** gives:
    - `r` itself for a whole template parameter object or a whole
      `PtrBase::Const` object (they are their own constants).
    - The variable's value for a whole variable usable in constant
      expressions.
    - For any other (sub)object, the value read from the Oven heap:
      `Oven::meta_freeze_at` (`kiln_oven/src/meta/value.rs`) thaws
      the pointer into the calling frame's registers above
      `MetaCtx::top`, loads the scalar or freezes the aggregate at that
      address and gives its constant. The complete object must be a
      variable usable in constant expressions, a template parameter
      object or a `Const` object, or it fails with `the value of 'v' is
      not usable in a constant expression`.
- **`operator_of(r)`** gives the `operators` enumerator as an integer.
  The numbering is GCC 16.1's `[meta.reflection.operators]` order (see
  `kiln_meta_queries.h`).
- **`symbol_of(op)` / `u8symbol_of(op)`** give a `std::string_view` /
  `std::u8string_view` of the operator's spelling, made the same way as
  `identifier_of` (spec/reflection.md §3.5, `MetaOut::Str`). The view's
  data is NUL-terminated (`operators.cpp`). The callee's declared return
  type picks the result shape: a record gives the view, a pointer the
  frozen string.
- **`variable_of(r)`** gives the `ParmVar` of the function definition
  whose body is being evaluated, for a `FnParam` of that function.
- **`type_of(r)`** of an annotation is the type of its underlying
  constant, `const T` for a class `T` (spec/reflection.md §7; WP-571).
  `is_const` / `is_volatile` inspect that type.
- **`type_of(r)`** of a variable (or static data member) is the type its
  latest declaration so far gives it, though the reflection designates
  the first: a later declaration can complete an array of unknown bound
  ([basic.types.general]/6, [dcl.array]/8), so after
  `struct S { static const char k[]; };` and
  `inline const char S::k[] = "SAFE";` it is `const char[5]`, whether `r`
  came from `^^S::k` or from `members_of`, as `decltype(S::k)` is and as
  GCC 16.1 answers; before the definition it is `const char[]`.
  `size_of` measures the same type. An answer that is still an array of
  unknown bound raises `run.impure` (§2), since a later declaration can
  change it; a complete one cannot (WP-795). A static data member of a
  class template specialization still of unknown bound has its
  definition instantiated by `type_of` first ([temp.inst]/4), so
  `template<class T> T C<T>::v[] = {1, 2, 3};` gives
  `type_of(^^C<int>::v) == ^^int[3]`, as GCC 16.1 answers; before the
  out-of-class definition is seen it stays `int[]`
  (`meta.reflection.queries/type-of-static-member-unknown-bound.cpp`;
  WP-796 via WP-879). A variable template specialization of unknown
  bound is not completed yet (nor is `sizeof` of it:
  `temp.inst/p4-var-template-unknown-bound.cpp`, xfail).
- **`return_type_of(r)`** gives the declared return type of a function or
  function type, canonical like `type_of`.
- **Structured bindings and enumerators** (WP-1537): a structured
  binding is not *has-type*, so `type_of` of one fails; an enumerator
  reflected inside its enum-specifier has the type of its initializing
  value ([dcl.enum]/5), the enumeration after the closing brace.
  `is_final` of a class being defined reads the class-key's `final`.
- **Undeduced placeholder types** ([meta.reflection.queries] *has-type*,
  [meta.reflection.member.queries]/3; WP-1243). A function whose type
  still contains an undeduced placeholder (`auto spin();` before any
  definition, `static auto sv();`, `auto f();` at namespace scope) has
  no type: `type_of` and `return_type_of` fail (§3), and `is_const`,
  `is_volatile`, `is_lvalue_reference_qualified` and
  `is_rvalue_reference_qualified` answer false, since they read
  `type_of(r)` only when *has-type* holds. `parameters_of`,
  `is_noexcept` and the other function queries have no such condition
  and answer as usual. Such a function is not members-of-representable,
  so `members_of` (and `static_data_members_of` for a variable) omits it
  rather than report a member whose type cannot be asked; this is the
  DIS rule and GCC 16.1's answer. Once a definition deduces the type
  (`auto Gizmo::spin() { return 2L; }` before the query point) the
  member is reported and typed (`type_of == ^^long()`). A member
  function of a class template specialization whose definition is not
  instantiated stays undeduced: reflecting it or listing members
  instantiates nothing, as in GCC 16.1 (`type_of(^^S<int>::f)` fails for
  `auto f() { return T{}; }`). A variable declared `auto` is undeduced
  only inside its own initializer, where `^^v` is ill-formed
  ([dcl.spec.auto.general]/11, as `auto v = v;`; GCC 16.1 agrees); its
  type is deduced from the initializer's type before the initializer is
  evaluated, so `members_of` called from inside the initializer reports
  it, as in GCC 16.1. Tests:
  `meta.reflection.member.queries/members-of-undeduced-placeholder.cpp`,
  `expr.reflect/reflect-auto-var-own-initializer-neg.cpp`.
- **`is_vararg_function`** is the working draft's name ([meta.syn],
  [meta.reflection.queries]) for P3096's `has_ellipsis_parameter`; GCC
  16.1 uses it too. kiln has no row for the paper's old name.

## 2. Purity and cost
- **Stable rows.** Every row is `Stable` except four. A `Stable` row
  leaves `run.impure` alone, so a consteval function built only from
  `Stable` queries is memoized by the Oven's call memo (§3.6).
  Memoization happens in the Oven's consteval-call memo, keyed by the
  calling function and its `info` argument bits. Sema keeps no
  per-`(op, ReflectionId)` cache of its own: each query is O(1) in the
  reflection, apart from a walk over contexts or redeclarations, and
  allocates nothing.
- **PointDependent rows.**
  - `is_complete_type` and `is_enumerable_type`: completion happens later
    in the TU.
  - `has_default_argument` and `is_defaulted`: a later redeclaration
    adds a default argument or a `= default`.
  - `variable_of`: the answer depends on which function is being
    evaluated. The oven table classes it `PointDependent`, and
    spec/reflection.md §3.3 follows the table. Decided in WP-503 to keep
    it: [meta.reflection.queries] lets `variable_of(r)` answer only
    inside the function whose parameter `r` is. So the same call
    (same callee, same `info` bits) answers differently in different
    function bodies, and a call memo keyed only on the arguments would
    be wrong.

  These rows raise `run.impure`, so a caller of one is never memoized.
  `type_of` is `Stable` but raises `run.impure` itself for a variable
  whose type is still an array of unknown bound (§1, WP-795). It, and
  `return_type_of`, `is_const`, `is_volatile` and the two
  ref-qualifier queries, also raise it for a function whose type still
  contains an undeduced placeholder (§1, WP-1243): a later definition
  deduces it, so the answer before must not be memoized for after.
- **Budget.** `meta.reflection.queries/oven-budget.cpp` pins the cost.
  It runs 10,000 calls per loop and checks the report counters:

  | Loop | `allocs` | `memo_hits` |
  |---|---|---|
  | `Stable` queries over 4 reflections | 0 | ≥ 9,996 |
  | a `PointDependent` query | 0 | 0 |
  | direct intrinsic calls | 0 | 0 (intrinsics are not memoized) |

## 3. Failures
A failing query is not a constant expression. Its `MetaError` note
follows the shape of GCC 16.1's `std::meta::exception::what()`. Wording
parity is out of scope. The negative tests are `entity-neg.cpp`,
`operators-parameters-neg.cpp`, `constant-of-neg.cpp` and
`constant-of-explicit-object-neg.cpp`.

| Op | Fails when | Note |
|---|---|---|
| `type_of` | `r` has no type ([meta.reflection.queries] *has-type*; also a function whose type contains an undeduced placeholder, §1) | `'X' has no type` |
| `parent_of` | `r` is null, the global namespace, a type without a declaration, a value or object, or a function or variable with C language linkage (`has_parent` false) | `reflection does not represent an entity with parent` |
| `object_of` | `r` is not a variable of static storage duration | `reflection does not represent an object with static storage duration, or a reference to such an object` |
| `constant_of` | `r` is not a value, annotation, enumerator, object, variable, structured binding or function a splice may designate alone (a non-static data member, an implicit object member function, a template without arguments, a type, a namespace) | `reflection does not represent an annotation or a valid argument to a splice-expression` |
| `constant_of` | the entity has no type | `reflection does not have a type` |
| `constant_of` | the variable, binding or (sub)object's complete object is not usable in constant expressions | `the value of 'v' is not usable in a constant expression` |
| `constant_of` | the value's type is not structural (a `mutable` or non-public member) | `'T' is not a structural type` |
| `constant_of` | the value is not a valid constant template argument (a pointer to a string literal) | `the value refers to a string literal, which is not a valid constant template argument` |
| `operator_of` | `r` is not an operator function (template) | `reflection does not represent an operator function or operator function template` |
| `symbol_of`, `u8symbol_of` | `op` is not an `operators` enumerator | `operators argument is not a valid operator` |
| `return_type_of` | `r` is not a function or function type, or is a function whose type contains an undeduced placeholder (§1) | `reflection does not represent a function or function type with a return type` |
| `variable_of` | `r` is not a parameter of the function being evaluated | `reflection does not represent parameter of current function` |

`constant_of` of an object reads a value of every object type: a class
with members, a union, an array, a pointer to member, a reflection, a
closure, and a `_BitInt` wider than 128 bits (WP-1743;
`meta.reflection.queries/constant-of-objects.cpp`).

`constant_of` of a function is `reflect_function([: R :])`, the function
itself, for every function a splice-expression may designate on its own:
a non-member function, a static member function, and an explicit-object
member function ([dcl.fct]/6; a conversion function or operator too, and
a specialization of an explicit-object member function template), which
[expr.prim.id.general]/4 does not restrict to a member access, so
`&[: R :]` is an ordinary pointer to function ([expr.unary.op]/3) and
`extract<F*>` of the reflection gives it. An implicit object member
function, a constructor, a destructor and a template without arguments
fail with the splice note above (WP-1782;
`meta.reflection.queries/constant-of-explicit-object*.cpp`; GCC 16.1
agrees). The match over reflectee and declaration kinds is exhaustive,
so a new kind needs a decision.

## 4. Row semantics that need a word
- **Linkage** ([basic.link]) belongs to a name. A type alias, a
  namespace alias, a non-static data member, a scoped enumerator, a
  fundamental type, a closure type, an unnamed class without a typedef
  name for linkage, and a local entity all have none. A `Type` answers
  for its class or enumeration declaration.
- **Access** (`is_public` and its siblings) answers for a member. A
  member of an anonymous union answers for the field it names. A
  non-member answers false.
- **`is_namespace_member`** is false for a fundamental type.
- **Anonymous unions (WP-1482).** A member of an anonymous union is the
  union's member at namespace scope too ([class.union.anon]/1):
  `is_class_member` true, `is_namespace_member` false, `parent_of` the
  union. `members_of` of the namespace lists the union's unnamed
  variable and its class. `main` has C++ language linkage, so it has a
  parent; a function with internal linkage in `extern "C"` has no C
  language linkage of its name; an alias template has its namespace's
  linkage; a local of a consteval block has the block's closure call
  operator as parent. GCC 16.1 differs on these four
  (docs/status/deviations.md;
  `meta.reflection.queries/linkage-parent-draft.cpp`).
- **`parent_of` / `has_parent`** look through linkage specifications
  and `export` declarations ([dcl.link]/4): a class, typedef, alias,
  enumeration or namespace declared in an `extern "C"` or `extern "C++"`
  block has the enclosing namespace as parent. Only functions and
  variables have a language linkage ([dcl.link]/1), so only one with C
  language linkage (`has_c_language_linkage`, block or bare form) has
  no parent; GCC 16.1 agrees (WP-533,
  `meta.syn/parent-of-linkage-spec{,-neg}.cpp`).
- **`has_template_arguments`** is true for a class, function or variable
  template specialization, and for an alias template specialization
  that is kept as written.
- **`is_data_member_spec`** is true for `Reflectee::DataMemberSpec`
  (`data_member_spec`; `meta.reflection.define.aggregate/`).
- **`is_annotation`** is true for `Reflectee::Annotation`
  (`annotations_of`; `meta.reflection.annotation/`).
- **`is_base`** is true for a direct base relationship (`BaseSpec`,
  from `bases_of`). It has the access of its base-specifier, and it is
  neither a class nor a namespace member (`members.cpp`).

## 5. Deviations from GCC 16.1 (kiln follows the IS)
Rechecked against g++ 16.1 `-std=c++26 -freflection` in WP-503:
- `has_linkage(^^NA)` for a namespace alias: GCC says true
  (`linkage-storage.cpp`).
- `is_namespace_member(^^int)`: GCC says true (`members.cpp`).
- `constant_of` of an array: GCC gives a reflection of a fresh variable,
  not the content-interned `reflect_constant_array` object the IS
  requires. So in GCC:
  - `constant_of(^^arr) != reflect_constant_array(arr)`, and
    `constant_of(constant_of(^^arr))` differs from `constant_of(^^arr)`;
  - `constant_of` of a reference to a string literal array throws
    `reflect_constant failed`.

  `constant-of-values.cpp` lines 55, 60 and 89 fail under GCC.
- `members_of` of a namespace or class holding a deduction guide: GCC
  lists the guide (as `__dguide_<name>`); a deduction guide is not a
  members-of-representable member ([meta.reflection.member.queries]/3,
  [temp.deduct.guide]), so kiln leaves it out (WP-1490;
  `meta.reflection.member.queries/members-of-deduction-guide.cpp`, which
  therefore has no GCC oracle).
- `has_identifier` of a parameter of a function template specialization
  instantiated from a function parameter pack (`T... b`): GCC says false.
  [meta.reflection.names]/1 asks whether a declaration of the function
  or of the templated function it is a specialization of introduces a
  name for the corresponding parameter, and `T... b` names it `b`; the
  "instantiated from a function parameter pack" exclusion is in the
  variable bullet only. kiln says true, `identifier_of` is `b`
  (g++.dg/reflect `has_identifier1.C` line 211 fails under kiln).
- `extract` of a block-scope variable usable in constant expressions
  whose lifetime did not begin within the evaluation (a `constexpr` or
  constant-initialized `const int` local of an enclosing function, of a
  function the evaluation is not running, reached directly or through a
  template argument), WP-1521. By value, `extract` is
  `extract-value<U>(constant_of(r))` ([meta.reflection.extract]/12), and
  `constant_of` asks only that `[: R :]` be a valid splice-expression:
  the variable's constant, whatever its storage duration (kiln once
  applied the reference rule here and rejected it). By reference,
  *extract-ref* /5.3 accepts a variable usable in constant expressions
  too; the reference designates the variable's object, whose address is
  not a permitted result of a constexpr reference or pointer outside the
  function ([expr.const]; clang's "address of non-static constexpr
  variable" note). A local whose lifetime began in this evaluation is
  read live, or, once its block ended, fails (`extract`: the lifetime
  note; `constant_of`: "read of variable 'x' outside its lifetime",
  [meta.reflection.queries] `constant_of`'s example); see spec/oven.md
  "Reflected locals" for how the Oven tells them apart. A structured
  binding is extracted by value through `constant_of` as well (GCC 16.1
  rejects it; the text decides). GCC 16.1 agrees on the rest. Tests:
  `meta.reflection.extract/local-constant.cpp`, `local-constant-neg.cpp`,
  `local-constant-binding.cpp`.
- `extract` by value and `constant_of` of a structured binding, of an
  automatic variable and of a parameter whose lifetime began within the
  evaluation (WP-2161): `constant_of(r)` is `reflect_constant([: R :])`,
  and the splice reads the binding (a member or element of the
  decomposition, or a tuple-like holding variable's referent) or the
  variable where its live frame holds it ([expr.const]/10; the
  [meta.reflection.queries] example rejects only an object outside its
  lifetime), also from a function its reflection was passed down to;
  `extract<U>` by value is `extract-value<U>` of that. A reference's
  `constant_of` is its referent's value. A parameter's variable is a
  splice operand in its function (`constant_of` no longer rejects it as
  "not a valid argument to a splice-expression"). Read as the frame holds
  it at the call (never memoized). A binding or variable of a frame
  outside the evaluation (a non-constexpr function's, seen from an
  immediate invocation or a `static_assert` in it) or of a returned frame
  keeps the earlier failures. GCC 16.1 agrees on a binding, a local and a
  parameter of the running frame, and rejects the rest (another frame's
  variable, a tuple-like binding, a reference local's `constant_of`); the
  text decides. Not yet: a bit-field binding (WP-2191) and `^^` of a value
  pack in a pack expansion (WP-2190). Tests:
  `meta.reflection.extract/binding-local.cpp`, `binding-local-kiln.cpp`,
  `binding-local-neg.cpp`, `binding-bit-field.cpp` and `binding-pack.cpp`
  (xfail).
- `extract` of a reference variable whose lifetime began within the
  evaluation (a reference parameter or a reference local of a consteval
  function, reflected in its body), WP-1489. `^^r` there represents a
  variable ([basic.pre]/6: a reference declaration other than a
  non-static data member declares one; a parameter declaration is such
  a declaration), not a parameter reflection (`parameters_of` gives
  those, and `extract` of one is not constant: it is not a variable or
  object). [meta.reflection.extract] *extract-ref* is constant when `r`
  represents a variable of suitable type and that variable is usable in
  constant expressions *or its lifetime began within the core constant
  expression currently under evaluation*, and returns a reference to
  the object the variable refers to; *extract-value* reads that object
  under the same condition. A reference's lifetime begins when its
  initialization completes ([basic.life]/1), which for a parameter is
  at the call, inside the evaluation. So `consteval int f(int &r) {
  return extract<int>(^^r); }` called from a constant evaluation with a
  live `int` is a constant expression with that `int`'s value. GCC 16.1
  rejects it (`reflect_constant failed`). The text decides: kiln accepts
  it for a reference **parameter** (WP-1505): the caller bound the
  reference and the parameter's register holds the referent's address
  for the whole activation (`ParamInfo::is_ref`), so
  `Oven::meta_ref_param` finds it in the innermost live frame of the
  running evaluation with nothing added to a frame; once that frame has
  returned it is "not a variable of static storage duration, nor one
  whose lifetime began within the evaluation", and the type rules are
  those of a local (`extract<long &>` of an `int &`, or dropping a
  const, is a mismatch). Tests: `meta.reflection.extract/reference-param.cpp`,
  `reference-param-neg.cpp`. A reference **local** is accepted too
  (WP-1515): its register is reused once its block ends, so a reference
  whose reflection its function takes gets a frame object holding the
  referent's address, made with its block and ended with it
  (spec/oven.md "Reflected locals"; `Oven::meta_ref_local`); a function
  that reflects no reference local is compiled as before. A reflection
  that escaped the block (or the frame) is rejected with the same note
  and a "declared here" note at the variable. Tests:
  `meta.reflection.extract/reference-local.cpp`, `reference-local-neg.cpp`.
- No longer a deviation: a closure type at namespace or class scope has
  no linkage in GCC 16.1 either (`linkage-storage.cpp`).

## 6. Status
**[meta.syn] coverage (WP-1505).** Every `consteval` function the working
draft's [meta.syn] declares in `std::meta` has a row, under the
draft's name. Checked against the synopsis as of 2026-11 (237 names):
- Missing before WP-1505, added: `is_applicable_type`,
  `is_nothrow_applicable_type`, `apply_result` (P1317;
  spec/reflection-traits.md §5). libstdc++ 16.1's `<meta>` does not
  declare them (nor does it define `std::is_applicable`), so the test
  header `kiln_meta_traits.h` declares them; a later `<meta>` that
  declares them as intrinsics (`consteval bool is_applicable_type(info,
  info);`) reaches the rows by name.
- Under another name: none. P3096's `has_ellipsis_parameter` is
  `is_vararg_function` in the draft (§1).
- Declared by libstdc++ 16.1's `<meta>` as library code over the
  intrinsics, not rows (`gcc16_meta_names.txt`): `define_static_array`,
  `define_static_object`, `define_static_string`, `is_string_literal`
  and the `access_context` members.

Every row in §1–§4 is implemented, apart from the §3 gaps.
`parameters_of` (WP-478) and `template_of` / `template_arguments_of`
(WP-482) are specified elsewhere. No test in
`meta.reflection.queries/` is disabled (WP-503 enabled the
`parameters-p3096.cpp`, `parent-dealias.cpp` and `values-objects.cpp`
blocks).
