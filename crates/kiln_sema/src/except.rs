//! Exception handling ([except]): exception specifications on function
//! declarators ([except.spec]) and the `noexcept` operator
//! ([expr.unary.noexcept]) (spec/sema.md §5.3).

use kiln_ast::decl::{DeclFlags, DefinitionBits};
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::types::{BuiltinType, ExcSpec, FuncProto, NoexceptValue};
use kiln_ast::{DeclId, ExprId};
use kiln_ast::{QualType, expr::ValueCategory};
use kiln_diag::{DiagId, FixIt};
use kiln_oven::EvalContext;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::{self, ConvCtx};
use crate::declspec::Declarator;

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
    /// A class type: the contextual conversion through its conversion
    /// functions ([conv.general]/4).
    Class,
    /// No implicit conversion to `bool` exists: scoped enumerations.
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
        match self.bool_cce(e, start, "noexcept specifier argument") {
            Some((ce, value)) => {
                let value = if value { NoexceptValue::True } else { NoexceptValue::False };
                ExcSpec::NoexceptExpr { expr: ce, value }
            }
            None => self.invalid_noexcept_spec(loc),
        }
    }

    /// A contextually converted constant expression of type `bool`
    /// ([expr.const]/12; clang `CheckConvertedConstantExpression` with
    /// `CCEK_Noexcept` / `CCEK_ExplicitBool`): the operand of a
    /// *noexcept-specifier* ([except.spec]/2) or an *explicit-specifier*
    /// ([dcl.fct.spec]/4). `e` is not value-dependent; `at` is where the
    /// diagnostics point and `what` names the operand in them. A class
    /// type converts through its conversion functions (the contextual
    /// conversion of [conv.general]/4), an integral or unscoped
    /// enumeration type unless narrowing. Returns the `ConstantExpr`
    /// holding the converted operand and its value, or `None` after an
    /// error was reported.
    pub(crate) fn bool_cce(&mut self, e: ExprId, at: Loc, what: &str) -> Option<(ExprId, bool)> {
        // Diagnostics name the operand's type as written (clang prints
        // `int *const`, not the converted `int *`).
        let written = self.ast.exprs.get(e).ty();
        let mut c = conv::lvalue_to_rvalue(self, e);
        let ty = conv::strip_cv(self, self.ast.exprs.get(c).ty());
        match self.bool_cce_kind(ty) {
            BoolCce::Integral => {
                if let Some(v) = self.fold_int(c)
                    && v != 0
                    && v != 1
                {
                    let d = self
                        .diag(DiagId::sema_cce_narrowing_value, at)
                        .arg(what)
                        .arg(v.to_string())
                        .arg(self.ty_arg(QualType::BOOL));
                    // A `-Wc++11-narrowing` error by default; clang goes
                    // on with the converted value.
                    self.emit(d);
                }
            }
            BoolCce::Class => {
                // Not the lvalue conversion: a conversion function takes
                // the object as it is.
                if let Ok(x) = conv::contextually_convert_to_bool(self, e) {
                    c = x;
                } else {
                    let d = self
                        .diag(DiagId::sema_cce_not_implicitly_convertible, at)
                        .arg(self.ty_arg(written))
                        .arg(self.ty_arg(QualType::BOOL));
                    self.emit(d);
                    return None;
                }
            }
            BoolCce::NotAllowed => {
                let d = self
                    .diag(DiagId::sema_cce_conversion_not_allowed, at)
                    .arg(self.ty_arg(written))
                    .arg(self.ty_arg(QualType::BOOL));
                self.emit(d);
                return None;
            }
            BoolCce::NotConvertible => {
                let d = self
                    .diag(DiagId::sema_cce_not_implicitly_convertible, at)
                    .arg(self.ty_arg(written))
                    .arg(self.ty_arg(QualType::BOOL));
                self.emit(d);
                return None;
            }
            BoolCce::Bool | BoolCce::Other => {}
        }
        let c = match conv::implicit_convert(self, c, QualType::BOOL, ConvCtx::Initialization) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, at);
                return None;
            }
        };
        let c = self.act_on_full_expr(c);
        match self.eval_const(c, EvalContext::converted(at)) {
            Ok(v) => {
                let value = self.const_truth(v).unwrap_or(false);
                let cv = self.int_const(QualType::BOOL, i128::from(value));
                Some((self.builder().constant_expr(c, Some(cv)), value))
            }
            Err(f) => {
                let d = self.diag(DiagId::sema_expr_not_cce, at).arg(what);
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                None
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
            return BoolCce::Class;
        }
        if self.is_error_type(ty) {
            return BoolCce::Other;
        }
        BoolCce::NotAllowed
    }

    /// `throw ( type-id-list )` ([except.spec], C++14 [except.spec]/1):
    /// `throw()` is the non-throwing `DynamicNone`, accepted silently as
    /// clang does in every mode; a non-empty list was removed in C++17
    /// and is diagnosed there (an error by default,
    /// `-Wdynamic-exception-spec`). Before C++17 it is valid (deprecated:
    /// clang's `-Wdeprecated-dynamic-exception-spec` is off by default and
    /// not implemented). `ok` is false after a syntax error in the list.
    /// `rparen` is the closing `)`.
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
        if self.std >= kiln_lex::Std::Cxx17 {
            let d = self
                .diag(DiagId::sema_dynamic_exception_spec, loc)
                .with_note(self.diag(DiagId::note_use_noexcept_false, loc));
            self.emit(d);
        }
        if !ok {
            return ExcSpec::None;
        }
        ExcSpec::Dynamic(self.ast.types.type_list(types))
    }

    /// `noexcept ( e )` ([expr.unary.noexcept]): a `bool` prvalue, `true`
    /// unless the unevaluated operand `e` is potentially-throwing
    /// ([except.spec]/6; see [`Sema::can_throw`]). `rparen` is the `)`.
    pub fn act_on_noexcept_expr(&mut self, e: ExprId, loc: Loc, rparen: Loc) -> ExprId {
        // [expr.ref]/7.3.2: a bound member function (or builtin function)
        // operand can only be called (clang's `CheckPlaceholderExpr`).
        if self.is_err_expr(e) || self.check_builtin_fn_use(e) {
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
    /// - a `throw` expression;
    /// - a `new` expression whose allocation function is not
    ///   non-throwing, and a `delete` expression whose deallocation
    ///   function or destroyed type's destructor is not (WP-783);
    /// - a `dynamic_cast` to a reference type that needs a run-time check
    ///   (not produced by the M2 parser);
    /// - any expression with a potentially-throwing immediate
    ///   subexpression ([except.spec]/6.4), except that the unevaluated
    ///   operands of `sizeof`, `alignof` and `noexcept` are not looked at.
    ///
    /// Implicit calls are seen through their nodes: constructors
    /// (`CXXConstructExpr`), destructors of bound temporaries
    /// (`CXXBindTemporaryExpr`) and conversion functions (member calls).
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
                | ExprKind::CXXTypeidExpr
                // [expr.prim.id.dtor]/2: as clang, the object expression
                // of a pseudo-destructor is not looked at either, so
                // `noexcept(f().~I())` is true for any `f`.
                | ExprKind::CXXPseudoDestructorExpr => continue,
                ExprKind::CXXThrowExpr => return true,
                // [except.spec]/6.1: a new-expression invokes its
                // allocation function (then the initializer, a child);
                // a placement `::new (p) T(...)` through the non-throwing
                // `operator new(size_t, void*)` is not potentially-throwing
                // when the initialization is not (clang `canThrow`, WP-783).
                ExprKind::CXXNewExpr
                    if exprs.new_extra(x).operator_new.is_none_or(|f| self.fn_can_throw(f)) =>
                {
                    return true;
                }
                // A delete-expression invokes the destroyed type's
                // destructor and the deallocation function (clang
                // `canThrow`): `delete p` with a `noexcept(false)`
                // destructor is potentially-throwing.
                ExprKind::CXXDeleteExpr if self.delete_can_throw(x) => return true,
                ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr
                    if self.callee_can_throw(exprs.callee(x)) =>
                {
                    return true;
                }
                ExprKind::CXXMemberCallExpr if self.member_call_can_throw(exprs.callee(x)) => {
                    return true;
                }
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
            exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        false
    }

    /// Whether function `f`'s type has a non-throwing exception
    /// specification (an unresolved one counts as throwing).
    pub(crate) fn decl_is_nothrow(&self, f: DeclId) -> bool {
        let t = &self.ast.types;
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        t.proto(ty).is_some_and(|p| t.exc_spec(p).is_nothrow())
    }

    /// Whether an implicitly invoked function `f` (an allocation or
    /// deallocation function, a destructor) may throw: clang
    /// `canCalleeThrow` on a declaration, so `NoThrowAttr` counts as
    /// non-throwing.
    fn fn_can_throw(&self, f: DeclId) -> bool {
        !self.ast.has_nothrow_attr(f) && !self.decl_is_nothrow(f)
    }

    /// Whether delete-expression `e` may throw through its deallocation
    /// function or the destructor of the type it destroys (the element
    /// type for `delete[]`).
    fn delete_can_throw(&self, e: ExprId) -> bool {
        let exprs = &self.ast.exprs;
        let t = &self.ast.types;
        if exprs.operator_delete(e).is_some_and(|f| self.fn_can_throw(f)) {
            return true;
        }
        let Some(mut dt) = t.pointee(exprs.get(exprs.delete_operand(e)).ty()) else {
            return false;
        };
        while let Some(el) = t.element(dt) {
            dt = el;
        }
        // A trivial destructor is implicit and non-throwing ([except.spec]/8)
        // whether or not its specification was resolved.
        t.as_record(dt)
            .and_then(|r| self.ast.definition(r))
            .filter(|&r| !self.defn_bits(r).contains(DefinitionBits::TRIVIAL_DTOR))
            .and_then(|r| self.ast.destructor(r))
            .is_some_and(|d| self.fn_can_throw(d))
    }

    /// Whether a member call through `callee` (a `MemberExpr`, or a
    /// `.*`/`->*` operator) may throw.
    fn member_call_can_throw(&self, callee: ExprId) -> bool {
        let c = self.ignore_parens(callee);
        if self.ast.exprs.get(c).kind() == ExprKind::MemberExpr {
            return !self.decl_is_nothrow(self.ast.exprs.decl(c));
        }
        // `(o.*pm)()`: the pointer-to-member's function type (the
        // `.*` / `->*` itself is a bound-member prvalue).
        let t = &self.ast.types;
        let mut ty = self.ast.exprs.get(c).ty();
        if self.ast.exprs.get(c).kind() == ExprKind::BinaryOperator
            && let Some(&rhs) = self.ast.exprs.children(c, &self.ast.lists).get(1)
            && conv::scs::member_pointer_parts(self, self.ast.exprs.get(rhs).ty()).is_some()
            && let Some(p) = t.exc_pointee(self.ast.exprs.get(rhs).ty())
        {
            ty = p;
        }
        t.proto(ty).is_none_or(|p| !t.exc_spec(p).is_nothrow())
    }

    /// Whether calling through `callee` may throw: its (pointer to)
    /// function type does not have a non-throwing exception specification.
    fn callee_can_throw(&self, callee: ExprId) -> bool {
        // clang `canCalleeThrow`: a callee declared with `NoThrowAttr`
        // (a recognized C library function, spec/sema.md §5.4) cannot.
        let c = self.ignore_paren_imp_casts(callee);
        if self.ast.exprs.get(c).kind() == ExprKind::DeclRefExpr
            && self.ast.has_nothrow_attr(self.ast.exprs.decl(c))
        {
            return false;
        }
        // A pseudo-destructor call has no effect ([expr.prim.id.dtor]/2)
        // and so is not potentially-throwing (clang `canThrow`).
        if self.ast.exprs.get(c).kind() == ExprKind::CXXPseudoDestructorExpr {
            return false;
        }
        let t = &self.ast.types;
        let mut ty = self.ast.exprs.get(callee).ty();
        if let Some(p) = t.exc_pointee(ty) {
            ty = p;
        }
        match t.proto(ty) {
            Some(p) => !t.exc_spec(p).is_nothrow(),
            None => true,
        }
    }
}

impl Sema<'_, '_> {
    /// A member function declarator whose *noexcept-specifier* the parser
    /// saved for a complete-class context ([class.mem.general]/7,
    /// spec/sema-class.md §2.7): `f`'s type carries `ExcSpec::Deferred(f)`
    /// and `f` sits in `unparsed_exc` until
    /// [`Sema::act_on_exception_spec`].
    pub(crate) fn defer_late_noexcept(&mut self, f: DeclId, d: &Declarator) {
        if d.late_noexcept.is_none() {
            return;
        }
        let Some(ty) = self.ast.decls.get(f).ty() else { return };
        if self.ast.types.proto(ty).is_none() {
            return;
        }
        let t = self.with_exc_spec(ty, ExcSpec::Deferred(f));
        self.ast.decls.set_ty(f, t);
        self.unparsed_exc.insert(f);
    }

    /// The replayed exception specification `spec` of member (or friend)
    /// function `f` (spec/sema-class.md §2.7): `f`'s type takes it as
    /// written. The declarations still waiting for it are `f` alone (a
    /// friend's earlier declarations keep their own), which is then
    /// compared with them ([`Sema::check_delayed_exc_equiv`]).
    pub(crate) fn set_member_exception_spec(&mut self, f: DeclId, spec: ExcSpec) {
        if !self.unparsed_exc.remove(&f) {
            return;
        }
        let redecls: Vec<DeclId> = self.ast.redecls_of(f).collect();
        for d in redecls {
            let Some(ty) = self.ast.decls.get(d).ty() else { continue };
            if self.written_exc_spec(ty) != ExcSpec::Deferred(f) {
                continue;
            }
            let t = self.with_exc_spec(ty, spec);
            self.ast.decls.set_ty(d, t);
        }
        self.check_delayed_exc_equiv(f);
    }

    /// [except.spec]/5 for the delayed pairs naming `f`, a redeclaration
    /// or previous declaration whose specification was unknown when the
    /// redeclaration was declared (clang `CheckDelayedMemberExceptionSpecs`
    /// → `CheckEquivalentExceptionSpec`): it must match the previous
    /// declaration's, `exception specification in declaration does not
    /// match previous declaration` at `f`.
    pub(crate) fn check_delayed_exc_equiv(&mut self, f: DeclId) {
        let mut pending = Vec::new();
        self.delayed_exc_equiv.retain(|&(n, p)| {
            if n == f || p == f {
                pending.push((n, p));
                false
            } else {
                true
            }
        });
        for (n, p) in pending {
            let (Some(nty), Some(pty)) = (self.declared_fn_type(n), self.declared_fn_type(p))
            else {
                continue;
            };
            let (ne, pe) = (self.written_exc_spec(nty), self.written_exc_spec(pty));
            let unknown = |e: ExcSpec| {
                matches!(
                    e,
                    ExcSpec::Deferred(_)
                        | ExcSpec::DependentNoexcept(_)
                        | ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. }
                )
            };
            // A templated class's friend with a dependent `noexcept(expr)`
            // matches no other declaration's specification (clang
            // `CheckEquivalentExceptionSpecImpl`: its `canThrow` is
            // `CT_Dependent`); each instantiation is checked again.
            let dependent_friend = unknown(ne)
                && !matches!(ne, ExcSpec::Deferred(_))
                && self.is_dependent_friend_fn(n);
            if !dependent_friend && (unknown(ne) || unknown(pe)) {
                // The other one is still to come: check again then.
                self.delayed_exc_equiv.push((n, p));
                continue;
            }
            if !dependent_friend && ne.is_nothrow() == pe.is_nothrow() {
                continue;
            }
            let loc = self.ast.decls.get(n).loc();
            if ne == ExcSpec::None {
                // Written without one: clang's missing-specification path.
                let name = self.decl_name(n);
                let _ = self.merge_missing_exc_spec(p, pty, nty, (loc, &name, None), false);
                continue;
            }
            let msg = self.diag(DiagId::sema_exception_spec_mismatch, loc);
            let msg = self.note_prev(msg, p, false);
            self.emit(per_instantiation_exc(msg, ne));
        }
    }

    /// Resolves the exception specification of function `f` before an
    /// expression takes its type at `loc` (C++17: it is part of the type).
    /// A specification not yet parsed is left to [`Sema::exc_spec_needed`].
    pub(crate) fn resolve_exc_for_type(&mut self, f: DeclId, loc: Loc) {
        if (!self.in_template() || self.exc_resolvable_in_template(f)) && !self.exc_unparsed(f) {
            self.resolve_exception_spec(f, loc);
        }
    }

    /// A use of function `f` at `loc` needs its exception specification
    /// ([except.spec]/13): clang's `MarkFunctionReferenced` resolves it
    /// even in unevaluated operands (for `noexcept`), and diagnoses one
    /// still awaiting its complete-class context. Uses in an unevaluated
    /// operand call this directly; evaluated ones through
    /// `mark_function_referenced`.
    pub(crate) fn exc_spec_needed(&mut self, f: DeclId, loc: Loc) {
        if (!self.in_template() || self.exc_resolvable_in_template(f))
            && !self.diagnose_unparsed_exc_spec(f, loc)
        {
            self.resolve_exception_spec(f, loc);
        }
    }

    /// Whether the uninstantiated `noexcept(expr)` of `f` is instantiated
    /// when a template definition names it: `f` is a specialization with
    /// no dependent argument (a function template specialization or a
    /// member of a class template specialization, [`Sema::lazy_exc_spec`])
    /// whose type a non-dependent expression takes there (clang's
    /// `ResolveExceptionSpec` on a `DeclRefExpr`, in any context). Its
    /// type then keeps `noexcept` in a template definition as outside
    /// one: `decltype(&h<true>)` is `void (*)() noexcept` in a template
    /// parameter's type or a template body, and a deduced argument
    /// carries it (spec/types.md §5.4, WP-756).
    fn exc_resolvable_in_template(&self, f: DeclId) -> bool {
        self.uninstantiated_exc.contains_key(&self.ast.first(f))
            && !self.ast.decls.get(f).ty().is_some_and(|t| self.is_dependent_type(t))
    }

    /// Whether function `d` of a class template, being instantiated now
    /// as a member or `friend`, keeps its dependent `noexcept(expr)`
    /// uninstantiated until needed ([temp.inst]/14; clang's
    /// `EST_Uninstantiated`, which `InitFunctionInstantiation` gives every
    /// instantiated declaration). A member template's (or a friend
    /// template's) redeclared pattern waits too: its specializations
    /// substitute the original pattern's with all their levels
    /// ([`Sema::defer_spec_exception_spec`]), whether it is defined in
    /// the class or only declared there: the class may still be
    /// incomplete ([class.mem.general]/7; libc++ `counted_iterator`'s
    /// `iter_swap`). A member of a still-dependent specialization is
    /// substituted at once.
    pub(crate) fn lazy_exc_spec(&self, d: DeclId, p: &FuncProto) -> bool {
        let tmpl = self.ast.described_template(d).is_some();
        matches!(
            self.ast.types.exc_spec(p),
            ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. }
        ) && self.ast.templates.dependent_depth <= u32::from(tmpl)
            && !self.ast.decls.get(self.ctx).has(DeclFlags::DEPENDENT_CTX)
    }

    /// The exception specification of function template specialization
    /// `spec` of pattern `pat`, whose own is `Deferred`: still awaiting its
    /// complete-class context (a friend template of a class being
    /// defined), or a member template's redeclared pattern
    /// ([`Sema::lazy_exc_spec`]). `spec` waits too, to be substituted
    /// from the original pattern with all its levels when needed. Returns
    /// whether it does.
    pub(crate) fn defer_spec_exception_spec(&mut self, pat: DeclId, spec: DeclId) -> bool {
        let first = self.ast.first(pat);
        let origin = match self.uninstantiated_exc.get(&first) {
            Some(&o) => o,
            None if self.exc_unparsed(pat) => pat,
            None => return false,
        };
        self.uninstantiated_exc.insert(spec, origin);
        true
    }

    /// Instantiates the deferred `noexcept(expr)` of `f`, a member or
    /// friend of a class template specialization or a function template
    /// specialization, from its pattern at `poi` (clang's
    /// `InstantiateExceptionSpec`): substituted in `f`'s context, with
    /// its parameters and `this`. A substitution failure leaves `f`
    /// potentially throwing, as clang (`EST_None`). A pattern still
    /// awaiting its complete-class context leaves `f` waiting.
    pub(crate) fn instantiate_exception_spec(&mut self, f: DeclId, poi: Loc) {
        let Some(&pattern) = self.uninstantiated_exc.get(&f) else { return };
        if self.exc_unparsed(pattern) {
            return;
        }
        self.uninstantiated_exc.remove(&f);
        let pty = self.ast.decls.get(pattern).ty().unwrap_or(QualType::ERROR);
        let Some(pp) = self.sugared_proto(pty) else { return };
        let spec = match self.ast.types.exc_spec(&pp) {
            ExcSpec::NoexceptExpr { expr, value: NoexceptValue::Dependent(_) } => {
                let quals = pp.info().this_quals();
                let resolved = self.substitute_exception_spec(f, pattern, expr, quals, poi);
                resolved.unwrap_or(ExcSpec::None)
            }
            ExcSpec::Deferred(_) => return,
            // Known once the pattern's complete-class context was parsed.
            other => other,
        };
        let redecls: Vec<DeclId> = self.ast.redecls_of(f).collect();
        for d in redecls {
            let Some(ty) = self.ast.decls.get(d).ty() else { continue };
            if !matches!(self.written_exc_spec(ty), ExcSpec::Deferred(_)) {
                continue;
            }
            let t = self.with_exc_spec(ty, spec);
            self.ast.decls.set_ty(d, t);
        }
    }

    /// `noexcept(expr)` of pattern `pattern` substituted for member `f`;
    /// `None` when the substitution fails or no instantiation context is
    /// found.
    fn substitute_exception_spec(
        &mut self,
        f: DeclId,
        pattern: DeclId,
        expr: ExprId,
        quals: kiln_ast::types::Quals,
        poi: Loc,
    ) -> Option<ExcSpec> {
        use crate::templ::stack::FrameKind;
        use crate::templ::subst::Substituter;
        // The specialization whose arguments substitute it: `f` itself (a
        // function template specialization, all its levels), or the class
        // template specialization `f` is a member or friend of.
        let (spec, args, pairs, frame_args) = if self.is_function_spec(f) {
            let (tmpl, args) = self.spec_args(f)?;
            let pairs = self.inst_context_pairs(self.spec_pairs_ctx(f, tmpl));
            (f, args, pairs, self.ast.spec_info(f).map(|(_, i)| i.args))
        } else {
            let spec = self.inst_owner(f)?;
            let (tmpl, args) = self.spec_args(spec)?;
            let pclass = self.spec_pattern_class(spec, tmpl)?;
            let mut pairs = self.inst_context_pairs(spec);
            if pairs.first().is_none_or(|&(_, x)| x != spec) {
                pairs.insert(0, (pclass, spec));
            }
            (spec, args, pairs, None)
        };
        if !self.push_inst_frame(FrameKind::ExceptionSpec, f, pattern, poi) {
            return None;
        }
        if let Some(l) = frame_args {
            self.set_frame_args(l);
        }
        let params = |s: &Self, d: DeclId| -> Vec<DeclId> {
            s.ast
                .decls
                .function(d)
                .map(|x| s.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default()
        };
        let (pp, fp) = (params(self, pattern), params(self, f));
        // A static member function has no `this`: an implicit one in the
        // pattern is "invalid use of 'this' outside of a non-static
        // member function" now ([expr.prim.this]/3; clang's
        // `TransformCXXThisExpr` in `InstantiateExceptionSpec`). Every
        // implicit object member function has one, constructors,
        // destructors and conversion functions included (WP-792).
        let method = self.is_implicit_object_member(f);
        let at = self.ast.exprs.get(expr).loc();
        let start = self.begin_loc(expr);
        // A friend's is in its class ([class.friend]/7), whose members it
        // may name. A function template specialization takes its template's
        // (the friend template declared in the class specialization).
        let lex = if self.is_function_spec(f) {
            self.spec_args(f).and_then(|(t, _)| self.templated_function(t)).unwrap_or(f)
        } else {
            f
        };
        let ctx = self
            .ast
            .decls
            .lexical_ctx(lex)
            .filter(|&c| self.kind_of(c) == kiln_ast::decl::DeclKind::CXXRecord)
            .or_else(|| self.ast.decls.get(f).ctx())
            .unwrap_or(self.ast.tu);
        let r = self.in_inst_context(ctx, |s| {
            let this = s.act_on_start_this_scope(method, quals);
            let mut sub = Substituter::new(s, &args);
            sub.entity = Some(spec);
            sub.map_contexts(&pairs);
            sub.map_params(&pp, &fp);
            let e = sub.expr(expr);
            let r = e.ok().map(|e| sub.sema.act_on_noexcept_spec(e, at, start));
            s.act_on_end_this_scope(this);
            r
        });
        self.pop_inst_frame();
        r
    }

    /// clang's `ResolveExceptionSpec` diagnostic for a function whose
    /// *noexcept-specifier* is needed ([except.spec]/13) before the
    /// complete-class context it is parsed in: `exception specification is
    /// not available until end of class definition`. Returns true then.
    pub(crate) fn diagnose_unparsed_exc_spec(&mut self, f: DeclId, loc: Loc) -> bool {
        if !self.exc_unparsed(f) {
            return false;
        }
        self.error(DiagId::sema_exception_spec_not_parsed, loc);
        true
    }

    /// Whether the declaration `f` names (or its first declaration) still
    /// waits for its *noexcept-specifier*'s complete-class context: a
    /// friend redeclaration may wait while an earlier declaration does not.
    pub(crate) fn exc_unparsed(&self, f: DeclId) -> bool {
        let first = self.ast.first(f);
        self.unparsed_exc.contains(&f)
            || self.unparsed_exc.contains(&first)
            || self.uninstantiated_exc.get(&first).is_some_and(|p| self.unparsed_exc.contains(p))
    }

    /// A `throw` or `try` at `loc` with exceptions disabled
    /// (`-fno-exceptions`, [except.pre], spec/sema.md "Exceptions
    /// disabled"; WP-607): an error (`id`) unless `loc` is in a system
    /// header (clang `ActOnCXXThrow`/`ActOnCXXTryBlock`) or inside a
    /// template definition, where it waits for the instantiation (GCC's
    /// dependent `THROW_EXPR`; the substitution calls `act_on_throw` /
    /// `act_on_try` again outside the template). A location is reported
    /// once, however many specializations instantiate it.
    /// Returns whether the check waits for an instantiation.
    pub(crate) fn check_exceptions_enabled(&mut self, id: DiagId, loc: Loc) -> bool {
        if self.exceptions || self.diags.sm().is_in_system_header(loc) {
            return false;
        }
        if self.in_template() {
            return true;
        }
        if self.exc_disabled_seen.insert(loc) {
            self.error(id, loc);
        }
        false
    }

    /// An instantiation shares the non-dependent pattern expression `e`
    /// (the substitution does not call `act_on_throw` again): each
    /// throw-expression in it whose exceptions-disabled error was deferred
    /// ([`Sema::check_exceptions_enabled`]) is reported now.
    pub(crate) fn note_shared_throws(&mut self, e: ExprId) {
        if self.exc_disabled_pattern.is_empty() || self.in_template() {
            return;
        }
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            if self.exc_disabled_pattern.contains(&x) {
                let loc = self.ast.exprs.get(x).loc();
                self.check_exceptions_enabled(DiagId::sema_throw_disabled, loc);
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
    }

    /// `throw` / `throw e` ([expr.throw]): a prvalue of type `void`. The
    /// exception object's type is `e`'s with arrays and functions decayed
    /// and top-level cv removed ([expr.throw]/3); it must not be (a
    /// pointer to) an incomplete type other than cv `void`
    /// ([expr.throw]/2, clang's `CheckCXXThrowOperand`). The operand is
    /// copy-initialized into it after the implicit move
    /// ([class.copy.elision]/3) when the variable's scope does not extend
    /// past the innermost try-block (`thrown_var_in_scope`, WP-100).
    /// `loc` is the `throw` keyword.
    pub fn act_on_throw(&mut self, operand: Option<ExprId>, loc: Loc) -> ExprId {
        let r = self.build_throw(operand, loc);
        if self.check_exceptions_enabled(DiagId::sema_throw_disabled, loc)
            && self.ast.exprs.get(r).kind() == ExprKind::CXXThrowExpr
        {
            self.exc_disabled_pattern.insert(r);
        }
        r
    }

    fn build_throw(&mut self, operand: Option<ExprId>, loc: Loc) -> ExprId {
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
        // [except.throw]/5, [class.abstract]/3: an object of class type is
        // not abstract and its destructor is potentially invoked, so it is
        // accessible and not deleted (clang `CheckCXXThrowOperand`).
        if !pointer {
            if let Some(c) = self.abstract_class(obj) {
                let msg = self.diag(DiagId::sema_throw_abstract, loc).arg(self.ty_arg(obj));
                let msg = self.pure_virtual_notes(msg, c);
                self.emit(msg);
                return self.recover(QualType::VOID, &[e], loc);
            }
            let at = self.expr_loc(e);
            let access = (at, DiagId::sema_throw_object_dtor_access);
            if self.mark_element_dtor_referenced(obj, access, at).is_none() {
                return self.recover(QualType::VOID, &[e], loc);
            }
        }
        let moved = if self.thrown_var_in_scope(e) { self.implicit_move(e) } else { e };
        let init = self.with_init_entity(crate::init::entity::EXCEPTION, |s| {
            s.convert_for_init(moved, obj, ConvCtx::Initialization)
        });
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

impl Sema<'_, '_> {
    /// [except.spec]/5 for a redeclaration of type `ty` (named `fname` at
    /// `loc`) that writes no exception specification where the previous
    /// declaration `p`, of type `pty`, has an incompatible one: clang 19's
    /// missing-specification path of `CheckEquivalentExceptionSpec`
    /// (spec/sema.md §3, "Exception specifications on redeclarations").
    /// Returns `ty` with the specification it inherits and whether the
    /// redeclaration is an error (and so invalid). `replaceable` says
    /// whether it declares a replaceable global allocation function;
    /// `fix_at` is where clang's fix-it inserts the specification.
    ///
    /// - `p` non-throwing, with C language linkage, and in a system header
    ///   (or a builtin, or without a location): silently `throw()`, since
    ///   C libraries may add non-throwing specifications.
    /// - a replaceable global allocation function, or `p` declared GNU
    ///   `nothrow` (WP-186): the `ExtWarn` `-Wmissing-exception-spec`,
    ///   inheriting the specification.
    /// - otherwise the error, still inheriting a non-dependent
    ///   specification, as clang updates the invalid declaration's type.
    pub(crate) fn merge_missing_exc_spec(
        &mut self,
        p: DeclId,
        pty: QualType,
        ty: QualType,
        (loc, fname, fix_at): (Loc, &str, Option<Loc>),
        replaceable: bool,
    ) -> (QualType, bool) {
        debug_assert!(
            !self.ast.decls.get(p).has(DeclFlags::INVALID),
            "an invalid previous declaration is never compared (pick_prev_function)"
        );
        let old = self.written_exc_spec(pty);
        let first = self.ast.redecls_of(p).last().unwrap_or(p);
        let ploc = self.ast.decls.get(p).loc();
        if old.is_nothrow()
            && kiln_abi::mangle::is_extern_c(&self.ast, first)
            && (!ploc.is_valid()
                || self.diags.sm().is_in_system_header(ploc)
                || self.ast.decls.builtin_id(first).is_some()
                || self.lib_builtin_info(first).is_some())
        {
            return (self.with_exc_spec(ty, ExcSpec::DynamicNone), false);
        }
        let inherited = match old {
            ExcSpec::NoexceptExpr { value: NoexceptValue::True, .. } => {
                Some(ExcSpec::BasicNoexcept)
            }
            ExcSpec::NoexceptExpr { value: NoexceptValue::False, .. } => Some(ExcSpec::None),
            ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. }
            | ExcSpec::DependentNoexcept(_)
            | ExcSpec::Deferred(_) => None,
            e => Some(e),
        };
        let ty = inherited.map_or(ty, |e| self.with_exc_spec(ty, e));
        // clang: a missing GNU `nothrow` is the extension warning too.
        let error = !(replaceable && inherited.is_some()) && old != ExcSpec::NothrowAttr;
        let id = if error {
            DiagId::sema_missing_exception_spec
        } else {
            DiagId::sema_missing_exception_spec_ext
        };
        let spelled = self.exc_spec_spelling(old);
        let mut msg = self.diag(id, loc).arg(fname).arg(spelled.clone());
        if let Some(at) = fix_at {
            msg = msg.fixit(FixIt::insert(at, format!(" {spelled}")));
        }
        let msg = self.note_prev(msg, p, false);
        self.emit(msg);
        (ty, error)
    }
}

/// Where clang's missing-exception-specification fix-it inserts the
/// specification: just past the end of the function declarator `d`'s
/// parameter list and cv- and ref-qualifiers
/// (`getLocForEndOfToken(FTLoc.getLocalRangeEnd())`). None with a trailing
/// return type (clang gives none) and for a `)` or qualifier from a macro.
pub(crate) fn missing_exc_spec_fixit_loc(d: &Declarator) -> Option<Loc> {
    let (rparen, ref_qual) = d.chunks.iter().find_map(|c| match c {
        crate::declspec::Chunk::Function { rparen, ref_qual, trailing, .. } => {
            Some(trailing.is_none().then_some((*rparen, *ref_qual)))
        }
        _ => None,
    })??;
    // The innermost function declarator's qualifiers ([`Declarator::
    // fn_qual_locs`]): a ref-qualifier follows any cv-qualifiers.
    let [c, v, r] = d.fn_qual_locs;
    let last = match (r, ref_qual) {
        (Some(r), kiln_ast::types::RefQual::LValue) => Some((r, 1)),
        (Some(r), kiln_ast::types::RefQual::RValue) => Some((r, 2)),
        _ => match (c, v) {
            (Some(c), Some(v)) if v.to_raw() > c.to_raw() => Some((v, 8)),
            (Some(c), _) => Some((c, 5)),
            (None, Some(v)) => Some((v, 8)),
            (None, None) => None,
        },
    };
    let (at, len) = last.unwrap_or((rparen, 1));
    at.is_file().then(|| at.with_offset(len))
}

/// A redeclaration's `noexcept(expr)` mismatch inside an instantiation is
/// reported for each instantiation, also when the template definition
/// reported it (clang substitutes and compares the operand every time,
/// spec/diag.md §7.4).
pub(crate) fn per_instantiation_exc(msg: kiln_diag::Diag, ne: ExcSpec) -> kiln_diag::Diag {
    if matches!(ne, ExcSpec::NoexceptExpr { .. }) { msg.per_instantiation() } else { msg }
}

/// How a function type's exception specification bounds what it throws,
/// for [`Sema::exc_spec_compat_error`] (clang `CanThrowResult` with the
/// `EST_Dynamic` distinction).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ExcBound {
    /// Throws nothing.
    Cannot,
    /// May throw anything.
    Any,
    /// `throw(T...)`: the listed types.
    Listed,
    /// Dependent or not yet computed: not checked.
    Unknown,
}

fn exc_bound(e: ExcSpec) -> ExcBound {
    match e {
        ExcSpec::Dynamic(_) => ExcBound::Listed,
        ExcSpec::DependentNoexcept(_)
        | ExcSpec::Deferred(_)
        | ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. } => ExcBound::Unknown,
        e if e.is_nothrow() => ExcBound::Cannot,
        _ => ExcBound::Any,
    }
}

impl Sema<'_, '_> {
    /// Before C++17, [except.spec]/5 (C++14): when a pointer to function,
    /// reference to function or pointer to member function is assigned,
    /// initialized, passed or returned, the target's exception
    /// specification must allow at least what the source's does, and
    /// the specifications of function types among its return and
    /// parameter types must be equivalent. The check is clang
    /// `CheckExceptionSpecCompatibility` (`CheckExceptionSpecSubset`,
    /// `CheckParamExceptionSpec`): it reads the specifications as
    /// written, which before C++17 are sugar over the canonical type
    /// (spec/types.md §5.4), and is skipped under `-fno-exceptions` and
    /// for dependent or not yet computed specifications. From C++17 the
    /// specification is part of the type and the conversion itself fails.
    /// Returns the error to report at the start of the source expression.
    pub(crate) fn exc_spec_compat_error(
        &self,
        from: QualType,
        to: QualType,
    ) -> Option<conv::ConvError> {
        if self.std >= kiln_lex::Std::Cxx17 || !self.exceptions {
            return None;
        }
        let (target_spec, target) = self.underlying_fn_proto(to)?;
        let (source_spec, source) = self.underlying_fn_proto(from)?;
        let t = &self.ast.types;
        let (target_bound, source_bound) = (exc_bound(target_spec), exc_bound(source_spec));
        if target_bound == ExcBound::Unknown || source_bound == ExcBound::Unknown {
            return None;
        }
        let incompatible =
            || Some(conv::ConvError::new(DiagId::sema_incompatible_exception_specs, Vec::new()));
        if target_bound == ExcBound::Any || source_bound == ExcBound::Cannot {
            return self.param_exc_specs_error(&target, &source);
        }
        if source_bound == ExcBound::Any || target_bound == ExcBound::Cannot {
            return incompatible();
        }
        let (ExcSpec::Dynamic(target_list), ExcSpec::Dynamic(source_list)) =
            (target_spec, source_spec)
        else {
            return None;
        };
        let handlers: Vec<QualType> = t.type_list_types(target_list).to_vec();
        let thrown: Vec<QualType> = t.type_list_types(source_list).to_vec();
        for e in thrown {
            let e = self.non_reference(e);
            if !handlers.iter().any(|&h| self.handler_can_catch(h, e)) {
                return incompatible();
            }
        }
        self.param_exc_specs_error(&target, &source)
    }

    /// clang `GetUnderlyingFunction`: the function type `q` is, or that
    /// it points or refers to: its written exception specification, and
    /// the prototype of the first function node in its sugar chain, whose
    /// return and parameter types keep their own written specifications.
    fn underlying_fn_proto(&self, q: QualType) -> Option<(ExcSpec, FuncProto)> {
        let t = &self.ast.types;
        let f = t.exc_pointee(q).unwrap_or(q);
        let exc = t.exc_spec(t.proto(f)?);
        let mut w = f;
        for _ in 0..256 {
            if let Some(p) = t.proto_of(w.ty()) {
                return Some((exc, *p));
            }
            let n = t.desugar_once(w);
            if t.is_canonical(w.ty()) || n.ty() == w.ty() {
                break;
            }
            w = n;
        }
        t.proto(f).map(|p| (exc, *p))
    }

    /// clang `CheckParamExceptionSpec`: function types in the return and
    /// parameter types of `sup` and `sub` have equivalent specifications.
    fn param_exc_specs_error(&self, sup: &FuncProto, sub: &FuncProto) -> Option<conv::ConvError> {
        let deep = |which: &str| {
            Some(conv::ConvError::new(
                DiagId::sema_deep_exception_specs_differ,
                vec![which.to_owned()],
            ))
        };
        if !self.exc_specs_equivalent(sup.ret(), sub.ret()) {
            return deep("0");
        }
        let t = &self.ast.types;
        let pairs: Vec<(QualType, QualType)> =
            t.params(sup).iter().copied().zip(t.params(sub).iter().copied()).collect();
        if pairs.into_iter().any(|(a, b)| !self.exc_specs_equivalent(a, b)) {
            return deep("1");
        }
        None
    }

    /// clang `CheckSpecForTypesEquivalent`: true unless both `a` and `b`
    /// are (pointers or references to) function types whose exception
    /// specifications are not equivalent ([except.spec]/3 before C++17:
    /// both non-throwing, both throw anything, or the same type list).
    fn exc_specs_equivalent(&self, a: QualType, b: QualType) -> bool {
        let (Some((ea, _)), Some((eb, _))) =
            (self.underlying_fn_proto(a), self.underlying_fn_proto(b))
        else {
            return true;
        };
        let t = &self.ast.types;
        match (exc_bound(ea), exc_bound(eb)) {
            (ExcBound::Unknown, _)
            | (_, ExcBound::Unknown)
            | (ExcBound::Cannot, ExcBound::Cannot)
            | (ExcBound::Any, ExcBound::Any) => true,
            (ExcBound::Listed, ExcBound::Listed) => {
                let (ExcSpec::Dynamic(la), ExcSpec::Dynamic(lb)) = (ea, eb) else { return true };
                let canon = |l| -> Vec<QualType> {
                    let mut v: Vec<QualType> = t
                        .type_list_types(l)
                        .iter()
                        .map(|&x| t.canonical(x).without_quals())
                        .collect();
                    v.sort();
                    v.dedup();
                    v
                };
                canon(la) == canon(lb)
            }
            _ => false,
        }
    }

    /// clang `handlerCanCatch` ([except.handle]/3): a handler of type
    /// `handler` matches an exception of type `exc` (a reference already
    /// stripped): the same type, a public unambiguous base class, or a
    /// pointer that converts by a derived-to-base pointer or
    /// qualification conversion (or to `void *`).
    fn handler_can_catch(&self, handler: QualType, exc: QualType) -> bool {
        let t = &self.ast.types;
        let h = t.canonical(self.non_reference(handler)).without_quals();
        let e = t.canonical(exc).without_quals();
        if t.same_type(h, e) {
            return true;
        }
        if let (Some(hc), Some(ec)) = (t.as_record(h), t.as_record(e)) {
            return self.is_public_unambiguous_base(ec, hc);
        }
        if !(t.is_pointer(h) && t.is_pointer(e)) {
            return false;
        }
        let (Some(hp), Some(ep)) = (t.pointee(h), t.pointee(e)) else { return false };
        let (hp, ep) = (t.canonical(hp), t.canonical(ep));
        if !hp.quals().contains(ep.quals()) {
            return false;
        }
        let (hu, eu) = (hp.without_quals(), ep.without_quals());
        if t.same_type(hu, eu) {
            return true;
        }
        if t.builtin(hu) == Some(BuiltinType::Void) {
            return !t.is_function(eu);
        }
        match (t.as_record(hu), t.as_record(eu)) {
            (Some(hc), Some(ec)) => self.is_public_unambiguous_base(ec, hc),
            _ => false,
        }
    }

    /// True if class `base` is a public, unambiguous base of class
    /// `derived`.
    fn is_public_unambiguous_base(&self, derived: DeclId, base: DeclId) -> bool {
        let Some(def) = self.ast.definition(derived) else { return false };
        let conv::bases::BaseSearch::Unique(path) = conv::bases::find_base(self, def, base) else {
            return false;
        };
        let mut from = def;
        path.specs.iter().zip(&path.classes).all(|(&i, &c)| {
            let written = self.ast.decls.base_spec(i).access;
            let ok = self.base_access(from, written) == kiln_ast::decl::Access::Public;
            from = c;
            ok
        })
    }
}
