//! `thread_local` variables ([basic.stc.thread]; spec/codegen.md §7f):
//! clang's Itanium C++ ABI thread-local support (`ItaniumCXXABI::
//! EmitThreadLocalInitFuncs`, `EmitThreadLocalVarDeclLValue`).
//!
//! A `thread_local` global is an LLVM `thread_local` global (general
//! dynamic model). Its dynamic initializer is an ordinary
//! `__cxx_global_var_init[.N]` that is called, instead of at startup,
//! from the internal `__tls_init`, guarded by the thread-local i8
//! `__tls_guard` ([basic.start.dynamic]/7: before the first odr-use in
//! each thread); destructors register with `__cxa_thread_atexit`
//! ([basic.start.term]/1). A variable that is not constant-initialized,
//! needs destruction, or whose initializer is not visible is accessed
//! through its wrapper `_ZTW<name>`, which calls the variable's
//! initialization function `_ZTH<name>` (an alias of `__tls_init` where
//! the variable is defined, an `extern_weak` declaration elsewhere) and
//! returns the variable's address; every other access computes the
//! address with `@llvm.threadlocal.address`. A function-local
//! `thread_local` is guarded in place by its own thread-local `_ZGV`
//! byte, without `__cxa_guard_*` ([stmt.dcl]/3).

use kiln_abi::mangle;
use kiln_ast::DeclId;
use kiln_ast::decl::{PayloadClass, TlsKind, VarStorage};
use kiln_llvm::{
    Attr, AttrKind, AttrList, CallOpts, ComdatKind, FuncId, GlobalId, IntPredicate, IrBuilder,
    Linkage, TlsModel, TypeId as LTy, Value, Visibility,
};

use crate::fir::{Op, V};
use crate::module::{Cg, R, err, gval};

impl Cg<'_> {
    /// Whether variable `d` has thread storage duration.
    pub(crate) fn var_is_tls(&self, d: DeclId) -> bool {
        self.ast.decls.var(d).is_some_and(|v| v.spec.tls() != TlsKind::None)
    }

    /// Makes global `g` of variable `d` thread-local; a namespace-scope
    /// variable joins clang's `CXXThreadLocals` (in creation order).
    pub(crate) fn mark_tls(&mut self, d: DeclId, g: GlobalId, local: bool) {
        if !self.var_is_tls(d) {
            return;
        }
        self.m.global_mut(g).thread_local = Some(TlsModel::GeneralDynamic);
        let first = self.ast.first(d);
        // Only `thread_local` (clang `TLS_Dynamic`) variables get `_ZTW`
        // wrappers; `__thread` ones are always accessed directly.
        if !local && self.is_dynamic_tls(first) && !self.gi.tls_vars.contains(&first) {
            self.gi.tls_vars.push(first);
        }
    }

    /// The defining declaration of namespace-scope or static data member
    /// variable `first` (clang `getInitializingDeclaration`'s candidate):
    /// one with an initializer, or one that is not `extern` and, in a
    /// class, `inline`.
    fn tls_var_def(&self, first: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        ast.redecls_of(first).find(|&r| {
            let Some(v) = ast.decls.var(r) else { return false };
            if v.init.is_some() {
                return true;
            }
            if v.spec.storage() == VarStorage::Extern || ast.decls.is_single_line_linkage(r) {
                return false;
            }
            let in_class = ast
                .decls
                .get(r)
                .ctx()
                .is_some_and(|c| ast.decls.get(c).kind().payload() == PayloadClass::Record);
            !in_class || self.var_is_inline(r)
        })
    }

    /// Whether this translation unit defines `first` (an imported module
    /// unit's non-discardable definition belongs to that unit).
    fn tls_defined_here(&self, first: DeclId) -> bool {
        self.tls_var_def(first).is_some_and(|def| {
            self.discardable_var(def) || !self.ast.modules.is_imported_named_unit_decl(def)
        })
    }

    /// Whether thread-local variable `d` is `thread_local` (clang
    /// `TLS_Dynamic`) rather than `__thread` / `_Thread_local`.
    fn is_dynamic_tls(&self, d: DeclId) -> bool {
        self.ast.decls.var(d).is_some_and(|v| v.spec.tls() == TlsKind::Dynamic)
    }

    /// clang `EmitGlobalVarDeclLValue` + `usesThreadWrapperFunction`: a
    /// `thread_local` variable that may need destruction or is not known
    /// to be constant-initialized (never a `__thread` one).
    pub(crate) fn uses_thread_wrapper(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let first = ast.first(d);
        // An `alias` variable is accessed directly, as clang does (the
        // aliasee's initialization is not this declaration's).
        if !self.is_dynamic_tls(first) || ast.alias_definition(first).is_some() {
            return false;
        }
        let Some(ty) = ast.decls.get(first).ty() else { return true };
        if !ast.types.is_reference(ty) && self.var_destructed_class(first, ty).is_some() {
            return true;
        }
        if ast.decls.var(ast.latest(first)).is_some_and(|v| v.spec.is_constinit()) {
            return false;
        }
        if self.tls_var_def(first).is_none() {
            return true;
        }
        let init = ast
            .redecls_of(first)
            .find_map(|r| ast.decls.var(r).and_then(|v| v.init))
            .filter(|&i| !ast.is_trivial_default_construction(i));
        init.is_some_and(|i| self.folds.get(i).is_none())
    }

    /// `Builder.CreateThreadLocalAddress(g)` in the current function:
    /// `call align N ptr @llvm.threadlocal.address.p0(ptr align N @g)`.
    pub(crate) fn threadlocal_address(&mut self, g: GlobalId) -> V {
        let align = self.m.global(g).align.unwrap_or(1);
        let f = kiln_llvm::intrinsics::threadlocal_address(&mut self.m);
        let a = self.m.attr_set(&[Attr::Align(align)]);
        let opts = CallOpts {
            attrs: AttrList { ret: a, params: vec![a], ..AttrList::default() },
            ..CallOpts::default()
        };
        let fn_ty = self.m.func(f).ty();
        let r = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(f)),
                args: vec![V::C(gval(g))],
                opts: Box::new(opts),
            },
            LTy::PTR,
        );
        self.f.calls.push((crate::fir::Fir::last_inst(r), Some(f), true));
        r
    }

    /// The thread-local global a scalar load or store addresses directly
    /// (clang `EmitLoadOfScalar`/`EmitStoreOfScalar` on a thread-local
    /// `GlobalValue`), else `None`.
    pub(crate) fn direct_tls_global(&self, p: V) -> Option<GlobalId> {
        let V::C(c) = p else { return None };
        let kiln_llvm::ValueRef::Global(g) = c.get() else { return None };
        self.m.global(g).thread_local.is_some().then_some(g)
    }

    /// Pointer `p` of a scalar load or store, through
    /// `@llvm.threadlocal.address` when it is a thread-local global.
    pub(crate) fn scalar_tls_addr(&mut self, p: V) -> V {
        match self.direct_tls_global(p) {
            Some(g) => self.threadlocal_address(g),
            None => p,
        }
    }

    /// The address of thread-local namespace-scope variable `d` with
    /// global `g` in the current thread: through its wrapper when
    /// [`Self::uses_thread_wrapper`] (for a reference, the referee's
    /// address; `true`), else `@llvm.threadlocal.address` (`false`).
    pub(crate) fn tls_var_addr(&mut self, d: DeclId, g: GlobalId) -> R<(V, bool)> {
        if !self.uses_thread_wrapper(d) {
            return Ok((self.threadlocal_address(g), false));
        }
        let w = self.tls_wrapper(d)?;
        let fn_ty = self.m.func(w).ty();
        let r = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(w)),
                args: Vec::new(),
                opts: Box::default(),
            },
            LTy::PTR,
        );
        self.f.calls.push((crate::fir::Fir::last_inst(r), Some(w), false));
        Ok((r, true))
    }

    /// clang `getOrCreateThreadLocalWrapper`: `ptr @_ZTW<name>()`,
    /// internal for an internal variable, else `weak_odr hidden` in its
    /// own comdat (made `linkonce_odr` at the end of the translation
    /// unit when the variable is not defined here).
    fn tls_wrapper(&mut self, d: DeclId) -> R<FuncId> {
        let ast = self.ast;
        let first = ast.first(d);
        if let Some(&(_, w)) = self.gi.tls_wrappers.iter().find(|x| x.0 == first) {
            return Ok(w);
        }
        let name = mangle::mangle_thread_local(ast, &mut self.anon, first, "_ZTW");
        let fty = self.m.types.function(LTy::PTR, &[], false);
        let w = match self.m.add_function(&name, fty) {
            Ok(w) => w,
            Err(e) => {
                return err(Some(ast.decls.get(d).loc()), format!("bad thread wrapper: {e:?}"));
            }
        };
        let internal = mangle::is_internal_symbol(ast, first);
        let comdat = (!internal).then(|| self.m.comdat(&name, ComdatKind::Any));
        let ret = self.m.attr_set(&[Attr::Flag(AttrKind::NoUndef)]);
        let mut fa = Vec::new();
        self.target_attrs(&mut fa);
        let fa = self.m.attr_set(&fa);
        let wf = self.m.func_mut(w);
        wf.attrs.ret = ret;
        wf.attrs.func = fa;
        if internal {
            wf.linkage = Linkage::Internal;
        } else {
            wf.linkage = Linkage::WeakOdr;
            wf.visibility = Visibility::Hidden;
            wf.comdat = comdat;
        }
        self.nounwind.insert(w, !self.opts.exceptions);
        self.gi.tls_wrappers.push((first, w));
        Ok(w)
    }

    /// clang `EmitCXXThreadLocalInitFunc`: `__tls_init` running the
    /// ordered thread-local initializers, the wrappers of the
    /// non-discardable thread-local variables defined here, and every
    /// wrapper's body with its `_ZTH` initialization function.
    pub(crate) fn emit_tls_trailer(&mut self) -> R<()> {
        let ast = self.ast;
        let inits = std::mem::take(&mut self.gi.tls_inits);
        let mut ordered = Vec::new();
        let mut unordered = Vec::new();
        for (f, d) in inits {
            if self.var_instantiation_linkage(ast.first(d)).is_some() {
                unordered.push((ast.first(d), f));
            } else {
                ordered.push(f);
            }
        }
        let mut tls_init = None;
        if !ordered.is_empty() {
            let f = self.tls_init_function()?;
            let guard = self.m.add_global("__tls_guard", LTy::I8);
            let zero = self.m.const_int(LTy::I8, 0);
            if let Err(e) = self.m.set_initializer(guard, Some(zero)) {
                return err(None, format!("bad __tls_guard: {e:?}"));
            }
            let gm = self.m.global_mut(guard);
            gm.linkage = Linkage::Internal;
            gm.align = Some(1);
            gm.thread_local = Some(TlsModel::GeneralDynamic);
            self.emit_guarded_init_fn(f, guard, &ordered)?;
            tls_init = Some(f);
        }
        let vars = self.gi.tls_vars.clone();
        for first in vars {
            if self.tls_defined_here(first)
                && self.tls_var_def(first).is_some_and(|def| !self.discardable_var(def))
            {
                self.tls_wrapper(first)?;
            }
        }
        let wrappers = self.gi.tls_wrappers.clone();
        for (first, w) in wrappers {
            let init = unordered.iter().find(|x| x.0 == first).map(|x| x.1).or(tls_init);
            self.emit_tls_wrapper_body(first, w, init)?;
        }
        Ok(())
    }

    /// The internal `void()` `__tls_init` (clang
    /// `CreateGlobalInitOrCleanUpFunction` with `TLS`: no section).
    fn tls_init_function(&mut self) -> R<FuncId> {
        let fty = self.m.types.function(LTy::VOID, &[], false);
        let f = match self.m.add_function("__tls_init", fty) {
            Ok(f) => f,
            Err(e) => return err(None, format!("bad __tls_init: {e:?}")),
        };
        self.m.func_mut(f).linkage = Linkage::Internal;
        Ok(f)
    }

    /// The body of wrapper `w` of variable `first`, whose initialization
    /// function where it is defined here is `init`.
    fn emit_tls_wrapper_body(&mut self, first: DeclId, w: FuncId, init: Option<FuncId>) -> R<()> {
        let ast = self.ast;
        let loc = Some(ast.decls.get(first).loc());
        let g = self.get_gvar(first)?;
        let defined = self.tls_defined_here(first);
        if !defined && self.m.func(w).linkage == Linkage::WeakOdr {
            self.m.func_mut(w).linkage = Linkage::LinkOnceOdr;
        }
        // SetLLVMFunctionAttributesForDefinition (no CodeGenFunction: no
        // "min-legal-vector-width"); `noinline` only at -O0.
        let mut fa = Vec::new();
        if !self.opts.optimize() {
            fa.push(Attr::Flag(AttrKind::NoInline));
        }
        if !self.opts.exceptions {
            fa.push(Attr::Flag(AttrKind::NoUnwind));
        }
        self.def_attrs(&mut fa);
        self.target_attrs(&mut fa);
        let fa = self.m.attr_set(&fa);
        self.m.func_mut(w).attrs.func = fa;
        let name = mangle::mangle_thread_local(ast, &mut self.anon, first, "_ZTH");
        let void_fn = self.m.types.function(LTy::VOID, &[], false);
        let (lin, vis, dso) = {
            let v = self.m.global(g);
            (v.linkage, v.visibility, v.dso_local)
        };
        // None: constant initialization; Some(true): call the alias;
        // Some(false): call it if the weak declaration resolved.
        let call: Option<(Value, bool)> = if !self.uses_thread_wrapper(first) {
            None
        } else if defined {
            match init {
                Some(f) => {
                    let a = match self.m.add_alias(&name, void_fn, Value::func(f)) {
                        Ok(a) => a,
                        Err(e) => return err(loc, format!("bad thread-local init alias: {e:?}")),
                    };
                    let am = self.m.alias_mut(a);
                    am.linkage = lin;
                    am.visibility = vis;
                    am.dso_local = dso;
                    Some((Value::alias(a), true))
                }
                None => None,
            }
        } else {
            let f = match self.m.add_function(&name, void_fn) {
                Ok(f) => f,
                Err(e) => return err(loc, format!("bad thread-local init declaration: {e:?}")),
            };
            let mut da = Vec::new();
            self.target_attrs(&mut da);
            let da = self.m.attr_set(&da);
            let fm = self.m.func_mut(f);
            fm.linkage = Linkage::ExternWeak;
            fm.attrs.func = da;
            fm.visibility = vis;
            fm.dso_local = dso;
            Some((Value::func(f), false))
        };
        let align = self.var_align(first);
        let is_ref = ast.decls.get(first).ty().is_some_and(|t| ast.types.is_reference(t));
        let tla = kiln_llvm::intrinsics::threadlocal_address(&mut self.m);
        let a = self.m.attr_set(&[Attr::Align(align)]);
        let tl_opts = CallOpts {
            attrs: AttrList { ret: a, params: vec![a], ..AttrList::default() },
            ..CallOpts::default()
        };
        let tla_ty = self.m.func(tla).ty();
        let null = self.m.const_null(LTy::PTR);
        let mut b = IrBuilder::new(&mut self.m, w);
        let r = (|| {
            let entry = b.append_block("");
            b.position_at_end(entry);
            match call {
                None => {}
                Some((callee, true)) => {
                    b.call_ex(void_fn, callee, &[], &CallOpts::default())?;
                }
                Some((callee, false)) => {
                    let have = b.icmp(IntPredicate::Ne, callee, null)?;
                    let ib = b.append_block("");
                    let xb = b.append_block("");
                    b.cond_br(have, ib, xb)?;
                    b.position_at_end(ib);
                    b.call_ex(void_fn, callee, &[], &CallOpts::default())?;
                    b.br(xb)?;
                    b.position_at_end(xb);
                }
            }
            let mut v = b.call_ex(tla_ty, Value::func(tla), &[gval(g)], &tl_opts)?;
            if is_ref {
                v = b.load(LTy::PTR, v, align)?;
            }
            b.ret(Some(v))
        })();
        if let Err(e) = r {
            return err(loc, format!("bad thread wrapper body: {e:?}"));
        }
        Ok(())
    }
}
