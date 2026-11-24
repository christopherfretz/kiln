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
//! extension" at the first. WP-444. Otherwise, before C++20, the first
//! asm-declaration or try-block statement gets "... is a C++20
//! extension", and a function-try-block "function try block in constexpr
//! function is a C++20 extension" (clang `CheckConstexprFunctionBody`,
//! WP-891).
//!
//! Before the body rules, the signature rule ([dcl.constexpr]/3 before
//! C++23, clang `CheckConstexprReturnType` / `CheckConstexprParameterTypes`):
//! a non-constructor's return type and each parameter type are literal
//! (`void` is not before C++14), checked at the written definition only,
//! never for a dependent or incomplete type (WP-1123).

use kiln_ast::decl::{ConstexprKind, CtorInitKind, DeclFlags, DeclKind, VarStorage};
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, StmtId};
use kiln_diag::DiagId;

use crate::Sema;

impl Sema<'_, '_> {
    /// clang `CheckConstexprFunctionDefinition` for the definition of `f`
    /// with `body`, up to the never-constant check: the virtual-base
    /// rule, then [dcl.constexpr]/3 before C++23 and the missing `return`
    /// (see the module docs). Run at the end of the body, ahead of the
    /// analysis-based warnings (an error here suppresses them, as in
    /// clang); for a lambda's call operator only when it is declared
    /// `constexpr` or `consteval` explicitly.
    pub(crate) fn check_constexpr_definition(&mut self, f: DeclId, body: StmtId) {
        self.check_constexpr_virtual_base(f);
        self.check_constexpr_signature(f);
        self.constexpr_body_vars(f, body);
    }

    /// clang `CheckConstexprReturnType` and `CheckConstexprParameterTypes`
    /// ([dcl.constexpr]/3 before C++23, removed by P2448R2): the return
    /// type of a constexpr function other than a constructor, then each
    /// parameter type, must be a literal type (`void` is one from C++14).
    /// The first that is not is an error with `RequireLiteralType`'s
    /// notes, and the function is invalid. Checked at the definition,
    /// skipping dependent types; an instantiation is not checked (clang
    /// makes it silently non-constexpr, which no constant expression can
    /// tell apart: it cannot pass or return the non-literal value).
    /// WP-1123.
    fn check_constexpr_signature(&mut self, f: DeclId) {
        let kind = self.fn_spec(f).constexpr_kind();
        if self.std >= kiln_lex::Std::Cxx23
            || kind == ConstexprKind::None
            || !self.inst.stack.frames.is_empty()
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
        {
            return;
        }
        let consteval = u8::from(kind == ConstexprKind::Consteval);
        let ctor = self.kind_of(f) == DeclKind::CXXConstructor;
        let ret = if ctor { kiln_ast::QualType::ERROR } else { self.fn_ret(f) };
        let msg = if self.non_literal_signature_type(ret, true) {
            let msg = self
                .diag(DiagId::sema_constexpr_non_literal_return, self.ast.decls.get(f).loc())
                .arg(consteval)
                .arg(self.ty_arg(ret));
            self.with_non_literal_notes(msg, ret)
        } else {
            let Some((i, p, ty)) = self.non_literal_param(f) else { return };
            let msg = self
                .diag(DiagId::sema_constexpr_non_literal_param, self.ast.decls.get(p).loc())
                .arg(consteval)
                .arg(u8::from(ctor))
                .arg(crate::expr::ordinal(i + 1))
                .arg(self.ty_arg(ty));
            self.with_non_literal_notes(msg, ty)
        };
        self.emit(msg);
        self.invalidate(f);
        self.templ.body_invalid.insert(f);
    }

    /// The first parameter of `f` (its index, declaration and type) whose
    /// type is not literal, as [`Sema::check_constexpr_signature`] checks.
    fn non_literal_param(&self, f: DeclId) -> Option<(usize, DeclId, kiln_ast::QualType)> {
        let params = self.ast.decls.function(f)?.params;
        self.ast.lists.iter::<DeclId>(params).enumerate().find_map(|(i, p)| {
            let node = self.ast.decls.get(p);
            let ty = node.ty()?;
            (!node.has(DeclFlags::INVALID) && self.non_literal_signature_type(ty, false))
                .then_some((i, p, ty))
        })
    }

    /// Whether `ty`, a constexpr function's return type (`ret`) or a
    /// parameter type, breaks [dcl.constexpr]/3 before C++23: a complete,
    /// non-dependent type that is not literal (an incomplete one is
    /// diagnosed by the definition itself).
    fn non_literal_signature_type(&self, ty: kiln_ast::QualType, ret: bool) -> bool {
        if self.is_error_type(ty) || self.is_dependent_type(ty) || self.contains_undeduced_auto(ty)
        {
            return false;
        }
        if self.ast.types.is_void(ty) {
            return ret && self.std < kiln_lex::Std::Cxx14;
        }
        let elem = self.base_element(ty);
        if let Some(r) = self.ast.types.as_record(elem)
            && self.ast.definition(r).is_none()
        {
            return false;
        }
        !self.is_literal_type(ty)
    }

    /// The never-constant check of the definition of `f` with `body`
    /// (`never_constant.rs`), once the body is attached.
    pub(crate) fn check_constexpr_body_vars(&mut self, f: DeclId, body: StmtId) {
        if !self.is_lambda_call_op(f) {
            self.check_never_constant(f, body);
        }
    }

    /// The same for the call operator `f` of a lambda declared
    /// `constexpr` or `consteval` explicitly, at the end of its `body`.
    pub(crate) fn check_constexpr_lambda_body_vars(&mut self, f: DeclId, body: StmtId) {
        self.check_never_constant(f, body);
    }

    /// The call operator `f` of a lambda not declared `constexpr`, at the
    /// end of its `body` (in a template, and again when an enclosing
    /// template is instantiated): before C++23 it is constexpr only if no
    /// variable it defines has a non-literal type ([expr.prim.lambda.
    /// closure]/5 with [dcl.constexpr]/3; clang `CheckConstexprKind::
    /// CheckValid`), and its return and parameter types are literal
    /// ([`Sema::check_constexpr_signature`]). A generic lambda's specializations follow its
    /// pattern.
    pub(crate) fn implicit_constexpr_lambda_body_vars(&mut self, f: DeclId, body: StmtId) {
        let spec = self.fn_spec(f);
        if spec.constexpr_kind() != ConstexprKind::Constexpr
            || (self.body_stmts(body).invalid.is_none()
                && (self.std >= kiln_lex::Std::Cxx23
                    || (self.first_non_literal_var(body).is_none()
                        && self.pre23_body_valid(body)
                        && !self.non_literal_signature_type(self.fn_ret(f), true)
                        && self.non_literal_param(f).is_none())))
        {
            return;
        }
        let s = spec.with_constexpr_kind(ConstexprKind::None);
        self.ast.decls.respecify_function(f, s);
    }

    /// Before C++23, a body with none of the statements or static
    /// variables a constexpr function may hold only as an extension, nor
    /// before C++20 a variable without an initializer (clang
    /// `CheckConstexprKind::CheckValid` fails on each).
    fn pre23_body_valid(&self, body: StmtId) -> bool {
        let stmts = self.body_stmts(body);
        let pre20 = self.std < kiln_lex::Std::Cxx20;
        stmts.cxx23.is_none()
            && stmts.var_exts.iter().all(|&(_, e)| e == VarExt::NoInit && !pre20)
            && (!pre20 || stmts.cxx20.is_none())
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
        // clang `CheckConstexprFunctionBody`: a function-try-block before
        // C++20, ahead of the statement walk (WP-891).
        if self.std < kiln_lex::Std::Cxx20
            && self.ast.stmts.get(body).kind() == StmtKind::CXXTryStmt
        {
            let msg = self
                .diag(
                    DiagId::sema_constexpr_function_try_block_cxx20,
                    self.ast.stmts.get(body).loc(),
                )
                .arg(u8::from(ctor));
            self.emit(msg);
        }
        let stmts = self.body_stmts(body);
        let var = if pre23 { self.first_non_literal_var(body) } else { None };
        // clang `CheckConstexprDeclStmt`: each static or thread_local
        // variable before the first error is a C++23 extension, and
        // before C++20 each variable defined without an initializer a
        // C++20 one, in source order (WP-1677).
        if pre23 {
            let stop = [
                stmts.invalid.map(|s| self.ast.stmts.get(s).loc()),
                var.map(|v| self.ast.decls.get(v).loc()),
            ]
            .into_iter()
            .flatten()
            .map(kiln_source::Loc::to_raw)
            .min();
            for &(loc, ext) in &stmts.var_exts {
                if stop.is_some_and(|m| loc.to_raw() >= m) {
                    break;
                }
                let msg = match ext {
                    VarExt::Static(thread) => self
                        .diag(DiagId::sema_constexpr_static_var, loc)
                        .arg(u8::from(ctor))
                        .arg(u8::from(thread)),
                    VarExt::NoInit if self.std < kiln_lex::Std::Cxx20 => {
                        self.diag(DiagId::sema_constexpr_local_var_no_init, loc).arg(u8::from(ctor))
                    }
                    VarExt::NoInit => continue,
                };
                self.emit(msg);
            }
        }
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
            self.invalidate(f);
            self.templ.body_invalid.insert(f);
            return;
        }
        let Some(v) = var else {
            // clang reports the C++23 statement, else the C++20 one.
            let ext = if pre23 && let Some(loc) = stmts.cxx23 {
                Some((DiagId::sema_constexpr_body_invalid_stmt_cxx23, loc))
            } else if self.std < kiln_lex::Std::Cxx20 {
                stmts.cxx20.map(|loc| (DiagId::sema_constexpr_body_invalid_stmt_cxx20, loc))
            } else {
                None
            };
            if let Some((id, loc)) = ext {
                let msg = self.diag(id, loc).arg(u8::from(ctor));
                self.emit(msg);
            }
            if ctor && self.std < kiln_lex::Std::Cxx20 {
                self.check_constexpr_ctor_inits(f);
            }
            if !ctor && !stmts.has_return {
                self.check_constexpr_missing_return(f, kind);
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
        self.invalidate(f);
        // A template's deduction still runs (`TemplState::body_invalid`).
        self.templ.body_invalid.insert(f);
    }

    /// [dcl.constexpr]/4 before C++20 (P1331R2 lifts it): a constexpr
    /// constructor initializes every non-variant non-static data member
    /// (a union's, exactly one variant member; an anonymous union
    /// member's, one of its members). clang `CheckConstexprFunctionBody`
    /// makes a breach a C++20 extension warning at the constructor, "...
    /// does not initialize all members" with a note at each member left
    /// out (an anonymous struct or union member at itself, unless it has
    /// none to initialize), or for a union "... does not initialize any
    /// member". A member is initialized by a mem-initializer, a default
    /// member initializer, or (class type, not anonymous) default
    /// construction; a delegating constructor and a constructor of a
    /// dependent class are not checked (WP-1685).
    fn check_constexpr_ctor_inits(&mut self, f: DeclId) {
        let Some(class) = self.ast.decls.get(f).ctx() else { return };
        // clang checks a constructor that is not in a dependent context
        // (a template's pattern), and an instantiation only for validity,
        // silently.
        if self.is_dependent_decl_context(class)
            || self.is_dependent_decl_context(f)
            || !self.inst.stack.frames.is_empty()
            || self.is_invalid_decl(class)
        {
            return;
        }
        let mut inited = std::collections::HashSet::new();
        for ci in self.ast.ctor_inits(f) {
            match ci.kind {
                CtorInitKind::Delegating => return,
                CtorInitKind::Field => {
                    inited.insert(DeclId::new(ci.target));
                }
                CtorInitKind::IndirectField => {
                    let chain = self.ast.indirect_field_chain(DeclId::new(ci.target));
                    inited.extend(self.ast.lists.iter::<DeclId>(chain));
                }
                CtorInitKind::Base | CtorInitKind::VirtualBase => {}
            }
        }
        let loc = self.ast.decls.get(f).loc();
        if self.is_union(class) {
            let fields = self.special_fields(class);
            if !fields.is_empty() && !fields.iter().any(|&m| self.ctor_member_inited(m, &inited, 0))
            {
                let msg = self.diag(DiagId::sema_constexpr_union_ctor_no_init, loc);
                self.emit(msg);
            }
            return;
        }
        let mut missing = Vec::new();
        for m in self.special_fields(class) {
            self.ctor_missing_members(m, &inited, &mut missing, 0);
        }
        if missing.is_empty() {
            return;
        }
        let mut msg = self.diag(DiagId::sema_constexpr_ctor_missing_init, loc);
        for m in missing {
            let at = self.ast.decls.get(m).loc();
            msg = msg.with_note(self.diag(DiagId::note_constexpr_ctor_missing_init, at));
        }
        self.emit(msg);
    }

    /// The anonymous struct or union a field's type is, if it is one.
    fn anonymous_member_record(&self, m: DeclId) -> Option<DeclId> {
        if !self.is_anonymous_member(m) {
            return None;
        }
        self.ast.decls.get(m).ty().and_then(|t| self.ast.types.as_record(t))
    }

    /// Whether member `m` of a constexpr constructor's class is
    /// initialized ([`Sema::check_constexpr_ctor_inits`]): named by a
    /// mem-initializer (`inited`), with a default member initializer, of
    /// a class type default-constructed, or an anonymous struct or union
    /// one of whose members is.
    fn ctor_member_inited(
        &self,
        m: DeclId,
        inited: &std::collections::HashSet<DeclId>,
        depth: usize,
    ) -> bool {
        if inited.contains(&m) || self.field_has_nsdmi(m) {
            return true;
        }
        if let Some(r) = self.anonymous_member_record(m) {
            return depth < 32
                && self
                    .special_fields_iter(r)
                    .any(|x| self.ctor_member_inited(x, inited, depth + 1));
        }
        let Some(t) = self.ast.decls.get(m).ty() else { return true };
        !self.ast.types.is_reference(t) && self.element_class(t).is_some()
    }

    /// clang `CheckConstexprCtorInitializer` for member `m`: the members
    /// left out, into `out`.
    fn ctor_missing_members(
        &self,
        m: DeclId,
        inited: &std::collections::HashSet<DeclId>,
        out: &mut Vec<DeclId>,
        depth: usize,
    ) {
        if self.is_invalid_decl(m) || depth > 32 {
            return;
        }
        let anon = self.anonymous_member_record(m);
        // An anonymous union without members, or an anonymous struct
        // without any, needs no initializer.
        if let Some(r) = anon
            && self.special_fields_iter(r).next().is_none()
        {
            return;
        }
        if !self.ctor_member_inited(m, inited, depth) {
            out.push(m);
            return;
        }
        if let Some(r) = anon {
            let union = self.is_union(r);
            for x in self.special_fields(r) {
                if !union || self.ctor_member_inited(x, inited, depth + 1) {
                    self.ctor_missing_members(x, inited, out, depth + 1);
                }
            }
        }
    }

    /// clang `CheckConstexprMissingReturn` ([dcl.constexpr]/3 in C++11;
    /// spec/sema.md §7.10): a constexpr function other than a constructor
    /// whose body has no `return` statement is an error ("no return
    /// statement in constexpr function", the function made invalid, so
    /// the never-constant check and the flow warnings are skipped). Not
    /// for a `void` or dependent return type from C++14, nor for any
    /// return type from C++23 (P2448R2), where clang's remaining
    /// diagnostic is the off-by-default `-Wpre-c++14-compat` one, which
    /// kiln does not have. An undeduced placeholder (a templated
    /// function's) counts as dependent; outside a template it was
    /// deduced `void` by now.
    fn check_constexpr_missing_return(&mut self, f: DeclId, kind: ConstexprKind) {
        let ret = self.fn_ret(f);
        if self.is_error_type(ret) {
            return;
        }
        let void_or_dependent = self.ast.types.is_void(ret)
            || self.is_dependent_type(ret)
            || self.contains_undeduced_auto(ret);
        if (self.std >= kiln_lex::Std::Cxx14 && void_or_dependent)
            || self.std >= kiln_lex::Std::Cxx23
        {
            return;
        }
        let msg = self
            .diag(DiagId::sema_constexpr_body_no_return, self.ast.decls.get(f).loc())
            .arg(u8::from(kind == ConstexprKind::Consteval));
        self.emit(msg);
        self.invalidate(f);
        self.templ.body_invalid.insert(f);
    }

    /// The statements of `body` that the constexpr rules look at, in
    /// source order (clang's walk does not enter expressions: a `return`
    /// in a lambda or a statement expression does not count).
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
                // C++11 allows no iteration statement (nor `continue`):
                // clang does not take them as C++14 extensions.
                StmtKind::WhileStmt
                | StmtKind::DoStmt
                | StmtKind::ForStmt
                | StmtKind::CXXForRangeStmt
                | StmtKind::ContinueStmt
                    if self.std < kiln_lex::Std::Cxx14 =>
                {
                    out.invalid = Some(s);
                    return out;
                }
                StmtKind::ReturnStmt => out.has_return = true,
                StmtKind::DeclStmt => {
                    for v in self.ast.lists.iter::<DeclId>(st.decl_stmt_decls(s)) {
                        let loc = self.ast.decls.get(v).loc();
                        if let Some(thread) = self.local_static_kind(v) {
                            out.var_exts.push((loc, VarExt::Static(thread)));
                        }
                        if self.is_uninit_local(v) {
                            out.var_exts.push((loc, VarExt::NoInit));
                        }
                    }
                }
                StmtKind::LabelStmt | StmtKind::GotoStmt => {
                    out.cxx23.get_or_insert(st.get(s).loc());
                }
                // A function-try-block (the body itself) is reported apart.
                StmtKind::GCCAsmStmt | StmtKind::CXXTryStmt if s != body => {
                    out.cxx20.get_or_insert(st.get(s).loc());
                }
                _ => {}
            }
            // clang walks the children of an iteration or `switch`
            // statement, its init-statement first (not an `if`'s: only
            // its branches); a condition variable and a range-`for`
            // variable are always initialized.
            let init = match st.get(s).kind() {
                StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => st.loop_data(s).init,
                StmtKind::SwitchStmt => st.switch_data(s).init,
                StmtKind::CXXForRangeStmt => st.range_for_data(s).init,
                _ => None,
            };
            let children = self.children(s);
            stack.extend(children.into_iter().rev());
            stack.extend(init);
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

    /// For a block-scope variable `v` of static or thread storage
    /// duration (not `extern`): whether it is `thread_local`.
    fn local_static_kind(&self, v: DeclId) -> Option<bool> {
        if self.kind_of(v) != DeclKind::Var {
            return None;
        }
        let spec = self.ast.decls.var(v)?.spec;
        // A block-scope `extern` declaration defines nothing (CWG2552).
        if spec.storage() == VarStorage::Extern {
            return None;
        }
        if spec.tls() != kiln_ast::decl::TlsKind::None {
            return Some(true);
        }
        (spec.storage() == VarStorage::Static).then_some(false)
    }

    /// Whether `v` is a block-scope variable definition of a
    /// non-dependent type for which no initialization is performed
    /// ([dcl.init.general]/7.3: a scalar, or an array of them; a class
    /// type's default constructor is initialization even when trivial,
    /// as clang's `hasInit`), allowed in a constexpr function from C++20
    /// (P1331R2; clang `ext_constexpr_local_var_no_init`).
    fn is_uninit_local(&self, v: DeclId) -> bool {
        let node = self.ast.decls.get(v);
        if node.kind() != DeclKind::Var || node.has(DeclFlags::INVALID) {
            return false;
        }
        let Some(data) = self.ast.decls.var(v) else { return false };
        if data.init.is_some() || data.spec.storage() == VarStorage::Extern {
            return false;
        }
        node.ty().is_some_and(|ty| !self.is_error_type(ty) && !self.is_dependent_type(ty))
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
    /// The first asm-declaration or try-block statement (allowed from
    /// C++20).
    cxx20: Option<kiln_source::Loc>,
    /// A `return` statement was seen (clang's `ReturnStmts`).
    has_return: bool,
    /// Each variable of static or thread storage duration defined
    /// (allowed from C++23) and each defined without an initializer
    /// (allowed from C++20), in source order: its location and which.
    var_exts: Vec<(kiln_source::Loc, VarExt)>,
}

/// A variable a constexpr function body may define only as an extension.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VarExt {
    /// Of static (`false`) or thread (`true`) storage duration.
    Static(bool),
    /// With no initialization performed.
    NoInit,
}
