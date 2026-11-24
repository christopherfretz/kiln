//! The value of an immediate invocation of a default argument or default
//! member initializer at each use (spec/ast.md §5.8.2, WP-3278, WP-3279):
//! CWG 2631 ([expr.const]/16, [dcl.fct.default]/5, [class.base.init]/9)
//! evaluates it at each use, in the context of that use, so
//! `len(__builtin_FUNCTION())` as a default argument is the length of the
//! caller's name. `Ast::default_use_immediate_value` reads the value of a
//! use without evaluating anything; the invocation's own `ConstantExpr`
//! (shared by every use) holds no value that pretends to be a use's.

use std::sync::Arc;

use kiln_ast::consts::Scalar;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

const SRC: &str = "\
consteval unsigned len(const char *s) { unsigned n = 0; while (s[n]) ++n; return n; }
consteval int id(int x) { return x; }
void log(unsigned n = len(__builtin_FUNCTION()));
void plain(int k = id(7));
unsigned inner(unsigned n = len(__builtin_FUNCTION()));
void outer(unsigned m = inner());
struct A { unsigned n = len(__builtin_FUNCTION()); };
void f() { log(); plain(); }
void ghij() { log(); plain(); outer(); }
void abcdefg() { A a{}; }
";

fn parse(f: impl FnOnce(&Ast)) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", SRC.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let diags = sink.take();
    assert!(diags.is_empty(), "{diags:?}");
    f(&tu.ast);
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

/// The only parameter of function `name`.
fn param(ast: &Ast, name: &str) -> DeclId {
    let f = function(ast, name);
    let p = ast.decls.function(f).expect("a function").params;
    ast.lists.at::<DeclId>(p, 0).expect("a parameter")
}

/// Every expression of kind `k`, in creation order.
fn exprs_of(ast: &Ast, k: ExprKind) -> Vec<ExprId> {
    (1..=ast.exprs.len() as u32)
        .map(ExprId::new)
        .filter(|&e| ast.exprs.get(e).kind() == k)
        .collect()
}

/// The uses of parameter `p`'s default argument made in function `ctx`.
fn uses_of(ast: &Ast, p: DeclId, ctx: DeclId) -> Vec<ExprId> {
    exprs_of(ast, ExprKind::CXXDefaultArgExpr)
        .into_iter()
        .filter(|&u| {
            ast.exprs.default_arg_param(u) == p
                && ast.exprs.source_loc_values(u).is_none_or(|v| v.ctx == ctx)
        })
        .collect()
}

/// The immediate invocation (the outermost `ConstantExpr`) of `p`'s
/// default argument.
fn invocation_of(ast: &Ast, p: DeclId) -> ExprId {
    let mut e = ast.decls.param(p).and_then(|x| x.default_arg).expect("a default argument");
    loop {
        match ast.exprs.get(e).kind() {
            ExprKind::ConstantExpr => return e,
            ExprKind::ExprWithCleanups | ExprKind::ImplicitCastExpr => e = ast.exprs.operand(e),
            k => panic!("no invocation: {k:?}"),
        }
    }
}

fn int(ast: &Ast, v: ConstValueId) -> u128 {
    match ast.consts.scalar(v) {
        Some(Scalar::Int { bits, .. }) => bits.to_u128().expect("narrow"),
        other => panic!("not an integer: {other:?}"),
    }
}

/// CWG 2631: `log()` in `f` and in `ghij` each evaluate `len(...)` in
/// their own context, and each use keeps its own value; the shared
/// `ConstantExpr` holds neither (it is not the declaration's evaluation
/// with an empty name either).
#[test]
fn each_use_keeps_its_value_expr_const_16() {
    parse(|ast| {
        let p = param(ast, "log");
        let c = invocation_of(ast, p);
        assert_eq!(ast.exprs.constant_value(c), None);
        assert_eq!(ast.exprs.default_immediate_value(c), None, "no shared value pretends");
        for (name, want) in [("f", 1), ("ghij", 4)] {
            let ctx = function(ast, name);
            let us = uses_of(ast, p, ctx);
            assert_eq!(us.len(), 1, "{name}");
            let u = us[0];
            let v = ast.default_use_immediate_value(u, c).expect("a value at the use");
            assert_eq!(int(ast, v), want, "{name}");
            let rec = ast.exprs.use_immediates(u);
            assert_eq!(rec.len(), 1);
            assert_eq!(rec[0].invocation, c);
            assert_eq!(ast.exprs.use_immediate(u, c), Some(v));
        }
    });
}

/// A use without source-location values shares the one value every use
/// computes (`default_immediate_value`) and keeps no record of its own.
#[test]
fn context_free_uses_share_one_value_dcl_fct_default_5() {
    parse(|ast| {
        let p = param(ast, "plain");
        let c = invocation_of(ast, p);
        let us: Vec<ExprId> = exprs_of(ast, ExprKind::CXXDefaultArgExpr)
            .into_iter()
            .filter(|&u| ast.exprs.default_arg_param(u) == p)
            .collect();
        assert_eq!(us.len(), 2);
        for u in us {
            assert!(ast.exprs.use_immediates(u).is_empty());
            let v = ast.default_use_immediate_value(u, c).expect("the shared value");
            assert_eq!(int(ast, v), 7);
        }
        assert!(ast.exprs.default_immediate_value(c).is_some());
    });
}

/// A default argument used inside another one is evaluated in the context
/// of the outermost use: `outer()` in `ghij` records `inner`'s invocation
/// with `"ghij"`'s length on its own use.
#[test]
fn nested_default_argument_is_the_outer_uses_expr_const_16() {
    parse(|ast| {
        let p = param(ast, "outer");
        let c = invocation_of(ast, param(ast, "inner"));
        let us = uses_of(ast, p, function(ast, "ghij"));
        assert_eq!(us.len(), 1);
        let v = ast.default_use_immediate_value(us[0], c).expect("a value at the outer use");
        assert_eq!(int(ast, v), 4);
        assert_eq!(ast.exprs.default_immediate_value(c), None);
    });
}

/// [class.base.init]/9, [dcl.init.aggr]/5: a default member initializer's
/// invocation in an aggregate initialization takes the function holding
/// it.
#[test]
fn default_member_initializer_use_class_base_init_9() {
    parse(|ast| {
        let uses = exprs_of(ast, ExprKind::CXXDefaultInitExpr);
        let ctx = function(ast, "abcdefg");
        let u = uses
            .into_iter()
            .find(|&u| ast.exprs.source_loc_values(u).is_some_and(|v| v.ctx == ctx))
            .expect("the aggregate initialization's use");
        let rec = ast.exprs.use_immediates(u);
        assert_eq!(rec.len(), 1);
        assert_eq!(int(ast, rec[0].value), 7);
        let f = ast.exprs.default_init_field(u);
        assert!(named(ast, f, "n"));
        assert_eq!(ast.default_use_immediate_value(u, rec[0].invocation), Some(rec[0].value));
    });
}
