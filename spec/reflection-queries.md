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
- **`has_ellipsis_parameter`** (P3096) is the row `is_vararg_function`,
  which is GCC 16.1's name. `<meta>` (WP-484) spells both.

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
  whose type is still an array of unknown bound (§1, WP-795).
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
`operators-parameters-neg.cpp` and `constant-of-neg.cpp`.

| Op | Fails when | Note |
|---|---|---|
| `type_of` | `r` has no type ([meta.reflection.queries] *has-type*) | `'X' has no type` |
| `parent_of` | `r` is null, the global namespace, a type without a declaration, a value or object, or a function or variable with C language linkage (`has_parent` false) | `reflection does not represent an entity with parent` |
| `object_of` | `r` is not a variable of static storage duration | `reflection does not represent an object with static storage duration, or a reference to such an object` |
| `constant_of` | `r` is not a value, annotation, enumerator, object, variable, structured binding or function (a non-static member, a type, a namespace) | `reflection does not represent an annotation or a valid argument to a splice-expression` |
| `constant_of` | the entity has no type | `reflection does not have a type` |
| `constant_of` | the variable, binding or (sub)object's complete object is not usable in constant expressions | `the value of 'v' is not usable in a constant expression` |
| `constant_of` | the value's type is not structural (a `mutable` or non-public member) | `'T' is not a structural type` |
| `constant_of` | the value is not a valid constant template argument (a pointer to a string literal) | `the value refers to a string literal, which is not a valid constant template argument` |
| `operator_of` | `r` is not an operator function (template) | `reflection does not represent an operator function or operator function template` |
| `symbol_of`, `u8symbol_of` | `op` is not an `operators` enumerator | `operators argument is not a valid operator` |
| `return_type_of` | `r` is not a function or function type | `reflection does not represent a function or function type with a return type` |
| `variable_of` | `r` is not a parameter of the function being evaluated | `reflection does not represent parameter of current function` |

**Not implemented yet.** These fail with a `kiln:` note:
- `constant_of` of a subobject the Oven cannot read: a `_BitInt` wider
  than 128 bits (`constant_of an object of type 'T' is not implemented
  yet`).
- `constant_of` of an explicit-object member function: the
  "not a valid argument to a splice-expression" note, because only
  static member functions are handled yet.

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
- No longer a deviation: a closure type at namespace or class scope has
  no linkage in GCC 16.1 either (`linkage-storage.cpp`).

## 6. Status
Every row in §1–§4 is implemented, apart from the §3 gaps.
`parameters_of` (WP-478) and `template_of` / `template_arguments_of`
(WP-482) are specified elsewhere. No test in
`meta.reflection.queries/` is disabled (WP-503 enabled the
`parameters-p3096.cpp`, `parent-dealias.cpp` and `values-objects.cpp`
blocks).
