//! GCC's `__is_invocable(F, Args...)` and `__is_nothrow_invocable(F,
//! Args...)` (WP-1667; spec/sema.md §5.4): whether
//! `INVOKE(declval<F>(), declval<Args>()...)` ([func.require]/1) is
//! well-formed when treated as an unevaluated operand, and for the second
//! form also known not to throw ([meta.rel]). The call is built in a
//! SFINAE context, as `__is_constructible`'s initialization is.
//! `__has_builtin` answers 0 for both (as clang 22, which lacks them), so
//! libstdc++ keeps its library `std::is_invocable` at every `-std`.

use kiln_ast::expr::{BinaryOp, TypeTrait, UnaryOp, ValueCategory};
use kiln_ast::types::{ArgView, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::bases::is_derived_from;
use crate::templ::stack::FrameKind;

impl Sema<'_, '_> {
    /// `__is_invocable` / `__is_nothrow_invocable` (`t`) of `args`
    /// (`F` first). Every operand must be complete, `void` or an array of
    /// unknown bound (GCC's "incomplete type" error); `None` after it.
    pub(super) fn eval_invocable(
        &mut self,
        t: TypeTrait,
        args: &[(QualType, Loc)],
        loc: Loc,
    ) -> Option<bool> {
        for &(q, at) in args {
            let tys = &self.ast.types;
            if tys.is_void(q)
                || tys.is_reference(q)
                || tys.is_function(q)
                || matches!(tys.canonical_view(q), TypeView::IncompleteArray(_))
            {
                continue;
            }
            let e = self.base_element(q);
            if !self.trait_complete(e, at) {
                self.trait_incomplete(DiagId::sema_incomplete_type_trait, at, e);
                return None;
            }
        }
        if args.iter().any(|&(q, _)| self.ast.types.is_void(q) || !self.declval_formable(q)) {
            return Some(false);
        }
        let nothrow = t == TypeTrait::IsNothrowInvocable;
        let tu = self.ast.tu;
        let saved = std::mem::replace(&mut self.ctx, tu);
        let r = self.in_sfinae(FrameKind::ConstraintSubstitution, tu, None, loc, |s| {
            s.enter_unevaluated();
            let e = s.build_invoke(args, loc);
            s.exit_unevaluated();
            e.is_some_and(|e| !(s.is_err_expr(e) || (nothrow && s.can_throw(e))))
        });
        self.ctx = saved;
        Some(matches!(r, Some((true, Ok(())))))
    }

    /// `declval<A>()` ([declval]): an lvalue for an lvalue reference or a
    /// function type, else an xvalue.
    pub(super) fn declval_operand(&mut self, q: QualType, at: Loc) -> ExprId {
        let (ty, vc) = match self.ast.types.canonical_view(q) {
            TypeView::LValueRef(p) => (p, ValueCategory::LValue),
            TypeView::RValueRef(p) if self.ast.types.is_function(p) => (p, ValueCategory::LValue),
            TypeView::RValueRef(p) => (p, ValueCategory::XValue),
            _ if self.ast.types.is_function(q) => (q, ValueCategory::LValue),
            _ => (q, ValueCategory::XValue),
        };
        self.builder().opaque_value_of(ty, vc, at)
    }

    /// `INVOKE(f, t1, ..., tN)` ([func.require]/1) on `declval` operands:
    /// `(t1.*f)(t2, ..., tN)` (or `t1.*f` for a data member, `N == 1`)
    /// when `F` is a pointer to member, else `f(t1, ..., tN)`. `None`
    /// when no form applies.
    fn build_invoke(&mut self, args: &[(QualType, Loc)], loc: Loc) -> Option<ExprId> {
        let (&(fq, fat), rest) = args.split_first()?;
        let f = self.declval_operand(fq, fat);
        let fty = match self.ast.types.canonical_view(fq) {
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => p,
            _ => fq,
        };
        let TypeView::MemberPointer { pointee, class } = self.ast.types.canonical_view(fty) else {
            let ops: Vec<ExprId> =
                rest.iter().map(|&(q, at)| self.declval_operand(q, at)).collect();
            return Some(self.act_on_call(f, &ops, loc, loc));
        };
        let (&(oq, oat), rest) = rest.split_first()?;
        let is_fn = self.ast.types.is_function(pointee);
        if !is_fn && !rest.is_empty() {
            return None;
        }
        let obj = self.invoke_object(oq, oat, QualType::unqual(class));
        if self.is_err_expr(obj) {
            return None;
        }
        let m = self.act_on_binary(BinaryOp::PtrMemD, obj, f, loc);
        if !is_fn || self.is_err_expr(m) {
            return Some(m);
        }
        let m = self.act_on_paren(m, loc, loc);
        let ops: Vec<ExprId> = rest.iter().map(|&(q, at)| self.declval_operand(q, at)).collect();
        Some(self.act_on_call(m, &ops, loc, loc))
    }

    /// The object operand `t1` of a pointer to member of class `class`:
    /// `t1` itself when its class is `class` or derived from it,
    /// `t1.get()` (an lvalue of the wrapped type) for a
    /// `std::reference_wrapper` specialization, else `*t1`.
    fn invoke_object(&mut self, q: QualType, at: Loc, class: QualType) -> ExprId {
        let bare = match self.ast.types.canonical_view(q) {
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => p,
            _ => q,
        };
        let c = self.ast.types.as_record(class);
        if let (Some(c), Some(r)) = (c, self.ast.types.as_record(bare)) {
            let same = self.ast.first(c) == self.ast.first(r);
            if same || (self.require_complete_silent(bare, at) && is_derived_from(self, r, c)) {
                return self.declval_operand(q, at);
            }
            if let Some(inner) = self.reference_wrapper_arg(r) {
                return self.builder().opaque_value_of(inner, ValueCategory::LValue, at);
            }
        }
        let obj = self.declval_operand(q, at);
        self.act_on_unary(UnaryOp::Deref, obj, at)
    }

    /// `T` when class `r` is `std::reference_wrapper<T>`.
    fn reference_wrapper_arg(&self, r: DeclId) -> Option<QualType> {
        let (_, info) = self.ast.spec_info(r)?;
        let name = self.ast.decls.get(info.template).name().as_ident()?;
        if self.ast.symbols().as_str(name) != "reference_wrapper"
            || !self.is_in_std_namespace(info.template)
        {
            return None;
        }
        match self.ast.types.args(info.args).first()?.view() {
            ArgView::Type(t) => Some(t),
            _ => None,
        }
    }
}
