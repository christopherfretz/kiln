//! GNU `__null` (spec/sema.md §5.8): the integer null pointer constant
//! glibc's and clang's `<stddef.h>` define `NULL` as, with clang's two
//! warnings about using it as an integer: `-Wnull-conversion` (an
//! implicit conversion to a non-pointer scalar type, which also covers
//! `nullptr`) and
//! `-Wnull-arithmetic` (an operand of an arithmetic or comparison
//! operator).

use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::types::BuiltinType;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::{ExpansionKind, Loc};

use crate::Sema;

impl Sema<'_, '_> {
    /// GNU `__null`: an integer null pointer constant whose type is the
    /// signed integer type as wide as a data pointer (`long` on LP64, as
    /// clang's `ActOnGNUNullExpr`).
    pub fn act_on_gnu_null(&mut self, loc: Loc) -> ExprId {
        let pw = u16::from(self.target().pointer_bytes);
        let size = |s: &Self, b: BuiltinType| s.target().builtin_size_align(b).0;
        let ty = if pw == size(self, BuiltinType::Int) {
            QualType::INT
        } else if pw != size(self, BuiltinType::Long) && pw == size(self, BuiltinType::LongLong) {
            QualType::LONGLONG
        } else {
            QualType::LONG
        };
        self.builder().gnu_null(ty, loc)
    }

    /// clang's `DiagnoseNullConversion` for `__null` or an expression of
    /// type `std::nullptr_t` (`src`, implicit casts and parentheses
    /// stripped; `!nullptr`, `nullptr && b`, WP-851) implicitly converted
    /// to `to`: a
    /// warning unless `to` is `__null`'s own type, a pointer, a pointer to
    /// member, `std::nullptr_t` or not a scalar type. Reported at
    /// `src`, or at the `NULL` that expanded to `__null`; nothing when that
    /// location and the context `cc` (both taken out of macro arguments)
    /// are in different files or macro expansions.
    pub(crate) fn check_null_conversion(&mut self, src: ExprId, to: QualType, cc: Loc) {
        let from = self.ast.exprs.get(src).ty();
        let canon = |s: &Self, q: QualType| s.ast.types.canonical(q).without_quals();
        if canon(self, from) == canon(self, to)
            || self.ast.types.is_dependent(to)
            || !self.is_scalar(to)
            || self.ast.types.is_pointer(to)
            || self.is_member_pointer(to)
            || self.is_nullptr_type(to)
        {
            return;
        }
        let sm = self.diags.sm();
        let mut loc = self.top_macro_caller_loc(self.ast.exprs.get(src).loc());
        let cc = self.top_macro_caller_loc(cc);
        let gnu = self.ast.exprs.get(src).kind() == ExprKind::GNUNullExpr;
        if gnu
            && let Some(x) = sm.expansion_info(loc)
            && let ExpansionKind::Macro { name } = x.kind
            && sm.span_text(name) == Some("NULL")
        {
            loc = x.expansion.lo;
        }
        if sm.entry_bounds(loc) != sm.entry_bounds(cc) {
            return;
        }
        let what = if gnu { "NULL" } else { "nullptr" };
        let d = self.diag(DiagId::sema_null_conversion, loc).arg(what).arg(self.ty_arg(to));
        self.emit(d);
    }

    /// clang's `SourceManager::getTopMacroCallerLoc`: out of macro
    /// arguments to where the argument was written.
    fn top_macro_caller_loc(&self, mut loc: Loc) -> Loc {
        let sm = self.diags.sm();
        for _ in 0..256 {
            match sm.expansion_info(loc) {
                Some(x) if x.kind == ExpansionKind::MacroArg => {
                    loc = sm.immediate_spelling_loc(loc);
                }
                _ => break,
            }
        }
        loc
    }

    /// clang's `checkArithmeticNull`: `__null` as an operand of the
    /// built-in arithmetic, shift, bitwise or (`compare`) comparison
    /// operator `op` at `loc`. An arithmetic use always warns; a
    /// comparison warns when only one side is `__null` and the other is
    /// neither a pointer nor an array or function. Nothing when the other
    /// operand is a pointer to member or a function.
    pub(crate) fn check_arithmetic_null(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) {
        use BinaryOp as B;
        let compare = matches!(op, B::LT | B::GT | B::LE | B::GE | B::EQ | B::NE | B::Cmp);
        let arith = matches!(
            op,
            B::Add | B::Sub | B::Mul | B::Div | B::Rem | B::Shl | B::Shr | B::And | B::Xor | B::Or
        ) || op.is_compound_assignment();
        if !compare && !arith {
            return;
        }
        let is_null = |s: &Self, e: ExprId| {
            let e = s.strip_impcast(e);
            s.ast.exprs.get(e).kind() == ExprKind::GNUNullExpr
        };
        let (ln, rn) = (is_null(self, l), is_null(self, r));
        if !ln && !rn {
            return;
        }
        let other = if ln { self.ety(r) } else { self.ety(l) };
        if self.is_member_pointer(other) || self.ast.types.is_function(other) {
            return;
        }
        if !compare {
            let mut d = self.diag(DiagId::sema_null_in_arithmetic, loc);
            for (null, e) in [(ln, l), (rn, r)] {
                if null {
                    d = d.range(self.expr_range(e));
                }
            }
            self.emit(d);
            return;
        }
        if ln == rn || self.ast.types.is_pointer(other) || self.ast.types.is_array(other) {
            return;
        }
        let d = self
            .diag(DiagId::sema_null_in_comparison, loc)
            .arg(if ln { "1" } else { "0" })
            .arg(self.ty_arg(other))
            .range(self.expr_range(l))
            .range(self.expr_range(r));
        self.emit(d);
    }
}
