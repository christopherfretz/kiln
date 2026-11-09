# M6 — C++26 reflection: plan and decisions

Supervisor-owned plan for ROADMAP M6, derived from the WP-267 spike review on
2026-11-03. Work packages are BACKLOG WP-470..485 (the M6-NN labels below map
1:1, M6-01 = WP-470). `spec/reflection.md` (WP-470) is the normative contract;
this file is the rationale and the dependency graph.

## Decisions taken (2026-11-03)

- **D1 Reference compiler** (revised 2026-11-03). GCC 16.1.0 is provisioned
  on the dev host (`.toolchain/gcc-16.1/bin/g++-16`, outside this repo;
  `scripts/env.sh` exports it as `KILN_GCC`) and goes into the CI path:
  `oracle: gcc` tests (spec/harness.md "GCC oracle") and the deterministic
  reflection counter gates (`oven:` budgets) run in merge-queue CI. The
  vs-GCC perf table, the `--scaling` sweeps and the private corpora stay
  off the queue (manual or nightly on the dev host). CI wall time must not
  grow for it: the GCC-bearing image is adopted only when it pays for its
  own pull (docs/status/perf.md "CI time"); until then CI leaves `KILN_GCC`
  unset and the GCC comparisons are skipped, never failed.
- **D2 `<meta>` vehicle.** libstdc++ 16.1's `<meta>` is the production target
  and is available now (`__cpp_lib_reflection 202603L`). Until WP-484 consumes
  it, `tests/support/kiln_meta.h` is the test vehicle. The p2996 fork's
  `<experimental/meta>` shim is optional and advisory.
- **D3 Authoritative text.** The C++26 IS wording as implemented by GCC 16.1
  (`__cpp_impl_reflection 202603L`); names and signatures follow GCC 16.1's
  `<meta>` where a paper revision differs, with the difference noted in
  `spec/reflection.md`.
- **D4 `members_of`** includes implicitly declared special members, as the IS
  requires and GCC 16.1 does (verified: `members_of(^^S)` for `struct S{int
  a;}` has more than one element). The first query forces their declaration.
- **D5 Reflection-table growth** is charged to `-fconstexpr-memory` through a
  `refl_bytes` counter and interned by content; the per-evaluation overlay
  table is an M7 item.
- **D6 `-freflection`** stays opt-in through M6 (GCC 16 also requires it).
- **D7 `__kiln_define_member`** requires `-freflection` and warns under
  `-Wkiln-extension`.

The rest of this file is the planner's breakdown, kept verbatim apart from the
decisions above.

Inputs read: ROADMAP M6/M7, ARCHITECTURE "The Oven", AGENTS.md, VALIDATION.md, spec/ast.md §10, spec/sema.md §8a, spec/oven.md "Reflection runtime", spec/parse.md §5.2, spec/lex.md, spec/types.md, docs/status/reflection-spike.md, cxx-features.md, BACKLOG WP-030/102/103/139/283/454/467, and the spike code (`kiln_ast/src/reflect.rs`, `kiln_sema/src/reflect.rs`, `kiln_oven/src/vm/meta.rs`, `compile.rs:5227 meta_call`, `lib.rs:194 MetaOp`, `ty.rs:372 intern`/`:646 refresh`, `vm.rs:1675` memo, `mem.rs:1554 TrialMode`). Main at 466032b7f. WP-283 (R1–R5) has not started.

Facts that change the spike's estimate:
- **P3068 constexpr exceptions is done** (STANDARD_SUPPORT.md). The spike called it the largest unknown and part of the critical path. P3560 (`std::meta::exception`) is now an ordinary WP.
- **libc++ 22 ships no `<meta>`.** `.toolchain/libcxx-llvmorg-22.1.8/libcxx/include/experimental/` holds only `__simd iterator memory propagate_const simd type_traits utility`. The experimental `<meta>` exists only in the Bloomberg p2996 fork's libc++, and that fork is not in the container. **GCC 16.1 is not installed either**: only `g++-14` and `/usr/include/c++/14`. Both are open questions (§6).
- `TrialMode::Full` (`mem.rs:1562`) exists but nothing uses it yet. Reflection (`can_substitute`) can be its first real user, which closes the WP-139 item.

## 1. Scope

Normative target: the C++26 IS wording of P2996R13 plus the papers adopted with it. The stable names below are P2996R13's. Before writing spec/reflection.md, **check every section name and function name against the final IS draft**, because NB-comment resolutions renamed some things. **(verify)** marks anything whose IS status I am not sure of.

| Area | IS surface | Spike today |
|---|---|---|
| Tokens [lex.operators] | `^^`, `[:`, `:]` (only with `-freflection`) | done (`lexer.rs:1368`) |
| `std::meta::info` [basic.fundamental] | `decltype(^^::)`, scalar, structural (NTTP), `==`/`!=`, consteval-only type [basic.types.general] | value, NTTP, `==`, `?:` done; consteval-only rules and `^^::` missing |
| Reflection operator [expr.reflect] | `^^` on `::`, namespace (and alias), type-id (alias kept), id-expression (variable, function, enumerator, data member, structured binding), template-name, concept; overload set ill-formed; reflections of local entities | types, namespaces, vars, non-template functions, members; aliases canonicalized (**diverges** from ast §10); template-ids unsupported |
| Splices [basic.splice] | expression `[: r :]` [expr.prim.splice]; type `typename [: r :]` [dcl.type.splice]; scope `[: r :]::` in a nested-name-specifier; template `template [: r :] <args>` [temp.names]; member access `e.[: m :]` / `e->[: m :]` [expr.ref], including base subobjects (P3293); `using namespace [: r :]` **(verify)**; dependent splices everywhere | type splice and template-argument splice, non-dependent only |
| Consteval blocks P3289 [dcl.pre] | `consteval { ... }`, "plainly constant-evaluated", injection points and evaluation context [expr.const] | none (the spike gates on sema's `inst.eval_loc`) |
| Names [meta.reflection.names] | `has_identifier`, `identifier_of`, `u8identifier_of`, `display_string_of`, `u8display_string_of`, `source_location_of` | `name_of -> const char*` (not IS) |
| Queries [meta.reflection.queries] | `type_of`, `object_of`, `constant_of`, `parent_of`, `dealias`, `template_of`, `template_arguments_of`, `has_template_arguments`, `operator_of`/`symbol_of`/`u8symbol_of` + `enum class operators`, about 80 `is_*`/`has_*` predicates (access, virtual/final/override, deleted/defaulted/user-provided, linkage and storage duration, entity kinds, special members, templates, `is_value`/`is_object`, `is_complete_type`, `is_enumerable_type`, `is_bit_field`, `is_mutable_member`, `has_default_member_initializer`, `has_parent`, …) | `type_of` only |
| Parameters P3096 | `parameters_of`, `variable_of`, `return_type_of`, `is_function_parameter`, `is_explicit_object_parameter`, `has_default_argument`, `has_ellipsis_parameter` | none |
| Access P3547 [meta.reflection.access.context] | `access_context::{current, unprivileged, unchecked}`, `.via(cls)`, `scope()`, `designating_class()`, `is_accessible`, `has_inaccessible_{nonstatic_data_members,bases,subobjects}` | none |
| Members [meta.reflection.member.queries] | `members_of(r, ctx)`, `bases_of`, `static_data_members_of`, `nonstatic_data_members_of`, `subobjects_of` **(verify)**, `enumerators_of`, all returning `vector<info>` | `__kiln_members_count`/`__kiln_member_at` → `member_list<N>` |
| Layout [meta.reflection.layout] | `member_offset{bytes,bits}`, `offset_of`, `size_of`, `alignment_of`, `bit_size_of` | none |
| Values [meta.reflection.extract], [meta.reflection.result] | `extract<T>`, `reflect_constant`, `reflect_object`, `reflect_function` | none |
| Static storage P3491 | `reflect_constant_string`, `reflect_constant_array`, `define_static_string`, `define_static_array`, `define_static_object`, `std::is_string_literal` **(verify placement)** | none (WP-454 lists `define_static_array`) |
| Substitution [meta.reflection.substitute] | `can_substitute`, `substitute` (any `reflection_range`) | none |
| Definition [meta.reflection.define.aggregate] | `data_member_options` (`name_type`, `alignment`, `bit_width`, `no_unique_address`), `data_member_spec`, `is_data_member_spec`, `define_aggregate(cls, R&&)` | kiln extension `__kiln_define_member` (kept, operator decision) |
| Type traits [meta.reflection.traits] | `is_*_type` for every [meta.unary] category and property, `is_same_type` and the other relations, `is_*_invocable_type`, transformations (`remove_cvref`, `decay`, `common_type`, `underlying_type`, `invoke_result`, …), `rank`, `extent`, `tuple_size`/`tuple_element`, `variant_size`/`variant_alternative`, `type_order` **(verify; P2830 may be `std::type_order` only)** | none |
| Annotations P3394 [dcl.attr.annotation] | `[[=constant-expression]]` on declarations, `annotations_of`, `annotations_of_with_type`, `is_annotation`, `constant_of`/`extract` on annotations | none |
| Errors P3560 [meta.reflection.exception] | failures throw `std::meta::exception` (`what`, `u8what`, `from`, `where`); an uncaught one makes the expression non-constant | trap with notes (cannot be caught) |
| Expansion statements P1306 | done (WP-447). Reflection adds iteration over `define_static_array(...)`, and needs WP-454/467 follow-ups | done |
| Macros | `__cpp_impl_reflection`, `__cpp_lib_reflection` (202506L), `__cpp_impl_consteval_blocks`?, `__cpp_lib_define_static`?, annotations macro? **(verify all values)** | none predefined |

**Out of scope for M6**:
- token injection (P3294; not in C++26)
- reflection of attributes (P3385), expressions or statements
- `define_class` beyond aggregates; function or body injection
- reflection mangling interop *before* WP-102 decides (a placeholder ships, §2.8)
- the clang-wording long tail of reflection diagnostics (operator 11-03 rule)

`__kiln_define_member` stays as a documented kiln extension: `-Wkiln-extension`, rebased onto the new dispatch.

**Spike vs IS contradictions found in the code:**
- (a) `name_of` (`tests/support/kiln_meta.h`, `MetaOp::NameOf`) is not IS. Replace it with the names family and keep `name_of` only in the test header.
- (b) `reflect_members` (`kiln_sema/src/reflect.rs:296`) drops implicitly declared special members. P2996 includes them in `members_of`, which forces them to be declared **(verify)**. Its doc comment gives laziness as the reason. The fix is to force the declarations on the first `members_of`.
- (c) `members_of` takes no `access_context`. In the IS every enumeration query takes one.
- (d) `meta_intrinsic_of` (`reflect.rs:166`) requires `std::meta` to sit directly under the TU and matches by name only. libc++ declares `std::__1::meta`, and the IS has overloads. It must skip inline namespaces and match name plus signature.
- (e) Every meta op increments `run.impure` (`vm/meta.rs:38`). That defeats memoization of every reflection query (perf, §3.3).
- (f) Errors are uncatchable traps. P3560 requires a catchable `std::meta::exception`.
- (g) `^^Alias` canonicalizes (`reflect.rs:39`), against ast §10 and `dealias`.
- (h) The spike's mutation adds to a class mid-definition. IS `define_aggregate` completes an *incomplete* class from a consteval block. The extension keeps the spike's rule. The IS path is new (M6-11).

## 2. Contract outline: `spec/reflection.md` (new; written by M6-01)

**Supersedes:**
- spec/ast.md §10 becomes a 5-line pointer.
- spec/sema.md §8a moves into reflection.md §4–§7. sema.md keeps a pointer.
- spec/oven.md "Reflection runtime" moves into reflection.md §3. oven.md keeps the bridge signatures in "Interface" and a pointer.
- parse.md §5.2 keeps the grammar only and points to reflection.md §5 for semantics.
- lex.md and types.md references stay.
- docs/status/reflection-spike.md is frozen as history.

1. **Scope and conformance.** The paper list of §1, the macro values, the flag policy (`-freflection`, later on by default at `-std=c++26` **once the IS ships; operator call**), and the extension list.
2. **Representation.**
   - `ReflectionId(u32)` indexes `ReflectionTable`. The Oven value is `index-1` in an 8-byte `TyKind::Reflection`, so `info{}` is null.
   - `Reflectee` grows to: `Null`, `Type(QualType /*alias sugar kept at top*/)`, `Decl(first DeclId)`, `Namespace(DeclId)`, `NamespaceAlias(DeclId)`, `Template(TemplateNameId)`, `Value(ConstValueId /*typed*/)`, `Object(ConstValueId /*static-storage pointer*/)`, `BaseSpec(derived first DeclId, u32 index)`, `DataMemberSpec(DmsId)`, `Annotation(AnnId)`. Each variant fits in ≤ 12 bytes and the size is asserted.
   - **Identity:** two reflections are equal iff their ids are equal. The rules that make this true are: declarations compare by first declaration (`Sema::same_entity`, R3); types are canonical except for the top-level alias; values are interned by type and content in ConstStore; annotations are distinct per occurrence even when their values are equal.
   - **Lifetime:** ids are valid for the whole TU (append-only arenas, ast §2.4). They freeze to `Scalar::Reflection` and survive across evaluations, as NTTPs, and in memo keys. §3.4 says how table growth is accounted.
   - **Typed copy (decided):** a byte-wise copy of `info` is a typed copy. `memcpy` and `bit_cast` between `info` and a non-`info` type are rejected. The shadow has no kind bits, so the R5 promise is deleted from oven.md.
3. **Oven runtime.**
   - One bridge entry point: `SemaBridge::meta(op: MetaOp, args: &[u128], out: &mut MetaOut, ctx: &MetaCtx, oven: &mut Oven) -> Result<(), MetaError>`.
   - `MetaOp` is a family enum: `Query(QueryOp)`, `Trait(TraitOp)`, `Member(MemberOp)`, `Value(ValueOp)`, `Define(DefineOp)`, `Annot(AnnotOp)`, `Access(AccessOp)`, `Template(TemplOp)`, `Ext(ExtOp)`. Each sub-enum lives in its own file, so the family WPs do not collide (the same pattern as kiln_diag's area files).
   - Operands become a variable-length register list; `compile.rs:5230` `take(3)` goes away.
   - Every op has a **purity class**: `Stable`, `PointDependent` or `Mutating` (§3.3).
   - `MetaCtx` carries the evaluation point (scope DeclId plus a position) for `access_context::current()` and namespace `members_of`. It also carries the `plainly` bit (R2).
   - Results: a scalar, an `info`, or a **materialized class object** (`vector<info>`, `string_view`, `u8string_view`, `member_offset`, `access_context`, `source_location`), built by a recipe that sema resolves at compile time (M6-06).
4. **The consteval boundary.**
   - consteval-only types: a variable of a consteval-only type must be `constexpr` or live in an immediate function, and no consteval-only value may reach codegen [basic.types.general], [expr.const].
   - Immediate escalation of `^^` and splices.
   - "Plainly constant-evaluated" ([expr.const]): consteval blocks and the initializers the IS lists **(verify list)**. It is the only context where injection is allowed.
   - A trial (`try_fold`, `__builtin_constant_p`, SFINAE probing) never mutates. A `Mutating` op in a trial fails silently, which keeps the spike's rule but derives it from `run.plain` instead of `inst.eval_loc`.
5. **Splicing, per grammar position.**
   - The operand is a converted constant expression of type `info`.
   - Resolution is designated-entity: what each position accepts (type, expression, template, namespace or scope, member, base subobject) and the error for each misfit.
   - Dependent splices are a `Splice` type node, a `CXXSpliceExpr` and a splice NNS, all substituted in templ/subst.
   - Fast path: when the operand is a substituted NTTP or another frozen constant, read the id without starting the Oven.
6. **`define_aggregate`.**
   - Preconditions: the class is incomplete and not being defined; the call is plainly constant-evaluated; the injection point lies in the right scope [expr.const] **(verify exact rule)**; the specs are valid; the names are unique.
   - Effect: sema appends `INJECTED` `Field`s (with the injection point in a cold map) and runs `act_on_finish_record`. **The class is complete when `define_aggregate` returns**, so the rest of the same evaluation sees it complete. The Oven upgrades that one provisional slot at once, not at the next evaluation.
   - `data_member_spec` is content-interned, so equal options give equal reflections **(verify)**.
   - The `__kiln_define_member` extension rules (current sema §8a) live here.
7. **Annotations.**
   - The value is copy-initialized, of structural type, and frozen. It is stored per declaration (first declaration plus redeclarations, in order) in a cold `DeclId → ListId<AnnId>` map.
   - Annotations are instantiated with templates.
   - `annotations_of` order is declaration order **(verify)**.
8. **Error model and ABI.**
   - Every bridge failure is a `MetaError { notes, kind }`. With P3560 the Oven throws `std::meta::exception(what, from = ^^fn, where)` through the P3068 path. A silent failure stays silent for trials.
   - Diagnostics follow GCC 16.1's shape where kiln has no clang wording.
   - **Mangling placeholder:** a reflection inside a template argument of an emitted symbol mangles as a kiln-vendor form, deterministic within the TU and cached per `ReflectionId`, until WP-102 picks GCC's Itanium form. It is flagged in spec/abi-mangle as "not link-compatible". *Superseded (WP-501):* the mangling is structural, GCC 16.1's `L Dm <kind> <entity> E` by default (WP-909) or `L u6__info … E` under `-fkiln-reflection-mangling=kiln`, with the reflectee's linkage, so equal reflections link across TUs (spec/reflection.md §8.3).
9. **Tests and status.** Directory map: `tests/conformance/{expr.reflect, basic.splice, meta.reflection.*, dcl.attr.annotation, dcl.pre.consteval-block}/`. Plus the `docs/STANDARD_SUPPORT.md` reflection rows and the open questions.

## 3. Performance and memory design (Oven)

1. **Storage of `info`.**
   - Keep the 8-byte scalar and the u32 table index. Register moves, compares and memo keys are then integer ops, and there is no per-value heap.
   - Add a dense `DeclId → ReflectionId` side index, allocated only under `-freflection` and 4 B per declaration. `members_of` then costs one array load per member instead of an FxHashMap probe.
   - The overflow `expect` at `kiln_ast/src/reflect.rs:61` becomes the diagnostic "too many reflections".
2. **`members_of` and its siblings: cache once, slice per call.**
   - Today each `MemberAt` calls `reflect_members`, which rebuilds a `Vec<DeclId>` and then a `Vec<ReflectionId>` (`vm/meta.rs:57`, `reflect.rs:296`). A loop over n members costs O(n²) work and allocates per call.
   - Fix: sema keeps a `MemberCache: FxHashMap<DeclId, (u32 start, u32 len)>` into one flat `Vec<ReflectionId>` pool. It is filled on the first query, after implicit special members have been declared.
   - Each element carries a 1-byte kind mask and a 1-byte access tag. `members_of`, `nonstatic_data_members_of`, `static_data_members_of` and the access filtering then become one filtered copy out of the pool.
   - **Invalidation:**
     - A complete class never changes. P2996 adds members only by *completing* a class, and `__kiln_define_member` works only before `}`. Class entries are therefore immutable and are never invalidated.
     - Namespaces grow append-only. The cache stores the prefix it has seen and extends it. The evaluation point picks the visible prefix (point-dependent).
     - `bases_of`, `enumerators_of` and `parameters_of` get their own lists in the same pool.
   - The bridge result is borrowed (`&[ReflectionId]` re-borrowed through `bridge.ast()`), not a fresh `Vec`. The steady-state Rust allocation per call is 0.
3. **Purity: re-enable memoization.**
   - Replace the blanket `run.impure += 1` (`vm/meta.rs:38`) with a per-op class.
   - `Stable` ops leave `impure` unchanged, so their callers memoize. They are a pure function of their arguments: type traits, `type_of`, names, layout, predicates, and member queries on a complete class.
   - `PointDependent` ops bump `impure`: `is_complete_type`, namespace `members_of`, `access_context::current`, anything on an incomplete class.
   - `Mutating` ops bump `impure` and are forbidden in trials: `define_aggregate`, `__kiln_define_member`, and on-demand instantiation.
   - The memo already refuses any call that did heap operations (`vm.rs:1756`). Functions that return `vector<info>` therefore stay unmemoized. That is correct, and it is why §3.2 makes the vector build itself O(n) with no hidden O(n) bridge work.
4. **Memory reclamation.**
   - `vector<info>` and strings that the Oven materializes are ordinary `std::allocator` allocations. They are reclaimed on `delete` by the size-class allocator, the same as every other transient, so the "reclaim mid-evaluation" rule holds unchanged.
   - Name strings (`identifier_of`) are frozen string objects cached per Symbol. The thaw cache maps each to one static-storage allocation per TU, so they cost no transient allocation per call.
   - **The one non-reclaimable growth is reflection-table entries** created by `reflect_constant`/`reflect_object` and by fresh `data_member_spec`s. An `info` can sit anywhere in Oven memory, so freeing entries mid-evaluation would need a GC.
   - Proposal: (a) intern by content, so loops over repeated values stay bounded; (b) charge table growth to the evaluation as a new counter `refl_bytes`, included in `peak_bytes` and in the `-fconstexpr-memory` limit; (c) M7 option: a per-evaluation overlay table whose entries are promoted when they escape into a frozen result, an NTTP or a memo entry, and dropped at the end of the evaluation. Document this as the only exception, with its bound.
5. **Transactional mode (WP-139).**
   - `can_substitute` and the SFINAE-style probing inside `substitute` run their instantiation-time constant evaluations under `checkpoint(TrialMode::Full)` and roll back on failure. This is the second user WP-139 asks for.
   - Interned reflections are kept on rollback. They are append-only and harmless, and dropping them would only save memory if the §3.4 overlay exists.
   - Mutating ops inside any checkpoint fail silently, as in §2.4.
6. **`define_aggregate` cost.**
   - O(n) in the number of specs. Specs are read from Oven memory into a reused scratch buffer, and names are interned as Symbols straight from the bytes. The spike builds a `String` per name (`vm/meta.rs:76 read_name`).
   - Uniqueness is checked with one FxHashSet over the spec names, not a lookup per member.
   - One `act_on_finish_record` and one layout per class.
   - The Oven refreshes only the target class's provisional slot. More generally, replace `Types::refresh` (`ty.rs:646`), which scans every provisional slot at the start of every evaluation, and `provisional.contains` (`ty.rs:375`), a linear scan per intern hit. The replacement is a sema completion epoch (`ast.completion_epoch`, bumped when a class completes) plus a `FxHashSet<OTy>`. Refresh is skipped when the epoch has not changed. Without this, a TU with thousands of forward declarations pays O(#incomplete) per evaluation.
7. **Splice fast path.** See §2.5: there is no nested Oven run for NTTP or frozen operands. Instantiating `[: R :]` per distinct `R` is the expected cost. WP-467 tracks the per-element cost of `template for`, a shared hotspot.
8. **Budgets** (initial; M6-14 records the GCC 16.1 numbers and the supervisor recalibrates them):

   | Synthetic shape (WP-030 port, `bench/reflect/`) | kiln wall | kiln peak RSS | Oven counters |
   |---|---|---|---|
   | S1: 2,000 aggregates × 16 members, serializer via `template for` over `nonstatic_data_members_of` + `identifier_of` + member splice | ≤ 0.5× GCC 16.1 | ≤ 0.5× GCC, ≤ 400 MiB absolute | `peak_arena` independent of N; 0 live allocs after each evaluation |
   | S2: 500 enums × 64 enumerators, enum↔string tables via `enumerators_of` | ≤ 0.5× GCC | ≤ 0.5× GCC | ≤ 1 µs and 0 Rust allocs per cached query (host-normalized) |
   | S3: `define_aggregate` generating 1,000 structs (struct-of-arrays) plus one with 10,000 members | ≤ 20 ms for the 10k struct | ≤ 4 MiB growth for the 10k struct | `refl_bytes` ≤ 32 B per spec |
   | S4: annotation-filtered field selection (`annotations_of_with_type`) on S1's types | ≤ 1.2× S1 | ≤ 1.1× S1 | — |
   | S5: type-list metaprogramming via `substitute` / `template_arguments_of` (10k substitutions) | ≤ 0.5× GCC | ≤ 0.5× GCC | instantiation count reported |
   | S6: namespace `members_of` scan over 5,000 declarations, repeated 100× | linear | flat | cached prefix hit rate ≥ 99% |

   **Scaling gate:** each shape runs at N and 4N. A time or RSS ratio above 4.6 fails, which catches quadratic behaviour independent of host speed.
9. **Per-PR tracking.**
   - The merge queue runs the S-tier `bench/reflect` (N small, ≤ 30 s total) and gates on the deterministic counters: steps 0%, `peak_bytes`/`peak_arena`/`refl_bytes` 2%, and the `oven:` budgets in tests.
   - Wall time and RSS are recorded but advisory there, because shared runners are noisy.
   - New `-fconstexpr-report` fields: `meta_calls`, `meta_cache_hits`, `refl_bytes`, `injected_decls`, `instantiations_from_meta`.
   - The scaling gate and the vs-GCC table (`docs/status/perf.md`, a reflection section) run on the dev box by `cargo xtask bench --suite reflect --scaling`, manually or nightly.

## 4. Work packages

Size: S ≈ 1–2 days, M ≈ 3–5 days, L ≈ one week-long slot (< ~2k lines each).

Every WP follows the same rules:
- tests are `oracle: none` goldens written from the IS text (`skip(clang)` implies none)
- every test names its standard section
- the brief header carries the reference-workload naming rule
- new diagnostic ids go in kiln_diag area files

**M6-01 — spec/reflection.md + dispatch skeleton (M).**
- *Scope:* write spec/reflection.md §1–§9 and replace the superseded sections with pointers.
  - Refactor the bridge to `SemaBridge::meta(...)` with family sub-enums, each family in its own file: `kiln_oven/src/meta/{mod,query,traits,member,value,define,annot,access,templ}.rs`, plus a sema mirror `kiln_sema/src/meta/*.rs`. Move the intrinsic halves out of `kiln_sema/src/reflect.rs:149-430`; reflect.rs keeps `act_on_*`.
  - Variable operand list (`compile.rs:5227`).
  - Purity classes replace `vm/meta.rs:38`.
  - `MetaCtx` with the evaluation point.
  - `meta_intrinsic_of` skips inline namespaces (`std::__1::meta`) and matches by signature.
  - Port the 4 spike ops onto it with no behaviour change.
- *Deps:* none. *Parallel with:* 02, 03, 04, 12, 14.
- *Tests:* the spike's `reflection/*.cpp` stay green. Add `reflection/dispatch-inline-ns.cpp` and a memo test in which a `Stable` query is memoized and a `PointDependent` one is not (`-fconstexpr-report`).

**M6-02 — spike hardening R1/R3/R5 + table and type-table scale (S).**
- *Scope:*
  - `kiln_abi::ensure_layout` (`layout/mod.rs:516`) refuses an incomplete class (R1).
  - `Sema::same_entity`, plus a test-side lint that no `meta/` code compares `DeclId` raw (R3).
  - Delete the shadow-kind promise from oven.md; the typed-copy rule is decided (R5).
  - The overflow diagnostic (`kiln_ast/src/reflect.rs:61`).
  - The `Types::refresh` completion epoch and the `provisional` set (`ty.rs:375,646`).
- *Deps:* none. *Parallel:* all of wave 1.
- *Tests:* unit tests for `ensure_layout` → None. The existing `class.mem/incomplete-pointer-in-constant-evaluation.cpp`. A stress test with 5,000 forward-declared classes and 5,000 evaluations whose steps and wall stay flat.

**M6-03 — representation and the `^^` operator (M).**
- *Scope:* the full `Reflectee` set (§2.2) and the dense `DeclId` index.
  - `^^::`, namespace aliases, alias sugar kept (fixes `reflect.rs:39`), templates and concepts, enumerators, structured bindings, function template specializations; overload sets rejected.
  - Freezing and NTTP for every kind (`templ/nttp.rs`).
  - Printing (`notes.rs:753`, ast dump `CXXReflectExpr`).
  - consteval-only type checks for `info` variables.
- *Files:* kiln_ast reflect.rs, kiln_parse reflect.rs (operand side), kiln_sema reflect.rs `act_on_reflect_*`, templ/nttp.rs.
- *Deps:* none (land after 01 if both edit `kiln_sema/src/reflect.rs`; the cut lines differ). *Parallel:* 01, 02, 04, 12, 14.
- *Tests:* `expr.reflect/*.cpp` (each operand kind, equality matrix, alias vs dealias, rejected operands), `basic.types.general/consteval-only.cpp`.

**M6-04 — consteval blocks (P3289) + plainly-constant-evaluated bit (R2) (M).**
- *Scope:*
  - parse `consteval { }` as a block-declaration and member-declaration
  - sema evaluation as an immediate context
  - `run.plain` in the Oven, set from `EvalContext`; it replaces the `inst.eval_loc` gate (`reflect.rs:280,339`)
  - immediate escalation of `^^` and splices
  - feature macro
  - STANDARD_SUPPORT.md row
- *Deps:* none. *Parallel:* wave 1.
- *Tests:* `dcl.pre/consteval-block*.cpp` (namespace, class and template scope; side effects via the extension), a trial-never-mutates test.

**M6-05 — splices in every position, including dependent ones (L).**
- *Scope:*
  - expression splice (`kiln_parse/src/expr.rs:1329`)
  - scope splice in a nested-name-specifier
  - member access `e.[: m :]` and base subobjects (P3293)
  - `using T = [: r :];` without `typename`
  - dependent `Splice` type node, `CXXSpliceExpr` and splice NNS, with substitution in `templ/subst/*` (replaces `reflect.rs:116` "unsupported: dependent splice")
  - the §2.5 fast path
  - `template [: r :]` is M6-13.
- *Files:* kiln_parse reflect.rs/expr.rs, new `kiln_sema/src/splice.rs`, kiln_ast types/expr, templ/subst.
- *Deps:* 03 (soft: designation by kind). *Parallel:* 06, 07, 08, 16.
- *Tests:* `basic.splice/`, `expr.prim.splice/`, `dcl.type.splice/`, `expr.ref/splice-*`; run tests for member splices through codegen.

**M6-06 — result materialization, names family, P3560 exceptions (M).**
- *Scope:*
  - sema-resolved construction recipes for class-typed results (`vector<info>`, `string_view`, `u8string_view`, `source_location`, `member_offset`, `access_context`) and `reflection_range` arguments read as (data, size) spans
  - `MetaError` → `std::meta::exception` thrown via P3068
  - the names family: `has_identifier`, `identifier_of`, `u8identifier_of`, `display_string_of`, `u8display_string_of`, `source_location_of`, as per-Symbol cached frozen strings
  - retire `name_of` outside the test header
  - grow `tests/support/kiln_meta.h` into a `<meta>`-shaped test header, still the kiln vehicle until M6-15
- *Deps:* 01. *Parallel:* 05, 07, 08, 16.
- *Tests:* `meta.reflection.names/*.cpp`, `meta.reflection.exception/*.cpp` (catch in constexpr, uncaught gives a note), alloc and reclaim budget tests (`oven:` live allocs 0).

**M6-07 — queries and predicates (M/L).**
- *Scope:*
  - `type_of`, `object_of`, `constant_of`, `parent_of`, `dealias`, `has_template_arguments`
  - `operator_of`, `symbol_of`
  - all `is_*`/`has_*` predicates, including P3096 parameter predicates and `return_type_of`/`variable_of`
- *Files:* `meta/query.rs` (both crates).
- *Deps:* 01, 03. *Parallel:* 05, 06, 08, 16.
- *Tests:* `meta.reflection.queries/*.cpp`, one file per predicate group, each with the IS example verbatim where one exists.

**M6-08 — type traits family (M).**
- *Scope:* every [meta.reflection.traits] function, delegating to the existing builtin trait engine (`kiln_sema/src/templ/builtins/traits.rs`) so that `__is_*` and `is_*_type` cannot diverge. Transformations return the canonical type except where the IS says otherwise **(verify alias retention)**.
- *Deps:* 01. *Parallel:* 05, 06, 07, 16.
- *Tests:* `meta.reflection.traits/*.cpp`, cross-checked by a generated test asserting `is_X_type(^^T) == std::is_X_v<T>` over a type zoo; that part has a clang 22 oracle for the RHS.

**M6-09 — members, access, layout (L).**
- *Scope:*
  - `members_of`, `bases_of`, `static_data_members_of`, `nonstatic_data_members_of`, `enumerators_of`, `parameters_of`, `subobjects_of` **(verify)**
  - the §3.2 `MemberCache`
  - forced declaration of implicit special members
  - `access_context` (`current`/`unprivileged`/`unchecked`/`via`), `is_accessible`, `has_inaccessible_*`
  - `offset_of`, `size_of`, `alignment_of`, `bit_size_of`
  - `annotations_of`, `annotations_of_with_type`, `is_annotation`
  - retire `__kiln_members_count`/`__kiln_member_at`
- *Deps:* 01, 03, 06 (12 for the annotation queries). *Parallel:* 10, 11, 13.
- *Tests:* `meta.reflection.member.queries/`, `meta.reflection.access.*/`, `meta.reflection.layout/` (offsets cross-checked against `layout` tests, which do have a clang oracle), `bench/reflect` S1/S2/S6 scaling.

**M6-10 — values and static storage (P3491) (M).**
- *Scope:*
  - `extract<T>`, `reflect_constant`, `reflect_object`, `reflect_function`
  - `reflect_constant_string`/`_array`, `define_static_string`/`_array`/`_object`, `is_string_literal`
  - the `refl_bytes` counter (§3.4)
  - the `template for` iteration over `define_static_array` that WP-454 lists (moved here)
- *Deps:* 01, 03, 06. *Parallel:* 09, 11, 13.
- *Tests:* `meta.reflection.extract/`, `meta.define.static/`, `stmt.expand/reflection-*.cpp`, a stress test of 1e5 distinct `reflect_constant`s under `-fconstexpr-memory` (bounded and diagnosed).

**M6-11 — `define_aggregate`, `data_member_spec` (M).**
- *Scope:*
  - `data_member_options` with `name_type` (both string kinds), `data_member_spec`, `is_data_member_spec`
  - `define_aggregate`: completion from the evaluation, single-slot Oven upgrade, all §2.6 rules, the §3.6 cost design
  - the injection point recorded for diagnostics
  - rebase `__kiln_define_member` onto `Define(ExtOp)` with `-Wkiln-extension`
- *Deps:* 01, 02, 04, 06. *Parallel:* 09, 10, 13.
- *Tests:* `meta.reflection.define.aggregate/*.cpp` including run tests (layout and codegen), every negative rule, `bench/reflect` S3, the existing `define-member*.cpp` kept green.

**M6-12 — annotations, syntax and semantics (P3394) (M).**
- *Scope:*
  - `[[=e]]` in attribute-specifier-seqs (parse, including the rules for mixing with attributes **(verify)**)
  - constant evaluation and freezing; structural-type check
  - the cold per-declaration storage, merged across redeclarations
  - template instantiation of annotations
  - AST dump
  - The query functions are in M6-09.
- *Deps:* none (`AnnId` is in M6-03's `Reflectee`; land 03 first or stub it). *Parallel:* wave 1.
- *Tests:* `dcl.attr.annotation/*.cpp` (positions, invalid operands, templates, redeclarations).

**M6-13 — templates: `substitute`, `can_substitute`, template splices (L).**
- *Scope:*
  - `template_of`, `template_arguments_of`, `substitute`, `can_substitute` (instantiation from the bridge through the lent Oven)
  - `can_substitute` under `TrialMode::Full` (`mem.rs:1562`; closes WP-139's second-user item and its conformance test)
  - `template [: r :] <args>` in type, expression and NNS positions
  - concepts via `substitute`
- *Deps:* 01, 03, 05, 06. *Parallel:* 09, 10, 11.
- *Tests:* `meta.reflection.substitute/`, `temp.names/splice-*.cpp`, `bench/reflect` S5.

**M6-14 — validation and perf infrastructure (M).**
- *Scope:*
  - `cargo xtask gen-reflect`: a Rust generator for the §3.8 S1–S6 shapes in `bench/reflect/`, the clean-room WP-030 port; nothing copied from the reference workload
  - `--scaling` mode and the new `-fconstexpr-report` counters wired into `xtask bench`
  - harness:
    - an `oracle-advisory` outcome (crash or unsupported never fails kiln; WP-102 policy)
    - a `gcc` compiler kind plus `skip(gcc)` and `oracle: gcc` once a GCC 16.1 path is configured (`KILN_GCC=`)
    - `cargo xtask test --external <dir>` for a private corpus that lives outside the public repo (the WP-103 ICE minimizations, kept under kiln-helper), with `never-crash` semantics
- *Deps:* none. *Parallel:* wave 1.
- *Tests:* xtask fixtures for each new outcome; the generated S-tier entries recorded as baselines, xfail(kiln) until their families land.

**M6-15 — standard `<meta>` headers and feature macros (L).**
- *Scope:*
  - Consume GCC 16.1's libstdc++ `<meta>`: a name-plus-signature table, its builtin calling convention, and range arguments. That convention is **(verify)** by reading the header.
  - The p2996-fork `<experimental/meta>` `__metafunction` shim only if the operator wants the fork's tests (advisory).
  - `__cpp_impl_reflection` and `__cpp_lib_reflection` at the IS values.
  - STANDARD_SUPPORT.md reflection rows.
  - `xtask libstdcxx` with GCC 16 headers.
  - Kiln-owned `<meta>` only if both are blocked.
- *Deps:* 06–11, 13; GCC 16.1 headers provisioned. *Parallel:* 16 plus the cluster WPs.
- *Tests:* the same reflection programs through `kiln_meta.h` and libstdc++ `<meta>` (header differential); the GCC 16.1 differential (`oracle: gcc`) on the whole reflection corpus; libstdc++ testsuite `reflection/` files via the WP-188 runner **(verify which exist)**.

**M6-16 — reflection mangling and codegen of info-templated entities (M).**
- *Scope:*
  - Ship the §2.8 placeholder now: fast, cached per `ReflectionId`, never a structural walk per use.
  - Codegen of runtime functions and variables specialized on `info` NTTPs (serializer shapes).
  - Switch to GCC 16.1's Itanium encoding once WP-102 decides; link-compat test vs GCC.
- *Deps:* 03 (placeholder); WP-102 (final). *Parallel:* 05–08.
- *Tests:* `codegen/reflection-mangle*.cpp` (determinism, uniqueness), run tests; `bench/reflect` S1 emit time.

```
wave 1 (now):  01   02   03   04   12   14          (05 once 03 merges)
                |         |    |    |
wave 2:        06   07(01,03)  08(01)  05(03)  16(03)
                |
wave 3:        09(01,03,06,12)  10(01,03,06)  11(01,02,04,06)  13(01,03,05,06)
                \_______________ all ______________/
wave 4:        15 (needs GCC 16.1 headers)  + cluster WPs from GCC differential / reference workload
```

| Stage | Runnable at once |
|---|---|
| now | 01, 02, 03, 04, 12, 14 |
| 01 merged | 05, 06, 07, 08, 16 (+ wave 1 tails) |
| 06 merged | 09, 10, 11, 13 (+ 07/08 tails) |
| families merged | 15 + clusters |

Total is about 11–13 week-long slots, against the spike's estimate of 20–30. The estimate drops because P3068 is done, P1306 is done, and the dispatch skeleton makes the families mechanical. The reference-workload bring-up adds cluster WPs at wave 4 that cannot be sized yet.

## 5. Validation strategy

**Oracles, in order of authority:**
1. IS text goldens: `skip(clang)` (which implies `oracle: none`), with `stdout:` for run tests and `diag:` for negatives. Each test quotes its section and, where one exists, the IS example.
2. **GCC 16.1 differential** once provisioned: `oracle: gcc`, run results, plus accept/reject. Wording is not compared (operator 11-03).
3. The p2996 fork is advisory only: spot checks of AST shape and diagnostics; its crashes count as "advisory: oracle crashed" (M6-14).
4. clang 22 still checks everything that is not reflection (traits RHS, layouts, IR of the generated code where a splice-free twin exists).

**Private corpora:** the WP-103 ICE minimizations (≈20 GCC 16.1 cases and any clang/p2996 cases) and the reference workload itself live under kiln-helper (never in kiln/). The supervisor runs them with `cargo xtask test --external` and `xtask bench --external`; only counts and synthetic reductions reach the public repo. The rule is never crash, never overflow, never exceed the budget. A reflection case that is not yet implemented is `xfail(kiln)` with the owning WP.

**Header differential:** every reflection test that uses only IS names also runs through libstdc++ `<meta>` (M6-15). Different outcomes are a bug in one of the two shims.

**Merge-queue CI** (net per-job CI time must not grow; D1):
- the reflection conformance dirs in kiln mode, with their `oracle: gcc` comparisons: a bounded set, each test compiled once by GCC 16.1 (no run-twice shapes), when the CI image carries GCC (D1)
- the deterministic reflection counter gates: the `oven:` budgets of the `bench/reflect` shapes (§3.8) once their `xfail(kiln)` markers go, and the stress cases with `oven:` budgets

The vs-GCC perf table, the `--scaling` sweeps and the private corpora stay off the merge queue: manual or nightly on the dev host. GCC is never built from source in CI.

**M6 exit measures:**
- (a) the reference workload builds and its tests pass under kiln with libstdc++ 16.1 `<meta>`
- (b) compile wall time and peak RSS recorded against GCC 16.1 on it and on S1–S6, with kiln ≤ GCC on both and the §3.8 targets met or a written reason given
- (c) the private ICE corpus at 0 crashes
- (d) every reflection row in STANDARD_SUPPORT.md `done`
- (e) the GCC differential green on the public reflection corpus, or each divergence triaged with an IS citation
- (f) scaling gate passes for S1–S6. **Met (WP-638, 2026-11-05):** S1–S6 compile on libstdc++ 16.1 and all six sweeps pass.
  4N/N growth for kiln (time, RSS) is S1 1.88x, 1.95x; S2 2.42x, 2.38x; S3 1.91x, 1.09x; S4 1.82x, 1.91x; S5 1.18x, 1.27x; S6 1.19x, 1.01x.
  `peak_arena` is flat on S1, S2, S4 and S5. S7/S8 were already green.
  At the bench N, kiln/GCC is 0.20x–0.94x in time and 0.08x–0.34x in memory.
  The tables are in docs/status/perf-reference-workload.md "kiln vs GCC 16.1".

**M6 exit record (2026-11-05):**
- (a) Met. The reference workload: all 49 slugs of its harness generate, compile and run their tests under kiln with libstdc++ 16.1 `<meta>` (private harness, supervisor-measured 2026-11-05).
- (b) Met with one written reason. 48 slugs are measurable on both compilers: generator stage wall 619 s (kiln) vs 1277 s (g++ 16.1), 0.48x; peak RSS sum 29.6 GiB vs 88.7 GiB, 0.33x, kiln lower on every slug; whole-harness wall 1831 s vs 2493 s, 0.73x; production-compile stage 1.04x overall, with one outlier (one wide binding TU: 264 s vs 76 s) (private harness, supervisor-measured 2026-11-05). The written reason is docs/status/perf-reference-workload.md "`synth_accessor_god` production TU (WP-633)": kiln's frontend takes 14 s of it, and the rest is LLVM's JumpThreading and MemCpyOpt on one 92k-instruction registration function; clang 19 on the same IR shape takes 432 s. The §3.8 target of 115 s for that TU is carried into M7 (WP-641, WP-642). The S1–S6 ratios are in (f).
- (c) Met. The private ICE corpus (36 reproducer sources): kiln has 0 crashes and 0 timeouts, and every diagnosis matches the finding's expectation; g++ 16.1 ICEs on 6 of them (supervisor-measured 2026-11-05).
- (d) Met (WP-639): every row of docs/STANDARD_SUPPORT.md "C++26 reflection" is `done`. The paper row P3394R4 stays `partial` and lists what is missing (base-specifier annotations, a parameter pack's annotations, string-literal pointers; spec/annotations.md §9).
- (e) Met. 98 conformance files run `oracle: gcc` (88 of them reflection tests) and are green in merge-queue CI; one triaged divergence, recorded with its IS citation in docs/status/deviations.md "GCC 16.1 reflection divergences".
- (f) Recorded above.

## 6. Risks and open questions for the operator

1. **GCC 16.1 is not in the container** (only g++-14). Options:
   - the operator adds it to the sandbox image (preferred)
   - a headers-only libstdc++ 16.1 tree under `.toolchain/`: enough for M6-15 but not for the differential
   - building from source in the workspace: hours of CPU on a shared host

   M6-15 and the exit criterion depend on this choice.
2. **No libc++ 22 `<meta>` exists**, which contradicts "libc++ 22's experimental header first". Choose between:
   - (a) fetch the p2996 fork's libc++ `<experimental/meta>` (headers only) as the early vehicle
   - (b) skip it: kiln_meta.h is the test vehicle until libstdc++ 16.1 `<meta>` arrives
   - (c) promote a kiln-owned conforming `<meta>`

   Recommend (b), and (a) only if the fork's tests are wanted.
3. **IS vs P2996R13 / fork divergences:** names and signatures changed through NB resolution (e.g. `members_of` taking `access_context`, `subobjects_of`, `type_order`, the feature macro values). M6-01 must pin spec/reflection.md to the IS draft text. Which draft number is authoritative?
4. **Implicit special members in `members_of`**: the spike omits them and the IS (I believe) includes them, which forces their declaration. That changes lazy-declaration order and therefore some diagnostics and layouts-in-progress. Confirm we follow the IS.
5. **Reflection-table growth** (§3.4) is the one memory that is not reclaimed during an evaluation. Is charging it to `-fconstexpr-memory` and deferring the overlay to M7 acceptable?
6. **`-freflection` default:** keep it opt-in through M6, or turn it on at `-std=c++26` (GCC 16 requires `-freflection`)?
7. **`__kiln_define_member`** stays as an extension. Should it be usable outside `-freflection`, and should it warn under `-pedantic` only?
