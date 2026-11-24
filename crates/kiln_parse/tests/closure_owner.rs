//! Closure owners (spec/ast.md §5.9 "Closure owners", spec/sema-lambda.md
//! §3.6; WP-2023): `Ast::closure_owner` names the declaration whose text
//! holds a closure's lambda-expression, for the closures `LambdaMangling`
//! gives a context ([expr.prim.lambda.closure]/2 and Itanium ABI §5.1.8
//! numbering sites) and for those it leaves without one, with their
//! instantiations ([temp.inst]).

use std::sync::Arc;

use kiln_ast::ids::LambdaId;
use kiln_ast::{Ast, DeclId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// One closure per case, each case on its own line (the test keys on
/// the line of the lambda's `[`).
const SRC: &str = "struct S {\n\
  void f(int a = [] { return 1; }());\n\
  int m = [] { return 2; }();\n\
};\n\
inline auto iv = [] { return 3; };\n\
template <class T> auto tv = [] { return T(); };\n\
template <class T> auto sig(T) -> decltype([] { return T(); }()) { return {}; }\n\
template <class T = decltype([] {})> struct D {};\n\
void nd(decltype([] {}) p);\n\
auto nr() -> decltype([] { return 4; }());\n\
template <class T> requires (sizeof([] {}) > 0) void rq(T) {}\n\
template <class T> void trq(T) requires (sizeof([] {}) > 0) {}\n\
template <class T> using A = decltype([] {});\n\
void fd(int x = [] { return 5; }());\n\
auto px = [] { return 6; };\n\
void body() { auto l = [] {}; }\n\
inline auto nest = [] { return [] {}; };\n\
template <class T> void tb() { auto l = [] {}; }\n\
template <class T> struct C {\n\
  int m = [] { return 7; }();\n\
  void g(int a = [] { return 8; }()) {}\n\
};\n\
static_assert(sizeof([] {}) > 0);\n\
using NA = decltype([] {});\n\
template <class T> struct M { void h(decltype([] {}) p); };\n\
template <class T> concept K = requires { [] {}; };\n\
void lc() { struct L { int q = [] { return 9; }(); }; }\n\
template <class T> requires (sizeof([] {}) > 0) struct RC {};\n\
void use() {\n\
  tv<int>();\n\
  sig(1);\n\
  D<> d;\n\
  rq(1);\n\
  trq(1);\n\
  A<int> a;\n\
  tb<int>();\n\
  C<int> c;\n\
  c.g();\n\
  M<int> mm;\n\
  RC<int> rc;\n\
}\n";

/// A declaration as `Kind name`, with ` spec` for a specialization or
/// a member of one.
fn describe(ast: &Ast, d: DeclId) -> String {
    let n = ast.decls.get(d);
    let name = n.name().as_ident().map_or("-", |s| ast.symbols().as_str(s));
    let mut s = format!("{:?} {name}", n.kind());
    let mut x = Some(d);
    let mut spec = false;
    while let Some(y) = x {
        spec |= ast.spec_info(y).is_some();
        x = ast.decls.get(y).ctx();
    }
    if spec {
        s.push_str(" spec");
    }
    s
}

/// Each closure's line and owner, in creation order.
fn owners() -> Vec<(u32, String)> {
    owners_with(SRC, |ast| ast)
}

/// [`owners`] of source `src`, in the AST `f` gives for the parsed one.
fn owners_with(src: &str, f: impl FnOnce(Ast) -> Ast) -> Vec<(u32, String)> {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let diags = sink.take();
    assert!(diags.is_empty(), "{diags:?}");
    let ast = &f(tu.ast);
    let mut out = Vec::new();
    for i in 1..=ast.exprs.lambdas.len() {
        let l = ast.exprs.lambdas.get(LambdaId::new(u32::try_from(i).unwrap()));
        let line = sm.line_col(l.lbracket).map_or(0, |lc| lc.line);
        let o = ast.closure_owner(l.closure).expect("a closure has an owner");
        out.push((line, describe(ast, o)));
    }
    out
}

/// Every case of spec/ast.md §5.9, parsed and instantiated. The lines
/// `LambdaMangling.context` sets (2, 3, 5, 6, 7) and those it leaves
/// unset (8 to 15, 23 to 28), then the instantiations `use` triggers.
#[test]
fn closure_owners_expr_prim_lambda_closure_2() {
    let want: &[(u32, &str)] = &[
        // An in-class default argument: the parameter.
        (2, "ParmVar a"),
        // A default member initializer: the field.
        (3, "Field m"),
        // An inline variable, a variable template: the variable.
        (5, "Var iv"),
        (6, "Var tv"),
        // A function template's signature: the function.
        (7, "Function sig"),
        // A default template argument: the template parameter.
        (8, "TemplateTypeParm T"),
        // `decltype` in a non-template signature: the parameter, the
        // function.
        (9, "ParmVar p"),
        (10, "Function nr"),
        // A template head's and a trailing requires-clause: the templated
        // function.
        (11, "Function rq"),
        (12, "Function trq"),
        // An alias template: the alias.
        (13, "TypeAlias A"),
        // A namespace-scope function's default argument: the parameter.
        (14, "ParmVar x"),
        // A plain namespace-scope variable.
        (15, "Var px"),
        // A function body: the function, not the local variable.
        (16, "Function body"),
        // A lambda's body: its call operator.
        (17, "Var nest"),
        (17, "CXXMethod -"),
        (18, "Function tb"),
        (21, "ParmVar a"),
        (20, "Field m"),
        // No declaration but the translation unit holds it.
        (23, "TranslationUnit -"),
        (24, "TypeAlias NA"),
        (25, "ParmVar p"),
        // A requires-expression's parameter scope is skipped.
        (26, "Concept K"),
        (27, "Field q"),
        (28, "CXXRecord RC"),
        // Instantiations ([temp.inst]): the variable template's and
        // function template's specializations (its declaration's
        // signature)...
        (6, "Var tv spec"),
        (7, "Function sig spec"),
        // ...the pattern's owner where no declaration is instantiated: a
        // default template argument at a use, constraint checks, an alias
        // template...
        (8, "TemplateTypeParm T"),
        (11, "Function rq"),
        (12, "Function trq"),
        (13, "TypeAlias A"),
        // ...the instantiated field, parameter and member function (its
        // substituted signature)...
        (20, "Field m spec"),
        (21, "ParmVar a spec"),
        (25, "CXXMethod h spec"),
        (28, "CXXRecord RC"),
        // ...and the specialization whose body is instantiated.
        (18, "Function tb spec"),
    ];
    let got = owners();
    let got: Vec<(u32, &str)> = got.iter().map(|(l, d)| (*l, d.as_str())).collect();
    assert_eq!(got, want);
}

/// Lambdas in the default arguments of a lambda's parameters (one case
/// per line): the parameter owns the closure, as for a function's
/// parameter ([dcl.fct.default]/3, [expr.prim.lambda.general]/3).
const LAMBDA_DEFAULTS: &str = "auto f = [](int x = [] { return 1; }()) { return x; };\n\
inline int fn() {\n\
  auto l = [](int y = [] { return 2; }()) { return y; };\n\
  auto gl = [](auto a, int z = [] { return 3; }()) { return a + z; };\n\
  auto nn = [](int p = [](int q = [] { return 4; }()) { return q; }()) { return p; };\n\
  struct L { int m(int r = [] { return 5; }()) { return r; } };\n\
  struct M { int k = [](int w = [] { return 6; }()) { return w; }(); };\n\
  struct N { int n() { auto o = [](int v = [] { return 8; }()) { return v; }; return o(); } };\n\
  return l() + gl(1) + nn() + L().m() + M().k + N().n();\n\
}\n\
template <class T> int tf() {\n\
  auto t = [](T u = [] { return T(7); }()) { return u; };\n\
  return t();\n\
}\n\
int use() { return f() + fn() + tf<int>(); }\n";

/// Each closure of [`LAMBDA_DEFAULTS`] is owned by the parameter whose
/// default argument holds it, nested and generic lambdas, a local class's
/// member function and an instantiation included (spec/ast.md §5.9).
#[test]
fn closure_owners_lambda_param_default_dcl_fct_default_3() {
    let got = owners_with(LAMBDA_DEFAULTS, |ast| ast);
    let got: Vec<(u32, &str)> = got.iter().map(|(l, d)| (*l, d.as_str())).collect();
    let want: &[(u32, &str)] = &[
        // At namespace scope: the variable, then the parameter.
        (1, "Var f"),
        (1, "ParmVar x"),
        // In a function body: the function owns the lambda, its parameter
        // the closure of the default argument (nested: the innermost
        // parameter); a generic lambda's parameter too.
        (3, "Function fn"),
        (3, "ParmVar y"),
        (4, "Function fn"),
        (4, "ParmVar z"),
        (5, "Function fn"),
        (5, "ParmVar p"),
        (5, "ParmVar q"),
        // A local class's member function, default member initializer and
        // member function body.
        (6, "ParmVar r"),
        (7, "Field k"),
        (7, "ParmVar w"),
        (8, "CXXMethod n"),
        (8, "ParmVar v"),
        // The generic lambda's default argument, instantiated for its
        // specialization's parameter at the call `gl(1)`.
        (4, "ParmVar z spec"),
        // A function template: the pattern's, then the specialization's.
        (12, "Function tf"),
        (12, "ParmVar u"),
        (12, "Function tf spec"),
        (12, "ParmVar u spec"),
    ];
    assert_eq!(got, want);
}

/// A module interface whose closures have owners other than their
/// contexts (each on its own line). A lambda in a type (`decltype([] {})`)
/// is not carried by the image (a replay fallback), so none is here.
const IFACE: &str = "export module m;\n\
export struct S { void f(int a = [] { return 1; }()) {} int q = [] { return 2; }(); };\n\
export inline auto iv = [] { return 3; };\n\
export inline int fd(int x = [] { return 5; }()) { return x; }\n\
export template <class T> inline int ft(T t) { return [] { return 7; }() + t; }\n\
export inline int uc() { return ft(1); }\n";

/// A PCH snapshot carries the owners: the AST restored from its
/// snapshot encoding (spec/pch.md; the cold maps are snapshot state)
/// answers as the parsed one.
#[test]
fn closure_owners_survive_the_pch_snapshot() {
    let restored = owners_with(SRC, |ast| {
        let bytes = kiln_snap::to_bytes(&ast);
        kiln_snap::from_bytes::<Ast>(&bytes).expect("the snapshot decodes")
    });
    assert_eq!(restored, owners());
}

/// What [`parse_module`] returns.
type Parsed =
    (Vec<(u32, String)>, kiln_parse::ParsedTu, Option<kiln_pp::bmi::Capsule>, Vec<Option<String>>);

/// Parses `src` (a module unit when `record`) with modules on; `files`
/// are the module files it may import. Returns each closure's line
/// and owner, the parsed unit, and the image imports' replay reasons.
fn parse_module(src: &str, record: bool, files: Vec<(String, std::path::PathBuf)>) -> Parsed {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cppm", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts =
        ParseOptions { std: kiln_lex::Std::Cxx20, record_image: record, ..ParseOptions::default() };
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
    let diags = sink.take();
    assert!(diags.is_empty(), "{diags:?}");
    let imports = pp.image_imports().iter().map(|i| i.replayed.clone()).collect();
    let capsule = pp.take_capsule();
    let ast = &tu.ast;
    let mut out = Vec::new();
    for i in 1..=ast.exprs.lambdas.len() {
        let l = ast.exprs.lambdas.get(LambdaId::new(u32::try_from(i).unwrap()));
        let line = sm.line_col(l.lbracket).map_or(0, |lc| lc.line);
        let o = ast.closure_owner(l.closure).expect("a closure has an owner");
        out.push((line, describe(ast, o)));
    }
    (out, tu, capsule, imports)
}

/// spec/modules.md §11.11 (the lambdas section's `owner` word): an
/// importer that builds the module from its declaration image sees the
/// owners the interface's parse gave ([module.interface],
/// [expr.prim.lambda.closure]/2).
#[test]
fn closure_owners_survive_the_module_image_module_interface() {
    let (want, tu, capsule, _) = parse_module(IFACE, true, Vec::new());
    let image = tu.image.expect("an image");
    assert_eq!(image.fallback, None);
    let mut capsule = capsule.expect("a module interface");
    capsule
        .sections
        .push(kiln_pp::bmi::Section { tag: kiln_pp::bmi::SECTION_IMAGE, bytes: image.bytes });
    let dir = std::env::temp_dir().join(format!("kiln-closure-owner-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pcm = dir.join("m.pcm");
    std::fs::write(&pcm, kiln_pp::bmi::write(&capsule)).unwrap();
    let user = "import m;\nint use() { S s; s.f(); return fd() + iv() + uc(); }\n";
    let (got, _, _, imports) = parse_module(user, false, vec![("m".to_owned(), pcm)]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(imports, [None], "built from the image");
    let owners = |v: &[(u32, String)]| {
        let mut o: Vec<String> = v.iter().map(|(_, d)| d.clone()).collect();
        o.sort();
        o
    };
    assert_eq!(
        want.iter().map(|(_, d)| d.as_str()).collect::<Vec<_>>(),
        ["ParmVar a", "Field q", "Var iv", "ParmVar x", "Function ft", "Function ft spec"]
    );
    // The image's closures (the instantiated one included).
    let got: Vec<String> = owners(&got);
    for w in owners(&want) {
        assert!(got.contains(&w), "{w} missing from {got:?}");
    }
}
