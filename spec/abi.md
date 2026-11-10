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
pub fn ensure_layout<'a>(&'a mut Ast, DeclId, &Target) -> Option<&'a RecordLayout>; // None: incomplete
pub struct Layouter<'a>   // a session: memoizes records, answers is_pod / is_empty
pub struct LayoutCache;   // a finished session's records (Layouter::into_cache / with_cache;
                          // into_complete_cache drops the incomplete classes, §1)
pub fn subobjects(&Ast, record: DeclId, &Target) -> Vec<(Subobject, u64)>; // §8.3
pub const MAX_SUBOBJECTS: usize;              // 65536 (SubobjectId is u16)
impl Layouter { fn subobjects(&mut self, DeclId) -> Vec<(Subobject, u64)> }
pub fn padding_mask(&mut Layouter, QualType) -> Option<Vec<u8>>; // §3.3; None: over 16 MiB
pub const MAX_PADDING_MASK_BYTES: u64;
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
- **Total.** No input panics. Incomplete, dependent or recursive
  (`struct S { S s; }`, rejected by sema) types get the degenerate
  layout (size 0, align 1). A class sema flagged invalid
  (sema-class.md §2.3) has size, data size and every alignment 1, as
  clang's `getASTRecordLayout` gives it, with every field and base at
  offset 0. Arithmetic is in bits and saturating.
- **No recursion on the native stack.** Record dependencies are resolved
  by an explicit post-order stack, the empty-subobject walks and the
  dump use explicit stacks too, and the type printer is depth-capped.
  Arbitrarily nested input is safe.
- **Memoized.** A `Layouter` computes each class once, keyed by its first
  declaration. `ensure_layout` writes every class it computed into
  `Ast::layouts` (write-once, spec/types.md §11). Classes already there
  are reused, not recomputed. `Layouter::into_cache` detaches a
  session's records (`LayoutCache`) and `Layouter::with_cache` resumes
  them in a later session over the same AST; the caller guarantees that
  every class in the cache is still complete and unchanged (sema keeps one
  for the deallocation-function lookup of virtual destructors; WP-450).
  `Layouter::into_complete_cache` detaches them without the classes the
  session laid out while they were incomplete (no definition, or not yet
  at `}`): sema lays out a class still being defined over its cache and
  keeps only its complete bases and members (WP-428, sema-class.md §6.5).
- **Complete classes only.** `ensure_layout` returns `None` for a class
  that is incomplete ([basic.types.general]/5): it has no definition, or
  its definition has not reached `}` (`DefinitionBits::COMPLETE` is
  unset, e.g. `S` inside its own member-specification). Such a class has
  no layout yet, and a cached one would be write-once wrong after the
  class completes. Every caller handles `None`: sema already diagnoses
  `sizeof`/`alignof`/member access of an incomplete type before it asks
  for a layout, the Oven keeps the class an opaque provisional slot
  (spec/oven.md, type table) and rebuilds it once the class completes,
  and the remaining callers (packed and `__builtin_offsetof` checks,
  `std::source_location`, codegen folding) treat it as "no layout"
  rather than reading a bogus one. `record_layout` and `Layouter` still
  answer for any class; `ensure_layout` is the cached entry point (WP-471).
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
| `_Float32`, `_Float64`, `_Float128` (P1467) | 4/4, 8/8, 16/16 | same |
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

### 3.2 Declaration alignment (`layout::decl_align`, WP-312)
`decl_align(ast, d, target, for_alignof)` is clang's
`ASTContext::getDeclAlign(d, ForAlignof)`, in bytes, at least 1:
- a variable with an alignment attribute: the attribute's alignment
  alone ([dcl.align]/5; a GNU `aligned` may lower it);
- otherwise its type's; a reference is its referee for `alignof`
  (`for_alignof`) and a pointer for storage; for storage an x86-64 array
  of at least 16 bytes is raised to 16 (`LargeArrayMinWidth`);
- a data member: the strictest of its attribute and type, capped by
  what its offset guarantees in the class (the class alignment, or the
  lowest set bit of a non-zero offset); in a packed class, or when
  packed itself, its attribute's alignment only.

No minimum global alignment applies on the M2 targets. Users: codegen
`var_align` (global, `alloca`, `catch` and static-local storage), the
Oven's `SemaBridge::decl_align` (`__builtin_assume_aligned`) and
`SemaBridge::alignof_decl` (`alignof` of an expression naming `d`).

### 3.3 Padding bits (`layout::padding`, WP-774)

`padding_mask(lay, T)` describes the object representation of a `T`. It
returns one byte per byte of `T`, and a set bit in it is a padding bit:
a bit that holds no part of `T`'s value ([basic.types.general]/4). The
rules are GCC's `__builtin_clear_padding` (`gimple-fold.cc`
`clear_padding_type`), the only consumer (spec/codegen.md "Clearing
padding"). A bit is a value bit when the value representation of some
scalar subobject covers it:
- every byte of a scalar is a value byte, `bool`, `std::nullptr_t`,
  enumerations, pointers, member pointers, `__fp16`, `_Complex` and
  vectors included. The exceptions are x87 `long double` / `_Float64x`
  (80 value bits of 128) and `_BitInt(N)` (N value bits);
- a named bit-field's declared bits, capped at its type's width;
- the vtable pointer;
- each element of an array, and the first `sizeof(T)` bytes of an
  `_Atomic(T)`.
Everything else is padding:
- gaps between and after members, and the tail of a non-POD base that
  the derived class does not reuse. A base is its non-virtual part;
  virtual bases count only in a complete object;
- unnamed bit-fields;
- the byte of an empty class, and empty bases and empty
  `[[no_unique_address]]` members (a non-empty one is laid out as a base
  subobject);
- in a union, a bit that is padding in every member.
Bit-field bits number little-endian, the only byte order kiln targets.
An object over 16 MiB (`MAX_PADDING_MASK_BYTES`) gets `None`. Class
masks are memoized for each call.

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
5. **Unadjusted alignment** (clang `ASTRecordLayout::getUnadjustedAlignment`,
   WP-583). Two more values, in bytes, at least 1:
   `unadjusted_field_align` is the maximum `field_align` of step 2 (after
   `packed`, the field's own `alignas` and the cap; a field of class type
   contributes that class's *adjusted* alignment) over the ordinary and
   bit-fields, not the wide bit-fields and not the class's own
   `aligned`/`alignas` of step 1 (clang 19/20). `unadjusted_align` also
   counts each non-virtual base not placed as an empty base at offset 0,
   with its capped `nvalign` (clang 21+, llvm-project#135551).
   `Layouter::type_unadjusted_align(q, bases)` reads one of them for a
   record (looking through typedefs, so a typedef's `aligned` does not
   count) and the type's alignment for any other type. The AAPCS64
   classification uses it (spec/landed/abi/WP-583.md).

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
    tag keyword;
  - a substituted template parameter prints its canonical replacement,
    whose classes and enumerations carry their keyword and scope
    (`enum E t`, `struct Q *p` in `P<E>`, `F<Q>`);
  - template closers are split (`P<P<int> > *`,
    `PrintOptions::split_closers`).
- **Specialization names** ([temp.names], WP-324). A class template
  specialization, in a header, a scope or a type, is named with its
  arguments as clang's `printTag` does under the C policy
  (`types::write_arg_list_c`): canonical arguments with their scope,
  tag types with their keyword, trailing default arguments omitted,
  values as template arguments (`struct S<int, 4>`,
  `struct P<struct P<char> >`, `struct N<true, 'a', 3>`,
  `struct ns::W<int *>`, `struct Outer<int>::In<char>`). An alias
  template specialization names the class it stands for. Test:
  `tests/conformance/temp.inst/layout-specialization-names.cpp`.
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
  At least 40 programs parsed by the real front end. `kiln++ -fsyntax-only
  -fdump-record-layouts` must equal `clang -Xclang
  -fdump-record-layouts-complete` byte for byte on both triples, block by
  block, after dropping clang's builtin records (`__NSConstantString_tag`,
  `__va_list_tag`, `std::__va_list`). Without clang the test only
  checks that kiln accepts each program. Base classes join the corpus
  once the parser accepts base clauses (WP-065).
- **`kiln_driver/tests/vtable_layouts.rs`** (WP-078b). Small programs
  run through the real front end. `kiln++ -fsyntax-only
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
- **Driver.** `kiln++ -fsyntax-only -fdump-record-layouts[-complete]`
  (driver.md) calls `dump_all_record_layouts`. It dumps every class of
  `record_definitions(ast)`, the order clang 19's
  `-fdump-record-layouts-complete` uses:
  - The classes are the complete, valid, non-dependent definitions.
  - The order is the order the classes were completed
    (`DeclArena::completed_records`, spec/ast.md): a class follows the
    classes defined inside it, top-level classes are in source order,
    and a class inside a member function body follows the class (the
    body is parsed after the enclosing `}`). An implicit instantiation
    ([temp.inst]/2) comes where it was first required to be complete:
    before a class whose member required it (`struct H { S<char, 2> s;
    };` dumps `S<char, 2>` first), and a member class of a
    specialization after that specialization (WP-324).
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
- **`VTableSession`** (WP-414): a translation-unit session over the
  free functions above. One `Layouter` and `Classes` fact cache is shared
  by every query, and `vtable_layout`, `construction_vtable`, `vtt`
  (`VttLayout`: the VTT entries with the groups they name) and
  `subobjects` (`SubobjectTable`: each subobject with its offset and a
  `(class, offset)` → `SubobjectId` index) are cached per class. Each
  query is refuelled to the builders' full step budget (`Classes::refuel`),
  so a session returns exactly what the free functions return, each of
  which runs in a fresh session. Codegen holds one per module.

### 8.6 Landed (WP-079): class lowering, argument passing, mangling, member pointers
- **Record lowering** (`irtype.rs`, clang `CGRecordLowering`):
  - the vptr comes first, then non-virtual bases and fields, then
    virtual bases;
  - a class whose non-virtual size differs from its size also gets a
    `%struct.S.base` subobject type (`IrType::RecordBase`,
    `getBaseSubobjectLLVMType`);
  - a potentially-overlapping field (`[[no_unique_address]]` of
    non-empty class type) is stored as its class's base-subobject type,
    as the layout places it, so later fields may use its tail padding
    (WP-158);
  - a union whose non-virtual size (its data size) is short of its size
    also gets a `%union.U.base` type, lowered as the union padded to its
    data size (clang 19 `lowerUnion(isNonVirtualBaseType)`), which a
    potentially-overlapping member of that union type uses (WP-289);
  - `IrRecord::base_index` and `vbase_index` map each base to its
    element;
  - a `_BitInt(N)` member, array element or object is the integer of
    its ABI size (`i256` for `_BitInt(200)`), unless LLVM allocates that
    `iN` in a different number of bytes than the type's size, when it is
    the byte array `[size x i8]` (`_BitInt(129)`: `i129` allocates 32
    bytes, the type 24; clang `typeRequiresSplitIntoByteArray`); a value
    is loaded and stored as the integer of the size either way
    (`IrTypes::convert_load_store`, clang `convertTypeForLoadStore`),
    and a constant initializer of a byte-array slot is emitted as bytes
    (WP-465; `tests/codegen/bit_int_wide_record.cpp`);
  - padding (clang `insertPadding`) fills the gap before each member
    with storage. Those members are disjoint and sorted by offset: an
    empty `[[no_unique_address]]` member has no storage (several may
    share an offset, with each other and with the next member), a
    potentially-overlapping one is stored as its base-subobject type,
    and a bit-field run as its access units. That is an internal
    invariant (clang's `assert(Offset >= Size)`): the gap is a
    `checked_sub`, debug-asserted, and a violation drops the padding
    rather than wrapping. No diagnostic, since no input reaches it
    (WP-294; `tests/layout/nua_overlap_bitfields.cpp` and
    `tests/codegen/record_type_nua_bitfields.cpp` cover both shapes on
    both triples);
  - no member, element or value is ever `i0` (invalid IR): a builtin
    without a size (an error or dependent placeholder, which only an
    undiagnosed instantiation failure lets reach codegen) lowers to
    `i8`, like the other unrepresentable builtins (`IrTypes::builtin`;
    WP-582, spec/landed/codegen/WP-582.md).
- **Argument passing** (`call.rs`): `can_pass_in_registers` follows
  clang's `canPassInRegisters`. A class with a non-trivial copy or move
  constructor or destructor, or with all of them deleted, is passed
  indirectly and returned via `sret`.
- **Mangling** (`mangle.rs`): the Itanium mangler covers classes,
  templates, structor variants (C1/C2/D0/D1/D2), local entities and
  member pointer types. It is checked against clang by
  `kiln_codegen/tests/mangle_vs_clang.rs` over `tests/mangle/`. A GNU
  `__restrict` method qualifier is left out of a member function's
  nested-name (`int T::f() const __restrict` is `_ZNK1T1fEv`, as clang's
  `mangleNestedName` writes only `V` and `K`) but kept in a
  pointer-to-member function type (`M1TrKFivE`; WP-1091,
  `tests/mangle/restrict_method.cpp`).
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

### 8.7 Landed (WP-080): vtable and RTTI mangling and layout
- **Mangling** (`mangle.rs`): `_ZTV` (vtable), `_ZTI` / `_ZTS` (type_info
  and its name, for any type), `_ZTT` (VTT), `_ZTC<derived><offset>_<base>`
  (construction vtable), and thunks `_ZThn<off>_` (non-virtual `this`
  adjustment), `_ZTv<off>_<vcall>_` (virtual), `_ZTch...h...` /
  `_ZTcv...` (covariant). Destructor thunks mangle the D1/D0 target.
- **Vtable layout** (`kiln_abi::vtable`, Itanium §2.5): per group, vcall
  and vbase offsets, offset-to-top, RTTI, then the slots. A virtual
  destructor takes two slots (D1, D0). `vbase_offset_offsets` gives the
  (negative) byte offset of each vbase-offset word from the address
  point.
- **RTTI layout** (Itanium §2.9.5): every type_info starts
  `{ ptr vtable + 16, ptr name }`. `__si_class_type_info` adds the base
  pointer; `__vmi_class_type_info` adds `i32 flags, i32 base_count` and a
  `{ ptr, i64 offset_flags }` per direct base; `__pointer_type_info`
  adds `i32 flags, ptr pointee`. Enums, functions and arrays have no
  extra fields.
- **VTT** (Itanium §2.6, `kiln_abi::vtable::vtt`): the class's own
  primary vtable address point, then per non-virtual base with virtual
  bases its sub-VTT (construction vtable `_ZTC`), then the secondary
  virtual pointers, then the virtual bases' sub-VTTs. Codegen reads two
  indices from it: a base subobject's sub-VTT index (the first component
  pointing into its construction vtable) and a secondary vptr's index
  (the component for that subobject at address point 0 of the group).
  The base-object structor of a class with virtual bases takes the VTT
  as a second, implicit `ptr` parameter after `this`.
- **Member function pointers to virtual functions**: see 8.6 (x86-64
  `ptr = offset + 1`; AArch64 `adj = 2 * adj + 1`).

### 8.8 Landed (WP-107): `__int128`
- Layout 16/16 on both targets (§3); mangling `n` / `o`, also through the
  implicit `__int128_t` / `__uint128_t` typedefs (`tests/mangle/int128.cpp`).
- Calls: AArch64 passes and returns `i128` (an even general-register
  pair, AAPCS64 C.8); x86-64 coerces to `{ i64, i64 }`, two INTEGER
  eightbytes, on the stack when fewer than two registers remain.
  IR-exact on both triples (`tests/codegen/int128.cpp`,
  `tests/codegen/int128_ops.cpp`).

### 8.9 Landed (WP-075): closure types
- **Linkage** (`is_unnumbered_closure`, `has_internal_linkage`,
  `is_unique_external`, `type_externally_visible`): a closure type with
  a mangling number (spec/sema-lambda.md §3.5) takes its context's linkage; one without has
  no linkage, its members are internal, and a type built from it has no
  linkage. A namespace-scope variable whose type, or a function whose
  parameter types, have no linkage is unique-external (clang
  `getLVForNamespaceScopeDecl`): emitted `internal`, keeping its name.
- **Mangling** (`mangle.rs`, Itanium §5.1.8): a numbered closure is
  `<closure-type-name> ::= Ul <lambda-sig> E [ <number> ] _` (`Ul..E_`
  for number 1, `Ul..E0_` for 2, ...), its signature the call operator's
  parameter types (`v` for none; a generic lambda's invented parameters
  as `T_`-style template parameters at the lambda's level). An unnumbered
  one mangles as an unnamed class, `$_N`, from the module's
  `AnonIds` table (clang `getAnonymousStructId`: per function for a
  class in a function, else one module-wide counter), numbered in
  first-mangled order: codegen names an internal-linkage definition when
  it is deferred, as clang `EmitGlobal` does, so a closure in a later
  template argument of an earlier internal function takes its `$_N`
  before the closures in that function's body (WP-802,
  `expr.prim.lambda.closure/internal-closure-name-numbering-run.cpp`).
  A closure created
  in a default argument of a function's parameter mangles in that
  function's local scope with `d [<number>] _` (the parameter counted
  from the last, clang `mangleLocalName`); one created in the initializer
  of a variable or data member has `<closure-prefix> ::= [ <prefix> ]
  <unqualified-name> M` (a substitution candidate), or `<template-prefix>
  <template-args> M` for a variable template specialization (clang
  `mangleClosurePrefix`; WP-182: `_ZNK2vtIiEMUliE_clEi`). `decltype(auto)` is
  `Dc`; an undeduced `auto` / `decltype(auto)` (`Da` / `Dc`) is not a
  substitution candidate (clang `isTypeSubstitutable`; WP-203: a nested
  generic lambda's return is `Da`, not a back-reference).
  `tests/codegen/lambda_names.cpp` and
  `tests/codegen/lambda_default_arg_template.cpp` check it IR-exact
  (symbols included) against clang.

### 8.10 Landed (WP-131): local entities of template instantiations
- **Linkage** (`fn_shares_local_linkage`): a local class, and hence its
  members, takes its function's linkage when that function is inline
  *or a template instantiation*. This covers a function template
  specialization (implicitly or explicitly instantiated) and a member of
  a class template specialization that is not explicitly specialized
  (clang `getLVForLocalDecl`: `isInlined() ||
  isTemplateInstantiation(TSK)`). Otherwise the local class has no
  linkage and its members are internal. The function's linkage counts
  its template arguments (WP-801): a local class or closure type of
  `f<(anonymous)::A>`, a unique-external specialization, is internal
  like `f<A>` itself, as are its members and static locals; they are
  never `linkonce_odr`, which would let the linker merge two
  translation units' distinct lambdas.
- **Discriminator** (`AnonIds::local_tag`): `Z <encoding> E <entity>
  [<discriminator>]`. A named class or enumeration declared directly in
  a function, after the first one of the same name there, gets `_0`,
  `_1`, ..., `__10_` (Itanium §5.1.6; clang's mangling number, counted
  per name in declaration order). A member of such a class carries its
  class's discriminator after the whole nested name
  (`_ZZ1fvEN1M1gE_0v`). The table is built in one pass over the
  declarations, on the first use. clang numbers *internal* local
  classes in first-mangle order instead; kiln uses declaration order
  for those too, which differs only when a later same-named class is
  mangled first. `tests/codegen/local_class_template.cpp` checks this.

### 8.11 Landed (WP-140): inline-asm target rules (`asm.rs`)
Per-target (aarch64, x86_64) data and checks behind clang's `TargetInfo`
asm hooks, shared by sema and codegen: `ConstraintInfo` and
`validate_output_constraint` / `validate_input_constraint` (alternatives,
`+`/`=`/`&`, tied numbers and `[name]`s, immediate classes),
`validate_constraint_modifier` (the aarch64 `w`/`x` width warning),
register and clobber names (`is_valid_register_name`, `is_valid_clobber`,
`normalized_register_name`), `global_register_size_mismatch` (clang's
`validateGlobalRegisterVariable` for GNU global register variables:
`None` when unsuitable, else whether the size differs; WP-327, spec/sema.md
[dcl.stc]), `machine_clobbers`, `simplify_constraint`
(clang's `SimplifyConstraint`), and the template parser
`analyze_asm_string` (pieces: text, operand with modifier; clang's
errors) with `generate_asm_string` producing LLVM's `$`-syntax as bytes.
Not implemented: other targets, immediate range validation of operand
sizes.

### 8.12 Landed (WP-171): `va_arg` classification
`call::classify_va_arg(tys, q) -> VaArgInfo { kind, needed_int,
needed_sse }`: on aarch64 the parameter classification (`aarch64_arg`;
the register counts are unused), on x86_64 `x86_arg(q, 0)` with its
GPR/SSE counts (both zero means the argument is read from memory).
On aarch64 the register-offset and stack round-up of `va_arg` use the
unadjusted alignment, as the classification does (WP-583).

### 8.13 Landed (WP-176): register passing and call-order details
- *`can_pass_in_registers`* (`call.rs`, clang `canPassInRegisters` under
  the Itanium C++ ABI): a class is passed indirectly when any copy or
  move constructor or the destructor is non-trivial and not deleted, or
  when every copy/move constructor is deleted. There is no "a trivial
  copy constructor suffices" shortcut (that is the MSVC x64 rule), so a
  class with a trivial copy constructor and a user-provided move
  constructor goes in memory. Golden: `tests/codegen/class_nontrivial_move_abi.cpp`.
  Only eligible members count ([special]/6, WP-1109): a copy/move
  constructor that is deleted or `not_selected` (constraints unsatisfied,
  or less constrained than one of the same signature; spec/sema-class.md
  §4.7.2) is skipped, as clang skips `isDeleted() ||
  isIneligibleOrNotSelected()`, so `K(const K&) requires B = default;
  K(const K&) {}` goes in registers for `B` true. Golden:
  `tests/codegen/abi_matrix_eligible.cpp`.
- *Call IR order* (codegen): a call whose aggregate result is ignored
  gets no destination, so its `sret`/coerce temporary is made after the
  arguments' `agg.tmp`s (clang `AggExprEmitter::withReturnValueSlot`);
  a virtual call loads its callee from the vtable after the arguments
  are evaluated (`Callee::Virtual`, resolved in `call_vals_to`).
- *`typeid` constants*: a `const std::type_info*` or `&typeid(T)` in a
  constant initializer folds to `PtrBase::TypeInfo(T)` (oven `freeze`/
  `thaw`) and is emitted as the RTTI global `_ZTI...` (`consts.rs`).
  Golden: `tests/codegen/typeid_constant.cpp`.
- *Aggregate conditional*: `c ? a : b` of class type emits each arm
  into the destination slot (clang `AggExprEmitter::
  VisitAbstractConditionalOperator`; a constant condition emits one arm).
- *Reference members in aggregate init*: the initializer is bound
  (`emit_ref_binding`) and its address stored into the member's slot
  (`init_field_lvalue` uses `field_lvalue_init`, clang
  `EmitLValueForFieldInitialization`), never stored through the member.

### 8.14 Landed (WP-117): template mangling
- **Members of class template specializations** defined out of line
  mangle with the specialization's arguments (`N <template-prefix>
  <template-args> <name> E`). Templated definitions (patterns, partial
  specializations, dependent friends) are never emitted
  (`ModuleCx::is_templated`). Implicitly instantiated static data
  members are `linkonce_odr` in a comdat. `tests/mangle/out_of_line.cpp`.
- **Class-type NTTPs**: `Xtl <type> <fields> E` (clang's trimming of
  trailing zero members), `so` subobject paths for pointers or references
  at an offset, `di` designators for unions, and `_ZTA` template
  parameter objects. `tests/mangle/class_nttp.cpp`.
- **`<template-param-decl>`** (clang 18+, cxx-abi #47): `Tn <type>` /
  `Tp` before the argument of an overloadable function template's
  non-type parameter whose type is dependent or undeduced. `Dc` is
  `decltype(auto)`. Variable template specializations are `_Z1vIiE`.
- **Dependent template-specialization types** in signatures
  (`dependent_spec`): the template's unscoped name (`St`-prefixed in
  `std`), or `N <template-prefix> <template-args> E` when it is nested,
  then the arguments, e.g. `St16initializer_listIT_E` and `N1N1WIT_EE`.
  A template template parameter is `T_` (a substitution candidate,
  `Sub::TemplateParm`, the same one used when it is a template argument)
  followed by its arguments. `tests/mangle/dependent_spec.cpp`.
- Constrained parameters, requires-clauses and `Dk` placeholders: §8.16.
- **Reflection arguments** (`std::meta::info` values): by default
  (`-fkiln-reflection-mangling=gcc`, WP-909) GCC 16.1's
  `L Dm <kind> <entity> E`, where the info type itself is `Dm`, the same
  in every TU with the reflectee's linkage and link-compatible with
  g++-16; under `-fkiln-reflection-mangling=kiln` the vendor literal
  `L u6__info <kind> <entity> E` (the same kind letters and entity
  encodings). The grammar, the per-kind table and the
  linkage rule are spec/reflection.md §8.3; `spec/landed/abi/WP-501.md`
  (superseding the TU-local `L u6__info <id> E` of WP-485).
- **Known divergences from GCC 16.1** (not reflection-specific, found by
  WP-501's `gcc_link_compat`; kiln follows clang here):
  - GCC writes `L` before the members of an anonymous namespace
    (`N12_GLOBAL__N_1L5anonvE`);
  - an empty non-type pack argument is `JE` in GCC but `TpTnDmJE`-style in
    clang and kiln (`_Z2pkIJEEiv` vs the clang form);
  - an `sr` whose qualifier is a template-id closes one `E` earlier in GCC
    and counts substitutions differently (`…EE1cfp_ES1_` vs `…EEE1cfp_ES0_`);
  - GCC gives template static data members `STB_GNU_UNIQUE` binding (`nm`
    letter `u`; kiln emits a weak `V`).

### 8.15 Landed (WP-172): abi tags and dependent signature types
**abi_tag.** `[[gnu::abi_tag("a","b")]]` / `__attribute__((abi_tag(...)))`
is an `AttrKind::ABI_TAG` node whose `a` is the interned symbol of the
sorted, de-duplicated tags joined by `,` (a non-string argument is
`parse_attribute_requires_string`; an empty list attaches nothing).
`mangle::abi_tags(ast, d)` finds the tags of a declaration: any redecl
carrying the attribute, else the template pattern (`template_info`,
`spec_info` of the primary) or the instantiation source
(`instantiated_from`). For an instantiated *function* member of a class
template clang 19 looks only at the pattern's definition when one exists
(an out-of-line `template<class T> int Y<T>::f(){}` without a repeated
tag mangles untagged); static data members and member templates of
non-templates keep the declaration's tag. Since sema copies a pattern's
attributes onto its instantiations (WP-182), an instantiated function, a
non-explicit function specialization and the pattern of a member template
instantiated with its class never use their own attributes: their tags
come from the walk above (for the member template, from the original
member's definition: `X4<int>::X4<int>(int)` is untagged when the
out-of-line `template<class T> template<class U> X4<T>::X4(U){}` is). Tags mangle as `B<source-name>`
each, after the unqualified name (identifiers, `C2`/`D2`, operator codes),
and a tagged namespace-scope variable is mangled (`_Z1vB1x`) rather than
emitted under its plain name. Standard substitutions: §8.20; namespace tags and implicit tags
from return and variable types: §8.21.

**Dependent signature types** (`mangle/dep.rs`, clang `CXXNameMangler`):
a dependent template specialization is the unscoped template name plus
arguments at namespace scope or directly in `std`, else
`N <template-prefix> <args> E`; a template template parameter is `T_`
(candidate `Sub::TemplateTemplateParm`). `typename P::id` is
`[Ts|Tu|Te] N <prefix(P)> <id> E` and `P::template id<A>` is
`N <prefix(P)> <id> <args> E`, where the prefix of a nested-name-specifier
is nothing for `::`, the namespace prefix, the template prefix plus
arguments of a specialization (the specialization the candidate) or the
type, and for an identifier level `<prefix> <id>` (a candidate, clang
≥ 15); a dependent template name `P::template id` is itself a candidate.
A dependent unary transform is `u <spelling> I <base> E`
(`u20__remove_reference_tIT_E`). A value-dependent template argument
(an `Expr` or `Profile` argument, the profile's first expression) follows
`mangleExpression`: literals unwrapped (`Li1E`, `Lb1E`, `LDnE`), anything
else in `X ... E`; parentheses, implicit casts, `ConstantExpr` and
substituted parameters are transparent; a parameter reference is `T_`;
`DependentScopeDeclRefExpr` is an unresolved name (`sr` levels of the
qualifier as written, a template parameter or `decltype` level an
unresolved type with no closing `E`, a specialization level its source
name and its arguments as the qualifier writes them, without the
defaulted ones the canonical type adds, so `R<T>::value` for
`template <class, class = void> struct R` is `sr1RIT_EE5value` (WP-783),
else `E` after the levels); unary/binary operator,
`qu`, `st`/`sz`, `at`/`az` and `cv`/`sc`/`cc`/`rc`/`dc` encodings;
`sp <expr>` for a pack expansion, `sZ <param>` for `sizeof...`,
`u <trait> <type>* E` for a type trait, `cl <callee> <args> E` for a call
and an `UnresolvedLookupExpr` as an unresolved name (the parser records
the written qualifier of a qualified function template-id,
`set_template_id_qualifier`, so `std::declval<T>()` is
`clsr3stdE7declvalIT_EE`). A dependent `decltype(e)` is `Dt <expr> E` for
an id-expression or member access, else `DT <expr> E`. A builtin function a
dependent call names is `L_Z <source-name> E` (clang `mangleDeclRefExpr`;
WP-726: `DTclL_Z16__builtin_strlenEfp_EE`). An array whose bound is value-dependent
is `A <expr> _ <element>` (`const wchar_t (&)[N]` is `RAT__Kw`, `T[N + 1]`
is `AplT0_Li1E_T_`; WP-686, `tests/mangle/dependent_array.cpp`), never
the vendor placeholder, which would merge overloads on the element type. The arguments of
a dependent specialization of a known template group those of its
parameter pack into one `J ... E` (clang's converted list:
`NS_3tupIJDpT_EE`). A function parameter named in a `decltype` of the
signature is `fp [cv] [n-1] _` / `fL <L-1> p [cv] [n-1] _` (WP-298):
`Mangler::fn_depth` is clang's `FunctionTypeDepth`, one level for the
encoded function's bare function type and one per nested function type,
less one while that type's result is mangled, so `-> decltype(x + y)` is
`DTplfp_fp0_E`, `f(T x, decltype(x))` is `DtfL0p_E` and a parameter type
`void (*)(decltype(x))` gives `fL1p_` (`tests/mangle/decltype_function_param.cpp`).
The nesting `L` is that depth less the parameter's own function-scope
depth (WP-305; clang `mangleFunctionParam`: `FunctionTypeDepth -
getFunctionScopeDepth() - in-result`), so a parameter of a nested
function declarator named in that declarator's own `decltype`
(`void f(T a, auto (*p)(T b) -> decltype(a + b))`: `DTplfL0p_fp_E`) is
`fp`. The scope depth is the number of prototype scopes enclosing the
parameter's own ([basic.scope.param]), stored in the high byte of
`ParamData::flags` (`ParamData::scope_depth`, set by
`Sema::act_on_prototype_param` and `act_on_requires_param`, kept by
substitution) and part of the canonical profile of a parameter reference
in a `decltype` (`types/profile_expr.rs`), as clang profiles a
`ParmVarDecl` by depth and index: `decltype(a)` for an outer and an inner
`a` at the same index are different types. `fn_depth` is pushed, as
clang pushes `FunctionTypeDepth`:
- once around the encoded function's name (`Mangler::encoding`), so its
  template arguments, template-head requires-clause and a closure named
  in it see depth 1, and once more by the bare function type;
- once around a closure's `Ul <lambda-sig> E` parameters (a generic
  lambda's `decltype(x)` in an `operator()` name is `fL1p_`);
- for a trailing requires-clause, one level in its result (its
  parameters are the function's: depth 0 after re-entering its scope); a
  template-head clause uses the depth of its context;
- for a requires-expression with parameters, one level for `rQ <param
  types>` and one in result for its requirements; its parameters have
  scope depth = the enclosing prototype scopes plus enclosing
  requires-parameter clauses (`Sema::requires_param_scopes`, kept by
  `act_on_start_requires_params`/`act_on_end_requires_params`), and a
  requires-expression's body is no scope of its own.
A pattern parameter named in its own signature is substituted with the
signature (a nested declarator's parameter, in `Sema::unowned_params`,
or the function's own when only its type is substituted for deduction
against a target type): `Subst::nested_prototype_param` makes a fresh
parameter keeping index and scope depth, as clang's
`TransformFunctionTypeParams` does (`tests/mangle/function_param_depth.cpp`).
An expression with no encoding yet is an error, `cannot yet mangle
expression type <kind>` at its innermost unencodable subexpression, as in
clang, in a template argument, a `decltype` operand, a dependent array
bound (WP-593, `spec/landed/abi/WP-933.md`) and a requires-clause
(§8.16); the placeholder kiln writes into the name meanwhile (`XLi0EE`,
`u7unknown`) is never emitted. WP-818's rules (§8.29) cover the cases the
standard-library sweep found: non-dependent `sizeof`, substituted
`sizeof...` and enumeration parameters, dependent CTAD, alias templates
and template template parameters in qualifiers, the injected-class-name
in constraints, and abi_tag ordering. The
template-parameter-declaration prefixes `Tn`/`Tp` are §8.14's (WP-117);
the dependent template-specialization types there are the ones above
(`dependent_tst`, with a pack's arguments grouped by `converted_args`).
**First spelling per canonical type (WP-790).** clang keeps, in each
canonical dependent node (template specialization type, dependent
template specialization, `DependentDecltypeType`,
`DependentSizedArrayType`), the expressions of the spelling that first
created it, and mangles every later spelling of that canonical type
from them. kiln does the same: when `Canon` interns a new canonical
type of those kinds, `Types::note_arg_origin` records the written
`ArgListId` (only when it holds an expression argument) and
`Types::note_expr_origin` the written `ExprId`, keyed by the canonical
`TypeId`, first writer wins. The mangler reads them back
(`Types::arg_origin`, `Types::expr_origin`, falling back to the
profile's first expression) for the `compound` TST/DTST/decltype/array
cases and for `dep.rs`'s `type_prefix` and unresolved types. So the
choice is per enclosing canonical type, not per expression profile:
`R<T>::value` and `R<T, void>::value` profile alike, so
`EI<R<T>::value, T>` and `EI<R<T, void>::value, T>` are one canonical
type; if a signature writes the first, both mangle `RIT_EE`, while
`EI<R<T, void>::value, T *>` written first makes both `T *` forms
`RIT_vEE`, in the same TU, as clang 19 (kiln used to take the profile's
first expression in the TU for every enclosing type). Across
TUs the names agree whenever the TUs agree on the first spelling, which
is the same hazard clang has (`tests/codegen/dependent_expr_first_spelling.cpp`
for both orders in one TU, `tests/conformance/temp.over.link/dependent-expr-first-spelling-two-units-run.cpp`
for two linked TUs).
**Expression forms and member qualifiers (WP-1029).** `__extension__ e`
mangles as `e`, but makes a `decltype` operand `DT`. An imaginary literal
is `L <complex type> <zero real> _ <imaginary> E` (`LCd0000000000000000_4000000000000000E`).
A pack-index expression `p...[i]` writes nothing, and a pack-index type
is its pattern while dependent and its own substitution candidate. A
member access on an explicit object of dependent type keeps its
qualifier as spelled: the first level is its identifier with no
substitution (`t.T::f()` is `sr1TE1f`). Elsewhere a typedef level is its
own name (`sr1P4SelfE`), a namespace alias its alias name, a
substituted template parameter an unresolved type (`srS0_1m`) and a
class template specialization its template-id (`sr1BIiEE1f`). A
specialization the qualifier names without a written template-id (the
current instantiation, `C::` or `C<X>::` in a member of `C<int>`) is
clang's `Type::Record` case, the bare `sr1CE1f` (WP-1169,
`tests/mangle/current_instantiation_qualifier.cpp`). The
expression profile keeps those spellings apart (spec/types.md §7.2).
Details, before/after names and the sweep: `spec/landed/abi/WP-1029.md`.
**Pack indexes, lambdas, unresolved qualifiers (WP-1060).** A
`pack...[i]` that template substitution resolved still writes nothing:
clang 19 and 22 keep a non-dependent `PackIndexingExpr` in the
signature, so `Y<1, 2>::g` with `decltype(u + I...[1])` is `DTplfp_E`
(`TemplateTables::pack_index_origin` maps the selected element to its
pack-index expression). A lambda-expression in a dependent signature is
`L <closure type> E` (clang `LambdaExprClass`): the closure of
`decltype([]{ return 12; }())` in a template parameter type is
unnumbered, `$_N`. A lambda in a default template argument is a new
closure type at each use ([temp.param]/10, as clang), numbered then per
<lambda-sig> in the template's namespace or class (clang
`getCurrentMangleNumberContext` in an instantiation): two uses of
`template<class T = decltype([]{})> int f()` call `f<UlvE_>` and
`f<UlvE0_>`; the closure has its namespace's or class's linkage, none at
file scope. Non-member unresolved names (`gs`, `sr`, `srN`) were audited
against clang for global-, nested- and template-qualified names and need
no change. Details: `spec/landed/abi/WP-1060.md`.
**Substituted packs, default arguments, new initializers (WP-1214).**
A pack substituted while a class template is instantiated, into a
member template's signature that still expands the member's own pack
(`R<S, I>` in `tup<R<C, I>...>` of a member of `conn<idx<I...>>`), has
no Itanium production; clang 19 and 22 write the vendor token
`_SUBSTPACK_`: a non-type pack (`SubstNonTypeTemplateParmPackExpr`) is
`X_SUBSTPACK_E` as a template argument and no substitution candidate, a
type pack (`SubstTemplateTypeParmPackType`) a bare `_SUBSTPACK_` and a
candidate like any type (`tl<T, U>...` in a member of `ty<T...>` is
`2tlIJ_SUBSTPACK_T_EE`). A default argument (`CXXDefaultArgExpr`) or
default member initializer (`CXXDefaultInitExpr`) in a non-dependent
subexpression of a dependent operand mangles as the expression it stands
for (`h()` is `clL_Z1hiELi3EE` for `int h(int = 3)`). A braced list
mangles its elements as written (clang `mangleInitListElements` on the
syntactic form): the value- and default-member-initialized members the
semantic form fills in are dropped, `A{}` is `tl1AE`. A new-expression
with any initializer is `pi` (`il` when braced) with a constructor
call's arguments inlined, default arguments included, so a
default-initialized class with its implicit constructor call is `new B`
`nw_1BpiLi1ELi2EE` for `B(int = 1, int = 2)` and `new Q` `nw_1QpiE` for
an aggregate, while `new int` stays `nw_iE`; the expression profile
keeps the initialization style (`new B(3)` and `new B{3}` are not
equivalent). Tests: `tests/mangle/substituted_pack.cpp`,
`tests/mangle/default_arg_operand.cpp`,
`tests/codegen/mangle_substituted_pack.cpp`,
`tests/conformance/temp.over.link/substituted-pack-two-units-run.cpp`.
**Closures in function template signatures (WP-1077).** A lambda in
a function template's return type, parameter type or NTTP type is
substituted with the signature whenever deduction forms a
specialization (clang runs `DeduceTemplateArguments` in the pattern's
context): the closure is numbered per <lambda-sig> in the pattern,
shared across specializations, and mangles as the pattern's local
entity, `_ZZ3retT_ENKUlvE_clEv` for `ret<int>`, `UlvE0_` for the next
specialization. A repeat of an existing specialization numbers nothing.
A member's signature substituted while its class template is
instantiated is numbered in the class specialization
(`_ZNK1SIiEUlvE1_clEv`). Such a closure has its owner's linkage
(`closure_signature_fn`: the pattern's, through `tag_externally_visible`
and `has_internal_linkage`), so it and the specializations naming it are
`linkonce_odr`. The specialization's own name keeps the pattern's
unnumbered closure, `_Z3retIiEDTL3$_0EET_`, as clang. A call with a non-empty explicit
template argument list substitutes the signature, a trailing return type
included, once more in the explicit-argument step, so `ret2<int>`'s
closure is `UlvE0_` (WP-1086; the counting rule is spec/sema-lambda.md
§3.5). A member template's signature substituted during class
instantiation has a never-dependent closure (clang `LDK_NeverDependent`,
spec/sema-lambda.md §9, WP-1141): `decltype` of it is the closure type,
`_ZN1SIiE4mretIiEENS0_UlvE_ET_`, and a generic one's parameters keep
their depth, `NS0_UlTL0__E_E` (`tests/mangle/lambda_in_member_template_signature.cpp`).
One difference remains (xfail repro): clang resolves the address of an
explicit template-id more than once in some conversions and deduces
templates for an address that a non-template matches, which number more
closures than kiln (spec/sema-lambda.md §3.5). Details:
`spec/landed/abi/WP-1077.md`.

### 8.16 Landed (WP-204): constrained templates (`mangle/constraint.rs`)
clang 19's mangling of C++20 constraints (cxx-abi #24, #47), checked by
`tests/mangle/constraints.cpp` (86 symbols, both triples):
- **`<template-param-decl>`** of an overloadable function template's
  parameter when clang's `needToMangleTemplateParam` says so: `Ty` is
  never written alone; a type-constrained type parameter is `Tk
  <type-constraint>`; a template template parameter whose head (mangled
  with depths ignored, `Mangler::head_only`) differs from the argument
  template's head is `Tt <param-decls> [Q <constraint>] E`; `Tp` prefixes
  a pack. `Tn` is as in §8.14.
- **`<type-constraint>`**: the concept's unscoped name (a substitution
  candidate; `St`-prefixed in `std`) followed by `I <args> E` when it has
  explicit arguments, or `N <prefix> <name> [I <args> E] E` when the
  concept is nested in a class or namespace other than `std`.
- **requires-clauses** `Q <constraint-expression>`: the template-head
  clause inside `I ... E` after the arguments, the trailing clause after
  the bare function type (from the pattern of a specialization, or from
  the uninstantiated member of an instantiated class template).
  Template parameters inside a constraint keep their depth
  (`TL<d-1>_<i-1>_`, `Mangler::real_depth`). That includes the clause of
  a template template parameter's head (`Tt ... Q <constraint> E`,
  `template_param_list`): the head's own parameters are one level below
  the template declaring the `Tt` parameter (`TL1__` for a member
  template of a class template), an earlier parameter of the declaring
  head and an enclosing class template's parameters keep their own
  depth (`TL0__T_`), and a `Tt` nested in that head goes one level
  deeper again; no depth is rebased to the encoding being mangled, as
  in clang (checked against clang 19 by `tests/mangle/tt_requires_depth.cpp`,
  WP-314). A generic lambda's explicit template-head
  (`Ul <template-param-decl>* <lambda-sig> E`) is not yet mangled
  (§8.16 "Not implemented").
- **Constraint expressions** (`dep.rs`): a concept-id is an unresolved
  name (`[sr <nns> E] <source-name> I <args> E`, packs flattened, the
  written qualifier kept by `Sema::set_template_id_qualifier`); folds are
  `fl`/`fr`/`fL`/`fR` + operator code (a right fold iff the left operand
  holds the unexpanded pack); requires-expressions are `rQ <param-types>
  _ <requirements> E` / `rq <requirements> E` with `X <expr> [N] [R
  <type-constraint>]`, `T <type>` and `Q <expr>` requirements; parameter
  references are `fp [cv] [n-1] _` or `fL<nest-1>p...` (the nesting
  rule of §8.15: one level per enclosing `rQ`, less the parameter's
  scope depth). Member
  access (`dt`/`pt`), pseudo-destructors (`dn`), `ix`, `cv`/`tl`
  construction, `nw`/`na` new-expressions and the assignment operators
  (`aS`, `pL`, ...) are added for these.
- **Constrained placeholders** `Dk`/`DK <type-constraint>` (`C auto`,
  `C decltype(auto)`); never substitution candidates.
- **Member-like constrained friends** (a non-template friend with a
  trailing requires-clause, [temp.friend]/9) mangle in their lexical
  class with `F` before the name: `_ZN1XIiEF2frES0_Q1CIT_E`.
  Friend *templates* (clang `isMemberLikeConstrainedFriend`, WP-1146):
  - A friend template whose constraints name a parameter of the
    enclosing class template (sema's `friend_template_apart`, recorded
    as `TemplateTables::member_like_friend`, template → class
    specialization) gets `F` before its source name
    (`<unqualified-name> ::= F <source-name>`). From clang 20 on its
    template prefix is that class's (`_ZN1AIiEF2g2IiEEiT_PS0_Q1CIS2_E`);
    at `-fclang-compat=19` it has none (`_ZNF2g2IiEEiT_P1AIiEQ1CIS0_E`).
    Constrained only in its template-head, its specializations stay
    unscoped: `_ZF2g3IiQ1CIT_EEiS0_P1AIiE` under both.
  - A specialization instantiated from a friend declaration with a
    trailing requires-clause (the template's definition, else its latest
    declaration) takes a nested name even at namespace scope:
    `_ZN2n1IiEEiT_P1SQ1CIS0_E`, `_ZN2g1IiEEiT_P1AIiEQ1CITL0__E`. From
    clang 20 on the template's first declaration must be a friend too:
    `template<class U> int n3(U) requires C<U>;` at namespace scope,
    then defined as a friend, is `_Z2n3IiEiT_Q1CIS0_E` (clang 19:
    `_ZN2n3...`). `AnonIds::with_clang_compat` selects the version;
    `tests/conformance/temp.friend/p9-member-like-friend-template-mangling-*two-units-run.cpp`
    link against both releases' names, and `tests/mangle/constraints.cpp`
    checks clang 19's (the differential runs the mangler at 19).
- **Braced new-initializers (WP-231).** `new T{a, b}` is `nw _ <type>
  il <braced-expression>* E` (nested braced lists recurse as `il ... E`,
  packs as `sp`); an array new `new T[2]{...}` is `na _ <element type>
  il ... E` without the bound, as clang 19. A constraint expression kiln
  cannot mangle is the error `cg_cannot_mangle_expression` at the
  expression, and the TU produces no output (WP-818; it used to mangle
  as `Qu7unknown`, a name that differs from clang's).
  An expression splice (P2996) is GCC 16's `DS <expression>
  [<template-args>] E`.
- **Not implemented:** a generic lambda's explicit template-head in its
  closure type name (`[]<class T>(){}` is clang's `UlTyvE_`, kiln's
  `UlvE_`, which also shifts later discriminators).
- **Anonymous unions at namespace scope (WP-204).** The unnamed variable
  of a namespace-scope anonymous union ([class.union.anon]/2) is mangled
  with the source name of the union's first named data member, found
  through nested anonymous records (clang `getFirstNamedDataMember`),
  and gets no internal-linkage `L` even though it is `static`:
  `static union { int ga; };` is `_Z2ga`. `tests/codegen/anon_union_global.cpp`.
- **Concept and variable template template parameters (P2841R7,
  WP-746).** Matched to clang 22 (no Itanium ABI wording yet; clang 22
  is the reference, compared exactly by
  `tests/conformance/temp.param/ir-concept-variable-ttp-mangling.cpp`):
  a concept or variable template argument is its template name like a
  class template argument (`_Z1fI3IntiEiT0_`, `_Z2pkIJ3IntS0_EEiv`); a
  use `C<T>` / `V<T>` in a signature or constraint is an unresolved name
  `<source-name> I <args> E` (`X1CIT0_EE`, `Q1CIT0_E`, folds
  `fraa2CsIT0_E`); a head that differs from the argument's is `Tt Tp Ty
  E` as for a type template template parameter (`_Z2fvITtTpTyE3IntEiv`).
  A type-constraint (`template <C T>`, `C auto`) naming a concept template
  parameter mangles as the nested name `N <owner> <parameter name> E`,
  the owner being the template whose head declares the parameter (the
  function template `TkN2ct1CEi`, the class template `TkN1X1CEi`, inside
  a namespace `TkNS_2ct1CEi`): `TemplateTables::ttp_owner` records each
  concept template parameter's templated declaration at
  `template_decl`, and `Mangler::parent` answers it. Forms clang 22
  rejects have no mangling choice beyond these rules and are not run.
### 8.17 Landed (WP-211): `_Complex` lowering and mangling
- **IR type.** `_Complex T` is `{ T, T }` (clang `ConvertType`). The layout
  is in §3: twice the size of `T`, aligned to `T`.
- **Argument passing.** The call classification follows clang, and
  `tests/codegen/complex_calls.cpp` checks it on both targets.
  - **AAPCS64:** a complex floating type is a homogeneous aggregate of
    two elements, passed as `[2 x T]` with `alignstack`. A complex
    integer type is coerced to integer registers like a two-member
    struct.
  - **x86-64 SysV:** `_Complex float` is `<2 x float>`; `_Complex double`
    is two SSE eightbytes. `_Complex long double` is `ComplexX87`: it is
    returned as `{ x86_fp80, x86_fp80 }` and passed in memory. Complex
    integers are classified as `Integer`.
- **Mangling** (Itanium `<type> ::= C <type>`): `Cf`, `Cd`, `Ce`, `Ci`,
  `Ch`, ..., including through pointers, references and template
  arguments. `tests/mangle/complex.cpp` checks it.
- **`va_arg`** (WP-237) reads `_Complex T` with the argument's
  classification: on AAPCS64 a complex floating type is an HFA of two
  (each part from its own 16-byte `v` register slot, copied into a
  contiguous temporary; 8 or 16 bytes on the stack), a complex integer
  one or two general-register slots; on x86-64 `_Complex float` is one
  SSE eightbyte, `_Complex double` two (reassembled from `fp_offset`
  slots 16 apart), `_Complex long double` comes from the overflow area
  and complex integers are INTEGER. `tests/codegen/va_arg_complex.cpp`.
- **Multiply and divide** follow clang's complex range (spec/codegen.md
  §7n): full (`__mul?c3` behind a NaN check, `__div?c3`; the library
  routines' names come from the element type: `s` float, `d` double,
  `x` x87, `t` binary128), basic (algebraic) or improved (Smith's
  division). Constant evaluation always computes the full-range result
  (spec/oven.md "Complex values"), as clang does.
- **Complex math builtins** (WP-237): `__builtin_cabs`, `__builtin_cexp`,
  ... (C23 7.3, the 57 `f`/plain/`l` forms libstdc++ `<complex>` calls)
  are declared with clang's types and lowered to a call of the library
  function under the same classification as any `_Complex` argument or
  result. A `const` one (`cproj`) whose argument or result is passed by
  address (x86-64 `_Complex long double`) is `memory(argmem: readwrite)`
  (clang `AddPotentialArgAccess`). `tests/codegen/complex_math_builtins.cpp`.
- **Deviations.** `-fcomplex-arithmetic=promoted` (clang widens a
  division's operands to the next type with twice the exponent range,
  warning when none exists) is rejected with "unsupported argument", and
  clang 22's `-ffp-model=fast`, which selects it, with
  `drv_unsupported_flag` (spec/driver.md "Floating-point flags");
  clang's `-Woverriding-option` warnings between complex-range flags are
  not issued. `_Complex _Float16` is supported (`ext.complex/complex-half.cpp`).
  `_Complex __bf16` (spelled directly or through a type-name, as
  libstdc++ `<complex>` writes `__complex__ decltype(0.0bf16)` under
  `__STDCPP_BFLOAT16_T__`) is accepted as GCC 14 accepts it, where clang
  rejects it ("'_Complex type-name' is invalid"; WP-365): a pair of
  `__bf16` (`{ bfloat, bfloat }`, size 4, align 2), passed like
  `_Complex _Float16` (AArch64 an HFA of two `bfloat`, x86-64
  `<2 x bfloat>` in one SSE register), mangled `Cu6__bf16` / `CDF16b`.
  Its `+ - * /` and compound assignments, like `_Complex _Float16`'s on
  x86-64 (clang `EmitPromotedComplexBinaryOperator`, the element types
  `f16_excess` names), are computed in `_Complex float` (`__mulsc3`,
  `__divsc3`; nested operands stay in `float`) and truncated once, which
  is what GCC 14 emits for `_Complex __bf16`
  (`ext.complex/complex-bf16.cpp`, `libstdcxx-complex-bf16.cpp`).
  `_Complex` of `_Float32`, `_Float64` or `_Float128` (GCC 14 accepts
  them; clang 19 has no `_FloatN` in C++; WP-365) is laid out, passed,
  computed and mangled as its ABI twin's `_Complex float` / `double` /
  `__float128` (x86-64: `<2 x float>`, two SSE eightbytes, memory for the
  32-byte pair; AArch64: an HFA of two; `__mulsc3`/`__muldc3`/`__multc3`
  and the `__div?c3` forms) with the mangling `CDF32_`/`CDF64_`/`CDF128_`;
  constant evaluation is that of the element type, and conversions follow
  the extended types' ranks (`ext.complex/complex-floatn.cpp`,
  `complex-floatn-run.cpp`, whose calls go through asm labels naming the
  GCC symbols; on AArch64 kiln and g++ 14 objects were also linked
  against each other in both directions). `__float128` is not diagnosed on
  AArch64, where clang 19 rejects it ("__float128 is not supported on
  this target"; `Target::has_float128` is false there but unread): kiln
  accepts it as binary128.

### 8.18 Landed (WP-361): P1467 extended floating-point types (WP-361)
The C++23 extended floating-point types (spec/types.md §5.1) are
classified, passed and laid out as their ABI twin
(`BuiltinType::abi_twin`; `call.rs` `view`): `_Float32` as `float`,
`_Float64` as `double`, `_Float128` as `__float128` (x86-64 SSE+SSEUP;
AArch64 an IEEE quad in a Q register, the same as `long double`), while
`_Float16` (`half`) and `__bf16` (`bfloat`) were already classified SSE /
HFA members. Their LLVM types are `float`, `double`, `fp128`, `half`
and `bfloat`, on both targets; a homogeneous aggregate of them is an
AArch64 HFA as its twin's is.

| type | x86-64 class | AArch64 | mangling |
|---|---|---|---|
| `_Float16` | SSE (`half`) | V reg / HFA | `DF16_` |
| `__bf16` | SSE (`bfloat`) | V reg / HFA | `DF16b` (x86-64), `u6__bf16` (AArch64, not a substitution candidate, as clang) |
| `_Float32` | SSE (`float`) | V reg / HFA | `DF32_` |
| `_Float64` | SSE (`double`) | V reg / HFA | `DF64_` |
| `_Float128` | SSE+SSEUP (`fp128`) | Q reg / HFA | `DF128_` |
| `_Float32x` (GCC, WP-380) | as `double` | as `double` | `DF32x` |
| `_Float64x` (GCC, WP-380) | as `long double` (X87, `x86_fp80`) | as `long double` (Q reg, `fp128`) | `DF64x` |

On x86-64 an SSE+SSEUP pair is coerced as clang's `GetByteVectorType`
(WP-365, `call.rs` `byte_vector`): a record that wraps a single element
(`isSingleElementStruct`, `single_element`: through one-element arrays,
nested records and empty bases, with no padding beyond the element) is
passed and returned as that element's type when it is `fp128` or a
vector, so `struct Q { _Float128 x; }` is `fp128` and `struct V { v4 v; }`
`<4 x float>`; any other SSE+SSEUP type as `<N x double>`. Tests:
`tests/codegen/fp128_wrapper_abi.cpp`,
`tests/conformance/basic.extended.fp/f128-wrapper-args-run.cpp`, and
`tests/mangle/extended_fp.cpp`, `tests/codegen/bfloat16_arith.cpp`,
`tests/conformance/basic.extended.fp/p1467-run.cpp` (calls on both CI
targets).

**Backend defect workaround (x86-64, LLVM 19; WP-386):** `llc-19`
(kiln's LLVM 19 backend, and clang 19's) widens `bfloat` to `float` and,
in FastISel (`-O0`), takes a `bfloat` call result from a callee defined in
the same module, passed straight on as another call's argument (`g(f())`),
for the already widened value: it runs `__truncsfbf2` on the raw bf16 bits,
so the callee receives garbage. External callees and the optimizing
selector are unaffected; LLVM 22 compiles the same IR correctly. kiln
spills instead of diagnosing (a diagnostic would reject valid code that
works under LLVM 22): when the target is x86-64, `optimize` is off and
`CodegenOptions::llvm_major < 22`, every `bfloat` call result is stored to
an entry-block temporary and loaded back (`expr.rs`, end of `call_with`),
and the backend extends a loaded `bfloat` correctly. The IR at LLVM 22, or
when optimizing, is unchanged (clang's). clang 19 itself still miscompiles
such programs, so `bf16-call-result-x86_64-run.cpp` (`requires:
target-x86_64`, `clang>=22`, `oracle: none`) checks kiln's output against
a fixed `stdout:`.

### 8.19 Landed (WP-363): module names

`mangle.rs`, clang 22's `mangleModuleName`; the contract is
spec/modules.md §7. A namespace-scope entity with linkage attached to a
named module (`Ast::module_attachment`: in a purview and not in a linkage
specification) gets `W<len><component>` per dotted name component before
its unqualified name; namespaces, closures and internal-linkage entities
get none, and partitions never appear. Each module-name prefix is a
substitution candidate (`Sub::Module`). Module-attached variables are
always mangled (never the plain C name), and a `const` variable attached to
a named module has module linkage, not internal (P2788).
`mangle_module_initializer` gives `_ZGI<module-name>[WP<partition>]`
(not emitted yet). Tested differentially against clang by
`crates/kiln_codegen/tests/mangle_vs_clang.rs` over
`tests/mangle/modules.cpp` and `tests/mangle/module_partition.cpp`
(target-neutral; no `check-x86_64` golden is needed).

### 8.20 Landed (WP-373): standard abbreviations, tagged inline namespaces
- **Standard abbreviations** (Itanium `<substitution>`, clang
  `mangleStandardSubstitution`; `Mangler::std_abbrev`): for an entity
  whose parent is `::std` itself (not a nested or inline namespace),
  the template `allocator` is `Sa` and `basic_string` is `Sb`, and the
  specializations `basic_string<char, char_traits<char>,
  allocator<char>>` is `Ss`, `basic_istream<char, char_traits<char>>`
  `Si`, `basic_ostream<...>` `So` and `basic_iostream<...>` `Sd`. They
  are tried before the substitution table and before `St`, wherever a
  prefix, template prefix, unscoped template, template name or class
  type is mangled; they are never candidates themselves (the
  specialization `SaIcE` is). `std::__cxx11::basic_string` stays
  spelled out. A template attached to a named module (and a
  `char_traits` / `allocator` argument attached to one) gets no
  abbreviation (clang `getOwningModuleForLinkage`; WP-389,
  `tests/mangle/std_abbrev_module.cpp`,
  `tests/mangle/std_abbrev_module_global.cpp`).
- **Tagged inline namespaces**: a namespace's own `abi_tag` is not
  mangled (`St7__cxx11`, not `St7__cxx11B5cxx11`); its tags are
  implied by its name.
### 8.21 Landed (WP-384): implicit abi tags, namespace tags
Checked symbol for symbol against clang 19 and 22 (`tests/mangle/
std_substitutions.cpp`, `std_cxx11_abi_tag.cpp`, `std_libcxx_spelling.cpp`,
`abi_tag_implicit.cpp`) and by linking libstdc++ programs
(`tests/conformance/temp.explicit/libstdcxx-locale-name-run.cpp`).
- Standard abbreviations: §8.20 (libc++'s `std::__1` spelling, which
  none applies to, in
  `tests/mangle/std_libcxx_spelling.cpp`).
- **Namespace tags.** A namespace's `abi_tag` is never written: it marks
  the namespace's tags as already used (clang `AbiTagState`), so they are
  not repeated as implicit tags of names inside it. Only the first
  declaration of the namespace counts (clang reads the canonical decl).
  Sema accepts the attribute only on a named inline namespace; on a
  non-inline or anonymous one it is dropped with
  `sema_abi_tag_namespace` ("'abi_tag' attribute on {non-inline|anonymous}
  namespace ignored", `-Wignored-attributes`;
  `tests/conformance/namespace.def/abi-tag-non-inline-namespace.cpp`).
  Every entity declared directly in libstdc++'s `inline namespace
  __cxx11 __attribute__((abi_tag("cxx11")))` takes that tag from it:
  `_ZNSt7__cxx1112basic_stringIcSt11char_traitsIcESaIcEEC2Ev`, with no
  `B5cxx11` on the namespace or the class.
- **Implicit tags** (clang `makeFunctionReturnTypeTags`,
  `makeVariableTypeTags`): the mangler tracks the tags *used* (written,
  or a namespace's) and *emitted* while mangling a name. For a function
  whose return type (dependent or not; through pointers, references,
  template arguments, function types) mentions tagged entities, it
  mangles the name and bare function type once with derived tags off
  (`Mangler::trial_tags`, restoring output and substitutions afterwards),
  then writes the return type's tags missing from that trial's used set
  as extra `B` tags of the function's own name, merged with its explicit
  tags and sorted: `_ZNKSt6locale4nameB5cxx11Ev`, `_Z4wrapB1xv`,
  `_ZN1M3getB1xEv`, `_Z1fB1xI1AENT_4typeEv`, `_ZN1YB1y1mB1xEv`. A
  tag the parameters already carry is not repeated. Variables do the same
  with their type (`_Z2gsB5cxx11`), and a namespace-scope variable whose
  type has tags is mangled rather than emitted under its C name
  (`is_unmangled`, `var_type_has_tags`). The extra tags go to the
  function's unqualified name only, also when it is an unscoped template
  name, but not to a member template's name inside a nested template
  prefix (`_ZN1SI1AE1hIS0_EENT_4typeEv`). Inside a local name the used
  set restarts from the tags the enclosing encoding emitted
  (`_ZZ5localB5cxx11vE1sB5cxx11`). Constructors, destructors and
  conversion functions have no return-type tags.

### 8.22 Landed (WP-389): internal types in signatures
- **Unique-external functions** (`is_unique_external`,
  `fn_type_externally_visible`, `args_externally_visible`; clang
  `getLVForNamespaceScopeDecl` / `getLVForClassMember` /
  `getLVForTemplateArgumentList`): a namespace-scope function or a
  member function is emitted `internal`, keeping its name, when its
  return type or a parameter type is not externally visible (names a
  type in an unnamed namespace, a local or unnamed class, directly or
  through a typedef, pointer, reference, function type or template
  argument). Inline functions and function templates are included. The
  return type counts only as written: a deduced `auto` /
  `decltype(auto)` return (`FnSpec::return_deduced`, set when sema
  deduces a placeholder) does not make the function internal, a
  trailing return type does. A specialization (function, class or
  variable) is also internal when one of its template arguments is not
  externally visible: a type, a template whose template has internal
  linkage, or a pointer / member-pointer value naming an internal
  entity (packs recurse). A class-type value is walked at any depth
  (WP-397, `value_externally_visible`, clang `getLVForValue`): every
  pointer, reference and member-pointer subobject of its frozen
  `ByteImage` (its `relocs`, which cover nested members, array
  elements, base subobjects and a union's active member) must be
  externally visible. A pointer's base is visible when it is a
  declaration or function that is not internal, a lifetime-extended
  temporary whose extending declaration is not internal, or the
  `type_info` of an externally visible type; a string literal or a
  constant-evaluation object is not. A pointer to a template parameter
  object follows that object's linkage. A template parameter object
  (`_ZTAX...`) is itself internal when its type or its value is not
  externally visible (`param_object_externally_visible`); codegen then
  emits it `internal` (clang 19 keeps its comdat, clang 22 drops it).
  A class specialization that is internal this way makes its
  members, vtable and RTTI internal too. The mangled name is unchanged
  (no `L` prefix: that follows the storage class only). Tests:
  `tests/codegen/linkage_internal_types.cpp`,
  `tests/codegen/linkage_internal_nttp.cpp` (IR diff against clang 19
  and 22).
- **Cost** (WP-1076): one linkage query (`type_externally_visible` and
  the functions above, `Vis` in `mangle.rs`) memoizes each canonical
  class, enumeration, function and member-pointer type it visits, so a
  type is walked once per query, not once per path through a template
  argument DAG (`Fooob<A8, A8, …>` nested nine deep is 10^9 paths; clang
  caches the linkage in the type). Tests:
  `tests/stress/linkage-type-dag-10x10.cpp`,
  `tests/stress/linkage-pack-dag-36x17.cpp`.

### 8.23 Landed (WP-386): clang 20+ call lowering at `-fclang-compat=22`
`IrTypes::with_clang_compat` carries the codegen compat level (the
driver's `-fclang-compat`, default 22 as the driver) into classification. At 22,
`aarch64_arg` follows clang 20+ (`AArch64ABIInfo::classifyArgumentType`,
GCC's convention): an empty class (`[class.prop]`, no data members, size
1) is no longer ignored-or-`i8` but goes down the generic small-aggregate
path, so it is passed as `i64` (`trunc` to the `i8` slot in the callee);
a variadic empty argument likewise. A zero-size struct (the GNU
`int a[0]` member) is still ignored, and an empty class *returned* is
still `void` (clang 22 kept `classifyReturnType`). x86-64 is unchanged
(an empty class is `Ignore` there under both). `-fclang-abi-compat=19`
in clang 22 does not restore the old `i8`, so kiln keys on the compat
level only. Below 22 the clang 19 lowering (`i8` / ignored) is kept.
The alignment `aarch64_arg` classifies by (even-pair `i128`, HFA slot
alignment) is the unadjusted one, spec/landed/abi/WP-583.md.
The IR-level companions (the data layout, `dead_on_return`, GEP `nuw`)
are spec/codegen.md §6a. Tests: `tests/conformance/expr.call/
ir-clang22-argument-passing.cpp` (exact against clang 22) and the
oracle-22 `ir_vs_clang` corpus, whose former empty-class `Abi`
deviations now match.
The full classification audit against clang 19 and 22 (empty classes
and `[[no_unique_address]]`, unnamed bit-fields, AArch64 pointer
aggregates, x86-64 ABI offset 8, `va_arg` temporaries) and the
differential matrix that gates it are spec/landed/abi/WP-766.md.

### 8.24 Landed (WP-423): unnamed-type discriminators
- **Numbering** (`AnonIds::unnamed_tag`, Itanium §5.1.8
  `<unnamed-type-name> ::= Ut [<nonnegative number>] _`): an unnamed
  class, union or enumeration with no typedef name for linkage, declared
  in a class or a function, gets its context's next number, counted in
  declaration order over every such type of the context (anonymous
  unions included, nested blocks of a function included; lambdas and
  typedef-named classes excluded) — clang's per-context mangling number
  keyed by the empty name. The first mangles `Ut_`, the second `Ut0_`,
  the k-th `Ut{k-2}_` (`_ZN1AUt0_E`, `_ZZ1hvENUt0_1fEv`). The table is
  built in one pass over the declarations on first use.
- **Linkage** (`tag_externally_visible`): only a namespace-scope unnamed
  class without a typedef name lacks linkage outright (clang
  `getLVForNamespaceScopeDecl` `hasNameForLinkage`); a member one takes
  its class's linkage and a local one its function's (§8.10), so the
  members of an unnamed local class of an inline function are
  `linkonce_odr` in a comdat. `tests/mangle/unnamed_types.cpp`,
  `tests/codegen/tbaa_unnamed_o1.cpp`.

### 8.25 Landed (WP-458): x86-64 `i128` call lowering at `-fclang-compat=22`
clang 20+ (`X86_64ABIInfo`, LLVM 20 "Pass `__int128` as `i128`") passes
and returns a value whose two eightbytes are INTEGER as `i128` when
`GetINTEGERTypeAtOffset` finds an `i128` at offset 0 of its IR type
(descending through struct and array elements; `integer_at` in
`kiln_abi/src/call.rs`): `__int128`, `unsigned __int128`,
`_BitInt(128)`, an enum with such an underlying type, and a class or
union whose memory type starts with one (`struct { __int128 v; }`,
`struct { _BitInt(65) v; }`, a one-element array, a bit-field of
`__int128`, a union with an `__int128` member). It takes both integer
registers, as `{ i64, i64 }` did, but a stack-passed or `byval` copy
now has `i128`'s 16-byte alignment (data layout `i128:128`), so a kiln
TU calling a clang 20+ function with an `__int128` spilled to the
stack reads the same slot. A bare `_BitInt(65..127)` (IR type `iN`,
not `i128`), two `long`s and `_Complex long` stay `{ i64, i64 }`. An
argument that finds no two free registers is passed in memory as
before (clang 19 already passes a scalar `__int128` there as `i128`).
`va_arg` classifies through the same `x86_arg`; its IR is unchanged
(the register-save-area read does not depend on the coerced type).
Below 20 (`-fclang-compat=19`) the clang 19 lowering (`{ i64, i64 }`,
two `i64` arguments) is kept. AArch64 already passes `i128`. Tests:
`tests/codegen/int128_abi.cpp` (both triples, oracle 19 and 22),
`tests/conformance/expr.call/ir-clang22-int128-x86-64.cpp` (exact
against clang 22); the oracle-22 x86-64 `Abi` deviations of
`int128`, `int128_ops`, `abi_decls`, `bit_int_arith`,
`atomic_lv_libcall`, `atomic_wide`, `builtins_bits`, `va_arg` and
`new_placement_nothrow` now match, and `atomic_builtins` /
`builtins_overflow` differ only in instructions (`Structure`, as on
AArch64).
### 8.26 Landed (WP-460): `[[clang::trivial_abi]]` and trivial-for-call
- *Trivial for calls* (clang `hasTrivial*ForCall`): the class bits
  `TRIVIAL_ABI` (attribute in effect) and `COPY_CTOR_FOR_CALL`,
  `MOVE_CTOR_FOR_CALL`, `DTOR_FOR_CALL` (a declared member that is
  trivial for calls only because every subobject's is) are set by sema
  at the class's `}` (spec/sema.md "The `trivial_abi` attribute").
  `can_pass_in_registers` (§8.13) reads "trivial" as trivial-for-call:
  a member is trivial, or the class has `TRIVIAL_ABI`, or the member is
  not user-provided and its kind's `*_FOR_CALL` bit is set. The "all
  copy/move constructors deleted → indirect" rule is unchanged. Layout,
  mangling and the coerced register types are those of any class of the
  same layout passed in registers.
- *Destroyed in callee* (`is_destroyed_in_callee`, clang
  `isParamDestroyedInCallee` under Itanium): a class passed in
  registers whose destructor is not trivial. Codegen: spec/codegen.md
  §7ac.
- Tests: `tests/codegen/trivial_abi_subobjects.cpp` (signatures of
  dropped/propagated cases, both oracles, both targets),
  `tests/conformance/dcl.attr/trivial-abi-attribute.cpp`
  (`__can_pass_in_regs`).

### 8.27 Landed (WP-462): block-scope `extern` names, shared and structor asm labels
- **Block-scope `extern` declarations.** A function declaration or an
  `extern` variable declaration at block scope names the entity of its
  innermost enclosing namespace ([basic.link]/6), so it mangles there,
  as clang's `mangleNameWithAbiTags` walks a local container out to its
  namespace for an entity with linkage (`mangle::local_extern_ns`, used
  by the context walk): `void f() { extern void g(int); }` declares
  `_Z1gi`, and in an instantiation of `template<class T> void f() {
  extern void g(int); extern int ev; }` the names are `_Z1gi` and `ev`,
  not `_ZZ1fIiEvvE1gi` / `_ZZ1fIiEvvE2ev`. A block-scope variable
  without `extern` stays a local entity (§8.10).
- **One label, one symbol.** An asm label replaces the mangled name of
  the entity it is on (spec/sema.md "GNU asm labels"), so declarations
  of different types whose labels are equal name one symbol: codegen
  makes one function or global for them (spec/codegen.md §7z), never a
  suffixed `@lbl.1`.
- **Structors and conversion functions** take a label as other member
  functions do. Every structor variant (C1/C2, D0/D1/D2) mangles as the
  label, so the first variant defined is the definition and the
  complete-object alias (C1 → C2, D1 → D2) is not emitted; a virtual
  destructor's vtable names the label in both destructor slots, as
  clang does. A class with virtual bases: the complete-object variant
  replaces the base-object definition (WP-492, spec/codegen.md §7z).

### Landed sections after WP-488: `spec/landed/abi/`
From WP-488 on, a work package's Landed section is not appended here as
another `### 8.NN`: it is its own file, `spec/landed/abi/WP-NNN.md`
(title line `# WP-NNN — <title>`, the same content as a subsection
above), so open pull requests never conflict on this chapter
(spec/README.md "Landed sections"). They are as normative as this file;
read that directory after the last numbered subsection above.

### 8.28 Landed (WP-803): `pass_object_size` and `overloadable`
A function declared `overloadable` is mangled even with C language
linkage (clang `shouldMangleCXXName`). A `pass_object_size(t)` parameter
appends the vendor qualifier `U17pass_object_size<t>`
(`U25pass_dynamic_object_size<t>`) after the parameter's type in the
`<bare-function-type>` (clang `mangleBareFunctionType`; `mangle.rs`
`bare_function_type`), e.g. `_Z5namedPKcU17pass_object_size0i`. The
hidden `size_t` arguments are not part of the mangled type.

### 8.29 Landed (WP-818): standard-library mangling parity and the sweep
`scripts/mangle-sweep.sh` (docs/VALIDATION.md "Mangling sweep") compiles
each TU in `tests/mangle-sweep/` with kiln++ and clang++-19, against
libc++ and libstdc++, with `-std=c++26 -O0 -S -emit-llvm -fno-exceptions
-w -U__cpp_explicit_this_parameter` (clang 19 leaves that macro undefined,
kiln defines it as clang 22 does, and libstdc++ selects overloads on it).
Both IRs then go through `opt-19 -passes=always-inline`: clang runs the
always-inliner itself even at -O0 and kiln leaves it to LLVM, so without
the pass the `always_inline` functions that survive differ. Every `_Z`
symbol defined, declared or referenced must appear in both;
internal-linkage closure names `$_N` are compared without their number.

The rules the sweep found are listed in `spec/landed/abi/WP-818.md`:
non-dependent `sizeof`/`alignof` as literals, substituted `sizeof...` and
enumeration parameters, dependent CTAD names (untagged, with the
expression profile keyed on the template name), alias templates and
template template parameter specializations in qualifiers, the
injected-class-name as a substitution in constraints, `J...E` grouping of
a template template parameter's pack, abominable function types, closure
numbering in pack expansions, abi_tag ordering for out-of-line members,
and the inheritance of `always_inline`/`noinline` across redeclarations.

The sweep went from 988 mismatches (libc++ 840, libstdc++ 148) to
17 (libc++ 16, libstdc++ 1). What remains is clang-only: calls clang 19 emits that kiln does
not, none of them a different name for the same entity.

- libc++ `__introsort<..., false>` (`sort` with `greater<>` and a
  projection): clang emits the dead arm of `_UseBitSetPartition ?
  __bitset_partition(...) : __partition_with_equals_on_right(...)`, an
  aggregate-valued conditional it does not fold at -O0, and with it the
  `__bitset_partition` helpers (`__populate_left_bitset`,
  `__swap_bitmap_pos`, `__libcpp_ctz`, `ranges::iter_swap`, ...). kiln
  folds the constant condition.
- libstdc++ `std::visit`: kiln folds `__as(v).valueless_by_exception()`
  for a never-valueless variant to `false` (a constant expression under
  P2280, which clang 19 does not implement), so it does not call
  `__variant::__as`.

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
