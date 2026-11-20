//! The x86 builtins whose results are not one intrinsic call's value
//! (WP-1739): the AMX `*_internal` builtins, on tile registers
//! (`x86_amx`) converted from and to `<256 x i32>`; the Key Locker
//! builtins, whose `{ i8/i32, <2 x i64>... }` results are stored through
//! pointers; and clang 19's MMX `maskmovq`, emitted as clang 20+'s
//! `<xmmintrin.h>` defines `_mm_maskmove_si64` (as the other MMX builtins,
//! `x86_mmx.rs`). Each follows clang's `EmitX86BuiltinExpr`.

use kiln_ast::ExprId;
use kiln_llvm::intrinsics::{self, TargetMem as Mem};
use kiln_llvm::{BinOp, CastOp, Flags, IntPredicate, TypeId as LTy};

use crate::fir::{Op, V};
use crate::module::{Cg, R, err};

/// The operands of an AMX `*_internal` builtin: `S` a shape (`i16`), `P`
/// a pointer, `I` a stride (`i64`), `T` a tile (`<256 x i32>`, converted
/// to `x86_amx`).
fn amx_operands(n: &str) -> Option<(&'static str, bool)> {
    // (operand kinds, whether the builtin returns a tile)
    Some(match n {
        "tileloadd64_internal" | "tileloaddt164_internal" => ("SSPI", true),
        "tilestored64_internal" => ("SSPIT", false),
        "tilezero_internal" => ("SS", true),
        "tdpbssd_internal"
        | "tdpbsud_internal"
        | "tdpbusd_internal"
        | "tdpbuud_internal"
        | "tdpbf16ps_internal"
        | "tdpfp16ps_internal"
        | "tcmmimfp16ps_internal"
        | "tcmmrlfp16ps_internal" => ("SSSTTT", true),
        _ => return None,
    })
}

impl Cg<'_> {
    /// The AMX, Key Locker and `maskmovq` builtin `n` (without its
    /// `__builtin_ia32_` prefix), or `None` for another builtin.
    #[allow(
        clippy::option_option,
        reason = "outer: lowered here or not; inner: the call's value, `None` when void"
    )]
    pub(crate) fn x86_kl_amx(
        &mut self,
        n: &str,
        e: ExprId,
        args: &[ExprId],
    ) -> R<Option<Option<V>>> {
        let known = amx_operands(n).is_some()
            || n == "maskmovq"
            || n.starts_with("encodekey")
            || (n.starts_with("aes") && n.ends_with("kl_u8"));
        if !known {
            return Ok(None);
        }
        let mut ops = Vec::with_capacity(args.len());
        if let Some((kinds, tile_ret)) = amx_operands(n) {
            // clang (`EmitBuiltinExpr`'s target-intrinsic path) declares
            // the intrinsic first, then converts each tile operand right
            // after emitting it.
            let tile = self.m.types.x86_amx();
            let vt = self.m.types.vector(LTy::I32, 256);
            let p = self.m.types.ptr(0);
            let kind_ty = |k: u8| match k {
                b'S' => LTy::I16,
                b'P' => p,
                b'I' => LTy::I64,
                _ => tile,
            };
            let params: Vec<(LTy, bool)> = kinds.bytes().map(|k| (kind_ty(k), false)).collect();
            let base = n.trim_end_matches("_internal");
            let ret = if tile_ret { tile } else { LTy::VOID };
            let intr = format!("llvm.x86.{base}.internal");
            let f = intrinsics::target_intrinsic(&mut self.m, &intr, ret, &params, Mem::Unknown);
            if args.len() != kinds.len() {
                return err(self.loc_of(e), format!("wrong operand count for {n}"));
            }
            for (k, &a) in kinds.bytes().zip(args) {
                let v = self.emit_scalar(a)?;
                ops.push(if k == b'T' { self.x86_cast_tile(v, vt, tile, true) } else { v });
            }
            let r = self.intrinsic_call(f, ops);
            return Ok(Some(tile_ret.then(|| self.x86_cast_tile(r, vt, tile, false))));
        }
        for &a in args {
            ops.push(self.emit_scalar(a)?);
        }
        let need = if n == "encodekey256_u32" { 4 } else { 3 };
        if ops.len() != need {
            return err(self.loc_of(e), format!("wrong operand count for {n}"));
        }
        let v = match n {
            "maskmovq" => {
                self.x86_maskmovq(&ops, e)?;
                return Ok(Some(None));
            }
            "encodekey128_u32" | "encodekey256_u32" => self.x86_encodekey(n, &ops),
            _ => self.x86_aes_kl(n, &ops),
        };
        Ok(Some(Some(v)))
    }

    /// `llvm.x86.cast.vector.to.tile.v256i32` (`to_tile`) or
    /// `llvm.x86.cast.tile.to.vector.v256i32`.
    fn x86_cast_tile(&mut self, v: V, vt: LTy, tile: LTy, to_tile: bool) -> V {
        let (name, ret, from) = if to_tile {
            ("llvm.x86.cast.vector.to.tile.v256i32", tile, vt)
        } else {
            ("llvm.x86.cast.tile.to.vector.v256i32", vt, tile)
        };
        let f = intrinsics::target_intrinsic(&mut self.m, name, ret, &[(from, false)], Mem::None);
        self.intrinsic_call(f, vec![v])
    }

    /// `encodekey{128,256}_u32(htype, key..., out)`: the handle's 16-byte
    /// parts (three, or four) stored through `out` unaligned, the status
    /// the value.
    fn x86_encodekey(&mut self, n: &str, ops: &[V]) -> V {
        let wide = n == "encodekey256_u32";
        let (keys, parts, fields) = if wide { (2, 4, 7) } else { (1, 3, 6) };
        let q2 = self.m.types.vector(LTy::I64, 2);
        let mut tys = vec![LTy::I32];
        tys.extend(std::iter::repeat_n(q2, fields));
        let st = self.m.types.struct_literal(&tys, false);
        let mut params = vec![(LTy::I32, false)];
        params.extend(std::iter::repeat_n((q2, false), keys));
        let intr = if wide { "llvm.x86.encodekey256" } else { "llvm.x86.encodekey128" };
        let f = intrinsics::target_intrinsic(&mut self.m, intr, st, &params, Mem::Unknown);
        let r = self.intrinsic_call(f, ops[..=keys].to_vec());
        let out = ops[keys + 1];
        for i in 0..parts {
            let x = self.f.fir.push(Op::ExtractValue { agg: r, idx: i + 1 }, q2);
            let off = self.icn(LTy::I32, i128::from(i) * 16);
            let p = self.gep(LTy::I8, out, vec![off], false);
            self.store(x, p, 1, false);
        }
        self.f.fir.push(Op::ExtractValue { agg: r, idx: 0 }, LTy::I32)
    }

    /// `aes{enc,dec}{,wide}{128,256}kl_u8(out, in, handle)`: on success
    /// (bit 0 of the status) the result blocks stored through `out`,
    /// otherwise zeroes; the status the value.
    fn x86_aes_kl(&mut self, n: &str, ops: &[V]) -> V {
        let q2 = self.m.types.vector(LTy::I64, 2);
        let p = self.m.types.ptr(0);
        let intr = format!("llvm.x86.{}", n.trim_end_matches("_u8"));
        let (out, input, handle) = (ops[0], ops[1], ops[2]);
        let blocks: u32 = if n.contains("wide") { 8 } else { 1 };
        let mut tys = vec![LTy::I8];
        tys.extend(std::iter::repeat_n(q2, blocks as usize));
        let st = self.m.types.struct_literal(&tys, false);
        let (params, args) = if blocks == 1 {
            (vec![(q2, false), (p, false)], vec![input, handle])
        } else {
            let mut args = vec![handle];
            for i in 0..blocks {
                let idx = self.icn(LTy::I32, i128::from(i));
                let at = self.gep(q2, input, vec![idx], false);
                args.push(self.load(q2, at, 16, false));
            }
            let mut params = vec![(p, false)];
            params.extend(std::iter::repeat_n((q2, false), blocks as usize));
            (params, args)
        };
        let f = intrinsics::target_intrinsic(&mut self.m, &intr, st, &params, Mem::Unknown);
        let r = self.intrinsic_call(f, args);
        let status = self.f.fir.push(Op::ExtractValue { agg: r, idx: 0 }, LTy::I8);
        let ok = self.cast(CastOp::Trunc, status, LTy::I1);
        // The single form extracts its block before the branch.
        let single =
            (blocks == 1).then(|| self.f.fir.push(Op::ExtractValue { agg: r, idx: 1 }, q2));
        let no_error = self.f.fir.create_block();
        let error = self.f.fir.create_block();
        let end = self.f.fir.create_block();
        self.cond_br(ok, no_error, error);
        self.emit_block(no_error, false);
        match single {
            Some(x) => {
                self.store(x, out, 16, false);
            }
            None => {
                for i in 0..blocks {
                    let x = self.f.fir.push(Op::ExtractValue { agg: r, idx: i + 1 }, q2);
                    let idx = self.icn(LTy::I32, i128::from(i));
                    let at = self.gep(q2, out, vec![idx], false);
                    self.store(x, at, 16, false);
                }
            }
        }
        self.br(end);
        self.emit_block(error, false);
        let zero = V::C(self.m.const_zero(q2));
        if blocks == 1 {
            self.store(zero, out, 16, false);
        } else {
            for i in 0..blocks {
                // clang 19 extracted each (unused) block here first.
                if self.opts.clang_compat < 22 {
                    self.f.fir.push(Op::ExtractValue { agg: r, idx: i + 1 }, q2);
                }
                let idx = self.icn(LTy::I32, i128::from(i));
                let at = self.gep(q2, out, vec![idx], false);
                self.store(zero, at, 16, false);
            }
        }
        self.br(end);
        self.emit_block(end, false);
        self.f.fir.push(Op::ExtractValue { agg: r, idx: 0 }, LTy::I8)
    }

    /// clang 20+'s `_mm_maskmove_si64(d, n, p)`: `maskmovdqu` of `d`
    /// any-extended and `n` zero-extended to 128 bits, except that a `p`
    /// 8 to 15 bytes before a 4 KiB page boundary is backed up by 8 bytes
    /// and both operands shifted left by 8 bytes to match, so the 128-bit
    /// store cannot touch the next page.
    fn x86_maskmovq(&mut self, ops: &[V], e: ExprId) -> R<()> {
        let (d, m, p) = (ops[0], ops[1], ops[2]);
        let d = self.x86_anyext128(d);
        let m = self.x86_zext128(m);
        let addr = self.cast(CastOp::PtrToInt, p, LTy::I64);
        let mask = self.icn(LTy::I64, 0xfff);
        let low = self.bin(BinOp::And, Flags::NONE, addr, mask);
        let lo = self.icn(LTy::I64, 4096 - 15);
        let hi = self.icn(LTy::I64, 4096 - 8);
        let ge = self.icmp(IntPredicate::Uge, low, lo);
        let le = self.icmp(IntPredicate::Ule, low, hi);
        let near = self.bin(BinOp::And, Flags::NONE, ge, le);
        let q2 = self.m.types.vector(LTy::I64, 2);
        let back = self.icn(LTy::I64, -8);
        let p8 = self.gep(LTy::I8, p, vec![back], true);
        let p = self.select(near, p8, p);
        let mut wide = Vec::with_capacity(2);
        for v in [d, m] {
            let v = self.cast(CastOp::BitCast, v, q2);
            let shifted = self.x86_byteshift(v, 8, true, q2);
            let v = self.select(near, shifted, v);
            let b16 = self.m.types.vector(LTy::I8, 16);
            wide.push(self.cast(CastOp::BitCast, v, b16));
        }
        wide.push(p);
        self.x86_call_builtin("__builtin_ia32_maskmovdqu", wide, e)?;
        Ok(())
    }
}
