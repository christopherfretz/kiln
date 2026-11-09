//! Coroutines (spec/codegen.md §7o, WP-351): a port of clang's
//! `CGCoroutine.cpp` for the switched-resume lowering. The function is
//! emitted pre-split (`presplitcoroutine`) with the `llvm.coro.*`
//! intrinsics; LLVM's `CoroEarly`/`CoroSplit`/`CoroCleanup` passes (run by the
//! driver, spec/driver.md) build the frame and the resume/destroy clones.

use kiln_ast::decl::builtin as id;
use kiln_ast::expr::{CoawaitParts, ExprKind, SuspendKind};
use kiln_ast::types::{ExcSpec, NoexceptValue};
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_llvm::intrinsics::{Coro, coro};
use kiln_llvm::{Attr, AttrKind, CastOp, Flags, IntPredicate, Linkage, TypeId as LTy};

use crate::builtins::BuiltinCall;
use crate::cleanup::{Cleanup, JumpDest};
use crate::expr::{LV, Opaque};
use crate::fir::{Blk, Fir, Op, V};
use crate::func::{Addr, Fx, RetKind};
use crate::module::{Cg, R, err};

/// Which suspend point an implicit or explicit await belongs to (clang
/// `AwaitKind`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AwaitKind {
    Init,
    Normal,
    Yield,
    Final,
}

/// The coroutine state of the function being emitted (clang
/// `CGCoroData`).
#[derive(Debug)]
pub(crate) struct CoroData {
    /// The `llvm.coro.id` token.
    pub id: V,
    /// The `llvm.coro.begin` frame handle.
    pub begin: Option<V>,
    /// `coro.ret`: where a suspended coroutine returns to its caller.
    pub suspend_bb: Blk,
    /// Leaving through the coroutine's cleanups (a destroyed suspension).
    pub cleanup_jd: Option<JumpDest>,
    /// `coro.final`: the final suspend point (`co_return`).
    pub final_jd: Option<JumpDest>,
    pub kind: AwaitKind,
    pub coreturns: u32,
    /// The last `llvm.coro.free` call (clang `LastCoroFree`), moved to
    /// guard the deallocation.
    pub last_free: Option<u32>,
    /// `__promise.unhandled_exception();`.
    pub on_exception: Option<StmtId>,
    /// `<init>.resume.eh`: an exception escaped the initial
    /// `await_resume` and the body is skipped.
    pub resume_eh: Option<V>,
    /// The ordinals of the last `co_await` and `co_yield` (clang
    /// `AwaitNum`/`YieldNum`), naming the suspend wrappers.
    pub await_num: u32,
    pub yield_num: u32,
    /// The state came from a hand-written `__builtin_coro_id` (clang
    /// `CoroIdExpr`), not from a coroutine body: no `presplitcoroutine`.
    pub hand_written: bool,
}

/// How the value of a `co_await`/`co_yield` is wanted.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Want {
    Ignored,
    Scalar,
    Lvalue,
    Agg(Addr, bool),
}

/// The value of a `co_await`/`co_yield` (clang `LValueOrRValue`).
pub(crate) enum Got {
    None,
    Scalar(Option<V>),
    Lv(LV),
}

impl Cg<'_> {
    fn null_ptr(&mut self) -> V {
        V::C(self.m.const_null(LTy::PTR))
    }

    fn coro_fn(&mut self, which: Coro) -> kiln_llvm::FuncId {
        coro(&mut self.m, which, LTy::I64)
    }

    /// `llvm.coro.end(args)`: `i1` on LLVM 19, `void` on LLVM 22
    /// (`CodegenOptions::llvm_major`).
    fn call_coro_end(&mut self, args: Vec<V>) -> V {
        let which = if self.opts.llvm_major >= 22 { Coro::EndVoid } else { Coro::End };
        let fe = self.coro_fn(which);
        self.intrinsic_call(fe, args)
    }

    /// Whether the code after `end` (a [`Self::call_coro_end`]) is in the
    /// ramp function: LLVM 19's `coro.end` result, LLVM 22's
    /// `llvm.coro.is_in_ramp()`.
    fn coro_in_ramp(&mut self, end: V) -> V {
        if self.opts.llvm_major >= 22 {
            let f = self.coro_fn(Coro::IsInRamp);
            self.intrinsic_call(f, vec![])
        } else {
            end
        }
    }

    fn coro_data(&mut self) -> &mut CoroData {
        self.f.coro.as_deref_mut().expect("coroutine state")
    }

    /// clang `EmitCoroutineBody` ([dcl.fct.def.coroutine]/5).
    pub(crate) fn emit_coroutine_body(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let c = *ast.stmts.coroutine_body(s);
        let null = self.null_ptr();
        let Some(entry) = self.f.fir.cur() else { return Ok(()) };
        // Coroutines always get lifetime markers (clang `GenerateCode`).
        self.f.lifetime_markers = true;
        self.f.bypasses = Some(Box::new(crate::lifetime::detect_bypasses(ast, c.body)));
        let alloc_bb = self.f.fir.create_block();
        let init_bb = self.f.fir.create_block();
        let final_bb = self.f.fir.create_block();
        let ret_bb = self.f.fir.create_block();

        let align = V::C(self.m.const_int(LTy::I32, i128::from(self.target.default_new_align)));
        let fid = self.coro_fn(Coro::Id);
        let id = self.intrinsic_call(fid, vec![align, null, null, null]);
        let id_inst = Fir::last_inst(id);
        self.f.coro = Some(Box::new(CoroData {
            id,
            begin: None,
            suspend_bb: ret_bb,
            cleanup_jd: None,
            final_jd: None,
            kind: AwaitKind::Normal,
            coreturns: 0,
            last_free: None,
            on_exception: None,
            resume_eh: None,
            await_num: 0,
            yield_num: 0,
            hand_written: false,
        }));

        // `mem = coro.alloc() ? allocate : null` (the allocation may be
        // elided, [dcl.fct.def.coroutine]/9).
        let fa = self.coro_fn(Coro::Alloc);
        let should_alloc = self.intrinsic_call(fa, vec![id]);
        self.cond_br(should_alloc, alloc_bb, init_bb);
        self.emit_block(alloc_bb, false);
        let mem = self.emit_scalar(c.allocate)?;
        let alloc_cont = self.f.fir.cur();
        if let Some(r) = c.return_on_alloc_failure {
            let fail = self.f.fir.create_block();
            let ok = self.icmp(IntPredicate::Ne, mem, null);
            self.cond_br(ok, init_bb, fail);
            self.emit_block(fail, false);
            self.emit_stmt(r)?;
        } else if alloc_cont.is_some() {
            self.br(init_bb);
        }
        self.emit_block(init_bb, false);
        let mut inc = vec![(null, entry)];
        if let Some(b) = alloc_cont {
            inc.push((mem, b));
        }
        let phi = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc }, LTy::PTR);
        let fb = self.coro_fn(Coro::Begin);
        let begin = self.intrinsic_call(fb, vec![id, phi]);
        self.coro_data().begin = Some(begin);

        // `GetReturnObjectManager::EmitGroAlloca`.
        let gro = match c.result_decl {
            Some(d) => Some(self.emit_gro_alloca(d)?),
            None => None,
        };

        let cleanup_jd = self.jump_dest_to(ret_bb);
        self.coro_data().cleanup_jd = Some(cleanup_jd);
        let sc = self.begin_scope(false);
        self.push_cleanup(Cleanup::CoroDelete { dealloc: c.deallocate });
        // The parameter copies ([dcl.fct.def.coroutine]/13): the body
        // refers to the copies (clang `ParamReferenceReplacerRAII`).
        let mut replaced: Vec<(DeclId, Option<Addr>)> = Vec::new();
        for m in ast.lists.iter::<StmtId>(c.param_moves).collect::<Vec<_>>() {
            self.emit_stmt(m)?;
            for v in ast.lists.iter::<DeclId>(ast.stmts.decl_stmt_decls(m)).collect::<Vec<_>>() {
                let (Some(p), Some(&copy)) = (self.copied_param(v), self.f.locals.get(&v)) else {
                    continue;
                };
                replaced.push((p, self.f.locals.insert(p, copy)));
            }
        }
        self.emit_stmt(c.promise_stmt)?;
        if let Some(p) = self.f.locals.get(&c.promise).copied() {
            // clang 19 inserts `bitcast ptr %promise to ptr` before coro.id;
            // clang 22 passes the promise alloca itself.
            let pv = if self.opts.clang_compat < 22 {
                let cast =
                    Op::Cast { op: CastOp::BitCast, flags: Flags::NONE, v: p.v, to: LTy::PTR };
                self.f.fir.insert_before(id_inst, cast, LTy::PTR)
            } else {
                p.v
            };
            self.f.fir.set_call_arg(id_inst, 1, pv);
        }
        // `GetReturnObjectManager::EmitGroInit`.
        match (gro, c.return_stmt) {
            (Some((d, addr, flag)), _) => {
                let q = ast.decls.get(d).ty().expect("gro type");
                if let Some(init) = ast.decls.var(d).and_then(|v| v.init) {
                    self.emit_var_init(d, q, addr, init)?;
                }
                let t = V::C(self.m.const_bool(true));
                self.store(t, flag, 1, false);
            }
            (None, Some(_)) => {
                let sc = self.begin_scope(false);
                self.store_return_value(c.return_value)?;
                self.end_scope(sc);
            }
            (None, None) => self.emit_ignored(c.return_value)?,
        }
        self.push_cleanup_kind(Cleanup::CoroEnd, false, true);

        self.coro_data().kind = AwaitKind::Init;
        self.coro_data().on_exception = c.on_exception;
        self.emit_ignored(c.init_suspend)?;
        let final_jd = self.jump_dest_to(final_bb);
        self.coro_data().final_jd = Some(final_jd);
        self.coro_data().kind = AwaitKind::Normal;

        if let Some(h) = c.on_exception {
            let mut cont = None;
            if let Some(flag) = self.coro_data().resume_eh
                && self.f.fir.cur().is_some()
            {
                let body = self.f.fir.create_block();
                let k = self.f.fir.create_block();
                let skip = self.load(LTy::I1, flag, 1, false);
                self.cond_br(skip, k, body);
                self.emit_block(body, false);
                cont = Some(k);
            }
            self.enter_catch_all();
            self.emit_body_and_fallthrough(c.body, c.on_fallthrough)?;
            self.exit_catch_all(h)?;
            if let Some(k) = cont {
                self.emit_block(k, false);
            }
        } else {
            self.emit_body_and_fallthrough(c.body, c.on_fallthrough)?;
        }

        let fallthrough = self.f.fir.cur().is_some();
        if fallthrough || self.coro_data().coreturns > 0 {
            self.emit_block(final_bb, false);
            self.coro_data().kind = AwaitKind::Final;
            self.emit_ignored(c.final_suspend)?;
        } else {
            self.emit_block(final_bb, true);
        }
        self.end_scope(sc);
        for (p, old) in replaced.into_iter().rev() {
            match old {
                Some(a) => self.f.locals.insert(p, a),
                None => self.f.locals.remove(&p),
            };
        }

        self.emit_block(ret_bb, false);
        // `coro.end` precedes the return: the resume and destroy clones
        // end here.
        let f = V::C(self.m.const_bool(false));
        let none = V::C(self.m.const_none_token());
        self.call_coro_end(vec![null, f, none]);
        match (gro, c.return_stmt) {
            (Some(_), Some(r)) => self.emit_stmt(r)?,
            (None, Some(_)) => {
                let rd = self.f.ret_dest.expect("return destination");
                self.branch_through_cleanup(rd);
            }
            _ => {}
        }
        Ok(())
    }

    /// clang `emitBodyAndFallthrough`: flowing off the end runs the
    /// implicit `co_return;` ([stmt.return.coroutine]/3).
    fn emit_body_and_fallthrough(&mut self, body: StmtId, ft: Option<StmtId>) -> R<()> {
        self.emit_stmt(body)?;
        if self.f.fir.cur().is_some()
            && let Some(s) = ft
        {
            self.emit_stmt(s)?;
        }
        Ok(())
    }

    /// The parameter whose copy is the variable `v` (its initializer
    /// names the parameter).
    fn copied_param(&self, v: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        let mut stack = vec![ast.decls.var(v)?.init?];
        while let Some(e) = stack.pop() {
            if ast.exprs.get(e).kind() == ExprKind::DeclRefExpr {
                let d = ast.exprs.decl(e);
                if ast.decls.param(d).is_some() {
                    return Some(d);
                }
            }
            ast.exprs.children_into(e, &ast.lists, &mut stack);
        }
        None
    }

    /// `GetReturnObjectManager::EmitGroAlloca`: the `__coro_gro`
    /// variable (outside the frame), its destructor guarded by the
    /// `gro.active` flag.
    fn emit_gro_alloca(&mut self, d: DeclId) -> R<(DeclId, Addr, V)> {
        let ast = self.ast;
        let flag = self.f.fir.alloca(LTy::I1, 1);
        let f = V::C(self.m.const_bool(false));
        self.store(f, flag, 1, false);
        let Some(q) = ast.decls.get(d).ty() else {
            return err(Some(ast.decls.get(d).loc()), "coroutine result without a type");
        };
        let ty = self.mty(q);
        let align = self.var_align(d);
        let a = self.f.fir.alloca(ty, align);
        let kind = self.m.md_kind("coro.outside.frame");
        let md = self.m.md_tuple(&[]);
        self.f.fir.add_md(Fir::last_inst(a), kind, md);
        let addr = Addr { v: a, ty, align };
        self.f.locals.insert(d, addr);
        self.var_lifetime(d, a, ty);
        if let Some(rd) = self.var_destructed_class(d, q) {
            self.push_cleanup(Cleanup::FlaggedDtor { addr: a, record: rd, flag });
        }
        Ok((d, addr, flag))
    }

    /// clang `EmitCoreturnStmt` ([stmt.return.coroutine]).
    pub(crate) fn emit_coreturn(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        if self.f.coro.is_none() {
            return err(Some(ast.stmts.get(s).loc()), "co_return outside a coroutine body");
        }
        self.coro_data().coreturns += 1;
        if let Some(op) = ast.stmts.coreturn_operand(s)
            && ast.types.is_void(ast.exprs.get(op).ty())
            && ast.exprs.get(op).kind() != ExprKind::InitListExpr
        {
            let sc = self.begin_scope(false);
            self.emit_ignored(op)?;
            self.end_scope(sc);
        }
        if let Some(call) = ast.stmts.coreturn_promise_call(s) {
            self.emit_ignored(call)?;
        }
        if self.f.fir.cur().is_some() {
            let jd = self.coro_data().final_jd.expect("final suspend destination");
            self.branch_through_cleanup(jd);
        }
        Ok(())
    }

    /// clang `emitSuspendExpression`: a `co_await` or `co_yield`
    /// ([expr.await]/5).
    pub(crate) fn emit_coawait(&mut self, e: ExprId, want: Want) -> R<Got> {
        let ast = self.ast;
        if self.f.coro.is_none() {
            return err(self.loc_of(e), "co_await outside a coroutine body");
        }
        let Some(p) = ast.exprs.coawait_parts(e, &ast.lists) else {
            return err(self.loc_of(e), "incomplete co_await expression");
        };
        let kind = if ast.exprs.get(e).kind() == ExprKind::CoyieldExpr {
            AwaitKind::Yield
        } else {
            self.coro_data().kind
        };
        let lv = self.emit_lvalue(p.common)?;
        let old = self.f.opaque.insert(p.common, Opaque::Lv(lv));
        let r = self.emit_suspend(e, &p, kind, want, lv);
        match old {
            Some(o) => self.f.opaque.insert(p.common, o),
            None => self.f.opaque.remove(&p.common),
        };
        r
    }

    fn emit_suspend(
        &mut self,
        e: ExprId,
        p: &CoawaitParts,
        kind: AwaitKind,
        want: Want,
        awaiter: LV,
    ) -> R<Got> {
        let prefix = self.suspend_prefix(kind);
        let ready = self.f.fir.create_block();
        let suspend = self.f.fir.create_block();
        let cleanup = self.f.fir.create_block();
        self.branch_on_bool(p.ready, ready, suspend)?;
        self.emit_block(suspend, false);
        let null = self.null_ptr();
        let fs = self.coro_fn(Coro::Save);
        let save = self.intrinsic_call(fs, vec![null]);
        // clang 19: `await_suspend` is outlined into a wrapper that
        // `llvm.coro.await.suspend.*` calls once the coroutine is
        // suspended, so no frame value is live across it; for a handle,
        // CoroSplit resumes the result with a `musttail` call (symmetric
        // transfer, [expr.await]/5.1).
        let sk = self.ast.exprs.coawait_suspend_kind(e);
        let wrapper = self.await_suspend_wrapper(p, sk, &prefix, awaiter)?;
        let begin = self.coro_data().begin.expect("coro.begin");
        let which = match sk {
            SuspendKind::Void => Coro::AwaitSuspendVoid,
            SuspendKind::Bool => Coro::AwaitSuspendBool,
            SuspendKind::Handle => Coro::AwaitSuspendHandle,
        };
        let fi = self.coro_fn(which);
        let rty = if sk == SuspendKind::Bool { LTy::I1 } else { LTy::VOID };
        let args = vec![awaiter.addr.v, begin, V::C(kiln_llvm::Value::func(wrapper))];
        let r = if sk == SuspendKind::Handle || self.stmt_can_throw(p.suspend) {
            self.runtime_call_or_invoke(fi, rty, args, &[])
        } else {
            self.nounwind_runtime_call(fi, rty, args)
        };
        if sk == SuspendKind::Bool {
            // `false` resumes at once.
            let real = self.f.fir.create_block();
            self.cond_br(r, real, ready);
            self.emit_block(real, false);
        }
        if self.f.fir.cur().is_some() {
            let fsus = self.coro_fn(Coro::Suspend);
            let fin = V::C(self.m.const_bool(kind == AwaitKind::Final));
            let r = self.intrinsic_call(fsus, vec![save, fin]);
            let ret_bb = self.coro_data().suspend_bb;
            let zero = V::C(self.m.const_int(LTy::I8, 0));
            let one = V::C(self.m.const_int(LTy::I8, 1));
            self.f.fir.push(
                Op::Switch { c: r, default: ret_bb, cases: vec![(zero, ready), (one, cleanup)] },
                LTy::VOID,
            );
        }
        // clang `EmitBlock(CleanupBlock)`: placed after the suspend block.
        self.emit_block(cleanup, true);
        if self.f.fir.cur().is_some() {
            let jd = self.coro_data().cleanup_jd.expect("coroutine cleanup destination");
            self.branch_through_cleanup(jd);
        }
        self.emit_block(ready, false);

        // An exception from the initial `await_resume` is handled as if
        // thrown by the body ([dcl.fct.def.coroutine]/5, CWG2451).
        if let Some(h) = self.coro_data().on_exception
            && kind == AwaitKind::Init
            && self.stmt_can_throw(p.resume)
        {
            let flag = self.f.fir.alloca(LTy::I1, 1);
            self.coro_data().resume_eh = Some(flag);
            let t = V::C(self.m.const_bool(true));
            self.store(t, flag, 1, false);
            self.enter_catch_all();
            // clang emits the call as the statement of the `try` body
            // (`EmitStmt(TryBody)`): its full-expression cleanups (the
            // destructor of a class-type result) end inside the handler's
            // scope, before the flag store ([class.temporary]/4).
            let sc = self.begin_scope(false);
            let r = self.emit_ignored(p.resume);
            self.end_scope(sc);
            r?;
            if self.f.fir.cur().is_some() {
                let f = V::C(self.m.const_bool(false));
                self.store(f, flag, 1, false);
            }
            self.exit_catch_all(h)?;
            return Ok(Got::None);
        }
        Ok(match want {
            Want::Ignored => {
                self.emit_ignored(p.resume)?;
                Got::None
            }
            Want::Scalar => Got::Scalar(self.emit_scalar_opt(p.resume)?),
            Want::Lvalue => Got::Lv(self.emit_lvalue(p.resume)?),
            Want::Agg(dest, zeroed) => {
                self.emit_agg(p.resume, dest, zeroed)?;
                Got::None
            }
        })
    }

    /// clang `StmtCanThrow`: `e` contains a call whose callee may throw
    /// (an indirect call, or one not `noexcept`), or binds a temporary
    /// whose destructor may.
    fn stmt_can_throw(&self, e: ExprId) -> bool {
        let ast = self.ast;
        // clang `FunctionCanThrow`: only a `noexcept` specification is
        // non-throwing here (`throw()` is not).
        let fn_can_throw = |d: DeclId| {
            !ast.decls.get(d).ty().and_then(|t| ast.types.proto(t)).is_some_and(|p| {
                matches!(
                    ast.types.exc_spec(p),
                    ExcSpec::Nothrow
                        | ExcSpec::BasicNoexcept
                        | ExcSpec::NoexceptExpr { value: NoexceptValue::True, .. }
                )
            })
        };
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            match ast.exprs.get(x).kind() {
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr
                | ExprKind::UserDefinedLiteral => {
                    let mut c = ast.exprs.callee(x);
                    loop {
                        match ast.exprs.get(c).kind() {
                            ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => {
                                c = ast.exprs.operand(c);
                            }
                            ExprKind::MemberExpr | ExprKind::DeclRefExpr => break,
                            _ => return true,
                        }
                    }
                    if fn_can_throw(ast.exprs.decl(c)) {
                        return true;
                    }
                }
                ExprKind::CXXBindTemporaryExpr => {
                    let q = ast.exprs.get(x).ty();
                    if let Some(rd) = ast.types.as_record(q)
                        && ast.destructor(rd).is_some_and(fn_can_throw)
                    {
                        return true;
                    }
                }
                _ => {}
            }
            ast.exprs.children_into(x, &ast.lists, &mut stack);
        }
        false
    }

    /// clang `buildSuspendPrefixStr`: `init`, `final`, `await[N]` or
    /// `yield[N]` (numbered from the second).
    fn suspend_prefix(&mut self, kind: AwaitKind) -> String {
        let d = self.coro_data();
        let (base, n) = match kind {
            AwaitKind::Init => ("init", 0),
            AwaitKind::Final => ("final", 0),
            AwaitKind::Normal => {
                d.await_num += 1;
                ("await", d.await_num)
            }
            AwaitKind::Yield => {
                d.yield_num += 1;
                ("yield", d.yield_num)
            }
        };
        if n > 1 { format!("{base}{n}") } else { base.to_owned() }
    }

    /// clang `generateAwaitSuspendWrapper`: the private
    /// `<coroutine>.__await_suspend_wrapper__<prefix>(ptr awaiter, ptr
    /// frame)` evaluating the `await_suspend` call (returning its `bool`,
    /// or the address of its handle), with the awaiter and
    /// `__builtin_coro_frame()` bound to the parameters.
    fn await_suspend_wrapper(
        &mut self,
        p: &CoawaitParts,
        sk: SuspendKind,
        prefix: &str,
        awaiter: LV,
    ) -> R<kiln_llvm::FuncId> {
        let cur = self.f.func.expect("current function");
        let name = format!("{}.__await_suspend_wrapper__{prefix}", self.m.func(cur).name());
        let rty = match sk {
            SuspendKind::Void => LTy::VOID,
            SuspendKind::Bool => LTy::I1,
            SuspendKind::Handle => LTy::PTR,
        };
        let fty = self.m.types.function(rty, &[LTy::PTR, LTy::PTR], false);
        let f = match self.m.add_function(&name, fty) {
            Ok(f) => f,
            Err(e) => return err(self.loc_of(p.suspend), format!("bad suspend wrapper: {e:?}")),
        };
        let noundef = Attr::Flag(AttrKind::NoUndef);
        let a0 = self.m.attr_set(&[Attr::Flag(AttrKind::NonNull), noundef.clone()]);
        let a1 = self.m.attr_set(std::slice::from_ref(&noundef));
        let ret = if rty == LTy::VOID { None } else { Some(self.m.attr_set(&[noundef])) };
        // clang 19 `PrivateLinkage`, clang 22 `InternalLinkage`.
        let linkage =
            if self.opts.clang_compat < 22 { Linkage::Private } else { Linkage::Internal };
        let func = self.m.func_mut(f);
        func.linkage = linkage;
        if let Some(r) = ret {
            func.attrs.ret = r;
        }
        func.attrs.params = vec![a0, a1];
        let decl = self.f.decl;
        let saved = std::mem::replace(&mut self.f, Fx::empty());
        let r = self.emit_await_suspend_wrapper(f, p, sk, rty, awaiter, decl);
        self.f = saved;
        r?;
        Ok(f)
    }

    /// `StartFunction`, the `await_suspend` call and `FinishFunction` of
    /// the suspend wrapper `f` of coroutine `decl`.
    fn emit_await_suspend_wrapper(
        &mut self,
        f: kiln_llvm::FuncId,
        p: &CoawaitParts,
        sk: SuspendKind,
        rty: LTy,
        awaiter: LV,
        decl: Option<DeclId>,
    ) -> R<()> {
        self.f.fir = Fir::new(vec![LTy::PTR, LTy::PTR]);
        self.start_fp();
        self.f.func = Some(f);
        self.f.decl = decl;
        self.di_wrapper_start(f);
        let ret_block = self.f.fir.create_block();
        self.f.ret_block = Some(ret_block);
        self.f.ret_dest = Some(JumpDest { blk: ret_block, depth: Some(0), index: 1 });
        self.f.cl.next_index = 2;
        if rty != LTy::VOID {
            let align = if rty == LTy::PTR { 8 } else { 1 };
            let slot = self.f.fir.alloca(rty, align);
            self.f.ret_slot = Some(Addr { v: slot, ty: rty, align });
            self.f.ret_kind = RetKind::Scalar;
        }
        self.f.cl.prologue = self.f.cl.depth();
        let pa = 8;
        // The parameters are `void *` `ImplicitParamDecl`s: their prolog
        // stores are tagged, the reloads below are raw `CreateLoad`s.
        let a0 = self.f.fir.alloca(LTy::PTR, pa);
        let st = self.store(V::Arg(0), a0, pa, false);
        self.tbaa_void_ptr_store(st);
        let a1 = self.f.fir.alloca(LTy::PTR, pa);
        let st = self.store(V::Arg(1), a1, pa, false);
        self.tbaa_void_ptr_store(st);
        let aw = self.load(LTy::PTR, a0, pa, false);
        self.f.await_frame = Some(self.load(LTy::PTR, a1, pa, false));
        let lv = LV { addr: Addr { v: aw, ..awaiter.addr }, ..awaiter };
        self.f.opaque.insert(p.common, Opaque::Lv(lv));
        if sk == SuspendKind::Void {
            self.emit_ignored(p.suspend)?;
        } else {
            let v = self.emit_scalar(p.suspend)?;
            if self.f.fir.cur().is_some()
                && let Some(slot) = self.f.ret_slot
            {
                self.store(v, slot.v, slot.align, false);
            }
        }
        // FinishFunction: the epilog keeps the suspend call's location.
        self.di_finish_function(kiln_source::Loc::INVALID);
        let prologue = self.f.cl.prologue;
        self.pop_cleanup_blocks(prologue);
        self.emit_return_block();
        self.emit_epilog()?;
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
        // FinishFunction adds x86's `min-legal-vector-width` here too.
        let mut fa = vec![Attr::Flag(AttrKind::AlwaysInline), Attr::Flag(AttrKind::MustProgress)];
        if self.is_x86() {
            fa.push(Attr::string("min-legal-vector-width", &self.f.vector_width.to_string()));
        }
        // clang 22 (llvm-project 86b95b0): keep the `.__await_suspend_
        // wrapper__` suffix for sample-profile name canonicalization.
        if self.opts.clang_compat >= 22 {
            fa.push(Attr::string("sample-profile-suffix-elision-policy", "selected"));
        }
        let fa = self.m.attr_set(&fa);
        self.m.func_mut(f).attrs.func = fa;
        let fir = std::mem::replace(&mut self.f.fir, Fir::new(Vec::new()));
        let loc = decl.map(|d| self.ast.decls.get(d).loc());
        if self.f.uses_ph {
            let Some(d) = decl else { return err(loc, "suspend wrapper outside a function") };
            self.pending.push((f, d, fir));
            return Ok(());
        }
        if let Err(e) = fir.lower(&mut self.m, f, &self.phs) {
            return err(loc, format!("IR lowering failed: {e:?}"));
        }
        Ok(())
    }

    /// `CallCoroDelete`: `if (coro.free(id, frame)) deallocate`. As
    /// clang, the deallocation is emitted first and the `coro.free` call
    /// it made is then moved up to guard it.
    pub(crate) fn emit_coro_delete(&mut self, dealloc: ExprId) -> R<()> {
        if self.f.coro.is_none() {
            return Ok(());
        }
        let Some(saved_bb) = self.f.fir.cur() else { return Ok(()) };
        let free_bb = self.f.fir.create_block();
        self.emit_block(free_bb, false);
        let prev = self.coro_data().last_free.take();
        let r = self.emit_ignored(dealloc);
        let last = std::mem::replace(&mut self.coro_data().last_free, prev);
        r?;
        let after = self.f.fir.create_block();
        self.emit_block(after, false);
        let (Some(cf), Some(t)) = (last, self.f.fir.terminator(saved_bb)) else {
            return err(
                self.loc_of(dealloc),
                "Deallocation expressoin does not refer to coro.free",
            );
        };
        self.f.fir.move_before(cf, t);
        self.f.fir.erase(t);
        self.f.fir.set_cur(Some(saved_bb));
        let null = self.null_ptr();
        let c = self.icmp(IntPredicate::Ne, V::I(cf), null);
        self.cond_br(c, free_bb, after);
        self.f.fir.set_cur(Some(after));
        Ok(())
    }

    /// `CallCoroEnd` (landing-pad model). LLVM 19: the ramp function
    /// resumes unwinding, the resume clone continues with the cleanups.
    /// LLVM 22 (clang 22): the other way round, the ramp function runs the
    /// remaining cleanups (freeing the frame) and the clones resume.
    pub(crate) fn emit_coro_end_cleanup(&mut self) {
        let null = self.null_ptr();
        let t = V::C(self.m.const_bool(true));
        let none = V::C(self.m.const_none_token());
        let end = self.call_coro_end(vec![null, t, none]);
        let in_ramp = self.coro_in_ramp(end);
        let resume = self.eh_resume_block();
        let cont = self.f.fir.create_block();
        if self.opts.llvm_major >= 22 {
            self.cond_br(in_ramp, cont, resume);
        } else {
            self.cond_br(in_ramp, resume, cont);
        }
        self.emit_block(cont, false);
    }

    /// clang `EmitCoroutineIntrinsic`: a `__builtin_coro_*` call.
    pub(crate) fn emit_coro_builtin(&mut self, b: u32, e: ExprId) -> R<BuiltinCall> {
        let ast = self.ast;
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        let q = ast.exprs.get(e).ty();
        let coro_id = self.f.coro.as_deref().map(|c| c.id);
        let which = match b {
            id::CORO_FRAME => {
                return match self.f.coro.as_deref().and_then(|c| c.begin).or(self.f.await_frame) {
                    Some(v) => Ok(BuiltinCall::Emitted(Some(v))),
                    None => err(
                        self.loc_of(e),
                        "this builtin expect that __builtin_coro_begin has been used earlier in this function",
                    ),
                };
            }
            id::CORO_FREE => Coro::Free,
            id::CORO_SIZE | id::CORO_ALIGN => {
                let which = if b == id::CORO_SIZE { Coro::Size } else { Coro::Align };
                let t = self.mty(q);
                let f = coro(&mut self.m, which, t);
                return Ok(BuiltinCall::Emitted(Some(self.intrinsic_call(f, Vec::new()))));
            }
            id::CORO_RESUME => Coro::Resume,
            id::CORO_DESTROY => Coro::Destroy,
            id::CORO_DONE => Coro::Done,
            id::CORO_PROMISE => Coro::Promise,
            id::CORO_NOOP => Coro::Noop,
            id::CORO_ID => Coro::Id,
            id::CORO_ALLOC => Coro::Alloc,
            id::CORO_BEGIN => Coro::Begin,
            id::CORO_END => Coro::End,
            id::CORO_SUSPEND => Coro::Suspend,
            _ => return err(self.loc_of(e), "unknown coroutine builtin"),
        };
        let mut vals = Vec::new();
        match which {
            Coro::Alloc | Coro::Begin | Coro::Free => match coro_id {
                Some(i) => vals.push(i),
                None => {
                    return err(
                        self.loc_of(e),
                        "this builtin expect that __builtin_coro_id has been used earlier in this function",
                    );
                }
            },
            Coro::Suspend => vals.push(V::C(self.m.const_none_token())),
            _ => {}
        }
        for &a in &args {
            vals.push(self.emit_scalar(a)?);
        }
        if which == Coro::End {
            // `__builtin_coro_end` returns whether it is in the ramp
            // function; on LLVM 22 that is `llvm.coro.is_in_ramp()` (clang
            // 22 stores the `void` call's non-value instead).
            vals.push(V::C(self.m.const_none_token()));
            let end = self.call_coro_end(vals);
            return Ok(BuiltinCall::Emitted(Some(self.coro_in_ramp(end))));
        }
        let f = self.coro_fn(which);
        let r = self.intrinsic_call(f, vals);
        // clang lets C code emit `coro.id`/`coro.begin` by hand: the token
        // is remembered for later `coro.alloc`/`begin`/`free` calls
        // (`createCoroData`), the handle for `coro.frame`, and the last
        // `coro.free` for `CallCoroDelete`.
        match (which, self.f.coro.as_deref_mut()) {
            (Coro::Id, Some(c)) => {
                let msg = if c.hand_written {
                    "only one __builtin_coro_id can be used in a function"
                } else {
                    "__builtin_coro_id shall not be used in a C++ coroutine"
                };
                return err(self.loc_of(e), msg);
            }
            (Coro::Id, None) => {
                let suspend_bb = self.f.fir.create_block();
                self.f.coro = Some(Box::new(CoroData {
                    id: r,
                    begin: None,
                    suspend_bb,
                    cleanup_jd: None,
                    final_jd: None,
                    kind: AwaitKind::Normal,
                    coreturns: 0,
                    last_free: None,
                    on_exception: None,
                    resume_eh: None,
                    await_num: 0,
                    yield_num: 0,
                    hand_written: true,
                }));
            }
            (Coro::Begin, Some(c)) => c.begin = Some(r),
            (Coro::Free, Some(c)) => c.last_free = Some(Fir::last_inst(r)),
            _ => {}
        }
        let void = matches!(which, Coro::Resume | Coro::Destroy);
        Ok(BuiltinCall::Emitted((!void).then_some(r)))
    }

    /// The `presplitcoroutine` attribute of coroutine definition `d`,
    /// and `coro_only_destroy_when_complete` when its return type is a
    /// class marked `[[clang::coro_only_destroy_when_complete]]` (clang
    /// `CodeGenFunction::EmitCoroutineBody`).
    pub(crate) fn coro_fn_attrs(&self, d: DeclId, extra: &mut Vec<Attr>) {
        if self.f.coro.as_deref().is_none_or(|c| c.hand_written) {
            return;
        }
        extra.push(Attr::Flag(AttrKind::PresplitCoroutine));
        let ast = self.ast;
        let ret = ast
            .decls
            .get(d)
            .ty()
            .and_then(|t| ast.types.proto(t))
            .map(kiln_ast::types::FuncProto::ret);
        if ret.and_then(|r| ast.types.as_record(r)).is_some_and(|r| {
            self.fn_attr(r, kiln_ast::attr::AttrKind::CORO_ONLY_DESTROY_WHEN_COMPLETE).is_some()
        }) {
            extra.push(Attr::Flag(AttrKind::CoroDestroyOnlyWhenComplete));
        }
    }
}
