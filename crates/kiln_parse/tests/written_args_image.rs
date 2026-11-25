//! Arguments and bounds as written across a module image and a PCH
//! snapshot (spec/ast.md §8.3 "Arguments as written", spec/types.md §7.2
//! "Written bounds", spec/modules.md §11.11 "Arguments as written";
//! WP-3100, WP-3101, WP-3102): a tooling client reads the same written
//! template arguments (a non-type argument the expression written, never
//! its value) and the same written array bounds, whether the
//! declarations came from source, an imported module's declaration image
//! or a precompiled header ([module.interface], [temp.spec.partial],
//! [temp.expl.spec], [dcl.array]/1, [dcl.fct]/5). A deduced variable's
//! placeholder kind is carried too (WP-3583, [dcl.type.auto.deduct]).

use std::fmt::Write as _;
use std::sync::Arc;

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ArgView, PrintOptions, print_type};
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The shapes, as a module interface (`export ` where a declaration
/// needs it) and as a header (without).
const SHAPES: &str = "\
@constexpr int f(int x) { return x; }
@int arr[f(2)] = {1, 2};
@int by_ref(int (&a)[f(2)]) { return a[1]; }
@int par(int a[f(3)]) { return a[2]; }
@int lit(int b[4]) { return b[0]; }
@template <class T> struct TA { T a[f(5)]; };
@template <class T, int N> struct P { static constexpr int k = 0; };
template <class T> struct P<T, f(1)> { static constexpr int k = 1; };
template <> struct P<int, f(2)> { static constexpr int k = 2; };
template <> struct P<char, 3> { static constexpr int k = 3; };
@template <class T, int N> constexpr int v = 0;
template <class T> constexpr int v<T, f(3)> = 3;
template <> constexpr int v<int, f(4)> = 4;
@template <int N> int fh() { return 0; }
template <> int fh<f(6)>() { return 6; }
@template <class T> struct O {
  template <class U, int N> struct In { static constexpr int k = 0; };
  template <class U> struct In<U, f(7)> { static constexpr int k = 7; };
};
@inline long dv(long w) { decltype(auto) dv2 = w; return dv2; }
";

const USE: &str = "int use() {\n\
  TA<int> ta{};\n\
  int three[3] = {1, 2, 3};\n\
  return by_ref(arr) + par(three) + lit(three) + P<int, 2>::k + P<char, 1>::k + P<char, 3>::k\n\
    + v<int, 4> + v<char, 3> + fh<6>() + O<long>::In<char, 7>::k + int(sizeof(ta.a))\n\
    + int(dv(1));\n\
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

/// The declarations named `name` of kind `kind`, in order.
fn named(ast: &Ast, name: &str, kind: DeclKind) -> Vec<DeclId> {
    (1..=ast.decls.len() as u32)
        .map(DeclId::new)
        .filter(|&d| {
            let n = ast.decls.get(d);
            n.kind() == kind && n.name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == name)
        })
        .collect()
}

fn ty(ast: &Ast, q: QualType) -> String {
    print_type(&ast.types, ast, q, PrintOptions::SUGARED)
}

/// Expression `e` in pre-order: each node's kind, a literal's value, a
/// reference's name.
fn expr(ast: &Ast, e: ExprId, out: &mut String) {
    let x = ast.exprs.get(e);
    let _ = write!(out, "{:?}", x.kind());
    match x.kind() {
        ExprKind::IntegerLiteral => {
            let _ = write!(out, "={}", ast.exprs.int_value(e).unwrap_or(u64::MAX));
        }
        ExprKind::DeclRefExpr => {
            let d = ast.exprs.decl(e);
            if let Some(s) = ast.decls.get(d).name().as_ident() {
                let _ = write!(out, "={}", ast.symbols().as_str(s));
            }
        }
        _ => {}
    }
    let kids = ast.exprs.children(e, &ast.lists);
    if !kids.is_empty() {
        out.push('(');
        for (k, c) in kids.into_iter().enumerate() {
            if k > 0 {
                out.push(' ');
            }
            expr(ast, c, out);
        }
        out.push(')');
    }
}

/// `d`'s arguments as written, or "none".
fn written_args(ast: &Ast, d: DeclId) -> String {
    let Some(w) = ast.templates.args_as_written(d) else { return "none".to_owned() };
    let mut out = String::new();
    for &a in ast.types.args(w) {
        match a.view() {
            ArgView::Type(q) => {
                let _ = write!(out, "type {}", ty(ast, q));
            }
            ArgView::Expr { expr: e, .. } => {
                out.push_str("expr ");
                expr(ast, e, &mut out);
            }
            v => {
                let _ = write!(out, "{v:?}");
            }
        }
        out.push_str("; ");
    }
    out
}

/// The expressions written in `d`'s type.
fn written_bounds(ast: &Ast, d: DeclId) -> String {
    let mut roots = Vec::new();
    ast.types.written_exprs(ast.decls.get(d).ty().expect("a type"), &mut roots);
    let mut out = String::new();
    for e in roots {
        expr(ast, e, &mut out);
        out.push_str("; ");
    }
    out
}

fn explicit(ast: &Ast, d: DeclId) -> bool {
    ast.spec_info(d).is_some_and(|(_, i)| i.kind == SpecKind::ExplicitSpecialization)
}

/// What a client reads from the declarations of [`SHAPES`] in `ast`: per
/// shape, its written arguments or written bounds.
fn report(ast: &Ast) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, kind) in
        [("P", DeclKind::CXXRecord), ("v", DeclKind::Var), ("In", DeclKind::CXXRecord)]
    {
        for d in named(ast, name, kind) {
            let what = if ast.partial_spec_info(d).is_some() {
                "partial"
            } else if explicit(ast, d) {
                "explicit"
            } else {
                continue;
            };
            out.push((format!("{name} {what}"), written_args(ast, d)));
        }
    }
    for d in named(ast, "fh", DeclKind::Function) {
        if ast.templates.args_as_written(d).is_some() {
            out.push(("fh explicit".to_owned(), written_args(ast, d)));
        }
    }
    for (name, kind) in [
        ("arr", DeclKind::Var),
        ("a", DeclKind::ParmVar),
        ("b", DeclKind::ParmVar),
        ("a", DeclKind::Field),
        ("par", DeclKind::Function),
    ] {
        for d in named(ast, name, kind) {
            out.push((format!("{name} {kind:?} bounds"), written_bounds(ast, d)));
        }
    }
    // WP-3583: the placeholder `dv2`'s type was deduced from.
    for d in named(ast, "dv2", DeclKind::Var) {
        let kind = ast.decls.var(d).map(|v| v.spec.placeholder());
        out.push(("dv2 placeholder".to_owned(), format!("{kind:?}")));
    }
    out.sort();
    out
}

/// The written report of the shapes parsed from source; checks that it
/// holds what WP-3100..3102 promise, so the comparisons below compare
/// something.
fn source_report() -> Vec<(String, String)> {
    let (tu, _, _) = parse(&format!("{}{USE}", shapes(false)), false, Vec::new());
    let r = report(&tu.ast);
    let get = |k: &str| -> Vec<&str> {
        r.iter().filter(|(n, _)| n == k).map(|(_, v)| v.as_str()).collect()
    };
    // [temp.expl.spec]: `f(2)` stays the call; `3` the literal.
    let p = get("P explicit");
    assert_eq!(p.len(), 2, "{r:?}");
    assert!(p.iter().any(|w| w.contains("CallExpr") && w.contains("=f")), "{p:?}");
    assert!(p.iter().any(|w| w.contains("expr IntegerLiteral=3")), "{p:?}");
    for k in ["P partial", "v partial", "v explicit", "fh explicit"] {
        assert!(get(k).iter().any(|w| w.contains("CallExpr")), "{k}: {r:?}");
    }
    // [temp.spec.partial.member]: the member template's partial and its
    // instantiation in `O<long>` (WP-3102) both have a row with the call.
    let inn = get("In partial");
    assert_eq!(inn.len(), 2, "the pattern's and O<long>'s: {r:?}");
    assert!(inn.iter().all(|w| w.contains("CallExpr")), "{inn:?}");
    // [dcl.fct]/5 (WP-3101): the adjusted parameter's written bound.
    assert!(get("par Function bounds")[0].contains("CallExpr"), "{r:?}");
    assert!(get("b ParmVar bounds")[0].contains("IntegerLiteral=4"), "{r:?}");
    assert_eq!(get("dv2 placeholder"), ["Some(DecltypeAuto)"], "{r:?}");
    r
}

/// spec/modules.md §11.11 "Arguments as written": the importer of a
/// module built from its declaration image reads the written arguments
/// and bounds the interface's parse gave, and the explicit
/// specialization `P<int, f(2)>` is imported without evaluating `f(2)`
/// (WP-3100: that failed with "undefined function 'f' cannot be used in
/// a constant expression").
#[test]
fn written_arguments_and_bounds_survive_the_module_image() {
    let want = source_report();
    let iface = format!("export module m;\n{}", shapes(true));
    let (tu, capsule, _) = parse(&iface, true, Vec::new());
    let image = tu.image.expect("an image");
    assert_eq!(image.fallback, None, "the interface is carried by its image");
    let mut capsule = capsule.expect("a module interface");
    capsule
        .sections
        .push(kiln_pp::bmi::Section { tag: kiln_pp::bmi::SECTION_IMAGE, bytes: image.bytes });
    let dir = std::env::temp_dir().join(format!("kiln-written-args-image-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pcm = dir.join("m.pcm");
    std::fs::write(&pcm, kiln_pp::bmi::write(&capsule)).unwrap();
    let user = format!("import m;\n{USE}");
    let (got, _, imports) = parse(&user, false, vec![("m".to_owned(), pcm)]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(imports, [None], "built from the image");
    assert_eq!(report(&got.ast), want);
}

/// spec/pch.md: a PCH snapshot carries the same table and bounds (the
/// AST restored from its snapshot encoding answers as the parsed one).
#[test]
fn written_arguments_and_bounds_survive_the_pch_snapshot() {
    let want = source_report();
    let (tu, _, _) = parse(&format!("{}{USE}", shapes(false)), false, Vec::new());
    let bytes = kiln_snap::to_bytes(&tu.ast);
    let restored = kiln_snap::from_bytes::<Ast>(&bytes).expect("the snapshot decodes");
    assert_eq!(report(&restored), want);
}

/// [dcl.fct]/5 (WP-3101): an array parameter's written bound, constant or
/// a call, stays on the parameter's type as `Decayed` sugar, which prints
/// as the pointer; the canonical type, so the function's type, its
/// redeclarations and its overloads, is the pointer's.
#[test]
fn an_adjusted_parameter_keeps_its_written_bound() {
    let src = "constexpr int f(int x) { return x; }\n\
               int par(int a[f(3)]);\n\
               int par(int *a);\n\
               int par(int a[7]) { return a[0]; }\n\
               int par(long *a);\n\
               static_assert(__is_same(decltype(par(static_cast<int *>(nullptr))), int));\n";
    let (tu, _, _) = parse(src, false, Vec::new());
    let ast = &tu.ast;
    let pars: Vec<DeclId> = named(ast, "par", DeclKind::Function);
    assert_eq!(pars.len(), 4);
    let canon = |d: DeclId| ast.types.canonical(ast.decls.get(d).ty().unwrap());
    assert_eq!(canon(pars[0]), canon(pars[1]), "one function type");
    assert_eq!(canon(pars[0]), canon(pars[2]));
    assert_ne!(canon(pars[0]), canon(pars[3]), "an overload");
    let first = ast.decls.get(pars[0]).ty().unwrap();
    assert_eq!(ty(ast, first), "int (int *)", "prints as adjusted");
    assert_eq!(ast.redecls_of(pars[0]).count(), 3, "redeclarations");
    let params = named(ast, "a", DeclKind::ParmVar);
    let calls: Vec<String> = params.iter().map(|&p| written_bounds(ast, p)).collect();
    assert!(calls[0].contains("CallExpr") && calls[0].contains("=f"), "{calls:?}");
    assert_eq!(calls[1], "", "a pointer as written has no bound");
    assert!(calls[2].contains("IntegerLiteral=7"), "{calls:?}");
    let p0 = ast.decls.get(params[0]).ty().unwrap();
    assert_eq!(ty(ast, p0), "int *");
    assert_eq!(
        ast.types.canonical(p0),
        ast.types.canonical(ast.decls.get(params[1]).ty().unwrap())
    );
}
