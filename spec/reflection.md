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
   -freflection`) is the behavioural oracle. The differential is the
   harness's `oracle: gcc` (WP-483; spec/harness.md "GCC oracle"), and
   merge-queue CI runs it against GCC 16.1 (WP-502, docs/VALIDATION.md).
   88 reflection conformance files (98 in the corpus) carry `oracle:
   gcc`; the rest stay kiln goldens (`oracle: none`, §9). A triaged
   divergence is recorded in docs/status/deviations.md "GCC 16.1
   reflection divergences".
3. **Library vehicle** (D2; WP-484). The primary header is libstdc++
   16.1's real `<meta>` (`__cpp_lib_reflection 202603L`), reached with
   `--gcc-toolchain=<GCC 16.1 prefix>`: every consteval function it
   declares is an intrinsic row (§3.1) and every class it returns or takes
   (`vector<info>`, `string_view`, `optional`, `source_location`,
   `access_context`, `data_member_options`, `std::meta::exception`,
   `strong_ordering`, the range templates) is read and written by layout
   (§3.5). `tests/support/kiln_meta.h` stays the test vehicle of the
   corpus, which runs on any host without GCC; the header-differential
   mode (`cargo xtask test --meta-header`, spec/harness.md) runs that
   corpus again over the real header, so the two cannot drift. libc++ 22
   ships no `<meta>`.
4. **Flag** (D6; default per operator decision Q7, WP-920). Everything
   here requires C++26 (`-std=c++26`/`gnu++26`), where reflection is **on
   by default**; `-fno-reflection` turns it off and the last of
   `-freflection`/`-fno-reflection` wins (`-freflection` alone is a
   no-op there; implemented: `reflection/default-on.cpp`). Off, the lexer
   produces none of `^^`, `[:`, `:]` and no rule below is reachable
   (implemented: `reflection/flag-off-26.cpp`). Below C++26 nothing
   changes from before WP-920: reflection is off whatever the flag, an
   explicit `-freflection` is accepted silently but lexes no reflection
   token and predefines no `__cpp_impl_reflection` (implemented:
   `reflection/flag-off.cpp`, `reflection/below-26-flag.cpp`). The cost
   of the default is recorded in docs/status/oss.md "WP-920".
5. **Feature macros** (WP-484), GCC 16.1's values with reflection on: `__cpp_impl_reflection 202603L`; with `<meta>`,
   `__cpp_lib_reflection 202603L` and `__cpp_lib_define_static 202506L`.
   GCC 16.1 defines no separate consteval-block or annotation macro.
   kiln predefines `__cpp_impl_reflection` whenever reflection is on
   (WP-525: libstdc++ 16's `<meta>` is empty without it; spec/driver.md
   "Predefines"); the library macros are the header's own (`<version>`
   keys them on `__cpp_impl_reflection`), and both are GCC-compared by
   `meta.syn/feature-macros.cpp` (WP-484).
6. **Extensions.** `__kiln_define_member` (§6.3) is the only one. It
   requires `-freflection` and warns under `-Wkiln-extension` (D7,
   on by default). `std::meta::name_of`, `__kiln_access_via` and the
   recipe helpers of §3.5 (`__kiln_reflection_range`, `__kiln_member_offset`,
   `__kiln_access_context`) are test-header conveniences, not IS.
   `name_of` stays beside `identifier_of`. The spike's
   `__kiln_members_count` / `__kiln_member_at` were retired by WP-478.
7. **Out of scope for M6.** Token injection (P3294), reflection of
   attributes (P3385), expressions or statements, `define_class` beyond
   aggregates, function or body injection, and clang wording parity for
   reflection diagnostics. (Link-compatible reflection mangling landed
   with WP-501, §8.3.)

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
- Overflow of the u32 index is a diagnostic, not a panic
  (`sema_too_many_reflections`; WP-471).

### 2.2 `Reflectee`
`ReflectionTable` interns `ReflectionId → Reflectee`. The variants are:

| Variant | Payload | Designates | Status |
|---|---|---|---|
| `Null` | — | the null reflection | implemented |
| `Type(QualType)` | the type, alias sugar kept at the top (§2.3) | a type, or a type alias | implemented, dependent `^^typename T::type` included (WP-519) |
| `Decl(DeclId)` | first declaration | variable, function, data member, enumerator, structured binding | implemented: operator, literal operator, conversion and destructor functions and dependent qualified names (`^^T::x`) included (WP-519; spec/reflection-repr.md §2; literal operator pinned by `meta.reflection.queries/functions.cpp`, WP-830); a member name in `^^C::m` is access-checked ([class.access]/4; `expr.reflect/member-access*.cpp`, WP-830) |
| `Namespace(DeclId)` | first declaration | a namespace (`^^::` is the TU) | implemented |
| `NamespaceAlias(DeclId)` | the alias declaration | a namespace alias | implemented |
| `Template(TemplateNameId)` | the template name | a class, function, variable or alias template, or a concept | implemented, dependent `^^T::template m<U>` included (WP-519) |
| `Value(ConstValueId)` | a typed frozen value | a value (`reflect_constant`, a template argument) | implemented (WP-479) |
| `Object(ConstValueId)` | a pointer to a static-storage object | an object (`reflect_object`) | implemented (WP-479) |
| `BaseSpec(DeclId, u32)` | derived class's first declaration, base index | a direct base class relationship | implemented (WP-478) |
| `DataMemberSpec(DmsId)` | an interned spec | a `data_member_spec` result | implemented (WP-480) |
| `FnParam(DeclId, u32)` | function's first declaration, parameter index | a function parameter (`parameters_of`) | implemented (WP-478) |
| `Annotation(AnnId)` | one annotation occurrence | an annotation (§7) | implemented (WP-481; queries WP-478; on base-specifiers and per element of a function parameter pack, WP-830) |

- Each variant's payload is at most 8 bytes: a compile-time assertion
  keeps `size_of::<Reflectee>()` at most 12 bytes
  (`kiln_ast/src/reflect.rs`).
- `ReflectionTable::get` returns `Null` for an id it did not hand out.

### 2.3 Identity
Two reflections are equal iff their ids are equal. Interning makes this
the IS equality ([expr.eq]):
- **Declarations** compare by entity. The interned `DeclId` is the
  entity's first declaration, as computed by `Sema::same_entity` and its
  first-declaration helper, which is the identity primitive for every
  declaration-valued `Reflectee` (WP-471). Redeclarations, and the same
  entity named through a using-declaration, reflect equal.
- **Types** are canonicalized except for a type alias at the top: `^^const
  int == ^^int const` and `^^decltype(1) == ^^int`, but `^^Alias !=
  ^^int`, and `dealias(^^Alias) == ^^int` ([meta.reflection.queries];
  GCC 16.1 agrees; `expr.reflect/alias-vs-dealias-identity.cpp`).
- **Namespace aliases** reflect the alias declaration
  (`NamespaceAlias`), never the namespace it names: `^^M != ^^N` for
  `namespace M = N;`, two aliases of one namespace (or an alias of an
  alias) compare unequal, and only `^^M == ^^M` ([expr.eq]/7: neither the
  same entity nor any other listed case; GCC 16.1 agrees; WP-505,
  `expr.eq/reflection-namespace-alias.cpp`, `reflection/reflect-operator.cpp`).
- **Values** are interned by type and content in `ConstStore`:
  `reflect_constant(3) == reflect_constant(3)` (GCC 16.1 agrees; WP-479).
- **`data_member_spec`** results are interned by content: equal
  arguments give equal reflections (GCC 16.1 agrees; §6.1).
- **Annotations** are distinct per occurrence, even with equal values
  (§7; GCC 16.1 agrees; WP-481).

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
  WP-479). A per-evaluation overlay table is an M7 item. This is the
  one exception to the Oven's reclaim-during-evaluation rule.

### 2.5 Typed copy (decided)
- A byte-wise copy of an `info` object is a *typed* copy: loads and
  stores are typed, so an `info` can be copied only as an `info`.
- `memcpy`/`memmove` between an `info` object and an object of another
  type is not a constant expression ("cannot constant evaluate 'memcpy'
  from object of type 'unsigned long' to object of type
  'std::meta::info'"; implemented: `reflection/typed-copy-memcpy-neg.cpp`), and
  `__builtin_bit_cast` to or from `info` is rejected by sema (not
  trivially copyable; implemented: `reflection/value-model-neg.cpp`).
- The shadow therefore needs no reflection kind; it keeps only its init
  and pointer bits. No shadow-kind extension is planned.

## 3. The Oven runtime

The value family (`extract`, `reflect_constant*`, P3491 `define_static_*`, `refl_bytes`) is specified in spec/reflection-values.md (WP-479).

**Reading a variable through a reflection** (WP-638; [temp.inst]/5 and /8,
[meta.reflection.extract]): reading a variable through a reflection
instantiates its definition as an odr-use would. `extract`, `constant_of`
and `object_of` of a variable (or of the object of one) first call
`Sema::use_reflected_var` (`crates/kiln_sema/src/meta/mod.rs`), which
runs the odr-use path of a spelled reference (`note_var_referenced`, the
deferred-instantiation queue; potentially evaluated whatever context the
evaluation runs in), so a static data member of a specialization that
`substitute` produced, or that only `members_of` /
`static_data_members_of` reached, has its initializer; a splice names
the variable and takes the ordinary path. Declaration queries (`type_of`,
`is_const`, `identifier_of`, ...) instantiate only the declaration, so a
member whose initializer would be ill-formed stays reflectable; reading
it diagnoses the initializer at the instantiation, with the "in
instantiation of static data member" note at the reading call. Tests:
`meta.reflection.substitute/substitute-static-member-init.cpp`,
`meta.reflection.extract/static-member-instantiate{,-neg}.cpp`.

**Using a function through a reflection** (WP-1244; [basic.def.odr]/4,
[temp.inst]/5 and /8, [meta.reflection.extract]): `extract` of a function
reflection to a function pointer or a pointer to member function forms
the pointer as `&f` / `&C::f` would, so it odr-uses the function: once
the extract succeeds, `Sema::use_reflected_function`
(`crates/kiln_sema/src/meta/mod.rs`) runs `mark_used_at` with the Oven
lent back, potentially evaluated whatever context the evaluation runs in.
A member of a class template specialization, or a function template
specialization, has its definition instantiated, at once when it is
constexpr (it may be needed for constant evaluation; the end-of-TU queue
otherwise), with the "in instantiation of member function" note at the
`extract` call (as GCC 16.1). A splice in a call, `obj.[: r :]` and
`&[: r :]` name the function and take the ordinary path. A failed
`extract` (a type mismatch; a reference `T`, which `extract` rejects for
a function) and the declaration queries (`members_of`, `type_of`,
`parameters_of`, ...) instantiate nothing, so a body ill-formed for the
specialization stays reflectable. Tests:
`meta.reflection.extract/function-{instantiate-neg,no-instantiate,instantiate-run}.cpp`.

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
- **The name table** (WP-484). `kiln_oven/src/meta/gcc16_meta_names.txt`
  lists every consteval function libstdc++ 16.1's `<meta>` declares
  (free functions of `std::meta` and members of its classes), each marked
  `intrinsic` (declared without a body) or `library` (defined in the
  header over the intrinsics). The unit tests of `kiln_oven::meta` check
  that every intrinsic has a row, that no library function has one, and
  that every row which is not a kiln extension (§1.6) is a listed
  intrinsic; with `KILN_GCC16_ROOT` set, a further test re-derives the
  list from the installed header and fails if it differs. Body-less
  helpers the header's own definitions call (`reflect_constant_string`
  and `reflect_constant_array`'s, `_S_exception_cvt_*_utf8`) are
  intrinsics too.
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

- **Query · Stable · WP-470, done**: `type_of` (it raises `run.impure`
  itself while a variable's type is still an array of unknown bound:
  spec/reflection-queries.md §1, WP-795).
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
  `annotations_of_with_type` (of a namespace alias: empty, never the
  aliased namespace's; WP-573, spec/reflection-members.md §5).
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
  qualify. libstdc++ 16.1's layouts (WP-484): `string_view` is
  `(size, const C *)`; `vector<info>` is `_Vector_impl`'s three pointers
  under its allocator base; `source_location` is one `const __impl *`;
  `access_context` is two `info` members (scope, designating class);
  `optional<T>` (the `data_member_options` members) is a payload union
  and a `bool` engaged flag, read by [`Oven::meta_dms_options`]'s leaf
  walk (§6.1), as is `_Name`, which holds a `u8string` or a `string`;
  `type_order`'s result is `std::strong_ordering`, built from its
  `less`/`equal`/`greater` static members.

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
- **Result types that follow GCC 16.1** (WP-484). `type_of` of an object
  or value reflection of a const object (a template parameter object, a
  subobject of a const variable), or of a class-type annotation, is
  `const`-qualified, and `is_const` of it is true (WP-571). An empty
  `reflect_constant_array` reflects a `const std::array<T, 0>` template
  parameter object (the header's own `array`), where a non-empty one
  reflects a `const T[N]`. `members_of` of a namespace lists the members
  of nested `extern "C"`/`extern "C++"` and `export` blocks in place, in
  declaration order; `members_of` of a namespace alias lists the aliased
  namespace's, and `is_namespace` of an alias is true (WP-1228). A
  class's list ends with the implicit `operator==` of each defaulted
  member `operator<=>` (WP-1228).
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
  - A namespace's list is in the creation order of each member's first
    declaration (GCC 16.1's `DECL_UID` order; spec/reflection-members.md).
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
- `-fconstexpr-report` carries `refl_bytes` (WP-479; spec/oven.md
  "Reporting protocol"), which is included in `peak_bytes` and in the
  `-fconstexpr-memory` limit (§2.4). The plan's further fields
  (`meta_calls`, `meta_cache_hits`, `injected_decls`,
  `instantiations_from_meta`) were not added: WP-483's bench gates on the
  existing counters (`steps`, `peak_bytes`, `peak_arena`, `evaluations`).
- The reflection bench shapes (`bench/reflect/` S1–S10, spec/bench.md
  "Reflection shapes") and the 4N scaling gate are WP-483's; their
  results are in docs/status/perf-history.md "WP-483" and
  docs/status/perf-reference-workload.md "kiln vs GCC 16.1".

### 3.9 The names family
- [meta.reflection.names] (P2996R13) as implemented by
  `kiln_sema/src/meta/names.rs`. A query whose answer is not defined
  fails with `Failed` and the note shown (thrown per §8.2).

  | Function | Result |
  |---|---|
  | `has_identifier(r)` | whether `identifier_of(r)` succeeds |
  | `identifier_of(r)`, `u8identifier_of(r)` | a class, enumeration, enumerator, variable, function, data member, namespace, namespace alias, type alias or template (a `Template` reflection answers for its templated declaration): the identifier it was declared with; an unnamed class or enumeration with a typedef name for linkage ([dcl.typedef]/9): that name; a literal operator: its suffix. A type: as its alias when it is a cv-unqualified typedef or alias-declaration type (not an alias template specialization), else as its class or enumeration when it is a cv-unqualified one. Otherwise (a specialization, a constructor, destructor, operator or conversion function, an unnamed entity, the global namespace, any other type or value) fails: "'R' has no identifier" |
  | `display_string_of(r)`, `u8display_string_of(r)` | implementation-defined; never fails; GCC 16.1's string (below): a type as GCC's diagnostic `%T`, a declaration as `%D`, a value or object as its source spelling (below), anything else as `^^…` |
  | `source_location_of(r)` | the location of the declared name, with an empty `function_name`; a reflection with no declaration (a fundamental or compound type, a value, the translation unit) gives `source_location{}`. The file name follows `-fkiln-source-path-style=` (below), then `-ffile-prefix-map=`/`-fmacro-prefix-map=` apply |

- **`display_string_of` of a type or declaration** is GCC 16.1's
  (gcc/cp/reflect.cc `eval_display_string_of` formats with the
  diagnostic `%T` and `%D` of gcc/cp/error.cc), reproduced by
  `kiln_sema/src/meta/gcc_names.rs`
  (`meta.reflection.names/display-string-of-gcc-types.cpp`, `oracle: gcc`):
  GCC's fundamental type names (`long unsigned int`, `short int`, `long
  long int`, `__int128 unsigned`); its declarator spacing (`const char*`,
  `int&`, `const int* const`, `int [3]`, `char* [2]`, `int (&)[2]`,
  `void(int, ...)`, `int (*)(int)`, `void (ns::S::*)() const`); class
  templates' arguments without trailing defaults and with `> >` between
  closers; inline and unnamed (`{anonymous}`) namespaces in scopes; a type
  alias or alias template specialization followed by ` {aka T}` with its
  typedefs stripped; a function with `static`/`virtual`/`constexpr`/
  `consteval`, return type, parameter types as declared (`std::size_t`),
  cv- and ref-qualifiers (`int ns::S::h() &&`); an enumerator scoped as
  its enumeration's scope (`ns::e1`, `ns::F::a`); the global namespace
  `::`; a template with its header (`template<class T, int N> struct
  ns::TT`, `template<class T> using ns::A = ns::TT<T, 5>`); a function
  template specialization or member of a class template specialization
  through its pattern with its bindings (`void ns::TT<T, N>::mem(T) [with
  T = int; int N = 3]`; followed by the typedefs the
  signature names, `; size_type = ...`; a member template of a class
  template specialization through the primary's member template,
  `void ns::E<T>::n(U) [with U = int; T = int]`); a constrained
  template's associated constraints (WP-1243, `kiln_sema/src/meta/
  gcc_names/requires.rs`, `meta.reflection.names/
  display-string-of-gcc-requires.cpp`): after the header, GCC's
  `pp_cxx_requires_clause` padding `  requires ` then the constraint and
  a space (`template<class T>  requires  is_integral_v<T> struct
  ns::OnlyInt`), the constraint being the template parameters'
  type-constraints in order (a constrained pack as `(... && C<Ts>)`, a
  constrained placeholder NTTP as `C<decltype(N)>` with header `auto N`)
  then the requires-clause, joined as one left-associated `&&` chain; a
  function's trailing requires-clause follows its declarator, before
  `[with ...]` (`void ns::g(T) requires  C<T> [with T = int]`). The
  expression prints as GCC's C precedence printer: a binary operator's
  operands bare at its own precedence level (right one level tighter),
  any other operand form (template-id, concept-id, `T::v`, fold,
  parenthesized looser operator) inside parentheses (`(integral<T>) &&
  (is_signed_v<T>)`, `(N + 1) * 2 > 0`); unary operators raw (`!(C<T>)`,
  `- N`); `sizeof (T)`, `alignof (T)`, `sizeof ... (Ts ...)`; a
  conditional without its colon (`c ? a b`, as GCC); folds as `(... &&
  C<Ts>)`, `(N + ... +0)`; a requires-expression as `requires(T t) {a +
  b;{t + t}noexcept -> decltype(auto) [requires std::same_as<<placeholder>,
  T>];requires sizeof (T) > 1;}`; a dependent `decltype(e)` as written;
  a variable template-id of an explicit specialization by its name only
  (`ns::pv`, GCC); a parameter as
  `<parameter x of F>` / `<unnamed parameter 2 of F>`; a base as `D: B`;
  the null reflection `<null reflection>`.
- **The file name of `source_location_of`** follows
  `-fkiln-source-path-style=` (spec/driver.md; WP-636), as
  `__builtin_FILE()` and `std::source_location::current()` do, so one
  translation unit spells a file one way (`SourceManager::
  presumed_file_name`). By default (`clang`) it is the presumed name as
  written (`-I //opt/inc/./` gives `//opt/inc/./x.h`;
  `meta.reflection.names/source-location-of-include-path-clang.cpp`).
  Under `gcc` it is GCC 16.1's: the absolute path of a file reached
  through `#include` has its empty and `.` components dropped and
  `dir/..` folded where `dir` is a real directory, not a symbolic link
  (`-I //opt/inc` gives `/opt/inc/x.h`); the main file, a `#line` name and
  relative paths are kept
  (`meta.reflection.names/source-location-of-include-path.cpp`).
  `__FILE__` keeps clang's spelling under both. The line and
  column are GCC's per-entity location
  (`meta.reflection.names/source-location-of-definition.cpp`): a class or
  enumeration where it is defined, a function or variable at its
  definition else its latest declaration, a parameter in the
  definition's declarator.
- **Known differences from GCC 16.1's names** (not yet matched):
  `display_string_of` of a scalar value or object reflection keeps
  kiln's spelling for characters, pointers and subobjects (below); a
  canonical template parameter GCC names after the first
  template that formed it (`is_integral<t_TYPE>` from another template)
  prints by its name in the template being printed (also inside a
  requires-clause, `std::is_integral<_Tp>::value`); a call to a function
  template-id in a requires-clause prints unparenthesized
  (`fn<T>()`; GCC `(fn<T>)()`); an abbreviated function template's
  invented parameters are numbered from 1 per template (`auto:1`; GCC
  counts across the TU); `source_location_of`
  of `^^S::f` for a member function or `^^f` for a function template is
  `0:0` in GCC (its operand stays an overload set there) and the
  function's location in kiln; an out-of-line definition's column is
  GCC's start of the qualified name, kiln's the unqualified name.
- **Matched to GCC 16.1 against the papers' wording** (so a generator
  sees the same values under both; to revisit): inside a `template for`
  body, `^^A` for a type alias `A` not declared in a template is the
  aliased type (`^^A == ^^int`; GCC's `tsubst` drops one alias level
  there, [expr.reflect] would keep the alias;
  `expr.reflect/alias-in-expansion-body.cpp`); `identifier_of` of a
  parameter of a function template specialization is the name in the
  template's definition, else its latest declaration, even when the
  declarations disagree (P3096 makes `has_identifier` false then;
  `meta.reflection.names/parameter-identifier-instantiation.cpp`).

- **`display_string_of` of a value or object** (WP-564, WP-1229): the
  diagnostic constant printer (spec/ast.md §9.3, template-argument style
  with no integer literal suffix and `enum_cast`; `Sema::value_spelling`,
  `value_spelling_gcc`, `object_spelling` in `kiln_sema/src/meta/names.rs`).
  A class value, and every constant inside a type's name, uses GCC 16.1's
  constant style (`PrintOptions::gcc`); a scalar value uses kiln's
  spelling with GCC's floating form (`PrintOptions::gcc_float`). GCC
  16.1 prints the same except where noted
  (`meta.reflection.names/display-string-of-{values,floating,class-args,reflection-args}*.cpp`):
  - integers in decimal with no suffix (`86400` for `86400L`, `5` for a
    `short`); `true`/`false`; `nullptr`;
  - characters as literals with diagnostics' escapes and the encoding
    prefix (`'a'`, `'\n'`, `u8'a'`; `signed`/`unsigned char` as `'A'`;
    GCC prints `'\012'` and the code of the prefixed and small ones);
  - an enumeration value as its enumerator, qualified as diagnostics
    qualify it (`Col::red`, `ns::e1` for an unscoped one), else as a cast
    of the fully scoped type (`(ns::F)9`);
  - floating point exactly as GCC's `pp_c_floating_constant` (WP-1229;
    `kiln_ast::consts::format_float_gcc`): the value rounded
    half-to-even to `max_digits10` = `2 + p*643/2136` significant digits
    (9 for `float`, 17 for `double`, 21 for x87 `long double`, 36 for
    binary128) as `d.ddd` with trailing zeros cropped to one after the
    point, then `e` and the signed decimal exponent (`1.0e+0`,
    `1.0000000000000001e-1`, `-2.5e-30`); zero `0.0` / `-0.0`; infinity
    `+Inf` / `-Inf`; NaN `+QNaN` / `-QNaN` (`SNaN` when the quiet bit is
    clear); then the type's suffix (`f` for `float`, `l` for
    `long double`, none for `double` and `__bf16`, `f16` `f32` `f64`
    `f128` `f32x` `f64x` for the `_FloatN[x]` types; `-Inff`). Inside a
    specialization's name the suffix is kept, so `W<1.0e+0f>` and
    `W<1.0e+0>` differ (binary128 on aarch64: `0.1L` is
    `1.00000000000000000000000000000000005e-1l`);
  - pointers as `&x` plus the designator (`&arr[1]`, `&fn`; GCC
    `& gv`, `fn`), member pointers `&S::m`, a null pointer of any type
    `nullptr` (GCC `0`);
  - a reflection as `^^` and its operand as GCC prints it in a constant
    (`Sema::gcc_reflection_operand` in `kiln_sema/src/meta/gcc_names.rs`):
    a type as `display_string_of` prints it without ` {aka ...}`
    (`^^const ns::S*`, `^^int [2]`); a namespace, alias, variable, data
    member or enumerator fully qualified, inline and unnamed namespaces
    shown (`^^ns::in::Q`, `^^ns::{anonymous}::An`, `^^::`); a non-member
    function with its parameter types (`^^ns::g(int, double)`), a member
    function by its qualified name (`^^ns::S::mf`); a member of a class
    template specialization through the specialization
    (`^^ns::TT<int>::t`); a class or function template or concept by its
    qualified name, a variable template with its parameters
    (`^^ns::vt<T>`); a value reflection as `^^` and its spelling
    (`^^5`, `^^1.5e+0f`, `^^ns::In{3}`), an object reflection as `^^`
    and the object (`^^gv`). So `derived_from_<^^tie::Anchor>`;
  - a class value (a class-type template argument, `reflect_constant` of
    a class value, a member of one) as `T{...}` with `T` as
    `display_string_of` prints the type, members in order, a `char`
    array as a string literal (`fixed_string<12>{"CopyInFiles"}`), any
    other array with its type (`Nested{Pt{4}, int [2]{5, 6}}`), a union
    as its active member's value with no designator (`U{2.0e+0f}`), a
    union with no active member or an empty class as `T()` (`E0()`), a
    null pointer member as `0` (`NP{0}`), a non-null pointer member as
    `&x` (GCC `((const int*)(& gv))`);
  - an object (`reflect_object`, a reference template argument) as its
    qualified name and designator (`ns::nv`, `arr[1]`, `pt.x`; GCC
    `pt.Pt::x`); a template parameter object as its value (`Pt{3}`).

  The `^^(value of type T)` / `^^(object of type T)` placeholder is kept
  only where no part has a source spelling: an unnamed class or closure
  type, a temporary or `Const` base, a subobject of a template parameter
  object, an undecodable subobject, a nested reflection of a base
  specifier, parameter, data member spec or annotation. `identifier_of`
  / `has_identifier` are unchanged (a value has no identifier).
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
  (`kiln_codegen/src/consts.rs`). A namespace-scope variable whose
  initializer compares or queries reflections but whose own type is not
  consteval-only (`constexpr bool b = type_of(^^x) == ^^int;`, an array
  of such bools, a `define_static_string` pointer) is emitted with its
  folded value and may be odr-used at runtime; the reflection operands
  never reach codegen (GCC 16.1 agrees; WP-505,
  `basic.types.general/consteval-only-namespace-scope-bool-run.cpp`).

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
- **A Transactional checkpoint alone is not a trial for the bridge**
  (WP-538). `MetaCtx::trial` is "a fold, or a Speculative checkpoint is
  open" (`Memory::in_speculative_trial`), not "any checkpoint is open".
  The checkpoint `substitute`/`can_substitute` opens only makes the
  nested evaluations' memory effects undoable; those evaluations are
  real ones: the constexpr initializer of a variable template
  specialization `substitute(^^V, args)` instantiates ([temp.inst]/8) is
  manifestly constant-evaluated, and its metafunction calls may
  instantiate and evaluate (the traits, the instantiating queries) and
  throw as anywhere else. The substitution itself keeps its SFINAE
  frame: an invalid substitution of the arguments is still a failure,
  not a hard error. A `Mutating` op still fails under any checkpoint (a
  rollback cannot undo a class definition), with the note "'X' cannot
  run while a substitution is being tried (std::meta::substitute)" when
  no trial hides it (follow-up: GCC 16.1 lets a consteval block of a
  class template instantiated there define an aggregate).
- A bridge failure with no note outside a trial gets the Oven's note
  "kiln: internal: 'X' failed without a reason": a silent failure is
  only for a trial, whose notes are dropped.
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
  | type | `typename [: r :]` [dcl.type.splice]; a bare `[: r :]` in a type-only context ([temp.res.general]/4: an alias-declaration, a namespace-scope or member decl-specifier, a member function's parameter, a trailing return type, a template parameter's default, a cast's or `new`'s type-id); in a template argument list only as `typename [: r :]` (a bare `[: r :]` forming a whole template-argument is ill-formed, [temp.names]/1: `parse_splice_template_arg_unparenthesized`, `temp.names/splice-template-argument-unparenthesized.cpp`; a splice that only starts an argument, `[: r :] + 1`, `([: r :])`, `[: r :]::type`, is an ordinary operand, `temp.names/splice-template-argument.cpp`; WP-830) | a type or a type alias | implemented (`dcl.type.splice/`) |
  | expression | `[: r :]` [expr.prim.splice] | a variable, function, enumerator, structured binding, value or object; a non-static data member or member function only as the operand of `&` (a pointer to member) or in an unevaluated operand ([expr.prim.id.general]/4) | implemented (`expr.prim.splice/`); a value or object as below ("Value and object splices", WP-550: `expr.prim.splice/value-*.cpp`, `object-*.cpp`), an object with no declaration included (WP-830) |
  | member access | `e.[: m :]`, `e->[: m :]` [expr.ref] | a member of `e`'s class or of one of its bases (data member, static member, member function, enumerator), or a direct base (P3293: the base subobject, same value category) | implemented; the base form is `basic.splice/p3293-base-subobject.cpp` |
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
- **Value and object splices** (`splice_object.rs`; WP-550). A value
  reflection splices to a prvalue of the value's type: the expression a
  constant template argument of that value is substituted as
  (`Sema::nttp_arg_expr`: an integer, enumerator, `bool` or character
  literal, a floating literal, `nullptr`, `&x` / `&f`, `&C::m`, or the
  `CXXReflectExpr` of an `info` value). An object reflection
  (`reflect_object`; `reflect_constant` of a class value and a
  class-type template argument reflect the template parameter object,
  [temp.param]/8, which `nttp_value_expr` makes when the value has none
  yet) splices to an lvalue of the object's type: a `DeclRefExpr` to the
  variable or template parameter object, then the derived-to-base
  conversions, member accesses and subscripts reaching the subobject at
  the reflection's byte offset (`Sema::object_lvalue`, recovered from the
  record layout: direct non-virtual bases first, then the virtual bases
  of an object that is not itself a base subobject, then fields, skipping
  bit-fields and references; members sharing an offset are tried in order
  until one holds an object of the pointee type). A pointer or reference
  template argument into a subobject is substituted the same way
  (`&outer.in.b[1]`). So `&[: reflect_object(v.m[i]) :] == &v.m[i]`, an
  object splice of a mutable variable is assignable, and one of a template
  parameter object or `constexpr` variable is `const`. The misuses get
  kiln's ordinary diagnostics (`value-object-errors.cpp`; GCC 16.1 rejects
  each with its own wording): "expression is not assignable" and "cannot
  take the address of an rvalue" for a value splice, "read-only variable
  is not assignable" / "cannot assign to variable ... with
  const-qualified type" for a const object, and `reflect_object` of an
  automatic variable is not a constant expression ("pointer to 'x' is not
  a constant expression"). An object no declaration names (a
  `PtrBase::Const` object: a `reflect_constant_string` /
  `reflect_constant_array` result, a `define_static_array` element, or a
  subobject of one) splices to `*p` of the constant pointer `p` to it
  (`Sema::const_object_lvalue`; a `const` template parameter object,
  [temp.param]/8), which the Oven and codegen read as any constant pointer
  (`expr.prim.splice/object-no-declaration-run.cpp`; WP-830). An object
  template argument of `substitute`/`reflection_template_arg` at a
  non-zero offset of its complete object is the same subobject lvalue
  (`meta.reflection.substitute/object-subobject-args-run.cpp`). A string
  literal object ([lex.string]/9), a `std::type_info` object
  ([expr.typeid]/1) and a lifetime-extended temporary splice the same
  way. `reflect_object` cannot reach them ([temp.arg.nontype]/6), but
  `object_of(^^R)` of a reference `R` usable in constant expressions is
  the object `R` refers to: the pointer value of `R`'s initializer,
  interned cv-unqualified like every object reflection, so
  `object_of(^^r) == object_of(^^v)` for `r` bound to `v`. String literal
  and type_info objects are const for `type_of`/`is_const`
  (`expr.prim.splice/p2-unnamed-static-object-run.cpp`, compared with
  GCC 16.1; WP-870 via WP-879). GCC gives the type_info object an
  internal pseudo type not derived from `std::type_info`; kiln gives it
  `const std::type_info`, the type [expr.typeid]/1 names.
- **Member functions.** A member-function splice is not a qualified-id, so
  `e.[: ^^B::f :]()` of a virtual `f` is a virtual call ([expr.call]/2; the
  [dcl.contract.func] example). `&[: ^^B::f :]` is a pointer to member.
- **Access.** Splices are not access-checked: a splice of a private member
  is valid from anywhere (P2996; GCC 16.1 agrees). Access is the business of
  the query that produced the reflection (`access_context`, WP-478).
  The name in a reflect-expression is: `^^C::m` of a member inaccessible at
  the `^^` is ill-formed ([class.access]/4; `expr.reflect/member-access*.cpp`,
  WP-830). GCC 16.1 checks data members and nested types but accepts a
  private member function's name; kiln follows the draft
  (`member-access-function-neg.cpp` has no oracle).
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
  Each hit counts in the Oven's totals (`EvalTotals::splice_fast`, noted
  by `OvenBridge::into_parts`), reported as the `total` line's
  `splice_fast` key of the constexpr report (spec/oven.md "Reporting
  protocol"; WP-869); every other operand is evaluated as a constant
  expression. 1,000 type, 1,000 expression and
  1,000 scope splices of three frozen operands start no evaluation
  (`basic.splice/frozen-operand-fast-path.cpp`: `evaluations<=4`, the
  three initializers and a static_assert, and `splice_fast>=3000`). Each distinct `R` in `[: R :]`
  costs one instantiation, as expected.
- **NTTPs.** A substituted `info` template argument is rebuilt as the
  `CXXReflectExpr` of the same entity (implemented).
- **Locals across a lambda** (`splice_local.rs`; WP-781). `^^x` naming
  a variable declared by an init-capture ([expr.reflect]/7.1.1) or a
  local entity for which a lambda scope intervenes between its
  declaration and the reflect-expression (/7.1.4) is ill-formed, at the
  name, even in an unevaluated operand. A potentially-evaluated splice
  designating such a local is ill-formed too ([expr.prim.splice]/2.1.4),
  with a note at the declaration; an enclosing `typeid` does not make it
  unevaluated (`Sema::enter_typeid_operand` marks the levels it adds, so
  `typeid([: r :])` is rejected while `typeid(sizeof([: r :]))` and
  `sizeof(typeid([: r :]))` are not), and `sizeof`/`decltype` operands
  are fine. "Intervenes" walks the lexical context from the current one
  to the local's home and fails at the first lambda call operator; the
  lambda scope begins after the lambda-introducer ([basic.scope.lambda]),
  so a lambda's declarator (parameter defaults, trailing return type) is
  inside it, except for the lambda's own parameters, while an
  init-capture's initializer is outside it. Neither form is an odr-use,
  so neither captures, and a capture-default does not help. A splice
  operand is a converted constant expression, so a constexpr local it
  names (the `info` variable `r` itself) is not odr-used either
  ([basic.def.odr]/5): a captureless lambda may splice a local `r`
  holding a reflection of a namespace-scope variable or a type. Both
  hold in contract predicates. GCC 16.1 rejects the same cases ("'^^'
  cannot be applied a local entity for which there is an intervening
  lambda expression"; "use of local variable with automatic storage from
  containing function in a splice expression")
  (`expr.reflect/p7-local-across-lambda.cpp`,
  `expr.prim.splice/p2-local-across-lambda{,-run}.cpp`, `oracle: gcc`).
  Not covered: /2.5's local of a non-enclosing function.

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

14. Each element of `annotations` has a type: "reflection does not have
    a type" (WP-484; `^^int`, a null `info`, a namespace).

**Annotations** (P3394's `data_member_options::annotations`, a
`vector<info>`; WP-484, GCC 16.1's semantics). The Oven reads the vector
through the `vector<info>` recipe (§3.5). Each element stands for its
constant: a value reflection or a reflection of a class-type template
parameter object is kept as it is, an annotation reflection becomes its
value, and any other reflection with a type (a variable, an object)
becomes `constant_of` of it. The list is interned with the description,
so equal lists give the same reflection. `define_aggregate` attaches one
annotation per element to the injected member, in order, with no source
location (`source_location_of` gives line 0), so `annotations_of` of the
member returns them and each `constant_of` is the element's constant.

The result is a `DataMemberSpec(DmsId)` reflection. The description
(type, name `Symbol`, alignment, width, `no_unique_address`, the
annotation list) is interned by content in the Oven
(`kiln_oven/src/meta/define.rs`, `DmsTable`), so two calls with the same
options give the same reflection, whichever form the name was spelled
in. `is_data_member_spec` tests for that kind.

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
   `inst.eval_loc` and `MetaCtx::plainly`, WP-473), and is not a
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

## 7. Annotations (syntax and storage WP-481; queries WP-478)
- Syntax: `[[=constant-expression]]` in an attribute-specifier-seq on a
  declaration ([dcl.attr.annotation]). An annotation may not be combined
  with a `using` prefix. The grammar is spec/parse.md "Annotations" and
  spec/annotations.md §2 (WP-481); the gaps still open against P3394R4
  are spec/annotations.md §9.
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
- A namespace alias carries no annotations (its definition admits none),
  and `annotations_of` of one does not see the aliased namespace's: it is
  empty, not an error (WP-573).
  `constant_of` and `extract` on an annotation yield its value, and
  `is_annotation` tests the kind.
- An annotation has a type (P3394R4 *has-type*; WP-571): `type_of(a)` is
  `type_of(constant_of(a))`, the type `T` of the underlying constant
  (`Annotation::written_type`). A class `T`'s constant is a template
  parameter object, so `type_of(a)` is `const T` and `is_const(a)` holds;
  a scalar `T` (`[[=1]]`) is cv-unqualified. GCC 16.1 agrees
  (`meta.reflection.annotation/type-of-annotation.cpp`).

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

### 8.3 Mangling (WP-501)
A reflection value inside a template argument of an emitted symbol mangles
structurally, by naming its reflectee (`kiln_abi/src/mangle/reflect.rs`). It
follows GCC 16.1's `write_reflection` (the cxx-abi issue #208 proposal) kind
for kind. Equal reflections in two TUs give the same string, so the
specialization keeps its ordinary linkage. The driver flag
`-fkiln-reflection-mangling=<gcc|kiln|kiln-hashed>` (spec/driver.md) picks
the spelling; the default is `gcc` (WP-909, the operator's 1.0 decision
on WP-879's study below), link-compatible with g++-16. `kiln` and
`kiln-hashed` are experimental opt-ins (docs/STABILITY.md). The PCH and
module-file option fingerprints record the effective form, so a unit
built without the flag matches a consumer built without it.

- **Grammar.**

  ```
  <expr-primary> ::= L <info-type> <reflection> E
  <info-type>    ::= u6__info                 # kiln, kiln-hashed
                 ::= Dm                       # gcc
  <reflection>   ::= <kind> <operand>         # kiln, gcc (table below)
                 ::= h <32 lowercase hex>     # kiln-hashed
  <compact>      ::= _ | <n-1> _              # index n
  ```

  `<info-type>` is also the mangling of the type `std::meta::info`
  everywhere (`template <class T>` with `T = info` gives `_Z1kIu6__infoEiv`
  in kiln mode, `_Z1kIDmEiv` in gcc mode). `u6__info` is a vendor type and
  so a substitution candidate. `Dm` is a builtin and is not one. The two
  modes therefore number later substitutions differently: `two<^^int,
  ^^int>` is `_Z3twoILu6__infotyiELS0_tyiEEiv` (kiln) and
  `_Z3twoILDmtyiELDmtyiEEiv` (gcc). `h<^^P>(Box<P>, P)` is
  `_Z1hILu6__infoty1PES1_Ei3BoxIT0_ES3_` and `_Z1hILDmty1PES0_Ei3BoxIT0_ES2_`.
  Apart from `<info-type>` and that numbering, the two modes write the same
  `<reflection>` bytes. The one exception is `ta` (below). `kiln-hashed`
  writes the FNV-1a-128 of the kiln-mode `<reflection>`, mangled in a fresh
  substitution context so that the hash depends on the reflectee alone. It
  is a prototype for bounding name length and is not link-compatible with
  anything.
- **Per kind.** Each row is `_Z1fIL` + body + `EEiv` for `template <info R>
  int f()`. The gcc body replaces `u6__info` with `Dm`. Every gcc row is
  byte-identical to `g++-16 -freflection`
  (`crates/kiln_driver/tests/reflection_mangling.rs`, the `KINDS` table, all
  41 rows).

  | kind | operand | kiln body |
  |---|---|---|
  | `nu` null | `info{}` | `u6__infonu` |
  | `ty` type | `^^int`, `^^const int*`, `^^TT<int>` | `u6__infotyi`, `…tyPKi`, `…ty2TTIiE` |
  | `ta` alias | `^^I`, `^^ns::A` (alias template spec) | `…ta1I_i`, `…ta2ns1A_NS1_1SE` (gcc `…NS0_…`) |
  | `ns` / `gs` | `^^ns`, `^^::` | `…ns2ns`, `…gs` |
  | `na` ns alias | `^^na` | `…na2na` |
  | `vr` variable | `^^var`, `^^S::sm` | `…vr3var`, `…vrN1S2smE` |
  | `fn` function | `^^g`, `^^S::mf`, ctor, dtor | `…fn1gv`, `…fnN1S2mfEv`, `…fnN2ns1SC4Ev`, `…fnN2ns1SD4Ev` |
  | `dm` data member | `^^S::dm`, a member of an anonymous union, an unnamed member | `…dm1S2dm`, `…dm2ns1S2u1`, `…dm2ns1S1b` |
  | `en` enumerator | `^^S::e1`, `^^EC::b` | `…en1S1E2e1`, `…en2EC1b` |
  | `ct` `ft` `vt` `at` `co` | class, function, variable, alias templates, concept | `…ct2TT`, `…ft2tf`, `…vt2tv`, `…at2TA`, `…co1C` |
  | `sb` structured binding | `^^sb2` | `…sb3sb2` |
  | `pa` parameter | `parameters_of(^^pfn)[1]` | `…pa0_3pfnid` (`<compact>` index, then the function) |
  | `ba` base | `bases_of(^^D2)[1]` | `…ba0_2D2` |
  | `vl` value | `42`, `'c'`, `true`, `EC::a`, `nullptr`, `&arr[1]`, `^^int` | `…vlLi42E`, `…vlLc99E`, `…vlLb1E`, `…vlL2EC0E`, `…vlLDnE`, `…vladixL_Z3arrELl1E`, `…vlLS0_tyiE` (gcc `…vlLDmtyiE`) |
  | `ob` object | `var`, `arr[1]`, a class value `Lit{3}` | `…obL_Z3varE`, `…obixL_Z3arrELl1E`, `…obL_Z17_ZTAXtl3LitLi3EEEE` |
  | `ds` data member spec | `{.name="x"}`, `{.bit_width=3}` | `…dsi_1x___`, `…dsi___3_` |
  | `an` annotation | `annotations_of(^^v)[0]` | `…an_` |

  `ds` is `ds <type> _ [<source-name>] _ [<align>] _ [<width>] _ [n]
  <annotation template-arg>*`, where `n` marks `no_unique_address`. A
  constructor writes `C4` and a destructor `D4`, the unified structor of
  GCC, whatever variant is being emitted. In kiln mode an alias-template
  specialization writes the alias's own arguments. In gcc mode it writes
  the arguments of the underlying class specialization, as g++-16 does,
  when those determine the alias's (every alias parameter appears
  deducibly in the defining-type-id, `alias_args_recoverable`), and the
  alias's own otherwise, where g++-16's spelling collides (spec/abi.md
  §8.14, WP-1250).
- **Linkage.** A reflection takes its reflectee's linkage
  (`reflect::reflection_externally_visible`, which `value_externally_visible`
  calls). A namespace-scope or class-member entity, a type, or a value is
  external when the entity it names is. A local entity is internal, and so
  is an annotation (`an`), because its index is TU-local. A data member
  spec is external iff its type and all its annotations are. An internal
  reflection makes the specialization internal, together with everything
  mangled under it (the unique-external rule of spec/abi.md §8.22).
  Otherwise `f<^^int>`, `S<^^int>::count`, their guards, vtables and RTTI
  are `linkonce_odr` in a comdat and fold across TUs, as for a type
  argument (`two_translation_units_link` in the driver test).
- **Dependent operands.** A reflect-expression that is still dependent
  where a signature is mangled writes `L <info-type> ty <type> E` for a
  dependent type, and `L <info-type> de <expression> E` for a dependent
  name. `template <class T> void f(K<^^T>)` gives
  `_Z1fI1SEv1KILu6__infotyT_EE`, and `g(K<^^T::x>)` gives
  `_Z1gI1SEv1KILu6__infodesrT_1xEE`. The gcc spellings
  (`…KILDmtyT_EE`, `…KILDmdesrT_1xEE`) match g++-16. A dependent alias
  template-id is `ta` as above, not the aliased type (`^^AT<T>` is
  `…KILDmta2ATIPS0_E_2TTIS2_EEE`). A dependent function or variable
  template-id is `de` with its qualifier and template arguments
  (`…KILDmde1gIS0_EEE`, `…KILDmdesr2nsE2nvIS0_EEE`): the parser records a
  qualified variable template-id's qualifier on the operand, as for any
  qualified id, so `^^ns::nv<T>` and `^^nv<T>` are different signatures.
  Where g++-16 drops those arguments or that qualifier its symbols
  collide, and kiln keeps them in both modes (spec/abi.md §8.14 "Known
  divergences from GCC 16.1", WP-1250).
- **GCC link compatibility.** In gcc mode, kiln objects and g++-16 objects
  link against each other for reflection-keyed symbols. `gcc_link_compat`
  requires every `Dm` symbol defined by both compilers to be equal, with at
  least 25 of them. The corpus test
  `tests/conformance/expr.reflect/reflection-mangle-gcc-run.cpp` prints the
  `typeid(...).name()` of a class template keyed by every kind and is
  compared with g++-16's output (WP-879). `gcc_dependent_template_id_link`
  links a kiln object defining signatures with a dependent reflected
  alias or variable template-id against a g++-16 caller (WP-1250). The
  reflection-specific exceptions are the GCC 16.1 collisions of
  spec/abi.md §8.14. The remaining name differences
  are general ABI divergences, not reflection ones (spec/abi.md §8):
  - GCC writes `L` before an anonymous namespace's members
    (`N12_GLOBAL__N_1L5anonvE`);
  - GCC writes an empty non-type pack as `JE` without `TpTnDm`;
  - GCC writes an `sr` whose qualifier is a template-id with one fewer `E`
    and a different substitution count (`…EE1cfp_ES1_`, while kiln and
    clang write `…EEE1cfp_ES0_`);
  - GCC gives template static data members `STB_GNU_UNIQUE` (`nm` letter
    `u`, where kiln writes `V`).
- **`type_info` of the info type.** The C++ runtime defines no `type_info`
  for `std::meta::info`, so `typeid(info)` defines `_ZTI<info-type>` and
  `_ZTS<info-type>` where used, `linkonce_odr` in a comdat with
  `__fundamental_type_info`'s vtable, as g++-16 does for `_ZTIDm`, in every
  form (WP-879; before it kiln referenced an undefined external one).
  `kiln_codegen/tests/reflection_mangle.rs`, `expr.reflect/typeid-info-run.cpp`.
- **Cost.** These numbers are for bench shape S9 (spec/bench.md):
  - N types `lib::Tnnnn`;
  - `f<^^Tnnnn>()` and `S<^^Tnnnn>::count` for each type;
  - `f<^^W<W<W<Tnnnn>>>>()` for every fourth type.

  Each figure is the median of 11 runs on aarch64, as wall ms / max RSS
  KiB. `base` is main before WP-501, which used the TU-local id literal
  with internal linkage.

  | N | mode | base | kiln | gcc | kiln-hashed | g++-16 |
  |---|---|---|---|---|---|---|
  | 500 | `-c` | 54 / 64024 | 61 / 67828 | 60 / 67928 | 62 / 67660 | 129 / 48492 |
  | 2000 | `-c` | 196 / 74468 | 224 / 88460 | 221 / 88200 | 231 / 89492 | 442 / 102228 |
  | 500 | `-S -emit-llvm` | 23 / 17092 | 25 / 18052 | 25 / 17860 | 26 / 18052 | 109 / 48492 (`-S`) |
  | 2000 | `-S -emit-llvm` | 82 / 25716 | 91 / 28036 | 90 / 27652 | 96 / 28996 | 336 / 100360 (`-S`) |

  The reflection symbols number 1750 at N=500 and 7000 at N=2000. Their
  length (max / mean) is 28 / 24.7 in base, 50 / 37.1 in kiln mode,
  44 / 31.1 in gcc mode (g++-16 is the same), and 57 / 54.1 in kiln-hashed
  mode.

  The overhead is linear in N, at about 4 µs per specialization. It comes
  from two sources:
  - **Cross-TU linkage itself.** Seven `linkonce_odr` comdat symbols per
    type, instead of internal ones. The same program keyed by types
    (`template <class R>`, `f<lib::Tnnnn>`) costs the same as base in total
    (82 ms).
  - **Longer names.** The new spelling run with internal linkage costs
    1.05× wall and 1.03× RSS against base. Against the type-keyed program,
    codegen takes +3.8 ms and +0.3 MB at N=2000, in proportion to the
    roughly 20% larger IR text.

  Sema is unchanged (55.5 ms in both). The WP-501 pass bar (1.05 wall,
  1.02 RSS against base at N=2000) is therefore not met: the ratios are
  1.11 / 1.09 for `-S -emit-llvm` and 1.14 / 1.19 for `-c`. The bar
  compared against a TU-local form that could not link across TUs. Against
  a type-keyed template of the same linkage shape, codegen RSS is within
  3%.
- **Cost on deep keys (WP-879).** Keys that are nested template-ids
  (bench shape S10; the study's generator varies N, depth D and width W,
  spec/landed/sema/WP-879.md has the command lines and every number):
  - **Length.** The mangled name grows linearly in D × W, and the forms
    differ by a constant: `Dm` is 6 bytes shorter than `u6__info` per
    reflection literal, so the gcc form is 1-5% shorter on these keys.
    kiln-hashed is fixed at 57 bytes.
  - **Time.** Mangling costs about 30 ns per emitted byte in both forms
    (measured in-process, best of 30), linear in N, D and W. Neither form
    is superlinear. At N=500, D=6, W=6 it is 12.7 ms (kiln) and 12.8 ms
    (gcc) of about 200 ms for `-c`. kiln-hashed costs the same, because it
    hashes the full name.
  - **Substitutions.** A key whose arguments repeat the previous level
    (the source doubles per level) grows its name by only 16 bytes per
    level, and mangling time grows with the name, not with the source.
  - **Whole compile.** Wall time, CPU time and peak RSS of `kiln` and
    `gcc` mode agree within run-to-run noise, at every size measured.
    The objects differ only by the symbol bytes.
- A consteval-only value reaching codegen stays the error of §4.1, not a
  mangling question: a non-template `info` variable with static storage
  that is not `constexpr` is rejected before any symbol exists
  (`expr.reflect/reflection-mangle-neg.cpp`).

## 9. Tests and status
- **Format.** Reflection tests are kiln goldens: `//@ args: -freflection
  -I tests/support`, `skip(clang)`, `oracle: none`, with `strict-diags`
  where they check diagnostics. Each test names the IS section it
  exercises. A test that needs clang 22 behaviour uses `//@ requires:
  clang>=22` or `//@ min-oracle: 22`. A test whose verdict GCC 16.1 can
  check carries `oracle: gcc` (WP-483), which merge-queue CI runs
  (WP-502).
- **Directories.** The spike's tests stay in `tests/conformance/reflection/`.
  New tests go by IS clause:

  | Directory | Covers |
  |---|---|
  | `expr.reflect/` | the `^^` operator (§2) |
  | `basic.splice/` | splices in every position (§5) |
  | `dcl.pre/consteval-block-*.cpp` | consteval blocks, plainly constant-evaluated (§4.2) |
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
- **Bench.** The `bench/reflect/` shapes and the scaling gate are WP-483's
  (S1–S6; S7/S8 WP-551, S9 WP-501, S10 WP-879; spec/bench.md).
- **Status.** `docs/STANDARD_SUPPORT.md` "C++26 reflection" has one
  row per WP-470..485. The spike's findings are frozen in
  docs/status/reflection-spike.md.
