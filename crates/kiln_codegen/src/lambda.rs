//! Closure members without a body of their own (spec/codegen.md, WP-075):
//! the static invoker `__invoke` of a captureless lambda (clang
//! `CodeGenFunction::EmitLambdaStaticInvokeBody`).

use kiln_ast::lambda::CaptureKind;
use kiln_ast::{DeclId, LambdaId};
use kiln_llvm::{TypeId as LTy, Value};

use crate::expr::LV;
use crate::fir::V;
use crate::func::Addr;
use crate::module::{Cg, R};

impl Cg<'_> {
    /// clang `StartFunction` in a lambda's function call operator `l`:
    /// remembers the closure object `closure`, and points `this` at the
    /// captured object (the copy for `*this`, the pointer held for
    /// `this`).
    pub(crate) fn lambda_prolog(&mut self, l: LambdaId, closure: V) -> R<()> {
        let ast = self.ast;
        self.f.closure = Some((closure, l));
        let data = ast.exprs.lambdas.get(l);
        let Some(c) = data.this_capture() else { return Ok(()) };
        let Some(field) = c.field else { return Ok(()) };
        let kind = c.kind;
        let fq = ast.decls.get(field).ty().expect("capture type");
        let lv = self.closure_field_lvalue(field, fq)?;
        let (this, obj) = if kind == CaptureKind::StarThis {
            (lv.addr.v, fq)
        } else {
            let p = self.load(LTy::PTR, lv.addr.v, lv.addr.align, false);
            (p, ast.types.pointee(fq).unwrap_or(fq))
        };
        self.f.this = Some(this);
        if let Some(rd) = ast.types.as_record(obj) {
            self.f.this_class = Some(rd);
        }
        Ok(())
    }

    /// The closure object of call operator of lambda `l` with explicit
    /// object parameter `obj` ([expr.prim.lambda.closure]/5; clang
    /// `EmitLValueForLambdaField`): the parameter, or its referent, as
    /// its closure-type base when it is of a class derived from it.
    pub(crate) fn explicit_object_closure(&mut self, l: LambdaId, obj: DeclId) -> R<V> {
        let ast = self.ast;
        let pq = ast.decls.get(obj).ty().expect("param type");
        let a = self.f.locals[&obj];
        let (p, cls) = if ast.types.is_reference(pq) {
            let t = ast.types.pointee(pq).unwrap_or(pq);
            (self.load(LTy::PTR, a.v, a.align, false), t)
        } else {
            (a.v, pq)
        };
        let closure = ast.exprs.lambdas.get(l).closure;
        let Some(rd) = ast.types.as_record(cls) else { return Ok(p) };
        if ast.first(rd) == ast.first(closure) {
            return Ok(p);
        }
        match self.nv_base_offset(rd, closure, 0) {
            Some(off) => Ok(self.byte_offset(p, off)),
            None => crate::module::err(
                Some(ast.decls.get(obj).loc()),
                "explicit object parameter of a class not derived non-virtually from the closure",
            ),
        }
    }

    /// The offset of `base`, reached through non-virtual bases only, in
    /// class `rd` (depth-first in declaration order).
    fn nv_base_offset(&mut self, rd: DeclId, base: DeclId, depth: u32) -> Option<u64> {
        let ast = self.ast;
        if ast.first(rd) == ast.first(base) {
            return Some(0);
        }
        if depth > 64 {
            return None;
        }
        let def = self.class_def(rd)?;
        let bases = self.tys.layouter().record(def).bases.clone();
        bases.into_iter().find_map(|(b, o)| self.nv_base_offset(b, base, depth + 1).map(|x| x + o))
    }

    /// clang `EmitLValueForLambdaField`: capture member `field` of the
    /// closure object of the call operator being emitted.
    pub(crate) fn closure_field_lvalue(&mut self, field: DeclId, q: kiln_ast::QualType) -> R<LV> {
        let ast = self.ast;
        let (v, l) = self.f.closure.expect("in a lambda call operator");
        let rd = ast.exprs.lambdas.get(l).closure;
        let ty = self.record_ty(rd);
        let (_, align) = self.this_layout(rd);
        self.field_lvalue(Addr { v, ty, align }, field, q)
    }

    /// clang `EmitCapturedFieldLValue` for a reference to variable `d`
    /// (flagged as naming an enclosing variable or capture): the member
    /// capturing it in the current call operator's closure, if any.
    pub(crate) fn captured_var_lvalue(
        &mut self,
        d: DeclId,
        q: kiln_ast::QualType,
    ) -> R<Option<LV>> {
        let ast = self.ast;
        let Some((_, l)) = self.f.closure else { return Ok(None) };
        let d = ast.first(d);
        let field =
            ast.exprs.lambdas.get(l).captures.iter().find_map(|c| {
                (c.var.map(|v| ast.first(v)) == Some(d)).then_some(c.field).flatten()
            });
        let Some(field) = field else { return Ok(None) };
        let fq = ast.decls.get(field).ty().unwrap_or(q);
        let lv = self.closure_field_lvalue(field, fq)?;
        // The member's type may differ from the expression's in
        // qualification (a non-`mutable` lambda's copies are `const`).
        Ok(Some(LV { q, ..lv }))
    }

    /// clang `EmitLambdaDelegatingInvokeBody` + `EmitForwardingCallToLambda`:
    /// the body of the static invoker (parameters `params`, spilled by the
    /// prolog) calls the call operator `call_op` on an unused closure
    /// temporary, forwarding each parameter (`EmitDelegateCallArg`), and
    /// returns its result.
    pub(crate) fn emit_lambda_invoke_body(&mut self, call_op: DeclId, params: &[DeclId]) -> R<()> {
        let ast = self.ast;
        let first = ast.first(call_op);
        let Some(closure) = ast.decls.get(first).ctx() else {
            return crate::module::err(
                Some(ast.decls.get(call_op).loc()),
                "invoker without a closure",
            );
        };
        let cq = self.class_type(closure);
        let unused = self.temp(cq);
        let mut vals = Vec::with_capacity(params.len());
        let mut ptys = Vec::with_capacity(params.len());
        for &p in params {
            let q = ast.decls.get(p).ty().expect("param type");
            let a = self.f.locals[&p];
            vals.push(self.delegate_call_arg(a, q));
            ptys.push(q);
        }
        let fty = ast.decls.get(first).ty().expect("call operator type");
        let sig = self.call_sig_this(fty, false, &ptys, Some(closure));
        let f = self.get_fn(call_op)?;
        let rq = sig.ret;
        let agg = self.is_aggregate(rq);
        // The return slot is the caller's only when returned indirectly;
        // an aggregate returned directly lands in a temporary that is then
        // copied (`EmitReturnOfRValue`).
        let dest = if sig.sret() {
            self.f.ret_slot
        } else if agg {
            Some(self.temp(rq))
        } else {
            None
        };
        let r = self.call_vals(
            &sig,
            V::C(Value::func(f)),
            Some(f),
            Some(unused.v),
            &vals,
            &[],
            dest,
            &[],
        )?;
        if sig.sret() {
            return Ok(());
        }
        self.delegate_cx_return(rq, sig.sret());
        if let Some(slot) = self.f.ret_slot {
            if agg {
                if let Some(tmp) = dest {
                    let size = self.size_align(rq).0;
                    self.memcpy(slot, tmp.v, tmp.align, size, false);
                }
            } else if let Some(x) = r {
                self.store_scalar(x, slot, rq);
            }
        }
        Ok(())
    }
}
