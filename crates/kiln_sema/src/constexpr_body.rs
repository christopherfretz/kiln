//! The variable rule of a constexpr function body before C++23
//! ([dcl.constexpr]/3 in C++20: the body may not define a variable of
//! non-literal type; clang `CheckConstexprDeclStmt` under
//! `CheckConstexprFunctionDefinition`; spec/sema.md §7.10).
//!
//! Checked when a `constexpr` or `consteval` function definition is
//! finished, in statement order: the first such variable is an error (with
//! `RequireLiteralType`'s notes) and makes the function invalid, so a
//! constant expression calling it reports nothing more, as clang reports
//! once. In a template (a function template, a member of a class template,
//! a generic lambda) only a variable of non-dependent type is checked, at
//! the definition; instantiations are not checked (clang makes an
//! instantiation that breaks the rule silently non-constexpr). A lambda's
//! call operator is checked only when the lambda is declared `constexpr`
//! or `consteval` explicitly; an implicitly constexpr one that breaks the
//! rule is silently not constexpr instead, also when an enclosing
//! template is instantiated. WP-414.

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, VarStorage};
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, StmtId};
use kiln_diag::DiagId;

use crate::Sema;

impl Sema<'_, '_> {
    /// [dcl.constexpr]/3 before C++23 for the definition of `f` with
    /// `body` (see the module docs).
    pub(crate) fn check_constexpr_body_vars(&mut self, f: DeclId, body: StmtId) {
        if !self.is_lambda_call_op(f) {
            self.constexpr_body_vars(f, body);
        }
    }

    /// The same rule for the call operator `f` of a lambda declared
    /// `constexpr` or `consteval` explicitly, at the end of its `body`.
    pub(crate) fn check_constexpr_lambda_body_vars(&mut self, f: DeclId, body: StmtId) {
        self.constexpr_body_vars(f, body);
    }

    /// The call operator `f` of a lambda not declared `constexpr`, at the
    /// end of its `body` (in a template, and again when an enclosing
    /// template is instantiated): before C++23 it is constexpr only if no
    /// variable it defines has a non-literal type ([expr.prim.lambda.
    /// closure]/5 with [dcl.constexpr]/3; clang `CheckConstexprKind::
    /// CheckValid`). A generic lambda's specializations follow its
    /// pattern.
    pub(crate) fn implicit_constexpr_lambda_body_vars(&mut self, f: DeclId, body: StmtId) {
        let spec = self.fn_spec(f);
        if self.std >= kiln_lex::Std::Cxx23
            || spec.constexpr_kind() != ConstexprKind::Constexpr
            || self.first_non_literal_var(body).is_none()
        {
            return;
        }
        let s = spec.with_constexpr_kind(ConstexprKind::None);
        self.ast.decls.respecify_function(f, s);
    }

    fn constexpr_body_vars(&mut self, f: DeclId, body: StmtId) {
        if self.std >= kiln_lex::Std::Cxx23
            || !self.inst.stack.frames.is_empty()
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
            || self.fn_spec(f).constexpr_kind() == ConstexprKind::None
        {
            return;
        }
        let Some(v) = self.first_non_literal_var(body) else { return };
        let ty = self.ast.decls.get(v).ty().unwrap_or(kiln_ast::QualType::ERROR);
        let ctor = self.kind_of(f) == DeclKind::CXXConstructor;
        let msg = self
            .diag(DiagId::sema_constexpr_local_var_non_literal, self.ast.decls.get(v).loc())
            .arg(u8::from(ctor))
            .arg(self.ty_arg(ty));
        let msg = self.with_non_literal_notes(msg, ty);
        self.emit(msg);
        self.ast.decls.set_flags(f, DeclFlags::INVALID);
        // A template's deduction still runs (`TemplState::body_invalid`).
        self.templ.body_invalid.insert(f);
    }

    /// The first variable defined in `body`, in source order, whose type
    /// is a complete non-literal type.
    fn first_non_literal_var(&self, body: StmtId) -> Option<DeclId> {
        let st = &self.ast.stmts;
        let mut stack = vec![body];
        while let Some(s) = stack.pop() {
            let mut vars: Vec<DeclId> = Vec::new();
            let mut subs: Vec<StmtId> = Vec::new();
            match st.get(s).kind() {
                StmtKind::DeclStmt => {
                    vars.extend(self.ast.lists.iter::<DeclId>(st.decl_stmt_decls(s)));
                }
                StmtKind::IfStmt => {
                    let d = st.if_data(s);
                    subs.extend(d.init);
                    vars.extend(d.cond_var);
                }
                StmtKind::SwitchStmt => {
                    let d = st.switch_data(s);
                    subs.extend(d.init);
                    vars.extend(d.cond_var);
                }
                StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                    let d = st.loop_data(s);
                    subs.extend(d.init);
                    vars.extend(d.cond_var);
                }
                StmtKind::CXXForRangeStmt => {
                    let d = st.range_for_data(s);
                    subs.extend(d.init);
                    vars.push(d.range);
                    vars.extend(d.begin);
                    vars.extend(d.end);
                    vars.push(d.loop_var);
                }
                _ => {}
            }
            // An init-statement precedes the condition variable.
            for &i in &subs {
                if let Some(v) = self.first_non_literal_var(i) {
                    return Some(v);
                }
            }
            if let Some(v) = vars.into_iter().find(|&v| self.is_non_literal_local(v)) {
                return Some(v);
            }
            let children = self.children(s);
            stack.extend(children.into_iter().rev());
        }
        None
    }

    /// Whether `v` is a local variable definition of a complete,
    /// non-dependent, non-literal type.
    fn is_non_literal_local(&self, v: DeclId) -> bool {
        let node = self.ast.decls.get(v);
        // An invalid declaration (no viable constructor, say) is checked
        // too, as clang's.
        if node.kind() != DeclKind::Var {
            return false;
        }
        if self.ast.decls.var(v).is_some_and(|x| x.spec.storage() == VarStorage::Extern) {
            return false;
        }
        let Some(ty) = node.ty() else { return false };
        !self.is_error_type(ty) && !self.is_dependent_type(ty) && !self.is_literal_type(ty)
    }
}
