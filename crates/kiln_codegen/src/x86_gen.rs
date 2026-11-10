//! x86 builtins clang lowers to generic IR or to a changed operand list
//! (spec/landed/codegen/WP-630.md, WP-1242): the AVX/AVX2 subvector
//! extracts, inserts and lane permutes, F16C conversions, FMA, and the
//! AVX-512 mask, select, compare, masked memory and shuffle families.
//! Every lowering follows clang's `CodeGenFunction::EmitX86BuiltinExpr`
//! (clang 19 and 22 agree on them): the operands are emitted first, in
//! order, an integer constant one as a constant of its parameter type;
//! then the family's IR is built from them.

use kiln_ast::ExprId;
use kiln_ast::decl::builtin as id;
use kiln_llvm::{
    Attr, BinOp, CastOp, FastMath, Flags, IntPredicate, TypeId as LTy, intrinsics,
};

use crate::fir::{Fir, Op, V};
use crate::module::{Cg, R, err};
use crate::x86::Mem;

/// A family of builtins sharing one lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fam {
    /// `extract128i256`, `vextractf128_*`, `extract{f,i}*_mask`.
    Extract,
    /// `insert128i256`, `vinsertf128_*`, `insert{f,i}*`.
    Insert,
    /// `permti256`, `vperm2f128_*`.
    Perm2x128,
    /// `vcvtph2ps*`.
    CvtPh2Ps,
    /// The packed FMA builtins (`vfmadd{ph,ps,pd}*`, `vfmsub*_mask3`):
    /// whether add-sub.
    Fma(bool),
    /// The scalar FMA builtins.
    ScalarFma,
    /// `k{and,andn,or,xnor,xor,not,mov,unpck,ortest*,shift*}`.
    MaskOp,
    /// `{,u}cmp{b,w,d,q}*_mask`: whether signed.
    IntCmp(bool),
    /// `cmp{ph,ps,pd}*_mask`.
    FpCmp,
    /// `vpcom*`: whether signed.
    Vpcom(bool),
    /// The rotates and concatenating shifts (`fshl` / `fshr`).
    Funnel,
    /// `pternlog{d,q}*_mask{,z}`: whether zero-masking.
    Ternlog(bool),
    /// The AVX-512 integer to floating-point conversions: whether signed.
    IntToFp(bool),
    /// `select{b,w,d,q,ph,pbf,ps,pd}_*`.
    Select,
    /// `select{sh,ss,sd}_128`.
    ScalarSelect,
    /// `vplzcnt{d,q}_*`.
    Lzcnt,
    /// `sqrtph`, `sqrtph256`, `sqrt{ph,ps,pd}512`.
    Sqrt,
    /// `sqrt{sh,ss,sd}_round_mask`.
    ScalarSqrt,
    /// `reduce_f{add,mul,max,min}_*`.
    Reduce,
    /// `fpclass{ph,ps,pd}*_mask`.
    FpClass,
    /// `vp2intersect_{d,q}_*`.
    P2Intersect,
    /// `vpshufbitqmb*_mask`.
    ShufBitQmb,
    /// `cvtneps2bf16_*_mask`, `cvtsbf162ss_32`.
    Bf16,
    /// `cvtmask2{b,w,d,q}*`.
    MaskToVec,
    /// `cvt{b,w,d,q}2mask*`.
    VecToMask,
    /// `pmovqd512_mask`, `pmovwb512_mask`.
    Trunc,
    /// The masked loads (`load*_mask`, `loaddqu*`, `movdqa*load*`):
    /// whether aligned.
    MaskedLoad(bool),
    /// The masked stores: whether aligned.
    MaskedStore(bool),
    /// `expandload*_mask`.
    ExpandLoad,
    /// `compressstore*_mask`.
    CompressStore,
    /// `expand*_mask` / `compress*_mask` (register forms): whether compress.
    CompressExpand(bool),
    /// `gather3{div,siv}*`.
    Gather,
    /// `scatter{div,siv}*` (the AVX-512VL ones).
    Scatter,
    /// `xsave*`, `xrstor*`, `xsetbv`.
    Xsave,
    /// `prefetchi`.
    Prefetchi,
    /// `vf{c}maddcph512_mask`, `vf{c}maddcsh_round_mask{,3}`.
    ComplexFma,
    /// `vpermil{pd,ps}{,256,512}`.
    Permil,
    /// `permd{f,i}{256,512}`.
    Perm4x64,
    /// `align{d,q}*`.
    Valign,
    /// `shuf_{f,i}{32x4,64x2}{,_256}`.
    ShufLanes,
}

/// The size suffixes of a family name (`""`, `"128"`, ...).
fn sized<'a>(n: &'a str, stem: &str, sizes: &[&str]) -> Option<&'a str> {
    let rest = n.strip_prefix(stem)?;
    sizes.iter().any(|s| *s == rest).then_some(rest)
}

/// The family of x86 builtin `n` (without `__builtin_ia32_`).
#[allow(clippy::too_many_lines, reason = "one arm per family, as clang's switch")]
fn family(n: &str) -> Option<Fam> {
    use Fam::*;
    const V3: &[&str] = &["128", "256", "512"];
    const U3: &[&str] = &["_128", "_256", "_512"];
    const K4: &[&str] = &["qi", "hi", "si", "di"];
    let ends = |s: &str| n.ends_with(s);
    Some(match n {
        "extract128i256" | "vextractf128_pd256" | "vextractf128_ps256" | "vextractf128_si256" => {
            Extract
        }
        "insert128i256" | "vinsertf128_pd256" | "vinsertf128_ps256" | "vinsertf128_si256" => Insert,
        "permti256" | "vperm2f128_pd256" | "vperm2f128_ps256" | "vperm2f128_si256" => Perm2x128,
        "vcvtph2ps" | "vcvtph2ps256" | "vcvtph2ps_mask" | "vcvtph2ps256_mask"
        | "vcvtph2ps512_mask" => CvtPh2Ps,
        "vfmaddps" | "vfmaddpd" | "vfmaddph" | "vfmaddps256" | "vfmaddpd256" | "vfmaddph256" => {
            Fma(false)
        }
        "vfmaddss3" | "vfmaddsd3" | "vfmaddss" | "vfmaddsd" => ScalarFma,
        "kmovb" | "kmovw" | "kmovd" | "kmovq" => MaskOp,
        "vpcomb" | "vpcomw" | "vpcomd" | "vpcomq" => Vpcom(true),
        "vpcomub" | "vpcomuw" | "vpcomud" | "vpcomuq" => Vpcom(false),
        "vprotb" | "vprotw" | "vprotd" | "vprotq" | "vprotbi" | "vprotwi" | "vprotdi"
        | "vprotqi" => Funnel,
        "selectsh_128" | "selectss_128" | "selectsd_128" => ScalarSelect,
        "sqrtph" | "sqrtph256" | "sqrtph512" | "sqrtps512" | "sqrtpd512" => Sqrt,
        "sqrtsh_round_mask" | "sqrtss_round_mask" | "sqrtsd_round_mask" => ScalarSqrt,
        "cvtneps2bf16_128_mask" | "cvtneps2bf16_256_mask" | "cvtneps2bf16_512_mask"
        | "cvtsbf162ss_32" => Bf16,
        "pmovqd512_mask" | "pmovwb512_mask" => Trunc,
        "cvtdq2ps512_mask" | "cvtqq2ps512_mask" | "cvtqq2pd512_mask" | "vcvtw2ph512_mask"
        | "vcvtdq2ph512_mask" | "vcvtqq2ph512_mask" => IntToFp(true),
        "cvtudq2ps512_mask" | "cvtuqq2ps512_mask" | "cvtuqq2pd512_mask" | "vcvtuw2ph512_mask"
        | "vcvtudq2ph512_mask" | "vcvtuqq2ph512_mask" => IntToFp(false),
        "xsave" | "xsave64" | "xrstor" | "xrstor64" | "xsaveopt" | "xsaveopt64" | "xrstors"
        | "xrstors64" | "xsavec" | "xsavec64" | "xsaves" | "xsaves64" | "xsetbv" => Xsave,
        "prefetchi" => Prefetchi,
        "vfmaddcph512_mask" | "vfcmaddcph512_mask" | "vfmaddcsh_round_mask"
        | "vfcmaddcsh_round_mask" | "vfmaddcsh_round_mask3" | "vfcmaddcsh_round_mask3" => {
            ComplexFma
        }
        "vpermilpd" | "vpermilps" | "vpermilpd256" | "vpermilps256" | "vpermilpd512"
        | "vpermilps512" => Permil,
        "permdf256" | "permdi256" | "permdf512" | "permdi512" => Perm4x64,
        "shuf_f32x4" | "shuf_f64x2" | "shuf_i32x4" | "shuf_i64x2" | "shuf_f32x4_256"
        | "shuf_f64x2_256" | "shuf_i32x4_256" | "shuf_i64x2_256" => ShufLanes,
        _ if (n.starts_with("extractf") || n.starts_with("extracti")) && ends("_mask") => Extract,
        _ if n.starts_with("insertf") || n.starts_with("inserti") => Insert,
        _ if n.starts_with("vfmaddsub") && n.contains("512_mask") => Fma(true),
        _ if n.starts_with("vfmsubadd") && ends("512_mask3") => Fma(true),
        _ if n.starts_with("vfmadd") && n.contains("512_mask") => Fma(false),
        _ if n.starts_with("vfmsub") && ends("512_mask3") => Fma(false),
        _ if (n.starts_with("vfmadd") || n.starts_with("vfmsub")) && n.contains("3_mask") => {
            ScalarFma
        }
        _ if ["kand", "kandn", "kor", "kxnor", "kxor", "knot", "kunpck", "kortestc", "kortestz"]
            .iter()
            .any(|s| sized(n, s, K4).is_some())
            || sized(n, "kshiftli", K4).is_some()
            || sized(n, "kshiftri", K4).is_some() =>
        {
            MaskOp
        }
        _ if ["cmpb", "cmpw", "cmpd", "cmpq"].iter().any(|s| {
            n.strip_suffix("_mask").and_then(|m| sized(m, s, V3)).is_some()
        }) =>
        {
            IntCmp(true)
        }
        _ if ["ucmpb", "ucmpw", "ucmpd", "ucmpq"].iter().any(|s| {
            n.strip_suffix("_mask").and_then(|m| sized(m, s, V3)).is_some()
        }) =>
        {
            IntCmp(false)
        }
        _ if ["cmpph", "cmpps", "cmppd"].iter().any(|s| {
            n.strip_suffix("_mask").and_then(|m| sized(m, s, V3)).is_some()
        }) =>
        {
            FpCmp
        }
        _ if ["prold", "prolq", "prolvd", "prolvq", "prord", "prorq", "prorvd", "prorvq"]
            .iter()
            .chain(&["vpshldw", "vpshldd", "vpshldq", "vpshrdw", "vpshrdd", "vpshrdq"])
            .chain(&["vpshldvw", "vpshldvd", "vpshldvq", "vpshrdvw", "vpshrdvd", "vpshrdvq"])
            .any(|s| sized(n, s, V3).is_some()) =>
        {
            Funnel
        }
        _ if ["pternlogd", "pternlogq"].iter().any(|s| {
            n.strip_suffix("_mask").and_then(|m| sized(m, s, V3)).is_some()
        }) =>
        {
            Ternlog(false)
        }
        _ if ["pternlogd", "pternlogq"].iter().any(|s| {
            n.strip_suffix("_maskz").and_then(|m| sized(m, s, V3)).is_some()
        }) =>
        {
            Ternlog(true)
        }
        _ if ["selectb", "selectw", "selectd", "selectq", "selectph", "selectpbf", "selectps"]
            .iter()
            .chain(&["selectpd"])
            .any(|s| sized(n, s, U3).is_some()) =>
        {
            Select
        }
        _ if sized(n, "vplzcntd", U3).is_some() || sized(n, "vplzcntq", U3).is_some() => Lzcnt,
        _ if ["reduce_fadd_", "reduce_fmul_", "reduce_fmax_", "reduce_fmin_"].iter().any(|s| {
            sized(n, s, &["pd512", "ps512", "ph512", "ph256", "ph128"]).is_some()
        }) =>
        {
            Reduce
        }
        _ if ["fpclassph", "fpclassps", "fpclasspd"].iter().any(|s| {
            n.strip_suffix("_mask").and_then(|m| sized(m, s, V3)).is_some()
        }) =>
        {
            FpClass
        }
        _ if sized(n, "vp2intersect_d", U3).is_some()
            || sized(n, "vp2intersect_q", U3).is_some() =>
        {
            P2Intersect
        }
        _ if n.strip_suffix("_mask").and_then(|m| sized(m, "vpshufbitqmb", V3)).is_some() => {
            ShufBitQmb
        }
        _ if ["cvtmask2b", "cvtmask2w", "cvtmask2d", "cvtmask2q"]
            .iter()
            .any(|s| sized(n, s, V3).is_some()) =>
        {
            MaskToVec
        }
        _ if ["cvtb2mask", "cvtw2mask", "cvtd2mask", "cvtq2mask"]
            .iter()
            .any(|s| sized(n, s, V3).is_some()) =>
        {
            VecToMask
        }
        _ if ends("_mask") && masked_mem(n).is_some() => return masked_mem(n),
        _ if n.starts_with("gather3div") || n.starts_with("gather3siv") => Gather,
        _ if n.starts_with("scatterdiv") || n.starts_with("scattersiv") => Scatter,
        _ if ["alignd", "alignq"].iter().any(|s| sized(n, s, V3).is_some()) => Valign,
        _ => return None,
    })
}

/// The masked memory family of `n` (ending in `_mask`).
fn masked_mem(n: &str) -> Option<Fam> {
    let m = n.strip_suffix("_mask")?;
    let ty = |s: &str| {
        ["pd", "ps", "df", "sf", "di", "si", "hi", "qi"]
            .iter()
            .any(|t| sized(s, t, &["128", "256", "512"]).is_some())
    };
    let ty_dq = |s: &str| {
        ["qi", "hi", "si", "di"].iter().any(|t| sized(s, t, &["128", "256", "512"]).is_some())
    };
    let v = |s: &str| ["128", "256", "512"].contains(&s);
    Some(if m.strip_prefix("loadu").is_some_and(ty) {
        Fam::MaskedLoad(false)
    } else if m.strip_prefix("loada").is_some_and(ty) {
        Fam::MaskedLoad(true)
    } else if m.strip_prefix("loaddqu").is_some_and(ty_dq) {
        Fam::MaskedLoad(false)
    } else if matches!(m, "loadsh128" | "loadss128" | "loadsd128") {
        Fam::MaskedLoad(false)
    } else if m.strip_prefix("movdqa32load").or(m.strip_prefix("movdqa64load")).is_some_and(v) {
        Fam::MaskedLoad(true)
    } else if m.strip_prefix("storeu").is_some_and(ty) {
        Fam::MaskedStore(false)
    } else if m.strip_prefix("storea").is_some_and(ty) {
        Fam::MaskedStore(true)
    } else if m.strip_prefix("storedqu").is_some_and(ty_dq) {
        Fam::MaskedStore(false)
    } else if matches!(m, "storesh128" | "storess128" | "storesd128") {
        Fam::MaskedStore(false)
    } else if m.strip_prefix("movdqa32store").or(m.strip_prefix("movdqa64store")).is_some_and(v)
    {
        Fam::MaskedStore(true)
    } else if m.strip_prefix("expandload").is_some_and(ty) {
        Fam::ExpandLoad
    } else if m.strip_prefix("compressstore").is_some_and(ty) {
        Fam::CompressStore
    } else if m.strip_prefix("expand").is_some_and(ty) {
        Fam::CompressExpand(false)
    } else if m.strip_prefix("compress").is_some_and(ty) {
        Fam::CompressExpand(true)
    } else {
        return None;
    })
}

impl Cg<'_> {
    /// The call `e` of x86 builtin `n` (builtin `b`) when it is one of
    /// this module's families: `Ok(None)` when it is not.
    #[allow(clippy::too_many_lines, reason = "one arm per family, as clang's switch")]
    #[allow(
        clippy::option_option,
        reason = "outer: lowered here or not; inner: the call's value, `None` when void"
    )]
    pub(crate) fn x86_generic(
        &mut self,
        b: u32,
        n: &str,
        e: ExprId,
        args: &[ExprId],
        rty: LTy,
    ) -> R<Option<Option<V>>> {
        let Some(fam) = family(n) else { return Ok(None) };
        let mut ops = self.x86_ops(b, args)?;
        let need = match fam {
            Fam::MaskOp if n.starts_with("knot") || n.starts_with("kmov") => 1,
            Fam::CvtPh2Ps | Fam::Lzcnt | Fam::Bf16 | Fam::MaskToVec | Fam::VecToMask => 1,
            Fam::Sqrt | Fam::Reduce => 1,
            Fam::MaskOp | Fam::Permil | Fam::Perm4x64 | Fam::Extract | Fam::Prefetchi => 2,
            Fam::Funnel if n.starts_with("vprot") || n.starts_with("pro") => 2,
            Fam::Xsave | Fam::P2Intersect => 2,
            Fam::Scatter | Fam::Gather => 5,
            Fam::ComplexFma | Fam::ScalarSqrt => 5,
            Fam::Ternlog(_) => 5,
            Fam::IntToFp(_) | Fam::MaskedLoad(_) | Fam::MaskedStore(_) | Fam::ExpandLoad => 3,
            Fam::CompressStore | Fam::CompressExpand(_) | Fam::Trunc => 3,
            Fam::IntCmp(_) | Fam::FpCmp => 4,
            Fam::FpClass | Fam::ShufBitQmb | Fam::Select | Fam::ScalarSelect => 3,
            _ => 3,
        };
        if ops.len() < need {
            return err(self.loc_of(e), format!("too few arguments to __builtin_ia32_{n}"));
        }
        let k = |s: &Self, v: V| s.cint(v).map_or(0, |c| c.1 as u32);
        let ty = |s: &Self, v: V| s.f.fir.ty(&s.m, v);
        let z64 = V::C(self.m.const_int(LTy::I64, 0));
        let v = match fam {
            Fam::Extract => {
                let (_, sn) = self.x86_lanes(ty(self, ops[0]));
                let (_, dn) = self.x86_lanes(rty);
                let idx = (k(self, ops[1]) & (sn / dn - 1)) * dn;
                let r = self.shuffle(ops[0], None, (idx..idx + dn).collect());
                if ops.len() == 4 { self.x86_select(ops[3], r, ops[2]) } else { r }
            }
            Fam::Insert => {
                let (_, dn) = self.x86_lanes(ty(self, ops[0]));
                let (_, sn) = self.x86_lanes(ty(self, ops[1]));
                let idx = (k(self, ops[2]) & (dn / sn - 1)) * sn;
                let widen = (0..dn).map(|i| if i >= sn { sn + i % sn } else { i }).collect();
                let w = self.shuffle(ops[1], None, widen);
                let mask = (0..dn)
                    .map(|i| if i >= idx && i < idx + sn { i - idx + dn } else { i })
                    .collect();
                self.shuffle(ops[0], Some(w), mask)
            }
            Fam::Perm2x128 => {
                let im = k(self, ops[2]);
                let t = ty(self, ops[0]);
                let (_, n) = self.x86_lanes(t);
                let mut out = [ops[0]; 2];
                let mut mask = vec![0; n as usize];
                for l in 0..2 {
                    out[l as usize] = if im & (1 << (l * 4 + 3)) != 0 {
                        V::C(self.m.const_zero(t))
                    } else if im & (1 << (l * 4 + 1)) != 0 {
                        ops[1]
                    } else {
                        ops[0]
                    };
                    for i in 0..n / 2 {
                        let mut idx = l * n + i;
                        if im & (1 << (l * 4)) != 0 {
                            idx += n / 2;
                        }
                        mask[(l * (n / 2) + i) as usize] = idx;
                    }
                }
                self.shuffle(out[0], Some(out[1]), mask)
            }
            Fam::CvtPh2Ps => {
                if ops.len() == 4 && k(self, ops[3]) != 4 {
                    let p = self.x86_params(&ops, &[3]);
                    let f = intrinsics::target_intrinsic(
                        &mut self.m,
                        "llvm.x86.avx512.mask.vcvtph2ps.512",
                        rty,
                        &p,
                        Mem::None,
                    );
                    self.intrinsic_call(f, ops)
                } else {
                    let (_, dn) = self.x86_lanes(rty);
                    let mut src = ops[0];
                    if self.x86_lanes(ty(self, src)).1 != dn {
                        src = self.shuffle(src, None, vec![0, 1, 2, 3]);
                    }
                    let ht = self.m.types.vector(LTy::HALF, dn);
                    let h = self.cast(CastOp::BitCast, src, ht);
                    let r = self.cast(CastOp::FPExt, h, rty);
                    if ops.len() >= 3 { self.x86_select(ops[2], r, ops[1]) } else { r }
                }
            }
            Fam::Fma(addsub) => {
                let subtract = n.starts_with("vfmsub");
                let ph = n.contains("ph");
                let iid = n.contains("512").then(|| {
                    let sfx = if ph {
                        "ph"
                    } else if n.contains("ps") {
                        "ps"
                    } else {
                        "pd"
                    };
                    let isa = if ph { "avx512fp16" } else { "avx512" };
                    let op = if addsub { "vfmaddsub" } else { "vfmadd" };
                    format!("llvm.x86.{isa}.{op}.{sfx}.512")
                });
                let (a, b2) = (ops[0], ops[1]);
                let c = if subtract { self.fneg(ops[2]) } else { ops[2] };
                let last = ops[ops.len() - 1];
                let vt = ty(self, a);
                let r = match iid {
                    Some(iid) if addsub || k(self, last) != 4 => {
                        let p = [(vt, false), (vt, false), (vt, false), (LTy::I32, true)];
                        let f = intrinsics::target_intrinsic(&mut self.m, &iid, vt, &p, Mem::None);
                        self.intrinsic_call(f, vec![a, b2, c, last])
                    }
                    _ => {
                        let f = intrinsics::fp_op(&mut self.m, "llvm.fma", vt, 3);
                        self.intrinsic_call(f, vec![a, b2, c])
                    }
                };
                if n.ends_with("_mask") {
                    self.x86_select(ops[3], r, ops[0])
                } else if n.ends_with("_maskz") {
                    let z = V::C(self.m.const_zero(vt));
                    self.x86_select(ops[3], r, z)
                } else if n.ends_with("_mask3") {
                    self.x86_select(ops[3], r, ops[2])
                } else {
                    r
                }
            }
            Fam::ScalarFma => {
                let (upper, zero, pt, neg) = if n == "vfmaddss" || n == "vfmaddsd" {
                    (V::C(self.m.const_zero(ty(self, ops[0]))), false, 0, false)
                } else if n.ends_with("_maskz") {
                    (ops[0], true, 0, false)
                } else if n.ends_with("_mask3") {
                    (ops[2], false, 2, n.starts_with("vfmsub"))
                } else {
                    (ops[0], false, 0, false)
                };
                self.x86_scalar_fma(ops, upper, zero, pt, neg)
            }
            Fam::MaskOp => self.x86_mask_op(n, &ops, rty),
            Fam::IntCmp(signed) => {
                let cc = k(self, ops[2]) & 7;
                self.x86_masked_cmp(cc, signed, ops[0], ops[1], Some(ops[3]))
            }
            Fam::FpCmp => {
                let p = crate::x86::imm_cmp(k(self, ops[2]) & 0x1f);
                let (_, n) = self.x86_lanes(ty(self, ops[0]));
                let c = self.fcmp(p, ops[0], ops[1]);
                self.x86_mask_result(c, n, Some(ops[3]))
            }
            Fam::Vpcom(signed) => {
                use IntPredicate::{Eq, Ne, Sge, Sgt, Sle, Slt, Uge, Ugt, Ule, Ult};
                let t = ty(self, ops[0]);
                let p = match k(self, ops[2]) & 7 {
                    0 => Some(if signed { Slt } else { Ult }),
                    1 => Some(if signed { Sle } else { Ule }),
                    2 => Some(if signed { Sgt } else { Ugt }),
                    3 => Some(if signed { Sge } else { Uge }),
                    4 => Some(Eq),
                    5 => Some(Ne),
                    _ => None,
                };
                match p {
                    Some(p) => {
                        let c = self.icmp(p, ops[0], ops[1]);
                        self.cast(CastOp::SExt, c, t)
                    }
                    None if k(self, ops[2]) & 7 == 6 => V::C(self.m.const_zero(t)),
                    None => self.x86_ones(t),
                }
            }
            Fam::Funnel => {
                let rot = n.starts_with("vprot") || n.starts_with("pro");
                let right = n.starts_with("pror") || n.starts_with("vpshrd");
                let (a, b2, amt) = if rot {
                    (ops[0], ops[0], ops[1])
                } else if right {
                    (ops[1], ops[0], ops[2])
                } else {
                    (ops[0], ops[1], ops[2])
                };
                self.x86_funnel(a, b2, amt, right)
            }
            Fam::Ternlog(zero) => {
                let t = ty(self, ops[0]);
                let w = self.x86_bits(t).unwrap_or(128);
                let ew = if self.x86_elt_bits(t) == 32 { "d" } else { "q" };
                let iid = format!("llvm.x86.avx512.pternlog.{ew}.{w}");
                let p = [(t, false), (t, false), (t, false), (LTy::I32, true)];
                let f = intrinsics::target_intrinsic(&mut self.m, &iid, t, &p, Mem::None);
                let r = self.intrinsic_call(f, ops[..4].to_vec());
                let pt = if zero { V::C(self.m.const_zero(t)) } else { ops[0] };
                self.x86_select(ops[4], r, pt)
            }
            Fam::IntToFp(signed) => {
                let t = ty(self, ops[1]);
                let st = ty(self, ops[0]);
                let r = if k(self, ops[3]) != 4 {
                    let base = if signed {
                        "llvm.x86.avx512.sitofp.round"
                    } else {
                        "llvm.x86.avx512.uitofp.round"
                    };
                    let iid = intrinsics::overloaded_name(&self.m, base, &[t, st]);
                    let p = [(st, false), (LTy::I32, true)];
                    let f = intrinsics::target_intrinsic(&mut self.m, &iid, t, &p, Mem::None);
                    self.intrinsic_call(f, vec![ops[0], ops[3]])
                } else {
                    let op = if signed { CastOp::SIToFP } else { CastOp::UIToFP };
                    self.cast(op, ops[0], t)
                };
                self.x86_select(ops[2], r, ops[1])
            }
            Fam::Select => self.x86_select(ops[0], ops[1], ops[2]),
            Fam::ScalarSelect => {
                let a = self.extract_elt(ops[1], z64);
                let b2 = self.extract_elt(ops[2], z64);
                let r = self.x86_scalar_select(ops[0], a, b2);
                self.insert_elt(ops[1], r, z64)
            }
            Fam::Lzcnt => {
                let t = ty(self, ops[0]);
                let f = intrinsics::ctlz(&mut self.m, t);
                let no = V::C(self.m.const_bool(false));
                self.intrinsic_call(f, vec![ops[0], no])
            }
            Fam::Sqrt => {
                let t = ty(self, ops[0]);
                if ops.len() == 2 && k(self, ops[1]) != 4 {
                    let iid = match n {
                        "sqrtph512" => "llvm.x86.avx512fp16.sqrt.ph.512",
                        "sqrtps512" => "llvm.x86.avx512.sqrt.ps.512",
                        _ => "llvm.x86.avx512.sqrt.pd.512",
                    };
                    let p = [(t, false), (LTy::I32, true)];
                    let f = intrinsics::target_intrinsic(&mut self.m, iid, t, &p, Mem::None);
                    self.intrinsic_call(f, ops)
                } else {
                    let f = intrinsics::fp_op(&mut self.m, "llvm.sqrt", t, 1);
                    self.intrinsic_call(f, vec![ops[0]])
                }
            }
            Fam::ScalarSqrt => {
                if k(self, ops[4]) != 4 {
                    let iid = match n {
                        "sqrtsh_round_mask" => "llvm.x86.avx512fp16.mask.sqrt.sh",
                        "sqrtsd_round_mask" => "llvm.x86.avx512.mask.sqrt.sd",
                        _ => "llvm.x86.avx512.mask.sqrt.ss",
                    };
                    let p = self.x86_params(&ops, &[4]);
                    let f = intrinsics::target_intrinsic(&mut self.m, iid, rty, &p, Mem::None);
                    self.intrinsic_call(f, ops)
                } else {
                    let a = self.extract_elt(ops[1], z64);
                    let et = ty(self, a);
                    let f = intrinsics::fp_op(&mut self.m, "llvm.sqrt", et, 1);
                    let a = self.intrinsic_call(f, vec![a]);
                    let src = self.extract_elt(ops[2], z64);
                    let a = self.x86_scalar_select(ops[3], a, src);
                    self.insert_elt(ops[0], a, z64)
                }
            }
            Fam::Reduce => {
                let op = &n[7..11];
                let (f, vals, flag) = if op == "fadd" || op == "fmul" {
                    let vt = ty(self, ops[1]);
                    let f = intrinsics::vector_reduce_start(&mut self.m, op, vt);
                    (f, vec![ops[0], ops[1]], FastMath::REASSOC)
                } else {
                    let vt = ty(self, ops[0]);
                    let f = intrinsics::vector_reduce(&mut self.m, op, vt);
                    (f, vec![ops[0]], FastMath::NNAN)
                };
                let old = self.f.fir.fmf();
                self.f.fir.set_fmf(FastMath(old.0 | flag.0));
                let r = self.intrinsic_call(f, vals);
                self.f.fir.set_fmf(old);
                r
            }
            Fam::FpClass => {
                let t = ty(self, ops[0]);
                let (et, n2) = self.x86_lanes(t);
                let w = self.x86_bits(t).unwrap_or(128);
                let iid = if et == LTy::HALF {
                    format!("llvm.x86.avx512fp16.fpclass.ph.{w}")
                } else if et == LTy::FLOAT {
                    format!("llvm.x86.avx512.fpclass.ps.{w}")
                } else {
                    format!("llvm.x86.avx512.fpclass.pd.{w}")
                };
                let mask_in = ops.remove(2);
                let bt = self.m.types.vector(LTy::I1, n2);
                let p = [(t, false), (LTy::I32, true)];
                let f = intrinsics::target_intrinsic(&mut self.m, &iid, bt, &p, Mem::None);
                let c = self.intrinsic_call(f, ops);
                self.x86_mask_result(c, n2, Some(mask_in))
            }
            Fam::P2Intersect => {
                let t = ty(self, ops[0]);
                let (_, n2) = self.x86_lanes(t);
                let w = self.x86_bits(t).unwrap_or(128);
                let dq = if self.x86_elt_bits(t) == 32 { "d" } else { "q" };
                let iid = format!("llvm.x86.avx512.vp2intersect.{dq}.{w}");
                let bt = self.m.types.vector(LTy::I1, n2);
                let st = self.m.types.struct_literal(&[bt, bt], false);
                let p = [(t, false), (t, false)];
                let f = intrinsics::target_intrinsic(&mut self.m, &iid, st, &p, Mem::None);
                let c = self.intrinsic_call(f, vec![ops[0], ops[1]]);
                for (field, to) in [(0, ops[2]), (1, ops[3])] {
                    let x = self.f.fir.push(Op::ExtractValue { agg: c, idx: field }, bt);
                    let x = self.x86_mask_result(x, n2, None);
                    let xt = ty(self, x);
                    let align = u64::from(self.m.types.int_width(xt).unwrap_or(8) / 8);
                    self.store(x, to, align, false);
                }
                return Ok(Some(None));
            }
            Fam::ShufBitQmb => {
                let t = ty(self, ops[0]);
                let (_, n2) = self.x86_lanes(t);
                let w = self.x86_bits(t).unwrap_or(128);
                let iid = format!("llvm.x86.avx512.vpshufbitqmb.{w}");
                let mask_in = ops.remove(2);
                let bt = self.m.types.vector(LTy::I1, n2);
                let p = [(t, false), (t, false)];
                let f = intrinsics::target_intrinsic(&mut self.m, &iid, bt, &p, Mem::None);
                let c = self.intrinsic_call(f, ops);
                self.x86_mask_result(c, n2, Some(mask_in))
            }
            Fam::Bf16 => {
                if n == "cvtsbf162ss_32" {
                    self.cast(CastOp::FPExt, ops[0], LTy::FLOAT)
                } else if n == "cvtneps2bf16_128_mask" {
                    let (_, n2) = self.x86_lanes(ty(self, ops[0]));
                    ops[2] = self.x86_mask_vec(ops[2], n2);
                    let p = self.x86_params(&ops, &[]);
                    let f = intrinsics::target_intrinsic(
                        &mut self.m,
                        "llvm.x86.avx512bf16.mask.cvtneps2bf16.128",
                        rty,
                        &p,
                        Mem::None,
                    );
                    self.intrinsic_call(f, ops)
                } else {
                    let w = if n.contains("256") { 256 } else { 512 };
                    let iid = format!("llvm.x86.avx512bf16.cvtneps2bf16.{w}");
                    let p = [(ty(self, ops[0]), false)];
                    let f = intrinsics::target_intrinsic(&mut self.m, &iid, rty, &p, Mem::None);
                    let r = self.intrinsic_call(f, vec![ops[0]]);
                    self.x86_select(ops[2], r, ops[1])
                }
            }
            Fam::MaskToVec => {
                let (_, n2) = self.x86_lanes(rty);
                let m = self.x86_mask_vec(ops[0], n2);
                self.cast(CastOp::SExt, m, rty)
            }
            Fam::VecToMask => {
                let z = V::C(self.m.const_zero(ty(self, ops[0])));
                self.x86_masked_cmp(1, true, ops[0], z, None)
            }
            Fam::Trunc => {
                let t = ty(self, ops[1]);
                let r = self.cast(CastOp::Trunc, ops[0], t);
                self.x86_select(ops[2], r, ops[1])
            }
            Fam::MaskedLoad(aligned) => {
                let vt = ty(self, ops[1]);
                let align = if aligned { self.x86_vec_align(args[1]) } else { 1 };
                let (_, n2) = self.x86_lanes(vt);
                let m = self.x86_mask_vec(ops[2], n2);
                let f = intrinsics::masked_load(&mut self.m, vt);
                if self.m.ir_version >= 22 {
                    let r = self.intrinsic_call(f, vec![ops[0], m, ops[1]]);
                    self.x86_param_align(r, 0, align);
                    r
                } else {
                    let a = V::C(self.m.const_int(LTy::I32, i128::from(align)));
                    self.intrinsic_call(f, vec![ops[0], a, m, ops[1]])
                }
            }
            Fam::MaskedStore(aligned) => {
                let vt = ty(self, ops[1]);
                let align = if aligned { self.x86_vec_align(args[1]) } else { 1 };
                let (_, n2) = self.x86_lanes(vt);
                let m = self.x86_mask_vec(ops[2], n2);
                let f = intrinsics::masked_store(&mut self.m, vt);
                if self.m.ir_version >= 22 {
                    let r = self.intrinsic_call(f, vec![ops[1], ops[0], m]);
                    self.x86_param_align(r, 1, align);
                } else {
                    let a = V::C(self.m.const_int(LTy::I32, i128::from(align)));
                    self.intrinsic_call(f, vec![ops[1], ops[0], a, m]);
                }
                return Ok(Some(None));
            }
            Fam::ExpandLoad => {
                let vt = ty(self, ops[1]);
                let (_, n2) = self.x86_lanes(vt);
                let m = self.x86_mask_vec(ops[2], n2);
                let f = intrinsics::masked_expandload(&mut self.m, vt);
                self.intrinsic_call(f, vec![ops[0], m, ops[1]])
            }
            Fam::CompressStore => {
                let vt = ty(self, ops[1]);
                let (_, n2) = self.x86_lanes(vt);
                let m = self.x86_mask_vec(ops[2], n2);
                let f = intrinsics::masked_compressstore(&mut self.m, vt);
                self.intrinsic_call(f, vec![ops[1], ops[0], m]);
                return Ok(Some(None));
            }
            Fam::CompressExpand(compress) => {
                let vt = ty(self, ops[1]);
                let (_, n2) = self.x86_lanes(vt);
                let m = self.x86_mask_vec(ops[2], n2);
                let base = if compress {
                    "llvm.x86.avx512.mask.compress"
                } else {
                    "llvm.x86.avx512.mask.expand"
                };
                let iid = intrinsics::overloaded_name(&self.m, base, &[vt]);
                let mt = ty(self, m);
                let p = [(vt, false), (vt, false), (mt, false)];
                let f = intrinsics::target_intrinsic(&mut self.m, &iid, vt, &p, Mem::None);
                self.intrinsic_call(f, vec![ops[0], ops[1], m])
            }
            Fam::Gather => {
                let (_, a) = self.x86_lanes(ty(self, ops[0]));
                let (_, c) = self.x86_lanes(ty(self, ops[2]));
                ops[3] = self.x86_mask_vec(ops[3], a.min(c));
                let (stem, t) = n.split_at(n.len() - 2);
                let iid = format!("llvm.x86.avx512.mask.{stem}.{t}");
                let p = self.x86_params(&ops, &[4]);
                let f = intrinsics::target_intrinsic(&mut self.m, &iid, rty, &p, Mem::Read);
                self.intrinsic_call(f, ops)
            }
            Fam::Scatter => {
                let (_, a) = self.x86_lanes(ty(self, ops[2]));
                let (_, c) = self.x86_lanes(ty(self, ops[3]));
                ops[1] = self.x86_mask_vec(ops[1], a.min(c));
                let (stem, t) = n.split_at(n.len() - 2);
                let iid = format!("llvm.x86.avx512.mask.{stem}.{t}");
                let p = self.x86_params(&ops, &[4]);
                let f =
                    intrinsics::target_intrinsic(&mut self.m, &iid, LTy::VOID, &p, Mem::Unknown);
                self.intrinsic_call(f, ops);
                return Ok(Some(None));
            }
            Fam::Xsave => {
                let k32 = V::C(self.m.const_int(LTy::I64, 32));
                let hi = self.bin(BinOp::LShr, Flags::NONE, ops[1], k32);
                let hi = self.cast(CastOp::Trunc, hi, LTy::I32);
                let lo = self.cast(CastOp::Trunc, ops[1], LTy::I32);
                ops[1] = hi;
                ops.push(lo);
                let iid = format!("llvm.x86.{n}");
                let p = self.x86_params(&ops, &[]);
                let f =
                    intrinsics::target_intrinsic(&mut self.m, &iid, LTy::VOID, &p, Mem::Unknown);
                self.intrinsic_call(f, ops);
                return Ok(Some(None));
            }
            Fam::Prefetchi => {
                let f = intrinsics::prefetch(&mut self.m);
                let z = V::C(self.m.const_int(LTy::I32, 0));
                self.intrinsic_call(f, vec![ops[0], z, ops[1], z]);
                return Ok(Some(None));
            }
            Fam::ComplexFma => {
                let conj = n.starts_with("vfc");
                let c = if conj { "vfcmadd" } else { "vfmadd" };
                let sh = n.contains("csh");
                let iid = if sh {
                    format!("llvm.x86.avx512fp16.mask.{c}.csh")
                } else {
                    format!("llvm.x86.avx512fp16.mask.{c}.cph.512")
                };
                let p = self.x86_params(&ops, &[4]);
                let t = ty(self, ops[0]);
                let f = intrinsics::target_intrinsic(&mut self.m, &iid, t, &p, Mem::None);
                let r = self.intrinsic_call(f, ops.clone());
                if n.ends_with("mask3") {
                    self.shuffle(r, Some(ops[2]), vec![0, 5, 6, 7])
                } else if sh {
                    let one = V::C(self.m.const_int(LTy::I8, 1));
                    let m = self.bin(BinOp::And, Flags::NONE, ops[3], one);
                    self.x86_select(m, r, ops[0])
                } else {
                    self.x86_select(ops[3], r, ops[0])
                }
            }
            Fam::Permil => {
                let t = ty(self, ops[0]);
                let (_, total) = self.x86_lanes(t);
                let lanes = self.x86_bits(t).unwrap_or(128) / 128;
                let per = total / lanes;
                let mut im = (k(self, ops[1]) & 0xff) * 0x0101_0101;
                let mut mask = Vec::with_capacity(total as usize);
                for l in (0..total).step_by(per as usize) {
                    for _ in 0..per {
                        mask.push(im % per + l);
                        im /= per;
                    }
                }
                self.shuffle(ops[0], None, mask)
            }
            Fam::Perm4x64 => {
                let t = ty(self, ops[0]);
                let (_, total) = self.x86_lanes(t);
                let im = k(self, ops[1]);
                let mask = (0..total).map(|i| (i & !3) + ((im >> (2 * (i & 3))) & 3)).collect();
                self.shuffle(ops[0], None, mask)
            }
            Fam::Valign => {
                let (_, total) = self.x86_lanes(ty(self, ops[0]));
                let shift = (k(self, ops[2]) & 0xff) & (total - 1);
                self.shuffle(ops[1], Some(ops[0]), (0..total).map(|i| i + shift).collect())
            }
            Fam::ShufLanes => {
                let t = ty(self, ops[0]);
                let (_, total) = self.x86_lanes(t);
                let lanes = if self.x86_bits(t) == Some(512) { 4 } else { 2 };
                let per = total / lanes;
                let mut im = k(self, ops[2]);
                let mut mask = Vec::with_capacity(total as usize);
                for l in (0..total).step_by(per as usize) {
                    let mut idx = (im % lanes) * per;
                    im /= lanes;
                    if l >= total / 2 {
                        idx += total;
                    }
                    mask.extend((0..per).map(|i| idx + i));
                }
                self.shuffle(ops[0], Some(ops[1]), mask)
            }
        };
        Ok(Some(Some(v)))
    }

    /// The operands of call `args` to builtin `b`, emitted in order (clang
    /// `EmitX86BuiltinExpr`'s `Ops`): an integer constant one is a
    /// constant of its parameter type.
    fn x86_ops(&mut self, b: u32, args: &[ExprId]) -> R<Vec<V>> {
        let ice = id::ice_params(b);
        let mut ops = Vec::with_capacity(args.len());
        for (i, &a) in args.iter().enumerate() {
            let t = self.lty(self.ast.exprs.get(a).ty());
            if ice & (1 << i) != 0 && self.m.types.int_width(t).is_some() {
                let c = self.x86_imm(a)?;
                ops.push(V::C(self.m.const_int(t, c)));
            } else {
                ops.push(self.emit_scalar(a)?);
            }
        }
        Ok(ops)
    }

    /// The `(type, immarg)` parameters of an intrinsic taking `ops`, the
    /// operands at `imm` being `immarg`.
    fn x86_params(&self, ops: &[V], imm: &[usize]) -> Vec<(LTy, bool)> {
        ops.iter().enumerate().map(|(i, &v)| (self.f.fir.ty(&self.m, v), imm.contains(&i))).collect()
    }

    /// The alignment of the vector type of argument `a` (clang
    /// `getTypeAlignInChars` of the aligned masked loads and stores).
    fn x86_vec_align(&mut self, a: ExprId) -> u64 {
        let t = self.lty(self.ast.exprs.get(a).ty());
        u64::from(self.x86_bits(t).unwrap_or(128) / 8)
    }

    /// An `align` attribute on parameter `i` of the call `r`.
    fn x86_param_align(&mut self, r: V, i: usize, align: u64) {
        let a = self.m.attr_set(&[Attr::Align(align)]);
        if let Some(o) = self.f.fir.call_opts_mut(Fir::last_inst(r)) {
            o.attrs.set_param(i, a);
        }
    }

    /// An all-ones constant of integer or integer vector type `t`.
    fn x86_ones(&mut self, t: LTy) -> V {
        let (et, n) = self.x86_lanes(t);
        let one = self.m.const_int(et, -1);
        if self.m.types.vector_info(t).is_none() {
            return V::C(one);
        }
        V::C(self.m.const_vector(&vec![one; n as usize]).unwrap_or(one))
    }

    /// Whether `v` is an all-ones integer constant.
    fn x86_all_ones(&self, v: V) -> bool {
        self.cint(v).is_some_and(|(_, x, w)| {
            let m = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
            w > 0 && x & m == m
        })
    }

    /// clang `getMaskVecValue`: integer mask `m` as `<W x i1>`, its low
    /// `n` elements when `n` is below 8.
    pub(crate) fn x86_mask_vec(&mut self, m: V, n: u32) -> V {
        let mt = self.f.fir.ty(&self.m, m);
        let w = self.m.types.int_width(mt).unwrap_or(8);
        let vt = self.m.types.vector(LTy::I1, w);
        let v = self.cast(CastOp::BitCast, m, vt);
        if n < 8 { self.shuffle(v, Some(v), (0..n).collect()) } else { v }
    }

    /// clang `EmitX86Select`: `a` where mask `m` is set, else `b`.
    pub(crate) fn x86_select(&mut self, m: V, a: V, b: V) -> V {
        if self.x86_all_ones(m) {
            return a;
        }
        let (_, n) = self.x86_lanes(self.f.fir.ty(&self.m, a));
        let mv = self.x86_mask_vec(m, n);
        self.select(mv, a, b)
    }

    /// clang `EmitX86ScalarSelect`: `a` if bit 0 of mask `m` is set.
    fn x86_scalar_select(&mut self, m: V, a: V, b: V) -> V {
        if self.x86_all_ones(m) {
            return a;
        }
        let mt = self.f.fir.ty(&self.m, m);
        let w = self.m.types.int_width(mt).unwrap_or(8);
        let vt = self.m.types.vector(LTy::I1, w);
        let v = self.cast(CastOp::BitCast, m, vt);
        let z = V::C(self.m.const_int(LTy::I64, 0));
        let c = self.extract_elt(v, z);
        self.select(c, a, b)
    }

    /// clang `EmitX86MaskedCompareResult`: the `<n x i1>` compare `c`,
    /// masked by `mask_in`, widened to 8 lanes and bitcast to an integer.
    fn x86_mask_result(&mut self, c: V, n: u32, mask_in: Option<V>) -> V {
        let mut c = c;
        if let Some(m) = mask_in
            && !self.x86_all_ones(m)
        {
            let mv = self.x86_mask_vec(m, n);
            c = self.bin(BinOp::And, Flags::NONE, c, mv);
        }
        if n < 8 {
            let ct = self.f.fir.ty(&self.m, c);
            let z = V::C(self.m.const_zero(ct));
            let mask = (0..8).map(|i| if i < n { i } else { i % n + n }).collect();
            c = self.shuffle(c, Some(z), mask);
        }
        let it = self.m.types.int(n.max(8));
        self.cast(CastOp::BitCast, c, it)
    }

    /// clang `EmitX86MaskedCompare` with condition code `cc` (0 eq, 1 lt,
    /// 2 le, 3 false, 4 ne, 5 ge, 6 gt, 7 true).
    fn x86_masked_cmp(&mut self, cc: u32, signed: bool, a: V, b: V, mask_in: Option<V>) -> V {
        use IntPredicate::{Eq, Ne, Sge, Sgt, Sle, Slt, Uge, Ugt, Ule, Ult};
        let (_, n) = self.x86_lanes(self.f.fir.ty(&self.m, a));
        let bt = self.m.types.vector(LTy::I1, n);
        let c = match cc {
            3 => V::C(self.m.const_zero(bt)),
            7 => self.x86_ones(bt),
            _ => {
                let p = match cc {
                    0 => Eq,
                    1 => {
                        if signed {
                            Slt
                        } else {
                            Ult
                        }
                    }
                    2 => {
                        if signed {
                            Sle
                        } else {
                            Ule
                        }
                    }
                    4 => Ne,
                    5 => {
                        if signed {
                            Sge
                        } else {
                            Uge
                        }
                    }
                    _ => {
                        if signed {
                            Sgt
                        } else {
                            Ugt
                        }
                    }
                };
                self.icmp(p, a, b)
            }
        };
        self.x86_mask_result(c, n, mask_in)
    }

    /// clang `EmitX86FunnelShift`: `llvm.fsh{l,r}(a, b, amt)`, a scalar
    /// amount cast to the element type and splat.
    fn x86_funnel(&mut self, a: V, b: V, amt: V, right: bool) -> V {
        let t = self.f.fir.ty(&self.m, a);
        let mut amt = amt;
        if self.f.fir.ty(&self.m, amt) != t {
            let (et, _) = self.x86_lanes(t);
            amt = self.x86_int_cast(amt, et);
            amt = self.splat(amt, t);
        }
        let name = if right { "llvm.fshr" } else { "llvm.fshl" };
        let f = intrinsics::fp_op(&mut self.m, name, t, 3);
        self.intrinsic_call(f, vec![a, b, amt])
    }

    /// clang `CreateIntCast(v, to, /*isSigned*/ false)`.
    fn x86_int_cast(&mut self, v: V, to: LTy) -> V {
        let from = self.f.fir.ty(&self.m, v);
        let (fw, tw) = (self.m.types.int_width(from), self.m.types.int_width(to));
        match (fw, tw) {
            (Some(a), Some(b)) if a > b => self.cast(CastOp::Trunc, v, to),
            (Some(a), Some(b)) if a < b => self.cast(CastOp::ZExt, v, to),
            _ => v,
        }
    }

    /// clang `EmitScalarFMAExpr`.
    fn x86_scalar_fma(&mut self, ops: Vec<V>, upper: V, zero: bool, pt: usize, neg: bool) -> V {
        let mut ops = ops;
        let rnd = if ops.len() > 4 { self.cint(ops[4]).map_or(4, |c| c.1) } else { 4 };
        if neg {
            ops[2] = self.fneg(ops[2]);
        }
        let z = V::C(self.m.const_int(LTy::I64, 0));
        for op in ops.iter_mut().take(3) {
            *op = self.extract_elt(*op, z);
        }
        let t = self.f.fir.ty(&self.m, ops[0]);
        let mut r = if rnd == 4 {
            let f = intrinsics::fp_op(&mut self.m, "llvm.fma", t, 3);
            self.intrinsic_call(f, ops[..3].to_vec())
        } else {
            let iid = match t {
                LTy::HALF => "llvm.x86.avx512fp16.vfmadd.f16",
                LTy::FLOAT => "llvm.x86.avx512.vfmadd.f32",
                _ => "llvm.x86.avx512.vfmadd.f64",
            };
            let p = [(t, false), (t, false), (t, false), (LTy::I32, true)];
            let f = intrinsics::target_intrinsic(&mut self.m, iid, t, &p, Mem::None);
            self.intrinsic_call(f, vec![ops[0], ops[1], ops[2], ops[4]])
        };
        if ops.len() > 3 {
            let mut pass = if zero { V::C(self.m.const_zero(t)) } else { ops[pt] };
            if neg && pt == 2 {
                pass = self.extract_elt(upper, z);
            }
            r = self.x86_scalar_select(ops[3], r, pass);
        }
        self.insert_elt(upper, r, z)
    }

    /// The `k*` mask builtins (clang `EmitX86MaskLogic` and the
    /// `kshift`, `kunpck`, `kortest`, `knot`, `kmov` cases).
    fn x86_mask_op(&mut self, n: &str, ops: &[V], rty: LTy) -> V {
        let t = self.f.fir.ty(&self.m, ops[0]);
        let w = self.m.types.int_width(t).unwrap_or(8);
        let logic = |s: &mut Self, op: BinOp, inv: bool| {
            let mut l = s.x86_mask_vec(ops[0], w);
            let r = s.x86_mask_vec(ops[1], w);
            if inv {
                let bt = s.f.fir.ty(&s.m, l);
                let ones = s.x86_ones(bt);
                l = s.bin(BinOp::Xor, Flags::NONE, l, ones);
            }
            let x = s.bin(op, Flags::NONE, l, r);
            s.cast(CastOp::BitCast, x, t)
        };
        let stem = ["qi", "hi", "si", "di", "b", "w", "d", "q"]
            .iter()
            .find_map(|s| n.strip_suffix(s))
            .unwrap_or(n);
        match stem {
            "kand" => logic(self, BinOp::And, false),
            "kandn" => logic(self, BinOp::And, true),
            "kor" => logic(self, BinOp::Or, false),
            "kxnor" => logic(self, BinOp::Xor, true),
            "kxor" => logic(self, BinOp::Xor, false),
            "knot" | "kmov" => {
                let mut v = self.x86_mask_vec(ops[0], w);
                if stem == "knot" {
                    let bt = self.f.fir.ty(&self.m, v);
                    let ones = self.x86_ones(bt);
                    v = self.bin(BinOp::Xor, Flags::NONE, v, ones);
                }
                self.cast(CastOp::BitCast, v, t)
            }
            "kunpck" => {
                let l = self.x86_mask_vec(ops[0], w);
                let r = self.x86_mask_vec(ops[1], w);
                let l = self.shuffle(l, Some(l), (0..w / 2).collect());
                let r = self.shuffle(r, Some(r), (0..w / 2).collect());
                let x = self.shuffle(r, Some(l), (0..w).collect());
                self.cast(CastOp::BitCast, x, t)
            }
            "kortestc" | "kortestz" => {
                let or = logic(self, BinOp::Or, false);
                let c = if stem == "kortestc" {
                    self.x86_ones(t)
                } else {
                    V::C(self.m.const_zero(t))
                };
                let cmp = self.icmp(IntPredicate::Eq, or, c);
                self.cast(CastOp::ZExt, cmp, rty)
            }
            _ => {
                // `kshift{l,r}i`.
                let shift = self.cint(ops[1]).map_or(0, |c| c.1 as u32) & 0xff;
                if shift >= w {
                    return V::C(self.m.const_zero(t));
                }
                let v = self.x86_mask_vec(ops[0], w);
                let bt = self.f.fir.ty(&self.m, v);
                let z = V::C(self.m.const_zero(bt));
                let x = if n.starts_with("kshiftl") {
                    self.shuffle(z, Some(v), (0..w).map(|i| w + i - shift).collect())
                } else {
                    self.shuffle(v, Some(z), (0..w).map(|i| i + shift).collect())
                };
                self.cast(CastOp::BitCast, x, t)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Fam, family};

    #[test]
    fn families() {
        assert_eq!(family("extract128i256"), Some(Fam::Extract));
        assert_eq!(family("extracti64x4_mask"), Some(Fam::Extract));
        assert_eq!(family("inserti64x4"), Some(Fam::Insert));
        assert_eq!(family("permti256"), Some(Fam::Perm2x128));
        assert_eq!(family("vcvtph2ps256"), Some(Fam::CvtPh2Ps));
        assert_eq!(family("vfmaddpd256"), Some(Fam::Fma(false)));
        assert_eq!(family("vfmsubaddps512_mask3"), Some(Fam::Fma(true)));
        assert_eq!(family("vfmaddsubpd512_maskz"), Some(Fam::Fma(true)));
        assert_eq!(family("vfmsubpd512_mask3"), Some(Fam::Fma(false)));
        assert_eq!(family("vfmsubss3_mask3"), Some(Fam::ScalarFma));
        assert_eq!(family("vfmaddsh3_maskz"), Some(Fam::ScalarFma));
        assert_eq!(family("kandnhi"), Some(Fam::MaskOp));
        assert_eq!(family("kshiftridi"), Some(Fam::MaskOp));
        assert_eq!(family("ucmpw512_mask"), Some(Fam::IntCmp(false)));
        assert_eq!(family("cmppd256_mask"), Some(Fam::FpCmp));
        assert_eq!(family("vpshrdvq512"), Some(Fam::Funnel));
        assert_eq!(family("storedquqi512_mask"), Some(Fam::MaskedStore(false)));
        assert_eq!(family("movdqa64load256_mask"), Some(Fam::MaskedLoad(true)));
        assert_eq!(family("compressstoresi128_mask"), Some(Fam::CompressStore));
        assert_eq!(family("compressdf256_mask"), Some(Fam::CompressExpand(true)));
        assert_eq!(family("gather3siv4di"), Some(Fam::Gather));
        // 1:1 table rows stay out.
        assert_eq!(family("vfmaddsubps"), None);
        assert_eq!(family("vcvtps2ph256"), None);
        assert_eq!(family("cmpps"), None);
        assert_eq!(family("scatterdpd"), None);
        assert_eq!(family("gathersiv2df"), None);
    }
}
