//! `decltype` ([dcl.type.decltype]), `decltype(auto)` and deduced return
//! types ([dcl.spec.auto], [dcl.type.auto.deduct]; spec/sema.md §4.3.1).

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::types::{AutoKeyword, Quals, TypeFlags, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

/// A call whose incomplete class prvalue result is checked at the end of
/// the enclosing `decltype` operand ([dcl.type.decltype]/2).
#[derive(Clone, Copy, Debug)]
pub(crate) struct DeferredCall {
    /// The call's location (its node's).
    pub loc: Loc,
    /// The result type.
    pub ty: QualType,
    /// The function the callee names.
    pub callee: Option<DeclId>,
    /// Where the callee begins (the diagnostic's location).
    pub at: Loc,
}

impl Sema<'_, '_> {
    /// Enters the operand of `decltype(e)`: unevaluated, and calls of an
    /// incomplete class result deferred ([dcl.type.decltype]/2).
    pub fn enter_decltype(&mut self) {
        self.enter_unevaluated();
        self.decltype_calls.push((self.unevaluated, Vec::new()));
    }

    /// Leaves the operand `e` (`None` after a parse error) of
    /// `decltype(e)`: the deferred calls other than `e` itself (under
    /// parentheses and as the right operand of a comma) need a complete
    /// result type (clang `ActOnDecltypeExpression`).
    pub fn exit_decltype(&mut self, e: Option<ExprId>) {
        self.exit_unevaluated();
        let Some((_, calls)) = self.decltype_calls.pop() else { return };
        if calls.is_empty() {
            return;
        }
        let exempt = e.and_then(|e| self.decltype_top_call(e));
        for c in calls {
            if Some(c.loc) != exempt && self.check_call_return(c) {
                // The destructor check `maybe_bind_temporary` deferred.
                if let Some(class) =
                    self.ast.types.as_record(c.ty).and_then(|r| self.ast.definition(r))
                {
                    let _ = self.check_temporary_dtor(class, c.ty, c.at);
                }
            }
        }
    }

    /// The location of the call `e` denotes for [dcl.type.decltype]/2.
    fn decltype_top_call(&self, e: ExprId) -> Option<Loc> {
        let mut e = e;
        for _ in 0..1024 {
            let n = self.ast.exprs.get(e);
            match n.kind() {
                ExprKind::ParenExpr => e = self.ast.exprs.operand(e),
                ExprKind::BinaryOperator
                    if self.ast.exprs.binary_op(e) == kiln_ast::expr::BinaryOp::Comma =>
                {
                    e = self.ast.exprs.operands(e)[1];
                }
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr => return Some(n.loc()),
                _ => return None,
            }
        }
        None
    }

    /// clang's `CheckCallReturnType`: a prvalue result needs a complete
    /// type ([expr.call]/14). False after reporting that it does not.
    pub(crate) fn check_call_return(&mut self, c: DeferredCall) -> bool {
        if self.require_complete(c.ty, c.loc) {
            return true;
        }
        let msg = match c.callee {
            Some(d) => {
                let name = self.decl_name(d);
                let note = self
                    .diag(DiagId::note_entity_declared_at, self.ast.decls.get(d).loc())
                    .arg(format!("'{name}'"));
                self.diag(DiagId::sema_call_incomplete_return, c.at)
                    .arg(format!("'{name}'"))
                    .arg(self.ty_arg(c.ty))
                    .with_note(note)
            }
            None => {
                self.diag(DiagId::sema_call_function_incomplete_return, c.at).arg(self.ty_arg(c.ty))
            }
        };
        self.emit(self.incomplete_note(msg, c.ty));
        false
    }

    /// `_Atomic ( t )` (C11 6.7.2.4, a clang extension in C++): the atomic
    /// type of `t`, or `None` after diagnosing an array, function,
    /// reference, atomic, cv-qualified or `_BitInt` `t` (clang
    /// `BuildAtomicType`, which rejects every bit-precise integer type).
    /// A dependent `t` is checked at instantiation.
    pub fn act_on_atomic_type(&mut self, t: QualType, loc: Loc) -> Option<QualType> {
        if t == QualType::ERROR {
            return None;
        }
        let dependent = self.ast.types.qflags(t).intersects(TypeFlags::DEPENDENT);
        if !dependent {
            let c = self.ast.types.canonical(t);
            let bad = match self.ast.types.view(c.ty()) {
                _ if self.ast.types.is_array(c) => Some(0),
                TypeView::Function(_) => Some(1),
                TypeView::LValueRef(_) | TypeView::RValueRef(_) => Some(2),
                TypeView::Atomic(_) => Some(3),
                _ if !c.quals().is_empty() => Some(4),
                TypeView::BitInt { .. } => Some(5),
                _ => None,
            };
            if let Some(k) = bad {
                let msg = self
                    .diag(DiagId::sema_atomic_specifier_bad_type, loc)
                    .arg(k.to_string())
                    .arg(self.ty_arg(t));
                self.emit(msg);
                return None;
            }
        }
        let r = self.ast.types.atomic(t);
        // Where substitution diagnoses the operand (§5.8 written
        // locations): the `_Atomic` of the template definition.
        if dependent {
            self.note_written_loc(crate::templ::written::WrittenKey::Type(r), loc);
        }
        Some(r)
    }

    /// `decltype ( e )` ([dcl.type.decltype]/1): the type `decltype(e)`
    /// sugar over the computed type, or `None` when `e` is erroneous.
    /// `e` was parsed as an unevaluated operand.
    pub fn act_on_decltype(&mut self, e: ExprId, _loc: Loc, _rparen: Loc) -> Option<QualType> {
        let e = self.resolve_single_overload_candidate(e);
        if self.is_err_expr(e) || self.check_overload_use(e) {
            return None;
        }
        // `decltype(C::f)` of a non-static member function: the operand is
        // a full-expression ([intro.execution]/5) naming the member without
        // an object ([expr.prim.id.general]/4), diagnosed here so that a
        // substitution fails in its SFINAE context ([temp.deduct.general]/8).
        let x = self.ignore_parens(e);
        if let Some(r) = self.member_refs.get(&x).copied()
            && r.error
            && self.kind_of(r.decl) == DeclKind::CXXMethod
        {
            self.member_refs.remove(&x);
            self.error(DiagId::sema_member_call_without_object, self.eloc(x));
            return None;
        }
        // A bound member function or builtin function operand can only be
        // called ([expr.ref]/7.3.2); clang's `CheckPlaceholderExpr` rejects
        // it, so a substitution forming one fails ([temp.deduct.general]/8).
        if self.check_builtin_fn_use(e) {
            return None;
        }
        self.check_unevaluated_side_effects(e);
        // An instantiation-dependent operand: the type is computed at
        // instantiation, canonically `DependentDecltype` (clang's
        // `getDecltypeType`; the underlying type is `<dependent type>` for a
        // type-dependent operand).
        let deps = self.ast.exprs.get(e).deps();
        if deps.contains(kiln_ast::expr::ExprDeps::INSTANTIATION)
            && !self.names_template_local(e, deps)
        {
            let u = if deps.contains(kiln_ast::expr::ExprDeps::TYPE) {
                QualType::DEPENDENT
            } else {
                self.decltype_of(e)
            };
            let p = self.dependent_profile(e);
            return Some(self.ast.types.decltype_type(e, u, Some(p)));
        }
        let u = self.decltype_of(e);
        Some(self.ast.types.decltype_type(e, u, None))
    }

    /// Whether the `decltype` operand `e` (with dependence `deps`) only
    /// names a variable or parameter with automatic storage of a template
    /// whose type is not dependent (`std::vector<int> V; decltype(V)`, or
    /// `decltype((V))`). Its reference is instantiation-dependent only so
    /// that instantiation rebinds it to the instantiated local
    /// (`decl_deps`); clang's `computeDependence(DeclRefExpr *)` does not
    /// make it so, and `decltype(V)::iterator` names a member of a known
    /// class ([temp.dep.type]/7, LLVM's MappedIteratorTest.cpp).
    fn names_template_local(&self, e: ExprId, deps: kiln_ast::expr::ExprDeps) -> bool {
        if deps != kiln_ast::expr::ExprDeps::INSTANTIATION {
            return false;
        }
        let x = self.ignore_parens(e);
        if self.ast.exprs.get(x).kind() != ExprKind::DeclRefExpr {
            return false;
        }
        let d = self.ast.exprs.decl(x);
        matches!(self.kind_of(d), DeclKind::Var | DeclKind::ParmVar)
            && self.has_local_storage(d)
            && self.ast.decls.get(d).ty().is_some_and(|t| {
                !self.ast.types.qflags(t).intersects(
                    TypeFlags::DEPENDENT_INSTANTIATION
                        | TypeFlags::UNEXPANDED_PACK
                        | TypeFlags::UNDEDUCED,
                )
            })
    }

    /// The type `decltype(e)` denotes ([dcl.type.decltype]/1): the
    /// declared type of an unparenthesized id-expression or class member
    /// access; otherwise `T&&` for an xvalue, `T&` for an lvalue and `T`
    /// for a prvalue.
    pub(crate) fn decltype_of(&mut self, e: ExprId) -> QualType {
        let n = *self.ast.exprs.get(e);
        if matches!(n.kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr) {
            let d = self.ast.exprs.decl(e);
            // An enumerator inside its enumeration's body has the type
            // of its initializing value ([dcl.enum]/5).
            if let Some(t) = self.enumerator_pre_brace_type(d) {
                return t;
            }
            if let Some(t) = self.ast.decls.get(d).ty() {
                return t;
            }
        }
        if n.kind() == ExprKind::SubstNonTypeTemplateParmExpr
            && self.ast.types.as_record(n.ty()).is_some()
        {
            // A template parameter object ([temp.param]/8): the
            // parameter's (deduced) class type, without the `const` of
            // the object ([dcl.type.decltype]/1.3).
            return crate::conv::strip_cv(self, n.ty());
        }
        match n.value_category() {
            ValueCategory::LValue => self.ast.types.lvalue_ref_to(n.ty()),
            ValueCategory::XValue => self.ast.types.rvalue_ref_to(n.ty()),
            ValueCategory::PRValue => n.ty(),
        }
    }

    /// The keyword of an undeduced placeholder type `q` (`auto`,
    /// `decltype(auto)`), if `q` is one.
    pub(crate) fn undeduced_keyword(&self, q: QualType) -> Option<AutoKeyword> {
        match self.ast.types.view(self.ast.types.canonical(q).ty()) {
            TypeView::Auto { keyword, deduced: None, .. } => Some(keyword),
            _ => None,
        }
    }

    /// The type deduced for the placeholder `declared` from `e`
    /// ([dcl.type.auto.deduct]/4): for `auto`, the decayed type of `e`
    /// without top-level cv-qualifiers, plus the cv-qualifiers written
    /// on `declared`; for `decltype(auto)`, `decltype(e)`.
    pub(crate) fn deduce_placeholder(&mut self, declared: QualType, e: ExprId) -> QualType {
        if self.is_err_expr(e) {
            return QualType::ERROR;
        }
        if self.undeduced_keyword(declared) == Some(AutoKeyword::DecltypeAuto) {
            // `decltype(auto)` admits no written cv; the only qualifier is
            // the `const` `constexpr` adds ([dcl.constexpr]/6), which a
            // reference ignores.
            let t = self.decltype_of(e);
            let q = declared.quals();
            if q.is_empty() || t == QualType::ERROR || self.ast.types.is_reference(t) {
                return t;
            }
            return self.ast.types.qualify(t, q);
        }
        let ety = self.ast.exprs.get(e).ty();
        let t = &self.ast.types;
        let base = if t.is_array(ety) {
            let el = t.element(ety).unwrap_or(QualType::ERROR);
            self.ast.types.pointer_to(el)
        } else if t.is_function(ety) {
            self.ast.types.pointer_to(ety)
        } else {
            let c = self.ast.types.canonical(ety);
            if self.ast.types.effective_quals(ety).is_empty() { ety } else { c.without_quals() }
        };
        let q = declared.quals();
        if q.is_empty() { base } else { self.ast.types.qualify(base, q) }
    }

    /// True if `q` is an undeduced `auto` under zero or more pointer and
    /// reference declarators and function return types (`auto`, `const
    /// auto &`, `auto **`, `auto (*)(int) -> auto`, ...;
    /// [dcl.spec.auto.general]/3, [`Sema::placeholder_step`]).
    pub(crate) fn contains_undeduced_auto(&self, q: QualType) -> bool {
        self.contained_placeholder(q).is_some()
    }

    /// The type deduced for a declared type `declared` containing a
    /// placeholder from initializer `e` ([dcl.type.auto.deduct]/4): a
    /// top-level placeholder as [`Self::deduce_placeholder`]; otherwise
    /// template argument deduction from a call ([temp.deduct.call]/2-4)
    /// with `auto` as the template parameter, restricted to the pointer
    /// and reference declarators [`Self::contains_undeduced_auto`]
    /// admits. `None` when deduction fails (the caller reports).
    pub(crate) fn deduce_from(&mut self, declared: QualType, e: ExprId) -> Option<QualType> {
        if self.undeduced_keyword(declared).is_some() {
            return Some(self.deduce_placeholder(declared, e));
        }
        if self.is_err_expr(e) {
            return Some(QualType::ERROR);
        }
        let n = *self.ast.exprs.get(e);
        let ety = n.ty();
        let t = &self.ast.types;
        // The declarator's own reference node keeps its pointee's sugar
        // (`auto (&r) = x;` is `int (&)`, WP-191).
        let view = match t.view(declared.ty()) {
            v @ (TypeView::LValueRef(_) | TypeView::RValueRef(_)) => v,
            _ => t.canonical_view(declared),
        };
        match view {
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                let rref = matches!(t.canonical_view(declared), TypeView::RValueRef(_));
                // A forwarding reference `auto &&` bound to an lvalue
                // deduces `A &` ([temp.deduct.call]/3); the reference
                // collapses ([dcl.ref]/7).
                if rref
                    && n.value_category() == ValueCategory::LValue
                    && self.undeduced_keyword(p).is_some()
                    && t.canonical(p).quals().is_empty()
                {
                    return Some(self.ast.types.lvalue_ref_to(ety));
                }
                let inner = self.deduce_match(p, ety)?;
                Some(if rref {
                    self.ast.types.rvalue_ref_to(inner)
                } else {
                    self.ast.types.lvalue_ref_to(inner)
                })
            }
            _ => {
                // A non-reference P: A decays and loses its top-level cv
                // ([temp.deduct.call]/2).
                let a = if t.is_array(ety) {
                    let el = t.element(ety).unwrap_or(QualType::ERROR);
                    self.ast.types.pointer_to(el)
                } else if t.is_function(ety) {
                    self.ast.types.pointer_to(ety)
                } else {
                    crate::conv::strip_cv(self, ety)
                };
                self.deduce_match(declared, a)
            }
        }
    }

    /// Matches `p` (pointers down to a placeholder) against `a`, returning
    /// `p` with the placeholder replaced; the placeholder deduces `a`
    /// without the cv-qualifiers written on it ([temp.deduct.call]/4.2
    /// allows the extra qualification). `p`'s own top-level cv is the
    /// caller's.
    fn deduce_match(&mut self, p: QualType, a: QualType) -> Option<QualType> {
        let t = &self.ast.types;
        let pq = t.canonical(p).quals();
        // Declarator parentheses and pointers are matched as written, so
        // their sugar survives (`auto (*p) = &x;` is `int (*)`, WP-191).
        let view = match t.view(p.ty()) {
            TypeView::Paren(inner) => {
                let r = self.deduce_match(inner, a)?;
                let r = self.ast.types.paren_type(r);
                return Some(if p.quals().is_empty() {
                    r
                } else {
                    self.ast.types.qualify(r, p.quals())
                });
            }
            v @ (TypeView::Pointer(_) | TypeView::Function(_)) => v,
            _ => t.canonical_view(p),
        };
        match view {
            TypeView::Auto { keyword, deduced: None, concept, args, pack } => {
                let aq = self.ast.types.effective_quals(a);
                let rest = Quals::from_bits(aq.bits() & !pq.bits());
                let base = crate::conv::strip_cv(self, a);
                let mut leaf =
                    if rest.is_empty() { base } else { self.ast.types.qualify(base, rest) };
                // An array keeps the deduced `auto` as sugar, as clang's
                // `AutoType` does: the declarator then prints without
                // parentheses (`int &[2]`, not `int (&)[2]`).
                // The cv written on the placeholder comes off the element
                // type ([basic.type.qualifier]/3: an array's cv is its
                // element's).
                if self.ast.types.is_array(leaf) {
                    let c = self.ast.types.canonical(a);
                    let u = self.ast.types.unqualified(c);
                    let u = self.ast.types.canonical(u);
                    let el = if rest.is_empty() { u } else { self.ast.types.qualify(u, rest) };
                    leaf = self.ast.types.auto_type(keyword, Some(el), concept, args, pack);
                }
                Some(if pq.is_empty() { leaf } else { self.ast.types.qualify(leaf, pq) })
            }
            TypeView::Pointer(pp) => {
                let TypeView::Pointer(ap) = t.canonical_view(a) else { return None };
                let inner = self.deduce_match(pp, ap)?;
                let r = self.ast.types.pointer_to(inner);
                Some(if pq.is_empty() { r } else { self.ast.types.qualify(r, pq) })
            }
            // Below a declarator: a reference matches exactly.
            TypeView::LValueRef(pp) | TypeView::RValueRef(pp) => {
                let lref = matches!(view, TypeView::LValueRef(_));
                let inner = match t.canonical_view(a) {
                    TypeView::LValueRef(ap) if lref => self.deduce_match(pp, ap)?,
                    TypeView::RValueRef(ap) if !lref => self.deduce_match(pp, ap)?,
                    _ => return None,
                };
                Some(if lref {
                    self.ast.types.lvalue_ref_to(inner)
                } else {
                    self.ast.types.rvalue_ref_to(inner)
                })
            }
            TypeView::Function(pf) => self.deduce_match_function(pf, a),
            _ => None,
        }
    }

    /// [`Self::deduce_match`] for a function type `pf` whose return type
    /// holds the placeholder (`auto (*p)(int) -> auto = f;`,
    /// [temp.deduct.type]/8 `T (T)`): `a` must be a function type with
    /// the same parameters, variadic-ness, cv- and ref-qualifiers; the
    /// return types match recursively. The result keeps `pf`'s sugar and
    /// exception specification (a non-throwing `a` converts to a
    /// potentially-throwing `pf`, [temp.deduct.call]/4.3, not back).
    fn deduce_match_function(
        &mut self,
        pf: kiln_ast::types::FuncProto,
        a: QualType,
    ) -> Option<QualType> {
        let t = &self.ast.types;
        let TypeView::Function(af) = t.canonical_view(a) else { return None };
        let pinfo = pf.info().with_trailing_return(false);
        let ainfo = af.info().with_trailing_return(false);
        let pexc = t.canonical_exc(t.exc_spec(&pf));
        let aexc = t.canonical_exc(t.exc_spec(&af));
        let canon_params: Vec<QualType> =
            t.params(&pf).iter().map(|&x| t.canonical(x).without_quals()).collect();
        if pinfo != ainfo
            || canon_params.as_slice() != t.params(&af)
            || (pexc != aexc && aexc != kiln_ast::types::ExcSpec::Nothrow)
        {
            return None;
        }
        let ret = self.deduce_match(pf.ret(), af.ret())?;
        let declared_params = self.ast.types.params(&pf).to_vec();
        let exc = self.ast.types.exc_spec(&pf);
        Some(self.ast.types.function(ret, &declared_params, pf.info(), exc))
    }

    /// `return e;` (or `return;` when `value` is `None`) in a function
    /// with a deduced return type ([dcl.spec.auto.general]/9,
    /// [dcl.type.auto.deduct]/2.3). Deduces the return type from the
    /// first return statement and checks later ones against it; returns
    /// the (deduced) return type to convert `value` to. `value` has not
    /// been through the implicit move yet.
    pub(crate) fn deduce_return(&mut self, value: Option<ExprId>, loc: Loc) -> QualType {
        let Some(state) = self.func.as_ref() else { return QualType::ERROR };
        let (fdecl, ret, done) = (state.decl, state.ret, state.deduced);
        let Some(declared) = state.auto_ret else { return ret };
        let kw = self.undeduced_keyword(declared).unwrap_or(AutoKeyword::Auto);
        // In a templated function (a dependent context) deduction happens
        // when the definition is instantiated, even from a return statement
        // with a non-type-dependent operand ([dcl.spec.auto.general]/12;
        // clang's `DeduceFunctionTypeFromReturnExpr` skips a dependent
        // context). Only the instantiation's non-discarded returns then
        // participate ([stmt.if]/2), so `if constexpr (c) return; else
        // return 1;` deduces per specialization. The declared type keeps
        // its placeholder (clang prints `auto`).
        if self.in_template() || value.is_some_and(|e| self.expr_is_type_dependent(e)) {
            // In the pattern of an expansion statement of a non-template
            // function, its expansions deduce.
            let templated = self.in_template_outside_expansions();
            if let Some(st) = self.func.as_mut()
                && templated
                && !st.deduced
            {
                st.deduced = true;
                st.ret = QualType::DEPENDENT;
            }
            return QualType::DEPENDENT;
        }
        let deduced = match value {
            None => QualType::VOID,
            Some(e) if kw == AutoKeyword::DecltypeAuto => {
                // An implicitly movable entity named by a parenthesized
                // id-expression is an xvalue ([expr.prim.id.unqual]/4,
                // C++23): `decltype((x))` is then `T&&`. Before C++23 it
                // stays an lvalue (`T&`), as in clang.
                let moved = self.implicit_move(e);
                if self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr
                    || self.std < kiln_lex::Std::Cxx23
                {
                    self.decltype_of(e)
                } else {
                    self.decltype_of(moved)
                }
            }
            Some(e) => {
                if let Some(t) = self.deduce_from(declared, e) {
                    t
                } else {
                    let msg = self
                        .diag(DiagId::sema_auto_fn_deduction_failure, self.ast.exprs.get(e).loc())
                        .arg(self.ty_arg(declared))
                        .arg(self.ty_arg(self.ast.exprs.get(e).ty()));
                    self.emit(msg);
                    QualType::ERROR
                }
            }
        };
        if !done {
            if !self.is_error_type(deduced) && !self.ast.types.is_dependent(deduced) {
                // An instantiation's diagnostic points at its pattern's
                // type-constraint.
                let pattern = self.ast.instantiated_from(fdecl).or_else(|| {
                    let (_, s) = self.ast.spec_info(fdecl)?;
                    let from = s.from;
                    self.pattern_of(from)
                });
                let locs = &self.inst.concepts.placeholder_locs;
                let at = locs
                    .get(&fdecl)
                    .or_else(|| pattern.and_then(|p| locs.get(&p)))
                    .copied()
                    .unwrap_or(self.ast.decls.get(fdecl).loc());
                let _ = self.check_placeholder_constraint(declared, deduced, at);
            }
            self.set_deduced_return(fdecl, deduced);
            return deduced;
        }
        if !self.is_error_type(deduced)
            && !self.is_error_type(ret)
            && !self.ast.types.is_dependent(ret)
            && self.ast.types.canonical(deduced) != self.ast.types.canonical(ret)
        {
            let sel = u8::from(kw == AutoKeyword::DecltypeAuto);
            let msg = self
                .diag(DiagId::sema_auto_fn_different_deductions, loc)
                .arg(sel)
                .arg(self.ty_arg(deduced))
                .arg(self.ty_arg(ret));
            self.emit(msg);
            // clang drops the return statement (`StmtError`): its operand
            // is not converted to the earlier deduction.
            return QualType::ERROR;
        }
        ret
    }

    /// Records `ret` as the deduced return type of function `f` and of
    /// all its redeclarations (clang's `adjustDeducedFunctionResultType`
    /// updates every redeclaration's type); the declared placeholder
    /// return type is kept for matching later redeclarations
    /// ([`Sema::declared_fn_type`]).
    pub(crate) fn set_deduced_return(&mut self, f: DeclId, ret: QualType) {
        if let Some(state) = self.func.as_mut()
            && state.decl == f
        {
            state.ret = ret;
            state.deduced = true;
        }
        let Some(fty) = self.ast.decls.get(f).ty() else { return };
        let TypeView::Function(p) = self.ast.types.view(fty.ty()) else { return };
        let params = self.ast.types.params(&p).to_vec();
        let exc = self.ast.types.exc_spec(&p);
        let first = self.ast.first(f);
        let placeholder = self.contains_undeduced_auto(p.ret());
        if placeholder {
            self.declared_returns.entry(first).or_insert(p.ret());
        }
        let nty = self.ast.types.function(ret, &params, p.info(), exc);
        let redecls: Vec<DeclId> = self.ast.redecls_of(f).collect();
        let deduced = kiln_ast::decl::FnSpec::new().with_return_deduced(true);
        for d in redecls {
            self.ast.decls.set_ty(d, nty);
            // The linkage looks at the type as written (clang
            // `getLVForNamespaceScopeDecl`), which a deduced return type
            // is not part of.
            if placeholder && self.ast.decls.function(d).is_some() {
                self.ast.decls.mark_function(d, deduced);
            }
        }
        self.finish_source_locs(f);
    }

    /// The type of function `f` as declared: with the placeholder return
    /// type it was declared with when that has been deduced since
    /// ([dcl.spec.auto.general]/13: a redeclaration uses the placeholder,
    /// not the deduced type; clang's `getDeclaredReturnType`).
    pub(crate) fn declared_fn_type(&mut self, f: DeclId) -> Option<QualType> {
        let fty = self.ast.decls.get(f).ty()?;
        let Some(&ret) = self.declared_returns.get(&self.ast.first(f)) else { return Some(fty) };
        let TypeView::Function(p) = self.ast.types.view(fty.ty()) else { return Some(fty) };
        let params = self.ast.types.params(&p).to_vec();
        let exc = self.ast.types.exc_spec(&p);
        Some(self.ast.types.function(ret, &params, p.info(), exc))
    }

    /// A redeclaration of type `ty` (declared with a placeholder return
    /// type) of function `prev`, whose return type was deduced: `ty` with
    /// the deduced return type (clang's `MergeFunctionDecl`).
    pub(crate) fn adopt_deduced_return(&mut self, prev: DeclId, ty: QualType) -> QualType {
        let Some(prev_ty) = self.ast.decls.get(prev).ty() else { return ty };
        let Some(deduced) = self.ast.types.proto(prev_ty).map(kiln_ast::types::FuncProto::ret)
        else {
            return ty;
        };
        let TypeView::Function(p) = self.ast.types.view(ty.ty()) else { return ty };
        if !self.contains_undeduced_auto(p.ret()) || self.contains_undeduced_auto(deduced) {
            return ty;
        }
        let params = self.ast.types.params(&p).to_vec();
        let exc = self.ast.types.exc_spec(&p);
        self.ast.types.function(deduced, &params, p.info(), exc)
    }

    /// Uses of a function whose return type is still undeduced
    /// ([dcl.spec.auto.general]/11): reports and returns true.
    /// A member of a class template specialization is instantiated first
    /// (clang's `DeduceReturnType`; [dcl.spec.auto.general]/12 deduces
    /// when the definition is instantiated).
    pub(crate) fn check_undeduced_use(&mut self, d: DeclId, loc: Loc) -> bool {
        let undeduced = |s: &Self| {
            let fty = s.ast.decls.get(d).ty()?;
            let ret = s.ast.types.proto(fty).map(kiln_ast::types::FuncProto::ret)?;
            s.contains_undeduced_auto(ret).then_some(())
        };
        if undeduced(self).is_none() {
            return false;
        }
        if self.ast.instantiated_from(d).is_some() {
            self.instantiate_function_definition(d, loc);
            if undeduced(self).is_none() {
                return false;
            }
        }
        if self.undeduced_dependent_use(d) {
            return false;
        }
        let name = self.decl_name(d);
        let note = self
            .diag(DiagId::note_entity_declared_at, self.ast.decls.get(d).loc())
            .arg(format!("'{name}'"));
        let msg =
            self.diag(DiagId::sema_auto_fn_used_before_defined, loc).arg(name).with_note(note);
        self.emit(msg);
        true
    }

    /// True if a use of `d` inside a template definition, whose return
    /// type is still undeduced, is dependent: `d` is declared in the
    /// templated class being defined, so its definition is instantiated
    /// (and the type deduced) with each specialization. The reference is
    /// type-dependent (clang accepts `using C = decltype(get());` after
    /// `static auto get() { ... }` in a class template; libc++
    /// `move_iterator::__get_iter_concept`). So is a specialization of
    /// a member function template of that class (`compute<true>()` in a
    /// static member's initializer, before the member template's body is
    /// parsed; Boost.Hana `detail::flatten_indices`): its enclosing
    /// arguments are the class's dependent ones.
    pub(crate) fn undeduced_dependent_use(&self, d: DeclId) -> bool {
        let dependent = |x: DeclId| self.ast.decls.get(x).has(DeclFlags::DEPENDENT_CTX);
        self.in_template()
            && (dependent(d) || self.spec_args(d).is_some_and(|(t, _)| dependent(t)))
            && self.ast.decls.get(d).ty().is_some_and(|fty| {
                self.ast.types.proto(fty).is_some_and(|p| self.contains_undeduced_auto(p.ret()))
            })
    }
}
