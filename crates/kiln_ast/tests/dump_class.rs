//! Dump lines of the class additions (spec/ast-dump.md §3, WP-066a):
//! base paths, named casts, ctor-initializers, `Overrides:`, friends,
//! `parent`, the delete message and the §5.1 expression kinds, each
//! transcribed from clang 19's `-ast-dump` of the snippet in the comment.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "each test transcribes a C++ snippet node by node"
)]

mod common;

use common::{INT, builtin, new_ast};
use kiln_ast::decl::{
    Access, BaseSpec, CtorInit, CtorInitKind, DeclFlags, DeclKind, DeclMisc, DefinitionBits,
    FnSpec, FriendKind, FriendTarget, InitStyle, TagKind, VarSpec,
};
use kiln_ast::dump::{DumpOptions, dump_to_string};
use kiln_ast::expr::{
    CastKind, ConstructInfo, ExprFlags, ExprKind, NonOdrUse, Radix, StorageDuration, StringKind,
    TypeidOperand, ValueCategory,
};
use kiln_ast::names::DeclName;
use kiln_ast::strings::StringEncoding;
use kiln_ast::types::{ExcSpec, FuncInfo};
use kiln_ast::{Ast, DeclHead, DeclId, ExprId, QualType, RawId};
use kiln_source::Loc;

const L: Loc = Loc::INVALID;

fn name(ast: &Ast, s: &str) -> DeclName {
    DeclName::ident(ast.symbols().intern(s))
}

fn record(ast: &mut Ast, ctx: DeclId, s: &str) -> DeclId {
    let h = DeclHead::new(ctx, name(ast, s), L);
    ast.builder().record(h, TagKind::Struct, None)
}

fn fn_ty(ast: &mut Ast, ret: QualType, params: &[QualType], exc: ExcSpec) -> QualType {
    ast.types.function(ret, params, FuncInfo::new(), exc)
}

fn method(
    ast: &mut Ast,
    c: DeclId,
    kind: DeclKind,
    n: DeclName,
    ty: QualType,
    spec: FnSpec,
) -> DeclId {
    let h = DeclHead::new(c, n, L).access(Access::Public);
    ast.builder().function(h, kind, ty, spec, None)
}

fn check(got: &str, want: &str) {
    assert!(got == want, "dump mismatch\n--- got ---\n{got}\n--- want ---\n{want}");
}

/// ```c++
/// struct F;
/// struct B { virtual int f(); };
/// struct D final : B {
///   friend struct F;
///   friend void h();
///   int m = 2;
///   D() : m(1) {}
///   int f() override;
///   void g() = delete("no");
/// };
/// ```
#[test]
fn class_members() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let int = builtin(INT);
    let f_rec = record(&mut ast, tu, "F");
    let f_ty = ast.decls.get(f_rec).ty().unwrap();
    let b = record(&mut ast, tu, "B");
    ast.builder().begin_record_definition(b).unwrap();
    let int_fn = fn_ty(&mut ast, int, &[], ExcSpec::None);
    let nf = name(&ast, "f");
    let virt = FnSpec::new().with_virtual(true).with_virtual_written(true);
    let bf = method(&mut ast, b, DeclKind::CXXMethod, nf, int_fn, virt);
    ast.builder().complete_record_definition(b, DefinitionBits::POLYMORPHIC);
    let b_ty = ast.decls.get(b).ty().unwrap();

    let d = record(&mut ast, tu, "D");
    ast.builder().begin_record_definition(d).unwrap();
    let base = BaseSpec {
        ty: b_ty,
        loc: L,
        is_virtual: false,
        access: Access::None,
        pack_expansion: false,
    };
    ast.decls.set_bases(d, &[base], &mut ast.lists).unwrap();
    ast.builder().friend(d, L, FriendTarget::Type { ty: f_ty, owned_tag: None });
    let void_fn = fn_ty(&mut ast, QualType::VOID, &[], ExcSpec::None);
    let h = DeclHead::new(tu, name(&ast, "h"), L)
        .lexical(d)
        .misc(DeclMisc::new().with_friend(FriendKind::Undeclared));
    let hf = ast.builder().function(h, DeclKind::Function, void_fn, FnSpec::new(), None);
    ast.builder().friend(d, L, FriendTarget::Decl(hf));
    let two = ast.builder().int_literal(int, 2, Radix::Decimal, L);
    let h = DeclHead::new(d, name(&ast, "m"), L).access(Access::Public);
    let m = ast.builder().field(h, int, None, 0);
    ast.decls.set_field_init(m, two).unwrap();
    let ctor_ty = fn_ty(&mut ast, QualType::VOID, &[], ExcSpec::None);
    let cn = ast.names.constructor(ast.types.canonical(ast.decls.get(d).ty().unwrap()).ty());
    let k = method(
        &mut ast,
        d,
        DeclKind::CXXConstructor,
        cn,
        ctor_ty,
        FnSpec::new().with_implicit_inline(true),
    );
    let one = ast.builder().int_literal(int, 1, Radix::Decimal, L);
    let init = CtorInit {
        target: m.raw(),
        kind: CtorInitKind::Field,
        written_index: 0,
        init: one,
        loc: L,
    };
    ast.builder().set_ctor_inits(k, &[init]).unwrap();
    let body = ast.builder().compound(&[], L, L);
    ast.builder().set_body(k, body).unwrap();
    let df = method(
        &mut ast,
        d,
        DeclKind::CXXMethod,
        nf,
        int_fn,
        FnSpec::new().with_virtual(true).with_override(true),
    );
    ast.builder().set_overrides(df, &[bf]).unwrap();
    let ng = name(&ast, "g");
    let g = method(&mut ast, d, DeclKind::CXXMethod, ng, void_fn, FnSpec::new().with_deleted(true));
    let id = ast.strings.intern(StringEncoding::Ordinary, b"no");
    let arr = ast.types.constant_array(QualType::new(QualType::CHAR.ty(), 1), 3, None);
    let msg = ast.builder().string_literal(arr, StringKind::Unevaluated, id, L);
    ast.decls.cold.delete_message.insert(g, msg).unwrap();
    ast.builder()
        .complete_record_definition(d, DefinitionBits::POLYMORPHIC.with(DefinitionBits::FINAL));

    let got = dump_to_string(&ast, &DumpOptions::default());
    check(
        &got,
        r#"TranslationUnitDecl
|-CXXRecordDecl struct F
|-CXXRecordDecl struct B definition
| `-CXXMethodDecl f 'int ()' virtual
`-CXXRecordDecl struct D definition
  |-public 'B'
  |-FinalAttr final
  |-FriendDecl 'F'
  |-FriendDecl
  | `-FunctionDecl parent friend_undeclared h 'void ()'
  |-FieldDecl m 'int'
  | `-IntegerLiteral 'int' 2
  |-CXXConstructorDecl D 'void ()' implicit-inline
  | |-CXXCtorInitializer Field 'm' 'int'
  | | `-IntegerLiteral 'int' 1
  | `-CompoundStmt
  |-CXXMethodDecl f 'int ()'
  | |-Overrides: [ B::f 'int ()' ]
  | `-OverrideAttr
  `-CXXMethodDecl g 'void ()' delete implicit-inline
    `-delete message: StringLiteral unevaluated "no"
"#,
    );
    let ids = dump_to_string(&ast, &DumpOptions { ids: true, ..DumpOptions::default() });
    let want = format!(" parent=#d{} friend_undeclared h", tu.raw());
    assert!(ids.contains(&want), "{ids}");
    let want = format!("Overrides: [ B::f #d{} 'int ()' ]", bf.raw());
    assert!(ids.contains(&want), "{ids}");
}

fn var(ast: &mut Ast, s: &str, ty: QualType, init: Option<ExprId>, spec: VarSpec) -> DeclId {
    let tu = ast.tu;
    let h = DeclHead::new(tu, name(ast, s), L);
    let v = ast.builder().var(h, ty, spec, None);
    if let Some(e) = init {
        ast.builder().set_var_init(v, e).unwrap();
    }
    v
}

/// The §5.1 expression lines (each as a variable's initializer; the
/// snippets are clang's, the AST is cut down to the node under test):
///
/// ```c++
/// struct V {};
/// struct B : virtual V { int x = 1; int get(); };
/// B b;
/// V &rv = static_cast<V &>(b);         V *pv = &b;
/// int i = int(1);                      const B &r = B{};
/// B t = B();                           B l{};      // x's NSDMI
/// int g(int a = 4); int ga = g();      int arr[3](1, 2);
/// int mc = b.get();                    // typeid(int); this; [=]{arr;}
/// ```
#[test]
fn class_expressions() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let int = builtin(INT);
    let lv = ValueCategory::LValue;
    let pr = ValueCategory::PRValue;
    let v = record(&mut ast, tu, "V");
    ast.builder().begin_record_definition(v).unwrap();
    ast.builder().complete_record_definition(v, DefinitionBits::empty());
    let v_ty = ast.decls.get(v).ty().unwrap();
    let b = record(&mut ast, tu, "B");
    ast.builder().begin_record_definition(b).unwrap();
    let base = BaseSpec {
        ty: v_ty,
        loc: L,
        is_virtual: true,
        access: Access::None,
        pack_expansion: false,
    };
    ast.decls.set_bases(b, &[base], &mut ast.lists).unwrap();
    let one = ast.builder().int_literal(int, 1, Radix::Decimal, L);
    let h = DeclHead::new(b, name(&ast, "x"), L).access(Access::Public);
    let x = ast.builder().field(h, int, None, 0);
    ast.decls.set_field_init(x, one).unwrap();
    let b_ty = ast.decls.get(b).ty().unwrap();
    let nothrow = fn_ty(&mut ast, QualType::VOID, &[], ExcSpec::Nothrow);
    let cn = ast.names.constructor(ast.types.canonical(b_ty).ty());
    let h = DeclHead::new(b, cn, L).access(Access::Public).flags(DeclFlags::IMPLICIT);
    let spec = FnSpec::new().with_inline(true);
    let k = ast.builder().function(h, DeclKind::CXXConstructor, nothrow, spec, None);
    let dn = ast.names.destructor(ast.types.canonical(b_ty).ty());
    let h = DeclHead::new(b, dn, L).access(Access::Public).flags(DeclFlags::IMPLICIT);
    let dtor = ast.builder().function(h, DeclKind::CXXDestructor, nothrow, spec, None);
    let get_ty = fn_ty(&mut ast, int, &[], ExcSpec::None);
    let ng = name(&ast, "get");
    let get = method(&mut ast, b, DeclKind::CXXMethod, ng, get_ty, FnSpec::new());
    ast.builder().complete_record_definition(b, DefinitionBits::empty());
    let base_index = ast.lists.get(ast.decls.record(b).unwrap().bases)[0];
    let path = Some(ast.lists.push_raw(&[base_index]));

    let bv = var(&mut ast, "b", b_ty, None, VarSpec::new());
    let ref_b =
        |ast: &mut Ast| ast.builder().decl_ref(bv, b_ty, L, None, NonOdrUse::None, ExprFlags::NONE);
    // static_cast<V &>(b)
    let e = ref_b(&mut ast);
    let sc = ast.builder().cast(
        ExprKind::CXXStaticCastExpr,
        CastKind::DerivedToBase,
        e,
        v_ty,
        lv,
        path,
        L,
        ExprFlags::NONE,
    );
    let v_ref = ast.types.lvalue_ref_to(v_ty);
    var(&mut ast, "rv", v_ref, Some(sc), VarSpec::new());
    // int(1)
    let e = ast.builder().int_literal(int, 1, Radix::Decimal, L);
    let fc = ast.builder().functional_cast(CastKind::NoOp, e, int, pr, None, L, L, ExprFlags::NONE);
    var(&mut ast, "i", int, Some(fc), VarSpec::new());
    // const B &r = B{};
    let cb = QualType::new(b_ty.ty(), 1);
    let cb_ref = ast.types.lvalue_ref_to(cb);
    let r = var(&mut ast, "r", cb_ref, None, VarSpec::new());
    let info = ConstructInfo { list: true, zeroing: true, ..ConstructInfo::COMPLETE };
    let tmp = ast.builder().temporary_object(b_ty, k, &[], info, ExprFlags::NONE, L, L);
    let noop = ast.builder().implicit_cast(CastKind::NoOp, tmp, cb, pr);
    let let_ = ast.builder().lifetime_extended_temporary(r, 0, cb, L);
    let mt =
        ast.builder().materialize_temporary(noop, cb, true, StorageDuration::Automatic, Some(let_));
    ast.builder().set_var_init(r, mt).unwrap();
    // B t = B();
    let tmp = ast.builder().temporary_object(
        b_ty,
        k,
        &[],
        ConstructInfo { zeroing: true, ..ConstructInfo::COMPLETE },
        ExprFlags::NONE,
        L,
        L,
    );
    let bt = ast.builder().bind_temporary(tmp, dtor);
    let cl = ast.builder().expr_with_cleanups(bt, true);
    var(&mut ast, "t", b_ty, Some(cl), VarSpec::new());
    // x's default member initializer, as used in `B l{};`
    let di = ast.builder().default_init(x, tu, int, pr, L);
    var(&mut ast, "di", int, Some(di), VarSpec::new());
    // g(int a = 4): the default argument as used in g()
    let int_int = fn_ty(&mut ast, int, &[int], ExcSpec::None);
    let h = DeclHead::new(tu, name(&ast, "g"), L);
    let g = ast.builder().function(h, DeclKind::Function, int_int, FnSpec::new(), None);
    let na = name(&ast, "a");
    let a = ast.builder().param(g, na, L, int, 0, 0);
    ast.builder().set_params(g, &[a]).unwrap();
    let four = ast.builder().int_literal(int, 4, Radix::Decimal, L);
    ast.decls.set_default_arg(a, four).unwrap();
    let da = ast.builder().default_arg(a, tu, int, pr, L);
    var(&mut ast, "ga", int, Some(da), VarSpec::new());
    // int arr[3](1, 2);
    let arr_ty = ast.types.constant_array(int, 3, None);
    let one = ast.builder().int_literal(int, 1, Radix::Decimal, L);
    let two = ast.builder().int_literal(int, 2, Radix::Decimal, L);
    let fill = ast.builder().implicit_value_init(int);
    let pl = ast.builder().paren_list_init(arr_ty, &[one, two], Some(fill), L, L);
    let arr = var(
        &mut ast,
        "arr",
        arr_ty,
        Some(pl),
        VarSpec::new().with_init_style(InitStyle::ParenList),
    );
    // the copy of `arr` in a lambda capture
    let ar = ast.builder().decl_ref(arr, arr_ty, L, None, NonOdrUse::None, ExprFlags::NONE);
    let ov = ast.builder().opaque_value(ar);
    let ul = builtin(15);
    let idx = ast.builder().array_init_index(ul, L);
    let ail = ast.builder().array_init_loop(arr_ty, ov, idx);
    var(&mut ast, "cap", arr_ty, Some(ail), VarSpec::new());
    // b.get()
    let e = ref_b(&mut ast);
    let bm = builtin(31);
    let callee = ast.builder().member(e, get, bm, false, L, None);
    let mc = ast.builder().member_call(callee, &[], int, pr, L, L, ExprFlags::NONE);
    var(&mut ast, "mc", int, Some(mc), VarSpec::new());
    // typeid(int), this
    let tid = ast.builder().typeid_expr(TypeidOperand::Type(int), false, int, L, L);
    var(&mut ast, "ti", int, Some(tid), VarSpec::new());
    let bp = ast.types.pointer_to(b_ty);
    let th = ast.builder().this_expr(bp, true, L);
    var(&mut ast, "th", bp, Some(th), VarSpec::new());

    let got = dump_to_string(&ast, &DumpOptions::default());
    check(
        &got,
        r"TranslationUnitDecl
|-CXXRecordDecl struct V definition
|-CXXRecordDecl struct B definition
| |-virtual public 'V'
| |-FieldDecl x 'int'
| | `-IntegerLiteral 'int' 1
| `-CXXMethodDecl get 'int ()'
|-VarDecl b 'B'
|-VarDecl rv 'V &' cinit
| `-CXXStaticCastExpr 'V' lvalue static_cast<V &> <DerivedToBase (virtual V)>
|   `-DeclRefExpr 'B' lvalue Var 'b' 'B'
|-VarDecl i 'int' cinit
| `-CXXFunctionalCastExpr 'int' functional cast to int <NoOp>
|   `-IntegerLiteral 'int' 1
|-VarDecl r 'const B &' cinit
| `-MaterializeTemporaryExpr 'const B' lvalue extended by Var 'r' 'const B &'
|   `-ImplicitCastExpr 'const B' <NoOp>
|     `-CXXTemporaryObjectExpr 'B' 'void () noexcept' list zeroing
|-VarDecl t 'B' cinit
| `-ExprWithCleanups 'B'
|   `-CXXBindTemporaryExpr 'B' (CXXTemporary)
|     `-CXXTemporaryObjectExpr 'B' 'void () noexcept' zeroing
|-VarDecl di 'int' cinit
| `-CXXDefaultInitExpr 'int' has rewritten init
|   `-IntegerLiteral 'int' 1
|-FunctionDecl g 'int (int)'
| `-ParmVarDecl a 'int' cinit
|   `-IntegerLiteral 'int' 4
|-VarDecl ga 'int' cinit
| `-CXXDefaultArgExpr 'int'
|   `-IntegerLiteral 'int' 4
|-VarDecl arr 'int[3]' parenlistinit
| `-CXXParenListInitExpr 'int[3]'
|   |-array_filler: ImplicitValueInitExpr 'int'
|   |-IntegerLiteral 'int' 1
|   `-IntegerLiteral 'int' 2
|-VarDecl cap 'int[3]' cinit
| `-ArrayInitLoopExpr 'int[3]'
|   |-OpaqueValueExpr 'int[3]' lvalue
|   | `-DeclRefExpr 'int[3]' lvalue Var 'arr' 'int[3]'
|   `-ArrayInitIndexExpr 'unsigned long'
|-VarDecl mc 'int' cinit
| `-CXXMemberCallExpr 'int'
|   `-MemberExpr '<bound member function type>' .get
|     `-DeclRefExpr 'B' lvalue Var 'b' 'B'
|-VarDecl ti 'int' cinit
| `-CXXTypeidExpr 'int' lvalue type_operand='int'
`-VarDecl th 'B *' cinit
  `-CXXThisExpr 'B *' implicit this
",
    );
}
