//! Coroutines (spec/codegen.md §7o, WP-351): a port of clang's
//! `CGCoroutine.cpp` for the switched-resume lowering. The function is
//! emitted pre-split (`presplitcoroutine`) with the `llvm.coro.*`
//! intrinsics; LLVM's CoroEarly/CoroSplit/CoroCleanup passes (run by the
//! driver, spec/driver.md) build the frame and the resume/destroy clones.

use kiln_ast::decl::builtin as id;
use kiln_ast::expr::{CoawaitParts, ExprKind, SuspendKind};
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_llvm::intrinsics::{Coro, coro};
use kiln_llvm::{AttrKind, IntPredicate, TypeId as LTy};

use crate::builtins::BuiltinCall;
use crate::cleanup::{Cleanup, JumpDest};
use crate::expr::{LV, Opaque};
use crate::fir::{Blk, Fir, Op, V};
use crate::func::Addr;
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
    /// The `llvm.coro.free` result the deallocation's
    /// `__builtin_coro_free` stands for.
    pub cur_free: Option<V>,
    /// `__promise.unhandled_exception();`.
    pub on_exception: Option<StmtId>,
    /// `<init>.resume.eh`: an exception escaped the initial
    /// `await_resume` and the body is skipped.
    pub resume_eh: Option<V>,
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

    fn coro_data(&mut self) -> &mut CoroData {
        self.f.coro.as_deref_mut().expect("coroutine state")
    }

    /// clang `EmitCoroutineBody` ([dcl.fct.def.coroutine]/5).
    pub(crate) fn emit_coroutine_body(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let c = *ast.stmts.coroutine_body(s);
        let null = self.null_ptr();
        let Some(entry) = self.f.fir.cur() else { return Ok(()) };
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
            cur_free: None,
            on_exception: None,
            resume_eh: None,
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
            self.f.fir.set_call_arg(id_inst, 1, p.v);
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
        let fe = self.coro_fn(Coro::End);
        let f = V::C(self.m.const_bool(false));
        let none = V::C(self.m.const_none_token());
        self.intrinsic_call(fe, vec![null, f, none]);
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
            stack.extend(ast.exprs.children(e, &ast.lists));
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
        let r = self.emit_suspend(e, &p, kind, want);
        match old {
            Some(o) => self.f.opaque.insert(p.common, o),
            None => self.f.opaque.remove(&p.common),
        };
        r
    }

    fn emit_suspend(&mut self, e: ExprId, p: &CoawaitParts, kind: AwaitKind, want: Want) -> R<Got> {
        let ast = self.ast;
        let ready = self.f.fir.create_block();
        let suspend = self.f.fir.create_block();
        let cleanup = self.f.fir.create_block();
        self.branch_on_bool(p.ready, ready, suspend)?;
        self.emit_block(suspend, false);
        let null = self.null_ptr();
        let fs = self.coro_fn(Coro::Save);
        let save = self.intrinsic_call(fs, vec![null]);
        match ast.exprs.coawait_suspend_kind(e) {
            SuspendKind::Void => self.emit_ignored(p.suspend)?,
            SuspendKind::Bool => {
                let b = self.emit_scalar(p.suspend)?;
                let real = self.f.fir.create_block();
                if self.f.fir.cur().is_some() {
                    self.cond_br(b, real, ready);
                }
                self.emit_block(real, false);
            }
            // Symmetric transfer: the returned handle is resumed (a tail
            // call after CoroSplit).
            SuspendKind::Handle => {
                let h = self.emit_scalar(p.suspend)?;
                if self.f.fir.cur().is_some() {
                    let fr = self.coro_fn(Coro::Resume);
                    self.intrinsic_call(fr, vec![h]);
                }
            }
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
            self.f.fir.set_cur(None);
        }
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
            && self.resume_can_throw(p.resume)
        {
            let flag = self.f.fir.alloca(LTy::I1, 1);
            self.coro_data().resume_eh = Some(flag);
            let t = V::C(self.m.const_bool(true));
            self.store(t, flag, 1, false);
            self.enter_catch_all();
            self.emit_ignored(p.resume)?;
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

    /// clang `StmtCanThrow` for an `await_resume()` call: false when the
    /// called function is non-throwing.
    fn resume_can_throw(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let mut e = e;
        loop {
            let k = ast.exprs.get(e).kind();
            match k {
                ExprKind::ParenExpr
                | ExprKind::ImplicitCastExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ExprWithCleanups => e = ast.exprs.operand(e),
                ExprKind::CXXMemberCallExpr | ExprKind::CallExpr => {
                    let mut c = ast.exprs.callee(e);
                    loop {
                        match ast.exprs.get(c).kind() {
                            ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => {
                                c = ast.exprs.operand(c);
                            }
                            ExprKind::MemberExpr | ExprKind::DeclRefExpr => break,
                            _ => return true,
                        }
                    }
                    let d = ast.exprs.decl(c);
                    let nothrow = ast
                        .decls
                        .get(d)
                        .ty()
                        .and_then(|t| ast.types.proto(t))
                        .is_some_and(|p| ast.types.exc_spec(p).is_nothrow());
                    if !nothrow {
                        return true;
                    }
                    return ast
                        .exprs
                        .children(e, &ast.lists)
                        .into_iter()
                        .any(|a| self.subexpr_can_throw(a));
                }
                _ => return true,
            }
        }
    }

    fn subexpr_can_throw(&self, e: ExprId) -> bool {
        let ast = self.ast;
        match ast.exprs.get(e).kind() {
            ExprKind::MemberExpr | ExprKind::DeclRefExpr | ExprKind::OpaqueValueExpr => false,
            ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => {
                self.subexpr_can_throw(ast.exprs.operand(e))
            }
            _ => true,
        }
    }

    /// `CallCoroDelete`: `if (coro.free(id, frame)) deallocate`.
    pub(crate) fn emit_coro_delete(&mut self, dealloc: ExprId) -> R<()> {
        let Some(data) = self.f.coro.as_deref() else { return Ok(()) };
        let (id, begin) = (data.id, data.begin.expect("coro.begin"));
        let ff = self.coro_fn(Coro::Free);
        let mem = self.intrinsic_call(ff, vec![id, begin]);
        let free_bb = self.f.fir.create_block();
        let after = self.f.fir.create_block();
        let null = self.null_ptr();
        let c = self.icmp(IntPredicate::Ne, mem, null);
        self.cond_br(c, free_bb, after);
        self.emit_block(free_bb, false);
        let saved = self.coro_data().cur_free.replace(mem);
        let r = self.emit_ignored(dealloc);
        self.coro_data().cur_free = saved;
        r?;
        self.emit_block(after, false);
        Ok(())
    }

    /// `CallCoroEnd` (landing-pad model): the ramp function resumes
    /// unwinding; the resume clone continues with the cleanups.
    pub(crate) fn emit_coro_end_cleanup(&mut self) {
        let fe = self.coro_fn(Coro::End);
        let null = self.null_ptr();
        let t = V::C(self.m.const_bool(true));
        let none = V::C(self.m.const_none_token());
        let r = self.intrinsic_call(fe, vec![null, t, none]);
        let resume = self.eh_resume_block();
        let cont = self.f.fir.create_block();
        self.cond_br(r, resume, cont);
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
                return match self.f.coro.as_deref().and_then(|c| c.begin) {
                    Some(v) => Ok(BuiltinCall::Emitted(Some(v))),
                    None => err(
                        self.loc_of(e),
                        "this builtin expect that __builtin_coro_begin has been used earlier in this function",
                    ),
                };
            }
            id::CORO_FREE => {
                if let Some(v) = self.f.coro.as_deref().and_then(|c| c.cur_free) {
                    for &a in &args {
                        self.emit_ignored(a)?;
                    }
                    return Ok(BuiltinCall::Emitted(Some(v)));
                }
                Coro::Free
            }
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
            vals.push(V::C(self.m.const_none_token()));
        }
        let f = self.coro_fn(which);
        let r = self.intrinsic_call(f, vals);
        let void = matches!(which, Coro::Resume | Coro::Destroy);
        Ok(BuiltinCall::Emitted((!void).then_some(r)))
    }

    /// The `presplitcoroutine` attribute of a coroutine definition.
    pub(crate) fn coro_fn_attrs(&self, extra: &mut Vec<kiln_llvm::Attr>) {
        if self.f.coro.is_some() {
            extra.push(kiln_llvm::Attr::Flag(AttrKind::PresplitCoroutine));
        }
    }
}
