//! AST dump tests (spec/ast-dump.md): `AstBuilder` reconstructions of the
//! clang-19 goldens under `tests/conformance/ast/` (byte for byte), one
//! golden per node kind, the option matrix, the filter, locations
//! (including macro locations), stability and whitespace hygiene.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    reason = "each test transcribes a C++ snippet node by node"
)]

mod common;

use common::new_ast;
use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::consts::{IntBits, Scalar};
use kiln_ast::decl::{
    Access, BaseSpec, ConstexprKind, DeclFlags, DeclKind, DefinitionBits, EnumData, FieldData,
    FnSpec, FnStorage, LinkageLang, TagKind, VarSpec, VarStorage,
};
use kiln_ast::dump::{DumpOptions, Filter, dump, dump_to_string};
use kiln_ast::expr::{
    BinaryOp, CastKind, ExprFlags, ExprKind, NonOdrUse, Radix, StringKind, TraitKind, UnaryOp,
    ValueCategory,
};
use kiln_ast::names::DeclName;
use kiln_ast::stmt::{CaseData, IfData, IfKind, LoopData, SwitchData};
use kiln_ast::strings::StringEncoding;
use kiln_ast::types::{ExcSpec, FuncInfo};
use kiln_ast::{Ast, ConstValueId, DeclHead, DeclId, ExprId, QualType, StmtId};
use kiln_source::{ExpansionKind, Loc, SourceManager, Span};

const L: Loc = Loc::INVALID;
const INT: QualType = QualType::INT;
const PR: ValueCategory = ValueCategory::PRValue;

/// A golden from `tests/conformance/ast/`, without its trailing newline
/// handling: the dump is compared to the whole file.
macro_rules! golden {
    ($path:literal) => {
        include_str!(concat!("../../../tests/conformance/ast/", $path, ".cpp.expected"))
    };
}

/// Builder shorthands over one AST.
struct Fx {
    ast: Ast,
}

impl Fx {
    fn new() -> Fx {
        Fx { ast: new_ast() }
    }

    fn tu(&self) -> DeclId {
        self.ast.tu
    }

    fn name(&self, s: &str) -> DeclName {
        if s.is_empty() { DeclName::EMPTY } else { DeclName::ident(self.ast.symbols().intern(s)) }
    }

    fn head(&self, ctx: DeclId, s: &str) -> DeclHead {
        DeclHead::new(ctx, self.name(s), L)
    }

    fn fn_ty(&mut self, ret: QualType, params: &[QualType]) -> QualType {
        self.ast.types.function(ret, params, FuncInfo::new(), ExcSpec::None)
    }

    fn ptr(&mut self, q: QualType) -> QualType {
        self.ast.types.pointer_to(q)
    }

    fn int_val(&mut self, ty: QualType, width: u32, v: i128) -> ConstValueId {
        let signed =
            v < 0 || [INT, QualType::LONG, QualType::LONGLONG, QualType::SHORT].contains(&ty);
        let s = Scalar::Int { ty, signed, bits: IntBits::from_i128(width, v) };
        self.ast.consts.push_scalar(s).expect("value")
    }

    fn bool_val(&mut self, v: bool) -> ConstValueId {
        self.ast.consts.push_scalar(Scalar::Bool { ty: QualType::BOOL, value: v }).expect("value")
    }

    fn lit(&mut self, v: u64) -> ExprId {
        self.lit_ty(INT, v)
    }

    fn lit_ty(&mut self, ty: QualType, v: u64) -> ExprId {
        self.ast.builder().int_literal(ty, v, Radix::Decimal, L)
    }

    fn uneval(&mut self, s: &str) -> ExprId {
        let id = self.ast.strings.intern(StringEncoding::Ordinary, s.as_bytes());
        let len = s.len() as u64 + 1;
        let ty = self.ast.types.constant_array(QualType::new(QualType::CHAR.ty(), 1), len, None);
        self.ast.builder().string_literal(ty, StringKind::Unevaluated, id, L)
    }

    /// A reference to `d` with its declared type.
    fn dref(&mut self, d: DeclId) -> ExprId {
        let ty = self.ast.decls.get(d).ty().expect("typed");
        self.ast.builder().decl_ref(d, ty, L, None, NonOdrUse::None, ExprFlags::NONE)
    }

    fn ic(&mut self, ck: CastKind, e: ExprId, ty: QualType) -> ExprId {
        self.ast.builder().implicit_cast(ck, e, ty, PR)
    }

    /// `LValueToRValue` of `e`, at `e`'s type.
    fn rv(&mut self, e: ExprId) -> ExprId {
        let ty = self.ast.exprs.get(e).ty();
        self.ic(CastKind::LValueToRValue, e, ty)
    }

    /// `ref` loaded: `rv(dref(d))`.
    fn load(&mut self, d: DeclId) -> ExprId {
        let e = self.dref(d);
        self.rv(e)
    }

    /// The C++23 implicit move of a returned local: `rv(xvalue NoOp)`.
    fn moved(&mut self, d: DeclId) -> ExprId {
        let e = self.dref(d);
        let ty = self.ast.exprs.get(e).ty();
        let x = self.ast.builder().implicit_cast(CastKind::NoOp, e, ty, ValueCategory::XValue);
        self.rv(x)
    }

    fn bin(&mut self, op: BinaryOp, l: ExprId, r: ExprId, ty: QualType) -> ExprId {
        self.ast.builder().binary(op, l, r, ty, L)
    }

    /// A direct call to function `f`.
    fn call(&mut self, f: DeclId, args: &[ExprId]) -> ExprId {
        let fty = self.ast.decls.get(f).ty().expect("typed");
        let ret = self.ast.types.proto(fty).expect("proto").ret();
        let pty = self.ptr(fty);
        let r = self.dref(f);
        let callee = self.ic(CastKind::FunctionToPointerDecay, r, pty);
        self.ast.builder().call(callee, args, ret, PR, L, L, ExprFlags::NONE)
    }

    fn ret(&mut self, e: Option<ExprId>) -> StmtId {
        self.ast.builder().return_stmt(e, None, L)
    }

    fn block(&mut self, s: &[StmtId]) -> StmtId {
        self.ast.builder().compound(s, L, L)
    }

    fn decl_stmt(&mut self, d: DeclId) -> StmtId {
        self.ast.builder().decl_stmt(&[d], L)
    }

    fn expr_stmt(&mut self, e: ExprId) -> StmtId {
        self.ast.builder().expr_stmt(e)
    }

    /// A function declaration with its parameters (`(name, type, used)`).
    fn func(
        &mut self,
        ctx: DeclId,
        name: &str,
        ret: QualType,
        params: &[(&str, QualType, bool)],
        spec: FnSpec,
        flags: DeclFlags,
    ) -> (DeclId, Vec<DeclId>) {
        let ptys: Vec<QualType> = params.iter().map(|p| p.1).collect();
        let ty = self.fn_ty(ret, &ptys);
        let h = self.head(ctx, name).flags(flags);
        let f = self.ast.builder().function(h, DeclKind::Function, ty, spec, None);
        let mut ps = Vec::new();
        for (i, &(n, t, used)) in params.iter().enumerate() {
            let n = self.name(n);
            let p = self.ast.builder().param(f, n, L, t, i as u16, 0);
            if used {
                self.ast.decls.set_flags(p, DeclFlags::USED);
            }
            ps.push(p);
        }
        self.ast.builder().set_params(f, &ps).expect("params");
        (f, ps)
    }

    fn body(&mut self, f: DeclId, stmts: &[StmtId]) {
        let b = self.block(stmts);
        self.ast.builder().set_body(f, b).expect("body");
    }

    fn var(
        &mut self,
        ctx: DeclId,
        name: &str,
        ty: QualType,
        spec: VarSpec,
        flags: DeclFlags,
    ) -> DeclId {
        let h = self.head(ctx, name).flags(flags);
        self.ast.builder().var(h, ty, spec, None)
    }

    fn init(&mut self, v: DeclId, e: ExprId) {
        self.ast.builder().set_var_init(v, e).expect("init");
    }

    fn record(&mut self, ctx: DeclId, name: &str, tag: TagKind, flags: DeclFlags) -> DeclId {
        let h = self.head(ctx, name).flags(flags);
        self.ast.builder().record(h, tag, None)
    }

    fn field(
        &mut self,
        r: DeclId,
        name: &str,
        ty: QualType,
        width: Option<ExprId>,
        flags: DeclFlags,
    ) -> DeclId {
        let h = self.head(r, name).flags(flags);
        self.ast.builder().field(h, ty, width, 0)
    }

    /// An enumeration (`flags`: `EnumData::SCOPED`...; `fixed`: the
    /// written underlying type).
    fn enumeration(
        &mut self,
        ctx: DeclId,
        name: &str,
        flags: u32,
        fixed: Option<QualType>,
        df: DeclFlags,
    ) -> DeclId {
        let h = self.head(ctx, name).flags(df);
        self.ast.builder().enumeration(h, flags, fixed.map(|t| (t, t)), None)
    }

    /// An enumerator with its computed value.
    fn enumerator(
        &mut self,
        e: DeclId,
        name: &str,
        init: Option<ExprId>,
        value: i128,
        flags: DeclFlags,
    ) -> DeclId {
        let h = self.head(e, name).flags(flags);
        let d = self.ast.builder().enumerator(h, init);
        let ety = self.ast.decls.get(e).ty().expect("enum type");
        let width = if value.unsigned_abs() >> 31 == 0 { 32 } else { 64 };
        let v = self.int_val(ety, width, value);
        self.ast.decls.set_enumerator_value(d, v).expect("value");
        d
    }

    fn typedef(
        &mut self,
        ctx: DeclId,
        name: &str,
        kind: DeclKind,
        under: QualType,
        flags: DeclFlags,
    ) -> DeclId {
        let h = self.head(ctx, name).flags(flags);
        self.ast.builder().typedef(h, kind, under, None)
    }

    /// `ConstantExpr` of `e` with the integer value `v` (type of `e`).
    fn constant(&mut self, e: ExprId, width: u32, v: i128) -> ExprId {
        let ty = self.ast.exprs.get(e).ty();
        let c = self.int_val(ty, width, v);
        self.ast.builder().constant_expr(e, Some(c))
    }

    fn dump(&self) -> String {
        dump_to_string(&self.ast, &DumpOptions::default())
    }

    fn dump_with(&self, opts: &DumpOptions<'_>) -> String {
        dump_to_string(&self.ast, opts)
    }
}

const NONE: DeclFlags = DeclFlags::empty();
const USED: DeclFlags = DeclFlags::USED;
const REFD: DeclFlags = DeclFlags::REFERENCED;

fn cinit() -> VarSpec {
    VarSpec::new()
}

/// Asserts equality with a readable diff on failure.
#[track_caller]
fn check(got: &str, want: &str) {
    if got != want {
        let first = got.lines().zip(want.lines()).enumerate().find(|(_, (g, w))| g != w);
        let msg = first.map_or_else(String::new, |(i, (g, w))| {
            format!("first difference at line {}:\n  got:  {g:?}\n  want: {w:?}\n", i + 1)
        });
        panic!("dump mismatch\n{msg}--- got ---\n{got}--- want ---\n{want}");
    }
}

// ---------------------------------------------------------------------------
// Conformance goldens (tests/conformance/ast/), byte for byte
// ---------------------------------------------------------------------------

/// spec/ast-dump.md §4 = `dcl.dcl/spec-example`.
fn spec_example() -> Fx {
    let mut x = Fx::new();
    let tu = x.tu();
    // struct P { int x; int y; };
    let p = x.record(tu, "P", TagKind::Struct, REFD);
    x.ast.builder().begin_record_definition(p).expect("defn");
    let px = x.field(p, "x", INT, None, REFD);
    x.field(p, "y", INT, None, NONE);
    x.ast.builder().complete_record_definition(p, DefinitionBits::empty());
    let pty = x.ast.decls.get(p).ty().expect("record type");
    // enum E { A, B = 5 };
    let e = x.enumeration(tu, "E", 0, None, NONE);
    x.ast.builder().begin_enum_definition(e).expect("defn");
    x.enumerator(e, "A", None, 0, NONE);
    let five = x.lit(5);
    let c5 = x.constant(five, 32, 5);
    let c5 = x.ic(CastKind::IntegralCast, c5, QualType::UINT);
    x.enumerator(e, "B", Some(c5), 5, NONE);
    let uint = Some((QualType::UINT, QualType::UINT));
    x.ast.builder().complete_enum_definition(e, uint).expect("complete");
    // constexpr int sq(int x) { return x * x; }
    let spec = FnSpec::new().with_constexpr_kind(ConstexprKind::Constexpr);
    let (sq, sp) = x.func(tu, "sq", INT, &[("x", INT, true)], spec, USED);
    let (l, r) = (x.load(sp[0]), x.load(sp[0]));
    let m = x.bin(BinaryOp::Mul, l, r, INT);
    let rs = x.ret(Some(m));
    x.body(sq, &[rs]);
    // static_assert(sq(3) == 9, "msg");
    let three = x.lit(3);
    let c = x.call(sq, &[three]);
    let nine = x.lit(9);
    let cond = x.bin(BinaryOp::EQ, c, nine, QualType::BOOL);
    let msg = x.uneval("msg");
    x.ast.builder().static_assert(tu, L, cond, Some(msg));
    // typedef int Arr[3];
    let arr3 = x.ast.types.constant_array(INT, 3, None);
    let arr = x.typedef(tu, "Arr", DeclKind::Typedef, arr3, REFD);
    // const Arr ca = {1, 2, 3};
    let arr_ty = x.ast.decls.get(arr).ty().expect("typedef type");
    let carr = QualType::new(arr_ty.ty(), 1);
    let ca = x.var(tu, "ca", carr, cinit(), NONE);
    let es = [x.lit(1), x.lit(2), x.lit(3)];
    let il = x.ast.builder().init_list(carr, &es, None, None, L, L);
    x.init(ca, il);
    // long f(char c, int *p) { ... }
    let ip = x.ptr(INT);
    let long = QualType::LONG;
    let (f, fp) =
        x.func(tu, "f", long, &[("c", QualType::CHAR, true), ("p", ip, true)], FnSpec::new(), REFD);
    //   int a[3] = {1};
    let av = x.var(f, "a", arr3, cinit(), USED);
    let filler = x.ast.builder().implicit_value_init(INT);
    let one = x.lit(1);
    let il = x.ast.builder().init_list(arr3, &[one], Some(filler), None, L, L);
    x.init(av, il);
    let s1 = x.decl_stmt(av);
    //   P q = {1, 2};
    let q = x.var(f, "q", pty, cinit(), USED);
    let es = [x.lit(1), x.lit(2)];
    let il = x.ast.builder().init_list(pty, &es, None, None, L, L);
    x.init(q, il);
    let s2 = x.decl_stmt(q);
    //   for (int i = 0; i < 3; ++i) a[i] += c;
    let i = x.var(f, "i", INT, cinit(), USED);
    let zero = x.lit(0);
    x.init(i, zero);
    let init = x.decl_stmt(i);
    let li = x.load(i);
    let three = x.lit(3);
    let cond = x.bin(BinaryOp::LT, li, three, QualType::BOOL);
    let ir = x.dref(i);
    let inc = x.ast.builder().unary(UnaryOp::PreInc, ir, INT, L, ExprFlags::NONE);
    let ar = x.dref(av);
    let decay = x.ic(CastKind::ArrayToPointerDecay, ar, ip);
    let li = x.load(i);
    let sub = x.ast.builder().subscript(decay, li, INT, L);
    let lc = x.load(fp[0]);
    let lc = x.ic(CastKind::IntegralCast, lc, INT);
    let ca_ = x.ast.builder().compound_assign(BinaryOp::AddAssign, sub, lc, INT, INT, INT, L);
    let body = x.expr_stmt(ca_);
    let data = LoopData {
        init: Some(init),
        cond_var: None,
        cond: Some(cond),
        inc: Some(inc),
        body: Some(body),
    };
    let s3 = x.ast.builder().for_stmt(data, L);
    //   if (p) return q.x;
    let lp = x.load(fp[1]);
    let pb = x.ic(CastKind::PointerToBoolean, lp, QualType::BOOL);
    let qr = x.dref(q);
    let mx = x.ast.builder().member(qr, px, INT, false, L, None);
    let mx = x.rv(mx);
    let mx = x.ic(CastKind::IntegralCast, mx, long);
    let then = x.ret(Some(mx));
    let data = IfData { init: None, cond_var: None, cond: Some(pb), then: Some(then), els: None };
    let s4 = x.ast.builder().if_stmt(IfKind::Ordinary, data, L);
    //   return (long)sizeof(P) + f(c, nullptr);
    let so = x.ast.builder().trait_of_type(TraitKind::SizeOf, pty, QualType::ULONG, L, L);
    let so = x.ast.builder().cast(
        ExprKind::ImplicitCastExpr,
        CastKind::IntegralCast,
        so,
        long,
        PR,
        None,
        L,
        ExprFlags::PART_OF_EXPLICIT_CAST,
    );
    let so = x.ast.builder().cast(
        ExprKind::CStyleCastExpr,
        CastKind::NoOp,
        so,
        long,
        PR,
        None,
        L,
        ExprFlags::NONE,
    );
    let lc = x.load(fp[0]);
    let np = x.ast.builder().nullptr_literal(QualType::NULLPTR, L);
    let np = x.ic(CastKind::NullToPointer, np, ip);
    let rc = x.call(f, &[lc, np]);
    let sum = x.bin(BinaryOp::Add, so, rc, long);
    let s5 = x.ret(Some(sum));
    x.body(f, &[s1, s2, s3, s4, s5]);
    x
}

#[test]
fn golden_spec_example() {
    check(&spec_example().dump(), golden!("dcl.dcl/spec-example"));
}

#[test]
fn golden_static_assert() {
    let mut x = Fx::new();
    let tu = x.tu();
    let t = x.ast.builder().bool_literal(QualType::BOOL, true, L);
    x.ast.builder().static_assert(tu, L, t, None);
    let (a, b) = (x.lit(1), x.lit(1));
    let sum = x.bin(BinaryOp::Add, a, b, INT);
    let two = x.lit(2);
    let eq = x.bin(BinaryOp::EQ, sum, two, QualType::BOOL);
    let msg = x.uneval("math");
    x.ast.builder().static_assert(tu, L, eq, Some(msg));
    let so =
        x.ast.builder().trait_of_type(TraitKind::SizeOf, QualType::CHAR, QualType::ULONG, L, L);
    let one = x.lit(1);
    let one = x.ic(CastKind::IntegralCast, one, QualType::ULONG);
    let eq = x.bin(BinaryOp::EQ, so, one, QualType::BOOL);
    let msg = x.uneval("bytes");
    x.ast.builder().static_assert(tu, L, eq, Some(msg));
    check(&x.dump(), golden!("dcl.pre/static-assert"));
}

#[test]
fn golden_storage() {
    let mut x = Fx::new();
    let tu = x.tu();
    let st = VarSpec::new().with_storage(VarStorage::Static);
    let ex = VarSpec::new().with_storage(VarStorage::Extern);
    let v = x.var(tu, "file_static", INT, st, NONE);
    let e = x.lit(1);
    x.init(v, e);
    x.var(tu, "declared", INT, ex, NONE);
    let inl = x.var(tu, "inl", INT, VarSpec::new().with_inline(true), USED);
    let e = x.lit(2);
    x.init(inl, e);
    let v = x.var(tu, "defined_extern", INT, ex, NONE);
    let e = x.lit(3);
    x.init(v, e);
    let (f, _) = x.func(tu, "counter", INT, &[], FnSpec::new(), NONE);
    let calls = x.var(f, "calls", INT, st, USED);
    let s1 = x.decl_stmt(calls);
    let r = x.dref(calls);
    let inc = x.ast.builder().unary(UnaryOp::PreInc, r, INT, L, ExprFlags::NONE);
    let inc = x.rv(inc);
    let li = x.load(inl);
    let sum = x.bin(BinaryOp::Add, inc, li, INT);
    let s2 = x.ret(Some(sum));
    x.body(f, &[s1, s2]);
    check(&x.dump(), golden!("dcl.stc/storage"));
}

#[test]
fn golden_extern_c() {
    let mut x = Fx::new();
    let tu = x.tu();
    let c1 = x.ast.builder().linkage_spec(tu, LinkageLang::C, L);
    let (c_func, _) = x.func(c1, "c_func", INT, &[("", INT, false)], FnSpec::new(), USED);
    let c2 = x.ast.builder().linkage_spec(tu, LinkageLang::C, L);
    let c_var = x.var(c2, "c_var", INT, VarSpec::new(), USED);
    x.func(c2, "c_other", QualType::VOID, &[], FnSpec::new(), NONE);
    let cxx = x.ast.builder().linkage_spec(tu, LinkageLang::Cxx, L);
    let (cxx_func, _) = x.func(cxx, "cxx_func", INT, &[], FnSpec::new(), USED);
    let (u, _) = x.func(tu, "use", INT, &[], FnSpec::new(), NONE);
    let lv = x.load(c_var);
    let a = x.call(c_func, &[lv]);
    let b = x.call(cxx_func, &[]);
    let sum = x.bin(BinaryOp::Add, a, b, INT);
    let r = x.ret(Some(sum));
    x.body(u, &[r]);
    check(&x.dump(), golden!("dcl.link/extern-c"));
}

#[test]
fn golden_deleted() {
    let mut x = Fx::new();
    let tu = x.tu();
    let del = FnSpec::new().with_deleted(true);
    x.func(tu, "nope", QualType::VOID, &[("", INT, false)], del, NONE);
    let (n2, _) =
        x.func(tu, "nope", QualType::VOID, &[("", QualType::LONG, false)], FnSpec::new(), NONE);
    x.body(n2, &[]);
    let inl = FnSpec::new().with_inline(true);
    let (fin, ps) = x.func(tu, "in", INT, &[("x", INT, true)], inl, USED);
    let m = x.moved(ps[0]);
    let r = x.ret(Some(m));
    x.body(fin, &[r]);
    let st = FnSpec::new().with_storage(FnStorage::Static);
    let (fst, _) = x.func(tu, "st", INT, &[], st, USED);
    let z = x.lit(0);
    let r = x.ret(Some(z));
    x.body(fst, &[r]);
    let ex = FnSpec::new().with_storage(FnStorage::Extern);
    let (fex, _) = x.func(tu, "ex", INT, &[], ex, USED);
    let (call, _) = x.func(tu, "call", INT, &[], FnSpec::new(), NONE);
    let a = x.call(fst, &[]);
    let a = x.call(fin, &[a]);
    let b = x.call(fex, &[]);
    let sum = x.bin(BinaryOp::Add, a, b, INT);
    let r = x.ret(Some(sum));
    x.body(call, &[r]);
    check(&x.dump(), golden!("dcl.fct.def.delete/deleted"));
}

#[test]
fn golden_implicit_move() {
    let mut x = Fx::new();
    let tu = x.tu();
    let bx = x.record(tu, "Box", TagKind::Struct, NONE);
    x.ast.builder().begin_record_definition(bx).expect("defn");
    x.field(bx, "v", INT, None, NONE);
    x.ast.builder().complete_record_definition(bx, DefinitionBits::empty());
    let (local, _) = x.func(tu, "local", INT, &[], FnSpec::new(), NONE);
    let xv = x.var(local, "x", INT, cinit(), USED);
    let one = x.lit(1);
    x.init(xv, one);
    let s1 = x.decl_stmt(xv);
    let m = x.moved(xv);
    let s2 = x.ret(Some(m));
    x.body(local, &[s1, s2]);
    let (param, ps) = x.func(tu, "param", INT, &[("p", INT, true)], FnSpec::new(), NONE);
    let m = x.moved(ps[0]);
    let r = x.ret(Some(m));
    x.body(param, &[r]);
    let g = x.var(tu, "global_g", INT, VarSpec::new(), USED);
    let (global, _) = x.func(tu, "global", INT, &[], FnSpec::new(), NONE);
    let lg = x.load(g);
    let r = x.ret(Some(lg));
    x.body(global, &[r]);
    check(&x.dump(), golden!("stmt.return/implicit-move"));
}

#[test]
fn golden_filter_local() {
    let mut x = Fx::new();
    let tu = x.tu();
    let (f, ps) = x.func(tu, "counter_next", INT, &[("step", INT, true)], FnSpec::new(), NONE);
    let st = VarSpec::new().with_storage(VarStorage::Static);
    let counter = x.var(f, "counter", INT, st, USED);
    let z = x.lit(0);
    x.init(counter, z);
    let s1 = x.decl_stmt(counter);
    let lhs = x.dref(counter);
    let rhs = x.load(ps[0]);
    let ca = x.ast.builder().compound_assign(BinaryOp::AddAssign, lhs, rhs, INT, INT, INT, L);
    let s2 = x.expr_stmt(ca);
    let lc = x.load(counter);
    let s3 = x.ret(Some(lc));
    x.body(f, &[s1, s2, s3]);
    let (other, _) = x.func(tu, "other", INT, &[], FnSpec::new(), NONE);
    let z = x.lit(0);
    let r = x.ret(Some(z));
    x.body(other, &[r]);
    let filter = Filter { name: Some("counter".into()), kind: None };
    let opts = DumpOptions { filter: Some(filter), ..DumpOptions::default() };
    check(&x.dump_with(&opts), golden!("dcl.dcl/filter-local"));
}

#[test]
fn golden_unscoped_enum_values() {
    let mut x = Fx::new();
    let tu = x.tu();
    let color = x.enumeration(tu, "Color", 0, None, REFD);
    x.ast.builder().begin_enum_definition(color).expect("defn");
    let red = x.enumerator(color, "Red", None, 0, REFD);
    let four = x.lit(4);
    let four = x.constant(four, 32, 4);
    x.enumerator(color, "Green", Some(four), 4, NONE);
    let blue = x.enumerator(color, "Blue", None, 5, REFD);
    let two = x.lit(2);
    let neg = x.ast.builder().unary(UnaryOp::Minus, two, INT, L, ExprFlags::NONE);
    let neg = x.constant(neg, 32, -2);
    x.enumerator(color, "Neg", Some(neg), -2, NONE);
    let after = x.enumerator(color, "After", None, -1, REFD);
    x.ast.builder().complete_enum_definition(color, Some((INT, INT))).expect("complete");
    let cty = x.ast.decls.get(color).ty().expect("enum type");
    let (pick, ps) = x.func(tu, "pick", INT, &[("c", cty, true)], FnSpec::new(), NONE);
    let lc = x.load(ps[0]);
    let lc = x.ic(CastKind::IntegralCast, lc, INT);
    let b = x.dref(blue);
    let b = x.ic(CastKind::IntegralCast, b, INT);
    let eq = x.bin(BinaryOp::EQ, lc, b, QualType::BOOL);
    let (a, r) = (x.dref(after), x.dref(red));
    let c = x.ast.builder().conditional(eq, a, r, cty, L);
    let c = x.ic(CastKind::IntegralCast, c, INT);
    let r = x.ret(Some(c));
    x.body(pick, &[r]);
    check(&x.dump(), golden!("dcl.enum/unscoped-values"));
}

#[test]
fn golden_scoped_fixed_enums() {
    let mut x = Fx::new();
    let tu = x.tu();
    let small = x.enumeration(tu, "Small", EnumData::SCOPED, Some(QualType::UCHAR), REFD);
    x.ast.builder().begin_enum_definition(small).expect("defn");
    x.enumerator(small, "a", None, 0, NONE);
    let three = x.lit(3);
    let three = x.ic(CastKind::IntegralCast, three, QualType::UCHAR);
    let three = x.constant(three, 8, 3);
    x.enumerator(small, "b", Some(three), 3, NONE);
    x.enumerator(small, "c", None, 4, NONE);
    x.ast.builder().complete_enum_definition(small, None).expect("complete");
    let flags = EnumData::SCOPED | EnumData::SCOPED_STRUCT;
    let wide = x.enumeration(tu, "Wide", flags, Some(QualType::LONGLONG), NONE);
    x.ast.builder().begin_enum_definition(wide).expect("defn");
    let one = x.lit_ty(QualType::LONGLONG, 1);
    let forty = x.lit(40);
    let shl = x.bin(BinaryOp::Shl, one, forty, QualType::LONGLONG);
    let shl = x.constant(shl, 64, 1 << 40);
    x.enumerator(wide, "big", Some(shl), 1 << 40, NONE);
    x.ast.builder().complete_enum_definition(wide, None).expect("complete");
    let plain = x.enumeration(tu, "Plain", 0, Some(QualType::SHORT), NONE);
    x.ast.builder().begin_enum_definition(plain).expect("defn");
    x.enumerator(plain, "p0", None, 0, NONE);
    x.ast.builder().complete_enum_definition(plain, None).expect("complete");
    let sty = x.ast.decls.get(small).ty().expect("enum type");
    let (f, ps) = x.func(tu, "to_int", INT, &[("s", sty, true)], FnSpec::new(), NONE);
    let r = x.dref(ps[0]);
    let r = x.ast.builder().cast(
        ExprKind::ImplicitCastExpr,
        CastKind::LValueToRValue,
        r,
        sty,
        PR,
        None,
        L,
        ExprFlags::PART_OF_EXPLICIT_CAST,
    );
    let c = x.ast.builder().cast(
        ExprKind::CStyleCastExpr,
        CastKind::IntegralCast,
        r,
        INT,
        PR,
        None,
        L,
        ExprFlags::NONE,
    );
    let r = x.ret(Some(c));
    x.body(f, &[r]);
    check(&x.dump(), golden!("dcl.enum/scoped-fixed"));
}

#[test]
fn golden_if_constexpr_consteval() {
    let mut x = Fx::new();
    let tu = x.tu();
    let ce = FnSpec::new().with_constexpr_kind(ConstexprKind::Constexpr);
    let (pick, ps) = x.func(tu, "pick", INT, &[("v", INT, true)], ce, USED);
    let v = ps[0];
    // if constexpr (sizeof(int) == 4) { v += 1; } else { v -= 1; }
    let so = x.ast.builder().trait_of_type(TraitKind::SizeOf, INT, QualType::ULONG, L, L);
    let four = x.lit(4);
    let four = x.ic(CastKind::IntegralCast, four, QualType::ULONG);
    let eq = x.bin(BinaryOp::EQ, so, four, QualType::BOOL);
    let t = x.bool_val(true);
    let cond = x.ast.builder().constant_expr(eq, Some(t));
    let arm = |x: &mut Fx, op| {
        let r = x.dref(v);
        let one = x.lit(1);
        let e = x.ast.builder().compound_assign(op, r, one, INT, INT, INT, L);
        let s = x.expr_stmt(e);
        x.block(&[s])
    };
    let then = arm(&mut x, BinaryOp::AddAssign);
    let els = arm(&mut x, BinaryOp::SubAssign);
    let data =
        IfData { init: None, cond_var: None, cond: Some(cond), then: Some(then), els: Some(els) };
    let s1 = x.ast.builder().if_stmt(IfKind::Constexpr, data, L);
    // if consteval { return v * 2; } else { return v; }
    let lv = x.load(v);
    let two = x.lit(2);
    let m = x.bin(BinaryOp::Mul, lv, two, INT);
    let r = x.ret(Some(m));
    let then = x.block(&[r]);
    let mv = x.moved(v);
    let r = x.ret(Some(mv));
    let els = x.block(&[r]);
    let data = IfData { init: None, cond_var: None, cond: None, then: Some(then), els: Some(els) };
    let s2 = x.ast.builder().if_stmt(IfKind::Consteval, data, L);
    x.body(pick, &[s1, s2]);
    // constexpr int k = pick(3);
    let k = x.var(
        tu,
        "k",
        QualType::new(QualType::INT.ty(), 1),
        VarSpec::new().with_constexpr(true),
        NONE,
    );
    let three = x.lit(3);
    let c = x.call(pick, &[three]);
    x.init(k, c);
    let eight = x.int_val(INT, 32, 8);
    x.ast.decls.begin_eval(k).expect("not evaluated");
    x.ast.decls.finish_eval(k, Some(eight));
    // int rt(int v) { if !consteval { return pick(v); } }
    let (rt, ps) = x.func(tu, "rt", INT, &[("v", INT, true)], FnSpec::new(), NONE);
    let lv = x.load(ps[0]);
    let c = x.call(pick, &[lv]);
    let r = x.ret(Some(c));
    let then = x.block(&[r]);
    let data = IfData { init: None, cond_var: None, cond: None, then: Some(then), els: None };
    let s = x.ast.builder().if_stmt(IfKind::NotConsteval, data, L);
    x.body(rt, &[s]);
    check(&x.dump(), golden!("stmt.if/if-constexpr-consteval"));
}

#[test]
fn golden_if_init_else() {
    let mut x = Fx::new();
    let tu = x.tu();
    let (f, ps) = x.func(tu, "classify", INT, &[("v", INT, true)], FnSpec::new(), NONE);
    let v = ps[0];
    let r = x.var(f, "r", INT, cinit(), USED);
    let lv = x.load(v);
    let three = x.lit(3);
    let rem = x.bin(BinaryOp::Rem, lv, three, INT);
    x.init(r, rem);
    let init = x.decl_stmt(r);
    let lr = x.load(r);
    let zero = x.lit(0);
    let cond = x.bin(BinaryOp::EQ, lr, zero, QualType::BOOL);
    let zero = x.lit(0);
    let then = x.ret(Some(zero));
    let lv = x.load(v);
    let zero = x.lit(0);
    let lt = x.bin(BinaryOp::LT, lv, zero, QualType::BOOL);
    let one = x.lit(1);
    let neg = x.ast.builder().unary(UnaryOp::Minus, one, INT, L, ExprFlags::NONE);
    let r2 = x.ret(Some(neg));
    let data = IfData { init: None, cond_var: None, cond: Some(lt), then: Some(r2), els: None };
    let inner = x.ast.builder().if_stmt(IfKind::Ordinary, data, L);
    let data = IfData {
        init: Some(init),
        cond_var: None,
        cond: Some(cond),
        then: Some(then),
        els: Some(inner),
    };
    let s1 = x.ast.builder().if_stmt(IfKind::Ordinary, data, L);
    let w = x.var(f, "w", INT, cinit(), USED);
    let lv = x.load(v);
    let one = x.lit(1);
    let sub = x.bin(BinaryOp::Sub, lv, one, INT);
    x.init(w, sub);
    let lw = x.load(w);
    let cond = x.ic(CastKind::IntegralToBoolean, lw, QualType::BOOL);
    let mw = x.moved(w);
    let then = x.ret(Some(mw));
    let data =
        IfData { init: None, cond_var: Some(w), cond: Some(cond), then: Some(then), els: None };
    let s2 = x.ast.builder().if_stmt(IfKind::Ordinary, data, L);
    let one = x.lit(1);
    let s3 = x.ret(Some(one));
    x.body(f, &[s1, s2, s3]);
    check(&x.dump(), golden!("stmt.if/if-init-else"));
}

#[test]
fn golden_while_do() {
    let mut x = Fx::new();
    let tu = x.tu();
    let (f, ps) = x.func(tu, "count", INT, &[("n", INT, true)], FnSpec::new(), NONE);
    let c = x.var(f, "c", INT, cinit(), USED);
    let zero = x.lit(0);
    x.init(c, zero);
    let s1 = x.decl_stmt(c);
    // while (int m = n - c) { if (m == 2) continue; ++c; }
    let m = x.var(f, "m", INT, cinit(), USED);
    let (ln, lc) = (x.load(ps[0]), x.load(c));
    let sub = x.bin(BinaryOp::Sub, ln, lc, INT);
    x.init(m, sub);
    let lm = x.load(m);
    let cond = x.ic(CastKind::IntegralToBoolean, lm, QualType::BOOL);
    let lm = x.load(m);
    let two = x.lit(2);
    let eq = x.bin(BinaryOp::EQ, lm, two, QualType::BOOL);
    let cont = x.ast.builder().continue_stmt(L);
    let data = IfData { init: None, cond_var: None, cond: Some(eq), then: Some(cont), els: None };
    let i = x.ast.builder().if_stmt(IfKind::Ordinary, data, L);
    let rc = x.dref(c);
    let inc = x.ast.builder().unary(UnaryOp::PreInc, rc, INT, L, ExprFlags::NONE);
    let inc = x.expr_stmt(inc);
    let body = x.block(&[i, inc]);
    let data =
        LoopData { cond_var: Some(m), cond: Some(cond), body: Some(body), ..LoopData::default() };
    let s2 = x.ast.builder().while_stmt(data, L);
    // do { c--; } while (c > 0);
    let rc = x.dref(c);
    let dec = x.ast.builder().unary(UnaryOp::PostDec, rc, INT, L, ExprFlags::NONE);
    let dec = x.expr_stmt(dec);
    let body = x.block(&[dec]);
    let lc = x.load(c);
    let zero = x.lit(0);
    let gt = x.bin(BinaryOp::GT, lc, zero, QualType::BOOL);
    let s3 = x.ast.builder().do_stmt(body, gt, L);
    // while (true) break;
    let t = x.ast.builder().bool_literal(QualType::BOOL, true, L);
    let brk = x.ast.builder().break_stmt(L);
    let data = LoopData { cond: Some(t), body: Some(brk), ..LoopData::default() };
    let s4 = x.ast.builder().while_stmt(data, L);
    let mc = x.moved(c);
    let s5 = x.ret(Some(mc));
    x.body(f, &[s1, s2, s3, s4, s5]);
    check(&x.dump(), golden!("stmt.while/while-do"));
}

#[test]
fn golden_switch_cases() {
    let mut x = Fx::new();
    let tu = x.tu();
    let op = x.enumeration(tu, "Op", 0, None, REFD);
    x.ast.builder().begin_enum_definition(op).expect("defn");
    let ks: Vec<DeclId> = ["Add", "Sub", "Neg"]
        .iter()
        .zip(0..)
        .map(|(n, v)| x.enumerator(op, n, None, v, REFD))
        .collect();
    x.ast
        .builder()
        .complete_enum_definition(op, Some((QualType::UINT, QualType::UINT)))
        .expect("complete");
    let opty = x.ast.decls.get(op).ty().expect("enum type");
    let params = [("op", opty, true), ("a", INT, true), ("b", INT, true)];
    let (f, ps) = x.func(tu, "eval", INT, &params, FnSpec::new(), NONE);
    let (pop, pa, pb) = (ps[0], ps[1], ps[2]);
    let label = |x: &mut Fx, k: DeclId, v: i128| {
        let r = x.dref(k);
        let r = x.ic(CastKind::IntegralCast, r, INT);
        x.constant(r, 32, v)
    };
    // switch (int t = a; op) { ... }
    let t = x.var(f, "t", INT, cinit(), USED);
    let la = x.load(pa);
    x.init(t, la);
    let init = x.decl_stmt(t);
    let lop = x.load(pop);
    let cond = x.ic(CastKind::IntegralCast, lop, INT);
    let (lt, lb) = (x.load(t), x.load(pb));
    let sum = x.bin(BinaryOp::Add, lt, lb, INT);
    let r = x.ret(Some(sum));
    let lhs = label(&mut x, ks[0], 0);
    let c1 = x.ast.builder().case_stmt(CaseData { lhs, rhs: None, sub: r }, L);
    let ra = x.dref(pa);
    let la = x.load(pa);
    let neg = x.ast.builder().unary(UnaryOp::Minus, la, INT, L, ExprFlags::NONE);
    let asg = x.ast.builder().binary(BinaryOp::Assign, ra, neg, INT, L);
    let asg = x.expr_stmt(asg);
    let lhs = label(&mut x, ks[2], 2);
    let c3 = x.ast.builder().case_stmt(CaseData { lhs, rhs: None, sub: asg }, L);
    let lhs = label(&mut x, ks[1], 1);
    let c2 = x.ast.builder().case_stmt(CaseData { lhs, rhs: None, sub: c3 }, L);
    let ra = x.dref(pa);
    let one = x.lit(1);
    let ca = x.ast.builder().compound_assign(BinaryOp::AddAssign, ra, one, INT, INT, INT, L);
    let ca = x.expr_stmt(ca);
    let brk = x.ast.builder().break_stmt(L);
    let def = x.ast.builder().default_stmt(brk, L);
    let body = x.block(&[c1, c2, ca, def]);
    let cases = x.ast.builder().list(&[c1, c2, c3, def]);
    let data = SwitchData { init: Some(init), cond_var: None, cond, body, cases };
    let s1 = x.ast.builder().switch_stmt(data, L);
    // switch (b) case 1: return 1;
    let lb = x.load(pb);
    let one = x.lit(1);
    let r = x.ret(Some(one));
    let one = x.lit(1);
    let lhs = x.constant(one, 32, 1);
    let c = x.ast.builder().case_stmt(CaseData { lhs, rhs: None, sub: r }, L);
    let cases = x.ast.builder().list(&[c]);
    let data = SwitchData { init: None, cond_var: None, cond: lb, body: c, cases };
    let s2 = x.ast.builder().switch_stmt(data, L);
    let ma = x.moved(pa);
    let s3 = x.ret(Some(ma));
    x.body(f, &[s1, s2, s3]);
    check(&x.dump(), golden!("stmt.switch/switch-cases"));
}

#[test]
fn golden_bitfield_access() {
    let mut x = Fx::new();
    let tu = x.tu();
    let fl = x.record(tu, "Flags", TagKind::Struct, REFD);
    x.ast.builder().begin_record_definition(fl).expect("defn");
    let bits = |x: &mut Fx, name: &str, ty: QualType, w: u64, flags: DeclFlags| {
        let e = x.lit(w);
        let e = x.constant(e, 32, i128::from(w));
        x.field(fl, name, ty, Some(e), flags)
    };
    let fa = bits(&mut x, "a", QualType::UINT, 3, REFD);
    let fb = bits(&mut x, "b", INT, 5, REFD);
    bits(&mut x, "", QualType::UINT, 0, NONE);
    let fc = bits(&mut x, "c", QualType::BOOL, 1, REFD);
    x.ast.builder().complete_record_definition(fl, DefinitionBits::empty());
    let fty = x.ast.decls.get(fl).ty().expect("record type");
    let params = [("f", fty, true), ("which", QualType::BOOL, true)];
    let (touch, ps) = x.func(tu, "touch", INT, &params, FnSpec::new(), NONE);
    let (pf, pw) = (ps[0], ps[1]);
    let mem = |x: &mut Fx, m: DeclId| {
        let r = x.dref(pf);
        let ty = x.ast.decls.get(m).ty().expect("field type");
        x.ast.builder().member(r, m, ty, false, L, None)
    };
    let uint = QualType::UINT;
    // f.a = 5;
    let ma = mem(&mut x, fa);
    let five = x.lit(5);
    let five = x.ic(CastKind::IntegralCast, five, uint);
    let s1 = x.ast.builder().binary(BinaryOp::Assign, ma, five, uint, L);
    let s1 = x.expr_stmt(s1);
    // ++f.b;
    let mb = mem(&mut x, fb);
    let s2 = x.ast.builder().unary(UnaryOp::PreInc, mb, INT, L, ExprFlags::NONE);
    let s2 = x.expr_stmt(s2);
    // f.b += 2;
    let mb = mem(&mut x, fb);
    let two = x.lit(2);
    let s3 = x.ast.builder().compound_assign(BinaryOp::AddAssign, mb, two, INT, INT, INT, L);
    let s3 = x.expr_stmt(s3);
    // (which ? f.a : f.a) = 1;
    let lw = x.load(pw);
    let (m1, m2) = (mem(&mut x, fa), mem(&mut x, fa));
    let c = x.ast.builder().conditional(lw, m1, m2, uint, L);
    let p = x.ast.builder().paren(c, L, L);
    let one = x.lit(1);
    let one = x.ic(CastKind::IntegralCast, one, uint);
    let s4 = x.ast.builder().binary(BinaryOp::Assign, p, one, uint, L);
    let s4 = x.expr_stmt(s4);
    // (f.a, f.b) = 3;
    let (m1, m2) = (mem(&mut x, fa), mem(&mut x, fb));
    let comma = x.ast.builder().binary(BinaryOp::Comma, m1, m2, INT, L);
    let p = x.ast.builder().paren(comma, L, L);
    let three = x.lit(3);
    let s5 = x.ast.builder().binary(BinaryOp::Assign, p, three, INT, L);
    let s5 = x.expr_stmt(s5);
    // return f.a + f.b + f.c;
    let ma = mem(&mut x, fa);
    let ma = x.rv(ma);
    let ma = x.ic(CastKind::IntegralCast, ma, INT);
    let mb = mem(&mut x, fb);
    let mb = x.rv(mb);
    let ab = x.bin(BinaryOp::Add, ma, mb, INT);
    let mc = mem(&mut x, fc);
    let mc = x.rv(mc);
    let mc = x.ic(CastKind::IntegralCast, mc, INT);
    let sum = x.bin(BinaryOp::Add, ab, mc, INT);
    let s6 = x.ret(Some(sum));
    x.body(touch, &[s1, s2, s3, s4, s5, s6]);
    // AstBuilder only marks the MemberExpr itself as a bit-field; the
    // operators sema builds over it (`=`, `++`, `+=`, `?:`, `,`) are
    // pushed as ordinary objects, so they lose clang's `bitfield` tag.
    // Everything else reproduces byte for byte.
    let want: String = golden!("class.bit/bitfield-access")
        .lines()
        .map(|l| if l.contains("MemberExpr") { l.to_owned() } else { l.replace(" bitfield", "") })
        .fold(String::new(), |acc, l| acc + &l + "\n");
    check(&x.dump(), &want);
}

/// `namespace ns { int x; } namespace ns { } inline namespace v1 { }
/// struct B { }; struct B2 { }; struct D : B, virtual protected B2
/// { mutable int m; }; typedef int T; using U = T *; [[nodiscard]] int g();`
fn decl_kinds() -> Fx {
    let mut x = Fx::new();
    let tu = x.tu();
    let h = x.head(tu, "ns");
    let ns = x.ast.builder().namespace(h, false, None);
    x.var(ns, "x", INT, VarSpec::new(), NONE);
    let h = x.head(tu, "ns");
    x.ast.builder().namespace(h, false, Some(ns));
    let h = x.head(tu, "v1");
    x.ast.builder().namespace(h, true, None);
    let empty = |x: &mut Fx, name: &str| {
        let r = x.record(tu, name, TagKind::Struct, REFD);
        x.ast.builder().begin_record_definition(r).expect("defn");
        x.ast.builder().complete_record_definition(r, DefinitionBits::empty());
        x.ast.decls.get(r).ty().expect("record type")
    };
    let b = empty(&mut x, "B");
    let b2 = empty(&mut x, "B2");
    let d = x.record(tu, "D", TagKind::Struct, NONE);
    x.ast.builder().begin_record_definition(d).expect("defn");
    let bases = [
        BaseSpec { ty: b, loc: L, is_virtual: false, access: Access::None, pack_expansion: false },
        BaseSpec {
            ty: b2,
            loc: L,
            is_virtual: true,
            access: Access::Protected,
            pack_expansion: false,
        },
    ];
    x.ast.decls.set_bases(d, &bases, &mut x.ast.lists).expect("bases");
    let h = x.head(d, "m");
    x.ast.builder().field(h, INT, None, FieldData::MUTABLE);
    x.ast.builder().complete_record_definition(d, DefinitionBits::empty());
    let t = x.typedef(tu, "T", DeclKind::Typedef, INT, REFD);
    let tty = x.ast.decls.get(t).ty().expect("typedef type");
    let tp = x.ptr(tty);
    x.typedef(tu, "U", DeclKind::TypeAlias, tp, NONE);
    let (g, _) = x.func(tu, "g", INT, &[], FnSpec::new(), NONE);
    let a = x.ast.attrs.push(AttrNode {
        kind: AttrKind::NODISCARD,
        syntax: AttrSyntax::Cxx11,
        flags: 0,
        loc: L,
        a: 0,
        b: 0,
    });
    x.ast.builder().attach_attrs(g, &[a]).expect("attrs");
    x
}

/// `main.cpp` holding `int a;`, a `#define X int b;` expanded on line 3,
/// and a declaration from `h.h`.
fn located() -> (Fx, SourceManager, kiln_source::FileId) {
    let sm = SourceManager::new();
    let main = sm.add_virtual("main.cpp", "int a;\n#define X int b;\nX\n".into()).expect("file");
    let hdr = sm.add_virtual("h.h", "int h;\n".into()).expect("file");
    let mut x = Fx::new();
    let tu = x.tu();
    let at = |name: &str, loc: Loc, x: &Fx| DeclHead::new(tu, x.name(name), loc);
    let h = at("h", sm.loc(hdr, 4), &x);
    x.ast.builder().var(h, INT, VarSpec::new(), None);
    let h = at("a", sm.loc(main, 4), &x);
    let a = x.ast.builder().var(h, INT, VarSpec::new(), None);
    let one = x.ast.builder().int_literal(INT, 1, Radix::Decimal, sm.loc(main, 4));
    x.init(a, one);
    let name = Span::new(sm.loc(main, 24), sm.loc(main, 25));
    let b = sm
        .add_expansion(sm.loc(main, 21), 1, name, ExpansionKind::Macro { name })
        .expect("expansion");
    let h = at("b", b, &x);
    x.ast.builder().var(h, INT, VarSpec::new(), None);
    (x, sm, main)
}

const DECL_KINDS: &str = "\
TranslationUnitDecl
|-NamespaceDecl ns
| `-VarDecl x 'int'
|-NamespaceDecl ns original Namespace 'ns'
|-NamespaceDecl v1 inline
|-CXXRecordDecl referenced struct B definition
|-CXXRecordDecl referenced struct B2 definition
|-CXXRecordDecl struct D definition
| |-public 'B'
| |-virtual protected 'B2'
| `-FieldDecl m 'int' mutable
|-TypedefDecl referenced T 'int'
|-TypeAliasDecl U 'T *'
`-FunctionDecl g 'int ()'
  `-WarnUnusedResultAttr
";

#[test]
fn per_kind_declarations() {
    check(&decl_kinds().dump(), DECL_KINDS);
}

#[test]
fn option_ids() {
    let x = decl_kinds();
    let got = x.dump_with(&DumpOptions { ids: true, ..DumpOptions::default() });
    check(
        &got,
        "\
TranslationUnitDecl #d1
|-NamespaceDecl #d2 ns
| `-VarDecl #d3 x 'int'
|-NamespaceDecl #d4 prev=#d2 ns original Namespace #d2 'ns'
|-NamespaceDecl #d5 v1 inline
|-CXXRecordDecl #d6 referenced struct B definition
|-CXXRecordDecl #d7 referenced struct B2 definition
|-CXXRecordDecl #d8 struct D definition
| |-public 'B'
| |-virtual protected 'B2'
| `-FieldDecl #d9 m 'int' mutable
|-TypedefDecl #d10 referenced T 'int'
|-TypeAliasDecl #d11 U 'T *'
`-FunctionDecl #d12 g 'int ()'
  `-WarnUnusedResultAttr #a1
",
    );
    // Expressions and statements carry `#eN` / `#sN`.
    let got = spec_example().dump_with(&DumpOptions { ids: true, ..DumpOptions::default() });
    assert!(got.lines().any(|l| l.contains("CompoundStmt #s")), "{got}");
    assert!(got.lines().any(|l| l.contains("Expr #e")), "{got}");
    // Without ids, nothing id-like appears.
    assert!(!spec_example().dump().contains(" #"));
}

#[test]
fn option_canonical_types() {
    let x = decl_kinds();
    let got = x.dump_with(&DumpOptions { canonical_types: true, ..DumpOptions::default() });
    let want = DECL_KINDS
        .replace("x 'int'", "x 'int':'int'")
        .replace("'B'\n", "'B':'B'\n")
        .replace("'B2'\n", "'B2':'B2'\n")
        .replace("m 'int'", "m 'int':'int'")
        .replace("T 'int'", "T 'int':'int'")
        .replace("U 'T *'", "U 'T *':'int *'")
        .replace("g 'int ()'", "g 'int ()':'int ()'");
    check(&got, &want);
}

#[test]
fn option_implicit() {
    let mut x = decl_kinds();
    let tu = x.tu();
    x.var(tu, "__hidden", INT, VarSpec::new(), DeclFlags::IMPLICIT);
    let g =
        x.ast.members(tu).find(|&d| x.ast.decls.get(d).kind() == DeclKind::Function).expect("g");
    let ty = x.ast.decls.get(g).ty().expect("type");
    let h = x.head(tu, "g");
    let g2 = x.ast.builder().function(h, DeclKind::Function, ty, FnSpec::new(), Some(g));
    let a = x.ast.attrs.push(AttrNode {
        kind: AttrKind::NODISCARD,
        syntax: AttrSyntax::Cxx11,
        flags: 3,
        loc: L,
        a: 0,
        b: 0,
    });
    x.ast.builder().attach_attrs(g2, &[a]).expect("attrs");
    let plain = x.dump();
    assert!(!plain.contains("__hidden") && !plain.contains("Inherited"), "{plain}");
    let all = x.dump_with(&DumpOptions { implicit: true, ..DumpOptions::default() });
    let tail = "\
|-FunctionDecl g 'int ()'
| `-WarnUnusedResultAttr
|-VarDecl implicit __hidden 'int'
`-FunctionDecl g 'int ()'
  `-WarnUnusedResultAttr Inherited Implicit
";
    assert!(all.ends_with(tail), "{all}");
}

#[test]
fn option_locs_and_files() {
    let (x, sm, main) = located();
    let opts = DumpOptions {
        locs: true,
        sources: Some(&sm),
        main_file: Some(main),
        ..DumpOptions::default()
    };
    // Main-file locations print as `L:C`; the macro expansion as
    // `@expansion[spelling]`; the `h.h` declaration is filtered out.
    check(
        &x.dump_with(&opts),
        "\
TranslationUnitDecl
|-VarDecl @1:5 a 'int' cinit
| `-IntegerLiteral @1:5 'int' 1
`-VarDecl @3:1[2:15] b 'int'
",
    );
    let all = DumpOptions { all_files: true, ..opts.clone() };
    check(
        &x.dump_with(&all),
        "\
TranslationUnitDecl
|-VarDecl @h.h:1:5 h 'int'
|-VarDecl @1:5 a 'int' cinit
| `-IntegerLiteral @1:5 'int' 1
`-VarDecl @3:1[2:15] b 'int'
",
    );
    // Without a main file every location names its file.
    let nomain = DumpOptions { main_file: None, ..all.clone() };
    check(
        &x.dump_with(&nomain),
        "\
TranslationUnitDecl
|-VarDecl @h.h:1:5 h 'int'
|-VarDecl @main.cpp:1:5 a 'int' cinit
| `-IntegerLiteral @main.cpp:1:5 'int' 1
`-VarDecl @main.cpp:3:1[main.cpp:2:15] b 'int'
",
    );
    // `locs` off: no locations, and main-file filtering still applies.
    let off = DumpOptions { locs: false, ..opts };
    check(
        &x.dump_with(&off),
        "\
TranslationUnitDecl
|-VarDecl a 'int' cinit
| `-IntegerLiteral 'int' 1
`-VarDecl b 'int'
",
    );
    // Without a SourceManager nothing is filtered and no location prints.
    let bare = DumpOptions { locs: true, ..DumpOptions::default() };
    assert!(x.dump_with(&bare).starts_with("TranslationUnitDecl\n|-VarDecl h 'int'\n"));
}

#[test]
fn filter_by_name_and_kind() {
    let x = decl_kinds();
    let by_name = |s: &str| {
        let f = Filter { name: Some(s.into()), kind: None };
        x.dump_with(&DumpOptions { filter: Some(f), ..DumpOptions::default() })
    };
    check(&by_name("ns::x"), "Dumping ns::x:\nVarDecl x 'int'\n\n");
    // A substring match; the search does not descend into a match.
    check(
        &by_name("ns"),
        "\
Dumping ns:
NamespaceDecl ns
`-VarDecl x 'int'

Dumping ns:
NamespaceDecl ns original Namespace 'ns'

",
    );
    check(&by_name("nomatch"), "");
    let f = Filter { name: None, kind: Some(DeclKind::Field) };
    let got = x.dump_with(&DumpOptions { filter: Some(f), ..DumpOptions::default() });
    check(&got, "Dumping D::m:\nFieldDecl m 'int' mutable\n\n");
    let f = Filter { name: Some("B".into()), kind: Some(DeclKind::CXXRecord) };
    let got = x.dump_with(&DumpOptions { filter: Some(f), ..DumpOptions::default() });
    check(
        &got,
        "\
Dumping B:
CXXRecordDecl referenced struct B definition

Dumping B2:
CXXRecordDecl referenced struct B2 definition

",
    );
}

/// Every fixture, dumped under every option combination.
fn all_dumps() -> Vec<String> {
    let (located, sm, main) = located();
    let fixtures = [spec_example(), decl_kinds(), located];
    let mut out = Vec::new();
    for bits in 0..32u32 {
        let opts = DumpOptions {
            ids: bits & 1 != 0,
            locs: bits & 2 != 0,
            canonical_types: bits & 4 != 0,
            implicit: bits & 8 != 0,
            all_files: bits & 16 != 0,
            filter: None,
            sources: Some(&sm),
            main_file: Some(main),
        };
        for x in &fixtures {
            out.push(x.dump_with(&opts));
        }
    }
    out
}

#[test]
fn no_trailing_whitespace() {
    for d in all_dumps() {
        assert!(d.ends_with('\n'), "{d}");
        for l in d.lines() {
            assert!(!l.ends_with(' ') && !l.ends_with('\t') && !l.is_empty(), "{l:?} in\n{d}");
        }
    }
}

#[test]
fn stable_and_writer_agnostic() {
    assert_eq!(all_dumps(), all_dumps());
    let x = spec_example();
    let opts = DumpOptions { ids: true, ..DumpOptions::default() };
    let mut buf = Vec::new();
    dump(&x.ast, &opts, &mut buf).expect("write to a Vec");
    assert_eq!(String::from_utf8(buf).expect("utf-8"), dump_to_string(&x.ast, &opts));
    assert_eq!(x.dump_with(&opts), x.dump_with(&opts));
}
