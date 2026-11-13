//! Checked integer arithmetic and stack allocation builtins (WP-270):
//! clang `EmitBuiltinExpr` for `__builtin_{add,sub,mul}_overflow` and
//! the typed `__builtin_{s,u}{,l,ll}{add,sub,mul}_overflow` forms, the
//! carry builtins `__builtin_{add,sub}c{b,s,,l,ll}` (WP-557) and
//! `__builtin_alloca{,_with_align}` (spec/codegen.md §3, "Builtins").

use kiln_ast::decl::builtin as id;
use kiln_ast::{ExprId, QualType};
use kiln_llvm::intrinsics::OverflowOp;
use kiln_llvm::{BinOp, CastOp, Flags, IntPredicate, TypeId as LTy};

use crate::fir::{Op, V};
use crate::module::{Cg, R, err};

/// The width and signedness of an integer type (clang
/// `getIntegerWidthAndSignedness`).
#[derive(Copy, Clone)]
struct Ws {
    w: u32,
    signed: bool,
}

impl Cg<'_> {
    fn ws(&mut self, q: QualType) -> Ws {
        let t = self.lty(q);
        Ws { w: self.m.types.int_width(t).unwrap_or(1), signed: self.is_signed(q) }
    }

    /// clang `EmitOverflowIntrinsic`: the value and the overflow flag of
    /// `llvm.{op}.with.overflow` on `x`, `y` (the flag is extracted first).
    fn overflow_intrinsic(&mut self, op: OverflowOp, x: V, y: V) -> (V, V) {
        let ty = self.f.fir.ty(&self.m, x);
        let f = kiln_llvm::intrinsics::with_overflow(&mut self.m, op, ty);
        let rty = self.m.types.struct_literal(&[ty, LTy::I1], false);
        let r = self.intrinsic_call_ty(f, vec![x, y], rty);
        let carry = self.f.fir.push(Op::ExtractValue { agg: r, idx: 1 }, LTy::I1);
        let v = self.f.fir.push(Op::ExtractValue { agg: r, idx: 0 }, ty);
        (v, carry)
    }

    /// The builtin `b` of [`id::checked_op`] called with `args`: the
    /// overflow flag, after storing the wrapped result ([gnu] checked
    /// arithmetic; clang `Builtin::BI__builtin_add_overflow` and
    /// `BI__builtin_sadd_overflow`).
    pub(crate) fn emit_checked(&mut self, b: u32, e: ExprId, args: &[ExprId]) -> R<V> {
        let (Some((op, typed)), &[x, y, r]) = (id::checked_op(b), args) else {
            return err(self.loc_of(e), "malformed checked arithmetic builtin");
        };
        let ast = self.ast;
        let xq = ast.exprs.get(x).ty();
        if typed {
            let signed = self.is_signed(xq);
            let xv = self.emit_scalar(x)?;
            let yv = self.emit_scalar(y)?;
            let (p, al) = self.ptr_align(r)?;
            let (v, carry) = self.overflow_intrinsic(ovf_op(op, signed), xv, yv);
            self.store(v, p, al, false);
            return Ok(carry);
        }
        let yq = ast.exprs.get(y).ty();
        let Some(rq) = ast.types.pointee(ast.exprs.get(r).ty()) else {
            return err(self.loc_of(e), "overflow builtin result is not a pointer");
        };
        let (xi, yi, ri) = (self.ws(xq), self.ws(yq), self.ws(rq));
        let vol = ast.types.effective_quals(rq).is_volatile();
        if op == 2 && xi.w.max(yi.w) >= ri.w && xi.signed != yi.signed {
            return self.mixed_sign_mul(x, xi, y, yi, r, rq, ri, vol);
        }
        if op == 2 && xi.w == yi.w && yi.w == ri.w && !xi.signed && !yi.signed && ri.signed {
            // `EmitCheckedUnsignedMultiplySignedResult`: past `INT_MAX`
            // overflows too; the result pointer is emitted last.
            let xv = self.emit_scalar(x)?;
            let yv = self.emit_scalar(y)?;
            let (v, carry) = self.overflow_intrinsic(OverflowOp::UMul, xv, yv);
            let ty = self.f.fir.ty(&self.m, v);
            let max = V::C(self.m.const_int(ty, int_max(ri.w)));
            let big = self.icmp(IntPredicate::Ugt, v, max);
            let carry = self.bin(BinOp::Or, Flags::NONE, carry, big);
            let (p, al) = self.ptr_align(r)?;
            self.store_mem(v, rq, p, al, vol);
            return Ok(carry);
        }
        // `EncompassingIntegerType`: signed if any is, wide enough for all.
        let signed = xi.signed || yi.signed || ri.signed;
        let w = [xi, yi, ri].iter().map(|t| t.w + u32::from(signed && !t.signed)).max();
        let ety = self.m.types.int(w.unwrap_or(1));
        let xv = self.emit_scalar(x)?;
        let yv = self.emit_scalar(y)?;
        let (p, al) = self.ptr_align(r)?;
        let xv = self.int_cast(xv, ety, xi.signed);
        let yv = self.int_cast(yv, ety, yi.signed);
        let (mut v, mut carry) = if op == 2 && signed && w.unwrap_or(1) > 64 {
            self.wide_signed_mul(xv, yv)
        } else {
            self.overflow_intrinsic(ovf_op(op, signed), xv, yv)
        };
        if w.unwrap_or(1) > ri.w {
            let rty = self.lty(rq);
            let t = self.cast(CastOp::Trunc, v, rty);
            let back = self.int_cast(t, ety, ri.signed);
            let lost = self.icmp(IntPredicate::Ne, v, back);
            carry = self.bin(BinOp::Or, Flags::NONE, carry, lost);
            v = t;
        }
        self.store_mem(v, rq, p, al, vol);
        Ok(carry)
    }

    /// The carry builtin `b` of [`id::carry_op`] (WP-557; clang
    /// `EmitBuiltinExpr` `BI__builtin_addcb`..): two
    /// `llvm.u{add,sub}.with.overflow` steps, `x op y` then `op carryin`;
    /// the OR of their flags, zero-extended, is stored through the
    /// carry-out pointer and the second sum is the value.
    pub(crate) fn emit_carry(&mut self, b: u32, e: ExprId, args: &[ExprId]) -> R<V> {
        let (Some(minus), &[x, y, ci, r]) = (id::carry_op(b), args) else {
            return err(self.loc_of(e), "malformed carry builtin");
        };
        let xv = self.emit_scalar(x)?;
        let yv = self.emit_scalar(y)?;
        let cv = self.emit_scalar(ci)?;
        let (p, al) = self.ptr_align(r)?;
        let op = if minus { OverflowOp::USub } else { OverflowOp::UAdd };
        let (s1, c1) = self.overflow_intrinsic(op, xv, yv);
        let (s2, c2) = self.overflow_intrinsic(op, s1, cv);
        let c = self.bin(BinOp::Or, Flags::NONE, c1, c2);
        let ty = self.f.fir.ty(&self.m, xv);
        let c = self.cast(CastOp::ZExt, c, ty);
        self.store(c, p, al, false);
        Ok(s2)
    }

    /// clang `EmitCheckedMixedSignMultiply`: the magnitude of the signed
    /// operand times the unsigned one, then the sign and range checks.
    #[allow(clippy::too_many_arguments, reason = "clang's helper signature")]
    fn mixed_sign_mul(
        &mut self,
        x: ExprId,
        xi: Ws,
        y: ExprId,
        yi: Ws,
        r: ExprId,
        rq: QualType,
        ri: Ws,
        vol: bool,
    ) -> R<V> {
        let (s, si, u, ui) = if xi.signed { (x, xi, y, yi) } else { (y, yi, x, xi) };
        let mut sv = self.emit_scalar(s)?;
        let mut uv = self.emit_scalar(u)?;
        if si.w < ui.w {
            let t = self.f.fir.ty(&self.m, uv);
            sv = self.cast(CastOp::SExt, sv, t);
        }
        if ui.w < si.w {
            let t = self.f.fir.ty(&self.m, sv);
            uv = self.cast(CastOp::ZExt, uv, t);
        }
        let oty = self.f.fir.ty(&self.m, sv);
        let zero = V::C(self.m.const_int(oty, 0));
        let (p, al) = self.ptr_align(r)?;
        let mty = self.mty(rq);
        let neg = self.icmp(IntPredicate::Slt, sv, zero);
        let abs_neg = self.bin(BinOp::Sub, Flags::NONE, zero, sv);
        let abs = self.fold_select(neg, abs_neg, sv);
        let (ur, uo) = self.overflow_intrinsic(OverflowOp::UMul, abs, uv);
        let ow = si.w.max(ui.w);
        let carry = if ri.signed {
            // Overflow past `INT_MAX + IsNegative`.
            let max = V::C(self.m.const_int(oty, int_max(ri.w)));
            let nz = self.cast(CastOp::ZExt, neg, oty);
            let max = self.bin(BinOp::Add, Flags::NONE, max, nz);
            let so = self.icmp(IntPredicate::Ugt, ur, max);
            self.bin(BinOp::Or, Flags::NONE, uo, so)
        } else {
            let nz = self.icmp(IntPredicate::Ne, ur, zero);
            let under = self.bin(BinOp::And, Flags::NONE, neg, nz);
            let mut c = self.bin(BinOp::Or, Flags::NONE, uo, under);
            if ri.w < ow {
                let max = V::C(self.m.const_int(oty, uint_max(ri.w)));
                let to = self.icmp(IntPredicate::Ugt, ur, max);
                c = self.bin(BinOp::Or, Flags::NONE, c, to);
            }
            c
        };
        let negr = self.bin(BinOp::Sub, Flags::NONE, zero, ur);
        let v = self.fold_select(neg, negr, ur);
        let v = self.int_cast(v, mty, false);
        self.store_mem(v, rq, p, al, vol);
        Ok(carry)
    }

    /// `llvm.smul.with.overflow` on integers wider than 64 bits, expanded
    /// inline: LLVM lowers the `i128` form to the compiler-rt
    /// `__muloti4` call on most 64-bit targets, which libgcc does not
    /// provide (GCC expands it inline). The magnitudes are multiplied
    /// unsigned; the product overflows past `INT_MAX + IsNegative` (as
    /// [`Cg::mixed_sign_mul`]), and the wrapped value is the magnitude
    /// product negated when the signs differ (spec/codegen.md §3).
    fn wide_signed_mul(&mut self, x: V, y: V) -> (V, V) {
        let ty = self.f.fir.ty(&self.m, x);
        let w = self.m.types.int_width(ty).unwrap_or(128);
        let zero = V::C(self.m.const_int(ty, 0));
        let mag = |cg: &mut Self, v: V| {
            let neg = cg.icmp(IntPredicate::Slt, v, zero);
            let minus = cg.bin(BinOp::Sub, Flags::NONE, zero, v);
            (neg, cg.fold_select(neg, minus, v))
        };
        let (nx, ax) = mag(self, x);
        let (ny, ay) = mag(self, y);
        let (ur, uo) = self.overflow_intrinsic(OverflowOp::UMul, ax, ay);
        let neg = self.bin(BinOp::Xor, Flags::NONE, nx, ny);
        // `INT_MAX` of any width (an `i129` encompassing type included).
        let mut words = vec![u64::MAX; w.div_ceil(64) as usize];
        if let Some(top) = words.get_mut(((w - 1) / 64) as usize) {
            *top &= !(1u64 << ((w - 1) % 64));
        }
        let max = V::C(self.m.const_int_words(ty, &words));
        let nz = self.cast(CastOp::ZExt, neg, ty);
        let max = self.bin(BinOp::Add, Flags::NONE, max, nz);
        let so = self.icmp(IntPredicate::Ugt, ur, max);
        let carry = self.bin(BinOp::Or, Flags::NONE, uo, so);
        let negr = self.bin(BinOp::Sub, Flags::NONE, zero, ur);
        (self.fold_select(neg, negr, ur), carry)
    }

    /// `IRBuilder::CreateSelect`: folded only when every operand is a
    /// constant (`ConstantFolder::FoldSelect`).
    fn fold_select(&mut self, c: V, t: V, f: V) -> V {
        if matches!((t, f), (V::C(_), V::C(_))) {
            return self.select(c, t, f);
        }
        let ty = self.f.fir.ty(&self.m, t);
        self.f.fir.push(Op::Select { c, t, f }, ty)
    }

    /// clang `CreateStore(EmitToMemory(v, q), ...)`: an `i1` is widened,
    /// as is a `_BitInt(N)` stored in a wider slot.
    fn store_mem(&mut self, v: V, q: QualType, p: V, al: u64, vol: bool) {
        let mty = self.mty(q);
        let v = if self.f.fir.ty(&self.m, v) == LTy::I1 {
            self.cast(CastOp::ZExt, v, mty)
        } else {
            self.emit_to_memory(v, q, mty)
        };
        self.store(v, p, al, vol);
    }

    /// clang `Builtin::BI__builtin_alloca{,_with_align}`: a dynamic
    /// `alloca i8, i64 %n` where the call is, aligned to the target's
    /// `__BIGGEST_ALIGNMENT__` or to the requested alignment in bits.
    /// Under `-ftrivial-auto-var-init=zero|pattern` (and kiln's C++26
    /// default) the memory is then filled ([`Cg::auto_init_alloca`]),
    /// except for the `_uninitialized` variants.
    pub(crate) fn emit_alloca(&mut self, b: u32, e: ExprId, args: &[ExprId]) -> R<V> {
        let Some(&n) = args.first() else {
            return err(self.loc_of(e), "__builtin_alloca needs a size");
        };
        if self.in_split_coroutine() {
            // Sema rejects it (`sema_coroutine_dynamic_alloca`); WP-1374.
            let name = id::info(b).map_or("__builtin_alloca", |i| i.name);
            return err(
                self.loc_of(e),
                format!(
                    "a call to '{name}' in a coroutine is not supported: the coroutine frame \
                     cannot hold a dynamically sized stack allocation"
                ),
            );
        }
        let count = self.emit_scalar(n)?;
        let align = if matches!(b, id::ALLOCA_WITH_ALIGN | id::ALLOCA_WITH_ALIGN_UNINITIALIZED) {
            let Some(&a) = args.get(1) else {
                return err(self.loc_of(e), "__builtin_alloca_with_align needs an alignment");
            };
            let av = self.emit_scalar(a)?;
            match self.cint(av) {
                Some((_, bits, _)) => u64::try_from(bits / 8).unwrap_or(1).max(1),
                None => return err(self.loc_of(a), "alignment is not a constant"),
            }
        } else {
            self.target.biggest_align
        };
        let p = self.f.fir.push(Op::DynAlloca { ty: LTy::I8, count, align }, LTy::PTR);
        if !matches!(b, id::ALLOCA_UNINITIALIZED | id::ALLOCA_WITH_ALIGN_UNINITIALIZED) {
            self.auto_init_alloca(p, count, align);
        }
        Ok(p)
    }
}

fn ovf_op(op: u8, signed: bool) -> OverflowOp {
    match (op, signed) {
        (0, true) => OverflowOp::SAdd,
        (0, false) => OverflowOp::UAdd,
        (1, true) => OverflowOp::SSub,
        (1, false) => OverflowOp::USub,
        (_, true) => OverflowOp::SMul,
        (_, false) => OverflowOp::UMul,
    }
}

/// `APInt::getSignedMaxValue(w)` as the low bits of an `i128`.
fn int_max(w: u32) -> i128 {
    if w >= 128 { i128::MAX } else { (1i128 << (w - 1)) - 1 }
}

/// `APInt::getMaxValue(w)` as the low bits of an `i128`.
fn uint_max(w: u32) -> i128 {
    if w >= 128 { -1 } else { ((1u128 << w) - 1) as i128 }
}
