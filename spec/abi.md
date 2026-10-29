# spec: kiln_abi — targets, type and record layout

Status: **implemented through WP-066c**: Itanium layout of classes with
non-virtual bases (empty-base optimization, tail-padding reuse,
`[[no_unique_address]]`, `#pragma pack`, `packed`/`aligned`), the
base-subobject table, and the `-fdump-record-layouts` dump of all of
them (§8). Virtual bases, the vptr and primary bases are reserved for
WP-078b (§8.5) and do not reshape the interface.

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
`ItaniumRecordLayoutBuilder`, `EmptySubobjectMap`, `DumpRecordLayout`).
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
- Not yet supported: virtual bases, the vptr and primary bases (WP-078b,
  §8.5); a dynamic class is laid out as if it had neither. MS layout,
  `-fpack-struct` and other triples are out of scope.

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
| unnamed bit-fields affect class alignment | no | yes (AAPCS64 §10.1.8) |
| `_BitInt` max alignment | 8 | 16 |
| `_Atomic` promotion width | 16 bytes | 16 bytes |
| vector alignment cap | none | 16 |
| tail-padding rule | C++03 POD (Itanium) | C++03 POD (Itanium) |

`Target::types` is the `TargetTypes` stored in `TypeContext`.

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
- **`layout_vs_clang.rs`.** Seeded random classes, 320 per target in 8
  translation units, for both targets. clang runs on at most 8 processes
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

### 8.5 Reserved for WP-078b (vptr, primary base, virtual bases)
- **`RecordLayout`** (kiln_ast `types/layout.rs`) already carries
  `vbases`, `vptr` and `primary_base`. WP-066c leaves them empty.
  WP-078b may add `size_of_largest_empty_subobject` and `has_own_vfptr`
  if codegen needs them.
- **Steps**, in order:
  1. **Primary base.** Choose it (II.1): the first non-virtual dynamic
     base; failing that, the first nearly-empty virtual base that is not
     an indirect primary base; failing that, the first such base that is
     an indirect primary base.
  2. **vptr.** Place it at 0 (8 bytes, align 8) if the class is dynamic
     with no primary base. The primary base goes at 0, and §8.1 skips
     it.
  3. **Non-virtual bases and fields** follow, as now.
  4. **`nvsize` and `nvalign`** are taken.
  5. **Virtual bases** (III), in inheritance-graph order, each one not
     already a primary base. They use the §8.1 rule at offsets ≥
     `dsize`, and `size` grows to cover them.
- **Dynamic classes.** `nvsize` and `dsize` stop being POD-derived for
  them.
- **Empty-subobject map.** It gains virtual bases: they are recorded
  once, at their complete-object offset.
- **Subobject table.** After the non-virtual subtree, it gains one entry
  per virtual base, with `is_virtual` set and `parent` the first
  subobject in preorder that names it.
- **Dump.** The vptr prints as `{off} |   (C vtable pointer)`. Virtual
  bases print once, after the most-derived class's fields, as
  `(virtual base)`.

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
