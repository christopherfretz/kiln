//! Unit tests for the canonical expression profiler (`Ast::profile_expr`,
//! spec/types.md §7.2): equivalence of expressions involving template
//! parameters ([temp.over.link]/5).

mod common;

use common::{DEPENDENT, INT, builtin, new_ast};
use kiln_ast::decl::{DeclKind, FnSpec, ParamData};
use kiln_ast::expr::{BinaryOp, ExprFlags, NonOdrUse, Radix, TraitKind, UnaryOp};
use kiln_ast::names::DeclName;
use kiln_ast::{Ast, DeclHead, DeclId, ExprId, QualType};
use kiln_source::Loc;

const L: Loc = Loc::INVALID;

fn head(ast: &Ast, s: &str) -> DeclHead {
    DeclHead::new(ast.tu, DeclName::ident(ast.symbols().intern(s)), L)
}

/// `int <name>` as a non-type template parameter at (`depth`, `index`).
fn nttp(ast: &mut Ast, name: &str, depth: u16, index: u16) -> DeclId {
    let h = head(ast, name);
    ast.builder().non_type_template_parm(h, builtin(INT), depth, index, false)
}

/// `class <name>` at (`depth`, `index`); returns the decl and its type.
fn ttp(ast: &mut Ast, name: &str, depth: u16, index: u16) -> (DeclId, QualType) {
    let h = head(ast, name);
    let d = ast.builder().template_type_parm(h, depth, index, false, false);
    (d, ast.decls.get(d).ty().expect("parm type"))
}

fn dref(ast: &mut Ast, d: DeclId) -> ExprId {
    let ty = ast.decls.get(d).ty().unwrap_or(builtin(DEPENDENT));
    ast.builder().decl_ref(d, ty, L, None, NonOdrUse::None, ExprFlags::NONE)
}

fn lit(ast: &mut Ast, v: u64) -> ExprId {
    ast.builder().int_literal(builtin(INT), v, Radix::Decimal, L)
}

/// `<parm> + <v>`.
fn plus(ast: &mut Ast, d: DeclId, v: u64) -> ExprId {
    let (l, r) = (dref(ast, d), lit(ast, v));
    ast.builder().dependent_binary(BinaryOp::Add, l, r, L)
}

fn sizeof_ty(ast: &mut Ast, t: QualType) -> ExprId {
    ast.builder().trait_of_type(TraitKind::SizeOf, t, builtin(INT), L, L)
}

/// A function parameter of type `ty` at `index` of a fresh function.
fn fn_param(ast: &mut Ast, fname: &str, ty: QualType, index: u16, flags: u16) -> DeclId {
    let h = head(ast, fname);
    let f = ast.builder().function(h, DeclKind::Function, builtin(DEPENDENT), FnSpec::new(), None);
    let n = DeclName::ident(ast.symbols().intern("p"));
    ast.builder().param(f, n, L, ty, index, flags)
}

/// The same expression profiled twice gets the same profile, and two
/// separately built but identical expressions get it too.
#[test]
fn same_expression_twice_is_equal() {
    let mut ast = new_ast();
    let n = nttp(&mut ast, "N", 0, 0);
    let e = plus(&mut ast, n, 1);
    let e2 = plus(&mut ast, n, 1);
    let p = ast.profile_expr(e);
    assert_eq!(ast.profile_expr(e), p);
    assert_eq!(ast.profile_expr(e2), p);
    assert!(ast.exprs_equivalent(e, e2));
    // The first expression is kept for printing.
    assert_eq!(ast.types.profile_expr(p), Some(e));
}

/// [temp.over.link]/5: `N + 1` and `N + 2` are not equivalent.
#[test]
fn different_literals_differ() {
    let mut ast = new_ast();
    let n = nttp(&mut ast, "N", 0, 0);
    let (e1, e2) = (plus(&mut ast, n, 1), plus(&mut ast, n, 2));
    assert_ne!(ast.profile_expr(e1), ast.profile_expr(e2));
    assert!(!ast.exprs_equivalent(e1, e2));
}

/// Different operators differ: `N + 1` vs `N - 1`.
#[test]
fn different_operators_differ() {
    let mut ast = new_ast();
    let n = nttp(&mut ast, "N", 0, 0);
    let e1 = plus(&mut ast, n, 1);
    let (l, r) = (dref(&mut ast, n), lit(&mut ast, 1));
    let e2 = ast.builder().dependent_binary(BinaryOp::Sub, l, r, L);
    assert!(!ast.exprs_equivalent(e1, e2));
}

/// Operand order matters: `N + 1` vs `1 + N` (functionally equivalent
/// only, [temp.over.link]/6, so not equivalent).
#[test]
fn operand_order_matters() {
    let mut ast = new_ast();
    let n = nttp(&mut ast, "N", 0, 0);
    let e1 = plus(&mut ast, n, 1);
    let (l, r) = (lit(&mut ast, 1), dref(&mut ast, n));
    let e2 = ast.builder().dependent_binary(BinaryOp::Add, l, r, L);
    assert!(!ast.exprs_equivalent(e1, e2));
}

/// `sizeof(T)` and `sizeof(U)` differ when `T`, `U` are different
/// parameters, and agree when they are the same position.
#[test]
fn sizeof_of_different_parameters_differs() {
    let mut ast = new_ast();
    let (_, t) = ttp(&mut ast, "T", 0, 0);
    let (_, u) = ttp(&mut ast, "U", 0, 1);
    let (_, t2) = ttp(&mut ast, "T2", 0, 0);
    let (st, su, st2) = (sizeof_ty(&mut ast, t), sizeof_ty(&mut ast, u), sizeof_ty(&mut ast, t2));
    assert_ne!(ast.profile_expr(st), ast.profile_expr(su));
    assert_eq!(ast.profile_expr(st), ast.profile_expr(st2));
}

/// Template parameters compare by (depth, index), not by declaration or
/// name: `N + 1` in one template head and `M + 1` in another are
/// equivalent; a parameter at another depth or index is not.
#[test]
fn template_parameters_compare_by_position() {
    let mut ast = new_ast();
    let n = nttp(&mut ast, "N", 0, 0);
    let m = nttp(&mut ast, "M", 0, 0);
    let deeper = nttp(&mut ast, "N", 1, 0);
    let second = nttp(&mut ast, "K", 0, 1);
    let en = plus(&mut ast, n, 1);
    let em = plus(&mut ast, m, 1);
    let ed = plus(&mut ast, deeper, 1);
    let es = plus(&mut ast, second, 1);
    assert_eq!(ast.profile_expr(en), ast.profile_expr(em));
    assert_ne!(ast.profile_expr(en), ast.profile_expr(ed));
    assert_ne!(ast.profile_expr(en), ast.profile_expr(es));
}

/// Function parameters compare by (index, pack, type): `decltype(x + 1)`
/// in a redeclaration naming the parameter `y` is equivalent.
#[test]
fn function_parameters_compare_by_index() {
    let mut ast = new_ast();
    let (_, t) = ttp(&mut ast, "T", 0, 0);
    let x = fn_param(&mut ast, "f", t, 0, 0);
    let y = fn_param(&mut ast, "f", t, 0, 0);
    let z = fn_param(&mut ast, "f", t, 1, 0);
    let pk = fn_param(&mut ast, "f", t, 0, ParamData::PACK);
    let ex = plus(&mut ast, x, 1);
    let ey = plus(&mut ast, y, 1);
    let ez = plus(&mut ast, z, 1);
    let ep = plus(&mut ast, pk, 1);
    assert!(ast.exprs_equivalent(ex, ey));
    assert!(!ast.exprs_equivalent(ex, ez));
    assert!(!ast.exprs_equivalent(ex, ep));
}

/// Parentheses are kept ([temp.over.link] follows the one-definition
/// rule's token comparison): `(N)` differs from `N`.
#[test]
fn parentheses_are_kept() {
    let mut ast = new_ast();
    let n = nttp(&mut ast, "N", 0, 0);
    let e = dref(&mut ast, n);
    let e2 = dref(&mut ast, n);
    let p = ast.builder().paren(e2, L, L);
    assert!(!ast.exprs_equivalent(e, p));
}

/// Unary operators carry their opcode.
#[test]
fn unary_opcode_matters() {
    let mut ast = new_ast();
    let n = nttp(&mut ast, "N", 0, 0);
    let (a, b) = (dref(&mut ast, n), dref(&mut ast, n));
    let neg = ast.builder().dependent_unary(UnaryOp::Minus, a, L);
    let not = ast.builder().dependent_unary(UnaryOp::Not, b, L);
    assert!(!ast.exprs_equivalent(neg, not));
}

/// The profile records the expression's dependence.
#[test]
fn profile_records_dependence() {
    use kiln_ast::types::TypeFlags;
    let mut ast = new_ast();
    let n = nttp(&mut ast, "N", 0, 0);
    let e = plus(&mut ast, n, 1);
    let p = ast.profile_expr(e);
    assert!(ast.types.profile_flags(p).contains(TypeFlags::DEPENDENT));
}

/// A deeply nested expression is profiled without recursion.
#[test]
fn deep_nesting_does_not_overflow() {
    let mut ast = new_ast();
    let n = nttp(&mut ast, "N", 0, 0);
    let mut e = dref(&mut ast, n);
    for _ in 0..200_000 {
        let one = lit(&mut ast, 1);
        e = ast.builder().dependent_binary(BinaryOp::Add, e, one, L);
    }
    let p = ast.profile_expr(e);
    assert_eq!(ast.profile_expr(e), p);
}
