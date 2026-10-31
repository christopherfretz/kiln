//! Closure members without a body of their own (spec/codegen.md, WP-075):
//! the static invoker `__invoke` of a captureless lambda (clang
//! `CodeGenFunction::EmitLambdaStaticInvokeBody`).

use kiln_ast::DeclId;
use kiln_llvm::{TypeId as LTy, Value};

use crate::fir::V;
use crate::func::CallArg;
use crate::module::{Cg, R};

impl Cg<'_> {
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
            vals.push(if self.referee(q).is_some() {
                CallArg::Scalar(self.load(LTy::PTR, a.v, a.align, false))
            } else if self.is_aggregate(q) {
                CallArg::Agg(a)
            } else {
                CallArg::Scalar(self.load_scalar(a, q))
            });
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
