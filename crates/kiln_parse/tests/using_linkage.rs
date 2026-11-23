//! Using-declaration shadows and language linkage as the `Ast` keeps them
//! after the translation unit, for tooling clients (spec/ast.md §4.2 and
//! §4.5; WP-2025): `Ast::using_shadows` / `Ast::shadow_using`
//! ([namespace.udecl], [enum.udecl], [class.inhctor.init]),
//! `Ast::language_linkage` ([dcl.link]), and how repeated
//! using-declarations and namespace aliases are kept ([namespace.udecl],
//! [namespace.alias]). Each test parses a TU, drops sema, and reads the
//! `Ast` alone.

use std::sync::Arc;

use kiln_ast::decl::DeclKind;
use kiln_ast::linkage::{LanguageLinkage, Linkage};
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

/// Every declaration of kind `kind` named `n`, in creation order.
fn all(ast: &Ast, i: &Interner, kind: DeclKind, n: &str) -> Vec<DeclId> {
    ast.decls.ids().filter(|&d| ast.decls.get(d).kind() == kind && name(ast, i, d) == n).collect()
}

/// The only declaration of kind `kind` named `n`.
fn one(ast: &Ast, i: &Interner, kind: DeclKind, n: &str) -> DeclId {
    let v = all(ast, i, kind, n);
    assert_eq!(v.len(), 1, "{kind:?} {n}: {v:?}");
    v[0]
}

/// The kind of `d`'s semantic context.
fn ctx_kind(ast: &Ast, d: DeclId) -> DeclKind {
    ast.decls.get(ast.decls.get(d).ctx().expect("a context")).kind()
}

/// The shadows of `u`, checked against the reverse link, mapped to the
/// declarations they introduce.
fn targets(ast: &Ast, u: DeclId) -> Vec<DeclId> {
    ast.using_shadows(u)
        .map(|s| {
            assert_eq!(ast.shadow_using(s), Some(u), "reverse link of {s:?}");
            ast.using_target(s).expect("a shadow names its target")
        })
        .collect()
}

/// [namespace.udecl]/1, /2: a namespace-scope using-declarator introduces
/// one shadow per declaration lookup finds, in lookup order.
#[test]
fn namespace_scope_using() {
    let (ast, i) = parse(
        "namespace n { void f(); void f(int); int g; }\n\
         using n::f;\n\
         using n::g;\n",
    );
    let fs = all(&ast, &i, DeclKind::Function, "f");
    let g = one(&ast, &i, DeclKind::Var, "g");
    let uf = one(&ast, &i, DeclKind::Using, "f");
    let ug = one(&ast, &i, DeclKind::Using, "g");
    assert_eq!(targets(&ast, uf), fs);
    assert_eq!(targets(&ast, ug), vec![g]);
    for s in ast.using_shadows(uf) {
        assert_eq!(ast.decls.get(s).kind(), DeclKind::UsingShadow);
    }
    // A declaration that is no shadow has no using-declaration.
    assert_eq!(ast.shadow_using(g), None);
    assert_eq!(ast.using_shadows(g).len(), 0);
}

/// [namespace.udecl]/3, /14: a member using-declarator introduces the base
/// class members in the derived class; one hidden by a member of the
/// derived class stays in the list.
#[test]
fn class_scope_using() {
    let (ast, i) = parse(
        "struct B { void m(); void m(int); int x; };\n\
         struct D : B { using B::m; using B::x; void m(int); };\n",
    );
    let ms: Vec<DeclId> = all(&ast, &i, DeclKind::CXXMethod, "m");
    assert_eq!(ms.len(), 3);
    let x = one(&ast, &i, DeclKind::Field, "x");
    let um = one(&ast, &i, DeclKind::Using, "m");
    let ux = one(&ast, &i, DeclKind::Using, "x");
    assert_eq!(ctx_kind(&ast, um), DeclKind::CXXRecord);
    assert_eq!(targets(&ast, um), ms[..2].to_vec());
    assert_eq!(targets(&ast, ux), vec![x]);
}

/// [enum.udecl]/2: a using-enum-declarator introduces the enumerator
/// names, at namespace and at class scope.
#[test]
fn using_enum() {
    let (ast, i) = parse(
        "enum class E { a, b, c };\n\
         namespace n { using enum E; }\n\
         struct S { using enum E; };\n",
    );
    let es: Vec<DeclId> =
        ["a", "b", "c"].iter().map(|n| one(&ast, &i, DeclKind::EnumConstant, n)).collect();
    let ues = all(&ast, &i, DeclKind::UsingEnum, "");
    let ues: Vec<DeclId> = if ues.is_empty() {
        ast.decls.ids().filter(|&d| ast.decls.get(d).kind() == DeclKind::UsingEnum).collect()
    } else {
        ues
    };
    assert_eq!(ues.len(), 2);
    assert_eq!(ctx_kind(&ast, ues[0]), DeclKind::Namespace);
    assert_eq!(ctx_kind(&ast, ues[1]), DeclKind::CXXRecord);
    for u in ues {
        assert_eq!(targets(&ast, u), es);
    }
}

/// [namespace.udecl]/3, /14, [class.inhctor.init]: `using B::B;`
/// introduces one constructor shadow per constructor of `B` lookup finds,
/// the implicit copy and move constructors included ([over.match.funcs]/9
/// excludes those from overload resolution, not from the declaration).
/// One a later constructor of the derived class hides stays in the list.
#[test]
fn inheriting_constructors() {
    let (ast, i) = parse(
        "struct B { B(int); B(double); B(char); };\n\
         struct D : B { using B::B; D(char); };\n\
         D d(1);\n",
    );
    let b = one(&ast, &i, DeclKind::CXXRecord, "B");
    let dd = one(&ast, &i, DeclKind::CXXRecord, "D");
    let in_class = |kind: DeclKind, class: DeclId| -> Vec<DeclId> {
        ast.decls
            .ids()
            .filter(|&d| ast.decls.get(d).kind() == kind && ast.decls.get(d).ctx() == Some(class))
            .collect()
    };
    let ctors = in_class(DeclKind::CXXConstructor, b);
    assert_eq!(ctors.len(), 5, "three written, the implicit copy and move");
    let us = in_class(DeclKind::Using, dd);
    assert_eq!(us.len(), 1);
    let u = us[0];
    for s in ast.using_shadows(u) {
        assert_eq!(ast.decls.get(s).kind(), DeclKind::ConstructorUsingShadow);
        assert_eq!(ast.decls.get(s).ctx(), Some(dd));
    }
    assert_eq!(targets(&ast, u), ctors);
}

/// [temp.res.general]/5, [temp.inst]/3: a dependent using-declarator
/// introduces nothing; the using-declaration its instantiation makes in
/// the specialization has the shadows.
#[test]
fn dependent_using() {
    let (ast, i) = parse(
        "template<class T> struct D : T { using T::m; };\n\
         struct B { void m(); void m(int); };\n\
         D<B> d;\n\
         void use() { d.m(); }\n",
    );
    let pattern = one(&ast, &i, DeclKind::UnresolvedUsingValue, "m");
    assert_eq!(ast.using_shadows(pattern).len(), 0);
    let u = one(&ast, &i, DeclKind::Using, "m");
    // The specialization `D<B>`, not the pattern.
    assert_eq!(ctx_kind(&ast, u), DeclKind::CXXRecord);
    assert_ne!(ast.decls.get(u).ctx(), ast.decls.get(pattern).ctx());
    assert_eq!(targets(&ast, u), all(&ast, &i, DeclKind::CXXMethod, "m"));
}

/// [namespace.udecl]/1, [temp.variadic]/5: a pack of using-declarators
/// expands at instantiation into one using-declaration per element, each
/// with the shadows of its own base.
#[test]
fn using_declarator_pack() {
    let (ast, i) = parse(
        "struct A { void g(int); };\n\
         struct B { void g(double); };\n\
         template<class... Ts> struct O : Ts... { using Ts::g...; };\n\
         O<A, B> o;\n\
         void use() { o.g(1); o.g(1.0); }\n",
    );
    let gs = all(&ast, &i, DeclKind::CXXMethod, "g");
    assert_eq!(gs.len(), 2);
    let us = all(&ast, &i, DeclKind::Using, "g");
    assert_eq!(us.len(), 2, "one per pack element");
    for (&u, &g) in us.iter().zip(&gs) {
        assert_eq!(targets(&ast, u), vec![g]);
    }
}

/// [namespace.udecl]/1 at block scope, in a function template body: the
/// pattern's using-declaration has its shadow, and so has each
/// instantiation's.
#[test]
fn block_scope_using_in_template() {
    let (ast, i) = parse(
        "namespace n { int h(int); }\n\
         template<class T> int call(T t) { using n::h; return h(t); }\n\
         int k = call(1);\n",
    );
    let h = one(&ast, &i, DeclKind::Function, "h");
    let us = all(&ast, &i, DeclKind::Using, "h");
    assert!(!us.is_empty());
    for u in us {
        assert_eq!(ctx_kind(&ast, u), DeclKind::Function);
        assert_eq!(targets(&ast, u), vec![h]);
    }
}

/// [namespace.udecl], [namespace.alias] (KQ1 Q14): two using-declarations
/// of one name, and two namespace aliases of one name, are separate
/// declarations, never chained as redeclarations: neither is an entity
/// ([basic.pre]/3); each denotes the same entity.
#[test]
fn repeated_using_and_alias() {
    let (ast, i) = parse(
        "namespace n { void f(); }\n\
         using n::f;\n\
         using n::f;\n\
         namespace a = n;\n\
         namespace a = n;\n",
    );
    let f = one(&ast, &i, DeclKind::Function, "f");
    let us = all(&ast, &i, DeclKind::Using, "f");
    assert_eq!(us.len(), 2);
    for &u in &us {
        assert_eq!(ast.first(u), u);
        assert_eq!(ast.prev(u), None);
    }
    let shadows: Vec<DeclId> = us.iter().flat_map(|&u| ast.using_shadows(u)).collect();
    assert_eq!(shadows.len(), 2, "each using-declaration has its own shadow");
    assert_ne!(shadows[0], shadows[1]);
    for (&s, &u) in shadows.iter().zip(&us) {
        assert_eq!(ast.shadow_using(s), Some(u));
        assert_eq!(ast.first(s), s);
        assert_eq!(ast.strip_shadow(s), f);
    }
    let aliases = all(&ast, &i, DeclKind::NamespaceAlias, "a");
    assert_eq!(aliases.len(), 2);
    let n = one(&ast, &i, DeclKind::Namespace, "n");
    for &a in &aliases {
        assert_eq!(ast.first(a), a);
        assert_eq!(ast.prev(a), None);
        assert_eq!(ast.resolve_alias(a), n);
    }
}

/// The language linkage of the function or variable named `n`, from
/// every declaration of it.
fn lang(ast: &Ast, i: &Interner, kind: DeclKind, n: &str) -> LanguageLinkage {
    let ds = all(ast, i, kind, n);
    assert!(!ds.is_empty(), "no {kind:?} {n}");
    let l = ast.language_linkage(ds[0]);
    for &d in &ds[1..] {
        assert_eq!(ast.language_linkage(d), l, "{n}: redeclarations agree");
    }
    l
}

/// [dcl.link]/2, /4: declarations in an `extern "C"` block and an
/// `extern "C"` declaration have C language linkage; others C++.
#[test]
fn extern_c_block_and_declaration() {
    let (ast, i) = parse(
        "extern \"C\" { void cf(); int cv; }\n\
         extern \"C\" void cd();\n\
         extern \"C\" int cdv;\n\
         void plain();\n\
         int plainv;\n",
    );
    assert_eq!(lang(&ast, &i, DeclKind::Function, "cf"), LanguageLinkage::C);
    assert_eq!(lang(&ast, &i, DeclKind::Var, "cv"), LanguageLinkage::C);
    assert_eq!(lang(&ast, &i, DeclKind::Function, "cd"), LanguageLinkage::C);
    assert_eq!(lang(&ast, &i, DeclKind::Var, "cdv"), LanguageLinkage::C);
    assert_eq!(lang(&ast, &i, DeclKind::Function, "plain"), LanguageLinkage::Cxx);
    assert_eq!(lang(&ast, &i, DeclKind::Var, "plainv"), LanguageLinkage::Cxx);
}

/// [dcl.link]/6: a redeclaration without a linkage specification takes
/// the language linkage of the first declaration.
#[test]
fn redeclaration_inherits_language_linkage() {
    let (ast, i) = parse(
        "extern \"C\" void r();\n\
         void r();\n\
         void r() {}\n\
         extern \"C\" int rv;\n\
         extern int rv;\n",
    );
    assert_eq!(all(&ast, &i, DeclKind::Function, "r").len(), 3);
    assert_eq!(lang(&ast, &i, DeclKind::Function, "r"), LanguageLinkage::C);
    assert_eq!(lang(&ast, &i, DeclKind::Var, "rv"), LanguageLinkage::C);
}

/// [dcl.link]/4: the innermost linkage specification decides; an
/// `extern "C++"` nested in an `extern "C"` gives C++ language linkage.
#[test]
fn nested_extern_cxx() {
    let (ast, i) = parse(
        "extern \"C\" {\n\
           extern \"C++\" { void cpp(); int cppv; }\n\
           extern \"C++\" void cpp2();\n\
           void c();\n\
         }\n",
    );
    assert_eq!(lang(&ast, &i, DeclKind::Function, "cpp"), LanguageLinkage::Cxx);
    assert_eq!(lang(&ast, &i, DeclKind::Var, "cppv"), LanguageLinkage::Cxx);
    assert_eq!(lang(&ast, &i, DeclKind::Function, "cpp2"), LanguageLinkage::Cxx);
    assert_eq!(lang(&ast, &i, DeclKind::Function, "c"), LanguageLinkage::C);
}

/// [dcl.link]/4, [basic.link]/6: a block-scope `extern` declaration has
/// the language linkage of the linkage specification around it; a local
/// variable has no linkage and so none.
#[test]
fn block_scope_extern() {
    let (ast, i) = parse(
        "extern \"C\" void outer() { extern void blk(); extern int blkv; int local = 0; (void)local; }\n\
         void cxx_outer() { extern void blk2(); }\n",
    );
    assert_eq!(lang(&ast, &i, DeclKind::Function, "outer"), LanguageLinkage::C);
    assert_eq!(lang(&ast, &i, DeclKind::Function, "blk"), LanguageLinkage::C);
    assert_eq!(lang(&ast, &i, DeclKind::Var, "blkv"), LanguageLinkage::C);
    assert_eq!(lang(&ast, &i, DeclKind::Function, "blk2"), LanguageLinkage::Cxx);
    assert_eq!(lang(&ast, &i, DeclKind::Var, "local"), LanguageLinkage::None);
    // The language rule reflection's `has_external_linkage` reads.
    assert_eq!(ast.linkage(one(&ast, &i, DeclKind::Function, "blk2")), Linkage::External);
    assert_eq!(ast.linkage(one(&ast, &i, DeclKind::Var, "blkv")), Linkage::External);
    assert_eq!(ast.linkage(one(&ast, &i, DeclKind::Var, "local")), Linkage::None);
}

/// The linkage of every declaration of kind `kind` named `n`, checked to
/// agree across redeclarations.
fn link(ast: &Ast, i: &Interner, kind: DeclKind, n: &str) -> Linkage {
    let ds = all(ast, i, kind, n);
    assert!(!ds.is_empty(), "no {kind:?} {n}");
    let l = ast.linkage(ds[0]);
    for &d in &ds[1..] {
        assert_eq!(ast.linkage(d), l, "{n}: redeclarations agree");
    }
    l
}

/// [basic.link]/4, /6, [dcl.meaning.general]/3.5 (WP-2623): a block-scope
/// function or `extern` declaration declares a member of the innermost
/// enclosing namespace, so it is internal in a function (a member
/// function included) of an unnamed namespace, external in a `static`
/// function outside one, and takes the linkage of an earlier declaration
/// it redeclares.
#[test]
fn block_scope_extern_in_unnamed_namespace() {
    let (ast, i) = parse(
        "namespace { void g() { extern int v; extern void h(); }\n\
           struct A { void m() { extern int mv; } }; }\n\
         static void s() { extern int sv; extern void sh(); }\n\
         static int pv; void p() { extern int pv; }\n\
         namespace N { namespace { void q() { extern int nv; } } }\n",
    );
    assert_eq!(link(&ast, &i, DeclKind::Var, "v"), Linkage::Internal);
    assert_eq!(link(&ast, &i, DeclKind::Function, "h"), Linkage::Internal);
    assert_eq!(link(&ast, &i, DeclKind::Var, "mv"), Linkage::Internal);
    assert_eq!(link(&ast, &i, DeclKind::Var, "nv"), Linkage::Internal);
    assert_eq!(link(&ast, &i, DeclKind::Var, "sv"), Linkage::External);
    assert_eq!(link(&ast, &i, DeclKind::Function, "sh"), Linkage::External);
    assert_eq!(all(&ast, &i, DeclKind::Var, "pv").len(), 2);
    assert_eq!(link(&ast, &i, DeclKind::Var, "pv"), Linkage::Internal);
    assert_eq!(lang(&ast, &i, DeclKind::Var, "v"), LanguageLinkage::None);
    assert_eq!(lang(&ast, &i, DeclKind::Var, "sv"), LanguageLinkage::Cxx);
}

/// [basic.link]/4, [dcl.link]/4 (WP-2624): a function or variable of an
/// unnamed namespace first declared in an `extern "C"` context (a
/// block-scope `extern` one in an `extern "C"` function included) has
/// external linkage and C language linkage, as in clang and as the
/// mangler says; one not in an `extern "C"` context is internal.
#[test]
fn extern_c_in_unnamed_namespace() {
    let (ast, i) = parse(
        "namespace { extern \"C\" void f(); extern \"C\" int x; void plain();\n\
           extern \"C\" void k() { extern int w; extern void m(); } }\n\
         extern \"C\" { namespace { void cf(); } }\n\
         extern \"C\" void f3(); namespace { extern \"C\" void f3(); }\n",
    );
    for (kind, n) in [
        (DeclKind::Function, "f"),
        (DeclKind::Var, "x"),
        (DeclKind::Function, "k"),
        (DeclKind::Var, "w"),
        (DeclKind::Function, "m"),
        (DeclKind::Function, "cf"),
        (DeclKind::Function, "f3"),
    ] {
        assert_eq!(link(&ast, &i, kind, n), Linkage::External, "{n}");
        assert_eq!(lang(&ast, &i, kind, n), LanguageLinkage::C, "{n}");
        assert!(all(&ast, &i, kind, n).iter().all(|&d| ast.is_extern_c(d)), "{n}");
    }
    assert_eq!(link(&ast, &i, DeclKind::Function, "plain"), Linkage::Internal);
    assert_eq!(lang(&ast, &i, DeclKind::Function, "plain"), LanguageLinkage::None);
}

/// [dcl.link]/5: a C language linkage is ignored for class members; a
/// static data member and a member function in an `extern "C"` block
/// have C++ language linkage.
#[test]
fn static_member_is_cxx() {
    let (ast, i) = parse(
        "extern \"C\" {\n\
           struct S { static int sm; static void sf(); void mf(); int field; };\n\
         }\n\
         int S::sm = 0;\n",
    );
    assert_eq!(lang(&ast, &i, DeclKind::Var, "sm"), LanguageLinkage::Cxx);
    assert_eq!(lang(&ast, &i, DeclKind::CXXMethod, "sf"), LanguageLinkage::Cxx);
    assert_eq!(lang(&ast, &i, DeclKind::CXXMethod, "mf"), LanguageLinkage::Cxx);
    // A non-static data member is no variable ([dcl.link]/1).
    assert_eq!(lang(&ast, &i, DeclKind::Field, "field"), LanguageLinkage::None);
}

/// [dcl.link]/1: only functions and variables whose names have external
/// linkage have a language linkage: types, namespaces, enumerators and
/// names with internal linkage have none.
#[test]
fn no_language_linkage() {
    let (ast, i) = parse(
        "extern \"C\" {\n\
           struct T {};\n\
           enum En { e };\n\
           static void sfn();\n\
           static int sv;\n\
         }\n\
         namespace ns {}\n\
         void use(int p) { (void)p; }\n",
    );
    assert_eq!(lang(&ast, &i, DeclKind::CXXRecord, "T"), LanguageLinkage::None);
    assert_eq!(lang(&ast, &i, DeclKind::EnumConstant, "e"), LanguageLinkage::None);
    assert_eq!(lang(&ast, &i, DeclKind::Function, "sfn"), LanguageLinkage::None);
    assert_eq!(lang(&ast, &i, DeclKind::Var, "sv"), LanguageLinkage::None);
    assert_eq!(lang(&ast, &i, DeclKind::Namespace, "ns"), LanguageLinkage::None);
    assert_eq!(lang(&ast, &i, DeclKind::ParmVar, "p"), LanguageLinkage::None);
    assert_eq!(lang(&ast, &i, DeclKind::Function, "use"), LanguageLinkage::Cxx);
}
