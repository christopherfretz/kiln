# spec/reflection-traits.md — the type-trait family (WP-477)

Normative contract for the [meta.reflection.traits] functions of P2996R13
(`std::meta::is_void_type` … `type_order`): the `Trait` op family of
spec/reflection.md §3.3, `kiln_oven/src/meta/traits.rs` (the rows) and
`kiln_sema/src/meta/traits.rs` (the answers). Test vehicle:
`tests/support/kiln_meta_traits.h` (the declarations; `<meta>` is WP-484).
Tests: `tests/conformance/meta.reflection.traits/`.

## 1. One evaluator

**Rule.** Every trait function is answered by the builtin trait engine
that answers `__is_X(T)` / `__X(T)` (`kiln_sema/src/templ/builtins/`), so
`is_X_type(^^T)` and `__is_X(T)` cannot diverge. A trait that exists in
only one place is added to the engine first and both consumers use it.

### 1.1 Quiet engine queries

- `Sema::eval_type_trait` (the boolean and binary traits),
  `apply_unary_transform` (`__remove_cv` … `__underlying_type`),
  `add_quals_transform` (`add_const` / `add_volatile` / `add_cv`:
  `TypeContext::qualify`, which leaves references and function types
  unchanged, [meta.trans.cv]), `array_rank` / `array_extent`,
  `is_meta_info_type` and `is_structural_type` are the entry points.
- `Sema::quiet_trait(f)` runs one of them with diagnostics off. Instead
  of a diagnostic, the engine records the first failure as a
  `TraitFailure`:
  - `Incomplete(T)`: a completeness requirement (§2.2) failed;
  - `BadOperand(T)`: an operand outside a transformation's domain
    (`make_signed(bool)`, `underlying_type(int)`);
  - `Unimplemented`: the engine row has no implementation yet.
- Completeness checks in quiet mode use `require_complete_silent`: they
  may instantiate a class template specialization (as the builtin does)
  but emit nothing.
- `std::meta::info` is a fundamental, scalar and structural type
  ([basic.fundamental]/17 as amended by P2996R13; [temp.param]/7), so
  `__is_scalar(info)`, `__is_fundamental(info)` and the reflection traits
  are true. It stays not trivially copyable, trivial or POD: a byte copy of
  an `info` is a typed copy (spec/reflection.md §2.5, decided), and
  `__builtin_bit_cast` relies on this to reject forging one.

### 1.2 Library delegation

The traits with no `__trait` builtin are answered by instantiating the
TU's own `std::` class template for the represented types
(`traits/library.rs`): `std::NAME<Args...>::value` for a boolean or size,
`std::NAME<Args...>::type` (canonical) for a type.

| Function | Class template |
|---|---|
| `is_swappable_with_type`, `is_swappable_type`, `is_nothrow_swappable_with_type`, `is_nothrow_swappable_type` | `std::is_swappable_with` etc. |
| `is_invocable_type`, `is_invocable_r_type`, `is_nothrow_invocable_type`, `is_nothrow_invocable_r_type` | `std::is_invocable` etc. |
| `invoke_result`, `common_type`, `common_reference`, `unwrap_reference`, `unwrap_ref_decay` | `std::invoke_result` etc. |
| `tuple_size`, `variant_size` | `std::tuple_size`, `std::variant_size` |
| `tuple_element(I, T)`, `variant_alternative(I, T)` | `std::tuple_element<I, T>`, `std::variant_alternative<I, T>` |

- A user specialization of the class template counts ([namespace.std]/2).
- The class template must be declared (`<type_traits>`, `<tuple>`,
  `<variant>`, `<functional>`): otherwise the call fails (§2.1).
- Engine builtins for these (no instantiation) are a follow-up.

### 1.3 Range forms

- The IS forms taking a `reflection_range` (`is_constructible_type(T, R&&)`,
  the invocable family, `invoke_result`, `common_type`, `common_reference`)
  are not yet intrinsics: their `"ix"` / `"iix"` / `"x"` rows fail with
  "the reflection_range form of 'F' is not implemented yet" (spec/reflection.md
  §3.5 range reading is WP-475's).
- Each has a `(const info* data, size_t n)` row (`…TypeN`, signature `"ixx"`
  / `"iixx"` / `"xx"`): the leading `info` operands, then `n` elements read
  from `data` with `Oven::meta_info_at` (the element type must be `info`).
  At most 2^16 elements; up to 8 need no heap buffer.
- `kiln_meta_traits.h` implements the IS signatures as templates
  (`R = std::initializer_list<info>`) in the non-inline namespace
  `std::meta::__kiln_range` (so they are not themselves intrinsics, §3.1),
  redeclared into `std::meta`. Each copies the range into a
  constant-evaluation buffer and calls the `(data, n)` row.
- `extent(info, unsigned i = 0)` is not a range form.

### 1.4 Operands

- An `info` operand represents a type ([meta.reflection.traits]/2: T is
  "the type or type alias represented by" it). A reflection of a typedef
  or alias declaration is its type. Anything else fails (§2.1).
- The operand is used canonically: an alias is the type it denotes.

## 2. Failures

### 2.1 Notes

Every failure is a `MetaError` with one note at the call
(spec/reflection.md §8.1); the call is not a constant expression. The
text is stable (tests pin it); its shape follows GCC 16.1, and wording
parity is out of scope.

| Failure | Note |
|---|---|
| Not a type (p2) | `'R' does not represent a type` |
| Incomplete operand (§2.2) | `invalid use of incomplete type 'T'` |
| `underlying_type` of a non-enum | `'T' is not an enumeration type` |
| `make_signed` / `make_unsigned` out of domain | `'make_signed' requires a non-bool integral or enumeration type, not 'T'` |
| Library template not declared (§1.2) | `'F' needs 'std::NAME', which is not declared (include <H>)` |
| Library template not defined for the operands | `'std::NAME' is not defined for these operands` |
| Library template has no result member | `'std::NAME' has no member 'value'` (or `'type'`) `for these operands` |
| A range too long | `'F': the range is too long` |
| A `reflection_range` row (§1.3) | `kiln: the reflection_range form of 'F' is not implemented yet; use the (const info*, size_t) overload (kiln_meta_traits.h)` |

- `std::meta::exception` (P3560) replaces the note when WP-475 lands.

### 2.2 Completeness

- An operand must be complete where [meta.unary.prop], [meta.rel] and
  [meta.trans] require it of the associated class template: the engine's
  per-trait rule (`table::completeness`, the same check `__is_X` makes).
- An incomplete operand makes the call not a constant subexpression
  ([meta.reflection.traits]/3.2: instantiating the associated
  specialization would have undefined behavior, [meta.rqmts]/5,
  [res.on.functions]/2.5), so kiln diagnoses it where GCC 16.1 returns
  a value (docs/status/deviations.md "GCC 16.1 reflection divergences").
- A failure is never cached: the Oven memoizes only successful calls
  (spec/reflection.md §3.6), so once the type is complete the same call
  succeeds (`failures-neg.cpp`).

### 2.3 Trials

- A query that may instantiate or evaluate is *heavy* and runs with the
  Oven lent back (spec/reflection.md §3.2): every library-delegated trait,
  `is_structural_type`, every multi-operand engine trait except the
  relations `is_same_type`, `is_layout_compatible_type` and the base-of
  family, the destructible family, `is_implicit_lifetime_type`, and any
  completeness-requiring trait, relation or `underlying_type` whose operand
  names a not-yet-instantiated class template specialization
  (`trait_is_light`). The rest are *light* and run in place.
- In a trial, or outside a constant evaluation that sema started, it
  fails instead (spec/reflection.md §4.3), with the note "'X' is not
  evaluated during a speculative evaluation (it may instantiate)" (a
  trial's notes are dropped; the note is there so that the refusal can
  never surface as a bare "not a constant expression", WP-538).
- The Transactional checkpoint `substitute` opens around an
  instantiation is not a trial (spec/reflection.md §4.3): a heavy trait
  asked from the initializer of a variable template specialization that
  `substitute(^^V, args)` instantiates answers as it does outside the
  substitution (`meta.reflection.substitute/substitute-var-template-traits.cpp`,
  `substitute-var-template-assign-destroy.cpp`,
  `substitute-memo-door-nested.cpp`), and fails with its own note
  (`substitute-var-template-traits-neg.cpp`). WP-538.

## 3. Performance

- Each call is one engine query: no `String` or `Vec` on success (strings
  are built only for a failure note; a range form of more than 8 elements
  uses one heap buffer), and no type sugar is built beyond the engine's
  own.
- Every row is `Stable`, so a consteval function reaching only traits is
  memoized. Budget test (`budget.cpp`): 10,000 calls of such a function
  are 9,999 `memo_hits` and 0 `allocs`; 10,000 direct calls in a loop are
  0 `allocs`.
- There is no sema-side answer cache: the Oven memo is the cache.

## 4. Results and the alias rule

- A boolean trait returns `bool`; `rank`, `extent`, `tuple_size` and
  `variant_size` return `size_t`.
- A transformation returns a reflection of the type `std::X_t<T>`
  denotes, canonical: never a type alias ([meta.reflection.traits]/5-6
  P2996R13: the result "represents the type denoted by" the associated
  member typedef; a type, not an alias, is returned). So
  `remove_const(^^const Alias) == ^^int` for `using Alias = int;`, and
  `common_type({^^A, ^^B})` is the type `std::common_type_t<A, B>` names.
  No transformation retains an alias.
- `^^` keeps a top-level alias (WP-472: `^^std::common_type_t<A, B>`
  reflects the alias), so a test compares a result with
  `is_same_type(r, ^^Alias)` (or `dealias`, WP-476), not `==`.

## 5. Contract table

Completeness: — none; **E** the element type (extents removed) complete
or `void`; **E'** as E, an unknown bound accepted; **C** a class or union
complete; **N** a non-union class complete; **A** each operand complete,
`void` or an array of unknown bound ([meta.unary.prop]); **L** the
library template's own requirements (§1.2). Result: **b** `bool`, **z**
`size_t`, **i** `info`.

| Function | Engine | Compl. | Result |
|---|---|---|---|
| `is_void_type` … `is_function_type` (primary categories), `is_reference_type` … `is_member_pointer_type` (composite), `is_const_type`, `is_volatile_type`, `is_signed_type`, `is_unsigned_type`, `is_bounded_array_type`, `is_unbounded_array_type`, `is_scoped_enum_type` | `__is_X` | — | b |
| `is_reflection_type` | `is_meta_info_type` | — | b |
| `is_trivially_copyable_type`, `is_standard_layout_type`, `has_virtual_destructor`, `has_unique_object_representations` | `__is_X` / `__has_X` | E | b |
| `is_empty_type`, `is_polymorphic_type`, `is_abstract_type` | `__is_X` | N | b |
| `is_final_type`, `is_aggregate_type`, `is_implicit_lifetime_type` | `__is_X` | C | b |
| `is_structural_type` | `is_structural_type` ([temp.param]/7) | C | b |
| `is_destructible_type`, `is_trivially_destructible_type`, `is_nothrow_destructible_type` | `__is_X` | E' | b |
| `is_constructible_type(T, (data, n))`, `is_trivially_…`, `is_nothrow_…` | `__is_{,trivially_,nothrow_}constructible(T, Args…)` | A | b |
| `is_{,trivially_,nothrow_}default_constructible_type(T)` | `__is_…constructible(T)` | A | b |
| `is_{,trivially_,nothrow_}copy_constructible_type(T)` | `__is_…constructible(T, __add_lvalue_reference(const T))` | A | b |
| `is_{,trivially_,nothrow_}move_constructible_type(T)` | `__is_…constructible(T, __add_rvalue_reference(T))` | A | b |
| `is_{,trivially_,nothrow_}assignable_type(T, U)` | `__is_…assignable(T, U)` | A | b |
| `is_{,trivially_,nothrow_}copy_assignable_type(T)` | `__is_…assignable(T&, const T&)` (through the engine's transforms) | A | b |
| `is_{,trivially_,nothrow_}move_assignable_type(T)` | `__is_…assignable(T&, T&&)` | A | b |
| `is_{,nothrow_}swappable{_with,}_type` | `std::is_{,nothrow_}swappable{_with,}` | L | b |
| `reference_constructs_from_temporary`, `reference_converts_from_temporary` | `__reference_X_from_temporary` | A | b |
| `rank`, `extent(T, i)` | `__array_rank`, `__array_extent` | — | z |
| `is_same_type` | `__is_same` | — | b |
| `is_base_of_type`, `is_virtual_base_of_type`, `is_pointer_interconvertible_base_of_type` | `__is_X` | N (derived) | b |
| `is_convertible_type(From, To)`, `is_nothrow_convertible_type` | `__is_convertible`, `__is_nothrow_convertible` | A | b |
| `is_layout_compatible_type` | `__is_layout_compatible` | A | b |
| `is_{,nothrow_}invocable{,_r}_type(…, (data, n))` | `std::is_{,nothrow_}invocable{,_r}` | L | b |
| `remove_const`, `remove_volatile`, `remove_cv`, `remove_reference`, `add_lvalue_reference`, `add_rvalue_reference`, `remove_extent`, `remove_all_extents`, `remove_pointer`, `add_pointer`, `remove_cvref`, `decay` | `__X` (`apply_unary_transform`) | — | i |
| `add_const`, `add_volatile`, `add_cv` | `add_quals_transform` | — | i |
| `make_signed`, `make_unsigned` | `__make_signed`, `__make_unsigned` | C (an enum) | i |
| `underlying_type` | `__underlying_type` | C (an enum) | i |
| `invoke_result((data, n))`, `common_type((data, n))`, `common_reference((data, n))`, `unwrap_reference`, `unwrap_ref_decay` | `std::X<…>::type` | L | i |
| `tuple_size`, `variant_size` | `std::X<T>::value` | L | z |
| `tuple_element(I, T)`, `variant_alternative(I, T)` | `std::X<I, T>::type` | L | i |
| `type_order` | GCC 16.1's order: `strcmp` of the canonical types' Itanium manglings (gcc/cp/method.cc `type_order_value`; `meta.reflection.traits/type-order-mangling.cpp`), with `std::meta::info` as GCC's `Dm` whatever `-fkiln-reflection-mangling` says (WP-909), interned id on a tie; the `std::strong_ordering` static member of the result type | — | i |

An operand may be a cv- or ref-qualified function type (`^^FnC` for
`using FnC = int() const;`, or `^^int() const`): the operand of `^^`
declares nothing, so [dcl.fct]/6 does not restrict it (`DeclSite::Reflect`;
GCC 16.1 agrees; `reflect-qualified-function-type{,-neg}.cpp`).

All 121 rows of `kiln_oven/src/meta/traits.rs` (111 IS signatures and the
10 `(data, n)` rows of §1.3) are `Stable` and owned by WP-477.

## 6. Tests

`tests/conformance/meta.reflection.traits/`: `categories.cpp`,
`properties.cpp`, `operations.cpp`, `queries.cpp`, `relations.cpp`,
`transformations.cpp`, `library.cpp`, `tuple-variant.cpp`,
`failures-neg.cpp`, `budget.cpp`, `reflect-qualified-function-type.cpp`
and its `-neg` (`oracle: gcc`), and the generated `traits-zoo.cpp`
(`is_X_type(^^T) == std::is_X_v<T>` over a type zoo, kiln only) with
`traits-zoo-rhs.cpp` (`std::is_X_v<T> == V`, V from clang 22,
`min-oracle: 22`), both written by `gen/gen_zoo.py`.
