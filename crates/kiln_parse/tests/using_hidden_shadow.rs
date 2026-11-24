//! The hidden mark of a using-shadow as the `Ast` keeps it after the
//! translation unit, for tooling clients (`DeclArena::hidden_shadow`,
//! spec/ast.md §4.2; [namespace.udecl]/4, /14). A class's record is the
//! same whatever the order of the member and the using-declarator
//! (WP-3145), and whatever the rest of the unit uses: an implicit member
//! that hides a shadow is declared, and the mark set, when the class is
//! completed (WP-3146). Each test parses a TU, drops sema, and reads the
//! `Ast` alone.

use std::sync::Arc;

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::{Ast, DeclId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ParseOptions, parse_file};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The `Ast` of `src` (which must compile cleanly) and its interner.
fn parse(src: &str) -> (Ast, Arc<Interner>) {
    let interner = Arc::new(Interner::new());
    (parse_in(src, &interner), interner)
}

fn parse_in(src: &str, interner: &Arc<Interner>) -> Ast {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let ast = parse_file(&sm, interner, file, &engine, target, &opts);
    let diags: Vec<String> = sink.take().iter().map(|d| d.message.clone()).collect();
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    ast
}

/// The simple name of `d`.
fn name(ast: &Ast, i: &Interner, d: DeclId) -> String {
    ast.names.simple_spelling(ast.decls.get(d).name(), i).unwrap_or_default()
}

/// The class definition named `n` (a specialization when `spec`).
fn class(ast: &Ast, i: &Interner, n: &str, spec: bool) -> DeclId {
    let v: Vec<DeclId> = ast
        .decls
        .ids()
        .filter(|&d| {
            ast.decls.get(d).kind() == DeclKind::CXXRecord
                && name(ast, i, d) == n
                && ast.definition(d) == Some(d)
                && ast.spec_info(d).is_some() == spec
        })
        .collect();
    assert_eq!(v.len(), 1, "class {n}: {v:?}");
    v[0]
}

/// The only `Using` member of class `c`.
fn using_of(ast: &Ast, c: DeclId) -> DeclId {
    let v: Vec<DeclId> =
        ast.members(c).filter(|&m| ast.decls.get(m).kind() == DeclKind::Using).collect();
    assert_eq!(v.len(), 1, "one using-declaration in {c:?}: {v:?}");
    v[0]
}

/// The members of `c` of kind `kind` (functions or constructors), in
/// member order: what a shadow's target or its hider is identified by.
fn fns(ast: &Ast, c: DeclId, kind: DeclKind) -> Vec<DeclId> {
    ast.members(c).filter(|&m| ast.decls.get(m).kind() == kind).collect()
}

/// One shadow of a record: the position of its target among the base's
/// members of that kind, and, when it is hidden, whether the member of the
/// class that hides it is implicit and how many parameters it has.
type Shadow = (usize, Option<(bool, usize)>);

/// The record of class `c`'s using-declaration: its shadows in order, each
/// with its target (among `base`'s members of kind `kind`) and its hidden
/// mark. Every mark names a member of `c`.
fn record(ast: &Ast, c: DeclId, base: DeclId, kind: DeclKind) -> Vec<Shadow> {
    let targets = fns(ast, base, kind);
    ast.using_shadows(using_of(ast, c))
        .map(|s| {
            assert_eq!(ast.decls.get(s).ctx(), Some(c), "shadow {s:?} in its class");
            let t = ast.using_target(s).expect("a shadow names its target");
            let at = targets.iter().position(|&x| x == t).expect("target is a base member");
            let by = ast.decls.hidden_shadow(s).map(|h| {
                assert_eq!(ast.decls.get(h).ctx(), Some(c), "hider {h:?} is a member of the class");
                assert_eq!(ast.decls.get(h).kind(), kind);
                let implicit = ast.decls.get(h).flags().contains(DeclFlags::IMPLICIT);
                let f = ast.decls.function(h).expect("a function");
                (implicit, ast.lists.iter::<DeclId>(f.params).len())
            });
            (at, by)
        })
        .collect()
}

/// [namespace.udecl]/14 (WP-3145): `void f()` hides `B::f()` before or
/// after `using B::f;`; both orders keep the shadow, marked hidden, and
/// `B::f(int)`'s, not hidden. The same in a class template specialization
/// (the members are redone in declaration order; its shadows are listed
/// newest first, as clang's, so the records are compared sorted).
#[test]
fn member_hides_in_either_order() {
    let (ast, i) = parse(
        "struct B { void f(); void f(int); };\n\
         struct D : B { using B::f; void f(); };\n\
         struct E : B { void f(); using B::f; };\n\
         template <class T> struct F : B { void f(); using B::f; };\n\
         F<int> x;\n",
    );
    let b = class(&ast, &i, "B", false);
    let want: Vec<Shadow> = vec![(0, Some((false, 0))), (1, None)];
    for (n, spec) in [("D", false), ("E", false), ("F", true)] {
        let c = class(&ast, &i, n, spec);
        let mut got = record(&ast, c, b, DeclKind::CXXMethod);
        got.sort();
        assert_eq!(got, want, "class {n}");
    }
}

/// [namespace.udecl]/14 with [class.inhctor.init] (WP-3145): `G(int)`
/// hides the inherited `C(int)` before or after `using C::C;`. Both
/// orders keep one shadow per constructor of `C` (its implicit copy and
/// move constructors included), `C(int)`'s marked hidden.
#[test]
fn constructor_hides_in_either_order() {
    let (ast, i) = parse(
        "struct C { C(int); C(double); };\n\
         struct G : C { using C::C; G(int); };\n\
         struct H : C { H(int); using C::C; };\n",
    );
    let base = class(&ast, &i, "C", false);
    let n = fns(&ast, base, DeclKind::CXXConstructor).len();
    assert_eq!(n, 4, "two written, the implicit copy and move");
    let mut want: Vec<Shadow> = (0..n).map(|k| (k, None)).collect();
    want[0].1 = Some((false, 1));
    for c in ["G", "H"] {
        let c = class(&ast, &i, c, false);
        assert_eq!(record(&ast, c, base, DeclKind::CXXConstructor), want);
    }
}

/// The base members `D`'s using-declarator brings in, in `B`'s member
/// order: `operator=(const D&)`, `operator=(D&&)`, `operator=(int)`, then
/// `B`'s implicit copy and move assignment operators.
const ASSIGN: &str = "struct D;\n\
     struct B { B &operator=(const D &); B &operator=(D &&); B &operator=(int); };\n\
     struct D : B { using B::operator=; };\n";

/// [namespace.udecl]/4, /14, [class.copy.assign]/2, /4 (WP-3146): the
/// implicitly-declared copy and move assignment operators of `D` hide
/// `B::operator=(const D&)` and `B::operator=(D&&)`. They are declared at
/// `D`'s `}`, so the record (the members and the marks) is the same with
/// and without a use of either assignment in the unit.
#[test]
fn implicit_assignment_marks_at_completion() {
    let want: Vec<Shadow> =
        vec![(0, Some((true, 1))), (1, Some((true, 1))), (2, None), (3, None), (4, None)];
    let mut members = Vec::new();
    for src in [
        ASSIGN.to_owned(),
        format!("{ASSIGN}void u(D &a, D &b) {{ a = b; }}\n"),
        format!("{ASSIGN}void u(D &a, D &b) {{ a = static_cast<D &&>(b); }}\n"),
    ] {
        let (ast, i) = parse(&src);
        let b = class(&ast, &i, "B", false);
        let d = class(&ast, &i, "D", false);
        assert_eq!(fns(&ast, b, DeclKind::CXXMethod).len(), 5, "{src}");
        assert_eq!(record(&ast, d, b, DeclKind::CXXMethod), want, "{src}");
        let shape: Vec<(DeclKind, bool)> = ast
            .members(d)
            .map(|m| {
                let n = ast.decls.get(m);
                (n.kind(), n.flags().contains(DeclFlags::IMPLICIT))
            })
            .collect();
        members.push(shape);
    }
    assert_eq!(members[0], members[1], "a use of the copy assignment adds nothing to D");
    assert_eq!(members[0], members[2], "nor a use of the move assignment");
}

/// A user-declared copy assignment hides `B::operator=(const D&)` and
/// suppresses the implicit move assignment ([class.copy.assign]/4), so
/// `B::operator=(D&&)` stays visible: the mark follows the members that
/// are declared, not the signatures alone.
#[test]
fn user_declared_assignment_suppresses_implicit_move() {
    let (ast, i) = parse(
        "struct D;\n\
         struct B { B &operator=(const D &); B &operator=(D &&); };\n\
         struct D : B { using B::operator=; D &operator=(const D &); };\n",
    );
    let b = class(&ast, &i, "B", false);
    let d = class(&ast, &i, "D", false);
    let want: Vec<Shadow> = vec![(0, Some((false, 1))), (1, None), (2, None), (3, None)];
    assert_eq!(record(&ast, d, b, DeclKind::CXXMethod), want);
}
