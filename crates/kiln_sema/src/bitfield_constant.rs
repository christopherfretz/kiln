//! `-Wbitfield-constant-conversion` (spec/sema.md §7.12): an integral
//! constant stored into a bit-field too narrow to hold its value (clang's
//! `AnalyzeBitFieldAssignment`), by a simple assignment or by the
//! initialization of the bit-field member (`CheckBitFieldInitialization`:
//! aggregate initialization, a mem-initializer, a default member
//! initializer). A value 1 stored into a one-bit wide signed bit-field is
//! reported under its subgroup `-Wsingle-bit-bitfield-constant-conversion`.

use kiln_ast::expr::{ExprKind, UnaryOp};
use kiln_ast::types::BuiltinType;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::rank::{int_signed, int_width, is_integral_or_unscoped_enum};
use crate::conv::{eval_width, source_bit_field};

impl Sema<'_, '_> {
    /// clang's `AnalyzeBitFieldAssignment` for `lhs = rhs` (located at the
    /// operator, `at`): whether the truncation was diagnosed, in which case
    /// the implicit conversions of `rhs` are not analyzed.
    pub(crate) fn check_bit_field_assignment(&mut self, lhs: ExprId, rhs: ExprId, at: Loc) -> bool {
        let Some(f) = source_bit_field(self, lhs) else { return false };
        self.check_bit_field_value(f, rhs, at)
    }

    /// clang's `CheckBitFieldInitialization`: the initialization of
    /// member `field` by `init` (the converted initializer), located at
    /// `at` (the initialization kind's location). A braced scalar
    /// initializer `{v}` reports `v` at itself first, then the list at `at`
    /// with the bit-field's declared type (the list's type in clang).
    pub(crate) fn check_bit_field_init(&mut self, field: DeclId, init: ExprId, at: Loc) {
        if self.ast.decls.field(field).and_then(|f| f.bit_width).is_none() {
            return;
        }
        let s = self.strip_impcast(init);
        if self.ast.exprs.get(s).kind() == ExprKind::InitListExpr {
            let inits: Vec<ExprId> =
                self.ast.lists.iter(self.ast.exprs.init_list_inits(s)).collect();
            if let [elem] = inits[..] {
                let el = self.begin_loc(elem);
                self.check_bit_field_value(field, elem, el);
                self.bit_field_truncation(field, elem, Some(s), at);
            }
            return;
        }
        self.check_bit_field_value(field, init, at);
    }

    fn check_bit_field_value(&mut self, field: DeclId, init: ExprId, at: Loc) -> bool {
        self.bit_field_truncation(field, init, None, at)
    }

    /// The value of `init` (implicit casts and parentheses stripped) stored
    /// into bit-field `field`; `list` is the braced list whose type the
    /// value takes, if any.
    fn bit_field_truncation(
        &mut self,
        field: DeclId,
        init: ExprId,
        list: Option<ExprId>,
        at: Loc,
    ) -> bool {
        let Some(fty) = self.ast.decls.get(field).ty() else { return false };
        let Some(w) = self.ast.decls.field(field).and_then(|f| f.bit_width) else { return false };
        let Some(fw) = eval_width(self, w) else { return false };
        if self.ast.decls.get(field).has(kiln_ast::decl::DeclFlags::INVALID)
            || self.ast.types.builtin(fty) == Some(BuiltinType::Bool)
            || self.ast.types.is_dependent(fty)
        {
            return false;
        }
        let orig = self.strip_impcast(init);
        let on = *self.ast.exprs.get(orig);
        if !on.deps().is_none() {
            return false;
        }
        // The value's type: the operand's, or the list's.
        let vty = list.map_or(on.ty(), |l| self.ast.exprs.get(l).ty());
        let integral =
            is_integral_or_unscoped_enum(self, vty) || self.ast.types.as_enum(vty).is_some();
        if !integral || int_width(self, vty).is_none() {
            return false;
        }
        let Some(mut ow) = int_width(self, vty) else { return false };
        if ow >= 128 || fw == 0 {
            return false;
        }
        let Some(v) = self.fold_int(orig) else { return false };
        let v_signed = int_signed(self, vty);
        // clang: a negated or complemented constant (or an unsigned one)
        // needs only its significant bits (`APInt::getSignificantBits`).
        if list.is_none()
            && (!v_signed || v < 0)
            && on.kind() == ExprKind::UnaryOperator
            && matches!(self.ast.exprs.unary_op(orig), UnaryOp::Minus | UnaryOp::Not)
        {
            let sh = 128 - ow;
            let sv = (v << sh) >> sh;
            let lz = if sv < 0 { (!sv).leading_zeros() } else { sv.leading_zeros() };
            ow = 129 - lz;
        }
        if ow <= fw {
            return false;
        }
        let truncated = truncate(v, fw, int_signed(self, fty));
        if truncated == v {
            return false;
        }
        let one_bit = fw == 1 && v == 1;
        let id = if one_bit {
            DiagId::sema_single_bit_bitfield_constant_conversion
        } else {
            DiagId::sema_bitfield_constant_conversion
        };
        let range = self.expr_range(list.unwrap_or(init));
        let d = self
            .diag(id, at)
            .arg(self.ty_arg(vty))
            .arg(v.to_string())
            .arg(truncated.to_string())
            .range(range)
            .allow_duplicate();
        self.emit(d);
        true
    }
}

/// The low `w` (< 128) bits of `v`, read as a `w`-bit integer of
/// signedness `signed`.
fn truncate(v: i128, w: u32, signed: bool) -> i128 {
    let sh = 128 - w;
    if signed { (v << sh) >> sh } else { v & ((1i128 << w) - 1) }
}
