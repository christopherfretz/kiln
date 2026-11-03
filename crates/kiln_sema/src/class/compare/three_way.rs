//! The synthesized three-way comparison ([class.spaceship]/1; clang
//! `BuildSynthesizedThreeWayComparison`, spec/sema-class.md §8.3): in a
//! defaulted `operator<=>` whose declared return type `R` is a
//! comparison category, a subobject pair `a`, `b` with no usable `<=>`
//! compares as
//! - `a == b ? R::equal : a < b ? R::less : R::greater` (`strong_ordering`),
//! - with `equivalent` for `equal` (`weak_ordering`),
//! - `a == b ? equivalent : a < b ? less : b < a ? greater : unordered`
//!   (`partial_ordering`).
//!
//! The form is a `PseudoObjectExpr`: syntactically `a <=> b` (a
//! `BinaryOperator` of the result's type), semantically `a` and `b` bound
//! to `OpaqueValueExpr`s (each evaluated once) and the conditional chain
//! over them, an lvalue of the category's `const` constants.

use kiln_ast::class::{CmpCategory, CmpResult};
use kiln_ast::expr::BinaryOp;
use kiln_ast::{DeclId, ExprId};
use kiln_source::Loc;

use super::CmpUsage;
use crate::Sema;

impl Sema<'_, '_> {
    /// `l <=> r` synthesized from `==` and `<` for defaulted `<=>` `f`
    /// (`None` after a diagnostic, or when `f`'s return type is not a
    /// comparison category: the analysis deleted `f` then).
    pub(super) fn synth_three_way(
        &mut self,
        f: DeclId,
        l: ExprId,
        r: ExprId,
        loc: Loc,
    ) -> Option<ExprId> {
        let ret = self.fn_ret(f);
        let cat = self.cmp_category_of(ret)?;
        self.cmp_category_type(cat, loc, CmpUsage::Defaulted)?;
        let ovl = self.builder().opaque_value(l);
        let ovr = self.builder().opaque_value(r);
        let eq = self.binary_operator(BinaryOp::EQ, ovl, ovr, None, loc);
        let eq = self.three_way_ok(eq)?;
        let less = self.binary_operator(BinaryOp::LT, ovl, ovr, None, loc);
        let less = self.three_way_ok(less)?;
        let greater = if cat == CmpCategory::Partial {
            let g = self.binary_operator(BinaryOp::LT, ovr, ovl, None, loc);
            Some(self.three_way_ok(g)?)
        } else {
            None
        };
        let first =
            if cat == CmpCategory::Strong { CmpResult::Equal } else { CmpResult::Equivalent };
        let chain = [
            (Some(eq), first),
            (Some(less), CmpResult::Less),
            (greater, CmpResult::Greater),
            (None, CmpResult::Unordered),
        ];
        let last = if cat == CmpCategory::Partial { 3 } else { 2 };
        let mut result: Option<ExprId> = None;
        for &(cmp, value) in chain[..=last].iter().rev() {
            let var = self.ast.cmp_categories.get(cat).and_then(|i| i.values[value as usize])?;
            // clang builds the references without a location.
            let v = self.build_decl_ref(var, Loc::INVALID);
            let v = self.three_way_ok(v)?;
            result = Some(match (result, cmp) {
                (Some(rest), Some(c)) => {
                    let e = self.act_on_conditional(c, v, rest, loc);
                    self.three_way_ok(e)?
                }
                _ => v,
            });
        }
        let result = result?;
        let (ty, vk) = (self.ety(result), self.ast.exprs.get(result).value_category());
        let syntactic = self.builder().syntactic_binary(BinaryOp::Cmp, l, r, ty, vk, loc);
        Some(self.builder().pseudo_object(syntactic, &[ovl, ovr, result], 2))
    }

    fn three_way_ok(&self, e: ExprId) -> Option<ExprId> {
        (!self.is_err_expr(e)).then_some(e)
    }
}
