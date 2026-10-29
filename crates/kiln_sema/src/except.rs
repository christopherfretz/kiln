//! Exception handling ([except]): exception specifications on function
//! declarators ([except.spec]) and the `noexcept` operator
//! ([expr.unary.noexcept]) (spec/sema.md §5.3).

use kiln_ast::ExprId;
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::types::{BuiltinType, ExcSpec, NoexceptValue};
use kiln_ast::{QualType, expr::ValueCategory};
use kiln_diag::DiagId;
use kiln_oven::EvalContext;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::{self, ConvCtx};

/// How a `noexcept` specifier argument converts to `bool`.
enum BoolCce {
    /// Already `bool`.
    Bool,
    /// An integral or unscoped enumeration type: an integral conversion,
    /// allowed unless narrowing ([expr.const]/12).
    Integral,
    /// A conversion a converted constant expression may not contain
    /// (floating-integral, boolean conversions from pointers, ...).
    NotAllowed,
    /// No implicit conversion to `bool` exists: class types (until
    /// conversion functions, M3) and scoped enumerations.
    NotConvertible,
    /// The error type: already diagnosed; the conversion recovers.
    Other,
}

impl Sema<'_, '_> {
    /// `noexcept ( e )` on a function declarator ([except.spec]/2): `e`
    /// is a contextually converted constant expression of type `bool`,
    /// evaluated by the Oven. Returns `NoexceptExpr` with the
    /// `ConstantExpr`-wrapped operand and its value; after an error the
    /// specification is `noexcept(false)`, as clang recovers. `loc` is
    /// the `noexcept` keyword; `start` the operand's first token, where
    /// its diagnostics point (a qualified name's own location is after
    /// its nested-name-specifier, which keeps no location).
    pub fn act_on_noexcept_spec(&mut self, e: ExprId, loc: Loc, start: Loc) -> ExcSpec {
        if self.is_err_expr(e) {
            return self.invalid_noexcept_spec(loc);
        }
        if self.expr_is_value_dependent(e) {
            let p = self.dependent_profile(e);
            return ExcSpec::NoexceptExpr { expr: e, value: NoexceptValue::Dependent(p) };
        }
        let at = start;
        // Diagnostics name the operand's type as written (clang prints
        // `int *const`, not the converted `int *`).
        let written = self.ast.exprs.get(e).ty();
        let c = conv::lvalue_to_rvalue(self, e);
        let ty = conv::strip_cv(self, self.ast.exprs.get(c).ty());
        match self.bool_cce_kind(ty) {
            BoolCce::Integral => {
                if let Some(v) = self.fold_int(c)
                    && v != 0
                    && v != 1
                {
                    let d = self
                        .diag(DiagId::sema_cce_narrowing_value, at)
                        .arg("noexcept specifier argument")
                        .arg(v.to_string())
                        .arg(self.ty_arg(QualType::BOOL));
                    self.emit(d);
                    return self.invalid_noexcept_spec(loc);
                }
            }
            BoolCce::NotAllowed => {
                let d = self
                    .diag(DiagId::sema_cce_conversion_not_allowed, at)
                    .arg(self.ty_arg(written))
                    .arg(self.ty_arg(QualType::BOOL));
                self.emit(d);
                return self.invalid_noexcept_spec(loc);
            }
            BoolCce::NotConvertible => {
                let d = self
                    .diag(DiagId::sema_cce_not_implicitly_convertible, at)
                    .arg(self.ty_arg(written))
                    .arg(self.ty_arg(QualType::BOOL));
                self.emit(d);
                return self.invalid_noexcept_spec(loc);
            }
            BoolCce::Bool | BoolCce::Other => {}
        }
        let c = match conv::implicit_convert(self, c, QualType::BOOL, ConvCtx::Initialization) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, at);
                return self.invalid_noexcept_spec(loc);
            }
        };
        let c = self.act_on_full_expr(c);
        match self.eval_const(c, EvalContext::converted(at)) {
            Ok(v) => {
                let value = self.const_truth(v).unwrap_or(false);
                let cv = self.int_const(QualType::BOOL, i128::from(value));
                let ce = self.builder().constant_expr(c, Some(cv));
                let value = if value { NoexceptValue::True } else { NoexceptValue::False };
                ExcSpec::NoexceptExpr { expr: ce, value }
            }
            Err(f) => {
                let d = self.diag(DiagId::sema_expr_not_cce, at).arg("noexcept specifier argument");
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                self.invalid_noexcept_spec(loc)
            }
        }
    }

    /// clang's recovery for an invalid `noexcept` argument:
    /// `noexcept(false)`.
    fn invalid_noexcept_spec(&mut self, loc: Loc) -> ExcSpec {
        let f = self.builder().bool_literal(QualType::BOOL, false, loc);
        ExcSpec::NoexceptExpr { expr: f, value: NoexceptValue::False }
    }

    fn bool_cce_kind(&self, ty: QualType) -> BoolCce {
        let t = &self.ast.types;
        if t.builtin(ty) == Some(BuiltinType::Bool) {
            return BoolCce::Bool;
        }
        if t.is_integral(ty) {
            return BoolCce::Integral;
        }
        if let Some(en) = t.as_enum(ty) {
            return if self.is_scoped_enum(en) {
                BoolCce::NotConvertible
            } else {
                BoolCce::Integral
            };
        }
        if t.as_record(ty).is_some() {
            return BoolCce::NotConvertible;
        }
        if self.is_error_type(ty) {
            return BoolCce::Other;
        }
        BoolCce::NotAllowed
    }

    /// `throw ( type-id-list )` ([except.spec], C++14 [except.spec]/1):
    /// `throw()` is the non-throwing `DynamicNone`, accepted silently as
    /// clang does in every mode; a non-empty list was removed in C++17
    /// and is diagnosed (an error by default, `-Wdynamic-exception-spec`).
    /// `ok` is false after a syntax error in the list. `rparen` is the
    /// closing `)`.
    pub fn act_on_dynamic_exception_spec(
        &mut self,
        types: &[QualType],
        ok: bool,
        loc: Loc,
        _rparen: Loc,
    ) -> ExcSpec {
        if types.is_empty() && ok {
            return ExcSpec::DynamicNone;
        }
        let d = self
            .diag(DiagId::sema_dynamic_exception_spec, loc)
            .with_note(self.diag(DiagId::note_use_noexcept_false, loc));
        self.emit(d);
        if !ok {
            return ExcSpec::None;
        }
        ExcSpec::Dynamic(self.ast.types.type_list(types))
    }

    /// `noexcept ( e )` ([expr.unary.noexcept]): a `bool` prvalue, `true`
    /// unless the unevaluated operand `e` is potentially-throwing
    /// ([except.spec]/6; see [`Sema::can_throw`]). `rparen` is the `)`.
    pub fn act_on_noexcept_expr(&mut self, e: ExprId, loc: Loc, rparen: Loc) -> ExprId {
        if self.is_err_expr(e) {
            return self.recover_err(&[e], loc);
        }
        self.check_unevaluated_side_effects(e);
        let value = !self.can_throw(e);
        self.builder().noexcept_expr(e, value, QualType::BOOL, loc, rparen)
    }

    /// Whether `e` is potentially-throwing ([except.spec]/6), clang's
    /// `Sema::canThrow` restricted to the expressions M2 builds:
    ///
    /// - a call whose callee's function type is not non-throwing
    ///   ([except.spec]/6.1); a callee without a prototype counts as
    ///   throwing;
    /// - a `throw` or `new` expression (not produced by the M2 parser);
    /// - a `dynamic_cast` to a reference type that needs a run-time check
    ///   (not produced by the M2 parser);
    /// - any expression with a potentially-throwing immediate
    ///   subexpression ([except.spec]/6.4), except that the unevaluated
    ///   operands of `sizeof`, `alignof` and `noexcept` are not looked at.
    ///
    /// `typeid` of a polymorphic glvalue cannot occur: M2 has no
    /// polymorphic classes. Implicit calls (constructors, destructors,
    /// conversion functions) do not exist in M2 either.
    pub fn can_throw(&self, e: ExprId) -> bool {
        let exprs = &self.ast.exprs;
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let n = exprs.get(x);
            match n.kind() {
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::TypeTraitExpr
                | ExprKind::ArrayTypeTraitExpr
                | ExprKind::ExpressionTraitExpr
                | ExprKind::CXXTypeidExpr => continue,
                ExprKind::CXXThrowExpr | ExprKind::CXXNewExpr => return true,
                ExprKind::CallExpr if self.callee_can_throw(exprs.callee(x)) => return true,
                ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr
                    if !self.decl_is_nothrow(exprs.constructor(x)) =>
                {
                    return true;
                }
                ExprKind::CXXBindTemporaryExpr
                    if self
                        .ast
                        .types
                        .as_record(n.ty())
                        .and_then(|r| self.ast.definition(r))
                        .and_then(|r| self.ast.destructor(r))
                        .is_some_and(|d| !self.decl_is_nothrow(d)) =>
                {
                    return true;
                }
                ExprKind::CXXDynamicCastExpr
                    if n.value_category() != ValueCategory::PRValue
                        && exprs.cast_kind(x) == CastKind::Dynamic =>
                {
                    return true;
                }
                _ => {}
            }
            stack.extend(exprs.children(x, &self.ast.lists));
        }
        false
    }

    /// Whether function `f`'s type has a non-throwing exception
    /// specification (an unresolved one counts as throwing).
    fn decl_is_nothrow(&self, f: kiln_ast::DeclId) -> bool {
        let t = &self.ast.types;
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        t.proto(ty).is_some_and(|p| t.exc_spec(p).is_nothrow())
    }

    /// Whether calling through `callee` may throw: its (pointer to)
    /// function type does not have a non-throwing exception specification.
    fn callee_can_throw(&self, callee: ExprId) -> bool {
        let t = &self.ast.types;
        let mut ty = self.ast.exprs.get(callee).ty();
        if let Some(p) = t.pointee(ty) {
            ty = p;
        }
        match t.proto(ty) {
            Some(p) => !t.exc_spec(p).is_nothrow(),
            None => true,
        }
    }
}

impl Sema<'_, '_> {
    /// `throw` / `throw e` ([expr.throw]): a prvalue of type `void`. The
    /// exception object's type is `e`'s with arrays and functions decayed
    /// and top-level cv removed ([expr.throw]/3); it must not be (a
    /// pointer to) an incomplete type other than cv `void`
    /// ([expr.throw]/2, clang's `CheckCXXThrowOperand`). The operand is
    /// copy-initialized into it after the implicit move
    /// ([class.copy.elision]/3; kiln has no try-blocks or lambdas yet, so
    /// every local is in scope). `loc` is the `throw` keyword.
    pub fn act_on_throw(&mut self, operand: Option<ExprId>, loc: Loc) -> ExprId {
        if !self.exceptions {
            self.error(DiagId::sema_throw_disabled, loc);
        }
        let Some(e) = operand else {
            return self.builder().throw_expr(None, QualType::VOID, loc);
        };
        if self.is_err_expr(e) {
            return self.recover(QualType::VOID, &[e], loc);
        }
        let ety = self.ety(e);
        let obj = if self.ast.types.is_array(ety) {
            let elem = self.ast.types.element(ety).unwrap_or(QualType::ERROR);
            self.ast.types.pointer_to(elem)
        } else if self.ast.types.is_function(ety) {
            self.ast.types.pointer_to(ety)
        } else {
            conv::strip_cv(self, ety)
        };
        let (checked, pointer) = match self.ast.types.pointee(obj) {
            Some(p) if self.ast.types.is_pointer(obj) => (p, true),
            _ => (obj, false),
        };
        if (!pointer || !self.ast.types.is_void(checked))
            && !self.ast.types.is_function(checked)
            && !self.require_complete(checked, loc)
        {
            let id = if pointer {
                DiagId::sema_throw_incomplete_ptr
            } else {
                DiagId::sema_throw_incomplete
            };
            let msg = self.diag(id, loc).arg(self.ty_arg(checked));
            self.emit(self.incomplete_note(msg, checked));
            return self.recover(QualType::VOID, &[e], loc);
        }
        let moved = self.implicit_move(e);
        let init = self.convert_for_init(moved, obj, ConvCtx::Initialization);
        if self.is_err_expr(init) {
            return self.recover(QualType::VOID, &[init], loc);
        }
        self.builder().throw_expr(Some(init), QualType::VOID, loc)
    }

    /// True if `e` is a (possibly parenthesized) throw-expression
    /// ([expr.cond]/2.1).
    pub(crate) fn is_throw_expr(&self, mut e: ExprId) -> bool {
        loop {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ParenExpr => e = self.ast.exprs.operand(e),
                k => return k == ExprKind::CXXThrowExpr,
            }
        }
    }
}
