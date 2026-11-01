//! `-Wunused-variable` (clang `ShouldDiagnoseUnusedDecl` /
//! `DiagnoseUnusedDecl`, spec/sema.md "Unused variables"): a local
//! variable that is never referenced, diagnosed when its scope is left
//! (sorted by location, none once an error occurred in the scope), or,
//! for a variable of dependent type, when it is instantiated.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, VarStorage};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;

use crate::Sema;
use crate::scope::{Ns, Scope};

impl Sema<'_, '_> {
    /// clang `ActOnPopScope`: the unused variables of scope `s`, which is
    /// being left. Not in an instantiation (a variable of dependent type
    /// is checked as it is instantiated, [`Self::diagnose_unused_inst_var`]).
    pub(crate) fn diagnose_unused_locals(&mut self, s: &Scope) {
        if s.locals.is_empty()
            || !self.inst.stack.is_empty()
            || self.diags.error_count() > s.errors
            || !self.diags.is_enabled(DiagId::sema_unused_variable)
        {
            return;
        }
        let mut found: Vec<DeclId> = Vec::new();
        for &(_, d, ns) in &s.locals {
            if ns == Ns::Ordinary && !found.contains(&d) && self.should_diagnose_unused_var(d, d) {
                found.push(d);
            }
        }
        found.sort_by_key(|&d| self.ast.decls.get(d).loc());
        for d in found {
            self.emit_unused_var(d);
        }
    }

    /// clang `TemplateDeclInstantiator::VisitVarDecl`: local variable `v`
    /// instantiated from `pattern`, whose type is dependent (clang defers
    /// its diagnostic to here); referenced as the pattern is.
    pub(crate) fn diagnose_unused_inst_var(&mut self, pattern: DeclId, v: DeclId) {
        let Some(t) = self.ast.decls.get(pattern).ty() else { return };
        if self.ast.types.is_dependent(t)
            && self.diags.is_enabled(DiagId::sema_unused_variable)
            && self.should_diagnose_unused_var(v, pattern)
        {
            self.emit_unused_var(v);
        }
    }

    fn emit_unused_var(&mut self, v: DeclId) {
        let loc = self.ast.decls.get(v).loc();
        let msg = self.diag(DiagId::sema_unused_variable, loc).arg(self.decl_arg(v));
        self.emit(msg);
    }

    /// clang `ShouldDiagnoseUnusedDecl` for variable `v`, referenced
    /// when `pattern` (itself, or the pattern it is instantiated from) is.
    fn should_diagnose_unused_var(&mut self, v: DeclId, pattern: DeclId) -> bool {
        let n = *self.ast.decls.get(v);
        let p = *self.ast.decls.get(pattern);
        if n.kind() != DeclKind::Var
            || n.has(DeclFlags::INVALID)
            || [n, p].iter().any(|x| x.has(DeclFlags::REFERENCED) || x.has(DeclFlags::USED))
        {
            return false;
        }
        let Some(name) = n.name().as_ident() else { return false };
        let Some(var) = self.ast.decls.var(v).copied() else { return false };
        let automatic = matches!(var.spec.storage(), VarStorage::None | VarStorage::Register)
            && var.spec.tls() == kiln_ast::decl::TlsKind::None;
        // A C++26 placeholder (`_` with automatic storage).
        if automatic && self.ast.symbols().as_str(name) == "_" {
            return false;
        }
        if [AttrKind::UNUSED, AttrKind::MAYBE_UNUSED]
            .iter()
            .any(|&k| self.find_attr(pattern, k).is_some())
        {
            return false;
        }
        // Only variables local to a function (not an `extern` one, an
        // exception variable has its own, unimplemented warning).
        if !n.ctx().is_some_and(|c| self.ast.decls.get(c).kind().is_function())
            || var.spec.storage() == VarStorage::Extern
            || var.spec.is_exception_var()
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
                && !self.trivially_constructed(v, i)
            {
                return false;
            }
        }
        true
    }

    /// Whether declaration `d` is marked `[[maybe_unused]]` /
    /// `__attribute__((unused))`.
    fn unused_marked(&self, d: DeclId) -> bool {
        self.find_attr(d, AttrKind::UNUSED).is_some()
            || self.find_attr(d, AttrKind::MAYBE_UNUSED).is_some()
    }

    /// clang's constructor test for an unused class variable `v` with
    /// initializer `init`: a non-trivial constructor counts only when the
    /// initializer is a constant; a type-dependent initializer (a
    /// constructor not known yet) does not.
    fn trivially_constructed(&mut self, v: DeclId, init: ExprId) -> bool {
        if self.expr_is_type_dependent(init) {
            return false;
        }
        if !matches!(
            self.ast.exprs.get(init).kind(),
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr
        ) {
            return true;
        }
        let c = self.ast.exprs.constructor(init);
        if self.ast.decls.function(c).is_some_and(|f| f.spec.is_trivial()) {
            return true;
        }
        if self.expr_is_value_dependent(init) {
            return false;
        }
        let loc = self.ast.decls.get(v).loc();
        self.eval_const(init, kiln_oven::EvalContext::init(v, loc)).is_ok()
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
