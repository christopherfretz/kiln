//! Function-name spans (spec/ast.md §24, WP-3280): with the table on,
//! every function-name string (`__PRETTY_FUNCTION__`, `__func__`,
//! `__builtin_FUNCTION()`, `source_location::function_name()`) has one
//! record per (string, function), whose entries give the bytes of each
//! declaration name printed and the declaration. The accessors answer at
//! a `PredefinedExpr`, at a source-location use and at a pointer into the
//! made string; they answer `None` in exactly the cases §24.5 lists. Every
//! parse here runs the self-check (§24.7) and must give no error. The
//! sources compile with clang++-19 and clang++-22.

use std::sync::Arc;

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::fn_names::{self, FnNameAt, FnNameRole, FnNameView};
use kiln_ast::{Ast, DeclId, ExprId};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// A `std::source_location` shaped as libstdc++'s.
const SOURCE_LOCATION: &str = "\
namespace std {
struct source_location {
  struct __impl {
    const char *_M_file_name;
    const char *_M_function_name;
    unsigned _M_line;
    unsigned _M_column;
  };
  using __bsl_ty = decltype(__builtin_source_location());
  static consteval source_location current(__bsl_ty __p = __builtin_source_location()) noexcept {
    source_location r;
    r._M_impl = static_cast<const __impl *>(__p);
    return r;
  }
  constexpr const char *function_name() const noexcept {
    return _M_impl ? _M_impl->_M_function_name : \"\";
  }
  const __impl *_M_impl = nullptr;
};
}
";

/// Parses `src` as C++26 with the table on (and self-checked) when `on`,
/// asserts no error, and runs `f`.
fn parse_with<R>(src: &str, on: bool, f: impl FnOnce(&Ast, &SourceManager) -> R) -> R {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions {
        std: kiln_lex::Std::Cxx26,
        fn_names: on,
        fn_names_verify: on,
        ..ParseOptions::default()
    };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let errors: Vec<String> =
        sink.take().iter().filter(|d| d.level >= Level::Error).map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "{errors:?}\n{src}");
    f(&tu.ast, &sm)
}

fn parse<R>(src: &str, f: impl FnOnce(&Ast) -> R) -> R {
    parse_with(src, true, |ast, _| f(ast))
}

fn named(ast: &Ast, d: DeclId, name: &str) -> bool {
    ast.decls.get(d).name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == name)
}

/// The function named `name`.
fn function(ast: &Ast, name: &str) -> DeclId {
    (1..=ast.decls.len() as u32)
        .map(DeclId::new)
        .find(|&d| ast.decls.get(d).kind() == DeclKind::Function && named(ast, d, name))
        .expect(name)
}

/// Every expression of kind `k`, in creation order.
fn exprs_of(ast: &Ast, k: ExprKind) -> Vec<ExprId> {
    (1..=ast.exprs.len() as u32)
        .map(ExprId::new)
        .filter(|&e| ast.exprs.get(e).kind() == k)
        .collect()
}

/// The view's string.
fn bytes(v: &FnNameView<'_>) -> String {
    String::from_utf8(v.bytes.to_vec()).expect("UTF-8")
}

/// `(role, text, depth)` of each entry.
fn entries(v: &FnNameView<'_>) -> Vec<(FnNameRole, String, u16)> {
    (0..v.spans.len())
        .map(|i| {
            let text = String::from_utf8(v.text(i).to_vec()).expect("UTF-8");
            (v.spans[i].role, text, v.spans[i].depth)
        })
        .collect()
}

/// The view of the only `PredefinedExpr` with a record whose string is
/// `want`.
fn predefined<'a>(ast: &'a Ast, want: &str) -> FnNameView<'a> {
    let mut found: Vec<FnNameView<'a>> = exprs_of(ast, ExprKind::PredefinedExpr)
        .into_iter()
        .filter_map(|e| ast.fn_name_of_predefined(e))
        .filter(|v| v.bytes == want.as_bytes())
        .collect();
    assert_eq!(found.len(), 1, "{want}");
    found.remove(0)
}

/// R1, R2, R3, R6, R8 (spec/ast.md §24.4): the scope's specialization,
/// its canonical argument one depth in, the name, a parameter as the
/// pattern writes it and the binding of the same template parameter.
#[test]
fn predefined_expr_entries_dcl_fct_def_general_8() {
    let src = "\
namespace ns {
struct S {};
template <class T> struct C { void f(T) { (void)__PRETTY_FUNCTION__; } };
}
void use() { ns::C<ns::S>{}.f({}); }
";
    parse(src, |ast| {
        let v = predefined(ast, "void ns::C<ns::S>::f(T) [T = ns::S]");
        let got = entries(&v);
        let want = [
            (FnNameRole::Scope, "ns", 0),
            (FnNameRole::Scope, "C", 0),
            (FnNameRole::Scope, "ns", 1),
            (FnNameRole::Scope, "S", 1),
            (FnNameRole::Name, "f", 0),
            (FnNameRole::Param, "T", 0),
            (FnNameRole::Binding, "T", 0),
            (FnNameRole::Arg, "ns", 0),
            (FnNameRole::Arg, "S", 0),
        ];
        let got: Vec<(FnNameRole, &str, u16)> =
            got.iter().map(|(r, t, d)| (*r, t.as_str(), *d)).collect();
        assert_eq!(got, want);
        // The parameter and the binding name the same template parameter.
        assert_eq!(v.spans[5].entity, v.spans[6].entity);
        assert!(v.spans[5].entity.is_some());
        // Both `ns` entries are one namespace, normalised (`Ast::first`).
        assert_eq!(v.spans[0].entity, v.spans[7].entity);
        assert_eq!(v.spans[0].entity, v.spans[2].entity);
        // The inner `S` is owned by the scope `C`, and `S` is qualified by
        // the inner `ns`.
        assert_eq!(v.owner(3), Some(1));
        assert_eq!(v.qualifier(3), Some(2));
        // The function the string is printed for is the instantiation.
        assert!(named(ast, v.function, "f"));
        assert_eq!(v.spans[4].entity, Some(ast.first(v.function)));
        assert!(fn_names::verify(ast, v.bytes, v.spans).is_ok());
    });
}

/// A default argument's `__builtin_FUNCTION()` is made per use
/// ([expr.const]/16, WP-3279): two callers give two strings and two
/// records, each naming its caller.
#[test]
fn per_use_default_argument_names_each_caller_support_srcloc() {
    let src = "\
const char *deflt(const char *s = __builtin_FUNCTION()) { return s; }
void a1() { deflt(); }
void a2() { deflt(); }
";
    parse(src, |ast| {
        let uses = exprs_of(ast, ExprKind::CXXDefaultArgExpr);
        let mut seen = Vec::new();
        for u in uses {
            let v = ast.fn_name_at_use(u, FnNameAt::Function).expect("a record at the use");
            assert_eq!(entries(&v), [(FnNameRole::Name, bytes(&v), 0)]);
            assert_eq!(v.spans[0].entity, Some(ast.first(v.function)));
            seen.push(bytes(&v));
            // No `__impl` object at this use.
            assert!(ast.fn_name_at_use(u, FnNameAt::Location).is_none());
        }
        seen.sort();
        assert_eq!(seen, ["a1", "a2"]);
        assert_eq!(v_len(ast, "a1") + v_len(ast, "a2"), 2);
    });
}

/// The number of records printed for function `name`.
fn v_len(ast: &Ast, name: &str) -> usize {
    let f = function(ast, name);
    ast.fn_names.iter(ast).filter(|r| r.function == f).count()
}

/// `std::source_location::current()`'s `__impl` object holds the pretty
/// name of the use's function ([support.srcloc.cons]/2), reached through
/// `FnNameAt::Location`.
#[test]
fn source_location_function_name_support_srcloc_cons_2() {
    let src = format!(
        "{SOURCE_LOCATION}\
namespace ns {{ struct S {{}}; void b1(S) {{ auto l = std::source_location::current(); (void)l; }} }}
"
    );
    parse(&src, |ast| {
        let b1 = function(ast, "b1");
        let v = exprs_of(ast, ExprKind::CXXDefaultArgExpr)
            .into_iter()
            .filter(|&u| ast.exprs.source_loc_values(u).is_some_and(|x| x.ctx == b1))
            .find_map(|u| ast.fn_name_at_use(u, FnNameAt::Location))
            .expect("the __impl object's function name");
        assert_eq!(bytes(&v), "void ns::b1(S)");
        let got = entries(&v);
        assert_eq!(got[0], (FnNameRole::Scope, "ns".to_owned(), 0));
        assert_eq!(got[1], (FnNameRole::Name, "b1".to_owned(), 0));
        assert_eq!(got[2], (FnNameRole::Param, "S".to_owned(), 0));
    });
}

/// [temp.deduct]: `ovl(1)` and `ovl(1L)` are two specializations, two
/// strings, two records; each binding's argument is its own type.
#[test]
fn two_deductions_two_records_temp_deduct() {
    let src = "\
template <class T> void ovl(T) { (void)__PRETTY_FUNCTION__; }
void u() { ovl(1); ovl(1L); }
";
    parse(src, |ast| {
        let a = predefined(ast, "void ovl(T) [T = int]");
        let b = predefined(ast, "void ovl(T) [T = long]");
        assert_ne!(a.function, b.function);
        assert_eq!(a.spans.len(), 3, "name, param, binding: `int` is a keyword");
        assert_eq!(a.spans[1].entity, b.spans[1].entity, "one pattern parameter");
        assert_eq!(ast.fn_names.len(), 2);
    });
}

/// R3: a constructor's and a destructor's class name (`CLASS_NAME`, the
/// `~` excluded), an operator in one entry, a conversion function's
/// target nested one depth in.
#[test]
fn special_members_and_operators_class_ctor_class_dtor() {
    let src = "\
namespace ns {
struct S {};
struct K {
  K() { (void)__PRETTY_FUNCTION__; }
  ~K() { (void)__FUNCTION__; }
  bool operator==(const K &) const { (void)__func__; return true; }
  operator S() { (void)__PRETTY_FUNCTION__; return {}; }
};
}
void u() { ns::K k, j; (void)(k == j); (void)static_cast<ns::S>(k); }
";
    parse(src, |ast| {
        let ctor = predefined(ast, "ns::K::K()");
        assert_eq!(ctor.spans[2].flags & fn_names::CLASS_NAME, fn_names::CLASS_NAME);
        assert_eq!(entries(&ctor)[2].1, "K");
        let dtor = predefined(ast, "~K");
        assert_eq!(entries(&dtor), [(FnNameRole::Name, "K".to_owned(), 0)]);
        assert_eq!(dtor.spans[0].lo, 1);
        let op = predefined(ast, "operator==");
        assert_eq!(entries(&op), [(FnNameRole::Name, "operator==".to_owned(), 0)]);
        // clang prints a class target by its bare name.
        let conv = predefined(ast, "S ns::K::operator S()");
        let got = entries(&conv);
        assert_eq!(got[0], (FnNameRole::Return, "S".to_owned(), 0));
        assert_eq!(got[3], (FnNameRole::Name, "operator S".to_owned(), 0));
        assert_eq!(got[4], (FnNameRole::Name, "S".to_owned(), 1));
        assert_eq!(conv.owner(4), Some(3));
        assert_eq!(conv.spans[4].entity, conv.spans[0].entity);
    });
}

/// R8: an abbreviated template's invented parameter prints `x:auto`; its
/// binding covers only `x` and names the function parameter, `INVENTED`.
#[test]
fn invented_parameter_binding_dcl_fct_22() {
    let src = "\
namespace ns { struct S {}; void inv(auto x) { (void)__PRETTY_FUNCTION__; } }
void u() { ns::inv(ns::S{}); }
";
    parse(src, |ast| {
        let v = predefined(ast, "void ns::inv(auto) [x:auto = ns::S]");
        let i = v.spans.iter().position(|s| s.role == FnNameRole::Binding).expect("a binding");
        assert_eq!(v.text(i), b"x");
        assert_eq!(v.spans[i].flags & fn_names::INVENTED, fn_names::INVENTED);
        let p = v.spans[i].entity.expect("the parameter");
        assert_eq!(ast.decls.get(p).kind(), DeclKind::ParmVar);
    });
}

/// A contract site's string goes through the same funnel; the record is
/// reached by `get(lit, function)` ([basic.contract.eval]).
#[test]
fn contract_site_string_through_get_basic_contract_eval() {
    let src = "\
namespace ns { int k(int x) pre(x > 0) { return x; } }
int u() { return ns::k(1); }
";
    parse(src, |ast| {
        let k = function(ast, "k");
        let r = ast.fn_names.iter(ast).find(|r| r.function == k).expect("the contract's record");
        let spans = ast.fn_names.get(r.lit, r.function).expect("get");
        assert_eq!(ast.strings.bytes(r.lit), b"int ns::k(int)");
        assert_eq!(spans.len(), 2);
    });
}

/// §24.5 cases 1 and 4: a literal the source spells has no record, and
/// an offset past the end of the string answers `None`; a pointer inside
/// it clips the entries and marks a cut one `partial` (the client
/// review's Q9).
#[test]
fn none_cases_and_clipping_lex_string() {
    let src = "\
namespace ns { void fun() { const char *p = \"fun\"; (void)p; (void)__func__; } }
";
    parse(src, |ast| {
        for e in exprs_of(ast, ExprKind::StringLiteral) {
            if ast.exprs.string_maker(e).is_none() && ast.fn_names.predefined_function(e).is_none()
            {
                let lit = ast.exprs.string_lit(e);
                assert!(ast.fn_name_of_string(lit, e, 0).is_none(), "a spelled literal");
            }
        }
        let v = predefined(ast, "fun");
        let e = exprs_of(ast, ExprKind::PredefinedExpr)
            .into_iter()
            .find(|&e| ast.fn_name_of_predefined(e).is_some())
            .expect("__func__");
        let sl = ast.exprs.predefined_name(e).expect("its literal");
        assert!(ast.fn_name_of_string(v.lit, sl, 3).is_some(), "one past the last byte");
        assert!(ast.fn_name_of_string(v.lit, sl, 4).is_none(), "past the end");
        let w = ast.fn_name_of_string(v.lit, sl, 1).expect("inside");
        let vis: Vec<_> = w.visible(Some(1)).collect();
        assert_eq!(vis.len(), 1);
        assert!(vis[0].partial);
        assert_eq!((vis[0].lo, vis[0].hi), (0, 1));
    });
}

/// Off: no record, every accessor answers `None`, and the memory report
/// has no `fn_names` row.
#[test]
fn off_records_nothing() {
    let src = "\
namespace ns { void fun() { (void)__PRETTY_FUNCTION__; } }
const char *deflt(const char *s = __builtin_FUNCTION()) { return s; }
void a1() { deflt(); }
";
    parse_with(src, false, |ast, _| {
        assert!(!ast.fn_names.is_on());
        assert!(ast.fn_names.is_empty());
        assert_eq!(ast.fn_names.entry_count(), 0);
        assert!(ast.fn_names.mem_row().is_none());
        for e in exprs_of(ast, ExprKind::PredefinedExpr) {
            assert!(ast.fn_name_of_predefined(e).is_none());
        }
        for u in exprs_of(ast, ExprKind::CXXDefaultArgExpr) {
            assert!(ast.fn_name_at_use(u, FnNameAt::Function).is_none());
        }
    });
}

/// The dump is ordered by the source alone (the client review's
/// requirement e): two parses give the same text.
#[test]
fn dump_is_deterministic() {
    let src = "\
namespace ns {
template <class T> void b(T) { (void)__PRETTY_FUNCTION__; }
template <class T> void a(T) { (void)__PRETTY_FUNCTION__; }
}
void u() { ns::b(1); ns::a('c'); ns::b(2.0); ns::a(1); }
";
    let one = parse_with(src, true, |ast, sm| ast.fn_names.dump(ast, sm, None));
    let two = parse_with(src, true, |ast, sm| ast.fn_names.dump(ast, sm, None));
    assert_eq!(one, two);
    let firsts: Vec<&str> = one.lines().filter(|l| l.starts_with("fn-name ")).collect();
    assert_eq!(firsts.len(), 4, "{one}");
    // `b`'s two records come first (b is declared first), by bytes.
    assert!(firsts[0].contains("[T = double]"), "{one}");
    assert!(firsts[1].contains("[T = int]"), "{one}");
}

/// R4, R9: a closure or an unnamed enum as a template argument is one
/// synthesized entry over its whole `(lambda at F:L:C)` /
/// `(unnamed enum at F:L:C)` spelling, path included, naming the closure
/// class or the enum (the client review's Q3: one entry, no sub-ranges).
#[test]
fn unnamed_spellings_with_locations_are_one_entry() {
    let src = "\
namespace ns {
template <class F> struct H { static void run() { (void)__PRETTY_FUNCTION__; } };
enum { e0 };
template <class T> void ty(T) { (void)__PRETTY_FUNCTION__; }
void loc() { auto k = [] {}; H<decltype(k)>::run(); ty(e0); }
}
";
    parse(src, |ast| {
        let views: Vec<FnNameView<'_>> = exprs_of(ast, ExprKind::PredefinedExpr)
            .into_iter()
            .filter_map(|e| ast.fn_name_of_predefined(e))
            .collect();
        let mut seen = 0;
        for v in &views {
            for (i, s) in v.spans.iter().enumerate() {
                let t = String::from_utf8(v.text(i).to_vec()).expect("UTF-8");
                if t.starts_with("(lambda at t.cpp:5:23)")
                    || t.starts_with("(unnamed enum at t.cpp:3:1)")
                {
                    assert_eq!(s.flags & fn_names::SYNTHESIZED, fn_names::SYNTHESIZED, "{t}");
                    let d = s.entity.expect("an unnamed entity");
                    let k = ast.decls.get(d).kind();
                    assert!(matches!(k, DeclKind::CXXRecord | DeclKind::Enum), "{k:?}");
                    seen += 1;
                }
            }
        }
        // `H<(lambda ...)>::run`: in the scope and in the bracketed list;
        // `ty<(unnamed enum ...)>`: in the bracketed list.
        assert_eq!(seen, 3, "{:?}", views.iter().map(bytes).collect::<Vec<_>>());
    });
}
