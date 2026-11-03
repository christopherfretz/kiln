# spec/reflection.md — C++26 reflection (M6)

Normative contract for static reflection in kiln: the `std::meta::info`
value, the `^^` operator, splices, consteval blocks, the `std::meta`
library intrinsics and the Oven runtime that evaluates them, annotations,
`define_aggregate`, and the error model. It supersedes spec/ast.md §10,
spec/sema.md §8a and spec/oven.md "Reflection runtime", which are now
pointers here. The grammar stays in spec/parse.md §5.2; the tokens in
spec/lex.md Phase 3; the builtin type in spec/types.md.

**Conventions.** Each rule is either *implemented* (a test below checks
it) or carries the work package that implements it (`→ WP-47x`). Where
the IS and the WP-267 spike differ, the IS rule is the rule and the
spike's behaviour is recorded only as the current state of an open WP.
Section references in brackets are to the C++26 IS.

## 1. Scope and conformance

1. **Authoritative text** (D3). The C++26 IS reflection wording as GCC
   16.1 implements it (`__cpp_impl_reflection 202603L`): P2996R13 and the
   papers adopted with it, which are P3096 (parameters), P3289 (consteval
   blocks), P3293 (base-subobject splices), P3394 (annotations), P3491
   (`define_static_*`), P3547 (access contexts) and P3560
   (`std::meta::exception`). Names and signatures follow GCC 16.1's
   `<meta>` where a paper revision differs. A difference found later is
   recorded in this section, with the GCC spelling winning.
2. **Reference compiler** (D1). GCC 16.1 (`g++-16 -std=c++26
   -freflection`) is the behavioural oracle. The differential runs off the
   merge queue (→ WP-483). Tests stay kiln goldens (`oracle: none`, §9).
3. **Library vehicle** (D2). The production target is libstdc++ 16.1's
   `<meta>` (`__cpp_lib_reflection 202603L`), consumed by → WP-484. Until
   then the test vehicle is `tests/support/kiln_meta.h`, which declares
   the intrinsics with the IS names in `std::meta`. libc++ 22 ships no
   `<meta>`.
4. **Flag** (D6). Everything here requires `-freflection` and
   `-std=c++26`; without the flag the lexer produces none of `^^`, `[:`,
   `:]` and no rule below is reachable (implemented:
   `reflection/flag-off.cpp`). Turning it on by default at `-std=c++26` is
   an operator decision taken after M6.
5. **Feature macros** (→ WP-484), GCC 16.1's values under
   `-freflection`: `__cpp_impl_reflection 202603L`; with `<meta>`,
   `__cpp_lib_reflection 202603L` and `__cpp_lib_define_static 202506L`.
   GCC 16.1 defines no separate consteval-block or annotation macro.
   kiln predefines `__cpp_impl_reflection` under `-freflection` at C++26
   (WP-525: libstdc++ 16's `<meta>` is empty without it; spec/driver.md
   "Predefines"); the library macros wait until the corresponding
   surface passes its tests.
6. **Extensions.** `__kiln_define_member` (§6.3) is the only one. It
   requires `-freflection` and warns under `-Wkiln-extension` (D7,
   on by default). `std::meta::name_of`, `__kiln_access_via` and the
   recipe helpers of §3.5 (`__kiln_reflection_range`, `__kiln_member_offset`,
   `__kiln_access_context`) are test-header conveniences, not IS.
   `name_of` stays beside `identifier_of`. The spike's
   `__kiln_members_count` / `__kiln_member_at` were retired by WP-478.
7. **Out of scope for M6.** Token injection (P3294), reflection of
   attributes (P3385), expressions or statements, `define_class` beyond
   aggregates, function or body injection, link-compatible reflection
   mangling (§8.3), and clang wording parity for reflection diagnostics.

## 2. Representation

### 2.1 The value
- `std::meta::info` is the builtin type `MetaInfo` (spec/types.md),
  `decltype(^^int)`. It is a scalar, structural type: it may be an NTTP,
  and it compares with `==` and `!=` ([expr.eq]) (implemented:
  `reflection/value-model{,-neg}.cpp`). It is a consteval-only type
  (§4.1).
- A value is a `ReflectionId(u32)` into the AST's `ReflectionTable`.
  `ReflectionId`s are 1-based and dense; table slot 1 is the null
  reflection. In the Oven the value is an 8-byte scalar of
  `TyKind::Reflection` whose bits are `index - 1`
  (`ReflectionId::bits`/`from_bits`), so a value-initialized `info{}` is
  null. It freezes to and thaws from `Scalar::Reflection`. A bit pattern
  that is not an index the table handed out does not freeze ("kiln: value
  is not a reflection the compiler produced") and is not a valid
  intrinsic operand (§8.1).
- Overflow of the u32 index is a diagnostic, not a panic (→ WP-471).

### 2.2 `Reflectee`
`ReflectionTable` interns `ReflectionId → Reflectee`. The variants are:

| Variant | Payload | Designates | Status |
|---|---|---|---|
| `Null` | — | the null reflection | implemented |
| `Type(QualType)` | the type, alias sugar kept at the top (§2.3) | a type, or a type alias | implemented without alias sugar (→ WP-472) |
| `Decl(DeclId)` | first declaration | variable, function, data member, enumerator, class, structured binding, namespace | implemented for the spike's operands; the rest → WP-472 |
| `Namespace(DeclId)` | first declaration | a namespace (`^^::` is the TU) | today a `Decl`; split → WP-472 |
| `NamespaceAlias(DeclId)` | the alias declaration | a namespace alias | → WP-472 |
| `Template(TemplateNameId)` | the template name | a class, function, variable or alias template, or a concept | → WP-472 |
| `Value(ConstValueId)` | a typed frozen value | a value (`reflect_constant`, a template argument) | → WP-479 |
| `Object(ConstValueId)` | a pointer to a static-storage object | an object (`reflect_object`) | → WP-479 |
| `BaseSpec(DeclId, u32)` | derived class's first declaration, base index | a direct base class relationship | implemented (`bases_of`, WP-478) |
| `DataMemberSpec(DmsId)` | an interned spec | a `data_member_spec` result | → WP-480 |
| `Annotation(AnnId)` | one annotation occurrence | an annotation (§7) | → WP-481 |

- Each variant's payload is at most 12 bytes. A compile-time assertion
  on `size_of::<Reflectee>()` (at most 16 bytes) is added with the first
  variant beyond the spike's three (→ WP-472).
- `ReflectionTable::get` returns `Null` for an id it did not hand out.

### 2.3 Identity
Two reflections are equal iff their ids are equal. Interning makes this
the IS equality ([expr.eq]):
- **Declarations** compare by entity. The interned `DeclId` is the
  entity's first declaration, as computed by `Sema::same_entity` and its
  first-declaration helper, which is the identity primitive for every
  declaration-valued `Reflectee` (→ WP-471). Redeclarations, and the same
  entity named through a using-declaration, reflect equal.
- **Types** are canonicalized except for a type alias at the top: `^^const
  int == ^^int const` and `^^decltype(1) == ^^int`, but `^^Alias !=
  ^^int`, and `dealias(^^Alias) == ^^int` ([meta.reflection.queries];
  GCC 16.1 agrees). *The spike canonicalizes every type* (`^^Alias ==
  ^^int`); keeping the top-level alias → WP-472.
- **Values** are interned by type and content in `ConstStore`:
  `reflect_constant(3) == reflect_constant(3)` (GCC 16.1 agrees; → WP-479).
- **`data_member_spec`** results are interned by content: equal
  arguments give equal reflections (GCC 16.1 agrees; §6.1).
- **Annotations** are distinct per occurrence, even with equal values
  (§7; GCC 16.1 agrees; → WP-481).

### 2.4 Lifetime
- Ids are valid for the whole translation unit: the table and every arena
  it points into are append-only (spec/ast.md §2.4). An id may therefore
  live in a frozen constant, an NTTP, a memo key, a static-storage object,
  or across evaluations, and never dangles.
- Table growth inside an evaluation (`reflect_constant`,
  `reflect_object`, fresh `data_member_spec`s) cannot be reclaimed
  mid-evaluation, since an `info` may sit anywhere in Oven memory. It is
  bounded by content interning and charged to the evaluation as
  `refl_bytes`, which counts against `-fconstexpr-memory` (D5;
  → WP-479). A per-evaluation overlay table is an M7 item. This is the
  one exception to the Oven's reclaim-during-evaluation rule.

### 2.5 Typed copy (decided)
- A byte-wise copy of an `info` object is a *typed* copy: loads and
  stores are typed, so an `info` can be copied only as an `info`.
- `memcpy`/`memmove` between an `info` object and an object of another
  type is not a constant expression ("cannot constant evaluate 'memcpy'
  from object of type 'unsigned long' to object of type
  'std::meta::info'"; implemented, a dedicated test → WP-472), and
  `__builtin_bit_cast` to or from `info` is rejected by sema (not
  trivially copyable; implemented: `reflection/value-model-neg.cpp`).
- The shadow therefore needs no reflection kind; it keeps only its init
  and pointer bits. No shadow-kind extension is planned.

## 3. The Oven runtime

The value family (`extract`, `reflect_constant*`, P3491 `define_static_*`, `refl_bytes`) is specified in spec/reflection-values.md (WP-479).

### 3.1 Recognizing an intrinsic
- The `std::meta` functions have no bodies: the library declares them, and
  a direct call to a declaration that sema recognizes compiles to one KBC
  instruction. `SemaBridge::meta_intrinsic(f) -> Option<MetaOp>` is the
  recognizer, and sema's `meta_intrinsic_of` implements it
  (`kiln_sema/src/meta/mod.rs`).
- `f` is recognized iff all of these hold (implemented:
  `reflection/dispatch-inline-ns.cpp`):
  1. reflection is on, and `f` is a function or a member function;
  2. its enclosing context, with inline namespaces skipped, is either a
     namespace named `meta` whose enclosing context (inline namespaces
     skipped) is a namespace `std` at translation-unit scope, or a class
     directly inside such a `meta` (the op is then looked up as
     `Class::name`, e.g. `access_context::current`), or the translation
     unit itself for an op that is not in `std::meta`
     (`__kiln_define_member`). So `std::meta`, `std::__1::meta` (with
     `__1` inline, as in libc++) and an inline namespace inside `meta` all
     qualify, and `std::other` does not;
  3. `(name, signature)` matches a row of the op tables (§3.3), where the
     signature is the parameter list with each parameter classified as
     `info` (its canonical unqualified type is `MetaInfo`) or not. An
     overload with another signature, such as `type_of(int)`, is an
     ordinary undefined function.
- Matching is a linear scan of the tables, run once per call site when the
  call is compiled, never per evaluation.
- A recognized call with more than `MAX_META_ARGS` (8) arguments is not
  compiled (unsupported). No table row has more than three.

### 3.2 The instruction and the bridge entry point
- The call compiles to `Op::Builtin` with aux `bi::META | code << 8`, where
  `code` is `family << 8 | row`, and operands `[dst, base, n, site + 1,
  elem, result type]`. `site` indexes the call's compile-time record (its
  callee, the recipes of §3.5 and the throw of §8.2); 0 means none. The
  `n` arguments are evaluated into the consecutive
  registers `base..base+n`. A class or array argument is passed by
  address (a glvalue's address, or a materialized temporary's); a pointer
  argument records its element type as `elem` (what `meta_c_string`
  reads). `base` and `n` are raw operands, not registers.
- The VM (`vm/meta.rs`) copies the argument bits, builds the `MetaCtx`
  (§3.4), applies the purity rules (§3.3), and makes the single bridge
  call:

```rust
fn meta(&mut self, op: MetaOp, args: &[u128], out: &mut MetaOut,
        ctx: &MetaCtx, oven: &mut Oven) -> Result<(), MetaError>;
```

  The default implementation returns `Err(MetaError::silent())`. The
  bridge receives the running Oven so that a query that must evaluate
  (completing a template specialization, checking a member type) lends it
  back to sema for a nested evaluation (R-OVEN-1, sema's `lend_oven`).
- `MetaOp` is a family enum. Each family is one file in
  `kiln_oven/src/meta/`, holding one `meta_family!` table whose rows are
  `Variant = ("source name", "signature", Purity, owner WP)`:

  | `MetaOp` variant | Family enum | File | IS clause |
  |---|---|---|---|
  | `Query` | `QueryOp` | `query.rs` | [meta.reflection.queries], [meta.reflection.names], P3096 |
  | `Trait` | `TraitOp` | `traits.rs` | [meta.reflection.traits] |
  | `Member` | `MemberOp` | `member.rs` | [meta.reflection.member.queries], [meta.reflection.layout] |
  | `Value` | `ValueOp` | `value.rs` | [meta.reflection.extract], [meta.reflection.result], [meta.reflection.array], [meta.define.static] |
  | `Define` | `DefineOp` | `define.rs` | [meta.reflection.define.aggregate] |
  | `Annot` | `AnnotOp` | `annot.rs` | [meta.reflection.annotation] |
  | `Access` | `AccessOp` | `access.rs` | [meta.reflection.scope], [meta.reflection.access.context], [meta.reflection.access.queries] |
  | `Template` | `TemplOp` | `templ.rs` | [meta.reflection.substitute], template queries |
  | `Ext` | `ExtOp` | `ext.rs` | kiln extensions and test conveniences (§1.6) |

  A work package adds rows to a family, or adds a family file and a
  `MetaOp` variant. It never changes the bridge signature. Rows are only
  appended. Sema dispatches per family in `kiln_sema/src/meta/` (one file
  per family, mirroring the Oven's). A row without a sema body fails with
  the note "kiln: reflection function 'std::meta::X' is not implemented
  yet (WP-NNN)" (`MetaErrorKind::NotImplemented`).
- **Results** (`MetaOut`): `Scalar(u128)` (a `bool` or an integer of the
  call's result type), `Info(ReflectionId)`, or `Frozen(ConstValueId)` (a
  frozen value, thawed as the call's result type; today a pointer, such
  as `name_of`'s string). The class-typed results of §3.5 are
  `Str(ConstValueId, len)`, `SrcLoc(Option<ConstValueId>)`,
  `Pair(u128, u128)` and `Infos(n)` (the `n` reflections the bridge left
  in `Oven::meta_infos`). A successful call that writes nothing, or a
  variant the call's recipe does not take, is an internal error ("kiln:
  internal: 'X' produced no result of its type").
- **String operands** are read with `Oven::meta_c_string(p, ctx)`: up to
  the NUL, at most 4096 code units (more is an invalid name). An unreadable
  pointer is the Oven's own fault, reported as for any other load.

### 3.3 Purity classes
Every op has one purity class. The class decides memoization (§3.6) and
whether the op may run in a trial (§4.3):

| Class | Meaning | VM effect |
|---|---|---|
| `Stable` | a pure function of the operands for the rest of the TU | none: a caller whose other work is pure is memoized |
| `PointDependent` | depends on what the TU has declared or completed at the evaluation point | `run.impure += 1`: the calling evaluation is not memoized |
| `Mutating` | changes the program | `run.impure += 1`, and in a trial (a fold, `__builtin_constant_p`, a speculative checkpoint) a silent failure that never reaches the bridge |

**Rules for classifying an op.**
- A query on a complete class, a type, a value, a name or a layout is
  `Stable`. A complete class never changes, since the IS adds members only
  by completing a class (§6), and `__kiln_define_member` works only before
  the closing `}`.
- A query whose answer can change later in the TU is `PointDependent`:
  completeness (`is_complete_type`, `is_enumerable_type`), anything a later
  redeclaration can change (`has_default_argument`, `is_defaulted`),
  namespace `members_of` (namespaces grow), the evaluation point itself
  (`access_context::current` and its siblings), annotations (a later
  redeclaration adds more), and `substitute`/`can_substitute` (they may
  instantiate). The spike's member queries are `PointDependent` because
  they also accept a class still being defined.
- An op that defines anything is `Mutating`.
- A `Stable` op that fails on an incomplete class is still `Stable`. The
  failure is not memoized, because only successful calls are.

**The op table.** This list covers every row in `kiln_oven/src/meta/`
(241 ops). The Rust tables are the source of truth: a difference between
this list and the tables is a spec bug, fixed in the same change.

- **Query · Stable · WP-470, done**: `type_of`.
- **Query · Stable · WP-476, done** (contract: spec/reflection-queries.md):
  `object_of`, `constant_of`, `parent_of`,
  `dealias`, `has_template_arguments`, `operator_of`, `symbol_of`,
  `u8symbol_of`, `return_type_of`, `is_function_parameter`,
  `is_explicit_object_parameter`, `is_vararg_function`, `is_public`,
  `is_protected`, `is_private`, `is_virtual`, `is_pure_virtual`,
  `is_override`, `is_final`, `is_deleted`, `is_user_provided`,
  `is_user_declared`, `is_explicit`, `is_noexcept`, `is_bit_field`,
  `is_enumerator`, `is_const`, `is_volatile`, `is_mutable_member`,
  `is_lvalue_reference_qualified`, `is_rvalue_reference_qualified`,
  `has_static_storage_duration`, `has_thread_storage_duration`,
  `has_automatic_storage_duration`, `has_internal_linkage`,
  `has_module_linkage`, `has_external_linkage`, `has_c_language_linkage`,
  `has_linkage`, `is_variable`, `is_type`, `is_namespace`, `is_type_alias`,
  `is_namespace_alias`, `is_function`, `is_conversion_function`,
  `is_operator_function`, `is_literal_operator`,
  `is_special_member_function`, `is_constructor`, `is_default_constructor`,
  `is_copy_constructor`, `is_move_constructor`, `is_assignment`,
  `is_copy_assignment`, `is_move_assignment`, `is_destructor`,
  `is_template`, `is_function_template`, `is_variable_template`,
  `is_class_template`, `is_alias_template`,
  `is_conversion_function_template`, `is_operator_function_template`,
  `is_literal_operator_template`, `is_constructor_template`, `is_concept`,
  `is_structured_binding`, `is_value`, `is_object`, `is_class_member`,
  `is_namespace_member`, `is_nonstatic_data_member`, `is_static_member`,
  `is_base`, `has_default_member_initializer`, `has_parent`.
- **Query · Stable · WP-475**: `has_identifier`, `identifier_of`,
  `u8identifier_of`, `display_string_of`, `u8display_string_of`,
  `source_location_of` (§3.9), and the helpers `__kiln_reflection_range`,
  `__kiln_member_offset`, `__kiln_access_context` (§3.5).
- **Query · PointDependent · WP-476, done**: `variable_of`,
  `has_default_argument`, `is_defaulted`, `is_complete_type`,
  `is_enumerable_type`.
- **Trait · Stable · WP-477**: `is_void_type`, `is_null_pointer_type`,
  `is_integral_type`, `is_floating_point_type`, `is_array_type`,
  `is_pointer_type`, `is_lvalue_reference_type`, `is_rvalue_reference_type`,
  `is_member_object_pointer_type`, `is_member_function_pointer_type`,
  `is_enum_type`, `is_union_type`, `is_class_type`, `is_function_type`,
  `is_reflection_type`, `is_reference_type`, `is_arithmetic_type`,
  `is_fundamental_type`, `is_object_type`, `is_scalar_type`,
  `is_compound_type`, `is_member_pointer_type`, `is_const_type`,
  `is_volatile_type`, `is_trivially_copyable_type`,
  `is_standard_layout_type`, `is_empty_type`, `is_polymorphic_type`,
  `is_abstract_type`, `is_final_type`, `is_aggregate_type`,
  `is_structural_type`, `is_signed_type`, `is_unsigned_type`,
  `is_bounded_array_type`, `is_unbounded_array_type`, `is_scoped_enum_type`,
  `is_constructible_type`, `is_default_constructible_type`,
  `is_copy_constructible_type`, `is_move_constructible_type`,
  `is_assignable_type`, `is_copy_assignable_type`,
  `is_move_assignable_type`, `is_swappable_with_type`, `is_swappable_type`,
  `is_destructible_type`, `is_trivially_constructible_type`,
  `is_trivially_assignable_type`, `is_trivially_copy_assignable_type`,
  `is_trivially_move_assignable_type`, `is_trivially_destructible_type`,
  `is_nothrow_constructible_type`, `is_nothrow_copy_constructible_type`,
  `is_nothrow_move_constructible_type`, `is_nothrow_assignable_type`,
  `is_nothrow_copy_assignable_type`, `is_nothrow_move_assignable_type`,
  `is_nothrow_swappable_with_type`, `is_nothrow_swappable_type`,
  `is_nothrow_destructible_type`, `is_implicit_lifetime_type`,
  `has_virtual_destructor`, `reference_converts_from_temporary`, `rank`,
  `extent`, `is_same_type`, `is_base_of_type`, `is_virtual_base_of_type`,
  `is_convertible_type`, `is_nothrow_convertible_type`,
  `is_layout_compatible_type`, `is_invocable_type`, `is_invocable_r_type`,
  `is_nothrow_invocable_type`, `is_nothrow_invocable_r_type`,
  `remove_const`, `remove_volatile`, `remove_cv`, `add_const`,
  `add_volatile`, `add_cv`, `remove_reference`, `add_lvalue_reference`,
  `add_rvalue_reference`, `make_signed`, `make_unsigned`, `remove_extent`,
  `remove_all_extents`, `remove_pointer`, `add_pointer`, `remove_cvref`,
  `decay`, `common_type`, `common_reference`, `underlying_type`,
  `invoke_result`, `unwrap_reference`, `unwrap_ref_decay`, `tuple_size`,
  `tuple_element`, `variant_size`, `variant_alternative`, `type_order`.
  Full contract and every row (with the `(data, n)` range rows): spec/reflection-traits.md.
- **Member · PointDependent · WP-478, done**: `members_of`.
- **Member · Stable · WP-478, done**: `bases_of`, `static_data_members_of`,
  `nonstatic_data_members_of`, `subobjects_of`, `enumerators_of`,
  `parameters_of`, `offset_of`, `size_of`, `alignment_of`, `bit_size_of`.
- **Access · PointDependent · WP-478, done**: `access_context::current`,
  `current_function`, `current_class`, `current_namespace`.
- **Access · Stable · WP-478, done**: `is_accessible`, `has_inaccessible_bases`,
  `has_inaccessible_nonstatic_data_members`, `has_inaccessible_subobjects`,
  `__kiln_access_via` (`via`'s check).
  Full contract and every row of the three families: spec/reflection-members.md.
- **Value · Stable · WP-479**: `extract`, `reflect_constant`,
  `reflect_object`, `reflect_function`, `reflect_constant_string`,
  `reflect_constant_array`.
- **Template · Stable · WP-482**: `template_of`, `template_arguments_of`
  (implemented, `kiln_sema/src/meta/templ.rs`; spec/landed/reflection/WP-482.md).
- **Template · PointDependent · WP-482**: `can_substitute`, `substitute`
  (implemented: a SFINAE frame inside a Transactional Oven trial, memoized
  on the (template, arguments) key; spec/landed/reflection/WP-482.md).
- **Define · Stable · WP-480**: `data_member_spec`, `is_data_member_spec`.
- **Define · Mutating · WP-480**: `define_aggregate`.
- **Annot · PointDependent · WP-478, done**: `annotations_of`,
  `annotations_of_with_type`.
- **Annot · Stable · WP-478, done**: `is_annotation`.
- **Ext · Stable · WP-470, done**: `name_of`.
- **Ext · Mutating · WP-470, done**: `__kiln_define_member`.

### 3.4 `MetaCtx`
The context the VM passes with every call:

| Field | Meaning | Status |
|---|---|---|
| `scope: Option<DeclId>` | the scope of the evaluation point, for `access_context::current()` and namespace `members_of`; `None` means the bridge's current context | always `None` until → WP-473 sets it from the evaluation context |
| `loc: Loc` | the call's location: the position half of the evaluation point, and where failure notes point | implemented |
| `plainly: bool` | the evaluation is plainly constant-evaluated (§4.2), the only context that may inject | implemented: set from `Oven::plain()` (`EvalContext::plain`, WP-473) |
| `trial: bool` | the evaluation is a trial: `run.fold`, or an open memory checkpoint | implemented |
| `elem` (crate-private) | the element type of the call's last pointer argument | implemented |

The evaluation point is the pair `(scope, loc)`. A `PointDependent` op
answers as of that point.

### 3.5 Materialized results and range arguments
- **Recipes.** When a call of an intrinsic is compiled, the Oven asks sema
  for the call's *shapes* (`SemaBridge::meta_shape`): one for a
  class-typed result, one for each `reflection_range` parameter. It then
  matches each shape against the class's layout, once per call site, into
  a *recipe*: the offsets and types of the scalar leaves the object is
  built from (at most 4 leaves, nesting depth at most 8; a union or a
  bit-field leaf does not match). The match is by layout, not by header:
  libc++'s, libstdc++'s and `tests/support/kiln_meta.h`'s classes all
  qualify.

  | Shape | Results of | Layout required | Filled from |
  |---|---|---|---|
  | `Str` | `identifier_of`, `display_string_of`, `symbol_of` (`string_view`); the `u8` forms (`u8string_view`) | a `const C *` (C the op's character type) and an integer size, either order | the frozen string's address and length |
  | `SrcLoc` | `source_location_of` | one pointer (to an `__impl` of `file_name`, `function_name`, `line`, `column`) | a frozen `__impl`, or null |
  | `Vector` | every op returning `vector<info>` | three `info *` (begin, end, end of capacity) in offset order | a fresh allocation (below) |
  | `Pair` | `offset_of` (`member_offset`), `access_context::current` and the `access_context` factories | two scalar leaves in offset order | the two values |
  | `Range` | each `reflection_range` parameter (`substitute`, `can_substitute`, …) | `(info *, size)` (`span`, `initializer_list`), `(begin, end, …)` (`vector`), or an array of `info` (`array<info, N>`, a built-in array) | read before the call as a pointer and a length |

- **Mismatch.** A result or argument class that does not match its shape
  is not an error of the declaration: the call compiles to a trap, and
  evaluating it is not a constant expression with the note "'T' cannot
  be used by 'F': it does not hold …" at the call. A call of an op with
  no shape returning a class is the same trap.
- **`vector<info>`.** The elements are one `ARRAY | STD` heap allocation
  of `n` `info`, exactly as `std::allocator<info>::allocate(n)` makes, and
  their lifetimes begin as by `construct_at`. The vector owns it: its
  destructor's `deallocate` reclaims it, and one that escapes the
  evaluation is the ordinary transient-allocation error. An empty result
  allocates nothing (three null pointers).
- **Strings.** Each (reflection, string kind) is computed once per TU and
  frozen as a NUL-terminated array of `char` or `char8_t`; identical text
  shares one frozen object, and its thaw is one static-storage object per
  TU. A names query therefore allocates nothing per call (`allocs` 0)
  and repeating it costs no static bytes (`names/budget.cpp`). The
  spike's `name_of` is kept and works the same way. The string's
  encoding is UTF-8; the ordinary-literal encoding is assumed to be
  UTF-8 as well.
- **`source_location`.** The `__impl` is the one
  `__builtin_source_location` would produce for the declaration's
  presumed location, with an empty function name, so equal locations
  share it.
- **Range arguments.** The Oven passes the bridge a packed `data | n <<
  64` in place of the argument, read with `Oven::meta_range_len` and
  `meta_range_at`. The range is not copied. A range whose elements cannot
  be read (uninitialized, out of the object) is the evaluator's ordinary
  fault.
- **Test helpers.** `__kiln_reflection_range(r)` (the range as a
  `vector<info>`), `__kiln_member_offset(bytes, bits)` and
  `__kiln_access_context(scope, cls)` exercise the `Range`, `Vector` and
  `Pair` recipes before the ops that use them land (`Query` family,
  WP-475).

### 3.6 Memoization
- A function whose parameters include `std::meta::info` is eligible for
  the memo like one taking integers (spec/oven.md "Memoization"). The key
  is the id's bits, which are stable for the TU (§2.4). Implemented.
- A call that ran a non-`Stable` op has `run.impure` raised, so neither it
  nor any call whose span contains it is cached (implemented:
  `reflection/memo-purity.cpp`, which checks `memo_hits` per evaluation).
- A `Mutating` call is never replayed. This is the property
  `reflection/define-member-neg.cpp` depends on.
- Functions returning `vector<info>` allocate, so the memo refuses them
  anyway. §3.7 keeps their cost linear instead.

### 3.7 Member lists and the type table
- **Member cache** (WP-478, done; spec/reflection-members.md §2). One
  list per (class, enumeration or namespace, list kind), built on the
  first query of a class after its implicit special members are declared
  (D4). Each entry carries its reflection and a kind and access mask, so
  `members_of`, the data-member filters and access filtering are one
  filtered copy into the Oven's reusable result buffer.
  - A complete class's list is immutable.
  - A namespace's list is a prefix extended as the namespace grows.
  - `bases_of` and `enumerators_of` have their own lists.
- **Dense index** (deferred). The member path interns each entry's
  reflection once when the list is built, so the per-query path does no
  hash probe; a `DeclId → ReflectionId` side array is not needed for it.
- **Incomplete classes in the Oven's type table.** A class interned while
  incomplete, or while it is being defined, is a *provisional* `Opaque`
  slot whose layout is not computed (`ensure_layout` returns `None` for
  an incomplete class, spec/abi.md §1). Once the class completes, the
  slot is rebuilt **in place**, so an `OTy` already held stays valid. No
  object of a provisional type can exist. The provisional slots are a
  hash set; `Types::refresh` runs at the start of every evaluation and on
  an `intern` hit on a provisional slot, and does nothing unless sema's
  class-completion epoch (`DeclArena::completion_epoch`, moved only by
  `complete_definition`) changed since the last refresh; it then visits
  the provisional slots in creation order, so thousands of
  forward-declared classes cost nothing per evaluation
  (`tests/stress/provisional-types-scale.cpp`; WP-471).
  `define_aggregate` refreshes its target's slot immediately (§6.2).

### 3.8 Accounting
- A reflection call is ordinary Oven work: steps, calls and bytes of the
  evaluation, plus the bridge's sema work, which appears only in
  `wall_us`.
- `memo_hits` (spec/oven.md "Reporting protocol") counts memo hits per
  evaluation. It is the one counter that differs with the memo on or off,
  and the `oven:` test directive accepts `memo_hits>=N` floors
  (spec/harness.md).
- New `-fconstexpr-report` fields (→ WP-483): `meta_calls`,
  `meta_cache_hits`, `refl_bytes`, `injected_decls`,
  `instantiations_from_meta`. `refl_bytes` is included in `peak_bytes`
  and in the `-fconstexpr-memory` limit (§2.4).
- Performance budgets for the reflection bench shapes (S1–S6) and the
  4N scaling gate are recorded in docs/status/perf.md by → WP-483.

### 3.9 The names family
- [meta.reflection.names] (P2996R13) as implemented by
  `kiln_sema/src/meta/names.rs`. A query whose answer is not defined
  fails with `Failed` and the note shown (thrown per §8.2).

  | Function | Result |
  |---|---|
  | `has_identifier(r)` | whether `identifier_of(r)` succeeds |
  | `identifier_of(r)`, `u8identifier_of(r)` | a class, enumeration, enumerator, variable, function, data member, namespace, namespace alias, type alias or template (a `Template` reflection answers for its templated declaration): the identifier it was declared with; an unnamed class or enumeration with a typedef name for linkage ([dcl.typedef]/9): that name; a literal operator: its suffix. A type: as its alias when it is a cv-unqualified typedef or alias-declaration type (not an alias template specialization), else as its class or enumeration when it is a cv-unqualified one. Otherwise (a specialization, a constructor, destructor, operator or conversion function, an unnamed entity, the global namespace, any other type or value) fails: "'R' has no identifier" |
  | `display_string_of(r)`, `u8display_string_of(r)` | implementation-defined; never fails: a type prints as in diagnostics (`const int *`), a declaration as its name, anything else as `^^…` |
  | `source_location_of(r)` | the location of the declared name, with an empty `function_name`; a reflection with no declaration (a fundamental or compound type, a value, the translation unit) gives `source_location{}` |

- Template parameters and lambdas (their `identifier_of`) are deferred,
  with type aliases as reflections distinct from their types (§2.2).

## 4. The consteval boundary

### 4.1 Consteval-only types
- The consteval-only rules (which types are consteval-only; the
  variable, function and expression checks; template leaf tracking) are
  specified once, in `spec/reflection-repr.md` §6. The escalation
  behaviour of a consteval-only expression in an immediate-escalating
  function (the function becomes immediate, with
  `note_immediate_function_reason_consteval_only`) is specified in
  `spec/consteval-blocks.md` §4.
- No value of consteval-only type reaches codegen. Codegen's constant
  emitter stays as the backstop for the known gaps: it rejects such a
  value with "reflection values cannot be emitted"
  (`kiln_codegen/src/consts.rs`).

### 4.2 Plainly constant-evaluated
- An evaluation is *plainly constant-evaluated* ([expr.const]) when it is
  the evaluation of a consteval block (`consteval { … }`, P3289) or of
  another construct the IS lists as such. GCC 16.1 is the reference for
  the exact list. Only such an evaluation may inject (`define_aggregate`).
- The Oven learns this from the evaluation context (`EvalContext`), sets
  `MetaCtx::plainly`, and keeps it in `run`. It replaces sema's
  `inst.eval_loc` gate that the spike uses.
- A consteval block is a declaration ([dcl.pre]). It is evaluated where
  it appears, and its evaluation's scope and location are the injection
  point (§6.2).

### 4.3 Trials never mutate
- A trial is a speculative evaluation whose result may be discarded:
  `try_fold`, `__builtin_constant_p`, constant-initialization probing,
  SFINAE-style probing inside `can_substitute` and `substitute`, and any
  evaluation under a memory checkpoint.
- A `Mutating` op in a trial fails silently, before the bridge is called
  (implemented). If the evaluation is also manifestly constant-evaluated,
  that evaluation reports the failure.
- `can_substitute` runs its instantiation-time evaluations under
  a Transactional checkpoint and rolls back on failure. Interned
  reflections survive the rollback (append-only, §2.4); so does nothing
  else: no instantiation-report row, no diagnostic, no Oven bytes
  (implemented, WP-482: `reflection/substitute-trial-rollback.cpp`).

## 5. Splicing (WP-474)
The grammar is in spec/parse.md §5.2. This section gives the semantics;
the sema is `kiln_sema/src/splice.rs` (`Sema::splice_entity` reads every
position's operand).

- **Operand.** The splice operand is a converted constant expression of
  type `std::meta::info` ([basic.splice]), in every position. A non-`info`
  operand gets "splice operand of type {0} is not a reflection", and a
  non-constant one gets "splice operand must be a constant expression"
  followed by the Oven's notes (`basic.splice/p1-not-info.cpp`,
  `basic.splice/p1-not-constant.cpp`, `reflection/splice-type-neg.cpp`).
- **Designated entity.** The splice designates the operand's `Reflectee`.
  Each grammar position accepts these kinds:

  | Position | Syntax | Accepts | Status |
  |---|---|---|---|
  | type | `typename [: r :]` [dcl.type.splice]; a bare `[: r :]` in a type-only context ([temp.res.general]/4: an alias-declaration, a namespace-scope or member decl-specifier, a member function's parameter, a trailing return type, a template parameter's default, a cast's or `new`'s type-id); `[: r :]` as a template type argument | a type or a type alias | implemented (`dcl.type.splice/`) |
  | expression | `[: r :]` [expr.prim.splice] | a variable, function, enumerator, structured binding, value or object; a non-static data member or member function only as the operand of `&` (a pointer to member) or in an unevaluated operand ([expr.prim.id.general]/4) | implemented (`expr.prim.splice/`) |
  | member access | `e.[: m :]`, `e->[: m :]` [expr.ref] | a member of `e`'s class or of one of its bases (data member, static member, member function, enumerator), or a direct base (P3293: the base subobject, same value category) | implemented; the base form is untested until `bases_of` (WP-478) |
  | scope | `[: r :]::` in a nested-name-specifier | a namespace, a namespace alias, a class or an enumeration (or an alias of one) | implemented (`basic.splice/splice-scope-specifier.cpp`, `namespace.udecl/splice-scope.cpp`) |
  | template | `template [: r :] < args >` [temp.names] in an expression, after `typename` or in a type-only context, and as a scope (`template [: r :] < args > ::`); `template [: r :]` alone before a call | a class or alias template in a type or scope; a variable template, concept or function template in an expression | implemented (`temp.names/splice-*.cpp`; spec/landed/reflection/WP-482.md) |
  | namespace | `using namespace [: r :];` | a namespace other than the global namespace, not dependent ([namespace.udir]/1) | implemented (`namespace.udir/`) |

  A misfit gets "splice in a {position} position does not designate a
  {kinds}" (`sema_splice_not_type`, `_not_expression`, `_not_member`,
  `_not_scope`, `_not_namespace`); a splice before a template argument
  list that designates no template gets "splice before a template
  argument list does not designate a template"
  (`sema_splice_not_template`). A constructor or destructor is
  ill-formed in any splice-expression, the member access form included
  (`sema_splice_special_member`, [expr.prim.splice]/2). A splice-scope-
  specifier in a declarative nested-name-specifier (an out-of-line
  definition's declarator-id, a class-head-name or enum-head-name) gets
  `sema_splice_declarative_scope` ([expr.prim.id.qual]/3; GCC 16.1 accepts
  some of these). A dependent using-directive splice gets
  `sema_splice_dependent_namespace` at the template definition (GCC 16.1
  accepts it) and the global namespace `sema_splice_global_namespace`.
- **Member functions.** A member-function splice is not a qualified-id, so
  `e.[: ^^B::f :]()` of a virtual `f` is a virtual call ([expr.call]/2; the
  [dcl.contract.func] example). `&[: ^^B::f :]` is a pointer to member.
- **Access.** Splices are not access-checked: a splice of a private member
  is valid from anywhere (P2996; GCC 16.1 agrees). Access is the business of
  the query that produced the reflection (`access_context`, WP-478).
- **Dependent splices.** A value-dependent operand produces a `Splice`
  type node (a type splice), a `CXXSpliceExpr` (an expression or member
  splice; with the object expression when that or the operand is
  dependent) or an `Nns::Splice` (a scope splice; the nested-name-specifier
  is always dependent). Each is resolved by the `templ/subst` arm for its
  kind when substitution makes the operand non-dependent, through the same
  `splice.rs` entry points as a non-dependent splice, so a misfit is
  reported in the specialization with its instantiation note
  (`temp.dep/splice-*.cpp`).
- **Fast path.** When the operand is already a frozen reflection, sema
  reads its id without an Oven run: through parentheses, implicit casts
  and a substituted NTTP (`SubstNonTypeTemplateParmExpr`), a `ConstantExpr`
  (its stored value), a `CXXReflectExpr` (its reflection) or a
  `DeclRefExpr` to a constexpr `info` variable whose value is computed.
  Each hit counts in `Sema::splice_fast_count()` (not an Oven counter, so
  the constexpr report does not show it); every other operand is
  evaluated as a constant expression. 1,000 type, 1,000 expression and
  1,000 scope splices of three frozen operands start no evaluation
  (`basic.splice/frozen-operand-fast-path.cpp`: `evaluations<=4`, the
  three initializers and a static_assert). Each distinct `R` in `[: R :]`
  costs one instantiation, as expected.
- **NTTPs.** A substituted `info` template argument is rebuilt as the
  `CXXReflectExpr` of the same entity (implemented).

## 6. Defining classes

### 6.1 `data_member_spec`
`data_member_spec(type, data_member_options)` validates its options
([meta.reflection.define.aggregate]/3-5). The checks are GCC 16.1's, in
its order (verified line by line:
`meta.reflection.define.aggregate/data-member-spec-neg.cpp`). Each
failure throws `std::meta::exception` (§8.2), so a `try` block catches
it, with `from() == ^^data_member_spec`:
1. `type` reflects a type: "'X' does not reflect a type".
2. It is an object or reference type: "'T' is not an object or reference
   type" (`void`, function types, `int[]` is accepted here and rejected
   by `define_aggregate`).
3. A given name is an identifier ([lex.name]): "'n' is not a valid
   identifier". `name_type` holds either form (a `u8string` or a
   `string`, §3.5); the Oven reads its flag, then the pointer and length,
   and the bytes are checked as UTF-8 by `kiln_lex::is_identifier`
   (XID_Start/XID_Continue, `$` not accepted).
4. It is not a keyword or alternative token: "'n' is a keyword, not an
   identifier".
5. A name or a bit-field width is given: "neither 'name' nor 'bit_width'
   specified".
6. Not both `alignment` and `bit_width`: "both 'alignment' and
   'bit_width' specified".
7. An alignment is a positive power of two: "'alignment' a is not a
   positive power of two".
8. It is no smaller than the type's: "'alignment' a is smaller than the
   alignment n of 'T'" (checked when `T` is complete and not a reference;
   no instantiation is started for it).
9. A width needs an integral or enumeration type: "'bit_width' specified
   with non-integral and non-enumeration type 'T'".
10. Not a width with `no_unique_address`: "'bit_width' specified with
    'no_unique_address'".
11. A width is not negative: "'bit_width' w is negative" (nor above
    `INT_MAX`).
12. Width zero has no name: "'bit_width' 0 specified with a 'name'".
13. An unnamed bit-field's type is not cv-qualified: "unnamed bit-field of
    cv-qualified type 'T'".

The result is a `DataMemberSpec(DmsId)` reflection. The description
(type, name `Symbol`, alignment, width, `no_unique_address`) is interned
by content in the Oven (`kiln_oven/src/meta/define.rs`, `DmsTable`), so
two calls with the same options give the same reflection, whichever form
the name was spelled in. `is_data_member_spec` tests for that kind.

### 6.2 `define_aggregate`
**Preconditions**, checked in order. They are "Constant When" ones: a
failure is not a constant expression (a hard error with notes, §8.1),
never a `std::meta::exception`, so a `try` block does not catch it:
1. The evaluation is plainly constant-evaluated (§4.2): "'std::meta::
   define_aggregate' can only be called from a 'consteval' block". A
   `static_assert`, a constexpr variable's initializer or a template
   argument is not. In a trial the call fails silently (§4.3), so a
   `__builtin_constant_p` probe never mutates.
2. `cls` reflects a class type ("'X' is not a class type"), not
   cv-qualified, not being defined ("'C' is being defined") and not
   complete ("'C' is already complete", plus "definition is here"; a
   second `define_aggregate` of the same class lands here).
3. **Scope rule** (GCC 16.1's): no function or class scope lies between
   the class's target scope and the consteval block's scope, that is, on
   either path below their nearest common enclosing scope. A lambda body
   counts as a function scope. Otherwise: "'f' intervenes between the
   scope of 'C' and the 'consteval' block" (or "a lambda intervenes"),
   plus "'consteval' block defined here". So a block in `f` cannot
   complete a namespace-scope class, nor a block at namespace scope a
   class local to `f` or a member class `H::M`; a block in `H` can
   complete `H::M`, and a block in `f` a class local to `f`.
4. Every element of the range is a description with a complete type (or
   a reference): "element i ('X') is not a data member description",
   "type 'T' of data member description i is incomplete" (the class
   itself, an incomplete class, `int[]`; a template specialization is
   completed on demand first), "type 'T' of data member description i is
   an abstract class".
5. Member names are unique: "name 'n' used in multiple data member
   descriptions". `_` may repeat ([basic.scope.scope]/5), and unnamed
   bit-fields have no name.

Checks 4 and 5 are one pass over the range with one `FxHashSet<Symbol>`
(reserved to the range's length), so the first failing element is the
one reported, as in GCC.

**Effect.**
- Sema starts the record's definition and appends one public `Field` per
  description, in order (`Sema::inject_field`). Each is flagged
  `DeclFlags::INJECTED`; its injection point (the consteval block) goes
  in the cold `injection_point` map, and a later redeclaration conflict
  notes "member injected here". The location of the call is the field's
  location. Alignment becomes an `alignas`, the width a bit-field width
  (a width above the type's size warns as a declared one does),
  `no_unique_address` the attribute. `#pragma pack` at the block does not
  apply.
- Sema runs `act_on_finish_record_definition` once, which computes the
  layout and the aggregate and triviality bits once. The class is
  complete when `define_aggregate` returns, so the rest of the same
  evaluation already sees it as complete (`sizeof`, `new C{...}`, member
  access).
- **Provisional slots.** A type slot the Oven gives an incomplete class
  stays `Opaque` and is rebuilt in place once the class is complete, so
  pointers already naming it stay valid. The Oven keys these slots by the
  class's first declaration and keeps their set (`FxHashSet<OTy>`) for
  `intern`'s membership test. The AST logs each completed record; the
  log's length is the completion epoch, and the Oven keeps the epoch it
  has applied. A refresh (before each evaluation, and after each
  `Mutating` op so the rest of the evaluation sees the class complete)
  reads only the log entries since that epoch and rebuilds their slots:
  O(classes completed since), O(1) when none was, never a scan of all
  provisional slots (`stress/incomplete-classes-many.cpp`: 5,000
  provisional slots, 10,000 evaluations, 100 completions).
- The result is `cls`.

**Cost.** O(n) in the number of descriptions: one hash set, one record
finish. Names are already `Symbol`s in the interned descriptions.
`stress/define-aggregate-10k.cpp` budgets a 10,000-member class.

**This is the one exception** to "a class is frozen at its closing `}`"
(spec/ast.md §2.4). It is serialized through sema, so the Oven never
observes a half-completed class.

### 6.3 The `__kiln_define_member` extension
`__kiln_define_member(info cls, info type, const char* name)`, declared at
TU scope. It is `Mutating`. It appends a public non-static data member to
a class *while that class is being defined*, which is a different rule
from §6.2. Like every intrinsic it exists only under `-freflection`
(§3.1). Each call that is evaluated warns "'__kiln_define_member' is a
kiln extension; use 'std::meta::define_aggregate'" under
`-Wkiln-extension` (on by default; `-Wno-kiln-extension` silences it).
It injects through §6.2's path (`Sema::inject_field`). When the call
succeeds, it appends a `Field` named `name` of
type `type`, flagged `DeclFlags::INJECTED` and carrying the call's
location, at the point the member-specification has reached, and returns
the member's reflection. Completion at `}` provides the layout,
aggregate-ness and the rest of the definition bits. Every later lookup
finds the member: later code in the same evaluation, later member
declarations, complete-class contexts, and code after the class. In a
class template, an unqualified use in a member function body is looked up
at definition time ([temp.res]), so it needs `this->v`.

The conditions, checked in order (implemented:
`reflection/define-member{,-neg,-run}.cpp`):
1. The evaluation is manifestly constant-evaluated (sema's
   `inst.eval_loc`; `MetaCtx::plainly` after → WP-473), and is not a
   trial. Otherwise the call fails silently.
2. `cls` reflects a class type: "'X' does not reflect a class type".
3. That class is the innermost class being defined, compared by first
   declaration. Otherwise the note is "'__kiln_define_member' can only add
   a member to the class whose member-specification is being evaluated
   ('S' is not)", plus "class being defined is 'I'" when there is one.
4. `type` reflects a complete object type other than `cls`. A template
   specialization is completed on demand. Otherwise the note is "'X' is
   not a complete object type for a data member", or "'X' does not
   reflect a type".
5. `name` is an identifier: "'n' is not an identifier".
6. `name` is not declared in the class yet ([class.mem]/5). Otherwise
   there are three notes: "duplicate member 'x'" at the call, "previous
   declaration is here", and "in evaluation of '__kiln_define_member' for
   class 'S' being defined here" at the class.

The supported contexts are the constant evaluations inside the
member-specification. In practice that means `static_assert`, called
directly or through a consteval helper, in a class or a class template;
each specialization evaluates its own.

## 7. Annotations (syntax and storage → WP-481; queries → WP-478)
- Syntax: `[[=constant-expression]]` in an attribute-specifier-seq on a
  declaration ([dcl.attr.annotation]). An annotation may not be combined
  with a `using` prefix. The grammar goes in spec/parse.md with → WP-481.
- The value is copy-initialized from the expression, must have
  structural type, and is frozen. Each occurrence is a distinct
  `Reflectee::Annotation(AnnId)`, even when two values are equal (§2.3).
- Storage: a cold map `DeclId (first declaration) → ListId<AnnId>`, kept
  in declaration order. Redeclarations append. `annotations_of(r)` lists
  them in that order: first the first declaration's, then each later
  redeclaration's that precedes the evaluation point. GCC 16.1 agrees, so
  `annotations_of` is `PointDependent`.
- Annotations on a templated entity are instantiated with it.
- `annotations_of_with_type(r, T)` keeps the annotations whose type is
  `remove_const(T)` (spec/reflection-members.md §5).
  `constant_of` and `extract` on an annotation yield its value, and
  `is_annotation` tests the kind.

## 8. Errors and ABI

### 8.1 `MetaError`
- Every bridge failure is a `MetaError { notes, kind }`:

  | `kind` | Meaning | Oven behaviour |
  |---|---|---|
  | `Failed` with notes | not a constant expression, for the reason in the notes | trap: the first note becomes the message, then the call stack, then the remaining notes |
  | `Failed` with no notes | a silent failure (`MetaError::silent()`) | the evaluation fails with no diagnostic. Only for trials, and for the extension's not-manifestly-constant case |
  | `NotImplemented { wp }` | the row has no sema body | trap with "kiln: reflection function 'X' is not implemented yet (WP-NNN)" |
  | `Fault(OvenFault)` | an evaluator fault while reading an operand (`meta_c_string`), or an operand the compiled call cannot have produced (`MetaError::invalid()`, e.g. an `info` operand that is no reflection) | reported exactly like the same fault in ordinary code |

- User input never panics a bridge method: every failure path returns a
  `MetaError`.

### 8.2 `std::meta::exception`
- **The class.** When a call of an intrinsic is compiled, sema looks up
  `std::meta::exception` (`SemaBridge::meta_exception`). It is used only
  if it is a complete class with a non-deleted constructor of three
  parameters `(S, std::meta::info, L)`, `S` a by-value `u8string_view` or
  `string_view` and `L` a by-value `source_location`, both trivially
  destructible; the `u8string_view` one is preferred (P3560R2's order).
- **When it is thrown.** A `Failed` error with notes from a call that is
  not in a trial (§4.3), and that a try block of the running evaluation
  encloses, is thrown: the call sets the site's flag register and the
  block compiled after it runs `bi::META_EXC` (spec/oven.md), which fills
  the message, `from` and `where` and constructs the object with that
  constructor; it is then thrown as by a throw-expression (P3068R6), and
  unwinds and is caught like any other exception. The message is the
  first note's text; `from` is `^^F`, the reflection of the intrinsic
  called; `where` is the call's location (function name empty).
- **Uncaught.** An exception no handler catches makes the evaluation not
  a constant expression with P3068's note "unhandled exception: W" (`W`
  its `what()`), at the throwing call.
- **Not thrown.** A failure outside every try block traps as before (the
  first note is the message, then the call stack, the remaining notes),
  and so does one when the class is not declared or does not qualify,
  and before C++26. A silent failure and
  any failure in a trial are never thrown: a trial stays silent and an
  enclosing handler sees nothing (`exception/trial-silent.cpp`).
- `kiln_meta.h`'s class copies the message into its own `new[]` buffers
  (`char` and `char8_t`), so a copy outlives the original. With a
  standard library whose `std::exception` is not usable in constant
  expressions (libc++ 19, libstdc++ 14) it derives from a local
  polymorphic base with the same `what()`.
- Where kiln has no clang wording to match, diagnostics follow GCC
  16.1's shape. Wording parity is out of scope (§1.7).

### 8.3 Mangling placeholder (WP-485)
- **Form.** A reflection inside a template argument of an emitted symbol
  mangles as a kiln vendor literal (`kiln_abi/src/mangle.rs`,
  `Mangler::scalar_value`):

  ```
  <expr-primary> ::= L <info-type> <number> E    # a reflection value
  <info-type>    ::= u6__info                    # std::meta::info
  ```

  `<number>` is the reflection's `ReflectionId` index in decimal (§2.3;
  the null reflection is 0). `<info-type>` is the mangling of the type
  `std::meta::info` everywhere (`template <class T>` with `T = info`
  gives `_Z1kIu6__infoEiv`), and it is a substitution candidate like any
  vendor type, so a second literal in one symbol writes `LS<seq>_<id>E`
  and later substitutions count past it: `two<^^I, ^^J>` is
  `_Z3twoILu6__info4ELS0_5EEiv`, `h<^^P>(Box<P>, P)` is
  `_Z1hILu6__info3E1PEi3BoxIT0_ES3_`. The literal composes wherever a
  template argument value does: in a pack (`J…E`), a nested
  template-id's arguments, a class-type NTTP's `Xtl … E`, and a template
  parameter object's `_ZTA`.
- **Properties.** Deterministic within the TU (the id is allocated at the
  reflection's first evaluation, so the same source gives the same ids).
  Distinct reflections give distinct strings, because the id is the
  identity of §2.3: a type and its alias (`^^int`, `^^I`, `^^J`) are
  three literals. The encoding reads only the id, never the reflectee, so
  no cache is needed: the id is both the key and the payload, written in
  at most ten digits. `AnonIds::reflection_literals()` counts the
  literals written for the module; the 2,000-specialization test of
  `crates/kiln_codegen/tests/reflection_mangle.rs` asserts it.
- **Linkage.** The id is TU-local, so the same `f<^^int>` in two TUs has
  two different strings, and two different reflections could share one.
  A template argument holding a reflection (directly, inside a class-type
  NTTP's value, or in a template parameter object) therefore makes the
  specialization, and everything mangled under it (its statics, vtable,
  RTTI, guards), internal (`mangle::value_externally_visible`, the
  unique-external rule of spec/abi.md §8.22). Each TU has its
  own copy: `S<^^int>::count` is not shared across TUs, and the RTTI of
  `S<^^int>` compares unequal across them. The form is **not
  link-compatible with GCC or clang** (spec/abi.md §8).
- **Follow-up (→ WP-102).** WP-102 switches to GCC 16.1's encoding
  (cxx-abi issue #208: `std::meta::info` is `Dm`, a value is
  `L Dm <kind> … E` naming the reflectee), at which point the
  specializations get their normal linkage again. Its link-compat test: a
  two-TU program, one TU compiled by kiln and one by `g++-16`, each
  defining and calling `template <std::meta::info R> int f()` for
  `^^int`, an alias, a namespace-scope variable and a data member, with
  `inline` statics, linked together; the run checks the shared statics
  and `&f<^^int>` agree.
- A consteval-only value reaching codegen stays the error of §4.1, not a
  mangling question: a non-template `info` variable with static storage
  that is not `constexpr` is rejected before any symbol exists
  (`expr.reflect/reflection-mangle-neg.cpp`).

## 9. Tests and status
- **Format.** Reflection tests are kiln goldens: `//@ args: -freflection
  -I tests/support`, `skip(clang)`, `oracle: none`, with `strict-diags`
  where they check diagnostics. Each test names the IS section it
  exercises. A test that needs clang 22 behaviour uses `//@ requires:
  clang>=22` or `//@ min-oracle: 22`. GCC 16.1 differential runs live off
  the merge queue (→ WP-483).
- **Directories.** The spike's tests stay in `tests/conformance/reflection/`.
  New tests go by IS clause:

  | Directory | Covers |
  |---|---|
  | `expr.reflect/` | the `^^` operator (§2) |
  | `basic.splice/` | splices in every position (§5) |
  | `dcl.pre.consteval-block/` | consteval blocks, plainly constant-evaluated (§4.2) |
  | `meta.reflection.queries/`, `meta.reflection.names/` | queries and names |
  | `meta.reflection.traits/` | type traits |
  | `meta.reflection.member.queries/`, `meta.reflection.layout/`, `meta.reflection.access.context/`, `meta.reflection.access.queries/` | members, layout, access (spec/reflection-members.md) |
  | `meta.reflection.extract/`, `meta.reflection.result/`, `meta.define.static/` | values and static storage |
  | `meta.reflection.substitute/` | substitution |
  | `meta.reflection.define.aggregate/` | `define_aggregate` (§6) |
  | `dcl.attr.annotation/`, `meta.reflection.annotation/` | annotations (§7) |
  | `meta.reflection.exception/` | the error model (§8) |

- **WP-470 tests.** `reflection/dispatch-inline-ns.cpp` covers §3.1.
  `reflection/memo-purity.cpp` covers §3.3 and §3.6.
- **Bench.** The `bench/reflect/` shapes S1–S6 and the scaling gate are
  added by → WP-483.
- **Status.** `docs/status/cxx-features.md` "C++26 reflection (M6)" has one
  row per WP-470..485. The spike's findings are frozen in
  docs/status/reflection-spike.md.
