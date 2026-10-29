# spec: kiln_ast types — `TypeContext`, `QualType`, canonical types

Status: **implemented** (WP-042; design WP-040). Normative for the type
layer; sections marked *(M3+)* fix the representation now so later
milestones do not reshape it. Where the implementation differs from the
design text below, §18 records the deviation and is authoritative.
Companion: `spec/ast.md` (declarations, expressions, the dump format).

Standard: [basic.types], [basic.fundamental], [basic.compound],
[basic.type.qualifier], [basic.extended.fp], [conv.rank], [dcl.type],
[dcl.ptr], [dcl.ref], [dcl.mptr], [dcl.array], [dcl.fct], [dcl.spec.auto],
[dcl.type.decltype], [except.spec], [temp.type], [temp.dep.type],
[temp.over.link] (equivalence of dependent types), [meta.reflection]
(`std::meta::info`). clang 19 is the reference for extensions, for type
printing and for which types are distinct.

## 1. Goals and constraints
1. **O(1) language type equality.** `same_type(a, b)` is two array loads and
   one `u32` compare. No structural walk, ever, after construction.
2. **Sugar is kept** for diagnostics, the AST dump and reflection
   (`std::meta::dealias`, `display_string_of`), but costs nothing when not
   written: sugar nodes exist only for what the source actually spelled.
3. **Compact.** A type reference is 4 bytes. A type node is 12 bytes of
   header; larger payloads live in per-kind side arenas (§4). Every arena
   reports its bytes (§9).
4. **Immutable once built.** Types are hash-consed and never mutated;
   what changes (a deferred exception specification, a deduced `auto`) is
   expressed by building a new type and re-pointing the *declaration*.
5. **Safe Rust, plain data.** Nodes are `Copy` structs of `u32`s in `Vec`s:
   no `Rc`, no lifetimes, serializable by copying arrays (§10).

## 2. Ids and `QualType`
```rust
pub struct TypeId(NonZeroU32);   // index into TypeContext::nodes; 0 is reserved (niche)
pub struct QualType(NonZeroU32); // bits 0..=2: Quals; bits 3..=31: TypeId index
bitflags Quals: u8 { CONST = 1, VOLATILE = 2, RESTRICT = 4 }  // the "fast" qualifiers
```
- `QualType` is 4 bytes and `Option<QualType>` is 4 bytes. 29 index bits
  allow 2^29 − 1 types per TU (≈ 6 GB of nodes before the id space runs
  out; exhaustion is a fatal diagnostic, never a panic).
- `RESTRICT` is GNU `__restrict` (glibc headers declare `memcpy` with it).
- All other qualifiers (address spaces, `__ptrauth`, `__unaligned`) are
  *not* bits: they are an `ExtQual` node kind (§5, reserved, M7+), as
  clang's `ExtQuals`. C++26 conformance and libc++ need none of them.
- `QualType::ty()`, `quals()`, `without_quals()` are pure bit operations.
  **Adding** qualifiers is `TypeContext::qualify(q, quals)`, never a bit
  operation, because of the array rule below.

### 2.1 Arrays and qualifiers ([basic.type.qualifier]/3, [dcl.array]/6)
cv-qualifiers applied to an array type apply to its elements. Invariant:
**a `QualType` never carries `Quals` on a node whose canonical type is an
array.** `qualify(q, cv)` on such a type returns a `QualifiedArray` sugar
node (spelled `const A` when `A` is a typedef for `int[3]`) whose canonical
type is the array of `cv`-qualified elements (`const int[3]`, no bits on
the array). `qualify` on a type whose canonical form is a function or
reference type drops the qualifiers ([dcl.fct]/7, [dcl.ref]/1; sema warns).
`TypeContext::unqualified(q)` (clang's `getUnqualifiedArrayType`) strips
cv from arrays' elements as well; `effective_quals(q)` reports the
qualifiers of the element type for arrays.

## 3. Canonical and sugared types
- Every node has a canonical type: `canon: Vec<QualType>` parallel to
  `nodes`. For a canonical node `canon[t] == QualType(t, {})`; for sugar it
  is the fully desugared type, which **may carry qualifiers** (a typedef of
  `const int` has canonical `const int`).
- `canonical(q) = canon[q.ty()].with_bits(q.quals())`: one load and an OR.
  Correct because of §2.1 (a canonical array never receives bits).
- `same_type(a, b) = canonical(a) == canonical(b)`. `same_unqualified`
  compares after clearing bits (and through `unqualified` for arrays).
- `is_canonical(t)` is the `SUGAR` flag being clear (§6). `desugar_once(q)`
  removes one level of sugar (for "aka" and resugaring); `desugar(q)` is
  `canonical`, kept as a name for readability at call sites.
- Kind queries (`as_pointer`, `as_record`, `is_integral`, ...) look at the
  **canonical** node unless the function name says `sugared_`.
- Each canonical node's children are canonical (structural induction), so
  canonical identity is structural identity and hash-consing (§7) makes it
  `u32` identity.

## 4. Storage layout
```rust
struct TypeNode {        // 8 bytes, align 4
    kind: TypeKind,      // u8 (§5)
    flags: TypeFlags,    // u8 (§6)
    aux: u16,            // small per-kind operand
    data: u32,           // inline operand (QualType / DeclId / u32) or side-arena index
}
pub struct TypeContext {
    nodes: Vec<TypeNode>,          // 8 B/type
    canon: Vec<QualType>,          // 4 B/type, hot path of same_type
    intern: InternTable,           // §7, ~5-6 B/interned type
    protos: Vec<FuncProto>,        // 16 B per function type
    type_lists: Vec<QualType>,     // function parameter lists (4 B per param)
    pairs: Vec<[u32; 2]>,          // two-operand payloads (member pointer, elaborated, ...)
    triples: Vec<[u32; 3]>,        // three-operand payloads (subst parm, attributed, ...)
    arrays: Vec<ConstArray>,       // { elem: QualType, size: u64, size_expr: Option<ExprId> } 16 B
    names: TemplateNameTable,      // §8.1
    nns: NnsTable,                 // §8.2
    args: TemplateArgTable,        // §8.3 (interned argument lists)
    profiles: ProfileTable,        // §7.2 (canonical expression fingerprints)
}
```
The **per-node budget is 12 bytes** of header (`TypeNode` + `canon`); with
the interning table, ≈18 bytes for every kind whose payload fits in `aux` +
`data` (builtins, pointers, references, records, enums, typedefs,
incomplete arrays, template type parameters, atomics). Kinds with larger
payloads add 8–16 bytes in a side arena. For comparison, clang 19's
`PointerType` is 32 bytes plus 8 of `FoldingSet` link, `FunctionProtoType`
≥ 48 + 8/param; kiln: 12 + 16 + 4/param (+ interning).

## 5. Type kinds
`k` = how the node is created: **I** interned (hash-consed on its operands),
**D** one per declaration (created with the declaration, never looked up),
**O** one per occurrence (not deduplicated). `c` = canonical-capable (C),
always sugar (S), or canonical exactly when dependent (S/C). Payload is
`aux`/`data`/side record. "dep" marks kinds that only occur in templates.

### 5.1 Builtins (pre-interned at fixed ids, `TypeId::INT` etc.)
Ids 1..=33 are created by `TypeContext::new(&TargetTypes)` in this order and
are stable (serialization, `const` comparisons). 34..=63 are reserved.

| id | kind | prints | id | kind | prints |
|---|---|---|---|---|---|
| 1 | Void | `void` | 18 | Int128 | `__int128` |
| 2 | Bool | `bool` | 19 | UInt128 | `unsigned __int128` |
| 3 | Char | `char` | 20 | Float16 | `_Float16` (= `std::float16_t`) |
| 4 | SChar | `signed char` | 21 | BFloat16 | `__bf16` (= `std::bfloat16_t`) |
| 5 | UChar | `unsigned char` | 22 | Float | `float` |
| 6 | WChar | `wchar_t` | 23 | Double | `double` |
| 7 | Char8 | `char8_t` | 24 | LongDouble | `long double` |
| 8 | Char16 | `char16_t` | 25 | Float128 | `__float128` (target-gated) |
| 9 | Char32 | `char32_t` | 26 | Half | `__fp16` (storage-only, ARM) |
| 10 | Short | `short` | 27 | NullPtr | `std::nullptr_t` |
| 11 | UShort | `unsigned short` | 28 | MetaInfo | `std::meta::info` (*M6*) |
| 12 | Int | `int` | 29 | Dependent | `<dependent type>` (placeholder for type-dependent exprs) |
| 13 | UInt | `unsigned int` | 30 | Overload | `<overloaded function type>` |
| 14 | Long | `long` | 31 | BoundMember | `<bound member function type>` |
| 15 | ULong | `unsigned long` | 32 | BuiltinFn | `<builtin fn type>` |
| 16 | LongLong | `long long` | 33 | Error | `<error type>` (recovery; suppresses cascades) |
| 17 | ULongLong | `unsigned long long` | | | |

Builtins are one `TypeKind::Builtin` with the id as `data`, so "is
builtin" is one compare and "which builtin" is the id. Sizes, alignments,
`char`/`wchar_t` signedness and the `long double` format come from
`TargetTypes` (filled by `kiln_abi` from the target triple; §11), not from
this table. `__builtin_va_list`, `__NSConstantString`, `__int128_t`,
`size_t`-like predefined typedefs are *implicit declarations* made by sema
at TU start, not builtins. `std::float32_t`/`float64_t`/`float128_t` are
not supported by clang 19's C++ mode either; adding them is a new builtin
id each, no representation change.

### 5.2 Structural kinds
| kind | k | c | payload | notes |
|---|---|---|---|---|
| `BitInt` | I | C | aux = signed bit, data = width | `_BitInt(N)`, `unsigned _BitInt(N)` (clang ext in C++) |
| `DependentBitInt` dep | I | C | pairs [signed, ProfileId] | width is value-dependent |
| `Pointer` | I | C | data = pointee QualType | |
| `LValueRef` | I | C | data = referee QualType | reference collapsing ([dcl.ref]/7) is done by the constructor: the referee is never a reference |
| `RValueRef` | I | C | data = referee QualType | |
| `MemberPointer` | I | C | pairs [pointee QualType, class TypeId] | class stays sugared in sugar nodes |
| `ConstantArray` | I | C | data = arrays index | `size_expr` (`int[N]` as written in a template) is sugar: canonical has `None` |
| `IncompleteArray` | I | C | data = element QualType | `int[]` |
| `VariableArray` | O | C | pairs [element, size ExprId] | GNU VLA; never equal to another type (as clang) |
| `DependentSizedArray` dep | I | S/C | pairs [element, size ExprId] / [element, ProfileId] | §7.2 |
| `Function` | I | C | data = protos index | §5.4 |
| `Record` | D | C | data = first `DeclId` of the class | one per class, shared by its redeclarations; a specialization of a class template is its own `Record` |
| `Enum` | D | C | data = first `DeclId` of the enum | |
| `Complex` | I | C | data = element QualType | GNU `_Complex` (*M4*) |
| `Atomic` | I | C | data = value QualType | `_Atomic(T)`; libc++ `<atomic>` uses it via `__c11_atomic_*` (*M4*) |
| `Vector` | I | C | aux = `VectorKind` (Generic, Neon, NeonPoly, Ext), pairs [element, count] | `vector_size`, `ext_vector_type` (libc++ `simd_utils.h`) (*M4*) |
| `DependentVector` dep | I | S/C | aux = kind, pairs [element, size ExprId / ProfileId] | |
| `ExtQual` | I | C | pairs [base TypeId, ext-qual word] | reserved: address spaces etc. (M7+) |

### 5.3 Sugar and dependent kinds
| kind | k | c | payload | notes |
|---|---|---|---|---|
| `Typedef` | D | S | data = typedef/alias `DeclId` | `typedef`, `using X = ...`; canonical = canonical(underlying) |
| `Using` | I | S | pairs [UsingShadow `DeclId`, underlying QualType] | a type named through a using-declaration (*M3*) |
| `Elaborated` | I | S | aux = keyword (none/struct/class/union/enum/typename), pairs [NnsId or 0, named QualType] | created **only** when a keyword or a qualifier was written (§15 R4) |
| `QualifiedArray` | I | S | aux = Quals, data = base TypeId | §2.1 |
| `Paren`, `Decayed`, `Adjusted` | — | — | — | **not represented** (§15 R6): parameter types are stored adjusted; `ParmVar` keeps the written type if it differs |
| `TemplateTypeParm` | D | S | data = `TemplateTypeParm` `DeclId` | the name a template uses; canonical = the canonical parm below |
| `CanonTemplateTypeParm` dep | I | C | aux = depth (15 bits) \| pack bit, data = index | prints `type-parameter-D-I`; makes `template<class T> f(T)` and `template<class U> f(U)` the same type |
| `SubstTemplateTypeParm` | I | S | triples [replacement QualType, associated `DeclId`, index \| pack index] | "this `int` was `T`": diagnostics, warnings' template heuristics (*M3*; see open question in ast.md) |
| `SubstTemplateTypeParmPack` dep | I | C | triples [associated `DeclId`, index, pack `ArgListId`] | an argument pack substituted before its expansion |
| `TemplateSpecialization` | I | S or S/C | triples [TemplateNameId, ArgListId (as written), aliased QualType or 0] | `std::vector<int>` as written; canonical = the specialization's `Record` / the alias target; dependent (`vector<T>`) ⇒ canonical form has canonical name and args |
| `InjectedClassName` dep | D | S | data = class template pattern `DeclId` | `S` inside `template<..> struct S`; canonical = dependent `TemplateSpecialization` of `S` with its parameters as arguments (clang has a canonical node here; we do not need one) |
| `DependentName` dep | I | S/C | aux = keyword, pairs [NnsId, Symbol] | `typename T::type` |
| `DependentTemplateSpecialization` dep | I | S/C | triples [NnsId, Symbol, ArgListId] | `typename T::template X<U>` |
| `PackExpansion` | I | C | pairs [pattern QualType, num_expansions + 1 or 0] | `Ts...` |
| `PackIndexing` | O | S or S/C | triples [pattern, index ExprId, selected QualType or 0] | C++26 `Ts...[I]`; canonical = selected type once the index is known |
| `Decltype` | O | S or S/C | pairs [ExprId, underlying QualType] | canonical = underlying, unless the expression is **instantiation-dependent**: then `DependentDecltype` |
| `DependentDecltype` dep | I | C | data = ProfileId | §7.2 |
| `TypeOf` | O | S | aux = unqual bit \| expr-or-type bit, pairs [ExprId or QualType, underlying] | `__typeof__`, `typeof_unqual` (GNU/C23 ext) |
| `UnaryTransform` | I | S or S/C | aux = transform, pairs [base, result or 0] | `__underlying_type`, `__remove_cvref`, `__decay`, `__make_signed`, ... (clang builtins libc++ 19 uses when `__has_builtin` says so) |
| `Auto` | I | S or C | aux = keyword (auto, decltype(auto), `__auto_type`) \| pack bit, triples [deduced or 0, concept `DeclId` or 0, ArgListId or 0] | undeduced ⇒ canonical (placeholder, flag `UNDEDUCED`), constraint included; deduced ⇒ sugar for the deduced type |
| `DeducedTemplateSpecialization` | I | S or C | pairs [TemplateNameId, deduced or 0] | CTAD placeholder |
| `Attributed` | I | S | aux = attribute kind, pairs [modified, equivalent] | type attributes (nullability, `noderef`, calling conventions); canonical = canonical(equivalent) |
| `Splice` dep | O | S or S/C | pairs [operand ExprId, designated QualType or 0] | P2996 `typename [: r :]` (*M6*); canonical = designated type, or a dependent node keyed by the operand's profile |

Not represented (out of scope, rejected): `FunctionNoProto` (C only),
Objective-C, OpenCL pipes/samplers, matrices, HLSL, `BTFTagAttributed`,
`MacroQualified`, `CountAttributed`, SVE sizeless types (added as builtins
if `arm_sve.h` support is ever required).

### 5.4 Function types ([dcl.fct], [except.spec])
```rust
struct FuncProto {      // 16 bytes
    ret: QualType,
    params: u32,        // start in type_lists (adjusted parameter types, [dcl.fct]/5)
    n_params: u16,
    info: FuncInfo,     // u16: variadic | ref_qual(2: none,&,&&) | this_quals(3: cvr)
                        //      | cc(4: default, preserve_most, vectorcall, ...) | noreturn | trailing_return
    exc: ExcSpec,       // u32: 3-bit tag + 29-bit operand
}
enum ExcSpec /* tag */ {
    None,                    // potentially throwing (no spec, noexcept(false), throw(T...) pre-17 canon)
    Nothrow,                 // canonical non-throwing
    DynamicNone,             // `throw()` as written   (sugar; canonical Nothrow from C++17, None before)
    BasicNoexcept,           // `noexcept` as written  (sugar; canonical Nothrow)
    NoexceptExpr(ExprId),    // `noexcept(e)`: sugar resolving to Nothrow/None, or dependent
    Dynamic(type list),      // `throw(A, B)` (C++98..14 only; not part of the type)
    DependentNoexcept(ProfileId), // canonical form of a value-dependent noexcept(e)
    Deferred(DeclId),        // Unevaluated (implicit/defaulted members) or Uninstantiated
                             // (specializations, [temp.inst]/14), told apart by the decl
}
```
- Canonicalization: params and return canonical; `trailing_return` cleared;
  `exc` canonical: from C++17 `Nothrow`/`None`/`DependentNoexcept`/
  `Deferred`; before C++17 the exception specification is not part of the
  type and the canonical form is `None` (clang's behavior).
- `Deferred(D)` types are canonical and distinct per declaration. Sema
  resolves them (`resolve_exception_spec(D)`) before any comparison that
  depends on the specification and then re-points `D` and its
  redeclarations at a new, resolved type. Types are never patched in place
  (clang's `updateExceptionSpec` mutates; we cannot and need not).
- Calling conventions other than the target default only arise through
  attributes (`Attributed` sugar over a `Function` whose `cc` differs).

## 6. Flags
`TypeFlags` (u8), computed at construction as the union over operands plus
the node's own contribution:

| bit | meaning |
|---|---|
| `DEPENDENT` | dependent type [temp.dep.type] |
| `INSTANTIATION_DEPENDENT` | mentions a template parameter somewhere (incl. inside a non-dependent `decltype`) |
| `VARIABLY_MODIFIED` | contains a VLA |
| `UNEXPANDED_PACK` | contains an unexpanded parameter pack |
| `CONTAINS_ERROR` | built from an `Error` type or a recovery expression |
| `UNDEDUCED` | contains an undeduced placeholder (`auto`, CTAD) |
| `SUGAR` | `canon[t] != t` |
| (reserved) | |

Canonical and sugar nodes for the same type have the same flags except
`SUGAR` and, for `Decltype`/`TypeOf`, `INSTANTIATION_DEPENDENT`.

## 7. Interning (hash-consing)
### 7.1 The table
Construction functions (`pointer_to`, `lvalue_ref_to`, `array_of`,
`function`, `elaborated`, ...) compute a structural key `(kind, aux, data,
side-record words)` over operands that are already ids, hash it (FxHash,
as `kiln_lex::Interner`), and probe `intern`. On a miss they push the node
(and side record), compute its canonical type by building the canonical
node from the canonical operands (which recursively interns), and insert.
A hit returns the existing id. Because operands are ids, keys are a few
words and hashing is O(1) per node; building a type of depth d costs d
probes in the worst case and 1 in the common case.

`InternTable` stores only `TypeId`s; keys are re-read from the arenas on
probe, so no key is stored twice. The natural implementation is
`hashbrown::HashTable<TypeId>` (≈ 5 B per entry at 7/8 load); the
std-only fallback is `HashMap<u64 /*hash*/, TypeId>` with collisions
chained through a `next_same_hash: Vec<TypeId>` (≈ 18–22 B per entry).
See open question T1.

Kinds marked **D** and **O** in §5 never go through the table: the
declaration (or the occurrence) creates the node and keeps its id.

### 7.2 Dependent types and expressions ([temp.over.link])
Redeclaration matching of templates compares canonical types, so the
canonical form of a dependent type must identify *equivalent* types:
- template type parameters canonicalize to `CanonTemplateTypeParm(depth,
  index, pack)`; names and declarations are sugar;
- `DependentName`/`DependentTemplateSpecialization` canonicalize their
  nested-name-specifier (§8.2) and keep the `Symbol`; the `typename`
  keyword is sugar;
- dependent template specializations use the canonical `TemplateName`
  and the canonical argument list (§8);
- types containing an expression (`DependentSizedArray`, `DependentVector`,
  `DependentBitInt`, `DependentDecltype`, `noexcept(expr)`, `Splice`, and
  `PackIndexing` with a dependent index) canonicalize the expression to a
  **`ProfileId`**: the interned sequence of `u32` words produced by
  `profile_expr(&Ast, e)` (a pre-order walk emitting node kind, operator,
  canonical types of type operands, first `DeclId`s of non-dependent
  referenced declarations, `(depth, index)` of template parameters,
  literal values and unresolved names; `ImplicitCast` and `ConstantExpr`
  wrappers are skipped, parentheses are kept because [temp.over.link]/5
  is ODR-, i.e. token-based). Two expressions are equivalent iff their
  profiles are equal; functionally-equivalent-but-not-equivalent
  expressions are ill-formed NDR and are not diagnosed (as clang).
  The canonical node keeps the first expression seen for printing.
- After substitution in an instantiation, the same constructors run on
  the substituted operands and produce non-dependent canonical types; no
  separate "dependent to concrete" mapping exists.

## 8. Names inside types
### 8.1 `TemplateNameId(u32)`
Interned: `Template(DeclId)` (canonical: the first decl of the template),
`Qualified(NnsId, TemplateNameId, template_kw)` sugar,
`Dependent(NnsId, Symbol | OverloadedOperator)`,
`TemplateTemplateParm` (sugar = decl; canonical = `(depth, index, pack)`),
`SubstTemplateTemplateParm(param, replacement)` sugar,
`SubstTemplateTemplateParmPack(param, ArgListId)`, `Using(UsingShadow,
TemplateNameId)` sugar, `Overloaded(Vec<DeclId>)` (an unresolved set),
`Splice(ExprId)` (*M6*). Each entry stores its canonical name, so
`canonical_template_name` is O(1).

### 8.2 `NnsId(u32)` — nested-name-specifiers
Interned prefix chains: `Global` (`::`), `Namespace(prefix, DeclId)`,
`NamespaceAlias(prefix, DeclId)` (sugar), `Type(prefix, QualType)` (`T::`,
`vector<int>::`; the `template` keyword is a bit), `Identifier(prefix,
Symbol)` (dependent `T::x::`), `Splice(ExprId)` (*M6* `[: r :]::`).
Canonical: aliases resolved, types canonical, redundant prefixes of
non-dependent entities dropped (the entity determines the scope).

### 8.3 Template arguments and `ArgListId`
```rust
pub struct TemplateArg { kind: ArgKind /*u8*/, a: u32, b: u32 }  // 12 bytes
enum ArgKind {
    Type,            // a = QualType
    Value,           // a = parameter type QualType, b = ConstValueId  (all non-type args once evaluated:
                     //    integral, enum, pointer/reference to entity, nullptr, member pointer,
                     //    floating, class-type structural value, std::meta::info)
    Template,        // a = TemplateNameId
    TemplateExpansion, // a = TemplateNameId, b = num_expansions + 1 or 0
    Expr,            // a = ExprId: value-dependent or not-yet-converted argument (sugar/dependent)
    Pack,            // a = ArgListId (an expanded pack)
    Null,            // not yet deduced (deduction scratch only; never stored in a canonical list)
}
pub struct ArgListId(NonZeroU32);   // interned list of TemplateArg
```
- Clang's separate Integral/Declaration/NullPtr/StructuralValue kinds
  collapse into `Value` because `ConstValueId`s are interned (spec/ast.md
  §9): equal values have equal ids, so canonical argument equality is
  word equality.
- A canonical list has canonical types, canonical template names, `Value`
  instead of non-dependent `Expr`, and dependent `Expr`s replaced by
  `Expr` of the canonical *profile* (encoded as `a = ProfileId`, `b = 1`).
  `canonical_args(ArgListId)` is cached per list (side vector), so it is
  O(1) after the first call.
- The specialization key of every template is `(first DeclId of the
  template, canonical ArgListId)`: an 8-byte key into one global map
  (spec/ast.md §8).

## 9. Memory accounting
`TypeContext::memory() -> MemReport` with one row per arena: element
count, `len * size_of`, and `capacity * size_of` (what is actually held),
plus the intern table's allocation. `MemReport` is the type shared by all
of kiln_ast (spec/ast.md §2.3). The bench records `types.bytes` and
`types.count` per TU; regressions are tracked like the Oven counters.

## 10. Serialization (sketch, *M4* modules/PCH)
All arenas are vectors of fixed-size little-endian `u32` words; a TU's
types serialize by writing them with a header of counts. Ids are
TU-local: importing a module either copies its types into the importer's
context through the constructors (re-interning; correct by construction)
or, later, maps id ranges (clang's per-module base offsets). Builtin ids
1..=63 are identical in every context, which makes most remaps trivial.
`Symbol`s and `DeclId`s inside payloads are remapped by the same pass.

## 11. Layout hooks
Layout is target knowledge and belongs to `kiln_abi`, which depends on
`kiln_ast`; the AST only stores results.
```rust
pub struct TargetTypes {   // filled by kiln_abi::target(triple); stored in TypeContext
    pub size_align: [(u16 /*bytes*/, u16 /*align*/); 34], // per builtin id 1..=33 (0 unused)
    pub char_signed: bool, pub wchar_signed: bool, pub long_double: FloatFormat,
    pub size_t: TypeId, pub ptrdiff_t: TypeId, pub intptr_t: TypeId, pub pointer_bytes: u8,
}
pub struct RecordLayout {             // Itanium [class] layout, all offsets from the object start
    pub size: u64, pub align: u32, pub preferred_align: u32,
    pub data_size: u64,               // dsize: tail padding reusable by derived classes
    pub nv_size: u64, pub nv_align: u32,
    pub fields: Box<[u64]>,           // bit offset per FieldDecl, in declaration order
    pub bases: Box<[(DeclId, u64)]>,  // direct non-virtual bases, byte offsets
    pub vbases: Box<[(DeclId, u64)]>, // all virtual bases (complete-object offsets)
    pub vptr: Option<u64>, pub primary_base: Option<(DeclId, bool /*virtual*/)>,
}
pub struct LayoutCache { /* FxHashMap<DeclId /*first decl*/, Box<RecordLayout>> */ }
```
- `Ast::layouts` is a write-once cache: sema calls
  `kiln_abi::record_layout(&Ast, record) -> RecordLayout` the first time a
  complete, non-dependent class needs `sizeof`, a field offset, codegen
  or the Oven, then `ast.layouts.insert(record, layout)`. Readers
  (`SemaBridge::layout`, codegen) never compute. Inserting twice is a
  `debug_assert!` failure.
- `size_align(q)` for non-record types is computed (not cached) by kiln_abi
  from `TargetTypes` (arrays multiply, references are pointers, member
  pointers are 1 or 2 pointer words, etc.).
- kiln_sema therefore depends on kiln_abi (open question T4).

## 12. Printing
`TypePrinter` renders a `QualType` as clang 19's `QualType::getAsString`
does with the C++ `PrintingPolicy`, because diagnostics tests compare text
with clang's. Options (`PrintOptions`): `canonical` (print `canonical(q)`),
`desugar` (print `desugar_top(q)`, the AST dump's second string),
`full_scope` (qualify every name; default for canonical, off for sugar,
which prints as written), `suppress_tag` (omit written `struct`/`enum`
keywords). Presets: `SUGARED`, `CANONICAL` (canonical + full scope),
`DESUGARED`. Declaration names, expressions and constant values come from
the caller through the `TypeNames` trait (implemented by `Ast`); `RawNames`
prints placeholders for AST-less use. Entry points: `print_type`,
`write_type`, `aka` (the ` (aka 'U')` string or `None`) and `dump_strings`
(the `'T'`/`'T':'U'` pair).

Algorithm (clang's `TypePrinter`, declarator inside-out): `print(q, inner)`
where `inner` is the declarator text so far (empty at the top):
- leaf kinds print `quals name` then `" " + inner` if `inner` is non-empty;
  qualifiers print in the order `const volatile __restrict`, **before** a
  leaf (`const int`) and **after** a `*`/`&` (`int *const`);
- `Pointer(p)`: `print(p, "*" + quals + inner)`, wrapping `( )` around the
  new inner when `p` is an array or function (`int (*)[3]`, `int (*)(int)`);
  a space separates the pointee from `*` (`int *`, `char **`);
- `LValueRef`/`RValueRef`: same with `&`/`&&`; `MemberPointer`: `C::*`;
- arrays: `print(elem, inner + "[N]")` (`int[3]`, `int[]`, `int[N]` when
  the size expression is printed, `char *[3]`);
- functions: `print(ret, inner + "(params)" + suffix)` where params are
  printed with `, ` (variadic adds `, ...`; no params is `()`), and suffix
  is ` const`, ` volatile`, ` &`/` &&`, then ` noexcept`, ` noexcept(expr)`,
  ` throw()`, ` throw(A)` as spelled; trailing return types print
  `auto (params) -> R`;
- names: records/enums/typedefs print their (optionally scoped) name, with
  template arguments `<int, 2>` (no space before `>>`; values printed per
  spec/ast.md §9.3); unnamed classes print `(unnamed struct at F:L:C)`,
  anonymous `(anonymous struct at F:L:C)`, closures `(lambda at F:L:C)`,
  the anonymous namespace `(anonymous namespace)`, where `F` is the file
  name as the driver names it in diagnostics;
- `CanonTemplateTypeParm` prints `type-parameter-D-I`; sugar parameters
  print their name; `PackExpansion` appends `...`; `DependentName` prints
  `typename T::type` (keyword only if written); `Decltype` prints
  `decltype(expr)` with the expression printer; `Auto` prints `auto` /
  `decltype(auto)` / `C auto` when undeduced and the deduced type when
  deduced; `Atomic` `_Atomic(T)`; `BitInt` `_BitInt(N)`; `Vector`
  `__attribute__((__vector_size__(N * sizeof(T)))) T` and ext vectors
  `T __attribute__((ext_vector_type(N)))`; `Elaborated` prints its keyword
  and qualifier as written; `QualifiedArray` prints `const A`.

Diagnostics print `'T'` and add ` (aka 'U')` when `U`, the printed
canonical type (`full_scope`), differs textually from `T` (clang's
`desugarForDiagnostic`; deviations, if any, are recorded in spec/diag.md
§11). Representative goldens (each a unit test, checked against clang 19):

| type | prints |
|---|---|
| pointer to const int | `const int *` |
| const pointer to int | `int *const` |
| pointer to array of 3 int | `int (*)[3]` |
| array of 3 pointer to char | `char *[3]` |
| pointer to function (int) returning int | `int (*)(int)` |
| function returning pointer to function | `void (*(int))(char)` |
| pointer to that | `void (*(*)(int))(char)` |
| reference to array | `int (&)[3]` |
| member function pointer | `void (S::*)(int) const &` |
| member data pointer | `int S::*` |
| noexcept function | `void () noexcept` |
| variadic | `int (const char *, ...)` |
| typedef to const array | `const Arr` (canonical `const int[3]`) |
| `std::nullptr_t` | `std::nullptr_t` |
| `_BitInt(13)` unsigned | `unsigned _BitInt(13)` |

## 13. Interface sketch
```rust
impl TypeContext {
    pub fn new(target: TargetTypes) -> Self;
    // queries (O(1))
    pub fn canonical(&self, q: QualType) -> QualType;
    pub fn same_type(&self, a: QualType, b: QualType) -> bool;
    pub fn kind(&self, t: TypeId) -> TypeKind;          // sugared node's own kind
    pub fn canonical_kind(&self, q: QualType) -> TypeKind;
    pub fn flags(&self, t: TypeId) -> TypeFlags;
    pub fn desugar_once(&self, q: QualType) -> QualType;
    pub fn pointee(&self, q: QualType) -> Option<QualType>;  // pointers, references, member ptrs (canonical)
    pub fn element(&self, q: QualType) -> Option<QualType>; // arrays, vectors, complex
    pub fn proto(&self, q: QualType) -> Option<&FuncProto>;  pub fn params(&self, &FuncProto) -> &[QualType];
    pub fn as_record(&self, q: QualType) -> Option<DeclId>; pub fn as_enum(&self, q: QualType) -> Option<DeclId>;
    // construction (interning)
    pub fn qualify(&mut self, q: QualType, cv: Quals) -> QualType;
    pub fn unqualified(&mut self, q: QualType) -> QualType;
    pub fn pointer_to(&mut self, q: QualType) -> QualType;
    pub fn lvalue_ref_to(&mut self, q: QualType) -> QualType;  // collapses
    pub fn rvalue_ref_to(&mut self, q: QualType) -> QualType;  // collapses
    pub fn member_pointer(&mut self, pointee: QualType, class: TypeId) -> QualType;
    pub fn constant_array(&mut self, elem: QualType, n: u64, size_expr: Option<ExprId>) -> QualType;
    pub fn incomplete_array(&mut self, elem: QualType) -> QualType;
    pub fn function(&mut self, ret: QualType, params: &[QualType], info: FuncInfo, exc: ExcSpec) -> QualType;
    pub fn record_type(&mut self, first_decl: DeclId) -> TypeId;   // called once per class by sema
    pub fn enum_type(&mut self, first_decl: DeclId) -> TypeId;
    pub fn typedef_type(&mut self, decl: DeclId, underlying: QualType) -> TypeId;
    pub fn elaborated(&mut self, kw: TagKeyword, nns: Option<NnsId>, named: QualType) -> QualType;
    // ... one constructor per kind in §5; dependent constructors take ProfileIds from Ast
    pub fn memory(&self) -> MemReport;
}
pub fn print_type(ctx: &TypeContext, names: &dyn TypeNames, q: QualType, opts: PrintOptions) -> String;
```
(The implemented signatures are in rustdoc; §18 lists where they differ.)
`TypeContext` lives in `Ast` (spec/ast.md §2) because printing and
profiling need declarations and expressions; the type layer itself only
stores ids.

## 14. Tests (`crates/kiln_ast/tests/types_*.rs`)
- Interning: constructing the same type twice yields the same id; sugar
  vs canonical ids; reference collapsing table ([dcl.ref]/7, all 4 cases);
  array-qualifier rule (§2.1) incl. multi-dimensional arrays.
- Canonical equality matrix over builtins, pointers, arrays, functions
  (param adjustment is sema's, but canonical params are tested), noexcept
  in C++14 vs C++17 ([except.spec]/13), `throw()` vs `noexcept`.
- Dependent equivalence ([temp.over.link]): `T` vs `U` at the same
  position, `typename T::type`, `T[N]` vs `T[M]` (different), `decltype`
  of equivalent expressions, `vector<T>` vs `vector<U>`.
- Flags propagation for each kind.
- Printing: the §12 table plus one row per kind, each verified against
  clang 19 (`-ast-dump` type strings).
- Memory: `size_of::<TypeNode>() == 8`, `size_of::<QualType>() == 4`,
  `size_of::<Option<QualType>>() == 4`, `size_of::<FuncProto>() == 16`,
  `size_of::<TemplateArg>() == 12` (static assertions).

## 15. Rejected alternatives
- **R1 `Vec<TypeKind>` with a Rust `enum` per kind.** Idiomatic, but the
  enum is as large as its largest variant (a function prototype or a
  template specialization: 24–32 bytes + tag) for *every* type, including
  the ~60% that are pointers, references and records. The tagged 8-byte
  node + side arenas costs one extra indexed load for large kinds only.
  Accessors return a typed view enum (`TypeView::Pointer(QualType)`, ...)
  so call sites still `match`.
- **R2 8-byte `QualType` (`u32` id + `u32` qualifiers, or a 64-bit word).**
  Room for address spaces inline, but doubles every type reference in
  every expression node (spec/ast.md §5: one `QualType` per expression)
  and in every side record. Three bits cover all of C++; the rare
  extended qualifiers become nodes (R2 is clang's `ExtQuals` trade-off,
  kept).
- **R3 Canonical-only types (no sugar).** Halves type count and makes
  equality trivially `==` on the stored id. Rejected: diagnostics must
  say `std::string`, not `std::basic_string<char, ...>`; reflection must
  distinguish `^^Alias` from `^^int` ([meta.reflection] `dealias`);
  the AST dump must match clang's `'Arr':'int[3]'` form. Our cost is the
  `canon` vector (4 B/type) and one extra load on comparison.
- **R4 Always-Elaborated sugar (clang 16+).** clang wraps every type name
  in `ElaboratedType`, even unqualified `S`; this cost measurable memory
  (D112374 review: +several % on large TUs) to simplify its printer.
  kiln creates `Elaborated` only for a written keyword or qualifier; the
  printer's `full_scope` rule (§12) yields the same strings. The dump
  prints the written form, which in the M2 subset coincides with clang's.
  Known exception (found by WP-062's namespace goldens, not yet fixed):
  for an enumeration declared in a namespace or class, an enumerator's
  type, and a written qualified type `N::E`, print `'E':'N::E'` in kiln
  but `'N::E'` in clang (kiln makes no `Elaborated` node carrying the
  qualifier there).
- **R5 A thread-safe `&self` interner (sharded locks, as
  `kiln_lex::Interner`).** Sema is single-threaded per TU (spec/ast.md
  §13 Q8); parallelism is across TUs. A lock per type construction costs
  ~20 ns on the hottest path of template instantiation. `TypeContext`
  takes `&mut self` for construction and is `Send` but not shared.
- **R6 `TypeLoc` (clang's per-occurrence source-location tree), `Paren`,
  `Decayed`, `Adjusted` nodes.** TypeLoc roughly doubles type-related
  memory for declarations and exists for tooling (rewriters, IDE ranges)
  that kiln does not have. Diagnostics point at the declarator's
  location stored in the declaration. Parameter types are stored
  adjusted ([dcl.fct]/5); `ParmVarDecl` keeps its written type in its own
  `ty` slot. Revisit only if a conformance diagnostic needs a sub-type
  range (open question T5).
- **R7 Structural comparison without hash-consing.** Saves the intern
  table (~6 B/type) but makes `same_type` O(size), which is the inner loop
  of overload resolution and template argument deduction; and canonical
  ids would no longer be usable as hash keys (specialization map, §8.3).

## 16. Open questions (with recommendation)
- **T1 Dependency on `hashbrown`** for `HashTable<TypeId>` (keys stored
  only in arenas, §7.1). *Recommend yes*: pure safe API, already a
  transitive dependency of std; saves ~14 B/type over the std fallback,
  which stays implementable behind the same `InternTable` type if the
  dependency is refused. The same table type is reused for `ConstStore`
  and `ProfileTable`.
- **T2 `Symbol` in kiln_ast.** Dependent names and `DeclName`s store
  `kiln_lex::Symbol`. *Recommend* kiln_ast depends on kiln_lex (the
  interner is a leaf module), or move `Interner` into kiln_source; either
  is fine, the former is no code motion.
- **T3 `SubstTemplateTypeParm` sugar** costs one node per (parameter,
  argument, instantiation) and inflates heavily-instantiating TUs (the
  mirrorbind case). *Recommend* keep it (diagnostics like clang's
  "with T = int" and the dump need it), interned so repeats are free, and
  measure on the M7 corpus; a driver flag `-fno-subst-sugar` may drop it
  (types then print as the replacement only).
- **T4 kiln_sema → kiln_abi dependency** for record layout (§11).
  *Recommend yes*; the alternative (layout in kiln_ast) puts target
  knowledge in the AST crate.
- **T5 TypeLoc deferral** (R6). *Recommend* defer indefinitely; spec the
  extension point as a side table keyed by declaration, never a change to
  `QualType`.
- **T6 `InjectedClassName` without a canonical node** (§5.3 deviation
  from clang). clang keeps a canonical `InjectedClassNameType` and
  compares it specially with the dependent specialization. *Recommend*
  canonicalizing to the dependent `TemplateSpecialization`, which makes
  `S` and `S<T>` inside the template the same type by construction.

## 17. Supervisor decisions (2026-10-28, binding)
- T1 **approved**: `hashbrown` (safe API only; `HashTable`) is an allowed
  dependency for kiln_ast; keep the std fallback compiling behind the same
  `InternTable` type.
- T2 **decided differently**: `Symbol`/`Interner` move to `kiln_source`
  (`kiln_source::symbol`), re-exported from `kiln_lex` for compatibility.
  kiln_ast must not depend on kiln_lex (layering: the AST is consumed by
  the Oven and codegen, which have no business linking a lexer).
- T3 approved (keep subst sugar, interned, measure in M7, `-fno-subst-sugar`).
- T4 approved. T5 approved (defer). T6 approved (canonicalize to the
  dependent specialization; keep an `InjectedClassName` *sugar* node so
  printing says `S`).

## 18. Implementation notes and deviations (WP-042)
Authoritative where it disagrees with §§2–14.

Representation and encodings:
- `Typedef`, `Using` and `InjectedClassName` payloads are `pairs`
  `[decl, underlying]` / `[pattern, injected]`. `DependentBitInt` stores
  only the width profile. `Subst` aux = `pack_index + 1` (0 = none;
  indices ≥ 65535 are not recorded). `TemplateSpecialization` aux bit 0 =
  alias. `CanonTemplateTypeParm` aux = `depth | pack << 15`. `Auto` aux =
  `keyword | pack << 2`.
- `AUX_PROFILE` (aux bit `0x100`) marks the profile-keyed canonical form of
  `DependentSizedArray`, `DependentVector`, `PackIndexing`, `TypeOf` and
  `Splice`: the sugar node keys on the written expression, the canonical
  node on the `ProfileId`.
- Parameter counts are `u16` (65535 parameters); the `noexcept(expr)`
  operand and type ids are 29-bit; on any overflow the context sets
  `exhausted()` and returns `QualType::ERROR` (or sentinel id 1 for
  profiles, argument lists, nns and template names). No panics on
  user-derivable input.
- `ExtQual` takes a `QualType` base. `Record`/`Enum` have
  `record_type_dependent` / `enum_type_dependent` variants (members of the
  current instantiation).
- Constructors of declaration types (`record_type`, `enum_type`,
  `typedef_type`, `using_type`, `template_type_parm`,
  `canon_template_type_parm`, `injected_class_name`) return `TypeId`;
  every other constructor returns `QualType` (`From<TypeId> for QualType`).
- `TargetTypes` is a plain struct filled by `kiln_abi`; its `Default` is
  an unknown target (all sizes 0).
- `hashbrown` is a plain dependency of kiln_ast (the const store and
  string pool use it too); the `std-intern` feature selects the std
  fallback type `InternTable` instead (same API; built and tested with
  `--features std-intern`).

Canonical forms:
- The canonical keyword of `DependentName`/`DependentTemplateSpecialization`
  is `typename` (`None` → `Typename`), so `T::x` and `typename T::x` are the
  same type.
- `Decltype` with a profile canonicalizes to `DependentDecltype(profile)`.
- `VariableArray` nodes are never interned (one node per occurrence, as
  clang).
- `TemplateName::Overloaded` sets are not deduplicated.
- `Nns::Splice` is canonical as written.
- Template arguments: `ArgKind::Profile` is the canonical form of a
  dependent expression argument; `Expr` arguments store their profile in
  the second payload word. `TemplateName::Qualified` holds the underlying
  `TemplateNameId`; `Splice` / `SpliceProfile` names were added (P2996).

Printing:
- Defaulted template arguments are not tracked by the type layer:
  `TypeNames::specialization_args` must return the argument list with
  defaulted arguments already removed.
- Nesting deeper than 400 prints `...` (balanced) instead of recursing.
- `ExtQual` prints `__attribute__((address_space(N))) T` (approximate: the
  ext-qual word is printed raw).
- kiln creates no `Elaborated` node for a name written without keyword or
  qualifier (R4), so a written `S` is a bare `Record`: `dump_strings`
  omits the second string when it would repeat the first, where clang
  prints `'S':'S'`. The `-fdump-ast` normalizer (WP-046) treats `'X':'X'`
  as `'X'`.
- clang 19 prints a bare `RecordType` (what desugaring leaves) scoped:
  `DESUGARED` scopes the top-level record/enum, giving clang's
  `'S':'ns::S'`.

Measured cost (live bytes per new type including the intern table's
share, `types_mem` test, x86-64, both table implementations): record 16,
pointer 16, typedef 24, constant array 32, function with 2 parameters 40,
dependent sized array (with its profile) 76, `W<int, R>` (with its
argument list) 72.

## 19. Dump second-string rule (2026-10-28)
In AST dumps (`spec/ast-dump.md` §2) the second string of `'X':'Y'` is
clang's **top-level-only desugar** of the written type (`desugar_top`), not
the canonical type, and is omitted when equal. `print_type` with
`PrintOptions::CANONICAL` remains the canonical printer for diagnostics.
