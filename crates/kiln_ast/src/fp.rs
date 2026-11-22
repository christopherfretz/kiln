//! Floating-point semantics of a scope (spec/codegen.md "Floating-point
//! semantics"; clang's `FPOptions` / `FPOptionsOverride`).
//!
//! The command line gives the translation unit's [`FpOptions`]; the FP
//! pragmas (`#pragma STDC FP_CONTRACT`, `#pragma clang fp`,
//! `#pragma float_control`) give an [`FpOverride`] of some of its
//! fields. Sema records the override in effect in each compound statement
//! (relative to the command line, so codegen applies it to its own base);
//! a template instantiation keeps its pattern's.

/// When `a * b + c` may be fused ([expr.pre]/6, C23 7.12.2;
/// `-ffp-contract=`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash, kiln_snap::Snap)]
pub enum FpContract {
    /// Never (`off`).
    Off,
    /// Within one expression (`on`, clang's C++ default): `llvm.fmuladd`.
    #[default]
    On,
    /// Anywhere the optimizer likes (`fast`): the `contract` flag.
    Fast,
}

/// The rounding mode a scope declares (clang `ConstRoundingMode`;
/// `#pragma STDC FENV_ROUND`, C23 7.6.2; spec/codegen.md
/// "Floating-point semantics"). A directed mode applies to the scope's
/// operations at run time (`round.<mode>` constrained intrinsics) and in
/// constant evaluation (spec/oven.md "Floating point").
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash, kiln_snap::Snap)]
pub enum FpRound {
    /// `FE_DYNAMIC`, the default: the dynamic mode under `FENV_ACCESS ON`,
    /// else round to nearest (C23 7.6.2/3).
    #[default]
    Dynamic,
    /// `FE_TONEAREST`.
    Nearest,
    /// `FE_UPWARD`.
    Upward,
    /// `FE_DOWNWARD`.
    Downward,
    /// `FE_TOWARDZERO`.
    TowardZero,
    /// `FE_TONEARESTFROMZERO` (ties away from zero).
    NearestAway,
}

impl FpRound {
    /// Whether this is a directed mode (one constant evaluation rounds
    /// in): neither `FE_DYNAMIC` nor `FE_TONEAREST`.
    pub fn is_directed(self) -> bool {
        !matches!(self, FpRound::Dynamic | FpRound::Nearest)
    }

    /// LLVM's metadata name (`round.<mode>`, the LLVM language
    /// reference's "Constrained Floating-Point Intrinsics").
    pub fn llvm_name(self) -> &'static str {
        match self {
            FpRound::Dynamic => "round.dynamic",
            FpRound::Nearest => "round.tonearest",
            FpRound::Upward => "round.upward",
            FpRound::Downward => "round.downward",
            FpRound::TowardZero => "round.towardzero",
            FpRound::NearestAway => "round.tonearestaway",
        }
    }
}

/// How floating-point operations evaluate (clang `FPEvalMethodKind`;
/// `#pragma clang fp eval_method`, `-ffp-eval-method=`, C23 5.2.4.2.2
/// `FLT_EVAL_METHOD`): an operand of a type ranked below `double`
/// (`Double`) or `long double` (`Extended`) is converted to it by the
/// usual unary conversions (spec/sema.md "Excess precision").
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash, kiln_snap::Snap)]
pub enum FpEval {
    /// `source` (`FLT_EVAL_METHOD` 0, the default on every kiln target).
    #[default]
    Source,
    /// `double` (1).
    Double,
    /// `extended` (2).
    Extended,
}

/// The floating-point exception behavior (clang `FPExceptionModeKind`;
/// `#pragma clang fp exceptions`, `#pragma float_control(except, ...)`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, kiln_snap::Snap)]
pub enum FpExcept {
    /// Exceptions are not observed (`fpexcept.ignore`).
    Ignore,
    /// Spurious exceptions may be raised, none is lost (`fpexcept.maytrap`).
    MayTrap,
    /// Exceptions are exactly the source's (`fpexcept.strict`).
    Strict,
}

/// The FP semantics in effect (clang `FPOptions`, the fields kiln
/// implements, spec/codegen.md). The default is clang's for C++ on
/// Linux: `fp_contract(on)` and `errno`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, kiln_snap::Snap)]
#[allow(clippy::struct_excessive_bools, reason = "LLVM's fast-math flags")]
pub struct FpOptions {
    /// Contraction.
    pub contract: FpContract,
    /// `reassoc` (`-fassociative-math` with `-fno-signed-zeros`).
    pub reassoc: bool,
    /// `arcp` (`-freciprocal-math`).
    pub arcp: bool,
    /// `nnan` (`-fno-honor-nans`).
    pub nnan: bool,
    /// `ninf` (`-fno-honor-infinities`).
    pub ninf: bool,
    /// `nsz` (`-fno-signed-zeros`).
    pub nsz: bool,
    /// `afn` (`-fapprox-func`).
    pub afn: bool,
    /// Library math functions may set `errno` (`-fmath-errno`, the Linux
    /// default): they are not lowered to LLVM intrinsics.
    pub math_errno: bool,
    /// The declared rounding mode.
    pub round: FpRound,
    /// The exception behavior a pragma specified; `None`: strict under
    /// `FENV_ACCESS ON`, else ignored.
    pub except: Option<FpExcept>,
    /// `#pragma STDC FENV_ACCESS ON` (C23 7.6.1): the program may test
    /// the status flags and change the control modes.
    pub fenv_access: bool,
    /// The evaluation method.
    pub eval: FpEval,
    /// `-frounding-math` (clang `LangOptions::RoundingMath`): `FE_DYNAMIC`
    /// is the dynamic mode even outside `FENV_ACCESS ON`. A command-line
    /// option only; no pragma changes it.
    pub rounding_math: bool,
}

impl FpOptions {
    /// The effective rounding mode (clang `FPOptions::getRoundingMode`):
    /// `Dynamic` is round to nearest outside `FENV_ACCESS ON` without
    /// `-frounding-math`.
    pub fn rounding(&self) -> FpRound {
        if self.round == FpRound::Dynamic && !self.fenv_access && !self.rounding_math {
            FpRound::Nearest
        } else {
            self.round
        }
    }

    /// The effective exception behavior (clang
    /// `FPOptions::getExceptionMode`).
    pub fn exceptions(&self) -> FpExcept {
        self.except.unwrap_or(if self.fenv_access { FpExcept::Strict } else { FpExcept::Ignore })
    }

    /// Whether FP operations must respect the floating-point environment
    /// (clang `isFPConstrained`): a function containing such a scope is
    /// `strictfp` and emits its FP operations as
    /// `llvm.experimental.constrained.*` (spec/codegen.md).
    pub fn constrained(&self) -> bool {
        self.rounding() != FpRound::Nearest
            || self.exceptions() != FpExcept::Ignore
            || self.fenv_access
    }

    /// Whether an FP operation's result depends on the dynamic
    /// environment, so a fold outside a constant context is not attempted
    /// (clang `checkFloatingPointResult`, which fails an inexact result
    /// under the dynamic mode and any exception when exceptions or the
    /// environment are observed). A directed declared mode alone folds,
    /// in that mode.
    pub fn env_dependent(&self) -> bool {
        self.rounding() == FpRound::Dynamic
            || self.exceptions() != FpExcept::Ignore
            || self.fenv_access
    }

    /// Whether value-changing optimizations are off (clang
    /// `Sema::isPreciseFPEnabled`).
    pub fn precise(&self) -> bool {
        !self.reassoc && !self.nsz && !self.arcp && !self.afn
    }
}

impl Default for FpOptions {
    fn default() -> Self {
        FpOptions { math_errno: true, ..FpOverride::NONE.vals }
    }
}

/// [`FpOverride::mask`] bits: which [`FpOptions`] fields an override sets
/// (values stable within a PCH `FORMAT_VERSION`, spec/ast.md §20).
pub mod fp_bit {
    /// `contract`.
    pub const CONTRACT: u16 = 1;
    /// `reassoc`.
    pub const REASSOC: u16 = 2;
    /// `arcp`.
    pub const ARCP: u16 = 4;
    /// `nnan`.
    pub const NNAN: u16 = 8;
    /// `ninf`.
    pub const NINF: u16 = 16;
    /// `nsz`.
    pub const NSZ: u16 = 32;
    /// `afn`.
    pub const AFN: u16 = 64;
    /// `math_errno`.
    pub const ERRNO: u16 = 128;
    /// `round`.
    pub const ROUND: u16 = 256;
    /// `except`.
    pub const EXCEPT: u16 = 512;
    /// `fenv_access`.
    pub const FENV: u16 = 1024;
    /// `eval`.
    pub const EVAL: u16 = 2048;
    /// Every bit, named after its [`super::FpOptions`] field.
    pub const ALL: [(&str, u16); 12] = [
        ("contract", CONTRACT),
        ("reassoc", REASSOC),
        ("arcp", ARCP),
        ("nnan", NNAN),
        ("ninf", NINF),
        ("nsz", NSZ),
        ("afn", AFN),
        ("math_errno", ERRNO),
        ("round", ROUND),
        ("except", EXCEPT),
        ("fenv_access", FENV),
        ("eval", EVAL),
    ];
}

/// Some [`FpOptions`] fields set by pragmas (clang `FPOptionsOverride`):
/// the fields in `mask` take their value from `vals`, the others the
/// command line's.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, kiln_snap::Snap)]
pub struct FpOverride {
    /// The [`fp_bit`]s set.
    pub mask: u16,
    /// The values of the set fields.
    pub vals: FpOptions,
    /// `#pragma STDC CX_LIMITED_RANGE ON|OFF` (C23 7.3.4): complex
    /// multiplication and division are algebraic (`ON`) or Annex G's
    /// (`OFF`); `None` is the command line's `-fcomplex-arithmetic=`.
    pub cx_limited: Option<bool>,
    /// On the body of a function (kept by its instantiations): a scope in
    /// it is [`FpOptions::constrained`], so the function is `strictfp`
    /// (clang `FunctionDecl::usesFPIntrin`).
    pub strict_fn: bool,
}

impl FpOverride {
    /// No field set.
    pub const NONE: FpOverride = FpOverride {
        mask: 0,
        vals: FpOptions {
            contract: FpContract::On,
            reassoc: false,
            arcp: false,
            nnan: false,
            ninf: false,
            nsz: false,
            afn: false,
            math_errno: false,
            round: FpRound::Dynamic,
            except: None,
            fenv_access: false,
            eval: FpEval::Source,
            rounding_math: false,
        },
        cx_limited: None,
        strict_fn: false,
    };

    /// Whether no field is set.
    pub fn is_empty(&self) -> bool {
        self.mask == 0 && self.cx_limited.is_none() && !self.strict_fn
    }

    /// `base` with the set fields replaced.
    pub fn apply(&self, base: FpOptions) -> FpOptions {
        let pick = |bit: u16, o: bool, b: bool| if self.mask & bit != 0 { o } else { b };
        let v = &self.vals;
        FpOptions {
            contract: if self.mask & fp_bit::CONTRACT != 0 { v.contract } else { base.contract },
            reassoc: pick(fp_bit::REASSOC, v.reassoc, base.reassoc),
            arcp: pick(fp_bit::ARCP, v.arcp, base.arcp),
            nnan: pick(fp_bit::NNAN, v.nnan, base.nnan),
            ninf: pick(fp_bit::NINF, v.ninf, base.ninf),
            nsz: pick(fp_bit::NSZ, v.nsz, base.nsz),
            afn: pick(fp_bit::AFN, v.afn, base.afn),
            math_errno: pick(fp_bit::ERRNO, v.math_errno, base.math_errno),
            round: if self.mask & fp_bit::ROUND != 0 { v.round } else { base.round },
            except: if self.mask & fp_bit::EXCEPT != 0 { v.except } else { base.except },
            fenv_access: pick(fp_bit::FENV, v.fenv_access, base.fenv_access),
            eval: if self.mask & fp_bit::EVAL != 0 { v.eval } else { base.eval },
            rounding_math: base.rounding_math,
        }
    }

    /// Sets the evaluation method (`#pragma clang fp eval_method`).
    pub fn set_eval(&mut self, m: FpEval) {
        self.mask |= fp_bit::EVAL;
        self.vals.eval = m;
    }

    /// Sets the declared rounding mode (`#pragma STDC FENV_ROUND`).
    pub fn set_round(&mut self, r: FpRound) {
        self.mask |= fp_bit::ROUND;
        self.vals.round = r;
    }

    /// Sets the exception behavior.
    pub fn set_except(&mut self, e: FpExcept) {
        self.mask |= fp_bit::EXCEPT;
        self.vals.except = Some(e);
    }

    /// Sets the contraction mode (`None`: back to the command line's,
    /// `#pragma STDC FP_CONTRACT DEFAULT`).
    pub fn set_contract(&mut self, c: Option<FpContract>) {
        if let Some(c) = c {
            self.mask |= fp_bit::CONTRACT;
            self.vals.contract = c;
        } else {
            self.mask &= !fp_bit::CONTRACT;
            self.vals.contract = FpContract::On;
        }
    }

    /// Sets the boolean field `bit` to `on`.
    pub fn set_flag(&mut self, bit: u16, on: bool) {
        self.mask |= bit;
        let v = &mut self.vals;
        match bit {
            fp_bit::REASSOC => v.reassoc = on,
            fp_bit::ARCP => v.arcp = on,
            fp_bit::NNAN => v.nnan = on,
            fp_bit::NINF => v.ninf = on,
            fp_bit::NSZ => v.nsz = on,
            fp_bit::AFN => v.afn = on,
            fp_bit::ERRNO => v.math_errno = on,
            fp_bit::FENV => v.fenv_access = on,
            _ => unreachable!("not a boolean FP field: {bit}"),
        }
    }

    /// The `math_errno` override, if set (clang lowers a library call to
    /// an intrinsic under `-O1`+ when a pragma turned `errno` off).
    pub fn math_errno(&self) -> Option<bool> {
        (self.mask & fp_bit::ERRNO != 0).then_some(self.vals.math_errno)
    }

    /// `#pragma float_control(precise, on|off)` (clang
    /// `FPOptionsOverride::setFPPreciseEnabled`): `on` is IEEE semantics
    /// with `errno` and `fp_contract(on)`, `off` the fast-math set
    /// without `errno` and with `fp_contract(fast)`.
    pub fn set_precise(&mut self, on: bool) {
        for bit in
            [fp_bit::REASSOC, fp_bit::NNAN, fp_bit::NINF, fp_bit::NSZ, fp_bit::ARCP, fp_bit::AFN]
        {
            self.set_flag(bit, !on);
        }
        self.set_flag(fp_bit::ERRNO, on);
        self.set_contract(Some(if on { FpContract::On } else { FpContract::Fast }));
    }
}
