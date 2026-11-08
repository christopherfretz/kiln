//! Constant evaluation of GNU `_Complex` values (WP-211; spec/oven.md
//! "Complex values").
//!
//! A complex object is stored as `T[2]` (`ty.rs`): the real part, then the
//! imaginary part. The operations clang 19's `ComplexExprEvaluator` folds
//! are computed on the two parts in registers (`cx_regs`), the operands in
//! order (left, then right); the result is stored into the destination
//! object (`cx_init`) or kept in registers for an enclosing operation.
//! A complex value produced by anything else (a call, a list, `?:`) is
//! materialized and its parts loaded.
//!
//! As in clang, a real floating operand of a binary operator stays real:
//! it gets a `+0` imaginary part for the Annex G multiplication and
//! division, and the other forms compute only the parts it affects, with
//! the `float` operators that fault on a NaN result or a division by
//! zero (`handleFloatFloatBinOp`). A compound assignment with a complex
//! operand, which clang 19 and 22 do not fold, is folded as GCC does
//! (WP-380): `a = a op b` on the parts.

use kiln_ast::consts::FloatKind;

use kiln_ast::decl::builtin;
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, UnaryOp};
use kiln_ast::{ExprId, QualType};

use super::{Access, Compiler, Place, cmp_of, float_code, int_of, skip_parens};
use crate::complex::cx;
use crate::kbc::{Op, ak, int_aux};
use crate::ty::{OTy, TyKind};

const INVALID: &str = "subexpression not valid in a constant expression";

impl Compiler<'_> {
    /// The element type of complex type `t`.
    pub(super) fn cx_elem(&self, t: OTy) -> Option<OTy> {
        if !self.o.types.is_complex(self.b.ast(), t) {
            return None;
        }
        match self.kind(t) {
            TyKind::Array { elem, .. } => Some(elem),
            _ => None,
        }
    }

    /// Whether expression `e` has complex type.
    pub(super) fn is_complex_expr(&mut self, e: ExprId) -> bool {
        let t = self.ety(e);
        self.cx_elem(t).is_some()
    }

    /// The two arguments of `e` when it is a call of `__builtin_complex`.
    fn builtin_complex_args(&self, e: ExprId) -> Option<[ExprId; 2]> {
        let ast = self.ast();
        if ast.exprs.get(e).kind() != ExprKind::CallExpr {
            return None;
        }
        let mut c = ast.exprs.callee(e);
        while ast.exprs.get(c).kind().is_cast() || ast.exprs.get(c).kind() == ExprKind::ParenExpr {
            c = ast.exprs.operand(c);
        }
        if ast.exprs.get(c).kind() != ExprKind::DeclRefExpr
            || ast.decls.builtin_id(ast.exprs.decl(c)) != Some(builtin::COMPLEX)
        {
            return None;
        }
        match ast.lists.get(ast.exprs.call_args(e))[..] {
            [a, b] => Some([ExprId::new(a), ExprId::new(b)]),
            _ => None,
        }
    }

    /// Whether complex prvalue `e` is an operation computed on its parts
    /// here (rather than by the generic aggregate initialization).
    pub(super) fn cx_op(&self, e: ExprId) -> bool {
        let ast = self.ast();
        let n = ast.exprs.get(e);
        if n.value_category().is_glvalue() {
            return false;
        }
        match n.kind() {
            ExprKind::UnaryOperator => matches!(
                ast.exprs.unary_op(e),
                UnaryOp::Plus
                    | UnaryOp::Minus
                    | UnaryOp::Not
                    | UnaryOp::Extension
                    | UnaryOp::PostInc
                    | UnaryOp::PostDec
            ),
            ExprKind::BinaryOperator => matches!(
                ast.exprs.binary_op(e),
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
            ),
            ExprKind::CallExpr => self.builtin_complex_args(e).is_some(),
            ExprKind::ImaginaryLiteral => true,
            // A substituted `_Complex` template argument (WP-810).
            ExprKind::SubstNonTypeTemplateParmExpr => self.cx_op(ast.exprs.subst_replacement(e)),
            ExprKind::ConstantExpr => self.cx_const(e, None).is_some(),
            k if k.is_cast() => matches!(
                ast.exprs.cast_kind(e),
                CastKind::IntegralRealToComplex
                    | CastKind::FloatingRealToComplex
                    | CastKind::IntegralComplexCast
                    | CastKind::FloatingComplexCast
                    | CastKind::IntegralComplexToFloatingComplex
                    | CastKind::FloatingComplexToIntegralComplex
            ),
            _ => false,
        }
    }

    /// Initializes the complex object of type `ty` at `p` from operation
    /// `e` (`cx_op`).
    pub(super) fn cx_init(&mut self, e: ExprId, p: u32, ty: OTy) {
        let Some(elem) = self.cx_elem(ty) else { return self.unsupported(e) };
        let [re, im] = self.cx_regs(e);
        let es = self.o.types.size(elem);
        self.emit_at(Op::InitStore, 0, &[p, re, elem.0], e);
        let q = self.elem_ptr(p, es, e);
        self.emit_at(Op::InitStore, 0, &[q, im, elem.0], e);
    }

    /// The register values of the parts of `e` when it is a constant
    /// expression holding a frozen complex value (a substituted `_Complex`
    /// template argument, WP-810): each part read from the image's bytes
    /// (little-endian, real part first). With no `elem` the parts are
    /// not decoded (zero): only whether there is such a value.
    fn cx_const(&self, e: ExprId, elem: Option<OTy>) -> Option<[u128; 2]> {
        let ast = self.ast();
        if ast.exprs.get(e).kind() != ExprKind::ConstantExpr {
            return None;
        }
        let img = ast.consts.aggregate(ast.exprs.constant_value(e)?)?;
        if ast.types.as_complex(img.ty).is_none() {
            return None;
        }
        let Some(elem) = elem else { return Some([0, 0]) };
        let es = usize::try_from(self.o.types.size(elem)).ok()?.min(16);
        let part = |i: usize| -> Option<u128> {
            let b = img.bytes.get(i * es..(i + 1) * es)?;
            let v = b.iter().rev().fold(0u128, |a, &x| (a << 8) | u128::from(x));
            Some(match int_of(self.kind(elem)) {
                Some((w, s)) => crate::arith::canon(v, w, s),
                None => v,
            })
        };
        Some([part(0)?, part(1)?])
    }

    /// The parts of the complex object at `p`, read with `akind`.
    fn cx_load(&mut self, p: u32, elem: OTy, akind: u32, e: ExprId) -> [u32; 2] {
        let es = self.o.types.size(elem);
        let loc = self.begin(e);
        let re = self.reg();
        self.emit_site(Op::Load, akind, &[re, p, elem.0], e, loc);
        let q = self.elem_ptr(p, es, e);
        let im = self.reg();
        self.emit_site(Op::Load, akind, &[im, q, elem.0], e, loc);
        [re, im]
    }

    /// The parts of complex expression `e`, each in its own register.
    pub(super) fn cx_regs(&mut self, e: ExprId) -> [u32; 2] {
        let e = skip_parens(self.ast(), e);
        let ty = self.ety(e);
        let Some(elem) = self.cx_elem(ty) else {
            self.unsupported(e);
            return [self.reg(), self.reg()];
        };
        let node = self.node(e);
        if node.kind() == ExprKind::SubstNonTypeTemplateParmExpr {
            let r = self.ast().exprs.subst_replacement(e);
            return self.cx_regs(r);
        }
        if let Some([re, im]) = self.cx_const(e, Some(elem)) {
            let r = [self.reg(), self.reg()];
            self.konst(r[0], re);
            self.konst(r[1], im);
            return r;
        }
        if node.kind() == ExprKind::ConstantExpr {
            let op = self.ast().exprs.operand(e);
            return self.cx_regs(op);
        }
        if node.kind().is_cast() {
            let op = self.ast().exprs.operand(e);
            match self.ast().exprs.cast_kind(e) {
                CastKind::LValueToRValue => {
                    let p = self.lv_mem(op, Access::Read);
                    return self.cx_load(p, elem, ak::READ, op);
                }
                CastKind::NoOp if !node.value_category().is_glvalue() => {
                    return self.cx_regs(op);
                }
                _ => {}
            }
        }
        if !self.cx_op(e) {
            let p = if node.value_category().is_glvalue() {
                self.lv_mem(e, Access::Read)
            } else {
                self.materialize(e, ty)
            };
            return self.cx_load(p, elem, ak::READ, e);
        }
        match node.kind() {
            ExprKind::UnaryOperator => self.cx_unary(e, elem),
            ExprKind::BinaryOperator => self.cx_binary(e, elem),
            ExprKind::CallExpr => {
                let Some([a, b]) = self.builtin_complex_args(e) else {
                    self.unsupported(e);
                    return [self.reg(), self.reg()];
                };
                [self.rv_copy(a), self.rv_copy(b)]
            }
            ExprKind::ImaginaryLiteral => {
                // GNU `2.0i`: a `+0` real part (the zero bit pattern).
                let re = self.reg();
                self.konst(re, 0);
                let sub = self.ast().exprs.operand(e);
                [re, self.rv_copy(sub)]
            }
            _ => self.cx_cast(e, elem),
        }
    }

    /// The parts of operand `x` of a complex operation, and whether it is
    /// real (its imaginary part a `+0` constant).
    fn cx_operand(&mut self, x: ExprId) -> ([u32; 2], bool) {
        if self.is_complex_expr(x) {
            return (self.cx_regs(x), false);
        }
        let re = self.rv_copy(x);
        let im = self.reg();
        self.konst(im, 0);
        ([re, im], true)
    }

    fn cx_cast(&mut self, e: ExprId, elem: OTy) -> [u32; 2] {
        let x = self.ast().exprs.operand(e);
        let xt = self.ety(x);
        match self.ast().exprs.cast_kind(e) {
            CastKind::IntegralRealToComplex | CastKind::FloatingRealToComplex => {
                let mut re = self.rv_copy(x);
                if self.kind(xt) != self.kind(elem) {
                    let d = self.reg();
                    self.convert(re, xt, elem, d, e);
                    re = d;
                }
                let im = self.reg();
                self.konst(im, 0);
                [re, im]
            }
            _ => {
                let Some(xe) = self.cx_elem(xt) else {
                    self.unsupported(e);
                    return [self.reg(), self.reg()];
                };
                let src = self.cx_regs(x);
                if self.kind(xe) == self.kind(elem) {
                    return src;
                }
                // clang reports an out-of-range part at the operand.
                let at = self.sites_len();
                let out = src.map(|r| {
                    let d = self.reg();
                    self.convert(r, xe, elem, d, e);
                    d
                });
                let begin = self.begin(x);
                self.o.sites[at..].iter_mut().for_each(|s| s.loc = begin);
                out
            }
        }
    }

    /// `-x` (both parts), `~x` (the conjugate), `+x`, `__extension__ x`.
    fn cx_unary(&mut self, e: ExprId, elem: OTy) -> [u32; 2] {
        let op = self.ast().exprs.unary_op(e);
        let x = self.ast().exprs.operand(e);
        if matches!(op, UnaryOp::PostInc | UnaryOp::PostDec) {
            let place = self.lv_rmw(x, e);
            return self.cx_inc_dec(place, elem, op, e);
        }
        let [re, im] = self.cx_regs(x);
        match op {
            UnaryOp::Minus => [self.cx_neg(re, elem, e), self.cx_neg(im, elem, e)],
            UnaryOp::Not => [re, self.cx_neg(im, elem, e)],
            _ => [re, im],
        }
    }

    /// `++`/`--` of the complex object at `place`: clang's
    /// `IncDecSubobjectHandler` steps the real part only. Returns the old
    /// parts.
    pub(super) fn cx_inc_dec(
        &mut self,
        place: Place,
        elem: OTy,
        op: UnaryOp,
        e: ExprId,
    ) -> [u32; 2] {
        let Place::Mem(p) = place else {
            self.unsupported(e);
            return [self.reg(), self.reg()];
        };
        let inc = matches!(op, UnaryOp::PostInc | UnaryOp::PreInc);
        let akind = if inc { ak::INCREMENT } else { ak::DECREMENT };
        let old = self.cx_load(p, elem, akind, e);
        let new = self.reg();
        let one = self.reg();
        match self.kind(elem) {
            TyKind::Float(k) => {
                self.konst(one, crate::arith::int_to_float(k, 1, true));
                let fop = if inc { Op::FAdd } else { Op::FSub };
                self.emit_at(fop, float_code(k), &[new, old[0], one], e);
            }
            k => {
                let (w, s) = int_of(k).unwrap_or((32, true));
                self.konst(one, 1);
                let iop = if inc { Op::IAdd } else { Op::ISub };
                self.emit_at(iop, int_aux(w as u8, s), &[new, old[0], one], e);
            }
        }
        self.store(Place::Mem(p), new, elem, akind, e);
        old
    }

    /// The negation of part `r` of element type `elem`: an integer wraps
    /// (`APSInt` negation).
    fn cx_neg(&mut self, r: u32, elem: OTy, e: ExprId) -> u32 {
        let d = self.reg();
        match self.kind(elem) {
            TyKind::Float(k) => {
                self.emit(Op::FNeg, float_code(k), &[d, r]);
            }
            k => {
                let (w, s) = int_of(k).unwrap_or((32, true));
                let u = self.reg();
                self.emit_at(Op::INeg, int_aux(w as u8, false), &[u, r], e);
                self.emit(Op::ICast, w | w << 9 | u32::from(s) << 17, &[d, u]);
            }
        }
        d
    }

    fn cx_binary(&mut self, e: ExprId, elem: OTy) -> [u32; 2] {
        let op = self.ast().exprs.binary_op(e);
        let [a, b] = self.ast().exprs.operands(e);
        let l = self.cx_operand(a);
        let r = self.cx_operand(b);
        let sub = match op {
            BinaryOp::Add => cx::ADD,
            BinaryOp::Sub => cx::SUB,
            BinaryOp::Mul => cx::MUL,
            _ => cx::DIV,
        };
        self.cx_arith(e, elem, sub, l, r)
    }

    /// Operation `sub` (`cx`) on parts `l` and `r` of element type `elem`,
    /// each with whether it is a real operand (`cx_operand`).
    fn cx_arith(
        &mut self,
        e: ExprId,
        elem: OTy,
        sub: u32,
        ([ra, ia], lreal): ([u32; 2], bool),
        ([rc, id], rreal): ([u32; 2], bool),
    ) -> [u32; 2] {
        if self.kind(elem) == TyKind::Float(FloatKind::BFloat16) {
            // `_Complex __bf16` as GCC computes it: in `_Complex float`
            // (`__mulsc3`/`__divsc3` at run time), each part rounded to
            // bf16 once per operation (spec/oven.md "Complex values").
            let f = self.ty(QualType::FLOAT);
            let up = |c: &mut Self, r: u32| c.cx_convert(r, elem, f, e);
            let l = ([up(self, ra), up(self, ia)], lreal);
            let r = ([up(self, rc), up(self, id)], rreal);
            let out = self.cx_arith(e, f, sub, l, r);
            return out.map(|r| self.cx_convert(r, f, elem, e));
        }
        let (dr, di) = (self.reg(), self.reg());
        let aux = match self.kind(elem) {
            TyKind::Float(k) => {
                let fc = float_code(k);
                match (sub, lreal, rreal) {
                    // (a)(c + id), (a + ib)(c): `handleFloatFloatBinOp`.
                    (cx::MUL, true, _) => {
                        self.emit_at(Op::FMul, fc, &[dr, ra, rc], e);
                        self.emit_at(Op::FMul, fc, &[di, ra, id], e);
                        return [dr, di];
                    }
                    (cx::MUL, _, true) => {
                        self.emit_at(Op::FMul, fc, &[dr, rc, ra], e);
                        self.emit_at(Op::FMul, fc, &[di, rc, ia], e);
                        return [dr, di];
                    }
                    (cx::DIV, _, true) => {
                        self.emit_at(Op::FDiv, fc, &[dr, ra, rc], e);
                        self.emit_at(Op::FDiv, fc, &[di, ia, rc], e);
                        return [dr, di];
                    }
                    _ => {}
                }
                sub | fc << 2
            }
            k => {
                let (w, s) = int_of(k).unwrap_or((32, true));
                sub | 1 << 6 | w << 7 | u32::from(s) << 15
            }
        };
        self.emit_at(Op::Cx, aux, &[dr, di, ra, ia, rc, id], e);
        // A real operand contributes no imaginary part: `b` or `±d`
        // unchanged (a `-0` stays `-0`).
        if matches!(sub, cx::ADD | cx::SUB) && (lreal || rreal) {
            if rreal {
                self.emit(Op::Mov, 0, &[di, ia]);
            } else if sub == cx::ADD {
                self.emit(Op::Mov, 0, &[di, id]);
            } else {
                let d = self.cx_neg(id, elem, e);
                self.emit(Op::Mov, 0, &[di, d]);
            }
        }
        [dr, di]
    }

    /// Whether builtin binary operator `e` has a complex operand.
    pub(super) fn cx_operands(&mut self, e: ExprId) -> bool {
        let [a, b] = self.ast().exprs.operands(e);
        self.is_complex_expr(a) || self.is_complex_expr(b)
    }

    /// `a == b` / `a != b` with a complex operand, into `dst`: equal when
    /// both parts compare equal (a NaN part compares unequal).
    pub(super) fn cx_compare(&mut self, e: ExprId, dst: u32) {
        let op = self.ast().exprs.binary_op(e);
        let [a, b] = self.ast().exprs.operands(e);
        let elem = {
            let t = if self.is_complex_expr(a) { self.ety(a) } else { self.ety(b) };
            self.cx_elem(t).unwrap_or(t)
        };
        let Some(c) = cmp_of(op).filter(|_| matches!(op, BinaryOp::EQ | BinaryOp::NE)) else {
            return self.unsupported(e);
        };
        let ([ra, ia], _) = self.cx_operand(a);
        let ([rb, ib], _) = self.cx_operand(b);
        let k = self.kind(elem);
        let eq = cmp_of(BinaryOp::EQ).unwrap_or(c);
        let (er, ei) = (self.reg(), self.reg());
        self.compare(e, (k, k), eq, er, ra, rb);
        self.compare(e, (k, k), eq, ei, ia, ib);
        if op == BinaryOp::EQ {
            self.emit(Op::IAnd, int_aux(8, false), &[dst, er, ei]);
        } else {
            let t = self.reg();
            self.emit(Op::IAnd, int_aux(8, false), &[t, er, ei]);
            self.emit(Op::LNot, 0, &[dst, t]);
        }
    }

    /// A complex-to-scalar cast `e` into `dst`: the real part, or whether
    /// either part is nonzero.
    pub(super) fn cx_to_scalar(&mut self, e: ExprId, dst: u32) {
        let x = self.ast().exprs.operand(e);
        let xt = self.ety(x);
        let elem = self.cx_elem(xt).unwrap_or(xt);
        let [re, im] = self.cx_regs(x);
        match self.ast().exprs.cast_kind(e) {
            CastKind::IntegralComplexToReal | CastKind::FloatingComplexToReal => {
                self.emit(Op::Mov, 0, &[dst, re]);
            }
            _ => {
                let (tr, ti) = (self.reg(), self.reg());
                match self.kind(elem) {
                    TyKind::Float(k) => {
                        self.emit(Op::FToBool, float_code(k), &[tr, re]);
                        self.emit(Op::FToBool, float_code(k), &[ti, im]);
                    }
                    k => {
                        let (w, _) = int_of(k).unwrap_or((32, true));
                        self.emit(Op::ToBool, w, &[tr, re]);
                        self.emit(Op::ToBool, w, &[ti, im]);
                    }
                }
                self.emit(Op::IOr, int_aux(8, false), &[dst, tr, ti]);
            }
        }
    }

    /// `__real x` / `__imag x` as a prvalue into `dst`. Of a real `x`:
    /// `x` itself, or zero after evaluating `x`.
    pub(super) fn real_imag(&mut self, e: ExprId, dst: u32) {
        let op = self.ast().exprs.unary_op(e);
        let x = self.ast().exprs.operand(e);
        if self.is_complex_expr(x) {
            let [re, im] = self.cx_regs(x);
            let r = if op == UnaryOp::Real { re } else { im };
            self.emit(Op::Mov, 0, &[dst, r]);
        } else if op == UnaryOp::Real {
            self.rv_inner(x, dst);
        } else {
            self.effect(x);
            self.konst(dst, 0);
        }
    }

    /// `__real x` / `__imag x` as an lvalue: the part of complex object
    /// `x`, or the real object `x` itself.
    pub(super) fn real_imag_lv(&mut self, e: ExprId, acc: Access) -> Place {
        let op = self.ast().exprs.unary_op(e);
        let x = self.ast().exprs.operand(e);
        let xt = self.ety(x);
        let Some(elem) = self.cx_elem(xt) else { return self.lv(x, acc) };
        let p = self.lv_mem(x, acc);
        if op == UnaryOp::Real {
            return Place::Mem(p);
        }
        let es = self.o.types.size(elem);
        Place::Mem(self.elem_ptr(p, es, e))
    }

    /// `a = b` of complex type: the right operand first, then both parts
    /// stored.
    pub(super) fn cx_assign(&mut self, e: ExprId, elem: OTy) -> Place {
        let [a, b] = self.ast().exprs.operands(e);
        let [re, im] = self.cx_regs(b);
        let saved = self.rmw_loc.replace(self.node(e).loc());
        let p = self.lv_mem(a, Access::Write);
        self.rmw_loc = saved;
        self.store(Place::Mem(p), re, elem, ak::ASSIGN, e);
        let es = self.o.types.size(elem);
        let q = self.elem_ptr(p, es, e);
        self.store(Place::Mem(q), im, elem, ak::ASSIGN, e);
        Place::Mem(p)
    }

    /// Compound assignment `e` with a complex operand, as GCC folds it
    /// (spec/oven.md "Complex values"; clang 19 and 22 fail with the
    /// generic note): the right operand first (C++17 [expr.assign]/1),
    /// then `lhs = lhs op rhs` computed in `ComputeResultTy` like the
    /// binary operator and converted back to the left operand's type (its
    /// real part when the left operand is real).
    pub(super) fn cx_compound_assign(&mut self, e: ExprId) -> Place {
        let x = *self.ast().exprs.compound_assign(e);
        let lhs = ExprId::new(self.node(e).a());
        let lty = self.ety(lhs);
        let ct = self.ty(x.result_ty);
        let sub = match self.ast().exprs.binary_op(e) {
            BinaryOp::AddAssign => Some(cx::ADD),
            BinaryOp::SubAssign => Some(cx::SUB),
            BinaryOp::MulAssign => Some(cx::MUL),
            BinaryOp::DivAssign => Some(cx::DIV),
            _ => None,
        };
        let (Some(sub), Some(celem)) = (sub, self.cx_elem(ct)) else {
            let place = self.lv_rmw(lhs, e);
            self.trap(e, INVALID);
            return place;
        };
        let rhs = self.cx_operand(x.rhs);
        let rty = self.ety(x.rhs);
        let rhs = self.cx_coerce(rhs, rty, celem, e);
        let place = self.lv_rmw(lhs, e);
        let lelem = self.cx_elem(lty);
        let old = match (lelem, place) {
            (Some(le), Place::Mem(p)) => (self.cx_load(p, le, ak::COMPOUND, e), false),
            (Some(_), _) => {
                self.unsupported(e);
                return place;
            }
            (None, _) => {
                let re = self.reg();
                self.load(place, lty, ak::COMPOUND, e, re);
                let im = self.reg();
                self.konst(im, 0);
                ([re, im], true)
            }
        };
        let old = self.cx_coerce(old, lty, celem, e);
        let [re, im] = self.cx_arith(e, celem, sub, old, rhs);
        match (lelem, place) {
            (Some(le), Place::Mem(p)) => {
                let parts = [re, im].map(|r| self.cx_convert(r, celem, le, e));
                self.store(Place::Mem(p), parts[0], le, ak::COMPOUND, e);
                let es = self.o.types.size(le);
                let q = self.elem_ptr(p, es, e);
                self.store(Place::Mem(q), parts[1], le, ak::COMPOUND, e);
            }
            _ => {
                let v = self.cx_convert(re, celem, lty, e);
                self.store(place, v, lty, ak::COMPOUND, e);
            }
        }
        place
    }

    /// Operand parts `x` of type `xt` (complex, or real when `x.1`)
    /// converted to element type `elem`.
    fn cx_coerce(
        &mut self,
        x: ([u32; 2], bool),
        xt: OTy,
        elem: OTy,
        e: ExprId,
    ) -> ([u32; 2], bool) {
        let ([re, im], real) = x;
        let from = self.cx_elem(xt).unwrap_or(xt);
        let re = self.cx_convert(re, from, elem, e);
        let im = if real { im } else { self.cx_convert(im, from, elem, e) };
        ([re, im], real)
    }

    /// Register `r` of type `from` converted to `to` (itself when the
    /// kinds agree).
    fn cx_convert(&mut self, r: u32, from: OTy, to: OTy, e: ExprId) -> u32 {
        if self.kind(from) == self.kind(to) {
            return r;
        }
        let d = self.reg();
        self.convert(r, from, to, d, e);
        d
    }
}
