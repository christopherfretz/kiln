//! Lowering of lambda captures (WP-075; spec/sema-lambda.md
//! "Evaluation"): in the body of a closure's call operator, a name
//! denoting a captured entity is an access to the closure member
//! capturing it ([expr.prim.lambda.capture]/11), and `this` is the
//! captured `this` or the address of the captured `*this` copy
//! ([expr.prim.this]/2, [expr.prim.lambda.capture]/10).

use kiln_ast::expr::ExprFlags;
use kiln_ast::lambda::CaptureKind;
use kiln_ast::{DeclId, ExprId, LambdaId};

use super::{Compiler, Place, is_reference_decl};
use crate::kbc::{Op, ak};
use crate::ty::OTy;

impl Compiler<'_> {
    /// The prologue of lambda call operator `def`: the `this` of its
    /// body is the captured one (clang `StartFunction`).
    pub(super) fn lambda_prologue(&mut self, def: DeclId) {
        let Some(closure) = self.this_reg else { return };
        let ast = self.ast();
        let Some(l) = ast.call_op_lambda(def) else { return };
        let Some((kind, field)) =
            ast.exprs.lambdas.get(l).this_capture().and_then(|c| Some((c.kind, c.field?)))
        else {
            return;
        };
        let Some((p, fty)) = self.closure_member(closure, l, field, None) else { return };
        self.lambda_this = Some(if kind == CaptureKind::StarThis {
            p
        } else {
            let r = self.reg();
            self.emit(Op::Load, ak::READ, &[r, p, fty.0]);
            r
        });
    }

    /// The address and type of capture member `field` of the closure
    /// object `closure` points to (lambda `l`).
    fn closure_member(
        &mut self,
        closure: u32,
        l: LambdaId,
        field: DeclId,
        e: Option<ExprId>,
    ) -> Option<(u32, OTy)> {
        let ast = self.ast();
        let q = ast.decls.get(ast.exprs.lambdas.get(l).closure).ty()?;
        let rt = self.ty(q);
        let fi = *self.o.types.fields(rt).iter().find(|f| f.decl == field)?;
        if fi.offset == 0 {
            return Some((closure, fi.ty));
        }
        let r = self.reg();
        let args = [r, closure, fi.offset as u32];
        match e {
            Some(e) => self.emit_at(Op::Field, 1, &args, e),
            None => self.emit(Op::Field, 1, &args),
        };
        Some((r, fi.ty))
    }

    /// The place of `e`, a name of variable `d` flagged as denoting an
    /// enclosing variable, when the call operator being compiled captures
    /// it: its closure member, or the referent of a reference member.
    pub(super) fn captured_var_place(&mut self, e: ExprId, d: DeclId) -> Option<Place> {
        if !self.node(e).has(ExprFlags::REFERS_TO_ENCLOSING) {
            return None;
        }
        let closure = self.this_reg?;
        let ast = self.ast();
        let l = ast.call_op_lambda(self.cur_fn?)?;
        let d = ast.first(d);
        let field =
            ast.exprs.lambdas.get(l).captures.iter().find_map(|c| {
                (c.var.map(|v| ast.first(v)) == Some(d)).then_some(c.field).flatten()
            })?;
        let (p, fty) = self.closure_member(closure, l, field, Some(e))?;
        if is_reference_decl(self.ast(), field) {
            let a = self.reg();
            self.emit_at(Op::Load, ak::READ, &[a, p, fty.0], e);
            return Some(Place::Mem(a));
        }
        Some(Place::Mem(p))
    }
}
