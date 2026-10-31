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

use kiln_ast::decl::{DeclFlags, DeclKind, Defaulted, TlsKind, VarStorage};
use kiln_ast::expr::{
    BinaryOp, CastKind, ExprDeps, ExprFlags, ExprKind, StorageDuration, ValueCategory,
};
use kiln_ast::types::Quals;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::special::SpecialKind;
use crate::Sema;

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
        self.builder().materialize_temporary(
            e,
            ty,
            lvalue_ref,
            StorageDuration::FullExpression,
            None,
        )
    }

    /// clang `MaybeBindToTemporary`: a class prvalue `e` whose class has a
    /// non-trivial destructor is bound to a temporary destroyed at the end
    /// of the full-expression ([class.temporary]/4). The destructor must be
    /// accessible and not deleted ([class.dtor]/15) and is odr-used.
    pub(crate) fn maybe_bind_temporary(&mut self, e: ExprId) -> ExprId {
        let n = *self.ast.exprs.get(e);
        if n.value_category() != ValueCategory::PRValue || n.deps().contains(ExprDeps::TYPE) {
            return e;
        }
        let Some(r) = self.ast.types.as_record(n.ty()) else { return e };
        let Some(class) = self.ast.definition(r) else { return e };
        let flags = self.ast.decls.get(class).flags();
        if flags.contains(DeclFlags::INVALID) || flags.contains(DeclFlags::DEPENDENT_CTX) {
            return e;
        }
        if self.defn_bits(class).contains(kiln_ast::decl::DefinitionBits::TRIVIAL_DTOR) {
            return e;
        }
        let (super::special::Selected::Found(dtor) | super::special::Selected::Deleted(dtor)) =
            self.select_special(class, SpecialKind::Dtor, Quals::NONE, Quals::NONE)
        else {
            return e;
        };
        let loc = self.begin_loc(e);
        let ty = n.ty();
        let msg = self.special_member_access(dtor, class, |s, prot| {
            s.diag(DiagId::sema_temp_dtor_access, loc).arg(s.ty_arg(ty)).arg(u8::from(prot))
        });
        if let Some(msg) = msg {
            self.emit(msg);
        }
        self.mark_function_referenced(dtor, loc);
        self.diagnose_deleted_use(dtor, loc);
        self.cleanup.needs = true;
        self.cleanup.side_effects = true;
        self.builder().bind_temporary(e, dtor)
    }

    /// clang `MarkFunctionReferenced` for a use of function `f` at `loc`
    /// ([basic.def.odr]/7): an implicitly defined (defaulted, not deleted)
    /// special member is defined now (spec/sema-class.md §4.5), except a
    /// trivial default constructor (only odr-used) or destructor (not
    /// even odr-used); then `f` is odr-used.
    pub(crate) fn mark_function_referenced(&mut self, f: DeclId, loc: Loc) {
        self.ast.decls.set_flags(f, DeclFlags::REFERENCED);
        // clang's `MarkFunctionReferenced` resolves the exception
        // specification even in unevaluated operands (for `noexcept`).
        if !self.in_template() && !self.diagnose_unparsed_exc_spec(f, loc) {
            self.resolve_exception_spec(f);
        }
        if self.unevaluated > 0 || self.in_template() {
            return;
        }
        let first = self.ast.first(f);
        if self.func.as_ref().is_some_and(|s| self.ast.first(s.decl) == first) {
            // A recursive use from the function's own body.
            return;
        }
        if let Some(kind) = self.special_kind_of(first) {
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
        // A member function of a class template specialization is
        // instantiated ([temp.inst]/5).
        self.mark_used_at(f, loc);
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
                // returned expression.
                let msg = self.diag(DiagId::sema_temp_return_local, self.begin_loc(init));
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
    /// derived-to-base casts, class member access, array subscripts of
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
                let m = self.ast.exprs.decl(e);
                if self.kind_of(m) == DeclKind::Field {
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
