//! Constant evaluation of `_BitInt(N)` with `N > 128` (WP-445; spec/oven.md
//! "`_BitInt`").
//!
//! A wide `_BitInt` is a memory-class type (`TyKind::WideInt`): its
//! objects, parameters and results live in memory, as an aggregate's do,
//! and registers hold only addresses. An operation reads its operands'
//! objects and writes its result's (`Op::Wide`), so a prvalue operand is
//! evaluated into a temporary (`wide_place`) and a result into the object
//! it initializes (`wide_init`). Conversions to and from a narrow integer
//! go through a register (`wide::FROM`, `wide::TO`), as do those to and
//! from a floating type (`wide::TO_FLOAT`, `wide::FROM_FLOAT`).

use kiln_ast::ExprId;
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, UnaryOp};

use kiln_source::Loc;

use super::{
    Access, Compiler, Place, REINTERPRET_CONV, cmp_of, eval_int, float_code, int_of, int_op,
    skip_parens,
};
use crate::kbc::{Op, ak, bi, cce, cmp, int_aux, wide};
use crate::mem::Origin;
use crate::ty::{OTy, TyKind};

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
    pub(super) fn wide_place(&mut self, e: ExprId) -> u32 {
        let e = skip_parens(self.ast(), e);
        let n = self.node(e);
        if n.value_category().is_glvalue() {
            let place = self.lv(e, Access::Read);
            let ty = self.ety(e);
            return self.wide_read_place(place, ty, ak::READ, false, e);
        }
        let ty = self.ety(e);
        self.materialize(e, ty)
    }

    /// The address of the wide value of type `ty` at `place`: a
    /// bit-field's is a temporary it is read into (`rmw`: the read of a
    /// read-modify-write, access kind `k`).
    fn wide_read_place(&mut self, place: Place, ty: OTy, k: u32, rmw: bool, e: ExprId) -> u32 {
        match place {
            Place::Mem(p) => p,
            Place::Bits { p, bit, width } => {
                let t = self.wide_temp(ty, e);
                let f = if rmw { wide::RMW } else { 0 };
                let x = bit | width << 3;
                self.wide(wide::LOAD_BITS | wide::INIT | f, k, [t, p, 0], ty, x, e);
                t
            }
            Place::Reg(_) | Place::Lane { .. } => {
                self.trap(e, "kiln: internal: the address of a register value was taken");
                self.reg()
            }
            // A bit-field conditional ([expr.cond]/5): the selected
            // operand is read into one temporary.
            Place::Cond { c, i } => {
                let [pt, pf] = self.cond_places[i as usize];
                let t = self.wide_temp(ty, e);
                let j = self.jmp_if(c, false);
                self.wide_read_into(pt, t, ty, k, rmw, e);
                let end = self.jmp();
                self.patch_here(j);
                self.wide_read_into(pf, t, ty, k, rmw, e);
                self.patch_here(end);
                t
            }
        }
    }

    /// Reads the wide value at `place` into the temporary `t`.
    fn wide_read_into(&mut self, place: Place, t: u32, ty: OTy, k: u32, rmw: bool, e: ExprId) {
        if let Place::Bits { p, bit, width } = place {
            let f = if rmw { wide::RMW } else { 0 };
            self.wide(wide::LOAD_BITS | wide::INIT | f, k, [t, p, 0], ty, bit | width << 3, e);
        } else {
            let q = self.wide_read_place(place, ty, k, rmw, e);
            self.wide(wide::MOV | wide::INIT, 0, [t, q, 0], ty, 0, e);
        }
    }

    /// Writes the wide value at `src` back to bit-field `place` (a no-op
    /// for an object, which the operation wrote in place).
    fn wide_write_bits(&mut self, place: Place, src: u32, ty: OTy, k: u32, init: bool, e: ExprId) {
        if matches!(place, Place::Bits { .. } | Place::Cond { .. }) {
            self.wide_store(place, src, ty, k, init, e);
        }
    }

    /// Stores the wide value at `src` to `place` (access kind `k`): a
    /// conditional's to the operand its condition selected.
    fn wide_store(&mut self, place: Place, src: u32, ty: OTy, k: u32, init: bool, e: ExprId) {
        let f = if init { wide::INIT } else { 0 };
        match place {
            Place::Mem(dst) => self.wide(wide::MOV | f, k, [dst, src, 0], ty, 0, e),
            Place::Bits { p, bit, width } => {
                self.wide(wide::STORE_BITS | f, k, [p, src, 0], ty, bit | width << 3, e);
            }
            Place::Cond { c, i } => {
                let [pt, pf] = self.cond_places[i as usize];
                let j = self.jmp_if(c, false);
                self.wide_store(pt, src, ty, k, init, e);
                let end = self.jmp();
                self.patch_here(j);
                self.wide_store(pf, src, ty, k, init, e);
                self.patch_here(end);
            }
            // A wide value is memory-class: never a register, nor a lane
            // place (only a `bool` vector's lanes are: a wide lane is an
            // array element, `lane_place`).
            Place::Reg(_) | Place::Lane { .. } => {
                unreachable!("wide value in a register or a bit lane: {e:?}")
            }
        }
    }

    /// Initializes the wide bit-field of declared type `ty` at `q` (bit
    /// `bit`, `width` bits) from `x`, or with zero for `None`.
    pub(super) fn wide_field_init(
        &mut self,
        x: Option<ExprId>,
        q: u32,
        ty: OTy,
        bw: (u32, u32),
        e: ExprId,
    ) {
        let at = x.unwrap_or(e);
        let t = self.wide_temp(ty, at);
        match x {
            Some(x) => self.init(x, t, ty),
            None => {
                self.emit_at(Op::ZeroInit, 0, &[t, ty.0], e);
            }
        }
        let place = Place::Bits { p: q, bit: bw.0, width: bw.1 };
        self.wide_write_bits(place, t, ty, ak::INIT, true, at);
    }

    /// The value of integer literal `e` as little-endian 128-bit chunks.
    fn literal_chunks(&self, e: ExprId) -> Vec<u128> {
        let ast = self.ast();
        if let Some(v) = ast.exprs.int_value(e) {
            return vec![u128::from(v)];
        }
        let words = match ast.exprs.literal_const(e).and_then(|c| ast.consts.scalar(c)) {
            Some(kiln_ast::consts::Scalar::Int { bits, .. }) => bits.words().to_vec(),
            _ => return vec![eval_int(ast, e).unwrap_or(0)],
        };
        words
            .chunks(2)
            .map(|c| u128::from(c[0]) | c.get(1).map_or(0, |&h| u128::from(h) << 64))
            .collect()
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
        let (r, (w, s)) = self.index_rv(c, true);
        (r, w | u32::from(s) << 8)
    }

    /// The register value of integer operand `i` of a subscript, a pointer
    /// offset or a shift count (`copy`: an `rv_copy`), and its width and
    /// signedness: a wide `_BitInt` clamped to 128 bits, as wide as a
    /// register gets (a value out of them is out of range for every array
    /// and every shift; spec/oven.md "`_BitInt`").
    pub(super) fn index_rv(&mut self, i: ExprId, copy: bool) -> (u32, (u32, bool)) {
        let it = self.ety(i);
        if let Some((_, s)) = self.o.types.wide(it) {
            let a = self.wide_place(i);
            let r = self.reg();
            let x = 128 | u32::from(s) << 8;
            self.wide(wide::TO, 0, [r, a, 0], it, x | 1 << 9, i);
            return (r, (128, s));
        }
        let r = if copy { self.rv_copy(i) } else { self.rv(i) };
        (r, int_of(self.kind(it)).unwrap_or((64, true)))
    }

    /// Initializes the wide object of type `ty` at `p` from `e`, if `e` is
    /// an operation computed here (else the generic aggregate forms
    /// apply: a call, `?:`, a comma, a glvalue's copy).
    pub(super) fn wide_init(&mut self, e: ExprId, p: u32, ty: OTy) -> bool {
        let n = self.node(e);
        let init = wide::INIT;
        match n.kind() {
            ExprKind::IntegerLiteral => {
                // A literal wider than 128 bits (a substituted non-type
                // template argument; C++ spells none): its 128-bit chunks,
                // most significant first, shifted in.
                let chunks = self.literal_chunks(e);
                if chunks.len() <= 1 {
                    let v = chunks.first().copied().unwrap_or(0);
                    let r = self.reg();
                    self.konst(r, v);
                    self.wide(wide::FROM | init, 0, [p, r, 0], ty, 128, e);
                    return true;
                }
                let (r, cnt) = (self.reg(), self.reg());
                self.konst(cnt, 128);
                let mut acc = self.wide_temp(ty, e);
                self.konst(r, chunks[chunks.len() - 1]);
                self.wide(wide::FROM | init, 0, [acc, r, 0], ty, 128, e);
                for &c in chunks[..chunks.len() - 1].iter().rev() {
                    let (sh, lo, next) =
                        (self.wide_temp(ty, e), self.wide_temp(ty, e), self.wide_temp(ty, e));
                    self.wide(wide::SHL | init, 0, [sh, acc, cnt], ty, 128, e);
                    self.konst(r, c);
                    self.wide(wide::FROM | init, 0, [lo, r, 0], ty, 128, e);
                    self.wide(wide::OR | init, 0, [next, sh, lo], ty, 0, e);
                    acc = next;
                }
                self.wide(wide::MOV | init, 0, [p, acc, 0], ty, 0, e);
            }
            ExprKind::CXXScalarValueInitExpr => {
                self.emit_at(Op::ZeroInit, 0, &[p, ty.0], e);
            }
            ExprKind::SubstNonTypeTemplateParmExpr => {
                let r = self.ast().exprs.subst_replacement(e);
                self.init_inner(r, p, ty);
            }
            ExprKind::CallExpr => return self.wide_builtin_init(e, p, ty),
            // One lane of a vector of wide lanes (a prvalue vector's).
            ExprKind::ExtVectorElementExpr => {
                let lanes = self.ext_elt_regs(e);
                let Some(&a) = lanes.first() else { return true };
                self.wide(wide::MOV | init, 0, [p, a, 0], ty, 0, e);
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
                let ck = self.ast().exprs.cast_kind(e);
                if k == ExprKind::CXXReinterpretCastExpr {
                    // [expr.const]/5.15, whatever conversion it performs
                    // (`reinterpret_cast<S>(s)`, `reinterpret_cast<S>(p)`;
                    // `cast`'s rule).
                    self.cce_check(e, "reinterpret_cast is not allowed in a constant expression");
                }
                match ck {
                    CastKind::PointerToIntegral => {
                        // As `cast`'s, into a wide integer: the address's
                        // 64 bits, zero-extended.
                        if k != ExprKind::CXXReinterpretCastExpr {
                            self.cce_check(e, REINTERPRET_CONV);
                        }
                        let r = self.rv(op);
                        let v = self.reg();
                        self.cce(e, cce::PTR_TO_INT, v, r, REINTERPRET_CONV);
                        self.wide(wide::FROM | init, 0, [p, v, 0], ty, 64, e);
                    }
                    CastKind::LValueToRValue => {
                        // The read is noted at the operand's begin, as a
                        // narrow load's (`v[2]`: at `v`).
                        let a = self.wide_place(op);
                        let at = self.begin(e);
                        self.wide_loc(wide::MOV | init, 0, [p, a, 0], ty, 0, e, at);
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
                    CastKind::FloatingToIntegral => {
                        // clang reports an out-of-range value at the
                        // operand's begin.
                        let r = self.rv(op);
                        let x = self.float_x(ot);
                        let at = self.sites_len();
                        self.wide(wide::FROM_FLOAT | init, 0, [p, r, 0], ty, x, e);
                        let begin = self.begin(op);
                        self.o.sites[at..].iter_mut().for_each(|s| s.loc = begin);
                    }
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
                        let place = self.lv_rmw(x, e);
                        let inc = op == UnaryOp::PostInc;
                        let k = if inc { ak::INCREMENT } else { ak::DECREMENT };
                        let q = self.wide_read_place(place, ty, k, true, e);
                        self.wide(wide::MOV | init, 0, [p, q, 0], ty, 0, e);
                        let sub = if inc { wide::INC } else { wide::DEC };
                        self.wide(sub | wide::RMW, k, [q, q, 0], ty, 0, e);
                        self.wide_write_bits(place, q, ty, k, false, e);
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
        let place = self.lv(a, Access::Write);
        self.rmw_loc = saved;
        self.wide_store(place, src, ty, ak::ASSIGN, false, e);
        place
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
            unreachable!("compound assignment without a wide operation: {e:?}");
        };
        let shift = matches!(sub, wide::SHL | wide::SHR);
        if let TyKind::Ptr(pointee) = self.kind(lty) {
            // `p += i`, `p -= i` with a wide offset ([expr.ass]/7, the
            // right operand first, [expr.ass]/1).
            let k = ak::COMPOUND;
            let (ri, (w, s)) = self.index_rv(x.rhs, true);
            let place = self.lv_rmw(lhs, e);
            self.volatile_glvalue(lhs, k, e);
            let (old, new) = (self.reg(), self.reg());
            self.load(place, lty, k, e, old);
            let aux = int_aux(w as u8, s) | u32::from(sub == wide::SUB) << 9;
            self.emit_ptr_add(aux, &[new, old, ri, pointee.0], e);
            self.store(place, new, lty, k, e);
            return place;
        }
        if shift && self.o.types.wide(lty).is_none() {
            // A narrow left operand shifted by a wide count: computed in
            // its promoted type.
            let k = ak::COMPOUND;
            let (c, cx) = self.shift_count(x.rhs);
            let place = self.lv_rmw(lhs, e);
            self.volatile_glvalue(lhs, k, e);
            let old = self.reg();
            self.load(place, lty, k, e, old);
            let (a, res, new) = (self.reg(), self.reg(), self.reg());
            self.convert(old, lty, ct, a, e);
            let (w, s) = int_of(self.kind(ct)).unwrap_or((32, true));
            let iop = if sub == wide::SHL { Op::IShl } else { Op::IShr };
            let aux = int_aux(w as u8, s) | (cx & 0xFF) << 9 | (cx >> 8 & 1) << 17;
            self.emit_at(iop, aux, &[res, a, c], e);
            self.convert(res, ct, lty, new, e);
            self.store(place, new, lty, k, e);
            return place;
        }
        if self.o.types.wide(ct).is_none() && !shift {
            return self.wide_float_compound(e, lhs, lty, x.rhs, ct);
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
                // A wide computation type has integral operands (a
                // floating one makes it floating, a pointer `lhs` is
                // above, a complex one is `cx_compound_assign`'s).
                unreachable!("wide compound assignment of a non-integral operand: {e:?}");
            }
        };
        let place = self.lv_rmw(lhs, e);
        // A wide bit-field: computed in a temporary, written back.
        let bits = matches!(place, Place::Bits { .. } | Place::Cond { .. })
            && self.o.types.wide(lty).is_some();
        let (place, bf) = if bits {
            (Place::Mem(self.wide_read_place(place, lty, k, true, e)), Some(place))
        } else {
            (place, None)
        };
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
            // A wide left operand is in memory (a bit-field's above, a
            // lane of a vector of wide lanes an array element).
            _ => unreachable!("wide compound assignment to a non-memory place: {e:?}"),
        }
        if let (Some(bf), Place::Mem(t)) = (bf, place) {
            self.wide_write_bits(bf, t, lty, k, false, e);
            return bf;
        }
        place
    }

    /// `x` of `wide::TO_FLOAT`/`wide::FROM_FLOAT` for floating type `t`.
    fn float_x(&mut self, t: OTy) -> u32 {
        match self.kind(t) {
            TyKind::Float(k) => float_code(k),
            _ => float_code(kiln_ast::consts::FloatKind::Double),
        }
    }

    /// `a op= b` of floating computation type `ct` with a wide operand
    /// ([expr.ass]/7: `a = a op b`, `a` evaluated once): the wide side
    /// converted to `ct`, the result converted back to `a`'s type.
    fn wide_float_compound(
        &mut self,
        e: ExprId,
        lhs: ExprId,
        lty: OTy,
        rhs: ExprId,
        ct: OTy,
    ) -> Place {
        let op = self.ast().exprs.binary_op(e);
        let Some(iop) = int_op(op) else {
            // `wide_compound_assign` came here for an arithmetic operator.
            unreachable!("wide floating compound assignment without an operation: {e:?}");
        };
        let k = ak::COMPOUND;
        let fx = self.float_x(ct) | super::round_code(self.ast().exprs.fp_round(e)) << 4;
        let rt = self.ety(rhs);
        let b = self.reg();
        if self.o.types.wide(rt).is_some() {
            let src = self.wide_place(rhs);
            self.wide(wide::TO_FLOAT, 0, [b, src, 0], rt, fx, e);
        } else {
            let r = self.rv_copy(rhs);
            self.convert(r, rt, ct, b, e);
        }
        let bf = self.lv_rmw(lhs, e);
        let a = self.reg();
        let wide_lhs = self.o.types.wide(lty).is_some();
        let place = if wide_lhs && matches!(bf, Place::Bits { .. } | Place::Cond { .. }) {
            Place::Mem(self.wide_read_place(bf, lty, k, true, e))
        } else {
            bf
        };
        match place {
            Place::Mem(q) if wide_lhs => {
                self.wide(wide::TO_FLOAT | wide::RMW, k, [a, q, 0], lty, fx, e);
            }
            _ if !wide_lhs => {
                let old = self.reg();
                self.load(place, lty, k, e, old);
                self.convert(old, lty, ct, a, e);
            }
            _ => unreachable!("wide floating compound assignment to a non-memory place: {e:?}"),
        }
        let res = self.reg();
        self.arith(iop, ct, rt, res, a, b, e);
        if let Place::Mem(q) = place
            && wide_lhs
        {
            self.wide(wide::FROM_FLOAT, k, [q, res, 0], lty, fx, e);
            self.wide_write_bits(bf, q, lty, k, false, e);
        } else {
            let new = self.reg();
            self.convert(res, ct, lty, new, e);
            self.store(place, new, lty, k, e);
        }
        bf
    }

    /// `++`/`--` of the wide object at `place`.
    pub(super) fn wide_inc_dec(&mut self, place: Place, ty: OTy, op: UnaryOp, e: ExprId) {
        let inc = matches!(op, UnaryOp::PostInc | UnaryOp::PreInc);
        let k = if inc { ak::INCREMENT } else { ak::DECREMENT };
        let sub = if inc { wide::INC } else { wide::DEC };
        let q = self.wide_read_place(place, ty, k, true, e);
        self.wide(sub | wide::RMW, k, [q, q, 0], ty, 0, e);
        self.wide_write_bits(place, q, ty, k, false, e);
    }

    /// Register `dst` = the comparison `c` (`cmp`) of the wide values at
    /// `a` and `b`, of type `ty`.
    pub(super) fn wide_cmp_at(&mut self, c: u32, dst: u32, [a, b]: [u32; 2], ty: OTy, e: ExprId) {
        self.wide(wide::CMP, 0, [dst, a, b], ty, c, e);
    }

    /// Register `dst` = the comparison `c` of the wide switch condition
    /// at `cond` (type `ty`) with the case value `v`.
    pub(super) fn wide_case_cmp(&mut self, c: u32, dst: u32, cond: u32, ty: OTy, v: ExprId) {
        let vt = self.ety(v);
        let b = if vt == ty {
            self.wide_place(v)
        } else if self.o.types.wide(vt).is_some() {
            let src = self.wide_place(v);
            let t = self.wide_temp(ty, v);
            self.wide(wide::CAST | wide::INIT, 0, [t, src, 0], ty, vt.0, v);
            t
        } else {
            let r = self.rv(v);
            let x = self.narrow_code(vt);
            let t = self.wide_temp(ty, v);
            self.wide(wide::FROM | wide::INIT, 0, [t, r, 0], ty, x, v);
            t
        };
        self.wide(wide::CMP, 0, [dst, cond, b], ty, c, v);
    }

    /// The register value of integer operand `i` of an integral to pointer
    /// conversion or of a vector lane subscript (which clang truncates to
    /// 64 bits): a wide `_BitInt` truncated to its low 64 bits.
    pub(super) fn low64_rv(&mut self, i: ExprId) -> u32 {
        let it = self.ety(i);
        let Some((_, s)) = self.o.types.wide(it) else { return self.rv(i) };
        let a = self.wide_place(i);
        let r = self.reg();
        self.wide(wide::TO, 0, [r, a, 0], it, 64 | u32::from(s) << 8, i);
        r
    }

    /// The builtin binary operator `e` of a narrow type with a wide
    /// operand, into register `dst`: a comparison, a pointer offset, or a
    /// shift of a narrow left operand by a wide count (an arithmetic or
    /// bitwise operator's result, and a wide left operand's shift, are
    /// wide: `wide_init`).
    pub(super) fn wide_binary(&mut self, e: ExprId, dst: u32) {
        let op = self.ast().exprs.binary_op(e);
        let [l, r] = self.ast().exprs.operands(e);
        let (lt, rt) = (self.ety(l), self.ety(r));
        if let Some(c) = cmp_of(op) {
            // The usual arithmetic conversions gave both operands one type
            // ([expr.rel]/4, [expr.eq]/7): a narrow one is converted to
            // the wide one.
            if rt != lt {
                unreachable!("wide comparison of operands of distinct types: {e:?}");
            }
            let a = self.wide_place(l);
            let b = self.wide_place(r);
            return self.wide(wide::CMP, 0, [dst, a, b], lt, c, e);
        }
        let ptr = match (self.kind(lt), self.kind(rt)) {
            (TyKind::Ptr(p), _) | (_, TyKind::Ptr(p)) => Some(p),
            _ => None,
        };
        match op {
            // [expr.add]/4: `p + i`, `i + p`, `p - i`, left operand first.
            BinaryOp::Add | BinaryOp::Sub if let Some(pointee) = ptr => {
                let left_ptr = matches!(self.kind(lt), TyKind::Ptr(_));
                let (rp, (ri, (w, s))) = if left_ptr {
                    let rp = self.rv(l);
                    (rp, self.index_rv(r, false))
                } else {
                    let i = self.index_rv(l, false);
                    (self.rv(r), i)
                };
                let aux = int_aux(w as u8, s) | u32::from(op == BinaryOp::Sub) << 9;
                self.emit_ptr_add(aux, &[dst, rp, ri, pointee.0], e);
            }
            // [expr.shift]/4: the left operand is sequenced first.
            BinaryOp::Shl | BinaryOp::Shr => {
                let a = self.rv_copy(l);
                let (c, x) = self.shift_count(r);
                let ty = self.ety(e);
                let (w, s) = int_of(self.kind(ty)).unwrap_or((32, true));
                let iop = if op == BinaryOp::Shl { Op::IShl } else { Op::IShr };
                let aux = int_aux(w as u8, s) | (x & 0xFF) << 9 | (x >> 8 & 1) << 17;
                self.emit_at(iop, aux, &[dst, a, c], e);
            }
            _ => unreachable!("narrow {op:?} with a wide operand: {e:?}"),
        }
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
            CastKind::IntegralToFloating => {
                let a = self.wide_place(op);
                // x >> 4: the `FENV_ROUND` mode (`round_code`).
                let x = self.float_x(ty) | super::round_code(self.ast().exprs.fp_round(e)) << 4;
                self.wide(wide::TO_FLOAT, 0, [dst, a, 0], ot, x, e);
            }
            // Otherwise a wide operand with a narrow result is an integral
            // one's conversion: an integral to pointer one is `cast`'s (as
            // a narrow operand's), a splat to a vector is a vector
            // operation (`vector.rs`), and every other kind has an operand
            // or a result of another category, or a wide result
            // (`wide_init`).
            ck => unreachable!("{ck:?} of a wide operand to a narrow type: {e:?}"),
        }
    }
}

impl Compiler<'_> {
    /// `wide` with its instruction's notes at `loc`.
    #[allow(clippy::too_many_arguments)]
    fn wide_loc(&mut self, sub: u32, k: u32, args: [u32; 3], ty: OTy, x: u32, e: ExprId, loc: Loc) {
        let [d, a, b] = args;
        self.emit_site(Op::Wide, sub | k << 8, &[d, a, b, ty.0, x], e, loc);
    }

    /// The width and signedness of integer type `t`, narrow or wide.
    fn any_int_ws(&self, t: OTy) -> (u32, bool) {
        self.o.types.wide(t).or_else(|| int_of(self.kind(t))).unwrap_or((1, false))
    }

    /// The address of a temporary of wide type `to` holding the value of
    /// integer operand `x` (narrow or wide), converted.
    fn wide_conv_place(&mut self, x: ExprId, to: OTy, e: ExprId) -> u32 {
        let xt = self.ety(x);
        if xt == to {
            return self.wide_place(x);
        }
        let t = self.wide_temp(to, e);
        if self.o.types.wide(xt).is_some() {
            let a = self.wide_place(x);
            self.wide(wide::CAST | wide::INIT, 0, [t, a, 0], to, xt.0, e);
        } else {
            let r = self.rv(x);
            let rx = self.narrow_code(xt);
            self.wide(wide::FROM | wide::INIT, 0, [t, r, 0], to, rx, e);
        }
        t
    }

    /// The checked arithmetic builtins (`__builtin_add_overflow` and
    /// kin, `k` 0 add, 1 sub, 2 mul; GCC's `_p` predicates with `p`
    /// `None`) with a wide `_BitInt` operand or result type `rt`: the
    /// exact result in a synthetic signed integer wide enough for it,
    /// then wrapped to `rt` (stored through register `p`); register
    /// `dst` = whether the wrapped value differs from the exact one
    /// (clang `IntExprEvaluator` `BI__builtin_*_overflow`, in arbitrary
    /// precision). `x`, `y`, then `r` are evaluated: `r` the result
    /// pointer (`store`), else the `_p` form's third argument, evaluated
    /// only when it has side effects (as `builtin_call`'s narrow form).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn wide_checked(
        &mut self,
        k: u8,
        [x, y]: [ExprId; 2],
        (r, store): (ExprId, bool),
        rt: OTy,
        dst: u32,
        e: ExprId,
        loc: Loc,
    ) {
        let (xt, yt) = (self.ety(x), self.ety(y));
        let (wa, _) = self.any_int_ws(xt);
        let (wb, _) = self.any_int_ws(yt);
        let (wr, sr) = self.any_int_ws(rt);
        // Signed, one bit above the exact result's magnitude and above
        // every value of `rt`.
        let need = if k == 2 { wa + wb } else { wa.max(wb) + 1 };
        let q = self.ast().exprs.get(e).ty();
        let w = self.o.types.wide_of((need + 1).max(wr + 1), true, q);
        let a = self.wide_conv_place(x, w, e);
        let b = self.wide_conv_place(y, w, e);
        let p = if store {
            Some(self.rv(r))
        } else {
            if super::builtins::has_side_effects(self.ast(), r)
                || super::builtins::contains_call(self.ast(), r)
            {
                self.effect(r);
            }
            None
        };
        let sub = [wide::ADD, wide::SUB, wide::MUL][usize::from(k.min(2))];
        let exact = self.wide_temp(w, e);
        self.wide(sub | wide::INIT, 0, [exact, a, b], w, 0, e);
        let back = self.wide_temp(w, e);
        if self.o.types.wide(rt).is_some() {
            let t = self.wide_temp(rt, e);
            self.wide(wide::CAST | wide::INIT, 0, [t, exact, 0], rt, w.0, e);
            self.wide(wide::CAST | wide::INIT, 0, [back, t, 0], w, rt.0, e);
            if let Some(p) = p {
                self.wide_loc(wide::MOV, ak::ASSIGN, [p, t, 0], rt, 0, e, loc);
            }
        } else {
            let v = self.reg();
            let rx = wr | u32::from(sr) << 8;
            self.wide(wide::TO, 0, [v, exact, 0], w, rx, e);
            self.wide(wide::FROM | wide::INIT, 0, [back, v, 0], w, rx, e);
            if let Some(p) = p {
                self.emit_site(Op::Store, ak::ASSIGN, &[p, v, rt.0], e, loc);
            }
        }
        self.wide(wide::CMP, 0, [dst, exact, back], w, cmp::NE, e);
    }

    /// The GCC generic bit builtin of `BITS` kind `kind` on the wide
    /// operand `x` into register `dst` (`fb`: `__builtin_clzg`/`ctzg`'s
    /// fallback for zero, evaluated after `x`).
    pub(super) fn wide_bit_count(
        &mut self,
        kind: u32,
        x: ExprId,
        fb: Option<ExprId>,
        dst: u32,
        e: ExprId,
        loc: Loc,
    ) {
        let xt = self.ety(x);
        let a = self.wide_place(x);
        let (f, has) = match fb {
            Some(f) => (self.rv(f), 1),
            None => (dst, 0),
        };
        self.wide_loc(wide::BITS, 0, [dst, a, f], xt, kind | has << 4, e, loc);
    }

    /// `__builtin_is_aligned(x, al)` of a wide `x` into register `dst`, or
    /// `__builtin_align_up`/`_down` (`k` 0/1) into the wide object at
    /// `dst` (clang `IntExprEvaluator`: the alignment, evaluated first,
    /// must be a power of two; the check's note is at the alignment).
    pub(super) fn wide_align(&mut self, k: u32, [x, al]: [ExprId; 2], dst: u32, e: ExprId) {
        let xt = self.ety(x);
        let at = self.begin(al);
        let (xw, _) = self.o.types.wide(xt).unwrap_or((129, false));
        let (b, lg) = match self.wide_align_arg(al, xw, false, e) {
            Some(b) => (b, 4),
            None => {
                let alt = self.ety(al);
                let (aw, asg) = int_of(self.kind(alt)).unwrap_or((64, false));
                let t2 = aw | u32::from(asg) << 8;
                let b = self.rv(al);
                // The check (`bi::ALIGN` kind 3): a wide operand bounds
                // no narrow alignment (width 0).
                let c = self.reg();
                self.emit_site(Op::Builtin, bi::ALIGN | (3 << 9) << 8, &[c, c, b, c, 0, t2], e, at);
                (b, 0)
            }
        };
        let a = self.wide_place(x);
        let init = if k == 2 { 0 } else { wide::INIT };
        self.wide_loc(wide::ALIGN | init, 0, [dst, a, b], xt, k | lg, e, at);
    }

    /// For a wide alignment `al` of an alignment builtin whose source is
    /// `sw` bits wide (64 for a pointer): checks it (`wide::ALIGN_ARG`)
    /// and returns a register holding its base-2 logarithm, or with
    /// `value` (a source of at most 128 bits) the alignment itself. None
    /// for a narrow `al`.
    pub(super) fn wide_align_arg(
        &mut self,
        al: ExprId,
        sw: u32,
        value: bool,
        e: ExprId,
    ) -> Option<u32> {
        let alt = self.ety(al);
        self.o.types.wide(alt)?;
        let at = self.begin(al);
        let a = self.wide_place(al);
        let d = self.reg();
        let x = sw | u32::from(value) << 11;
        self.wide_loc(wide::ALIGN_ARG, 0, [d, a, 0], alt, x, e, at);
        Some(d)
    }

    /// Register `dst` = the address of a temporary holding the wide
    /// result of builtin call `e` (a call evaluated without a result
    /// object, as `call_frame` gives a class result one).
    pub(super) fn wide_builtin_value(&mut self, e: ExprId, dst: u32) {
        let ty = self.ety(e);
        self.alloc(dst, ty, Origin::Temp(e), false, e);
        if !self.wide_builtin_init(e, dst, ty) {
            unreachable!(
                "a wide builtin result of an alignment, element-wise or reduction builtin: {e:?}"
            );
        }
    }

    /// Initializes the wide object of type `ty` at `p` from a call `e` of
    /// builtin `bid` whose result is wide: the alignment builtins, the
    /// element-wise ones and the reductions of a vector of wide lanes
    /// (clang 22 `IntExprEvaluator`; clang 19 folds none of the
    /// element-wise ones or reductions). Returns false for any other call.
    fn wide_builtin_init(&mut self, e: ExprId, p: u32, ty: OTy) -> bool {
        use kiln_ast::decl::builtin as id;
        let Some(bid) = self.direct_builtin(e) else { return false };
        let ast = self.ast();
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        match (bid, &args[..]) {
            (id::ALIGN_UP | id::ALIGN_DOWN, &[x, al]) => {
                let k = u32::from(bid == id::ALIGN_DOWN);
                self.wide_align(k, [x, al], p, e);
            }
            (
                id::ELEMENTWISE_ABS
                | id::ELEMENTWISE_MAX
                | id::ELEMENTWISE_MIN
                | id::ELEMENTWISE_ADD_SAT
                | id::ELEMENTWISE_SUB_SAT,
                &[x, ..],
            ) => {
                if self.o.clang_compat < 22 {
                    let at = self.begin(e);
                    self.trap_loc(at, super::builtins::INVALID.into());
                    return true;
                }
                let a = self.wide_place(x);
                let (sub, b) = match (bid, args.get(1)) {
                    (id::ELEMENTWISE_ABS, _) | (_, None) => (wide::ABS, 0),
                    (_, Some(&y)) => {
                        let sub = match bid {
                            id::ELEMENTWISE_MAX => wide::MAX,
                            id::ELEMENTWISE_MIN => wide::MIN,
                            id::ELEMENTWISE_ADD_SAT => wide::ADD_SAT,
                            _ => wide::SUB_SAT,
                        };
                        (sub, self.wide_place(y))
                    }
                };
                self.wide(sub | wide::INIT, 0, [p, a, b], ty, 0, e);
            }
            (id::REDUCE_ADD..=id::REDUCE_XOR, &[x]) => self.wide_reduce(e, bid, x, p, ty),
            _ => return false,
        }
        true
    }

    /// `__builtin_reduce_*(x)` of a vector of wide lanes into the object
    /// of type `ty` at `p` (as `vec_reduce`: the lanes combined in order,
    /// an overflow noted at the call).
    fn wide_reduce(&mut self, e: ExprId, bid: u32, x: ExprId, p: u32, ty: OTy) {
        use kiln_ast::decl::builtin as id;
        let begin = self.begin(e);
        if self.o.clang_compat < 22 {
            // As a narrow one's (`builtin_call`).
            return self.trap_loc(begin, "subexpression not valid in a constant expression".into());
        }
        let lanes = self.vec_regs(x);
        let Some((&first, rest)) = lanes.split_first() else { return };
        self.wide(wide::MOV | wide::INIT, 0, [p, first, 0], ty, 0, e);
        for &r in rest {
            let sub = match bid {
                id::REDUCE_MIN | id::REDUCE_MAX => {
                    // `*p = r` when `r` orders before (min) / after (max).
                    let c = if bid == id::REDUCE_MIN { cmp::LT } else { cmp::GT };
                    let t = self.reg();
                    self.wide(wide::CMP, 0, [t, r, p], ty, c, e);
                    let j = self.jmp_if(t, false);
                    self.wide(wide::MOV, 0, [p, r, 0], ty, 0, e);
                    self.patch_here(j);
                    continue;
                }
                id::REDUCE_ADD => wide::ADD,
                id::REDUCE_MUL => wide::MUL,
                id::REDUCE_AND => wide::AND,
                id::REDUCE_OR => wide::OR,
                _ => wide::XOR,
            };
            self.wide_loc(sub, 0, [p, p, r], ty, 0, e, begin);
        }
    }
}

/// Lane operations of a vector of wide `_BitInt` lanes (`vector.rs`): a
/// lane's register holds the address of a temporary holding its value,
/// which nothing writes after it is made (an operation's result is a new
/// one), so lanes may share one.
impl Compiler<'_> {
    /// A copy of the wide lane of type `ty` at `q`, read (and noted) at
    /// `loc`: the address of a new temporary.
    pub(super) fn wide_lane_load(&mut self, q: u32, ty: OTy, e: ExprId, loc: Loc) -> u32 {
        let t = self.wide_temp(ty, e);
        self.wide_loc(wide::MOV | wide::INIT, 0, [t, q, 0], ty, 0, e, loc);
        t
    }

    /// Initializes the wide lane of type `ty` at `q` from lane `r`.
    pub(super) fn wide_lane_store(&mut self, q: u32, r: u32, ty: OTy, e: ExprId) {
        self.wide(wide::MOV | wide::INIT, 0, [q, r, 0], ty, 0, e);
    }

    /// Register `d` = the comparison `c` of wide lanes `a` and `b`.
    pub(super) fn wide_lane_cmp(&mut self, c: u32, d: u32, [a, b]: [u32; 2], ty: OTy, e: ExprId) {
        self.wide(wide::CMP, 0, [d, a, b], ty, c, e);
    }

    /// Register `d` = whether wide lane `r` is nonzero.
    pub(super) fn wide_lane_truth(&mut self, d: u32, r: u32, ty: OTy, e: ExprId) {
        self.wide(wide::TO_BOOL, 0, [d, r, 0], ty, 0, e);
    }

    /// `-r` (wrapping, as a vector lane's: clang negates `APSInt` lanes
    /// without an overflow check) or `~r` of wide lane `r`.
    pub(super) fn wide_lane_unary(&mut self, minus: bool, r: u32, ty: OTy, e: ExprId) -> u32 {
        let t = self.wide_temp(ty, e);
        match self.o.types.wide(ty) {
            Some((width, true)) if minus => {
                let q = self.o.types.info(ty).q;
                let u = self.o.types.wide_of(width, false, q);
                let (a, n) = (self.wide_temp(u, e), self.wide_temp(u, e));
                self.wide(wide::CAST | wide::INIT, 0, [a, r, 0], u, ty.0, e);
                self.wide(wide::NEG | wide::INIT, 0, [n, a, 0], u, 0, e);
                self.wide(wide::CAST | wide::INIT, 0, [t, n, 0], ty, u.0, e);
            }
            _ => {
                let sub = if minus { wide::NEG } else { wide::NOT };
                self.wide(sub | wide::INIT, 0, [t, r, 0], ty, 0, e);
            }
        }
        t
    }

    /// `arith` of lanes `ra`, `rb` where the result type `ty` is wide, or
    /// a shift's count type `bt` is (register `dst` = the result: a new
    /// temporary's address for a wide `ty`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn wide_arith(
        &mut self,
        iop: Op,
        ty: OTy,
        bt: OTy,
        dst: u32,
        ra: u32,
        rb: u32,
        e: ExprId,
    ) {
        let shift = matches!(iop, Op::IShl | Op::IShr);
        let (c, cx) = match self.o.types.wide(bt) {
            // A wide count, clamped to 128 bits (`shift_count`).
            Some((_, s)) if shift => {
                let c = self.reg();
                let x = 128 | u32::from(s) << 8;
                self.wide(wide::TO, 0, [c, rb, 0], bt, x | 1 << 9, e);
                (c, x)
            }
            None if shift => (rb, self.narrow_code(bt)),
            _ => (rb, 0),
        };
        if self.o.types.wide(ty).is_none() {
            // A narrow lane shifted by a wide count.
            let (w, s) = int_of(self.kind(ty)).unwrap_or((32, true));
            let aux = int_aux(w as u8, s) | (cx & 0xFF) << 9 | (cx >> 8 & 1) << 17;
            self.emit_at(iop, aux, &[dst, ra, c], e);
            return;
        }
        let sub = match iop {
            Op::IAdd => wide::ADD,
            Op::ISub => wide::SUB,
            Op::IMul => wide::MUL,
            Op::IDiv => wide::DIV,
            Op::IRem => wide::REM,
            Op::IShl => wide::SHL,
            Op::IShr => wide::SHR,
            Op::IAnd => wide::AND,
            Op::IOr => wide::OR,
            Op::IXor => wide::XOR,
            _ => unreachable!("wide lane arithmetic {iop:?}"),
        };
        let t = self.wide_temp(ty, e);
        self.wide(sub | wide::INIT, 0, [t, ra, c], ty, cx, e);
        self.emit(Op::Mov, 0, &[dst, t]);
    }

    /// `convert` where `from` or `to` is a wide `_BitInt` (a lane's
    /// conversion: `__builtin_convertvector`, a splat).
    pub(super) fn wide_convert(&mut self, src: u32, from: OTy, to: OTy, dst: u32, e: ExprId) {
        if self.o.types.wide(to).is_some() {
            let t = self.wide_temp(to, e);
            let init = wide::INIT;
            match self.kind(from) {
                TyKind::WideInt { .. } => {
                    self.wide(wide::CAST | init, 0, [t, src, 0], to, from.0, e);
                }
                TyKind::Float(_) => {
                    let x = self.float_x(from);
                    self.wide(wide::FROM_FLOAT | init, 0, [t, src, 0], to, x, e);
                }
                _ => {
                    let x = self.narrow_code(from);
                    self.wide(wide::FROM | init, 0, [t, src, 0], to, x, e);
                }
            }
            self.emit(Op::Mov, 0, &[dst, t]);
            return;
        }
        match self.kind(to) {
            TyKind::Bool => self.wide(wide::TO_BOOL, 0, [dst, src, 0], from, 0, e),
            TyKind::Float(_) => {
                let x = self.float_x(to);
                self.wide(wide::TO_FLOAT, 0, [dst, src, 0], from, x, e);
            }
            _ => {
                let x = self.narrow_code(to);
                self.wide(wide::TO, 0, [dst, src, 0], from, x, e);
            }
        }
    }

    /// An element-wise builtin `bid` on wide lanes `a` (and `b`): the
    /// address of a new temporary holding the result.
    pub(super) fn wide_lane_elementwise(
        &mut self,
        bid: u32,
        [a, b]: [u32; 2],
        ty: OTy,
        e: ExprId,
    ) -> u32 {
        use kiln_ast::decl::builtin as id;
        let sub = match bid {
            id::ELEMENTWISE_ABS => wide::ABS,
            id::ELEMENTWISE_MAX => wide::MAX,
            id::ELEMENTWISE_MIN => wide::MIN,
            id::ELEMENTWISE_ADD_SAT => wide::ADD_SAT,
            _ => wide::SUB_SAT,
        };
        let t = self.wide_temp(ty, e);
        self.wide(sub | wide::INIT, 0, [t, a, b], ty, 0, e);
        t
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
