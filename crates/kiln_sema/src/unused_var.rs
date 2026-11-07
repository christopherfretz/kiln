//! Unused local variables (clang `ShouldDiagnoseUnusedDecl` /
//! `DiagnoseUnusedDecl` / `DiagnoseUnusedButSetDecl`, spec/sema.md
//! "Unused variables"): `-Wunused-variable` for a local variable (or
//! structured binding declaration) never referenced,
//! `-Wunused-exception-parameter` for such an exception variable, and
//! `-Wunused-but-set-variable` for one referenced only as the target of
//! discarded assignments. Diagnosed when the scope is left (sorted by
//! location, none once an error occurred in the scope), or, for a
//! variable of dependent type, when it is instantiated.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, VarStorage};
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::names::OverloadedOperator;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;
use crate::scope::{Ns, Scope};

impl Sema<'_, '_> {
    /// clang `ActOnPopScope`: the unused variables of scope `s`, which is
    /// being left. Not in an instantiation (a variable of dependent type
    /// is checked as it is instantiated, [`Self::diagnose_unused_inst_var`]).
    pub(crate) fn diagnose_unused_locals(&mut self, s: &Scope) {
        if s.locals.is_empty() {
            return;
        }
        let quiet =
            !self.inst.stack.is_empty() || self.diags.unrecoverable_error_count() > s.errors;
        let mut seen: Vec<DeclId> = Vec::new();
        // The parameters' warnings come first (clang
        // `ActOnFinishFunctionBody`, ahead of the body scope's pop).
        let mut params: Vec<(Loc, Diag)> = Vec::new();
        let mut found: Vec<(Loc, Diag)> = Vec::new();
        for &(_, d, ns) in &s.locals {
            if ns != Ns::Ordinary {
                continue;
            }
            // A structured binding stands for its decomposition.
            let d = match self.ast.decls.get(d).kind() {
                DeclKind::Binding => match self.ast.decls.binding_decomp(d) {
                    Some(v) => v,
                    None => continue,
                },
                _ => d,
            };
            if seen.contains(&d) {
                continue;
            }
            seen.push(d);
            let count = self.refs_minus_assign.remove(&d);
            if quiet {
                continue;
            }
            if let Some(msg) = self.unused_var_diag(d, d) {
                found.push((self.ast.decls.get(d).loc(), msg));
            } else if let Some(msg) = self.unused_but_set_diag(d, count) {
                let to = if self.ast.decls.get(d).kind() == DeclKind::ParmVar {
                    &mut params
                } else {
                    &mut found
                };
                to.push((self.ast.decls.get(d).loc(), msg));
            }
        }
        params.sort_by_key(|&(l, _)| l);
        found.sort_by_key(|&(l, _)| l);
        for (_, msg) in params.into_iter().chain(found) {
            self.emit(msg);
        }
    }

    /// clang `TemplateDeclInstantiator::VisitVarDecl`: local variable `v`
    /// instantiated from `pattern`, whose type is dependent (clang defers
    /// its diagnostic to here); referenced as the pattern is.
    pub(crate) fn diagnose_unused_inst_var(&mut self, pattern: DeclId, v: DeclId) {
        let Some(t) = self.ast.decls.get(pattern).ty() else { return };
        if self.ast.types.is_dependent(t)
            && let Some(msg) = self.unused_var_diag(v, pattern)
        {
            self.emit(msg);
        }
    }

    /// clang `DiagnoseUnusedDecl` for variable `v` (referenced when
    /// `pattern` is): the warning, when enabled and due.
    fn unused_var_diag(&mut self, v: DeclId, pattern: DeclId) -> Option<Diag> {
        let exception = self.ast.decls.var(v).is_some_and(|x| x.spec.is_exception_var());
        let id = if exception {
            DiagId::sema_unused_exception_param
        } else {
            DiagId::sema_unused_variable
        };
        if !self.diags.is_enabled(id) || !self.should_diagnose_unused_var(v, pattern) {
            return None;
        }
        let loc = self.ast.decls.get(v).loc();
        let arg = if self.ast.decls.is_decomposition(v) {
            format!("'{}'", self.decomp_name(v))
        } else {
            self.decl_arg(v)
        };
        Some(self.diag(id, loc).arg(arg))
    }

    /// clang `DiagnoseUnusedButSetDecl` for local variable or function
    /// parameter `v` (`-Wunused-but-set-parameter`, [dcl.fct]/13), whose
    /// references minus discarded assignments to it are `count`.
    fn unused_but_set_diag(&mut self, v: DeclId, count: Option<(i32, bool)>) -> Option<Diag> {
        let (refs, cond) = count?;
        let n = *self.ast.decls.get(v);
        let id = match n.kind() {
            DeclKind::Var => DiagId::sema_unused_but_set_variable,
            DeclKind::ParmVar => DiagId::sema_unused_but_set_parameter,
            _ => return None,
        };
        if !n.has(DeclFlags::REFERENCED) || !self.diags.is_enabled(id) {
            return None;
        }
        let name = n.name().as_ident()?;
        // A variable marked unused, or a placeholder `_` (which behaves
        // as if `[[maybe_unused]]`; a parameter is never a placeholder).
        if self.unused_marked(v)
            || (id == DiagId::sema_unused_but_set_variable
                && self.has_automatic_storage(v)
                && self.ast.symbols().as_str(name) == "_")
        {
            return None;
        }
        let ty = self.base_element(n.ty()?);
        if self.ast.types.is_reference(ty) || self.ast.types.is_dependent(ty) {
            return None;
        }
        // Any class type (gcc's behavior), or an enumeration marked unused.
        if self.ast.types.as_record(ty).is_some() {
            return None;
        }
        if let TypeView::Enum(decl) = self.ast.types.view(ty.ty())
            && self.unused_marked(decl)
        {
            return None;
        }
        // A condition variable is referenced once by the condition itself.
        if refs > 0 && !(cond && refs == 1) {
            return None;
        }
        Some(self.diag(id, n.loc()).arg(self.decl_arg(v)))
    }

    /// clang `DoMarkVarDeclReferenced`: a reference to variable `d`
    /// counts for `-Wunused-but-set-variable` when it is a local, not
    /// `extern`, variable, or a function parameter.
    pub(crate) fn count_var_ref(&mut self, d: DeclId) {
        let n = *self.ast.decls.get(d);
        let local = n.ctx().is_some_and(|c| self.ast.decls.get(c).kind().is_function())
            && self.ast.decls.var(d).is_some_and(|x| x.spec.storage() != VarStorage::Extern);
        if local || n.kind() == DeclKind::ParmVar {
            self.refs_minus_assign.entry(d).or_insert((0, false)).0 += 1;
        }
    }

    /// clang `MaybeDecrementCount` (from `IgnoredValueConversions`):
    /// discarded-value expression `e`, as written (parentheses count), an
    /// assignment, compound assignment, `operator=` call or increment /
    /// decrement whose target is a variable named directly does not count
    /// as a reference to it; a compound one or increment of a `volatile`
    /// variable still does.
    pub(crate) fn discard_assignment_ref(&mut self, e: ExprId) {
        let ex = &self.ast.exprs;
        let (lhs, compound) = match ex.get(e).kind() {
            ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator => {
                let op = ex.binary_op(e);
                let ch = ex.children(e, &self.ast.lists);
                let (l, r) = (ch[0], ch[1]);
                let dependent = self.ast.types.is_dependent(self.ety(l))
                    || self.ast.types.is_dependent(self.ety(r));
                if (dependent && op != BinaryOp::Assign) || !op.is_assignment() {
                    return;
                }
                (l, !dependent && op.is_compound_assignment())
            }
            ExprKind::CXXOperatorCallExpr => {
                if ex.operator_call_op(e) != OverloadedOperator::Equal {
                    return;
                }
                let Some(l) = self.ast.lists.iter::<ExprId>(ex.call_args(e)).next() else { return };
                (l, false)
            }
            ExprKind::UnaryOperator if ex.unary_op(e).is_inc_dec() => (ex.operand(e), true),
            _ => return,
        };
        if self.ast.exprs.get(lhs).kind() != ExprKind::DeclRefExpr {
            return;
        }
        let v = self.ast.exprs.decl(lhs);
        if !matches!(self.ast.decls.get(v).kind(), DeclKind::Var | DeclKind::ParmVar) {
            return;
        }
        if compound && self.ast.decls.get(v).ty().is_some_and(|t| t.quals().is_volatile()) {
            return;
        }
        if let Some(c) = self.refs_minus_assign.get_mut(&v) {
            c.0 -= 1;
        }
    }

    /// Whether variable `v` has automatic storage duration.
    pub(crate) fn has_automatic_storage(&self, v: DeclId) -> bool {
        self.ast.decls.var(v).is_some_and(|var| {
            matches!(var.spec.storage(), VarStorage::None | VarStorage::Register)
                && var.spec.tls() == kiln_ast::decl::TlsKind::None
        })
    }

    /// clang `ShouldDiagnoseUnusedDecl` for variable `v`, referenced
    /// when `pattern` (itself, or the pattern it is instantiated from) is.
    fn should_diagnose_unused_var(&mut self, v: DeclId, pattern: DeclId) -> bool {
        let n = *self.ast.decls.get(v);
        let p = *self.ast.decls.get(pattern);
        if n.kind() != DeclKind::Var || n.has(DeclFlags::INVALID) {
            return false;
        }
        let Some(var) = self.ast.decls.var(v).copied() else { return false };
        if self.ast.decls.is_decomposition(v) {
            // A structured binding declaration: unused when none of its
            // bindings is referenced or marked unused (the variable itself
            // is referenced by the bindings), unless all are placeholders.
            let mut bindings = self.decomp_binding_list(v);
            bindings.extend(self.decomp_binding_list(pattern));
            if bindings
                .iter()
                .any(|&b| self.ast.decls.get(b).has(DeclFlags::REFERENCED) || self.unused_marked(b))
            {
                return false;
            }
            let automatic = self.has_automatic_storage(v);
            if bindings.iter().all(|&b| {
                automatic
                    && self
                        .ast
                        .decls
                        .get(b)
                        .name()
                        .as_ident()
                        .is_some_and(|s| self.ast.symbols().as_str(s) == "_")
            }) {
                return false;
            }
        } else {
            if [n, p].iter().any(|x| x.has(DeclFlags::REFERENCED) || x.has(DeclFlags::USED)) {
                return false;
            }
            // An init-capture's variable is referenced by its capture
            // (clang: only `-Wunused-lambda-capture` names it).
            if self.is_init_capture_var(v) || self.is_init_capture_var(pattern) {
                return false;
            }
            let Some(name) = n.name().as_ident() else { return false };
            // A C++26 placeholder (`_` with automatic storage).
            if self.has_automatic_storage(v) && self.ast.symbols().as_str(name) == "_" {
                return false;
            }
        }
        if [AttrKind::UNUSED, AttrKind::MAYBE_UNUSED]
            .iter()
            .any(|&k| self.find_attr(pattern, k).is_some())
        {
            return false;
        }
        // Only variables local to a function (not an `extern` one).
        if !n.ctx().is_some_and(|c| self.ast.decls.get(c).kind().is_function())
            || var.spec.storage() == VarStorage::Extern
        {
            return false;
        }
        let Some(mut ty) = n.ty() else { return false };
        // A typedef marked unused (its outermost level only).
        if let TypeView::Typedef { decl, .. } = self.ast.types.view(ty.ty())
            && self.unused_marked(decl)
        {
            return false;
        }
        let mut init = var.init.map(|i| self.skip_cleanups(i));
        // A reference that extends a temporary's lifetime: the
        // temporary's type and initializer.
        if let Some(i) = init
            && self.ast.exprs.get(i).kind() == ExprKind::MaterializeTemporaryExpr
            && self.ast.exprs.lifetime_extended_temporary(i).is_some()
        {
            ty = self.non_reference(ty);
            init = Some(self.ignore_implicit_as_written(self.ast.exprs.operand(i)));
        }
        if self.ast.types.is_dependent(ty) || !self.is_complete(ty) {
            return false;
        }
        let elem = self.base_element(ty);
        if let Some(r) = self.ast.types.as_record(elem) {
            if self.unused_marked(r) {
                return false;
            }
            let Some(def) = self.ast.definition(r) else { return false };
            if !self.defn_bits(def).contains(DefinitionBits::TRIVIAL_DTOR) {
                return false;
            }
            if let Some(i) = init
                && !self.trivially_constructed(v, def, i)
            {
                return false;
            }
        }
        true
    }

    /// Whether variable `v` is an init-capture's variable
    /// ([expr.prim.lambda.capture]/6): owned by a lambda's call operator
    /// and recorded on one of its init-captures.
    fn is_init_capture_var(&self, v: DeclId) -> bool {
        let Some(op) = self.ast.decls.get(v).ctx() else { return false };
        let Some(l) = self.lambda_of_call_op(op) else { return false };
        self.ast.exprs.lambdas.get(l).captures.iter().any(|c| c.init && c.var == Some(v))
    }

    /// Whether declaration `d` is marked `[[maybe_unused]]` /
    /// `__attribute__((unused))`.
    fn unused_marked(&self, d: DeclId) -> bool {
        self.find_attr(d, AttrKind::UNUSED).is_some()
            || self.find_attr(d, AttrKind::MAYBE_UNUSED).is_some()
    }

    /// clang's constructor test for an unused variable `v` of class type
    /// (definition `def`) with initializer `init`: a non-trivial
    /// constructor counts only when the initializer is not a constant (one
    /// referring to an automatic object, constant by P2686R5, is not); a
    /// type-dependent initializer (a constructor not known yet) counts
    /// when the class declares a non-trivial constructor, and an
    /// unresolved construction with dependent arguments always does.
    fn trivially_constructed(&mut self, v: DeclId, def: DeclId, init: ExprId) -> bool {
        let kind = self.ast.exprs.get(init).kind();
        if matches!(kind, ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr) {
            let c = self.ast.exprs.constructor(init);
            if !self.ast.decls.function(c).is_some_and(|f| f.spec.is_trivial()) {
                if self.expr_is_value_dependent(init) {
                    return false;
                }
                let loc = self.ast.decls.get(v).loc();
                // A value referring to an automatic object is constant
                // only by C++26 P2686R5, which clang 22 (whose warning
                // this is) does not implement: not a constant here (a
                // guard: only a `constexpr` variable's value can, for now,
                // spec/sema.md §4.9).
                match self.eval_const(init, kiln_oven::EvalContext::init(v, loc)) {
                    Ok(c) if kiln_oven::automatic_referent(&self.ast, c).is_none() => {}
                    _ => return false,
                }
            }
        }
        if self.expr_is_type_dependent(init) && self.declares_nontrivial_ctor(def) {
            return false;
        }
        kind != ExprKind::CXXUnresolvedConstructExpr
    }

    /// Whether class definition `def` declares a constructor that is not
    /// trivial (clang `RD->ctors()`: the declared ones, including
    /// constructor templates' patterns not counted).
    fn declares_nontrivial_ctor(&self, def: DeclId) -> bool {
        self.ast.decls.members(def).any(|m| {
            self.ast.decls.get(m).kind() == DeclKind::CXXConstructor
                && !self.ast.decls.function(m).is_some_and(|f| f.spec.is_trivial())
        })
    }

    fn skip_cleanups(&self, e: ExprId) -> ExprId {
        if self.ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups {
            self.ast.exprs.operand(e)
        } else {
            e
        }
    }

    /// clang `IgnoreImplicitAsWritten`: implicit casts and full-expression
    /// wrappers.
    fn ignore_implicit_as_written(&self, mut e: ExprId) -> ExprId {
        for _ in 0..64 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ImplicitCastExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr => e = self.ast.exprs.operand(e),
                _ => break,
            }
        }
        e
    }
}
