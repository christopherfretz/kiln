//! WP-658: the redeclaration index (spec/sema.md §3) changes no result.
//! Each program declares more same-name entities in one namespace than
//! the index's threshold, then redeclares some of them through friend
//! declarations (hidden friends, friend templates with different heads,
//! instantiated friends, using-declarators, invalid, deleted and
//! retyped declarations). It is analysed twice, with the index and with
//! every redeclaration search scanning its whole lookup entry
//! (`ParseOptions::full_redecl_scan`, the test-only entry), and the two
//! must agree on every diagnostic and note (the `prev` a note names), on
//! the AST dump, and on every declaration's redeclaration link (the
//! `prev` chosen).

mod common;

use std::fmt::Write as _;
use std::sync::Arc;

use common::{Parsed, opts19, parse_with};
use kiln_ast::DeclId;
use kiln_ast::decl::DeclFlags;
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ParseOptions, parse_file};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// More entries than the index threshold (`redecl_index::MIN_LEN`).
const N: usize = 48;

/// Each declaration's offset, its previous declaration's offset (if it
/// has one), and whether it is invalid.
fn chains(src: &str, opts: &ParseOptions) -> Vec<String> {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let dopts = DiagOptions { clang_compat: opts.clang_compat, ..DiagOptions::default() };
    let engine = DiagEngine::new(&sm, &dopts, MemorySink::new());
    let ast = parse_file(&sm, &interner, file, &engine, Target::for_arch(Arch::X86_64), opts);
    let off = |d: DeclId| sm.offset_of(ast.decls.get(d).loc()).map_or(u32::MAX, |(_, o)| o);
    let mut out = Vec::new();
    for i in 1..=ast.decls.len() {
        let d = DeclId::new(i as u32);
        let Some(r) = ast.decls.redecl(d) else { continue };
        let n = ast.decls.get(d);
        let bad = if n.has(DeclFlags::INVALID) { " invalid" } else { "" };
        out.push(format!("{:?} @{} prev={:?}{bad}", n.kind(), off(d), r.prev.map(off)));
    }
    out
}

/// Analyses `src` both ways and asserts they agree; returns the indexed
/// result.
#[track_caller]
fn same_both_ways(src: &str) -> Parsed {
    let full_opts = ParseOptions { full_redecl_scan: true, ..opts19() };
    let fast = parse_with(src, &opts19());
    let full = parse_with(src, &full_opts);
    assert_eq!(fast.diags, full.diags, "diagnostics differ with the redeclaration index");
    assert_eq!(fast.dump, full.dump, "AST dump differs with the redeclaration index");
    assert_eq!(chains(src, &opts19()), chains(src, &full_opts), "redeclaration links differ");
    fast
}

/// `N` classes, each with hidden friends of the same names in one
/// namespace.
fn classes(out: &mut String, from: usize) {
    for i in from..from + N {
        let _ = writeln!(
            out,
            "struct C{i} {{ int v; friend bool operator==(const C{i}&, const C{i}&) {{ return true; }} \
             friend void swap(C{i}&, C{i}&) {{}} friend int get(C{i}*, ...); }};"
        );
    }
}

/// Hidden friends redeclared by later friends and namespace-scope
/// declarations, a redefinition (its note names the definition), and a
/// variadic/non-variadic pair.
#[test]
fn hidden_friends_match_the_full_path() {
    let mut s = String::from("namespace n {\n");
    classes(&mut s, 0);
    s.push_str(
        "struct D { friend bool operator==(const C3&, const C3&); friend void swap(C7&, C7&); \
         friend int get(C9*); friend int get(C9*, ...); };\n\
         bool operator==(const C5&, const C5&);\n\
         struct E { friend void swap(C11&, C11&) {} friend bool operator==(const C2&, const C2&) { return false; } };\n\
         void f() { C1 a, b; (void)(a == b); swap(a, b); C9 c; get(&c); get(&c, 1); }\n}\n",
    );
    let p = same_both_ways(&s);
    p.assert_diag("redefinition of 'swap'");
    p.assert_diag("redefinition of 'operator=='");
}

/// Overload sets declared at namespace scope, then befriended: the
/// friend redeclares the one with its parameter types, or declares a
/// new overload; a friend differing only in its return type is an
/// error.
#[test]
fn overload_sets_match_the_full_path() {
    let mut s = String::from("namespace n {\n");
    for i in 0..N {
        let _ =
            writeln!(s, "struct T{i} {{}}; void ov(T{i}); int ov(T{i}, int); void ov(T{i}*, ...);");
    }
    s.push_str(
        "struct F { friend void ov(T4); friend int ov(T4, int) { return 1; } friend void ov(T5, long); \
         friend void ov(T6*, ...); friend long ov(T7, int); };\n\
         void ov(T4) {}\n\
         void g() { ov(T4{}); ov(T5{}, 1L); ov(T4{}, 1); }\n}\n",
    );
    let p = same_both_ways(&s);
    p.assert_diag("functions that differ only in their return type cannot be overloaded");
}

/// Friend templates with different heads (type, non-type, two
/// parameters, constrained) against same-parameter namespace templates
/// and non-template functions of the same name.
#[test]
fn templates_with_different_heads_match_the_full_path() {
    let mut s = String::from("namespace n {\n");
    for i in 0..N {
        let _ = writeln!(
            s,
            "struct K{i} {{}}; template <class T> void tf(T, K{i}*); template <int M> void tf(K{i}, int); \
             void tf(K{i}*, K{i}*);"
        );
    }
    s.push_str(
        "template <class T> concept Small = sizeof(T) < 8;\n\
         struct G {\n\
           template <class T> friend void tf(T, K3*);\n\
           template <class U> friend void tf(U, K4*) {}\n\
           template <int M> friend void tf(K5, int);\n\
           template <long M> friend void tf(K6, int);\n\
           template <class T, class U> friend void tf(T, K7*);\n\
           template <Small T> friend void tf(T, K8*);\n\
           friend void tf(K9*, K9*);\n\
         };\n\
         template <class V> void tf(V, K4*) {}\n\
         void h() { tf(1, (K3*)nullptr); tf<1>(K5{}, 1); tf((K9*)nullptr, (K9*)nullptr); }\n}\n",
    );
    let p = same_both_ways(&s);
    p.assert_diag("redefinition of 'tf'");
}

/// Friends declared by the instantiations of class templates: each
/// specialization's hidden friends join the namespace's set, and two
/// instantiations defining the same function are a redefinition.
#[test]
fn instantiated_friends_match_the_full_path() {
    let mut s = String::from(
        "namespace n {\n\
         template <class T> struct W { friend bool operator==(const W&, const W&) { return true; } \
         template <class U> friend bool operator==(const W&, const W<U*>&) { return false; } };\n\
         template <class T> struct R { friend void dup(int) {} friend void dup(T) {} };\n",
    );
    for i in 0..N {
        let _ = writeln!(
            s,
            "struct A{i} {{}}; void dup(A{i}*); W<A{i}> w{i}; bool b{i} = w{i} == w{i};"
        );
    }
    s.push_str("void dup(long) {}\nR<long> r1; R<char> r2;\n}\n");
    let p = same_both_ways(&s);
    p.assert_diag("redefinition of 'dup'");
}

/// A namespace's using-declarators among many same-name functions: a
/// friend overloads the target, or, corresponding to it, cannot
/// befriend it.
#[test]
fn using_declarators_match_the_full_path() {
    let mut s = String::from(
        "namespace a { struct U {}; void uf(U); void uf(int, U); }\nnamespace n {\nusing a::uf;\n",
    );
    for i in 0..N {
        let _ = writeln!(s, "struct B{i} {{ friend void uf(B{i}) {{}} }};");
    }
    s.push_str(
        "struct H { friend void uf(a::U); friend void uf(long, a::U); friend void uf(B3); };\n}\n",
    );
    same_both_ways(&s);
}

/// Invalid declarations (an undeclared parameter type, a non-prototype
/// error type, a conflicting other entity), deleted functions, and
/// functions whose deduced return type is formed after their entry was
/// indexed (a retyped declaration).
#[test]
fn invalid_deleted_and_retyped_match_the_full_path() {
    let mut s = String::from("namespace n {\n");
    for i in 0..N {
        let _ = writeln!(
            s,
            "struct V{i} {{}}; void dl(V{i}) = delete; auto rt(V{i}) {{ return {i}; }} void bad(V{i}*);"
        );
    }
    s.push_str(
        "void bad(V1, Undeclared1);\n\
         void bad(V2*, Undeclared2);\n\
         int rt0 = rt(V0{});\n\
         struct I {\n\
           friend void dl(V1);\n\
           friend void dl(V2) {}\n\
           friend auto rt(V3);\n\
           friend int rt(V4);\n\
           friend void bad(V1, Undeclared1);\n\
           friend void bad(V2*, int);\n\
           friend void bad(V3*);\n\
           friend Missing bad(V7);\n\
         };\n\
         int k;\n\
         struct J { friend void k(V8); };\n\
         auto rt(V3);\n\
         int use = rt(V3{});\n}\n",
    );
    let p = same_both_ways(&s);
    p.assert_diag("unknown type name 'Undeclared1'");
    p.assert_diag("redefinition of 'dl'");
    p.assert_diag("functions that differ only in their return type cannot be overloaded");
    p.assert_diag("redefinition of 'k' as different kind of symbol");
}

/// Members of an inline namespace (visible in the enclosing namespace,
/// not redeclared by its friends) beside the enclosing namespace's own
/// hidden friends.
#[test]
fn inline_namespace_members_match_the_full_path() {
    let mut s = String::from("namespace n {\ninline namespace v1 {\n");
    for i in 0..N {
        let _ = writeln!(s, "struct L{i} {{}}; void in(L{i});");
    }
    s.push_str("}\n");
    for i in 0..N {
        let _ =
            writeln!(s, "struct M{i} {{ friend void in(L{i}) {{}} friend void in(M{i}) {{}} }};");
    }
    s.push_str("void q() { in(L1{}); in(M1{}); }\n}\n");
    same_both_ways(&s);
}

/// A class with many overloaded members, defined out of line and
/// befriended by another class.
#[test]
fn members_match_the_full_path() {
    let mut s = String::from("namespace n {\nstruct P {\n");
    for i in 0..N {
        let _ = writeln!(s, "  void m(int (&)[{}]); int m(char (&)[{}]) const;", i + 1, i + 1);
    }
    s.push_str("};\n");
    for i in (0..N).step_by(5) {
        let _ = writeln!(s, "void P::m(int (&)[{}]) {{}}", i + 1);
    }
    s.push_str(
        "int P::m(char (&)[3]) { return 0; }\n\
         struct Q { friend void P::m(int (&)[2]); friend int P::m(char (&)[4]) const; };\n}\n",
    );
    same_both_ways(&s);
}

/// WP-661: the index (built for the friends below), its journals and the
/// explicit-argument memo have memory rows (`Sema::overload_memory`,
/// `Ast::memory`).
#[test]
fn memory_rows_report_the_index_and_the_memo() {
    use kiln_lex::Std;
    use kiln_lex::literal::{EncodingParams, LiteralOptions};
    use kiln_parse::{Parser, TokenSource};
    use kiln_pp::{PpOptions, Preprocessor};
    use kiln_sema::Sema;

    let mut src = String::from("namespace ns {\n");
    for i in 1..=N {
        writeln!(src, "void f(char (&)[{i}]);").unwrap();
    }
    src.push_str("struct D { friend void f(char (&)[3]); friend void f(char (&)[40]); };\n");
    src.push_str("template <class T> T t(T x) { return x; }\n}\n");
    src.push_str("int a = ns::t<int>(1) + ns::t<int>(2);\n");
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let diags = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::X86_64);
    let mut pp = Preprocessor::new(&sm, &diags, &interner, PpOptions::new(Std::Cxx26));
    pp.enter_main_file(file);
    let ast = kiln_ast::Ast::new(
        Arc::clone(&interner),
        kiln_ast::types::TypeContext::new(target.types.clone()),
    );
    let enc = EncodingParams {
        char_signed: target.types.char_signed,
        wchar_bits: 32,
        wchar_signed: target.types.wchar_signed,
    };
    let s = Sema::for_target(ast, &diags, target);
    let mut p = Parser::new(s, TokenSource::new(pp, &diags), LiteralOptions::new(Std::Cxx26, enc));
    p.parse_translation_unit();
    let (s, _) = p.finish();
    assert_eq!(diags.error_count(), 0, "{:#?}", sink.diagnostics());
    let ovl = s.overload_memory();
    let index = ovl.row("sema.redecl_index").expect("index row");
    assert!(index.count >= N as u64 && index.live_bytes > 0, "{index:?}");
    let memo = ovl.row("sema.explicit_memo").expect("memo row");
    assert!(memo.count >= 1 && memo.live_bytes > 0, "{memo:?}");
    let mem = s.ast.memory();
    for name in ["journal.lookup", "journal.decl_types", "journal.redecls"] {
        let row = mem.row(name).unwrap_or_else(|| panic!("no {name} row"));
        assert!(row.reserved_bytes >= row.live_bytes, "{row:?}");
    }
}
