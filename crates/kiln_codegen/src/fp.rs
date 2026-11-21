//! Floating-point semantics (spec/codegen.md "Floating-point semantics",
//! WP-806): the command line's [`FpOptions`] and the FP pragma scopes
//! give each instruction its fast-math flags and each function its
//! `*-fp-math` attributes (clang `CodeGenFunction::SetFastMathFlags`,
//! `CGFPOptionsRAII`, `getTrivialDefaultFunctionAttributes`).

use kiln_ast::StmtId;
use kiln_ast::fp::{FpContract, FpExcept, FpOptions, FpOverride};
use kiln_llvm::{Attr, AttrKind, FastMath, TypeId, Value};

use crate::fir::{Fir, Op, V};
use crate::module::Cg;

/// Whether the constrained form of FP intrinsic `base` (`llvm.<base>`,
/// LLVM `ConstrainedOps.def`) takes a rounding mode; `None`: it has none
/// kiln emits (the intrinsic stays as is, as clang's `fabs`, `copysign`,
/// `canonicalize`, `maximumnum` do).
pub(crate) fn constrained_rounds(base: &str) -> Option<bool> {
    Some(match base {
        "fma" | "fmuladd" | "nearbyint" | "rint" | "sqrt" | "pow" | "powi" | "sin" | "cos"
        | "exp" | "exp2" | "log" | "log10" | "log2" | "lrint" | "llrint" | "ldexp" => true,
        "ceil" | "floor" | "round" | "roundeven" | "trunc" | "maxnum" | "minnum" | "lround"
        | "llround" => false,
        _ => return None,
    })
}

/// The fast-math flags of `o` (clang `SetFastMathFlags`): `contract` only
/// for `fp_contract(fast)`, which `on` lowers as `llvm.fmuladd` instead.
pub(crate) fn fmf_of(o: FpOptions) -> FastMath {
    let mut f = FastMath::NONE;
    for (on, b) in [
        (o.reassoc, FastMath::REASSOC),
        (o.nnan, FastMath::NNAN),
        (o.ninf, FastMath::NINF),
        (o.nsz, FastMath::NSZ),
        (o.arcp, FastMath::ARCP),
        (o.afn, FastMath::AFN),
        (o.contract == FpContract::Fast, FastMath::CONTRACT),
    ] {
        if on {
            f = f.with(b);
        }
    }
    f
}

/// The `*-fp-math` function attributes a pragma scope can turn off
/// (clang `CGFPOptionsRAII::ConstructorHelper`'s `mergeFnAttrValue`),
/// with their value under `o`.
fn merged_attrs(o: FpOptions) -> [(&'static str, bool); 4] {
    [
        ("no-infs-fp-math", o.ninf),
        ("no-nans-fp-math", o.nnan),
        ("no-signed-zeros-fp-math", o.nsz),
        ("unsafe-fp-math", unsafe_fp_math(o)),
    ]
}

/// clang's `unsafe-fp-math`: every value-changing flag but the
/// finite-math ones, with contraction across statements.
fn unsafe_fp_math(o: FpOptions) -> bool {
    o.reassoc && o.arcp && o.nsz && o.afn && o.contract == FpContract::Fast
}

/// The FP state saved by [`Cg::enter_fp`].
pub(crate) type SavedFp = Option<(FpOptions, FpOverride)>;

impl Cg<'_> {
    /// Starts a function's FP state at the command line's (after its
    /// `Fir` is created).
    pub(crate) fn start_fp(&mut self) {
        let o = self.opts.fp;
        self.f.fp = o;
        self.f.fp_ovr = FpOverride::NONE;
        self.f.fp_attrs_off = 0;
        self.f.strict = false;
        self.f.fir.set_base_fmf(fmf_of(o));
    }

    /// Enters the FP scope of compound statement `s` (its recorded pragma
    /// override applied to the command line): the instructions emitted
    /// until [`Self::leave_fp`] get its fast-math flags, and a non-empty
    /// body merges its `*-fp-math` attributes into the function's.
    pub(crate) fn enter_fp(&mut self, s: StmtId) -> SavedFp {
        let ast = self.ast;
        let ovr = ast.stmts.compound_fp(s);
        if ovr == self.f.fp_ovr {
            return None;
        }
        let o = ovr.apply(self.opts.fp);
        let saved = (self.f.fp, self.f.fp_ovr);
        self.f.fp = o;
        self.f.fp_ovr = ovr;
        self.f.fir.set_fmf(fmf_of(o));
        if ast.lists.iter::<StmtId>(ast.stmts.compound_body(s)).next().is_some() {
            for (i, (_, v)) in merged_attrs(o).into_iter().enumerate() {
                if !v {
                    self.f.fp_attrs_off |= 1 << i;
                }
            }
        }
        Some(saved)
    }

    /// clang `IRBuilder::CreateConstrainedFPCall` (in a `strictfp`
    /// function, [`crate::func::Fx::strict`]): `llvm.experimental.
    /// constrained.<op>` overloaded on `overload`, on `args`, then the
    /// predicate `pred`, the scope's rounding mode when `round`, and its
    /// exception behavior. Never folded.
    pub(crate) fn constrained(
        &mut self,
        op: &str,
        ret: TypeId,
        overload: &[TypeId],
        mut args: Vec<V>,
        pred: Option<&str>,
        round: bool,
    ) -> V {
        let base = format!("llvm.experimental.constrained.{op}");
        let name = kiln_llvm::intrinsics::overloaded_name(&self.m, &base, overload);
        let tys: Vec<TypeId> = args.iter().map(|&a| self.f.fir.ty(&self.m, a)).collect();
        let mut mds = Vec::with_capacity(3);
        if let Some(p) = pred {
            mds.push(self.m.md_string(p.as_bytes()));
        }
        if round {
            let r = self.f.fp.rounding().llvm_name();
            mds.push(self.m.md_string(r.as_bytes()));
        }
        let x: &[u8] = match self.f.fp.exceptions() {
            FpExcept::Ignore => b"fpexcept.ignore",
            FpExcept::MayTrap => b"fpexcept.maytrap",
            FpExcept::Strict => b"fpexcept.strict",
        };
        mds.push(self.m.md_string(x));
        let f = kiln_llvm::intrinsics::constrained(&mut self.m, &name, ret, &tys, mds.len());
        args.extend(mds.into_iter().map(|m| V::C(Value::metadata(m))));
        let fn_ty = self.m.func(f).ty();
        let c = self.f.fir.push(
            Op::Call { fn_ty, callee: V::C(Value::func(f)), args, opts: Box::default() },
            ret,
        );
        self.f.calls.push((Fir::last_inst(c), Some(f), true));
        c
    }

    /// The constrained form of a call of FP intrinsic `base` on `args`
    /// in a `strictfp` function (clang `emitUnaryMaybeConstrainedFPBuiltin`
    /// and kin), if it has one.
    pub(crate) fn constrained_intrinsic(
        &mut self,
        base: &str,
        ret: TypeId,
        args: &[V],
    ) -> Option<V> {
        let round = constrained_rounds(base).filter(|_| self.f.strict)?;
        let arg = args.first().map_or(ret, |&a| self.f.fir.ty(&self.m, a));
        let over: &[TypeId] = if matches!(base, "lrint" | "llrint" | "lround" | "llround") {
            &[ret, arg]
        } else {
            &[ret]
        };
        let over = over.to_vec();
        Some(self.constrained(base, ret, &over, args.to_vec(), None, round))
    }

    /// clang `CodeGenFunction::StartFunction` of a `strictfp` function:
    /// every call in it is `strictfp` too, but for an implicit member's
    /// or an initialization function's `EmitCall` calls
    /// ([`crate::func::Fx::strict_calls`]).
    pub(crate) fn strict_call_sites(&mut self) {
        if !self.f.strict {
            return;
        }
        let mut skip = if self.f.strict_calls { Vec::new() } else { self.f.emit_calls.clone() };
        skip.sort_unstable();
        for i in 0..self.f.fir.inst_count() as u32 {
            if skip.binary_search(&i).is_ok() {
                continue;
            }
            let Some(o) = self.f.fir.call_opts_mut(i) else { continue };
            let mut a = self.m.attrs_of(o.attrs.func).to_vec();
            if !a.contains(&Attr::Flag(AttrKind::StrictFP)) {
                a.push(Attr::Flag(AttrKind::StrictFP));
                o.attrs.func = self.m.attr_set(&a);
            }
        }
    }

    /// Leaves the FP scope [`Self::enter_fp`] entered.
    pub(crate) fn leave_fp(&mut self, saved: SavedFp) {
        if let Some((o, ovr)) = saved {
            self.f.fp = o;
            self.f.fp_ovr = ovr;
            self.f.fir.set_fmf(fmf_of(o));
        }
    }

    /// The command line's FP function attributes, on every declaration
    /// and definition (clang `getTrivialDefaultFunctionAttributes`).
    pub(crate) fn fp_fn_attrs(&self, out: &mut Vec<Attr>) {
        let o = self.opts.fp;
        if let Some(d) = &self.opts.fp_denormal {
            out.push(Attr::string("denormal-fp-math", d));
        }
        if o.ninf {
            out.push(Attr::string("no-infs-fp-math", "true"));
        }
        if o.nnan {
            out.push(Attr::string("no-nans-fp-math", "true"));
        }
        if o.afn {
            out.push(Attr::string("approx-func-fp-math", "true"));
        }
        if unsafe_fp_math(o) {
            out.push(Attr::string("unsafe-fp-math", "true"));
        }
        // clang: only when the command line's exceptions are ignored
        // (WP-1798: `-ffp-exception-behavior=maytrap|strict` drop it).
        if o.exceptions() == FpExcept::Ignore {
            out.push(Attr::string("no-trapping-math", "true"));
        }
        if o.nsz {
            out.push(Attr::string("no-signed-zeros-fp-math", "true"));
        }
    }

    /// Rewrites the definition attributes `fa` of the function just
    /// emitted for the FP scopes it entered: an attribute a scope turned
    /// off is `"false"` (clang `mergeFnAttrValue`).
    pub(crate) fn fp_def_attrs(&self, fa: &mut Vec<Attr>) {
        if self.f.strict {
            fa.push(Attr::Flag(AttrKind::StrictFP));
        }
        let off = self.f.fp_attrs_off;
        if off == 0 {
            return;
        }
        for (i, (name, _)) in merged_attrs(self.opts.fp).into_iter().enumerate() {
            if off & (1 << i) == 0 {
                continue;
            }
            for a in fa.iter_mut() {
                if let Attr::Str(k, v) = a
                    && &**k == name
                    && &**v == "true"
                {
                    *v = "false".into();
                }
            }
        }
    }

    /// Whether `a * b + c` fuses into `llvm.fmuladd` in the current scope
    /// (`fp_contract(on)`, clang `tryEmitFMulAdd`).
    pub(crate) fn contract_on(&self) -> bool {
        self.f.fp.contract == FpContract::On
    }
}
