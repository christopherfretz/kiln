//! Floating-point semantics (spec/codegen.md "Floating-point semantics",
//! WP-806): the command line's [`FpOptions`] and the FP pragma scopes
//! give each instruction its fast-math flags and each function its
//! `*-fp-math` attributes (clang `CodeGenFunction::SetFastMathFlags`,
//! `CGFPOptionsRAII`, `getTrivialDefaultFunctionAttributes`).

use kiln_ast::StmtId;
use kiln_ast::fp::{FpContract, FpOptions, FpOverride};
use kiln_llvm::{Attr, FastMath};

use crate::module::Cg;

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
        out.push(Attr::string("no-trapping-math", "true"));
        if o.nsz {
            out.push(Attr::string("no-signed-zeros-fp-math", "true"));
        }
    }

    /// Rewrites the definition attributes `fa` of the function just
    /// emitted for the FP scopes it entered: an attribute a scope turned
    /// off is `"false"` (clang `mergeFnAttrValue`).
    pub(crate) fn fp_def_attrs(&self, fa: &mut [Attr]) {
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
