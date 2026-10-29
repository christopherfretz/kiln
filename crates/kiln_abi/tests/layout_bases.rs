//! Non-virtual base layout (Itanium C++ ABI §2.4 II.1–II.3, spec/abi.md
//! §6): empty-base optimization, the empty-subobject rule, tail-padding
//! reuse, `[[no_unique_address]]`, `#pragma pack` and the subobject
//! table. Expected values are transcribed from `clang++-19
//! -fdump-record-layouts`; when clang is installed every model is also
//! dumped by clang and compared with kiln's dump byte for byte.

mod common;

use common::{FieldDef, Item, MARKER, Model, RecDef, Ty, blocks, clang_available, run_clang};
use kiln_abi::target::target;
use kiln_abi::{dump_record_layout, record_layout, subobjects};
use kiln_ast::class::SubobjectId;
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

fn nua(name: &str, ty: Ty) -> FieldDef {
    let mut f = FieldDef::new(name, ty);
    f.nua = true;
    f
}

/// `struct name : bases... { fields }`.
fn st(m: &mut Model, name: &str, bases: &[usize], fields: Vec<FieldDef>) -> usize {
    let mut r = RecDef::new(name, TagKind::Struct, fields);
    r.bases = bases.to_vec();
    m.push(Item::Record(r))
}

fn with(m: &mut Model, r: RecDef) -> usize {
    m.push(Item::Record(r))
}

/// `(sizeof, alignof, dsize, nvsize, base byte offsets, field bit offsets)`.
type Expect<'a> = (u64, u32, u64, u64, &'a [u64], &'a [u64]);

fn check(m: &Model, triple: &str, i: usize, want: Expect<'_>) {
    cross_check(m, triple);
    let tgt = target(triple).expect("target");
    let (ast, decls) = m.ast(&tgt);
    let l = record_layout(&ast, decls[i], &tgt);
    let bases: Vec<u64> = l.bases.iter().map(|&(_, o)| o).collect();
    let got = (l.size, l.align, l.data_size, l.nv_size, &bases[..], &*l.fields);
    assert_eq!(got, want, "{triple} {}", m.name_of(i));
    assert_eq!(l.nv_align, l.align, "nvalign");
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

/// Itanium §2.4 II.1: an empty base goes at offset 0 unless another
/// subobject of the same type is there ([intro.object]/9); then at the
/// next aligned offset past dsize.
///
/// ```c++
/// struct E {};
/// struct E2 : E {};           // E at 0; sizeof 1, dsize 0, nvsize 1
/// struct D : E, E2 { char c; }; // E at 0, E2 at 1 (its E would clash), c at 0
/// ```
#[test]
fn empty_base_same_type_rule() {
    let mut m = Model::default();
    let e = st(&mut m, "E", &[], vec![]);
    let e2 = st(&mut m, "E2", &[e], vec![]);
    let d = st(&mut m, "D", &[e, e2], vec![f("c", b(B::Char))]);
    for t in BOTH {
        check(&m, t, e2, (1, 1, 0, 1, &[0], &[]));
        check(&m, t, d, (2, 1, 1, 2, &[0, 1], &[0]));
    }
}

/// [dcl.attr.nouniqueaddr]: a `[[no_unique_address]]` empty member may
/// share an address with other subobjects, but never with one of the
/// same type.
///
/// ```c++
/// struct E {};
/// struct R : E { [[no_unique_address]] E e; int r; }; // E 0, e 1, r 0; sizeof 4
/// ```
#[test]
fn no_unique_address_member_after_empty_base() {
    let mut m = Model::default();
    let e = st(&mut m, "E", &[], vec![]);
    let ety = Ty::Named(e);
    let r = st(&mut m, "R", &[e], vec![nua("e", ety), f("r", b(B::Int))]);
    for t in BOTH {
        check(&m, t, r, (4, 4, 4, 4, &[0], &[8, 0]));
    }
}

/// Itanium §2.4 II.3: a derived class reuses the tail padding of a
/// non-POD base (dsize < sizeof); a POD base's dsize is its sizeof.
///
/// ```c++
/// struct P { int i; char c; P() {} };   // sizeof 8, dsize 5
/// struct Q : P { char d; };             // d at 5; sizeof 8, dsize 6
/// struct PodP { int i; char c; };       // sizeof 8, dsize 8
/// struct PodQ : PodP { char d; };       // d at 8; sizeof 12, dsize 9
/// ```
#[test]
fn tail_padding_reuse_and_pod() {
    let mut m = Model::default();
    let mut p = RecDef::new("P", TagKind::Struct, vec![f("i", b(B::Int)), f("c", b(B::Char))]);
    p.user_ctor = true;
    let p = with(&mut m, p);
    let q = st(&mut m, "Q", &[p], vec![f("d", b(B::Char))]);
    let pp = st(&mut m, "PodP", &[], vec![f("i", b(B::Int)), f("c", b(B::Char))]);
    let pq = st(&mut m, "PodQ", &[pp], vec![f("d", b(B::Char))]);
    for t in BOTH {
        check(&m, t, p, (8, 4, 5, 5, &[], &[0, 32]));
        check(&m, t, q, (8, 4, 6, 6, &[0], &[40]));
        check(&m, t, pp, (8, 4, 8, 8, &[], &[0, 32]));
        check(&m, t, pq, (12, 4, 9, 9, &[0], &[64]));
    }
}

/// `#pragma pack(N)` caps member alignment (clang `MaxFieldAlignment`)
/// for fields and bit-fields; a class derived outside the pragma reuses
/// the packed base's tail.
///
/// ```c++
/// #pragma pack(push, 2)
/// struct K { char c; double d; int x : 3; }; // d at 2, x at 10:0-2; sizeof 12, align 2
/// #pragma pack(pop)
/// struct L : K { char z; };                   // z at 12; sizeof 14, dsize 13
/// ```
#[test]
fn pragma_pack_caps_alignment() {
    let mut m = Model::default();
    let mut k = RecDef::new(
        "K",
        TagKind::Struct,
        vec![f("c", b(B::Char)), f("d", b(B::Double)), FieldDef::bits(Some("x"), b(B::Int), 3)],
    );
    k.pack = Some(2);
    let k = with(&mut m, k);
    let l = st(&mut m, "L", &[k], vec![f("z", b(B::Char))]);
    for t in BOTH {
        check(&m, t, k, (12, 2, 12, 12, &[], &[0, 16, 80]));
        check(&m, t, l, (14, 2, 13, 13, &[0], &[96]));
    }
}

/// Non-empty bases in order at aligned offsets past dsize; an empty base
/// goes at 0 when no subobject of its type is there; empty members are
/// placed like empty bases but are never at the same address as another
/// `E`.
///
/// ```c++
/// struct E {};
/// struct B1 { char a; };
/// struct B2 : B1 { short s; };     // B1 0, s at 2; sizeof 4
/// struct M : B2, E { E e2; char t; }; // B2 0, E 0, e2 4, t 5; sizeof 6
/// ```
#[test]
fn mixed_bases_and_empty_members() {
    let mut m = Model::default();
    let e = st(&mut m, "E", &[], vec![]);
    let b1 = st(&mut m, "B1", &[], vec![f("a", b(B::Char))]);
    let b2 = st(&mut m, "B2", &[b1], vec![f("s", b(B::Short))]);
    let mm = st(&mut m, "M", &[b2, e], vec![f("e2", Ty::Named(e)), f("t", b(B::Char))]);
    for t in BOTH {
        check(&m, t, b2, (4, 2, 4, 4, &[0], &[16]));
        check(&m, t, mm, (6, 2, 6, 6, &[0, 0], &[32, 40]));
    }
}

/// A base's alignment raises the class's; `alignas` on the base class
/// carries into the derived class, and an empty over-aligned base at 0
/// raises alignment without adding size.
///
/// ```c++
/// struct alignas(16) A {};       // sizeof 16, empty
/// struct S : A { char c; };      // A 0, c 0; sizeof 16, align 16
/// struct I { int i; };
/// struct T : I { char c; double d; }; // c 4, d 8; sizeof 16
/// ```
#[test]
fn base_alignment_and_alignas() {
    let mut m = Model::default();
    let mut a = RecDef::new("A", TagKind::Struct, vec![]);
    a.alignas = Some(16);
    let a = with(&mut m, a);
    let s = st(&mut m, "S", &[a], vec![f("c", b(B::Char))]);
    let i = st(&mut m, "I", &[], vec![f("i", b(B::Int))]);
    let tt = st(&mut m, "T", &[i], vec![f("c", b(B::Char)), f("d", b(B::Double))]);
    for t in BOTH {
        check(&m, t, s, (16, 16, 1, 16, &[0], &[0]));
        check(&m, t, tt, (16, 8, 16, 16, &[0], &[32, 64]));
    }
}

/// Bit-fields continue in the tail of a non-POD base's last
/// storage unit (Itanium §2.4 II.2: allocation starts at dsize, in bits
/// rounded to the byte).
///
/// ```c++
/// struct H { int a : 3; H() {} };   // sizeof 4, dsize 1
/// struct J : H { int b : 5; };      // b at 1:0-4; sizeof 4, dsize 2
/// ```
#[test]
fn bit_fields_after_base() {
    let mut m = Model::default();
    let mut h = RecDef::new("H", TagKind::Struct, vec![FieldDef::bits(Some("a"), b(B::Int), 3)]);
    h.user_ctor = true;
    let h = with(&mut m, h);
    let j = st(&mut m, "J", &[h], vec![FieldDef::bits(Some("b"), b(B::Int), 5)]);
    for t in BOTH {
        check(&m, t, j, (4, 4, 2, 2, &[0], &[8]));
    }
}

/// The subobject table is a preorder of the non-virtual base graph:
/// 0 is the complete object, then each base's subtree in declaration
/// order, with complete-object byte offsets.
///
/// ```c++
/// struct E {};
/// struct B1 { char a; };
/// struct B2 : B1 { short s; };
/// struct M : B2, E { E e2; char t; };
/// // M@0, B2@0, B1@0, E@0
/// ```
#[test]
fn subobject_table_preorder() {
    let mut m = Model::default();
    let e = st(&mut m, "E", &[], vec![]);
    let b1 = st(&mut m, "B1", &[], vec![f("a", b(B::Char))]);
    let b2 = st(&mut m, "B2", &[b1], vec![f("s", b(B::Short))]);
    let d = st(&mut m, "D", &[e, b1], vec![]);
    let mm = st(&mut m, "M", &[b2, e, d], vec![f("t", b(B::Char))]);
    let tgt = target(X86).expect("target");
    let (ast, decls) = m.ast(&tgt);
    let table = subobjects(&ast, decls[mm], &tgt);
    let got: Vec<(usize, u16, u16, u64)> = table
        .iter()
        .map(|(s, off)| {
            let class = decls.iter().position(|&x| x == s.class).expect("class");
            (class, s.parent.0, s.base_index, *off)
        })
        .collect();
    let l = record_layout(&ast, decls[mm], &tgt);
    let d_off = l.bases[2].1;
    let dl = record_layout(&ast, decls[d], &tgt);
    assert_eq!(
        got,
        vec![
            (mm, 0, 0, 0),
            (b2, 0, 0, 0),
            (b1, 1, 0, 0),
            (e, 0, 1, 0),
            (d, 0, 2, d_off),
            (e, 4, 0, d_off + dl.bases[0].1),
            (b1, 4, 1, d_off + dl.bases[1].1),
        ]
    );
    assert!(table.iter().all(|(s, _)| !s.is_virtual));
    assert_eq!(table[0].0.parent, SubobjectId::COMPLETE);
    cross_check(&m, X86);
    cross_check(&m, A64);
}
