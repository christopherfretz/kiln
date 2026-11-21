//! Lowering of lambda captures (WP-075; spec/sema-lambda.md
//! "Evaluation"): in the body of a closure's call operator, a name
//! denoting a captured entity is an access to the closure member
//! capturing it ([expr.prim.lambda.capture]/11), and `this` is the
//! captured `this` or the address of the captured `*this` copy
//! ([expr.prim.this]/2, [expr.prim.lambda.capture]/10).

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprFlags, NonOdrUse};
use kiln_ast::lambda::CaptureKind;
use kiln_ast::{DeclId, ExprId, LambdaId};

use super::{Compiler, Local, Place, is_reference_decl};
use crate::kbc::{Op, ak};
use crate::ty::OTy;

impl Compiler<'_> {
    /// The prologue of lambda call operator `def`: the closure object is
    /// the implicit or explicit object parameter
    /// ([expr.prim.lambda.closure]/5), and the `this` of its body is the
    /// captured one (clang `StartFunction`).
    pub(super) fn lambda_prologue(&mut self, def: DeclId) {
        let ast = self.ast();
        let Some(l) = ast.call_op_lambda(def) else { return };
        let closure = match self.this_reg {
            Some(r) => r,
            None => {
                let explicit =
                    ast.decls.function(def).is_some_and(|f| f.spec.has_explicit_object());
                let Some(&obj) = self.cur_params.first().filter(|_| explicit) else { return };
                let Some(r) = self.explicit_object_closure(l, obj) else { return };
                r
            }
        };
        self.closure_reg = Some(closure);
        let ast = self.ast();
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

    /// The address of the closure object of the call operator of lambda
    /// `l` with explicit object parameter `obj`: the parameter, or its
    /// referent, as its closure-type base when it is of a class derived
    /// from it (clang `EmitLValueForLambdaField`).
    fn explicit_object_closure(&mut self, l: LambdaId, obj: DeclId) -> Option<u32> {
        let Some(Local::Mem(p)) = self.locals.get(&obj).copied() else { return None };
        let ast = self.ast();
        let pq = ast.decls.get(obj).ty()?;
        let cls = if ast.types.is_reference(pq) { ast.types.pointee(pq)? } else { pq };
        let cq = ast.decls.get(ast.exprs.lambdas.get(l).closure).ty()?;
        let (from, to) = (self.ty(cls), self.ty(cq));
        let off = self.nv_base_offset(from, to, 0)?;
        if off == 0 {
            return Some(p);
        }
        let r = self.reg();
        self.emit(Op::Field, 0, &[r, p, off as u32]);
        Some(r)
    }

    /// The offset of base class `base` in class `cls`, reached through
    /// non-virtual bases only (depth-first in declaration order).
    fn nv_base_offset(&self, cls: OTy, base: OTy, depth: u32) -> Option<u64> {
        if self.o.types.same_class(cls, base) {
            return Some(0);
        }
        if depth > 64 {
            return None;
        }
        self.o.types.fields(cls)[..self.o.types.vbase_start(cls)]
            .iter()
            .filter(|f| f.base)
            .find_map(|f| self.nv_base_offset(f.ty, base, depth + 1).map(|x| x + f.offset))
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
        let Some(closure) = self.closure_reg else {
            return self.closure_copy_outside_call(e, d);
        };
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

    /// P2686R5 [expr.const.init]/3-9: the variable `e`, a name of
    /// variable or binding `d` in a lambda body that denotes an enclosing
    /// entity (sema's `REFERS_TO_ENCLOSING`) and odr-uses it, evaluated
    /// outside a call of the lambda (a `static_assert` or a constant
    /// initializer in the body), reads, when that variable's value refers
    /// to an automatic object: `d` itself, the decomposition variable of a
    /// binding of a reference `auto& [a, b] = cl;`, or a tuple-like
    /// binding's reference holding variable. The automatic object is not
    /// constexpr-referenceable from the lambda body (another function
    /// parameter scope), so the value is not constexpr-representable
    /// there and the variable's object is not usable in constant
    /// expressions: it is an external object ([`super::GKind::Unrepresentable`]).
    /// Sema made the name an odr-use for the same reason
    /// (`Sema::representable_here`). A CWG 3135 holding variable (an
    /// object of type `Ti`) and a by-copy capture of a non-reference
    /// (`closure_copy_outside_call`) are not this case.
    pub(super) fn unrepresentable_here(&mut self, e: ExprId, d: DeclId) -> Option<DeclId> {
        if self.closure_reg.is_some() || !self.node(e).has(ExprFlags::REFERS_TO_ENCLOSING) {
            return None;
        }
        let ast = self.ast();
        if ast.exprs.non_odr_use(e) != NonOdrUse::None {
            return None;
        }
        let v = match ast.decls.get(d).kind() {
            DeclKind::Var => d,
            DeclKind::Binding => {
                let v = match ast.decls.binding_var(d) {
                    Some(h) if crate::value::holding_object_temp(ast, h).is_some() => return None,
                    Some(h) => h,
                    None => ast.decls.binding_decomp(d)?,
                };
                if !is_reference_decl(ast, v) {
                    return None;
                }
                v
            }
            _ => return None,
        };
        let v = ast.first(v);
        let Some(kiln_ast::decl::VarEvalState::Value(c)) = ast
            .redecls_of(v)
            .find_map(|r| ast.decls.var(r).filter(|x| x.init.is_some()))
            .map(|x| x.eval.state())
        else {
            return None;
        };
        crate::value::automatic_referent(ast, c).map(|_| v)
    }

    /// The place of `e`, a name of variable `d` captured by copy, evaluated
    /// outside a call of its lambda's call operator (a constant expression
    /// in the body: `constexpr int j = *&n;`): an odr-use names a member
    /// of a closure object that does not exist in this evaluation
    /// ([expr.prim.lambda.capture]/11, [expr.const]/10), so it is the
    /// unbacked [`GKind::Capture`] object: reading it is diagnosed and its
    /// address is not a constant. A name that is not an odr-use (sema's
    /// `NonOdrUse`) is the variable itself, read as before; so is a
    /// by-reference capture, which names the variable
    /// (spec/sema-lambda.md §7).
    ///
    /// [`GKind::Capture`]: super::GKind::Capture
    fn closure_copy_outside_call(&mut self, e: ExprId, d: DeclId) -> Option<Place> {
        let ast = self.ast();
        if !self.node(e).has(ExprFlags::NAMES_CLOSURE_COPY)
            || ast.exprs.non_odr_use(e) != NonOdrUse::None
            || !matches!(ast.decls.get(d).kind(), DeclKind::Var | DeclKind::Binding)
            || ast.decls.get(d).ty().is_none_or(|q| ast.types.is_reference(q))
        {
            return None;
        }
        let d = ast.first(d);
        let ty = self.ety(e);
        let g = self.o.capture_global(d, ty);
        let r = self.reg();
        self.emit_at(Op::GlobalAddr, 0, &[r, g], e);
        Some(Place::Mem(r))
    }
}
