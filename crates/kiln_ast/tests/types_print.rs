//! Type printer goldens (spec/types.md §12). Every expected string was
//! produced by clang 19 (`-fsyntax-only -Xclang -ast-dump` on the
//! equivalent declaration; the pairs are its `'sugared':'desugared'`
//! strings).
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "short-lived type handles"
)]

use std::collections::HashMap;

use kiln_ast::ids::{ArgListId, ConstValueId, DeclId, ExprId, QualType, TypeId};
use kiln_ast::types::{
    AutoKeyword, ExcSpec, FuncInfo, Nns, PrintOptions, Quals, RawNames, RefQual, TagKeyword,
    TemplateArg, TemplateName, TypeContext, TypeFlags, TypeNames, VectorKind, aka, dump_strings,
    print_type,
};
use kiln_source::symbol::Symbol;

/// A stand-in for `Ast`: declarations with a name and a scope prefix.
#[derive(Default)]
struct Names {
    decls: Vec<(String, String)>,
    exprs: Vec<String>,
    syms: Vec<String>,
    spec_args: HashMap<DeclId, ArgListId>,
}

impl Names {
    fn decl(&mut self, name: &str, scope: &str) -> DeclId {
        self.decls.push((name.to_owned(), scope.to_owned()));
        DeclId::new(self.decls.len() as u32)
    }
    fn expr(&mut self, text: &str) -> ExprId {
        self.exprs.push(text.to_owned());
        ExprId::new(self.exprs.len() as u32)
    }
    fn sym(&mut self, text: &str) -> Symbol {
        self.syms.push(text.to_owned());
        Symbol::from_index(self.syms.len() as u32)
    }
}

impl TypeNames for Names {
    fn write_decl_name(&self, decl: DeclId, scoped: bool, out: &mut String) {
        let (name, scope) = &self.decls[decl.index() as usize - 1];
        if scoped {
            out.push_str(scope);
        }
        out.push_str(name);
    }
    fn specialization_args(&self, decl: DeclId) -> Option<ArgListId> {
        self.spec_args.get(&decl).copied()
    }
    fn write_expr(&self, expr: ExprId, out: &mut String) {
        out.push_str(&self.exprs[expr.index() as usize - 1]);
    }
    fn write_value(&self, _ty: QualType, value: ConstValueId, out: &mut String) {
        out.push_str(&value.index().to_string());
    }
    fn write_symbol(&self, sym: Symbol, out: &mut String) {
        out.push_str(&self.syms[sym.index() as usize - 1]);
    }
}

struct Fx {
    ctx: TypeContext,
    n: Names,
    s: TypeId,
}

impl Fx {
    fn new() -> Fx {
        let mut n = Names::default();
        let mut ctx = TypeContext::default();
        let sd = n.decl("S", "ns::");
        let s = ctx.record_type(sd);
        Fx { ctx, n, s }
    }
    fn p(&self, q: QualType) -> String {
        print_type(&self.ctx, &self.n, q, PrintOptions::SUGARED)
    }
    fn canon(&self, q: QualType) -> String {
        print_type(&self.ctx, &self.n, q, PrintOptions::CANONICAL)
    }
    /// clang's AST-dump pair: `'a'` or `'a':'b'`.
    fn dump(&self, q: QualType) -> String {
        match dump_strings(&self.ctx, &self.n, q) {
            (a, None) => format!("'{a}'"),
            (a, Some(b)) => format!("'{a}':'{b}'"),
        }
    }
    fn q(&mut self, q: QualType, cv: Quals) -> QualType {
        self.ctx.qualify(q, cv)
    }
    fn ptr(&mut self, q: QualType) -> QualType {
        self.ctx.pointer_to(q)
    }
    fn arr(&mut self, q: QualType, n: u64) -> QualType {
        self.ctx.constant_array(q, n, None)
    }
    fn func(&mut self, ret: QualType, params: &[QualType]) -> QualType {
        self.ctx.function(ret, params, FuncInfo::new(), ExcSpec::None)
    }
}

const INT: QualType = QualType::INT;

#[test]
fn qualifiers() {
    let mut f = Fx::new();
    let ci = f.q(INT, Quals::CONST);
    let t = f.ptr(ci);
    assert_eq!(f.p(t), "const int *");
    let p = f.ptr(INT);
    let t = f.q(p, Quals::CONST);
    assert_eq!(f.p(t), "int *const");
    let pp = f.ptr(t);
    let t = f.q(pp, Quals::VOLATILE);
    assert_eq!(f.p(t), "int *const *volatile");
    let t = f.q(p, Quals::RESTRICT);
    assert_eq!(f.p(t), "int *__restrict");
    let cvi = f.q(INT, Quals::CV);
    let t = f.ptr(cvi);
    assert_eq!(f.p(t), "const volatile int *");
    assert_eq!(f.p(ci), "const int");
}

#[test]
fn pointers_and_arrays() {
    let mut f = Fx::new();
    let a3 = f.arr(INT, 3);
    let t = f.ptr(a3);
    assert_eq!(f.p(t), "int (*)[3]");
    let pc = f.ptr(QualType::CHAR);
    let t = f.arr(pc, 3);
    assert_eq!(f.p(t), "char *[3]");
    let t = f.ctx.lvalue_ref_to(a3);
    assert_eq!(f.p(t), "int (&)[3]");
    let t = f.arr(a3, 2);
    assert_eq!(f.p(t), "int[2][3]");
    let ca = f.q(a3, Quals::CONST);
    let t = f.ptr(ca);
    assert_eq!(f.p(t), "const int (*)[3]");
    let t = f.ctx.incomplete_array(INT);
    assert_eq!(f.p(t), "int[]");
}

#[test]
fn functions() {
    let mut f = Fx::new();
    let fi = f.func(INT, &[INT]);
    let pfi = f.ptr(fi);
    assert_eq!(f.p(pfi), "int (*)(int)");
    let fc = f.func(QualType::VOID, &[QualType::CHAR]);
    let pfc = f.ptr(fc);
    let g = f.func(pfc, &[INT]);
    assert_eq!(f.p(g), "void (*(int))(char)");
    let t = f.ptr(g);
    assert_eq!(f.p(t), "void (*(*)(int))(char)");
    let t = f.arr(pfi, 2);
    assert_eq!(f.p(t), "int (*[2])(int)");
    let t = f.func(QualType::VOID, &[pfi]);
    assert_eq!(f.p(t), "void (int (*)(int))");
    let info = FuncInfo::new();
    let t = f.ctx.function(QualType::VOID, &[], info, ExcSpec::BasicNoexcept);
    assert_eq!(f.p(t), "void () noexcept");
    let t = f.ctx.function(QualType::VOID, &[], info, ExcSpec::DynamicNone);
    assert_eq!(f.p(t), "void () throw()");
    let cc = f.q(QualType::CHAR, Quals::CONST);
    let pcc = f.ptr(cc);
    let t = f.ctx.function(INT, &[pcc], info.with_variadic(true), ExcSpec::None);
    assert_eq!(f.p(t), "int (const char *, ...)");
    let t = f.ctx.function(INT, &[], info.with_variadic(true), ExcSpec::None);
    assert_eq!(f.p(t), "int (...)");
    let t = f.ctx.function(INT, &[], info.with_trailing_return(true), ExcSpec::None);
    assert_eq!(f.p(t), "auto () -> int");
    let ex = f.n.expr("sizeof(int) > 2");
    let t = f.ctx.function(
        QualType::VOID,
        &[],
        info,
        ExcSpec::NoexceptExpr { expr: ex, value: kiln_ast::types::NoexceptValue::True },
    );
    assert_eq!(f.p(t), "void () noexcept(sizeof(int) > 2)");
    // clang keeps the written exception specification in the `aka`
    // desugaring: `void (*)() noexcept(sizeof(int) > 2)` gets none.
    let pt = f.ptr(t);
    assert_eq!(aka(&f.ctx, &f.n, pt), None);
    let list = f.ctx.type_list(&[INT, QualType::CHAR]);
    let t = f.ctx.function(QualType::VOID, &[], info, ExcSpec::Dynamic(list));
    assert_eq!(f.p(t), "void () throw(int, char)");
}

#[test]
fn member_pointers_and_references() {
    let mut f = Fx::new();
    let s = f.s;
    let info = FuncInfo::new().with_this_quals(Quals::CONST).with_ref_qual(RefQual::LValue);
    let m = f.ctx.function(QualType::VOID, &[INT], info, ExcSpec::None);
    let t = f.ctx.member_pointer(m, s);
    assert_eq!(f.p(t), "void (ns::S::*)(int) const &");
    let t = f.ctx.member_pointer(INT, s);
    assert_eq!(f.p(t), "int ns::S::*");
    let info = FuncInfo::new().with_ref_qual(RefQual::RValue);
    let m = f.ctx.function(QualType::VOID, &[], info, ExcSpec::None);
    let t = f.ctx.member_pointer(m, s);
    assert_eq!(f.p(t), "void (ns::S::*)() &&");
    let t = f.ctx.rvalue_ref_to(INT);
    assert_eq!(f.p(t), "int &&");
    let v = f.func(QualType::VOID, &[]);
    let t = f.ctx.lvalue_ref_to(v);
    assert_eq!(f.p(t), "void (&)()");
}

#[test]
fn sugar_pairs() {
    let mut f = Fx::new();
    let s = QualType::unqual(f.s);
    // typedef int Arr[3];
    let a3 = f.arr(INT, 3);
    let arr_d = f.n.decl("Arr", "");
    let arr = f.ctx.typedef_type(arr_d, a3).into();
    let t = f.q(arr, Quals::CONST);
    assert_eq!(f.dump(t), "'const Arr':'const int[3]'");
    let t = f.q(arr, Quals::VOLATILE);
    assert_eq!(f.dump(t), "'volatile Arr':'volatile int[3]'");
    let ca = f.q(arr, Quals::CONST);
    let t = f.ptr(ca);
    assert_eq!(f.p(t), "const Arr *");
    // A written `S` (kiln: bare record) and `struct S`.
    assert_eq!(f.dump(s), "'S':'ns::S'");
    let t = f.ctx.elaborated(TagKeyword::Struct, None, s);
    assert_eq!(f.dump(t), "'struct S':'ns::S'");
    let mut g = PrintOptions::SUGARED;
    g.suppress_tag = true;
    assert_eq!(print_type(&f.ctx, &f.n, t, g), "S");
    // enum ns::En
    let ns = f.n.decl("ns", "");
    let en_d = f.n.decl("En", "ns::");
    let en = f.ctx.enum_type(en_d).into();
    let nns = f.ctx.nns(Nns::Namespace { prefix: None, ns });
    let t = f.ctx.elaborated(TagKeyword::Enum, Some(nns), en);
    assert_eq!(f.dump(t), "'enum ns::En':'ns::En'");
    // using PS = ns::S*; const PS
    let nss = f.ctx.elaborated(TagKeyword::None, Some(nns), s);
    let ps = f.ptr(nss);
    let ps_d = f.n.decl("PS", "");
    let pst = f.ctx.typedef_type(ps_d, ps).into();
    let t = f.q(pst, Quals::CONST);
    assert_eq!(f.dump(t), "'const PS':'ns::S *const'");
    assert_eq!(aka(&f.ctx, &f.n, t).as_deref(), Some("ns::S *const"));
    assert_eq!(aka(&f.ctx, &f.n, INT), None);
    // typedef void F(); F *
    let fv = f.func(QualType::VOID, &[]);
    let fd = f.n.decl("F", "");
    let ft = f.ctx.typedef_type(fd, fv).into();
    let t = f.ptr(ft);
    assert_eq!(f.p(t), "F *");
    assert_eq!(f.canon(t), "void (*)()");
    // using-declared type
    let ud = f.n.decl("U", "");
    let t: QualType = f.ctx.using_type(ud, s).into();
    assert_eq!(f.dump(t), "'U':'ns::S'");
}

#[test]
fn builtin_like() {
    let mut f = Fx::new();
    let t = f.ctx.bit_int(false, 13);
    assert_eq!(f.p(t), "unsigned _BitInt(13)");
    let t = f.ctx.bit_int(true, 7);
    assert_eq!(f.p(t), "_BitInt(7)");
    let e = f.n.expr("nullptr");
    let t = f.ctx.decltype_type(e, QualType::NULLPTR, None);
    assert_eq!(f.dump(t), "'decltype(nullptr)':'std::nullptr_t'");
    let t = f.ctx.atomic(INT);
    assert_eq!(f.p(t), "_Atomic(int)");
    let t = f.ctx.complex(QualType::FLOAT);
    assert_eq!(f.p(t), "_Complex float");
    let t = f.ctx.vector(INT, 4, VectorKind::Generic);
    assert_eq!(f.p(t), "__attribute__((__vector_size__(4 * sizeof(int)))) int");
    let t = f.ctx.vector(QualType::FLOAT, 4, VectorKind::Ext);
    assert_eq!(f.p(t), "float __attribute__((ext_vector_type(4)))");
    let t = f.ctx.auto_type(AutoKeyword::Auto, None, None, None, false);
    assert_eq!(f.p(t), "auto");
    let t = f.ctx.auto_type(AutoKeyword::Auto, Some(INT), None, None, false);
    assert_eq!(f.p(t), "int");
    assert_eq!(f.p(QualType::ERROR), "<error type>");
}

/// `template <class T, class... Ts> struct W;` and friends.
struct Tpl {
    f: Fx,
    t: QualType,
    ts: QualType,
    w: kiln_ast::ids::TemplateNameId,
    ns_nns: kiln_ast::ids::NnsId,
}

fn tpl() -> Tpl {
    let mut f = Fx::new();
    let td = f.n.decl("T", "");
    let tsd = f.n.decl("Ts", "");
    let t = f.ctx.template_type_parm(td, 0, 0, false).into();
    let ts = f.ctx.template_type_parm(tsd, 0, 1, true).into();
    let wd = f.n.decl("W", "ns::");
    let w = f.ctx.template_name_of(wd);
    let ns = f.n.decl("ns", "");
    let ns_nns = f.ctx.nns(Nns::Namespace { prefix: None, ns });
    Tpl { f, t, ts, w, ns_nns }
}

#[test]
fn templates() {
    let Tpl { mut f, t, ts, w, ns_nns } = tpl();
    // ns::W<T>
    let args = f.ctx.arg_list(&[TemplateArg::ty(t)]);
    let wt = f.ctx.template_specialization(w, args, None, false);
    let e = f.ctx.elaborated(TagKeyword::None, Some(ns_nns), wt);
    assert_eq!(f.dump(e), "'ns::W<T>':'W<T>'");
    assert_eq!(f.canon(e), "ns::W<type-parameter-0-0>");
    // ns::W<Ts...> *
    let exp = f.ctx.pack_expansion(ts, None);
    let args = f.ctx.arg_list(&[TemplateArg::ty(exp)]);
    let wts = f.ctx.template_specialization(w, args, None, false);
    let e = f.ctx.elaborated(TagKeyword::None, Some(ns_nns), wts);
    let p = f.ptr(e);
    assert_eq!(f.p(p), "ns::W<Ts...> *");
    assert!(!f.ctx.flags(exp.ty()).contains(TypeFlags::UNEXPANDED_PACK));
    // X<T, Ts...> *
    let xd = f.n.decl("X", "");
    let x = f.ctx.template_name_of(xd);
    let args = f.ctx.arg_list(&[TemplateArg::ty(t), TemplateArg::ty(exp)]);
    let xt = f.ctx.template_specialization(x, args, None, false);
    let p = f.ptr(xt);
    assert_eq!(f.p(p), "X<T, Ts...> *");
    // ns::W<int *> as a class template specialization, named via alias
    // template `template <class U> using Al = ns::W<U *>;` as `Al<int>`.
    let pi = f.ptr(INT);
    let w_pi_d = f.n.decl("W", "ns::");
    let w_pi_args = f.ctx.arg_list(&[TemplateArg::ty(pi)]);
    f.n.spec_args.insert(w_pi_d, w_pi_args);
    let w_pi: QualType = f.ctx.record_type(w_pi_d).into();
    let ald = f.n.decl("Al", "");
    let al = f.ctx.template_name_of(ald);
    let args = f.ctx.arg_list(&[TemplateArg::ty(INT)]);
    let alt = f.ctx.template_specialization(al, args, Some(w_pi), true);
    assert_eq!(f.dump(alt), "'Al<int>':'ns::W<int *>'");
    // ns::W<ns::W<int>> (canonical) and W<W<int>> && (no `> >`)
    let w_i_d = f.n.decl("W", "ns::");
    let a = f.ctx.arg_list(&[TemplateArg::ty(INT)]);
    f.n.spec_args.insert(w_i_d, a);
    let w_i: QualType = f.ctx.record_type(w_i_d).into();
    let w_wi_d = f.n.decl("W", "ns::");
    let a = f.ctx.arg_list(&[TemplateArg::ty(w_i)]);
    f.n.spec_args.insert(w_wi_d, a);
    let w_wi: QualType = f.ctx.record_type(w_wi_d).into();
    assert_eq!(f.canon(w_wi), "ns::W<ns::W<int>>");
    let a1 = f.ctx.arg_list(&[TemplateArg::ty(INT)]);
    let inner = f.ctx.template_specialization(w, a1, Some(w_i), false);
    let a2 = f.ctx.arg_list(&[TemplateArg::ty(inner)]);
    let outer = f.ctx.template_specialization(w, a2, Some(w_wi), false);
    let r = f.ctx.rvalue_ref_to(outer);
    assert_eq!(f.p(r), "W<W<int>> &&");
    // An empty pack argument prints nothing: X<int>.
    let a = f.ctx.arg_list(&[TemplateArg::ty(INT), TemplateArg::pack(ArgListId::EMPTY)]);
    let t2 = f.ctx.template_specialization(x, a, None, false);
    assert_eq!(f.p(t2), "X<int>");
    // Non-type arguments.
    let e = f.n.expr("N + 1");
    let a = f
        .ctx
        .arg_list(&[TemplateArg::expr(e, None), TemplateArg::value(INT, ConstValueId::new(3))]);
    let t3 = f.ctx.template_specialization(x, a, None, false);
    assert_eq!(f.p(t3), "X<N + 1, 3>");
}

#[test]
fn dependent() {
    let Tpl { mut f, t, ts, .. } = tpl();
    // typename T::type
    let tn = f.ctx.nns(Nns::Type { prefix: None, ty: t, template_kw: false });
    let sym = f.n.sym("type");
    let d = f.ctx.dependent_name(TagKeyword::Typename, tn, sym);
    assert_eq!(f.p(d), "typename T::type");
    // T[sizeof(T)]
    let e = f.n.expr("sizeof(T)");
    let p = f.ctx.profile(&[1, 2, 3], e, TypeFlags::DEPENDENT_INSTANTIATION);
    let a = f.ctx.dependent_sized_array(t, e, p);
    assert_eq!(f.p(a), "T[sizeof(T)]");
    // decltype(T())
    let e = f.n.expr("T()");
    let p = f.ctx.profile(&[4, 5], e, TypeFlags::DEPENDENT_INSTANTIATION);
    let d = f.ctx.decltype_type(e, QualType::ERROR, Some(p));
    assert_eq!(f.p(d), "decltype(T())");
    // void (Ts...)
    let exp = f.ctx.pack_expansion(ts, None);
    let fv = f.func(QualType::VOID, &[exp]);
    assert_eq!(f.p(fv), "void (Ts...)");
    // typedef T TT;
    let ttd = f.n.decl("TT", "");
    let tt: QualType = f.ctx.typedef_type(ttd, t).into();
    assert_eq!(f.dump(tt), "'TT':'T'");
    let ct = f.q(t, Quals::CONST);
    assert_eq!(f.p(ct), "const T");
    let pt = f.ptr(t);
    assert_eq!(f.canon(pt), "type-parameter-0-0 *");
    // typename T::template X<int>
    let sym = f.n.sym("X");
    let args = f.ctx.arg_list(&[TemplateArg::ty(INT)]);
    let d = f.ctx.dependent_template_specialization(TagKeyword::Typename, tn, sym, args);
    assert_eq!(f.p(d), "typename T::template X<int>");
    // A substituted T = const int, written `const T`, prints `const int`.
    let assoc = f.n.decl("tmpl", "");
    let ci = f.q(INT, Quals::CONST);
    let sub = f.ctx.subst_template_type_parm(ci, assoc, 0, None);
    let csub = f.q(sub, Quals::CONST);
    assert_eq!(f.p(csub), "const int");
    let psub = f.ptr(sub);
    assert_eq!(f.p(psub), "const int *");
}

#[test]
fn unnamed_and_raw() {
    let mut f = Fx::new();
    let l = f.n.decl("(lambda at t2.cpp:19:12)", "");
    let lt: QualType = f.ctx.record_type(l).into();
    assert_eq!(f.p(lt), "(lambda at t2.cpp:19:12)");
    let u = f.n.decl("(unnamed struct at t2.cpp:20:1)", "");
    let ut: QualType = f.ctx.record_type(u).into();
    let e = f.ctx.elaborated(TagKeyword::Struct, None, ut);
    assert_eq!(
        f.dump(e),
        "'struct (unnamed struct at t2.cpp:20:1)':'(unnamed struct at t2.cpp:20:1)'"
    );
    let p = f.ptr(lt);
    assert_eq!(print_type(&f.ctx, &RawNames, p, PrintOptions::SUGARED), "decl#2 *");
}

#[test]
fn deep_nesting_is_bounded() {
    let mut f = Fx::new();
    let mut q = INT;
    for _ in 0..5000 {
        q = f.ptr(q);
        q = f.arr(q, 1);
    }
    let s = f.p(q);
    assert!(s.contains("..."));
    // Balanced output: as many `(` as `)`.
    assert_eq!(s.matches('(').count(), s.matches(')').count());
}

#[test]
fn template_names() {
    let Tpl { mut f, t, w, ns_nns, .. } = tpl();
    let q = f.ctx.template_name(TemplateName::Qualified {
        nns: ns_nns,
        template_kw: false,
        underlying: w,
    });
    let args = f.ctx.arg_list(&[TemplateArg::template(q)]);
    let xd = f.n.decl("X", "");
    let x = f.ctx.template_name_of(xd);
    let xt = f.ctx.template_specialization(x, args, None, false);
    assert_eq!(f.p(xt), "X<ns::W>");
    // CTAD placeholder `ns::W` and its deduced form.
    let d = f.ctx.deduced_template_specialization(q, None);
    assert_eq!(f.p(d), "ns::W");
    let _ = t;
}
