//! An instantiated unnamed member class links to its pattern
//! (spec/ast.md §8.2, WP-3273), as an unnamed member enumeration does:
//! `instantiated_from` of the class instantiated for
//! `struct { T x; } m;` or an anonymous union ([class.union.anon]) of a
//! class template is the pattern's unnamed class, also for nested
//! unnamed classes, unnamed classes of member class templates and of
//! partial specializations ([temp.inst]/3). The link is a member's
//! (`member_spec_info`), never a specialization's (`spec_info`).
//! Reduced from real code (a tooling client follows instantiations back
//! to their patterns). Every source here compiles with clang++-19 and
//! clang++-22.

use std::sync::Arc;

use kiln_ast::Ast;
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::DeclId;
use kiln_ast::templates::SpecKind;
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Parses `src` (no errors allowed) and runs `check` on the unit.
fn parse<R>(src: &str, check: impl FnOnce(&Ast) -> R) -> R {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let errors: Vec<String> =
        sink.take().iter().filter(|d| d.level >= Level::Error).map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "{errors:?}\n{src}");
    check(&tu.ast)
}

fn unnamed_class(ast: &Ast, d: DeclId) -> bool {
    let n = ast.decls.get(d);
    n.kind() == DeclKind::CXXRecord && n.name().is_empty() && ast.definition(d) == Some(d)
}

/// Whether `d` is (inside) an instantiated class: a specialization or a
/// member of one, through its enclosing classes.
fn instantiated(ast: &Ast, d: DeclId) -> bool {
    let mut c = Some(d);
    while let Some(x) = c.filter(|&x| ast.decls.get(x).kind() == DeclKind::CXXRecord) {
        if ast.spec_info(x).is_some() || ast.member_spec_info(x).is_some() {
            return true;
        }
        c = ast.decls.get(x).ctx();
    }
    false
}

/// Every instantiated unnamed class links to an unnamed class of the
/// pattern that is not itself instantiated, as a member (never as a
/// specialization), and is the member its enclosing class records for
/// that pattern. Returns how many were checked.
fn check_links(src: &str) -> usize {
    parse(src, |ast| {
        let mut n = 0;
        for x in ast.decls.ids().filter(|&d| unnamed_class(ast, d) && instantiated(ast, d)) {
            let p = ast.instantiated_from(x).unwrap_or_else(|| panic!("no pattern for {x:?}"));
            assert!(unnamed_class(ast, p), "pattern {p:?} of {x:?} is an unnamed class");
            assert!(!instantiated(ast, p), "pattern {p:?} of {x:?} is the template's");
            assert!(ast.spec_info(x).is_none(), "{x:?} is no specialization");
            let (_, info) = ast.member_spec_info(x).expect("a member's link");
            assert_eq!(info.kind, SpecKind::ImplicitInstantiation);
            let owner = ast.decls.get(x).ctx().expect("an enclosing class");
            assert_eq!(ast.templates.find_member(owner, p), Some(x));
            // The same tag as the pattern.
            assert_eq!(ast.decls.record(x).map(|r| r.tag), ast.decls.record(p).map(|r| r.tag));
            n += 1;
        }
        n
    })
}

#[test]
fn member_object_and_anonymous_union() {
    let src = "template <class T> struct S {\n\
                 struct { T x; } m;\n\
                 union { T a; int b; };\n\
                 struct { T c; };\n\
               };\n\
               S<int> s1; S<long> s2;\n\
               int f() { return s1.m.x + s1.a + s1.c + s2.b; }\n";
    assert_eq!(check_links(src), 6);
}

#[test]
fn nested_unnamed_classes() {
    let src = "template <class T> struct S {\n\
                 struct { struct { T y; } in; union { T u; char k; }; } m;\n\
               };\n\
               S<int> s;\n\
               int f() { return s.m.in.y + s.m.u; }\n";
    assert_eq!(check_links(src), 3);
}

#[test]
fn member_class_template() {
    let src = "template <class T> struct O {\n\
                 template <class U> struct I { struct { T t; U u; } m; union { U a; T b; }; };\n\
                 struct N { struct { T v; } m; };\n\
               };\n\
               O<int>::I<long> i;\n\
               O<char>::N n;\n\
               long f() { return i.m.t + i.m.u + i.a + n.m.v; }\n";
    assert_eq!(check_links(src), 3);
}

#[test]
fn partial_specialization() {
    let src = "template <class T, class U> struct P;\n\
               template <class T> struct P<T, int> { union { T a; int b; }; struct { T c; } m; };\n\
               template <class T> struct P<T *, T> { struct { T d; } m; };\n\
               P<char, int> p1;\n\
               P<long *, long> p2;\n\
               long f() { return p1.a + p1.m.c + p2.m.d; }\n";
    assert_eq!(check_links(src), 3);
}

#[test]
fn unnamed_enumeration_still_linked() {
    // The record unnamed member classes now share.
    let src = "template <class T> struct S { enum { A = sizeof(T) } e; };\n\
               S<int> s;\n";
    parse(src, |ast| {
        let e = ast
            .decls
            .ids()
            .find(|&d| {
                ast.decls.get(d).kind() == DeclKind::Enum
                    && ast.decls.get(d).name().is_empty()
                    && ast.member_spec_info(d).is_some()
            })
            .expect("an instantiated unnamed enumeration");
        assert!(ast.instantiated_from(e).is_some());
    });
}
