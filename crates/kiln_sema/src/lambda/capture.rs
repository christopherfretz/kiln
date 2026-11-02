//! Lambda captures ([expr.prim.lambda.capture]; WP-075,
//! spec/sema-lambda.md §3).
//!
//! A name of a variable of an enclosing function, or `this`, used in a
//! lambda's compound-statement is captured by every lambda between the
//! use and the entity's home (clang `tryCaptureVariable` /
//! `CheckCXXThisCapture`). Explicit captures are processed at the
//! introducer; implicit ones at the odr-use: at once for a variable that
//! cannot be usable in constant expressions, else at the end of the
//! full-expression once the use is known not to be `non_odr_use_constant`.
//! The closure's members and their initializers are made at the lambda's
//! `}` ([`Sema::finish_captures`]).

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprDeps, UnaryOp};
use kiln_ast::ids::{ExprId, LambdaId};
use kiln_ast::lambda::{CaptureDefault, CaptureKind, LambdaCapture};
use kiln_ast::names::DeclName;
use kiln_ast::types::{AutoKeyword, Quals};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use super::{LambdaIntroducer, LambdaScope};
use crate::Sema;
use crate::class::InitEntity;
use crate::init::Initializer;
use crate::lookup::LookupKind;

/// How a variable is being captured (clang `TryCaptureKind`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum TryCapture {
    /// Through a capture-default.
    Implicit,
    /// A simple-capture `x` of the lambda being introduced.
    ExplicitByCopy,
    /// A simple-capture `&x` of the lambda being introduced.
    ExplicitByRef,
}

/// The outcome of [`Sema::try_capture_var`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Captured {
    /// The variable is not from outside the innermost lambda (or has no
    /// automatic storage duration): nothing to capture.
    NotNeeded,
    /// Captured by every lambda up to its home: the type of an
    /// id-expression naming it ([expr.prim.id.unqual]/3).
    Yes(QualType),
    /// It cannot be captured (diagnosed when building).
    Failed,
}

/// An init-capture whose variable is declared when the body opens.
#[derive(Copy, Clone, Debug)]
pub(crate) struct InitCapture {
    /// Its index among the lambda's captures.
    pub(crate) index: usize,
    pub(crate) name: Symbol,
    pub(crate) loc: Loc,
    pub(crate) ty: QualType,
    /// The converted initializer.
    pub(crate) init: ExprId,
}

impl Sema<'_, '_> {
    /// The lambda whose function call operator is `f`.
    pub(crate) fn lambda_of_call_op(&self, f: DeclId) -> Option<LambdaId> {
        self.ast.call_op_lambda(f)
    }

    /// The context enclosing lambda `l`'s closure type.
    fn lambda_parent(&self, l: LambdaId) -> Option<DeclId> {
        self.ast.decls.get(self.ast.exprs.lambdas.get(l).closure).ctx()
    }

    /// The open lambda `l`'s saved state.
    fn lambda_scope_mut(&mut self, l: LambdaId) -> Option<&mut LambdaScope> {
        self.lambdas.iter_mut().rev().find(|s| s.id == l)
    }

    /// The type of the member holding capture `i` of lambda `l`.
    fn capture_type(&self, l: LambdaId, i: usize) -> QualType {
        if let Some(s) = self.lambdas.iter().rev().find(|s| s.id == l)
            && let Some(&t) = s.cap_tys.get(i)
        {
            return t;
        }
        self.ast.exprs.lambdas.get(l).captures[i]
            .field
            .and_then(|f| self.ast.decls.get(f).ty())
            .unwrap_or(QualType::ERROR)
    }

    /// The lambda whose init-capture declares variable `d`.
    pub(crate) fn init_capture_owner(&self, d: DeclId) -> Option<(LambdaId, usize)> {
        let f = self.ast.decls.get(d).ctx()?;
        let l = self.lambda_of_call_op(f)?;
        let i = self
            .ast
            .exprs
            .lambdas
            .get(l)
            .captures
            .iter()
            .position(|c| c.init && c.var == Some(d))?;
        Some((l, i))
    }

    /// True if `d` is a variable or parameter with automatic storage
    /// duration ([basic.stc.auto]).
    pub(crate) fn has_local_storage(&self, d: DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::ParmVar => true,
            DeclKind::Var => {
                let Some(v) = self.ast.decls.var(d) else { return false };
                v.spec.tls() == kiln_ast::decl::TlsKind::None && !self.has_static_storage(d)
            }
            // A structured binding has its decomposition's storage
            // ([expr.prim.lambda.capture]/12 captures it as an entity).
            DeclKind::Binding => {
                self.ast.decls.binding_decomp(d).is_some_and(|v| self.has_local_storage(v))
            }
            _ => false,
        }
    }

    /// clang `VarDecl::mightBeUsableInConstantExpressions`: a reference,
    /// a `constexpr` variable or a `const` non-volatile integral or
    /// enumeration variable (not a parameter). Such a use is captured only
    /// once it is known to be an odr-use.
    pub(crate) fn might_be_constant(&self, d: DeclId) -> bool {
        if self.kind_of(d) != DeclKind::Var {
            return false;
        }
        let Some(t) = self.ast.decls.get(d).ty() else { return false };
        if self.ast.types.is_reference(t) {
            return true;
        }
        if self.ast.decls.var(d).is_some_and(|v| v.spec.is_constexpr()) {
            return true;
        }
        let q = self.ast.types.effective_quals(t);
        q.is_const() && !q.is_volatile() && self.is_integral_or_enum(t)
    }

    /// True if a use of variable `d` here may need a capture: it is a
    /// local of another function, or an init-capture.
    /// Whether context `c` is (lexically) nested in `outer`.
    fn is_nested_in(&self, c: DeclId, outer: DeclId) -> bool {
        let mut c = Some(c);
        for _ in 0..4096 {
            let Some(x) = c else { return false };
            if x == outer {
                return true;
            }
            c = self.ast.decls.lexical_ctx(x).or_else(|| self.ast.decls.get(x).ctx());
        }
        false
    }

    pub(crate) fn may_need_capture(&self, d: DeclId) -> bool {
        matches!(self.kind_of(d), DeclKind::Var | DeclKind::ParmVar | DeclKind::Binding)
            && (self.ast.decls.get(d).ctx() != Some(self.ctx)
                || self.init_capture_owner(d).is_some())
            && (self.has_local_storage(d) || self.init_capture_owner(d).is_some())
    }

    /// The lambdas between the current context and variable `d`'s home
    /// `home` that capture `d` anew, innermost first, and the capture
    /// they start from (an existing capture, or `d`'s own init-capture
    /// `owner`); `Err` with the outcome when no capture is possible.
    #[allow(clippy::type_complexity, reason = "a private helper's result")]
    fn capture_chain(
        &mut self,
        d: DeclId,
        loc: Loc,
        top: Option<LambdaId>,
        home: DeclId,
        owner: Option<(LambdaId, usize)>,
        build: bool,
    ) -> Result<(Vec<LambdaId>, Option<(LambdaId, usize)>), Captured> {
        let mut chain: Vec<LambdaId> = top.into_iter().collect();
        let mut start = None;
        let mut c = self.ctx;
        for _ in 0..4096 {
            if c == home {
                start = owner;
                break;
            }
            if let Some(l) = self.lambda_of_call_op(c) {
                let data = self.ast.exprs.lambdas.get(l);
                if let Some(i) = data.captures.iter().position(|x| x.var == Some(d)) {
                    start = Some((l, i));
                    break;
                }
                // A generic lambda's call operator being instantiated:
                // its closure is complete, and the pattern captured every
                // variable a specialization may odr-use.
                if !self.lambdas.iter().any(|s| s.id == l) {
                    return Err(Captured::NotNeeded);
                }
                if data.default == CaptureDefault::None {
                    if build {
                        self.diagnose_impcap(d, loc, l);
                    }
                    return Err(Captured::Failed);
                }
                chain.push(l);
                let Some(p) = self.lambda_parent(l) else { return Err(Captured::Failed) };
                c = p;
                continue;
            }
            if self.kind_of(c) == DeclKind::RequiresExprBody
                && let Some(p) = self.ast.decls.get(c).ctx()
            {
                c = p;
                continue;
            }
            // A reference from outside any function nested in the
            // variable's (a default argument, an instantiated exception
            // specification) has nothing to capture in.
            if !self.ast.decls.get(c).kind().is_function() || !self.is_nested_in(c, home) {
                return Err(Captured::NotNeeded);
            }
            if build {
                self.diagnose_enclosing_local(d, loc, home);
            }
            return Err(Captured::Failed);
        }
        Ok((chain, start))
    }

    /// clang `tryCaptureVariable`: captures variable `d`, named at `loc`,
    /// in every lambda between the current context and its home, the
    /// innermost as `kind` (`top` is the lambda being introduced for an
    /// explicit capture). Without `build` only computes the outcome.
    pub(crate) fn try_capture_var(
        &mut self,
        d: DeclId,
        loc: Loc,
        kind: TryCapture,
        top: Option<LambdaId>,
        ellipsis: Loc,
        build: bool,
    ) -> Captured {
        if !matches!(self.kind_of(d), DeclKind::Var | DeclKind::ParmVar | DeclKind::Binding) {
            return Captured::NotNeeded;
        }
        let owner = self.init_capture_owner(d);
        if owner.is_none() && !self.has_local_storage(d) {
            return Captured::NotNeeded;
        }
        let Some(home) = self.ast.decls.get(d).ctx() else { return Captured::NotNeeded };
        let (chain, start) = match self.capture_chain(d, loc, top, home, owner, build) {
            Ok(x) => x,
            Err(c) => return c,
        };
        if chain.is_empty() && start.is_none() {
            return Captured::NotNeeded;
        }
        let (mut cap, mut rf) = if let Some((l, i)) = start {
            let cap = self.capture_type(l, i);
            let data = self.ast.exprs.lambdas.get(l);
            let copy = data.captures[i].kind == CaptureKind::ByCopy;
            let konst = copy && self.lambda_capture_const(l);
            let mut rf = self.non_reference(cap);
            if konst {
                rf = self.ast.types.qualify(rf, Quals::CONST);
            }
            (cap, rf)
        } else {
            let t = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
            (t, self.non_reference(t))
        };
        for (j, &l) in chain.iter().enumerate().rev() {
            let explicit = j == 0 && top == Some(l);
            let data = self.ast.exprs.lambdas.get(l);
            let by_ref = if explicit {
                kind == TryCapture::ExplicitByRef
            } else {
                data.default == CaptureDefault::ByRef
            };
            let mutable = !self.lambda_capture_const(l);
            if by_ref {
                cap = self.ast.types.lvalue_ref_to(rf);
            } else {
                // [expr.prim.lambda.capture]/10: the member has the
                // referenced type of a reference to an object.
                if self.ast.types.is_reference(cap) {
                    let p = self.non_reference(cap);
                    if !self.ast.types.is_function(p) {
                        cap = p;
                    }
                }
                rf = self.non_reference(cap);
                if !mutable && !self.ast.types.is_reference(cap) {
                    rf = self.ast.types.qualify(rf, Quals::CONST);
                }
            }
            if build {
                let c = LambdaCapture {
                    kind: if by_ref { CaptureKind::ByRef } else { CaptureKind::ByCopy },
                    var: Some(d),
                    field: None,
                    loc,
                    ellipsis,
                    implicit: !explicit,
                    init: false,
                };
                self.add_capture(l, c, cap);
            }
        }
        Captured::Yes(rf)
    }

    /// Whether a capture by copy of lambda `l` is `const` in its body
    /// ([expr.prim.id.unqual]/3): with an explicit object parameter, when
    /// the parameter's (referenced) type is const-qualified; otherwise
    /// unless the lambda is `mutable` (clang
    /// `LambdaScopeInfo::lambdaCaptureShouldBeConst`).
    pub(crate) fn lambda_capture_const(&self, l: LambdaId) -> bool {
        let data = self.ast.exprs.lambdas.get(l);
        let obj = data.call_op.filter(|&f| self.has_explicit_object(f)).and_then(|f| {
            let x = self.ast.decls.function(f)?;
            self.ast.lists.iter::<DeclId>(x.params).next()
        });
        match obj.and_then(|p| self.ast.decls.get(p).ty()) {
            Some(t) => {
                let t = self
                    .ast
                    .types
                    .pointee(t)
                    .filter(|_| self.ast.types.is_reference(t))
                    .unwrap_or(t);
                self.ast.types.canonical(t).quals().contains(Quals::CONST)
            }
            None => !data.mutable,
        }
    }

    /// [expr.prim.lambda.closure]/5 (P0847) for a call at `loc` of `f`:
    /// [`Self::check_lambda_explicit_object_type`] for a specialization
    /// of a generic lambda's call operator (a non-template one was
    /// checked where it was declared). Returns false after an error.
    pub(crate) fn check_lambda_explicit_object_call(&mut self, f: DeclId, loc: Loc) -> bool {
        let Some(l) = self.ast.call_op_lambda(f) else { return true };
        let op = self.ast.exprs.lambdas.get(l).call_op;
        if op.is_some_and(|op| self.ast.first(op) == self.ast.first(f)) {
            return true;
        }
        self.check_lambda_explicit_object_type(f, loc)
    }

    /// [expr.prim.lambda.closure]/5 (P0847): the explicit object
    /// parameter of `f`, the call operator of a lambda with a
    /// lambda-capture, is of the closure type or of a class publicly and
    /// unambiguously derived from it (clang
    /// `DiagnoseInvalidExplicitObjectParameterInLambda`). Reports a
    /// violation at `loc` and returns false.
    pub(crate) fn check_lambda_explicit_object_type(&mut self, f: DeclId, loc: Loc) -> bool {
        if !self.has_explicit_object(f) {
            return true;
        }
        let Some(l) = self.ast.call_op_lambda(f) else { return true };
        let data = self.ast.exprs.lambdas.get(l);
        if data.default == CaptureDefault::None && data.captures.is_empty() {
            return true;
        }
        let closure = data.closure;
        let Some(p) =
            self.ast.decls.function(f).and_then(|x| self.ast.lists.iter::<DeclId>(x.params).next())
        else {
            return true;
        };
        let Some(t) = self.ast.decls.get(p).ty() else { return true };
        let t = self.ast.types.pointee(t).filter(|_| self.ast.types.is_reference(t)).unwrap_or(t);
        let t = QualType::unqual(self.ast.types.canonical(t).ty());
        if self.ast.types.is_dependent(t) || t == QualType::ERROR {
            return true;
        }
        let Some(r) = self.ast.types.as_record(t) else {
            return self.lambda_object_not_derived(t, loc);
        };
        if self.ast.first(r) == self.ast.first(closure) {
            return true;
        }
        let paths = crate::conv::bases::base_paths(self, r, closure);
        let Some(path) = paths.first() else {
            return self.lambda_object_not_derived(t, loc);
        };
        let closure_ty = QualType::unqual(self.ast.types.record_type(closure));
        self.check_lambda_object_base_access(t, closure_ty, path, loc)
    }

    /// `the type must be the same as, or derived from the lambda`.
    fn lambda_object_not_derived(&mut self, t: QualType, loc: Loc) -> bool {
        let m = self
            .diag(DiagId::sema_explicit_object_lambda_capture_type, loc)
            .arg(self.ty_arg(t))
            .arg(0u8);
        self.emit(m);
        false
    }

    /// Records capture `c` of open lambda `l` with member type `ty`.
    fn add_capture(&mut self, l: LambdaId, c: LambdaCapture, ty: QualType) {
        self.ast.exprs.lambdas.get_mut(l).captures.push(c);
        if let Some(s) = self.lambda_scope_mut(l) {
            s.cap_tys.push(ty);
        }
    }

    /// The type of a use of variable `d` at `loc` and whether it refers
    /// to an enclosing variable or capture (clang
    /// `getCapturedDeclRefType`), and the immediate capture of a variable
    /// that cannot be usable in constant expressions.
    pub(crate) fn capture_at_decl_ref(&mut self, d: DeclId, loc: Loc) -> Option<QualType> {
        if !self.may_need_capture(d) {
            return None;
        }
        let r = self.try_capture_var(d, loc, TryCapture::Implicit, None, Loc::INVALID, false);
        if self.unevaluated == 0 && !self.might_be_constant(d) {
            let _ = self.try_capture_var(d, loc, TryCapture::Implicit, None, Loc::INVALID, true);
        }
        match r {
            Captured::Yes(t) => Some(t),
            _ => None,
        }
    }

    /// At the end of a full-expression: a use of `d` at `loc` that is an
    /// odr-use captures it, if it might be usable in constant expressions
    /// (any other was captured when named).
    pub(crate) fn capture_odr_use(&mut self, d: DeclId, loc: Loc) {
        if self.might_be_constant(d) && self.may_need_capture(d) {
            let _ = self.try_capture_var(d, loc, TryCapture::Implicit, None, Loc::INVALID, true);
        }
    }

    /// `err_lambda_impcap` for variable `d` used at `loc` in lambda `l`,
    /// which has no capture-default, with clang's notes and fix-it notes
    /// (`buildLambdaCaptureFixit`).
    fn diagnose_impcap(&mut self, d: DeclId, loc: Loc, l: LambdaId) {
        let name = self.decl_name(d);
        let data = self.ast.exprs.lambdas.get(l);
        let (lbracket, rbracket) = (data.lbracket, data.rbracket);
        let explicit_vars = data.captures.iter().any(|c| {
            !c.implicit && !c.init && matches!(c.kind, CaptureKind::ByCopy | CaptureKind::ByRef)
        });
        let mut msg = self.diag(DiagId::sema_lambda_impcap, loc).arg(name.clone());
        msg = msg.with_note(
            self.diag(DiagId::note_entity_declared_at, self.ast.decls.get(d).loc())
                .arg(format!("'{name}'")),
        );
        msg = msg.with_note(self.diag(DiagId::note_lambda_decl, lbracket));
        msg = msg
            .with_note(self.diag(DiagId::note_lambda_capture_by_value, rbracket).arg(name.clone()));
        msg = msg.with_note(self.diag(DiagId::note_lambda_capture_by_ref, rbracket).arg(name));
        if !explicit_vars {
            let at = lbracket.with_offset(1);
            msg = msg.with_note(self.diag(DiagId::note_lambda_default_by_value, at));
            msg = msg.with_note(self.diag(DiagId::note_lambda_default_by_ref, at));
        }
        self.emit(msg);
    }

    /// A use at `loc` of local `d` of `home` from a context that cannot
    /// capture (a local class's member function).
    fn diagnose_enclosing_local(&mut self, d: DeclId, loc: Loc, home: DeclId) {
        let name = self.decl_name(d);
        let what = if self.lambda_of_call_op(home).is_some() {
            "lambda expression".to_string()
        } else {
            format!("function '{}'", self.decl_name(home))
        };
        let note = self
            .diag(DiagId::note_entity_declared_at, self.ast.decls.get(d).loc())
            .arg(format!("'{name}'"));
        let msg = self
            .diag(DiagId::sema_reference_to_local_in_enclosing, loc)
            .arg(name)
            .arg(what)
            .with_note(note);
        self.emit(msg);
    }

    /// clang `CheckCXXThisCapture`: captures `this` named at `loc` in
    /// every lambda up to the enclosing member function; `top` is the
    /// lambda being introduced by an explicit `this` (`true`: `*this`).
    /// False after an error.
    pub(crate) fn capture_this(&mut self, loc: Loc, top: Option<(LambdaId, bool)>) -> bool {
        let mut chain: Vec<LambdaId> = top.map(|t| t.0).into_iter().collect();
        let mut c = self.ctx;
        for _ in 0..4096 {
            let Some(l) = self.lambda_of_call_op(c) else {
                if self.kind_of(c) == DeclKind::RequiresExprBody
                    && let Some(p) = self.ast.decls.get(c).ctx()
                {
                    c = p;
                    continue;
                }
                break;
            };
            let data = self.ast.exprs.lambdas.get(l);
            if data.this_capture().is_some() {
                break;
            }
            if data.default == CaptureDefault::None {
                let rbracket = data.rbracket;
                if let Some(f) = data.call_op {
                    self.ast.decls.set_flags(f, DeclFlags::INVALID);
                }
                let mut msg = self.diag(DiagId::sema_this_capture, loc).arg(1u32);
                if top.is_none() {
                    msg =
                        msg.with_note(self.diag(DiagId::note_lambda_this_capture_fixit, rbracket));
                }
                self.emit(msg);
                return false;
            }
            chain.push(l);
            let Some(p) = self.lambda_parent(l) else { break };
            c = p;
        }
        let Some(this_ty) = self.current_this() else { return false };
        for (j, &l) in chain.iter().enumerate() {
            let explicit = j == 0 && top.is_some_and(|t| t.0 == l);
            let star = explicit && top.is_some_and(|t| t.1);
            let ty = if star {
                self.pointee_sugared(this_ty).unwrap_or(QualType::ERROR)
            } else {
                this_ty
            };
            let c = LambdaCapture {
                kind: if star { CaptureKind::StarThis } else { CaptureKind::This },
                var: None,
                field: None,
                loc,
                ellipsis: Loc::INVALID,
                implicit: !explicit,
                init: false,
            };
            self.add_capture(l, c, ty);
        }
        true
    }

    /// A potentially-evaluated use of `this` at `loc` (written, or the
    /// implicit object of a member access) captures it.
    pub(crate) fn note_this_use(&mut self, loc: Loc) {
        if self.unevaluated == 0 && !self.lambdas.is_empty() {
            let _ = self.capture_this(loc, None);
        }
    }

    /// The explicit captures of lambda `id` ([expr.prim.lambda.capture]/2-8;
    /// clang `ActOnLambdaExpressionAfterIntroducer`).
    #[allow(clippy::too_many_lines, reason = "one arm per capture form, as clang")]
    pub(crate) fn act_on_explicit_captures(&mut self, id: LambdaId, intro: &LambdaIntroducer) {
        // Each name with whether it is an init-capture and its variable.
        let mut names: Vec<(Symbol, bool, Option<DeclId>)> = Vec::new();
        for c in &intro.captures {
            let Some((sym, nloc)) = c.name else {
                // `this` / `*this`.
                if self.ast.exprs.lambdas.get(id).this_capture().is_some() {
                    let msg = self.diag(DiagId::sema_capture_more_than_once, c.loc).arg("this");
                    self.emit(msg);
                    continue;
                }
                if self.current_this().is_none() {
                    let msg = self.diag(DiagId::sema_this_capture, c.loc).arg(0u32);
                    self.emit(msg);
                    continue;
                }
                let _ = self.capture_this(c.loc, Some((id, c.kind == CaptureKind::StarThis)));
                continue;
            };
            let text = self.ast.symbols().as_str(sym).to_string();
            if let Some(init) = c.init {
                // clang skips an init-capture whose initializer is invalid.
                if self.is_error_expr(init) {
                    continue;
                }
                // An instantiated init-capture (`var` names the
                // pattern's variable, templ/subst/lambda.rs) was checked
                // in the pattern; the elements of an expanded
                // init-capture pack `...xs = init` share a name.
                // [basic.scope.scope]/5 (P2169): init-captures `_` are
                // name-independent.
                let placeholder = self.is_placeholder_name(sym);
                let repeated = names.iter().any(|&(s, _, _)| s == sym);
                if c.var.is_none()
                    && repeated
                    && placeholder
                    && names.iter().all(|e| e.0 != sym || e.1)
                {
                    self.diagnose_placeholder_redecl(nloc);
                } else if c.var.is_none() && repeated {
                    let msg = self.diag(DiagId::sema_capture_more_than_once, nloc).arg(text);
                    self.emit(msg);
                    continue;
                }
                names.push((sym, true, c.var));
                let by_ref = c.kind == CaptureKind::ByRef;
                self.add_init_capture(id, sym, nloc, by_ref, c.ellipsis, init);
                continue;
            }
            let default = intro.default;
            if c.kind == CaptureKind::ByRef && default == CaptureDefault::ByRef {
                self.error(DiagId::sema_ref_capture_with_ref_default, nloc);
                continue;
            }
            if c.kind == CaptureKind::ByCopy && default == CaptureDefault::ByCopy {
                self.error(DiagId::sema_copy_capture_with_copy_default, nloc);
                continue;
            }
            let found = match c.var {
                Some(v) => {
                    crate::lookup::Found { decls: vec![v], found: vec![v], ..Default::default() }
                }
                None => self.lookup_name(sym, LookupKind::Ordinary),
            };
            if found.is_empty() {
                let msg = self.diag(DiagId::sema_undeclared_var_use, nloc).arg(text);
                self.emit(msg);
                continue;
            }
            let Some(var) = found.single().filter(|&d| {
                matches!(self.kind_of(d), DeclKind::Var | DeclKind::ParmVar | DeclKind::Binding)
            }) else {
                let field = found.single().is_some_and(|d| self.kind_of(d) == DeclKind::Field);
                let id = if field {
                    DiagId::sema_capture_class_member
                } else {
                    DiagId::sema_capture_not_variable
                };
                let msg = self.diag(id, nloc).arg(text);
                self.emit(msg);
                continue;
            };
            // The elements of a capture pack instantiated from
            // `ts...` share a name, not a variable (templ/subst/lambda.rs
            // names each one).
            // A capture of `_` after an init-capture `_` (clang).
            if let Some(&(_, init, _)) = names.iter().find(|&&(s, i, v)| {
                s == sym
                    && (c.var.is_none() || v == Some(var))
                    && !(i && self.is_placeholder_name(s))
            }) {
                let msg = self.diag(DiagId::sema_capture_more_than_once, nloc).arg(text);
                self.emit(msg);
                if !init {
                    self.ast.decls.set_flags(var, DeclFlags::INVALID);
                }
                continue;
            }
            names.push((sym, false, Some(var)));
            if self.ast.decls.get(var).flags().contains(DeclFlags::INVALID) {
                continue;
            }
            if !self.has_local_storage(var) {
                let note = self
                    .diag(DiagId::note_entity_declared_at, self.ast.decls.get(var).loc())
                    .arg(format!("'{text}'"));
                let msg =
                    self.diag(DiagId::sema_capture_not_automatic, nloc).arg(text).with_note(note);
                self.emit(msg);
                continue;
            }
            let kind = if c.kind == CaptureKind::ByRef {
                TryCapture::ExplicitByRef
            } else {
                TryCapture::ExplicitByCopy
            };
            let ellipsis = c.ellipsis.unwrap_or(Loc::INVALID);
            let _ = self.try_capture_var(var, nloc, kind, Some(id), ellipsis, true);
        }
    }

    /// The init-capture `name = init` (`&name = init` when `by_ref`) of
    /// lambda `id` ([expr.prim.lambda.capture]/6): its type is deduced as
    /// for `auto name = init;` (`auto &name = init;`); the variable is
    /// declared when the body opens. With `ellipsis`, an init-capture
    /// pack `...name = init` ([expr.prim.lambda.capture]/17): `init` is
    /// the pattern, which must name an unexpanded pack, and the variable
    /// is a pack of type `auto...` (clang `buildLambdaInitCaptureInitialization`
    /// with an ellipsis), expanded when the enclosing template is
    /// instantiated.
    fn add_init_capture(
        &mut self,
        id: LambdaId,
        name: Symbol,
        loc: Loc,
        by_ref: bool,
        ellipsis: Option<Loc>,
        init: ExprId,
    ) {
        let auto = self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false);
        let declared = if by_ref { self.ast.types.lvalue_ref_to(auto) } else { auto };
        let ellipsis = ellipsis.filter(|&l| {
            let packs = self.ast.exprs.get(init).deps().contains(ExprDeps::UNEXPANDED_PACK);
            if !packs {
                self.error(DiagId::sema_pack_expansion_without_packs, l);
            }
            packs
        });
        if let Some(l) = ellipsis {
            // The pattern stays as written; each element is deduced and
            // initialized when the pack is expanded.
            let ty = self.ast.types.pack_expansion(declared, None);
            let index = self.ast.exprs.lambdas.get(id).captures.len();
            let c = LambdaCapture {
                kind: if by_ref { CaptureKind::ByRef } else { CaptureKind::ByCopy },
                var: None,
                field: None,
                loc,
                ellipsis: l,
                implicit: false,
                init: true,
            };
            self.add_capture(id, c, ty);
            if let Some(s) = self.lambda_scope_mut(id) {
                s.init_caps.push(InitCapture { index, name, loc, ty, init });
            }
            return;
        }
        let ty = if let Some(t) = self.deduce_from(declared, init) {
            t
        } else {
            let ety = self.ety(init);
            let msg = self
                .diag(DiagId::sema_auto_var_deduction_failure, loc)
                .arg(self.ast.symbols().as_str(name).to_string())
                .arg(self.ty_arg(declared))
                .arg(self.ty_arg(ety));
            self.emit(msg);
            QualType::ERROR
        };
        // A class that is (a member of) the current instantiation is a
        // dependent type ([temp.dep.type]/7): its constructors are chosen
        // at instantiation, as for a variable (`defer_dependent_init`).
        let dependent_class =
            self.ast.types.is_dependent(ty) && self.ast.types.as_record(ty).is_some();
        let conv = if self.is_error_type(ty) || dependent_class {
            init
        } else {
            // clang `InitializedEntity::InitializeLambdaCapture`.
            self.with_init_entity(crate::init::entity::CAPTURE, |s| {
                s.perform_init(ty, Initializer::Expr(init), loc, false)
            })
        };
        let index = self.ast.exprs.lambdas.get(id).captures.len();
        let c = LambdaCapture {
            kind: if by_ref { CaptureKind::ByRef } else { CaptureKind::ByCopy },
            var: None,
            field: None,
            loc,
            ellipsis: Loc::INVALID,
            implicit: false,
            init: true,
        };
        self.add_capture(id, c, ty);
        if let Some(s) = self.lambda_scope_mut(id) {
            s.init_caps.push(InitCapture { index, name, loc, ty, init: conv });
        }
    }

    /// The init-capture variables of lambda `id`, at its introducer: in
    /// scope in the *lambda-declarator* ([expr.prim.lambda.capture]/6,
    /// P2036, a DR) through [`Sema::lambda_init_captures_named`], in the
    /// enclosing context until the call operator adopts them
    /// ([`Sema::declare_init_captures`]).
    pub(crate) fn create_init_capture_vars(&mut self, id: LambdaId) {
        let caps: Vec<InitCapture> =
            self.lambda_scope_mut(id).map(|s| std::mem::take(&mut s.init_caps)).unwrap_or_default();
        let ctx = self.ctx;
        let mut vars = Vec::with_capacity(caps.len());
        for ic in caps {
            let independent = self.is_placeholder_name(ic.name);
            let v = self.builder().init_capture_var(
                ctx,
                DeclName::ident(ic.name),
                ic.loc,
                ic.ty,
                independent,
            );
            let _ = self.builder().set_var_init(v, ic.init);
            self.ast.exprs.lambdas.get_mut(id).captures[ic.index].var = Some(v);
            vars.push((ic.name, v));
        }
        if let Some(s) = self.lambda_scope_mut(id) {
            s.init_cap_vars = vars;
        }
    }

    /// The init-capture variable named `name` of a lambda whose
    /// introducer was at scope depth `depth` (found by unqualified lookup
    /// before the scopes below it, after the lambda's template and
    /// function parameters).
    /// Several name-independent init-captures `_` are all found
    /// ([basic.scope.scope]/5), in declaration order.
    pub(crate) fn lambda_init_captures_named(&self, depth: usize, name: Symbol) -> Vec<DeclId> {
        let Some(s) = self
            .lambdas
            .iter()
            .rev()
            .filter(|s| s.scope_base == depth)
            .find(|s| s.init_cap_vars.iter().any(|e| e.0 == name))
        else {
            return Vec::new();
        };
        let mut v: Vec<DeclId> =
            s.init_cap_vars.iter().filter(|e| e.0 == name).map(|e| e.1).collect();
        let last = v.len() - 1;
        if !self.ast.decls.get(v[last]).misc().name_independent() {
            v.drain(..last);
        }
        v
    }

    /// Declares the init-capture variables of lambda `id`, whose call
    /// operator `f` is opening its body, in the body's scope: `f`
    /// adopts the variables [`Sema::create_init_capture_vars`] made.
    pub(crate) fn declare_init_captures(&mut self, id: LambdaId, f: DeclId) {
        let vars: Vec<(Symbol, DeclId)> =
            self.lambda_scope_mut(id).map(|s| s.init_cap_vars.clone()).unwrap_or_default();
        let dependent = self.ast.templates.dependent_depth > 0;
        for (name, v) in vars {
            self.ast.decls.set_init_capture_owner(v, f, dependent);
            self.push_local(name, v);
        }
    }

    /// The type of `this` in the body of lambda `id` whose enclosing
    /// `this` has type `outer`: a `*this` capture designates the
    /// closure's copy, `const` unless the lambda is `mutable`
    /// ([expr.prim.this]/2, [expr.prim.lambda.closure]/5).
    pub(crate) fn lambda_body_this(
        &mut self,
        id: LambdaId,
        outer: Option<QualType>,
    ) -> Option<QualType> {
        let data = self.ast.exprs.lambdas.get(id);
        let Some(i) = data.captures.iter().position(|c| c.kind == CaptureKind::StarThis) else {
            return outer;
        };
        let konst = self.lambda_capture_const(id);
        let mut t = self.capture_type(id, i);
        if konst {
            t = self.ast.types.qualify(t, Quals::CONST);
        }
        Some(self.ast.types.pointer_to(t))
    }

    /// At the `}` of lambda `id` (the enclosing context restored): the
    /// closure's unnamed members, one per capture in order
    /// ([expr.prim.lambda.capture]/10-12), and their initializers
    /// ([expr.prim.lambda.capture]/15), and clang's deprecation of an
    /// implicit `this` capture by `[=]`.
    pub(crate) fn finish_captures(&mut self, id: LambdaId, closure: DeclId, cap_tys: &[QualType]) {
        let data = self.ast.exprs.lambdas.get(id);
        if data.default != CaptureDefault::None || !data.captures.is_empty() {
            self.ast
                .decls
                .set_definition_bits(closure, kiln_ast::decl::DefinitionBits::LAMBDA_CAPTURES);
        }
        if data.captures.is_empty() {
            return;
        }
        let caps = data.captures.clone();
        let (default, default_loc) = (data.default, data.default_loc);
        let mut inits = Vec::with_capacity(caps.len());
        for (i, c) in caps.iter().enumerate() {
            let ty = cap_tys.get(i).copied().unwrap_or(QualType::ERROR);
            let h = DeclHead::new(closure, DeclName::EMPTY, c.loc).flags(DeclFlags::IMPLICIT);
            let field = self.builder().field(h, ty, None, 0);
            self.ast.exprs.lambdas.get_mut(id).captures[i].field = Some(field);
            let at = if c.implicit { default_loc } else { c.loc };
            let init = match c.kind {
                CaptureKind::This => {
                    if c.implicit && default == CaptureDefault::ByCopy {
                        let note = self.diag(DiagId::note_deprecated_this_capture, default_loc);
                        let msg =
                            self.diag(DiagId::sema_deprecated_this_capture, c.loc).with_note(note);
                        self.emit(msg);
                    }
                    let t = self.current_this().unwrap_or(ty);
                    Some(self.builder().this_expr(t, c.implicit, at))
                }
                CaptureKind::StarThis => {
                    let t = self.current_this().unwrap_or(QualType::ERROR);
                    let this = self.builder().this_expr(t, c.implicit, at);
                    let obj = self.builtin_unary(UnaryOp::Deref, this, at);
                    if self.is_dependent_type(ty) {
                        // Initialized when instantiated.
                        Some(obj)
                    } else {
                        self.init_subobject_from(ty, obj, InitEntity::Field(field), at).ok()
                    }
                }
                _ if c.init => c.var.and_then(|v| self.ast.decls.var(v).and_then(|x| x.init)),
                _ => c.var.and_then(|v| {
                    let r = self.build_decl_ref_extra(v, at, None);
                    self.init_subobject_from(ty, r, InitEntity::Field(field), at).ok()
                }),
            };
            inits.push(init.unwrap_or_else(|| self.act_on_error_expr(&[], at)));
        }
        let list = self.builder().list(&inits);
        self.ast.exprs.lambdas.get_mut(id).inits = Some(list);
    }
}
