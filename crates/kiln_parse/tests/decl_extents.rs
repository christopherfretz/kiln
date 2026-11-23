//! The declaration-extent table (spec/ast.md §19): off by default, it
//! records nothing and reports no memory; on, every declaration the
//! parser parsed has a row, read through `Ast::decl_extent`.

use std::sync::Arc;

use kiln_ast::decl::DeclFlags;
use kiln_ast::decl_extents::DeclEnd;
use kiln_ast::{Ast, DeclId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::symbol::Interner;
use kiln_source::{SourceManager, Span};

/// Every kind of declaration kiln parses outside a function body and
/// outside a module unit, with no instantiation.
const SRC: &str = "\
[[maybe_unused]] static const int a = 1, *b = nullptr;
namespace n::m { int v; }
namespace n { inline namespace i {} namespace {} namespace al = n::m; }
extern \"C\" int c_fn(int);
extern \"C\" { int c_var; }
struct S {
  S() : x(0) {}
  S(const S &) = default;
  virtual ~S();
  virtual void pure() = 0;
  operator int() const;
  S &operator=(const S &) = delete;
  int x, y : 3;
  static int z;
  enum E { e0, e1 = 2 };
  using T = int;
  friend void fr(S);
  friend class Other;
public:
  void late() { int in_body = 1; (void)in_body; }
  static_assert(true);
};
S::~S() {}
int S::z = 0;
template <class T, int N = 1> requires (N > 0) struct Tp { T t; template <class U> void f(U); };
template <class T, int N> requires (N > 0) template <class U> void Tp<T, N>::f(U) {}
template <> struct Tp<char, 1> {};
template <class T> struct Tp<T *, 1> {};
template <class T> concept C = true;
template <class T> using Al = T *;
template <class T> T tv = T();
template <class T> Tp(T) -> Tp<T, 1>;
typedef int Td, *TdP;
using namespace n;
using n::m::v;
enum class Ec : int { r, g };
int operator\"\"_k(unsigned long long);
void fn(int p, int = 2) try { struct Local { int l, k; }; auto [q, r] = Local{}; if (int c = 1) {} } catch (...) {}
";

fn parse(on: bool) -> (SourceManager, kiln_source::FileId, kiln_parse::ParsedTu) {
    parse_src(SRC, on)
}

fn parse_src(src: &str, on: bool) -> (SourceManager, kiln_source::FileId, kiln_parse::ParsedTu) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions { decl_extents: on, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    assert_eq!(engine.error_count(), 0, "the source has errors");
    drop(engine);
    (sm, file, tu)
}

/// The declarations named `name` of kind `kind` (by `Debug`), in order.
fn find_all(ast: &Ast, name: &str, kind: &str) -> Vec<DeclId> {
    (1..=ast.decls.len() as u32)
        .map(DeclId::new)
        .filter(|&d| {
            let n = ast.decls.get(d);
            format!("{:?}", n.kind()) == kind
                && n.name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == name)
        })
        .collect()
}

/// The first declaration named `name` of kind `kind`.
fn find(ast: &Ast, name: &str, kind: &str) -> DeclId {
    *find_all(ast, name, kind).first().unwrap_or_else(|| panic!("no {kind} '{name}'"))
}

fn text(sm: &SourceManager, s: Span) -> &str {
    sm.span_text(s).unwrap_or("?")
}

#[test]
fn off_records_nothing() {
    let (sm, file, tu) = parse(false);
    let t = &tu.ast.decl_extents;
    assert!(!t.is_on());
    assert_eq!(t.len(), 0);
    assert!(t.dump(&tu.ast, &sm, Some(file)).is_empty());
    assert!(!tu.ast.memory().rows.iter().any(|r| r.name == "decl_extents"));
}

/// The table holds no spare capacity once the parse ends (spec/ast.md
/// §19.8, WP-2092): what it reserves is what it uses.
#[test]
fn the_parse_trims_the_table() {
    let (_, _, tu) = parse(true);
    let mem = tu.ast.memory();
    let row = mem.rows.iter().find(|r| r.name == "decl_extents").expect("a decl_extents row");
    assert!(row.count > 0);
    assert_eq!(row.reserved_bytes, row.live_bytes);
}

/// Every declaration of the parse that is not implicit has a row.
#[test]
fn every_written_declaration_has_a_row() {
    let (sm, file, tu) = parse(true);
    let ast = &tu.ast;
    assert!(ast.memory().rows.iter().any(|r| r.name == "decl_extents"));
    let mut missing = Vec::new();
    for d in (1..=ast.decls.len() as u32).map(DeclId::new) {
        let n = ast.decls.get(d);
        if n.has(DeclFlags::IMPLICIT)
            || n.has(DeclFlags::INJECTED)
            || d == ast.tu
            || sm.line_col(n.loc()).is_none_or(|lc| lc.file != file)
        {
            continue;
        }
        match ast.decl_extent(d) {
            None => missing.push(format!("{:?}@{:?}", n.kind(), sm.line_col(n.loc()))),
            Some(e) => {
                assert!(e.full.lo.is_valid() && e.full.hi.is_valid(), "{:?}", n.kind());
                assert!(e.full.lo.to_raw() <= e.own.lo.to_raw(), "{:?}", n.kind());
                assert!(e.own.hi.to_raw() <= e.full.hi.to_raw(), "{:?}", n.kind());
            }
        }
    }
    assert!(missing.is_empty(), "declarations without a row: {missing:#?}");
}

#[test]
fn the_api_answers_from_the_row() {
    let (sm, _, tu) = parse(true);
    let ast = &tu.ast;
    // A declarator group: the shared prefix and each own range.
    let b = ast.decl_extent(find(ast, "b", "Var")).expect("b");
    assert_eq!(text(&sm, b.full), "[[maybe_unused]] static const int a = 1, *b = nullptr;");
    assert_eq!(text(&sm, b.own), "*b = nullptr");
    assert_eq!(text(&sm, b.name), "b");
    assert_eq!(text(&sm, b.prefix().expect("prefix")), "[[maybe_unused]] static const int");
    assert_eq!(text(&sm, b.semi().expect("semi")), ";");
    assert_eq!(b.end, DeclEnd::Semi);
    // A nested namespace definition: `m` qualified from `n`.
    let m = ast.decl_extent(find(ast, "m", "Namespace")).expect("m");
    assert_eq!(text(&sm, m.qualified_name()), "n::m");
    assert_eq!(text(&sm, m.body), "{ int v; }");
    assert_eq!(m.end, DeclEnd::Brace);
    // A member template declared in its class, then defined out of line:
    // each declaration has its own row.
    let fs = find_all(ast, "f", "CXXMethod");
    assert_eq!(fs.len(), 2, "f: the declaration and the definition");
    let fd = ast.decl_extent(fs[0]).expect("f in class");
    assert_eq!(text(&sm, fd.full), "template <class U> void f(U);");
    assert!(!fd.body.lo.is_valid());
    let fe = ast.decl_extent(fs[1]).expect("f out of line");
    assert!(text(&sm, fe.full).starts_with("template <class T, int N> requires (N > 0) template"));
    assert_eq!(text(&sm, fe.qualified_name()), "Tp<T, N>::f");
    assert_eq!(text(&sm, fe.body), "{}");
    // A template and its pattern share one row.
    let tp = find(ast, "Tp", "CXXRecord");
    let tt = ast.described_template(tp).expect("the class template");
    assert!(std::ptr::eq(ast.decl_extent(tp).unwrap(), ast.decl_extent(tt).unwrap()));
    // Bodies: a ctor-initializer, `= default`, `= 0`, a function-try-block.
    let ctor = ast.decl_extent(find_ctor(ast)).expect("S()");
    assert_eq!(text(&sm, ctor.body), ": x(0) {}");
    let pure = ast.decl_extent(find(ast, "pure", "CXXMethod")).expect("pure");
    assert_eq!(text(&sm, pure.body), "= 0");
    let fun = ast.decl_extent(find(ast, "fn", "Function")).expect("fn");
    assert!(
        text(&sm, fun.body).starts_with("try {") && text(&sm, fun.body).ends_with("catch (...) {}")
    );
}

/// The user-declared default constructor of `S`.
fn find_ctor(ast: &Ast) -> DeclId {
    (1..=ast.decls.len() as u32)
        .map(DeclId::new)
        .find(|&d| {
            let n = ast.decls.get(d);
            format!("{:?}", n.kind()) == "CXXConstructor" && !n.has(DeclFlags::IMPLICIT)
        })
        .expect("a constructor")
}

/// Declarations kiln gives one `DeclId` across redeclarations.
const REDECLS: &str = "\
template <class T> struct Q {};
template <> struct Q<int>;
template <> struct Q<int>;
template <> struct Q<int> { int m; };
template <class T> void t(T) {}
extern template void t(int);
template void t(int);
template void t(short);
";

/// One row per written declaration (spec/ast.md §19.1, WP-2470): a
/// declaration kiln reuses across redeclarations (an explicit
/// specialization, an explicit instantiation) keeps every row in source
/// order, and `decl_extent` answers from the last.
#[test]
fn every_redeclaration_keeps_its_row() {
    let (sm, _, tu) = parse_src(REDECLS, true);
    let ast = &tu.ast;
    let t = &ast.decl_extents;
    let rows = |d: DeclId| -> Vec<&str> { t.rows_of(d).map(|e| text(&sm, e.full)).collect() };
    let q: Vec<DeclId> =
        find_all(ast, "Q", "CXXRecord").into_iter().filter(|&d| t.rows_of(d).count() > 1).collect();
    assert_eq!(q.len(), 1, "one specialization Q<int>");
    assert_eq!(
        rows(q[0]),
        [
            "template <> struct Q<int>;",
            "template <> struct Q<int>;",
            "template <> struct Q<int> { int m; };"
        ]
    );
    assert_eq!(text(&sm, ast.decl_extent(q[0]).expect("Q<int>").body), "{ int m; }");
    let mut ts: Vec<Vec<&str>> = find_all(ast, "t", "Function")
        .into_iter()
        .map(rows)
        .filter(|r| !r.is_empty() && !r.iter().any(|x| x.contains('{')))
        .collect();
    ts.sort();
    assert_eq!(
        ts,
        [
            vec!["extern template void t(int);", "template void t(int);"],
            vec!["template void t(short);"],
        ]
    );
}

/// Explicit instantiations with attributes (WP-2541).
const INST_ATTRS: &str = "\
template <class T> void f(T) {}
template <class T> struct C {};
template <class T> T v = T();
extern template __attribute__((weak)) void f<int>(int);
template __attribute__((visibility(\"hidden\"))) void f<int>(int) __attribute__((weak));
template struct __attribute__((visibility(\"hidden\"))) C<long>;
template __attribute__((weak)) int v<int>;
";

/// An explicit instantiation declares nothing, so its row records its
/// attributes as written: each one's kind and extent, from its name to
/// its last token, the dropped one after the declarator too (spec/ast.md
/// §19.2; WP-2541). Sema merges the decl-specifier ones onto the
/// specialization.
#[test]
fn an_explicit_instantiation_row_keeps_its_attributes() {
    use kiln_ast::attr::{AttrKind, AttrSyntax};
    let (sm, file, tu) = parse_src(INST_ATTRS, true);
    let ast = &tu.ast;
    let table = &ast.decl_extents;
    let written = |x: DeclId, row: usize| -> Vec<(AttrKind, AttrSyntax, &str)> {
        table.row_attrs(x, row).map(|a| (a.kind, a.syntax, text(&sm, a.span))).collect()
    };
    let fs: Vec<DeclId> = find_all(ast, "f", "Function")
        .into_iter()
        .filter(|&x| table.rows_of(x).count() == 2)
        .collect();
    assert_eq!(fs.len(), 1, "f<int>, written twice");
    let vis = (AttrKind::VISIBILITY, AttrSyntax::Gnu, "visibility(\"hidden\")");
    assert_eq!(written(fs[0], 0), [(AttrKind::WEAK, AttrSyntax::Gnu, "weak")]);
    assert_eq!(written(fs[0], 1), [vis, (AttrKind::WEAK, AttrSyntax::Gnu, "weak")]);
    let class = find_all(ast, "C", "CXXRecord")
        .into_iter()
        .find(|&x| table.row_attrs(x, 0).next().is_some() || table.row_attrs(x, 1).next().is_some())
        .expect("C<long> has a written attribute");
    let last = table.rows_of(class).count() - 1;
    assert_eq!(written(class, last), [vis]);
    let var = find_all(ast, "v", "Var")
        .into_iter()
        .find(|&x| table.row_attrs(x, 0).next().is_some())
        .expect("v<int> has a written attribute");
    assert_eq!(written(var, 0), [(AttrKind::WEAK, AttrSyntax::Gnu, "weak")]);
    // Merged: the decl-specifier attributes only.
    let merged: Vec<AttrKind> =
        ast.lists.iter(ast.attrs.of_decl(fs[0])).map(|a| ast.attrs.get(a).kind).collect();
    assert!(merged.contains(&AttrKind::WEAK) && merged.contains(&AttrKind::VISIBILITY));
    let dump = table.dump(ast, &sm, Some(file));
    assert!(dump.contains(" attr=Weak@4:32-4:36"), "{dump}");
    assert!(dump.contains(" attr=Visibility@5:25-5:45"), "{dump}");
}
