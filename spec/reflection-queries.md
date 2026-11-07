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
- **`constant_of(r)`** gives `Value(cv)` with `cv = int_const(canonical
  type, value)`. A constant of an integral or enumeration type from an
  enumerator, a template argument value or a variable usable in constant
  expressions is supported: a `constexpr` one, or a constant-initialized
  `const` non-volatile integral one. If the value was not yet evaluated,
  it is evaluated in a lent Oven.
- **`operator_of(r)`** gives the `operators` enumerator as an integer.
  The numbering is GCC 16.1's `[meta.reflection.operators]` order (see
  `kiln_meta_queries.h`).
- **`symbol_of(op)` / `u8symbol_of(op)`** give a frozen NUL-terminated
  `const char*` / `const char8_t*` in the same way `name_of` does (the `x`
  encoding). The switch to `string_view` waits for WP-475's
  materialization.
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
  change it; a complete one cannot (WP-795).
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
    spec/reflection.md §3.3 follows the table.

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
parity is out of scope. The negative tests are `entity-neg.cpp` and
`operators-parameters-neg.cpp`.

| Op | Fails when | Note |
|---|---|---|
| `type_of` | `r` has no type ([meta.reflection.queries] *has-type*) | `'X' has no type` |
| `parent_of` | `r` is null, the global namespace, a type without a declaration, a value or object, or a function or variable with C language linkage (`has_parent` false) | `reflection does not represent an entity with parent` |
| `object_of` | `r` is not a variable of static storage duration | `reflection does not represent an object with static storage duration, or a reference to such an object` |
| `constant_of` | `r` is not a value, enumerator, object or variable | `reflection does not represent an annotation or a valid argument to a splice-expression` |
| `constant_of` | the entity has no type | `reflection does not have a type` |
| `constant_of` | the variable is not usable in constant expressions | `the value of 'v' is not usable in a constant expression` |
| `operator_of` | `r` is not an operator function (template) | `reflection does not represent an operator function or operator function template` |
| `symbol_of`, `u8symbol_of` | `op` is not an `operators` enumerator | `operators argument is not a valid operator` |
| `return_type_of` | `r` is not a function or function type | `reflection does not represent a function or function type with a return type` |
| `variable_of` | `r` is not a parameter of the function being evaluated | `reflection does not represent parameter of current function` |

**Not implemented yet.** These fail with a `kiln:` note:
- `object_of` of a reference variable;
- `constant_of` of a class, array, floating or reference variable, or of
  an `Object` (WP-479).

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
- **`is_data_member_spec` is always false** until WP-480 builds
  `DataMemberSpec`.
- **`is_annotation`** is true for `Reflectee::Annotation`. Nothing
  builds one until WP-481.
- **`is_base` is false** for every reflection that exists today. Before
  `bases_of` (WP-478), nothing yields a `BaseSpec`.

## 5. Deviations from GCC 16.1 (kiln follows the IS)
- `has_linkage(^^NA)` for a namespace alias: GCC says true.
- `is_namespace_member(^^int)`: GCC says true.
- A namespace-scope lambda's closure type: GCC gives it linkage.

## 6. Status
Every row in §1–§4 is implemented. `parameters_of` belongs to WP-478,
and `template_of` / `template_arguments_of` belong to WP-482. The
positive tests in `parameters-p3096.cpp` that need `parameters_of` are
kept disabled until it exists. The IS example lines that need
`bases_of`, `is_union_type`, references or class constants are disabled
in the same way.
