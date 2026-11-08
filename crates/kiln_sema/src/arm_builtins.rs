//! The operand checks clang 19 `SemaARM::CheckAArch64BuiltinFunctionCall`
//! applies to the ACLE builtins after their arguments are converted
//! (spec/codegen.md "Target builtins", WP-846): the system-register
//! string of `__builtin_arm_rsr`/`wsr` and their 64-bit, pointer and
//! 128-bit forms (`BuiltinARMSpecialReg`), the PSTATE immediates, and the
//! `prefetch` and barrier immediate ranges. Every error is at the call's
//! begin, as clang's.
//!
//! The exclusive accesses are checked as their call is built
//! (`builtin_arm_exclusive`). clang's size check there (a pointee wider
//! than 16 bytes) cannot fire on `AArch64`: every integer, floating and
//! pointer type is at most 16 bytes (`_BitInt` wider than 128 bits is
//! rejected when the type is formed).

use kiln_ast::ExprId;
use kiln_ast::decl::builtin;
use kiln_ast::expr::ExprKind;
use kiln_diag::DiagId;

use crate::Sema;

/// The immediate operands of `name` (an ACLE builtin without its
/// `__builtin_arm_` prefix) as `(index, lo, hi)`.
fn ranges(name: &str) -> &'static [(usize, i128, i128)] {
    match name {
        "prefetch" => &[(1, 0, 1), (2, 0, 3), (3, 0, 1), (4, 0, 1)],
        "dmb" | "dsb" | "isb" => &[(0, 0, 15)],
        _ => &[],
    }
}

/// The largest immediate an `MSR (immediate)` PSTATE field `reg` takes,
/// if `reg` names one (case-insensitively).
fn pstate_limit(reg: &str) -> Option<i128> {
    const FIELDS: [(&str, i128); 10] = [
        ("spsel", 15),
        ("daifclr", 15),
        ("daifset", 15),
        ("pan", 15),
        ("uao", 15),
        ("dit", 15),
        ("ssbs", 15),
        ("tco", 15),
        ("allint", 1),
        ("pm", 1),
    ];
    FIELDS.iter().find(|(n, _)| n.eq_ignore_ascii_case(reg)).map(|&(_, m)| m)
}

/// Whether the five `o0:op1:CRn:CRm:op2` fields of a system-register
/// string are base-10 integers within their encoding's ranges (LLVM
/// `StringRef::getAsInteger` accepts a leading `-`, never a `+`). `o0` is
/// at most `max_o0`: 1 for clang 19, 3 (the full `op0`) for clang 22.
fn valid_encoding(fields: &[&str], max_o0: i32) -> bool {
    let max = [max_o0, 7, 15, 15, 7];
    fields.len() == 5
        && fields.iter().zip(max).all(|(f, max)| {
            let digits = f.strip_prefix('-').unwrap_or(f);
            !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit())
                && f.parse::<i32>().is_ok_and(|v| (0..=max).contains(&v))
        })
}

impl Sema<'_, '_> {
    /// Checks the converted operands of the ACLE builtin `id`; false
    /// after an error (the call is invalid).
    pub(crate) fn check_arm_builtin(&mut self, id: u32, callee: ExprId, args: &[ExprId]) -> bool {
        let Some(name) =
            builtin::arm::row(id).and_then(|r| r.info.name.strip_prefix("__builtin_arm_"))
        else {
            return true;
        };
        if matches!(name, "rsr" | "rsrp" | "wsr" | "wsrp" | "rsr64" | "wsr64" | "rsr128" | "wsr128")
        {
            return self.check_arm_special_reg(id, name, callee, args);
        }
        ranges(name).iter().all(|&(i, lo, hi)| self.check_arm_range(id, callee, (args, i), lo, hi))
    }

    /// clang `BuiltinConstantArgRange`: operand `i` of the call of `id`
    /// is an integer constant in `[lo, hi]` (a value-dependent one is
    /// checked when instantiated).
    fn check_arm_range(
        &mut self,
        id: u32,
        callee: ExprId,
        arg: (&[ExprId], usize),
        lo: i128,
        hi: i128,
    ) -> bool {
        let Some(a) = arg.0.get(arg.1).copied().filter(|&a| !self.expr_is_value_dependent(a))
        else {
            return true;
        };
        let at = self.begin_loc(callee);
        let int = self.ast.types.is_integral(self.ety(a));
        let Some(v) = self.fold_int(a).filter(|_| int) else {
            let name = builtin::info(id).map_or("", |e| e.name);
            let d = self.diag(DiagId::sema_constant_integer_arg_type, at).arg(name.to_string());
            self.emit(d);
            return false;
        };
        if (lo..=hi).contains(&v) {
            return true;
        }
        let d = self
            .diag(DiagId::sema_argument_invalid_range, at)
            .arg(v.to_string())
            .arg(lo.to_string())
            .arg(hi.to_string());
        self.emit(d);
        false
    }

    /// clang `BuiltinARMSpecialReg`: the register operand is a string
    /// literal that is either a name (unchecked, except that a write of a
    /// PSTATE field takes an immediate in its range) or five
    /// colon-separated encoding fields in range.
    fn check_arm_special_reg(
        &mut self,
        id: u32,
        name: &str,
        callee: ExprId,
        args: &[ExprId],
    ) -> bool {
        let Some(&a) = args.first() else { return true };
        if self.expr_is_value_dependent(a) {
            return true;
        }
        let at = self.begin_loc(callee);
        let lit = self.ignore_paren_imp_casts(a);
        if self.ast.exprs.get(lit).kind() != ExprKind::StringLiteral {
            let d = self.diag(DiagId::sema_expr_not_string_literal, at);
            self.emit(d);
            return false;
        }
        let (_, bytes) = self.ast.strings.get(self.ast.exprs.string_lit(lit));
        let reg = String::from_utf8_lossy(bytes.strip_suffix(&[0]).unwrap_or(bytes)).into_owned();
        let fields: Vec<&str> = reg.split(':').collect();
        if fields.len() > 1 {
            let max_o0 = if self.clang_compat >= 22 { 3 } else { 1 };
            if valid_encoding(&fields, max_o0) {
                return true;
            }
            let d = self.diag(DiagId::sema_arm_invalid_specialreg, at);
            self.emit(d);
            return false;
        }
        if args.len() != 2 || name.ends_with("128") {
            return true;
        }
        match pstate_limit(&reg) {
            Some(max) => self.check_arm_range(id, callee, (args, 1), 0, max),
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings() {
        let ok = |s: &str| valid_encoding(&s.split(':').collect::<Vec<_>>(), 1);
        assert!(ok("1:7:15:15:7"));
        assert!(ok("0:0:0:0:0"));
        assert!(!ok("2:2:3:4:5"));
        assert!(!ok("1:2:3:4"));
        assert!(!ok("1:7:16:15:7"));
        assert!(!ok("a:b:c:d:e"));
        assert!(!ok("+1:2:3:4:5"));
        assert!(!ok(":2:3:4:5"));
        assert!(!ok("-1:2:3:4:5"));
        assert!(valid_encoding(&["3", "3", "14", "0", "2"], 3));
        assert!(!valid_encoding(&["4", "3", "14", "0", "2"], 3));
        assert_eq!(pstate_limit("SPSel"), Some(15));
        assert_eq!(pstate_limit("pm"), Some(1));
        assert_eq!(pstate_limit("tpidr_el0"), None);
    }
}
