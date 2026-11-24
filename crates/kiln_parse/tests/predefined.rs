//! A `PredefinedExpr` (`__func__`, `__FUNCTION__`,
//! `__PRETTY_FUNCTION__`, [dcl.fct.def.general]/8) records the function
//! it names, read through `ExprArena::predefined_function` (spec/ast.md
//! §16, WP-2823): the innermost enclosing function as clang's
//! `getPredefinedExprDecl` finds it. clang 19 and 22 agree on each case
//! here (`-ast-dump`): a lambda's `__func__` is `operator()`, a
//! constructor's member initializer names the constructor, a coroutine
//! names itself; at namespace scope, in a default argument, in a default
//! member initializer and in a template head's requires-clause there is
//! no function (clang warns and names the translation unit). In a
//! template pattern the node is dependent and names the pattern's
//! function; an instantiation's names the specialization.

use std::sync::Arc;

use kiln_ast::dump::AstNames;
use kiln_ast::expr::{ExprKind, PredefinedKind};
use kiln_ast::{Ast, DeclId, ExprId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// A minimal coroutine library: just what a `co_return` needs.
const CORO: &str = "\
namespace std {
template <class R, class...> struct coroutine_traits { using promise_type = typename R::promise_type; };
template <class P = void> struct coroutine_handle {
  void* p;
  static coroutine_handle from_address(void* a) noexcept { return {a}; }
  void* address() const noexcept { return p; }
};
struct suspend_never {
  bool await_ready() const noexcept { return true; }
  template <class H> void await_suspend(H) const noexcept {}
  void await_resume() const noexcept {}
};
}
struct task {
  struct promise_type {
    task get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};
";

/// One case per line, from line 23 (after `CORO`).
const SRC: &str = "\
const char *ns = __func__;
void f() { const char *a = __func__; }
auto l = [] { return __FUNCTION__; };
void g(const char *s = __func__);
struct C { const char *m; C() : m(__PRETTY_FUNCTION__) {} };
template <class T> void t() { const char *a = __FUNCTION__; }
template void t<int>();
template <class T> requires (sizeof(__func__) > 0) void r() {}
void h() { auto k = [] { return __func__; }; }
task co() { const char *a = __func__; co_return; }
struct D { const char *p = __func__; };
void gl() { auto k = [](auto x) { return __func__; }; k(1); }
template <class T> struct M { const char *f() { return __func__; } };
const char *use = M<int>().f();
";

/// The line of `CORO` + `SRC` where `SRC` begins.
const FIRST: u32 = 23;

/// Parses the cases and describes each `PredefinedExpr` in creation
/// order as `L kind -> name (how)`, checking on the way that a
/// non-dependent node's literal is the recorded function's name.
fn describe() -> Vec<String> {
    let sm = SourceManager::new();
    let src = format!("{CORO}{SRC}");
    let file = sm.add_virtual("t.cpp", src).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let ast = &tu.ast;
    let names = AstNames::new(ast);
    let mut out = Vec::new();
    for i in 1..=ast.exprs.len() as u32 {
        let e = ExprId::new(i);
        let n = ast.exprs.get(e);
        if n.kind() != ExprKind::PredefinedExpr {
            continue;
        }
        let line = sm.line_col(n.loc()).expect("a file location").line - FIRST + 1;
        let kind = ast.exprs.predefined_kind(e).spelling();
        let func = ast.exprs.predefined_function(e);
        let lit = ast.exprs.predefined_name(e);
        let Some(f) = func else {
            assert_eq!(lit.map(|s| string(ast, s)), Some(String::new()), "line {line}");
            out.push(format!("{line} {kind} -> none"));
            continue;
        };
        assert!(ast.decls.get(f).kind().is_function(), "line {line}");
        let mut name = String::new();
        names.write_name(f, &mut name);
        if let Some(s) = lit
            && ast.exprs.predefined_kind(e) != PredefinedKind::PrettyFunction
        {
            assert_eq!(string(ast, s), name, "line {line}: the literal names the function");
        }
        out.push(format!("{line} {kind} -> {name}{}", how(ast, f, lit.is_none())));
    }
    out
}

/// The bytes of string literal `s`.
fn string(ast: &Ast, s: ExprId) -> String {
    String::from_utf8_lossy(ast.strings.bytes(ast.exprs.string_lit(s))).into_owned()
}

/// How function `f` relates to what was written: a lambda's call
/// operator, a template's pattern (the node `dependent`), a
/// specialization instantiated from the pattern on another line.
fn how(ast: &Ast, f: DeclId, dependent: bool) -> String {
    let mut s = String::new();
    let parent = ast.decls.get(f).ctx();
    if parent.is_some_and(|p| ast.closure_lambda(p).is_some()) {
        s.push_str(" lambda");
    }
    if dependent {
        s.push_str(" pattern");
    }
    if let Some(p) = ast.instantiated_from(f)
        && p != f
    {
        s.push_str(" spec");
    }
    s
}

#[test]
fn each_predefined_expr_records_its_function() {
    let got = describe();
    let want = [
        "1 __func__ -> none",
        "2 __func__ -> f",
        "3 __FUNCTION__ -> operator() lambda",
        "4 __func__ -> none",
        "5 __PRETTY_FUNCTION__ -> C",
        "6 __FUNCTION__ -> t pattern",
        "6 __FUNCTION__ -> t spec",
        "8 __func__ -> none",
        "9 __func__ -> operator() lambda",
        "10 __func__ -> co",
        "11 __func__ -> none",
        "12 __func__ -> operator() lambda pattern",
        "12 __func__ -> operator() lambda spec",
        "13 __func__ -> f pattern",
        "13 __func__ -> f spec",
    ];
    assert_eq!(got, want, "{}", got.join("\n"));
}
