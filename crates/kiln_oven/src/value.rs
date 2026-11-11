//! Freezing results into the `ConstStore`, thawing stored constants into
//! static memory, and resolving globals (namespace-scope variables and
//! string literals) on first use.

use kiln_ast::consts::{
    ByteImage, ByteRange, ConstValue, IntBits, NullKind, PtrBase, Reloc, Scalar, UnionMember,
};
use kiln_ast::decl::{DeclKind, VarEvalState};
use kiln_ast::expr::ExprKind;
use kiln_ast::strings::StringEncoding;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType, StringLitId};
use kiln_source::Loc;

use crate::compile::{GKind, GState, RetKind, is_local_var};
use crate::failure::{EvalFailure, FailKind, Note};
use crate::mem::{Origin, Ptr, UNBACKED, flags, fn_ptr};
use crate::ty::{OTy, TyKind};
use crate::vm::{Counters, value_len};
use crate::{Oven, SemaBridge};

/// Cap on nested global initializations evaluated on demand (a deeper
/// chain reads the variable as unknown).
const MAX_NESTING: u32 = 512;

/// Reusable buffers of the freezer (no allocation per evaluation once
/// warm).
#[derive(Debug, Default)]
pub(crate) struct Scratch {
    bytes: Vec<u8>,
    cover: Vec<u8>,
    relocs: Vec<Reloc>,
    padding: Vec<ByteRange>,
    unions: Vec<UnionMember>,
}

impl Scratch {
    /// Heap bytes held (see `Oven::footprint`).
    pub(crate) fn footprint(&self) -> usize {
        self.bytes.capacity()
            + self.cover.capacity()
            + self.relocs.capacity() * size_of::<Reloc>()
            + self.padding.capacity() * size_of::<ByteRange>()
            + self.unions.capacity() * size_of::<UnionMember>()
    }
}

/// How a variable reads in constant expressions.
pub(crate) struct VarInfo {
    /// Usable in constant expressions ([expr.const]/4): constexpr, or a
    /// const-qualified integral or enumeration type.
    pub usable: bool,
    pub init: Option<ExprId>,
    pub state: VarEvalState,
    pub ty: QualType,
    pub is_const: bool,
    /// Not usable, but readable by a lax fold ([`crate::Oven::fold_rvalue`],
    /// clang `EvaluateAsRValue`): `const` non-volatile with an initializer
    /// (clang `findCompleteObject` only notes such a read).
    pub lax: bool,
}

pub(crate) fn var_info(ast: &Ast, d: DeclId) -> VarInfo {
    let mut info = VarInfo {
        usable: false,
        init: None,
        state: VarEvalState::NotEvaluated,
        ty: ast.decls.get(d).ty().unwrap_or(QualType::INT),
        is_const: false,
        lax: false,
    };
    // A template parameter object ([temp.param]/8): a `const` object
    // whose value is the template argument's.
    if ast.decls.get(d).kind() == DeclKind::TemplateParamObject {
        if let Some(cv) = ast.templates.param_object_value(d) {
            info.state = VarEvalState::Value(cv);
            info.usable = true;
            info.is_const = true;
        }
        return info;
    }
    // A parameter outside its call: never usable, whatever its type (its
    // value is the caller's; a default argument is not an initializer).
    if ast.decls.get(d).kind() == DeclKind::ParmVar {
        return info;
    }
    let mut constexpr = false;
    for r in ast.redecls_of(d) {
        if let Some(v) = ast.decls.var(r) {
            constexpr |= v.spec.is_constexpr();
            if info.init.is_none() && v.init.is_some() {
                info.init = v.init;
                info.state = v.eval.state();
                info.ty = ast.decls.get(r).ty().unwrap_or(info.ty);
            }
        }
    }
    let c = ast.types.canonical(info.ty);
    info.is_const = c.has_const_bit();
    let integral = ast.types.is_integral(c) || ast.types.as_enum(c).is_some();
    // Potentially-constant ([expr.const]/3): constexpr, a reference, or
    // const-qualified integral or enumeration type.
    info.usable = constexpr || ast.types.is_reference(c) || (info.is_const && integral);
    let q = ast.types.effective_quals(info.ty);
    // A namespace-scope array compound literal of `const` type is read
    // through its initializer (clang `handleLValueToRValueConversion`
    // evaluates the literal's initializer; a non-`const` one is not
    // constant; WP-466).
    if ast.decls.compound_literal_of(d).is_some() {
        info.usable = q.is_const() && !q.is_volatile();
    }
    // [expr.const]/10.9: a volatile object is never read, even through a
    // glvalue whose type drops the qualifier (`const_cast`), so a volatile
    // variable's object is external (WP-948; clang `findCompleteObject`).
    if q.is_volatile() {
        info.usable = false;
    }
    info.lax = !info.usable
        && q.is_const()
        && !q.is_volatile()
        && (info.init.is_some() || matches!(info.state, VarEvalState::Value(_)));
    info
}

/// Why a value cannot be frozen.
pub(crate) enum Freeze {
    Fault(FailKind),
    Notes(Vec<Note>),
}

/// The automatic variable (a non-`static`, non-`thread_local` local or a
/// parameter) whose object constant `v` points to or refers to, directly
/// or through a subobject or a constant object, if any: such a value is
/// a constant only within its variable's function (P2686R5
/// [expr.const.init]/2, "constexpr-referenceable"), so it is not
/// constexpr-representable elsewhere and has no constant form in IR.
/// A temporary whose lifetime an automatic variable extends is such an
/// object too; its `LifetimeExtendedTemporary` is returned (it lives in
/// the extending variable's function).
pub fn automatic_referent(ast: &Ast, v: ConstValueId) -> Option<DeclId> {
    fn walk(ast: &Ast, v: ConstValueId, depth: u32) -> Option<DeclId> {
        if depth > 64 {
            return None;
        }
        match ast.consts.scalar(v) {
            Some(Scalar::Pointer { base: PtrBase::Decl(d), .. }) => {
                let tls =
                    ast.decls.var(d).is_some_and(|x| x.spec.tls() != kiln_ast::decl::TlsKind::None);
                (is_local_var(ast, d) && !tls).then_some(d)
            }
            Some(Scalar::Pointer { base: PtrBase::Const(c), .. }) => walk(ast, c, depth + 1),
            Some(Scalar::Pointer { base: PtrBase::Temporary(t), .. }) => {
                let l = ast.decls.lifetime_extended(t)?;
                if is_local_var(ast, l.extending) {
                    return Some(t);
                }
                walk(ast, l.value?, depth + 1)
            }
            Some(_) => None,
            None => ast.consts.relocs(v).iter().find_map(|r| walk(ast, r.value, depth + 1)),
        }
    }
    walk(ast, v, 0)
}

impl Oven {
    /// A top-level evaluation with registers from `fb`: `object` is the
    /// variable initialized.
    pub(crate) fn eval_inner(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        object: Option<DeclId>,
        fb: u32,
    ) -> Result<ConstValueId, EvalFailure> {
        self.run.object = object;
        let pre = object.and_then(|d| self.place_init_object(b, d));
        let r = self.eval_at(b, e, object, fb);
        if let Some(g) = pre {
            self.globals[g as usize].state = GState::Unresolved;
        }
        r
    }

    /// The object of non-local variable `d` while its initializer runs:
    /// its global, if unresolved, is resolved to its placeholder, and
    /// `Some` of the global the caller resets afterwards. The static
    /// space it takes counts in `static_bytes` (WP-663: the fast path of
    /// [`Oven::is_constant_initialized`] places it as well).
    pub(crate) fn place_init_object(&mut self, b: &mut dyn SemaBridge, d: DeclId) -> Option<u32> {
        let ast = b.ast();
        if is_local_var(ast, d) || ast.decls.var(d).is_none() {
            return None;
        }
        let g = self.global_index(ast, d);
        if self.globals[g as usize].state != GState::Unresolved {
            return None;
        }
        let q = ast.decls.get(d).ty().unwrap_or(QualType::INT);
        let ty = self.types.intern(b, q);
        let p = self.placeholder(g, ty, ast_first(b, d));
        self.globals[g as usize].state = GState::Resolved(p);
        Some(g)
    }

    /// Makes global `g`, the variable whose initializer the running thunk
    /// evaluates, designate that thunk's result object at `p` until
    /// [`Oven::unbind_object`] (`Op::BindObject`, WP-1100): during its own
    /// initialization a variable's name designates the object under
    /// construction ([basic.life]/1, [class.cdtor]; clang's
    /// `EvaluatingDecl`), so its initialized members can be read.
    pub(crate) fn bind_object(&mut self, g: u32, p: u64) {
        self.unbind_object();
        let prev = std::mem::replace(&mut self.globals[g as usize].state, GState::Bound(p));
        self.bound_obj = Some((g, prev));
    }

    /// Ends the binding [`Oven::bind_object`] made, restoring the global's
    /// previous state: `(global, object)` of it, if any.
    pub(crate) fn unbind_object(&mut self) -> Option<(u32, u64)> {
        let (g, prev) = self.bound_obj.take()?;
        let st = &mut self.globals[g as usize].state;
        let GState::Bound(p) = *st else { return None };
        *st = prev;
        Some((g, p))
    }

    /// Runs `f`, a separate evaluation started while another one's
    /// variable may be bound to its object under construction: that
    /// binding is lifted for `f` (another initializer naming the variable
    /// reads its placeholder, as clang reads the variable outside its
    /// lifetime) and made again after it.
    pub(crate) fn unbound<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let outer = self.unbind_object();
        let r = f(self);
        if let Some((g, p)) = outer {
            self.bind_object(g, p);
        }
        r
    }

    /// The static storage of global `g` while its initializer runs (reused
    /// by later evaluations of the same variable).
    fn placeholder(&mut self, g: u32, ty: OTy, d: DeclId) -> u64 {
        let info = *self.types.info(ty);
        let fl =
            flags::CONST | flags::RESOLVING | if info.has_union { flags::HAS_UNION } else { 0 };
        let slot = self.globals[g as usize].slot;
        if slot != 0 {
            let p = Ptr::decode(slot);
            let a = self.mem.alloc_of_mut(p);
            if u64::from(a.size) == info.size && a.ty == ty {
                a.flags = fl;
                a.origin = Origin::Global(d);
                let (base, size) = (u64::from(a.base), info.size);
                self.mem.stat.set_init(base, size, false);
                self.mem.stat.set_ptr(base, size, false);
                return slot;
            }
        }
        let p = self.mem.alloc_static(ty, info.size, fl, Origin::Global(d));
        self.globals[g as usize].slot = p;
        p
    }

    /// Compiles and runs `e` with its frame at `fb` and freezes the result.
    pub(crate) fn eval_at(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        object: Option<DeclId>,
        fb: u32,
    ) -> Result<ConstValueId, EvalFailure> {
        let void_ok = std::mem::take(&mut self.void_next);
        let addr = std::mem::take(&mut self.addr_next);
        let unchecked = std::mem::take(&mut self.unchecked_next);
        let f = self.compile_thunk(b, e, object, addr);
        self.let_pending.clear();
        let run = self.exec(b, f, fb);
        // The variable's name designated its result object while the
        // initializer ran (`Op::BindObject`); from here on, its
        // placeholder again.
        self.unbind_object();
        run?;
        let v = self.regs[fb as usize];
        // clang's `CheckConstantExpression` location: the variable for an
        // initializer (`EvaluateAsInitializer`), else the expression.
        let loc = match object {
            Some(d) => b.ast().decls.get(d).loc(),
            None => expr_loc(b.ast(), e),
        };
        let r = match self.funcs[f as usize].ret {
            // A `void` immediate invocation ([expr.const]/16): `true`
            // stands for its value, as for [`Self::eval_call`]; consumers
            // of a `void` `ConstantExpr` never read it (spec/oven.md
            // "Nested immediate invocations").
            RetKind::Void if void_ok => {
                let ty = self.types.intern(b, QualType::BOOL);
                self.freeze_scalar(b, ty, 1, loc)
            }
            RetKind::Void => {
                Err(Freeze::Notes(vec![Note::new(loc, "expression of type 'void' has no value")]))
            }
            RetKind::Scalar(ty) => self.freeze_scalar(b, ty, v, loc),
            // A wide `_BitInt` result is a scalar in the store (`IntBits`),
            // so that a fold reads it as an integer (WP-465).
            RetKind::Sret(ty) if self.types.wide(ty).is_some() => {
                self.freeze_object(b, v as u64, loc)
            }
            RetKind::Sret(ty) => self.freeze_aggregate(b, ty, v as u64, loc),
        };
        // [expr.const]/5.20: every allocation is deallocated within the
        // evaluation (checked after the result, as clang). Codegen's folds
        // outside a constant context only note it
        // ([`crate::Oven::set_fold_leaks`]).
        let leaks_ok = self.fold_leaks && self.run.fold && !self.run.const_context;
        let r = r.and_then(|cv| match self.leak_notes(b) {
            Some(n) if !leaks_ok => Err(Freeze::Notes(n)),
            _ => Ok(cv),
        });
        let cv = r.map_err(|f| match f {
            Freeze::Fault(k) => EvalFailure::silent(k),
            Freeze::Notes(n) if self.run.silent || unchecked => {
                drop(n);
                EvalFailure::silent(FailKind::NotConstant)
            }
            Freeze::Notes(notes) => EvalFailure { kind: FailKind::NotConstant, notes },
        })?;
        // [dcl.constexpr]/6: a `constexpr` variable has constant
        // destruction ([expr.const]/7), checked on the object just built.
        if let (Some(d), RetKind::Sret(ty)) = (object, self.funcs[f as usize].ret)
            && !self.run.silent
            && b.ast().decls.var(d).is_some_and(|x| x.spec.is_constexpr())
            && let Some(df) = self.compile_destroy(b, d, ty)
        {
            self.regs[fb as usize] = v;
            self.exec(b, df, fb)
                .map_err(|e| EvalFailure { kind: FailKind::Destruction, notes: e.notes })?;
        }
        self.freeze_lets(b, loc)?;
        Ok(cv)
    }

    /// Runs the message job of [`Oven::evaluate_chars`] with its frame at
    /// `fb`: the characters are left in `msg_chars`; the value is the
    /// size.
    pub(crate) fn eval_chars(
        &mut self,
        b: &mut dyn SemaBridge,
        size: ExprId,
        data: ExprId,
        fb: u32,
    ) -> Result<ConstValueId, EvalFailure> {
        self.run.object = None;
        self.msg_chars.clear();
        let f = self.compile_chars(b, size, data);
        self.let_pending.clear();
        self.exec(b, f, fb)?;
        let v = self.regs[fb as usize];
        let loc = expr_loc(b.ast(), size);
        let RetKind::Scalar(ty) = self.funcs[f as usize].ret else {
            return Err(EvalFailure::silent(FailKind::NotConstant));
        };
        self.freeze_scalar(b, ty, v, loc).map_err(|f| match f {
            Freeze::Fault(k) => EvalFailure::silent(k),
            Freeze::Notes(notes) => EvalFailure { kind: FailKind::NotConstant, notes },
        })
    }

    /// Runs the job of [`Oven::fold_object_size`] with its frame at `fb`:
    /// the size, a `size_t`.
    pub(crate) fn eval_object_size(
        &mut self,
        b: &mut dyn SemaBridge,
        p: ExprId,
        ty: u32,
        fb: u32,
    ) -> Result<ConstValueId, EvalFailure> {
        self.run.object = None;
        let f = self.compile_object_size(b, p, ty);
        self.let_pending.clear();
        self.exec(b, f, fb)?;
        let v = self.regs[fb as usize];
        let RetKind::Scalar(rt) = self.funcs[f as usize].ret else {
            return Err(EvalFailure::silent(FailKind::NotConstant));
        };
        let loc = expr_loc(b.ast(), p);
        self.freeze_scalar(b, rt, v, loc).map_err(|_| EvalFailure::silent(FailKind::NotConstant))
    }

    /// Runs the job of [`Oven::evaluate_with_params`] with its frame at
    /// `fb`: the condition's value.
    pub(crate) fn eval_subst(
        &mut self,
        b: &mut dyn SemaBridge,
        cond: Option<ExprId>,
        at: ExprId,
        fb: u32,
    ) -> Result<ConstValueId, EvalFailure> {
        self.run.object = None;
        let binds = std::mem::take(&mut self.subst_binds);
        let f = self.compile_subst(b, cond, &binds);
        self.subst_binds = binds;
        self.let_pending.clear();
        self.exec(b, f, fb)?;
        let v = self.regs[fb as usize];
        let RetKind::Scalar(ty) = self.funcs[f as usize].ret else {
            return Err(EvalFailure::silent(FailKind::NotConstant));
        };
        let loc = expr_loc(b.ast(), at);
        let r = self.freeze_scalar(b, ty, v, loc).map_err(|f| match f {
            Freeze::Fault(k) => EvalFailure::silent(k),
            Freeze::Notes(notes) => EvalFailure { kind: FailKind::NotConstant, notes },
        });
        match self.leak_notes(b) {
            None => r,
            Some(_) => Err(EvalFailure::silent(FailKind::NotConstant)),
        }
    }

    /// Runs a call of function `f` without arguments at `at`
    /// ([`crate::Oven::evaluate_call`]): its result, frozen.
    pub(crate) fn eval_call(
        &mut self,
        b: &mut dyn SemaBridge,
        f: DeclId,
        at: Loc,
        fb: u32,
    ) -> Result<ConstValueId, EvalFailure> {
        self.run.object = None;
        let Some(stub) = self.compile_call(b, f, at) else {
            return Err(EvalFailure::silent(FailKind::NotConstant));
        };
        self.let_pending.clear();
        self.exec(b, stub, fb)?;
        // A `void` call has no value: `true` stands for it.
        let (ty, v) = match self.funcs[stub as usize].ret {
            RetKind::Scalar(ty) => (ty, self.regs[fb as usize]),
            _ => (self.types.intern(b, QualType::BOOL), 1),
        };
        let r = self.freeze_scalar(b, ty, v, at).map_err(|f| match f {
            Freeze::Fault(k) => EvalFailure::silent(k),
            Freeze::Notes(notes) => EvalFailure { kind: FailKind::NotConstant, notes },
        });
        match self.leak_notes(b) {
            None => r,
            Some(_) => Err(EvalFailure::silent(FailKind::NotConstant)),
        }
    }

    /// Runs the destruction of variable `d` (`Oven::has_constant_destruction`):
    /// whether it succeeds without leaking.
    pub(crate) fn destroy_var(
        &mut self,
        b: &mut dyn SemaBridge,
        d: DeclId,
        value: Option<ConstValueId>,
    ) -> bool {
        let ast = b.ast();
        let Some(q) = ast.decls.get(d).ty() else { return false };
        let is_const = ast.types.effective_quals(q).is_const();
        let ty = self.types.intern(b, q);
        let Some(df) = self.compile_destroy(b, d, ty) else { return true };
        let size = self.types.size(ty);
        let p = if is_const {
            // Its lifetime is treated as begun in the evaluation, except
            // for `mutable` members (clang `lifetimeStartedInEvaluation`).
            // A writable static object (`Origin::Global` is read-only).
            let fl = self.static_flags(ty);
            let p = self.mem.alloc_static(ty, size, fl, Origin::Var(d));
            if let Some(cv) = value {
                self.thaw_into(b, cv, p, 1);
            }
            p
        } else {
            self.external(b, ty, d)
        };
        self.regs[0] = u128::from(p);
        let ok = self.exec(b, df, 0).is_ok() && self.leak_notes(b).is_none();
        self.mem.free_static(p, &self.types);
        ok
    }

    /// Whether static temporary `t` is extended by the variable being
    /// initialized.
    fn extends_object(&self, ast: &Ast, t: DeclId) -> bool {
        let Some(l) = ast.decls.lifetime_extended(t) else { return false };
        self.run.object.is_some_and(|o| ast.first(o) == ast.first(l.extending))
    }

    /// clang `CheckLValueConstantExpression`: a lifetime-extended
    /// temporary whose type is destructed ([basic.life]/1, a non-trivial
    /// destructor of the type, an element or a member) cannot be part of
    /// a constant result: the destruction would have to run at program
    /// exit ([class.temporary]/6, [expr.const]/14). clang's
    /// `note_constexpr_unsupported_temporary_nontrivial_dtor`, at the
    /// `MaterializeTemporaryExpr`.
    fn check_let_destruction(&self, b: &mut dyn SemaBridge, t: DeclId) -> Result<(), Freeze> {
        let ast = b.ast();
        let mte = let_mte(ast, t);
        let ty = mte.map(|e| ast.exprs.get(e).ty()).or_else(|| ast.decls.get(t).ty());
        let Some(ty) = ty.filter(|&q| ast.needs_destruction(q)) else { return Ok(()) };
        if self.run.silent {
            return Err(Freeze::Fault(FailKind::NotConstant));
        }
        let loc = mte.map_or_else(|| ast.decls.get(t).loc(), |e| ast.exprs.get(e).loc());
        Err(Freeze::Notes(vec![Note::new(
            loc,
            format!(
                "non-trivial destruction of lifetime-extended temporary with type '{}' used in \
                 the result of a constant expression is not yet supported",
                Self::type_name(ast, ty)
            ),
        )]))
    }

    /// Freezes the static temporaries the result points to (and those
    /// their values point to) into their `LetData.value`s.
    fn freeze_lets(&mut self, b: &mut dyn SemaBridge, loc: Loc) -> Result<(), EvalFailure> {
        let mut i = 0;
        let mut out = Vec::new();
        while i < self.let_pending.len() {
            let (t, p) = self.let_pending[i];
            i += 1;
            let r = self.freeze_object(b, p, loc).map_err(|f| match f {
                Freeze::Fault(k) => EvalFailure::silent(k),
                Freeze::Notes(notes) => EvalFailure { kind: FailKind::NotConstant, notes },
            })?;
            out.push((t, r));
        }
        self.let_pending.clear();
        for (t, v) in out {
            b.set_temporary_value(t, v);
        }
        Ok(())
    }

    /// The value of the complete object at `p`.
    pub(crate) fn freeze_object(
        &mut self,
        b: &mut dyn SemaBridge,
        p: u64,
        loc: Loc,
    ) -> Result<ConstValueId, Freeze> {
        let Some(a) = self.alloc_any(Ptr::decode(p)).copied() else {
            return Err(Freeze::Fault(FailKind::NotConstant));
        };
        if let Some((width, signed)) = self.types.wide(a.ty) {
            // A wide `_BitInt` is a scalar in the store (`IntBits`).
            let Ok(mut w) = self.wide_load(p, a.ty, crate::kbc::ak::READ) else {
                return Err(self.uninit(b, None, a.ty, loc));
            };
            crate::bigint::canon(&mut w, width, false);
            w.truncate(IntBits::words_for(width));
            let Some(bits) = IntBits::from_words(width, &w) else {
                return Err(Freeze::Fault(FailKind::NotConstant));
            };
            let s = Scalar::Int { ty: self.types.info(a.ty).q, signed, bits };
            return b.consts().push_scalar(s).map_err(|_| Freeze::Fault(FailKind::MemoryLimit));
        }
        if self.types.is_aggregate(a.ty) {
            return self.freeze_aggregate(b, a.ty, p, loc);
        }
        let Ok(v) = self.load(p, a.ty, crate::kbc::ak::READ) else {
            return Err(self.uninit(b, None, a.ty, loc));
        };
        self.freeze_scalar(b, a.ty, v, loc)
    }

    /// clang's `note_constexpr_memory_leak` for the allocations of the
    /// running context still live: the first made, and how many others.
    pub(crate) fn leak_notes(&self, b: &dyn SemaBridge) -> Option<Vec<Note>> {
        if self.mem.heap_count == 0 {
            return None;
        }
        let mut first: Option<(u32, ExprId)> = None;
        let mut n = 0u64;
        for (_, a) in self.mem.heap_live_of(self.mem.ctx) {
            if let Origin::Heap(e, k) = a.origin {
                n += 1;
                if first.is_none_or(|(f, _)| k < f) {
                    first = Some((k, e));
                }
            }
        }
        let (_, e) = first?;
        if self.run.silent {
            return Some(Vec::new());
        }
        let others = match n - 1 {
            0 => String::new(),
            1 => " (along with 1 other memory leak)".into(),
            k => format!(" (along with {k} other memory leaks)"),
        };
        let loc = b.ast().exprs.get(e).loc();
        Some(vec![Note::new(loc, format!("allocation performed here was not deallocated{others}"))])
    }

    pub(crate) fn freeze_scalar(
        &mut self,
        b: &mut dyn SemaBridge,
        ty: OTy,
        v: u128,
        loc: Loc,
    ) -> Result<ConstValueId, Freeze> {
        if let TyKind::MemPtr { .. } = self.types.kind(ty) {
            if let Some(m) = self.mp.get(v).filter(|e| e.func).map(|e| e.member) {
                self.check_not_immediate(b, m, false, loc)?;
            }
            return self.mp_freeze(b, ty, v).map_err(|_| Freeze::Fault(FailKind::MemoryLimit));
        }
        let s = self.scalar_of(b, ty, v, loc)?;
        b.consts().push_scalar(s).map_err(|_| Freeze::Fault(FailKind::MemoryLimit))
    }

    /// The store form of a scalar of type `ty` with register value `v`.
    fn scalar_of(
        &mut self,
        b: &mut dyn SemaBridge,
        ty: OTy,
        v: u128,
        loc: Loc,
    ) -> Result<Scalar<'static>, Freeze> {
        let info = *self.types.info(ty);
        Ok(match info.kind {
            TyKind::Bool => Scalar::Bool { ty: info.q, value: v != 0 },
            TyKind::Int { width, signed } => {
                let w = u32::from(width);
                let m = if w >= 128 { v } else { v & ((1u128 << w) - 1) };
                Scalar::Int { ty: info.q, signed, bits: IntBits::from_u128(w, m) }
            }
            TyKind::Float(kind) => {
                let bits = value_len(info.size, info.kind) * 8;
                let m = if bits >= 128 { v } else { v & ((1u128 << bits) - 1) };
                Scalar::Float { ty: info.q, kind, bits: m }
            }
            TyKind::NullPtr => Scalar::Null { ty: info.q, kind: NullKind::Pointer },
            TyKind::Reflection => match kiln_ast::ids::ReflectionId::from_bits(v) {
                Some(id) => Scalar::Reflection { ty: info.q, id },
                None => {
                    return Err(Freeze::Notes(vec![Note::new(
                        loc,
                        "kiln: value is not a reflection the compiler produced",
                    )]));
                }
            },
            TyKind::Ptr(pointee) => {
                let desig = self.void_designated(pointee, (v >> 64) as u32);
                self.ptr_of(b, info.q, pointee, v as u64, desig, loc)?
            }
            _ => {
                return Err(Freeze::Notes(vec![Note::new(
                    loc,
                    "kiln: value of this type cannot be represented as a constant",
                )]));
            }
        })
    }

    /// [expr.const]/22: a constant expression's value does not point
    /// to (or refer to, or name as a member) an immediate function
    /// (clang `note_consteval_address_accessible`).
    fn check_not_immediate(
        &self,
        b: &dyn SemaBridge,
        d: DeclId,
        reference: bool,
        loc: Loc,
    ) -> Result<(), Freeze> {
        let ast = b.ast();
        let immediate = ast.decls.function(d).is_some_and(|f| {
            f.spec.constexpr_kind() == kiln_ast::decl::ConstexprKind::Consteval
                || f.spec.escalated()
        });
        if !immediate {
            return Ok(());
        }
        if self.run.silent {
            return Err(Freeze::Fault(FailKind::NotConstant));
        }
        let what = if reference { "reference" } else { "pointer" };
        Err(Freeze::Notes(vec![
            Note::new(
                loc,
                format!("{what} to a consteval declaration is not a constant expression"),
            ),
            Note::new(ast.decls.get(d).loc(), "declared here"),
        ]))
    }

    /// [dcl.ref]/5, [expr.const]/22: a reference constant expression
    /// refers to an object, never one past the end of one (clang
    /// `CheckLValueConstantExpression`, `note_constexpr_past_end`; both
    /// clang 19 and 22, WP-1021). A pointer may (an extension).
    fn check_ref_past_end(
        &self,
        b: &dyn SemaBridge,
        q: QualType,
        pointee: OTy,
        p: Ptr,
        a: &crate::mem::Alloc,
        loc: Loc,
    ) -> Result<(), Freeze> {
        let offset = u64::from(p.off);
        let size = u64::from(a.size);
        // An object of unknown or zero size (`extern int a[];`) has no
        // end the offset could reach.
        if !b.ast().types.is_reference(q) || !(p.past || (size != 0 && offset >= size)) {
            return Ok(());
        }
        if self.run.silent {
            return Err(Freeze::Fault(FailKind::NotConstant));
        }
        let ast = b.ast();
        // A subobject designator (`&s.a + 1`, `a[2]`) against the
        // complete object (`&x + 1`).
        let what = if pointee != a.ty || offset < size { "subobject of " } else { "" };
        let (name, at, here) = match a.origin {
            Origin::Global(d) | Origin::External(d) | Origin::Var(d) => {
                let mut name = String::new();
                kiln_ast::dump::AstNames::new(ast).write_name(d, &mut name);
                (format!("'{name}'"), ast.decls.get(d).loc(), "declared here")
            }
            Origin::Let(t) => (
                "temporary".to_owned(),
                let_mte(ast, t).map_or(Loc::INVALID, |e| ast.exprs.get(e).loc()),
                "temporary created here",
            ),
            _ => return Err(Freeze::Fault(FailKind::NotConstant)),
        };
        let mut notes = vec![Note::new(
            loc,
            format!(
                "dereferenced pointer past the end of {what}{name} is not a constant expression"
            ),
        )];
        if at != Loc::INVALID {
            notes.push(Note::new(at, here));
        }
        Err(Freeze::Notes(notes))
    }

    /// Whether automatic variable `d` is constexpr-referenceable from the
    /// initializing declaration of the variable being initialized
    /// (P2686R5 [expr.const.init]/2, C++26): that variable is automatic
    /// too ([expr.const.init]/4: one with static or thread storage
    /// duration must be representable at namespace scope) and the
    /// smallest enclosing scope of both that is not a block scope is the
    /// same function parameter scope (a lambda's body is its own).
    /// Deliberately narrowed to a variable declared `constexpr` (WP-467):
    /// the rule would also make `auto& e = local;` constant-initialized,
    /// which no compiler does yet, so that case waits for an oracle
    /// (spec/sema.md §4.9).
    fn referenceable_from_object(&self, b: &dyn SemaBridge, d: DeclId) -> bool {
        let Some(v) = self.run.object else { return false };
        let ast = b.ast();
        let ctx = ast.decls.get(d).ctx();
        b.cxx26()
            && is_local_var(ast, v)
            && ast.decls.var(v).is_some_and(|x| x.spec.is_constexpr())
            && ctx.is_some_and(|c| ast.decls.get(c).kind().is_function())
            && ast.decls.get(v).ctx() == ctx
    }

    /// The designated type a frozen `void *` keeps ([`Scalar::Pointer`]
    /// `designated`): the type its `kbc::VOID_TAG` tag bits `tag`
    /// recorded, for a pointer to `void`.
    fn void_designated(&self, pointee: OTy, tag: u32) -> Option<QualType> {
        let t = OTy(tag.checked_sub(1)?);
        let named = self.types.kind(pointee) == TyKind::Void && !self.types.is_synthetic_array(t);
        named.then(|| self.types.info(t).q)
    }

    /// The frozen value of a pointer that points at nothing
    /// (`mem::NOWHERE`): the result of a cast only a fold performs
    /// (`Op::Cce`), as clang's lvalue with an invalid designator, which its
    /// `ConstantEmitter` emits as the base's address plus the offset (C23
    /// 6.6p9: an address constant may be formed with casts). An integer
    /// cast to a pointer (frame slot 0) is [`PtrBase::Absolute`]; a pointer
    /// into a static object keeps that object (the allocation now in its
    /// slot: static slots are not reused within an evaluation) and its
    /// offset, with the cast's type. Anything else has no constant
    /// address. spec/oven.md "Pointers that point at nothing".
    #[cold]
    fn nowhere_of(
        &mut self,
        b: &mut dyn SemaBridge,
        q: QualType,
        pointee: OTy,
        p: Ptr,
        loc: Loc,
    ) -> Result<Scalar<'static>, Freeze> {
        if p.slot == 0 {
            let offset = u64::from(p.off);
            let past = false;
            return Ok(Scalar::Pointer { ty: q, base: PtrBase::Absolute, offset, designated: None, past });
        }
        let Some(a) = self.mem.static_in_slot(p.slot).filter(|a| a.live) else {
            return Err(Freeze::Fault(FailKind::NotConstant));
        };
        let raw = Ptr { generation: a.generation, ..p }.encode();
        self.ptr_of(b, q, pointee, raw, None, loc)
    }

    fn ptr_of(
        &mut self,
        b: &mut dyn SemaBridge,
        q: QualType,
        pointee: OTy,
        raw: u64,
        designated: Option<QualType>,
        loc: Loc,
    ) -> Result<Scalar<'static>, Freeze> {
        if raw == 0 {
            return Ok(Scalar::Null { ty: q, kind: NullKind::Pointer });
        }
        let p = Ptr::decode(raw);
        if Ptr::is_nowhere(raw) && !p.is_fn() {
            return self.nowhere_of(b, q, pointee, p, loc);
        }
        if p.is_fn() {
            return match self.funcs.get(p.off as usize).and_then(|f| f.decl) {
                Some(d) => {
                    let r = b.ast().types.is_reference(q);
                    self.check_not_immediate(b, d, r, loc)?;
                    Ok(Scalar::Pointer {
                        ty: q,
                        base: PtrBase::Function(d),
                        offset: 0,
                        designated: None,
                        past: false,
                    })
                }
                None => Err(Freeze::Fault(FailKind::NotConstant)),
            };
        }
        let offset = u64::from(p.off);
        let Some(a) = self.alloc_any(p).copied() else {
            if p.is_heap() && !self.run.silent {
                // A deleted allocation whose slot was reused.
                let what = if offset != 0 || p.past { "subobject of " } else { "" };
                return Err(Freeze::Notes(vec![Note::new(
                    loc,
                    format!("pointer to {what}heap-allocated object is not a constant expression"),
                )]));
            }
            return Err(Freeze::Notes(vec![Note::new(
                loc,
                "pointer to an object outside its lifetime is not a constant expression",
            )]));
        };
        // One past the end of a subobject whose end is not its complete
        // object's (`&s.a + 1`, `&a[0].base + 1`): the frozen value keeps
        // the bit, which the offset alone cannot tell from a pointer to
        // the next subobject (canonical form: [`Scalar::Pointer`] `past`;
        // WP-948).
        let past = p.past && offset < u64::from(a.size);
        // A variable with automatic storage duration reached through the
        // globals path (outside its frame, or read by a lax fold) has no
        // constant address ([expr.const]/14).
        let automatic = matches!(a.origin, Origin::Global(d) | Origin::External(d) | Origin::Unknown(d)
            if is_local_var(b.ast(), d));
        // P2686R5 [expr.const.init]/2: an automatic object is
        // constexpr-referenceable from the initializing declaration of an
        // automatic variable of the same function (its frozen address is
        // that of the object in the variable's own invocation).
        let referenceable = automatic
            && matches!(a.origin, Origin::Global(d) | Origin::External(d)
                if self.referenceable_from_object(b, d));
        // The object of unknown value of a namespace-scope reference
        // (P2280; clang `CheckLValueConstantExpression`
        // `allowConstexprUnknown`).
        if let Origin::Unknown(d) = a.origin
            && !automatic
        {
            if self.run.silent {
                return Err(Freeze::Fault(FailKind::NotConstant));
            }
            let ast = b.ast();
            let mut name = String::new();
            kiln_ast::dump::AstNames::new(ast).write_name(d, &mut name);
            return Err(Freeze::Notes(vec![
                Note::new(loc, format!("initializer of '{name}' is not a constant expression")),
                Note::new(ast.decls.get(d).loc(), "declared here"),
            ]));
        }
        // An object with thread storage duration has no constant address
        // either ([expr.const]/14 permits static storage duration only;
        // clang `CheckLValueConstantExpression` rejects it without a note).
        if let Origin::Global(d) | Origin::External(d) = a.origin
            && b.ast().decls.var(d).is_some_and(|v| v.spec.tls() != kiln_ast::decl::TlsKind::None)
        {
            return Err(Freeze::Fault(FailKind::NotConstant));
        }
        if p.is_static() && (!automatic || referenceable) {
            self.check_ref_past_end(b, q, pointee, p, &a, loc)?;
            let base = match a.origin {
                Origin::Global(d) | Origin::External(d) => PtrBase::Decl(d),
                Origin::Str(lit, Some(origin)) => PtrBase::StringLit { lit, origin },
                Origin::Const(c) => PtrBase::Const(c),
                Origin::Let(t) => PtrBase::Temporary(t),
                // `&typeid(T)` ([expr.const]/14: the type_info object has
                // static storage duration), e.g. libc++'s
                // `static constexpr __policy` in <functional> (WP-176).
                Origin::TypeInfo(t) => PtrBase::TypeInfo(t),
                // `constexpr void *q = &&l;`: a label address (WP-444).
                Origin::Label(e) => {
                    PtrBase::Label { label: b.ast().exprs.addr_label(e), origin: e }
                }
                _ => return Err(Freeze::Fault(FailKind::NotConstant)),
            };
            return Ok(Scalar::Pointer { ty: q, base, offset, designated, past });
        }
        // A temporary extended by the variable being initialized
        // ([class.temporary]/6): its value is frozen with the result.
        if let Origin::Let(t) = a.origin
            && a.live
            && self.extends_object(b.ast(), t)
        {
            // clang `CheckLValueConstantExpression`: a lifetime-extended
            // temporary of destructed type cannot be part of a constant
            // result, so its variable is dynamically initialized and the
            // destructor registered at exit ([basic.start.term]; WP-231).
            self.check_let_destruction(b, t)?;
            self.check_ref_past_end(b, q, pointee, p, &a, loc)?;
            if !self.let_pending.iter().any(|&(x, _)| x == t) {
                self.let_pending.push((t, Ptr::with_off(raw, 0, false)));
            }
            return Ok(Scalar::Pointer {
                ty: q,
                base: PtrBase::Temporary(t),
                offset,
                designated,
                past,
            });
        }
        if let (Origin::Var(d), Some(o)) = (a.origin, self.run.object)
            && d == o
            && a.live
        {
            let ast = b.ast();
            // Nor does the variable itself when it is `thread_local`
            // (`S *self = this;` in an NSDMI, a `std::string`'s SSO
            // pointer): the variable is dynamically initialized, once per
            // thread, instead of from a `.tdata` image holding the main
            // thread's address.
            if ast.decls.var(d).is_some_and(|v| v.spec.tls() != kiln_ast::decl::TlsKind::None) {
                return Err(Freeze::Fault(FailKind::NotConstant));
            }
            return Ok(Scalar::Pointer {
                ty: q,
                base: PtrBase::Decl(ast.first(d)),
                offset,
                designated,
                past,
            });
        }
        if self.run.silent {
            return Err(Freeze::Fault(FailKind::NotConstant));
        }
        let ast = b.ast();
        let sub = offset != 0 || p.past || pointee != a.ty;
        let what = if sub { "subobject of " } else { "" };
        let kind = if ast.types.is_reference(q) { "reference" } else { "pointer" };
        let mut notes = Vec::with_capacity(2);
        match a.origin {
            Origin::Var(d) | Origin::Global(d) | Origin::External(d) | Origin::Unknown(d) => {
                let mut name = String::new();
                kiln_ast::dump::AstNames::new(ast).write_name(d, &mut name);
                // clang `note_constexpr_non_global`.
                let kind = if ast.types.is_reference(q) { "reference" } else { "pointer" };
                notes.push(Note::new(
                    loc,
                    format!("{kind} to {what}'{name}' is not a constant expression"),
                ));
                notes.push(Note::new(ast.decls.get(d).loc(), "declared here"));
            }
            Origin::Temp(e) => {
                notes.push(Note::new(
                    loc,
                    format!("{kind} to {what}temporary is not a constant expression"),
                ));
                notes.push(Note::new(ast.exprs.get(e).loc(), "temporary created here"));
            }
            Origin::Let(t) => {
                notes.push(Note::new(
                    loc,
                    format!("{kind} to {what}temporary is not a constant expression"),
                ));
                if let Some(e) = let_mte(ast, t) {
                    notes.push(Note::new(ast.exprs.get(e).loc(), "temporary created here"));
                }
            }
            // clang's note_constexpr_dynamic_alloc, before the leak check;
            // the allocation is noted while it is live.
            Origin::Heap(e, _) => {
                notes.push(Note::new(
                    loc,
                    format!("{kind} to {what}heap-allocated object is not a constant expression"),
                ));
                if a.live {
                    notes.push(Note::new(ast.exprs.get(e).loc(), "heap allocation performed here"));
                }
            }
            _ => notes.push(Note::new(
                loc,
                "pointer to an object outside its lifetime is not a constant expression",
            )),
        }
        Err(Freeze::Notes(notes))
    }

    pub(crate) fn freeze_aggregate(
        &mut self,
        b: &mut dyn SemaBridge,
        ty: OTy,
        raw: u64,
        loc: Loc,
    ) -> Result<ConstValueId, Freeze> {
        let size = self.types.size(ty) as usize;
        let mut s = std::mem::take(&mut self.scratch);
        s.bytes.clear();
        s.bytes.resize(size, 0);
        s.cover.clear();
        s.cover.resize(size, 0);
        s.relocs.clear();
        s.padding.clear();
        s.unions.clear();
        let r = self.walk(b, &mut s, ty, raw, 0, None, loc);
        let out = r.and_then(|()| {
            s.relocs.sort_unstable_by_key(|r| r.at);
            s.unions.sort_unstable_by_key(|u| (u.at, u.field));
            let mut i = 0;
            while i < size {
                if s.cover[i] == 0 {
                    let start = i;
                    while i < size && s.cover[i] == 0 {
                        s.bytes[i] = 0;
                        i += 1;
                    }
                    s.padding.push(ByteRange { start: start as u32, len: (i - start) as u32 });
                } else {
                    i += 1;
                }
            }
            let img = ByteImage {
                ty: self.types.info(ty).q,
                bytes: &s.bytes,
                relocs: &s.relocs,
                padding: &s.padding,
                unions: &s.unions,
            };
            b.consts().push_aggregate(img).map_err(|_| Freeze::Fault(FailKind::NotConstant))
        });
        self.scratch = s;
        out
    }

    fn uninit(&self, b: &dyn SemaBridge, field: Option<DeclId>, ty: OTy, loc: Loc) -> Freeze {
        if self.run.silent {
            return Freeze::Fault(FailKind::NotConstant);
        }
        let ast = b.ast();
        let named = field.filter(|&f| !ast.decls.get(f).name().is_empty());
        if let Some(f) = named {
            let mut name = String::new();
            kiln_ast::dump::AstNames::new(ast).write_name(f, &mut name);
            Freeze::Notes(vec![
                Note::new(loc, format!("subobject '{name}' is not initialized")),
                Note::new(ast.decls.get(f).loc(), "subobject declared here"),
            ])
        } else {
            let t = Self::type_name(ast, self.types.info(ty).q);
            Freeze::Notes(vec![Note::new(
                loc,
                format!("subobject of type '{t}' is not initialized"),
            )])
        }
    }

    pub(crate) fn type_name(ast: &Ast, q: QualType) -> String {
        kiln_ast::types::print_type(&ast.types, ast, q, kiln_ast::types::PrintOptions::SUGARED)
    }

    /// Copies the object of type `t` at `raw` into the image at byte `at`.
    fn walk(
        &mut self,
        b: &mut dyn SemaBridge,
        s: &mut Scratch,
        t: OTy,
        raw: u64,
        at: usize,
        field: Option<DeclId>,
        loc: Loc,
    ) -> Result<(), Freeze> {
        let info = *self.types.info(t);
        let p = Ptr::decode(raw);
        let Some(a) = self.alloc_any(p).copied() else {
            return Err(Freeze::Fault(FailKind::NotConstant));
        };
        if a.base == UNBACKED {
            return Err(Freeze::Fault(FailKind::NotConstant));
        }
        let st = p.space();
        let abs = u64::from(a.base) + u64::from(p.off);
        match info.kind {
            TyKind::Bool | TyKind::Int { .. } | TyKind::WideInt { .. } | TyKind::Float(_) => {
                let len = value_len(info.size, info.kind);
                let sp = self.mem.space(st);
                if !sp.is_init(abs, len) {
                    return Err(self.uninit(b, field, t, loc));
                }
                let (a0, a1) = (abs as usize, (abs + len) as usize);
                s.bytes[at..at + len as usize].copy_from_slice(&sp.bytes[a0..a1]);
                s.cover[at..at + len as usize].fill(1);
            }
            TyKind::Ptr(_) | TyKind::NullPtr | TyKind::Reflection => {
                let sp = self.mem.space(st);
                if !sp.is_init(abs, info.size) {
                    return Err(self.uninit(b, field, t, loc));
                }
                // A `void *` keeps the designated type of its tag.
                let v = sp.read(abs, info.size) | u128::from(self.mem.void_tag(st, abs)) << 64;
                let sc = self.scalar_of(b, t, v, loc)?;
                let id =
                    b.consts().push_scalar(sc).map_err(|_| Freeze::Fault(FailKind::NotConstant))?;
                s.relocs.push(Reloc { at: at as u32, value: id });
                s.cover[at..at + info.size as usize].fill(1);
            }
            TyKind::MemPtr { .. } => {
                let sp = self.mem.space(st);
                if !sp.is_init(abs, info.size) {
                    return Err(self.uninit(b, field, t, loc));
                }
                let v = sp.read(abs, info.size);
                let id =
                    self.mp_freeze(b, t, v).map_err(|_| Freeze::Fault(FailKind::NotConstant))?;
                s.relocs.push(Reloc { at: at as u32, value: id });
                s.cover[at..at + info.size as usize].fill(1);
            }
            TyKind::BoolVec { n, .. } => {
                // The lane bytes; the bits and bytes past the last lane
                // are padding (zero in the image).
                let len = n.div_ceil(8);
                let sp = self.mem.space(st);
                if !sp.is_init(abs, len) {
                    return Err(self.uninit(b, field, t, loc));
                }
                let (a0, a1) = (abs as usize, (abs + len) as usize);
                s.bytes[at..at + len as usize].copy_from_slice(&sp.bytes[a0..a1]);
                if n % 8 != 0 {
                    s.bytes[at + len as usize - 1] &= (1u8 << (n % 8)) - 1;
                }
                s.cover[at..at + len as usize].fill(1);
            }
            TyKind::Array { elem, n } => {
                let e = *self.types.info(elem);
                let es = e.size as usize;
                if matches!(e.kind, TyKind::Int { .. } | TyKind::Bool) {
                    let len = e.size * n;
                    let sp = self.mem.space(st);
                    if sp.is_init(abs, len) {
                        let (a0, a1) = (abs as usize, (abs + len) as usize);
                        s.bytes[at..at + len as usize].copy_from_slice(&sp.bytes[a0..a1]);
                        s.cover[at..at + len as usize].fill(1);
                        return Ok(());
                    }
                }
                for i in 0..n as usize {
                    let r = Ptr::with_off(raw, u64::from(p.off) + (i * es) as u64, false);
                    self.walk(b, s, elem, r, at + i * es, field, loc)?;
                }
            }
            TyKind::Record { union, first, count, decl, .. } => {
                let (first, count) = (first as usize, count as usize);
                if !union && !self.mem.unconstructed.is_empty() {
                    self.unconstructed_base(b, raw, t, decl, first..first + count)?;
                }
                if union {
                    let key = Ptr::union_key(raw, u64::from(p.off), t);
                    let Some(&i) = self.mem.unions.get(&key) else { return Ok(()) };
                    let f = self.types.fields[first + i as usize];
                    s.unions.push(UnionMember { at: at as u32, field: f.decl });
                    if self.cxx_year >= 2026
                        && f.width.is_none()
                        && matches!(self.types.kind(f.ty), TyKind::Array { .. })
                    {
                        let fr = Ptr::with_off(raw, u64::from(p.off) + f.offset, false);
                        let fa = at + f.offset as usize;
                        return self.walk_elements(b, s, f.ty, fr, fa, Some(f.decl), loc);
                    }
                    return self.walk_field(b, s, raw, at, f, loc);
                }
                for i in first..first + count {
                    let f = self.types.fields[i];
                    self.walk_field(b, s, raw, at, f, loc)?;
                }
            }
            TyKind::Void | TyKind::Func | TyKind::Opaque => {
                if info.size != 0 {
                    return Err(Freeze::Fault(FailKind::NotConstant));
                }
            }
        }
        Ok(())
    }

    /// The active array member of a union, `t` at `raw` (C++26, P3726R2
    /// [expr.const]/2): an element outside its lifetime is an inactive
    /// union elemental subobject, not a constituent value, so it is left
    /// out of the image (uncovered) instead of failing as uninitialized.
    /// An element is outside its lifetime when it holds no initialized
    /// byte (spec/oven.md "C++26 union lifetime"); an element of an
    /// element array is one too. Only this path pays the test.
    #[allow(clippy::too_many_arguments)]
    fn walk_elements(
        &mut self,
        b: &mut dyn SemaBridge,
        s: &mut Scratch,
        t: OTy,
        raw: u64,
        at: usize,
        field: Option<DeclId>,
        loc: Loc,
    ) -> Result<(), Freeze> {
        let TyKind::Array { elem, n } = self.types.kind(t) else {
            return self.walk(b, s, t, raw, at, field, loc);
        };
        let p = Ptr::decode(raw);
        let Some(a) = self.alloc_any(p).copied().filter(|a| a.base != UNBACKED) else {
            return Err(Freeze::Fault(FailKind::NotConstant));
        };
        let abs = u64::from(a.base) + u64::from(p.off);
        let es = self.types.size(elem);
        if self.mem.space(p.space()).is_init(abs, es * n) {
            return self.walk(b, s, t, raw, at, field, loc);
        }
        for i in 0..n {
            if es != 0 && !self.mem.space(p.space()).any_init(abs + i * es, es) {
                continue;
            }
            let r = Ptr::with_off(raw, u64::from(p.off) + i * es, false);
            self.walk_elements(b, s, elem, r, at + (i * es) as usize, field, loc)?;
        }
        Ok(())
    }

    /// clang `CheckEvaluationResult` of a base subobject without a value:
    /// a direct base of the class `t` (definition `rdef`, fields `range`)
    /// at `raw` whose constructor never ran, its bytes outside their
    /// lifetime (`Memory::set_unconstructed`; an empty base by identity,
    /// WP-401), is "constructor of base class 'B' is not called" at the
    /// base-specifier (WP-390); the first such base as written.
    fn unconstructed_base(
        &self,
        b: &dyn SemaBridge,
        raw: u64,
        t: OTy,
        rdef: DeclId,
        range: std::ops::Range<usize>,
    ) -> Result<(), Freeze> {
        let off = u64::from(Ptr::decode(raw).off);
        let mut dead: Vec<OTy> = Vec::new();
        for i in range {
            let f = self.types.fields[i];
            if !f.base {
                break;
            }
            let n = self.types.copy_size(f.ty);
            if n != 0 && self.mem.ended(Ptr::with_off(raw, off + f.offset, false), n) {
                dead.push(f.ty);
            }
        }
        for &(bt, o) in self.types.empty_bases(t) {
            if self.mem.empty_unconstructed(Ptr::with_off(raw, off + o, false), bt) {
                dead.push(bt);
            }
        }
        if dead.is_empty() {
            return Ok(());
        }
        if self.run.silent {
            return Err(Freeze::Fault(FailKind::NotConstant));
        }
        let ast = b.ast();
        let canon: Vec<_> =
            dead.iter().map(|&d| ast.types.canonical(self.types.info(d).q).ty()).collect();
        let spec = ast
            .decls
            .bases(rdef, &ast.lists)
            .filter(|s| !s.is_virtual)
            .find(|s| canon.contains(&ast.types.canonical(s.ty).ty()));
        let (loc, i) = match spec {
            Some(s) => {
                let c = ast.types.canonical(s.ty).ty();
                (s.loc, canon.iter().position(|&d| d == c).unwrap_or(0))
            }
            None => (ast.decls.get(rdef).loc(), 0),
        };
        let q = self.types.info(dead[i]).q;
        let name = Self::type_name(ast, q);
        Err(Freeze::Notes(vec![Note::new(
            loc,
            format!("constructor of base class '{name}' is not called"),
        )]))
    }

    fn walk_field(
        &mut self,
        b: &mut dyn SemaBridge,
        s: &mut Scratch,
        raw: u64,
        at: usize,
        f: crate::ty::FieldInfo,
        loc: Loc,
    ) -> Result<(), Freeze> {
        let p = Ptr::decode(raw);
        let fr = Ptr::with_off(raw, u64::from(p.off) + f.offset, false);
        let Some(width) = f.width else {
            // clang notes an uninitialized base by its type, a member by
            // its name.
            let named = (!f.base).then_some(f.decl);
            return self.walk(b, s, f.ty, fr, at + f.offset as usize, named, loc);
        };
        if width == 0 || b.ast().decls.get(f.decl).name().is_empty() {
            return Ok(());
        }
        let n = u64::from((f.bit + width).div_ceil(8));
        let Some(a) = self.alloc_any(p).copied().filter(|a| a.base != UNBACKED) else {
            return Err(Freeze::Fault(FailKind::NotConstant));
        };
        let abs = u64::from(a.base) + u64::from(p.off) + f.offset;
        let sp = self.mem.space(p.space());
        if !sp.is_init(abs, n) {
            return Err(self.uninit(b, Some(f.decl), f.ty, loc));
        }
        // The bits `[bit, bit + width)` of the bytes from `abs`, a byte at
        // a time (a wide `_BitInt` bit-field spans more than 16 bytes).
        let (lo, hi) = (f.bit, f.bit + width);
        let o = at + f.offset as usize;
        for i in 0..n as usize {
            let (b0, b1) = (i as u32 * 8, i as u32 * 8 + 8);
            let (from, to) = (lo.max(b0) - b0, hi.min(b1) - b0);
            let mask = (((1u16 << to) - 1) & !((1u16 << from) - 1)) as u8;
            s.bytes[o + i] |= sp.read(abs + i as u64, 1) as u8 & mask;
            s.cover[o + i] = 1;
        }
        Ok(())
    }

    // ---- globals ------------------------------------------------------------

    /// Resolves global `g` (first use): returns its address. `top` is the
    /// first free register (for a nested evaluation of its initializer).
    pub(crate) fn resolve_global(&mut self, b: &mut dyn SemaBridge, g: u32, top: u32) -> u64 {
        let p = match self.globals[g as usize].kind {
            GKind::Str(s, ty, e) => self.alloc_string(b, s, ty, e),
            GKind::Var(d) => {
                // A variable with no initializer yet (`extern const int
                // n;` before its definition) may get one later in the
                // translation unit: its resolution is kept for this
                // evaluation only. While it still has none, the next
                // evaluation resolves it to the same object again, so the
                // object is made once and reused: re-making it would cost a
                // static slot per evaluation (spec/oven.md "Unknown
                // references").
                let info = var_info(b.ast(), d);
                let provisional =
                    info.init.is_none() && !matches!(info.state, VarEvalState::Value(_));
                let reusable = provisional
                    && matches!(info.state, VarEvalState::NotEvaluated | VarEvalState::NotConstant);
                let ty = reusable.then(|| self.types.intern(b, info.ty));
                let reuse =
                    self.provisional_objs.get(&g).copied().filter(|&(t, p)| {
                        Some(t) == ty && self.mem.alloc_of(Ptr::decode(p)).is_ok()
                    });
                let p = if let Some((_, p)) = reuse {
                    p
                } else {
                    let mut p = self.resolve_var(b, g, d, top);
                    if b.cxx23() {
                        p = self.bind_unknown(b, d, p);
                    }
                    match ty {
                        Some(t) => _ = self.provisional_objs.insert(g, (t, p)),
                        None => _ = self.provisional_objs.remove(&g),
                    }
                    p
                };
                if provisional {
                    self.provisional.push(g);
                }
                p
            }
        };
        if let GState::Unresolved = self.globals[g as usize].state {
            self.globals[g as usize].state = GState::Resolved(p);
        }
        p
    }

    fn alloc_string(
        &mut self,
        b: &mut dyn SemaBridge,
        s: StringLitId,
        ty: OTy,
        e: Option<ExprId>,
    ) -> u64 {
        let size = self.types.size(ty);
        let p = self.mem.alloc_static(ty, size, flags::CONST, Origin::Str(s, e));
        let base = u64::from(
            self.mem.statics[(Ptr::decode(p).slot & !crate::mem::STATIC_BIT) as usize].base,
        );
        let bytes = b.ast().strings.bytes(s);
        let n = bytes.len().min(size as usize);
        let (a0, a1) = (base as usize, (base + size) as usize);
        self.mem.stat.bytes[a0..a1].fill(0);
        self.mem.stat.bytes[a0..a0 + n].copy_from_slice(&bytes[..n]);
        self.mem.stat.set_init(base, size, true);
        p
    }

    /// The static object of lifetime-extended temporary `t`, thawed from
    /// its `LetData.value`: readable if it is usable in constant
    /// expressions (a `const` temporary extended by a variable that is,
    /// [expr.const]/4), else an external object whose accesses clang
    /// reports as outside the expression that created it.
    fn resolve_let(&mut self, b: &mut dyn SemaBridge, t: DeclId, top: u32) -> u64 {
        if let Some(&p) = self.lets.get(&t) {
            return p;
        }
        let ast = b.ast();
        let q = ast.decls.get(t).ty().unwrap_or(QualType::INT);
        let l = ast.decls.lifetime_extended(t).copied();
        // An array temporary is const through its elements.
        let usable = (ast.types.canonical(q).has_const_bit()
            || ast.types.effective_quals(q).is_const())
            && l.is_some_and(|l| var_info(ast, l.extending).usable);
        let value = l.and_then(|l| l.value);
        let ty = self.types.intern(b, q);
        let size = self.types.size(ty);
        let hu = self.static_flags(ty);
        let p = match value {
            Some(cv) if usable => {
                let p = self.mem.alloc_static(ty, size, flags::CONST | hu, Origin::Let(t));
                self.lets.insert(t, p);
                self.thaw_into(b, cv, p, top);
                p
            }
            _ => self.mem.alloc_static(ty, size, flags::EXTERNAL | flags::CONST, Origin::Let(t)),
        };
        self.lets.insert(t, p);
        p
    }

    /// P2280 (C++23 [expr.const]/10): a variable of reference type not
    /// usable in constant expressions (a parameter outside its call, an
    /// `extern` reference, one whose initializer is not constant) is
    /// "treated as binding to an unspecified object of the referenced
    /// type whose lifetime ... includes the entire constant evaluation".
    /// Its external object `p` becomes a reference to that object, of
    /// origin [`Origin::Unknown`] (one per variable): naming the
    /// reference succeeds and only an access of the object is diagnosed
    /// (clang `evaluateVarDeclInit` `AllowConstexprUnknown`).
    fn bind_unknown(&mut self, b: &mut dyn SemaBridge, d: DeclId, p: u64) -> u64 {
        let pp = Ptr::decode(p);
        let Some(a) = self.mem.alloc_any(pp) else { return p };
        if !matches!(a.origin, Origin::External(x) if x == d) {
            return p;
        }
        let slot_ty = a.ty;
        let ast = b.ast();
        let Some(q) = ast.decls.get(d).ty() else { return p };
        if !ast.types.is_reference(q) {
            return p;
        }
        let Some(rq) = ast.types.pointee(q) else { return p };
        if ast.types.is_function(rq) {
            return p;
        }
        let u = if let Some(&u) = self.unknowns.get(&d) {
            u
        } else {
            let ty = self.types.intern(b, rq);
            let size = self.types.size(ty);
            if size > u64::from(u32::MAX) {
                // Not representable (32-bit offsets): naming it stays
                // diagnosed, as before C++23.
                return p;
            }
            let hu = self.static_flags(ty) & flags::HAS_UNION;
            // No bytes: every access of the object is diagnosed.
            let u = self.mem.alloc_unbacked(
                ty,
                size,
                flags::EXTERNAL | flags::CONST | hu,
                Origin::Unknown(d),
            );
            self.unknowns.insert(d, u);
            u
        };
        let slot = (pp.slot & !crate::mem::STATIC_BIT) as usize;
        let base = u64::from(self.mem.statics[slot].base) + u64::from(pp.off);
        let len = self.types.size(slot_ty).min(8);
        let a = self.mem.alloc_of_mut(pp);
        a.flags = flags::CONST;
        a.origin = Origin::Global(d);
        self.mem.stat.write(base, len, u128::from(u));
        self.mem.stat.set_init(base, len, true);
        self.mem.stat.set_ptr(base, len, true);
        p
    }

    /// The object of a variable not usable in constant expressions: every
    /// access is diagnosed, so it holds no bytes ([`Memory::alloc_unbacked`]),
    /// except a reference's pointer slot, which [`Oven::bind_unknown`]
    /// rebinds to its unknown object.
    ///
    /// [`Memory::alloc_unbacked`]: crate::mem::Memory::alloc_unbacked
    fn external(&mut self, b: &mut dyn SemaBridge, ty: OTy, d: DeclId) -> u64 {
        let size = self.types.size(ty);
        let ast = b.ast();
        let fl = flags::EXTERNAL | flags::CONST;
        if ast.decls.get(d).ty().is_some_and(|q| ast.types.is_reference(q)) {
            self.mem.alloc_static(ty, size, fl, Origin::External(d))
        } else {
            self.mem.alloc_unbacked(ty, size, fl, Origin::External(d))
        }
    }

    fn resolve_var(&mut self, b: &mut dyn SemaBridge, g: u32, d: DeclId, top: u32) -> u64 {
        let info = var_info(b.ast(), d);
        if !info.lax {
            return self.resolve_var_in(b, g, d, top, false);
        }
        // A lax variable: one object in both modes, holding its value,
        // which only a lax fold may read (`mem::LAX_READ`; a strict read
        // faults "read of non-constexpr variable" whatever it holds), so
        // the variable has one address and a pointer to it held by a
        // usable variable (`constexpr const double *p = &d;`, resolved
        // strictly) reads it in a lax fold (spec/oven.md "Lax folds").
        // The value is the stored one, or else its initializer evaluated
        // laxly in either mode: clang's `VarDecl::evaluateValue` only
        // notes a read of a non-constexpr variable, so it keeps the value
        // of an initializer that reads another lax variable.
        if let VarEvalState::Value(cv) = info.state {
            let ty = self.types.intern(b, info.ty);
            let size = self.types.size(ty);
            let fl = flags::EXTERNAL | flags::CONST | self.static_flags(ty);
            let p =
                self.mem.alloc_static_in(ty, size, fl, Origin::External(d), crate::mem::LAX_READ);
            self.globals[g as usize].state = GState::Resolved(p);
            self.thaw_into(b, cv, p, top);
            return p;
        }
        if info.state == VarEvalState::Evaluating {
            // Mid-initialization: kept for this evaluation only.
            self.provisional.push(g);
        }
        let lax = (self.run.lax, self.mem.lax);
        (self.run.lax, self.mem.lax) = (true, true);
        let p = self.resolve_var_in(b, g, d, top, true);
        (self.run.lax, self.mem.lax) = lax;
        p
    }

    /// A static object of type `ty` for global `d`.
    fn global_obj(&mut self, ty: OTy, fl: u8, d: DeclId) -> u64 {
        let size = self.types.size(ty);
        self.mem.alloc_static(ty, size, fl, Origin::Global(d))
    }

    fn resolve_var_in(
        &mut self,
        b: &mut dyn SemaBridge,
        g: u32,
        d: DeclId,
        top: u32,
        lax_read: bool,
    ) -> u64 {
        let info = var_info(b.ast(), d);
        let ty = self.types.intern(b, info.ty);
        if !(info.usable || info.lax && (self.run.lax || lax_read)) {
            return self.external(b, ty, d);
        }
        let hu = self.static_flags(ty);
        match info.state {
            VarEvalState::Value(cv) => {
                let p = self.global_obj(ty, flags::CONST | hu, d);
                self.globals[g as usize].state = GState::Resolved(p);
                self.thaw_into(b, cv, p, top);
                p
            }
            VarEvalState::NotConstant => match info.init {
                // Not constant-initialized: a fold reads the value of its
                // initializer evaluated non-manifestly (below).
                Some(init) if (info.usable || info.lax) && self.run.nesting < MAX_NESTING => {
                    self.refold_var_init(b, g, d, ty, init, top, hu)
                }
                _ => self.external(b, ty, d),
            },
            VarEvalState::Evaluating => {
                // Mid-initialization: kept for this evaluation only (an
                // automatic variable has no `eval_inner` placeholder to
                // restore; WP-465).
                self.provisional.push(g);
                self.global_obj(ty, flags::CONST | flags::RESOLVING | (hu & flags::HAS_UNION), d)
            }
            VarEvalState::NotEvaluated => {
                let Some(init) = info.init else { return self.external(b, ty, d) };
                if self.run.nesting >= MAX_NESTING {
                    return self.external(b, ty, d);
                }
                match self.eval_var_init(b, g, d, ty, init, top, hu, lax_read, true) {
                    Ok(p) => p,
                    Err(_) if info.usable && !lax_read || info.lax && lax_read => {
                        self.refold_var_init(b, g, d, ty, init, top, hu)
                    }
                    Err(p) => p,
                }
            }
        }
    }

    /// The object of potentially-constant variable `d` whose constant
    /// initialization failed: clang `VarDecl::evaluateValue` then
    /// evaluates its initializer `init` non-manifestly
    /// (`__builtin_is_constant_evaluated()` false) and laxly, and a fold
    /// reads that value after noting "initializer of 'x' is not a constant
    /// expression" (`const int cx = __builtin_is_constant_evaluated() ? x
    /// : 9;` folds to 9 in `int a[cx]`, `__builtin_constant_p(cx)` and
    /// codegen). So the object is [`crate::mem::LAX_READ`]: a strict read
    /// still faults with that note (spec/oven.md "Lax folds").
    #[allow(clippy::too_many_arguments)]
    fn refold_var_init(
        &mut self,
        b: &mut dyn SemaBridge,
        g: u32,
        d: DeclId,
        ty: OTy,
        init: ExprId,
        top: u32,
        hu: u8,
    ) -> u64 {
        let lax = (self.run.lax, self.mem.lax);
        (self.run.lax, self.mem.lax) = (true, true);
        let r = self.eval_var_init(b, g, d, ty, init, top, hu, true, false);
        (self.run.lax, self.mem.lax) = lax;
        match r {
            Ok(p) | Err(p) => p,
        }
    }

    /// Evaluates initializer `init` of global `g` (variable `d`) in its own
    /// evaluation context, manifestly constant-evaluated iff `manifest`,
    /// into a fresh static object: `Ok` holds its value (a [`crate::mem::
    /// LAX_READ`] one iff `lax_read`), `Err` an external object.
    #[allow(clippy::too_many_arguments)]
    fn eval_var_init(
        &mut self,
        b: &mut dyn SemaBridge,
        g: u32,
        d: DeclId,
        ty: OTy,
        init: ExprId,
        top: u32,
        hu: u8,
        lax_read: bool,
        manifest: bool,
    ) -> Result<u64, u64> {
        let p = self.placeholder(g, ty, d);
        self.globals[g as usize].state = GState::Resolved(p);
        let saved = (
            self.run.counters,
            self.mem.allocated,
            self.mem.peak,
            self.run.object,
            self.run.silent,
            self.run.const_context,
            self.run.fold,
            self.run.cev_warn,
            self.run.ub_check,
            self.run.contracts_off,
        );
        let mark = self.mem.mark();
        // Its own evaluation context: its allocations are numbered
        // from 0, leak-checked and freed at its end.
        let heap = (self.mem.heap_allocs, self.mem.ctx);
        let ctx = self.ctx_depth;
        self.ctx_depth += 1;
        self.mem.ctx = ctx;
        self.mem.heap_allocs = 0;
        self.run.nesting += 1;
        self.run.counters = Counters::default();
        self.run.object = Some(d);
        self.run.silent = true;
        // Constant initialization is manifestly constant-evaluated;
        // a re-fold after it failed is not.
        self.run.const_context = manifest;
        self.run.fold = !manifest;
        self.run.cev_warn = false;
        self.run.ub_check = false;
        // Its own initialization: its contract assertions are evaluated
        // ([expr.const.init]/4.1 ignores only the variable's own).
        self.run.contracts_off = false;
        let r = self.unbound(|o| o.eval_at(b, init, Some(d), top));
        self.mem.free_heap_of(ctx, &self.types);
        self.mem.release(mark, &self.types);
        self.ctx_depth -= 1;
        (self.mem.heap_allocs, self.mem.ctx) = heap;
        self.run.nesting -= 1;
        (
            self.run.counters,
            self.mem.allocated,
            self.mem.peak,
            self.run.object,
            self.run.silent,
            self.run.const_context,
            self.run.fold,
            self.run.cev_warn,
            self.run.ub_check,
            self.run.contracts_off,
        ) = saved;
        // The placeholder's storage now belongs to the variable.
        self.globals[g as usize].slot = 0;
        let a = self.mem.alloc_of_mut(Ptr::decode(p));
        a.frame = 0;
        if let Ok(cv) = r {
            a.flags = a.flags & !flags::RESOLVING | hu;
            if lax_read {
                a.flags |= flags::EXTERNAL;
                a.origin = Origin::External(d);
                a.frame = crate::mem::LAX_READ;
            }
            self.thaw_into(b, cv, p, top);
            Ok(p)
        } else {
            a.flags = flags::EXTERNAL | flags::CONST;
            a.origin = Origin::External(d);
            Err(p)
        }
    }

    // ---- thawing ------------------------------------------------------------

    /// The type flags of a static object of type `ty` thawed from a value
    /// (its lifetime began outside the evaluation): unions, and `mutable`
    /// members whose reads are diagnosed ([expr.const]/5.9).
    fn static_flags(&self, ty: OTy) -> u8 {
        let info = self.types.info(ty);
        (if info.has_union { flags::HAS_UNION } else { 0 })
            | if info.has_mutable { flags::MUTABLE_OUTSIDE } else { 0 }
    }

    /// The static object holding stored constant `cv`.
    pub(crate) fn thaw(&mut self, b: &mut dyn SemaBridge, cv: ConstValueId, top: u32) -> u64 {
        if let Some(&p) = self.thawed.get(&cv) {
            return p;
        }
        let q = b.consts().ty(cv);
        let ty = self.types.intern(b, q);
        let size = self.types.size(ty);
        let hu = self.static_flags(ty);
        let p = self.mem.alloc_static(ty, size, flags::CONST | hu, Origin::Const(cv));
        self.thawed.insert(cv, p);
        self.thaw_into(b, cv, p, top);
        p
    }

    /// Writes stored constant `cv` into the static object at `p`.
    pub(crate) fn thaw_into(&mut self, b: &mut dyn SemaBridge, cv: ConstValueId, p: u64, top: u32) {
        let pp = Ptr::decode(p);
        let base = u64::from(self.mem.statics[(pp.slot & !crate::mem::STATIC_BIT) as usize].base)
            + u64::from(pp.off);
        let aty = self.mem.statics[(pp.slot & !crate::mem::STATIC_BIT) as usize].ty;
        let scalar = match b.consts().get(cv) {
            ConstValue::Scalar(s) => Some(s.ty()),
            ConstValue::Aggregate(_) => None,
        };
        if scalar.is_some() {
            self.thaw_scalar(b, cv, aty, base, top);
            return;
        }
        let (relocs, unions): (Vec<Reloc>, Vec<UnionMember>) = {
            let st = b.consts();
            let Some(img) = st.aggregate(cv) else { return };
            let len = img.bytes.len() as u64;
            let a0 = base as usize;
            self.mem.stat.bytes[a0..a0 + img.bytes.len()].copy_from_slice(img.bytes);
            self.mem.stat.set_init(base, len, true);
            self.mem.stat.set_ptr(base, len, false);
            for r in img.padding {
                self.mem.stat.set_init(base + u64::from(r.start), u64::from(r.len), false);
            }
            (img.relocs.to_vec(), img.unions.to_vec())
        };
        if pp.off == 0 && self.types.info(aty).has_vptr {
            // The dynamic-type words are not part of the image.
            self.thaw_vptrs(b, crate::mem::Sp::Static, base, aty);
        }
        for r in relocs {
            let at = u64::from(r.at);
            let ty = self.deepest_at(aty, at);
            if let TyKind::MemPtr { .. } = self.types.kind(ty) {
                let v = self.mp_thaw(b, r.value);
                let n = self.types.size(ty);
                self.mem.stat.write(base + at, n, v);
                continue;
            }
            if let TyKind::Reflection = self.types.kind(ty) {
                let v = match b.consts().scalar(r.value) {
                    Some(Scalar::Reflection { id, .. }) => u128::from(id.bits()),
                    _ => 0,
                };
                self.mem.stat.write(base + at, 8, v);
                continue;
            }
            let v = self.thaw_ptr(b, r.value, ty, top);
            self.put_thawed_ptr(base + at, v);
        }
        for u in unions {
            let at = u64::from(u.at);
            let idx =
                self.types.unions(aty).iter().filter(|&&(o, _)| o == at).find_map(|&(_, ut)| {
                    let i = self.types.fields(ut).iter().position(|f| f.decl == u.field)?;
                    Some((i, ut))
                });
            if let Some((i, ut)) = idx {
                self.mem.union_thaw(Ptr::union_key(p, u64::from(pp.off) + at, ut), i as u32);
            }
        }
    }

    fn thaw_scalar(
        &mut self,
        b: &mut dyn SemaBridge,
        cv: ConstValueId,
        ty: OTy,
        base: u64,
        top: u32,
    ) {
        let info = *self.types.info(ty);
        let len = value_len(info.size, info.kind);
        if let TyKind::WideInt { width, signed } = info.kind {
            // Canonical limbs, extended over the object (as `Op::Wide`
            // stores them).
            let w = match b.consts().scalar(cv) {
                Some(Scalar::Int { bits, .. }) => bits.words().to_vec(),
                _ => vec![0],
            };
            let mut w = crate::bigint::extend(&w, (len / 8) as usize, false);
            crate::bigint::canon(&mut w, width, signed);
            for (i, &x) in w.iter().enumerate() {
                self.mem.stat.write(base + i as u64 * 8, 8, u128::from(x));
            }
            self.mem.stat.set_init(base, len, true);
            self.mem.stat.set_ptr(base, len, false);
            return;
        }
        let (v, is_ptr) = match b.consts().scalar(cv) {
            Some(Scalar::Int { bits, .. }) => (bits.to_u128().unwrap_or(0), false),
            Some(Scalar::Bool { value, .. }) => (u128::from(value), false),
            Some(Scalar::Float { bits, .. }) => (bits, false),
            Some(Scalar::Pointer { .. }) => (0, true),
            Some(Scalar::MemberPtr { .. }) => (self.mp_thaw(b, cv), false),
            Some(Scalar::Reflection { id, .. }) => (u128::from(id.bits()), false),
            _ => (0, false),
        };
        if is_ptr {
            let v = self.thaw_ptr(b, cv, ty, top);
            self.mem.stat.set_init(base, len, true);
            self.put_thawed_ptr(base, v);
            return;
        }
        self.mem.stat.write(base, len, v);
        self.mem.stat.set_init(base, len, true);
        self.mem.stat.set_ptr(base, len, false);
    }

    /// Writes thawed pointer register value `v` (tag bits included) to
    /// static byte `abs`, with its pointer mark.
    fn put_thawed_ptr(&mut self, abs: u64, v: u128) {
        let mark = match (v as u64, (v >> 64) as u32) {
            (0, _) => 0,
            (_, tag) => self.mem.set_void_tag(crate::mem::Sp::Static, abs, tag),
        };
        self.mem.stat.write(abs, 8, u128::from(v as u64));
        self.mem.stat.set_ptr_mark(abs, 8, mark);
    }

    /// The VM pointer register value for stored pointer value `cv` of type
    /// `ty`: a `void *` that keeps its designated type gets it back as
    /// its `kbc::VOID_TAG` tag bits (spec/oven.md "Construction").
    pub(crate) fn thaw_ptr(
        &mut self,
        b: &mut dyn SemaBridge,
        cv: ConstValueId,
        ty: OTy,
        top: u32,
    ) -> u128 {
        let Some(Scalar::Pointer { base, offset, designated, past, .. }) = b.consts().scalar(cv)
        else {
            return 0;
        };
        let tag = match designated {
            Some(d) => u128::from(self.types.intern(b, d).0 + 1) << 64,
            None => 0,
        };
        u128::from(self.thaw_ptr_raw(b, base, offset, past, ty, top)) | tag
    }

    fn thaw_ptr_raw(
        &mut self,
        b: &mut dyn SemaBridge,
        base: PtrBase,
        offset: u64,
        past: bool,
        ty: OTy,
        top: u32,
    ) -> u64 {
        let pointee = match self.types.kind(ty) {
            TyKind::Ptr(p) => Some(p),
            _ => None,
        };
        let root = match base {
            PtrBase::Function(d) => {
                let f = self.func_index(b.ast(), d);
                return fn_ptr(f);
            }
            PtrBase::Decl(d) => {
                let g = self.global_index(b.ast(), d);
                match self.globals[g as usize].state {
                    GState::Resolved(p) | GState::Bound(p) => p,
                    GState::Unresolved => self.resolve_global(b, g, top),
                }
            }
            // The object of the literal's own evaluation: a pointer frozen
            // from `"a"` thaws to the object `"a"` creates, so it compares
            // equal to itself and unspecified to other literals.
            PtrBase::StringLit { lit, origin } => {
                let ast = b.ast();
                let (enc, units) = (ast.strings.encoding(lit), ast.strings.unit_count(lit));
                let cq = match enc {
                    StringEncoding::Ordinary => QualType::CHAR,
                    StringEncoding::Utf8 => QualType::CHAR8,
                    StringEncoding::Utf16 => QualType::CHAR16,
                    StringEncoding::Utf32 => QualType::CHAR32,
                    StringEncoding::Wide16 | StringEncoding::Wide32 => QualType::WCHAR,
                };
                let g = match self.lit_exprs.get(&origin) {
                    Some(&g) => g,
                    None => {
                        let et = self.types.intern(b, cq);
                        let at = self.types.array_of(et, units + 1);
                        self.string_expr_index(origin, lit, at)
                    }
                };
                match self.globals[g as usize].state {
                    GState::Resolved(p) | GState::Bound(p) => p,
                    GState::Unresolved => self.resolve_global(b, g, top),
                }
            }
            PtrBase::Const(c) => self.thaw(b, c, top),
            PtrBase::Temporary(d) => self.resolve_let(b, d, top),
            // The one object `typeid` evaluates to, so a thawed `&typeid(T)`
            // compares equal to a fresh one.
            PtrBase::TypeInfo(q) => self.type_info(q, pointee.unwrap_or(ty)),
            PtrBase::Label { origin, .. } => self.label_addr(b, origin),
            // An integer cast to a pointer points at nothing again
            // (`mem::NOWHERE`, frame slot 0, the integer its offset).
            PtrBase::Absolute => {
                let off = u32::try_from(offset).unwrap_or(u32::MAX);
                return Ptr::with_off(crate::mem::NOWHERE, u64::from(off), false);
            }
        };
        let rp = Ptr::decode(root);
        let Some(a) = self.alloc_any(rp).copied() else { return root };
        // A stored past-the-end bit (`Scalar::Pointer` `past`), else one
        // inferred: at the end of the complete object, or where no
        // subobject of the pointee type starts.
        let past = past
            || match pointee {
                Some(pt) if !matches!(self.types.kind(pt), TyKind::Void | TyKind::Opaque) => {
                    offset >= u64::from(a.size) || !self.types.has_subobject(a.ty, offset, pt)
                }
                _ => offset >= u64::from(a.size),
            };
        Ptr::with_off(root, offset, past)
    }
}

fn ast_first(b: &dyn SemaBridge, d: DeclId) -> DeclId {
    b.ast().first(d)
}

/// The `MaterializeTemporaryExpr` that created static temporary `t` (in
/// its extending variable's initializer), for "temporary created here".
pub(crate) fn let_mte(ast: &Ast, t: DeclId) -> Option<ExprId> {
    let ext = ast.decls.lifetime_extended(t)?.extending;
    let init = ast.redecls_of(ext).find_map(|r| ast.decls.var(r).and_then(|v| v.init))?;
    let mut work = vec![init];
    while let Some(e) = work.pop() {
        if ast.exprs.get(e).kind() == ExprKind::MaterializeTemporaryExpr
            && ast.exprs.lifetime_extended_temporary(e) == Some(t)
        {
            return Some(e);
        }
        ast.exprs.children_into(e, &ast.lists, &mut work);
    }
    None
}

/// clang's `Expr::getExprLoc` for an evaluation root, as far as it
/// differs from kiln's node location: wrappers are looked through, a
/// `CallExpr` is at its callee's start and a `CXXMemberCallExpr` at the
/// member name.
pub(crate) fn expr_loc(ast: &Ast, mut e: ExprId) -> Loc {
    let x = &ast.exprs;
    loop {
        let k = x.get(e).kind();
        e = match k {
            ExprKind::ConstantExpr
            | ExprKind::ExprWithCleanups
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::CXXBindTemporaryExpr
            | ExprKind::ImplicitCastExpr => x.operand(e),
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr => {
                let c = x.callee(e);
                let c =
                    if x.get(c).kind() == ExprKind::ImplicitCastExpr { x.operand(c) } else { c };
                let ck = x.get(c).kind();
                if ck == ExprKind::DeclRefExpr {
                    return ast.templates.expr_begin.get(c).copied().unwrap_or(x.get(c).loc());
                }
                if ck == ExprKind::MemberExpr {
                    return x.get(c).loc();
                }
                return x.get(e).loc();
            }
            _ => return x.get(e).loc(),
        };
    }
}
