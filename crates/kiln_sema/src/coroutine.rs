//! Coroutines ([dcl.fct.def.coroutine], [expr.await], [expr.yield],
//! [stmt.return.coroutine]; spec/sema.md §coroutines, clang
//! `SemaCoroutine.cpp`).
//!
//! The first `co_await`/`co_yield`/`co_return` of a function body makes
//! it a coroutine: the context is checked, the parameter copies and the
//! promise object are declared, and the implicit initial and final
//! suspend points are built. At the end of the body
//! ([`Sema::complete_coroutine_body`]) the body is wrapped in a
//! `CoroutineBodyStmt` carrying the statements and expressions that
//! code generation lowers. A template pattern keeps the dependent forms
//! (`DependentCoawaitExpr`, a one-operand `CoyieldExpr`, a `CoreturnStmt`
//! without promise call); its instantiation builds the coroutine.

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, FnSpec, FnStorage, InitStyle, VarSpec};
use kiln_ast::expr::{CastKind, CoawaitParts, ExprKind, SuspendKind, UnaryOp, ValueCategory};
use kiln_ast::names::DeclName;
use kiln_ast::stmt::CoroutineBodyData;
use kiln_ast::types::{Nns, RefQual, TagKeyword, TemplateArg};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType, StmtId};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;
use crate::conv::{self, ConvCtx};
use crate::expr::CastStyle;
use crate::init::Initializer;
use crate::lookup::LookupKind;
use crate::overload::oper::{OperForm, OperResult};
use crate::scope::ScopeKind;
use crate::templ::actions::TemplateIdRef;
use crate::using::Qualifier;

/// The coroutine state of a function body (clang's `FunctionScopeInfo`
/// coroutine members).
#[derive(Clone, Debug, Default)]
pub(crate) struct CoroState {
    /// The first coroutine keyword as written (clang
    /// `FirstCoroutineStmtLoc`).
    pub(crate) first: Option<(Loc, &'static str)>,
    /// The promise object `__promise`, once built.
    pub(crate) promise: Option<DeclId>,
    /// The parameter copies: (parameter, `DeclStmt` of its copy).
    pub(crate) moves: Vec<(DeclId, StmtId)>,
    /// The implicit initial and final suspend expressions: `None` until
    /// built, `Some(None)` after they failed.
    #[allow(clippy::option_option, reason = "not built yet vs. failed")]
    pub(crate) suspends: Option<Option<(ExprId, ExprId)>>,
    /// `std::coroutine_handle<P>`, once looked up.
    pub(crate) handle_ty: Option<QualType>,
}

/// The parts of an awaiter's three calls.
struct AwaitCalls {
    opaque: ExprId,
    ready: ExprId,
    suspend: ExprId,
    resume: ExprId,
    kind: SuspendKind,
}

impl Sema<'_, '_> {
    // -- context ----------------------------------------------------------------

    /// The function whose body is being analysed, when the current
    /// context is that function (not a default argument, a class, or a
    /// namespace).
    fn coro_function(&self) -> Option<DeclId> {
        let f = self.func.as_ref()?.decl;
        (f == self.ctx && self.ast.decls.get(f).kind().is_function()).then_some(f)
    }

    /// The `co_await`/`co_yield` restrictions of [expr.await]/2 (clang
    /// `checkSuspensionContext`): a potentially-evaluated expression
    /// outside a handler.
    fn check_suspension_context(&mut self, loc: Loc, kw: &'static str) -> bool {
        if self.unevaluated > 0 {
            let d = self.diag(DiagId::sema_coroutine_unevaluated_context, loc).arg(kw);
            self.emit(d);
            return false;
        }
        for s in self.scopes.iter().rev() {
            if s.catch_handler {
                let d = self.diag(DiagId::sema_coroutine_within_handler, loc).arg(kw);
                self.emit(d);
                return false;
            }
            if s.kind == ScopeKind::Function {
                break;
            }
        }
        true
    }

    /// clang `isValidCoroutineContext`: a function that may be a
    /// coroutine ([class.ctor]/11, [class.dtor]/17, [basic.start.main]/3,
    /// [expr.const]/10, [dcl.spec.auto.general]/15,
    /// [dcl.fct.def.coroutine]/1).
    fn valid_coroutine_context(&mut self, loc: Loc, kw: &'static str) -> Option<DeclId> {
        let Some(f) = self.coro_function() else {
            let d = self.diag(DiagId::sema_coroutine_outside_function, loc).arg(kw);
            self.emit(d);
            return None;
        };
        let bad = |s: &Self, sel: u8| {
            let d = s.diag(DiagId::sema_coroutine_invalid_func_context, loc).arg(sel).arg(kw);
            s.emit(d);
        };
        match self.kind_of(f) {
            DeclKind::CXXConstructor => {
                bad(self, 0);
                return None;
            }
            DeclKind::CXXDestructor => {
                bad(self, 1);
                return None;
            }
            _ => {}
        }
        if self.is_main_function(f) {
            bad(self, 2);
            return None;
        }
        let mut ok = true;
        let spec = self.fn_spec(f);
        match spec.constexpr_kind() {
            ConstexprKind::Constexpr | ConstexprKind::Consteval
                if self.is_lambda_call_op(f) && !self.lambda_explicitly_constexpr(f) =>
            {
                // An implicitly constexpr lambda call operator is not
                // constexpr once it is a coroutine ([expr.prim.lambda.
                // closure]/5: it does not satisfy the requirements).
                let s = spec.with_constexpr_kind(ConstexprKind::None);
                self.ast.decls.respecify_function(f, s);
            }
            ConstexprKind::Constexpr => {
                bad(self, 3);
                ok = false;
            }
            ConstexprKind::Consteval => {
                bad(self, 6);
                ok = false;
            }
            ConstexprKind::None => {}
        }
        if self.func.as_ref().is_some_and(|s| s.auto_ret.is_some() && !s.deduced) {
            bad(self, 4);
            ok = false;
        }
        if spec.is_variadic() {
            bad(self, 5);
            ok = false;
        }
        ok.then_some(f)
    }

    /// `main` at namespace scope of the global namespace.
    pub(crate) fn is_main_function(&self, f: DeclId) -> bool {
        self.decl_name(f) == "main"
            && self
                .ast
                .decls
                .get(f)
                .ctx()
                .is_some_and(|c| self.ast.redecl_context(c) == self.ast.tu)
    }

    /// clang `checkCoroutineContext`: the context is valid, the function
    /// is a coroutine, and its promise exists (built at the first
    /// keyword). Returns the promise.
    fn check_coroutine_context(
        &mut self,
        loc: Loc,
        kw: &'static str,
        implicit: bool,
    ) -> Option<DeclId> {
        let f = self.valid_coroutine_context(loc, kw)?;
        self.mark_coroutine(f, loc, kw, implicit);
        if let Some(p) = self.coro_state().and_then(|c| c.promise) {
            return Some(p);
        }
        if self.in_template() {
            return None;
        }
        if !self.build_param_moves(f, loc) {
            return None;
        }
        let p = self.build_promise(f, loc)?;
        if let Some(c) = self.coro_state_mut() {
            c.promise = Some(p);
        }
        Some(p)
    }

    /// Records the first keyword and marks `f` a coroutine.
    fn mark_coroutine(&mut self, f: DeclId, loc: Loc, kw: &'static str, implicit: bool) {
        let Some(st) = self.func.as_mut() else { return };
        let c = st.coro.get_or_insert_with(Box::default);
        if c.first.is_none() && !implicit {
            c.first = Some((loc, kw));
        }
        self.ast.decls.mark_function(f, FnSpec::new().with_coroutine(true));
    }

    pub(crate) fn coro_state(&self) -> Option<&CoroState> {
        self.func.as_ref()?.coro.as_deref()
    }

    fn coro_state_mut(&mut self) -> Option<&mut CoroState> {
        self.func.as_mut()?.coro.as_deref_mut()
    }

    /// The `note_declared_coroutine_here` note for the first keyword.
    fn note_coroutine_here(&self) -> Option<Diag> {
        let (loc, kw) = self.coro_state()?.first?;
        Some(self.diag(DiagId::note_declared_coroutine_here, loc).arg(kw))
    }

    /// Runs `f` outside the enclosing full-expression: its cleanups,
    /// pending odr-uses, lifetime-extension collector and unevaluated
    /// depth are set aside (the implicit parts of a coroutine are their
    /// own full-expressions).
    fn coro_isolated<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let cleanup = std::mem::take(&mut self.cleanup);
        let odr = std::mem::take(&mut self.pending_odr);
        let ext = self.extend_collector.take();
        let unev = std::mem::replace(&mut self.unevaluated, 0);
        let r = f(self);
        self.cleanup = cleanup;
        let mine = std::mem::replace(&mut self.pending_odr, odr);
        self.pending_odr.extend(mine);
        self.extend_collector = ext;
        self.unevaluated = unev;
        r
    }

    /// Whether `stash` holds an error.
    pub(crate) fn stash_has_error(&self, stash: &[Diag]) -> bool {
        stash.iter().any(|d| {
            d.id()
                .is_some_and(|id| self.diags.severity_at(id, d.loc()) >= kiln_diag::Severity::Error)
        })
    }

    // -- parameter copies and the promise ------------------------------------------

    /// An implicit local variable `name` of type `ty` (clang
    /// `buildVarDecl`).
    fn coro_var(&mut self, name: &str, ty: QualType, loc: Loc, style: InitStyle) -> DeclId {
        let sym = self.intern(name);
        let mut h = DeclHead::new(self.ctx, DeclName::ident(sym), loc);
        h.flags = h.flags.with(DeclFlags::IMPLICIT);
        let spec = VarSpec::new().with_init_style(style);
        self.builder().var(h, ty, spec, None)
    }

    /// [dcl.fct.def.coroutine]/13: a copy of each parameter, initialized
    /// from an xvalue of a class or rvalue-reference parameter and from
    /// an lvalue of the others (clang `buildCoroutineParameterMoves`).
    fn build_param_moves(&mut self, f: DeclId, loc: Loc) -> bool {
        if self.coro_state().is_some_and(|c| !c.moves.is_empty()) {
            return false;
        }
        let mut moves = Vec::new();
        for p in self.param_decls(f) {
            let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
            if self.ast.types.is_dependent(pty) {
                continue;
            }
            let nr = self.non_reference(pty);
            let r = self.builder().decl_ref(
                p,
                nr,
                loc,
                None,
                kiln_ast::expr::NonOdrUse::None,
                kiln_ast::expr::ExprFlags::NONE,
            );
            let by_move = self.ast.types.as_record(pty).is_some()
                || self.ast.types.canonical_kind(pty) == kiln_ast::types::TypeKind::RValueRef;
            let init = if by_move {
                self.builder().implicit_cast(CastKind::NoOp, r, nr, ValueCategory::XValue)
            } else {
                r
            };
            let name = self.ast.decls.get(p).name();
            let name = name.as_ident().map(|s| self.ast.symbols().as_str(s).to_string());
            let v = self.coro_var(name.as_deref().unwrap_or(""), pty, loc, InitStyle::Call);
            self.act_on_init(v, Initializer::Paren(vec![init], loc, loc));
            let s = self.act_on_decl_stmt(&[v], loc);
            moves.push((p, s));
        }
        if let Some(c) = self.coro_state_mut() {
            c.moves = moves;
        }
        true
    }

    /// The copy of parameter `p`.
    fn param_copy(&self, p: DeclId) -> Option<DeclId> {
        let s = self.coro_state()?.moves.iter().find(|m| m.0 == p)?.1;
        self.ast.lists.iter::<DeclId>(self.ast.stmts.decl_stmt_decls(s)).next()
    }

    /// `std::NAME`: the namespace and the declarations found.
    fn std_lookup(&self, name: &str) -> Option<(DeclId, Vec<DeclId>)> {
        let std = self.intern("std");
        let ns = self.lookup_qualified(self.ast.tu, std, LookupKind::Nested);
        let ns = ns.decls.iter().copied().find(|&d| self.kind_of(d) == DeclKind::Namespace)?;
        let n = self.intern(name);
        let found = self.lookup_qualified(ns, n, LookupKind::Ordinary);
        if found.decls.is_empty() {
            return None;
        }
        Some((self.ast.first(ns), found.decls))
    }

    /// The class template `std::NAME` (clang `lookupCoroutineTraits`,
    /// `lookupCoroutineHandleType`): diagnosed when missing or not a
    /// class template.
    fn std_coro_template(
        &mut self,
        name: &str,
        loc: Loc,
        malformed: DiagId,
    ) -> Option<(DeclId, DeclId)> {
        let Some((ns, found)) = self.std_lookup(name) else {
            let d = self
                .diag(DiagId::sema_implied_coroutine_type_not_found, loc)
                .arg(format!("std::{name}"));
            self.emit(d);
            return None;
        };
        if let Some(&t) = found.iter().find(|&&d| self.kind_of(d) == DeclKind::ClassTemplate) {
            return Some((ns, t));
        }
        self.error(malformed, self.ast.decls.get(found[0]).loc());
        None
    }

    /// The declared return type of `f`, as written.
    fn written_return(&self, f: DeclId) -> QualType {
        let Some(ty) = self.ast.decls.get(f).ty() else { return QualType::ERROR };
        let t = &self.ast.types;
        t.proto_of(ty.ty())
            .or_else(|| t.proto(ty))
            .map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret)
    }

    /// The promise type `std::coroutine_traits<R, P...>::promise_type`
    /// ([dcl.fct.def.coroutine]/3, /4; clang `lookupPromiseType`).
    fn lookup_promise_type(&mut self, f: DeclId, kw_loc: Loc) -> Option<QualType> {
        let floc = self.ast.decls.get(f).loc();
        let (ns, tmpl) = self.std_coro_template(
            "coroutine_traits",
            kw_loc,
            DiagId::sema_malformed_std_coroutine_traits,
        )?;
        let mut args = vec![TemplateArg::ty(self.written_return(f))];
        // The implicit object parameter of a non-static member function
        // ([over.match.funcs.general]/4).
        if let Some(this) = self.method_this_type(f) {
            let obj = self.ast.types.pointee(this).unwrap_or(QualType::ERROR);
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            let rq = self.ast.types.proto(fty).map_or(RefQual::None, |p| p.info().ref_qual());
            let t = if rq == RefQual::RValue {
                self.ast.types.rvalue_ref_to(obj)
            } else {
                self.ast.types.lvalue_ref_to(obj)
            };
            args.push(TemplateArg::ty(t));
        }
        let (params, _) = self.fn_params(f).unwrap_or_default();
        args.extend(params.into_iter().map(TemplateArg::ty));
        let name = self.template_name_for(tmpl);
        let Ok(TemplateIdRef::Type(traits)) = self.act_template_id(name, &args, kw_loc, kw_loc)
        else {
            return None;
        };
        if !self.require_complete_silent(traits, kw_loc) {
            let d = self
                .diag(DiagId::sema_coroutine_type_missing_specialization, kw_loc)
                .arg(self.ty_arg(traits));
            self.emit(d);
            return None;
        }
        let rec = self.ast.types.as_record(traits)?;
        let def = self.ast.definition(rec)?;
        let pt = self.intern("promise_type");
        let found = self.lookup_in_class(def, pt, LookupKind::Ordinary);
        let Some(&p) = found.decls.iter().find(|&&d| crate::lookup::is_type_kind(self.kind_of(d)))
        else {
            let std = self.ast.types.nns(Nns::Namespace { prefix: None, ns });
            let qual = self.ast.types.elaborated(TagKeyword::None, Some(std), traits);
            let d = self
                .diag(DiagId::sema_implied_std_coroutine_traits_promise_type_not_found, floc)
                .arg(self.ty_arg(qual));
            self.emit(d);
            return None;
        };
        let promise_ty = self.type_of_type_decl(p);
        if self.is_error_type(promise_ty) {
            // The lookup already reported why (e.g. no member
            // `promise_type` in the return type); clang says nothing more.
            return None;
        }
        let std = self.ast.types.nns(Nns::Namespace { prefix: None, ns });
        let nns =
            self.ast.types.nns(Nns::Type { prefix: Some(std), ty: traits, template_kw: false });
        let elab = self.ast.types.elaborated(TagKeyword::None, Some(nns), promise_ty);
        if self.ast.types.as_record(promise_ty).is_none() {
            let d = self
                .diag(DiagId::sema_implied_std_coroutine_traits_promise_type_not_class, floc)
                .arg(self.ty_arg(elab));
            self.emit(d);
            return None;
        }
        if !self.require_complete_silent(promise_ty, floc) {
            let d = self
                .diag(DiagId::sema_coroutine_promise_type_incomplete, floc)
                .arg(self.ty_arg(elab));
            self.emit(d);
            return None;
        }
        Some(promise_ty)
    }

    /// clang `buildCoroutinePromise`: `__promise`, constructed from the
    /// lvalues of the object and the parameter copies when that works,
    /// else default-initialized ([dcl.fct.def.coroutine]/5).
    fn build_promise(&mut self, f: DeclId, kw_loc: Loc) -> Option<DeclId> {
        let ty = self.lookup_promise_type(f, kw_loc)?;
        let floc = self.ast.decls.get(f).loc();
        let mut args = Vec::new();
        if self.method_this_type(f).is_some() && !self.is_lambda_call_op(f) {
            let this = self.act_on_this(floc);
            args.push(self.act_on_unary(UnaryOp::Deref, this, floc));
        }
        for p in self.param_decls(f) {
            if let Some(c) = self.param_copy(p) {
                let cty = self.ast.decls.get(c).ty().unwrap_or(QualType::ERROR);
                let nr = self.non_reference(cty);
                args.push(self.builder().decl_ref(
                    c,
                    nr,
                    floc,
                    None,
                    kiln_ast::expr::NonOdrUse::None,
                    kiln_ast::expr::ExprFlags::NONE,
                ));
            }
        }
        if !args.is_empty() {
            let v = self.coro_var("__promise", ty, floc, InitStyle::Call);
            let ((), stash) =
                self.capturing(|s| s.act_on_init(v, Initializer::Paren(args, floc, floc)));
            let failed =
                self.stash_has_error(&stash) || self.ast.decls.get(v).has(DeclFlags::INVALID);
            if !failed {
                self.release(stash, Vec::new());
                return Some(v);
            }
        }
        let v = self.coro_var("__promise", ty, floc, InitStyle::Call);
        self.act_on_uninit(v);
        Some(v)
    }

    /// The type a type declaration declares.
    fn type_of_type_decl(&mut self, d: DeclId) -> QualType {
        match self.kind_of(d) {
            DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::TemplateTypeParm => {
                self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR)
            }
            _ => self.tag_type(d),
        }
    }

    // -- the awaiter protocol ------------------------------------------------------

    /// `base.NAME(args)` (clang `buildMemberCall`).
    fn coro_member_call(&mut self, base: ExprId, loc: Loc, name: &str, args: &[ExprId]) -> ExprId {
        let sym = self.intern(name);
        let m = self.act_on_member(base, sym, loc, loc, false);
        if self.is_err_expr(m) {
            return m;
        }
        self.act_on_call(m, args, loc, loc)
    }

    /// `__promise.NAME(args)` (clang `buildPromiseCall`).
    fn promise_call(&mut self, promise: DeclId, loc: Loc, name: &str, args: &[ExprId]) -> ExprId {
        let r = self.build_decl_ref(promise, loc);
        self.coro_member_call(r, loc, name, args)
    }

    /// The definition of the promise's class.
    fn promise_record(&self, promise: DeclId) -> Option<DeclId> {
        let ty = self.ast.decls.get(promise).ty()?;
        let r = self.ast.types.as_record(ty)?;
        self.ast.definition(r)
    }

    /// Whether the promise class has a member `name` (clang
    /// `lookupMember`).
    fn promise_has_member(&self, promise: DeclId, name: &str) -> Option<DeclId> {
        let rec = self.promise_record(promise)?;
        let sym = self.intern(name);
        self.lookup_in_class(rec, sym, LookupKind::Ordinary).decls.first().copied()
    }

    /// `operator co_await(e)` if one applies, else `e` ([expr.await]/3.3;
    /// clang `buildOperatorCoawaitCall`).
    fn operator_coawait(&mut self, e: ExprId, loc: Loc) -> ExprId {
        match self.try_overloaded_operator(
            OperForm::Unary(UnaryOp::Coawait),
            &[e],
            &[],
            loc,
            Loc::INVALID,
        ) {
            OperResult::Done(x) => x,
            OperResult::Converted(v) => v.first().copied().unwrap_or(e),
            OperResult::Builtin | OperResult::BuiltinNotes(_) => e,
        }
    }

    /// `std::coroutine_handle<P>::from_address(__builtin_coro_frame())`
    /// (clang `buildCoroutineHandle`).
    fn coroutine_handle(&mut self, promise_ty: QualType, loc: Loc) -> Option<ExprId> {
        let ty = if let Some(t) = self.coro_state().and_then(|c| c.handle_ty) {
            t
        } else {
            let (_, tmpl) = self.std_coro_template(
                "coroutine_handle",
                loc,
                DiagId::sema_malformed_std_coroutine_handle,
            )?;
            let name = self.template_name_for(tmpl);
            let Ok(TemplateIdRef::Type(t)) =
                self.act_template_id(name, &[TemplateArg::ty(promise_ty)], loc, loc)
            else {
                return None;
            };
            if !self.require_complete_silent(t, loc) {
                let d = self
                    .diag(DiagId::sema_coroutine_type_missing_specialization, loc)
                    .arg(self.ty_arg(t));
                self.emit(d);
                return None;
            }
            if let Some(c) = self.coro_state_mut() {
                c.handle_ty = Some(t);
            }
            t
        };
        let rec = self.ast.types.as_record(ty)?;
        let def = self.ast.definition(rec)?;
        let fa = self.intern("from_address");
        if self.lookup_in_class(def, fa, LookupKind::Ordinary).decls.is_empty() {
            let d =
                self.diag(DiagId::sema_coroutine_handle_missing_member, loc).arg("from_address");
            self.emit(d);
            return None;
        }
        let (ns, _) = self.std_lookup("coroutine_handle")?;
        let std = self.ast.types.nns(Nns::Namespace { prefix: None, ns });
        let nns = self.ast.types.nns(Nns::Type { prefix: Some(std), ty, template_kw: false });
        let callee = self.act_on_id_expression(fa, loc, Some(Qualifier { ctx: def, nns, loc }));
        if self.is_err_expr(callee) {
            return None;
        }
        let frame = self.coro_builtin_call("__builtin_coro_frame", &[], loc);
        let h = self.act_on_call(callee, &[frame], loc, loc);
        (!self.is_err_expr(h)).then_some(h)
    }

    /// A call of the builtin `name`.
    fn coro_builtin_call(&mut self, name: &str, args: &[ExprId], loc: Loc) -> ExprId {
        let sym = self.intern(name);
        let f = self.act_on_id_expression(sym, loc, None);
        self.act_on_call(f, args, loc, loc)
    }

    /// The function a call calls, if it names one.
    fn coro_callee(&self, call: ExprId) -> Option<DeclId> {
        let mut x = call;
        loop {
            match self.ast.exprs.get(x).kind() {
                ExprKind::ExprWithCleanups
                | ExprKind::ImplicitCastExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::MaterializeTemporaryExpr => {
                    x = self.ast.exprs.children(x, &self.ast.lists)[0];
                }
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr => {
                    return self.callee_decl(x);
                }
                _ => return None,
            }
        }
    }

    /// `note_coroutine_promise_call_implicitly_required` for `call`.
    fn note_implicit_call(&self, call: ExprId, loc: Loc) -> Option<Diag> {
        let f = self.coro_callee(call)?;
        Some(
            self.diag(DiagId::note_coroutine_promise_call_implicitly_required, loc)
                .arg(format!("'{}'", self.decl_name(f))),
        )
    }

    /// The `await_ready`, `await_suspend` and `await_resume` calls on the
    /// awaiter `common` ([expr.await]/3.6-3.8; clang `buildCoawaitCalls`).
    fn coawait_calls(&mut self, promise: DeclId, loc: Loc, common: ExprId) -> Option<AwaitCalls> {
        let opaque = self.builder().opaque_value_lvalue(common);
        let mut ok = true;
        // await-ready, contextually converted to bool, is its own
        // full-expression.
        let saved = std::mem::take(&mut self.cleanup);
        let call = self.coro_member_call(opaque, loc, "await_ready", &[]);
        if self.is_err_expr(call) {
            self.cleanup = saved;
            return None;
        }
        let ready = match conv::contextually_convert_to_bool(self, call) {
            Ok(c) => self.finish_cleanups(c),
            Err(err) => {
                let mut d = err.diag_at(self, self.eloc(call));
                if let Some(f) = self.coro_callee(call) {
                    let n = self.diag(
                        DiagId::note_await_ready_no_bool_conversion,
                        self.ast.decls.get(f).loc(),
                    );
                    d = d.with_note(n);
                }
                if let Some(n) = self.note_implicit_call(call, loc) {
                    d = d.with_note(n);
                }
                self.emit(d);
                ok = false;
                call
            }
        };
        self.cleanup = saved;
        let promise_ty = self.ast.decls.get(promise).ty().unwrap_or(QualType::ERROR);
        let h = self.coroutine_handle(promise_ty, loc)?;
        let saved = std::mem::take(&mut self.cleanup);
        let call = self.coro_member_call(opaque, loc, "await_suspend", &[h]);
        if self.is_err_expr(call) {
            self.cleanup = saved;
            return None;
        }
        let callee = self.coro_callee(call);
        let rt = callee.map_or_else(|| self.ety(call), |f| self.written_return(f));
        let mut kind = SuspendKind::Void;
        let mut suspend = None;
        if !self.ast.types.is_reference(rt) && self.ast.types.as_record(rt).is_some() {
            // A `coroutine_handle` resumed after suspending (symmetric
            // transfer): `await_suspend(h).address()`.
            let addr = self.coro_member_call(call, loc, "address", &[]);
            if !self.is_err_expr(addr) {
                suspend = Some(self.finish_cleanups(addr));
                kind = SuspendKind::Handle;
            }
        }
        if suspend.is_none() {
            let urt = self.ast.types.unqualified(rt);
            let is_bool = self.ast.types.same_type(urt, QualType::BOOL);
            if self.ast.types.is_reference(rt) || (!is_bool && !self.ast.types.is_void(rt)) {
                let at = callee.map_or(loc, |f| self.ast.decls.get(f).loc());
                let mut d = self
                    .diag(DiagId::sema_await_suspend_invalid_return_type, at)
                    .arg(self.ty_arg(rt));
                if let Some(n) = self.note_implicit_call(call, loc) {
                    d = d.with_note(n);
                }
                self.emit(d);
                ok = false;
            } else {
                kind = if is_bool { SuspendKind::Bool } else { SuspendKind::Void };
            }
            suspend = Some(self.finish_cleanups(call));
        }
        self.cleanup = saved;
        let resume = self.coro_member_call(opaque, loc, "await_resume", &[]);
        if self.is_err_expr(resume) {
            ok = false;
        }
        // The awaiter is destroyed at the end of the enclosing
        // full-expression.
        self.cleanup.needs = true;
        ok.then_some(AwaitCalls { opaque, ready, suspend: suspend?, resume, kind })
    }

    /// clang `BuildResolvedCoawaitExpr`: `operand` as written, `awaiter`
    /// the result of `operator co_await`.
    fn build_resolved_coawait(
        &mut self,
        promise: DeclId,
        loc: Loc,
        operand: ExprId,
        awaiter: ExprId,
        implicit: bool,
        yield_: bool,
    ) -> Option<ExprId> {
        let common = self.materialize_awaiter(awaiter);
        let call_loc = if yield_ { loc } else { self.eloc(common) };
        let calls = self.coawait_calls(promise, call_loc, common)?;
        let kind = if yield_ { ExprKind::CoyieldExpr } else { ExprKind::CoawaitExpr };
        let parts = CoawaitParts {
            operand,
            common,
            ready: calls.ready,
            suspend: calls.suspend,
            resume: calls.resume,
            opaque: calls.opaque,
        };
        Some(self.builder().coawait_expr(kind, parts, implicit, calls.kind, loc))
    }

    /// A prvalue awaiter is materialized (an lvalue used several times).
    fn materialize_awaiter(&mut self, e: ExprId) -> ExprId {
        if self.ast.exprs.get(e).value_category() == ValueCategory::PRValue {
            let ty = self.ety(e);
            self.materialize_temporary(e, ty, true)
        } else {
            e
        }
    }

    /// `co_await e` in the coroutine with `promise` (clang
    /// `BuildUnresolvedCoawaitExpr`).
    fn build_unresolved_coawait(&mut self, promise: DeclId, loc: Loc, e: ExprId) -> Option<ExprId> {
        let mut x = e;
        if self.promise_has_member(promise, "await_transform").is_some() {
            let t = self.promise_call(promise, loc, "await_transform", &[e]);
            if self.is_err_expr(t) {
                self.error(
                    DiagId::note_coroutine_promise_implicit_await_transform_required_here,
                    loc,
                );
                return None;
            }
            x = t;
        }
        let awaiter = self.operator_coawait(x, loc);
        if self.is_err_expr(awaiter) {
            return None;
        }
        self.build_resolved_coawait(promise, loc, e, awaiter, false, false)
    }

    // -- the body start ------------------------------------------------------------

    /// clang `ActOnCoroutineBodyStart`: the promise, then the initial and
    /// final suspend points (at the function's location).
    #[allow(clippy::option_option, reason = "invalid context vs. no promise")]
    fn coroutine_body_start(&mut self, loc: Loc, kw: &'static str) -> Option<Option<DeclId>> {
        self.coro_isolated(|s| {
            let valid = s.valid_coroutine_context(loc, kw).is_some();
            if !valid {
                return None;
            }
            let promise = s.check_coroutine_context(loc, kw, false);
            if s.in_template() {
                return Some(None);
            }
            let promise = promise?;
            if s.coro_state().is_some_and(|c| c.suspends.is_none()) {
                let built = s.build_suspends(promise);
                if let Some(c) = s.coro_state_mut() {
                    c.suspends = Some(built);
                }
            }
            Some(Some(promise))
        })
    }

    /// The initial and final suspend points, and the final one's
    /// non-throwing check ([dcl.fct.def.coroutine]/5, /15).
    fn build_suspends(&mut self, promise: DeclId) -> Option<(ExprId, ExprId)> {
        let f = self.func.as_ref()?.decl;
        let floc = self.ast.decls.get(f).loc();
        let init = self.build_suspend(promise, "initial_suspend", 0, floc)?;
        let fin = self.build_suspend(promise, "final_suspend", 1, floc)?;
        if !self.check_final_suspend_nothrow(fin, floc) {
            return None;
        }
        Some((init, fin))
    }

    fn build_suspend(
        &mut self,
        promise: DeclId,
        name: &str,
        which: u8,
        floc: Loc,
    ) -> Option<ExprId> {
        let (r, stash) = self.capturing(|s| {
            let call = s.promise_call(promise, floc, name, &[]);
            if s.is_err_expr(call) {
                return None;
            }
            let aw = s.operator_coawait(call, floc);
            if s.is_err_expr(aw) {
                return None;
            }
            let e = s.build_resolved_coawait(promise, floc, call, aw, true, false)?;
            Some(s.act_on_full_expr(e))
        });
        if let Some(e) = r {
            self.release(stash, Vec::new());
            return Some(e);
        }
        let mut notes = vec![
            self.diag(DiagId::note_coroutine_promise_suspend_implicitly_required, floc).arg(which),
        ];
        if let Some(n) = self.note_coroutine_here() {
            notes.push(n);
        }
        self.release(stash, notes);
        None
    }

    /// clang `checkFinalSuspendNoThrow`: the functions the final suspend
    /// calls (not their arguments) and the destructors of their results
    /// are non-throwing.
    fn check_final_suspend_nothrow(&mut self, e: ExprId, floc: Loc) -> bool {
        let mut throwing: Vec<DeclId> = Vec::new();
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let k = self.ast.exprs.get(x).kind();
            match k {
                ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                    let ctor = self.ast.exprs.decl(x);
                    self.check_decl_nothrow(ctor, floc, &mut throwing);
                    if let Some(class) = self.ast.decls.get(ctor).ctx()
                        && let Some(d) = self.ast.destructor(class)
                    {
                        self.check_decl_nothrow(d, floc, &mut throwing);
                    }
                }
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr => {
                    if self.ast.exprs.get(x).is_type_dependent() {
                        continue;
                    }
                    if let Some(f) = self.callee_decl(x)
                        && self.ast.decls.builtin_id(f)
                            != Some(kiln_ast::decl::builtin::CORO_RESUME)
                    {
                        self.check_decl_nothrow(f, floc, &mut throwing);
                    }
                    let rt = self.ety(x);
                    if let Some(r) = self.ast.types.as_record(rt)
                        && let Some(def) = self.ast.definition(r)
                        && !self.trivially_destructible_class(def)
                        && let Some(d) = self.ast.destructor(def)
                    {
                        self.check_decl_nothrow(d, floc, &mut throwing);
                    }
                }
                _ => self.ast.exprs.children_into(x, &self.ast.lists, &mut stack),
            }
        }
        if throwing.is_empty() {
            return true;
        }
        let mut ends: Vec<Loc> = throwing.iter().map(|&d| self.decl_end_loc(d)).collect();
        ends.sort();
        let mut d = self.diag(DiagId::sema_coroutine_promise_final_suspend_requires_nothrow, floc);
        for l in ends {
            d = d.with_note(self.diag(DiagId::note_coroutine_function_declare_noexcept, l));
        }
        self.emit(d);
        false
    }

    fn check_decl_nothrow(&mut self, f: DeclId, at: Loc, out: &mut Vec<DeclId>) {
        if out.contains(&f) || !self.ast.decls.get(f).kind().is_function() {
            return;
        }
        self.resolve_exception_spec(f, at);
        if !self.decl_is_nothrow(f) {
            out.push(f);
        }
    }

    /// Whether class `c` has a trivial destructor.
    fn trivially_destructible_class(&self, c: DeclId) -> bool {
        self.ast.destructor(c).is_none_or(|d| self.fn_spec(d).is_trivial())
    }

    /// clang's `Decl::getEndLoc` approximated: the `}` of a definition,
    /// else the declarator's last token for a member declaration (its
    /// pattern's, for an instantiated member; `ColdDeclData::
    /// declarator_end`), else the name.
    fn decl_end_loc(&self, d: DeclId) -> Loc {
        if let Some(def) = self.ast.definition(d)
            && let Some(b) = self.ast.decls.function(def).and_then(|x| x.body)
            && self.ast.stmts.get(b).kind() == kiln_ast::stmt::StmtKind::CompoundStmt
        {
            return self.ast.stmts.rbrace(b);
        }
        let ends = &self.ast.decls.cold.declarator_end;
        if let Some(&l) =
            ends.get(d).or_else(|| self.ast.instantiated_from(d).and_then(|p| ends.get(p)))
        {
            return l;
        }
        self.ast.decls.get(d).loc()
    }

    // -- the actions ---------------------------------------------------------------

    /// `co_await e` ([expr.await]).
    pub fn act_on_coawait(&mut self, e: ExprId, loc: Loc) -> ExprId {
        if !self.check_suspension_context(loc, "co_await") {
            return self.recover_err(&[e], loc);
        }
        self.build_coawait(e, loc)
    }

    /// `co_await e` once the suspension context is checked (also the
    /// instantiation of a `DependentCoawaitExpr`).
    pub(crate) fn build_coawait(&mut self, e: ExprId, loc: Loc) -> ExprId {
        let Some(promise) = self.coroutine_body_start(loc, "co_await") else {
            return self.recover_err(&[e], loc);
        };
        let Some(promise) = promise else {
            return self.builder().dependent_coawait(e, false, loc);
        };
        if self.is_err_expr(e) {
            return self.recover_err(&[e], loc);
        }
        match self.build_unresolved_coawait(promise, loc, e) {
            Some(x) => x,
            None => self.recover_err(&[e], loc),
        }
    }

    /// `co_yield e` ([expr.yield]): `co_await __promise.yield_value(e)`.
    pub fn act_on_coyield(&mut self, e: ExprId, loc: Loc) -> ExprId {
        if !self.check_suspension_context(loc, "co_yield") {
            let _ = self.take_braced_arg(e);
            return self.recover_err(&[], loc);
        }
        self.build_coyield(e, loc)
    }

    pub(crate) fn build_coyield(&mut self, e: ExprId, loc: Loc) -> ExprId {
        let Some(promise) = self.coroutine_body_start(loc, "co_yield") else {
            let _ = self.take_braced_arg(e);
            return self.recover_err(&[], loc);
        };
        let Some(promise) = promise else {
            let _ = self.take_braced_arg(e);
            return self.builder().dependent_coawait(e, true, loc);
        };
        if self.is_err_expr(e) {
            return self.recover_err(&[e], loc);
        }
        let aw = self.promise_call(promise, loc, "yield_value", &[e]);
        if self.is_err_expr(aw) {
            return aw;
        }
        let awaiter = self.operator_coawait(aw, loc);
        if self.is_err_expr(awaiter) {
            return awaiter;
        }
        match self.build_resolved_coawait(promise, loc, e, awaiter, false, true) {
            Some(x) => x,
            None => self.recover_err(&[], loc),
        }
    }

    /// `co_return e;` ([stmt.return.coroutine]).
    pub fn act_on_coreturn(&mut self, e: Option<ExprId>, loc: Loc) -> StmtId {
        self.build_coreturn(e, loc)
    }

    pub(crate) fn build_coreturn(&mut self, e: Option<ExprId>, loc: Loc) -> StmtId {
        let Some(promise) = self.coroutine_body_start(loc, "co_return") else {
            if let Some(e) = e {
                let _ = self.take_braced_arg(e);
            }
            return self.builder().coreturn_stmt(None, None, false, loc);
        };
        let Some(promise) = promise else {
            if let Some(x) = e {
                let _ = self.take_braced_arg(x);
            }
            return self.builder().coreturn_stmt(e, None, false, loc);
        };
        let (operand, call) = self.coreturn_call(promise, e, loc);
        self.builder().coreturn_stmt(operand, call, false, loc)
    }

    /// The operand and promise call of `co_return e;` (clang
    /// `BuildCoreturnStmt`): `return_value(e)` for a braced list or an
    /// operand not of type `void`, else `e;` and `return_void()`.
    fn coreturn_call(
        &mut self,
        promise: DeclId,
        e: Option<ExprId>,
        loc: Loc,
    ) -> (Option<ExprId>, Option<ExprId>) {
        let braced = e.is_some_and(|x| self.is_braced_arg(x));
        if let Some(x) = e
            && self.is_err_expr(x)
        {
            return (e, None);
        }
        let (operand, call) = match e {
            Some(x) if braced || !self.ast.types.is_void(self.ety(x)) => {
                let m = if braced { x } else { self.implicit_move(x) };
                (Some(x), self.promise_call(promise, loc, "return_value", &[m]))
            }
            _ => {
                let operand = e.map(|x| self.act_on_full_expr(x));
                (operand, self.promise_call(promise, loc, "return_void", &[]))
            }
        };
        if self.is_err_expr(call) {
            return (operand, None);
        }
        (operand, Some(self.act_on_full_expr(call)))
    }

    /// A `return` statement in the body (diagnosed at its end in a
    /// coroutine, [stmt.return.coroutine]/1).
    pub(crate) fn note_return_stmt(&mut self, loc: Loc) {
        if let Some(st) = self.func.as_mut()
            && st.first_return.is_none()
        {
            st.first_return = Some(loc);
        }
    }

    // -- the body end --------------------------------------------------------------

    /// clang `CheckCompletedCoroutineBody`: wraps the body of a
    /// coroutine in a `CoroutineBodyStmt` ([dcl.fct.def.coroutine]/5).
    /// `body` unchanged for a template pattern or after an error.
    pub(crate) fn complete_coroutine_body(&mut self, body: StmtId) -> StmtId {
        let Some(st) = self.func.as_ref() else { return body };
        let f = st.decl;
        let Some(coro) = st.coro.as_deref() else { return body };
        let first_return = st.first_return;
        if coro.first.is_none() {
            return body;
        }
        if let Some(r) = first_return {
            let mut d = self.diag(DiagId::sema_return_in_coroutine, r);
            if let Some(n) = self.note_coroutine_here() {
                d = d.with_note(n);
            }
            self.emit(d);
        }
        if self.in_template() {
            return body;
        }
        self.check_coroutine_post_params(f);
        let Some(coro) = self.coro_state().cloned() else { return body };
        let Some(promise) = coro.promise else {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
            return body;
        };
        let Some(Some((init_suspend, final_suspend))) = coro.suspends else {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
            return body;
        };
        let built = self.coro_isolated(|s| {
            s.build_coroutine_stmts(f, promise, &coro, body, init_suspend, final_suspend)
        });
        if let Some(s) = built {
            s
        } else {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
            body
        }
    }

    #[allow(clippy::too_many_lines, reason = "one step per [dcl.fct.def.coroutine]/5 statement")]
    fn build_coroutine_stmts(
        &mut self,
        f: DeclId,
        promise: DeclId,
        coro: &CoroState,
        body: StmtId,
        init_suspend: ExprId,
        final_suspend: ExprId,
    ) -> Option<StmtId> {
        let floc = self.ast.decls.get(f).loc();
        let rec = self.promise_record(promise)?;
        let promise_stmt = self.act_on_decl_stmt(&[promise], floc);
        // makeReturnObject.
        let gro = self.promise_call(promise, floc, "get_return_object", &[]);
        if self.is_err_expr(gro) {
            return None;
        }
        // makeOnException.
        let on_exception = if self.promise_has_member(promise, "unhandled_exception").is_none() {
            if self.exceptions {
                let rty = self.tag_type(rec);
                let d = self
                    .diag(DiagId::sema_coroutine_promise_unhandled_exception_required, floc)
                    .arg(self.ty_arg(rty))
                    .with_note(
                        self.diag(DiagId::note_entity_defined_here, self.ast.decls.get(rec).loc())
                            .arg(self.ty_arg(rty)),
                    );
                self.emit(d);
                return None;
            }
            None
        } else if self.exceptions {
            let c = self.promise_call(promise, floc, "unhandled_exception", &[]);
            if self.is_err_expr(c) {
                return None;
            }
            let c = self.act_on_full_expr(c);
            Some(self.builder().expr_stmt(c))
        } else {
            None
        };
        // makeOnFallthrough.
        let rv = self.promise_has_member(promise, "return_void");
        let rval = self.promise_has_member(promise, "return_value");
        let on_fallthrough = match (rv, rval) {
            (Some(a), Some(b)) => {
                let rty = self.tag_type(rec);
                let d = self
                    .diag(DiagId::sema_coroutine_promise_incompatible_return_functions, floc)
                    .arg(self.ty_arg(rty))
                    .with_note(
                        self.diag(
                            DiagId::note_member_first_declared_here,
                            self.ast.decls.get(a).loc(),
                        )
                        .arg("'return_void'"),
                    )
                    .with_note(
                        self.diag(
                            DiagId::note_member_first_declared_here,
                            self.ast.decls.get(b).loc(),
                        )
                        .arg("'return_value'"),
                    );
                self.emit(d);
                return None;
            }
            (None, None) => {
                let l = self.ast.decls.get(rec).loc();
                Some(self.builder().null_stmt(l))
            }
            (Some(_), None) => {
                let call = self.promise_call(promise, floc, "return_void", &[]);
                if self.is_err_expr(call) {
                    return None;
                }
                let call = self.act_on_full_expr(call);
                Some(self.builder().coreturn_stmt(None, Some(call), true, floc))
            }
            (None, Some(_)) => None,
        };
        // makeGroDeclAndReturnStmt.
        let ret = self.written_return(f);
        let gro_ty = self.ety(gro);
        let mut result_decl = None;
        let mut return_stmt = None;
        let return_value;
        if self.ast.types.is_void(ret) {
            return_value = self.act_on_full_expr(gro);
        } else if self.ast.types.is_void(gro_ty) {
            let _ = self.convert_for_init(gro, ret, ConvCtx::Return);
            self.note_gro_declared(gro);
            return None;
        } else if self.ast.types.same_type(gro_ty, ret) {
            let v = self.convert_for_init(gro, ret, ConvCtx::Return);
            if self.is_err_expr(v) {
                self.note_gro_declared(gro);
                return None;
            }
            let v = self.act_on_full_expr(v);
            return_value = v;
            return_stmt = Some(self.builder().return_stmt(Some(v), None, floc));
        } else {
            let v = self.coro_var("__coro_gro", gro_ty, floc, InitStyle::C);
            let ((), stash) = self.capturing(|s| s.act_on_init(v, Initializer::Expr(gro)));
            let failed = self.stash_has_error(&stash);
            self.release(stash, Vec::new());
            if failed || self.ast.decls.get(v).has(DeclFlags::INVALID) {
                return None;
            }
            // The `DeclStmt` marks the variable as a local of the body.
            let _ = self.act_on_decl_stmt(&[v], floc);
            let r = self.build_decl_ref(v, floc);
            let r = self.implicit_move(r);
            let (conv, stash) = self.capturing(|s| {
                let c = s.convert_for_init(r, ret, ConvCtx::Return);
                if s.is_err_expr(c) { None } else { Some(s.act_on_full_expr(c)) }
            });
            let Some(conv) = conv else {
                let mut notes = Vec::new();
                if let Some(m) = self.coro_callee(gro) {
                    notes.push(
                        self.diag(DiagId::note_member_declared_named, self.ast.decls.get(m).loc())
                            .arg(self.decl_name(m)),
                    );
                }
                notes.extend(self.note_coroutine_here());
                self.release(stash, notes);
                return None;
            };
            self.release(stash, Vec::new());
            return_value = gro;
            result_decl = Some(v);
            return_stmt = Some(self.builder().return_stmt(Some(conv), None, floc));
        }
        // makeReturnOnAllocFailure.
        let alloc_fail =
            self.promise_has_member(promise, "get_return_object_on_allocation_failure");
        let mut return_on_alloc_failure = None;
        if let Some(m) = alloc_fail {
            let is_static = self.kind_of(m) == DeclKind::CXXMethod
                && self.fn_spec(m).storage() == FnStorage::Static;
            if !is_static {
                let rty = self.tag_type(rec);
                let mut d = self
                    .diag(
                        DiagId::sema_coroutine_promise_get_return_object_on_allocation_failure,
                        self.ast.decls.get(m).loc(),
                    )
                    .arg(self.ty_arg(rty));
                if let Some(n) = self.note_coroutine_here() {
                    d = d.with_note(n);
                }
                self.emit(d);
                return None;
            }
            let callee = self.build_decl_ref(m, floc);
            let call = self.act_on_call(callee, &[], floc, floc);
            if self.is_err_expr(call) {
                return None;
            }
            let v = if self.ast.types.is_void(ret) {
                call
            } else {
                self.convert_for_init(call, ret, ConvCtx::Return)
            };
            if self.is_err_expr(v) {
                return None;
            }
            let v = self.act_on_full_expr(v);
            return_on_alloc_failure = Some(self.builder().return_stmt(Some(v), None, floc));
        }
        // makeNewAndDeleteExpr.
        let (allocate, deallocate) =
            self.coro_new_delete(f, promise, floc, alloc_fail.is_some())?;
        let moves: Vec<StmtId> = coro.moves.iter().map(|m| m.1).collect();
        let param_moves = self.ast.lists.push(&moves);
        let data = CoroutineBodyData {
            body,
            promise,
            promise_stmt,
            init_suspend,
            final_suspend,
            on_exception,
            on_fallthrough,
            allocate,
            deallocate,
            return_value,
            result_decl,
            return_stmt,
            return_on_alloc_failure,
            param_moves,
        };
        Some(self.builder().coroutine_body_stmt(data, floc))
    }

    /// clang `noteMemberDeclaredHere` after a failed initialization from
    /// `get_return_object()`.
    fn note_gro_declared(&mut self, gro: ExprId) {
        let mut notes = Vec::new();
        if let Some(m) = self.coro_callee(gro) {
            notes.push(
                self.diag(DiagId::note_member_declared_named, self.ast.decls.get(m).loc())
                    .arg(self.decl_name(m)),
            );
        }
        notes.extend(self.note_coroutine_here());
        for n in notes {
            self.emit(n);
        }
    }

    /// One `operator new` lookup for the coroutine frame (clang
    /// `LookupAllocationFunction`): the promise's scope or the global
    /// one, with `(size, [align_val_t,] place...)`. Returns the function
    /// and whether it takes the alignment.
    fn coro_find_new(
        &mut self,
        promise_ty: QualType,
        global: bool,
        place: &[ExprId],
        pass_align: bool,
        loc: Loc,
    ) -> (Option<DeclId>, bool) {
        use kiln_ast::names::OverloadedOperator as OO;
        let fns = self.alloc_fns(promise_ty, OO::New, global, loc);
        // Templates join the overload set as in a new-expression
        // (`template <class A> operator new(size_t, allocator_arg_t, A)`).
        let found = if fns.templates.is_empty() {
            self.find_operator_new(&fns.fns, place, pass_align)
        } else {
            let all: Vec<DeclId> = fns.fns.iter().chain(&fns.templates).copied().collect();
            self.find_operator_new_template(&all, place, pass_align, loc, &mut None)
        };
        match found {
            Some((n, a)) => (Some(n), a),
            None => (None, false),
        }
    }

    /// The frame's allocation and deallocation calls
    /// ([dcl.fct.def.coroutine]/9-12; clang `makeNewAndDeleteExpr`).
    #[allow(
        clippy::too_many_lines,
        reason = "the new/delete lookup cascade of [dcl.fct.def.coroutine]/9-12"
    )]
    fn coro_new_delete(
        &mut self,
        f: DeclId,
        promise: DeclId,
        loc: Loc,
        requires_nothrow: bool,
    ) -> Option<(ExprId, ExprId)> {
        use kiln_ast::names::OverloadedOperator as OO;
        self.declare_global_new_delete();
        let promise_ty = self.ast.decls.get(promise).ty().unwrap_or(QualType::ERROR);
        let rec = self.promise_record(promise)?;
        let new_name = DeclName::operator(OO::New);
        let contains_new =
            !self.lookup_in_class_dn(rec, new_name, LookupKind::Ordinary).decls.is_empty();
        let mut place: Vec<ExprId> = Vec::new();
        if contains_new {
            if self.method_this_type(f).is_some() && !self.is_lambda_call_op(f) {
                let this = self.act_on_this(loc);
                place.push(self.act_on_unary(UnaryOp::Deref, this, loc));
            }
            for p in self.param_decls(f) {
                let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                if self.ast.types.is_dependent(pty) {
                    continue;
                }
                let ploc = self.ast.decls.get(p).loc();
                let nr = self.non_reference(pty);
                place.push(self.builder().decl_ref(
                    p,
                    nr,
                    ploc,
                    None,
                    kiln_ast::expr::NonOdrUse::None,
                    kiln_ast::expr::ExprFlags::NONE,
                ));
            }
        }
        // clang `LookupAllocationFunction`: the promise's scope when it
        // declares `operator new`, else the global scope. Under
        // `-fcoro-aligned-allocation` (P2014R0 option 2) the
        // `(size, align_val_t, ...)` forms are preferred: aligned with
        // placement arguments, aligned, plain with placement arguments,
        // plain.
        let aligned = self.coro_aligned_allocation;
        let (mut new, mut passed) =
            self.coro_find_new(promise_ty, !contains_new, &place, aligned, loc);
        if contains_new && !place.is_empty() && (new.is_none() || (aligned && !passed)) {
            (new, passed) = self.coro_find_new(promise_ty, false, &[], aligned, loc);
        }
        let mut non_aligned_in_promise = false;
        if contains_new && aligned && (new.is_none() || !passed) {
            non_aligned_in_promise = new.is_some();
            (new, passed) = self.coro_find_new(promise_ty, false, &place, false, loc);
            if new.is_none() && !place.is_empty() {
                (new, passed) = self.coro_find_new(promise_ty, false, &[], false, loc);
            }
        }
        let is_global = new.is_some_and(|n| {
            self.ast.decls.get(n).ctx().is_none_or(|c| self.kind_of(c) != DeclKind::CXXRecord)
        });
        if requires_nothrow && (new.is_none() || is_global) {
            let Some((_, found)) = self.std_lookup("nothrow") else {
                self.error(DiagId::sema_implicit_coroutine_std_nothrow_type_not_found, loc);
                return None;
            };
            let nt = self.build_decl_ref(found[0], loc);
            place = vec![nt];
            (new, passed) = self.coro_find_new(promise_ty, true, &place, aligned, loc);
        }
        if non_aligned_in_promise && let Some(n) = new {
            let d = self
                .diag(DiagId::sema_non_aligned_allocation_function, self.ast.decls.get(n).loc())
                .arg(format!("'{}'", self.decl_name(f)));
            self.emit(d);
        }
        let callee_name = format!("'{}'", self.decl_name(f));
        let Some(new) = new else {
            if contains_new {
                let d = self
                    .diag(DiagId::sema_coroutine_unusable_new, loc)
                    .arg(self.ty_arg(promise_ty))
                    .arg(callee_name);
                self.emit(d);
            } else if requires_nothrow {
                let d = self
                    .diag(DiagId::sema_coroutine_unfound_nothrow_new, loc)
                    .arg(callee_name)
                    .arg(u8::from(aligned));
                self.emit(d);
            }
            return None;
        };
        if requires_nothrow {
            self.resolve_exception_spec(new, loc);
            if !self.decl_is_nothrow(new) {
                let n = format!("'{}'", self.decl_name(new));
                let d = self
                    .diag(
                        DiagId::sema_coroutine_promise_new_requires_nothrow,
                        self.ast.decls.get(new).loc(),
                    )
                    .arg(n.clone())
                    .with_note(
                        self.diag(DiagId::note_coroutine_promise_call_implicitly_required, loc)
                            .arg(n),
                    );
                self.emit(d);
                return None;
            }
        }
        // findDeleteForPromise: the class's, else the global one; the
        // sized form is preferred.
        let dels = self.alloc_fns(promise_ty, OO::Delete, false, loc);
        let mut del = self.usual_delete(&dels.fns, false, aligned, true, false);
        if del.is_none() && dels.in_class {
            let rty = self.tag_type(rec);
            self.no_suitable_delete(OO::Delete, rty, &dels.fns, loc);
            return None;
        }
        if del.is_none() {
            let g = self.alloc_fns(promise_ty, OO::Delete, true, loc);
            del = self.usual_delete(&g.fns, false, aligned, true, false);
        }
        let (del, _) = del?;
        self.mark_function_referenced(del, loc);
        let size = self.coro_builtin_call("__builtin_coro_size", &[], loc);
        // `static_cast<std::align_val_t>(__builtin_coro_align())`.
        let frame_align = |s: &mut Self| {
            let a = s.coro_builtin_call("__builtin_coro_align", &[], loc);
            let avt = s.global_ops.align_val_t.unwrap_or(QualType::ERROR);
            s.act_on_cast(CastStyle::Static, avt, a, loc, loc)
        };
        let new_ref = self.build_decl_ref(new, loc);
        let mut args = vec![size];
        if aligned && passed {
            args.push(frame_align(self));
        }
        let nparams = self.fn_params(new).map_or(0, |p| p.0.len());
        if nparams > args.len() {
            args.extend(place);
        }
        let alloc = self.act_on_call(new_ref, &args, loc, loc);
        if self.is_err_expr(alloc) {
            return None;
        }
        let alloc = self.act_on_full_expr(alloc);
        let frame = self.coro_builtin_call("__builtin_coro_frame", &[], loc);
        let free = self.coro_builtin_call("__builtin_coro_free", &[frame], loc);
        let del_ref = self.build_decl_ref(del, loc);
        let mut dargs = vec![free];
        let size_t = self.size_type();
        let second = self.fn_params(del).and_then(|(p, _)| p.get(1).copied());
        let sized = second.is_some_and(|t| {
            let u = self.ast.types.unqualified(t);
            self.ast.types.same_type(u, size_t)
        });
        if sized {
            dargs.push(self.coro_builtin_call("__builtin_coro_size", &[], loc));
        }
        // P2014R0: the alignment is passed when the selected function
        // takes it (clang never forces the aligned form).
        let next = self.fn_params(del).and_then(|(p, _)| p.get(dargs.len()).copied());
        if aligned
            && let (Some(t), Some(avt)) = (next, self.global_ops.align_val_t)
            && {
                let u = self.ast.types.unqualified(t);
                self.ast.types.same_type(u, avt)
            }
        {
            dargs.push(frame_align(self));
        }
        let dealloc = self.act_on_call(del_ref, &dargs, loc, loc);
        if self.is_err_expr(dealloc) {
            return None;
        }
        let dealloc = self.act_on_full_expr(dealloc);
        Some((alloc, dealloc))
    }
}
