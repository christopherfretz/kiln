//! Static thread-local variables: GNU `__thread` and C11 `_Thread_local`
//! ([basic.stc.thread], spec/sema.md "Static TLS"; clang `TLS_Static`).
//!
//! Unlike `thread_local`, a static TLS variable has neither dynamic
//! initialization nor a non-trivial destructor (the GNU C++98 edits for
//! `__thread`): clang `CheckCompleteVariableDeclaration` reports "type of
//! thread-local variable has non-trivial destruction" at the variable, or
//! "initializer for thread-local variable must be a constant expression"
//! at the first part of the initializer that `Expr::isConstantInitializer`
//! rejects, each with `note: use 'thread_local' to allow this` from C++11
//! on. A dependent variable is checked when instantiated. Redeclarations
//! keep one thread storage kind ([dcl.stc]/4, clang `MergeVarDecl`).

use kiln_ast::decl::{DeclFlags, DefinitionBits, TlsKind};
use kiln_ast::expr::{CastKind, ExprDeps, ExprFlags, ExprKind, UnaryOp, ValueCategory};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// clang `CheckCompleteVariableDeclaration` for a static TLS variable
    /// definition `v` (after its initializer, or its default
    /// initialization). `paren` is the `(` of a one-expression
    /// parenthesized initializer.
    pub(crate) fn check_static_tls(&mut self, v: DeclId, paren: Option<Loc>) {
        let vn = *self.ast.decls.get(v);
        let Some(data) = self.ast.decls.var(v) else { return };
        if data.spec.tls() != TlsKind::Static || vn.has(DeclFlags::INVALID) {
            return;
        }
        let init = data.init;
        let Some(ty) = vn.ty() else { return };
        if self.is_error_type(ty) {
            return;
        }
        let loc = vn.loc();
        let note = self.std >= kiln_lex::Std::Cxx11;
        let dependent_ty = self.is_dependent_type(ty);
        if !dependent_ty && !self.ast.types.is_reference(ty) && self.is_destructed_type(ty) {
            let mut msg = self.diag(DiagId::sema_thread_nontrivial_dtor, loc).per_instantiation();
            if note {
                msg = msg.with_note(self.diag(DiagId::note_use_thread_local, loc));
            }
            self.emit(msg);
            return;
        }
        let Some(init) = init else { return };
        if self.has_errors_in(init) {
            return;
        }
        let deps = self.ast.exprs.get(init).deps();
        let mut at = None;
        let culprit = if deps.intersects(ExprDeps::TYPE_VALUE_INST) {
            // An instantiation-dependent initializer is checked when
            // instantiated; clang 19 rejects it in the template
            // definition unless it is a braced list (fixed after clang 19).
            if self.clang_compat >= 20 || self.ast.exprs.get(init).kind() == ExprKind::InitListExpr
            {
                return;
            }
            // clang 19 keeps `(e)` of a dependent type or a type-dependent
            // `e` as a `ParenListExpr`, reported at its `(`; kiln keeps
            // only a longer list (whose location is its `(`).
            if dependent_ty || deps.contains(ExprDeps::TYPE) {
                at = paren;
            }
            init
        } else {
            let for_ref = !dependent_ty && self.ast.types.is_reference(ty);
            let Some(c) = self.static_init_culprit(init, for_ref) else { return };
            c
        };
        let at = at.unwrap_or_else(|| self.culprit_loc(culprit));
        // clang re-runs the check on every instantiation (clang 19 also
        // in the definition).
        let mut msg = self.diag(DiagId::sema_thread_dynamic_init, at).per_instantiation();
        if note {
            msg = msg.with_note(self.diag(DiagId::note_use_thread_local, loc));
        }
        self.emit(msg);
    }

    /// clang `Expr::isConstantInitializer(Ctx, for_ref, &Culprit)`: the
    /// subexpression of initializer `e` that keeps it from being a
    /// constant initializer, or `None`. Aggregate lists, trivial
    /// constructions, parentheses and value-preserving casts are looked
    /// through; anything else must fold (laxly, as clang's
    /// `isEvaluatable`; a glvalue as an address).
    fn static_init_culprit(&mut self, e: ExprId, for_ref: bool) -> Option<ExprId> {
        let n = *self.ast.exprs.get(e);
        if for_ref {
            return match n.kind() {
                ExprKind::ExprWithCleanups => {
                    self.static_init_culprit(self.ast.exprs.operand(e), true)
                }
                ExprKind::MaterializeTemporaryExpr => {
                    self.static_init_culprit(self.ast.exprs.operand(e), false)
                }
                _ => (!self.folds_as_address(e)).then_some(e),
            };
        }
        match n.kind() {
            ExprKind::ExprWithCleanups
            | ExprKind::ConstantExpr
            | ExprKind::ParenExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::CXXBindTemporaryExpr => {
                return self.static_init_culprit(self.ast.exprs.operand(e), false);
            }
            ExprKind::SubstNonTypeTemplateParmExpr => {
                return self.static_init_culprit(self.ast.exprs.subst_replacement(e), false);
            }
            ExprKind::ImplicitValueInitExpr | ExprKind::NoInitExpr => return None,
            ExprKind::UnaryOperator if self.ast.exprs.unary_op(e) == UnaryOp::Extension => {
                return self.static_init_culprit(self.ast.exprs.operand(e), false);
            }
            ExprKind::CXXConstructExpr if self.trivial_construction(e) => {
                let arg = self.ast.lists.iter::<ExprId>(self.ast.exprs.construct_args(e)).next();
                return arg.and_then(|a| self.static_init_culprit(a, false));
            }
            // `T x{}` of a dependent `T` (and `{}` of any type) folds;
            // a non-empty list of another type is folded whole.
            ExprKind::InitListExpr
                if self
                    .ast
                    .lists
                    .iter::<ExprId>(self.ast.exprs.init_list_inits(e))
                    .next()
                    .is_none() =>
            {
                return None;
            }
            // `int x{f()}`: the list of a scalar is looked through after
            // clang 19 (whose culprit is the list, at its `{`).
            ExprKind::InitListExpr if self.clang_compat >= 20 && self.is_scalar(n.ty()) => {
                let elems: Vec<ExprId> =
                    self.ast.lists.iter(self.ast.exprs.init_list_inits(e)).collect();
                if let [x] = elems[..] {
                    return self.static_init_culprit(x, false);
                }
            }
            ExprKind::InitListExpr
                if matches!(
                    self.ast.types.canonical_view(n.ty()),
                    TypeView::ConstantArray(_) | TypeView::IncompleteArray(_) | TypeView::Record(_)
                ) =>
            {
                let elems: Vec<ExprId> =
                    self.ast.lists.iter(self.ast.exprs.init_list_inits(e)).collect();
                // A glvalue element binds a reference member.
                return elems.into_iter().find_map(|x| {
                    let glvalue = self.ast.exprs.get(x).value_category() != ValueCategory::PRValue;
                    self.static_init_culprit(x, glvalue)
                });
            }
            k if k.is_cast()
                && matches!(
                    self.ast.exprs.cast_kind(e),
                    CastKind::NoOp
                        | CastKind::ToUnion
                        | CastKind::ConstructorConversion
                        | CastKind::NonAtomicToAtomic
                        | CastKind::AtomicToNonAtomic
                        | CastKind::NullToPointer
                ) =>
            {
                return self.static_init_culprit(self.ast.exprs.operand(e), false);
            }
            _ => {}
        }
        // `isEvaluatable(Ctx, SE_AllowUndefinedBehavior)`; an
        // lvalue-to-rvalue conversion is folded whole (clang folds its
        // operand as an rvalue, the same location).
        self.fold_rvalue(e).is_none().then_some(e)
    }

    /// clang `EvaluateAsLValue`: whether glvalue `e` designates a
    /// constant address (folded as `&e`, a node built only for the fold);
    /// a prvalue folds as itself.
    fn folds_as_address(&mut self, e: ExprId) -> bool {
        let n = *self.ast.exprs.get(e);
        if n.value_category() == ValueCategory::PRValue {
            return self.fold_rvalue(e).is_some();
        }
        let ty = self.ast.types.pointer_to(n.ty());
        let a = self.builder().unary(UnaryOp::AddrOf, e, ty, n.loc(), ExprFlags::CANNOT_OVERFLOW);
        self.fold_rvalue(a).is_some()
    }

    /// A `CXXConstructExpr` calling a trivial constructor of a class with
    /// a trivial destructor.
    fn trivial_construction(&self, e: ExprId) -> bool {
        let ctor = self.ast.exprs.constructor(e);
        self.ast.decls.function(ctor).is_some_and(|f| f.spec.is_trivial())
            && self
                .ast
                .decls
                .get(ctor)
                .ctx()
                .and_then(|c| self.ast.definition(c))
                .is_some_and(|d| self.defn_bits(d).contains(DefinitionBits::TRIVIAL_DTOR))
    }

    /// [dcl.stc]/4 (clang `MergeVarDecl`): redeclaration `loc` of
    /// variable `name` with thread storage kind `tls` follows `prev` of
    /// another kind; `prev_def` whether `prev` is a definition (the
    /// note's wording). The declaration is still merged.
    pub(crate) fn check_tls_redecl(
        &mut self,
        prev: DeclId,
        tls: TlsKind,
        prev_def: bool,
        loc: Loc,
        name: &str,
    ) {
        let Some(old) = self.ast.decls.var(prev).map(|x| x.spec.tls()) else { return };
        if old == tls {
            return;
        }
        let msg = match (old, tls) {
            (TlsKind::None, _) => self.diag(DiagId::sema_thread_non_thread, loc).arg(name),
            (_, TlsKind::None) => self.diag(DiagId::sema_non_thread_thread, loc).arg(name),
            _ => self
                .diag(DiagId::sema_thread_thread_different_kind, loc)
                .arg(name)
                .arg(u8::from(tls == TlsKind::Dynamic)),
        };
        let msg = self.note_prev(msg, prev, prev_def);
        self.emit(msg);
    }
}
