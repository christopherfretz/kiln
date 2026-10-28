//! `ConstStore` behaviour (WP-043, spec/ast.md §9): round-trips, interning,
//! image validation, shadow runs, printing goldens, sizes, memory report.
#![allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::match_same_arms,
    reason = "table-style test data"
)]

use std::mem::size_of;

use kiln_ast::consts::{
    ByteImage, ByteRange, ConstError, ConstStore, ConstValue, FloatKind, IntBits, IntStyle, Member,
    MemberKind, NoEnv, NullKind, PrintEnv, PrintOptions, PrintStyle, PtrBase, Reloc, Scalar,
    ShadowKind, ShadowRun, Shape, UnionMember, ValueKind,
};
use kiln_ast::strings::{StringEncoding, StringPool};
use kiln_ast::{ConstValueId, DeclId, HeapSize, QualType, ReflectionId, StringLitId, TypeId};

fn ty(n: u32) -> QualType {
    QualType::unqual(TypeId::new(n))
}

// Test type ids (meaningful only to `Env` below).
const INT: u32 = 12;
const UINT: u32 = 13;
const ULONG: u32 = 14;
const SHORT: u32 = 15;
const CHAR: u32 = 16;
const BOOL: u32 = 17;
const DOUBLE: u32 = 18;
const ENUM: u32 = 19;
const INT_PTR: u32 = 40;
const INT_REF: u32 = 41;
const ARR3: u32 = 42; // int[3]
const PAIR: u32 = 43; // struct P { int a; int *p; }  (size 16)
const NEST: u32 = 44; // struct N { int x; struct { int y; } in; int arr[2]; } (size 16)
const UNION: u32 = 45; // union U { int i; char c; } (size 4)
const BITS: u32 = 46; // struct B { int a : 3; int : 5; unsigned b : 4; } (size 4)
const DEEP: u32 = 47; // int[1][1][1]
const ARR1: u32 = 48; // int[1]
const ARR11: u32 = 49; // int[1][1]
const BIG: u32 = 50; // int[1000]
const MEMPTR: u32 = 51;
const NULLPTR_T: u32 = 52;
const INFO: u32 = 53;
const FN_PTR: u32 = 54;
const INNER: u32 = 55; // struct { int y; }
const ARR2: u32 = 56; // int[2]
const BITINT200: u32 = 57;
const LDOUBLE: u32 = 58; // x87 long double, 16 bytes
const LD_HOLDER: u32 = 59; // struct { long double v; }
const BI_ARR: u32 = 60; // _BitInt(200)[1]

fn int(v: i128) -> Scalar<'static> {
    Scalar::Int { ty: ty(INT), signed: true, bits: IntBits::from_i128(32, v) }
}

fn member(name: &str, t: u32, off: u64, size: u64, field: u32) -> Member {
    Member {
        name: name.to_string(),
        ty: ty(t),
        offset_bits: off * 8,
        size,
        bit_width: None,
        field: Some(DeclId::new(field)),
    }
}

struct Env;

impl PrintEnv for Env {
    fn shape(&self, t: QualType) -> Shape {
        let int =
            |width, signed, suffix| Shape::Int { width, signed, style: IntStyle::Plain { suffix } };
        match t.ty().index() {
            INT => int(32, true, ""),
            UINT => int(32, false, "U"),
            ULONG => int(64, false, "UL"),
            SHORT => Shape::Int { width: 16, signed: true, style: IntStyle::Cast },
            CHAR => Shape::Int {
                width: 8,
                signed: true,
                style: IntStyle::Char(StringEncoding::Ordinary),
            },
            ENUM => Shape::Int { width: 32, signed: true, style: IntStyle::Enum },
            BITINT200 => {
                Shape::Int { width: 200, signed: true, style: IntStyle::Plain { suffix: "" } }
            }
            BOOL => Shape::Bool,
            DOUBLE => Shape::Float(FloatKind::Double),
            LDOUBLE => Shape::Float(FloatKind::X87Extended),
            INT_PTR => Shape::Pointer { pointee: Some(ty(INT)), reference: false },
            INT_REF => Shape::Pointer { pointee: Some(ty(INT)), reference: true },
            FN_PTR => Shape::Pointer { pointee: None, reference: false },
            MEMPTR => Shape::MemberPointer,
            NULLPTR_T => Shape::NullPtr,
            INFO => Shape::Reflection,
            BI_ARR => Shape::Array { elem: ty(BITINT200), stride: 32, len: 1 },
            ARR3 => Shape::Array { elem: ty(INT), stride: 4, len: 3 },
            ARR2 => Shape::Array { elem: ty(INT), stride: 4, len: 2 },
            ARR1 => Shape::Array { elem: ty(INT), stride: 4, len: 1 },
            ARR11 => Shape::Array { elem: ty(ARR1), stride: 4, len: 1 },
            DEEP => Shape::Array { elem: ty(ARR11), stride: 4, len: 1 },
            BIG => Shape::Array { elem: ty(INT), stride: 4, len: 1000 },
            PAIR => Shape::Record {
                members: vec![member("a", INT, 0, 4, 100), member("p", INT_PTR, 8, 8, 101)],
                union: false,
            },
            INNER => Shape::Record { members: vec![member("y", INT, 0, 4, 102)], union: false },
            NEST => Shape::Record {
                members: vec![
                    member("x", INT, 0, 4, 103),
                    member("in", INNER, 4, 4, 104),
                    member("arr", ARR2, 8, 8, 105),
                ],
                union: false,
            },
            UNION => Shape::Record {
                members: vec![member("i", INT, 0, 4, 106), member("c", CHAR, 0, 1, 107)],
                union: true,
            },
            BITS => {
                let bf = |name: &str, t, off, w, f: Option<u32>| Member {
                    name: name.to_string(),
                    ty: ty(t),
                    offset_bits: off,
                    size: 0,
                    bit_width: Some(w),
                    field: f.map(DeclId::new),
                };
                Shape::Record {
                    members: vec![
                        bf("a", INT, 0, 3, Some(108)),
                        bf("", INT, 3, 5, None),
                        bf("b", UINT, 8, 4, Some(109)),
                    ],
                    union: false,
                }
            }
            LD_HOLDER => {
                Shape::Record { members: vec![member("v", LDOUBLE, 0, 16, 110)], union: false }
            }
            _ => Shape::Unknown,
        }
    }

    fn decl_name(&self, d: DeclId, out: &mut String) {
        out.push_str(match d.index() {
            1 => "x",
            2 => "arr",
            3 => "s",
            4 => "f",
            5 => "S::m",
            6 => "S::g",
            7 => "ref_tmp",
            _ => "?",
        });
    }

    fn decl_type(&self, d: DeclId) -> Option<QualType> {
        match d.index() {
            1 => Some(ty(INT)),
            2 => Some(ty(ARR3)),
            3 => Some(ty(NEST)),
            7 => Some(ty(INT)),
            _ => None,
        }
    }

    fn type_name(&self, t: QualType, out: &mut String) {
        out.push_str(match t.ty().index() {
            SHORT => "short",
            ENUM => "E",
            PAIR => "P",
            NEST => "N",
            INNER => "N::(anonymous)",
            UNION => "U",
            _ => "T",
        });
    }

    fn enumerator(&self, _t: QualType, bits: &IntBits<'_>, out: &mut String) -> bool {
        if bits.to_i128() == Some(1) {
            out.push_str("E::A");
            return true;
        }
        false
    }

    fn reflection(&self, id: ReflectionId, out: &mut String) {
        out.push_str(if id.index() == 1 { "^^int" } else { "^^?" });
    }
}

fn le(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn img<'a>(
    bytes: &'a [u8],
    relocs: &'a [Reloc],
    padding: &'a [ByteRange],
    unions: &'a [UnionMember],
) -> ByteImage<'a> {
    ByteImage { ty: ty(900), bytes, relocs, padding, unions }
}

fn agg(t: u32, bytes: &[u8]) -> ByteImage<'_> {
    ByteImage { ty: ty(t), bytes, relocs: &[], padding: &[], unions: &[] }
}

// ---------------------------------------------------------------------------
// Round-trips and interning

fn roundtrip(store: &mut ConstStore, s: Scalar<'_>) -> ConstValueId {
    let id = store.push_scalar(s).expect("push");
    assert_eq!(store.get(id), ConstValue::Scalar(s), "round-trip {s:?}");
    assert_eq!(store.push_scalar(s), Ok(id), "dedup {s:?}");
    assert_eq!(store.lookup(&ConstValue::Scalar(s)), Some(id));
    assert_eq!(store.ty(id), s.ty());
    assert_eq!(store.scalar(id), Some(s));
    assert!(store.aggregate(id).is_none());
    assert!(store.bytes(id).is_empty() && store.relocs(id).is_empty());
    assert_eq!(store.shadow(id).count(), 0);
    id
}

#[test]
fn int_widths_roundtrip() {
    let mut st = ConstStore::default();
    let mut ids = Vec::new();
    for width in [1u32, 8, 16, 31, 32, 33, 63, 64, 65, 96, 127, 128] {
        let max = if width == 128 { u128::MAX } else { (1u128 << width) - 1 };
        for v in [
            0u128,
            1,
            max,
            max >> 1,
            (max >> 1) + 1,
            0x5555_5555_5555_5555_5555_5555_5555_5555 & max,
        ] {
            for signed in [false, true] {
                let s = Scalar::Int {
                    ty: ty(1000 + width),
                    signed,
                    bits: IntBits::from_u128(width, v),
                };
                ids.push(roundtrip(&mut st, s));
            }
        }
    }
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    let distinct_inputs: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), distinct_inputs.len());
    // Sign extension through from_i128 and back.
    let s = Scalar::Int { ty: ty(INT), signed: true, bits: IntBits::from_i128(8, -128) };
    let id = roundtrip(&mut st, s);
    let Some(Scalar::Int { bits, .. }) = st.scalar(id) else { panic!() };
    assert_eq!(bits.to_i128(), Some(-128));
    assert_eq!(bits.to_u128(), Some(0x80));
    let min = Scalar::Int { ty: ty(1128), signed: true, bits: IntBits::from_i128(128, i128::MIN) };
    let id = roundtrip(&mut st, min);
    let Some(Scalar::Int { bits, .. }) = st.scalar(id) else { panic!() };
    assert_eq!(bits.to_i128(), Some(i128::MIN));
}

#[test]
fn wide_bitint_roundtrip() {
    let mut st = ConstStore::default();
    let words = [u64::MAX, 7, u64::MAX, (1 << 8) - 1]; // 200 bits, all-ones top word
    let bits = IntBits::from_words(200, &words).unwrap();
    assert!(IntBits::from_words(200, &[0, 0, 0, 1 << 8]).is_none(), "bit 200 set");
    assert!(IntBits::from_words(200, &[0, 0, 0]).is_none(), "too few words");
    let s = Scalar::Int { ty: ty(BITINT200), signed: true, bits };
    let id = roundtrip(&mut st, s);
    let Some(Scalar::Int { bits: back, .. }) = st.scalar(id) else { panic!() };
    assert_eq!(back.words(), &words);
    assert!(back.sign_bit());
    // Same value, different width (type) → different value.
    let words2 = [u64::MAX, 7, u64::MAX, (1 << 9) - 1];
    let s2 = Scalar::Int {
        ty: ty(BITINT200 + 100),
        signed: true,
        bits: IntBits::from_words(201, &words2).unwrap(),
    };
    assert_ne!(roundtrip(&mut st, s2), id);
    // The words went to the pool once per distinct value.
    let mem = st.heap_bytes();
    assert_eq!(mem.row("words").unwrap().count, 8);
    // A small negative _BitInt(200) prints exactly.
    let m3 = IntBits::from_words(200, &[u64::MAX - 2, u64::MAX, u64::MAX, 0xff]).unwrap();
    assert_eq!(m3.to_i128(), Some(-3));
    let mut s = String::new();
    m3.write_decimal(true, &mut s);
    assert_eq!(s, "-3");
    s.clear();
    m3.write_decimal(false, &mut s);
    assert_eq!(s, "1606938044258990275541962092341162602522202993782792835301373"); // 2^200 - 3
}

#[test]
fn every_scalar_kind_roundtrips() {
    let mut st = ConstStore::with_pointer_bytes(8);
    roundtrip(&mut st, Scalar::Bool { ty: ty(BOOL), value: true });
    roundtrip(&mut st, Scalar::Bool { ty: ty(BOOL), value: false });
    let samples: [(FloatKind, u128); 6] = [
        (FloatKind::Half, 0x3e00),
        (FloatKind::BFloat16, 0x3fc0),
        (FloatKind::Single, u128::from(1.5f32.to_bits())),
        (FloatKind::Double, u128::from(1.5f64.to_bits())),
        (FloatKind::X87Extended, (0x3fffu128 << 64) | (0xc000_0000_0000_0000)),
        (FloatKind::Quad, 0x3fff_8000_0000_0000_0000_0000_0000_0000),
    ];
    for (i, (kind, bits)) in samples.into_iter().enumerate() {
        let t = ty(2000 + i as u32);
        let id = roundtrip(&mut st, Scalar::Float { ty: t, kind, bits });
        assert_eq!(
            st.display(id, &StringPool::new(), &NoEnv, &PrintOptions::DEFAULT),
            "1.5e0",
            "{kind:?}"
        );
        // -0.0 and +0.0 are distinct values.
        let sign = 1u128 << (kind.bits() - 1);
        let pz = roundtrip(&mut st, Scalar::Float { ty: t, kind, bits: 0 });
        let nz = roundtrip(&mut st, Scalar::Float { ty: t, kind, bits: sign });
        assert_ne!(pz, nz);
        assert_eq!(st.kind(nz), ValueKind::Float);
    }
    for kind in [NullKind::Pointer, NullKind::MemberData, NullKind::MemberFunction] {
        roundtrip(&mut st, Scalar::Null { ty: ty(NULLPTR_T), kind });
    }
    let c = roundtrip(&mut st, int(5));
    for base in [
        PtrBase::Decl(DeclId::new(1)),
        PtrBase::Function(DeclId::new(4)),
        PtrBase::Temporary(DeclId::new(7)),
        PtrBase::StringLit(StringLitId::new(1)),
        PtrBase::Const(c),
        PtrBase::TypeInfo(ty(INT)),
    ] {
        let id = roundtrip(&mut st, Scalar::Pointer { ty: ty(INT_PTR), base, offset: 0 });
        assert_eq!(st.kind(id), ValueKind::Pointer);
        if !matches!(base, PtrBase::Function(_)) {
            roundtrip(&mut st, Scalar::Pointer { ty: ty(INT_PTR), base, offset: 12 });
            // Offsets beyond 32 bits go to the word pool.
            roundtrip(&mut st, Scalar::Pointer { ty: ty(INT_PTR), base, offset: 1 << 40 });
        }
    }
    let path = [DeclId::new(30), DeclId::new(31)];
    for (kind, derived) in [(MemberKind::Data, false), (MemberKind::Function, true)] {
        roundtrip(
            &mut st,
            Scalar::MemberPtr {
                ty: ty(MEMPTR),
                member: DeclId::new(5),
                kind,
                derived,
                path: &path,
            },
        );
        roundtrip(
            &mut st,
            Scalar::MemberPtr { ty: ty(MEMPTR), member: DeclId::new(5), kind, derived, path: &[] },
        );
    }
    let r = roundtrip(&mut st, Scalar::Reflection { ty: ty(INFO), id: ReflectionId::new(1) });
    assert_eq!(st.kind(r), ValueKind::Reflection);
    assert_eq!(st.reloc_width(r), Some(8));
    assert_eq!(st.ids().count(), st.len());
}

#[test]
fn type_is_part_of_identity() {
    let mut st = ConstStore::default();
    let a = st.push_scalar(int(1)).unwrap();
    let b = st
        .push_scalar(Scalar::Int { ty: ty(UINT), signed: false, bits: IntBits::from_u128(32, 1) })
        .unwrap();
    let c = st
        .push_scalar(Scalar::Int { ty: ty(INT), signed: false, bits: IntBits::from_u128(32, 1) })
        .unwrap();
    assert!(a != b && a != c && b != c);
    assert_eq!(st.len(), 3);
}

#[test]
fn reloc_widths() {
    let mut st = ConstStore::with_pointer_bytes(4);
    assert_eq!(st.pointer_bytes(), 4);
    let p = st
        .push_scalar(Scalar::Pointer {
            ty: ty(INT_PTR),
            base: PtrBase::Decl(DeclId::new(1)),
            offset: 0,
        })
        .unwrap();
    let n = st.push_scalar(Scalar::Null { ty: ty(INT_PTR), kind: NullKind::Pointer }).unwrap();
    let nd = st.push_scalar(Scalar::Null { ty: ty(MEMPTR), kind: NullKind::MemberData }).unwrap();
    let nf =
        st.push_scalar(Scalar::Null { ty: ty(MEMPTR), kind: NullKind::MemberFunction }).unwrap();
    let md = st
        .push_scalar(Scalar::MemberPtr {
            ty: ty(MEMPTR),
            member: DeclId::new(5),
            kind: MemberKind::Data,
            derived: false,
            path: &[],
        })
        .unwrap();
    let mf = st
        .push_scalar(Scalar::MemberPtr {
            ty: ty(MEMPTR),
            member: DeclId::new(6),
            kind: MemberKind::Function,
            derived: false,
            path: &[],
        })
        .unwrap();
    let i = st.push_scalar(int(0)).unwrap();
    let got: Vec<_> = [p, n, nd, nf, md, mf, i].iter().map(|&id| st.reloc_width(id)).collect();
    assert_eq!(got, [Some(4), Some(4), Some(4), Some(8), Some(4), Some(8), None]);
}

// ---------------------------------------------------------------------------
// Aggregates

#[test]
fn aggregate_with_nested_relocations() {
    let mut st = ConstStore::default();
    // inner: P{7, &x}
    let px = st
        .push_scalar(Scalar::Pointer {
            ty: ty(INT_PTR),
            base: PtrBase::Decl(DeclId::new(1)),
            offset: 0,
        })
        .unwrap();
    let bytes = le(&[7, 0, 0, 0]);
    let relocs = [Reloc { at: 8, value: px }];
    let padding = [ByteRange { start: 4, len: 4 }];
    let inner_img =
        ByteImage { ty: ty(PAIR), bytes: &bytes, relocs: &relocs, padding: &padding, unions: &[] };
    let inner = st.push_aggregate(inner_img).unwrap();
    assert_eq!(st.kind(inner), ValueKind::Aggregate);
    assert_eq!(st.get(inner), ConstValue::Aggregate(inner_img));
    // outer: P{9, &inner.a} — a pointer into another frozen value.
    let pin = st
        .push_scalar(Scalar::Pointer { ty: ty(INT_PTR), base: PtrBase::Const(inner), offset: 0 })
        .unwrap();
    let bytes2 = le(&[9, 0, 0, 0]);
    let relocs2 = [Reloc { at: 8, value: pin }];
    let outer = st
        .push_aggregate(ByteImage {
            ty: ty(PAIR),
            bytes: &bytes2,
            relocs: &relocs2,
            padding: &padding,
            unions: &[],
        })
        .unwrap();
    assert_ne!(inner, outer);
    assert_eq!(st.relocs(outer), &relocs2);
    assert_eq!(st.bytes(outer), &bytes2[..]);
    assert_eq!(st.padding(outer), &padding);
    assert_eq!(st.unions(outer), &[]);
    // Thaw-style walk: follow the outer pointer to the inner image.
    let Some(Scalar::Pointer { base: PtrBase::Const(c), .. }) =
        st.scalar(st.relocs(outer)[0].value)
    else {
        panic!()
    };
    assert_eq!(c, inner);
    assert_eq!(st.relocs(c)[0].value, px);
    let strings = StringPool::new();
    let opts = PrintOptions::DEFAULT;
    assert_eq!(st.display(inner, &strings, &Env, &opts), "{7, &x}");
    assert_eq!(st.display(outer, &strings, &Env, &opts), format!("{{9, &#c{}.a}}", inner.index()));
    // Shadow summary: value, padding, pointer.
    let runs: Vec<ShadowRun> = st.shadow(outer).collect();
    assert_eq!(
        runs,
        [
            ShadowRun { range: ByteRange { start: 0, len: 4 }, kind: ShadowKind::Value },
            ShadowRun { range: ByteRange { start: 4, len: 4 }, kind: ShadowKind::Padding },
            ShadowRun { range: ByteRange { start: 8, len: 8 }, kind: ShadowKind::Pointer(pin) },
        ]
    );
}

#[test]
fn shadow_covers_image_exactly() {
    let mut st = ConstStore::with_pointer_bytes(8);
    let n = st.push_scalar(Scalar::Null { ty: ty(INT_PTR), kind: NullKind::Pointer }).unwrap();
    // Value bytes are 1; bytes under relocations and padding are 0.
    let zero = |i: usize| (8..16).contains(&i) || (20..32).contains(&i) || i >= 39;
    let bytes: Vec<u8> = (0..40).map(|i| u8::from(!zero(i))).collect();
    let relocs = [Reloc { at: 8, value: n }, Reloc { at: 24, value: n }];
    let padding = [ByteRange { start: 20, len: 4 }, ByteRange { start: 39, len: 1 }];
    let id = st
        .push_aggregate(ByteImage {
            ty: ty(900),
            bytes: &bytes,
            relocs: &relocs,
            padding: &padding,
            unions: &[],
        })
        .unwrap();
    let runs: Vec<(u32, u32, ShadowKind)> =
        st.shadow(id).map(|r| (r.range.start, r.range.len, r.kind)).collect();
    assert_eq!(
        runs,
        [
            (0, 8, ShadowKind::Value),
            (8, 8, ShadowKind::Pointer(n)),
            (16, 4, ShadowKind::Value),
            (20, 4, ShadowKind::Padding),
            (24, 8, ShadowKind::Pointer(n)),
            (32, 7, ShadowKind::Value),
            (39, 1, ShadowKind::Padding),
        ]
    );
    // An empty aggregate has no runs.
    let e = st.push_aggregate(agg(901, &[])).unwrap();
    assert_eq!(st.shadow(e).count(), 0);
}

#[test]
fn union_active_member_is_identity() {
    let mut st = ConstStore::default();
    let bytes = [0u8; 4];
    let pad = [ByteRange { start: 1, len: 3 }];
    let as_i = [UnionMember { at: 0, field: DeclId::new(106) }];
    let as_c = [UnionMember { at: 0, field: DeclId::new(107) }];
    let none_pad = [ByteRange { start: 0, len: 4 }];
    let i = st
        .push_aggregate(ByteImage {
            ty: ty(UNION),
            bytes: &bytes,
            relocs: &[],
            padding: &[],
            unions: &as_i,
        })
        .unwrap();
    let c = st
        .push_aggregate(ByteImage {
            ty: ty(UNION),
            bytes: &bytes,
            relocs: &[],
            padding: &pad,
            unions: &as_c,
        })
        .unwrap();
    let none = st
        .push_aggregate(ByteImage {
            ty: ty(UNION),
            bytes: &bytes,
            relocs: &[],
            padding: &none_pad,
            unions: &[],
        })
        .unwrap();
    assert!(i != c && c != none && i != none);
    // Same bytes and padding, different active member: still distinct.
    let c2 = st
        .push_aggregate(ByteImage {
            ty: ty(UNION),
            bytes: &bytes,
            relocs: &[],
            padding: &pad,
            unions: &as_i,
        })
        .unwrap();
    assert_ne!(c2, c);
    let strings = StringPool::new();
    let o = PrintOptions::DEFAULT;
    assert_eq!(st.display(i, &strings, &Env, &o), "{.i = 0}");
    assert_eq!(st.display(c, &strings, &Env, &o), "{.c = 0}");
    assert_eq!(st.display(none, &strings, &Env, &o), "{}");
    assert_eq!(
        st.display(i, &strings, &Env, &PrintOptions::with_style(PrintStyle::TemplateArg)),
        "U{.i = 0}"
    );
}

#[test]
fn dedup_of_a_thousand_int_array() {
    let mut st = ConstStore::default();
    let words: Vec<u32> = (0..1000).collect();
    let bytes = le(&words);
    let a = st.push_aggregate(agg(BIG, &bytes)).unwrap();
    let before = st.heap_bytes();
    // A separately built, equal image (as a second evaluation would produce).
    let bytes2 = le(&words);
    let b = st.push_aggregate(agg(BIG, &bytes2)).unwrap();
    assert_eq!(a, b);
    assert_eq!(st.len(), 1);
    let after = st.heap_bytes();
    assert_eq!(after.row("bytes").unwrap().live_bytes, 4000);
    assert_eq!(before.live_bytes(), after.live_bytes());
    // One different element is a different value.
    let mut words3 = words.clone();
    words3[999] = 0;
    let c = st.push_aggregate(agg(BIG, &le(&words3))).unwrap();
    assert_ne!(a, c);
    assert_eq!(st.heap_bytes().row("bytes").unwrap().live_bytes, 8000);
    // Same bytes, different type: different value.
    let d = st.push_aggregate(agg(BIG + 1000, &bytes)).unwrap();
    assert_ne!(a, d);
    assert_eq!(st.lookup(&ConstValue::Aggregate(agg(BIG, &bytes2))), Some(a));
    assert_eq!(st.lookup(&ConstValue::Aggregate(agg(BIG, &[1, 2]))), None);
}

#[test]
fn malformed_images_are_rejected() {
    let mut st = ConstStore::with_pointer_bytes(8);
    let n = st.push_scalar(Scalar::Null { ty: ty(INT_PTR), kind: NullKind::Pointer }).unwrap();
    let i = st.push_scalar(int(1)).unwrap();
    let z = [0u8; 32];
    let mut nz = [0u8; 32];
    nz[9] = 1;
    let malformed = |st: &mut ConstStore, img: ByteImage<'_>| {
        let r = st.push_aggregate(img);
        assert!(matches!(r, Err(ConstError::Malformed(_))), "{img:?} -> {r:?}");
        assert_eq!(st.lookup(&ConstValue::Aggregate(img)), None);
    };
    let r = |at, value| Reloc { at, value };
    let p = |start, len| ByteRange { start, len };
    let u = |at, f| UnionMember { at, field: DeclId::new(f) };
    malformed(&mut st, img(&z, &[r(8, n), r(0, n)], &[], &[])); // unsorted
    malformed(&mut st, img(&z, &[r(0, n), r(4, n)], &[], &[])); // overlapping
    malformed(&mut st, img(&z, &[r(28, n)], &[], &[])); // out of bounds
    malformed(&mut st, img(&nz, &[r(8, n)], &[], &[])); // non-zero under reloc
    malformed(&mut st, img(&z, &[r(0, i)], &[], &[])); // not pointer-like
    malformed(&mut st, img(&z, &[], &[p(0, 0)], &[])); // empty padding
    malformed(&mut st, img(&z, &[], &[p(0, 4), p(4, 4)], &[])); // not coalesced
    malformed(&mut st, img(&z, &[], &[p(8, 4), p(0, 4)], &[])); // unsorted
    malformed(&mut st, img(&z, &[], &[p(30, 4)], &[])); // out of bounds
    malformed(&mut st, img(&nz, &[], &[p(8, 4)], &[])); // non-zero padding
    malformed(&mut st, img(&z, &[r(8, n)], &[p(12, 8)], &[])); // padding over reloc
    malformed(&mut st, img(&z, &[r(8, n)], &[p(0, 9)], &[])); // padding over reloc start
    malformed(&mut st, img(&z, &[], &[], &[u(4, 1), u(0, 1)])); // unsorted unions
    malformed(&mut st, img(&z, &[], &[], &[u(0, 1), u(0, 1)])); // duplicate
    malformed(&mut st, img(&z, &[], &[], &[u(32, 1)])); // out of bounds
    // The well-formed neighbours are accepted.
    st.push_aggregate(img(
        &z,
        &[r(0, n), r(8, n)],
        &[p(16, 4), p(24, 8)],
        &[u(0, 1), u(0, 2), u(4, 1)],
    ))
    .unwrap();
    st.push_aggregate(img(&nz, &[r(0, n)], &[p(9, 1)], &[])).unwrap_err();
    st.push_aggregate(img(&nz, &[r(0, n)], &[p(10, 22)], &[])).unwrap();
}

#[test]
fn malformed_scalars_are_rejected() {
    let mut st = ConstStore::default();
    let bad = |st: &mut ConstStore, s: Scalar<'_>| {
        assert!(matches!(st.push_scalar(s), Err(ConstError::Malformed(_))), "{s:?}");
        assert_eq!(st.lookup(&ConstValue::Scalar(s)), None);
    };
    bad(&mut st, Scalar::Float { ty: ty(DOUBLE), kind: FloatKind::Half, bits: 1 << 16 });
    bad(&mut st, Scalar::Float { ty: ty(DOUBLE), kind: FloatKind::X87Extended, bits: 1 << 80 });
    bad(
        &mut st,
        Scalar::Pointer { ty: ty(FN_PTR), base: PtrBase::Function(DeclId::new(4)), offset: 1 },
    );
    bad(
        &mut st,
        Scalar::Pointer { ty: ty(INT_PTR), base: PtrBase::Const(ConstValueId::new(99)), offset: 0 },
    );
    assert!(st.is_empty());
    let e = ConstError::Malformed("x");
    assert_eq!(e.to_string(), "malformed constant value: x");
}

// ---------------------------------------------------------------------------
// Printing goldens (spec/ast.md §9.3)

fn show(st: &ConstStore, strings: &StringPool, id: ConstValueId) -> String {
    st.display(id, strings, &Env, &PrintOptions::DEFAULT)
}

fn show_targ(st: &ConstStore, strings: &StringPool, id: ConstValueId) -> String {
    st.display(id, strings, &Env, &PrintOptions::with_style(PrintStyle::TemplateArg))
}

#[test]
fn print_scalars() {
    let mut st = ConstStore::default();
    let mut strings = StringPool::new();
    let hi = strings.intern(StringEncoding::Ordinary, b"hi");
    let wide = strings.intern(StringEncoding::Utf16, &[b'a', 0, b'b', 0]);
    let mut push = |s: Scalar<'_>| st.push_scalar(s).unwrap();
    let cases: Vec<(ConstValueId, &str, &str)> = vec![
        (push(int(-5)), "-5", "-5"),
        (
            push(Scalar::Int {
                ty: ty(ULONG),
                signed: false,
                bits: IntBits::from_u128(64, u128::from(u64::MAX)),
            }),
            "18446744073709551615",
            "18446744073709551615UL",
        ),
        (
            push(Scalar::Int { ty: ty(UINT), signed: false, bits: IntBits::from_u128(32, 5) }),
            "5",
            "5U",
        ),
        (
            push(Scalar::Int { ty: ty(SHORT), signed: true, bits: IntBits::from_i128(16, -2) }),
            "-2",
            "(short)-2",
        ),
        (
            push(Scalar::Int { ty: ty(CHAR), signed: true, bits: IntBits::from_u128(8, 97) }),
            "97",
            "'a'",
        ),
        (
            push(Scalar::Int { ty: ty(CHAR), signed: true, bits: IntBits::from_u128(8, 39) }),
            "39",
            "'\\''",
        ),
        (
            push(Scalar::Int { ty: ty(ENUM), signed: true, bits: IntBits::from_i128(32, 1) }),
            "1",
            "E::A",
        ),
        (
            push(Scalar::Int { ty: ty(ENUM), signed: true, bits: IntBits::from_i128(32, 7) }),
            "7",
            "(E)7",
        ),
        (
            push(Scalar::Int {
                ty: ty(1128),
                signed: true,
                bits: IntBits::from_i128(128, i128::MIN),
            }),
            "-170141183460469231731687303715884105728",
            "-170141183460469231731687303715884105728",
        ),
        (push(Scalar::Bool { ty: ty(BOOL), value: true }), "true", "true"),
        (
            push(Scalar::Float {
                ty: ty(DOUBLE),
                kind: FloatKind::Double,
                bits: u128::from(0.1f64.to_bits()),
            }),
            "1e-1",
            "1e-1",
        ),
        (
            push(Scalar::Float {
                ty: ty(DOUBLE),
                kind: FloatKind::Double,
                bits: u128::from((-0.0f64).to_bits()),
            }),
            "-0e0",
            "-0e0",
        ),
        (
            push(Scalar::Float {
                ty: ty(DOUBLE),
                kind: FloatKind::Double,
                bits: u128::from(f64::INFINITY.to_bits()),
            }),
            "inf",
            "inf",
        ),
        (
            push(Scalar::Float {
                ty: ty(DOUBLE),
                kind: FloatKind::Double,
                bits: u128::from(f64::NAN.to_bits()),
            }),
            "nan",
            "nan",
        ),
        (
            push(Scalar::Float {
                ty: ty(DOUBLE),
                kind: FloatKind::Double,
                bits: u128::from(1e100f64.to_bits()),
            }),
            "1e100",
            "1e100",
        ),
        (push(Scalar::Null { ty: ty(INT_PTR), kind: NullKind::Pointer }), "nullptr", "nullptr"),
        (
            push(Scalar::Pointer {
                ty: ty(INT_PTR),
                base: PtrBase::Decl(DeclId::new(1)),
                offset: 0,
            }),
            "&x",
            "&x",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(INT_REF),
                base: PtrBase::Decl(DeclId::new(1)),
                offset: 0,
            }),
            "&x",
            "x",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(INT_REF),
                base: PtrBase::Temporary(DeclId::new(7)),
                offset: 0,
            }),
            "&ref_tmp",
            "ref_tmp",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(INT_PTR),
                base: PtrBase::Decl(DeclId::new(2)),
                offset: 8,
            }),
            "&arr[2]",
            "&arr[2]",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(INT_PTR),
                base: PtrBase::Decl(DeclId::new(2)),
                offset: 0,
            }),
            "&arr[0]",
            "&arr[0]",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(INT_PTR),
                base: PtrBase::Decl(DeclId::new(2)),
                offset: 12,
            }),
            "&arr[3]",
            "&arr[3]",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(FN_PTR),
                base: PtrBase::Decl(DeclId::new(2)),
                offset: 0,
            }),
            "&arr",
            "&arr",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(FN_PTR),
                base: PtrBase::Decl(DeclId::new(2)),
                offset: 6,
            }),
            "&arr+6",
            "&arr+6",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(INT_PTR),
                base: PtrBase::Decl(DeclId::new(2)),
                offset: 16,
            }),
            "&arr+16",
            "&arr+16",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(INT_PTR),
                base: PtrBase::Decl(DeclId::new(3)),
                offset: 4,
            }),
            "&s.in.y",
            "&s.in.y",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(INT_PTR),
                base: PtrBase::Decl(DeclId::new(3)),
                offset: 12,
            }),
            "&s.arr[1]",
            "&s.arr[1]",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(INT_PTR),
                base: PtrBase::Decl(DeclId::new(3)),
                offset: 0,
            }),
            "&s.x",
            "&s.x",
        ),
        (
            push(Scalar::Pointer { ty: ty(INT_PTR), base: PtrBase::StringLit(hi), offset: 0 }),
            "\"hi\"",
            "\"hi\"",
        ),
        (
            push(Scalar::Pointer { ty: ty(INT_PTR), base: PtrBase::StringLit(hi), offset: 1 }),
            "&\"hi\"[1]",
            "&\"hi\"[1]",
        ),
        (
            push(Scalar::Pointer { ty: ty(INT_PTR), base: PtrBase::StringLit(wide), offset: 2 }),
            "&u\"ab\"[1]",
            "&u\"ab\"[1]",
        ),
        (
            push(Scalar::Pointer {
                ty: ty(FN_PTR),
                base: PtrBase::Function(DeclId::new(4)),
                offset: 0,
            }),
            "&f",
            "&f",
        ),
        (
            push(Scalar::Pointer { ty: ty(FN_PTR), base: PtrBase::TypeInfo(ty(SHORT)), offset: 0 }),
            "&typeid(short)",
            "&typeid(short)",
        ),
        (
            push(Scalar::MemberPtr {
                ty: ty(MEMPTR),
                member: DeclId::new(5),
                kind: MemberKind::Data,
                derived: false,
                path: &[],
            }),
            "&S::m",
            "&S::m",
        ),
        (push(Scalar::Reflection { ty: ty(INFO), id: ReflectionId::new(1) }), "^^int", "^^int"),
    ];
    for (id, dump, targ) in cases {
        assert_eq!(show(&st, &strings, id), dump);
        assert_eq!(show_targ(&st, &strings, id), targ);
    }
    // NoEnv falls back to ids.
    let p = st
        .push_scalar(Scalar::Pointer {
            ty: ty(INT_PTR),
            base: PtrBase::Decl(DeclId::new(9)),
            offset: 4,
        })
        .unwrap();
    assert_eq!(st.display(p, &strings, &NoEnv, &PrintOptions::DEFAULT), "&#d9+4");
}

#[test]
fn print_aggregates() {
    let mut st = ConstStore::default();
    let strings = StringPool::new();
    let a3 = st.push_aggregate(agg(ARR3, &le(&[1, 2, 3]))).unwrap();
    assert_eq!(show(&st, &strings, a3), "{1, 2, 3}");
    assert_eq!(show_targ(&st, &strings, a3), "{1, 2, 3}");
    let n = st.push_aggregate(agg(NEST, &le(&[1, 2, 3, u32::MAX]))).unwrap();
    assert_eq!(show(&st, &strings, n), "{1, {2}, {3, -1}}");
    assert_eq!(show_targ(&st, &strings, n), "N{1, N::(anonymous){2}, {3, -1}}");
    // Bit-fields: a = -1 (3 bits), unnamed 5 bits skipped, b = 9 (4 bits).
    let bf_bytes = [0b0000_0111u8, 0b0000_1001, 0, 0];
    let bf = st.push_aggregate(agg(BITS, &bf_bytes)).unwrap();
    assert_eq!(show(&st, &strings, bf), "{-1, 9}");
    assert_eq!(show_targ(&st, &strings, bf), "T{-1, 9U}");
    // Depth limit.
    let deep = st.push_aggregate(agg(DEEP, &le(&[4]))).unwrap();
    assert_eq!(show(&st, &strings, deep), "{{{4}}}");
    let o = PrintOptions { max_depth: 2, ..PrintOptions::DEFAULT };
    assert_eq!(st.display(deep, &strings, &Env, &o), "{{{...}}}");
    let o0 = PrintOptions { max_depth: 0, ..PrintOptions::DEFAULT };
    assert_eq!(st.display(deep, &strings, &Env, &o0), "{...}");
    // Width limit.
    let words: Vec<u32> = (0..1000).collect();
    let big = st.push_aggregate(agg(BIG, &le(&words))).unwrap();
    let o = PrintOptions { max_elems: 3, ..PrintOptions::DEFAULT };
    assert_eq!(st.display(big, &strings, &Env, &o), "{0, 1, 2, ...}");
    let o = PrintOptions { max_elems: 0, ..PrintOptions::DEFAULT };
    assert_eq!(st.display(big, &strings, &Env, &o), "{...}");
    assert!(show(&st, &strings, big).ends_with(", 31, ...}"));
    let o = PrintOptions { max_elems: 1, ..PrintOptions::DEFAULT };
    assert_eq!(st.display(n, &strings, &Env, &o), "{1, ...}");
    // A long double member reads only its 10 value bytes.
    let mut ld = vec![0u8; 16];
    ld[7] = 0xc0;
    ld[8] = 0xff;
    ld[9] = 0x3f;
    let ld_pad = [ByteRange { start: 10, len: 6 }];
    let ldv = st
        .push_aggregate(ByteImage {
            ty: ty(LD_HOLDER),
            bytes: &ld,
            relocs: &[],
            padding: &ld_pad,
            unions: &[],
        })
        .unwrap();
    assert_eq!(show(&st, &strings, ldv), "{1.5e0}");
    // _BitInt(200) reads 200 value bits; bytes 25..32 are padding.
    let mut bi = vec![0xffu8; 25];
    bi[0] = 0xfd;
    bi.resize(32, 0);
    let bi_pad = [ByteRange { start: 25, len: 7 }];
    let biv = st
        .push_aggregate(ByteImage {
            ty: ty(BI_ARR),
            bytes: &bi,
            relocs: &[],
            padding: &bi_pad,
            unions: &[],
        })
        .unwrap();
    assert_eq!(show(&st, &strings, biv), "{-3}");
    // Unknown types print opaquely.
    let unk = st.push_aggregate(agg(901, &[1, 2])).unwrap();
    assert_eq!(show(&st, &strings, unk), "{...}");
    assert_eq!(st.display(a3, &strings, &NoEnv, &PrintOptions::DEFAULT), "{...}");
}

// ---------------------------------------------------------------------------
// Sizes and memory

#[test]
fn sizes() {
    const { assert!(ConstStore::NODE_BYTES <= 16) };
    assert_eq!(size_of::<ConstValueId>(), 4);
    assert_eq!(size_of::<Option<ConstValueId>>(), 4);
    assert_eq!(size_of::<Reloc>(), 8);
    assert_eq!(size_of::<ByteRange>(), 8);
    assert_eq!(size_of::<UnionMember>(), 8);
}

#[test]
fn memory_report() {
    let mut st = ConstStore::default();
    let empty = st.heap_bytes();
    assert_eq!(empty.row("nodes").unwrap().count, 0);
    for v in 0..100 {
        st.push_scalar(int(v)).unwrap();
    }
    let n = st.push_scalar(Scalar::Null { ty: ty(INT_PTR), kind: NullKind::Pointer }).unwrap();
    let bytes = [0u8; 16];
    let relocs = [Reloc { at: 0, value: n }];
    let padding = [ByteRange { start: 12, len: 4 }];
    st.push_aggregate(ByteImage {
        ty: ty(PAIR),
        bytes: &bytes,
        relocs: &relocs,
        padding: &padding,
        unions: &[],
    })
    .unwrap();
    let m = st.heap_bytes();
    let row = |name: &str| m.row(name).unwrap_or_else(|| panic!("row {name}"));
    assert_eq!(row("nodes").count, 102);
    assert_eq!(row("nodes").live_bytes, 102 * 16);
    assert_eq!(row("aggs").count, 1);
    assert_eq!(row("bytes").live_bytes, 16);
    assert_eq!(row("relocs").count, 1);
    assert_eq!(row("padding").count, 1);
    assert_eq!(row("intern").count, 102);
    assert!(row("intern").reserved_bytes >= row("intern").live_bytes);
    assert!(m.reserved_bytes() >= m.live_bytes());
    let text = m.to_string();
    assert!(text.contains("nodes") && text.contains("intern"), "{text}");
    // The pool is reported under `strings`.
    let mut sp = StringPool::new();
    sp.intern(StringEncoding::Ordinary, b"abc");
    let sm = sp.heap_bytes();
    assert_eq!(sm.row("bytes").unwrap().live_bytes, 3);
    assert_eq!(sm.row("entries").unwrap().count, 1);
}
