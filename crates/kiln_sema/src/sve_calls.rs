//! The checks of a call of an SVE or SME ACLE builtin alias (WP-1792,
//! WP-1793; spec/sema.md "SVE and SME builtin calls"): clang
//! `Sema::CheckBuiltinFunctionCall`'s integer-constant operands, then
//! `SemaARM::CheckSVEBuiltinFunctionCall` / `CheckSMEBuiltinFunctionCall`:
//! the streaming mode the builtin needs (`checkArmStreamingBuiltin`), the
//! ZA and ZT0 state it uses (warnings) and its immediates' values
//! (`CheckImmediateArg`). The per-builtin data is clang's own
//! ([`kiln_ast::sve::checks`], generated from clang-tblgen's tables): clang
//! 22's at `-fclang-compat=20` and later, clang 19's below, as the
//! lowering ([`kiln_ast::sve::lookup`]). Every diagnostic is at the call's
//! start; a value-dependent operand is checked when its template is
//! instantiated. The required target features themselves are checked in
//! codegen, as clang's `checkTargetFeatures` (`err_builtin_needs_feature`).

use kiln_ast::sve::{self, Checks, ImmRule, Mode};
use kiln_ast::types::sme;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;

impl Sema<'_, '_> {
    /// The checks of the call of `d`, an alias of SVE or SME builtin `b`,
    /// with the converted arguments `args`; false after an error (the call
    /// is invalid, as clang's `ExprError`).
    pub(crate) fn check_sve_builtin_call(
        &mut self,
        d: DeclId,
        b: Symbol,
        callee: ExprId,
        args: &[ExprId],
    ) -> bool {
        let version = if self.clang_compat >= 20 { 22 } else { 19 };
        let Some(c) = sve::checks(self.ast.symbols().as_str(b), version) else { return true };
        let at = self.begin_loc(callee);
        let name = self.ast.decls.get(d).name().as_ident().map(|s| self.ast.symbols().as_str(s));
        let name = name.unwrap_or_default().to_owned();
        // clang `CheckBuiltinFunctionCall`: the `I` operands first.
        for k in c.ice_args() {
            if args.get(k).is_some_and(|&a| self.sve_imm_value(a) == ImmValue::NotConstant) {
                self.sve_not_constant(&name, at);
                return false;
            }
        }
        if let Some(f) = self.sve_caller(version) {
            if let Some(mode) = c.mode
                && !self.check_streaming_builtin(f, mode, &c, version, at)
            {
                return false;
            }
            let bits = self.fn_sme_bits(f);
            let new = self.arm_new_of(f);
            if c.za && sme::za(bits) == 0 && new & 1 == 0 {
                let msg = self.diag(DiagId::sema_arm_za_builtin_no_za_state, at);
                self.emit(msg);
            }
            if c.zt0 && sme::zt0(bits) == 0 && new & 2 == 0 {
                let msg = self.diag(DiagId::sema_arm_zt0_builtin_no_zt0_state, at);
                self.emit(msg);
            }
        }
        // `PerformSVEImmChecks`: every check, each failing one reported.
        let mut ok = true;
        for k in c.imm_checks() {
            let (Some(&a), Some(rule)) = (args.get(k.arg), k.rule()) else { continue };
            ok &= self.check_sve_imm(a, rule, &name, at);
        }
        ok
    }

    /// The function a builtin call's streaming mode and state are checked
    /// against (clang `getCurFunctionDecl`): at clang 22 the current
    /// function, a lambda's call operator included; at clang 19 a lambda's
    /// call operator stands for the function its lambda is in
    /// (`AllowLambda` false), and outside any function nothing is checked.
    fn sve_caller(&self, version: u32) -> Option<DeclId> {
        let mut f = self.ctx;
        if version < 22 {
            for _ in 0..64 {
                if !self.is_lambda_call_op(f) {
                    break;
                }
                let class = self.ast.decls.get(f).ctx()?;
                f = self.ast.decls.get(class).ctx()?;
            }
        }
        Some(f).filter(|&f| self.ast.decls.get(f).kind().is_function())
    }

    /// clang `checkArmStreamingBuiltin`: builtin checks `c` (mode `mode`)
    /// called from function `f`; false after an error.
    fn check_streaming_builtin(
        &mut self,
        f: DeclId,
        mode: Mode,
        c: &Checks,
        version: u32,
        at: Loc,
    ) -> bool {
        // clang `getArmStreamingFnType`.
        let bits = self.fn_sme_bits(f);
        let fn_mode =
            if self.find_attr(f, kiln_ast::attr::AttrKind::ARM_LOCALLY_STREAMING).is_some()
                || bits & sme::STREAMING != 0
            {
                Mode::Streaming
            } else if bits & sme::STREAMING_COMPATIBLE != 0 {
                Mode::Compatible
            } else {
                Mode::NonStreaming
            };
        let mut need = mode;
        if mode == Mode::VerifyRuntime {
            let has = |s: &Self, n: &str| {
                let module = if n == "sve" { s.sve } else { s.module_feature(n) };
                s.fn_feature(Some(f), n, module)
            };
            // A streaming function without `sme` can never compile: nothing
            // more is reported.
            if fn_mode == Mode::Streaming && !has(self, "sme") {
                return true;
            }
            let (nonstreaming_ok, streaming_ok) = if version >= 22 {
                // The expression is `SVE-EXPR|SME-EXPR`, split at its first
                // top-level `|`; each half is evaluated alone.
                let (nonstreaming, streaming) = split_top_level_pipe(c.features);
                let on = |n: &str| has(self, n);
                (sve::features_hold(nonstreaming, &on), sve::features_hold(streaming, &on))
            } else {
                // clang 19: the whole expression with every `sme*` feature
                // off, then with every `sve*` one off; a streaming-compatible
                // caller satisfying the first is accepted.
                let nonstreaming_feature = |n: &str| !n.starts_with("sme") && has(self, n);
                let streaming_feature = |n: &str| !n.starts_with("sve") && has(self, n);
                let nonstreaming_ok = sve::features_hold(c.features, &nonstreaming_feature);
                if nonstreaming_ok && fn_mode == Mode::Compatible {
                    return true;
                }
                (nonstreaming_ok, sve::features_hold(c.features, &streaming_feature))
            };
            need = match (nonstreaming_ok, streaming_ok) {
                (true, true) | (false, false) => return true,
                (true, false) => Mode::NonStreaming,
                (false, true) => Mode::Streaming,
            };
        }
        let what = match (fn_mode, need) {
            (Mode::Streaming | Mode::Compatible, Mode::NonStreaming) => "non-streaming",
            (Mode::NonStreaming | Mode::Compatible, Mode::Streaming) => "streaming",
            _ => return true,
        };
        let msg = self.diag(DiagId::sema_arm_sm_incompat_builtin, at).arg(what);
        self.emit(msg);
        false
    }

    /// The value of the immediate operand `a` as clang's
    /// `BuiltinConstantArg` sees it.
    fn sve_imm_value(&mut self, a: ExprId) -> ImmValue {
        if self.expr_is_value_dependent(a) {
            return ImmValue::Dependent;
        }
        let t = self.ety(a);
        let width = crate::conv::rank::int_width(self, t);
        // An integer constant expression (clang `getIntegerConstantExpr`):
        // evaluated as one, not folded (an out-of-range value cast to an
        // enumeration is no constant at clang 22).
        let ctx = kiln_oven::EvalContext::new(kiln_oven::EvalKind::Other, self.begin_loc(a));
        let v = self.eval_int(a, ctx).ok();
        let (Some(v), Some(w)) = (v, width) else { return ImmValue::NotConstant };
        let w = w.clamp(1, 64);
        let low = v & ((1i128 << w) - 1);
        let sext = if low >> (w - 1) & 1 == 1 { low - (1i128 << w) } else { low };
        #[allow(clippy::cast_possible_truncation, reason = "sign-extended from at most 64 bits")]
        ImmValue::Value(v, sext as i64)
    }

    /// clang `err_constant_integer_arg_type` for the call of `name` at `at`.
    fn sve_not_constant(&mut self, name: &str, at: Loc) {
        let msg = self.diag(DiagId::sema_constant_integer_arg_type, at).arg(name.to_owned());
        self.emit(msg);
    }

    /// clang `CheckImmediateArg` for operand `a` and rule `rule`; false
    /// after an error.
    fn check_sve_imm(&mut self, a: ExprId, rule: ImmRule, name: &str, at: Loc) -> bool {
        let value = self.sve_imm_value(a);
        if value == ImmValue::Dependent {
            return true;
        }
        // `BuiltinConstantArgRange` checks nothing in a constant-evaluated
        // context (`isConstantEvaluatedContext`); the other checks do.
        if matches!(rule, ImmRule::Range(..) | ImmRule::RangeMultiple(..))
            && self.in_constant_evaluated()
        {
            return true;
        }
        let ImmValue::Value(shown, v) = value else {
            self.sve_not_constant(name, at);
            return false;
        };
        let (id, args) = match rule {
            ImmRule::Range(lo, hi) | ImmRule::RangeMultiple(lo, hi, _)
                if !(lo..=hi).contains(&v) =>
            {
                (
                    DiagId::sema_argument_invalid_range,
                    vec![shown.to_string(), lo.to_string(), hi.to_string()],
                )
            }
            ImmRule::RangeMultiple(_, _, m) if v % m != 0 => {
                (DiagId::sema_argument_not_multiple, vec![m.to_string()])
            }
            ImmRule::Rot90Or270 if !matches!(v, 90 | 270) => {
                (DiagId::sema_rotation_argument_to_cadd, Vec::new())
            }
            ImmRule::RotAll90 if !matches!(v, 0 | 90 | 180 | 270) => {
                (DiagId::sema_rotation_argument_to_cmla, Vec::new())
            }
            _ => return true,
        };
        let mut msg = self.diag(id, at);
        for x in args {
            msg = msg.arg(x);
        }
        self.emit(msg);
        false
    }
}

/// An immediate operand's value (clang `BuiltinConstantArg`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ImmValue {
    /// Value-dependent: checked at instantiation.
    Dependent,
    /// Not an integer constant expression.
    NotConstant,
    /// The value in its type, and sign-extended from its width
    /// (`APSInt::getSExtValue`).
    Value(i128, i64),
}

/// `features` split at its first `|` outside parentheses (clang
/// `checkArmStreamingBuiltin`'s `FindTopLevelPipe`): the non-streaming
/// (SVE) half and the streaming (SME) half.
fn split_top_level_pipe(features: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (i, c) in features.bytes().enumerate() {
        match c {
            b'|' if depth == 0 => return (&features[..i], &features[i + 1..]),
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
    }
    (features, "")
}

#[cfg(test)]
mod tests {
    use super::split_top_level_pipe;

    #[test]
    fn top_level_pipe() {
        assert_eq!(split_top_level_pipe("(sve,(sve2|sme))|sme"), ("(sve,(sve2|sme))", "sme"));
        assert_eq!(split_top_level_pipe("sve|sme"), ("sve", "sme"));
        assert_eq!(split_top_level_pipe("sve"), ("sve", ""));
    }
}
