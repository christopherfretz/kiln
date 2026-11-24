//! The key printer (spec/types.md §12.1, spec/ast.md §20.5; WP-2477,
//! WP-2545): `AstNames::type_key` prints two different types apart where
//! clang's diagnostic text, which kiln's diagnostic printer follows,
//! prints them alike. Each case parses a TU, takes the types of the
//! variables named `v<N>`, and compares their keys and their diagnostic
//! (canonical) spelling.

use std::sync::Arc;

use kiln_ast::decl::DeclKind;
use kiln_ast::diag_names::DiagNames;
use kiln_ast::dump::AstNames;
use kiln_ast::types::{PrintOptions, print_type};
use kiln_ast::{Ast, QualType};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ParseOptions, parse_file};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The types of the variables `v0`, `v1`, ... of `src` (which must
/// compile cleanly), in name order, as (key, diagnostic canonical
/// spelling).
fn keys(src: &str) -> Vec<(String, String)> {
    let interner = Arc::new(Interner::new());
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions::default();
    let ast: Ast = parse_file(&sm, &interner, file, &engine, Target::for_arch(Arch::X86_64), &opts);
    let diags: Vec<String> = sink.take().iter().map(|d| d.message.clone()).collect();
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    let mut vars: Vec<(u32, QualType)> = Vec::new();
    for d in ast.decls.ids() {
        let n = ast.decls.get(d);
        if n.kind() != DeclKind::Var {
            continue;
        }
        let Some(name) = ast.names.simple_spelling(n.name(), &interner) else { continue };
        if let Some(i) = name.strip_prefix('v').and_then(|i| i.parse().ok()) {
            vars.push((i, n.ty().expect("a variable's type")));
        }
    }
    // A function template's variables repeat in each instantiation, in
    // creation order (the pattern's first).
    vars.sort_by_key(|v| v.0);
    let key_names = AstNames::for_keys(&ast, &sm);
    let diag_names = DiagNames::new(&ast, Some(&sm), 19);
    vars.iter()
        .map(|&(_, q)| {
            (key_names.type_key(q), print_type(&ast.types, &diag_names, q, PrintOptions::CANONICAL))
        })
        .collect()
}

/// The types of the variables `v0`, `v1`, ... of `src` as the
/// uncaught-exception note names an exception object's type
/// (`DiagNames::thrown_type_name`, WP-3408) at `-fclang-compat=<compat>`,
/// with the plain diagnostic printer's canonical spelling.
fn thrown(src: &str, compat: u32) -> Vec<(String, String)> {
    let interner = Arc::new(Interner::new());
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions::default();
    let ast: Ast = parse_file(&sm, &interner, file, &engine, Target::for_arch(Arch::X86_64), &opts);
    let diags: Vec<String> = sink.take().iter().map(|d| d.message.clone()).collect();
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    let mut vars: Vec<(u32, QualType)> = Vec::new();
    for d in ast.decls.ids() {
        let n = ast.decls.get(d);
        if n.kind() != DeclKind::Var {
            continue;
        }
        let Some(name) = ast.names.simple_spelling(n.name(), &interner) else { continue };
        if let Some(i) = name.strip_prefix('v').and_then(|i| i.parse().ok()) {
            vars.push((i, n.ty().expect("a variable's type")));
        }
    }
    vars.sort_by_key(|v| v.0);
    let names = DiagNames::new(&ast, Some(&sm), compat);
    vars.iter()
        .map(|&(_, q)| {
            (names.thrown_type_name(q), print_type(&ast.types, &names, q, PrintOptions::CANONICAL))
        })
        .collect()
}

/// WP-3408 (spec/oven.md "Exceptions", spec/types.md §12.1): the
/// thrown-type printer is the diagnostic printer on the canonical,
/// cv-unqualified type, except that a class or enumeration declared in a
/// function prints through it, at both compatibility levels and inside
/// template arguments; so a local class and a class of the unnamed
/// namespace never print as the namespace-scope `error`, which the
/// diagnostic printer prints alike.
#[test]
fn thrown_type_names_follow_one_rule() {
    let src = "struct error {};\n\
               namespace lib { inline namespace v0 { struct error {}; } }\n\
               namespace app { namespace lib { inline namespace v0 { struct error {}; } } }\n\
               namespace m { inline namespace v1 { struct e {}; } inline namespace v2 { struct e {}; } }\n\
               namespace u { namespace { struct error {}; } }\n\
               template <class T> struct box { struct nested {}; };\n\
               using alias = const lib::v0::error;\n\
               error v0;\n\
               lib::v0::error v1;\n\
               alias v2;\n\
               app::lib::v0::error v3;\n\
               m::v1::e v4;\n\
               box<alias>::nested v5;\n\
               u::error v12;\n\
               const app::lib::error *v6;\n\
               void f() { struct error {}; error v7; box<error> v8; }\n\
               namespace q { void g(int) { enum E { x }; struct o { struct in {}; }; E v9; o::in v10; } }\n\
               void h() { struct error {}; error x; volatile error &v11 = x; }\n";
    for compat in [19, 22] {
        let got = thrown(src, compat);
        let t: Vec<&str> = got.iter().map(|g| g.0.as_str()).collect();
        assert_eq!(
            t,
            [
                "error",
                "lib::error",
                "lib::error",
                "app::lib::error",
                "m::v1::e",
                "box<const lib::error>::nested",
                "const app::lib::error *",
                "f()::error",
                "box<f()::error>",
                "q::g(int)::E",
                "q::g(int)::o::in",
                "volatile h()::error &",
                "u::(anonymous namespace)::error",
            ][..],
            "compat {compat}"
        );
        // Where no function is involved it is the diagnostic printer's
        // canonical spelling; there the local classes print bare.
        assert_eq!(got[12].0, got[12].1, "{compat}");
        // `v2`'s type is `const`: an exception object's type drops it.
        assert_eq!(got[2].1, "const lib::error", "{compat}");
        for i in [0, 1, 3, 4, 5, 6] {
            assert_eq!(got[i].0, got[i].1, "{compat}: v{i}");
        }
        assert_eq!(got[7].1, "error", "{compat}");
        assert_eq!(got[8].1, "box<error>", "{compat}");
    }
}

/// Asserts the keys are `want`, all distinct, while the diagnostic
/// spellings of the first two are `alike`.
fn check(src: &str, want: &[&str], alike: &str) {
    let got = keys(src);
    let k: Vec<&str> = got.iter().map(|g| g.0.as_str()).collect();
    assert_eq!(k, want, "keys");
    for (i, a) in k.iter().enumerate() {
        assert!(!k[..i].contains(a), "{a} printed twice");
    }
    assert_eq!(got[0].1, alike);
    assert_eq!(got[1].1, alike);
}

#[test]
fn null_pointer_arguments_keep_their_type() {
    check(
        "template <auto P> struct S {};\n\
         template <int *P> struct T {};\n\
         struct X { int m; };\n\
         S<(int *)nullptr> v0;\n\
         S<(char *)nullptr> v1;\n\
         S<nullptr> v2;\n\
         S<(int X::*)nullptr> v3;\n\
         T<nullptr> v4;\n",
        &[
            "S<(int *)nullptr>",
            "S<(char *)nullptr>",
            "S<nullptr>",
            "S<(int X::*)nullptr>",
            "T<nullptr>",
        ],
        "S<nullptr>",
    );
}

#[test]
fn local_classes_carry_their_function() {
    check(
        "template <class T> struct store {};\n\
         namespace n { void f(int); }\n\
         void n::f(int) { struct Local {}; store<Local> v0; }\n\
         void g() { struct Local {}; store<Local> v1; }\n\
         void g(long) { struct Local { struct In {}; }; store<Local::In> v2; }\n\
         struct C {\n\
           void m() const & { struct Local {}; store<Local> v3; }\n\
           void m() && { struct Local {}; store<Local> v4; }\n\
         };\n\
         template <class U> void k() { struct Local {}; store<Local> v5; }\n\
         template void k<int>();\n\
         template void k<long>();\n\
         void s() { { struct Local {}; store<Local> v6; } { struct Local {}; store<Local> v7; } }\n\
         void t() { static int x; struct Local {}; store<Local> v8; }\n",
        &[
            "store<n::f(int)::Local>",
            "store<g()::Local>",
            "store<g(long)::Local::In>",
            "store<C::m() const &::Local>",
            "store<C::m() &&::Local>",
            "store<k()::Local>",
            "store<k<int>()::Local>",
            "store<k<long>()::Local>",
            "store<s()::Local>",
            "store<s()::Local#2>",
            "store<t()::Local>",
        ],
        "store<Local>",
    );
}

#[test]
fn closures_and_unnamed_local_classes_carry_their_location() {
    let got = keys(
        "template <class T> struct store {};\n\
         void f() { auto l = [] {}; store<decltype(l)> v0; }\n\
         void g() { auto l = [] {}; store<decltype(l)> v1; }\n\
         void h() { struct { int a; } u; store<decltype(u)> v2; }\n\
         void i() { struct { int a; } u; store<decltype(u)> v3; }\n",
    );
    let k: Vec<&str> = got.iter().map(|g| g.0.as_str()).collect();
    assert_eq!(
        k,
        [
            "store<f()::(lambda at t.cpp:2:21)>",
            "store<g()::(lambda at t.cpp:3:21)>",
            "store<h()::(unnamed struct at t.cpp:4:12)>",
            "store<i()::(unnamed struct at t.cpp:5:12)>",
        ]
    );
}

#[test]
fn local_entities_as_values_carry_their_function() {
    check(
        "template <int *P> struct T {};\n\
         template <auto E> struct U {};\n\
         void f() { static int x; T<&x> v0; enum E { e }; U<e> v2; }\n\
         void g() { static int x; T<&x> v1; enum E { e }; U<e> v3; }\n",
        &["T<&f()::x>", "T<&g()::x>", "U<f()::e>", "U<g()::e>"],
        "T<&x>",
    );
}

/// WP-3481 (spec/types.md §12.1 "Declaration keys"): a pointer argument
/// names its object or function by its declaration key, so the static
/// locals `x` of sibling blocks ([stmt.dcl]/3) and two overloads ([over])
/// print apart, where the diagnostic printer prints them alike.
#[test]
fn declarations_as_values_are_told_apart() {
    check(
        "template <int *P> struct T {};\n\
         template <auto F> struct H {};\n\
         int ov(int);\n\
         int ov(long);\n\
         void s() { { static int x; T<&x> v0; } { static int x; T<&x> v1; } }\n\
         H<static_cast<int (*)(int)>(ov)> v2;\n\
         H<static_cast<int (*)(long)>(ov)> v3;\n",
        &["T<&s()::x>", "T<&s()::x#2>", "H<&ov(int)>", "H<&ov(long)>"],
        "T<&x>",
    );
}
