//! Exception handling ([except]): exception specifications on function
//! declarators ([except.spec]) and the `noexcept` operator
//! ([expr.unary.noexcept]) (spec/sema.md §5.3).

use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::types::{BuiltinType, ExcSpec, NoexceptValue};
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
                    // A `-Wc++11-narrowing` error by default; clang goes
                    // on with the converted value.
                    self.emit(d);
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
                | ExprKind::CXXTypeidExpr => continue,
                ExprKind::CXXThrowExpr | ExprKind::CXXNewExpr => return true,
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
            stack.extend(exprs.children(x, &self.ast.lists));
        }
        false
    }

    /// Whether function `f`'s type has a non-throwing exception
    /// specification (an unresolved one counts as throwing).
    fn decl_is_nothrow(&self, f: DeclId) -> bool {
        let t = &self.ast.types;
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        t.proto(ty).is_some_and(|p| t.exc_spec(p).is_nothrow())
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
            && let Some((p, _)) =
                conv::scs::member_pointer_parts(self, self.ast.exprs.get(rhs).ty())
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

    /// The replayed exception specification `spec` of member function `f`
    /// (spec/sema-class.md §2.7): `f`'s type takes it as written.
    pub(crate) fn set_member_exception_spec(&mut self, f: DeclId, spec: ExcSpec) {
        if !self.unparsed_exc.remove(&f) {
            return;
        }
        let redecls: Vec<DeclId> = self.ast.redecls_of(f).collect();
        for d in redecls {
            let Some(ty) = self.ast.decls.get(d).ty() else { continue };
            let t = self.with_exc_spec(ty, spec);
            self.ast.decls.set_ty(d, t);
        }
    }

    /// Resolves the exception specification of function `f` before an
    /// expression takes its type at `loc` (C++17: it is part of the type).
    /// A specification not yet parsed is left to [`Sema::exc_spec_needed`].
    pub(crate) fn resolve_exc_for_type(&mut self, f: DeclId, loc: Loc) {
        if !self.in_template() && !self.unparsed_exc.contains(&self.ast.first(f)) {
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
        if !self.in_template() && !self.diagnose_unparsed_exc_spec(f, loc) {
            self.resolve_exception_spec(f, loc);
        }
    }

    /// Whether member `d` of a class template, being instantiated now,
    /// keeps its dependent `noexcept(expr)` uninstantiated until needed
    /// ([temp.inst]/14; clang's `EST_Uninstantiated`). A member template's
    /// pattern, and a member of a still-dependent specialization, are
    /// substituted at once.
    pub(crate) fn lazy_exc_spec(&self, d: DeclId, p: &kiln_ast::types::FuncProto) -> bool {
        matches!(
            self.ast.types.exc_spec(p),
            ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. }
        ) && !self.in_template()
            && self.ast.described_template(d).is_none()
            && !self.ast.decls.get(self.ctx).has(kiln_ast::decl::DeclFlags::DEPENDENT_CTX)
    }

    /// Instantiates the deferred `noexcept(expr)` of `f`, a member of a
    /// class template specialization, from its pattern at `poi` (clang's
    /// `InstantiateExceptionSpec`): substituted in the member's context,
    /// with its parameters and `this`. A substitution failure leaves `f`
    /// potentially throwing, as clang (`EST_None`).
    pub(crate) fn instantiate_exception_spec(&mut self, f: DeclId, poi: Loc) {
        let Some(pattern) = self.uninstantiated_exc.remove(&f) else { return };
        let pty = self.ast.decls.get(pattern).ty().unwrap_or(QualType::ERROR);
        let Some(pp) = self.sugared_proto(pty) else { return };
        let ExcSpec::NoexceptExpr { expr, .. } = self.ast.types.exc_spec(&pp) else { return };
        let quals = pp.info().this_quals();
        let resolved = self.substitute_exception_spec(f, pattern, expr, quals, poi);
        let spec = resolved.unwrap_or(ExcSpec::None);
        let redecls: Vec<DeclId> = self.ast.redecls_of(f).collect();
        for d in redecls {
            let Some(ty) = self.ast.decls.get(d).ty() else { continue };
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
        let spec = self.inst_owner(f)?;
        let (tmpl, args) = self.spec_args(spec)?;
        let pclass = self.spec_pattern_class(spec, tmpl)?;
        let mut pairs = self.inst_context_pairs(spec);
        if pairs.first().is_none_or(|&(_, x)| x != spec) {
            pairs.insert(0, (pclass, spec));
        }
        if !self.push_inst_frame(FrameKind::ExceptionSpec, f, pattern, poi) {
            return None;
        }
        let params = |s: &Self, d: DeclId| -> Vec<DeclId> {
            s.ast
                .decls
                .function(d)
                .map(|x| s.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default()
        };
        let (pp, fp) = (params(self, pattern), params(self, f));
        let method = self.kind_of(f) == kiln_ast::decl::DeclKind::CXXMethod;
        let at = self.ast.exprs.get(expr).loc();
        let start = self.begin_loc(expr);
        let ctx = self.ast.decls.get(f).ctx().unwrap_or(self.ast.tu);
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
        let first = self.ast.first(f);
        if !self.unparsed_exc.contains(&first) {
            return false;
        }
        self.error(DiagId::sema_exception_spec_not_parsed, loc);
        true
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
    /// - a replaceable global allocation function: the `ExtWarn`
    ///   `-Wmissing-exception-spec`, inheriting the specification.
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
        let error = !(replaceable && inherited.is_some());
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
/// specification: just past the `)` of the function declarator `d`
/// (`getLocForEndOfToken(FTLoc.getLocalRangeEnd())`). None with a trailing
/// return type (clang gives none) and, as kiln does not record where a
/// cv- or ref-qualifier ends, after either; none for a `)` from a macro.
pub(crate) fn missing_exc_spec_fixit_loc(d: &Declarator) -> Option<Loc> {
    d.chunks.iter().find_map(|c| match c {
        crate::declspec::Chunk::Function { rparen, quals, ref_qual, trailing, .. } => Some(
            (trailing.is_none()
                && quals.is_empty()
                && *ref_qual == kiln_ast::types::RefQual::None
                && rparen.is_file())
            .then(|| rparen.with_offset(1)),
        ),
        _ => None,
    })?
}
