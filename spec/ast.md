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
parsing, then moved to codegen. Sema is single-threaded per TU (§13 Q8);
`Ast: Send`, not `Sync`-shared during construction.

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
    misc: DeclMisc,      // u16: access(2) friend(2) local_extern(1) module_ownership(3)
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
- The lexical context is stored in a sparse map `lexical_ctx: FxHashMap<
  DeclId, DeclId>` only when it differs from `ctx` (out-of-line member
  definitions, friends, local externs): flag `HAS_LEXICAL_CTX`.
- Source range: the begin location (decl-specifiers start) is in the
  payload of kinds that need it for diagnostics; the end is computed.
- Identifier namespace (IDNS: ordinary / tag / member / namespace / label
  / template-param) is a function of `kind` and `misc.friend`/
  `local_extern`, not stored.

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
  `UsingShadow` per declaration the lookup found that was not already
  introduced, an `UnresolvedUsingIfExists`'s for `using_if_exists`), an
  inheriting-constructor `using B::B;` (one `ConstructorUsingShadow` per
  constructor of the base, its implicit copy and move constructors
  included, that no constructor the class declared earlier hides), and `using
  enum E;` (one per enumerator). A dependent using-declarator
  (`UnresolvedUsingValue`, `UnresolvedUsingTypename`) introduces none;
  the `Using` its instantiation makes in the class specialization (or in
  the instantiated function body) has the instantiation's shadows. A
  shadow hidden later ([namespace.udecl]/14, `hidden_shadow`) stays in
  the list. The module image carries the link (each shadow record names
  its using-declaration's record, spec/modules.md §11.11 kind 21), and a
  snapshot carries the maps. Cost: a forward-map entry and an 8 B list
  span per using-declaration, 4 B per shadow in the list arena and a
  reverse-map entry per shadow (9 B live each); about 24 KB on a TU of
  fifteen standard headers, spec/landed/ast/WP-2025.md.
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
                                    //   (`virtual` is semantic: written or overriding; `virtual_written`
                                    //   is the specifier, which the dump prints. LATE bits, set after
                                    //   the body or at `}`: trivial, has_contracts, is_coroutine,
                                    //   escalated, deleted)
    templ: TemplLink,               // §8.2
}
struct VarData {                    // 24 bytes
    redecl: Redecl,
    init: Option<ExprId>,
    spec: VarSpec,                  // u32: storage(3) tls(2) init_style(2: c/call/list/paren-list) constexpr(1) constinit(1)
                                    //   inline(1) nrvo(1) exception_var(1) cxx_for_range(1) ...
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
  ([basic.link]/6; external when it is the first). A namespace is
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
  `extern "C"` context keeps it, as in clang. In C every function and
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
reads the payloads. Tests: `crates/kiln_parse/tests/using_linkage.rs`.

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
  | `OffsetOfExpr` | `ListId` of the array index expressions | `b` = byte offset (`u32`); with cold `offsetof_layout` steps (an index that is not a constant, or an offset past `u32`) the value is their sum | `offsetof_value`, `offsetof_layout` |
  | `CompoundLiteralExpr` | the `InitListExpr` | | `compound_literal_init` |
  | `CXXScalarValueInitExpr` | none | | |
- Source ranges are computed (`begin(e)` walks to the leftmost child;
  kinds ending in a closing token store it in their side record or cold
  map); no stored ranges.
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
  column: u32, location: Option<ConstValueId> }`; the `ConstValueId`s are
  interned pointers to string-literal objects (one per distinct string)
  and, for `location`, to the frozen `__impl` object of that location.
  The key is the `SourceLocExpr` itself (its own location and context),
  or a `CXXDefaultArgExpr` that uses a default argument containing one
  (the call's location and the caller's context, [dcl.fct.default]/9 via
  [support.srcloc.cons]/2), or a `CXXDefaultInitExpr` using a default
  member initializer containing one (the constructor's or the aggregate
  initialization's location and context). Consumers (Oven, codegen) take
  the values of the outermost `CXXDefaultArgExpr`/`CXXDefaultInitExpr` in
  scope that has an entry, else the `SourceLocExpr`'s own.

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
  or type; the alias (`using A = ...` / an alias template); the variable
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
  Template parameters, template heads and alias declarations claim
  closures only (not written references). Carried by PCH snapshots
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
`SPACE_DEPENDENT`) until instantiation. The bit lives in the existing
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

### 8.3 Specialization keys
One global `specializations: FxHashMap<(DeclId /*first decl of template*/,
ArgListId /*canonical*/), DeclId>` for class, function, variable, alias
(alias specializations are types, not entries) and partial
specializations (keyed by their canonical pattern args; listed per
template in `partials: DeclId -> ListId`). The template decl's payload
lists its specializations in creation order (dump order, ast-dump.md §5).

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
type-dependent operand, built in a template, keeps the non-member
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
  sets).
- Written by sema at the expression (`note_dependent_oper`,
  spec/sema-overload.md §2.6) and by the module-image importer, which
  carries the set with the expression (`COLD_OPER`, spec/modules.md
  §11.11). Snapshots carry the table; the hash index is rebuilt on the
  first intern after a read (§17). Memory rows `templ.oper_lookup`,
  `templ.oper_sets`, `templ.oper_cands`, `templ.oper_index`.

Tests: `crates/kiln_parse/tests/sema_facts.rs`
(`dependent_operator_definition_set`), `oper_lookup.rs` unit tests,
`tests/conformance/module.interface/image-oper-lookup-run.cpp`.

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
              past: bool },                       // one past the end of a subobject (WP-948)
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
  (spec/oven.md). Identity is `(type, base, offset)`. The designator path
  is not stored: printing recovers it from the type and offset (§9.3).
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
  | pointer | 16 (+8 if the offset is ≥ 2^32; +16 for a designated type) |
  | member pointer | 16 + 4 per path entry |
  | aggregate | 16 + 24 (pool starts and cached 64-bit hash) + `sizeof(T)` + 8 per relocation, padding run and union entry |

  - Every value also costs about 5 bytes of intern table. The table is a
    hashbrown `HashTable<ConstValueId>` keyed by FxHash. It stores ids
    only; keys are re-read from the pools.
  - An aggregate's hash is cached, so growing the table never rehashes
    images.
  - Pools are `u32`-indexed: 4 GiB of image bytes in total.
  - `HeapSize` reports these rows: `nodes`, `words`, `paths`, `aggs`,
    `bytes`, `relocs`, `padding`, `unions`, `intern`.
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
    `path`. The offset is unsigned: a pointer before its base is never a
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
| `VarData.eval` | `NotEvaluated → Evaluating → Value(id) / NotConstant`; `Evaluating` detects self-reference; used for `constexpr`/`constinit` vars, `const` integral vars usable in constant expressions, and static-storage constant initialization (codegen emits the value) |
| `EnumConstant` payload | value (`Int`) |
| `TemplateArg::Value` | the converted non-type argument (spec/types.md §8.3) |
| `TemplateParamObject` | the object's value |
| `IfStmt` | `if constexpr` via its `ConstantExpr`; `if consteval` needs no value |
| `Annotation` attr | annotation value (M6) |

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
`Ast::file_scope_asm_string`). `labels` is always empty (no `asm goto`).

**Landed (WP-140): `PredefinedExpr`.** `__func__`, `__FUNCTION__`,
`__PRETTY_FUNCTION__` ([dcl.fct.def.general]/8; clang `PredefinedExpr`):
`aux` is the `PredefinedKind` (`Func`, `Function`, `PrettyFunction`;
`Exprs::predefined_kind`), `a` the `StringLiteral` holding the name
(`Exprs::predefined_name`, its only child). An lvalue of `const char[N]`;
in a dependent context it has type `QualType::DEPENDENT` and no child.
Built by `AstBuilder::predefined_expr(kind, ty, name, loc)`.
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
  (§18: not written, read back empty and off; the restoring compilation
  turns recording on again from its own options, so a written-reference
  dump covers the names after the snapshot only, until stage 5).
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
field `Ast::written_refs`. Built in five stages:

| stage | content | row |
|---|---|---|
| 1 | the table, its iterators and dump; expression names, member names, nested-name-specifier components before them, with the `qualifier` chain | WP-1930 |
| 2 | type names in every type-specifier context, template names, base specifiers, member-initializer ids | WP-1931 |
| 3 | declarative contexts: using-declarations and -directives, namespace alias targets, declarator-id qualifiers and the names they redeclare, elaborated and friend references, specialization targets, concept names, designators, deduction-guide names; their lookup provenance; the identifier-accounting test (§18.8) | WP-1932, WP-2100, WP-2131 |
| 4 | per-specialization entries for dependent names | WP-1933 |
| 5 | carriage in PCH snapshots and module images | WP-1934 |

### 18.1 Contract

- **Opt-in.** `WrittenRefs::is_on()` is false unless the front end's
  `ParseOptions::written_refs` is set (the driver's
  `-fkiln-dump-written-refs` sets it). Off, every recording site costs
  one predictable branch on that flag and allocates nothing; the table
  is empty, `Ast::memory()` has no row for it and the snapshot and image
  layouts are those of a compiler without it.
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
| `range` | `Span` (8) | the name as written: `lo` the start of its first token, `hi` the end of its last token, as kiln token locations (macro provenance intact, §18.5). The name only: `a::b::f` is three entries. An operator-function-id (`operator==`, `operator""_x`, `operator new[]`) is one entry over all its tokens; a destructor name `~T` has the range of `T` |
| `entity` | `Option<DeclId>` (4) | the declaration the name denotes after resolution: for an overloaded name, the function overload resolution selected (a function template specialization when it selected one); through a using-shadow, the underlying declaration. `None` for a dependent name in a template pattern and for a name whose resolution failed (flags tell which) |
| `found` | `Option<DeclId>` (4) | what lookup found when it differs from `entity`: the using-shadow (`found_decl` of the reference), or the function or variable template whose specialization `entity` is |
| `enclosing` | `Option<DeclId>` (4) | the innermost declaration whose source extent contains the name's first token (§18.3); `None` only when the table was filled without a TU |
| `qualifier` | `RefQualifier` (4) | for a name written after `::`: the entry of the component immediately to its left, or `GLOBAL` for a leading `::`; `NONE` for an unqualified name |
| `role` | `RefRole` (1) | the syntactic role (§18.4) |
| `flags` | `u8` (1) | `DEPENDENT` (a dependent name in a template pattern: no entity until instantiation), `UNRESOLVED` (lookup or overload resolution failed), `MEMBER` (a member or member template name written after `.` or `->`, §18.7) |
| `how` | `RefLookup` (1) | how lookup found the name (§18.7, WP-2130) |

One padding byte remains. Deviation from a 4-byte `entity`-only design:
`found` and `qualifier` are stored, not derived, because deriving `found`
needs the expression (not kept for nested-name-specifier components) and
deriving the chain needs range arithmetic over tokens.

`RefQualifier` is a `u32`: 0 none, `u32::MAX` global, otherwise a
`WrittenRefId` index + 1. A qualifier entry always precedes the entry it
qualifies.

### 18.3 `enclosing`

Stored, not derived from ranges. The rule: the innermost declaration
whose source extent contains the name's first token — the particular
redeclaration (a reopened namespace, the definition of a function, the
class being defined). For a name inside a declaration (its type, its
initializer, its default argument, its declarator-id qualifier) that is
the declaration being declared.

Mechanism: an entry is recorded with sema's current lexical context
(`Sema::ctx`: the namespace, class or function whose body is being
parsed). A declaration whose names are parsed before its `DeclId`
exists claims them: the parser takes `Ast::written_ref_mark()` (entry
count, decl count) at the declaration's start and calls
`claim_written_refs(mark, d)` once `d` exists (after the declarator,
again after the initializer). A claim sets `enclosing = d` on entries
after the mark whose current `enclosing` was created before the mark,
so a declaration nested inside (a lambda, a local class, a parameter)
keeps its own entries: the innermost claim wins. Claimers in stage 1:
simple declarations (each declarator), function definitions, member
declarators, function parameters (their default arguments included,
also when late-parsed), late-parsed default member initializers and
concept definitions (their constraint-expression). A requires-expression's
parameter scope is skipped: its names are enclosed by the context
around it. Other declarations (template parameters and a template
head's requires-clause, enumerators, alias and `static_assert`
declarations, structured bindings, condition variables) leave their
names enclosed by the lexical context; WP-1980 adds them. Claimers
added in stage 3: using-declarations (each using-declarator claims its
qualifier and name), namespace alias definitions (the target and its
qualifier), a tag that a class-specifier, enum-specifier or
elaborated-type-specifier declares (its class-head-name and qualifier;
an explicit or partial class specialization's template name), and the
declarators of stage 1 also claim their `redecl`, `spec-target` and
`elaborated` entries. A friend's tag names are not claimed: they stay
enclosed by the class granting friendship. A using-directive declares
no named entity, so its names stay enclosed by the context around it.
The same marks and claims decide closure owners (§5.9, WP-2023): every
claim also claims the closures created since its mark, and template
parameters, template heads and alias declarations claim closures (not
yet names).

### 18.4 Roles

`RefRole` is `#[repr(u8)]` and covers the full set; stage 1 fills
`Expr`, `Member` and `Nns`, and `Concept` for a concept-id parsed as an
id-expression; stage 2 (WP-1931) fills `Type`, `Template`, `Base` and
`MemInit`; stage 3 (WP-1932) fills `UsingDecl`, `UsingDirective`,
`NamespaceAlias`, `Elaborated`, `SpecTarget`, `Designator` and
`Redecl` (added by stage 3), `Concept` for a type-constraint, and
`Template` for a deduction guide's name.

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

Not in the table: a declaration's own name at its declaration (an
unqualified first declaration, including a first `friend class Y;`),
keywords, labels, attribute names, module names, and implicit references with no
written name (implicit conversions and constructor calls, operator
uses such as `a == b`, coroutine and structured-binding helpers).

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
  has no `qualifier` link;
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
  role `type`, entity the typedef or template parameter;
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
  qualifier (`friend class T::X;`) has no entity and `DEPENDENT`. A
  friend function template specialization (`friend void f<>(int);`)
  is `elaborated` too, its entity the template;
- specialization targets, role `spec-target`: the template name of an
  explicit specialization ([temp.expl.spec]/1), a partial
  specialization ([temp.spec.partial.general]/1) and an explicit
  instantiation definition or declaration ([temp.explicit]/2, `extern
  template` included), after the names in its template argument list:
  a class template's in `parse_class_template_id_tag`, a function or
  variable template's in `parse_specialization_declarator_id`. Its
  entity is the primary template. An explicit instantiation or
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
  each member name of `__builtin_offsetof(T, a.b[i].c)`
  ([support.types.layout]/1), with `MEMBER` and the member lookup found
  (the member written, past the anonymous members on its way), `DEPENDENT`
  for each name of a designator deferred to instantiation, `UNRESOLVED`
  for a name lookup did not find;
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
  `DEPENDENT`. Instantiation records nothing. Stage 4 adds a separate
  table keyed by (pattern entry, specialization) — reserved, not
  allocated: the main table's entry shape does not change. Default
  arguments and default member initializers are entries once, at the
  written site.
- **PCH and modules.** Not carried in stage 1: the field is transient
  in the snapshot (§17), so a TU restored from a snapshot or importing a
  module has entries for the names it parsed itself only. Stage 5 writes
  the table into snapshots and images.

### 18.6 The dump

`-fkiln-dump-written-refs` (with `-fsyntax-only` or any compiling mode)
prints, after diagnostics' end of the TU, one line per entry:

```
<line>:<col>-<line>:<col>  <role>  '<spelling>' -> <entity> [found=<decl>] [q=<line>:<col>|q=::] [in=<enclosing>] [dep] [exp=<line>:<col>] [how=<how>] [udir=<ns>@<line>:<col>] [adl-ns=<ns>,...] [adl-cls=<class>,...] [adl-enum=<enum>,...]
```

Positions are the spelling locations of `lo` and `hi` (the end column
is one past the name's last character), prefixed with the file's base
name and `:` when not in the main file. `<spelling>` is the source text
of the range. The fields after `exp=` are the lookup provenance
(§18.7.5). `<entity>`, `found=` and `in=` print the declaration's
qualified name (lambdas as `(lambda)`; a function or template
parameter unqualified) and `@<line>:<col>` of its location; no entity prints `(dependent)` or `(unresolved)`; `in=(tu)`
for the translation unit. `q=` names the qualifier entry by its start
position. `exp=` is the outermost expansion point of a name written in
a macro. The harness directive `written-refs: <golden>`
(spec/harness.md) compares this dump with a golden file.

Tests: `crates/kiln_ast/src/written_refs.rs` (entry size, off records
nothing, claims, truncation), `crates/kiln_parse/tests/written_refs.rs`
(off and on, `enclosed_by` on stage-2 and stage-3 entries),
`crates/kiln_parse/tests/written_refs_accounting.rs` (§18.8),
`tests/conformance/written_refs/` (one per role family of every stage,
the worked example, macros, tentative parses, overload patching,
dependent names, error recovery; stage 3: `using-decls`,
`using-directives`, `redecls`, `elaborated`, `spec-targets`,
`concepts-decl`, `designators`, `guides`, `decl-macros`,
`decl-dependent`, `decl-tentative`, `decl-errors`).

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
  the snapshot and image layouts are those of a compiler without it.
- **The entry stays 28 bytes**: `how` is one byte in the entry's former
  padding (§18.2). The using-directive and the argument-dependent set
  live in a sparse side table keyed by `WrittenRefId`, with the sets
  interned (one copy per distinct set), so an entry with neither costs
  nothing extra. Transient like the entries (§18.5): not in snapshots
  or images until stage 5 (WP-1934); `truncate` drops the side rows of
  the entries it drops.
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
lookup at instantiation (stage 4): `None`, no set.

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
  `__attribute__((...))`, `__declspec(...)` or the attribute operand of
  `__builtin_has_attribute(e, ...)`, or before `::` in one), an
  attribute's keyword argument (a lone identifier argument of an
  attribute that reads it as one of its own words, clang
  `attributeHasIdentifierArg` / `VariadicIdentifierArgument`:
  `mode(byte)`, `format(printf, 1, 2)`, `cpu_dispatch(atom)`,
  `enum_extensibility(open)`, `visibility`, `pcs`,
  `zero_call_used_regs`, `function_return`; and the builtin a builtin
  alias names, `__clang_arm_builtin_alias(__builtin_x)`),
  a label (`l:`, `goto l`, `&&l`, the names of a `__label__`
  declaration, an `asm goto` label),
  an `asm` symbolic operand name, or a module name (after `module` /
  `import` up to `;`).

Macro names, directive text, `#pragma` text and the text of skipped
conditional blocks never reach the parser and are not counted. A file
with an error is skipped (its recovery records what it can, §18.4).
The default test runs over `tests/conformance/written_refs/` and wants
nothing left over; `KILN_REFS_DIR=<dir>` (an ignored test) runs it over
a directory tree and prints what is left grouped by the token before
it, `KILN_REFS_STRICT=1` making any leftover fail.

Over `tests/conformance` (WP-1932: 3388 files without errors, 311138
identifiers) the run found 2381 identifiers left over. The table gaps
it found were fixed here: the names of `sizeof...(pack)` (stage 1),
offsetof member-designators, and §19 rows for for-range-declarations
and expansion-statement variables and their bindings, a condition's
bindings, exception declarations, requires-expression parameters and
init-captures, and friend declarations that overwrote the row of the
class or member they name. The classes left (82 identifiers) are rows
of their own:

- names in the arguments of thread-safety attributes
  (`guarded_by(mu)`, `requires_capability(mu)`, `acquire_capability(...)`:
  30), which kiln knows and skips unanalysed (WP-606), so their
  names are never looked up (WP-2170);
- the type-names of a qualified pseudo-destructor-name (`p->I::~I()`,
  `p->::N::I::~I()`, `t->T::~T()`, `p->A<int>::~A<int>()`: 40, both the
  name before `::~` and the destroyed name; WP-2171);
- the pack of a pack-indexing nested-name-specifier
  (`typename Ts...[0]::type`: 10; WP-2172);
- two diagnosed files on which the test's in-process front end panics
  in the Oven's `convert` (WP-2141's ICE), reported as `panic` lines,
  not identifiers.

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
- **No row**: implicit declarations (special members, implicit
  deduction guides, builtins, injected class names, a lambda's closure
  type and its members), instantiations and their members (a client
  maps a specialization to its pattern with `template_info` and reads
  the pattern's row), the parameter a `(void)` clause makes and drops,
  and the declarations an error recovery skipped.
  An explicit specialization and an explicit instantiation are written
  declarations: the row is the specialization's, its range that of the
  declaration that names it.
- **Read access** (all on `DeclExtents`): `is_on()`, `get(d)`,
  `iter()` (every declaration with a row in `DeclId` order, a shared row
  once per declaration), `len()` (rows, a shared row once),
  `dump(ast, sm, main)` (§19.7). Helpers on the row: `semi()` (the `;`
  as a span), `qualified_name()` (`[qual, name.hi)`), `prefix()` (the
  decl-specifier part shared by a declarator group).
- **Writes** go through `set(d, e)`, `entry(d)`, `share(alias, d)` and
  `get_mut(d)` (no-ops when off), `forget(d)` (a declaration the
  parse made and dropped) and `mark()`/`truncate(mark)` (§19.6).

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
per declaration up to the last one with a row.

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
declares), a requires-expression's parameters, access specifiers
(`public:`), and a declaration an error cut short. The optional `;` after an in-class member function definition
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
§18.8 found the overwrite).

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
  129), with its on bit; the consumer then sets that bit from its own
  options. `-fkiln-decl-extents` is part of the PCH identity (a producer
  and a consumer must agree); `-fkiln-dump-decl-extents` is not, and
  makes the producer take two passes.
- **Module images** (spec/modules.md §11.11; IMAGE_VERSION 62). A
  module unit parsed with the table on carries a row per record that
  has one and per parameter of a function record, located in the
  image's location space; an importer whose table is on gives the
  declarations it builds the same rows (a template sharing its
  pattern's again). Declarations inside bodies are not carried; an
  extent location the image cannot carry fails the record as any other
  location does. A unit replayed from source records its rows by
  parsing. The module cache key includes `decl-extents`.

### 19.7 The dump

`-fkiln-dump-decl-extents` prints, at the end of the TU, one line per
declaration with a row in `DeclId` order:

```
<Kind> '<name>'  full=<L:C>-<L:C> <;|}|-> [own=..] [name=<L:C>-<L:C> '<text>'] [qual=<L:C>] [body=..] [specs-end=<L:C>] [exp=<L:C>]
```

Positions are spelling locations, prefixed with the file's base name and
`:` outside the main file. `own=` is printed when it differs from
`full`; `<text>` is the name's source text; `exp=` the expansion point of
a `full` that starts in a macro. A lambda in a qualified name prints as
`(lambda at <file>:L:C)` without directories. The harness directive
`decl-extents: <golden>` (spec/harness.md) compares the dump with a
golden file.

### 19.8 Cost

Measured with a release build on aarch64, `-fsyntax-only -std=c++26`
(WP-2090; `-fkiln-memory-report` for the table and AST totals, peak RSS
from the OS):

- *`<bits/stdc++.h>`* (libstdc++ 14): 168,688 declarations in the arena,
  89,071 with a row (the rest are implicit or instantiated). The table
  holds 4,547,312 bytes live: 51 bytes per row (44 for the row, the
  index's 4 per arena declaration spread over the rows), 27 per arena
  declaration. Accounted memory grows from 42.9 MB to 47.4 MB (+10.6%),
  peak RSS from about 91.6 MB to 96.0 MB (+4.7%); compile time is
  within noise (0.9-1.3 s either way).
- *A synthetic TU of 320,000 written declarations* (20,000 namespaces,
  each with a class of three fields and a method, a function of two
  parameters, a variable group, a typedef and an enumeration): 320,000
  rows, 15,360,300 bytes live (48 bytes per row) and 25.6 MB reserved
  (vector doubling). Accounted memory grows from 47.0 MB to 62.4 MB
  (+33%: every declaration is written and there are no instantiations
  to dilute it), peak RSS from 90.9 MB to 107.2 MB (+18%); compile time
  is within noise (0.7-0.9 s).
- *Off* (the default): accounted memory is byte-identical to the build
  before the table, peak RSS and compile time are within noise, and the
  bench/oven counters are identical.

So the table stays opt-in: a tenth of the AST on a header-heavy TU and a
third on one made only of written declarations is too much to charge
every compilation.

Tests: `crates/kiln_ast/src/decl_extents.rs` (row size, off records
nothing, set and share, `semi`/`prefix`, truncation, lambda names),
`crates/kiln_parse/tests/decl_extents.rs` (the API on parsed source),
`crates/kiln_driver/src/snap_tests.rs` (`snapshot_round_trip_decl_extents`)
and `tests/conformance/decl_extents/` (one per case of §19.2-§19.6,
macros, templates, a PCH and a module-image round trip).

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

### 20.5 Stability

Every value in these tables (a row's roles, an enum maximum, a field's
bits, a named constant, a descriptor) is stable within a snapshot
`FORMAT_VERSION`: a change to any of them is a format bump, and the
tests above fail on one that is not.
