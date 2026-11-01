//! Constant evaluation of GNU/clang vector operations (WP-173;
//! spec/oven.md "Vectors").
//!
//! A vector object is stored as an array of its lanes (`ty.rs`). The
//! operations clang 19's `VectorExprEvaluator` folds are computed lane by
//! lane in registers: each operand vector is evaluated whole (its lanes
//! loaded into registers), then the lanes are combined in order, so the
//! first faulting lane gives the note, as in clang. The result is stored
//! into the destination object (`vec_init`) or kept in registers for an
//! enclosing operation (`vec_regs`); only a vector produced by something
//! else (a call, a list, a compound literal) is materialized.
//!
//! What clang 19 does not fold is rejected as it rejects it: a lane
//! read or write `v[i]` and `__builtin_reduce_*` with the generic note,
//! a vector-condition `?:` and an ext vector element access silently,
//! a bit cast of a vector to a scalar with the generic note.

use kiln_ast::ExprId;
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, UnaryOp};

use super::{Access, Compiler, Place, cmp_of, float_code, int_of, int_op, skip_parens};
use crate::kbc::{Op, ak, bi, int_aux};
use crate::mem::Origin;
use crate::ty::{OTy, TyKind};

impl Compiler<'_> {
    /// The lane type and count of vector type `t`.
    pub(super) fn lanes(&self, t: OTy) -> Option<(OTy, u64)> {
        if !self.o.types.is_vector(self.b.ast(), t) {
            return None;
        }
        match self.kind(t) {
            TyKind::Array { elem, n } => Some((elem, n)),
            _ => None,
        }
    }

    /// Whether expression `e` has vector type.
    pub(super) fn is_vector_expr(&mut self, e: ExprId) -> bool {
        let t = self.ety(e);
        self.lanes(t).is_some()
    }

    /// Whether vector prvalue `e` is an operation computed lane-wise here
    /// (rather than by the generic aggregate initialization).
    pub(super) fn vec_op(&self, e: ExprId) -> bool {
        let ast = self.ast();
        let n = ast.exprs.get(e);
        if n.value_category().is_glvalue() {
            return false;
        }
        match n.kind() {
            ExprKind::UnaryOperator => matches!(
                ast.exprs.unary_op(e),
                UnaryOp::Plus | UnaryOp::Minus | UnaryOp::Not | UnaryOp::LNot
            ),
            ExprKind::BinaryOperator => ast.exprs.binary_op(e) != BinaryOp::Cmp,
            ExprKind::ConvertVectorExpr
            | ExprKind::ConditionalOperator
            | ExprKind::ShuffleVectorExpr => true,
            k if k.is_cast() => {
                matches!(ast.exprs.cast_kind(e), CastKind::VectorSplat | CastKind::BitCast)
            }
            _ => false,
        }
    }

    /// Initializes the vector object of type `ty` at `p` from operation
    /// `e` (`vec_op`).
    pub(super) fn vec_init(&mut self, e: ExprId, p: u32, ty: OTy) {
        let Some((elem, _)) = self.lanes(ty) else { return self.unsupported(e) };
        let n = self.node(e);
        if n.kind().is_cast() && self.ast().exprs.cast_kind(e) == CastKind::BitCast {
            let r = self.reg();
            return self.vec_bitcast(e, r, Some(p));
        }
        let regs = self.vec_regs(e);
        self.store_lanes(p, elem, &regs, e);
    }

    /// The address of vector `x`: its object for a glvalue, else a
    /// temporary.
    fn vec_addr(&mut self, x: ExprId) -> u32 {
        if self.node(x).value_category().is_glvalue() {
            self.lv_mem(x, Access::Read)
        } else {
            let t = self.ety(x);
            self.materialize(x, t)
        }
    }

    /// The lanes of the vector at `p`, read with access kind `akind`.
    fn load_lanes(&mut self, p: u32, elem: OTy, n: u64, akind: u32, e: ExprId) -> Vec<u32> {
        let es = self.o.types.size(elem);
        let loc = self.begin(e);
        (0..n)
            .map(|i| {
                let q = self.elem_ptr(p, i * es, e);
                let r = self.reg();
                self.emit_site(Op::Load, akind, &[r, q, elem.0], e, loc);
                r
            })
            .collect()
    }

    /// Initializes the lanes of the vector at `p` from `regs`.
    fn store_lanes(&mut self, p: u32, elem: OTy, regs: &[u32], e: ExprId) {
        let es = self.o.types.size(elem);
        for (i, &r) in regs.iter().enumerate() {
            let q = self.elem_ptr(p, i as u64 * es, e);
            self.emit_at(Op::InitStore, 0, &[q, r, elem.0], e);
        }
    }

    /// The lanes of vector prvalue (or glvalue) `e`, each in a register
    /// (a register may hold several lanes: callers only read them).
    pub(super) fn vec_regs(&mut self, e: ExprId) -> Vec<u32> {
        let e = skip_parens(self.ast(), e);
        let ty = self.ety(e);
        let Some((elem, n)) = self.lanes(ty) else {
            self.unsupported(e);
            return Vec::new();
        };
        let node = self.node(e);
        if node.kind().is_cast() {
            let op = self.ast().exprs.operand(e);
            match self.ast().exprs.cast_kind(e) {
                CastKind::LValueToRValue => {
                    let p = self.lv_mem(op, Access::Read);
                    return self.load_lanes(p, elem, n, ak::READ, op);
                }
                CastKind::NoOp if !node.value_category().is_glvalue() => {
                    return self.vec_regs(op);
                }
                _ => {}
            }
        }
        if !self.vec_op(e) {
            let p = self.vec_addr(e);
            return self.load_lanes(p, elem, n, ak::READ, e);
        }
        match node.kind() {
            ExprKind::UnaryOperator => self.vec_unary(e, elem),
            ExprKind::BinaryOperator => self.vec_binary(e, elem, n),
            ExprKind::ConvertVectorExpr => {
                let x = self.ast().exprs.operand(e);
                let xt = self.ety(x);
                let Some((xe, _)) = self.lanes(xt) else {
                    self.unsupported(e);
                    return Vec::new();
                };
                let src = self.vec_regs(x);
                src.into_iter()
                    .map(|r| {
                        let d = self.reg();
                        self.convert(r, xe, elem, d, e);
                        d
                    })
                    .collect()
            }
            ExprKind::ConditionalOperator => self.vec_cond(e, n),
            ExprKind::ShuffleVectorExpr => {
                self.unsupported(e);
                Vec::new()
            }
            _ => {
                let op = self.ast().exprs.operand(e);
                if self.ast().exprs.cast_kind(e) == CastKind::VectorSplat {
                    let xt = self.ety(op);
                    let r = self.rv_copy(op);
                    let r = if self.kind(xt) == self.kind(elem) {
                        r
                    } else {
                        let d = self.reg();
                        self.convert(r, xt, elem, d, e);
                        d
                    };
                    return vec![r; n as usize];
                }
                // A bit cast: through a temporary of the result type.
                let d = self.reg();
                self.alloc(d, ty, Origin::Temp(e), false, e);
                let r = self.reg();
                self.vec_bitcast(e, r, Some(d));
                self.load_lanes(d, elem, n, ak::READ, e)
            }
        }
    }

    /// The lanes of operand `x` of an operation on `n` lanes: a scalar
    /// operand gives every lane.
    fn vec_or_scalar(&mut self, x: ExprId, n: u64) -> (Vec<u32>, OTy) {
        let t = self.ety(x);
        match self.lanes(t) {
            Some((elem, _)) => (self.vec_regs(x), elem),
            None => {
                let r = self.rv_copy(x);
                (vec![r; n as usize], t)
            }
        }
    }

    /// The comparison result `c` (a `bool` register) as a lane of
    /// signed integer type `elem`: all ones when true.
    fn lane_mask(&mut self, c: u32, elem: OTy, e: ExprId) -> u32 {
        let (w, _) = int_of(self.kind(elem)).unwrap_or((32, true));
        let i = self.reg();
        self.emit(Op::ICast, 1 | w << 9 | 1 << 17, &[i, c]);
        let d = self.reg();
        self.emit_at(Op::INeg, int_aux(w as u8, true), &[d, i], e);
        d
    }

    /// Whether lane `r` of type `t` is nonzero, as a `bool` register.
    fn lane_truth(&mut self, r: u32, t: OTy) -> u32 {
        let d = self.reg();
        match self.kind(t) {
            TyKind::Float(k) => {
                self.emit(Op::FToBool, float_code(k), &[d, r]);
            }
            k => {
                let (w, _) = int_of(k).unwrap_or((32, true));
                self.emit(Op::ToBool, w, &[d, r]);
            }
        }
        d
    }

    fn vec_unary(&mut self, e: ExprId, elem: OTy) -> Vec<u32> {
        let op = self.ast().exprs.unary_op(e);
        let x = self.ast().exprs.operand(e);
        if op == UnaryOp::Plus {
            return self.vec_regs(x);
        }
        let xt = self.ety(x);
        let xe = self.lanes(xt).map_or(elem, |l| l.0);
        let src = self.vec_regs(x);
        src.into_iter()
            .map(|r| {
                let d = self.reg();
                match (op, self.kind(xe)) {
                    (UnaryOp::Minus, TyKind::Float(k)) => {
                        self.emit(Op::FNeg, float_code(k), &[d, r]);
                    }
                    (UnaryOp::Minus, k) => {
                        // clang 19 negates an integer lane with wrapping
                        // (`APSInt` negation, no overflow check).
                        let (w, s) = int_of(k).unwrap_or((32, true));
                        let u = self.reg();
                        self.emit_at(Op::INeg, int_aux(w as u8, false), &[u, r], e);
                        self.emit(Op::ICast, w | w << 9 | u32::from(s) << 17, &[d, u]);
                    }
                    (UnaryOp::Not, k) => {
                        let (w, s) = int_of(k).unwrap_or((32, true));
                        self.emit(Op::INot, int_aux(w as u8, s), &[d, r]);
                    }
                    _ => {
                        // `!`: the lane compared equal to zero.
                        let t = self.lane_truth(r, xe);
                        self.emit(Op::LNot, 0, &[d, t]);
                        return self.lane_mask(d, elem, e);
                    }
                }
                d
            })
            .collect()
    }

    fn vec_binary(&mut self, e: ExprId, elem: OTy, n: u64) -> Vec<u32> {
        let op = self.ast().exprs.binary_op(e);
        let [a, b] = self.ast().exprs.operands(e);
        if op == BinaryOp::Comma {
            self.effect(a);
            return self.vec_regs(b);
        }
        let (la, ae) = self.vec_or_scalar(a, n);
        let (lb, be) = self.vec_or_scalar(b, n);
        let kinds = (self.kind(ae), self.kind(be));
        let mut out = Vec::with_capacity(la.len());
        for (&ra, &rb) in la.iter().zip(&lb) {
            let d = self.reg();
            if let Some(c) = cmp_of(op) {
                self.compare(e, kinds, c, d, ra, rb);
                out.push(self.lane_mask(d, elem, e));
            } else if matches!(op, BinaryOp::LAnd | BinaryOp::LOr) {
                // Element-wise, without short-circuit.
                let ta = self.lane_truth(ra, ae);
                let tb = self.lane_truth(rb, be);
                // clang 19 gives a true lane the value 1 here (not -1 as
                // at run time, spec/oven.md "Vectors").
                let o = if op == BinaryOp::LAnd { Op::IAnd } else { Op::IOr };
                self.emit(o, int_aux(8, false), &[d, ta, tb]);
                let (w, _) = int_of(self.kind(elem)).unwrap_or((32, true));
                let r = self.reg();
                self.emit(Op::ICast, 8 | w << 9 | 1 << 17, &[r, d]);
                out.push(r);
            } else if let Some(iop) = int_op(op) {
                self.arith(iop, elem, be, d, ra, rb, e);
                out.push(d);
            } else {
                self.unsupported(e);
                return out;
            }
        }
        out
    }

    /// `c ? t : f` of vector type: the scalar-condition form selects an
    /// operand; clang 19 does not fold the element-wise form.
    fn vec_cond(&mut self, e: ExprId, n: u64) -> Vec<u32> {
        let c = ExprId::new(self.node(e).a());
        if self.is_vector_expr(c) {
            // clang 19 fails silently: no note, no call stack.
            self.trap(e, "");
            return Vec::new();
        }
        let rc = self.rv(c);
        let out: Vec<u32> = (0..n).map(|_| self.reg()).collect();
        let [t, f] = self.ast().exprs.cond_arms(e);
        let j = self.jmp_if(rc, false);
        let m = self.next;
        self.vec_select(t, &out);
        self.next = m;
        let end = self.jmp();
        self.patch_here(j);
        self.vec_select(f, &out);
        self.next = m;
        self.patch_here(end);
        out
    }

    fn vec_select(&mut self, x: ExprId, out: &[u32]) {
        let regs = self.vec_regs(x);
        for (&d, &r) in out.iter().zip(&regs) {
            self.emit(Op::Mov, 0, &[d, r]);
        }
    }

    /// A bit cast `e` (`CastKind::BitCast`) to or from a vector type:
    /// into the object `mem` points to when given, else as the scalar
    /// value in `dst`.
    pub(super) fn vec_bitcast(&mut self, e: ExprId, dst: u32, mem: Option<u32>) {
        let x = self.ast().exprs.operand(e);
        let (ts, td) = (self.ety(x), self.ety(e));
        let src = if self.node(x).value_category().is_glvalue() {
            self.lv_mem(x, Access::Read)
        } else if self.o.types.is_aggregate(ts) {
            self.materialize(x, ts)
        } else {
            let r = self.rv(x);
            let p = self.reg();
            self.alloc(p, ts, Origin::Temp(e), false, e);
            self.emit_at(Op::InitStore, 0, &[p, r, ts.0], e);
            p
        };
        let loc = self.begin(e);
        if let Some(m) =
            self.bit_cast_unsupported(ts, true).or_else(|| self.bit_cast_unsupported(td, false))
        {
            return self.trap_loc(loc, m);
        }
        let (b, sub) = mem.map_or((src, 0), |p| (p, bi::TO_MEM));
        self.emit_site(
            Op::Builtin,
            bi::BIT_CAST | sub << 8,
            &[dst, src, b, src, ts.0, td.0],
            e,
            loc,
        );
    }

    /// `a = b` of vector type: the right operand first, then a copy.
    pub(super) fn vec_assign(&mut self, e: ExprId, ty: OTy) -> Place {
        let [a, b] = self.ast().exprs.operands(e);
        let src = self.vec_addr(b);
        let saved = self.rmw_loc.replace(self.node(e).loc());
        let dstp = self.lv_mem(a, Access::Write);
        self.rmw_loc = saved;
        self.emit_at(Op::Copy, 0, &[dstp, src, ty.0], e);
        Place::Mem(dstp)
    }

    /// Compound assignment `e` of vector type, lane-wise; the new value
    /// is copied over the object.
    pub(super) fn vec_compound_assign(&mut self, e: ExprId, ty: OTy) -> Place {
        let ast = self.ast();
        let x = *ast.exprs.compound_assign(e);
        let op = ast.exprs.binary_op(e);
        let lhs = ExprId::new(self.node(e).a());
        let Some((elem, n)) = self.lanes(ty) else {
            self.unsupported(e);
            return Place::Mem(self.reg());
        };
        let Some(iop) = int_op(op) else {
            self.unsupported(e);
            return Place::Mem(self.reg());
        };
        // [expr.assign]/1: the right operand is sequenced first.
        let (rb, be) = self.vec_or_scalar(x.rhs, n);
        let Place::Mem(p) = self.lv_rmw(lhs, e) else {
            self.unsupported(e);
            return Place::Mem(self.reg());
        };
        let old = self.load_lanes(p, elem, n, ak::COMPOUND, e);
        let new: Vec<u32> = old
            .iter()
            .zip(&rb)
            .map(|(&ra, &rb)| {
                let d = self.reg();
                self.arith(iop, elem, be, d, ra, rb, e);
                d
            })
            .collect();
        let t = self.reg();
        self.alloc(t, ty, Origin::Temp(e), false, e);
        self.store_lanes(t, elem, &new, e);
        self.emit_at(Op::Copy, 0, &[p, t, ty.0], e);
        Place::Mem(p)
    }
}
