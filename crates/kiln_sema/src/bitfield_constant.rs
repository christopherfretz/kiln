//! `-Wbitfield-constant-conversion` (spec/sema.md §7.12): an integral
//! constant stored into a bit-field too narrow to hold its value (clang's
//! `AnalyzeBitFieldAssignment`), by a simple assignment or by the
//! initialization of the bit-field member (`CheckBitFieldInitialization`:
//! aggregate initialization, a mem-initializer, a default member
//! initializer). A value 1 stored into a one-bit wide signed bit-field is
//! reported under its subgroup `-Wsingle-bit-bitfield-constant-conversion`.

use kiln_ast::consts::{IntBits, Scalar};
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
        if fw == 0 {
            return false;
        }
        let v_signed = int_signed(self, vty);
        let unary = list.is_none()
            && on.kind() == ExprKind::UnaryOperator
            && matches!(self.ast.exprs.unary_op(orig), UnaryOp::Minus | UnaryOp::Not);
        if ow >= 128 {
            // `__int128` and wide `_BitInt`s: the same rule on the words.
            let Some(v) = self.try_fold(orig) else { return false };
            let Some(Scalar::Int { bits, .. }) = self.ast.consts.scalar(v) else { return false };
            let words = bits.words().to_vec();
            let fs = int_signed(self, fty);
            let Some((vt, tt, one)) = wide_truncation(&words, ow, v_signed, fw, fs, unary) else {
                return false;
            };
            return self.emit_bit_field_truncation(vty, init, list, at, (vt, tt, one));
        }
        let Some(v) = self.fold_int(orig) else { return false };
        // clang: a negated or complemented constant (or an unsigned one)
        // needs only its significant bits (`APInt::getSignificantBits`).
        if unary && (!v_signed || v < 0) {
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
        self.emit_bit_field_truncation(
            vty,
            init,
            list,
            at,
            (v.to_string(), truncated.to_string(), one_bit),
        )
    }

    /// The `-Wbitfield-constant-conversion` diagnostic of a value of type
    /// `vty` and its truncation (decimal texts; `one_bit`: the subgroup).
    fn emit_bit_field_truncation(
        &mut self,
        vty: kiln_ast::QualType,
        init: ExprId,
        list: Option<ExprId>,
        at: Loc,
        (v, truncated, one_bit): (String, String, bool),
    ) -> bool {
        let id = if one_bit {
            DiagId::sema_single_bit_bitfield_constant_conversion
        } else {
            DiagId::sema_bitfield_constant_conversion
        };
        let range = self.expr_range(list.unwrap_or(init));
        let d = self
            .diag(id, at)
            .arg(self.ty_arg(vty))
            .arg(v)
            .arg(truncated)
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

/// The `ow`-bit value `words` (signedness `vs`) stored into an `fw`-bit
/// bit-field of signedness `fs`, for widths of 128 bits and more (clang
/// `APSInt` arithmetic): `None` when the value survives, else the decimal
/// value, the decimal truncated value and whether it is the one-bit case.
/// `unary`: a negated or complemented operand, which needs only its
/// significant bits.
fn wide_truncation(
    words: &[u64],
    mut ow: u32,
    vs: bool,
    fw: u32,
    fs: bool,
    unary: bool,
) -> Option<(String, String, bool)> {
    let bit = |w: &[u64], i: u32| w[(i / 64) as usize] >> (i % 64) & 1 == 1;
    let orig_w = ow;
    let neg = vs && bit(words, ow - 1);
    if unary && (!vs || neg) {
        // `APInt::getSignificantBits`: width − sign bits + 1.
        let top = bit(words, ow - 1);
        let sign_bits = (0..ow).rev().take_while(|&i| bit(words, i) == top).count() as u32;
        ow = orig_w - sign_bits + 1;
    }
    if ow <= fw {
        return None;
    }
    // Both extended (each by its own signedness) to a common width.
    let n = IntBits::words_for(orig_w.max(fw)) + 1;
    let ext = |w: &[u64], width: u32, signed: bool| -> Vec<u64> {
        let fill = signed && bit(w, width - 1);
        (0..n * 64)
            .step_by(64)
            .map(|b: usize| {
                let mut x = 0u64;
                for j in 0..64u32 {
                    let i = b as u32 + j;
                    if if i < width { bit(w, i) } else { fill } {
                        x |= 1 << j;
                    }
                }
                x
            })
            .collect()
    };
    let mut t: Vec<u64> = words[..IntBits::words_for(fw).min(words.len())].to_vec();
    t.resize(IntBits::words_for(fw), 0);
    if !fw.is_multiple_of(64) {
        let last = t.len() - 1;
        t[last] &= (1u64 << (fw % 64)) - 1;
    }
    if ext(words, orig_w, vs) == ext(&t, fw, fs) {
        return None;
    }
    let (mut vt, mut tt) = (String::new(), String::new());
    IntBits::from_words(orig_w, words)?.write_decimal(vs, &mut vt);
    IntBits::from_words(fw, &t)?.write_decimal(fs, &mut tt);
    let one = fw == 1 && vt == "1";
    Some((vt, tt, one))
}
