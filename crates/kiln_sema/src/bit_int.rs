//! `_BitInt(N)` / `unsigned _BitInt(N)` (spec/types.md §5.3): building
//! the type from its width expression (clang `Sema::BuildBitIntType`).
//!
//! The C23 bit-precise integer types are a clang extension in C++
//! (`-Wbit-int-extension`, pedantic). The width is an integral constant
//! expression (folding allowed, as clang); a value-dependent width makes
//! a `DependentBitInt` that substitution rebuilds through
//! [`Sema::build_bit_int_type`]. Signed widths are at least 2, unsigned
//! at least 1, and both at most the target's `BITINT_MAXWIDTH`
//! (`kiln_abi::Target::bit_int_max_width`).

use kiln_ast::expr::ExprDeps;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::{Sema, conv};

impl Sema<'_, '_> {
    /// The type `_BitInt(width)` (`unsigned` for `unsigned _BitInt`), size
    /// errors at `loc` (clang: the decl-specifier's start; the keyword
    /// on instantiation). `QualType::ERROR` after a diagnostic.
    pub fn build_bit_int_type(&mut self, unsigned: bool, width: ExprId, loc: Loc) -> QualType {
        if self.is_err_expr(width) {
            return QualType::ERROR;
        }
        let deps = self.ast.exprs.get(width).deps();
        if deps.intersects(ExprDeps::VALUE_INST) || self.expr_is_value_dependent(width) {
            let p = self.dependent_profile(width);
            return self.ast.types.dependent_bit_int(!unsigned, p);
        }
        let Some(n) = self.bit_int_width_value(width) else { return QualType::ERROR };
        if !unsigned && n < 2 {
            let d = self.diag(DiagId::sema_bit_int_bad_size, loc).arg("0");
            self.emit(d);
            return QualType::ERROR;
        }
        if unsigned && n < 1 {
            let d = self.diag(DiagId::sema_bit_int_bad_size, loc).arg("1");
            self.emit(d);
            return QualType::ERROR;
        }
        let max = self.abi.bit_int_max_width;
        if n > u128::from(max) {
            let d = self
                .diag(DiagId::sema_bit_int_max_size, loc)
                .arg(if unsigned { "1" } else { "0" })
                .arg(max.to_string());
            self.emit(d);
            return QualType::ERROR;
        }
        let Ok(n) = u32::try_from(n) else { return QualType::ERROR };
        self.ast.types.bit_int(!unsigned, n)
    }

    /// The value of the non-dependent `_BitInt` width `w` (clang
    /// `VerifyIntegerConstantExpression` with `AllowFold`), zero-extended
    /// from the width of its type as clang's `getZExtValue` (so a
    /// negative width is huge). `None` after a diagnostic.
    fn bit_int_width_value(&mut self, w: ExprId) -> Option<u128> {
        let wt = self.ast.exprs.get(w).ty();
        let t = &self.ast.types;
        if !(t.is_integral(wt)
            || t.as_enum(wt).is_some_and(|en| !self.is_scoped_enum(en))
            || t.as_record(wt).is_some()
            || self.is_error_type(wt))
        {
            let msg = self
                .diag(DiagId::sema_ice_not_integral, self.begin_loc(w))
                .arg(self.ty_arg(wt))
                .range(self.expr_range(w));
            self.emit(msg);
            return None;
        }
        let c = conv::decay(self, w);
        let errors = self.diags.error_count();
        let c = self.act_on_constant_full_expr(c);
        if self.diags.error_count() > errors {
            return None;
        }
        let v = match self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(w))) {
            Ok(v) => v,
            Err(f) => self.fold_not_ice(c, f, self.begin_loc(w))?,
        };
        if v >= 0 {
            return u128::try_from(v).ok();
        }
        let ct = self.ast.exprs.get(c).ty();
        let bits = crate::constfold::int_info(&self.ast, ct).map_or(32, |(b, _)| b);
        Some(if bits >= 128 { v.cast_unsigned() } else { v.cast_unsigned() & ((1u128 << bits) - 1) })
    }
}
