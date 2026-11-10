//! try-blocks and handlers ([except.pre], [except.handle]; spec/sema.md
//! §7.6, WP-100): exception declarations, the handler-order warnings,
//! the scope rule for a thrown variable's implicit move
//! ([class.copy.elision]/3.2) and function-try-blocks.

use kiln_ast::decl::{Access, DeclFlags, DeclKind, InitStyle, VarSpec};
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::stmt::StmtKind;
use kiln_ast::types::{ExcSpec, TypeView};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::bases::{BaseSearch, find_base};
use crate::conv::{self, ConvCtx};
use crate::declspec::{DeclSpec, Declarator};
use crate::scope::{Scope, ScopeKind};

/// clang's `CatchHandlerType`: a handler's type with references and one
/// level of pointer stripped, unqualified, and whether it was a pointer.
type HandlerKey = (QualType, bool);

impl Sema<'_, '_> {
    /// Enters the block scope of a try-block's compound statement (clang
    /// `TryScope`), where the search for a thrown variable's scope stops
    /// ([class.copy.elision]/3.2).
    pub fn push_try_scope(&mut self) {
        let mut s = Scope::new(ScopeKind::Block, None);
        s.try_barrier = true;
        self.scopes.push(s);
    }

    /// Enters the scope of a handler ([except.pre]/1; clang
    /// `CatchScope`), where `co_await` and `co_yield` are ill-formed
    /// ([expr.await]/2).
    pub fn push_catch_scope(&mut self) {
        self.push_scope(ScopeKind::Control);
        if let Some(s) = self.scopes.last_mut() {
            s.catch_handler = true;
        }
    }

    /// The exception-declaration of a handler ([except.handle]/1–3,
    /// clang `ActOnExceptionDeclarator` + `BuildExceptionDeclaration`):
    /// declares the exception variable in the handler's scope.
    pub fn act_on_exception_decl(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
    ) -> Option<DeclId> {
        // [temp.local]/6 (clang `ActOnExceptionDeclarator`; WP-1042).
        if let Some((n, l)) = d.name {
            self.check_template_param_shadow(n, l);
        }
        let type_loc = ds.type_loc.unwrap_or(ds.loc);
        let base = self.catch_placeholder_check(base, type_loc, d);
        self.warn_redundant_parens(None, d);
        let mut ty = self.type_for_declarator_at(base, type_loc, d);
        ty = self.check_qualified_fn_type(ty, ds, d);
        let loc = d.name_loc();
        let mut invalid = ds.invalid || d.invalid || self.is_error_type(ty);
        // [temp.variadic]/12: the handler then catches `int`.
        if !invalid && self.diagnose_unexpanded_type_written(ty, "exception type", loc) {
            ty = QualType::INT;
            invalid = true;
        }
        if !invalid && matches!(self.ast.types.canonical_view(ty), TypeView::RValueRef(_)) {
            self.error(DiagId::sema_catch_rvalue_ref, loc);
            invalid = true;
        }
        // [except.handle]/2: arrays and functions decay.
        if self.ast.types.is_array(ty) {
            let elem = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
            ty = self.ast.types.pointer_to(elem);
        } else if self.ast.types.is_function(ty) {
            ty = self.ast.types.pointer_to(ty);
        }
        // [except.handle]/1: no incomplete type, pointer or reference to
        // one (other than cv void*).
        if !invalid && !self.ast.types.is_dependent(ty) {
            let (checked, id) = if self.ast.types.is_pointer(ty) {
                let p = self.ast.types.pointee(ty).unwrap_or(QualType::ERROR);
                (p, DiagId::sema_catch_incomplete_ptr)
            } else if self.ast.types.is_reference(ty) {
                let p = self.ast.types.pointee(ty).unwrap_or(QualType::ERROR);
                (p, DiagId::sema_catch_incomplete_ref)
            } else {
                (ty, DiagId::sema_catch_incomplete)
            };
            let void_ptr =
                id == DiagId::sema_catch_incomplete_ptr && self.ast.types.is_void(checked);
            if !void_ptr
                && !self.ast.types.is_function(checked)
                && !self.require_complete(checked, loc)
            {
                let msg = self.diag(id, loc).arg(self.ty_arg(checked));
                self.emit(self.incomplete_note(msg, checked));
                invalid = true;
            }
        }
        let name = d.name.map(|n| n.0);
        let ctx = self.ctx;
        let mut h = DeclHead::new(ctx, name.map_or(DeclName::EMPTY, DeclName::ident), loc);
        if invalid {
            h.flags = h.flags.with(DeclFlags::INVALID);
        }
        let spec = VarSpec::new().with_exception_var(true);
        let v = self.builder().var(h, ty, spec, None);
        self.handler_type_locs.insert(v, type_loc);
        let mut attrs = ds.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        if !attrs.is_empty() {
            self.check_decl_attrs(v, &mut attrs);
            let _ = self.builder().attach_attrs(v, &attrs);
        }
        if !invalid {
            self.check_abstract_var(v);
        }
        if let Some(n) = name {
            self.register_local(n, v);
        }
        if !self.ast.decls.get(v).has(DeclFlags::INVALID) {
            self.init_exception_var(v, ty, loc);
        }
        Some(v)
    }

    /// [except.handle]/15: a handler variable of class type is
    /// copy-initialized from the exception object (an lvalue
    /// `OpaqueValueExpr`); a non-trivial constructor becomes its
    /// initializer, and its destructor is potentially invoked.
    fn init_exception_var(&mut self, v: DeclId, ty: QualType, loc: Loc) {
        if self.ast.types.is_dependent(ty) || self.ast.types.as_record(ty).is_none() {
            return;
        }
        let obj_ty = conv::strip_cv(self, ty);
        let opaque = self.builder().opaque_value_of(obj_ty, ValueCategory::LValue, loc);
        let init = self.convert_for_init(opaque, ty, ConvCtx::Initialization);
        if self.is_err_expr(init) {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            return;
        }
        let ctor = self.ast.exprs.get(init).kind() == ExprKind::CXXConstructExpr
            && !self.is_trivial_construct(init);
        if ctor {
            let init = self.finish_cleanups(init);
            let _ = self.builder().set_var_init(v, init);
            self.ast.decls.mark_var(v, VarSpec::new().with_init_style(InitStyle::C));
        }
        self.check_var_destructor(v, ty, loc);
    }

    fn is_trivial_construct(&self, e: ExprId) -> bool {
        let c = self.ast.exprs.constructor(e);
        self.ast.decls.function(c).is_some_and(|f| f.spec.is_trivial())
    }

    /// A handler ([except.handle]).
    pub fn act_on_catch(&mut self, var: Option<DeclId>, block: StmtId, loc: Loc) -> StmtId {
        self.builder().catch_stmt(var, block, loc)
    }

    /// A try-block ([except.pre], clang `ActOnCXXTryBlock`): `None` (the
    /// statement is dropped) when a catch-all handler is not last.
    pub fn act_on_try(&mut self, loc: Loc, block: StmtId, handlers: &[StmtId]) -> Option<StmtId> {
        // A try-block is always rebuilt by the substitution, so a deferred
        // check needs no record (unlike a shared throw-expression).
        let _ = self.check_exceptions_enabled(DiagId::sema_try_disabled, loc);
        let mut seen: Vec<(HandlerKey, DeclId)> = Vec::new();
        let mut seen_bases: Vec<(HandlerKey, DeclId)> = Vec::new();
        for (i, &h) in handlers.iter().enumerate() {
            let Some(v) = self.ast.stmts.catch_var(h) else {
                if i + 1 < handlers.len() {
                    self.error(DiagId::sema_early_catch_all, self.ast.stmts.get(h).loc());
                    return None;
                }
                continue;
            };
            let vn = *self.ast.decls.get(v);
            if vn.has(DeclFlags::INVALID) {
                continue;
            }
            let caught = vn.ty().unwrap_or(QualType::ERROR);
            let key = self.handler_key(caught);
            if let Some(rec) = self.ast.types.as_record(key.0)
                && let Some(def) = self.ast.definition(rec)
            {
                if let Some(prev) = self.public_base_handler(def, key.1, &seen_bases) {
                    self.warn_earlier_handler(v, prev);
                }
                let canon = (self.ast.types.canonical(key.0).without_quals(), key.1);
                if !seen_bases.iter().any(|(k, _)| *k == canon) {
                    seen_bases.push((canon, v));
                }
            }
            if let Some(&(_, prev)) = seen.iter().find(|(k, _)| *k == key) {
                self.warn_earlier_handler(v, prev);
            } else {
                seen.push((key, v));
            }
        }
        Some(self.builder().try_stmt(block, handlers, loc))
    }

    /// clang `CatchHandlerType`: the top-level cv-qualifiers are dropped
    /// before a reference or pointer is stripped, so the pointee keeps its
    /// own (`int` and `const int &` are different handlers; `const int`
    /// and `int &` are not).
    fn handler_key(&self, ty: QualType) -> HandlerKey {
        let t = &self.ast.types;
        let mut q = t.canonical(ty).without_quals();
        let pointer = t.is_pointer(q);
        if pointer || t.is_reference(q) {
            q = t.canonical(t.pointee(q).unwrap_or(QualType::ERROR));
        }
        (q, pointer)
    }

    /// The earlier handler whose type is a public, unambiguous base of
    /// class `def` with the same pointer-ness (clang
    /// `CatchTypePublicBases`).
    fn public_base_handler(
        &self,
        def: DeclId,
        pointer: bool,
        seen: &[(HandlerKey, DeclId)],
    ) -> Option<DeclId> {
        for &((bt, p), h) in seen {
            if p != pointer {
                continue;
            }
            let Some(base) = self.ast.types.as_record(bt) else { continue };
            let BaseSearch::Unique(path) = find_base(self, def, base) else { continue };
            let mut from = def;
            let public = path.specs.iter().zip(&path.classes).all(|(&i, &c)| {
                let written = self.ast.decls.base_spec(i).access;
                let ok = self.base_access(from, written) == Access::Public;
                from = c;
                ok
            });
            if public {
                return Some(h);
            }
        }
        None
    }

    fn warn_earlier_handler(&mut self, v: DeclId, prev: DeclId) {
        let at = |s: &Self, d: DeclId| {
            s.handler_type_locs.get(&d).copied().unwrap_or_else(|| s.ast.decls.get(d).loc())
        };
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        let pty = self.ast.decls.get(prev).ty().unwrap_or(QualType::ERROR);
        let note = self
            .diag(DiagId::note_previous_exception_handler, at(self, prev))
            .arg(self.ty_arg(pty));
        let msg = self
            .diag(DiagId::sema_caught_by_earlier_handler, at(self, v))
            .arg(self.ty_arg(ty))
            .with_note(note)
            // clang checks the handlers again on every instantiation, also
            // when the definition already warned (spec/diag.md §7.4).
            .allow_duplicate();
        self.emit(msg);
    }

    /// [class.copy.elision]/3.2: the variable named by the operand of a
    /// throw-expression may be moved from only if its scope does not
    /// extend beyond the innermost enclosing try-block (clang
    /// `IsThrownVarInScope`).
    pub(crate) fn thrown_var_in_scope(&self, e: ExprId) -> bool {
        // clang `IsThrownVarInScope` strips with `IgnoreParens`.
        let id = self.ignore_parens(e);
        if self.ast.exprs.get(id).kind() != ExprKind::DeclRefExpr {
            return false;
        }
        let d = self.ast.exprs.decl(id);
        for s in self.scopes.iter().rev() {
            if s.locals.iter().any(|l| l.1 == d) {
                return true;
            }
            if s.try_barrier || !matches!(s.kind, ScopeKind::Block | ScopeKind::Control) {
                return false;
            }
        }
        false
    }

    /// The body of a function-try-block of constructor `f`: a `return`
    /// in a handler is ill-formed ([except.handle]/13, clang
    /// `DiagnoseReturnInConstructorExceptionHandler`).
    /// Also warns about each member access through `this` in a handler of
    /// a constructor's or destructor's function-try-block, whose object is
    /// gone ([except.handle]/10, clang `IsInFnTryBlockHandler`).
    pub(crate) fn check_function_try_block(&mut self, f: DeclId, body: StmtId) {
        let kind = self.kind_of(f);
        if self.ast.stmts.get(body).kind() != StmtKind::CXXTryStmt
            || !matches!(kind, DeclKind::CXXConstructor | DeclKind::CXXDestructor)
        {
            return;
        }
        let handlers: Vec<StmtId> =
            self.ast.lists.iter(self.ast.stmts.try_handlers(body)).collect();
        for &h in &handlers {
            for loc in self.this_member_refs(self.ast.stmts.catch_block(h)) {
                let msg = self
                    .diag(DiagId::sema_cdtor_function_try_handler_mem_expr, loc)
                    .arg(u8::from(kind == DeclKind::CXXDestructor));
                self.emit(msg);
            }
        }
        if kind != DeclKind::CXXConstructor {
            return;
        }
        for h in handlers {
            let mut todo = vec![self.ast.stmts.catch_block(h)];
            while let Some(s) = todo.pop() {
                if self.ast.stmts.get(s).kind() == StmtKind::ReturnStmt {
                    self.error(DiagId::sema_return_in_ctor_handler, self.ast.stmts.get(s).loc());
                }
                let mut kids = self.children(s);
                kids.reverse();
                todo.extend(kids);
            }
        }
    }
}

impl Sema<'_, '_> {
    /// The locations of the member accesses `this->m` (explicit or
    /// implicit `this`, under implicit casts only) in statement `s`,
    /// outside lambda bodies.
    fn this_member_refs(&self, s: StmtId) -> Vec<Loc> {
        let mut out = Vec::new();
        let mut stmts = vec![s];
        while let Some(s) = stmts.pop() {
            let (mut exprs, mut subs) = (Vec::new(), Vec::new());
            self.stmt_parts(s, &mut exprs, &mut subs);
            for e in exprs {
                let mut stack = vec![e];
                while let Some(x) = stack.pop() {
                    let k = self.ast.exprs.get(x).kind();
                    if k == ExprKind::LambdaExpr {
                        continue;
                    }
                    if k == ExprKind::MemberExpr {
                        let mut b = self.ast.exprs.operand(x);
                        while self.ast.exprs.get(b).kind() == ExprKind::ImplicitCastExpr {
                            b = self.ast.exprs.operand(b);
                        }
                        if self.ast.exprs.get(b).kind() == ExprKind::CXXThisExpr {
                            out.push(self.ast.exprs.get(x).loc());
                        }
                    }
                    let mut kids = self.ast.exprs.children(x, &self.ast.lists);
                    kids.reverse();
                    stack.extend(kids);
                }
            }
            subs.reverse();
            stmts.extend(subs);
        }
        out
    }

    /// clang's `checkThrowInNonThrowingFunc` (-Wexceptions): each
    /// throw-expression in the body of a function with a non-throwing
    /// exception specification ([except.spec]/5) that no enclosing handler
    /// of the function catches ([except.handle]/3) — `std::terminate` is
    /// called ([except.terminate]). Only the throws reachable in the
    /// body's graph count (spec/sema.md §7.5.1); a body with no
    /// throw-expression builds none.
    pub(crate) fn check_throw_in_nothrow(&mut self, f: DeclId, body: StmtId) {
        let t = &self.ast.types;
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let Some(spec) = t.proto(ty).map(|p| t.exc_spec(p)) else { return };
        if !spec.is_nothrow() {
            return;
        }
        let mut found = Vec::new();
        let mut handlers = Vec::new();
        self.escaping_throws(body, &mut handlers, &mut found);
        if found.is_empty() {
            return;
        }
        let Some(mut found) = self.with_cfg(body, Self::cfg_escaping_throws) else {
            return;
        };
        if found.is_empty() {
            return;
        }
        // clang visits the CFG blocks, built back to front.
        found.sort_by_key(|&e| std::cmp::Reverse(self.ast.exprs.get(e).loc()));
        let fname = self.fn_diag_name(f);
        let floc = self.ast.decls.get(f).loc();
        let dealloc = self.ast.decls.get(f).name().as_operator().is_some_and(|o| {
            matches!(o, OverloadedOperator::Delete | OverloadedOperator::ArrayDelete)
        });
        let dtor = self.ast.decls.get(f).kind() == DeclKind::CXXDestructor;
        // `noexcept`/`throw()` written vs. the implicit specification of a
        // destructor or deallocation function ([except.spec]/8, /9).
        let implicit = matches!(spec, ExcSpec::Nothrow);
        for e in found {
            let loc = self.ast.exprs.get(e).loc();
            let note = if dtor || dealloc {
                self.diag(DiagId::note_throw_in_dtor, floc)
                    .arg(u8::from(dealloc))
                    .arg(u8::from(implicit))
            } else {
                self.diag(DiagId::note_throw_in_function, floc)
            };
            let msg = self
                .diag(DiagId::sema_throw_in_nothrow_func, loc)
                .arg(fname.clone())
                .with_note(note);
            self.emit(msg);
        }
    }

    /// The throw-expressions in `s` not caught by `handlers` (the handler
    /// lists of the enclosing try-blocks, innermost last).
    fn escaping_throws(&self, s: StmtId, handlers: &mut Vec<StmtId>, out: &mut Vec<ExprId>) {
        let st = &self.ast.stmts;
        let n = st.get(s);
        let mut exprs: Vec<ExprId> = Vec::new();
        let mut subs: Vec<StmtId> = Vec::new();
        if n.kind() == StmtKind::CXXTryStmt {
            handlers.push(s);
            self.escaping_throws(st.try_block(s), handlers, out);
            handlers.pop();
            for h in self.ast.lists.iter::<StmtId>(st.try_handlers(s)) {
                self.escaping_throws(st.catch_block(h), handlers, out);
            }
            return;
        }
        self.stmt_parts(s, &mut exprs, &mut subs);
        for e in exprs {
            self.escaping_throws_in_expr(e, handlers, out);
        }
        for c in subs {
            self.escaping_throws(c, handlers, out);
        }
    }

    /// The expressions directly in statement `s` and its sub-statements
    /// (in that order).
    pub(crate) fn stmt_parts(&self, s: StmtId, exprs: &mut Vec<ExprId>, subs: &mut Vec<StmtId>) {
        let st = &self.ast.stmts;
        match st.get(s).kind() {
            StmtKind::ExprStmt => exprs.push(st.expr_stmt_expr(s)),
            StmtKind::ReturnStmt => exprs.extend(st.return_value(s)),
            StmtKind::IndirectGotoStmt => exprs.push(st.indirect_goto_target(s)),
            StmtKind::CoreturnStmt => {
                exprs.extend(st.coreturn_operand(s));
                exprs.extend(st.coreturn_promise_call(s));
            }
            StmtKind::DeclStmt => {
                for d in self.ast.lists.iter::<DeclId>(st.decl_stmt_decls(s)) {
                    exprs.extend(self.ast.decls.var(d).and_then(|v| v.init));
                }
            }
            StmtKind::IfStmt => {
                let d = *st.if_data(s);
                subs.extend(d.init);
                exprs.extend(d.cond_var.and_then(|v| self.ast.decls.var(v).and_then(|x| x.init)));
                exprs.extend(d.cond);
            }
            StmtKind::SwitchStmt => {
                let d = *st.switch_data(s);
                subs.extend(d.init);
                exprs.extend(d.cond_var.and_then(|v| self.ast.decls.var(v).and_then(|x| x.init)));
                exprs.push(d.cond);
            }
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                let d = *st.loop_data(s);
                subs.extend(d.init);
                exprs.extend(d.cond_var.and_then(|v| self.ast.decls.var(v).and_then(|x| x.init)));
                exprs.extend(d.cond);
                exprs.extend(d.inc);
            }
            _ => {}
        }
        subs.extend(self.children(s));
    }

    fn escaping_throws_in_expr(&self, e: ExprId, handlers: &[StmtId], out: &mut Vec<ExprId>) {
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let k = self.ast.exprs.get(x).kind();
            if k == ExprKind::LambdaExpr {
                continue;
            }
            if k == ExprKind::CXXThrowExpr && !self.throw_caught(x, handlers) {
                out.push(x);
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
    }

    /// Whether some handler of the enclosing try-blocks catches the throw
    /// `e` (clang `isThrowCaught`: a catch-all, the same type, or a base
    /// class, through one reference or pointer level).
    pub(crate) fn throw_caught(&self, e: ExprId, tries: &[StmtId]) -> bool {
        let t = &self.ast.types;
        let thrown = self.ast.exprs.throw_operand(e).map(|o| self.ast.exprs.get(o).ty());
        for &tr in tries.iter().rev() {
            for h in self.ast.lists.iter::<StmtId>(self.ast.stmts.try_handlers(tr)) {
                let Some(v) = self.ast.stmts.catch_var(h) else { return true };
                let Some(thrown) = thrown else { continue };
                let mut caught = t.canonical(self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR));
                if t.is_reference(caught) {
                    caught = t.pointee(caught).unwrap_or(QualType::ERROR);
                }
                let mut th = t.canonical(thrown);
                if t.is_pointer(th) && t.is_pointer(caught) {
                    th = t.pointee(th).unwrap_or(QualType::ERROR);
                    caught = t.pointee(caught).unwrap_or(QualType::ERROR);
                }
                let (th, caught) =
                    (t.canonical(th).without_quals(), t.canonical(caught).without_quals());
                if th == caught {
                    return true;
                }
                if let (Some(d), Some(b)) = (t.as_record(th), t.as_record(caught))
                    && let Some(def) = self.ast.definition(d)
                    && !matches!(find_base(self, def, b), BaseSearch::NotDerived)
                {
                    return true;
                }
            }
        }
        false
    }
}
