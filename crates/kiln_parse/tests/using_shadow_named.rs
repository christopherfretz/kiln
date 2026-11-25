//! The shadow a using-declaration named, when what its lookup found was
//! itself a using-shadow (`Ast::shadow_named`, spec/ast.md §4.2,
//! WP-3401; [namespace.udecl]/1, /2, [class.member.lookup]). In
//! `namespace acme { using lib::string; } using acme::string;` the second
//! shadow's target is `lib::string` (a shadow's target is never a
//! shadow), and its record names `acme`'s shadow: the first link of the
//! chain a tooling client walks to rename through using-declarations.
//! Each test parses a TU, drops sema, and reads the `Ast` alone.

use std::sync::Arc;

use kiln_ast::decl::DeclKind;
use kiln_ast::diag_names::DiagNames;
use kiln_ast::{Ast, DeclId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_file};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The `Ast` of `src`, which must compile cleanly.
fn parse(src: &str) -> Ast {
    let interner = Arc::new(Interner::new());
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::X86_64);
    let ast = parse_file(&sm, &interner, file, &engine, target, &ParseOptions::default());
    let diags: Vec<String> = sink.take().iter().map(|d| d.message.clone()).collect();
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    ast
}

/// `d`'s name qualified by its enclosing namespaces, classes and
/// functions (`acme::string`, `D::f`).
fn qualified(ast: &Ast, d: DeclId) -> String {
    let mut parts = Vec::new();
    let mut at = Some(d);
    while let Some(x) = at {
        let n = ast.decls.get(x);
        if n.kind() == DeclKind::TranslationUnit {
            break;
        }
        if let Some(s) = n.name().as_ident() {
            parts.push(ast.symbols().as_str(s).to_owned());
        }
        at = n.ctx();
    }
    parts.reverse();
    parts.join("::")
}

/// Where `d` is declared: `(tu)` for the translation unit, a class
/// specialization by its type (`D<Base>`), else the qualified name (a
/// function template's specializations print as the template, `h`).
fn owner(ast: &Ast, d: DeclId) -> String {
    let c = ast.decls.get(d).ctx().expect("a context");
    if ast.decls.get(c).kind() == DeclKind::TranslationUnit {
        return "(tu)".to_owned();
    }
    if ast.decls.get(c).kind() == DeclKind::CXXRecord
        && ast.spec_info(c).is_some()
        && let Some(t) = ast.decls.get(c).ty()
    {
        return DiagNames::new(ast, None, 22).type_arg(t, None).trim_matches('\'').to_owned();
    }
    qualified(ast, c)
}

/// A shadow's target: its qualified name, a function's type after it.
fn target(ast: &Ast, d: DeclId) -> String {
    let mut s = qualified(ast, d);
    if ast.decls.get(d).kind().is_function()
        && let Some(t) = ast.decls.get(d).ty()
    {
        s.push(' ');
        s.push_str(&DiagNames::new(ast, None, 22).type_arg(t, None));
    }
    s
}

/// One line per using-shadow of `ast`, in declaration order:
/// `owner: target <- named-owner` (`-` when the shadow's lookup found its
/// target directly). Also checks the record's invariants: the named
/// declaration is a shadow of the same target, never the shadow itself.
fn report(ast: &Ast) -> Vec<String> {
    let mut out = Vec::new();
    for s in ast.decls.ids() {
        let k = ast.decls.get(s).kind();
        if !matches!(k, DeclKind::UsingShadow | DeclKind::ConstructorUsingShadow) {
            continue;
        }
        let t = ast.strip_shadow(s);
        let named = match ast.shadow_named(s) {
            Some(n) => {
                assert_ne!(n, s, "a shadow names itself");
                assert_eq!(ast.decls.get(n).kind(), k, "a shadow names a shadow of its kind");
                assert_eq!(
                    ast.first(ast.strip_shadow(n)),
                    ast.first(t),
                    "the named shadow is of the same entity"
                );
                owner(ast, n)
            }
            None => "-".to_owned(),
        };
        out.push(format!("{}: {} <- {named}", owner(ast, s), target(ast, t)));
    }
    out
}

/// The chain a client walks from shadow `s`: each link's owner, first
/// link first, until a shadow whose lookup found its target directly.
fn chain(ast: &Ast, s: DeclId) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = ast.shadow_named(s);
    while let Some(n) = at {
        out.push(owner(ast, n));
        at = ast.shadow_named(n);
        assert!(out.len() < 16, "a chain of shadows ends");
    }
    out
}

/// The shadows (of any kind) declared directly in context `c`.
fn shadows_in(ast: &Ast, c: DeclId) -> Vec<DeclId> {
    ast.decls
        .ids()
        .filter(|&s| {
            ast.decls.get(s).ctx() == Some(c)
                && matches!(
                    ast.decls.get(s).kind(),
                    DeclKind::UsingShadow | DeclKind::ConstructorUsingShadow
                )
        })
        .collect()
}

/// The declaration of kind `kind` whose qualified name is `q` (one only).
fn decl(ast: &Ast, q: &str, kind: DeclKind) -> DeclId {
    let v: Vec<DeclId> = ast
        .decls
        .ids()
        .filter(|&d| ast.decls.get(d).kind() == kind && qualified(ast, d) == q)
        .collect();
    assert!(!v.is_empty(), "{q}: none");
    v[0]
}

const NAMESPACES: &str = "namespace lib { struct string {}; void f(int); void f(double); }\n\
     namespace acme { using lib::string; using lib::f; }\n\
     namespace acme2 { using acme::string; }\n\
     using acme2::string;\n\
     namespace mix { using lib::f; void f(char); }\n\
     namespace both { using acme::f; using mix::f; }\n\
     namespace d { using namespace lib; }\n\
     namespace direct { using d::string; using lib::f; }\n";

/// [namespace.udecl]/1: at namespace scope, each shadow of a
/// using-declarator whose lookup found a shadow names it; one found
/// directly, or through a using-directive (not a using-declaration),
/// names none.
#[test]
fn namespace_scope_names_the_shadow_found() {
    let ast = parse(NAMESPACES);
    let r = report(&ast);
    let want = [
        "acme: lib::string <- -",
        "acme: lib::f 'void (int)' <- -",
        "acme: lib::f 'void (double)' <- -",
        "acme2: lib::string <- acme",
        "(tu): lib::string <- acme2",
    ];
    for w in want {
        assert!(r.iter().any(|l| l == w), "{w}: {r:#?}");
    }
    assert!(r.contains(&"direct: lib::string <- -".to_owned()), "{r:#?}");
    // The chain from the global shadow: acme2's, then acme's, then the end.
    let tu = ast.tu;
    let global = shadows_in(&ast, tu);
    assert_eq!(global.len(), 1);
    assert_eq!(chain(&ast, global[0]), ["acme2", "acme"]);
}

/// An overload set: one record per shadow, each naming the shadow its
/// own element was found through; an element declared in the scope
/// searched names none.
#[test]
fn overload_set_one_record_per_shadow() {
    let ast = parse(NAMESPACES);
    let r = report(&ast);
    for w in [
        "mix: lib::f 'void (int)' <- -",
        "mix: lib::f 'void (double)' <- -",
        "direct: lib::f 'void (int)' <- -",
        "both: lib::f 'void (int)' <- acme",
        "both: lib::f 'void (double)' <- acme",
    ] {
        assert!(r.iter().any(|l| l == w), "{w}: {r:#?}");
    }
    // `using mix::f;` in `both` repeats `lib::f(int)` and `lib::f(double)`
    // (each a new shadow, naming `mix`'s) and adds `mix::f(char)` directly.
    let both = decl(&ast, "both", DeclKind::Namespace);
    let usings: Vec<DeclId> =
        ast.members(both).filter(|&m| ast.decls.get(m).kind() == DeclKind::Using).collect();
    assert_eq!(usings.len(), 2);
    let second: Vec<String> = ast
        .using_shadows(usings[1])
        .map(|s| {
            format!(
                "{} <- {}",
                target(&ast, ast.strip_shadow(s)),
                ast.shadow_named(s).map_or("-".to_owned(), |n| owner(&ast, n))
            )
        })
        .collect();
    let mut sorted = second.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        [
            "lib::f 'void (double)' <- mix",
            "lib::f 'void (int)' <- mix",
            "mix::f 'void (char)' <- -",
        ],
        "{second:?}"
    );
    // Zero cost for the rest: only the shadows that found a shadow have a
    // row (acme2's, the global one, both's four).
    assert_eq!(ast.decls.cold.shadow_named.len(), 6);
}

/// A function-local using-declaration ([namespace.udecl]/2, block scope)
/// names the shadow its lookup found, as at namespace scope.
#[test]
fn block_scope_names_the_shadow_found() {
    let ast = parse(&format!(
        "{NAMESPACES}void g() {{ using acme::string; using acme::f; string s; f(1); }}\n"
    ));
    let r: Vec<String> = report(&ast).into_iter().filter(|l| l.starts_with("g: ")).collect();
    assert_eq!(
        r,
        [
            "g: lib::string <- acme",
            "g: lib::f 'void (int)' <- acme",
            "g: lib::f 'void (double)' <- acme",
        ]
    );
}

const CLASSES: &str = "struct Mid { void f(); void f(int); };\n\
     struct Base : Mid { using Mid::f; void f(char); };\n\
     struct D : Base { using Base::f; };\n\
     struct E : D { using D::f; };\n\
     struct H1 : Base { void f(); using Base::f; };\n\
     struct H2 : Base { using Base::f; void f(); };\n";

/// [namespace.udecl]/1, [class.member.lookup]: `using Base::f;` where
/// `Base` got `f` from its own `using Mid::f;` names `Base`'s shadows; a
/// member `Base` declares itself is found directly. A shadow a member of
/// the class hides ([namespace.udecl]/14) keeps its record, in either
/// declaration order.
#[test]
fn class_scope_names_the_base_shadow() {
    let ast = parse(CLASSES);
    let r = report(&ast);
    for w in [
        "Base: Mid::f 'void ()' <- -",
        "Base: Mid::f 'void (int)' <- -",
        "D: Base::f 'void (char)' <- -",
        "D: Mid::f 'void ()' <- Base",
        "D: Mid::f 'void (int)' <- Base",
        "E: Base::f 'void (char)' <- D",
        "E: Mid::f 'void ()' <- D",
        "E: Mid::f 'void (int)' <- D",
    ] {
        assert!(r.iter().any(|l| l == w), "{w}: {r:#?}");
    }
    let e = decl(&ast, "E", DeclKind::CXXRecord);
    let e = ast.definition(e).expect("defined");
    for s in shadows_in(&ast, e) {
        let want: &[&str] = if ast.decls.get(ast.strip_shadow(s)).ctx() == ast.decls.get(e).ctx() {
            &[]
        } else if qualified(&ast, ast.strip_shadow(s)) == "Base::f" {
            &["D"]
        } else {
            &["D", "Base"]
        };
        assert_eq!(chain(&ast, s), want, "{}", target(&ast, ast.strip_shadow(s)));
    }
    // Both orders: `Mid::f()` hidden by `H1::f()` / `H2::f()`, named the same.
    for h in ["H1", "H2"] {
        let c = ast.definition(decl(&ast, h, DeclKind::CXXRecord)).expect("defined");
        let mut rows: Vec<(String, bool, String)> = shadows_in(&ast, c)
            .into_iter()
            .map(|s| {
                (
                    target(&ast, ast.strip_shadow(s)),
                    ast.shadow_hider(s).is_some(),
                    ast.shadow_named(s).map_or("-".to_owned(), |n| owner(&ast, n)),
                )
            })
            .collect();
        rows.sort();
        assert_eq!(
            rows,
            [
                ("Base::f 'void (char)'".to_owned(), false, "-".to_owned()),
                ("Mid::f 'void ()'".to_owned(), true, "Base".to_owned()),
                ("Mid::f 'void (int)'".to_owned(), false, "Base".to_owned()),
            ],
            "{h}"
        );
    }
}

/// WP-3403's Q3: a using-declarator repeated through another base makes
/// its own shadow, which replaces the earlier one in lookup. Its record names
/// the shadow its own lookup found, which is not the chain link: through
/// `B1` (which has `using A::f;`) `B1`'s shadow, through `B2` none, in
/// either order. At namespace scope a repeated using-declaration names
/// what its lookup found each time.
#[test]
fn repeated_using_declarators_name_what_lookup_found() {
    for (src, first, second) in [
        ("struct D : B1, B2 { using B1::f; using B2::f; };", Some("B1"), None),
        ("struct D : B1, B2 { using B2::f; using B1::f; };", None, Some("B1")),
    ] {
        let ast = parse(&format!(
            "struct A {{ void f(); }};\n\
             struct B1 : virtual A {{ using A::f; }};\n\
             struct B2 : virtual A {{}};\n\
             {src}\n"
        ));
        let d = ast.definition(decl(&ast, "D", DeclKind::CXXRecord)).expect("defined");
        let usings: Vec<DeclId> =
            ast.members(d).filter(|&m| ast.decls.get(m).kind() == DeclKind::Using).collect();
        let s: Vec<DeclId> = usings
            .iter()
            .map(|&u| {
                let v: Vec<DeclId> = ast.using_shadows(u).collect();
                assert_eq!(v.len(), 1, "{src}");
                v[0]
            })
            .collect();
        let named = |x: DeclId| ast.shadow_named(x).map(|n| owner(&ast, n));
        assert_eq!(named(s[0]).as_deref(), first, "{src}");
        assert_eq!(named(s[1]).as_deref(), second, "{src}");
        assert_ne!(ast.shadow_named(s[1]), Some(s[0]), "not the earlier shadow: {src}");
    }
    let ast = parse(
        "namespace lib { struct string {}; }\n\
         namespace acme { using lib::string; }\n\
         using acme::string;\n\
         using acme::string;\n",
    );
    let global = shadows_in(&ast, ast.tu);
    assert_eq!(global.len(), 2);
    let acme = shadows_in(&ast, decl(&ast, "acme", DeclKind::Namespace));
    assert_eq!(
        global.iter().map(|&s| ast.shadow_named(s)).collect::<Vec<_>>(),
        [Some(acme[0]), Some(acme[0])]
    );
}

/// [enum.udecl]: `using enum E;` names the enumeration, whose enumerators
/// it introduces without a lookup of their names: its shadows have no
/// record, also when the enumeration itself was found through a
/// using-declaration. A using-declarator naming an enumerator through a
/// shadow of it has one.
#[test]
fn using_enum_shadows_name_nothing() {
    let ast = parse(
        "namespace lib { enum class E { a, b }; }\n\
         namespace acme { using lib::E; }\n\
         struct S { using enum acme::E; };\n\
         namespace n { using enum lib::E; }\n\
         namespace m { using n::a; }\n",
    );
    let r = report(&ast);
    for w in [
        "S: lib::E::a <- -",
        "S: lib::E::b <- -",
        "n: lib::E::a <- -",
        "n: lib::E::b <- -",
        "acme: lib::E <- -",
        "m: lib::E::a <- n",
    ] {
        assert!(r.iter().any(|l| l == w), "{w}: {r:#?}");
    }
}

/// [namespace.udecl]/3, [class.inhctor.init]: `using B::B;` where `B`
/// inherited `A(int)` through `using A::A;` makes a
/// `ConstructorUsingShadow` of `A(int)` that names `B`'s (the nominated
/// base's shadow, `DeclArena::ctor_shadow_nominated`); one of a
/// constructor `B` declares itself names none.
#[test]
fn inheriting_constructor_names_the_nominated_shadow() {
    let ast = parse(
        "struct A { A(int); };\n\
         struct B : A { using A::A; B(char); };\n\
         struct C : B { using B::B; };\n",
    );
    let c = ast.definition(decl(&ast, "C", DeclKind::CXXRecord)).expect("defined");
    let b = ast.definition(decl(&ast, "B", DeclKind::CXXRecord)).expect("defined");
    let mut rows: Vec<String> = shadows_in(&ast, c)
        .into_iter()
        .filter(|&s| !ast.decls.get(ast.strip_shadow(s)).has(kiln_ast::decl::DeclFlags::IMPLICIT))
        .map(|s| {
            let n = ast.shadow_named(s);
            assert_eq!(n, ast.decls.ctor_shadow_nominated(s));
            if let Some(n) = n {
                assert_eq!(ast.decls.get(n).ctx(), Some(b), "B's shadow");
            }
            format!(
                "{} <- {}",
                target(&ast, ast.strip_shadow(s)),
                n.map_or("-".to_owned(), |n| owner(&ast, n))
            )
        })
        .collect();
    rows.sort();
    // A constructor's name is no identifier: `qualified` prints its class.
    assert_eq!(rows, ["A 'void (int)' <- B", "B 'void (char)' <- -"]);
}

/// [temp.inst]/3, [temp.dep.type]: a dependent `using T::f;` in a class
/// template makes no shadow in the pattern; each specialization's
/// using-declaration has its own records, by what its lookup found. A
/// non-dependent one is redone in each specialization (in reverse
/// overload order, `Sema::reverse_using_shadows`), each shadow keeping
/// the record of its own element.
#[test]
fn class_template_specializations_record_their_own() {
    let ast = parse(&format!(
        "{CLASSES}\
         template <class T> struct G : T {{ using T::f; }};\n\
         template <class T> struct N : Base {{ using Base::f; T t; }};\n\
         G<Base> g1; G<Mid> g2; N<int> n1;\n"
    ));
    let r = report(&ast);
    for w in [
        "G<Base>: Base::f 'void (char)' <- -",
        "G<Base>: Mid::f 'void ()' <- Base",
        "G<Base>: Mid::f 'void (int)' <- Base",
        "G<Mid>: Mid::f 'void ()' <- -",
        "G<Mid>: Mid::f 'void (int)' <- -",
        "N: Base::f 'void (char)' <- -",
        "N: Mid::f 'void ()' <- Base",
        "N: Mid::f 'void (int)' <- Base",
        "N<int>: Base::f 'void (char)' <- -",
        "N<int>: Mid::f 'void ()' <- Base",
        "N<int>: Mid::f 'void (int)' <- Base",
    ] {
        assert!(r.iter().any(|l| l == w), "{w}: {r:#?}");
    }
    assert!(!r.iter().any(|l| l.starts_with("G: ")), "the pattern has no shadows: {r:#?}");
}

/// A function template's block-scope using-declarations are redone in
/// each specialization ([temp.inst]): a dependent `using T::e;` naming
/// an enumerator a class got through its own using-declaration names
/// that class's shadow; a non-dependent one names what it named in the
/// pattern.
#[test]
fn function_template_specializations_record_their_own() {
    let ast = parse(
        "namespace lib { struct string {}; }\n\
         namespace acme { using lib::string; }\n\
         struct X { enum K { e }; };\n\
         struct Y : X { using X::e; };\n\
         template <class T> int h() { using T::e; using acme::string; return e; }\n\
         int u = h<Y>() + h<X>();\n",
    );
    let mut r: Vec<String> = report(&ast).into_iter().filter(|l| l.starts_with("h: ")).collect();
    r.sort();
    // The pattern's `using acme::string;` and each specialization's two.
    assert_eq!(
        r,
        [
            "h: X::K::e <- -",
            "h: X::K::e <- Y",
            "h: lib::string <- acme",
            "h: lib::string <- acme",
            "h: lib::string <- acme",
        ]
    );
}

const IMAGE_SRC: &str = "namespace lib { export struct string {}; export void f(int); export void f(double); }\n\
     export namespace acme { using lib::string; using lib::f; }\n\
     export namespace acme2 { using acme::string; using acme::f; }\n\
     export struct Mid { void f(); void f(int); };\n\
     export struct Base : Mid { using Mid::f; void f(char); };\n\
     export struct D : Base { using Base::f; };\n";

/// spec/pch.md: a PCH snapshot carries the records.
#[test]
fn records_survive_the_pch_snapshot() {
    let ast = parse(&format!(
        "{NAMESPACES}{CLASSES}\
         struct A {{ A(int); }}; struct B : A {{ using A::A; }}; struct C : B {{ using B::B; }};\n"
    ));
    let want = report(&ast);
    assert!(want.iter().filter(|l| !l.ends_with("<- -")).count() >= 10, "{want:#?}");
    let bytes = kiln_snap::to_bytes(&ast);
    let restored = kiln_snap::from_bytes::<Ast>(&bytes).expect("the snapshot decodes");
    assert_eq!(report(&restored), want);
}

/// Parses `src` with modules on (a module unit recording its image when
/// `record`); `files` are the module files it may import. Returns the
/// parsed unit, the module capsule, and the image imports' replay
/// reasons.
fn parse_module(
    src: &str,
    record: bool,
    files: Vec<(String, std::path::PathBuf)>,
) -> (kiln_parse::ParsedTu, Option<kiln_pp::bmi::Capsule>, Vec<Option<String>>) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cppm", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts =
        ParseOptions { std: kiln_lex::Std::Cxx26, record_image: record, ..ParseOptions::default() };
    let mut ppo = kiln_pp::PpOptions::new(opts.std);
    ppo.parser_pragmas = true;
    ppo.modules = Some(kiln_pp::ModuleOptions {
        module_files: files,
        record,
        image: true,
        ..kiln_pp::ModuleOptions::default()
    });
    let mut pp = kiln_pp::Preprocessor::new(&sm, &engine, &interner, ppo);
    pp.enter_main_file(file);
    let target = Target::for_arch(Arch::X86_64);
    let (tu, mut pp) = kiln_parse::parse_preprocessed(
        pp,
        Arc::clone(&interner),
        &engine,
        target,
        &opts,
        ConstexprLimits::default(),
    );
    let diags: Vec<String> = sink.take().into_iter().map(|d| d.message).collect();
    assert!(diags.is_empty(), "{diags:?}");
    let imports = pp.image_imports().iter().map(|i| i.replayed.clone()).collect();
    (tu, pp.take_capsule(), imports)
}

/// spec/modules.md §11.11 kind 21: the importer of a module built from
/// its declaration image has the records the interface's parse made
/// ([module.interface]), at namespace and class scope.
#[test]
fn records_survive_the_module_image() {
    let iface = format!("export module m;\n{IMAGE_SRC}");
    let (src, _, _) = parse_module(&iface, false, Vec::new());
    let (tu, capsule, _) = parse_module(&iface, true, Vec::new());
    let image = tu.image.expect("an image");
    assert_eq!(image.fallback, None, "the interface is carried by its image");
    let mut capsule = capsule.expect("a module interface");
    capsule
        .sections
        .push(kiln_pp::bmi::Section { tag: kiln_pp::bmi::SECTION_IMAGE, bytes: image.bytes });
    let dir = std::env::temp_dir().join(format!("kiln-shadow-named-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pcm = dir.join("m.pcm");
    std::fs::write(&pcm, kiln_pp::bmi::write(&capsule)).unwrap();
    let user = "import m;\nint use() { return 0; }\n";
    let (got, _, imports) = parse_module(user, false, vec![("m".to_owned(), pcm)]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(imports, [None], "built from the image");
    let mut want = report(&src.ast);
    want.sort();
    assert_eq!(want.iter().filter(|l| !l.ends_with("<- -")).count(), 5, "{want:#?}");
    let mut have = report(&got.ast);
    have.sort();
    assert_eq!(have, want);
}

/// The accessor is total: `None` for every declaration that is not a
/// shadow, and for a shadow whose lookup found its target directly.
#[test]
fn the_accessor_is_total() {
    let ast = parse(&format!("{NAMESPACES}{CLASSES}"));
    let mut some = 0;
    for d in (1..=ast.decls.len() as u32).filter_map(DeclId::try_new) {
        let k = ast.decls.get(d).kind();
        if ast.shadow_named(d).is_some() {
            assert!(matches!(k, DeclKind::UsingShadow | DeclKind::ConstructorUsingShadow));
            some += 1;
        }
    }
    assert!(some > 0);
}
