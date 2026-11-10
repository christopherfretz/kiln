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

mod gsl;
mod lifetime;

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

    /// The discarded-value expression `e` (the operand of a cast to
    /// `void`, the left operand of a comma) in a lifetime-extending
    /// context: a class prvalue undergoes the temporary materialization
    /// conversion ([expr.context]/2, [expr.static.cast]/6, C++17), so
    /// the temporary is one the enclosing declaration extends (clang
    /// `IgnoredValueConversions` under `isInLifetimeExtendingContext`).
    /// The contexts are those with a collector
    /// ([`Sema::begin_for_range_init`]): a for-range-initializer (P2718,
    /// [class.temporary]/7) and an enumerating expansion-init-list
    /// ([class.temporary]/8, CWG3043). Elsewhere the prvalue is only
    /// bound: a discarded temporary is destroyed with its
    /// full-expression ([class.temporary]/6 extends nothing it does not
    /// bind), so materializing it changes nothing. Only class types:
    /// a scalar temporary's lifetime is not observable.
    pub(crate) fn materialize_discarded(&mut self, e: ExprId) -> ExprId {
        if self.extend_collector.is_none()
            || self.unevaluated > 0
            || self.std < kiln_lex::Std::Cxx17
        {
            return e;
        }
        let n = *self.ast.exprs.get(e);
        if n.value_category() != ValueCategory::PRValue
            || n.deps().contains(ExprDeps::TYPE)
            || self.ast.types.as_record(n.ty()).is_none()
        {
            return e;
        }
        self.materialize_temporary(e, n.ty(), false)
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
        // clang's `DiagnoseUseOfDecl` of a deleted destructor makes the
        // expression an error, so nothing initialized from it is checked
        // again (`auto l = [x = B()] {};`, WP-834).
        if self.is_deleted_fn(dtor) {
            return self.recover_err(&[e], loc);
        }
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

    /// [class.temporary]/8-9 (CWG3043): every temporary materialized in
    /// the initializer of the automatic variable `v` that would be
    /// destroyed at the end of its full-expression persists for `v`'s
    /// lifetime instead (an enumerating expansion statement's
    /// for-range-declaration and the element it is initialized from, a
    /// destructuring expansion statement's `auto&& [u...]` and its
    /// expansion-initializer; spec/sema.md §7.6a). Nested
    /// full-expressions (statement expressions, lambda bodies),
    /// immediate invocations and unevaluated operands keep theirs, and
    /// so does an object that is only bound (`CXXBindTemporaryExpr`)
    /// and never materialized, such as a by-value argument. A discarded
    /// class prvalue is materialized when built
    /// ([`Sema::materialize_discarded`]: the expansion-init-list and the
    /// expansion-initializer are lifetime-extending contexts), so it is
    /// extended here too. A default argument's temporaries are created
    /// in the full-expression of its call ([intro.execution]/5) and are
    /// extended as well, but its initializer is shared by every call:
    /// the use gets a copy of its own with fresh temporaries
    /// ([`Sema::fresh_temporaries`], clang's rewritten default argument
    /// in a lifetime-extending context).
    pub(crate) fn extend_init_temporaries(&mut self, v: DeclId) {
        if self.var_storage(v) != StorageDuration::Automatic {
            return;
        }
        let Some(init) = self.ast.decls.var(v).and_then(|x| x.init) else { return };
        self.extend_walk(v, init, false, 0);
    }

    /// The default arguments used in `__range`'s for-range-initializer
    /// `init` (P2718R0, [class.temporary]/7): their temporaries, rebuilt
    /// for the use, persist with `__range` like the others (collected
    /// while parsing; the first `index` of them are numbered).
    pub(crate) fn extend_range_default_args(&mut self, range: DeclId, init: ExprId, index: u32) {
        self.extend_walk(range, init, true, index);
    }

    /// The walk of [`Sema::extend_init_temporaries`]: extends the
    /// full-expression temporaries of `init` to `v` (with `defaults_only`,
    /// only those its default arguments create).
    fn extend_walk(&mut self, v: DeclId, init: ExprId, defaults_only: bool, mut index: u32) {
        let mut root = init;
        {
            let ex = &self.ast.exprs;
            while matches!(ex.get(root).kind(), ExprKind::ExprWithCleanups | ExprKind::ConstantExpr)
            {
                root = ex.operand(root);
            }
        }
        let mut found = Vec::new();
        let mut uses = Vec::new();
        let mut defaults = Vec::new();
        let mut stack = self.take_walk_stack(root);
        // Inside a default argument every temporary is extended.
        let mut collect = !defaults_only;
        loop {
            let ex = &self.ast.exprs;
            while let Some(x) = stack.pop() {
                match ex.get(x).kind() {
                    ExprKind::MaterializeTemporaryExpr => {
                        if ex.temporary_storage(x) == StorageDuration::FullExpression {
                            if collect {
                                found.push(x);
                            }
                        } else if ex.lifetime_extended_temporary(x).is_some() {
                            index += 1;
                        }
                        ex.children_into(x, &self.ast.lists, &mut stack);
                    }
                    // A use rebuilt already (a shared pattern node, or a
                    // nested default argument) holds its own temporaries.
                    ExprKind::CXXDefaultArgExpr => match ex.default_arg_rewritten(x) {
                        Some(r) => defaults.push(r),
                        None => uses.push(x),
                    },
                    // An immediate invocation's temporaries end within its
                    // own constant evaluation ([expr.const]/16).
                    ExprKind::ExprWithCleanups
                    | ExprKind::ConstantExpr
                    | ExprKind::LambdaExpr
                    | ExprKind::StmtExpr
                    | ExprKind::UnaryExprOrTypeTraitExpr
                    | ExprKind::CXXNoexceptExpr
                    | ExprKind::ExpressionTraitExpr
                    | ExprKind::RequiresExpr
                    | ExprKind::ConceptSpecializationExpr => {}
                    ExprKind::CXXTypeidExpr if !ex.typeid_potentially_evaluated(x) => {}
                    _ => ex.children_into(x, &self.ast.lists, &mut stack),
                }
            }
            for u in std::mem::take(&mut uses) {
                let Some(d) = self.ast.default_arg_expr(u) else { continue };
                let Some(r) = self.fresh_temporaries(d, 0) else { continue };
                if self.ast.exprs.set_default_arg_rewritten(u, r).is_ok() {
                    defaults.push(r);
                }
            }
            if defaults.is_empty() {
                break;
            }
            stack.append(&mut defaults);
            collect = true;
        }
        self.put_walk_stack(stack);
        // Codegen and the Oven destroy them in reverse order of
        // construction, after `v`, as they do the temporaries `v` binds;
        // the index only numbers them (an automatic temporary is never
        // mangled).
        for mte in found {
            let n = *self.ast.exprs.get(mte);
            let let_decl = self.builder().lifetime_extended_temporary(v, index, n.ty(), n.loc());
            let _ = self.ast.exprs.extend_temporary(mte, let_decl, StorageDuration::Automatic);
            index = index.saturating_add(1);
        }
    }

    /// A copy of the expression `e` of a default argument, shared by
    /// every call, whose full-expression temporaries are fresh
    /// `MaterializeTemporaryExpr`s (and whose nested default arguments
    /// are rebuilt too), the nodes above them copied and the rest shared;
    /// `None` when it materializes none, or when a node on the way is of
    /// a kind [`kiln_ast::ExprArena::clone_with_children`] does not copy
    /// (its temporaries then stay with the call's full-expression).
    /// Nested full-expressions and unevaluated operands are not entered,
    /// as in [`Sema::extend_init_temporaries`].
    ///
    /// Evaluated kinds left uncopied, whose temporaries stay with the
    /// call's full-expression: `CXXDefaultInitExpr` (the member's
    /// initializer is shared, also outside a default argument; clang 22
    /// rebuilds it: a WP-1166 follow-up) and the vendor builtins
    /// `AtomicExpr`, `ShuffleVectorExpr`, `GenericSelectionExpr` and
    /// `PseudoObjectExpr`. The others cannot hold a temporary of a used
    /// default argument: C-only `DesignatedInitUpdateExpr`;
    /// `ArrayInitLoopExpr` and `CXXInheritedCtorInitExpr` (lambda
    /// captures, implicit members, bindings, inherited constructors);
    /// `CoawaitExpr` and `CoyieldExpr` (ill-formed there, [expr.await]/2,
    /// [expr.yield]/1); dependent and invalid kinds (never used); an
    /// `OpaqueValueExpr` (rebound with its owner).
    fn fresh_temporaries(&mut self, e: ExprId, depth: u32) -> Option<ExprId> {
        const MAX_DEPTH: u32 = 64;
        if depth > MAX_DEPTH {
            return None;
        }
        let ex = &self.ast.exprs;
        let kind = ex.get(e).kind();
        match kind {
            ExprKind::ExprWithCleanups
            | ExprKind::ConstantExpr
            | ExprKind::LambdaExpr
            | ExprKind::StmtExpr
            | ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::ExpressionTraitExpr
            | ExprKind::RequiresExpr
            | ExprKind::ConceptSpecializationExpr => return None,
            ExprKind::CXXTypeidExpr if !ex.typeid_potentially_evaluated(e) => return None,
            ExprKind::MaterializeTemporaryExpr
                if ex.temporary_storage(e) != StorageDuration::FullExpression =>
            {
                return None;
            }
            ExprKind::CXXDefaultArgExpr => {
                let inner = self.ast.default_arg_expr(e)?;
                let r = self.fresh_temporaries(inner, depth + 1)?;
                let u = self.ast.exprs.clone_with_children(e, &mut self.ast.lists, &[])?;
                let _ = self.ast.exprs.set_default_arg_rewritten(u, r);
                return Some(u);
            }
            // `a ?: b`: the condition and the true arm read the common
            // expression through `OpaqueValueExpr`s bound to it, rebound
            // to its copy.
            ExprKind::BinaryConditionalOperator => {
                let [common, cond, t, f] = ex.binary_conditional_operands(e, &self.ast.lists);
                let c2 = self.fresh_temporaries(common, depth + 1);
                let f2 = self.fresh_temporaries(f, depth + 1);
                let (cond, t) = match c2 {
                    Some(c2) => (
                        self.rebind_opaque(cond, common, c2, depth + 1)?,
                        self.rebind_opaque(t, common, c2, depth + 1)?,
                    ),
                    None if f2.is_some() => (cond, t),
                    None => return None,
                };
                let kids = [c2.unwrap_or(common), cond, t, f2.unwrap_or(f)];
                return self.ast.exprs.clone_with_children(e, &mut self.ast.lists, &kids);
            }
            _ => {}
        }
        let kids = ex.children(e, &self.ast.lists);
        let mut changed = kind == ExprKind::MaterializeTemporaryExpr;
        let mut new = Vec::with_capacity(kids.len());
        for k in kids {
            match self.fresh_temporaries(k, depth + 1) {
                Some(r) => {
                    changed = true;
                    new.push(r);
                }
                None => new.push(k),
            }
        }
        if !changed {
            return None;
        }
        self.ast.exprs.clone_with_children(e, &mut self.ast.lists, &new)
    }

    /// `e` with the `OpaqueValueExpr`s bound to `old` bound to `new`
    /// instead (the nodes above them copied); `None` when a node on the
    /// way is of a kind [`kiln_ast::ExprArena::clone_with_children`] does
    /// not copy.
    fn rebind_opaque(&mut self, e: ExprId, old: ExprId, new: ExprId, depth: u32) -> Option<ExprId> {
        if depth > 64 {
            return None;
        }
        let ex = &self.ast.exprs;
        if ex.get(e).kind() == ExprKind::OpaqueValueExpr {
            return Some(match ex.opaque_source(e) {
                Some(s) if s == old => self.ast.exprs.clone_opaque(e, new),
                _ => e,
            });
        }
        let kids = ex.children(e, &self.ast.lists);
        let mut new_kids = Vec::with_capacity(kids.len());
        for &k in &kids {
            new_kids.push(self.rebind_opaque(k, old, new, depth + 1)?);
        }
        if new_kids == kids {
            return Some(e);
        }
        self.ast.exprs.clone_with_children(e, &mut self.ast.lists, &new_kids)
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
