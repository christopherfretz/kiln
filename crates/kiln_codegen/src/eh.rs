//! Exception handling (spec/codegen.md §7b, WP-100): ports of clang's
//! `CGException.cpp` and of the Itanium parts of `ItaniumCXXABI.cpp`
//! (landing pads, EH dispatch, terminate scopes, the call-or-invoke
//! choice of `EmitCall`).

use kiln_llvm::{
    Attr, AttrKind, AttrList, CallOpts, ComdatKind, FuncId, IrBuilder, Linkage, TypeId as LTy,
    UwTableKind, Value, Visibility,
};

use crate::cleanup::{Depth, Kind};
use crate::fir::{Blk, Fir, Op, V};
use crate::module::Cg;

impl Cg<'_> {
    /// `getExceptionSlot`.
    pub(crate) fn exn_slot(&mut self) -> V {
        if let Some(s) = self.f.cl.exn_slot {
            return s;
        }
        let s = self.f.fir.alloca(LTy::PTR, 8);
        self.f.cl.exn_slot = Some(s);
        s
    }

    /// `getEHSelectorSlot`.
    pub(crate) fn sel_slot(&mut self) -> V {
        if let Some(s) = self.f.cl.sel_slot {
            return s;
        }
        let s = self.f.fir.alloca(LTy::I32, 4);
        self.f.cl.sel_slot = Some(s);
        s
    }

    /// `getExceptionFromSlot`.
    pub(crate) fn exn_from_slot(&mut self) -> V {
        let s = self.exn_slot();
        self.load(LTy::PTR, s, 8, false)
    }

    /// A runtime function (clang `CreateRuntimeFunction`): declared once,
    /// with function attributes `fa`.
    pub(crate) fn rt_fn(&mut self, name: &str, ret: LTy, params: &[LTy], fa: &[Attr]) -> FuncId {
        if let Some(f) = self.m.get_function(name) {
            return f;
        }
        let ty = self.m.types.function(ret, params, false);
        let f = self.m.add_function(name, ty).expect("a function type");
        let a = self.m.attr_set(fa);
        self.m.func_mut(f).attrs.func = a;
        f
    }

    /// `CurFn->setPersonalityFn(getOpaquePersonalityFn(..))` with the
    /// GNU C++ personality.
    fn set_personality(&mut self) {
        let f = self.f.func.expect("current function");
        if self.m.func(f).personality.is_some() {
            return;
        }
        let p = match self.m.get_function("__gxx_personality_v0") {
            Some(p) => p,
            None => {
                let ty = self.m.types.function(LTy::I32, &[], true);
                self.m.add_function("__gxx_personality_v0", ty).expect("a function type")
            }
        };
        self.m.func_mut(f).personality = Some(Value::func(p));
    }

    /// `getInvokeDest`: the landing pad of the current scope, `None` when
    /// a call cannot unwind into anything.
    pub(crate) fn invoke_dest(&mut self) -> Option<Blk> {
        if !self.opts.exceptions || self.f.cl.innermost_eh() == 0 {
            return None;
        }
        if let Some(lp) = self.f.cl.stack.last().and_then(|s| s.cached_lpad) {
            return Some(lp);
        }
        self.set_personality();
        let lp = self.emit_landing_pad();
        for s in self.f.cl.stack.iter_mut().rev() {
            s.cached_lpad = Some(lp);
            if s.is_eh_scope() {
                break;
            }
        }
        Some(lp)
    }

    /// `EmitLandingPad`.
    fn emit_landing_pad(&mut self) -> Blk {
        let ie = self.f.cl.innermost_eh();
        match &self.f.cl.stack[ie - 1].kind {
            Kind::Terminate => return self.terminate_lpad(),
            _ => {
                if let Some(lp) = self.f.cl.stack[ie - 1].cached_lpad {
                    return lp;
                }
            }
        }
        let saved = self.f.fir.cur();
        self.f.fir.set_cur(None);
        let lpad = self.f.fir.create_block();
        self.emit_block(lpad, false);
        // Clauses: walk the scopes from the innermost out.
        let mut catch_all = false;
        let mut cleanup = false;
        let mut clauses: Vec<V> = Vec::new();
        'walk: for s in self.f.cl.stack.iter().rev() {
            match &s.kind {
                Kind::Cleanup(_) => cleanup = cleanup || s.eh,
                Kind::Terminate => {
                    catch_all = true;
                    break 'walk;
                }
                Kind::Catch(hs) => {
                    for h in hs {
                        let Some(t) = h.rtti else {
                            catch_all = true;
                            break 'walk;
                        };
                        if !clauses.contains(&t) {
                            clauses.push(t);
                        }
                    }
                }
            }
        }
        if catch_all {
            clauses.push(V::C(self.m.const_null(LTy::PTR)));
        }
        let lp_ty = self.lpad_ty();
        let lp = self.f.fir.push(
            Op::LandingPad { cleanup: !catch_all && cleanup, clauses },
            lp_ty,
        );
        let exn = self.f.fir.push(Op::ExtractValue { agg: lp, idx: 0 }, LTy::PTR);
        let es = self.exn_slot();
        self.store(exn, es, 8, false);
        let sel = self.f.fir.push(Op::ExtractValue { agg: lp, idx: 1 }, LTy::I32);
        let ss = self.sel_slot();
        self.store(sel, ss, 4, false);
        let next = self.eh_dispatch_block(ie);
        self.br(next);
        self.f.fir.set_cur(saved);
        lpad
    }

    /// `{ ptr, i32 }`.
    fn lpad_ty(&mut self) -> LTy {
        self.m.types.struct_literal(&[LTy::PTR, LTy::I32], false)
    }

    /// `getEHDispatchBlock`.
    pub(crate) fn eh_dispatch_block(&mut self, d: Depth) -> Blk {
        if d == 0 {
            return self.eh_resume_block();
        }
        if let Some(b) = self.f.cl.stack[d - 1].cached_eh {
            return b;
        }
        let b = match &self.f.cl.stack[d - 1].kind {
            Kind::Catch(hs) if hs.len() == 1 && hs[0].rtti.is_none() => hs[0].blk,
            Kind::Catch(_) | Kind::Cleanup(_) => self.f.fir.create_block(),
            Kind::Terminate => self.terminate_handler(),
        };
        self.f.cl.stack[d - 1].cached_eh = Some(b);
        b
    }

    /// `getEHResumeBlock`: re-raises the in-flight exception.
    fn eh_resume_block(&mut self) -> Blk {
        if let Some(b) = self.f.cl.eh_resume {
            return b;
        }
        let saved = self.f.fir.cur();
        let b = self.f.fir.create_block();
        self.f.fir.set_cur(Some(b));
        let exn = self.exn_from_slot();
        let ss = self.sel_slot();
        let sel = self.load(LTy::I32, ss, 4, false);
        let lp_ty = self.lpad_ty();
        let p = V::C(self.m.const_poison(lp_ty));
        let v = self.f.fir.push(Op::InsertValue { agg: p, val: exn, idx: 0 }, lp_ty);
        let v = self.f.fir.push(Op::InsertValue { agg: v, val: sel, idx: 1 }, lp_ty);
        self.f.fir.push(Op::Resume { v }, LTy::VOID);
        self.f.fir.set_cur(saved);
        self.f.cl.eh_resume = Some(b);
        b
    }

    /// `call void @__clang_call_terminate(ptr %exn) noreturn nounwind`
    /// (`emitTerminateForUnexpectedException`), then `unreachable`.
    fn call_terminate(&mut self, exn: V) {
        let t = self.clang_call_terminate_fn();
        let fn_ty = self.m.func(t).ty();
        let fa = self.m.attr_set(&[Attr::Flag(AttrKind::NoReturn), Attr::Flag(AttrKind::NoUnwind)]);
        let opts =
            CallOpts { attrs: AttrList { func: fa, ..AttrList::default() }, ..CallOpts::default() };
        let c = self.f.fir.push(
            Op::Call { fn_ty, callee: V::C(Value::func(t)), args: vec![exn], opts: Box::new(opts) },
            LTy::VOID,
        );
        self.f.calls.push((Fir::last_inst(c), Some(t), true));
        self.f.fir.push(Op::Unreachable, LTy::VOID);
    }

    /// `getTerminateLandingPad`.
    fn terminate_lpad(&mut self) -> Blk {
        if let Some(b) = self.f.cl.terminate_lpad {
            return b;
        }
        let saved = self.f.fir.cur();
        let b = self.f.fir.create_block();
        self.f.fir.set_cur(Some(b));
        self.set_personality();
        let lp_ty = self.lpad_ty();
        let null = V::C(self.m.const_null(LTy::PTR));
        let lp = self.f.fir.push(Op::LandingPad { cleanup: false, clauses: vec![null] }, lp_ty);
        let exn = self.f.fir.push(Op::ExtractValue { agg: lp, idx: 0 }, LTy::PTR);
        self.call_terminate(exn);
        self.f.fir.set_cur(saved);
        self.f.cl.terminate_lpad = Some(b);
        b
    }

    /// `getTerminateHandler`.
    fn terminate_handler(&mut self) -> Blk {
        if let Some(b) = self.f.cl.terminate_handler {
            return b;
        }
        let saved = self.f.fir.cur();
        let b = self.f.fir.create_block();
        self.f.fir.set_cur(Some(b));
        let exn = self.exn_from_slot();
        self.call_terminate(exn);
        self.f.fir.set_cur(saved);
        self.f.cl.terminate_handler = Some(b);
        b
    }

    /// `getClangCallTerminateFn`: `__clang_call_terminate(ptr)` calls
    /// `__cxa_begin_catch` then `std::terminate`.
    fn clang_call_terminate_fn(&mut self) -> FuncId {
        const NAME: &str = "__clang_call_terminate";
        if let Some(f) = self.m.get_function(NAME) {
            return f;
        }
        let ty = self.m.types.function(LTy::VOID, &[LTy::PTR], false);
        let f = self.m.add_function(NAME, ty).expect("a function type");
        let mut fa = vec![
            Attr::Flag(AttrKind::NoInline),
            Attr::Flag(AttrKind::NoReturn),
            Attr::Flag(AttrKind::NoUnwind),
            Attr::UwTable(UwTableKind::Async),
        ];
        self.target_attrs(&mut fa);
        let fa = self.m.attr_set(&fa);
        let pa = self.m.attr_set(&[Attr::Flag(AttrKind::NoUndef)]);
        let comdat = self.m.comdat(NAME, ComdatKind::Any);
        {
            let func = self.m.func_mut(f);
            func.attrs.func = fa;
            func.attrs.params = vec![pa];
            func.linkage = Linkage::LinkOnceOdr;
            func.visibility = Visibility::Hidden;
            func.dso_local = false;
            func.comdat = Some(comdat);
        }
        self.nounwind.insert(f, true);
        let begin = self.rt_fn("__cxa_begin_catch", LTy::PTR, &[LTy::PTR], &[]);
        let term = self.rt_fn("_ZSt9terminatev", LTy::VOID, &[], &[]);
        let nu = self.m.attr_set(&[Attr::Flag(AttrKind::NoUnwind)]);
        let nr = self.m.attr_set(&[Attr::Flag(AttrKind::NoReturn), Attr::Flag(AttrKind::NoUnwind)]);
        let begin_ty = self.m.func(begin).ty();
        let term_ty = self.m.func(term).ty();
        let mut b = IrBuilder::new(&mut self.m, f);
        let entry = b.append_block("");
        b.position_at_end(entry);
        let opts = |func| CallOpts { attrs: AttrList { func, ..AttrList::default() }, ..CallOpts::default() };
        let r = b
            .call_ex(begin_ty, Value::func(begin), &[Value::arg(0)], &opts(nu))
            .and_then(|_| b.call_ex(term_ty, Value::func(term), &[], &opts(nr)))
            .and_then(|_| b.unreachable());
        if let Err(e) = r {
            self.f.error.get_or_insert(crate::CodegenError {
                loc: None,
                msg: format!("cannot build `{NAME}`: {e:?}"),
            });
        }
        f
    }

    /// clang `EmitCall`'s call-or-invoke emission: an `invoke` when the
    /// call may throw and an EH scope is active, else a `call`. The call
    /// cannot throw when its site is `nounwind` (`site_nounwind`) or the
    /// callee is known `nounwind` now.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_call_op(
        &mut self,
        fn_ty: LTy,
        callee: V,
        args: Vec<V>,
        opts: CallOpts,
        rty: LTy,
        fid: Option<FuncId>,
        site_nounwind: bool,
    ) -> V {
        let cannot_throw =
            site_nounwind || fid.is_some_and(|f| self.nounwind.get(&f).copied().unwrap_or(false));
        let dest = if cannot_throw { None } else { self.invoke_dest() };
        let r = self.call_or_invoke_to(fn_ty, callee, args, opts, rty, dest);
        self.f.calls.push((Fir::last_inst(r), fid, site_nounwind));
        r
    }

    fn call_or_invoke_to(
        &mut self,
        fn_ty: LTy,
        callee: V,
        args: Vec<V>,
        opts: CallOpts,
        rty: LTy,
        dest: Option<Blk>,
    ) -> V {
        match dest {
            None => self.f.fir.push(Op::Call { fn_ty, callee, args, opts: Box::new(opts) }, rty),
            Some(unwind) => {
                let normal = self.f.fir.create_block();
                let r = self.f.fir.push(
                    Op::Invoke { fn_ty, callee, args, opts: Box::new(opts), normal, unwind },
                    rty,
                );
                self.emit_block(normal, false);
                r
            }
        }
    }

    /// `EmitRuntimeCallOrInvoke`: a call of runtime function `f` with
    /// call-site attributes `site`, an `invoke` whenever an EH scope is
    /// active.
    pub(crate) fn runtime_call_or_invoke(
        &mut self,
        f: FuncId,
        ret: LTy,
        args: Vec<V>,
        site: &[AttrKind],
    ) -> V {
        let fn_ty = self.m.func(f).ty();
        let attrs: Vec<Attr> = site.iter().map(|&k| Attr::Flag(k)).collect();
        let fa = self.m.attr_set(&attrs);
        let opts =
            CallOpts { attrs: AttrList { func: fa, ..AttrList::default() }, ..CallOpts::default() };
        let dest = self.invoke_dest();
        let r = self.call_or_invoke_to(fn_ty, V::C(Value::func(f)), args, opts, ret, dest);
        self.f.calls.push((Fir::last_inst(r), Some(f), site.contains(&AttrKind::NoUnwind)));
        r
    }

    /// `EmitNounwindRuntimeCall`.
    pub(crate) fn nounwind_runtime_call(&mut self, f: FuncId, ret: LTy, args: Vec<V>) -> V {
        let fn_ty = self.m.func(f).ty();
        let fa = self.m.attr_set(&[Attr::Flag(AttrKind::NoUnwind)]);
        let opts =
            CallOpts { attrs: AttrList { func: fa, ..AttrList::default() }, ..CallOpts::default() };
        let r = self.f.fir.push(
            Op::Call { fn_ty, callee: V::C(Value::func(f)), args, opts: Box::new(opts) },
            ret,
        );
        self.f.calls.push((Fir::last_inst(r), Some(f), true));
        r
    }

    /// `EmitNoreturnRuntimeCallOrInvoke`: an `invoke` to the unreachable
    /// block, or a `call` then `unreachable`. The insertion point stays
    /// in the (terminated) block.
    pub(crate) fn noreturn_runtime_call_or_invoke(&mut self, f: FuncId, args: Vec<V>) {
        let fn_ty = self.m.func(f).ty();
        let fa = self.m.attr_set(&[Attr::Flag(AttrKind::NoReturn)]);
        let opts =
            CallOpts { attrs: AttrList { func: fa, ..AttrList::default() }, ..CallOpts::default() };
        let callee = V::C(Value::func(f));
        let r = if let Some(unwind) = self.invoke_dest() {
            let normal = self.unreachable_block();
            self.f.fir.push(
                Op::Invoke { fn_ty, callee, args, opts: Box::new(opts), normal, unwind },
                LTy::VOID,
            )
        } else {
            let r = self
                .f
                .fir
                .push(Op::Call { fn_ty, callee, args, opts: Box::new(opts) }, LTy::VOID);
            self.f.fir.push(Op::Unreachable, LTy::VOID);
            r
        };
        self.f.calls.push((Fir::last_inst(r), Some(f), false));
    }

    /// `FinishFunction`'s `EmitIfUsed` of the EH blocks (before the
    /// unreachable block).
    pub(crate) fn finish_eh(&mut self) {
        self.f.cl.eh_resume_used =
            self.f.cl.eh_resume.is_some_and(|b| self.f.fir.block_uses(b) > 0);
        for b in [
            self.f.cl.eh_resume.take(),
            self.f.cl.terminate_lpad.take(),
            self.f.cl.terminate_handler.take(),
        ]
        .into_iter()
        .flatten()
        {
            if self.f.fir.block_uses(b) > 0 {
                self.f.fir.place_at_end(b);
            } else {
                self.f.fir.erase_block(b);
            }
        }
    }
}
