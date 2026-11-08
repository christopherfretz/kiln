//! The floating-point flags (spec/driver.md "Floating-point flags",
//! WP-806): clang 19's `RenderFloatingPointOptions` state machine over
//! `-ffast-math`, its parts, `-ffp-model=`, `-ffp-contract=` and the
//! floating-point environment flags, in command-line order.
//!
//! The flags that would need the floating-point environment (trapping,
//! dynamic rounding, a wider evaluation method) are errors, never
//! silently ignored: kiln does not implement `#pragma STDC FENV_ACCESS`
//! semantics (spec/codegen.md "Floating-point semantics").

use kiln_ast::fp::{FpContract, FpOptions};
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
    /// The `-fdenormal-fp-math=` mode, `"output,input"` (`None`: IEEE).
    pub denormal: Option<String>,
    /// The `_Complex` multiply/divide algorithm (clang's cc1
    /// `-complex-range=`; none is full).
    pub complex_range: ComplexRange,
}

/// clang's `LangOptions::ComplexRangeKind` as the driver tracks it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Range {
    Kiln(ComplexRange),
    /// `promoted` (clang 22's `-ffp-model=fast`): division in the next
    /// wider type, which kiln does not implement.
    Promoted,
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
    /// clang 22's rules over clang 19's (`-fclang-compat=`),
    /// `default_denormal` is the target's denormal mode (x86-64 with
    /// `crtfastmath.o`: `preserve-sign`). Flags whose final effect needs
    /// the FP environment are reported to `errors`.
    #[must_use]
    pub fn resolve(
        &self,
        ofast: bool,
        clang22: bool,
        default_denormal: Option<&str>,
        errors: &mut Vec<ArgError>,
    ) -> FpModel {
        let mut s = State::new(default_denormal, clang22);
        for a in &self.seq {
            s.apply(a, ofast);
        }
        s.finish(errors)
    }
}

/// clang's local variables in `RenderFloatingPointOptions`.
#[allow(clippy::struct_excessive_bools, reason = "clang's flags")]
struct State<'a> {
    honor_infs: bool,
    honor_nans: bool,
    approx_func: bool,
    math_errno: bool,
    assoc: bool,
    recip: bool,
    signed_zeros: bool,
    /// The flag that made math trap (`-ftrapping-math`,
    /// `-ffp-exception-behavior=maytrap|strict`, `-ffp-model=strict`).
    trapping: Option<String>,
    /// The flag that turned on dynamic rounding.
    rounding: Option<String>,
    /// The flag that selected a wider evaluation method.
    eval: Option<String>,
    /// The flag that selected `_Float16` without excess precision.
    excess: Option<String>,
    contract: &'static str,
    last_seen_contract: Option<&'static str>,
    seen_unsafe: bool,
    denormal: Option<String>,
    default_denormal: Option<&'a str>,
    /// clang 22's rules (`-ffp-model=fast` keeps NaNs and infinities and
    /// promotes complex division; `aggressive` is the old `fast`).
    clang22: bool,
    /// The complex range, and the flag that set it.
    range: Option<(Range, String)>,
    /// `-ffp-model=aggressive` under clang 19 (an unsupported value).
    aggressive19: bool,
}

impl<'a> State<'a> {
    fn new(default_denormal: Option<&'a str>, clang22: bool) -> Self {
        State {
            honor_infs: true,
            honor_nans: true,
            approx_func: false,
            // The Linux toolchains' default (clang `IsMathErrnoDefault`).
            math_errno: true,
            assoc: false,
            recip: false,
            signed_zeros: true,
            trapping: None,
            rounding: None,
            eval: None,
            excess: None,
            contract: "on",
            last_seen_contract: None,
            seen_unsafe: false,
            denormal: default_denormal.map(str::to_owned),
            default_denormal,
            clang22,
            range: None,
            aggressive19: false,
        }
    }

    fn set_range(&mut self, r: Range, flag: &str) {
        self.range = Some((r, flag.to_owned()));
    }

    /// clang's `applyFastMath` (with the basic complex range of
    /// `-ffast-math`, `-Ofast` and `-ffp-model=aggressive`).
    fn fast_math(&mut self, flag: &str) {
        self.set_range(Range::Kiln(ComplexRange::Basic), flag);
        self.honor_infs = false;
        self.honor_nans = false;
        self.math_errno = false;
        self.assoc = true;
        self.recip = true;
        self.approx_func = true;
        self.signed_zeros = false;
        self.trapping = None;
        self.rounding = None;
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

    fn apply(&mut self, a: &str, ofast: bool) {
        let owned = || Some(a.to_owned());
        match a {
            "-Ofast" if ofast => self.fast_math(a),
            "-Ofast" => {}
            "-ffast-math" => self.fast_math(a),
            "-fno-fast-math" => {
                // clang 22 drops the complex range too; clang 19 keeps it.
                if self.clang22 {
                    self.range = None;
                }
                self.honor_infs = true;
                self.honor_nans = true;
                self.math_errno = true;
                self.assoc = false;
                self.recip = false;
                self.approx_func = false;
                self.signed_zeros = true;
                self.denormal = self.default_denormal.map(str::to_owned);
                self.restore_contract();
            }
            "-funsafe-math-optimizations" => {
                self.assoc = true;
                self.recip = true;
                self.signed_zeros = false;
                self.approx_func = true;
                self.trapping = None;
                self.contract = "fast";
                self.seen_unsafe = true;
            }
            "-fno-unsafe-math-optimizations" => {
                self.assoc = false;
                self.recip = false;
                self.signed_zeros = true;
                self.approx_func = false;
                self.denormal = None;
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
            "-ftrapping-math" => self.trapping = owned(),
            "-fno-trapping-math" => self.trapping = None,
            "-frounding-math" => self.rounding = owned(),
            "-fno-rounding-math" => self.rounding = None,
            "-fcx-limited-range" => self.set_range(Range::Kiln(ComplexRange::Basic), a),
            "-fcx-fortran-rules" => self.set_range(Range::Kiln(ComplexRange::Improved), a),
            "-fno-cx-limited-range" | "-fno-cx-fortran-rules" => {
                self.set_range(Range::Kiln(ComplexRange::Full), a);
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
            // trapping stay).
            self.honor_infs = true;
            self.honor_nans = true;
            self.approx_func = false;
            self.math_errno = true;
            self.assoc = false;
            self.recip = false;
            self.signed_zeros = true;
            self.contract = "on";
            self.denormal = None;
            match v {
                "fast" if self.clang22 => {
                    // clang 22: fast math without the finite-math flags,
                    // and promoted complex division.
                    self.fast_math(a);
                    (self.honor_infs, self.honor_nans) = (true, true);
                    self.set_range(Range::Promoted, a);
                }
                "fast" => self.fast_math(a),
                "aggressive" if self.clang22 => self.fast_math(a),
                "aggressive" => self.aggressive19 = true,
                "strict" => {
                    self.trapping = owned();
                    self.rounding = owned();
                    self.contract = "off";
                }
                _ => {}
            }
            if self.clang22 && matches!(v, "precise" | "strict") {
                self.set_range(Range::Kiln(ComplexRange::Full), a);
            }
        } else if let Some(v) = a.strip_prefix("-ffp-exception-behavior=") {
            self.trapping = (v != "ignore").then(owned).flatten();
        } else if let Some(v) = a.strip_prefix("-ffp-eval-method=") {
            self.eval = (v != "source").then(owned).flatten();
        } else if let Some(v) = a.strip_prefix("-fexcess-precision=") {
            self.excess = (v == "16").then(owned).flatten();
        } else if let Some(v) = a.strip_prefix("-fcomplex-arithmetic=") {
            if let Some(r) = ComplexRange::from_arg(v) {
                self.set_range(Range::Kiln(r), a);
            }
        } else if let Some(v) = a.strip_prefix("-fdenormal-fp-math=") {
            let v = if v.contains(',') { v.to_owned() } else { format!("{v},{v}") };
            self.denormal = (v != "ieee,ieee").then_some(v);
        }
    }

    fn finish(self, errors: &mut Vec<ArgError>) -> FpModel {
        let mut named: Vec<&String> = Vec::new();
        for f in [&self.trapping, &self.rounding, &self.eval, &self.excess].into_iter().flatten() {
            // One error per flag (`-ffp-model=strict` sets two fields).
            if !named.contains(&f) {
                named.push(f);
                errors.push(ArgError::UnsupportedByKiln(f.clone()));
            }
        }
        if self.aggressive19 {
            errors.push(ArgError::UnsupportedValue("-ffp-model=".into(), "aggressive".into()));
        }
        let complex_range = match &self.range {
            None => ComplexRange::Full,
            Some((Range::Kiln(r), _)) => *r,
            Some((Range::Promoted, flag)) => {
                // Promoted complex division (clang 22 `-ffp-model=fast`)
                // is not implemented: an error, never full or basic
                // (spec/driver.md "Floating-point flags").
                errors.push(ArgError::UnsupportedByKiln(format!(
                    "{flag} (promoted complex arithmetic; add -fcomplex-arithmetic=basic)"
                )));
                ComplexRange::Full
            }
        };
        let unsafe_math = self.assoc && self.recip && !self.signed_zeros && self.approx_func;
        let fast_math = !self.honor_infs && !self.honor_nans && !self.math_errno && unsafe_math;
        FpModel {
            opts: FpOptions {
                contract: match self.contract {
                    "off" => FpContract::Off,
                    "on" => FpContract::On,
                    _ => FpContract::Fast,
                },
                reassoc: self.assoc && !self.signed_zeros,
                arcp: self.recip,
                nnan: !self.honor_nans,
                ninf: !self.honor_infs,
                nsz: !self.signed_zeros,
                afn: self.approx_func,
                math_errno: self.math_errno,
            },
            backend_fuse: self.contract == "fast",
            fast_math,
            unsafe_math,
            denormal: self.denormal,
            complex_range,
        }
    }
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
        let m = f.resolve(ofast, clang22, None, &mut errors);
        (m, errors)
    }

    #[test]
    fn clang22_fp_models() {
        // clang 22: `fast` keeps NaNs and infinities (no `__FAST_MATH__`)
        // and promotes complex division (an error in kiln); `aggressive`
        // is the old `fast`.
        let (m, e) = model_at(true, &["-ffp-model=fast"]);
        assert!(!m.fast_math && m.unsafe_math && !m.opts.nnan && !m.opts.ninf);
        assert!(m.opts.reassoc && !m.opts.math_errno && m.opts.contract == FpContract::Fast);
        assert!(
            matches!(&e[..], [ArgError::UnsupportedByKiln(f)] if f.starts_with("-ffp-model=fast"))
        );
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
    fn environment_flags_are_errors() {
        let e = |a: &[&str]| model(a).1;
        assert_eq!(
            e(&["-frounding-math"]),
            [ArgError::UnsupportedByKiln("-frounding-math".into())]
        );
        assert_eq!(e(&["-ffp-model=strict"]).len(), 1);
        assert!(e(&["-ffp-model=strict", "-fno-rounding-math", "-fno-trapping-math"]).is_empty());
        assert!(e(&["-ftrapping-math", "-ffast-math"]).is_empty());
        assert!(e(&["-ffp-exception-behavior=ignore", "-ffp-eval-method=source"]).is_empty());
        assert_eq!(e(&["-ffp-eval-method=double"]).len(), 1);
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
}
