//! Hand-built hierarchies probing the Itanium vtable builder (spec/abi.md
//! §8, Itanium C++ ABI §2.5–§2.7): component order, address points,
//! `this`- and return-adjusting thunks, vbase-offset offsets, method
//! indices, construction vtables and the VTT. The expected values were
//! transcribed from `clang++-19 -fdump-vtable-layouts` (and, for the VTT,
//! from the `@_ZTT` initializer clang emits); when clang is installed
//! every dump is also compared with clang's byte for byte.

#![allow(clippy::many_single_char_names, reason = "short names mirror the C++ under test")]

mod common;

use std::fmt::Write as _;

use common::{
    DtorDef, FieldDef, Item, MethodDef, MethodKind, Model, RecDef, Ty, clang_available,
    run_clang_vtables, vtable_blocks,
};
use kiln_abi::subobjects;
use kiln_abi::target::{Target, target};
use kiln_abi::vtable::{
    SubobjectId, Thunk, VTableComponent, VTableLayout, construction_vtable,
    dump_construction_vtable, dump_vtable_layout, vtable_layout, vtt, vtt_vtables,
};
use kiln_ast::decl::TagKind;
use kiln_ast::dump::AstNames;
use kiln_ast::types::BuiltinType as B;
use kiln_ast::{Ast, DeclId};

const BOTH: [&str; 2] = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];

fn int(name: &str) -> FieldDef {
    FieldDef::new(name, Ty::B(B::Int))
}

/// `virtual ret name();`
fn vm_ret(name: &str, ret: Ty) -> MethodDef {
    MethodDef {
        name: name.to_owned(),
        ret,
        params: Vec::new(),
        is_const: false,
        virtual_written: true,
        pure: false,
        kind: MethodKind::Normal,
        is_virtual: false,
    }
}

fn vm(name: &str) -> MethodDef {
    vm_ret(name, Ty::B(B::Void))
}

/// `struct name : bases... { methods...; fields }` with out-of-line
/// constructor and member definitions, so clang emits the vtables.
fn class(
    m: &mut Model,
    name: &str,
    bases: &[usize],
    virt: &[usize],
    methods: Vec<MethodDef>,
    fields: Vec<FieldDef>,
) -> usize {
    let mut r = RecDef::new(name, TagKind::Struct, fields);
    r.bases = bases.to_vec();
    r.virtual_bases = virt.to_vec();
    r.methods = methods;
    r.out_of_line = true;
    r.user_ctor = true;
    m.push(Item::Record(r))
}

fn name(ast: &Ast, d: DeclId) -> String {
    let mut s = String::new();
    AstNames::new(ast).write_qualified_name(d, &mut s);
    s
}

fn component(ast: &Ast, c: &VTableComponent) -> String {
    match *c {
        VTableComponent::VCallOffset(o) => format!("vcall {o}"),
        VTableComponent::VBaseOffset(o) => format!("vbase {o}"),
        VTableComponent::OffsetToTop(o) => format!("top {o}"),
        VTableComponent::Rtti(d) => format!("rtti {}", name(ast, d)),
        VTableComponent::Func(d) => name(ast, d),
        VTableComponent::CompleteDtor(d) => format!("D1 {}", name(ast, d)),
        VTableComponent::DeletingDtor(d) => format!("D0 {}", name(ast, d)),
        VTableComponent::UnusedFunc(d) => format!("unused {}", name(ast, d)),
    }
}

fn thunk(ast: &Ast, t: &Thunk) -> String {
    let mut s = format!("this {:?} ret {:?}", t.this, t.ret);
    if let Some(m) = t.method {
        let _ = write!(s, " method {}", name(ast, m));
    }
    if let Some(c) = t.this_class {
        let _ = write!(s, " this_class {}", name(ast, c));
    }
    s
}

/// A vtable group rendered for comparison: components, `(subobject class,
/// offset) → address point`, thunks by component, per-method thunks,
/// vbase-offset offsets and method indices.
#[derive(Debug, PartialEq, Eq)]
struct Shape {
    components: Vec<String>,
    starts: Vec<u32>,
    address_points: Vec<(String, u64, u32)>,
    thunks: Vec<(u32, String)>,
    method_thunks: Vec<(String, String)>,
    vbase_offset_offsets: Vec<(String, i64)>,
    method_indices: Vec<(String, u32)>,
}

fn shape(ast: &Ast, md: DeclId, tgt: &Target, l: &VTableLayout) -> Shape {
    let subs = subobjects(ast, md, tgt);
    let sub = |id: SubobjectId| {
        let (s, off) = &subs[id.index()];
        (name(ast, s.class), *off)
    };
    let mut address_points: Vec<(String, u64, u32)> = l
        .address_points
        .iter()
        .map(|&(id, i)| {
            let (c, o) = sub(id);
            (c, o, i)
        })
        .collect();
    address_points.sort();
    let mut method_indices: Vec<(String, u32)> =
        l.method_indices.iter().map(|&(d, i)| (name(ast, d), i)).collect();
    method_indices.sort();
    Shape {
        components: l.components.iter().map(|c| component(ast, c)).collect(),
        starts: l.vtable_starts.clone(),
        address_points,
        thunks: l.thunks.iter().map(|(i, t)| (*i, thunk(ast, t))).collect(),
        method_thunks: l
            .method_thunks
            .iter()
            .map(|(d, t)| (name(ast, *d), thunk(ast, t)))
            .collect(),
        vbase_offset_offsets: l
            .vbase_offset_offsets
            .iter()
            .map(|&(d, o)| (name(ast, d), o))
            .collect(),
        method_indices,
    }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|&x| x.to_owned()).collect()
}

fn ap(v: &[(&str, u64, u32)]) -> Vec<(String, u64, u32)> {
    v.iter().map(|&(c, o, i)| (c.to_owned(), o, i)).collect()
}

fn named<T: Copy>(v: &[(&str, T)]) -> Vec<(String, T)> {
    v.iter().map(|&(c, x)| (c.to_owned(), x)).collect()
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter().map(|&(a, b)| (a.to_owned(), b.to_owned())).collect()
}

/// Every vtable (and VTT construction vtable) of `m` dumps exactly as
/// clang dumps it.
fn cross_check(m: &Model, triple: &str) {
    if !clang_available() {
        return;
    }
    let src = m.cpp();
    let theirs = vtable_blocks(
        &run_clang_vtables(triple, &src).unwrap_or_else(|e| panic!("clang: {e}\n{src}")),
    );
    let tgt = target(triple).expect("target");
    let (ast, decls) = m.ast(&tgt);
    let mut n = 0;
    for (i, r) in m.records() {
        let Ok(ours) = dump_vtable_layout(&ast, decls[i], &tgt) else { continue };
        let mut all = vec![ours];
        for sub in vtt_vtables(&ast, decls[i], &tgt).expect("vtt").into_iter().skip(1) {
            all.push(dump_construction_vtable(&ast, decls[i], sub, &tgt).expect("construction"));
        }
        for ours in all {
            let head = ours.lines().next().unwrap_or_default();
            assert_eq!(theirs.get(head), Some(&ours), "{triple} {}\n{src}", r.name);
            n += 1;
        }
    }
    assert_eq!(n, theirs.len(), "{triple}: clang dumped vtables kiln did not\n{src}");
}

fn layout_shape(m: &Model, triple: &str, i: usize) -> Shape {
    let tgt = target(triple).expect("target");
    let (ast, decls) = m.ast(&tgt);
    let l = vtable_layout(&ast, decls[i], &tgt).expect("vtable");
    shape(&ast, decls[i], &tgt, &l)
}

/// §2.5.2: a secondary vtable's slot for an overrider declared in the
/// derived class gets a non-virtual `this`-adjusting thunk (the
/// adjustment is minus the base's offset), named after the overridden
/// method.
#[test]
fn non_virtual_this_thunk() {
    let mut m = Model::default();
    let a = class(&mut m, "A", &[], &[], vec![vm("f")], vec![int("a")]);
    let b = class(&mut m, "B", &[], &[], vec![vm("f")], vec![int("b")]);
    let c = class(&mut m, "C", &[a, b], &[], vec![vm("f")], vec![]);
    for t in BOTH {
        cross_check(&m, t);
        let th = "this (-16, 0) ret (0, 0) method B::f this_class B";
        let want = Shape {
            components: s(&["top 0", "rtti C", "C::f", "top -16", "rtti C", "C::f"]),
            starts: vec![0, 3],
            address_points: ap(&[("A", 0, 2), ("B", 16, 5), ("C", 0, 2)]),
            thunks: vec![(5, th.to_owned())],
            method_thunks: pairs(&[("C::f", th)]),
            vbase_offset_offsets: vec![],
            method_indices: named(&[("C::f", 0)]),
        };
        assert_eq!(layout_shape(&m, t, c), want, "{t}");
    }
}

/// §2.5.2–§2.7: an overrider of a virtual base's method gets a vcall
/// offset in the base's secondary vtable and a virtual `this` thunk
/// reading it; the vbase offset sits before the primary address point.
#[test]
fn virtual_this_thunk_and_vbase_offset() {
    let mut m = Model::default();
    let v = class(&mut m, "V", &[], &[], vec![vm("g")], vec![int("v")]);
    let l = class(&mut m, "L", &[v], &[v], vec![vm("g")], vec![int("l")]);
    for t in BOTH {
        cross_check(&m, t);
        let th = "this (0, -24) ret (0, 0) method V::g this_class V";
        let want = Shape {
            components: s(&[
                "vbase 16",
                "top 0",
                "rtti L",
                "L::g",
                "vcall -16",
                "top -16",
                "rtti L",
                "L::g",
            ]),
            starts: vec![0, 4],
            address_points: ap(&[("L", 0, 3), ("V", 16, 7)]),
            thunks: vec![(7, th.to_owned())],
            method_thunks: pairs(&[("L::g", th)]),
            vbase_offset_offsets: named(&[("V", -24)]),
            method_indices: named(&[("L::g", 0)]),
        };
        assert_eq!(layout_shape(&m, t, l), want, "{t}");
    }
}

/// §2.6: a diamond over a virtual base. The complete vtable group, the
/// construction vtables of `L`-in-`D` and `R`-in-`D` (whose vbase and
/// vcall offsets reflect `D`'s layout, not `L`'s or `R`'s), and the VTT
/// in clang's order: primary, secondary VTTs, then secondary virtual
/// pointers.
#[test]
fn diamond_construction_vtables_and_vtt() {
    let mut m = Model::default();
    let v = class(&mut m, "V", &[], &[], vec![vm("g")], vec![int("v")]);
    let l = class(&mut m, "L", &[v], &[v], vec![vm("g")], vec![int("l")]);
    let r = class(&mut m, "R", &[v], &[v], vec![], vec![int("r")]);
    let d = class(&mut m, "D", &[l, r], &[], vec![], vec![]);
    for t in BOTH {
        cross_check(&m, t);
        let tgt = target(t).expect("target");
        let (ast, decls) = m.ast(&tgt);
        let md = decls[d];
        let th = "this (0, -24) ret (0, 0) method V::g this_class V";
        let complete = Shape {
            components: s(&[
                "vbase 32",
                "top 0",
                "rtti D",
                "L::g",
                "vbase 16",
                "top -16",
                "rtti D",
                "vcall -32",
                "top -32",
                "rtti D",
                "L::g",
            ]),
            starts: vec![0, 4, 7],
            address_points: ap(&[("D", 0, 3), ("L", 0, 3), ("R", 16, 7), ("V", 32, 10)]),
            thunks: vec![(10, th.to_owned())],
            method_thunks: vec![],
            vbase_offset_offsets: named(&[("V", -24)]),
            method_indices: named(&[("L::g", 0)]),
        };
        let l_layout = vtable_layout(&ast, md, &tgt).expect("vtable");
        assert_eq!(shape(&ast, md, &tgt, &l_layout), complete, "{t}");

        let groups = vtt_vtables(&ast, md, &tgt).expect("vtt vtables");
        let subs = subobjects(&ast, md, &tgt);
        let at = |id: SubobjectId| {
            let (s, off) = &subs[id.index()];
            (name(&ast, s.class), *off)
        };
        let groups: Vec<_> = groups.into_iter().map(at).collect();
        assert_eq!(groups, named(&[("D", 0), ("L", 0), ("R", 16)]), "{t}");

        let ids = vtt_vtables(&ast, md, &tgt).expect("vtt vtables");
        let in_l = construction_vtable(&ast, md, ids[1], &tgt).expect("L-in-D");
        let want_l = Shape {
            components: s(&[
                "vbase 32",
                "top 0",
                "rtti L",
                "L::g",
                "vcall -32",
                "top -32",
                "rtti L",
                "L::g",
            ]),
            starts: vec![0, 4],
            address_points: ap(&[("L", 0, 3), ("V", 32, 7)]),
            thunks: vec![(7, th.to_owned())],
            method_thunks: vec![],
            vbase_offset_offsets: named(&[("V", -24)]),
            method_indices: named(&[("L::g", 0)]),
        };
        assert_eq!(shape(&ast, md, &tgt, &in_l), want_l, "{t}");
        let in_r = construction_vtable(&ast, md, ids[2], &tgt).expect("R-in-D");
        let want_r = Shape {
            components: s(&["vbase 16", "top 0", "rtti R", "vcall 0", "top -16", "rtti R", "V::g"]),
            starts: vec![0, 3],
            address_points: ap(&[("R", 16, 3), ("V", 32, 6)]),
            thunks: vec![],
            method_thunks: vec![],
            vbase_offset_offsets: named(&[("V", -24)]),
            method_indices: vec![],
        };
        assert_eq!(shape(&ast, md, &tgt, &in_r), want_r, "{t}");

        // `@_ZTT1D`: _ZTV1D[D], _ZTC1D0_1L[L], _ZTC1D0_1L[V],
        // _ZTC1D16_1R[R], _ZTC1D16_1R[V], _ZTV1D[V], _ZTV1D[R].
        let table: Vec<(u32, String, u64)> = vtt(&ast, md, &tgt)
            .expect("vtt")
            .into_iter()
            .map(|e| {
                let (c, o) = at(e.base);
                (e.vtable_index, c, o)
            })
            .collect();
        let want: Vec<(u32, String, u64)> = [
            (0, "D", 0),
            (1, "L", 0),
            (1, "V", 32),
            (2, "R", 16),
            (2, "V", 32),
            (0, "V", 32),
            (0, "R", 16),
        ]
        .iter()
        .map(|&(i, c, o)| (i, c.to_owned(), o))
        .collect();
        assert_eq!(table, want, "{t}");
        // The bases themselves: `V` has no VTT; `L`'s has its own vtable
        // and its `V` secondary virtual pointer.
        assert_eq!(vtt(&ast, decls[v], &tgt).expect("vtt"), vec![], "{t}");
        assert_eq!(vtt(&ast, decls[l], &tgt).expect("vtt").len(), 2, "{t}");
    }
}

/// §2.5.3 / [class.virtual]/8: a covariant overrider whose return type
/// needs a non-virtual adjustment (`Q` is at offset 4 in `W`) occupies
/// the overridden slot through a return thunk and gets a new slot of
/// its own, which is its method index (`T0::h` keeps slot 0).
#[test]
fn covariant_return_thunk() {
    let mut m = Model::default();
    let q = class(&mut m, "Q", &[], &[], vec![], vec![int("q")]);
    let z = class(&mut m, "Z", &[], &[], vec![], vec![int("z")]);
    let w = class(&mut m, "W", &[z, q], &[], vec![], vec![]);
    let t0 =
        class(&mut m, "T0", &[], &[], vec![vm_ret("h", Ty::Ptr(Box::new(Ty::Named(q))))], vec![]);
    let t1 =
        class(&mut m, "T1", &[t0], &[], vec![vm_ret("h", Ty::Ptr(Box::new(Ty::Named(w))))], vec![]);
    for t in BOTH {
        cross_check(&m, t);
        let th = "this (0, 0) ret (4, 0) method T0::h this_class T0";
        let want = Shape {
            components: s(&["top 0", "rtti T1", "T1::h", "T1::h"]),
            starts: vec![0],
            address_points: ap(&[("T0", 0, 2), ("T1", 0, 2)]),
            thunks: vec![(2, th.to_owned())],
            method_thunks: pairs(&[("T1::h", th)]),
            vbase_offset_offsets: vec![],
            method_indices: named(&[("T0::h", 0), ("T1::h", 1)]),
        };
        assert_eq!(layout_shape(&m, t, t1), want, "{t}");
    }
}

/// §2.5.2: a virtual destructor takes two slots, complete (D1) then
/// deleting (D0); its method index is the D1 slot. An implicit
/// destructor overriding a virtual one is virtual too.
#[test]
fn destructor_slots() {
    let mut m = Model::default();
    let mut x = RecDef::new("X", TagKind::Struct, vec![]);
    x.methods = vec![vm("f"), vm("g")];
    x.dtor = Some(DtorDef { virtual_written: true, pure: false, pos: 1 });
    x.out_of_line = true;
    x.user_ctor = true;
    let x = m.push(Item::Record(x));
    let y = class(&mut m, "Y", &[x], &[], vec![vm("g")], vec![]);
    for t in BOTH {
        cross_check(&m, t);
        let want_x = Shape {
            components: s(&["top 0", "rtti X", "X::f", "D1 X::~X", "D0 X::~X", "X::g"]),
            starts: vec![0],
            address_points: ap(&[("X", 0, 2)]),
            thunks: vec![],
            method_thunks: vec![],
            vbase_offset_offsets: vec![],
            method_indices: named(&[("X::f", 0), ("X::g", 3), ("X::~X", 1)]),
        };
        assert_eq!(layout_shape(&m, t, x), want_x, "{t}");
        let want_y = Shape {
            components: s(&["top 0", "rtti Y", "X::f", "D1 Y::~Y", "D0 Y::~Y", "Y::g"]),
            starts: vec![0],
            address_points: ap(&[("X", 0, 2), ("Y", 0, 2)]),
            thunks: vec![],
            method_thunks: vec![],
            vbase_offset_offsets: vec![],
            method_indices: named(&[("X::f", 0), ("Y::g", 3), ("Y::~Y", 1)]),
        };
        assert_eq!(layout_shape(&m, t, y), want_y, "{t}");
    }
}
