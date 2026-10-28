//! One test per layout rule (spec/abi.md §4–§5), each naming the
//! standard or Itanium C++ ABI section it exercises. Expected values are
//! hard-coded; when `clang++-19` is installed every model is also dumped
//! by clang and compared with kiln's dump byte for byte.

#![allow(clippy::many_single_char_names, reason = "short names mirror the C++ under test")]

mod common;

use common::{
    EnumDef, FieldDef, Item, MARKER, Model, RecDef, Ty, TypedefDef, blocks, clang_available,
    run_clang,
};
use kiln_abi::target::target;
use kiln_abi::{Layouter, dump_record_layout, ensure_layout, record_layout, type_size_align};
use kiln_ast::decl::TagKind;
use kiln_ast::types::BuiltinType as B;

const X86: &str = "x86_64-unknown-linux-gnu";
const A64: &str = "aarch64-unknown-linux-gnu";
const BOTH: [&str; 2] = [X86, A64];

fn b(b: B) -> Ty {
    Ty::B(b)
}

fn f(name: &str, ty: Ty) -> FieldDef {
    FieldDef::new(name, ty)
}

fn bf(name: &str, ty: Ty, w: u64) -> FieldDef {
    FieldDef::bits(Some(name), ty, w)
}

fn unnamed(ty: Ty, w: u64) -> FieldDef {
    FieldDef::bits(None, ty, w)
}

fn st(m: &mut Model, name: &str, fields: Vec<FieldDef>) -> usize {
    m.push(Item::Record(RecDef::new(name, TagKind::Struct, fields)))
}

fn with(m: &mut Model, rec: RecDef) -> usize {
    m.push(Item::Record(rec))
}

/// `(sizeof, alignof, dsize, field bit offsets)`.
type Expect<'a> = (u64, u32, u64, &'a [u64]);

/// Checks the layout of item `i` on `triple`, then compares every record
/// dump with clang's.
fn check(m: &Model, triple: &str, i: usize, want: Expect<'_>) {
    cross_check(m, triple);
    let tgt = target(triple).expect("target");
    let (ast, decls) = m.ast(&tgt);
    let l = record_layout(&ast, decls[i], &tgt);
    let got = (l.size, l.align, l.data_size, &*l.fields);
    assert_eq!(got, want, "{triple} {}", m.name_of(i));
    // Without bases, nvsize is dsize rounded up to whole bytes.
    assert_eq!(l.nv_size, l.data_size, "nvsize");
}

/// Every record of `m` dumps exactly as clang dumps it.
fn cross_check(m: &Model, triple: &str) {
    if !clang_available() {
        return;
    }
    let src = m.cpp();
    let theirs = blocks(&run_clang(triple, &src).unwrap_or_else(|e| panic!("clang: {e}\n{src}")));
    let tgt = target(triple).expect("target");
    let (ast, decls) = m.ast(&tgt);
    for (i, r) in m.records() {
        let ours = dump_record_layout(&ast, decls[i], &tgt);
        let ours = ours.strip_prefix(MARKER).expect("marker");
        let head = ours.lines().next().unwrap_or_default();
        assert_eq!(theirs.get(head).map(String::as_str), Some(ours), "{triple} {}\n{src}", r.name);
    }
}

// ---------------------------------------------------------------------
// [class.mem]/26, [expr.sizeof]/2, Itanium ABI §2.4 (II.2): fields in
// declaration order at the next offset aligned for their type; sizeof
// rounds up to the alignment.
// ---------------------------------------------------------------------

#[test]
fn struct_fields_in_order_with_padding() {
    let mut m = Model::default();
    let s = st(&mut m, "S", vec![f("a", b(B::Char)), f("b", b(B::Int)), f("c", b(B::Char))]);
    for t in BOTH {
        check(&m, t, s, (12, 4, 12, &[0, 32, 64]));
    }
}

/// [class.union]/2: every member at offset 0; size of the largest,
/// rounded to the strictest alignment.
#[test]
fn union_members_at_zero() {
    let mut m = Model::default();
    let fields = vec![f("a", b(B::Char)), f("b", b(B::Double)), f("c", b(B::Int).arr(3))];
    let u = with(&mut m, RecDef::new("U", TagKind::Union, fields));
    for t in BOTH {
        check(&m, t, u, (16, 8, 16, &[0, 0, 0]));
    }
}

/// [class.mem]/29: an empty class has nonzero size (1); Itanium §2.2.
#[test]
fn empty_class_has_size_one() {
    let mut m = Model::default();
    let e = st(&mut m, "E", vec![]);
    let z = st(&mut m, "Z", vec![unnamed(b(B::Int), 0)]);
    for t in BOTH {
        check(&m, t, e, (1, 1, 1, &[]));
        let tgt = target(t).expect("target");
        let (ast, decls) = m.ast(&tgt);
        let mut lay = Layouter::new(&ast, &tgt);
        assert!(lay.is_empty(decls[e]));
        // [class.bit]/2: an unnamed bit-field is not a member, so `Z` is
        // empty too.
        assert!(lay.is_empty(decls[z]));
    }
}

// ---------------------------------------------------------------------
// Bit-fields: [class.bit]/1 (allocation is implementation-defined) and
// Itanium §2.4 II.2.2 ("C bit-field layout": a bit-field never crosses
// a boundary of its declared type's alignment unit).
// ---------------------------------------------------------------------

#[test]
fn bit_fields_pack_within_their_unit() {
    let mut m = Model::default();
    let s = st(
        &mut m,
        "S",
        vec![
            bf("a", b(B::Int), 3),
            bf("b", b(B::Int), 30),
            bf("c", b(B::Char), 2),
            bf("d", b(B::Short), 9),
        ],
    );
    for t in BOTH {
        // `b` would straddle bit 32, so it starts there; `c` fits in
        // the last char unit of the second int; `d` starts a new short.
        check(&m, t, s, (12, 4, 12, &[0, 32, 62, 64]));
    }
}

#[test]
fn bit_field_straddling_its_unit_moves() {
    let mut m = Model::default();
    let s = st(&mut m, "S", vec![f("a", b(B::Char)), bf("b", b(B::LongLong), 60)]);
    for t in BOTH {
        check(&m, t, s, (16, 8, 16, &[0, 64]));
    }
}

/// [class.bit]/2: a zero-width bit-field pads to the next unit of its
/// type. Only on `AArch64` (AAPCS64 §10.1.8, clang
/// `useZeroLengthBitfieldAlignment`) does it also raise the alignment
/// of the class.
#[test]
fn zero_width_bit_field() {
    let mut m = Model::default();
    let s = st(&mut m, "S", vec![f("a", b(B::Char)), unnamed(b(B::Int), 0), f("b", b(B::Char))]);
    check(&m, X86, s, (5, 1, 5, &[0, 32, 32]));
    check(&m, A64, s, (8, 4, 8, &[0, 32, 32]));
}

/// Unnamed bit-fields do not affect the class alignment on x86-64
/// (System V psABI §3.1.2: "unnamed bit-fields' types do not affect the
/// alignment of a structure or union"); they do on `AArch64`.
#[test]
fn unnamed_bit_field_alignment() {
    let mut m = Model::default();
    let s = st(&mut m, "S", vec![f("a", b(B::Char)), unnamed(b(B::Int), 4), f("b", b(B::Char))]);
    check(&m, X86, s, (3, 1, 3, &[0, 8, 16]));
    check(&m, A64, s, (4, 4, 4, &[0, 8, 16]));
}

/// [class.bit]/1: a width larger than the type's is padding. Itanium
/// §2.4 II.2.1: the field is allocated as the largest integral type no
/// wider than the width, the rest is padding.
#[test]
fn wide_bit_field() {
    let mut m = Model::default();
    let s = st(&mut m, "S", vec![f("a", b(B::Char)), bf("b", b(B::Int), 40), f("c", b(B::Char))]);
    for t in BOTH {
        check(&m, t, s, (12, 4, 12, &[0, 32, 72]));
    }
}

// ---------------------------------------------------------------------
// Alignment specifiers and GNU attributes: [dcl.align]/5–6 and the GCC
// `packed` / `aligned` attributes.
// ---------------------------------------------------------------------

#[test]
fn packed_struct_has_byte_alignment() {
    let mut m = Model::default();
    let mut r = RecDef::new(
        "P",
        TagKind::Struct,
        vec![f("a", b(B::Char)), f("b", b(B::Int)), f("c", b(B::Short))],
    );
    r.packed = true;
    let p = with(&mut m, r);
    for t in BOTH {
        check(&m, t, p, (7, 1, 7, &[0, 8, 40]));
    }
}

#[test]
fn packed_field_and_explicit_alignment() {
    let mut m = Model::default();
    let mut x = f("x", b(B::Int));
    x.packed = true;
    let s = st(&mut m, "S", vec![f("c", b(B::Char)), x]);
    // In a packed class `aligned` still raises a field's alignment.
    let mut y = f("y", b(B::Int));
    y.aligned = Some(4);
    let mut r = RecDef::new("Q", TagKind::Struct, vec![f("c", b(B::Char)), y]);
    r.packed = true;
    let q = with(&mut m, r);
    for t in BOTH {
        check(&m, t, s, (5, 1, 5, &[0, 8]));
        check(&m, t, q, (8, 4, 8, &[0, 32]));
    }
}

#[test]
fn aligned_attribute_without_argument() {
    let mut m = Model::default();
    let mut r = RecDef::new("S", TagKind::Struct, vec![f("c", b(B::Char))]);
    r.aligned = Some(0);
    let s = with(&mut m, r);
    for t in BOTH {
        // GCC: the largest alignment ever used for any data type (16).
        check(&m, t, s, (16, 16, 16, &[0]));
    }
}

/// [dcl.align]/2: `alignas` on a member over-aligns it and its class.
#[test]
fn alignas_member_and_class() {
    let mut m = Model::default();
    let mut bm = f("b", b(B::Char));
    bm.alignas = Some(16);
    let s = st(&mut m, "S", vec![f("a", b(B::Char)), bm]);
    let mut r = RecDef::new("C", TagKind::Struct, vec![f("a", b(B::Int))]);
    r.alignas = Some(64);
    let c = with(&mut m, r);
    for t in BOTH {
        check(&m, t, s, (32, 16, 32, &[0, 128]));
        check(&m, t, c, (64, 64, 64, &[0]));
    }
}

/// GCC `aligned` on a typedef sets its alignment, lowering it too.
#[test]
fn aligned_typedefs() {
    let mut m = Model::default();
    let a8 = m.push(Item::Typedef(TypedefDef {
        name: "A8".into(),
        ty: b(B::Int),
        aligned: Some(8),
        vector_bytes: None,
    }));
    let a1 = m.push(Item::Typedef(TypedefDef {
        name: "A1".into(),
        ty: b(B::Int),
        aligned: Some(1),
        vector_bytes: None,
    }));
    let s = st(&mut m, "S", vec![f("c", b(B::Char)), f("x", Ty::Named(a8))]);
    let u = st(&mut m, "U", vec![f("c", b(B::Char)), f("x", Ty::Named(a1))]);
    for t in BOTH {
        check(&m, t, s, (16, 8, 16, &[0, 64]));
        check(&m, t, u, (5, 1, 5, &[0, 8]));
    }
}

// ---------------------------------------------------------------------
// [[no_unique_address]]: [dcl.attr.nouniqueaddr]/2, [intro.object]/9,
// Itanium §2.4 II.3 (empty fields placed at offset 0 unless that makes
// two subobjects of the same type share an address).
// ---------------------------------------------------------------------

#[test]
fn no_unique_address_empty_members_overlap() {
    let mut m = Model::default();
    let e = st(&mut m, "E", vec![]);
    let nua = |name: &str| FieldDef { nua: true, ..f(name, Ty::Named(e)) };
    let s = st(&mut m, "S", vec![nua("a"), f("x", b(B::Int))]);
    // Two `E` subobjects may not share an address.
    let t2 = st(&mut m, "T", vec![nua("a"), nua("b"), f("x", b(B::Int))]);
    for t in BOTH {
        check(&m, t, s, (4, 4, 4, &[0, 0]));
        check(&m, t, t2, (4, 4, 4, &[0, 8, 0]));
    }
}

/// Itanium §2.4 II.3 / §1.1 "POD for the purpose of layout": tail
/// padding of a non-POD class (here: members with different access,
/// C++03 [class]/4) is reused by a following `[[no_unique_address]]`
/// neighbour; a POD's is not.
#[test]
fn tail_padding_reuse_via_no_unique_address() {
    let mut m = Model::default();
    let pod = st(&mut m, "P", vec![f("i", b(B::Int)), f("c", b(B::Char))]);
    let mut c = f("c", b(B::Char));
    c.private = true;
    let np = st(&mut m, "N", vec![f("i", b(B::Int)), c]);
    let wp = st(
        &mut m,
        "WP",
        vec![FieldDef { nua: true, ..f("p", Ty::Named(pod)) }, f("d", b(B::Char))],
    );
    let wn =
        st(&mut m, "WN", vec![FieldDef { nua: true, ..f("n", Ty::Named(np)) }, f("d", b(B::Char))]);
    for t in BOTH {
        check(&m, t, pod, (8, 4, 8, &[0, 32]));
        check(&m, t, np, (8, 4, 5, &[0, 32]));
        check(&m, t, wp, (12, 4, 12, &[0, 64]));
        // `WN` has a non-POD member, so it is not POD either.
        check(&m, t, wn, (8, 4, 6, &[0, 40]));
    }
}

/// A user-provided constructor also makes a class non-POD (C++03
/// [class]/4 via aggregate, [dcl.init.aggr]/1).
#[test]
fn user_constructor_makes_non_pod() {
    let mut m = Model::default();
    let mut r = RecDef::new("C", TagKind::Struct, vec![f("i", b(B::Int)), f("c", b(B::Char))]);
    r.user_ctor = true;
    let c = with(&mut m, r);
    for t in BOTH {
        check(&m, t, c, (8, 4, 5, &[0, 32]));
    }
}

// ---------------------------------------------------------------------
// Arrays: [dcl.array]/1 (contiguous elements), the GNU zero-length
// array extension and flexible array members (C11 6.7.2.1/18, a GNU
// extension in C++).
// ---------------------------------------------------------------------

#[test]
fn flexible_and_zero_length_arrays() {
    let mut m = Model::default();
    let fam = st(&mut m, "F", vec![f("n", b(B::Int)), f("d", Ty::Flex(Box::new(b(B::Double))))]);
    let z = st(&mut m, "Z", vec![f("c", b(B::Char)), f("z", b(B::Int).arr(0))]);
    let only = st(&mut m, "O", vec![f("z", b(B::Int).arr(0))]);
    for t in BOTH {
        check(&m, t, fam, (8, 8, 8, &[0, 64]));
        check(&m, t, z, (4, 4, 4, &[0, 32]));
        // Only GNU zero-length arrays: sizeof is 0.
        check(&m, t, only, (0, 4, 0, &[0]));
    }
}

#[test]
fn arrays_of_records() {
    let mut m = Model::default();
    let p = st(&mut m, "P", vec![f("i", b(B::Short)), f("c", b(B::Char))]);
    let s = st(&mut m, "S", vec![f("c", b(B::Char)), f("a", Ty::Named(p).arr(3).arr(2))]);
    for t in BOTH {
        check(&m, t, s, (26, 2, 26, &[0, 16]));
    }
}

// ---------------------------------------------------------------------
// Scalar types: [basic.fundamental], [basic.compound], the target ABIs
// (System V x86-64 psABI §3.1.2, AAPCS64 §5) and clang extensions.
// ---------------------------------------------------------------------

#[test]
fn fundamental_types_per_target() {
    let tx = target(X86).expect("x86");
    let ta = target(A64).expect("aarch64");
    for t in [&tx, &ta] {
        assert_eq!(t.builtin_size_align(B::Bool), (1, 1));
        assert_eq!(t.builtin_size_align(B::Long), (8, 8));
        assert_eq!(t.builtin_size_align(B::Int128), (16, 16));
        assert_eq!(t.builtin_size_align(B::LongDouble), (16, 16));
        assert_eq!(t.builtin_size_align(B::WChar), (4, 4));
        assert_eq!(t.builtin_size_align(B::NullPtr), (8, 8));
    }
    // `char` and `wchar_t` signedness, `__float128`.
    assert!(tx.types.char_signed && tx.types.wchar_signed);
    assert!(!ta.types.char_signed && !ta.types.wchar_signed);
    assert!(tx.has_float128);
    assert!(!ta.has_float128);
}

/// [basic.compound]/3: pointers; Itanium §2.3: data member pointers are
/// one `ptrdiff_t`, member function pointers two.
#[test]
fn member_pointers() {
    let mut m = Model::default();
    let c = st(&mut m, "C", vec![]);
    let pf = Ty::MemPtr(Box::new(Ty::Func(Box::new(b(B::Void)), vec![])), c);
    let s = st(
        &mut m,
        "S",
        vec![f("p", Ty::MemPtr(Box::new(b(B::Int)), c)), f("f", pf), f("q", b(B::Char).ptr())],
    );
    for t in BOTH {
        check(&m, t, s, (32, 8, 32, &[0, 64, 192]));
    }
}

/// clang `_BitInt(N)` (C23 6.2.5): alignment is the smallest power of
/// two holding N bits, capped at 8 (x86-64) or 16 (`AArch64`) bytes.
#[test]
fn bit_int_layout() {
    let mut m = Model::default();
    let s = st(&mut m, "S", vec![f("a", Ty::BitInt(true, 7)), f("b", Ty::BitInt(false, 65))]);
    check(&m, X86, s, (24, 8, 24, &[0, 64]));
    check(&m, A64, s, (32, 16, 32, &[0, 128]));
}

/// `_Atomic(T)` promotes small types to a power-of-two size and aligns
/// to it (clang `getTypeInfo`, `MaxAtomicPromoteWidth` = 128).
#[test]
fn atomic_layout() {
    let mut m = Model::default();
    let s = st(
        &mut m,
        "S",
        vec![
            f("c", b(B::Char)),
            f("ld", Ty::Atomic(Box::new(b(B::LongDouble)))),
            f("cd", Ty::Complex(Box::new(b(B::Float)))),
        ],
    );
    for t in BOTH {
        // clang does not treat an `_Atomic` member as C++03 POD, so the
        // tail padding is outside dsize.
        check(&m, t, s, (48, 16, 40, &[0, 128, 256]));
    }
}

/// GCC vector types: size `vector_size`, aligned to it; `AArch64` caps the
/// alignment at 16 bytes.
#[test]
fn vector_layout() {
    let mut m = Model::default();
    let v = m.push(Item::Typedef(TypedefDef {
        name: "V".into(),
        ty: b(B::Int),
        aligned: None,
        vector_bytes: Some(32),
    }));
    let s = st(&mut m, "S", vec![f("c", b(B::Char)), f("v", Ty::Named(v))]);
    check(&m, X86, s, (64, 32, 64, &[0, 256]));
    check(&m, A64, s, (48, 16, 48, &[0, 128]));
}

/// [dcl.enum]/7–8: an enumeration has the layout of its underlying type.
#[test]
fn enum_layout() {
    let mut m = Model::default();
    let e8 = m.push(Item::Enum(EnumDef { name: "E8".into(), fixed: Some(B::UChar) }));
    let ei = m.push(Item::Enum(EnumDef { name: "EI".into(), fixed: None }));
    let s = st(
        &mut m,
        "S",
        vec![f("a", Ty::Named(e8)), f("b", Ty::Named(ei)), bf("c", Ty::Named(e8), 3)],
    );
    for t in BOTH {
        check(&m, t, s, (12, 4, 12, &[0, 32, 64]));
    }
}

// ---------------------------------------------------------------------
// API surface.
// ---------------------------------------------------------------------

#[test]
fn type_size_align_and_cache_fill() {
    let mut m = Model::default();
    let p = st(&mut m, "P", vec![f("d", b(B::Double)), f("c", b(B::Char))]);
    let s = st(&mut m, "S", vec![f("p", Ty::Named(p).arr(2))]);
    let tgt = target(X86).expect("target");
    let (mut ast, decls) = m.ast(&tgt);
    let ty = ast.decls.get(decls[s]).ty().expect("type");
    let tl = type_size_align(&ast, ty, &tgt);
    assert_eq!((tl.size, tl.align), (32, 8));
    let l = ensure_layout(&mut ast, decls[s], &tgt).clone();
    assert_eq!(l.size, 32);
    // The nested class was cached as well.
    assert_eq!(ast.layouts.get(decls[p]).map(|l| l.size), Some(16));
}

/// The dump matches clang's `-fdump-record-layouts` text (spec/abi.md
/// §6) without needing clang installed.
#[test]
fn dump_format() {
    let mut m = Model::default();
    let e = st(&mut m, "E", vec![]);
    let p = st(&mut m, "P", vec![f("i", b(B::Int))]);
    let s = st(
        &mut m,
        "S",
        vec![
            f("b", b(B::Bool)),
            bf("x", b(B::UInt), 3),
            unnamed(b(B::Int), 0),
            FieldDef { nua: true, ..f("e", Ty::Named(e)) },
            f("p", Ty::Named(p)),
            f("fp", Ty::Func(Box::new(b(B::Int)), vec![]).ptr()),
        ],
    );
    let tgt = target(X86).expect("target");
    let (ast, decls) = m.ast(&tgt);
    let got = dump_record_layout(&ast, decls[s], &tgt);
    let want = "
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
";
    assert_eq!(got, want);
    cross_check(&m, X86);
}
