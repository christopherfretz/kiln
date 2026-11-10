# spec/reflection-members.md — member, access, layout and annotation queries

Contract for the `Member`, `Access` and `Annot` op families of
spec/reflection.md §3.3 (P2996R13 [meta.reflection.member.queries],
[meta.reflection.access.context], [meta.reflection.access.queries],
[meta.reflection.layout]; P3394R4 [meta.reflection.annotation]). Code:
`crates/kiln_oven/src/meta/{member,access,annot,member_cache}.rs` (op
rows, cache), `crates/kiln_sema/src/meta/{member,access,annot}.rs`
(answers), `crates/kiln_sema/src/class/access.rs` (the access check).
GCC 16.1 is the reference compiler; tests compare verdicts with it.

## 1. Rows and status

| op | purity | result | status |
|---|---|---|---|
| `members_of(r, ctx)` | PointDependent | `vector<info>` | done |
| `bases_of(r, ctx)` | Stable | `vector<info>` | done |
| `static_data_members_of(r, ctx)` | Stable | `vector<info>` | done |
| `nonstatic_data_members_of(r, ctx)` | Stable | `vector<info>` | done |
| `subobjects_of(r, ctx)` | Stable | `vector<info>` | done |
| `enumerators_of(r)` | Stable | `vector<info>` | done |
| `parameters_of(r)` | Stable | `vector<info>` | done |
| `offset_of(r)` | Stable | `member_offset` | done |
| `size_of`, `alignment_of`, `bit_size_of` | Stable | `size_t` | done |
| `access_context::current()` | PointDependent | `access_context` | done |
| `access_context::unprivileged()`, `unchecked()` | — (header) | `access_context` | done |
| `access_context::via(cls)` | Stable (`__kiln_access_via`) | `access_context` | done |
| `current_function`, `current_class`, `current_namespace` | PointDependent | `info` | done |
| `is_accessible(r, ctx)` | Stable | `bool` | done |
| `has_inaccessible_{nonstatic_data_members,bases,subobjects}` | Stable | `bool` | done |
| `annotations_of(r)`, `annotations_of_with_type(r, t)` | PointDependent | `vector<info>` | done |
| `is_annotation(r)` | Stable | `bool` | done |

The retired spike ops `__kiln_members_count` / `__kiln_member_at` are
gone; `__kiln_define_member` (spec/reflection.md §6.3) stays.

## 2. Member lists and the cache

- **Order.** A class's members are its member declarations in
  declaration order, then the implicitly declared special members in
  the order default constructor, copy constructor, copy assignment, move
  constructor, move assignment, destructor (GCC 16.1's order; each only
  if [class.default.ctor], [class.copy.ctor], [class.copy.assign] and
  [class.dtor] declare it), then the implicit `operator==` of each
  explicitly defaulted member `operator<=>` ([class.compare.default]/5:
  same parameters, ref-qualifier, access and `constexpr`ness, returns
  `bool`, defaulted, not user-declared; declared at the class's `}`
  unless the class declares an `operator==`; GCC 16.1's position, after
  the special members; WP-1228). A friend `<=>`'s implicit `==` is a
  friend, so not a member. `members_of` declares them first (D4,
  [class.mem]/5 "implicitly-declared … as if"). Not members: friend
  declarations ([class.friend]/1), access-specifiers, using-declarations
  and -directives, static_assert-declarations, deduction guides,
  enumerators and parameters (reached by their own queries), and
  implicit declarations other than the special members and the
  implicit `operator==`s (the injected-class-name). An unnamed bit-field is a member but not a data
  member. Template specializations are not members; their templates
  are (`never_a_member`, `build_class`).
- **Namespaces** (`^^::` included; a namespace alias, of an alias or
  not, is dealiased first, [meta.reflection.member.queries]/1, so it
  lists the aliased namespace's members, the same reflections; GCC 16.1
  agrees; WP-1228): the members declared before the
  evaluation point, first declarations only, across every block of the
  namespace (reopenings, linkage specifications, export blocks), in
  the creation order of each entity's first declaration, GCC 16.1's
  `DECL_UID` order: a function first declared as a friend inside a class
  sits where that friend declaration is
  (`meta.reflection.member.queries/namespace-members-friend-order.cpp`).
  A namespace's list is collected incrementally (per-block cursors plus
  the global declaration count as the stamp, so an unchanged namespace
  walks nothing, and a grown one walks only new declarations) and then
  sorted by first declaration.
- **Classes and enumerations** must be complete at the evaluation point
  (a class template specialization is instantiated first). A member
  function whose trailing requires-clause is not satisfied (a member of a
  class template specialization redeclared from a constrained pattern
  member) is not members-of-representable and is left out (P2996R13
  [meta.reflection.member.queries]/2; GCC 16.1 agrees;
  `meta.reflection.member.queries/members-of-unsatisfied-constraints.cpp`). The list is
  built once and never changes.
- **Filters.** `static_data_members_of`, `nonstatic_data_members_of` and
  `subobjects_of` (bases then non-static data members) filter the same
  list by kind bit (`member_kind`). `bases_of` is its own list of
  `BaseSpec` reflections in base-specifier order.
- **Steady state.** A query copies the accepted entries into the Oven's
  reusable result buffer (`Oven::meta_infos`), which the `Vector`
  recipe materializes. A repeated query allocates nothing in Rust and
  one result vector in the evaluation (`tests/stress/meta-members-2000.cpp`:
  10,000 queries of a 2,006-member class, `allocs<=10000`).
- `enumerators_of` needs an enumeration with an enumerator list (not an
  opaque declaration); `parameters_of` takes a function or function
  type and returns `FnParam` reflections (an explicit object parameter
  counts, as in GCC 16.1).

## 3. Access

- An `access_context` is a pair (scope, designating class) of `info`.
  `current()` is the function, class or namespace the evaluation point
  is immediately in (a default argument is evaluated at its call:
  [meta.reflection.access.context]/5); `unprivileged()` is (`^^::`,
  null); `unchecked()` is (null, null); `via(cls)` replaces the
  designating class with `cls`, which must be null or a complete class
  (a union is a class, [class.pre]/1; GCC 16.1 rejects a union).
- `is_accessible(r, ctx)`: an entity that is not a class member or
  direct base is accessible. Otherwise the member's class (a base's
  derived class) must be complete. A null scope accepts; a designating
  class that is neither the member's class nor derived from it rejects;
  else sema's [class.access] check from the scope, naming class the
  designating class (default: the member's class). [class.protected]
  applies with the designating class as the object expression's class,
  so a protected member from a derived class's member needs
  `via(^^Derived)` (GCC 16.1 agrees).
- One access check: `Sema::member_accessible_in` /
  `base_accessible_in` over an `AccessScope` built from the scope
  (`set_access_scope`) — the same code ordinary name lookup uses, not a
  second implementation. A public member of the class itself skips the
  check (`member_kind::PUBLIC`).
- The list queries take an access context and keep only the accessible
  entries; `has_inaccessible_*` ask the same question of each unchecked
  entry of a complete class.

## 4. Layout

- `offset_of(r)` of a non-static data member or unnamed bit-field is its
  offset in its class; of a direct base its offset in a complete object
  of the derived class (a virtual base too). `bits` is below 8 except
  that it carries a bit-field's position within its byte.
- `size_of`, `alignment_of` and `bit_size_of` of a type, object, value,
  variable, non-static data member or direct base agree with `sizeof` /
  `alignof` (a variable's or member's `alignas` counts; a reference
  member measures as a pointer). `bit_size_of` of a bit-field is its
  width; `size_of` and `alignment_of` reject one.
- All of it reads the record layout `offsetof`/`sizeof` use
  (`kiln_abi::ensure_layout`); `tests/conformance/meta.reflection.layout/`
  checks the `tests/layout` classes both ways.

## 5. Annotations

- `annotations_of(r)` of a type, type alias (its own annotations, not
  its target's), variable, function, function parameter, namespace,
  enumerator, direct base or non-static data member: every
  declaration's annotations, first declaration first, each in source
  order (spec/annotations.md §4). A function parameter's are those of
  the parameter in every declaration of the function. A lazily
  instantiated specialization's are substituted first
  (`require_annotations`); a class template specialization is
  instantiated first.
- A namespace alias is a namespace for `annotations_of` (`is_namespace`
  is true of it) with its own annotations, not its target's; a
  namespace-alias-definition admits none, so `annotations_of` and
  `annotations_of_with_type` of an alias are always empty, even when
  the aliased namespace is annotated (GCC 16.1 agrees; WP-573).
- One annotation written on a declaration with several declarators is
  one annotation of each entity (`[[=1]] int x, y;`: the same
  reflection; [dcl.attr.annotation]/3). Sema maps each annotation
  attribute to the `AnnId` it produced.
- `annotations_of_with_type(r, t)` keeps the annotations whose type is
  `remove_const(t)` (an annotation's type is cv-unqualified, so a
  volatile `t` matches none); `t` must be a complete type or alias.
- `is_annotation(r)` is true of exactly `Reflectee::Annotation`.

## 6. Failures

Each throws `std::meta::exception` (spec/reflection.md §8.2) whose
`what()` is GCC 16.1's text, e.g. "neither complete class type nor
namespace", "not a complete class type", "incomplete parent class",
"reflection unsuitable for offset_of", "reflection not suitable for
size_of", "reflection with incomplete type in size_of", "via argument
other than null or complete class type reflection", "current scope does
not represent a function". The negative files in the five test
directories list every one.

## 7. Known gaps

1. `^^C::priv` names a private member from outside without an error
   (GCC rejects); the operand check belongs to the reflect operand
   (`crates/kiln_sema/src/reflect_operand.rs`).
2. `constexpr auto v = annotations_of(r)[0];` is rejected (the
   temporary vector escapes): P3032R2's manifestly-constant-evaluated
   immediate context is not implemented; `static_assert` forms work.
3. The `Vector` recipe stores the result element by element
   (`crates/kiln_oven/src/meta/materialize.rs`), which dominates a large
   query's cost; a bulk store would remove it.
4. A base-specifier's annotations are not recorded
   (spec/annotations.md §9 item 1), so `annotations_of` of a base is empty.
