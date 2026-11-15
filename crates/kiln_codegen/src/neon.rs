//! `AArch64` NEON builtins (`__builtin_neon_*`; spec/landed/codegen/WP-558.md "Target
//! features", WP-558): the per-function feature check, then two tables
//! lowered as clang 19 `EmitAArch64BuiltinExpr`: the bf16 subset, an
//! intrinsic call each ([`lowering`], also `vqtbl1`, `vmull_p64`), and the
//! load/store, lane, splat, narrowing, permute and arithmetic builtins
//! ([`shape`], WP-628; spec/landed/codegen/WP-628.md), extended by WP-1421
//! with one-call builtins ([`Gen`]), short sequences ([`Sp`]), compares and
//! the multi-register loads and stores (spec/codegen.md, the NEON
//! builtins bullet). Any other NEON builtin is a "not supported yet" error when
//! its caller is emitted.

use kiln_ast::ExprId;
use kiln_ast::decl::builtin::neon;
use kiln_llvm::{
    BinOp, CastOp, Flags, FloatPredicate, IntPredicate, MemOpts, Ordering, SyncScope,
    TypeId as LTy, intrinsics,
};

use crate::builtins::BuiltinCall;
use crate::fir::{Op, V};
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
    const U32: T = T::S(E::I32);
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
        // The two- to four-register lookups and the lookups with a
        // fallback `vqtbx` (WP-1288): the tables are `<16 x i8>` each,
        // the index (and fallback) as wide as the result.
        "vqtbl2_v" => ("llvm.aarch64.neon.tbl2.v8i8", B8I, &[B16, B16, B8I]),
        "vqtbl2q_v" => ("llvm.aarch64.neon.tbl2.v16i8", B16, &[B16, B16, B16]),
        "vqtbl3_v" => ("llvm.aarch64.neon.tbl3.v8i8", B8I, &[B16, B16, B16, B8I]),
        "vqtbl3q_v" => ("llvm.aarch64.neon.tbl3.v16i8", B16, &[B16, B16, B16, B16]),
        "vqtbl4_v" => ("llvm.aarch64.neon.tbl4.v8i8", B8I, &[B16, B16, B16, B16, B8I]),
        "vqtbl4q_v" => ("llvm.aarch64.neon.tbl4.v16i8", B16, &[B16, B16, B16, B16, B16]),
        "vqtbx1_v" => ("llvm.aarch64.neon.tbx1.v8i8", B8I, &[B8I, B16, B8I]),
        "vqtbx1q_v" => ("llvm.aarch64.neon.tbx1.v16i8", B16, &[B16, B16, B16]),
        "vqtbx2_v" => ("llvm.aarch64.neon.tbx2.v8i8", B8I, &[B8I, B16, B16, B8I]),
        "vqtbx2q_v" => ("llvm.aarch64.neon.tbx2.v16i8", B16, &[B16, B16, B16, B16]),
        "vqtbx3_v" => ("llvm.aarch64.neon.tbx3.v8i8", B8I, &[B8I, B16, B16, B16, B8I]),
        "vqtbx3q_v" => ("llvm.aarch64.neon.tbx3.v16i8", B16, &[B16, B16, B16, B16, B16]),
        "vqtbx4_v" => ("llvm.aarch64.neon.tbx4.v8i8", B8I, &[B8I, B16, B16, B16, B16, B8I]),
        "vqtbx4q_v" => ("llvm.aarch64.neon.tbx4.v16i8", B16, &[B16, B16, B16, B16, B16, B16]),
        // The dot products (WP-803; clang `EmitCommonNeonBuiltinExpr`'s
        // `vdot` case): overloaded on the accumulator and the byte vector.
        "vdot_s32" => ("llvm.aarch64.neon.sdot.v2i32.v8i8", I2, &[I2, B8I, B8I]),
        "vdot_u32" => ("llvm.aarch64.neon.udot.v2i32.v8i8", I2, &[I2, B8I, B8I]),
        "vdotq_s32" => ("llvm.aarch64.neon.sdot.v4i32.v16i8", I4, &[I4, B16, B16]),
        "vdotq_u32" => ("llvm.aarch64.neon.udot.v4i32.v16i8", I4, &[I4, B16, B16]),
        // The 64-bit polynomial multiply: `<16 x i8>`, bitcast to `poly128_t`.
        "vmull_p64" => ("llvm.aarch64.neon.pmull64", B16, &[T::S(E::I64), T::S(E::I64)]),
        // The cryptographic extension (WP-811; clang `AArch64SIMDIntrinsicMap`
        // and `AArch64SISDIntrinsicMap`, `aarch64_crypto_*`, none overloaded;
        // the AES builtins' trailing type code is not passed).
        "vaeseq_u8" => ("llvm.aarch64.crypto.aese", B16, &[B16, B16]),
        "vaesdq_u8" => ("llvm.aarch64.crypto.aesd", B16, &[B16, B16]),
        "vaesmcq_u8" => ("llvm.aarch64.crypto.aesmc", B16, &[B16]),
        "vaesimcq_u8" => ("llvm.aarch64.crypto.aesimc", B16, &[B16]),
        "vsha1cq_u32" => ("llvm.aarch64.crypto.sha1c", I4, &[I4, U32, I4]),
        "vsha1mq_u32" => ("llvm.aarch64.crypto.sha1m", I4, &[I4, U32, I4]),
        "vsha1pq_u32" => ("llvm.aarch64.crypto.sha1p", I4, &[I4, U32, I4]),
        "vsha1h_u32" => ("llvm.aarch64.crypto.sha1h", U32, &[U32]),
        "vsha1su0q_u32" => ("llvm.aarch64.crypto.sha1su0", I4, &[I4, I4, I4]),
        "vsha1su1q_u32" => ("llvm.aarch64.crypto.sha1su1", I4, &[I4, I4]),
        "vsha256hq_u32" => ("llvm.aarch64.crypto.sha256h", I4, &[I4, I4, I4]),
        "vsha256h2q_u32" => ("llvm.aarch64.crypto.sha256h2", I4, &[I4, I4, I4]),
        "vsha256su0q_u32" => ("llvm.aarch64.crypto.sha256su0", I4, &[I4, I4]),
        "vsha256su1q_u32" => ("llvm.aarch64.crypto.sha256su1", I4, &[I4, I4, I4]),
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
    /// `vtbl{1,2,3,4}_v` (operands `t0`..`t{k-1}`, `i`; WP-803, WP-1288):
    /// `tbl{P}.v8i8(pairs, i)`, the 64-bit tables concatenated in pairs
    /// (the last padded with zero; clang `packTBLDVectorList`).
    Tbl(u8),
    /// `vtbx{1,2,3,4}_v` (operands `a`, `t0`..`t{k-1}`, `i`; WP-1288;
    /// clang `EmitAArch64TblBuiltinExpr`): for an even `k`,
    /// `tbx{k/2}.v8i8(a, pairs, i)`; for an odd `k`, the `tbl` of the
    /// padded pairs, with `a`'s byte where `i >= 8k` (an `and`/`or`
    /// select by `sext (icmp uge i, 8k)`).
    Tbx(u8),
    /// `vqmovun_v` (operand `v`; WP-803): as [`Shape::QMovn`] with
    /// `sqxtun`.
    QMovun,
    /// `vpadal[q]_v` (operands `a`, `b`; WP-803): `add ({s,u}addlp.VTy.N(b
    /// as N)), (bitcast a to VTy)`, `N` as for [`Shape::Paddl`].
    Padal,
    /// One intrinsic call, [`Gen`] (WP-1421).
    Gen(Gen),
    /// A short instruction sequence, [`Sp`] (WP-1421).
    Sp(Sp),
    /// A comparison, `sext` of an `icmp`/`fcmp` (WP-1421).
    Cmp(Cmp, CmpForm),
    /// `vld1[q]_x{k}_v`, `vld{k}[q]_dup_v`, `vld{k}[q]_lane_v` ([`Lk`];
    /// WP-1421): the intrinsic's struct stored to `r` as [`Shape::LdN`].
    LdX(Lk, u8),
    /// `vst1[q]_x{k}_v` (`false`; operands `p`, `v1`..`vk`) and
    /// `vst{k}[q]_lane_v` (`true`; then `lane`; WP-1421):
    /// `st1x{k}(v1 .. vk, p)`, `st{k}lane(v1 .. vk, i64 lane, p)`.
    StX(bool, u8),
    /// `vldap1[q]_lane_<t>` (operands `p`, `v`, `lane`; WP-1421):
    /// `insertelement v, (load atomic acquire of p), lane`.
    LdAcq,
    /// `vstl1[q]_lane_<t>` (operands `p`, `v`, `lane`; WP-1421): `store
    /// atomic release (extractelement v, lane), p`.
    StRel,
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

/// The type of an operand, result or overload of a [`Gen`] call: relative
/// to `VTy` (the trailing type code's vector), to operand 0's type, or to
/// the builtin's declared result type.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum G {
    /// `VTy`.
    V,
    /// The float vector of `VTy`'s shape (clang `GetFloatNeonType`).
    VF,
    /// `VTy`'s lanes at half the element width (clang
    /// `getTruncatedElementVectorType`).
    VN,
    /// `VTy`'s lanes at twice the element width.
    VW,
    /// `VTy` at twice the lanes (`vqdmulhq_lane_v`'s result).
    V2,
    /// The 64-bit vector of `VTy`'s element.
    VD,
    /// The 128-bit vector of `VTy`'s element.
    VQ,
    /// `VTy`'s width in `half` lanes (`vfmlal`).
    VH,
    /// `VTy`'s width in `i8` lanes (`vmmla`, `vusdot`).
    VB,
    /// Operand 0's type.
    A0,
    /// The vector of `bits` bits of operand 0's type, one lane for 0
    /// (clang `VectorizeArgTypes` with `Use64BitVectors`).
    A0v(u32),
    /// The declared result type.
    R,
    /// The vector of `bits` bits of the declared result type (clang
    /// `VectorizeRetType` with `Use64BitVectors`/`Use128BitVectors`).
    Rv(u32),
    /// The first overload type at twice the element width.
    O0W,
    /// The first overload type at half the element width.
    O0N,
    /// A fixed type.
    Fix(T),
}

/// How the last operand of a [`Gen`] call, an integer immediate `n`, is
/// passed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Imm {
    /// The last operand is not an immediate.
    No,
    /// `n` as its own type (`i32`), then converted to the parameter.
    Plain,
    /// As [`Imm::Plain`], to an `immarg` parameter.
    Arg,
    /// `-n` of the parameter type (clang's `vrshrd_n`).
    Neg,
    /// The splat of `n` (clang `EmitNeonShiftVector`).
    Splat,
}

/// A NEON builtin lowered to one intrinsic call (clang 19's
/// `AArch64SIMDIntrinsicMap` and `AArch64SISDIntrinsicMap` rows through
/// `EmitCommonNeonBuiltinExpr` / `EmitCommonNeonSISDBuiltinExpr`, and the
/// one-call cases of `EmitAArch64BuiltinExpr`; WP-1421). Each operand is
/// converted to its parameter as clang's `EmitNeonCall` and the SISD
/// path do: a bitcast between equal widths, a scalar inserted at lane 0
/// of a `poison` vector, an integer extended (signed or unsigned
/// builtin) or truncated.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct Gen {
    /// The intrinsic for a signed (or float) and an unsigned builtin; a
    /// name not starting `llvm.` is `llvm.aarch64.neon.<name>`.
    s: &'static str,
    u: &'static str,
    /// The overload types (empty: not overloaded).
    ovl: &'static [G],
    ret: G,
    /// One per operand before the type code.
    params: &'static [G],
    imm: Imm,
    /// The result is converted to the declared type (clang's generic
    /// tail: a bitcast, lane 0 of a wider vector, or a truncation), else
    /// returned as is (clang's `return EmitNeonCall(...)` cases).
    declared: bool,
    /// Operands 0 and 1 swap (`vcale`, `vcalt`: `facge`/`facgt` of
    /// `b`, `a`).
    swap: bool,
}

impl Gen {
    /// Whether the builtin takes a trailing type code (any type is
    /// relative to `VTy`).
    fn coded(&self) -> bool {
        let v = |g: &G| {
            matches!(g, G::V | G::VF | G::VN | G::VW | G::V2 | G::VD | G::VQ | G::VH | G::VB)
        };
        v(&self.ret) || self.ovl.iter().any(v) || self.params.iter().any(v)
    }

    /// The indices of the immediate and the type code.
    fn imm_args(&self) -> &'static [usize] {
        let np = self.params.len();
        match (self.imm != Imm::No, self.coded()) {
            (true, true) => two(np - 1),
            (true, false) => one(np - 1),
            (false, true) => one(np),
            (false, false) => &[],
        }
    }
}

/// The `T` of a fixed [`G`].
const F16X4: G = G::Fix(T::V(E::F16, 4));
const F16X8: G = G::Fix(T::V(E::F16, 8));
const I32: G = G::Fix(T::S(E::I32));
const I64: G = G::Fix(T::S(E::I64));

/// The [`Gen`] builtin `n` (without the `__builtin_neon_` prefix).
#[allow(clippy::too_many_lines, reason = "one table row per builtin family")]
fn gen_call(n: &str) -> Option<Gen> {
    use G::{A0, A0v, O0N, O0W, R, Rv, V, V2, VB, VD, VF, VH, VN, VQ, VW};
    let su = |s, u, ovl, ret, params| Gen {
        s,
        u,
        ovl,
        ret,
        params,
        imm: Imm::No,
        declared: true,
        swap: false,
    };
    let g = |s, ovl, ret, params| su(s, s, ovl, ret, params);
    // `as_is` for clang's `return EmitNeonCall(...)` cases.
    let as_is = |x: Gen| Gen { declared: false, ..x };
    let imm = |x: Gen, imm| Gen { imm, ..x };
    // The scalar (`s`, `d`) and the 64-bit-vector SISD forms (`b`, `h`).
    let sisd = |s, u, k: usize, b: &str| {
        let p: &'static [G] = match (k, b) {
            (1, "b" | "h") => &[A0v(64)],
            (2, "b" | "h") => &[A0v(64), A0v(64)],
            (3, "b" | "h") => &[A0v(64), A0v(64), A0v(64)],
            (1, _) => &[A0],
            (2, _) => &[A0, A0],
            _ => &[A0, A0, A0],
        };
        let o: &'static [G] = if matches!(b, "b" | "h") { &[A0v(64)] } else { &[A0] };
        su(s, u, o, o[0], p)
    };
    if let Some(c) = scalar_cvt_call(n) {
        return Some(c);
    }
    // The SISD integer families: `vqaddb_s8`, `vqshlh_n_u16`, ...
    let (op, rest) = n.split_once('_')?;
    let (stem, w) = op.split_at_checked(op.len().checked_sub(1)?)?;
    let scalar_int = rest.len() <= 5
        && matches!(w, "b" | "h" | "s" | "d")
        && matches!(rest.trim_start_matches("n_").as_bytes().first(), Some(b's' | b'u'));
    if scalar_int {
        let nimm = rest.starts_with("n_");
        let x = match (stem, nimm) {
            ("vqabs", false) => Some(sisd("sqabs", "sqabs", 1, w)),
            ("vqneg", false) => Some(sisd("sqneg", "sqneg", 1, w)),
            ("vqadd", false) => Some(sisd("sqadd", "uqadd", 2, w)),
            ("vqsub", false) => Some(sisd("sqsub", "uqsub", 2, w)),
            ("vqshl", false) => Some(sisd("sqshl", "uqshl", 2, w)),
            ("vqrshl", false) => Some(sisd("sqrshl", "uqrshl", 2, w)),
            ("vsqadd", false) => Some(sisd("usqadd", "usqadd", 2, w)),
            ("vuqadd", false) => Some(sisd("suqadd", "suqadd", 2, w)),
            ("vqdmulh", false) => Some(sisd("sqdmulh", "sqdmulh", 2, w)),
            ("vqrdmulh", false) => Some(sisd("sqrdmulh", "sqrdmulh", 2, w)),
            ("vqrdmlah", false) => Some(sisd("sqrdmlah", "sqrdmlah", 3, w)),
            ("vqrdmlsh", false) => Some(sisd("sqrdmlsh", "sqrdmlsh", 3, w)),
            ("vrshl", false) if w == "d" => Some(sisd("srshl", "urshl", 2, w)),
            ("vshl", false) if w == "d" => Some(sisd("sshl", "ushl", 2, w)),
            ("vqshl", true) => Some(imm(sisd("sqshl", "uqshl", 2, w), Imm::Plain)),
            ("vqshlu", true) => Some(imm(sisd("sqshlu", "sqshlu", 2, w), Imm::Plain)),
            ("vrshr", true) if w == "d" => Some(imm(sisd("srshl", "urshl", 2, w), Imm::Neg)),
            ("vqdmull", false) if w == "h" => Some(g("sqdmull", &[Rv(128)], Rv(128), &[O0N, O0N])),
            ("vqdmull", false) if w == "s" => Some(g("sqdmulls.scalar", &[], R, &[A0, A0])),
            ("vqmovn", false) if w == "d" => {
                Some(su("scalar.sqxtn", "scalar.uqxtn", &[R, A0], R, &[A0]))
            }
            ("vqmovun", false) if w == "d" => Some(g("scalar.sqxtun", &[R, A0], R, &[A0])),
            ("vqmovn", false) => Some(su("sqxtn", "uqxtn", &[Rv(64)], Rv(64), &[O0W])),
            ("vqmovun", false) => Some(g("sqxtun", &[Rv(64)], Rv(64), &[O0W])),
            _ => None,
        };
        let shrn = match stem {
            "vqshrn" => Some(("sqshrn", "uqshrn")),
            "vqrshrn" => Some(("sqrshrn", "uqrshrn")),
            "vqshrun" => Some(("sqshrun", "sqshrun")),
            "vqrshrun" => Some(("sqrshrun", "sqrshrun")),
            _ => None,
        };
        let x = match shrn {
            Some((s, u)) if nimm && w == "d" => Some(su(s, u, &[R], R, &[A0, I32])),
            Some((s, u)) if nimm => Some(su(s, u, &[Rv(64)], Rv(64), &[O0W, I32])),
            _ => x,
        };
        if let Some(x) = x {
            return Some(if shrn.is_some() { imm(x, Imm::Plain) } else { x });
        }
    }
    let fac = |s, swap| Gen { swap, ..as_is(g(s, &[V, VF], V, &[VF, VF])) };
    let fac_s = |s, swap, h: bool| Gen {
        swap,
        ..if h { g(s, &[I32, A0], I32, &[A0, A0]) } else { g(s, &[R, A0], R, &[A0, A0]) }
    };
    let crypto3 = |s| as_is(g(s, &[], V, &[V, V, V]));
    let lane = |s, ovl, ret, params| imm(as_is(g(s, ovl, ret, params)), Imm::Plain);
    let fml = |s| as_is(g(s, &[V, VH], V, &[V, VH, VH]));
    let mm = |s, u| as_is(su(s, u, &[V, VB], V, &[V, VB, VB]));
    let un = |s| g(s, &[V], V, &[V]);
    let bin = |s| g(s, &[V], V, &[V, V]);
    let f1 = |s| g(s, &[A0], A0, &[A0]);
    let f2 = |s| g(s, &[A0], A0, &[A0, A0]);
    let fv = |s| g(s, &[R, A0], R, &[A0]);
    Some(match n {
        "vbcaxq_s8" | "vbcaxq_s16" | "vbcaxq_s32" | "vbcaxq_s64" | "vbcaxq_u8" | "vbcaxq_u16"
        | "vbcaxq_u32" | "vbcaxq_u64" => {
            su("llvm.aarch64.crypto.bcaxs", "llvm.aarch64.crypto.bcaxu", &[V], V, &[V, V, V])
        }
        "veor3q_s8" | "veor3q_s16" | "veor3q_s32" | "veor3q_s64" | "veor3q_u8" | "veor3q_u16"
        | "veor3q_u32" | "veor3q_u64" => {
            su("llvm.aarch64.crypto.eor3s", "llvm.aarch64.crypto.eor3u", &[V], V, &[V, V, V])
        }
        "vrax1q_u64" => g("llvm.aarch64.crypto.rax1", &[], V, &[V, V]),
        "vxarq_u64" => imm(as_is(g("llvm.aarch64.crypto.xar", &[], V, &[V, V, I64])), Imm::Arg),
        "vsha512hq_u64" => crypto3("llvm.aarch64.crypto.sha512h"),
        "vsha512h2q_u64" => crypto3("llvm.aarch64.crypto.sha512h2"),
        "vsha512su0q_u64" => as_is(g("llvm.aarch64.crypto.sha512su0", &[], V, &[V, V])),
        "vsha512su1q_u64" => crypto3("llvm.aarch64.crypto.sha512su1"),
        "vsm3partw1q_u32" => crypto3("llvm.aarch64.crypto.sm3partw1"),
        "vsm3partw2q_u32" => crypto3("llvm.aarch64.crypto.sm3partw2"),
        "vsm3ss1q_u32" => crypto3("llvm.aarch64.crypto.sm3ss1"),
        "vsm3tt1aq_u32" | "vsm3tt1bq_u32" | "vsm3tt2aq_u32" | "vsm3tt2bq_u32" => {
            let s = match n {
                "vsm3tt1aq_u32" => "llvm.aarch64.crypto.sm3tt1a",
                "vsm3tt1bq_u32" => "llvm.aarch64.crypto.sm3tt1b",
                "vsm3tt2aq_u32" => "llvm.aarch64.crypto.sm3tt2a",
                _ => "llvm.aarch64.crypto.sm3tt2b",
            };
            imm(as_is(g(s, &[], V, &[V, V, V, I64])), Imm::Arg)
        }
        "vsm4eq_u32" => as_is(g("llvm.aarch64.crypto.sm4e", &[], V, &[V, V])),
        "vsm4ekeyq_u32" => as_is(g("llvm.aarch64.crypto.sm4ekey", &[], V, &[V, V])),
        "vcadd_rot90_f16" | "vcadd_rot90_f32" | "vcaddq_rot90_f16" | "vcaddq_rot90_f32"
        | "vcaddq_rot90_f64" => bin("vcadd.rot90"),
        "vcadd_rot270_f16" | "vcadd_rot270_f32" | "vcaddq_rot270_f16" | "vcaddq_rot270_f32"
        | "vcaddq_rot270_f64" => bin("vcadd.rot270"),
        "vcage_v" | "vcageq_v" | "vcage_f16" | "vcageq_f16" => fac("facge", false),
        "vcagt_v" | "vcagtq_v" | "vcagt_f16" | "vcagtq_f16" => fac("facgt", false),
        "vcale_v" | "vcaleq_v" | "vcale_f16" | "vcaleq_f16" => fac("facge", true),
        "vcalt_v" | "vcaltq_v" | "vcalt_f16" | "vcaltq_f16" => fac("facgt", true),
        "vcaged_f64" | "vcages_f32" => fac_s("facge", false, false),
        "vcagtd_f64" | "vcagts_f32" => fac_s("facgt", false, false),
        "vcaled_f64" | "vcales_f32" => fac_s("facge", true, false),
        "vcaltd_f64" | "vcalts_f32" => fac_s("facgt", true, false),
        "vcageh_f16" => fac_s("facge", false, true),
        "vcagth_f16" => fac_s("facgt", false, true),
        "vcaleh_f16" => fac_s("facge", true, true),
        "vcalth_f16" => fac_s("facgt", true, true),
        "vcls_v" | "vclsq_v" => un("cls"),
        "vcvt_n_f16_s16" | "vcvt_n_f16_u16" | "vcvtq_n_f16_s16" | "vcvtq_n_f16_u16"
        | "vcvt_n_f32_v" | "vcvtq_n_f32_v" | "vcvt_n_f64_v" | "vcvtq_n_f64_v" => {
            imm(as_is(su("vcvtfxs2fp", "vcvtfxu2fp", &[VF, V], VF, &[V, I32])), Imm::Plain)
        }
        "vcvt_n_s16_f16" | "vcvt_n_u16_f16" | "vcvtq_n_s16_f16" | "vcvtq_n_u16_f16"
        | "vcvt_n_s32_v" | "vcvtq_n_s32_v" | "vcvt_n_u32_v" | "vcvtq_n_u32_v" | "vcvt_n_s64_v"
        | "vcvtq_n_s64_v" | "vcvt_n_u64_v" | "vcvtq_n_u64_v" => {
            imm(as_is(su("vcvtfp2fxs", "vcvtfp2fxu", &[V, VF], V, &[VF, I32])), Imm::Plain)
        }
        "vcvtx_f32_v" => as_is(g("fcvtxn", &[VN, V], VN, &[V])),
        "vcvtxd_f32_f64" => g("llvm.aarch64.sisd.fcvtxn", &[], R, &[A0]),
        "vfmlal_low_f16" | "vfmlalq_low_f16" => fml("fmlal"),
        "vfmlal_high_f16" | "vfmlalq_high_f16" => fml("fmlal2"),
        "vfmlsl_low_f16" | "vfmlslq_low_f16" => fml("fmlsl"),
        "vfmlsl_high_f16" | "vfmlslq_high_f16" => fml("fmlsl2"),
        "vmmlaq_s32" | "vmmlaq_u32" => mm("smmla", "ummla"),
        "vusmmlaq_s32" => mm("usmmla", "usmmla"),
        "vusdot_s32" | "vusdotq_s32" => mm("usdot", "usdot"),
        "vmulx_v" | "vmulxq_v" | "vmulx_f16" | "vmulxq_f16" => as_is(bin("fmulx")),
        "vpmaxnm_v" | "vpmaxnmq_v" | "vpmaxnm_f16" | "vpmaxnmq_f16" => as_is(bin("fmaxnmp")),
        "vpminnm_v" | "vpminnmq_v" | "vpminnm_f16" | "vpminnmq_f16" => as_is(bin("fminnmp")),
        "vqdmulh_lane_v" => lane("sqdmulh.lane", &[V, VD], V, &[V, VD, I32]),
        "vqdmulhq_lane_v" => lane("sqdmulh.lane", &[V2, VD], V2, &[V2, VD, I32]),
        "vqdmulh_laneq_v" | "vqdmulhq_laneq_v" => lane("sqdmulh.laneq", &[V, VQ], V, &[V, VQ, I32]),
        "vqrdmulh_lane_v" => lane("sqrdmulh.lane", &[V, VD], V, &[V, VD, I32]),
        "vqrdmulhq_lane_v" => lane("sqrdmulh.lane", &[V2, VD], V2, &[V2, VD, I32]),
        "vqrdmulh_laneq_v" | "vqrdmulhq_laneq_v" => {
            lane("sqrdmulh.laneq", &[V, VQ], V, &[V, VQ, I32])
        }
        "vqdmull_v" => g("sqdmull", &[V], V, &[VN, VN]),
        "vqrdmlah_s16" | "vqrdmlah_s32" | "vqrdmlahq_s16" | "vqrdmlahq_s32" => {
            g("sqrdmlah", &[V], V, &[V, V, V])
        }
        "vqrdmlsh_s16" | "vqrdmlsh_s32" | "vqrdmlshq_s16" | "vqrdmlshq_s32" => {
            g("sqrdmlsh", &[V], V, &[V, V, V])
        }
        "vraddhn_v" => g("raddhn", &[V], V, &[VW, VW]),
        "vrsubhn_v" => g("rsubhn", &[V], V, &[VW, VW]),
        "vrnd32x_f32" | "vrnd32x_f64" | "vrnd32xq_f32" | "vrnd32xq_f64" => un("frint32x"),
        "vrnd32z_f32" | "vrnd32z_f64" | "vrnd32zq_f32" | "vrnd32zq_f64" => un("frint32z"),
        "vrnd64x_f32" | "vrnd64x_f64" | "vrnd64xq_f32" | "vrnd64xq_f64" => un("frint64x"),
        "vrnd64z_f32" | "vrnd64z_f64" | "vrnd64zq_f32" | "vrnd64zq_f64" => un("frint64z"),
        "vrndi_v" | "vrndiq_v" | "vrndi_f16" | "vrndiq_f16" => as_is(un("llvm.nearbyint")),
        "vrndx_v" | "vrndxq_v" | "vrndx_f16" | "vrndxq_f16" => as_is(un("llvm.rint")),
        "vrbit_v" | "vrbitq_v" => as_is(un("llvm.bitreverse")),
        "vsqadd_v" | "vsqaddq_v" => as_is(bin("usqadd")),
        "vuqadd_v" | "vuqaddq_v" => as_is(bin("suqadd")),
        "vmul_v" | "vmulq_v" => bin("pmul"),
        "vqshlu_n_v" | "vqshluq_n_v" => imm(as_is(bin("sqshlu")), Imm::Splat),
        "vslid_n_s64" | "vslid_n_u64" => {
            imm(g("vsli", &[A0v(0)], A0v(0), &[A0v(0), A0v(0), I32]), Imm::Plain)
        }
        "vsrid_n_s64" | "vsrid_n_u64" => {
            imm(g("vsri", &[A0v(0)], A0v(0), &[A0v(0), A0v(0), I32]), Imm::Plain)
        }
        "vabdd_f64" | "vabds_f32" | "vabdh_f16" => f2("llvm.aarch64.sisd.fabd"),
        "vabsd_s64" => f1("abs"),
        "vabsh_f16" => f1("llvm.fabs"),
        "vmaxh_f16" => f2("fmax"),
        "vminh_f16" => f2("fmin"),
        "vmaxnmh_f16" => f2("fmaxnm"),
        "vminnmh_f16" => f2("fminnm"),
        "vmulxd_f64" | "vmulxs_f32" | "vmulxh_f16" => f2("fmulx"),
        "vrecped_f64" | "vrecpes_f32" | "vrecpeh_f16" => f1("frecpe"),
        "vrecpsd_f64" | "vrecpss_f32" | "vrecpsh_f16" => f2("frecps"),
        "vrecpxd_f64" | "vrecpxs_f32" | "vrecpxh_f16" => f1("frecpx"),
        "vrsqrted_f64" | "vrsqrtes_f32" | "vrsqrteh_f16" => f1("frsqrte"),
        "vrsqrtsd_f64" | "vrsqrtss_f32" | "vrsqrtsh_f16" => f2("frsqrts"),
        "vrndah_f16" => f1("llvm.round"),
        "vrndh_f16" => f1("llvm.trunc"),
        "vrndih_f16" => f1("llvm.nearbyint"),
        "vrndmh_f16" => f1("llvm.floor"),
        "vrndnh_f16" | "vrndns_f32" => f1("llvm.roundeven"),
        "vrndph_f16" => f1("llvm.ceil"),
        "vrndxh_f16" => f1("llvm.rint"),
        "vsqrth_f16" => f1("llvm.sqrt"),
        "vmaxnmv_f32" | "vmaxnmvq_f32" | "vmaxnmvq_f64" | "vpmaxnms_f32" | "vpmaxnmqd_f64" => {
            fv("fmaxnmv")
        }
        "vminnmv_f32" | "vminnmvq_f32" | "vminnmvq_f64" | "vpminnms_f32" | "vpminnmqd_f64" => {
            fv("fminnmv")
        }
        "vpmaxs_f32" | "vpmaxqd_f64" => fv("fmaxv"),
        "vpmins_f32" | "vpminqd_f64" => fv("fminv"),
        "vpaddd_s64" | "vpaddd_u64" => fv("uaddv"),
        "vmaxnmv_f16" => g("fmaxnmv", &[R, F16X4], R, &[F16X4]),
        "vmaxnmvq_f16" => g("fmaxnmv", &[R, F16X8], R, &[F16X8]),
        "vminnmv_f16" => g("fminnmv", &[R, F16X4], R, &[F16X4]),
        "vminnmvq_f16" => g("fminnmv", &[R, F16X8], R, &[F16X8]),
        _ => return None,
    })
}

/// The scalar conversions `vcvt[r]{s,d,h}[_n]_<to>_<from>` that are one
/// call (clang's `AArch64SISDIntrinsicMap` rows and its `vcvt*h_*16_f16`
/// cases, whose 16-bit result is an `i32` call truncated; the 16-bit
/// source of `vcvth_n_f16_{s,u}16` is extended to `i32`).
fn scalar_cvt_call(n: &str) -> Option<Gen> {
    let (to, from, r, fixed) = scalar_cvt_parts(n)?;
    let g = |s, u, ovl, ret, params, imm| Gen {
        s,
        u,
        ovl,
        ret,
        params,
        imm,
        declared: true,
        swap: false,
    };
    let int16 = |t: &str| t.get(1..) == Some("16");
    if to.starts_with('f') {
        // int -> float: only the fixed-point forms are calls.
        if !fixed {
            return None;
        }
        let (s, u) = ("vcvtfxs2fp", "vcvtfxu2fp");
        return Some(if int16(from) {
            g(s, u, &[G::R, I32], G::R, &[I32, I32], Imm::Plain)
        } else {
            g(s, u, &[G::R, G::A0], G::R, &[G::A0, I32], Imm::Plain)
        });
    }
    let (s, u) = match (fixed, r) {
        (true, _) => ("vcvtfp2fxs", "vcvtfp2fxu"),
        (false, b'a') => ("fcvtas", "fcvtau"),
        (false, b'm') => ("fcvtms", "fcvtmu"),
        (false, b'n') => ("fcvtns", "fcvtnu"),
        (false, b'p') => ("fcvtps", "fcvtpu"),
        (false, _) => ("fcvtzs", "fcvtzu"),
    };
    let ret = if int16(to) { I32 } else { G::R };
    let ovl: &'static [G] = if int16(to) { &[I32, G::A0] } else { &[G::R, G::A0] };
    Some(if fixed {
        g(s, u, ovl, ret, &[G::A0, I32], Imm::Plain)
    } else {
        g(s, u, ovl, ret, &[G::A0], Imm::No)
    })
}

/// `vcvt[r]{s,d,h}[_n]_<to>_<from>` split: the two type names, the
/// rounding letter (`z` for none) and whether it is fixed-point (`_n`).
fn scalar_cvt_parts(n: &str) -> Option<(&str, &str, u8, bool)> {
    let rest = n.strip_prefix("vcvt")?;
    let b = rest.as_bytes();
    let (r, rest) = match b {
        [r @ (b'a' | b'm' | b'n' | b'p'), b's' | b'd' | b'h', b'_', ..] => (*r, &rest[2..]),
        [b's' | b'd' | b'h', b'_', ..] => (b'z', &rest[1..]),
        _ => return None,
    };
    let rest = rest.strip_prefix('_')?;
    let (fixed, rest) = rest.strip_prefix("n_").map_or((false, rest), |r| (true, r));
    let (to, from) = rest.split_once('_')?;
    let ty = |t: &str| {
        matches!(t.as_bytes().first(), Some(b's' | b'u' | b'f'))
            && matches!(t.get(1..), Some("16" | "32" | "64"))
    };
    (ty(to) && ty(from) && (to.starts_with('f') != from.starts_with('f')))
        .then_some((to, from, r, fixed))
}

/// A comparison predicate, [`Shape::Cmp`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Cmp {
    Eq,
    Ge,
    Gt,
    Le,
    Lt,
}

/// The operand form of a [`Shape::Cmp`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum CmpForm {
    /// `vc<op>z[q]_{v,f16}` (operand `v`, type code): against zero, the
    /// `sext` to `VTy`.
    Vec,
    /// `vc<op>z{d,s,h}_<t>` (operand `x`): against zero, the `sext` to the
    /// declared type.
    Zero,
    /// `vc<op>{d,s,h}_<t>` (operands `a`, `b`).
    Two,
}

/// Which `ld{2,3,4}` variant a [`Shape::LdX`] is.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Lk {
    /// `vld1[q]_x{k}_v` (operands `r`, `p`): `ld1x{k}`.
    X1,
    /// `vld{k}[q]_dup_v` (operands `r`, `p`): `ld{k}r`.
    Dup,
    /// `vld{k}[q]_lane_v` (operands `r`, `p`, `v1`..`vk`, `lane`):
    /// `ld{k}lane(v1 .. vk, i64 lane, p)`.
    Lane,
}

/// The NEON builtins clang 19 `EmitAArch64BuiltinExpr` /
/// `EmitCommonNeonBuiltinExpr` lower to a short instruction sequence
/// (WP-1421; the operand names are the builtin's).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Sp {
    /// `vadd[q]_v` (`a`, `b`, code): `xor` as `<8|16 x i8>`, bitcast to
    /// `VTy`.
    AddV,
    /// `vaddq_p128` (`a`, `b`): `xor` as `<16 x i8>`, bitcast to `i128`.
    AddP128,
    /// `v{add,sub}hn_v` (`a`, `b`, code): `trunc (lshr (add|sub a, b),
    /// splat(w/2)) to VTy` over the two as `VTy` at twice the width.
    Hn(bool),
    /// `vrsra[q]_n_v` (`a`, `b`, `n`, code): `add a, {s,u}rshl(b,
    /// splat(-n))`.
    Rsra,
    /// `vqdml{a,s}l_v` (`acc`, `a`, `b`, code): `sq{add,sub}(acc,
    /// sqdmull(a, b))` on `VTy`, the factors at half the width.
    Qdmlal(bool),
    /// `vqdml{a,s}l{h_s16,s_s32}[_lane[q]]` (`acc`, `b`, `c`[, `lane`]):
    /// `sqdmull` of the two `i16` inserted at lane 0 (its lane 0), or
    /// `sqdmulls.scalar` of the `i32`, then `sq{add,sub}` with `acc`;
    /// with a lane, `c` is that lane of a vector.
    QdmlalS { sub: bool, lane: bool },
    /// `vmul_lane[q]_v` (`a`, `b`, `lane`, code): `fmul` of `a` as
    /// `double` and lane `lane` of `b` as `<1|2 x double>`, bitcast to
    /// `VTy`.
    MulLane(bool),
    /// `vmul_n_f64` (`a`, `x`): `fmul (bitcast a to double), x`.
    MulN,
    /// `vpaddd_f64`, `vpadds_f32` (`v`): `fadd` of lanes 0 and 1.
    PAddF,
    /// `vfma{h,s,d}_lane[q]_<t>` (`a`, `b`, `v`, `lane`): `fma(b,
    /// extractelement v, lane, a)`.
    FmaLaneS,
    /// `vmulxh_lane[q]_f16` (`a`, `v`, `lane`): `fmulx.f16(a,
    /// extractelement v, lane)`.
    MulxLane,
    /// `vfmah_f16`, `vfmsh_f16` (`a`, `b`, `c`): `fma(b, c, a)` (`b`
    /// negated); clang emits `b` a second time.
    Fmah(bool),
    /// `vaddd_<t>`, `vsubd_<t>`, `v{add,sub,mul,div}h_f16` (`a`, `b`).
    Bin(BinOp),
    /// `vnegd_s64` (`sub 0, x`), `vnegh_f16` (`fneg x`).
    Neg,
    /// `vshld_n_<t>` (`x`, `n`): `shl x, n`.
    ShlD,
    /// `vshrd_n_<t>` (`x`, `n`): `ashr x, min(n, 63)`; `lshr x, n`, zero
    /// for 64.
    ShrD,
    /// `vsrad_n_<t>` (`a`, `b`, `n`): `a` plus `b` shifted as by
    /// [`Sp::ShrD`]; `a` for an unsigned shift by 64.
    SraD,
    /// `vrsrad_n_<t>` (`a`, `b`, `n`): `add a, {s,u}rshl.i64(b, -n)`.
    RsraD,
    /// `vtstd_<t>` (`a`, `b`): `sext (icmp ne (and a, b), 0)`.
    TstD,
    /// `vcvt{s,d,h}_f<w>_{s,u}<w>` (`x`): `sitofp`/`uitofp` to the
    /// declared type.
    CvtS,
}

impl Sp {
    const fn arity(self) -> usize {
        match self {
            Sp::PAddF | Sp::Neg | Sp::CvtS => 1,
            Sp::AddP128 | Sp::MulN | Sp::Bin(_) | Sp::ShlD | Sp::ShrD | Sp::TstD => 2,
            Sp::AddV | Sp::Hn(_) | Sp::MulxLane | Sp::Fmah(_) | Sp::SraD | Sp::RsraD => 3,
            Sp::QdmlalS { lane: false, .. } => 3,
            Sp::Rsra | Sp::Qdmlal(_) | Sp::QdmlalS { .. } | Sp::MulLane(_) | Sp::FmaLaneS => 4,
        }
    }

    const fn imm_args(self) -> &'static [usize] {
        match self {
            Sp::AddP128
            | Sp::MulN
            | Sp::Bin(_)
            | Sp::TstD
            | Sp::PAddF
            | Sp::Neg
            | Sp::CvtS
            | Sp::Fmah(_)
            | Sp::QdmlalS { lane: false, .. } => &[],
            Sp::ShlD | Sp::ShrD => &[1],
            Sp::AddV | Sp::Hn(_) | Sp::MulxLane | Sp::SraD | Sp::RsraD => &[2],
            Sp::Qdmlal(_) | Sp::QdmlalS { .. } | Sp::FmaLaneS => &[3],
            Sp::Rsra | Sp::MulLane(_) => &[2, 3],
        }
    }
}

/// The [`Sp`] builtin `n`.
fn special(n: &str) -> Option<Sp> {
    Some(match n {
        "vadd_v" | "vaddq_v" => Sp::AddV,
        "vaddq_p128" => Sp::AddP128,
        "vaddhn_v" => Sp::Hn(false),
        "vsubhn_v" => Sp::Hn(true),
        "vrsra_n_v" | "vrsraq_n_v" => Sp::Rsra,
        "vqdmlal_v" => Sp::Qdmlal(false),
        "vqdmlsl_v" => Sp::Qdmlal(true),
        "vqdmlalh_s16" | "vqdmlals_s32" => Sp::QdmlalS { sub: false, lane: false },
        "vqdmlslh_s16" | "vqdmlsls_s32" => Sp::QdmlalS { sub: true, lane: false },
        "vqdmlalh_lane_s16" | "vqdmlalh_laneq_s16" | "vqdmlals_lane_s32" | "vqdmlals_laneq_s32" => {
            Sp::QdmlalS { sub: false, lane: true }
        }
        "vqdmlslh_lane_s16" | "vqdmlslh_laneq_s16" | "vqdmlsls_lane_s32" | "vqdmlsls_laneq_s32" => {
            Sp::QdmlalS { sub: true, lane: true }
        }
        "vmul_lane_v" => Sp::MulLane(false),
        "vmul_laneq_v" => Sp::MulLane(true),
        "vmul_n_f64" => Sp::MulN,
        "vpaddd_f64" | "vpadds_f32" => Sp::PAddF,
        "vfmah_lane_f16" | "vfmah_laneq_f16" | "vfmas_lane_f32" | "vfmas_laneq_f32"
        | "vfmad_lane_f64" | "vfmad_laneq_f64" => Sp::FmaLaneS,
        "vmulxh_lane_f16" | "vmulxh_laneq_f16" => Sp::MulxLane,
        "vfmah_f16" => Sp::Fmah(false),
        "vfmsh_f16" => Sp::Fmah(true),
        "vaddd_s64" | "vaddd_u64" => Sp::Bin(BinOp::Add),
        "vsubd_s64" | "vsubd_u64" => Sp::Bin(BinOp::Sub),
        "vaddh_f16" => Sp::Bin(BinOp::FAdd),
        "vsubh_f16" => Sp::Bin(BinOp::FSub),
        "vmulh_f16" => Sp::Bin(BinOp::FMul),
        "vdivh_f16" => Sp::Bin(BinOp::FDiv),
        "vnegd_s64" | "vnegh_f16" => Sp::Neg,
        "vshld_n_s64" | "vshld_n_u64" => Sp::ShlD,
        "vshrd_n_s64" | "vshrd_n_u64" => Sp::ShrD,
        "vsrad_n_s64" | "vsrad_n_u64" => Sp::SraD,
        "vrsrad_n_s64" | "vrsrad_n_u64" => Sp::RsraD,
        "vtstd_s64" | "vtstd_u64" => Sp::TstD,
        _ if scalar_cvt_parts(n)
            .is_some_and(|(to, _, r, fixed)| to.starts_with('f') && r == b'z' && !fixed) =>
        {
            Sp::CvtS
        }
        _ => return None,
    })
}

/// The [`Shape::Cmp`] builtin `n`: `vc<op>z[q]_{v,f16}`, `vc<op>z{d,s,h}_<t>`,
/// `vc<op>{d,s,h}_<t>`.
fn compare(n: &str) -> Option<(Cmp, CmpForm)> {
    let rest = n.strip_prefix("vc")?;
    let (op, rest) = rest.split_at_checked(2)?;
    let c = match op {
        "eq" => Cmp::Eq,
        "ge" => Cmp::Ge,
        "gt" => Cmp::Gt,
        "le" => Cmp::Le,
        "lt" => Cmp::Lt,
        _ => return None,
    };
    let (zero, rest) = rest.strip_prefix('z').map_or((false, rest), |r| (true, r));
    let form = match rest {
        "_v" | "q_v" | "_f16" | "q_f16" if zero => CmpForm::Vec,
        "d_s64" | "d_u64" | "d_f64" | "s_f32" | "h_f16" if zero => CmpForm::Zero,
        "d_s64" | "d_u64" | "d_f64" | "s_f32" | "h_f16" => CmpForm::Two,
        _ => return None,
    };
    Some((c, form))
}

/// Whether fixed-type NEON builtin `n` is unsigned (clang's separate
/// `_u` builtins): a `_u<width>` type in its name.
fn unsigned_name(n: &str) -> bool {
    n.split('_').any(|t| t.strip_prefix('u').is_some_and(|w| w.parse::<u32>().is_ok()))
}

/// `n` small integer argument lists ([`Shape::imm_args`]).
const fn one(i: usize) -> &'static [usize] {
    match i {
        0 => &[0],
        1 => &[1],
        2 => &[2],
        3 => &[3],
        4 => &[4],
        5 => &[5],
        6 => &[6],
        _ => &[7],
    }
}

const fn two(i: usize) -> &'static [usize] {
    match i {
        0 => &[0, 1],
        1 => &[1, 2],
        2 => &[2, 3],
        3 => &[3, 4],
        4 => &[4, 5],
        5 => &[5, 6],
        _ => &[6, 7],
    }
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
    fn arity(self) -> usize {
        match self {
            Shape::Gen(g) => g.params.len() + usize::from(g.coded()),
            Shape::Sp(s) => s.arity(),
            Shape::Cmp(_, CmpForm::Zero) => 1,
            Shape::Cmp(..) => 2,
            Shape::LdX(Lk::Lane, k) => k as usize + 4,
            Shape::LdX(..) => 3,
            Shape::StX(lane, k) => k as usize + 2 + usize::from(lane),
            Shape::LdAcq | Shape::StRel => 4,
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
            Shape::Tbx(k) => k as usize + 3,
        }
    }

    /// The indices of the integer immediates (lane, type code), which
    /// codegen folds (clang `EmitScalarOrConstFoldImmArg`).
    fn imm_args(self) -> &'static [usize] {
        match self {
            Shape::Gen(g) => g.imm_args(),
            Shape::Sp(s) => s.imm_args(),
            Shape::Cmp(_, CmpForm::Vec) => &[1],
            Shape::Cmp(..) => &[],
            Shape::LdX(Lk::Lane, k) => two(k as usize + 2),
            Shape::LdX(..) => &[2],
            Shape::StX(false, k) => one(k as usize + 1),
            Shape::StX(true, k) => two(k as usize + 1),
            Shape::LdAcq | Shape::StRel => &[2, 3],
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
            Shape::Tbl(2) | Shape::Tbx(1) => &[3],
            Shape::Tbl(3) | Shape::Tbx(2) => &[4],
            Shape::Tbl(_) | Shape::Tbx(3) => &[5],
            Shape::Tbx(_) => &[6],
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

/// The rounding letter of `vcvt{n,a,m,p}[q]_{s,u}{16,32,64}` ([`Shape::CvtR`]).
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
    matches!(ty, "s16" | "u16" | "s32" | "u32" | "s64" | "u64").then_some(r)
}

/// The [`Shape`] of NEON builtin `name`, if it has one.
fn shape(name: &str) -> Option<Shape> {
    let n = name.strip_prefix(neon::PREFIX)?;
    base_shape(n).or_else(|| more_shape(n))
}

/// The shapes of WP-1421: [`Sp`], [`Shape::Cmp`], the multi-vector
/// loads and stores, [`Gen`], and the `half` rounding conversions
/// `vcvt{a,m,n,p}[q]_{s,u}16_f16` ([`Shape::CvtR`]).
fn more_shape(n: &str) -> Option<Shape> {
    if let Some(s) = special(n) {
        return Some(Shape::Sp(s));
    }
    if let Some((c, f)) = compare(n) {
        return Some(Shape::Cmp(c, f));
    }
    if let Some(s) = ldst(n) {
        return Some(s);
    }
    if let Some(g) = gen_call(n) {
        return Some(Shape::Gen(g));
    }
    let b = n.strip_suffix("_f16")?;
    let (_, ty) = b.split_once('_')?;
    matches!(ty, "s16" | "u16").then_some(())?;
    cvt_rounding(b).map(Shape::CvtR)
}

/// The multi-vector load and store builtins ([`Shape::LdX`],
/// [`Shape::StX`]) and the acquire/release lane forms; a `bf16` builtin
/// is its `_v` twin (`vld1_bf16_x2` is `vld1_x2_v`).
fn ldst(n: &str) -> Option<Shape> {
    if n.starts_with("vldap1_lane_") || n.starts_with("vldap1q_lane_") {
        return Some(Shape::LdAcq);
    }
    if n.starts_with("vstl1_lane_") || n.starts_with("vstl1q_lane_") {
        return Some(Shape::StRel);
    }
    let b = match n.strip_suffix("_v").or_else(|| n.strip_suffix("_bf16")) {
        Some(b) => b.to_owned(),
        None if n.contains("_bf16_x") => n.replacen("_bf16", "", 1),
        None => return None,
    };
    let (op, rest) = b.split_once('_')?;
    let op = op.strip_suffix('q').unwrap_or(op);
    let k = |s: &str| s.parse::<u8>().ok().filter(|k| (2..=4).contains(k));
    match (op, rest) {
        ("vld1", x) => x.strip_prefix('x').and_then(k).map(|k| Shape::LdX(Lk::X1, k)),
        ("vst1", x) => x.strip_prefix('x').and_then(k).map(|k| Shape::StX(false, k)),
        (o, "dup") => o.strip_prefix("vld").and_then(k).map(|k| Shape::LdX(Lk::Dup, k)),
        (o, "lane") => match o.strip_prefix("vld").and_then(k) {
            Some(k) => Some(Shape::LdX(Lk::Lane, k)),
            None => o.strip_prefix("vst").and_then(k).map(|k| Shape::StX(true, k)),
        },
        _ => None,
    }
}

/// The shapes of WP-628 and later, before WP-1421's.
fn base_shape(n: &str) -> Option<Shape> {
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
            "vtbl3" if n.ends_with("_v") => Some(Shape::Tbl(3)),
            "vtbl4" if n.ends_with("_v") => Some(Shape::Tbl(4)),
            "vtbx1" if n.ends_with("_v") => Some(Shape::Tbx(1)),
            "vtbx2" if n.ends_with("_v") => Some(Shape::Tbx(2)),
            "vtbx3" if n.ends_with("_v") => Some(Shape::Tbx(3)),
            "vtbx4" if n.ends_with("_v") => Some(Shape::Tbx(4)),
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
            Shape::Gen(g) => return self.emit_neon_gen(g, name, &args, e).map(Some),
            Shape::Sp(sp) => return self.emit_neon_sp(sp, name, &args, e).map(Some),
            Shape::Cmp(c, f) => return self.emit_neon_cmp(c, f, name, &args, e).map(Some),
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
                let (p, a) = self.ptr_align(args[0])?;
                Some(self.load(vt, p, a, false))
            }
            Shape::St1 => {
                let (p, a) = self.ptr_align(args[0])?;
                let v = self.emit_scalar(args[1])?;
                let v = self.neon_as(v, vt, args[1])?;
                self.store(v, p, a, false);
                None
            }
            Shape::Ld1Lane => {
                let (p, a) = self.ptr_align(args[0])?;
                let v = self.emit_scalar(args[1])?;
                let lane = self.neon_imm(args[2], name)?;
                let v = self.neon_as(v, vt, args[1])?;
                let x = self.load(et, p, a, false);
                let lane = i32c(self, lane);
                Some(self.insert_elt(v, x, lane))
            }
            Shape::St1Lane => {
                let (p, a) = self.ptr_align(args[0])?;
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
                let (p, a) = self.ptr_align(args[0])?;
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
            Shape::LdrQ | Shape::StrQ | Shape::Gen(_) | Shape::Sp(_) | Shape::Cmp(..) => None,
            Shape::LdX(lk, k) => {
                // `Ops[1] = call(F, ...); CreateDefaultAlignedStore(Ops[1], Ops[0])`.
                let r = self.emit_scalar(args[0])?;
                let p = self.emit_scalar(args[1])?;
                let pt = self.m.types.ptr(0);
                let ku = usize::from(k);
                let st = self.m.types.struct_literal(&vec![vt; ku], false);
                let mut vals = Vec::with_capacity(ku + 2);
                let mut ps = Vec::with_capacity(ku + 2);
                let base = match lk {
                    Lk::X1 => format!("llvm.aarch64.neon.ld1x{k}"),
                    Lk::Dup => format!("llvm.aarch64.neon.ld{k}r"),
                    Lk::Lane => {
                        // `std::rotate(Ops + 1, Ops + 2)`: the vectors (as
                        // `VTy`), the lane (`zext` to `i64`), the pointer.
                        let vs = &args[2..2 + ku];
                        let mut raw = Vec::with_capacity(ku);
                        for &a in vs {
                            raw.push(self.emit_scalar(a)?);
                        }
                        let lane = self.neon_imm(args[2 + ku], name)?;
                        for (v, &a) in raw.into_iter().zip(vs) {
                            vals.push(self.neon_as(v, vt, a)?);
                            ps.push((vt, false));
                        }
                        vals.push(V::C(self.m.const_int(LTy::I64, lane)));
                        ps.push((LTy::I64, false));
                        format!("llvm.aarch64.neon.ld{k}lane")
                    }
                };
                vals.push(p);
                ps.push((pt, false));
                let full = intrinsics::overloaded_name(&self.m, &base, &[vt, pt]);
                let f = intrinsics::target_intrinsic(
                    &mut self.m,
                    &full,
                    st,
                    &ps,
                    intrinsics::TargetMem::ArgRead,
                );
                let v = self.intrinsic_call(f, vals);
                self.store(v, r, u64::from(elt.bits() * n / 8), false);
                None
            }
            Shape::StX(lane, k) => {
                // `std::rotate(Ops, Ops + 1)`: the vectors, [the lane,] the
                // pointer; every operand is emitted before the casts.
                let p = self.emit_scalar(args[0])?;
                let ku = usize::from(k);
                let vs = &args[1..=ku];
                let mut raw = Vec::with_capacity(ku);
                for &a in vs {
                    raw.push(self.emit_scalar(a)?);
                }
                let l = if lane { Some(self.neon_imm(args[ku + 1], name)?) } else { None };
                let mut vals = Vec::with_capacity(ku + 2);
                for (v, &a) in raw.into_iter().zip(vs) {
                    vals.push(self.neon_as(v, vt, a)?);
                }
                let pt = self.m.types.ptr(0);
                let f = if let Some(l) = l {
                    vals.push(V::C(self.m.const_int(LTy::I64, l)));
                    let base = format!("llvm.aarch64.neon.st{k}lane");
                    let full = intrinsics::overloaded_name(&self.m, &base, &[vt, pt]);
                    intrinsics::neon_store_lane(&mut self.m, &full, vt, ku)
                } else {
                    let base = format!("llvm.aarch64.neon.st1x{k}");
                    let full = intrinsics::overloaded_name(&self.m, &base, &[vt, pt]);
                    intrinsics::neon_store_n(&mut self.m, &full, vt, ku)
                };
                vals.push(p);
                self.intrinsic_call(f, vals);
                None
            }
            Shape::LdAcq | Shape::StRel => {
                let (p, a) = self.ptr_align(args[0])?;
                let v = self.emit_scalar(args[1])?;
                let lane = self.neon_imm(args[2], name)?;
                let v = self.neon_as(v, vt, args[1])?;
                let lane = i32c(self, lane);
                let acq = s == Shape::LdAcq;
                let mem = MemOpts {
                    align: a,
                    volatile: false,
                    ordering: if acq { Ordering::Acquire } else { Ordering::Release },
                    scope: SyncScope::SYSTEM,
                };
                if acq {
                    let x = self.f.fir.push(Op::Load { ty: et, ptr: p, mem }, et);
                    Some(self.insert_elt(v, x, lane))
                } else {
                    let x = self.extract_elt(v, lane);
                    self.f.fir.push(Op::Store { val: x, ptr: p, mem }, LTy::VOID);
                    None
                }
            }
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
            Shape::Tbl(k) | Shape::Tbx(k) => {
                if elt != E::I8 || n != 8 {
                    return err(self.loc_of(e), format!("invalid type code for {name}"));
                }
                let tbx = matches!(s, Shape::Tbx(_));
                let first = usize::from(tbx);
                let a = if tbx {
                    let a = self.emit_scalar(args[0])?;
                    Some(self.neon_as(a, vt, args[0])?)
                } else {
                    None
                };
                let k = usize::from(k);
                let mut tables = Vec::with_capacity(k);
                for &t in &args[first..first + k] {
                    let v = self.emit_scalar(t)?;
                    tables.push(self.neon_as(v, vt, t)?);
                }
                let ix = first + k;
                let i = self.emit_scalar(args[ix])?;
                let i = self.neon_as(i, vt, args[ix])?;
                // clang `packTBLDVectorList`: each pair as one 128-bit
                // table, an odd last one beside zero.
                let mut pairs = Vec::with_capacity(k.div_ceil(2));
                for c in tables.chunks(2) {
                    let hi = match c.get(1) {
                        Some(&t1) => t1,
                        None => V::C(self.m.const_zero(vt)),
                    };
                    pairs.push(self.shuffle(c[0], Some(hi), (0..16).collect()));
                }
                let tt = self.neon_ty(T::V(E::I8, 16));
                let np = pairs.len();
                if let Some(a) = a
                    && k % 2 == 0
                {
                    let full = format!("llvm.aarch64.neon.tbx{np}.v8i8");
                    let mut ps = vec![vt];
                    ps.extend(std::iter::repeat_n(tt, np));
                    ps.push(vt);
                    let f = intrinsics::target_nomem(&mut self.m, &full, vt, &ps);
                    let mut vals = vec![a];
                    vals.extend(pairs);
                    vals.push(i);
                    Some(self.intrinsic_call(f, vals))
                } else {
                    let full = format!("llvm.aarch64.neon.tbl{np}.v8i8");
                    let mut ps: Vec<LTy> = std::iter::repeat_n(tt, np).collect();
                    ps.push(vt);
                    let f = intrinsics::target_nomem(&mut self.m, &full, vt, &ps);
                    let mut vals = pairs;
                    vals.push(i);
                    let r = self.intrinsic_call(f, vals);
                    match a {
                        None => Some(r),
                        Some(a) => {
                            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                            let lim = (8 * k) as i128;
                            let bound = self.neon_splat(et, n, lim, e, name)?;
                            let c = self.icmp(IntPredicate::Uge, i, bound);
                            let m = self.cast(CastOp::SExt, c, vt);
                            let ones = self.neon_splat(et, n, -1, e, name)?;
                            let x = self.bin(BinOp::And, Flags::NONE, m, a);
                            let not = self.bin(BinOp::Xor, Flags::NONE, m, ones);
                            let y = self.bin(BinOp::And, Flags::NONE, not, r);
                            Some(self.bin(BinOp::Or, Flags::NONE, x, y))
                        }
                    }
                }
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

/// The resolved types of a [`Gen`] call: the type code's vector, operand
/// 0's type, the declared result type, the first overload type.
#[derive(Copy, Clone)]
struct GenCx {
    vty: Option<(E, u32)>,
    a0: LTy,
    rty: LTy,
    o0: Option<LTy>,
}

impl Cg<'_> {
    /// The trailing type code `a`'s vector and whether it is unsigned.
    fn neon_vty(&mut self, a: ExprId, name: &str) -> R<(E, u32, bool)> {
        let code = self.neon_imm(a, name)?;
        match type_code(code) {
            Some(T::V(el, n)) => Ok((el, n, code & 0x10 != 0)),
            _ => err(self.loc_of(a), format!("invalid type code for {name}")),
        }
    }

    /// The element of scalar type `t`.
    fn neon_elt(t: LTy) -> Option<E> {
        Some(match t {
            LTy::I8 => E::I8,
            LTy::I16 => E::I16,
            LTy::I32 => E::I32,
            LTy::I64 => E::I64,
            LTy::HALF => E::F16,
            LTy::BFLOAT => E::Bf16,
            LTy::FLOAT => E::F32,
            LTy::DOUBLE => E::F64,
            _ => return None,
        })
    }

    /// The LLVM type of [`G`] `g` (`None`: no such type for this type
    /// code).
    fn gen_ty(&mut self, g: G, cx: GenCx) -> Option<LTy> {
        let vec_of = |cg: &mut Self, t: LTy, bits: u32| {
            let e = Self::neon_elt(t)?;
            let n = if bits == 0 { 1 } else { bits / e.bits() };
            Some(cg.neon_ty(T::V(e, n)))
        };
        let t = match g {
            G::A0 => return Some(cx.a0),
            G::R => return Some(cx.rty),
            G::A0v(b) => return vec_of(self, cx.a0, b),
            G::Rv(b) => return vec_of(self, cx.rty, b),
            G::O0W | G::O0N => {
                let (et, n, _) = self.m.types.vector_info(cx.o0?)?;
                let e = Self::neon_elt(et)?;
                T::V(if g == G::O0W { e.widened()? } else { narrowed(e)? }, n)
            }
            G::Fix(t) => t,
            G::V | G::VF | G::VN | G::VW | G::V2 | G::VD | G::VQ | G::VH | G::VB => {
                let (e, n) = cx.vty?;
                let bits = e.bits() * n;
                match g {
                    G::VF => T::V(float_of(e)?, n),
                    G::VN => T::V(narrowed(e)?, n),
                    G::VW => T::V(e.widened()?, n),
                    G::V2 => T::V(e, 2 * n),
                    G::VD => T::V(e, 64 / e.bits()),
                    G::VQ => T::V(e, 128 / e.bits()),
                    G::VH => T::V(E::F16, bits / 16),
                    G::VB => T::V(E::I8, bits / 8),
                    _ => T::V(e, n),
                }
            }
        };
        Some(self.neon_ty(t))
    }

    /// Operand `v` converted to parameter type `to` as clang's
    /// `EmitNeonCall` and `EmitCommonNeonSISDBuiltinExpr` do: a bitcast
    /// between equal widths; a scalar truncated (or bitcast) to the
    /// element and inserted at lane 0 of a `poison` vector; an integer
    /// extended (`unsigned`: `zext`) or truncated.
    fn neon_adapt(&mut self, v: V, to: LTy, unsigned: bool, at: ExprId) -> R<V> {
        let from = self.f.fir.ty(&self.m, v);
        if from == to {
            return Ok(v);
        }
        let (fb, tb) = (self.neon_bits(from), self.neon_bits(to));
        if fb.is_some() && fb == tb {
            return Ok(self.cast(CastOp::BitCast, v, to));
        }
        let ints = self.m.types.is_int(from) && self.m.types.is_int(to);
        match (self.m.types.vector_info(to), fb, tb) {
            (Some((et, _, false)), Some(fb), _) if self.m.types.vector_info(from).is_none() => {
                let x = if from == et {
                    v
                } else if self.neon_bits(et) == Some(fb) {
                    self.cast(CastOp::BitCast, v, et)
                } else if self.m.types.is_int(from) && self.m.types.is_int(et) {
                    self.cast(CastOp::Trunc, v, et)
                } else {
                    return err(self.loc_of(at), "NEON builtin operand has an unexpected type");
                };
                let p = V::C(self.m.const_poison(to));
                let zero = V::C(self.m.const_int(LTy::I64, 0));
                Ok(self.insert_elt(p, x, zero))
            }
            (None, Some(fb), Some(tb)) if ints && fb < tb => {
                Ok(self.cast(if unsigned { CastOp::ZExt } else { CastOp::SExt }, v, to))
            }
            (None, Some(fb), Some(tb)) if ints && fb > tb => Ok(self.cast(CastOp::Trunc, v, to)),
            _ => err(self.loc_of(at), "NEON builtin operand has an unexpected type"),
        }
    }

    /// Call result `r` as the declared type `rty`: clang's SISD lane 0 of
    /// a wider vector, a bitcast between equal widths, or the truncation
    /// of the `i32` result of a 16-bit conversion.
    fn neon_result(&mut self, r: V, rty: LTy, e: ExprId) -> R<V> {
        let from = self.f.fir.ty(&self.m, r);
        if from == rty {
            return Ok(r);
        }
        let (fb, tb) = (self.neon_bits(from), self.neon_bits(rty));
        match (fb, tb) {
            (Some(a), Some(b)) if a == b => Ok(self.cast(CastOp::BitCast, r, rty)),
            (Some(a), Some(b)) if a > b => {
                if let Some((et, _, false)) = self.m.types.vector_info(from) {
                    let zero = V::C(self.m.const_int(LTy::I64, 0));
                    let x = self.extract_elt(r, zero);
                    return self.neon_bitcast(x, et, rty, e);
                }
                if self.m.types.is_int(from) && self.m.types.is_int(rty) {
                    return Ok(self.cast(CastOp::Trunc, r, rty));
                }
                err(self.loc_of(e), "NEON builtin result has an unexpected type")
            }
            _ => err(self.loc_of(e), "NEON builtin result has an unexpected type"),
        }
    }

    /// The call `e` of [`Gen`] builtin `name`.
    fn emit_neon_gen(&mut self, g: Gen, name: &str, args: &[ExprId], e: ExprId) -> R<V> {
        let ast = self.ast;
        let np = g.params.len();
        let bad = |cg: &Self| err(cg.loc_of(e), format!("invalid type code for {name}"));
        let rty = self.lty(ast.exprs.get(e).ty());
        let a0 = match args.first() {
            Some(&a) => self.lty(ast.exprs.get(a).ty()),
            None => rty,
        };
        // clang emits the operands left to right, the immediate folded,
        // then converts each to its parameter.
        let mut vals = Vec::with_capacity(np);
        let mut imm = None;
        for (i, &a) in args[..np].iter().enumerate() {
            if i + 1 == np && g.imm != Imm::No {
                imm = Some(self.neon_imm(a, name)?);
                vals.push(None);
            } else {
                vals.push(Some(self.emit_scalar(a)?));
            }
        }
        let (vty, unsigned) = if g.coded() {
            let (el, n, u) = self.neon_vty(args[np], name)?;
            (Some((el, n)), u)
        } else {
            (None, unsigned_name(name.strip_prefix(neon::PREFIX).unwrap_or(name)))
        };
        let mut cx = GenCx { vty, a0, rty, o0: None };
        let mut ovl = Vec::with_capacity(g.ovl.len());
        for &o in g.ovl {
            let Some(t) = self.gen_ty(o, cx) else { return bad(self) };
            cx.o0 = cx.o0.or(Some(t));
            ovl.push(t);
        }
        let Some(ret) = self.gen_ty(g.ret, cx) else { return bad(self) };
        let mut params = Vec::with_capacity(np);
        for &p in g.params {
            let Some(t) = self.gen_ty(p, cx) else { return bad(self) };
            params.push(t);
        }
        let mut ops = Vec::with_capacity(np);
        for (i, v) in vals.into_iter().enumerate() {
            ops.push(match (v, imm) {
                (Some(v), _) => v,
                (None, Some(k)) => match g.imm {
                    Imm::Neg => V::C(self.m.const_int(params[i], -k)),
                    Imm::Splat => {
                        let Some((et, n, _)) = self.m.types.vector_info(params[i]) else {
                            return bad(self);
                        };
                        self.neon_splat(et, n, k, e, name)?
                    }
                    _ => {
                        let t = self.lty(ast.exprs.get(args[i]).ty());
                        V::C(self.m.const_int(t, k))
                    }
                },
                (None, None) => return bad(self),
            });
        }
        let mut order: Vec<usize> = (0..np).collect();
        if g.swap && np >= 2 {
            ops.swap(0, 1);
            order.swap(0, 1);
        }
        for (i, op) in ops.iter_mut().enumerate() {
            *op = self.neon_adapt(*op, params[i], unsigned, args[order[i]])?;
        }
        let base = if unsigned { g.u } else { g.s };
        let base = if base.starts_with("llvm.") {
            base.to_owned()
        } else {
            format!("llvm.aarch64.neon.{base}")
        };
        let f = if base.starts_with("llvm.aarch64.") {
            let full = if ovl.is_empty() {
                base
            } else {
                intrinsics::overloaded_name(&self.m, &base, &ovl)
            };
            let ps: Vec<(LTy, bool)> = params
                .iter()
                .enumerate()
                .map(|(i, &t)| (t, g.imm == Imm::Arg && i + 1 == np))
                .collect();
            // LLVM declares these without `DefaultAttrsIntrinsic`.
            let plain = full
                .strip_prefix("llvm.aarch64.crypto.")
                .is_some_and(|c| c.starts_with("sm3") || c.starts_with("sm4"));
            let mem = if plain {
                intrinsics::TargetMem::UnknownNone
            } else {
                intrinsics::TargetMem::None
            };
            intrinsics::target_intrinsic(&mut self.m, &full, ret, &ps, mem)
        } else {
            intrinsics::fp_op(&mut self.m, &base, ret, np)
        };
        let r = self.intrinsic_call(f, ops);
        if g.declared { self.neon_result(r, rty, e) } else { Ok(r) }
    }

    /// The call `e` of comparison builtin `name` (clang 19
    /// `EmitAArch64CompareBuiltinExpr` against zero, which compares as
    /// the type the operand had before a bitcast, and the scalar
    /// two-operand cases).
    fn emit_neon_cmp(
        &mut self,
        c: Cmp,
        form: CmpForm,
        name: &str,
        args: &[ExprId],
        e: ExprId,
    ) -> R<V> {
        let rty = self.lty(self.ast.exprs.get(e).ty());
        let unsigned = unsigned_name(name.strip_prefix(neon::PREFIX).unwrap_or(name));
        let l = self.emit_scalar(args[0])?;
        let (l, r, to) = if form == CmpForm::Two {
            let r = self.emit_scalar(args[1])?;
            (l, r, rty)
        } else {
            let to = if form == CmpForm::Vec {
                let (el, n, _) = self.neon_vty(args[1], name)?;
                if !cmp_quad_ok(name, el.bits() * n == 128) {
                    return err(self.loc_of(args[1]), format!("invalid type code for {name}"));
                }
                self.neon_ty(T::V(el, n))
            } else {
                rty
            };
            let oty = match l {
                V::I(i) => match self.f.fir.op(i) {
                    Op::Cast { op: CastOp::BitCast, v, .. } => {
                        let v = *v;
                        self.f.fir.ty(&self.m, v)
                    }
                    _ => self.f.fir.ty(&self.m, l),
                },
                _ => self.f.fir.ty(&self.m, l),
            };
            let l = self.cast(CastOp::BitCast, l, oty);
            (l, V::C(self.m.const_zero(oty)), to)
        };
        let ty = self.f.fir.ty(&self.m, l);
        let x = if self.m.types.is_float_or_float_vector(ty) {
            let p = match c {
                Cmp::Eq => FloatPredicate::Oeq,
                Cmp::Ge => FloatPredicate::Oge,
                Cmp::Gt => FloatPredicate::Ogt,
                Cmp::Le => FloatPredicate::Ole,
                Cmp::Lt => FloatPredicate::Olt,
            };
            self.fcmp(p, l, r)
        } else {
            let u = unsigned && form == CmpForm::Two;
            let p = match c {
                Cmp::Eq => IntPredicate::Eq,
                Cmp::Ge if u => IntPredicate::Uge,
                Cmp::Gt if u => IntPredicate::Ugt,
                Cmp::Le if u => IntPredicate::Ule,
                Cmp::Lt if u => IntPredicate::Ult,
                Cmp::Ge => IntPredicate::Sge,
                Cmp::Gt => IntPredicate::Sgt,
                Cmp::Le => IntPredicate::Sle,
                Cmp::Lt => IntPredicate::Slt,
            };
            self.icmp(p, l, r)
        };
        Ok(self.cast(CastOp::SExt, x, to))
    }

    /// A target intrinsic call `name(vals)` of `memory(none)`, overloaded
    /// on `ovl`.
    fn neon_call(&mut self, name: &str, ovl: &[LTy], ret: LTy, vals: Vec<V>) -> V {
        let ps: Vec<LTy> = vals.iter().map(|&v| self.f.fir.ty(&self.m, v)).collect();
        let full = intrinsics::overloaded_name(&self.m, name, ovl);
        let f = intrinsics::target_nomem(&mut self.m, &full, ret, &ps);
        self.intrinsic_call(f, vals)
    }

    /// The call `e` of [`Sp`] builtin `name`.
    #[allow(clippy::too_many_lines, reason = "one arm per builtin family")]
    fn emit_neon_sp(&mut self, sp: Sp, name: &str, args: &[ExprId], e: ExprId) -> R<V> {
        let rty = self.lty(self.ast.exprs.get(e).ty());
        let unsigned = unsigned_name(name.strip_prefix(neon::PREFIX).unwrap_or(name));
        let bad = |cg: &Self| err(cg.loc_of(e), format!("invalid type code for {name}"));
        let i32c = |cg: &mut Self, x: i128| V::C(cg.m.const_int(LTy::I32, x));
        let i64c = |cg: &mut Self, x: i128| V::C(cg.m.const_int(LTy::I64, x));
        Ok(match sp {
            Sp::AddV | Sp::AddP128 => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let (bt, to) = if sp == Sp::AddV {
                    let (el, n, _) = self.neon_vty(args[2], name)?;
                    let bt = self.neon_ty(T::V(E::I8, el.bits() * n / 8));
                    (bt, self.neon_ty(T::V(el, n)))
                } else {
                    (self.neon_ty(T::V(E::I8, 16)), LTy::I128)
                };
                let a = self.neon_as(a, bt, args[0])?;
                let b = self.neon_as(b, bt, args[1])?;
                let x = self.bin(BinOp::Xor, Flags::NONE, a, b);
                self.neon_bitcast(x, bt, to, e)?
            }
            Sp::Hn(sub) => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let (el, n, _) = self.neon_vty(args[2], name)?;
                let Some(w) = el.widened() else { return bad(self) };
                let wt = self.neon_ty(T::V(w, n));
                let a = self.neon_as(a, wt, args[0])?;
                let b = self.neon_as(b, wt, args[1])?;
                let x = self.bin(if sub { BinOp::Sub } else { BinOp::Add }, Flags::NONE, a, b);
                let wet = self.neon_ty(T::S(w));
                let s = self.neon_splat(wet, n, i128::from(el.bits()), e, name)?;
                let x = self.bin(BinOp::LShr, Flags::NONE, x, s);
                let vt = self.neon_ty(T::V(el, n));
                self.cast(CastOp::Trunc, x, vt)
            }
            Sp::Rsra => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let k = self.neon_imm(args[2], name)?;
                let (el, n, u) = self.neon_vty(args[3], name)?;
                if !is_int(el) || !(1..=i128::from(el.bits())).contains(&k) {
                    return err(self.loc_of(args[2]), format!("invalid shift for {name}"));
                }
                let vt = self.neon_ty(T::V(el, n));
                let et = self.neon_ty(T::S(el));
                let b = self.neon_as(b, vt, args[1])?;
                let s = self.neon_splat(et, n, -k, e, name)?;
                let base = if u { "llvm.aarch64.neon.urshl" } else { "llvm.aarch64.neon.srshl" };
                let t = self.neon_call(base, &[vt], vt, vec![b, s]);
                let a = self.neon_as(a, vt, args[0])?;
                self.bin(BinOp::Add, Flags::NONE, a, t)
            }
            Sp::Qdmlal(sub) => {
                let acc = self.emit_scalar(args[0])?;
                let a = self.emit_scalar(args[1])?;
                let b = self.emit_scalar(args[2])?;
                let (el, n, _) = self.neon_vty(args[3], name)?;
                let Some(h) = narrowed(el).filter(|_| matches!(el, E::I32 | E::I64)) else {
                    return bad(self);
                };
                let vt = self.neon_ty(T::V(el, n));
                let ht = self.neon_ty(T::V(h, n));
                let a = self.neon_as(a, ht, args[1])?;
                let b = self.neon_as(b, ht, args[2])?;
                let m = self.neon_call("llvm.aarch64.neon.sqdmull", &[vt], vt, vec![a, b]);
                let acc = self.neon_as(acc, vt, args[0])?;
                let base = if sub { "llvm.aarch64.neon.sqsub" } else { "llvm.aarch64.neon.sqadd" };
                self.neon_call(base, &[vt], vt, vec![acc, m])
            }
            Sp::QdmlalS { sub, lane } => {
                let acc = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let mut c = None;
                if lane {
                    let v = self.emit_scalar(args[2])?;
                    let l = self.neon_imm(args[3], name)?;
                    let l = i32c(self, l);
                    c = Some(self.extract_elt(v, l));
                }
                let accty = self.f.fir.ty(&self.m, acc);
                let m = if accty == LTy::I32 {
                    // `vectorWrapScalar16` each factor, `sqdmull.v4i32`, lane 0.
                    let vt = self.neon_ty(T::V(E::I16, 4));
                    let p = V::C(self.m.const_poison(vt));
                    let z = i64c(self, 0);
                    let bw = self.insert_elt(p, b, z);
                    let c = match c {
                        Some(c) => c,
                        None => self.emit_scalar(args[2])?,
                    };
                    let cw = self.insert_elt(p, c, z);
                    let wt = self.neon_ty(T::V(E::I32, 4));
                    let m = self.neon_call("llvm.aarch64.neon.sqdmull", &[wt], wt, vec![bw, cw]);
                    self.extract_elt(m, z)
                } else {
                    let c = match c {
                        Some(c) => c,
                        None => self.emit_scalar(args[2])?,
                    };
                    self.neon_call("llvm.aarch64.neon.sqdmulls.scalar", &[], LTy::I64, vec![b, c])
                };
                let base = if sub { "llvm.aarch64.neon.sqsub" } else { "llvm.aarch64.neon.sqadd" };
                self.neon_call(base, &[accty], accty, vec![acc, m])
            }
            Sp::MulLane(q) => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let l = self.neon_imm(args[2], name)?;
                let (el, n, _) = self.neon_vty(args[3], name)?;
                let vt = self.neon_ty(T::V(el, n));
                let a = self.neon_as(a, LTy::DOUBLE, args[0])?;
                let bt = self.neon_ty(T::V(E::F64, if q { 2 } else { 1 }));
                let b = self.neon_as(b, bt, args[1])?;
                let l = i32c(self, l);
                let x = self.extract_elt(b, l);
                let r = self.bin(BinOp::FMul, Flags::NONE, a, x);
                self.neon_bitcast(r, LTy::DOUBLE, vt, e)?
            }
            Sp::MulN => {
                let a = self.emit_scalar(args[0])?;
                let a = self.neon_as(a, LTy::DOUBLE, args[0])?;
                let x = self.emit_scalar(args[1])?;
                let x = self.neon_as(x, LTy::DOUBLE, args[1])?;
                self.bin(BinOp::FMul, Flags::NONE, a, x)
            }
            Sp::PAddF => {
                let v = self.emit_scalar(args[0])?;
                let z = i64c(self, 0);
                let o = i64c(self, 1);
                let x = self.extract_elt(v, z);
                let y = self.extract_elt(v, o);
                self.bin(BinOp::FAdd, Flags::NONE, x, y)
            }
            Sp::FmaLaneS => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let v = self.emit_scalar(args[2])?;
                let l = self.neon_imm(args[3], name)?;
                let l = i32c(self, l);
                let x = self.extract_elt(v, l);
                let f = intrinsics::fp_op(&mut self.m, "llvm.fma", rty, 3);
                self.intrinsic_call(f, vec![b, x, a])
            }
            Sp::MulxLane => {
                let a = self.emit_scalar(args[0])?;
                let v = self.emit_scalar(args[1])?;
                let l = self.neon_imm(args[2], name)?;
                let l = i32c(self, l);
                let x = self.extract_elt(v, l);
                self.neon_call("llvm.aarch64.neon.fmulx", &[LTy::HALF], LTy::HALF, vec![a, x])
            }
            Sp::Fmah(neg) => {
                // clang emits operand 1 twice (its loop, then the case).
                let a = self.emit_scalar(args[0])?;
                self.emit_scalar(args[1])?;
                let b = self.emit_scalar(args[1])?;
                let b = if neg { self.fneg(b) } else { b };
                let c = self.emit_scalar(args[2])?;
                let f = intrinsics::fp_op(&mut self.m, "llvm.fma", LTy::HALF, 3);
                self.intrinsic_call(f, vec![b, c, a])
            }
            Sp::Bin(op) => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                self.bin(op, Flags::NONE, a, b)
            }
            Sp::Neg => {
                let x = self.emit_scalar(args[0])?;
                let t = self.f.fir.ty(&self.m, x);
                if self.m.types.is_float(t) {
                    self.fneg(x)
                } else {
                    let z = V::C(self.m.const_int(t, 0));
                    self.bin(BinOp::Sub, Flags::NONE, z, x)
                }
            }
            Sp::ShlD | Sp::ShrD | Sp::SraD => {
                let (a, b, k) = if sp == Sp::SraD {
                    let a = self.emit_scalar(args[0])?;
                    let b = self.emit_scalar(args[1])?;
                    (Some(a), b, self.neon_imm(args[2], name)?)
                } else {
                    let x = self.emit_scalar(args[0])?;
                    (None, x, self.neon_imm(args[1], name)?)
                };
                let k = k & 0xffff_ffff;
                let s = if sp == Sp::ShlD {
                    let k = i64c(self, k);
                    self.bin(BinOp::Shl, Flags::NONE, b, k)
                } else if !unsigned {
                    let k = i64c(self, k.min(63));
                    self.bin(BinOp::AShr, Flags::NONE, b, k)
                } else if k == 64 {
                    match a {
                        Some(a) => return Ok(a),
                        None => V::C(self.m.const_int(LTy::I64, 0)),
                    }
                } else {
                    let k = i64c(self, k);
                    self.bin(BinOp::LShr, Flags::NONE, b, k)
                };
                match a {
                    Some(a) => self.bin(BinOp::Add, Flags::NONE, a, s),
                    None => s,
                }
            }
            Sp::RsraD => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let k = self.neon_imm(args[2], name)?;
                let k = i64c(self, -k);
                let base =
                    if unsigned { "llvm.aarch64.neon.urshl" } else { "llvm.aarch64.neon.srshl" };
                let r = self.neon_call(base, &[LTy::I64], LTy::I64, vec![b, k]);
                self.bin(BinOp::Add, Flags::NONE, a, r)
            }
            Sp::TstD => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let x = self.bin(BinOp::And, Flags::NONE, a, b);
                let z = i64c(self, 0);
                let c = self.icmp(IntPredicate::Ne, x, z);
                self.cast(CastOp::SExt, c, LTy::I64)
            }
            Sp::CvtS => {
                let x = self.emit_scalar(args[0])?;
                self.cast(if unsigned { CastOp::UIToFP } else { CastOp::SIToFP }, x, rty)
            }
        })
    }
}

/// Whether the type code of compare-against-zero builtin `name` (a
/// [`CmpForm::Vec`]) may name a 128-bit vector (`quad`): as clang's
/// `NeonTypeFlags`, the quad bit must agree with the `q` of the name
/// before its type suffix (`vceqzq_v`, `vcltzq_f16`). Sema does not check
/// it; clang rejects a mismatch as "incompatible constant for this
/// `__builtin_neon` function".
fn cmp_quad_ok(name: &str, quad: bool) -> bool {
    let base = name.strip_prefix(neon::PREFIX).unwrap_or(name);
    let stem = base.rsplit_once('_').map_or(base, |(s, _)| s);
    stem.ends_with('q') == quad
}

/// The element of half `e`'s width (clang
/// `getTruncatedElementVectorType`; `float` to `half`).
const fn narrowed(e: E) -> Option<E> {
    match e {
        E::I16 => Some(E::I8),
        E::I32 => Some(E::I16),
        E::I64 => Some(E::I32),
        E::F32 => Some(E::F16),
        E::F64 => Some(E::F32),
        _ => None,
    }
}

/// The float element of `e`'s width (clang `GetFloatNeonType`).
const fn float_of(e: E) -> Option<E> {
    match e {
        E::I16 | E::F16 => Some(E::F16),
        E::I32 | E::F32 => Some(E::F32),
        E::I64 | E::F64 => Some(E::F64),
        _ => None,
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
    use super::{
        Cmp, CmpForm, E, Fp, G, I32, Imm, Lk, Perm, Red, Shape, Sp, T, cmp_quad_ok,
        scalar_cvt_parts, shape, type_code, unsigned_name,
    };

    fn s(n: &str) -> Option<Shape> {
        shape(&format!("__builtin_neon_{n}"))
    }

    /// Every type code the shipped `arm_neon.h` and `arm_fp16.h` pass to
    /// a compare-against-zero builtin names a vector and passes the quad
    /// check of [`cmp_quad_ok`] (the `q_f16` forms pass 49).
    #[test]
    fn header_compare_codes_accepted() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../resource/include/");
        let mut seen = 0;
        for h in ["arm_neon.h", "arm_fp16.h"] {
            let text = std::fs::read_to_string(format!("{dir}{h}")).unwrap();
            for (at, _) in text.match_indices("__builtin_neon_v") {
                let rest = &text[at..];
                let end = rest
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(rest.len());
                let name = &rest[..end];
                if !matches!(shape(name), Some(Shape::Cmp(_, CmpForm::Vec))) {
                    continue;
                }
                let call = &rest[end..rest.find(';').unwrap()];
                let code = call.trim_end_matches(')').rsplit(',').next().unwrap().trim();
                let code: i128 = code.parse().unwrap_or_else(|_| panic!("{name}: {call}"));
                let Some(T::V(e, n)) = type_code(code) else {
                    panic!("{name}: type code {code} is not a vector")
                };
                assert!(cmp_quad_ok(name, e.bits() * n == 128), "{name}: type code {code}");
                seen += 1;
            }
        }
        assert!(seen >= 60, "only {seen} compare-against-zero calls found");
        assert!(!cmp_quad_ok("__builtin_neon_vceqzq_v", false));
        assert!(!cmp_quad_ok("__builtin_neon_vceqz_v", true));
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
        assert_eq!(s("vtbl4_v"), Some(Shape::Tbl(4)));
        assert_eq!(s("vtbx3_v"), Some(Shape::Tbx(3)));
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
        assert_eq!(s("vaddvq_p8"), None);
        assert_eq!(s("vmovn_bf16"), None);
    }

    /// Every NEON builtin classifies without a panic, and a shape's
    /// immediates are operands.
    #[test]
    fn every_row_classifies() {
        for r in kiln_ast::decl::builtin::neon::rows() {
            if let Some(s) = shape(r.info.name) {
                assert!(s.imm_args().iter().all(|&i| i < s.arity()), "{}", r.info.name);
            }
        }
    }

    /// The WP-1421 shapes.
    #[test]
    fn more_shapes() {
        assert_eq!(s("vadd_v"), Some(Shape::Sp(Sp::AddV)));
        assert_eq!(
            s("vqdmlalh_laneq_s16"),
            Some(Shape::Sp(Sp::QdmlalS { sub: false, lane: true }))
        );
        assert_eq!(s("vcvts_f32_u32"), Some(Shape::Sp(Sp::CvtS)));
        assert_eq!(s("vcvth_f16_s16"), Some(Shape::Sp(Sp::CvtS)));
        assert_eq!(s("vceqz_v"), Some(Shape::Cmp(Cmp::Eq, CmpForm::Vec)));
        assert_eq!(s("vcltzq_f16"), Some(Shape::Cmp(Cmp::Lt, CmpForm::Vec)));
        assert_eq!(s("vcgezd_s64"), Some(Shape::Cmp(Cmp::Ge, CmpForm::Zero)));
        assert_eq!(s("vcled_u64"), Some(Shape::Cmp(Cmp::Le, CmpForm::Two)));
        assert_eq!(s("vld1_x2_v"), Some(Shape::LdX(Lk::X1, 2)));
        assert_eq!(s("vld1q_bf16_x3"), Some(Shape::LdX(Lk::X1, 3)));
        assert_eq!(s("vld3_dup_v"), Some(Shape::LdX(Lk::Dup, 3)));
        assert_eq!(s("vld4q_lane_bf16"), Some(Shape::LdX(Lk::Lane, 4)));
        assert_eq!(s("vst1q_x4_v"), Some(Shape::StX(false, 4)));
        assert_eq!(s("vst2_lane_v"), Some(Shape::StX(true, 2)));
        assert_eq!(s("vldap1q_lane_p64"), Some(Shape::LdAcq));
        assert_eq!(s("vstl1_lane_s64"), Some(Shape::StRel));
        assert_eq!(s("vcvtm_u16_f16"), Some(Shape::CvtR(b'm')));
        let g = |n: &str| match s(n) {
            Some(Shape::Gen(g)) => g,
            x => panic!("{n}: {x:?}"),
        };
        assert_eq!(g("vrndxq_v").s, "llvm.rint");
        assert_eq!(g("vqaddb_s8").u, "uqadd");
        assert_eq!(g("vqaddb_s8").params, [G::A0v(64), G::A0v(64)]);
        assert_eq!(g("vqrshrnh_n_u16").imm, Imm::Plain);
        assert_eq!(g("vrshrd_n_s64").imm, Imm::Neg);
        assert!(g("vcales_f32").swap);
        assert!(g("vcageq_v").coded());
        assert!(!g("vmaxnmv_f16").coded());
        assert_eq!(g("vxarq_u64").imm_args(), [2, 3]);
        assert_eq!(g("vqdmulhq_lane_v").imm_args(), [2, 3]);
        assert_eq!(g("vcvth_n_s16_f16").ret, I32);
        assert_eq!(g("vcvtas_u32_f32").u, "fcvtau");
        assert!(unsigned_name("vcvtas_u32_f32"));
        assert!(!unsigned_name("vqmovuns_s32"));
        assert_eq!(scalar_cvt_parts("vcvtnh_s64_f16"), Some(("s64", "f16", b'n', false)));
        assert_eq!(scalar_cvt_parts("vcvth_n_f16_u32"), Some(("f16", "u32", b'z', true)));
        assert_eq!(scalar_cvt_parts("vcvt_f32_v"), None);
    }
}
