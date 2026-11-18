//! GCC's `__integer_pack(N)` (WP-1667; spec/sema.md §5.4): the built-in
//! pack behind libstdc++'s `std::make_integer_sequence` when
//! `__make_integer_seq` is missing. In a template, `__integer_pack(N)` is
//! an `int` prvalue naming an unexpanded pack whose elements are `0, 1,
//! ..., N-1`; a pack expansion over a pattern that contains it expands
//! to one element per value. Only the node's own substitution does any
//! work: nothing changes for a pattern that does not name it.

use kiln_ast::expr::ExprDeps;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use super::builtin_templates::MAX_SEQ_LEN;
use crate::templ::pack::PackName;
use crate::templ::subst::{SubstFail, SubstResult, Substituter};
use crate::{Sema, conv};

impl Sema<'_, '_> {
    /// `__integer_pack ( n )` at `loc`. Outside a template it is an error
    /// (GCC's "use of built-in parameter pack outside of a template");
    /// `n` must have integral or unscoped enumeration type, checked here
    /// when it is not type-dependent and its value at instantiation.
    pub fn act_on_integer_pack(&mut self, n: ExprId, loc: Loc, rparen: Loc) -> ExprId {
        if self.is_err_expr(n) {
            return n;
        }
        if !(self.in_template() || self.is_dependent_decl_context(self.ctx)) {
            self.error(DiagId::sema_integer_pack_outside_template, loc);
            return self.recover_err(&[n], loc);
        }
        if !self.ast.exprs.get(n).deps().contains(ExprDeps::TYPE) && !self.integer_pack_type_ok(n) {
            return self.recover_err(&[n], loc);
        }
        self.builder().integer_pack_expr(n, QualType::INT, loc, rparen)
    }

    /// Whether `n`'s type is integral or an unscoped enumeration (else
    /// `sema_ice_not_integral`, as an integral constant expression's).
    fn integer_pack_type_ok(&mut self, n: ExprId) -> bool {
        let ty = self.ast.exprs.get(n).ty();
        let t = &self.ast.types;
        if t.is_integral(ty)
            || t.as_enum(ty).is_some_and(|en| !self.is_scoped_enum(en))
            || self.is_error_type(ty)
        {
            return true;
        }
        let msg = self
            .diag(DiagId::sema_ice_not_integral, self.begin_loc(n))
            .arg(self.ty_arg(ty))
            .range(self.expr_range(n));
        self.emit(msg);
        false
    }

    /// The length of `__integer_pack(n)` for the non-dependent `n`: an
    /// integral constant expression in `0..=MAX_SEQ_LEN` (the limit of
    /// `__make_integer_seq`), taken in its own type (GCC converts it to
    /// `int` first). `None` after a diagnostic.
    fn integer_pack_length(&mut self, n: ExprId) -> Option<u32> {
        if self.is_err_expr(n) || !self.integer_pack_type_ok(n) {
            return None;
        }
        let at = self.begin_loc(n);
        let c = conv::decay(self, n);
        let errors = self.diags.error_count();
        let c = self.act_on_constant_full_expr(c);
        if self.diags.error_count() > errors {
            return None;
        }
        let v = match self.eval_int(c, EvalContext::new(EvalKind::Other, at)) {
            Ok(v) => v,
            Err(mut f) => {
                let at = Self::fold_lone_invalid(&mut f, at);
                let d = self.not_ice_diag(DiagId::sema_expr_not_ice, at);
                self.emit(Self::with_failure_notes(d, f));
                return None;
            }
        };
        if v < 0 {
            self.error(DiagId::sema_integer_seq_negative_length, at);
            return None;
        }
        if v > MAX_SEQ_LEN {
            let msg = self
                .diag(DiagId::sema_integer_seq_too_long, at)
                .arg(v.to_string())
                .arg(MAX_SEQ_LEN.to_string());
            self.emit(msg);
            return None;
        }
        u32::try_from(v).ok()
    }
}

impl Substituter<'_, '_, '_> {
    /// Substitutes `__integer_pack(N)` (`e`): a still dependent `N` keeps
    /// the node (a retained level); otherwise the built-in pack of
    /// length `N` is reached like a pack argument and the element is the
    /// `int` literal of the current index.
    pub(crate) fn integer_pack(&mut self, e: ExprId, loc: Loc, end: Loc) -> SubstResult<ExprId> {
        let len = match self.pack.integer_pack {
            Some((x, len)) if x == e && !self.pack.probing => len,
            _ => {
                let o = self.sema.ast.exprs.integer_pack_operand(e);
                let n = self.expr(o)?;
                let deps = self.sema.ast.exprs.get(n).deps();
                if deps.intersects(ExprDeps::TYPE_VALUE_INST)
                    || self.sema.expr_is_value_dependent(n)
                {
                    return Ok(self.sema.builder().integer_pack_expr(n, QualType::INT, loc, end));
                }
                let len = self.sema.integer_pack_length(n).ok_or(SubstFail)?;
                if !deps.contains(ExprDeps::UNEXPANDED_PACK) {
                    self.pack.integer_pack = Some((e, len));
                }
                len
            }
        };
        let name = |_: &Self| PackName { name: "__integer_pack".to_owned(), ..PackName::default() };
        let i = self.pack_reached(len, name, loc)?;
        Ok(self.int_literal(QualType::INT, i128::from(i), loc))
    }
}
