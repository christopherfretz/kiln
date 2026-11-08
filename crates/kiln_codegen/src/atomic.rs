//! Atomic operations (WP-154; spec/codegen.md §7d): clang `CGAtomic`'s
//! `EmitAtomicExpr` for the `__c11_atomic_*` / `__atomic_*` builtins, the
//! loads, stores and read-modify-writes of `_Atomic` lvalues, and the atomic
//! builtins `CGBuiltin` lowers itself (`test_and_set`, `clear`, the fences,
//! `is_lock_free`).
//!
//! The shapes follow clang 19 at -O0: a constant memory order picks one
//! ordering (an invalid one emits nothing), a non-constant one switches over
//! the C ABI values; operands travel through `iN` temporaries of the atomic
//! width. Sizes that need a libcall use the generic `__atomic_*` ABI
//! (WP-165). Floating `min`/`max_fetch` deviate from clang 19 (which emits
//! an invalid `icmp`): the result is `llvm.minnum` / `llvm.maxnum`.

use kiln_ast::decl::builtin::atomic::{self as at, Form, Rmw};
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind};
use kiln_ast::{ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_llvm::{
    Attr, AttrKind, AttrList, BinOp, CallOpts, CastOp, CmpXchgOpts, Flags, IntPredicate, MemOpts,
    Ordering, RmwOp, SyncScope, TypeId as LTy, Value,
};
use kiln_source::Span;

use crate::expr::LV;
use crate::fir::{Blk, Fir, Op, V};
use crate::func::Addr;
use crate::module::{Cg, R, err};
use crate::tbaa::Tb;

/// An atomic access with `ordering` (`MemOpts` of a plain one otherwise).
fn amem(align: u64, volatile: bool, ordering: Ordering, scope: SyncScope) -> MemOpts {
    MemOpts { align, volatile, ordering, scope }
}

/// clang's ordering for a constant C ABI memory order of a
/// read-modify-write (`None`: invalid for the operation, nothing emitted).
fn success_order(o: u128, is_load: bool, is_store: bool) -> Option<Ordering> {
    match o {
        0 => Some(Ordering::Monotonic),
        1 | 2 if !is_store => Some(Ordering::Acquire),
        3 if !is_load => Some(Ordering::Release),
        4 if !is_load && !is_store => Some(Ordering::AcqRel),
        5 => Some(Ordering::SeqCst),
        _ => None,
    }
}

/// The operands of an `AtomicExpr` once `EmitAtomicExpr` has evaluated
/// them (addresses already typed as the operation's `iN`).
struct AtomicOps {
    id: u32,
    form: Form,
    ptr: Addr,
    val1: Option<Addr>,
    val2: Option<Addr>,
    dest: Option<Addr>,
    order_fail: Option<V>,
    weak: Option<V>,
    volatile: bool,
    /// A cmpxchg whose `expected` operand was copied into an atomic-sized
    /// temporary: the original object (pointer, alignment) and its size in
    /// bytes, written back from the old value on failure.
    expected_back: Option<(V, u64, u64)>,
    /// `AtomicExpr::getValueType` (the `_Atomic` value type, or the
    /// pointee of a GNU builtin's pointer).
    value_q: QualType,
}

impl Cg<'_> {
    /// A block created and appended to the function at once (clang
    /// `createBasicBlock(name, CurFn)`).
    fn block_at_end(&mut self) -> Blk {
        let b = self.f.fir.create_block();
        self.f.fir.place_at_end(b);
        b
    }

    fn ci32(&mut self, v: i128) -> V {
        V::C(self.m.const_int(LTy::I32, v))
    }

    /// `DataLayout::getTypeSizeInBits` of `q`'s memory type (80 for the
    /// x87 `long double`, the allocation size otherwise).
    fn mem_bits(&mut self, q: QualType) -> u64 {
        let t = self.mty(q);
        match self.m.types.float_width(t) {
            Some(w) => u64::from(w),
            None => self.size_align(q).0 * 8,
        }
    }

    /// clang `EmitValToTemp`: `e` evaluated into a fresh temporary.
    fn val_to_temp(&mut self, e: ExprId, ty: LTy) -> R<Addr> {
        let q = self.ast.exprs.get(e).ty();
        let t = self.temp(q);
        if self.is_aggregate(q) {
            self.emit_agg(e, t, false)?;
        } else {
            let v = self.emit_scalar(e)?;
            self.store_scalar(v, t, q);
        }
        Ok(Addr { ty, ..t })
    }

    // ---- AtomicExpr ----------------------------------------------------

    /// clang `EmitAtomicExpr`; an aggregate result is copied to `agg`.
    pub(crate) fn emit_atomic_expr(&mut self, e: ExprId, agg: Option<Addr>) -> R<Option<V>> {
        let ast = self.ast;
        let id = ast.exprs.atomic_op(e);
        let Some(form) = at::form(id) else {
            return err(self.loc_of(e), "unknown atomic operation");
        };
        let subs: Vec<ExprId> = ast.lists.iter(ast.exprs.atomic_subs(e)).collect();
        let sub = |i: usize| subs.get(i).copied();
        let Some(ptr_e) = sub(0) else { return err(self.loc_of(e), "malformed AtomicExpr") };
        let Some(atomic_q) = ast.types.pointee(ast.exprs.get(ptr_e).ty()) else {
            return err(self.loc_of(e), "atomic operation on a non-pointer");
        };
        let value_q = ast.types.as_atomic(atomic_q).unwrap_or(atomic_q);
        let volatile = self.is_volatile(atomic_q);
        let size = self.size_align(atomic_q).0;
        if size == 0 {
            return err(self.loc_of(e), "atomic operation on a zero-sized object");
        }
        let use_libcall = !size.is_power_of_two() || size > 16;
        let rmw = at::rmw(id);
        let fp = ast.types.is_floating(value_q);
        let (pv, palign) = self.ptr_align(ptr_e)?;
        if form == Form::Init {
            // clang `EmitAtomicInit`: a plain initialization.
            let Some(v1) = sub(1) else { return err(self.loc_of(e), "malformed AtomicExpr") };
            let addr = Addr { v: pv, ty: self.mty(atomic_q), align: palign };
            self.atomic_init(v1, LV { addr, q: atomic_q, bf: None, vidx: None, tb: Tb::Type })?;
            return Ok(None);
        }
        self.atomic_alignment_warnings(e, size, palign);
        let Some(order_e) = sub(1) else { return err(self.loc_of(e), "malformed AtomicExpr") };
        let order = self.emit_scalar(order_e)?;
        // `ShouldCastToIntPtrTy`: floating add/sub/min/max work on the
        // floating type, everything else on `iN`.
        let int_ty = if use_libcall {
            LTy::PTR
        } else {
            self.m.types.int(u32::try_from(size * 8).unwrap_or(8))
        };
        let op_ty = if fp
            && rmw.is_some_and(|(r, _)| matches!(r, Rmw::Add | Rmw::Sub | Rmw::Max | Rmw::Min))
        {
            self.mty(value_q)
        } else {
            int_ty
        };
        let ptr = Addr { v: pv, ty: op_ty, align: palign };
        let mut ops = AtomicOps {
            id,
            form,
            ptr,
            val1: None,
            val2: None,
            dest: None,
            order_fail: None,
            weak: None,
            volatile,
            expected_back: None,
            value_q,
        };
        let need = |i: usize| -> R<ExprId> {
            sub(i).map_or_else(|| err(None, "malformed AtomicExpr"), Ok)
        };
        // The source types of `val1`/`val2`, for `convertToAtomicIntPointer`.
        let pointee = |x: ExprId| ast.types.pointee(ast.exprs.get(x).ty());
        let ty_of = |x: ExprId| Some(ast.exprs.get(x).ty());
        let mut src: [Option<QualType>; 2] = [None, None];
        match form {
            Form::Init | Form::Load => {}
            Form::LoadCopy => {
                let (v, a) = self.ptr_align(need(2)?)?;
                ops.dest = Some(Addr { v, ty: op_ty, align: a });
            }
            Form::Copy if !at::by_value(id) => {
                let (v, a) = self.ptr_align(need(2)?)?;
                ops.val1 = Some(Addr { v, ty: op_ty, align: a });
                src[0] = pointee(need(2)?);
            }
            Form::GnuXchg => {
                let (v, a) = self.ptr_align(need(2)?)?;
                ops.val1 = Some(Addr { v, ty: op_ty, align: a });
                src[0] = pointee(need(2)?);
                let (v, a) = self.ptr_align(need(3)?)?;
                ops.dest = Some(Addr { v, ty: op_ty, align: a });
            }
            Form::C11CmpXchg | Form::GnuCmpXchg => {
                let (v, a) = self.ptr_align(need(2)?)?;
                ops.val1 = Some(Addr { v, ty: op_ty, align: a });
                src[0] = pointee(need(2)?);
                ops.val2 = Some(if id == at::CMPXCHG {
                    let (v, a) = self.ptr_align(need(4)?)?;
                    src[1] = pointee(need(4)?);
                    Addr { v, ty: op_ty, align: a }
                } else {
                    src[1] = ty_of(need(4)?);
                    self.val_to_temp(need(4)?, op_ty)?
                });
                ops.order_fail = Some(self.emit_scalar(need(3)?)?);
                if form == Form::GnuCmpXchg {
                    ops.weak = Some(self.emit_scalar(need(5)?)?);
                }
            }
            Form::Arithmetic
                if at::is_c11(id)
                    && matches!(rmw, Some((Rmw::Add | Rmw::Sub, _)))
                    && ast.types.is_pointer(value_q) =>
            {
                // A C11 pointer add/sub counts elements.
                let v1 = need(2)?;
                let v1q = ast.exprs.get(v1).ty();
                let x = self.emit_scalar(v1)?;
                let pointee = ast.types.pointee(value_q).expect("pointer");
                let inc = if ast.types.is_void(pointee) || ast.types.is_function(pointee) {
                    1
                } else {
                    self.size_align(pointee).0
                };
                let c = self.i64c(i128::from(inc));
                let x = self.bin(BinOp::Mul, Flags::NONE, x, c);
                let t = self.temp(v1q);
                self.store_scalar(x, t, v1q);
                ops.val1 = Some(Addr { ty: op_ty, ..t });
                src[0] = Some(v1q);
            }
            _ => {
                let v1 = need(2)?;
                src[0] = ty_of(v1);
                ops.val1 = Some(self.val_to_temp(v1, op_ty)?);
            }
        }
        if op_ty == int_ty {
            // clang `AtomicInfo::convertToAtomicIntPointer`: an operand
            // whose memory type is not as wide as the atomic one is copied
            // into an atomic-sized temporary first.
            // At `-fclang-compat=22` the padding is zeroed first and a
            // failed compare-exchange writes the old value back to
            // `*expected` itself (C11 7.17.7.4p2); clang 19 did neither,
            // so a padded compare-exchange could fail on garbage
            // (spec/codegen.md §6a).
            let abits = size * 8;
            let c22 = self.opts.clang_compat >= 22;
            let is_cx = matches!(form, Form::C11CmpXchg | Form::GnuCmpXchg);
            for (i, (slot, q)) in
                [(&mut ops.val1, src[0]), (&mut ops.val2, src[1])].into_iter().enumerate()
            {
                let (Some(a), Some(q)) = (*slot, q) else { continue };
                let bits = self.mem_bits(q);
                if bits != abits {
                    let t = self.temp(atomic_q);
                    let n = abits.min(bits) / 8;
                    if c22 && bits < abits {
                        self.memset(t, 0, abits / 8, false);
                    }
                    self.memcpy(t, a.v, a.align, n, false);
                    if c22 && is_cx && i == 0 {
                        ops.expected_back = Some((a.v, a.align, n));
                    }
                    *slot = Some(Addr { ty: op_ty, ..t });
                }
            }
        }
        let rq = ast.exprs.get(e).ty();
        let is_cmpxchg = matches!(form, Form::C11CmpXchg | Form::GnuCmpXchg);
        if ops.dest.is_none() {
            if is_cmpxchg {
                ops.dest = Some(self.temp(rq));
            } else if !ast.types.is_void(rq) {
                let t = self.temp(atomic_q);
                ops.dest = Some(Addr { ty: op_ty, ..t });
            }
        }
        let is_store = form == Form::Copy;
        let is_load = matches!(form, Form::Load | Form::LoadCopy);
        if use_libcall {
            let r = self.atomic_expr_libcall(e, &ops, size, order)?;
            if is_cmpxchg {
                // The `bool` comes straight from the call.
                return Ok(Some(r));
            }
        } else if let Some((_, o, _)) = self.cint(order) {
            if let Some(ord) = success_order(o, is_load, is_store) {
                self.atomic_op(&ops, ord)?;
            }
        } else {
            let mono = self.block_at_end();
            let acq = (!is_store).then(|| self.block_at_end());
            let rel = (!is_load).then(|| self.block_at_end());
            let acq_rel = (!is_load && !is_store).then(|| self.block_at_end());
            let seq = self.block_at_end();
            let cont = self.block_at_end();
            let o = self.int_cast(order, LTy::I32, false);
            let mut cases = Vec::new();
            if let Some(b) = acq {
                cases.push((self.ci32(1), b));
                cases.push((self.ci32(2), b));
            }
            if let Some(b) = rel {
                cases.push((self.ci32(3), b));
            }
            if let Some(b) = acq_rel {
                cases.push((self.ci32(4), b));
            }
            cases.push((self.ci32(5), seq));
            self.f.fir.push(Op::Switch { c: o, default: mono, cases }, LTy::VOID);
            let arms = [
                (Some(mono), Ordering::Monotonic),
                (acq, Ordering::Acquire),
                (rel, Ordering::Release),
                (acq_rel, Ordering::AcqRel),
                (Some(seq), Ordering::SeqCst),
            ];
            for (b, ord) in arms {
                let Some(b) = b else { continue };
                self.f.fir.set_cur(Some(b));
                self.atomic_op(&ops, ord)?;
                self.br(cont);
            }
            self.f.fir.set_cur(Some(cont));
        }
        if ast.types.is_void(rq) {
            return Ok(None);
        }
        let d = ops.dest.expect("a result slot");
        if self.is_aggregate(rq) {
            if let Some(to) = agg {
                let n = self.size_align(rq).0;
                let c = self.memcpy(to, d.v, d.align, n, false);
                // `EmitAggregateCopy` of the result temporary.
                self.tbaa_struct_copy(c, rq);
            }
            return Ok(None);
        }
        let ty = self.mty(rq);
        Ok(Some(self.load_scalar(Addr { ty, ..d }, rq)))
    }

    /// clang `EmitAtomicOp` with ordering `ord`.
    fn atomic_op(&mut self, ops: &AtomicOps, ord: Ordering) -> R<()> {
        let mem = amem(ops.ptr.align, ops.volatile, ord, SyncScope::SYSTEM);
        let ty = ops.ptr.ty;
        match ops.form {
            Form::C11CmpXchg => self.cmpxchg_failure_set(ops, at::is_c11_weak(ops.id), ord),
            Form::GnuCmpXchg => {
                let w = ops.weak.expect("weak operand");
                if let Some((_, x, _)) = self.cint(w) {
                    self.cmpxchg_failure_set(ops, x != 0, ord);
                } else {
                    let strong = self.block_at_end();
                    let weak = self.block_at_end();
                    let cont = self.block_at_end();
                    let f = V::C(self.m.const_bool(false));
                    self.f.fir.push(
                        Op::Switch { c: w, default: weak, cases: vec![(f, strong)] },
                        LTy::VOID,
                    );
                    self.f.fir.set_cur(Some(strong));
                    self.cmpxchg_failure_set(ops, false, ord);
                    self.br(cont);
                    self.f.fir.set_cur(Some(weak));
                    self.cmpxchg_failure_set(ops, true, ord);
                    self.br(cont);
                    self.f.fir.set_cur(Some(cont));
                }
            }
            Form::Load | Form::LoadCopy => {
                let v = self.f.fir.push(Op::Load { ty, ptr: ops.ptr.v, mem }, ty);
                let d = ops.dest.expect("load destination");
                self.store(v, d.v, d.align, false);
            }
            Form::Copy => {
                let s = ops.val1.expect("stored value");
                let v = self.load(s.ty, s.v, s.align, false);
                self.f.fir.push(Op::Store { val: v, ptr: ops.ptr.v, mem }, LTy::VOID);
            }
            Form::Init => {}
            Form::Xchg | Form::GnuXchg | Form::Arithmetic => {
                let fp = self.ast.types.is_floating(ops.value_q);
                let signed = self.is_signed(ops.value_q);
                let (op, post) = match at::rmw(ops.id) {
                    None => (RmwOp::Xchg, None),
                    Some((r, post)) => {
                        let op = match r {
                            Rmw::Add if fp => RmwOp::FAdd,
                            Rmw::Add => RmwOp::Add,
                            Rmw::Sub if fp => RmwOp::FSub,
                            Rmw::Sub => RmwOp::Sub,
                            Rmw::And => RmwOp::And,
                            Rmw::Or => RmwOp::Or,
                            Rmw::Xor => RmwOp::Xor,
                            Rmw::Nand => RmwOp::Nand,
                            Rmw::Max if fp => RmwOp::FMax,
                            Rmw::Max if signed => RmwOp::Max,
                            Rmw::Max => RmwOp::UMax,
                            Rmw::Min if fp => RmwOp::FMin,
                            Rmw::Min if signed => RmwOp::Min,
                            Rmw::Min => RmwOp::UMin,
                        };
                        (op, post.then_some(r))
                    }
                };
                let s = ops.val1.expect("operand");
                let v = self.load(s.ty, s.v, s.align, false);
                let old = self.f.fir.push(Op::AtomicRmw { op, ptr: ops.ptr.v, val: v, mem }, s.ty);
                let res = match post {
                    None => old,
                    Some(Rmw::Max | Rmw::Min) if fp => {
                        // Deviation: clang 19 emits an (invalid) `icmp`
                        // here; the new value is what `atomicrmw fmin` /
                        // `fmax` stored, `llvm.minnum` / `llvm.maxnum`.
                        let name =
                            if post == Some(Rmw::Max) { "llvm.maxnum" } else { "llvm.minnum" };
                        let f = kiln_llvm::intrinsics::fp_op(&mut self.m, name, s.ty, 2);
                        self.intrinsic_call(f, vec![old, v])
                    }
                    Some(Rmw::Max | Rmw::Min) => {
                        let max = post == Some(Rmw::Max);
                        let p = match (max, signed) {
                            (true, true) => IntPredicate::Sgt,
                            (true, false) => IntPredicate::Ugt,
                            (false, true) => IntPredicate::Slt,
                            (false, false) => IntPredicate::Ult,
                        };
                        let c = self.icmp(p, old, v);
                        self.select(c, old, v)
                    }
                    Some(r) => {
                        let b = match r {
                            Rmw::Add if fp => BinOp::FAdd,
                            Rmw::Add => BinOp::Add,
                            Rmw::Sub if fp => BinOp::FSub,
                            Rmw::Sub => BinOp::Sub,
                            Rmw::Or => BinOp::Or,
                            Rmw::Xor => BinOp::Xor,
                            _ => BinOp::And,
                        };
                        let x = self.bin(b, Flags::NONE, old, v);
                        if r == Rmw::Nand {
                            let m = V::C(self.m.const_int(s.ty, -1));
                            self.bin(BinOp::Xor, Flags::NONE, x, m)
                        } else {
                            x
                        }
                    }
                };
                let d = ops.dest.expect("result slot");
                self.store(res, d.v, d.align, false);
            }
        }
        Ok(())
    }

    /// clang `emitAtomicCmpXchgFailureSet`.
    fn cmpxchg_failure_set(&mut self, ops: &AtomicOps, weak: bool, succ: Ordering) {
        let fo = ops.order_fail.expect("failure order");
        if let Some((_, x, w)) = self.cint(fo) {
            let fail = match crate::func::sext(x, w) {
                1 | 2 => Ordering::Acquire,
                5 => Ordering::SeqCst,
                _ => Ordering::Monotonic,
            };
            self.emit_cmpxchg(ops, weak, succ, fail);
            return;
        }
        let mono = self.block_at_end();
        let acq = self.block_at_end();
        let seq = self.block_at_end();
        let cont = self.block_at_end();
        let cases = vec![(self.ci32(1), acq), (self.ci32(2), acq), (self.ci32(5), seq)];
        self.f.fir.push(Op::Switch { c: fo, default: mono, cases }, LTy::VOID);
        for (b, fail) in
            [(mono, Ordering::Monotonic), (acq, Ordering::Acquire), (seq, Ordering::SeqCst)]
        {
            self.f.fir.set_cur(Some(b));
            self.emit_cmpxchg(ops, weak, succ, fail);
            self.br(cont);
        }
        self.f.fir.set_cur(Some(cont));
    }

    /// clang `emitAtomicCmpXchg`.
    fn emit_cmpxchg(&mut self, ops: &AtomicOps, weak: bool, success: Ordering, failure: Ordering) {
        let expected = ops.val1.expect("expected");
        let desired = ops.val2.expect("desired");
        let old_v = self.load(expected.ty, expected.v, expected.align, false);
        let new_v = self.load(desired.ty, desired.v, desired.align, false);
        let cx_opts = CmpXchgOpts {
            align: ops.ptr.align,
            success,
            failure,
            weak,
            volatile: ops.volatile,
            scope: SyncScope::SYSTEM,
        };
        let (old, ok) = self.cmpxchg_pair(ops.ptr.v, old_v, new_v, expected.ty, cx_opts);
        let store_expected = self.block_at_end();
        let cont = self.block_at_end();
        self.cond_br(ok, cont, store_expected);
        self.f.fir.set_cur(Some(store_expected));
        if let Some((orig, align, n)) = ops.expected_back {
            // C11 7.17.7.4p2: a failed compare-exchange loads the object's
            // value into `*expected` itself, not only into the atomic-sized
            // copy (clang 22; clang 19 wrote the copy only).
            let t = self.f.fir.alloca(expected.ty, expected.align);
            self.store(old, t, expected.align, false);
            self.memcpy(Addr { v: orig, ty: expected.ty, align }, t, expected.align, n, false);
        } else {
            self.store(old, expected.v, expected.align, false);
        }
        self.br(cont);
        self.f.fir.set_cur(Some(cont));
        let r = ops.dest.expect("cmpxchg.bool");
        self.store_scalar(ok, r, QualType::BOOL);
    }

    /// `cmpxchg` and its two `extractvalue`s.
    fn cmpxchg_pair(&mut self, ptr: V, cmp: V, new: V, ty: LTy, opts: CmpXchgOpts) -> (V, V) {
        let st = self.m.types.struct_literal(&[ty, LTy::I1], false);
        let pair = self.f.fir.push(Op::CmpXchg { ptr, cmp, new, opts }, st);
        let old = self.f.fir.push(Op::ExtractValue { agg: pair, idx: 0 }, ty);
        let ok = self.f.fir.push(Op::ExtractValue { agg: pair, idx: 1 }, LTy::I1);
        (old, ok)
    }

    // ---- _Atomic lvalues -----------------------------------------------

    /// The type an `_Atomic` access of value type `t` uses (clang 19
    /// `AtomicInfo::shouldCastToInt`): integers, pointers and IEEE floats
    /// keep theirs (floats not for `cmpxchg`), the rest go through `iN`.
    fn atomic_access_ty(&mut self, t: LTy, size: u64, cmpxchg: bool) -> LTy {
        let keep = (self.m.types.int_width(t).is_some_and(|w| w != 1))
            || t == LTy::PTR
            || (!cmpxchg && self.m.types.float_width(t).is_some_and(|w| w != 80));
        if keep { t } else { self.m.types.int(u32::try_from(size * 8).unwrap_or(8)) }
    }

    /// clang `AtomicInfo::shouldUseLibcall`: the target has no lock-free
    /// instruction for an `_Atomic` object of type `q`.
    /// clang `EmitAtomicExpr`'s `-Watomic-alignment` warnings at the
    /// start of `e`: the pointer's alignment is not a multiple of the size,
    /// and the size exceeds the target's maximum inline atomic width.
    fn atomic_alignment_warnings(&mut self, e: ExprId, size: u64, align: u64) {
        let span = self.loc_of(e).map_or(Span::INVALID, Span::empty);
        if !align.is_multiple_of(size) {
            let d = Diag::new(DiagId::cg_atomic_misaligned, span).arg(size).arg(align);
            self.diags.push(d);
        }
        let max = u64::from(self.target.types.max_atomic_inline_bytes);
        if size > max {
            // clang 22 dropped clang 19's doubled space before `bytes)`.
            let id = if self.opts.clang_compat >= 22 {
                DiagId::cg_atomic_oversized
            } else {
                DiagId::cg_atomic_oversized_19
            };
            self.diags.push(Diag::new(id, span).arg(size).arg(max));
        }
    }

    fn atomic_lv_libcall(&mut self, q: QualType) -> bool {
        let (size, align) = self.size_align(q);
        size > u64::from(self.target.types.max_atomic_inline_bytes) || align < size
    }

    /// A generic `__atomic_*` library call (`emitAtomicLibcall`): every
    /// argument `noundef`, the callee `nounwind willreturn`; `bool_ret` for
    /// `__atomic_compare_exchange`, whose `bool` result is returned.
    fn atomic_libcall(&mut self, name: &str, args: Vec<V>, params: &[LTy], bool_ret: bool) -> V {
        let fa = [Attr::Flag(AttrKind::NoUnwind), Attr::Flag(AttrKind::WillReturn)];
        let rty = if bool_ret { LTy::I1 } else { LTy::VOID };
        let f = self.rt_fn(name, rty, params, &fa);
        let fn_ty = self.m.func(f).ty();
        let nu = self.m.attr_set(&[Attr::Flag(AttrKind::NoUndef)]);
        let ret = if !bool_ret {
            AttrList::default().ret
        } else if self.is_x86() {
            self.m.attr_set(&[Attr::Flag(AttrKind::NoUndef), Attr::Flag(AttrKind::ZExt)])
        } else {
            nu
        };
        let opts = CallOpts {
            attrs: AttrList { ret, params: vec![nu; args.len()], ..AttrList::default() },
            ..CallOpts::default()
        };
        let r = self.f.fir.push(
            Op::Call { fn_ty, callee: V::C(Value::func(f)), args, opts: Box::new(opts) },
            rty,
        );
        self.f.calls.push((Fir::last_inst(r), Some(f), true));
        r
    }

    /// The library call of an `AtomicExpr` whose size is not a power of
    /// two or above 16 bytes (clang 19 `EmitAtomicExpr`'s `UseLibcall`):
    /// the generic `size_t`-first libatomic functions, the memory orders
    /// passed as they are.
    fn atomic_expr_libcall(&mut self, e: ExprId, ops: &AtomicOps, size: u64, order: V) -> R<V> {
        let n = self.i64c(i128::from(size));
        let mut args = vec![n, ops.ptr.v];
        let addr = |a: Option<Addr>| a.map(|a| a.v).map_or_else(|| err(None, "malformed"), Ok);
        let (name, cmpxchg) = match ops.form {
            Form::C11CmpXchg | Form::GnuCmpXchg => {
                args.push(addr(ops.val1)?);
                args.push(addr(ops.val2)?);
                args.push(order);
                args.push(ops.order_fail.expect("failure order"));
                ("__atomic_compare_exchange", true)
            }
            Form::Xchg | Form::GnuXchg => {
                args.push(addr(ops.val1)?);
                args.push(addr(ops.dest)?);
                args.push(order);
                ("__atomic_exchange", false)
            }
            Form::Copy => {
                args.push(addr(ops.val1)?);
                args.push(order);
                ("__atomic_store", false)
            }
            Form::Load | Form::LoadCopy => {
                args.push(addr(ops.dest)?);
                args.push(order);
                ("__atomic_load", false)
            }
            _ => return err(self.loc_of(e), format!("atomic operation of {size} bytes")),
        };
        let params: Vec<LTy> = (0..args.len())
            .map(|i| match i {
                0 => LTy::I64,
                i if i + if cmpxchg { 2 } else { 1 } >= args.len() => LTy::I32,
                _ => LTy::PTR,
            })
            .collect();
        Ok(self.atomic_libcall(name, args, &params, cmpxchg))
    }

    /// `__atomic_load` / `__atomic_store (size, obj, tmp, seq_cst)`.
    fn atomic_lv_libcall_op(&mut self, store: bool, lv: LV, tmp: V) {
        let size = self.size_align(lv.q).0;
        let n = self.i64c(i128::from(size));
        let o = self.ci32(5);
        let name = if store { "__atomic_store" } else { "__atomic_load" };
        let params = [LTy::I64, LTy::PTR, LTy::PTR, LTy::I32];
        self.atomic_libcall(name, vec![n, lv.addr.v, tmp, o], &params, false);
    }

    /// clang `EmitAtomicLoad` of the `_Atomic` lvalue `lv` (`seq_cst`).
    pub(crate) fn atomic_load_lv(&mut self, lv: LV, vq: QualType) -> V {
        if self.atomic_lv_libcall(lv.q) {
            let t = self.temp(lv.q);
            self.atomic_lv_libcall_op(false, lv, t.v);
            let ty = self.mty(vq);
            return self.load_scalar(Addr { ty, ..t }, vq);
        }
        let vol = self.is_volatile(lv.q);
        let size = self.size_align(lv.q).0;
        let ty = self.mty(vq);
        let at = self.atomic_access_ty(ty, size, false);
        let mem = amem(lv.addr.align, vol, Ordering::SeqCst, SyncScope::SYSTEM);
        let v = self.f.fir.push(Op::Load { ty: at, ptr: lv.addr.v, mem }, at);
        // `EmitAtomicLoadOp` decorates the load with the lvalue's tag.
        self.tbaa_mark(v, lv.q, lv.tb);
        if at != ty {
            // Through memory (`ConvertIntToValueOrAtomic`).
            let t = self.temp(lv.q);
            self.store(v, t.v, t.align, false);
            return self.load_scalar(Addr { ty, ..t }, vq);
        }
        if self.has_bool_rep(vq) { self.cast(CastOp::Trunc, v, LTy::I1) } else { v }
    }

    /// Whether `q` is `_Atomic(T)` with `T` an aggregate (clang
    /// `getEvaluationKind` is `TEK_Aggregate`).
    pub(crate) fn is_atomic_aggregate(&self, q: QualType) -> bool {
        self.ast.types.as_atomic(q).is_some_and(|v| self.is_aggregate(v))
    }

    /// clang `EmitAtomicLoad` of the aggregate `_Atomic` lvalue `lv`
    /// into the slot `dest` (`ConvertToValueOrAtomic` slams the integer
    /// into the slot; the libcall loads straight into it).
    pub(crate) fn atomic_agg_load(&mut self, lv: LV, dest: Addr) {
        if self.atomic_lv_libcall(lv.q) {
            self.atomic_lv_libcall_op(false, lv, dest.v);
            return;
        }
        let vol = self.is_volatile(lv.q);
        let size = self.size_align(lv.q).0;
        let at = self.m.types.int(u32::try_from(size * 8).unwrap_or(8));
        let mem = amem(lv.addr.align, vol, Ordering::SeqCst, SyncScope::SYSTEM);
        let v = self.f.fir.push(Op::Load { ty: at, ptr: lv.addr.v, mem }, at);
        self.tbaa_mark(v, lv.q, lv.tb);
        let slot_vol = std::mem::take(&mut self.f.slot_volatile);
        self.store(v, dest.v, dest.align, slot_vol);
    }

    /// clang `EmitAtomicStore` of the aggregate r-value in `src` (a slot
    /// of the `_Atomic` type) to the `_Atomic` lvalue `lv`.
    pub(crate) fn atomic_agg_store(&mut self, src: Addr, lv: LV) {
        if self.atomic_lv_libcall(lv.q) {
            self.atomic_lv_libcall_op(true, lv, src.v);
            return;
        }
        let vol = self.is_volatile(lv.q);
        let size = self.size_align(lv.q).0;
        let at = self.m.types.int(u32::try_from(size * 8).unwrap_or(8));
        let v = self.load(at, src.v, src.align, false);
        let mem = amem(lv.addr.align, vol, Ordering::SeqCst, SyncScope::SYSTEM);
        let st = self.f.fir.push(Op::Store { val: v, ptr: lv.addr.v, mem }, LTy::VOID);
        self.tbaa_mark(st, lv.q, lv.tb);
    }

    /// clang `AggExprEmitter::VisitCastExpr` for `NonAtomicToAtomic` and
    /// `AtomicToNonAtomic` casts of aggregates `e` into `dest`.
    pub(crate) fn agg_atomic_cast(&mut self, e: ExprId, dest: Addr, zeroed: bool) -> R<()> {
        let ast = self.ast;
        let to_atomic = ast.exprs.cast_kind(e) == CastKind::NonAtomicToAtomic;
        let s = ast.exprs.operand(e);
        let (aq, vq) = if to_atomic {
            (ast.exprs.get(e).ty(), ast.exprs.get(s).ty())
        } else {
            (ast.exprs.get(s).ty(), ast.exprs.get(e).ty())
        };
        let asize = self.size_align(aq).0;
        if self.size_align(vq).0 == asize {
            return self.emit_agg(s, dest, zeroed);
        }
        // `findPeephole`: the reverse cast under no-op casts and parens.
        let peep =
            if to_atomic { CastKind::AtomicToNonAtomic } else { CastKind::NonAtomicToAtomic };
        let mut op = s;
        loop {
            let n = ast.exprs.get(op);
            if n.kind() == ExprKind::ParenExpr
                || (n.kind().is_cast() && ast.exprs.cast_kind(op) == CastKind::NoOp)
            {
                op = ast.exprs.operand(op);
            } else {
                break;
            }
        }
        if ast.exprs.get(op).kind().is_cast() && ast.exprs.cast_kind(op) == peep {
            return self.emit_agg(ast.exprs.operand(op), dest, zeroed);
        }
        if to_atomic {
            if !zeroed {
                self.memset(dest, 0, asize, false);
            }
            let aty = self.mty(aq);
            let v = self.struct_gep(aty, dest.v, 0);
            let vty = self.mty(vq);
            return self.emit_agg(s, Addr { v, ty: vty, align: dest.align }, true);
        }
        let t = self.temp(aq);
        self.emit_agg(s, t, false)?;
        let aty = self.mty(aq);
        let v = self.struct_gep(aty, t.v, 0);
        let n = self.size_align(vq).0;
        self.memcpy(dest, v, t.align, n, false);
        Ok(())
    }

    /// clang `EmitAtomicInit`: `init` stored into the `_Atomic` object
    /// `lv` (not atomically). The object is zeroed first when it has
    /// padding or its value does not fill it (`emitMemSetZeroIfNecessary`;
    /// an aggregate from an `_Atomic` initializer is copied whole), and the
    /// value goes into the padded struct's first field (`projectValue`).
    pub(crate) fn atomic_init(&mut self, init: ExprId, lv: LV) -> R<()> {
        let ast = self.ast;
        let vq = ast.types.as_atomic(lv.q).unwrap_or(lv.q);
        let asize = self.size_align(lv.q).0;
        let padded = self.size_align(vq).0 != asize;
        let vty = self.mty(vq);
        let project = |cg: &mut Self| {
            let v = if padded { cg.struct_gep(lv.addr.ty, lv.addr.v, 0) } else { lv.addr.v };
            Addr { v, ty: vty, align: lv.addr.align }
        };
        if self.is_aggregate(vq) {
            if ast.types.as_atomic(ast.exprs.get(init).ty()).is_some() {
                return self.emit_agg(init, lv.addr, false);
            }
            if padded {
                self.memset(lv.addr, 0, asize, false);
            }
            let dest = project(self);
            return self.emit_agg(init, dest, padded);
        }
        let v = match self.folds.get(init) {
            Some(c) => V::C(self.const_scalar_value(c, vq)?),
            None => self.emit_scalar(init)?,
        };
        if padded || self.mem_bits(lv.q) != asize * 8 {
            self.memset(lv.addr, 0, asize, false);
        }
        let dest = project(self);
        // `emitCopyIntoMemory` stores through the `_Atomic` lvalue's tag.
        let st = self.store_scalar_tb(v, dest, vq, Tb::None);
        self.tbaa_mark(st, lv.q, lv.tb);
        Ok(())
    }

    /// clang `AtomicInfo::materializeRValue`: `v` in a temporary of the
    /// `_Atomic` type, its padding zeroed first.
    fn materialize_atomic(&mut self, v: V, lv: LV, vq: QualType) -> V {
        let t = self.temp(lv.q);
        let (full, _) = self.size_align(lv.q);
        let vty = self.mty(vq);
        let bits = self.m.types.float_width(vty).map(u64::from);
        if bits.is_some_and(|b| b < full * 8) || self.size_align(vq).0 < full {
            self.memset(t, 0, full, false);
        }
        self.store_scalar(v, Addr { ty: vty, ..t }, vq);
        t.v
    }

    /// clang `EmitAtomicStore` of `v` to the `_Atomic` lvalue `lv`
    /// (`seq_cst`; initializations are plain stores, not this).
    pub(crate) fn atomic_store_lv(&mut self, v: V, lv: LV, vq: QualType) {
        if self.atomic_lv_libcall(lv.q) {
            let t = self.materialize_atomic(v, lv, vq);
            self.atomic_lv_libcall_op(true, lv, t);
            return;
        }
        let vol = self.is_volatile(lv.q);
        let size = self.size_align(lv.q).0;
        let ty = self.mty(vq);
        let v = if self.has_bool_rep(vq) && self.f.fir.ty(&self.m, v) == LTy::I1 {
            self.cast(CastOp::ZExt, v, ty)
        } else {
            v
        };
        let at = self.atomic_access_ty(ty, size, false);
        let v = if at == ty {
            v
        } else {
            let t = self.temp(lv.q);
            self.store(v, t.v, t.align, false);
            self.load(at, t.v, t.align, false)
        };
        let mem = amem(lv.addr.align, vol, Ordering::SeqCst, SyncScope::SYSTEM);
        let st = self.f.fir.push(Op::Store { val: v, ptr: lv.addr.v, mem }, LTy::VOID);
        // `EmitAtomicStore` decorates the store with the lvalue's tag.
        self.tbaa_mark(st, lv.q, lv.tb);
    }

    /// `seq_cst` `atomicrmw` on an `_Atomic` lvalue (never volatile, as
    /// clang's increment and compound-assignment fast paths).
    fn atomic_rmw_lv(&mut self, op: RmwOp, lv: LV, amt: V) -> V {
        let ty = self.f.fir.ty(&self.m, amt);
        let mem = amem(lv.addr.align, false, Ordering::SeqCst, SyncScope::SYSTEM);
        self.f.fir.push(Op::AtomicRmw { op, ptr: lv.addr.v, val: amt, mem }, ty)
    }

    /// clang `EmitAtomicCompareExchange` (`seq_cst`, strong) of the
    /// `_Atomic` lvalue `lv`: the old value and the success flag.
    fn atomic_cmpxchg_lv(&mut self, lv: LV, vq: QualType, exp: V, des: V) -> R<(V, V)> {
        if self.atomic_lv_libcall(lv.q) {
            // `EmitAtomicCompareExchangeLibcall`; the old value is read
            // back from the expected buffer.
            let et = self.materialize_atomic(exp, lv, vq);
            let dt = self.materialize_atomic(des, lv, vq);
            let size = self.size_align(lv.q).0;
            let n = self.i64c(i128::from(size));
            let o = self.ci32(5);
            let params = [LTy::I64, LTy::PTR, LTy::PTR, LTy::PTR, LTy::I32, LTy::I32];
            let args = vec![n, lv.addr.v, et, dt, o, o];
            let ok = self.atomic_libcall("__atomic_compare_exchange", args, &params, true);
            let (_, align) = self.size_align(lv.q);
            let ty = self.mty(vq);
            let old = self.load_scalar(Addr { v: et, ty, align }, vq);
            return Ok((old, ok));
        }
        let vol = self.is_volatile(lv.q);
        let size = self.size_align(lv.q).0;
        let ty = self.f.fir.ty(&self.m, exp);
        let at = self.atomic_access_ty(ty, size, true);
        if at != ty && self.m.types.float_width(ty).is_none_or(|w| w == 80) {
            return err(None, format!("atomic compare-exchange of {vq:?} is not supported yet"));
        }
        let (exp, des) = if at == ty {
            (exp, des)
        } else {
            (self.cast(CastOp::BitCast, exp, at), self.cast(CastOp::BitCast, des, at))
        };
        let opts = CmpXchgOpts {
            align: lv.addr.align,
            success: Ordering::SeqCst,
            failure: Ordering::SeqCst,
            weak: false,
            volatile: vol,
            scope: SyncScope::SYSTEM,
        };
        let (old, ok) = self.cmpxchg_pair(lv.addr.v, exp, des, at, opts);
        let old = if at == ty { old } else { self.cast(CastOp::BitCast, old, ty) };
        // A `bool` comes back as its value and goes to memory again
        // (`ConvertIntToValueOrAtomic`, then `EmitToMemory`).
        let old = if self.has_bool_rep(vq) {
            let b = self.cast(CastOp::Trunc, old, LTy::I1);
            self.cast(CastOp::ZExt, b, ty)
        } else {
            old
        };
        Ok((old, ok))
    }

    /// The `atomic_op` loop of clang's increments and compound
    /// assignments: `op` computes the new value from the old one; returns
    /// the old value loaded first and the new value stored.
    fn atomic_update_loop(
        &mut self,
        lv: LV,
        vq: QualType,
        op: &mut dyn FnMut(&mut Self, V) -> R<V>,
    ) -> R<(V, V)> {
        let input = self.atomic_load_lv(lv, vq);
        let value = if self.has_bool_rep(vq) {
            let t = self.mty(vq);
            self.cast(CastOp::ZExt, input, t)
        } else {
            input
        };
        let start = self.f.fir.cur().expect("insertion point");
        let body = self.block_at_end();
        self.br(body);
        self.f.fir.set_cur(Some(body));
        let ty = self.f.fir.ty(&self.m, value);
        let phi = self.f.fir.push(Op::Phi { ty, inc: vec![(value, start)] }, ty);
        let new = op(self, phi)?;
        let cur = self.f.fir.cur().expect("insertion point");
        let cont = self.block_at_end();
        let (old, ok) = self.atomic_cmpxchg_lv(lv, vq, phi, new)?;
        self.f.fir.add_incoming(Fir::last_inst(phi), old, cur);
        self.cond_br(ok, cont, body);
        self.f.fir.set_cur(Some(cont));
        Ok((input, new))
    }

    /// clang `EmitScalarPrePostIncDec` of the `_Atomic` lvalue `lv`.
    pub(crate) fn atomic_inc_dec(
        &mut self,
        e: ExprId,
        lv: LV,
        vq: QualType,
        inc: bool,
        pre: bool,
    ) -> R<V> {
        let ast = self.ast;
        if inc && self.is_bool(vq) {
            // clang `EmitScalarPrePostIncDec`: pre-increment stores `true`
            // and yields it; post-increment swaps in `true` and yields the
            // old byte (its memory form, as clang does).
            let one = self.icn(LTy::I8, 1);
            if pre {
                let vol = self.is_volatile(lv.q);
                let mem = amem(lv.addr.align, vol, Ordering::SeqCst, SyncScope::SYSTEM);
                self.f.fir.push(Op::Store { val: one, ptr: lv.addr.v, mem }, LTy::VOID);
                return Ok(self.icn(LTy::I1, 1));
            }
            return Ok(self.atomic_rmw_lv(RmwOp::Xchg, lv, one));
        }
        let ty = self.mty(vq);
        if ast.types.is_integral(vq) {
            let amt = self.icn(ty, 1);
            let (op, b) = if inc { (RmwOp::Add, BinOp::Add) } else { (RmwOp::Sub, BinOp::Sub) };
            let old = self.atomic_rmw_lv(op, lv, amt);
            return Ok(if pre { self.bin(b, Flags::NONE, old, amt) } else { old });
        }
        if ast.types.is_floating(vq)
            && self.m.types.float_width(ty).is_some_and(u32::is_power_of_two)
        {
            let amt = self.fconst(ty, 1.0);
            let (op, b) = if inc { (RmwOp::FAdd, BinOp::FAdd) } else { (RmwOp::FSub, BinOp::FSub) };
            let old = self.atomic_rmw_lv(op, lv, amt);
            return Ok(if pre { self.bin(b, Flags::NONE, old, amt) } else { old });
        }
        let (input, new) = self
            .atomic_update_loop(lv, vq, &mut |cg, old| cg.inc_dec_value(e, vq, old, inc, false))?;
        Ok(if pre { new } else { input })
    }

    /// clang `EmitCompoundAssignLValue` on the `_Atomic` lvalue `lv`
    /// (`rv` already evaluated): integer `+= -= &= |= ^=` are one
    /// `atomicrmw`, the rest a compare-exchange loop.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn atomic_compound_assign(
        &mut self,
        e: ExprId,
        op: BinaryOp,
        lv: LV,
        vq: QualType,
        rv: V,
        rq: QualType,
        lhs_ty: QualType,
        result_ty: QualType,
    ) -> R<V> {
        let ast = self.ast;
        if ast.types.is_integral(vq) && !self.is_bool(vq) {
            let ops = match op {
                BinaryOp::Add => Some((RmwOp::Add, BinOp::Add)),
                BinaryOp::Sub => Some((RmwOp::Sub, BinOp::Sub)),
                BinaryOp::And => Some((RmwOp::And, BinOp::And)),
                BinaryOp::Xor => Some((RmwOp::Xor, BinOp::Xor)),
                BinaryOp::Or => Some((RmwOp::Or, BinOp::Or)),
                _ => None,
            };
            if let Some((rop, b)) = ops {
                // clang converts to the `_Atomic` type itself, which is
                // not a signed integer type (so a `short` target is
                // zero-extended in the loop below, and so on).
                let amt = self.conv(rv, rq, lv.q);
                let old = self.atomic_rmw_lv(rop, lv, amt);
                return Ok(self.bin(b, Flags::NONE, old, amt));
            }
        }
        let ptr = ast.types.is_pointer(vq);
        let (_, new) = self.atomic_update_loop(lv, vq, &mut |cg, old| {
            let lhs = if ptr { old } else { cg.conv(old, lv.q, lhs_ty) };
            let res = cg.arith(e, op, lhs, rv, lhs_ty, rq, result_ty)?;
            Ok(if ptr { res } else { cg.conv(res, result_ty, lv.q) })
        })?;
        Ok(new)
    }

    // ---- builtins ------------------------------------------------------

    /// The `__atomic_*` / `__c11_atomic_*` builtins `CGBuiltin` lowers
    /// itself (the folded lock-free queries never get here).
    pub(crate) fn emit_atomic_builtin(
        &mut self,
        b: u32,
        e: ExprId,
        args: &[ExprId],
    ) -> R<Option<V>> {
        let arg = |i: usize| args.get(i).copied().map_or_else(|| err(None, "missing argument"), Ok);
        match b {
            at::TEST_AND_SET | at::CLEAR => {
                let a0 = arg(0)?;
                let volatile = self.arg_pointee_volatile(a0);
                let (p, align) = self.ptr_align(a0)?;
                let set = b == at::TEST_AND_SET;
                let nv = self.icn(LTy::I8, i128::from(set));
                let order = self.emit_scalar(arg(1)?)?;
                let mem = |o| amem(align, volatile, o, SyncScope::SYSTEM);
                if let Some((_, o, _)) = self.cint(order) {
                    if set {
                        let ord = match o {
                            1 | 2 => Ordering::Acquire,
                            3 => Ordering::Release,
                            4 => Ordering::AcqRel,
                            5 => Ordering::SeqCst,
                            _ => Ordering::Monotonic,
                        };
                        let r = self.f.fir.push(
                            Op::AtomicRmw { op: RmwOp::Xchg, ptr: p, val: nv, mem: mem(ord) },
                            LTy::I8,
                        );
                        let z = self.icn(LTy::I8, 0);
                        return Ok(Some(self.icmp(IntPredicate::Ne, r, z)));
                    }
                    let ord = match o {
                        3 => Ordering::Release,
                        5 => Ordering::SeqCst,
                        _ => Ordering::Monotonic,
                    };
                    self.f.fir.push(Op::Store { val: nv, ptr: p, mem: mem(ord) }, LTy::VOID);
                    return Ok(None);
                }
                let cont = self.block_at_end();
                let arms: &[(Ordering, &[i128])] = if set {
                    &[
                        (Ordering::Monotonic, &[0]),
                        (Ordering::Acquire, &[1, 2]),
                        (Ordering::Release, &[3]),
                        (Ordering::AcqRel, &[4]),
                        (Ordering::SeqCst, &[5]),
                    ]
                } else {
                    &[
                        (Ordering::Monotonic, &[0]),
                        (Ordering::Release, &[3]),
                        (Ordering::SeqCst, &[5]),
                    ]
                };
                let blocks: Vec<Blk> = arms.iter().map(|_| self.block_at_end()).collect();
                let o = self.int_cast(order, LTy::I32, false);
                let mut cases = Vec::new();
                for (&(_, vals), &blk) in arms.iter().zip(&blocks) {
                    for &x in vals {
                        cases.push((self.ci32(x), blk));
                    }
                }
                self.f.fir.push(Op::Switch { c: o, default: blocks[0], cases }, LTy::VOID);
                let phi = if set {
                    self.f.fir.set_cur(Some(cont));
                    Some(self.f.fir.push(Op::Phi { ty: LTy::I8, inc: Vec::new() }, LTy::I8))
                } else {
                    None
                };
                for (&(ord, _), &blk) in arms.iter().zip(&blocks) {
                    self.f.fir.set_cur(Some(blk));
                    if set {
                        let r = self.f.fir.push(
                            Op::AtomicRmw { op: RmwOp::Xchg, ptr: p, val: nv, mem: mem(ord) },
                            LTy::I8,
                        );
                        self.f.fir.add_incoming(Fir::last_inst(phi.expect("phi")), r, blk);
                    } else {
                        self.f.fir.push(Op::Store { val: nv, ptr: p, mem: mem(ord) }, LTy::VOID);
                    }
                    self.br(cont);
                }
                self.f.fir.set_cur(Some(cont));
                Ok(phi.map(|r| {
                    let z = self.icn(LTy::I8, 0);
                    self.icmp(IntPredicate::Ne, r, z)
                }))
            }
            at::THREAD_FENCE | at::SIGNAL_FENCE | at::C11_THREAD_FENCE | at::C11_SIGNAL_FENCE => {
                let scope = if matches!(b, at::SIGNAL_FENCE | at::C11_SIGNAL_FENCE) {
                    SyncScope::SINGLE_THREAD
                } else {
                    SyncScope::SYSTEM
                };
                let order = self.emit_scalar(arg(0)?)?;
                if let Some((_, o, _)) = self.cint(order) {
                    let ord = match o {
                        1 | 2 => Some(Ordering::Acquire),
                        3 => Some(Ordering::Release),
                        4 => Some(Ordering::AcqRel),
                        5 => Some(Ordering::SeqCst),
                        _ => None,
                    };
                    if let Some(ordering) = ord {
                        self.f.fir.push(Op::Fence { ordering, scope }, LTy::VOID);
                    }
                    return Ok(None);
                }
                let arms = [
                    (Ordering::Acquire, &[1, 2][..]),
                    (Ordering::Release, &[3][..]),
                    (Ordering::AcqRel, &[4][..]),
                    (Ordering::SeqCst, &[5][..]),
                ];
                let blocks: Vec<Blk> = arms.iter().map(|_| self.block_at_end()).collect();
                let cont = self.block_at_end();
                let o = self.int_cast(order, LTy::I32, false);
                let mut cases = Vec::new();
                for (&(_, vals), &blk) in arms.iter().zip(&blocks) {
                    for &x in vals {
                        cases.push((self.ci32(x), blk));
                    }
                }
                self.f.fir.push(Op::Switch { c: o, default: cont, cases }, LTy::VOID);
                for (&(ordering, _), &blk) in arms.iter().zip(&blocks) {
                    self.f.fir.set_cur(Some(blk));
                    self.f.fir.push(Op::Fence { ordering, scope }, LTy::VOID);
                    self.br(cont);
                }
                self.f.fir.set_cur(Some(cont));
                Ok(None)
            }
            at::IS_LOCK_FREE | at::C11_IS_LOCK_FREE => {
                // `bool __atomic_is_lock_free(size_t, void *)`; the C11
                // form passes a null pointer (an `_Atomic` object is
                // always suitably aligned).
                let n = self.emit_scalar(arg(0)?)?;
                let p = if b == at::IS_LOCK_FREE {
                    self.emit_scalar(arg(1)?)?
                } else {
                    V::C(self.m.const_null(LTy::PTR))
                };
                let f = self.rt_fn("__atomic_is_lock_free", LTy::I1, &[LTy::I64, LTy::PTR], &[]);
                let fn_ty = self.m.func(f).ty();
                let nu = self.m.attr_set(&[Attr::Flag(AttrKind::NoUndef)]);
                let ret = if self.is_x86() {
                    self.m.attr_set(&[Attr::Flag(AttrKind::NoUndef), Attr::Flag(AttrKind::ZExt)])
                } else {
                    nu
                };
                let opts = CallOpts {
                    attrs: AttrList { ret, params: vec![nu, nu], ..AttrList::default() },
                    ..CallOpts::default()
                };
                let dest = self.invoke_dest();
                let r = self.call_or_invoke_to(
                    fn_ty,
                    V::C(Value::func(f)),
                    vec![n, p],
                    opts,
                    LTy::I1,
                    dest,
                );
                self.f.calls.push((Fir::last_inst(r), Some(f), false));
                Ok(Some(r))
            }
            _ => {
                let name = kiln_ast::decl::builtin::info(b).map_or("?", |i| i.name);
                err(self.loc_of(e), format!("code generation for {name} is not supported yet"))
            }
        }
    }

    /// The GCC `__sync` builtins (clang `EmitBuiltinExpr` with
    /// `MakeBinaryAtomicValue`, `EmitBinaryAtomicPost` and
    /// `MakeAtomicCmpXchgValue`): call `e` of the `_N` form or extension
    /// `b` with `args` as sema converted them. Every operation is
    /// `seq_cst` on an `iN` of the value's size (a pointer through
    /// `ptrtoint`/`inttoptr`, a `bool` through its `i8`), never volatile:
    /// `fetch_and_<op>` is an `atomicrmw` returning the old value,
    /// `<op>_and_fetch` applies the operation again to the old value
    /// (`nand`: `and` then `xor -1`), the compare-and-swaps a `cmpxchg`
    /// (the old value, or the success bit), `lock_test_and_set` and
    /// `swap` an `xchg`; `lock_release` is a `release` store of 0 and
    /// `__sync_synchronize` a `seq_cst` fence. A pointer less aligned than
    /// the value's size warns (`-Wsync-alignment`) and the access takes
    /// the natural alignment.
    pub(crate) fn emit_sync_builtin(&mut self, b: u32, e: ExprId, args: &[ExprId]) -> R<Option<V>> {
        use kiln_ast::decl::builtin::sync::{self as sy, Op as SOp};
        let arg = |i: usize| args.get(i).copied().map_or_else(|| err(None, "missing argument"), Ok);
        if b == sy::SYNCHRONIZE {
            let fence = Op::Fence { ordering: Ordering::SeqCst, scope: SyncScope::SYSTEM };
            self.f.fir.push(fence, LTy::VOID);
            return Ok(None);
        }
        let op = match b {
            sy::FETCH_AND_MAX => SOp::Fetch(Rmw::Max),
            sy::FETCH_AND_MIN => SOp::Fetch(Rmw::Min),
            sy::FETCH_AND_UMAX | sy::FETCH_AND_UMIN => SOp::Fetch(Rmw::Max),
            _ => match sy::op(b) {
                Some((op, _)) => op,
                None => return err(self.loc_of(e), "unknown __sync builtin"),
            },
        };
        let a0 = arg(0)?;
        let ast = self.ast;
        let pq = ast.exprs.get(a0).ty();
        let Some(vq) = ast.types.pointee(pq).map(QualType::without_quals) else {
            return err(self.loc_of(e), "__sync builtin without a pointer");
        };
        let (p, align) = self.ptr_align(a0)?;
        let bytes = self.size_align(vq).0.max(1);
        let align = if align.is_multiple_of(bytes) {
            align
        } else {
            // At the call's start (clang `getBeginLoc`): the callee's.
            let start = self.ast.exprs.callee(e);
            let span = self.loc_of(start).map_or(Span::INVALID, Span::empty);
            self.diags.push(Diag::new(DiagId::cg_sync_op_misaligned, span));
            bytes
        };
        let ity = self.m.types.int(u32::try_from(bytes * 8).unwrap_or(8));
        let mem = |o| amem(align, false, o, SyncScope::SYSTEM);
        if op == SOp::LockRelease {
            let z = self.icn(ity, 0);
            self.f.fir.push(Op::Store { val: z, ptr: p, mem: mem(Ordering::Release) }, LTy::VOID);
            return Ok(None);
        }
        // clang `EmitToInt` / `EmitFromInt`.
        let to_int = |cg: &mut Self, v: V| {
            let v = cg.emit_to_memory(v, vq, ity);
            if cg.f.fir.ty(&cg.m, v) == LTy::PTR { cg.cast(CastOp::PtrToInt, v, ity) } else { v }
        };
        let from_int = |cg: &mut Self, v: V, vt: LTy| {
            let v = cg.emit_from_memory(v, vq);
            if vt == LTy::PTR { cg.cast(CastOp::IntToPtr, v, vt) } else { v }
        };
        if matches!(op, SOp::ValCas | SOp::BoolCas) {
            let c = self.emit_scalar(arg(1)?)?;
            let vt = self.f.fir.ty(&self.m, c);
            let c = to_int(self, c);
            let n = self.emit_scalar(arg(2)?)?;
            let n = to_int(self, n);
            let opts = CmpXchgOpts {
                align,
                success: Ordering::SeqCst,
                failure: Ordering::SeqCst,
                weak: false,
                volatile: false,
                scope: SyncScope::SYSTEM,
            };
            // clang `MakeAtomicCmpXchgValue`: only the field it returns.
            let st = self.m.types.struct_literal(&[ity, LTy::I1], false);
            let pair = self.f.fir.push(Op::CmpXchg { ptr: p, cmp: c, new: n, opts }, st);
            if op == SOp::BoolCas {
                return Ok(Some(self.f.fir.push(Op::ExtractValue { agg: pair, idx: 1 }, LTy::I1)));
            }
            let old = self.f.fir.push(Op::ExtractValue { agg: pair, idx: 0 }, ity);
            return Ok(Some(from_int(self, old, vt)));
        }
        let v = self.emit_scalar(arg(1)?)?;
        let vt = self.f.fir.ty(&self.m, v);
        let v = to_int(self, v);
        let (kind, post) = match op {
            SOp::Fetch(r) => (r, None),
            SOp::Post(r) => (r, Some(r)),
            _ => (Rmw::Add, None),
        };
        let rop = match (op, b) {
            (SOp::LockTestAndSet | SOp::Swap, _) => RmwOp::Xchg,
            (_, sy::FETCH_AND_UMAX) => RmwOp::UMax,
            (_, sy::FETCH_AND_UMIN) => RmwOp::UMin,
            _ => match kind {
                Rmw::Add => RmwOp::Add,
                Rmw::Sub => RmwOp::Sub,
                Rmw::And => RmwOp::And,
                Rmw::Or => RmwOp::Or,
                Rmw::Xor => RmwOp::Xor,
                Rmw::Nand => RmwOp::Nand,
                Rmw::Max => RmwOp::Max,
                Rmw::Min => RmwOp::Min,
            },
        };
        let mut r = self
            .f
            .fir
            .push(Op::AtomicRmw { op: rop, ptr: p, val: v, mem: mem(Ordering::SeqCst) }, ity);
        if let Some(post) = post {
            let bop = match post {
                Rmw::Add => BinOp::Add,
                Rmw::Sub => BinOp::Sub,
                Rmw::Or => BinOp::Or,
                Rmw::Xor => BinOp::Xor,
                Rmw::And | Rmw::Nand | Rmw::Max | Rmw::Min => BinOp::And,
            };
            r = self.bin(bop, Flags::NONE, r, v);
            if post == Rmw::Nand {
                let ones = self.icn(ity, -1);
                r = self.bin(BinOp::Xor, Flags::NONE, r, ones);
            }
        }
        Ok(Some(from_int(self, r, vt)))
    }

    /// Whether the pointer argument `a` (implicit conversions stripped, as
    /// clang's `IgnoreImpCasts`) points to a volatile object.
    fn arg_pointee_volatile(&self, a: ExprId) -> bool {
        let ast = self.ast;
        let mut a = a;
        while ast.exprs.get(a).kind() == ExprKind::ImplicitCastExpr {
            a = ast.exprs.operand(a);
        }
        ast.types.pointee(ast.exprs.get(a).ty()).is_some_and(|p| self.is_volatile(p))
    }
}
