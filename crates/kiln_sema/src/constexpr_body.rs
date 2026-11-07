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
//!
//! The statement rule ([dcl.constexpr]/3, clang `CheckConstexprFunction
//! Stmt`) is checked in the same walk for the one statement kiln parses
//! that no language level allows, a GNU indirect `goto *p` ("statement not
//! allowed in constexpr function", the function made invalid); and before
//! C++23 a body that passes both rules but holds a label or a `goto`
//! gets "use of this statement in a constexpr function is a C++23
//! extension" at the first. WP-444.

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
            self.check_never_constant(f);
        }
    }

    /// [dcl.constexpr]/6 (no diagnostic required): "constexpr function
    /// never produces a constant expression" at the name of `f`, a
    /// `constexpr` or `consteval` function just defined, when no call of
    /// it is a constant expression (clang `Expr::isPotentialConstantExpr`
    /// under `CheckConstexprFunctionBodies`: on before C++23, else only
    /// with `-Winvalid-constexpr`; an error by default). Narrowly: a
    /// non-member or static member function without parameters, outside
    /// templates and instantiations, whose call fails in its own body on
    /// a signed integer overflow (the note "value N is outside the range
    /// of representable values of type 'T'"); any other failure is not
    /// reported (spec/sema.md §5.28).
    pub(crate) fn check_never_constant(&mut self, f: DeclId) {
        let kind = self.fn_spec(f).constexpr_kind();
        if kind == ConstexprKind::None
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
            || !self.inst.stack.frames.is_empty()
            || self.in_template()
            || !self
                .diags
                .group_flag("invalid-constexpr")
                .unwrap_or(self.std < kiln_lex::Std::Cxx23)
        {
            return;
        }
        match self.kind_of(f) {
            DeclKind::Function => {}
            DeclKind::CXXMethod if self.is_static_method(f) => {}
            _ => return,
        }
        let loc = self.ast.decls.get(f).loc();
        let Err(mut fail) = self.evaluate_call(f, loc) else { return };
        // The call's own "in call to" note; any other is a failure in a
        // callee, which clang does not report.
        if !fail.notes.last().is_some_and(|n| n.loc == loc && n.msg.starts_with("in call to ")) {
            return;
        }
        fail.notes.pop();
        if fail.notes.len() != 1
            || !fail.notes[0].msg.contains(" is outside the range of representable values of type ")
        {
            return;
        }
        let msg = self
            .diag(DiagId::sema_constexpr_never_constant, loc)
            .arg(0u8)
            .arg(u8::from(kind == ConstexprKind::Consteval));
        let msg = Self::with_failure_notes(msg, fail);
        self.emit(msg);
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
        if spec.constexpr_kind() != ConstexprKind::Constexpr
            || (self.body_stmts(body).invalid.is_none()
                && (self.std >= kiln_lex::Std::Cxx23 || self.first_non_literal_var(body).is_none()))
        {
            return;
        }
        let s = spec.with_constexpr_kind(ConstexprKind::None);
        self.ast.decls.respecify_function(f, s);
    }

    fn constexpr_body_vars(&mut self, f: DeclId, body: StmtId) {
        let kind = self.fn_spec(f).constexpr_kind();
        if !self.inst.stack.frames.is_empty()
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
            || kind == ConstexprKind::None
        {
            return;
        }
        let ctor = self.kind_of(f) == DeclKind::CXXConstructor;
        let pre23 = self.std < kiln_lex::Std::Cxx23;
        let stmts = self.body_stmts(body);
        let var = if pre23 { self.first_non_literal_var(body) } else { None };
        // The first violation in source order is reported.
        if let Some(s) = stmts.invalid
            && var.is_none_or(|v| {
                self.ast.stmts.get(s).loc().to_raw() < self.ast.decls.get(v).loc().to_raw()
            })
        {
            let msg = self
                .diag(DiagId::sema_constexpr_body_invalid_stmt, self.ast.stmts.get(s).loc())
                .arg(u8::from(ctor))
                .arg(u8::from(kind == ConstexprKind::Consteval));
            self.emit(msg);
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
            self.templ.body_invalid.insert(f);
            return;
        }
        let Some(v) = var else {
            if pre23 && let Some(loc) = stmts.cxx23 {
                let msg = self
                    .diag(DiagId::sema_constexpr_body_invalid_stmt_cxx23, loc)
                    .arg(u8::from(ctor));
                self.emit(msg);
            }
            return;
        };
        let ty = self.ast.decls.get(v).ty().unwrap_or(kiln_ast::QualType::ERROR);
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

    /// The statements of `body` that the constexpr rules look at, in
    /// source order (clang's walk does not enter expressions).
    fn body_stmts(&self, body: StmtId) -> BodyStmts {
        let st = &self.ast.stmts;
        let mut out = BodyStmts::default();
        let mut stack = vec![body];
        while let Some(s) = stack.pop() {
            match st.get(s).kind() {
                StmtKind::IndirectGotoStmt => {
                    out.invalid = Some(s);
                    return out;
                }
                StmtKind::LabelStmt | StmtKind::GotoStmt => {
                    out.cxx23.get_or_insert(st.get(s).loc());
                }
                _ => {}
            }
            let children = self.children(s);
            stack.extend(children.into_iter().rev());
        }
        out
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
                // clang 19 `CheckConstexprFunctionStmt` checks only an
                // if-statement's branches: its init-statement and
                // condition variable are not (WP-726; googletest's
                // `EXPECT_*` declares a non-literal `AssertionResult`
                // there).
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

/// What [`Sema::body_stmts`] found.
#[derive(Default)]
struct BodyStmts {
    /// The first statement no language level allows.
    invalid: Option<StmtId>,
    /// The first label or `goto` (allowed from C++23).
    cxx23: Option<kiln_source::Loc>,
}
