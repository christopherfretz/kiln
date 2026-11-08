//! Calls of the SVE intrinsics `arm_sve.h` declares as aliases of clang
//! builtins (`__attribute__((__clang_arm_builtin_alias(B)))`, WP-810;
//! spec/landed/codegen/WP-810.md "Builtin aliases").
//!
//! A call of such a function is a call of builtin `B`, never of an
//! external function: it is lowered as clang 19 `EmitAArch64SVEBuiltinExpr`
//! for the families [`lowering`] recognizes (`svcnt*`, `svptrue*`,
//! `svwhilel*`, `svld1`, `svst1`, `svdup_n`, the predicated arithmetic and
//! the reductions), and is the hard error "code generation for SVE builtin
//! 'B' (called as 'f') is not supported yet" for every other one. The
//! calling function needs target feature `sve` or `sme` (clang
//! `checkTargetFeatures`, `err_builtin_needs_feature`).

use kiln_ast::{DeclId, ExprId};
use kiln_diag::{Diag, DiagId};
use kiln_llvm::{TypeId as LTy, intrinsics};
use kiln_source::Span;
use kiln_source::symbol::Symbol;

use crate::fir::V;
use crate::module::{Cg, R, err};

/// An SVE element type: `s8` ... `u64`, `f16`, `f32`, `f64`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct Elt {
    /// `b's'`, `b'u'` or `b'f'`.
    kind: u8,
    /// The width in bits.
    bits: u32,
}

impl Elt {
    fn parse(s: &str) -> Option<Elt> {
        let (kind, bits) = match s {
            "s8" => (b's', 8),
            "s16" => (b's', 16),
            "s32" => (b's', 32),
            "s64" => (b's', 64),
            "u8" => (b'u', 8),
            "u16" => (b'u', 16),
            "u32" => (b'u', 32),
            "u64" => (b'u', 64),
            "f16" => (b'f', 16),
            "f32" => (b'f', 32),
            "f64" => (b'f', 64),
            _ => return None,
        };
        Some(Elt { kind, bits })
    }

    const fn float(self) -> bool {
        self.kind == b'f'
    }

    /// The minimum lane count of its SVE vector (`128 / bits`).
    const fn lanes(self) -> u32 {
        128 / self.bits
    }
}

/// How a predicated operation treats inactive lanes.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Pred {
    /// `_m`: merge from the first operand.
    M,
    /// `_x`: don't care (the `.u` intrinsic).
    X,
    /// `_z`: zero (the first operand is selected against zero).
    Z,
}

/// A builtin [`lowering`] recognizes.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Op {
    /// `svcnt{b,h,w,d}[_pat]`: `llvm.aarch64.sve.cnt{b,h,w,d}(i32 pat)`.
    Cnt { suffix: char, pat: bool },
    /// `svptrue[_pat]_bN`: `llvm.aarch64.sve.ptrue.<P>(i32 pat)`.
    Ptrue { bits: u32, pat: bool },
    /// `svwhile{lt,le}_bN_T`: `llvm.aarch64.sve.while{lt,le,lo,ls}`.
    While { cmp: &'static str, bits: u32, ty: Elt },
    /// `svld1_T`: `llvm.aarch64.sve.ld1.<V>(pg, ptr)`.
    Ld1(Elt),
    /// `svst1_T`: `llvm.aarch64.sve.st1.<V>(v, pg, ptr)`.
    St1(Elt),
    /// `svdup_n_T`: `llvm.aarch64.sve.dup.x.<V>(x)`.
    Dup(Elt),
    /// A predicated operation `sv<op>[_n]_T_{m,x,z}` of `arity` vector
    /// operands after the predicate (`_n`: the last one a scalar, splat).
    Arith { intr: &'static str, ty: Elt, pred: Pred, n: bool, arity: usize },
    /// A reduction `sv<op>v_T`: `llvm.aarch64.sve.<intr>.<V>(pg, v)`.
    Reduce { intr: &'static str, ty: Elt },
}

/// The arithmetic intrinsic for `op` on `t`, with its vector arity.
fn arith(op: &str, t: Elt) -> Option<(&'static str, usize)> {
    let (s, u, f) = match op {
        "add" => ("add", "add", "fadd"),
        "sub" => ("sub", "sub", "fsub"),
        "mul" => ("mul", "mul", "fmul"),
        "div" if t.float() || t.bits >= 32 => ("sdiv", "udiv", "fdiv"),
        "max" => ("smax", "umax", "fmax"),
        "min" => ("smin", "umin", "fmin"),
        "and" if !t.float() => ("and", "and", ""),
        "orr" if !t.float() => ("orr", "orr", ""),
        "eor" if !t.float() => ("eor", "eor", ""),
        "mla" => ("mla", "mla", "fmla"),
        _ => return None,
    };
    let arity = if op == "mla" { 3 } else { 2 };
    Some((
        match t.kind {
            b's' => s,
            b'u' => u,
            _ => f,
        },
        arity,
    ))
}

/// The reduction intrinsic for `op` (`addv`, ...) on `t`.
fn reduce(op: &str, t: Elt) -> Option<&'static str> {
    let (s, u, f) = match op {
        "addv" => ("saddv", "uaddv", "faddv"),
        "maxv" => ("smaxv", "umaxv", "fmaxv"),
        "minv" => ("sminv", "uminv", "fminv"),
        "andv" if !t.float() => ("andv", "andv", ""),
        "orv" if !t.float() => ("orv", "orv", ""),
        "eorv" if !t.float() => ("eorv", "eorv", ""),
        _ => return None,
    };
    Some(match t.kind {
        b's' => s,
        b'u' => u,
        _ => f,
    })
}

/// The predicate width suffix `bN`.
fn pred_bits(s: &str) -> Option<u32> {
    match s {
        "b8" => Some(8),
        "b16" => Some(16),
        "b32" => Some(32),
        "b64" => Some(64),
        _ => None,
    }
}

/// The lowering of builtin `name` (`__builtin_sve_svadd_s32_m`), or `None`
/// when kiln does not lower it.
fn lowering(name: &str) -> Option<Op> {
    let n = name.strip_prefix("__builtin_sve_sv")?;
    let parts: Vec<&str> = n.split('_').collect();
    Some(match parts.as_slice() {
        [c] | [c, "pat"] if c.len() == 4 && c.starts_with("cnt") => {
            let suffix = c.chars().nth(3)?;
            if !matches!(suffix, 'b' | 'h' | 'w' | 'd') {
                return None;
            }
            Op::Cnt { suffix, pat: parts.len() == 2 }
        }
        ["ptrue", b] => Op::Ptrue { bits: pred_bits(b)?, pat: false },
        ["ptrue", "pat", b] => Op::Ptrue { bits: pred_bits(b)?, pat: true },
        [w @ ("whilelt" | "whilele"), b, t] => {
            let ty = Elt::parse(t)?;
            if ty.float() || ty.bits < 32 {
                return None;
            }
            let cmp = match (*w, ty.kind) {
                ("whilelt", b's') => "whilelt",
                ("whilelt", _) => "whilelo",
                (_, b's') => "whilele",
                _ => "whilels",
            };
            Op::While { cmp, bits: pred_bits(b)?, ty }
        }
        ["ld1", t] => Op::Ld1(Elt::parse(t)?),
        ["st1", t] => Op::St1(Elt::parse(t)?),
        ["dup", "n", t] => Op::Dup(Elt::parse(t)?),
        [op, rest @ ..] if rest.len() == 2 || (rest.len() == 3 && rest[0] == "n") => {
            let n = rest.len() == 3;
            let ty = Elt::parse(rest[rest.len() - 2])?;
            let pred = match rest[rest.len() - 1] {
                "m" => Pred::M,
                "x" => Pred::X,
                "z" => Pred::Z,
                _ => return None,
            };
            let (intr, arity) = arith(op, ty)?;
            Op::Arith { intr, ty, pred, n, arity }
        }
        [op, t] => {
            let ty = Elt::parse(t)?;
            Op::Reduce { intr: reduce(op, ty)?, ty }
        }
        _ => return None,
    })
}

impl Cg<'_> {
    /// The call `e` of `d`, an alias of builtin `b` (WP-810): lowered as
    /// clang, or a hard error naming the builtin. Never an external call.
    pub(crate) fn emit_sve_alias(&mut self, d: DeclId, b: Symbol, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        let name = ast.symbols().as_str(b).to_owned();
        let called = ast.decls.get(d).name().as_ident().map(|s| ast.symbols().as_str(s).to_owned());
        let called = called.unwrap_or_default();
        let on = self.feature_map(self.f.decl);
        if on.get("sve") != Some(&true) && on.get("sme") != Some(&true) {
            // clang `checkTargetFeatures`: the error names the function
            // called, then emission goes on with a poison value.
            let diag = Diag::new(DiagId::cg_builtin_needs_feature, Span::empty(ast.begin_loc(e)))
                .arg(called)
                .arg("sve|sme".to_owned());
            self.diags.push(diag);
            let rty = self.lty(ast.exprs.get(e).ty());
            return Ok((rty != LTy::VOID).then(|| V::C(self.m.const_poison(rty))));
        }
        let Some(op) = lowering(&name) else {
            return err(
                self.loc_of(e),
                format!(
                    "code generation for SVE builtin '{name}' (called as '{called}') is not \
                     supported yet"
                ),
            );
        };
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        self.emit_sve_op(op, &name, &args, e)
    }

    /// The `<vscale x n x t>` type.
    fn sve_vec(&mut self, t: Elt) -> LTy {
        let et = match (t.kind, t.bits) {
            (b'f', 16) => LTy::HALF,
            (b'f', 32) => LTy::FLOAT,
            (b'f', 64) => LTy::DOUBLE,
            (_, 8) => LTy::I8,
            (_, 16) => LTy::I16,
            (_, 32) => LTy::I32,
            _ => LTy::I64,
        };
        self.m.types.scalable_vector(et, t.lanes())
    }

    /// The predicate type for `bits`-wide lanes, `<vscale x 128/bits x i1>`.
    fn sve_pred(&mut self, bits: u32) -> LTy {
        self.m.types.scalable_vector(LTy::I1, 128 / bits)
    }

    /// The attributes of SVE intrinsic `base` (after `llvm.aarch64.sve.`):
    /// LLVM 22 makes the integer ones `speculatable`.
    fn sve_mem(&self, base: &str) -> intrinsics::TargetMem {
        if self.m.ir_version >= 22 && !base.starts_with('f') && base != "ptrue" {
            intrinsics::TargetMem::Speculatable
        } else {
            intrinsics::TargetMem::None
        }
    }

    /// `llvm.aarch64.sve.<base>.<overloads>(params) -> ret`, called.
    fn sve_call(
        &mut self,
        base: &str,
        overloads: &[LTy],
        ret: LTy,
        params: &[(LTy, bool)],
        args: Vec<V>,
    ) -> V {
        let full =
            intrinsics::overloaded_name(&self.m, &format!("llvm.aarch64.sve.{base}"), overloads);
        let mem = self.sve_mem(base);
        let f = intrinsics::target_intrinsic(&mut self.m, &full, ret, params, mem);
        self.intrinsic_call(f, args)
    }

    /// The `svbool_t` `pg` narrowed to `bits`-wide lanes (clang
    /// `EmitSVEPredicateCast`; nothing for 8-bit lanes).
    fn sve_from_svbool(&mut self, pg: V, bits: u32) -> V {
        if bits == 8 {
            return pg;
        }
        let pt = self.sve_pred(bits);
        let all = self.sve_pred(8);
        self.sve_call("convert.from.svbool", &[pt], pt, &[(all, false)], vec![pg])
    }

    /// The `bits`-lane predicate `p` widened to `svbool_t`.
    fn sve_to_svbool(&mut self, p: V, bits: u32) -> V {
        if bits == 8 {
            return p;
        }
        let pt = self.sve_pred(bits);
        let all = self.sve_pred(8);
        self.sve_call("convert.to.svbool", &[pt], all, &[(pt, false)], vec![p])
    }

    /// The immediate operand `a` (an `svpattern`; clang requires a
    /// constant expression, `EmitScalarOrConstFoldImmArg`).
    fn sve_imm(&mut self, a: ExprId, name: &str) -> R<V> {
        // An `svpattern` enumerator folds to a constant.
        let v = self.emit_scalar(a)?;
        match self.cint(v) {
            Some((_, k, w)) => {
                let k = crate::func::sext(k, w);
                Ok(V::C(self.m.const_int(LTy::I32, k)))
            }
            None => err(self.loc_of(a), format!("argument to {name} must be a constant integer")),
        }
    }

    /// The IR types `op`'s operands and result have in `arm_sve.h`
    /// (`None`: an immediate, `VOID`: no result).
    fn sve_signature(&mut self, op: Op) -> (Vec<Option<LTy>>, LTy) {
        let svbool = self.sve_pred(8);
        let ptr = self.m.types.ptr(0);
        let scalar = |cg: &mut Self, t: Elt| {
            let v = cg.sve_vec(t);
            cg.m.types.vector_info(v).map_or(LTy::I8, |x| x.0)
        };
        match op {
            Op::Cnt { pat, .. } => (if pat { vec![None] } else { vec![] }, LTy::I64),
            Op::Ptrue { pat, .. } => (if pat { vec![None] } else { vec![] }, svbool),
            Op::While { ty, .. } => {
                let st = scalar(self, ty);
                (vec![Some(st), Some(st)], svbool)
            }
            Op::Ld1(t) => (vec![Some(svbool), Some(ptr)], self.sve_vec(t)),
            Op::St1(t) => {
                let vt = self.sve_vec(t);
                (vec![Some(svbool), Some(ptr), Some(vt)], LTy::VOID)
            }
            Op::Dup(t) => (vec![Some(scalar(self, t))], self.sve_vec(t)),
            Op::Arith { ty, n, arity, .. } => {
                let vt = self.sve_vec(ty);
                let mut v = vec![Some(svbool)];
                v.extend(std::iter::repeat_n(Some(vt), arity));
                if n {
                    v[arity] = Some(scalar(self, ty));
                }
                (v, vt)
            }
            Op::Reduce { intr, ty } => {
                let vt = self.sve_vec(ty);
                let rt =
                    if intr.ends_with("addv") && !ty.float() { LTy::I64 } else { scalar(self, ty) };
                (vec![Some(svbool), Some(vt)], rt)
            }
        }
    }

    fn emit_sve_op(&mut self, op: Op, name: &str, args: &[ExprId], e: ExprId) -> R<Option<V>> {
        // `arm_sve.h` declares each alias with its builtin's signature; any
        // other declaration is an error (clang emits an invalid call).
        let (params, ret) = self.sve_signature(op);
        let ast = self.ast;
        let mut ok = args.len() == params.len() && self.lty(ast.exprs.get(e).ty()) == ret;
        for (&a, p) in args.iter().zip(&params) {
            let t = self.lty(ast.exprs.get(a).ty());
            ok &= p.map_or(self.m.types.is_int(t), |p| p == t);
        }
        if !ok {
            return err(
                self.loc_of(e),
                format!("the function called does not have the signature of SVE builtin '{name}'"),
            );
        }
        let i32imm = [(LTy::I32, true)];
        Ok(Some(match op {
            Op::Cnt { suffix, pat } => {
                let k = if pat {
                    self.sve_imm(args[0], name)?
                } else {
                    V::C(self.m.const_int(LTy::I32, 31))
                };
                self.sve_call(&format!("cnt{suffix}"), &[], LTy::I64, &i32imm, vec![k])
            }
            Op::Ptrue { bits, pat } => {
                let k = if pat {
                    self.sve_imm(args[0], name)?
                } else {
                    V::C(self.m.const_int(LTy::I32, 31))
                };
                let pt = self.sve_pred(bits);
                let p = self.sve_call("ptrue", &[pt], pt, &i32imm, vec![k]);
                self.sve_to_svbool(p, bits)
            }
            Op::While { cmp, bits, ty } => {
                let a = self.emit_scalar(args[0])?;
                let b = self.emit_scalar(args[1])?;
                let st = if ty.bits == 32 { LTy::I32 } else { LTy::I64 };
                let pt = self.sve_pred(bits);
                let p = self.sve_call(cmp, &[pt, st], pt, &[(st, false), (st, false)], vec![a, b]);
                self.sve_to_svbool(p, bits)
            }
            Op::Ld1(ty) => {
                let pg = self.emit_scalar(args[0])?;
                let p = self.emit_scalar(args[1])?;
                let pg = self.sve_from_svbool(pg, ty.bits);
                let vt = self.sve_vec(ty);
                let pt = self.sve_pred(ty.bits);
                let ptr = self.m.types.ptr(0);
                let full = intrinsics::overloaded_name(&self.m, "llvm.aarch64.sve.ld1", &[vt]);
                let f = intrinsics::target_intrinsic(
                    &mut self.m,
                    &full,
                    vt,
                    &[(pt, false), (ptr, false)],
                    intrinsics::TargetMem::ArgRead,
                );
                self.intrinsic_call(f, vec![pg, p])
            }
            Op::St1(ty) => {
                let pg = self.emit_scalar(args[0])?;
                let p = self.emit_scalar(args[1])?;
                let v = self.emit_scalar(args[2])?;
                let pg = self.sve_from_svbool(pg, ty.bits);
                let vt = self.sve_vec(ty);
                let pt = self.sve_pred(ty.bits);
                let full = intrinsics::overloaded_name(&self.m, "llvm.aarch64.sve.st1", &[vt]);
                let f = intrinsics::sve_store(&mut self.m, &full, vt, pt);
                self.intrinsic_call(f, vec![v, pg, p]);
                return Ok(None);
            }
            Op::Dup(ty) => {
                let x = self.emit_scalar(args[0])?;
                let vt = self.sve_vec(ty);
                let st = self.f.fir.ty(&self.m, x);
                self.sve_call("dup.x", &[vt], vt, &[(st, false)], vec![x])
            }
            Op::Arith { intr, ty, pred, n, arity } => {
                let mut vals = Vec::with_capacity(arity + 1);
                for &a in args {
                    vals.push(self.emit_scalar(a)?);
                }
                let vt = self.sve_vec(ty);
                let pt = self.sve_pred(ty.bits);
                vals[0] = self.sve_from_svbool(vals[0], ty.bits);
                if n {
                    let last = vals.len() - 1;
                    vals[last] = self.splat(vals[last], vt);
                }
                if pred == Pred::Z {
                    let zero = V::C(self.m.const_zero(vt));
                    vals[1] = self.select(vals[0], vals[1], zero);
                }
                let base = if pred == Pred::X { format!("{intr}.u") } else { intr.to_owned() };
                let mut params = vec![(pt, false)];
                params.extend(std::iter::repeat_n((vt, false), arity));
                self.sve_call(&base, &[vt], vt, &params, vals)
            }
            Op::Reduce { intr, ty } => {
                let pg = self.emit_scalar(args[0])?;
                let v = self.emit_scalar(args[1])?;
                let pg = self.sve_from_svbool(pg, ty.bits);
                let vt = self.sve_vec(ty);
                let pt = self.sve_pred(ty.bits);
                let rt = self.lty(self.ast.exprs.get(e).ty());
                self.sve_call(intr, &[vt], rt, &[(pt, false), (vt, false)], vec![pg, v])
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let s32 = Elt { kind: b's', bits: 32 };
        assert_eq!(lowering("__builtin_sve_svcntw"), Some(Op::Cnt { suffix: 'w', pat: false }));
        assert_eq!(lowering("__builtin_sve_svcntd_pat"), Some(Op::Cnt { suffix: 'd', pat: true }));
        assert_eq!(lowering("__builtin_sve_svcnt_s32_m"), None);
        assert_eq!(lowering("__builtin_sve_svcntp_b8"), None);
        assert_eq!(
            lowering("__builtin_sve_svptrue_pat_b16"),
            Some(Op::Ptrue { bits: 16, pat: true })
        );
        assert_eq!(
            lowering("__builtin_sve_svwhilele_b8_u64"),
            Some(Op::While { cmp: "whilels", bits: 8, ty: Elt { kind: b'u', bits: 64 } })
        );
        assert_eq!(lowering("__builtin_sve_svwhilelt_b8_s16"), None);
        assert_eq!(
            lowering("__builtin_sve_svadd_n_s32_z"),
            Some(Op::Arith { intr: "add", ty: s32, pred: Pred::Z, n: true, arity: 2 })
        );
        assert_eq!(
            lowering("__builtin_sve_svdiv_u32_x"),
            Some(Op::Arith {
                intr: "udiv",
                ty: Elt { kind: b'u', bits: 32 },
                pred: Pred::X,
                n: false,
                arity: 2
            })
        );
        assert_eq!(lowering("__builtin_sve_svdiv_s8_x"), None);
        assert_eq!(lowering("__builtin_sve_svand_f32_m"), None);
        assert_eq!(
            lowering("__builtin_sve_svaddv_s32"),
            Some(Op::Reduce { intr: "saddv", ty: s32 })
        );
        assert_eq!(lowering("__builtin_sve_svld1_bf16"), None);
        assert_eq!(lowering("__builtin_sve_svld1_s32_x2"), None);
        assert_eq!(lowering("__builtin_sve_svld1_vnum_s32"), None);
        assert_eq!(lowering("__builtin_sve_svdup_n_s32_m"), None);
        assert_eq!(lowering("__builtin_sve_svadd_s32"), None);
        assert_eq!(lowering("__builtin_sme_svzero_za"), None);
    }

    /// The share of `arm_sve.h`'s builtin aliases kiln lowers (WP-810
    /// reports the counts; every other one is a hard error when called).
    #[test]
    fn arm_sve_h_coverage() {
        let h = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../resource/include/arm_sve.h"
        ))
        .expect("arm_sve.h");
        let mut names: Vec<&str> = h
            .split("__clang_arm_builtin_alias(")
            .skip(1)
            .filter_map(|s| s.split(')').next())
            .collect();
        names.sort_unstable();
        names.dedup();
        let lowered = names.iter().filter(|n| lowering(n).is_some()).count();
        eprintln!("arm_sve.h builtins: {} total, {lowered} lowered", names.len());
        assert_eq!((names.len(), lowered), (7749, LOWERED));
    }

    /// The number of `arm_sve.h` builtins [`lowering`] covers.
    const LOWERED: usize = 720;
}
