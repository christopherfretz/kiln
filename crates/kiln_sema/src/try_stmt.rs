//! try-blocks and handlers ([except.pre], [except.handle]; spec/sema.md
//! §7.6, WP-100): exception declarations, the handler-order warnings,
//! the scope rule for a thrown variable's implicit move
//! ([class.copy.elision]/3.2) and function-try-blocks.

use kiln_ast::decl::{Access, DeclFlags, DeclKind, InitStyle, VarSpec};
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::names::DeclName;
use kiln_ast::stmt::StmtKind;
use kiln_ast::types::TypeView;
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

    /// The exception-declaration of a handler ([except.handle]/1–3,
    /// clang `ActOnExceptionDeclarator` + `BuildExceptionDeclaration`):
    /// declares the exception variable in the handler's scope.
    pub fn act_on_exception_decl(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
    ) -> Option<DeclId> {
        let type_loc = ds.type_loc.unwrap_or(ds.loc);
        let mut ty = self.type_for_declarator_at(base, type_loc, d);
        let loc = d.name_loc();
        let mut invalid = ds.invalid || d.invalid || self.is_error_type(ty);
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
            let void_ptr = id == DiagId::sema_catch_incomplete_ptr && self.ast.types.is_void(checked);
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
        if !self.exceptions {
            self.error(DiagId::sema_try_disabled, loc);
        }
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

    fn handler_key(&self, ty: QualType) -> HandlerKey {
        let t = &self.ast.types;
        let mut q = t.canonical(ty);
        let pointer = t.is_pointer(q);
        if pointer || t.is_reference(q) {
            q = t.pointee(q).unwrap_or(QualType::ERROR);
        }
        (t.canonical(q).without_quals(), pointer)
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
        let note = self.diag(DiagId::note_previous_exception_handler, at(self, prev)).arg(self.ty_arg(pty));
        let msg = self
            .diag(DiagId::sema_caught_by_earlier_handler, at(self, v))
            .arg(self.ty_arg(ty))
            .with_note(note);
        self.emit(msg);
    }

    /// [class.copy.elision]/3.2: the variable named by the operand of a
    /// throw-expression may be moved from only if its scope does not
    /// extend beyond the innermost enclosing try-block (clang
    /// `IsThrownVarInScope`).
    pub(crate) fn thrown_var_in_scope(&self, e: ExprId) -> bool {
        let mut id = e;
        while self.ast.exprs.get(id).kind() == ExprKind::ParenExpr {
            id = self.ast.exprs.operand(id);
        }
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

    /// True if expression statement `e` is a throw-expression (possibly
    /// under its full-expression's cleanups): control does not continue.
    pub(crate) fn is_throw_stmt_expr(&self, mut e: ExprId) -> bool {
        if self.ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups {
            e = self.ast.exprs.operand(e);
        }
        self.is_throw_expr(e)
    }

    /// The body of a function-try-block of constructor `f`: a `return`
    /// in a handler is ill-formed ([except.handle]/13, clang
    /// `DiagnoseReturnInConstructorExceptionHandler`).
    pub(crate) fn check_function_try_block(&mut self, f: DeclId, body: StmtId) {
        if self.ast.stmts.get(body).kind() != StmtKind::CXXTryStmt
            || self.kind_of(f) != DeclKind::CXXConstructor
        {
            return;
        }
        let handlers: Vec<StmtId> = self.ast.lists.iter(self.ast.stmts.try_handlers(body)).collect();
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
