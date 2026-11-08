//! Temporaries and cleanups ([class.temporary], [intro.execution]/5;
//! spec/sema-class.md §5.3-§5.5): temporary materialization, binding of
//! temporaries with non-trivial destructors, the `ExprWithCleanups` of a
//! full-expression, lifetime extension ([class.temporary]/6) and the
//! diagnostics of temporaries outliving their full-expression.
//!
//! Lifetime extension is decided after an initializer is built, as clang
//! does (`checkExprLifetime`): the `MaterializeTemporaryExpr`s a reference
//! (or an aggregate's reference members) binds are found by
//! [`Sema::temporaries_bound_by`] and extended in place
//! (`ExprArena::extend_temporary`, spec/sema-class.md §8.2).

use kiln_ast::decl::{
    ConstexprKind, DeclFlags, DeclKind, Defaulted, DefinitionBits, TlsKind, VarStorage,
};
use kiln_ast::expr::{
    BinaryOp, CastKind, ExprDeps, ExprFlags, ExprKind, StorageDuration, ValueCategory,
};
use kiln_ast::types::Quals;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::special::SpecialKind;
use crate::Sema;

mod stack_addr;

/// Where a reference is bound, for the diagnostics of a temporary that
/// would not live long enough (clang `checkExprLifetime`'s entity kinds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindingSite {
    /// A `return` operand of a function returning a reference.
    Return,
    /// A mem-initializer of reference member `field`.
    Member(DeclId),
}

impl Sema<'_, '_> {
    /// Temporary materialization ([conv.rval]) of the prvalue `e` as a
    /// glvalue of type `ty` (an lvalue if `lvalue_ref`): the
    /// full-expression needs cleanups (clang
    /// `CreateMaterializeTemporaryExpr`).
    pub(crate) fn materialize_temporary(
        &mut self,
        e: ExprId,
        ty: QualType,
        lvalue_ref: bool,
    ) -> ExprId {
        self.cleanup.needs = true;
        let m = self.builder().materialize_temporary(
            e,
            ty,
            lvalue_ref,
            StorageDuration::FullExpression,
            None,
        );
        // P2718: a temporary of a for-range-initializer is extended to
        // `__range` (spec/sema-class.md §5.4).
        if self.unevaluated == 0
            && let Some(c) = self.extend_collector.as_mut()
        {
            c.push(m);
        }
        m
    }

    /// clang `MaybeBindToTemporary`: a class (or array of class) prvalue
    /// `e` whose class has a non-trivial destructor is bound to a temporary destroyed at the end
    /// of the full-expression ([class.temporary]/4). The destructor must be
    /// accessible and not deleted ([class.dtor]/15) and is odr-used.
    pub(crate) fn maybe_bind_temporary(&mut self, e: ExprId) -> ExprId {
        let n = *self.ast.exprs.get(e);
        if n.value_category() != ValueCategory::PRValue
            || n.deps().contains(ExprDeps::TYPE)
            || n.kind() == ExprKind::CXXBindTemporaryExpr
        {
            return e;
        }
        // The call of a conversion function is bound when it is built, so
        // its `UserDefinedConversion` cast (or a no-op cast of that) is
        // already a bound temporary: binding it again destroys the object
        // twice (`S{tl}` with `TL::operator S()`, CWG2327; WP-811).
        let mut inner = e;
        while self.ast.exprs.get(inner).kind() == ExprKind::ImplicitCastExpr
            && matches!(
                self.ast.exprs.cast_kind(inner),
                CastKind::UserDefinedConversion | CastKind::NoOp
            )
        {
            inner = self.ast.exprs.operand(inner);
            if self.ast.exprs.get(inner).kind() == ExprKind::CXXBindTemporaryExpr {
                return e;
            }
        }
        // An array temporary binds the destructor of its base element
        // class (clang searches through the array types).
        let mut base = n.ty();
        while self.ast.types.is_array(base) {
            base = self.ast.types.element(base).unwrap_or(QualType::ERROR);
        }
        let Some(r) = self.ast.types.as_record(base) else { return e };
        let Some(class) = self.ast.definition(r) else { return e };
        let flags = self.ast.decls.get(class).flags();
        if flags.contains(DeclFlags::INVALID) || flags.contains(DeclFlags::DEPENDENT_CTX) {
            return e;
        }
        // clang checks the destructor's access and deletion even when it
        // is trivial (then it binds nothing); an implicit trivial
        // destructor passes both.
        let bits = self.defn_bits(class);
        let trivial = bits.contains(DefinitionBits::TRIVIAL_DTOR);
        if trivial
            && !bits.contains(DefinitionBits::USER_DECLARED_DTOR)
            && !bits.contains(DefinitionBits::DEFAULTED_DTOR_DELETED)
        {
            return e;
        }
        // [dcl.type.decltype]/2: a call operand of `decltype` makes no
        // temporary; `exit_decltype` checks the destructor of every such
        // call but the operand itself (clang `DelayedDecltypeBinds`).
        if matches!(
            n.kind(),
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr
        ) && self.decltype_calls.last().is_some_and(|x| x.0 == self.unevaluated)
        {
            return e;
        }
        let loc = self.begin_loc(e);
        let Some(dtor) = self.check_temporary_dtor(class, n.ty(), loc) else { return e };
        if trivial {
            return e;
        }
        self.cleanup.needs = true;
        self.cleanup.side_effects = true;
        self.builder().bind_temporary(e, dtor)
    }

    /// The destructor checks of a temporary of class `class` (type `ty`)
    /// at `loc`: accessible, not deleted, odr-used. The destructor, if
    /// one is selected.
    pub(crate) fn check_temporary_dtor(
        &mut self,
        class: DeclId,
        ty: QualType,
        loc: Loc,
    ) -> Option<DeclId> {
        let (super::special::Selected::Found(dtor) | super::special::Selected::Deleted(dtor)) =
            self.select_special(class, SpecialKind::Dtor, Quals::NONE, Quals::NONE)
        else {
            return None;
        };
        let msg = self.special_member_access(dtor, class, |s, prot| {
            s.diag(DiagId::sema_temp_dtor_access, loc).arg(s.ty_arg(ty)).arg(u8::from(prot))
        });
        if let Some(msg) = msg {
            self.emit(msg);
        }
        self.mark_function_referenced(dtor, loc);
        self.diagnose_deleted_use(dtor, loc);
        Some(dtor)
    }

    /// clang `MarkFunctionReferenced` for a use of function `f` at `loc`
    /// ([basic.def.odr]/7): an implicitly defined (defaulted, not deleted)
    /// special member is defined now (spec/sema-class.md §4.5), except a
    /// trivial default constructor (only odr-used) or destructor (not
    /// even odr-used); then `f` is odr-used.
    ///
    /// In a discarded statement `f` is not odr-used ([stmt.if]/2,
    /// [`Sema::in_discarded_statement`]): only a constexpr `f` is defined
    /// or instantiated, as constant evaluation may need it (clang's
    /// `NeededForConstantEvaluation` through
    /// `isImplicitlyDefinableConstexprFunction`), and `f` is not marked
    /// used. In a template definition only a constexpr defaulted special
    /// member is defined, for the same reason.
    pub(crate) fn mark_function_referenced(&mut self, f: DeclId, loc: Loc) {
        self.ast.decls.set_flags(f, DeclFlags::REFERENCED);
        self.define_cmp_on_use(f, loc);
        self.exc_spec_needed(f, loc);
        self.require_annotations(f, loc);
        if self.unevaluated > 0 {
            return;
        }
        if self.in_template() {
            self.define_constexpr_special_in_template(f, loc);
            return;
        }
        let first = self.ast.first(f);
        if self.func.as_ref().is_some_and(|s| self.ast.first(s.decl) == first) {
            // A recursive use from the function's own body.
            return;
        }
        let discarded = self.in_discarded_statement();
        if discarded && !self.declared_constexpr(first) {
            return;
        }
        if self.inherited_ctor_base(first).is_some() {
            self.define_inheriting_ctor(first, loc);
        } else if let Some(kind) = self.special_kind_of(first) {
            let spec = self.ast.decls.function(first).map(|x| x.spec).unwrap_or_default();
            if spec.defaulted() != Defaulted::No && !spec.is_deleted() {
                // clang 19 odr-uses a trivial default constructor without
                // defining it, and does not even odr-use a trivial
                // destructor.
                match kind {
                    SpecialKind::Dtor if spec.is_trivial() => return,
                    SpecialKind::DefaultCtor if spec.is_trivial() => {}
                    _ => self.define_implicit_special(first, kind, loc),
                }
            }
        }
        self.define_lambda_conversion(first, loc);
        if discarded {
            // Instantiated now for constant evaluation, but not used.
            self.note_used_for_instantiation(f, loc);
            return;
        }
        // A member function of a class template specialization is
        // instantiated ([temp.inst]/5).
        self.mark_used_at(f, loc);
    }

    /// A use of `f` in a template definition: `f` is not odr-used, but a
    /// constexpr defaulted special member of a non-dependent class is
    /// defined, as a constant expression in the template (a `constexpr`
    /// or `constinit` initializer) is evaluated at its definition (clang's
    /// `NeededForConstantEvaluation` through
    /// `isImplicitlyDefinableConstexprFunction`).
    fn define_constexpr_special_in_template(&mut self, f: DeclId, loc: Loc) {
        let first = self.ast.first(f);
        let Some(kind) = self.special_kind_of(first) else { return };
        let spec = self.ast.decls.function(first).map(|x| x.spec).unwrap_or_default();
        if spec.defaulted() == Defaulted::No || spec.is_deleted() || !self.declared_constexpr(first)
        {
            return;
        }
        match kind {
            SpecialKind::Dtor | SpecialKind::DefaultCtor if spec.is_trivial() => {}
            // Defined outside the template (as at an instantiation):
            // in the template's context the members it uses would only
            // be referenced, never odr-used, and stay undefined when a
            // later use finds it already defined.
            _ => {
                let Some(class) = self.ast.decls.get(first).ctx() else { return };
                self.in_inst_context(class, |s| s.define_implicit_special(first, kind, loc));
            }
        }
    }

    /// Whether some declaration of function `f` is `constexpr` or
    /// `consteval`.
    fn declared_constexpr(&self, f: DeclId) -> bool {
        self.ast.redecls_of(f).any(|r| self.fn_spec(r).constexpr_kind() != ConstexprKind::None)
    }

    /// The end of a full-expression: wraps `e` in an `ExprWithCleanups`
    /// if it materialized or bound a temporary, and resets the state.
    pub(crate) fn finish_cleanups(&mut self, e: ExprId) -> ExprId {
        let st = std::mem::take(&mut self.cleanup);
        if !st.needs || self.ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups {
            return e;
        }
        self.builder().expr_with_cleanups(e, st.side_effects)
    }

    /// A subexpression `e` that template instantiation reuses unchanged
    /// (a non-dependent operand of a rebuilt full-expression): the
    /// temporaries it materializes or binds, and those of the default
    /// arguments it uses, outside a nested `ExprWithCleanups` or lambda,
    /// belong to the full-expression being built, as if `e` had been
    /// built again ([intro.execution]/5; clang's `TreeTransform` rebuilds
    /// the operand, and `MaybeBindToTemporary` and
    /// `CreateMaterializeTemporaryExpr` set `ExprNeedsCleanups`). Without
    /// this, `B b(S{42});` in a function template loses its
    /// `ExprWithCleanups`, and the `S` temporary outlives the declaration.
    pub(crate) fn note_shared_temporaries(&mut self, e: ExprId) {
        if (self.cleanup.needs && self.cleanup.side_effects) || self.unevaluated > 0 {
            return;
        }
        let mut stack = self.take_walk_stack(e);
        while let Some(x) = stack.pop() {
            match self.ast.exprs.get(x).kind() {
                ExprKind::MaterializeTemporaryExpr => {
                    self.cleanup.needs = true;
                    self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
                }
                ExprKind::CXXBindTemporaryExpr => {
                    self.cleanup.needs = true;
                    self.cleanup.side_effects = true;
                    break;
                }
                // As `note_default_use` does for a default argument it
                // builds.
                ExprKind::CXXDefaultArgExpr => {
                    let p = self.ast.exprs.default_arg_param(x);
                    if let Some(d) = self.ast.decls.param(p).and_then(|p| p.default_arg)
                        && self.ast.exprs.get(d).kind() == ExprKind::ExprWithCleanups
                    {
                        self.cleanup.needs = true;
                        self.cleanup.side_effects |= self.ast.exprs.cleanups_have_side_effects(d);
                    }
                }
                // Unevaluated operands ([expr.context]/1) and lambda
                // bodies own no temporaries of this full-expression.
                ExprKind::ExprWithCleanups
                | ExprKind::LambdaExpr
                | ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::ExpressionTraitExpr
                | ExprKind::RequiresExpr
                | ExprKind::ConceptSpecializationExpr => {}
                ExprKind::CXXTypeidExpr if !self.ast.exprs.typeid_potentially_evaluated(x) => {}
                _ => self.ast.exprs.children_into(x, &self.ast.lists, &mut stack),
            }
        }
        self.put_walk_stack(stack);
    }

    /// Enters a context whose temporaries belong to no enclosing
    /// full-expression (an unevaluated operand).
    pub(crate) fn push_cleanups(&mut self) {
        let st = std::mem::take(&mut self.cleanup);
        self.cleanup_saved.push(st);
    }

    /// Leaves the context of [`Sema::push_cleanups`]: its temporaries are
    /// discarded (clang restores `ParentCleanup` for unevaluated contexts).
    pub(crate) fn pop_cleanups(&mut self) {
        self.cleanup = self.cleanup_saved.pop().unwrap_or_default();
    }

    /// The storage duration of variable `v` ([basic.stc]), the duration
    /// of the temporaries it extends.
    fn var_storage(&self, v: DeclId) -> StorageDuration {
        let Some(data) = self.ast.decls.var(v) else { return StorageDuration::Automatic };
        if data.spec.tls() != TlsKind::None {
            return StorageDuration::Thread;
        }
        let local = self.ast.decls.get(v).ctx().is_some_and(|c| self.kind_of(c).is_function());
        if !local || matches!(data.spec.storage(), VarStorage::Static | VarStorage::Extern) {
            StorageDuration::Static
        } else {
            StorageDuration::Automatic
        }
    }

    /// The initializer `init` of variable `v` was built: extends the
    /// temporaries bound to `v` (a reference) or to the reference members
    /// of the aggregate `v` ([class.temporary]/6).
    pub(crate) fn extend_temporaries(&mut self, v: DeclId, init: ExprId) {
        let vty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        let mut found = Vec::new();
        if self.ast.types.is_reference(vty) {
            self.temporaries_bound_by(init, &mut found, 0);
        } else {
            self.temporaries_in_aggregate(init, &mut found, 0);
        }
        if found.is_empty() {
            return;
        }
        let storage = self.var_storage(v);
        for (i, mte) in found.into_iter().enumerate() {
            let n = *self.ast.exprs.get(mte);
            let index = u32::try_from(i).unwrap_or(u32::MAX);
            let let_decl = self.builder().lifetime_extended_temporary(v, index, n.ty(), n.loc());
            let _ = self.ast.exprs.extend_temporary(mte, let_decl, storage);
        }
    }

    /// The returned-reference-to-temporary diagnostic: P2748R5 made it
    /// ill-formed in C++26 ([stmt.return]/6); clang warns
    /// (`-Wreturn-stack-address`) in earlier modes.
    pub(crate) fn return_temp_ref_id(&self) -> DiagId {
        if self.std >= kiln_lex::Std::Cxx26 {
            DiagId::sema_temp_return_local
        } else {
            DiagId::sema_return_local_temp_ref
        }
    }

    /// The diagnostics of a reference bound at `site` to a temporary that
    /// would be destroyed first ([class.temporary]/6.2, 6.11;
    /// [stmt.return]/6): `returning reference to local temporary object`
    /// and the reference-member error. `init` is the reference's
    /// initializer.
    pub(crate) fn check_dangling_temporary(&mut self, site: BindingSite, init: ExprId) {
        let mut found = Vec::new();
        self.temporaries_bound_by(init, &mut found, 0);
        let Some(&mte) = found.first() else { return };
        let loc = self.begin_loc(mte);
        match site {
            BindingSite::Return => {
                // [stmt.return]/6 (P2748): ill-formed; clang points at the
                // temporary (`checkExprLifetime`'s `DiagRange` is the
                // local's range): the operand of `static_cast<const int&>(c)`.
                // Before C++26 it is `-Wreturn-stack-address`.
                let msg = self.diag(self.return_temp_ref_id(), loc);
                self.emit(msg);
            }
            BindingSite::Member(field) => {
                let msg =
                    self.diag(DiagId::sema_temp_member_ref, loc)
                        .arg(self.decl_arg(field))
                        .with_note(self.diag(
                            DiagId::note_temp_member_ref_here,
                            self.ast.decls.get(field).loc(),
                        ));
                self.emit(msg);
            }
        }
    }

    /// The temporaries a reference initialized by the glvalue `e` binds
    /// ([class.temporary]/6: through parentheses, no-op and
    /// derived-to-base casts, access to a non-reference member, array subscripts of
    /// array temporaries, the right operand of a comma and both arms of a
    /// conditional), followed by those bound by the reference members of
    /// an aggregate temporary.
    pub(crate) fn temporaries_bound_by(&self, e: ExprId, out: &mut Vec<ExprId>, depth: u32) {
        if depth > 256 {
            return;
        }
        let n = *self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr | ExprKind::ExprWithCleanups | ExprKind::ConstantExpr => {
                self.temporaries_bound_by(self.ast.exprs.operand(e), out, depth + 1);
            }
            k if k.is_cast()
                && matches!(
                    self.ast.exprs.cast_kind(e),
                    CastKind::NoOp | CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase
                ) =>
            {
                self.temporaries_bound_by(self.ast.exprs.operand(e), out, depth + 1);
            }
            ExprKind::MemberExpr if !n.has(ExprFlags::ARROW) => {
                // [class.temporary]/6: a member access designating a
                // non-static data member of non-reference type; a
                // reference member denotes what it was bound to.
                let m = self.ast.exprs.decl(e);
                let mty = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
                if self.kind_of(m) == DeclKind::Field && !self.ast.types.is_reference(mty) {
                    self.temporaries_bound_by(self.ast.exprs.operand(e), out, depth + 1);
                }
            }
            ExprKind::ArraySubscriptExpr => {
                let [base, _] = self.ast.exprs.operands(e);
                let mut b = base;
                while self.ast.exprs.get(b).kind() == ExprKind::ParenExpr {
                    b = self.ast.exprs.operand(b);
                }
                if self.ast.exprs.get(b).kind() == ExprKind::ImplicitCastExpr
                    && self.ast.exprs.cast_kind(b) == CastKind::ArrayToPointerDecay
                {
                    self.temporaries_bound_by(self.ast.exprs.operand(b), out, depth + 1);
                }
            }
            ExprKind::BinaryOperator if self.ast.exprs.binary_op(e) == BinaryOp::Comma => {
                self.temporaries_bound_by(self.ast.exprs.operands(e)[1], out, depth + 1);
            }
            ExprKind::ConditionalOperator => {
                let [t, f] = self.ast.exprs.cond_arms(e);
                self.temporaries_bound_by(t, out, depth + 1);
                self.temporaries_bound_by(f, out, depth + 1);
            }
            ExprKind::MaterializeTemporaryExpr => {
                if self.ast.exprs.lifetime_extended_temporary(e).is_none() {
                    out.push(e);
                }
                self.temporaries_in_aggregate(self.ast.exprs.operand(e), out, depth + 1);
            }
            _ => {}
        }
    }

    /// The temporaries bound by the reference members of the aggregate
    /// prvalue `e` (an `InitListExpr` or parenthesized aggregate
    /// initialization, through no-op casts), recursively.
    fn temporaries_in_aggregate(&self, e: ExprId, out: &mut Vec<ExprId>, depth: u32) {
        if depth > 256 {
            return;
        }
        let n = *self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr
            | ExprKind::ExprWithCleanups
            | ExprKind::CXXBindTemporaryExpr
            | ExprKind::ConstantExpr => {
                self.temporaries_in_aggregate(self.ast.exprs.operand(e), out, depth + 1);
            }
            k if k.is_cast() && self.ast.exprs.cast_kind(e) == CastKind::NoOp => {
                self.temporaries_in_aggregate(self.ast.exprs.operand(e), out, depth + 1);
            }
            // Both arms of a prvalue conditional (clang's
            // `visitLocalsRetainedByInitializer`).
            ExprKind::ConditionalOperator => {
                let [t, f] = self.ast.exprs.cond_arms(e);
                self.temporaries_in_aggregate(t, out, depth + 1);
                self.temporaries_in_aggregate(f, out, depth + 1);
            }
            // The backing array of a `std::initializer_list` is extended
            // like a reference bound to it ([dcl.init.list]/6).
            ExprKind::CXXStdInitializerListExpr => {
                self.temporaries_bound_by(self.ast.exprs.operand(e), out, depth + 1);
            }
            ExprKind::InitListExpr | ExprKind::CXXParenListInitExpr => {
                let list = if n.kind() == ExprKind::InitListExpr {
                    self.ast.exprs.init_list_inits(e)
                } else {
                    self.ast.exprs.paren_list_inits(e)
                };
                let inits: Vec<ExprId> = self.ast.lists.iter::<ExprId>(list).collect();
                if self.ast.types.is_array(n.ty()) {
                    for i in inits {
                        self.temporaries_in_aggregate(i, out, depth + 1);
                    }
                    return;
                }
                let Some(r) = self.ast.types.as_record(n.ty()) else { return };
                // The initializers follow the bases, then the fields
                // ([dcl.init.aggr]/2); a union has one.
                let def = self.ast.definition(r).unwrap_or(r);
                let nbases = self.ast.decls.bases(def, &self.ast.lists).count();
                let mut fields = self.init_fields(r);
                if n.kind() == ExprKind::InitListExpr
                    && let Some(f) = self.ast.exprs.init_list(e).union_field
                {
                    fields = vec![f];
                }
                for (i, &init) in inits.iter().enumerate() {
                    if i < nbases {
                        self.temporaries_in_aggregate(init, out, depth + 1);
                        continue;
                    }
                    let Some(&f) = fields.get(i - nbases) else { break };
                    let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                    if self.ast.types.is_reference(fty) {
                        self.temporaries_bound_by(init, out, depth + 1);
                    } else {
                        self.temporaries_in_aggregate(init, out, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }
}
