//! try-blocks and handlers (spec/codegen.md §7b, WP-100): ports of
//! clang's `EnterCXXTryStmt` / `ExitCXXTryStmt` /
//! `emitCatchDispatchBlock` (`CGException.cpp`) and of the Itanium
//! `emitBeginCatch` / `InitCatchParam` (`ItaniumCXXABI.cpp`).

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_llvm::{IntPredicate, TypeId as LTy};

use crate::cleanup::{Cleanup, Depth, Handler, Kind};
use crate::debuginfo::func::VarSlot;
use crate::expr::{LV, Opaque};
use crate::fir::{Blk, Op, V};
use crate::func::Addr;
use crate::module::{Cg, R, err};
use crate::tbaa::Tb;

/// `TargetCodeGenInfo::getSizeOfUnwindException`: the
/// `_Unwind_Exception` header before the thrown object.
const UNWIND_HEADER_SIZE: i128 = 32;

impl Cg<'_> {
    /// `EmitCXXTryStmt` ([stmt.pre], [except.pre]/1).
    pub(crate) fn emit_try_stmt(&mut self, s: StmtId) -> R<()> {
        self.enter_try(s)?;
        self.emit_stmt(self.ast.stmts.try_block(s))?;
        self.exit_try(s, false)
    }

    /// `EnterCXXTryStmt`: pushes a catch scope with one `catch` block per
    /// handler, typed by the type-info of the handler's type without
    /// reference and cv-qualifiers ([except.handle]/3); `catch (...)`
    /// has none.
    pub(crate) fn enter_try(&mut self, s: StmtId) -> R<()> {
        let ast = self.ast;
        let mut handlers = Vec::new();
        for h in ast.lists.iter::<StmtId>(ast.stmts.try_handlers(s)).collect::<Vec<_>>() {
            let blk = self.f.fir.create_block();
            let rtti = match ast.stmts.catch_var(h) {
                Some(v) => {
                    let q = self.var_type(v)?;
                    let q = if ast.types.is_reference(q) {
                        ast.types.pointee(q).expect("referenced type")
                    } else {
                        q
                    };
                    Some(V::C(self.rtti(q)?))
                }
                None => None,
            };
            handlers.push(Handler { rtti, blk });
        }
        self.push_catch(handlers);
        Ok(())
    }

    fn var_type(&self, v: DeclId) -> R<QualType> {
        match self.ast.decls.get(v).ty() {
            Some(q) => Ok(q),
            None => err(Some(self.ast.decls.get(v).loc()), "handler variable without a type"),
        }
    }

    /// `ExitCXXTryStmt`: emits the dispatch and the handlers after the
    /// try-block; `fn_try` for a function-try-block, whose handlers in a
    /// constructor or destructor rethrow when control reaches their end
    /// ([except.handle]/14).
    pub(crate) fn exit_try(&mut self, s: StmtId, fn_try: bool) -> R<()> {
        let ast = self.ast;
        let d = self.f.cl.depth();
        let used = self.f.cl.stack[d - 1].cached_eh.is_some_and(|b| self.f.fir.block_uses(b) > 0);
        if !used {
            self.pop_catch();
            return Ok(());
        }
        self.emit_catch_dispatch(d);
        let Kind::Catch(handlers) = self.pop_catch().kind else {
            unreachable!("catch scope");
        };
        let cont = self.f.fir.create_block();
        if self.f.fir.cur().is_some() {
            self.br(cont);
        }
        let implicit_rethrow = fn_try
            && self.f.decl.is_some_and(|f| {
                matches!(
                    ast.decls.get(f).kind(),
                    DeclKind::CXXConstructor | DeclKind::CXXDestructor
                )
            });
        let stmts: Vec<StmtId> = ast.lists.iter(ast.stmts.try_handlers(s)).collect();
        for (h, &c) in handlers.iter().zip(&stmts).rev() {
            self.emit_block_after_uses(h.blk);
            let sc = self.begin_scope(false);
            self.emit_begin_catch(c)?;
            self.emit_stmt(ast.stmts.catch_block(c))?;
            if implicit_rethrow && self.f.fir.cur().is_some() {
                let f = self.rt_fn("__cxa_rethrow", LTy::VOID, &[], &[]);
                self.runtime_call_or_invoke(f, LTy::VOID, Vec::new(), &[]);
                self.f.fir.push(Op::Unreachable, LTy::VOID);
                self.f.fir.set_cur(None);
            }
            self.end_scope(sc);
            if self.f.fir.cur().is_some() {
                self.br(cont);
            }
        }
        self.emit_block(cont, false);
        Ok(())
    }

    /// `EnterCXXTryStmt` for the implicit `catch (...)` of a coroutine
    /// body (spec/codegen.md §7o).
    pub(crate) fn enter_catch_all(&mut self) {
        let blk = self.f.fir.create_block();
        self.push_catch(vec![Handler { rtti: None, blk }]);
    }

    /// `ExitCXXTryStmt` for [`Cg::enter_catch_all`]: the handler runs
    /// `handler` (`__promise.unhandled_exception();`).
    pub(crate) fn exit_catch_all(&mut self, handler: StmtId) -> R<()> {
        if let Some(cont) = self.exit_catch_all_to(None, |cg| cg.emit_stmt(handler))? {
            self.emit_block(cont, false);
        }
        Ok(())
    }

    /// [`Cg::exit_catch_all`] whose handler body `body` emits (after
    /// `__cxa_begin_catch`, inside the handler's scope); the try region
    /// and a handler that ends normally branch to `cont` (a new block
    /// when `None`), which is returned for the caller to place. `None`:
    /// nothing could throw, no handler emitted (and no branch to `cont`).
    pub(crate) fn exit_catch_all_to(
        &mut self,
        cont: Option<Blk>,
        body: impl FnOnce(&mut Self) -> R<()>,
    ) -> R<Option<Blk>> {
        let d = self.f.cl.depth();
        let used = self.f.cl.stack[d - 1].cached_eh.is_some_and(|b| self.f.fir.block_uses(b) > 0);
        if !used {
            self.pop_catch();
            return Ok(None);
        }
        self.emit_catch_dispatch(d);
        let Kind::Catch(handlers) = self.pop_catch().kind else {
            unreachable!("catch scope");
        };
        let cont = cont.unwrap_or_else(|| self.f.fir.create_block());
        if self.f.fir.cur().is_some() {
            self.br(cont);
        }
        self.emit_block_after_uses(handlers[0].blk);
        let sc = self.begin_scope(false);
        let exn = self.exn_from_slot();
        self.call_begin_catch(exn, true);
        body(self)?;
        self.end_scope(sc);
        if self.f.fir.cur().is_some() {
            self.br(cont);
        }
        Ok(Some(cont))
    }

    /// `emitCatchDispatchBlock`: compares the selector against each
    /// handler's type index (`llvm.eh.typeid.for`), falling through to
    /// the enclosing EH scope.
    fn emit_catch_dispatch(&mut self, d: Depth) {
        let Kind::Catch(hs) = self.f.cl.stack[d - 1].kind.clone() else {
            unreachable!("catch scope");
        };
        let dispatch = self.f.cl.stack[d - 1].cached_eh.expect("dispatch block");
        if hs.len() == 1 && hs[0].rtti.is_none() {
            return;
        }
        let saved = self.f.fir.cur();
        self.emit_block_after_uses(dispatch);
        let typeid_for = kiln_llvm::intrinsics::eh_typeid_for(&mut self.m);
        let ss = self.sel_slot();
        let sel = self.load(LTy::I32, ss, 4, false);
        for i in 0..hs.len() {
            let h = hs[i];
            let (next, end) = if i + 1 == hs.len() {
                let enc = self.f.cl.enclosing_eh(d);
                (self.eh_dispatch_block(enc), true)
            } else if hs[i + 1].rtti.is_none() {
                (hs[i + 1].blk, true)
            } else {
                (self.f.fir.create_block(), false)
            };
            let rtti = h.rtti.expect("not a catch-all");
            let tid = self.nounwind_runtime_call(typeid_for, LTy::I32, vec![rtti]);
            let m = self.icmp(IntPredicate::Eq, sel, tid);
            self.cond_br(m, h.blk, next);
            if end {
                break;
            }
            self.emit_block(next, false);
        }
        self.f.fir.set_cur(saved);
    }

    /// `EmitBlockAfterUses`: places `b` after the block of its most
    /// recent user (first in LLVM's use list), else at the end.
    fn emit_block_after_uses(&mut self, b: Blk) {
        let after = self
            .f
            .fir
            .branches_to(b)
            .into_iter()
            .max()
            .and_then(|i| self.f.fir.inst_block(i))
            .filter(|&p| self.f.fir.is_placed(p));
        match after {
            Some(p) => self.f.fir.place_after(p, b),
            None => self.f.fir.place_at_end(b),
        }
        self.f.fir.set_cur(Some(b));
    }

    /// `ItaniumCXXABI::emitBeginCatch`: the handler variable (if any),
    /// initialized from the exception object, under the end-catch
    /// cleanup.
    fn emit_begin_catch(&mut self, h: StmtId) -> R<()> {
        let ast = self.ast;
        let Some(var) = ast.stmts.catch_var(h) else {
            let exn = self.exn_from_slot();
            self.call_begin_catch(exn, true);
            return Ok(());
        };
        // `EmitAutoVarAlloca`.
        let q = self.var_type(var)?;
        let ty = self.mty(q);
        let align = self.var_align(var);
        let a = self.f.fir.alloca(ty, align);
        let addr = Addr { v: a, ty, align };
        self.f.locals.insert(var, addr);
        if self.dbg_on() {
            self.di_declare_var(var, VarSlot { addr: a, arg: None, deref: false });
        }
        self.init_catch_param(var, q, addr)?;
        // `EmitAutoVarCleanups`.
        if self.f.fir.cur().is_none() {
            return Ok(());
        }
        if let Some(rd) = self.destructed_class(q) {
            self.push_cleanup(Cleanup::Dtor { addr: a, record: rd, base: false });
        }
        Ok(())
    }

    /// `CallBeginCatch`: `__cxa_begin_catch(exn)`, then the end-catch
    /// cleanup (`might_throw`: the caught object's destructor may run
    /// in `__cxa_end_catch`).
    fn call_begin_catch(&mut self, exn: V, might_throw: bool) -> V {
        let f = self.rt_fn("__cxa_begin_catch", LTy::PTR, &[LTy::PTR], &[]);
        let r = self.nounwind_runtime_call(f, LTy::PTR, vec![exn]);
        self.push_cleanup(Cleanup::EndCatch { might_throw });
        r
    }

    /// `InitCatchParam`: initializes the handler variable `var` of type
    /// `q` at `addr` from the exception object ([except.handle]/15).
    fn init_catch_param(&mut self, var: DeclId, q: QualType, addr: Addr) -> R<()> {
        let ast = self.ast;
        let exn = self.exn_from_slot();
        if ast.types.is_reference(q) {
            let caught = ast.types.pointee(q).expect("referenced type");
            let rec = ast.types.as_record(caught).is_some();
            let mut adj = self.call_begin_catch(exn, rec);
            // A pointer caught by reference: `__cxa_begin_catch` returns
            // the pointer itself, not its address.
            if ast.types.is_pointer(caught) {
                let pt = ast.types.pointee(caught).expect("pointee");
                if ast.types.as_record(pt).is_none() {
                    let off = V::C(self.m.const_int(LTy::I32, UNWIND_HEADER_SIZE));
                    adj = self.gep(LTy::I8, exn, vec![off], false);
                } else {
                    // `exn.byref.tmp`: the adjusted pointer, by value.
                    let tmp = self.f.fir.alloca(LTy::PTR, 8);
                    self.store(adj, tmp, 8, false);
                    adj = tmp;
                }
            }
            self.store(adj, addr.v, addr.align, false);
            return Ok(());
        }
        if !self.is_aggregate(q) {
            let adj = self.call_begin_catch(exn, false);
            let nullptr =
                matches!(ast.types.canonical_view(q), TypeView::Builtin(BuiltinType::NullPtr));
            if ast.types.is_pointer(q) || nullptr {
                self.store(adj, addr.v, addr.align, false);
                return Ok(());
            }
            let align = self.natural_align(q);
            let src = LV {
                addr: Addr { v: adj, ty: addr.ty, align },
                q,
                bf: None,
                vidx: None,
                tb: Tb::Type,
            };
            let v = self.load_lv(src);
            self.store_scalar(v, addr, q);
            return Ok(());
        }
        let Some(rd) = ast.types.as_record(q) else {
            return err(Some(ast.decls.get(var).loc()), "an array handler type");
        };
        let (_, exn_align) = self.this_layout(rd);
        let init = ast.decls.var(var).and_then(|v| v.init);
        let Some(init) = init else {
            // A trivial copy (`EmitAggregateCopy`).
            let adj = self.call_begin_catch(exn, true);
            if !self.class_bits(rd).contains(kiln_ast::decl::DefinitionBits::EMPTY) {
                let n = self.copy_size(rd, false);
                self.memcpy(addr, adj, exn_align, n, false);
            }
            return Ok(());
        };
        let gep = self.rt_fn("__cxa_get_exception_ptr", LTy::PTR, &[LTy::PTR], &[]);
        let adj = self.nounwind_runtime_call(gep, LTy::PTR, vec![exn]);
        let opaque = self.find_in_copy_construct(init)?;
        let lv = LV {
            addr: Addr { v: adj, ty: addr.ty, align: exn_align },
            q,
            bf: None,
            vidx: None,
            tb: Tb::Type,
        };
        self.f.opaque.insert(opaque, Opaque::Lv(lv));
        // The copy constructor runs in a terminate scope
        // ([except.terminate]/1.1).
        self.push_terminate();
        let r = self.emit_agg_into(init, addr, false);
        self.pop_terminate();
        self.f.opaque.remove(&opaque);
        r?;
        self.call_begin_catch(exn, true);
        Ok(())
    }

    /// `OpaqueValueExpr::findInCopyConstruct`.
    fn find_in_copy_construct(&self, init: ExprId) -> R<ExprId> {
        let ast = self.ast;
        let mut e = init;
        if ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups {
            e = self.sub(e);
        }
        if ast.exprs.get(e).kind() == ExprKind::MaterializeTemporaryExpr {
            e = self.sub(e);
        }
        if ast.exprs.get(e).kind() != ExprKind::CXXConstructExpr {
            return err(self.loc_of(init), "handler initializer is not a construction");
        }
        let Some(mut e) = ast.lists.iter::<ExprId>(ast.exprs.construct_args(e)).next() else {
            return err(self.loc_of(init), "handler copy without an argument");
        };
        while ast.exprs.get(e).kind() == ExprKind::ImplicitCastExpr {
            e = ast.exprs.operand(e);
        }
        if ast.exprs.get(e).kind() != ExprKind::OpaqueValueExpr {
            return err(self.loc_of(init), "handler copy not of the exception object");
        }
        Ok(e)
    }
}
