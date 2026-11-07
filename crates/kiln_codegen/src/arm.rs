//! `AArch64` ACLE builtins (`__builtin_arm_*`; spec/landed/sema/WP-724.md),
//! lowered as clang 19 `EmitAArch64BuiltinExpr`: the CRC32 checksums, bit
//! reversal and leading-zero / sign-bit counts, the hint instructions and
//! the barriers. Any other ACLE builtin is a "not supported yet" error when
//! its caller is emitted (the `arm_acle.h` wrappers of the rest still
//! type-check).

use kiln_ast::ExprId;
use kiln_ast::decl::builtin::arm;
use kiln_llvm::TypeId as LTy;
use kiln_llvm::intrinsics::{self, TargetMem};

use crate::builtins::BuiltinCall;
use crate::fir::V;
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
        let n = if hint(base).is_some() {
            0
        } else if crc32(base).is_some() {
            2
        } else {
            1
        };
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
            _ => {
                return err(
                    self.loc_of(e),
                    format!("code generation for {name} is not supported yet"),
                );
            }
        };
        Ok(BuiltinCall::Emitted(Some(v)))
    }
}
