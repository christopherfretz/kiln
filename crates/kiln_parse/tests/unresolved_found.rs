//! The using-shadow an unresolved name's lookup found, per element of its
//! declarations (spec/ast.md §8.8; WP-3400): a tooling client reads,
//! for each declaration an `UnresolvedLookupExpr` names, the
//! using-declaration's shadow through which lookup reached it
//! ([namespace.udecl]/1, [basic.lookup.qual], [basic.lookup.unqual],
//! [temp.dep.candidate]/1, clang's `UnresolvedLookupExpr::decls()`), in
//! a template body never instantiated, in a substituted body, and in the
//! `Ast` restored from a PCH snapshot or built from a module image. Each
//! test parses a TU and reads the `Ast` alone.

use std::fmt::Write as _;
use std::sync::Arc;

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::{Ast, DeclId, ExprId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The shapes, as a module interface (`export ` where a declaration
/// needs it) and as a header (without). `dep` is the namespace that
/// declares the entities; `legacy`, `acme` and `lib::v1` (inline) name
/// them through using-declarations.
const SHAPES: &str = "\
@namespace dep {
  template <class T> T &&forward(T &t) { return static_cast<T &&>(t); }
  template <class T> int sink(T &&) { return 0; }
  int plain(int);
  int plain(long);
}
@namespace legacy { using dep::forward; using dep::plain; }
@namespace acme { using dep::forward; }
@namespace lib { inline namespace v1 { using dep::forward; } }
@template <class T> int q_legacy(T &&t) { return dep::sink(legacy::forward<T>(t)); }
@template <class T> int q_acme(T &&t) { return dep::sink(acme::forward<T>(t)); }
@template <class T> int q_inline(T &&t) { return dep::sink(lib::forward<T>(t)); }
@template <class T> int q_direct(T &&t) { return dep::sink(dep::forward<T>(t)); }
@template <class T> int q_set(T t) { return legacy::plain(t); }
@namespace user {
  using dep::plain;
  int plain(char);
  template <class T> int u_set(T t) { return plain(t); }
}
";

/// A generic lambda in a function template and a class
/// using-declaration, which a module image does not carry (the interface
/// falls back to replay), so the image test leaves them out.
const OTHER_SHAPES: &str = "\
namespace user {
  template <class T> int u_lambda(T t) {
    auto l = [](auto x) { return plain(x); };
    return l(t);
  }
}
struct B { static int m(int); };
template <class T> struct D : B {
  using B::m;
  static int m(char);
  int call(T t) { return m(t); }
};
";

const USE: &str = "int use() {\n\
  int i = 0;\n\
  return q_legacy(i) + q_acme(i) + q_inline(i) + q_direct(i) + q_set(1)\n\
    + user::u_set(1) + user::u_lambda(1) + D<int>().call(1);\n\
}\n";

fn shapes(export: bool) -> String {
    SHAPES.replace('@', if export { "export " } else { "" })
}

/// Parses `src` (a module unit when `record`) with modules on; `files`
/// are the module files it may import. Returns the parsed unit, the
/// module capsule, and the image imports' replay reasons.
fn parse(
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

fn exprs(ast: &Ast) -> impl Iterator<Item = ExprId> + '_ {
    (1..=ast.exprs.len() as u32).map(ExprId::new)
}

/// `d`'s name qualified by its enclosing namespaces and classes
/// (`dep::forward`, `lib::v1::forward`, `D::m`).
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

/// Who declares `d`: its context's qualified name, with the class's
/// template arguments when it is a specialization (`D<int>`).
fn owner(ast: &Ast, d: DeclId) -> String {
    let c = ast.decls.get(d).ctx().expect("a context");
    let mut s = qualified(ast, c);
    if ast.spec_info(c).is_some() {
        s.push_str("<spec>");
    }
    s
}

/// What a client reads from every unresolved name `name` of `ast`: per
/// node (sorted), its elements as `target via shadow-owner` or
/// `target direct`, the shadow checked to be a `UsingShadow` of the
/// target. Also checks that [`Ast::unresolved_shadow`] and
/// [`Ast::unresolved_found_pairs`] agree.
fn report(ast: &Ast, name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in exprs(ast) {
        if ast.exprs.get(e).kind() != ExprKind::UnresolvedLookupExpr {
            continue;
        }
        let n = ast.unresolved_name(e);
        if n.as_ident().is_none_or(|s| ast.symbols().as_str(s) != name) {
            continue;
        }
        let pairs: Vec<(DeclId, Option<DeclId>)> = ast.unresolved_found_pairs(e).collect();
        assert_eq!(pairs.iter().map(|p| p.0).collect::<Vec<_>>(), ast.unresolved_decls(e));
        let mut line = String::new();
        for (i, &(d, s)) in pairs.iter().enumerate() {
            assert_eq!(ast.unresolved_shadow(e, i), s);
            let _ = write!(line, "{} ", qualified(ast, d));
            match s {
                Some(s) => {
                    assert_eq!(ast.decls.get(s).kind(), DeclKind::UsingShadow);
                    assert_eq!(ast.first(ast.strip_shadow(s)), ast.first(d));
                    let _ = write!(line, "via {}; ", owner(ast, s));
                }
                None => line.push_str("direct; "),
            }
        }
        assert_eq!(ast.unresolved_shadow(e, pairs.len()), None, "past the end");
        out.push(line);
    }
    out.sort();
    out.dedup();
    out
}

/// Everything a client reads from the shapes, by name.
fn full_report(ast: &Ast) -> Vec<(String, Vec<String>)> {
    ["forward", "plain", "m"].iter().map(|&n| (n.to_owned(), report(ast, n))).collect()
}

fn source_ast() -> Ast {
    parse(&format!("{}{OTHER_SHAPES}{USE}", shapes(false)), false, Vec::new()).0.ast
}

/// [namespace.udecl]/1, [basic.lookup.qual]/1: `legacy::forward<T>`
/// names `dep::forward` through `legacy`'s using-declaration, and
/// `acme::forward<T>` the same entity through `acme`'s, two distinct
/// answers; `lib::forward<T>` reaches the shadow in the inline namespace
/// `lib::v1` ([namespace.def]/7); `dep::forward<T>` finds the template
/// itself.
#[test]
fn qualified_template_ids_keep_the_shadow_found() {
    let ast = source_ast();
    let fwd = report(&ast, "forward");
    assert_eq!(
        fwd,
        [
            "dep::forward direct; ",
            "dep::forward via acme; ",
            "dep::forward via legacy; ",
            "dep::forward via lib::v1; ",
        ],
        "{fwd:?}"
    );
    // The two shadows of one entity are distinct declarations.
    let shadows: Vec<DeclId> = exprs(&ast)
        .filter(|&e| ast.exprs.get(e).kind() == ExprKind::UnresolvedLookupExpr)
        .filter_map(|e| ast.unresolved_shadow(e, 0))
        .filter(|&s| qualified(&ast, ast.strip_shadow(s)) == "dep::forward")
        .collect();
    let mut owners: Vec<String> = shadows.iter().map(|&s| owner(&ast, s)).collect();
    owners.sort();
    owners.dedup();
    assert_eq!(owners, ["acme", "legacy", "lib::v1"]);
}

/// [temp.dep.candidate]/1, [basic.lookup.unqual]: the ordinary-lookup
/// candidates of an unqualified dependent call (`plain(t)` in `user`)
/// keep, element by element, the shadow of `user`'s using-declaration
/// for `dep::plain(int)` and `dep::plain(long)`, and none for
/// `user::plain(char)` declared there; a qualified overload set
/// (`legacy::plain(t)`) keeps both shadows. The order of the
/// declarations is the node's own (unqualified: newest first).
#[test]
fn dependent_call_candidates_keep_the_shadow_found() {
    let ast = source_ast();
    let plain = report(&ast, "plain");
    assert!(
        plain.contains(&"dep::plain via legacy; dep::plain via legacy; ".to_owned()),
        "{plain:?}"
    );
    assert!(
        plain
            .contains(&"user::plain direct; dep::plain via user; dep::plain via user; ".to_owned()),
        "{plain:?}"
    );
}

/// [temp.inst]/3, [expr.prim.lambda.closure]: a generic lambda's body in
/// `u_lambda<int>` is substituted and stays dependent; its rebuilt
/// `plain(x)` keeps the shadows of the pattern's node (one line for both
/// nodes, so more than one node reports it).
#[test]
fn instantiation_keeps_the_shadow_found() {
    let ast = source_ast();
    let lambda_nodes: Vec<ExprId> = exprs(&ast)
        .filter(|&e| ast.exprs.get(e).kind() == ExprKind::UnresolvedLookupExpr)
        .filter(|&e| {
            ast.unresolved_name(e).as_ident().is_some_and(|s| ast.symbols().as_str(s) == "plain")
        })
        .filter(|&e| ast.unresolved_found_pairs(e).filter(|p| p.1.is_some()).count() == 2)
        .filter(|&e| ast.unresolved_decls(e).len() == 3)
        .collect();
    assert!(lambda_nodes.len() >= 3, "u_set, u_lambda's pattern and its substitution");
}

/// [namespace.udecl]/1, [class.member.lookup]: `m(t)` in the class
/// template `D` finds `D::m(char)` and, through `D`'s using-declaration,
/// `B::m`; the node keeps the shadow, a member of `D`, and its
/// instantiation the shadow in `D<int>`.
#[test]
fn a_class_using_declaration_is_kept_for_a_dependent_member_call() {
    let ast = source_ast();
    let m = report(&ast, "m");
    assert!(m.iter().any(|l| l.contains("B::m via D; ")), "{m:?}");
    // [temp.inst]/3: the call rebuilt in `D<int>::call` names the shadow
    // of `D<int>`'s instantiated using-declaration.
    assert!(m.iter().any(|l| l.contains("B::m via D<spec>; ")), "{m:?}");
    assert!(m.iter().all(|l| !l.contains("D::m via")), "D::m(char) is found directly: {m:?}");
}

/// The accessors are total: a node that is not an unresolved name, and a
/// node with no shadow, answer `None` and an empty or shadow-free list.
#[test]
fn the_accessors_are_total() {
    let ast = source_ast();
    for e in exprs(&ast) {
        let ule = ast.exprs.get(e).kind() == ExprKind::UnresolvedLookupExpr;
        if !ule {
            assert_eq!(ast.unresolved_shadow(e, 0), None);
            assert_eq!(ast.unresolved_found_pairs(e).count(), 0);
        } else if ast.templates.unresolved_found.get(e).is_none() {
            assert!(ast.unresolved_found_pairs(e).all(|p| p.1.is_none()));
        }
    }
}

/// spec/pch.md: a PCH snapshot carries the record (the `Ast` restored
/// from its snapshot encoding answers as the parsed one).
#[test]
fn the_shadow_found_survives_the_pch_snapshot() {
    let ast = source_ast();
    let want = full_report(&ast);
    let bytes = kiln_snap::to_bytes(&ast);
    let restored = kiln_snap::from_bytes::<Ast>(&bytes).expect("the snapshot decodes");
    assert_eq!(full_report(&restored), want);
}

/// spec/modules.md §11.11: the importer of a module built from its
/// declaration image reads the shadows the interface's parse recorded
/// on the template patterns' unresolved names ([module.interface]).
#[test]
fn the_shadow_found_survives_the_module_image() {
    let iface = format!("export module m;\n{}", shapes(true));
    let (src, _, _) = parse(&iface, false, Vec::new());
    let (tu, capsule, _) = parse(&iface, true, Vec::new());
    let image = tu.image.expect("an image");
    assert_eq!(image.fallback, None, "the interface is carried by its image");
    let mut capsule = capsule.expect("a module interface");
    capsule
        .sections
        .push(kiln_pp::bmi::Section { tag: kiln_pp::bmi::SECTION_IMAGE, bytes: image.bytes });
    let dir = std::env::temp_dir().join(format!("kiln-unresolved-found-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pcm = dir.join("m.pcm");
    std::fs::write(&pcm, kiln_pp::bmi::write(&capsule)).unwrap();
    let user = "import m;\nint use() { return 0; }\n";
    let (got, _, imports) = parse(user, false, vec![("m".to_owned(), pcm)]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(imports, [None], "built from the image");
    // The patterns' nodes: the interface's own report, without any use.
    let want: Vec<(String, Vec<String>)> = full_report(&src.ast);
    assert_eq!(want[0].1.len(), 4, "the four `forward` shapes: {want:?}");
    assert_eq!(want[1].1.len(), 2, "the two `plain` sets: {want:?}");
    assert_eq!(full_report(&got.ast), want);
}
