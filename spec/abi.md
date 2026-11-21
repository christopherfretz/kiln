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

**Typedef-carried alignment** (one rule, `kiln_ast` `layout_sugar.rs`,
WP-1567, WP-1547). Which sugar carries such an alignment is decided in
one place, `Ast::layout_desugar_once` and `Ast::layout_attributed_typedef`,
asked by this table's walk (`type_info`), by the Oven's `alignof`
(`typedef_attributed`: only when it holds does the Oven ask the ABI
instead of its canonical type table), by Sema's over-aligned array
element check and by codegen's known pointee alignment
(`typedef_align_attr`). The walk follows every sugar node and goes
through a cv-qualified array to its base and through an array to its
element (`alignof(const T)`, `alignof(T[2])` for an aligned array
typedef `T`). A type substituted for a template type parameter of a
function, variable or class template is read canonical: clang
instantiates them from the canonical converted arguments, so in
`f<D>()` with `typedef double D __attribute__((aligned(32)))`,
`alignof(U)`, a local `U` and a load through `U*` are 8-aligned, as in
clang 22 and GCC 16.1 (kiln keeps the first use's sugar in a function
specialization's arguments for diagnostics; without the rule the result
depended on that spelling). An alias template's substituted arguments
keep their sugar (`alignof(AL<D>)` is 32 with clang 22; canonical at
`-fclang-compat=19`, as clang 19 substitutes).

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
- every byte of a scalar is a value byte, `bool`, a `std::nullptr_t`
  that is not inside a class (the object itself, or an array of them),
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
- a `std::nullptr_t` member, or one in a member or base at any depth
  (GCC 16.1: its bytes hold no value, [conv.lval]/3.1 never reads them;
  `g++.dg/torture/builtin-clear-padding-2`, WP-1694);
- the byte of an empty class, and empty bases and empty
  `[[no_unique_address]]` members (a non-empty one is laid out as a base
  subobject);
- in a union, a bit that is padding in every member.
Bit-field bits number little-endian, the only byte order kiln targets.
An object over 16 MiB (`MAX_PADDING_MASK_BYTES`) gets `None`. Class
masks are memoized for each call.

### 3.4 Layout compatibility (`layout::compat`, WP-1472)

One computation of the common initial sequence ([class.mem.general]/23)
serves `__is_layout_compatible`, GCC's `__builtin_is_corresponding_member`
and `__builtin_is_pointer_interconvertible_with_class` (sema, the Oven
through `SemaBridge::member_layout`, codegen's folding bridge and its
run-time lowering). Callers lay the classes out first (`ensure_layout`).
- `layout_compatible(a, b)` ([basic.types.general]/11): the same type
  ignoring cv (array elements included); enums with the same underlying
  type; standard-layout unions whose members pair in any order (same
  type, same width, no `[[no_unique_address]]`); standard-layout structs
  whose common initial sequence covers every member of both.
- The sequence pairs the members of the class that declares the
  members (`base_with_fields`) in declaration order while both are
  layout-compatible, have the same bit-field width (or none), are both
  or neither `[[no_unique_address]]` (CWG2759), have the same alignment
  (`decl_max_align` raises a type's) and sit at the same offset (GCC
  16.1; the text alone continues past differently placed members of
  layout-compatible types of different alignment). An unnamed bit-field
  is a member and ends it on a mismatch. clang 19/22 exclude any
  `[[no_unique_address]]` member; kiln follows the text and GCC.
- `corresponding_member(s1, s2, m1, m2)` ([meta.member]/4): both classes
  standard-layout structs and `m1`/`m2` the same position in the
  sequence; a member of an anonymous struct (`IndirectField`) is looked
  up in the corresponding pair of anonymous structs (GCC). A member of
  an anonymous union is false (GCC 16.1 says "sorry").
- `pointer_interconvertible_member(s, m)` ([meta.member]/2,
  [basic.compound]/5): any member of a union; the first member (unnamed
  bit-fields skipped) of a standard-layout struct, a member of its base
  when that base declares the members; through anonymous members
  transitively.
- Run time (`corresponding_offsets`, `interconvertible_member_of_type`):
  codegen lowers pointer-interconvertible to `mp == 0` (Itanium data
  member pointers are byte offsets) when the class can have such a
  member of that type, else false; corresponding to `m1 == m2` and `m1`
  one of the sequence's offsets. Pointers to member functions are false.

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
- **Bounds.** Every walk is bounded. The following answer `TooComplex`
  and never panic:
  - a dynamic chain deeper than 4096 (`MAX_DYNAMIC_DEPTH`: the builders
    recurse once per level, as clang's do, on kiln's 1 GiB stack);
  - more than `MAX_SUBOBJECTS` base paths;
  - more than 2^26 builder steps in one query (a vtable group, a
    construction group or a VTT; `Classes::refuel`). Every recursive
    walk ticks once per call, and the final-overrider collection once
    per class visited and once per method an overrider replaces
    eagerly. The steps are linear in the depth of a single-inheritance
    chain (about 6 per level and class, WP-1503), so the budget bounds
    what multiplies: virtual-inheritance lattices, whose subobject and
    construction-group walks grow with the number of paths.
- **Per-class and per-method memos** (`Classes`, WP-1503; each a fact of
  its key alone, so the order of the queries changes no answer):
  - `all_overridden(m)`: the methods `m` overrides transitively (clang
    `ComputeAllOverriddenMethods`), deduplicated with a set;
  - `paths_to(derived, base)`: the `paths(derived)` entries of class
    `base`, through an index built once per class (the this- and
    return-adjustment searches no longer scan every subobject);
  - `find_nearest_overridden` answers from the first direct override when
    it is declared in the last primary base (the chain case) and from
    `all_overridden` otherwise; `add_methods` keeps its primary bases in a
    set beside the list.
  - The final-overrider map (`overriders.rs` `OvMap`) in which every
    method has one entry with one overrider (a single-inheritance chain,
    also below a virtual base merged into it; WP-1520) replaces only the
    entries of the methods an overrider directly overrides; an entry
    naming an overrider whose own entry was replaced later forwards to
    it, resolved once (`OvMap::materialize`) before a merge and before
    the map is read. With single entries this is what the eager walk
    writes. clang's eager walk over every transitively overridden method
    is quadratic in the depth; once a merge gives some method a second
    entry or overrider (`OvMap::multi`: a class reached along two paths)
    the map replaces eagerly.
  - Construction groups (`template.rs`, WP-1650). The construction
    group of class `B` laid out at `off` in layout class `L` depends on
    `L` only through `P = (off, L's offset of each virtual base of B)`:
    every layout-class offset the builders compute is `P[s] + c` for one
    symbol `s` (`Off`, which also carries its value; every decision and
    every hash key uses the value, so a build is unchanged). A
    construction build records the vcall, vbase and offset-to-top
    components and the address points as such forms, and each
    comparison between forms of two different symbols (a nearly empty
    virtual primary base at its derived class's offset or not, a
    `this` adjustment's base offset, an overrider at its slot's
    subobject) with its outcome. `Classes` keeps the last recorded group
    per class (at most one per class); a later construction group of
    the same class with the same virtual-ness whose `P` gives every
    recorded comparison the same outcome is that group with the forms
    evaluated at `P`, without final overriders or a walk. A form that
    names a virtual base outside `B`'s (not expected) disables the
    record. Complete groups record nothing. `check_depth` is answered
    once per class, and a class's subobject table is built once per
    session (`Classes::subobject_table`, which `VTableSession` shares).
    So a single-inheritance chain of depth n below a virtual base builds
    each class's group once (O(n) each) and evaluates the others: the
    cost of a construction group is its size, the j+1 address points
    of class `S_j`'s group (its primary chain shares one vptr; clang
    lists them all, and its builder is cubic over such a chain). The
    -O0 IR of the chain is O(n^2) by the ABI and is now what the time
    follows (a 300-deep chain with a virtual destructor, release: 7.2 s
    before, 0.85 s after; 92.9 G instructions before, 8.2 G after).
  None of this is stored on `ClassInfo`: a class without virtual
  functions has the same record as before.
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
- **RTTI names of unit-local types** (WP-1651): under libstdc++ the
  `_ZTS` string of a type local to the unit starts with GCC's `*`, so the
  library compares its `type_info` by address; the rule, its exceptions
  and the switch are spec/landed/abi/WP-1651.md.
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
x86-64 `f` (any x87 stack register) is refused in an `=` output; in a
`+` output it sets `ConstraintInfo::x87_stack`, which sema refuses from
`-fclang-compat=22` as clang 22 does (`invalid output constraint '+f' in
asm`, any alternative; clang 19 accepts it and llc may then abort; `t`
and `u` stay valid outputs, plain or read-write; WP-1543, tests
`dcl.asm/x87-*-x86_64.cpp`).
Not implemented: other targets, immediate range validation of operand
sizes.

### 8.12 Landed (WP-171): `va_arg` classification
`call::classify_va_arg(tys, q) -> VaArgInfo { kind, needed_int,
needed_sse }`: on aarch64 the parameter classification (`aarch64_arg`;
the register counts are unused), on x86_64 `x86_arg(q, 0, named =
false)` with its GPR/SSE counts (both zero means the argument is read
from memory; an unnamed vector wider than 128 bits always is, at every
AVX level, spec/landed/abi/WP-1418.md).
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
- **Dependent reflected template-ids** (WP-1250). A reflect-expression of
  a template-id that is still dependent in a signature (a decltype return
  type, a parameter type through a dependent template argument, a
  requires-clause) writes, in both modes:
  - a function template-id (`^^g<T>`, `^^ns::operator== <T>`,
    `^^S<T>::template m<int>`) and a variable template-id (`^^vt<T>`,
    `^^ns::nv<T>`) as `de <unresolved-name>`, with its qualifier (`sr`)
    and its template arguments: `…KILDmde1gIS0_EEE`,
    `…KILDmdesr2nsEoneqIS0_EEE`, `…KILDmdesr2nsE2nvIS0_EEE`;
  - an alias template-id (`^^AT<T>`) as `ta <alias> <args> _ <aliased
    type>`, not as the aliased type (`ty`), which is a different
    reflection: before WP-1250 `p(T, K<^^AT<T>>)` and
    `p(T, K<^^TT<T*>>)` had one symbol.

  Declarations that differ only in the reflected template-id's arguments
  or qualifier are distinct ([temp.over.link]/5), and across TUs a
  `linkonce_odr` comdat folds equal symbols silently, so a lossy
  spelling is a miscompile, not a diagnostic. The rule: in gcc mode kiln
  writes g++-16's spelling where that spelling is injective, and keeps
  its own otherwise (each such case is a GCC 16.1 ABI bug, listed below);
  in kiln mode the arguments are always kept. g++-16 equals kiln for
  `^^vt<T>` (`de2vtIS0_E`) and for an alias whose parameters all appear,
  deducibly ([temp.deduct.type]/8), in its defining-type-id: gcc mode
  then writes the aliased class specialization's arguments as GCC does
  (`ta2ATIPS0_E_2TTIS2_E`), which determine the alias's
  (`mangle::reflect::alias_args_recoverable`); kiln mode writes the
  alias's own (`ta2ATIS0_E_2TTIPS0_E`). Deduced forms are the type
  parameter, compounds of deduced parts, a class template
  specialization's type, template and bare non-type arguments, an array
  bound (`template <class T, int N> using Arr = TT<T[N]>` matches GCC's
  `ta3ArrIA3_iE_2TTIS0_E`) and a recoverable alias; anything else is
  conservatively not deduced.

  A parameter that appears only in a non-deduced context
  ([temp.deduct.type]/5: a nested-name-specifier, a decltype operand)
  is still recoverable from one specialization's spelling when its
  argument there is dependent: the qualifier then stays a dependent name
  and is written structurally ([temp.dep.type]), carrying the argument
  (WP-1260). So gcc mode also writes GCC's form when every alias
  parameter is deduced or, with a dependent argument (a non-empty pack
  whose elements are all dependent, for a pack), appears in a dependent
  qualifier (`typename RC<T>::type`, `typename RN<N>::type`,
  `RC<T>::template X<U>`) or a decltype (`decltype(T{})`,
  `decltype(sizeof(T))`) of the defining-type-id, through nested aliases
  and class specialization arguments
  (`mangle::reflect::alias_args_preserved`). `^^D<T>`, `template <class
  T> using D = typename RC<T>::type`, is g++-16's `ta1D_N2RCIS0_E4typeE`;
  `^^D2<T, int>` (`typename RC2<T, U>::type`) `ta2D2_N3RC2IS0_iE4typeE`;
  `^^DP<T, int>` (`TT2<typename RC<T>::type, U>`)
  `ta2DPIN2RCIS0_E4typeEiE_3TT2IS4_iE`; nested `^^D<AT<T>>`
  `ta2N1_N2RCI2TTIPS0_EE4typeE`; `^^DT<T>` (`decltype(T{})`)
  `ta2DT_DTtlS0_EE`. This is decided per specialization, so the two forms
  mix within one alias: a dependent qualifier makes the aliased type
  dependent, a resolved one makes it the resolved type, and a kiln form
  always has an argument list (`I…E`) that differs from the aliased
  type's, so no gcc-form symbol equals a kiln-form one.

  Otherwise an alias with a parameter that is not deduced writes its own
  arguments in both modes, dependent or not: one that does not appear
  (`template <class T, class U> using F = TT<T>`, `ta1FIS0_iE_…`;
  `f<^^F<int, long>>` is `…ta1FIilE_2TTIiE…`; `template <class T> using
  ND = int`, `^^ND<T>` is `ta2NDIS0_E_i`), one in a non-deduced context
  whose argument is not dependent (`D = typename RC<T>::type`, a
  remove_const: `f<^^D<const int>>` is `…ta1DIKiE_i…`, g++-16's `ta1D_i`
  is also `D<int>`'s; `^^DP<const int, T>` is `ta2DPIKiS0_E_…`), or an
  alias of a non-class type that does not name it (`using V = void`).
  The symbols are `spec/landed/abi/WP-1250.md` and
  `spec/landed/abi/WP-1260.md`; the tests are
  `tests/conformance/expr.reflect/template-id-signature-*-run.cpp`,
  `template-id-nondeduced-*-run.cpp` and the driver's
  `gcc_dependent_template_id_link` and `gcc_nondeduced_alias_link` (a
  kiln object linked with a g++-16 one).
- **Known divergences from GCC 16.1.** Reflection-specific, where GCC's
  spelling gives distinct declarations one symbol and g++-16 fails to
  assemble a TU holding both ("symbol … is already defined";
  `expr.reflect/template-id-signature-collide-run.cpp`, `xfail(gcc)`);
  kiln keeps the information and so does not link with g++-16 for these
  (WP-1250):
  - a dependent function template-id is `ft <name>` without its
    arguments (`^^g<T>` and `^^g<T*>` are both `ft1g`,
    `^^ns::operator== <T>` is `ft2nseq`, `^^Q::m<T>` is `ft1Q1m`);
  - a dependent qualified variable template-id drops its qualifier
    (`^^ns::nv<T>` and `^^nv<T>` are both `de2nvIS0_E`);
  - an alias template-id is its aliased type's (class specialization)
    arguments even when they do not determine the alias's (`^^F<T, int>`
    and `^^F<T, long>` are both `ta1FIS0_E_2TTIS0_E`; `^^D<int>` and
    `^^D<const int>` are both `ta1D_i`; non-dependent too; `^^ND<T>` and
    `^^ND<T*>` with `ND = int` are both `ta2ND_i`; `^^DP<int, T>` and
    `^^DP<const int, T>` are both `ta2DPIiS0_E_…`, a qualifier with a
    non-dependent argument resolving;
    `expr.reflect/template-id-nondeduced-collide-run.cpp`);
  - g++-16 rejects `^^S<T>::template m<int>` and `^^T::template m<int>`
    at substitution ("'m' was not declared", a rejects-valid), so those
    have no GCC symbol; kiln writes `desr1SIS0_EE1mIiE`.

  Also, beside a constrained function template's symbol g++-16 emits an
  alias without the constraint (`_Z2q1IiEvT_` for `_Z2q1IiQ…EEvT_`, its
  ABI-compat name; not reflection-specific); kiln emits only the
  constrained one, which is what GCC's callers reference.

  Not reflection-specific, found by WP-501's `gcc_link_compat`; kiln
  follows clang here:
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
`qu`, `st`/`sz`, `at`/`az`, `u11__alignof__` and `cv`/`sc`/`cc`/`rc`/`dc` encodings;
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
An expression clang has no encoding for either is clang's error at its
innermost unencodable subexpression, in a template argument, a `decltype`
operand, a dependent array bound (WP-593, `spec/landed/abi/WP-933.md`) and
a requires-clause (§8.16): `cannot yet mangle expression type <kind>`,
`?: operator with omitted middle operand cannot be mangled`, `cannot yet
mangle __datasizeof expression`; the placeholder kiln writes into the name
meanwhile (`XLi0EE`, `u7unknown`) is never emitted. Every other kind has
its encoding (WP-1736, "Every expression kind" below). WP-818's rules (§8.29) cover the cases the
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
`2tlIJ_SUBSTPACK_T_EE`). A template template parameter pack
(`SubstTemplateTemplateParmPack`, WP-1359) is the bare `_SUBSTPACK_` as
a template name, clang's `mangleType(TemplateName)` spelling (clang 19
and 22 crash before writing it): `TTs<Us>...` in a member of
`S<template<class> class... TTs>` is `Dp_SUBSTPACK_IT_E`, and
`W<TTs, Us>...` is
`Dp1WI_SUBSTPACK_T_E`. A default argument (`CXXDefaultArgExpr`) or
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
**`sizeof`, `alignof`, `__alignof__`, `__datasizeof` (WP-1226).** clang
19 and 22 (`mangleExpression`, `UnaryExprOrTypeTraitExprClass`) agree. A
non-instantiation-dependent operand is the result's integer literal,
typed as the integer type it is implicitly converted to, else `size_t`:
`alignof`/`__alignof__` of an expression naming a variable or data
member is that declaration's alignment (`decl_align`, `alignas`
included), `__datasizeof` a class's `dsize` (a POD's size), any other
type's size, and the preferred alignment is the ABI one on both
targets. A dependent `sizeof` is `st <type>` / `sz <expr>`, `alignof`
`at <type>` / `az <expr>`; GNU `__alignof__` is the vendor extended
expression `u11__alignof__ <type> E`, or `u11__alignof__ <template-arg>
E` for an expression (a non-primary one wrapped in `X ... E`), the form
since clang 12 (libstdc++'s `aligned_storage<sizeof(T)>::type` is
`N...15aligned_storageIXstT_EXu11__alignof__N...21__aligned_storage_msaIXstS1_EE6__typeEEEE4typeE`).
A dependent `__datasizeof` or `vec_step` has no encoding (clang's
"cannot yet mangle __datasizeof expression"; kiln's
`cg_cannot_mangle_trait`, at the operand as clang 22, where clang 19 gives
no location). Tests:
`tests/mangle/trait_operand.cpp`, `tests/codegen/mangle_alignof_operand.cpp`,
`tests/conformance/temp.over.link/alignof-operand-two-units-run.cpp`.
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

**Every expression kind (WP-1736).** `Mangler::expression_node` decides
each `ExprKind` (an exhaustive match; `docs/status/static-gaps.md`
GAP-041 lists the kinds that are errors). clang 22 is the oracle, and
clang 19 agrees on every form below:

- `sizeof...` of a function parameter pack is `sZ <function-param>`
  (`sZfp_`), of a template parameter pack `sZ <template-param>`, also
  when substitution has replaced it by its length
  (`Templates::size_of_pack_origin`). A pack bound to elements some of
  which are pack expansions (`Al<int, U...>` with `template <class... T>
  using Al = A<sizeof...(T)>`) is clang's partially substituted
  `SizeOfPackExpr`, `sP <template-arg>* E` (`XsPiDpT_EE`): substitution
  records the partial arguments on the expression it forms
  (`Templates::size_of_pack_partial`).
- A `decltype` or GNU `typeof` qualifier is an unresolved type even when
  it is not dependent, the type it denotes and no closing `E`:
  `&decltype(ll)::operator()<T>` is `adsrN2llMUlDpT_E_EonclIT_E`,
  `decltype(s)::g(t)` is `clsr1S1gfp_E` (a class there is its plain
  name, which LLVM's demangler does not read; GNU c++filt does).
- A rewritten comparison (`CXXRewrittenBinaryOperator`) is its written
  operator and operands, `X{} != X{}` is `netl1XEtlS0_E`, not the `!(a ==
  b)` it means.
- A designated initializer is `di <source-name>`, `dx <index>` or `dX
  <first> <last>` per designator, then the initializer
  (`ildi1afp_di1bLi2EE`).
- A function parameter pack of an enclosing class template expanded in a
  member template's pattern (`FunctionParmPackExpr`) is clang's vendor
  form `v110_SUBSTPACK <function-param>`.
- `co_await` / `co_yield` (never in a signature, [expr.await]/2) are
  clang's vendor forms `v18co_await` / `v18co_yield`, the unary
  `co_await` operator `aw`; `CXXStdInitializerListExpr` is its operand.
- A `decltype` whose operand is instantiation-dependent only through
  references to non-dependent locals or parameters of a template
  (`decltype(t + 1)` of an `int t`) is not dependent, as in clang
  (`computeDependence(DeclRefExpr *)`): its type, `i`, not `DTplfp_Li1EE`
  (`Sema::names_template_local`).
- Pack indexing writes nothing, as clang 22 (§ "Expression forms and
  member qualifiers"); the `sy` production GCC 16 uses is not clang's.
  An expression splice follows GCC 16.1, `DS <expression>
  [<template-args>] E`, and a reflection `L <info-type> <reflection> E`
  (clang 22 has no P2996 encoding).

The kinds clang reports as "cannot yet mangle expression type" (the
GNU and vendor builtins `__builtin_choose_expr`, `_Generic`,
`__builtin_offsetof`, `__func__`, `__builtin_LINE()` and the other
source locations, `__builtin_bit_cast`, `__array_rank`,
`__is_lvalue_expr`, `__builtin_shufflevector`,
`__builtin_convertvector`, vector element access, `__builtin_va_arg`,
the atomic builtins, statement expressions, compound literals, `#embed`)
are the same errors; the kinds that never reach a mangled operand
(initialization forms of a function body, recovery, transparent nodes)
are errors too, which clang asserts on. Tests:
`tests/mangle/expression_kinds.cpp` (with the `// demangles:` round
trip through llvm-cxxfilt / c++filt),
`tests/conformance/temp.over.link/expression-kinds-two-units-run.cpp`,
`crates/kiln_driver/tests/codegen.rs::unmangleable_kinds_are_clangs_errors`.

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
  WP-314). A generic lambda's explicit template-head is part of its
  closure name, `Ul <template-param-decl>* <lambda-sig> E`
  (`tests/mangle/lambda_signature.cpp`, WP-818).
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
- **Generic lambda template-heads.** A generic lambda's explicit
  template-head is in its closure type name as clang 22 writes it
  (`[]<class T>(){}` is `UlTyvE_`), and closures are numbered per
  signature (`tests/mangle/lambda_signature.cpp`, WP-818).
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
- **Without linkage** (WP-1651): one that is not externally visible (at
  namespace scope, in an unnamed namespace, local to a function whose
  local entities have no linkage) has no mangling number and is clang's
  `$_N` (`AnonIds::id`), not `Ut_`.
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

### 8.30 Landed (WP-1275): C names (`Ast::lang_c`)
A C translation unit (`Ast::lang_c`, set by the parser from
`ParseOptions::c`) has no mangling (clang `shouldMangleCXXName` is false
outside C++): every function and variable is its plain name, an
internal-linkage `static int s;` included (`s`, not `_ZL1s`); only an
`overloadable` function keeps its Itanium name. Every entity with external
linkage has C language linkage (`is_extern_c`; a local variable has none
unless it is a block-scope `extern`). A file-scope `const` object has
external linkage ([C17 6.2.2]/5; C++'s internal-`const` rule is off). A
static local is `<function's name>.<name>` (clang `getStaticDeclName`,
`main.n`), a repeated one uniqued by LLVM (`.1`). Tests:
`tests/c/linkage/symbol-names.c`, `tests/c/interop/`.

### 8.31 Landed (WP-1361): run-time `std::meta::info`
`std::meta::info` is classified, passed and laid out as `std::nullptr_t`,
its ABI twin (`BuiltinType::abi_twin` maps `MetaInfo` to `NullPtr`): size
8, alignment 8, LLVM type `ptr`, INTEGER class on x86-64 and a GPR on
AArch64, without `noundef`; a class holding one is classified as a class
holding a pointer. A run-time `info` always holds the null reflection
(spec/codegen.md "Run-time reflections").

### 8.32 Landed (WP-1739): AAPCS64 pure scalable types and SVE tuples
From `-fclang-compat=20` (`call::aarch64_pure_scalable`, clang 20+
`AArch64ABIInfo`):
- **Register counts.** Arguments are classified in order, counting the
  SIMD/FP registers (NSRN, at most 8) and SVE predicate registers (NPRN,
  at most 4) they take: a floating-point scalar, a vector, a
  fixed-length SVE data vector and a 64- or 128-bit illegal vector take
  one Z; an HFA/HVA takes one per member; `svbool_t`, `svcount_t` and a
  fixed-length predicate take one P; an SVE builtin takes its vector
  count. Arguments past the prototype's named ones are unnamed (clang
  `IsNamedArg`), as is every `va_arg` read.
- **Pure scalable type** (`pure_scalable`, `passAsPureScalableType`): a
  class (no union, passable in registers; empty bases and fields
  skipped) or nonempty array whose members are, at every depth,
  fixed-length SVE vectors and predicates, or an SVE tuple; at most 12
  parts; at least one. It is checked after the HFA/HVA rule, and a
  fixed-length SVE vector is no HFA/HVA base from clang 20.
- **Passing.** A named pure scalable argument whose Z and P counts both
  still fit goes as `ArgKind::CoerceAndExpand`: one IR argument per part,
  of its scalable type (`<vscale x 128/E x T>` for data, `<vscale x 16 x
  i1>` for a predicate); otherwise (unnamed, or out of registers) it is
  `Indirect` without `byval`. A return value is classified the same way
  with all registers free: one part is returned as itself, several as a
  packed literal struct of them, more than 8 Z or 4 P in memory (`sret`).
  An SVE tuple instead is `Direct` with its IR type, a struct of its
  vectors (`IrTypes::convert`; clang 19 used one wide scalable vector),
  flattened to one IR argument per vector.
- **Memory view.** The `coerce` struct is the class's IR type flattened
  (`flatten_ir`, clang `flattenType`: arrays expanded, `[N x i8]` padding
  kept whole and paired with no part). Codegen (spec/codegen.md §6)
  addresses each non-padding element with a struct GEP and converts with
  `llvm.vector.extract`/`insert` (a predicate through `<vscale x 2N x
  i8>`). Were the non-padding elements and the parts ever to differ in
  number, the class goes by reference.
- `-fclang-compat=19` keeps clang 19's lowering: such a class is an
  ordinary aggregate (an HVA when its vectors are 128-bit).
Tests: `tests/conformance/ext.arm.sve/ir-sve-pure-scalable-types.cpp`,
`ir-sve-tuples-by-value.cpp` (IR only, no SVE hardware on the test hosts),
`kiln_driver` `sve_pure_scalable_by_value`.

### 8.33 Landed (WP-1745): Swift calling conventions, x86 interrupt
`swiftcall` (`swiftcc`) and `swiftasynccall` (`swifttailcc`) bypass the
target ABI (clang `arrangeLLVMFunctionInfo` → `swiftcall::computeABIInfo`;
`kiln_abi` `swift.rs`), on both targets:
- **Classes.** One that cannot be passed in registers is `Indirect`
  without `byval` (the class's alignment). Otherwise it is lowered
  (`SwiftAggLowering`) into byte-range entries: the vptr, the
  non-virtual bases at their offsets and the fields (union members all
  at the start), down to scalars by `ConvertType` (`bool` stays `i1`);
  arrays per element, `_Complex` as its two parts, member pointers,
  bit-fields (the bytes they touch) and illegal integers (other than
  1, 8, 16, 32, 64, 128 bits) opaque, an `_Atomic` padding opaque. A
  vector is split into legal vectors (x86-64: more than 8 and at most 16
  bytes; AArch64: a power-of-2 element count in 8 bytes, or in 16 with
  more than one element), else elements; a value not at a multiple of
  its natural alignment (store size rounded to a power of 2) is opaque
  (a vector is split first). Overlapping entries merge (equal types
  stay, integer beats pointer, else opaque). Then entries meeting in one
  8-byte chunk merge when neither is floating-point or a vector, and
  each run of opaque bytes becomes, per chunk, the smallest aligned
  power-of-2 integer covering it.
- **Passing.** No entries: `Ignore`. More than four registers (a
  pointer one, an integer one per 64 bits, a float or vector one):
  `Indirect` without `byval` (an `sret` return). Otherwise
  `ArgKind::CoerceAndExpand`: the entries as separate IR values, each
  stored to or loaded from the object viewed as the entries' struct with
  `[N x i8]` padding; a return of several is a non-packed literal struct
  (an SVE pure scalable return stays packed, §8.32).
- **Others.** A vector is lowered as a class of its own (so a Swift
  `<8 x float>` is two `<4 x float>`, `noundef` as its elements; a
  scalar's temporary is aligned to the larger of the entries' struct and
  the IR type's preferred alignment, a result's made before the
  arguments'). `_Complex` is returned `Direct` and passed as its two
  values (clang `Expand`; written as a `CoerceAndExpand` of the parts).
  `void` is `Ignore`; everything else is `Direct` with its IR type and
  no extension attribute. `min-legal-vector-width` counts the expanded
  parts.
- **`swiftasynccall` tail calls** (clang `EmitReturnStmt`): in a
  `swiftasynccall` function, a return whose operand (after its
  full-expression cleanups) is itself a call of a `swiftasynccall`
  function type (direct, through a pointer or reference, or a member
  call; not a member-pointer call, whose callee has the bound member
  function type) emits that call `musttail` followed by `ret`.
- **Mangling**: `U9swiftcall` / `U14swiftasynccall` (vendor qualifiers
  on the function type).
- **Swift parameter ABIs** (spec/sema.md §4.5; clang
  `ConstructAttributeList`'s `ParameterABI` switch, on a declaration and
  a call site alike): `swift_context` adds `swiftself`,
  `swift_async_context` `swiftasync`, `swift_error_result` `swifterror`;
  `swift_indirect_result` adds `noalias`, `sret(<pointer type>)` on the
  first one when the function returns `void` and has no `sret` of its
  own, and `dereferenceable(sizeof)` / `align` of a complete,
  constant-size pointee (replacing a reference's own). A
  `swift_error_result` argument (clang `EmitCall`) is passed as an
  `alloca swifterror ptr` temporary (`swifterror.temp`) that holds the
  error value loaded from the argument, stored back to it right after
  the call (not after a `musttail` call); the callee (clang
  `EmitFunctionProlog`) copies the incoming error value to a
  `swifterror.temp` of its own, which the parameter then points to, and
  copies it back at the end of the function on the normal path
  (`Cleanup::CopyBackSwiftError`, clang `CopyBackSwiftError`). clang 19
  and 22 agree. Mangling (clang `mangleExtParameterInfo`): inside a
  function type, a non-ordinary parameter is preceded by
  `U13swift_context`, `U19swift_async_context`, `U18swift_error_result`
  or `U21swift_indirect_result` (not a substitution candidate); a
  function's own name does not mention them.
- **x86 `interrupt`** (spec/sema.md §4.5): a definition gets
  `x86_intrcc`, its first parameter `byval(T)` of the pointee type (`i8`
  for `void`, an opaque `type {}` for an incomplete class) and
  `"disable-tail-calls"="true"`.
Tests: `tests/conformance/dcl.attr.cconv/ir-swiftcall-{x86_64,aarch64}.cpp`,
`swiftasync-musttail-{x86_64,aarch64}.cpp`, `swiftcall-diags-x86_64.cpp`,
`x86-interrupt-ir-x86_64.cpp`.

## Definition hash (`odr_def`)
`kiln_abi::odr_def` (WP-1582, package K-3b) is the serializer of a
function definition for the link-time ODR checks: record kind 2 of
`.mason.meta` (spec/codegen.md "Link-time metadata"; mortar `spec/odr.md`
O-10..O-13). This section is its contract: a later kiln reproduces these
bytes, or changes `ODR_DEF_SCHEME`. Codegen's kind 2 rows call it
(WP-1583, K-3c; spec/codegen.md §5.5 "Kind 2").

**Scheme.** `ODR_DEF_SCHEME = "kiln-odr-1"`, separate from the layout
serialization's scheme. Bump rule (O-11): every change to what is
serialized or how changes the constant; the golden test
`crates/kiln_codegen/tests/mason_def_hash.golden` pins the bytes and says
so when it fails. History: `d0` (K-3a) the class and declared type only,
every row `PARTIAL`; `d1` (K-3b) the serializer core: declarators,
statements, names, literals, operators, calls, casts; `d2` (K-3c) the
init area (below), ctor-initializers, synthesized members, a
destructor's deallocation function, variable definitions, and
`visibility` taken off the attribute list; `d3` (K-3d) closures and the
rows of closure members, local classes and enumerations, the GNU,
vector, atomic and coroutine forms, `asm`, constraints (below); `d4`
(K-3e) function template specializations by entity (the signature
area), the listed attributes, the declaration's default arguments,
`USES_INTERNAL`, and names without abi tags (below); `d5` (WP-1608,
WP-1626) a deferred `noexcept` from its pattern, dependent
requires-expressions and parenthesized lists, a definition's own
dependent `noexcept` and trailing requires-clause by structure, the
narrower overload test of the short form, and `target_clones`' clone
list; and (WP-1636..1638, mortar's probe of 11-17) an empty attribute
area that no longer depends on the rest of the unit, the `noreturn`
type bit left to the attribute record, and no `USES_INTERNAL` for an
externally visible closure under a function type. `kiln-odr-1`
(WP-1586, K-3f): `d5`'s serialization frozen and renamed, not one byte
changed (the golden digests are those of `d5`). Any later change to what
is serialized or how is `kiln-odr-2`. Objects written under a `d`
scheme (`kiln-odr-1.d0` .. `kiln-odr-1.d5`) are incomparable with
`kiln-odr-1` objects: the scheme names differ, so mortar treats such a
pair as incomparable (a miss, never a finding) even where `d5`'s bytes
are equal.

**Digest.** `DefHasher::function(def) -> DefDigest { hash, partial, uses_internal }`:
`hash` is the FNV-1a 128 (offset `0x6c62272e07bb014262b821756295c58d`,
prime 2^88 + 0x13b; `kiln_abi::odr::fnv1a_128`) of the stream; `partial`
is O-15's flag (some construct the scheme covers was skipped);
`uses_internal` is O-16's (below).

**Stream.** Every node opens with a one-byte tag and has a fixed layout
after it; a list is a little-endian `u32` count then its items, a string
a `u32` length then its bytes, an absent optional `0xA0`. The stream is
self-delimiting, so two different trees never give the same bytes.

| tag | item | layout after the tag |
|---|---|---|
| `0x01 + StmtKind` | a statement | per kind, below |
| `0x21 + ExprKind` | an expression | per kind, below |
| `0xA0` | absent optional | — |
| `0xA1` | a local (parameter, variable, binding, label) | `u32` index |
| `0xA2` | an external entity | FNV-1a 128 of its mangled name (16 bytes) |
| `0xA3` | a type | FNV-1a 128 of its `<type>` encoding (16 bytes) |
| `0xA4` | per-TU placeholder (O-2) | — |
| `0xA5` | value placeholder (X17) | expression tag, kind byte |
| `0xA6` | skipped construct (`partial`) | class byte (`e` `s` `d` `i`), code byte |
| `0xA7` | an enumerator | enumeration (`0xA3`/`0xA4`), name, `i128` value |
| `0xA8` | a non-static data member | class (`0xA3`/`0xA4`), name |
| `0xA9` | a local declaration | index, type, specifier bits, bindings, initializer |
| `0xAA` | the declarator | below |
| `0xAB` | a variable definition | declaration kind, type, thread-local kind, initialization style and initializer, or `0xA0` |
| `0xAC` | a ctor-initializer | `CtorInitKind` byte, target (base: a type item; member: `0xA8` item; delegating: nothing), initializer |
| `0xAD` | a synthesized member | — (after the declarator) |
| `0xAE` | a deallocation function | an entity item |
| `0xB0` | a closure digest | FNV-1a 128 of the closure's serialization (16 bytes) |
| `0xB1` | a captured entity, inside its closure | `u32` capture position |
| `0xB2` | a local class, enumeration or closure type | shape count, (type-kind, qualifiers) per pointer or reference level, qualifiers, depth byte, `u32` index; a closure then its digest |
| `0xB3` | a member of a local class or closure | the class's depth and index, operator byte or name, declaration kind, type |
| `0xB4` | a local class or enumeration definition | index, kind, bases, members |
| `0xB5` | a closure member's row | closure digest, then `0` or (`1`, function type, template arguments) |
| `0xB6` | a local of an enclosing scope inside a closure | depth byte, `u32` index |
| `0xB7` | a dependent type (signature area) | qualifiers, type kind, then its layout |
| `0xB8` | a template parameter | kind byte, `u32` depth, `u32` index, pack byte |
| `0xB9` | a function parameter by position | `u32` index, pack byte, scope depth, type |
| `0xBA` | a template by entity | its kind, scope path, name |
| `0xBB` | a function template specialization | FNV-1a 128 of its item (16 bytes) |
| `0xBC` | a nested-name-specifier | its canonical form |
| `0xBD` | the listed attributes | record count, sorted records (kind, arguments), the `indeterminate` parameter bits |
| `0xBE` | the declaration's default arguments | `u32` count, per parameter `0` (none), `1` and the expression, or `2` (a dependent default of an instantiation) |

Both kind enums are numbered stably (spec/ast.md); a new kind takes a new
number and the exhaustive dispatch (no wildcard arm) does not compile
until it is given a rule.

**Declarator.** `0xAA`, the declaration kind, specifier bits (storage
class; `inline` written or implied; `constexpr` only when written: never
on implicit or defaulted members or closure call operators, N25;
`consteval`; `virtual`, pure, `override`, `final`, `explicit`, deleted,
defaulted, explicit object parameter), the return type, the parameter
types (count + types), the function-type bits other than "written
trailing" (variadic, ref-qualifier, `this` qualifiers, calling
convention; not the `noreturn` bit, which `[[gnu::noreturn]]` and
`__attribute__((noreturn))` set on the type and `[[noreturn]]` does not:
the listed-attribute record carries all three, WP-1637), the EFFECTIVE `noexcept` as one byte, the
trailing requires-clause (an expression) or `0xA0`. Then the body
statement. Parameters are locals 0..n-1; parameter names and the
function's own name are not hashed. An implicit or defaulted member
(synthesized) writes `0xAD` after the declarator and its effective
`noexcept` byte is the fixed `2` (sema computes it only when a unit needs
it; it follows from the class and the members it calls). Between the
declarator and the body: a constructor writes its ctor-initializers
(count, then `0xAC` items in initialization order as sema completed them:
virtual bases once, direct bases, members from their mem-initializer,
default member initializer or the implicit rule, or the delegating
call); a destructor writes `0xAE` and the `operator delete` its deleting
destructor calls (sema's `dtor_delete`, recorded with every virtual
destructor's definition), or `0xA0`. A synthesized member is hashed as
the ctor-initializers and body sema built for it: they are a function
of the class definition (whose layout kind 3 covers) and of the special
members overload resolution selected for each subobject (hashed by
name), so two units that agree on the class agree on it; it is built
whenever the member is defined, so no row depends on how a unit used it.

**Variables.** `DefHasher::variable` (inline variables, static data
members of class templates, variable template specializations): `0xAB`,
the declaration kind, the declared type, the thread-local kind, a
listed attributes (below), then the initialization style and the
INITIALIZER as an expression from the declaration that holds it (the
in-class one of a member defined out of line), or `0xA0`. Never the
evaluated value (K9): constant and dynamic initialization of one
expression hash alike; `inline`, `constexpr`, `constinit` and `static`
are not hashed (an out-of-line definition need not repeat them, N22).

**Names and types.** A referenced function or variable is its mangled
name, memoized per first declaration (a function template
specialization: the signature area below); a type is its canonical type's
`<type>` encoding, memoized per canonical type (a typedef equals the type
it names). Both pass the structural not-TU-local test first (O-17,
`NotTuLocal`; and, for a function template specialization, a scan of the
encoding); a name or type that may hold a per-TU component (unnamed
namespace, local entity, unnamed type, closure) is `0xA4`, which is a
miss, not `partial`. The hasher mangles with its own `AnonIds`, never the
module's, built `without_abi_tags`: a named entity, type, variable or
enumeration contributes no abi tag, its own, implicit, or from an
enclosing class or inline namespace, and a global-scope variable
mangled only for its tags is its plain name (O-10 applied to
references: a library built with its hiding macro empty names the
untagged symbol where its users name the tagged one). Visibility and
`exclude_from_explicit_instantiation` never reach a name. Codegen's own
symbol names are reused only in a module without an `abi_tag`
attribute. An enumerator is (enumeration, name, value), so an unnamed
enumeration's enumerator is (`0xA4`, name, value). A local is its index in
order of first appearance (a `goto` before its label numbers the label).

**Statements.** A block: count and statements, a contract assertion and a
declaration statement holding only typedefs, aliases, using-declarations
and -directives, `static_assert`s or block-scope function declarations
are left out (they change nothing the definition does). Each other kind:
its tag, then its operands in source order (`if`: the `if` kind byte
(`if`, `constexpr`, `consteval`, `!consteval`), init, condition variable,
condition, then, else; loops: init, condition variable, condition,
increment, body; range-`for`: init, range variable, begin and end
variables, condition, increment, loop variable, body, so it differs from
the iterator loop written out; `case`: low, high, statement; labels and
`goto`: the label as a local). An attributed statement is its statement,
preceded, when it carries `[[assume(e)]]`, by its tag, the count and the
assumption expressions (no other statement attribute is hashed).
A local variable: `0xA9`, its index, type, bits (storage class,
thread-local kind, initialization style, `constexpr`, `constinit`), its
bindings (count + indexes) or `0xA0`, its initializer or `0xA0`.

**Expressions.** Transparent (their operand hashes in their place):
implicit conversions, `ConstantExpr` (never its cached value),
temporaries and cleanups, parentheses, a substituted template parameter
(its replacement), an opaque value (its source). Literals by value and
type, never by spelling: an integer as its type and value words, a
floating literal as its type and bits, a character as its type and code
point, a string as its kind, type, length and bytes, `true`/`false`,
`nullptr`. A literal token from a builtin macro (`__LINE__`, `__FILE__`,
`__COUNTER__`, ...) or from `#`/`##`, and any string literal concatenated
from pieces, is `0xA5`; so are `__func__`-like names and the
source-location builtins (X17, not `partial`). A name: its entity, local,
enumerator or member as above. A member access: a byte (`->`; qualified
name of a virtual member, which suppresses the virtual call), the member,
the object (an implicit `this` is `this`, so `m` and `this->m` are equal).
Operators: the operator byte and operands. Calls (`f(x)`, member calls,
overloaded operators): the operator byte, the callee subexpression (it
names the resolved function), the argument count and arguments. A
rewritten comparison: the written operator and the semantic form.
`sizeof`/`alignof`: the trait, then the type or the operand. A cast
written in the source: the cast kind sema chose, the value category, the
type, the operand.

**Init area** (`odr_def/init.rs`, K-3c). A construct expression: an
ELIDABLE copy or move (`Ast::elidable_copy_source`) is transparent, its
source in its place (`return T();` at C++14 equals C++17); else the
kind, the construction bits (complete, base, delegating or virtual base;
list; `std::initializer_list`; zeroing; never immediate-escalation), the
type, the constructor, the arguments. An inherited constructor call: the
construction kind, virtual bit, constructor. An initializer list and a
parenthesized aggregate initialization: the SEMANTIC form (type, the
union member initialized or `0xA0`, the elements in field order, so a
designated initializer is its resolved field and a skipped field its
value-initialization, the array filler). Value-initialization
(`ImplicitValueInitExpr`, `T()`): the type. `NoInitExpr`, the array
index: the tag. An array init loop, a designated-init update: both
operands. `std::initializer_list` construction and a compound literal:
the type and the operand. A default argument (`CXXDefaultArgExpr`) and a
default member initializer (`CXXDefaultInitExpr`): their expression
hashed AT THE USE (O-13 K1, K2). `new`: global and array bits, the
initialization style (`()`, `{}`, none), the allocated type, the
allocation and deallocation functions, the placement arguments (count or
`0xA0`), the array bound, the initializer. `delete`: global and array
bits, the deallocation function, the operand. `throw`: the operand or
`0xA0`. `noexcept(e)`: the operand only (its value reads exception
specifications computed lazily). `typeid`: type or expression form and
its operand. Type traits and expression traits: the trait, the value,
the types or operand; array traits: trait, type, value, dimension.

**Areas and states.** Every expression and statement kind is one of:

| area | state | kinds |
|---|---|---|
| transparent | hashed through | `ImplicitCastExpr` `ConstantExpr` `MaterializeTemporaryExpr` `CXXBindTemporaryExpr` `ExprWithCleanups` `ParenExpr` `SubstNonTypeTemplateParmExpr` `OpaqueValueExpr` `AttributedStmt` |
| stmt | hashed | `CompoundStmt` `DeclStmt` `ExprStmt` `NullStmt` `BreakStmt` `ContinueStmt` `IfStmt` `SwitchStmt` `CaseStmt` `DefaultStmt` `WhileStmt` `DoStmt` `ForStmt` `CXXForRangeStmt` `ReturnStmt` `LabelStmt` `GotoStmt` `IndirectGotoStmt` `CXXTryStmt` `CXXCatchStmt` |
| expr | hashed | `IntegerLiteral` `FloatingLiteral` `CharacterLiteral` `StringLiteral` `CXXBoolLiteralExpr` `CXXNullPtrLiteralExpr` `GNUNullExpr` `CXXThisExpr` `PredefinedExpr` `SourceLocExpr` `DeclRefExpr` `MemberExpr` `UnaryOperator` `BinaryOperator` `CompoundAssignOperator` `ConditionalOperator` `ArraySubscriptExpr` `CallExpr` `CXXMemberCallExpr` `CXXOperatorCallExpr` `CXXRewrittenBinaryOperator` `UnaryExprOrTypeTraitExpr` `CStyleCastExpr` `CXXFunctionalCastExpr` `CXXStaticCastExpr` `CXXConstCastExpr` `CXXReinterpretCastExpr` `CXXDynamicCastExpr` `BuiltinBitCastExpr` `CXXPseudoDestructorExpr` |
| excluded (X16) | not hashed, not `partial` | `ContractAssertStmt` |
| init (K-3c) | hashed | `InitListExpr` `ImplicitValueInitExpr` `CXXConstructExpr` `CXXTemporaryObjectExpr` `CXXInheritedCtorInitExpr` `CXXDefaultArgExpr` `CXXDefaultInitExpr` `CXXScalarValueInitExpr` `CXXParenListInitExpr` `ArrayInitLoopExpr` `ArrayInitIndexExpr` `CXXStdInitializerListExpr` `CompoundLiteralExpr` `NoInitExpr` `DesignatedInitUpdateExpr` `CXXNewExpr` `CXXDeleteExpr` `CXXThrowExpr` `CXXNoexceptExpr` `CXXTypeidExpr` `TypeTraitExpr` `ArrayTypeTraitExpr` `ExpressionTraitExpr`; constructors, destructors, implicit and defaulted members, variables |
| init (K-3c) | `partial` | an exception specification sema had not computed when the unit ended (not on synthesized members, whose `noexcept` is not hashed) |
| lambda (K-3d) | hashed | `LambdaExpr` `CoawaitExpr` `CoyieldExpr` `CoroutineBodyStmt` `CoreturnStmt`; local classes and enumerations; closure member rows |
| lambda (K-3d) | `partial` | a local class template, member template or alias template; a member of a local class the rule above does not name |
| misc (K-3d) | hashed | `ImaginaryLiteral` `BinaryConditionalOperator` `OffsetOfExpr` `StmtExpr` `AddrLabelExpr` `ChooseExpr` `VAArgExpr` `AtomicExpr` `ShuffleVectorExpr` `ConvertVectorExpr` `ExtVectorElementExpr` `PseudoObjectExpr` `GenericSelectionExpr` `GCCAsmStmt`; `RequiresExpr` `ConceptSpecializationExpr` outside a template |
| misc (K-3d) | `partial` | `UserDefinedLiteral` (sema builds calls: not met) `EmbedExpr` `SizeOfPackExpr` `PackIndexingExpr` `CXXReflectExpr` `CXXSpliceExpr` `RecoveryExpr` `CXXExpansionStmt`; a block under a floating-point `#pragma` |
| attrs (K-3e) | hashed | the listed attributes (below) on the function, a variable or a local; `[[assume]]`; the declaration's default arguments |
| attrs (K-3e) | `partial` | `target_clones` read from an AST that does not keep its clone list (none since `d5`) |
| signature (K-3e) | hashed | a function template specialization named in a definition; dependent types, template parameters, function parameters by position, nested-name-specifiers, dependent `noexcept` (a deferred one from its pattern, `d5`), the template's requires-clause, `RequiresExpr` and `ParenListExpr` (`d5`), inside a signature or a generic lambda's pattern; a definition's own dependent `noexcept` and trailing requires-clause (`d5`) |
| signature (K-3e) | `partial` | a specialization whose signature holds an uncovered node (a deferred `noexcept` with no pattern, an uncovered dependent kind) when its template has an overload that could share its short form (below; else the short form, not `partial`) |
| dependent (K-6) | `partial` | `UnresolvedLookupExpr` `UnresolvedMemberExpr` `DependentScopeDeclRefExpr` `CXXDependentScopeMemberExpr` `SubstNonTypeTemplateParmPackExpr` `FunctionParmPackExpr` `DesignatedInitExpr` `PackExpansionExpr` `CXXFoldExpr` `CXXUnresolvedConstructExpr` `DependentCoawaitExpr` `CXXExpansionSelectExpr` outside the signature area; a dependent `ChooseExpr`, `GenericSelectionExpr` or `CoyieldExpr` (in a generic lambda's pattern); a template's own definition |

**Signature area** (`odr_def/signature.rs`, K-3e; mortar's ruling of
11-17). A function template specialization named in a definition (a
callee, an address taken, a default argument, a member pointer) is
`0xBB` and the FNV-1a 128 of its item, memoized per specialization: the
item is `0xBB`, the declaration kind, the template (`0xBA`: kind, scope
path of namespaces by name and a non-templated class by its type item,
name), the canonical template arguments' `<template-args>` encoding
(non-dependent, so the spelling is the value's), then `1` and the
template's signature digest, or `0` (the short form). The signature
digest, memoized per template (first declaration, so a redeclaration
cannot change it), hashes the template parameter list (count; kind,
pack, a non-type parameter's type, the type-constraint; the
requires-clause), the pattern's canonical function type and its trailing
requires-clause, with every dependent type serialized by structure
(`0xB7`), template parameters by depth and index (`0xB8`), function
parameters by position (`0xB9`), nested-name-specifiers canonically
(`0xBC`), dependent expressions by their equivalence class's canonical
profile and a dependent `noexcept` by its expression: never a mangling,
whose dependent parts print the unit's first equivalent spelling. A
`noexcept` sema has not instantiated yet (`ExcSpec::Deferred(p)`, a
member template of a class template specialization) is hashed from
the pattern `p` it waits on (or, waiting on itself, the declaration it
is instantiated from): its dependent `noexcept(e)` as above, a known one
by value; the hasher instantiates nothing (`d5`). A dependent
requires-expression is its expression tag, its parameters' types, then per
requirement a byte (1 simple, 2 type, 3 compound, 4 nested), a
compound's `noexcept` bit, and its operands, parameters by position; a
dependent parenthesized list is its expressions (`d5`). A signature
holding a node the area does not cover (a deferred `noexcept` with no
pattern, a dependent kind the K-6 row lists) takes the short form; the
row is then `PARTIAL` only when the template's scope has another
function template of the same name that could share the short form
(`d5`; before, any of the same template parameter count): the same
template parameter kinds in order (type, constant by the class of its
type: integral or enumeration, floating, pointer or member pointer,
reference, class, or any for a dependent or placeholder type, which
matches every class; template), a pack on either side matching any
list, and a function parameter count range (fewest: the least over the
pattern's declarations, one less for a class member; most: unbounded
with a pack or `...`) that overlaps the callee's. Another kind sequence
gives other template arguments, so another short form; a range that
cannot take the call's argument count is never chosen for it. A
definition's own dependent `noexcept(e)` and trailing requires-clause
(a member of a class template specialization keeps its pattern's until
a unit needs them) are hashed the same way (`d5`). A member
function of a class template specialization and a variable template
specialization are named by their mangled names (their signature and
type are the instantiation's, with no dependent part); a member
template specialization takes the item, its scope path naming the
class specialization by its type item.

**Attributes** (`odr_def/attrs.rs`, K-3e). O-10's closed list
(scheme 1), as WRITTEN on the declaration chain (an instantiation: its
pattern's chain), never an implicit or inherited copy, as a set: each
record (kind, arguments) on its own, sorted and deduplicated; `0xA0`
when the chain has none, whatever the rest of the unit holds (before
WP-1636 a unit with a listed attribute anywhere wrote an empty record
list instead).

| attribute | hashed as | O-10 choice |
|---|---|---|
| `alignas(e)` / `aligned(e)` | `1`, the expression (dependent: by structure) | the spelling is not hashed: the argument decides |
| `alignas(T)` | `2`, the type | equal to `alignas(alignof(T))` only by value: differ (miss) |
| `aligned` (no argument) | `0` | the target's maximum, a token of its own |
| `noreturn` / `[[noreturn]]` | present | `[[noreturn]]` and `[[gnu::noreturn]]` equal |
| `target("s")`, `target_version("s")` | the string bytes | not normalized (`"avx2,sse4.2"` against `"sse4.2,avx2"`: differ) |
| `target_clones` | the clone list: the strings split at commas, trimmed, joined by `,` (`d5`) | `"a,b"` and `"a", "b"` equal; the order is hashed |
| `indeterminate` | a bit per parameter | any declaration of the chain writing it on the parameter |
| `[[assume(e)]]` | the expressions, at the statement | |
| everything else | not hashed | `visibility`, `exclude_from_explicit_instantiation`, `abi_tag`, `weak`, `section`, `deprecated`, `nodiscard`, `maybe_unused`, ... |

Legal differences: `[[nodiscard]]` and `[[deprecated]]` may differ
between declarations in different units ([dcl.attr.nodiscard],
[dcl.attr.deprecated] allow a redeclaration to add them) and are not
hashed; `[[noreturn]]` on the first declaration in one unit and not in
another is ill-formed, no diagnostic required ([dcl.attr.noreturn]), so it
is hashed, and a unit that adds it only on a later redeclaration differs
from one that never writes it (a miss mortar reports, not a false
match).

**Default arguments** (`odr_def/decl.rs`, K-3e). The rule: each
parameter hashes the default argument that a namespace- or class-scope
declaration of the function's redeclaration chain gives it, as
accumulated at the end of the unit, and for an instantiation the
pattern's default when it is not dependent (a dependent one is the
fixed token `2`: it is instantiated only where a unit uses it).
`0xA0` when no parameter has one, else `0xBE` (layout above). Hazards:
a default added by a later redeclaration is hashed (a unit that does not
see the redeclaration differs); a block-scope redeclaration's defaults
are skipped (they hold only in that block); default template arguments
are not hashed (they are part of the template's declaration, and a
callee's are in its template arguments); an uninstantiated default of a
class-template member is the token `2`, so a unit that used it equals
one that did not; a friend declaration's default (only on a definition,
[dcl.fct.default]) is hashed like any other.

**`USES_INTERNAL`** (`odr_def/internal.rs`, O-16, strict only). Set
during the walk when the definition odr-uses a function or variable
with internal linkage, or names a TU-local type (an unnamed-namespace
class or enumeration, a per-TU placeholder of a type whose TU-local
part the walk locates, or one it does not locate, such as a closure
under a function type, that is not externally visible:
`mangle::type_externally_visible`; a closure of an inline function is
the same entity in every unit, WP-1638), other than its own local
entities. Not set by a constant read by value (N17), an
unevaluated operand, or a discarded one (sema's non-odr-use mark). A
closure's memo carries the flag to every use. Inside a signature
digest nothing sets it (the signature names no definition's use). A
row with the flag still hashes its tokens: two units' rows may be
equal.

**Misses and exclusions** (WP-1586, K-3f; each row pinned by the test
named, so a change of behaviour fails it; the O-13 corpus case, when
there is one, is `tests/mason/corpus/<case>/`, spec/harness.md "The O-13
corpus"). A *split* is two equal definitions whose rows differ (a report
on equal meaning); a *pass* is two different definitions whose rows agree
(no report); *no row* means the symbol is not compared at all.

| # | what | effect | why | pinned by |
|---|---|---|---|---|
| 1 | an attribute (`noreturn`, `indeterminate`) written only on a later redeclaration the other unit does not see | split | hashed as written on the chain; `noreturn` missing from a first declaration in another unit is ill-formed, no diagnostic required | `mason_def_hash::mutations_move_the_digest` ("noreturn on a redeclaration", "indeterminate on a redeclaration") |
| 2 | a default argument added by a later redeclaration only one unit sees | split | the defaults accumulated at the end of the unit are hashed | `mason_def_hash::mutations_move_the_digest` ("default argument on a later redeclaration") |
| 3 | `alignas(T)` against `alignas(alignof(T))` | split | the type and the expression are hashed apart | `mason_def_hash::attribute_misses_split_equal_meaning` |
| 4 | `target` strings not normalized (`"avx2,sse4.2"`, `"sse4.2,avx2"`) | split | the string bytes are hashed | `mason_def_hash::attribute_misses_split_equal_meaning` |
| 5 | a short-form callee of a template with an overload that could share its short form | pass, the row `PARTIAL` | the callee is not fully named | `mason_def_hash::short_form_with_overloads_is_partial` |
| 6 | a dependent node of the K-6 kinds | pass, the row `PARTIAL` | a token per kind (mortar's ruling 2) | `mason_def_hash::partial_tokens_are_per_kind` |
| 7 | `noreturn` reaching a function only through a typedef'd function type | pass | no attribute record; the type bit is not hashed (WP-1637) | `mason_defs::noreturn_spellings_agree`; corpus `typedef-noreturn` |
| 8 | a synthesized member's effective exception specification (a `noexcept(false)` member destructor makes the implicit destructor potentially throwing) | pass | a synthesized member's declarator is the fixed byte `2`, and its callees are named by symbol only. WP-1586 remainder: hash a synthesized member's effective noexcept | corpus `effective-noexcept` |
| 9 | `__FILE__` and the source-location builtins inside one literal (`"at " __FILE__` against `"in " __FILE__`) | pass | the whole literal is the value placeholder (X17, mortar's ruling (b): must-equal across routes outranks detection) | corpus `file-provenance`; `mason_def_hash::golden_digests` ("placeholders") |
| 10 | contract assertions | pass | not hashed (X16) | `mason_def_hash::must_equal_pairs` ("contract assertion (X16)") |
| 11 | a reference to the enclosing scope's captures from inside a generic lambda specialization's own body | pass | the placeholder | `mason_defs::closure_rows` |
| 12 | a TU-local component (unnamed namespace, internal entity) | `USES_INTERNAL`, its part the per-TU placeholder (O-2) | not comparable across units | `mason_defs::uses_internal_across_units`; `mason_tu_local::named_cases` |
| 13 | `alignas` or `packed` on a class | carried by kind 3 (the layout), not kind 2 | layout is kind 3's | corpus `alignas-changed`, `packed-changed` |
| 14 | a lambda in a declarator, a default argument or a trailing requires-clause (X15) | no row | the closure type has no stable name across units | `mason_defs::units_and_definitions_without_rows`; corpus `lambda-in-declarator` |
| 15 | a C unit (N21) | no row | kind 2 is C++ only | `mason_defs::units_and_definitions_without_rows` |
| 16 | internal linkage, `weak` (N5), `gnu_inline` / `available_externally` (N7), an ordinary strong definition | no row | not a vague definition a linker merges | `mason_defs::definition_rows_golden` |
| 17 | an `alias` or `ifunc` symbol (the aliasee has the row), a coroutine's outlined parts, an `always_inline` inline function | no row | decided forms (O-14) | `mason_defs::decided_forms` |
| 18 | a closure's static invoker and conversion function; a local class's members | no row | their definitions are the enclosing function's (O-14, ruling 1) | `mason_defs::closure_rows` |

A skipped construct writes `0xA6`, its class and its kind's code in place
of its subtree (one token per node kind, mortar's ruling 2), so equal
source still gives equal digests.

**Not hashed** (each would make equal definitions differ between units):
the value cached on a `ConstantExpr` and a variable's evaluated value
(computed only when a unit needs them); odr-use marks and the
`REFERENCED` bit (they depend on how the unit uses the entity); the
overload-candidate and ADL flags; immediate-escalation, triviality, NRVO
and return-deduction bits; which specializations a unit instantiated;
any hash-map order; source locations, comments, macro names and spelling;
the types and value categories of nodes other than literals and casts
(they follow from the operands and the resolved entities).

**Closures** (`odr_def/lambda.rs`, K-3d). A lambda-expression is its tag,
`1` and the closure type's index in the definition (a local entity,
numbered at its first appearance like a variable, so a type mention
before the lambda-expression numbers it) or `0` for a closure of another
context (a default member initializer's or default argument's, hashed at
the use), then `0xB0` and the closure's digest. Never a mangling number.
The digest is FNV-1a 128 of the closure's own serialization: `0xB0`; the
capture count, then per capture in sema's capture order (the closure's
member order: explicit captures as written, then implicit ones in order
of first odr-use) its kind byte (`this`, `*this`, by copy, by reference,
VLA bound), a flag byte (init-capture, pack) and the captured entity as
the ENCLOSING scope names it (a local by index, a capture of an
enclosing closure by position) or `0xA0` (`this`, `*this`, an
init-capture, a VLA bound); the capture members' initializers as
expressions of the enclosing scope (an init-capture's initializer, the
copy construction sema chose); then, in the closure's own scope (its
call operator's locals, captured entities `0xB1`), the call operator's
declarator (the rule above: `constexpr` never, `consteval`, `static` and
an explicit object parameter as written, the effective `noexcept`),
`mutable`, a generic lambda's template parameters (count; kind, pack, a
non-type parameter's type, the type-constraint) and requires-clause or
`0xA0`, and the body. The capture-default is not hashed (`[=]` and `[x]`
capturing only `x` are equal; `[&]` and `[=]` differ by the kinds). A
generic lambda's body is its pattern: its dependent nodes are the
dependent kinds below. The digest is memoized per closure (a closure
naming its own type meets a zero digest). A closure type named only as a
type (`decltype([]{})`) is `0xB2` followed by its digest. A local entity
of an enclosing scope named inside a closure without capture (an
unevaluated operand, a constant) is `0xB6`.

**Closure member rows** (O-14). The call operator's row, and each
emitted specialization of a generic call operator's, hash `0xB5`, the
closure's digest as its enclosing definition's walk computed it (the
outermost function enclosing it through call operators and local
classes is walked to fill the memo; a closure inside an instantiated
generic call operator's body: that specialization), then `0`, or `1`,
the specialization's function type and template arguments by encoding.
A closure outside any function (a namespace-scope variable's initializer,
a default member initializer, a default argument) is computed on its own:
its context is the closure itself (none of these can capture a local).
The static invoker and the conversion function have no row; a
reference to the enclosing scope's captures from inside a
specialization's own body is the placeholder (a miss).

**Local classes and enumerations** are hashed where they are declared:
`0xB4`, the index, the declaration kind, the bases (count; type, virtual
and access bits) for a class, then the members in declaration order
(implicit ones, typedefs and access specifiers left out): a field as
kind, name, type, flags, bit-width, default member initializer; an
enumerator as name and value; a member function as kind, operator or
name and its definition inline (declarator, ctor-initializers or
deallocation function, body; its parameters and locals share the
enclosing numbering); a nested class recursively; anything else
`partial`. A reference to a member is `0xB3`. Their member functions
have no rows (the symbols are the enclosing function's, O-14 and ruling
1). Static locals hash inside their function (above).

**X15** (`lambda_in_declarator`). A function definition gets no row when
its function type or its template arguments (on the definition or the
template it was instantiated from) hold a closure type declared in a
declarator (its mangling context is the definition or its template, or
it has no mangling context and no function encloses it), or when a
default argument or the trailing requires-clause holds a
lambda-expression. Decided on the AST: the same on every route.

**Misc area** (`odr_def/misc.rs`, K-3d). Statement expression: its block.
Label address and `asm goto` labels: the label as a local. `__builtin_choose_expr`:
the condition and the operand it selected. `_Generic`: the controlling
type (a type operand, or the controlling expression's type) and the
SELECTED association only (the others are never evaluated). GNU `?:`:
the common operand, the condition over it, the false arm. Atomic
builtins: the operation and the operands. `__builtin_shufflevector`: the
operands and index expressions; `__builtin_convertvector` and an
imaginary literal: the type and operand; an ext-vector element access:
the accessor string and base; `va_arg`: the type and operand;
`offsetof`: the type and the computed offset or its steps. A constraint
outside a template (`ConceptSpecializationExpr`, `RequiresExpr`): its
satisfaction value. A pseudo-object: its semantic result. Coroutines:
`co_await`/`co_yield` the implicit bit, the suspend kind, the written
operand, the awaiter and the resolved `await_ready`, `await_suspend` and
`await_resume` calls; `co_return` its operand and the promise call; a
coroutine body the promise variable, the initial suspend point, the
written body, the final suspend point, `unhandled_exception`, the
implicit `co_return;`, the frame allocation and deallocation calls,
`get_return_object` and the allocation-failure return (the parameter
copies and the returned-object plumbing follow from these: not hashed).
`asm`: the flag bits, the output count, the template bytes, each operand
(name, constraint bytes, expression), the clobbers, the labels. A
file-scope `asm` is in no definition.

**Walk.** An explicit work stack, never recursion: no depth of expression
or statement nesting overflows the stack (a closure's digest recurses
once per nesting level of lambdas, not of expressions). No per-node allocation; the
hasher keeps one stream buffer, one stack and its memo maps per unit.

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
