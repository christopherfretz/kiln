//! GNU complex types in expressions (spec/sema.md §GNU complex): the
//! `__real` / `__imag` operators, `__builtin_complex`, and the scalar
//! cast of a real arm to a complex result. The conversions live in
//! `conv/` (scs.rs, apply.rs, arith.rs).

use kiln_ast::expr::{CastKind, ExprFlags, UnaryOp};
use kiln_ast::types::BuiltinType;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::init::{InitItem, InitList, InitValue};
use crate::{Sema, conv};

impl Sema<'_, '_> {
    /// `__real e` / `__imag e` (clang's `CheckRealImagOperand`): the
    /// element type of a complex operand, or a real arithmetic operand's
    /// own type. `__real` keeps the operand's value category; `__imag`
    /// does for a complex operand and is a prvalue (zero) otherwise.
    pub(crate) fn real_imag(&mut self, op: UnaryOp, e: ExprId, loc: Loc) -> ExprId {
        let t = self.ety(e);
        if self.ast.types.is_dependent(t) {
            return self.builder().dependent_unary(op, e, loc);
        }
        let ty = match self.ast.types.as_complex(t) {
            Some(el) => el,
            None if conv::rank::is_arithmetic_or_unscoped_enum(self, t) => conv::strip_cv(self, t),
            None => {
                let name = if op == UnaryOp::Real { "__real" } else { "__imag" };
                let msg = self
                    .diag(DiagId::sema_realimag_invalid_type, loc)
                    .arg(self.ty_arg(t))
                    .arg(name);
                self.emit(msg);
                return self.recover_err(&[e], loc);
            }
        };
        self.builder().unary(op, e, ty, loc, ExprFlags::CANNOT_OVERFLOW)
    }

    /// clang's `PrepareScalarCast` from a real arithmetic `e` to the
    /// complex type `to`: the element conversion, then `*RealToComplex`.
    /// Any other `e` (already complex, or `to` not complex) is returned
    /// as is.
    pub(crate) fn real_to_complex(&mut self, e: ExprId, to: QualType) -> ExprId {
        let Some(el) = self.ast.types.as_complex(to) else { return e };
        let from = conv::strip_cv(self, self.ety(e));
        if self.ast.types.as_complex(from).is_some() {
            return e;
        }
        let fl = self.ast.types.is_floating(el);
        let mut e = e;
        if !self.ast.types.same_unqualified(from, el) {
            let ck = match (self.ast.types.is_floating(from), fl) {
                (true, true) => CastKind::FloatingCast,
                (true, false) => CastKind::FloatingToIntegral,
                (false, true) => CastKind::IntegralToFloating,
                (false, false) => CastKind::IntegralCast,
            };
            e = conv::cast_to(self, ck, e, el, None);
        }
        let ck = if fl { CastKind::FloatingRealToComplex } else { CastKind::IntegralRealToComplex };
        conv::cast_to(self, ck, e, to, None)
    }

    /// `__builtin_complex(re, im)` (clang's `BuiltinComplex`): two
    /// operands of the same real floating type `T` give a `_Complex T`.
    pub(crate) fn builtin_complex(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 2, rparen) {
            return None;
        }
        let mut conv_args = Vec::with_capacity(2);
        let mut dependent = false;
        for &a in args {
            let t = self.ety(a);
            if self.ast.types.is_dependent(t) {
                dependent = true;
                conv_args.push(a);
                continue;
            }
            if !self.ast.types.is_floating(t) {
                let msg = self
                    .diag(DiagId::sema_call_requires_real_fp, self.begin_loc(a))
                    .arg(self.ty_arg(t))
                    .range(self.expr_range(a));
                self.emit(msg);
                return None;
            }
            conv_args.push(conv::lvalue_to_rvalue(self, a));
        }
        if dependent {
            return Some((conv_args, QualType::DEPENDENT));
        }
        let (rt, it) = (self.ety(conv_args[0]), self.ety(conv_args[1]));
        if !self.ast.types.same_unqualified(rt, it) {
            let msg = self
                .diag(DiagId::sema_call_different_arg_types, self.begin_loc(conv_args[0]))
                .arg(self.ty_arg(rt))
                .arg(self.ty_arg(it));
            self.emit(msg);
            return None;
        }
        let el = conv::strip_cv(self, rt);
        // clang 19 rejects the half types at the callee (clang 22 accepts
        // them, as it accepts `__bf16`: spec/sema.md "GNU complex").
        let spelled = match self.ast.types.builtin(el) {
            _ if self.clang_compat >= 22 => None,
            Some(BuiltinType::Float16) => Some("_Float16"),
            Some(BuiltinType::Half) => Some("half"),
            _ => None,
        };
        if let Some(name) = spelled {
            let msg =
                self.diag(DiagId::sema_invalid_complex_spec, self.begin_loc(callee)).arg(name);
            self.emit(msg);
            return None;
        }
        Some((conv_args, self.ast.types.complex(el)))
    }

    /// `_Complex T z = {re, im}` (clang's `CheckComplexType`): a list of
    /// two or more clauses initializes the real and imaginary parts, each
    /// converted to `T` as a list element (narrowing is checked); a third
    /// clause is excess. Shorter lists initialize a scalar.
    pub(crate) fn complex_list_init(&mut self, ty: QualType, l: &InitList) -> Option<ExprId> {
        let elem = self.ast.types.as_complex(ty)?;
        let parts: Vec<ExprId> = l.items[..l.items.len().min(2)]
            .iter()
            .filter_map(|i| match i {
                InitItem { value: InitValue::Expr(e), designator } if designator.is_empty() => {
                    Some(*e)
                }
                _ => None,
            })
            .collect();
        if parts.len() != 2 {
            return None;
        }
        let elem = conv::strip_cv(self, elem);
        let inits: Vec<ExprId> = parts
            .into_iter()
            .map(|e| {
                self.check_narrowing(e, elem, elem);
                self.convert_for_init(e, elem, conv::ConvCtx::Initialization)
            })
            .collect();
        if let Some(extra) = l.items.get(2)
            && self.is_c()
        {
            self.excess_c(extra, "2");
        } else if let Some(extra) = l.items.get(2) {
            let at = match &extra.value {
                InitValue::Expr(e) => self.ast.exprs.get(*e).loc(),
                InitValue::List(inner) => inner.lbrace,
            };
            let msg = self.diag(DiagId::sema_excess_initializers, at).arg("2");
            self.emit(msg);
        }
        Some(self.builder().init_list(ty, &inits, None, None, l.lbrace, l.rbrace))
    }
}
