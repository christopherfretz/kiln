//! The hidden mark of a using-shadow as the `Ast` keeps it after the
//! translation unit, for tooling clients (`DeclArena::hidden_shadow`,
//! spec/ast.md §4.2; [namespace.udecl]/4, /14). A class's record is the
//! same whatever the order of the member and the using-declarator
//! (WP-3145), and whatever the rest of the unit uses: an implicit member
//! that hides a shadow is declared, and the mark set, when the class is
//! completed (WP-3146). Each test parses a TU, drops sema, and reads the
//! `Ast` alone.

use std::fmt::Write as _;
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
    parse_with(src, interner, &ParseOptions::default())
}

fn parse_with(src: &str, interner: &Arc<Interner>, opts: &ParseOptions) -> Ast {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::X86_64);
    let ast = parse_file(&sm, interner, file, &engine, target, opts);
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

// -- WP-3403: the record a tooling client reads (spec/ast.md §4.2) -------

/// Every `Using` member of class `c`, in member order.
fn usings(ast: &Ast, c: DeclId) -> Vec<DeclId> {
    ast.members(c).filter(|&m| ast.decls.get(m).kind() == DeclKind::Using).collect()
}

/// What a client reads from using-declaration `u`: per shadow (sorted),
/// its target and its hider, checked against every accessor. A hider is
/// a first declaration of a member of the shadow's class (R3), and the
/// shadow is among the ones it hides ([`Ast::shadows_hidden_by`]).
fn named(ast: &Ast, u: DeclId) -> Vec<(DeclId, Option<DeclId>)> {
    let shadows: Vec<DeclId> = ast.using_shadows(u).collect();
    let rows: Vec<kiln_ast::UsingNamed> = ast.using_named(u).collect();
    assert_eq!(rows.len(), shadows.len());
    let mut out = Vec::new();
    for (r, &s) in rows.iter().zip(&shadows) {
        assert_eq!(r.shadow, s);
        assert_eq!(Some(r.target), ast.using_target(s));
        assert_eq!(r.hider, ast.shadow_hider(s));
        assert_eq!(r.hider, ast.decls.hidden_shadow(s));
        assert_eq!(ast.shadow_using(s), Some(u));
        if let Some(h) = r.hider {
            assert_eq!(ast.first(h), h, "the hider is the member's first declaration");
            assert_eq!(ast.decls.get(h).ctx(), ast.decls.get(s).ctx(), "a member of the class");
            assert!(ast.shadows_hidden_by(h).any(|x| x == s), "shadows_hidden_by({h:?})");
        }
        out.push((r.target, r.hider));
    }
    out.sort();
    out
}

/// The members of `c` named `n` (declarations of any kind), in order.
fn named_members(ast: &Ast, i: &Interner, c: DeclId, n: &str) -> Vec<DeclId> {
    ast.members(c)
        .filter(|&m| {
            !matches!(
                ast.decls.get(m).kind(),
                DeclKind::Using | DeclKind::UsingShadow | DeclKind::ConstructorUsingShadow
            ) && name(ast, i, m) == n
        })
        .collect()
}

/// A class template specialization of template `n` (its only one).
fn spec_of(ast: &Ast, i: &Interner, n: &str) -> DeclId {
    class(ast, i, n, true)
}

/// G1 ([namespace.udecl]/14, [temp.inst]/3): in a class template
/// specialization the member declared AFTER the using-declarator hides
/// the shadow as in the pattern and as in a non-template class: the
/// members are redone in declaration order, so the hiding is checked
/// when the member is redone.
#[test]
fn member_after_using_hides_in_a_specialization() {
    let (ast, i) = parse(
        "struct B { void f(); void f(int); };\n\
         template <class T> struct F : B { using B::f; void f(); };\n\
         template <class T> struct G : B { void f(); using B::f; };\n\
         F<int> x;\n\
         G<int> y;\n",
    );
    let b = class(&ast, &i, "B", false);
    let bf = named_members(&ast, &i, b, "f");
    for (n, spec) in [("F", false), ("F", true), ("G", false), ("G", true)] {
        let c = class(&ast, &i, n, spec);
        let own = named_members(&ast, &i, c, "f");
        assert_eq!(own.len(), 1, "{n}");
        let mut want = vec![(bf[0], Some(own[0])), (bf[1], None)];
        want.sort();
        assert_eq!(named(&ast, usings(&ast, c)[0]), want, "class {n} (spec: {spec})");
    }
}

/// R2: when the correspondence depends on a template parameter (`void
/// f(T)` against `B::f(int)`), the pattern is not marked (a dependent
/// parameter-type-list corresponds to nothing) and each specialization is
/// marked per its arguments, in either order.
#[test]
fn dependent_hider_marks_per_specialization() {
    let (ast, i) = parse(
        "struct B { void f(int); void f(double); };\n\
         template <class T> struct G : B { using B::f; void f(T); };\n\
         template <class T> struct Gc : B { using B::f; void f(T); };\n\
         template <class T> struct H : B { void f(T); using B::f; };\n\
         template <class T> struct Hc : B { void f(T); using B::f; };\n\
         G<int> g1; Gc<char> g2; H<int> h1; Hc<char> h2;\n",
    );
    let b = class(&ast, &i, "B", false);
    let bf = named_members(&ast, &i, b, "f");
    for n in ["G", "Gc", "H", "Hc"] {
        let p = class(&ast, &i, n, false);
        let mut want = vec![(bf[0], None), (bf[1], None)];
        want.sort();
        assert_eq!(named(&ast, usings(&ast, p)[0]), want, "pattern {n} is unmarked");
    }
    for (n, hides) in [("G", true), ("Gc", false), ("H", true), ("Hc", false)] {
        let c = spec_of(&ast, &i, n);
        let own = named_members(&ast, &i, c, "f")[0];
        let mut want = vec![(bf[0], hides.then_some(own)), (bf[1], None)];
        want.sort();
        assert_eq!(named(&ast, usings(&ast, c)[0]), want, "specialization of {n}");
    }
}

/// R3: the hider is exactly the member that corresponds to the shadow's
/// target among the overloads of that name, and it is the in-class (first)
/// declaration: an out-of-line definition answers [`Ast::shadows_hidden_by`]
/// the same and is never the mark.
#[test]
fn hider_is_the_corresponding_in_class_declaration() {
    let (ast, i) = parse(
        "struct B { void f(int); void f(char); };\n\
         struct D : B { void f(); void f(int); void f(double); using B::f; };\n\
         void D::f(int) {}\n",
    );
    let b = class(&ast, &i, "B", false);
    let d = class(&ast, &i, "D", false);
    let bf = named_members(&ast, &i, b, "f");
    let df = named_members(&ast, &i, d, "f");
    let mut want = vec![(bf[0], Some(df[1])), (bf[1], None)];
    want.sort();
    assert_eq!(named(&ast, usings(&ast, d)[0]), want);
    let def = (1..=ast.decls.len() as u32)
        .filter_map(DeclId::try_new)
        .find(|&x| x != df[1] && ast.first(x) == df[1])
        .expect("the out-of-line definition");
    let a: Vec<DeclId> = ast.shadows_hidden_by(def).collect();
    let b2: Vec<DeclId> = ast.shadows_hidden_by(df[1]).collect();
    assert_eq!(a, b2);
    assert_eq!(a.len(), 1);
    assert!(ast.shadows_hidden_by(df[0]).next().is_none(), "f() hides nothing");
}

/// [namespace.udecl]/14 with [class.conv.fct]: a conversion function hides
/// the base's for the same converted-to type, in either order; another
/// type's is not hidden.
#[test]
fn conversion_function_hides_in_either_order() {
    let (ast, i) = parse(
        "struct B { operator int() const; operator long() const; };\n\
         struct D : B { using B::operator int; using B::operator long; operator int() const; };\n\
         struct E : B { operator int() const; using B::operator int; using B::operator long; };\n",
    );
    let b = class(&ast, &i, "B", false);
    let conv = fns(&ast, b, DeclKind::CXXConversion);
    for n in ["D", "E"] {
        let c = class(&ast, &i, n, false);
        let own = fns(&ast, c, DeclKind::CXXConversion);
        assert_eq!(own.len(), 1, "{n}");
        let u = usings(&ast, c);
        assert_eq!(named(&ast, u[0]), vec![(conv[0], Some(own[0]))], "{n}: operator int");
        assert_eq!(named(&ast, u[1]), vec![(conv[1], None)], "{n}: operator long");
    }
}

/// [namespace.udecl]/14, [temp.over.link]/6: a member function template
/// hides the base member template with an equivalent template head and
/// the same parameter-type-list, in either order; the hider is the
/// `FunctionTemplate`, not its pattern. A non-template never hides a
/// template.
#[test]
fn member_template_hides_in_either_order() {
    let (ast, i) = parse(
        "struct B { template <class T> void g(T); template <class T> void g(T, int); void g(); };\n\
         struct D : B { using B::g; template <class U> void g(U); };\n\
         struct E : B { template <class U> void g(U); using B::g; };\n\
         struct N : B { void g(int); using B::g; };\n",
    );
    let b = class(&ast, &i, "B", false);
    let bt = fns(&ast, b, DeclKind::FunctionTemplate);
    let bg = named_members(&ast, &i, b, "g");
    assert_eq!(bg.len(), 3);
    for n in ["D", "E"] {
        let c = class(&ast, &i, n, false);
        let own = fns(&ast, c, DeclKind::FunctionTemplate);
        let mut want = vec![(bt[0], Some(own[0])), (bt[1], None), (bg[2], None)];
        want.sort();
        assert_eq!(named(&ast, usings(&ast, c)[0]), want, "{n}");
    }
    let c = class(&ast, &i, "N", false);
    assert!(named(&ast, usings(&ast, c)[0]).iter().all(|r| r.1.is_none()), "N hides nothing");
}

/// Q5 / [namespace.udecl]/14 ("override and/or hide"), [class.virtual]/2:
/// a virtual override hides the shadow of the function it overrides, one
/// mark in either order whose hider is the overrider; the override itself
/// is read from [`Ast::overridden_methods`].
#[test]
fn virtual_override_is_one_mark_in_either_order() {
    let (ast, i) = parse(
        "struct V { virtual void f(); void f(int); };\n\
         struct W : V { using V::f; void f() override; };\n\
         struct X : V { void f() override; using V::f; };\n\
         void X::f() {}\n",
    );
    let v = class(&ast, &i, "V", false);
    let vf = named_members(&ast, &i, v, "f");
    for n in ["W", "X"] {
        let c = class(&ast, &i, n, false);
        let own = named_members(&ast, &i, c, "f")[0];
        let mut want = vec![(vf[0], Some(own)), (vf[1], None)];
        want.sort();
        assert_eq!(named(&ast, usings(&ast, c)[0]), want, "{n}");
        assert_eq!(ast.overridden_methods(own).collect::<Vec<_>>(), vec![vf[0]], "{n}");
    }
    // Any redeclaration answers; a non-method has none.
    let x = class(&ast, &i, "X", false);
    let xf = named_members(&ast, &i, x, "f")[0];
    let def = (1..=ast.decls.len() as u32)
        .filter_map(DeclId::try_new)
        .find(|&d| d != xf && ast.first(d) == xf)
        .expect("the out-of-line definition");
    assert_eq!(ast.overridden_methods(def).collect::<Vec<_>>(), vec![vf[0]]);
    assert_eq!(ast.overridden_methods(vf[1]).len(), 0);
    assert_eq!(ast.overridden_methods(v).len(), 0);
}

/// [namespace.udecl]/14 with [class.inhctor.init] in a class template
/// specialization: the inherited `C(int)` is hidden by the
/// specialization's `G(int)` declared before or after `using C::C;`.
#[test]
fn constructor_hides_in_a_specialization_in_either_order() {
    let (ast, i) = parse(
        "struct C { C(int); C(double); };\n\
         template <class T> struct G : C { using C::C; G(int); };\n\
         template <class T> struct H : C { H(int); using C::C; };\n\
         G<int> g(1); H<int> h(1);\n",
    );
    let base = class(&ast, &i, "C", false);
    let ctors = fns(&ast, base, DeclKind::CXXConstructor);
    for n in ["G", "H"] {
        let c = spec_of(&ast, &i, n);
        let own: Vec<DeclId> = fns(&ast, c, DeclKind::CXXConstructor)
            .into_iter()
            .filter(|&k| !ast.decls.get(k).flags().contains(DeclFlags::IMPLICIT))
            .collect();
        assert_eq!(own.len(), 1, "{n}");
        let got = named(&ast, usings(&ast, c)[0]);
        assert!(got.contains(&(ctors[0], Some(own[0]))), "{n}: {got:?}");
        assert!(got.contains(&(ctors[1], None)), "{n}: {got:?}");
    }
}

/// R1, [class.compare.default]/5: a defaulted `operator<=>` declares an
/// `operator==` only when the class declares none, and a class-scope
/// `using B::operator==;` counts (as in clang 19, clang 22 and GCC 14,
/// where `a == b` below calls `B::operator==(const E &)`): no implicit
/// member hides the shadows, used or not.
#[test]
fn no_implicit_equality_beside_a_using_declarator() {
    let src = "struct E;\n\
        namespace std { struct strong_ordering { int v; static const strong_ordering equal; }; }\n\
        struct B { bool operator==(const B &) const; bool operator==(const E &) const; \
                   std::strong_ordering operator<=>(const B &) const; };\n\
        struct E : B { using B::operator==; \
          std::strong_ordering operator<=>(const E &) const = default; };\n";
    let mut records = Vec::new();
    for s in [src.to_owned(), format!("{src}bool u(const E &a, const E &b) {{ return a == b; }}\n")]
    {
        let (ast, i) = parse(&s);
        let e = class(&ast, &i, "E", false);
        assert!(
            named_members(&ast, &i, e, "operator==").is_empty(),
            "no implicit operator== in E: {s}"
        );
        let got = named(&ast, usings(&ast, e)[0]);
        assert!(got.iter().all(|r| r.1.is_none()), "{got:?}");
        records.push(got.len());
    }
    assert_eq!(records[0], records[1], "a use adds nothing");
}

/// R1 ([namespace.udecl]/4, /14, [class.copy.assign]/2, /4) in a class
/// template specialization: `using B::operator=;` makes the
/// specialization declare its implicit copy and move assignment operators
/// at its `}`, which hide the corresponding shadows, used or not; each
/// hider carries `DeclFlags::IMPLICIT`.
#[test]
fn implicit_assignment_marks_in_a_specialization() {
    let src = "template <class T> struct D;\n\
        struct B { B &operator=(const D<int> &); B &operator=(D<int> &&); B &operator=(int); };\n\
        template <class T> struct D : B { using B::operator=; };\n\
        D<int> *p;\n\
        int n = sizeof(D<int>);\n";
    let mut records = Vec::new();
    for s in [
        src.to_owned(),
        format!("{src}void u(D<int> &a, D<int> &b) {{ a = b; a = static_cast<D<int> &&>(b); }}\n"),
    ] {
        let (ast, i) = parse(&s);
        let b = class(&ast, &i, "B", false);
        let d = spec_of(&ast, &i, "D");
        let ops = fns(&ast, b, DeclKind::CXXMethod);
        let got = named(&ast, usings(&ast, d)[0]);
        for (k, want_hidden) in [(0, true), (1, true), (2, false)] {
            let row = got.iter().find(|r| r.0 == ops[k]).expect("a shadow per base operator=");
            assert_eq!(row.1.is_some(), want_hidden, "B's operator= #{k}: {got:?} ({s})");
            if let Some(h) = row.1 {
                assert!(ast.decls.get(h).flags().contains(DeclFlags::IMPLICIT), "implicit hider");
            }
        }
        records.push(got);
    }
    assert_eq!(records[0].len(), records[1].len(), "a use adds no shadow");
}

/// Q3 ([namespace.udecl]/1, /14): two class-scope using-declarators that
/// name one entity through different bases each make a shadow; the later
/// one replaces the earlier in lookup. A member declared after both, or
/// before both, hides the shadows of the entity it corresponds to, in
/// both using-declarations' lists.
#[test]
fn repeated_using_through_another_base_makes_its_own_shadow() {
    let (ast, i) = parse(
        "struct A { void f(); void f(int); };\n\
         struct B1 : virtual A {}; struct B2 : virtual A {};\n\
         struct D : B1, B2 { using B1::f; using B2::f; };\n\
         struct E : B1, B2 { using B1::f; using B2::f; void f(); };\n\
         struct F : B1, B2 { void f(); using B1::f; using B2::f; };\n\
         struct G : B1, B2 { using B1::f; void f(); using B2::f; };\n",
    );
    let a = class(&ast, &i, "A", false);
    let af = named_members(&ast, &i, a, "f");
    for n in ["D", "E", "F", "G"] {
        let c = class(&ast, &i, n, false);
        let us = usings(&ast, c);
        assert_eq!(us.len(), 2, "{n}");
        let own = named_members(&ast, &i, c, "f").first().copied();
        let mut want = vec![(af[0], own), (af[1], None)];
        want.sort();
        for &u in &us {
            assert_eq!(named(&ast, u), want, "{n}: each using-declaration names both");
        }
        // Lookup holds the later shadows (and the member).
        let fname = ast.decls.get(af[0]).name();
        let table: Vec<DeclId> = ast.lookup_local(c, fname).iter().collect();
        let later: Vec<DeclId> = ast.using_shadows(us[1]).collect();
        for s in later {
            assert!(table.contains(&s), "{n}: the later shadow is in lookup");
        }
        for s in ast.using_shadows(us[0]) {
            assert!(!table.contains(&s), "{n}: the earlier shadow was replaced");
        }
    }
}

/// [namespace.udecl]/1, [enum.udecl]: `using E::A;` in a class naming an
/// enumerator of a member enumeration makes no shadow (the enumerator is
/// already a member): `using_named` is empty there.
#[test]
fn member_enumerator_using_names_nothing() {
    let (ast, i) = parse("struct S { enum E { A }; using E::A; };\n");
    let s = class(&ast, &i, "S", false);
    let u = usings(&ast, s);
    assert_eq!(u.len(), 1);
    assert_eq!(ast.using_named(u[0]).len(), 0);
    assert_eq!(ast.using_shadows(u[0]).len(), 0);
}

/// R4: one rule in both orders. `Ast::lookup_local` is the raw table and
/// holds the hidden shadow; sema's lookup results never do: no reference
/// of the written-reference table and no found list built from a lookup
/// names a hidden shadow.
#[test]
fn lookups_never_find_a_hidden_shadow() {
    let interner = Arc::new(Interner::new());
    let opts = ParseOptions { written_refs: true, ..ParseOptions::default() };
    let ast = parse_with(
        "struct B { int f() { return 1; } int f(int) { return 2; } };\n\
         struct D : B { using B::f; int f() { return 3; } };\n\
         struct E : B { int f() { return 3; } using B::f; };\n\
         template <class T> struct F : B {\n\
           using B::f; int f() { return 3; } int g(T t) { return f(t) + f(); } };\n\
         template <class T> struct G : B {\n\
           int f() { return 3; } using B::f; int g(T t) { return f(t) + f(); } };\n\
         template <class T> int v(T t, D &d, E &e) { return d.f(t) + e.f(t); }\n\
         int u(D &d, E &e, F<int> &x, G<int> &y) {\n\
           return d.f() + e.f() + x.f() + d.f(0) + x.g(1) + y.g(1) + v(1, d, e); }\n",
        &interner,
        &opts,
    );
    let i = &*interner;
    for (n, spec) in
        [("D", false), ("E", false), ("F", false), ("F", true), ("G", false), ("G", true)]
    {
        let c = class(&ast, i, n, spec);
        let u = usings(&ast, c)[0];
        let hidden: Vec<DeclId> =
            ast.using_shadows(u).filter(|&s| ast.shadow_hider(s).is_some()).collect();
        assert_eq!(hidden.len(), 1, "{n}");
        let fname = ast.decls.get(hidden[0]).name();
        let table: Vec<DeclId> = ast.lookup_local(c, fname).iter().collect();
        assert!(table.contains(&hidden[0]), "{n}: lookup_local holds the hidden shadow");
    }
    let mut seen = 0;
    for &(_, set) in ast.written_refs.found_rows() {
        for fd in ast.written_refs.found_set(set).decls {
            seen += 1;
            assert!(ast.shadow_hider(fd.found).is_none(), "a found set names hidden {fd:?}");
        }
    }
    assert!(seen > 0, "the written-reference table has found sets");
}

/// The marks of a specialization (G1, R2) and the chained shadows (Q3)
/// survive the PCH snapshot.
#[test]
fn specialization_marks_survive_the_snapshot() {
    let (ast, i) = parse(
        "struct B { void f(int); void f(double); };\n\
         template <class T> struct G : B { using B::f; void f(T); };\n\
         struct A { void f(); };\n\
         struct B1 : virtual A {}; struct B2 : virtual A {};\n\
         struct E : B1, B2 { using B1::f; using B2::f; void f(); };\n\
         G<int> g;\n",
    );
    let read = |ast: &Ast| -> Vec<Vec<(DeclId, Option<DeclId>)>> {
        let g = spec_of(ast, &i, "G");
        let e = class(ast, &i, "E", false);
        let mut v = vec![named(ast, usings(ast, g)[0])];
        for u in usings(ast, e) {
            v.push(named(ast, u));
        }
        v
    };
    let want = read(&ast);
    assert!(want[0].iter().any(|r| r.1.is_some()), "G<int>'s shadow of B::f(int) is hidden");
    assert!(want[1..].iter().all(|v| v.len() == 1 && v[0].1.is_some()), "{want:?}");
    let bytes = kiln_snap::to_bytes(&ast);
    let restored = kiln_snap::from_bytes::<Ast>(&bytes).expect("the snapshot decodes");
    assert_eq!(read(&restored), want);
}

/// The accessors are total: a declaration that is not a using-declaration,
/// a shadow or a member function answers empty or `None`.
#[test]
fn the_accessors_are_total() {
    let (ast, _) = parse(
        "struct B { void f(); };\n\
         struct D : B { using B::f; void f(); int x; };\n\
         int g;\n",
    );
    for d in (1..=ast.decls.len() as u32).filter_map(DeclId::try_new) {
        let k = ast.decls.get(d).kind();
        if !matches!(k, DeclKind::Using | DeclKind::UsingEnum) {
            assert_eq!(ast.using_named(d).len(), 0, "{k:?}");
        }
        if !matches!(k, DeclKind::UsingShadow | DeclKind::ConstructorUsingShadow) {
            assert_eq!(ast.shadow_hider(d), None, "{k:?}");
        }
        if !(k.is_function() || k == DeclKind::FunctionTemplate) {
            assert_eq!(ast.shadows_hidden_by(d).count(), 0, "{k:?}");
        }
        let _ = ast.overridden_methods(d).len();
    }
}

/// Q6 (spec/ast.md §8.8, [temp.inst]/3): `m(t)` in the class template `D`
/// finds `D::m(T)` and, through `D`'s using-declaration, `B::m(int)` and
/// `B::m(double)`. In `D<int>` the specialization's `m(int)` hides the
/// shadow of `B::m(int)`: the substituted set leaves `B::m(int)` out and
/// holds the hider (before WP-3403 the call was ambiguous).
#[test]
fn substituted_found_list_holds_the_hider() {
    use kiln_ast::expr::ExprKind;
    let (ast, i) = parse(
        "struct B { static int m(int); static int m(double); };\n\
         template <class T> struct D : B {\n\
           using B::m;\n\
           static int m(T);\n\
           int call(T t) { return m(t); }\n\
         };\n\
         int use() { return D<int>().call(1); }\n",
    );
    let mut rows = Vec::new();
    for e in (1..=ast.exprs.len() as u32).map(kiln_ast::ExprId::new) {
        if ast.exprs.get(e).kind() != ExprKind::UnresolvedLookupExpr {
            continue;
        }
        if ast.unresolved_name(e).as_ident().is_none_or(|s| ast.symbols().as_str(s) != "m") {
            continue;
        }
        let mut row = Vec::new();
        for (d, s) in ast.unresolved_found_pairs(e) {
            let owner = |x: DeclId| {
                let c = ast.decls.get(x).ctx().expect("a member");
                format!(
                    "{}{}",
                    name(&ast, &i, c),
                    if ast.spec_info(c).is_some() { "<int>" } else { "" }
                )
            };
            let mut cell = owner(d);
            if let Some(s) = s {
                let _ = write!(cell, " via {}", owner(s));
                if let Some(h) = ast.shadow_hider(s) {
                    let _ = write!(cell, " hidden by {}", owner(h));
                }
            }
            row.push(cell);
        }
        row.sort();
        rows.push(row.join(", "));
    }
    rows.sort();
    rows.dedup();
    // The pattern's `m(t)` lists `D::m(T)` and both base members through
    // `D`'s shadows; `D<int>`'s lists its `m(int)` and `B::m(double)`
    // through `D<int>`'s shadow, not the hidden `B::m(int)`.
    let want = vec!["B via D, B via D, D".to_owned(), "B via D<int>, D<int>".to_owned()];
    assert_eq!(rows, want, "found lists of m");
    assert!(rows.iter().all(|r| !r.contains("hidden by")), "no found list names a hidden shadow");
}
