//! Behavioural tests for `kiln_snap` and `#[derive(Snap)]` (spec/snap.md).
//!
//! No C++ standard section applies: snapshots are compiler-internal state
//! (spec/snap.md "Scope"); each test names the spec/snap.md section it
//! backs.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::hash::BuildHasherDefault;
use std::marker::PhantomData;
use std::num::{NonZeroU8, NonZeroU32, NonZeroU64};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use kiln_snap::{
    ErrorKind, Reader, Schema, Snap, SnapError, Writer, from_bytes, schema_hash, to_bytes,
    to_bytes_canonical,
};

fn round<T: Snap>(v: &T) -> T {
    let bytes = to_bytes(v);
    match from_bytes::<T>(&bytes) {
        Ok(x) => x,
        Err(e) => panic!("round trip of {}: {e}", std::any::type_name::<T>()),
    }
}

// ----- derive shapes (spec/snap.md "The derive") ------------------------

#[derive(Snap, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Id(NonZeroU32);

#[derive(Snap, Debug, Clone, Copy, PartialEq)]
struct Rec {
    id: Id,
    parent: Option<Id>,
    flags: u8,
    kind: Kind,
    span: (u32, u32),
}

#[derive(Snap, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
enum Kind {
    A = 3,
    B = 1,
    C,
}

#[derive(Snap, Debug, Clone, PartialEq)]
enum Shape {
    Empty,
    Point(i32, i32),
    Named { name: String, sides: Vec<u16> },
    Nested(Box<Shape>),
}

#[derive(Snap, Debug, Clone, Copy, PartialEq)]
enum Fixed {
    Small(u8),
    Big { a: u64, b: u32 },
    None,
}

#[derive(Snap, Debug, Clone, PartialEq, Default)]
struct Unit;

#[derive(Snap, Debug, Clone, PartialEq)]
struct Tuple(pub u16, pub(crate) String, Unit);

#[derive(Snap, Debug, Clone, PartialEq)]
struct Generic<T: Clone, const N: usize>
where
    T: std::fmt::Debug,
{
    items: [T; N],
    rest: Vec<T>,
}

#[derive(Snap, Debug, Clone, PartialEq)]
struct GenTuple<A, B = u8>(A, B)
where
    A: Clone;

#[derive(Debug, Default, PartialEq)]
struct NotSnap(u32);

/// A field codec for `#[snap(with = ...)]`: stores the value plus one.
mod plus_one {
    use kiln_snap::{Reader, SnapError, Writer};

    #[allow(clippy::trivially_copy_pass_by_ref, reason = "the `with` signature")]
    pub fn put(v: &u32, w: &mut Writer) {
        w.u32(v + 1);
    }

    pub fn get(r: &mut Reader<'_>) -> Result<u32, SnapError> {
        Ok(r.u32()? - 1)
    }
}

#[derive(Snap, Debug, PartialEq)]
struct Attrs {
    kept: u32,
    #[snap(skip)]
    cache: NotSnap,
    #[snap(with = "plus_one")]
    shifted: u32,
    r#type: bool,
}

/// `bound` replaces the automatic `T: Snap` bounds.
#[derive(Snap, Debug, PartialEq)]
#[snap(bound = "T: Snap + Default")]
struct Bounded<T> {
    #[snap(skip)]
    t: T,
    n: u8,
}

#[derive(Snap, Debug, PartialEq)]
enum Empty {}

#[test]
fn derive_round_trips_every_shape() {
    let id = |n| Id(NonZeroU32::new(n).unwrap());
    let rec = Rec { id: id(7), parent: Some(id(1)), flags: 3, kind: Kind::C, span: (4, 9) };
    assert_eq!(round(&rec), rec);
    let rec2 = Rec { parent: None, kind: Kind::A, ..rec };
    assert_eq!(round(&rec2), rec2);
    for s in [
        Shape::Empty,
        Shape::Point(-1, 2),
        Shape::Named { name: "tri".into(), sides: vec![1, 2, 3] },
        Shape::Nested(Box::new(Shape::Nested(Box::new(Shape::Point(5, 6))))),
    ] {
        assert_eq!(round(&s), s);
    }
    for f in [Fixed::Small(9), Fixed::Big { a: u64::MAX, b: 2 }, Fixed::None] {
        assert_eq!(round(&f), f);
    }
    assert_eq!(round(&Unit), Unit);
    let t = Tuple(5, "x".into(), Unit);
    assert_eq!(round(&t), t);
    let g = Generic::<i64, 3> { items: [1, -2, 3], rest: vec![4, 5] };
    assert_eq!(round(&g), g);
    let gt: GenTuple<String> = GenTuple("a".into(), 4);
    assert_eq!(round(&gt), gt);
    let a = Attrs { kept: 1, cache: NotSnap(99), shifted: 41, r#type: true };
    let back = round(&a);
    assert_eq!(back, Attrs { cache: NotSnap(0), ..a });
    // `with` wrote its own encoding (41 + 1).
    let bytes = to_bytes(&Attrs { kept: 0, cache: NotSnap(0), shifted: 41, r#type: false });
    assert_eq!(&bytes[4..8], &42u32.to_le_bytes());
    let b = Bounded { t: 5u64, n: 2 };
    assert_eq!(round(&b), Bounded { t: 0, n: 2 });
}

#[test]
fn derive_enum_tag_is_the_variant_index() {
    // Explicit discriminants and `repr` do not change the encoding: the
    // tag is the declaration index.
    assert_eq!(to_bytes(&Kind::A), [0]);
    assert_eq!(to_bytes(&Kind::B), [1]);
    assert_eq!(to_bytes(&Kind::C), [2]);
    let e = from_bytes::<Empty>(&[0]).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::BadTag(0));
}

// ----- fixed sizes (spec/snap.md "Fixed-size fast path") -----------------

#[test]
fn fixed_sizes_are_computed() {
    assert_eq!(u32::SIZE, Some(4));
    assert_eq!(<char>::SIZE, Some(4));
    assert_eq!(<bool>::SIZE, Some(1));
    assert_eq!(<usize>::SIZE, Some(8));
    assert_eq!(<Option<NonZeroU32>>::SIZE, Some(4));
    assert_eq!(<Option<NonZeroU64>>::SIZE, Some(8));
    // A derived newtype keeps the niche.
    assert_eq!(Id::SIZE, Some(4));
    assert_eq!(<Option<Id>>::SIZE, Some(4));
    // Without a niche: a tag byte plus the zero-padded value.
    assert_eq!(<Option<u32>>::SIZE, Some(5));
    assert_eq!(<Option<Option<Id>>>::SIZE, Some(5));
    assert_eq!(Kind::SIZE, Some(1));
    assert_eq!(Rec::SIZE, Some(4 + 4 + 1 + 1 + 8));
    assert_eq!(Fixed::SIZE, Some(1 + 12));
    assert_eq!(<[Id; 3]>::SIZE, Some(12));
    assert_eq!(<(u8, Kind, ())>::SIZE, Some(2));
    assert_eq!(Unit::SIZE, Some(0));
    assert_eq!(<PhantomData<String>>::SIZE, Some(0));
    assert_eq!(<Cell<u16>>::SIZE, Some(2));
    assert_eq!(<OnceLock<Id>>::SIZE, Some(4));
    assert_eq!(<AtomicU32>::SIZE, Some(4));
    for none in [
        <Shape>::SIZE,
        <String>::SIZE,
        <Vec<u8>>::SIZE,
        <Box<u32>>::SIZE,
        <Rc<u32>>::SIZE,
        <Attrs>::SIZE,
    ] {
        assert_eq!(none, None);
    }
    // Every value of a fixed-size type encodes to exactly SIZE bytes.
    assert_eq!(to_bytes(&Fixed::Small(1)).len(), 13);
    assert_eq!(to_bytes(&Fixed::None).len(), 13);
    assert_eq!(to_bytes(&None::<u32>).len(), 5);
    assert_eq!(to_bytes(&None::<Id>), [0, 0, 0, 0]);
}

#[test]
fn fixed_size_vec_is_one_flat_block() {
    let v: Vec<Rec> = (1..=1000)
        .map(|n| Rec {
            id: Id(NonZeroU32::new(n).unwrap()),
            parent: NonZeroU32::new(n / 2).map(Id),
            flags: n as u8,
            kind: [Kind::A, Kind::B, Kind::C][n as usize % 3],
            span: (n, n * 2),
        })
        .collect();
    let bytes = to_bytes(&v);
    // LEB128(1000) is two bytes.
    assert_eq!(bytes.len(), 2 + 1000 * Rec::SIZE.unwrap());
    assert_eq!(round(&v), v);
    let boxed: Box<[Rec]> = v.clone().into_boxed_slice();
    assert_eq!(to_bytes(&boxed), bytes);
    assert_eq!(&*round(&boxed), &v[..]);
    let deque: VecDeque<Rec> = v.iter().copied().collect();
    assert_eq!(round(&deque), deque);
    // An invalid element in the block is reported at its own offset.
    let mut bad = bytes.clone();
    let at = 2 + 10 * Rec::SIZE.unwrap() + 9; // element 10's `kind`
    bad[at] = 7;
    let e = from_bytes::<Vec<Rec>>(&bad).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::BadTag(7));
    assert_eq!(e.offset(), at);
    assert_eq!(e.path(), ["Rec.kind"]);
}

// ----- standard impls (spec/snap.md "Standard impls") -------------------

#[test]
fn standard_impls_round_trip() {
    assert_eq!(
        round(&(1u8, -2i16, 3u32, -4i64, 5u128, -6i128, usize::MAX, isize::MIN)),
        (1u8, -2i16, 3u32, -4i64, 5u128, -6i128, usize::MAX, isize::MIN)
    );
    let nan = f64::from_bits(0x7ff8_0000_dead_beef);
    assert_eq!(round(&nan).to_bits(), nan.to_bits());
    assert_eq!(round(&-0.0f32).to_bits(), (-0.0f32).to_bits());
    assert_eq!(round(&('€', true, ())), ('€', true, ()));
    assert_eq!(round(&NonZeroU8::new(200)), NonZeroU8::new(200));
    let r: Result<u8, String> = Err("no".into());
    assert_eq!(round(&r), r);
    assert_eq!(round(&Ok::<u8, String>(3)), Ok(3));
    assert_eq!(round(&Box::<str>::from("héllo")), Box::from("héllo"));
    let cow: std::borrow::Cow<'static, str> = "borrowed".into();
    assert_eq!(round(&cow), cow);
    assert_eq!(round(&PathBuf::from("/a/b c/d.h")), PathBuf::from("/a/b c/d.h"));
    assert_eq!(round(&Cell::new(5u8)).get(), 5);
    assert_eq!(*round(&RefCell::new(vec![1u8])).borrow(), vec![1]);
    let once = OnceCell::new();
    assert_eq!(round(&once).get(), None::<&u8>);
    once.set(4u8).unwrap();
    assert_eq!(round(&once).get(), Some(&4));
    let lock = OnceLock::from(String::from("x"));
    assert_eq!(round(&lock).get().map(String::as_str), Some("x"));
    assert_eq!(*round(&Mutex::new(9u16)).lock().unwrap(), 9);
    assert_eq!(*round(&RwLock::new(10u16)).read().unwrap(), 10);
    assert_eq!(round(&AtomicU32::new(11)).load(Ordering::Relaxed), 11);
    let bt: BTreeMap<String, Vec<u8>> = [("b".into(), vec![2]), ("a".into(), vec![])].into();
    assert_eq!(round(&bt), bt);
    let bs: BTreeSet<i32> = [3, -1, 2].into();
    assert_eq!(round(&bs), bs);
    assert_eq!(round(&[Some(1u8), None, Some(3)]), [Some(1u8), None, Some(3)]);
    assert_eq!(round(&[String::from("a"), String::from("b")]), ["a", "b"]);
}

/// An `FxHash` copy (the workspace's own lives in `kiln_source`, above
/// this crate).
#[derive(Default, Clone, Copy)]
struct Fx(u64);

impl std::hash::Hasher for Fx {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    fn write_u32(&mut self, i: u32) {
        self.write_u64(u64::from(i));
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

type FxBuild = BuildHasherDefault<Fx>;

#[test]
fn hash_maps_round_trip_and_canonical_is_order_free() {
    let mut a: HashMap<u32, String, FxBuild> = HashMap::default();
    let mut b: HashMap<u32, String, FxBuild> = HashMap::default();
    for n in 0..500u32 {
        a.insert(n, n.to_string());
    }
    for n in (0..500u32).rev() {
        b.insert(n, n.to_string());
    }
    for n in (0..500u32).step_by(3) {
        a.remove(&n);
        b.remove(&n);
    }
    b.shrink_to_fit();
    let ra = round(&a);
    assert_eq!(ra, a);
    // Same capacity after a round trip.
    assert_eq!(ra.capacity(), a.capacity());
    assert_eq!(to_bytes_canonical(&a), to_bytes_canonical(&b));
    assert_eq!(to_bytes_canonical(&ra), to_bytes_canonical(&a));
    let set: HashSet<String> = ["x", "y", "z"].iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(round(&set), set);
    let mut hb: hashbrown::HashMap<u64, u8, FxBuild> = hashbrown::HashMap::default();
    hb.extend((0..100).map(|n| (n * 7, n as u8)));
    assert_eq!(round(&hb), hb);
    let hs: hashbrown::HashSet<u16, FxBuild> = (0..50).collect();
    assert_eq!(round(&hs), hs);
}

// ----- Rc / Arc sharing (spec/snap.md "Shared pointers") ----------------

#[derive(Snap, Debug)]
struct Def {
    name: String,
    body: Vec<u32>,
}

#[derive(Snap, Debug)]
struct Table {
    a: Rc<Def>,
    b: Rc<Def>,
    c: Rc<Def>,
    stack: Vec<Rc<Def>>,
    names: Vec<Rc<str>>,
    slice: (Rc<[u8]>, Rc<[u8]>),
    arc: (Arc<Def>, Arc<Def>),
    arc_str: (Arc<str>, Arc<str>),
}

#[test]
fn rc_sharing_is_preserved() {
    let d1 = Rc::new(Def { name: "one".into(), body: vec![1] });
    let d2 = Rc::new(Def { name: "one".into(), body: vec![1] }); // equal, not shared
    let n: Rc<str> = Rc::from("n");
    let s: Rc<[u8]> = Rc::from(&b"xy"[..]);
    let ad = Arc::new(Def { name: "a".into(), body: vec![] });
    let astr: Arc<str> = Arc::from("s");
    let t = Table {
        a: d1.clone(),
        b: d2.clone(),
        c: d1.clone(),
        stack: vec![d2.clone(), d1.clone()],
        names: vec![n.clone(), n.clone(), Rc::from("n")],
        slice: (s.clone(), s.clone()),
        arc: (ad.clone(), ad.clone()),
        arc_str: (astr.clone(), Arc::from("s")),
    };
    let back = round(&t);
    assert!(Rc::ptr_eq(&back.a, &back.c));
    assert!(!Rc::ptr_eq(&back.a, &back.b));
    assert!(Rc::ptr_eq(&back.stack[0], &back.b));
    assert!(Rc::ptr_eq(&back.stack[1], &back.a));
    assert!(Rc::ptr_eq(&back.names[0], &back.names[1]));
    assert!(!Rc::ptr_eq(&back.names[0], &back.names[2]));
    assert!(Rc::ptr_eq(&back.slice.0, &back.slice.1));
    assert!(Arc::ptr_eq(&back.arc.0, &back.arc.1));
    assert!(!Arc::ptr_eq(&back.arc_str.0, &back.arc_str.1));
    assert_eq!(back.a.name, "one");
    assert_eq!(&*back.names[2], "n");
    assert_eq!(&*back.slice.0, b"xy");
    // Each shared allocation is written once: the second `Rc<Def>` of d1
    // is a one-byte back-reference.
    let one = to_bytes(&(d1.clone(),));
    let two = to_bytes(&(d1.clone(), d1.clone()));
    assert_eq!(two.len(), one.len() + 1);
    // Strong counts: the restored table holds the only references.
    assert_eq!(Rc::strong_count(&back.a), 3);
}

#[test]
fn shared_errors() {
    #[derive(Snap, Debug)]
    struct Node(Vec<Rc<Node>>);
    // A back-reference to an index never written.
    let e = from_bytes::<Rc<u8>>(&[5]).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::SharedOutOfRange(4));
    // A back-reference to an object of another type: `(Rc<u8>, Rc<u16>)`
    // whose second element points at the first.
    let e = from_bytes::<(Rc<u8>, Rc<u16>)>(&[0, 7, 1]).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::SharedWrongType);
    assert_eq!(e.offset(), 3);
    // A cycle: an `Rc<Vec<Rc<..>>>` element referring to its own parent.
    let e = from_bytes::<Rc<Node>>(&[0, 1, 1]).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::SharedCycle);
}

// ----- errors (spec/snap.md "Errors") -----------------------------------

#[derive(Snap, Debug, Clone, PartialEq)]
struct Doc {
    title: String,
    shapes: Vec<Shape>,
    recs: Vec<Rec>,
    tags: BTreeMap<u16, Option<String>>,
    map: HashMap<String, Vec<i64>, FxBuild>,
    shared: Vec<Rc<str>>,
    ch: char,
}

fn sample_doc() -> Doc {
    let s: Rc<str> = Rc::from("shared");
    Doc {
        title: "doc".into(),
        shapes: vec![Shape::Point(1, 2), Shape::Named { name: "n".into(), sides: vec![3] }],
        recs: vec![Rec {
            id: Id(NonZeroU32::new(3).unwrap()),
            parent: None,
            flags: 0,
            kind: Kind::B,
            span: (0, 1),
        }],
        tags: [(1, Some("t".into())), (2, None)].into(),
        map: [("k".to_owned(), vec![-1, 2])].into_iter().collect(),
        shared: vec![s.clone(), s],
        ch: 'z',
    }
}

#[test]
fn every_truncation_is_an_error() {
    let doc = sample_doc();
    let bytes = to_bytes(&doc);
    assert_eq!(round(&doc), doc);
    for n in 0..bytes.len() {
        let e = from_bytes::<Doc>(&bytes[..n]).unwrap_err();
        assert!(e.offset() <= n, "{n}: {e}");
    }
    let mut long = bytes.clone();
    long.push(0);
    let e = from_bytes::<Doc>(&long).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::TrailingBytes(1));
}

#[test]
fn corrupted_input_never_panics() {
    let bytes = to_bytes(&sample_doc());
    // Every single-byte change at every position, to a few values.
    for i in 0..bytes.len() {
        for v in [0u8, 1, 2, 0x7f, 0x80, 0xff] {
            let mut b = bytes.clone();
            b[i] = v;
            let _ = from_bytes::<Doc>(&b);
        }
    }
}

#[test]
fn invalid_values_are_reported_with_type_and_offset() {
    let check = |r: Result<(), SnapError>, kind: ErrorKind, ty: &str, at: usize| {
        let e = r.unwrap_err();
        assert_eq!(e.kind(), &kind, "{e}");
        assert!(e.type_name().contains(ty), "{e}");
        assert_eq!(e.offset(), at, "{e}");
    };
    check(from_bytes::<bool>(&[2]).map(drop), ErrorKind::BadValue("bool"), "bool", 0);
    check(
        from_bytes::<(u8, char)>(&[0, 0, 0xd8, 0, 0]).map(drop),
        ErrorKind::BadValue("char"),
        "char",
        1,
    );
    check(
        from_bytes::<NonZeroU32>(&[0, 0, 0, 0]).map(drop),
        ErrorKind::BadValue("zero NonZero"),
        "NonZero",
        0,
    );
    check(from_bytes::<String>(&[2, 0xc3, 0x28]).map(drop), ErrorKind::BadValue("UTF-8"), "str", 1);
    check(from_bytes::<Option<u8>>(&[2, 0]).map(drop), ErrorKind::BadTag(2), "Option", 0);
    check(from_bytes::<Shape>(&[9]).map(drop), ErrorKind::BadTag(9), "Shape", 0);
    // A length larger than the bytes left: rejected before allocating.
    check(
        from_bytes::<Vec<u64>>(&[0xff, 0xff, 0xff, 0xff, 0x0f]).map(drop),
        ErrorKind::LengthTooLarge(0xffff_ffff),
        "u64",
        0,
    );
    check(
        from_bytes::<Vec<String>>(&[3, 0, 0]).map(drop),
        ErrorKind::LengthTooLarge(3),
        "String",
        0,
    );
    check(from_bytes::<u32>(&[1, 2]).map(drop), ErrorKind::Truncated { needed: 2 }, "u32", 0);
    // An over-long LEB128 length.
    let e = from_bytes::<Vec<u8>>(&[0xff; 11]).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::BadValue("LEB128 length"));
    // A duplicate key in a map.
    let e = from_bytes::<BTreeSet<u8>>(&[2, 1, 1]).unwrap_err();
    assert!(matches!(e.kind(), ErrorKind::Invalid(m) if m == "duplicate key"));
    // The field path names the enclosing fields, outermost first.
    let mut b = to_bytes(&sample_doc());
    let pos = b.iter().rposition(|&x| x == b'z').unwrap();
    b[pos..pos + 4].copy_from_slice(&0xd800u32.to_le_bytes());
    let e = from_bytes::<Doc>(&b).unwrap_err();
    assert_eq!(e.path(), ["Doc.ch"]);
    let mut b = to_bytes(&sample_doc());
    let pos = b.iter().position(|&x| x == b'n').unwrap(); // Shape::Named name
    b[pos] = 0xff;
    let e = from_bytes::<Doc>(&b).unwrap_err();
    assert_eq!(e.path(), ["Doc.shapes", "Shape::Named.name"]);
    assert!(e.to_string().contains("(in Doc.shapes > Shape::Named.name)"), "{e}");
}

#[test]
fn reader_and_writer_primitives() {
    let mut w = Writer::new();
    w.length(0);
    w.length(127);
    w.length(128);
    w.length(usize::MAX);
    w.str("ab");
    w.zeros(2);
    let bytes = w.into_bytes();
    let mut r = Reader::new(&bytes);
    assert_eq!(r.length().unwrap(), 0);
    assert_eq!(r.length().unwrap(), 127);
    assert_eq!(r.length().unwrap(), 128);
    assert_eq!(r.length().unwrap(), usize::MAX);
    assert_eq!(r.str().unwrap(), "ab");
    r.skip(2).unwrap();
    r.finish().unwrap();
}

// ----- schema hash (spec/snap.md "Schema hash") -------------------------

/// The schema hash of `T` with every `module::` occurrence replaced by
/// `M::`, so that two copies of a type in different modules compare by
/// structure only.
fn shape_hash<T: Snap>(module: &str) -> u64 {
    let mut s = Schema::new();
    T::schema(&mut s);
    let mut d: Vec<String> = s.descriptors().iter().map(|d| d.replace(module, "M::")).collect();
    d.sort();
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in d.join("\n").bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

macro_rules! schema_variant {
    ($m:ident { $($inner:tt)* } $($root:tt)*) => {
        #[allow(dead_code, reason = "only the schema is read")]
        mod $m {
            use kiln_snap::Snap;
            #[derive(Snap)]
            pub struct Inner { $($inner)* }
            #[derive(Snap)]
            pub struct Root { $($root)* }
        }
    };
}

schema_variant!(base { a: u32, b: String } x: Vec<Inner>, y: Option<u8>);
schema_variant!(same { a: u32, b: String } x: Vec<Inner>, y: Option<u8>);
schema_variant!(added { a: u32, b: String, c: u8 } x: Vec<Inner>, y: Option<u8>);
schema_variant!(removed { a: u32 } x: Vec<Inner>, y: Option<u8>);
schema_variant!(renamed { a: u32, bb: String } x: Vec<Inner>, y: Option<u8>);
schema_variant!(reordered { b: String, a: u32 } x: Vec<Inner>, y: Option<u8>);
schema_variant!(retyped { a: u64, b: String } x: Vec<Inner>, y: Option<u8>);
schema_variant!(root_retyped { a: u32, b: String } x: Vec<Inner>, y: Option<u16>);
schema_variant!(skipped { a: u32, #[snap(skip)] b: String } x: Vec<Inner>, y: Option<u8>);

mod enum_a {
    #[derive(kiln_snap::Snap)]
    pub enum E {
        A,
        B(u8),
    }
}
mod enum_b {
    #[derive(kiln_snap::Snap)]
    pub enum E {
        B(u8),
        A,
    }
}
mod enum_c {
    #[derive(kiln_snap::Snap)]
    #[repr(u8)]
    pub enum E {
        A = 2,
        B(u8),
    }
}

#[test]
fn schema_hash_sees_every_field_change() {
    let h = |m: &str| match m {
        "base" => shape_hash::<base::Root>("base::"),
        "same" => shape_hash::<same::Root>("same::"),
        "added" => shape_hash::<added::Root>("added::"),
        "removed" => shape_hash::<removed::Root>("removed::"),
        "renamed" => shape_hash::<renamed::Root>("renamed::"),
        "reordered" => shape_hash::<reordered::Root>("reordered::"),
        "retyped" => shape_hash::<retyped::Root>("retyped::"),
        "root_retyped" => shape_hash::<root_retyped::Root>("root_retyped::"),
        "skipped" => shape_hash::<skipped::Root>("skipped::"),
        _ => unreachable!(),
    };
    // The control: an identical copy hashes the same.
    assert_eq!(h("base"), h("same"));
    for m in ["added", "removed", "renamed", "reordered", "retyped", "root_retyped", "skipped"] {
        assert_ne!(h("base"), h(m), "{m}");
    }
    assert_ne!(shape_hash::<enum_a::E>("enum_a::"), shape_hash::<enum_b::E>("enum_b::"));
    assert_ne!(shape_hash::<enum_a::E>("enum_a::"), shape_hash::<enum_c::E>("enum_c::"));
    // Deterministic, and the root's own name is part of it.
    assert_eq!(schema_hash::<base::Root>(), schema_hash::<base::Root>());
    assert_ne!(schema_hash::<base::Root>(), schema_hash::<same::Root>());
}

#[test]
fn schema_descriptors_hold_names_only() {
    #[derive(Snap)]
    #[allow(dead_code, reason = "only the schema is read")]
    struct Tiny {
        a: u32,
        #[snap(skip)]
        b: Vec<u8>,
        c: Option<bool>,
    }
    let mut s = Schema::new();
    Tiny::schema(&mut s);
    let tiny = std::any::type_name::<Tiny>();
    let mut want = vec![
        "bool = bool".to_owned(),
        "core::option::Option<bool> = option 0:bool".to_owned(),
        format!("{tiny} = struct a:u32 b:skip:alloc::vec::Vec<u8> c:core::option::Option<bool>"),
        "u32 = int".to_owned(),
    ];
    want.sort();
    assert_eq!(s.descriptors(), want);
    // Registering twice is a no-op.
    Tiny::schema(&mut s);
    assert_eq!(s.descriptors().len(), 4);
}

// ----- streaming reads (spec/snap.md "Streaming reads") -----------------

/// A source that hands out at most `step` bytes per `read`, as a pipe or
/// a short file read may.
struct Trickle<'a> {
    data: &'a [u8],
    step: usize,
}

impl std::io::Read for Trickle<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(self.step).min(self.data.len());
        buf[..n].copy_from_slice(&self.data[..n]);
        self.data = &self.data[n..];
        Ok(n)
    }
}

fn stream<T: Snap>(bytes: &[u8], len: usize, step: usize) -> Result<T, SnapError> {
    let mut r = Reader::streaming(Trickle { data: bytes, step }, len, 0);
    let v = T::get(&mut r)?;
    r.finish()?;
    Ok(v)
}

/// A value with fixed-size runs, strings, records and shared parts.
type Mixed = (Vec<u64>, String, Vec<String>, Vec<Rec>, Vec<Rc<Vec<u32>>>);

#[test]
fn streaming_reader_decodes_what_a_slice_reader_does() {
    // Fixed-size items spanning many windows (the batched fast path),
    // strings longer than a window, and shared objects across windows.
    let big: Vec<u64> = (0..200_000u64).map(|i| i.wrapping_mul(0x9e37_79b9)).collect();
    let long = "x".repeat(kiln_snap::WINDOW * 2 + 17);
    let words: Vec<String> = (0..5000).map(|i| format!("word{i}")).collect();
    let rec = Rec {
        id: Id(NonZeroU32::new(7).unwrap()),
        parent: None,
        flags: 3,
        kind: Kind::C,
        span: (1, 2),
    };
    let recs = vec![rec; 70_000];
    let shared = Rc::new(vec![1u32, 2, 3]);
    let v: Mixed = (big, long, words, recs, vec![Rc::clone(&shared), Rc::clone(&shared)]);
    let bytes = to_bytes(&v);
    for step in [1 << 20, 4093, 1] {
        let got: Mixed = stream(&bytes, bytes.len(), step).unwrap();
        assert_eq!(got.0, v.0);
        assert_eq!(got.1, v.1);
        assert_eq!(got.2, v.2);
        assert_eq!(got.3, v.3);
        assert!(Rc::ptr_eq(&got.4[0], &got.4[1]));
        assert_eq!(*got.4[0], *shared);
    }
}

#[test]
fn streaming_reader_reports_a_short_source_and_trailing_bytes() {
    let bytes = to_bytes(&vec![5u32; 100_000]);
    // The source ends before the promised length: an I/O error, no panic.
    let short = &bytes[..bytes.len() - 3];
    let e = stream::<Vec<u32>>(short, bytes.len(), 1 << 16).unwrap_err();
    assert!(matches!(e.kind(), ErrorKind::Io(_)), "{e}");
    // A length shorter than the encoding: truncated, as a slice reader.
    let e = stream::<Vec<u32>>(&bytes, bytes.len() - 3, 1 << 16).unwrap_err();
    assert!(matches!(e.kind(), ErrorKind::LengthTooLarge(_) | ErrorKind::Truncated { .. }), "{e}");
    // Bytes left over: trailing, with the count of the unread source.
    let mut longer = bytes.clone();
    longer.extend_from_slice(&[0; 9]);
    let e = stream::<Vec<u32>>(&longer, longer.len(), 1 << 16).unwrap_err();
    assert!(matches!(e.kind(), ErrorKind::TrailingBytes(9)), "{e}");
}
