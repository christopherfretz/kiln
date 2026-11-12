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

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, InitStyle, VarSpec};
use kiln_ast::expr::{ExprDeps, UnaryOp};
use kiln_ast::ids::{ExprId, LambdaId};
use kiln_ast::lambda::{CaptureDefault, CaptureKind, LambdaCapture};
use kiln_ast::names::DeclName;
use kiln_ast::types::{AutoKeyword, Quals};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::{DiagId, FixIt};
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, Span};

use super::{LambdaIntroducer, LambdaScope};
use crate::Sema;
use crate::class::InitEntity;
use crate::conv::bases::{BaseSearch, ambiguous_paths_string, find_base};
use crate::declspec::ParamInfo;
use crate::init::Initializer;
use crate::lookup::LookupKind;

/// Whether a declaration of kind `k` names an entity a lambda can
/// capture ([expr.prim.lambda.capture]/4: a local entity, [basic.pre]/9):
/// a variable, a structured binding or a result binding.
fn is_capturable_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::Var | DeclKind::ParmVar | DeclKind::Binding | DeclKind::ContractResultName
    )
}

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
    /// Its variable, with its deduced type and converted initializer.
    pub(crate) var: DeclId,
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
            // A result binding is a local entity ([basic.pre]/9): a lambda
            // in its postcondition captures it (spec/contracts.md §5).
            DeclKind::ParmVar | DeclKind::ContractResultName => true,
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
        is_capturable_kind(self.kind_of(d))
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
                // variable a specialization may odr-use. A use that is not
                // one still has the type a capture would give it under a
                // capture-default ([expr.prim.id.unqual]/3).
                if !self.lambdas.iter().any(|s| s.id == l) {
                    if build || data.default == CaptureDefault::None {
                        return Err(Captured::NotNeeded);
                    }
                    chain.push(l);
                    let Some(p) = self.lambda_parent(l) else { return Err(Captured::Failed) };
                    c = p;
                    continue;
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
        if !is_capturable_kind(self.kind_of(d)) {
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
        if let Some((l, i)) = start {
            self.mark_capture_used(l, i, build);
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
        let mut fresh: Vec<LambdaId> = Vec::new();
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
                if !explicit {
                    fresh.push(l);
                }
            }
        }
        if build {
            self.note_contract_capture_use(Some(d), loc, &fresh);
        }
        // A pack names its pattern ([temp.variadic]/5), as an uncaptured
        // pack's `DeclRefExpr`: a use of an init-capture pack `xs` of type
        // `auto...` names an element of a type deduced at instantiation,
        // a dependent type (clang deduces the pattern's `auto` as
        // dependent), not `const auto...`.
        if let kiln_ast::types::TypeView::PackExpansion { pattern, .. } =
            self.ast.types.view(rf.ty())
        {
            rf = if self.contains_undeduced_auto(pattern) {
                QualType::DEPENDENT
            } else {
                self.ast.types.qualify(pattern, rf.quals())
            };
        }
        Captured::Yes(rf)
    }

    /// Whether a capture by copy of lambda `l` is `const` in its body
    /// ([expr.prim.id.unqual]/3): with an explicit object parameter, when
    /// the parameter's (referenced) type is const-qualified; otherwise
    /// unless the lambda is `mutable` (clang
    /// `LambdaScopeInfo::lambdaCaptureShouldBeConst`).
    ///
    /// The call operator is the one being analysed: inside a
    /// specialization of a generic lambda's call operator (its body
    /// instantiated), that specialization's explicit object parameter
    /// decides, so `const Self&` makes the captures const there and
    /// `Self&` does not (WP-364).
    pub(crate) fn lambda_capture_const(&self, l: LambdaId) -> bool {
        let data = self.ast.exprs.lambdas.get(l);
        let op = self.current_call_op(l).or(data.call_op);
        let obj = op.filter(|&f| self.has_explicit_object(f)).and_then(|f| {
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

    /// The call operator of lambda `l` (or a specialization of it) that
    /// encloses the current context, if any.
    fn current_call_op(&self, l: LambdaId) -> Option<DeclId> {
        let mut c = self.ctx;
        for _ in 0..4096 {
            if self.lambda_of_call_op(c) == Some(l) {
                return Some(c);
            }
            c = self.ast.decls.get(c).ctx()?;
        }
        None
    }

    /// [temp.dep.expr]/3 (P0847): whether a use of variable `d` in the
    /// current context names a capture by copy (existing, or implied by a
    /// `=` default) of a lambda whose explicit object parameter has a
    /// dependent type; such a use is type-dependent, its constness being
    /// that of each specialization's object parameter (clang
    /// `isCapturedByCopyInLambdaWithExplicitObjectParameter`).
    pub(crate) fn captured_by_copy_in_dependent_object_lambda(&self, d: DeclId) -> bool {
        let Some(home) = self.ast.decls.get(d).ctx() else { return false };
        let mut c = self.ctx;
        for _ in 0..4096 {
            if c == home {
                return false;
            }
            if let Some(l) = self.lambda_of_call_op(c) {
                let dependent = self.has_explicit_object(c)
                    && self
                        .ast
                        .decls
                        .function(c)
                        .and_then(|x| self.ast.lists.iter::<DeclId>(x.params).next())
                        .and_then(|p| self.ast.decls.get(p).ty())
                        .is_some_and(|t| self.ast.types.is_dependent(t));
                let data = self.ast.exprs.lambdas.get(l);
                if dependent {
                    let by_copy = match data.captures.iter().find(|x| x.var == Some(d)) {
                        Some(x) => x.kind == CaptureKind::ByCopy,
                        None => data.default == CaptureDefault::ByCopy,
                    };
                    if by_copy {
                        return true;
                    }
                }
                let Some(p) = self.lambda_parent(l) else { return false };
                c = p;
                continue;
            }
            if self.kind_of(c) == DeclKind::RequiresExprBody
                && let Some(p) = self.ast.decls.get(c).ctx()
            {
                c = p;
                continue;
            }
            return false;
        }
        false
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
        let closure_ty = QualType::unqual(self.ast.types.record_type(closure));
        match find_base(self, r, closure) {
            BaseSearch::NotDerived => self.lambda_object_not_derived(t, loc),
            BaseSearch::Unique(path) => {
                self.check_lambda_object_base_access(t, closure_ty, &path, loc)
            }
            BaseSearch::Ambiguous(paths) => {
                let shown = ambiguous_paths_string(self, t, &paths);
                let m = self
                    .diag(DiagId::sema_explicit_object_lambda_ambiguous_base, loc)
                    .arg(self.ty_arg(closure_ty))
                    .arg(shown);
                self.emit(m);
                false
            }
        }
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
            s.cap_use.push(0);
        }
    }

    /// clang `Capture::markUsed`: capture `i` of lambda `l` is named, as
    /// an odr-use when `odr` (`-Wunused-lambda-capture`). No-op once the
    /// lambda is complete.
    fn mark_capture_used(&mut self, l: LambdaId, i: usize, odr: bool) {
        if let Some(u) = self.lambda_scope_mut(l).and_then(|s| s.cap_use.get_mut(i)) {
            *u |= if odr { 1 } else { 2 };
        }
    }

    /// The index of lambda `l`'s `this` / `*this` capture.
    fn this_capture_index(&self, l: LambdaId) -> Option<usize> {
        self.ast
            .exprs
            .lambdas
            .get(l)
            .captures
            .iter()
            .position(|c| matches!(c.kind, CaptureKind::This | CaptureKind::StarThis))
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
        // A discarded statement only formally odr-uses `d` (clang's
        // `OdrUseContext::FormallyOdrUsed`): nothing is captured.
        if self.unevaluated == 0 && !self.might_be_constant(d) && !self.in_discarded_statement() {
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
        if self.might_be_constant(d) && self.may_need_capture(d) && !self.in_discarded_statement() {
            let _ = self.try_capture_var(d, loc, TryCapture::Implicit, None, Loc::INVALID, true);
        }
    }

    /// A use `e` of variable `d` in a dependent context (a generic
    /// lambda's body, or a lambda in a template): when `d` is a local of
    /// an enclosing function that might be usable in constant
    /// expressions, its capture waits for the end of the full-expression
    /// ([`Sema::resolve_potential_captures`]); any other was captured
    /// when named. A reference whose initializer is a constant
    /// expression is never captured (clang `DoMarkPotentialCapture`).
    pub(crate) fn note_potential_capture(&mut self, e: ExprId, d: DeclId) {
        if self.lambdas.is_empty() || !self.might_be_constant(d) || !self.may_need_capture(d) {
            return;
        }
        let is_ref =
            self.ast.types.is_reference(self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR));
        if is_ref && self.usable_in_constant_expressions(d) {
            return;
        }
        self.potential_caps.push(e);
    }

    /// The end of a full-expression `full` in a dependent context:
    /// [expr.prim.lambda.capture]/7-8 (clang
    /// `CheckIfAnyEnclosingLambdasMustCaptureAnyPotentialCaptures`).
    /// Each potential capture is captured by the enclosing lambdas unless
    /// `full` is not instantiation-dependent and the use is a potential
    /// result `constant` (an lvalue-to-rvalue conversion is applied to
    /// it) of a variable usable in constant expressions, which is no
    /// odr-use. A generic lambda's call operator is instantiated after
    /// its closure type is complete, so this is where a captured
    /// reference such as `const T& r = *p; [&](const auto& x) { return x
    /// == r; }` gets its member (WP-581). When no lambda can capture it,
    /// the use is diagnosed now if it must be an odr-use in every
    /// instantiation.
    pub(crate) fn resolve_potential_captures(
        &mut self,
        full: ExprId,
        constant: &crate::expr::ConstantCandidates,
    ) {
        let dependent = self.ast.exprs.get(full).deps().intersects(ExprDeps::TYPE_VALUE_INST);
        for r in std::mem::take(&mut self.potential_caps) {
            let d = self.ast.exprs.decl(r);
            if !dependent && constant.contains(r) && self.usable_in_constant_expressions(d) {
                continue;
            }
            let loc = self.eloc(r);
            let probe =
                self.try_capture_var(d, loc, TryCapture::Implicit, None, Loc::INVALID, false);
            if matches!(probe, Captured::Yes(_)) || !dependent || self.never_constant(d) {
                let _ =
                    self.try_capture_var(d, loc, TryCapture::Implicit, None, Loc::INVALID, true);
            }
        }
    }

    /// clang `VariableCanNeverBeAConstantExpression`: variable `d`, with a
    /// non-dependent type and initializer, is not usable in constant
    /// expressions.
    fn never_constant(&mut self, d: DeclId) -> bool {
        let Some(t) = self.ast.decls.get(d).ty() else { return false };
        if self.is_dependent_type(t) {
            return false;
        }
        match self.ast.decls.var(d).and_then(|v| v.init) {
            None => true,
            Some(init) if self.expr_is_value_dependent(init) => false,
            Some(_) => !self.usable_in_constant_expressions(d),
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

    /// Whether `this` named here could be captured by every enclosing
    /// lambda up to the member function without a diagnostic: each
    /// captured it already or has a *capture-default*
    /// ([expr.prim.lambda.capture]/8; clang's `CheckCXXThisCapture` with
    /// `BuildAndDiagnose` false).
    pub(crate) fn this_capturable(&self) -> bool {
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
                return false;
            }
            let Some(p) = self.lambda_parent(l) else { break };
            c = p;
        }
        self.current_this().is_some()
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
            if let Some(i) = self.this_capture_index(l) {
                self.mark_capture_used(l, i, true);
                break;
            }
            let data = self.ast.exprs.lambdas.get(l);
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
        let fresh: Vec<LambdaId> = chain
            .iter()
            .enumerate()
            .filter(|&(j, &l)| !(j == 0 && top.is_some_and(|t| t.0 == l)))
            .map(|(_, &l)| l)
            .collect();
        self.note_contract_capture_use(None, loc, &fresh);
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

    /// [expr.prim.lambda.closure]/10 (P2900R14): a reference at `loc` to
    /// the local entity `d` (`None`: `*this`) that the open lambdas
    /// `fresh` have just captured implicitly. A lambda for which the
    /// reference is in one of its contract assertions (its call
    /// operator's function contract assertions, or a `contract_assert`
    /// in its body, a nested lambda's included) remembers a fresh capture
    /// as contract-only; a reference outside its contract assertions
    /// clears that. Free when no contract assertion is open and none is
    /// pending.
    fn note_contract_capture_use(&mut self, d: Option<DeclId>, loc: Loc, fresh: &[LambdaId]) {
        if self.contract_ctx.is_empty() && self.lambdas.iter().all(|s| s.contract_caps.is_empty()) {
            return;
        }
        for i in 0..self.lambdas.len() {
            // A contract assertion opened while lambda `i` was open is
            // one of its own or lies in its body.
            let inside = self.contract_ctx.iter().any(|c| c.lambdas > i);
            let s = &mut self.lambdas[i];
            if inside {
                if fresh.contains(&s.id) {
                    s.contract_caps.push((d, loc));
                }
            } else if !s.contract_caps.is_empty() {
                s.contract_caps.retain(|x| x.0 != d);
            }
        }
    }

    /// At a lambda's end: each implicit capture `caps` left referenced
    /// only in the lambda's contract assertions makes the program
    /// ill-formed ([expr.prim.lambda.closure]/10: "adding a contract
    /// assertion to an existing C++ program cannot cause additional
    /// captures"); GCC 16.1's wording. The enclosing lambda does not
    /// report the same reference again.
    pub(crate) fn check_contract_only_captures(&mut self, caps: &[(Option<DeclId>, Loc)]) {
        for &(d, loc) in caps {
            let name = d.map_or_else(|| "this".to_string(), |d| self.decl_name(d));
            let mut msg =
                self.diag(DiagId::sema_contract_only_implicit_capture, loc).arg(name.clone());
            if let Some(d) = d {
                msg = msg.with_note(
                    self.diag(DiagId::note_entity_declared_at, self.ast.decls.get(d).loc())
                        .arg(format!("'{name}'")),
                );
            }
            self.emit(msg);
            if let Some(outer) = self.lambdas.last_mut() {
                outer.contract_caps.retain(|x| *x != (d, loc));
            }
        }
    }

    /// A potentially-evaluated use of `this` at `loc` (written, or the
    /// implicit object of a member access) captures it.
    pub(crate) fn note_this_use(&mut self, loc: Loc) {
        if self.unevaluated == 0 && !self.lambdas.is_empty() {
            let _ = self.capture_this(loc, None);
        }
    }

    /// [expr.prim.lambda.capture]/5: a lambda parameter does not have the
    /// name of an explicit capture (simple, init or pack capture; implicit
    /// captures do not count). Reported at the parameter's name with a note
    /// at the capture (clang `err_parameter_shadow_capture`). Checked in
    /// the pattern only.
    pub(crate) fn check_param_shadows_capture(&mut self, id: LambdaId, params: &[ParamInfo]) {
        if !self.inst.stack.is_empty() {
            return;
        }
        let caps: Vec<(DeclId, Loc)> = self
            .ast
            .exprs
            .lambdas
            .get(id)
            .captures
            .iter()
            .filter(|c| !c.implicit)
            .filter_map(|c| Some((c.var?, c.loc)))
            .collect();
        if caps.is_empty() {
            return;
        }
        for p in params {
            let Some((sym, ploc)) = p.name else { continue };
            let name = DeclName::ident(sym);
            let Some(&(var, cloc)) =
                caps.iter().find(|&&(v, _)| self.ast.decls.get(v).name() == name)
            else {
                continue;
            };
            let note = self
                .diag(DiagId::note_var_explicitly_captured_here, cloc)
                .arg(self.decl_arg(var))
                .arg(true);
            let msg = self.diag(DiagId::sema_parameter_shadow_capture, ploc).with_note(note);
            self.emit(msg);
        }
    }

    /// [temp.local]/6: an explicit capture (simple, by-reference or init)
    /// with the name of a parameter of the lambda's own
    /// *template-parameter-list* redeclares it: "declaration of 'x'
    /// shadows template parameter" at the capture, with a note at the
    /// parameter (clang, `expr.prim.lambda.capture/p5.cpp`). An enclosing
    /// template's parameters do not count. Checked in the pattern only;
    /// `heads` is the template-head depth before the lambda.
    pub(crate) fn check_capture_shadows_template_param(&mut self, id: LambdaId, heads: usize) {
        if !self.inst.stack.is_empty() {
            return;
        }
        let tparams = self.head_params_from(heads);
        if tparams.is_empty() {
            return;
        }
        let caps: Vec<(DeclId, Loc)> = self
            .ast
            .exprs
            .lambdas
            .get(id)
            .captures
            .iter()
            .filter(|c| !c.implicit)
            .filter_map(|c| Some((c.var?, c.loc)))
            .collect();
        for (var, cloc) in caps {
            let name = self.ast.decls.get(var).name();
            let Some(sym) = name.as_ident() else { continue };
            if let Some(&p) = tparams.iter().find(|&&p| self.ast.decls.get(p).name() == name) {
                self.diag_template_param_shadow(sym, cloc, p);
            }
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
            if let Some((style, init)) = &c.init {
                // clang skips an init-capture whose initializer is invalid.
                if self.initializer_has_error(init) {
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
                self.add_init_capture(id, sym, nloc, by_ref, c.ellipsis, *style, init.clone());
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
            let Some(var) = found.single().filter(|&d| is_capturable_kind(self.kind_of(d))) else {
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
        // The written ranges, for `-Wunused-lambda-capture`, when every
        // explicit capture was accepted (clang gives up on the lambda
        // otherwise).
        let explicit =
            self.ast.exprs.lambdas.get(id).captures.iter().filter(|c| !c.implicit).count();
        if explicit == intro.captures.len()
            && let Some(s) = self.lambda_scope_mut(id)
        {
            s.cap_ranges.extend(intro.captures.iter().map(|c| (c.loc, c.end)));
        }
    }

    /// clang `DiagnoseUnusedLambdaCapture` from `BuildLambdaExpr`: at the
    /// end of lambda `id` (scope `top`) outside a dependent context, each
    /// explicit capture that was never odr-used (`-Wunused-lambda-capture`),
    /// unless copying it may have side effects (an init-capture whose
    /// initializer may, a volatile copy, a copy of a class that is
    /// incomplete or not trivially copyable or destructible), it is an
    /// init-capture `_`, or a generic lambda's init-capture named without
    /// an odr-use. The fix-it removes the capture with the comma before it
    /// (after the last used capture or the default), or the comma after it
    /// when nothing used precedes it.
    pub(crate) fn diagnose_unused_lambda_captures(&mut self, id: LambdaId, top: &LambdaScope) {
        if top.cap_ranges.is_empty()
            || self.in_template()
            || !self.diags.is_enabled(DiagId::sema_unused_lambda_capture)
        {
            return;
        }
        let data = self.ast.exprs.lambdas.get(id);
        let (default, default_loc, lbracket) = (data.default, data.default_loc, data.lbracket);
        let n = top.cap_ranges.len();
        let generic = top.call_template.is_some();
        let mut has_prev = default != CaptureDefault::None;
        let mut prev = if has_prev { default_loc } else { lbracket };
        for (i, &(begin, end)) in top.cap_ranges.iter().enumerate() {
            let Some(&c) = self.ast.exprs.lambdas.get(id).captures.get(i) else { break };
            let used = top.cap_use.get(i).copied().unwrap_or(1);
            let mut diagnosed = false;
            if used & 1 == 0 && !(generic && used & 2 != 0 && c.init) {
                let fix = if !end.is_valid() {
                    None
                } else if !has_prev && i + 1 != n {
                    self.token_end(end)
                        .and_then(|e| Some(Span::new(begin, self.next_token_end(e)?)))
                } else {
                    self.token_end(prev).zip(self.token_end(end)).map(|(b, e)| Span::new(b, e))
                };
                let ty = top.cap_tys.get(i).copied().unwrap_or(QualType::ERROR);
                if !self.capture_has_side_effects(c, ty) && !self.capture_is_placeholder(c) {
                    let name = match c.var {
                        Some(v) => self.decl_arg(v),
                        None => "'this'".to_string(),
                    };
                    let mut d = self
                        .diag(DiagId::sema_unused_lambda_capture, c.loc)
                        .arg(name)
                        .arg(u32::from(used & 2 != 0));
                    if let Some(f) = fix.filter(|f| !f.lo.is_macro() && !f.hi.is_macro()) {
                        d = d.fixit(FixIt::remove(f));
                    }
                    // The elements of an expanded capture pack share a
                    // location; clang warns for each.
                    self.emit(d.allow_duplicate());
                    diagnosed = true;
                }
            }
            if end.is_valid() {
                has_prev |= !diagnosed;
                prev = end;
            }
        }
    }

    /// clang `CaptureHasSideEffects` for capture `c` whose member has
    /// type `ty`.
    fn capture_has_side_effects(&self, c: LambdaCapture, ty: QualType) -> bool {
        if c.init
            && let Some(v) = c.var
            && let Some(init) = self.ast.decls.var(v).and_then(|x| x.init)
            && self.has_possible_side_effects(init)
        {
            return true;
        }
        if !matches!(c.kind, CaptureKind::ByCopy | CaptureKind::StarThis) {
            return false;
        }
        if ty.quals().is_volatile() {
            return true;
        }
        let base = self.base_element(ty);
        let Some(r) = self.ast.types.as_record(base) else { return false };
        let bits = self.defn_bits(r);
        !bits.contains(DefinitionBits::COMPLETE)
            || !bits.contains(DefinitionBits::TRIVIAL_COPY_CTOR)
            || !bits.contains(DefinitionBits::TRIVIAL_DTOR)
    }

    /// An init-capture `_` ([basic.scope.scope]/5; clang
    /// `isPlaceholderVar`).
    fn capture_is_placeholder(&self, c: LambdaCapture) -> bool {
        c.init
            && c.var.is_some_and(|v| {
                self.ast.decls.get(v).name().as_ident().is_some_and(|s| self.is_placeholder_name(s))
            })
    }

    /// `at` plus the length of the token after the whitespace there:
    /// where clang's printed fix-it range ends for a token range ending
    /// at `at` (`Lexer::MeasureTokenLength` lexes raw from `at`).
    fn next_token_end(&self, at: Loc) -> Option<Loc> {
        let sm = self.diags.sm();
        let (file, pos) = sm.offset_of(at)?;
        let text = sm.text(file).as_bytes();
        let ws = text.get(pos as usize..)?.iter().take_while(|c| c.is_ascii_whitespace()).count();
        let tok = at.with_offset(u32::try_from(ws).ok()?);
        let len = sm.same_entry_distance(tok, self.token_end(tok)?)?;
        Some(at.with_offset(len))
    }

    /// Whether init-capture initializer `init` holds an invalid
    /// expression at its top level (the parser's recovery).
    fn initializer_has_error(&self, init: &Initializer) -> bool {
        match init {
            Initializer::Expr(e) => self.is_error_expr(*e),
            Initializer::Paren(es, ..) => es.iter().any(|&e| self.is_error_expr(e)),
            Initializer::List(l) => Self::list_exprs(l).iter().any(|&e| self.is_error_expr(e)),
        }
    }

    /// The init-capture `name initializer` (`&name initializer` when
    /// `by_ref`) of lambda `id` ([expr.prim.lambda.capture]/6): its
    /// variable is declared as if by `auto name initializer;` (`auto
    /// &name initializer;`), with the same deduction
    /// ([dcl.type.auto.deduct]: `{e}` deduces from `e`, `= {e, ...}` an
    /// `std::initializer_list`, `(e)` from `e`, more than one expression
    /// is ill-formed) and the same initialization: `( ... )` and `{ ... }`
    /// direct-initialize, `=` copy-initializes ([dcl.init.general]/15-16).
    /// The variable is published when the introducer ends
    /// ([`Sema::create_init_capture_vars`]) and declared when the body
    /// opens. With `ellipsis`, an init-capture pack `...name initializer`
    /// ([expr.prim.lambda.capture]/17): the initializer is the pattern,
    /// which must name an unexpanded pack, and the variable is a pack of
    /// type `auto...` (clang `buildLambdaInitCaptureInitialization` with
    /// an ellipsis), expanded when the enclosing template is
    /// instantiated.
    #[allow(clippy::too_many_arguments, reason = "the parts of one written capture")]
    fn add_init_capture(
        &mut self,
        id: LambdaId,
        name: Symbol,
        loc: Loc,
        by_ref: bool,
        ellipsis: Option<Loc>,
        style: InitStyle,
        init: Initializer,
    ) {
        let auto = self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false);
        let declared = if by_ref { self.ast.types.lvalue_ref_to(auto) } else { auto };
        let ctx = self.ctx;
        let independent = self.is_placeholder_name(name);
        let v =
            self.builder().init_capture_var(ctx, DeclName::ident(name), loc, declared, independent);
        self.ast.decls.mark_var(v, VarSpec::new().with_init_style(style));
        let index = self.ast.exprs.lambdas.get(id).captures.len();
        let kind = if by_ref { CaptureKind::ByRef } else { CaptureKind::ByCopy };
        let ellipsis = ellipsis.filter(|&l| {
            let packs = Self::initializer_exprs(&init)
                .iter()
                .any(|&e| self.ast.exprs.get(e).deps().contains(ExprDeps::UNEXPANDED_PACK));
            if !packs {
                self.error(DiagId::sema_pack_expansion_without_packs, l);
            }
            packs
        });
        let (ty, conv) = if ellipsis.is_some() {
            // The pattern stays as written, as one expression (a
            // dependent variable initializer's form); each element is
            // deduced and initialized when the pack is expanded.
            let pattern = self.dependent_initializer(&init);
            (self.ast.types.pack_expansion(declared, None), pattern)
        } else {
            let direct = style == InitStyle::List;
            // Deduced as `auto x <init>`, reported in clang's lambda-capture
            // wording (`deduce_var_auto` reads the entity).
            let ty = self
                .with_init_entity(crate::init::entity::CAPTURE, |s| {
                    s.deduce_var_auto(v, declared, &init, loc, direct)
                })
                .unwrap_or(QualType::ERROR);
            // A dependent initializer is kept for instantiation, as a
            // variable's (`dependent_init_form`); clang
            // `InitializedEntity::InitializeLambdaCapture` otherwise.
            let conv = match self.dependent_init_form(ty, &init) {
                Some(e) => e,
                None => self.with_init_entity(crate::init::entity::CAPTURE, |s| {
                    s.perform_init(ty, init, loc, direct)
                }),
            };
            (ty, conv)
        };
        self.ast.decls.set_ty(v, ty);
        let _ = self.builder().set_var_init(v, conv);
        let c = LambdaCapture {
            kind,
            var: Some(v),
            field: None,
            loc,
            ellipsis: ellipsis.unwrap_or(Loc::INVALID),
            implicit: false,
            init: true,
        };
        self.add_capture(id, c, ty);
        if let Some(s) = self.lambda_scope_mut(id) {
            s.init_caps.push(InitCapture { index, name, var: v });
        }
    }

    /// The top-level expressions of initializer `init`.
    fn initializer_exprs(init: &Initializer) -> Vec<ExprId> {
        match init {
            Initializer::Expr(e) => vec![*e],
            Initializer::Paren(es, ..) => es.clone(),
            Initializer::List(l) => Self::list_exprs(l),
        }
    }

    /// `init` kept as written, as one expression: the expression itself,
    /// a `ParenListExpr` or an `InitListExpr` (what
    /// `templ/subst` rebuilds an [`Initializer`] from, per the
    /// variable's init style).
    fn dependent_initializer(&mut self, init: &Initializer) -> ExprId {
        match init {
            Initializer::Expr(e) => *e,
            Initializer::Paren(es, ..) if es.len() == 1 && !self.is_braced_arg(es[0]) => es[0],
            Initializer::Paren(es, l, r) => {
                let es = self.dependent_args(es);
                self.builder().paren_list(&es, *l, *r)
            }
            Initializer::List(l) => self.dependent_init_list(l),
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
        let vars: Vec<(Symbol, DeclId)> = caps.iter().map(|ic| (ic.name, ic.var)).collect();
        debug_assert!(
            caps.iter()
                .all(|ic| self.ast.exprs.lambdas.get(id).captures[ic.index].var == Some(ic.var)),
            "init-capture variable recorded on its capture"
        );
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

    /// The type of `this` in the body of `f`, a specialization of a
    /// generic lambda's call operator instantiated after the lambda
    /// (spec/sema-lambda.md): the `this` of the context enclosing the
    /// closure type, not the closure's own ([expr.prim.this]/2,
    /// [expr.prim.lambda.closure]/12), as clang's `getCurrentThisType`
    /// walks out of lambdas. That context is a member function (or
    /// another lambda's call operator), or the class whose default member
    /// initializer holds a lambda that may capture `this`.
    pub(crate) fn instantiated_lambda_this(&mut self, f: DeclId) -> Option<QualType> {
        let closure = self.ast.decls.get(f).ctx()?;
        let id = self.ast.decls.record(closure).and_then(|r| r.lambda)?;
        let outer = self.ast.decls.get(closure).ctx()?;
        let outer_this = match self.kind_of(outer) {
            k if k.is_function() && self.is_lambda_call_op(outer) => {
                self.instantiated_lambda_this(outer)
            }
            k if k.is_function() => self.method_this_type(outer),
            DeclKind::CXXRecord => {
                let data = self.ast.exprs.lambdas.get(id);
                let may_capture = data.default != CaptureDefault::None
                    || data
                        .captures
                        .iter()
                        .any(|c| matches!(c.kind, CaptureKind::This | CaptureKind::StarThis));
                if !may_capture {
                    return None;
                }
                let t = self.this_class_type(outer);
                Some(self.ast.types.pointer_to(t))
            }
            _ => None,
        };
        self.lambda_body_this(id, outer_this)
    }

    /// At the `}` of lambda `id` (the enclosing context restored): the
    /// closure's unnamed members, one per capture in order
    /// ([expr.prim.lambda.capture]/10-12), and their initializers
    /// ([expr.prim.lambda.capture]/15), and clang's deprecation of an
    /// implicit `this` capture by `[=]`.
    pub(crate) fn finish_captures(&mut self, id: LambdaId, closure: DeclId, cap_tys: &[QualType]) {
        let data = self.ast.exprs.lambdas.get(id);
        if data.default != CaptureDefault::None || !data.captures.is_empty() {
            self.ast.decls.set_definition_bits(closure, DefinitionBits::LAMBDA_CAPTURES);
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
