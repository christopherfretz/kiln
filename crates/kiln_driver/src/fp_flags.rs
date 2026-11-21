//! The floating-point flags (spec/driver.md "Floating-point flags",
//! WP-806): clang 19's `RenderFloatingPointOptions` state machine over
//! `-ffast-math`, its parts, `-ffp-model=`, `-ffp-contract=` and the
//! floating-point environment flags, in command-line order.
//!
//! The environment flags (`-f[no-]trapping-math`, `-f[no-]rounding-math`,
//! `-ffp-exception-behavior=`, `-ffp-model=strict`, `-ffp-eval-method=`)
//! set the translation unit's FP state as the pragmas set a scope's
//! (WP-1798, spec/codegen.md "Floating-point semantics");
//! `-fexcess-precision=16` stays an error.

use kiln_ast::fp::{FpContract, FpEval, FpExcept, FpOptions};
use kiln_codegen::ComplexRange;

use crate::args::ArgError;

/// The FP flags without a value.
const FLAGS: &[&str] = &[
    "-ffast-math",
    "-fno-fast-math",
    "-funsafe-math-optimizations",
    "-fno-unsafe-math-optimizations",
    "-ffinite-math-only",
    "-fno-finite-math-only",
    "-fhonor-infinities",
    "-fno-honor-infinities",
    "-fhonor-nans",
    "-fno-honor-nans",
    "-fapprox-func",
    "-fno-approx-func",
    "-fmath-errno",
    "-fno-math-errno",
    "-fassociative-math",
    "-fno-associative-math",
    "-freciprocal-math",
    "-fno-reciprocal-math",
    "-fsigned-zeros",
    "-fno-signed-zeros",
    "-ftrapping-math",
    "-fno-trapping-math",
    "-frounding-math",
    "-fno-rounding-math",
];

/// The FP flags with a value from a set (clang's "unsupported argument"
/// otherwise).
const VALUED: &[(&str, &[&str])] = &[
    ("-ffp-contract=", &["fast", "on", "off", "fast-honor-pragmas"]),
    ("-ffp-model=", &["fast", "precise", "strict", "aggressive"]),
    ("-ffp-exception-behavior=", &["ignore", "maytrap", "strict"]),
    ("-ffp-eval-method=", &["source", "double", "extended"]),
    ("-fexcess-precision=", &["fast", "standard", "16"]),
];

/// The denormal modes (`llvm::DenormalMode`'s spellings).
const DENORMAL: &[&str] = &["ieee", "preserve-sign", "positive-zero", "dynamic"];

/// The FP flags of a command line, in order (`-Ofast` included: it
/// implies `-ffast-math` only when it is the last optimization level).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FpFlags {
    seq: Vec<String>,
}

/// The floating-point semantics a command line selects.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent cc1 flags, as clang's driver tracks them"
)]
pub struct FpModel {
    /// The translation unit's FP options (clang's cc1 `-menable-no-infs`,
    /// `-mreassociate`, ..., `-ffp-contract=`).
    pub opts: FpOptions,
    /// `-ffp-contract=fast` exactly (not `fast-honor-pragmas`): the
    /// backend fuses across statements too (clang `AllowFPOpFusion`,
    /// `llc -fp-contract=fast`).
    pub backend_fuse: bool,
    /// clang's cc1 `-ffast-math`: `__FAST_MATH__`.
    pub fast_math: bool,
    /// clang's cc1 `-funsafe-math-optimizations`: `__ARM_FP_FAST`.
    pub unsafe_math: bool,
    /// `-ffp-eval-method=` was given (any value; clang
    /// `FEM_UnsetOnCommandLine` otherwise): `__FLT_EVAL_METHOD__` is
    /// [`FpOptions::eval`]'s, and `#pragma clang fp reassociate(on)` /
    /// `reciprocal(on)` are errors.
    pub eval_on_command_line: bool,
    /// The `-fdenormal-fp-math=` mode, `"output,input"` (`None`: IEEE).
    pub denormal: Option<String>,
    /// The `_Complex` multiply/divide algorithm (clang's cc1
    /// `-complex-range=`; none is full).
    pub complex_range: ComplexRange,
    /// The driver's warnings, in command-line order (clang emits them
    /// once per compile job).
    pub warnings: Vec<FpWarning>,
}

/// A warning of the FP state machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FpWarning {
    /// clang 22 `warn_drv_overriding_complex_range`
    /// (`-Woverriding-complex-range`): the flag that implied the old
    /// range, the overriding flag, and the old and new range names
    /// (`full`, `improved`, `promoted`, `basic`, `none`).
    ComplexRange { implied_by: String, flag: String, old: &'static str, new: &'static str },
    /// `warn_drv_overriding_option` (`-Woverriding-option`): the
    /// overridden option, the one that overrides it.
    Option(String, String),
}

impl FpFlags {
    /// Records `arg` if it is an FP flag: `None` if not, else its value
    /// error.
    pub fn parse_arg(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        if FLAGS.contains(&arg) {
            self.seq.push(arg.to_owned());
            return Some(Ok(()));
        }
        if let Some(v) = arg.strip_prefix("-fdenormal-fp-math=") {
            let ok = v.split_once(',').map_or(DENORMAL.contains(&v), |(a, b)| {
                DENORMAL.contains(&a) && DENORMAL.contains(&b)
            });
            if !ok {
                return Some(Err(ArgError::InvalidFlagValue(arg.to_owned(), v.to_owned())));
            }
            self.seq.push(arg.to_owned());
            return Some(Ok(()));
        }
        let &(opt, vals) = VALUED.iter().find(|(o, _)| arg.starts_with(o))?;
        let v = &arg[opt.len()..];
        if !vals.contains(&v) {
            return Some(Err(ArgError::UnsupportedValue(opt.to_owned(), v.to_owned())));
        }
        self.seq.push(arg.to_owned());
        Some(Ok(()))
    }

    /// Records `-Ofast` (a `-O` flag: `-ffast-math` when it is the last).
    pub fn note_ofast(&mut self) {
        self.seq.push("-Ofast".to_owned());
    }

    /// Records a valid complex-range flag (`-f[no-]cx-limited-range`,
    /// `-f[no-]cx-fortran-rules`, `-fcomplex-arithmetic=full|improved|basic`):
    /// clang orders them with the FP flags.
    pub fn note_complex(&mut self, arg: &str) {
        self.seq.push(arg.to_owned());
    }

    /// The flag among `-ffast-math`, `-funsafe-math-optimizations` and
    /// `-ffp-model=fast` that makes the link add `crtfastmath.o` (clang
    /// `isFastMathRuntimeAvailable`: the last of them and their negations,
    /// unless `-Ofast` is the level, which `clang++` sees itself).
    #[must_use]
    pub fn fast_math_runtime_flag(&self) -> Option<&str> {
        let last = self.seq.iter().rev().find(|a| {
            matches!(
                a.as_str(),
                "-ffast-math"
                    | "-fno-fast-math"
                    | "-funsafe-math-optimizations"
                    | "-fno-unsafe-math-optimizations"
            ) || a.starts_with("-ffp-model=")
        })?;
        matches!(
            last.as_str(),
            "-ffast-math"
                | "-funsafe-math-optimizations"
                | "-ffp-model=fast"
                | "-ffp-model=aggressive"
        )
        .then_some(last.as_str())
    }

    /// Runs the state machine (clang `RenderFloatingPointOptions`):
    /// `ofast` says `-Ofast` is the optimization level, `clang22` selects
    /// clang 22's rules over clang 19's (`-fclang-compat=`). Flags whose
    /// final effect needs the FP environment are reported to `errors`.
    #[must_use]
    pub fn resolve(&self, ofast: bool, clang22: bool, errors: &mut Vec<ArgError>) -> FpModel {
        let mut s = State::new(clang22, ofast);
        for a in &self.seq {
            s.apply(a);
        }
        s.finish(errors)
    }
}

/// clang's local variables in `RenderFloatingPointOptions`.
#[allow(clippy::struct_excessive_bools, reason = "clang's flags")]
struct State {
    honor_infs: bool,
    honor_nans: bool,
    approx_func: bool,
    math_errno: bool,
    assoc: bool,
    recip: bool,
    signed_zeros: bool,
    /// clang's cc1 `-ffp-exception-behavior=` (`None`: not given;
    /// `-f[no-]trapping-math`, `-ffp-model=strict` set it too).
    except: Option<FpExcept>,
    /// `-frounding-math` (`-ffp-model=strict` too).
    rounding: bool,
    /// `-ffp-eval-method=` (`None`: not given).
    eval: Option<FpEval>,
    /// The flag that selected `_Float16` without excess precision.
    excess: Option<String>,
    contract: &'static str,
    last_seen_contract: Option<&'static str>,
    seen_unsafe: bool,
    /// The last `-fdenormal-fp-math=` (`None`: IEEE). No other flag
    /// changes it, and no target has another default: clang 19 and 22
    /// emit no `denormal-fp-math` for x86-64 Linux even when the link adds
    /// `crtfastmath.o`.
    denormal: Option<String>,
    /// clang 22's rules (`-ffp-model=fast` keeps NaNs and infinities and
    /// promotes complex division; `aggressive` is the old `fast`).
    clang22: bool,
    /// The complex range (`None`: clang's `CX_None`, full).
    range: Option<ComplexRange>,
    /// clang 22's `LastComplexRangeOption`: the flag that set the range
    /// (empty after `-fno-fast-math`).
    range_flag: String,
    /// clang 19's `GccRangeComplexOption`: the last `-f[no-]cx-*` flag.
    gcc_range_flag: String,
    /// The last `-ffp-model=` value that took effect (clang `FPModel`).
    fp_model: Option<String>,
    /// `-Ofast` is the optimization level (clang `OFastEnabled`).
    ofast: bool,
    warnings: Vec<FpWarning>,
    /// `-ffp-model=aggressive` under clang 19 (an unsupported value).
    aggressive19: bool,
}

impl State {
    fn new(clang22: bool, ofast: bool) -> Self {
        State {
            honor_infs: true,
            honor_nans: true,
            approx_func: false,
            // The Linux toolchains' default (clang `IsMathErrnoDefault`).
            math_errno: true,
            assoc: false,
            recip: false,
            signed_zeros: true,
            except: None,
            rounding: false,
            eval: None,
            excess: None,
            contract: "on",
            last_seen_contract: None,
            seen_unsafe: false,
            denormal: None,
            clang22,
            range: None,
            range_flag: String::new(),
            gcc_range_flag: String::new(),
            fp_model: None,
            ofast,
            warnings: Vec::new(),
            aggressive19: false,
        }
    }

    /// clang 22 `tools::setComplexRange`: a change of a set range warns
    /// unless [`overrides_silently`]. clang 19 sets the range without
    /// it ([`Self::set_range_19`] for its own warnings).
    fn set_range(&mut self, r: Option<ComplexRange>, flag: &str) {
        if self.clang22
            && let Some(old) = self.range
            && Some(old) != r
            && !overrides_silently(&self.range_flag, flag)
        {
            self.warnings.push(FpWarning::ComplexRange {
                implied_by: self.range_flag.clone(),
                flag: flag.to_owned(),
                old: old.name(),
                new: r.map_or("none", ComplexRange::name),
            });
        }
        flag.clone_into(&mut self.range_flag);
        self.range = r;
    }

    /// clang 19 `EmitComplexRangeDiag`: `overriding '<a>' option with
    /// '<b>'` when both are set and differ.
    fn range_diag_19(&mut self, a: String, b: &str) {
        if !a.is_empty() && !b.is_empty() && a != b {
            self.warnings.push(FpWarning::Option(a, b.to_owned()));
        }
    }

    /// clang 19's complex-range flags with their `-Woverriding-option`
    /// warnings, as its `RenderFloatingPointOptions` spells them
    /// (`-complex-range=<r>`, `-fcomplex-arithmetic=<r>`, the last
    /// `-f[no-]cx-*` flag).
    fn set_range_19(&mut self, a: &str) {
        let render = |r: Option<ComplexRange>| {
            r.map_or(String::new(), |r| format!("-complex-range={}", r.name()))
        };
        let arith = |r: ComplexRange| format!("-fcomplex-arithmetic={}", r.name());
        let gcc = self.gcc_range_flag.clone();
        let (new, set_gcc) = match a {
            "-fcx-limited-range" => {
                if gcc.is_empty() {
                    if self.range != Some(ComplexRange::Basic) {
                        self.range_diag_19(render(self.range), a);
                    }
                } else if gcc != "-fno-cx-limited-range" {
                    self.range_diag_19(gcc, a);
                }
                (ComplexRange::Basic, true)
            }
            "-fno-cx-limited-range" => {
                if gcc.is_empty() {
                    self.range_diag_19(render(self.range), a);
                } else if gcc != "-fcx-limited-range" && gcc != "-fno-cx-fortran-rules" {
                    self.range_diag_19(gcc, a);
                }
                (ComplexRange::Full, true)
            }
            "-fcx-fortran-rules" => {
                let old = if gcc.is_empty() { render(self.range) } else { gcc };
                self.range_diag_19(old, a);
                (ComplexRange::Improved, true)
            }
            "-fno-cx-fortran-rules" => {
                if gcc.is_empty() {
                    self.range_diag_19(render(self.range), a);
                } else if gcc != "-fno-cx-limited-range" {
                    self.range_diag_19(gcc, a);
                }
                (ComplexRange::Full, true)
            }
            _ => {
                let Some(r) =
                    a.strip_prefix("-fcomplex-arithmetic=").and_then(ComplexRange::from_arg)
                else {
                    return;
                };
                let warn = match gcc.as_str() {
                    "" => false,
                    "-fcx-limited-range" => r != ComplexRange::Basic,
                    "-fcx-fortran-rules" => true,
                    _ => r != ComplexRange::Improved,
                };
                if warn {
                    self.range_diag_19(gcc, &arith(r));
                }
                (r, false)
            }
        };
        if set_gcc {
            a.clone_into(&mut self.gcc_range_flag);
        }
        self.range = Some(new);
    }

    /// clang's `applyFastMath`: `aggressive` (`-ffast-math`, `-Ofast`,
    /// clang 22's `-ffp-model=aggressive` and clang 19's `fast`) drops
    /// NaNs and infinities and selects the basic complex range; clang
    /// 22's `-ffp-model=fast` keeps them and selects the promoted range.
    fn fast_math(&mut self, aggressive: bool, flag: &str) {
        let range = if aggressive { ComplexRange::Basic } else { ComplexRange::Promoted };
        if self.clang22 {
            self.set_range(Some(range), flag);
        } else {
            // clang 19 warns when a full, improved or promoted range
            // gives way.
            if let Some(
                old @ (ComplexRange::Full | ComplexRange::Improved | ComplexRange::Promoted),
            ) = self.range
            {
                let new = if self.gcc_range_flag.is_empty() {
                    "-fcomplex-arithmetic=basic".to_owned()
                } else {
                    self.gcc_range_flag.clone()
                };
                self.range_diag_19(format!("-fcomplex-arithmetic={}", old.name()), &new);
            }
            self.range = Some(ComplexRange::Basic);
        }
        self.honor_infs = !aggressive;
        self.honor_nans = !aggressive;
        self.math_errno = false;
        self.assoc = true;
        self.recip = true;
        self.approx_func = true;
        self.signed_zeros = false;
        self.except = None;
        self.rounding = false;
        self.contract = "fast";
        self.seen_unsafe = true;
    }

    /// `-fno-fast-math` / `-fno-unsafe-math-optimizations` restore the
    /// last `-ffp-contract=`, else `on` after a fast-math flag.
    fn restore_contract(&mut self) {
        if let Some(c) = self.last_seen_contract {
            self.contract = c;
        } else if self.seen_unsafe {
            self.contract = "on";
        }
    }

    fn apply(&mut self, a: &str) {
        match a {
            "-Ofast" if self.ofast => self.fast_math(true, a),
            "-Ofast" => {}
            "-ffast-math" => self.fast_math(true, a),
            "-fno-fast-math" => {
                // clang 22 drops the complex range too (a warning unless
                // it was full); clang 19 keeps it.
                if self.clang22 {
                    if self.range == Some(ComplexRange::Full) {
                        self.range = None;
                    } else {
                        self.set_range(None, a);
                    }
                    self.range_flag.clear();
                }
                self.honor_infs = true;
                self.honor_nans = true;
                self.math_errno = true;
                self.assoc = false;
                self.recip = false;
                self.approx_func = false;
                self.signed_zeros = true;
                self.restore_contract();
            }
            "-funsafe-math-optimizations" => {
                self.assoc = true;
                self.recip = true;
                self.signed_zeros = false;
                self.approx_func = true;
                self.except = None;
                self.contract = "fast";
                self.seen_unsafe = true;
            }
            "-fno-unsafe-math-optimizations" => {
                self.assoc = false;
                self.recip = false;
                self.signed_zeros = true;
                self.approx_func = false;
                self.restore_contract();
            }
            "-ffinite-math-only" => (self.honor_infs, self.honor_nans) = (false, false),
            "-fno-finite-math-only" => (self.honor_infs, self.honor_nans) = (true, true),
            "-fhonor-infinities" => self.honor_infs = true,
            "-fno-honor-infinities" => self.honor_infs = false,
            "-fhonor-nans" => self.honor_nans = true,
            "-fno-honor-nans" => self.honor_nans = false,
            "-fapprox-func" => self.approx_func = true,
            "-fno-approx-func" => self.approx_func = false,
            "-fmath-errno" => self.math_errno = true,
            "-fno-math-errno" => self.math_errno = false,
            "-fassociative-math" => self.assoc = true,
            "-fno-associative-math" => self.assoc = false,
            "-freciprocal-math" => self.recip = true,
            "-fno-reciprocal-math" => self.recip = false,
            "-fsigned-zeros" => self.signed_zeros = true,
            "-fno-signed-zeros" => self.signed_zeros = false,
            "-ftrapping-math" => self.except = Some(FpExcept::Strict),
            "-fno-trapping-math" => self.except = Some(FpExcept::Ignore),
            "-frounding-math" => self.rounding = true,
            "-fno-rounding-math" => self.rounding = false,
            "-fcx-limited-range"
            | "-fcx-fortran-rules"
            | "-fno-cx-limited-range"
            | "-fno-cx-fortran-rules"
                if !self.clang22 =>
            {
                self.set_range_19(a);
            }
            "-fcx-limited-range" => self.set_range(Some(ComplexRange::Basic), a),
            "-fcx-fortran-rules" => self.set_range(Some(ComplexRange::Improved), a),
            "-fno-cx-limited-range" | "-fno-cx-fortran-rules" => {
                self.set_range(Some(ComplexRange::Full), a);
            }
            _ => self.apply_valued(a),
        }
    }

    fn apply_valued(&mut self, a: &str) {
        let owned = || Some(a.to_owned());
        if let Some(v) = a.strip_prefix("-ffp-contract=") {
            let c = VALUED[0].1.iter().find(|&&x| x == v).copied().unwrap_or("on");
            self.contract = c;
            self.last_seen_contract = Some(c);
        } else if let Some(v) = a.strip_prefix("-ffp-model=") {
            // Every model starts from the defaults (rounding and
            // trapping stay; clang 22 keeps the contraction too).
            self.honor_infs = true;
            self.honor_nans = true;
            self.approx_func = false;
            self.math_errno = true;
            self.assoc = false;
            self.recip = false;
            self.signed_zeros = true;
            if !self.clang22 {
                self.contract = "on";
            }
            // Only clang 22's `aggressive` (clang 19's `fast`) agrees
            // with `-Ofast`; another model is ignored after the reset.
            if self.ofast && v != if self.clang22 { "aggressive" } else { "fast" } {
                self.warnings.push(FpWarning::Option(a.to_owned(), "-Ofast".to_owned()));
                return;
            }
            if let Some(m) = self.fp_model.as_deref()
                && m != v
            {
                let old = format!("-ffp-model={m}");
                self.warnings.push(FpWarning::Option(old, a.to_owned()));
            }
            match v {
                "fast" => self.fast_math(!self.clang22, a),
                "aggressive" if self.clang22 => self.fast_math(true, a),
                "aggressive" => {
                    self.aggressive19 = true;
                    return;
                }
                "strict" => {
                    self.except = Some(FpExcept::Strict);
                    self.rounding = true;
                    self.contract = "off";
                }
                _ => self.contract = "on",
            }
            self.fp_model = Some(v.to_owned());
            if self.clang22 && matches!(v, "precise" | "strict") {
                self.set_range(Some(ComplexRange::Full), a);
            }
        } else if let Some(v) = a.strip_prefix("-ffp-exception-behavior=") {
            self.except = Some(match v {
                "maytrap" => FpExcept::MayTrap,
                "strict" => FpExcept::Strict,
                _ => FpExcept::Ignore,
            });
        } else if let Some(v) = a.strip_prefix("-ffp-eval-method=") {
            self.eval = Some(match v {
                "double" => FpEval::Double,
                "extended" => FpEval::Extended,
                _ => FpEval::Source,
            });
        } else if let Some(v) = a.strip_prefix("-fexcess-precision=") {
            self.excess = (v == "16").then(owned).flatten();
        } else if let Some(v) = a.strip_prefix("-fcomplex-arithmetic=") {
            if !self.clang22 {
                self.set_range_19(a);
            } else if let Some(r) = ComplexRange::from_arg(v) {
                self.set_range(Some(r), a);
            }
        } else if let Some(v) = a.strip_prefix("-fdenormal-fp-math=") {
            let v = if v.contains(',') { v.to_owned() } else { format!("{v},{v}") };
            self.denormal = (v != "ieee,ieee").then_some(v);
        }
    }

    fn finish(self, errors: &mut Vec<ArgError>) -> FpModel {
        if let Some(f) = &self.excess {
            errors.push(ArgError::UnsupportedByKiln(f.clone()));
        }
        if self.aggressive19 {
            errors.push(ArgError::UnsupportedValue("-ffp-model=".into(), "aggressive".into()));
        }
        let complex_range = self.range.unwrap_or(ComplexRange::Full);
        // Trapping math (clang's `TrappingMath`: strict exceptions) keeps
        // reassociation and so the unsafe and fast math off.
        let trapping = self.except == Some(FpExcept::Strict);
        let reassoc = self.assoc && !self.signed_zeros && !trapping;
        let unsafe_math = reassoc && self.recip && self.approx_func;
        let fast_math = !self.honor_infs && !self.honor_nans && !self.math_errno && unsafe_math;
        if self.eval.is_some() {
            // clang `FixupInvocation`: an evaluation method with a
            // value-changing option, one error per option.
            let opts = [(self.approx_func, 0), (reassoc, 1), (self.recip, 2)];
            for (_, i) in opts.into_iter().filter(|o| o.0) {
                errors.push(ArgError::IncompatibleFpEvalMethod(i));
            }
        }
        FpModel {
            opts: FpOptions {
                contract: match self.contract {
                    "off" => FpContract::Off,
                    "on" => FpContract::On,
                    _ => FpContract::Fast,
                },
                reassoc,
                arcp: self.recip,
                nnan: !self.honor_nans,
                ninf: !self.honor_infs,
                nsz: !self.signed_zeros,
                afn: self.approx_func,
                math_errno: self.math_errno,
                except: self.except,
                rounding_math: self.rounding,
                eval: self.eval.unwrap_or_default(),
                ..FpOptions::default()
            },
            backend_fuse: self.contract == "fast",
            fast_math,
            unsafe_math,
            eval_on_command_line: self.eval.is_some(),
            denormal: self.denormal,
            complex_range,
            warnings: self.warnings,
        }
    }
}

/// clang 22 `emitComplexRangeDiag`'s silent pairs: the same flag, a
/// flag and its negation, `-ffast-math` then `-fno-fast-math`, a
/// `-ffp-model=` then `-ffast-math`, `-fno-fast-math` or another model,
/// and one `-fcomplex-arithmetic=` then another.
fn overrides_silently(last: &str, new: &str) -> bool {
    let pairs = [
        ("-fcx-limited-range", "-fno-cx-limited-range"),
        ("-fno-cx-limited-range", "-fcx-limited-range"),
        ("-fcx-fortran-rules", "-fno-cx-fortran-rules"),
        ("-fno-cx-fortran-rules", "-fcx-fortran-rules"),
        ("-ffast-math", "-fno-fast-math"),
    ];
    let model = last.starts_with("-ffp-model=");
    last == new
        || last.is_empty()
        || new.is_empty()
        || pairs.contains(&(last, new))
        || (model && (new == "-ffast-math" || new == "-fno-fast-math"))
        || (model && new.starts_with("-ffp-model="))
        || (last.starts_with("-fcomplex-arithmetic=") && new.starts_with("-fcomplex-arithmetic="))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(args: &[&str]) -> (FpModel, Vec<ArgError>) {
        model_at(false, args)
    }

    fn model_at(clang22: bool, args: &[&str]) -> (FpModel, Vec<ArgError>) {
        let mut f = FpFlags::default();
        let mut errors = Vec::new();
        for a in args {
            if *a == "-Ofast" {
                f.note_ofast();
            } else if a.contains("cx-") || a.starts_with("-fcomplex-arithmetic=") {
                f.note_complex(a);
            } else if let Some(Err(e)) = f.parse_arg(a) {
                errors.push(e);
            }
        }
        let ofast = args.iter().rev().find(|a| a.starts_with("-O")) == Some(&"-Ofast");
        let m = f.resolve(ofast, clang22, &mut errors);
        (m, errors)
    }

    #[test]
    fn clang22_fp_models() {
        // clang 22: `fast` keeps NaNs and infinities (no `__FAST_MATH__`)
        // and promotes complex division; `aggressive` is the old `fast`.
        let (m, e) = model_at(true, &["-ffp-model=fast"]);
        assert!(!m.fast_math && m.unsafe_math && !m.opts.nnan && !m.opts.ninf);
        assert!(m.opts.reassoc && !m.opts.math_errno && m.opts.contract == FpContract::Fast);
        assert!(e.is_empty() && m.warnings.is_empty());
        assert_eq!(m.complex_range, ComplexRange::Promoted);
        let (m, e) = model_at(true, &["-ffp-model=fast", "-fcx-fortran-rules"]);
        assert!(e.is_empty() && m.complex_range == ComplexRange::Improved);
        let (m, e) = model_at(true, &["-ffp-model=aggressive"]);
        assert!(
            e.is_empty() && m.fast_math && m.opts.nnan && m.complex_range == ComplexRange::Basic
        );
        let (m, e) = model_at(true, &["-ffp-model=fast", "-ffp-model=precise"]);
        assert!(e.is_empty() && m.opts == FpOptions::default());
        assert_eq!(m.complex_range, ComplexRange::Full);
        let (m, _) = model_at(true, &["-fcx-limited-range", "-fno-fast-math"]);
        assert_eq!(m.complex_range, ComplexRange::Full);
        // clang 19: `fast` is -ffast-math, `aggressive` an unknown value,
        // and -fno-fast-math keeps the range.
        let (m, e) = model(&["-ffp-model=fast"]);
        assert!(e.is_empty() && m.fast_math && m.complex_range == ComplexRange::Basic);
        let (_, e) = model(&["-ffp-model=aggressive"]);
        assert_eq!(e, [ArgError::UnsupportedValue("-ffp-model=".into(), "aggressive".into())]);
        let (m, _) = model(&["-fcx-limited-range", "-fno-fast-math"]);
        assert_eq!(m.complex_range, ComplexRange::Basic);
        let (m, _) = model(&["-fcomplex-arithmetic=promoted", "-fno-fast-math"]);
        assert_eq!(m.complex_range, ComplexRange::Promoted);
        let (m, _) = model(&["-fcomplex-arithmetic=promoted", "-ffp-model=strict"]);
        assert_eq!(m.complex_range, ComplexRange::Promoted);
    }

    /// The `-complex-range=` clang 22 and 19 pass to `-cc1` (`-###`), and
    /// clang 22's `-Woverriding-complex-range` warnings.
    #[test]
    #[allow(clippy::too_many_lines, reason = "one table of command lines")]
    fn complex_range_interactions() {
        use ComplexRange::{Basic, Full, Improved, Promoted};
        let cr = |w: (&str, &str, &str, &str)| FpWarning::ComplexRange {
            implied_by: w.0.into(),
            flag: w.1.into(),
            old: match w.2 {
                "full" => "full",
                "basic" => "basic",
                "improved" => "improved",
                _ => "promoted",
            },
            new: match w.3 {
                "full" => "full",
                "basic" => "basic",
                "improved" => "improved",
                "none" => "none",
                _ => "promoted",
            },
        };
        let at = |c22: bool, a: &[&str]| {
            let (m, e) = model_at(c22, a);
            assert!(e.is_empty(), "{a:?}: {e:?}");
            (m.complex_range, m.warnings)
        };
        for v in ["full", "improved", "promoted", "basic"] {
            let flag = format!("-fcomplex-arithmetic={v}");
            assert_eq!(at(true, &[&flag]).0.name(), v);
            assert_eq!(at(false, &[&flag]).0.name(), v);
        }
        // Silent pairs.
        assert_eq!(
            at(true, &["-fcomplex-arithmetic=basic", "-fcomplex-arithmetic=promoted"]),
            (Promoted, vec![])
        );
        assert_eq!(at(true, &["-fcx-limited-range", "-fno-cx-limited-range"]), (Full, vec![]));
        assert_eq!(at(true, &["-ffast-math", "-fno-fast-math"]), (Full, vec![]));
        assert_eq!(at(true, &["-ffp-model=fast", "-ffast-math"]), (Basic, vec![]));
        assert_eq!(at(true, &["-ffp-model=fast", "-fno-fast-math"]), (Full, vec![]));
        assert_eq!(at(true, &["-ffp-model=strict", "-fno-fast-math"]), (Full, vec![]));
        assert_eq!(
            at(true, &["-ffp-model=fast", "-ffp-model=precise"]),
            (Full, vec![FpWarning::Option("-ffp-model=fast".into(), "-ffp-model=precise".into())])
        );
        // Warnings (clang 22 22.1.8 text).
        assert_eq!(
            at(true, &["-ffp-model=fast", "-fcx-limited-range"]),
            (Basic, vec![cr(("-ffp-model=fast", "-fcx-limited-range", "promoted", "basic"))])
        );
        assert_eq!(
            at(true, &["-fcomplex-arithmetic=promoted", "-fno-fast-math"]),
            (
                Full,
                vec![cr(("-fcomplex-arithmetic=promoted", "-fno-fast-math", "promoted", "none"))]
            )
        );
        assert_eq!(
            at(true, &["-fcomplex-arithmetic=basic", "-ffp-model=fast"]),
            (
                Promoted,
                vec![cr(("-fcomplex-arithmetic=basic", "-ffp-model=fast", "basic", "promoted"))]
            )
        );
        assert_eq!(
            at(true, &["-fcx-fortran-rules", "-ffast-math"]),
            (Basic, vec![cr(("-fcx-fortran-rules", "-ffast-math", "improved", "basic"))])
        );
        assert_eq!(
            at(true, &["-fcomplex-arithmetic=promoted", "-ffp-model=precise"]),
            (
                Full,
                vec![cr((
                    "-fcomplex-arithmetic=promoted",
                    "-ffp-model=precise",
                    "promoted",
                    "full"
                ))]
            )
        );
        // `-fno-fast-math` clears the flag: no warning after it.
        assert_eq!(
            at(true, &["-fcx-limited-range", "-fno-fast-math", "-fcx-fortran-rules"]).0,
            Improved
        );
        // -Ofast: another model is ignored after its reset.
        let (m, e) = model_at(true, &["-Ofast", "-ffp-model=precise"]);
        assert!(e.is_empty() && m.complex_range == Basic && m.opts.math_errno);
        assert_eq!(m.opts.contract, FpContract::Fast);
        assert_eq!(m.warnings, [FpWarning::Option("-ffp-model=precise".into(), "-Ofast".into())]);
        let (m, _) = model_at(true, &["-Ofast", "-ffp-model=fast"]);
        assert!(m.complex_range == Basic && m.warnings.len() == 1);
        let (m, _) = model_at(true, &["-Ofast", "-ffp-model=aggressive"]);
        assert!(m.complex_range == Basic && m.warnings.is_empty());
        let (m, _) = model(&["-Ofast", "-ffp-model=fast"]);
        assert!(m.complex_range == Basic && m.warnings.is_empty());
        let (m, e) = model(&["-Ofast", "-ffp-model=strict"]);
        assert!(e.is_empty() && m.complex_range == Basic && m.opts.contract == FpContract::On);
        assert_eq!(m.warnings, [FpWarning::Option("-ffp-model=strict".into(), "-Ofast".into())]);
        // clang 19 warns its own way about the complex flags, never with
        // -Woverriding-complex-range.
        let opt = |a: &str, b: &str| FpWarning::Option(a.into(), b.into());
        assert_eq!(
            at(false, &["-ffp-model=fast", "-fcx-fortran-rules"]),
            (Improved, vec![opt("-complex-range=basic", "-fcx-fortran-rules")])
        );
        assert_eq!(
            at(false, &["-fcomplex-arithmetic=promoted", "-ffast-math"]),
            (Basic, vec![opt("-fcomplex-arithmetic=promoted", "-fcomplex-arithmetic=basic")])
        );
        assert_eq!(
            at(false, &["-fcx-limited-range", "-fcomplex-arithmetic=improved"]),
            (Improved, vec![opt("-fcx-limited-range", "-fcomplex-arithmetic=improved")])
        );
        assert_eq!(at(false, &["-fcx-limited-range", "-fno-cx-limited-range"]), (Full, vec![]));
    }

    #[test]
    fn fast_math_and_parts() {
        let (m, e) = model(&["-ffast-math"]);
        assert!(e.is_empty() && m.fast_math && m.unsafe_math && m.backend_fuse);
        assert!(m.opts.reassoc && m.opts.nnan && m.opts.ninf && !m.opts.math_errno);
        let (m, _) = model(&["-ffast-math", "-fmath-errno"]);
        assert!(!m.fast_math && m.unsafe_math);
        let (m, _) = model(&["-ffast-math", "-fno-reciprocal-math"]);
        assert!(!m.unsafe_math && m.opts.reassoc && !m.opts.arcp);
        let (m, _) = model(&["-fassociative-math"]);
        assert!(!m.opts.reassoc);
        let (m, _) = model(&["-fassociative-math", "-fno-signed-zeros"]);
        assert!(m.opts.reassoc && m.opts.contract == FpContract::On);
        let (m, _) = model(&["-ffast-math", "-fno-fast-math"]);
        assert_eq!(m.opts, FpOptions::default());
        let (m, _) = model(&["-ffp-contract=fast", "-ffast-math", "-fno-fast-math"]);
        assert_eq!(m.opts.contract, FpContract::Fast);
        let (m, _) = model(&["-ffast-math", "-ffp-model=precise"]);
        assert_eq!(m.opts, FpOptions::default());
        let (m, _) = model(&["-Ofast", "-O2"]);
        assert_eq!(m.opts, FpOptions::default());
        let (m, _) = model(&["-Ofast"]);
        assert!(m.fast_math);
        let (m, _) = model(&["-ffp-contract=fast-honor-pragmas"]);
        assert!(m.opts.contract == FpContract::Fast && !m.backend_fuse);
    }

    #[test]
    fn environment_flags() {
        // clang 19 and 22 `-###`: the cc1 `-frounding-math`,
        // `-ffp-exception-behavior=` and `-ffp-eval-method=` they pass.
        let ok = |a: &[&str]| {
            let (m, e) = model(a);
            assert!(e.is_empty(), "{a:?}: {e:?}");
            m
        };
        let m = ok(&["-frounding-math"]);
        assert!(m.opts.rounding_math && m.opts.except.is_none());
        assert_eq!(m.opts.rounding(), kiln_ast::fp::FpRound::Dynamic);
        assert!(m.opts.constrained() && m.opts.exceptions() == FpExcept::Ignore);
        assert!(!ok(&["-frounding-math", "-fno-rounding-math"]).opts.constrained());
        assert!(!ok(&["-frounding-math", "-ffast-math"]).opts.rounding_math);
        let m = ok(&["-ffp-model=strict"]);
        assert!(m.opts.rounding_math && m.opts.except == Some(FpExcept::Strict));
        assert_eq!(m.opts.contract, FpContract::Off);
        let m = ok(&["-ffp-model=strict", "-ffp-model=precise"]);
        assert!(m.opts.rounding_math && m.opts.except == Some(FpExcept::Strict));
        assert_eq!(m.opts.contract, FpContract::On);
        let m = ok(&["-ffp-model=strict", "-fno-trapping-math"]);
        assert!(m.opts.rounding_math && m.opts.except == Some(FpExcept::Ignore));
        assert_eq!(ok(&["-ftrapping-math"]).opts.except, Some(FpExcept::Strict));
        assert_eq!(ok(&["-fno-trapping-math"]).opts.except, Some(FpExcept::Ignore));
        assert_eq!(ok(&["-ftrapping-math", "-ffast-math"]).opts.except, None);
        assert_eq!(
            ok(&["-ftrapping-math", "-fno-trapping-math"]).opts.except,
            Some(FpExcept::Ignore)
        );
        let m = ok(&["-ffp-exception-behavior=maytrap"]);
        assert!(m.opts.except == Some(FpExcept::MayTrap) && !m.opts.rounding_math);
        assert_eq!(
            ok(&["-ffp-exception-behavior=maytrap", "-fno-trapping-math"]).opts.except,
            Some(FpExcept::Ignore)
        );
        // Strict exceptions keep reassociation, and so the unsafe and
        // fast math, off; may-trap ones do not.
        let m = ok(&["-ffast-math", "-ftrapping-math"]);
        assert!(!m.opts.reassoc && m.opts.arcp && m.opts.afn && m.opts.nsz);
        assert!(!m.unsafe_math && !m.fast_math);
        assert!(!ok(&["-ftrapping-math", "-fassociative-math", "-fno-signed-zeros"]).opts.reassoc);
        let m = ok(&[
            "-ffp-exception-behavior=maytrap",
            "-fassociative-math",
            "-fno-signed-zeros",
            "-freciprocal-math",
            "-fapprox-func",
        ]);
        assert!(m.opts.reassoc && m.unsafe_math);
        let m = ok(&["-ffp-eval-method=double"]);
        assert!(m.eval_on_command_line && m.opts.eval == FpEval::Double);
        let m = ok(&["-ffp-eval-method=source"]);
        assert!(m.eval_on_command_line && m.opts.eval == FpEval::Source);
        assert!(!ok(&[]).eval_on_command_line);
        assert_eq!(ok(&["-ffp-eval-method=extended"]).opts.eval, FpEval::Extended);
        // With a value-changing option: one error per option (clang
        // `FixupInvocation`).
        let e = |a: &[&str]| model(a).1;
        assert_eq!(
            e(&["-ffp-eval-method=source", "-ffast-math"]),
            [0, 1, 2].map(ArgError::IncompatibleFpEvalMethod)
        );
        assert_eq!(
            e(&["-ffp-eval-method=double", "-fapprox-func", "-freciprocal-math"]),
            [0, 2].map(ArgError::IncompatibleFpEvalMethod)
        );
        assert!(e(&["-ffp-eval-method=double", "-fassociative-math"]).is_empty());
        assert_eq!(
            e(&["-fexcess-precision=16"]),
            [ArgError::UnsupportedByKiln("-fexcess-precision=16".into())]
        );
        assert_eq!(
            e(&["-ffp-contract=bogus"]),
            [ArgError::UnsupportedValue("-ffp-contract=".into(), "bogus".into())]
        );
        assert_eq!(
            e(&["-fdenormal-fp-math=ieee,x"]),
            [ArgError::InvalidFlagValue("-fdenormal-fp-math=ieee,x".into(), "ieee,x".into())]
        );
    }

    #[test]
    fn runtime_flag() {
        let r = |a: &[&str]| {
            let mut f = FpFlags::default();
            for x in a {
                f.parse_arg(x);
            }
            f.fast_math_runtime_flag().map(str::to_owned)
        };
        assert_eq!(r(&["-ffast-math"]).as_deref(), Some("-ffast-math"));
        assert_eq!(r(&["-ffast-math", "-fno-unsafe-math-optimizations"]), None);
        assert_eq!(r(&["-ffp-model=fast"]).as_deref(), Some("-ffp-model=fast"));
        assert_eq!(r(&["-ffp-model=fast", "-ffp-model=precise"]), None);
    }

    // clang 19 and 22: only `-fdenormal-fp-math=` sets the denormal mode.
    // The fast-math flags do not (not even on x86-64 with `crtfastmath.o`),
    // and the flags that undo them leave an explicit mode alone.
    #[test]
    fn denormal_only_from_its_flag() {
        for a in [&["-ffast-math"][..], &["-Ofast"], &["-funsafe-math-optimizations"]] {
            assert_eq!(model(a).0.denormal, None, "{a:?}");
        }
        let ps = Some("preserve-sign,preserve-sign");
        for undo in [
            "-ffast-math",
            "-fno-fast-math",
            "-fno-unsafe-math-optimizations",
            "-ffp-model=precise",
            "-ffp-model=fast",
        ] {
            let (m, _) = model(&["-fdenormal-fp-math=preserve-sign", undo]);
            assert_eq!(m.denormal.as_deref(), ps, "{undo}");
        }
        let (m, _) = model(&["-fdenormal-fp-math=preserve-sign", "-fdenormal-fp-math=ieee"]);
        assert_eq!(m.denormal, None);
    }
}
