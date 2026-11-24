//! WP-3404 class member lookup through base lattices
//! ([class.member.lookup], [class.mi]/4; spec/sema.md "Class member lookup
//! cost"): a virtual base is one subobject however many paths reach it, so
//! the search visits it once; non-virtual repeated bases stay distinct
//! subobjects. Expected messages follow `clang++-19 -std=c++26
//! -fsyntax-only` (paths included: clang lists one path per subobject).
#![allow(
    clippy::many_single_char_names,
    reason = "a..f, s, h, r: classes named as in the C++ they spell, sema, heads"
)]

mod conv_common;

use conv_common::{L, lvalue, name, with_sema};
use kiln_ast::decl::{Access, BaseSpec, DeclKind, DefinitionBits, FnSpec, TagKind};
use kiln_ast::types::{ExcSpec, FuncInfo, Quals};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::MemorySink;
use kiln_sema::Sema;
use kiln_sema::lookup::LookupKind;

fn messages(sink: &MemorySink) -> Vec<String> {
    fn walk(d: &kiln_diag::Diagnostic, out: &mut Vec<String>) {
        out.push(format!("{}: {}", d.level.text_name(), d.message));
        for n in &d.notes {
            walk(n, out);
        }
    }
    let mut out = Vec::new();
    for d in sink.take() {
        walk(&d, &mut out);
    }
    out
}

/// A complete `struct n : bases {}`, each base `(class, virtual)`, with a
/// public member function `f()` when `with_f`.
fn class(s: &mut Sema<'_, '_>, n: &str, bases: &[(DeclId, bool)], with_f: bool) -> DeclId {
    let h = DeclHead::new(s.ast.tu, name(s, n), L);
    let r = s.ast.builder().record(h, TagKind::Struct, None);
    s.ast.builder().begin_record_definition(r).expect("first definition");
    let specs: Vec<BaseSpec> = bases
        .iter()
        .map(|&(b, is_virtual)| BaseSpec {
            ty: s.ast.decls.get(b).ty().expect("record type"),
            loc: L,
            is_virtual,
            access: Access::Public,
            pack_expansion: false,
        })
        .collect();
    let ast = &mut s.ast;
    ast.decls.set_bases(r, &specs, &mut ast.lists).expect("bases once");
    if with_f {
        let ft = s.ast.types.function(
            QualType::INT,
            &[],
            FuncInfo::new().with_this_quals(Quals::NONE),
            ExcSpec::None,
        );
        let h = DeclHead::new(r, name(s, "f"), L);
        let f = s.ast.builder().function(h, DeclKind::CXXMethod, ft, FnSpec::new(), None);
        s.ast.decls.set_access(f, Access::Public);
    }
    s.ast.builder().complete_record_definition(r, DefinitionBits::empty());
    r
}

/// `L0a { f(); }; L0b : virtual L0a {};` and, for k = 1..=n,
/// `Lka, Lkb : virtual L(k-1)a, virtual L(k-1)b {}`: 2^n paths from `Lna`
/// to `L0a`, 2n + 1 classes. Returns `Lna`.
fn virtual_lattice(s: &mut Sema<'_, '_>, n: usize, is_virtual: bool) -> DeclId {
    let mut a = class(s, "L0a", &[], true);
    let mut b = class(s, "L0b", &[(a, is_virtual)], false);
    for k in 1..=n {
        let bases = [(a, is_virtual), (b, is_virtual)];
        let na = class(s, &format!("L{k}a"), &bases, false);
        let nb = class(s, &format!("L{k}b"), &bases, false);
        (a, b) = (na, nb);
    }
    a
}

#[test]
fn virtual_lattice_search_visits_each_class_once() {
    with_sema(|s, sink| {
        let top = virtual_lattice(s, 20, true);
        let f = name(s, "f");
        // Lna, then L(k)a and L(k)b for k < n: 2n + 1 classes, each once
        // (it was 2^n paths before WP-3404).
        assert_eq!(s.class_member_search_visits(top, f, LookupKind::Ordinary), 41);
        let sym = s.ast.symbols().intern("f");
        let found = s.lookup_in_class(top, sym, LookupKind::Ordinary);
        assert_eq!(found.decls.len(), 1);
        assert!(found.ambiguity.is_none());
        assert_eq!(messages(sink), Vec::<String>::new());
    });
}

#[test]
fn non_virtual_lattice_subobjects_stay_distinct() {
    with_sema(|s, sink| {
        // The same shape with non-virtual bases: each path from L3a to
        // L0a (2^3 of them) designates a distinct L0a subobject, so `f` is
        // ambiguous and every path is listed.
        let top = virtual_lattice(s, 3, false);
        let f = name(s, "f");
        let sym = s.ast.symbols().intern("f");
        let found = s.lookup_in_class(top, sym, LookupKind::Ordinary);
        let Some(kiln_sema::lookup::MemberAmbiguity::Subobjects { paths, .. }) = &found.ambiguity
        else {
            panic!("expected a subobject ambiguity, got {:?}", found.ambiguity);
        };
        assert_eq!(paths.len(), 8);
        // A non-virtual base is searched once per path: L3a, the 2 + 4
        // classes of levels 2 and 1, and below each level-1 class L0a, L0b
        // and L0b's L0a.
        assert_eq!(s.class_member_search_visits(top, f, LookupKind::Ordinary), 1 + 2 + 4 + 4 * 3);
        assert_eq!(messages(sink), Vec::<String>::new());
    });
}

#[test]
fn mixed_lattice_lists_one_path_per_subobject() {
    with_sema(|s, sink| {
        // struct A { int f(); }; struct B : A {};
        // struct C : virtual B {}; struct D2 : virtual B {};
        // struct E : A {}; struct F : C, D2, E {};
        let a = class(s, "A", &[], true);
        let b = class(s, "B", &[(a, false)], false);
        let c = class(s, "C", &[(b, true)], false);
        let d2 = class(s, "D2", &[(b, true)], false);
        let e = class(s, "E", &[(a, false)], false);
        let f = class(s, "F", &[(c, false), (d2, false), (e, false)], false);
        let fty = s.ast.decls.get(f).ty().expect("record type");
        let obj = lvalue(s, "x", fty);
        let sym = s.ast.symbols().intern("f");
        let m = s.act_on_member(obj, sym, L, L, false);
        assert!(s.is_error_expr(m));
        // `F -> D2 -> B -> A` is the subobject `F -> C -> B -> A` names.
        assert_eq!(
            messages(sink),
            [
                "error: non-static member 'f' found in multiple base-class subobjects of type \
                 'A':\n    struct F -> C -> B -> A\n    struct F -> E -> A",
                "note: member found by ambiguous name lookup",
            ]
        );
    });
}

#[test]
fn non_virtual_diamond_lists_both_paths() {
    with_sema(|s, sink| {
        let a = class(s, "A", &[], true);
        let b = class(s, "B", &[(a, false)], false);
        let c = class(s, "C", &[(a, false)], false);
        let d = class(s, "D", &[(b, false), (c, false)], false);
        let dty = s.ast.decls.get(d).ty().expect("record type");
        let obj = lvalue(s, "x", dty);
        let sym = s.ast.symbols().intern("f");
        let m = s.act_on_member(obj, sym, L, L, false);
        assert!(s.is_error_expr(m));
        assert_eq!(
            messages(sink),
            [
                "error: non-static member 'f' found in multiple base-class subobjects of type \
                 'A':\n    struct D -> B -> A\n    struct D -> C -> A",
                "note: member found by ambiguous name lookup",
            ]
        );
    });
}
