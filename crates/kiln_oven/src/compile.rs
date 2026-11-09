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
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, StorageDuration, TraitKind, UnaryOp};
use kiln_ast::lambda::ClosureThunk;
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::types::{TypeKind, TypeView};
use kiln_ast::{DeclId, ExprId, QualType, StmtId, StringLitId};
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::failure::Note;
use crate::kbc::{Op, ak, cce, cmp, int_aux, ptr_add_aux};
use crate::mem::{Origin, flags, fn_ptr};
use crate::ty::{OTy, Scal, TyKind, eval_int};
use crate::{Oven, SemaBridge};

mod bitcast;
mod builtins;
mod class;
mod complex;
mod except;
mod lambda;
mod mptr;
mod new_delete;
mod rtti;
mod vector;
mod wide;

pub(crate) use builtins::designator_type;
pub(crate) use new_delete::{is_named, is_std_named};

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
    /// An invalid declaration (its template instantiation failed): a
    /// call is "subexpression not valid" (clang `isInvalidDecl`).
    Invalid,
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
    /// The base-object variant of a constructor or destructor of a class
    /// with virtual bases, which leaves them alone (spec/oven.md
    /// "Virtual bases"; Itanium C2/D2).
    pub base_obj: bool,
    /// A constructor or destructor: the object its `this` designates is
    /// not volatile while it runs ([class.ctor.general]/5,
    /// [class.dtor]/5; `Oven::volatile_path`).
    pub structor: bool,
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
    /// The result object of the evaluation of this variable's own
    /// initializer (`Op::BindObject`, WP-1100): the object under
    /// construction its name designates while the initializer runs
    /// ([basic.life]/1, clang's `EvaluatingDecl`). Naming it makes the
    /// running calls impure (not memoized): its value changes as it is
    /// initialized.
    Bound(u64),
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
    Bits {
        p: u32,
        bit: u32,
        width: u32,
    },
    /// Lane `idx` (a register) of the `bool` vector of type `vty` at `p`
    /// (`TyKind::BoolVec`: a bit, so no address).
    Lane {
        p: u32,
        idx: u32,
        vty: OTy,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Access {
    Read,
    Write,
    Rmw,
    Addr,
}

/// The access to the vector object of a lane access `v[i]` / `v.x` made
/// as `acc` (clang 22): writing a lane modifies the vector without
/// assigning it, so a union member holding it is checked to be active
/// rather than made active ([class.union.general]/6 names only member
/// access and array subscript; clang `StartLifetimeOfUnionMemberHandler`
/// stops at a vector).
fn lane_base_access(acc: Access) -> Access {
    if acc == Access::Write { Access::Rmw } else { acc }
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
        && ![e, ast.exprs.operands(e)[0], ast.exprs.operands(e)[1]].iter().any(|&x| {
            let q = ast.exprs.get(x).ty();
            ast.types.as_complex(q).is_some()
                || matches!(ast.types.canonical_view(q), TypeView::BitInt { width, .. } if width > 128)
        })
}

pub(crate) fn skip_parens(ast: &kiln_ast::Ast, mut e: ExprId) -> ExprId {
    loop {
        match ast.exprs.get(e).kind() {
            ExprKind::ParenExpr => e = ast.exprs.operand(e),
            // GNU `__extension__` is transparent (clang `IgnoreParens`).
            ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::Extension => {
                e = ast.exprs.operand(e);
            }
            ExprKind::ChooseExpr | ExprKind::GenericSelectionExpr
                if ast.exprs.selected_operand(e).is_some() =>
            {
                e = ast.exprs.selected_operand(e).expect("guarded");
            }
            ExprKind::SubstNonTypeTemplateParmExpr => e = ast.exprs.subst_replacement(e),
            // [over.match.oper]/8: the rewritten form is its semantic
            // expression.
            ExprKind::CXXRewrittenBinaryOperator => e = ast.exprs.rewritten_semantic(e),
            _ => break,
        }
    }
    e
}

/// The holding variables of decomposition `d`'s tuple-like bindings
/// ([dcl.struct.bind]/4), in order; empty for any other declaration.
pub(crate) fn holding_vars(ast: &kiln_ast::Ast, d: DeclId) -> Vec<DeclId> {
    ast.decls.decomp_bindings(d).map_or_else(Vec::new, |l| {
        ast.lists.iter::<DeclId>(l).filter_map(|b| ast.decls.binding_var(b)).collect()
    })
}

/// Whether `q` is a literal type ([basic.types.general]/10), as sema
/// records it for each class (`DefinitionBits::LITERAL`).
fn is_literal_type(ast: &kiln_ast::Ast, q: QualType) -> bool {
    let mut t = q;
    while let Some(e) = ast.types.element(t) {
        t = e;
    }
    let TypeView::Record(r) = ast.types.canonical_view(t) else { return true };
    ast.definition(r).and_then(|d| ast.decls.record(d)).is_none_or(|x| {
        !x.defn.contains(kiln_ast::decl::DefinitionBits::COMPLETE)
            || x.defn.contains(kiln_ast::decl::DefinitionBits::LITERAL)
    })
}

/// Whether `d` is a variable with automatic storage duration.
pub(crate) fn is_local_var(ast: &kiln_ast::Ast, d: DeclId) -> bool {
    let n = ast.decls.get(d);
    match n.kind() {
        DeclKind::ParmVar => true,
        DeclKind::Var => {
            let Some(v) = ast.decls.var(d) else { return false };
            matches!(v.spec.storage(), VarStorage::None | VarStorage::Register)
                && v.spec.tls() == kiln_ast::decl::TlsKind::None
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
            base_obj: false,
            structor: false,
        });
        self.func_map.insert(d, i);
        i
    }

    /// The function index of the base-object variant of constructor or
    /// destructor `d` of a class with virtual bases ([class.base.init]/7:
    /// only the most derived class's constructor initializes them;
    /// [class.dtor]/13 likewise destroys them).
    pub(crate) fn func_index_base(&mut self, ast: &kiln_ast::Ast, d: DeclId) -> u32 {
        let d = ast.first(d);
        if let Some(&i) = self.base_funcs.get(&d) {
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
            base_obj: true,
            structor: false,
        });
        self.base_funcs.insert(d, i);
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
    /// `object`, else a temporary) in register 0. With `addr` (a glvalue
    /// immediate invocation, [`Oven::evaluate`]), the address of the
    /// entity glvalue `e` designates, as for a reference's initializer.
    pub(crate) fn compile_thunk(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        object: Option<DeclId>,
        addr: bool,
    ) -> u32 {
        let root = self.srcloc_root.take();
        if let Some(at) = root {
            if let Some(&f) = self.thunks_at.get(&(e, at, addr)) {
                return f;
            }
        } else if let Some(&f) = self.thunks.get(&(e, object, addr)) {
            // Keyed by the object too: a variable's initializer compiles
            // differently from a fold of the same expression (a
            // reference's binds its referent's address where the fold
            // reads its value; extended temporaries are the variable's).
            // A fold first (sema's check of an initializer's immediate
            // invocations) must not answer for the initialization (WP-640).
            return f;
        }
        #[cfg(feature = "op-stats")]
        crate::trial_stats::note_thunk();
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
            base_obj: false,
            structor: false,
        });
        let glvalue = b.ast().exprs.get(e).value_category().is_glvalue();
        let ref_ty = match object.filter(|&d| is_reference_var(b.ast(), d)) {
            // A reference's initializer: the address of its referent.
            Some(d) => Some(b.ast().decls.get(d).ty().unwrap_or(QualType::INT)),
            // A glvalue immediate invocation: the address it designates.
            None if addr && glvalue && object.is_none() => {
                let q = b.ast().exprs.get(e).ty();
                b.lvalue_ref_to(q)
            }
            None => None,
        };
        let mut c = Compiler::new(self, b);
        c.srcloc = root;
        let entry = c.here();
        let ty = c.ety(e);
        if let Some(d) = object {
            c.begin_static_extended(e, d);
        }

        let ret = if let Some(q) = ref_ty {
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
                if let Some(d) = object {
                    // While its initializer runs, the variable names the
                    // object under construction (WP-1100, [basic.life]/1):
                    // its members initialized so far can be read
                    // (`constexpr P p{1, p.x + 1};`).
                    let g = c.o.global_index(c.b.ast(), d);
                    c.emit(Op::BindObject, 0, &[r, g]);
                }
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
        let nregs = c.end_unit();
        let f = &mut self.funcs[idx as usize];
        f.entry = entry;
        f.state = FState::Ready;
        f.nregs = nregs;
        f.ret = ret;
        match root {
            Some(at) => {
                self.thunks_at.insert((e, at, addr), idx);
                self.last_thunk = None;
            }
            None => {
                self.thunks.insert((e, object, addr), idx);
                self.note_thunk(idx, (e, object, addr));
            }
        }
        idx
    }

    /// The function of [`Oven::evaluate_chars`]: in one scope, `size`,
    /// then `data` (a pointer to characters), then `bi::MSG_CHARS`;
    /// returns the size. Not cached (a failed `static_assert` is
    /// evaluated once).
    pub(crate) fn compile_chars(
        &mut self,
        b: &mut dyn SemaBridge,
        size: ExprId,
        data: ExprId,
    ) -> u32 {
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
            base_obj: false,
            structor: false,
        });
        let mut c = Compiler::new(self, b);
        let entry = c.here();
        let size_ty = c.ety(size);
        let data_ty = c.ety(data);
        let ct = match c.kind(data_ty) {
            TyKind::Ptr(t) => t,
            _ => data_ty,
        };
        c.open_scope(&[], true);
        let n = c.reg();
        c.rv_into(size, n);
        let p = c.reg();
        c.rv_into(data, p);
        let loc = c.begin(data);
        c.emit_site(Op::Builtin, crate::kbc::bi::MSG_CHARS, &[n, n, p, p, ct.0, 0], data, loc);
        c.close_scope();
        c.emit(Op::Ret, 1, &[n]);
        let nregs = c.end_unit();
        let f = &mut self.funcs[idx as usize];
        f.entry = entry;
        f.state = FState::Ready;
        f.nregs = nregs;
        f.ret = RetKind::Scalar(size_ty);
        idx
    }

    /// The function of [`Oven::fold_object_size`] (WP-808): in one
    /// scope, clang's `tryEvaluateBuiltinObjectSize` of pointer `p` for
    /// type `ty` ([`Compiler::object_size`] without the side-effect
    /// default); returns the size as a `size_t`. Not cached (each
    /// `pass_object_size` argument is folded once).
    pub(crate) fn compile_object_size(
        &mut self,
        b: &mut dyn SemaBridge,
        p: ExprId,
        ty: u32,
    ) -> u32 {
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
            base_obj: false,
            structor: false,
        });
        let size_t = QualType::unqual(b.ast().types.target().size_t);
        let mut c = Compiler::new(self, b);
        let entry = c.here();
        let st = c.ty(size_t);
        c.open_scope(&[], true);
        let t = c.reg();
        c.konst(t, u128::from(ty & 3));
        let n = c.reg();
        let begin = c.begin(p);
        c.object_size(p, p, t, n, begin, false, Some(ty & 3));
        c.close_scope();
        c.emit(Op::Ret, 1, &[n]);
        let nregs = c.end_unit();
        let f = &mut self.funcs[idx as usize];
        f.entry = entry;
        f.state = FState::Ready;
        f.nregs = nregs;
        f.ret = RetKind::Scalar(st);
        idx
    }

    /// The function of [`Oven::evaluate_with_params`]: in one scope, each
    /// parameter of `binds` initialized from its argument (a reference
    /// bound, an object initialized as a local variable), then `cond`
    /// (`None`: `true`); returns its value. Not cached.
    pub(crate) fn compile_subst(
        &mut self,
        b: &mut dyn SemaBridge,
        cond: Option<ExprId>,
        binds: &[(DeclId, ExprId)],
    ) -> u32 {
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
            base_obj: false,
            structor: false,
        });
        let mut c = Compiler::new(self, b);
        let entry = c.here();
        let n = c.reg();
        c.open_scope(&[], true);
        for &(d, arg) in binds {
            let q = c.b.ast().decls.get(d).ty().unwrap_or(QualType::INT);
            let r = c.reg();
            if c.b.ast().types.is_reference(q) {
                c.bind_ref(arg, r);
            } else {
                let ty = c.ty(q);
                c.alloc_var(r, ty, d, false);
                c.init(arg, r, ty);
                c.end_local_init(d, r, ty);
            }
            c.locals.insert(d, Local::Mem(r));
        }
        let ty = match cond {
            Some(e) => {
                let ty = c.ety(e);
                c.rv_into(e, n);
                ty
            }
            None => {
                c.konst(n, 1);
                c.ty(QualType::BOOL)
            }
        };
        c.close_scope();
        c.emit(Op::Ret, 1, &[n]);
        let nregs = c.end_unit();
        let f = &mut self.funcs[idx as usize];
        f.entry = entry;
        f.state = FState::Ready;
        f.nregs = nregs;
        f.ret = RetKind::Scalar(ty);
        idx
    }

    /// A function calling function `f` without arguments at `at`
    /// ([`crate::Oven::evaluate_call`]), returning what it returns: `None`
    /// if `f` has parameters or returns a reference or an aggregate.
    pub(crate) fn compile_call(
        &mut self,
        b: &mut dyn SemaBridge,
        f: DeclId,
        at: Loc,
    ) -> Option<u32> {
        let ast = b.ast();
        let fty = ast.decls.get(f).ty()?;
        let TypeView::Function(p) = ast.types.canonical_view(fty) else { return None };
        let ret_q = p.ret();
        if p.num_params() != 0 || p.variadic() || ast.types.is_reference(ret_q) {
            return None;
        }
        let fi = self.func_index(b.ast(), f);
        let mut c = Compiler::new(self, b);
        let rty = c.ty(ret_q);
        let void = c.o.types.kind(rty) == TyKind::Void;
        if c.o.types.is_aggregate(rty) {
            return None;
        }
        let entry = c.here();
        let n = c.reg();
        c.emit_loc(Op::Call, 0, &[fi, n], at);
        c.emit(Op::Ret, u32::from(!void), &[n]);
        let nregs = c.end_unit();
        let idx = self.funcs.len() as u32;
        self.funcs.push(Func {
            decl: None,
            entry,
            state: FState::Ready,
            nregs,
            ret: if void { RetKind::Void } else { RetKind::Scalar(rty) },
            params: Vec::new(),
            memo: false,
            this_reg: None,
            this_ty: None,
            base_obj: false,
            structor: false,
        });
        Some(idx)
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
        let nregs = c.end_unit();
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
            base_obj: false,
            structor: false,
        });
        Some(idx)
    }

    /// Compiles the conversion function `d` of a closure type to pointer to
    /// function: it returns the address of `target`
    /// ([expr.prim.lambda.closure]/8).
    fn compile_closure_conversion(
        &mut self,
        b: &mut dyn SemaBridge,
        idx: u32,
        d: DeclId,
        target: DeclId,
    ) {
        let ast = b.ast();
        let fty = ast.decls.get(d).ty().unwrap_or(QualType::INT);
        let ret_q = match ast.types.canonical_view(fty) {
            TypeView::Function(p) => p.ret(),
            _ => QualType::INT,
        };
        let f = self.func_index(b.ast(), target);
        let mut c = Compiler::new(self, b);
        let rty = c.ty(ret_q);
        let this_reg = c.reg();
        let entry = c.here();
        let r = c.reg();
        c.konst(r, u128::from(fn_ptr(f)));
        c.emit(Op::Ret, 1, &[r]);
        let nregs = c.end_unit();
        let cls = {
            let ast = c.b.ast();
            ast.decls.get(d).ctx().and_then(|k| ast.decls.get(ast.first(k)).ty())
        };
        let this_ty = cls.map(|q| c.ty(q));
        let f = &mut self.funcs[idx as usize];
        f.entry = entry;
        f.state = FState::Ready;
        f.nregs = nregs;
        f.ret = RetKind::Scalar(rty);
        f.params = Vec::new();
        f.memo = false;
        f.this_reg = Some(this_reg);
        f.this_ty = this_ty;
    }

    /// Compiles function `idx` if it can be: sets its state.
    pub(crate) fn compile_func(&mut self, b: &mut dyn SemaBridge, idx: u32) {
        #[cfg(feature = "op-stats")]
        crate::trial_stats::note_func();
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
        // A closure's conversion function and static invoker have no body
        // (spec/sema-lambda.md §Evaluation): the conversion returns the
        // invoker; the invoker runs the call operator's body without `this`.
        let thunk = ast.closure_thunk(ast.latest(d));
        if let Some(ClosureThunk::Conversion { target }) = thunk {
            self.compile_closure_conversion(b, idx, d, target);
            return;
        }
        let (d, invoker) = match thunk {
            Some(ClosureThunk::Invoker { call_op }) => (call_op, true),
            _ => (d, false),
        };
        let ast = b.ast();
        let Some(fd) = ast.decls.function(ast.latest(d)) else {
            self.funcs[idx as usize].state = FState::NotConstexpr;
            return;
        };
        if fd.spec.constexpr_kind() == ConstexprKind::None {
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
        if ast.decls.get(def).has(kiln_ast::decl::DeclFlags::INVALID) {
            self.funcs[idx as usize].state = FState::Invalid;
            return;
        }
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
        let std_construct =
            is_std_named(ast, def, "construct_at") || is_std_named(ast, def, "__construct_at");
        let base_obj = self.funcs[idx as usize].base_obj;
        let mut c = Compiler::new(self, b);
        c.base_obj = base_obj;
        c.std_ice = std_ice;
        c.std_construct = std_construct;
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
        let contracts = c.scan_contracts(def);
        c.scan_inits(def);
        c.cur_fn = Some(def);
        c.cur_params.clone_from(&params);
        if matches!(ret, RetKind::Sret(_)) {
            c.reg();
        }
        let mut this_ty = None;
        if class::has_this(c.b.ast(), def) && !invoker {
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
        // The function contract assertions ([dcl.contract.func]/7,
        // spec/contracts.md §6.4): the preconditions after the parameters
        // are initialized, before a constructor's ctor-initializers; the
        // postconditions at each return. A function without any pays
        // nothing.
        for (src, own) in std::mem::take(&mut c.contract_aliases) {
            if let Some(&l) = c.locals.get(&own) {
                c.locals.insert(src, l);
            }
        }
        for s in contracts {
            if c.b.ast().stmts.contract_kind(s) == kiln_ast::stmt::ContractKind::Pre {
                c.contract_check(s);
            } else {
                c.posts.push(s);
            }
        }
        let ast = c.b.ast();
        if ast.stmts.get(body).kind() == StmtKind::CXXTryStmt {
            // A function-try-block ([except.pre]/4): its handlers cover a
            // constructor's ctor-initializers and a destructor's
            // destruction of its subobjects ([except.handle]/13).
            let cdtor = matches!(
                ast.decls.get(def).kind(),
                DeclKind::CXXConstructor | DeclKind::CXXDestructor
            );
            c.lambda_prologue(def);
            c.try_stmt(body, cdtor.then_some(def));
        } else {
            c.member_prologue(def);
            c.lambda_prologue(def);
            c.stmt(body);
        }
        // Falling off the end ([stmt.return]/4).
        if ret == RetKind::Void {
            if !c.posts.is_empty() {
                c.check_posts(None);
            }
            c.emit(Op::Ret, 0, &[0]);
        } else {
            let ast = c.b.ast();
            let end = match ast.stmts.get(body).kind() {
                StmtKind::CXXTryStmt => {
                    let hs = ast.stmts.try_handlers(body);
                    let last = ast.lists.iter::<StmtId>(hs).last();
                    last.map_or(ast.stmts.try_block(body), |h| ast.stmts.catch_block(h))
                }
                _ => body,
            };
            let loc = ast.stmts.rbrace(end);
            c.trap_loc(loc, "control reached end of constexpr function".into());
        }
        let nregs = c.end_unit();
        let this_reg = c.this_reg;
        let memo = c.o.budget.memo_entries > 0
            && !c.uses_cc
            && this_reg.is_none()
            && !matches!(ret, RetKind::Sret(_) | RetKind::Void)
            && infos.len() <= 4
            && infos.iter().all(|p| {
                // An `info` parameter is its id's bits, as stable a key as
                // an integer (spec/reflection.md §3.6).
                matches!(c.o.types.scal(p.ty), Some(Scal::Int { .. } | Scal::Bool | Scal::Float(_)))
                    || matches!(c.o.types.kind(p.ty), TyKind::Reflection)
            })
            && !matches!(ret, RetKind::Scalar(t) if c.o.types.scal(t) == Some(Scal::Ptr));
        let steps = std::mem::take(&mut c.ctor_steps);
        if steps.is_empty() {
            self.ctor_steps.remove(&idx);
        } else {
            self.ctor_steps.insert(idx, steps.into_boxed_slice());
        }
        let structor = matches!(
            b.ast().decls.get(def).kind(),
            DeclKind::CXXConstructor | DeclKind::CXXDestructor
        );
        let f = &mut self.funcs[idx as usize];
        f.entry = entry;
        f.state = FState::Ready;
        f.nregs = nregs;
        f.ret = ret;
        f.params = infos;
        f.memo = memo;
        f.this_reg = this_reg;
        f.this_ty = this_ty;
        f.structor = structor;
    }
}

pub(crate) struct Compiler<'a> {
    pub o: &'a mut Oven,
    pub b: &'a mut dyn SemaBridge,
    next: u32,
    pub max: u32,
    locals: FxHashMap<DeclId, Local>,
    mem_vars: FxHashSet<DeclId>,
    /// The parameters the function contract assertions name, paired with
    /// the definition's, when they were written on another declaration.
    contract_aliases: Vec<(DeclId, DeclId)>,
    pending: Vec<Loc>,
    breakables: Vec<Breakable>,
    /// The enclosing statement expressions: the start of the statement of
    /// each being evaluated, and how many breakables were open outside it
    /// (a `return`, or a `break`/`continue` leaving it, is unsupported).
    stmt_exprs: Vec<(Loc, usize)>,
    /// The enclosing switch statements, innermost last: the labels of
    /// each nested in sub-statements of its body (`None`: all its labels
    /// are direct children of the body).
    switches: Vec<Option<NestedLabels>>,
    /// Mark registers of the open scopes (`None`: allocates nothing).
    scopes: Vec<Option<u32>>,
    ret: RetKind,
    /// The function returns a reference (`ret` is the address).
    ret_ref: bool,
    /// The body creates temporaries (scopes are needed per statement).
    has_temps: bool,
    /// The postconditions checked at each return (spec/contracts.md
    /// §6.4; empty unless the function has some and the semantic checks).
    posts: Vec<StmtId>,
    /// The operator location of the innermost read-modify-write being
    /// compiled (clang reports union active-member faults there).
    rmw_loc: Option<Loc>,
    /// The evaluated common expressions of the enclosing GNU binary
    /// conditionals, keyed by the expression (an `OpaqueValueExpr` names
    /// its source).
    opaque: FxHashMap<ExprId, Opaque>,
    /// The exception object a handler's parameter is being initialized
    /// from (`compile/except.rs`).
    catch_obj: Option<u32>,
    /// The function being compiled is `std::is_constant_evaluated`.
    std_ice: bool,
    /// The function being compiled is `std::construct_at` (or libc++'s
    /// `std::__construct_at`): its placement new constructs `*location`
    /// ([specialized.construct]/2), a `T` whatever designator the
    /// `void *` argument lost (libc++ passes it through `__voidify`).
    std_construct: bool,
    /// The code reads the evaluation context (`Op::IsConstEval`): the
    /// function is not memoized.
    uses_cc: bool,
    /// The register holding `this` (a non-static member function).
    this_reg: Option<u32>,
    /// The object a default member initializer of an aggregate is
    /// initializing (its `this`, [class.mem.general]/11).
    this_override: Option<u32>,
    /// In a lambda call operator capturing `this` or `*this`: the
    /// captured `this` (the `this` of its body; `closure_reg` is the
    /// closure).
    lambda_this: Option<u32>,
    /// In a lambda call operator: the closure object (`this_reg`, or the
    /// explicit object parameter as the closure type).
    closure_reg: Option<u32>,
    /// The storage of the lifetime-extended temporaries of the variable
    /// being initialized, allocated before its initializer's cleanups
    /// scope (keyed by the `MaterializeTemporaryExpr`).
    ext_temps: FxHashMap<ExprId, u32>,
    /// Local class objects declared `const`: allocated modifiable, made
    /// const once initialized ([class.ctor.general]/5).
    late_const: FxHashSet<DeclId>,
    /// The function being compiled (`None`: a thunk).
    cur_fn: Option<DeclId>,
    /// Compiling the base-object variant of a constructor or destructor
    /// ([`Func::base_obj`]).
    base_obj: bool,
    /// Its parameters (forwarded by an inheriting constructor).
    cur_params: Vec<DeclId>,
    /// The index register of the innermost `ArrayInitLoopExpr`.
    array_index: Option<u32>,
    /// The outermost default argument being compiled that holds a
    /// source-location builtin: the builtins in it take the values of
    /// that use (clang `SourceLocExprScopeGuard`; spec/oven.md, Builtins).
    srcloc: Option<ExprId>,
    /// Comma left operands not evaluated: the ignored values of the
    /// `__builtin_object_size` trial being compiled (WP-302).
    skip_effects: Vec<ExprId>,
    /// The construction progress of the constructor being compiled
    /// ([`Oven::ctor_steps`]).
    ctor_steps: Vec<crate::CtorStep>,
    /// The list initializing a constructor's member (or an element of
    /// it) that records element steps ([`Oven::ctor_steps`]).
    elem_track: Option<ElemTrack>,
}

/// A list whose elements record construction steps: it initializes entry
/// `field` of the constructor's class, at `off` bytes from `this`.
#[derive(Clone, Copy)]
pub(crate) struct ElemTrack {
    pub(crate) list: ExprId,
    pub(crate) field: u32,
    pub(crate) off: u64,
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
            contract_aliases: Vec::new(),
            pending: Vec::new(),
            rmw_loc: None,
            opaque: FxHashMap::default(),
            catch_obj: None,
            breakables: Vec::new(),
            stmt_exprs: Vec::new(),
            switches: Vec::new(),
            scopes: Vec::new(),
            ret: RetKind::Void,
            ret_ref: false,
            has_temps: false,
            posts: Vec::new(),
            std_ice: false,
            std_construct: false,
            uses_cc: false,
            this_reg: None,
            this_override: None,
            lambda_this: None,
            closure_reg: None,
            ext_temps: FxHashMap::default(),
            late_const: FxHashSet::default(),
            cur_fn: None,
            base_obj: false,
            cur_params: Vec::new(),
            array_index: None,
            srcloc: None,
            skip_effects: Vec::new(),
            ctor_steps: Vec::new(),
            elem_track: None,
        }
    }

    // ---- emission -------------------------------------------------------

    /// Ends the unit: pads the code (`kbc::pad`, so the VM's fetch window
    /// of its last instruction is in bounds) and returns its register
    /// count.
    fn end_unit(&mut self) -> u32 {
        crate::kbc::pad(&mut self.o.code, &mut self.o.padded);
        self.max
    }

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

    /// Emits `PtrAdd` `[dst, p, idx, elem]` with `aux` (index width,
    /// signedness, negate) and the element step of [`ptr_add_aux`]: the
    /// size's shift plus one when it is a power of two that can no longer
    /// change (the type is not an incomplete class's `Opaque` slot), so
    /// the run loop steps within an array inline, reading no size.
    fn emit_ptr_add(&mut self, aux: u32, args: &[u32; 4], e: ExprId) -> usize {
        let elem = OTy(args[3]);
        let es = self.o.types.size(elem).max(1);
        let step = if aux & 0x1FF != 128
            && es.is_power_of_two()
            && es.trailing_zeros() < ptr_add_aux::MASK
            && !matches!(self.kind(elem), TyKind::Opaque)
        {
            es.trailing_zeros() + 1
        } else {
            0
        };
        self.emit_at(Op::PtrAdd, aux | step << ptr_add_aux::SHIFT, args, e)
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
                        && !matches!(
                            ast.exprs.operator_call_op(e),
                            O::Call | O::Subscript | O::Arrow
                        );
                    match ast.lists.at::<ExprId>(args, 0) {
                        Some(a) if !prefix => a,
                        _ => return n.loc(),
                    }
                }
                ExprKind::CXXRewrittenBinaryOperator => ast.exprs.rewritten_semantic(e),
                ExprKind::PseudoObjectExpr => ast.exprs.pseudo_syntactic(e, &ast.lists),
                ExprKind::CXXBindTemporaryExpr | ExprKind::ExprWithCleanups => ast.exprs.operand(e),
                ExprKind::BinaryConditionalOperator => {
                    ast.exprs.binary_conditional_operands(e, &ast.lists)[0]
                }
                ExprKind::ImplicitCastExpr
                | ExprKind::ConstantExpr
                | ExprKind::ExtVectorElementExpr => ast.exprs.operand(e),
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
                // A qualified name begins at its qualifier (`::operator
                // new`, `std::construct_at`; clang's `getBeginLoc`), which
                // sema records when it is not the name's own location.
                ExprKind::DeclRefExpr => {
                    return ast.templates.expr_begin.get(e).copied().unwrap_or(n.loc());
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
        // A unit's first `here` is its entry: emit it over the previous
        // unit's padding (`kbc::unpad`; a no-op mid-unit, where the code
        // does not end in padding).
        crate::kbc::unpad(&mut self.o.code, &mut self.o.padded);
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

    /// Registers the note of a conditional trap (`Op::Cce`).
    fn trap_index(&mut self, loc: Loc, msg: String) -> u32 {
        let i = self.o.traps.len() as u32;
        self.o.traps.push(TrapInfo { loc, msg, extra: Vec::new() });
        i
    }

    fn trap(&mut self, e: ExprId, msg: &str) {
        let loc = self.b.ast().exprs.get(e).loc();
        self.trap_loc(loc, msg.to_owned());
    }

    /// Traps (and returns true) if `e` is a `_BitInt(N)` with `N > 128`:
    /// the Oven's integers are register values of at most 128 bits; the
    /// arbitrary-precision path is not implemented yet (spec/oven.md
    /// "`_BitInt`").
    fn wide_bit_int(&mut self, e: ExprId) -> bool {
        let q = self.b.ast().exprs.get(e).ty();
        let types = &self.b.ast().types;
        let wide = matches!(
            types.view_if(types.canonical(q).ty(), TypeKind::BitInt),
            Some(TypeView::BitInt { width, .. }) if width > 128
        );
        if wide {
            self.trap(
                e,
                "kiln: constant evaluation of a _BitInt wider than 128 bits is not supported yet",
            );
        }
        wide
    }

    /// [expr.const]/10.22 (before C++26 and P3068, as clang 19):
    /// evaluating a throw-expression is not a core constant expression;
    /// the generic note at the `throw`.
    fn throw_invalid(&mut self, e: ExprId) {
        self.trap(e, "subexpression not valid in a constant expression");
    }

    /// clang `LValue::addUnsizedArray`: a subobject of array of unknown
    /// bound (a flexible array member, spec/sema.md §5.8) does not decay
    /// in a constant expression. Traps (with clang's note at `op`'s
    /// begin) and returns true for such an operand `op` of a decay.
    fn unsized_member_decay(&mut self, op: ExprId) -> bool {
        let ast = self.b.ast();
        let inner = skip_parens(ast, op);
        if !matches!(ast.types.canonical_view(ast.exprs.get(op).ty()), TypeView::IncompleteArray(_))
            || ast.exprs.get(inner).kind() != ExprKind::MemberExpr
        {
            return false;
        }
        let at = self.begin(op);
        self.trap_loc(
            at,
            "array-to-pointer decay of array member without known bound is not supported"
                .to_owned(),
        );
        true
    }

    fn unsupported(&mut self, e: ExprId) {
        let n = self.b.ast().exprs.get(e);
        let k = n.kind();
        if k == ExprKind::RecoveryExpr || n.deps().contains(kiln_ast::expr::ExprDeps::ERROR) {
            // An erroneous expression was diagnosed when it was built: the
            // evaluation fails without a note (clang fails silently on an
            // expression that `containsErrors()`).
            return self.trap(e, "");
        }
        if matches!(
            k,
            ExprKind::AtomicExpr
                | ExprKind::VAArgExpr
                | ExprKind::DesignatedInitUpdateExpr
                | ExprKind::CoawaitExpr
                | ExprKind::CoyieldExpr
                | ExprKind::DependentCoawaitExpr
        ) {
            // clang 19 never constant-evaluates an atomic builtin, a
            // `__builtin_va_arg` or a `DesignatedInitUpdateExpr` (its
            // evaluator has no visitor for one): the generic note at the
            // expression (the lock-free queries are folded in
            // `compile/builtins.rs` before they reach here). Nor a
            // `co_await`/`co_yield` ([expr.const]/5.23, WP-351).
            return self.throw_invalid(e);
        }
        if k.is_cast() && self.b.ast().exprs.cast_kind(e) == CastKind::BuiltinFnToFnPtr {
            // A builtin's callee evaluated apart from its call (a call
            // strips the cast to reach the builtin, `call_inner`): clang's
            // `PointerExprEvaluator` has no case for the cast, so the
            // generic note at the callee (WP-996).
            return self.throw_invalid(e);
        }
        if k.is_cast()
            && self.b.ast().exprs.cast_kind(e) == CastKind::IntegralToBoolean
            && matches!(self.b.ast().types.canonical_view(n.ty()), TypeView::Vector { .. })
        {
            // clang 22's `VectorExprEvaluator` has no case for an integer to
            // `bool` ext vector conversion (sema's clang 20+
            // `ICK_Boolean_Conversion` between vectors): the generic note.
            return self.throw_invalid(e);
        }
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
        let fl = u32::from(is_const) | self.volatile_flag(ty, origin);
        self.emit_at(Op::Alloc, fl, &[dst, ty.0, oi], e);
    }

    fn alloc_var(&mut self, dst: u32, ty: OTy, d: DeclId, is_const: bool) {
        let oi = self.o.push_origin(Origin::Var(d));
        let loc = self.b.ast().decls.get(d).loc();
        let fl = u32::from(is_const) | self.volatile_flag(ty, Origin::Var(d));
        self.emit_loc(Op::Alloc, fl, &[dst, ty.0, oi], loc);
    }

    /// The [`flags::VOLATILE`] bit of a frame object of type `ty` made
    /// for `origin`: a volatile variable or temporary (recorded as such
    /// for `Oven::volatile_path`), or one holding a volatile member
    /// (WP-1019).
    fn volatile_flag(&mut self, ty: OTy, origin: Origin) -> u32 {
        let ast = self.b.ast();
        let whole = match origin {
            Origin::Var(d) => {
                ast.decls.get(d).ty().is_some_and(|q| ast.types.effective_quals(q).is_volatile())
            }
            Origin::Temp(e) => ast.types.effective_quals(ast.exprs.get(e).ty()).is_volatile(),
            _ => false,
        };
        match origin {
            Origin::Var(d) if whole => {
                self.o.volatile_decls.insert(d);
            }
            Origin::Temp(e) if whole => {
                self.o.volatile_exprs.insert(e);
            }
            _ => {}
        }
        if whole || self.o.types.info(ty).has_volatile { u32::from(flags::VOLATILE) } else { 0 }
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

/// clang's `note_constexpr_invalid_cast` for a conversion a
/// `reinterpret_cast` performs ([expr.const]/5.15).
const REINTERPRET_CONV: &str = "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression";

/// GCC's note for a contract violation in a constant evaluation
/// ([basic.contract.eval]/8, spec/contracts.md §6.4).
const CONTRACT_FALSE: &str = "contract predicate is false in constant expression";

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
            StmtKind::AttributedStmt => {
                for a in ast.lists.iter::<kiln_ast::AttrId>(ast.stmts.stmt_attrs(s)) {
                    let node = *ast.attrs.get(a);
                    if node.kind == kiln_ast::attr::AttrKind::ASSUME
                        && let Some(e) = <ExprId as kiln_ast::ids::RawId>::from_raw(node.a)
                    {
                        self.expr(e, false);
                    }
                }
                self.stmt(ast.stmts.sub_stmt(s));
            }
            StmtKind::WhileStmt | StmtKind::DoStmt | StmtKind::ForStmt => {
                let d = *ast.stmts.loop_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                d.cond_var.into_iter().for_each(|x| self.decl(x));
                d.cond.into_iter().for_each(|x| self.expr(x, false));
                d.inc.into_iter().for_each(|x| self.expr(x, true));
                d.body.into_iter().for_each(|x| self.stmt(x));
            }
            StmtKind::CXXForRangeStmt => {
                let d = *ast.stmts.range_for_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                self.decl(d.range);
                d.begin.into_iter().chain(d.end).for_each(|x| self.decl(x));
                d.cond.into_iter().for_each(|x| self.expr(x, false));
                d.inc.into_iter().for_each(|x| self.expr(x, true));
                self.decl(d.loop_var);
                self.stmt(d.body);
            }
            StmtKind::CXXExpansionStmt if ast.stmts.expansion_expanded(s) => {
                let d = *ast.stmts.expansion_data(s);
                d.init.into_iter().for_each(|x| self.stmt(x));
                for &x in ast.lists.get(d.prelude).iter().chain(ast.lists.get(d.expansions)) {
                    self.stmt(StmtId::new(x));
                }
            }
            StmtKind::ReturnStmt => {
                if let Some(v) = ast.stmts.return_value(s) {
                    self.expr(v, false);
                }
            }
            StmtKind::ContractAssertStmt => self.expr(ast.stmts.contract_cond(s), false),
            StmtKind::CXXTryStmt => {
                self.stmt(ast.stmts.try_block(s));
                for h in ast.lists.iter::<StmtId>(ast.stmts.try_handlers(s)) {
                    ast.stmts.catch_var(h).into_iter().for_each(|x| self.decl(x));
                    self.stmt(ast.stmts.catch_block(h));
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
        self.decl_one(d);
        for h in holding_vars(ast, d) {
            self.decl_one(h);
        }
    }

    fn decl_one(&mut self, d: DeclId) {
        let ast = self.ast;
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
        if matches!(ast.types.canonical_view(n.ty()), TypeView::Vector { .. }) {
            // A vector operation may materialize its operands (WP-173).
            self.temps = true;
        }
        match n.kind() {
            ExprKind::DeclRefExpr => {
                let d = ast.exprs.decl(e);
                // A result binding whose address is taken (a lambda in
                // its postcondition captures it by reference) is copied
                // to memory (`check_posts`, spec/contracts.md §6.4).
                if (!ok || self.cur == Some(d))
                    && (is_local_var(ast, d)
                        || ast.decls.get(d).kind() == DeclKind::ContractResultName)
                {
                    self.mem.insert(d);
                }
            }
            ExprKind::ParenExpr => self.expr(ast.exprs.operand(e), ok),
            // `__builtin_choose_expr` / `_Generic`: only the chosen
            // operand is evaluated.
            ExprKind::ChooseExpr | ExprKind::GenericSelectionExpr => {
                if let Some(c) = ast.exprs.selected_operand(e) {
                    self.expr(c, ok);
                }
            }
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
                    // A glvalue cast that drops `const` (`const_cast<int&>(c)
                    // = 2`) may write the variable: it lives in memory, whose
                    // const flag faults that write ([dcl.type.cv]/4; WP-465).
                    CastKind::NoOp
                        if n.value_category().is_glvalue()
                            && (!const_object(ast, ast.exprs.get(op).ty())
                                || const_object(ast, n.ty())) =>
                    {
                        self.expr(op, ok);
                    }
                    CastKind::ToVoid => self.expr(op, true),
                    _ => self.expr(op, false),
                }
            }
            ExprKind::UnaryOperator => {
                let op = ast.exprs.unary_op(e);
                let x = ast.exprs.operand(e);
                match op {
                    UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::Extension => self.expr(x, ok),
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
            | ExprKind::ExpressionTraitExpr
            | ExprKind::CXXReflectExpr
            | ExprKind::ConceptSpecializationExpr
            | ExprKind::RequiresExpr => {}
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
        StmtKind::LabelStmt | StmtKind::AttributedStmt => hunt_cost(ast, StmtId::new(n.b()), locs),
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

/// Whether `s` is an `AttributedStmt` (`[[likely]]`, `[[unlikely]]`)
/// directly on a `case`/`default` label: transparent to the switch's
/// label collection, charging its own step (clang `EvaluateStmt`).
fn attributed_label(ast: &kiln_ast::Ast, s: StmtId) -> bool {
    ast.stmts.get(s).kind() == StmtKind::AttributedStmt
        && matches!(
            ast.stmts.get(ast.stmts.sub_stmt(s)).kind(),
            StmtKind::CaseStmt | StmtKind::DefaultStmt
        )
}

/// The `case`/`default` labels of a switch nested in sub-statements of
/// its body ([stmt.switch]/2 places no restriction; Duff's device): the
/// dispatch reaches each through the scope entries of the statements
/// enclosing it (spec/oven.md "Switch dispatch").
struct NestedLabels {
    /// The selected label's index plus one while a jump descends through
    /// the enclosing statements' scope entries; 0 otherwise.
    sel: u32,
    labels: Vec<NestedLabel>,
    /// Each statement enclosing a nested label inside the switch body,
    /// with the indices of the labels in it.
    paths: FxHashMap<StmtId, Vec<usize>>,
}

struct NestedLabel {
    stmt: StmtId,
    /// The jumps toward the label not yet patched: each lands on the next
    /// hunt site of the label's path ([`Compiler::hunt_site`]), or on the
    /// label's pad once the label is compiled.
    open: Vec<usize>,
    /// The out-of-line pad clearing `sel` and jumping to the label: its
    /// pc and the operand of its jump.
    pad: u32,
    pad_jmp: usize,
    reached: bool,
}

/// Collects the `case`/`default` labels nested in `s` (not those of a
/// nested switch) in source order into `out`, recording in `paths` each
/// statement on the way to one (`path` holds the enclosing ones).
fn nested_labels(
    ast: &kiln_ast::Ast,
    s: StmtId,
    path: &mut Vec<StmtId>,
    out: &mut Vec<StmtId>,
    paths: &mut FxHashMap<StmtId, Vec<usize>>,
) {
    let n = *ast.stmts.get(s);
    match n.kind() {
        StmtKind::CaseStmt | StmtKind::DefaultStmt => {
            for &p in path.iter() {
                paths.entry(p).or_default().push(out.len());
            }
            out.push(s);
            let sub = if n.kind() == StmtKind::CaseStmt {
                ast.stmts.case_data(s).sub
            } else {
                ast.stmts.default_sub(s)
            };
            nested_labels(ast, sub, path, out, paths);
        }
        StmtKind::LabelStmt | StmtKind::AttributedStmt => {
            nested_labels(ast, StmtId::new(n.b()), path, out, paths);
        }
        StmtKind::CompoundStmt => {
            path.push(s);
            for &c in ast.lists.get(ast.stmts.compound_body(s)) {
                nested_labels(ast, StmtId::new(c), path, out, paths);
            }
            path.pop();
        }
        StmtKind::DoStmt | StmtKind::WhileStmt | StmtKind::ForStmt => {
            if let Some(b) = ast.stmts.loop_data(s).body {
                path.push(s);
                nested_labels(ast, b, path, out, paths);
                path.pop();
            }
        }
        StmtKind::IfStmt => {
            let d = *ast.stmts.if_data(s);
            path.push(s);
            for x in d.then.into_iter().chain(d.els) {
                nested_labels(ast, x, path, out, paths);
            }
            path.pop();
        }
        _ => {}
    }
}

/// The values a `case` label selects (`lo..=hi`; `None` for `default`).
fn case_range(ast: &kiln_ast::Ast, s: StmtId) -> Option<(u128, u128)> {
    if ast.stmts.get(s).kind() != StmtKind::CaseStmt {
        return None;
    }
    let cd = *ast.stmts.case_data(s);
    let lo = eval_int(ast, cd.lhs).unwrap_or(0);
    let hi = cd.rhs.and_then(|r| eval_int(ast, r)).unwrap_or(lo);
    Some((lo, hi))
}

/// The steps clang charges hunting for the label `target` nested in `s`
/// (`EvaluateStmt` with a non-null `Case`: it descends labels, compound
/// statements, loop bodies and `if` branches, stepping into a `for`'s or
/// an `if`'s init-statement on the way), pushed on `locs`; whether
/// `target` was found. The label's own step is charged at the label.
fn hunt_to(ast: &kiln_ast::Ast, s: StmtId, target: StmtId, locs: &mut Vec<Loc>) -> bool {
    if s == target {
        return true;
    }
    let n = *ast.stmts.get(s);
    locs.push(n.loc());
    match n.kind() {
        StmtKind::CaseStmt => hunt_to(ast, ast.stmts.case_data(s).sub, target, locs),
        StmtKind::DefaultStmt => hunt_to(ast, ast.stmts.default_sub(s), target, locs),
        StmtKind::LabelStmt | StmtKind::AttributedStmt => {
            hunt_to(ast, StmtId::new(n.b()), target, locs)
        }
        StmtKind::CompoundStmt => ast
            .lists
            .get(ast.stmts.compound_body(s))
            .iter()
            .any(|&c| hunt_to(ast, StmtId::new(c), target, locs)),
        StmtKind::DoStmt | StmtKind::WhileStmt | StmtKind::ForStmt => {
            let d = *ast.stmts.loop_data(s);
            if let Some(i) = d.init {
                hunt_to(ast, i, target, locs);
            }
            d.body.is_some_and(|b| hunt_to(ast, b, target, locs))
        }
        StmtKind::IfStmt => {
            let d = *ast.stmts.if_data(s);
            if let Some(i) = d.init {
                hunt_to(ast, i, target, locs);
            }
            d.then.into_iter().chain(d.els).any(|x| hunt_to(ast, x, target, locs))
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
    /// The index of a label nested in a sub-statement of the body
    /// ([`NestedLabels`]).
    nested: Option<usize>,
}

impl Compiler<'_> {
    fn scan(&mut self, body: StmtId, params: &[DeclId]) {
        let ast = self.b.ast();
        let mut s = Scan { ast, mem: &mut self.mem_vars, cur: None, temps: false };
        s.stmt(body);
        self.has_temps = s.temps;
        let _ = params;
    }

    /// The function contract assertions of `def` checked in a constant
    /// evaluation ([basic.contract.eval]; none under the ignore
    /// semantic), scanned as the body's statements are.
    fn scan_contracts(&mut self, def: DeclId) -> Vec<StmtId> {
        let ast = self.b.ast();
        let Some(l) = ast.function_contracts(def).filter(|_| ast.contract_semantic.checks()) else {
            return Vec::new();
        };
        let list: Vec<StmtId> = ast.lists.iter::<StmtId>(l).collect();
        let mut s = Scan { ast, mem: &mut self.mem_vars, cur: None, temps: false };
        for &c in &list {
            s.stmt(c);
        }
        self.has_temps |= s.temps;
        // Written on another declaration: its parameters are `def`'s.
        for (src, own) in ast.contract_param_aliases(def) {
            if self.mem_vars.contains(&src) {
                self.mem_vars.insert(own);
            }
            self.contract_aliases.push((src, own));
        }
        list
    }

    /// A contract assertion `s` under a checking semantic
    /// ([basic.contract.eval]/4-8, spec/contracts.md §6.4): its predicate
    /// is evaluated in place (its temporaries in a scope of their own); a
    /// violation is `Op::ContractFail`'s. Under observe the predicate is a
    /// trial (`Op::ContractTrial`), so that one that is not a constant
    /// can be reported and evaluation go on.
    fn contract_check(&mut self, s: StmtId) {
        let ast = self.ast();
        let cond = ast.stmts.contract_cond(s);
        let loc = ast.stmts.get(s).loc();
        let observe = ast.contract_semantic == kiln_ast::stmt::ContractSemantic::Observe;
        let m = self.next;
        let skip = self.emit(Op::ContractSkip, 0, &[0]) + 1;
        let trap = self.trap_index(loc, CONTRACT_FALSE.into());
        let pred = |c: &mut Self| {
            if c.has_temps {
                c.open_scope(&[], true);
                let r = c.rv(cond);
                c.close_scope();
                r
            } else {
                c.rv(cond)
            }
        };
        if observe {
            let pc = self.emit_loc(Op::ContractTrial, 0, &[0, 0, trap], loc);
            let v = pred(self);
            self.emit(Op::Ret, 1, &[v]);
            let end = self.here();
            self.patch(pc + 1, v);
            self.patch(pc + 2, end);
        } else {
            let r = pred(self);
            let ok = self.jmp_if(r, true);
            self.emit_loc(Op::ContractFail, 0, &[trap], loc);
            self.patch_here(ok);
        }
        self.patch_here(skip);
        self.next = m;
    }

    /// The postconditions at a normal return ([dcl.contract.func]/7,
    /// spec/contracts.md §6.4): the locals are destroyed, then each is
    /// checked with its result name bound to the result object in place
    /// (`res`: the register holding the value, or for a reference or a
    /// class result its address).
    fn check_posts(&mut self, res: Option<u32>) {
        self.exit_scopes(0);
        let posts = std::mem::take(&mut self.posts);
        for &p in &posts {
            let ast = self.ast();
            if let (Some(rn), Some(r)) = (ast.stmts.contract_result_name(p), res) {
                let l = if self.ret_ref || matches!(self.ret, RetKind::Sret(_)) {
                    Local::Mem(r)
                } else if self.mem_vars.contains(&rn) {
                    // Its address is taken: a copy in memory.
                    let RetKind::Scalar(ty) = self.ret else { continue };
                    let q = self.reg();
                    self.alloc_var(q, ty, rn, true);
                    self.emit(Op::InitStore, 0, &[q, r, ty.0]);
                    Local::Mem(q)
                } else {
                    Local::Reg(r)
                };
                self.locals.insert(rn, l);
            }
            self.contract_check(p);
        }
        self.posts = posts;
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
        // A volatile variable's reads go to its allocation, which
        // diagnoses them ([expr.const]/10.8, WP-1019).
        let ast = self.b.ast();
        if ast.decls.get(d).ty().is_some_and(|q| ast.types.effective_quals(q).is_volatile()) {
            return true;
        }
        self.b.ast().decls.var(d).is_some_and(|v| v.init.is_none())
    }

    /// Opens a scope allocating the memory locals among `decls`.
    fn open_scope(&mut self, decls: &[DeclId], force: bool) {
        let mut mem = Vec::new();
        let mut force = force;
        let ast = self.b.ast();
        let decls: Vec<DeclId> =
            decls.iter().flat_map(|&d| std::iter::once(d).chain(holding_vars(ast, d))).collect();
        for &d in &decls {
            let ast = self.b.ast();
            if !is_local_var(ast, d) || ast.decls.get(d).kind() != DeclKind::Var {
                continue;
            }
            // A reference may extend the lifetime of a temporary to the
            // end of this scope ([class.temporary]/6).
            // So may a non-reference one: an enumerating expansion
            // statement's element ([class.temporary]/8, CWG3043).
            force |= self.has_temps && (is_reference_var(ast, d) || extends_temporaries(ast, d));
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
    /// A class result initializes the object at `obj` (`(pointer, type)`)
    /// instead (WP-799).
    fn stmt_expr(&mut self, e: ExprId, dst: Option<u32>) {
        self.stmt_expr_to(e, dst, None);
    }

    fn stmt_expr_to(&mut self, e: ExprId, dst: Option<u32>, obj: Option<(u32, OTy)>) {
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
            match (obj, dst) {
                (Some((p, ty)), _) => self.init(x, p, ty),
                (None, Some(dst)) => self.rv_into(x, dst),
                (None, None) => self.effect(x),
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
                let hunt = self.hunt_begin(s);
                self.open_scope(&decls, false);
                self.hunt_site(s, hunt);
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
                let hunt = self.hunt_begin(s);
                self.open_scope(&decls, false);
                self.hunt_site(s, hunt);
                if let Some(i) = d.init {
                    self.stmt(i);
                }
                if let Some(v) = d.cond_var {
                    self.decl_one(v);
                }
                match kind {
                    // `if consteval` takes its compound statement iff the
                    // evaluation is manifestly constant-evaluated
                    // ([stmt.if]/4): a run-time test of the evaluation's
                    // context, as `__builtin_is_constant_evaluated()`, since
                    // a trial fold runs the same code (spec/oven.md §9.1).
                    IfKind::Consteval | IfKind::NotConsteval => {
                        let r = self.reg();
                        // aux 16: the context alone (no -Wconstant-evaluated).
                        self.emit(Op::IsConstEval, 16, &[r]);
                        let j = self.jmp_if(r, kind == IfKind::NotConsteval);
                        if let Some(t) = d.then {
                            self.stmt(t);
                        }
                        if let Some(e) = d.els {
                            let end = self.jmp();
                            self.patch_here(j);
                            self.stmt(e);
                            self.patch_here(end);
                        } else {
                            self.patch_here(j);
                        }
                    }
                    _ => {
                        let j = match d.cond {
                            Some(c) => {
                                let r = self.rv(c);
                                self.cond_bindings(d.cond_var);
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
            StmtKind::CXXForRangeStmt => {
                self.range_for_stmt(s);
                self.next = m;
            }
            StmtKind::CXXExpansionStmt if ast.stmts.expansion_expanded(s) => {
                self.expansion_stmt(s);
                self.next = m;
            }
            StmtKind::BreakStmt | StmtKind::ContinueStmt => {
                let brk = n.kind() == StmtKind::BreakStmt;
                let idx = if brk {
                    self.breakables.len().checked_sub(1)
                } else {
                    self.breakables.iter().rposition(|b| b.conts.is_some())
                };
                // A jump leaving a statement expression is unsupported
                // (clang `note_constexpr_stmt_expr_unsupported`): one in
                // a loop or switch header leaves it for the statement,
                // whose breakable is not open yet (spec/sema.md §7.4a).
                if let Some(&(at, outside)) = self.stmt_exprs.last()
                    && idx.is_none_or(|i| i < outside)
                {
                    self.trap_loc(at, STMT_EXPR_UNSUPPORTED.into());
                    return;
                }
                let Some(idx) = idx else {
                    self.trap_loc(n.loc(), "kiln: 'break' or 'continue' outside a loop".into());
                    return;
                };
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
                // clang 22 drops a `return` whose operand has an error
                // (`containsErrors()`): evaluation goes on past it, to
                // "control reached end of constexpr function" when nothing
                // else returns (spec/oven.md "Erroneous expressions").
                if self.o.clang_compat >= 22
                    && v.is_some_and(|v| ast.exprs.is_erroneous_operand(v, &ast.lists))
                {
                    return;
                }
                let posts = !self.posts.is_empty();
                match (self.ret, v) {
                    (RetKind::Sret(ty), Some(v)) => {
                        self.init(v, 0, ty);
                        if posts {
                            self.check_posts(Some(0));
                        }
                        self.emit(Op::Ret, 0, &[0]);
                    }
                    (RetKind::Scalar(_), Some(v)) if self.ret_ref => {
                        let r = self.reg();
                        self.bind_ref(v, r);
                        if posts {
                            self.check_posts(Some(r));
                        }
                        self.emit(Op::Ret, 1, &[r]);
                    }
                    (RetKind::Scalar(_), Some(v)) => {
                        let r = self.rv(v);
                        if posts {
                            self.check_posts(Some(r));
                        }
                        self.emit(Op::Ret, 1, &[r]);
                    }
                    (_, v) => {
                        if let Some(v) = v {
                            self.effect(v);
                        }
                        if posts {
                            self.check_posts(None);
                        }
                        self.emit(Op::Ret, 0, &[0]);
                    }
                }
                self.next = m;
            }
            StmtKind::CaseStmt | StmtKind::DefaultStmt if self.is_nested_label(s) => {
                self.nested_label(s);
                let ast = self.b.ast();
                let sub = if n.kind() == StmtKind::CaseStmt {
                    ast.stmts.case_data(s).sub
                } else {
                    ast.stmts.default_sub(s)
                };
                self.stmt(sub);
            }
            StmtKind::CaseStmt | StmtKind::DefaultStmt => {
                self.trap_loc(
                    n.loc(),
                    "kiln: a 'case' label outside its switch body is not supported yet".into(),
                );
            }
            // [expr.const]/5: evaluating a goto statement is not a core
            // constant expression (clang: the generic note at the `goto`).
            // [dcl.asm]/1, [expr.const]/5: an asm statement is never a
            // core constant expression (clang: the generic note at `asm`;
            // spec/oven.md, WP-140).
            // A coroutine body or `co_return` ([dcl.constexpr]/3,
            // [expr.const]/5.23: a coroutine is never constexpr; clang's
            // `EvaluateStmt` has no case for either: the generic note at
            // the statement, spec/oven.md, WP-351).
            StmtKind::GotoStmt
            | StmtKind::IndirectGotoStmt
            | StmtKind::GCCAsmStmt
            | StmtKind::CoroutineBodyStmt
            | StmtKind::CoreturnStmt => {
                self.trap_loc(n.loc(), "subexpression not valid in a constant expression".into());
            }
            // A label is evaluated by evaluating its statement (clang
            // charges a step for each: `EvaluateStmt` recurses).
            StmtKind::LabelStmt => self.stmt(StmtId::new(n.b())),
            // [expr.const]/5.25 (C++20, P1002): a try-block is evaluated by
            // evaluating its compound statement (clang `EvaluateStmt`, one
            // step for the try itself); from C++26 (P3068) its handlers
            // catch exceptions thrown in it (`compile/except.rs`).
            StmtKind::CXXTryStmt => self.try_stmt(s, None),
            // [dcl.attr.assume]/1, [expr.const]/5.8: an assumption that
            // would evaluate to false is not a core constant expression;
            // one with possible side effects is ignored (clang
            // `EvaluateStmt`, `note_constexpr_assumption_failed` at the
            // assumption's `getExprLoc`).
            StmtKind::AttributedStmt => {
                let nodes: Vec<kiln_ast::attr::AttrNode> = ast
                    .lists
                    .iter::<kiln_ast::AttrId>(ast.stmts.stmt_attrs(s))
                    .map(|a| *ast.attrs.get(a))
                    .collect();
                let sub = ast.stmts.sub_stmt(s);
                for node in nodes {
                    if node.kind != kiln_ast::attr::AttrKind::ASSUME
                        || node.flags & kiln_ast::attr::AttrNode::ASSUME_IGNORED != 0
                    {
                        continue;
                    }
                    let Some(e) = <ExprId as kiln_ast::ids::RawId>::from_raw(node.a) else {
                        continue;
                    };
                    let r = if self.has_temps {
                        self.open_scope(&[], true);
                        let r = self.rv(e);
                        self.close_scope();
                        r
                    } else {
                        self.rv(e)
                    };
                    let ok = self.jmp_if(r, true);
                    self.trap_loc(Loc::from_raw(node.b), "assumption evaluated to false".into());
                    self.patch_here(ok);
                }
                self.stmt(sub);
            }
            // [stmt.contract.assert]: not evaluated under the ignore
            // semantic (spec/contracts.md §6.4).
            StmtKind::ContractAssertStmt => {
                if ast.contract_semantic.checks() {
                    self.contract_check(s);
                }
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
        let hunt = self.hunt_begin(s);
        self.open_scope(&decls, false);
        self.hunt_site(s, hunt);
        if let Some(i) = d.init {
            self.stmt(i);
        }
        let top = self.here();
        let iter = self.has_temps || d.cond_var.is_some();
        let break_depth = self.scopes.len();
        if iter {
            let decls: Vec<DeclId> = d.cond_var.into_iter().collect();
            let hunt = self.hunt_begin(s).map(|_| top);
            self.open_scope(&decls, true);
            self.hunt_site(s, hunt);
        }
        let mut exit = None;
        if kind != StmtKind::DoStmt {
            if let Some(v) = d.cond_var {
                self.decl_one(v);
            }
            if let Some(c) = d.cond {
                let m = self.next;
                let r = self.rv(c);
                // The bindings' registers live as long as the loop (the
                // statement's caller resets `next`).
                if !self.cond_bindings(d.cond_var) {
                    self.next = m;
                }
                exit = Some(self.jmp_if(r, false));
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

    /// A range-based `for` (clang `EvaluateStmt` on `CXXForRangeStmt`):
    /// the init-statement and `__range`, `__begin`, `__end` in the
    /// statement's scope; per iteration the condition, the loop variable
    /// and the body, then `++__begin` before the loop variable is
    /// destroyed. The temporaries of the for-range-initializer extended
    /// to `__range` (P2718) live in the statement's scope.
    fn range_for_stmt(&mut self, s: StmtId) {
        let ast = self.b.ast();
        let d = *ast.stmts.range_for_data(s);
        let loc = ast.stmts.get(s).loc();
        let (Some(begin), Some(end), Some(cond), Some(inc)) = (d.begin, d.end, d.cond, d.inc)
        else {
            self.trap_loc(loc, "kiln: a range-based for statement without its iterators".into());
            return;
        };
        let mut decls = self.stmt_decls(d.init, None);
        decls.extend([d.range, begin, end]);
        self.open_scope(&decls, false);
        if let Some(i) = d.init {
            self.stmt(i);
        }
        // One step for each implicit declaration statement, as clang.
        for v in [d.range, begin, end] {
            self.pending.push(self.b.ast().decls.get(v).loc());
            self.decl(v);
        }
        let top = self.here();
        let break_depth = self.scopes.len();
        self.open_scope(&[d.loop_var], self.has_temps);
        let iter = self.scopes.last().copied().flatten();
        let m = self.next;
        let r = self.rv(cond);
        let exit = self.jmp_if(r, false);
        self.next = m;
        let cont_depth = self.scopes.len();
        self.breakables.push(Breakable {
            breaks: Vec::new(),
            conts: Some(Vec::new()),
            depth: break_depth,
            cont_depth,
        });
        self.pending.push(self.b.ast().decls.get(d.loop_var).loc());
        self.decl(d.loop_var);
        self.stmt(d.body);
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
        let m = self.next;
        self.effect(inc);
        self.next = m;
        if let Some(mark) = iter {
            self.emit(Op::ScopeExit, 0, &[mark]);
        }
        let j = self.jmp();
        self.patch(j, top);
        self.patch_here(exit);
        self.close_scope();
        let end_pc = self.here();
        for j in b.breaks {
            self.patch(j, end_pc);
        }
        self.close_scope();
    }

    /// An expanded expansion statement ([stmt.expand]/5, P1306R5): the
    /// init-statement and the prelude in the statement's scope, then each
    /// element's block in order. `break` leaves the statement; `continue`
    /// ends the current element's block (the next one follows).
    fn expansion_stmt(&mut self, s: StmtId) {
        let ast = self.b.ast();
        let d = *ast.stmts.expansion_data(s);
        let prelude: Vec<u32> = ast.lists.get(d.prelude).to_vec();
        let elems: Vec<u32> = ast.lists.get(d.expansions).to_vec();
        let mut decls = self.stmt_decls(d.init, None);
        decls.extend(self.block_decls(&prelude));
        self.open_scope(&decls, false);
        if let Some(i) = d.init {
            self.stmt(i);
        }
        for p in prelude {
            self.stmt(StmtId::new(p));
        }
        let depth = self.scopes.len();
        let mut breaks = Vec::new();
        for e in elems {
            self.breakables.push(Breakable {
                breaks: Vec::new(),
                conts: Some(Vec::new()),
                depth,
                cont_depth: depth,
            });
            self.stmt(StmtId::new(e));
            let b = self.breakables.pop().unwrap_or(Breakable {
                breaks: Vec::new(),
                conts: None,
                depth: 0,
                cont_depth: 0,
            });
            let next = self.here();
            for j in b.conts.unwrap_or_default() {
                self.patch(j, next);
            }
            breaks.extend(b.breaks);
        }
        let end = self.here();
        for j in breaks {
            self.patch(j, end);
        }
        self.close_scope();
    }

    /// Where a hunt site of statement `s` begins ([`Compiler::hunt_site`]):
    /// `Some(pc)` before `s` opens a scope when `s` encloses a label
    /// nested in the innermost switch's body.
    fn hunt_begin(&mut self, s: StmtId) -> Option<u32> {
        let on_path = matches!(self.switches.last(), Some(Some(ns)) if ns.paths.contains_key(&s));
        on_path.then(|| self.here())
    }

    /// A hunt site: statement `s`, enclosing nested labels of the
    /// innermost switch, opened a scope from `begin`. A jump toward one
    /// of those labels lands at `begin`, so the scope's objects are
    /// allocated as on entry, then goes on toward its label (one test of
    /// `sel` on the ordinary path). Nothing is emitted when the scope
    /// allocated nothing: the jump goes on past it.
    fn hunt_site(&mut self, s: StmtId, begin: Option<u32>) {
        let Some(begin) = begin else { return };
        if self.o.code.len() as u32 == begin {
            return;
        }
        let Some(Some(ns)) = self.switches.last_mut() else { return };
        let Some(ks) = ns.paths.get(&s).cloned() else { return };
        let sel = ns.sel;
        let ks: Vec<usize> = ks.into_iter().filter(|&k| !ns.labels[k].reached).collect();
        if ks.is_empty() {
            return;
        }
        for &k in &ks {
            for j in std::mem::take(&mut ns.labels[k].open) {
                self.o.code[j] = begin;
            }
        }
        let skip = self.jmp_if(sel, false);
        let mut out = Vec::new();
        if let [k] = ks[..] {
            out.push((k, self.jmp()));
        } else {
            let t = self.reg();
            for &k in &ks {
                self.konst(t, k as u128 + 1);
                self.emit(Op::ICmp, int_aux(32, false) | cmp::EQ << 9, &[t, sel, t]);
                out.push((k, self.jmp_if(t, true)));
            }
        }
        self.patch_here(skip);
        if let Some(Some(ns)) = self.switches.last_mut() {
            for (k, j) in out {
                ns.labels[k].open.push(j);
            }
        }
    }

    /// Whether `s` is a label nested in a sub-statement of the innermost
    /// switch's body.
    fn is_nested_label(&self, s: StmtId) -> bool {
        matches!(self.switches.last(), Some(Some(ns)) if ns.labels.iter().any(|l| l.stmt == s))
    }

    /// The nested label `s` ([`Compiler::is_nested_label`]): the jumps
    /// toward it land on its pad, which clears `sel`.
    fn nested_label(&mut self, s: StmtId) {
        let Some(Some(ns)) = self.switches.last() else { return };
        let Some(k) = ns.labels.iter().position(|l| l.stmt == s) else { return };
        // The label is before its own step, as a direct child's.
        let own = self.pending.pop();
        let label = self.here();
        self.pending.extend(own);
        let Some(Some(ns)) = self.switches.last_mut() else { return };
        let l = &mut ns.labels[k];
        l.reached = true;
        let (pad, pad_jmp, open) = (l.pad, l.pad_jmp, std::mem::take(&mut l.open));
        self.o.code[pad_jmp] = label;
        for j in open {
            self.o.code[j] = pad;
        }
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
        // Collect the case labels: direct children of the body and their
        // chains, then those nested in sub-statements of the body.
        let mut targets: Vec<CaseTarget> = Vec::new();
        let mut nested: Vec<StmtId> = Vec::new();
        let mut paths: FxHashMap<StmtId, Vec<usize>> = FxHashMap::default();
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
                    StmtKind::CaseStmt => (case_range(ast, cur), ast.stmts.case_data(cur).sub),
                    StmtKind::DefaultStmt => (None, ast.stmts.default_sub(cur)),
                    // `[[likely]] case 1:`: one step on the way in.
                    _ if attributed_label(ast, cur) => {
                        chain.push(ast.stmts.get(cur).loc());
                        cur = ast.stmts.sub_stmt(cur);
                        continue;
                    }
                    _ => break,
                };
                targets.push(CaseTarget {
                    stmt: cur,
                    range,
                    pre: chain.clone(),
                    stub_jmp: 0,
                    nested: None,
                });
                chain.push(ast.stmts.get(cur).loc());
                cur = sub;
            }
            // The labels nested in the child's statement, each reached
            // after the steps of hunting through it.
            let first = nested.len();
            nested_labels(ast, cur, &mut Vec::new(), &mut nested, &mut paths);
            for (i, &l) in nested.iter().enumerate().skip(first) {
                let mut at = chain.clone();
                hunt_to(ast, cur, l, &mut at);
                let range = case_range(ast, l);
                targets.push(CaseTarget { stmt: l, range, pre: at, stub_jmp: 0, nested: Some(i) });
            }
            hunt_cost(ast, c, &mut pre);
        }
        let ctx_decls = self.stmt_decls(d.init, d.cond_var);
        self.open_scope(&ctx_decls, false);
        if let Some(i) = d.init {
            self.stmt(i);
        }
        if let Some(v) = d.cond_var {
            self.decl_one(v);
        }
        let cond_ty = self.ety(d.cond);
        let (cw, cs) = match self.o.types.kind(cond_ty) {
            TyKind::Int { width, signed } => (u32::from(width), signed),
            _ => (8, false),
        };
        let wide_cond = self.o.types.wide(cond_ty).is_some();
        let c = if wide_cond { self.wide_place(d.cond) } else { self.rv(d.cond) };
        // [stmt.pre]/4 (P0963): tuple-like bindings after the condition.
        self.cond_bindings(d.cond_var);
        let block_decls = if body_is_compound {
            let ch: Vec<u32> = children.iter().map(|s| s.index()).collect();
            self.block_decls(&ch)
        } else {
            Vec::new()
        };
        self.open_scope(&block_decls, false);
        // A label nested in a sub-statement is reached by a jump that
        // descends through the scope entries on its way: `sel` names it.
        let sel = if nested.is_empty() {
            None
        } else {
            let r = self.reg();
            self.konst(r, 0);
            Some(r)
        };
        // Dispatch.
        let t = self.reg();
        let mut stubs: Vec<(usize, usize)> = Vec::new();
        let aux_eq = int_aux(cw as u8, cs) | cmp::EQ << 9;
        for (i, tg) in targets.iter().enumerate() {
            let Some((lo, hi)) = tg.range else { continue };
            if wide_cond {
                // A wide condition: each case value compared in memory.
                let cd = *self.b.ast().stmts.case_data(tg.stmt);
                if let Some(rhs) = cd.rhs {
                    let u = self.reg();
                    self.wide_case_cmp(cmp::GE, u, c, cond_ty, cd.lhs);
                    let skip = self.jmp_if(u, false);
                    self.wide_case_cmp(cmp::LE, u, c, cond_ty, rhs);
                    let j = self.jmp_if(u, true);
                    stubs.push((i, j));
                    self.patch_here(skip);
                } else {
                    self.wide_case_cmp(cmp::EQ, t, c, cond_ty, cd.lhs);
                    let j = self.jmp_if(t, true);
                    stubs.push((i, j));
                }
                continue;
            }
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
        let mut labels: Vec<NestedLabel> = nested
            .iter()
            .map(|&stmt| NestedLabel { stmt, open: Vec::new(), pad: 0, pad_jmp: 0, reached: false })
            .collect();
        for &(i, j) in &stubs {
            self.patch_here(j);
            let pre = targets[i].pre.clone();
            self.pending.extend(pre);
            match (targets[i].nested, sel) {
                (Some(k), Some(sel)) => {
                    self.konst(sel, k as u128 + 1);
                    let j = self.jmp();
                    labels[k].open.push(j);
                }
                _ => targets[i].stub_jmp = self.jmp(),
            }
        }
        // The nested labels' pads, out of line.
        if let Some(sel) = sel {
            for l in &mut labels {
                l.pad = self.here();
                self.konst(sel, 0);
                l.pad_jmp = self.jmp();
            }
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
        self.switches.push(sel.map(|sel| NestedLabels { sel, labels, paths }));
        for &ch in &children {
            let mut cur = ch;
            loop {
                let ast = self.b.ast();
                let k = ast.stmts.get(cur).kind();
                let sub = match k {
                    StmtKind::CaseStmt => ast.stmts.case_data(cur).sub,
                    StmtKind::DefaultStmt => ast.stmts.default_sub(cur),
                    _ if attributed_label(ast, cur) => {
                        let sub = ast.stmts.sub_stmt(cur);
                        self.step(cur);
                        cur = sub;
                        continue;
                    }
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
        let ns = self.switches.pop().flatten();
        self.close_scope();
        let end = self.here();
        for j in b.into_iter().chain(end_jumps) {
            self.patch(j, end);
        }
        // A nested label the body did not compile (one in a statement
        // the descent does not enter, which sema rejects): selecting it
        // fails the evaluation.
        if let Some(ns) = ns
            && ns.labels.iter().any(|l| !l.reached)
        {
            let over = self.jmp();
            for l in ns.labels.iter().filter(|l| !l.reached) {
                let at = self.here();
                for &j in l.open.iter().chain([&l.pad_jmp]) {
                    self.patch(j, at);
                }
                let loc = self.b.ast().stmts.get(l.stmt).loc();
                self.trap_loc(
                    loc,
                    "kiln: a 'case' label in this statement is not supported yet".into(),
                );
            }
            self.patch_here(over);
        }
        // A target whose stub was never reached by the linear pass (cannot
        // happen for direct children) would jump to 0: point it at the end.
        for tg in targets.iter().filter(|t| t.nested.is_none()) {
            if tg.stub_jmp != 0 && self.o.code[tg.stub_jmp] == 0 {
                self.o.code[tg.stub_jmp] = end;
            }
        }
        self.close_scope();
    }

    /// A declaration in a `DeclStmt` (or a condition variable); a
    /// decomposition's holding variables follow it.
    fn decl(&mut self, d: DeclId) {
        self.decl_one(d);
        for h in holding_vars(self.b.ast(), d) {
            self.decl_one(h);
        }
    }

    /// A condition variable's tuple-like bindings, initialized after the
    /// condition is evaluated ([stmt.pre]/4, P0963).
    /// Whether there were any.
    fn cond_bindings(&mut self, v: Option<DeclId>) -> bool {
        let hs = v.map(|v| holding_vars(self.b.ast(), v)).unwrap_or_default();
        for &h in &hs {
            self.decl_one(h);
        }
        !hs.is_empty()
    }

    fn decl_one(&mut self, d: DeclId) {
        // A constexpr local's initializer is a constant expression of its
        // own ([expr.const]/22): not part of a consteval block's plainly
        // constant-evaluated evaluation (spec/consteval-blocks.md §3).
        let ast = self.b.ast();
        let constexpr_local = ast.decls.get(d).kind() == DeclKind::Var
            && is_local_var(ast, d)
            && ast.decls.var(d).is_some_and(|v| v.spec.is_constexpr());
        if !constexpr_local {
            return self.decl_one_var(d);
        }
        let saved = self.reg();
        self.emit(Op::IsConstEval, 4, &[saved]);
        self.decl_one_var(d);
        self.emit(Op::IsConstEval, 8, &[saved]);
    }

    fn decl_one_var(&mut self, d: DeclId) {
        let ast = self.b.ast();
        let n = *ast.decls.get(d);
        if n.kind() != DeclKind::Var {
            return;
        }
        let Some(v) = ast.decls.var(d).copied() else { return };
        if !is_local_var(ast, d) {
            // clang `EvaluateStmt`: a static or thread-local variable not
            // usable in constant expressions; a `const` integral or
            // enumeration one is taken as usable (its initializer is
            // checked where it is read). WP-1123.
            let tls = v.spec.tls() != kiln_ast::decl::TlsKind::None;
            let usable = v.spec.is_constexpr()
                || n.ty().is_some_and(|q| {
                    let quals = ast.types.effective_quals(q);
                    quals.is_const()
                        && !quals.is_volatile()
                        && (ast.types.is_integral(q)
                            || matches!(ast.types.canonical_view(q), TypeView::Enum(_)))
                });
            if (tls || v.spec.storage() == VarStorage::Static) && !usable {
                let what = if tls { "thread_local" } else { "static" };
                self.trap_loc(
                    n.loc(),
                    format!("control flows through the definition of a {what} variable"),
                );
            }
            return;
        }
        if n.has(kiln_ast::decl::DeclFlags::INVALID) {
            // A variable whose declaration sema already diagnosed (no
            // viable constructor, say): the evaluation fails with no note,
            // as clang's on the erroneous declaration.
            self.trap_loc(n.loc(), String::new());
            return;
        }
        if is_reference_var(ast, d) {
            let r = self.reg();
            if let Some(init) = v.init {
                let m = self.next;
                let ext = self.begin_extended(init);
                self.bind_ref(init, r);
                self.ref_chk(r, n.ty().unwrap_or(QualType::INT), init);
                self.end_extended(&ext);
                self.next = m;
            }
            self.locals.insert(d, Local::Mem(r));
            return;
        }
        let q = n.ty().unwrap_or(QualType::INT);
        if let Some(init) = v.init
            && self.cur_fn.is_some()
            && !self.literal_check_relaxed()
            && !is_literal_type(ast, q)
        {
            // clang `CheckLiteralType` (from `EvaluateInPlace`): a local
            // variable of non-literal type, named with its initializer's
            // whole (array) type. clang 22 drops the check from C++23 on
            // (P2448R2: [basic.types.general]/10 no longer constrains
            // constant evaluation): its constructor or destructor decides.
            let t = Oven::type_name(ast, ast.exprs.get(init).ty().without_quals());
            self.trap(
                init,
                &format!("non-literal type '{t}' cannot be used in a constant expression"),
            );
        }
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
                        // A scalar's initializer extends temporaries only
                        // in an enumerating expansion statement's element
                        // ([class.temporary]/8, CWG3043).
                        let ext = self.begin_extended(init);
                        self.rv_into(init, r);
                        self.end_extended(&ext);
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

pub(crate) fn scalar_bits(s: &Scalar<'_>) -> Option<u128> {
    match s {
        Scalar::Int { bits, signed, .. } => {
            let w = bits.width();
            bits.to_u128().map(|v| crate::arith::canon(v, w, *signed))
        }
        Scalar::Bool { value, .. } => Some(u128::from(*value)),
        Scalar::Float { bits, .. } => Some(*bits),
        Scalar::Null { .. } => Some(0),
        Scalar::Reflection { id, .. } => Some(u128::from(id.bits())),
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
        if self.wide_bit_int(e) {
            return;
        }
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
            // spec/sema-templates.md §8.3, §8.5: Sema checked satisfaction.
            ExprKind::ConceptSpecializationExpr | ExprKind::RequiresExpr => {
                let v = self.ast().exprs.satisfied(e).unwrap_or(false);
                self.konst(dst, u128::from(v));
            }
            // spec/sema-templates.md §9.1, §9.4: Sema evaluated the trait.
            ExprKind::TypeTraitExpr | ExprKind::ExpressionTraitExpr => {
                let v = self.ast().exprs.type_trait_value(e);
                self.konst(dst, u128::from(v));
            }
            // spec/reflection.md §2.1: the interned id's value.
            ExprKind::CXXReflectExpr => match self.ast().exprs.reflect_value(e) {
                Some(id) => self.konst(dst, u128::from(id.bits())),
                None => self.unsupported(e),
            },
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
            ExprKind::AddrLabelExpr => {
                self.emit_at(Op::LabelAddr, 0, &[dst, e.index()], e);
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
            ExprKind::ExtVectorElementExpr => {
                // A prvalue (so prvalue-based) one-lane access.
                let r = self.ext_elt_regs(e).first().copied().unwrap_or(dst);
                self.emit(Op::Mov, 0, &[dst, r]);
            }
            ExprKind::ChooseExpr | ExprKind::GenericSelectionExpr
                if self.ast().exprs.selected_operand(e).is_some() =>
            {
                let op = self.ast().exprs.selected_operand(e).expect("guarded");
                self.rv_inner(op, dst);
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                let op = self.ast().exprs.rewritten_semantic(e);
                self.rv_inner(op, dst);
            }
            ExprKind::PseudoObjectExpr => {
                let r = self.bind_pseudo(e, Access::Read);
                self.rv_inner(r, dst);
                self.unbind_pseudo(e);
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
                // A glvalue's value is the address it designates
                // (`Oven::evaluate`, WP-640), not a value of its type.
                let cv = ast.exprs.constant_value(e).filter(|_| !n.value_category().is_glvalue());
                let scalar = cv.and_then(|c| ast.consts.scalar(c));
                let v = scalar.as_ref().and_then(scalar_bits);
                let ptr = matches!(scalar, Some(Scalar::Pointer { .. }));
                if let Some(v) = v {
                    self.konst(dst, v);
                } else if let (true, Some(cv)) = (ptr, cv) {
                    // A pointer constant (a substituted template argument
                    // into a `define_static_array` object): thawed.
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
            // An enumerator named by a class member access (`r.E`,
            // `p->E`): the object expression is evaluated, as a glvalue
            // for `.` (nothing is read) and as a pointer for `->`, then
            // discarded ([expr.ref]/7.5; clang `VisitIgnoredBaseExpression`).
            ExprKind::MemberExpr
                if self.ast().decls.enumerator(self.ast().exprs.decl(e)).is_some() =>
            {
                let ast = self.ast();
                let v = ast.decls.enumerator(ast.exprs.decl(e)).and_then(|x| x.value);
                let bits = v.and_then(|v| ast.consts.scalar(v)).and_then(|s| scalar_bits(&s));
                let base = ast.exprs.operand(e);
                let m = self.next;
                if self.node(e).has(kiln_ast::expr::ExprFlags::ARROW) {
                    self.rv(base);
                } else {
                    self.lv(base, Access::Addr);
                }
                self.next = m;
                let (w, s) = int_of(self.kind(ty)).unwrap_or((64, true));
                self.konst(dst, crate::arith::canon(bits.unwrap_or(0), w, s));
            }
            ExprKind::UnaryExprOrTypeTraitExpr => self.trait_expr(e, dst, ty),
            // An invalid class's layout is a placeholder: no note (clang).
            ExprKind::OffsetOfExpr if self.ast().exprs.offsetof_invalid_record(e) => {
                self.trap(e, "");
            }
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
                let fold = self.bcp_fold_enter(ExprId::new(n.a()));
                let c = self.rv(ExprId::new(n.a()));
                let [t, f] = self.ast().exprs.cond_arms(e);
                let j = self.jmp_if(c, false);
                self.rv_into(t, dst);
                let end = self.jmp();
                self.patch_here(j);
                self.rv_into(f, dst);
                self.patch_here(end);
                self.bcp_fold_exit(fold);
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
            ExprKind::CXXThrowExpr => self.throw_expr(e),
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
                Some(x) => {
                    let saved = self.enter_default_arg(e);
                    self.rv_inner(x, dst);
                    self.srcloc = saved;
                }
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
        // The size of a variable length array (GNU extension) is not a
        // constant: clang's generic note at the `sizeof`.
        if matches!(kind, TraitKind::SizeOf | TraitKind::DataSizeOf) && is_vla(ast, q) {
            return self.throw_invalid(e);
        }
        // `alignof e` naming a variable or data member is that
        // declaration's alignment (clang `GetAlignOfExpr`): an `alignas`
        // on it counts.
        if matches!(kind, TraitKind::AlignOf | TraitKind::PreferredAlignOf)
            && let Some(d) = self.trait_operand_decl(e)
        {
            let v = self.b.alignof_decl(d);
            if v != 0 {
                let (w, s) = int_of(self.kind(ty)).unwrap_or((64, false));
                self.konst(dst, crate::arith::canon(u128::from(v), w, s));
                return;
            }
        }
        let t = self.ty(q);
        let v = match kind {
            TraitKind::SizeOf => self.o.types.size(t),
            // clang's `getTypeInfoDataSizeInChars`: a class's `dsize` (its
            // size without the tail padding a derived class may reuse),
            // any other type's size.
            TraitKind::DataSizeOf => {
                let ast = self.b.ast();
                match ast.types.as_record(q).and_then(|r| ast.definition(r)) {
                    Some(def) => match self.b.ensure_layout(def) {
                        Some(l) => l.data_size,
                        None => self.o.types.size(t),
                    },
                    None => self.o.types.size(t),
                }
            }
            TraitKind::AlignOf | TraitKind::PreferredAlignOf => {
                u64::from(self.typedef_align(q).unwrap_or_else(|| self.o.types.info(t).align))
            }
            TraitKind::VecStep => {
                self.unsupported(e);
                return;
            }
        };
        let (w, s) = int_of(self.kind(ty)).unwrap_or((64, false));
        self.konst(dst, crate::arith::canon(u128::from(v), w, s));
    }

    /// The alignment in bytes of `q` when a typedef in its sugar carries
    /// `__attribute__((aligned))` (clang `getTypeInfo` of a `TypedefType`
    /// honours the typedef's own alignment; the Oven's type model is
    /// canonical and drops it), via `kiln_abi`; `None` when no typedef in
    /// the sugar chain has attributes. libc++'s `atomic_ref` names its
    /// object through `typedef _Tp _Aligned_Tp
    /// __attribute__((aligned(required_alignment)))`.
    pub(super) fn typedef_align(&mut self, q: QualType) -> Option<u32> {
        let ast = self.ast();
        let mut x = q;
        let attributed = loop {
            if let TypeView::Typedef { decl, .. } = ast.types.view(x.ty())
                && ast.decls.get(decl).has(kiln_ast::decl::DeclFlags::HAS_ATTRS)
            {
                break true;
            }
            let d = ast.types.desugar_once(x);
            if d.ty() == x.ty() {
                break false;
            }
            x = d;
        };
        if !attributed {
            return None;
        }
        let (size, align) = self.b.opaque_size_align(q);
        (size != 0).then_some(align)
    }

    /// The declaration an `alignof e` operand names: `e` (under
    /// parentheses) is a `DeclRefExpr` or `MemberExpr`.
    fn trait_operand_decl(&self, e: ExprId) -> Option<DeclId> {
        let ast = self.ast();
        if ast.exprs.trait_type_arg(e).is_some() {
            return None;
        }
        let op = skip_parens(ast, ExprId::new(ast.exprs.get(e).a()));
        match ast.exprs.get(op).kind() {
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => Some(ast.exprs.decl(op)),
            _ => None,
        }
    }

    fn unary(&mut self, e: ExprId, dst: u32, ty: OTy) {
        let op = self.ast().exprs.unary_op(e);
        let x = self.ast().exprs.operand(e);
        match op {
            UnaryOp::Plus | UnaryOp::Extension => self.rv_inner(x, dst),
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
            UnaryOp::Real | UnaryOp::Imag => self.real_imag(e, dst),
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
                // The read itself reports a null pointer (as "dereferencing"
                // at clang 22: `notes.rs`), so no separate check.
                let p = Place::Mem(self.rv(x));
                self.load(p, ty, ak::READ, e, dst);
            }
            UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec => {
                let place = self.lv_rmw(x, e);
                let xty = self.ety(x);
                self.inc_dec(place, xty, op, Some(dst), e);
            }
            UnaryOp::Coawait => self.unsupported(e),
        }
    }

    /// Increment/decrement of `place`; `out` receives the old value for a
    /// postfix operator, the new one for a prefix one.
    fn inc_dec(&mut self, place: Place, ty: OTy, op: UnaryOp, out: Option<u32>, e: ExprId) {
        if self.o.types.wide(ty).is_some() {
            // A wide value is never read through `out` (the postfix form
            // is `wide_init`'s).
            return self.wide_inc_dec(place, ty, op, e);
        }
        if let Some(elem) = self.cx_elem(ty) {
            // clang `IncDecSubobjectHandler`: the real part only.
            // A complex value is never read through `out` (the postfix
            // form is a `cx_op`), so it is left unwritten.
            let _ = (self.cx_inc_dec(place, elem, op, e), out);
            return;
        }
        let inc = matches!(op, UnaryOp::PostInc | UnaryOp::PreInc);
        let akind = if inc { ak::INCREMENT } else { ak::DECREMENT };
        let x = self.ast().exprs.operand(e);
        self.volatile_glvalue(x, akind, e);
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
                self.emit_ptr_add(aux, &[new, old, one, pointee.0], e);
            }
            k => {
                let (w, s) = int_of(k).unwrap_or((32, true));
                let one = self.reg();
                self.konst(one, 1);
                if k == TyKind::Bool {
                    // C++17 removed `++` on bool; `--` never existed. Sema
                    // rejects both; treat as `= true` for robustness.
                    self.konst(new, 1);
                } else if s && w < 32 {
                    // [expr.pre.incr]/1: `x += 1`, computed in the
                    // promoted `int` (which cannot overflow from a
                    // narrower type) and converted back modulo 2^w
                    // ([conv.integral]/3); clang's `canOverflow()` is
                    // false here. Unsigned narrow types wrap already.
                    let wide = self.reg();
                    self.emit_at(
                        if inc { Op::IAdd } else { Op::ISub },
                        int_aux(32, true),
                        &[wide, old, one],
                        e,
                    );
                    let aux = 32 | 1 << 8 | w << 9 | 1 << 17;
                    self.emit(Op::ICast, aux, &[new, wide]);
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
            if let Place::Bits { .. } | Place::Lane { .. } = place
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
        if self.cx_operands(e) {
            return self.cx_compare(e, dst);
        }
        if self.wide_operands(e) {
            return self.wide_compare(e, dst);
        }
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

    /// The builtin comparison `c` of `ra` and `rb`, operands of kinds
    /// `kinds` (after the usual conversions), into `dst`.
    fn compare(&mut self, e: ExprId, kinds: (TyKind, TyKind), c: u32, dst: u32, ra: u32, rb: u32) {
        match kinds {
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
            // Interned reflections likewise ([expr.eq]/6, P2996).
            (TyKind::MemPtr { func, .. }, _) | (_, TyKind::MemPtr { func, .. }) => {
                if func {
                    self.emit_at(Op::MpCmp, 0, &[ra, rb], e);
                }
                self.emit(Op::ICmp, int_aux(64, false) | c << 9, &[dst, ra, rb]);
            }
            (TyKind::Reflection, _) | (_, TyKind::Reflection) => {
                self.emit(Op::ICmp, int_aux(64, false) | c << 9, &[dst, ra, rb]);
            }
            (k, _) => {
                let (w, s) = int_of(k).unwrap_or((32, true));
                self.emit(Op::ICmp, int_aux(w as u8, s) | c << 9, &[dst, ra, rb]);
            }
        }
    }

    /// The builtin three-way comparison `e` ([expr.spaceship]/4-8) into
    /// the comparison-category object at `p`: its single member holds
    /// the integer of the selected value (clang's `VisitBinCmp`: for
    /// `partial_ordering`, `==` then `<` then `>` else unordered).
    fn three_way(&mut self, e: ExprId, p: u32, ty: OTy) {
        let TyKind::Record { decl, .. } = self.kind(ty) else {
            return self.unsupported(e);
        };
        let Some(info) = self.ast().cmp_categories.for_record(decl).copied() else {
            return self.unsupported(e);
        };
        let Some(fi) = self.o.types.fields(ty).iter().copied().find(|f| f.decl == info.field)
        else {
            return self.unsupported(e);
        };
        let [a, b] = self.ast().exprs.operands(e);
        let (at, bt) = (self.ety(a), self.ety(b));
        let kinds = (self.kind(at), self.kind(bt));
        let wide = self.o.types.wide(at).is_some() && at == bt;
        let (ra, rb) = if wide {
            let ra = self.wide_place(a);
            (ra, self.wide_place(b))
        } else {
            let ra = self.rv(a);
            (ra, self.rv(b))
        };
        let r = self.reg();
        let c = self.reg();
        let partial = info.kind == kiln_ast::class::CmpCategory::Partial;
        let int = |x: i64| x as i128 as u128;
        // Tests in order; the first that holds selects its value.
        let mut tests = vec![
            (cmp::EQ, int(info.equal_or_equiv_int())),
            (cmp::LT, int(info.int(kiln_ast::class::CmpResult::Less))),
        ];
        let fallback = if partial {
            tests.push((cmp::GT, int(info.int(kiln_ast::class::CmpResult::Greater))));
            int(info.int(kiln_ast::class::CmpResult::Unordered))
        } else {
            int(info.int(kiln_ast::class::CmpResult::Greater))
        };
        let mut ends = Vec::new();
        for (k, v) in tests {
            if wide {
                self.wide_cmp_at(k, c, [ra, rb], at, e);
            } else {
                self.compare(e, kinds, k, c, ra, rb);
            }
            let j = self.jmp_if(c, false);
            self.konst(r, v);
            ends.push(self.jmp());
            self.patch_here(j);
        }
        self.konst(r, fallback);
        for j in ends {
            self.patch_here(j);
        }
        if self.o.types.size(ty) > self.o.types.size(fi.ty) {
            self.emit_at(Op::ZeroInit, 0, &[p, ty.0], e);
        }
        let q = self.elem_ptr(p, fi.offset, e);
        self.emit_at(Op::InitStore, 0, &[q, r, fi.ty.0], e);
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
            self.compare(e, (ak_, bk), c, dst, ra, rb);
            return;
        }
        if matches!(op, BinaryOp::Add | BinaryOp::Sub) {
            if let (TyKind::Ptr(pa), TyKind::Ptr(_)) = (ak_, bk) {
                let ra = self.opnd(a, ga, false);
                let rb = self.opnd(b, gb, false);
                // [expr.add]/5: pointers to a zero-size element type
                // (`int[0]`) have no element count between them; aux 1
                // sends the VM to its zero-size check (clang
                // `note_constexpr_pointer_subtraction_zero_size`, WP-948).
                let zero = u32::from(
                    matches!(self.o.types.kind(pa), TyKind::Array { .. })
                        && self.o.types.size(pa) == 0,
                );
                self.emit_at(Op::PtrSub, zero, &[dst, ra, rb, pa.0], e);
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
                self.emit_ptr_add(aux, &[dst, rp, ri, pointee.0], e);
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
        let op = self.ast().exprs.operand(e);
        if self.is_wide_expr(op) && self.o.types.wide(ty).is_none() {
            return self.wide_to_scalar(e, dst, ty);
        }
        let ast = self.ast();
        let ck = ast.exprs.cast_kind(e);
        let ek = ast.exprs.get(e).kind();
        let reinterpret = ek == ExprKind::CXXReinterpretCastExpr;
        if reinterpret {
            // [expr.const]/5.15, whatever conversion it performs. A fold
            // continues past the conversions clang's does (`Op::Cce`).
            let msg = "reinterpret_cast is not allowed in a constant expression";
            if !matches!(
                ck,
                CastKind::NoOp
                    | CastKind::BitCast
                    | CastKind::IntegralToPointer
                    | CastKind::PointerToIntegral
            ) {
                self.trap(e, msg);
                return;
            }
            self.cce_check(e, msg);
        }
        match ck {
            CastKind::LValueToRValue => {
                let q = self.node(op).ty();
                if q.quals_bits() & 2 != 0 {
                    let msg = format!(
                        "read of volatile-qualified type '{}' is not allowed in a constant expression",
                        Oven::type_name(self.ast(), q)
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
            CastKind::NoOp
            | CastKind::UserDefinedConversion
            | CastKind::ConstructorConversion
            | CastKind::NonAtomicToAtomic
            | CastKind::AtomicToNonAtomic => {
                self.rv_inner(op, dst);
            }
            CastKind::DerivedToBase
            | CastKind::UncheckedDerivedToBase
            | CastKind::BaseToDerived => {
                let p = self.rv(op);
                self.class_cast(e, p, dst, true, ck == CastKind::BaseToDerived);
            }
            CastKind::Dynamic => self.dynamic_cast(e, dst, false),
            CastKind::IntegralComplexToReal
            | CastKind::FloatingComplexToReal
            | CastKind::IntegralComplexToBoolean
            | CastKind::FloatingComplexToBoolean => self.cx_to_scalar(e, dst),
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
                if self.unsized_member_decay(op) {
                    self.konst(dst, 0);
                } else {
                    let p = self.lv_mem(op, Access::Addr);
                    if p != dst {
                        self.emit(Op::Mov, 0, &[dst, p]);
                    }
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
            CastKind::BitCast if self.lanes(ty).is_some() || self.is_vector_expr(op) => {
                if self.lanes(ty).is_some() {
                    self.vec_bitcast(e, dst, None);
                } else {
                    // clang 19 folds no bit cast of a vector to a scalar
                    // (spec/oven.md "Vectors").
                    let loc = self.begin(e);
                    self.trap_loc(loc, "subexpression not valid in a constant expression".into());
                }
            }
            CastKind::BitCast => {
                let from = self.ety(op);
                let (fk, tk) = (self.kind(from), self.kind(ty));
                let r = self.rv(op);
                match (fk, tk) {
                    (TyKind::Ptr(fp), TyKind::Ptr(tp))
                        if reinterpret
                            && !matches!(self.kind(fp), TyKind::Func)
                            && !matches!(self.kind(tp), TyKind::Func) =>
                    {
                        // clang: a cast to the same type (`CK_NoOp`) keeps
                        // the designator valid.
                        if fp == tp {
                            self.emit(Op::Mov, 0, &[dst, r]);
                        } else {
                            self.cce(e, cce::PTR, dst, r, REINTERPRET_CONV);
                        }
                    }
                    (TyKind::Ptr(fp), TyKind::Ptr(tp)) if !reinterpret => {
                        let fvoid = self.kind(fp) == TyKind::Void;
                        let tvoid = self.kind(tp) == TyKind::Void;
                        // clang permits any cast from `void *` within
                        // `std::allocator<T>::allocate` (an empty
                        // allocation's `T[0]` included).
                        let in_allocate = fvoid
                            && self.cur_fn.is_some_and(|f| {
                                new_delete::std_allocator_elem(self.ast(), f, "allocate").is_some()
                            });
                        let func = matches!(self.kind(fp), TyKind::Func)
                            || matches!(self.kind(tp), TyKind::Func);
                        if func && fp != tp && self.o.clang_compat >= 22 {
                            // A C-style cast between a function pointer and
                            // `void *` performs a reinterpret_cast
                            // ([expr.cast]/4, [expr.const]/10.15): clang 22
                            // and GCC 16.1 reject it; clang 19 folds it
                            // (WP-830).
                            self.cce(e, cce::PTR, dst, r, REINTERPRET_CONV);
                        } else if tvoid && !fvoid {
                            // `T *` to `void *` keeps the designated
                            // subobject's type for a later placement new
                            // (spec/oven.md "Construction").
                            self.emit(Op::VoidCast, crate::kbc::VOID_TAG, &[dst, r, fp.0]);
                        } else if tvoid || fp == tp || in_allocate {
                            self.emit(Op::Mov, 0, &[dst, r]);
                        } else if fvoid && !self.b.cxx26() {
                            self.void_cast_pre26(e, op, dst, r);
                        } else if fvoid {
                            self.emit_at(Op::VoidCast, 0, &[dst, r, tp.0], e);
                        } else {
                            // A function pointer included: a fold keeps
                            // the function it designates (clang's invalid
                            // designator; a call through it fails).
                            self.cce(e, cce::PTR, dst, r, REINTERPRET_CONV);
                        }
                    }
                    // A `reinterpret_cast` to or from a function pointer
                    // (`cce_check` above failed it outside a fold).
                    (TyKind::Ptr(_), TyKind::Ptr(_)) => {
                        self.cce(e, cce::PTR, dst, r, REINTERPRET_CONV);
                    }
                    _ => self.trap(e, REINTERPRET_CONV),
                }
            }
            CastKind::LValueToRValueBitCast => self.bit_cast(e, dst, None),
            CastKind::IntegralToPointer | CastKind::PointerToIntegral => {
                // clang notes the cast before evaluating its operand.
                if !reinterpret {
                    self.cce_check(e, REINTERPRET_CONV);
                }
                let r = self.rv(op);
                let aux = if ck == CastKind::IntegralToPointer {
                    cce::INT_TO_PTR
                } else {
                    cce::PTR_TO_INT
                };
                self.cce(e, aux, dst, r, REINTERPRET_CONV);
            }
            _ => self.unsupported(e),
        }
    }

    /// [expr.const]/5.15 before C++26 (P2738): a conversion from `void *`
    /// to a pointer to object type is never a core constant expression.
    /// As clang, at the cast: a null operand names only the source type, a
    /// non-null one adds "in C++ standards before C++2c" (spec/oven.md
    /// "Casts from void *").
    /// A fold continues with the result pointing at nothing (`Op::Cce`).
    fn void_cast_pre26(&mut self, e: ExprId, op: ExprId, dst: u32, r: u32) {
        let ast = self.ast();
        let src = Oven::quoted_aka(ast, ast.exprs.get(op).ty());
        let msg = format!("cast from {src} is not allowed in a constant expression");
        let loc = self.begin(e);
        let nz = self.reg();
        self.emit(Op::PtrToBool, 0, &[nz, r]);
        let j = self.jmp_if(nz, true);
        let t = self.trap_index(loc, msg.clone());
        self.emit_loc(Op::Cce, cce::PTR, &[dst, r, t], loc);
        let end = self.jmp();
        self.patch_here(j);
        let t = self.trap_index(loc, format!("{msg} in C++ standards before C++2c"));
        self.emit_loc(Op::Cce, cce::PTR, &[dst, r, t], loc);
        self.patch_here(end);
    }

    /// `Op::Cce` `CHECK` with note `msg` at `e`: fails outside a fold.
    fn cce_check(&mut self, e: ExprId, msg: &str) {
        let loc = self.b.ast().exprs.get(e).loc();
        let t = self.trap_index(loc, msg.to_owned());
        self.emit_loc(Op::Cce, cce::CHECK, &[0, 0, t], loc);
    }

    /// `Op::Cce` conversion `aux` of register `r` into `dst`, failing
    /// with note `msg` at `e` outside a fold.
    fn cce(&mut self, e: ExprId, aux: u32, dst: u32, r: u32, msg: &str) {
        let loc = self.b.ast().exprs.get(e).loc();
        let t = self.trap_index(loc, msg.to_owned());
        self.emit_loc(Op::Cce, aux, &[dst, r, t], loc);
    }

    /// The function an lvalue of function type designates, as a function
    /// pointer.
    fn fn_lv(&mut self, e: ExprId) -> u32 {
        let e = skip_parens(self.ast(), e);
        let n = self.node(e);
        match n.kind() {
            // A reference to a function (`_Pred& __pred`): its referent.
            ExprKind::DeclRefExpr
                if !self.ast().decls.get(self.ast().exprs.decl(e)).kind().is_function() =>
            {
                self.lv_mem(e, Access::Read)
            }
            ExprKind::DeclRefExpr => {
                let d = self.ast().exprs.decl(e);
                let f = self.o.func_index(self.b.ast(), d);
                let r = self.reg();
                self.konst(r, u128::from(fn_ptr(f)));
                r
            }
            // A static member function named by a member access (`t.f`):
            // the object expression is evaluated, then discarded
            // ([expr.ref]/7.3.1).
            ExprKind::MemberExpr
                if self.ast().decls.get(self.ast().exprs.decl(e)).kind().is_function() =>
            {
                let d = self.ast().exprs.decl(e);
                let base = self.ast().exprs.operand(e);
                if self.node(e).has(kiln_ast::expr::ExprFlags::ARROW) {
                    self.rv(base);
                } else {
                    self.lv(base, Access::Addr);
                }
                let f = self.o.func_index(self.b.ast(), d);
                let r = self.reg();
                self.konst(r, u128::from(fn_ptr(f)));
                r
            }
            ExprKind::UnaryOperator if self.ast().exprs.unary_op(e) == UnaryOp::Deref => {
                let x = self.ast().exprs.operand(e);
                self.rv(x)
            }
            // `static_cast<F&&>(f)` and other no-op casts of a function
            // lvalue ([expr.static.cast]/4) designate the same function.
            k if k == ExprKind::ConstantExpr
                || (k.is_cast() && self.ast().exprs.cast_kind(e) == CastKind::NoOp) =>
            {
                let x = self.ast().exprs.operand(e);
                self.fn_lv(x)
            }
            // A reference to a function reached otherwise (a member
            // `P&& p` bound to `f`, then `s.p(…)`): the reference holds the
            // function's address, which is the place [`Self::lv`] yields.
            _ => self.lv_mem(e, Access::Addr),
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
            Place::Lane { p, idx, vty } => {
                self.emit_at(Op::LoadLane, akind, &[dst, p, idx, vty.0], e);
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
            Place::Lane { p, idx, vty } => {
                self.emit_at(Op::StoreLane, 0, &[p, src, idx, vty.0], e);
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

    /// Evaluates the semantic expressions of a `PseudoObjectExpr` before
    /// its result, in order, binding each `OpaqueValueExpr`'s source
    /// once for its uses (spec/oven.md "Pseudo-object expressions"); returns the result. The
    /// bindings live until [`Self::unbind_pseudo`].
    fn bind_pseudo(&mut self, e: ExprId, acc: Access) -> ExprId {
        let ast = self.ast();
        let result = ast.exprs.pseudo_result(e, &ast.lists);
        for s in ast.exprs.pseudo_semantics(e, &ast.lists) {
            if s == result {
                break;
            }
            match self.ast().exprs.opaque_source(s) {
                Some(src) if self.node(s).kind() == ExprKind::OpaqueValueExpr => {
                    let v = if self.node(src).value_category().is_glvalue() {
                        Opaque::Lv(self.lv(src, acc))
                    } else {
                        Opaque::Rv(self.rv(src))
                    };
                    self.opaque.insert(src, v);
                }
                _ => self.effect(s),
            }
        }
        result
    }

    /// Drops the bindings [`Self::bind_pseudo`] made for `e`.
    fn unbind_pseudo(&mut self, e: ExprId) {
        let ast = self.ast();
        let sources: Vec<ExprId> = ast
            .exprs
            .pseudo_semantics(e, &ast.lists)
            .into_iter()
            .filter(|&s| ast.exprs.get(s).kind() == ExprKind::OpaqueValueExpr)
            .filter_map(|s| ast.exprs.opaque_source(s))
            .collect();
        for src in sources {
            self.opaque.remove(&src);
        }
    }

    fn opaque_of(&self, e: ExprId) -> Option<Opaque> {
        match self.ast().exprs.opaque_source(e) {
            Some(src) => self.opaque.get(&src).copied(),
            None => self.catch_object().map(Opaque::Lv),
        }
    }

    fn lv_mem(&mut self, e: ExprId, acc: Access) -> u32 {
        match self.lv(e, acc) {
            Place::Mem(p) => p,
            Place::Reg(_) | Place::Bits { .. } | Place::Lane { .. } => {
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
            ExprKind::ChooseExpr | ExprKind::GenericSelectionExpr
                if self.ast().exprs.selected_operand(e).is_some() =>
            {
                let op = self.ast().exprs.selected_operand(e).expect("guarded");
                self.lv(op, acc)
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                let op = self.ast().exprs.rewritten_semantic(e);
                self.lv(op, acc)
            }
            ExprKind::PseudoObjectExpr => {
                let r = self.bind_pseudo(e, acc);
                let p = self.lv(r, acc);
                self.unbind_pseudo(e);
                p
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
                let d = self.ast().exprs.decl(e);
                // A function lvalue ([expr.prim.id.unqual]/3) bound to a
                // reference (`P&& p = f`): its place is its address.
                if self.ast().decls.get(d).kind().is_function() {
                    return Place::Mem(self.fn_lv(e));
                }
                if let Some(p) = self.captured_var_place(e, d) {
                    return p;
                }
                let ast = self.ast();
                if ast.decls.get(d).kind().is_function() {
                    // A function lvalue (bound to a reference to function):
                    // its address is the function pointer value.
                    return Place::Mem(self.fn_lv(e));
                }
                if ast.decls.get(d).kind() == DeclKind::Binding {
                    // A binding names its expression ([dcl.struct.bind]).
                    let Some(b) = ast.decls.binding_expr(d) else {
                        self.unsupported(e);
                        return Place::Mem(self.reg());
                    };
                    return self.lv(b, acc);
                }
                if let Some(l) = self.locals.get(&d).copied() {
                    return match l {
                        Local::Reg(r) => Place::Reg(r),
                        Local::Mem(p) => Place::Mem(p),
                    };
                }
                // A function parameter outside its call (an expression
                // evaluated in the body: a `static_assert`, a template
                // argument) has no frame for it. Like an automatic variable
                // outside its frame, it takes the globals path as an
                // external object (`value::var_info`): naming it succeeds
                // (clang `VisitVarDecl` `Success(VD)`), so `X<&q>` fails in
                // the result check ("pointer to 'q' is not a constant
                // expression"), and only a read faults ("function parameter
                // 'q' with unknown value ...", `notes.rs`).
                if !matches!(
                    ast.decls.get(d).kind(),
                    DeclKind::Var | DeclKind::ParmVar | DeclKind::TemplateParamObject
                ) {
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
            // `__func__` and friends designate their `StringLiteral`
            // ([dcl.fct.def.general]/8; clang `getFunctionName`).
            ExprKind::PredefinedExpr if self.ast().exprs.predefined_name(e).is_some() => {
                let sl = self.ast().exprs.predefined_name(e).unwrap_or(e);
                self.lv(sl, acc)
            }
            // A namespace-scope array literal names its static object, an
            // implicit internal variable (WP-466).
            ExprKind::CompoundLiteralExpr
                if let Some(d) = self.ast().exprs.compound_literal_var(e) =>
            {
                let g = self.o.global_index(self.b.ast(), d);
                let r = self.reg();
                self.emit_at(Op::GlobalAddr, 0, &[r, g], e);
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
            ExprKind::MemberExpr
                if self.ast().decls.get(self.ast().exprs.decl(e)).kind().is_function() =>
            {
                Place::Mem(self.fn_lv(e))
            }
            ExprKind::MemberExpr => self.member(e, acc),
            ExprKind::ArraySubscriptExpr
                if self.o.clang_compat >= 22 && {
                    let [a, _] = self.ast().exprs.operands(e);
                    self.is_vector_expr(a)
                } =>
            {
                // clang 22 evaluates a lane access as an element of the
                // lane array (spec/oven.md "Vectors").
                let [a, i] = self.ast().exprs.operands(e);
                let p = self.vec_object(a, lane_base_access(acc));
                let ri = self.rv(i);
                let ru = self.reg();
                self.emit(Op::ICast, 64 << 9, &[ru, ri]);
                self.lane_place(p, ru, a, e)
            }
            ExprKind::ExtVectorElementExpr
                if self.o.clang_compat >= 22 && self.ext_elt_lane(e).is_some() =>
            {
                // clang 22: a one-lane access of a vector object is that
                // lane (spec/oven.md "Vectors").
                let lane = self.ext_elt_lane(e).expect("guarded");
                let ast = self.ast();
                let base = ast.exprs.operand(e);
                let p = if ast.exprs.get(e).has(kiln_ast::expr::ExprFlags::ARROW) {
                    self.rv(base)
                } else {
                    self.vec_object(base, lane_base_access(acc))
                };
                let ri = self.reg();
                self.konst(ri, u128::from(lane));
                self.lane_place(p, ri, base, e)
            }
            ExprKind::ArraySubscriptExpr
                if {
                    let [a, b] = self.ast().exprs.operands(e);
                    self.is_vector_expr(a) || self.is_vector_expr(b)
                } =>
            {
                // clang 19 does not evaluate a vector lane access: the
                // generic note at the base (spec/oven.md "Vectors").
                let loc = self.begin(e);
                self.trap_loc(loc, "subexpression not valid in a constant expression".into());
                Place::Mem(self.reg())
            }
            ExprKind::ExtVectorElementExpr => {
                // Nor an ext vector element access of a glvalue vector,
                // silently (a prvalue one is `ext_elt_regs`).
                self.trap(e, "");
                Place::Mem(self.reg())
            }
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
                if decay && self.unsized_member_decay(self.ast().exprs.operand(pe0)) {
                    return Place::Mem(self.reg());
                }
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
                // clang diagnoses the element step at the base's begin
                // (`this->b[n]` at `this`, `(*this).b[n]` at the paren).
                let pc = self.emit_ptr_add(int_aux(w as u8, s), &[r, p, i, ety.0], e);
                let loc = self.begin(pe);
                if let Some(site) = self.o.sites.last_mut().filter(|x| x.pc == pc as u32) {
                    site.loc = loc;
                }
                Place::Mem(r)
            }
            ExprKind::UnaryOperator => {
                let op = self.ast().exprs.unary_op(e);
                let x = self.ast().exprs.operand(e);
                match op {
                    UnaryOp::Deref => {
                        let p = self.rv(x);
                        // clang 22 diagnoses `*null` at the `*`, even when
                        // the lvalue is not accessed (`&*p`).
                        if self.o.clang_compat >= 22 {
                            let r = self.reg();
                            self.emit_at(Op::Field, 2, &[r, p, 0], e);
                            Place::Mem(r)
                        } else {
                            Place::Mem(p)
                        }
                    }
                    UnaryOp::Real | UnaryOp::Imag => self.real_imag_lv(e, acc),
                    UnaryOp::Extension => self.lv(x, acc),
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
                let fold = self.bcp_fold_enter(ExprId::new(n.a()));
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
                self.bcp_fold_exit(fold);
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
            // [expr.reinterpret.cast]/11: the glvalue `reinterpret_cast`
            // designates the operand's object; not a constant expression
            // ([expr.const]/5.15) outside a fold (`Op::Cce`).
            ExprKind::CXXReinterpretCastExpr
                if n.value_category().is_glvalue()
                    && self.ast().exprs.cast_kind(e) == CastKind::LValueBitCast =>
            {
                self.cce_check(e, "reinterpret_cast is not allowed in a constant expression");
                let op = self.ast().exprs.operand(e);
                let sub = match acc {
                    Access::Write => Access::Write,
                    Access::Addr => Access::Addr,
                    Access::Read | Access::Rmw => Access::Addr,
                };
                Place::Mem(self.lv_mem(op, sub))
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
            ExprKind::CXXTypeidExpr => Place::Mem(self.typeid(e)),
            ExprKind::CXXDynamicCastExpr
                if n.value_category().is_glvalue()
                    && self.ast().exprs.cast_kind(e) == CastKind::Dynamic =>
            {
                let r = self.reg();
                self.dynamic_cast(e, r, true);
                Place::Mem(r)
            }
            ExprKind::ExprWithCleanups => {
                let op = self.ast().exprs.operand(e);
                self.open_scope(&[], true);
                let p = self.lv(op, acc);
                self.close_scope();
                p
            }
            ExprKind::CXXDefaultInitExpr if self.default_init_of(e).is_some() => {
                let x = self.default_init_of(e).unwrap_or(e);
                let saved = self.enter_default_arg(e);
                let r = self.lv(x, acc);
                self.srcloc = saved;
                r
            }
            ExprKind::CXXThrowExpr => {
                self.throw_expr(e);
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
        if ast.decls.get(fd).kind() == DeclKind::Var {
            // A static data member named by a class member access: the
            // object expression is evaluated, then discarded
            // ([expr.ref]/2; clang `VisitIgnoredBaseExpression`).
            self.effect(base);
            let g = self.o.global_index(self.b.ast(), fd);
            let r = self.reg();
            self.emit_at(Op::GlobalAddr, 0, &[r, g], e);
            if is_reference_var(self.b.ast(), fd) {
                let q = self.ast().decls.get(fd).ty().unwrap_or(QualType::INT);
                let rt = self.ty(q);
                let a = self.reg();
                self.emit_at(Op::Load, ak::READ, &[a, r, rt.0], e);
                return Place::Mem(a);
            }
            return Place::Mem(r);
        }
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
            TyKind::Record { union, .. } => (union, self.o.types.field_index(rt, fd)),
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
            // At clang 22 `*null` already failed at the `*`, so a null
            // lvalue here (`p[0].m`) fails as `->` does.
            let pass_null = !arrow && self.o.clang_compat < 22;
            self.emit_at(Op::Field, u32::from(pass_null), &[r, p, fi.offset as u32], e);
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
        if self.lanes(ty).is_some() {
            return self.vec_assign(e, ty);
        }
        if let Some(elem) = self.cx_elem(ty) {
            return self.cx_assign(e, elem);
        }
        if self.o.types.wide(ty).is_some() {
            return self.wide_assign(e, ty);
        }
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
        self.volatile_glvalue(a, ak::ASSIGN, e);
        self.store(place, r, ty, ak::ASSIGN, e);
        place
    }

    /// A modification `k` (at `e`) through glvalue `x` of volatile-qualified
    /// type traps, as a read does (`LValueToRValue`; clang
    /// `note_constexpr_access_volatile_type`, [expr.const]/10.8).
    /// An assignment whose left operand names a union member (clang
    /// `HandleUnionActiveMemberChange`) is checked there first, at the left
    /// operand's location.
    fn volatile_glvalue(&mut self, x: ExprId, k: u32, e: ExprId) {
        let q = self.node(x).ty();
        if q.quals_bits() & 2 != 0 {
            let msg = format!(
                "{} volatile-qualified type '{}' is not allowed in a constant expression",
                ak::verb(k),
                Oven::type_name(self.ast(), q)
            );
            let at = if k == ak::ASSIGN && self.names_union_member(x) { x } else { e };
            self.trap(at, &msg);
        }
    }

    /// Whether the member-access path of glvalue `x` (through `.`/`->`,
    /// array subscripts and derived-to-base conversions, but not
    /// parentheses, as clang walks it) names a union member.
    fn names_union_member(&mut self, x: ExprId) -> bool {
        let mut e = x;
        loop {
            let ast = self.ast();
            let n = *ast.exprs.get(e);
            match n.kind() {
                ExprKind::MemberExpr => {
                    let fd = ast.decls.get(ast.exprs.decl(e));
                    if fd.kind() != DeclKind::Field || is_reference_decl(ast, ast.exprs.decl(e)) {
                        return false;
                    }
                    let base = ExprId::new(n.a());
                    let bt = self.ty(ast.exprs.get(base).ty());
                    let rt = match self.kind(bt) {
                        TyKind::Ptr(p) if n.has(kiln_ast::expr::ExprFlags::ARROW) => p,
                        _ => bt,
                    };
                    if matches!(self.kind(rt), TyKind::Record { union: true, .. }) {
                        return true;
                    }
                    e = base;
                }
                ExprKind::ArraySubscriptExpr => e = ast.exprs.operands(e)[0],
                ExprKind::ImplicitCastExpr
                    if matches!(
                        ast.exprs.cast_kind(e),
                        CastKind::NoOp
                            | CastKind::DerivedToBase
                            | CastKind::UncheckedDerivedToBase
                            | CastKind::ArrayToPointerDecay
                    ) =>
                {
                    e = ast.exprs.operand(e);
                }
                _ => return false,
            }
        }
    }

    fn compound_assign(&mut self, e: ExprId) -> Place {
        let ast = self.ast();
        let n = *ast.exprs.get(e);
        let x = *ast.exprs.compound_assign(e);
        let op = ast.exprs.binary_op(e);
        let lhs = ExprId::new(n.a());
        let lty = self.ety(lhs);
        if self.lanes(lty).is_some() {
            return self.vec_compound_assign(e, lty);
        }
        if self.cx_elem(lty).is_some() || self.is_complex_expr(x.rhs) {
            return self.cx_compound_assign(e);
        }
        let ct = self.ty(x.result_ty);
        // `b += p` for a `bool b` (a pointer computation stored into a
        // non-pointer): clang's `ExprConstant` has no rule for it.
        if matches!(self.kind(ct), TyKind::Ptr(_)) && !matches!(self.kind(lty), TyKind::Ptr(_)) {
            self.trap(e, "subexpression not valid in a constant expression");
            return self.lv_rmw(lhs, e);
        }
        if self.o.types.wide(lty).is_some()
            || self.o.types.wide(ct).is_some()
            || self.is_wide_expr(x.rhs)
        {
            return self.wide_compound_assign(e);
        }
        let r = self.rv_copy(x.rhs);
        let place = self.lv_rmw(lhs, e);
        let ty = self.ety(lhs);
        let rty = self.ety(x.rhs);
        self.volatile_glvalue(lhs, ak::COMPOUND, e);
        let old = self.reg();
        self.load(place, ty, ak::COMPOUND, e, old);
        let new = self.reg();
        if let TyKind::Ptr(pointee) = self.kind(ty) {
            let (w, s) = int_of(self.kind(rty)).unwrap_or((64, true));
            let aux = int_aux(w as u8, s) | u32::from(op == BinaryOp::SubAssign) << 9;
            self.emit_ptr_add(aux, &[new, old, r, pointee.0], e);
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
        // An object of an invalid class (diagnosed, [class.mem]; its
        // layout is a placeholder) is never constructed: the evaluation
        // fails without a note (clang `HandleConstructorCall` and
        // `RecordExprEvaluator` on an invalid decl).
        if matches!(
            n.kind(),
            ExprKind::CXXConstructExpr
                | ExprKind::CXXTemporaryObjectExpr
                | ExprKind::InitListExpr
                | ExprKind::ImplicitValueInitExpr
        ) && is_invalid_record(self.ast(), n.ty())
        {
            return self.trap(e, "");
        }
        match n.kind() {
            ExprKind::ParenExpr | ExprKind::ConstantExpr => {
                let op = self.ast().exprs.operand(e);
                return self.init_inner(op, p, ty);
            }
            ExprKind::UnaryOperator if self.ast().exprs.unary_op(e) == UnaryOp::Extension => {
                let op = self.ast().exprs.operand(e);
                return self.init_inner(op, p, ty);
            }
            ExprKind::ChooseExpr | ExprKind::GenericSelectionExpr
                if self.ast().exprs.selected_operand(e).is_some() =>
            {
                let op = self.ast().exprs.selected_operand(e).expect("guarded");
                return self.init_inner(op, p, ty);
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                let op = self.ast().exprs.rewritten_semantic(e);
                return self.init_inner(op, p, ty);
            }
            ExprKind::PseudoObjectExpr => {
                let r = self.bind_pseudo(e, Access::Read);
                self.init_inner(r, p, ty);
                self.unbind_pseudo(e);
                return;
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
            ExprKind::CompoundLiteralExpr if self.o.types.is_aggregate(ty) => {
                let init = self.ast().exprs.compound_literal_init(e);
                return self.init_inner(init, p, ty);
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
                let saved = self.enter_default_arg(e);
                self.init_inner(x, p, ty);
                self.srcloc = saved;
                return;
            }
            ExprKind::ArrayInitLoopExpr => {
                self.array_init_loop(e, p, ty);
                return;
            }
            ExprKind::BinaryOperator if self.ast().exprs.binary_op(e) == BinaryOp::Cmp => {
                self.three_way(e, p, ty);
                return;
            }
            ExprKind::CXXStdInitializerListExpr => {
                self.std_initializer_list(e, p, ty);
                return;
            }
            ExprKind::CXXParenListInitExpr => {
                let ast = self.ast();
                let inits: Vec<ExprId> =
                    ast.lists.iter::<ExprId>(ast.exprs.paren_list_inits(e)).collect();
                let filler = ast.exprs.paren_list_filler(e);
                let union_field = ast.exprs.paren_list_union_field(e);
                self.init_aggr(e, &inits, filler, union_field, p, ty);
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
        if self.vec_op(e) && self.lanes(ty).is_some() {
            return self.vec_init(e, p, ty);
        }
        if self.cx_op(e) && self.cx_elem(ty).is_some() {
            return self.cx_init(e, p, ty);
        }
        if self.o.types.wide(ty).is_some() && self.wide_init(e, p, ty) {
            return;
        }
        if !self.o.types.is_aggregate(ty) {
            let r = self.rv(e);
            self.emit_at(Op::InitStore, 0, &[p, r, ty.0], e);
            return;
        }
        match n.kind() {
            ExprKind::InitListExpr => self.init_list(e, p, ty),
            // [expr.prim.lambda.capture]/15: the closure's members are
            // direct-initialized from the captures, in order.
            ExprKind::LambdaExpr => {
                let ast = self.ast();
                let inits: Vec<ExprId> = ast
                    .exprs
                    .lambda(e)
                    .inits
                    .map(|l| ast.lists.iter::<ExprId>(l).collect())
                    .unwrap_or_default();
                self.init_aggr(e, &inits, None, None, p, ty);
            }
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
                let fold = self.bcp_fold_enter(ExprId::new(n.a()));
                let c = self.rv(ExprId::new(n.a()));
                let [t, f] = self.ast().exprs.cond_arms(e);
                let j = self.jmp_if(c, false);
                self.init(t, p, ty);
                let end = self.jmp();
                self.patch_here(j);
                self.init(f, p, ty);
                self.patch_here(end);
                self.bcp_fold_exit(fold);
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
                        | CastKind::NonAtomicToAtomic
                        | CastKind::AtomicToNonAtomic
                ) =>
            {
                let op = self.ast().exprs.operand(e);
                // `_Atomic(T)` holds its `T` at the same address (layout in
                // "Types"): a class `T` converts to or from it by
                // initializing the `T` (clang's `_Atomic` extension;
                // libc++'s `__cxx_atomic_base_impl(T)`, WP-334). The
                // object initialized or copied is the operand's `T`; the
                // promoted tail of a wider `_Atomic(T)` stays padding.
                let ty = match self.ast().exprs.cast_kind(e) {
                    CastKind::NonAtomicToAtomic => self.ety(op),
                    _ => ty,
                };
                if self.node(op).value_category().is_glvalue() {
                    let src = self.lv_mem(op, Access::Read);
                    self.emit_at(Op::Copy, 1, &[p, src, ty.0], e);
                } else {
                    self.init_inner(op, p, ty);
                }
            }
            ExprKind::StmtExpr => self.stmt_expr_to(e, None, Some((p, ty))),
            _ if n.value_category().is_glvalue() => {
                let src = self.lv_mem(e, Access::Read);
                self.emit_at(Op::Copy, 1, &[p, src, ty.0], e);
            }
            _ => self.unsupported(e),
        }
    }

    /// Records the step of the element of type `ty` (`len` bytes, `local`
    /// bytes into the list `tr` tracks) about to be initialized from `x`,
    /// and tracks `x` itself when it is a list initializing an aggregate
    /// element ([`Oven::ctor_steps`]).
    fn elem_step(
        &mut self,
        tr: Option<ElemTrack>,
        local: u64,
        ty: OTy,
        len: u64,
        x: Option<ExprId>,
    ) {
        let Some(t) = tr else { return };
        let off = t.off + local;
        let (Ok(o), Ok(n)) = (u32::try_from(off), u32::try_from(len)) else { return };
        let kind = self.kind(ty);
        let record = matches!(kind, TyKind::Record { .. });
        let mut cur = crate::CTOR_STEP_ELEM | t.field;
        if record {
            cur |= crate::CTOR_STEP_RECORD;
        }
        let pc = self.o.code.len() as u32;
        self.ctor_steps.push(crate::CtorStep { pc, cur, off: o, len: n });
        self.elem_track = x.and_then(|x| self.tracked_list(x, ty)).map(|list| ElemTrack {
            list,
            field: t.field,
            off,
        });
    }

    /// `x` when it is a list (or parenthesized list) aggregate-initializing
    /// a non-union class or an array of type `ty`.
    pub(crate) fn tracked_list(&self, x: ExprId, ty: OTy) -> Option<ExprId> {
        let ast = self.ast();
        let mut x = x;
        while matches!(
            ast.exprs.get(x).kind(),
            ExprKind::ParenExpr | ExprKind::ConstantExpr | ExprKind::ExprWithCleanups
        ) {
            x = ast.exprs.operand(x);
        }
        let list = matches!(
            ast.exprs.get(x).kind(),
            ExprKind::InitListExpr | ExprKind::CXXParenListInitExpr
        );
        let aggr =
            matches!(self.kind(ty), TyKind::Array { .. } | TyKind::Record { union: false, .. });
        (list && aggr).then_some(x)
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
        let tr = self.elem_track.take_if(|t| t.list == e);
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
                    self.elem_step(tr, i as u64 * es, elem, es, Some(x));
                    self.init(x, q, elem);
                    self.elem_track = None;
                }
                if rest > 0
                    && !zero_fill
                    && let Some(f) = filler
                {
                    // A loop over the remaining elements.
                    self.elem_step(tr, inits.len() as u64 * es, elem, rest * es, None);
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
                    self.emit_ptr_add(int_aux(64, false), &[q, p, k, elem.0], e);
                    self.init(f, q, elem);
                    self.emit(Op::IAdd, int_aux(64, false), &[k, k, one]);
                    let j = self.jmp();
                    self.patch(j, top);
                    self.patch_here(exit);
                }
            }
            TyKind::BoolVec { elem, n } => {
                // One bit per lane (`ty.rs`); the lanes not listed are
                // false.
                if inits.len() as u64 > n || inits.iter().any(|&x| self.is_vector_expr(x)) {
                    return self.unsupported(e);
                }
                if (inits.len() as u64) < n {
                    self.emit_at(Op::ZeroInit, 0, &[p, ty.0], e);
                }
                for (i, &x) in inits.iter().enumerate() {
                    let r = self.rv(x);
                    let q = self.elem_ptr(p, i as u64 / 8, e);
                    let bw = (i % 8) as u32 | 1 << 8;
                    self.emit_at(Op::StoreBits, 1, &[q, r, elem.0, bw], x);
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
                    let x = it.next().copied();
                    self.elem_step(tr, off, bty, self.o.types.size(bty), x);
                    match x {
                        Some(x) => self.init(x, q, bty),
                        None => {
                            self.emit_at(Op::ZeroInit, 0, &[q, bty.0], e);
                        }
                    }
                    self.elem_track = None;
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
                    self.elem_step(tr, fi.offset, fi.ty, self.o.types.size(fi.ty), x.map(|x| x.0));
                    if let Some((x, dflt)) = x {
                        let saved = self.this_override;
                        if dflt {
                            self.this_override = Some(p);
                        }
                        self.init_field(x, p, fi, e);
                        self.elem_track = None;
                        self.this_override = saved;
                    } else {
                        // [dcl.init.aggr]/5.3: copy-initialized from `{}`.
                        let q = self.elem_ptr(p, fi.offset, e);
                        match fi.width {
                            Some(w) if self.o.types.wide(fi.ty).is_some() => {
                                self.wide_field_init(None, q, fi.ty, (fi.bit, w), e);
                            }
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
    /// A `std::initializer_list<E>` over its backing array
    /// ([dcl.init.list]/5): the first field points at the array, the
    /// second holds its length (libstdc++, libc++) or points past its end.
    fn std_initializer_list(&mut self, e: ExprId, p: u32, ty: OTy) {
        let arr = self.ast().exprs.operand(e);
        let aty = self.ety(arr);
        let TyKind::Array { elem, n } = self.kind(aty) else { return self.unsupported(e) };
        let &[f0, f1] = self.o.types.fields(ty) else { return self.unsupported(e) };
        if !matches!(self.kind(f0.ty), TyKind::Ptr(_)) || f0.width.is_some() {
            return self.unsupported(e);
        }
        let a = self.lv_mem(arr, Access::Addr);
        let q0 = self.elem_ptr(p, f0.offset, e);
        self.emit_at(Op::InitStore, 0, &[q0, a, f0.ty.0], e);
        let r = self.reg();
        match self.kind(f1.ty) {
            TyKind::Int { .. } if f1.width.is_none() => self.konst(r, u128::from(n)),
            TyKind::Ptr(_) => {
                let k = self.reg();
                self.konst(k, u128::from(n));
                self.emit_ptr_add(int_aux(64, false), &[r, a, k, elem.0], e);
            }
            _ => return self.unsupported(e),
        }
        let q1 = self.elem_ptr(p, f1.offset, e);
        self.emit_at(Op::InitStore, 0, &[q1, r, f1.ty.0], e);
    }

    fn elem_ptr(&mut self, p: u32, off: u64, e: ExprId) -> u32 {
        if off == 0 {
            return p;
        }
        let r = self.reg();
        self.emit_at(Op::Field, 0, &[r, p, off as u32], e);
        r
    }

    /// [expr.add]/4: clang 22 rejects binding a reference of type `rq`
    /// (in register `r`) to the object past the end (WP-948), for a
    /// variable or a data member (WP-1021); clang 19 and a parameter do
    /// not check. clang notes the initializer's start (`arr[3]`).
    fn ref_chk(&mut self, r: u32, rq: QualType, init: ExprId) {
        if self.o.clang_compat < 22 {
            return;
        }
        let pq = self.b.ast().types.pointee(rq).unwrap_or(rq);
        let ty = self.ty(pq);
        let pc = self.emit(Op::RefChk, 0, &[r, ty.0]);
        let loc = self.begin(init);
        self.o.sites.push(Site { pc: pc as u32, loc, e: Some(init) });
    }

    fn init_field(&mut self, x: ExprId, p: u32, fi: crate::ty::FieldInfo, e: ExprId) {
        let q = self.elem_ptr(p, fi.offset, e);
        if is_reference_decl(self.ast(), fi.decl) {
            let r = self.reg();
            self.bind_ref(x, r);
            // A lambda's init-capture is not checked (clang 22).
            if self.ast().exprs.get(e).kind() != ExprKind::LambdaExpr {
                let rq = self.ast().decls.get(fi.decl).ty().unwrap_or(QualType::INT);
                self.ref_chk(r, rq, x);
            }
            self.emit_at(Op::InitStore, 0, &[q, r, fi.ty.0], x);
            return;
        }
        match fi.width {
            Some(w) if self.o.types.wide(fi.ty).is_some() => {
                self.wide_field_init(Some(x), q, fi.ty, (fi.bit, w), e);
            }
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
        if self.skip_effects.contains(&e) {
            return;
        }
        let m = self.next;
        let n = self.node(e);
        let ty = self.ety(e);
        if self.vec_op(e) && self.lanes(ty).is_some() {
            let _ = self.vec_regs(e);
            self.next = m;
            return;
        }
        match n.kind() {
            ExprKind::CXXThrowExpr => self.throw_expr(e),
            ExprKind::StmtExpr => self.stmt_expr(e, None),
            ExprKind::ParenExpr => {
                let op = self.ast().exprs.operand(e);
                self.effect(op);
            }
            ExprKind::UnaryOperator if self.ast().exprs.unary_op(e) == UnaryOp::Extension => {
                let op = self.ast().exprs.operand(e);
                self.effect(op);
            }
            // An immediate invocation sema left unvalued: one nested in an
            // immediate-escalating function or lambda (P2564; the call is
            // part of the enclosing evaluation, [expr.const]/16), or one
            // evaluated with the enclosing initializer. Evaluated in place,
            // as `rv_inner` does for a value (spec/oven.md "Nested
            // immediate invocations").
            ExprKind::ConstantExpr if self.ast().exprs.constant_value(e).is_none() => {
                let op = self.ast().exprs.operand(e);
                self.effect(op);
            }
            // A `void` immediate invocation sema evaluated on its own: its
            // side effects ended with that evaluation (it is a full
            // constant expression, [expr.const]/16).
            ExprKind::ConstantExpr if self.kind(ty) == TyKind::Void => {}
            ExprKind::ChooseExpr | ExprKind::GenericSelectionExpr
                if self.ast().exprs.selected_operand(e).is_some() =>
            {
                let op = self.ast().exprs.selected_operand(e).expect("guarded");
                self.effect(op);
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                let op = self.ast().exprs.rewritten_semantic(e);
                self.effect(op);
            }
            ExprKind::PseudoObjectExpr => {
                let r = self.bind_pseudo(e, Access::Read);
                self.effect(r);
                self.unbind_pseudo(e);
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
                    _ if self.o.types.wide(ty).is_some() => {
                        let _ = self.materialize(e, ty);
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
            // `void()` (an empty comma fold, [temp.variadic]/10) or a
            // discarded `T()`: no effect.
            ExprKind::CXXScalarValueInitExpr => {}
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
        let r = self.call_inner(e, sret);
        // `std::is_within_lifetime(&u.m)` (WP-418): the call's verdict and
        // its argument's union designators, which the parameter loses
        // (spec/oven.md "Builtins").
        let ast = self.ast();
        let callee = skip_parens(ast, ast.exprs.callee(e));
        let callee = if ast.exprs.get(callee).kind().is_cast() {
            skip_parens(ast, ast.exprs.operand(callee))
        } else {
            callee
        };
        if ast.exprs.get(callee).kind() == ExprKind::DeclRefExpr
            && let [x] = ast.lists.get(ast.exprs.call_args(e))[..]
            && is_std_function(ast, ast.exprs.decl(callee), "is_within_lifetime")
            && let Some(ok) = self.union_designators(ExprId::new(x), Some(r))
        {
            return ok;
        }
        r
    }

    fn call_inner(&mut self, e: ExprId, sret: Option<u32>) -> u32 {
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
        // `p->~T()` on a scalar ([expr.prim.id.dtor]/2, [expr.call]/5):
        // the object expression is evaluated and the object's lifetime
        // ends ([basic.life]/1.4; tracked for `std::allocator` storage,
        // spec/oven.md "Transient allocation").
        if ast.exprs.get(c).kind() == ExprKind::CXXPseudoDestructorExpr {
            let (base, arrow, destroyed) = ast.exprs.pseudo_destructor(c);
            let p = if arrow { self.rv(base) } else { self.lv_mem(base, Access::Addr) };
            let t = self.ty(destroyed);
            let loc = self.begin(base);
            self.emit_site(Op::EndLife, 3, &[p, t.0], c, loc);
            return self.reg();
        }
        let direct = (ast.exprs.get(c).kind() == ExprKind::DeclRefExpr
            && ast.decls.get(ast.exprs.decl(c)).kind().is_function())
        .then(|| ast.exprs.decl(c));
        if let Some(d) = direct
            && let Some(
                b @ (kiln_ast::decl::builtin::IS_CONSTANT_EVALUATED
                | kiln_ast::decl::builtin::KILN_PLAINLY_CONSTANT_EVALUATED),
            ) = ast.decls.builtin_id(d)
        {
            let r = self.reg();
            // aux bit 1: `__kiln_plainly_constant_evaluated` (WP-473).
            let aux = if b == kiln_ast::decl::builtin::IS_CONSTANT_EVALUATED {
                u32::from(self.std_ice)
            } else {
                2
            };
            let pc = self.emit_at(Op::IsConstEval, aux, &[r], e);
            let loc = self.begin(e);
            if let Some(site) = self.o.sites.last_mut().filter(|x| x.pc == pc as u32) {
                site.loc = loc;
            }
            self.uses_cc = true;
            return r;
        }
        // `std::uncaught_exceptions()` declared `constexpr` (P3068R6, a
        // library predating its rollback by P3842R2): the count of this
        // evaluation's uncaught exceptions ([except.uncaught]/1). C++26
        // as published has it non-`constexpr` (libc++ 19 and 22,
        // libstdc++ 14), so such a call is not constant (spec/oven.md
        // "Exceptions").
        if let Some(d) = direct
            && args.is_empty()
            && is_std_function(ast, d, "uncaught_exceptions")
            && ast
                .decls
                .function(ast.latest(d))
                .is_some_and(|f| f.spec.constexpr_kind() != ConstexprKind::None)
        {
            let r = self.reg();
            let loc = self.begin(e);
            self.emit_site(Op::Builtin, crate::kbc::bi::UNCAUGHT, &[r, r, r, r, 0, 0], e, loc);
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
        // clang's standard-library builtins (`isStdBuiltin`; spec/oven.md
        // "Calls"): a `constexpr` `std::move`, `forward`, `forward_like`,
        // `move_if_noexcept`, `as_const`, `addressof` or `__addressof` is
        // its argument's lvalue (or address), evaluated without a call:
        // no frame, no step, no depth, as clang's `VisitCallExpr` does.
        if let Some(d) = direct
            && let [x] = args[..]
            && is_std_builtin(ast, d)
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
            && let Some(bid) = ast.decls.builtin_id(d).or_else(|| ast.decls.gnu_lib_twin(d))
            && let Some(r) = self.builtin_call(e, bid, &args)
        {
            return r;
        }
        // WP-940: a C library function clang's evaluator knows (clang
        // `VisitBuiltinCallExpr`'s `BIstrlen`, `BImemcmp`, `BIstrchr`,
        // `BImemcpy` cases) evaluates as its `__builtin_` twin after a
        // note that makes it no constant expression: a fold continues
        // past the note (`Op::Cce`), so codegen and `__builtin_constant_p`
        // fold `strlen("abc")` as clang does.
        if let Some(d) = direct
            && let Some(bid) = self
                .ast()
                .decls
                .lib_builtin(d)
                .filter(|&b| kiln_ast::decl::builtin::lib_evaluated(b))
        {
            let loc = self.begin(e);
            // clang `getQuotedName` of the library builtin.
            let name = kiln_ast::decl::builtin::info(bid)
                .and_then(|i| i.name.strip_prefix("__builtin_"))
                .unwrap_or_default();
            let msg =
                format!("non-constexpr function '{name}' cannot be used in a constant expression");
            let t = self.trap_index(loc, msg);
            self.emit_loc(Op::Cce, cce::CHECK, &[0, 0, t], loc);
            if let Some(r) = self.builtin_call(e, bid, &args) {
                return r;
            }
        }
        // Reflection intrinsics (WP-267 spike; spec/oven.md "Reflection
        // runtime"): recognized by the bridge, run natively.
        if let Some(d) = direct
            && let Some(op) = self.b.meta_intrinsic(d)
        {
            return self.meta_call(e, (op, d), &args, sret);
        }
        // `::operator new`/`::operator delete` (spec/oven.md "Transient
        // allocation": constant only in `std::allocator<T>::allocate` /
        // `deallocate`).
        if let Some(d) = direct
            && let Some(alloc) = new_delete::global_alloc_form(self.ast(), d)
        {
            return self.global_alloc_call(e, alloc, &args);
        }
        // An operator call whose callee is a member function: the first
        // argument is the object ([over.match.oper]/2, clang's
        // `EmitCXXOperatorMemberCallExpr`); for an explicit object member
        // function it is the (converted) first argument, as in a plain
        // call ([over.call.func]/3).
        let ast = self.ast();
        if let Some(md) = direct
            && ast.exprs.get(e).kind() == ExprKind::CXXOperatorCallExpr
            && ast.decls.get(md).kind() == DeclKind::CXXMethod
            && !ast.decls.function(md).is_some_and(|f| f.spec.has_explicit_object())
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
    is_std_function(ast, d, "is_constant_evaluated")
}

/// Whether function `d` is one of the standard functions clang evaluates
/// as a builtin (`isStdBuiltin`, `Builtins.td` `CxxLibBuiltin`; the shape
/// test of codegen's `is_std_builtin`): `std::move`, `move_if_noexcept`,
/// `forward`, `forward_like`, `as_const`, `addressof` or `__addressof`,
/// declared `constexpr`, with one non-variadic reference parameter and a
/// reference or pointer return to the same type up to qualifiers.
fn is_std_builtin(ast: &kiln_ast::Ast, d: DeclId) -> bool {
    const NAMES: [&str; 7] = [
        "move",
        "move_if_noexcept",
        "forward",
        "forward_like",
        "as_const",
        "addressof",
        "__addressof",
    ];
    if ast.decls.get(d).kind() != DeclKind::Function
        || !NAMES.iter().any(|n| is_std_function(ast, d, n))
        || !ast
            .decls
            .function(ast.latest(d))
            .is_some_and(|f| f.spec.constexpr_kind() != ConstexprKind::None)
    {
        return false;
    }
    let Some(fty) = ast.decls.get(d).ty() else { return false };
    let Some(p) = ast.types.proto(fty) else { return false };
    let [param] = ast.types.params(p) else { return false };
    if p.variadic() || !ast.types.is_reference(*param) {
        return false;
    }
    let ret = p.ret();
    (ast.types.is_reference(ret) || ast.types.is_pointer(ret))
        && match (ast.types.pointee(*param), ast.types.pointee(ret)) {
            (Some(a), Some(b)) => ast.types.same_unqualified(a, b),
            _ => false,
        }
}

/// Whether function `d` is named `name` in namespace `std` at
/// translation-unit scope, possibly through inline namespaces (clang's
/// `isInStdNamespace`).
pub(crate) fn is_std_function(ast: &kiln_ast::Ast, d: DeclId, name: &str) -> bool {
    let decls = &ast.decls;
    let named = |d: DeclId, n: &str| {
        decls.get(d).name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == n)
    };
    if !named(d, name) {
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
/// Whether the initializer of non-reference variable `d` extends a
/// temporary to `d`'s lifetime ([class.temporary]/8, CWG3043: only an
/// enumerating expansion statement's element does). Its temporaries
/// sit under an `ExprWithCleanups`; any other initializer is not walked.
fn extends_temporaries(ast: &kiln_ast::Ast, d: DeclId) -> bool {
    let Some(init) = ast.decls.var(d).and_then(|v| v.init) else { return false };
    if ast.exprs.get(init).kind() != ExprKind::ExprWithCleanups {
        return false;
    }
    let mut work = vec![ast.exprs.operand(init)];
    while let Some(e) = work.pop() {
        match ast.exprs.get(e).kind() {
            ExprKind::MaterializeTemporaryExpr
                if ast.exprs.temporary_storage(e) == StorageDuration::Automatic =>
            {
                return true;
            }
            ExprKind::LambdaExpr | ExprKind::StmtExpr | ExprKind::ExprWithCleanups => {}
            _ => ast.exprs.children_into(e, &ast.lists, &mut work),
        }
    }
    false
}

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
    ast.default_arg_expr(e)
}

impl Compiler<'_> {
    /// Enters default argument (or default member initializer use) `e`:
    /// the source-location builtins in it
    /// take the values of this use unless an enclosing default argument
    /// already set them. Returns the scope to restore.
    fn enter_default_arg(&mut self, e: ExprId) -> Option<ExprId> {
        let saved = self.srcloc;
        if saved.is_none() && self.ast().exprs.source_loc_values(e).is_some() {
            self.srcloc = Some(e);
        }
        saved
    }

    /// A call of reflection intrinsic `op` (`bi::META`;
    /// spec/reflection.md §3): the arguments are evaluated in order into
    /// consecutive registers (a scalar or `info` argument by value, a
    /// class or array argument by address), whose first index and count
    /// are the builtin's raw `a`/`b` operands. A class-typed result is
    /// built in `sret` (or a temporary) through its recipe, and a
    /// `reflection_range` argument is read as a span (§3.5); a call that
    /// can throw `std::meta::exception` is followed by its throw (§8.2).
    /// Operand 3 is the call's `MetaSite` index + 1 (0: none).
    fn meta_call(
        &mut self,
        e: ExprId,
        (op, callee): (crate::MetaOp, DeclId),
        args: &[ExprId],
        sret: Option<u32>,
    ) -> u32 {
        use crate::meta::materialize::{ExcSite, MetaSite, Shape, recipe};
        if args.len() > crate::MAX_META_ARGS {
            self.unsupported(e);
            return self.reg();
        }
        let mut site =
            MetaSite { e, callee, result: None, ranges: [None; crate::MAX_META_ARGS], exc: None };
        // The recipes, matched once per call site.
        let rty = self.ety(e);
        let is_class = |c: &Self, t: OTy| matches!(c.kind(t), TyKind::Record { .. });
        let mut bad = None;
        if is_class(self, rty) && !self.node(e).value_category().is_glvalue() {
            let q = self.ast().exprs.get(e).ty();
            match self.b.meta_shape(op, None, q) {
                Ok(Some(sh)) => match recipe(&self.o.types, rty, sh) {
                    Ok(rc) => site.result = Some(rc),
                    Err(why) => bad = Some((e, q, why)),
                },
                Ok(None) => bad = Some((e, q, "is not a result kiln materializes".into())),
                Err(why) => bad = Some((e, q, why)),
            }
        }
        for (i, &x) in args.iter().enumerate() {
            // Every argument is asked: a range parameter may be given a
            // built-in array, or a value that is no range at all.
            let xt = self.ety(x);
            if bad.is_some() {
                continue;
            }
            let q = self.ast().exprs.get(x).ty();
            match self.b.meta_shape(op, Some(i), q) {
                Ok(Some(sh)) => match recipe(&self.o.types, xt, sh) {
                    Ok(rc) => {
                        site.ranges[i] = Some(rc);
                        if sh == Shape::Range {
                            self.o.meta.info_ty = self.o.meta.info_ty.or(match rc {
                                crate::meta::materialize::Recipe::Vector { elem, .. }
                                | crate::meta::materialize::Recipe::Inline { elem, .. } => {
                                    Some(elem).filter(|&t| self.kind(t) == TyKind::Reflection)
                                }
                                crate::meta::materialize::Recipe::Span { ptr, .. } => {
                                    match self.kind(ptr.ty) {
                                        TyKind::Ptr(t) => Some(t),
                                        _ => None,
                                    }
                                }
                                _ => None,
                            });
                        }
                    }
                    Err(why) => bad = Some((x, q, why)),
                },
                Ok(None) => {}
                Err(why) => bad = Some((x, q, why)),
            }
        }
        if let Some((at, q, why)) = bad {
            let t = Oven::type_name(self.ast(), q);
            let msg = format!("'{t}' cannot be used by '{}': it {why}", op.display());
            self.trap(at, &msg);
            return self.reg();
        }
        let n = args.len() as u32;
        let base = self.next;
        self.next += n;
        self.max = self.max.max(self.next);
        let mut t1 = 0;
        // `reflect_object(T&)`'s and `reflect_function(T&)`'s reference
        // parameter is given its argument's address (WP-479; a class or
        // array argument is passed by address anyway).
        let value_op = matches!(op, crate::MetaOp::Value(_));
        let by_addr = matches!(
            op,
            crate::MetaOp::Value(crate::ValueOp::ReflectObject | crate::ValueOp::ReflectFunction)
        );
        let refs: Vec<bool> = if by_addr {
            let ast = self.ast();
            let fty = ast.decls.get(callee).ty();
            fty.and_then(|q| ast.types.proto(q))
                .map(|p| ast.types.params(p).iter().map(|&t| ast.types.is_reference(t)).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        for (i, &x) in args.iter().enumerate() {
            let slot = base + i as u32;
            let xt = self.ety(x);
            let by_ref = refs.get(i).copied().unwrap_or(false)
                && self.ast().exprs.get(x).value_category().is_glvalue();
            if by_ref && !matches!(self.kind(xt), TyKind::Record { .. } | TyKind::Array { .. }) {
                let r = match self.lv(x, Access::Addr) {
                    Place::Mem(p) => p,
                    _ => {
                        // A register local has no address: it is no
                        // object of static storage duration either.
                        let r = self.reg();
                        self.konst(r, 0);
                        r
                    }
                };
                self.emit(Op::Mov, 0, &[slot, r]);
                continue;
            }
            match self.kind(xt) {
                TyKind::Record { .. } | TyKind::Array { .. } => {
                    let glvalue = self.ast().exprs.get(x).value_category().is_glvalue();
                    let r = if glvalue {
                        self.lv_mem(x, Access::Addr)
                    } else {
                        self.materialize(x, xt)
                    };
                    self.emit(Op::Mov, 0, &[slot, r]);
                }
                // A value op's `const T&` parameter bound to a scalar
                // glvalue (`reflect_constant(x)`) is given its value.
                _ if value_op && self.ast().exprs.get(x).value_category().is_glvalue() => {
                    if let TyKind::Ptr(ct) = self.kind(xt) {
                        t1 = ct.0;
                    }
                    let p = self.lv(x, Access::Read);
                    self.load(p, xt, ak::READ, x, slot);
                }
                TyKind::Ptr(ct) => {
                    t1 = ct.0;
                    self.rv_into(x, slot);
                }
                _ => self.rv_into(x, slot),
            }
        }
        let dst = self.reg();
        if site.result.is_some() {
            match sret {
                Some(p) => {
                    self.emit(Op::Mov, 0, &[dst, p]);
                }
                None => self.alloc(dst, rty, Origin::Temp(e), false, e),
            }
        }
        let exc = if self.b.cxx26() { self.b.meta_exception() } else { None };
        let exc = exc.and_then(|x| {
            let (wt, at) = (self.ty(x.what), self.ty(x.at));
            let what = recipe(&self.o.types, wt, Shape::Str).ok()?;
            let at_rc = recipe(&self.o.types, at, Shape::SrcLoc).ok()?;
            Some((x, wt, at, what, at_rc))
        });
        if let Some((_, _, _, what, at)) = exc {
            let flag = self.reg();
            self.konst(flag, 0);
            site.exc = Some(ExcSite { flag, what, at });
        }
        let idx = if value_op
            || site.result.is_some()
            || site.exc.is_some()
            || site.ranges.iter().any(Option::is_some)
        {
            self.o.meta.sites.push(site);
            self.o.meta.sites.len() as u32
        } else {
            0
        };
        let t2 = rty.0;
        let loc = self.begin(e);
        self.emit_site(
            Op::Builtin,
            crate::kbc::bi::META | op.code() << 8,
            &[dst, base, n, idx, t1, t2],
            e,
            loc,
        );
        if let (Some((x, wt, at, _, _)), Some(xs)) = (exc, site.exc) {
            self.meta_throw_block(e, (x, wt, at), xs.flag, idx, loc);
        }
        dst
    }

    /// The throw of the `std::meta::exception` a failing intrinsic call
    /// `e` (site `idx`) raises, run when its `flag` register is set
    /// (spec/reflection.md §8.2): the exception object is constructed by
    /// `ctor` from the message, `^^fn` and location `bi::META_EXC` fills
    /// in, then thrown as by a throw-expression.
    fn meta_throw_block(
        &mut self,
        e: ExprId,
        (x, wt, at): (crate::ExcCtor, OTy, OTy),
        flag: u32,
        idx: u32,
        loc: Loc,
    ) {
        use crate::kbc::NO_DTOR;
        use crate::vm::exc::EXC_FREE;
        let skip = self.jmp_if(flag, false);
        let m = self.next;
        let ty = self.ty(x.class);
        let obj = self.reg();
        let oi = self.o.push_origin(Origin::Heap(e, 0));
        self.emit_site(Op::New, 0, &[obj, ty.0, obj, oi, 0], e, loc);
        self.emit_loc(Op::PushCleanup, 4, &[obj, EXC_FREE, ty.0], loc);
        let base = self.next;
        for _ in 0..4 {
            self.reg();
        }
        self.emit(Op::Mov, 0, &[base, obj]);
        self.alloc(base + 1, wt, Origin::Temp(e), false, e);
        self.alloc(base + 3, at, Origin::Temp(e), false, e);
        self.emit_site(
            Op::Builtin,
            crate::kbc::bi::META_EXC,
            &[base + 2, base + 1, base + 3, idx, 0, 0],
            e,
            loc,
        );
        let f = self.o.func_index(self.b.ast(), x.ctor);
        self.emit_at(Op::Call, 0, &[f, base], e);
        let dtor = self.dtor_of(ty).unwrap_or(NO_DTOR);
        self.emit_site(Op::Throw, 0, &[obj, ty.0, dtor], e, loc);
        self.next = m;
        self.patch_here(skip);
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

/// Whether `q` is a variable length array, or an array of them (GNU
/// extension; clang `VariableArrayType`).
pub(crate) fn is_vla(ast: &kiln_ast::Ast, q: QualType) -> bool {
    let mut q = q;
    loop {
        match ast.types.canonical_view(q) {
            TypeView::VariableArray { .. } => return true,
            TypeView::ConstantArray(a) => q = a.elem,
            _ => return false,
        }
    }
}

/// Whether `q` (or its array element type) is a class whose definition
/// is invalid ([class.mem]): its layout is clang's size-1 placeholder.
fn is_invalid_record(ast: &kiln_ast::Ast, mut q: QualType) -> bool {
    while let Some(e) = ast.types.element(q) {
        q = e;
    }
    ast.types
        .as_record(q)
        .and_then(|r| ast.definition(r))
        .is_some_and(|d| ast.decls.get(d).has(kiln_ast::decl::DeclFlags::INVALID))
}
