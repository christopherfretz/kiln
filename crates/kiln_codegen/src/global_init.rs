//! Dynamic initialization and destruction of variables with static
//! storage duration ([basic.start.dynamic], [basic.start.term];
//! spec/codegen.md §7e): clang's `CGDeclCXX.cpp` and
//! `ItaniumCXXABI::EmitGuardedInit`.
//!
//! A global whose initializer is not a constant gets a zero initial
//! value and an internal `__cxx_global_var_init[.N]` function that
//! initializes it (and registers its destructor with `__cxa_atexit`).
//! Ordered initializers run, in declaration order, from the one
//! `_GLOBAL__sub_I_<file>` function; the unordered ones (inline
//! variables, template instantiations) are guarded by `_ZGV` variables
//! and get their own `@llvm.global_ctors` entries, keyed to the
//! variable's comdat. Static locals are guarded in place
//! ([stmt.dcl]/3) and references extending temporaries bind `_ZGR`
//! globals ([class.temporary]/6).

use kiln_abi::mangle::{self, Structor};
use kiln_ast::expr::ExprKind;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_llvm::{
    Attr, AttrKind, BinOp, CallOpts, ComdatKind, Flags, FuncId, GlobalId, IntPredicate, Linkage,
    MemOpts, Ordering, TypeId as LTy, UwTableKind, Value, Visibility,
};
use kiln_source::fx::FxHashMap;

use crate::cleanup::{Cleanup, JumpDest};
use crate::fir::{Fir, Op, V};
use crate::func::{Addr, Fx};
use crate::module::{Cg, R, err, gval};

/// `DelayedCXXInitPosition` value of a variable whose initializer has
/// been emitted.
const DONE: u32 = u32::MAX;

/// Module-wide dynamic-initialization state.
#[derive(Default)]
pub(crate) struct GlobalInits {
    /// clang `CXXGlobalInits`: the ordered initializers, in order; `None`
    /// for a slot reserved by a deferred variable not (yet) emitted.
    ordered: Vec<Option<FuncId>>,
    /// clang `DelayedCXXInitPosition`: first declaration → its reserved
    /// slot in `ordered`, or [`DONE`].
    delayed: FxHashMap<DeclId, u32>,
    /// clang `GlobalCtors`: (lexical order, function, comdat key).
    ctors: Vec<(u32, FuncId, Option<GlobalId>)>,
    /// clang `LLVMUsed`.
    used: Vec<GlobalId>,
    /// Guard variables by first declaration (clang
    /// `StaticLocalDeclGuardMap`).
    guards: FxHashMap<DeclId, GlobalId>,
    /// The discriminator index of each static local (its `_ZGV` and
    /// `_ZGR` names).
    pub local_index: FxHashMap<DeclId, u32>,
    /// `_ZGR` globals by lifetime-extended temporary (clang
    /// `MaterializedGlobalTemporaryMap`).
    temps: FxHashMap<DeclId, GlobalId>,
}

/// clang `getTransformedFileName`: the module name's file name with
/// every character outside `[A-Za-z0-9._]` replaced by `_`.
fn transformed_file_name(module: &str) -> String {
    let base = module.rsplit('/').next().unwrap_or("");
    let base = if base.is_empty() { "<null>" } else { base };
    base.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '.' { c } else { '_' })
        .collect()
}

impl Cg<'_> {
    // ---- EmitGlobal / DeclMustBeEmitted -------------------------------

    /// clang `EmitGlobal`: a deferred C++ variable with an initializer
    /// reserves its position among the ordered initializers.
    pub(crate) fn note_deferred_init(&mut self, first: DeclId, d: DeclId) {
        if self.ast.decls.var(d).is_some_and(|v| v.init.is_some())
            && !self.gi.delayed.contains_key(&first)
        {
            let pos = u32::try_from(self.gi.ordered.len()).expect("initializer count");
            self.gi.delayed.insert(first, pos);
            self.gi.ordered.push(None);
        }
    }

    /// clang `DeclMustBeEmitted` for a discardable variable: it has a
    /// non-trivial destructor, or an initializer that is not a constant
    /// and may have side effects.
    pub(crate) fn var_must_emit(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let Some(ty) = ast.decls.get(d).ty() else { return false };
        if !ast.types.is_reference(ty) && self.destructed_class(ty).is_some() {
            return true;
        }
        let Some(init) = ast.decls.var(d).and_then(|v| v.init) else { return false };
        self.folds.get(init).is_none() && self.may_have_side_effects(init)
    }

    /// clang `Expr::HasSideEffects(Ctx, /*IncludePossibleEffects=*/true)`.
    fn may_have_side_effects(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let exprs = &ast.exprs;
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            match exprs.get(x).kind() {
                ExprKind::UnaryOperator if exprs.unary_op(x).is_inc_dec() => return true,
                ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator
                    if exprs.binary_op(x).is_assignment() =>
                {
                    return true;
                }
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr
                | ExprKind::UserDefinedLiteral
                | ExprKind::CXXNewExpr
                | ExprKind::CXXDeleteExpr
                | ExprKind::AtomicExpr
                | ExprKind::CXXThrowExpr
                | ExprKind::CoawaitExpr
                | ExprKind::CoyieldExpr => return true,
                ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                    let ctor = exprs.constructor(x);
                    if ast.decls.function(ctor).is_none_or(|f| !f.spec.is_trivial()) {
                        return true;
                    }
                    stack.extend(exprs.children(x, &ast.lists));
                }
                ExprKind::ExprWithCleanups if exprs.cleanups_have_side_effects(x) => return true,
                ExprKind::CXXTypeidExpr if exprs.typeid_potentially_evaluated(x) => return true,
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::LambdaExpr
                | ExprKind::RequiresExpr => {}
                _ => stack.extend(exprs.children(x, &ast.lists)),
            }
        }
        false
    }

    // ---- EmitCXXGlobalVarDeclInitFunc ---------------------------------

    /// An internal `void()` function in `.text.startup` (clang
    /// `CreateGlobalInitOrCleanUpFunction`).
    fn init_function(&mut self, name: &str) -> R<FuncId> {
        let fty = self.m.types.function(LTy::VOID, &[], false);
        let f = match self.m.add_function(name, fty) {
            Ok(f) => f,
            Err(e) => return err(None, format!("bad initialization function: {e:?}")),
        };
        let func = self.m.func_mut(f);
        func.linkage = Linkage::Internal;
        func.section = Some(".text.startup".into());
        Ok(f)
    }

    /// The function attributes of an initialization function.
    fn init_fn_attrs(&mut self, vector_width: u64) -> kiln_llvm::AttrSetId {
        let mut a = vec![Attr::Flag(AttrKind::NoInline)];
        if !self.opts.exceptions {
            a.push(Attr::Flag(AttrKind::NoUnwind));
        }
        a.push(Attr::UwTable(UwTableKind::Async));
        self.target_attrs(&mut a);
        if self.is_x86() {
            a.push(Attr::string("min-legal-vector-width", &vector_width.to_string()));
        }
        self.m.attr_set(&a)
    }

    /// `StartFunction` for an initialization function `f`.
    fn start_init_fn(&mut self, f: FuncId) {
        self.f = Fx::empty();
        self.f.fir = Fir::new(Vec::new());
        self.f.func = Some(f);
        self.f.mustprogress = false;
        let ret_block = self.f.fir.create_block();
        self.f.ret_block = Some(ret_block);
        self.f.ret_dest = Some(JumpDest { blk: ret_block, depth: Some(0), index: 1 });
        self.f.cl.next_index = 2;
        self.f.cl.prologue = self.f.cl.depth();
    }

    /// `FinishFunction` for an initialization function `f`, then its
    /// attributes and lowering (`d` locates errors).
    fn finish_init_fn(&mut self, f: FuncId, d: DeclId) -> R<()> {
        let prologue = self.f.cl.prologue;
        self.pop_cleanup_blocks(prologue);
        self.emit_return_block();
        self.emit_epilog();
        self.finish_eh();
        self.emit_unreachable_if_used();
        if let Some(e) = self.f.error.take() {
            return Err(e);
        }
        if let Some(rb) = self.f.ret_block
            && self.f.fir.is_placed(rb)
            && self.f.fir.block_uses(rb) == 0
        {
            self.f.fir.erase_block(rb);
            self.f.ret_block = None;
        }
        let fa = self.init_fn_attrs(self.f.vector_width);
        self.m.func_mut(f).attrs.func = fa;
        self.nounwind.insert(f, !self.opts.exceptions);
        let fir = std::mem::replace(&mut self.f.fir, Fir::new(Vec::new()));
        if self.f.uses_ph {
            self.pending.push((f, d, fir));
            return Ok(());
        }
        if let Err(e) = fir.lower(&mut self.m, f, &self.phs) {
            return err(Some(self.ast.decls.get(d).loc()), format!("IR lowering failed: {e:?}"));
        }
        Ok(())
    }

    /// clang `EmitCXXGlobalVarDeclInitFunc`: the initialization function
    /// of global `g` of variable `d`, which initializes it when
    /// `perform` and registers its destructor.
    pub(crate) fn emit_var_init_fn(&mut self, d: DeclId, g: GlobalId, perform: bool) -> R<()> {
        let ast = self.ast;
        let first = ast.first(d);
        if self.gi.delayed.get(&first) == Some(&DONE) {
            return Ok(());
        }
        let f = self.init_function("__cxx_global_var_init")?;
        // GenerateCXXGlobalVarDeclInitFunc.
        let saved = std::mem::replace(&mut self.f, Fx::empty());
        self.start_init_fn(f);
        let lin = self.m.global(g).linkage;
        let r = if matches!(
            lin,
            Linkage::Weak | Linkage::WeakOdr | Linkage::LinkOnce | Linkage::LinkOnceOdr
        ) {
            self.guarded_init(d, g, perform)
        } else {
            self.global_var_decl_init(d, g, perform)
        };
        let r = r.and_then(|()| self.finish_init_fn(f, d));
        self.f = saved;
        r?;
        let internal = mangle::is_internal_symbol(ast, d);
        let key = (!internal).then_some(g);
        let unordered =
            self.var_instantiation_linkage(first).is_some() || (self.var_is_inline(d) && !internal);
        if unordered {
            // [basic.start.dynamic]/1: partially-ordered or unordered
            // initialization, in its own `llvm.global_ctors` entry.
            let lex = match self.gi.delayed.get(&first) {
                Some(&p) => p,
                None => u32::try_from(self.gi.ordered.len()).expect("initializer count"),
            };
            self.gi.ctors.push((lex, f, key));
            if let Some(k) = key {
                self.gi.used.push(k);
                if let Some(c) = self.m.global(g).comdat {
                    self.m.func_mut(f).comdat = Some(c);
                }
            }
        } else {
            match self.gi.delayed.get(&first) {
                None => self.gi.ordered.push(Some(f)),
                Some(&p) => self.gi.ordered[p as usize] = Some(f),
            }
        }
        self.gi.delayed.insert(first, DONE);
        Ok(())
    }

    /// clang `EmitCXXGlobalVarDeclInit`: initializes global `g` of
    /// variable `d` (when `perform`) and registers its destructor.
    fn global_var_decl_init(&mut self, d: DeclId, g: GlobalId, perform: bool) -> R<()> {
        let ast = self.ast;
        let first = ast.first(d);
        let ty = ast.decls.get(d).ty().expect("var type");
        let loc = Some(ast.decls.get(d).loc());
        let init = ast
            .decls
            .var(d)
            .and_then(|v| v.init)
            .or_else(|| ast.redecls_of(first).find_map(|r| ast.decls.var(r).and_then(|x| x.init)))
            .filter(|&i| !ast.is_trivial_default_construction(i));
        let align = self.global_align(ty);
        let mt = self.mty(ty);
        let addr = Addr { v: V::C(gval(g)), ty: mt, align };
        if ast.types.is_reference(ty) {
            let Some(init) = init else { return err(loc, "reference without initializer") };
            return self.emit_var_init(d, ty, addr, init);
        }
        if perform && let Some(init) = init {
            self.emit_var_init(d, ty, addr, init)?;
        }
        // EmitDeclDestroy.
        if let Some(rd) = self.destructed_class(ty) {
            if self.array_elem(ty).is_some() {
                return err(
                    loc,
                    "destruction of a static array of class type is not supported yet (M3)",
                );
            }
            self.register_global_dtor(rd, V::C(gval(g)))?;
        }
        Ok(())
    }

    /// `@__dso_handle` (clang `CreateRuntimeVariable`, hidden).
    fn dso_handle(&mut self) -> GlobalId {
        if let Some(g) = self.m.get_global("__dso_handle") {
            return g;
        }
        let g = self.m.add_global("__dso_handle", LTy::I8);
        self.m.global_mut(g).visibility = Visibility::Hidden;
        g
    }

    /// clang `registerGlobalDtor` / `emitGlobalDtorWithCXAAtExit`: the
    /// complete destructor of class `record` runs on the object at `addr`
    /// at exit ([basic.start.term]/1).
    fn register_global_dtor(&mut self, record: DeclId, addr: V) -> R<()> {
        let ast = self.ast;
        let Some(def) = self.class_def(record) else { return Ok(()) };
        let Some(dtor) = ast.destructor(def) else {
            return err(Some(ast.decls.get(def).loc()), "destructor not declared");
        };
        let (callee, _) = self.structor_ref(dtor, Structor::Complete)?;
        let h = self.dso_handle();
        let nounwind = [Attr::Flag(AttrKind::NoUnwind)];
        let atexit =
            self.rt_fn("__cxa_atexit", LTy::I32, &[LTy::PTR, LTy::PTR, LTy::PTR], &nounwind);
        self.nounwind_runtime_call(atexit, LTy::I32, vec![callee, addr, V::C(gval(h))]);
        Ok(())
    }

    // ---- guarded initialization ----------------------------------------

    /// The function-local `static` `d` with global `g` at its declaration
    /// statement (clang `AddInitializerToStaticVarDecl`): a guarded
    /// initialization when its initializer is not a constant or it has a
    /// non-trivial destructor ([stmt.dcl]/3).
    pub(crate) fn static_local_init(&mut self, d: DeclId, g: GlobalId) -> R<()> {
        let ast = self.ast;
        if self.f.fir.cur().is_none() {
            return Ok(());
        }
        let Some(v) = ast.decls.var(d) else { return Ok(()) };
        let ty = ast.decls.get(d).ty().expect("var type");
        let init = v.init.filter(|&i| !ast.is_trivial_default_construction(i));
        let perform = init.is_some_and(|e| self.folds.get(e).is_none());
        let dtor = !ast.types.is_reference(ty) && self.destructed_class(ty).is_some();
        if perform || dtor { self.guarded_init(d, g, perform) } else { Ok(()) }
    }

    /// Whether static local `first` (or a global) needs dynamic
    /// initialization or destruction: its global is then not constant.
    pub(crate) fn static_needs_init(&self, d: DeclId, init: Option<ExprId>) -> bool {
        let ast = self.ast;
        let ty = ast.decls.get(d).ty().expect("var type");
        let init = init.filter(|&i| !ast.is_trivial_default_construction(i));
        init.is_some_and(|e| self.folds.get(e).is_none())
            || (!ast.types.is_reference(ty) && self.destructed_class(ty).is_some())
    }

    /// The `_ZGV` guard variable of `d` with global `g` (clang
    /// `ItaniumCXXABI::EmitGuardedInit`, creation part).
    fn guard_var(&mut self, d: DeclId, g: GlobalId, local: bool, ty: LTy, align: u64) -> GlobalId {
        let ast = self.ast;
        let first = ast.first(d);
        if let Some(&x) = self.gi.guards.get(&first) {
            return x;
        }
        let index = self.gi.local_index.get(&first).copied().unwrap_or(0);
        let name = mangle::mangle_guard(ast, &mut self.anon, first, index);
        let guard = self.m.add_global(&name, ty);
        let zero = self.m.const_int(ty, 0);
        let _ = self.m.set_initializer(guard, Some(zero));
        let (lin, dso, vis, comdat) = {
            let v = self.m.global(g);
            (v.linkage, v.dso_local, v.visibility, v.comdat)
        };
        let weak = matches!(
            lin,
            Linkage::Weak | Linkage::WeakOdr | Linkage::LinkOnce | Linkage::LinkOnceOdr
        );
        let gc = if !local && comdat.is_some() {
            comdat
        } else if weak {
            Some(self.m.comdat(&name, ComdatKind::Any))
        } else {
            None
        };
        let gm = self.m.global_mut(guard);
        gm.linkage = lin;
        gm.dso_local = dso;
        gm.visibility = vis;
        gm.align = Some(align);
        gm.comdat = gc;
        self.gi.guards.insert(first, guard);
        guard
    }

    /// clang `ItaniumCXXABI::EmitGuardedInit`: initializes `g` of variable
    /// `d` once, checking (and for a static local or a non-template inline
    /// variable, acquiring) its guard variable.
    pub(crate) fn guarded_init(&mut self, d: DeclId, g: GlobalId, perform: bool) -> R<()> {
        let ast = self.ast;
        let first = ast.first(d);
        let local = self.static_locals.contains_key(&first);
        let non_template_inline =
            self.var_is_inline(d) && self.var_instantiation_linkage(first).is_none();
        let threadsafe = local || non_template_inline;
        let internal = matches!(self.m.global(g).linkage, Linkage::Internal | Linkage::Private);
        let int8 = !threadsafe && internal;
        let (gty, galign) = if int8 { (LTy::I8, 1) } else { (LTy::I64, 8) };
        let guard = self.guard_var(d, g, local, gty, galign);
        let gp = V::C(gval(guard));
        let check = self.f.fir.create_block();
        let end = self.f.fir.create_block();
        // The first byte of the guard; acquire when thread-safe. ARM
        // (AArch64) tests only its low bit.
        let mut mem = MemOpts::aligned(galign);
        if threadsafe {
            mem.ordering = Ordering::Acquire;
        }
        let li = self.f.fir.push(Op::Load { ty: LTy::I8, ptr: gp, mem }, LTy::I8);
        let v = if !self.is_x86() && !int8 {
            let one = V::C(self.m.const_int(LTy::I8, 1));
            self.bin(BinOp::And, Flags::NONE, li, one)
        } else {
            li
        };
        let zero = V::C(self.m.const_int(LTy::I8, 0));
        let needs = self.icmp(IntPredicate::Eq, v, zero);
        // EmitCXXGuardedInitBranch: a static local is initialized once
        // in 2^20 entries.
        let br = self.f.fir.push(Op::CondBr { c: needs, t: check, f: end }, LTy::VOID);
        if local {
            let s = self.m.md_string(b"branch_weights");
            let w1 = self.m.const_int(LTy::I32, 1);
            let w2 = self.m.const_int(LTy::I32, 1024 * 1024 - 1);
            let (Ok(m1), Ok(m2)) = (self.m.md_const(w1), self.m.md_const(w2)) else {
                return err(None, "bad branch weights");
            };
            let md = self.m.md_tuple(&[Some(s), Some(m1), Some(m2)]);
            let k = self.m.md_kind("prof");
            self.f.fir.add_md(Fir::last_inst(br), k, md);
        }
        self.emit_block(check, false);
        let nounwind = [Attr::Flag(AttrKind::NoUnwind)];
        if threadsafe {
            let acq = self.rt_fn("__cxa_guard_acquire", LTy::I32, &[LTy::PTR], &nounwind);
            let r = self.nounwind_runtime_call(acq, LTy::I32, vec![gp]);
            let init = self.f.fir.create_block();
            let z = V::C(self.m.const_int(LTy::I32, 0));
            let t = self.icmp(IntPredicate::Ne, r, z);
            self.cond_br(t, init, end);
            self.push_cleanup_kind(Cleanup::GuardAbort { guard: gp }, false, true);
            self.emit_block(init, false);
        } else if !local {
            // Set before the initialization so that references to the
            // variable during it do not restart it.
            let one = V::C(self.m.const_int(LTy::I8, 1));
            self.store(one, gp, galign, false);
        }
        self.global_var_decl_init(d, g, perform)?;
        if threadsafe {
            self.pop_cleanup_block(false);
            let rel = self.rt_fn("__cxa_guard_release", LTy::VOID, &[LTy::PTR], &nounwind);
            self.nounwind_runtime_call(rel, LTy::VOID, vec![gp]);
        } else if local {
            let one = V::C(self.m.const_int(LTy::I8, 1));
            self.store(one, gp, galign, false);
        }
        self.emit_block(end, false);
        Ok(())
    }

    /// The `GuardAbort` cleanup: `__cxa_guard_abort` on the exceptional
    /// edge of a guarded initialization ([stmt.dcl]/3: the next pass
    /// retries).
    pub(crate) fn emit_guard_abort(&mut self, guard: V) {
        let nounwind = [Attr::Flag(AttrKind::NoUnwind)];
        let f = self.rt_fn("__cxa_guard_abort", LTy::VOID, &[LTy::PTR], &nounwind);
        self.nounwind_runtime_call(f, LTy::VOID, vec![guard]);
    }

    // ---- lifetime-extended temporaries ---------------------------------

    /// The `_ZGR` global of lifetime-extended temporary `t` of type `ty`
    /// (clang `GetAddrOfGlobalTemporary`): holding its constant value
    /// when it has one, else zero until the extending declaration's
    /// initialization stores it.
    pub(crate) fn let_global(&mut self, t: DeclId, ty: QualType) -> R<GlobalId> {
        if let Some(&g) = self.gi.temps.get(&t) {
            return Ok(g);
        }
        let ast = self.ast;
        let loc = Some(ast.decls.get(t).loc());
        let Some(l) = ast.decls.lifetime_extended(t).copied() else {
            return err(loc, "temporary without an extending declaration");
        };
        let vfirst = ast.first(l.extending);
        let local_index = self.gi.local_index.get(&vfirst).copied().unwrap_or(0);
        let name = mangle::mangle_ref_temporary(
            ast,
            &mut self.anon,
            vfirst,
            local_index,
            l.mangling_index,
        );
        let init = match l.value {
            Some(c) => Some(self.const_value(c, ty, None, true)?),
            None => None,
        };
        let lt = match init {
            Some(k) => self.m.const_type(k),
            None => self.mty(ty),
        };
        // The extending variable's definition linkage; an external one
        // becomes internal.
        let lin = if let Some(&sg) = self.static_locals.get(&vfirst) {
            self.m.global(sg).linkage
        } else if mangle::is_internal_symbol(ast, vfirst) {
            Linkage::Internal
        } else if let Some(l) = self.var_instantiation_linkage(vfirst) {
            l
        } else if self.var_is_inline(vfirst) {
            Linkage::LinkOnceOdr
        } else {
            Linkage::Internal
        };
        let g = self.m.add_global(&name, lt);
        self.gi.temps.insert(t, g);
        let value = match init {
            Some(k) => k,
            None => self.m.const_zero(lt),
        };
        if let Err(e) = self.m.set_initializer(g, Some(value)) {
            return err(loc, format!("bad initializer: {e:?}"));
        }
        let weak = matches!(
            lin,
            Linkage::Weak | Linkage::WeakOdr | Linkage::LinkOnce | Linkage::LinkOnceOdr
        );
        let comdat = weak.then(|| self.m.comdat(&name, ComdatKind::Any));
        let align = self.size_align(ty).1;
        let gv = self.m.global_mut(g);
        gv.linkage = lin;
        gv.align = Some(align);
        gv.comdat = comdat;
        // `setGVProperties` runs on the global before it has an
        // initializer unless the value is constant.
        gv.dso_local = !matches!(lin, Linkage::Internal | Linkage::Private) && init.is_some();
        Ok(g)
    }

    /// The address of static-storage temporary `m` of type `sq` and
    /// whether it still needs initializing (clang
    /// `createReferenceTemporary` for `SD_Static` and the
    /// `hasInitializer` test of `EmitMaterializeTemporaryExpr`).
    pub(crate) fn static_temporary(&mut self, m: ExprId, sq: QualType) -> R<(Addr, bool)> {
        let ast = self.ast;
        let Some(t) = ast.exprs.lifetime_extended_temporary(m) else {
            return err(self.loc_of(m), "static temporary without an extending declaration");
        };
        let fresh = !self.gi.temps.contains_key(&t);
        let has_value = ast.decls.lifetime_extended(t).is_some_and(|l| l.value.is_some());
        let g = self.let_global(t, sq)?;
        let ty = self.mty(sq);
        let align = self.size_align(sq).1;
        Ok((Addr { v: V::C(gval(g)), ty, align }, fresh && !has_value))
    }

    /// clang `pushTemporaryCleanup` for `SD_Static`: the destructor of a
    /// class temporary `m` at `a` runs at exit (an array temporary's
    /// elements are not destroyed, as clang).
    pub(crate) fn register_temporary_dtor(&mut self, m: ExprId, a: Addr) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(m).ty();
        if self.array_elem(q).is_some() {
            return Ok(());
        }
        match self.destructed_class(q) {
            Some(rd) => self.register_global_dtor(rd, a.v),
            None => Ok(()),
        }
    }

    // ---- the module trailer --------------------------------------------

    /// clang `EmitCXXGlobalInitFunc` and `EmitCtorList`: the
    /// `_GLOBAL__sub_I_<file>` function running the ordered initializers,
    /// `@llvm.global_ctors` and `@llvm.used`.
    pub(crate) fn emit_global_init_trailer(&mut self) -> R<()> {
        while matches!(self.gi.ordered.last(), Some(None)) {
            self.gi.ordered.pop();
        }
        if !self.gi.ordered.is_empty() {
            let name =
                format!("_GLOBAL__sub_I_{}", transformed_file_name(&self.opts.source_filename));
            let f = self.init_function(&name)?;
            let saved = std::mem::replace(&mut self.f, Fx::empty());
            self.start_init_fn(f);
            let fns: Vec<FuncId> = self.gi.ordered.iter().flatten().copied().collect();
            for callee in fns {
                let fn_ty = self.m.func(callee).ty();
                self.f.fir.push(
                    Op::Call {
                        fn_ty,
                        callee: V::C(Value::func(callee)),
                        args: Vec::new(),
                        opts: Box::new(CallOpts::default()),
                    },
                    LTy::VOID,
                );
            }
            let r = self.finish_init_fn(f, self.ast.tu);
            self.f = saved;
            r?;
            self.gi.ctors.push((DONE, f, None));
        }
        if self.gi.ctors.is_empty() {
            return Ok(());
        }
        let mut ctors = std::mem::take(&mut self.gi.ctors);
        ctors.sort_by_key(|&(lex, _, _)| lex);
        let sty = self.m.types.struct_literal(&[LTy::I32, LTy::PTR, LTy::PTR], false);
        let mut elems = Vec::with_capacity(ctors.len());
        for (_, f, key) in ctors {
            let prio = self.m.const_int(LTy::I32, 65535);
            let data = match key {
                Some(k) => gval(k),
                None => self.m.const_null(LTy::PTR),
            };
            match self.m.const_struct(sty, &[prio, Value::func(f), data]) {
                Ok(c) => elems.push(c),
                Err(e) => return err(None, format!("bad llvm.global_ctors entry: {e:?}")),
            }
        }
        self.appending_global("llvm.global_ctors", sty, &elems, None)?;
        let used = std::mem::take(&mut self.gi.used);
        if !used.is_empty() {
            let elems: Vec<Value> = used.into_iter().map(gval).collect();
            self.appending_global("llvm.used", LTy::PTR, &elems, Some("llvm.metadata"))?;
        }
        Ok(())
    }

    /// An `appending` array global of `elems` of type `elem`.
    fn appending_global(
        &mut self,
        name: &str,
        elem: LTy,
        elems: &[Value],
        section: Option<&str>,
    ) -> R<()> {
        let init = match self.m.const_array(elem, elems) {
            Ok(c) => c,
            Err(e) => return err(None, format!("bad @{name}: {e:?}")),
        };
        let ty = self.m.const_type(init);
        let g = self.m.add_global(name, ty);
        if let Err(e) = self.m.set_initializer(g, Some(init)) {
            return err(None, format!("bad @{name}: {e:?}"));
        }
        let gv = self.m.global_mut(g);
        gv.linkage = Linkage::Appending;
        gv.section = section.map(Into::into);
        Ok(())
    }
}
