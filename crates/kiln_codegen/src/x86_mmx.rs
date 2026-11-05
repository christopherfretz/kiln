//! clang 19's MMX builtins (`__builtin_ia32_paddb`, ...) and its `vpopcnt*`
//! ones (spec/landed/codegen/WP-630.md "MMX"; WP-630). clang 19 lowered
//! them to `x86_mmx` intrinsics, which LLVM 21 no longer has; clang 20
//! dropped the builtins and rebuilt `<mmintrin.h>`, `<xmmintrin.h>`,
//! `<emmintrin.h>` and `<tmmintrin.h>` on generic vector code and the SSE2
//! / SSSE3 builtins. Each builtin here is emitted as the clang 21 header's
//! definition of the intrinsic that calls it: 64-bit vector arithmetic,
//! or the 128-bit builtin on the operands widened (`__anyext128`,
//! `__zext128`, or the two operands concatenated) and the result's low 64
//! bits (`__trunc64`).

use kiln_ast::ExprId;
use kiln_llvm::{BinOp, CastOp, Flags, IntPredicate, SHUFFLE_POISON, TypeId as LTy, intrinsics};

use crate::fir::V;
use crate::module::{Cg, R, err};

/// The 128-bit builtin computing MMX builtin `n` on widened operands, and
/// how its operands are widened.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wide {
    /// Each operand `__anyext128` (the low half; the shift count of the
    /// `*i` forms stays a scalar).
    Any,
    /// Each operand `__zext128`.
    Zero,
    /// The two operands concatenated, with a zero second operand.
    Concat,
}

/// The 128-bit form of the MMX builtin `n`.
fn wide(n: &str) -> Option<(&'static str, Wide)> {
    use Wide::{Any, Concat, Zero};
    Some(match n {
        "pmulhw" => ("pmulhw128", Any),
        "pmaddwd" => ("pmaddwd128", Any),
        "pmulhuw" => ("pmulhuw128", Any),
        "pavgb" => ("pavgb128", Any),
        "pavgw" => ("pavgw128", Any),
        "pmaddubsw" => ("pmaddubsw128", Any),
        "pmulhrsw" => ("pmulhrsw128", Any),
        "psignb" => ("psignb128", Any),
        "psignw" => ("psignw128", Any),
        "psignd" => ("psignd128", Any),
        "psllw" => ("psllw128", Any),
        "pslld" => ("pslld128", Any),
        "psllq" => ("psllq128", Any),
        "psrlw" => ("psrlw128", Any),
        "psrld" => ("psrld128", Any),
        "psrlq" => ("psrlq128", Any),
        "psraw" => ("psraw128", Any),
        "psrad" => ("psrad128", Any),
        "psllwi" => ("psllwi128", Any),
        "pslldi" => ("pslldi128", Any),
        "psllqi" => ("psllqi128", Any),
        "psrlwi" => ("psrlwi128", Any),
        "psrldi" => ("psrldi128", Any),
        "psrlqi" => ("psrlqi128", Any),
        "psrawi" => ("psrawi128", Any),
        "psradi" => ("psradi128", Any),
        "psadbw" => ("psadbw128", Zero),
        "packsswb" => ("packsswb128", Concat),
        "packssdw" => ("packssdw128", Concat),
        "packuswb" => ("packuswb128", Concat),
        "phaddw" => ("phaddw128", Concat),
        "phaddd" => ("phaddd128", Concat),
        "phaddsw" => ("phaddsw128", Concat),
        "phsubw" => ("phsubw128", Concat),
        "phsubd" => ("phsubd128", Concat),
        "phsubsw" => ("phsubsw128", Concat),
        _ => return None,
    })
}

impl Cg<'_> {
    /// The MMX or `vpopcnt` builtin `n` (its `__builtin_ia32_` prefix
    /// removed) of call `e`: `None` when `n` is not one this lowers.
    pub(crate) fn x86_mmx(
        &mut self,
        n: &str,
        e: ExprId,
        args: &[ExprId],
        rty: LTy,
    ) -> R<Option<Option<V>>> {
        let mut ops = Vec::with_capacity(args.len());
        let lowered = wide(n).is_some()
            || n.starts_with("vpopcnt")
            || matches!(
                n,
                "paddb"
                    | "paddw"
                    | "paddd"
                    | "paddq"
                    | "psubb"
                    | "psubw"
                    | "psubd"
                    | "psubq"
                    | "pmullw"
                    | "pand"
                    | "pandn"
                    | "por"
                    | "pxor"
                    | "paddsb"
                    | "paddsw"
                    | "paddusb"
                    | "paddusw"
                    | "psubsb"
                    | "psubsw"
                    | "psubusb"
                    | "psubusw"
                    | "pcmpeqb"
                    | "pcmpeqw"
                    | "pcmpeqd"
                    | "pcmpgtb"
                    | "pcmpgtw"
                    | "pcmpgtd"
                    | "punpckhbw"
                    | "punpckhwd"
                    | "punpckhdq"
                    | "punpcklbw"
                    | "punpcklwd"
                    | "punpckldq"
                    | "pmaxsw"
                    | "pmaxub"
                    | "pminsw"
                    | "pminub"
                    | "pabsb"
                    | "pabsw"
                    | "pabsd"
                    | "pmovmskb"
                    | "pshufb"
                    | "pshufw"
                    | "palignr"
                    | "vec_init_v2si"
                    | "vec_init_v4hi"
                    | "vec_init_v8qi"
                    | "cvtpi2ps"
                    | "cvtps2pi"
                    | "cvttps2pi"
                    | "cvtpd2pi"
                    | "cvttpd2pi"
                    | "cvtpi2pd"
                    | "movntq"
                    | "pmuludq"
            );
        if !lowered {
            return Ok(None);
        }
        // The immediates (`pshufw`, `palignr`) are folded, as clang folds
        // them into its shuffles.
        let imm = |s: &mut Self, i: usize| -> R<u32> {
            match args.get(i).and_then(|&a| s.fold_int(a)) {
                Some(v) => Ok(v as u32),
                None => err(s.loc_of(e), "x86 builtin immediate is not a constant"),
            }
        };
        let imm_arg = matches!(n, "pshufw" | "palignr");
        for (i, &a) in args.iter().enumerate() {
            if imm_arg && i + 1 == args.len() {
                break;
            }
            ops.push(self.emit_scalar(a)?);
        }
        let op = |i: usize| ops.get(i).copied();
        let (Some(a), b) = (op(0), op(1)) else {
            return err(self.loc_of(e), format!("too few arguments to {n}"));
        };
        let two = |s: &Self| match b {
            Some(b) => Ok(b),
            None => err(s.loc_of(e), format!("too few arguments to {n}")),
        };
        let at = self.f.fir.ty(&self.m, a);
        let v = match n {
            _ if n.starts_with("vpopcnt") => {
                let f = intrinsics::ctpop(&mut self.m, at);
                self.intrinsic_call(f, vec![a])
            }
            "paddb" | "paddw" | "paddd" | "paddq" | "psubb" | "psubw" | "psubd" | "psubq"
            | "pmullw" | "pand" | "por" | "pxor" => {
                let op = match n {
                    "pmullw" => BinOp::Mul,
                    "pand" => BinOp::And,
                    "por" => BinOp::Or,
                    "pxor" => BinOp::Xor,
                    _ if n.starts_with("padd") => BinOp::Add,
                    _ => BinOp::Sub,
                };
                let y = two(self)?;
                self.bin(op, Flags::NONE, a, y)
            }
            "pandn" => {
                let ones = self.x86_splat(at, -1);
                let na = self.bin(BinOp::Xor, Flags::NONE, a, ones);
                self.bin(BinOp::And, Flags::NONE, na, two(self)?)
            }
            "paddsb" | "paddsw" | "paddusb" | "paddusw" | "psubsb" | "psubsw" | "psubusb"
            | "psubusw" => {
                let name = match (n.starts_with("padd"), n.contains("us")) {
                    (true, false) => "llvm.sadd.sat",
                    (true, true) => "llvm.uadd.sat",
                    (false, false) => "llvm.ssub.sat",
                    (false, true) => "llvm.usub.sat",
                };
                let f = intrinsics::fp_op(&mut self.m, name, at, 2);
                self.intrinsic_call(f, vec![a, two(self)?])
            }
            "pmaxsw" | "pmaxub" | "pminsw" | "pminub" => {
                let name = match n {
                    "pmaxsw" => "llvm.smax",
                    "pmaxub" => "llvm.umax",
                    "pminsw" => "llvm.smin",
                    _ => "llvm.umin",
                };
                let f = intrinsics::fp_op(&mut self.m, name, at, 2);
                self.intrinsic_call(f, vec![a, two(self)?])
            }
            "pabsb" | "pabsw" | "pabsd" => {
                let f = intrinsics::abs(&mut self.m, at);
                let no = V::C(self.m.const_bool(false));
                self.intrinsic_call(f, vec![a, no])
            }
            "pcmpeqb" | "pcmpeqw" | "pcmpeqd" | "pcmpgtb" | "pcmpgtw" | "pcmpgtd" => {
                let p = if n.contains("eq") { IntPredicate::Eq } else { IntPredicate::Sgt };
                let c = self.icmp(p, a, two(self)?);
                self.cast(CastOp::SExt, c, at)
            }
            "punpckhbw" | "punpckhwd" | "punpckhdq" | "punpcklbw" | "punpcklwd" | "punpckldq" => {
                let (_, count) = self.x86_lanes(at);
                let half = count / 2;
                let first = if n.starts_with("punpckh") { half } else { 0 };
                let mask = (0..count).map(|i| first + i / 2 + (i % 2) * count).collect();
                self.shuffle(a, Some(two(self)?), mask)
            }
            "vec_init_v2si" | "vec_init_v4hi" | "vec_init_v8qi" => {
                let mut v = V::C(self.m.const_poison(rty));
                for (i, &x) in ops.iter().enumerate() {
                    let idx = V::C(self.m.const_int(LTy::I64, i as i128));
                    v = self.insert_elt(v, x, idx);
                }
                v
            }
            // `__builtin_shufflevector` with a zero vector, lanes from the
            // first operand only.
            "pshufw" => {
                let im = imm(self, 1)?;
                let zero = V::C(self.m.const_zero(at));
                let mask = (0..4).map(|i| (im >> (2 * i)) & 3).collect();
                self.shuffle(a, Some(zero), mask)
            }
            // `psrldqi128_byteshift` of `b` then `a`, the low 64 bits.
            "palignr" => {
                let shift = imm(self, 2)? & 0xff;
                let q = self.m.types.vector(LTy::I64, 1);
                let a = self.cast(CastOp::BitCast, a, q);
                let b = self.cast(CastOp::BitCast, two(self)?, q);
                let both = self.shuffle(a, Some(b), vec![1, 0]);
                let q2 = self.m.types.vector(LTy::I64, 2);
                let s = self.x86_byteshift(both, shift, false, q2);
                self.x86_trunc64(s)
            }
            "pmovmskb" => {
                let z = self.x86_zext128(a);
                let r = self.x86_call_builtin("__builtin_ia32_pmovmskb128", vec![z], e)?;
                return Ok(Some(Some(r)));
            }
            // `pshufb128` of `a` repeated in both halves.
            "pshufb" => {
                let i2 = self.m.types.vector(LTy::I32, 2);
                let a2 = self.cast(CastOp::BitCast, a, i2);
                let zero = V::C(self.m.const_zero(i2));
                let rep = self.shuffle(a2, Some(zero), vec![0, 1, 0, 1]);
                let bb = self.x86_anyext128(two(self)?);
                let r = self.x86_call_builtin("__builtin_ia32_pshufb128", vec![rep, bb], e)?;
                self.x86_trunc64(r)
            }
            // `cvtps2dq` / `cvttps2dq` of the low two elements, the upper
            // two zeroed.
            "cvtps2pi" | "cvttps2pi" => {
                let i4 = self.m.types.vector(LTy::I32, 4);
                let a4 = self.cast(CastOp::BitCast, a, i4);
                let zero = V::C(self.m.const_zero(i4));
                let low = self.shuffle(a4, Some(zero), vec![0, 1, 4, 5]);
                let name = if n == "cvtps2pi" {
                    "__builtin_ia32_cvtps2dq"
                } else {
                    "__builtin_ia32_cvttps2dq"
                };
                let r = self.x86_call_builtin(name, vec![low], e)?;
                self.x86_trunc64(r)
            }
            "cvtpd2pi" | "cvttpd2pi" => {
                let name = if n == "cvtpd2pi" {
                    "__builtin_ia32_cvtpd2dq"
                } else {
                    "__builtin_ia32_cvttpd2dq"
                };
                let r = self.x86_call_builtin(name, vec![a], e)?;
                self.x86_trunc64(r)
            }
            "cvtpi2pd" => self.cast(CastOp::SIToFP, a, rty),
            // `pmuludq128`'s lowering on the widened operands.
            "pmuludq" => {
                let x = self.x86_anyext128(a);
                let y = self.x86_anyext128(two(self)?);
                let q2 = self.m.types.vector(LTy::I64, 2);
                let r = self.x86_muldq(x, y, false, q2);
                self.x86_trunc64(r)
            }
            // `__builtin_convertvector` of `b` widened with zeroes, lanes
            // 0 and 1 replacing `a`'s.
            "cvtpi2ps" => {
                let wb = self.x86_zext128(two(self)?);
                let f4 = self.cast(CastOp::SIToFP, wb, rty);
                self.shuffle(a, Some(f4), vec![4, 5, 2, 3])
            }
            // `__builtin_nontemporal_store` through the `__m64 *`.
            "movntq" => {
                let st = self.store(two(self)?, a, 8, false);
                self.mark_nontemporal(st)?;
                return Ok(Some(None));
            }
            _ => {
                let Some((wn, how)) = wide(n) else { return Ok(None) };
                let wn = format!("__builtin_ia32_{wn}");
                let mut wops = Vec::with_capacity(2);
                match how {
                    Wide::Concat => {
                        let q = self.m.types.vector(LTy::I64, 1);
                        let x = self.cast(CastOp::BitCast, a, q);
                        let y = self.cast(CastOp::BitCast, two(self)?, q);
                        wops.push(self.shuffle(x, Some(y), vec![0, 1]));
                        let q2 = self.m.types.vector(LTy::I64, 2);
                        wops.push(V::C(self.m.const_zero(q2)));
                    }
                    Wide::Zero => {
                        wops.push(self.x86_zext128(a));
                        let y = two(self)?;
                        wops.push(self.x86_zext128(y));
                    }
                    Wide::Any => {
                        wops.push(self.x86_anyext128(a));
                        let y = two(self)?;
                        // The `*i` shifts' count is a scalar.
                        let scalar = self.m.types.vector_info(self.f.fir.ty(&self.m, y)).is_none();
                        wops.push(if scalar { y } else { self.x86_anyext128(y) });
                    }
                }
                let r = self.x86_call_builtin(&wn, wops, e)?;
                self.x86_trunc64(r)
            }
        };
        let vt = self.f.fir.ty(&self.m, v);
        if vt != rty && rty != LTy::VOID {
            return Ok(Some(Some(self.x86_bitcast(v, vt, rty, e)?)));
        }
        Ok(Some(Some(v)))
    }

    /// The constant vector of type `t` with every element `x`.
    fn x86_splat(&mut self, t: LTy, x: i128) -> V {
        let (et, n) = self.x86_lanes(t);
        let k = self.m.const_int(et, x);
        self.m.const_vector(&vec![k; n as usize]).map_or(V::C(k), V::C)
    }

    /// clang's `__anyext128`: a 64-bit vector as the low half of a
    /// `<4 x i32>`, the upper half poison.
    fn x86_anyext128(&mut self, v: V) -> V {
        let i2 = self.m.types.vector(LTy::I32, 2);
        let v = self.cast(CastOp::BitCast, v, i2);
        let zero = V::C(self.m.const_zero(i2));
        self.shuffle(v, Some(zero), vec![0, 1, SHUFFLE_POISON, SHUFFLE_POISON])
    }

    /// clang's `__zext128`: the upper half zero.
    fn x86_zext128(&mut self, v: V) -> V {
        let i2 = self.m.types.vector(LTy::I32, 2);
        let v = self.cast(CastOp::BitCast, v, i2);
        let zero = V::C(self.m.const_zero(i2));
        self.shuffle(v, Some(zero), vec![0, 1, 2, 3])
    }

    /// clang's `__trunc64`: the low 64 bits of a 128-bit vector, as
    /// `<1 x i64>`.
    fn x86_trunc64(&mut self, v: V) -> V {
        let q2 = self.m.types.vector(LTy::I64, 2);
        let v = self.cast(CastOp::BitCast, v, q2);
        let zero = V::C(self.m.const_zero(q2));
        self.shuffle(v, Some(zero), vec![0])
    }
}
