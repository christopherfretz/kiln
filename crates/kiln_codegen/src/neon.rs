//! `AArch64` NEON builtins (`__builtin_neon_*`; spec/landed/codegen/WP-558.md "Target
//! features", WP-558): the per-function feature check, then two tables
//! lowered as clang 19 `EmitAArch64BuiltinExpr`: the bf16 subset, an
//! intrinsic call each ([`lowering`], also `vqtbl1`, `vmull_p64`), and the
//! load/store, lane, splat, narrowing, permute and arithmetic builtins
//! ([`shape`], WP-628; spec/landed/codegen/WP-628.md). Any other NEON
//! builtin is a "not supported yet" error when its caller is emitted.

use kiln_ast::ExprId;
use kiln_ast::decl::builtin::neon;
use kiln_llvm::{BinOp, CastOp, Flags, IntPredicate, TypeId as LTy, intrinsics};

use crate::builtins::BuiltinCall;
use crate::fir::V;
use crate::module::{Cg, R, err};

/// The element type of a NEON vector.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum E {
    /// `i8`.
    I8,
    /// `i16`.
    I16,
    /// `i32`.
    I32,
    /// `i64`.
    I64,
    /// `half`.
    F16,
    /// `bfloat`.
    Bf16,
    /// `float`.
    F32,
    /// `double`.
    F64,
}

impl E {
    /// The width in bits.
    const fn bits(self) -> u32 {
        match self {
            E::I8 => 8,
            E::I16 | E::F16 | E::Bf16 => 16,
            E::I32 | E::F32 => 32,
            E::I64 | E::F64 => 64,
        }
    }

    /// The integer element of twice the width (`vmovn`'s source; clang
    /// `getExtendedElementVectorType`).
    const fn widened(self) -> Option<E> {
        match self {
            E::I8 => Some(E::I16),
            E::I16 => Some(E::I32),
            E::I32 => Some(E::I64),
            _ => None,
        }
    }

    /// The element a fixed-type builtin's suffix (`_i8`, `_f32`, ...) names.
    fn from_suffix(s: &str) -> Option<E> {
        Some(match s {
            "i8" => E::I8,
            "i16" => E::I16,
            "i32" => E::I32,
            "i64" => E::I64,
            "f16" => E::F16,
            "bf16" => E::Bf16,
            "f32" => E::F32,
            "f64" => E::F64,
            _ => return None,
        })
    }
}

/// An LLVM type of a lowered NEON builtin.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum T {
    /// A scalar.
    S(E),
    /// `<N x E>`.
    V(E, u32),
}

impl T {
    /// The 64-bit (`quad` false) or 128-bit vector of `e`.
    const fn vec(e: E, quad: bool) -> T {
        T::V(e, if quad { 128 } else { 64 } / e.bits())
    }
}

/// The vector type a NEON type-code immediate selects (clang 19
/// `NeonTypeFlags` decoded by `GetNeonType` for `AArch64`, where `half` and
/// `bfloat` are legal): bits 0-3 the element (`Int8`, `Int16`, `Int32`,
/// `Int64`, `Poly8`, `Poly16`, `Poly64`, `Poly128`, `Float16`, `Float32`,
/// `Float64`, `BFloat16`), bit 4 unsigned, bit 5 quad (128-bit).
fn type_code(code: i128) -> Option<T> {
    let quad = code & 0x20 != 0;
    let e = match code & 0xf {
        0 | 4 => E::I8,
        1 | 5 => E::I16,
        2 => E::I32,
        3 | 6 => E::I64,
        // `Poly128` is `<16 x i8>` whatever the quad bit.
        7 => return Some(T::V(E::I8, 16)),
        8 => E::F16,
        9 => E::F32,
        10 => E::F64,
        11 => E::Bf16,
        _ => return None,
    };
    (0..64).contains(&code).then_some(T::vec(e, quad))
}

/// The intrinsic, result and parameter types of NEON builtin `name`
/// (clang 19 `AArch64SIMDIntrinsicMap` / `EmitAArch64BuiltinExpr`; the
/// trailing type-code operand of a `_f32` builtin is not passed).
fn lowering(name: &str) -> Option<(&'static str, T, &'static [T])> {
    const F32: T = T::S(E::F32);
    const BF16: T = T::S(E::Bf16);
    const F2: T = T::V(E::F32, 2);
    const F4: T = T::V(E::F32, 4);
    const B4: T = T::V(E::Bf16, 4);
    const B8: T = T::V(E::Bf16, 8);
    const B16: T = T::V(E::I8, 16);
    const B8I: T = T::V(E::I8, 8);
    const I2: T = T::V(E::I32, 2);
    const I4: T = T::V(E::I32, 4);
    let q: &'static [T] = &[F4, B8, B8];
    Some(match name.strip_prefix(neon::PREFIX)? {
        "vbfdot_f32" => ("llvm.aarch64.neon.bfdot.v2f32.v4bf16", F2, &[F2, B4, B4]),
        "vbfdotq_f32" => ("llvm.aarch64.neon.bfdot.v4f32.v8bf16", F4, q),
        "vbfmlalbq_f32" => ("llvm.aarch64.neon.bfmlalb", F4, q),
        "vbfmlaltq_f32" => ("llvm.aarch64.neon.bfmlalt", F4, q),
        "vbfmmlaq_f32" => ("llvm.aarch64.neon.bfmmla", F4, q),
        "vcvth_bf16_f32" => ("llvm.aarch64.neon.bfcvt", BF16, &[F32]),
        "__a64_vcvtq_low_bf16_f32" => ("llvm.aarch64.neon.bfcvtn", B8, &[F4]),
        "vcvtq_high_bf16_f32" => ("llvm.aarch64.neon.bfcvtn2", B8, &[B8, F4]),
        // The one-register table lookups (WP-628): every type code is a
        // byte vector.
        "vqtbl1_v" => ("llvm.aarch64.neon.tbl1.v8i8", T::V(E::I8, 8), &[B16, T::V(E::I8, 8)]),
        "vqtbl1q_v" => ("llvm.aarch64.neon.tbl1.v16i8", B16, &[B16, B16]),
        // The dot products (WP-803; clang `EmitCommonNeonBuiltinExpr`'s
        // `vdot` case): overloaded on the accumulator and the byte vector.
        "vdot_s32" => ("llvm.aarch64.neon.sdot.v2i32.v8i8", I2, &[I2, B8I, B8I]),
        "vdot_u32" => ("llvm.aarch64.neon.udot.v2i32.v8i8", I2, &[I2, B8I, B8I]),
        "vdotq_s32" => ("llvm.aarch64.neon.sdot.v4i32.v16i8", I4, &[I4, B16, B16]),
        "vdotq_u32" => ("llvm.aarch64.neon.udot.v4i32.v16i8", I4, &[I4, B16, B16]),
        // The 64-bit polynomial multiply: `<16 x i8>`, bitcast to `poly128_t`.
        "vmull_p64" => ("llvm.aarch64.neon.pmull64", B16, &[T::S(E::I64), T::S(E::I64)]),
        _ => return None,
    })
}

/// A NEON builtin lowered to plain IR, not an intrinsic call (clang 19
/// `EmitAArch64BuiltinExpr` / `EmitCommonNeonBuiltinExpr` cases; a `_bf16`
/// builtin is its `_v` twin, clang `NEONEquivalentIntrinsicMap`). `VTy` is
/// the vector of the trailing type-code immediate; `p` the pointer operand
/// and `A` its known alignment (clang `EmitPointerWithAlignment`); `lane`
/// an `i32` immediate.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Shape {
    /// `vld1[q]_v`: `load VTy, ptr p, align A`.
    Ld1,
    /// `vst1[q]_v`: `store (bitcast v to VTy), ptr p, align A`.
    St1,
    /// `vld1[q]_lane_v`: `insertelement (bitcast v to VTy), (load elt, ptr
    /// p, align A), lane`.
    Ld1Lane,
    /// `vst1[q]_lane_v`: `store (extractelement (bitcast v to VTy), lane),
    /// ptr p, align A`.
    St1Lane,
    /// `vld{2,3,4}[q]_v` (operands `r`, `p`): `store (call {VTy x N}
    /// @llvm.aarch64.neon.ldN.VTy.p0(p)), ptr r` at the struct's ABI
    /// alignment (clang 19 `EmitAArch64BuiltinExpr` `vld2_v`...).
    LdN(u32),
    /// `vst{2,3,4}[q]_v` (operands `p`, `v1`..`vN`): `call
    /// @llvm.aarch64.neon.stN.VTy.p0(bitcast v1 to VTy, ..., p)`.
    StN(u32),
    /// `vld1[q]_dup_v`: the loaded element inserted at lane 0 of `poison`,
    /// then `shufflevector` with a zero splat mask.
    Ld1Dup,
    /// `vget[q]_lane_<t>`, `vdup<b|h|s|d>_lane[q]_<t>`: `extractelement
    /// (bitcast v to T), lane`.
    GetLane(T),
    /// `vset[q]_lane_<t>` (operands `x`, `v`, `lane`): `insertelement
    /// (bitcast v to T), x, lane`.
    SetLane(T),
    /// `splat[q]_lane[q]_v`: `shufflevector (bitcast v to VTy), (same),
    /// splat(lane)`, `VTy`'s lane count times `mul` divided by `div` lanes.
    Splat { mul: u32, div: u32 },
    /// `vmovn_v`: `trunc (bitcast v to <N x 2E>) to VTy`.
    Movn,
    /// `vldrq_p128`: `load i128, ptr p, align 16`.
    LdrQ,
    /// `vstrq_p128`: `store i128 v, ptr p, align 16`.
    StrQ,
    /// `vext[q]_v` (operands `a`, `b`, `n`): `shufflevector (bitcast a to
    /// VTy), (bitcast b to VTy), <n, n+1, ...>`.
    Ext,
    /// `vzip[q]_v`, `vuzp[q]_v`, `vtrn[q]_v` (operands `p`, `a`, `b`): for
    /// `i` in 0..2, `store (shufflevector a, b, mask_i), (gep VTy, p, i)`
    /// at `VTy`'s ABI alignment.
    Perm(Perm),
    /// `vshr[q]_n_v` (operands `v`, `n`): `lshr`/`ashr` (unsigned/signed
    /// type code) of `bitcast v to VTy` by `splat(n)`; `n` equal to the
    /// element width is zero (unsigned) or a shift by width - 1 (signed).
    ShrN,
    /// An elementwise intrinsic call on `VTy` ([`Call`], [`call`]).
    Call(Call),
    /// `vbsl[q]_v` (operands `m`, `a`, `b`): with `BitTy` the integer
    /// vector of `VTy`'s shape, `or (and m, a), (and (xor m, splat(-1)),
    /// b)` over the three bitcast to `BitTy`, then bitcast to `VTy`.
    Bsl,
    /// `vtst[q]_v` (operands `a`, `b`): `sext (icmp ne (and a, b),
    /// zeroinitializer) to VTy` over the two bitcast to `VTy` (an integer
    /// vector).
    Tst,
    /// `vcvt[q]_f{32,64}_v`, `vcvt[q]_f16_{s,u}16`: `sitofp`/`uitofp`
    /// (signed/unsigned type code) of `bitcast v to VTy` (an integer
    /// vector) to the float vector of the same shape.
    CvtF,
    /// `vcvt[q]_{s,u}{32,64}_v`, `vcvt[q]_{s,u}16_f16`: `fcvtz{s,u}.VTy.F`
    /// (signed/unsigned type code) of `bitcast v to F`, `F` the float
    /// vector of `VTy`'s shape.
    CvtI,
    /// `v{max,min,add}v[q]_<t>` (operand `v`, no type code): the
    /// `{s,u,f}{max,min,add}v.R.T` call on `v` of type `T`, `R` `i32` (`i64`
    /// for 64-bit elements) truncated to a narrower element, or the float
    /// element. The `bool` is unsigned.
    Reduce(Red, T, bool),
    /// `vshl[q]_n_v` (operands `v`, `n`): `shl (bitcast v to VTy),
    /// splat(n)`, `n` in `0..width`.
    ShlN,
    /// `vshrn_n_v` (operands `v`, `n`): with `W` the vector of `VTy`'s
    /// lanes at twice the width, `trunc (lshr/ashr (bitcast v to W),
    /// splat(n)) to VTy`, `n` in `1..=width`.
    ShrnN,
    /// `vsra[q]_n_v` (operands `a`, `b`, `n`): `add a, s` over the two
    /// bitcast to `VTy`, `s` being `b` shifted as by [`Shape::ShrN`].
    Sra,
    /// `vsri[q]_n_v` (operands `a`, `b`, `n`): `vsri.VTy(a, b, i32 n)`
    /// over the two bitcast to `VTy`, `n` in `1..=width`.
    Sri,
    /// `vsli[q]_n_v` (operands `a`, `b`, `n`): `vsli.VTy(a, b, i32 n)`
    /// over the two bitcast to `VTy`, `n` in `0..width`.
    Sli,
    /// `vmovl_v` (operand `v`): `sext`/`zext` (signed/unsigned type code)
    /// of `bitcast v to N`, `N` the vector of `VTy`'s lanes at half the
    /// width.
    Movl,
    /// `vpaddl_v` (operand `v`): `{s,u}addlp.VTy.N(bitcast v to N)`, `N`
    /// twice `VTy`'s lanes at half the width (clang
    /// `EmitCommonNeonBuiltinExpr`; WP-792).
    Paddl,
    /// `vqmovn_v` (operand `v`): `{s,u}qxtn.VTy(bitcast v to W)`, `W` as
    /// for [`Shape::ShrnN`], bitcast to the declared type.
    QMovn,
    /// `vmull_v` (operands `a`, `b`; WP-724): `{s,u,p}mull.VTy` (signed,
    /// unsigned or polynomial type code) of `a` and `b` bitcast to the
    /// vector of `VTy`'s lanes at half the width.
    Mull,
    /// `vfma[q]_lane[q]_v`, `vfma[q]_lane[q]_f16` (operands `a`, `b`,
    /// `c`, `lane`): `fma` of `b`, `c`'s lane splat and `a`. `laneq` is a
    /// 128-bit `c`.
    FmaLane { laneq: bool },
    /// `vcvt_f16_f32`, `vcvt_f32_f16`, `vcvt_f32_f64`, `vcvt_f64_f32`
    /// (operand `v`): the [`Fp`] conversion.
    CvtFp(Fp),
    /// `vshll_n_v` (operands `v`, `n`; WP-803): `shl ({s,z}ext (bitcast v
    /// to the half-width lanes) to VTy), splat(n)` (clang
    /// `EmitCommonNeonBuiltinExpr`).
    ShllN,
    /// `vrshrn_n_v` (operands `v`, `n`; WP-803): `rshrn.VTy(bitcast v to
    /// W, i32 n)`, `W` as for [`Shape::ShrnN`] (clang 19
    /// `EmitAArch64BuiltinExpr` returns it as is).
    RshrnN,
    /// `vq{r,}shr{u,}n_n_v` (operands `v`, `n`; WP-803): as
    /// [`Shape::RshrnN`] with the signed or unsigned intrinsic.
    QShrnN { s: &'static str, u: &'static str },
    /// `vqshl[q]_n_v`, `vrshr[q]_n_v` (operands `v`, `n`; WP-803):
    /// `{s,u}{qshl,rshl}.VTy(v, splat(n))`, `-n` for a right shift (clang
    /// `EmitNeonShiftVector`), `VTy` as is.
    ShlCall { s: &'static str, u: &'static str, right: bool },
    /// `vcvt{n,a,m,p}[q]_{s,u}{32,64}_v` (operand `v`; WP-803):
    /// `fcvt{n,a,m,p}{s,u}.VTy.FTy(bitcast v to FTy)`.
    CvtR(u8),
    /// `vtbl1_v`, `vtbl2_v` (operands `t0`[, `t1`], `i`; WP-803):
    /// `tbl1.v8i8(shuffle t0, t1 or zero, i)`.
    Tbl(u8),
    /// `vqmovun_v` (operand `v`; WP-803): as [`Shape::QMovn`] with
    /// `sqxtun`.
    QMovun,
    /// `vpadal[q]_v` (operands `a`, `b`; WP-803): `add ({s,u}addlp.VTy.N(b
    /// as N)), (bitcast a to VTy)`, `N` as for [`Shape::Paddl`].
    Padal,
}

/// A floating-point width conversion, [`Shape::CvtFp`]: the
/// `vcvtfp2hf`/`vcvthf2fp` calls (bitcast to the declared type) or an
/// `fptrunc`/`fpext` (clang 19 `EmitAArch64BuiltinExpr`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Fp {
    /// `vcvt_f16_f32`: `vcvtfp2hf(bitcast v to <4 x float>)`.
    F16F32,
    /// `vcvt_f32_f16`: `vcvthf2fp(bitcast v to <4 x i16>)`.
    F32F16,
    /// `vcvt_f32_f64`: `fptrunc (bitcast v to <2 x double>) to <2 x float>`.
    F32F64,
    /// `vcvt_f64_f32`: `fpext (bitcast v to <2 x float>) to <2 x double>`.
    F64F32,
}

/// A horizontal reduction, [`Shape::Reduce`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Red {
    Max,
    Min,
    Add,
    /// `vaddlv`: the sum in the next wider element (`uaddlv`/`saddlv`).
    AddLong,
}

/// An elementwise NEON intrinsic: its overloaded base names for a
/// signed-integer, unsigned-integer and floating-point `VTy` (empty: that
/// class is an invalid type code), the operand count before the type code,
/// and how the result is returned.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct Call {
    s: &'static str,
    u: &'static str,
    f: &'static str,
    arity: usize,
    /// Bitcast to the builtin's declared type (clang
    /// `EmitCommonNeonBuiltinExpr`'s default), else `VTy` as is (clang's
    /// own `EmitAArch64BuiltinExpr` case).
    declared: bool,
    /// The first operand is passed last (`vfma`'s accumulator).
    acc_last: bool,
}

/// The [`Shape::Call`] builtin of base name `b` (`vmax` for `vmax_v`,
/// `vmaxq_v`, `vmax_f16`, `vmaxq_f16`; clang 19 `EmitAArch64BuiltinExpr`
/// and `AArch64SIMDIntrinsicMap`).
fn call(b: &str) -> Option<Call> {
    let c = |s, u, f, arity| Call { s, u, f, arity, declared: false, acc_last: false };
    let d = |s, u, f, arity| Call { declared: true, ..c(s, u, f, arity) };
    let i2 = |s, u| c(s, u, "", 2);
    Some(match b {
        "vabs" => c("llvm.aarch64.neon.abs", "llvm.aarch64.neon.abs", "llvm.fabs", 1),
        "vabd" => {
            c("llvm.aarch64.neon.sabd", "llvm.aarch64.neon.uabd", "llvm.aarch64.neon.fabd", 2)
        }
        "vclz" => d("llvm.ctlz", "llvm.ctlz", "", 1),
        "vcnt" => c("llvm.ctpop", "llvm.ctpop", "", 1),
        "vfma" => Call { acc_last: true, ..c("", "", "llvm.fma", 3) },
        "vmax" => {
            c("llvm.aarch64.neon.smax", "llvm.aarch64.neon.umax", "llvm.aarch64.neon.fmax", 2)
        }
        "vmaxnm" => c("", "", "llvm.aarch64.neon.fmaxnm", 2),
        "vmin" => {
            c("llvm.aarch64.neon.smin", "llvm.aarch64.neon.umin", "llvm.aarch64.neon.fmin", 2)
        }
        "vminnm" => c("", "", "llvm.aarch64.neon.fminnm", 2),
        "vpadd" => {
            d("llvm.aarch64.neon.addp", "llvm.aarch64.neon.addp", "llvm.aarch64.neon.faddp", 2)
        }
        "vpmax" => {
            c("llvm.aarch64.neon.smaxp", "llvm.aarch64.neon.umaxp", "llvm.aarch64.neon.fmaxp", 2)
        }
        "vpmin" => {
            c("llvm.aarch64.neon.sminp", "llvm.aarch64.neon.uminp", "llvm.aarch64.neon.fminp", 2)
        }
        "vqadd" => {
            Call { declared: true, ..i2("llvm.aarch64.neon.sqadd", "llvm.aarch64.neon.uqadd") }
        }
        "vhadd" => {
            Call { declared: true, ..i2("llvm.aarch64.neon.shadd", "llvm.aarch64.neon.uhadd") }
        }
        "vrhadd" => {
            Call { declared: true, ..i2("llvm.aarch64.neon.srhadd", "llvm.aarch64.neon.urhadd") }
        }
        "vhsub" => {
            Call { declared: true, ..i2("llvm.aarch64.neon.shsub", "llvm.aarch64.neon.uhsub") }
        }
        // The shifts by a signed per-lane register count.
        "vshl" => Call { declared: true, ..i2("llvm.aarch64.neon.sshl", "llvm.aarch64.neon.ushl") },
        "vrshl" => {
            Call { declared: true, ..i2("llvm.aarch64.neon.srshl", "llvm.aarch64.neon.urshl") }
        }
        "vqshl" => {
            Call { declared: true, ..i2("llvm.aarch64.neon.sqshl", "llvm.aarch64.neon.uqshl") }
        }
        "vqrshl" => {
            Call { declared: true, ..i2("llvm.aarch64.neon.sqrshl", "llvm.aarch64.neon.uqrshl") }
        }
        "vqabs" => d("llvm.aarch64.neon.sqabs", "", "", 1),
        "vqneg" => d("llvm.aarch64.neon.sqneg", "", "", 1),
        "vqdmulh" => d("llvm.aarch64.neon.sqdmulh", "", "", 2),
        "vqrdmulh" => d("llvm.aarch64.neon.sqrdmulh", "", "", 2),
        "vqsub" => {
            Call { declared: true, ..i2("llvm.aarch64.neon.sqsub", "llvm.aarch64.neon.uqsub") }
        }
        "vrecpe" => c("", "llvm.aarch64.neon.urecpe", "llvm.aarch64.neon.frecpe", 1),
        "vrecps" => d("", "", "llvm.aarch64.neon.frecps", 2),
        "vrsqrte" => c("", "llvm.aarch64.neon.ursqrte", "llvm.aarch64.neon.frsqrte", 1),
        "vrsqrts" => d("", "", "llvm.aarch64.neon.frsqrts", 2),
        "vrnd" => c("", "", "llvm.trunc", 1),
        "vrnda" => c("", "", "llvm.round", 1),
        "vrndm" => c("", "", "llvm.floor", 1),
        "vrndn" => c("", "", "llvm.roundeven", 1),
        "vrndp" => c("", "", "llvm.ceil", 1),
        "vsqrt" => c("", "", "llvm.sqrt", 1),
        // `vcmla[q][_rot{90,180,270}]_{f16,f32,f64}`.
        "vcmla" => d("", "", "llvm.aarch64.neon.vcmla.rot0", 3),
        "vcmla_rot90" | "vcmlaq_rot90" => d("", "", "llvm.aarch64.neon.vcmla.rot90", 3),
        "vcmla_rot180" | "vcmlaq_rot180" => d("", "", "llvm.aarch64.neon.vcmla.rot180", 3),
        "vcmla_rot270" | "vcmlaq_rot270" => d("", "", "llvm.aarch64.neon.vcmla.rot270", 3),
        _ => return None,
    })
}

/// The two-register permutes, clang 19 `EmitCommonNeonBuiltinExpr`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Perm {
    /// `vzip`: interleave the low halves, then the high halves.
    Zip,
    /// `vuzp`: the even lanes, then the odd lanes.
    Uzp,
    /// `vtrn`: the even lanes of each pair transposed, then the odd.
    Trn,
}

impl Perm {
    /// Shuffle mask `vi` (0 or 1) over two `n`-lane vectors.
    fn mask(self, vi: u32, n: u32) -> Vec<u32> {
        match self {
            Perm::Zip => (0..n)
                .step_by(2)
                .flat_map(|i| {
                    let x = (i + vi * n) >> 1;
                    [x, x + n]
                })
                .collect(),
            Perm::Uzp => (0..n).map(|i| 2 * i + vi).collect(),
            Perm::Trn => (0..n).step_by(2).flat_map(|i| [i + vi, i + n + vi]).collect(),
        }
    }
}

impl Shape {
    /// The number of operands, the trailing type code included.
    const fn arity(self) -> usize {
        match self {
            Shape::LdrQ | Shape::Reduce(..) => 1,
            Shape::LdN(_) => 3,
            Shape::StN(k) => k as usize + 2,
            Shape::Call(c) => c.arity + 1,
            Shape::Bsl | Shape::Sra | Shape::Sri | Shape::Sli => 4,
            Shape::FmaLane { .. } => 5,
            Shape::Ld1
            | Shape::Ld1Dup
            | Shape::GetLane(_)
            | Shape::Movn
            | Shape::StrQ
            | Shape::CvtF
            | Shape::CvtI
            | Shape::Movl
            | Shape::Paddl
            | Shape::QMovn
            | Shape::QMovun
            | Shape::CvtFp(_) => 2,
            Shape::St1 | Shape::SetLane(_) | Shape::Splat { .. } | Shape::ShrN | Shape::Tst => 3,
            Shape::ShlN | Shape::ShrnN | Shape::Mull => 3,
            Shape::ShllN | Shape::RshrnN | Shape::Padal => 3,
            Shape::QShrnN { .. } | Shape::ShlCall { .. } => 3,
            Shape::CvtR(_) => 2,
            Shape::Ld1Lane | Shape::St1Lane | Shape::Ext | Shape::Perm(_) => 4,
            Shape::Tbl(k) => k as usize + 2,
        }
    }

    /// The indices of the integer immediates (lane, type code), which
    /// codegen folds (clang `EmitScalarOrConstFoldImmArg`).
    const fn imm_args(self) -> &'static [usize] {
        match self {
            Shape::Ld1
            | Shape::Ld1Dup
            | Shape::GetLane(_)
            | Shape::Movn
            | Shape::CvtF
            | Shape::CvtI
            | Shape::Movl
            | Shape::Paddl
            | Shape::QMovn
            | Shape::QMovun
            | Shape::CvtFp(_) => &[1],
            Shape::Call(c) => match c.arity {
                1 => &[1],
                2 => &[2],
                _ => &[3],
            },
            Shape::Bsl => &[3],
            Shape::St1 | Shape::SetLane(_) | Shape::Tst | Shape::Mull => &[2],
            Shape::Ld1Lane | Shape::St1Lane | Shape::Ext => &[2, 3],
            Shape::Splat { .. } | Shape::ShrN | Shape::ShlN | Shape::ShrnN => &[1, 2],
            Shape::ShllN | Shape::RshrnN | Shape::QShrnN { .. } => &[1, 2],
            Shape::ShlCall { .. } => &[1, 2],
            Shape::CvtR(_) => &[1],
            Shape::Padal | Shape::Tbl(1) => &[2],
            Shape::Tbl(_) => &[3],
            Shape::Sra | Shape::Sri | Shape::Sli => &[2, 3],
            Shape::FmaLane { .. } => &[3, 4],
            Shape::Perm(_) => &[3],
            Shape::LdN(_) => &[2],
            Shape::StN(2) => &[3],
            Shape::StN(3) => &[4],
            Shape::StN(_) => &[5],
            Shape::LdrQ | Shape::StrQ | Shape::Reduce(..) => &[],
        }
    }
}

/// The argument indices of NEON builtin `b`'s immediates codegen folds,
/// for the fold prepass (empty for any other builtin).
pub(crate) fn imm_args(b: u32) -> &'static [usize] {
    neon::row(b).and_then(|r| shape(r.info.name)).map_or(&[], Shape::imm_args)
}

/// The rounding letter of `vcvt{n,a,m,p}[q]_{s,u}{32,64}` ([`Shape::CvtR`]).
fn cvt_rounding(b: &str) -> Option<u8> {
    let (op, ty) = b.split_once('_')?;
    let op = op.strip_suffix('q').unwrap_or(op);
    let r = match op {
        "vcvtn" => b'n',
        "vcvta" => b'a',
        "vcvtm" => b'm',
        "vcvtp" => b'p',
        _ => return None,
    };
    matches!(ty, "s32" | "u32" | "s64" | "u64").then_some(r)
}

/// The [`Shape`] of NEON builtin `name`, if it has one.
fn shape(name: &str) -> Option<Shape> {
    let n = name.strip_prefix(neon::PREFIX)?;
    match n {
        "vldrq_p128" => return Some(Shape::LdrQ),
        "vstrq_p128" => return Some(Shape::StrQ),
        _ => {}
    }
    if let Some(b) = n.strip_suffix("_v").or_else(|| n.strip_suffix("_bf16")) {
        let s = match b {
            "vld1" | "vld1q" => Some(Shape::Ld1),
            "vst1" | "vst1q" => Some(Shape::St1),
            "vld1_lane" | "vld1q_lane" => Some(Shape::Ld1Lane),
            "vst1_lane" | "vst1q_lane" => Some(Shape::St1Lane),
            "vld1_dup" | "vld1q_dup" => Some(Shape::Ld1Dup),
            "vld2" | "vld2q" => Some(Shape::LdN(2)),
            "vld3" | "vld3q" => Some(Shape::LdN(3)),
            "vld4" | "vld4q" => Some(Shape::LdN(4)),
            "vst2" | "vst2q" => Some(Shape::StN(2)),
            "vst3" | "vst3q" => Some(Shape::StN(3)),
            "vst4" | "vst4q" => Some(Shape::StN(4)),
            "splat_lane" | "splatq_laneq" => Some(Shape::Splat { mul: 1, div: 1 }),
            "splatq_lane" => Some(Shape::Splat { mul: 2, div: 1 }),
            "splat_laneq" => Some(Shape::Splat { mul: 1, div: 2 }),
            "vmovn" if n.ends_with("_v") => Some(Shape::Movn),
            "vext" | "vextq" if n.ends_with("_v") => Some(Shape::Ext),
            "vzip" | "vzipq" if n.ends_with("_v") => Some(Shape::Perm(Perm::Zip)),
            "vuzp" | "vuzpq" if n.ends_with("_v") => Some(Shape::Perm(Perm::Uzp)),
            "vtrn" | "vtrnq" if n.ends_with("_v") => Some(Shape::Perm(Perm::Trn)),
            "vshr_n" | "vshrq_n" if n.ends_with("_v") => Some(Shape::ShrN),
            "vbsl" | "vbslq" if n.ends_with("_v") => Some(Shape::Bsl),
            "vtst" | "vtstq" if n.ends_with("_v") => Some(Shape::Tst),
            "vshl_n" | "vshlq_n" if n.ends_with("_v") => Some(Shape::ShlN),
            "vshrn_n" if n.ends_with("_v") => Some(Shape::ShrnN),
            "vsra_n" | "vsraq_n" if n.ends_with("_v") => Some(Shape::Sra),
            "vsri_n" | "vsriq_n" if n.ends_with("_v") => Some(Shape::Sri),
            "vsli_n" | "vsliq_n" if n.ends_with("_v") => Some(Shape::Sli),
            "vmovl" if n.ends_with("_v") => Some(Shape::Movl),
            "vpaddl" | "vpaddlq" if n.ends_with("_v") => Some(Shape::Paddl),
            "vqmovn" if n.ends_with("_v") => Some(Shape::QMovn),
            "vmull" if n.ends_with("_v") => Some(Shape::Mull),
            "vtbl1" if n.ends_with("_v") => Some(Shape::Tbl(1)),
            "vtbl2" if n.ends_with("_v") => Some(Shape::Tbl(2)),
            "vqrshrn_n" if n.ends_with("_v") => Some(Shape::QShrnN {
                s: "llvm.aarch64.neon.sqrshrn",
                u: "llvm.aarch64.neon.uqrshrn",
            }),
            "vqshrn_n" if n.ends_with("_v") => {
                Some(Shape::QShrnN { s: "llvm.aarch64.neon.sqshrn", u: "llvm.aarch64.neon.uqshrn" })
            }
            "vqrshrun_n" if n.ends_with("_v") => {
                let i = "llvm.aarch64.neon.sqrshrun";
                Some(Shape::QShrnN { s: i, u: i })
            }
            "vqshrun_n" if n.ends_with("_v") => {
                let i = "llvm.aarch64.neon.sqshrun";
                Some(Shape::QShrnN { s: i, u: i })
            }
            "vqshl_n" | "vqshlq_n" if n.ends_with("_v") => Some(Shape::ShlCall {
                s: "llvm.aarch64.neon.sqshl",
                u: "llvm.aarch64.neon.uqshl",
                right: false,
            }),
            "vrshr_n" | "vrshrq_n" if n.ends_with("_v") => Some(Shape::ShlCall {
                s: "llvm.aarch64.neon.srshl",
                u: "llvm.aarch64.neon.urshl",
                right: true,
            }),
            "vshll_n" if n.ends_with("_v") => Some(Shape::ShllN),
            "vrshrn_n" if n.ends_with("_v") => Some(Shape::RshrnN),
            "vqmovun" if n.ends_with("_v") => Some(Shape::QMovun),
            "vpadal" | "vpadalq" if n.ends_with("_v") => Some(Shape::Padal),
            "vcvt_f32" | "vcvtq_f32" | "vcvt_f64" | "vcvtq_f64" if n.ends_with("_v") => {
                Some(Shape::CvtF)
            }
            "vcvt_s32" | "vcvtq_s32" | "vcvt_u32" | "vcvtq_u32" | "vcvt_s64" | "vcvtq_s64"
            | "vcvt_u64" | "vcvtq_u64"
                if n.ends_with("_v") =>
            {
                Some(Shape::CvtI)
            }
            _ if n.ends_with("_v") => cvt_rounding(b).map(Shape::CvtR),
            _ => None,
        };
        if s.is_some() {
            return s;
        }
    }
    match n {
        "vcvt_f16_s16" | "vcvt_f16_u16" | "vcvtq_f16_s16" | "vcvtq_f16_u16" => {
            return Some(Shape::CvtF);
        }
        "vcvt_s16_f16" | "vcvt_u16_f16" | "vcvtq_s16_f16" | "vcvtq_u16_f16" => {
            return Some(Shape::CvtI);
        }
        "vfma_lane_v" | "vfmaq_lane_v" | "vfma_lane_f16" | "vfmaq_lane_f16" => {
            return Some(Shape::FmaLane { laneq: false });
        }
        "vfma_laneq_v" | "vfmaq_laneq_v" | "vfma_laneq_f16" | "vfmaq_laneq_f16" => {
            return Some(Shape::FmaLane { laneq: true });
        }
        "vcvt_f16_f32" => return Some(Shape::CvtFp(Fp::F16F32)),
        "vcvt_f32_f16" => return Some(Shape::CvtFp(Fp::F32F16)),
        "vcvt_f32_f64" => return Some(Shape::CvtFp(Fp::F32F64)),
        "vcvt_f64_f32" => return Some(Shape::CvtFp(Fp::F64F32)),
        _ => {}
    }
    // The elementwise calls: `vmax_v`, `vmaxq_v`, and the `_f16` twins
    // (`vcmla` also `_f32`, `_f64`).
    let cmla = n.starts_with("vcmla");
    let base = n.strip_suffix("_v").or_else(|| n.strip_suffix("_f16")).or_else(|| {
        cmla.then(|| n.strip_suffix("_f32").or_else(|| n.strip_suffix("_f64"))).flatten()
    });
    if let Some(c) = base.and_then(|b| call(b.strip_suffix('q').unwrap_or(b))) {
        return Some(Shape::Call(c));
    }
    // The reductions: `vmaxvq_u8`, `vaddv_f32`, ...
    if let Some((op, sfx)) = n.split_once('_') {
        let (op, quad) = op.strip_suffix('q').map_or((op, false), |o| (o, true));
        let red = match op {
            "vmaxv" => Some(Red::Max),
            "vminv" => Some(Red::Min),
            "vaddv" => Some(Red::Add),
            "vaddlv" => Some(Red::AddLong),
            _ => None,
        };
        if let Some(red) = red {
            let (unsigned, e) = match sfx.split_at_checked(1)? {
                ("s", b) => (false, E::from_suffix(&format!("i{b}"))?),
                ("u", b) => (true, E::from_suffix(&format!("i{b}"))?),
                ("f", _) if red != Red::AddLong => (false, E::from_suffix(sfx)?),
                _ => return None,
            };
            return Some(Shape::Reduce(red, T::vec(e, quad), unsigned));
        }
    }
    // The fixed-type lane builtins: `vget_lane_i8`, `vgetq_lane_f64`,
    // `vdups_laneq_f32`, `vset_lane_bf16`, ...
    let (op, rest) = n.split_once('_')?;
    let (lane, sfx) = rest.split_once('_')?;
    let e = E::from_suffix(sfx)?;
    let quad = match (op, lane) {
        ("vget" | "vset" | "vdupb" | "vduph" | "vdups" | "vdupd", "lane") => false,
        ("vgetq" | "vsetq", "lane") | ("vdupb" | "vduph" | "vdups" | "vdupd", "laneq") => true,
        _ => return None,
    };
    let t = T::vec(e, quad);
    Some(if op.starts_with("vset") { Shape::SetLane(t) } else { Shape::GetLane(t) })
}

impl Cg<'_> {
    fn neon_ty(&mut self, t: T) -> LTy {
        let elt = |e: E| match e {
            E::I8 => LTy::I8,
            E::I16 => LTy::I16,
            E::I32 => LTy::I32,
            E::I64 => LTy::I64,
            E::F16 => LTy::HALF,
            E::Bf16 => LTy::BFLOAT,
            E::F32 => LTy::FLOAT,
            E::F64 => LTy::DOUBLE,
        };
        match t {
            T::S(e) => elt(e),
            T::V(e, n) => self.m.types.vector(elt(e), n),
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
            LTy::I128 => Some(128),
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
        if let Some(s) = shape(name) {
            return Ok(BuiltinCall::Emitted(self.emit_neon_shape(s, name, e)?));
        }
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
        // clang's `vbfdot` and `vdot` cases return the intrinsic's
        // result as is (the header casts it to its vector type at once; a
        // store or cast of it goes by its own type, as in clang); the
        // others go through `EmitCommonNeonBuiltinExpr`'s bitcast to the
        // builtin's declared type.
        if ["bfdot.", "sdot.", "udot."]
            .iter()
            .any(|d| intr.strip_prefix("llvm.aarch64.neon.").is_some_and(|r| r.starts_with(d)))
        {
            return Ok(BuiltinCall::Emitted(Some(r)));
        }
        Ok(BuiltinCall::Emitted(Some(self.neon_bitcast(r, ret, rty, e)?)))
    }

    /// The integer immediate operand `a` (clang
    /// `EmitScalarOrConstFoldImmArg`; sema has checked it is constant).
    fn neon_imm(&mut self, a: ExprId, name: &str) -> R<i128> {
        match self.fold_int(a) {
            Some(v) => Ok(v),
            None => err(self.loc_of(a), format!("argument to {name} must be a constant integer")),
        }
    }

    /// `v` bitcast to `to`, by its own type: a builtin result that kept
    /// its intrinsic's type (`vld1`, `vbfdot`) is not its declared type.
    fn neon_as(&mut self, v: V, to: LTy, e: ExprId) -> R<V> {
        let from = self.f.fir.ty(&self.m, v);
        self.neon_bitcast(v, from, to, e)
    }

    /// The constant `<n x et>` splat of `k`.
    fn neon_splat(&mut self, et: LTy, n: u32, k: i128, e: ExprId, name: &str) -> R<V> {
        let c = self.m.const_int(et, k);
        match self.m.const_vector(&vec![c; n as usize]) {
            Ok(s) => Ok(V::C(s)),
            Err(_) => err(self.loc_of(e), format!("invalid type code for {name}")),
        }
    }

    /// `v` (of `<n x elt>`) shifted right by immediate `k` (clang
    /// `EmitNeonRShiftImm`): `lshr`/`ashr` (unsigned/signed type `code`)
    /// by `splat(k)`, `k` in `1..=width`; a full-width shift is zero
    /// (unsigned) or a shift by width - 1 (signed).
    fn neon_rshift(
        &mut self,
        v: V,
        k: i128,
        code: i128,
        (elt, n): (E, u32),
        at: ExprId,
        name: &str,
    ) -> R<V> {
        let bits = i128::from(elt.bits());
        if !is_int(elt) || !(1..=bits).contains(&k) {
            return err(self.loc_of(at), format!("invalid shift for {name}"));
        }
        let vt = self.neon_ty(T::V(elt, n));
        let unsigned = code & 0x10 != 0;
        if unsigned && k == bits {
            return Ok(V::C(self.m.const_zero(vt)));
        }
        let k = if k == bits { k - 1 } else { k };
        let et = self.neon_ty(T::S(elt));
        let s = self.neon_splat(et, n, k, at, name)?;
        let op = if unsigned { BinOp::LShr } else { BinOp::AShr };
        Ok(self.bin(op, Flags::NONE, v, s))
    }

    /// The call `e` of NEON builtin `name` of shape `s`, emitted in
    /// clang's order: the operands left to right (a pointer with its
    /// alignment), then the casts and the operation. A result keeps the
    /// vector type the operation produced (clang returns it unconverted;
    /// the header's cast to the user type is a bitcast or nothing).
    fn emit_neon_shape(&mut self, s: Shape, name: &str, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        if args.len() != s.arity() {
            return err(self.loc_of(e), format!("wrong number of arguments to {name}"));
        }
        let mut code = 0;
        let t = match s {
            Shape::LdrQ => {
                let p = self.emit_scalar(args[0])?;
                return Ok(Some(self.load(LTy::I128, p, 16, false)));
            }
            Shape::StrQ => {
                let p = self.emit_scalar(args[0])?;
                let v = self.emit_scalar(args[1])?;
                self.store(v, p, 16, false);
                return Ok(None);
            }
            Shape::GetLane(t) | Shape::SetLane(t) | Shape::Reduce(_, t, _) => Some(t),
            _ => {
                code = self.neon_imm(args[s.arity() - 1], name)?;
                type_code(code)
            }
        };
        let Some(T::V(elt, n)) = t else {
            return err(self.loc_of(e), format!("invalid type code for {name}"));
        };
        let vt = self.neon_ty(T::V(elt, n));
        let et = self.neon_ty(T::S(elt));
        let i32c = |cg: &mut Self, x: i128| V::C(cg.m.const_int(LTy::I32, x));
        Ok(match s {
            Shape::Ld1 => {
                let (p, a) = self.pointer_and_align(args[0])?;
                Some(self.load(vt, p, a, false))
            }
            Shape::St1 => {
                let (p, a) = self.pointer_and_align(args[0])?;
                let v = self.emit_scalar(args[1])?;
                let v = self.neon_as(v, vt, args[1])?;
                self.store(v, p, a, false);
                None
            }
            Shape::Ld1Lane => {
                let (p, a) = self.pointer_and_align(args[0])?;
                let v = self.emit_scalar(args[1])?;
                let lane = self.neon_imm(args[2], name)?;
                let v = self.neon_as(v, vt, args[1])?;
                let x = self.load(et, p, a, false);
                let lane = i32c(self, lane);
                Some(self.insert_elt(v, x, lane))
            }
            Shape::St1Lane => {
                let (p, a) = self.pointer_and_align(args[0])?;
                let v = self.emit_scalar(args[1])?;
                let lane = self.neon_imm(args[2], name)?;
                let v = self.neon_as(v, vt, args[1])?;
                let lane = i32c(self, lane);
                let x = self.extract_elt(v, lane);
                self.store(x, p, a, false);
                None
            }
            Shape::LdN(k) => {
                // `Ops[1] = call(F, Ops[1]); CreateDefaultAlignedStore(Ops[1], Ops[0])`.
                let r = self.emit_scalar(args[0])?;
                let p = self.emit_scalar(args[1])?;
                let pt = self.m.types.ptr(0);
                let st = self.m.types.struct_literal(&vec![vt; k as usize], false);
                let base = format!("llvm.aarch64.neon.ld{k}");
                let full = intrinsics::overloaded_name(&self.m, &base, &[vt, pt]);
                let f = intrinsics::target_intrinsic(
                    &mut self.m,
                    &full,
                    st,
                    &[(pt, false)],
                    intrinsics::TargetMem::ArgRead,
                );
                let v = self.intrinsic_call(f, vec![p]);
                // The struct's ABI alignment is its 64- or 128-bit vector's.
                self.store(v, r, u64::from(elt.bits() * n / 8), false);
                None
            }
            Shape::StN(k) => {
                // `std::rotate(Ops, Ops + 1)`: the vectors, then the pointer.
                // Every operand is emitted before the vectors are bitcast.
                let p = self.emit_scalar(args[0])?;
                let vs = &args[1..=k as usize];
                let mut raw = Vec::with_capacity(vs.len());
                for &a in vs {
                    raw.push(self.emit_scalar(a)?);
                }
                let mut vals = Vec::with_capacity(vs.len() + 1);
                for (v, &a) in raw.into_iter().zip(vs) {
                    vals.push(self.neon_as(v, vt, a)?);
                }
                vals.push(p);
                let pt = self.m.types.ptr(0);
                let base = format!("llvm.aarch64.neon.st{k}");
                let full = intrinsics::overloaded_name(&self.m, &base, &[vt, pt]);
                let f = intrinsics::neon_store_n(&mut self.m, &full, vt, k as usize);
                self.intrinsic_call(f, vals);
                None
            }
            Shape::Ld1Dup => {
                let (p, a) = self.pointer_and_align(args[0])?;
                let x = self.load(et, p, a, false);
                let undef = V::C(self.m.const_poison(vt));
                let zero = i32c(self, 0);
                let v = self.insert_elt(undef, x, zero);
                Some(self.shuffle(v, Some(v), vec![0; n as usize]))
            }
            Shape::GetLane(_) => {
                let v = self.emit_scalar(args[0])?;
                let v = self.neon_as(v, vt, args[0])?;
                let lane = self.neon_imm(args[1], name)?;
                let lane = i32c(self, lane);
                Some(self.extract_elt(v, lane))
            }
            Shape::SetLane(_) => {
                let x = self.emit_scalar(args[0])?;
                let v = self.emit_scalar(args[1])?;
                let v = self.neon_as(v, vt, args[1])?;
                let lane = self.neon_imm(args[2], name)?;
                let lane = i32c(self, lane);
                Some(self.insert_elt(v, x, lane))
            }
            Shape::Splat { mul, div } => {
                let v = self.emit_scalar(args[0])?;
                let lane = self.neon_imm(args[1], name)?;
                let v = self.neon_as(v, vt, args[0])?;
                let Ok(lane) = u32::try_from(lane) else {
                    return err(self.loc_of(args[1]), format!("invalid lane for {name}"));
                };
                Some(self.shuffle(v, Some(v), vec![lane; (n * mul / div) as usize]))
            }
            Shape::LdrQ | Shape::StrQ => None,
            Shape::Ext => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let k = self.neon_imm(args[2], name)?;
                let a = self.neon_as(a, vt, args[0])?;
                let b = self.neon_as(b, vt, args[1])?;
                let Some(k) = u32::try_from(k).ok().filter(|&k| k < n) else {
                    return err(self.loc_of(args[2]), format!("invalid lane for {name}"));
                };
                Some(self.shuffle(a, Some(b), (k..k + n).collect()))
            }
            Shape::Perm(perm) => {
                let p = self.emit_scalar(args[0])?;
                let a = self.emit_scalar(args[1])?;
                let b = self.emit_scalar(args[2])?;
                let a = self.neon_as(a, vt, args[1])?;
                let b = self.neon_as(b, vt, args[2])?;
                let align = u64::from(elt.bits() * n / 8);
                for vi in 0..2 {
                    let i = i32c(self, i128::from(vi));
                    let at = self.gep(vt, p, vec![i], true);
                    let v = self.shuffle(a, Some(b), perm.mask(vi, n));
                    self.store(v, at, align, false);
                }
                None
            }
            Shape::ShrN => {
                let v = self.emit_scalar(args[0])?;
                let k = self.neon_imm(args[1], name)?;
                let v = self.neon_as(v, vt, args[0])?;
                Some(self.neon_rshift(v, k, code, (elt, n), args[1], name)?)
            }
            Shape::Movn => {
                let Some(w) = elt.widened() else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let v = self.emit_scalar(args[0])?;
                let wt = self.neon_ty(T::V(w, n));
                let v = self.neon_as(v, wt, args[0])?;
                Some(self.cast(CastOp::Trunc, v, vt))
            }
            Shape::Call(c) => {
                let unsigned = code & 0x10 != 0;
                let base = match elt {
                    E::F16 | E::F32 | E::F64 => c.f,
                    E::I8 | E::I16 | E::I32 | E::I64 if unsigned => c.u,
                    E::I8 | E::I16 | E::I32 | E::I64 => c.s,
                    E::Bf16 => "",
                };
                if base.is_empty() {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                }
                let mut vals = Vec::with_capacity(c.arity);
                for &a in &args[..c.arity] {
                    vals.push(self.emit_scalar(a)?);
                }
                for (v, &a) in vals.iter_mut().zip(&args) {
                    *v = self.neon_as(*v, vt, a)?;
                }
                if c.acc_last {
                    vals.rotate_left(1);
                }
                let f = if base == "llvm.ctlz" {
                    vals.push(V::C(self.m.const_int(LTy::I1, 0)));
                    intrinsics::ctlz(&mut self.m, vt)
                } else if base.starts_with("llvm.aarch64.") {
                    let full = intrinsics::overloaded_name(&self.m, base, &[vt]);
                    intrinsics::target_nomem(&mut self.m, &full, vt, &vec![vt; c.arity])
                } else {
                    intrinsics::fp_op(&mut self.m, base, vt, c.arity)
                };
                let r = self.intrinsic_call(f, vals);
                if c.declared {
                    let rty = self.lty(ast.exprs.get(e).ty());
                    Some(self.neon_bitcast(r, vt, rty, e)?)
                } else {
                    Some(r)
                }
            }
            Shape::Tst => {
                if !is_int(elt) {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                }
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let a = self.neon_as(a, vt, args[0])?;
                let b = self.neon_as(b, vt, args[1])?;
                let x = self.bin(BinOp::And, Flags::NONE, a, b);
                let zero = V::C(self.m.const_zero(vt));
                let c = self.icmp(IntPredicate::Ne, x, zero);
                Some(self.cast(CastOp::SExt, c, vt))
            }
            Shape::Bsl => {
                let Some(bt) = int_elt(elt.bits()).map(|i| self.neon_ty(T::V(i, n))) else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let mut vals = Vec::with_capacity(3);
                for &a in &args[..3] {
                    vals.push(self.emit_scalar(a)?);
                }
                for (v, &a) in vals.iter_mut().zip(&args) {
                    *v = self.neon_as(*v, bt, a)?;
                }
                let [m, a, b] = [vals[0], vals[1], vals[2]];
                let Some(ie) = int_elt(elt.bits()) else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let it = self.neon_ty(T::S(ie));
                let c = self.m.const_int(it, -1);
                let Ok(ones) = self.m.const_vector(&vec![c; n as usize]) else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let x = self.bin(BinOp::And, Flags::NONE, m, a);
                let not = self.bin(BinOp::Xor, Flags::NONE, m, V::C(ones));
                let y = self.bin(BinOp::And, Flags::NONE, not, b);
                let r = self.bin(BinOp::Or, Flags::NONE, x, y);
                Some(self.neon_bitcast(r, bt, vt, e)?)
            }
            Shape::CvtF | Shape::CvtI | Shape::CvtR(_) => {
                let Some(fe) = float_elt(elt) else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let ft = self.neon_ty(T::V(fe, n));
                let unsigned = code & 0x10 != 0;
                let v = self.emit_scalar(args[0])?;
                if s == Shape::CvtF {
                    let v = self.neon_as(v, vt, args[0])?;
                    let op = if unsigned { CastOp::UIToFP } else { CastOp::SIToFP };
                    Some(self.cast(op, v, ft))
                } else {
                    let v = self.neon_as(v, ft, args[0])?;
                    let r = if let Shape::CvtR(r) = s { char::from(r) } else { 'z' };
                    let base =
                        format!("llvm.aarch64.neon.fcvt{r}{}", if unsigned { 'u' } else { 's' });
                    let full = intrinsics::overloaded_name(&self.m, &base, &[vt, ft]);
                    let f = intrinsics::target_nomem(&mut self.m, &full, vt, &[ft]);
                    Some(self.intrinsic_call(f, vec![v]))
                }
            }
            Shape::ShlN => {
                let v = self.emit_scalar(args[0])?;
                let k = self.neon_imm(args[1], name)?;
                if !is_int(elt) || !(0..i128::from(elt.bits())).contains(&k) {
                    return err(self.loc_of(args[1]), format!("invalid shift for {name}"));
                }
                let v = self.neon_as(v, vt, args[0])?;
                let s = self.neon_splat(et, n, k, e, name)?;
                Some(self.bin(BinOp::Shl, Flags::NONE, v, s))
            }
            Shape::ShrnN => {
                let Some(w) = elt.widened() else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let v = self.emit_scalar(args[0])?;
                let k = self.neon_imm(args[1], name)?;
                if !(1..=i128::from(elt.bits())).contains(&k) {
                    return err(self.loc_of(args[1]), format!("invalid shift for {name}"));
                }
                let wt = self.neon_ty(T::V(w, n));
                let wet = self.neon_ty(T::S(w));
                let v = self.neon_as(v, wt, args[0])?;
                let s = self.neon_splat(wet, n, k, e, name)?;
                let op = if code & 0x10 != 0 { BinOp::LShr } else { BinOp::AShr };
                let r = self.bin(op, Flags::NONE, v, s);
                Some(self.cast(CastOp::Trunc, r, vt))
            }
            Shape::Sra => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let k = self.neon_imm(args[2], name)?;
                let a = self.neon_as(a, vt, args[0])?;
                let b = self.neon_as(b, vt, args[1])?;
                let r = self.neon_rshift(b, k, code, (elt, n), args[2], name)?;
                Some(self.bin(BinOp::Add, Flags::NONE, a, r))
            }
            Shape::Sri | Shape::Sli => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let k = self.neon_imm(args[2], name)?;
                let bits = i128::from(elt.bits());
                let (range, base) = if s == Shape::Sri {
                    (1..=bits, "llvm.aarch64.neon.vsri")
                } else {
                    (0..=bits - 1, "llvm.aarch64.neon.vsli")
                };
                if !is_int(elt) || !range.contains(&k) {
                    return err(self.loc_of(args[2]), format!("invalid shift for {name}"));
                }
                let a = self.neon_as(a, vt, args[0])?;
                let b = self.neon_as(b, vt, args[1])?;
                let full = intrinsics::overloaded_name(&self.m, base, &[vt]);
                let f = intrinsics::target_nomem(&mut self.m, &full, vt, &[vt, vt, LTy::I32]);
                let k = i32c(self, k);
                Some(self.intrinsic_call(f, vec![a, b, k]))
            }
            Shape::Movl => {
                let half = matches!(elt, E::I16 | E::I32 | E::I64).then(|| int_elt(elt.bits() / 2));
                let Some(Some(h)) = half else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let v = self.emit_scalar(args[0])?;
                let ht = self.neon_ty(T::V(h, n));
                let v = self.neon_as(v, ht, args[0])?;
                let op = if code & 0x10 != 0 { CastOp::ZExt } else { CastOp::SExt };
                Some(self.cast(op, v, vt))
            }
            Shape::Paddl => {
                let half = matches!(elt, E::I16 | E::I32 | E::I64).then(|| int_elt(elt.bits() / 2));
                let Some(Some(h)) = half else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let v = self.emit_scalar(args[0])?;
                let nt = self.neon_ty(T::V(h, 2 * n));
                let v = self.neon_as(v, nt, args[0])?;
                let base = if code & 0x10 != 0 {
                    "llvm.aarch64.neon.uaddlp"
                } else {
                    "llvm.aarch64.neon.saddlp"
                };
                let full = intrinsics::overloaded_name(&self.m, base, &[vt, nt]);
                let f = intrinsics::target_nomem(&mut self.m, &full, vt, &[nt]);
                Some(self.intrinsic_call(f, vec![v]))
            }
            Shape::QMovn | Shape::QMovun => {
                let Some(w) = elt.widened() else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let v = self.emit_scalar(args[0])?;
                let wt = self.neon_ty(T::V(w, n));
                let v = self.neon_as(v, wt, args[0])?;
                let base = if s == Shape::QMovun {
                    "llvm.aarch64.neon.sqxtun"
                } else if code & 0x10 != 0 {
                    "llvm.aarch64.neon.uqxtn"
                } else {
                    "llvm.aarch64.neon.sqxtn"
                };
                let full = intrinsics::overloaded_name(&self.m, base, &[vt]);
                let f = intrinsics::target_nomem(&mut self.m, &full, vt, &[wt]);
                let r = self.intrinsic_call(f, vec![v]);
                let rty = self.lty(ast.exprs.get(e).ty());
                Some(self.neon_bitcast(r, vt, rty, e)?)
            }
            Shape::Tbl(k) => {
                if elt != E::I8 || n != 8 {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                }
                let t0 = self.emit_scalar(args[0])?;
                let t0 = self.neon_as(t0, vt, args[0])?;
                let t1 = if k == 2 {
                    let t1 = self.emit_scalar(args[1])?;
                    self.neon_as(t1, vt, args[1])?
                } else {
                    V::C(self.m.const_zero(vt))
                };
                let ix = usize::from(k);
                let i = self.emit_scalar(args[ix])?;
                let i = self.neon_as(i, vt, args[ix])?;
                let t = self.shuffle(t0, Some(t1), (0..16).collect());
                let tt = self.neon_ty(T::V(E::I8, 16));
                let f = intrinsics::target_nomem(
                    &mut self.m,
                    "llvm.aarch64.neon.tbl1.v8i8",
                    vt,
                    &[tt, vt],
                );
                Some(self.intrinsic_call(f, vec![t, i]))
            }
            Shape::ShllN => {
                let half = matches!(elt, E::I16 | E::I32 | E::I64).then(|| int_elt(elt.bits() / 2));
                let Some(Some(h)) = half else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let v = self.emit_scalar(args[0])?;
                let k = self.neon_imm(args[1], name)?;
                if !(0..=i128::from(h.bits())).contains(&k) {
                    return err(self.loc_of(args[1]), format!("invalid shift for {name}"));
                }
                let ht = self.neon_ty(T::V(h, n));
                let v = self.neon_as(v, ht, args[0])?;
                let op = if code & 0x10 != 0 { CastOp::ZExt } else { CastOp::SExt };
                let v = self.cast(op, v, vt);
                let s = self.neon_splat(et, n, k, e, name)?;
                Some(self.bin(BinOp::Shl, Flags::NONE, v, s))
            }
            Shape::RshrnN | Shape::QShrnN { .. } => {
                let Some(w) = elt.widened() else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let v = self.emit_scalar(args[0])?;
                let k = self.neon_imm(args[1], name)?;
                if !(1..=i128::from(elt.bits())).contains(&k) {
                    return err(self.loc_of(args[1]), format!("invalid shift for {name}"));
                }
                let wt = self.neon_ty(T::V(w, n));
                let v = self.neon_as(v, wt, args[0])?;
                let base = match s {
                    Shape::QShrnN { u, .. } if code & 0x10 != 0 => u,
                    Shape::QShrnN { s, .. } => s,
                    _ => "llvm.aarch64.neon.rshrn",
                };
                let full = intrinsics::overloaded_name(&self.m, base, &[vt]);
                let f = intrinsics::target_nomem(&mut self.m, &full, vt, &[wt, LTy::I32]);
                let k = i32c(self, k);
                Some(self.intrinsic_call(f, vec![v, k]))
            }
            Shape::ShlCall { s: si, u, right } => {
                let v = self.emit_scalar(args[0])?;
                let k = self.neon_imm(args[1], name)?;
                let bits = i128::from(elt.bits());
                let ok = if right { (1..=bits).contains(&k) } else { (0..bits).contains(&k) };
                if !is_int(elt) || !ok {
                    return err(self.loc_of(args[1]), format!("invalid shift for {name}"));
                }
                let v = self.neon_as(v, vt, args[0])?;
                let sh = self.neon_splat(et, n, if right { -k } else { k }, e, name)?;
                let base = if code & 0x10 != 0 { u } else { si };
                let full = intrinsics::overloaded_name(&self.m, base, &[vt]);
                let f = intrinsics::target_nomem(&mut self.m, &full, vt, &[vt, vt]);
                Some(self.intrinsic_call(f, vec![v, sh]))
            }
            Shape::Padal => {
                let half = matches!(elt, E::I16 | E::I32 | E::I64).then(|| int_elt(elt.bits() / 2));
                let Some(Some(h)) = half else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let nt = self.neon_ty(T::V(h, 2 * n));
                let b = self.neon_as(b, nt, args[1])?;
                let base = if code & 0x10 != 0 {
                    "llvm.aarch64.neon.uaddlp"
                } else {
                    "llvm.aarch64.neon.saddlp"
                };
                let full = intrinsics::overloaded_name(&self.m, base, &[vt, nt]);
                let f = intrinsics::target_nomem(&mut self.m, &full, vt, &[nt]);
                let t = self.intrinsic_call(f, vec![b]);
                let a = self.neon_as(a, vt, args[0])?;
                Some(self.bin(BinOp::Add, Flags::NONE, t, a))
            }
            // clang 19 `EmitAArch64BuiltinExpr` `vmull_v`: the result is
            // `VTy` as is.
            Shape::Mull => {
                let half = matches!(elt, E::I16 | E::I32 | E::I64).then(|| int_elt(elt.bits() / 2));
                let Some(Some(h)) = half else {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                };
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let ht = self.neon_ty(T::V(h, n));
                let a = self.neon_as(a, ht, args[0])?;
                let b = self.neon_as(b, ht, args[1])?;
                let base = if (4..=7).contains(&(code & 0xf)) {
                    "llvm.aarch64.neon.pmull"
                } else if code & 0x10 != 0 {
                    "llvm.aarch64.neon.umull"
                } else {
                    "llvm.aarch64.neon.smull"
                };
                let full = intrinsics::overloaded_name(&self.m, base, &[vt]);
                let f = intrinsics::target_nomem(&mut self.m, &full, vt, &[ht, ht]);
                Some(self.intrinsic_call(f, vec![a, b]))
            }
            Shape::FmaLane { laneq } => {
                if !matches!(elt, E::F16 | E::F32 | E::F64) {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                }
                let mut vals = Vec::with_capacity(3);
                for &a in &args[..3] {
                    vals.push(self.emit_scalar(a)?);
                }
                let [a, b, c] = [vals[0], vals[1], vals[2]];
                let lane = self.neon_imm(args[3], name)?;
                let quad = elt.bits() * n == 128;
                let src = match (laneq, quad) {
                    (true, _) => 128 / elt.bits(),
                    (false, true) => n / 2,
                    (false, false) => n,
                };
                // A lane of the two-`c` shuffle: clang's sema bounds a
                // `vfmaq_lane_v` lane by `VTy`, not `c` (`vfmaq_lane_f64`
                // takes lane 1 of a `float64x1_t`).
                let Some(lane) = u32::try_from(lane).ok().filter(|&l| l < 2 * src) else {
                    return err(self.loc_of(args[3]), format!("invalid lane for {name}"));
                };
                let st = self.neon_ty(T::V(elt, src));
                if !laneq {
                    // clang `EmitCommonNeonBuiltinExpr`: the lane first,
                    // then `EmitNeonCall`'s casts of `b` and `a`.
                    let c = self.neon_as(c, st, args[2])?;
                    let l = self.shuffle(c, Some(c), vec![lane; n as usize]);
                    let b = self.neon_as(b, vt, args[1])?;
                    let a = self.neon_as(a, vt, args[0])?;
                    let f = intrinsics::fp_op(&mut self.m, "llvm.fma", vt, 3);
                    Some(self.intrinsic_call(f, vec![b, l, a]))
                } else if n == 1 {
                    // `vfma_laneq_f64`: the scalar `fma.f64`.
                    let a = self.neon_as(a, et, args[0])?;
                    let b = self.neon_as(b, et, args[1])?;
                    let c = self.neon_as(c, st, args[2])?;
                    let lane = i32c(self, i128::from(lane));
                    let x = self.extract_elt(c, lane);
                    let f = intrinsics::fp_op(&mut self.m, "llvm.fma", et, 3);
                    let r = self.intrinsic_call(f, vec![b, x, a]);
                    Some(self.cast(CastOp::BitCast, r, vt))
                } else {
                    let a = self.neon_as(a, vt, args[0])?;
                    let b = self.neon_as(b, vt, args[1])?;
                    let c = self.neon_as(c, st, args[2])?;
                    let l = self.shuffle(c, Some(c), vec![lane; n as usize]);
                    let f = intrinsics::fp_op(&mut self.m, "llvm.fma", vt, 3);
                    Some(self.intrinsic_call(f, vec![l, b, a]))
                }
            }
            Shape::CvtFp(fp) => {
                let v = self.emit_scalar(args[0])?;
                let (from, to) = match fp {
                    Fp::F16F32 => (T::V(E::F32, 4), T::V(E::I16, 4)),
                    Fp::F32F16 => (T::V(E::I16, 4), T::V(E::F32, 4)),
                    Fp::F32F64 => (T::V(E::F64, 2), T::V(E::F32, 2)),
                    Fp::F64F32 => (T::V(E::F32, 2), T::V(E::F64, 2)),
                };
                let (from, to) = (self.neon_ty(from), self.neon_ty(to));
                let v = self.neon_as(v, from, args[0])?;
                let intr = match fp {
                    Fp::F16F32 => "llvm.aarch64.neon.vcvtfp2hf",
                    Fp::F32F16 => "llvm.aarch64.neon.vcvthf2fp",
                    Fp::F32F64 => return Ok(Some(self.cast(CastOp::FPTrunc, v, to))),
                    Fp::F64F32 => return Ok(Some(self.cast(CastOp::FPExt, v, to))),
                };
                let f = intrinsics::target_nomem(&mut self.m, intr, to, &[from]);
                let r = self.intrinsic_call(f, vec![v]);
                let rty = self.lty(ast.exprs.get(e).ty());
                Some(self.neon_bitcast(r, to, rty, e)?)
            }
            Shape::Reduce(red, _, unsigned) => {
                let float = matches!(elt, E::F16 | E::F32 | E::F64);
                let p = match (red, float, unsigned) {
                    (Red::Max, true, _) => "fmaxv",
                    (Red::Min, true, _) => "fminv",
                    (Red::Add, true, _) => "faddv",
                    (Red::Max, false, false) => "smaxv",
                    (Red::Max, false, true) => "umaxv",
                    (Red::Min, false, false) => "sminv",
                    (Red::Min, false, true) => "uminv",
                    (Red::Add, false, false) => "saddv",
                    (Red::Add, false, true) => "uaddv",
                    (Red::AddLong, _, false) => "saddlv",
                    (Red::AddLong, _, true) => "uaddlv",
                };
                let rt = match elt {
                    _ if float => et,
                    E::I64 => LTy::I64,
                    E::I32 if red == Red::AddLong => LTy::I64,
                    _ => LTy::I32,
                };
                // `vaddlv` returns the wider element (clang truncates the
                // `i32` sum of `u8` lanes to `i16`).
                let et = if red == Red::AddLong { self.lty(ast.exprs.get(e).ty()) } else { et };
                let v = self.emit_scalar(args[0])?;
                let v = self.neon_as(v, vt, args[0])?;
                let base = format!("llvm.aarch64.neon.{p}");
                let full = intrinsics::overloaded_name(&self.m, &base, &[rt, vt]);
                let f = intrinsics::target_nomem(&mut self.m, &full, rt, &[vt]);
                let r = self.intrinsic_call(f, vec![v]);
                Some(if rt == et { r } else { self.cast(CastOp::Trunc, r, et) })
            }
        })
    }
}

/// Whether `e` is an integer element.
const fn is_int(e: E) -> bool {
    matches!(e, E::I8 | E::I16 | E::I32 | E::I64)
}

/// The integer element of `bits` bits.
const fn int_elt(bits: u32) -> Option<E> {
    match bits {
        8 => Some(E::I8),
        16 => Some(E::I16),
        32 => Some(E::I32),
        64 => Some(E::I64),
        _ => None,
    }
}

/// The float element of integer element `e`'s width (`vcvt`).
const fn float_elt(e: E) -> Option<E> {
    match e {
        E::I16 => Some(E::F16),
        E::I32 => Some(E::F32),
        E::I64 => Some(E::F64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{E, Fp, Perm, Red, Shape, T, shape, type_code};

    fn s(n: &str) -> Option<Shape> {
        shape(&format!("__builtin_neon_{n}"))
    }

    #[test]
    fn type_codes() {
        assert_eq!(type_code(48), Some(T::V(E::I8, 16)));
        assert_eq!(type_code(16), Some(T::V(E::I8, 8)));
        assert_eq!(type_code(41), Some(T::V(E::F32, 4)));
        assert_eq!(type_code(10), Some(T::V(E::F64, 1)));
        assert_eq!(type_code(8), Some(T::V(E::F16, 4)));
        assert_eq!(type_code(38), Some(T::V(E::I64, 2)));
        assert_eq!(type_code(43), Some(T::V(E::Bf16, 8)));
        assert_eq!(type_code(7), Some(T::V(E::I8, 16)));
        assert_eq!(type_code(12), None);
        assert_eq!(type_code(64), None);
    }

    #[test]
    fn perm_masks() {
        assert_eq!(Perm::Zip.mask(0, 4), [0, 4, 1, 5]);
        assert_eq!(Perm::Zip.mask(1, 4), [2, 6, 3, 7]);
        assert_eq!(Perm::Uzp.mask(1, 4), [1, 3, 5, 7]);
        assert_eq!(Perm::Trn.mask(0, 4), [0, 4, 2, 6]);
        assert_eq!(Perm::Trn.mask(1, 4), [1, 5, 3, 7]);
    }

    #[test]
    fn shapes() {
        assert_eq!(s("vld1q_v"), Some(Shape::Ld1));
        assert_eq!(s("vld1_bf16"), Some(Shape::Ld1));
        assert_eq!(s("vst1q_lane_v"), Some(Shape::St1Lane));
        assert_eq!(s("splatq_lane_v"), Some(Shape::Splat { mul: 2, div: 1 }));
        assert_eq!(s("vgetq_lane_i64"), Some(Shape::GetLane(T::V(E::I64, 2))));
        assert_eq!(s("vdupb_laneq_i8"), Some(Shape::GetLane(T::V(E::I8, 16))));
        assert_eq!(s("vset_lane_f64"), Some(Shape::SetLane(T::V(E::F64, 1))));
        assert_eq!(s("vmovn_v"), Some(Shape::Movn));
        assert_eq!(s("vextq_v"), Some(Shape::Ext));
        assert_eq!(s("vuzp_v"), Some(Shape::Perm(Perm::Uzp)));
        assert_eq!(s("vshrq_n_v"), Some(Shape::ShrN));
        assert!(matches!(s("vmaxq_v"), Some(Shape::Call(c)) if c.f == "llvm.aarch64.neon.fmax"));
        assert!(matches!(s("vfma_f16"), Some(Shape::Call(c)) if c.acc_last && c.arity == 3));
        assert!(matches!(s("vpaddq_v"), Some(Shape::Call(c)) if c.declared));
        assert_eq!(s("vbslq_v"), Some(Shape::Bsl));
        assert_eq!(s("vtst_v"), Some(Shape::Tst));
        assert_eq!(s("vtstq_v"), Some(Shape::Tst));
        assert_eq!(s("vshlq_n_v"), Some(Shape::ShlN));
        assert_eq!(s("vshrn_n_v"), Some(Shape::ShrnN));
        assert_eq!(s("vsraq_n_v"), Some(Shape::Sra));
        assert_eq!(s("vsri_n_v"), Some(Shape::Sri));
        assert_eq!(s("vsliq_n_v"), Some(Shape::Sli));
        assert_eq!(s("vld2q_v"), Some(Shape::LdN(2)));
        assert_eq!(s("vst4_v"), Some(Shape::StN(4)));
        assert_eq!(s("vmovl_v"), Some(Shape::Movl));
        assert_eq!(s("vpaddlq_v"), Some(Shape::Paddl));
        assert!(matches!(s("vqdmulhq_v"), Some(Shape::Call(c)) if c.s.ends_with("sqdmulh")));
        assert_eq!(s("vqmovn_v"), Some(Shape::QMovn));
        assert_eq!(s("vmull_v"), Some(Shape::Mull));
        assert_eq!(s("vld4_v"), Some(Shape::LdN(4)));
        assert_eq!(s("vst3q_v"), Some(Shape::StN(3)));
        assert_eq!(s("vtbl2_v"), Some(Shape::Tbl(2)));
        assert_eq!(s("vcvtnq_u32_v"), Some(Shape::CvtR(b'n')));
        assert_eq!(s("vcvtm_s64_v"), Some(Shape::CvtR(b'm')));
        assert!(
            matches!(s("vqrshrun_n_v"), Some(Shape::QShrnN { u, .. }) if u.ends_with("sqrshrun"))
        );
        assert!(matches!(s("vrshrq_n_v"), Some(Shape::ShlCall { right: true, .. })));
        assert!(matches!(s("vhaddq_v"), Some(Shape::Call(c)) if c.u.ends_with("uhadd")));
        assert_eq!(s("vshll_n_v"), Some(Shape::ShllN));
        assert_eq!(s("vrshrn_n_v"), Some(Shape::RshrnN));
        assert_eq!(s("vqmovun_v"), Some(Shape::QMovun));
        assert_eq!(s("vpadalq_v"), Some(Shape::Padal));
        assert!(matches!(s("vqnegq_v"), Some(Shape::Call(c)) if c.s.ends_with("sqneg")));
        assert_eq!(s("vfmaq_lane_f16"), Some(Shape::FmaLane { laneq: false }));
        assert_eq!(s("vfma_laneq_v"), Some(Shape::FmaLane { laneq: true }));
        assert_eq!(s("vcvt_f32_f16"), Some(Shape::CvtFp(Fp::F32F16)));
        assert!(matches!(s("vclzq_v"), Some(Shape::Call(c)) if c.s == "llvm.ctlz" && c.declared));
        assert!(matches!(s("vrndn_f16"), Some(Shape::Call(c)) if c.f == "llvm.roundeven"));
        assert!(matches!(s("vcmlaq_rot90_f64"), Some(Shape::Call(c)) if c.arity == 3));
        assert!(matches!(s("vcmla_f32"), Some(Shape::Call(c)) if c.f.ends_with("rot0")));
        assert_eq!(s("vcvt_f32_f64_v"), None);
        assert_eq!(s("vcvtq_f32_v"), Some(Shape::CvtF));
        assert_eq!(s("vcvt_f16_u16"), Some(Shape::CvtF));
        assert_eq!(s("vcvtq_s16_f16"), Some(Shape::CvtI));
        assert_eq!(s("vmaxvq_u8"), Some(Shape::Reduce(Red::Max, T::V(E::I8, 16), true)));
        assert_eq!(s("vaddv_f32"), Some(Shape::Reduce(Red::Add, T::V(E::F32, 2), false)));
        assert_eq!(s("vaddlvq_u8"), Some(Shape::Reduce(Red::AddLong, T::V(E::I8, 16), true)));
        assert_eq!(s("vaddlv_s32"), Some(Shape::Reduce(Red::AddLong, T::V(E::I32, 2), false)));
        assert_eq!(s("vminv_s16"), Some(Shape::Reduce(Red::Min, T::V(E::I16, 4), false)));
        assert_eq!(s("vrndxq_v"), None);
        assert_eq!(s("vaddvq_p8"), None);
        assert_eq!(s("vld1_x2_v"), None);
        assert_eq!(s("vadd_v"), None);
        assert_eq!(s("vmovn_bf16"), None);
    }
}
