//! `AstBuilder` tests (spec/ast.md §14 `builder.rs`): dependence per
//! expression kind, value categories, redeclaration groups, lookup for
//! overload sets, transparent contexts and inline namespaces, P2169 `_`,
//! write-once slots and the memory report. Each test names the standard
//! section it checks.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "short names mirror the C++ snippets under test"
)]

mod common;

use common::{BOOL, DEPENDENT, ERROR, INT, builtin, const_builtin, new_ast};
use kiln_ast::decl::{
    DeclFlags, DeclKind, DeclMisc, DefinitionBits, EnumData, FieldData, FnSpec, LinkageLang,
    TagKind, VarSpec,
};
use kiln_ast::expr::{
    BinaryOp, CastKind, CharKind, ExprDeps, ExprFlags, ExprKind, NonOdrUse, ObjectKind, Radix,
    StringKind, TraitKind, UnaryOp, ValueCategory,
};
use kiln_ast::names::DeclName;
use kiln_ast::stmt::{IfData, IfKind, LoopData, StmtKind};
use kiln_ast::{Ast, DeclHead, DeclId, ExprId, StringLitId};
use kiln_source::Loc;

const L: Loc = Loc::INVALID;

fn name(ast: &Ast, s: &str) -> DeclName {
    DeclName::ident(ast.symbols().intern(s))
}

fn head(ast: &Ast, ctx: DeclId, s: &str) -> DeclHead {
    DeclHead::new(ctx, name(ast, s), L)
}

fn int_var(ast: &mut Ast, ctx: DeclId, s: &str) -> DeclId {
    let h = head(ast, ctx, s);
    ast.builder().var(h, builtin(INT), VarSpec::new(), None)
}

fn lit(ast: &mut Ast, v: u64) -> ExprId {
    ast.builder().int_literal(builtin(INT), v, Radix::Decimal, L)
}

/// A `ParmVar` standing in for a value-dependent operand: its type is the
/// `Dependent` placeholder.
fn dependent_ref(ast: &mut Ast) -> ExprId {
    let tu = ast.tu;
    let f = {
        let h = head(ast, tu, "tmpl_fn");
        ast.builder().function(h, DeclKind::Function, builtin(DEPENDENT), FnSpec::new(), None)
    };
    let n = name(ast, "t");
    let p = ast.builder().param(f, n, L, builtin(DEPENDENT), 0, 0);
    ast.builder().decl_ref(p, builtin(DEPENDENT), L, None, NonOdrUse::None, ExprFlags::NONE)
}

// ---------------------------------------------------------------------------
// Dependence (§5.1)
// ---------------------------------------------------------------------------

/// [temp.dep.expr]/1, [temp.dep.constexpr]/1: literals are never
/// dependent.
#[test]
fn literals_are_not_dependent_temp_dep_expr() {
    let mut ast = new_ast();
    let mut b = ast.builder();
    let es = [
        b.int_literal(builtin(INT), 1, Radix::Hex, L),
        b.char_literal(builtin(INT), CharKind::Ascii, 'a' as u32, L),
        b.bool_literal(builtin(BOOL), true, L),
        b.nullptr_literal(builtin(INT), L),
        b.string_literal(builtin(INT), StringKind::Ordinary, StringLitId::new(1), L),
    ];
    for e in es {
        assert!(ast.exprs.get(e).deps().is_none(), "{:?}", ast.exprs.get(e).kind());
    }
}

/// [temp.dep.expr]/3: an id-expression whose type is dependent is
/// type-dependent (and value- and instantiation-dependent).
#[test]
fn decl_ref_with_dependent_type_temp_dep_expr_3() {
    let mut ast = new_ast();
    let e = dependent_ref(&mut ast);
    let n = ast.exprs.get(e);
    assert!(n.deps().contains(ExprDeps::TYPE_VALUE_INST));
    assert_eq!(n.value_category(), ValueCategory::LValue);
}

/// [temp.dep.constexpr]/2.2: a reference to a constant variable whose
/// initializer is value-dependent is value-dependent; a non-const variable
/// is not.
#[test]
fn decl_ref_to_const_var_with_dependent_init_temp_dep_constexpr_2() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let dep = dependent_ref(&mut ast);
    let h = head(&ast, tu, "c");
    let c = ast.builder().var(h, const_builtin(INT), VarSpec::new(), None);
    ast.builder().set_var_init(c, dep).unwrap();
    let h = head(&ast, tu, "m");
    let m = ast.builder().var(h, builtin(INT), VarSpec::new(), None);
    ast.builder().set_var_init(m, dep).unwrap();
    let h = head(&ast, tu, "k");
    let k = ast.builder().var(h, builtin(INT), VarSpec::new().with_constexpr(true), None);
    ast.builder().set_var_init(k, dep).unwrap();

    let mut b = ast.builder();
    let rc = b.decl_ref(c, builtin(INT), L, None, NonOdrUse::None, ExprFlags::NONE);
    let rm = b.decl_ref(m, builtin(INT), L, None, NonOdrUse::None, ExprFlags::NONE);
    let rk = b.decl_ref(k, builtin(INT), L, None, NonOdrUse::None, ExprFlags::NONE);
    assert_eq!(ast.exprs.get(rc).deps(), ExprDeps::VALUE_INST);
    assert_eq!(ast.exprs.get(rk).deps(), ExprDeps::VALUE_INST);
    assert!(ast.exprs.get(rm).deps().is_none());
}

/// [temp.dep.expr]/1, [temp.dep.constexpr]/1: operators are dependent if
/// an operand is.
#[test]
fn operators_propagate_dependence_temp_dep_expr_1() {
    let mut ast = new_ast();
    let dep = dependent_ref(&mut ast);
    let one = lit(&mut ast, 1);
    let mut b = ast.builder();
    let add = b.binary(BinaryOp::Add, dep, one, builtin(DEPENDENT), L);
    let neg = b.unary(UnaryOp::Minus, add, builtin(DEPENDENT), L, ExprFlags::NONE);
    let paren = b.paren(neg, L, L);
    let call =
        b.call(paren, &[one], builtin(DEPENDENT), ValueCategory::PRValue, L, L, ExprFlags::NONE);
    let cond = b.conditional(one, call, one, builtin(DEPENDENT), L);
    let il = b.init_list(builtin(INT), &[one, cond], None, None, L, L);
    for e in [add, neg, paren, call, cond, il] {
        assert!(
            ast.exprs.get(e).deps().contains(ExprDeps::VALUE_INST),
            "{:?}",
            ast.exprs.get(e).kind()
        );
    }
    assert!(ast.exprs.get(cond).is_type_dependent());
    let plain = {
        let mut b = ast.builder();
        b.binary(BinaryOp::Mul, one, one, builtin(INT), L)
    };
    assert!(ast.exprs.get(plain).deps().is_none());
}

/// [temp.dep.expr]/3 (cast to a non-dependent type is not
/// type-dependent), [temp.dep.constexpr]/2 (but is value-dependent if its
/// operand is).
#[test]
fn casts_drop_type_dependence_temp_dep_expr_3() {
    let mut ast = new_ast();
    let dep = dependent_ref(&mut ast);
    let one = lit(&mut ast, 1);
    let mut b = ast.builder();
    let c = b.cast(
        ExprKind::CStyleCastExpr,
        CastKind::NoOp,
        dep,
        builtin(INT),
        ValueCategory::PRValue,
        None,
        L,
        ExprFlags::NONE,
    );
    assert_eq!(ast.exprs.get(c).deps(), ExprDeps::VALUE_INST);
    let mut b = ast.builder();
    let d = b.cast(
        ExprKind::CStyleCastExpr,
        CastKind::Dependent,
        one,
        builtin(DEPENDENT),
        ValueCategory::PRValue,
        None,
        L,
        ExprFlags::NONE,
    );
    assert_eq!(ast.exprs.get(d).deps(), ExprDeps::TYPE_VALUE_INST);
    let mut b = ast.builder();
    let i = b.implicit_cast(CastKind::IntegralCast, one, builtin(BOOL), ValueCategory::PRValue);
    assert!(ast.exprs.get(i).deps().is_none());
    assert_eq!(ast.exprs.cast_kind(i), CastKind::IntegralCast);
}

/// [temp.dep.constexpr]/3: `sizeof(T)` is value-dependent if `T` is
/// dependent; `sizeof e` if `e` is type-dependent. Neither is
/// type-dependent ([temp.dep.expr]/4).
#[test]
fn sizeof_is_value_dependent_only_temp_dep_constexpr_3() {
    let mut ast = new_ast();
    let dep = dependent_ref(&mut ast);
    let mut b = ast.builder();
    let st = b.trait_of_type(TraitKind::SizeOf, builtin(DEPENDENT), builtin(INT), L, L);
    let se = b.trait_of_expr(TraitKind::SizeOf, dep, builtin(INT), L);
    let si = b.trait_of_type(TraitKind::AlignOf, builtin(INT), builtin(INT), L, L);
    assert_eq!(ast.exprs.get(st).deps(), ExprDeps::VALUE_INST);
    assert_eq!(ast.exprs.get(se).deps(), ExprDeps::VALUE_INST);
    assert!(ast.exprs.get(si).deps().is_none());
    assert_eq!(ast.exprs.trait_type_arg(st), Some(builtin(DEPENDENT)));
    assert_eq!(ast.exprs.trait_type_arg(se), None);
    assert_eq!(ast.exprs.trait_kind(si), TraitKind::AlignOf);
}

/// spec/ast.md §5.3: a `RecoveryExpr` contains an error and is value-
/// dependent; without a type it is type-dependent; errors propagate to
/// parents and from the error type.
#[test]
fn recovery_and_error_propagation_spec_ast_5_3() {
    let mut ast = new_ast();
    let one = lit(&mut ast, 1);
    let mut b = ast.builder();
    let r = b.recovery(None, ValueCategory::PRValue, &[one], L);
    let typed = b.recovery(Some(builtin(INT)), ValueCategory::LValue, &[], L);
    let add = b.binary(BinaryOp::Add, r, one, builtin(INT), L);
    let err_ty = b.implicit_value_init(builtin(ERROR));
    assert!(ast.exprs.get(r).deps().contains(ExprDeps::ERROR));
    assert!(ast.exprs.get(r).deps().contains(ExprDeps::TYPE_VALUE_INST));
    assert!(!ast.exprs.get(typed).is_type_dependent());
    assert!(ast.exprs.get(typed).is_value_dependent());
    assert!(ast.exprs.get(add).deps().contains(ExprDeps::ERROR));
    assert_eq!(ast.exprs.get(err_ty).deps(), ExprDeps::ERROR);
    assert_eq!(ast.exprs.children(r, &ast.lists), vec![one]);
}

/// [expr.const]/13, spec/ast.md §5.5: `ConstantExpr` keeps its operand's
/// type, category and dependence; its value is write-once.
#[test]
fn constant_expr_value_once_expr_const() {
    let mut ast = new_ast();
    let five = lit(&mut ast, 5);
    let c = ast.builder().constant_expr(five, None);
    assert_eq!(ast.exprs.constant_value(c), None);
    let v = kiln_ast::ConstValueId::new(7);
    ast.exprs.set_constant_value(c, v).unwrap();
    assert_eq!(ast.exprs.constant_value(c), Some(v));
    assert!(ast.exprs.set_constant_value(c, v).is_err());
    assert_eq!(ast.exprs.int_value(five), Some(5));
}

/// [temp.dep.constexpr]: a value-dependent `ConstantExpr` never gets a
/// value.
#[test]
#[should_panic(expected = "value-dependent")]
fn constant_expr_value_dependent_panics_temp_dep_constexpr() {
    let mut ast = new_ast();
    let dep = dependent_ref(&mut ast);
    ast.builder().constant_expr(dep, Some(kiln_ast::ConstValueId::new(1)));
}

// ---------------------------------------------------------------------------
// Value categories (§5.2)
// ---------------------------------------------------------------------------

/// [expr.prim.literal]/1: a string literal is an lvalue, other literals
/// prvalues; [expr.prim.id.unqual]/3: variables, functions and data
/// members are lvalues, enumerators prvalues.
#[test]
fn primary_categories_expr_prim() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let v = int_var(&mut ast, tu, "v");
    let e = {
        let h = head(&ast, tu, "E");
        ast.builder().enumeration(h, 0, None, None)
    };
    ast.builder().begin_enum_definition(e).unwrap();
    let h = head(&ast, e, "A");
    let a = ast.builder().enumerator(h, None);
    let mut b = ast.builder();
    let s = b.string_literal(builtin(INT), StringKind::Utf8, StringLitId::new(3), L);
    let rv = b.decl_ref(v, builtin(INT), L, None, NonOdrUse::None, ExprFlags::NONE);
    let ra = b.decl_ref(a, builtin(INT), L, None, NonOdrUse::Constant, ExprFlags::NONE);
    assert_eq!(ast.exprs.get(s).value_category(), ValueCategory::LValue);
    assert_eq!(ast.exprs.string_lit(s), StringLitId::new(3));
    assert_eq!(ast.exprs.string_kind(s), StringKind::Utf8);
    assert_eq!(ast.exprs.get(rv).value_category(), ValueCategory::LValue);
    assert_eq!(ast.exprs.get(ra).value_category(), ValueCategory::PRValue);
    assert_eq!(ast.exprs.non_odr_use(ra), NonOdrUse::Constant);
    assert_eq!(ast.exprs.decl(rv), v);
}

/// [expr.unary.op]/1 (`*` is an lvalue), [expr.pre.incr]/1 (prefix
/// `++` is an lvalue), [expr.post.incr]/1 (postfix is a prvalue),
/// [expr.assign]/1, [expr.comma]/1, [expr.sub]/2.
#[test]
fn operator_categories_expr_unary_assign_comma_sub() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let v = int_var(&mut ast, tu, "v");
    let mut b = ast.builder();
    let rv = b.decl_ref(v, builtin(INT), L, None, NonOdrUse::None, ExprFlags::NONE);
    let one = b.int_literal(builtin(INT), 1, Radix::Decimal, L);
    let deref = b.unary(UnaryOp::Deref, rv, builtin(INT), L, ExprFlags::NONE);
    let pre = b.unary(UnaryOp::PreInc, rv, builtin(INT), L, ExprFlags::NONE);
    let post = b.unary(UnaryOp::PostInc, rv, builtin(INT), L, ExprFlags::NONE);
    let not = b.unary(UnaryOp::LNot, rv, builtin(BOOL), L, ExprFlags::NONE);
    let asg = b.binary(BinaryOp::Assign, rv, one, builtin(INT), L);
    let cmp = b.binary(BinaryOp::LT, rv, one, builtin(BOOL), L);
    let comma_l = b.binary(BinaryOp::Comma, one, rv, builtin(INT), L);
    let comma_r = b.binary(BinaryOp::Comma, rv, one, builtin(INT), L);
    let sub = b.subscript(rv, one, builtin(INT), L);
    let ca = b.compound_assign(
        BinaryOp::AddAssign,
        rv,
        one,
        builtin(INT),
        builtin(INT),
        builtin(INT),
        L,
    );
    let cat = |e| ast.exprs.get(e).value_category();
    assert_eq!(cat(deref), ValueCategory::LValue);
    assert_eq!(cat(pre), ValueCategory::LValue);
    assert_eq!(cat(post), ValueCategory::PRValue);
    assert_eq!(cat(not), ValueCategory::PRValue);
    assert_eq!(cat(asg), ValueCategory::LValue);
    assert_eq!(cat(cmp), ValueCategory::PRValue);
    assert_eq!(cat(comma_l), ValueCategory::LValue);
    assert_eq!(cat(comma_r), ValueCategory::PRValue);
    assert_eq!(cat(sub), ValueCategory::LValue);
    assert_eq!(cat(ca), ValueCategory::LValue);
    assert_eq!(ast.exprs.compound_assign(ca).rhs, one);
    assert_eq!(ast.exprs.binary_op(ca), BinaryOp::AddAssign);
    assert_eq!(ast.exprs.unary_op(pre), UnaryOp::PreInc);
    assert_eq!(ast.exprs.children(ca, &ast.lists), vec![rv, one]);
}

/// [expr.cond]/4-5: glvalue arms of the same category give that category;
/// otherwise a prvalue (after sema's conversions).
#[test]
fn conditional_category_expr_cond() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let v = int_var(&mut ast, tu, "v");
    let w = int_var(&mut ast, tu, "w");
    let mut b = ast.builder();
    let c = b.bool_literal(builtin(BOOL), true, L);
    let rv = b.decl_ref(v, builtin(INT), L, None, NonOdrUse::None, ExprFlags::NONE);
    let rw = b.decl_ref(w, builtin(INT), L, None, NonOdrUse::None, ExprFlags::NONE);
    let one = b.int_literal(builtin(INT), 1, Radix::Decimal, L);
    let both = b.conditional(c, rv, rw, builtin(INT), L);
    let mixed = b.conditional(c, rv, one, builtin(INT), L);
    assert_eq!(ast.exprs.get(both).value_category(), ValueCategory::LValue);
    assert_eq!(ast.exprs.get(mixed).value_category(), ValueCategory::PRValue);
    assert_eq!(ast.exprs.cond_arms(both), [rv, rw]);
}

/// [expr.ref]/6: `.` on a field takes the object's category (a prvalue
/// object is materialized to an xvalue), `->` gives an lvalue; a
/// bit-field has object kind bit-field; a non-static member function is
/// a prvalue.
#[test]
fn member_access_categories_expr_ref_6() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let h = head(&ast, tu, "P");
    let p = ast.builder().record(h, TagKind::Struct, None);
    ast.builder().begin_record_definition(p).unwrap();
    let bw = lit(&mut ast, 3);
    let h = head(&ast, p, "x");
    let x = ast.builder().field(h, builtin(INT), None, 0);
    let h = head(&ast, p, "bits");
    let bits = ast.builder().field(h, builtin(INT), Some(bw), FieldData::MUTABLE);
    let h = head(&ast, p, "f");
    let f = ast.builder().function(h, DeclKind::CXXMethod, builtin(INT), FnSpec::new(), None);
    ast.builder().complete_record_definition(p, DefinitionBits::AGGREGATE);
    let pty = ast.decls.get(p).ty().unwrap();
    let q = {
        let h = head(&ast, tu, "q");
        ast.builder().var(h, pty, VarSpec::new(), None)
    };
    let mut b = ast.builder();
    let rq = b.decl_ref(q, pty, L, None, NonOdrUse::None, ExprFlags::NONE);
    let prv = b.init_list(pty, &[], None, None, L, L);
    let dot = b.member(rq, x, builtin(INT), false, L, None);
    let tmp = b.member(prv, x, builtin(INT), false, L, None);
    let arrow = b.member(prv, x, builtin(INT), true, L, None);
    let bf = b.member(rq, bits, builtin(INT), false, L, None);
    let mf = b.member(rq, f, builtin(INT), false, L, None);
    let n = |e| *ast.exprs.get(e);
    assert_eq!(n(dot).value_category(), ValueCategory::LValue);
    assert_eq!(n(tmp).value_category(), ValueCategory::XValue);
    assert_eq!(n(arrow).value_category(), ValueCategory::LValue);
    assert!(n(arrow).has(ExprFlags::ARROW));
    assert_eq!(n(bf).object_kind(), ObjectKind::BitField);
    assert_eq!(n(dot).object_kind(), ObjectKind::Ordinary);
    assert_eq!(n(mf).value_category(), ValueCategory::PRValue);
    assert_eq!(ast.exprs.decl(bf), bits);
    assert!(ast.decls.field(bits).unwrap().is_mutable());
}

/// [expr.prim.paren]/1: a parenthesized expression has the same type,
/// category and dependence as the enclosed one; [expr.call]/13: the
/// category of a call follows the return type (supplied by sema).
#[test]
fn paren_and_call_categories_expr_prim_paren_expr_call() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let v = int_var(&mut ast, tu, "v");
    let mut b = ast.builder();
    let rv = b.decl_ref(v, builtin(INT), L, None, NonOdrUse::None, ExprFlags::NONE);
    let p = b.paren(rv, L, L);
    let c = b.call(p, &[rv, rv], builtin(INT), ValueCategory::XValue, L, L, ExprFlags::ADL);
    assert_eq!(ast.exprs.get(p).value_category(), ValueCategory::LValue);
    assert_eq!(ast.exprs.get(p).ty(), builtin(INT));
    assert_eq!(ast.exprs.get(c).value_category(), ValueCategory::XValue);
    assert!(ast.exprs.get(c).has(ExprFlags::ADL));
    assert_eq!(ast.exprs.callee(c), p);
    assert_eq!(ast.lists.iter::<ExprId>(ast.exprs.call_args(c)).collect::<Vec<_>>(), vec![rv, rv]);
    assert_eq!(ast.exprs.end_loc(c), Some(L));
}

/// [expr.mptr.oper]/6: the category of `.*` depends on the member, so
/// the plain binary constructor refuses it.
#[test]
#[should_panic(expected = "use ptr_mem")]
fn ptr_mem_requires_category_expr_mptr_oper() {
    let mut ast = new_ast();
    let one = lit(&mut ast, 1);
    ast.builder().binary(BinaryOp::PtrMemD, one, one, builtin(INT), L);
}

// ---------------------------------------------------------------------------
// Redeclarations (§4.4)
// ---------------------------------------------------------------------------

/// [basic.def]/2, [dcl.fct.def]: redeclarations share a group; lookup
/// returns the latest, the type refers to the first, the definition is
/// recorded once.
#[test]
fn redeclaration_groups_basic_def() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let n = name(&ast, "f");
    let h = DeclHead::new(tu, n, L);
    let f1 = ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), None);
    let f2 = ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), Some(f1));
    let body = ast.builder().compound(&[], L, L);
    ast.builder().set_body(f2, body).unwrap();
    let f3 = ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), Some(f1));
    for d in [f1, f2, f3] {
        assert_eq!(ast.first(d), f1);
        assert_eq!(ast.latest(d), f3);
        assert_eq!(ast.definition(d), Some(f2));
    }
    assert_eq!(ast.prev(f3), Some(f2));
    assert_eq!(ast.prev(f2), Some(f1));
    assert_eq!(ast.redecls_of(f1).collect::<Vec<_>>(), vec![f3, f2, f1]);
    assert_eq!(ast.lookup_local(tu, n).to_vec(), vec![f3]);
    assert_eq!(ast.members(tu).collect::<Vec<_>>(), vec![f1, f2, f3]);
    let body2 = ast.builder().compound(&[], L, L);
    assert!(ast.builder().set_body(f3, body2).is_err());

    let s = name(&ast, "S");
    let r1 = ast.builder().record(DeclHead::new(tu, s, L), TagKind::Class, None);
    let r2 = ast.builder().record(DeclHead::new(tu, s, L), TagKind::Class, Some(r1));
    assert_eq!(ast.decls.get(r1).ty(), ast.decls.get(r2).ty());
    ast.builder().begin_record_definition(r2).unwrap();
    assert!(ast.builder().begin_record_definition(r1).is_err());
    assert_eq!(ast.primary_context(r1), r2);
}

/// [class.mem]/1, spec/ast.md §2.4: a class's members are those declared
/// in its definition; the member list is frozen at `}` except for
/// implicitly declared members.
#[test]
fn class_member_list_frozen_class_mem() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let h = head(&ast, tu, "C");
    let c = ast.builder().record(h, TagKind::Class, None);
    ast.builder().begin_record_definition(c).unwrap();
    let h = head(&ast, c, "a");
    let a = ast.builder().field(h, builtin(INT), None, 0);
    ast.builder().complete_record_definition(c, DefinitionBits::EMPTY);
    let ctor_name = ast.names.constructor(ast.decls.get(c).ty().unwrap().ty());
    let h = DeclHead::new(c, ctor_name, L).flags(DeclFlags::IMPLICIT);
    let ctor =
        ast.builder().function(h, DeclKind::CXXConstructor, builtin(INT), FnSpec::new(), None);
    assert_eq!(ast.members(c).collect::<Vec<_>>(), vec![a, ctor]);
    assert_eq!(ast.lookup_local(c, ctor_name).single(), Some(ctor));
    let rec = ast.decls.record(c).unwrap();
    assert!(rec.defn.contains(DefinitionBits::COMPLETE));
}

/// spec/ast.md §2.4: a non-implicit member cannot be added after `}`.
#[test]
#[should_panic(expected = "complete class")]
fn late_member_panics_class_mem() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let h = head(&ast, tu, "C");
    let c = ast.builder().record(h, TagKind::Class, None);
    ast.builder().begin_record_definition(c).unwrap();
    ast.builder().complete_record_definition(c, DefinitionBits::EMPTY);
    let h = head(&ast, c, "late");
    ast.builder().field(h, builtin(INT), None, 0);
}

/// spec/ast.md §2.4, §9.2: write-once slots report the existing value.
#[test]
fn write_once_slots_spec_ast_2_4() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let v = int_var(&mut ast, tu, "v");
    let one = lit(&mut ast, 1);
    ast.builder().set_var_init(v, one).unwrap();
    let err = ast.builder().set_var_init(v, one).unwrap_err();
    assert_eq!(err.slot, "VarData.init");
    let f = {
        let h = head(&ast, tu, "f");
        ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), None)
    };
    let n = name(&ast, "p");
    let p = ast.builder().param(f, n, L, builtin(INT), 0, 0);
    ast.builder().set_params(f, &[p]).unwrap();
    assert!(ast.builder().set_params(f, &[p]).is_err());
    assert_eq!(
        ast.lists.iter::<DeclId>(ast.decls.function(f).unwrap().params).collect::<Vec<_>>(),
        vec![p]
    );
    // Parameters are in no member list or lookup table (§4.3).
    assert!(ast.lookup_local(tu, n).is_empty());
    assert_eq!(ast.members(tu).count(), 2);
}

// ---------------------------------------------------------------------------
// Lookup (§4.3)
// ---------------------------------------------------------------------------

/// [over.pre]/1, [class.name]/2: overloads form a set in declaration
/// order; a class and a function of the same name coexist (`struct stat`
/// / `stat()`).
#[test]
fn overload_sets_over_pre_class_name() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let n = name(&ast, "stat");
    let h = DeclHead::new(tu, n, L);
    let f1 = ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), None);
    let f2 = ast.builder().function(h, DeclKind::Function, builtin(BOOL), FnSpec::new(), None);
    let s = ast.builder().record(h, TagKind::Struct, None);
    let f1b = ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), Some(f1));
    let r = ast.lookup_local(tu, n);
    assert_eq!(r.to_vec(), vec![f1b, f2, s]);
    assert_eq!(r.len(), 3);
    assert!(r.single().is_none());
}

/// [dcl.link]/1, [dcl.enum]/11, [class.union.anon]/1: names declared in
/// a linkage specification, enumerators of an unscoped enumeration and
/// members of an anonymous union are found in the enclosing scope; scoped
/// enumerators are not.
#[test]
fn transparent_contexts_dcl_link_dcl_enum_class_union_anon() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let ls = ast.builder().linkage_spec(tu, LinkageLang::C, L);
    let cf = {
        let h = head(&ast, ls, "c_fn");
        ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), None)
    };
    assert_eq!(ast.lookup_local(tu, name(&ast, "c_fn")).single(), Some(cf));
    assert_eq!(ast.members(ls).collect::<Vec<_>>(), vec![cf]);
    assert_eq!(ast.members(tu).collect::<Vec<_>>(), vec![ls]);

    let e = {
        let h = head(&ast, tu, "E");
        ast.builder().enumeration(h, 0, None, None)
    };
    ast.builder().begin_enum_definition(e).unwrap();
    let a = {
        let h = head(&ast, e, "A");
        ast.builder().enumerator(h, None)
    };
    ast.builder().complete_enum_definition(e, Some((builtin(INT), builtin(INT)))).unwrap();
    assert_eq!(ast.lookup_local(tu, name(&ast, "A")).single(), Some(a));
    assert_eq!(ast.lookup_local(e, name(&ast, "A")).single(), Some(a));
    assert_eq!(ast.decls.get(a).ty(), ast.decls.get(e).ty());

    let se = {
        let h = head(&ast, tu, "SE");
        ast.builder().enumeration(h, EnumData::SCOPED, Some((builtin(INT), builtin(INT))), None)
    };
    ast.builder().begin_enum_definition(se).unwrap();
    let b = {
        let h = head(&ast, se, "B");
        ast.builder().enumerator(h, None)
    };
    assert!(
        ast.builder().complete_enum_definition(se, Some((builtin(INT), builtin(INT)))).is_err()
    );
    assert!(ast.lookup_local(tu, name(&ast, "B")).is_empty());
    assert_eq!(ast.lookup_local(se, name(&ast, "B")).single(), Some(b));
    assert!(ast.decls.enumeration(se).unwrap().is_fixed());

    // struct S { union { int u; }; };
    let s = {
        let h = head(&ast, tu, "S");
        ast.builder().record(h, TagKind::Struct, None)
    };
    ast.builder().begin_record_definition(s).unwrap();
    let anon = ast.builder().record(DeclHead::new(s, DeclName::EMPTY, L), TagKind::Union, None);
    ast.builder().begin_record_definition(anon).unwrap();
    let u = {
        let h = head(&ast, anon, "u");
        ast.builder().field(h, builtin(INT), None, 0)
    };
    ast.builder().complete_record_definition(anon, DefinitionBits::ANONYMOUS);
    let anon_ty = ast.decls.get(anon).ty().unwrap();
    let anon_field = ast.builder().field(
        DeclHead::new(s, DeclName::EMPTY, L).flags(DeclFlags::IMPLICIT),
        anon_ty,
        None,
        0,
    );
    let ind = {
        let h = head(&ast, s, "u");
        ast.builder().indirect_field(h, builtin(INT), &[anon_field, u])
    };
    assert_eq!(ast.lookup_local(s, name(&ast, "u")).single(), Some(ind));
    assert_eq!(
        ast.lists.iter::<DeclId>(ast.indirect_field_chain(ind)).collect::<Vec<_>>(),
        vec![anon_field, u]
    );
}

/// [namespace.def]/7: members of an inline namespace are also found in
/// the enclosing namespace (flagged `via_inline`), up a chain of inline
/// namespaces; reopening a namespace shares its table.
#[test]
fn inline_namespaces_namespace_def_7() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let std_ = {
        let h = head(&ast, tu, "std");
        ast.builder().namespace(h, false, None)
    };
    let v1 = {
        let h = head(&ast, std_, "v1");
        ast.builder().namespace(h, true, None)
    };
    let v2 = {
        let h = head(&ast, v1, "v2");
        ast.builder().namespace(h, true, None)
    };
    let x = int_var(&mut ast, v2, "x");
    let nx = name(&ast, "x");
    assert_eq!(ast.lookup_local(v2, nx).iter_with_inline().collect::<Vec<_>>(), vec![(x, false)]);
    assert_eq!(ast.lookup_local(v1, nx).iter_with_inline().collect::<Vec<_>>(), vec![(x, true)]);
    assert_eq!(ast.lookup_local(std_, nx).iter_with_inline().collect::<Vec<_>>(), vec![(x, true)]);
    assert!(ast.lookup_local(tu, nx).is_empty());
    assert_eq!(ast.lookup.inline_children(std_), &[v1]);
    assert_eq!(ast.lookup.inline_children(v1), &[v2]);

    // Reopen std and v2: the same tables; a redeclaration of x replaces it
    // everywhere.
    let std2 = {
        let h = head(&ast, tu, "std");
        ast.builder().namespace(h, false, Some(std_))
    };
    assert_eq!(ast.lookup_local(tu, name(&ast, "std")).single(), Some(std2));
    assert_eq!(ast.primary_context(std2), std_);
    let v2b = {
        let h = head(&ast, v1, "v2");
        ast.builder().namespace(h, true, Some(v2))
    };
    let x2 = {
        let h = head(&ast, v2b, "x");
        ast.builder().var(h, builtin(INT), VarSpec::new(), Some(x))
    };
    assert_eq!(ast.lookup_local(std2, nx).iter_with_inline().collect::<Vec<_>>(), vec![(x2, true)]);
    assert_eq!(ast.lookup_local(v2, nx).single(), Some(x2));
    assert_eq!(ast.members(v2b).collect::<Vec<_>>(), vec![x2]);
    assert_eq!(ast.lookup.inline_children(v1), &[v2]);
}

/// [basic.scope.scope]/5 (P2169): several name-independent declarations
/// of `_` in one scope all stay visible.
#[test]
fn name_independent_declarations_p2169_basic_scope_scope_5() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let s = {
        let h = head(&ast, tu, "S");
        ast.builder().record(h, TagKind::Struct, None)
    };
    ast.builder().begin_record_definition(s).unwrap();
    let us = name(&ast, "_");
    let h = DeclHead::new(s, us, L).misc(DeclMisc::new().with_name_independent(true));
    let a = ast.builder().field(h, builtin(INT), None, 0);
    let b = ast.builder().field(h, builtin(INT), None, 0);
    assert_eq!(ast.lookup_local(s, us).to_vec(), vec![a, b]);
    assert!(ast.decls.get(a).misc().name_independent());
}

// ---------------------------------------------------------------------------
// Statements (§6), attributes (§7), memory (§2.3, §15 Q6)
// ---------------------------------------------------------------------------

/// [stmt.block], [stmt.if], [stmt.for], [stmt.return]: statement payloads
/// round-trip.
#[test]
fn statements_round_trip_stmt() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let f = {
        let h = head(&ast, tu, "f");
        ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), None)
    };
    let i = int_var(&mut ast, f, "i");
    assert!(
        ast.lookup_local(f, name(&ast, "i")).is_empty(),
        "function scopes have no tables (§4.3)"
    );
    let mut b = ast.builder();
    let ds = b.decl_stmt(&[i], L);
    let one = b.int_literal(builtin(INT), 1, Radix::Decimal, L);
    let ret = b.return_stmt(Some(one), Some(i), L);
    let es = b.expr_stmt(one);
    let fo = b.for_stmt(
        LoopData {
            init: Some(ds),
            cond: Some(one),
            inc: Some(one),
            body: Some(es),
            ..LoopData::default()
        },
        L,
    );
    let ifs = b.if_stmt(
        IfKind::Constexpr,
        IfData { cond: Some(one), then: Some(ret), ..IfData::default() },
        L,
    );
    let body = b.compound(&[fo, ifs], L, L);
    b.set_body(f, body).unwrap();
    assert_eq!(ast.stmts.get(body).kind(), StmtKind::CompoundStmt);
    assert_eq!(
        ast.lists.iter::<kiln_ast::StmtId>(ast.stmts.compound_body(body)).collect::<Vec<_>>(),
        vec![fo, ifs]
    );
    assert_eq!(ast.stmts.loop_data(fo).init, Some(ds));
    assert_eq!(ast.stmts.if_kind(ifs), IfKind::Constexpr);
    assert_eq!(ast.stmts.if_data(ifs).then, Some(ret));
    assert_eq!(ast.stmts.return_value(ret), Some(one));
    assert_eq!(ast.stmts.return_nrvo(ret), Some(i));
    assert_eq!(ast.stmts.expr_stmt_expr(es), one);
    assert_eq!(
        ast.lists.iter::<DeclId>(ast.stmts.decl_stmt_decls(ds)).collect::<Vec<_>>(),
        vec![i]
    );
    assert_eq!(ast.decls.function(f).unwrap().body, Some(body));
}

/// [dcl.attr.grammar]/5, spec/ast.md §7: attributes attach once and set
/// `HAS_ATTRS`.
#[test]
fn attach_attributes_dcl_attr_grammar() {
    use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
    let mut ast = new_ast();
    let tu = ast.tu;
    let v = int_var(&mut ast, tu, "v");
    let a = ast.attrs.push(AttrNode {
        kind: AttrKind::MAYBE_UNUSED,
        syntax: AttrSyntax::Cxx11,
        flags: 0,
        loc: L,
        a: 0,
        b: 0,
    });
    ast.builder().attach_attrs(v, &[a]).unwrap();
    assert!(ast.decls.get(v).has(DeclFlags::HAS_ATTRS));
    assert_eq!(
        ast.lists.iter::<kiln_ast::AttrId>(ast.attrs.of_decl(v)).collect::<Vec<_>>(),
        vec![a]
    );
    assert!(ast.builder().attach_attrs(v, &[a]).is_err());
}

/// spec/ast.md §2.3, §15 Q6: the memory report has a row per arena and an
/// `instantiation.retained` row counting node growth inside (outermost)
/// instantiations, excluded from the total.
#[test]
fn memory_report_instantiation_row_spec_ast_15_q6() {
    let mut ast = new_ast();
    let r0 = ast.memory();
    let row = r0.row("instantiation.retained").expect("row present from day one");
    assert_eq!((row.count, row.live_bytes), (0, 0));
    ast.begin_instantiation();
    ast.begin_instantiation();
    for v in 0..100 {
        lit(&mut ast, v);
    }
    ast.end_instantiation();
    ast.end_instantiation();
    lit(&mut ast, 1000);
    let r = ast.memory();
    let row = r.row("instantiation.retained").unwrap();
    assert_eq!(row.count, 2);
    assert!(row.live_bytes >= 100 * 20, "{}", row.live_bytes);
    assert!(r.row("exprs.nodes").unwrap().live_bytes >= 101 * 20);
    let sum: u64 =
        r.rows.iter().filter(|r| !r.name.starts_with("instantiation")).map(|r| r.live_bytes).sum();
    assert_eq!(r.live_bytes(), sum);
}
