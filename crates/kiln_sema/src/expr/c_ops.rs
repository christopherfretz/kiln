//! C-only expression semantics (WP-1285): the C-style casts between
//! pointers and integers (C17 6.3.2.3p5-6, 6.5.4), the equality and
//! relational operators on pointers (C17 6.5.8, 6.5.9) and the
//! conditional operator (C17 6.5.15), with the GNU leniencies clang and
//! GCC accept by default in C: a constraint violation such as comparing
//! distinct pointer types or a pointer with an integer is a warning, and
//! the operands are converted the way clang's `CheckCompareOperands` and
//! `CheckConditionalOperands` (C path) convert them.
//!
//! Every entry point is called only when `Sema::is_c()`.

use kiln_ast::expr::{BinaryOp, CastKind};
use kiln_ast::types::TypeView;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::{CastStyle, CondOps};
use crate::Sema;
use crate::conv;
use crate::conv::ArithConvKind;
use crate::conv::c_assign;
use crate::type_diff::DiffArgs as _;

impl Sema<'_, '_> {
    /// The second of two type arguments: clang's
    /// `ConvertTypeToDiagnosticString` gives no `aka` to a type already
    /// printed in the same diagnostic.
    fn second_ty_arg(&self, first: QualType, second: QualType) -> String {
        if first == second { format!("'{}'", self.ty_str(second)) } else { self.ty_arg(second) }
    }

    /// A C-style cast between a pointer and a non-pointer scalar (clang
    /// `CheckCStyleCast`): a pointer to an integer narrower than a pointer
    /// warns `-Wpointer-to-int-cast` (C17 6.3.2.3p6: implementation-defined,
    /// not an error), a non-constant integer to a wider pointer warns
    /// `-Wint-to-pointer-cast`, and a floating operand or target is an
    /// error. `x` is the converted operand, `e` the operand as written.
    /// `None` when neither or both types are pointers, or the other type is
    /// not arithmetic (the generic cast checks apply).
    pub(super) fn c_pointer_int_cast(
        &mut self,
        x: ExprId,
        e: ExprId,
        to: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> Option<ExprId> {
        let from = self.ety(x);
        let (fp, tp) = (self.ast.types.is_pointer(from), self.ast.types.is_pointer(to));
        if fp == tp {
            return None;
        }
        let pw = u32::try_from(self.abi.pointer_size * 8).unwrap_or(64);
        let floating = |s: &Self, q: QualType| {
            s.ast.types.is_floating(q) || s.ast.types.as_complex(q).is_some()
        };
        if tp {
            if floating(self, from) {
                let at = self.begin_loc(e);
                let msg = self
                    .diag(DiagId::sema_c_cast_non_int_to_pointer, at)
                    .arg(self.ty_arg(from))
                    .range(self.expr_range(e));
                self.emit(msg);
                return Some(self.recover(to, &[e], loc));
            }
            if !self.is_integral_or_enum(from) {
                return None;
            }
            // clang `checkIntToPointerCast`: not from `_Bool`, an
            // enumeration or an integer constant expression.
            let narrow = conv::int_width(self, from).is_some_and(|w| w < pw);
            if narrow
                && self.ast.types.builtin(from) != Some(kiln_ast::types::BuiltinType::Bool)
                && self.ast.types.as_enum(from).is_none()
                && !c_assign::is_c_ice(self, x)
            {
                let void = self.ast.types.pointee(to).is_some_and(|p| self.ast.types.is_void(p));
                let id = if void {
                    DiagId::sema_c_int_to_void_pointer_cast
                } else {
                    DiagId::sema_c_int_to_pointer_cast
                };
                let msg = self
                    .diag(id, loc)
                    .arg(self.ty_arg(from))
                    .arg(self.ty_arg(to))
                    .range(self.expr_range(e));
                self.emit(msg);
            }
            let ck = CastKind::IntegralToPointer;
            return Some(self.explicit_cast(CastStyle::CStyle, ck, x, to, loc, rparen));
        }
        if floating(self, to) {
            let at = self.begin_loc(e);
            let msg = self
                .diag(DiagId::sema_c_cast_pointer_to_non_int, at)
                .arg(self.ty_arg(to))
                .range(self.expr_range(e));
            self.emit(msg);
            return Some(self.recover(to, &[e], loc));
        }
        if !self.is_integral_or_enum(to) {
            return None;
        }
        let is_bool = self.ast.types.builtin(to) == Some(kiln_ast::types::BuiltinType::Bool);
        if !is_bool && conv::int_width(self, to).is_some_and(|w| w < pw) {
            let void = self.ast.types.pointee(from).is_some_and(|p| self.ast.types.is_void(p));
            let en = self.ast.types.as_enum(to).is_some();
            let id = match (void, en) {
                (true, true) => DiagId::sema_c_void_pointer_to_enum_cast,
                (true, false) => DiagId::sema_c_void_pointer_to_int_cast,
                (false, true) => DiagId::sema_c_pointer_to_enum_cast,
                (false, false) => DiagId::sema_c_pointer_to_int_cast,
            };
            let msg = self
                .diag(id, loc)
                .arg(self.ty_arg(from))
                .arg(self.ty_arg(to))
                .range(self.expr_range(e));
            self.emit(msg);
        }
        let ck = if is_bool { CastKind::PointerToBoolean } else { CastKind::PointerToIntegral };
        Some(self.explicit_cast(CastStyle::CStyle, ck, x, to, loc, rparen))
    }

    /// An equality or relational operator with a pointer operand in C
    /// (clang `CheckCompareOperands`, C path), `l2` and `r2` the decayed
    /// operands of `l` and `r`. `None` leaves the operands to the common
    /// checks (a `nullptr_t` operand, a non-integer other operand).
    pub(super) fn c_pointer_comparison(
        &mut self,
        op: BinaryOp,
        (l, r): (ExprId, ExprId),
        (l2, r2): (ExprId, ExprId),
        loc: Loc,
    ) -> Option<ExprId> {
        let (lt, rt) = (self.ety(l2), self.ety(r2));
        let ordered = !matches!(op, BinaryOp::EQ | BinaryOp::NE);
        let (lp, rp) = (self.ast.types.is_pointer(lt), self.ast.types.is_pointer(rt));
        let warn = |s: &mut Self, id: DiagId| {
            let msg = s
                .diag(id, loc)
                .arg(s.ty_arg(lt))
                .arg(s.second_ty_arg(lt, rt))
                .range(s.expr_range(l))
                .range(s.expr_range(r));
            s.emit(msg);
        };
        if lp && rp {
            let lnull = c_assign::null_pointer_constant(self, l2).is_some();
            let rnull = c_assign::null_pointer_constant(self, r2).is_some();
            let lpe = self.ast.types.pointee(lt).unwrap_or(QualType::ERROR);
            let rpe = self.ast.types.pointee(rt).unwrap_or(QualType::ERROR);
            let (lpe, rpe) = (self.ast.types.canonical(lpe), self.ast.types.canonical(rpe));
            let (lf, rf) = (self.ast.types.is_function(lpe), self.ast.types.is_function(rpe));
            if ordered && lf && rf {
                warn(self, DiagId::sema_c_ordered_comparison_function_pointers);
            }
            if c_assign::compatible(self, lpe.without_quals(), rpe.without_quals()) {
                // C17 6.5.8p2: compatible object types, qualifiers aside.
            } else if !ordered && (self.ast.types.is_void(lpe) || self.ast.types.is_void(rpe)) {
                // C17 6.5.9p2: a pointer to void; against a function
                // pointer only a null pointer constant is standard.
                if (lf || rf) && !lnull && !rnull {
                    warn(self, DiagId::sema_c_comparison_fptr_to_void);
                }
            } else {
                warn(self, DiagId::sema_c_comparison_distinct_pointers);
            }
            let (mut l3, mut r3) = (l2, r2);
            if lpe != rpe {
                if lnull && !rnull {
                    l3 = conv::cast_to(self, CastKind::BitCast, l2, rt, None);
                } else {
                    r3 = conv::cast_to(self, CastKind::BitCast, r2, lt, None);
                }
            }
            let ty = self.truth_ty();
            return Some(self.builder().binary(op, l3, r3, ty, loc));
        }
        let int = |s: &Self, q: QualType| s.is_integral_or_enum(q);
        if !((lp && int(self, rt)) || (rp && int(self, lt))) {
            return None;
        }
        let (ie, pt) = if lp { (r2, lt) } else { (l2, rt) };
        let null = c_assign::null_pointer_constant(self, ie).is_some();
        if null {
            if ordered {
                warn(self, DiagId::sema_c_ordered_comparison_pointer_zero);
            }
        } else if ordered {
            warn(self, DiagId::sema_c_ordered_comparison_pointer_integer);
        } else {
            warn(self, DiagId::sema_c_comparison_pointer_integer);
        }
        let ck = if null { CastKind::NullToPointer } else { CastKind::IntegralToPointer };
        let ie = conv::cast_to(self, ck, ie, pt, None);
        let (l3, r3) = if lp { (l2, ie) } else { (ie, r2) };
        let ty = self.truth_ty();
        Some(self.builder().binary(op, l3, r3, ty, loc))
    }

    /// `c ? t : f` in C (C17 6.5.15; clang `CheckConditionalOperands`, C
    /// path): the operands undergo the usual unary conversions and the
    /// result is always a prvalue. `None` for vector operands (the common
    /// vector rules apply).
    pub(super) fn c_conditional(&mut self, o: CondOps) -> Option<ExprId> {
        let CondOps { c, c2, t, f, loc } = o;
        let (tt0, ft0) = (self.ety(t), self.ety(f));
        if self.is_vector_type(tt0) || self.is_vector_type(ft0) {
            return None;
        }
        // 6.5.15p3, p5: arithmetic operands, the usual arithmetic
        // conversions (`b ? s1 : s2` is `int`).
        if self.is_arith_operand(tt0) && self.is_arith_operand(ft0) {
            return Some(
                match conv::usual_arithmetic_conversions_kind(
                    self,
                    t,
                    f,
                    ArithConvKind::Conditional,
                ) {
                    Ok((t2, f2, ty)) => {
                        self.report_arith_warning(loc);
                        let t2 = self.real_to_complex(t2, ty);
                        let f2 = self.real_to_complex(f2, ty);
                        self.builder().conditional(c2, t2, f2, ty, loc)
                    }
                    Err(err) => self.conv_failed(&err, t, f, loc),
                },
            );
        }
        let t2 = conv::usual_unary_conversions(self, t);
        let f2 = conv::usual_unary_conversions(self, f);
        let (tt, ft) = (self.ety(t2), self.ety(f2));
        // 6.5.15p3, p5: the same structure or union type, unqualified.
        if self.ast.types.as_record(tt).is_some() && self.ast.types.same_unqualified(tt, ft) {
            let ty = conv::strip_cv(self, tt);
            return Some(self.builder().conditional(c2, t2, f2, ty, loc));
        }
        // 6.5.15p5: both `void`; one `void` side is a GNU extension.
        let (tv, fv) = (self.ast.types.is_void(tt), self.ast.types.is_void(ft));
        if tv || fv {
            if !(tv && fv) {
                let side = if tv { t2 } else { f2 };
                let at = self.begin_loc(side);
                let msg = self.diag(DiagId::sema_c_cond_one_void, at).range(self.expr_range(side));
                self.emit(msg);
            }
            let ty = if fv && !tv { ft } else { tt };
            let t3 = conv::cast_to(self, CastKind::ToVoid, t2, ty, None);
            let f3 = conv::cast_to(self, CastKind::ToVoid, f2, ty, None);
            return Some(self.builder().conditional(c2, t3, f3, ty, loc));
        }
        // C23 6.5.15p7: both `nullptr_t`.
        if self.is_nullptr_type(tt) && self.is_nullptr_type(ft) {
            return Some(self.builder().conditional(c2, t2, f2, tt, loc));
        }
        // 6.5.15p6: a null pointer constant takes the other pointer type.
        let (tp, fp) = (self.ast.types.is_pointer(tt), self.ast.types.is_pointer(ft));
        if tp && c_assign::null_pointer_constant(self, f2).is_some() {
            let f3 = conv::cast_to(self, CastKind::NullToPointer, f2, tt, None);
            return Some(self.builder().conditional(c2, t2, f3, tt, loc));
        }
        if fp && c_assign::null_pointer_constant(self, t2).is_some() {
            let t3 = conv::cast_to(self, CastKind::NullToPointer, t2, ft, None);
            return Some(self.builder().conditional(c2, t3, f2, ft, loc));
        }
        if tp && fp {
            let (t3, f3, ty) = self.c_cond_pointers(t2, f2, loc);
            return Some(self.builder().conditional(c2, t3, f3, ty, loc));
        }
        // GNU: a pointer and an integer give the pointer type, with a
        // warning (null pointer constants were taken above).
        let int = |s: &Self, q: QualType| s.is_integral_or_enum(q);
        if (tp && int(self, ft)) || (fp && int(self, tt)) {
            let msg = self
                .diag(DiagId::sema_c_cond_pointer_integer_mismatch, loc)
                .arg(self.ty_arg(tt))
                .arg(self.second_ty_arg(tt, ft))
                .range(self.expr_range(t2))
                .range(self.expr_range(f2));
            self.emit(msg);
            let (t3, f3, ty) = if tp {
                (t2, conv::cast_to(self, CastKind::IntegralToPointer, f2, tt, None), tt)
            } else {
                (conv::cast_to(self, CastKind::IntegralToPointer, t2, ft, None), f2, ft)
            };
            return Some(self.builder().conditional(c2, t3, f3, ty, loc));
        }
        let msg = self.diag(DiagId::sema_incompatible_cond_operands, loc).diff_args(self, tt, ft);
        self.emit(msg);
        Some(self.recover_err(&[c, t, f], loc))
    }

    /// Two pointer operands of `?:` in C (C17 6.5.15p6; clang
    /// `checkConditionalObjectPointersCompatibility`): against a pointer
    /// to void, a pointer to (qualified) void with both operands'
    /// qualifiers; pointers to compatible types, a pointer to their
    /// composite type with the union of the qualifiers; otherwise a
    /// `pointer type mismatch` warning and `void *`.
    fn c_cond_pointers(&mut self, t: ExprId, f: ExprId, loc: Loc) -> (ExprId, ExprId, QualType) {
        let (tt, ft) = (self.ety(t), self.ety(f));
        let lpe = self.pointee_sugared(tt).unwrap_or(QualType::ERROR);
        let rpe = self.pointee_sugared(ft).unwrap_or(QualType::ERROR);
        let (lq, rq) = (self.ast.types.effective_quals(lpe), self.ast.types.effective_quals(rpe));
        let (lv, rv) = (self.ast.types.is_void(lpe), self.ast.types.is_void(rpe));
        let (lf, rf) = (self.ast.types.is_function(lpe), self.ast.types.is_function(rpe));
        if lv && !rf {
            let dest = self.ast.types.qualify(lpe, rq);
            let dest = self.ast.types.pointer_to(dest);
            let t3 = conv::cast_to(self, CastKind::NoOp, t, dest, None);
            let f3 = conv::cast_to(self, CastKind::BitCast, f, dest, None);
            return (t3, f3, dest);
        }
        if rv && !lf {
            let dest = self.ast.types.qualify(rpe, lq);
            let dest = self.ast.types.pointer_to(dest);
            let t3 = conv::cast_to(self, CastKind::BitCast, t, dest, None);
            let f3 = conv::cast_to(self, CastKind::NoOp, f, dest, None);
            return (t3, f3, dest);
        }
        let (lu, ru) = (conv::strip_cv(self, lpe), conv::strip_cv(self, rpe));
        let ty = if c_assign::compatible(self, lu, ru) {
            let comp = self.c_composite(lu, ru);
            let comp = self.ast.types.qualify(comp, lq | rq);
            self.ast.types.pointer_to(comp)
        } else {
            let msg = self
                .diag(DiagId::sema_c_cond_incompatible_pointers, loc)
                .arg(self.ty_arg(tt))
                .arg(self.second_ty_arg(tt, ft))
                .range(self.expr_range(t))
                .range(self.expr_range(f));
            self.emit(msg);
            self.ast.types.pointer_to(QualType::VOID)
        };
        let t3 = conv::cast_to(self, CastKind::BitCast, t, ty, None);
        let f3 = conv::cast_to(self, CastKind::BitCast, f, ty, None);
        (t3, f3, ty)
    }

    /// The composite type of compatible types `a` and `b` (C17 6.2.7p3),
    /// at the outermost level: an array with a known size over one
    /// without, a prototype over a function without one; otherwise `a`.
    fn c_composite(&self, a: QualType, b: QualType) -> QualType {
        let types = &self.ast.types;
        if types.canonical(a) == types.canonical(b) {
            return a;
        }
        match (types.canonical_view(a), types.canonical_view(b)) {
            (TypeView::IncompleteArray(_), TypeView::ConstantArray(_)) => b,
            (TypeView::Function(f), TypeView::Function(g))
                if f.info().no_proto() && !g.info().no_proto() =>
            {
                b
            }
            _ => a,
        }
    }
}
