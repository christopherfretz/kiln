//! The mem-initializers of a constructor definition in a template, read
//! from the `Ast` alone after the parse (spec/ast.md §8.6, WP-3026):
//! `Ast::pattern_mem_inits` gives the `:`, and per mem-initializer in
//! written order the mem-initializer-id as the definition resolved it,
//! the written range, the pack expansion and the initializer's
//! expressions; a constructor that is no pattern has its resolved list
//! (`Ast::ctor_inits`) instead. A pattern constructor without a
//! ctor-initializer has a record too: no `:` and no mem-initializers
//! (WP-3200). The same record after a prefix snapshot
//! round trip (a PCH); the module-image round trip is
//! `kiln_driver/tests/front_end.rs` (`pattern_mem_inits_through_a_module_image`).

use std::fmt::Write as _;
use std::sync::Arc;

use kiln_ast::decl::DeclKind;
use kiln_ast::diag_names::DiagNames;
use kiln_ast::expr::ExprKind;
use kiln_ast::templates::{PatternMemInitArgs, PatternMemInitId};
use kiln_ast::types::{PrintOptions, print_type};
use kiln_ast::{Ast, DeclId, ExprId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::symbol::Interner;
use kiln_source::{Loc, SourceManager};

/// Every kind of mem-initializer-id and initializer in a pattern; nothing
/// is instantiated. Compiles with clang 19 and 22.
const SRC: &str = "\
struct B { B(int); };
template <class T> struct Base { Base(T); };
template <class T> struct Field { T x; Field(T a) : x(a) {} };
template <class T> struct Anon { union { int i; T t; }; Anon() : i(0) {} };
template <class T> struct NonDep : B { NonDep() : B(1) {} };
template <class T> struct Dep : Base<T> { Dep(T t) : Base<T>(t) {} };
template <class T> struct DelegA { DelegA(int); DelegA() : DelegA(0) {} };
template <class T> struct DelegB { DelegB(int); DelegB() : DelegB<T>(0) {} };
template <class... Bs> struct Pack : Bs... { Pack(int a) : Bs(a)... {} };
template <class T> struct Braced { T x; int a[2]; Braced() : x{1}, a{1, {2}} {} };
template <class T> struct Desig { struct P { int p, q; } m; Desig() : m{.q = 1} {} };
template <class T> struct MemTmpl { T x; template <class U> MemTmpl(U u) : x(u) {} };
template <class T> struct Outer { struct Inner { T v; Inner() : v() {} }; };
struct NonTmpl { int v; template <class U> NonTmpl(U u) : v(u) {} };
struct Plain { int v; Plain() : v(1) {} };
template <class T> struct Empty { T m; Empty() {} };
";

fn ident(ast: &Ast, d: DeclId) -> &str {
    ast.decls.get(d).name().as_ident().map_or("", |s| ast.symbols().as_str(s))
}

/// The constructors of class `class` (by its name) that have a pattern
/// record, in declaration order.
fn ctors(ast: &Ast, class: &str) -> Vec<DeclId> {
    ast.decls
        .ids()
        .filter(|&d| {
            ast.decls.get(d).kind() == DeclKind::CXXConstructor
                && ast.decls.get(d).ctx().is_some_and(|c| ident(ast, c) == class)
                && ast.pattern_mem_inits(d).is_some()
        })
        .collect()
}

fn lc(sm: &SourceManager, l: Loc) -> String {
    sm.line_col(sm.spelling_loc(l))
        .map_or_else(|| "?".to_owned(), |x| format!("{}:{}", x.line, x.col))
}

/// The shape of a braced list: its clauses' kinds, nested lists in braces.
fn list(ast: &Ast, sm: &SourceManager, e: ExprId) -> String {
    let ex = &ast.exprs;
    assert_eq!(ex.get(e).kind(), ExprKind::InitListExpr);
    assert!(ast.types.is_void(ex.get(e).ty()), "a pattern's list has type void");
    let items: Vec<String> = ast
        .lists
        .iter::<ExprId>(ex.init_list_inits(e))
        .map(|x| match ex.get(x).kind() {
            ExprKind::InitListExpr => list(ast, sm, x),
            k => format!("{k:?}"),
        })
        .collect();
    let end = ex.end_loc(e).map_or_else(|| "-".to_owned(), |l| lc(sm, l));
    format!("{{{}}}@{}..{end}", items.join(" "), lc(sm, ex.get(e).loc()))
}

/// The record of constructor `d`, one line per mem-initializer.
fn describe(ast: &Ast, sm: &SourceManager, d: DeclId) -> String {
    let names = DiagNames::new(ast, Some(sm), 19);
    let Some(p) = ast.pattern_mem_inits(d) else { return "none".to_owned() };
    let colon = p.colon.map_or_else(|| "none".to_owned(), |c| lc(sm, c));
    let mut out = format!(": {colon}{}", if p.failed { " failed" } else { "" });
    for m in &p.inits {
        let id = match m.id {
            PatternMemInitId::Member(f) => {
                format!("member {:?} {}", ast.decls.get(f).kind(), ident(ast, f))
            }
            PatternMemInitId::Type { ty, ident } => format!(
                "type '{}'{}",
                print_type(&ast.types, &names, ty, PrintOptions::SUGARED),
                ident.map_or(String::new(), |s| format!(" as {}", ast.symbols().as_str(s)))
            ),
            PatternMemInitId::Ident(s) => format!("ident {}", ast.symbols().as_str(s)),
        };
        let init = match &m.init {
            PatternMemInitArgs::Paren { args, lparen, rparen } => {
                let kinds: Vec<String> =
                    args.iter().map(|&a| format!("{:?}", ast.exprs.get(a).kind())).collect();
                format!("({})@{}..{}", kinds.join(" "), lc(sm, *lparen), lc(sm, *rparen))
            }
            PatternMemInitArgs::List(e) => list(ast, sm, *e),
        };
        let pack = if m.pack_expansion { " ..." } else { "" };
        let _ = write!(out, "\n  {id} [{}..{}]{pack} {init}", lc(sm, m.loc), lc(sm, m.end));
    }
    out
}

/// Parses `src` and hands the AST, the source manager and the error
/// count to `check`.
fn with_ast<R>(src: &str, check: impl FnOnce(&Ast, &SourceManager, u32) -> R) -> R {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let target = Target::for_arch(Arch::X86_64);
    let opts = ParseOptions::default();
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    check(&tu.ast, &sm, engine.error_count())
}

fn one(ast: &Ast, sm: &SourceManager, class: &str) -> String {
    let c = ctors(ast, class);
    assert_eq!(c.len(), 1, "{class}: {c:?}");
    describe(ast, sm, c[0])
}

#[test]
fn every_kind_of_pattern_mem_initializer() {
    with_ast(SRC, |ast, sm, errors| {
        assert_eq!(errors, 0);
        let got: Vec<String> = [
            "Field", "Anon", "NonDep", "Dep", "DelegA", "DelegB", "Pack", "Braced", "Desig",
            "MemTmpl", "Inner", "NonTmpl",
        ]
        .iter()
        .map(|c| format!("{c}{}", one(ast, sm, c)))
        .collect();
        let want = [
            "Field: 3:51\n  member Field x [3:53..3:56] (DeclRefExpr)@3:54..3:56",
            "Anon: 4:64\n  member IndirectField i [4:66..4:69] (IntegerLiteral)@4:67..4:69",
            "NonDep: 5:49\n  type 'B' as B [5:51..5:54] (IntegerLiteral)@5:52..5:54",
            "Dep: 6:52\n  type 'Base<T>' [6:54..6:63] (DeclRefExpr)@6:61..6:63",
            "DelegA: 7:58\n  type 'DelegA<T>' as DelegA [7:60..7:68] (IntegerLiteral)@7:66..7:68",
            "DelegB: 8:58\n  type 'DelegB<T>' [8:60..8:71] (IntegerLiteral)@8:69..8:71",
            // A template parameter's name is a type to the parser already.
            "Pack: 9:58\n  type 'Bs' [9:60..9:65] ... (DeclRefExpr)@9:62..9:64",
            "Braced: 10:60\n  member Field x [10:62..10:65] {IntegerLiteral}@10:63..10:65\n  \
             member Field a [10:68..10:76] {IntegerLiteral {IntegerLiteral}@10:73..10:75}@10:69..10:76",
            "Desig: 11:69\n  member Field m [11:71..11:79] {DesignatedInitExpr}@11:72..11:79",
            "MemTmpl: 12:74\n  member Field x [12:76..12:79] (DeclRefExpr)@12:77..12:79",
            "Inner: 13:63\n  member Field v [13:65..13:67] ()@13:66..13:67",
            "NonTmpl: 14:57\n  member Field v [14:59..14:62] (DeclRefExpr)@14:60..14:62",
        ];
        assert_eq!(got, want, "\n{}", got.join("\n"));
    });
}

/// A constructor that is no pattern: no record, its resolved list.
#[test]
fn a_constructor_that_is_no_pattern_has_its_resolved_list() {
    with_ast(SRC, |ast, _, _| {
        let plain: Vec<DeclId> = ast
            .decls
            .ids()
            .filter(|&d| {
                ast.decls.get(d).kind() == DeclKind::CXXConstructor
                    && ast.decls.get(d).ctx().is_some_and(|c| ident(ast, c) == "Plain")
                    && ast.decls.function(d).is_some_and(|f| f.body.is_some())
            })
            .collect();
        assert_eq!(plain.len(), 1);
        assert!(ast.pattern_mem_inits(plain[0]).is_none());
        assert_eq!(ast.ctor_inits(plain[0]).filter(|c| c.is_written()).count(), 1);
    });
}

/// A mem-initializer the definition rejects ([class.base.init]/2): the
/// record is `failed`; an identifier that names nothing stays as written
/// (`Ident`), also in a pack expansion (clang 19 and 22 report both at
/// the definition, `does not name a non-static data member or base
/// class`).
#[test]
fn a_failed_and_an_unresolved_mem_initializer() {
    const BAD: &str = "\
template <class T> struct D : T { D() : T(), x(1) {} };
template <class... Ts> struct P : Ts... { P() : foo()... {} };
";
    with_ast(BAD, |ast, sm, errors| {
        assert_eq!(errors, 2);
        assert_eq!(
            one(ast, sm, "D"),
            ": 1:39 failed\n  type 'T' [1:41..1:43] ()@1:42..1:43\n  ident x [1:46..1:49] (IntegerLiteral)@1:47..1:49"
        );
        assert_eq!(one(ast, sm, "P"), ": 2:47 failed\n  ident foo [2:49..2:54] ... ()@2:52..2:53");
    });
}

/// The records after a prefix snapshot round trip (what a PCH is): the
/// same `PatternMemInits` on the same constructors.
#[test]
fn pattern_mem_inits_through_a_prefix_snapshot() {
    use kiln_lex::Std;
    use kiln_parse::{new_sema, parse_prefix};
    use kiln_pp::{PpOptions, Preprocessor};
    use kiln_snap::{Reader, Writer};

    let target = Target::for_arch(Arch::X86_64);
    let opts = ParseOptions::default();
    let mut o = PpOptions::new(Std::Cxx26);
    o.parser_pragmas = true;
    o.main_is_header = true;
    let records = |ast: &Ast| -> Vec<(DeclId, kiln_ast::templates::PatternMemInits)> {
        ast.decls.ids().filter_map(|d| ast.pattern_mem_inits(d).map(|p| (d, p.clone()))).collect()
    };
    let (bytes, before, described) = {
        let sm = SourceManager::new();
        let interner = Arc::new(Interner::new());
        let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
        let file = sm.add_virtual("prefix.h", SRC.to_owned()).expect("location space");
        let mut pp = Preprocessor::new(&sm, &engine, &interner, o.clone());
        pp.enter_main_file(file);
        let p = parse_prefix(
            pp,
            Arc::clone(&interner),
            &engine,
            target.clone(),
            &opts,
            ConstexprLimits::default(),
        );
        p.ready.expect("the prefix is at rest");
        assert_eq!(engine.error_count(), 0);
        let before = records(&p.sema.ast);
        let described: Vec<String> =
            before.iter().map(|&(d, _)| describe(&p.sema.ast, &sm, d)).collect();
        let mut w = Writer::new();
        w.put(&sm);
        w.put(&interner);
        p.sema.snap_put(&mut w).expect("the prefix is at rest");
        (w.into_bytes(), before, described)
    };
    assert_eq!(before.len(), 13, "every pattern constructor of SRC, `Empty() {{}}` included");
    assert!(before.iter().any(|(_, p)| p.colon.is_none() && p.inits.is_empty()));
    let mut r = Reader::new(&bytes);
    let sm: SourceManager = r.get().expect("source manager");
    let interner: Arc<Interner> = r.get().expect("interner");
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut s = new_sema(Arc::clone(&interner), &engine, target, &opts, ConstexprLimits::default());
    s.snap_restore(&mut r).expect("restored");
    r.finish().expect("the whole snapshot is read");
    assert_eq!(records(&s.ast), before);
    let after: Vec<String> = before.iter().map(|&(d, _)| describe(&s.ast, &sm, d)).collect();
    assert_eq!(after, described);
}

/// Every pattern constructor definition has a record, the empty list
/// included (spec/ast.md §8.6, WP-3200; [class.base.init]/1: the
/// ctor-initializer is optional): no `:` (`colon == None`) and no
/// mem-initializers when none is written, whatever kind of pattern it is
/// (a class template's constructor, a constructor template, a member
/// class's, an out-of-class definition, a function-try-block), beside
/// the same constructors with a ctor-initializer, as before. A
/// constructor that is no pattern still has no record. Compiles with
/// clang 19 and 22.
#[test]
fn a_pattern_constructor_without_a_ctor_initializer_has_an_empty_record() {
    const EMPTY: &str = "\
template <class T> struct S { T m; S() {} S(int) : m() {} };
template <class T> struct C { T m; template <class U> C(U) {} };
struct NC { int v; template <class U> NC(U) {} };
template <class T> struct O { struct I { T v; I() {} }; };
template <class T> struct Out { Out(); };
template <class T> Out<T>::Out() {}
template <class T> struct Tr { Tr() try {} catch (...) {} };
template <class T> struct Del { Del(int); Del() : Del(0) {} };
template <class... Bs> struct P : Bs... { P() {} P(int a) : Bs(a)... {} };
struct N { int v; N() {} };
";
    with_ast(EMPTY, |ast, sm, errors| {
        assert_eq!(errors, 0);
        let s = ctors(ast, "S");
        assert_eq!(s.len(), 2, "both constructors of S have a record");
        let (empty, written) = (ast.pattern_mem_inits(s[0]).unwrap(), &describe(ast, sm, s[1]));
        assert_eq!(empty.colon, None);
        assert!(empty.inits.is_empty() && !empty.failed);
        assert_eq!(written, ": 1:50\n  member Field m [1:52..1:54] ()@1:53..1:54");
        let got: Vec<String> = ["C", "NC", "I", "Out", "Tr", "Del", "P"]
            .iter()
            .map(|c| {
                let all: Vec<String> =
                    ctors(ast, c).into_iter().map(|d| describe(ast, sm, d)).collect();
                format!("{c} {}", all.join(" | "))
            })
            .collect();
        let want = [
            "C : none",
            "NC : none",
            "I : none",
            "Out : none",
            "Tr : none",
            "Del : 8:49\n  type 'Del<T>' as Del [8:51..8:56] (IntegerLiteral)@8:54..8:56",
            "P : none | : 9:59\n  type 'Bs' [9:61..9:66] ... (DeclRefExpr)@9:63..9:65",
        ];
        assert_eq!(got, want, "\n{}", got.join("\n"));
        // No pattern: no record, the resolved list as before.
        let n: Vec<DeclId> = ast
            .decls
            .ids()
            .filter(|&d| {
                ast.decls.get(d).kind() == DeclKind::CXXConstructor
                    && ast.decls.get(d).ctx().is_some_and(|c| ident(ast, c) == "N")
                    && ast.decls.function(d).is_some_and(|f| f.body.is_some())
            })
            .collect();
        assert_eq!(n.len(), 1);
        assert!(ast.pattern_mem_inits(n[0]).is_none());
        assert!(ast.ctor_inits(n[0]).all(|c| !c.is_written()));
    });
}
