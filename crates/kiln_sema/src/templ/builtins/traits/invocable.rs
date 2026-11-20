//! GCC's `__is_invocable(F, Args...)` and `__is_nothrow_invocable(F,
//! Args...)` (WP-1667; spec/sema.md §5.4): whether
//! `INVOKE(declval<F>(), declval<Args>()...)` ([func.require]/1) is
//! well-formed when treated as an unevaluated operand, and for the second
//! form also known not to throw ([meta.rel]). The call is built in a
//! SFINAE context, as `__is_constructible`'s initialization is.
//! `__has_builtin` answers 0 for both (as clang 22, which lacks them), so
//! libstdc++ keeps its library `std::is_invocable` at every `-std`.
//!
//! Also clang's `__builtin_invoke(f, args...)` (WP-1740), the same
//! `INVOKE` on the written arguments, which libc++ 22's `std::__invoke`
//! uses.

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

    /// `__builtin_invoke(f, t1, ..., tN)` with callee `callee` (clang 21
    /// `BuiltinInvoke`): the expression `INVOKE(f, t1, ..., tN)`
    /// ([func.require]/1) denotes, which replaces the call: for a
    /// pointer to member function `f` of class `T`, `(t1.*f)(t2, ...,
    /// tN)` when `t1`'s class is `T` or derived from it, `(t1.get().*f)(t2,
    /// ..., tN)` for a `std::reference_wrapper`, else `((*t1).*f)(t2, ...,
    /// tN)`; for a pointer to data member the same object with `.*f`
    /// (exactly two arguments); otherwise `f(t1, ..., tN)`. The argument
    /// count errors are clang's at the call. `None` after a diagnostic.
    pub(crate) fn builtin_invoke(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<ExprId> {
        let loc = self.begin_loc(callee);
        let too_few = |s: &mut Self, n: usize| {
            let msg = s.diag(DiagId::sema_call_too_few_args_at_least, loc).arg(n).arg(args.len());
            s.emit(msg);
        };
        let Some((&f, rest)) = args.split_first() else {
            too_few(self, 1);
            return None;
        };
        let fty = self.ety(f);
        let TypeView::MemberPointer { pointee, class } = self.ast.types.canonical_view(fty) else {
            return Some(self.act_on_call(f, rest, loc, rparen));
        };
        let Some((&obj, rest)) = rest.split_first() else {
            too_few(self, 2);
            return None;
        };
        let is_fn = self.ast.types.is_function(pointee);
        if !is_fn && !self.builtin_arg_count(args, 2, rparen) {
            return None;
        }
        let obj = self.invoke_object_expr(obj, QualType::unqual(class), loc);
        if self.is_err_expr(obj) {
            return None;
        }
        let m = self.act_on_binary(BinaryOp::PtrMemD, obj, f, loc);
        if !is_fn || self.is_err_expr(m) {
            return Some(m);
        }
        let m = self.act_on_paren(m, loc, loc);
        Some(self.act_on_call(m, rest, loc, rparen))
    }

    /// The object operand of [`Sema::builtin_invoke`]: `t1` itself,
    /// `t1.get()` or `*t1`, as [`Sema::invoke_object`] chooses for a type.
    fn invoke_object_expr(&mut self, t1: ExprId, class: QualType, loc: Loc) -> ExprId {
        let bare = crate::conv::strip_cv(self, self.ety(t1));
        let at = self.begin_loc(t1);
        if let (Some(c), Some(r)) =
            (self.ast.types.as_record(class), self.ast.types.as_record(bare))
        {
            let same = self.ast.first(c) == self.ast.first(r);
            if same || (self.require_complete_silent(bare, at) && is_derived_from(self, r, c)) {
                return t1;
            }
            if self.reference_wrapper_arg(r).is_some() {
                let get = self.intern("get");
                let m = self.act_on_member(t1, get, loc, loc, false);
                if self.is_err_expr(m) {
                    return m;
                }
                return self.act_on_call(m, &[], loc, loc);
            }
        }
        self.act_on_unary(UnaryOp::Deref, t1, loc)
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
