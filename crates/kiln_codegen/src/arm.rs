//! `AArch64` ACLE builtins (`__builtin_arm_*`; spec/landed/sema/WP-724.md,
//! spec/landed/codegen/WP-846.md), lowered as clang 19
//! `EmitAArch64BuiltinExpr`: the CRC32 checksums, bit reversal and
//! leading-zero / sign-bit counts, the hint instructions, the barriers, the
//! exclusive accesses, the system register accesses, the FRINT/FJCVTZS
//! conversions, the random numbers, the 64-byte loads and stores, the GCS
//! builtins, `chkfeat`, `prefetch` and `trap`.

use kiln_ast::decl::builtin::arm;
use kiln_ast::expr::ExprKind;
use kiln_ast::{ExprId, QualType};
use kiln_llvm::intrinsics::{self, TargetMem};
use kiln_llvm::{Attr, AttrList, BinOp, CallOpts, CastOp, Flags, FuncId, TypeId as LTy, Value};

use crate::builtins::BuiltinCall;
use crate::fir::{Fir, Op, V};
use crate::module::{Cg, R, err};

/// The `llvm.aarch64.hint` immediate of a hint builtin (clang `HintID`).
fn hint(base: &str) -> Option<i128> {
    Some(match base {
        "nop" => 0,
        "yield" => 1,
        "wfe" => 2,
        "wfi" => 3,
        "sev" => 4,
        "sevl" => 5,
        _ => return None,
    })
}

/// The intrinsic of a CRC32 builtin and whether its data operand is 64-bit
/// (the 8- and 16-bit ones are zero-extended to `i32`).
fn crc32(base: &str) -> Option<(&'static str, bool)> {
    Some(match base {
        "crc32b" => ("llvm.aarch64.crc32b", false),
        "crc32cb" => ("llvm.aarch64.crc32cb", false),
        "crc32h" => ("llvm.aarch64.crc32h", false),
        "crc32ch" => ("llvm.aarch64.crc32ch", false),
        "crc32w" => ("llvm.aarch64.crc32w", false),
        "crc32cw" => ("llvm.aarch64.crc32cw", false),
        "crc32d" => ("llvm.aarch64.crc32x", true),
        "crc32cd" => ("llvm.aarch64.crc32cx", true),
        _ => return None,
    })
}

impl Cg<'_> {
    /// The ACLE builtin call `e` of builtin `b`, after clang's
    /// `checkTargetFeatures` (a builtin whose features the function lacks
    /// is an error and yields `poison`).
    pub(crate) fn emit_arm_builtin(&mut self, b: u32, e: ExprId) -> R<BuiltinCall> {
        let ast = self.ast;
        let Some(row) = arm::row(b) else {
            return err(self.loc_of(e), "not an ACLE builtin");
        };
        let rty = self.lty(ast.exprs.get(e).ty());
        if !self.arm_builtin_features_ok(&row, e) {
            let p = if rty == LTy::VOID { None } else { Some(V::C(self.m.const_poison(rty))) };
            return Ok(BuiltinCall::Emitted(p));
        }
        let name = row.info.name;
        let base = name.strip_prefix(arm::PREFIX).unwrap_or(name);
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        let n = arity(base);
        if args.len() < n {
            return err(self.loc_of(e), format!("too few arguments to {name}"));
        }
        if let Some(h) = hint(base) {
            let f = intrinsics::target_intrinsic(
                &mut self.m,
                "llvm.aarch64.hint",
                LTy::VOID,
                &[(LTy::I32, false)],
                TargetMem::Unknown,
            );
            let h = V::C(self.m.const_int(LTy::I32, h));
            self.intrinsic_call(f, vec![h]);
            return Ok(BuiltinCall::Emitted(None));
        }
        if let Some((intr, wide)) = crc32(base) {
            let c = self.emit_scalar(args[0])?;
            let v = self.emit_scalar(args[1])?;
            let dt = if wide { LTy::I64 } else { LTy::I32 };
            let v = self.int_cast(v, dt, false);
            let f = intrinsics::target_nomem(&mut self.m, intr, LTy::I32, &[LTy::I32, dt]);
            return Ok(BuiltinCall::Emitted(Some(self.intrinsic_call(f, vec![c, v]))));
        }
        let v = match base {
            "clrex" => {
                let f = intrinsics::target_intrinsic(
                    &mut self.m,
                    "llvm.aarch64.clrex",
                    LTy::VOID,
                    &[],
                    TargetMem::Unknown,
                );
                self.intrinsic_call(f, vec![]);
                return Ok(BuiltinCall::Emitted(None));
            }
            "dmb" | "dsb" | "isb" => {
                let x = self.emit_scalar(args[0])?;
                let intr = format!("llvm.aarch64.{base}");
                let f = intrinsics::target_intrinsic(
                    &mut self.m,
                    &intr,
                    LTy::VOID,
                    &[(LTy::I32, false)],
                    TargetMem::Unknown,
                );
                self.intrinsic_call(f, vec![x]);
                return Ok(BuiltinCall::Emitted(None));
            }
            "rbit" | "rbit64" => {
                let x = self.emit_scalar(args[0])?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = intrinsics::fp_op(&mut self.m, "llvm.bitreverse", ty, 1);
                self.intrinsic_call(f, vec![x])
            }
            // `ctlz(x, false)`: zero is defined (32 or 64), as clang.
            "clz" | "clz64" => {
                let x = self.emit_scalar(args[0])?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = intrinsics::ctlz(&mut self.m, ty);
                let no = V::C(self.m.const_bool(false));
                let r = self.intrinsic_call(f, vec![x, no]);
                self.int_cast(r, rty, false)
            }
            "cls" | "cls64" => {
                let x = self.emit_scalar(args[0])?;
                let ty = self.f.fir.ty(&self.m, x);
                let intr = format!("llvm.aarch64.{base}");
                let f = intrinsics::target_nomem(&mut self.m, &intr, LTy::I32, &[ty]);
                self.intrinsic_call(f, vec![x])
            }
            "ldrex" | "ldaex" => self.emit_arm_load_exclusive(base == "ldaex", e, args[0])?,
            "strex" | "stlex" => {
                self.emit_arm_store_exclusive(base == "stlex", args[0], args[1])?
            }
            "rsr" | "rsr64" | "rsrp" | "rsr128" | "wsr" | "wsr64" | "wsrp" | "wsr128" => {
                match self.emit_arm_sysreg(base, e, &args)? {
                    Some(v) => v,
                    None => return Ok(BuiltinCall::Emitted(None)),
                }
            }
            "rint32x" | "rint32xf" | "rint32z" | "rint32zf" | "rint64x" | "rint64xf"
            | "rint64z" | "rint64zf" => {
                let x = self.emit_scalar(args[0])?;
                let ty = self.f.fir.ty(&self.m, x);
                let op = base.strip_suffix('f').unwrap_or(base);
                let sfx = if ty == LTy::FLOAT { "f32" } else { "f64" };
                let intr = format!("llvm.aarch64.f{op}.{sfx}");
                let f = intrinsics::target_nomem(&mut self.m, &intr, ty, &[ty]);
                self.intrinsic_call(f, vec![x])
            }
            "jcvt" => {
                let x = self.emit_scalar(args[0])?;
                let f = intrinsics::target_nomem(
                    &mut self.m,
                    "llvm.aarch64.fjcvtzs",
                    LTy::I32,
                    &[LTy::DOUBLE],
                );
                self.intrinsic_call(f, vec![x])
            }
            "rndr" | "rndrrs" => {
                // The value and status, then the store through the operand
                // (clang emits the pointer after the call).
                let pair = self.m.types.struct_literal(&[LTy::I64, LTy::I1], false);
                let intr = format!("llvm.aarch64.{base}");
                let f = intrinsics::target_intrinsic(
                    &mut self.m,
                    &intr,
                    pair,
                    &[],
                    TargetMem::SideEffects,
                );
                let r = self.intrinsic_call(f, vec![]);
                let v = self.f.fir.push(Op::ExtractValue { agg: r, idx: 0 }, LTy::I64);
                let ok = self.f.fir.push(Op::ExtractValue { agg: r, idx: 1 }, LTy::I1);
                let (p, align) = self.ptr_align(args[0])?;
                self.store(v, p, align, false);
                self.cast(CastOp::ZExt, ok, LTy::I32)
            }
            "ld64b" | "st64b" | "st64bv" | "st64bv0" => match self.emit_arm_ls64(base, &args)? {
                Some(v) => v,
                None => return Ok(BuiltinCall::Emitted(None)),
            },
            "gcspopm" | "chkfeat" => {
                let x = self.emit_scalar(args[0])?;
                let intr = format!("llvm.aarch64.{base}");
                let f = if base == "chkfeat" {
                    intrinsics::target_nomem(&mut self.m, &intr, LTy::I64, &[LTy::I64])
                } else {
                    intrinsics::target_intrinsic(
                        &mut self.m,
                        &intr,
                        LTy::I64,
                        &[(LTy::I64, false)],
                        TargetMem::SideEffects,
                    )
                };
                self.intrinsic_call(f, vec![x])
            }
            "gcsss" => {
                let x = self.emit_scalar(args[0])?;
                let f = intrinsics::target_intrinsic(
                    &mut self.m,
                    "llvm.aarch64.gcsss",
                    LTy::PTR,
                    &[(LTy::PTR, false)],
                    TargetMem::SideEffects,
                );
                self.intrinsic_call(f, vec![x])
            }
            "prefetch" => {
                let mut ops = Vec::with_capacity(5);
                for &a in &args[..5] {
                    ops.push(self.emit_scalar(a)?);
                }
                let f = intrinsics::aarch64_prefetch(&mut self.m);
                self.intrinsic_call(f, ops);
                return Ok(BuiltinCall::Emitted(None));
            }
            // `llvm.aarch64.break`, with no `unreachable` after it: clang
            // returns the call as the builtin's value.
            "trap" => {
                let x = self.emit_scalar(args[0])?;
                let x = self.int_cast(x, LTy::I32, false);
                let f = intrinsics::aarch64_break(&mut self.m);
                self.intrinsic_call(f, vec![x]);
                return Ok(BuiltinCall::Emitted(None));
            }
            _ => {
                return err(
                    self.loc_of(e),
                    format!("code generation for {name} is not supported yet"),
                );
            }
        };
        Ok(BuiltinCall::Emitted(Some(v)))
    }

    /// A call of intrinsic `f` whose operand `idx` carries
    /// `elementtype(et)` (the exclusive accesses' address).
    fn intrinsic_call_elementtype(&mut self, f: FuncId, args: Vec<V>, idx: usize, et: LTy) -> V {
        let fn_ty = self.m.func(f).ty();
        let rty = self.m.types.function_info(fn_ty).map_or(LTy::VOID, |i| i.0);
        let none = self.m.attr_set(&[]);
        let mut params = vec![none; args.len()];
        params[idx] = self.m.attr_set(&[Attr::ElementType(et)]);
        let opts =
            CallOpts { attrs: AttrList { params, ..AttrList::default() }, ..CallOpts::default() };
        let r = self.f.fir.push(
            Op::Call { fn_ty, callee: V::C(Value::func(f)), args, opts: Box::new(opts) },
            rty,
        );
        self.f.calls.push((Fir::last_inst(r), Some(f), true));
        r
    }

    /// The width in bits of IR type `ty` (`DataLayout::getTypeSizeInBits`).
    fn ir_bits(&self, ty: LTy) -> u32 {
        self.m
            .types
            .int_width(ty)
            .unwrap_or_else(|| u32::try_from(self.store_size(ty) * 8).unwrap_or(u32::MAX))
    }

    /// `sizeof(q)` in bits (clang `ASTContext::getTypeSize`).
    fn ast_bits(&mut self, q: QualType) -> u32 {
        u32::try_from(self.size_align(q).0 * 8).unwrap_or(u32::MAX)
    }

    /// `__builtin_arm_ldrex(p)` / `__builtin_arm_ldaex(p)`: a 128-bit
    /// value is `llvm.aarch64.ld[a]xp`'s two halves joined, anything else
    /// `llvm.aarch64.ld[a]xr` with `elementtype(iN)` (`N` the value's
    /// size) truncated (or converted to a pointer) and bit-cast to the
    /// result type.
    fn emit_arm_load_exclusive(&mut self, acquire: bool, e: ExprId, p: ExprId) -> R<V> {
        let rq = self.ast.exprs.get(e).ty();
        let rty = self.lty(rq);
        let bits = self.ast_bits(rq);
        let a = if acquire { "a" } else { "" };
        let p = self.emit_scalar(p)?;
        if bits == 128 {
            let pair = self.m.types.struct_literal(&[LTy::I64, LTy::I64], false);
            let intr = format!("llvm.aarch64.ld{a}xp");
            let f = intrinsics::target_intrinsic(
                &mut self.m,
                &intr,
                pair,
                &[(LTy::PTR, false)],
                TargetMem::NoFree,
            );
            let r = self.intrinsic_call(f, vec![p]);
            let hi = self.f.fir.push(Op::ExtractValue { agg: r, idx: 1 }, LTy::I64);
            let lo = self.f.fir.push(Op::ExtractValue { agg: r, idx: 0 }, LTy::I64);
            let hi = self.cast(CastOp::ZExt, hi, LTy::I128);
            let lo = self.cast(CastOp::ZExt, lo, LTy::I128);
            let k = V::C(self.m.const_int(LTy::I128, 64));
            let hi = self.bin(BinOp::Shl, Flags::NUW, hi, k);
            let v = self.bin(BinOp::Or, Flags::NONE, hi, lo);
            return Ok(self.cast(CastOp::BitCast, v, rty));
        }
        let intr = format!("llvm.aarch64.ld{a}xr.p0");
        let f = intrinsics::target_intrinsic(
            &mut self.m,
            &intr,
            LTy::I64,
            &[(LTy::PTR, false)],
            TargetMem::NoFree,
        );
        let et = self.m.types.int(bits);
        let r = self.intrinsic_call_elementtype(f, vec![p], 0, et);
        if self.m.types.is_ptr(rty) {
            return Ok(self.cast(CastOp::IntToPtr, r, rty));
        }
        let it = self.m.types.int(self.ir_bits(rty));
        let v = self.cast(CastOp::Trunc, r, it);
        Ok(self.cast(CastOp::BitCast, v, rty))
    }

    /// `__builtin_arm_strex(v, p)` / `__builtin_arm_stlex(v, p)`: a
    /// 128-bit value goes through a temporary as two `i64` halves to
    /// `llvm.aarch64.st[l]xp`, anything else is bit-cast (or converted
    /// from a pointer) and zero-extended to `i64` for
    /// `llvm.aarch64.st[l]xr` with `elementtype(iN)`.
    fn emit_arm_store_exclusive(&mut self, release: bool, val: ExprId, p: ExprId) -> R<V> {
        let vq = self.ast.exprs.get(val).ty();
        let bits = self.ast_bits(vq);
        let l = if release { "l" } else { "" };
        if bits == 128 {
            let tmp = self.temp(vq);
            let v = self.emit_scalar(val)?;
            self.store(v, tmp.v, tmp.align, false);
            let pair = self.m.types.struct_literal(&[LTy::I64, LTy::I64], false);
            let w = self.load(pair, tmp.v, tmp.align, false);
            let a0 = self.f.fir.push(Op::ExtractValue { agg: w, idx: 0 }, LTy::I64);
            let a1 = self.f.fir.push(Op::ExtractValue { agg: w, idx: 1 }, LTy::I64);
            let p = self.emit_scalar(p)?;
            let intr = format!("llvm.aarch64.st{l}xp");
            let f = intrinsics::target_intrinsic(
                &mut self.m,
                &intr,
                LTy::I32,
                &[(LTy::I64, false), (LTy::I64, false), (LTy::PTR, false)],
                TargetMem::NoFree,
            );
            return Ok(self.intrinsic_call(f, vec![a0, a1, p]));
        }
        let v = self.emit_scalar(val)?;
        let p = self.emit_scalar(p)?;
        let vty = self.f.fir.ty(&self.m, v);
        let v = if self.m.types.is_ptr(vty) {
            self.cast(CastOp::PtrToInt, v, LTy::I64)
        } else {
            let it = self.m.types.int(self.ir_bits(vty));
            let v = self.cast(CastOp::BitCast, v, it);
            self.cast(CastOp::ZExt, v, LTy::I64)
        };
        let intr = format!("llvm.aarch64.st{l}xr.p0");
        let f = intrinsics::target_intrinsic(
            &mut self.m,
            &intr,
            LTy::I32,
            &[(LTy::I64, false), (LTy::PTR, false)],
            TargetMem::NoFree,
        );
        let et = self.m.types.int(bits);
        Ok(self.intrinsic_call_elementtype(f, vec![v, p], 1, et))
    }

    /// The bytes of the string literal `e` is (through parentheses and
    /// casts; clang `IgnoreParenCasts`), without the terminator.
    fn arm_string_literal(&self, mut e: ExprId) -> Option<&[u8]> {
        let ast = self.ast;
        loop {
            match ast.exprs.get(e).kind() {
                ExprKind::StringLiteral => break,
                k if k == ExprKind::ParenExpr || k.is_cast() => e = ast.exprs.operand(e),
                _ => return None,
            }
        }
        let (_, bytes) = ast.strings.get(ast.exprs.string_lit(e));
        Some(bytes.strip_suffix(&[0]).unwrap_or(bytes))
    }

    /// clang `EmitSpecialRegisterBuiltin`: the register named by the
    /// string literal (as written) is `!{!"name"}`, read with
    /// `llvm.read_volatile_register` or written with `llvm.write_register`
    /// on `i64` (`i128` for the 128-bit forms); `rsr`/`wsr` truncate /
    /// zero-extend a 32-bit value, `rsrp`/`wsrp` convert a pointer.
    fn emit_arm_sysreg(&mut self, base: &str, e: ExprId, args: &[ExprId]) -> R<Option<V>> {
        let Some(name) = self.arm_string_literal(args[0]).map(<[u8]>::to_vec) else {
            return err(self.loc_of(e), "expression is not a string literal");
        };
        let wide = base.ends_with("128");
        let reg_ty = if wide { LTy::I128 } else { LTy::I64 };
        let sfx = if wide { "i128" } else { "i64" };
        let s = self.m.md_string(&name);
        let md = self.m.md_tuple(&[Some(s)]);
        let md = V::C(Value::metadata(md));
        if base.starts_with('r') {
            let intr = format!("llvm.read_volatile_register.{sfx}");
            let f = intrinsics::target_intrinsic(
                &mut self.m,
                &intr,
                reg_ty,
                &[(LTy::METADATA, false)],
                TargetMem::Unknown,
            );
            let r = self.intrinsic_call(f, vec![md]);
            return Ok(Some(match base {
                "rsr" => self.cast(CastOp::Trunc, r, LTy::I32),
                "rsrp" => self.cast(CastOp::IntToPtr, r, LTy::PTR),
                _ => r,
            }));
        }
        let v = self.emit_scalar(args[1])?;
        let v = match base {
            "wsr" => self.cast(CastOp::ZExt, v, LTy::I64),
            "wsrp" => self.cast(CastOp::PtrToInt, v, LTy::I64),
            _ => v,
        };
        let intr = format!("llvm.write_register.{sfx}");
        let f = intrinsics::target_intrinsic(
            &mut self.m,
            &intr,
            LTy::VOID,
            &[(LTy::METADATA, false), (reg_ty, false)],
            TargetMem::NoCallback,
        );
        self.intrinsic_call(f, vec![md, v]);
        Ok(None)
    }

    /// The `FEAT_LS64` builtins: `ld64b(addr, v)` stores the eight words
    /// `llvm.aarch64.ld64b` returns through `v`; `st64b*(addr, v)` load
    /// them from `v` for the intrinsic. Both operands come first; each word
    /// is a plain `getelementptr i64, ptr v, i32 i` (clang `CreateGEP`).
    fn emit_arm_ls64(&mut self, base: &str, args: &[ExprId]) -> R<Option<V>> {
        let addr = self.emit_scalar(args[0])?;
        let vp = self.emit_scalar(args[1])?;
        let intr = format!("llvm.aarch64.{base}");
        if base == "ld64b" {
            let words = self.m.types.struct_literal(&[LTy::I64; 8], false);
            let f = intrinsics::target_intrinsic(
                &mut self.m,
                &intr,
                words,
                &[(LTy::PTR, false)],
                TargetMem::Unknown,
            );
            let r = self.intrinsic_call(f, vec![addr]);
            for i in 0..8u32 {
                let k = V::C(self.m.const_int(LTy::I32, i128::from(i)));
                let at = self.gep(LTy::I64, vp, vec![k], false);
                let w = self.f.fir.push(Op::ExtractValue { agg: r, idx: i }, LTy::I64);
                self.store(w, at, 8, false);
            }
            return Ok(None);
        }
        let mut ops = vec![addr];
        for i in 0..8 {
            let k = V::C(self.m.const_int(LTy::I32, i));
            let at = self.gep(LTy::I64, vp, vec![k], false);
            ops.push(self.load(LTy::I64, at, 8, false));
        }
        let ret = if base == "st64b" { LTy::VOID } else { LTy::I64 };
        let mut params = vec![(LTy::PTR, false)];
        params.extend([(LTy::I64, false); 8]);
        let f = intrinsics::target_intrinsic(&mut self.m, &intr, ret, &params, TargetMem::Unknown);
        let r = self.intrinsic_call(f, ops);
        Ok((ret != LTy::VOID).then_some(r))
    }
}

/// The number of operands of ACLE builtin `base` (checked by sema; the
/// lowering reads them unchecked).
fn arity(base: &str) -> usize {
    if hint(base).is_some() || base == "clrex" {
        0
    } else if crc32(base).is_some()
        || base.starts_with("wsr")
        || matches!(base, "strex" | "stlex" | "ld64b" | "st64b" | "st64bv" | "st64bv0")
    {
        2
    } else if base == "prefetch" {
        5
    } else {
        1
    }
}
