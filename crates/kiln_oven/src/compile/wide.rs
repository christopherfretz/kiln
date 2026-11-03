//! Constant evaluation of `_BitInt(N)` with `N > 128` (WP-445; spec/oven.md
//! "`_BitInt`").
//!
//! A wide `_BitInt` is a memory-class type (`TyKind::WideInt`): its
//! objects, parameters and results live in memory, as an aggregate's do,
//! and registers hold only addresses. An operation reads its operands'
//! objects and writes its result's (`Op::Wide`), so a prvalue operand is
//! evaluated into a temporary (`wide_place`) and a result into the object
//! it initializes (`wide_init`). Conversions to and from a narrow integer
//! go through a register (`wide::FROM`, `wide::TO`); a conversion to or
//! from a floating type is not supported yet (it traps).

use kiln_ast::ExprId;
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, UnaryOp};

use super::{Access, Compiler, Place, cmp_of, eval_int, int_of, skip_parens};
use crate::kbc::{Op, ak, wide};
use crate::mem::Origin;
use crate::ty::OTy;

const FLOAT: &str = "kiln: conversion between a _BitInt wider than 128 bits and a floating type \
                     is not supported in constant evaluation yet";

impl Compiler<'_> {
    /// Whether expression `e` has a wide `_BitInt` type.
    pub(super) fn is_wide_expr(&mut self, e: ExprId) -> bool {
        let t = self.ety(e);
        self.o.types.wide(t).is_some()
    }

    /// Whether an operand of binary operator `e` is a wide `_BitInt`.
    pub(super) fn wide_operands(&mut self, e: ExprId) -> bool {
        let [a, b] = self.ast().exprs.operands(e);
        self.is_wide_expr(a) || self.is_wide_expr(b)
    }

    fn wide(&mut self, sub: u32, k: u32, args: [u32; 3], ty: OTy, x: u32, e: ExprId) {
        let [d, a, b] = args;
        self.emit_at(Op::Wide, sub | k << 8, &[d, a, b, ty.0, x], e);
    }

    /// The address of the wide value of `e`: a glvalue's object, else a
    /// temporary holding the prvalue (an lvalue-to-rvalue conversion's
    /// read is a copy, so that its failure is noted at the operand, as
    /// clang notes it).
    fn wide_place(&mut self, e: ExprId) -> u32 {
        let e = skip_parens(self.ast(), e);
        let n = self.node(e);
        if n.value_category().is_glvalue() {
            return self.lv_mem(e, Access::Read);
        }
        let ty = self.ety(e);
        self.materialize(e, ty)
    }

    /// A fresh temporary of wide type `ty`.
    fn wide_temp(&mut self, ty: OTy, e: ExprId) -> u32 {
        let r = self.reg();
        self.alloc(r, ty, Origin::Temp(e), false, e);
        r
    }

    /// `x` of `wide::FROM`/`wide::TO` for narrow integer type `t`.
    fn narrow_code(&mut self, t: OTy) -> u32 {
        let (w, s) = int_of(self.kind(t)).unwrap_or((64, true));
        w | u32::from(s) << 8
    }

    /// The register value of a shift count `c` (`x`: its width | signed
    /// << 8): a wide count is clamped to 128 bits.
    fn shift_count(&mut self, c: ExprId) -> (u32, u32) {
        let ct = self.ety(c);
        if let Some((_, s)) = self.o.types.wide(ct) {
            let a = self.wide_place(c);
            let r = self.reg();
            let x = 128 | u32::from(s) << 8;
            self.wide(wide::TO, 0, [r, a, 0], ct, x | 1 << 9, c);
            return (r, x);
        }
        let r = self.rv_copy(c);
        (r, self.narrow_code(ct))
    }

    /// Initializes the wide object of type `ty` at `p` from `e`, if `e` is
    /// an operation computed here (else the generic aggregate forms
    /// apply: a call, `?:`, a comma, a glvalue's copy).
    pub(super) fn wide_init(&mut self, e: ExprId, p: u32, ty: OTy) -> bool {
        let n = self.node(e);
        let init = wide::INIT;
        match n.kind() {
            ExprKind::IntegerLiteral => {
                let v = eval_int(self.ast(), e).unwrap_or(0);
                let r = self.reg();
                self.konst(r, v);
                self.wide(wide::FROM | init, 0, [p, r, 0], ty, 128, e);
            }
            ExprKind::CXXScalarValueInitExpr => {
                self.emit_at(Op::ZeroInit, 0, &[p, ty.0], e);
            }
            ExprKind::InitListExpr => {
                let inits = self.ast().exprs.init_list_inits(e);
                match self.ast().lists.get(inits)[..] {
                    [] => {
                        self.emit_at(Op::ZeroInit, 0, &[p, ty.0], e);
                    }
                    [x] => self.init_inner(ExprId::new(x), p, ty),
                    _ => return false,
                }
            }
            k if k.is_cast() => {
                let op = self.ast().exprs.operand(e);
                let ot = self.ety(op);
                match self.ast().exprs.cast_kind(e) {
                    CastKind::LValueToRValue => {
                        let a = self.lv_mem(op, Access::Read);
                        self.wide(wide::MOV | init, 0, [p, a, 0], ty, 0, e);
                    }
                    CastKind::NoOp if !n.value_category().is_glvalue() => {
                        self.init_inner(op, p, ty);
                    }
                    CastKind::IntegralCast | CastKind::BooleanToSignedIntegral
                        if self.o.types.wide(ot).is_some() =>
                    {
                        let a = self.wide_place(op);
                        self.wide(wide::CAST | init, 0, [p, a, 0], ty, ot.0, e);
                    }
                    CastKind::IntegralCast => {
                        let r = self.rv(op);
                        let x = self.narrow_code(ot);
                        self.wide(wide::FROM | init, 0, [p, r, 0], ty, x, e);
                    }
                    CastKind::FloatingToIntegral => self.trap(e, FLOAT),
                    _ => return false,
                }
            }
            ExprKind::UnaryOperator => {
                let op = self.ast().exprs.unary_op(e);
                let x = self.ast().exprs.operand(e);
                match op {
                    UnaryOp::Plus | UnaryOp::Extension => self.init_inner(x, p, ty),
                    UnaryOp::Minus | UnaryOp::Not => {
                        let a = self.wide_place(x);
                        let sub = if op == UnaryOp::Minus { wide::NEG } else { wide::NOT };
                        self.wide(sub | init, 0, [p, a, 0], ty, 0, e);
                    }
                    UnaryOp::PostInc | UnaryOp::PostDec => {
                        let Place::Mem(q) = self.lv_rmw(x, e) else { return false };
                        let inc = op == UnaryOp::PostInc;
                        let k = if inc { ak::INCREMENT } else { ak::DECREMENT };
                        self.wide(wide::MOV | init, 0, [p, q, 0], ty, 0, e);
                        let sub = if inc { wide::INC } else { wide::DEC };
                        self.wide(sub | wide::RMW, k, [q, q, 0], ty, 0, e);
                    }
                    _ => return false,
                }
            }
            ExprKind::BinaryOperator => {
                let op = self.ast().exprs.binary_op(e);
                let Some(sub) = wide_sub(op) else { return false };
                let [l, r] = self.ast().exprs.operands(e);
                let a = self.wide_place(l);
                if matches!(sub, wide::SHL | wide::SHR) {
                    let (c, x) = self.shift_count(r);
                    self.wide(sub | init, 0, [p, a, c], ty, x, e);
                } else {
                    let b = self.wide_place(r);
                    self.wide(sub | init, 0, [p, a, b], ty, 0, e);
                }
            }
            _ => return false,
        }
        true
    }

    /// `a = b` of wide type `ty`.
    pub(super) fn wide_assign(&mut self, e: ExprId, ty: OTy) -> Place {
        let [a, b] = self.ast().exprs.operands(e);
        // [expr.assign]/1: the right operand is sequenced first.
        let src = self.wide_place(b);
        let saved = self.rmw_loc.replace(self.node(e).loc());
        let dst = self.lv_mem(a, Access::Write);
        self.rmw_loc = saved;
        self.wide(wide::MOV, ak::ASSIGN, [dst, src, 0], ty, 0, e);
        Place::Mem(dst)
    }

    /// `a op= b` with a wide `_BitInt` operand or computation type.
    pub(super) fn wide_compound_assign(&mut self, e: ExprId) -> Place {
        let ast = self.ast();
        let x = *ast.exprs.compound_assign(e);
        let op = ast.exprs.binary_op(e);
        let lhs = ExprId::new(ast.exprs.get(e).a());
        let lty = self.ety(lhs);
        let ct = self.ty(x.result_ty);
        let Some(sub) = wide_sub(op) else {
            self.unsupported(e);
            return Place::Mem(self.reg());
        };
        let shift = matches!(sub, wide::SHL | wide::SHR);
        if self.o.types.wide(ct).is_none() && !shift {
            self.trap(e, FLOAT);
            return Place::Mem(self.reg());
        }
        let k = ak::COMPOUND;
        // The right operand, converted to the computation type.
        let (b, bx) = if shift {
            self.shift_count(x.rhs)
        } else {
            let rt = self.ety(x.rhs);
            if rt == ct {
                (self.wide_place(x.rhs), 0)
            } else if self.o.types.wide(rt).is_some() {
                let src = self.wide_place(x.rhs);
                let t = self.wide_temp(ct, e);
                self.wide(wide::CAST | wide::INIT, 0, [t, src, 0], ct, rt.0, e);
                (t, 0)
            } else if int_of(self.kind(rt)).is_some() {
                let r = self.rv(x.rhs);
                let rx = self.narrow_code(rt);
                let t = self.wide_temp(ct, e);
                self.wide(wide::FROM | wide::INIT, 0, [t, r, 0], ct, rx, e);
                (t, 0)
            } else {
                self.trap(e, FLOAT);
                return Place::Mem(self.reg());
            }
        };
        let place = self.lv_rmw(lhs, e);
        let ct = if shift { lty } else { ct };
        match place {
            Place::Mem(q) if lty == ct => {
                self.wide(sub | wide::RMW, k, [q, q, b], ct, bx, e);
            }
            Place::Mem(q) if self.o.types.wide(lty).is_some() => {
                // A wider computation type: there and back.
                let t = self.wide_temp(ct, e);
                self.wide(wide::CAST | wide::INIT, 0, [t, q, 0], ct, lty.0, e);
                self.wide(sub, 0, [t, t, b], ct, bx, e);
                self.wide(wide::CAST, k, [q, t, 0], lty, ct.0, e);
            }
            _ if int_of(self.kind(lty)).is_some() => {
                // A narrow left operand: computed wide, converted back.
                let old = self.reg();
                self.load(place, lty, k, e, old);
                let lx = self.narrow_code(lty);
                let t = self.wide_temp(ct, e);
                self.wide(wide::FROM | wide::INIT, 0, [t, old, 0], ct, lx, e);
                self.wide(sub, 0, [t, t, b], ct, bx, e);
                let new = self.reg();
                self.wide(wide::TO, 0, [new, t, 0], ct, lx, e);
                self.store(place, new, lty, k, e);
            }
            _ => self.unsupported(e),
        }
        place
    }

    /// `++`/`--` of the wide object at `place`.
    pub(super) fn wide_inc_dec(&mut self, place: Place, ty: OTy, op: UnaryOp, e: ExprId) {
        let Place::Mem(q) = place else { return self.unsupported(e) };
        let inc = matches!(op, UnaryOp::PostInc | UnaryOp::PreInc);
        let k = if inc { ak::INCREMENT } else { ak::DECREMENT };
        let sub = if inc { wide::INC } else { wide::DEC };
        self.wide(sub | wide::RMW, k, [q, q, 0], ty, 0, e);
    }

    /// The comparison `e` of wide operands into register `dst`.
    pub(super) fn wide_compare(&mut self, e: ExprId, dst: u32) {
        let op = self.ast().exprs.binary_op(e);
        let [l, r] = self.ast().exprs.operands(e);
        let lt = self.ety(l);
        let (Some(c), true) = (cmp_of(op), self.ety(r) == lt) else {
            return self.unsupported(e);
        };
        let a = self.wide_place(l);
        let b = self.wide_place(r);
        self.wide(wide::CMP, 0, [dst, a, b], lt, c, e);
    }

    /// Cast `e` of a wide operand to scalar type `ty`, into `dst`.
    pub(super) fn wide_to_scalar(&mut self, e: ExprId, dst: u32, ty: OTy) {
        let op = self.ast().exprs.operand(e);
        let ot = self.ety(op);
        match self.ast().exprs.cast_kind(e) {
            CastKind::IntegralToBoolean => {
                let a = self.wide_place(op);
                self.wide(wide::TO_BOOL, 0, [dst, a, 0], ot, 0, e);
            }
            CastKind::IntegralCast if int_of(self.kind(ty)).is_some() => {
                let a = self.wide_place(op);
                let x = self.narrow_code(ty);
                self.wide(wide::TO, 0, [dst, a, 0], ot, x, e);
            }
            CastKind::IntegralToFloating => self.trap(e, FLOAT),
            _ => self.unsupported(e),
        }
    }
}

/// The `wide` sub-op of arithmetic, shift or bitwise operator `op` (or
/// its compound assignment).
fn wide_sub(op: BinaryOp) -> Option<u32> {
    Some(match op {
        BinaryOp::Add | BinaryOp::AddAssign => wide::ADD,
        BinaryOp::Sub | BinaryOp::SubAssign => wide::SUB,
        BinaryOp::Mul | BinaryOp::MulAssign => wide::MUL,
        BinaryOp::Div | BinaryOp::DivAssign => wide::DIV,
        BinaryOp::Rem | BinaryOp::RemAssign => wide::REM,
        BinaryOp::Shl | BinaryOp::ShlAssign => wide::SHL,
        BinaryOp::Shr | BinaryOp::ShrAssign => wide::SHR,
        BinaryOp::And | BinaryOp::AndAssign => wide::AND,
        BinaryOp::Or | BinaryOp::OrAssign => wide::OR,
        BinaryOp::Xor | BinaryOp::XorAssign => wide::XOR,
        _ => return None,
    })
}
