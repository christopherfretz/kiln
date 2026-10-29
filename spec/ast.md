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

### 2.4 Mutation discipline
After a node is created its header and side record are immutable, except:

| slot | written | by |
|---|---|---|
| `FunctionData.body` | once (definition parsed or instantiated) | sema |
| `VarData.init`, `VarData.eval` | init once; eval state machine §9.2 | sema / Oven via bridge |
| `RecordData` definition data, `members` | while the class is being defined; frozen at `}` (except `define_aggregate`, §10, and the row below) | sema |
| `RecordData.members.last` of a complete class | append-only: implicit special members and inheriting constructors, declared lazily (`append_implicit_member`, spec/sema-class.md §4.1) | sema |
| `VarData` `nrvo` bit | once, at the variable's scope exit (`set_nrvo`) | sema |
| cold `ctor_inits` of a constructor | once, with the body (`set_ctor_inits`; implicit initializers included) | sema |
| cold `overrides` of a method, `friends` of a class | once (`set_overrides` at the declaration, `set_friends` at `}`) | sema |
| `LetData.value` (`LifetimeExtendedTemporary`) | once | Oven via bridge |
| `FnSpec::deleted` | once, at `}` (`= default` members found deleted; part of `FnSpec::LATE`) | sema |
| `ParamData::DEFAULT_ARG_PENDING`, `ParamData.default_arg` | the flag set at the declaration; the argument once, when the late-parsed tokens are parsed ([class.mem]/7) | parser / sema |
| `DeclNode.ty` | re-pointed for deduced return types and resolved exception specs | sema |
| `RedeclGroup.latest/definition` | on each redeclaration / definition | sema |
| `DeclFlags::{USED, REFERENCED, INVALID}` | monotone set | sema |
| lookup tables, specialization maps, `layouts`, `virtuals`, `vtables`, `consts`, `types` | insert-only | sema / Oven / `kiln_abi` |

**Instantiation adds nodes, never rewrites patterns**: substituting a
pattern creates fresh decls/exprs/stmts (with `instantiated_from` links,
§8.3) appended to the arenas. Consequently instantiated bodies are retained
for the TU's life (§13 Q6).

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
                         //      name_independent(1, P2169 `_`) linkage cache(3) visibility(2) spare(2)
    name: DeclName,      // u32
    ctx: Option<DeclId>, // semantic DeclContext; None only for the TU
    next: Option<DeclId>,// next sibling in the *lexical* context's member list
    loc: Loc,            // the name's location ([dcl.decl]: the declarator-id)
    ty: Option<QualType>,// value decls: declared type; type decls: the declared type; else None
    data: u32,           // index into the kind's payload arena (0 = none)
}
```
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
| variables | `Var`R T, `ParmVar`, `ImplicitParam` (`this`, coroutine frame), `Decomposition` (structured bindings: `bindings: ListId`; P1061 pack), `Binding` (`binding: ExprId`, holding var for tuple-like), `VarTemplateSpecialization`R, `VarTemplatePartialSpecialization`R |
| templates | `ClassTemplate`R, `FunctionTemplate`R, `VarTemplate`R, `TypeAliasTemplate`R, `Concept`, `BuiltinTemplate` (`__make_integer_seq`, `__type_pack_element`), `TemplateTypeParm`, `NonTypeTemplateParm`, `TemplateTemplateParm`, `ImplicitConceptSpecialization`, `RequiresExprBody`★ |
| synthesized | `LifetimeExtendedTemporary` (payload `LetData { extending: DeclId, mangling_index: u32, value: Option<ConstValueId> }`; context the extending declaration's, in no member list or lookup table), `TemplateParamObject` (class-type NTTP object: type + `ConstValueId`), `StaticAssert` (cond `ExprId`, message `ExprId`, P2741), `Label`, `ContractResultName` (P2900 `post(r: ...)`) |

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
                                            spec/sema-class.md §2.9, bit 50 FINAL*/,
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
requires-clauses, contract lists (P2900: `ListId` of `ContractSpec { kind:
pre|post, cond: ExprId, result_name: Option<DeclId> }`), `asm` labels,
default arguments of templates, alignment overrides, deprecation messages.
A cold map costs ~16 B per *entry* and nothing for decls without one;
fields used by fewer than ~5% of decls of a kind go there.

- `asm_label: ColdMap<DeclId, StringLitId>` (WP-063): the GNU asm label
  of a function or variable, keyed by the entity's *first* declaration
  (a label on any redeclaration names the entity); each declaration
  also carries an `AsmLabel` attribute (§7).
- `builtin_id: ColdMap<DeclId, u32>` (WP-063): the builtin id of an
  implicit builtin `Function` decl, read through
  `DeclArena::builtin_id(d)`. Ids and spellings are in
  `kiln_ast::decl::builtin` (`IS_CONSTANT_EVALUATED = 722`, clang's
  numbering; `ALL`; `by_name`).
- `AstBuilder::linkage_spec_head(head, lang)` creates a
  `LinkageSpecDecl` from a full `DeclHead`, so implicit ones get
  `IMPLICIT` at creation (`linkage_spec` delegates to it).

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

### 4.4 Redeclaration chains
`RedeclGroup { first: DeclId, latest: DeclId, definition: Option<DeclId> }`
(12 B), shared by all redeclarations; each redeclarable payload stores
`prev` and `group`. `first(d)`, `latest(d)`, `definition(d)` are O(1).
Types and template keys refer to `first` (spec/types.md: `Record(first)`),
lookup returns `latest`, codegen emits from `definition`. A redeclaration
merges into its predecessor's group (sema checks compatibility). The
`ODR`-checked "is this the same entity" question is group identity.

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
  | `OffsetOfExpr` | `ListId` of the array index expressions | `b` = byte offset (`u32`) | `offsetof_value` |
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
| casts | `ImplicitCastExpr` (M2), `CStyleCastExpr` (M2), `CXXFunctionalCastExpr` (M2), `CXXStaticCastExpr`, `CXXConstCastExpr`, `CXXReinterpretCastExpr`, `CXXDynamicCastExpr`, `BuiltinBitCastExpr` (`__builtin_bit_cast`) |
| initialization | `InitListExpr` (M2; semantic form, §5.6), `ImplicitValueInitExpr` (M2), `CXXConstructExpr`, `CXXTemporaryObjectExpr`, `CXXInheritedCtorInitExpr`, `CXXDefaultArgExpr` (M2), `CXXDefaultInitExpr`, `CXXScalarValueInitExpr` (M2), `CXXParenListInitExpr` (P0960), `ArrayInitLoopExpr`, `ArrayInitIndexExpr`, `CXXStdInitializerListExpr`, `CompoundLiteralExpr` (GNU; M2), `NoInitExpr` |
| temporaries | `MaterializeTemporaryExpr` (extending decl / `LifetimeExtendedTemporary`), `CXXBindTemporaryExpr`, `ExprWithCleanups`, `OpaqueValueExpr` (M2) |
| memory | `CXXNewExpr` (incl. placement, P2747 constexpr placement new), `CXXDeleteExpr`, `CXXPseudoDestructorExpr` |
| exceptions/RTTI | `CXXThrowExpr` (P3068 constexpr exceptions; M2), `CXXNoexceptExpr`, `CXXTypeidExpr` (M2) |
| traits | `UnaryExprOrTypeTraitExpr` (M2: `sizeof`, `alignof`, `__alignof`), `TypeTraitExpr` (`__is_same`, ..., `__builtin_is_within_lifetime` is a call), `ArrayTypeTraitExpr`, `ExpressionTraitExpr`, `OffsetOfExpr` (M2), `SizeOfPackExpr`, `RequiresExpr`, `ConceptSpecializationExpr` |
| lambdas, packs | `LambdaExpr`, `PackExpansionExpr`, `CXXFoldExpr`, `PackIndexingExpr` (P2662), `CXXUnresolvedConstructExpr` |
| coroutines | `CoawaitExpr`, `DependentCoawaitExpr`, `CoyieldExpr` |
| constant | `ConstantExpr` (M2; caches the value, §9.2) |
| GNU / builtins | `StmtExpr` (M2), `ChooseExpr`, `VAArgExpr`, `AtomicExpr` (`__c11_atomic_*`, `__atomic_*`), `ShuffleVectorExpr`, `ConvertVectorExpr`, `ExtVectorElementExpr` |
| reflection (M6) | `CXXReflectExpr` (`^^`: operand type/decl/namespace/expr → `ReflectionId` when non-dependent), `CXXSpliceExpr` (`[: e :]` in expression position; designated entity once evaluated) |
| recovery | `RecoveryExpr` (children kept; `ERROR` dependence; type if known) |

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
its source expression (or 0), shared by every use.

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
  Designators as written are in a cold side table `designators: ExprId ->
  ListId` for diagnostics and the printer. (clang keeps both forms: ~2× for
  large initializers; §13 Q7.)
- `ParenExpr` is kept (it changes `decltype` and [temp.over.link]).
- No `DesignatedInitExpr`, no `CXXRewrittenBinaryOperator` syntactic form
  beyond its one child (the semantic call), no `TypoExpr` (typo correction
  is resolved before the node is built).

## 6. Statements
```rust
struct StmtNode { kind: StmtKind /*u8*/, flags: u8, aux: u16, loc: Loc, a: u32, b: u32 }  // 16 bytes
```
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
| `LabelStmt`, `GotoStmt`, `IndirectGotoStmt` | `LabelDecl` ids; computed goto reserved (GNU) |
| `AttributedStmt` | attrs `ListId` + sub (`[[likely]]`, `[[fallthrough]]`, `[[assume(e)]]`) |
| `CXXForRangeStmt` | clang's desugaring, all slots kept: `{init, range_var DeclStmt, begin DeclStmt, end DeclStmt, cond, inc, loop_var DeclStmt, body}`; for C++26 lifetime extension of temporaries in the range-init (P2718) the range var's init records extended temporaries |
| `CXXExpansionStmt` (P1306, M6) | `aux` = enumerating / iterating / destructuring; `{init, expansion_var pattern DeclStmt, range (expansion-init-list `ListId` or range expr), body pattern, instantiations: ListId}` where each instantiation is a `CompoundStmt` with the per-element variable; `break`/`continue` target the whole statement |
| `CXXTryStmt` | `{block, handlers: ListId}`; `CXXCatchStmt {exception_decl or catch-all, block}` |
| `CoroutineBodyStmt` | side record with clang's fixed slots: body, promise decl, initial/final suspend, exception handler, fallthrough handler, allocate, deallocate, return-value init, result decl, return stmt on alloc failure, param moves `ListId` |
| `CoreturnStmt` | `{operand, promise call}` |
| `GCCAsmStmt` | side record: template `StringLitId`, outputs/inputs `ListId` of `(constraint StringLitId, ExprId)`, clobbers, labels; volatile/goto bits |
| `ContractAssertStmt` (P2900) | `a` = cond, `aux` = evaluation semantic slot (resolved at codegen) |

## 7. Attributes
`AttrNode { kind: AttrKind /*u16*/, syntax: u8 /*[[ ]], GNU, keyword, declspec*/, flags: u8, loc, a, b }`
(16 B). Arguments: `ExprId`s, a `Symbol`, or a side record. Declarations
with attributes have `HAS_ATTRS` and an entry in `decl_attrs: FxHashMap<
DeclId, ListId>`. Annotations (P3394, M6) are `AttrKind::Annotation` with
`a` = `ConstValueId` (the annotation value, evaluated when not
dependent). Unknown attributes are diagnosed and dropped (not stored).
`AttrKind::ASM_LABEL` (88): `a` = raw `StringLitId` of the label,
flags bit 1 = inherited from the previous declaration.
`AttrKind::BUILTIN` (89): implicit, `a` = builtin id, invalid location.

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
    // DependentFriend(FriendSpecId) friend function template specializations (M3 detail)
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
    Pointer { ty, base: PtrBase, offset: u64 },   // also references; byte offset
    MemberPtr { ty, member: DeclId, kind: MemberKind, derived: bool, path: &'a [DeclId] },
    Reflection { ty, id: ReflectionId },          // std::meta::info (M6)
}
enum PtrBase { Decl(DeclId), Function(DeclId), Temporary(DeclId),
               StringLit { lit: StringLitId, origin: ExprId },
               Const(ConstValueId), TypeInfo(QualType) }
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
  - `Function` bases require offset 0.
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
  | pointer | 16 (+8 if the offset is ≥ 2^32) |
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
  - **Open: pointer identity.** `(void*)&s` and `(void*)&s.x` (same
    type, base and offset) intern as equal. That is right for [expr.eq],
    which compares addresses. [temp.type] treats them as different pointer
    values, and clang keeps them apart through the lvalue path. If a
    conformance test needs that, add a designator-depth disambiguator to
    `Pointer`. Pointers of different types (`&arr` vs `&arr[0]`) are
    already distinct.
  - **Open: bit-fields.** Bit-level padding inside a bit-field storage unit
    (unnamed bit-fields, unused high bits) is not tracked. Those bits must
    be zero.
  - **Open: byte order.** Big-endian targets are unsupported.

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

`PrintOptions { style, max_depth: 10, max_elems: 32 }` are the defaults
(`PrintOptions::DEFAULT`).

**Dump** (`PrintStyle::Dump`, the `value=` fields):
- **Integers:** decimal at any width (`-5`, `18446744073709551615`).
  Characters and enums are numbers too.
- **`bool`:** `true` or `false`.
- **Floats:** the shortest round-trip form in Rust `{:e}` notation, for
  every `FloatKind`: `1.5e0`, `1e-1`, `-0e0`, `inf`, `-inf`, `nan`.
- **Null values of any kind:** `nullptr`.
- **Pointers** print as `&name` plus a designator:
  - The designator is recovered by walking the base's type
    (`decl_type`, or the `Const` value's type) through array elements
    `[i]` (one-past-the-end included) and fields `.f`. Bases add no
    designator.
  - The walk stops when the offset is 0 and the type equals the pointee:
    `&arr` vs `&arr[0]`, `&s.in.y`, `&arr[3]`.
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
  - `Char(enc)` gives `'a'` or `u8'a'`, using the string escapes, with
    `'` escaped.
  - `Enum` gives the enumerator `E::X`, or `(E)7` when no enumerator
    matches.
- **References** print without `&` (`x`).
- **Class values** print as `S{1, 2}`.
- Everything else prints as in the dump.

**String escapes** (`StringPool::write_literal`):
- `\\ \" \n \t \r \0 \a \b \f \v` print as escapes.
- Printable ASCII prints verbatim.
- Every other code unit prints as `\x{hh}` (lowercase hex).

## 10. Reflection and injection (M6; representation fixed now)
- `ReflectionTable` interns `ReflectionId → Reflectee`: `Type(QualType)`,
  `Decl(DeclId)`, `Namespace(DeclId)`, `Template(TemplateNameId)`,
  `Value(ConstValueId)`, `Object(ConstValueId /*pointer*/)`,
  `BaseSpec(DeclId, index)`, `DataMemberSpec(...)`, `Null`.
- `^^T` reflects the type **as written**: a `QualType` that names an alias
  (`Typedef`/`Using` sugar at the top) keeps it; all other sugar is
  canonicalized before interning, so `^^const int == ^^int const` and
  `^^decltype(1) == ^^int`, but `^^Alias != ^^int` ([meta.reflection]
  `dealias`).
- `define_aggregate(r, specs)` completes an incomplete class from a
  constant evaluation: sema (through the bridge) appends `Field` decls
  flagged `INJECTED` to the class with an *injection point* (the
  consteval call's location and the evaluation context) recorded in a cold
  map, then completes the class and computes its layout. This is the one
  exception to "frozen at `}`" (§2.4) and is serialized through sema so
  the Oven never observes a half-completed class.
- Splices (`[: r :]`) are `CXXSpliceExpr` / `Splice` types / `Splice`
  nested-name-specifiers whose designated entity is resolved when the
  operand is not value-dependent.

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
7. **Semantic-only `InitListExpr`** (§5.6). *Recommend* yes; designators
   side table covers diagnostics.
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
    `total`/`instantiation`.
15. **Potentially-constant variables** ([expr.const]/3) are approximated in
    dependence propagation as "const-qualified or `constexpr`" (no
    integral/enumeration type check): over-approximating value dependence
    is safe (it only defers evaluation to instantiation).
