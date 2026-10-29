# spec: kiln_abi — targets, type and record layout

Status: **implemented for M2** (WP-050): classes without bases. §8 fixes
the M3 extension points (bases, virtual bases, vptr) so they do not
reshape the M2 interface.

Standard: [basic.fundamental], [basic.compound], [basic.align],
[intro.object]/9, [expr.sizeof], [expr.alignof], [class.mem]/26–29,
[class.bit], [class.union], [dcl.align], [dcl.attr.nouniqueaddr],
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
// dump (§6)
pub fn dump_record_layout(&Ast, DeclId, &Target) -> String;
pub fn write_record_layout(&mut Layouter, DeclId, &mut String);
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
- Not yet supported: bases (§8) and `#pragma pack` (`MaxFieldAlignment`,
  open question A3). MS layout and other triples are out of scope.

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
  - it has a private or protected non-static data member;
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

## 5. Record layout (Itanium §2.4 without bases; clang order)
State, all in bits: `size`, `data_size`, `align`, `unfilled_bits` and
the field offsets.

1. **Initialize.** `align = 8`, then raised to the class's maximum
   `aligned`/`alignas` value (§3.1).
2. **Each field, in declaration order:**
   - **Non-bit-field** (`LayoutField`):
     - Its alignment is `align(T)`.
     - For a `packed` field, or in a `packed` class, the type's
       alignment is replaced by 8 bits. Exception: in a `packed` class,
       a field of non-POD class type that is not itself packed keeps
       its alignment (clang ≥ 15, `-Wpacked-non-pod`).
     - The field's own `aligned`/`alignas` then raises the alignment,
       even when packed.
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
     - Wider than its type (`LayoutWideBitField`, [class.bit]/1): the
       field is allocated as the widest of `unsigned char`…
       `unsigned long long` that fits in the width. The rest is padding.
3. **Flexible and zero-length arrays.** A trailing `T[]` and `T[0]` have
   width 0 and keep their element alignment, so `sizeof` may be 0.
4. **Finish.**
   - `nvsize = data_size` rounded up to bytes. This is computed
     *before* the empty-class fix-up, so a non-POD empty class has
     `nvsize` 0.
   - An empty class gets `size = 8` bits.
   - `size = alignTo(size, align)`.
   - POD: `dsize = nvsize = size`. Otherwise `dsize = nvsize`.
   - `nvalign = align` and `preferred_align = align`.

`RecordLayout.fields` holds the bit offset of each `Field` declaration,
in declaration order, unnamed bit-fields included.

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
- **Unnamed classes and enums** print `(unnamed struct)`. clang appends
  `at file:line:col` there; kiln prints no location yet (open question
  A2).
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
- **`common/mod.rs`.** A small model that renders as C++ source and as a
  kiln AST built through `AstBuilder`. It will be retired for
  parser-built ASTs once WP-048 lands.

## 8. M3 extension points (bases, virtual bases, vptr)
The M2 code is structured so that M3 *adds* steps to the algorithm
rather than changing them:
- **`RecordLayout`** (spec/types.md §11) already carries `bases`,
  `vbases`, `vptr` and `primary_base`. M2 leaves them empty.
- **Layout order** (Itanium §2.4 I–IV), inserted around §5 step 2:
  1. choose the primary base (II.1);
  2. place the vptr at 0 if the class is dynamic with no primary base;
  3. place the non-virtual bases (II.2): empty bases at 0 when no
     conflict arises, otherwise at `alignTo(dsize, nvalign)`;
  4. lay out the fields (unchanged);
  5. compute `nvsize`/`nvalign`;
  6. place the virtual bases (III) in inheritance-graph order;
  7. finish.
- **Tail padding.** Bases are placed at `dsize` of the preceding data. A
  base contributes its `nvsize`, and its `dsize` when it is non-POD.
  §4's POD test already returns false for classes with bases. M3 adds
  the clang `isPOD` rules for bases and the `-fclang-abi-compat`
  variants.
- **Empty-subobject map.** `EmptySubobjectMap` already takes a class
  and an offset. M3 adds base subobjects (`CanPlaceBaseAtOffset`) and
  the `SizeOfLargestEmptySubobject` cut-off, which already exists as
  `RecordFacts::largest_empty`.
- **`ClassDesc.has_bases` / `dynamic`** already exist, and the POD and
  emptiness tests (§4) already account for bases. A class with bases is
  currently laid out *as if it had none* (fields only). Before M3,
  callers must not rely on such layouts.
- **Dump.** Bases print as `{offset} |   class B (base)` / `(primary
  base)` / `(virtual base)`, and the vptr as `(C vtable pointer)`. A
  non-POD footer prints `nvsize/nvalign` that differ from `dsize`.

## 9. Open questions (with recommendation)
- **A1.** `preferred_align` is equal to `align` on both targets. i386
  `double` is where it differs; out of scope.
- **A2.** Unnamed classes in the dump need source locations
  (`(unnamed struct at f.cpp:3:5)`). Recommendation: the dump gains
  `&SourceManager` once the driver wires it; the layout is unaffected.
- **A3.** `#pragma pack` and `-fpack-struct` (clang `MaxFieldAlignment`).
  Recommendation: add `max_field_align` to `ClassDesc` when sema records
  the pragma state on the class.
- **A4.** `[[no_unique_address]]` arrays of empty classes walk every
  element in the empty-subobject map, as clang does. Arrays with
  millions of elements are linear-time. Recommendation: cap the walk at
  the class's largest-empty-subobject offset, as clang's
  `SizeOfLargestEmptySubobject` early-out does for offsets past it.
