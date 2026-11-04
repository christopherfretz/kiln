//! `AArch64` NEON builtins (`__builtin_neon_*`; spec/landed/codegen/WP-558.md "Target
//! features", WP-558): the per-function feature check, then the bf16
//! subset lowered as clang 19 `EmitAArch64BuiltinExpr`. Any other NEON
//! builtin is a "not supported yet" error when its caller is emitted.

use kiln_ast::ExprId;
use kiln_ast::decl::builtin::neon;
use kiln_llvm::{CastOp, TypeId as LTy, intrinsics};

use crate::builtins::BuiltinCall;
use crate::fir::V;
use crate::module::{Cg, R, err};

/// An LLVM type of a lowered NEON intrinsic.
#[derive(Copy, Clone)]
enum T {
    /// `float`.
    F32,
    /// `bfloat`.
    Bf16,
    /// `<N x float>`.
    VF32(u32),
    /// `<N x bfloat>`.
    VBf16(u32),
}

/// The intrinsic, result and parameter types of NEON builtin `name`
/// (clang 19 `AArch64SIMDIntrinsicMap` / `EmitAArch64BuiltinExpr`; the
/// trailing type-code operand of a `_f32` builtin is not passed).
fn lowering(name: &str) -> Option<(&'static str, T, &'static [T])> {
    use T::{Bf16, F32, VBf16, VF32};
    let q: &'static [T] = &[VF32(4), VBf16(8), VBf16(8)];
    Some(match name.strip_prefix(neon::PREFIX)? {
        "vbfdot_f32" => {
            ("llvm.aarch64.neon.bfdot.v2f32.v4bf16", VF32(2), &[VF32(2), VBf16(4), VBf16(4)])
        }
        "vbfdotq_f32" => ("llvm.aarch64.neon.bfdot.v4f32.v8bf16", VF32(4), q),
        "vbfmlalbq_f32" => ("llvm.aarch64.neon.bfmlalb", VF32(4), q),
        "vbfmlaltq_f32" => ("llvm.aarch64.neon.bfmlalt", VF32(4), q),
        "vbfmmlaq_f32" => ("llvm.aarch64.neon.bfmmla", VF32(4), q),
        "vcvth_bf16_f32" => ("llvm.aarch64.neon.bfcvt", Bf16, &[F32]),
        "__a64_vcvtq_low_bf16_f32" => ("llvm.aarch64.neon.bfcvtn", VBf16(8), &[VF32(4)]),
        "vcvtq_high_bf16_f32" => ("llvm.aarch64.neon.bfcvtn2", VBf16(8), &[VBf16(8), VF32(4)]),
        _ => return None,
    })
}

impl Cg<'_> {
    fn neon_ty(&mut self, t: T) -> LTy {
        match t {
            T::F32 => LTy::FLOAT,
            T::Bf16 => LTy::BFLOAT,
            T::VF32(n) => self.m.types.vector(LTy::FLOAT, n),
            T::VBf16(n) => self.m.types.vector(LTy::BFLOAT, n),
        }
    }

    /// The width in bits of a NEON operand type (`None`: not a scalar
    /// float or fixed vector kiln lowers here).
    fn neon_bits(&self, t: LTy) -> Option<u32> {
        let scalar = |t: LTy| match t {
            LTy::FLOAT | LTy::I32 => Some(32),
            LTy::BFLOAT | LTy::HALF | LTy::I16 => Some(16),
            LTy::I8 => Some(8),
            LTy::DOUBLE | LTy::I64 => Some(64),
            _ => None,
        };
        match self.m.types.vector_info(t) {
            Some((e, n, false)) => scalar(e).map(|b| b * n),
            Some((_, _, true)) => None,
            None => scalar(t),
        }
    }

    /// `v` as type `to`: a bitcast between equal-width types.
    fn neon_bitcast(&mut self, v: V, from: LTy, to: LTy, e: ExprId) -> R<V> {
        if from == to {
            return Ok(v);
        }
        match (self.neon_bits(from), self.neon_bits(to)) {
            (Some(a), Some(b)) if a == b => Ok(self.cast(CastOp::BitCast, v, to)),
            _ => err(self.loc_of(e), "NEON builtin operand has an unexpected type"),
        }
    }

    /// The call `e` of NEON builtin `b`. A feature the calling function
    /// lacks is `err_builtin_needs_feature` and the call's value poison
    /// (emission continues; the driver fails on the error).
    pub(crate) fn emit_neon_builtin(&mut self, b: u32, e: ExprId) -> R<BuiltinCall> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let rty = self.lty(q);
        if !self.neon_builtin_features_ok(b, e) {
            let p = if rty == LTy::VOID { None } else { Some(V::C(self.m.const_poison(rty))) };
            return Ok(BuiltinCall::Emitted(p));
        }
        let name = neon::row(b).map_or("", |r| r.info.name);
        let Some((intr, ret, params)) = lowering(name) else {
            return err(self.loc_of(e), format!("code generation for {name} is not supported yet"));
        };
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        if args.len() < params.len() {
            return err(self.loc_of(e), format!("too few arguments to {name}"));
        }
        // clang emits every operand, then bitcasts each to its parameter.
        let mut vals = Vec::with_capacity(params.len());
        for &a in &args[..params.len()] {
            vals.push(self.emit_scalar(a)?);
        }
        let mut ptys = Vec::with_capacity(params.len());
        for ((v, &a), &p) in vals.iter_mut().zip(&args).zip(params) {
            let aty = self.lty(ast.exprs.get(a).ty());
            let pty = self.neon_ty(p);
            *v = self.neon_bitcast(*v, aty, pty, a)?;
            ptys.push(pty);
        }
        let ret = self.neon_ty(ret);
        let f = intrinsics::target_nomem(&mut self.m, intr, ret, &ptys);
        let r = self.intrinsic_call(f, vals);
        // clang's `vbfdot` case returns the intrinsic's `<N x float>`
        // as is (the header casts it to `float32xN_t` at once; a store or
        // cast of it goes by its own type, as in clang); the others go
        // through `EmitCommonNeonBuiltinExpr`'s bitcast to the builtin's
        // declared type.
        if intr.starts_with("llvm.aarch64.neon.bfdot.") {
            return Ok(BuiltinCall::Emitted(Some(r)));
        }
        Ok(BuiltinCall::Emitted(Some(self.neon_bitcast(r, ret, rty, e)?)))
    }
}
