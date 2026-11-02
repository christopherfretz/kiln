//! Lowering of throw-expressions and try-blocks (C++26 P3068, [except];
//! spec/oven.md "Exceptions"). Before C++26 a throw-expression is not a
//! core constant expression and a try-block is its compound statement
//! (clang 19).

use kiln_ast::{DeclId, ExprId, StmtId};

use super::{Compiler, Local, Place};
use crate::kbc::{NO_DTOR, Op};
use crate::mem::Origin;
use crate::vm::exc::EXC_FREE;
use crate::ty::TyKind;

impl Compiler<'_> {
    /// A throw-expression ([expr.throw]): the exception object is a heap
    /// object initialized from the operand; `throw;` rethrows the handled
    /// exception.
    pub(super) fn throw_expr(&mut self, e: ExprId) {
        if !self.b.cxx26() {
            return self.throw_invalid(e);
        }
        let ast = self.ast();
        let loc = ast.exprs.get(e).loc();
        let Some(x) = ast.exprs.throw_operand(e) else {
            self.emit_site(Op::Throw, 1, &[0, 0, 0], e, loc);
            return;
        };
        let m = self.next;
        let ty = self.ety(x);
        let dst = self.reg();
        let oi = self.o.push_origin(Origin::Heap(e, 0));
        self.emit_site(Op::New, 0, &[dst, ty.0, dst, oi, 0], e, loc);
        // An exception from the initialization releases the storage, as a
        // new-expression's ([expr.new]/28); `Throw` drops the registration.
        self.emit_loc(Op::PushCleanup, 4, &[dst, EXC_FREE, ty.0], loc);
        self.init(x, dst, ty);
        let dtor = self.dtor_of(ty).unwrap_or(NO_DTOR);
        self.emit_site(Op::Throw, 0, &[dst, ty.0, dtor], e, loc);
        self.next = m;
    }

    /// A try-block ([except.pre], [except.handle]): its handlers are
    /// registered for the compound statement; an exception unwound to
    /// them is matched against each in order, and resumes unwinding when
    /// none matches. `cdtor` is the constructor or destructor whose
    /// function-try-block this is: its ctor-initializers or the
    /// destruction of its subobjects are inside the try-block, and
    /// reaching the end of one of its handlers rethrows the handled
    /// exception ([except.handle]/13, 15).
    pub(super) fn try_stmt(&mut self, s: StmtId, cdtor: Option<DeclId>) {
        let ast = self.ast();
        let block = ast.stmts.try_block(s);
        let rethrow = cdtor.is_some();
        if !self.b.cxx26() {
            if let Some(def) = cdtor {
                self.member_prologue(def);
            }
            return self.stmt(block);
        }
        let try_loc = ast.stmts.get(s).loc();
        let handlers: Vec<StmtId> = ast.lists.iter::<StmtId>(ast.stmts.try_handlers(s)).collect();
        let m = self.next;
        self.open_scope(&[], true);
        let Some(&Some(mark)) = self.scopes.last() else { return };
        let at = self.emit(Op::TryEnter, 0, &[0]) + 1;
        if let Some(def) = cdtor {
            self.member_prologue(def);
        }
        self.stmt(block);
        self.close_scope();
        let done = self.jmp();
        // Unwound here: the try-block's memory is released (its
        // destructions ran while unwinding).
        self.patch_here(at);
        self.emit(Op::ScopeExit, 0, &[mark]);
        let mut ends = Vec::with_capacity(handlers.len());
        for h in handlers {
            ends.push(self.handler(h, rethrow));
        }
        self.emit_loc(Op::Throw, 2, &[0, 0, 0], try_loc);
        for j in ends {
            self.patch_here(j);
        }
        self.patch_here(done);
        self.next = m;
    }

    /// One handler: matches the in-flight exception ([except.handle]/3)
    /// and runs, or falls through to the next. Returns its exit jump.
    fn handler(&mut self, h: StmtId, rethrow: bool) -> usize {
        let m = self.next;
        let ast = self.ast();
        let var = ast.stmts.catch_var(h);
        let body = ast.stmts.catch_block(h);
        let hloc = ast.stmts.get(h).loc();
        // The handler's type: references and top-level cv dropped.
        let (aux, t, by_ref) = match var {
            None => (0, crate::ty::OTy(0), false),
            Some(v) => {
                let q = ast.decls.get(v).ty().unwrap_or(kiln_ast::QualType::INT);
                let by_ref = ast.types.is_reference(q);
                let q = if by_ref { ast.types.pointee(q).unwrap_or(q) } else { q };
                // `cv T` or `const T&` with T a pointer: converted
                // ([except.handle]/3.3-3.4); `T&` binds the object itself.
                let cq = ast.types.canonical(q).quals();
                let t = self.ty(q);
                let ptr = matches!(
                    self.kind(t),
                    TyKind::Ptr(_) | TyKind::MemPtr { .. } | TyKind::NullPtr
                );
                let conv = ptr && (!by_ref || (cq.is_const() && !cq.is_volatile()));
                (if conv { 2 } else { 1 }, t, by_ref && !conv)
            }
        };
        let dst = self.reg();
        let next = self.emit(Op::Catch, aux, &[dst, t.0, 0]) + 3;
        let decls: Vec<_> = var.into_iter().collect();
        self.open_scope(&decls, true);
        self.emit_loc(Op::CatchBegin, 0, &[], hloc);
        if let Some(v) = var {
            if by_ref {
                self.locals.insert(v, Local::Mem(dst));
            } else if aux == 2
                && self
                    .ast()
                    .types
                    .is_reference(self.ast().decls.get(v).ty().unwrap_or(kiln_ast::QualType::INT))
            {
                // `const T&` to a converted pointer: bound to a temporary
                // holding the converted value.
                let p = self.reg();
                self.alloc_var(p, t, v, true);
                self.emit(Op::InitStore, 0, &[p, dst, t.0]);
                self.locals.insert(v, Local::Mem(p));
            } else if let Some(Local::Mem(p)) = self.locals.get(&v).copied() {
                let init = self.ast().decls.var(v).and_then(|x| x.init);
                if let Some(i) = init {
                    let saved = self.catch_obj.replace(dst);
                    self.init(i, p, t);
                    self.catch_obj = saved;
                } else if aux == 2 {
                    self.emit(Op::InitStore, 0, &[p, dst, t.0]);
                } else {
                    self.emit(Op::Copy, 1, &[p, dst, t.0]);
                }
                self.end_local_init(v, p, t);
            } else {
                self.trap_loc(hloc, "kiln: internal: a handler's parameter has no storage".into());
            }
        }
        self.stmt(body);
        if rethrow {
            let end = self.ast().stmts.rbrace(body);
            self.emit_loc(Op::Throw, 1, &[0, 0, 0], end);
        }
        self.close_scope();
        let end = self.jmp();
        self.patch_here(next);
        self.next = m;
        end
    }

    /// The exception object a handler's parameter is initialized from
    /// (the `OpaqueValueExpr` of its initializer, which has no source).
    pub(super) fn catch_object(&self) -> Option<Place> {
        self.catch_obj.map(Place::Mem)
    }
}
