//! `-Winfinite-recursion` (clang `checkRecursiveFunction`, spec/sema.md
//! §7.5.3): every live path from the entry of a function's graph to its
//! exit calls the function itself.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::types::{Nns, TypeKind};
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_diag::DiagId;

use crate::Sema;

impl Sema<'_, '_> {
    /// `all paths through this function will call itself` at the start
    /// of body `body` of `f` when every live path from the entry of the
    /// body's graph meets a call of `f` before the exit, and the exit has
    /// a predecessor. Not for a specialization of a function template
    /// (clang runs on non-templated functions and members of class
    /// template specializations). A body that calls no `f` among the
    /// expressions built since it began (`expr_start`) builds no graph.
    pub(crate) fn check_infinite_recursion(&mut self, f: DeclId, body: StmtId, expr_start: u32) {
        if !self.diags.is_enabled(DiagId::sema_infinite_recursion) {
            return;
        }
        if let Some((_, info)) = self.ast.spec_info(f)
            && self.kind_of(info.template) == DeclKind::FunctionTemplate
        {
            return;
        }
        let f = self.ast.first(f);
        // A constructor's initializers were built ahead of its body.
        let ctor = self.ast.decls.get(f).kind() == DeclKind::CXXConstructor;
        if !ctor {
            let end = u32::try_from(self.ast.exprs.len()).unwrap_or(u32::MAX);
            if !((expr_start + 1)..=end)
                .filter_map(ExprId::try_new)
                .any(|e| self.is_recursive_call(f, e))
            {
                return;
            }
        }
        if self.with_cfg(body, |s, c| c.every_path_hits(|e| s.is_recursive_call(f, e)))
            != Some(true)
        {
            return;
        }
        let loc = self.ast.stmts.get(body).loc();
        let msg = self.diag(DiagId::sema_infinite_recursion, loc);
        self.emit(msg);
    }

    /// Whether `e` is a call of `f` (a first declaration) that recurses
    /// (clang `hasRecursiveCallInPath`): not one qualified by a class
    /// template specialization (`C<T>::f()`), nor a virtual call through
    /// an object other than `this`.
    fn is_recursive_call(&self, f: DeclId, e: ExprId) -> bool {
        let exprs = &self.ast.exprs;
        let kind = exprs.get(e).kind();
        if !matches!(
            kind,
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr
        ) {
            return false;
        }
        let Some(d) = self.callee_decl(e) else { return false };
        if self.ast.first(d) != f {
            return false;
        }
        let mut r = exprs.callee(e);
        while matches!(exprs.get(r).kind(), ExprKind::ImplicitCastExpr | ExprKind::ParenExpr) {
            match exprs.children(r, &self.ast.lists).first() {
                Some(&c) => r = c,
                None => break,
            }
        }
        if exprs.get(r).kind() == ExprKind::DeclRefExpr
            && let Some(q) = exprs.decl_ref_extra(r).and_then(|x| x.qualifier)
            && let Nns::Type { ty, .. } = self.ast.types.nns_view(q)
            && self.ast.types.kind(ty.ty()) == TypeKind::TemplateSpecialization
        {
            return false;
        }
        kind != ExprKind::CXXMemberCallExpr
            || exprs.get(exprs.member_call_object(e)).kind() == ExprKind::CXXThisExpr
            || !crate::class::is_virtual(&self.ast, d)
    }
}
