//! The KBC compiler: lowers a function body or a top-level expression to
//! KBC once, lazily (on the first call), and keeps it for the translation
//! unit.
//!
//! Locals whose address is never taken live in registers; the rest (and
//! every aggregate) live in abstract memory, allocated at the entry of
//! their block. Every construct the evaluator cannot evaluate compiles to
//! a `Trap` carrying its note, so compilation itself never fails: a
//! function is rejected only when control actually reaches the construct
//! ([expr.const]/10 is about evaluation, not about the function).

use kiln_ast::consts::{FloatKind, Scalar};
use kiln_ast::decl::{ConstexprKind, DeclKind, VarStorage};
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, TraitKind, UnaryOp};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType, StmtId, StringLitId};
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::failure::Note;
use crate::kbc::{Op, ak, cmp, int_aux};
use crate::mem::{Origin, fn_ptr};
use crate::ty::{OTy, Scal, TyKind, eval_int};
use crate::{Oven, SemaBridge};

mod bitcast;
mod builtins;
mod class;
mod mptr;
mod new_delete;

pub(crate) use builtins::designator_type;
pub(crate) use new_delete::is_std_named;

/// `Func::entry` of a function not compiled yet.
pub(crate) const UNCOMPILED: u32 = u32::MAX;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FState {
    /// Not compiled yet.
    Stub,
    Ready,
    /// Not `constexpr` ([expr.const]/10.2).
    NotConstexpr,
    /// Declared but not defined (yet): retried on every call.
    Undefined,
    Deleted,
    /// Its body is being made available (`SemaBridge::ensure_body` is
    /// running, and may evaluate nested constant expressions): a call to it
    /// meanwhile fails as a call to an undefined function.
    Compiling,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum RetKind {
    Void,
    Scalar(OTy),
    /// Aggregate: the caller passes the result object's address in
    /// register 0.
    Sret(OTy),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParamInfo {
    pub decl: DeclId,
    pub ty: OTy,
    /// The register holding the value (or, for `mem`, the address).
    pub reg: u32,
    pub mem: bool,
    /// A reference parameter: `reg` holds the referent's address.
    pub is_ref: bool,
}

#[derive(Debug)]
pub(crate) struct Func {
    /// `None` for a top-level thunk.
    pub decl: Option<DeclId>,
    pub entry: u32,
    pub state: FState,
    pub nregs: u32,
    pub ret: RetKind,
    pub params: Vec<ParamInfo>,
    /// Eligible for memoization (pure: scalar, non-pointer parameters).
    pub memo: bool,
    /// The register of the implicit object parameter of a non-static
    /// member function (after the result slot, before the parameters).
    pub this_reg: Option<u32>,
    /// Its class (the type `this` points to; for the call-stack notes).
    pub this_ty: Option<OTy>,
}

/// The source of a faulting instruction.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Site {
    pub pc: u32,
    pub loc: Loc,
    pub e: Option<ExprId>,
}

/// A compiled-in failure.
#[derive(Clone, Debug)]
pub(crate) struct TrapInfo {
    pub loc: Loc,
    pub msg: String,
    pub extra: Vec<Note>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum GKind {
    Var(DeclId),
    /// A string literal object; the `StringLiteral` expr that created it
    /// (`None`: thawed from a frozen pointer).
    Str(StringLitId, OTy, Option<ExprId>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum GState {
    Unresolved,
    /// Its storage: a static allocation holding the thawed value, the
    /// placeholder of a variable being initialized (flag `RESOLVING`), or
    /// an external object (reads and writes are diagnosed).
    Resolved(u64),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Global {
    pub kind: GKind,
    pub state: GState,
    /// A reusable placeholder allocation (0 = none): the storage of a
    /// variable whose own initializer is evaluated at top level.
    pub slot: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Local {
    Reg(u32),
    /// The register holds the object's address.
    Mem(u32),
}

/// A compiled lvalue.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Place {
    Reg(u32),
    Mem(u32),
    Bits { p: u32, bit: u32, width: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Access {
    Read,
    Write,
    Rmw,
    Addr,
}

struct Breakable {
    breaks: Vec<usize>,
    conts: Option<Vec<usize>>,
    /// `scopes.len()` outside the statement.
    depth: usize,
    /// `scopes.len()` outside the loop body.
    cont_depth: usize,
}

pub(crate) fn float_code(k: FloatKind) -> u32 {
    match k {
        FloatKind::Half => 0,
        FloatKind::BFloat16 => 1,
        FloatKind::Single => 2,
        FloatKind::Double => 3,
        FloatKind::X87Extended => 4,
        FloatKind::Quad => 5,
    }
}

pub(crate) fn float_of(c: u32) -> FloatKind {
    match c & 15 {
        0 => FloatKind::Half,
        1 => FloatKind::BFloat16,
        2 => FloatKind::Single,
        3 => FloatKind::Double,
        4 => FloatKind::X87Extended,
        _ => FloatKind::Quad,
    }
}

/// Strips parentheses.
/// Whether `e` is a builtin binary operator `op`.
fn is_binary(ast: &kiln_ast::Ast, e: ExprId, op: BinaryOp) -> bool {
    ast.exprs.get(e).kind() == ExprKind::BinaryOperator && ast.exprs.binary_op(e) == op
}

/// Whether `e` is a builtin `&&` or `||`.
fn is_logical(ast: &kiln_ast::Ast, e: ExprId) -> bool {
    ast.exprs.get(e).kind() == ExprKind::BinaryOperator
        && matches!(ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr)
}

/// Whether `e` is a builtin binary operator `binary_node` evaluates (a
/// prvalue of arithmetic, comparison or pointer arithmetic).
fn is_plain_binary(ast: &kiln_ast::Ast, e: ExprId) -> bool {
    let n = ast.exprs.get(e);
    if n.kind() != ExprKind::BinaryOperator || n.value_category().is_glvalue() {
        return false;
    }
    let op = ast.exprs.binary_op(e);
    !matches!(op, BinaryOp::Comma | BinaryOp::LAnd | BinaryOp::LOr | BinaryOp::Assign)
        && !op.is_compound_assignment()
        && !op.is_ptr_mem()
}

pub(crate) fn skip_parens(ast: &kiln_ast::Ast, mut e: ExprId) -> ExprId {
    loop {
        match ast.exprs.get(e).kind() {
            ExprKind::ParenExpr => e = ast.exprs.operand(e),
            ExprKind::SubstNonTypeTemplateParmExpr => e = ast.exprs.subst_replacement(e),
            // [over.match.oper]/8: the rewritten form is its semantic
            // expression.
            ExprKind::CXXRewrittenBinaryOperator => e = ast.exprs.rewritten_semantic(e),
            _ => break,
        }
    }
    e
}

/// Whether `d` is a variable with automatic storage duration.
pub(crate) fn is_local_var(ast: &kiln_ast::Ast, d: DeclId) -> bool {
    let n = ast.decls.get(d);
    match n.kind() {
        DeclKind::ParmVar => true,
        DeclKind::Var => {
            let Some(v) = ast.decls.var(d) else { return false };
            v.spec.storage() == VarStorage::None
                && n.ctx().is_some_and(|c| ast.decls.get(c).kind().is_function())
        }
        _ => false,
    }
}

impl Oven {
    /// The function index of `d` (a stub until first called).
    pub(crate) fn func_index(&mut self, ast: &kiln_ast::Ast, d: DeclId) -> u32 {
        let d = ast.first(d);
        if let Some(&i) = self.func_map.get(&d) {
            return i;
        }
        let i = self.funcs.len() as u32;
        self.funcs.push(Func {
            decl: Some(d),
            entry: UNCOMPILED,
            state: FState::Stub,
            nregs: 0,
            ret: RetKind::Void,
            params: Vec::new(),
            memo: false,
            this_reg: None,
            this_ty: None,
        });
        self.func_map.insert(d, i);
        i
    }

    /// The global index of variable `d`.
    pub(crate) fn global_index(&mut self, ast: &kiln_ast::Ast, d: DeclId) -> u32 {
        let d = ast.first(d);
        if let Some(&i) = self.global_map.get(&d) {
            return i;
        }
        let i = self.globals.len() as u32;
        self.globals.push(Global { kind: GKind::Var(d), state: GState::Unresolved, slot: 0 });
        self.global_map.insert(d, i);
        i
    }

    /// The string literal object of `StringLiteral` expr `e`: one object
    /// per expression, so `"a" == "a"` compares distinct objects (the
    /// pool's ids are storage dedup only; spec/ast.md §9.1).
    pub(crate) fn string_expr_index(&mut self, e: ExprId, s: StringLitId, ty: OTy) -> u32 {
        if let Some(&i) = self.lit_exprs.get(&e) {
            return i;
        }
        let i = self.globals.len() as u32;
        self.globals.push(Global {
            kind: GKind::Str(s, ty, Some(e)),
            state: GState::Unresolved,
            slot: 0,
        });
        self.lit_exprs.insert(e, i);
        i
    }

    /// Compiles a top-level thunk evaluating `e`: a scalar result in
    /// register 0, or the address of the result object (whose origin is
    /// `object`, else a temporary) in register 0.
    pub(crate) fn compile_thunk(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        object: Option<DeclId>,
    ) -> u32 {
        if let Some(&f) = self.thunks.get(&e)
            && (object.is_none() || self.funcs[f as usize].decl.is_none())
        {
            return f;
        }
        let idx = self.funcs.len() as u32;
        self.funcs.push(Func {
            decl: None,
            entry: UNCOMPILED,
            state: FState::Stub,
            nregs: 0,
            ret: RetKind::Void,
            params: Vec::new(),
            memo: false,
            this_reg: None,
            this_ty: None,
        });
        let mut c = Compiler::new(self, b);
        let entry = c.here();
        let ty = c.ety(e);
        let glvalue = c.b.ast().exprs.get(e).value_category().is_glvalue();
        if let Some(d) = object {
            c.begin_static_extended(e, d);
        }

        let ref_object = object.filter(|&d| is_reference_var(c.b.ast(), d));
        let ret = if let Some(d) = ref_object {
            // A reference's initializer: the address of its referent.
            let q = c.b.ast().decls.get(d).ty().unwrap_or(QualType::INT);
            let rt = c.ty(q);
            let r = c.reg();
            c.bind_ref(e, r);
            c.emit(Op::Ret, 1, &[r]);
            RetKind::Scalar(rt)
        } else if c.o.types.is_aggregate(ty) {
            let origin = match object {
                Some(d) => Origin::Var(d),
                None => Origin::Temp(e),
            };
            let r = c.reg();
            c.alloc(r, ty, origin, false, e);
            if glvalue {
                let p = c.lv_mem(e, Access::Read);
                c.emit_at(Op::Copy, 1, &[r, p, ty.0], e);
            } else {
                c.init(e, r, ty);
            }
            c.emit(Op::Ret, 0, &[r]);
            RetKind::Sret(ty)
        } else if c.o.types.kind(ty) == TyKind::Void {
            c.effect(e);
            c.emit(Op::Ret, 0, &[0]);
            RetKind::Void
        } else if glvalue {
            // The value of a scalar glvalue (sema's speculative folds of
            // an operand before its lvalue-to-rvalue conversion).
            let r = c.reg();
            let p = c.lv(e, Access::Read);
            c.load(p, ty, ak::READ, e, r);
            c.emit(Op::Ret, 1, &[r]);
            RetKind::Scalar(ty)
        } else {
            let r = c.reg();
            c.rv_into(e, r);
            c.emit(Op::Ret, 1, &[r]);
            RetKind::Scalar(ty)
        };
        let nregs = c.max;
        let f = &mut self.funcs[idx as usize];
        f.entry = entry;
        f.state = FState::Ready;
        f.nregs = nregs;
        f.ret = ret;
        self.thunks.insert(e, idx);
        idx
    }

    /// A function destroying the object of type `ty` whose address is in
    /// its register 0 (the constant destruction of `constexpr` variable
    /// `d`, [dcl.constexpr]/6), or `None` if destroying it does nothing.
    pub(crate) fn compile_destroy(
        &mut self,
        b: &mut dyn SemaBridge,
        d: DeclId,
        ty: OTy,
    ) -> Option<u32> {
        let mut c = Compiler::new(self, b);
        let f = c.dtor_of(ty)?;
        let entry = c.here();
        let p = c.reg();
        debug_assert_eq!(p, 0);
        // clang's `in call to 'v.~T()'` points at the variable.
        let loc = c.b.ast().decls.get(d).loc();
        c.emit_loc(Op::PushCleanup, 0, &[p, f, ty.0], loc);
        c.emit(Op::Ret, 1, &[p]);
        let nregs = c.max;
        let idx = self.funcs.len() as u32;
        self.funcs.push(Func {
            decl: None,
            entry,
            state: FState::Ready,
            nregs,
            ret: RetKind::Void,
            params: Vec::new(),
            memo: false,
            this_reg: None,
            this_ty: None,
        });
        Some(idx)
    }

    /// Compiles function `idx` if it can be: sets its state.
    pub(crate) fn compile_func(&mut self, b: &mut dyn SemaBridge, idx: u32) {
        let Some(d) = self.funcs[idx as usize].decl else { return };
        let ast = b.ast();
        let Some(fd) = ast.decls.function(ast.latest(d)) else {
            self.funcs[idx as usize].state = FState::NotConstexpr;
            return;
        };
        let spec = fd.spec;
        if spec.is_deleted() {
            self.funcs[idx as usize].state = FState::Deleted;
            return;
        }
        if spec.constexpr_kind() == ConstexprKind::None {
            self.funcs[idx as usize].state = FState::NotConstexpr;
            return;
        }
        self.funcs[idx as usize].state = FState::Compiling;
        let Some(body) = b.ensure_body(d, self) else {
            self.funcs[idx as usize].state = FState::Undefined;
            return;
        };
        let ast = b.ast();
        let def = ast.definition(d).unwrap_or(d);
        let params: Vec<DeclId> = ast
            .decls
            .function(def)
            .map(|f| ast.lists.get(f.params).iter().map(|&p| DeclId::new(p)).collect())
            .unwrap_or_default();
        let fty = ast.decls.get(def).ty().unwrap_or(QualType::INT);
        let ret_q = match ast.types.canonical_view(fty) {
            TypeView::Function(p) => p.ret(),
            _ => QualType::INT,
        };
        let std_ice = is_std_is_constant_evaluated(ast, def);
        let mut c = Compiler::new(self, b);
        c.std_ice = std_ice;
        let rty = c.ty(ret_q);
        let ret = if c.o.types.is_aggregate(rty) {
            RetKind::Sret(rty)
        } else if c.o.types.kind(rty) == TyKind::Void {
            RetKind::Void
        } else {
            RetKind::Scalar(rty)
        };
        c.ret = ret;
        c.ret_ref = c.b.ast().types.is_reference(ret_q);
        c.scan(body, &params);
        c.scan_inits(def);
        c.cur_fn = Some(def);
        c.cur_params.clone_from(&params);
        if matches!(ret, RetKind::Sret(_)) {
            c.reg();
        }
        let mut this_ty = None;
        if class::has_this(c.b.ast(), def) {
            c.this_reg = Some(c.reg());
            let ast = c.b.ast();
            let cls = ast.decls.get(def).ctx().and_then(|k| ast.decls.get(ast.first(k)).ty());
            this_ty = cls.map(|q| c.ty(q));
        }
        let mut infos = Vec::with_capacity(params.len());
        for &p in &params {
            let q = c.b.ast().decls.get(p).ty().unwrap_or(QualType::INT);
            let ty = c.ty(q);
            let r = c.reg();
            let is_ref = c.b.ast().types.is_reference(q);
            let mem = !is_ref && (c.o.types.is_aggregate(ty) || c.mem_vars.contains(&p));
            infos.push(ParamInfo { decl: p, ty, reg: r, mem, is_ref });
        }
        let entry = c.here();
        for pi in &infos {
            if pi.is_ref || c.o.types.is_aggregate(pi.ty) {
                // The caller bound the reference or materialized the
                // argument; the register holds its address.
                c.locals.insert(pi.decl, Local::Mem(pi.reg));
            } else if pi.mem {
                let p = c.reg();
                c.alloc_var(p, pi.ty, pi.decl, false);
                c.emit(Op::InitStore, 0, &[p, pi.reg, pi.ty.0]);
                c.locals.insert(pi.decl, Local::Mem(p));
            } else {
                c.locals.insert(pi.decl, Local::Reg(pi.reg));
            }
        }
        c.member_prologue(def);
        c.stmt(body);
        // Falling off the end ([stmt.return]/4).
        if ret == RetKind::Void {
            c.emit(Op::Ret, 0, &[0]);
        } else {
            let ast = c.b.ast();
            let loc = ast.stmts.rbrace(body);
            c.trap_loc(loc, "control reached end of constexpr function".into());
        }
        let nregs = c.max;
        let this_reg = c.this_reg;
        let memo = c.o.budget.memo_entries > 0
            && !c.uses_cc
            && this_reg.is_none()
            && !matches!(ret, RetKind::Sret(_) | RetKind::Void)
            && infos.len() <= 4
            && infos.iter().all(|p| {
                matches!(c.o.types.scal(p.ty), Some(Scal::Int { .. } | Scal::Bool | Scal::Float(_)))
            })
            && !matches!(ret, RetKind::Scalar(t) if c.o.types.scal(t) == Some(Scal::Ptr));
        let f = &mut self.funcs[idx as usize];
        f.entry = entry;
        f.state = FState::Ready;
        f.nregs = nregs;
        f.ret = ret;
        f.params = infos;
        f.memo = memo;
        f.this_reg = this_reg;
        f.this_ty = this_ty;
    }
}

pub(crate) struct Compiler<'a> {
    pub o: &'a mut Oven,
    pub b: &'a mut dyn SemaBridge,
    next: u32,
    pub max: u32,
    locals: FxHashMap<DeclId, Local>,
    mem_vars: FxHashSet<DeclId>,
    pending: Vec<Loc>,
    breakables: Vec<Breakable>,
    /// The enclosing statement expressions: the start of the statement of
    /// each being evaluated, and how many breakables were open outside it
    /// (a `return`, or a `break`/`continue` leaving it, is unsupported).
    stmt_exprs: Vec<(Loc, usize)>,
    /// Mark registers of the open scopes (`None`: allocates nothing).
    scopes: Vec<Option<u32>>,
    ret: RetKind,
    /// The function returns a reference (`ret` is the address).
    ret_ref: bool,
    /// The body creates temporaries (scopes are needed per statement).
    has_temps: bool,
    /// The operator location of the innermost read-modify-write being
    /// compiled (clang reports union active-member faults there).
    rmw_loc: Option<Loc>,
    /// The evaluated common expressions of the enclosing GNU binary
    /// conditionals, keyed by the expression (an `OpaqueValueExpr` names
    /// its source).
    opaque: FxHashMap<ExprId, Opaque>,
    /// The function being compiled is `std::is_constant_evaluated`.
    std_ice: bool,
    /// The code reads the evaluation context (`Op::IsConstEval`): the
    /// function is not memoized.
    uses_cc: bool,
    /// The register holding `this` (a non-static member function).
    this_reg: Option<u32>,
    /// The object a default member initializer of an aggregate is
    /// initializing (its `this`, [class.mem.general]/11).
    this_override: Option<u32>,
    /// The storage of the lifetime-extended temporaries of the variable
    /// being initialized, allocated before its initializer's cleanups
    /// scope (keyed by the `MaterializeTemporaryExpr`).
    ext_temps: FxHashMap<ExprId, u32>,
    /// Local class objects declared `const`: allocated modifiable, made
    /// const once initialized ([class.ctor.general]/5).
    late_const: FxHashSet<DeclId>,
    /// The function being compiled (`None`: a thunk).
    cur_fn: Option<DeclId>,
    /// Its parameters (forwarded by an inheriting constructor).
    cur_params: Vec<DeclId>,
    /// The index register of the innermost `ArrayInitLoopExpr`.
    array_index: Option<u32>,
    /// The outermost default argument being compiled that holds a
    /// source-location builtin: the builtins in it take the values of
    /// that use (clang `SourceLocExprScopeGuard`; spec/oven.md, Builtins).
    srcloc: Option<ExprId>,
}

/// The value an `OpaqueValueExpr` stands for.
#[derive(Clone, Copy)]
enum Opaque {
    Rv(u32),
    Lv(Place),
}

impl<'a> Compiler<'a> {
    fn new(o: &'a mut Oven, b: &'a mut dyn SemaBridge) -> Self {
        Compiler {
            o,
            b,
            next: 0,
            max: 0,
            locals: FxHashMap::default(),
            mem_vars: FxHashSet::default(),
            pending: Vec::new(),
            rmw_loc: None,
            opaque: FxHashMap::default(),
            breakables: Vec::new(),
            stmt_exprs: Vec::new(),
            scopes: Vec::new(),
            ret: RetKind::Void,
            ret_ref: false,
            has_temps: false,
            std_ice: false,
            uses_cc: false,
            this_reg: None,
            this_override: None,
            ext_temps: FxHashMap::default(),
            late_const: FxHashSet::default(),
            cur_fn: None,
            cur_params: Vec::new(),
            array_index: None,
            srcloc: None,
        }
    }

    // ---- emission -------------------------------------------------------

    fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let idx = self.o.step_locs.len() as u32;
        let n = self.pending.len() as u32;
        self.o.step_locs.append(&mut self.pending);
        self.o.code.push(Op::Step as u32 | n << 8);
        self.o.code.push(idx);
    }

    fn emit(&mut self, op: Op, aux: u32, args: &[u32]) -> usize {
        debug_assert_eq!(args.len(), op.operands(), "{op:?}");
        self.flush();
        let pc = self.o.code.len();
        self.o.code.push(op as u32 | aux << 8);
        self.o.code.extend_from_slice(args);
        pc
    }

    /// The place of the operand `x` of read-modify-write `op_e`.
    fn lv_rmw(&mut self, x: ExprId, op_e: ExprId) -> Place {
        let saved = self.rmw_loc.replace(self.b.ast().exprs.get(op_e).loc());
        let p = self.lv(x, Access::Rmw);
        self.rmw_loc = saved;
        p
    }

    fn emit_site(&mut self, op: Op, aux: u32, args: &[u32], e: ExprId, loc: Loc) -> usize {
        let pc = self.emit(op, aux, args);
        self.o.sites.push(Site { pc: pc as u32, loc, e: Some(e) });
        pc
    }

    fn sites_len(&self) -> usize {
        self.o.sites.len()
    }

    fn emit_at(&mut self, op: Op, aux: u32, args: &[u32], e: ExprId) -> usize {
        let pc = self.emit(op, aux, args);
        let loc = self.b.ast().exprs.get(e).loc();
        self.o.sites.push(Site { pc: pc as u32, loc, e: Some(e) });
        pc
    }

    /// The begin location of `e` (spec/ast.md: ranges are computed by
    /// walking to the leftmost child). clang reports an lvalue-to-rvalue
    /// conversion's notes at the begin of the converted expression.
    fn begin(&self, e: ExprId) -> Loc {
        let ast = self.b.ast();
        let mut e = e;
        loop {
            let n = ast.exprs.get(e);
            e = match n.kind() {
                ExprKind::BinaryOperator
                | ExprKind::CompoundAssignOperator
                | ExprKind::ArraySubscriptExpr
                | ExprKind::ConditionalOperator
                | ExprKind::MaterializeTemporaryExpr => ExprId::new(n.a()),
                ExprKind::CallExpr | ExprKind::CXXMemberCallExpr => ast.exprs.callee(e),
                // A prefix unary operator begins at its token.
                ExprKind::CXXOperatorCallExpr => {
                    use kiln_ast::names::OverloadedOperator as O;
                    let args = ast.exprs.call_args(e);
                    let prefix = ast.lists.get(args).len() == 1
                        && !matches!(ast.exprs.operator_call_op(e), O::Call | O::Subscript | O::Arrow);
                    match ast.lists.at::<ExprId>(args, 0) {
                        Some(a) if !prefix => a,
                        _ => return n.loc(),
                    }
                }
                ExprKind::CXXRewrittenBinaryOperator => ast.exprs.rewritten_semantic(e),
                ExprKind::CXXBindTemporaryExpr | ExprKind::ExprWithCleanups => ast.exprs.operand(e),
                ExprKind::BinaryConditionalOperator => {
                    ast.exprs.binary_conditional_operands(e, &ast.lists)[0]
                }
                ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr => ast.exprs.operand(e),
                ExprKind::MemberExpr => {
                    let base = ast.exprs.operand(e);
                    // An implicit `this` has no source of its own; an
                    // implicit anonymous-member step does (its base).
                    if ast.exprs.get(base).kind() == ExprKind::CXXThisExpr
                        && ast.exprs.this_is_implicit(base)
                    {
                        return n.loc();
                    }
                    base
                }
                ExprKind::UnaryOperator if ast.exprs.unary_op(e).is_postfix() => {
                    ast.exprs.operand(e)
                }
                _ => return n.loc(),
            };
        }
    }

    fn emit_loc(&mut self, op: Op, aux: u32, args: &[u32], loc: Loc) -> usize {
        let pc = self.emit(op, aux, args);
        self.o.sites.push(Site { pc: pc as u32, loc, e: None });
        pc
    }

    /// The current position as a jump target.
    fn here(&mut self) -> u32 {
        self.flush();
        self.o.code.len() as u32
    }

    /// Emits a jump; returns the index of its target word.
    fn jmp(&mut self) -> usize {
        self.emit(Op::Jmp, 0, &[0]) + 1
    }

    fn jmp_if(&mut self, cond: u32, when: bool) -> usize {
        self.emit(if when { Op::JmpIf } else { Op::JmpIfNot }, 0, &[cond, 0]) + 2
    }

    fn patch(&mut self, at: usize, target: u32) {
        self.o.code[at] = target;
    }

    fn patch_here(&mut self, at: usize) {
        let t = self.here();
        self.patch(at, t);
    }

    fn trap_loc(&mut self, loc: Loc, msg: String) {
        self.trap_notes(loc, msg, Vec::new());
    }

    fn trap_notes(&mut self, loc: Loc, msg: String, extra: Vec<Note>) {
        let i = self.o.traps.len() as u32;
        self.o.traps.push(TrapInfo { loc, msg, extra });
        self.emit_loc(Op::Trap, 0, &[i], loc);
    }

    fn trap(&mut self, e: ExprId, msg: &str) {
        let loc = self.b.ast().exprs.get(e).loc();
        self.trap_loc(loc, msg.to_owned());
    }

    /// [expr.const]/10.22 (as clang 19, before P3068): evaluating a
    /// throw-expression is not a core constant expression; the generic
    /// note at the `throw`.
    fn throw_invalid(&mut self, e: ExprId) {
        self.trap(e, "subexpression not valid in a constant expression");
    }

    fn unsupported(&mut self, e: ExprId) {
        let k = self.b.ast().exprs.get(e).kind();
        self.trap(e, &format!("kiln: constant evaluation of {k:?} is not supported yet"));
    }

    fn reg(&mut self) -> u32 {
        let r = self.next;
        self.next += 1;
        self.max = self.max.max(self.next);
        r
    }

    pub fn ty(&mut self, q: QualType) -> OTy {
        self.o.types.intern(&mut *self.b, q)
    }

    pub fn ety(&mut self, e: ExprId) -> OTy {
        let q = self.b.ast().exprs.get(e).ty();
        self.ty(q)
    }

    fn konst(&mut self, dst: u32, v: u128) {
        if v == (v as i64) as i128 as u128 {
            self.emit(Op::Const, 1, &[dst, v as u32, (v >> 32) as u32]);
        } else if v >> 64 == 0 {
            self.emit(Op::Const, 0, &[dst, v as u32, (v >> 32) as u32]);
        } else {
            self.emit(
                Op::Const2,
                0,
                &[dst, v as u32, (v >> 32) as u32, (v >> 64) as u32, (v >> 96) as u32],
            );
        }
    }

    fn alloc(&mut self, dst: u32, ty: OTy, origin: Origin, is_const: bool, e: ExprId) {
        let oi = self.o.push_origin(origin);
        self.emit_at(Op::Alloc, u32::from(is_const), &[dst, ty.0, oi], e);
    }

    fn alloc_var(&mut self, dst: u32, ty: OTy, d: DeclId, is_const: bool) {
        let oi = self.o.push_origin(Origin::Var(d));
        let loc = self.b.ast().decls.get(d).loc();
        self.emit_loc(Op::Alloc, u32::from(is_const), &[dst, ty.0, oi], loc);
    }
}

/// Whether an object of type `q` is const: its type, or for an array its
/// element type, is const-qualified ([basic.type.qualifier]/3).
pub(crate) fn const_object(ast: &kiln_ast::Ast, q: QualType) -> bool {
    let mut c = ast.types.canonical(q);
    while let Some(e) = ast.types.element(c) {
        c = ast.types.canonical(e);
    }
    c.has_const_bit()
}

/// clang's `note_constexpr_stmt_expr_unsupported`.
const STMT_EXPR_UNSUPPORTED: &str =
    "this use of statement expressions is not supported in a constant expression";

// ---- pre-scan: which locals need an address ----------------------------

struct Scan<'x> {
    ast: &'x kiln_ast::Ast,
    mem: &'x mut FxHashSet<DeclId>,
    cur: Option<DeclId>,
    temps: bool,
}

impl Scan<'_> {
    fn stmt(&mut self, s: StmtId) {
        let ast = self.ast;
        let n = *ast.stmts.get(s);
        match n.kind() {
            StmtKind::CompoundStmt => {
                for &c in ast.lists.get(ast.stmts.compound_body(s)) {
                    self.stmt(StmtId::new(c));
                }
            }
            StmtKind::DeclStmt => {
                for &d in ast.lists.get(ast.stmts.decl_stmt_decls(s)) {
                    self.decl(DeclId::new(d));
                }
            }
            StmtKind::ExprStmt => self.expr(ast.stmts.expr_stmt_expr(s), true),
            StmtKind::IfStmt => {
                let d = *ast.stmts.if_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                d.cond_var.into_iter().for_each(|x| self.decl(x));
                d.cond.into_iter().for_each(|x| self.expr(x, false));
                d.then.into_iter().for_each(|x| self.stmt(x));
                d.els.into_iter().for_each(|x| self.stmt(x));
            }
            StmtKind::SwitchStmt => {
                let d = *ast.stmts.switch_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                d.cond_var.into_iter().for_each(|x| self.decl(x));
                self.expr(d.cond, false);
                self.stmt(d.body);
            }
            StmtKind::CaseStmt => self.stmt(ast.stmts.case_data(s).sub),
            StmtKind::DefaultStmt => self.stmt(ast.stmts.default_sub(s)),
            StmtKind::LabelStmt => self.stmt(StmtId::new(n.b())),
            StmtKind::WhileStmt | StmtKind::DoStmt | StmtKind::ForStmt => {
                let d = *ast.stmts.loop_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                d.cond_var.into_iter().for_each(|x| self.decl(x));
                d.cond.into_iter().for_each(|x| self.expr(x, false));
                d.inc.into_iter().for_each(|x| self.expr(x, true));
                d.body.into_iter().for_each(|x| self.stmt(x));
            }
            StmtKind::ReturnStmt => {
                if let Some(v) = ast.stmts.return_value(s) {
                    self.expr(v, false);
                }
            }
            _ => {}
        }
    }

    fn decl(&mut self, d: DeclId) {
        let ast = self.ast;
        if ast.decls.get(d).kind() != DeclKind::Var {
            return;
        }
        if let Some(init) = ast.decls.var(d).and_then(|v| v.init) {
            // A `__builtin_bit_cast` result may be indeterminate
            // ([bit.cast]/2): the variable is built in memory.
            let mut x = init;
            while matches!(ast.exprs.get(x).kind(), ExprKind::ParenExpr | ExprKind::ConstantExpr) {
                x = ast.exprs.operand(x);
            }
            if ast.exprs.get(x).kind() == ExprKind::BuiltinBitCastExpr {
                self.mem.insert(d);
            }
            let saved = self.cur.replace(d);
            self.expr(init, false);
            self.cur = saved;
        }
    }

    fn expr(&mut self, e: ExprId, ok: bool) {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        match n.kind() {
            ExprKind::DeclRefExpr => {
                let d = ast.exprs.decl(e);
                if (!ok || self.cur == Some(d)) && is_local_var(ast, d) {
                    self.mem.insert(d);
                }
            }
            ExprKind::ParenExpr => self.expr(ast.exprs.operand(e), ok),
            ExprKind::SubstNonTypeTemplateParmExpr => self.expr(ast.exprs.subst_replacement(e), ok),
            ExprKind::CXXDefaultArgExpr => {
                if let Some(d) = default_arg_of(ast, e) {
                    self.expr(d, false);
                }
            }
            k if k.is_cast() => {
                let op = ast.exprs.operand(e);
                match ast.exprs.cast_kind(e) {
                    CastKind::LValueToRValue => self.expr(op, true),
                    CastKind::NoOp if n.value_category().is_glvalue() => self.expr(op, ok),
                    CastKind::ToVoid => self.expr(op, true),
                    _ => self.expr(op, false),
                }
            }
            ExprKind::UnaryOperator => {
                let op = ast.exprs.unary_op(e);
                let x = ast.exprs.operand(e);
                match op {
                    UnaryOp::PreInc | UnaryOp::PreDec => self.expr(x, ok),
                    UnaryOp::PostInc | UnaryOp::PostDec => self.expr(x, true),
                    _ => self.expr(x, false),
                }
            }
            ExprKind::BinaryOperator => {
                // R-WALK: binary spines of any depth, with a worklist
                // (the scan only collects a set, so order is free).
                let mut work = vec![(e, ok)];
                while let Some((x, ok)) = work.pop() {
                    let k = ast.exprs.get(x).kind();
                    if k == ExprKind::ParenExpr {
                        work.push((ast.exprs.operand(x), ok));
                        continue;
                    }
                    if k != ExprKind::BinaryOperator {
                        self.expr(x, ok);
                        continue;
                    }
                    let [a, b] = ast.exprs.operands(x);
                    match ast.exprs.binary_op(x) {
                        BinaryOp::Assign => work.extend([(a, ok), (b, false)]),
                        BinaryOp::Comma => work.extend([(a, true), (b, ok)]),
                        _ => work.extend([(a, false), (b, false)]),
                    }
                }
            }
            ExprKind::CompoundAssignOperator => {
                self.expr(ExprId::new(n.a()), ok);
                self.expr(ast.exprs.compound_assign(e).rhs, false);
            }
            ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::OffsetOfExpr
            | ExprKind::TypeTraitExpr
            | ExprKind::ArrayTypeTraitExpr
            | ExprKind::ExpressionTraitExpr => {}
            ExprKind::CallExpr
            | ExprKind::CXXMemberCallExpr
            | ExprKind::CXXOperatorCallExpr
            | ExprKind::CXXConstructExpr
            | ExprKind::CXXTemporaryObjectExpr => {
                let ty = n.ty();
                if matches!(
                    ast.types.canonical_view(ty),
                    TypeView::Record(_) | TypeView::ConstantArray(_)
                ) {
                    self.temps = true;
                }
                for c in ast.exprs.children(e, &ast.lists) {
                    let cty = ast.exprs.get(c).ty();
                    if matches!(ast.types.canonical_view(cty), TypeView::Record(_)) {
                        self.temps = true;
                    }
                    self.expr(c, false);
                }
            }
            ExprKind::StmtExpr => self.stmt(ast.exprs.stmt_expr_body(e)),
            ExprKind::MaterializeTemporaryExpr
            | ExprKind::CompoundLiteralExpr
            | ExprKind::ExprWithCleanups
            | ExprKind::CXXBindTemporaryExpr => {
                self.temps = true;
                for c in ast.exprs.children(e, &ast.lists) {
                    self.expr(c, false);
                }
            }
            _ => {
                for c in ast.exprs.children(e, &ast.lists) {
                    self.expr(c, false);
                }
            }
        }
    }
}

/// The steps clang charges while hunting for a `case` label through `s`
/// without finding it (`EvaluateStmt` with a non-null `Case`).
fn hunt_cost(ast: &kiln_ast::Ast, s: StmtId, locs: &mut Vec<Loc>) {
    let n = *ast.stmts.get(s);
    locs.push(n.loc());
    match n.kind() {
        StmtKind::CaseStmt => hunt_cost(ast, ast.stmts.case_data(s).sub, locs),
        StmtKind::DefaultStmt => hunt_cost(ast, ast.stmts.default_sub(s), locs),
        StmtKind::LabelStmt => hunt_cost(ast, StmtId::new(n.b()), locs),
        StmtKind::CompoundStmt => {
            for &c in ast.lists.get(ast.stmts.compound_body(s)) {
                hunt_cost(ast, StmtId::new(c), locs);
            }
        }
        StmtKind::DoStmt | StmtKind::WhileStmt | StmtKind::ForStmt => {
            if let Some(b) = ast.stmts.loop_data(s).body {
                hunt_cost(ast, b, locs);
            }
        }
        StmtKind::IfStmt => {
            let d = *ast.stmts.if_data(s);
            d.then.into_iter().chain(d.els).for_each(|x| hunt_cost(ast, x, locs));
        }
        _ => {}
    }
}

/// Whether `s` contains a `case`/`default` label (stopping at nested
/// switches).
fn has_case(ast: &kiln_ast::Ast, s: StmtId) -> bool {
    let n = *ast.stmts.get(s);
    match n.kind() {
        StmtKind::CaseStmt | StmtKind::DefaultStmt => true,
        StmtKind::LabelStmt => has_case(ast, StmtId::new(n.b())),
        StmtKind::CompoundStmt => {
            ast.lists.get(ast.stmts.compound_body(s)).iter().any(|&c| has_case(ast, StmtId::new(c)))
        }
        StmtKind::DoStmt | StmtKind::WhileStmt | StmtKind::ForStmt => {
            ast.stmts.loop_data(s).body.is_some_and(|x| has_case(ast, x))
        }
        StmtKind::IfStmt => {
            let d = *ast.stmts.if_data(s);
            d.then.into_iter().chain(d.els).any(|x| has_case(ast, x))
        }
        _ => false,
    }
}

struct CaseTarget {
    stmt: StmtId,
    /// `None` for `default`.
    range: Option<(u128, u128)>,
    pre: Vec<Loc>,
    stub_jmp: usize,
}

impl Compiler<'_> {
    fn scan(&mut self, body: StmtId, params: &[DeclId]) {
        let ast = self.b.ast();
        let mut s = Scan { ast, mem: &mut self.mem_vars, cur: None, temps: false };
        s.stmt(body);
        self.has_temps = s.temps;
        let _ = params;
    }

    /// Counts statement `s` as a step; the step-limit note is at its begin
    /// (clang's `Stmt::getBeginLoc`, the leftmost token of an expression
    /// statement).
    fn step(&mut self, s: StmtId) {
        let ast = self.b.ast();
        let loc = if ast.stmts.get(s).kind() == StmtKind::ExprStmt {
            self.begin(ast.stmts.expr_stmt_expr(s))
        } else {
            ast.stmts.get(s).loc()
        };
        self.pending.push(loc);
    }

    /// Whether local `d` of type `ty` lives in memory.
    fn needs_mem(&self, d: DeclId, ty: OTy) -> bool {
        if is_reference_var(self.b.ast(), d) {
            // A register holding the referent's address.
            return false;
        }
        if self.o.types.is_aggregate(ty) || self.mem_vars.contains(&d) {
            return true;
        }
        self.b.ast().decls.var(d).is_some_and(|v| v.init.is_none())
    }

    /// Opens a scope allocating the memory locals among `decls`.
    fn open_scope(&mut self, decls: &[DeclId], force: bool) {
        let mut mem = Vec::new();
        let mut force = force;
        for &d in decls {
            let ast = self.b.ast();
            if !is_local_var(ast, d) || ast.decls.get(d).kind() != DeclKind::Var {
                continue;
            }
            // A reference may extend the lifetime of a temporary to the
            // end of this scope ([class.temporary]/6).
            force |= self.has_temps && is_reference_var(ast, d);
            let q = ast.decls.get(d).ty().unwrap_or(QualType::INT);
            let mut is_const = const_object(ast, q);
            let ty = self.ty(q);
            if self.needs_mem(d, ty) {
                // A const class object is modifiable until its
                // constructor completes ([class.ctor.general]/5).
                if is_const && let Some(late) = self.is_late_const(ty) {
                    is_const = false;
                    if late {
                        self.late_const.insert(d);
                    }
                }
                mem.push((d, ty, is_const));
            }
        }
        if mem.is_empty() && !force {
            self.scopes.push(None);
            return;
        }
        let mark = self.reg();
        self.emit(Op::ScopeEnter, 0, &[mark]);
        for (d, ty, is_const) in mem {
            let p = self.reg();
            self.alloc_var(p, ty, d, is_const);
            self.locals.insert(d, Local::Mem(p));
        }
        self.scopes.push(Some(mark));
    }

    fn close_scope(&mut self) {
        if let Some(Some(mark)) = self.scopes.pop() {
            self.emit(Op::ScopeExit, 0, &[mark]);
        }
    }

    /// Releases the scopes from `depth` on (before a jump out of them).
    fn exit_scopes(&mut self, depth: usize) {
        if let Some(&Some(mark)) = self.scopes[depth..].iter().find(|m| m.is_some()) {
            self.emit(Op::ScopeExit, 0, &[mark]);
        }
    }

    /// A GNU statement expression (clang `VisitStmtExpr`): the statements
    /// of its block in order, then its last statement, which must be an
    /// expression statement, as the value (into `dst` when given). A
    /// statement that leaves it by `return`, `break` or `continue` is not
    /// supported, as in clang: the note is at that top-level statement.
    fn stmt_expr(&mut self, e: ExprId, dst: Option<u32>) {
        let ast = self.b.ast();
        let body = ast.exprs.stmt_expr_body(e);
        let children = ast.lists.get(ast.stmts.compound_body(body)).to_vec();
        let Some((&last, init)) = children.split_last() else { return };
        let last = StmtId::new(last);
        let m = self.next;
        let decls = self.block_decls(&children);
        self.open_scope(&decls, false);
        for &c in init {
            let c = StmtId::new(c);
            let at = self.stmt_begin(c);
            self.stmt_exprs.push((at, self.breakables.len()));
            self.stmt(c);
            self.stmt_exprs.pop();
        }
        let ast = self.b.ast();
        if ast.stmts.get(last).kind() == StmtKind::ExprStmt {
            let x = ast.stmts.expr_stmt_expr(last);
            self.step(last);
            let temps = self.has_temps;
            if temps {
                self.open_scope(&[], true);
            }
            match dst {
                Some(dst) => self.rv_into(x, dst),
                None => self.effect(x),
            }
            if temps {
                self.close_scope();
            }
        } else {
            let at = self.stmt_begin(last);
            self.trap_loc(at, STMT_EXPR_UNSUPPORTED.into());
        }
        self.close_scope();
        self.next = m;
    }

    /// Where statement `s` begins.
    fn stmt_begin(&self, s: StmtId) -> Loc {
        let ast = self.b.ast();
        if ast.stmts.get(s).kind() == StmtKind::ExprStmt {
            self.begin(ast.stmts.expr_stmt_expr(s))
        } else {
            ast.stmts.get(s).loc()
        }
    }

    fn block_decls(&self, children: &[u32]) -> Vec<DeclId> {
        let ast = self.b.ast();
        let mut v = Vec::new();
        for &c in children {
            let mut s = StmtId::new(c);
            // `l: int x = 1;` declares `x` in the enclosing block.
            while ast.stmts.get(s).kind() == StmtKind::LabelStmt {
                s = StmtId::new(ast.stmts.get(s).b());
            }
            if ast.stmts.get(s).kind() == StmtKind::DeclStmt {
                v.extend(
                    ast.lists.get(ast.stmts.decl_stmt_decls(s)).iter().map(|&d| DeclId::new(d)),
                );
            }
        }
        v
    }

    fn stmt_decls(&self, s: Option<StmtId>, var: Option<DeclId>) -> Vec<DeclId> {
        let mut v = match s {
            Some(s) => self.block_decls(&[s.index()]),
            None => Vec::new(),
        };
        v.extend(var);
        v
    }

    fn stmt(&mut self, s: StmtId) {
        self.step(s);
        let ast = self.b.ast();
        let n = *ast.stmts.get(s);
        let m = self.next;
        match n.kind() {
            StmtKind::CompoundStmt => {
                let children = ast.lists.get(ast.stmts.compound_body(s)).to_vec();
                let decls = self.block_decls(&children);
                self.open_scope(&decls, false);
                for c in children {
                    self.stmt(StmtId::new(c));
                }
                self.close_scope();
                self.next = m;
            }
            StmtKind::DeclStmt => {
                let decls = ast.lists.get(ast.stmts.decl_stmt_decls(s)).to_vec();
                for d in decls {
                    self.decl(DeclId::new(d));
                }
            }
            StmtKind::ExprStmt => {
                let e = ast.stmts.expr_stmt_expr(s);
                if self.has_temps {
                    self.open_scope(&[], true);
                    self.effect(e);
                    self.close_scope();
                } else {
                    self.effect(e);
                }
                self.next = m;
            }
            StmtKind::NullStmt => {}
            StmtKind::IfStmt => {
                let d = *ast.stmts.if_data(s);
                let kind = ast.stmts.if_kind(s);
                let decls = self.stmt_decls(d.init, d.cond_var);
                self.open_scope(&decls, false);
                if let Some(i) = d.init {
                    self.stmt(i);
                }
                if let Some(v) = d.cond_var {
                    self.decl(v);
                }
                match kind {
                    // Constant evaluation is always manifestly constant-evaluated.
                    IfKind::Consteval => {
                        if let Some(t) = d.then {
                            self.stmt(t);
                        }
                    }
                    IfKind::NotConsteval => {
                        if let Some(t) = d.els {
                            self.stmt(t);
                        }
                    }
                    _ => {
                        let j = match d.cond {
                            Some(c) => {
                                let r = self.rv(c);
                                Some(self.jmp_if(r, false))
                            }
                            None => None,
                        };
                        if let Some(t) = d.then {
                            self.stmt(t);
                        }
                        if let Some(e) = d.els {
                            let end = self.jmp();
                            if let Some(j) = j {
                                self.patch_here(j);
                            }
                            self.stmt(e);
                            self.patch_here(end);
                        } else if let Some(j) = j {
                            self.patch_here(j);
                        }
                    }
                }
                self.close_scope();
                self.next = m;
            }
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                self.loop_stmt(s, n.kind());
                self.next = m;
            }
            StmtKind::SwitchStmt => {
                self.switch_stmt(s);
                self.next = m;
            }
            StmtKind::BreakStmt | StmtKind::ContinueStmt => {
                let brk = n.kind() == StmtKind::BreakStmt;
                let idx = if brk {
                    self.breakables.len().checked_sub(1)
                } else {
                    self.breakables.iter().rposition(|b| b.conts.is_some())
                };
                let Some(idx) = idx else {
                    self.trap_loc(n.loc(), "kiln: 'break' or 'continue' outside a loop".into());
                    return;
                };
                if let Some(&(at, outside)) = self.stmt_exprs.last()
                    && idx < outside
                {
                    self.trap_loc(at, STMT_EXPR_UNSUPPORTED.into());
                    return;
                }
                let depth =
                    if brk { self.breakables[idx].depth } else { self.breakables[idx].cont_depth };
                self.exit_scopes(depth);
                let j = self.jmp();
                let b = &mut self.breakables[idx];
                if brk {
                    b.breaks.push(j);
                } else if let Some(c) = &mut b.conts {
                    c.push(j);
                }
            }
            StmtKind::ReturnStmt => {
                if let Some(&(at, _)) = self.stmt_exprs.last() {
                    self.trap_loc(at, STMT_EXPR_UNSUPPORTED.into());
                    return;
                }
                let v = ast.stmts.return_value(s);
                match (self.ret, v) {
                    (RetKind::Sret(ty), Some(v)) => {
                        self.init(v, 0, ty);
                        self.emit(Op::Ret, 0, &[0]);
                    }
                    (RetKind::Scalar(_), Some(v)) if self.ret_ref => {
                        let r = self.reg();
                        self.bind_ref(v, r);
                        self.emit(Op::Ret, 1, &[r]);
                    }
                    (RetKind::Scalar(_), Some(v)) => {
                        let r = self.rv(v);
                        self.emit(Op::Ret, 1, &[r]);
                    }
                    (_, v) => {
                        if let Some(v) = v {
                            self.effect(v);
                        }
                        self.emit(Op::Ret, 0, &[0]);
                    }
                }
                self.next = m;
            }
            StmtKind::CaseStmt | StmtKind::DefaultStmt => {
                self.trap_loc(
                    n.loc(),
                    "kiln: a 'case' label outside its switch body is not supported yet".into(),
                );
            }
            // [expr.const]/5: evaluating a goto statement is not a core
            // constant expression (clang: the generic note at the `goto`).
            StmtKind::GotoStmt | StmtKind::IndirectGotoStmt => {
                self.trap_loc(n.loc(), "subexpression not valid in a constant expression".into());
            }
            // A label is evaluated by evaluating its statement (clang
            // charges a step for each: `EvaluateStmt` recurses).
            StmtKind::LabelStmt => self.stmt(StmtId::new(n.b())),
            // The payload of an attributed statement is not yet defined by
            // kiln_ast (spec/ast.md §6; no builder exists).
            StmtKind::AttributedStmt => {
                self.trap_loc(
                    n.loc(),
                    format!("kiln: constant evaluation of {:?} is not supported yet", n.kind()),
                );
            }
            k => {
                self.trap_loc(
                    n.loc(),
                    format!("kiln: constant evaluation of {k:?} is not supported yet"),
                );
            }
        }
    }

    fn loop_stmt(&mut self, s: StmtId, kind: StmtKind) {
        let d = *self.b.ast().stmts.loop_data(s);
        let decls = self.stmt_decls(d.init, None);
        self.open_scope(&decls, false);
        if let Some(i) = d.init {
            self.stmt(i);
        }
        let top = self.here();
        let iter = self.has_temps || d.cond_var.is_some();
        let break_depth = self.scopes.len();
        if iter {
            let decls: Vec<DeclId> = d.cond_var.into_iter().collect();
            self.open_scope(&decls, true);
        }
        let mut exit = None;
        if kind != StmtKind::DoStmt {
            if let Some(v) = d.cond_var {
                self.decl(v);
            }
            if let Some(c) = d.cond {
                let m = self.next;
                let r = self.rv(c);
                exit = Some(self.jmp_if(r, false));
                self.next = m;
            }
        }
        let cont_depth = self.scopes.len();
        self.breakables.push(Breakable {
            breaks: Vec::new(),
            conts: Some(Vec::new()),
            depth: break_depth,
            cont_depth,
        });
        if let Some(b) = d.body {
            self.stmt(b);
        }
        let b = self.breakables.pop().unwrap_or(Breakable {
            breaks: Vec::new(),
            conts: None,
            depth: 0,
            cont_depth: 0,
        });
        let cont = self.here();
        for j in b.conts.unwrap_or_default() {
            self.patch(j, cont);
        }
        if kind == StmtKind::DoStmt {
            let mut back = None;
            if let Some(c) = d.cond {
                let m = self.next;
                let r = self.rv(c);
                if iter {
                    let mark = self.scopes.last().copied().flatten().unwrap_or(0);
                    self.emit(Op::ScopeExit, 0, &[mark]);
                }
                back = Some(self.jmp_if(r, true));
                self.next = m;
            }
            if let Some(j) = back {
                self.patch(j, top);
            }
            if iter {
                self.scopes.pop();
            }
        } else {
            if let Some(i) = d.inc {
                let m = self.next;
                self.effect(i);
                self.next = m;
            }
            if iter {
                let mark = self.scopes.last().copied().flatten().unwrap_or(0);
                self.emit(Op::ScopeExit, 0, &[mark]);
            }
            let j = self.jmp();
            self.patch(j, top);
            if let Some(x) = exit {
                self.patch_here(x);
            }
            if iter {
                self.close_scope();
            }
        }
        let end = self.here();
        for j in b.breaks {
            self.patch(j, end);
        }
        self.close_scope();
    }

    fn switch_stmt(&mut self, s: StmtId) {
        let ast = self.b.ast();
        let d = *ast.stmts.switch_data(s);
        let body = d.body;
        let body_is_compound = ast.stmts.get(body).kind() == StmtKind::CompoundStmt;
        let children: Vec<StmtId> = if body_is_compound {
            ast.lists.get(ast.stmts.compound_body(body)).iter().map(|&c| StmtId::new(c)).collect()
        } else {
            vec![body]
        };
        // Collect the case labels: direct children of the body and their chains.
        let mut targets: Vec<CaseTarget> = Vec::new();
        let mut nested = false;
        let mut pre = Vec::new();
        if body_is_compound {
            pre.push(ast.stmts.get(body).loc());
        }
        for &c in &children {
            let mut chain = pre.clone();
            let mut cur = c;
            loop {
                let k = ast.stmts.get(cur).kind();
                let (range, sub) = match k {
                    StmtKind::CaseStmt => {
                        let cd = *ast.stmts.case_data(cur);
                        let lo = eval_int(ast, cd.lhs).unwrap_or(0);
                        let hi = cd.rhs.and_then(|r| eval_int(ast, r)).unwrap_or(lo);
                        (Some((lo, hi)), cd.sub)
                    }
                    StmtKind::DefaultStmt => (None, ast.stmts.default_sub(cur)),
                    _ => {
                        nested |= has_case(ast, cur);
                        break;
                    }
                };
                targets.push(CaseTarget { stmt: cur, range, pre: chain.clone(), stub_jmp: 0 });
                chain.push(ast.stmts.get(cur).loc());
                cur = sub;
            }
            hunt_cost(ast, c, &mut pre);
        }
        let ctx_decls = self.stmt_decls(d.init, d.cond_var);
        self.open_scope(&ctx_decls, false);
        if let Some(i) = d.init {
            self.stmt(i);
        }
        if let Some(v) = d.cond_var {
            self.decl(v);
        }
        let cond_ty = self.ety(d.cond);
        let (cw, cs) = match self.o.types.kind(cond_ty) {
            TyKind::Int { width, signed } => (u32::from(width), signed),
            _ => (8, false),
        };
        let c = self.rv(d.cond);
        if nested {
            let loc = self.b.ast().stmts.get(s).loc();
            self.trap_loc(loc, "kiln: a 'case' label nested in a sub-statement of the switch body is not supported yet".into());
        }
        let block_decls = if body_is_compound {
            let ch: Vec<u32> = children.iter().map(|s| s.index()).collect();
            self.block_decls(&ch)
        } else {
            Vec::new()
        };
        self.open_scope(&block_decls, false);
        // Dispatch.
        let t = self.reg();
        let mut stubs: Vec<(usize, usize)> = Vec::new();
        let aux_eq = int_aux(cw as u8, cs) | cmp::EQ << 9;
        for (i, tg) in targets.iter().enumerate() {
            let Some((lo, hi)) = tg.range else { continue };
            let (lo, hi) = (crate::arith::canon(lo, cw, cs), crate::arith::canon(hi, cw, cs));
            if lo == hi {
                self.konst(t, lo);
                self.emit(Op::ICmp, aux_eq, &[t, c, t]);
                let j = self.jmp_if(t, true);
                stubs.push((i, j));
            } else {
                let u = self.reg();
                self.konst(t, lo);
                self.emit(Op::ICmp, int_aux(cw as u8, cs) | cmp::GE << 9, &[u, c, t]);
                let skip = self.jmp_if(u, false);
                self.konst(t, hi);
                self.emit(Op::ICmp, int_aux(cw as u8, cs) | cmp::LE << 9, &[u, c, t]);
                let j = self.jmp_if(u, true);
                stubs.push((i, j));
                self.patch_here(skip);
            }
        }
        let default = targets.iter().position(|t| t.range.is_none());
        let none = self.jmp();
        if let Some(di) = default {
            stubs.push((di, none));
        }
        // Stubs charging the hunt's steps.
        for &(i, j) in &stubs {
            self.patch_here(j);
            let pre = targets[i].pre.clone();
            self.pending.extend(pre);
            targets[i].stub_jmp = self.jmp();
        }
        let mut end_jumps = Vec::new();
        if default.is_none() {
            end_jumps.push(none);
        }
        // The body.
        let depth = self.scopes.len() - 1;
        self.breakables.push(Breakable {
            breaks: Vec::new(),
            conts: None,
            depth,
            cont_depth: depth,
        });
        for &ch in &children {
            let mut cur = ch;
            loop {
                let ast = self.b.ast();
                let k = ast.stmts.get(cur).kind();
                let sub = match k {
                    StmtKind::CaseStmt => ast.stmts.case_data(cur).sub,
                    StmtKind::DefaultStmt => ast.stmts.default_sub(cur),
                    _ => {
                        self.stmt(cur);
                        break;
                    }
                };
                let label = self.here();
                for tg in &targets {
                    if tg.stmt == cur && tg.stub_jmp != 0 {
                        self.o.code[tg.stub_jmp] = label;
                    }
                }
                self.step(cur);
                cur = sub;
            }
        }
        let b = self.breakables.pop().map(|b| b.breaks).unwrap_or_default();
        self.close_scope();
        let end = self.here();
        for j in b.into_iter().chain(end_jumps) {
            self.patch(j, end);
        }
        // A target whose stub was never reached by the linear pass (cannot
        // happen for direct children) would jump to 0: point it at the end.
        for tg in &targets {
            if tg.stub_jmp != 0 && self.o.code[tg.stub_jmp] == 0 {
                self.o.code[tg.stub_jmp] = end;
            }
        }
        self.close_scope();
    }

    /// A declaration in a `DeclStmt` (or a condition variable).
    fn decl(&mut self, d: DeclId) {
        let ast = self.b.ast();
        let n = *ast.decls.get(d);
        if n.kind() != DeclKind::Var {
            return;
        }
        let Some(v) = ast.decls.var(d).copied() else { return };
        if !is_local_var(ast, d) {
            if v.spec.storage() == VarStorage::Static && !v.spec.is_constexpr() {
                self.trap_loc(
                    n.loc(),
                    "control flows through the definition of a static variable".into(),
                );
            }
            return;
        }
        if is_reference_var(ast, d) {
            let r = self.reg();
            if let Some(init) = v.init {
                let m = self.next;
                let ext = self.begin_extended(init);
                self.bind_ref(init, r);
                self.end_extended(&ext);
                self.next = m;
            }
            self.locals.insert(d, Local::Mem(r));
            return;
        }
        let q = n.ty().unwrap_or(QualType::INT);
        let ty = self.ty(q);
        let local = self.locals.get(&d).copied();
        match local {
            Some(Local::Mem(p)) => {
                let m = self.next;
                let ext = v.init.map(|i| self.begin_extended(i)).unwrap_or_default();
                if let Some(init) = v.init {
                    self.init(init, p, ty);
                }
                self.end_extended(&ext);
                self.end_local_init(d, p, ty);
                self.next = m;
            }
            _ => {
                if self.needs_mem(d, ty) {
                    // Not hoisted (a condition variable of a loop iteration).
                    let mut is_const = const_object(self.b.ast(), q);
                    if is_const && let Some(late) = self.is_late_const(ty) {
                        is_const = false;
                        if late {
                            self.late_const.insert(d);
                        }
                    }
                    let p = self.reg();
                    self.alloc_var(p, ty, d, is_const);
                    self.locals.insert(d, Local::Mem(p));
                    let m = self.next;
                    let ext = v.init.map(|i| self.begin_extended(i)).unwrap_or_default();
                    if let Some(init) = v.init {
                        self.init(init, p, ty);
                    }
                    self.end_extended(&ext);
                    self.end_local_init(d, p, ty);
                    self.next = m;
                } else {
                    let r = self.reg();
                    if let Some(init) = v.init {
                        let m = self.next;
                        self.rv_into(init, r);
                        self.next = m;
                    }
                    self.locals.insert(d, Local::Reg(r));
                }
            }
        }
    }
}

// ---- expressions ---------------------------------------------------------

/// `(width, signed)` of an integral or bool type.
fn int_of(k: TyKind) -> Option<(u32, bool)> {
    match k {
        TyKind::Bool => Some((1, false)),
        TyKind::Int { width, signed } => Some((u32::from(width), signed)),
        _ => None,
    }
}

/// The range of values ([dcl.enum]/8) of an enumeration type without a
/// fixed underlying type, as clang's `EnumDecl::getValueRange` computes it
/// from the enumerators' significant bits; `None` for other types (and
/// ranges beyond 64 bits).
fn enum_range(ast: &kiln_ast::Ast, q: QualType) -> Option<(i64, i64)> {
    let TypeView::Enum(d) = ast.types.canonical_view(q) else { return None };
    let d = ast.definition(d).unwrap_or(d);
    let data = ast.decls.enumeration(d)?;
    if data.flags & kiln_ast::decl::EnumData::FIXED != 0 {
        return None;
    }
    let (mut pos, mut neg) = (0u32, 0u32);
    for m in ast.members(d) {
        let Some(v) = ast.decls.enumerator(m).and_then(|x| x.value) else { continue };
        let Some(Scalar::Int { bits, signed, .. }) = ast.consts.scalar(v) else { continue };
        let w = bits.width();
        let v = crate::arith::canon(bits.to_u128()?, w, signed).cast_signed();
        if v >= 0 {
            pos = pos.max(128 - v.leading_zeros()).max(1);
        } else {
            neg = neg.max(129 - v.leading_ones());
        }
    }
    if neg == 0 {
        if pos >= 64 {
            return None;
        }
        Some((0, (1i64 << pos) - 1))
    } else {
        let n = (pos + 1).max(neg);
        if n > 64 {
            return None;
        }
        let m = 1i128 << (n - 1);
        Some((i64::try_from(-m).ok()?, i64::try_from(m - 1).ok()?))
    }
}

fn scalar_bits(s: &Scalar<'_>) -> Option<u128> {
    match s {
        Scalar::Int { bits, signed, .. } => {
            let w = bits.width();
            bits.to_u128().map(|v| crate::arith::canon(v, w, *signed))
        }
        Scalar::Bool { value, .. } => Some(u128::from(*value)),
        Scalar::Float { bits, .. } => Some(*bits),
        Scalar::Null { .. } => Some(0),
        _ => None,
    }
}

fn int_op(op: BinaryOp) -> Option<Op> {
    Some(match op {
        BinaryOp::Mul | BinaryOp::MulAssign => Op::IMul,
        BinaryOp::Div | BinaryOp::DivAssign => Op::IDiv,
        BinaryOp::Rem | BinaryOp::RemAssign => Op::IRem,
        BinaryOp::Add | BinaryOp::AddAssign => Op::IAdd,
        BinaryOp::Sub | BinaryOp::SubAssign => Op::ISub,
        BinaryOp::Shl | BinaryOp::ShlAssign => Op::IShl,
        BinaryOp::Shr | BinaryOp::ShrAssign => Op::IShr,
        BinaryOp::And | BinaryOp::AndAssign => Op::IAnd,
        BinaryOp::Xor | BinaryOp::XorAssign => Op::IXor,
        BinaryOp::Or | BinaryOp::OrAssign => Op::IOr,
        _ => return None,
    })
}

fn float_op(op: Op) -> Option<Op> {
    Some(match op {
        Op::IAdd => Op::FAdd,
        Op::ISub => Op::FSub,
        Op::IMul => Op::FMul,
        Op::IDiv => Op::FDiv,
        _ => return None,
    })
}

fn cmp_of(op: BinaryOp) -> Option<u32> {
    Some(match op {
        BinaryOp::EQ => cmp::EQ,
        BinaryOp::NE => cmp::NE,
        BinaryOp::LT => cmp::LT,
        BinaryOp::LE => cmp::LE,
        BinaryOp::GT => cmp::GT,
        BinaryOp::GE => cmp::GE,
        _ => return None,
    })
}

impl Compiler<'_> {
    fn ast(&self) -> &kiln_ast::Ast {
        self.b.ast()
    }

    fn node(&self, e: ExprId) -> kiln_ast::expr::ExprNode {
        *self.b.ast().exprs.get(e)
    }

    fn kind(&self, t: OTy) -> TyKind {
        self.o.types.kind(t)
    }

    /// Evaluates scalar prvalue `e` into some register (possibly a local's
    /// own register: the caller must not write it).
    fn rv(&mut self, e: ExprId) -> u32 {
        let ast = self.ast();
        let e0 = skip_parens(ast, e);
        let n = *ast.exprs.get(e0);
        if n.kind() == ExprKind::ImplicitCastExpr
            && ast.exprs.cast_kind(e0) == CastKind::LValueToRValue
        {
            let op = skip_parens(ast, ast.exprs.operand(e0));
            if ast.exprs.get(op).kind() == ExprKind::DeclRefExpr
                && ast.exprs.get(op).ty().quals_bits() & 2 == 0
                && let Some(Local::Reg(r)) = self.locals.get(&ast.exprs.decl(op)).copied()
            {
                return r;
            }
        }
        if matches!(n.kind(), ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr)
            && !n.value_category().is_glvalue()
        {
            return self.call(e0, None);
        }
        if n.kind() == ExprKind::CXXMemberCallExpr && !n.value_category().is_glvalue() {
            return self.member_call(e0, None);
        }
        let r = self.reg();
        self.rv_into(e, r);
        r
    }

    /// Evaluates scalar prvalue `e` into a fresh register.
    fn rv_copy(&mut self, e: ExprId) -> u32 {
        let r = self.reg();
        self.rv_into(e, r);
        r
    }

    fn rv_into(&mut self, e: ExprId, dst: u32) {
        let m = self.next.max(dst + 1);
        self.next = m;
        self.rv_inner(e, dst);
        self.next = m;
    }

    fn rv_inner(&mut self, e: ExprId, dst: u32) {
        let n = self.node(e);
        let ty = self.ety(e);
        match n.kind() {
            ExprKind::IntegerLiteral => {
                let v = eval_int(self.ast(), e).unwrap_or(0);
                let (w, s) = int_of(self.kind(ty)).unwrap_or((64, true));
                self.konst(dst, crate::arith::canon(v, w, s));
            }
            ExprKind::CharacterLiteral => {
                let v = u128::from(self.ast().exprs.char_value(e));
                let (w, s) = int_of(self.kind(ty)).unwrap_or((32, false));
                self.konst(dst, crate::arith::canon(v, w, s));
            }
            ExprKind::CXXBoolLiteralExpr => {
                let v = self.ast().exprs.bool_value(e);
                self.konst(dst, u128::from(v));
            }
            ExprKind::CXXNoexceptExpr => {
                let v = self.ast().exprs.noexcept_value(e);
                self.konst(dst, u128::from(v));
            }
            // spec/sema-templates.md §9.1, §9.4: Sema evaluated the trait.
            ExprKind::TypeTraitExpr | ExprKind::ExpressionTraitExpr => {
                let v = self.ast().exprs.type_trait_value(e);
                self.konst(dst, u128::from(v));
            }
            ExprKind::ArrayTypeTraitExpr => {
                let v = self.ast().exprs.array_trait(e).value;
                let (w, s) = int_of(self.kind(ty)).unwrap_or((64, false));
                self.konst(dst, crate::arith::canon(u128::from(v), w, s));
            }
            ExprKind::CXXNullPtrLiteralExpr
            | ExprKind::GNUNullExpr
            | ExprKind::ImplicitValueInitExpr => {
                self.konst(dst, 0);
            }
            ExprKind::FloatingLiteral => {
                let ast = self.ast();
                let v = ast
                    .exprs
                    .literal_const(e)
                    .and_then(|c| ast.consts.scalar(c))
                    .and_then(|s| scalar_bits(&s));
                self.konst(dst, v.unwrap_or(0));
            }
            ExprKind::ParenExpr => {
                let op = self.ast().exprs.operand(e);
                self.rv_inner(op, dst);
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                let op = self.ast().exprs.rewritten_semantic(e);
                self.rv_inner(op, dst);
            }
            ExprKind::SubstNonTypeTemplateParmExpr => {
                let op = self.ast().exprs.subst_replacement(e);
                self.rv_inner(op, dst);
            }
            ExprKind::CompoundLiteralExpr => {
                let init = self.ast().exprs.compound_literal_init(e);
                self.rv_inner(init, dst);
            }
            ExprKind::StmtExpr => self.stmt_expr(e, Some(dst)),
            ExprKind::CXXDefaultArgExpr => match default_arg_of(self.ast(), e) {
                Some(d) => {
                    let saved = self.enter_default_arg(e);
                    self.rv_inner(d, dst);
                    self.srcloc = saved;
                }
                None => self.unsupported(e),
            },
            ExprKind::SourceLocExpr => self.source_loc(e, dst),
            ExprKind::ConstantExpr => {
                let ast = self.ast();
                let v = ast
                    .exprs
                    .constant_value(e)
                    .and_then(|c| ast.consts.scalar(c))
                    .and_then(|s| scalar_bits(&s));
                if let Some(v) = v {
                    self.konst(dst, v);
                } else {
                    let op = ast.exprs.operand(e);
                    self.rv_inner(op, dst);
                }
            }
            ExprKind::DeclRefExpr => {
                let ast = self.ast();
                let d = ast.exprs.decl(e);
                if let Some(v) = ast.decls.enumerator(d).and_then(|x| x.value) {
                    let bits = ast.consts.scalar(v).and_then(|s| scalar_bits(&s)).unwrap_or(0);
                    let (w, s) = int_of(self.kind(ty)).unwrap_or((64, true));
                    self.konst(dst, crate::arith::canon(bits, w, s));
                } else if ast.decls.get(d).kind().is_function() {
                    let f = self.o.func_index(self.b.ast(), d);
                    self.konst(dst, u128::from(fn_ptr(f)));
                } else {
                    self.unsupported(e);
                }
            }
            ExprKind::UnaryExprOrTypeTraitExpr => self.trait_expr(e, dst, ty),
            ExprKind::OffsetOfExpr => {
                let v = u128::from(self.ast().exprs.offsetof_value(e));
                let (w, s) = int_of(self.kind(ty)).unwrap_or((64, false));
                self.konst(dst, crate::arith::canon(v, w, s));
            }
            ExprKind::UnaryOperator => self.unary(e, dst, ty),
            ExprKind::BinaryOperator => self.binary(e, dst, ty),
            ExprKind::CompoundAssignOperator => {
                let place = self.compound_assign(e);
                self.load(place, ty, ak::READ, e, dst);
            }
            ExprKind::ConditionalOperator => {
                let c = self.rv(ExprId::new(n.a()));
                let [t, f] = self.ast().exprs.cond_arms(e);
                let j = self.jmp_if(c, false);
                self.rv_into(t, dst);
                let end = self.jmp();
                self.patch_here(j);
                self.rv_into(f, dst);
                self.patch_here(end);
            }
            ExprKind::BinaryConditionalOperator => {
                let [common, cond, t, f] = self.bind_common(e, Access::Read);
                let c = self.rv(cond);
                let j = self.jmp_if(c, false);
                self.rv_into(t, dst);
                let end = self.jmp();
                self.patch_here(j);
                self.rv_into(f, dst);
                self.patch_here(end);
                self.opaque.remove(&common);
            }
            ExprKind::OpaqueValueExpr if !n.value_category().is_glvalue() => {
                match self.opaque_of(e) {
                    Some(Opaque::Rv(r)) => {
                        if r != dst {
                            self.emit(Op::Mov, 0, &[dst, r]);
                        }
                    }
                    Some(Opaque::Lv(p)) => self.load(p, ty, ak::READ, e, dst),
                    None => self.unsupported(e),
                }
            }
            ExprKind::CXXScalarValueInitExpr => self.konst(dst, 0),
            ExprKind::CXXThrowExpr => self.throw_invalid(e),
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr
                if !n.value_category().is_glvalue() =>
            {
                let r = self.call(e, None);
                if r != dst {
                    self.emit(Op::Mov, 0, &[dst, r]);
                }
            }
            ExprKind::InitListExpr => {
                let inits = self.ast().lists.get(self.ast().exprs.init_list_inits(e)).to_vec();
                match inits.first() {
                    Some(&x) => self.rv_inner(ExprId::new(x), dst),
                    None => self.konst(dst, 0),
                }
            }
            // Transient allocation (spec/oven.md "Transient allocation").
            ExprKind::CXXNewExpr => self.new_expr(e, dst),
            ExprKind::CXXDeleteExpr => self.delete_expr(e),
            ExprKind::CXXThisExpr => self.this_expr(e, dst),
            ExprKind::ExprWithCleanups => {
                let op = self.ast().exprs.operand(e);
                self.open_scope(&[], true);
                self.rv_inner(op, dst);
                self.close_scope();
            }
            ExprKind::CXXBindTemporaryExpr => {
                let op = self.ast().exprs.operand(e);
                self.rv_inner(op, dst);
            }
            ExprKind::CXXDefaultInitExpr => match self.default_init_of(e) {
                Some(x) => self.rv_inner(x, dst),
                None => self.unsupported(e),
            },
            ExprKind::ArrayInitIndexExpr => match self.array_index {
                Some(k) => {
                    self.emit(Op::Mov, 0, &[dst, k]);
                }
                None => self.unsupported(e),
            },
            ExprKind::CXXMemberCallExpr if !n.value_category().is_glvalue() => {
                let r = self.member_call(e, None);
                if r != dst {
                    self.emit(Op::Mov, 0, &[dst, r]);
                }
            }
            k if k.is_cast() => self.cast(e, dst, ty),
            _ if n.value_category().is_glvalue() => {
                // A glvalue used as a value (an lvalue-to-rvalue conversion
                // sema left implicit).
                let p = self.lv(e, Access::Read);
                self.load(p, ty, ak::READ, e, dst);
            }
            _ => self.unsupported(e),
        }
    }

    fn trait_expr(&mut self, e: ExprId, dst: u32, ty: OTy) {
        let ast = self.ast();
        let kind = ast.exprs.trait_kind(e);
        let q = if let Some(q) = ast.exprs.trait_type_arg(e) {
            q
        } else {
            let op = ExprId::new(ast.exprs.get(e).a());
            ast.exprs.get(op).ty()
        };
        // [expr.sizeof]/2, [expr.alignof]/3: of a reference type, the
        // referenced type's.
        let q = match ast.types.canonical_view(q) {
            TypeView::LValueRef(r) | TypeView::RValueRef(r) => r,
            _ => q,
        };
        let t = self.ty(q);
        let v = match kind {
            TraitKind::SizeOf => self.o.types.size(t),
            TraitKind::AlignOf | TraitKind::PreferredAlignOf => {
                u64::from(self.o.types.info(t).align)
            }
            _ => {
                self.unsupported(e);
                return;
            }
        };
        let (w, s) = int_of(self.kind(ty)).unwrap_or((64, false));
        self.konst(dst, crate::arith::canon(u128::from(v), w, s));
    }

    fn unary(&mut self, e: ExprId, dst: u32, ty: OTy) {
        let op = self.ast().exprs.unary_op(e);
        let x = self.ast().exprs.operand(e);
        match op {
            UnaryOp::Plus => self.rv_inner(x, dst),
            UnaryOp::Minus => {
                let r = self.rv(x);
                match self.kind(ty) {
                    TyKind::Float(k) => {
                        self.emit(Op::FNeg, float_code(k), &[dst, r]);
                    }
                    k => {
                        let (w, s) = int_of(k).unwrap_or((32, true));
                        self.emit_at(Op::INeg, int_aux(w as u8, s), &[dst, r], e);
                    }
                }
            }
            UnaryOp::Not => {
                let r = self.rv(x);
                let (w, s) = int_of(self.kind(ty)).unwrap_or((32, true));
                self.emit(Op::INot, int_aux(w as u8, s), &[dst, r]);
            }
            UnaryOp::LNot => {
                let r = self.rv(x);
                self.emit(Op::LNot, 0, &[dst, r]);
            }
            UnaryOp::AddrOf => {
                if matches!(self.kind(ty), TyKind::MemPtr { .. }) {
                    self.mp_const(e, x, dst);
                    return;
                }
                let x0 = skip_parens(self.ast(), x);
                let xn = self.node(x0);
                let xty = self.ety(x0);
                if self.kind(xty) == TyKind::Func {
                    let r = self.fn_lv(x0);
                    self.emit(Op::Mov, 0, &[dst, r]);
                    return;
                }
                let _ = xn;
                let p = self.lv_mem(x, Access::Addr);
                if p != dst {
                    self.emit(Op::Mov, 0, &[dst, p]);
                }
            }
            UnaryOp::Deref => {
                let p = self.lv(e, Access::Read);
                self.load(p, ty, ak::READ, e, dst);
            }
            UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec => {
                let place = self.lv_rmw(x, e);
                let xty = self.ety(x);
                self.inc_dec(place, xty, op, Some(dst), e);
            }
            _ => self.unsupported(e),
        }
    }

    /// Increment/decrement of `place`; `out` receives the old value for a
    /// postfix operator, the new one for a prefix one.
    fn inc_dec(&mut self, place: Place, ty: OTy, op: UnaryOp, out: Option<u32>, e: ExprId) {
        let inc = matches!(op, UnaryOp::PostInc | UnaryOp::PreInc);
        let akind = if inc { ak::INCREMENT } else { ak::DECREMENT };
        let old = self.reg();
        self.load(place, ty, akind, e, old);
        let new = self.reg();
        match self.kind(ty) {
            TyKind::Float(k) => {
                let one = self.reg();
                let bits = crate::arith::int_to_float(k, 1, true);
                self.konst(one, bits);
                self.emit_at(
                    if inc { Op::FAdd } else { Op::FSub },
                    float_code(k),
                    &[new, old, one],
                    e,
                );
            }
            TyKind::Ptr(pointee) => {
                let one = self.reg();
                self.konst(one, 1);
                let aux = int_aux(64, true) | u32::from(!inc) << 9;
                self.emit_at(Op::PtrAdd, aux, &[new, old, one, pointee.0], e);
            }
            k => {
                let (w, s) = int_of(k).unwrap_or((32, true));
                let one = self.reg();
                self.konst(one, 1);
                if k == TyKind::Bool {
                    // C++17 removed `++` on bool; `--` never existed. Sema
                    // rejects both; treat as `= true` for robustness.
                    self.konst(new, 1);
                } else {
                    self.emit_at(
                        if inc { Op::IAdd } else { Op::ISub },
                        int_aux(w as u8, s),
                        &[new, old, one],
                        e,
                    );
                }
            }
        }
        self.store(place, new, ty, akind, e);
        if let Some(out) = out {
            let postfix = matches!(op, UnaryOp::PostInc | UnaryOp::PostDec);
            let src = if postfix { old } else { new };
            if let Place::Bits { .. } = place
                && !postfix
            {
                // The value of the bit-field after the store.
                self.load(place, ty, ak::READ, e, out);
                return;
            }
            self.emit(Op::Mov, 0, &[out, src]);
        }
    }

    fn binary(&mut self, e: ExprId, dst: u32, ty: OTy) {
        let op = self.ast().exprs.binary_op(e);
        let [a, b] = self.ast().exprs.operands(e);
        match op {
            BinaryOp::Comma => {
                // R-WALK (sema-templates.md Q16): a comma fold is a spine
                // of any depth; every operand but the last is a discarded
                // value, in order.
                let ast = self.ast();
                let mut rest = vec![a];
                let mut last = b;
                let mut c = skip_parens(ast, b);
                while is_binary(ast, c, BinaryOp::Comma) {
                    let [x, y] = ast.exprs.operands(c);
                    rest.push(x);
                    last = y;
                    c = skip_parens(ast, y);
                }
                self.effects(&rest);
                self.rv_inner(last, dst);
            }
            BinaryOp::LAnd | BinaryOp::LOr => self.logical(e, dst),
            BinaryOp::Assign => {
                let place = self.assign(e);
                self.load(place, ty, ak::READ, e, dst);
            }
            _ if op.is_compound_assignment() => {
                let place = self.compound_assign(e);
                self.load(place, ty, ak::READ, e, dst);
            }
            _ => self.arith_spine(e, dst, ty),
        }
    }

    /// `&&` / `||` chain `e` into `dst`, iterating along both spines
    /// (R-WALK): a left-deep chain `((a && b) || c)` evaluates its
    /// leftmost operand and then each right operand behind a
    /// short-circuit jump; a right-deep tail `a && (b && c)` jumps
    /// straight to the end.
    fn logical(&mut self, e: ExprId, dst: u32) {
        let ast = self.ast();
        let mut left = vec![e];
        let mut cur = skip_parens(ast, ast.exprs.operands(e)[0]);
        while is_logical(ast, cur) {
            left.push(cur);
            cur = skip_parens(ast, ast.exprs.operands(cur)[0]);
        }
        self.rv_into(cur, dst);
        // Bottom-up along the left spine; the top node's right operand
        // opens the right spine.
        for &n in left[1..].iter().rev() {
            let ast = self.ast();
            let lor = ast.exprs.binary_op(n) == BinaryOp::LOr;
            let rhs = ast.exprs.operands(n)[1];
            let j = self.jmp_if(dst, lor);
            self.rv_into(rhs, dst);
            self.patch_here(j);
        }
        let mut ends = Vec::new();
        let mut n = e;
        loop {
            let ast = self.ast();
            let lor = ast.exprs.binary_op(n) == BinaryOp::LOr;
            let rhs = skip_parens(ast, ast.exprs.operands(n)[1]);
            ends.push(self.jmp_if(dst, lor));
            let ast = self.ast();
            if !is_logical(ast, rhs) {
                self.rv_into(rhs, dst);
                break;
            }
            // `rhs` is itself a chain: its left spine, then its tail.
            let mut l = vec![rhs];
            let mut c = skip_parens(ast, ast.exprs.operands(rhs)[0]);
            while is_logical(ast, c) {
                l.push(c);
                c = skip_parens(ast, ast.exprs.operands(c)[0]);
            }
            self.rv_into(c, dst);
            for &m in l[1..].iter().rev() {
                let ast = self.ast();
                let lor = ast.exprs.binary_op(m) == BinaryOp::LOr;
                let r = ast.exprs.operands(m)[1];
                let j = self.jmp_if(dst, lor);
                self.rv_into(r, dst);
                self.patch_here(j);
            }
            n = rhs;
        }
        for j in ends {
            self.patch_here(j);
        }
    }

    /// The discarded-value expressions `xs`, in order.
    fn effects(&mut self, xs: &[ExprId]) {
        for &x in xs {
            self.effect(x);
        }
    }

    /// A non-logical, non-assigning builtin binary operator `e` into
    /// `dst`, iterating along a left-deep (`((a + b) + c) + d`) or a
    /// right-deep (`a + (b + (c + d))`) spine of such operators (R-WALK,
    /// sema-templates.md Q16: fold expressions over 65,536 elements).
    fn arith_spine(&mut self, e: ExprId, dst: u32, ty: OTy) {
        let ast = self.ast();
        let [a, b] = ast.exprs.operands(e);
        let (la, rb) = (skip_parens(ast, a), skip_parens(ast, b));
        if is_plain_binary(ast, la) {
            // Left spine, bottom-up, results ping-ponging between two
            // registers.
            let mut spine = vec![la];
            let mut c = skip_parens(ast, ast.exprs.operands(la)[0]);
            while is_plain_binary(ast, c) {
                spine.push(c);
                c = skip_parens(ast, ast.exprs.operands(c)[0]);
            }
            let m = self.next.max(dst + 1);
            self.next = m;
            let regs = [self.reg(), self.reg()];
            let base = self.next;
            let mut prev = None;
            for (i, &n) in spine.iter().rev().enumerate() {
                let r = regs[i & 1];
                let nty = self.ety(n);
                self.next = base;
                self.binary_node(n, r, nty, prev, None);
                prev = Some(r);
            }
            self.next = base;
            self.binary_node(e, dst, ty, prev, None);
            self.next = m;
            return;
        }
        if is_plain_binary(ast, rb) {
            // Right spine: every left operand first, in order, then the
            // innermost node and the rest bottom-up.
            let mut spine = vec![e];
            let mut c = rb;
            while is_plain_binary(self.ast(), c) {
                spine.push(c);
                c = skip_parens(self.ast(), self.ast().exprs.operands(c)[1]);
            }
            let m = self.next.max(dst + 1);
            self.next = m;
            let lefts: Vec<u32> = spine
                .iter()
                .map(|&n| {
                    let x = self.ast().exprs.operands(n)[0];
                    self.rv_copy(x)
                })
                .collect();
            let regs = [self.reg(), self.reg()];
            let base = self.next;
            let mut prev = None;
            for (i, (&n, &l)) in spine.iter().zip(&lefts).enumerate().rev() {
                let (r, nty) = if i == 0 { (dst, ty) } else { (regs[i & 1], self.ety(n)) };
                self.next = base;
                self.binary_node(n, r, nty, Some(l), prev);
                prev = Some(r);
            }
            self.next = m;
            return;
        }
        self.binary_node(e, dst, ty, None, None);
    }

    /// Operand `x`, or the register `given` already holding its value
    /// (`copy`: into a fresh register, not a local's own).
    fn opnd(&mut self, x: ExprId, given: Option<u32>, copy: bool) -> u32 {
        match given {
            Some(r) => r,
            None if copy => self.rv_copy(x),
            None => self.rv(x),
        }
    }

    /// One builtin binary operator `e` into `dst`; `ga` / `gb` hold its
    /// operands' values when a spine walk evaluated them already.
    fn binary_node(&mut self, e: ExprId, dst: u32, ty: OTy, ga: Option<u32>, gb: Option<u32>) {
        let op = self.ast().exprs.binary_op(e);
        let [a, b] = self.ast().exprs.operands(e);
        let at = self.ety(a);
        let bt = self.ety(b);
        let (ak_, bk) = (self.kind(at), self.kind(bt));
        if let Some(c) = cmp_of(op) {
            let ra = self.opnd(a, ga, false);
            let rb = self.opnd(b, gb, false);
            match (ak_, bk) {
                (TyKind::Float(k), _) => {
                    self.emit_at(Op::FCmp, float_code(k) | c << 4, &[dst, ra, rb], e);
                }
                (TyKind::Ptr(p), _) | (_, TyKind::Ptr(p)) => {
                    self.emit_at(Op::PtrCmp, c, &[dst, ra, rb, p.0], e);
                }
                (TyKind::NullPtr, _) => {
                    self.konst(dst, u128::from(matches!(c, cmp::EQ | cmp::LE | cmp::GE)));
                }
                // Interned member pointers: equal values share an index.
                (TyKind::MemPtr { .. }, _) | (_, TyKind::MemPtr { .. }) => {
                    self.emit(Op::ICmp, int_aux(64, false) | c << 9, &[dst, ra, rb]);
                }
                (k, _) => {
                    let (w, s) = int_of(k).unwrap_or((32, true));
                    self.emit(Op::ICmp, int_aux(w as u8, s) | c << 9, &[dst, ra, rb]);
                }
            }
            return;
        }
        if matches!(op, BinaryOp::Add | BinaryOp::Sub) {
            if let (TyKind::Ptr(pa), TyKind::Ptr(_)) = (ak_, bk) {
                let ra = self.opnd(a, ga, false);
                let rb = self.opnd(b, gb, false);
                self.emit_at(Op::PtrSub, 0, &[dst, ra, rb, pa.0], e);
                return;
            }
            let ptr_side = match (ak_, bk) {
                (TyKind::Ptr(p), k) => Some((a, b, p, k)),
                (k, TyKind::Ptr(p)) => Some((b, a, p, k)),
                _ => None,
            };
            if let Some((pe, _ie, pointee, ik)) = ptr_side {
                let (rp, ri) = if pe == a {
                    let rp = self.opnd(a, ga, false);
                    (rp, self.opnd(b, gb, false))
                } else {
                    let ri = self.opnd(a, ga, false);
                    (self.opnd(b, gb, false), ri)
                };
                let (w, s) = int_of(ik).unwrap_or((64, true));
                let aux = int_aux(w as u8, s) | u32::from(op == BinaryOp::Sub) << 9;
                self.emit_at(Op::PtrAdd, aux, &[dst, rp, ri, pointee.0], e);
                return;
            }
        }
        let Some(iop) = int_op(op) else {
            self.unsupported(e);
            return;
        };
        let shift = matches!(iop, Op::IShl | Op::IShr);
        // [expr.shift]/4: the left operand is sequenced first.
        let ra = self.opnd(a, ga, shift);
        let rb = self.opnd(b, gb, false);
        self.arith(iop, ty, bt, dst, ra, rb, e);
    }

    /// `dst = a op b` in type `ty` (`bt`: the right operand's type, for
    /// shifts).
    fn arith(&mut self, iop: Op, ty: OTy, bt: OTy, dst: u32, ra: u32, rb: u32, e: ExprId) {
        match self.kind(ty) {
            TyKind::Float(k) => match float_op(iop) {
                Some(fop) => {
                    self.emit_at(fop, float_code(k), &[dst, ra, rb], e);
                }
                None => self.unsupported(e),
            },
            k => {
                let (w, s) = int_of(k).unwrap_or((32, true));
                let mut aux = int_aux(w as u8, s);
                if matches!(iop, Op::IShl | Op::IShr) {
                    let (cw, cs) = int_of(self.kind(bt)).unwrap_or((32, true));
                    aux |= cw << 9 | u32::from(cs) << 17;
                }
                self.emit_at(iop, aux, &[dst, ra, rb], e);
            }
        }
    }

    /// Converts `src` of type `from` to `to` into `dst`.
    fn convert(&mut self, src: u32, from: OTy, to: OTy, dst: u32, e: ExprId) {
        let (fk, tk) = (self.kind(from), self.kind(to));
        match (fk, tk) {
            _ if fk == tk => {
                if src != dst {
                    self.emit(Op::Mov, 0, &[dst, src]);
                }
            }
            (TyKind::Int { .. } | TyKind::Bool, TyKind::Bool) => {
                let (w, _) = int_of(fk).unwrap_or((32, true));
                self.emit(Op::ToBool, w, &[dst, src]);
            }
            (TyKind::Int { .. } | TyKind::Bool, TyKind::Int { width, signed }) => {
                let (w, s) = int_of(fk).unwrap_or((32, true));
                let aux = w | u32::from(s) << 8 | u32::from(width) << 9 | u32::from(signed) << 17;
                self.emit(Op::ICast, aux, &[dst, src]);
            }
            (TyKind::Int { .. } | TyKind::Bool, TyKind::Float(k)) => {
                let (w, s) = int_of(fk).unwrap_or((32, true));
                self.emit_at(Op::IToF, w | u32::from(s) << 8 | float_code(k) << 9, &[dst, src], e);
            }
            (TyKind::Float(k), TyKind::Bool) => {
                self.emit(Op::FToBool, float_code(k), &[dst, src]);
            }
            (TyKind::Float(k), TyKind::Int { width, signed }) => {
                let aux = float_code(k) | u32::from(width) << 4 | u32::from(signed) << 12;
                self.emit_at(Op::FToI, aux, &[dst, src], e);
            }
            (TyKind::Float(a), TyKind::Float(b)) => {
                self.emit_at(Op::FCast, float_code(a) | float_code(b) << 4, &[dst, src], e);
            }
            (TyKind::Ptr(_), TyKind::Bool) => {
                self.emit(Op::PtrToBool, 0, &[dst, src]);
            }
            (TyKind::NullPtr, TyKind::Bool) => self.konst(dst, 0),
            (TyKind::Ptr(_) | TyKind::NullPtr, TyKind::Ptr(_) | TyKind::NullPtr) => {
                if src != dst {
                    self.emit(Op::Mov, 0, &[dst, src]);
                }
            }
            _ => self.unsupported(e),
        }
    }

    fn cast(&mut self, e: ExprId, dst: u32, ty: OTy) {
        let ast = self.ast();
        let ck = ast.exprs.cast_kind(e);
        let op = ast.exprs.operand(e);
        let ek = ast.exprs.get(e).kind();
        if ek == ExprKind::CXXReinterpretCastExpr {
            // [expr.const]/5.15, whatever conversion it performs.
            self.trap(e, "reinterpret_cast is not allowed in a constant expression");
            return;
        }
        match ck {
            CastKind::LValueToRValue => {
                let q = self.node(op).ty();
                if q.quals_bits() & 2 != 0 {
                    let msg = format!(
                        "read of volatile-qualified type '{}' is not allowed in a constant expression",
                        Oven::type_name(ast, q)
                    );
                    let loc = self.begin(op);
                    self.trap_loc(loc, msg);
                    return;
                }
                let p = self.lv(op, Access::Read);
                let at = self.sites_len();
                self.load(p, ty, ak::READ, op, dst);
                let begin = self.begin(op);
                self.o.sites[at..].iter_mut().for_each(|s| s.loc = begin);
            }
            CastKind::NoOp | CastKind::UserDefinedConversion | CastKind::ConstructorConversion => {
                self.rv_inner(op, dst);
            }
            CastKind::DerivedToBase
            | CastKind::UncheckedDerivedToBase
            | CastKind::BaseToDerived => {
                let p = self.rv(op);
                self.class_cast(e, p, dst, true, ck == CastKind::BaseToDerived);
            }
            CastKind::ToVoid => self.effect(op),
            CastKind::NullToPointer | CastKind::NullToMemberPointer => {
                self.effect(op);
                self.konst(dst, 0);
            }
            CastKind::BaseToDerivedMemberPointer | CastKind::DerivedToBaseMemberPointer => {
                self.mp_conv(e, dst, ck);
            }
            CastKind::MemberPointerToBoolean => {
                let r = self.rv(op);
                let z = self.reg();
                self.konst(z, 0);
                self.emit(Op::ICmp, int_aux(64, false) | cmp::NE << 9, &[dst, r, z]);
                self.next = z;
            }
            CastKind::ArrayToPointerDecay => {
                let p = self.lv_mem(op, Access::Addr);
                if p != dst {
                    self.emit(Op::Mov, 0, &[dst, p]);
                }
            }
            CastKind::FunctionToPointerDecay => {
                let r = self.fn_lv(op);
                if r != dst {
                    self.emit(Op::Mov, 0, &[dst, r]);
                }
            }
            CastKind::IntegralCast => {
                let from = self.ety(op);
                let r = self.rv(op);
                self.convert(r, from, ty, dst, e);
                if let Some((min, max)) = enum_range(self.b.ast(), self.b.ast().exprs.get(e).ty()) {
                    let lo = |v: i64| v.cast_unsigned() as u32;
                    let hi = |v: i64| (v.cast_unsigned() >> 32) as u32;
                    // clang checks (and prints) the operand's value.
                    let (w, sg) = int_of(self.kind(from)).unwrap_or((64, true));
                    let aux = int_aux(w as u8, sg);
                    self.emit_at(Op::EnumCheck, aux, &[r, lo(min), hi(min), lo(max), hi(max)], e);
                }
            }
            CastKind::FloatingToIntegral => {
                // clang reports an out-of-range value at the operand.
                let from = self.ety(op);
                let r = self.rv(op);
                let at = self.sites_len();
                self.convert(r, from, ty, dst, e);
                let begin = self.begin(op);
                self.o.sites[at..].iter_mut().for_each(|s| s.loc = begin);
            }
            CastKind::IntegralToBoolean
            | CastKind::IntegralToFloating
            | CastKind::FloatingToBoolean
            | CastKind::FloatingCast
            | CastKind::PointerToBoolean => {
                let from = self.ety(op);
                let r = self.rv(op);
                self.convert(r, from, ty, dst, e);
            }
            CastKind::BitCast => {
                let from = self.ety(op);
                let (fk, tk) = (self.kind(from), self.kind(ty));
                let r = self.rv(op);
                match (fk, tk) {
                    (TyKind::Ptr(fp), TyKind::Ptr(tp)) => {
                        let fvoid = self.kind(fp) == TyKind::Void;
                        let tvoid = self.kind(tp) == TyKind::Void;
                        if tvoid || fp == tp {
                            self.emit(Op::Mov, 0, &[dst, r]);
                        } else if fvoid && ek != ExprKind::CXXReinterpretCastExpr {
                            self.emit_at(Op::VoidCast, 0, &[dst, r, tp.0], e);
                        } else {
                            self.trap(e, "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression");
                        }
                    }
                    _ => self.trap(e, "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"),
                }
            }
            CastKind::LValueToRValueBitCast => self.bit_cast(e, dst, None),
            CastKind::IntegralToPointer | CastKind::PointerToIntegral => {
                self.trap(e, "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression");
            }
            _ => self.unsupported(e),
        }
    }

    /// The function an lvalue of function type designates, as a function
    /// pointer.
    fn fn_lv(&mut self, e: ExprId) -> u32 {
        let e = skip_parens(self.ast(), e);
        let n = self.node(e);
        match n.kind() {
            ExprKind::DeclRefExpr => {
                let d = self.ast().exprs.decl(e);
                let f = self.o.func_index(self.b.ast(), d);
                let r = self.reg();
                self.konst(r, u128::from(fn_ptr(f)));
                r
            }
            ExprKind::UnaryOperator if self.ast().exprs.unary_op(e) == UnaryOp::Deref => {
                let x = self.ast().exprs.operand(e);
                self.rv(x)
            }
            _ => {
                self.unsupported(e);
                self.reg()
            }
        }
    }

    // ---- places -----------------------------------------------------------

    fn load(&mut self, place: Place, ty: OTy, akind: u32, e: ExprId, dst: u32) {
        match place {
            Place::Reg(r) => {
                if r != dst {
                    self.emit(Op::Mov, 0, &[dst, r]);
                }
            }
            Place::Mem(p) => {
                self.emit_at(Op::Load, akind, &[dst, p, ty.0], e);
            }
            Place::Bits { p, bit, width } => {
                self.emit_at(Op::LoadBits, akind, &[dst, p, ty.0, bit | width << 8], e);
            }
        }
    }

    fn store(&mut self, place: Place, src: u32, ty: OTy, akind: u32, e: ExprId) {
        match place {
            Place::Reg(r) => {
                if r != src {
                    self.emit(Op::Mov, 0, &[r, src]);
                }
            }
            Place::Mem(p) => {
                self.emit_at(Op::Store, akind, &[p, src, ty.0], e);
            }
            Place::Bits { p, bit, width } => {
                self.emit_at(Op::StoreBits, 0, &[p, src, ty.0, bit | width << 8], e);
            }
        }
    }

    /// A glvalue that must have an address.
    /// Evaluates the common expression of a GNU binary conditional once
    /// and binds it for the `OpaqueValueExpr`s naming it; returns the
    /// operands.
    fn bind_common(&mut self, e: ExprId, acc: Access) -> [ExprId; 4] {
        let ast = self.ast();
        let ops = ast.exprs.binary_conditional_operands(e, &ast.lists);
        let common = ops[0];
        let v = if self.node(common).value_category().is_glvalue() {
            Opaque::Lv(self.lv(common, acc))
        } else {
            Opaque::Rv(self.rv(common))
        };
        self.opaque.insert(common, v);
        ops
    }

    fn opaque_of(&self, e: ExprId) -> Option<Opaque> {
        let src = self.ast().exprs.opaque_source(e)?;
        self.opaque.get(&src).copied()
    }

    fn lv_mem(&mut self, e: ExprId, acc: Access) -> u32 {
        match self.lv(e, acc) {
            Place::Mem(p) => p,
            Place::Reg(_) | Place::Bits { .. } => {
                self.trap(e, "kiln: internal: the address of a register value was taken");
                self.reg()
            }
        }
    }

    fn lv(&mut self, e: ExprId, acc: Access) -> Place {
        let n = self.node(e);
        match n.kind() {
            ExprKind::ParenExpr => {
                let op = self.ast().exprs.operand(e);
                self.lv(op, acc)
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                let op = self.ast().exprs.rewritten_semantic(e);
                self.lv(op, acc)
            }
            ExprKind::SubstNonTypeTemplateParmExpr => {
                let op = self.ast().exprs.subst_replacement(e);
                self.lv(op, acc)
            }
            ExprKind::CXXDefaultArgExpr if default_arg_of(self.ast(), e).is_some() => {
                let d = default_arg_of(self.ast(), e).unwrap_or(e);
                let saved = self.enter_default_arg(e);
                let r = self.lv(d, acc);
                self.srcloc = saved;
                r
            }
            ExprKind::DeclRefExpr => {
                let ast = self.ast();
                let d = ast.exprs.decl(e);
                if let Some(l) = self.locals.get(&d).copied() {
                    return match l {
                        Local::Reg(r) => Place::Reg(r),
                        Local::Mem(p) => Place::Mem(p),
                    };
                }
                // A function parameter outside its call: an expression
                // evaluated in the body (a `static_assert`, an array bound)
                // has no frame for it. An automatic variable outside its
                // frame is read like a global: usable if it is `constexpr`
                // or a `const` integral with a constant initializer
                // ([expr.const]/4), else its value is unknown.
                if ast.decls.get(d).kind() == DeclKind::ParmVar {
                    let name = {
                        let mut s = String::new();
                        kiln_ast::dump::AstNames::new(ast).write_name(d, &mut s);
                        s
                    };
                    let dl = ast.decls.get(d).loc();
                    let loc = ast.exprs.get(e).loc();
                    self.trap_notes(
                        loc,
                        format!(
                            "function parameter '{name}' with unknown value cannot be used in a constant expression"
                        ),
                        vec![Note::new(dl, "declared here")],
                    );
                    return Place::Mem(self.reg());
                }
                if ast.decls.get(d).kind() != DeclKind::Var {
                    self.unsupported(e);
                    return Place::Mem(self.reg());
                }
                let g = self.o.global_index(self.b.ast(), d);
                let r = self.reg();
                self.emit_at(Op::GlobalAddr, 0, &[r, g], e);
                if is_reference_var(self.b.ast(), d) {
                    // The variable holds its referent's address.
                    let q = self.ast().decls.get(d).ty().unwrap_or(QualType::INT);
                    let rt = self.ty(q);
                    let a = self.reg();
                    self.emit_at(Op::Load, ak::READ, &[a, r, rt.0], e);
                    return Place::Mem(a);
                }
                Place::Mem(r)
            }
            ExprKind::StringLiteral => {
                let s = self.ast().exprs.string_lit(e);
                let ty = self.ety(e);
                let g = self.o.string_expr_index(e, s, ty);
                let r = self.reg();
                self.emit_at(Op::GlobalAddr, 0, &[r, g], e);
                Place::Mem(r)
            }
            ExprKind::MemberExpr => self.member(e, acc),
            ExprKind::ArraySubscriptExpr => {
                let ast = self.ast();
                let [a, b] = ast.exprs.operands(e);
                let a_is_ptr =
                    matches!(ast.types.canonical_view(ast.exprs.get(a).ty()), TypeView::Pointer(_));
                let (pe, ie) = if a_is_ptr { (a, b) } else { (b, a) };
                let ety = self.ety(e);
                let pe0 = skip_parens(self.ast(), pe);
                let pn = self.node(pe0);
                let decay = pn.kind() == ExprKind::ImplicitCastExpr
                    && self.ast().exprs.cast_kind(pe0) == CastKind::ArrayToPointerDecay;
                let (p, i) = if decay {
                    let inner = self.ast().exprs.operand(pe0);
                    let sub = if acc == Access::Addr { Access::Addr } else { acc };
                    if pe == a {
                        let p = self.lv_mem(inner, sub);
                        (p, self.rv(ie))
                    } else {
                        let i = self.rv(ie);
                        (self.lv_mem(inner, sub), i)
                    }
                } else if pe == a {
                    let p = self.rv(pe);
                    (p, self.rv(ie))
                } else {
                    let i = self.rv(ie);
                    (self.rv(pe), i)
                };
                let it = self.ety(ie);
                let (w, s) = int_of(self.kind(it)).unwrap_or((64, true));
                let r = self.reg();
                // clang's `ArraySubscriptExpr::getExprLoc`: the base's.
                let pc = self.emit_at(Op::PtrAdd, int_aux(w as u8, s), &[r, p, i, ety.0], e);
                let loc = self.ast().exprs.get(pe).loc();
                if let Some(site) = self.o.sites.last_mut().filter(|x| x.pc == pc as u32) {
                    site.loc = loc;
                }
                Place::Mem(r)
            }
            ExprKind::UnaryOperator => {
                let op = self.ast().exprs.unary_op(e);
                let x = self.ast().exprs.operand(e);
                match op {
                    UnaryOp::Deref => Place::Mem(self.rv(x)),
                    UnaryOp::PreInc | UnaryOp::PreDec => {
                        let place = self.lv_rmw(x, e);
                        let xty = self.ety(x);
                        self.inc_dec(place, xty, op, None, e);
                        place
                    }
                    _ => {
                        self.unsupported(e);
                        Place::Mem(self.reg())
                    }
                }
            }
            ExprKind::BinaryOperator => {
                let op = self.ast().exprs.binary_op(e);
                match op {
                    BinaryOp::Assign => self.assign(e),
                    BinaryOp::Comma => {
                        let [a, b] = self.ast().exprs.operands(e);
                        self.effect(a);
                        self.lv(b, acc)
                    }
                    _ if op.is_compound_assignment() => self.compound_assign(e),
                    _ if op.is_ptr_mem() => self.mp_access(e, acc),
                    _ => {
                        self.unsupported(e);
                        Place::Mem(self.reg())
                    }
                }
            }
            ExprKind::CompoundAssignOperator => self.compound_assign(e),
            ExprKind::OpaqueValueExpr => match self.opaque_of(e) {
                Some(Opaque::Lv(p)) => p,
                _ => {
                    self.unsupported(e);
                    Place::Mem(self.reg())
                }
            },
            ExprKind::BinaryConditionalOperator => {
                let [common, cond, t, f] = self.bind_common(e, acc);
                let c = self.rv(cond);
                let r = self.reg();
                let j = self.jmp_if(c, false);
                let pt = self.lv_mem(t, acc);
                self.emit(Op::Mov, 0, &[r, pt]);
                let end = self.jmp();
                self.patch_here(j);
                let pf = self.lv_mem(f, acc);
                self.emit(Op::Mov, 0, &[r, pf]);
                self.patch_here(end);
                self.opaque.remove(&common);
                Place::Mem(r)
            }
            ExprKind::ConditionalOperator => {
                let c = self.rv(ExprId::new(n.a()));
                let [t, f] = self.ast().exprs.cond_arms(e);
                let r = self.reg();
                let j = self.jmp_if(c, false);
                let pt = self.lv_mem(t, acc);
                self.emit(Op::Mov, 0, &[r, pt]);
                let end = self.jmp();
                self.patch_here(j);
                let pf = self.lv_mem(f, acc);
                self.emit(Op::Mov, 0, &[r, pf]);
                self.patch_here(end);
                Place::Mem(r)
            }
            ExprKind::ConstantExpr => {
                let op = self.ast().exprs.operand(e);
                self.lv(op, acc)
            }
            k if k.is_cast()
                && matches!(
                    self.ast().exprs.cast_kind(e),
                    CastKind::NoOp
                        | CastKind::UserDefinedConversion
                        | CastKind::ConstructorConversion
                ) =>
            {
                let op = self.ast().exprs.operand(e);
                self.lv(op, acc)
            }
            k if k.is_cast()
                && n.value_category().is_glvalue()
                && matches!(
                    self.ast().exprs.cast_kind(e),
                    CastKind::DerivedToBase
                        | CastKind::UncheckedDerivedToBase
                        | CastKind::BaseToDerived
                ) =>
            {
                let down = self.ast().exprs.cast_kind(e) == CastKind::BaseToDerived;
                let op = self.ast().exprs.operand(e);
                let sub = match acc {
                    Access::Write => Access::Write,
                    Access::Addr => Access::Addr,
                    Access::Read | Access::Rmw => Access::Read,
                };
                let p = self.lv_mem(op, sub);
                let r = self.reg();
                self.class_cast(e, p, r, false, down);
                Place::Mem(r)
            }
            // A call returning a reference: the referent's address.
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr
                if n.value_category().is_glvalue() =>
            {
                Place::Mem(self.call(e, None))
            }
            ExprKind::CXXMemberCallExpr if n.value_category().is_glvalue() => {
                Place::Mem(self.member_call(e, None))
            }
            ExprKind::MaterializeTemporaryExpr => self.mte(e),
            ExprKind::ExprWithCleanups => {
                let op = self.ast().exprs.operand(e);
                self.open_scope(&[], true);
                let p = self.lv(op, acc);
                self.close_scope();
                p
            }
            ExprKind::CXXDefaultInitExpr if self.default_init_of(e).is_some() => {
                let x = self.default_init_of(e).unwrap_or(e);
                self.lv(x, acc)
            }
            ExprKind::CXXThrowExpr => {
                self.throw_invalid(e);
                Place::Mem(self.reg())
            }
            // A class prvalue used as a glvalue: materialize it, destroyed
            // at the end of the full-expression.
            _ if !n.value_category().is_glvalue() && {
                let ty = self.ety(e);
                self.o.types.is_aggregate(ty)
            } =>
            {
                let ty = self.ety(e);
                Place::Mem(self.materialize(e, ty))
            }
            _ => {
                self.unsupported(e);
                Place::Mem(self.reg())
            }
        }
    }

    fn member(&mut self, e: ExprId, acc: Access) -> Place {
        let ast = self.ast();
        let n = *ast.exprs.get(e);
        let base = ExprId::new(n.a());
        let fd = ast.exprs.decl(e);
        let arrow = n.has(kiln_ast::expr::ExprFlags::ARROW);
        let bq = ast.exprs.get(base).ty();
        let bt = self.ty(bq);
        let rt = if arrow {
            match self.kind(bt) {
                TyKind::Ptr(p) => p,
                _ => bt,
            }
        } else {
            bt
        };
        let (union, idx) = match self.kind(rt) {
            TyKind::Record { union, .. } => {
                (union, self.o.types.fields(rt).iter().position(|f| f.decl == fd))
            }
            _ => (false, None),
        };
        let Some(idx) = idx else {
            self.unsupported(e);
            return Place::Mem(self.reg());
        };
        let fi = self.o.types.fields(rt)[idx];
        let p = if arrow {
            self.rv(base)
        } else {
            let sub = match acc {
                Access::Write => Access::Write,
                Access::Addr => Access::Addr,
                Access::Read | Access::Rmw => Access::Read,
            };
            self.lv_mem(base, sub)
        };
        // `p->m` checks its operand for null even at offset 0 (clang:
        // "cannot access field of null pointer"); `.` on an lvalue cannot
        // see a null base (a later read says "dereferenced null pointer").
        let q = if fi.offset == 0 && !arrow {
            p
        } else {
            let r = self.reg();
            self.emit_at(Op::Field, u32::from(!arrow), &[r, p, fi.offset as u32], e);
            r
        };
        if union {
            match acc {
                Access::Write => {
                    self.emit_at(Op::SetActive, 0, &[p, idx as u32, rt.0], e);
                }
                Access::Read | Access::Rmw => {
                    let (akind, loc) = match (acc, self.rmw_loc) {
                        (Access::Rmw, Some(l)) => (ak::ASSIGN, l),
                        (Access::Rmw, None) => (ak::ASSIGN, self.node(e).loc()),
                        _ => (ak::READ, self.begin(e)),
                    };
                    self.emit_site(Op::ActiveCheck, akind, &[p, idx as u32, rt.0], e, loc);
                }
                Access::Addr => {}
            }
        }
        if is_reference_decl(self.ast(), fd) {
            // A reference member holds its referent's address.
            let a = self.reg();
            self.emit_at(Op::Load, ak::READ, &[a, q, fi.ty.0], e);
            return Place::Mem(a);
        }
        match fi.width {
            Some(w) => Place::Bits { p: q, bit: fi.bit, width: w },
            None => Place::Mem(q),
        }
    }

    /// `a = b` (scalar or trivially copyable aggregate); returns the place.
    fn assign(&mut self, e: ExprId) -> Place {
        let [a, b] = self.ast().exprs.operands(e);
        let ty = self.ety(a);
        if self.o.types.is_aggregate(ty) {
            let src = self.lv_mem(b, Access::Read);
            let dstp = self.lv_mem(a, Access::Write);
            self.emit_at(Op::Copy, 0, &[dstp, src, ty.0], e);
            return Place::Mem(dstp);
        }
        // [expr.assign]/1: the right operand is sequenced first.
        let r = self.rv_copy(b);
        let saved = self.rmw_loc.replace(self.node(e).loc());
        let place = self.lv(a, Access::Write);
        self.rmw_loc = saved;
        self.store(place, r, ty, ak::ASSIGN, e);
        place
    }

    fn compound_assign(&mut self, e: ExprId) -> Place {
        let ast = self.ast();
        let n = *ast.exprs.get(e);
        let x = *ast.exprs.compound_assign(e);
        let op = ast.exprs.binary_op(e);
        let lhs = ExprId::new(n.a());
        let r = self.rv_copy(x.rhs);
        let place = self.lv_rmw(lhs, e);
        let ty = self.ety(lhs);
        let rty = self.ety(x.rhs);
        let old = self.reg();
        self.load(place, ty, ak::COMPOUND, e, old);
        let new = self.reg();
        if let TyKind::Ptr(pointee) = self.kind(ty) {
            let (w, s) = int_of(self.kind(rty)).unwrap_or((64, true));
            let aux = int_aux(w as u8, s) | u32::from(op == BinaryOp::SubAssign) << 9;
            self.emit_at(Op::PtrAdd, aux, &[new, old, r, pointee.0], e);
        } else {
            let lt = self.ty(x.lhs_ty);
            let ct = self.ty(x.result_ty);
            let Some(iop) = int_op(op) else {
                self.unsupported(e);
                return place;
            };
            let shift = matches!(iop, Op::IShl | Op::IShr);
            let a = self.reg();
            self.convert(old, ty, lt, a, e);
            let b = if shift {
                r
            } else {
                let b = self.reg();
                self.convert(r, rty, ct, b, e);
                b
            };
            let a2 = if shift || lt == ct {
                a
            } else {
                let a2 = self.reg();
                self.convert(a, lt, ct, a2, e);
                a2
            };
            let res = self.reg();
            self.arith(iop, ct, rty, res, a2, b, e);
            self.convert(res, ct, ty, new, e);
        }
        self.store(place, new, ty, ak::COMPOUND, e);
        place
    }

    // ---- initialization ---------------------------------------------------

    /// Initializes the object of type `ty` at `p` from `e`.
    fn init(&mut self, e: ExprId, p: u32, ty: OTy) {
        let m = self.next.max(p + 1);
        self.next = m;
        self.init_inner(e, p, ty);
        self.next = m;
    }

    fn init_inner(&mut self, e: ExprId, p: u32, ty: OTy) {
        let n = self.node(e);
        match n.kind() {
            ExprKind::ParenExpr | ExprKind::ConstantExpr => {
                let op = self.ast().exprs.operand(e);
                return self.init_inner(op, p, ty);
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                let op = self.ast().exprs.rewritten_semantic(e);
                return self.init_inner(op, p, ty);
            }
            ExprKind::CXXDefaultArgExpr if default_arg_of(self.ast(), e).is_some() => {
                let d = default_arg_of(self.ast(), e).unwrap_or(e);
                let saved = self.enter_default_arg(e);
                self.init_inner(d, p, ty);
                self.srcloc = saved;
                return;
            }
            ExprKind::ImplicitValueInitExpr => {
                self.emit_at(Op::ZeroInit, 0, &[p, ty.0], e);
                return;
            }
            // Default-initialization by a trivial constructor performs no
            // initialization ([dcl.init.general]/7.1): the object stays
            // indeterminate.
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                self.construct(e, p, ty);
                return;
            }
            ExprKind::CXXInheritedCtorInitExpr => {
                self.inherited_ctor(e, p);
                return;
            }
            ExprKind::ExprWithCleanups => {
                let op = self.ast().exprs.operand(e);
                self.open_scope(&[], true);
                self.init_inner(op, p, ty);
                self.close_scope();
                return;
            }
            ExprKind::CXXBindTemporaryExpr => {
                let op = self.ast().exprs.operand(e);
                return self.init_inner(op, p, ty);
            }
            ExprKind::CXXDefaultInitExpr if self.default_init_of(e).is_some() => {
                let x = self.default_init_of(e).unwrap_or(e);
                return self.init_inner(x, p, ty);
            }
            ExprKind::ArrayInitLoopExpr => {
                self.array_init_loop(e, p, ty);
                return;
            }
            ExprKind::CXXParenListInitExpr => {
                let ast = self.ast();
                let inits: Vec<ExprId> =
                    ast.lists.iter::<ExprId>(ast.exprs.paren_list_inits(e)).collect();
                let filler = ast.exprs.paren_list_filler(e);
                self.init_aggr(e, &inits, filler, None, p, ty);
                return;
            }
            _ => {}
        }
        if n.kind().is_cast() && self.ast().exprs.cast_kind(e) == CastKind::LValueToRValueBitCast {
            // The result may hold indeterminate bytes ([bit.cast]/2).
            let r = self.reg();
            self.bit_cast(e, r, Some(p));
            return;
        }
        if !self.o.types.is_aggregate(ty) {
            let r = self.rv(e);
            self.emit_at(Op::InitStore, 0, &[p, r, ty.0], e);
            return;
        }
        match n.kind() {
            ExprKind::InitListExpr => self.init_list(e, p, ty),
            ExprKind::StringLiteral => {
                let sty = self.ety(e);
                let s = self.ast().exprs.string_lit(e);
                let g = self.o.string_expr_index(e, s, sty);
                let r = self.reg();
                self.emit_at(Op::GlobalAddr, 0, &[r, g], e);
                if self.o.types.size(sty) < self.o.types.size(ty) {
                    self.emit_at(Op::ZeroInit, 0, &[p, ty.0], e);
                }
                let cty = if self.o.types.size(sty) <= self.o.types.size(ty) { sty } else { ty };
                self.emit_at(Op::Copy, 1, &[p, r, cty.0], e);
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr
                if !n.value_category().is_glvalue() =>
            {
                self.call(e, Some(p));
            }
            ExprKind::CXXMemberCallExpr if !n.value_category().is_glvalue() => {
                self.member_call(e, Some(p));
            }
            ExprKind::ConditionalOperator => {
                let c = self.rv(ExprId::new(n.a()));
                let [t, f] = self.ast().exprs.cond_arms(e);
                let j = self.jmp_if(c, false);
                self.init(t, p, ty);
                let end = self.jmp();
                self.patch_here(j);
                self.init(f, p, ty);
                self.patch_here(end);
            }
            ExprKind::BinaryOperator if self.ast().exprs.binary_op(e) == BinaryOp::Comma => {
                let [a, b] = self.ast().exprs.operands(e);
                self.effect(a);
                self.init_inner(b, p, ty);
            }
            k if k.is_cast()
                && matches!(
                    self.ast().exprs.cast_kind(e),
                    CastKind::NoOp
                        | CastKind::LValueToRValue
                        | CastKind::UserDefinedConversion
                        | CastKind::ConstructorConversion
                ) =>
            {
                let op = self.ast().exprs.operand(e);
                if self.node(op).value_category().is_glvalue() {
                    let src = self.lv_mem(op, Access::Read);
                    self.emit_at(Op::Copy, 1, &[p, src, ty.0], e);
                } else {
                    self.init_inner(op, p, ty);
                }
            }
            _ if n.value_category().is_glvalue() => {
                let src = self.lv_mem(e, Access::Read);
                self.emit_at(Op::Copy, 1, &[p, src, ty.0], e);
            }
            _ => self.unsupported(e),
        }
    }

    fn init_list(&mut self, e: ExprId, p: u32, ty: OTy) {
        let ast = self.ast();
        let inits: Vec<ExprId> =
            ast.lists.get(ast.exprs.init_list_inits(e)).iter().map(|&x| ExprId::new(x)).collect();
        let extra = *ast.exprs.init_list(e);
        self.init_aggr(e, &inits, extra.array_filler, extra.union_field, p, ty);
    }

    /// Aggregate initialization ([dcl.init.aggr]) of the object of type
    /// `ty` at `p` from the elements `inits` of list (or parenthesized
    /// list) `e`; `filler` initializes the remaining array elements,
    /// `union_field` names the union member initialized.
    fn init_aggr(
        &mut self,
        e: ExprId,
        inits: &[ExprId],
        filler: Option<ExprId>,
        union_field: Option<DeclId>,
        p: u32,
        ty: OTy,
    ) {
        match self.kind(ty) {
            TyKind::Array { elem, n } => {
                if inits.len() as u64 > n {
                    self.unsupported(e);
                    return;
                }
                // A list of one string literal initializing a char array.
                if inits.len() == 1
                    && self.node(skip_parens(self.ast(), inits[0])).kind()
                        == ExprKind::StringLiteral
                {
                    let s = skip_parens(self.ast(), inits[0]);
                    self.init_inner(s, p, ty);
                    return;
                }
                let rest = n - inits.len() as u64;
                let zero_fill = rest > 0
                    && filler
                        .is_none_or(|f| self.node(f).kind() == ExprKind::ImplicitValueInitExpr);
                if zero_fill {
                    self.emit_at(Op::ZeroInit, 0, &[p, ty.0], e);
                }
                let es = self.o.types.size(elem);
                for (i, &x) in inits.iter().enumerate() {
                    let q = self.elem_ptr(p, i as u64 * es, e);
                    self.init(x, q, elem);
                }
                if rest > 0
                    && !zero_fill
                    && let Some(f) = filler
                {
                    // A loop over the remaining elements.
                    let k = self.reg();
                    let one = self.reg();
                    let lim = self.reg();
                    let c = self.reg();
                    self.konst(k, inits.len() as u128);
                    self.konst(one, 1);
                    self.konst(lim, u128::from(n));
                    let top = self.here();
                    self.emit(Op::ICmp, int_aux(64, false) | cmp::LT << 9, &[c, k, lim]);
                    let exit = self.jmp_if(c, false);
                    let q = self.reg();
                    self.emit_at(Op::PtrAdd, int_aux(64, false), &[q, p, k, elem.0], e);
                    self.init(f, q, elem);
                    self.emit(Op::IAdd, int_aux(64, false), &[k, k, one]);
                    let j = self.jmp();
                    self.patch(j, top);
                    self.patch_here(exit);
                }
            }
            TyKind::Record { union: true, .. } => {
                let fields = self.o.types.fields(ty).to_vec();
                let target = union_field.or_else(|| fields.first().map(|f| f.decl));
                let Some(idx) = target.and_then(|d| fields.iter().position(|f| f.decl == d)) else {
                    return;
                };
                self.emit_at(Op::SetActive, 1, &[p, idx as u32, ty.0], e);
                let fi = fields[idx];
                if let Some(&x) = inits.first() {
                    self.init_field(x, p, fi, e);
                } else {
                    let q = self.elem_ptr(p, fi.offset, e);
                    self.emit_at(Op::ZeroInit, 0, &[q, fi.ty.0], e);
                }
            }
            TyKind::Record { decl, .. } => {
                // The bases first, in declaration order ([dcl.init.aggr]/2).
                let ast = self.ast();
                let bases: Vec<QualType> =
                    ast.decls.bases(decl, &ast.lists).map(|b| b.ty).collect();
                let mut it = inits.iter();
                for bq in bases {
                    let bty = self.ty(bq);
                    let off = self.base_offset(ty, bty);
                    let m = self.next;
                    let q = self.elem_ptr(p, off, e);
                    match it.next() {
                        Some(&x) => self.init(x, q, bty),
                        None => {
                            self.emit_at(Op::ZeroInit, 0, &[q, bty.0], e);
                        }
                    }
                    self.next = m;
                }
                let fields = self.o.types.fields(ty).to_vec();
                for fi in fields {
                    let unnamed_bf = fi.width.is_some() && !self.has_name(fi.decl);
                    if unnamed_bf || fi.base {
                        continue;
                    }
                    // A default member initializer's `this` is the object
                    // being initialized ([class.mem.general]/11).
                    let x = match it.next() {
                        Some(&x) => Some((x, self.is_default_init(x))),
                        None => {
                            self.ast().decls.field(fi.decl).and_then(|f| f.init).map(|d| (d, true))
                        }
                    };
                    if let Some((x, dflt)) = x {
                        let saved = self.this_override;
                        if dflt {
                            self.this_override = Some(p);
                        }
                        self.init_field(x, p, fi, e);
                        self.this_override = saved;
                    } else {
                        // [dcl.init.aggr]/5.3: copy-initialized from `{}`.
                        let q = self.elem_ptr(p, fi.offset, e);
                        match fi.width {
                            Some(w) => {
                                let z = self.reg();
                                self.konst(z, 0);
                                self.emit_at(
                                    Op::StoreBits,
                                    1,
                                    &[q, z, fi.ty.0, fi.bit | w << 8],
                                    e,
                                );
                            }
                            None => {
                                self.emit_at(Op::ZeroInit, 0, &[q, fi.ty.0], e);
                            }
                        }
                    }
                }
            }
            _ => self.unsupported(e),
        }
    }

    fn has_name(&self, d: DeclId) -> bool {
        let mut s = String::new();
        kiln_ast::dump::AstNames::new(self.ast()).write_name(d, &mut s);
        !s.is_empty()
    }

    /// `p + off` (bytes).
    fn elem_ptr(&mut self, p: u32, off: u64, e: ExprId) -> u32 {
        if off == 0 {
            return p;
        }
        let r = self.reg();
        self.emit_at(Op::Field, 0, &[r, p, off as u32], e);
        r
    }

    fn init_field(&mut self, x: ExprId, p: u32, fi: crate::ty::FieldInfo, e: ExprId) {
        let q = self.elem_ptr(p, fi.offset, e);
        if is_reference_decl(self.ast(), fi.decl) {
            let r = self.reg();
            self.bind_ref(x, r);
            self.emit_at(Op::InitStore, 0, &[q, r, fi.ty.0], x);
            return;
        }
        match fi.width {
            Some(w) => {
                let r = self.rv(x);
                self.emit_at(Op::StoreBits, 1, &[q, r, fi.ty.0, fi.bit | w << 8], x);
            }
            None => self.init(x, q, fi.ty),
        }
    }

    // ---- effects and calls ------------------------------------------------

    /// Evaluates `e` for its side effects.
    fn effect(&mut self, e: ExprId) {
        let m = self.next;
        let n = self.node(e);
        let ty = self.ety(e);
        match n.kind() {
            ExprKind::CXXThrowExpr => self.throw_invalid(e),
            ExprKind::StmtExpr => self.stmt_expr(e, None),
            ExprKind::ParenExpr => {
                let op = self.ast().exprs.operand(e);
                self.effect(op);
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                let op = self.ast().exprs.rewritten_semantic(e);
                self.effect(op);
            }
            ExprKind::SubstNonTypeTemplateParmExpr => {
                let op = self.ast().exprs.subst_replacement(e);
                self.effect(op);
            }
            ExprKind::CompoundLiteralExpr => {
                let init = self.ast().exprs.compound_literal_init(e);
                self.effect(init);
            }
            ExprKind::CXXDefaultArgExpr if default_arg_of(self.ast(), e).is_some() => {
                let d = default_arg_of(self.ast(), e).unwrap_or(e);
                let saved = self.enter_default_arg(e);
                self.effect(d);
                self.srcloc = saved;
            }
            ExprKind::BinaryOperator => {
                let op = self.ast().exprs.binary_op(e);
                match op {
                    BinaryOp::Comma => {
                        // R-WALK: a comma fold of any shape, in order.
                        let mut work = vec![e];
                        while let Some(x) = work.pop() {
                            let ast = self.ast();
                            let x = skip_parens(ast, x);
                            if is_binary(ast, x, BinaryOp::Comma) {
                                let [a, b] = ast.exprs.operands(x);
                                work.push(b);
                                work.push(a);
                            } else {
                                self.effect(x);
                            }
                        }
                    }
                    BinaryOp::Assign => {
                        self.assign(e);
                    }
                    _ if op.is_compound_assignment() => {
                        self.compound_assign(e);
                    }
                    _ => {
                        let r = self.reg();
                        self.rv_into(e, r);
                    }
                }
            }
            ExprKind::CompoundAssignOperator => {
                self.compound_assign(e);
            }
            ExprKind::UnaryOperator if self.ast().exprs.unary_op(e).is_inc_dec() => {
                let op = self.ast().exprs.unary_op(e);
                let x = self.ast().exprs.operand(e);
                let place = self.lv_rmw(x, e);
                let xty = self.ety(x);
                self.inc_dec(place, xty, op, None, e);
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr
                if !n.value_category().is_glvalue() =>
            {
                if self.o.types.is_aggregate(ty) {
                    // A discarded class prvalue: a temporary
                    // ([expr.context]/2), destroyed at the end of the
                    // full-expression.
                    let _ = self.materialize(e, ty);
                } else if n.kind() != ExprKind::CXXMemberCallExpr {
                    self.call(e, None);
                } else {
                    self.member_call(e, None);
                }
            }
            ExprKind::ExprWithCleanups => {
                let op = self.ast().exprs.operand(e);
                self.open_scope(&[], true);
                self.effect(op);
                self.close_scope();
            }
            ExprKind::ConditionalOperator => {
                let c = self.rv(ExprId::new(n.a()));
                let [t, f] = self.ast().exprs.cond_arms(e);
                let j = self.jmp_if(c, false);
                self.effect(t);
                let end = self.jmp();
                self.patch_here(j);
                self.effect(f);
                self.patch_here(end);
            }
            k if k.is_cast() && self.ast().exprs.cast_kind(e) == CastKind::ToVoid => {
                let op = self.ast().exprs.operand(e);
                self.effect(op);
            }
            _ if n.value_category().is_glvalue() => {
                // A discarded glvalue is evaluated, not read.
                let _ = self.lv(e, Access::Addr);
            }
            ExprKind::CXXDeleteExpr => self.delete_expr(e),
            _ if self.kind(ty) == TyKind::Void => self.unsupported(e),
            _ if self.o.types.is_aggregate(ty) => {
                let _ = self.materialize(e, ty);
            }
            _ => {
                let r = self.reg();
                self.rv_into(e, r);
            }
        }
        self.next = m;
    }

    /// Compiles call `e`; the scalar result is in the returned register.
    /// `sret`: the result object's address (aggregate results).
    /// Binds a reference to glvalue `init`: its address into `dst`.
    fn bind_ref(&mut self, init: ExprId, dst: u32) {
        let m = self.next.max(dst + 1);
        self.next = m;
        if self.node(init).value_category().is_glvalue() {
            let p = self.lv_mem(init, Access::Addr);
            if p != dst {
                self.emit(Op::Mov, 0, &[dst, p]);
            }
        } else {
            // A prvalue sema did not wrap in a `MaterializeTemporaryExpr`:
            // a temporary of the full-expression.
            let ty = self.ety(init);
            let p = if self.o.types.is_aggregate(ty) {
                self.materialize(init, ty)
            } else {
                let r = self.reg();
                self.alloc(r, ty, Origin::Temp(init), false, init);
                self.init(init, r, ty);
                r
            };
            self.emit(Op::Mov, 0, &[dst, p]);
        }
        self.next = m;
    }

    /// Which parameters of the function `callee` designates are references.
    fn ref_params(&self, callee: ExprId) -> Vec<bool> {
        let ast = self.ast();
        let mut q = ast.exprs.get(callee).ty();
        if let Some(p) = ast.types.pointee(q) {
            q = p;
        }
        ast.types
            .proto(q)
            .map(|p| ast.types.params(p).iter().map(|&t| ast.types.is_reference(t)).collect())
            .unwrap_or_default()
    }

    fn call(&mut self, e: ExprId, sret: Option<u32>) -> u32 {
        let ty = self.ety(e);
        let ast = self.ast();
        let callee = ast.exprs.callee(e);
        let args: Vec<ExprId> =
            ast.lists.get(ast.exprs.call_args(e)).iter().map(|&x| ExprId::new(x)).collect();
        // The direct callee, if any.
        let mut c = skip_parens(ast, callee);
        while ast.exprs.get(c).kind().is_cast()
            && matches!(
                ast.exprs.cast_kind(c),
                CastKind::FunctionToPointerDecay | CastKind::BuiltinFnToFnPtr | CastKind::NoOp
            )
        {
            c = skip_parens(ast, ast.exprs.operand(c));
        }
        let direct = (ast.exprs.get(c).kind() == ExprKind::DeclRefExpr
            && ast.decls.get(ast.exprs.decl(c)).kind().is_function())
        .then(|| ast.exprs.decl(c));
        if let Some(d) = direct
            && ast.decls.builtin_id(d) == Some(kiln_ast::decl::builtin::IS_CONSTANT_EVALUATED)
        {
            let r = self.reg();
            let pc = self.emit_at(Op::IsConstEval, u32::from(self.std_ice), &[r], e);
            let loc = self.begin(e);
            if let Some(site) = self.o.sites.last_mut().filter(|x| x.pc == pc as u32) {
                site.loc = loc;
            }
            self.uses_cc = true;
            return r;
        }
        // `__builtin_addressof(x)`: the address of the lvalue `x`, as
        // `&x` without a user `operator&` ([specialized.addressof]).
        if let Some(d) = direct
            && ast.decls.builtin_id(d) == Some(kiln_ast::decl::builtin::ADDRESSOF)
            && let [x] = args[..]
        {
            let x0 = skip_parens(ast, x);
            let xty = self.ety(x0);
            if self.kind(xty) == TyKind::Func {
                return self.fn_lv(x0);
            }
            return self.lv_mem(x, Access::Addr);
        }
        // Library builtins (spec/oven.md "Builtins").
        if let Some(d) = direct
            && let Some(bid) = ast.decls.builtin_id(d)
            && let Some(r) = self.builtin_call(e, bid, &args)
        {
            return r;
        }
        // `std::allocator<T>::allocate`/`deallocate` (spec/oven.md
        // "Transient allocation": deferred to the templates wave, so this
        // never holds yet).
        if let Some(d) = direct
            && new_delete::is_std_allocator_call(self.ast(), d)
        {
            self.trap(
                e,
                "kiln: std::allocator in constant evaluation lands with the templates wave",
            );
            return self.reg();
        }
        // An operator call whose callee is a member function: the first
        // argument is the object ([over.match.oper]/2, clang's
        // `EmitCXXOperatorMemberCallExpr`).
        let ast = self.ast();
        if let Some(md) = direct
            && ast.exprs.get(e).kind() == ExprKind::CXXOperatorCallExpr
            && ast.decls.get(md).kind() == DeclKind::CXXMethod
            && let Some((&obj, rest)) = args.split_first()
        {
            return self.method_call(e, c, md, obj, false, false, rest, sret);
        }
        // `obj.f()` naming a static member function: `obj` is evaluated,
        // then `f` called directly ([expr.ref]/7.1).
        let ast = self.ast();
        let direct = match direct {
            None if ast.exprs.get(c).kind() == ExprKind::MemberExpr
                && ast.decls.get(ast.exprs.decl(c)).kind().is_function() =>
            {
                let d = ast.exprs.decl(c);
                let obj = ast.exprs.operand(c);
                self.effect(obj);
                Some(d)
            }
            d => d,
        };
        let target = match direct {
            Some(d) => class::Callee::Direct(d),
            None => class::Callee::Ind(self.rv_copy(callee)),
        };
        let refs = self.ref_params(callee);
        // clang's `CallExpr::getExprLoc`: the begin of the callee (kiln's
        // `loc` of a call is its `(`).
        let loc = matches!(
            self.ast().exprs.get(e).kind(),
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr
        )
        .then(|| self.begin(e));
        self.call_frame(e, target, ty, sret, None, &args, &refs, loc)
    }
}

/// Whether function `d` is `std::is_constant_evaluated` (clang's
/// `isInStdNamespace`: a function of that name in namespace `std` at
/// translation-unit scope, possibly through inline namespaces).
fn is_std_is_constant_evaluated(ast: &kiln_ast::Ast, d: DeclId) -> bool {
    let decls = &ast.decls;
    let named = |d: DeclId, n: &str| {
        decls.get(d).name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == n)
    };
    if !named(d, "is_constant_evaluated") {
        return false;
    }
    let mut ns = decls.get(d).ctx();
    while let Some(n) = ns
        && decls.namespace(n).is_some_and(kiln_ast::decl::NamespaceData::is_inline)
    {
        ns = decls.get(n).ctx();
    }
    ns.is_some_and(|n| {
        decls.get(n).kind() == DeclKind::Namespace
            && named(n, "std")
            && decls
                .get(n)
                .ctx()
                .is_some_and(|tu| decls.get(tu).kind() == DeclKind::TranslationUnit)
    })
}

/// Whether `d` is a variable of reference type.
fn is_reference_var(ast: &kiln_ast::Ast, d: DeclId) -> bool {
    is_reference_decl(ast, d)
}

/// Whether declaration `d` (a variable, parameter or field) has reference
/// type.
fn is_reference_decl(ast: &kiln_ast::Ast, d: DeclId) -> bool {
    ast.decls.get(d).ty().is_some_and(|t| ast.types.is_reference(t))
}

/// The expression a `CXXDefaultArgExpr` stands for: its parameter's
/// default argument, evaluated afresh at each call ([dcl.fct.default]/9).
fn default_arg_of(ast: &kiln_ast::Ast, e: ExprId) -> Option<ExprId> {
    let p = ast.exprs.default_arg_param(e);
    ast.decls.param(p).and_then(|x| x.default_arg)
}

impl Compiler<'_> {
    /// Enters default argument `e`: the source-location builtins in it
    /// take the values of this use unless an enclosing default argument
    /// already set them. Returns the scope to restore.
    fn enter_default_arg(&mut self, e: ExprId) -> Option<ExprId> {
        let saved = self.srcloc;
        if saved.is_none() && self.ast().exprs.source_loc_values(e).is_some() {
            self.srcloc = Some(e);
        }
        saved
    }

    /// `__builtin_LINE()` and the other source-location builtins
    /// ([support.srcloc.cons]): the values sema recorded for the
    /// expression, or for the default argument use enclosing it.
    fn source_loc(&mut self, e: ExprId, dst: u32) {
        use kiln_ast::expr::SourceLocKind as K;
        let ast = self.ast();
        let kind = ast.exprs.source_loc_kind(e);
        let key = self.srcloc.unwrap_or(e);
        let Some(v) = ast.exprs.source_loc_values(key).copied() else {
            return self.unsupported(e);
        };
        let cv = match kind {
            K::Line => return self.konst(dst, u128::from(v.line)),
            K::Column => return self.konst(dst, u128::from(v.column)),
            K::File => v.file,
            K::FileName => v.file_name,
            K::Function => v.function,
            K::SourceLocStruct => match v.location {
                Some(c) => c,
                None => return self.unsupported(e),
            },
        };
        let ty = self.ety(e);
        let raw = kiln_ast::RawId::raw(cv);
        let loc = self.begin(e);
        self.emit_site(
            Op::Builtin,
            crate::kbc::bi::THAW_PTR,
            &[dst, dst, dst, dst, raw, ty.0],
            e,
            loc,
        );
    }
}
