//! `AstBuilder` tests for the class additions (spec/sema-class.md §8.1,
//! WP-066a): access specifiers, friends and `ADL_ONLY`, ctor-initializers,
//! overrides, lifetime-extended temporaries, lazily appended implicit
//! members, NRVO, the §5.1 expression kinds and the class reads. Each test
//! names the standard section it checks.

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "short names mirror the C++ snippets under test"
)]

mod common;

use common::{DEPENDENT, INT, builtin, new_ast};
use kiln_ast::decl::{
    Access, CtorInit, CtorInitKind, DeclFlags, DeclKind, DeclMisc, DefinitionBits, FnSpec,
    FriendKind, FriendTarget, ParamData, TagKind, VarSpec,
};
use kiln_ast::expr::{
    CastKind, ConstructInfo, ConstructionKind, ExprDeps, ExprFlags, ExprKind, NonOdrUse,
    ObjectKind, Radix, StorageDuration, TypeidOperand, ValueCategory,
};
use kiln_ast::lookup::LookupFlags;
use kiln_ast::names::DeclName;
use kiln_ast::{Ast, ConstValueId, DeclHead, DeclId, ExprId, QualType, RawId};
use kiln_source::Loc;

const L: Loc = Loc::INVALID;

fn name(ast: &Ast, s: &str) -> DeclName {
    DeclName::ident(ast.symbols().intern(s))
}

fn class(ast: &mut Ast, s: &str) -> DeclId {
    let tu = ast.tu;
    let h = DeclHead::new(tu, name(ast, s), L);
    let c = ast.builder().record(h, TagKind::Struct, None);
    ast.builder().begin_record_definition(c).unwrap();
    c
}

fn class_ty(ast: &Ast, c: DeclId) -> QualType {
    ast.decls.get(c).ty().unwrap()
}

fn ctor(ast: &mut Ast, c: DeclId, flags: DeclFlags) -> DeclId {
    let n = ast.names.constructor(class_ty(ast, c).ty());
    let h = DeclHead::new(c, n, L).flags(flags).access(Access::Public);
    ast.builder().function(h, DeclKind::CXXConstructor, builtin(INT), FnSpec::new(), None)
}

fn lit(ast: &mut Ast, v: u64) -> ExprId {
    ast.builder().int_literal(builtin(INT), v, Radix::Decimal, L)
}

fn dependent_expr(ast: &mut Ast) -> ExprId {
    let tu = ast.tu;
    let h = DeclHead::new(tu, name(ast, "tmpl"), L);
    let f = ast.builder().function(h, DeclKind::Function, builtin(DEPENDENT), FnSpec::new(), None);
    let n = name(ast, "t");
    let p = ast.builder().param(f, n, L, builtin(DEPENDENT), 0, 0);
    ast.builder().decl_ref(p, builtin(DEPENDENT), L, None, NonOdrUse::None, ExprFlags::NONE)
}

// ---------------------------------------------------------------------------
// Declarations
// ---------------------------------------------------------------------------

/// [class.access.spec]: an access specifier is a member carrying its
/// access; it has no name and no lookup entry.
#[test]
fn access_spec_is_a_member_class_access_spec() {
    let mut ast = new_ast();
    let c = class(&mut ast, "C");
    let a = ast.builder().access_spec(c, Access::Private, L);
    let n = ast.decls.get(a);
    assert_eq!(n.kind(), DeclKind::AccessSpec);
    assert_eq!(n.misc().access(), Access::Private);
    assert_eq!(ast.members(c).collect::<Vec<_>>(), vec![a]);
}

/// [class.friend], [namespace.memdef]/3: a hidden friend is entered with
/// `ADL_ONLY` (skipped by `iter_ordinary`, kept for redeclaration
/// lookup); a friend type is stored in `ty`.
#[test]
fn hidden_friend_is_adl_only_namespace_memdef() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let c = class(&mut ast, "C");
    let fname = name(&ast, "f");
    let h = DeclHead::new(tu, fname, L)
        .lexical(c)
        .misc(DeclMisc::new().with_friend(FriendKind::Undeclared));
    let f = ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), None);
    let fr = ast.builder().friend(c, L, FriendTarget::Decl(f));
    let ft = ast.builder().friend(c, L, FriendTarget::Type(builtin(INT)));
    assert_eq!(ast.decls.friend_target(fr), FriendTarget::Decl(f));
    assert_eq!(ast.decls.friend_target(ft), FriendTarget::Type(builtin(INT)));
    // The befriended function is a lexical member of C, then the friends.
    assert_eq!(ast.members(c).collect::<Vec<_>>(), vec![f, fr, ft]);
    let r = ast.lookup_local(tu, fname);
    assert_eq!(r.iter().collect::<Vec<_>>(), vec![f]);
    assert_eq!(r.iter_ordinary().count(), 0);
    assert_eq!(r.iter_with_flags().collect::<Vec<_>>(), vec![(f, LookupFlags::ADL_ONLY)]);
    // A visible redeclaration replaces the entry and drops ADL_ONLY.
    let h = DeclHead::new(tu, fname, L);
    let f2 = ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), Some(f));
    let r = ast.lookup_local(tu, fname);
    assert_eq!(r.iter_ordinary().collect::<Vec<_>>(), vec![f2]);
    ast.builder().set_friends(c, &[f]).unwrap();
    assert!(ast.builder().set_friends(c, &[f]).is_err());
}

/// [over.match.oper]/3.2 vs [basic.lookup.argdep]/4: an ordinary overload
/// next to a hidden friend keeps only the ordinary one visible.
#[test]
fn hidden_friend_in_overload_set_basic_lookup_argdep() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let c = class(&mut ast, "C");
    let g = name(&ast, "g");
    let h = DeclHead::new(tu, g, L);
    let visible = ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), None);
    let h = DeclHead::new(tu, g, L)
        .lexical(c)
        .misc(DeclMisc::new().with_friend(FriendKind::Undeclared));
    let hidden = ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), None);
    let r = ast.lookup_local(tu, g);
    assert_eq!(r.iter().collect::<Vec<_>>(), vec![visible, hidden]);
    assert_eq!(r.iter_ordinary().collect::<Vec<_>>(), vec![visible]);
}

/// [class.base.init]: ctor-initializers are write-once, in order; a
/// non-constructor panics.
#[test]
fn ctor_inits_round_trip_class_base_init() {
    let mut ast = new_ast();
    let c = class(&mut ast, "C");
    let h = DeclHead::new(c, name(&ast, "x"), L);
    let x = ast.builder().field(h, builtin(INT), None, 0);
    let k = ctor(&mut ast, c, DeclFlags::empty());
    let one = lit(&mut ast, 1);
    let inits = [
        CtorInit {
            target: class_ty(&ast, c).ty().raw(),
            kind: CtorInitKind::Base,
            written_index: CtorInit::IMPLICIT,
            init: one,
            loc: L,
        },
        CtorInit {
            target: x.raw(),
            kind: CtorInitKind::Field,
            written_index: 0,
            init: one,
            loc: L,
        },
    ];
    assert_eq!(ast.ctor_inits(k).len(), 0);
    ast.builder().set_ctor_inits(k, &inits).unwrap();
    let got: Vec<CtorInit> = ast.ctor_inits(k).copied().collect();
    assert_eq!(got, inits);
    assert!(!got[0].is_written());
    assert!(got[1].is_written());
    assert!(ast.builder().set_ctor_inits(k, &inits).is_err());
    let mem = ast.memory();
    assert!(mem.row("decls.ctor_inits").unwrap().live_bytes >= 3 * 16);
}

#[test]
#[should_panic(expected = "non-constructor")]
fn ctor_inits_on_method_panics() {
    let mut ast = new_ast();
    let c = class(&mut ast, "C");
    let h = DeclHead::new(c, name(&ast, "m"), L);
    let m = ast.builder().function(h, DeclKind::CXXMethod, builtin(INT), FnSpec::new(), None);
    let _ = ast.builder().set_ctor_inits(m, &[]);
}

/// [class.virtual]/2: overridden methods are recorded once.
#[test]
fn overrides_write_once_class_virtual() {
    let mut ast = new_ast();
    let b = class(&mut ast, "B");
    let h = DeclHead::new(b, name(&ast, "f"), L);
    let spec = FnSpec::new().with_virtual(true);
    let bf = ast.builder().function(h, DeclKind::CXXMethod, builtin(INT), spec, None);
    let d = class(&mut ast, "D");
    let h = DeclHead::new(d, name(&ast, "f"), L);
    let spec = FnSpec::new().with_virtual(true).with_override(true);
    let df = ast.builder().function(h, DeclKind::CXXMethod, builtin(INT), spec, None);
    ast.builder().set_overrides(df, &[bf]).unwrap();
    let l = *ast.decls.cold.overrides.get(df).unwrap();
    assert_eq!(ast.lists.iter::<DeclId>(l).collect::<Vec<_>>(), vec![bf]);
    assert!(ast.builder().set_overrides(df, &[bf]).is_err());
    assert!(ast.decls.function(df).unwrap().spec.is_override());
}

/// [class.temporary]/6: the extended temporary joins no list or table;
/// its value is write-once.
#[test]
fn lifetime_extended_temporary_class_temporary() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let h = DeclHead::new(tu, name(&ast, "r"), L);
    let r = ast.builder().var(h, builtin(INT), VarSpec::new(), None);
    let members_before = ast.members(tu).count();
    let t = ast.builder().lifetime_extended_temporary(r, 0, builtin(INT), L);
    assert_eq!(ast.decls.get(t).kind(), DeclKind::LifetimeExtendedTemporary);
    assert_eq!(ast.decls.get(t).ctx(), Some(tu));
    assert_eq!(ast.members(tu).count(), members_before);
    let data = ast.decls.lifetime_extended(t).unwrap();
    assert_eq!((data.extending, data.mangling_index, data.value), (r, 0, None));
    let v = ConstValueId::new(1);
    ast.decls.set_lifetime_extended_value(t, v).unwrap();
    assert!(ast.decls.set_lifetime_extended_value(t, v).is_err());
}

/// [class.default.ctor]/1, spec/sema-class.md §4.1: implicit members are
/// appended to a complete class and found by `Ast::constructors` /
/// `Ast::destructor`.
#[test]
fn append_implicit_member_after_complete_class_mem() {
    let mut ast = new_ast();
    let c = class(&mut ast, "C");
    assert!(ast.constructors(c).is_empty());
    assert_eq!(ast.destructor(c), None);
    let user = ctor(&mut ast, c, DeclFlags::empty());
    ast.builder().complete_record_definition(c, DefinitionBits::empty());
    let ty = class_ty(&ast, c).ty();
    let cn = ast.names.constructor(ty);
    let h = DeclHead::new(c, cn, L).flags(DeclFlags::IMPLICIT);
    let copy = ast.builder().append_implicit_member(
        h,
        DeclKind::CXXConstructor,
        builtin(INT),
        FnSpec::new(),
    );
    let dn = ast.names.destructor(ty);
    let h = DeclHead::new(c, dn, L).flags(DeclFlags::IMPLICIT);
    let dtor = ast.builder().append_implicit_member(
        h,
        DeclKind::CXXDestructor,
        builtin(INT),
        FnSpec::new(),
    );
    assert_eq!(ast.constructors(c).iter().collect::<Vec<_>>(), vec![user, copy]);
    assert_eq!(ast.destructor(c), Some(dtor));
    assert_eq!(ast.members(c).collect::<Vec<_>>(), vec![user, copy, dtor]);
}

#[test]
#[should_panic(expected = "without IMPLICIT")]
fn append_implicit_member_requires_implicit() {
    let mut ast = new_ast();
    let c = class(&mut ast, "C");
    let h = DeclHead::new(c, name(&ast, "m"), L);
    ast.builder().append_implicit_member(h, DeclKind::CXXMethod, builtin(INT), FnSpec::new());
}

/// [class.copy.elision]/1.1: the NRVO bit is late and monotone; a
/// pending default argument is a parameter flag.
#[test]
fn nrvo_and_pending_default_arg_class_copy_elision() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let h = DeclHead::new(tu, name(&ast, "v"), L);
    let v = ast.builder().var(h, builtin(INT), VarSpec::new(), None);
    assert!(!ast.decls.var(v).unwrap().spec.is_nrvo());
    ast.builder().set_nrvo(v);
    assert!(ast.decls.var(v).unwrap().spec.is_nrvo());
    let h = DeclHead::new(tu, name(&ast, "f"), L);
    let f = ast.builder().function(h, DeclKind::Function, builtin(INT), FnSpec::new(), None);
    let n = name(&ast, "p");
    let p = ast.builder().param(f, n, L, builtin(INT), 0, 0);
    ast.decls.mark_default_arg_pending(p);
    assert_ne!(ast.decls.param(p).unwrap().flags & ParamData::DEFAULT_ARG_PENDING, 0);
}

/// spec/sema-class.md §2.9: `is_dynamic` and the new definition bits.
#[test]
fn definition_bits_dynamic_class_virtual() {
    assert!(!DefinitionBits::empty().is_dynamic());
    assert!(DefinitionBits::POLYMORPHIC.is_dynamic());
    assert!(DefinitionBits::HAS_VIRTUAL_BASES.is_dynamic());
    let b = DefinitionBits::empty().with(DefinitionBits::CAN_CONST_DEFAULT_INIT);
    assert!(b.contains(DefinitionBits::CAN_CONST_DEFAULT_INIT));
    assert!(!b.contains(DefinitionBits::POD));
    assert!(FnSpec::LATE.is_deleted());
}

// ---------------------------------------------------------------------------
// Expressions (spec/sema-class.md §5.1)
// ---------------------------------------------------------------------------

/// [class.ctor], [dcl.init]: construct expressions are prvalues carrying
/// the constructor, arguments and flags; dependence comes from the
/// arguments.
#[test]
fn construct_expr_payload_class_ctor() {
    let mut ast = new_ast();
    let c = class(&mut ast, "C");
    let k = ctor(&mut ast, c, DeclFlags::empty());
    let ty = class_ty(&ast, c);
    let one = lit(&mut ast, 1);
    let info = ConstructInfo { list: true, zeroing: true, ..ConstructInfo::COMPLETE };
    let e =
        ast.builder().construct(ty, k, &[one], info, ExprFlags::HAD_MULTIPLE_CANDIDATES, L, None);
    let n = *ast.exprs.get(e);
    assert_eq!(
        (n.kind(), n.value_category()),
        (ExprKind::CXXConstructExpr, ValueCategory::PRValue)
    );
    assert!(n.has(ExprFlags::HAD_MULTIPLE_CANDIDATES));
    assert_eq!(ast.exprs.constructor(e), k);
    assert_eq!(ast.exprs.construct_info(e), info);
    assert_eq!(ast.exprs.children(e, &ast.lists), vec![one]);
    assert!(n.deps().is_none());
    let dep = dependent_expr(&mut ast);
    let base = ConstructInfo { kind: ConstructionKind::VirtualBase, ..ConstructInfo::COMPLETE };
    let t = ast.builder().temporary_object(ty, k, &[dep, one], base, ExprFlags::NONE, L, L);
    assert_eq!(ast.exprs.get(t).kind(), ExprKind::CXXTemporaryObjectExpr);
    assert_eq!(ast.exprs.construct_info(t).kind, ConstructionKind::VirtualBase);
    assert!(ast.exprs.get(t).deps().contains(ExprDeps::TYPE));
    assert_eq!(ast.exprs.end_loc(t), Some(L));
    let i = ast.builder().inherited_ctor_init(ty, k, ConstructionKind::NonVirtualBase, true, L);
    assert_eq!(ast.exprs.constructor(i), k);
    assert_eq!(ast.exprs.inherited_ctor_info(i), (ConstructionKind::NonVirtualBase, true));
    for aux in 0..64 {
        assert_eq!(ConstructInfo::from_aux(aux).to_aux(), aux);
    }
}

/// [class.temporary], [conv.rval]: materialization yields an lvalue when
/// bound to an lvalue reference, else an xvalue; binds and cleanups keep
/// their operand's type; `ExprWithCleanups` keeps its category.
#[test]
fn temporaries_categories_class_temporary() {
    let mut ast = new_ast();
    let c = class(&mut ast, "C");
    let k = ctor(&mut ast, c, DeclFlags::empty());
    let ty = class_ty(&ast, c);
    let dn = ast.names.destructor(ty.ty());
    let h = DeclHead::new(c, dn, L);
    let dtor =
        ast.builder().function(h, DeclKind::CXXDestructor, builtin(INT), FnSpec::new(), None);
    let ce = ast.builder().construct(ty, k, &[], ConstructInfo::COMPLETE, ExprFlags::NONE, L, None);
    let bt = ast.builder().bind_temporary(ce, dtor);
    assert_eq!(ast.exprs.bound_destructor(bt), dtor);
    assert_eq!(ast.exprs.get(bt).ty(), ty);
    let tu = ast.tu;
    let h = DeclHead::new(tu, name(&ast, "r"), L);
    let r = ast.builder().var(h, ty, VarSpec::new(), None);
    let let_ = ast.builder().lifetime_extended_temporary(r, 0, ty, L);
    let m = ast.builder().materialize_temporary(bt, ty, true, StorageDuration::Static, Some(let_));
    assert_eq!(ast.exprs.get(m).value_category(), ValueCategory::LValue);
    assert_eq!(ast.exprs.temporary_storage(m), StorageDuration::Static);
    assert!(ast.exprs.bound_to_lvalue_ref(m));
    assert_eq!(ast.exprs.lifetime_extended_temporary(m), Some(let_));
    assert_eq!(ast.exprs.operand(m), bt);
    let ce2 =
        ast.builder().construct(ty, k, &[], ConstructInfo::COMPLETE, ExprFlags::NONE, L, None);
    let x =
        ast.builder().materialize_temporary(ce2, ty, false, StorageDuration::FullExpression, None);
    assert_eq!(ast.exprs.get(x).value_category(), ValueCategory::XValue);
    assert_eq!(ast.exprs.lifetime_extended_temporary(x), None);
    let w = ast.builder().expr_with_cleanups(m, true);
    assert_eq!(ast.exprs.get(w).value_category(), ValueCategory::LValue);
    assert!(ast.exprs.cleanups_have_side_effects(w));
    assert_eq!(ast.exprs.children(w, &ast.lists), vec![m]);
    assert_eq!(ast.exprs.children(m, &ast.lists), vec![bt]);
    assert_eq!(ast.exprs.children(bt, &ast.lists), vec![ce]);
}

#[test]
#[should_panic(expected = "disagree")]
fn materialize_full_expression_with_extension_panics() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let h = DeclHead::new(tu, name(&ast, "r"), L);
    let r = ast.builder().var(h, builtin(INT), VarSpec::new(), None);
    let t = ast.builder().lifetime_extended_temporary(r, 0, builtin(INT), L);
    let one = lit(&mut ast, 1);
    ast.builder().materialize_temporary(
        one,
        builtin(INT),
        true,
        StorageDuration::FullExpression,
        Some(t),
    );
}

/// [class.base.init]/9, [dcl.fct.default], [expr.prim.this],
/// [expr.call]/2: default initializers/arguments share their initializer
/// (no children); `this`; member calls expose their object argument.
#[test]
fn default_init_this_and_member_call_class_this() {
    let mut ast = new_ast();
    let c = class(&mut ast, "C");
    let h = DeclHead::new(c, name(&ast, "x"), L);
    let x = ast.builder().field(h, builtin(INT), None, 0);
    let k = ctor(&mut ast, c, DeclFlags::empty());
    let di = ast.builder().default_init(x, k, builtin(INT), ValueCategory::PRValue, L);
    assert_eq!(ast.exprs.default_init_field(di), x);
    assert_eq!(ast.exprs.used_context(di), k);
    assert_eq!(ast.exprs.children(di, &ast.lists), Vec::<ExprId>::new());
    let n = name(&ast, "p");
    let p = ast.builder().param(k, n, L, builtin(INT), 0, 0);
    let da = ast.builder().default_arg(p, c, builtin(INT), ValueCategory::PRValue, L);
    assert_eq!(ast.exprs.default_arg_param(da), p);
    assert_eq!(ast.exprs.used_context(da), c);
    let this = ast.builder().this_expr(builtin(INT), true, L);
    assert!(ast.exprs.this_is_implicit(this));
    let h = DeclHead::new(c, name(&ast, "m"), L);
    let m = ast.builder().function(h, DeclKind::CXXMethod, builtin(INT), FnSpec::new(), None);
    let callee = ast.builder().member(this, m, builtin(INT), true, L, None);
    let arg = lit(&mut ast, 2);
    let call = ast.builder().member_call(
        callee,
        &[arg],
        builtin(INT),
        ValueCategory::PRValue,
        L,
        L,
        ExprFlags::NONE,
    );
    assert_eq!(ast.exprs.callee(call), callee);
    assert_eq!(ast.lists.iter::<ExprId>(ast.exprs.call_args(call)).collect::<Vec<_>>(), vec![arg]);
    assert_eq!(ast.exprs.member_call_object(call), this);
    assert_eq!(ast.exprs.children(call, &ast.lists), vec![callee, arg]);
}

/// P0960, [class.copy.ctor]/14, [expr.typeid]: paren-list init, array
/// init loops (the opaque source is shared) and `typeid` dependence.
#[test]
fn paren_list_array_loop_typeid_expr_typeid() {
    let mut ast = new_ast();
    let one = lit(&mut ast, 1);
    let two = lit(&mut ast, 2);
    let pl = ast.builder().paren_list_init(builtin(INT), &[one], Some(two), L, L);
    assert_eq!(ast.exprs.children(pl, &ast.lists), vec![one, two]);
    assert_eq!(ast.exprs.paren_list_filler(pl), Some(two));
    let src = lit(&mut ast, 3);
    let ove = ast.builder().opaque_value(src);
    assert_eq!(ast.exprs.opaque_source(ove), Some(src));
    assert_eq!(ast.exprs.children(ove, &ast.lists), Vec::<ExprId>::new());
    let idx = ast.builder().array_init_index(builtin(INT), L);
    let lp = ast.builder().array_init_loop(builtin(INT), ove, idx);
    assert_eq!(ast.exprs.array_init_loop(lp), [ove, idx]);
    assert_eq!(ast.exprs.children(lp, &ast.lists), vec![ove, idx]);
    let t = ast.builder().typeid_expr(
        TypeidOperand::Type(builtin(DEPENDENT)),
        false,
        builtin(INT),
        L,
        L,
    );
    let n = *ast.exprs.get(t);
    assert_eq!(n.value_category(), ValueCategory::LValue);
    assert!(!n.deps().contains(ExprDeps::TYPE));
    assert!(n.deps().contains(ExprDeps::VALUE_INST));
    assert_eq!(ast.exprs.typeid_operand(t), TypeidOperand::Type(builtin(DEPENDENT)));
    assert_eq!(ast.exprs.children(t, &ast.lists), Vec::<ExprId>::new());
    let dep = dependent_expr(&mut ast);
    let t2 = ast.builder().typeid_expr(TypeidOperand::Expr(dep), true, builtin(INT), L, L);
    assert!(ast.exprs.typeid_potentially_evaluated(t2));
    assert!(!ast.exprs.get(t2).deps().contains(ExprDeps::TYPE));
    assert_eq!(ast.exprs.children(t2, &ast.lists), vec![dep]);
}

/// [expr.type.conv], [expr.dynamic.cast]: the cast wrappers record their
/// closing parenthesis.
#[test]
fn functional_and_dynamic_casts_expr_type_conv() {
    let mut ast = new_ast();
    let one = lit(&mut ast, 1);
    let f = ast.builder().functional_cast(
        CastKind::NoOp,
        one,
        builtin(INT),
        ValueCategory::PRValue,
        None,
        L,
        L,
        ExprFlags::NONE,
    );
    assert_eq!(ast.exprs.get(f).kind(), ExprKind::CXXFunctionalCastExpr);
    assert_eq!(ast.exprs.end_loc(f), Some(L));
    let d = ast.builder().dynamic_cast(
        CastKind::Dynamic,
        one,
        builtin(INT),
        ValueCategory::PRValue,
        None,
        L,
        L,
    );
    assert_eq!(ast.exprs.cast_kind(d), CastKind::Dynamic);
    assert_eq!(ast.exprs.get(d).object_kind(), ObjectKind::Ordinary);
}
