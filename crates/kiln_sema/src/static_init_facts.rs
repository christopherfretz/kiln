//! The static-initialization facts (spec/ast.md §25, WP-3650): with
//! `-fkiln-static-init-facts`, at the end of the translation unit, every
//! non-dependent variable of static or thread storage duration with a
//! definition gets the initialization and destruction kinds code
//! generation gives it, recorded on its `VarData`
//! ([`kiln_ast::decl::DeclArena::set_static_init_fact`]). Code generation
//! then reads them instead of deciding again (`kiln_codegen`
//! `fold::prepass`).
//!
//! Each variable is evaluated at most once: one whose evaluated value is
//! already recorded (a `constexpr` variable, a `const` integral one)
//! keeps it; any other's constant-initialization result is recorded as
//! its evaluated value (spec/ast.md §9.2), as clang's
//! `CheckCompleteVariableDeclaration` caches it.

use kiln_ast::decl::{DeclFlags, DeclKind, TlsKind, VarEvalState};
use kiln_ast::expr::{ExprKind, StorageDuration};
use kiln_ast::static_init::{NotEvaluatedWhy, StaticDestruction, StaticInit, StaticInitFact};
use kiln_ast::types::TypeView;
use kiln_ast::{ConstValueId, DeclId, ExprId, QualType};
use kiln_oven::{FailKind, StaticInitEval};
use kiln_source::fx::FxHashSet;

use crate::Sema;

impl Sema<'_, '_> {
    /// Records the static-initialization fact of every eligible variable
    /// that has none yet (spec/ast.md §25.2): the end of the translation
    /// unit, after its pending instantiations. A no-op unless the unit
    /// records them, in C (no dynamic initialization, C23 6.7.11p4) and in
    /// a prefix (a precompiled header: the unit that includes it records
    /// its variables' facts).
    pub fn record_static_init_facts(&mut self) {
        if !self.ast.static_init_facts || self.ast.lang_c || self.tu_prefix {
            return;
        }
        // A static local whose initializer is not constant may be folded
        // or not by code generation depending on the FP environment of
        // its enclosing blocks (`kiln_codegen` `fold::Collect`): any
        // environment-dependent FP state in the unit leaves those
        // undecided.
        let fp_env = self.ast.stmts.any_env_dependent_fp(self.fp);
        let candidates: Vec<DeclId> = self
            .ast
            .decls
            .ids()
            .filter(|&d| {
                let n = self.ast.decls.get(d);
                matches!(n.kind(), DeclKind::Var | DeclKind::VarTemplateSpecialization)
                    && !n.has(DeclFlags::DEPENDENT_CTX)
            })
            .collect();
        let mut done = FxHashSet::default();
        for d in candidates {
            if !self.static_init_eligible(d) || !done.insert(self.ast.first(d)) {
                continue;
            }
            self.record_static_init_fact(d, fp_env);
        }
    }

    /// Whether `d` is a definition of a variable of static or thread
    /// storage duration outside any dependent context, with a
    /// non-dependent type, that has no fact yet.
    fn static_init_eligible(&self, d: DeclId) -> bool {
        let Some(v) = self.ast.decls.var(d) else { return false };
        if !(self.has_static_storage(d) || v.spec.tls() != TlsKind::None)
            || !self.is_var_definition(d)
            || self.ast.static_init_fact(d).is_some()
            || self.ast.described_template(d).is_some()
            || self.ast.decls.get(d).ty().is_none_or(|t| self.is_dependent_type(t))
        {
            return false;
        }
        // Not inside a template pattern (a static data member of a class
        // template, a static local of a function template).
        let mut c = self.ast.decls.get(d).ctx();
        while let Some(x) = c {
            let n = self.ast.decls.get(x);
            if n.has(DeclFlags::DEPENDENT_CTX)
                || self.ast.described_template(x).is_some()
                || matches!(
                    n.kind(),
                    DeclKind::ClassTemplatePartialSpecialization
                        | DeclKind::VarTemplatePartialSpecialization
                )
            {
                return false;
            }
            c = n.ctx();
        }
        true
    }

    /// Records the fact of definition `d` on the declaration holding its
    /// initializer (else on `d`).
    fn record_static_init_fact(&mut self, d: DeclId, fp_env: bool) {
        let holder = std::iter::once(d)
            .chain(self.ast.redecls_of(d))
            .find(|&r| self.ast.decls.var(r).is_some_and(|v| v.init.is_some()))
            .unwrap_or(d);
        let init = self.ast.decls.var(holder).and_then(|v| v.init);
        let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let invalid = [d, holder].iter().any(|&x| self.ast.decls.get(x).has(DeclFlags::INVALID));
        let fact = if invalid {
            StaticInitFact {
                init: StaticInit::NotEvaluated(NotEvaluatedWhy::Invalid),
                destruction: None,
            }
        } else {
            match init {
                Some(e) if self.expr_is_value_dependent(e) => return,
                Some(e) => self.static_init_fact_of(d, holder, e, ty, fp_env),
                // Zero-initialization only ([basic.start.static]/2).
                None => StaticInitFact {
                    init: StaticInit::Constant,
                    destruction: Some(self.static_destruction(holder, ty, None, true)),
                },
            }
        };
        self.ast.decls.set_static_init_fact(holder, fact);
    }

    /// The fact of variable `d` (definition) whose initializer `e` is held
    /// by `holder`.
    fn static_init_fact_of(
        &mut self,
        d: DeclId,
        holder: DeclId,
        e: ExprId,
        ty: QualType,
        fp_env: bool,
    ) -> StaticInitFact {
        let not_evaluated =
            |why| StaticInitFact { init: StaticInit::NotEvaluated(why), destruction: None };
        if self.has_errors_in(e) {
            return not_evaluated(NotEvaluatedWhy::InitErrors);
        }
        if self.has_flexible_array_elements(ty, e) {
            return not_evaluated(NotEvaluatedWhy::FlexibleArrayMember);
        }
        // Code generation's fallback fold (spec/ast.md §25.1): a scalar
        // or initializer-list initializer, not in a static local under a
        // dynamic FP environment.
        let static_local = self.is_local_var(d);
        let fallback =
            self.is_fold_scalar(ty) || self.ast.exprs.get(e).kind() == ExprKind::InitListExpr;
        let eval = match self.ast.decls.var(holder).map(|v| v.eval.state()) {
            // Evaluated already: its state is reused.
            Some(VarEvalState::Value(c))
                if kiln_oven::automatic_referent(&self.ast, c).is_none() =>
            {
                StaticInitEval::Constant(c)
            }
            // Constant, but with no constant form: dynamic, as codegen
            // (no fallback fold).
            Some(VarEvalState::Value(_)) => StaticInitEval::NotConstant(FailKind::NotConstant),
            Some(VarEvalState::NotConstant) => {
                match (fallback && !(static_local && fp_env))
                    .then(|| self.fold_static_fallback(e))
                    .flatten()
                {
                    Some(v) => StaticInitEval::Folded(v),
                    None => StaticInitEval::NotConstant(FailKind::NotConstant),
                }
            }
            Some(VarEvalState::Evaluating) | None => {
                return not_evaluated(NotEvaluatedWhy::Stack);
            }
            Some(VarEvalState::NotEvaluated) => {
                if !self.static_init_stack_ok(e) {
                    return not_evaluated(NotEvaluatedWhy::Stack);
                }
                let eval = self.static_init_eval(e, holder, fallback && !(static_local && fp_env));
                // The constant-initialization result is the variable's
                // evaluated value; a resource limit gives no verdict.
                if self.ast.decls.begin_eval(holder).is_ok() {
                    match eval {
                        StaticInitEval::Constant(v) => self.ast.decls.finish_eval(holder, Some(v)),
                        StaticInitEval::NotConstant(
                            FailKind::StepLimit | FailKind::DepthLimit | FailKind::MemoryLimit,
                        ) => self.ast.decls.abort_eval(holder),
                        _ => self.ast.decls.finish_eval(holder, None),
                    }
                }
                eval
            }
        };
        let trivial = self.ast.is_trivial_default_construction(e);
        let (init, value) = match eval {
            StaticInitEval::Constant(v) => (StaticInit::Constant, Some(v)),
            // A trivial default constructor does nothing: the object is
            // zero-initialized ([basic.start.static]/2).
            _ if trivial => (StaticInit::Constant, None),
            StaticInitEval::Folded(_) => (StaticInit::EarlyStatic, None),
            StaticInitEval::NotConstant(k) => match k {
                FailKind::StepLimit => return not_evaluated(NotEvaluatedWhy::StepLimit),
                FailKind::DepthLimit => return not_evaluated(NotEvaluatedWhy::DepthLimit),
                FailKind::MemoryLimit => return not_evaluated(NotEvaluatedWhy::MemoryLimit),
                FailKind::NotConstant | FailKind::Destruction if static_local && fp_env => {
                    return not_evaluated(NotEvaluatedWhy::FpEnvironment);
                }
                FailKind::NotConstant | FailKind::Destruction => (StaticInit::Dynamic, None),
            },
        };
        let destruction = if self.ast.types.is_reference(ty) {
            if init == StaticInit::Dynamic && self.extends_destroyed_temporary(e) {
                StaticDestruction::Registered
            } else {
                StaticDestruction::Trivial
            }
        } else {
            // As codegen: the destruction is manifestly constant-evaluated
            // when the variable is constant-initialized.
            self.static_destruction(holder, ty, value, value.is_some())
        };
        StaticInitFact { init, destruction: Some(destruction) }
    }

    /// The destruction kind of variable `v` of type `ty` whose initializer
    /// value is `value` (a constant initializer's).
    fn static_destruction(
        &mut self,
        v: DeclId,
        ty: QualType,
        value: Option<ConstValueId>,
        const_context: bool,
    ) -> StaticDestruction {
        if self.ast.types.is_reference(ty) || !self.ast.needs_destruction(ty) {
            StaticDestruction::Trivial
        } else if self.has_constant_destruction(v, value, const_context) {
            StaticDestruction::Constant
        } else {
            StaticDestruction::Registered
        }
    }

    /// Whether `d` is declared in a function body (a static local).
    fn is_local_var(&self, d: DeclId) -> bool {
        let mut c = self.ast.decls.get(d).ctx();
        while let Some(x) = c {
            match self.ast.decls.get(x).kind() {
                k if k.is_function() => return true,
                DeclKind::TranslationUnit | DeclKind::Namespace => return false,
                _ => c = self.ast.decls.get(x).ctx(),
            }
        }
        false
    }

    /// Whether code generation folds an initializer of type `ty` again
    /// when it is not constant (`kiln_codegen` `fold::is_scalar`):
    /// arithmetic, enumeration, pointer, pointer to member or vector.
    fn is_fold_scalar(&self, ty: QualType) -> bool {
        matches!(
            self.ast.types.canonical_view(ty),
            TypeView::Builtin(_)
                | TypeView::BitInt { .. }
                | TypeView::Pointer(_)
                | TypeView::MemberPointer { .. }
                | TypeView::Enum(_)
                | TypeView::Vector { .. }
        )
    }

    /// GNU: whether initializer list `e` of a variable of class type `ty`
    /// may give its flexible array member elements (code generation folds
    /// those apart from the object).
    fn has_flexible_array_elements(&self, ty: QualType, e: ExprId) -> bool {
        if self.ast.exprs.get(e).kind() != ExprKind::InitListExpr {
            return false;
        }
        let Some(def) = self.ast.types.as_record(ty).and_then(|r| self.ast.definition(r)) else {
            return false;
        };
        self.ast
            .decls
            .members(def)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .any(|m| {
                self.ast.decls.get(m).ty().is_some_and(|t| {
                    matches!(self.ast.types.canonical_view(t), TypeView::IncompleteArray(_))
                })
            })
    }

    /// Whether initializer `e` of a reference binds a static or thread
    /// temporary ([class.temporary]/6) that has a non-trivial destructor
    /// and no constant value: its dynamic initialization registers that
    /// destructor (`kiln_codegen` `register_temporary_dtor`).
    fn extends_destroyed_temporary(&self, e: ExprId) -> bool {
        let ast = &self.ast;
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            match ast.exprs.get(x).kind() {
                ExprKind::LambdaExpr | ExprKind::RequiresExpr => continue,
                ExprKind::MaterializeTemporaryExpr
                    if matches!(
                        ast.exprs.temporary_storage(x),
                        StorageDuration::Static | StorageDuration::Thread
                    ) =>
                {
                    let has_value = ast
                        .exprs
                        .lifetime_extended_temporary(x)
                        .and_then(|t| ast.decls.lifetime_extended(t))
                        .is_some_and(|l| l.value.is_some());
                    let sub = ExprId::new(ast.exprs.get(x).a());
                    if !has_value && ast.needs_destruction(ast.exprs.get(sub).ty()) {
                        return true;
                    }
                }
                _ => {}
            }
            ast.exprs.children_into(x, &ast.lists, &mut stack);
        }
        false
    }
}
