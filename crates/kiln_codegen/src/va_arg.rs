//! `__builtin_va_arg` (clang `CodeGenFunction::EmitVAArg` and the target
//! `EmitVAArg` hooks): the AAPCS64 and `SysV` x86-64 `va_list` walks,
//! instruction for instruction as clang 19 emits them at `-O0`.

use kiln_abi::call::{ArgKind, classify_va_arg, unadjusted_align};
use kiln_abi::irtype::IrType;
use kiln_ast::ExprId;
use kiln_llvm::{BinOp, Flags, IntPredicate, TypeId as LTy};

use crate::fir::{Blk, Op, V};
use crate::func::Addr;
use crate::module::{Cg, R, err};

/// `Address::alignmentAtOffset`: the alignment of `align`-aligned memory
/// at byte offset `off`.
fn align_at(align: u64, off: u64) -> u64 {
    if off == 0 { align } else { align.min(1 << off.trailing_zeros()) }
}

impl Cg<'_> {
    /// clang `VisitVAArgExpr`: the value of `e` (a scalar), or `None`
    /// after copying an aggregate into `dest` (nothing is copied for an
    /// ignored aggregate, `dest == None`).
    pub(crate) fn emit_va_arg(&mut self, e: ExprId, dest: Option<Addr>) -> R<Option<V>> {
        let q = self.ast.exprs.get(e).ty();
        let Some(addr) = self.va_arg_addr(e)? else { return Ok(None) };
        if self.is_aggregate(q) {
            let Some(dest) = dest else { return Ok(None) };
            let size = self.size_align(q).0;
            let vol = self.is_volatile(q);
            self.agg_copy(q, dest, addr, size, vol);
            return Ok(None);
        }
        Ok(Some(self.load_scalar(addr, q)))
    }

    /// The address `__builtin_va_arg` `e` reads its value from, after
    /// advancing the `va_list`; `None` for an ignored (empty) type.
    pub(crate) fn va_arg_addr(&mut self, e: ExprId) -> R<Option<Addr>> {
        let ast = self.ast;
        let (list, _, _) = ast.exprs.va_arg(e);
        let q = ast.exprs.get(e).ty();
        let lq = ast.exprs.get(list).ty();
        let (base, base_align, rq) = if let Some(p) = ast.types.pointee(lq)
            && ast.types.is_pointer(lq)
        {
            let (v, a) = self.ptr_align(list)?;
            (v, a, p)
        } else {
            let lv = self.emit_lvalue(list)?;
            (lv.addr.v, lv.addr.align, lq)
        };
        let list_ty = self.mty(rq);
        let list = Addr { v: base, ty: list_ty, align: base_align };
        let info = classify_va_arg(&mut self.tys, q);
        if info.kind == ArgKind::Ignore {
            return Ok(None);
        }
        if self.is_x86() {
            self.x86_64_va_arg(list, q, &info.kind, info.needed_int, info.needed_sse).map(Some)
        } else {
            self.aapcs_va_arg(list, q, &info.kind).map(Some)
        }
    }

    fn i32v(&mut self, v: i128) -> V {
        V::C(self.m.const_int(LTy::I32, v))
    }

    /// `CreateStructGEP` on the `va_list` record: the field address and
    /// its alignment.
    fn va_field(&mut self, list: Addr, i: u32) -> (V, u64) {
        let off: [u64; 5] = if self.is_x86() { [0, 4, 8, 16, 0] } else { [0, 8, 16, 24, 28] };
        let p = self.struct_gep(list.ty, list.v, i);
        (p, align_at(list.align, off[i as usize]))
    }

    /// `emitRoundPointerUpToAlignment`.
    fn round_ptr_up(&mut self, p: V, align: u64) -> V {
        let a = i128::from(align);
        let k = self.i32v(a - 1);
        let p = self.gep(LTy::I8, p, vec![k], true);
        let f = kiln_llvm::intrinsics::ptrmask(&mut self.m);
        let mask = self.i64c(-a);
        self.intrinsic_call(f, vec![p, mask])
    }

    /// `emitMergePHI`: the address coming from either block.
    fn merge_phi(&mut self, a: Addr, ab: Blk, b: Addr, bb: Blk) -> Addr {
        let v =
            self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(a.v, ab), (b.v, bb)] }, LTy::PTR);
        Addr { v, ty: a.ty, align: a.align.min(b.align) }
    }

    /// clang `AArch64ABIInfo::EmitAAPCSVAArg` (little-endian).
    fn aapcs_va_arg(&mut self, list: Addr, q: kiln_ast::QualType, kind: &ArgKind) -> R<Addr> {
        let indirect = matches!(kind, ArgKind::Indirect { .. });
        let mut base_ty = match kind {
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => coerce.clone(),
            _ => IrType::Ptr,
        };
        let mut num_regs = 1;
        if let IrType::Array(el, n) = base_ty {
            base_ty = *el;
            num_regs = n;
        }
        let fpr = base_ty.is_fp() || matches!(base_ty, IrType::Vector(..));
        // `TyAlign` is the unadjusted (natural) alignment, as in the
        // parameter classification (AAPCS64 C.10; spec/abi.md §8.12): a
        // class's own `alignas(16)` neither rounds the register offset up
        // to an even pair nor the stack pointer up to 16.
        let ty_size = self.size_align(q).0;
        let ty_align = unadjusted_align(&mut self.tys, q);
        let mem_ty = self.mty(q);

        let maybe_reg = self.f.fir.create_block();
        let in_reg = self.f.fir.create_block();
        let on_stack = self.f.fir.create_block();
        let end = self.f.fir.create_block();

        let reg_size = if indirect { 8 } else { ty_size };
        let (offs_i, top_i, reg_size) =
            if fpr { (4, 2, 16 * num_regs) } else { (3, 1, reg_size.next_multiple_of(8)) };
        let (offs_p, offs_align) = self.va_field(list, offs_i);
        let mut reg_offs = self.load(LTy::I32, offs_p, offs_align, false);
        let zero = self.i32v(0);
        let using_stack = self.icmp(IntPredicate::Sge, reg_offs, zero);
        self.cond_br(using_stack, on_stack, maybe_reg);

        self.emit_block(maybe_reg, false);
        if !fpr && !indirect && ty_align > 8 {
            let a = i128::from(ty_align);
            let k = self.i32v(a - 1);
            reg_offs = self.bin(BinOp::Add, Flags::NONE, reg_offs, k);
            let m = self.i32v(-a);
            reg_offs = self.bin(BinOp::And, Flags::NONE, reg_offs, m);
        }
        let rs = self.i32v(i128::from(reg_size));
        let new_offs = self.bin(BinOp::Add, Flags::NONE, reg_offs, rs);
        self.store(new_offs, offs_p, offs_align, false);
        let in_regs = self.icmp(IntPredicate::Sle, new_offs, zero);
        self.cond_br(in_regs, in_reg, on_stack);

        self.emit_block(in_reg, false);
        let (top_p, top_align) = self.va_field(list, top_i);
        let reg_top = self.load(LTy::PTR, top_p, top_align, false);
        let base = self.gep(LTy::I8, reg_top, vec![reg_offs], true);
        let base_align = if fpr { 16 } else { 8 };
        let addr_ty = if indirect { LTy::PTR } else { mem_ty };
        let reg_addr = if fpr && num_regs > 1 && !indirect {
            // An HFA/HVA: members sit 16 bytes apart in q registers; copy
            // them into a contiguous `[N x base]` temporary.
            let el = self.ir(&base_ty);
            let (el_size, el_align) = self.ir_layout(el);
            let hfa = self.m.types.array(el, num_regs);
            let hfa_align = ty_align.max(el_align);
            let tmp = self.f.fir.alloca(hfa, hfa_align);
            for i in 0..num_regs {
                let off = self.i64c(i128::from(16 * i));
                let la = self.gep(LTy::I8, base, vec![off], true);
                let z = self.i64c(0);
                let ix = self.i64c(i128::from(i));
                let sa = self.gep(hfa, tmp, vec![z, ix], true);
                let v = self.load(el, la, align_at(base_align, 16 * i), false);
                self.store(v, sa, align_at(hfa_align, el_size * i), false);
            }
            Addr { v: tmp, ty: addr_ty, align: hfa_align }
        } else {
            Addr { v: base, ty: addr_ty, align: base_align }
        };
        let reg_from = self.f.fir.cur();
        self.emit_branch(end);

        self.emit_block(on_stack, false);
        let (stack_p, stack_align) = self.va_field(list, 0);
        let mut stack = self.load(LTy::PTR, stack_p, stack_align, false);
        if !indirect && ty_align > 8 {
            stack = self.round_ptr_up(stack, ty_align);
        }
        let stack_addr = Addr { v: stack, ty: addr_ty, align: ty_align.max(8) };
        let stack_size = if indirect { 8 } else { ty_size.next_multiple_of(8) };
        let sz = self.i64c(i128::from(stack_size));
        let new_stack = self.gep(LTy::I8, stack, vec![sz], true);
        self.store(new_stack, stack_p, stack_align, false);
        let stack_from = self.f.fir.cur();

        self.emit_block(end, false);
        let (Some(rb), Some(sb)) = (reg_from, stack_from) else {
            return err(None, "va_arg: malformed control flow");
        };
        let res = self.merge_phi(reg_addr, rb, stack_addr, sb);
        if indirect {
            let p = self.load(LTy::PTR, res.v, res.align, false);
            return Ok(Addr { v: p, ty: mem_ty, align: ty_align });
        }
        Ok(res)
    }

    /// clang `EmitX86_64VAArgFromMemory`.
    fn x86_64_va_arg_mem(&mut self, list: Addr, q: kiln_ast::QualType) -> Addr {
        let (p, pa) = self.va_field(list, 2);
        let mut area = self.load(LTy::PTR, p, pa, false);
        let (size, align) = self.size_align(q);
        if align > 8 {
            area = self.round_ptr_up(area, align);
        }
        let off = self.i32v(i128::from(size.next_multiple_of(8)));
        let next = self.gep(LTy::I8, area, vec![off], false);
        self.store(next, p, pa, false);
        let ty = self.mty(q);
        Addr { v: area, ty, align }
    }

    /// clang `X86_64ABIInfo::EmitVAArg`.
    fn x86_64_va_arg(
        &mut self,
        list: Addr,
        q: kiln_ast::QualType,
        kind: &ArgKind,
        ni: u32,
        ns: u32,
    ) -> R<Addr> {
        if ni == 0 && ns == 0 {
            return Ok(self.x86_64_va_arg_mem(list, q));
        }
        let mut in_regs = None;
        let mut gp = None;
        let mut fp = None;
        if ni > 0 {
            let (p, a) = self.va_field(list, 0);
            let off = self.load(LTy::I32, p, a, false);
            let lim = self.i32v(48 - i128::from(ni) * 8);
            in_regs = Some(self.icmp(IntPredicate::Ule, off, lim));
            gp = Some((p, a, off));
        }
        if ns > 0 {
            let (p, a) = self.va_field(list, 1);
            let off = self.load(LTy::I32, p, a, false);
            let lim = self.i32v(176 - i128::from(ns) * 16);
            let fits = self.icmp(IntPredicate::Ule, off, lim);
            in_regs = Some(match in_regs {
                Some(g) => self.bin(BinOp::And, Flags::NONE, g, fits),
                None => fits,
            });
            fp = Some((p, a, off));
        }
        let Some(in_regs) = in_regs else { unreachable!() };
        let in_reg = self.f.fir.create_block();
        let in_mem = self.f.fir.create_block();
        let cont = self.f.fir.create_block();
        self.cond_br(in_regs, in_reg, in_mem);

        self.emit_block(in_reg, false);
        let (rp, ra) = self.va_field(list, 3);
        let rsa = self.load(LTy::PTR, rp, ra, false);
        let mem_ty = self.mty(q);
        let (ty_size, ty_align) = self.size_align(q);
        let coerce = match kind {
            ArgKind::Direct { coerce, offset, .. } => Some((coerce.clone(), *offset)),
            _ => None,
        };
        let reg_addr = if let (Some((_, _, go)), Some((_, _, fo))) = (gp, fp) {
            let Some((IrType::Struct(elems, _), _)) = &coerce else {
                return err(None, "va_arg: mixed-register type without a struct coercion");
            };
            let st_ir = IrType::Struct(elems.clone(), false);
            let st = self.ir(&st_ir);
            let (Some(lo), Some(hi)) = (elems.first(), elems.get(1)) else {
                return err(None, "va_arg: malformed mixed-register coercion");
            };
            let lo_fp = lo.is_fp() || matches!(lo, IrType::Vector(..));
            let (lo, hi) = (self.ir(lo), self.ir(hi));
            let tmp = self.temp(q);
            let gpa = self.gep(LTy::I8, rsa, vec![go], false);
            let fpa = self.gep(LTy::I8, rsa, vec![fo], false);
            let (la, ha) = if lo_fp { (fpa, gpa) } else { (gpa, fpa) };
            let offs = self.struct_field_offsets(st);
            let lo_align = self.ir_layout(lo).1;
            let v = self.load(lo, la, lo_align, false);
            let d = self.struct_gep(st, tmp.v, 0);
            self.store(v, d, align_at(tmp.align, offs.first().copied().unwrap_or(0)), false);
            let hi_align = self.ir_layout(hi).1;
            let v = self.load(hi, ha, hi_align, false);
            let d = self.struct_gep(st, tmp.v, 1);
            self.store(v, d, align_at(tmp.align, offs.get(1).copied().unwrap_or(0)), false);
            tmp
        } else if ni > 0 || ns == 1 {
            let ((Some((_, _, off)), _) | (None, Some((_, _, off)))) = (gp, fp) else {
                unreachable!("a register va_arg uses gp or fp")
            };
            let alignment = if ni > 0 { 8 } else { 16 };
            let reg_size = if ni > 0 { u64::from(ni) * 8 } else { 16 };
            // clang 20+ reads a coerced value whose saved registers skip
            // its leading empty bytes (`offset` 8) or cover less than the
            // whole type (a trailing empty member or `alignas` padding)
            // into a temporary; clang 19 addresses the save area directly
            // (spec/abi.md §8.12).
            let partial = self.opts.clang_compat >= 20 && (reg_size < ty_size);
            match &coerce {
                Some((co, offset)) if *offset == 8 || partial => {
                    let co = self.ir(co);
                    let tmp = self.temp(q);
                    let a = self.gep(LTy::I8, rsa, vec![off], false);
                    let src = self.load(co, a, ty_align, false);
                    let po = self.i32v(i128::from(*offset));
                    let dst = self.gep(LTy::I8, tmp.v, vec![po], false);
                    self.store(src, dst, ty_align, false);
                    tmp
                }
                _ => {
                    let a = self.gep(LTy::I8, rsa, vec![off], false);
                    let reg = Addr { v: a, ty: mem_ty, align: alignment };
                    if ni > 0 && ty_align > 8 {
                        let tmp = self.temp(q);
                        self.memcpy(tmp, reg.v, reg.align, ty_size, false);
                        tmp
                    } else {
                        reg
                    }
                }
            }
        } else {
            // Two SSE eightbytes sit 16 bytes apart in the save area.
            let Some((_, _, fo)) = fp else { unreachable!() };
            let lo = self.gep(LTy::I8, rsa, vec![fo], false);
            let k16 = self.i64c(16);
            let hi = self.gep(LTy::I8, lo, vec![k16], true);
            let st_ir = match &coerce {
                Some((c @ IrType::Struct(..), _)) => c.clone(),
                _ => IrType::Struct(vec![IrType::Double, IrType::Double], false),
            };
            let IrType::Struct(elems, _) = &st_ir else { unreachable!() };
            let (Some(e0), Some(e1)) = (elems.first().cloned(), elems.get(1).cloned()) else {
                return err(None, "va_arg: malformed SSE-pair coercion");
            };
            let st = self.ir(&st_ir);
            let (e0, e1) = (self.ir(&e0), self.ir(&e1));
            let offs = self.struct_field_offsets(st);
            let tmp = self.temp(q);
            let v = self.load(e0, lo, 16, false);
            let d = self.struct_gep(st, tmp.v, 0);
            self.store(v, d, align_at(tmp.align, offs.first().copied().unwrap_or(0)), false);
            let v = self.load(e1, hi, 16, false);
            let d = self.struct_gep(st, tmp.v, 1);
            self.store(v, d, align_at(tmp.align, offs.get(1).copied().unwrap_or(0)), false);
            tmp
        };
        if let Some((p, a, o)) = gp {
            let k = self.i32v(i128::from(ni) * 8);
            let n = self.bin(BinOp::Add, Flags::NONE, o, k);
            self.store(n, p, a, false);
        }
        if let Some((p, a, o)) = fp {
            let k = self.i32v(i128::from(ns) * 16);
            let n = self.bin(BinOp::Add, Flags::NONE, o, k);
            self.store(n, p, a, false);
        }
        let reg_from = self.f.fir.cur();
        self.emit_branch(cont);

        self.emit_block(in_mem, false);
        let mem = self.x86_64_va_arg_mem(list, q);
        let mem_from = self.f.fir.cur();
        self.emit_block(cont, false);
        let (Some(rb), Some(mb)) = (reg_from, mem_from) else {
            return err(None, "va_arg: malformed control flow");
        };
        Ok(self.merge_phi(reg_addr, rb, mem, mb))
    }
}
