# spec: kiln_ast — the typed AST, templates, constant values, dump format

Status: **design** (WP-040). Normative for M2 where marked; sections marked
*(M3+)* fix representation decisions now and are implemented later.
Companion: `spec/types.md` (types, `TypeContext`, type printing).
Consumers: kiln_parse/kiln_sema (build), kiln_oven (reads, via
`SemaBridge`), kiln_codegen (reads), the harness (dump goldens).

## 1. Principles
1. **Index arenas, no pointer graph.** Every node lives in a `Vec` and is
   named by a 4-byte `NonZeroU32` id. No `Rc`, no lifetimes in node types,
   `#![forbid(unsafe_code)]`. Ids are TU-local.
2. **Append-only, write-once.** Nodes are never removed and, apart from the
   short list of late-bound slots in §2.4, never modified after creation.
   Ids handed out stay valid and mean the same thing for the whole TU, so
   the Oven and codegen may cache by id across evaluations.
3. **One semantic tree.** The AST is the sema output: every expression
   carries its type and value category and every conversion is explicit.
   There is no separate syntax tree (the parser drives sema directly, as
   clang). Syntactic forms that sema rewrites keep only what diagnostics,
   reflection and the dump need (§5.6).
4. **Compact fixed-size headers + side arenas.** Hot fields (kind, type,
   value category, two operands) are inline; anything larger or rarer is
   in a per-kind side record or a sparse "cold" map keyed by id.
5. **clang's model, kiln's encoding.** Node classes, children order and
   conversion kinds follow clang 19 so its AST is a usable oracle
  (ast-dump.md §6);
   the storage is ours.

## 2. Ownership and ids
### 2.1 Ids
```rust
pub struct DeclId(NonZeroU32);   pub struct ExprId(NonZeroU32);
pub struct StmtId(NonZeroU32);   pub struct AttrId(NonZeroU32);
pub struct ListId(NonZeroU32);   // immutable list of u32 ids in `lists`
pub struct ConstValueId(NonZeroU32); pub struct ReflectionId(NonZeroU32);
pub struct StringLitId(NonZeroU32);  // bytes of a string literal (`strings` pool)
// from types.md: TypeId, QualType, TemplateNameId, NnsId, ArgListId, ProfileId
```
`Option<XId>` is 4 bytes. Index 0 of each arena is a dummy. An id
type per arena makes mixing arenas a type error. Capacity is 2^32 − 1 per
arena; exhaustion is a fatal diagnostic, not a panic.

### 2.2 The `Ast`
```rust
pub struct Ast {
    pub types: TypeContext,        // spec/types.md
    pub decls: DeclArena,          // §4: headers + per-kind payloads + cold maps
    pub exprs: ExprArena,          // §5
    pub stmts: StmtArena,          // §6
    pub attrs: AttrArena,          // §7
    pub lists: IdLists,            // ListId -> &[u32]; (start: u32, len: u32) spans into one Vec<u32>
    pub strings: StringPool,       // literal bytes (code units, encoding-tagged)
    pub lookup: LookupTables,      // §4.3
    pub redecls: RedeclTable,      // §4.4
    pub templates: TemplateTables, // §8
    pub consts: ConstStore,        // §9
    pub reflections: ReflectionTable, // §10 (M6)
    pub layouts: LayoutCache,      // spec/types.md §11
    pub written_refs: WrittenRefs, // §18, opt-in (off by default)
    pub tu: DeclId,                // the TranslationUnitDecl
    symbols: Arc<kiln_lex::Interner>,
}
```
`Ast` is created by sema at TU start, owned by `kiln_sema::Sema` while
parsing, then moved to codegen. Sema is single-threaded per TU (§13 Q8):
one thread builds the tree. `Ast: Send + Sync` (a compile-time assertion
beside the struct, as `SourceManager`'s, WP-2321): a finished tree can be
moved, and read through `&Ast` from several threads at once (a tooling
client's host-call handler that reads another unit's tree, spec/host-calls.md
§3): every field is `Sync` (a cache is behind a lock or an atomic).

### 2.3 Memory accounting
Every arena implements `HeapSize { fn heap_bytes(&self) -> MemReport }`
(rows: name, count, live bytes, reserved bytes). `Ast::memory()`
aggregates; `-fconstexpr-report` and the bench record it per TU next to
the Oven counters (spec/bench.md). Size assertions (`size_of`) for every
header type are unit tests (§14), so a field addition that grows a header
fails CI rather than silently costing memory.

The per-instantiation node meter (`Ast::node_live_bytes`, read at every
instantiation frame push and pop) must not allocate. The decl, expr and
stmt arenas therefore list their vectors as `kiln_ast::VecDims`, which
holds a static name, length, capacity and element size and has
`live_bytes()` and `row() -> MemRow`. A `MemRow` and its `String` name
are built only for the report (WP-116: the meter was allocating about 1M
`String`s on `templates_wide`, roughly 9% of its instruction count, once
LLVM stopped eliding them). The annotation, attribute, redeclaration and
lookup arenas sum the same `VecDims` and, for their maps,
`mem::map_live_bytes` / `ColdMap::live_bytes`, so the meter builds no
`MemRow` there either (WP-645: about eight allocations per push or pop
before); their `heap_bytes` rows report the same numbers.

### 2.4 Mutation discipline
After a node is created its header and side record are immutable, except:

| slot | written | by |
|---|---|---|
| `FunctionData.body` | once (definition parsed or instantiated) | sema |
| `VarData.init`, `VarData.eval` | init once; eval state machine §9.2 | sema / Oven via bridge |
| `RecordData` definition data, `members` | while the class is being defined; frozen at `}` (except `define_aggregate`, §10, and the row below) | sema |
| `DeclArena::completed_records` | append-only: each class definition at its `}` (`complete_definition`; instantiations included), read as the layout dump's order (spec/abi.md §8.4; WP-324) | sema via builder |
| `DeclArena::top_level_ends` | append-only: (last member of the TU's member list, class-completion epoch) at the end of each top-level declaration of the TU (`end_top_level_decl`, called by the parser); a TU member belongs to the first end at or after it in the list, the classes complete to codegen there (spec/codegen.md §7z; WP-492) | parser |
| `RecordData.members.last` of a complete class | append-only: implicit special members and inheriting constructors, declared lazily (`append_implicit_member`, spec/sema-class.md §4.1) | sema |
| `VarData` `nrvo` bit | once, at the variable's scope exit (`set_nrvo`) | sema |
| `VarSpec` `init_style` `Call` | once, with the `CXXConstructExpr` of a default-initialized class variable (WP-067b; part of `VarSpec::LATE`, ORed in from `C` only) | sema |
| `VarSpec` `init_style` `ParenList` | once, over `Call`, when a parenthesized initializer turns out to initialize an aggregate (P0960; WP-067b) | sema |
| `ReturnStmt` NRVO candidate (`b`) | cleared once at the end of the function body when its variable is not `nrvo` (`clear_return_nrvo`, clang `computeNRVO`; WP-067b) | sema |
| cold `ctor_inits` of a constructor | once, with the body (`set_ctor_inits`; implicit initializers included) | sema |
| cold `init_fp` of a constructor, field or parameter | once, when its mem-initializers, default member initializer or default argument are parsed (the FP pragma state there, when not the command line's; read by `Ast::init_fp`, WP-1904, and for a parameter by `Ast::default_arg_fp`, an instantiated parameter getting its pattern's at substitution, WP-1919) | sema |
| cold `overrides` of a method, `friends` of a class | once (`set_overrides` at the declaration, `set_friends` at `}`) | sema |
| `LetData.value` (`LifetimeExtendedTemporary`) | once | Oven via bridge |
| `FnSpec::deleted` | once, at `}` (`= default` members found deleted; part of `FnSpec::LATE`) | sema |
| `FnSpec` `defaulted` (`Explicit`), `constexpr` (`Constexpr`) | once, at `= default` after the declarator (WP-067a; part of `FnSpec::LATE`, ORed in from `No`/`None` only) | sema |
| `ParamData::DEFAULT_ARG_PENDING`, `ParamData.default_arg` | the flag set at the declaration; the argument once, when the late-parsed tokens are parsed ([class.mem]/7) | parser / sema |
| `DeclNode.ty` | re-pointed for deduced return types and resolved exception specs | sema |
| `RedeclGroup.latest/definition` | on each redeclaration / definition | sema |
| `DeclFlags::{USED, REFERENCED, INVALID}` | monotone set | sema |
| lookup tables, specialization maps, `layouts`, `virtuals`, `vtables`, `consts`, `types` | insert-only | sema / Oven / `kiln_abi` |
| cold `member_overload` `template_args` (§5.10) | once, when the parser reaches the template-id after the overloaded access | sema |
| `templates.oper_lookups` (§8.5) | insert-only; one set per dependent operator expression, at its creation | sema / module-image importer |
| `templates.pattern_mem_inits` (§8.6) | once per constructor definition in a template, at the end of its ctor-initializer (before the body when it has none) | sema / module-image importer |
| `WrittenRefs` entries (§18) | append-only; `entity`/`found` of an overloaded name patched once by overload resolution; `enclosing` narrowed by declaration claims | parser / sema |

**Instantiation adds nodes, never rewrites patterns**: substituting a
pattern creates fresh decls/exprs/stmts (with `instantiated_from` links,
§8.3) appended to the arenas. Consequently instantiated bodies are retained
for the TU's life (§13 Q6).

**Every type an AST holds has a path** (WP-2120). The tables behind the
public fields are `#[doc(hidden)] pub`, their fields private, so that a
tooling client can name each type (a snapshot or a size account over an
`Ast`): `ast::InstantiationMeter`, `consts::{Node, AggRec, Pools}`,
`strings::Entry`, `types::args::{ArgTable, ListEntry}`,
`types::intern::{KeyedEntry, KeyedTable}`, `types::node::Store`,
`types::profile::{ProfileEntry, ProfileTable}`. They are not a stable
interface: they change with the representation.

## 3. Names
`DeclName(u32)`: a 3-bit tag in bits 29..=31 and a 29-bit payload.

| tag | name | payload |
|---|---|---|
| 0 | identifier | `Symbol` index (payload 0 + tag 0 = the empty name) |
| 1 | `operator@` | `OverloadedOperator` (`+`, `new[]`, `()`, `co_await`, `<=>`, ...) |
| 2 | constructor | index into `names.special` → canonical class `TypeId` |
| 3 | destructor | same |
| 4 | conversion function | index → canonical target `QualType` |
| 5 | literal operator | `Symbol` of the suffix |
| 6 | deduction guide | index → template `DeclId` |
| 7 | reserved | |

`names.special` is interned, so `DeclName` equality is `u32` equality and
`(context, DeclName)` is a hashable 8-byte lookup key.

## 4. Declarations
### 4.1 Header
```rust
struct DeclNode {        // 28 bytes
    kind: DeclKind,      // u8 (§4.2)
    flags: DeclFlags,    // u8: INVALID IMPLICIT USED REFERENCED HAS_ATTRS
                         //     HAS_LEXICAL_CTX DEPENDENT_CTX INJECTED
    misc: DeclMisc,      // u16: access(2) friend(2) local_extern(1) reserved(3, zero)
                         //      name_independent(1, P2169 `_`) linkage cache(3) visibility(2)
                         //      first(1) definition(1)
    name: DeclName,      // u32
    ctx: Option<DeclId>, // semantic DeclContext; None only for the TU
    next: Option<DeclId>,// next sibling in the *lexical* context's member list
    loc: Loc,            // the name's location ([dcl.decl]: the declarator-id)
    ty: Option<QualType>,// value decls: declared type; type decls: the declared type; else None
    data: u32,           // index into the kind's payload arena (0 = none)
}
```
- `misc.first`/`misc.definition` are query caches the arena owns
  (WP-655): set, `Ast::first(d) == d` / `Ast::definition(d) == Some(d)`;
  clear means "ask the redeclaration group" (§4.4). `DeclArena::push`
  computes them (a head's bits are ignored): a fresh group's first
  declaration, or a non-redeclarable non-template declaration, is first;
  a non-redeclarable declaration is its own definition. A template decl
  is first when its pattern is first and describes it; a concept always
  is; `Ast::set_definition(d)` sets `d`'s definition bit.
- The three reserved bits are zero: no declaration header records module
  ownership; the owner is `Ast::modules.owner(d)` (spec/modules.md), and
  the never-called `DeclMisc::with_module_ownership` and its enum were
  removed (WP-2477).
- The lexical context is stored in a sparse map `lexical_ctx: FxHashMap<
  DeclId, DeclId>` only when it differs from `ctx` (out-of-line member
  definitions, friends, local externs): flag `HAS_LEXICAL_CTX`.
- The namespace definition a namespace-scope declaration belongs to
  (clang's `getDeclContext()`, each `namespace N { }` a context of its
  own) is `ctx` unless the sparse map `written_ns` (memory row
  `written_ns`; snapshots carry it) says otherwise: for a qualified
  declarator naming a namespace (`int n::f() {}`), whose `ctx` is the
  first definition of `n`, the latest one when it is declared (what the
  qualifier's lookup found); for an explicit specialization of a
  function template, whose `ctx` is its template's, the definition it is
  written in or its qualifier names (WP-2725, WP-2626).
  `DeclArena::written_ns(d)` reads it, `set_written_ns` records it.
  Codegen's visibility walk starts there (spec/codegen.md §8).
- The declared type as written (sugar and placeholder included) of a
  deduced-placeholder variable or deduced-return function is stored in
  `ColdDeclData::written_decl_type: ColdMap<DeclId, QualType>` (memory
  row `written_decl_type`; snapshots and module images carry it;
  `DeclArena::written_decl_type(d)`, `AstBuilder::set_written_decl_type`;
  spec/types.md §8.4, WP-3640) only when the deduced part contains at
  least one written expression.
- Source range: the begin location (decl-specifiers start) is in the
  payload of kinds that need it for diagnostics; the end is computed.
- Identifier namespace (IDNS: ordinary / tag / member / namespace / label
  / template-param) is a function of `kind` and `misc.friend`/
  `local_extern`, not stored.

#### Silent failures (WP-3281)

`DeclArena::subst_failed(d) -> Option<SubstFailedKind>` (cold map
`subst_failed`, memory row `subst_failed`; the header has no spare flag
bit) marks a declaration kiln made `INVALID` while it was inside a
*trap* (a SFINAE context, or a diagnostic capture whose stash was then
discarded) that printed no error; the kind says what failed to form.
Sema writes it through its trap ledger only (spec/sema-templates.md
§5.7.3), at the end of the translation unit and before a snapshot;
`set_subst_failed` requires `INVALID` and keeps the first kind;
`subst_failed_decls()` lists the marks by id.

| value | `SubstFailedKind` | set on | its state |
|---|---|---|---|
| 1 | `FunctionSignature` | a function template specialization, or a deduction guide's, whose declaration failed to substitute ([temp.deduct.general]/8), or substituted while the frame trapped an error | substitution failed: type `ERROR`, no parameters; trapped error: full type and parameters. Neither is a usable specialization |
| 2 | `ExplicitSpecifier` | the same, when only its `explicit(expr)` failed ([temp.deduct.general]/7) | full type and parameters |
| 3 | `VariablePartialAmbiguity` | a variable template specialization whose partial-specialization choice was ambiguous ([temp.spec.partial.match]/1) | type `ERROR`, not listed as instantiated |
| 4 | `ClassPartialAmbiguity` | a class template specialization whose choice was ambiguous | a never-instantiated specialization (kind `Undeclared`, no definition, members or bases), still a nameable type (`X<int>*`) |
| 5 | `LocalOfFailedSignature` | an invalid declaration created while a kind 1 or 2 specialization failed to form (a parameter, a statement-expression's local), or whose owner chain reaches one | whatever its own failure left |
| 6 | `OtherTrapped` | any other invalidation under a trap or a discarded pack-expansion probe, and the duplicate of a definition merged from another unit's global module fragment (spec/modules.md §6) | as the site left it |

Value 0 is never written; `SubstFailedKind::from_value` gives `None`
for 0 and for any value above 6. A tooling client treats a kind it does
not know as indeterminate. The values are fixed: a new kind takes 7.

Not marked: a variable template specialization whose template or
matching partial specialization is only declared (a valid entity,
[temp.inst]/7); a class template specialization used inside its own
definition under a trap ([temp.inst]/2: the use's signature fails, the
class instantiates on its next use, valid); a candidate dropped for
unsatisfied constraints ([temp.deduct.general]/5: at clang compat 22 no
specialization is formed, at 19 it is formed and valid).

**The invariant.** In a translation unit without diagnosed errors,
every `INVALID` declaration reachable from the lists carries
`SUBST_FAILED` (`DeclArena::subst_failed`). An `INVALID` declaration
without it means kiln printed an error.

"Without diagnosed errors" means no diagnostic of error severity was
emitted. "Reachable from the lists" is the closure, from the translation
unit, over these lists and links:
- the member list of a record, an enumeration, a namespace (the
  translation unit is one) or a scope payload (`DeclArena::members`);
- a function's parameter list (`FunctionData::params`);
- the template parameter list of a template (`TemplateInfo::params`), a
  partial specialization (`PartialSpecInfo::params`) or a template
  template parameter (`TemplateParmInfo::params`);
- a template's pattern (`TemplateInfo::pattern`) and its specializations
  (`TemplateTables::specs_of`);
- the closure types whose `closure_owner` is a reached declaration.

The checker tests the stronger form: every `INVALID` declaration in the
arena, reachable or not.

**Rule A.** A declaration created while a marked kind 1 or 2
specialization S was formed belongs to something; none sits unmarked and
ownerless. From the declaration, ownership is followed until one of
these holds:
- it is kind 5 itself;
- it reaches S, or an independent entity formed as a side effect: a
  specialization (whose members belong to it), or an implicit deduction
  guide (memoised with its class template);
- a template is followed to its pattern;
- a template parameter is followed to the declaration whose parameter
  list holds it (a template, a partial specialization, a template
  template parameter); one older than the formation lists it;
- anything else is followed through `closure_owner`, else its lexical
  context; once that owner is older than the formation, the owner lists
  the declaration reached: as a member (a pattern through its template:
  `Al<int>` completed inside the failed signature lists its member
  template, whose pattern constructor and parameter are reached through
  it), or as a function's parameter.

Two exceptions are a known gap: valid scratch declarations held by no
list, built only to check or compare in every instantiation (not because
of the failure). Neither is `INVALID`, so the invariant is not affected.
- Constraint-satisfaction `ParmVar` copies. Satisfying a function
  template's trailing requires-clause substitutes its parameters into
  new `ParmVar`s whose context is the pattern; clang maps the
  specialization's own parameters instead
  (`addInstantiatedParametersToScope`). The checker accepts a `ParmVar`
  named like a parameter of its older function. One path from the lists
  reaches a copy: a variable template specialization first formed while
  the clause is satisfied stores its converted arguments as written
  (crates/kiln_sema/src/templ/var.rs:93 into `SpecInfo::args`,
  crates/kiln_ast/src/templates.rs:165), and a `decltype` argument keeps
  its operand (crates/kiln_ast/src/types/context.rs:185), a
  `DeclRefExpr` that names the copy: `v<decltype(x), int>` in
  `requires v<decltype(x), int>`. No other path exists. The substituted
  constraint expression is evaluated and dropped
  (crates/kiln_sema/src/templ/concepts/sat.rs:648); the satisfaction
  records and the atom cache keep a result, locations and text, no
  declaration (crates/kiln_sema/src/templ/concepts/mod.rs:38); a written
  reference or found set records only dependent lookups, and a
  parameter's name is resolved at the definition
  (crates/kiln_sema/src/written_specs.rs:93).
- Friend class template head comparison. Instantiating
  `template <class U> friend struct F;` substitutes the friend's head
  only to compare it with `F`'s (`check_friend_class_head`); Sema
  records that head, and the checker accepts its parameters. No path
  from the lists reaches one. The compared head is passed only to
  `check_template_heads_match`
  (crates/kiln_sema/src/templ/equiv.rs:159), which compares, diagnoses
  and keeps nothing. Every type or expression naming one of its
  parameters depends on it, so it forms no specialization, satisfaction
  record or atom-cache entry; a written reference or found set records
  no template parameter, which is always found at the definition.

No written reference (`Ast::written_refs`) is keyed by a kind 1 or 2
specialization.

The checker `-fkiln-verify-subst-failed` (or `KILN_VERIFY_SUBST_FAILED=1`
in the environment; spec/harness.md) reports each violation of the
invariant and of rule A as an error. Snapshots carry the map (an unknown
kind is a `BadTag` decode error, spec/pch.md); module images do not: an
importer re-forms the specializations, and the marks with them
(spec/modules.md). The dump prints ` subst-failed=<kind>` after
` invalid` (spec/ast-dump.md).

### 4.2 Kinds and payloads
Context kinds (★) own a member list `members: Members { first, last }` (append O(1)
through `next`) and a lookup table (§4.3). Functions are the semantic
context of their parameters and local declarations but own neither (§4.3). `R` = redeclarable (payload starts with `Redecl { prev:
Option<DeclId>, group: RedeclGroupId }`). `T` = may be templated (§8.2).

| group | kinds |
|---|---|
| scopes | `TranslationUnit`★, `Namespace`★R (inline bit; inline children in an append-only side map), `NamespaceAlias`R, `LinkageSpec`★ (C / C++), `Export`★ (M4), `Empty`, `FileScopeAsm` |
| using | `UsingDirective`, `Using`, `UsingEnum`, `UsingShadow`, `ConstructorUsingShadow`, `UsingPack`, `UnresolvedUsingValue`, `UnresolvedUsingTypename`, `UnresolvedUsingIfExists` (M3) |
| types | `Typedef`R, `TypeAlias`R T, `Enum`★R T (scoped, fixed underlying type, `int_type`, promotion type), `EnumConstant` (value: `ConstValueId`), `CXXRecord`★R T (struct/class/union; lambda closure bit), `ClassTemplateSpecialization`★R, `ClassTemplatePartialSpecialization`★R |
| members | `Field` (bit-width `ExprId`, default member init, mutable, `[[no_unique_address]]`), `IndirectField` (chain into anonymous unions), `AccessSpec` (no payload: the access is `misc.access`), `Friend` (no payload: a friend type is `DeclNode.ty` and `data` the raw `DeclId` of the class its elaborated-type-specifier first declared, or 0; a befriended function or class has `ty` None and `data` its `DeclId` — `Decls::friend_target` → `FriendTarget::{Decl, Type { ty, owned_tag }}`; P2893 pack), `FriendTemplate` |
| functions | `Function`R T, `CXXMethod`R T, `CXXConstructor`R T, `CXXDestructor`R T, `CXXConversion`R T, `CXXDeductionGuide` T |
| variables | `Var`R T, `ParmVar`, `ImplicitParam` (`this`, coroutine frame), `Binding` (one structured-binding name; no payload: its expression and holding variable are cold, see below; P1061 pack M4+). The decomposed object of a structured binding declaration (clang's `DecompositionDecl`) is a `Var` with an empty name and the cold `decomp_bindings` list, not a kind of its own (WP-122), `VarTemplateSpecialization`R, `VarTemplatePartialSpecialization`R |
| templates | `ClassTemplate`R, `FunctionTemplate`R, `VarTemplate`R, `TypeAliasTemplate`R, `Concept`, `BuiltinTemplate` (`__make_integer_seq`, `__type_pack_element`), `TemplateTypeParm`, `NonTypeTemplateParm`, `TemplateTemplateParm`, `ImplicitConceptSpecialization`, `RequiresExprBody`★ |
| synthesized | `LifetimeExtendedTemporary` (payload `LetData { extending: DeclId, mangling_index: u32, value: Option<ConstValueId> }`; context the extending declaration's, in no member list or lookup table), `TemplateParamObject` (class-type NTTP object: type + `ConstValueId`), `StaticAssert` (cond `ExprId`, message `ExprId`, P2741), `ConstevalBlock` (the StaticAssert payload: cond = the call of the block's synthesized consteval lambda, no message; P3289R1, spec/consteval-blocks.md), `Label`, `ContractResultName` (P2900 `post(r: ...)`) |

**Reserved declaration kinds** (WP-2026). `UsingPack`, `FriendTemplate`,
`ImplicitParam` and `ImplicitConceptSpecialization` are in `DeclKind` but
nothing constructs them: a using-declarator pack (`using Ts::f...;`)
expands at instantiation into one `Using` per element (clang groups
them in a `UsingPackDecl`); a friend template is a `Friend` naming the
template; `this` is
`CXXThisExpr` and the coroutine frame has no declaration; a concept-id is
checked in place (spec/sema-templates.md). They are kept, not removed,
because the numbering is stable: `DeclKind as u8` is part of the ODR
definition hash (spec/abi.md, `ODR_DEF_SCHEME`), of the PCH snapshot and
of the module image, and removing one renumbers every later kind. A
consumer may treat them as unreachable; the match arms that name them
stay so a later kiln can start building one without a format change.


Using-family payloads (WP-062; one inline `Payload::None(u32)` word,
read with `Ast::using_target`/`Ast::using_qualifier`):

| kind | payload word | in lookup? | in member list? | builder |
|---|---|---|---|---|
| `UsingDirective` | nominated namespace (first declaration) | no | yes | pushed by Sema (`act_on_using_directive`) |
| `Using` | `NnsId` of the qualifier as written (0: none) | no | yes | `using_decl` |
| `UsingShadow` (implicit) | target declaration (or an `UnresolvedUsingIfExists`) | yes | yes | `using_shadow(h, target, prev)` |
| `UsingEnum` | the enumeration's definition | no | yes | `using_enum` |
| `NamespaceAlias` | aliased namespace or alias, as written | yes | yes | `namespace_alias(h, target, prev)` |
| `UnresolvedUsingIfExists` | 0 | no | no | `unresolved_using_if_exists` |

- `Ast::strip_shadow(d)` looks through a `UsingShadow`;
  `Ast::resolve_alias(d)` follows alias chains to the namespace's first
  declaration.
- `prev` replaces an earlier shadow of the same target (or an earlier
  alias to the same namespace) in the lookup table; it is not yet a
  redeclaration-chain link (no `Redecl` payload for these kinds), so
  the dump prints no `prev`.
- `DeclRefExtra::found_decl` records the `UsingShadow` a `DeclRefExpr`'s
  name was found through (clang's `FoundDecl`).
- **Shadows of a using-declaration** (WP-2025, for tooling clients that
  read the `Ast` after the translation unit). `Ast::using_shadows(u)`
  gives the shadows using-declaration `u` introduced, in introduction
  order; `Ast::shadow_using(s)` the using-declaration of shadow `s`.
  Two write-once cold maps hold them: `using_shadows: ColdMap<DeclId,
  ListId>` (a `Using` or `UsingEnum` → a `ListId` of its shadows, no
  entry when it introduced none) and `shadow_using: ColdMap<DeclId,
  DeclId>` (the reverse). Sema writes both with
  `AstBuilder::set_using_shadows` once the declaration's last shadow is
  made: a namespace-, block- or class-scope `using N::f;` (one
  `UsingShadow` per declaration the lookup found, a repeated one too,
  WP-3403; none for an enumerator already a member of the class, an `UnresolvedUsingIfExists`'s for `using_if_exists`), an
  inheriting-constructor `using B::B;` (one `ConstructorUsingShadow` per
  constructor of the base, its implicit copy and move constructors
  included, one a constructor of the class hides marked hidden), and `using
  enum E;` (one per enumerator). A dependent using-declarator
  (`UnresolvedUsingValue`, `UnresolvedUsingTypename`) introduces none;
  the `Using` its instantiation makes in the class specialization (or in
  the instantiated function body) has the instantiation's shadows. A
  shadow a member of the class hides ([namespace.udecl]/4, /14) stays in
  the list, marked: `DeclArena::hidden_shadow(s)` names the member that
  hides it. The mark is the same whatever the order of the member and
  the using-declarator (WP-3145: `void f(); using B::f;` makes the shadow
  and marks it, as `using B::f; void f();` does), and it is decided by
  the class's `}` at the latest: an implicit member that hides a shadow
  (the copy or move assignment operator for `using B::operator=;`, the
  constructors for `using B::B;`) is declared there, so the record does
  not depend on what the rest of the unit uses (WP-3146). The dump
  leaves hidden shadows out, as clang's (which has none). The module image carries the link (each shadow record names
  its using-declaration's record, spec/modules.md §11.11 kind 21), and a
  snapshot carries the maps. Cost: a forward-map entry and an 8 B list
  span per using-declaration, 4 B per shadow in the list arena and a
  reverse-map entry per shadow (9 B live each); about 24 KB on a TU of
  fifteen standard headers, spec/landed/ast/WP-2025.md.
- **What a using-declaration names, for tooling clients** (WP-3403,
  [namespace.udecl]/1, /4, /14). The record alone answers which members
  a class-scope `using B::f;` names and which of them are hidden; a
  client needs no lookup of its own. The accessors are total:
  - `Ast::using_named(u)`: per shadow of `u`, in `using_shadows` order,
    a `UsingNamed { shadow, target, hider }`. The *named set* of `u` is
    the targets; the members `u` brings into its class are those whose
    `hider` is `None`. Empty for any other declaration, for a dependent
    using-declarator (read the `Using` of each specialization, whose
    marks are its own), and for a class-scope `using E::A;` naming an
    enumerator of a member enumeration: the enumerator is already a
    member of the class, so no shadow is made.
  - `Ast::shadow_hider(s)` (= `DeclArena::hidden_shadow`): the member
    of `s`'s class that hides it, `None` for a visible shadow or any
    other declaration.
  - `Ast::shadows_hidden_by(m)`: the shadows whose hider is `m`'s first
    declaration, in member order (a scan of the class's members, no
    storage): what renaming or removing `m` would make visible. Empty
    for anything but a member function or member function template.
  - `Ast::overridden_methods(m)`: the virtual functions `m` overrides,
    read at its first declaration, so any redeclaration answers.
  - `Ast::shadow_named(s)` (WP-3401, [namespace.udecl]/1): the shadow
    that `s`'s using-declarator named, when what its lookup found for
    `s`'s target was itself a using-shadow. In `namespace acme { using
    std::string; } using acme::string;` the second shadow's target is
    `std::string` (kiln, like clang, makes a shadow's target the entity,
    never a shadow), and `shadow_named` gives `acme`'s shadow. `None`
    for a shadow whose lookup found its target directly (declared in the
    scope searched, or made visible there by a using-directive: the
    directive route is not recorded), for the shadows of `using enum`
    (its enumerators are not looked up) and of an
    `UnresolvedUsingIfExists`, and for any other declaration.
    *Walking the chain:* the record is the first link only. A client
    reads `shadow_named(s)`, then `shadow_named` of that shadow, and so
    on; the chain ends at a shadow whose lookup found its target
    directly. Every link is a shadow of the same entity (its
    `strip_shadow` is `s`'s target), and a link is never `s` itself.
    The record is what the lookup found, not a redeclaration link: a
    using-declarator repeated through another base (*A repeated
    using-declarator* below) has its own record beside its `prev`, and
    `using B1::f;` names `B1`'s shadow only when `B1` has one. Per form:
    - namespace, block and class scope: one record per shadow of an
      overload set, each naming the shadow its own element was found
      through (`using Base::f;` where `Base` has `using Mid::f;` and a
      `void f(char);` of its own: `Base`'s shadows for `Mid::f`'s
      overloads, none for `Base::f(char)`); a hidden shadow keeps its
      record ([namespace.udecl]/14, either order);
    - an inheriting constructor (`using B::B;` where `B` has `using
      A::A;`): a `ConstructorUsingShadow` of an `A` constructor names
      `B`'s `ConstructorUsingShadow` of it, the nominated base's shadow
      it was inherited through (the same entry as
      `DeclArena::ctor_shadow_nominated`, clang's
      `getNominatedBaseClassShadowDecl`; no second row); one of a
      constructor `B` declares names none;
    - templates: a dependent using-declarator makes no shadows; each
      specialization's using-declaration (class or block scope) records
      what its own lookup found (`G<Base>`'s `using T::f;` names
      `Base`'s shadows, `G<Mid>`'s none); a non-dependent one is redone
      in each specialization with the pattern's records.

    Storage: `ColdDeclData::shadow_named: ColdMap<DeclId, DeclId>`
    (`UsingShadow` → the shadow found), written once by sema as the
    shadow is made (`introduce_shadow`, from `lookup::Found::shadow`); a
    shadow that found its target directly has no row, so the cost is a
    map entry per chained shadow only. A PCH snapshot carries the map; a
    module image carries it per shadow record (the `named` word,
    spec/modules.md §11.11 kind 21). The dump is unchanged, and no list
    changes order or targets.

  The rules the record keeps:
  - *One mark in either order.* A member declared before or after the
    using-declarator hides the shadow when the two correspond
    (`member_hides_using_target`, spec/sema.md "Member hiding"); the
    shadow is made either way. In a class template specialization the
    members are redone in declaration order, each through the parser's
    declarator actions (`act_on_declarator`), so a member function
    (template) redone after the using-declaration marks the shadows it
    hides as in a non-template class (`check_using_shadow_conflict`): a
    specialization's record has the same shape.
  - *Dependent correspondence.* In a pattern, a member whose
    parameter-type-list depends on a template parameter corresponds to
    no base member (`template <class T> struct D : B { using B::f; void
    f(T); };`: the pattern's shadow of `B::f(int)` is unmarked); each
    specialization marks per its arguments (`D<int>`'s is hidden by
    `D<int>::f(int)`, `D<char>`'s is not), in either order.
  - *The hider.* The member's declaration in the member-specification,
    which is its first declaration (`first(h) == h`), never an
    out-of-line definition. For a member template it is the
    `FunctionTemplate`; in a specialization, the specialization's
    member; for an implicitly-declared member, its implicit declaration
    (`DeclFlags::IMPLICIT`). It is exactly the member that corresponds
    to the shadow's target: the other overloads of that name hide
    nothing.
  - *Implicit hiders, by the class's `}`.* A mark never depends on what
    the unit uses. Every implicit member that can hide a shadow is
    declared by the class's `}` at the latest, and the mark is set when
    it is declared: the copy and move assignment operators for `using
    B::operator=;` (WP-3146), the default, copy and move constructors for
    `using B::B;` (declared eagerly, `HAS_INHERITED_CTOR`), in a class
    template specialization too. The `operator==` a defaulted
    `operator<=>` declares ([class.compare.default]/5) hides nothing:
    it is declared only when the class declares no `operator==`, and a
    `using B::operator==;` counts (clang 19, clang 22 and GCC 14 agree).
  - *A virtual override* hides the shadow of the function it overrides
    ([namespace.udecl]/14: members "override and/or hide"): one mark,
    whose hider is the overrider, in either order;
    `overridden_methods(hider)` tells an override from a plain hide.
  - *A conversion function* hides only the base's conversion function
    to the same type (the type is part of its name); a conversion
    function template cannot be named by a using-declarator.
  - *A repeated using-declarator.* A second class-scope
    using-declarator naming an entity another using-declarator of the
    class already introduced (through another class: `using B1::f;
    using B2::f;` with a virtual base `A` declaring `f`) makes its own
    shadow, which replaces the earlier one in lookup (as at namespace
    scope, and as clang chains `PrevShadow`). Each using-declaration
    keeps its shadow in its list, so both name the entity. There is no
    redeclaration group of shadows: two shadows of one class name one
    entity when their targets are the same declaration. A member that
    hides one of them marks every shadow of that entity in the class
    (`mark_hidden_shadow`).
  - *Lookup never finds a hidden shadow; the table holds it.*
    `Ast::lookup_local` is the raw per-context table ("no hiding") and
    returns a hidden shadow in either order, as both orders enter the
    shadow. Sema's lookup results never contain one (`lookup::Found`;
    the found declaration of a reference; the written-reference table's
    found sets, §18; an `UnresolvedLookupExpr`'s found list, §8.8,
    whether built from a lookup or substituted from its pattern: a
    substituted overload set leaves out an element whose shadow the
    specialization hides).

  Tests: `crates/kiln_parse/tests/using_hidden_shadow.rs` (each rule
  above, the snapshot round trip and totality),
  `crates/kiln_parse/tests/using_shadow_named.rs` (`shadow_named`: every
  form above, both orders, the snapshot and module image round trips),
  `crates/kiln_driver/tests/modules_image_using.rs` (the repeated
  using-declarators' marks in the module image; a class template's
  using-declarations are not in the image, spec/modules.md §11.11),
  `tests/conformance/namespace.udecl/p14-hidden-after-using-specialization-run.cpp`,
  `p14-repeated-using-through-bases-run.cpp`.
- **Repeated using-declarations and namespace aliases** ([namespace.udecl],
  [namespace.alias]; WP-2025 Q14). Two using-declarations of the same
  name in one scope are two `Using` declarations, never chained: each
  has its own shadows, and a shadow of a target an earlier shadow of the
  same scope already introduced (`using N::f; using N::f;` at namespace
  or block scope) is a new `UsingShadow` that replaces the earlier one in
  the lookup table (`prev` above), not a redeclaration of it
  (`Ast::first(s2) == s2`). In a class the second introduces nothing
  new through another class ([namespace.udecl]/10 diagnoses the same
  class twice). Two namespace aliases of one name (`namespace A = N;
  namespace A = N;`) are likewise two `NamespaceAlias` declarations, the
  later one in the lookup table, each its own first declaration; one
  naming another namespace is an error
  (`sema_redefinition_different_namespace_alias`). This matches the
  standard: a using-declarator and a namespace alias declare names, not
  entities ([basic.pre]/3 lists neither); both denote the same entity
  (`Ast::strip_shadow`, `Ast::resolve_alias`), which is what
  [namespace.udecl]/1 and [namespace.alias]/2 require. Pinned by
  `crates/kiln_parse/tests/using_linkage.rs` `repeated_using_and_alias`.

Hot payloads (dense arenas, one record per decl of the kind):
```rust
struct FunctionData {               // 24 bytes
    redecl: Redecl,                 // 8
    params: ListId,                 // ParmVar DeclIds
    body: Option<StmtId>,           // late-bound (§2.4)
    spec: FnSpec,                   // u32: storage(2) inline(1) implicit_inline(1) constexpr_kind(2: none/constexpr/consteval)
                                    //   virtual(1) pure(1) defaulted(2: no/=default/implicit) deleted(1) explicit(1)
                                    //   trivial(1) immediate_escalating(1) has_contracts(1) is_coroutine(1)
                                    //   escalated(1) variadic(1) override(1, bit 18) final(1, bit 19)
                                    //   explicit_object(1, bit 20) virtual_written(1, bit 21)
                                    //   storage_inherited(1, bit 22: storage taken from a previous
                                    //   declaration, not written; the dump omits it)
                                    //   not_selected(1, bit 23: a prospective destructor overload
                                    //   resolution did not select, [class.dtor]/4; LATE, set at `}`;
                                    //   `Ast::destructor` skips it; the dump omits it)
                                    //   return_deduced(1, bit 24: the return type was deduced from
                                    //   a placeholder, so the declared type is not the type as
                                    //   written; LATE; read by the linkage computation, WP-389)
                                    //   exc_spec_computed(1, bit 25: the exception specification was
                                    //   computed, not written; LATE, set on every redeclaration when
                                    //   sema resolves `ExcSpec::Deferred` (implicit or defaulted
                                    //   special member, defaulted comparison); WP-2505, below)
                                    //   (`virtual` is semantic: written or overriding; `virtual_written`
                                    //   is the specifier, which the dump prints. LATE bits, set after
                                    //   the body or at `}`: trivial, has_contracts, is_coroutine,
                                    //   escalated, deleted)
    templ: TemplLink,               // §8.2
}
```

A function's exception specification lives on its type
(`FunctionProto`'s `ExcSpec`). Once a deferred one is resolved
([except.spec]/7-8: an implicitly declared or explicitly defaulted
special member without a written specification, a defaulted comparison)
the type holds the result, the same type a written specification gives;
so `S() noexcept(false) = default;` and `S() = default;` computed to be
potentially-throwing have one type. `Ast::exception_spec_computed(f)`
tells them apart for tooling clients (WP-2505): true while the type
still defers to `f`'s entity (`ExcSpec::Deferred(first(f))`) or after
sema resolved it (`FnSpec::exc_spec_computed`), false for a written
specification on a defaulted function, and false for a specialization
whose written specification waits for instantiation (deferred to its
pattern). A member's written *noexcept-specifier* parsed late in the
complete-class context also defers to the member until it is parsed.
PCH snapshots carry it with `FnSpec` (no schema change, so no
`FORMAT_VERSION` bump: a snapshot of another kiln build is replayed
anyway, spec/pch.md "Format versioning"); module images carry it as
bit 25 of the function specifier bits (`IMAGE_VERSION` 83). Pinned by `crates/kiln_driver/tests/front_end.rs`
`exception_spec_computed_vs_written`.

```rust
struct VarData {                    // 24 bytes
    redecl: Redecl,
    init: Option<ExprId>,
    spec: VarSpec,                  // u32: storage(3) tls(2) init_style(2: c/call/list/paren-list) constexpr(1) constinit(1)
                                    //   inline(1) nrvo(1) exception_var(1) cxx_for_range(1) inline_implied(1)
                                    //   placeholder(2: none/auto/decltype(auto)/__auto_type, bits 14-15)
                                    //   static_init_fact(8, bits 16-23, §25.3; not in module images)
    eval: VarEval,                  // u32, §9.2: 0 NotEvaluated | MAX Evaluating | MAX-1 NotConstant | else ConstValueId
    templ: TemplLink,
}
struct RecordData { redecl: Redecl, members: Members /*first, last*/,
                    bases: ListId /*1-based indices into DeclArena::base_specs*/,
                    defn: DefinitionBits /*u64, clang's DefinitionData flags; bits 24–49
                                            spec/sema-class.md §2.9, bit 50 FINAL, 51 LAMBDA_CAPTURES,
                                            52 HAS_FLEXIBLE_ARRAY_MEMBER, 53 TRIVIAL_ABI, 54–56
                                            {COPY_CTOR,MOVE_CTOR,DTOR}_FOR_CALL (spec/abi.md §8.25)*/,
                    templ: TemplLink, lambda: Option<LambdaId>, tag: TagKind /*struct/class/union*/ }   // 40 bytes
struct BaseSpec { ty: QualType /*as written*/, loc: Loc, is_virtual: bool,
                  access: Access /*as written; None if defaulted*/, pack_expansion: bool, ... }
```
Cold data (only some decls have it) lives in `FxHashMap<DeclId, _>`
maps: class features (spec/sema-class.md §8.1) — `ctor_inits` (a `ListId`
of `CtorInit { target: u32, kind: CtorInitKind /*Base, VirtualBase, Field,
IndirectField, Delegating*/, written_index: u8 /*IMPLICIT = 255*/, init:
ExprId, loc }` in initialization order, read by `Ast::ctor_inits`),
`overrides` (`ListId` of overridden methods), `friends` (`ListId` of a
class's befriended first declarations), `key_function`, `dtor_delete` (the
operator delete a virtual destructor uses), `record_pack` —
`= delete("reason")` messages (P2573), `explicit(expr)`, trailing
requires-clauses, contract lists (P2900: `contracts`, a `ListId` of the
`ContractAssertStmt`s of a function's `pre`/`post` specifiers in written
order, keyed by the declaration that wrote them; `Ast::function_contracts`;
spec/contracts.md §4), `asm` labels,
default arguments of templates, alignment overrides, deprecation messages.
A cold map costs ~16 B per *entry* and nothing for decls without one;
fields used by fewer than ~5% of decls of a kind go there.

- `asm_label: ColdMap<DeclId, StringLitId>` (WP-063): the GNU asm label
  of a function or variable, keyed by the entity's *first* declaration
  (a label on any redeclaration names the entity); each declaration
  also carries an `AsmLabel` attribute (§7).
- `declarator_end: ColdMap<DeclId, Loc>` (WP-370, WP-381, WP-428): for a
  function (member, or at namespace or block scope) declared without a
  body, or defined by `= default`/`= delete`, the end of its declaration
  (clang `getEndLoc` before a body is attached): the start of the
  declarator's last token (`Declarator::end`), or for an `= default`/`=
  delete` written in a class the last character of the keyword (clang
  `ParseCXXInlineMethodDef` `setRangeEnd(KWEndLoc)`). An instantiation
  reads its pattern's, except one whose definition was instantiated from
  an out-of-line pattern definition, which copies that definition's
  (`AstBuilder::relocate_to_pattern_definition`). sema.md §7.13 points a
  note there; a defaulted assignment operator is defined there
  (sema-class.md §4.4).
- `builtin_id: ColdMap<DeclId, u32>` (WP-063): the builtin id of an
  implicit builtin `Function` decl, read through
  `DeclArena::builtin_id(d)`. Ids and spellings are in
  `kiln_ast::decl::builtin` (`IS_CONSTANT_EVALUATED = 722`, clang's
  numbering; `ALL`; `by_name`). Landed (WP-140): the C string builtins
  `STRCMP` (1140), `STRNCMP`, `STRCHR`, `STRRCHR`, `STRPBRK`, `STRSTR`,
  `WCSCMP`, `WCSNCMP`, `WCSCHR` (1386).
- Structured bindings (WP-122, sema.md §4.9). Write-once cold maps:
  `decomp_bindings: ColdMap<DeclId, ListId>` (decomposed `Var` → its
  `Binding`s, in order), `binding_decomp` (`Binding` → that `Var`),
  `binding_expr: ColdMap<DeclId, ExprId>` (the binding expression:
  `e[i]`, `e.m`, or a `DeclRefExpr` to the holding variable) and
  `binding_var` (`Binding` → holding variable, tuple-like only).
  `DeclArena::set_decomp_bindings(v, list, bindings)` and
  `set_binding(b, expr, holding)` write them; `decomp_bindings`,
  `is_decomposition`, `binding_decomp`, `binding_expr` and
  `binding_var` read them; `is_holding_var(h)` scans `binding_var` (the
  Oven's test of a temporary it extends, after cheaper ones). `AstBuilder::binding(head)` creates a
  `Binding` (type set later by sema), `set_decomposition(v, bindings)`
  links them, and `holding_var(head, ty, spec)` creates the implicit
  (`implicit`) `Var` a tuple-like binding refers to: a member of its
  lexical context that name lookup never finds. The dump prints the `Var` as `DecompositionDecl`
  (spec/ast-dump.md §3).
- `AstBuilder::linkage_spec_head(head, lang)` creates a
  `LinkageSpecDecl` from a full `DeclHead`, so implicit ones get
  `IMPLICIT` at creation (`linkage_spec` delegates to it).
- `AstBuilder::unlinked_alias(head, underlying)` (WP-119) creates a
  `TypeAlias` decl with its typedef type that is entered in no lookup
  table and no context's member list: Sema's dependent-scope marker for
  a dependent `decltype(e)::` qualifier (sema.md §4.3.1).

### 4.3 DeclContext lookup tables
- **One global table** `FxHashMap<(DeclId /*semantic ctx*/, DeclName),
  LookupEntry>`. `LookupEntry(u32)`: top bit clear = a single `DeclId`
  (so decl ids are capped at 2^31 − 1; exhaustion is fatal); set = index into `sets: Vec<SmallVec<[DeclId; 4]>>` (overload sets,
  tag + non-tag pairs `struct stat` / `stat()`, P2169 multiple `_`).
- The entry holds the **latest** declaration of each entity
  (redeclarations replace their predecessor in place, keeping overload-set
  order = order of first declaration).
- **Transparent contexts** (`LinkageSpec`, `Export`, unscoped `Enum`
  enumerators, anonymous unions/structs' `IndirectField`s) insert each
  name into the nearest non-transparent enclosing context as well as (for
  enums and anonymous records) their own.
- **Entry flags** (`LookupFlags`, stored with each `DeclId` of an entry):
  `VIA_INLINE` (below) and `ADL_ONLY` — a function first declared by a
  friend declaration, visible only to argument-dependent and
  redeclaration lookup until redeclared at namespace scope
  ([namespace.memdef]/3; owned by spec/sema-overload.md §2.3).
- **Inline namespaces**: names declared in inline namespace `I` are also
  inserted into each enclosing namespace up the chain of inline
  namespaces, flagged `via_inline` in the set entry ([namespace.def]/7);
  `inline_children: FxHashMap<DeclId, Vec<DeclId>>` serves ADL's
  associated namespaces (namespaces reopen, so this cannot be a `ListId`).
- Function-local scopes are **not** tables: block-scope lookup is sema's
  scope stack during parsing; afterwards function bodies are reached only
  through ids. Class scopes, namespaces and the TU are tables.
- Lookup *rules* (hiding, using-directives, ADL, two-phase) are sema's;
  the AST answers `lookup_local(ctx, name) -> &[DeclId]` and
  `members(ctx) -> impl Iterator<DeclId>` (lexical order).
- **Using-directives** (WP-339): `using_dirs: FxHashMap<DeclId,
  Vec<DeclId>>` records, per namespace or TU (primary context), the
  namespaces its namespace-scope `using namespace` directives nominate
  (first declarations, deduplicated, in source order).
  `Ast::add_using_directive(ctx, n)` records one, also on each enclosing
  namespace of an inline `ctx` up to the first non-inline one
  ([namespace.def.general]/7); `LookupTables::using_directives(ctx)` and
  `has_no_using_directives()` read them. Adding one bumps the table
  generation. Block-scope directives are sema's scope stack.

### 4.4 Redeclaration chains
`RedeclGroup { first: DeclId, latest: DeclId, definition: Option<DeclId> }`
(12 B), shared by all redeclarations; each redeclarable payload stores
`prev` and `group`. `first(d)`, `latest(d)`, `definition(d)` are O(1).
Types and template keys refer to `first` (spec/types.md: `Record(first)`),
lookup returns `latest`, codegen emits from `definition`. A redeclaration
merges into its predecessor's group (sema checks compatibility). The
`ODR`-checked "is this the same entity" question is group identity.
Walking a chain (`redecls_of`, `prev` repeatedly) is O(length), so
merging a redeclaration walks it only when the walk can change the
result; otherwise a file of N redeclarations of one entity is O(N²)
(spec/landed/abi/WP-1076.md).

### 4.5 Linkage and language linkage (WP-2025)
`kiln_ast::linkage` computes both, from the `Ast` alone, so that sema,
mangling, reflection and tooling clients (which read the `Ast` after the
translation unit, when sema is gone) ask one computation:

- `Ast::linkage(d) -> Linkage { None, Internal, Module, External }` is
  the language rule of [basic.link]/3-6. A variable, function, named
  class or enumeration (or one with a typedef name for linkage
  purposes), template or concept at namespace scope is internal in an
  unnamed namespace or when `static` (or a non-template, non-volatile
  `const` variable that is not `inline`, `extern`, attached to a named
  module or of C language linkage), module when attached to a named
  module and not exported, else external. A member takes its class's
  linkage. A local entity has none, except a block-scope function
  declaration or `extern` variable, which has its first declaration's
  ([basic.link]/6). When it is the first, it declares a member of the
  innermost enclosing namespace ([dcl.meaning.general]/3.5), so
  [basic.link]/4 makes it internal in a function of an unnamed namespace
  (`namespace { void g() { extern int v; } }`; WP-2623) and external
  otherwise, whatever its function's linkage (`static void g() { extern
  int v; }` declares an external `v`). **`extern "C"` in an unnamed
  namespace** (WP-2624): a function or variable whose target scope is a
  namespace (a block-scope `extern` one included) and whose first
  declaration is in an `extern "C"` context keeps external linkage (and
  C language linkage) in an unnamed namespace. This follows clang
  (`getLVForNamespaceScopeDecl`, `getLVForLocalDecl`), not the letter of
  [basic.link]/4 as CWG 1113 and CWG 1603 left it (every name in an
  unnamed namespace is internal, and [dcl.link]/1, /4 then give it no
  language linkage), which GCC 16.1 follows (internal; a local symbol
  that keeps the C name; distinct from a global `extern "C"` entity of
  the same name). Kiln takes clang's model because its sema already does
  (such a declaration is one entity with a global `extern "C"`
  declaration of the same name, CWG 14), and its symbol is clang's (an
  unmangled external name). `Ast::extern_c_escapes_unnamed_namespace`
  owns the exception; `Ast::linkage`, `Ast::is_extern_c` and the
  mangler's `has_internal_linkage` all read it. A namespace is
  internal when unnamed or inside an unnamed one. Anything else (an
  alias, a data member, an enumerator, a parameter, a deduction guide, a
  closure type, a using-declaration or shadow) has none. Reflection's
  `has_internal_linkage`/`has_external_linkage`/`has_module_linkage`
  read it (spec/reflection.md).
- `Ast::is_extern_c(d)` is clang's `isExternC`, the symbol view mangling
  and codegen read (`kiln_abi::mangle::is_extern_c` delegates): the
  first declaration is lexically inside an `extern "C"` linkage
  specification, the innermost one deciding (through enclosing
  functions and classes: a block-scope `extern` declaration in an
  `extern "C"` function has it), it is no class member, no member-like
  constrained friend ([dcl.link]/5) and has no internal linkage. A
  function or variable in an unnamed namespace first declared in an
  `extern "C"` context keeps it, as in clang (above), a block-scope
  `extern` one in an `extern "C"` function included. In C every function and
  variable with linkage has it.
- `Ast::language_linkage(d) -> LanguageLinkage { Cxx, C, None }` is
  [dcl.link] for tooling clients: `C` when `is_extern_c(d)` (and `d`
  has linkage), `Cxx` for any other function or variable whose name has
  external linkage, `None` for everything else (types, namespaces,
  templates, data members, enumerators, parameters, local variables,
  deduction guides, names with internal or module linkage:
  [dcl.link]/1 gives a language linkage to functions and variables with
  external linkage only). A redeclaration answers its first
  declaration's ([dcl.link]/6). A static data member or member function
  in an `extern "C"` block is `Cxx` ([dcl.link]/5).

Nothing is stored: each query walks the context chain (bounded) and
reads the payloads. Tests: `crates/kiln_parse/tests/using_linkage.rs`,
`tests/conformance/basic.link/local-extern-unnamed-namespace-ir.cpp`,
`tests/conformance/dcl.link/extern-c-unnamed-namespace-ir.cpp`,
`tests/conformance/meta.reflection.queries/linkage-block-scope-extern.cpp`,
`tests/conformance/meta.reflection.queries/linkage-extern-c-unnamed-namespace.cpp`.

## 5. Expressions
### 5.1 Header
```rust
struct ExprNode {        // 20 bytes
    kind: ExprKind,      // u8 (§5.3)
    bits: u8,            // value category (2: prvalue/lvalue/xvalue) | object kind (2: ordinary/bitfield/vector elt)
                         //   | 4 per-kind flags (e.g. part_of_explicit_cast, postfix, arrow, adl)
    deps: ExprDeps,      // u8: TYPE VALUE INSTANTIATION UNEXPANDED_PACK ERROR (+3 spare)
    aux: u8,             // operator / cast kind / literal radix / trait kind
    ty: QualType,        // never a reference type ([expr.type]/1); placeholder builtins for overload sets etc.
    loc: Loc,            // operator / start token location
    a: u32, b: u32,      // operands: ExprId/DeclId/ListId/side-record index, or inline payload
}
```
- Every expression has a type and a value category, *including* in
  templates (dependent expressions have type `Dependent` (builtin 29) or
  a dependent type, and category per [temp.dep.expr] best knowledge).
- **Dependence** (`deps`) is computed at construction from children and
  the node's own rule ([temp.dep.expr], [temp.dep.constexpr]); it is never
  recomputed. `ERROR` propagates from `RecoveryExpr` and error types.
  A variably modified type contributes the `INSTANTIATION` bit of its
  variable length array bounds (`AstBuilder::type_deps`, clang's
  `ArrayType` taking its size expression's dependence): `sizeof(int[w])`
  in a function template is rebuilt by each specialization with its own
  `w`, although `int[w]` depends on no template parameter (WP-1525).
- Inline encodings: `IntegerLiteral` ≤ 64 bits stores the value in `a|b`
  (wider: `a` = `ConstValueId`); `CharacterLiteral`/`CXXBoolLiteral`
  in `a`; `FloatingLiteral` in `a` = `ConstValueId`; unary/cast: `a` =
  operand; binary: `a`, `b`; `CallExpr`: `a` = callee, `b` = args `ListId`;
  `DeclRefExpr`: `a` = `DeclId`, `b` = side index for qualifier/template
  args/found-decl (0 if none); `MemberExpr`: `a` = base, `b` = member decl
  (rare extras in a cold map).
- WP-064 forms, each read through an `ExprArena` accessor
  (`CXXDefaultArgExpr`, `OpaqueValueExpr` and `CXXTypeidExpr` are
  class-feature payloads, below):

  | kind | `a` | `b` / `aux` | accessor |
  |---|---|---|---|
  | `BinaryConditionalOperator` | `ListId` of [common, condition, true arm, false arm] | | `binary_conditional_operands` |
  | `CXXThrowExpr` | operand or none (rethrow) | | `throw_operand` |
  | `StmtExpr` | the `CompoundStmt` | | `stmt_expr_body` |
  | `AddrLabelExpr` (GNU `&&l`, WP-424) | the `LabelDecl` | `cold.end_loc` = the label name's location; a `void *` prvalue; instantiation-dependent in a template's body so each specialization rebuilds it for its own label (WP-444) | `addr_label` |
  | `OffsetOfExpr` | `ListId` of the array index expressions | `b` = byte offset (`u32`); with cold `offsetof_layout` steps (an index that is not a constant, or an offset past `u32`) the value is their sum; cold `offsetof_designator` (non-dependent) or `offsetof_pattern` (value-dependent) keeps the operand type and the designator | `offsetof_value`, `offsetof_layout`, `offsetof_indices`, `offsetof_designator`, `offsetof_pattern` |
  | `CompoundLiteralExpr` | the `InitListExpr` | | `compound_literal_init` |
  | `CXXScalarValueInitExpr` | none | | |
- **`offsetof`'s designator** (WP-2360, for tooling clients). A
  non-dependent `OffsetOfExpr` keeps, in the cold row
  `ColdExprData::offsetof_designator` (`ExprArena::offsetof_designator`),
  the operand type as written (a typedef stays the typedef) and the
  designator as sema resolved it, as `OffsetofNode`s in clang's
  `OffsetOfNode` order: a `Base { class }` (the base class's definition)
  per base class subobject on the path to a member found in a base, then
  a `Field { field, start, end }` per member (one per member of an
  anonymous struct or union chain, outermost first, each at the written
  name's range), and an `Index { position, start, end }` per subscript,
  `position` being its index in the expression's index list
  (`offsetof_indices`, its children). So `__builtin_offsetof(A, y)` and
  `__builtin_offsetof(Bq, q)` over one layout, or a union's two members,
  are different rows with one value. An instantiation's row is its own,
  resolved for its arguments; the pattern, value-dependent, keeps the
  written form in `offsetof_pattern` instead. Nothing in the compiler
  reads the row (the value is `offsetof_value` or `offsetof_layout`'s);
  it is snapshot state (PCH `FORMAT_VERSION` 148). Module images do not
  carry an `OffsetOfExpr` (a body holding one falls back), so they need
  nothing. Test: `crates/kiln_parse/tests/offsetof_designator.rs`,
  `snap_tests::snapshot_round_trip_offsetof`.
- Source ranges are computed (`begin(e)` walks to the leftmost child;
  kinds ending in a closing token store it in their side record or cold
  map); no stored ranges.
- The begin walk costs the depth of a left-recursive chain (a chain of
  N member calls `x.f().f()...`, subscripts, `<<` operators): a client
  that asks for the begin of many nodes of one chain must not walk it
  from each (that is O(N^2) for the chain, WP-2201). Every node the walk
  passes begins where the walk ends, so `Ast::begin_loc_memo` remembers
  the answer for each node of a walk longer than `BEGIN_SHALLOW` (16)
  steps (codegen's debug and call locations, the implicit-conversion
  walk; the Oven's own begin walk does the same), and
  `Ast::begin_loc_known` stops at a node whose begin the caller already
  knows (sema keeps its last four answers, so building a chain one call
  at a time costs O(1) per call; a location needed only for a diagnostic
  or a default argument is found only then). Neither changes a location. Debug
  builds count the steps of the expression walks
  (`kiln_ast::expr::walk_steps`): the chain scaling tests
  (crates/kiln_parse/tests/chain_scaling.rs,
  crates/kiln_codegen/tests/chain_scaling.rs) compare the count at N and
  4N, which a walk repeated per node makes grow 16-fold. A second debug
  counter, `kiln_ast::expr::search_steps` (`note_search_steps`), counts
  searches sema does per element of a run (label and case-value
  searches, the scopes an unqualified lookup visits, lookup-stamp
  fingerprints, rebuilds of the scope stack's walk index; spec/sema.md
  §2.1): crates/kiln_parse/tests/parse_scaling.rs compares walk plus
  search steps at N and 4N for runs of labels and case labels, member
  call chains, `else if` chains, nested classes and closure chains
  (WP-2478).
- **No parent links.** `ParentMap::build(&Ast, root: StmtId)` builds one on
  demand (diagnostics needing context, `-Wparentheses`-style checks).

### 5.2 Value categories ([basic.lval])
`ty` is the type of the expression after [expr.type] adjustment (never a
reference); `lvalue`/`xvalue` are printed in the dump; prvalue is not.
Bit-fields set object kind `bitfield` (codegen and the Oven need the
`FieldDecl` from the `MemberExpr`).

### 5.3 Expression kinds
Names are clang's (the dump prints them). (M2) marks the M2 subset.

| group | kinds |
|---|---|
| literals | `IntegerLiteral` (M2), `FloatingLiteral` (M2), `CharacterLiteral` (M2: kind ascii/wide/utf8/16/32), `StringLiteral` (M2: `StringLitId`, kind), `CXXBoolLiteralExpr` (M2), `CXXNullPtrLiteralExpr` (M2), `UserDefinedLiteral`, `ImaginaryLiteral`, `EmbedExpr` (`#embed`), `GNUNullExpr` (`__null`, glibc `NULL`) |
| names | `DeclRefExpr` (M2), `MemberExpr` (M2), `CXXThisExpr`, `PredefinedExpr` (`__func__`, `__PRETTY_FUNCTION__`), `SourceLocExpr` (`__builtin_source_location`, `__builtin_LINE`...), `UnresolvedLookupExpr`, `UnresolvedMemberExpr`, `DependentScopeDeclRefExpr`, `CXXDependentScopeMemberExpr`, `SubstNonTypeTemplateParmExpr`, `SubstNonTypeTemplateParmPackExpr`, `FunctionParmPackExpr` |
| operators | `UnaryOperator` (M2), `BinaryOperator` (M2; incl. `,` and assignment), `CompoundAssignOperator` (M2; computation types in side record), `ConditionalOperator` (M2), `BinaryConditionalOperator` (GNU `?:`; M2), `ArraySubscriptExpr` (M2), `ParenExpr` (M2), `CallExpr` (M2), `CXXMemberCallExpr`, `CXXOperatorCallExpr`, `CXXRewrittenBinaryOperator` (`<=>`/`==` rewrites), `ParenListExpr` (dependent init) |
| casts | `ImplicitCastExpr` (M2), `CStyleCastExpr` (M2), `CXXFunctionalCastExpr` (M2), `CXXStaticCastExpr`, `CXXConstCastExpr`, `CXXReinterpretCastExpr`, `CXXDynamicCastExpr`, `BuiltinBitCastExpr` (`__builtin_bit_cast`). A named or functional cast whose type as written (clang `getTypeAsWritten`) is not its node type (plus `&`/`&&` by value category) keeps it in the cold `cast_written` map (`Exprs::cast_written_type` / `AstBuilder::set_cast_written_type`; WP-223), for the dump |
| initialization | `InitListExpr` (M2; semantic form, §5.6), `ImplicitValueInitExpr` (M2), `CXXConstructExpr`, `CXXTemporaryObjectExpr`, `CXXInheritedCtorInitExpr`, `CXXDefaultArgExpr` (M2), `CXXDefaultInitExpr`, `CXXScalarValueInitExpr` (M2), `CXXParenListInitExpr` (P0960), `ArrayInitLoopExpr`, `ArrayInitIndexExpr`, `CXXStdInitializerListExpr`, `CompoundLiteralExpr` (GNU; M2), `DesignatedInitUpdateExpr` (C99 designators; `a` = base, `b` = updater `InitListExpr`; WP-196), `NoInitExpr` (an updater slot that keeps the base's value; WP-196) |
| temporaries | `MaterializeTemporaryExpr` (extending decl / `LifetimeExtendedTemporary`), `CXXBindTemporaryExpr`, `ExprWithCleanups`, `OpaqueValueExpr` (M2), `PseudoObjectExpr` (WP-456: `a` = a list `[syntactic, semantics...]`, `b` = the index of the result among the semantics; the node's type, value category and object kind are the result's; the semantics are the `OpaqueValueExpr`s it binds, each before its uses, then the result. Built by `AstBuilder::pseudo_object(syntactic, semantics, result)`; read by `ExprArena::pseudo_syntactic`, `pseudo_semantics`, `pseudo_result`. The syntactic form of the synthesized three-way comparison is a `BinaryOperator` `<=>` of the result's type (`AstBuilder::syntactic_binary`), never evaluated. Dumped as clang: `PseudoObjectExpr` with the syntactic form, then the semantics, as children) |
| memory | `CXXNewExpr` (incl. placement, P2747 constexpr placement new), `CXXDeleteExpr`, `CXXPseudoDestructorExpr` |
| exceptions/RTTI | `CXXThrowExpr` (P3068 constexpr exceptions; M2), `CXXNoexceptExpr`, `CXXTypeidExpr` (M2) |
| traits | `UnaryExprOrTypeTraitExpr` (M2: `sizeof`, `alignof`, `__alignof`), `TypeTraitExpr` (`__is_same`, ..., `__builtin_is_within_lifetime` is a call), `ArrayTypeTraitExpr`, `ExpressionTraitExpr`, `OffsetOfExpr` (M2), `SizeOfPackExpr`, `RequiresExpr`, `ConceptSpecializationExpr` |
| lambdas, packs | `LambdaExpr`, `PackExpansionExpr`, `CXXFoldExpr`, `PackIndexingExpr` (P2662), `CXXUnresolvedConstructExpr` |
| coroutines | `CoawaitExpr`, `CoyieldExpr` (WP-351; clang's `CoroutineSuspendExpr`): `a` = a list `[operand, common (the awaiter), ready, suspend, resume, opaque]` (`ExprArena::coawait_parts` → `CoawaitParts`), the type and value category are `resume`'s; `opaque` is the lvalue `OpaqueValueExpr` binding `common` in the three calls, so a visitor walks only the first five (`children`); `aux` bit 0 = implicit (an initial/final suspend point or the `co_await` of a `co_yield`, `coawait_is_implicit`), bits 1-2 = `SuspendKind` `Void`/`Bool`/`Handle` (`coawait_suspend_kind`; `Handle`'s suspend expression is `await_suspend(h).address()`). In a template pattern a `co_yield` is a type-dependent `CoyieldExpr` whose list holds only the operand (`coawait_parts` is `None`) and a `co_await` is a `DependentCoawaitExpr` with `a` = the operand. `coawait_operand` reads the operand of all three. Built by `Builder::coawait_expr` / `dependent_coawait` / `opaque_value_lvalue` |
| constant | `ConstantExpr` (M2; caches the value, §9.2) |
| GNU / builtins | `StmtExpr` (M2), `ChooseExpr`, `VAArgExpr` (WP-171: `a` = the `va_list` operand, `b` = the written type, `cold.trait_rhs_loc` = the type's begin, `cold.end_loc` = the `)`; the node type is the written type without references, always a prvalue; accessor `ExprArena::va_arg`), `AtomicExpr` (`__c11_atomic_*`, `__atomic_*`; WP-154: `aux` = the builtin id (`ExprArena::atomic_op`), `a` = a list of the operands in clang's order `ptr`, `order`, `val1`, `order_fail`, `val2`, `weak`, only those the op takes (`atomic_subs`); the per-op shape is `kiln_ast::decl::builtin::atomic`), `ShuffleVectorExpr` (`__builtin_shufflevector`; WP-282: `a` = a list of the operands as written, the two vectors then the indices (`shuffle_subs`); `aux` bit 0 = the mask is known, then `b` = a raw list of the `u32` lane indices, `u32::MAX` for -1 (`shuffle_mask`); `cold.end_loc` = the `)`; dumped and printed as clang), `ConvertVectorExpr`, `ExtVectorElementExpr` (WP-307/312: `a` = the base vector, or for `->` (`ExprFlags::ARROW`) the pointer to it (`operand`), `b` = the accessor's `Symbol` (`ext_vector_accessor`); for `.` the value category is the base's, a glvalue with `ObjectKind::VectorComponent`; built by `Builder::ext_vector_element`; `kiln_ast::expr::ext_vector_access` decodes the accessor to lane numbers as clang's `getEncodedElementAccess`; dumped `ExtVectorElementExpr '<type>' <accessor>`) |
| reflection (M6) | `CXXReflectExpr` (`^^`: operand type/decl/namespace/expr → `ReflectionId` when non-dependent), `CXXSpliceExpr` (`[: e :]` in expression position; designated entity once evaluated; a dependent `template [: e :] < args >` keeps its arguments in the cold `splice_args` map, `Exprs::splice_template_args`, WP-482) |
| recovery | `RecoveryExpr` (children kept; `ERROR` dependence; type if known) |

**Reserved expression kinds** (WP-2026). `UserDefinedLiteral`,
`EmbedExpr` and `UnresolvedMemberExpr` are in `ExprKind` but nothing
constructs them: a user-defined literal is a plain `CallExpr`
(spec/sema-overload.md §6.7), `#embed` reaches the parser as the tokens
of its bytes (spec/pp.md), and an overloaded member access is a
`MemberExpr` of type `QualType::OVERLOAD` (§5.10). They are kept for the
same reason as the reserved declaration kinds (§4.2): `0x21 + ExprKind`
is part of the ODR definition hash, and the numbering is stable.


Class-feature payloads (`a`, `b`, `aux` of `CXXConstructExpr`,
`CXXTemporaryObjectExpr`, `CXXInheritedCtorInitExpr`,
`CXXMemberCallExpr`, `CXXThisExpr`, `CXXDefaultArgExpr`,
`CXXDefaultInitExpr`, `MaterializeTemporaryExpr`,
`CXXBindTemporaryExpr`, `ExprWithCleanups`, `CXXParenListInitExpr`,
`ArrayInitLoopExpr`, `ArrayInitIndexExpr`, `OpaqueValueExpr`,
`CXXTypeidExpr` and the named/functional casts) are spec/sema-class.md
§5.1, with typed accessors on `Exprs` (`construct_info`,
`member_call_object`, `lifetime_extended_temporary`, ...). Two flags are
shared `ExprFlags` bits: `ARG_IS_TYPE` (`typeid(T)` as `sizeof(T)`: `a` is
a raw `QualType`) and `HAD_MULTIPLE_CANDIDATES`. There is no `elidable`
construct flag (C++17 guaranteed elision). An `OpaqueValueExpr`'s `a` is
its source expression (or 0), shared by every use. A `CXXMemberCallExpr`
callee keeps the parentheses as written (`(o.f)()`, `((o.*pm))()`,
[expr.call]/1); `member_call_object` looks through them to the
`MemberExpr` or `.*`/`->*` operator, as clang's
`getImplicitObjectArgument` does. A `CXXDefaultArgExpr` used in a
lifetime-extending context may carry its own copy of the parameter's
default argument (cold `default_arg_rewritten`, write-once through
`set_default_arg_rewritten`; dump `has rewritten init`), which
`Ast::default_arg_expr` returns in place of the shared one (WP-1149,
spec/sema.md §7.6a). A `CXXDefaultInitExpr` may carry a copy of the
field's default member initializer in the same map (WP-1177), which
`Ast::default_init_expr` returns in place of the field's. `ExprArena::clone_with_children(e, lists, kids)`
copies one node with new children (same kind, type, flags and cold
payloads) for the structural kinds that rewrite needs, `None` otherwise.
A `LambdaExpr` copy gets a new `LambdaData` side record holding the new
capture initializers and shares the closure type, call operator and body
(WP-2507): the copy is the same lambda-expression, so it has the same
closure type.

**Lifetime-extended temporaries of instantiations** (WP-3271). Every
extended `MaterializeTemporaryExpr` reached from a variable's
initializer names a `LifetimeExtendedTemporary` whose `extending` is
that variable, and no other variable's initializer reaches it: an
instantiation's variables extend their own temporaries, never the
pattern's ([temp.inst]: each specialization's variables are distinct
objects, and so are the temporaries bound to them). This covers every
extending context: a local or static reference and what its initializer
binds (a member subobject of a temporary included), the reference
members of an aggregate, a structured binding's hidden variable, an
inline static data member or variable template specialization, and the
range-based `for`'s `__range` with its P2718R0 temporaries (default
arguments included). The substituter shares non-dependent
subexpressions with the pattern by id (spec/sema-templates.md §5.4);
in the initializer of a variable it instantiates (and of `__range`) it
first copies the spine above each such temporary
(`Sema::unshare_temporaries`): `ExprArena::clone_unextended(mte,
operand)` copies an MTE as a full-expression temporary with no record,
which extension then gives the instantiated variable's own record
(mangled `_ZGR<the specialization's name>_`, as clang). Copying stops
where a new full-expression or unevaluated operand starts (a nested
`ExprWithCleanups`, a `ConstantExpr`, a lambda, a statement expression,
`sizeof`, `noexcept`, a requires-expression, ...). The walk keeps its
own stack, so a tree of any depth is copied (WP-3292: it stopped below
256 levels), as does the walk that finds the temporaries a reference
binds (`Sema::temporaries_bound_by`, `temporaries_in_aggregate`: below
256 levels the temporary was not extended at all). The same copy is
made for an expansion statement's expansion-initializer and
expansion-init-list (WP-3291), whose range variable or element
declarations extend them. Constant evaluation is unchanged: the copies
are the same expressions.

Builtin *functions* (`__builtin_memcpy`, `__builtin_is_constant_evaluated`,
`__builtin_is_within_lifetime`, `__builtin_constant_p`, ...) are ordinary
`CallExpr`s whose callee is a `DeclRefExpr` to an implicit `Function`
decl carrying a `BuiltinId` in its cold data (as clang).

### 5.4 Cast kinds (`aux` of the cast kinds)
Each implicit conversion of [conv] is one `ImplicitCastExpr`; a standard
conversion sequence is a chain (`LValueToRValue` innermost). Kinds
(clang's `CastKind`, C++-relevant subset, stable numbering):

`Dependent`, `NoOp` (qualification, function-pointer `noexcept`,
identity), `LValueToRValue`, `LValueToRValueBitCast`, `LValueBitCast`,
`BitCast`, `ArrayToPointerDecay`, `FunctionToPointerDecay`,
`BuiltinFnToFnPtr`, `NullToPointer`, `NullToMemberPointer`,
`IntegralToPointer`, `PointerToIntegral`, `PointerToBoolean`,
`IntegralCast`, `IntegralToBoolean`, `BooleanToSignedIntegral`,
`IntegralToFloating`, `FloatingToIntegral`, `FloatingToBoolean`,
`FloatingCast`, `DerivedToBase`, `UncheckedDerivedToBase`, `BaseToDerived`,
`Dynamic`, `DerivedToBaseMemberPointer`, `BaseToDerivedMemberPointer`,
`MemberPointerToBoolean`, `ReinterpretMemberPointer`,
`UserDefinedConversion`, `ConstructorConversion`, `ToVoid`, `ToUnion`
(GNU), `VectorSplat`, `NonAtomicToAtomic`, `AtomicToNonAtomic`,
`IntegralRealToComplex`, `FloatingRealToComplex`, `IntegralComplexCast`,
`FloatingComplexCast`, `IntegralComplexToReal`, `FloatingComplexToReal`,
`IntegralComplexToBoolean`, `FloatingComplexToBoolean`,
`IntegralComplexToFloatingComplex`, `FloatingComplexToIntegralComplex`.

Derived-to-base casts store the base path (`ListId` of `BaseSpec`s) in a
side record. Explicit casts carry the same cast kind for their *last*
step; earlier steps are `ImplicitCastExpr` children with the
`part_of_explicit_cast` flag (clang's model; the M2 dump shows one).

### 5.5 Constant results on expressions
Sema wraps each expression that the language requires to be a constant
expression and that is *not value-dependent* in `ConstantExpr { a =
sub-expression, b = ConstValueId }`: case labels, array bounds, enumerator
initializers, bit-field widths, template arguments, `alignas`,
`noexcept(e)`, `explicit(e)`, `if constexpr` conditions, immediate
invocations ([expr.const]) and `[: :]` operands. The value is stored
once, by sema, from `Oven::evaluate`. `static_assert` conditions are not
wrapped (clang compatibility; the result is used immediately and not
needed later). Values of non-constant-context folding (`try_fold`) are
**not** stored in the AST; the Oven keeps its own bounded cache
(spec/oven.md).

### 5.6 Syntactic forms not kept
- `InitListExpr` keeps only the **semantic** form (one initializer per
  element/field in order; `array_filler` for the tail; union active field).
  (clang keeps both forms: ~2× for large initializers; §13 Q7.)
- Templates are the exception (WP-192). A braced list in a dependent
  context is an `InitListExpr` of type `void` holding the clauses as
  written. A designated clause is a `DesignatedInitExpr` (type `void`,
  value category of its initializer, located at its first designator)
  whose designators are in the cold map `designators: ExprId ->
  [Designator]`. A `Designator` is `Field { name, loc }` (loc = the
  name) or `Index { index, loc }` (loc = the `[`; a constant index is
  wrapped in its `ConstantExpr`, a value-dependent one is kept as
  written) or the GNU `Range { first, last, loc, ellipsis }` (WP-193;
  both bounds as for `Index`, dumped `[first ... last]`). `.m = e...` is a `PackExpansionExpr` of the
  `DesignatedInitExpr`, as in clang. A non-dependent designated list in
  a template pattern is analyzed at once, and its written form (the same
  `void` list) is kept in the cold map `syntactic_list: ExprId -> ExprId`
  of the semantic list. Instantiation rebuilds from that written form,
  as clang's `TransformInitListExpr` does from the syntactic form.
  Written forms are shared: the written form of a list nested in a
  designated list is the very node of the inner list's own written form
  (sema keeps the written forms of the lists of one outermost designated
  list by their braces while it is analyzed), so a list nested N deep
  has N + 1 written lists, not one copy of every inner level per level,
  (N + 1)(N + 2)/2 (WP-2478, a quadratic AST before; test
  `crates/kiln_parse/tests/parse_scaling.rs`, `nested_designated_lists`,
  counts the expression nodes at N and 4N). The written form is a DAG
  there; every walk of it (dump, instantiation, the PCH body writer's
  expression memo) handles a shared node.
  Outside templates no `DesignatedInitExpr` is built.
- `ParenExpr` is kept (it changes `decltype` and [temp.over.link]).
- No `CXXRewrittenBinaryOperator` syntactic form
  beyond its one child (the semantic call), no `TypoExpr` (typo correction
  is resolved before the node is built).

### 5.7 New- and delete-expressions (WP-061a)
- **`CXXNewExpr`**: `ty` is the result pointer type (for an array new, a
  pointer to the element type: `int (*)[3]` for `new int[n][3]`).
  - `a` indexes the `news` side table of `NewExtra` records
    (`Exprs::new_extra(e) -> &NewExtra`, `Copy`):
    - `allocated_ty`: the created object's type, or for an array new the
      element type (clang's `getAllocatedType`);
    - `array_size: Option<ExprId>`: the first bound converted to
      `size_t` (an `IntegerLiteral` synthesized from the initializer for
      `new T[]{...}`); `None` for a non-array new;
    - `init: Option<ExprId>`: the semantic initializer (`InitListExpr`
      with `array_filler` for an array, `InitListExpr 'T'` for a scalar
      `{x}`, `ImplicitValueInitExpr` for `()`, or the converted
      expression); `None` for default-initialization;
    - `placement: Option<ListId>`: the converted placement arguments;
    - `operator_new`, `operator_delete: Option<DeclId>`: the allocation
      function and the matching deallocation function (called if
      initialization throws);
    - `type_loc`: the start of the type-id.
  - `aux` is the `NewInitStyle` (`None`, `Parens`, `Braces`;
    `Exprs::new_init_style`).
  - Flags: `GLOBAL` (`::new`), `ARRAY_FORM` (an array new) and
    `PASS_ALIGN` (the allocated type is over-aligned, so the allocation
    function takes `std::align_val_t`).
  - The closing token (the initializer's `)`/`}` or the type-id's last
    token) is in the cold `end_loc` map.
  - Children (`Exprs::children`): array size, initializer, placement
    arguments (clang's order).
  - Dependence: type-dependent only through the allocated type; a
    type-dependent array bound, initializer or placement argument makes
    the new-expression value-dependent, not type-dependent
    ([temp.dep.expr]/3-4; clang's `turnTypeToValueDependence`), so
    `P p(new D(I()))` in a template is not checked as copy-initialization
    at definition time (WP-704; test
    `tests/conformance/temp.dep.expr/new-expression-dependent-initializer.cpp`).
- **`CXXDeleteExpr`**: `ty` is `void`; `a` is the operand (converted to a
  pointer prvalue; `Exprs::delete_operand`); `b` is the deallocation
  function (`Exprs::operator_delete`, `None` for an erroneous operand);
  `aux` bit 0 says it takes the size and bit 1 the alignment
  (`Exprs::delete_passes`). Flags: `GLOBAL`, `ARRAY_FORM` (`delete[]`, or
  an operand that points to an array: clang deletes it as an array) and
  `ARRAY_AS_WRITTEN` (`delete[]` was written). `ARRAY_AS_WRITTEN` and
  `PASS_ALIGN` share bit 6: they belong to different kinds.
- Builders: `Builder::cxx_new(ty, flags, style, extra, loc, end)` and
  `Builder::cxx_delete(operand, operator_delete, flags, sized, aligned,
  ty, loc)`.

### 5.8 Source-location builtins (WP-088)
- **`SourceLocExpr`**: `__builtin_FILE()`, `__builtin_FILE_NAME()`,
  `__builtin_FUNCTION()`, `__builtin_LINE()`, `__builtin_COLUMN()` and
  `__builtin_source_location()` ([support.srcloc.cons]; clang
  `SourceLocExpr`). Built by `AstBuilder::source_loc_expr(kind, ty, ctx,
  loc, rparen)`; `aux` is the `SourceLocKind`
  (`Exprs::source_loc_kind`), `a` the declaration context it appears in
  (`Exprs::source_loc_context`); the `)` is in the cold `end_loc` map.
  A prvalue of `const char *` (`FILE`, `FILE_NAME`, `FUNCTION`),
  `unsigned int` (`LINE`, `COLUMN`) or
  `const std::source_location::__impl *`, with its type's dependence; no
  children.
- **Values** (`exprs.cold.source_loc: ColdMap<ExprId, SourceLocValues>`;
  `Exprs::source_loc_values` / `set_source_loc_values`): sema computes
  them, since it owns the source manager and the function-name printer.
  `SourceLocValues { file, file_name, function: ConstValueId, line,
  column: u32, location: Option<ConstValueId>, at, ctx }`; the
  `ConstValueId`s are interned pointers to string objects kiln makes
  (one per content, role and `ctx`, §5.8.1) and, for `location`, to the
  frozen `__impl` object of that location.
  The key is the `SourceLocExpr` itself (its own location and context),
  or a `CXXDefaultArgExpr` that uses a default argument containing one
  (the call's location and the caller's context, [dcl.fct.default]/9 via
  [support.srcloc.cons]/2), or a `CXXDefaultInitExpr` using a default
  member initializer containing one (the constructor's or the aggregate
  initialization's location and context). Consumers (Oven, codegen) take
  the values of the outermost `CXXDefaultArgExpr`/`CXXDefaultInitExpr` in
  scope that has an entry, else the `SourceLocExpr`'s own.

#### 5.8.1 Kiln-made strings and their makers (WP-3000)
Some string objects no `StringLiteral` of the source spells are made by
kiln: the strings of the source-location builtins (and of
`std::source_location`), reflection's strings, and a contract site's
strings. Each is a `StringLiteral` expression without a location (the
`origin` of every `PtrBase::StringLit` pointer into it), and its maker is
recorded in `exprs.cold.string_maker: ColdMap<ExprId, StringMaker>`:
- `StringMaker::SourceLoc { role, decl }`: the value of `role`
  (`SrcLocRole::File` `__builtin_FILE()` / `file_name()`, `FileName`
  `__builtin_FILE_NAME()`, `Function` `__builtin_FUNCTION()`,
  `PrettyFunction` `function_name()`) of a use evaluated in `decl`.
  One object per (content, role, `decl`): every use whose
  `SourceLocValues` point at the object has that role and that `ctx`, so
  the record of §5.8 lists all of an object's uses. A default argument
  (or default member initializer) evaluated at a use counts as a use in
  the context of the call (or of the construction), as §5.8 records it
  (`Sema::note_default_arg_use`, [dcl.fct.default]/9 via
  [support.srcloc.cons]/2): `void g(const char *s =
  __builtin_FUNCTION())` called from `h()` makes `"h"` with
  `decl = h`.
- `StringMaker::Reflection`: `identifier_of` and the other name queries,
  `symbol_of`, a `meta::exception`'s message and the strings of
  `source_location_of` (one object per content and encoding among
  them).
- `StringMaker::ContractSite`: a contract assertion's file, function and
  predicate text (spec/contracts.md §6.3; one object per content among
  them).

Queries (for embedders): `ExprArena::string_maker(origin)`, and
`Ast::string_maker_of(v)` from a frozen pointer value to `(maker,
origin)`. A literal the source spells (and a predefined `__func__`) has
no maker.

The rule is the language's: nothing ties the identity of these strings
to their contents. [support.srcloc.obs] returns an NTBS and says nothing
of its identity; the builtins' strings are not *string-literal*s, and even
for those [lex.string]/9 leaves whether equal literals share an object
unspecified (they are potentially non-unique objects, [intro.object]/9).
The strings `define_static_string` and `define_static_array` make are
another path (template parameter objects, spec/reflection-values.md §4:
equal contents give one object) and stay shared by content.

The maker is the only difference: kiln-made strings with equal contents
and encoding have one address at run time (codegen's string pool merges
them, `private unnamed_addr`, as clang 19 and 22 emit one `.str` per
content) and are one object to the Oven's pointer comparison and
subtraction (spec/oven.md "Kiln-made strings"), so `f(1) == f(1L)` for
`constexpr const char *f(T) { return __builtin_FUNCTION(); }` is a
constant `true` as in clang 19 and 22 (whose
`getPredefinedStringLiteralFromCache` keeps one `StringLiteral` per
content) and GCC 16.1. A kiln-made string against a literal the source
spells stays a comparison of potentially overlapping literals.
Tests: `tests/conformance/support.srcloc/string-identity*.cpp`,
`meta.reflection.names/string-identity-srcloc.cpp`,
`kiln_parse/tests/string_maker.rs`.

#### 5.8.2 Immediate invocations of a default argument, per use (WP-3278, WP-3279)
An immediate invocation in a default argument or a default member
initializer is evaluated at each use, in the context of that use (CWG
2631; [expr.const]/16, [dcl.fct.default]/5, [class.base.init]/9 in the
working draft). Its `ConstantExpr` belongs to the shared default
argument (or initializer) and has no value of its own
(`constant_value` is `None`; the dump elides it at the declaration). The
values sema found are kept by use:
- **A use with source-location values** (a `CXXDefaultArgExpr` /
  `CXXDefaultInitExpr` with an entry in `exprs.cold.source_loc`, §5.8):
  `exprs.cold.use_imm: ColdMap<ExprId, Box<[UseImmediate]>>`, keyed by
  the use beside its `source_loc` entry. `UseImmediate { invocation:
  ExprId, value: ConstValueId }` is the value of the invocation at that
  use, in evaluation order, the invocations of the default arguments
  and initializers the use's argument uses included (they are evaluated
  in the outermost use's context too). Written once per (use,
  invocation) by sema (`ExprArena::note_use_immediate`); read with
  `ExprArena::use_immediates(u)` / `use_immediate(u, c)`.
- **A use without them**: every use evaluates the invocation the same
  way, so the value is kept once, on the invocation
  (`exprs.cold.default_imm`, `ExprArena::default_immediate_value(c)`).
  It is never the value at a use with source-location values: with
  `void log(unsigned n = len(__builtin_FUNCTION()))` called from `f`,
  `default_immediate_value` of the `len(...)` wrapper is `None` and the
  use's record holds 1.

`Ast::default_use_immediate_value(u, c)` answers for any use: the use's
record when it has source-location values, else `default_imm`. `None`
when sema kept no value: the invocation failed (an error was reported),
its value depends on the evaluation point (`access_context::current()`,
WP-1462), or the use is in a template pattern, or in a body imported
from a module image (images carry the use's location and context, not
its values: codegen evaluates the invocation at the use there). PCH
snapshots carry both maps (`ColdExprData`). The dump prints the value at
the outermost use on the path as the `ConstantExpr`'s `value:` (clang's
rebuilt default argument). Codegen's fold prepass and the Oven
(`Compiler::stored_value`) read these values instead of evaluating the
call again. Tests: `tests/conformance/expr.const/default-arg-immediate-per-use*.cpp`,
`tests/conformance/class.base.init/*default-member-init-immediate-per-use*.cpp`,
`kiln_parse/tests/default_use_immediate.rs`.

### 5.9 Lambdas (WP-075)
- **`LambdaExpr`** ([expr.prim.lambda]; spec/sema-lambda.md): `a` is its
  `LambdaId`, an index into `exprs.lambdas: LambdaArena` (side records
  `kiln_ast::lambda::LambdaData`, `Exprs::lambda_id`); its children are
  the initializers of the closure's capture members, in member order
  (clang `capture_inits`). A prvalue of the closure type. Built by
  `act_on_lambda_end`.
- **`LambdaData`**: `closure` (the `CXXRecordDecl`), `call_op` (the
  call operator, or for a generic lambda the pattern of its template),
  `default` / `default_loc` (`CaptureDefault`), `captures:
  Vec<LambdaCapture>` (kind `This`/`StarThis`/`ByCopy`/`ByRef`, the
  variable — an init-capture's own `VarDecl` —, the member `field`,
  `loc`, pack `ellipsis`, `implicit`, `init`), `inits: Option<ListId>`,
  `body: Option<StmtId>` (also the call operator's body), `lbracket` /
  `rbracket`, the bits `explicit_params`, `explicit_result`, `mutable`,
  and `mangling: LambdaMangling { number, context }` (abi.md §8.9).
  Written by sema while the lambda is open (§2.4: the record is
  completed at `}`), immutable after.
- **Closure classes**: `RecordData.lambda: Option<LambdaId>` names the
  same record (`DeclArena::set_closure`); `DefinitionBits::LAMBDA` and
  `LAMBDA_CAPTURES` (a capture-default or a capture). Queries on `Ast`:
  `closure_lambda(record)`, `call_op_lambda(f)` (the call operator or a
  specialization of its template), `closure_invoker(record)`, and
  `closure_thunk(f) -> Option<ClosureThunk>` (`Conversion { target }` /
  `Invoker { call_op }`: the body-less closure members the Oven and
  codegen synthesize).
- **Captured names**: a `DeclRefExpr` to a captured entity carries
  `ExprFlags::REFERS_TO_ENCLOSING` (clang `refersToEnclosingVariableOrCapture`).
- **Closure owners** (WP-2023): `Ast::closure_owner(record) ->
  Option<DeclId>` is the innermost declaration whose source text holds
  the closure's lambda-expression, for tooling clients that key a
  closure as a local type of a declaration; `None` for a record that is
  not a closure. Independent of `LambdaMangling` (that follows the
  Itanium numbering rules; this is set for every closure). The owner is:
  the template parameter, for one in a template parameter's default
  argument or type; the template's templated declaration (the function,
  class, variable or alias), for one in a template head's
  requires-clause; the function, for one in its signature, trailing
  requires-clause or body (a closure in a function body is owned by the
  innermost function whose body holds it, a nested lambda's call
  operator or a local class's member function included, never by a
  block-scope variable; a local class's default member initializer
  belongs to its field); the parameter, for one in its default argument
  or type (a lambda's parameter too, in a function body or not, nested
  lambdas' innermost; WP-2627); the alias (`using A = ...` / an alias template); the variable
  or field, for one in its initializer or type (a default member
  initializer included); the concept, for one in its
  constraint-expression; else the lexical context (a class, a namespace
  or the translation unit, e.g. for a `static_assert`). A
  requires-expression's parameter scope is skipped. An instantiated
  closure has the instantiated owner (spec/sema-lambda.md §3.6).
  Stored in the cold map `ColdDeclData::closure_owner` (own `MemRow`
  "closure_owner"), keyed by the closure record, only where the owner
  differs from the closure's semantic context (`closure_owner_default`,
  the context past a requires-expression body): a closure in a
  function body costs nothing. Decided as §18.3's `enclosing` is: at
  creation from sema's numbering state (`Sema::closure_site_owner`),
  then narrowed by the same marks and claims; a claim narrows only a
  closure whose owner existed at the mark and is not a function whose
  body holds it, and the parser's claims skip the closures substitution
  created meanwhile (an instantiation the declaration's text triggered).
  Template parameters claim closures only (not written references);
  template heads and alias declarations claim their names too, and
  enumerators and `static_assert`s claim names only (§18.3, WP-3143). Carried by PCH snapshots
  (the cold maps are snapshot state; FORMAT_VERSION 124) and module
  images (a lambda record's `owner` word, spec/modules.md; IMAGE_VERSION
  58). Tests: `crates/kiln_parse/tests/closure_owner.rs`.
- Memory: `exprs.lambdas` has its own `MemRow` ("lambdas", captures
  included).

### 5.10 Overloaded member accesses (WP-2020)
A member access whose lookup found several member functions, a member
function template or a static data member template ([expr.ref]/7; `x.f`,
`p->f`, an implicit `this->f`, `p->B::f<int>`) is a `MemberExpr` of type
`QualType::OVERLOAD` naming the first candidate (kiln's
`UnresolvedMemberExpr`). Sema keeps the candidate set for overload
resolution (`ovl.members`, spec/sema-overload.md §2.3) and consumes it
when a call or a target type resolves the access, to a new expression.
The facts of the access's name that resolution and instantiation read
are kept in the `Ast`, for the life of the `Ast` whether or not the
access is ever resolved, so a reader after the TU (a tooling client;
sema is gone) sees them in a template pattern:

- `cold.member_overload: ColdMap<ExprId, OverloadedMember>`, one entry
  per overloaded access (12 bytes): `qualifier: Option<NnsId>` (as
  written; no virtual dispatch, the lookup is in the qualifier's class),
  `template_args: Option<ArgListId>` (the explicit template arguments as
  written, [temp.arg.explicit]/1; recorded when the parser reaches them,
  `set_member_overload_template_args`; dependent arguments make the
  access a `CXXDependentScopeMemberExpr`, which holds them in the
  templates' `expr_nns` / `expr_targs` instead), and `naming_class:
  DeclId` ([class.access.base]/5: the class access is checked in, the
  qualifier's class for a qualified name). Written by sema where the
  access is created (`set_member_overload`) and by the module-image
  importer (spec/modules.md §11.11); the candidates themselves are not
  kept (a reader looks the name up again in the naming class). The
  ordinary `member_extra` of a `MemberExpr` (§5.3) is not used for an
  overloaded access, so its readers are unchanged.
- Query `Ast::member_access_name(e) -> Option<MemberAccessName>`
  (`qualifier`, `template_args`, `naming_class: Option<DeclId>`) for any
  member access: an overloaded `MemberExpr` gives all three, a resolved
  one its `member_extra` without a naming class, a
  `CXXDependentScopeMemberExpr` its qualifier and arguments without a
  naming class; `None` for an expression that is not a member access.
  `ExprArena::member_overload(e)` reads the raw record.
- Snapshots carry the map (§17); memory row `cold.member_overload`.

Tests: `crates/kiln_parse/tests/sema_facts.rs` (a non-dependent object, an
implicit `this->`, `p->Base::f<int>(x)` and `p->Base::f<T>(x)`).

### 5.11 Type operands as written (WP-3589)
**Stability promise.** Each type operand below is kept as written: the
type with the sugar the source wrote (a `decltype(e)` or `typeof(e)`, a
typedef or alias name, an array bound's written expression, a
template-id's arguments as written), not only its canonical type. This
holds in plain code, in a template's pattern and in its instances
(substitution rebuilds the sugar with the substituted expressions), and
across PCH snapshots and module images. A reader walks the operand with
`TypeContext::written_exprs` (spec/types.md §8.4), or an argument list
with `written_arg_exprs`, to reach every expression, and so every call,
written in it. The accessors are part of the promise: dropping one, or
answering with a canonical or completed type where this list says "as
written", is an interface change (docs/AGENTS.md rule 4).

| operand | expression kinds | accessor |
|---|---|---|
| a C-style cast's type | `CStyleCastExpr` | the node's type |
| a named or functional cast's type | `CXXStaticCastExpr`, `CXXConstCastExpr`, `CXXReinterpretCastExpr`, `CXXDynamicCastExpr`, `CXXFunctionalCastExpr` | `ExprArena::cast_written_type(e)`, else the node's type (a glvalue cast's written type is the node's type plus `&` / `&&` by value category; §5.3) |
| a functional cast's type in a pattern (dependent type or arguments) | `CXXUnresolvedConstructExpr` | `ExprArena::unresolved_construct_type(e)` |
| `sizeof` / `alignof` type operand (`__alignof`, `__datasizeof`, `vec_step` the same) | `UnaryExprOrTypeTraitExpr` | `ExprArena::trait_type_arg(e)` |
| type trait operands | `TypeTraitExpr` | `ExprArena::type_trait_args(e)` (a `ListId` of `QualType`s) |
| `typeid` type operand | `CXXTypeidExpr` | `ExprArena::typeid_operand(e)` (`TypeidOperand::Type`) |
| `new`'s allocated type | `CXXNewExpr` | `ExprArena::new_extra(e).allocated_ty` (an array new's element type; its first bound is `array_size`, an expression, §5.7) |
| a compound literal's type | `CompoundLiteralExpr` | `ExprArena::compound_literal_written_type(e)` |
| a name's explicit template arguments | `DeclRefExpr`, `MemberExpr`; in a pattern `UnresolvedLookupExpr`, `CXXDependentScopeMemberExpr`, `DependentScopeDeclRefExpr` | `ExprArena::decl_ref_extra(e).template_args`; an overloaded member access's `Ast::member_access_name(e).template_args` (§5.10); a dependent name's `TemplateTables::expr_targs` |
| a concept-id's arguments | `ConceptSpecializationExpr` | `ExprArena::concept_spec_args(e)` |

A compound literal of an array of unknown bound (`(T[]){1, 2}`, a GNU
extension in C++) has the type its list completes (`T[2]`,
[dcl.init.aggr]/4) as its node type; the written `T[]` is then kept in
the cold map `compound_literal_written: ColdMap<ExprId, QualType>`
(`AstBuilder::set_compound_literal_written`, recorded only when it
differs from the node's type, like `cast_written`), and instantiation
substitutes the written type (clang `TransformCompoundLiteralExpr` reads
`getTypeSourceInfo`). PCH snapshots carry the map (`FORMAT_VERSION`
187); a module image carries no compound literal yet (an interface
holding one in a body the image writes is replayed from source, which
keeps it); memory row `cold.compound_literal_written`.

Other type operands carry no promise yet: `va_arg`'s type,
`__builtin_offsetof`'s, `__builtin_bit_cast`'s and
`__builtin_convertvector`'s, an array type trait's, a pseudo-destructor
name's, a value-initialization `T()` and a temporary object `T{...}`.
A declaration's type is no operand: spec/types.md §8.4 and
`Ast::decl_written_exprs` cover it (nothing for a deduced variable).

Tests: `crates/kiln_parse/tests/written_bounds.rs`
`every_type_operand_is_kept_as_written` (each operand in plain code and
in a function template's pattern and instance),
`a_compound_literal_keeps_its_unknown_bound`.

## 6. Statements
```rust
struct StmtNode { kind: StmtKind /*u8*/, /* 1 padding byte */ aux: u16, loc: Loc, a: u32, b: u32 }  // 16 bytes
```
A statement has no flags byte: the former `flags: u8` had no reader or
writer and was removed (WP-2026); per-kind bits live in `aux`.

| kind | payload / notes |
|---|---|
| `CompoundStmt` (M2) | `a` = `ListId` of statements; rbrace loc in `b` |
| `DeclStmt` (M2) | `a` = `ListId` of decls |
| `ExprStmt` (M2) | `a` = `ExprId` (an expression statement; the dump elides it, §11) |
| `NullStmt` (M2) | |
| `IfStmt` (M2) | side record `{init, cond_var, cond, then, else}` (optional slots); `aux` = ordinary/`constexpr`/`consteval`/`!consteval`. `if constexpr`'s cond is a `ConstantExpr`; in an instantiation the discarded branch is not instantiated (its slot is empty); in a non-template both are kept and consumers use the constant |
| `SwitchStmt` (M2) | `{init, cond_var, cond, body}`; case list `ListId` (for jump tables / the Oven) |
| `CaseStmt` (M2) | `{lhs ConstantExpr, rhs (GNU range) or none, sub}` |
| `DefaultStmt` (M2), `BreakStmt` (M2), `ContinueStmt` (M2) | |
| `WhileStmt` (M2), `DoStmt` (M2), `ForStmt` (M2) | `{init, cond_var, cond, inc, body}` |
| `ReturnStmt` (M2) | `a` = value; `b` = NRVO candidate `DeclId` |
| `LabelStmt`, `GotoStmt` | `LabelDecl` ids |
| `IndirectGotoStmt` (GNU `goto *e;`, WP-444) | `a` = the target `ExprId`, converted to `const void *` unless type-dependent (`indirect_goto_target`); `b` = the `*` location (`indirect_goto_star`); `loc` = the `goto`. `Ast::indirect_goto_constant_target` gives the label of a target that is `&&l` after implicit casts and parentheses (clang `getConstantTarget`). The dump prints clang's `IndirectGotoStmt` line with the target as its child (`builder.indirect_goto_stmt`) |
| `AttributedStmt` | attrs `ListId` + sub (`[[likely]]`, `[[fallthrough]]`, `[[assume(e)]]`) |
| `CXXForRangeStmt` (WP-089) | clang's desugaring in the `RangeForData` side table: `{init: Option<StmtId>, range: DeclId, begin, end: Option<DeclId>, cond, inc: Option<ExprId>, loop_var: DeclId, body, colon: Loc}` (variables, not `DeclStmt`s; the dump prints each as clang's `DeclStmt`). `begin`/`end`/`cond`/`inc` are `None` in a template pattern whose range or loop-variable type is dependent. `colon` locates the implicit variables and expressions. In C++23 and later (P2718) the temporaries of the range initializer are `extended by Var '__range{d}'` (sema-class.md §5.4); `__range`, `__begin`, `__end` and the loop variable have `VarSpec` `cxx_for_range` (a LATE bit) |
| `CXXExpansionStmt` (WP-447) | `aux` = `ExpansionForm` (dependent / enumerating / iterating / destructuring, bits 0-1) plus bit 2 `EXPANDED`; `b` = the depth of its synthetic template level; `a` = index of an `ExpansionData` side record (`Stmts::expansion_data`): `{init: Option<StmtId>, decl: DeclId (the for-range-declaration, initialized from a `CXXExpansionSelectExpr`), exprs: ListId (expansion-init-list), range_init: Option<ExprId>, body: StmtId (pattern), prelude: ListId (expanded: `__range`/`__begin` or the structured binding `DeclStmt`s), expansions: ListId (expanded: one `CompoundStmt` `{ decl = element; body }` per element), colon: Loc}`. An expanded statement's code is its init, prelude and expansions; the pattern parts stay for the dump only. `break`/`continue` target the whole statement / the current element (spec/sema.md §7.6a) |
| `CXXExpansionSelectExpr` (WP-447, expression) | patterns only: the element of the expansion statement whose level is `a`; `aux` = its form; replaced by the element in each expansion |
| `CXXTryStmt` (WP-100) | `{block, handlers: ListId}` (`try_stmt`, `try_block`, `try_handlers`); `CXXCatchStmt {exception_decl or catch-all, block}` (`catch_stmt`, `catch_var`, `catch_block`). A class-type handler variable's init is a copy from an `OpaqueValueExpr` (`opaque_value_of(ty, vc, loc)`, a builder that makes a sourceless one) |
| `CoroutineBodyStmt` (WP-351) | `a` = index of a `CoroutineBodyData` side record (`Stmts::coroutine_body`) with clang's fixed slots: `body` (as written), `promise`/`promise_stmt`, `init_suspend`/`final_suspend` (implicit `CoawaitExpr`s), `on_exception` (`__promise.unhandled_exception();`, `None` with `-fno-exceptions`), `on_fallthrough` (implicit `co_return;`), `allocate`/`deallocate` (the `operator new`/`delete` calls over `__builtin_coro_size`/`__builtin_coro_free`), `return_value` (`get_return_object()`), `result_decl` (`__coro_gro` when its type is not the return type), `return_stmt` (`return __coro_gro;` converted, or `return get_return_object();` for the same type; `None` for `void`), `return_on_alloc_failure`, `param_moves` (`ListId` of `DeclStmt`s, parameter order). It replaces the function's body (`FunctionData::body`); built only outside template patterns (`Builder::coroutine_body_stmt`) |
| `CoreturnStmt` (WP-351) | `a` = the operand as written (`coreturn_operand`, `None` for `co_return;`), `b` = the promise call `return_value(e)`/`return_void()` as a full-expression (`coreturn_promise_call`, `None` in a template pattern), `aux` bit 0 = the implicit `co_return;` that ends the body (`coreturn_is_implicit`) |
| `GCCAsmStmt` | side record: template `StringLitId`, outputs/inputs `ListId` of `(constraint StringLitId, ExprId)`, clobbers, labels; volatile/goto bits |
| `ContractAssertStmt` (P2900, WP-706) | one contract assertion: a `contract_assert` statement or one `pre`/`post` of a function (spec/contracts.md §4). `a` = the predicate (`contract_cond`), `b` = a postcondition's `ContractResultName` (`contract_result_name`, 0 = none), `aux` bits 0-1 = `ContractKind` (assert/pre/post, `contract_kind`), bits 2-3 = the evaluation-semantic slot (`contract_semantic_slot`; 0 = the translation unit's `Ast::contract_semantic`) |

## 7. Attributes
`AttrNode { kind: AttrKind /*u16*/, syntax: u8 /*[[ ]], GNU, keyword, declspec*/, flags: u8, loc, a, b }`
(16 B). Arguments: `ExprId`s, a `Symbol`, or a side record. Declarations
with attributes have `HAS_ATTRS` and an entry in `decl_attrs: FxHashMap<
DeclId, ListId>`. `AttrArena::has_kind(k)` (whether any node of kind
`k` was ever pushed) is O(1), from a per-kind presence bitset set by
`push`; `Ast::inherited_attr` answers `None` from it without walking the
redeclaration chain (WP-1076). Unknown attributes are diagnosed and dropped (not
stored). Annotations (P3394, `[[=e]]`, WP-481) are not stored as
attributes: an `AttrKind::ANNOTATION` node (`a` = the operand `ExprId`)
lives only between the parser and sema, which moves each into the cold
side table `Ast::annots` (`kiln_ast::annot`: `Annotation { decl, value:
Option<ConstValueId>, loc, written_type, expr }`, 20 B, `AnnId`), keyed
by the declaration it is written on; `value` is `None` while the operand
is dependent. `AnnotArena::of_entity` gathers an entity's annotations
over its redeclaration chain (§4.4). Contract: spec/annotations.md.
`AttrKind::ASM_LABEL` (88): `a` = raw `StringLitId` of the label,
flags bit 1 = inherited from the previous declaration.
`AttrKind::BUILTIN` (89): implicit, `a` = builtin id, invalid location.
`AttrKind::FALLTHROUGH`: flags bit 2 (`FALLTHROUGH_SCOPED`) = written
with an attribute scope (`clang::`, `gnu::`; WP-203).

**Checked marker** (WP-2022, for tooling clients that read the AST after
the translation unit): flags bit 6 (`AttrNode::CHECKED`, any kind) = the
checks of the declaration or statement the attribute appertains to have
run on it (spec/sema.md §4.5 "Checked attributes"), so its argument words
have their checked meaning, not the as-written one. Several kinds are
rewritten by the check: `CLEANUP`'s `a` is the `ExprId` of the argument
as written before and the cleanup function's `DeclId` after;
`VISIBILITY`, `TLS_MODEL`, `CONSTRUCTOR`/`DESTRUCTOR`, `NONNULL` and
`CALLBACK` resolve their arguments likewise. Queries:
`AttrNode::is_checked()`, `AttrArena::is_checked(AttrId)`; sema sets it
with `AttrArena::mark_checked(&[AttrId])`, and `AttrArena::push` sets it
on an implicit node (bit 0), which sema builds in checked form. After the
translation unit every attribute in a declaration's list or an
`AttributedStmt`'s is checked (`kiln_parse/tests/attrs_checked.rs`); a
node no list reaches (one a check replaced or dropped) may not be. An
argument that depends on a template parameter keeps its deferred form
under its per-kind flag (`PRIORITY_DEPENDENT`, `MODE_DEFERRED`,
`SPACE_DEPENDENT`, `ALIGN_VALUE_DEPENDENT`) until instantiation. The bit lives in the existing
flags byte: the node stays 16 B, and the PCH snapshot and the module
image carry it with the flags unchanged in layout.

## 8. Templates (M3; representation fixed now)
### 8.1 Parameters
`TemplateParamList { depth: u16, params: ListId, requires: Option<ExprId> }`
in `templates.param_lists`; each template decl's payload has one. A
template parameter decl's payload is `{depth, index, pack, default,
type-constraint}`; its *canonical* type/name is `(depth, index, pack)`
(spec/types.md §5.3, §8.1).

### 8.2 `TemplLink` (in the payload of every templatable decl, 4 bytes)
```rust
pub struct TemplLink(u32);    // 0 = None; else 3-bit tag (bits 29..=31) + 29-bit index:
    // Pattern(TemplateIndex)       this decl is the pattern of a template decl (index into templates.decls)
    // Specialization(SpecId)       implicit/explicit specialization or instantiation of a template
    // MemberOfSpecialization(MemberSpecId)  member of a class template specialization
    //                              (clang's MemberSpecializationInfo)
    // (tag 4, once DependentFriend, is unused: nothing built it; WP-2026.
    //  A dependent friend template specialization is a Friend, spec/sema-templates.md)
struct SpecInfo {             // templates.specs, 24 bytes
    template: DeclId,         // first decl of the primary template
    args: ArgListId,          // as written (sugared); canonical via spec/types.md §8.3
    kind: SpecKind,           // Undeclared | ImplicitInstantiation | ExplicitSpecialization
                              //   | ExplicitInstantiationDeclaration | ExplicitInstantiationDefinition
    from: DeclId,             // pattern used: the primary template's pattern or a partial specialization
    from_args: Option<ArgListId>, // deduced partial-specialization args
    point_of_instantiation: Loc,
}
struct MemberSpecInfo { instantiated_from: DeclId, kind: SpecKind, point_of_instantiation: Loc } // 12 bytes
```
`instantiated_from(d)` is `SpecInfo.from` / `MemberSpecInfo.instantiated_from`
for decls with a `TemplLink`, and a sparse map for other kinds
(`Field`, `EnumConstant`, `ParmVar`, `Typedef`, `UsingShadow`, local
decls), so every instantiated decl reaches its pattern in O(1).
An unnamed member class (`struct { T x; } m;`, an anonymous struct or
union, nested ones, those of member class templates and partial
specializations) is a `MemberOfSpecialization` of the class it is
instantiated in, exactly as an unnamed member enumeration is
(`ImplicitInstantiation`, its `instantiated_from` the pattern's unnamed
class; clang's `setInstantiationOfMemberClass`), and never a
`Specialization`: it has no `SpecInfo`, no template arguments and no
name of its own (WP-3273). The implicit member object of an anonymous
struct or union and its injected `IndirectField`s reach theirs through
the sparse map.

### 8.3 Specialization keys
One global `specializations: FxHashMap<(DeclId /*first decl of template*/,
ArgListId /*canonical*/), DeclId>` for class, function, variable, alias
(alias specializations are types, not entries) and partial
specializations (keyed by their canonical pattern args; listed per
template in `partials: DeclId -> ListId`). The template decl's payload
lists its specializations in creation order (dump order, ast-dump.md §5).

**Arguments as written (WP-3016, WP-3142).** The key, `SpecInfo.args`
and `PartialSpecInfo.args` hold the converted arguments (a non-type
argument is its value: `P<T, f(1)>`'s second argument is `1`), which
matching ([temp.spec.partial.match]), partial ordering and the
specialization's identity read. The arguments as written are kept
beside them in one table, `TemplateTables::args_as_written(d)` (clang's
`getTemplateArgsAsWritten`), an `ArgListId` in which a non-type
argument is the expression written (`ArgView::Expr`), never its value,
also when it is instantiation-dependent but not dependent
(`h<sizeof(sizeof(f(T())))>` in a class template stays the expression,
not `8`). It has a row for: an explicit specialization or explicit
instantiation of a class template (the type printer prints these); a
partial specialization of a class or variable template (`d` its
declaration; printed with its converted arguments, as clang's injected
class name); an explicit specialization of a variable template; and
each declaration of an explicit specialization of a function template
that writes `<...>` (namespace or class scope, `d` that declaration),
none for one whose arguments are all deduced. The first declaration's
row stays, except as the class rule above says (clang 19 resets a
class's at its definition). A client reads the written expressions of
any of them with one routine, `TypeContext::written_arg_exprs`
(spec/types.md §8.4). The PCH snapshot carries the table, and so does
a module image (WP-3102): every specialization it lists carries its row
beside the arguments it is matched by, each non-type argument as its
expression section, which the importer builds and never converts or
evaluates (spec/modules.md §11.11 "Arguments as written"); an argument
a section cannot hold leaves that declaration without a row (it then
reads as before WP-3102: the converted arguments only). A partial
specialization of a member template redeclared in an instantiation of
its class ([temp.spec.partial.member]/2, `Sema::member_partial_spec`)
gets the original's row substituted with the enclosing arguments, as
clang's `InstantiateClassTemplatePartialSpecialization` substitutes
`getTemplateArgsAsWritten` (`In<U, f(7)>` in `O<long>` keeps `f(7)`;
WP-3102); none when that substitution fails.

A class-scope explicit specialization of a member function template in a
class template ([temp.expl.spec]/2, CWG 727) is redeclared in each
instantiation of the class as a specialization of the member template
there, its arguments as written substituted (the converted ones when it
has no row; clang's `VisitCXXMethodDecl`), so a non-type argument no
parameter deduces (`h<8>()`) still selects it; its `SpecInfo.from` is
the pattern class's declaration, whose definition it is instantiated
from (clang's member specialization info), the one exception to "an
explicit specialization is never instantiated" ([temp.expl.spec]/7).

### 8.4 Deduction results, substitution, instantiation stack
- Deduction produces a `DeducedArgs` scratch (a `Vec<TemplateArg>` with
  `Null` for undeduced) owned by sema; the result is interned as an
  `ArgListId` and stored only in the `SpecInfo` it produces. Failed
  deductions store nothing in the AST (diagnostic notes are rendered
  from sema's scratch).
- Substitution creates `SubstTemplateTypeParm` sugar (spec/types.md §5.3,
  open question T3), `SubstNonTypeTemplateParmExpr` (replacement expr +
  parameter decl) and `SubstTemplateTemplateParm` names.
- The **instantiation stack** (for "in instantiation of ... required
  here" notes, depth limits, SFINAE contexts) is sema state, not AST. The
  AST stores the point of instantiation per specialization (above).
- Function body instantiation is lazy (end of TU, or on demand when the
  Oven needs a body: `SemaBridge::ensure_body`, §13 Q2).

### 8.5 Definition-context operator lookup sets (WP-2021)
A unary, binary or compound-assignment operator expression with a
type-dependent operand, built in a template, and a fold-expression (its
operator; WP-2625), keeps the non-member
operator functions the unqualified lookup of its operator name (and of
its rewritten forms' names for `==`, `!=`, `<`, `>`, `<=`, `>=`,
[over.match.oper]/3.4) found at the definition ([temp.dep.candidate]/1,
[over.match.oper]/3.2; clang's `UnresolvedLookupExpr` callee). Its
instantiation uses that set plus argument-dependent lookup, never a new
unqualified lookup (spec/sema-templates.md §3.5 "Operators"). The set is
`Ast` state, kept for the life of the `Ast`:

- `templates.oper_lookups: kiln_ast::oper_lookup::OperLookups`, insert-
  only: per expression (`ColdMap<ExprId, OperSetId>`, map name
  `oper_lookup`) an interned set of `OperCandidate { decl, found }`
  (`found` is the `UsingShadow` through which lookup found `decl`, else
  `decl`), in lookup order. Sets are interned by content (one `Vec` of
  spans into one `Vec` of candidates, and a content-hash index), so the
  many `os << x` of a header's templates share one set of `operator<<`.
  An empty set is a lookup that found nothing (the instantiation has
  ADL only) and differs from no entry (the expression is not a dependent
  operator expression; `=`, `[]`, `()` and `->` have no non-member
  candidates and record nothing). A dependent expression that a
  substitution rebuilds, still dependent (a generic lambda's body),
  shares the set of its pattern.
- Query `Ast::dependent_oper_candidates(e) -> Option<&[OperCandidate]>`;
  `OperLookups::set_of(e)` gives the interned id (equal ids are equal
  sets), `OperLookups::span(id) -> SetSpan { start, len }` where its
  candidates are in the candidate array (public, WP-2477, for tooling
  clients that keep sets by position).
- Written by sema at the expression (`note_dependent_oper`,
  spec/sema-overload.md §2.6) and by the module-image importer, which
  carries the set with the expression (`COLD_OPER`, spec/modules.md
  §11.11). Snapshots carry the table; the hash index is rebuilt on the
  first intern after a read (§17). Memory rows `templ.oper_lookup`,
  `templ.oper_sets`, `templ.oper_cands`, `templ.oper_index`.

Tests: `crates/kiln_parse/tests/sema_facts.rs`
(`dependent_operator_definition_set`, `fold_operator_definition_set`),
`oper_lookup.rs` unit tests,
`tests/conformance/module.interface/image-oper-lookup-run.cpp`.

### 8.6 Pattern mem-initializers (WP-3026)
A constructor defined in a template (a member of a class template, of a
member class of one, a constructor template, a member template of a
class template) is not checked as a whole at its definition: its
mem-initializers are kept as written and resolved again by every
instantiation ([temp.res.general]/6, [temp.dep]). They are `Ast` state,
one record per such constructor definition, and Sema's instantiation
reads that record and nothing else:

- `templates.pattern_mem_inits: ColdMap<DeclId, PatternMemInits>` (map
  name and memory row `pattern_mem_inits`),
  keyed by the constructor definition. Query
  `Ast::pattern_mem_inits(ctor) -> Option<&PatternMemInits>`; written
  once (`AstBuilder::set_pattern_mem_inits`) at the end of the
  ctor-initializer by Sema and by the module-image importer.
- `PatternMemInits { colon, failed, inits }`: `colon` is `Some` of the
  `:` that begins the ctor-initializer, `None` when the definition has
  none (`S() {}`, WP-3200); `failed` when a mem-initializer failed its
  definition-time check (diagnosed; the instantiations then add no
  implicit initializers, [class.base.init]/9 is not applied after an
  error); the mem-initializers in written order.
- Every such constructor definition has a record, the empty list
  included (WP-3200): `S() {}` has `colon == None` and no
  mem-initializers, so a client tells "no ctor-initializer written" from
  a constructor that is no pattern (no record) on the `Ast` alone. The
  parser tells Sema which it is (`Sema::act_on_mem_initializers` takes
  `Option<Loc>`); `None` is never inferred from an empty list. The
  instantiation of the empty list is unchanged: the implicit
  initializers are added ([class.base.init]/9). The exception is an
  `INVALID` constructor definition: it gets a record only when a
  mem-initializer failed (`failed`, no list), as before. The cost is one
  cold-map entry with an empty list per pattern constructor: for
  `#include <bits/stdc++.h>` (libstdc++, `-std=c++26`) 1531 records, 108
  of them empty (1423 before), and the memory row `pattern_mem_inits`
  at 180 KB live of 43 MB.
- `PatternMemInit { id, loc, end, pack_expansion, init }`: `loc` is the
  first token of the mem-initializer-id (its qualifier included), `end`
  its last token (`)`, `}`, or the `...` of a pack expansion); the written
  range is `loc..=end`.
- `PatternMemInitId`: `Member(d)` for a non-static data member or an
  anonymous union's member (`Field` or `IndirectField`) of the class;
  `Type { ty, ident }` for a base or the class itself (a delegating
  constructor), `ty` as the definition resolved it: `B`, a dependent
  `Base<T>` (`TemplateSpecialization` of the parameter), the injected
  class `S<T>` for `S(..)` or `S<T>(..)`, a pack `Bs` for `Bs(a)...`,
  with `ident` the identifier when each instantiation looks it up again
  ([class.base.init]/2: `B(1)`, `S(0)`) and `None` when the type is
  bound at the definition (a qualified name, a template-id, `decltype`, a
  splice, a template parameter's name such as `T()` or `Bs(a)...`, or a
  typedef found outside a class with dependent bases); `Ident(s)` for an identifier the definition
  could not resolve (diagnosed, `failed` set; the instantiations skip it).
- `PatternMemInitArgs`: `Paren { args, lparen, rparen }` for `( list )`,
  the arguments' `ExprId`s (dependent ones as built at the definition);
  `List(e)` for a braced-init-list, `e` an `InitListExpr` of type `void`
  whose clauses are the written expressions, a nested braced list again
  such an `InitListExpr` and a designated clause `.q = 1` a
  `DesignatedInitExpr` (the same shape as any dependent braced list,
  §5.6). An instantiation substitutes these expressions; it never
  re-parses.

Why this shape: the record is the syntax a tooling client needs (which
entity each mem-initializer names, where, and with which expressions)
and nothing semantic, so it is the same for every instantiation and
survives a snapshot as plain data; a cold map keyed by the constructor
costs nothing for the constructors that are no patterns. The empty
list is a record too because a tooling client must see that the
definition writes no ctor-initializer without reading the source again
(the client request), and an absent record already means "no pattern".
A constructor
that is no pattern (a non-template class's non-template constructor, or
any instantiation) does not get a record: its resolved initializers,
implicit ones included, are `ColdDeclData::ctor_inits` (§2.4,
`Ast::ctor_inits`), as before. A failed constructor whose definition is
`INVALID` keeps no list.

Carriage: PCH snapshots carry the map with the `Ast` (§17; snapshot
format 166, 171 since `colon` is an `Option`). Module images carry the
record with the constructor, the empty one included, `colon: None` as
the location `NONE` (spec/modules.md §11; image version 89, 94 since
WP-3200): a `Member` by its name, looked
up again in the importing class; a braced list decomposed and rebuilt; an
image is refused (the source fallback) for a designated clause and for a
failed record.

The dump (§11) does not print the record. clang's dump of a pattern
constructor shows each `CXXCtorInitializer` with its argument list as a
`ParenListExpr 'NULL TYPE'` (or the written `InitListExpr 'void'`) only
where the member or an argument is dependent, and the initialization as
built at the definition otherwise (`v(1)` an `IntegerLiteral`, `c()` a
`CXXConstructExpr`, `w()` an `ImplicitValueInitExpr`); the record keeps
the written arguments only, so matching clang would need a second,
resolved list per pattern constructor.

Tests: `crates/kiln_parse/tests/pattern_mem_inits.rs` (every kind, a
failed and an unresolved one, the empty list of every kind of pattern
constructor, the record after a prefix snapshot round trip), `crates/kiln_driver/tests/front_end.rs`
(`pattern_mem_inits_through_a_module_image`).

### 8.7 Pattern syntax and the AST (WP-3027)
Sema side tables that hold parsed syntax of a pattern; each is either
reachable from the `Ast` or names the row that puts it there.

- the mem-initializers of a constructor in a template: reachable
  through `Ast::pattern_mem_inits` (§8.6).
- default member initializers of a class template's fields: reachable
  through the pattern field's `FieldData.init`.
- default arguments of a template's functions: reachable through the
  pattern parameter's `ParamData.default_arg`.
- a pattern's `noexcept(e)`: reachable through `ExcSpec::NoexceptExpr` on
  the pattern's function type (`ExcSpec::Deferred` names the pattern).
- requires-clauses: reachable through `TemplateParamList.requires` and
  `templates.trailing_requires`.
- both branches of a constexpr-if in a pattern: reachable through the
  pattern's `IfData`.
- `explicit(e)`: reachable through `ColdDeclData::explicit_expr`.
- dependent attribute arguments: reachable through `AttrNode` operands
  and the dependent type nodes.
- friends of a dependent class (the written name and template arguments,
  the special member and qualifier, the member friend of a dependent
  specialization, a method friend's dependent qualifier): their names as
  written references with WP-3008; the written qualifier, template
  arguments, the friend's own template-heads and trailing
  requires-clause in its `FriendForm` row (§23, WP-3164).
- the template-head of a partial specialization defined after its
  declaration: not yet, row WP-3120.
- the written placeholder of a structured binding declaration and of
  each declarator of a multi-declarator group in a pattern: not yet, row
  WP-3121.
- a template template parameter written without `class`/`typename`: not
  yet, row WP-3122.
- location-only tables (keyword, placeholder, `...` and attribute
  positions of pattern syntax whose tree is on the `Ast`): not yet, row
  WP-3123, only if a tooling client asks.

### 8.8 The using-shadows of an unresolved name (WP-3400)
An `UnresolvedLookupExpr` (§5: a dependent call's callee, a function
template-id, an overload set resolved at the call) holds the
declarations its lookup found, the targets: a using-declaration's
shadow is looked through ([namespace.udecl]/1), so `legacy::forward<T>`
after `namespace legacy { using std::forward; }` lists `std::forward`
itself. The node also keeps, per element, the declaration lookup
actually found when that is a using-shadow (clang's `FoundDecl` in
`UnresolvedLookupExpr::decls()`): a tooling client tells
`legacy::forward<T>` from `acme::forward<T>` (one entity, two shadows)
and from `std::forward<T>` (no shadow). The using-directive route is not
kept here (§18.7, §18.9.1 for the written-reference table's).

- **Storage** (design choice: a parallel cold list, not pairs).
  `templates.unresolved_found: ColdMap<ExprId, ListId>` (map name
  `unresolved_found`, memory row of that name): a list of `DeclId`s
  parallel to the node's declaration list, each the `UsingShadow` (or
  `ConstructorUsingShadow`) through which the element at that index was
  found, else the element itself. An entry exists only when at least one
  element was found through a shadow, so a node without shadows costs
  nothing: its header, its declaration list, their order and targets are
  what they were before WP-3400.
- **Written** by sema where a node is built from a lookup result
  (`lookup::Found::found`, parallel to `decls`): an unqualified or
  namespace-qualified function template name, an overload set
  (`build_overload_ref`), a widened function template-id
  (`widen_function_template_id_dn`), an assumed template-id
  ([temp.names]/3) and an operator-function-id template-id, and a
  dependent call's single callee (from the `DeclRefExpr`'s
  `found_decl`). `Ast::note_unresolved_found(e, found)` keeps an element
  only when it is a shadow of the same entity as the declaration at its
  index; it writes nothing for a node that is not an
  `UnresolvedLookupExpr`, a list of another length, no shadow, or a
  node that already has an entry.
- **Substitution** keeps it: a rebuilt node naming the declarations of
  its pattern (a namespace-scope dependent call, a qualified overload
  set, a generic lambda's body substituted still dependent) takes the
  pattern's shadows, element by element (by entity when the lengths
  differ); a shadow that is a member of a class template's pattern
  becomes the shadow the instantiated using-declaration introduced in
  the enclosing specialization (dropped if none). That map is structural:
  the specialization's shadow may be hidden there by a member of the
  specialization (`Ast::shadow_hider`, §4.2). A rebuilt overload set
  leaves out an element the pattern found through a shadow the
  specialization hides, since lookup in the specialization finds the
  hider, which the set has: in `template <class T> struct D : B { using
  B::m; static int m(T); int call(T t) { return m(t); } };`, `D<int>`'s
  `m(t)` lists `D<int>::m(int)` and `B::m(double)` through `D<int>`'s
  shadow, not `B::m(int)` (WP-3403; before, the call was ambiguous). So
  no found list names a hidden shadow. A node made by a new lookup at
  instantiation records that lookup's shadows.
- **Snapshots and images.** PCH snapshots carry the map (§17). Module
  images carry it per node (`COLD_SHADOWS`, spec/modules.md §11.11); the
  importer records it through `note_unresolved_found`, so the same
  checks apply.
- **Query** (total, for any expression of the `Ast`):
  `Ast::unresolved_shadow(e, i) -> Option<DeclId>`: the shadow of
  element `i` of `Ast::unresolved_decls(e)`. `None` when `e` is not an
  `UnresolvedLookupExpr`; when `i` is not an index of its declarations;
  when the element was not found through a using-declaration: declared
  in the scope searched, made visible by a using-directive, or not found
  by a lookup at all (a set kiln forms without one: a typo correction's
  candidates, a structured binding's `get`, a literal operator's
  templates, a splice, the two-phase recovery retry).
  `Ast::unresolved_found_pairs(e) -> impl Iterator<Item = (DeclId,
  Option<DeclId>)>`: every element with its shadow, in the node's order
  (unqualified: newest first, spec/sema-overload.md §2.1); empty when
  `e` is not an `UnresolvedLookupExpr`.

Tests: `crates/kiln_parse/tests/unresolved_found.rs` (qualified and
unqualified, an inline namespace, two shadows of one entity, a class
using-declaration and its instantiation, a generic lambda substituted,
totality, the PCH snapshot and module image round trips).

## 9. Constant values
### 9.1 `ConstStore` (`kiln_ast::consts`, WP-043)
All results of constant evaluation that the AST retains are **interned**
`ConstValueId`s: equal values (same type, same bits, same relocations,
same padding, same active union members) have the same id, so
template-argument equivalence ([temp.type]) and `ConstantExpr`
comparisons are id equality. The store is append-only; ids are never
invalidated.
```rust
// Logical view (borrowing; the freeze/thaw boundary, not the storage).
enum ConstValue<'a> { Scalar(Scalar<'a>), Aggregate(ByteImage<'a>) }
enum Scalar<'a> {                       // `ty` is the canonical, cv-unqualified type
    Int { ty, signed: bool, bits: IntBits<'a> },  // integral, enum, char; 1..=2^24-1 bits
    Bool { ty, value: bool },
    Float { ty, kind: FloatKind, bits: u128 },    // exact encoding (−0.0 ≠ +0.0, NaN payloads)
    Null { ty, kind: NullKind },                  // Pointer | MemberData | MemberFunction
    Pointer { ty, base: PtrBase, offset: u64,     // also references; byte offset
              designated: Option<QualType>,       // `void *`: the designated object's type
              past: bool,                         // one past the end of a subobject (WP-948)
              unions: UnionPath },                // union members entered off default (WP-2200)
    MemberPtr { ty, member: DeclId, kind: MemberKind, derived: bool, path: &'a [DeclId] },
    Reflection { ty, id: ReflectionId },          // std::meta::info (M6)
}
enum PtrBase { Decl(DeclId), Function(DeclId), Temporary(DeclId),
               StringLit { lit: StringLitId, origin: ExprId },
               Const(ConstValueId), TypeInfo(QualType),
               Label { label: DeclId, origin: ExprId }, Absolute,
               LabelDiff { plus: ExprId, minus: ExprId, size: u8 } }
enum FloatKind { Half, BFloat16, Single, Double, X87Extended, Quad }
struct ByteImage<'a> { ty: QualType, bytes: &'a [u8], relocs: &'a [Reloc],
                       padding: &'a [ByteRange], unions: &'a [UnionMember] }
struct Reloc { at: u32, value: ConstValueId }       // pointer-like subobject
struct ByteRange { start: u32, len: u32 }           // padding run
struct UnionMember { at: u32, field: DeclId }       // active member of the union at `at`
```
- **Types.** Every value carries its canonical, cv-unqualified type; the
  store does not canonicalize. The same bits with different types are
  different values.
- **Integers** are two's-complement `IntBits` (normalized `u64` words,
  bits above the width zero) plus `signed`. Widths up to 64 bits are inline.
  Wider ones (`__int128`, `_BitInt(N)` up to 2^24−1 bits) go to a word pool.
- **Floats** are the exact encoding in their format, zero-extended to
  `u128`. Bits above `kind.bits()` are `Malformed`. X87 is 80 bits with an
  explicit integer bit. PPC double-double is not supported.
- **Pointers** are `(base, byte offset)`, as in the Oven's memory model
  (spec/oven.md). Identity is `(type, base, offset, designated, past,
  unions)`. The designator path is not stored: printing, mangling and
  substitution recover it from the type and offset (§9.3), except for the
  members of unions it enters, which type and offset cannot tell apart
  (`union { int a; int b; } u`: `&u.a` and `&u.b`), below.
  - **Union paths** (WP-2200; [temp.type]/2.4: `&u.a` and `&u.b` are
    different template arguments, and clang's lvalue path mangles them
    `soiL_Z1uE_E` and `soiL_Z1uE_0E`). `unions` is an interned list of
    `FieldDecl`s (`UnionPath`, an id: `NONE` is 0 and the store keeps
    the lists in its `paths` pool, deduplicated, so equal lists have
    equal ids and the value stays `Copy` and is compared by id). Each
    entry is a member of a union the designator path from `base`'s
    object to the designated subobject (of the pointee type, or the
    `designated` type of a `void *`) enters, outermost first, and it is
    listed **only when it is not that union's default member**: the
    first non-bit-field member in declaration order whose bytes hold the
    position and that has a subobject of the pointee type there
    (`consts::union_default_member`, the one rule every reader applies
    where the list names no member of the union). The field also
    identifies its union, and a path crosses a union type at most once
    (no union contains itself), so a reader at a union takes the listed
    member of it, if any, else the default. This is the canonical form:
    a pointer to `u.a` is `NONE` however it was formed, and a pointer
    that crosses no union (the common case) is always `NONE` and costs
    nothing. `consts::canonical_union_path` (given a `PrintEnv`, the
    base object's type, the offset, the pointee and the members the
    pointer's designator names) computes it; the Oven calls it when it
    freezes a pointer whose register carries a union member designator
    (spec/oven.md "Union member designators"). A pointer without a
    pointee type (a `void *` with no `designated`) and a past-the-end
    pointer keep `NONE`.
  - `Function` bases carry offset 0 except in a C address constant
    (`(char *)f + 1`, C23 6.6p9, WP-1304).
  - A `LabelDiff` base is the difference of two label addresses as an
    integer, `&&plus - &&minus` plus `offset` (GNU C, clang's
    `AddrLabelDiff`; WP-1304): only a `Pointer` of integer type carries
    it, only a C static initializer's fold makes one, and its relocation
    is `size` bytes wide (1, 2, 4 or 8; code 8 + log2 `size`), the only
    relocation narrower than a pointer. Images (PCH, modules) refuse it,
    as `Absolute`.
  - A `Const(id)` base must already be in the store. It names a subobject
    of another frozen value (an object with static storage and no
    declaration of its own).
  - A `StringLit` base is the array object created by evaluating the
    `StringLiteral` expression `origin`; `lit` is its contents. Two
    pointers into equal literals from different expressions are different
    values, and comparing them is unspecified ([expr.const]/5.24). Neither
    id is validated (against the `StringPool` or the expression arena).
- **Member pointers** store the member's `DeclId` plus clang's
  member-pointer path and the derived flag. They do not store an offset,
  because union members share offsets and [temp.type] compares member
  identity.
- **Aggregates** (class, array, union, `_Complex`, vector) are a **byte
  image** in target layout. A constant expression's result is fully
  initialized ([expr.const]), so no init mask is stored. Images are
  little-endian and target-specific (one target per compilation).
  `push_aggregate` requires this canonical form and otherwise returns
  `ConstError::Malformed`:
  - **`bytes`:** the complete object, `sizeof(T)` bytes.
  - **`relocs`:** one entry for **every** pointer-like subobject, null
    included. Entries are sorted by `at`, non-overlapping and in bounds, and
    the image bytes they cover are zero. The value must be pointer-like. Its
    width is `reloc_width`:
    - pointer, null pointer, data-member pointer: one pointer;
    - member-function pointer, null or not: two pointers (Itanium);
    - reflection: 8 bytes.
  - **`padding`:** every byte that is not part of a value. That includes
    alignment and tail padding, the unused bytes of `long double`, and union
    bytes outside the active member (all of them when no member is active).
    Runs are sorted, non-empty, coalesced (never adjacent), in bounds,
    disjoint from relocations, and their bytes are zero. This is the
    "shadow summary".
  - **`unions`:** one entry per union subobject that has an active member,
    sorted by `(at, field)`. The `FieldDecl` also identifies the union, so
    nested unions at the same offset are unambiguous.
- **Freeze and thaw (the Oven's use).**
  - *Freeze* bottom-up:
    1. Intern each pointer-like subobject with `push_scalar`.
    2. Build the image, with zero bytes under relocations and padding.
    3. Call `push_aggregate`.
  - *Thaw*:
    1. `memcpy` `bytes(id)` into a new allocation.
    2. Walk `shadow(id)`. It yields maximal `Value`/`Padding` runs, plus
       one `Pointer(ConstValueId)` run per relocation, in order, covering
       the image exactly. Mark `Value` bytes initialized and `Padding`
       bytes uninitialized, and write the provenance of each `Pointer`.
    3. Set the active members from `unions(id)`.
  - No layout is re-derived. The store contains no Oven logic.
- **API.** `with_pointer_bytes(u8)`; `Default` is 8.
  - `Ast::new` uses `ConstStore::with_pointer_bytes` with
    `TargetTypes.pointer_bytes`.
  - Interning: `push_scalar`, `push_aggregate` and `push` return
    `Result<ConstValueId, ConstError>`. `lookup` finds an existing value
    without inserting.
  - Reading:
    - `get` returns the whole `ConstValue`; `ty` and `kind` return parts of
      it.
    - `scalar` and `aggregate` return `Option`.
    - `bytes`, `relocs`, `padding` and `unions` return the image tables
      (empty for scalars). `shadow` iterates the runs above.
    - `reloc_width`; `len` and `ids`.
    - `push_union_path(&[DeclId]) -> Result<UnionPath, ConstError>`
      interns a union path (the empty list is `UnionPath::NONE`);
      `union_path(UnionPath) -> &[DeclId]` reads one.
  - `ConstError` is either `Capacity` (an id space or a 4 GiB pool is full;
    a fatal diagnostic) or `Malformed` (a canonical-form violation; an
    internal error in the caller).
- **Storage.** Each value has a 16-byte node (`NODE_BYTES`) holding the
  type, a tag word with the kind, flags and a 24-bit extra field, and two
  payload words. Per-value costs:

  | value | bytes |
  |---|---|
  | int ≤ 64 bits, bool, float ≤ 64 bits, null, reflection | 16 |
  | wider int | 16 + 8 per word |
  | x87 or quad float | 16 + 16 |
  | pointer | 16 (+8 if the offset is ≥ 2^32; +16 for a designated type; +16 for a union path, whose list costs 8 + 4 per entry once per distinct list) |
  | member pointer | 16 + 4 per path entry |
  | aggregate | 16 + 24 (pool starts and cached 64-bit hash) + `sizeof(T)` + 8 per relocation, padding run and union entry |

  - Every value also costs about 5 bytes of intern table. The table is a
    hashbrown `HashTable<ConstValueId>` keyed by FxHash. It stores ids
    only; keys are re-read from the pools.
  - An aggregate's hash is cached, so growing the table never rehashes
    images.
  - Pools are `u32`-indexed: 4 GiB of image bytes in total.
  - A pointer with a union path sets bit 4 of the node's extra field
    (its base code is bits 0-3) and keeps its offset in the word pool,
    followed by the base's second id, the designated type and the
    `UnionPath`, each when present. Union path `i + 1` is the span
    `upaths[i]` of the `paths` pool; an index rebuilt from the pool
    (also after a snapshot restore) deduplicates them.
  - `HeapSize` reports these rows: `nodes`, `words`, `paths`, `aggs`,
    `bytes`, `relocs`, `padding`, `unions`, `upaths`, `intern`.
  - `content_bytes() -> u64` is the interned content: the length of every
    pool times its element size (the rows above except `intern`, without
    spare capacity). It is deterministic on every 64-bit host and does not
    change when a value already present is pushed again, so its difference
    across a freeze is what the freeze added. The Oven reports that
    difference as `frozen_bytes` (spec/oven.md, R-OVEN-2): freezing an
    aggregate equal to one already frozen yields the same `ConstValueId`
    and adds 0 bytes (`crates/kiln_ast/tests/consts.rs`,
    `crates/kiln_oven/tests/nested.rs`).
- **`StringPool`** (`kiln_ast::strings`, also WP-043).
  - `intern(StringEncoding, code_units: &[u8]) -> StringLitId` dedups by
    `(encoding, bytes)`. The code units are little-endian, `unit_bytes()`
    each, and exclude the terminating null. `try_intern` reports errors
    instead.
  - `get(id) -> (StringEncoding, &[u8])`, `unit(id, i)`, `unit_count`,
    `object_bytes` (`(units + 1) × unit size`), `write_literal`.
  - `StringEncoding` is `Ordinary`, `Utf8`, `Utf16`, `Utf32`, `Wide16` or
    `Wide32`.
  - Dedup is **storage only**. Equal ids do not make two literal
    evaluations the same object ([lex.string]), so the Oven must not fold
    `"a" == "a"` from ids.
- **Deviations from the original sketch, and open questions**
  - **Node size and pools.** A node is 16 bytes, not 12. Values have no
    `BlobId` or `ListId`: images, words and paths live in the store's own
    pools. `BlobId` is unused.
  - **Pointers.** Bases gained `Function` and `Const`; `Decl` is
    variables and template parameter objects only. Pointers store no
    `path` but the non-default union members on it (`unions`, WP-2200). The offset is unsigned: a pointer before its base is never a
    constant.
  - **Nulls and member pointers.** `NullPtr` became `Null { kind }` so that
    the image width is known. Member pointers carry `kind`.
  - **Shadow summary.** Only padding is stored. `Value` and `Pointer` runs
    are derived.
  - **Pointer identity.** A `void *` keeps the type of the object it
    designates (`designated`, set by the Oven's freeze from its void tag;
    `None` for every other pointer), and it is part of the value's
    identity: `(void*)&s` and `(void*)&s.x` (same type, base and offset)
    are different values, as [temp.type]/2.4 and clang's lvalue path keep
    them apart, and mangle differently (`cv <type> ad ...`). `past`
    (WP-948) marks a pointer one past the end of a subobject that does
    not end its complete object (`&s.a + 1`, `arr[0] + 2` in a 2x2
    array): its offset is that of the next member or element, which it
    must not reach through ([expr.add]/4, footnote; clang's lvalue path
    keeps the subobject). It is canonically `false` at or past the end of
    the complete object and for every pointer into it, so it is set only
    when it changes what the pointer may access (store flag `F_PAST`). Together with
    the offset it names the subobject. A template argument pointing to
    the first element of an array variable is re-keyed to the array
    (`Sema::undecayed_void_ptr_arg`): clang ignores that decay.
    Pointers of different types (`&arr` vs `&arr[0]`) were already
    distinct.
  - **Bit-fields.** The store keeps no bit-level padding inside a
    bit-field storage unit (unnamed bit-fields, unused high bits): those
    bits are stored as zero. No constant evaluation reads them: the Oven
    bit-casts a bit-field object member by member and skips unnamed
    bit-fields (`kiln_oven/src/vm/bitcast.rs`), so they stay
    indeterminate, as [bit.cast]/2 says
    (`bit.cast/constexpr-bit-field-padding-of-constant.cpp`).
  - **Byte order.** Every kiln target (aarch64 and x86-64 Linux) is
    little-endian; the image is laid out for that.

### 9.2 Where values are cached
| cache | content |
|---|---|
| `ConstantExpr.b` | value of a constant expression (§5.5) |
| `VarData.eval` | `NotEvaluated → Evaluating → Value(id) / NotConstant`; `Evaluating` detects self-reference; used for `constexpr`/`constinit` vars, `const` integral vars usable in constant expressions, and static-storage constant initialization (codegen emits the value); when it is written: below |
| `EnumConstant` payload | value (`Int`) |
| `TemplateArg::Value` | the converted non-type argument (spec/types.md §8.3) |
| `TemplateParamObject` | the object's value |
| `IfStmt` | `if constexpr` via its `ConstantExpr`; `if consteval` needs no value |
| `Annotation` attr | annotation value (M6) |

**When `VarData.eval` is written** (WP-2504). Every constant
initializer sema checks is evaluated when it is checked, and the
result is written then, once: `Value(id)` or `NotConstant`. The checks
are a `constexpr` (or `constexpr` structured binding's) initializer
(`check_constexpr_init`), an in-class initializer of a non-inline
`const` integral or enumeration static data member
(`check_in_class_static_init`: `struct S { static const int x = 1; };`),
and a `const` non-volatile integral or enumeration variable with static
storage (`fix_const_int_init`: a namespace-scope one, an out-of-line
static data member definition, `const int S::y = 2;`). The check runs
where the initializer is: at the declaration, or, for a member of a
class template specialization or a variable template specialization,
when the initializer is instantiated (at its first use, at the end of
the translation unit, or by an evaluation session's on-demand read). So
the two forms of a `const` static data member agree: each has its value
from its check (before WP-2504 the in-class form was evaluated by its
check but the value dropped). A dependent initializer (a template's
pattern) is not evaluated and stays `NotEvaluated`. A later use never
writes `eval` (except, with `-fkiln-static-init-facts`, the end-of-unit
pass of §25.2): an Oven that reads a variable still `NotEvaluated` (one
no check covers: a non-`const` one read by a lax fold, an initializer
not yet instantiated) evaluates its initializer into its own global
object, which that Oven keeps, and leaves `eval` as it is. A module or
precompiled header image carries `eval` as written
(`crates/kiln_parse/tests/eval_session_results.rs`
`evaluated_values_are_written_when_checked`).

### 9.3 Printing values
`ConstStore::print(id, &StringPool, &dyn PrintEnv, &PrintOptions, &mut
String)` (or `display(..) -> String`). The store knows bytes, not types,
so it reads type information through `PrintEnv`, whose methods all have
context-free defaults (`NoEnv`):
- `shape(ty) -> Shape` gives the layout facts the printer needs:
  - `Int { width, signed, style }` (value bits; `_BitInt(N)` reads N bits
    and its remaining object bits are padding), `Bool`, `Float(kind)`;
  - `Pointer { pointee, reference }`, `MemberPointer`, `Reflection`,
    `NullPtr`;
  - `Array { elem, stride, len }`;
  - `Record { members, union }`, with each `Member` carrying name, type,
    bit offset, size, bit-field width and `FieldDecl`, bases first;
  - `Unknown`.
- Names: `decl_name` (default `#d17`), `decl_type`, `type_name` (default
  `#t5`), `enumerator`, `reflection` (default `^^#r3`).
- A pointer's designator (`&u.b`, `&v.t.x`) enters each union by the
  member its union path names, else by the union's default member
  (§9.1 "Union paths"); other records are walked as before, each member
  holding the byte tried in order until one holds the pointee.

`PrintOptions { style, max_depth: 10, max_elems: 32, include_type: true,
enum_cast: false }` are the defaults (`PrintOptions::DEFAULT`).

`ConstStore::print_exact(..) -> bool` prints the same text and answers
whether it is exact: false when some part printed as a placeholder (an
`Unknown` shape's `{...}` or `?`, an undecodable subobject's `?`, a
`max_depth` `{...}` or `max_elems` `...` truncation, a `Const` base's
`#c12`, a `Temporary` or label base, an offset no designator recovers
(`+off`), an infinity or NaN). `display_string_of` of a value uses it
(spec/reflection.md §3.9, WP-564).

**Dump** (`PrintStyle::Dump`, the `value=` fields):
- **Integers:** decimal at any width (`-5`, `18446744073709551615`).
  Characters and enums are numbers too.
- **`bool`:** `true` or `false`.
- **Floats:** the shortest round-trip form in Rust `{:e}` notation, for
  every `FloatKind`: `1.5e0`, `1e-1`, `-0e0`, `inf`, `-inf`, `nan`.
  A `FloatingLiteral` in a printed expression (a `static_assert`
  requirement, a template argument) is clang's `PrintFloatingLiteral`
  instead: `APFloat::toString` with round-trip precision
  (`consts::fltstr::to_string(kind, bits, 0)`: `1`, `0.10000000000000001`,
  `2.5E+10`), a `.` after a bare integer, then the suffix of its type
  (`F` `float`, `L` `long double`, `F16` `_Float16`, `Q` `__float128`;
  none for `double` and `__fp16`): `1.`, `2.F`, `2.99999992E-5F` (WP-410).
- **Null values of any kind:** `nullptr`.
- **Pointers** print as `&name` plus a designator:
  - The designator is recovered by walking the base's type
    (`decl_type`, or the `Const` value's type) through array elements
    `[i]` (one-past-the-end included) and fields `.f`. Bases add no
    designator.
  - The walk stops when the offset is 0 and the type equals the pointee:
    `&arr` vs `&arr[0]`, `&s.in.y`, `&arr[3]`. The types are compared
    ignoring top-level cv, including the cv of array elements
    (`PrintEnv::same_unqualified`, clang `hasSameUnqualifiedType`), so a
    `const int (*)[2][2]` to the `int[2][2]` member of a const object
    prints `&fake.c`, not `&fake.c[0][0]` (WP-792).
  - If no designator can be recovered, the pointer prints as `&name+off`.
  - String literals print as `"hi"` at offset 0, otherwise `&"hi"[1]`
    (index in code units, with the `u8`/`u`/`U`/`L` prefix).
  - Other bases: `&f` (function), `&#c12.a` (a `Const` base),
    `&typeid(T)`.
  - References print like pointers.
- **Member pointers:** `&` followed by `decl_name(member)`.
- **Reflections:** `env.reflection`.
- **Aggregates:** `{1, 2, {3}}`.
  - Bases are nested braces. Unnamed bit-fields are skipped, and named
    bit-fields are decoded at bit offsets.
  - A union with an active member prints `{.x = 1}`; with none, `{}`.
  - An aggregate deeper than `max_depth` prints as `{...}`. After
    `max_elems` elements the list ends with `, ...`.
  - An aggregate of `Unknown` shape prints as `{...}`, and an undecodable
    subobject as `?`.

**Template arguments** (`PrintStyle::TemplateArg`), after clang's
`TemplateArgument::print`:
- **Integers** follow the type's `IntStyle`:
  - `Plain { suffix }` gives `5U`, `5L`, `5UL`, `5LL`, `5ULL` (no suffix
    for `int`).
  - `Cast` gives `(short)5`.
  - `Char(enc)` gives `'a'` or `u8'a'`, and `SmallChar` (`signed char`,
    `unsigned char`) gives `(signed char)'a'` when the type is included,
    spelled as clang's `CharacterLiteral::print` (WP-437): the simple
    escapes `\\ \' \a \b \f \n \r \t \v`, printable ASCII verbatim,
    otherwise `\xhh` below 256 (`'\x00'`, `'\xc8'`), `\uhhhh` up to
    `0xffff` and `\Uhhhhhhhh` above, lowercase and zero-padded (an
    ordinary `char` is masked to its byte). The expression printer
    (`CharacterLiteral` in a static-assertion requirement or a dependent
    argument) uses the same spelling; string literals keep §2's.
  - `Enum` gives the enumerator, fully qualified by its scope (clang
    `printQualifiedName`; an unscoped enumeration adds no component:
    `n::E::X`, `n::ub`), or `(n::E)7` when no enumerator matches. A
    cast prints the canonical type with every name scoped
    (`PrintEnv::arg_type_name`; `A<(n::E)5>` for `(n::EE)5`). Sema's
    `TypeNames::write_value` uses this style for every integer, so
    diagnostics print `S<n::E::b>` and `Ch<'x'>`, not `S<1>` and
    `Ch<120>` (WP-303). An enumeration whose underlying type is `bool`
    has the `Enum` style too (a one-byte unsigned integer shape, not
    `Bool`), and its enumerators' `bool` values match: `X<B::Nay>`,
    `[V = B::Nay]` in `__PRETTY_FUNCTION__`, not `1` (WP-683).
  - With `enum_cast` (and no `include_type`), an enumeration value no
    enumerator matches still prints as its cast (`(n::E)7`, not `7`), so
    the spelling re-parses as the enumeration (`display_string_of`).
- **References** print without `&` (`x`).
- **Class values** print as `S{1, 2}`: only the value's own class is
  named, a member or base subobject is a braced list as in the dump
  (`Out{{2}, 1}`, `D{{1}, 2}`; clang's `APValue::printPretty`). The GCC
  form (`gcc`) names every class (`Out{In{2}, 1}`).
- **Floats** print as clang's `APValue::printPretty` (`GetApproxValue`):
  the value rounded to `double` (to nearest, ties to even) in C's `%e`
  form, six digits after the point and an exponent of at least two
  digits (`1.000000e+00`, `-0.000000e+00`, `1.234568e+08`), an infinity
  `INF` / `-INF`, any NaN `nan` (`consts::format_float_clang`; WP-1414).
  `gcc_float` keeps GCC's form (spec/reflection.md §3.9).
- Everything else prints as in the dump.
- The type (suffix or cast) shows only where clang's `IncludeType` asks
  for it: in a type's argument list, an argument of a template parameter
  that is a non-type parameter with a placeholder type (`auto`, a class
  template placeholder; `Ast::nttp_has_deduced_type`, clang
  `shouldIncludeTypeForArgument`): `D<5UL>` for `template <auto N>`, `A<5>`
  for `template <unsigned long N>`. The type printer learns the template
  from the specialization or template-id (`write_arg_list_of`,
  `TypeNames::arg_includes_type` / `write_value_typed`). An argument kept
  as an expression prints as written, with its integer-literal suffix
  (`D<7ul>` written prints `D<7UL>`; WP-292). A function template
  specialization named in a diagnostic (`'f<2U>' has been explicitly
  marked deleted here`, `'d<9ULL>' is deprecated`) prints every one of its
  own arguments typed: clang's `FunctionDecl::getNameForDiagnostic` passes
  no parameter list, so `shouldIncludeTypeForArgument` is true
  (`SemaNames::typed_args_of`; a nested template-id keeps the rule above;
  test `temp.param/function-spec-name-typed-args.cpp`, WP-366). The same
  holds for the "in instantiation of function template specialization"
  note and the explicit-instantiation diagnostics (`'f<2UL>'`,
  `'C<4>::m<5U>'`; `Sema::fn_spec_decl_name`); a variable template
  specialization prints them bare (`'v<2>'`; test
  `temp.param/instantiation-note-typed-args.cpp`, WP-1844).

**String escapes** (`StringPool::write_literal`):
- `\\ \" \n \t \r \0 \a \b \f \v` print as escapes.
- Printable ASCII prints verbatim.
- Every other code unit prints as `\x{hh}` (lowercase hex).

## 10. Reflection and injection (M6)
Specified in **`spec/reflection.md`**: `ReflectionTable` and the
`Reflectee` variants (§2.2), identity and alias sugar (§2.3), and
`define_aggregate`'s injected `Field`s, which are the one exception to
§2.4's "frozen at `}`" (§6.2). Splice nodes are covered in §5.
Each variant's key, the `^^` operand mapping, template arguments,
printing and the consteval-only rules as implemented are in
**`spec/reflection-repr.md`**.

## 11. The AST dump format
Specified in **`spec/ast-dump.md`** (split out to keep both files
readable): the text format, driver options, the normative M2 node lines
with a clang-verified example, the template/lambda sketch, the clang
normalizer for differential tests and the harness `expect: ast` outcome.
The dump prints this AST as is: one line per node, children in the order
of §4–§6, every type, value category, cast kind and cached value visible.

## 12. Agent parallelization
The AST is the hub; the plan front-loads the pieces that let everything
downstream start *without the parser*. Ids (§2.1) and the `Ast` field
skeleton land first as a ~1-hour stub commit; after that the WPs below
proceed in parallel. Each WP's acceptance is a set of tests named here.

| WP | scope | interface consumed → produced | acceptance |
|---|---|---|---|
| A ast-core | ids, arenas, `IdLists`, `DeclName`, decl/expr/stmt headers + M2 payloads, lookup tables, redecl table, `MemReport`, **`AstBuilder`** (typed constructors computing dependence, used by sema, tests and the loader) | §2–§7 → `Ast`, `AstBuilder` | size asserts; builder unit tests; lookup/redecl tests |
| B types | `TypeContext`, interning, canonical types, M2 kinds, `TypePrinter` | spec/types.md | types.md §14 |
| C const-store | `ConstStore`, interning, value printing | §9 → `ConstValueId` | round-trip + printing tests |
| D dump | dump printer, options, filter | ast-dump.md, A+B+C → text | ast-dump.md §4 golden via `AstBuilder` |
| E dump-loader | text → `Ast` for the M2 subset (inverse of D; types re-parsed by a small declarator parser) | ast-dump.md → `Ast` | `load(dump(ast))` round-trips on all goldens |
| F clang-normalizer | `xtask clang-ast` | clang JSON → ast-dump.md text | M2 sample matches ast-dump.md §4 byte for byte |
| G harness | `expect: ast` outcome | ast-dump.md §7 | harness self-tests |
| H parser-M2 | declarations, declarators, expressions, statements → sema actions | lexer/pp → `AstBuilder` | M2 conformance tests (dump goldens) |
| I sema-conv | implicit conversion sequences → `ImplicitCastExpr` chains, usual arithmetic conversions, integral promotions, literal typing ([lex.icon] table) | `AstBuilder`, types | dump goldens per [conv]/[expr.arith.conv] paragraph |
| J abi-layout | `TargetTypes`, `record_layout` | spec/types.md §11 | layouts vs clang `-fdump-record-layouts` |
| K oven-compile | AST → KBC compiler for the M2 subset | `Ast` (from E's loader at first), `SemaBridge` (§13 Q2) | spec/oven.md tests, fed by loaded dumps |
| L codegen-M2 | AST → LLVM IR | `Ast` from E | IR/run tests on loaded dumps |

E is the multiplier: once it exists, K and L test on hand-written or
clang-normalized dumps (F) while H and I are still being built, and a
parser bug cannot block Oven or codegen progress. B and C are leaf work
with no dependency on each other. Conflict surface: A owns
`crates/kiln_ast/src/{ids,decl,expr,stmt}.rs`, B owns `types/`, C owns
`consts.rs`, D/E own `dump/`; `lib.rs` re-exports are the only shared
file.

## 13. Open questions (each with a recommendation)
1. **Type-layer questions** — `hashbrown` (types.md T1), kiln_ast →
   kiln_lex for `Symbol` (T2), `SubstTemplateTypeParm` sugar (T3),
   kiln_sema → kiln_abi (T4), TypeLoc deferral (T5), `InjectedClassName`
   canonical form (T6). Recommendations are in spec/types.md §16.
2. **Oven / sema borrow conflict.** spec/oven.md has `Oven::new(ast: &Ast,
   bridge: &mut dyn SemaBridge)`, but the bridge (sema) must *mutate* the
   Ast while the Oven runs (instantiating a body on `function_body`,
   `define_aggregate`, interning `ConstValue`s), which Rust rejects while
   `&Ast` is held. *Recommend* amending oven.md: the Oven holds no `&Ast`;
   `SemaBridge` gains `fn ast(&self) -> &Ast`, `fn ensure_body(&mut self,
   f: DeclId) -> Option<StmtId>`, `fn ensure_layout(&mut self, r: DeclId)
   -> &RecordLayout`, `fn consts(&mut self) -> &mut ConstStore`; the Oven
   re-borrows `bridge.ast()` per compile step and keeps only ids (valid
   forever by §2.4). Bonus: `Oven` loses its lifetime and can live for the
   whole TU, keeping its KBC and fold caches across evaluations.
3. **Dump uses clang's class names and glyphs.** *Recommend* yes: the
   clang oracle (ast-dump.md §6) and contributors' clang familiarity
   outweigh the freedom to rename; deviations are listed in ast-dump.md §4.
4. **Dump loader (WP E).** *Recommend* yes, M2 subset only, growing on
   demand; it is test infrastructure, not a product feature.
5. **Harness `expect: ast`.** *Recommend* yes (ast-dump.md §7).
6. **Instantiated bodies retained** (append-only arenas, §2.4). A heavy
   constexpr/template TU keeps every instantiation. *Recommend* accept
   for M2–M6 and measure in M7; the escape hatch is a per-function
   "body arena" that can be dropped after codegen, which the id design
   allows without changing node types.
7. **Semantic-only `InitListExpr`** (§5.6). *Recommend* yes. Designator
   diagnostics are emitted while the list is checked; template patterns
   keep the written form (§5.6).
8. **Single-threaded sema per TU.** *Recommend* yes; parallelism is across
   TUs and inside the Oven's own budgeted evaluation, not inside sema.
9. **TU-local ids and modules.** *Recommend* defer: M4 imports re-intern
   through constructors (types.md §10); id-range remapping only if the
   M7 profile demands it.
10. **`static_assert` message type.** clang 19 prints its type as `NULL
    TYPE`; kiln marks it `unevaluated`. *Recommend* keep the deviation
    (normalizer rewrites clang's form).

## 14. Tests (`crates/kiln_ast/tests/`)
- `layout.rs`: `size_of::<ExprNode>() == 20`, `StmtNode == 16`,
  `DeclNode == 28`, `FunctionData == 24`, `VarData == 24`, `AttrNode == 16`,
  `Option<ExprId> == 4`, and the types.md list.
- `builder.rs`: dependence propagation per expression kind; value
  categories of the M2 kinds; redeclaration groups; lookup for overload
  sets, transparent contexts and inline namespaces; P2169 `_` multiple
  declarations.
- `consts.rs`: interning identity (`-0.0` vs `+0.0`, union active member,
  pointer relocations), printing per §9.3.
- `dump/`: the ast-dump.md §4 example (built by `AstBuilder`) equals the
  golden; each M2 line form of ast-dump.md §3 has a focused golden; with
  `ids,locs`; filter mode; no trailing whitespace anywhere.
- `loader.rs` (WP E): `dump(load(text)) == text` for every golden.

## 15. Supervisor decisions (2026-10-28, binding)
Q1 see types.md §17. Q2 **approved**: spec/oven.md is amended (§"Interface")
so the Oven holds no `&Ast`; it lives for the whole TU and reaches the AST
through `SemaBridge::ast()`/`ensure_body`/`ensure_layout`/`consts()`.
Q3 yes (clang class names/glyphs). Q4 yes (dump loader, M2 subset, test
infrastructure). Q5 yes (`expect: ast`). Q6 accepted; `Ast::memory()` must
report instantiation-retained bytes as its own row from day one so M7 can
see it. Q7 yes. Q8 yes. Q9 defer. Q10 keep.
Work packages A–L (§12) are the M2 plan; ids in docs/BACKLOG.md are
WP-041..052 in that order.

## 16. Implementation refinements (WP-041, normative)
These refine or replace the text above where they differ; each says why.
1. **`Symbol`/`Interner` live in `kiln_source::symbol`** (and `FxHasher` in
   `kiln_source::fx`), not `kiln_lex`: `kiln_ast` must name symbols without
   depending on the lexer. `Ast::new(symbols: Arc<Interner>, types:
   TypeContext)`; `Ast::symbols()` borrows the interner.
2. **`DeclName` identifier payload = `Symbol` index + 1** (29 bits), so the
   all-zero `DeclName` is `DeclName::EMPTY` (§3). Non-identifier names
   (operators, conversion functions, constructors, destructors, deduction
   guides, literal operators) intern their extra data in **`Ast.names:
   NameTable`**, a field added to §2.2.
3. **Id exhaustion panics** (`"<Id> id space exhausted"`): 2^32 nodes is
   far beyond any memory budget, and the oven's budget (§9, oven.md) stops
   runaway evaluation long before; a diagnostic path would cost every push.
   Index 0 is a dummy in every arena so `Option<Id>` is 4 bytes.
4. **`ListId::EMPTY` = index 1** is the shared empty list; payloads default
   to it instead of `Option<ListId>`.
5. **`lookup_local(ctx, name) -> LookupResult`** (`None | One(DeclId) |
   Many(&[u32])`, with `single`, `len`, `to_vec`, `iter_with_inline`)
   instead of `&[DeclId]`: the single-decl case (≈95%) stores no set.
   Overload sets are `Vec<Vec<u32>>` (no `smallvec`, per the no-new-deps
   rule); the high bit of each raw entry is `via_inline`.
6. **Lookup placement.** `ctx` is first mapped through `redecl_context`
   (skips `LinkageSpec`/`Export`) and `primary_context` (namespace → first
   declaration; class/enum → its definition, else itself). Only TU,
   Namespace, CXXRecord, ClassTemplate(Partial)Specialization and Enum own
   tables; a decl in any other context (function, block) gets no table
   entry and is not appended to a member list. Unscoped enumerators are
   also entered in the enclosing context; inline-namespace members are
   entered in each enclosing namespace up the inline chain with
   `via_inline`; `add_inline_child` is called on the first inline
   declaration. A redeclaration replaces its predecessor in place (order
   preserved); a P2169 `_` declaration (`DeclMisc::name_independent`) is
   always appended. Friend declarations are entered; sema filters by
   `misc.friend`.
7. **Begin locations** of declarations whose begin ≠ `loc` and the
   **end locations** of `CallExpr`/`UnaryExprOrTypeTraitExpr` (`rparen`)
   live in cold `ColdMap`s (`DeclArena::begin_loc`, `ExprArena` cold
   `end_loc`): they are rarely read and would grow the hot header.
8. **`IndirectField`**: its payload is the `ListId` chain of fields (from
   the anonymous member to the named field), read with
   `Ast::indirect_field_chain`; the decl is `IMPLICIT`.
9. **Base specifiers** are stored in `DeclArena::base_specs` (a vector of
   `BaseSpec`); `RecordData::bases` lists 1-based indices into it.
10. **`ParamData`** = `{default_arg, original_ty, index: u16, flags:
    u16}`. Parameters are created after their function (their `ctx` is the
    function), so `FunctionData::params` is attached once afterwards with
    `AstBuilder::set_params` (write-once while the list is `EMPTY`).
    `set_params` also records the function's minimum argument count
    ([dcl.fct.default]; clang's `getMinRequiredArguments`: the counted
    parameters up to the last without a default argument or a
    `DEFAULT_ARG_PENDING` flag, a pack never counted) in a side vector
    parallel to the function payloads, one byte each (`DeclArena::min_args`;
    WP-663). The byte reads "unknown" (`None`) for more than 254 counted
    parameters and once a parameter of the set list gains a default
    (`set_default_arg`/`mark_default_arg_pending` on a parameter that had
    none), until `Ast::refresh_min_args` re-records it; a caller counts
    then. Whether the byte is known never changes a result. The side
    vector is a memory-report row (`decls.min_args`) but is left out of
    the node meter (`Ast::node_live_bytes`), so the instantiation report's
    bytes do not change with it.
    The high byte of `flags` (`ParamData::SCOPE_DEPTH_MASK`) is the
    parameter's function-scope depth, the number of prototype scopes
    enclosing its own (clang's `getFunctionScopeDepth`; `scope_depth()`,
    `scope_depth_flags(depth)`; WP-305, used by the `fL` mangling and the
    canonical profile of a parameter reference, spec/abi.md §8.15).
11. **`DeclRefExpr`/`MemberExpr` `aux` = `NonOdrUse`** ([basic.def.odr]/5
    classification, filled by sema).
12. **String literals:** `AstBuilder::string_literal` takes an already
    interned `StringLitId` (`StringPool::intern(encoding, code_units)`,
    owned by WP-043), keeping the builder free of encoding logic.
13. **Enums:** `AstBuilder::enumeration(h, flags, fixed: Option<(underlying,
    promotion)>, prev)` sets the types at declaration for a fixed
    underlying type; otherwise `complete_enum_definition(e, computed)`
    supplies them (and errors if they were fixed already).
14. **Instantiation meter (§15 Q6):** `begin_instantiation` /
    `end_instantiation` nest; only the outermost pair adds the growth of
    node bytes (decls, exprs, stmts, attrs, lists, lookup, redecls) to the
    total. `memory()` always has the row `instantiation.retained` (count =
    instantiations, live = reserved = retained bytes); it overlaps the
    arena rows, so `MemReport::live_bytes` excludes rows starting with
    `total`/`instantiation`. It also has the rows `journal.lookup`,
    `journal.decl_types` and `journal.redecls` (WP-661): the redeclaration
    index's change journals (spec/sema.md), which the node meter does not
    count.
15. **Potentially-constant variables** ([expr.const]/3) are approximated in
    dependence propagation as "const-qualified or `constexpr`" (no
    integral/enumeration type check): over-approximating value dependence
    is safe (it only defers evaluation to instantiation).

**Landed (WP-140): GNU asm nodes.** `GCCAsmStmt`'s side record is
`AsmData` (`StmtArena::asm_data`): template literal and location, a slice
of the operand table (`asm_operands(s)`: `AsmOperand { name, constraint,
constraint_loc, expr }`, outputs then inputs), clobbers and labels
(`ListId`s), the closing `)`, and `srclocs` (a raw `u32` list of clang's
`!srcloc` values, filled by sema); `aux` holds `asm_bits`
(`VOLATILE`, `GOTO`, `INLINE`, `BASIC`). Built with
`AstBuilder::gcc_asm_stmt(data, ops, bits, loc)`. A namespace-scope
asm-declaration is `DeclKind::FileScopeAsm` (`AstBuilder::file_scope_asm`,
`Ast::file_scope_asm_string`). For `asm goto` (`asm_bits::GOTO`), `labels`
holds the `LabelDecl`s named in the label operand list (each recorded as a
`RefRole::Label` written reference, §18.4).

**Landed (WP-140): `PredefinedExpr`.** `__func__`, `__FUNCTION__`,
`__PRETTY_FUNCTION__` ([dcl.fct.def.general]/8; clang `PredefinedExpr`):
`aux` is the `PredefinedKind` (`Func`, `Function`, `PrettyFunction`;
`Exprs::predefined_kind`), `a` the `StringLiteral` holding the name
(`Exprs::predefined_name`, its only child). An lvalue of `const char[N]`;
in a dependent context it has type `QualType::DEPENDENT` and no child.
Built by `AstBuilder::predefined_expr(kind, ty, name, func, loc)`.

**The function it names (WP-2823).** `b` is the function the name is
computed from, `Option<DeclId>` (`ExprArena::predefined_function`, role
`Opt(Decl)` in §20), so a tooling client finds it without searching by
extents. It is the innermost enclosing function of the expression's
context, as clang's `getPredefinedExprDecl` finds it
(`Sema::enclosing_function`), and always recorded (one word the node
already had, no table):

| written in | `predefined_function` | the name (clang 19 and 22, `-ast-dump`) |
|---|---|---|
| a function body, a constructor's member initializer, a coroutine | that function (the constructor, the coroutine) | `f`, `C::C()`, `co` |
| a lambda body | the closure's call operator (a generic lambda: its pattern, then each specialization) | `operator()` |
| a template pattern (a function template, a member of a class template) | the pattern's function; the node is type-dependent with no literal | none until instantiation |
| an instantiation | the specialization: instantiation rebuilds the node in the specialization (`TransformPredefinedExpr`) | `t`, `f` |
| namespace scope, a default argument, a default member initializer, a template head's requires-clause | `None`; `-Wpredefined-identifier-outside-function` (clang names the translation unit) | `""`, `top level` for `__PRETTY_FUNCTION__` |

clang's other contexts (a block, a captured statement, an Objective-C
method) do not exist in kiln. The module image carries the word (an
optional context declaration, `IMAGE_VERSION` 87) and the PCH snapshot
carries it as the node's word (`FORMAT_VERSION` 161, the roles row
changed). Tests: `crates/kiln_parse/tests/predefined.rs`, one case per
row.
`ExprArena::mark_instantiation_dependent(e)` sets
`ExprDeps::INSTANTIATION` on a `SourceLocExpr` or `CXXDefaultArgExpr`
whose function name changes per specialization (sema, §5.8).
`ExprArena::mark_type_dependent(e)` sets `ExprDeps::TYPE_VALUE_INST` on
a `DeclRefExpr` to a variable captured by copy in a lambda whose
explicit object parameter has a dependent type ([temp.dep.expr]/3;
sema-lambda.md §4.4).

## 17. Snapshots (WP-1380)

`Ast` is a `kiln_snap` root (spec/snap.md "Hand-written roots"); the
encoding is not called by the compiler yet. `crates/kiln_ast/src/ast/snap.rs`,
`types/snap.rs`, `consts/snap.rs` and `strings.rs` hold the hand-written
impls and their classification tables; every other table derives `Snap`.

- **State** (written as data): every arena and side table of
  `DeclArena`, `ExprArena`, `StmtArena`, `AttrArena`, `AnnotArena`,
  `IdLists`; the type context's node, canonical, side and name arenas,
  argument lists and expression profiles; the constant pools; the string
  bytes and entries; names, lookup sets, redeclarations, templates,
  reflections, modules, contract sites, the instantiation meter, the
  journals; the declaration-extent table with its on bit (§19.6); the
  shared `Arc<Interner>`. Hash maps are written as entries.
  **Every cache is state**: record layouts, class virtuals, vtable
  layouts and comparison categories are recomputable, but `-fdump-ast`
  prints cached layouts only and the categories record a check already
  diagnosed, so dropping them is observable.
- **Rebuilt** on read: the type context's intern tables (types,
  template names, nested-name-specifiers, argument lists, profiles) from
  the arenas plus the list of ids that were never interned (reserved
  nodes, VLA types), inserted in id order; the constant store's intern
  table; the string pool's table; the operator lookup sets' content
  index (§8.5, lazily, at the next intern). A later intern of existing content
  returns the old id, and new content gets the id it would have got in
  the original.
- **Transient**: `TypeContext.profile_scratch` (reset); `written_refs`
  is not part of the `Ast` encoding: since stage 5 (WP-1934) `Sema`'s
  root writes it as a section of its own right after the AST, and the
  restoring compilation's own option decides whether that section is
  read or skipped (§18.10.1). Within the table, the scratch of one
  declaration or instantiation (`inst_target`, `spec_pending`,
  `spec_last`, `spec_key`, empty at rest) is transient, its two
  indexes (`adl_index`, `spec_seen`) are rebuilt, and its `on` bit is
  the restoring compilation's.
- **Validated** on read (an `Invalid` error, never a panic): arena
  lengths agree (dummies present, `functions`/`min_args` paired), every
  node's payload or side index is inside the table it indexes, type
  lists and canonical links are in range, node kinds are known, every
  constant decodes and passes the interning checks, string entries lie
  in the byte pool, a record's base list and a cast's base path name
  base-specifiers in range. **Typed ids** (WP-1396): every id type of
  `ids.rs` decodes by hand, rejecting an index above its `MAX_INDEX` (and
  a `QualType` with type index 0) and recording the largest index read
  per id space (`ids::space`, `Reader::note_id`); the decoded AST checks
  each space's maximum against the arena it indexes (`Ast::check_ids`),
  and `Sema`'s restore checks again after the Oven and its own fields
  (which hold ids into the AST) are decoded. `MAX_INDEX` itself, the
  filler of the arenas' dummy entries, is not recorded. Not covered:
  ids packed into untyped `u32` slots (`ExprNode`/`StmtNode` operands
  other than cast paths, `IdLists` values other than base lists), which
  need a per-kind walk, and `DmsId` (it indexes the Oven's table).
- **Links out of the AST**: `Loc`, `Span` and `FileId` into the
  `SourceManager` (a separate root, which must be in the same snapshot);
  `Reflectee::DataMemberSpec(DmsId)` into the Oven's `DmsTable` (the
  `ReflectionTable.dms` copy exists only after codegen's `set_dms`);
  builtin ids in the cold decl maps into the compiled-in builtin tables
  (same binary only); the journals' readers are sema's side indexes.

Tests: `crates/kiln_driver/src/snap_tests.rs` (round trips of the
`bench/pch` prefixes and a GCC 16 `<meta>` prefix, corruption), and
`largest_records_are_fixed_size` in `types/snap.rs`.

## 18. The written-reference table (WP-1930)

An opt-in side table of the names written in the source, each with the
declaration it denotes. It serves tooling clients (cross-referencers,
renamers, include checkers) that need "what does this spelling name" for
every written name, without re-resolving. `kiln_ast::written_refs`; the
field `Ast::written_refs`. Built in stages:

| stage | content | row |
|---|---|---|
| 1 | the table, its iterators and dump; expression names, member names, nested-name-specifier components before them, with the `qualifier` chain | WP-1930 |
| 2 | type names in every type-specifier context, template names, base specifiers, member-initializer ids | WP-1931 |
| 3 | declarative contexts: using-declarations and -directives, namespace alias targets, declarator-id qualifiers and the names they redeclare, elaborated and friend references, specialization targets, concept names, designators, deduction-guide names; their lookup provenance; the identifier-accounting test (§18.8) | WP-1932, WP-2100, WP-2131 |
| 4 | per-specialization entries for dependent names, in a separate table keyed by (pattern entry, specialization) (§18.9) | WP-1933 |
| 5 | carriage in PCH snapshots and module images | WP-1934 |
| | the join with expressions; the leading `::` and the `~` (§18.11) | WP-2326 |
| | the found set of a dependent entry: what lookup found at the template definition (§18.12) | WP-2472 |
| 6 | operator uses and user-defined-literal suffixes outside dependent contexts: roles `operator` and `ud-suffix`, flags `REWRITTEN` and `REVERSED`, the closing token of `[]` and `()` (§18.13) | WP-2820 |
| 6b | dependent operators, folds, their found sets and per-specialization rows, `SPEC_BUILTIN` rows, dependent `(` (§18.13.6) | WP-2910 |
| 6c | `new`, `delete` and `co_await` entries, dependent ones with their rows (§18.13.6) | WP-2911 |

### 18.1 Contract

- **Opt-in.** `WrittenRefs::is_on()` is false unless the front end's
  `ParseOptions::written_refs` is set (the driver's
  `-fkiln-written-refs` and `-fkiln-dump-written-refs` set it, and the
  front-end library entry's `FrontEndOptions::written_refs`, WP-2320,
  spec/driver.md "Front-end library entry"). Off,
  every recording site costs one predictable branch on that flag and
  allocates nothing; the table is empty, `Ast::memory()` has no row for
  it, and a snapshot or image differs from a compiler without the table
  in its format version and one empty-section marker only (§18.10.4).
- **One entry per written name.** A name is recorded once where the
  parser consumed it, however many times lookup ran on it, when the
  expression it names is built: after the names in its own template
  argument list, before those of a following call's arguments; a
  qualifier's components come before the name they qualify. Clients that need source order sort by `range`. Names are recorded from parser call sites, never from the
  sema actions they call (those also run for implicit names and during
  instantiation), except where noted in §18.4.
- **Read access**: `iter()` (all entries, with their `WrittenRefId`),
  `get(id)`, `len()`, `enclosed_by(d)` (the entries whose `enclosing` is
  `d`), `dump(ast, sm, main)` (§18.6). Entries are never removed after
  the parse except by `truncate` (§18.5).

### 18.2 The entry (28 bytes, a `size_of` unit test)

| field | type | meaning |
|---|---|---|
| `range` | `Span` (8) | the name as written: `lo` the start of its first token, `hi` the end of its last token, as kiln token locations (macro provenance intact, §18.5). The name only: `a::b::f` is three entries. An operator-function-id (`operator==`, `operator""_x`, `operator new[]`) is one entry over all its tokens; a destructor name `~T` has the range of `T`; an operator use is its operator token (`[` or `(` of a subscript or call, the closing token in the leads, §18.11.2) and a ud-suffix the suffix within its literal token (§18.13) |
| `entity` | `Option<DeclId>` (4) | the declaration the name denotes after resolution: for an overloaded name, the function overload resolution selected (a function template specialization when it selected one); through a using-shadow, the underlying declaration. `None` for a dependent name in a template pattern and for a name whose resolution failed (flags tell which) |
| `found` | `Option<DeclId>` (4) | what lookup found when it differs from `entity`: the using-shadow (`found_decl` of the reference), or the function or variable template whose specialization `entity` is |
| `enclosing` | `Option<DeclId>` (4) | the innermost declaration whose source extent contains the name's first token (§18.3); `None` only when the table was filled without a TU |
| `qualifier` | `RefQualifier` (4) | for a name written after `::`: the entry of the component immediately to its left, `GLOBAL` for a leading `::`, or `SPECIFIER` after a `decltype(...)::` or splice specifier, which has no entry (§18.11.2, WP-2828); `NONE` for an unqualified name |
| `role` | `RefRole` (1) | the syntactic role (§18.4) |
| `flags` | `u8` (1) | `DEPENDENT` (a dependent name in a template pattern: no entity until instantiation), `UNRESOLVED` (lookup or overload resolution failed), `MEMBER` (a member or member template name written after `.` or `->`, §18.7), `REWRITTEN` (8: an operator entry whose function a rewritten comparison candidate selected, [over.match.oper]/3.4), `REVERSED` (16: the reversed synthesized candidate, always with `REWRITTEN`) (§18.13.3), `GROUP` (32: a name of a decl-specifier-seq a declarator group shares, §18.3.1) |
| `how` | `RefLookup` (1) | how lookup found the name (§18.7, WP-2130) |

**Failed lookup and dependence (WP-2291).** `UNRESOLVED` and
`DEPENDENT` never both hold, and an entry with an entity is never
`UNRESOLVED` (one that is also `DEPENDENT` prints `dep`, §18.6). Which
of the two an entry without an entity has:

- **Outside a template**, a name that names nothing is `UNRESOLVED`
  (`(unresolved)`): an undeclared identifier, a member name after `.` or
  `->` whose object expression is invalid or not of class type, a call
  no candidate fits, a name in an expression sema could not form (a
  template-id whose template argument is invalid). kiln builds a
  recovery expression for such a name, type-dependent like clang's
  `RecoveryExpr` (its type is unknown); that dependence is error
  recovery, not a dependent name, and the table reads the recovery
  expression as a failure, never as `DEPENDENT`.
- **In a template pattern**, a dependent name ([temp.dep.general]: a
  type- or value-dependent id-expression, a member of a dependent
  object expression or of an unknown specialization, a component of a
  dependent nested-name-specifier and the name after it, a dependent
  type name) is `DEPENDENT`, and so is the callee of an unqualified call
  with a dependent argument even when ordinary lookup found nothing
  (argument-dependent lookup at instantiation may find it,
  [temp.dep.candidate]). A non-dependent name is looked up and bound at
  the definition ([temp.res.general]); when that fails it is
  `UNRESOLVED`, exactly as outside a template, and clang diagnoses it at
  the definition too: an undeclared identifier, a member name after an
  invalid object expression, a name in an expression sema could not form
  (also when the name itself is dependent, `T::template f<undeclared>`:
  the expression is ill-formed in every specialization), and the callee
  of a call with no dependent argument whose overload resolution fails
  at the definition. Such a callee is recorded before its call's
  arguments are parsed, as `DEPENDENT` until they tell, and becomes
  `UNRESOLVED` when the call is resolved at the definition and fails
  (`written_call_failed`).

A `DEPENDENT` entry with a definition-context lookup also has a found
set, the declarations that lookup found, in a side table (§18.12); the
entry itself does not change.

One padding byte remains. Deviation from a 4-byte `entity`-only design:
`found` and `qualifier` are stored, not derived, because deriving `found`
needs the expression (not kept for nested-name-specifier components) and
deriving the chain needs range arithmetic over tokens.

`RefQualifier` is a `u32`: 0 none, `u32::MAX` global, otherwise a
`WrittenRefId` index + 1. A qualifier entry always precedes the entry it
qualifies. The tokens of a name outside its `range`, the leading `::`
of a globally qualified name and the `~` of a destructor name, are in a
cold side table (§18.11.2); the location an expression stores for a
name is `name_lo(id)` (§18.11.1).

### 18.3 `enclosing`

Stored, not derived from ranges. **The rule**: a name is held by the
innermost declaration whose text contains it — the particular
redeclaration (a reopened namespace, the definition of a function, the
class being defined). A name inside a declaration (its type, its
initializer or value, its default argument, its declarator-id
qualifier, its attributes) is held by the declaration being declared,
whether that declaration has a name or not; a name in no declaration
is held by the lexical context (the namespace, class or function whose
body holds it). The holder is what a tooling client takes as "what an
edit of this name changes": the unit to check again. The cases
(WP-3143 completed the rule for the last five):

| declaration | holds the names of |
|---|---|
| a declarator of a simple declaration or a member declaration (a variable, a function declaration, a field, a `typedef` name) | its declarator and initializer, bit-field width or `= 0`; the decl-specifier-seq when it is the group's first (§18.3.1) |
| a function definition | its decl-specifiers, declarator and body (outside the declarations nested in it) |
| a function parameter | its type and default argument (also late-parsed): so in `typedef void F(S);` the unnamed parameter holds `S`, as in a function declaration |
| a field with a default member initializer | the initializer, also late-parsed |
| a concept definition | its constraint-expression |
| an alias-declaration (an alias template's pattern) | its type-id |
| an enumerator | its attributes and its value |
| a `static_assert` | its condition and message: the `StaticAssert` declaration (a `DeclId` without a name; the dump prints `static_assert@<line>:<col>` of its keyword) |
| a template | its template-head (each template parameter's type and default argument, the requires-clause): held by the templated declaration, the same declaration that holds the rest of the template's names (a function template's function, a class template's class, an alias template's alias, a variable template's variable); where no template owns the head's parameter list (a class or variable template partial specialization, an out-of-line member of a class template), the declaration the head's declaration declared; an enclosing head (`template<class T> template<class U>`) is held by the same declaration |
| stage 3: a using-declarator, a namespace alias, a tag declared by a class-, enum- or elaborated-type-specifier | its qualifier and name (the target of an alias); a tag its class-head-name and qualifier, an explicit or partial class specialization's template name |

Not holders: a template parameter (its names are the template's: an
edit of them changes the template's signature, and the parameter is no
unit of its own), a requires-expression's parameter scope (its names
are held by the context around it), a friend's tag names (held by the
class granting friendship), a using-directive (it declares no named
entity) and an explicit instantiation of a function or variable (it
declares nothing; its names are held by the context its qualifier
enters).

Mechanism: an entry is recorded with sema's current lexical context
(`Sema::ctx`: the namespace, class or function whose body is being
parsed). A declaration whose names are parsed before its `DeclId`
exists claims them: the parser takes `Ast::written_ref_mark()` (entry
count, decl count) at the declaration's start and calls
`claim_written_refs(mark, d)` once `d` exists (after the declarator,
again after the initializer). A claim sets `enclosing = d` on entries
after the mark whose current `enclosing` was created before the mark,
so a declaration nested inside (a lambda, a local class, a parameter)
keeps its own entries: the innermost claim wins. The declarators of
stage 1 also claim their `redecl`, `spec-target` and `elaborated`
entries. A template head's mark is taken at `template`; when the
templated declaration is parsed, `claim_template_head(mark, parsed)`
gives the head's remaining entries to the templated declaration (the
pattern of the template whose parameter list the head is, else
`parsed`, the last declaration the head's simple-declaration declared).
An enumerator and a `static_assert` claim with `claim_written_names`,
names only: their closures keep the lexical context (§5.9). The same
marks and claims decide closure owners otherwise (§5.9, WP-2023):
every other claim also claims the closures created since its mark,
and template parameters claim closures (not names).

#### 18.3.1 Declarator groups (WP-3144)

In `S a, b;` the decl-specifier-seq is written once and belongs to each
declarator ([dcl.decl.general]/3: each init-declarator is as if it were
in a declaration by itself), so a rename of `S` changes `b` too. The
one entry of `S` is held by the group's first declaration (`enclosing
= a`, as every reader has always seen it) and has the flag `REF_GROUP`
(32); the group's declarations, in order, are in a cold side table
keyed by the first: `group(first) -> Option<&[DeclId]>`, `holders(id)
-> &[DeclId]` (the group for a `REF_GROUP` entry, else `enclosing`
alone), `groups()` (every group, by first declaration). Which entries
are shared: those recorded in the decl-specifier-seq and the leading
attributes (before the first declarator) that the first declaration
holds after its claim. A name in a class or enumeration the specifier
defines, or in a lambda in it, keeps its innermost holder and is not
shared. A group has a row only when an entry is shared; a declarator
whose declaration failed is not in it. The cases: a member declarator
list (`struct C { S a, b; };`, `C::a` and `C::b`), a `typedef` list
(`typedef S T1, T2;`), a function and a variable (`S f(), v;`: `f` and
`v`); a structured binding declaration is one declaration whose
bindings are no declarator group (no flag); a group of one has no flag
and no row. Set by `Sema::act_on_end_group`, from the entry range the
parser gives `group_after_specifiers` after the decl-specifier-seq
(`WrittenRefs::share_group`); a truncation drops the rows whose first
declaration was made after its mark.

Rationale: `enclosing` stays one 4-byte field (the entry stays 28
bytes; the flag takes a free bit), so a reader that wants one holder is
unchanged and one that wants every holder tests one bit. The list costs
8 bytes per group and 4 per declaration, only for groups that share a
name: nothing for the single-declarator declarations that are nearly
all of them. A list per entry would repeat the group for every name of
the specifier (`std::map<K, V> a, b;` has four), and a mark without the
list would leave the client to rebuild the group from source or from
the declaration-extent table, another opt-in table (§19.5). Carried:
the snapshot section's `groups` and `group_decls` (§18.10.1) and the
module image `refs` section's trailing groups (§18.10.2).

### 18.4 Roles

`RefRole` is `#[repr(u8)]` and covers the full set; stage 1 fills
`Expr`, `Member` and `Nns`, and `Concept` for a concept-id parsed as an
id-expression; stage 2 (WP-1931) fills `Type`, `Template`, `Base` and
`MemInit`; stage 3 (WP-1932) fills `UsingDecl`, `UsingDirective`,
`NamespaceAlias`, `Elaborated`, `SpecTarget`, `Designator` and
`Redecl` (added by stage 3), `Concept` for a type-constraint, and
`Template` for a deduction guide's name; stage 6 (WP-2820) adds and
fills `Operator` and `UdSuffix` (§18.13); WP-3688 adds and fills `Label`
for written label uses.

| role | dump | example (bracketed) | stage |
|---|---|---|---|
| `Expr` | `expr` | `x = [f](y);` `&[g]` `[Enumerator]` `case [E]:` capture `[[x]]{}` `[operator==](a, b)` | 1 |
| `Member` | `member` | `a.[m]` `p->[m]()` `p->~[X]()` (a class's destructor) `a.[operator+](b)` | 1 |
| `Type` | `type` | `[T] x;` `new [T]` `static_cast<[T]>(e)` `sizeof([T])` `catch ([E]&)` `typename A::[type]` `struct [S]* p;` (an existing class) `using enum [E];` `p->~[T]()` (a pseudo-destructor) | 2 |
| `Template` | `template` | `[vector]<int>` CTAD `[pair] p{1,2};` `X<[vector]>` `a.template [f]<int>()`; a deduction guide's name `[Pair](T, T) -> Pair<T>;` (stage 3) | 2, 3 |
| `Nns` | `nns` | `[a]::[b]::f()` `[T]::x` `[Enum]::e` `[alias]::f()`; declarator-ids in stage 3 | 1 |
| `Base` | `base` | `struct D : [B]` | 2 |
| `MemInit` | `mem-init` | `D() : [B](1), [m](2) {}` | 2 |
| `UsingDecl` | `using-decl` | `using a::[f];` `using typename T::[X];` `using B::[B];` `using Ts::[f]...;` | 3 |
| `UsingDirective` | `using-directive` | `using namespace [n];` | 3 |
| `NamespaceAlias` | `ns-alias` | `namespace x = a::[b];` | 3 |
| `Elaborated` | `elaborated` | `friend class [Y];` `friend void [f](int);` `class [X];` `enum class [E] : int;` naming an existing entity, `friend struct N::[G];` (a declaration; `class [X]* p;` is `type`, stage 2) | 3 |
| `SpecTarget` | `spec-target` | `template <> struct [hash]<T> {};` `template class [vector]<int>;` `template void [f](int);` `template void S<int>::[g]();` | 3 |
| `Concept` | `concept` | `template <[C] T>` `[C] auto x` `{ e } -> [C]`; a concept-id as an expression (`static_assert([C]<int>)`, `requires [C]<T>`) from stage 1 | 1, 3 |
| `Designator` | `designator` | `{ .[field] = 1 }` `{ .[a].[b] = 1 }` `{ [field]: 1 }` `__builtin_offsetof(S, [a].[b])` | 3 |
| `Redecl` | `redecl` | `void A::[f]() {}` `int C::[x] = 0;` `struct C::[In] {};` `A::[A]() {}` `A::~[A]() {}` | 3 |
| `Operator` | `operator` | `s [+] s` `a [!=] b` (rewritten) `m[[]1]` `f[(]x)` `p[->]m` `m[,] 4` `[++]m`; `[new] T` `[delete] p` `[delete[]] p` `::[new] T` `[co_await] e` (stage 6c) | 6 |
| `UdSuffix` | `ud-suffix` | `12[_k]` `1.5[_f]` `'x'[_c]` `"ab"[_s]` `R"(r)"[_s]` | 6 |
| `Label` | `label` | `goto [l];` `&&[l]` `asm goto ("" : : : : [out]);` | WP-3688 |

Not in the table: a declaration's own name at its declaration (an
unqualified first declaration, including a first `friend class Y;`, and
label declarations `l:` and `__label__ a, b;`, which are §19
declaration-extent rows), keywords, attribute names, module names,
built-in operators, and implicit references with no written token
(implicit conversions and constructor calls, range-for iterator
operations, coroutine and structured-binding helpers, the bodies of
defaulted comparisons; §18.13.1). An operator token that selects a
function is an `operator` entry (§18.13).

Label uses (`RefRole::Label`, WP-3688): `goto l`, `&&l`, and each label
operand of `asm goto` record a `Label` entry at the label identifier's
token with `entity` set to the function's (or enclosing block's
`__label__`) `LabelDecl` and `how` `Unqualified` (`None` / `UNRESOLVED`
when `&&l` is rejected outside a function). A non-`__label__` label
first mentioned by a forward jump updates its `LabelDecl` location to
its `l:` definition when defined (as clang). Computed `goto *p` records
no `Label` entry for `*p` (only the expression's own entries and any
`&&l` that feeds it). Inside a template pattern, labels are
function-local and resolve at definition to the pattern's `LabelDecl`,
with no per-specialization (`SpecRef`) or keyed (`SubstRef`) rows (the
same rule as local variables and parameters). `AstNames::decl_key` gives
every label a unique stable key (`f()::l`, with `#2`, `#3`, ... for
multiple `__label__` declarations of one name in a function). Carried in
PCH snapshots (`FORMAT_VERSION` 191) and module declaration images
(`IMAGE_VERSION` 104).

Stage-1 recording sites (all in `kiln_parse` unless noted):

- id-expressions (`parse_id_expression_rest`, `parse_operator_id_expression`):
  plain and qualified names, enumerators, `&f`, `case E:`, template-ids
  (the template name; its arguments are their own entries), operator-
  and literal-operator-ids, `X::~X()` in a member function; the pack
  named by `sizeof...(pack)` (`parse_sizeof_pack`, WP-1932: found by
  the identifier accounting of §18.8);
- member access (`parse_member_access_suffix`): `.`/`->` names,
  qualified members, `template` members (role `template` since stage 2),
  operator-function-ids, `p->~X()` naming a class's destructor (a
  pseudo-destructor is a stage-2 `type` entry);
- nested-name-specifier components preceding them
  (`parse_nested_name_upto`): namespace, namespace-alias, class, enum,
  typedef, template-id and dependent components, each with its own
  entry and `qualifier` link; a `decltype(...)::` component has no
  entry (its operand's names have their own) and the component after it
  has the `qualifier` `SPECIFIER`, with the `decltype` keyword as its
  `specifier_lo` (WP-2828, §18.11.2); so has the component after a
  splice-scope-specifier `[: r :]::` (the `[:`) or a
  splice-specialization-specifier `template [: r :]<...>::` and
  `typename [: r :]<...>::` (the `template` or the `[:`); a
  pack-index-specifier `Ts...[0]::` is a component: the pack's name is
  an `nns` entry that qualifies the next one (WP-1934);
- lambda captures (`kiln_sema::lambda::capture`, parse path only):
  `[x]`, `[&x]`, `[x...]`, enclosed by the declaration containing the
  lambda-expression (the closure type is not a written declaration); an
  init-capture declares a name and is not an entry.

The entity of an id-expression or member name comes from the built
expression (`DeclRefExpr`, `MemberExpr`; implicit casts and
`ConstantExpr` peeled). When the expression is an overload set
(`UnresolvedLookupExpr`, `UnresolvedMemberExpr`) the entry is made at
the name with no entity and remembered against the set's `ExprId`; the
overload-resolution funnels (`finish_overload_ref`,
`resolve_member_call`, address-of-overload resolution) patch
`entity`/`found` once with the winner. An unqualified template-id
that names member function templates (`s<char>()` inside a member) is
rebuilt as an implicit member access: the remembered set moves to the
rebuilt `UnresolvedMemberExpr` (`WrittenRefs::retarget`) before
`resolve_member_call` patches it. A concept-id parsed as an
id-expression (`ConceptSpecializationExpr`) records role `concept` with
the concept as entity. A set that is never resolved
(a dependent call in a pattern, a call that failed) stays without an
entity. Patching runs only outside instantiation (instantiation builds
new expressions; their entries are stage 4).

Stage-2 recording sites (WP-1931; all in `kiln_parse`, entries made by
`Sema::note_written_type_found`):

- type-names in a decl-specifier-seq or type-specifier-seq
  (`simple_type_name_spec`, the injected-class-name arms of
  `parse_type_name_spec`), so every context that parses one: simple
  declarations and members, parameters, return types (also trailing),
  conversion-function-ids, every cast (`static_cast<T>`, `(T)e`,
  `T(e)`, `T{e}`), new-expressions, `sizeof`/`alignof`/`noexcept`/
  `decltype` operands, template arguments,
  exception handlers, typedef and alias-declaration targets, default
  template arguments;
- template-ids in type contexts and CTAD placeholders
  (`parse_template_type_name`, `parse_qualified_type_rest`,
  `parse_ctad_placeholder`): the template name, role `template`, entity
  the class or alias template; template template arguments and
  parameter defaults (`at_template_name_arg`,
  `parse_template_template_param_default`): role `template`, the
  qualifier's components recorded as `nns` when the argument scan did
  not record them;
- the last component of a typename-specifier (`typename A::B::[type]`;
  the earlier components are `nns`);
- an elaborated-type-specifier that refers to an existing type
  (`parse_tag_specifier` with `TagUse::Reference`, a class or enum
  that existed before the specifier), role `type`; one that declares
  (`struct S;`, `friend class Y;`) is stage 3;
- the operand of `using enum` (`parse_using_enum`), role `type`;
- a pseudo-destructor's type name (`p->~T()` with `T` a scalar typedef
  or a template parameter; from the built `CXXPseudoDestructorExpr`),
  role `type`, entity the typedef or template parameter; in a qualified
  pseudo-destructor-name `p->[S::]F::~Q()` (WP-1934) both `F` (a
  type-name, not a nested-name-specifier: looked up as the access looks
  it up, `Sema::pseudo_destructor_first_type`) and `Q` are role `type`,
  qualified by `S`'s last component (or `::`), a simple-template-id's
  template name as the entity, both dependent under a dependent `S`;
- a pack-index-specifier that begins a nested-name-specifier
  (`typename Ts...[0]::type`, WP-1934): the pack's name, role `nns`,
  entity the pack, qualifying the next component;
- base-specifiers (`parse_base_specifier`): role `base` on the class
  name, on the template name of a template-id base (its arguments are
  their own entries) and on the last component of a qualified or
  dependent base;
- mem-initializer-ids (`parse_mem_initializer`): role `mem-init` on a
  member, a base, a delegating constructor's class name, and the
  template name of a template-id base. An identifier's entry is made
  before its arguments are parsed and filled once the initializer's
  target is known (`Sema::resolve_written_mem_init`).

Stage-2 rules. The entity of a type name comes from the type's sugar
(`written_type_entity`): the typedef, alias, template parameter,
injected class, class or enum; a template-id's or CTAD placeholder's
template; a pack-indexing type's pack. `found` is the using-shadow
lookup found when a using-declaration brought the type or template in
(a simple type name keeps no using sugar, so the site re-runs the
name's lookup only while recording). A dependent type name
(`typename T::type`, `T::base_type` as a base, a member of an unknown
specialization) has no entity and `DEPENDENT`; a name that names no
type (an error) has none and `UNRESOLVED`. A base or mem-initializer
written as a template-id is first recorded as a type name and its
outermost name re-roled (`WrittenRefs::set_role`). A type-name's entry
follows the entries of its template arguments and follows its
qualifier's entries, as an id-expression's does. Every stage-1 rule
applies unchanged: `enclosing` comes from the same claims, macro names
carry `exp=`, pattern names are recorded once, default arguments once
at their written site.

Stage-3 recording sites (WP-1932; all in `kiln_parse`, entries made by
`Sema::note_written_decl_name`, `note_written_using_declarator`,
`note_written_designator` and `note_written_type_found`):

- using-declarations (`parse_using_declaration`), role `using-decl`:
  one entry per using-declarator, over its unqualified-id (an
  operator-function-id is one entry; `using B::B` names the inherited
  constructor), after the qualifier's `nns` entries, made once the
  declarator's declaration exists ([namespace.udecl]/1). The entity is
  the first declaration the using-declarator introduced (the target of
  its first using-shadow; with no shadow, the first declaration a
  qualified lookup of the name finds): `using N::f;` with two
  overloads names the first declared. `found` is none. `typename`, a
  pack (`using Ts::f...;`) and a dependent qualifier give no entity and
  `DEPENDENT` (an unresolved using-declaration); a name the qualifier's
  scope does not declare gives `UNRESOLVED`. `using enum E;` stays a
  stage-2 `type` entry;
- using-directives (`parse_using_directive`), role `using-directive`:
  the nominated namespace name, after its qualifier's entries
  (`using namespace ::A::B;`). The entity is what a namespace-only
  lookup of the name finds ([namespace.udir]/1: only namespace names are
  considered, so a function of the same name does not hide it); a
  namespace alias names the alias. None found: `UNRESOLVED`;
- namespace alias definitions (`parse_namespace_alias`), role
  `ns-alias`: the target's last component, after its qualifier's
  entries, by the same namespace-only lookup ([namespace.alias]/1); the
  alias's own name is a declaration and has no entry;
- qualified declarator-ids (`note_declarator_ref` after
  `act_on_declarator`, `act_on_member` and function definitions), role
  `redecl`: the declared name of a declaration whose declarator-id is
  qualified ([dcl.meaning.general]/2: `void A::B::f() {}`, `int C::x =
  0;`, `A::A() {}`, `A::~[A]() {}` over the class name after `~`,
  `A::operator int()` over the whole conversion-function-id) names the
  first declaration of the entity it redeclares, after the qualifier's
  `nns` entries (`Declarator::id_link`); so does a qualified
  class-head-name of a class or enumeration definition (`struct
  C::In {};`, `template <> struct S<int>::In {};` naming the member of
  the specialization, [class.pre]/3). A qualified declarator-id that
  matches no declaration (diagnosed) has no entity and `UNRESOLVED`
  (`DEPENDENT` when the qualifier is dependent). A function
  definition's entry is made after its body, so it follows the body's
  entries (sort by `range` for source order);
- elaborated-type-specifiers and friend declarations, role
  `elaborated`: a declaring elaborated-type-specifier
  (`parse_tag_specifier`, [dcl.type.elab]/2) or opaque enum
  declaration that names an entity declared before it (`struct S;`
  after `S`, `enum class E : int;` after `E`), a friend class or
  function declaration that redeclares an earlier one (`friend class
  F;`, `friend void g(int);`), and every qualified friend (`friend
  struct N::G;`, `friend void N::h();`, [class.friend]) name its
  first declaration. A first declaration's name (`struct Fresh;`, an
  unqualified `friend class Unseen;` that declares the class) is the
  entity's own and has no entry. A friend through a dependent
  qualifier (`friend class T::X;`, `friend void T::f();`) has no
  entity and `DEPENDENT`. A friend function template specialization
  (`friend void f<>(int);`, [temp.friend]/1.3) is `elaborated` too,
  its entity the template whose specialization it befriends: the one
  its declaration matched, not the first that lookup of the name found
  (`f(T)` and `f(T, int)` are two templates; WP-3008), outside a
  template and in one when it depends on nothing of the template
  (`friend void f<int>(int);`, matched at the definition as clang's
  `CheckFunctionTemplateSpecialization` matches it). In a template
  pattern with dependent template arguments or type (`friend void
  f<>(T);`) the match waits for each specialization of the class
  (`templ/subst/decls.rs` `friend_decl`): the entry has no entity and
  `DEPENDENT`, its found set (§18.12) the function templates lookup of
  the name found, which are what each instantiation selects from
  (closed: no argument-dependent lookup; clang's
  `DependentFunctionTemplateSpecializationInfo`), and each
  instantiation adds a row naming the specialization befriended, found
  as its template (§18.9.4). The entry keeps the template argument
  list's `<` and `>` (§18.11.2: the arguments are kept nowhere else
  until the match). Sema patches the parser's entry once it has
  matched (`Sema::note_written_spec_template`,
  `note_written_friend_spec`, a sema exception to §18.1). A friend that
  is a member of every specialization of a class template
  ([temp.friend]/5: `template <class T> friend void S<T>::f();`, which
  clang does not model, `-Wunsupported-friend`) names the member of the
  class pattern its qualifier names, as an out-of-line member
  definition's qualifier names one ([temp.spec.partial.member]/1): the
  primary template's when the template-id's arguments are the friend's
  own parameters, each once, renamed or not; a partial
  specialization's own member when they are that partial
  specialization's arguments with its parameters renamed (`S<T *>::f`,
  a different entity from `S<T>::f`); then the member classes after it
  (`S<T>::In::g`); then the member named, of the friend's kind (a member
  function template when the friend has a template head of its own) and,
  among overloads, of the friend's type with its parameters renamed to
  the pattern's (`Sema::member_friend_pattern_member`, WP-3008). A
  qualifier that names no single pattern (`S2<T, int>::f`, matched per
  specialization only), a member the pattern does not declare and
  overloads the type does not tell apart leave the entry `DEPENDENT`;
- specialization targets, role `spec-target`: the template name of an
  explicit specialization ([temp.expl.spec]/1), a partial
  specialization ([temp.spec.partial.general]/1) and an explicit
  instantiation definition or declaration ([temp.explicit]/2, `extern
  template` included), after the names in its template argument list:
  a class template's in `parse_class_template_id_tag`, a function or
  variable template's in `parse_specialization_declarator_id`. Its
  entity is the primary template; for a function template, the one the
  declaration matched by deduction, which `note_declarator_ref` takes
  from the declaration (the specialization's template, or sema's
  explicit instantiation target): lookup may find several overloads
  and the parser records the first (WP-3008). An explicit instantiation or
  specialization of a function template written without a template
  argument list (`template int f(int *);`) is matched by deduction, so
  sema says what it named: the function template, or, for an explicit
  instantiation of a member of a class template specialization
  (`template void S<int>::g();`, `template S<int>::S();`, `template
  int S<int>::n;`), that member (`Sema::note_written_inst_target`, a
  sema exception to §18.1: the target is known only after matching;
  the parser records the entry once the declaration is done). An
  explicit instantiation of a function or variable declares nothing:
  its names are enclosed by the context around it;
- type-constraints (`parse_concept_name_or`), role `concept`: the
  concept name of a type-constraint in a template parameter list
  ([temp.param]/2), of a constrained placeholder (`C auto x`,
  [dcl.spec.auto.general]/1) and of a compound requirement's
  return-type-requirement (`{ e } -> C`, [expr.prim.req.compound]/1),
  qualified or not, after the names in its template argument list;
- designators (`parse_designators`, the GNU `field:` form), role
  `designator`: each field designator's identifier ([dcl.init.aggr]/3.1;
  `.a.b` and `.a[0].b`, a C99 extension, one entry per member). The
  entry is made at the designator with `MEMBER` and no entity
  (`DEPENDENT` inside a template, else `UNRESOLVED`) and remembered by
  its location (`WrittenRefs::defer_designator`); the initialization
  check (`Sema::designated_field`'s caller in `kiln_sema::init`) patches
  it once it finds the member in the aggregate's class
  (`resolve_written_designator`, the second sema exception: the
  aggregate is known only there). The last patch wins: a member of an
  anonymous union or struct is the member named
  ([class.union.anon]/1), not the anonymous member that contains it,
  and in overload resolution the chosen candidate's check runs last. A
  designator of a dependent aggregate in a pattern stays `DEPENDENT`;
  one naming no member stays `UNRESOLVED`;
- offsetof member-designators (`Sema::act_on_builtin_offsetof`, a
  sema site: only member lookup knows the member), role `designator`:
  each member name of `__builtin_offsetof(T, a.b[i].c)` and of the
  `offsetof` macro that spells it ([support.types.layout]/1; `a`, `b` and
  `c` are three entries), with `MEMBER` and the member lookup found (the
  member written, past the anonymous members on its way: the `Field`
  node of `ExprArena::offsetof_designator` for the name, §6), or
  `UNRESOLVED` when it names nothing: a name lookup did not find, and
  every name after a component that failed (a member not found, a
  bit-field, a subscript of a non-array, an operand that is not a
  complete class; WP-2471). In a template pattern whose designator is
  deferred to instantiation (a dependent operand type or a
  value-dependent index), the names are resolved as clang's
  `BuildBuiltinOffsetOf` resolves them at the definition: a name into a
  type that is not dependent is looked up there and has its member
  (`offsetof(S, a.b[N].c)` names `S::a`, `M::b` and `In::c`), each name
  from the first dependent type on is `DEPENDENT`, as is one into a class
  not complete at the definition (the table instantiates nothing), and a
  name not found there, with every name after it, is `UNRESOLVED`
  (WP-2471);
- deduction guides (WP-2100; `note_declarator_ref` on a
  `SpecialName::DeductionGuide` declarator), role `template`: the
  guide's own name names the class template ([temp.deduct.guide]/1),
  as the template-id after `->` does (stage 2).

Stage-3 rules. Every stage-1 rule applies: one entry per written name,
`qualifier` links to the qualifier's last component, macro names carry
`exp=`, a pattern's names are recorded once with dependent ones
`DEPENDENT`, and instantiation records nothing. The entity of a
declarative name is the first declaration of what it names (a
using-declaration's, a redeclaration's, a friend's), so all the names
of one entity have one entity id however many redeclarations there are.
`Redecl` is new in stage 3; `RefRole`'s other values were reserved by
stage 1.

### 18.5 Speculation, macros, templates, PCH

- **Speculation.** kiln's parser never re-parses consumed tokens:
  disambiguation (`kiln_parse::tentative`) is a side-effect-free token
  scan that classifies names without building or recording anything, and
  late-parsed regions (default arguments, default member initializers,
  member function bodies) are replayed exactly once. So a name has one
  entry however often it was classified, and an abandoned scan leaves
  none. `mark()`/`truncate(mark)` exist for any future rollback (they
  also drop pending overload patches and deferred designators); none is
  needed in stages 1 to 3 (a parenthesized qualified declarator-id
  `int (C::x) = 0;` and a declaration statement `C::T(u);` are
  classified by the scan and recorded once by the kept parse).
- **Macros.** `range` holds the token locations the lexer gives, so an
  entry for a name from a macro body or argument has a macro-expansion
  location and the source manager maps it to its spelling and expansion
  points as for diagnostics.
- **Templates.** Inside a pattern, a non-dependent name has its entity
  as resolved at definition; a dependent name (`DependentScopeDeclRefExpr`,
  dependent members, a dependent call's callee, a dependent
  nested-name-specifier component, a dependent type name) has `entity: None` and
  `DEPENDENT`. Instantiation adds no main-table entry: what a
  dependent name resolves to in each specialization goes to the
  separate per-specialization table (stage 4, §18.9), so the main
  table's entry shape and its one-entry-per-written-name rule do not
  change. What lookup found at the definition, before any
  specialization, is the entry's found set (§18.12). Default arguments
  and default member initializers are entries once, at the written site.
  The pack's name in a pack-indexing expression (`ts...[0]`,
  [expr.prim.pack.index]) names the pack (`ts@`), as it does in the
  expansion `ts...` and as the pack-index-specifier `Ts...[0]` names
  `Ts`: only the indexing is dependent (WP-2721; clang's
  `PackIndexingExpr` holds a `DeclRefExpr` to the pack). A member
  variable template's specialization (`d.twice<7>`) has the template as
  its found set and keeps its template arguments (the member access's
  `member_extra`) outside a template as inside one (WP-2724).
  **A template-id with dependent arguments** whose template name is not
  dependent (WP-3009): lookup at the definition resolved the template
  name ([temp.names]/2, [temp.res.general]/1), and when that decides
  what the name denotes, the entry names the template, with no
  `DEPENDENT` and no per-specialization rows: a class or alias
  template (`n::K<T> x;`, a `template` entry, stage 2), a concept
  (`C<T>`, `concept`) and a variable template (`n::v<T>`, an `expr`
  entry; `n::S::w<T>` and `n::D<int>::w<T>` in a class that is not
  dependent; `v<T>` after `using n::v;`, the template, as the
  non-dependent `v<int>` and a function template-id's found set keep
  no using-shadow).
  None of them is overloaded, so each specialization is that
  template's whatever the arguments ([temp.dep.expr]/3 makes the
  expression dependent, not the name). A function template-id
  (`n::g<T>(t)`) is an overload set: which template, and which
  specialization of it, is decided per specialization by overload
  resolution, and an unqualified one may gain candidates by
  argument-dependent lookup; it stays `DEPENDENT`, its found set
  (§18.12) the templates lookup found. So is a friend function template
  specialization with dependent arguments (§18.4), whose candidates are
  matched per specialization. A member template of a dependent class
  (`n::D<T>::template w<U>`, `T::template f<U>`) stays `DEPENDENT` with
  no definition-context lookup.
  **The rule, every form (WP-3405).** The template name of such a
  template-id has its one entry at the name token (§18.1: the whole
  template-id is no second entry; its arguments' names are their own
  entries, before it), and a rename of the template claims it with the
  rule a rename already uses: an entry whose `entity` or `found` is the
  template (the non-dependent `n::v<int>` names the specialization,
  `found` the template; the dependent `n::v<T>` names the template), and,
  for a function template-id, an entry of `entries_finding` (§18.12.1).
  So the entry is the template's in each context: qualified and
  unqualified, through a using-directive (`udir=`, §18.7.3) or a
  using-declaration; a type context, a template argument (`n::I<n::v<T>>`),
  a default template argument, an unevaluated operand, a
  requires-expression; a concept-id in a requires-clause, a nested
  requirement, a type-constraint, a constrained placeholder and a
  return-type-requirement; nested template-ids (`n::v<m::w<T>>`: `w` and
  `v` each name their template, `w`'s entry first). A member variable
  template of the current instantiation ([temp.dep.type]/1) follows the
  rule §18.12.2 gives members of the current instantiation: class member
  lookup at the definition found it, and that result is final
  ([temp.dep.type]/6) in a class without a dependent base, and also in
  one with a dependent base when the class declares the name itself:
  class member lookup returns the declaration set of a class that
  declares the name, before any base, dependent or not, so a base's
  member of the same name is hidden ([class.member.lookup]/2; WP-3450).
  So `mw<T>`, `S::mw<T>`, `this->mw<T>` and `this->template mw<T>` name
  the template (the last two with `MEMBER`; a using-declaration's shadow
  in `found`), in a nested class of a class template too. A
  using-declaration in the class of a non-dependent base's member is a
  declaration of the class (`using P::pw;`: `this->pw<T>` names `P::pw`,
  `found` the shadow); one of a dependent base's member (`using
  B<T>::mw;`) is not, as only the instantiation resolves it. Only when
  the class does not declare the name and has a dependent base, which
  could supply it, does a `this->` name stay `DEPENDENT` with its found
  set and `+base` (the instantiation looks it up again). A member function template stays an overload set
  (`this->template mf<T>()`: `DEPENDENT`, its found set), and a member
  template of an unknown specialization (`t.template w<T>` with `t` a
  `D<T>`) has no definition-context lookup. A partial specialization's
  member is its own. An entry that names its template has no
  per-specialization rows (§18.9): each specialization is that template's,
  and a client that needs the specialization forms it from the
  template and the arguments.
- **PCH and modules.** Stage 5 (§18.10) carries both tables in PCH
  snapshots and module images: a TU restored from a snapshot continues
  the prefix's table, and an importer built from an image merges the
  module unit's entries, as a replay of the unit records them. A
  source whose names could not be carried (recording was off when its
  snapshot or image was made) is listed by `unrecorded()`, never a
  silent gap.

### 18.6 The dump

`-fkiln-dump-written-refs` (with `-fsyntax-only` or any compiling mode)
prints, after diagnostics' end of the TU, one line per entry:

```
<line>:<col>-<line>:<col>  <role>  '<spelling>' -> <entity> [found=<decl>] [q=<line>:<col>|q=::[<line>:<col>]|q=specifier[:<line>:<col>]] [tilde=<line>:<col>] [close=<line>:<col>] [args=<line>:<col>-<line>:<col>] [in=<enclosing>] [shared=<decl>,...] [dep] [rewritten] [reversed] [exp=<line>:<col>] [how=<how>] [udir=<ns>@<line>:<col>] [adl-ns=<ns>,...] [adl-cls=<class>,...] [adl-enum=<enum>,...]
```

Positions are the spelling locations of `lo` and `hi` (the end column
is one past the name's last character), prefixed with the file's base
name and `:` when not in the main file. `<spelling>` is the source text
of the range. The fields after `exp=` are the lookup provenance
(§18.7.5). `<entity>`, `found=` and `in=` print the declaration's
qualified name (lambdas as `(lambda)`; a function or template
parameter unqualified) and `@<line>:<col>` of its location; no entity prints `(dependent)` or `(unresolved)`; `in=(tu)`
for the translation unit. `q=` names the qualifier entry by its start
position; `q=::` is followed by the position of the leading `::` when
it was recorded (always, since WP-2326; §18.11.2), `q=specifier:` by
the first token of a `decltype(...)::` or splice specifier (WP-2828),
and `tilde=` gives the `~` of a destructor name; `close=` the `]` or `)`
of a subscript or call operator entry, and `rewritten` and `reversed`
its flags (§18.13); `args=` the `<` and `>` of a friend function
template specialization's template argument list (§18.11.2);
`shared=` the rest of the declarator group that holds a `GROUP` entry
with `in=` (§18.3.1). A `static_assert` prints `static_assert@` and its
keyword's position. A
ud-suffix's position is a column inside its
literal token. `exp=` is the outermost expansion point of a name written in
a macro. The harness directive `written-refs: <golden>`
(spec/harness.md) compares this dump with a golden file. The developer
switch `KILN_WREF_RAW`, read from the run's environment (spec/driver.md
"Front-end library entry"; `WrittenRefs::set_raw_dump`), prefixes each
line with `#<raw> `, the raw location of the name's start.

Tests: `crates/kiln_ast/src/written_refs.rs` (entry size, off records
nothing, claims, truncation), `crates/kiln_parse/tests/written_refs.rs`
(off and on, `enclosed_by` on stage-2 and stage-3 entries),
`crates/kiln_parse/tests/written_refs_accounting.rs` (§18.8),
`tests/conformance/written_refs/` (one per role family of every stage,
the worked example, macros, tentative parses, overload patching,
dependent names, error recovery; stage 3: `using-decls`,
`using-directives`, `redecls`, `elaborated`, `spec-targets`,
`concepts-decl`, `designators`, `guides`, `decl-macros`,
`decl-dependent`, `decl-tentative`, `decl-errors`; stage 4: the
`inst-*` tests, §18.9.7; WP-3008: `friend-spec` (friend function
template specializations matched, dependent and never instantiated),
`friend-member` ([temp.friend]/5 friends of a primary's and a partial
specialization's members), `spec-target-overloads` (an explicit
specialization and instantiation of the second of two overloaded
templates), `var-template-id` (variable template-ids with dependent
arguments); WP-3405: `template-id-dependent-args` (every context and
form of §18.5's rule, nested ids, function template-ids' found sets,
`T::template x<int>`), `template-id-current-inst` (members of the current
instantiation, with and without a dependent base, an unknown
specialization, a partial specialization's member);
WP-3450: `this-member-dependent-base` (the class's own member variable
template with a dependent base, with and without `template`, hiding a
dependent base's, in a nested class; using-declarations of a dependent
and a non-dependent base's member; data and static members; overload
sets the class or a non-dependent base declares),
`template-id-concepts`, and the carriage goldens `template-id-pch`,
`template-id-pch-include`, `template-id-module-image` and
`template-id-module-replay`; `crates/kiln_parse/tests/written_refs.rs`
`rename_claims_dependent_template_ids` and `label_uses_and_decl_keys`;
stage 6: the `operators-*` tests and `udl`, §18.13.9; WP-3688: `labels`,
`labels-module-image`, `labels-module-replay`, and
`crates/kiln_parse/tests/decl_extents.rs`
`label_decl_extents_forward_local_and_attributed`).

### 18.7 Lookup provenance (WP-2130)

How each written name was found: the lookup kind, the using-directive
an unqualified or qualified lookup went through, and the associated
namespaces and entities of an argument-dependent lookup. Tooling clients
read it to make a first edit right (a rename across a using-directive,
or an unqualified call that argument-dependent lookup resolves, is
edited differently); it is an observation of lookup and never changes a
lookup result.

#### 18.7.1 Contract

- **Same switch as the table.** Recorded only when
  `ParseOptions::written_refs` is on. Off, §18.1 holds unchanged: one
  branch per recording site, nothing allocated (the provenance walk is
  never called, argument-dependent lookup collects nothing extra), and
  snapshots and images are as §18.1 says.
- **The entry stays 28 bytes**: `how` is one byte in the entry's former
  padding (§18.2). The using-directive and the argument-dependent set
  live in a sparse side table keyed by `WrittenRefId`, with the sets
  interned (one copy per distinct set), so an entry with neither costs
  nothing extra. Carried with the entries in snapshots and images
  (§18.10); `truncate` drops the side rows of the entries it drops.
- **Read access**: `WrittenRef::how`, `WrittenRefs::using_directive(id)`
  (the directive's `DeclId`), `WrittenRefs::adl_set(id)` (an `AdlSet`:
  `namespaces()`, `classes()`, `enums()`), `WrittenRefs::adl_set_id(id)`
  (equal ids are equal sets).
- **Where the facts come from.** Never from Sema state set by one
  function and read by another. The scope walk and the qualified
  namespace search return their provenance as a value beside their
  result (`Sema::lookup_name_provenance` and `lookup_qualified_walk`: the
  walks of `lookup_name` and the qualified search, with the
  directive each nominated declaration came through and the class scope
  the walk stopped in); argument-dependent lookup returns its
  associated set and the functions it found that ordinary lookup had
  already found (`AdlFacts`) to `resolve_unresolved_call`, which applies
  them to the call's entry once overload resolution has chosen. The
  facts are taken at the parser sites §18.1 and §18.4 already use, when
  the entry gets its entity (when it is made, when overload resolution
  patches it, when a mem-initializer-id is filled), so they never come
  from instantiation or from implicit names. The walk is re-run at the
  recording site with the found declaration's name, because the sema
  actions that build a name expression return the expression, not their
  lookup result: it is the same pure (`&self`) function over the same
  scopes, and its answer is used only when it found the same entity (by
  first declaration), so a disagreement leaves the default `how` and no
  directive rather than a wrong answer.

#### 18.7.2 `how`

`RefLookup` (`#[repr(u8)]`; dump spelling in parentheses):

| value | meaning |
|---|---|
| `None` (`none`) | no entity: a dependent name or a failed resolution (`flags` tell which) |
| `Unqualified` (`unqualified`) | the scope walk ([basic.lookup.unqual]) |
| `Qualified` (`qualified`) | lookup in the scope a nested-name-specifier names, a leading `::` included ([basic.lookup.qual]) |
| `Member` (`member`) | class member access, `.` or `->` (an `operator->` chain included), with or without a nested-name-specifier of its own ([basic.lookup.qual.general]/2, [class.member.lookup]) |
| `Adl` (`adl`) | the selected function was found only by argument-dependent lookup ([basic.lookup.argdep]) |
| `Both` (`both`) | ordinary lookup and argument-dependent lookup both found the selected function |
| `Builtin` (`builtin`) | the entity is an implicit builtin function declaration (`__builtin_*`, a library builtin used before its declaration) |
| `Injected` (`injected`) | lookup found an injected-class-name ([class.pre]/2, [temp.local]/1): a class's own name inside it or a class derived from it |

The rule, per role that stages 1 and 2 fill:

- **`Expr`, `Concept`**: `Unqualified` or `Qualified` by the qualifier
  link; a call's name is refined to `Adl` or `Both` (§18.7.4); an
  implicit builtin is `Builtin`. A lambda capture is `Unqualified`.
- **`Member`** and a member template's **`Template`** entry (`a.template
  f<int>()`): `Member` (the `MEMBER` flag). The components of a
  nested-name-specifier written inside the member access (`a.B::m`)
  are `nns` entries with their own rule.
- **`Type`, `Template`, `Nns`**: the scope walk or the qualified lookup
  ([basic.lookup.general]/4 type-only for a type-specifier,
  [basic.lookup.qual.general]/1 for a component before `::`):
  `Unqualified` or `Qualified`, `Injected` when what was found is the
  injected-class-name of the class whose scope the walk stopped in or of
  one of its bases (`S` inside `S`'s members, `S<T>` inside the class
  template `S`, `B` inside a class derived from `B`), or, after a
  qualifier naming class `C`, `C` itself or a base of `C`. A
  pseudo-destructor's type name (`p->~T()`) is looked up as a type
  ([expr.prim.id.dtor]): `Unqualified` or `Qualified`.
- **`Base`**: the type-only lookup of [class.derived.general]/2 from the
  class-head: `Unqualified`, `Qualified` or `Injected` by the same rule.
- **`MemInit`**: [class.base.init]/2 looks an unqualified identifier up
  in the scope of the constructor's class first: a member is
  `Unqualified`; a base or, for a delegating constructor, the class
  itself is found as its injected-class-name, so `Injected`; a qualified
  one is `Qualified` (or `Injected` by the qualified rule).

The rule per role that stage 3 fills (WP-2131), by the same mechanism
(the walk re-run at the recording site with the lookup kind the
construct uses):

- **`UsingDecl`**: `Qualified` (a using-declarator always has a
  nested-name-specifier, [namespace.udecl]/1); the walk is the ordinary
  qualified lookup ([basic.lookup.qual]), so a namespace member found
  through a using-directive of the nominated namespace names the
  directive ([namespace.qual]/2).
- **`UsingDirective`**, **`NamespaceAlias`**: `Unqualified` or
  `Qualified` by the qualifier link, the walk considering namespace
  names only ([namespace.udir]/1, [namespace.alias]/1), and a directive
  when a namespace was found only through another using-directive.
- **`Redecl`**, **`SpecTarget`**: `Unqualified` or `Qualified` by the
  qualifier link; an unqualified explicit specialization or
  instantiation inside a namespace finds its template by the scope
  walk, so a directive is named when one brought the template in.
- **`Elaborated`**: `Unqualified` or `Qualified` by the link, the walk
  considering tag names for a class or enumeration ([dcl.type.elab]/2,
  [basic.lookup.elab]) and ordinary names for a friend function.
- **`Concept`** (a type-constraint): as stage 1's: `Unqualified` or
  `Qualified`, with the directive rule.
- **`Designator`**: `Member` (the `MEMBER` flag): the member is found
  in the aggregate's class, as a member access's.
- **`Template`** of a deduction guide: `Unqualified` (the guide is
  declared in the class template's scope, [temp.deduct.guide]/3).

Every entry without an entity is `None`.

#### 18.7.3 The using-directive

`using_directive(id)` is set when the unqualified walk or a qualified
namespace lookup found the entry's declaration only because a
using-directive nominated the namespace it is a member of
([namespace.udir]/2, [namespace.qual]/2), and names that directive:

- the scope where the walk stopped (or the qualifier's namespace) gives
  the declaration from its own members first: then there is no
  directive, even when a directive also nominates a namespace with the
  same declaration;
- a **block-scope** directive, one in a **namespace** and one at **file
  scope** are recorded the same way; the walk reaches each one where
  [namespace.udir]/2 makes its namespace's members visible;
- a **transitive chain** (`using namespace b;` where `b` contains `using
  namespace a;`, [namespace.udir]/4): the innermost link, the directive
  that nominates the namespace the declaration is a member of (`using
  namespace a;` in `b`). When two chains reach the namespace, the one
  the walk reached it through first (kiln's visible-directive order:
  innermost scope first, each scope's directives in declaration order,
  a nominated namespace's own directives right after it);
- an **inline namespace** is not a directive: a member of an inline
  namespace of `N` is found as a member of `N` ([namespace.def.general]/7),
  so `using namespace N;` reaching `N::v1::x` names that directive, and
  `x` written inside `N` names none. A directive written in an inline
  namespace of `N` counts as `N`'s;
- a **using-declaration** is not a directive: `using a::f; f();` has the
  using-shadow in `found` (§18.2) and no directive. A using-declaration
  in a nominated namespace (`namespace b { using a::f; } using namespace
  b; f();`) has both: `found` the shadow, the directive the one naming
  `b`;
- a member found in a class scope, a local name, a template parameter
  and an injected-class-name never have one.

The directive is found from the namespace whose directives nominated the
declaration's namespace: the namespace-scope ones by its member lists
(every reopening, through linkage specifications and inline namespaces,
also when restored from a snapshot), the block-scope ones by the
block's scope; looked up once per (namespace, nominated) pair and
cached in the side table.

#### 18.7.4 Argument-dependent lookup

A call whose callee is an unqualified name that argument-dependent
lookup applies to ([basic.lookup.argdep]/1: not suppressed by a class
member, a block-scope function declaration or a non-function found by
ordinary lookup, nor by parentheses) gets, once overload resolution has
chosen:

- `how`: `Adl` when the winner (its template for a specialization) is
  one only argument-dependent lookup found; `Both` when ordinary lookup
  found it and argument-dependent lookup found it again (kiln compares
  by first declaration, as the candidate set does); else it keeps the
  ordinary `Unqualified`. For `Both`, `found` and the using-directive
  describe ordinary lookup's finding, because ordinary lookup alone
  already makes the call valid: an edit that keeps that path keeps the
  call, while the argument-dependent path depends on the arguments'
  types;
- `adl_set(id)`, whatever `how` is (also `none`, when overload
  resolution failed after the lookup ran): the associated set
  ([basic.lookup.argdep]/3) after the associated class template
  specializations were instantiated. Namespaces are primary contexts
  (the translation unit for the global namespace), classes and
  enumerations first declarations. The order is discovery order: the
  arguments left to right; within an argument's type, an entity before
  what it brings in (a class, then the class it is a member of, then
  the entities of its template arguments, then its bases, depth first
  in base-specifier order; an enumeration, then the class it is a
  member of); each class's and enumeration's innermost enclosing
  namespace when the class or enumeration is added; an inline namespace
  followed by its enclosing namespace, a namespace by its inline
  namespaces (/3, last sentences); every entity once. A pointer to
  function adds its parameter and return types' entities, a template
  specialization its own template arguments' (not a base's), an
  overload-set argument every candidate's;
- a **hidden friend** adds nothing to the set: it is found because its
  class is an associated class (/4.2) and is invisible to ordinary
  lookup, so a call that selects it is always `Adl`;
- an operator expression (`a + b`) has no written name and no entry
  (§18.4), so its argument-dependent lookup is not recorded; the
  operator-function-id written as a call (`operator+(a, b)`) is an
  `Expr` entry like any call.

A dependent call in a template pattern performs its argument-dependent
lookup at instantiation: its main-table entry has `None` and no set;
each specialization's resolution, with its `how` and its set, is a
per-specialization entry (§18.9.4).

#### 18.7.5 The dump

After `exp=` (§18.6), in this order and each only when present:

- `how=<how>` when `how` differs from the default the line already
  shows: `none` for no entity, `member` with the `MEMBER` flag,
  `qualified` with a `q=`, else `unqualified`. So the dump prints `adl`,
  `both`, `builtin` and `injected`, and any disagreement with the
  default;
- `udir=<ns>@<line>:<col>`: the nominated namespace's qualified name at
  the directive's location;
- `adl-ns=`, `adl-cls=`, `adl-enum=`: the set's namespaces (`::` for the
  global namespace), classes and enumerations, comma-separated, each
  printed as `<entity>` is, except that an unnamed class or enumeration
  prints as `(anonymous)@<line>:<col>` (its location names it); a list
  that is empty is not printed, and an empty set prints `adl-ns=` alone
  with nothing after it.

#### 18.7.6 Cost

Off, the recording sites test one `Option` and nothing is allocated;
on, provenance costs one re-run of the pure lookup walk (§18.7.1) per
resolved name that names a class (as a type, template, base, nested
name or mem-initializer) or a namespace member while a using-directive
exists, plus the side rows. Measured on aarch64 with the harness
profile, five interleaved rounds, medians (base = WP-2120):

- `<bits/stdc++.h>` `-fsyntax-only`: off 1.14 s / 91.8 MB against base
  1.16 s / 91.7 MB; on 1.38 s / 151.5 MB against base-on 1.30 s /
  151.0 MB. 204 820 entries; the table grows from 5 879 696 to
  5 898 456 bytes (+18 760, 0.09 bytes per entry; 6 422 lines print a
  `how=`, 103 a `udir=`, 505 an ADL set).
- `SemaExpr.cpp` (LLVM) `-fsyntax-only`: off 5.77 s / 311.5 MB against
  base 5.85 s / 311.1 MB; on 6.38 s / 425.3 MB against base-on 6.27 s /
  422.8 MB. 345 918 entries; the table grows from 9 768 200 to
  10 015 680 bytes (+247 480, 0.72 bytes per entry; 19 539 lines print a
  `how=` (15 883 `injected`, 3 074 `both`, 529 `builtin`, 53 `adl`),
  4 116 a `udir=`, 4 452 an ADL set).

Off, the tracked memory totals are byte-identical to the base and the
time and peak RSS are within noise. Both dumps are identical to the
base's once the provenance fields are removed: provenance does not
change what lookup finds.

Tests: `crates/kiln_ast/src/written_refs.rs` (the entry is still 28
bytes, sets are interned, truncation drops side rows, off records
nothing), `tests/conformance/written_refs/provenance-*.cpp` (each `how`
value; using-directives in a block, a namespace and at file scope, a
transitive chain, an inline namespace, a using-declaration; ADL through
a class-type argument, an enumeration, a pointer to function, a template
specialization's arguments and a hidden friend, with each set derived
from [basic.lookup.argdep]/3 in a comment; a call both lookups find;
an operator-function-id call; a qualified name through a namespace
alias; a member found in a base; unresolved and dependent names).

### 18.8 Identifier accounting (WP-1932)

A completeness check over whole translation units, the basis of stage
5's completeness test (WP-1934): `crates/kiln_parse/tests/written_refs_accounting.rs`
parses a file with the table and the declaration-extent table (§19) on,
preprocesses it again on its own, and takes every identifier token of
the preprocessed stream whose spelling is in the main file. Each must be
one of:

- **an entry's range**: covered by the range of some entry (its
  spelling location);
- **a declaration's own name**: the first identifier of a §19 row's
  `name` span (the declared name, not its template arguments);
- **a documented non-name**: a keyword or builtin trait keyword
  (`kiln_parse::keywords::Keywords`), a contextual keyword (`final`,
  `override`, `pre`, `post`, `import`, `module`), an alternative token
  ([lex.digraph]: `and`, `bitor`, ...), an attribute name (an
  identifier at an attribute list's own level in `[[...]]`,
  `__attribute__((...))` (or GCC's `__attribute((...))`),
  `__declspec(...)` or the attribute operand of
  `__builtin_has_attribute(e, ...)`, or before `::` in one), an
  attribute's keyword argument (a lone identifier argument of an
  attribute that reads it as one of its own words, clang
  `attributeHasIdentifierArg` / `VariadicIdentifierArgument`:
  `mode(byte)`, `format(printf, 1, 2)`, `cpu_dispatch(atom)`,
  `enum_extensibility(open)`, `visibility`, `pcs`,
  `zero_call_used_regs`, `function_return`, GCC's
  `access(read_only, 1)`; and the builtin a builtin
  alias names, `__clang_arm_builtin_alias(__builtin_x)`), an argument of
  an attribute clang does not know (one kiln warns `unknown attribute
  '...' ignored` about, WP-2290): every identifier of an unknown
  `[[...]]` attribute's argument list, whose tokens clang skips, and a
  lone identifier first argument of an unknown GNU attribute
  (`__attribute__((x(mu)))`), which clang takes as an identifier
  argument (`ParseAttributeArgsCommon`), looking nothing up (a later
  argument of an unknown GNU attribute is an expression and counts),
  a label declaration (a `LabelStmt` definition `l:`, located from the
  AST rather than any identifier followed by `:`, WP-3699; the names of a
  `__label__` declaration; label uses `goto l`, `&&l`, and `asm goto`
  label operands are §18 `label` entries, WP-3688),
  an `asm` symbolic operand name (`[name]`), or a module name (after
  `module` / `import` up to `;`).

Macro names, directive text, `#pragma` text and the text of skipped
conditional blocks never reach the parser and are not counted. A file
with an error is skipped (its recovery records what it can, §18.4).
The default test runs over `tests/conformance/written_refs/` and a
pinned list of `tests/conformance` directories (`PINNED`: lookup,
classes, friends, virtual functions, list-initialization, lambda
captures, using-declarations, deduction guides, dependent types,
explicit specializations, instantiation, template names, variadics and
`templates/`: 733 files without errors, 58989 identifiers, about 5
seconds in a debug build) and wants nothing
left over; `KILN_REFS_DIR=<dir>` (an ignored test) runs it over a
directory tree and prints what is left grouped by the token before it,
`KILN_REFS_STRICT=1` making any leftover fail. A second ignored test
(WP-1934) accounts the host's libstdc++: a TU that includes
`<bits/stdc++.h>`, with the driver's `-std=c++26` search list,
predefines and feature queries, every identifier of every header the
parser sees counted (`KILN_REFS_TU=<source>` replaces the TU).

Over `tests/conformance` (WP-1932: 3388 files without errors, 311138
identifiers) the run found 2381 identifiers left over. The table gaps
it found were fixed there: the names of `sizeof...(pack)` (stage 1),
offsetof member-designators, and §19 rows for for-range-declarations
and expansion-statement variables and their bindings, a condition's
bindings, exception declarations, requires-expression parameters and
init-captures, and friend declarations that overwrote the row of the
class or member they name. WP-1934 fixed two of the three classes left
(82 identifiers): the type-names of a qualified pseudo-destructor-name
(`p->I::~I()`, `p->::N::I::~I()`, `t->T::~T()`,
`p->A<int>::~A<int>()`: 40) and the pack of a pack-indexing
nested-name-specifier (`typename Ts...[0]::type`: 10), §18.4. Run
again (WP-1934: 3404 files, 311991 identifiers), 32 are left, a row and
an ICE of their own:

- names in the arguments of thread-safety attributes
  (`guarded_by(mu)`, `requires_capability(mu)`, `acquire_capability(...)`:
  30), which kiln knows and skips unanalysed (WP-606), so their
  names were never looked up; recorded since WP-2170 (below);
- two diagnosed files on which the test's in-process front end panics
  in the Oven's `convert` (an integer to an `Opaque` type: WP-2141's
  ICE), reported as `panic` lines, not identifiers.

Run again after WP-2170 (WP-2220: 3412 files, 312994 identifiers), 8
are left: the two panics, and 6 identifiers in the arguments of
attributes clang does not know (`frobnicate_lock(mu)`,
`[[guarded_by(mu)]]`, `[[clang::guarded_by(mu)]]` in
`dcl.attr.grammar/thread-safety-attributes-unknown.cpp`). Those are
not names: clang takes a lone identifier argument of an unknown GNU
attribute as an identifier argument and skips an unknown `[[...]]`
attribute's tokens, looking nothing up, so the table rightly has no
entry; since WP-2290 the test counts them as non-names (the documented
set above). Run again (WP-2290: 3481 files, 318645 identifiers),
nothing is left (the two panics are gone since WP-2173). Narrowing the
label-definition excuse from any `ident :` token to actual `LabelStmt`
locations in the AST (WP-3699; `ident_before_colon_excuse_only_covers_label_defs`)
exposed 0 leftovers across `tests/conformance` (3657 files, 334160
identifiers) and host libstdc++ `<bits/stdc++.h>` (387763 identifiers). The
pinned set is unchanged (733 files, 58989 identifiers, nothing left).

Over the host's libstdc++ (WP-1934: GCC 14's headers on aarch64 Linux
through `<bits/stdc++.h>`, 387763 identifiers) nothing is left. The
first run left 13, all non-names, now in the documented set above:
GCC's `__attribute((...))` spelling (`__abi_tag__`, `__unused__`: 8)
and the access mode of GCC's `access` attribute (`__read_only__`,
`__none__`: 5).

**Thread-safety attribute arguments (WP-2170).** While the table
records, the arguments of the GNU thread-safety attributes (clang's
`LateParsed` set: `guarded_by`, `pt_guarded_by`, `acquired_after`,
`acquired_before`, `lock_returned`, `locks_excluded`, the
`*_capability` and `*_lock_function` / `*_trylock_function` /
`*_locks_required` / `assert_*_lock` families, 26 names) are captured
at their `(` up to the matching `)` (the tokens the off path skips)
and parsed for their names alone: a comma-separated list of string
literals and assignment-expressions, unevaluated, with every
diagnostic they produce dropped, and the attribute keeps no argument
either way, so nothing is analysed and nothing else changes. When
they are parsed is clang's: an attribute in a class's
member-specification once the class is complete (after the member
function bodies of the outermost class, its entries claimed by the
member it belongs to, a member function's in its scope with `this`),
so it names members declared after it; one on a function declarator
elsewhere with its parameters in scope; any other where it is
written. The `[[clang::x(...)]]` spellings are parsed where they are
written already (clang does the same) and are unchanged. A name the
arguments' lookup does not find is the entry an undeclared name gets
anywhere else (§18.2, WP-2291): `UNRESOLVED`, printed `(unresolved)`,
and so is a member name after it (`guarded_by(m.x)` with `m`
undeclared); kiln gives no diagnostic, where clang 19 and 22 reject the
undeclared identifier (the arguments are not analysed, WP-606). A
dependent argument of a class template's member (`guarded_by(T::mu)`)
is `DEPENDENT`. The forms clang accepts with a
`-Wthread-safety-attributes` warning record what lookup finds, as any
name does: an argument naming a variable whose type is not a
capability (`guarded_by(i)` with `int i`), a `pt_guarded_by` of a
member that is not a pointer; a string literal argument
(`guarded_by("mu")`, which clang ignores as invalid) contains no name
and has no entry. No pattern node is remembered for them (§18.9.4):
no instantiation rebuilds an attribute's arguments, so a dependent one
has no rows, and a module image carries the entries without one.
Off, the arguments are skipped as before: the test is one branch at
the attribute's `(`.
`tests/conformance/written_refs/attr-thread-safety.cpp`, the API
test `thread_safety_arguments_are_entries` and the carry test
`thread_safety_arguments_keep_the_image`.

### 18.9 Stage 4: per-specialization entries (WP-1933)

In a template pattern a dependent name has one main-table entry with no
entity (§18.5). What it names is decided per specialization, at
instantiation ([temp.res.general]/1, [temp.dep.candidate]), and a
tooling client renaming `Vec::size` must also edit the `t.size()` of a
template instantiated with `Vec`. Stage 4 records, for each
specialization, what each dependent name of the pattern resolved to, in
a separate table: the main table keeps one entry per written name.

#### 18.9.1 Contract

- **The table.** `WrittenRefs::specs()` yields the rows (`SpecRef`, 32
  bytes, a `size_of` unit test), keyed by (pattern entry,
  specialization):

  | field | type | meaning |
  |---|---|---|
  | `entry` | `WrittenRefId` (4) | the pattern's main-table entry: `DEPENDENT`, no entity; or a member qualifier's component the definition resolved and the instantiation looked up again in the object's class (below) |
  | `spec` | `DeclId` (4) | the specialization whose instantiation resolved the name (§18.9.2) |
  | `entity` | `Option<DeclId>` (4) | what the name denotes there, as §18.2's `entity` (a function template's specialization, a class's first declaration, ...); `None`: unresolved (§18.9.3) |
  | `found` | `Option<DeclId>` (4) | what lookup found, when it differs from `entity` (the template of a specialization) |
  | `adl` | `Option<AdlSetId>` (4) | a dependent unqualified call's associated set in this specialization (§18.7.4), interned with the main table's sets |
  | `udir` | `Option<DeclId>` (4) | the using-directive ordinary lookup found `entity` (or `found`) through, as §18.7.3's: `None` when lookup found it directly, by argument-dependent lookup only (`how` `Adl`), or by qualified or member lookup |
  | `naming_class` | `Option<DeclId>` (4) | the lookup-context (naming) class ([class.access.base]/5, canonical first declaration via `Ast::first`) for a resolved `Member` or `Qualified` lookup in a class, including dependent member/class-scope operators (§18.13.6); `None` for unresolved, built-in, unqualified, ADL, namespace-qualified and enum-qualified rows (WP-3695) |
  | `flags` | `u8` | `MEMBER` (the entry's), `UNRESOLVED` without an entity; `REWRITTEN` and `REVERSED` for a dependent operator's resolution there, and `SPEC_BUILTIN` for one that selected a built-in candidate or applied no operator (no entity, `how` `builtin`, dumped `(builtin)`; §18.13.6) |
  | `how` | `RefLookup` | §18.7.2's `how` of the instantiation's lookup: `Member`, `Qualified`, `Unqualified`, `Adl`, `Both`, `Builtin`, or `None` without an entity |

  Not recorded per specialization: `Injected` (an injected-class-name
  names the pattern's class, which is not dependent).
- **Member qualifiers looked up again.** In `t.acme::B::f()` on a
  type-dependent object, the definition finds `acme` by unqualified
  lookup only, and the pattern's `acme` and `B` entries keep that
  resolution (namespace `acme`, `acme::B`). A specialization whose
  object's class has its own `acme` binds the qualifier there
  ([basic.lookup.qual.general]/2-3, spec/sema-templates.md "Dependent
  forms"): each component it formed so (`TemplState::relooked_nns`)
  gets a row on its resolved pattern entry when it names something else
  than the entry (`W::acme`, `W::acme::B`), with the member's row
  (`WB::f`). A specialization whose class has no `acme` adds no
  qualifier rows (WP-3406;
  `tests/conformance/written_refs/spec-member-qualifier-object-class.cpp`).
- **The using-directive.** Ordinary unqualified lookup of a dependent
  name runs at the pattern's definition ([temp.dep.candidate]/1,
  [temp.res.general]/1); only argument-dependent lookup runs at the
  point of instantiation. So the using-directive a row's declaration was
  found through is decided at the definition: when the pattern's entry
  is made, each namespace-scope candidate of the lookup is walked once
  (§18.7.3's walk, the scopes of the definition) and the directive it
  was found through, if any, is remembered per (entry, candidate); a
  row whose `entity` (or `found`, for a specialization of a found
  template) is one of them gets that directive. A dependent qualified
  or member name looks its name up in a class at instantiation (a
  namespace is never dependent) and gets none. A row whose `how` is
  `Both` keeps the directive of its ordinary half.
- **Kept once.** A row with the same content as a live row is not added
  again (a name rebuilt by two paths for one specialization, an alias
  template substituted twice with the same result). `naming_class` is
  part of a row's content (its `Eq`/`Hash` key), so two lookups resolving
  to the same inherited member through distinct lookup-context classes
  (for example `A<Der1>` and `A<Der2>` on an alias template) stay
  distinct rows. One entry can have several rows for one key where the
  key is a template (§18.9.2: an alias template, a default template
  argument, a concept), one per distinct resolution.
- **Order independence.** The set of rows does not depend on the order
  in which the specializations are instantiated; only the order of
  `specs()` (recording order) does. Clients and tests compare the rows
  sorted: the dump prints each entry's rows sorted (§18.9.5), and the
  API test compares sorted (pattern position, entity position,
  specialization position) triples. The dump prints a row's
  specializations (its key, its entity and `found`) with their
  canonical template arguments, scoped (§18.9.5), so two orders compare
  equal however each spelled the arguments (an alias, a
  using-declaration, a qualified name: [temp.type]/1 makes them one
  specialization).
- **Off.** Recording sites test one map, empty when off
  (`has_pattern_nodes`, §18.9.4): nothing is allocated and nothing is
  recorded; §18.9.6 measures it.
- **Read access**: `specs()` (live rows with their `SpecRefId`),
  `spec(id)`, `spec_len()`, `specs_of(entry)` (an entry's rows),
  `specs_for(spec)` (a specialization's rows), `specs_naming(d)` (rows
  whose `entity` or `found` is `d`), `specs_by_entry()` (one pass, for
  clients that query many entries), `spec_adl_set(&row)`. A rename of
  `d` edits the ranges of the main-table entries naming `d` and of the
  pattern entries of `specs_naming(d)`'s rows, and the entries whose
  found set has `d` (`entries_finding(d)`, §18.12.1), which also cover
  templates no specialization of which is instantiated.
- **Snapshots and images.** Carried with the main table (§18.10).

#### 18.9.2 The specialization

The key is the innermost instantiation the lookup runs for, read off the
instantiation stack (§8.4) from the innermost frame:

- a function template specialization's definition, implicit or by an
  explicit instantiation ([temp.explicit]/10): the specialization
  (`measure<Vec>`);
- a member of a class template specialization instantiated on use or by
  an explicit instantiation (a member function's definition, a static
  data member's initializer, a member class or enumeration,
  [temp.inst]/3-5, [temp.explicit]/11): that member of the
  specialization (`Table<Cfg>::lookup`); the members' declarations,
  instantiated with the class (a return type, a member typedef), and
  the class's other names: the class template specialization
  (`Table<Cfg>`);
- a function template's signature substituted with deduced or explicit
  arguments ([temp.deduct.general]/5): the specialization it forms
  (`len<Sized>`), which `get_function_spec_signature` sets as the key
  of the substitution it runs;
- a variable template specialization's initializer ([temp.inst]/7): the
  variable template specialization (`scale_of<Feet>`);
- an alias template's aliased type: an alias template specialization is
  not a declaration ([temp.alias]/2), so the key is the alias template,
  one row per distinct resolution;
- a default template argument: the template whose parameter it is,
  except a function template's default that a deduction uses
  ([temp.deduct.general]/5): the specialization the deduction forms
  (`call<n::S, 4>`). That default is substituted before the
  specialization exists, so its rows are keyed by the template until
  the deduction ends; `deduce_spec` then re-keys them to the
  specialization it formed, or drops them when it failed
  (`end_written_spec_deduction`, §18.9.3). A call with the argument
  written explicitly substitutes no default and gets no row for it; a
  default function argument used by a call ([temp.inst]/12): the called
  function (`from<P>`, `Holder<P>::pick`); a default member initializer
  ([temp.inst]/4): the field of the specialization;
- a generic lambda's call operator, a member function template of the
  closure type ([expr.prim.lambda.closure]/3): its specialization
  (`outer(const One &)::(lambda)::operator()<Two>`). The enclosing
  template's instantiation rebuilds the lambda's body with the name
  still dependent; the rebuilt node then stands for the same entry
  (§18.9.4) and each call operator specialization resolves it. A name
  the enclosing instantiation resolves (an unqualified call whose
  arguments depend only on the enclosing template's parameters, `f(t)`,
  found by argument-dependent lookup) gets its row from that
  instantiation, keyed as a non-generic lambda's would be;
- a member template of a class template ([temp.mem]/1): its
  specialization, the innermost (`Box<Small>::convert<Wide>`), for every
  name of its body, dependent on the inner parameters or only on the
  outer ones. The client reaches the outer specialization as the key's
  enclosing class (`Ast` decl context: `Box<Small>`). A name in the
  member template's declaration gets a row for the class specialization
  (the class's instantiation substitutes the member template's
  declaration) and one for each member template specialization (its
  signature's substitution).

- a name in a constraint, resolved by satisfaction
  ([temp.constr.atomic]/3: the atomic constraint's expression
  substituted with the satisfying arguments; [temp.constr.sat]): the
  declaration the constraint belongs to, the innermost constraint frame
  (`ConstraintSubstitution`, `ConstraintsCheck`, `ConceptCheck`)'s
  entity. That is the concept for the atoms of its
  constraint-expression, also when they are checked for a constrained
  declaration's `requires Sized<T>` (`Sized`); the constrained
  declaration for the atoms of its own requires-clause or
  type-constraint, a function or class template's templated declaration
  (`len2`), a member of a class template specialization as itself
  (`Box<V>::get`). The satisfying arguments are not in the key:
  satisfaction is a check, not a specialization ([temp.constr.decl]),
  and the rows are one per distinct resolution, as an alias template's.
  The key does not depend on which check reached the atom, so a hit of
  the satisfaction cache or the atomic-constraint cache, which skips
  the substitution, loses nothing: the rows the first check recorded
  have the same key, and "kept once" (§18.9.1) keeps the second
  check's from duplicating them. A requires-expression's requirements
  (`RequirementInstantiation`, and the `ConstraintSubstitution` frame of
  its body) are part of what contains it: of the constraint, or of the
  instantiation of a template whose definition has it (`if constexpr
  (requires { t.size(); })` in `body<V>`). A builtin trait's own check
  (its frame's entity is the translation unit) records nothing.

Frames inside an instantiation that are not one themselves (a lambda's
body substitution, a structured binding's initializer) are transparent.
A frame that is a check, not an instantiation, stops the walk with no
key, so nothing is recorded: partial specialization matching,
default-argument checking, template template argument matching,
building deduction guides, `std::meta::substitute`. A partial
specialization match has no `specs()` key; with keys on it is recorded
in the keyed table, under its `PartialMatch` key (§18.14).

#### 18.9.3 Failures

- **Substitution failure** ([temp.deduct.general]/8) is not an
  instantiation and records nothing. When the SFINAE frame of a function
  template `ft`'s deduction ends (`in_sfinae_held`), the rows it
  recorded keyed by `ft` itself are dropped, and when it failed also
  those keyed by the specializations of `ft` it formed; a lookup that
  fails inside a SFINAE context records no unresolved row. The frame
  of a default template argument the deduction substitutes keeps its
  rows keyed by `ft` when it succeeds, until the deduction ends: they
  become the formed specialization's, or are dropped with a deduction
  that fails at a later step (§18.9.2). Rows belong to the
  specializations that are formed: every candidate whose deduction
  succeeds forms its specialization ([temp.over]/1), and keeps the rows
  of its signature and its default template arguments whether or not
  overload resolution then chooses it; a candidate whose deduction
  fails keeps none, and so does one whose deduction never runs: at
  clang compat 22 a function template candidate named without explicit
  template arguments is deferred while the set may still have a perfect
  match (`overload/defer.rs`, spec/sema-overload.md §4.4), and when a
  non-template candidate is that match the template is never deduced,
  so no specialization of it is formed and its signature records no row.
  At compat 19, or when no candidate is a perfect match, the deferred
  template is deduced and keeps its specialization and rows as above
  (`written_refs/inst-deferred-candidate.cpp`). What the
  real instantiations nested in the deduction recorded stays (a class
  template specialization instantiated while the signature was
  substituted is instantiated, [temp.inst]/2), as do an alias
  template's rows (§18.9.2). A constraint's substitution
  (`ConstraintSubstitution`, `RequirementInstantiation`) is a SFINAE
  context too, but its frame drops nothing: what the satisfaction check
  resolved is recorded for the constraint's declaration whether or not
  the constraint is satisfied and whether or not the deduction around
  it succeeds (a function template's constraints are keyed by its
  templated declaration, not by the template the deduction frame
  drops). A name that fails there records no row.
- **An error** ([temp.res.general]/6: the program is ill-formed) keeps
  what the instantiation resolved: with kiln's error recovery, every
  name its rebuild reached, before the error and after it. The name
  whose rebuild failed, with an error diagnosed since that rebuild
  started and outside a SFINAE context, gets a row with no entity
  (`UNRESOLVED`). A rebuild given up without a diagnostic (a trial
  substitution) records nothing.
- **A reused alias substitution** records nothing again. An alias
  template specialization is a substitution, not an instantiation
  ([temp.alias]/2), and kiln substitutes it once per argument list (the
  sugared converted list at clang compat 22, the canonical one at 19;
  spec/sema-templates.md §4.10): its rows are recorded at that first
  substitution, keyed by the alias template (§18.9.2), and a later use
  of the same specialization takes the cached type and records none. A
  substitution that checked access, or whose error a SFINAE context
  trapped, is not cached and is redone (its rows are kept once,
  §18.9.1). When a declaration between two uses would change what the
  substitution finds (a better function for argument-dependent lookup
  of a dependent call in the aliased type), the program is ill-formed,
  no diagnostic required ([temp.dep.candidate]/1): kiln keeps the first
  result and its rows, as GCC does, where clang substitutes again at
  each use (`written_refs/inst-alias-first-use.cpp`). Default template
  arguments are not cached: each use substitutes them again.
- **A name that stays unresolved** at instantiation gets a row with no
  entity: a dependent call no viable function matched (with the
  argument-dependent set its lookup computed, §18.7.4), a member the
  class does not declare, a dependent type that names nothing.

#### 18.9.4 Recording sites

- **Pattern nodes.** While recording is on, the parse-time sites
  remember the pattern node each `DEPENDENT` entry was made from (the
  map `spec_nodes`, empty when off): `PatternNode::Expr(e)` for a
  dependent expression name (a `DependentScopeDeclRefExpr`, a
  `CXXDependentScopeMemberExpr`, the callee of a dependent call, an
  overloaded member access with dependent arguments) and
  `PatternNode::Type(q, loc)` for a dependent type name (the interned
  dependent type without qualifiers, and the §5.8 written location of
  its name), and `PatternNode::Decl(d)` for a using-declarator with a
  dependent qualifier in a class template (`using T::size;`, `using
  typename T::type;`: `d` is its `UnresolvedUsingValue` or
  `UnresolvedUsingTypename` member), for a friend function
  template specialization with dependent arguments or type (`friend
  void f<>(T);`: `d` is its `FriendDecl`, WP-3008), and
  `PatternNode::Oper(e, slot)`
  for the operator token of a dependent operator expression `e` (§18.13.6;
  slot 1 is a binary fold's second token, else 0), apart from
  `Expr(e)` because a dependent `t->m` is the node of both `->` and
  `m`.
- **Expressions.** `Substituter::expr` and `Substituter::callee` look
  the pattern node up after each rebuild. The rebuilt node's target
  (`written_target`, §18.4) gives the row: a declaration, a concept or a
  type at once; an overload set when overload resolution picks (the
  stage-1 funnels `resolve_written_ref`/`retarget_written_ref`, and
  for an overloaded member call's failure the member call's own); a
  node still dependent (a generic lambda's body, a member template
  substituted with the outer arguments only) becomes a further pattern
  node of the same entry. Whether a rebuilt overload set is still
  dependent is decided by its call, not by the context: in a context
  that is itself a pattern (a constraint, which satisfaction substitutes
  with the template as context; a generic lambda's body, which the
  enclosing instantiation rebuilds) the set is noted as a further node
  and also awaits resolution (`defer_spec`). A call whose arguments are
  no longer dependent resolves it, by ordinary or by argument-dependent
  lookup, which records the row and drops the node
  (`forget_pattern_node`); a call that stays dependent keeps the node
  (`act_on_call`, `written_spec_call_dependent`), and so does a set that
  is not called (WP-3584). A rebuild that fails reports per §18.9.3.
- **Qualifiers.** After a qualified name's row, the dependent
  components of the entry's qualifier chain are paired from the right
  with the substituted nested-name-specifier's components, and each
  gets a row naming its class or namespace (`T::inner::value`: `inner`
  names `A::inner`). A component still dependent is left to a later
  instantiation.
- **Types.** `Substituter::ty_node`'s `DependentName` branch (a
  `typename T::x`, a dependent class template name used as a
  placeholder) and its `DependentTemplateSpecialization` branch
  (`T::template rebind<U>`: the member template) record what the name
  found, with `found` for a specialization of a found template. An
  alias template's aliased type is substituted without §5.8 locations
  (diagnostics point at the alias's `using`); its pattern node is found
  at the written occurrence of the type nearest to that point. A type
  whose qualifier is still dependent after the substitution (a generic
  lambda's body rebuilt by the enclosing template's instantiation:
  `typename U::type`, `typename Pair<T, U>::type`,
  `U::template rebind<T>`) is rebuilt as a new dependent type; that
  type, at the substitution's anchor location, becomes a further pattern
  node of the same entry (`note_spec_still_dependent`), and the call
  operator specialization's instantiation, which substitutes the rebuilt
  type at the same anchor (a rebuilt type has no §5.8 locations of its
  own), resolves it and records the row keyed as §18.9.2 says for a
  generic lambda.
- **Further nodes.** A rebuild that leaves a node still dependent adds a
  further pattern node of the same entry only when the rebuilt node
  differs from the one substituted. A member template's signature and
  body therefore add none: the class's instantiation substitutes its
  declaration with the outer arguments, which gives a name that depends
  on the member template's own parameters back as the same node (no
  node, no row; a name that resolves gets the class specialization's
  row, §18.9.2), and the member template specialization substitutes the
  pattern's declaration (§18.9.2, `Box<A>::conv<B>`). A generic lambda's
  body, which the enclosing instantiation rebuilds, adds a further node
  for each rebuilt node that stays dependent, and records a row for each
  that resolves, whether by ordinary lookup or by argument-dependent
  lookup (`f(t)` with `t` of the enclosing template's parameter type).
  No further node is added for:
  - a rebuild that resolves: it records a row (an overload set, in any
    context, once its call resolves it; see "Expressions");
  - a rebuild that returns the pattern node unchanged (the class's
    instantiation over a member template);
  - a rebuild that fails (§18.9.3);
  - an entry that is not `DEPENDENT` (resolved at the definition);
  - anything while recording is off or pattern nodes are suspended
    (`no_nodes`, §18.8);
  - a type other than through `ty_node`'s `DependentName` and
    `DependentTemplateSpecialization` branches
    (`note_spec_still_dependent`, see "Types");
  - an operator token: `PatternNode::Oper` nodes are noted by the
    operator sites (§18.13.6), not by this rule;
  - a node read from a module image: the importer registers it again
    (`bmi_image/refs.rs`), no rebuild is involved.
- **Argument-dependent lookup.** A dependent unqualified call's row gets
  §18.7.4's `how` and set from the facts `resolve_unresolved_call`
  reports, as the main table's call entries do.
- **One candidate.** A rebuilt call whose lookup found exactly one
  non-template function takes `resolve_unresolved_call`'s direct path,
  which checks the arguments only when `act_on_call` converts them. Its
  row waits until then (`SingleWrittenSpec`, parked by the direct path):
  arguments that convert make the row name the function, as overload
  resolution choosing it would; arguments that do not (the candidate is
  not viable, [over.match.viable]/4) leave the name unresolved, as a
  set of two with no viable function does: no row inside a SFINAE
  context or a constraint, an `UNRESOLVED` row with the
  argument-dependent set after an error (§18.9.3). A call that ends
  before its arguments are converted (a builtin's custom checking, an
  implicit member call) records the function when `act_on_call`
  returns. The other one-candidate paths are not calls (the address of
  a single function, `resolve_single_overload_candidate`) and resolve at
  once.
- **Using-declarations.** The class template specialization's
  instantiation redoes each dependent using-declarator
  (`member_using`, `member_using_typename`) and records its row keyed
  by the class template specialization (§18.9.2: a member's
  declaration): the entity is the one §18.4 gives the declarator
  written with the substituted qualifier, the first declaration it
  introduced (an inherited constructor for `using T::T;`), else the
  first qualified lookup finds; `found` is empty (the using-declaration
  is the declaration). A pack of declarators (`using Ts::operator()...;`)
  gets one row per element. The qualifier's dependent components get
  rows as for any qualified name. A declarator that fails with an error
  gets a row with no entity (§18.9.3); one whose substituted qualifier
  is still dependent (a local class of a generic lambda) makes its new
  `UnresolvedUsing*` member a further pattern node of the entry. The
  names the using-declaration introduces, used in the class's members:
  an expression already finds the instantiated using-declaration's
  shadow (`size()` → `V::size`, `found` the shadow `D<V>::size`); a
  type (`t x;` after `using typename T::t;`) is an `UnresolvedUsing`
  type the substituter replaces by the instantiated declaration's
  type, which records the row the same way (entity the shadow's target,
  `found` the shadow). Such a type has no §5.8 location (no diagnostic
  needs one), so the recording site notes the location of each
  occurrence while recording is on, and the substituter finds the
  occurrence by it as for a dependent name.
- **Friends.** The class template specialization's instantiation
  matches each dependent friend function template specialization
  (`friend_decl`, `instantiate_friend_spec`) and records its row keyed
  by the class template specialization (§18.9.2: a member's
  declaration): the entity is the specialization befriended, `found`
  its template (`Sema::note_written_spec_friend`, WP-3008). One that
  matches no template, with an error, gets a row with no entity
  (§18.9.3). The qualifier's dependent components (`friend void
  Q<T>::f<int>(int);`) get no rows.
- **Using-directives.** The parse-time site of an unresolved
  unqualified name (`UnresolvedLookupExpr`) that remembers its pattern
  node also remembers its candidates' directives (§18.9.1, the map
  `spec_udirs`, empty when off and when no using-directive is in
  scope); `push_written_spec` reads it for an `Unqualified` or `Both`
  row.
- **Constraints.** No site of its own: an atom's substitution and a
  requirement's rebuild are `Substituter::expr` and `ty_node` as
  anywhere, and `written_spec_key` gives them §18.9.2's constraint key.

#### 18.9.5 The dump

After each entry's line (§18.6), its rows, sorted and without
duplicates (so the order of instantiation does not show):

```
<line>:<col>-<line>:<col>  <role>  '<spelling>' -> <entity>|(unresolved) [found=<decl>] spec=<specialization> [naming=<class>] [how=<how>] [udir=<ns>@<line>:<col>] [adl-ns=...] [adl-cls=...] [adl-enum=...]
```

The positions, role and spelling are the entry's. `<entity>`, `found=`
and the `adl-` lists print as in §18.6 and §18.7.5, `udir=` as
§18.7.5's; `spec=` prints the
specialization's qualified name with its template arguments (a class or
variable template specialization's too: `Table<Cfg>`,
`scale_of<Feet>`) and `@<line>:<col>` of its location (the pattern's
declaration). `naming=` prints the row's `naming_class` in the same form
as `spec=`, **only when it differs from the semantic parent of `entity`**
(`Ast::first` of `entity`'s semantic `DeclContext`); a missing `naming=`
in the dump therefore means "the entity's own semantic parent class or
none", whereas the API (`SpecRef::naming_class`, `SubstRef::naming_class`)
always carries the value for every resolved `Member`/`Qualified` class
lookup (WP-3695). On these lines every specialization, in `<entity>`,
`found=`, `spec=` and `naming=` and in their scopes, prints its canonical
template arguments, each type scoped as a canonical type prints
(`measure<lib::Item>`, not `measure<Alias>` or `measure<Item>` as some
use first wrote it; `DiagNames::canonical_args`): a specialization's
own arguments as first written are the first use's, which the order of
instantiation decides. The main table's lines keep the arguments as
written. `how=` appears when it differs from the default §18.7.5
derives from the row (no entity, its `MEMBER` flag, the entry's
qualifier). The lines are path-free as §18.6's are.

#### 18.9.6 Cost

Off, every recording site is one test of the empty pattern-node map
(an expression rebuild also skips reading the error count) and nothing
is allocated. On, the cost is the pattern-node map (one slot per
dependent entry), the rows and the de-duplication index. Measured on
aarch64 with the harness profile, interleaved rounds (base = WP-1932):

- `<bits/stdc++.h>` `-fsyntax-only`: off, the tracked memory totals are
  byte-identical to the base (42 945 192 bytes live) and peak RSS is
  91.8 MB for both; time, seven rounds, minimum 0.71 s for both, median
  0.83 s against 0.76 s on a loaded host (an earlier quiet run: 0.75 s
  against 0.78 s): within noise. On: 11 000 rows; the table grows from
  6 038 096 to 7 679 544 bytes live (+1 641 448, 149 bytes per row with
  the pattern-node map and the index), peak RSS from 153 MB to 160 MB.
- The same with a few instantiations of `vector`, `map`, `string`,
  `sort`, `unique_ptr`, `function`, `optional`, `variant` and a
  `views::filter` pipeline: 12 723 rows, +1 793 840 bytes live (141
  bytes per row), peak RSS 156 MB to 164 MB.

On, the main table's dump is identical to the base's once the `spec=`
lines are removed: stage 4 adds no main-table entry and changes none.

WP-2220 (base = WP-1934; the same TU, debug builds, seven interleaved
rounds on a host loaded by other builds, load average about 9): off,
the tracked memory totals are byte-identical to the base (42 945 192
bytes live) and peak RSS is 107 MB for both; time, minimum 9.44 s
against 10.97 s, median 12.0 s against 16.7 s: within the host's
noise. On: 11 829 rows against 11 000 (+829: the using-declarations,
constraints and still-dependent types now recorded), the table from
7 679 544 to 8 356 484 bytes live (+676 940, of which the 4-byte
`udir` is 47 316; the rest the new rows, their pattern nodes and the
using-directive map), peak RSS 115 MB to 116 MB. The main table's dump
(209 861 lines without the `spec=` lines) is identical to the base's.
The 29 `bench/oven` entries' `-fconstexpr-report` counters (all but
`wall_us`) are identical.

#### 18.9.7 Tests

`crates/kiln_ast/src/written_refs.rs` (the row is 32 bytes, off records
no pattern node, a forgotten pattern node is only its own entry's, a
row is kept once and an update that duplicates
another merges into it, a SFINAE drop removes one specialization's rows
only, truncation drops the rows and pattern nodes of the entries it
drops), `crates/kiln_parse/tests/written_refs.rs` (rename by table:
`specs_naming` finds both instantiated uses of a member `size` and not
the other class's; the rows with the uses swapped, compared sorted),
and `tests/conformance/written_refs/inst-*.cpp`, one per family: a
member call on a dependent object (`inst-member-call`), dependent
qualified names and their qualifier components (`inst-qualified`),
`typename`/`template` names (`inst-typename`), an unqualified dependent
call with argument-dependent lookup and its set (`inst-adl`), `this->`
and a dependent base's members (`inst-this-base`), one name resolved to
two entities (`inst-two-entities`), default arguments and a default
member initializer in two specializations (`inst-default-arg`), a
generic lambda (`inst-generic-lambda`), variable and alias templates
(`inst-var-template`), explicit instantiation (`inst-explicit`), a
member template of a class template (`inst-nested`), a substitution
failure (`inst-sfinae`), an instantiation error (`inst-error`),
function template default template arguments used by deduction, with
several candidates, carried by a precompiled header and a module
image (`inst-default-targ`, `inst-default-targ-pch`,
`inst-default-targ-module-*`), a dependent call with one candidate, viable or not, in a constraint, a deduction and a body
(`inst-adl-single`, `inst-adl-single-error`),
using-directives at file, namespace and block scope, with
argument-dependent lookup and both (`inst-udir`), dependent types a
generic lambda's body rebuilds still dependent (`inst-rebuilt`),
using-declarations with a dependent qualifier and the names they
introduce (`inst-using`), names in constraints and requires-expressions,
with a satisfaction-cache hit (`inst-constraint`), an unqualified call
found by argument-dependent lookup in a requires-clause, a nested
requirement and a constraint checked with a definition
(`inst-adl-constraint`) and in a generic lambda, resolved by the
enclosing instantiation or still dependent (`inst-adl-generic-lambda`),
a template candidate never deduced beside a perfect non-template match
and one deduced beside an imperfect one (`inst-deferred-candidate`), an
alias specialization reused after a better candidate is declared
(`inst-alias-first-use`, compared with GCC), and the
order of instantiation swapped (`inst-order-a`, `inst-order-b`, whose
goldens differ only in the lines of `run`), also with the
specializations first named through an alias in one and through the
class in the other (`inst-spelling-a`, `inst-spelling-b`). The stage-1 to stage-3
goldens with instantiated templates (`dependent`, `example`,
`qualifiers`, `type-combined`) gain their rows.

### 18.10 Stage 5: carriage in PCH snapshots and module images (WP-1934)

A tooling client indexes a translation unit as it is built: with a
precompiled header, or importing modules. Stage 5 makes the table the
same in those builds as in the one that reads every source as text.
The driver's `-fkiln-written-refs` turns recording on without the dump
(spec/driver.md); a PCH or a module file must be built with it for its
names to be carried.

#### 18.10.1 PCH snapshots

- **Written.** `Sema`'s snapshot root (`kiln_sema::snap`) writes, right
  after the AST, one length-prefixed section (`Ast::put_written_refs`):
  with recording on, the table's state as a `WrittenRefsSection`
  (`kiln_ast::written_refs`, its own `Snap` schema folded into
  `Sema`'s): the entries; the pending overload patches; the provenance
  side rows (using-directive, argument-dependent set); the interned sets;
  the directive cache; the deferred designators; the per-specialization
  rows, dropped rows included so every `SpecRefId` stays; the pattern
  nodes and their candidates' using-directives (§18.9.1); the unrecorded rows (§18.10.3); the leads (§18.11.2); the found sets, their
  elements and the (entry, set) rows (§18.12.6); the declarator groups of
  the shared entries (`groups`, `group_decls`, §18.3.1, WP-3144); the
  substitution keys, their rows and their roots (one span per key and
  the slots) and the expanded nodes (§18.14.5, WP-3540, WP-3541): a key
  is carried with its rows and its root, or not at all. Maps are
  written as their entries sorted by key, so the bytes do not depend on
  hashing.
  **Rebuilt** on read: the set index, the found-set index and the row
  de-duplication index, from the sets and the live rows. **Transient**: the scratch of one
  declaration or one instantiation (`inst_target`, `spec_pending`,
  `spec_last`, `spec_key`), empty at the end of a prefix.
- **Validated** on read (an `Invalid` error, never a panic): every
  entry, row and set id in range, every qualifier before the entry it
  qualifies, the leads sorted and each `::` on a globally qualified
  entry, the groups sorted by first declaration, of two or more each,
  and every `GROUP` entry's `enclosing` a group's first, no set interned twice, no live row kept twice, and the
  section's typed ids within the restored AST's arenas
  (`Ast::check_ids`).
- **Restored, recording on.** The restored table continues the
  prefix's: the prefix's entries keep their ids and the main file's
  are appended after them; a pattern of the prefix instantiated by the
  main file gives its per-specialization rows through the carried
  pattern nodes, as without the PCH. The dump equals the dump of the
  same main file compiled with the header as an `-include` (positions
  in the header print with its base name either way). What may differ
  is not printed: `DeclId`s, `SpecRefId`s and the order of
  per-specialization rows (the dump sorts them, §18.9.5). Clients
  that keep ids across builds must key by position, as for any two
  builds.
- **Restored, recording off.** The section is skipped by its length,
  undecoded and unallocated; the table is off and empty.
- **Identity.** `-fkiln-written-refs` and `-fkiln-dump-written-refs`
  are outside the PCH identity (spec/pch.md): a PCH made either way
  serves a compile made either way, and what the consumer sees follows
  §18.10.3.

#### 18.10.2 Module images

- **Written.** When the module unit is compiled with the table on, the
  image's last section, `refs` (spec/modules.md §11.11 "Written
  references"), carries the entries of the names written in the unit's
  own sources after its preamble, their provenance, the
  per-specialization rows of its carried specializations, the
  pattern nodes of its dependent entries and their found sets
  (§18.12.6), and, with keys on, the substitution keys with their rows
  and roots (§18.14.5): a key is carried with its rows and its root, or
  not at all (left out and counted). The preamble's names (the
  module declaration, the imports, and with them the names the imported
  units' replays recorded) are not carried: the importer replays the
  preamble (spec/modules.md §11.11 "Imports and re-exports") and
  records them by parsing, as it does for any source it reads. A name
  whose declaration (entity, found, enclosing, using-directive, a set's
  member, a found set's element), location, qualifier or pattern node the image cannot carry
  fails the image, and the importer replays the unit: correctness
  first. A per-specialization row whose specialization or entity is
  not a record is left out (the importer makes the row again when it
  instantiates that specialization). A unit whose own table has
  unrecorded rows (§18.10.3) fails the image too.
- **Imported, recording on: merged.** The importer appends the unit's
  entries to its own main table when it builds the unit's records, in
  the unit's recording order, mapped to the importer's declarations and
  locations (the module's files are in the importer's source manager,
  spec/modules.md "File table"). The table is merged, not kept per
  module, because a replay of the unit already records its names in the
  importer's table by parsing them (stages 1 to 4): merging makes the
  table, and the dump, the same whichever way the unit is imported
  (`module-image.cpp` and `module-replay.cpp` share one golden), and
  the importer's per-specialization rows for the unit's templates key
  to the merged pattern entries. A client reaches the unit's names by
  their files: an entry whose `range` is in one of the module's files.
  Rows the importer makes when it instantiates the unit's templates are
  the importer's (they are its instantiations), recorded as after a
  replay.
- **Imported, recording off.** The section is skipped by its length,
  undecoded (`decode_with(.., false)`).
- **Module cache.** The driver's implicit module builds
  (`module_cache.rs`) record when the compile that needs them does, and
  their cache key names `written-refs`, so a cached build without the
  table is not used by one that records.

#### 18.10.3 What was not recorded

A source whose names could have been carried but were not is listed by
`WrittenRefs::unrecorded()`, never left as a silent gap:

- `Unrecorded::Prefix { files }`: the snapshot was made with recording
  off and the consumer records; `files` is the number of files the
  restored source manager holds (the prefix's files, ids `0..files`).
  The restored table starts empty.
- `Unrecorded::Module { module, files }`: the unit `module` (its key)
  was built from an image made with recording off; `files` are its
  files in the importer's source manager.

The dump prints each row first, `unrecorded  precompiled prefix (<n>
files)` and `unrecorded  module <key> (<main file's base name>, <n>
files)`. A table with an unrecorded row is still carried by a later
snapshot (the rows with it); a module unit with one fails its image.
The substitution keys' own gaps (`PrefixKeys`, `ModuleKeys`,
`ModuleKeysLeftOut`, §18.14.5) are listed the same way and do not fail
an image.

#### 18.10.4 Cost

Off, nothing is recorded and nothing new runs: the snapshot is the
one a compiler without the table writes plus one byte (the section's
LEB128 length, 0), and the image is that plus one `u32` 0 (a PCH file
also stores its options as text, which gained the line of the new
`written_refs` option). Their
format versions are bumped (PCH `FORMAT_VERSION` 144, image
`IMAGE_VERSION` 77; 145 and 78 for WP-2220's per-specialization
using-directives; 147 and 80 for WP-2326's leads, §18.11.2; 161 and
87 for WP-2828's specifier leads; 164 and 88 for WP-2820's operator
entries, §18.13.8). A restore or import that does not record reads
the length and skips the section. Measured in §18.10.6.

#### 18.10.5 Tests

`crates/kiln_ast/src/written_refs_carry.rs` (off writes one byte and a
reader that does not record skips the section; an empty section read
with recording on is a prefix gap; a qualifier after its entry is
rejected), `crates/kiln_sema/src/snap.rs` (the table off adds one byte
after the AST's own encoding, on leaves that encoding unchanged),
`crates/kiln_sema/src/bmi_image/refs.rs` (the section round trips; off
it is one empty word after the image without it; skipped undecoded;
every validation), `crates/kiln_driver/tests/written_refs_carry.rs`
(every compiling file of `tests/conformance/written_refs/` split at a
top-level boundary, its first part precompiled: the dump through the
restored snapshot equals the dump with the part as an `-include`; a
recorded snapshot used by a compile that does not record; a module
compiled without the table is unrecorded, with it merged, and skipped by
an importer that does not record), `crates/kiln_parse/tests/written_refs.rs`
(rename by table across a restored prefix and the main file), and
`tests/conformance/written_refs/`: `pch.cpp` (through a PCH) and
`pch-include.cpp` (the header as text) share `pch.refs`; `pch-off.cpp`
(a PCH made without the table: the gap line); `module-image.cpp` and
`module-replay.cpp` share `module.refs`.

#### 18.10.6 Measurements (WP-1934)

Debug builds of the base (WP-1933) and of this stage, on an aarch64
Linux host, GCC 14's libstdc++, `-std=c++26 -fsyntax-only`, five runs
each interleaved (minimum / median seconds, peak RSS):

| compile | base | stage 5, off | stage 5, on |
|---|---|---|---|
| `#include <bits/stdc++.h>` and `main` | 9.96 / 10.75 s, 105 MiB | 9.67 / 10.81 s, 102 MiB | 9.93 / 11.56 s, 113 MiB |
| the same `main` with `-include-pch` of the header | 1.99 / 2.32 s, 99 MiB | 1.82 / 2.22 s, 99 MiB | 1.97 / 2.35 s, 107 MiB |

Off is within noise of the base, with and without the PCH. The PCH of
`<bits/stdc++.h>` is 37,579,848 bytes at the base and 37,579,874 off
(the section's byte and the 25-byte option line); with the table on it
is 43,840,681 (+16.7%), and restoring it with recording on costs about
8 MiB of peak RSS. The dump of the TU (220,970 lines: entries and
per-specialization rows) is the same through the PCH as with the header
included as text. The image of `Inputs/refs_mod.cppm` is 25,711 bytes
at the base, 25,715 off and 26,484 on. The 29 `bench/oven` entries'
`-fconstexpr-report` counters (all but `wall_us`) are identical between
the base and this stage.

### 18.11 The join and the leading `::` (WP-2326)

#### 18.11.1 The join with expressions

A client holding an expression (a cross-referencer walking the AST, a
tool that reports on a call) finds the entry of its name by location.

- **Rule.** For every `DeclRefExpr` and `MemberExpr` `e` built from an
  id-expression or a class member access's member name that the parser
  consumed, outside the exceptions below, exactly one entry `id` of
  role `expr`, `member` or `template` has
  `WrittenRefs::name_lo(id) == ast.exprs.get(e).loc()`, and its
  `entity` is `ast.exprs.decl(e)`. `name_lo(id)` is `range.lo`, except
  for a destructor name, where it is the `~` (`tilde(id)`, §18.11.2).
  `ExprNode::loc()` is the one location either expression kind stores:
  the name's, never the qualifier's or the object expression's. A name
  sema resolves once more after recording it follows: a function
  template-id resolved early from its explicit arguments and again
  against its target ([over.over]/1, `retarget_template_id`) has its
  entry re-pointed to the specialization the target deduces
  (`WrittenRefs::repoint`, which looks back over the last 4,096
  entries; before WP-2326 the entry kept the early one).
- **Where the location is**, for each way of writing the name:

| written | expression | `loc()` | the entry |
|---|---|---|---|
| `f`, `x` | `DeclRefExpr` | the identifier | `expr` |
| `N::f`, `::N::f` | `DeclRefExpr` | `f`, the last component | `expr`; its `qualifier` links the `nns` entries of `N` (and the `::`, §18.11.2); `qualified_lo(id)` is where the whole name begins |
| `f<int>`, `N::f<int>` | `DeclRefExpr` | `f`, the template name | `expr`, the specialization, `found` the template |
| `operator==`, `N::operator==` | `DeclRefExpr` | the `operator` keyword | `expr`, its range over every token of the operator-function-id |
| `a.operator==`, `a.operator int` | `MemberExpr` | the `operator` keyword | `member`, its range over the operator- or conversion-function-id |
| `a.m`, `p->m`, `p->N::m` | `MemberExpr` | the member name | `member` |
| `a.~T`, `p->N::~T` | `MemberExpr` | the `~` | `member` naming the destructor, `range` `T`, `tilde` the `~` |
| `a.template f<int>`, `p->template f<int>` | `MemberExpr` | `f` | `template` (the member template's specialization, `found` the template) |

- **Exceptions**: expressions with no name token of their own, which
  no entry joins (`implicit_name` in the test lists the same classes):
  1. an expression of an instantiation, of a member of a class template
     specialization the compiler instantiated, or a default argument or
     default member initializer used at a call or a construction (the
     children of `CXXDefaultArgExpr` and `CXXDefaultInitExpr`), the
     elements of an expanded expansion statement (`template for`, one
     instantiation of its body per element; its pattern is joined), or
     the body of a defaulted function (`= default`, synthesized): their
     names are the pattern's, joined there, and a specialization's
     resolutions are its per-specialization rows (§18.9);
  2. a reference to a declaration with no name or an implicit one: an
     anonymous union's member reached through its implicit field, a
     structured binding's hidden variable and the member access on it,
     a range-for's `__range`, `__begin` and `__end`;
  3. a condition variable's value (`if (auto t = e)`), which is at the
     variable's own name: a declaration, not a reference;
  4. an operator use's callee (`a + b`, `f(x)` calling `operator()`,
     `a = b`, `a[i]`), whose `loc()` is the operator token: it joins its
     `operator` entries by the operator join (§18.13.7), not by this
     rule;
  5. a reference sema built at a token that does not spell the name:
     the conversion
     function an implicit user-defined conversion calls (a `MemberExpr`
     at its operand, `int i = s;` puts it at `s`, or a `DeclRefExpr`
     for an explicit-object conversion function), a
     tuple-like structured binding's `get` (at the binding), a
     coroutine's `await_ready`, `await_suspend`, `await_resume` and
     promise calls (at `co_await`, `co_yield`, `co_return` or the
     function's body);
  6. an implicit capture's initializer (a capture-default's).

  A user-defined literal's literal operator (`1_km`), whose callee
  `DeclRefExpr` is at the ud-suffix, joins its `ud-suffix` entry by the
  rule (role `ud-suffix` instead of the three above; §18.13.4).

  One class joins by location with a different entity: a member access
  in a template pattern naming an overload set (type `OVERLOAD`) has
  the set's first member as its declaration while its entry names
  nothing and is `DEPENDENT` (or `UNRESOLVED`): the set is resolved per
  specialization (§18.9).
- **No expression-to-entry map.** A cold `ExprId → WrittenRefId` map
  would cost a hash-map slot per name expression (8 bytes of key and
  value at the map's load factor, about 10 to 18 bytes, a half of the
  28-byte entry again) and the overload-resolution and member-access
  funnels would have to hand the parser the expression they finally
  build (an overloaded name's expression is rebuilt after resolution,
  §18.4). The location rule makes the map unnecessary: a client indexes
  the entries by `name_lo` in one pass, or looks one name up by
  location in the table sorted by `range`. The rule holds either way.
- **Test.** `every_name_expression_joins_its_entry`
  (`crates/kiln_parse/tests/written_refs_accounting.rs`) parses every
  file of `tests/conformance/written_refs/` and the directories §18.8
  pins, walks the AST as the dump does (`kiln_ast::dump::for_each_written_expr`,
  which leaves instantiations out), and checks the rule for every
  `DeclRefExpr` and `MemberExpr` spelled in the main file: 790 files, 6,240 name expressions, no
  mismatch. Over all of `tests/conformance/` (3,467 files that compile,
  56,989 name expressions) it finds none either (WP-2326; the
  exceptions above and the template-id retarget came from that run).
  `every_name_expression_joins_its_entry_in_a_directory` (ignored) runs
  it on the `.cpp` files of `KILN_REFS_DIR` and fails on a mismatch with
  `KILN_REFS_STRICT` set. `tests/conformance/written_refs/join.cpp` has
  one of each row of the table above.

#### 18.11.2 The leading `::`, the `~` and the specifier

`RefLead { entry, colon, tilde, spec, close, langle, rangle }` (28
bytes) gives the tokens of an entry's name outside its `range`: the
leading `::` of a name whose `qualifier` is `GLOBAL`, the `~` of a
destructor name (an expression, a pseudo-destructor's type name, or a
destructor's declarator-id), the first token of the specifier before a
name whose `qualifier` is `SPECIFIER` (WP-2828), the closing `]` or `)`
of a subscript or call `operator` entry (§18.13.2, WP-2820), and the
`<` and `>` of the template argument list after a friend function
template specialization's name (WP-3008), each `Loc::INVALID` when
absent. A cold side table, sorted by entry, holds a row only for the
entries that have one:

- `global_colon(id)`: the `::` of entry `id`, whose qualifier is
  `GLOBAL` (the first component of `::a::b`, or `f` of `::f`);
- `tilde(id)`: the `~` of a destructor name;
- `close(id)`: the `]` or `)` of an `operator` entry at a `[` or `(`;
- `specifier_lo(id)`: the first token of the specifier before entry
  `id`, whose qualifier is `SPECIFIER`: the `decltype` keyword of
  `decltype(s)::m`, the `[:` of `[: r :]::m` and of
  `typename [: r :]<A>::m`, the `template` of `template [: r :]<A>::m`;
- `template_args(id)`: the `<` and `>` after entry `id`, a friend
  function template specialization's name (`friend void f<>(T);`), both
  or neither; a dependent friend's match waits for each specialization,
  and its declaration keeps no other record of where its arguments were
  written (clang's `DependentFunctionTemplateSpecializationInfo` keeps
  `TemplateArgumentsAsWritten`); the arguments' names are their own
  entries. Other template-ids keep theirs in the expression or type
  they built. Carried in snapshots and images with the other lead
  tokens (PCH `FORMAT_VERSION` 167, `IMAGE_VERSION` 90);
- `name_lo(id)`: the `~` or `range.lo`, the join location (§18.11.1);
- `qualified_lo(id)`: where the qualified name ending in `id` begins,
  following `qualifier` to the first component: its `::`, its
  specifier's first token, else its `range.lo`; for
  `decltype(s)::N::k` that is the `decltype`, for `Ts...[0]::v` the
  pack's name;
- `lead(id)`, `leads()`: the rows.

**The specifier (WP-2828).** A `decltype(...)::` or splice specifier is
not a name: the names of its operand have their own entries, and the
scope it denotes is a type, not a declaration a component entry could
link. Rather than an entry for the specifier (a new kind of entry, which
the identifier accounting of §18.8 and every client's "one entry per
written name" would have to except), the component after it records the
specifier in two places: its `qualifier` is `SPECIFIER`, so a client
sees that the name is qualified (before WP-2828 it was `NONE`, and its
lookup provenance re-ran an unqualified walk; a `SPECIFIER` entry has
none, as the scope is not kept), and its row's `spec` is the
specifier's first token, so `qualified_lo` reaches it. The parser takes
the link from `WrittenRefs::specifier_link(first)` where it parses the
specifier ahead (`Parser::annotate_decltype_scope`,
`set_decltype_scope`), through the scratch list of the leading `::`
(bit 30 of the in-flight link marks a specifier). A pack-index-specifier
needs neither: its pack's name is a component entry already.

The locations are kiln token locations, as `range`'s are: a `::`
spelled by a macro (`#define G ::`, `G a::f()`) is the macro's token,
whose spelling is in the macro's body and whose expansion point is the
macro's use; a `::` cut from its name by a comment, a line break or a
line splice is at its own token; one split by a line splice (`:\` and
`:` on the next line) is at its first `:`. `:: template G<int>::x`,
`typename ::a::A<T>::type`, `p->::a::S::~S()`, the pseudo-destructor
`i->::I::~I()` (both type names share the one `::`) and the
declarator-id `::a::S::~S` each keep theirs
(`tests/conformance/written_refs/global-colons.cpp`).

**Recorded at parse.** At the three places a leading `::` starts a
qualifier (`parse_nested_name_upto`, the pseudo-destructor's
`::F::~Q`, and the scanned qualifier of a template template argument),
the parser takes the link from `WrittenRefs::global_link(colon)`
instead of `RefQualifier::GLOBAL`: a `u32` with the global bit (bit 31)
set and the `::`'s index in a scratch list of locations below it.
`push` stores it as `GLOBAL` with the `::` as the entry's lead, so no
in-flight link reaches an entry and every reader of `qualifier` is
unchanged (entry ids stay below bit 31). The `~` is noted with the
destructor name (`Parser::note_wname_dtor`) and the destructor
declarator-id. The alternative, deriving the `::` as the token before
the first component's `range.lo`, was rejected although it stores
nothing: the table's clients have no token stream after the unit is
built (an importer or a PCH consumer never had the prefix's), the
token before the first component is `template` in `:: template G<int>`,
and for a macro's `::` the text before the name is the macro's name;
skipping comments and splices would need the lexer again.

**Cost.** Off, `global_link` is one branch on the flag and returns
`GLOBAL` (the parser called nothing before), the destructor sites
test the flag they already tested, and the name the parser holds for
`with_written_name` carries one more `Loc`: nothing is allocated and the entry
is still 28 bytes (its `size_of` test). On, each leading `::` costs 4
bytes of scratch and each globally qualified entry or destructor name
a 12-byte row. On `#include <bits/stdc++.h>` (debug build, GCC 14's
libstdc++, `-std=c++26 -fsyntax-only`) the table has 209,806 entries,
8,367,460 live bytes; 641 are globally qualified and 60 are destructor
names, 701 rows (8,412 bytes) and at least 641 scratch locations (2,564
bytes): about 11 KB, 0.13% of the table. Every `q=::` of its dump has
its position. WP-2828: `specifier_link` is the same one branch off;
on, a specifier costs 4 bytes of scratch and its entry a row, and a row
is 16 bytes (the `spec` word). On the same header (WP-2828's tree, the
same build and flags) the table has 254,158 entries; 3 are qualified by
a `decltype(...)::` specifier, 641 globally and 60 are destructor names,
704 rows (11,264 bytes); every `q=specifier` of its dump has its
position. WP-2820's `close` word makes a row 20 bytes (§18.13.8).

**Carriage.** The PCH section (§18.10.1) carries the rows as
`WrittenRefsSection::leads`, validated sorted, in range, each `::`
on an entry whose qualifier is `GLOBAL`, each specifier token on one
whose qualifier is `SPECIFIER` and each closing token on an `operator`
entry; the scratch list is never
carried (no in-flight link outlives its name). The image's `refs`
section (spec/modules.md §11.11) ends with them, (entry, colon, tilde,
spec, close) as five words with `NONE` for an absent location, its
locations in the offset check, validated as the snapshot's (a `close`
only on an `operator` entry); a `SPECIFIER` qualifier is the word
`Q_SPEC`. PCH `FORMAT_VERSION` 147, `IMAGE_VERSION` 80 (146 and 79 are
WP-2091's); the `spec` word and `SPECIFIER`: `FORMAT_VERSION` 161,
`IMAGE_VERSION` 87 (WP-2828); the `close` word: `FORMAT_VERSION` 164,
`IMAGE_VERSION` 88 (WP-2820).

**Tests.** `crates/kiln_ast/src/written_refs.rs`
(`leading_colons_and_tildes_are_kept_beside_the_entry`: the link,
normalization, off, `name_lo`, `qualified_lo`, sorted insertion,
truncation), `written_refs_carry.rs` (the rows round trip; a `::` on an
entry that is not globally qualified is rejected),
`crates/kiln_sema/src/bmi_image/refs.rs` (the rows round trip; a row of
no entry, a `::` on an entry without one and rows out of order are
rejected), `crates/kiln_parse/tests/written_refs.rs`
(`leading_colons_and_tildes_through_the_api`: a macro's `::`, a
comment between, the destructor's `MemberExpr` at its `~`),
`tests/conformance/written_refs/global-colons.cpp`, and the carriage
goldens: `Inputs/refs_pch.h` and `Inputs/refs_mod.cppm` end with a
`p->::N::Pt::~Pt()`, so `pch.refs` (through the snapshot and as text)
and `module.refs` (through the image and replayed) show the same
`q=::` and `tilde=` positions.

### 18.12 The found set of a dependent entry (WP-2472)

A `DEPENDENT` entry has no entity (§18.2): what it names is decided per
specialization (§18.9). But for most dependent names a lookup already
ran at the template definition, and its result constrains every
specialization: ordinary lookup of an unqualified dependent call's name
([temp.dep.candidate]/1), qualified lookup in a non-dependent scope, and
class member lookup in the current instantiation ([temp.dep.type]/6). A
tooling client renaming a declaration must know which declarations
such a name found, also in a template no specialization of which was
instantiated. The found set is that result.

#### 18.12.1 Contract

- **Names in array bounds.** A dependent name in an array bound whose
  value is known but whose expression is instantiation-dependent
  (`char (*)[sizeof(sizeof(f(T())))]`) is an entry like any other: the
  constant array type keeps the expression (spec/types.md §7.2 "Kept
  bounds"; WP-2720), so the entry's set is reachable from the
  declaration's written type, and each specialization substitutes the
  bound and records its row (§18.9).

- **Definition.** The found set of a `DEPENDENT` entry is the result of
  the name lookup the standard performs for its name in the template
  definition: unqualified lookup from the name's position
  ([basic.lookup.unqual]) for an unqualified name; qualified lookup in a
  non-dependent scope ([basic.lookup.qual]); class member lookup in the
  current instantiation ([class.member.lookup], [temp.dep.type]). It is
  fixed when the entry is made and never changes: a declaration that
  appears after the name is not in it (in a complete-class context the
  whole class is visible, [class.mem.general]).
- **Same switch as the table.** Recorded only when
  `ParseOptions::written_refs` is on, by the parse-time site that makes
  the entry (`note_written_name`, beside its pattern node, §18.9.4).
  Off, nothing changes: no branch is added (every recording site is
  inside a function that already returns when the table is off, and the
  rows are removed only where an overloaded name's pending patch, which
  exists only when on, is resolved). Names parsed for the table alone
  (§18.8: no pattern node) get no set.
- **Where the facts come from.** An overload set's list is the pattern
  node's own: the declarations of the `UnresolvedLookupExpr`, which
  instantiation itself selects from (spec/sema-templates.md §3.5), with
  their using-shadows and naming class, or the set of an overloaded
  member access (a `MemberExpr` of overloaded type). A dependent member
  access of the current instantiation (`this->a(t)`, `C::a(t)`, `a(t)`
  with an implicit `this`, `this->template mt<T>(t)`) re-runs, at the
  recording site, the class member lookup that decided it was dependent
  (`Sema::current_member_lookup`, which `try_dependent_member_q` uses
  too) or, for an implicit `this`, the scope walk: the same pure
  functions over the same scopes, as §18.7.1 re-runs the walk.
- **Storage.** A sparse side table: one row (entry, set) per entry with
  a set, sorted by entry by construction, read by binary search; the
  sets interned by content (elements, `how`, flags), one copy each. The
  28-byte entry does not change. A callee recorded before its arguments
  and then resolved at the definition loses its row
  (`WrittenRefs::resolve`, `written_call_failed`): its entry is no
  longer `DEPENDENT`. `truncate` drops the rows of the entries it drops.
- **Read access** (`kiln_ast::written_refs`):
  `found_at_definition(id) -> Option<FoundSet>`: `None` when the entry
  is not `DEPENDENT` or no lookup ran at its definition, else the set
  (`decls: &[FoundDecl]`, `how`, `flags`; `is_closed()`, `adl()`,
  `dependent_base()`, `unresolved_using()`, `contains(d)`);
  `found_set_id(id)` (equal ids are equal sets), `found_set(s)`;
  `found_udir(id, decl)`: the using-directive the definition lookup
  found an element through (§18.9.1's, by the element's `decl`);
  `entries_finding(d)`: the dependent entries whose set has `d` as an
  element's `decl` (a first declaration) or `found` (a using-shadow);
  `Ast::spec_origin(&row)`: the element a per-specialization row
  selected (§18.12.4).
- **Rename.** A rename of `d` edits the main-table entries naming `d`
  (§18.1), the entries of `entries_finding(d)`, and the pattern entries
  of `specs_naming(d)` (§18.9.1). For an entry of `entries_finding(d)`,
  a scan of its set's `decls` tells whether `d` is every element (the
  name always names `d` or a specialization of it), one of several (the
  edit renames a name other declarations share), or `d` is reached
  only through a using-shadow.

#### 18.12.2 Kinds

| entry (§18.9.4 pattern node) | the set is | `flags` |
|---|---|---|
| unqualified dependent call `f(t)`, a template-id `f<T>(t)`, an operator-function-id `operator+(t, t)` (`UnresolvedLookupExpr` with ADL) | ordinary unqualified lookup at the name | `ADL`: argument-dependent lookup at each instantiation adds to it |
| the same, nothing visible (`g(t)`, an assumed template name `get<0>(t)`, [temp.names]/3) | empty: lookup ran and found nothing | `ADL`: only argument-dependent lookup can resolve it |
| `(f)(t)`, `::f(t)`, `N::f(t)` with several overloads | unqualified (parenthesized) or qualified lookup | none: closed, every instantiation selects from it |
| a friend function template specialization with dependent arguments or type (`friend void f<>(T);`, `friend void n::f<T>(T);`, §18.4) | the function templates ordinary or qualified lookup of the name found at the definition | none: closed, each instantiation matches one of them ([temp.friend]/1.3) |
| one visible `f` with argument-dependent lookup suppressed (a block-scope declaration, a class member), one member found by `this->a(t)` | no set: resolved at the definition, the entry has its entity | |
| `this->a(t)`, `C::a(t)`, `a(t)` (implicit `this`), `this->mt(t)`, `this->template mt<T>(t)`: members of the current instantiation (`CXXDependentScopeMemberExpr`, or an overloaded member access) | class member lookup in the current instantiation: members of the pattern class and its non-dependent bases | none when the class searched has no dependent base, or declares the name itself (its declaration set is the result, a base's member of the name is hidden, [class.member.lookup]/2; a using-declaration of a non-dependent base's member counts, one of a dependent base's member does not; WP-3450); else `DEPENDENT_BASE`: the instantiation looks the name up again, and a different result is ill-formed ([temp.dep.type]/6; not diagnosed, §18.12.6) |
| a name a dependent using-declarator introduces (`using B<T>::bm; ... bm(t)`) | the class's lookup result, the `UnresolvedUsingValue` an element | `UNRESOLVED_USING`: what it names exists per specialization only |
| `T::x`, `B<T>::bm(t)` (a dependent qualifier), `t.m(t)`, `this->bm(t)` naming nothing the class declares in a class with a dependent base (a member of an unknown specialization), `using B<T>::bm;` itself, `typename T::ty`, a dependent nested-name-specifier component | no set (`None`): no lookup runs at the definition; all knowledge is per specialization (§18.9) | |
| a dependent operator `t + t`, a fold's operator, a `co_await` in a coroutine template (§18.13.6) | the definition-context lookup of `operator@` (`Ast::dependent_oper_candidates`, §8.5; for `co_await`, the unqualified lookup of `operator co_await` re-run at the recording site) | `ADL` |
| a dependent `t[i]`, `t(x)`, `t->m`, `t = u`, `new T`, `delete p` (§18.13.6) | no set: no definition-context lookup ([over.match.oper]/3.1: member candidates only; [expr.new]/12, [expr.delete]/9: the class's scope, then the global scope) | |

An unqualified name in a class template with a dependent base: the
dependent base is not examined, at the definition or at instantiation
([temp.dep.general]/4); a set found at namespace scope stays closed or
`ADL` and gets no `DEPENDENT_BASE`. A failed call (`UNRESOLVED`) has no
set. Argument-dependent lookup in the definition context is not
precomputed into the set: its declarations appear as rows with
`how=adl` (§18.9), as do hidden friends, which ordinary lookup never
finds ([class.friend]/7) and so are never elements.

#### 18.12.3 The element

`FoundDecl { decl, found }` (8 bytes), as `oper_lookup::OperCandidate`
and §18.2 do it: `decl` is the entity's first declaration (through a
using-declaration: what it names), so equal sets intern across
redeclarations; `found` is the using-shadow lookup found, else `decl`.
A dependent using-declarator is an element by itself (`decl` its
`UnresolvedUsingValue`). The elements are in the order overload
resolution enumerates them (innermost scope first, newest first);
clients treat them as a set. `how` is the definition lookup's kind:
`Unqualified`, `Qualified` (after a qualifier) or `Member` (after `.`
or `->`), by the entry's syntax (the entry's own `how` stays `None`,
§18.7.2). The flags are `FOUND_ADL` (1), `FOUND_DEPENDENT_BASE` (2)
and `FOUND_UNRESOLVED_USING` (4); no flag is `is_closed()`. `None`
(no definition lookup) and `Some` with no element (lookup ran, found
nothing) differ.

#### 18.12.4 The join with the per-specialization rows

The set is the specialization-independent half, a row (§18.9) one
specialization's selection. For a row `r` of an entry with set `S`, in a
specialization without errors: `r.how == Adl` implies `S` has `ADL` and
`r` is not from `S`; otherwise `spec_origin(r)` is an element's `decl`,
where `spec_origin` is the first declaration of what lookup found
(`found`, else `entity`, a using-shadow looked through), mapped by
`Ast::instantiated_from` to the pattern's member when it is a member of
a class template specialization, and a variable template's
specialization to its template. A row whose `how` is `Both` is from
`S` too. Exceptions, each visible to the client: an ill-formed
specialization (kiln's two-phase recovery selects a declaration found
after an error, spec/sema-templates.md §3.5), `DEPENDENT_BASE` (a member
of a dependent base), `UNRESOLVED_USING`, a `SPEC_BUILTIN` row (a built-in
operator selects no declaration, §18.13.6), an `operator` row whose `how`
is `Member` (a member operator function, a function call operator or a
surrogate's conversion function is looked up in the operand's class at
instantiation, [over.match.oper]/3.1, [over.call.object]/2, and is never
in the definition's set, which holds the non-member candidates), and a
member of a closure type
(`[&]<int I>() {...}.template operator()<1>()` in a function template:
the lambda-expression of each specialization defines a new closure type
([expr.prim.lambda.closure]/1), and kiln keeps no link from its members
to the pattern closure's, so `spec_origin` returns the specialization's
call operator template). The invariant is tested over
the written-reference corpus and the §18.8 directories
(`spec_rows_agree_with_definition_sets`).

#### 18.12.5 The dump

The entry's line ends with `def=<n>`, then `+adl`, `+base` and `+using`
for the flags; then one line per element, before the entry's `spec=`
lines (§18.9.5), sorted by the position of the element's `decl` (the
main file first):

```
<lo>-<hi>  <role>  '<spelling>' def=<decl> [found=<shadow>] [udir=<ns>@<line>:<col>]
```

`<decl>` and `<shadow>` print as an entity does (§18.6), `udir=` as
§18.7.5's. The dump through a PCH or a module image equals the dump
with the source as text.

#### 18.12.6 Cost and carriage

**Cost.** Off: zero (no branch added; the tracked memory totals on
`<bits/stdc++.h>` are byte-identical to the base, and the 29
`bench/oven` counters are identical). On: 8 bytes per row, 12 per
distinct set and 8 per element, plus the content index. Measured
(WP-2472, against the base, recording off and on, interleaved, three
rounds each, `-fsyntax-only -fkiln-dump-written-refs
-fkiln-memory-report`): `<bits/stdc++.h>` at `-std=c++26` has 11483
entries with a set of 25302 dependent entries (8829 closed, 1212
`ADL`, 784 `DEPENDENT_BASE`, 658 `DEPENDENT_BASE` and
`UNRESOLVED_USING`; 5525 of one element, 3390 of two); the table's
tracked memory grows from 8366308 to 8687172 bytes (+320864, +3.8%;
+0.6% of the TU's 51.3 MB tracked total). LLVM's `SemaExpr.cpp` at
`-std=c++17`: 10662 entries with a set of 24652, +265784 bytes
(23340264 to 23606048, +1.1%; +0.1% of 219 MB). Peak RSS grows by
5.5 MB and 4 MB (the dump's text, 32468 and 24506 more lines, most of
it); wall time is within the shared host's run-to-run noise (medians
1.68 s and 1.45 s, 7.51 s and 7.18 s, base and new). Off: the memory
report of both TUs is byte-identical to the base's, and the 29
`bench/oven` `-fconstexpr-report` counters (all but `wall_us`) are
identical.

**Carriage.** The PCH section (§18.10.1) carries the rows, the sets and
the elements (`WrittenRefsSection::found_rows`, `found_sets`,
`found_decls`), validated on read: rows sorted and in range, each set a
run of the elements, no set interned twice; the content index is
rebuilt. The image's `refs` section (spec/modules.md §11.11) ends with
the sets (`how`, flags, `decl, found` word pairs) and the rows (entry,
set) of the unit's own entries; an element the image cannot carry fails
the image and the importer replays (§18.10.2). A prefix or unit made
with recording off has no entries at all, so there is no "entries
without sets" state (§18.10.3). PCH `FORMAT_VERSION` 154,
`IMAGE_VERSION` 82.

Not done: the [temp.dep.type]/6 rule that a member of the current
instantiation whose lookup in the instantiation differs from the
definition's is ill-formed is not diagnosed (kiln looks the name up
again in the instantiated class and does not compare); folding the
per-candidate using-directives of §18.9.1 into the elements.

#### 18.12.7 Tests

`crates/kiln_ast/src/written_refs.rs` (`found_sets_are_interned`,
`found_rows_follow_their_entries`, `off_records_no_found_set`: the entry
still 28 bytes, interning, a popped row, truncation, off),
`written_refs_carry.rs` (`found_sets_round_trip_and_are_validated`),
`crates/kiln_sema/src/bmi_image/refs.rs` (the sets and rows round trip
and are validated), `crates/kiln_parse/tests/written_refs.rs`
(`rename_by_found_set`: all subjects, none, mixed),
`crates/kiln_parse/tests/written_refs_accounting.rs`
(`spec_rows_agree_with_definition_sets`: §18.12.4 over the corpus and
the §18.8 directories), and `tests/conformance/written_refs/found-*.cpp`:
one visible declaration (`found-one`), three through a declaration, a
using-declaration and a using-directive (`found-several`), none
(`found-none`), a hidden friend (`found-hidden-friend`), a later
overload (`found-later-overload`), closed sets and a block-scope
declaration (`found-closed`), members of the current instantiation
(`found-this`) and with a dependent base, an unknown specialization and
a dependent using-declarator (`found-this-base`), every `this->`,
qualified and unqualified form in a class with a dependent base, closed
when the class declares the name, `+base` when a non-dependent base
does (`this-member-dependent-base`, WP-3450), function template-ids
and an assumed template name (`found-template-id`), an
operator-function-id call and an operator expression
(`found-operator`), and the carriage goldens through a PCH and as text
(`found-pch`, `found-pch-include`) and through a module image and
replayed (`found-module-image`, `found-module-replay`). The goldens of
the earlier stages with dependent entries gain their `def=` lines.

### 18.13 Operator uses and user-defined literals (WP-2820)

Stage 6: the written tokens whose overload resolution or literal
operator lookup selects a function. A renamer of `operator==` or of
`operator""_km`, or a tool that removes an operator overload, finds
every use through the table without walking the AST.

#### 18.13.1 The rule

A written token that makes overload resolution select a function gets
an entry naming it, as any other entry (§18.2: a function template's
specialization when one was selected, `found` the template or the
using-shadow lookup found it through):

- the operator token of an operator expression whose operand of class
  or enumeration type made it an overloaded-operator use
  ([over.match.oper]/1-2, [over.sub], [over.call], [over.ref],
  [over.inc], [over.ass]), role `operator`;
- the ud-suffix of a user-defined literal ([lex.ext]/2-8,
  [over.literal]), role `ud-suffix`;
- the `new` and `delete` keywords of a new- or delete-expression
  ([expr.new]/12, [expr.delete]/9), naming the allocation or
  deallocation function, and the `co_await` keyword where an
  `operator co_await` applies ([expr.await]/3.3), role `operator`
  (stage 6c, WP-2911).

A token whose resolution selects a built-in candidate ([over.built];
also for class operands converted to a built-in type, `c + 1` with an
`operator int`), or that is never overloadable (`?:`, `.*` on a
pointer, `sizeof`), has no entry. A call the compiler makes with no
written token of its own has none: implicit conversions (whose
conversion function's only range would be the operand's own name,
already an entry), range-for iterator operations, structured-binding
`get`, awaiter and promise calls, the bodies of defaulted comparisons,
and the destructor a `delete` calls. An explicit operator call
(`a.operator+(b)`, `operator+(a, b)`, `operator""_k(12)`) is a name,
unchanged (§18.2, §18.11.1): its `(` is a function call, not an
operator use.

`REF_MEMBER` is not set on an `operator` entry (it means "written
after `.` or `->`"); a member candidate is told by `how=member`.

#### 18.13.2 Per form

| written | entries | range | entity |
|---|---|---|---|
| `s + s`, `t += s`, `-m`, `!m`, `++m`, `m++`, `x = y`, `a == b`, `a <=> b`, `&m` (overloaded) | one | the operator token (`and` and digraphs as written) | the selected `operator@`, member or not, a hidden friend, an operator template's specialization |
| `a = b` with an implicitly declared copy or move assignment, `a == b` with a defaulted or implied `operator==` | one | the token | the implicit or defaulted declaration ([class.copy.assign], [class.compare.default]) |
| `a[b]`, `a[b, c]` | one | the `[` (`<:`); `close(id)` the `]` | `operator[]` |
| `f(x)` on a class object, a lambda, a static or explicit-object `operator()` | one | the `(`; `close(id)` the `)` | `operator()` (a generic lambda's specialization, `found` the template) |
| `c(x)` through a surrogate call function ([over.call.object]/2) | one | the `(`; `close(id)` the `)` | the conversion function to a pointer or reference to function |
| `p->m` through `operator->` ([over.ref]/1) | one per `operator->` applied, in application order | the one `->` token | each `operator->` |
| `m, 4` with an overloaded `operator,` | one | the comma | `operator,` |
| a rewritten comparison | one or two (§18.13.3) | the written token | the selected candidate |
| a user-defined literal | one per suffixed piece (§18.13.4) | the suffix | the literal operator |
| a built-in operator, an implicit conversion | none | | |

Commas between the arguments of a multi-argument `operator[]` or a
call are separators, not operators. clang builds the overloaded comma
of a C-style cast's parenthesized operand (`(void)(m, 4)`) at the `(`
(`ParenListExpr`); kiln builds the call there too, and the entry is at
the comma itself. Operators in requires-expressions, unevaluated
operands (`sizeof`, `decltype`, `noexcept`) and default arguments are
recorded once at their written site, like names. In a macro an
operator token keeps its token's location (§18.5): the dump shows its
spelling position and `exp=` the outermost expansion.

#### 18.13.3 Rewritten and reversed comparisons

`a != b` selecting `operator==`, `a < b` (`>`, `<=`, `>=`) selecting
`operator<=>`, and the reversed synthesized candidates
([over.match.oper]/3.4, /8, /9) have an entry at the written token
naming the selected candidate, flagged `REWRITTEN`, and `REVERSED`
too when it is the reversed synthesized candidate (the standard counts
those among the rewritten ones, so `REVERSED` always comes with
`REWRITTEN`). Reversed: the selected call's first argument is the
written right operand.

When a relational comparison (or a reversed `<=>`) is rewritten
through `operator<=>` and the comparison of its result with zero is
itself an overloaded operator (`(a <=> b) < 0` with a user-defined
comparison category, or the standard categories' hidden friends), a
second entry at the same token names that comparison, after the first
(application order), flagged `REWRITTEN` and never `REVERSED` (it is
`0 @ (y <=> x)` when the first is reversed, not a reversed candidate
itself). A built-in comparison with zero (an integral `<=>` result)
gives no second entry. `!=`'s negation is the built-in `!`
([over.match.oper]/9).

The facts are read from the built `CXXRewrittenBinaryOperator` (its
semantic form, §5): the `!` peeled for `!=`, the `<=>` call at the
token among the operands of the comparison with zero.

#### 18.13.4 User-defined literals

| literal | entity |
|---|---|
| integer `12_k`, floating `1.5_f` (cooked) | the literal operator |
| `42_r` with only `operator""_r(const char *)` (raw) | the raw literal operator |
| `42_t` with `template <char...> operator""_t()` | the specialization `operator""_t<'4', '2'>`, `found` the template |
| character `'x'_c` and string `"ab"_s` of any encoding prefix, raw strings | the literal operator |
| `"abc"_f` with `template <Fixed F> operator""_f()` ([over.literal]/5) | the specialization, `found` the template |

- **Range**: the suffix's own source bytes, from its first character
  to the literal token's end. The start is found through the lexer's
  spelling map (`kiln_lex::source_offset`) when the token was cleaned,
  never as the end minus the suffix's length: a line splice inside the
  suffix (`5_\` newline `n`) is inside the range, as for an identifier
  with a splice, and a universal-character-name (`6_\u00e9`) is kept
  verbatim in the spelling, so its bytes map one for one. The parser
  passes the same location to sema as the literal's ud-suffix location
  (clang's `UDSuffixLoc`), where the literal operator's callee
  `DeclRefExpr` is: the callee joins its entry (§18.11.1).
- **Editable range** (`SourceManager::editable_file_range`, spec/source.md):
  the suffix's bytes for a literal in a file and for one in a macro
  argument (mapped to the argument's spelling); `None` for one in a
  macro body or made by `##`, whose suffix does not begin at an
  expansion's start. A client renaming `operator""_n` edits the
  entries with a range and reports the others.
- **Concatenation** (`"a" "b"_s "c"_s`): one entry per piece that
  carries the suffix, all naming the one literal operator
  ([lex.string.concat]/2).
- **No literal operator** (diagnosed): an entry, no entity,
  `UNRESOLVED`.
- **`how`**: `unqualified` (literal operator lookup is unqualified,
  with no argument-dependent part), `udir=` from the scope walk
  (§18.7.3). A user-defined literal is never dependent, so it has no
  per-specialization rows.

The entry starts inside a token: a client must not assume entries
start at token boundaries (the dump prints character positions).

#### 18.13.5 Failures

An operator expression whose overload resolution ran and failed (no
viable function, ambiguity, a deleted function: diagnosed) has an
`UNRESOLVED` entry at its token with no entity (and its `close` for
`[]` and `()`). Resolution ran when an operand has class or
enumeration type ([over.match.oper]/1; for `[]` and `()`, the object),
no operand is type-dependent and none contains an error. `p->m` on a
class with no `operator->` is `UNRESOLVED` too (clang's recovery
treats it as `.`). A failed built-in operator (`p + p` on pointers)
and one with an erroneous operand (`missing + a`) have none.

A new-, delete- or await-expression that failed (diagnosed: no
matching or an inaccessible allocation or deallocation function, an
ambiguous `operator co_await`, but also any other error in it, such as
its initialization or an awaiter's member, which the failed expression
does not tell apart) has an `UNRESOLVED` entry at its keyword (`::` as
its lead for `::new` and `::delete`), unless one of its operands
(placement arguments, initializer, the deleted pointer, the awaited
operand) contains an error: `delete missing` has none.

#### 18.13.6 Templates and later stages

In a template pattern a non-dependent operator (`s + s` with `s` of a
non-dependent class type) is resolved at the definition
([temp.res.general]/1) and recorded once there; nothing is recorded
for it during instantiation.

**Dependent operators (stage 6b, WP-2910).** An operator with a
type-dependent operand is a dependent name ([temp.dep.general]/2,
[temp.dep.expr]/1), bound at instantiation ([temp.dep.candidate]/1):

- **Entry.** A `DEPENDENT` entry of role `operator` at its token (the
  ranges of §18.13.2; `close` for `[]` and `()`), no entity, made when
  the operator could select an operator function in some
  specialization: an operand whose type is a class, an enumeration or
  dependent and not a pointer, array or function type (for `[]`, `=`
  and `()`: the object or callee only, [over.sub]/1, [over.ass]/1,
  [over.call]/1), and no operand containing an error. A fold over
  `int` non-type template parameters, `p + 1` with `p` of type `T *`,
  `a[i]` with `a` of type `T[3]` have no entry.
- **Found set** (§18.12.2). For an operator other than `=`, `[]`, `()`
  and `->`: the non-member candidates unqualified lookup of
  `operator@` found at the definition (`Ast::dependent_oper_candidates`,
  [over.match.oper]/3.2), flag `ADL`, `udir=` for a candidate made
  visible by a using-directive. `=`, `[]`, `()` and `->` have no set:
  their candidates are members only.
- **Rows** (§18.9). Each specialization whose instantiation reached the
  token has rows, keyed by `Ast::written_spec_key()` (a generic
  lambda's call operator specialization, a concept for a
  requirement's satisfaction check): the operator function selected,
  `how=member` for a member (a member operator, `operator[]`,
  `operator()`, `operator->`, a surrogate's conversion function),
  `how=adl` when the call's `ADL` flag says argument-dependent lookup
  alone found it, `udir=` for one the definition's using-directive
  made visible; a rewritten or reversed candidate
  ([over.match.oper]/3.4, /8) with `REWRITTEN` and `REVERSED` (one
  specialization may select `operator!=` directly and another
  `operator==` rewritten), two rows for `a < b` through `operator<=>`
  whose result has a user `operator<` (the `<=>`, then the `<`). A
  specialization that selected a built-in candidate ([over.built]),
  or called through a pointer to function, gets a row with flag
  `SPEC_BUILTIN`, no entity and no `found`, `how=builtin`, dumped
  `(builtin)`. An ill-formed specialization whose operator selects
  nothing ([over.match.oper]/3 finds no viable function, `->` on a
  class with no `operator->`) gets an `UNRESOLVED` row; one whose
  operand failed gets none. A failed substitution in a SFINAE context
  ([temp.deduct.general]/8) records no row. A specialization in which
  the operator is still dependent (a generic lambda's pattern inside a
  specialization) re-keys its rebuilt node (`PatternNode::Oper`,
  §18.9.4) for the next level.
- **Keyed rows** (§18.14). An operator token in a partial
  specialization's argument list is reached by a match, not by a
  specialization: with keys on, the match's `PartialMatch` key gets the
  token's row, with the same facts (`shl<T, void_t<decltype(val<T>() <<
  val<T>())>>` matched by `shl<a::X>`: `'<<' -> a::operator<<
  key=partial:shl@...<a::X, void> how=adl`), and the key's root holds
  the operator call with its callee.
- **Reached.** For an instantiated specialization a row exists exactly
  when the instantiation reached the token. A discarded `if
  constexpr` branch ([stmt.if]/2) is never reached and has none; a
  template never instantiated has the entry and its set only.
- **Folds.** Each operator token of a fold-expression
  ([expr.prim.fold]/1) is an entry (a binary fold's two tokens, two
  entries, `PatternNode::Oper` slots 0 and 1). The expansion
  ([temp.variadic]/11) applies the operator once per element pair;
  rows are kept once by content (§18.9.1), so a specialization has one
  row per distinct resolution: `sum(s, 1)` and `sum(s, s, s)` one
  each, a heterogeneous pack a row per operator function plus a
  `(builtin)` row for its built-in pairs. An expansion that applies no
  operator (an empty pack of `&&`, `||` or `,`, [temp.variadic]/12; a
  single element; an init with an empty pack) still reached the token
  and gets a `(builtin)` row, so that "a row exists exactly when the
  token was reached" holds for folds too (the design's "no row" for
  this case is replaced).
- **`->` chains.** `t->m` with a type-dependent `t` is an entry at
  `->`. A specialization whose `t` is a class applies `operator->`
  repeatedly ([over.ref]/1, [over.match.oper]/2): one row per
  `operator->`, pushed in application order (the dump sorts them, the
  API `specs_of` keeps the order). A pointer `t` gets one `(builtin)`
  row. An erroneous result records nothing.
- **The `(` rule.** A `DEPENDENT` entry at `(` is made when the callee
  is not a function name (not an `UnresolvedLookupExpr`,
  `UnresolvedMemberExpr`, `DependentScopeDeclRefExpr`,
  `CXXDependentScopeMemberExpr`, nor a `DeclRefExpr` or `MemberExpr`
  naming a function or function template) and the call is
  type-dependent through the callee or an argument (`Less lt;
  lt(a, b)`, `less<>{}(a, b)`, `t(1)` in a template). Its rows name
  the `operator()` selected (a member template's specialization,
  `found=` the template), a surrogate call's conversion function
  ([over.call.object]/2), or `(builtin)` for a pointer to function.
  **Not covered** (WP-3080): `t.f(x)` and `T::f(x)` where `f` turns
  out to be a data member of class type in a specialization: its name's
  row names the data member, and the `operator()` it calls has no entry
  or row.

Measured on `<bits/stdc++.h>` (§18.13.8), the `SPEC_BUILTIN` rows
(5,984) are fewer than the new entries (17,691), so the rows are kept
and the design's fallback (one entry flag `BUILTIN_IN_SOME_SPEC`) is
not used.

**New and delete (stage 6c, WP-2911).** The `new` keyword of a
new-expression (range the keyword) names the allocation function it
selected ([expr.new]/12: looked up in the scope of the allocated class,
or of an array's element class, then in the global scope; `::new` the
global scope only), and the `delete` keyword of a delete-expression
(`delete[]` one entry over its three tokens, `delete [ ]` as written)
the deallocation function ([expr.delete]/9-11: the usual, sized,
aligned or destroying form). `how` is `qualified` (a lookup in a named
scope, not the scope walk); `::new` and `::delete` have qualifier
`GLOBAL` with the `::` as lead (§18.11.2). The implicitly declared
global replaceable functions ([basic.stc.dynamic.general]/2) and those
of `<new>` are functions like any other; an allocation function
template's specialization (`template <class A> void *operator
new(size_t, A &)`) has `found=` the template. The entry of `delete p`
names the deallocation function the lookup in the static type's scope
selects ([expr.delete]/9-10); under a virtual destructor the call may
reach another (the one found at the dynamic type's destructor), so a
client must not read it as "the function called". The deallocation
function a new-expression calls if the initialization throws
([expr.new]/27), the destructor a `delete` calls and a deleting
destructor's own lookup have no token of their own: no entry. Not yet
(follow-up): a class-scope allocation function found through a
using-declaration (`using B::operator new;`) has `found` empty, not
the using-shadow (the new-expression keeps the function only).

A new-expression whose allocated type or an operand is dependent, and
a delete-expression whose operand is type-dependent ([temp.dep.expr]/3),
select nothing at the definition: a `DEPENDENT` entry with no found set
(nothing is looked up at the definition) and the pattern node
`PatternNode::Oper(e, 0)` of the expression. Each specialization that
reaches it has a row naming the function selected, `how=qualified`
(`spec=make<C>` names `C::operator new`, `spec=make<int>` the global
one). A specialization of a dependent one that selected no function
would get a `SPEC_BUILTIN` row; every valid specialization selects one
(the global functions count), so none is expected in practice. A
non-dependent new- or delete-expression in a template (`new int`) is
resolved at the definition and has no rows.

**Await-expressions (stage 6c, WP-2911).** The `co_await` keyword of
`co_await e` names the `operator co_await` applied ([expr.await]/3.3:
a member, `how=member`; a non-member found by unqualified lookup, or by
argument-dependent lookup alone, `how=adl`), to the operand or to the
result of the promise's `await_transform` ([expr.await]/3.2). When none
applies (the awaitable is its own awaiter) the keyword has no entry.
The calls of `await_transform`, `await_ready`, `await_suspend`,
`await_resume` and the promise's other members, `co_yield` (its
`yield_value` and any `operator co_await` applied to it,
[expr.yield]/1) and the initial and final suspend points
([dcl.fct.def.coroutine]/5) have no written token of their own: no
entry. In a coroutine template every await-expression is a
`DependentCoawaitExpr` (the promise is known only per specialization):
a `DEPENDENT` entry whose found set is the unqualified lookup of
`operator co_await` at the definition, re-run at the recording site
(flag `ADL`), with the pattern node `PatternNode::Oper(e, 0)`; each
specialization that reaches it has a row naming the `operator
co_await` applied, or a `SPEC_BUILTIN` row when none applies. A
discarded `if constexpr` branch has none, as for any dependent
operator.

#### 18.13.7 Recording sites, `how` and the operator join

- **Sites.** Recorded from the parser sites that consumed the token,
  never from the sema actions (which also run for implicit calls and
  during instantiation): `parse_rhs` (binary operators, not `?:`),
  `parse_braced_rhs` (`a = {...}`), `parse_paren_list_rest` (each
  comma at its own token), the prefix unary operators, postfix `++`
  and `--`, subscripts and calls (with the closing token), `->`
  (after the member access is built), and the literal parsers
  (`parse_number`, `parse_char`, `parse_string_literal`). Each calls
  `Sema::note_written_operator`, `note_written_arrow` or
  `note_written_udl` (`kiln_sema::written_refs_oper`) with the
  expression it built, which read the facts from it (no sema side
  state): a `CXXOperatorCallExpr` at the token's location, a
  `CXXRewrittenBinaryOperator`, a surrogate `CallExpr` (its callee the
  object under a user-defined conversion), the `operator->` calls at
  the `->` inside the member access (also inside a failed access's
  recovery), a user-defined literal's call. Off, each site is one
  branch on `records_written_refs()` (the subscript and call sites
  build their operand list only when on).
- **Stage 6b sites.** The same parser sites make a dependent operator's
  entry (`note_written_operator`, `note_written_arrow`: a type-dependent
  expression is not resolved, so the entry, its found set from the
  expression's candidate set, and the pattern node
  `PatternNode::Oper(e, 0)` are recorded instead), and the two
  fold-expression sites of `parse_paren_rest_inner` call
  `note_written_fold` with the fold's operator tokens (slots 0 and 1).
  The rows are made by
  `Substituter::expr` and `Substituter::callee` after each rebuild
  (`note_written_spec_oper`, beside `note_written_spec_expr`) and by
  the fold expansion (`fold_combine` per application,
  `fold_applied_none` for an expansion applying none), all behind
  `has_pattern_nodes()`.
- **Stage 6c sites.** `parse_new_expr` and `parse_delete_expr`
  (`crates/kiln_parse/src/new_delete.rs`) call
  `Sema::note_written_new_delete` with the expression built, the
  keyword's range (through the `]` for `delete[]`) and the `::` of
  `::new` / `::delete`; the `co_await` site of the cast-expression
  parser calls `Sema::note_written_coawait`. The facts are read from the
  `CXXNewExpr` (`NewExtra::operator_new`), the `CXXDeleteExpr`
  (`operator_delete`) and the `CoawaitExpr` (its awaiter, when it is an
  `operator co_await` call at the keyword); the rows of the dependent
  ones come from the same `note_written_spec_oper` hook, which reads the
  rebuilt expressions the same way. Off, each site is one branch on
  `records_written_refs()`.
- **`how`.** `member` when the function is a member (a member
  candidate, [over.match.oper]/3.1; `operator[]`, `operator()`,
  `operator->`, a surrogate's conversion function); `adl` when the
  call's `ADL` flag says argument-dependent lookup alone found it
  (clang's `IsADLCandidate`); else `unqualified`, with `udir=` from
  the scope walk (§18.7.3). Not yet distinguished (WP-2920): `both` (found by
  unqualified and argument-dependent lookup) and the associated sets
  (`adl-ns=` and the others); an entry found by both prints
  `unqualified` (or `found=` the using-shadow).
- **The operator join.** For every `CXXOperatorCallExpr` of the code
  as written (`for_each_written_expr`, instantiations left out) that is
  not type-dependent, whose callee names a function and whose location
  spells its operator (or is a rewritten comparison's), some entry of
  role `operator` with `range.lo` at that location names the callee's
  function. The calls with no token of their own are the exceptions
  (a range-for's, a defaulted comparison's body, the comma of a cast's
  parenthesized operand, whose call is at the `(`). Test
  `every_operator_use_joins_its_entries`
  (`crates/kiln_parse/tests/written_refs_accounting.rs`), over the
  same files as the name join (§18.11.1). Stage 6c adds: every
  new- and delete-expression of the code as written that selected a
  function has an `operator` entry naming it whose qualified name
  begins at the expression's location (`qualified_lo`: its `::`, else
  its keyword); an explicit `co_await`'s `operator co_await` call is a
  `CXXOperatorCallExpr` at the keyword and joins by the rule above,
  while the implicit ones (`co_yield`'s, the suspend points') are not
  at a `co_await` token and are exceptions.
- **Identifier accounting** (§18.8) is unchanged: operator tokens are
  not identifiers, and a ud-suffix is inside its literal token.

#### 18.13.8 Dump, carriage and cost

The dump (§18.6) prints `close=` after `tilde=` and `rewritten`,
`reversed` after `dep`:

```
31:4-31:5  operator  '[' -> lib::V::operator[]@11:8 close=31:6 in=use@30:6 how=member
30:5-30:6  operator  '<' -> operator<=>@20:3 in=use@25:6 rewritten
30:5-30:6  operator  '<' -> operator<@21:6 in=use@25:6 rewritten
33:5-33:7  operator  '==' -> operator==@23:6 in=use@25:6 rewritten reversed
44:6-44:8  ud-suffix  '_t' -> units::operator""_t<'1', '2', '3'>@21:36 found=units::operator""_t@21:36 in=use@38:6 udir=units@36:1
```

**Carriage.** The entries carry two new role values (validated by
`RefRole::from_u8`) and two flag bits; the leads gain the `close`
column (`RefLead` 16 to 20 bytes beside WP-2828's `spec`, a row only
for an entry with a `::`, a `~`, a specifier or a closing token),
validated to be on an `operator` entry, in the PCH section and as the
fifth word of the image's leads. PCH `FORMAT_VERSION` 164,
`IMAGE_VERSION` 88. Stage 6b (WP-2910) adds the pattern node
`PatternNode::Oper(e, slot)` (§18.9.4) to the snapshot's section and as
the image's node tag 3 (entry, expression, slot; a slot of 2 or more is
rejected), so a consumer's or importer's instantiations of a dependent
operator from the PCH or module record their rows: PCH
`FORMAT_VERSION` 168, `IMAGE_VERSION` 91. The flag `SPEC_BUILTIN` is a
`SpecRef::flags` bit and needs no new column.
`Inputs/refs_pch.h` and `Inputs/refs_mod.cppm` end with an overloaded
`+`, a `[]` with its close and a ud-suffix, then a dependent `t + t`
and a fold `(t + ...)` the consumer instantiates, so `pch.refs` and
`module.refs` show them, their found sets and the consumer's rows
through the snapshot and the image and as text.

**Cost.** Measured on `<bits/stdc++.h>` (`-std=c++26 -fsyntax-only`).
With the table off nothing is recorded: the parser's sites test
`records_written_refs()` before building an operand list, total live
heap is byte-identical to before (42,928,651 bytes), the Oven's
counters (allocations, bytes, peaks, calls, memo hits; everything but
wall time) are identical on all 29 `bench/oven` programs (34,176
evaluations), and wall time is within run-to-run noise (median 11.48 s
before, 11.62 s after, interleaved, overlapping ranges; peak RSS equal
at about 108 MB). With the table on, the header adds 1,275 entries to
209,806 (+0.6%: 1,248 `operator`, 27 `ud-suffix`; 58 with a `close`,
94 `rewritten`), 39,768 bytes of live table (8,687,200 to 8,726,968)
and 0.08% of total live heap (51,615,851 to 51,655,619 bytes); the
`close` column adds a lead row only for `[]` and `()` entries. These
were measured before WP-2828's `spec` column; beside it a row is 20
bytes, so `close` also costs 4 bytes on each of the header's other
rows (704 rows, 2,816 bytes).

Stage 6b (WP-2910), measured the same way against its base (WP-2820's
tip). With the table off nothing new is recorded: the parser's fold
sites and `note_written_operator` test `records_written_refs()` first
and the substituter's and fold expansion's hooks test
`has_pattern_nodes()` (an empty map when off); the memory report is
byte-identical (total live heap 42,928,549 bytes); the Oven's counters
(everything but wall time) are identical on all 29 `bench/oven`
programs (16,612 report lines); CPU time is within run-to-run noise on
a loaded host (median 12.82 s before, 12.59 s after, 9 interleaved
rounds, ranges 10.99-16.82 s and 11.15-15.72 s; peak RSS equal at about
108 MB). With the table on, the header adds 17,691 dependent `operator`
entries to 211,081 (+8.4%; `=` 2,195, `*` 2,097, `(` 1,917, `==`
1,283, ...), 7,114 rows from the header's own instantiations (5,984
`SPEC_BUILTIN`, 1,130 naming a function, none unresolved), 2,083,196
bytes of live table (8,730,076 to 10,813,272: the entries, their
pattern nodes, found-set rows and the rows) and 4.0% of total live heap
(51,658,625 to 53,741,821 bytes). The `SPEC_BUILTIN` rows (5,984) are
fewer than the new entries (17,691): the rows are kept (§18.13.6).

Stage 6c (WP-2911), measured the same way against its base (WP-2910's
tip), with debug builds. With the table off nothing new is recorded:
`note_written_new_delete` and `note_written_coawait` test
`records_written_refs()` first and the substituter's hook tests
`has_pattern_nodes()`; the memory report is byte-identical (total live
heap 43,094,089 bytes); the Oven's counters (everything but wall time)
are identical on all 29 `bench/oven` programs (234 report lines, 16,583
evaluations); wall time is within run-to-run noise on a loaded host
(median 11.41 s before, 11.07 s after, 5 interleaved rounds). With
the table on, the header adds 151 entries (128 dependent, 23 naming a function; `new` 100, `delete` 23,
`delete[]` 28; no `co_await`, which the header does not use) and 55 rows
from its own instantiations (all naming a function, none
`SPEC_BUILTIN`, none unresolved): `written_refs` 228,772 to 228,923
items, 14,856 bytes of live table (10,569,108 to 10,583,964) and 0.03%
of total live heap (53,663,197 to 53,678,053 bytes). Nothing is
carried that the snapshot and the image did not already carry, so
neither `FORMAT_VERSION` nor `IMAGE_VERSION` moves.

#### 18.13.9 Tests

`crates/kiln_ast/src/written_refs.rs`
(`operator_entries_keep_their_close_and_flags`: roles round trip,
`close`, the flags' dump, `RefLead` 16 and `WrittenRef` 28 bytes),
`written_refs_carry.rs` and `crates/kiln_sema/src/bmi_image/refs.rs`
(closes round trip; a close on an entry that is no operator is
rejected), `crates/kiln_parse/tests/written_refs.rs`
(`ud_suffix_ranges_are_the_suffix_bytes`: the editable ranges in a
file, a macro argument, a macro body, from `##`, across a splice, with
a UCN, raw and prefixed strings; `operator_entries_through_the_api`),
`crates/kiln_parse/tests/written_refs_accounting.rs` (the operator
join; the UDL callee joins its `ud-suffix` entry), and
`tests/conformance/written_refs/`: `operators.cpp`,
`operators-rewritten.cpp`, `operators-arrow.cpp`,
`operators-comma.cpp`, `operators-special.cpp`, `udl.cpp`,
`operators-macros.cpp`, `operators-requires.cpp`,
`operators-errors.cpp`, and the carriage goldens.

Stage 6b (WP-2910): `crates/kiln_ast/src/written_refs.rs`
(`operator_nodes_and_builtin_rows`: an operator token's node apart from
its expression's, both fold slots, a built-in row kept once and apart
from an unresolved one), `written_refs_carry.rs`
(`operator_pattern_nodes_and_builtin_rows_round_trip`) and
`crates/kiln_sema/src/bmi_image/refs.rs` (the image's tag 3 round trips;
a slot of 2 and an expression past the bodies are rejected),
`crates/kiln_parse/tests/written_refs.rs`
(`dependent_operator_entries_through_the_api`: the found set, a
function row from the set and a built-in row, a `->` chain's rows in
application order, a binary fold's two entries),
`written_refs_accounting.rs` (§18.12.4 with the `SPEC_BUILTIN` and
member-operator exceptions), and `tests/conformance/written_refs/`:
`operators-dependent.cpp`, `operators-dependent-arrow.cpp`,
`operators-dependent-call.cpp`, `operators-fold.cpp`,
`operators-dependent-sfinae.cpp`, `operators-dependent-errors.cpp`
(each with an instantiation selecting a user operator, one selecting a
built-in, a template never instantiated and a discarded `if constexpr`
branch where the form allows), `found-operator.cpp`, and the carriage
goldens (`Inputs/refs_pch.h`, `Inputs/refs_mod.cppm`: a dependent
`t + t` and a fold the consumer instantiates).

Stage 6c (WP-2911): `crates/kiln_parse/tests/written_refs.rs`
(`alloc_entries_through_the_api`: a class's own and the global
functions, `how=qualified`, the range of `delete[]`, `::new`'s
`GLOBAL` qualifier and lead, a dependent `::new T`'s row),
`written_refs_accounting.rs` (the operator join over new- and
delete-expressions), and `tests/conformance/written_refs/`:
`operators-new-delete.cpp` (class-scope and global, `new[]` and
`delete[]`, `::new` and `::delete`, placement, an allocation function
template, sized, aligned and destroying deallocation, a virtual
destructor, an unevaluated operand), `operators-new-delete-dependent.cpp`
(rows for a class's and the global functions, `::new`, placement, a
value-dependent bound, a class template's members, a template never
instantiated, a discarded branch), `operators-coawait.cpp` (member and
non-member `operator co_await`, by ADL, none, through
`await_transform`, `co_yield`, and a coroutine template's rows with a
`(builtin)` one), `operators-new-delete-errors.cpp`, and the carriage
goldens (`Inputs/refs_pch.h`, `Inputs/refs_mod.cppm`: a class's own
`new` and `delete`, `::new` and `::delete`, and a dependent
`delete new T` the consumer instantiates, through the snapshot and the
image). The entries reuse the role, qualifier, lead and pattern-node
shapes of stages 1 to 6b, so neither carriage changes (no
`FORMAT_VERSION` or `IMAGE_VERSION` bump).

### 18.14 Substitution keys (WP-3540)

A per-specialization row (§18.9) says what a dependent name resolved to
when a *specialization* was instantiated. Four substitutions of a
template pattern form no specialization of their own, and their rows
are keyed by the template today (§18.9.2): matching a class or variable
template partial specialization against a specialization's arguments
([temp.spec.partial.match]), checking associated constraints or a
concept-id for satisfaction ([temp.constr.constr]), substituting a
default template argument ([temp.param]/10) and substituting an alias
template's type-id ([temp.alias]/2). Tooling clients need, for each such
substitution that completed, what each dependent name of the pattern
bound to under it. The keyed table records that beside `specs()`, opt-in
under the table: `-fkiln-written-ref-keys` (driver),
`ParseOptions::written_ref_keys`, `FrontEndOptions::written_ref_keys`;
it implies `-fkiln-written-refs`.

`specs()`, its rows, their ids and their dump lines are not changed by
the keyed table, with the sub-switch on or off.

Stage 0 (WP-3540) built the table, its key scopes, its carriage and its
dump. Stage 1 (WP-3541) opens the scopes of the first kind,
`PartialMatch`, and gives every key a root (§18.14.10). Each later stage
opens the scopes of one more kind at the sites of §18.14.3's table and
states which completed substitutions of that kind get a key.

#### 18.14.1 The key

`WrittenRefs::subst_keys()` yields the keys (`SubstKey`, 24 bytes, a
`size_of` unit test), in commit order (not part of the contract: compare
sorted):

| field | type | meaning |
|---|---|---|
| `kind` | `SubstKind` (1) | `PartialMatch` (1), `Satisfaction` (2), `DefaultArg` (3), `Alias` (4) |
| `outcome` | `SubstOutcome` (1) | `Substituted` (1): matched, satisfied or substituted; `Unsatisfied` (2), on a `Satisfaction` key only: the check completed with the answer "not satisfied" |
| `template` | `DeclId` (4) | first declaration: the partial specialization; the concept or the constrained declaration as the satisfaction check gets it (a template, a partial specialization, a member of a class template specialization as itself); the template whose parameter has the default; the alias template |
| `args` | `ArgListId` (4) | the canonical arguments (`Types::canonical_args`): the specialization's being matched; the check's; the arguments before the defaulted parameter; the alias template's converted arguments |
| `extra` | `u32` (4) | `PartialMatch`: the deduced argument list (`SubstKey::deduced`, a payload); `DefaultArg`: the parameter (`SubstKey::parameter`); else 0 |
| (rows) | `(u32, u32)` (8) | the key's run of rows: start and length, which may be 0 |

A key's **identity** is (kind, template, arguments, and the parameter of
a `DefaultArg`); `outcome` and a `PartialMatch`'s deduced list are not
part of it. One identity has one key. Keys are formed for **concrete
arguments only**: a substitution with a dependent argument is part of
parsing the enclosing pattern, and its names are resolved again by a
later substitution with a key of its own. A key exists for a
substitution that **completed**, whether or not any row was recorded
under it, and for no substitution that failed: a satisfaction check that
completed unsatisfied has a key (`Unsatisfied`), a partial specialization
that did not match has none.

**Which substitutions get a key.** The general rule, for every kind: a
completed substitution with concrete arguments whose pattern is
instantiation-dependent. A partial specialization's argument list always
names the partial's parameters, so for `PartialMatch` the rule is: every
match that completed has a key, whether or not a row was recorded under
it, and there is no test of the pattern's entries. A match **completed**
when deduction succeeded, the substituted argument list equals the
specialization's arguments ([temp.spec.partial.match]/2,
[temp.deduct.type]/1), the substitution did not fail and the partial
specialization's associated constraints are satisfied. Class and
variable template partial specializations are alike. For `PartialMatch`:
`template` is the partial specialization's first declaration, `args` the
specialization's canonical arguments, `extra` the deduced argument list.
A member partial specialization of a class template specialization
(the partial specialization instantiated with its enclosing class) is
keyed by that member of the specialization, as the match gets it.
Stage 1 opens `PartialMatch` scopes only; the other kinds' rows below
arrive with their stages.

Read side: `subst_key(id)`, `subst_key_len()`, `subst_key_of(kind,
template, args, extra)` (the identity index), `subst_rows(id)` (the run,
contiguous), `subst_rows_of(entry)`, `subst_rows_naming(decl)` (as
`entity` or `found`), `subst_adl_set(row)`, `subst_keys_on()`,
`subst_keys_complete()` (§18.14.5). A `DefaultArg`'s parameter and a
`PartialMatch`'s deduced list are read through `SubstKey::parameter()`
and `SubstKey::deduced()`; the partial specialization's primary template
through `Ast::partial_spec_info(key.template)`; the key's root through
`subst_root(id)` (§18.14.10).

#### 18.14.2 The row

`subst_rows(key)` yields the key's rows (`SubstRef`, 28 bytes, a
`size_of` unit test): a `SpecRef` (§18.9.1) without `spec`, with the
same fields and meanings.

A keyed row carries what a per-specialization row (§18.9.1) carries:
the pattern's entry (`entry`); the entity the name denotes under the key
(`entity`), with `found` when lookup found something else; for an
unqualified call the lookup facts of the key's substitution, `how` and
the associated set (`adl`), the definition-time found set being the
entry's (`found_at_definition(entry)`, §18.12) and the row one selection
from it (§18.12.4); `udir`; `naming_class` (the lookup-context class of a
resolved `Member` or `Qualified` class lookup, canonical first
declaration, §18.9.1); and, for an entry that was substituted and
is still dependent, `SUBST_DEPENDENT` with no entity.

`flags` holds `SpecRef::flags`' bits and two of its own, in bits
`SpecRef` does not use:

- `SUBST_DEPENDENT` (1): the entry was substituted under the key and is
  still dependent; its name is resolved by a later substitution. No
  entity, `how` `None`.
- `SUBST_EXPANDED` (64): the row's entry is not written in the key's own
  template. It was reached through an alias template-id that the key's
  pattern keeps only as its expansion ([temp.alias]/2). The table notes
  the pattern nodes so reached (`expanded_nodes`, filled only with keys
  on): a node a rebuild leaves dependent while the innermost frame is an
  alias template's, or one rebuilt from such a node. A row made from one
  gets the flag unless its key is the `Alias` key of the template that
  holds the entry.

Under a key, an entry has a row when its name bound during the key's
substitution; an entry the substitution did not reach, or whose name
did not bind, has none. A name that resolves to nothing fails the
substitution (§18.14.3), so a committed key of the kinds other than
`Satisfaction` has no unresolved row.

#### 18.14.3 Key scopes

The substituter keeps a stack of **key scopes** (`Sema::key_scopes`,
transient, empty at rest and at a snapshot point), one per keyed
substitution in progress, each with a private row buffer
(`kiln_sema::written_keys`):

- **Opening** (`key_scope_open(kind, template, args, extra, payload)`), where
  the substitution's frame has its arguments. The scope is owned by the
  innermost instantiation frame (`InstFrame::key_scope`). It records
  (`Record`) unless its identity is already committed or already open
  further out (`Skip`: the outer run substitutes everything itself).
  With the sub-switch off it returns `None` after one bool test.
- **Attribution** (`Sema::written_row_keys`). A name resolved now has up
  to two row targets: the specialization `written_spec_key()` gives
  (§18.9.2, unchanged) and the key scope found by one walk from the
  innermost frame: a lambda substitution, a structured binding's
  initializer and a requirement's instantiation are transparent; a frame
  that owns a scope gives it; a constraint substitution continues
  outward only to the satisfaction check of the same entity; any other
  frame, every real instantiation included, ends the walk with none. A
  nested real instantiation therefore records in `specs()` under its own
  specialization and nothing in the enclosing key. Every recording site
  of §18.9.4 and §18.13.7 builds the row once and gives it to each
  target. With no scope open the walk is one test of an empty stack.
- **Kept once.** A row equal to one already in the buffer is not added.
  An overload set resolved later (§18.9.4's deferred resolution) patches
  its buffered row while the scope is open; if the scope closed
  meanwhile the keyed half is dropped.
- **Failure inside.** Under a scope, a name that resolves to nothing (an
  unresolved name, a failed call, a substitution failure the table
  records nothing for, §18.9.3) adds no row and *poisons* the scope.
- **Closing** (`key_scope_close(id, completed)`), in stack order, where
  the outcome is known. The buffer is committed as the key's run, also
  when empty, when the substitution completed, the scope records, it is
  not poisoned and no error was diagnosed since the open; otherwise the
  buffer is dropped and nothing is written to the table. A committed
  identity is never recorded again: later runs open `Skip`.
- **Suspended** (`key_scope_suspend`/`resume`): a check run inside a
  substitution that is not part of it records into no scope. A
  suspension is a depth: it silences the scopes open when it began (the
  scope stack's depth is remembered, nested suspensions keep the deepest);
  a scope opened during the suspension records as usual, so its key is
  not committed with rows missing.
- **The context of a scope.** For a row whose only target is a key scope,
  the declaration context the scope was opened in is the substitution's
  own, not a template pattern of its own (`written_ctx_dependent`); for
  a row with a specialization the test is §18.9.4's.

A key's outcome and rows are those of its first completed run, first in
translation order: the earliest point in the translation of the unit at
which the substitution is performed. Two parses of the same text
therefore give the same keys, outcomes and rows.

**`PartialMatch`** (stage 1). `deduce_partial` (`templ/order.rs`) opens
the scope after the deduced arguments are set on the deduction frame,
before the partial's arguments are substituted and compared
(`partial_args_match`), and closes it exactly once on every path after
the open: with `completed` false when the comparison fails or the
substitution fails, or when the constraint check fails; true otherwise.
The scope outlives the deduction frame: the frame is popped before the
partial's associated constraints are checked, and the constraint check
runs while the scope is open but reaches no frame that owns it, so the
names of the partial's requires-clause have no `PartialMatch` row (their
satisfaction is keyed by `Satisfaction`, a later stage). Partial
ordering ([temp.spec.partial.order]) substitutes and deduces again
without an instantiation frame of its own: it runs suspended and records
nothing. A repeated match of one (partial specialization, arguments)
opens `Skip`.

Where each kind's scope opens and closes (stage 1 opens `PartialMatch`
only):

| kind | open | completed |
|---|---|---|
| `PartialMatch` | after the deduced arguments are set in `deduce_partial` (`templ/order.rs`) | the substituted arguments equal the specialization's, no substitution failure, the partial's constraints satisfied |
| `Satisfaction` | after the check's arguments are set in `satisfy` (`templ/concepts/sat.rs`), not for dependent levels, a partly failed parameter mapping or a translation-unit entity | the check produced an answer, satisfied or not |
| `DefaultArg` | after the prior arguments are set in `default_template_arg_in` (`templ/args.rs`) | substituted, and converted where the site checks |
| `Alias` | after the alias template's frame is pushed in `alias_template_type` (`templ/class.rs`), arguments not dependent | the type-id substituted |

#### 18.14.4 Row order

`subst_rows(key)` is in the order the key's own substitution reached the
names, an atomic constraint answered from the cache contributing its
rows at the atom's position in the order its evaluation recorded them.
A key's root (§18.14.10) has its slots in the order of the written
arguments.
The order depends on the key's pattern and arguments only: not on other
keys, not on the order of instantiations, not on which answers came from
a cache. (A buffer holds one key's rows in recording order.) The order
*of keys* is commit order and is not part of the contract.

#### 18.14.5 Rollback, carriage and gaps

**Rollback.** `WrittenRefs::truncate` drops every key with a row of a
dropped entry, whole, and compacts the rows; the other keys keep their
order and are found again by identity. Expanded nodes of dropped entries
go. A dropped key's root goes with it (its span and slots; the slots of
the keys kept are compacted). No key is committed inside a tentative
parse that is then rolled back past its template (debug-asserted).

**PCH snapshot.** The section (§18.10.1) carries `keys_on` (whether the
producer recorded keys), the keys and rows as they are, the roots (one
span per key and the slots; whole arenas are carried, so a root's nodes
are there) and the expanded nodes (sorted); the identity index is
rebuilt. A restore checks, each an `Invalid` error: the keys' runs tile
the rows in order (an empty run starts where the next one does), no
identity twice, `Unsatisfied` only on a `Satisfaction` key, every row's
entry in range and `DEPENDENT`, every set in range, every expanded node
a pattern node, one root span per key, the spans contiguous in key order
and ending at the slot count, a known slot tag, only `Args` slots on a
`PartialMatch` key, and the typed ids against the restored arenas (a
`DefaultArg`'s parameter a declaration, a `PartialMatch`'s deduced list
0 or an argument list, the other kinds' `extra` 0, a slot's argument
list, expression or type in range and its unused payload 0). There is
no "keys without roots" state: a key is carried with its rows and its
root, or not at all. A consumer without keys drops what was carried;
a consumer with keys continues the prefix's keys (their ids kept, the
main file's appended).

**Module images.** The `refs` section (spec/modules.md §11.11) carries
`keys_on`, the keys (kind, outcome, template record, arguments,
parameter record or deduced list, root slots) and their rows, under the
rules of `specs`, and the counts of the keys it left out. A key is
carried with its rows and its root, or not at all: one whose template,
parameter, arguments, a row's entry, entity, set or using-directive, or
a root slot the image cannot carry is left out and counted. A root's
argument lists are written as a declaration's written arguments are
(sugar kept: an alias template-id with its arguments, a `decltype` with
its operand's expression), and its nodes are those the image reaches.
An importer that records keys maps each key and commits it (an identity
it already has is kept as it is); one it cannot map is counted with the
left-out ones. The matches the importer re-runs while it builds the
image's declarations are the unit's own: a scope opened meanwhile
commits nothing, so the image's key, with its rows and root, is the one
kept. Off, the section is skipped undecoded.

**Key gaps.** `WrittenRefs::unrecorded()` (§18.10.3) gains three variants,
listed only when the consumer records keys:

- `Unrecorded::PrefixKeys { files }`: the snapshot was made with the
  table but without keys. Nothing the prefix substituted has a key.
- `Unrecorded::ModuleKeys { module, files }`: the unit's image was made
  with the table but without keys.
- `Unrecorded::ModuleKeysLeftOut { module, keys, rows }`: the unit's
  image was made with keys and left `keys` keys out, holding `rows`
  rows.

`Prefix` and `Module` already imply the key gap and are not doubled.
The three do not fail a module unit's image (`Unrecorded::keys_only()`).
`subst_keys_complete()` is true when no gap of either kind is listed.

#### 18.14.6 The dump

With keys on, after an entry's `spec=` lines (§18.9.5), its keyed rows,
sorted and without duplicates, each printed as a `spec=` line with
`key=` in its place:

```
key=partial:can_swap@4:30<n::S, void>     the primary template's name, the partial specialization's position, the specialization's arguments
key=sat:Sw@3:28<n::S>
key=default:call@2:40<n::S>#1             #<parameter index>
key=alias:A@2:26<n::S>
```

`-> (dependent)` for a `SUBST_DEPENDENT` row, ` unsatisfied` after a row
of an `Unsatisfied` key and a trailing ` expanded` for `SUBST_EXPANDED`.
Arguments print canonical, as §18.9.5's. Every key prints, after the
entries, in the `keys` block: one line `keys  key=<...> rows=<n>` (with
` unsatisfied` for an `Unsatisfied` key), then one line per root slot,
`keys  key=<...> root[<i>]=<text>`, in slot order. The blocks are sorted
by their key. A slot's text: an `Args` slot's elements as written, not
canonicalised (an alias template-id with its arguments, a `decltype`
with its operand), comma-separated, `<empty>` for none; `-` for
`NotReached`; `failed` for `Failed`; a type as written for `Type`; the
expression for `Expr`. A root's text has no source position.
The gaps print first, with §18.10.3's: `unrecorded  precompiled prefix
keys (<n> files)`, `unrecorded  module <key> keys (<main file's base
name>, <n> files)`, `unrecorded  module <key> keys left out (<k> keys,
<r> rows)`. With keys off the dump is byte-identical to the dump without
the keyed table, and with keys on it differs only by these lines.

```
22:59-22:61  operator  '<<' -> a::operator<<@15:5 key=partial:shl@22:27<a::X, void> how=adl
keys  key=partial:shl@22:27<a::X, void> rows=3
keys  key=partial:shl@22:27<a::X, void> root[0]=a::X
keys  key=partial:shl@22:27<a::X, void> root[1]=void_t<decltype(val<a::X>() << val<a::X>())>
```

#### 18.14.7 The switch and cost

Off (the table on, keys off), nothing is kept: the keyed table allocates
nothing (`subst_mem() == (0, 0)`), the memory report has no new row, and
every recording site pays one bool test or one test of the empty scope
stack. Fixed costs that exist whatever the switch: `InstFrame` grows by
4 bytes (frames are transient), the snapshot section by one byte and
three empty lengths, the image's section by one byte, two empty lengths
and two `u32` counts. The sub-switch is outside a precompiled header's
identity, like `-fkiln-written-refs`, and part of an implicit module's
cache key (`written-ref-keys`).

On, a key is 24 bytes plus its identity-index slot, a row 28 bytes. The
memory report's `written_refs.subst` row (count: keyed rows; bytes: keys,
rows, the identity index, the expanded nodes and the pending overload
sets) is shown only with keys on, and its bytes are not in the
`written_refs` row. Every stage that opens a kind's scopes reports that
row on `<bits/stdc++.h>`.

Roots (§18.14.10) are recorded exactly when keys are. On, a root is
one 8-byte span per key plus 12 bytes per slot; the memory report's
`written_refs.roots` row (count: slots; bytes: spans and slots) and
`written_refs.subst.not_completed` row (count: runs; no bytes) are shown
only with keys on. Off, the snapshot section grows by two empty lengths
and the image's key record by nothing (no key is carried).

Versions: PCH `FORMAT_VERSION` 184, `IMAGE_VERSION` 98 (stage 0); 276
and 200 (stage 1: the roots).

#### 18.14.8 Changes to `specs()`

A change that adds, removes or re-keys rows of `specs()` lists, with the
change, the new rows a `specs()` reader sees: per changed golden of
`tests/conformance/written_refs/`, the added and removed `spec=` lines,
and a one-line rule saying which names gain or lose rows. A change to
the keyed table alone shows the opposite: the `spec=` lines of the
whole corpus unchanged.

Stage 1 (`PartialMatch` keys and roots) changes the keyed table alone:
it adds no `specs()` row and removes none. With keys on, the corpus's
dumps differ from those with keys off only by key lines.

#### 18.14.9 Tests

`crates/kiln_ast/src/written_refs_subst.rs` (both sizes; a commit makes
one contiguous run, an empty run is a key between two keys and as the
last, an identity is committed once and a `PartialMatch`'s deduced list
is no part of it; truncation drops whole keys; keys off allocates
nothing and has no memory row; the gaps), `written_refs_carry.rs` (keys,
rows and expanded nodes round trip through the section and the index is
rebuilt; each validation; a consumer without keys keeps none; a section
made without keys is `PrefixKeys`), `crates/kiln_sema/src/written_keys.rs`
(the scope API driven directly: a completed close commits, with rows and
without; nothing on failure, on poison, on an error diagnosed inside;
`Skip` for a committed and for a re-entrant identity; kept once;
suspended scopes record nothing; the innermost frame decides),
`crates/kiln_sema/src/bmi_image/refs.rs` (keys round trip, every
validation, a unit without keys), `crates/kiln_parse/tests/written_refs.rs`
(keys off allocates nothing after a parse; keys on, the same dump and a
`written_refs.subst` row of 0 rows) and
`crates/kiln_driver/tests/written_refs_carry.rs` (every compiling file of
`tests/conformance/written_refs/` dumps the same with the sub-switch; a
snapshot made without keys is a `PrefixKeys` gap and one made with keys
is none; a module unit compiled without keys is a `ModuleKeys` gap and
one compiled with them is none).

Stage 1 adds: in `written_refs_subst.rs`, the slot size, roots committed
with their keys (an empty root, a refused commit, truncation) and the
not-completed counts; in `written_refs_carry.rs`, roots round trip with
every tag and each root validation; in `written_keys.rs`, suspension as
a depth and a root committed with its key; in `bmi_image/refs.rs`, roots
round trip and each root validation; in
`crates/kiln_parse/tests/written_refs.rs`, a `PartialMatch` key through
the API (its rows, partial specialization, arguments, deduced list and
a root slot reaching the call, both orders), roots reaching an operator
and a constructor, a match inside a template definition and a generic
lambda's closure; in `crates/kiln_driver/tests/written_refs_carry.rs`, a
snapshot made without keys over a prefix with partial matches; and the
`subst-partial-*` files of `tests/conformance/written_refs/` (ADL,
operators, a constructor, qualified names, two matching partials, a
constrained partial, a non-match, zero-row keys, an empty pack, a
variable template, nested matches, a match in a template, a generic
lambda, both use orders, keys off, a PCH and a module image and
replay).

#### 18.14.10 Roots

A key's rows say what each written name of the pattern bound to. A
substitution also makes selections no written name has a row for: an
operator's function, a constructor, a conversion. A **root** is a handle
on what the key's substitution built, so a reader can walk the built
nodes and see every selection. `WrittenRefs::subst_root(id)` gives the
key's root: a slice of `RootSlot`s (12 bytes, a `size_of` unit test).

| tag | `RootTag` | holds |
|---|---|---|
| 0 | `NotReached` | nothing: the substitution did not reach this part |
| 1 | `Failed` | nothing: this part failed (a kind that completes with one) |
| 2 | `Expr` | an `ExprId` (`RootSlot::expr`, `as_expr`) |
| 3 | `Type` | a `QualType` (`RootSlot::ty`, `as_type`) |
| 4 | `Args` | an `ArgListId` (`RootSlot::args`, `as_args`) |

`RootTag` is non-exhaustive. A key always has its root, and the number
of its slots is fixed by the key's pattern:

| kind | slots |
|---|---|
| `PartialMatch` | one `Args` slot per written argument of the partial specialization, in order: a written pack group element by element; a pack expansion's slot holds its elements in order, possibly none; a bare non-type template parameter's slot holds the deduced value; any other argument's slot holds the argument as substituted |
| `Satisfaction`, `DefaultArg`, `Alias` | arrive with their keys |

What a reader may rely on: a root's nodes are built by the same builders
as an instantiated body's; the names and calls in them are resolved (no
unresolved lookup node in a root reached from concrete arguments, also
when the match is triggered inside a template definition); operands are
unevaluated: nothing in a root is odr-used and no definition is
instantiated for a function it names; a generic lambda's closure type is
a valid declaration, reachable from the root. A root is built from the
canonical deduced arguments, so it holds nothing of how any use spelled
its arguments; it keeps the partial specialization's own sugar (an alias
template-id with its written arguments, a `decltype` with its operand).
The root of a key is that of its first completed run (§18.14.3).

A limit: an `Args` slot holds an expression argument as substituted,
before the conversion to the primary template's parameter type that the
comparison with the specialization's arguments may run
([temp.arg.nontype]/1); the converted value is the key's `args`.

Roots are recorded exactly when keys are: one switch. The match builds
a root only for a scope that records (`key_scope_records`); the scope
keeps its slots (`key_scope_set_root`, pooled like the row buffer) and
the close commits them with the rows. A run that records and then
commits nothing (a match whose deduction succeeded and that then did not
complete: the comparison, the substitution or the constraints failed) is
counted per kind, `subst_not_completed(kind)` and the memory row
`written_refs.subst.not_completed` (count: the sum; no bytes). It counts
runs, not identities; it is this parse's own and is not carried. A match
rejected before any substitution (the shape prefilter or deduction) is
not counted.

## 19. The declaration-extent table (WP-2090)

An opt-in side table of the source extents of every declaration as
written. It serves tooling clients that rewrite source: remove, move,
rename or wrap a declaration, insert before or after one, or replace a
body, without re-lexing. `kiln_ast::decl_extents`; the field
`Ast::decl_extents`, read through `Ast::decl_extent(d)`. Parser helpers:
`kiln_parse::extents`.

### 19.1 Contract

- **Opt-in** (decided by measurement, §19.8: always on would cost a
  tenth of the AST's memory on `<bits/stdc++.h>` and a third on a TU of
  written declarations only, 48-51 bytes per row).
  `DeclExtents::is_on()` is false unless `ParseOptions::decl_extents`
  is set; the driver sets it for `-fkiln-decl-extents` and for
  `-fkiln-dump-decl-extents` (spec/driver.md).
  Off, every recording site costs one predictable branch and allocates
  nothing; the table is empty and `Ast::memory()` has no row for it.
  Producing the pending first token (§19.3) is a `Loc` move either way.
- **One row per declaration the parser parsed**, keyed by its `DeclId`,
  written where the parser consumed its tokens (never from a sema action,
  which also runs for implicit declarations and instantiations). A
  redeclaration is its own declaration and has its own row; so does each
  reopening of a namespace. The rows of a template and of its pattern
  (`FunctionTemplate`/`Function`, `ClassTemplate`/`CXXRecord`,
  `VarTemplate`/`Var`, `TypeAliasTemplate`/`TypeAlias`) are one shared
  row.
- **Keys** (WP-2470). Most redeclarations have a `DeclId` of their own.
  Some written declarations have none: the AST keeps them under the
  `DeclId` of the entity they redeclare or name, namely a repeated
  explicit specialization declaration of a class template
  (`template <> struct Q<int>;` twice, then its definition), a repeated
  partial specialization declaration, an opaque enumeration declaration
  repeated (`enum class E : int;` twice), an explicit instantiation of a
  class, function or variable specialization or of a member
  (`template struct S<int>;`, `template void t<int>(int);`,
  `template void t(short);`, `template int v<int>;`,
  `template void S<long>::m();`, `template int S<long>::sm;`, an
  explicit instantiation of a constructor) and an `extern template`
  declaration of any of these, and a label's definition (`l:`) after a
  GNU local label declaration of it (`__label__ l;`, WP-2630). A row is keyed by (`DeclId`, ordinal):
  the *n*th declaration of the entity written under that `DeclId` is its
  *n*th row, in source order, and no later declaration overwrites an
  earlier one's row. `get(d)` (and `Ast::decl_extent(d)`) is `d`'s last
  row, `rows_of(d)` every row of `d` in source order; a template sharing
  its pattern's row shares all of them. An explicit instantiation's row
  is keyed by the first declaration of the specialization it names (a
  function template specialization's, a static data member's of the
  class template specialization): the explicitly instantiated
  specialization when nothing declared it before, else the explicit
  specialization it names (which then has a row before it).
- **No row**: implicit declarations (special members and their
  parameters, implicit deduction guides, builtins, injected class names,
  a lambda's closure type and its members, a requires-expression's body,
  the scope of its parameters), instantiations and their members (a
  client maps a specialization to its pattern with `template_info` and
  reads the pattern's row) and every other copy a substitution makes of
  a written declaration (an instantiated function's parameters and
  locals, an expansion statement's copies of its body per element,
  [stmt.expand], a satisfaction check's requires-expression parameters),
  the parameter a `(void)` clause makes and drops, and the declarations
  an error recovery skipped. An elaborated-type-specifier that names a
  class or enumeration declared elsewhere declares nothing and adds no
  row ([dcl.type.elab]), qualified or not, and also when its qualifier
  names a class template specialization whose member lookup instantiates
  (`struct O<int>::S *p;`: the member is an instantiation, WP-2629); one
  that declares its class (`struct S *p;` with `S` undeclared,
  `friend class F;` with `F` undeclared) has the class's row.
  An explicit specialization and an explicit instantiation are written
  declarations: the row is the specialization's (or the member's, for
  `template void S<long>::m();`), its range that of the declaration that
  names it, with its own range, name and qualifier those of its
  declarator, as for any declarator (a class template's, in the
  class-specifier's place, those of its elaborated-type-specifier). An
  explicit instantiation of a function template without template
  arguments (`template void t(short);`, the specialization deduced,
  [temp.explicit]/8) has the name `t`. A friend declaration naming a
  specialization (`friend struct Q<int>;`) declares nothing new and
  adds none, whether the specialization has a row or is implicit
  (§19.5).
- **Read access** (all on `DeclExtents`): `is_on()`, `get(d)` (the last
  row), `rows_of(d)` (every row, in source order), `iter()` (every row,
  by declaration in `DeclId` order and a declaration's rows in source
  order, a shared row once per declaration), `len()` (rows, a shared
  row once),
  `module_decls()` (the module and import declarations' rows, §19.9),
  `dump(ast, sm, main)` (§19.7). Helpers on the row: `semi()` (the `;`
  as a span), `qualified_name()` (`[qual, name.hi)`), `prefix()` (the
  decl-specifier part shared by a declarator group).
- **Writes** go through `set(d, e)`, `entry(d)`, `share(alias, d)`,
  `push_module_decl(m)` and `get_mut(d)` (no-ops when off; each writes
  `d`'s last row), `redeclare(d)` (a later declaration of `d` under the
  same `DeclId` begins: a new, empty last row; the parser's
  `ext_redecl`), `forget(d)`
  (a declaration the parse made and dropped), `mark()`/`truncate(mark)`
  (§19.6) and `shrink_to_fit()`, which the parser calls once the
  translation unit is parsed (after the end-of-unit instantiations,
  which add no rows), so the table holds no spare capacity (WP-2092,
  §19.8).

### 19.2 The row (44 bytes, a `size_of` unit test)

Every `Span` is `[start of its first token, end of its last token)` in
kiln token locations (§19.6). An absent part is `Span::INVALID` /
`Loc::INVALID`.

| field | type | meaning |
|---|---|---|
| `full` | `Span` (8) | the whole declaration, from its first token (§19.3) to its last; shared by every entity one declaration declares |
| `own` | `Span` (8) | the part that is this entity's alone: a declarator's init-declarator (declarator, initializer, bit-field width, pure-specifier, function body, `= default`/`= delete`); a tag's class-specifier, enum-specifier or elaborated-type-specifier when a decl-specifier-seq declares it; a using-declarator; a friend-type-specifier of a list; a parameter's declarator (empty, at its end, when abstract); `full` for everything else |
| `name` | `Span` (8) | the unqualified-id of the declarator-id or the declared name: `f`, `operator+`, `operator()`, `operator new[]`, `~T`, `operator int*` (a conversion-function-id with its type), `operator""_x`, `S<int>` (a specialization's template-id with its arguments), a namespace's own identifier; a using-directive's nominated namespace and a using-declaration's member name; invalid for an unnamed declaration |
| `qual` | `Loc` (4) | the start of the nested-name-specifier before `name` (a leading `::` included; for a nested namespace definition, §19.4); invalid when unqualified |
| `body` | `Span` (8) | a function body with its ctor-initializer and a function-try-block's handlers; the braces of a class, enumeration, namespace, linkage specification, `export` block or consteval block; `= default`, `= delete` (with its reason) or `= 0` |
| `specs_end` | `Loc` (4) | the end of the decl-specifier-seq (§19.5); invalid when there is none (a constructor, a namespace) |
| `end` | `DeclEnd` (1) | the last token: `Semi` (`;`, `semi()` gives its span), `Brace` (the `}` that ends the body, `full.hi == body.hi`) or `Other` (§19.3) |

Three padding bytes remain. 44 bytes for the row plus 4 bytes of index
per declaration up to the last one with a row, and 8 more per later row
of a declaration written more than once under one `DeclId` (§19.1).

**Written attributes** (WP-2541). An explicit instantiation of a
function, variable or class declares nothing (the specialization's
`SpecKind` stands for it), and sema merges its decl-specifier
attributes onto the specialization, whose attribute list then holds
them by the location of their names only (spec/sema.md "Explicit
instantiation attributes"). So the row of an explicit instantiation
also records every attribute written on it, merged or dropped (one
after the declarator, one an error drops), in source order, as a
`WrittenAttr`: its `AttrKind`, its `AttrSyntax`, and its `span` from its
name (a scoped one's scope) to the end of its last token (the `)` of
its arguments). `DeclExtents::row_attrs(d, n)` gives those of `d`'s
`n`th row (`rows_of` order); other rows have none. 16 bytes each (with
the row index); the dump appends ` attr=<Kind>@<range>` (clang's
attribute class name without `Attr`) to the row.

### 19.3 First and last tokens

The first token is that of the outermost construct the declaration
begins with: an attribute-specifier-seq (including a block declaration's
statement attributes), the `template` of the first template head
(explicit specialization heads `template<>` and the requires-clause after
a head included), `extern "C"` of a braceless linkage specification,
`export` of a braceless export declaration, `__extension__`, or else the
first decl-specifier or the declarator. The parser notes the pending
first token where a declaration begins (`ext_lead`); a prefix construct
keeps it; the construct that makes the declaration takes it
(`ext_begin`); a class body, a compound statement and a template
parameter list's nested declarations set it aside and restore it, so a
lambda or a class in a template head's default argument never takes the
outer one. A braceless `extern "C"` or `export` has its own row too, of
the same `full` as the declaration it contains.

The last token is the `;` of a simple declaration, member declaration,
alias, using-, static_assert, concept, namespace alias or
explicit-instantiation declaration, or the `}` of a function body (the
last handler's, for a function-try-block), a namespace, a braced linkage
specification, an `export` block or a consteval block. A class or
enumeration definition ends at the `;` of its declaration; its `own`
ends at its `}`. `Other`: parameters and template parameters (before the
`,`, `)` or `>`), enumerators (before the `,` or `}`), condition
declarations, exception declarations, for-range-declarations (the loop
variable of a range-based for or expansion statement ([stmt.ranged],
[stmt.expand]), up to its declarator: the range after `:` is not
part of it), init-captures (from their first token, `&`, `...` or the
name, to the initializer's last; the variable [expr.prim.lambda.capture]/6
declares), a requires-expression's parameters, a label (from its
attribute-specifier-seq, or else its identifier, to the `:`, or to the
last GNU attribute after it: `l: __attribute__((unused));`), a
postcondition's result name (its result-name-introducer, from the
identifier to the `:`, [dcl.contract.res]), access specifiers
(`public:`), and a declaration an error cut short. A GNU local label
declaration (`__label__ a, b;`) ends at its `;`; each label's `own` and
`name` are its identifier (WP-2630). The optional `;` after an in-class member function definition
is not part of it.

### 19.4 Namespaces

Each `namespace N { ... }` is one row of the namespace declaration it
opens (a reopening is a new declaration): `full` from `namespace` (or
`inline`, or an attribute) to the `}`, `body` the braces, `name` the
identifier. An anonymous namespace has no `name`. `namespace a::b::c {`
opens three namespaces with one brace pair: each gets a row with the same
`full` and `body`; its `name` is its own identifier, and `qual` of `b`
and `c` is the start of `a` (so `qualified_name()` of `c` is `a::b::c`).
An inline component (`a::inline b`) is the same. A namespace alias is
one row from `namespace` to `;` with `name` the alias.

### 19.5 Declarator groups

`int a, *b = 0;` declares two variables that share `full` (`int a, *b =
0;`) and `specs_end` (after `int`; `prefix()` is `int`). Each `own` is its
init-declarator (`a`, `*b = 0`) and each `name` its identifier. A
class or enum defined in the group's decl-specifier-seq (`struct S {}
s, *p;`) has its own row first: `full` the declaration, `own` the
class-specifier. A structured binding declaration has the group row and
each binding a row whose `own` and `name` are its identifier (in a
condition or a for-range-declaration too). Fields,
bit-fields and member declarator lists are the same; a friend function
declaration gives the same row to the `Friend` and to the function it
declares. A friend declaration that names a class declared before it
(`friend struct P;`) or a member of another class (`friend void
H::h(G&);`, `friend B::~B();`) declares nothing new: the earlier
declaration keeps its row (WP-1932; the identifier-accounting test of
§18.8 found the overwrite); so does one naming a class template
specialization with a row (`friend struct Q<int>;` after `template <>
struct Q<int> {};`, WP-2470), and one naming an implicit specialization
or a member of one (`friend struct V<int>;`, `friend struct
O<char>::S;`) adds none (WP-2629). The `Friend` has its own row either
way.

### 19.6 Locations, speculation, PCH, modules

- **Raw `Loc`s.** Every bound is the location the lexer gave the token,
  so either end may lie in a macro expansion (`DECL(x)` gives a `full`
  starting in the macro body) and the two ends may be in different
  expansions; nothing is mapped to file ranges. A client maps them with
  the source manager (`spelling_loc`, `expansion_loc`) as diagnostics
  do. A `hi` is `lo + length` of the last token in its own location
  space.
- **Speculation.** The parser never re-parses consumed tokens (§18.5):
  a tentative scan records nothing, and late-parsed member bodies,
  default arguments and member initializers are captured tokens whose
  span is recorded when they are captured. `mark()`/`truncate(mark)`
  (dropping the later rows and their aliases) are there for any rollback
  and unit-tested.
- **PCH.** The table is written into snapshots (§17; FORMAT_VERSION
  129; 146 since WP-2091, which adds the module and import declarations'
  rows; 152 since WP-2470, which adds the later rows of §19.1; 157 since
  WP-2541, which adds the written attributes of §19.2), with its
  on bit; the consumer then sets that bit from its own
  options. `-fkiln-decl-extents` is part of the PCH identity (a producer
  and a consumer must agree); `-fkiln-dump-decl-extents` is not, and
  makes the producer take two passes.
- **Module images** (spec/modules.md §11.11; IMAGE_VERSION 62, 79
  since WP-2091, 81 since WP-2470, which carries every row of a
  declaration written more than once, 84 since WP-2541, which carries
  the rows' written attributes of §19.2). A module unit parsed with the
  table on carries a row
  per record that has one and per parameter of a function record,
  located in the image's location space; an importer whose table is on
  gives the declarations it builds the same rows (a template sharing its
  pattern's again). The declarations inside bodies are records once a
  carried body names them, so they have their rows too: the locals of an
  inline function, of an inline variable's initializer or of a
  template's pattern (instantiated in the importer or not), a
  declarator group's declarators, lambdas' parameters, init-captures
  and locals, block-scope classes and enumerations and their members,
  structured bindings, init-statements' and conditions' variables.
  A requires-expression's parameters, which are not records, are
  carried by their index among the image's requires-expression
  parameters (WP-2091). So a declaration reached through an imported
  definition has the row it has when the unit is parsed; only its
  `DeclId`, and so its place in the dump, differ. A body the image does
  not carry (a non-inline function's, which no importer reaches) has no
  rows in an importer built from the image, while replay parses it and
  records them. The module and import declarations (§19.9): an importer
  that replays the image's preamble (a partition, a unit with imports)
  preprocesses it and records their rows as replay does; an image
  without a preamble carries its module declaration's row, and the
  importer records it where it enters the unit. An extent location the
  image cannot carry fails the record (the image, for a
  requires-expression parameter or the module declaration) as any
  other location does. A unit replayed from source records its rows by
  parsing. The module cache key includes `decl-extents`.

### 19.7 The dump

`-fkiln-dump-decl-extents` prints, at the end of the TU, one line per
row (`iter()`): by declaration in `DeclId` order, a declaration written
more than once under one `DeclId` (§19.1) once per row in source order:

```
<Kind> '<name>'  full=<L:C>-<L:C> <;|}|-> [own=..] [name=<L:C>-<L:C> '<text>'] [qual=<L:C>] [body=..] [specs-end=<L:C>] [exp=<L:C>]
```

Positions are spelling locations, prefixed with the file's base name and
`:` outside the main file. `own=` is printed when it differs from
`full`; `<text>` is the name's source text; `exp=` the expansion point of
a `full` that starts in a macro. A lambda in a qualified name prints as
`(lambda at <file>:L:C)` without directories. A module or import
declaration's row (§19.9) prints before the row of the first
declaration made after it (at the end when none is):

```
<ModuleDecl|ImportDecl|HeaderImportDecl> '<name>'  full=<L:C>-<L:C> <;|-> [export=.. '<text>'] keyword=.. '<text>' [name=.. '<text>'] [partition=.. '<text>'] [attrs=.. '<text>']
```

`<name>` is the source text from the module-name to the end of the
partition (`M.N:P`, `:P`, `<h>`). The harness directive
`decl-extents: <golden>` (spec/harness.md) compares the dump with a
golden file; `decl-extents: <golden> unordered` compares the two as
sorted lines, for an importer built from a module image, whose
`DeclId`s follow the image's record order rather than the parse's.

### 19.8 Cost

Measured with a release build on aarch64, `-fsyntax-only -std=c++26`
(WP-2090, remeasured by WP-2092 once the parse trims the table;
`-fkiln-memory-report` for the table and AST totals, "live" the bytes in
use and "reserved" the bytes held, peak RSS from the OS):

- *`<bits/stdc++.h>`* (libstdc++ 14): 89,294 declarations with a row
  (the rest of the arena's are implicit or instantiated). The table
  holds 4,557,124 bytes live: 51 bytes per row (44 for the row, the
  index's 4 per arena declaration spread over the rows), 27 per arena
  declaration; it reserved 6.40 MB before the trim and reserves exactly
  what it uses after it. Accounted memory grows from 42.95 MB to
  47.50 MB live (+10.6%), and from 63.19 MB to 67.75 MB reserved (69.59
  MB before the trim); peak RSS from about 92.0 MB to 96.5 MB (+4.9%);
  compile time is within noise (1.3-2.5 s either way on a loaded host).
- *A synthetic TU of 320,000 written declarations* (20,000 namespaces,
  each with a class of three fields and a method declaration, a
  function declaration of two parameters, a group of two variables, a
  typedef and an enumeration of three enumerators, one line per
  namespace): 320,000 rows, 15,360,300 bytes live (48 bytes per row);
  the table reserved 25.6 MB before the trim (vector doubling) and
  reserves 15.36 MB after it. Accounted memory grows from 26.18 MB to
  41.54 MB live (+59%: every declaration is written, there are no
  bodies or instantiations to dilute it), and from 41.43 MB to 56.79 MB
  reserved (67.03 MB before the trim); peak RSS from 53.9 MB to 70.0 MB
  (+30%), the same before and after the trim, which comes after the
  peak; compile time is within noise (0.4-0.6 s).
- *Off* (the default): the memory report is byte-identical to the build
  before the table (and to the build before WP-2091), peak RSS and
  compile time are within noise, module images are byte-identical but
  for the version word and the file's trailing hash, and the bench/oven
  counters are identical.

So the table stays opt-in: a tenth of the AST on a header-heavy TU and
more than half on one made only of written declarations is too much to
charge every compilation.

### 19.9 Module and import declarations (WP-2091)

A module-declaration ([module.unit]/1) and a module-import-declaration
([module.import]/1) are preprocessing directives ([cpp.module],
[cpp.import]) that make no declaration node: sema acts on them
(spec/modules.md §4) and nothing has a `DeclId` to key a row by. The
row shape of §19.2 does not fit them either: it has no field for the
keyword, the partition or the attributes, and its `own`, `qual`,
`body` and `specs_end` mean nothing for them. So they are a kind of row
of their own, `ModuleDeclExtent` (64 bytes, a `size_of` unit test),
kept in source order in `module_decls()`:

| field | type | meaning |
|---|---|---|
| `full` | `Span` | from `export`, or else the keyword, to the `;` |
| `export` | `Span` | `export`; invalid when not exported |
| `keyword` | `Span` | `module` or `import` |
| `name` | `Span` | the module-name with every dotted component (`a.b`), or the header-name of a header unit (`<h>`, `"h"`); invalid for `import :P;` |
| `partition` | `Span` | the module-partition, from its `:` to its last component |
| `attrs` | `Span` | the attribute-specifier-seq before the `;` (the tokens the preprocessor skips to it) |
| `semi` | `Span` | the `;`; invalid when it is missing (an error; `full` then ends at the last part) |
| `before` | `u32` | the index of the first declaration made after it, the dump's position (§19.7) |
| `kind` | `ModuleDeclKind` | `Module`, `Import` (a named module or a partition) or `HeaderImport` |

The preprocessor notes the parts' ranges while it reads the directive
(`kiln_pp::DirectiveParts` on the `ModuleDirective` event) whether the
table is on or not; the parser makes the row when it hands the event to
sema, so an `export import` (`export` set), the module declarations and
imports of every unit it reads (the imported units' and header units'
too) and the imports of a non-module unit all have rows. The global
module fragment's `module;` and the private module fragment's
`module :private;` are not module-declarations and have none, nor does
a directive the preprocessor rejected. Rows of a unit built from a
module image: §19.6.

Tests: `crates/kiln_ast/src/decl_extents.rs` (row sizes, off records
nothing, set and share, `semi`/`prefix`, truncation, lambda names,
module rows only when on, the trimmed capacity),
`crates/kiln_parse/tests/decl_extents.rs` (the API on parsed source,
capacity equal to length after the parse; `every_written_declaration_has_a_row`:
every declaration of a source of every kind has a row but implicit
declarations and copies, and no instantiation has one),
`crates/kiln_driver/src/snap_tests.rs` (`snapshot_round_trip_decl_extents`)
and `tests/conformance/decl_extents/` (one per case of §19.2-§19.6,
macros, templates, a PCH and module-image round trips:
`module-image.cpp`/`module-replay.cpp` for an image without a preamble,
`module-bodies-image.cpp`/`module-bodies-replay.cpp` for the
declarations inside bodies, a partition, attributes, `export import`
and a header unit; `elaborated-refs.cpp`, `stmt-decls.cpp` and
`contract-result.cpp` for WP-2629/WP-2630).

## 20. Public tables for tooling clients (WP-2024)

Tooling clients that read the `Ast` directly (indexers, refactoring
tools, digesters) meet nodes whose words are untyped `u32`s. This
section lists the tables that say what each word holds, and the
promise that comes with them.

### 20.1 The operand-role table (`kiln_ast::roles`)

- `Role` names what a word holds: `Zero` (always 0), an id of an arena
  (`Expr`, `Stmt`, `Decl`, `Attr`, `TypeId`, `Profile`, `Nns`,
  `ArgList`, `TemplateName`, `Symbol`, `DeclName`, `StringLit`,
  `ConstValue`, `Reflection`, `Lambda`, `Annotation`), a `QualType`
  (`Type`), a raw `Loc`, a builtin id, `Opt(r)` (0 or an `r`),
  `List(r)` (a `ListId` of `r`s), a side-table index (`Side`) or record
  (`Record`), an enum value at most `max` (`Enum`), a `Bool`, a plain
  number (`Value`), `Bits(fields)` (named bit fields, each with its own
  role) and `Cond { when, then, otherwise }`, a role that depends on a
  flag bit, on `aux` (a bit or a value) or on the value of the node's
  `a` or `b` word (a marker such as an unchecked `visibility`'s
  `u32::MAX`). `Role::resolve(Words)` picks the branch for a node from
  its flags, `aux`, `a` and `b`; `Words::CHECKED_ATTR` is an
  attribute's final form, the one the module image carries.
- One row per kind: `TYPE_ROLES` (a `TypeNode`'s `aux` and `data`, and
  the words of its pair or triple payload, which
  `TypeContext::payload_words` returns), `EXPR_ROLES` (`a`, `b`, `aux`
  and the per-kind flag bits 4..8 of `ExprNode::raw_bits`),
  `STMT_ROLES` (`a`, `b`, `aux`), `ATTR_ROLES` (`a`, `b` and the
  per-kind flags, sorted by `AttrKind`; `ATTR_FLAGS` are the flags every
  attribute shares) and `COLD_MAPS` (every side map keyed by a node id:
  its owner, name, key and value). The lookups are `type_roles`,
  `expr_roles`, `stmt_roles` and `attr_roles`.
- The module-image writer and reader derive their per-attribute word
  checks from `ATTR_ROLES` (`bmi_image::attrs::op_of`), so the image and
  the table cannot drift; a golden copy of the earlier hand-written
  table pins the image format.
- Tests: `kiln_ast::roles` (a row for every kind, every cold map, enum
  maxima equal to the enums, non-overlapping fields),
  `kiln_sema::bmi_image` (the image accepts no word outside its role)
  and `crates/kiln_parse/tests/operand_roles.rs` (every word of a real
  AST fits its row; `KILN_ROLES_DIR` runs it over a directory).

### 20.2 Descriptors of the borrowed views

`TypeView`, `ArgView`, `ParamAbi`, `AddrSpace`, `ConstValue`, `Scalar`,
`ByteImage` and `DeclNameView` derive `kiln_snap::Describe`: a
descriptor text (variants with their discriminants, fields with their
types, as `Snap` writes it, spec/snap.md) that a client checks to know
it reads the same layout. The derive adds nothing to the snapshot
schema. `crates/kiln_ast/src/view_descriptors.txt` is the golden of the
eight descriptors (test `roles::tests::view_descriptors_are_pinned`).

### 20.3 Stable names

- `FuncInfo`'s layout constants (`VARIADIC`, `REF_SHIFT`,
  `QUALS_SHIFT`, `CC_SHIFT`, `NORETURN`, `TRAILING`, `NO_CSR`,
  `REGPARM_SHIFT`, `SME_SHIFT`), with `sme::FLAGS` and `sme::STATES`
  naming the SME bits.
- `fp_bit::ALL`: the floating-point option bits by name.
- `AddrSpace::NAMED`: the address-space words by name.
- `decl::atomic::ALL`: every atomic builtin id, sorted (ids 1..=38 are
  the operations).

### 20.4 Expression profiles

A `DependentDecltype`, a dependent array bound and a dependent
`noexcept` compare by the canonical profile of their expression
(spec/types.md §7.2). `Ast::expr_profile_words(e)` returns its word
sequence, the one `TypeContext::profile` interns. A typed walk of
those words (which of them are canonical type, nested-name-specifier,
argument-list, template-name and declaration ids) is not public yet.

### 20.5 Printed names: diagnostic, dump, key (WP-2477, WP-2545)

A client that keys anything on a printed type name uses the key printer,
`kiln_ast::dump::AstNames::for_keys(ast, sources).type_key(q)`: two
different types never print alike there. The diagnostic printer
(`DiagNames`, clang's text) and the dump printer (`AstNames`, the AST
dump's and debug information's text) follow clang, which prints, for
example, `S<(int *)nullptr>` and `S<(char *)nullptr>` both as
`S<nullptr>`, and a `Local` class of `f()` and one of `g()` both as
`Local`. spec/types.md §12.1 states which printer prints what.

### 20.6 Stability

Every value in these tables (a row's roles, an enum maximum, a field's
bits, a named constant, a descriptor) is stable within a snapshot
`FORMAT_VERSION`: a change to any of them is a format bump, and the
tests above fail on one that is not.

## 21. Written extents of expressions (WP-2327)

A tooling client that moves, wraps or replaces an expression as text
needs its first and last token exactly; a range one token short (clang's
`getEndLoc` falling back to the node's own location) breaks the edit.
This section is the promise kiln makes, and where it makes none.

### 21.1 The guarantee

- `kiln_ast::expr_extent::WrittenExtents::new(ast)` is the tree's view
  and its `get(e) -> Option<(Loc, Loc)>` the location of the first and
  of the last token of `e` as written, on the finished tree, or `None`.
  A `Some` is exact for every kind: there is no fallback to the node's
  own location. The two locations are token starts and may be macro
  locations. `new` makes one pass over the lambdas and one over the
  expressions (to find the nodes of 21.2 with nothing written: capture
  initializers, the names unwritten calls call by, synthesized array
  bounds), keeping them in a hash set: per node of the tree a one-word
  kind test at view time, and a set entry only for a node found; `get`
  reads only the tree and the set.
- `kiln_parse::ExprExtents::new(ast, sm, interner, lex_options)` is the
  client's view: its `written_extent(e)` is `WrittenExtents::get` when
  the pair is one file range by the rule of spec/lex.md "Token ranges"
  (and not a scratch buffer), else `None`, so a `Some` is always
  expressible; `written_range(e) -> Option<(FileId, u32, u32)>` is that
  range in bytes, end exclusive.
- Nothing is computed while parsing: the walk reads what the tree already
  holds (operands, `ExprArena::end_loc` and its cold side table of
  closing tokens, the qualified-name begins of `templates.expr_begin`).
  No node grew and no stored location was added; one per-kind flag bit
  (`ExprFlags::DESIGNATED` on `InitListExpr`, 21.2) is new, carried in
  the node's bits as every flag is, so the module image and the PCH
  snapshot keep their layout (no format bump). Sema records a few more
  closing tokens and qualifier begins in the tables it already has (the
  `>` of a member template-id and of a resolved function template-id,
  the `::` of a qualified builtin, the `)` of `__builtin_bit_cast` and
  `__builtin_choose_expr`), which clang's `getBeginLoc` / `getEndLoc`
  give too, and a member call whose object it converts
  keeps the `__extension__`, `__builtin_choose_expr` or `_Generic`
  around its callee, as clang converts the object in place (before,
  only parentheses were kept, and the call began at the member).
- WP-2350..WP-2353 record more in the same two cold maps (the
  `end_loc` side table and `templates.expr_begin`), only where the
  location is not the node's own: the last token of an id-expression or
  member designator that is not one token (an operator- or
  conversion-function-id, a dependent template-id's `>`, a
  pseudo-destructor's destroyed type, always), recorded by the parser
  after the name (`Sema::keep_name_extent`) and kept on the reference or
  member access the call resolves it to; the qualifier begin of a member
  access through an implicit `this` and of a qualified concept-id; the
  `.` or `[` a `DesignatedInitExpr` begins with; and the `(` or the cast
  a `CXXParenListInitExpr` begins with (`Sema::keep_paren_list_begin`).
  Sema's `init::Designator::Field` keeps its `.`, which changes the PCH
  snapshot schema (no node or image change).
- WP-2460..WP-2464 close the shapes WP-2350 left, in the same two maps
  (no node, image or snapshot change): a `CXXDependentScopeMemberExpr`
  with no object is an implicit `this->name` (a dependent
  conversion-function-id `operator T*()`, a member named by a dependent
  using-declarator), begun at its name or recorded qualifier and ended
  at its recorded last token; the overload set `&B::f` is turned into
  (`member_overload_as_lookup`) and the pointer to member it resolves to
  keep the qualifier's begin, as does a reference to a static member
  template's specialization through an implicit `this` (`s<0>`,
  `B::s<1>`); an array `new`'s parenthesized aggregate initialization
  keeps its `(`; and `&this->sf<int>`, resolved to the specialization,
  keeps the template-id's `>`.
- WP-2721..WP-2724, in the same maps and in one aux bit the node kind
  already has (no node or image change; the PCH snapshot schema names
  one more transient sema field, `direct_list_entity`, so
  `FORMAT_VERSION` is 184): `Ast::begin_loc`
  begins a qualified member access through an implicit `this`
  (`Q::f(2)` calling a non-static member of an overload set,
  `Q::t<int>(4)`) at the qualifier, as the client view already did; a
  constructor called for a braced list ends at its `}`; the copy a
  list's single element of the class makes ([dcl.init.list]/3.2) is
  that list's construction (`ExprArena::set_construct_list_braces`);
  and a member variable template's template arguments are recorded
  outside a template too, so `d.twice<7>` ends at its `>` from them.
- WP-2828 checked, and pins (`a_specifier_begins_the_name_it_qualifies`
  in `crates/kiln_parse/tests/expr_extents.rs`), that a name qualified
  by a `decltype(...)::` or splice specifier begins at the specifier's
  first token, as clang's `getBeginLoc` (19 and 22) begins
  `decltype(s)::m` at the `decltype`: `decltype(s)::N::k`,
  `&decltype(s)::f`, the dependent `decltype(t)::m`, `[:^^S:]::m` (the
  `[:`) and `template [:^^X:]<int>::v` (the `template`). The begin is
  the qualifier begin sema already records (`templates.expr_begin`, from
  the parser's `nns_start`); nothing changed. The written-reference
  entry of such a name now reaches the same token (§18.11.2).
- WP-3691 sets one per-kind flag bit (`ExprFlags::MEMBER_SYNTAX`, bit 4
  on `CallExpr`, clang's `CallExpr::usesMemberSyntax()`; carried in the
  node's existing flag byte, no PCH or image layout change) when sema
  builds a `CallExpr` for a C++23 explicit-object member function written
  with member syntax (`s.f(2)`, `(*p).f(2)`, `p->f(2)`, `this->f(2)`,
  implicit-object `f(2)` inside a member, and template instantiations of
  `t.f(2)`). `WrittenExtents::written_first` and `Ast::begin_loc` begin
  such calls at argument 0 (the object), while non-member calls such as
  `(&S::f)(s, 2)` keep their callee begin and explicit-object `operator()`
  (`CXXOperatorCallExpr`) already begins at its object.

### 21.2 Decisions

- **Implicit wrappers** (`ImplicitCastExpr`, `MaterializeTemporaryExpr`,
  `CXXBindTemporaryExpr`, `ExprWithCleanups`, `ConstantExpr`,
  `CXXStdInitializerListExpr`) have the extent of the written node
  inside them.
- **Nothing written**: `CXXDefaultArgExpr`, `CXXDefaultInitExpr`,
  `ImplicitValueInitExpr`, `NoInitExpr`, `ArrayInitLoopExpr`,
  `ArrayInitIndexExpr`, `OpaqueValueExpr`, an implicit `CoawaitExpr`
  (an initial or final suspend point), an implicit `CXXThisExpr`
  (also the object of `X::~X()` in a member function), the initializer
  of an implicit lambda capture and of a variable length array bound's
  capture, a reference to an implicitly declared variable (a coroutine's
  promise), a member access whose object the compiler made (an awaiter)
  a call with no written parentheses that is not a user-defined
  literal, the name such a call calls by (a coroutine's
  `from_address`), and an array bound `new` takes from an array typedef
  or completes from the initializer (`new int[]{1, 2}`) are `None`.
- **Calls without parentheses**: a user-defined literal (`12_k`,
  `"ab" "cd"_sv`) is its literal, through the last piece of a
  concatenation; an implicit conversion function call (`int i = s;`)
  is its object. `Ast::begin_loc` and `Ast::end_loc` agree (WP-2543,
  clang `UserDefinedLiteral::getBeginLoc`/`getEndLoc`, the same in
  clang 19 and 22): the call (`Ast::is_udl_call`) begins at the literal
  token's start, not at its callee, so an expression it begins
  (`12_k + 1`) begins there too; it ends at the literal, a concatenated
  string at its last piece. The callee (`DeclRefExpr` of
  `operator""_k`) keeps its own location, the ud-suffix, as clang's.
- **Split `>`** (WP-2542): a template-id closed by the first `>` of a
  `>>`, `>=` or `>>=` ([temp.names]/4; `>>>` lexes as `>>` then `>`)
  ends at that one character. The parser records each such `>` (the
  first character, and the second of a `>>` whose rest is a `>`) in
  `Ast::split_greaters`, the set sema's `token_end` reads; the client
  extents measure their last token with `kiln_lex::token_range_split`
  over it (spec/lex.md "End of a token"). The set is part of the AST's
  PCH snapshot and of a module image (spec/modules.md §11.11, `splits`
  section), so a unit built from either measures the same ends.
- **Brace elision**: a list whose braces were elided begins at its first
  written initializer and ends at its last; one a designator opens
  (`.a.y = 3`'s list of `a`) begins inside the designator, at a part
  the tree does not keep, and is `None` (`ExprFlags::DESIGNATED`).
- **Designated clauses** (a `DesignatedInitExpr`, kept in a dependent
  list): from the first designator's `.` or `[` (a GNU `name:`'s name)
  to the initializer's last token.
- **Parenthesized aggregate initialization** (`CXXParenListInitExpr`):
  from its `(` (`S s(1, 2)`, `S(1, 2)`, `new S(1, 2)`, `m(1, 2)` in a
  mem-initializer) or, made by a cast, from the cast's first token
  (`static_cast<S>(1)`, `(S)1`), as clang's `getBeginLoc`, to its stored
  end (the `)`; a C-style cast's operand); an array `new`'s
  (`new int[2](1, 2)`, `new int[](1, 2, 3)`, `new R[2][2]({R(4)})`) from
  its `(`. One whose begin was not recorded is `None`.
- **List construction** (WP-2723, clang's `CXXConstructExpr ... list`):
  a constructor called for a braced list (`N m{1}`, `h({1})`, the copy
  `{x}` of `g({x})` for a parameter of `x`'s class or a base of it)
  begins at a direct-list-initialized variable's name or, for a
  copy-list-initialization and an argument, at the `{`, and ends at
  the `}`; in a `T{x}` temporary the copy begins at `T`, as clang's
  `CXXTemporaryObjectExpr`. A `T{...}` keeps its own end. Not yet
  clang's: a reference bound directly to a list's single element
  (`f({x})` for `const O&`) and a prvalue element of the class
  (`g({O{x}})`), where clang keeps a transparent `InitListExpr` around
  the element and kiln has the element only, so the argument is `x`
  (resp. `O{x}`), not `{x}`.
- **Substituted template arguments**: a `SubstNonTypeTemplateParmExpr`
  (the value of a non-type template argument in an instantiated
  member's initializer) is the one token of the parameter's name it
  replaces.
- **Rewritten operators** (`a != b` as `!(a == b)`, a comparison
  through `<=>`): the `CXXRewrittenBinaryOperator` has the written
  extent, from its first written operand to its last; the nodes built
  inside it are ordinary nodes, whose implicit parts are `None`.
- **Implicit-this member access** (`x` in a member function) begins at
  the member's name; with a qualifier (`B::x`, `T::x` naming a member of
  a dependent base) at the qualifier, whose begin the parser records; a
  qualified access or `DeclRefExpr` whose begin was not recorded is
  `None`. A dependent member access kiln builds with no object at all is
  such an access too: a dependent conversion-function-id (`operator T*()`,
  ending at the conversion-type-id's last token) and a member named by a
  dependent using-declarator (`using T::f;` then `f()`: the
  declarator's qualifier is not written at the use, so it begins at the
  name, as clang's `UnresolvedMemberExpr`).
- **Node kinds may differ, extents may not.** Kiln's tree need not have
  clang's node kinds, only its extents: `&this->sf<int>` (a static
  member function template's specialization) is a resolved `MemberExpr`
  in kiln also in a template, where clang keeps an
  `UnresolvedMemberExpr`; both begin at `this` and end at the `>`
  (`tests/extents/kinds.txt` maps the kinds).
- First token: the first operand for binary, conditional, subscript,
  call, member-call, postfix and overloaded non-prefix operators, and
  the pattern of a pack expansion or a pack index; the
  node's own location for the kinds that begin with a keyword or a
  punctuator (casts, `sizeof`, `new`, `delete`, `throw`, lambdas, braced
  lists, parentheses, prefix operators); a name's qualifier begin when
  it has one.
- Last token: the stored closing token (`)`, `]`, `}`, `>` of a named
  cast is followed by its `)`) when the kind keeps one; the last operand
  for binary, conditional, C-style cast, prefix, `delete`, `co_await`,
  `co_yield` and `sizeof`/`alignof` of an expression (`sizeof(e)`'s
  parentheses are a `ParenExpr`); the list of a compound literal; the
  node itself for one-token literals, `this`, postfix operators, the
  `...` of a pack expansion and an ext-vector accessor; the last
  piece of a concatenated string literal; a plain identifier name; the
  stored last token of a name that is not one token (an operator,
  conversion or destructor name, template arguments) and of a
  pseudo-destructor's destroyed type (`p->~T`, `x.~decltype(x)`). Such
  a name with no stored end (a variable template specialization always
  has template arguments): `None`.

### 21.3 Kinds that may be `None`

The kinds for which stage 1 of `cargo xtask extents` (spec/harness.md
"Expression extents") finds a `None`, each with the reason; the
per-unit `None` counts are gated by `tests/extents/baseline.txt`, so
none of them grows unnoticed.

Two reasons recur, named here once:

- **(M) a part of a macro body**: the node's tokens are spelled in a
  `#define`; the rule of spec/lex.md "Token ranges" maps a whole
  expansion or one argument use, never a part of a body (nor a pasted
  or stringized token, spelled in a scratch buffer).
- **(W) a wrapper of a `None`**: an implicit wrapper (21.2) whose written
  node has none.

The counts are stage 1's at WP-2350 (`None` / all nodes of the kind;
`CXXDependentScopeMemberExpr` at WP-2460);
the baseline, not this table, is the gate.

| Kind | `None` | Reasons |
|---|---|---|
| `ArrayInitIndexExpr`, `ArrayInitLoopExpr` | 15/15 each | nothing written: an array member's copy in an implicit or defaulted copy |
| `ArraySubscriptExpr` | 138/1522 | M; the element accesses of a defaulted comparison |
| `BinaryOperator` | 11038/27448 | M (most: test macros over comparisons); a range-`for`'s `__begin != __end`; a defaulted comparison's operations |
| `CStyleCastExpr` | 623/2112 | M |
| `CXXBindTemporaryExpr`, `MaterializeTemporaryExpr`, `ExprWithCleanups`, `ConstantExpr`, `ImplicitCastExpr` | 2294/2930, 2430/4618, 1434/3265, 9/1180, 8891/56549 | W; M |
| `CXXBoolLiteralExpr`, `CXXNullPtrLiteralExpr`, `GNUNullExpr`, `FloatingLiteral`, `IntegerLiteral`, `StringLiteral` | 417/1912, 1/561, 1/54, 152/1029, 9938/39112, 344/2919 | M |
| `CXXConstructExpr` | 3862/6314 | M; a default construction (`S s;`, an element or member with no initializer): nothing written |
| `CXXDefaultArgExpr`, `CXXDefaultInitExpr` | 352/352, 522/522 | nothing written: the parameter's or member's default |
| `CXXDependentScopeMemberExpr` | 38/531 | M |
| `CXXFoldExpr` | 5/151 | the fold of a constrained pack (`C... Ts`, 21.2): no parentheses written |
| `CXXFunctionalCastExpr`, `CXXUnresolvedConstructExpr`, `CXXStaticCastExpr`, `CXXNoexceptExpr`, `CXXThrowExpr`, `LambdaExpr`, `OffsetOfExpr`, `TypeTraitExpr`, `UnaryExprOrTypeTraitExpr`, `CompoundAssignOperator` | 2492/5382, 94/496, 105/624, 29/171, 16/229, 45/970, 12/85, 68/1760, 141/1855, 256/1150 | M (`CXXStaticCastExpr`: one in an implicit member's body) |
| `CXXMemberCallExpr`, `CallExpr`, `MemberExpr`, `DeclRefExpr` | 2710/4809, 1472/12766, 4351/12292, 6272/45728 | M; the calls and names a coroutine, a range-`for`, a structured binding or a defaulted function makes (promise, awaiter, `begin`/`end`, `get`); the `operator()` a lambda call names; a conversion function's implicit call |
| `CXXOperatorCallExpr` | 73/1197 | M; a range-`for`'s `++__begin`/`*__begin` and a defaulted comparison's operations on class iterators and members |
| `CXXStdInitializerListExpr` | 2/71 | W (a list sema builds) |
| `CXXThisExpr` | 2517/2921 | an implicit `this`; M |
| `CoawaitExpr` | 48/58 | an implicit `co_await` (an initial or final suspend point) |
| `ConceptSpecializationExpr` | 125/761 | a type-constraint's immediately-declared constraint (`C<X> T` is `C<T, X>`): not written as an expression; M |
| `ConditionalOperator` | 239/1025 | M; a defaulted comparison's operations |
| `DependentScopeDeclRefExpr`, `UnresolvedLookupExpr` | 34/304, 249/651 | M; an `UnresolvedLookupExpr` of a dependent operator (`m[is...]`, an operator lookup at instantiation): no name written |
| `DesignatedInitUpdateExpr` | 19/19 | the update of an earlier initializer by a later designator (`{.a = x, .a.b = 1}`): not mapped |
| `ImplicitValueInitExpr`, `NoInitExpr`, `OpaqueValueExpr` | 861/861, 26/26, 117/117 | nothing written |
| `InitListExpr` | 398/4357 | a list sema builds for an element or member with no initializer (at the enclosing `}`); a list a designator opens; M |
| `ParenExpr` | 9412/11054 | M (the parentheses of a macro body, `#define F(x) (x)`) |
| `PseudoObjectExpr` | 5/5 | the operations a defaulted comparison makes |
| `UnaryOperator` | 1887/7590 | M; a range-`for`'s `++__begin` and `*__begin` |

### 21.4 The dump (`-fkiln-dump-expr-extents`)

With `-fsyntax-only`, one line per expression node of every written body
and initializer (function bodies; variable, field, enumerator, parameter
and template-parameter initializers; constraint and `noexcept`
expressions; template patterns, not instantiations; no implicit
members), in the order of the AST dump walk (`kiln_ast::dump::
written_exprs`): `<kind> TAB <file> TAB <begin> TAB <end>`, byte offsets
of `ExprExtents::written_range`, `end` exclusive. A `None` has `-` for
`end`, and for `begin` the offset of the node's own location when that
is file text (else `-`, with `-` for the file). A `Some` that were no
file range would print `<kind> TAB ! TAB unmapped TAB -`; the harness
fails on one.

### 21.5 Tests

`crates/kiln_parse/tests/expr_extents.rs` (every kind of 21.2,
`explicit_object_member_calls_begin_at_the_object`, and the
macro cases: an argument, a whole invocation, a part of a body, a
pasted token) and `cargo xtask extents` (stage 1 in `ci`); the xtask
test `the_none_table_is_the_baselines_kinds` holds the table of 21.3 to
exactly the kinds of the baseline.

## 22. Builtin-macro literals (WP-2821)

A tooling client that compares or rewrites literals must not treat the
value of `__LINE__` or `__FILE__` as one written in the source: moving
the file or inserting a line above changes it. The builtin-macro literal
table says which builtin macro a literal came from
(`kiln_ast::builtin_literals`).

### 22.1 What is recorded

`BuiltinMacro` names the builtins whose result is one literal token
whose value is a property of the place, order, time or target of the
expansion ([cpp.predefined], clang's extensions): `__LINE__`,
`__FILE__`, `__FILE_NAME__`, `__BASE_FILE__`, `__COUNTER__`,
`__INCLUDE_LEVEL__`, `__DATE__`, `__TIME__`, `__TIMESTAMP__` and
`__FLT_EVAL_METHOD__`. The feature queries (`__has_include`,
`__has_builtin`, `__has_cpp_attribute`, `__is_target_*`,
`__building_module`, ...) are not: their value is a function of their
written operand and the configuration, and they live almost only in
`#if` (the preprocessing record keeps their values, spec/pp.md
"Preprocessing record"). `_Pragma` makes no literal.

A literal is marked when one of its tokens is

- a recorded builtin's result: written directly, reached through a user
  macro (`#define L __LINE__`), or substituted for a macro parameter
  after pre-expansion (`F(__LINE__)`, at any depth);
- made by `##` ([cpp.concat]) from operands one of which is marked
  (`CAT(1, __LINE__)` through an expanding indirection gives `125`), or
  by `#` ([cpp.stringize]) from an operand that holds a marked token
  (`STR(__LINE__)` through an indirection gives `"24"`): the token takes
  the first marked operand's builtin.

A string literal of several pieces ([lex.string.concat]) is marked when
any piece is, with the first marked piece's builtin. `STR2(__LINE__)`
without the indirection stringizes the name, `"__LINE__"`, and is not
marked; nor is a pasted `CAT(4, 2)`.

### 22.2 How it is kept

Off by default. With `PpOptions::builtin_literals` (the driver's
`-fkiln-builtin-literals`, `FrontEndOptions::builtin_literals`) the
preprocessor keeps a map from the macro location of each token it made
for a recorded builtin, a paste or a stringizing of a marked operand
(each such token has an expansion entry of its own) to the builtin;
`Preprocessor::builtin_literal_of(loc)` follows a token's
immediate-spelling chain through argument relocations to it. The map is
keyed by those locations, not by the scratch spelling, which is shared
between equal texts. The parser asks it for the tokens of each integer,
floating, character and string literal (and so of a user-defined
literal's) and enters the literal in `Ast::builtin_literals`, keyed by
the literal's own location (its first token), which an instantiated copy
keeps. Off, nothing is allocated and each literal costs one branch.

### 22.3 Reading it

`Ast::literal_builtin(e)` gives the builtin of an `IntegerLiteral`,
`FloatingLiteral`, `CharacterLiteral`, `StringLiteral` or
`UserDefinedLiteral`, `None` for any other expression, a written
literal, and every literal of a unit compiled without the option.
`BuiltinMacro::name` spells the macro; `BuiltinLiterals::iter` lists the
table.

### 22.4 Snapshots, modules

The table is the unit's own: a snapshot (PCH) writes neither the AST's
table nor the preprocessor's map (both are restored empty, the
consumer's options deciding whether its own literals are recorded), and
a module file carries none, so a literal from a precompiled header or an
imported module's image is not marked. `-fkiln-builtin-literals` is
outside the PCH identity and off in a module built in process.

### 22.5 The dump

`-fdump-ast` appends ` from <name>` to a marked literal's line
(`IntegerLiteral 'int' 17 from __LINE__`); without the table nothing
changes. Tests: `crates/kiln_parse/tests/builtin_literals.rs` (every
builtin directly; `__LINE__`, `__FILE__` and `__COUNTER__` through a user
macro and as arguments; pasted into an integer, a suffixed and a
floating literal; stringized; a middle and a last string piece; written
literals; off by default), and
`tests/conformance/cpp.predefined/builtin-literal-provenance.cpp`.

## 23. A friend's written form (WP-3164)

A `Friend` declaration's target (`FriendTarget`) is what it befriends.
In a class template a friend named through a qualifier or a template-id
is resolved, and the target alone loses what was written: the qualifier
`Bq<sizeof(sizeof(f(T())))>::` of `friend void
Bq<sizeof(sizeof(f(T())))>::g();` holds an instantiation-dependent
expression whose validity is decided per specialization
([temp.res.general]/6), though its value (and so the target) is the
same as `Bq<8>::`'s. A tooling client comparing two versions of a
template must see the difference. `kiln_ast::friend_form` keeps a
friend's written form as one row per friend that has written parts.

### 23.1 When a row exists

`DeclArena::friend_forms` holds the rows in declaration order (the
order Sema declares the friends, patterns and instantiations alike);
`ColdDeclData::friend_form` maps a `Friend` to its row. A friend has a
row, in any class (templated or not), when

- its declarator-id is qualified (`friend void N::f();`, `friend int
  B<T>::m(T);`, `template <class U> friend void A<U>::f();`);
- its declarator-id is a template-id (`friend void h<int>();`, `friend
  bool operator==<>(const X&, const X&);`, [temp.friend]/1.3);
- it names a constructor, destructor or conversion function (`friend
  Z<T>::Z();`);
- it is a qualified class friend with a template-head of its own
  (`template <class U> friend class N::X;`, `template <class U> friend
  class A<U>::In;`).

`friend void f(int);`, `template <class U> friend void f(U);` and
`friend class C;` have none: the target declaration (or type) is their
whole written form. A qualified class friend without a head of its own
has none either: its target type keeps the elaborated type with the
qualifier as written (§23.2.1).

A friend an instantiation declares in a class that is not itself a
pattern has a row only while it is still matched per specialization
(`DEPENDENT`, below): a fully instantiated friend's target is
`FriendTarget::Decl` of the exact entity (`h<int>` for `friend void
h<>(T);` in `S<int>`). Such a row is a copy of the pattern's with the
substituted qualifier.

### 23.2 The row

`FriendForm` (40 bytes):

| field | type | meaning |
|---|---|---|
| `kind` | `FriendFormKind` (1) | `Function` (a qualified function or function template), `Specialization` ([temp.friend]/1.3), `Constructor`, `Destructor`, `Conversion`, `Class` |
| `flags` | `FriendFormFlags` (1) | `GLOBAL` (the qualifier begins with `::`), `TEMPLATE_KW` (`Bq<8>::template t<1>`), `EVERY_SPEC` (a [temp.friend]/5 friend of every specialization: its own head deduces the qualifier's arguments), `DEPENDENT` (matched per specialization of the class, §23.3), `UNSUPPORTED` (accepted with `-Wunsupported-friend` and ignored) |
| `name` | `DeclName` | the befriended name: as written (`g`, `h`), or the special member's own name when it is known |
| `name_loc` | `Loc` | where the name is written |
| `qualifier` | `Option<NnsId>` | the nested-name-specifier as written: its `Nns::Type` components keep their sugar, so `Bq<sizeof(sizeof(f(T())))>::` holds the expression and `Bq<8>::` does not |
| `qualifier_loc` | `Loc` | where the qualifier begins |
| `args` | `Option<ArgListId>` | the template-id's arguments as written (instantiation-dependent expressions included); `Some(EMPTY)` for `f<>`, `None` without a template-id |
| `entity` | `Option<DeclId>` | §23.3 |
| `found` | `ListId` of `DeclId` | the function templates lookup of a specialization's name found (first declarations); empty otherwise |
| `heads` | `ListId` of `TemplateParamListId` | the friend's own template-heads, outermost first; each head's parameters are declarations and its requires-clause is `TemplateParamList.requires` |
| `requires` | `Option<ExprId>` | the trailing requires-clause |

Ids, not copies: `heads` and `requires` are the lists and expressions a
target declaration holds, when there is one.

#### 23.2.1 Class friends

A class friend's written form is its type: `FriendTarget::Type` keeps
the elaborated type with its keyword and the qualifier as written
(`friend struct ::G;` is `struct ::G`, `friend struct
Bq<sizeof(sizeof(f(T())))>::Y;` is `struct Bq<sizeof (sizeof
(f(T())))>::Y` over `Bq<8>::Y`), as clang's dump shows it. A row adds
only an own head (`Class`, with `qualifier` the same specifier).

### 23.3 The entity

`entity` follows the written-reference referent rules (§18):

- a target `Decl(t)`: none, the target is the entity
  (`FriendFormView::entity` returns `t`);
- a specialization ([temp.friend]/1.3): the template its match chose;
  before the match (`DEPENDENT`) the one function template found, none
  when several were (the client compares `found`);
- a member template of a non-dependent class (`friend void
  Bq<8>::t<sizeof(T)>();`): that template;
- a dependent qualifier naming a specialization of class template `A`:
  the member of `A`'s primary pattern found by the name and, for a
  function, the declared type; none when the primary has no such member
  (it may be in an explicit or partial specialization);
- a constructor, destructor or conversion function: that special member
  of the class (its pattern when dependent), else none.

**With `DEPENDENT` set, `entity` is a CANDIDATE, not the referent**: what
the friend befriends is decided per specialization of its class, and an
explicit or partial specialization may make it another declaration (or
none). `found` never lists partial specialization candidates.

The written-reference entry at `name_loc` has the row's entity (WP-3008,
§18.4): one source of truth. A `DEPENDENT` row's entry is `DEPENDENT`
too, with the candidate as its entity (printed `dep`), and a
specialization's keeps its found set.

### 23.4 Reading it

`Ast::friend_form(fd)` answers in three ways:

- `FriendFormAnswer::Row(view)`: the row, through `FriendFormView`
  (`kind`, `flags`, `name`, `name_loc`, `qualifier`, `qualifier_loc`,
  `args`, `entity`, `found`, `heads` / `head(i)` / `head_params(i)` /
  `head_requires(i)`, `requires`);
- `FriendFormAnswer::None`: the friend has no written parts (§23.1);
- `FriendFormAnswer::NotCarried`: the friend came from a module image,
  which does not carry rows (§23.5); its form is unknown, not absent.

`DeclArena::friend_forms()` lists `(Friend, row)` pairs in declaration
order.

Sema matches friends from the row (spec/sema-class.md §2.4): a
specialization's name, arguments, found templates and location; a
special member's kind, head and qualifier; a [temp.friend]/5 friend's
qualifier, name, function type and requires-clause; a qualified
friend's qualifier and the context it names. Sema keeps only its own
state beside it: the substitution levels and caches of
`MemberFriends`, the `noexcept` deferral of a friend specialization
(`friend_late_exc`), and a function-to-`Friend` index
(`friend_fn_decls`).

### 23.5 Snapshots, modules

A snapshot (PCH) carries the rows and the map with the `Ast` (`Snap`),
in order (`pch_snapshot` `FORMAT_VERSION` 172). A module image does not
carry them: an imported `Friend` answers `NotCarried`, and the friends
that would need a row fall back as before (bmi_image/friend.rs, phase 2
of the design: row WP-3230).

### 23.6 The dump

`-fkiln-dump-friend-forms` (with `-fsyntax-only`, to the output, after
the other kiln dumps) prints one line per row in declaration order:

```
<friend loc> FriendForm <kind> [qualifier '<nns>'] name '<name>'@<loc>
  [args <args>] [heads <n>] [requires] [entity <decl>@<loc>]
  [found [<decl>@<loc>, ...]] [global] [template] [every-spec]
  [dependent] [unsupported]
```

(one line; wrapped here), positions as `line:col` in the main file and
`file:line:col` elsewhere. Tests: `crates/kiln_parse/tests/friend_forms.rs`
(dump goldens of the reproducers above and of special members,
[temp.friend]/5 friends (`UNSUPPORTED`), `::` and a non-templated class;
pairs differing only in an instantiation-dependent expression or a
head's constraint; a trailing requires-clause; `template`; the
instantiated target `h<int>`; the written-reference agreement) and
`crates/kiln_driver/tests/friend_forms_carry.rs` (the dump option's
golden, a snapshot round trip in order, `NotCarried` through a module
image). A head's default argument cannot tell two rows apart in valid
code: a friend class template declaration has none, and a friend
function template declaration with one must be a definition
([temp.param]/12), which a qualified friend never is ([class.friend]/6).

## 24. Function-name spans (WP-3280)

`__PRETTY_FUNCTION__`, `__FUNCTION__`, `__func__`,
`__builtin_FUNCTION()` and `std::source_location::function_name()`
produce bytes such as `void ns::C<ns::S>::f(T) [T = ns::S]`. A tooling
client that renames an entity must know which of those bytes name which
declaration. `kiln_ast::fn_names` keeps, when asked for, one record per
distinct (string, function) pair: the byte range of every declaration
name the printers wrote, and the declaration it names. The strings
themselves are unchanged (clang 19/22 parity as before); the table is a
side record of how they were printed.

### 24.1 Opt-in

`-fkiln-fn-names` records the table; `-fkiln-dump-fn-names` records and
prints it (§24.6). `FrontEndOptions::fn_names` is the in-process form,
`ParseOptions::fn_names` the parser's. Off by default: no table, no
marker, no work beyond a flag test per string. The options are not part
of a snapshot's identity (spec/pch.md "Identity"), and a module built in
process for an importer records nothing (§24.8). With the environment
variable `KILN_FN_NAMES_VERIFY=1` the table is on and every record is
self-checked as it is made (§24.7).

### 24.2 What is recorded

Every string the predefined-name printer makes for a function: the
literal of a `PredefinedExpr` (`__func__`, `__FUNCTION__`,
`__PRETTY_FUNCTION__`), the made string of a source-location builtin of
role `Function` or `PrettyFunction` (`__builtin_FUNCTION()`, the
`_M_function_name` of `__builtin_source_location()`'s `__impl` object,
per use, WP-3000), and the pretty name of a contract site's function
(reached through `get`, below). A string made for no function (`""`,
`top level`) has no record. Strings the source spells, reflection names
and diagnostic text are not covered.

### 24.3 The entry (16 bytes, a `size_of` unit test)

`FnNameSpan { lo: u32, hi: u32, entity: Option<DeclId>, role: u8,
flags: u8, depth: u16 }`: the bytes `[lo, hi)` of the string name
`entity`.

- `role` (`FnNameRole`): `Scope` (a namespace, class or function before
  the name), `Name` (the function's own name), `Return`, `Param` (a type
  in the parameter list, as the pattern writes it), `Binding` (a
  template parameter's name in `[P = A, ...]`), `Arg` (a name in a
  bracketed argument `A`).
- `flags`: `DEPENDENT` (1: a dependent name; `entity` is `None`),
  `SYNTHESIZED` (2: an unnamed spelling such as `(anonymous namespace)`,
  `(lambda)`, `(lambda at F:L:C)`, `(unnamed struct at F:L:C)`; always
  has an entity), `CLASS_NAME` (4: a constructor's or destructor's class
  name, the `~` excluded), `QUALIFIED` (8: the entry, or the `~` before
  a destructor's, follows `::`, and the `::` follows the previous entry at
  the same depth directly or after that entry's `<...>` arguments or
  `(...)` parameters: `C` in `ns::C`, `f` in `C<int>::f`, `L` in
  `loc()::L`), `INVENTED` (16: the `x` of an abbreviated
  template's `x:auto`, naming the function parameter). Bit 32 is
  reserved.
- `depth`: nesting inside an enclosing entry's template argument list,
  conversion target or scope function's parameter list; the owner of an
  entry at depth `d > 0` is the nearest earlier entry at `d - 1`
  (`FnNameView::owner`).

Entries are sorted by `lo`, nest properly and never overlap at one
depth. Each entity is normalised as §18's entries are: `Ast::first(d)`
of the declaration printed (one id for every name of one namespace or
class). The record key's function stays the declaration the string was
printed for.

### 24.4 Which tokens get entries

Every declaration name the printers write gets exactly one entry over
exactly its bytes, and the entity is the declaration whose name was
written. Keywords, builtin types, punctuation, `virtual `/`static `,
cv- and ref-qualifiers, `...`, literal values, `<dependent type>`,
`auto`/`decltype(auto)` and the `:auto` of `x:auto` get none.

- Namespaces: one `Scope` entry per printed component; an inline
  namespace is not printed; `(anonymous namespace)` is one synthesized
  entry.
- Classes in the scope: the record printed (for `C<ns::S>::`, the
  specialization; `Ast::spec_info` gives its template), its canonical
  arguments one depth in.
- The function's name: the identifier; an operator is one entry over
  `operator==`, `operator()`, `operator new`, `operator""_x`; a
  conversion function is one entry over `operator <type>` with the
  type's names one depth in (the target is the name's type, printed
  canonical: `operator W<S>` names the specialization `W<S>`, while a
  written return type `W<S>` names the template); a constructor's or destructor's class name,
  `CLASS_NAME`.
- Lambdas: the closure is a synthesized `Scope` over its spelling
  (`(lambda)` with clang 22, `(anonymous class)` under
  `-fclang-compat=19`), the call operator the `Name` `operator()`; as a
  type, `(lambda at F:L:C)` is one synthesized entry, path included.
- A function as a scope (a local class's member): a `Scope` entry for
  the function, the names of its printed parameters one depth in. The
  parameters print as their declarations write them (spec/sema.md §7.8),
  so a typedef or alias names the typedef or alias and a `decltype`
  operand's names their declarations; an instantiation's are the
  written types with its arguments substituted (WP-3402).
- Return and parameter types are printed as the pattern writes them: a
  typedef or alias name names the typedef or alias, a template-id the
  alias or class template (arguments one depth in), an injected-class-name
  the pattern record, a template parameter the pattern's parameter
  (an unnamed one, printed `type-parameter-D-I`, is synthesized).
- Dependent names: `DEPENDENT`, no entity; their non-dependent parts
  keep theirs.
- `[P = A, ...]`: `P` is a `Binding` of the template parameter (the
  same id a `Param`/`Return` entry of it carries); `x:auto`'s `x` is an
  `INVENTED` binding of the function parameter; `A` is printed
  canonical, so its type entities are namespaces, classes, enums and
  closures, never aliases; value arguments carry the names the constant
  printer writes (enumerators, variables, members); a pack's elements
  stay at the pack's depth.

Example, `void ns::C<ns::S>::f(T) [T = ns::S]`:

```
[5,7)   scope ns
[9,10)  scope C -> ns::C<ns::S>, q
[11,13) scope ns, d=1
[15,16) scope S, d=1, q
[19,20) name f, q
[21,22) param T -> template parameter T of C
[25,26) binding T -> the same parameter
[29,31) arg ns
[33,34) arg S, q
```

### 24.5 Reading it

`Ast::fn_names` holds the table: `is_on`, `get(lit, function)`,
`len`, `entry_count`, `iter(ast)`, `unrecorded_prefix()`. The accessors
return a `FnNameView { function, lit, bytes, spans, offset }`, with
`text(i)`, `owner(i)`, `qualifier(i)` and `visible(len)`, which clips
the entries to the `len` bytes from `offset` (the view of a client
holding a pointer into the string) and marks every clipped entry
`partial` (never dropping one silently).

- `fn_name_of_string(lit, origin, offset)`: the fields of a
  `PtrBase::StringLit` pointer and its byte offset, as a client meets
  them walking a constant.
- `fn_name_of_value(v)`: the same for a frozen pointer scalar.
- `fn_name_of_predefined(e)`: a `PredefinedExpr`'s string.
- `fn_name_at_use(u, FnNameAt::Function | FnNameAt::Location)`: the
  function name at a source-location use (`SourceLocExpr`,
  `CXXDefaultArgExpr` or `CXXDefaultInitExpr` with values, WP-3279):
  `__builtin_FUNCTION()`'s string, or the `__impl` object's
  `_M_function_name`.

With the table on, the answer is `None` exactly when:

1. the origin is neither a made string of role `Function` or
   `PrettyFunction` nor a `PredefinedExpr`'s literal (a literal the
   source spells, a reflection or contract string, a file name);
2. the string names no function (`""` or `top level` outside one);
3. the string was made in a restored PCH prefix (`unrecorded_prefix()`
   is then true), or it is the literal of a `PredefinedExpr` in a body
   read from a module image (no carriage yet, row WP-3340);
4. `offset` is past the end of the string;
5. for `fn_name_of_predefined` and `fn_name_at_use`: the node is not of
   the asked kind, or has no values (a dependent `PredefinedExpr` in a
   pattern, a use whose default holds no source-location builtin), or
   `Location` is asked of a use with no `__impl` object.

Off, every accessor answers `None`. A string the Oven builds from these
bytes (a copy, a `std::string`) is a new object with no entries: only a
pointer into the made string itself carries them. Two calls of one
function with different deduced arguments (`f(1)`, `f(1L)`) are two
functions and two records; two uses of one default argument in one
function share a string and a record.

### 24.6 The dump

`-fkiln-dump-fn-names` (with `-fsyntax-only` or a compile, to the
output, after the other kiln dumps) prints the records in `iter(ast)`
order: by the function's location, then the bytes, then the function's
key name (spec/types.md §12.1), a function of the source only:

```
fn-name '<bytes>' of <function>@<loc>
  [lo,hi) <role> '<text>' -> <entity>@<loc> [d=N] [q] [dep] [synth] [class-name] [invented]
```

`<entity>` is `(dependent)` for a dependent entry; a specialization is
printed with its arguments, a parameter unqualified. Positions are
`line:col` in the main file and `file:line:col` elsewhere. After a
restored PCH the dump starts with `fn-names: strings of the restored
prefix have no records`.

### 24.7 How it is recorded, and the self-check

The printers are the ones that make the bytes: with a sink
(`FnSpanSink`) they write in-band markers (private-use characters of
plane 15: an open marker carrying the entry's index, a close marker,
and group markers around template argument lists and a scope function's
parameters) at `AstNames::write_name`, the unnamed spellings, the type
printer's dependent identifiers and template parameter names. The
markers move with the bytes through every splice; `FnSpanSink::strip`
removes them in one pass and computes ranges, depth and `QUALIFIED`.
Without a sink no marker is written. When a (string, function) pair is
first made with the table on, sema prints it a second time with a sink
and keeps the record only if the stripped bytes equal the string's.

`fn_names::verify` checks a record: bounds and order, proper nesting,
that an entry has an entity exactly when it is not `DEPENDENT`, that the
entity exists, that a plain entry's bytes are its entity's identifier,
and the converse: every identifier in the bytes outside all entries
(not part of a number, character or string literal) is in a closed list
of keywords, builtin type spellings and fixed printer words, or a
builtin type family (`__SV`, `__clang_sv`, `__rvv_`, `__builtin_`); a
builtin type spelled with a scope (`std::meta::info`, the reflection
type, which has no declaration) is skipped whole when none of it is in
an entry (WP-3402).
Under `KILN_FN_NAMES_VERIFY=1` a mismatch or a failed check is the error
`sema_fn_names_verify_failed`. The lane runs over `tests/conformance`
(`KILN_FN_NAMES_VERIFY=1 cargo xtask test tests/conformance`: every
compile inherits the variable) and over `<bits/stdc++.h>` with
`_GLIBCXX_ASSERTIONS`.

### 24.8 Snapshots, modules, cost

Stage A does not carry the table. A snapshot (PCH) restores none
(`Snap` skips it; `FORMAT_VERSION` 178 only for the new diagnostic id
of the self-check, which the schema hash covers), and a consumer that
records sets `unrecorded_prefix`: the strings made by the prefix have no
records, those made after it do. A module image carries none
(`IMAGE_VERSION` unchanged): an importer's own strings are recorded, a
`PredefinedExpr` in an imported body is not. Carriage of both is row
WP-3340.

Cost: off, nothing. On, 16 bytes per entry plus a 16-byte record and an
index slot per string, the `fn_names` row of `-fkiln-memory-report`.
Measured with `-std=c++26 -D_GLIBCXX_ASSERTIONS -fsyntax-only` (WP-3280):
`#include <bits/stdc++.h>` alone makes 93 records with 699 entries,
`fn_names` 14916 bytes live (19856 reserved) of 43.27 MB; a unit that
also uses `vector`, `map`, `sort`, `regex` and `format` makes 118 records
with 1028 entries, 21148 bytes live (35648 reserved) of 49.94 MB. The
total live memory differs, flag off against on, by exactly the row; off,
there is no row. Both pass the self-check.

Tests: unit tests in `crates/kiln_ast/src/fn_names.rs` (entry size,
marker round trip, depth, malformed markers, clipping, the converse
check), `crates/kiln_parse/tests/fn_names.rs` (the accessors, per-use
strings, deduction, contracts, off), and the `fn-names:` goldens under
`tests/conformance/fn_names/`.

## 25. Static-initialization facts (WP-3650)

A tool that reads the AST (a binding generator, a refactoring tool) needs
to know, for each variable of static or thread storage duration, what
code generation will do with it: run an initializer at program start or
not, register a destructor or not. Without the facts it must evaluate
the initializers again and can disagree with the compiler. With
`-fkiln-static-init-facts` (`FrontEndOptions::static_init_facts`,
`ParseOptions::static_init_facts`, the transient `Ast::static_init_facts`)
sema records both on the variable, and code generation reads them
instead of deciding again, so the two agree by construction. Off by
default: off, nothing is recorded, nothing is evaluated beyond what was
before, and the IR is unchanged.

### 25.1 The rule (what code generation does)

For a variable `v` of static or thread storage duration with a
definition and initializer `e` (`kiln_codegen` `fold::prepass`):

- **Initialization** ([basic.start.static], [basic.start.dynamic]):
  - no initializer, or a trivial default construction: zero-initialized,
    *constant*;
  - `e` is a constant initializer ([expr.const]/2: evaluated as a
    constant expression in a manifestly constant-evaluated context,
    float UB folded, leaked allocations allowed as clang folds them) whose
    value holds no address of an automatic object: *constant*, the
    value is emitted;
  - otherwise, when `v`'s type is arithmetic, enumeration, pointer,
    pointer to member, vector, or `e` is an initializer list, and `e`
    folds outside a constant context (clang
    `ConstantEmitter::tryEmitPrivateForVarInit`; integer-pointer casts
    allowed): *early static* ([basic.start.static]/3), the folded value
    is emitted and no initializer runs. A static local in a function
    whose floating-point environment a fold must not assume is never
    folded so;
  - otherwise *dynamic*: an initializer runs at program start (a
    namespace-scope or static member variable, ordered or unordered per
    [basic.start.dynamic]), on first use (a static local, guarded), or
    per thread (`thread_local`).
- **Destruction** ([basic.start.term]):
  - a trivially destructible type, or a reference: *trivial*, except a
    reference whose dynamic initialization materializes a static or
    thread temporary ([class.temporary]/6) with a non-trivial destructor
    and no constant value: that temporary's destructor is *registered*;
  - a type with a non-trivial destructor whose destruction of the
    constant value is a constant expression ([expr.const]/7, evaluated in
    a manifestly constant-evaluated context only when `v` is
    constant-initialized): *constant*, nothing is registered;
  - otherwise *registered* (`__cxa_atexit`, `__cxa_thread_atexit` for a
    `thread_local`). `[[clang::no_destroy]]` and
    `-fno-c++-static-destructors` still suppress the registration in code
    generation; the fact records the kind before them.

### 25.2 When it is recorded

At the end of the translation unit, after its pending instantiations
(`Parser::end_translation_unit` → `Sema::record_static_init_facts`), for
every `Var` and `VarTemplateSpecialization` declaration that is a
definition, has static or thread storage duration, a non-dependent type,
and no template pattern among its contexts: namespace-scope and static
member variables, instantiated static data members of class templates,
variable template specializations, static and `thread_local` locals
(including those of instantiated function templates). Not in C (no
dynamic initialization) and not in a prefix (a precompiled header: the
unit that includes it records its own). Each entity is decided once,
on the declaration that holds its initializer (else its definition);
`Ast::static_init_fact(d)` reads it from any redeclaration.

Each variable is evaluated at most once. A variable whose `VarData.eval`
is already written (§9.2: `constexpr`, `const` integral) reuses it; for
any other the constant-initialization result is written to `eval` then
(`Value(id)` or `NotConstant`; a resource limit writes nothing), as
clang's `CheckCompleteVariableDeclaration` caches it. Code generation
emits a *constant* variable's value from `eval`, and reruns only the
early-static fold (the same `Oven::fold_static_fallback` the decision
ran). Writing `eval` for such a variable is the one change the option
makes beyond the fact: an `eval` value read later (debug info of a
static data member, a lax fold of a `const` object) is the same value
code generation would have folded.

Not decided, `NotEvaluated(why)` (code generation then decides on its
own as before): `invalid` (an invalid declaration), `init-errors` (an
initializer with errors), `step-limit`, `depth-limit`, `memory-limit`
(the evaluation hit a budget), `stack` (too little native stack, or a
variable caught mid-evaluation), `flexible-array-member` (a GNU flexible
array member given elements: folded apart from the object),
`fp-environment` (a static local whose initializer is not constant, in a
unit with an environment-dependent floating-point state). A
value-dependent initializer has no fact.

### 25.3 Storage

`kiln_ast::static_init::StaticInitFact { init: StaticInit, destruction:
Option<StaticDestruction> }`, in spare bits 16 to 23 of `VarSpec`
(`VarSpec::static_init_fact`, `DeclArena::set_static_init_fact`): bits 16
to 18 the initialization (0 none, 1 constant, 2 early static, 3 dynamic,
4 not evaluated), 19 to 21 the reason, 22 and 23 the destruction (0
undecided, 1 trivial, 2 constant, 3 registered). No node grows; a
round-trip unit test covers the encoding.

### 25.4 Dump

With the option on, a `VarDecl` line with a fact ends in
` static-init=<constant|early-static|dynamic|not-evaluated(<why>)>` and,
when decided, ` static-destroy=<trivial|constant|registered>`. Off, the
dump is unchanged.

### 25.5 Snapshots and modules

The option is not part of a snapshot's identity (spec/pch.md
"Identity"). A prefix records no facts, so a snapshot holds none (the
`VarSpec` bits are carried raw; `FORMAT_VERSION` unchanged). A module
image never carries them: its writer masks them
(`VarSpec::without_static_init_fact`, `IMAGE_VERSION` unchanged), and an
importer's own definitions get its own facts.

### 25.6 Self-check

`KILN_STATIC_INIT_FACTS=1` in the environment turns the option on for
every compile to code (not `-fsyntax-only`, whose dumps would change):
`KILN_STATIC_INIT_FACTS=1 cargo xtask test tests/codegen` checks that
code generation reading the facts gives the IR it gives deciding alone.

Tests: `crates/kiln_ast/src/static_init.rs` (encoding round trip),
`crates/kiln_driver/tests/static_init_facts.rs` (every kind, the IR the
same with the option on and off).
