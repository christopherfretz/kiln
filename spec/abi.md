# spec: kiln_abi — targets, type and record layout

Status: **implemented through WP-078b**: Itanium layout of classes with
non-virtual and virtual bases (empty-base optimization, tail-padding
reuse, `[[no_unique_address]]`, `#pragma pack`, `packed`/`aligned`, the
vptr, primary bases), the base-subobject table, the
`-fdump-record-layouts` dump of all of them (§8), and Itanium vtable
groups, construction vtables and VTTs with their
`-fdump-vtable-layouts` dump (§8.5).

Standard: [basic.fundamental], [basic.compound], [basic.align],
[intro.object]/9, [expr.sizeof], [expr.alignof], [class.mem]/26–29,
[class.bit], [class.union], [class.derived], [class.mi], [dcl.align],
[dcl.attr.nouniqueaddr],
[dcl.array], [dcl.enum]/7–8, [dcl.mptr].
ABI: Itanium C++ ABI §1.1 (definitions: *POD for the purpose of layout*,
*dsize*, *nvsize*, *nvalign*, *empty class*), §2.2 (POD data types),
§2.3 (member pointers), §2.4 (non-POD class layout); System V AMD64
psABI §3.1.2; AAPCS64 §5 and §10.1 (bit-fields).
Reference implementation: clang 19 (`ASTContext::getTypeInfo`,
`ItaniumRecordLayoutBuilder`, `EmptySubobjectMap`, `DumpRecordLayout`,
`ItaniumVTableBuilder`, `VTTBuilder`).
Where clang deviates from the ABI documents, **clang wins**: kiln must
produce objects that link with clang-compiled code.

## 1. Scope and interface
```rust
// target
pub enum Arch { X86_64, AArch64 }
pub struct Triple { pub arch: Arch }           // parse(), normalized(), Display
pub fn target(triple: &str) -> Result<Target, TripleError>;
pub struct Target { pub triple, pub types: TargetTypes, /* §2 facts */ }
impl Target { fn for_arch(Arch); fn builtin_size_align(BuiltinType) -> (u64, u64);
              fn bit_int_size_align_bits(u32) -> (u64, u64) }
// layout (sizes in bytes; RecordLayout per spec/types.md §11)
pub struct TypeLayout { pub size: u64, pub align: u64 }
pub fn type_size_align(&Ast, QualType, &Target) -> TypeLayout;
pub fn array_layout(&Ast, elem: QualType, count: u64, &Target) -> TypeLayout;
pub fn record_layout(&Ast, record: DeclId, &Target) -> RecordLayout;
pub fn ensure_layout<'a>(&'a mut Ast, DeclId, &Target) -> &'a RecordLayout;
pub struct Layouter<'a>   // a session: memoizes records, answers is_pod / is_empty
pub fn subobjects(&Ast, record: DeclId, &Target) -> Vec<(Subobject, u64)>; // §8.3
pub const MAX_SUBOBJECTS: usize;              // 65536 (SubobjectId is u16)
impl Layouter { fn subobjects(&mut self, DeclId) -> Vec<(Subobject, u64)> }
// dump (§6, §8.4)
pub struct DumpOptions<'a> { pub sources: Option<&'a SourceManager>, pub at_definition: bool }
pub fn dump_record_layout(&Ast, DeclId, &Target) -> String;
pub fn write_record_layout(&mut Layouter, DeclId, &mut String);
pub fn write_record_layout_with(&mut Layouter, DeclId, &DumpOptions, &mut String);
pub fn record_definitions(&Ast) -> Vec<DeclId>;             // -complete order
pub fn dump_all_record_layouts(&Ast, &Target, Option<&SourceManager>) -> String;
// vtable (§8.5); kiln_ast::class::{VTableLayout, VTableComponent, Thunk,
// VttComponent, Subobject, SubobjectId} re-exported
pub enum VTableError { NotDynamic, Incomplete, TooComplex }
pub fn vtable_layout(&Ast, record: DeclId, &Target) -> Result<VTableLayout, VTableError>;
pub fn ensure_vtable<'a>(&'a mut Ast, DeclId, &Target) -> Result<&'a VTableLayout, VTableError>;
pub fn construction_vtable(&Ast, most_derived: DeclId, base: SubobjectId, &Target)
    -> Result<VTableLayout, VTableError>;
pub fn vtt_vtables(&Ast, record: DeclId, &Target) -> Result<Vec<SubobjectId>, VTableError>;
pub fn vtt(&Ast, record: DeclId, &Target) -> Result<Vec<VttComponent>, VTableError>;
pub fn dump_vtable_layout(&Ast, DeclId, &Target) -> Result<String, VTableError>;
pub fn dump_construction_vtable(&Ast, most_derived: DeclId, base: SubobjectId, &Target)
    -> Result<String, VTableError>;
pub fn dump_vtable_group(&Ast, DeclId, &Target) -> Result<String, VTableError>;
pub fn dump_all_vtable_layouts(&Ast, &Target) -> String;
// dump_blocks (clang differentials: kiln_driver tests, `expect: layout|vtables`)
pub const RECORD_LAYOUT_MARKER: &str;       // "\n*** Dumping AST Record Layout\n"
pub const CLANG_BUILTIN_RECORDS: [&str; 3]; pub const VTABLE_BLOCK_HEADS: [&str; 5];
pub fn record_layout_blocks(&str) -> Vec<&str>;          // ordered, clang builtins dropped
pub fn vtable_blocks(&str) -> BTreeSet<String>;          // a set, `Original map` dropped
```
- **Total.** No input panics. Incomplete, dependent, invalid or
  recursive (`struct S { S s; }`, rejected by sema) types get the
  degenerate layout (size 0, align 1). Arithmetic is in bits and
  saturating.
- **No recursion on the native stack.** Record dependencies are resolved
  by an explicit post-order stack, the empty-subobject walks and the
  dump use explicit stacks too, and the type printer is depth-capped.
  Arbitrarily nested input is safe.
- **Memoized.** A `Layouter` computes each class once, keyed by its first
  declaration. `ensure_layout` writes every class it computed into
  `Ast::layouts` (write-once, spec/types.md §11). Classes already there
  are reused, not recomputed.
- **Pure.** The result depends only on the AST and the `Target`.
- Out of scope: MS layout, `-fpack-struct` and other triples.

## 2. Targets
`Triple::parse` accepts `{x86_64|amd64|x86_64h|aarch64|arm64}` +
`[-vendor]-linux-gnu`; anything else is a `TripleError`. The normalized
form is `<arch>-unknown-linux-gnu`.

| fact | x86_64 | aarch64 |
|---|---|---|
| `char`, `wchar_t` signed | yes | no |
| `long double` | x87 80-bit, 16/16 | IEEE quad, 16/16 |
| `__float128` | 16/16 | — |
| `__int128`, `_Float16`, `__bf16`, `__fp16` | 16/16, 2/2, 2/2, 2/2 | same |
| pointer, data member pointer | 8/8 | 8/8 |
| member function pointer | 16/8 | 16/8 |
| `max_align_t`, `aligned` (no argument) | 32/16, 16 | 32/16, 16 |
| `__STDCPP_DEFAULT_NEW_ALIGNMENT__` (`default_new_align`) | 16 | 16 |
| unnamed bit-fields affect class alignment | no | yes (AAPCS64 §10.1.8) |
| `_BitInt` max alignment | 8 | 16 |
| `_Atomic` promotion width | 16 bytes | 16 bytes |
| vector alignment cap | none | 16 |
| tail-padding rule | C++03 POD (Itanium) | C++03 POD (Itanium) |

`Target::types` is the `TargetTypes` stored in `TypeContext`.

`default_new_align` decides when a new- or delete-expression passes
`std::align_val_t` ([expr.new]/19, [expr.delete]/10: `alignof(T) >
16`) and the `align` attribute on an allocation call (spec/codegen.md
§3.1). Verified against clang 19's predefined macros on both triples
(`clang++-19 --target=<triple> -dM -E`).

**Operator-function names (WP-061a).** `kiln_abi::mangle` encodes an
operator function's <unqualified-name> as the Itanium
<operator-name> (`nw`, `na`, `dl`, `da`, `pl`, `ps`, `cl`, `ix`, ...;
unary and binary `+ - * &` are told apart by arity, counting the
implicit object parameter of a non-static member function). The
replaceable allocation functions mangle as clang's: `_Znwm`, `_Znam`,
`_ZdlPv`, `_ZdlPvm`, `_ZdlPvmSt11align_val_t`, `_ZnwmRKSt9nothrow_t`, ....

## 3. Type layout (clang `getTypeInfo`; sizes/alignments in bits)
| type | width | align |
|---|---|---|
| builtin | table §2 | table §2 |
| `_BitInt(N)` | `alignTo(N, A)` | `A = bit_ceil(clamp(N, 8, max))` |
| pointer, reference, `T C::*` | 64 | 64 |
| `R (C::*)(P...)` | 128 | 64 |
| `T[N]` | `N × width(T)` | `align(T)` |
| `T[]`, VLA, dependent array | 0 | `align(T)` |
| `_Complex T` | `2 × width(T)` | `align(T)` |
| `_Atomic(T)` | 0 → 8; ≤ 128 → `bit_ceil(width)` | promoted width when promoted, else `align(T)` |
| vector of N × T | `max(8, N × width(T))`, rounded to its power-of-two alignment | that width, capped per target |
| enum | underlying type ([dcl.enum]/7: fixed or computed; `int` if unknown) | idem |
| record | `RecordLayout::size × 8` | `RecordLayout::align × 8` |
| typedef / alias / `decltype` etc. | the underlying type | — |

A GCC `aligned` attribute on a typedef or an enum **replaces** the
alignment and may lower it. `typedef int A1 __attribute__((aligned(1)))`
has alignment 1.

### 3.1 Alignment attributes on declarations (encoding)
An `ALIGNED` `AttrNode` stores its argument in two fields:
- `a` is the argument's `ExprId`; 0 means "no argument", i.e. the
  target's biggest alignment.
- `b` is a raw `QualType` for `alignas(type-id)`.

`DeclArena::cold.alignment` holds further `alignas` expressions.

The maximum over all redeclarations wins. A value of 0 is ignored
([dcl.align]/4). Packing is `PACKED` (66); `[[no_unique_address]]` is
`FieldData::NO_UNIQUE_ADDRESS`.

## 4. Class properties
- **POD for the purpose of layout** (Itanium §1.1: the C++03 POD
  definition, clang `isPOD`). A class is *not* POD if any of these holds:
  - it has a base or is dynamic;
  - it has a private or protected non-static data member (a member of a
    `class` declared before any access specifier is private,
    [class.access]/3, also when sema records no access);
  - it has a default member initializer;
  - it has a reference, `_Atomic` or non-POD class member (arrays
    stripped);
  - it has a user-declared constructor, copy assignment or destructor,
    or a non-trivial special member.

  Unnamed bit-fields are ignored ([class.bit]/2).
- **Empty** (Itanium §1.1, clang `isEmpty`). A class is empty when it is
  not dynamic, has only empty non-virtual bases, and every field is
  either a zero-width unnamed bit-field or a `[[no_unique_address]]`
  member of empty class type. A non-zero unnamed bit-field makes the
  class non-empty, as it does in clang.

## 5. Record layout (Itanium §2.4; clang order)
State, all in bits: `size`, `data_size`, `align`, `unfilled_bits`,
`max_field_align` (§8.2; 0 = none), the base offsets and the field
offsets. Every alignment named "capped" below is `min(a,
max_field_align)` when `max_field_align` is not 0.

1. **Initialize.** `align = 8`, then raised to the class's maximum
   `aligned`/`alignas` value (§3.1); that value is not capped.
1a. **Non-virtual bases**, in declaration order (§8.1).
2. **Each field, in declaration order:**
   - **Non-bit-field** (`LayoutField`):
     - Its alignment is `align(T)`.
     - For a `packed` field, or in a `packed` class, the type's
       alignment is replaced by 8 bits. Exception: in a `packed` class,
       a field of non-POD class type that is not itself packed keeps
       its alignment (clang ≥ 15, `-Wpacked-non-pod`).
     - The field's own `aligned`/`alignas` then raises the alignment,
       even when packed. The result is capped.
     - The offset is `alignTo(data_size, field_align)`. In a union it
       is 0.
     - For class types and arrays of them, the empty-subobject map
       (`EmptySubobjectMap`, [intro.object]/9) bumps the offset by the
       field alignment until no two empty subobjects of the same type
       share an address.
   - **`[[no_unique_address]]` field of empty class type**
     ([dcl.attr.nouniqueaddr]/2): it occupies no data, and `data_size`
     is unchanged. It is first tried at offset 0. On a conflict it is
     tried at `alignTo(data_size, align)`, then in steps of its
     alignment. Only `size` grows to cover it.
   - **`[[no_unique_address]]` field of non-empty class type:** it uses
     the class's `dsize` instead of `sizeof`, so later fields may reuse
     its tail padding.
   - **Bit-field** (`LayoutBitField`, System V/AAPCS64 bit-field
     allocation):
     - The unit is `width(T)`. The field starts at `data_size` if it
       does not cross a unit boundary of `align(T)`; otherwise it starts
       at the next one.
     - A zero-width bit-field rounds up to its unit.
     - On x86-64, unnamed bit-fields contribute alignment 1. On AArch64
       they contribute `align(T)`.
     - A `packed` bit-field has alignment 1 bit.
     - Under `#pragma pack(N)`, for a non-zero width, the unit's
       alignment is capped. A `packed` bit-field then takes that capped
       unit alignment (not 1 bit); any other bit-field's alignment is
       capped (clang `LayoutBitField`).
     - Wider than its type (`LayoutWideBitField`, [class.bit]/1): the
       field is allocated as the widest of `unsigned char`…
       `unsigned long long` that fits in the width. The rest is padding.
3. **Flexible and zero-length arrays.** A trailing `T[]` and `T[0]` have
   width 0 and keep their element alignment, so `sizeof` may be 0.
4. **Finish.**
   - `nvsize = size` rounded up to bytes (clang takes `getSize()`,
     which an empty base or `[[no_unique_address]]` member may have
     grown past `data_size`). This is computed *before* the empty-class
     fix-up, so a non-POD empty class has `nvsize` 0.
   - An empty class gets `size = 8` bits.
   - `size = alignTo(size, align)`.
   - POD: `dsize = nvsize = size`. Otherwise `dsize = nvsize`.
   - `nvalign = align` and `preferred_align = align`.

`RecordLayout.fields` holds the bit offset of each `Field` declaration,
in declaration order, unnamed bit-fields included. `RecordLayout.bases`
holds each direct non-virtual base (first declaration) with its byte
offset, in declaration order.

## 6. Dump format (`-fdump-record-layouts`)
Byte-identical to clang 19's `DumpRecordLayout` in non-simple mode:
```

*** Dumping AST Record Layout
         0 | struct S
         0 |   _Bool b
     1:0-2 |   unsigned int x
       4:- |   int 
         0 |   struct E e (empty)
         4 |   struct P p
         4 |     int i
         8 |   int (*)(void) fp
           | [sizeof=16, dsize=16, align=8,
           |  nvsize=16, nvalign=8]
```
- **Offset column.** `%10d | ` is followed by two spaces per nesting
  level. A bit-field's offset is `byte:first-last`, or `byte:-` when it
  has width 0, right-aligned in 10 columns.
- **Header.** `{tag} {scoped name}`, then ` {field name}` when nested
  (an unnamed member still prints the space), then ` (empty)` for an
  empty class.
- **Nested classes.** A field of class type is expanded in place, with
  offsets relative to the outermost object. Arrays and `_Atomic` are not
  expanded.
- **Type printing.** Types print in clang's *C* printing policy:
  - `_Bool`, `nullptr_t`, `(void)`;
  - the class of a member pointer prints as a bare tag
    (`int struct C::*`);
  - written class, enum and typedef names print unqualified, without a
    tag keyword.
- **Unnamed classes and enums** print `(unnamed struct at F:L:C)` (the
  presumed location of the tag keyword) when `DumpOptions::sources` is
  given, and `(unnamed struct)` otherwise. An anonymous struct or union
  member prints `(anonymous struct at F:L:C)`.
- **Bases** print before the fields (§8.4).
- **Footer.** Printed only for the outermost class.

## 7. Tests (`crates/kiln_abi/tests/`)
- **`layout_rules.rs`.** One test per rule in §3–§6, each naming its
  standard or ABI section. Expected values are hard-coded. When
  `clang++-19` is installed, every model is also dumped by clang and
  compared byte for byte.
- **`layout_vs_clang.rs`.** Seeded random classes, 2000 per target in
  translation units of 40, for both targets. clang runs on at most 8 processes
  at once, and the test takes well under a second.
  - The generator covers:
    - every builtin, `_BitInt`, pointers, function pointers, member
      pointers, references, multi-dimensional and zero-length arrays,
      FAMs, `_Complex`, `_Atomic`, `decltype(nullptr)`, cv qualifiers,
      enums, aligned/under-aligned/vector typedefs, and nested classes
      and unions;
    - narrow, wide, zero-width and unnamed bit-fields;
    - `packed` and `aligned` on classes and fields, `alignas`,
      `[[no_unique_address]]` (empty and non-empty), access specifiers,
      and user constructors (non-POD).
  - Knobs: `KILN_ABI_DIFF_SEED`, `KILN_ABI_DIFF_RECORDS` and
    `KILN_ABI_DIFF_SHOW`.
  - Known divergences go in `XFAIL` with a reason. It is empty.
  - The test is skipped when clang is absent.
  - Since WP-066c it also generates 1–3 non-virtual bases per class
    (35 %; empty bases preferred 40 % of the time, so repeated empty
    types collide) and `#pragma pack(push, N)` (8 %, N ∈ {1, 2, 4, 8,
    16}) around a class.
  - Since WP-078b each base is virtual with p = 0.3 and a non-union
    class gets a virtual function with p = 0.2 (no FAM in a dynamic
    class), so the vptr, primary-base selection (including nearly-empty
    virtual bases and indirect primaries), repeated virtual bases and
    the `(virtual base)` / `(primary virtual base)` dump lines are
    covered. The default seed prints about 1200 virtual-base lines, 800
    primary-base lines and 2400 vptr lines across both targets.
- **`vtable_vs_clang.rs`** (WP-078b). Seeded random polymorphic
  hierarchies, 500 classes per target in translation units of 25: virtual
  and repeated bases (subobject graphs ≤ 48 nodes), overriding,
  covariant returns through non-virtual and virtual bases, pure, deleted
  and `consteval` virtual functions, virtual destructors. Every class has
  an out-of-line key function and a user constructor so clang emits (and
  dumps) its vtable. Each class's `dump_vtable_layout` and every
  `dump_construction_vtable` must equal clang's `-Xclang
  -fdump-vtable-layouts` blocks byte for byte, and `vtt` (with
  `vtt_vtables` and the address points) must name the same
  `getelementptr inbounds inrange(..) (.., ptr @_ZTV../_ZTC.., i32 0, i32
  K, i32 N)` entries as clang's `@_ZTT` initializer, read from the IR it
  writes. Knobs `KILN_VTABLE_DIFF_SEED`, `KILN_VTABLE_DIFF_CLASSES`;
  `XFAIL` is empty. The default seed checks about 4100 vtables and VTTs.
- **`vtable_rules.rs`** (WP-078b). Hand-built probes with values
  transcribed from clang 19 (and cross-checked against it when present):
  non-virtual `this` thunk, virtual `this` thunk with vbase offset
  offsets, the diamond's construction vtables and VTT, a covariant return
  thunk, destructor slots and method indices.
- **`layout_rules.rs`** gains dynamic-class probes (WP-078b): the vptr,
  primary base selection, nearly-empty virtual primary bases, and the
  `(primary virtual base)` label (§8.5).
- **`layout_bases.rs`.** Hand-written cases for §8 with the C++ in doc
  comments and expected `(sizeof, align, dsize, nvsize, base offsets,
  field offsets)` checked against clang 19: the same-type empty-base
  rule, `[[no_unique_address]]` after an empty base, tail-padding reuse
  of non-POD bases, `#pragma pack` capping base and field alignment,
  `alignas` bases, bit-fields after bases, and the subobject preorder.
- **`common/mod.rs`.** A small model that renders as C++ source and as a
  kiln AST built through `AstBuilder` (`RecDef::bases` and
  `RecDef::pack` included). It will be retired for parser-built ASTs
  once the parser accepts every construct it generates.
- **Corpus (`tests/layout/*.cpp`, `kiln_driver/tests/record_layouts.rs`).**
  At least 40 programs parsed by the real front end. `kiln -fsyntax-only
  -fdump-record-layouts` must equal `clang -Xclang
  -fdump-record-layouts-complete` byte for byte on both triples, block by
  block, after dropping clang's builtin records (`__NSConstantString_tag`,
  `__va_list_tag`, `std::__va_list`). Without clang the test only
  checks that kiln accepts each program. Base classes join the corpus
  once the parser accepts base clauses (WP-065).
- **`kiln_driver/tests/vtable_layouts.rs`** (WP-078b). Small programs
  run through the real front end. `kiln -fsyntax-only
  -fdump-vtable-layouts` must match clang's emission-time dump as a set
  of blocks, with `Original map` sections removed (spec/driver.md).

## 8. Bases, `#pragma pack`, subobjects (WP-066c)
Itanium §2.4 II.2 and clang `ItaniumRecordLayoutBuilder` for
non-virtual bases. The non-virtual part of the algorithm is final;
WP-078b *adds* the steps in §8.5 around it.

### 8.1 Non-virtual bases (clang `LayoutNonVirtualBases` / `LayoutBase`)
Each direct base-specifier that names a complete class and is not
`virtual` is placed in declaration order, before any field (§5 step 1a).
The base's own layout gives `nvsize`, `nvalign`, `size` and whether it
is empty.
- **Empty base.** It is tried at offset 0. If the empty-subobject map
  (§8.3) accepts it there, `size = max(size, base size)` and `data_size`
  is unchanged.
- **Otherwise** it goes at `alignTo(data_size, a)`, where `a` is the
  base's `nvalign` capped by `max_field_align` (§8.2) and at least 1
  byte. While the map rejects the offset, the offset advances by `a`.
  The loop terminates: past the last recorded empty subobject every
  offset fits.
  - A non-empty base sets `data_size = offset + nvsize` and
    `size = max(size, data_size)`. Fields that follow reuse the base's
    tail padding (its `nvsize` excludes it when the base is not POD for
    layout, §4).
  - An empty base placed away from 0 sets
    `size = max(size, offset + base size)` only.
- `align` is raised to the base's `nvalign`, uncapped for an empty base
  at 0 and capped otherwise. `__attribute__((packed))` on the derived
  class does not affect bases (clang ≥ 7 on Linux).
- A class with any base is not POD for layout (§4), so its own `dsize`
  and `nvsize` exclude its tail padding.

Probes (clang 19, both targets):
- `struct E {}; struct E2 : E {}; struct D : E, E2 { char c; };`: `E` at
  0, `E2` at 1 (its `E` would collide with `D`'s), `c` at 0, sizeof 2.
- `struct R : E { [[no_unique_address]] E e; int r; };`: base `E` at 0,
  `e` at 1, `r` at 0, sizeof 4.
- `struct P { int i; char c; P(); }; struct Q : P { char q; };`: `q` at
  5, sizeof 8, dsize 6.
- `struct S : alignas(16) A {}` follows `A`'s alignment: align 16.

### 8.2 `#pragma pack` (clang `MaxFieldAlignment`)
Sema keeps the pack state (spec/sema-class.md §6.1) and records the
value current at a class's `{` in `DeclArena::cold.record_pack:
DeclId → u16` (bytes; absent = none). `ClassDesc::max_field_align` is
that value in bits. A nonzero value caps:
- every field's alignment (after `packed` and `aligned`/`alignas`);
- the bit-field unit alignment and the bit-field alignment;
- the alignment of each non-empty base, and of an empty base not placed
  at 0.

The class's own `aligned`/`alignas` is not capped. `#pragma pack(N)`
does not make a class `packed`: a non-POD member keeps its alignment
up to N.

### 8.3 Empty-subobject map and subobject table
- **Map** (`layout/empty.rs`, clang `EmptySubobjectMap`). A class's
  subobjects are its non-virtual bases at their offsets and its fields
  of class or array-of-class type. `can_place_base` checks a base
  (`CanPlaceBaseAtOffset`) and `can_place_field` checks a field. On
  success, each records the empty subobjects it adds. Offsets at or
  above the class's *largest empty subobject* (the largest over bases
  and class-typed fields of the empty class's size, or of the member's
  own largest empty subobject) are not recorded, except while placing an
  empty base or an overlapping field. All walks use explicit stacks.
- **Subobject table** (`layout/subobject.rs`). `Layouter::subobjects(C)`
  and `kiln_abi::subobjects` return every base-class subobject of the
  most-derived class `C` as `(Subobject, byte offset)`, indexed by
  `SubobjectId` in preorder:
  - 0 is `C` itself (`parent = SubobjectId::COMPLETE`, `base_index`
    0);
  - then each direct non-virtual base's subtree in declaration order,
    with `parent` the subobject it is a direct base of and `base_index`
    its base-specifier's index.

  Repeated bases get one entry per occurrence. The table stops at
  `MAX_SUBOBJECTS` (65 536) entries, because a non-virtual hierarchy
  can have exponentially many subobjects. sema-class.md §3.5
  (`ClassVirtuals`) and codegen use these ids and offsets.

### 8.4 Dump of bases; `-complete`
- **Base lines.** After the class line, the non-virtual bases are
  printed, stably sorted by offset. Each is printed as
  `{off} |   {tag} {name} (base)`, or `(primary base)` (WP-078b), and is
  expanded recursively like a member, with no footer. The fields follow.
- **Footer.** It prints `dsize`/`nvsize`, which differ from `sizeof`
  for a non-POD class.
- **Driver.** `kiln -fsyntax-only -fdump-record-layouts[-complete]`
  (driver.md) calls `dump_all_record_layouts`. It dumps every class of
  `record_definitions(ast)`, the order clang 19's
  `-fdump-record-layouts-complete` uses:
  - The classes are the complete, valid, non-dependent definitions.
  - The order is lexical post-order: a class follows the classes
    defined inside it, and top-level classes are in source order. A
    class inside a function body is ordered as its own root, because
    member function bodies are parsed after the enclosing `}`.
  - Each class is named as at its closing brace
    (`DumpOptions::at_definition`). An unnamed class later given a
    typedef name for linkage still prints `(unnamed struct at …)`
    there.
  - Templates and invalid classes are skipped.
- **Deviation.** Plain `-fdump-record-layouts` behaves like `-complete`
  (open question A5). The dump is written even when the translation unit
  has errors.

### 8.5 Dynamic classes and vtables (WP-078b)
**Record layout.** `RecordLayout` (kiln_ast `types/layout.rs`) is
unchanged; WP-078b fills `vbases`, `vptr` and `primary_base`. The
algorithm ports clang's `ItaniumRecordLayoutBuilder`
(`layout/record.rs`, `layout/base_info.rs`):
1. **Dynamic** = polymorphic or with a virtual base (clang
   `isDynamicClass`).
2. **Primary base** (II.1, clang `DeterminePrimaryBase`): the first
   non-virtual dynamic base; else the first nearly-empty virtual base,
   in a preorder walk of the base-specifiers, that is not an indirect
   primary base; else the first nearly-empty virtual base that is one.
   Nearly empty = dynamic with `nvsize` of one pointer.
3. **vptr** at 0 (8 bytes, align 8) if the class is dynamic with no
   primary base; otherwise the primary base goes at 0 and §8.1 skips it.
4. **Non-virtual bases and fields** as in §8.1 and §5; then `nvsize`
   and `nvalign` are taken. For a dynamic class they are not
   POD-derived.
5. **Virtual bases** (III, clang `LayoutVirtualBases`), in
   inheritance-graph order (preorder over base-specifiers, each class
   expanded once), skipping each one that is the class's primary base or
   an indirect primary base. Each uses the §8.1 rule at offsets ≥
   `dsize`, and `size` grows to cover it. A virtual base claimed as the
   primary base of a base subobject (clang `BaseSubobjectInfo::Derived`)
   shares that subobject's offset (`AddPrimaryVirtualBaseOffsets`).
- **Base-subobject tree** (`base_info.rs`, clang `BaseSubobjectInfo`):
  one node per non-virtual path plus one per virtual base, built with an
  explicit stack and capped at 2^16 nodes (beyond the cap, nodes have no
  children).
- **Empty-subobject map.** It walks the tree: a base's non-virtual bases,
  the virtual primary base it claimed (at its own offset) and its
  class-typed fields. A field's own class contributes its virtual bases
  at their offsets too (clang `CanPlaceFieldSubobjectAtOffset`).
- **Subobject table** (§8.3). After the non-virtual subtree come the
  virtual bases, once each, in inheritance-graph order, each followed by
  its non-virtual subtree, with `is_virtual` set. A virtual base's
  `parent` is the first subobject whose class names it as a direct
  virtual base. This is the numbering of sema's `subobject_graph`.
- **Dump.**
  - The vptr prints as `{off} |   (C vtable pointer)` before the bases.
  - Non-virtual bases print in declaration order, stably sorted by
    offset.
  - Virtual bases print once, after the most-derived class's fields (and
    after those of a class-typed field's class), as `(virtual base)`.
  - A virtual base is labelled `(primary virtual base)` when its *class*
    is the primary base's class. clang compares classes, so a virtual
    base `P` is so labelled even when the primary base is a non-virtual
    `P` subobject. kiln matches this.

**Vtables** (`kiln_abi::vtable`, §1 signatures). These are ports of
clang 19's `ItaniumVTableBuilder`, `VCallAndVBaseOffsetBuilder`,
`FinalOverriders` and `VTTBuilder`, function by function, in
`vtable/{builder,overriders,classes,vtt,print}.rs`. Components, address
points, thunks and method indices are clang's.
- **Inputs:**
  - `crate::layout` (primary bases, base offsets, subobjects);
  - the override lists `ast.decls.cold.overrides`, written by sema
    (WP-078a);
  - the method kinds: destructors take two slots, and `consteval`
    virtual functions take none (clang `hasVtableSlot`).
- **Construction vtables and VTTs.**
  - `vtable_layout` returns the complete group.
  - `construction_vtable(most_derived, base)` returns the group used
    while subobject `base` is built. Its address points name subobjects
    of `most_derived`.
  - `vtt_vtables` lists the groups the VTT points into. Index 0 is
    `COMPLETE`, then the construction groups in VTT order.
  - `vtt` lists `VttComponent { vtable_index, base }`. The address
    point it holds is the group's `address_points` entry for `base`.
  - Both are empty for a class without virtual bases.
  - Codegen derives the IR form of an entry from these:
    - `K` is the vtable of the group whose range holds the address
      point (`vtable_starts`);
    - `N` is the address point minus that vtable's start;
    - the symbol is `_ZTV<class>` for group 0, else
      `_ZTC<class><offset>_<base>`.
- **`VTableLayout` fields** (kiln_ast `class.rs`; WP-078b adds
  `vtable_starts` and `method_thunks`, and `Thunk::{method, this_class}`):
  - `address_points` point at the slot after the RTTI. One may equal its
    vtable's end, for a vtable with no function slots.
  - `method_indices` count from the primary address point. They include
    the methods of the primary-base chain, and a destructor's entry is
    its complete-object slot. Construction groups carry them too, keyed
    to the base class.
  - `thunks` maps a slot to its adjustment.
  - `method_thunks` lists, for each overrider declared in the class, the
    thunks to emit (clang `Thunks`; empty for construction vtables).
  - `Thunk.this_class` is the class of `Thunk.method`.
- **Bounds.** Every walk is iterative and bounded. The following answer
  `TooComplex` and never panic:
  - a dynamic chain deeper than 256;
  - more than `MAX_SUBOBJECTS` base paths;
  - more than 2^26 builder steps.
  `Incomplete` covers a missing definition, a missing override-list
  entry, no unique final overrider, and an unknown subobject.
- **Dump** (`-fdump-vtable-layouts`; byte-identical to clang 19 for each
  block):
  - `dump_vtable_layout` prints `Vtable for`, `Thunks for`, `Virtual base
    offset offsets for` and `VTable indices for`.
  - `dump_construction_vtable` prints `Construction vtable for ('B', off)
    in 'D'`.
  - `dump_vtable_group` prints the class's dump followed by its VTT's
    construction vtables.
  - `dump_all_vtable_layouts` (driver `-fdump-vtable-layouts`,
    spec/driver.md) prints every group in `record_definitions` order.
  - Deviations from clang:
    - clang's `Original map` sections are not printed. They are
      debugging state that clang accumulates across classes.
    - Order and repetition: clang dumps in emission order and may repeat
      a construction vtable; kiln dumps each group once.
- **Gaps:**
  - Method names omit the ` [T = int]` suffix clang prints for members
    of class template specializations (`print.rs` `pretty_function`).
    Templates are not yet generated by any differential.
  - Implicit virtual destructors and implicitly declared members appear
    in the slots only once sema declares them (WP-067a). Explicit
    constructors and destructors are still "not supported yet" in sema's
    parser path, so the driver differential uses key functions only.
  - Conformance tests pin both dumps with `expect: layout` and
    `expect: vtables` (spec/harness.md "Layout and vtable tests", WP-084).
    They and `kiln_driver/tests/{record,vtable}_layouts.rs` split the dumps
    with `kiln_abi::dump_blocks`, which encodes the two deviations above.

### 8.6 Landed (WP-079): class lowering, argument passing, mangling, member pointers
- **Record lowering** (`irtype.rs`, clang `CGRecordLowering`):
  - the vptr comes first, then non-virtual bases and fields, then
    virtual bases;
  - a class whose non-virtual size differs from its size also gets a
    `%struct.S.base` subobject type (`IrType::RecordBase`,
    `getBaseSubobjectLLVMType`);
  - `IrRecord::base_index` and `vbase_index` map each base to its
    element.
- **Argument passing** (`call.rs`): `can_pass_in_registers` follows
  clang's `canPassInRegisters`. A class with a non-trivial copy or move
  constructor or destructor, or with all of them deleted, is passed
  indirectly and returned via `sret`.
- **Mangling** (`mangle.rs`): the Itanium mangler covers classes,
  templates, structor variants (C1/C2/D0/D1/D2), local entities and
  member pointer types. It is checked against clang by
  `kiln_codegen/tests/mangle_vs_clang.rs` over `tests/mangle/`.
- **Member pointers** (Itanium §2.3; clang `ItaniumCXXABI` with
  `UseARMMethodPtrABI` on AArch64):
  - A data member pointer is an `i64` byte offset. Null is `-1`.
  - A member function pointer is `{ i64 ptr, i64 adj }`. Null has
    `ptr == 0`.
  - On x86-64, `adj` is the `this` adjustment, and a virtual function's
    `ptr` is its vtable offset plus 1.
  - On AArch64, `adj` is `2 * adjustment + is_virtual`, and `ptr` is the
    plain vtable offset.
  - Conversion between base and derived scales the adjustment by 2 on
    AArch64.
  - Calling convention: an `i64`, or `{ i64, i64 }`. On x86-64 the pair
    is passed in two INTEGER eightbytes; on AArch64 it is coerced to
    `[2 x i64]`. Codegen is in spec/codegen.md §7.

## 9. Open questions (with recommendation)
- **A1.** `preferred_align` is equal to `align` on both targets. i386
  `double` is where it differs; out of scope.
- **A2.** *Resolved (WP-066c):* `DumpOptions::sources` prints unnamed
  classes with their location.
- **A3.** *Resolved for `#pragma pack` (WP-066c, §8.2).* `-fpack-struct`
  remains open; recommendation: the driver sets a default
  `max_field_align` that `#pragma pack` overrides, as clang does.
- **A4.** `[[no_unique_address]]` arrays of empty classes walk every
  element in the empty-subobject map, as clang does. Arrays with
  millions of elements are linear-time. Recommendation: cap the walk at
  the class's largest-empty-subobject offset, as clang's
  `SizeOfLargestEmptySubobject` early-out does for offsets past it.
- **A5.** Plain `-fdump-record-layouts` dumps every complete class, like
  `-complete`. clang dumps only classes it lays out on demand (those
  whose size or offsets something asked for), in that order.
  Recommendation: keep the superset; it is deterministic and the
  differential compares against `-complete`.
