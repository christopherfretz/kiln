//! `-Wmissing-noreturn` (clang `warn_suggest_noreturn_function`,
//! spec/sema.md §7.5.3): a function that could be declared `noreturn`.
//! The CFG form (clang `CheckFallThroughForBody`, both releases): a
//! function returning `void` whose body neither falls off its end nor
//! returns. The syntactic form (clang 22 `inferNoReturnAttr`): a body
//! that is one throw-expression; such a function is inferred
//! `noreturn`, which also quiets `-Wreturn-type` in a caller whose body
//! ends calling it.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_diag::DiagId;

use crate::Sema;

impl Sema<'_, '_> {
    /// The name of function `f` in a diagnostic: unqualified, with the
    /// arguments of a function template specialization (clang's
    /// `getNameForDiagnostic`: `'k<int>'`).
    pub(crate) fn fn_diag_name(&self, f: DeclId) -> String {
        let mut name = self.decl_name(f);
        if let Some((_, info)) = self.ast.spec_info(f)
            && self.kind_of(info.template) == DeclKind::FunctionTemplate
        {
            let names = crate::names::sema_names(self);
            kiln_ast::types::write_arg_list(&self.ast.types, &names, info.args, &mut name);
        }
        name
    }

    /// clang 22 `inferNoReturnAttr`, ahead of the body's other
    /// diagnostics: a function whose body is a single throw-expression
    /// (clang `isKnownToAlwaysThrow`) and is not declared `noreturn` nor
    /// virtual is inferred `noreturn` and gets `-Wmissing-noreturn` at its
    /// name. Not for the pattern of a function template; an
    /// instantiation of any other templated function inherits the
    /// pattern's inference silently, and so does a lambda's call
    /// operator below C++23. Runs when `-Wreturn-type` or
    /// `-Wmissing-noreturn` is enabled.
    pub(crate) fn infer_noreturn(&mut self, f: DeclId, body: StmtId) {
        if self.clang_compat < 22
            || (!self.diags.is_enabled(DiagId::sema_falloff_nonvoid)
                && !self.diags.is_enabled(DiagId::sema_missing_noreturn))
        {
            return;
        }
        if self.diags.uncompilable_error_count() > 0 {
            return;
        }
        let named_template = |s: &Self| {
            s.ast.spec_info(f).is_some_and(|(_, i)| {
                s.kind_of(i.template) == DeclKind::FunctionTemplate && !s.is_lambda_call_op(f)
            })
        };
        let inherits = !self.inst.stack.is_empty() && !named_template(self);
        if (self.in_template() && self.is_named_function_template(f))
            || self.ast.is_noreturn_fn(f)
            || crate::class::is_virtual(&self.ast, f)
            || !self.always_throws(body)
        {
            return;
        }
        let first = self.ast.first(f);
        // A lambda's call operator below C++23 is inferred silently: clang
        // 22 neither warns there nor at a later caller (`-Wreturn-type`
        // stays quiet), at namespace or block scope alike (range-v3
        // `chunk_by.cpp`, `[](int, int) -> bool { throw 0; }` under
        // `-std=c++20`).
        let silent = self.std < kiln_lex::Std::Cxx23 && self.is_lambda_call_op(f);
        if !self.inferred_noreturn.insert(first) || inherits || silent {
            return;
        }
        let loc = self.ast.decls.get(f).loc();
        let msg = self.diag(DiagId::sema_missing_noreturn, loc).arg(self.fn_diag_name(f));
        self.emit(msg);
    }

    /// clang `isKnownToAlwaysThrow`: the body is a compound statement of
    /// one throw-expression (under its cleanups).
    fn always_throws(&self, body: StmtId) -> bool {
        let st = &self.ast.stmts;
        if st.get(body).kind() != StmtKind::CompoundStmt {
            return false;
        }
        let mut it = self.ast.lists.iter::<StmtId>(st.compound_body(body));
        let (Some(only), None) = (it.next(), it.next()) else { return false };
        self.stmt_expr(only).is_some_and(|e| self.ast.exprs.get(e).kind() == ExprKind::CXXThrowExpr)
    }

    /// The expression of expression statement `s`, under its cleanups.
    fn stmt_expr(&self, s: StmtId) -> Option<ExprId> {
        let st = &self.ast.stmts;
        if st.get(s).kind() != StmtKind::ExprStmt {
            return None;
        }
        let e = st.expr_stmt_expr(s);
        if self.ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups {
            return self.ast.exprs.children(e, &self.ast.lists).first().copied();
        }
        Some(e)
    }

    /// clang 22: a body whose last statement is a throw-expression or a
    /// call of a function inferred `noreturn` does not warn
    /// `-Wreturn-type` (clang `CheckFallThroughForBody`).
    pub(crate) fn ends_in_inferred_noreturn(&self, body: StmtId) -> bool {
        let st = &self.ast.stmts;
        if self.clang_compat < 22 || st.get(body).kind() != StmtKind::CompoundStmt {
            return false;
        }
        let Some(last) = self.ast.lists.iter::<StmtId>(st.compound_body(body)).last() else {
            return false;
        };
        let Some(e) = self.stmt_expr(last) else { return false };
        match self.ast.exprs.get(e).kind() {
            ExprKind::CXXThrowExpr => true,
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                self.callee_decl(e)
                    .is_some_and(|d| self.inferred_noreturn.contains(&self.ast.first(d)))
            }
            _ => false,
        }
    }

    /// `-Wmissing-noreturn` at the start of body `body` of `f`, a
    /// function returning `void` (or a constructor or destructor) whose
    /// graph has no live edge into the exit that falls off or returns
    /// (clang `NeverFallThroughOrReturn`). Not for a virtual function, a
    /// lambda, an instantiation, nor a function declared or inferred
    /// `noreturn`; nothing without a graph.
    pub(crate) fn suggest_noreturn(&mut self, f: DeclId, body: StmtId) {
        if !self.diags.is_enabled(DiagId::sema_missing_noreturn)
            || !self.inst.stack.is_empty()
            || self.is_lambda_call_op(f)
            || crate::class::is_virtual(&self.ast, f)
            || self.inferred_noreturn.contains(&self.ast.first(f))
        {
            return;
        }
        let Some(k) = self.with_cfg(body, |_, c| c.exit_kinds()) else { return };
        if k.plain || k.ret {
            return;
        }
        let loc = self.ast.stmts.get(body).loc();
        let msg = self.diag(DiagId::sema_missing_noreturn, loc).arg(self.fn_diag_name(f));
        self.emit(msg);
    }
}
