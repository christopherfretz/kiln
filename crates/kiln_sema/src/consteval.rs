//! Immediate functions and immediate invocations ([expr.const]/16-19,
//! P2564; spec/sema.md §5.10).
//!
//! A call to an immediate function outside an immediate function context
//! is wrapped in a `ConstantExpr` when it is built (clang's
//! `CheckForImmediateInvocation`) and recorded as a candidate; a
//! `DeclRefExpr` naming one is recorded as a reference. At the end of the
//! full-expression ([`Sema::finish_immediate_invocations`]) the outermost
//! candidates are evaluated by the Oven (`EvalKind::Consteval`), nested
//! ones lose their wrapper, and a reference outside every candidate takes
//! the function's address. In an immediate-escalating function a failure
//! makes the function immediate instead of being an error.

use std::collections::{HashMap, HashSet};

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, Defaulted, FnSpec, VarEvalState};
use kiln_ast::expr::{ExprFlags, ExprKind};
use kiln_ast::templates::SpecKind;
use kiln_ast::{ConstValueId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalFailure, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::class::CleanupState;

/// Why an escalated function is immediate (clang's
/// `DiagnoseImmediateEscalatingReason`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Escalation {
    /// The call or the reference in its body.
    loc: Loc,
    /// The immediate function called or named.
    callee: DeclId,
    /// A call that is not a constant expression (else the address).
    call: bool,
    /// The member whose initializer holds the call (a constructor's
    /// default member initializer or mem-initializer; `loc` is the
    /// constructor).
    default_init: Option<DeclId>,
    /// `default_init` is a written mem-initializer ("the initializer of").
    written: bool,
    /// An immediate-escalating expression of this consteval-only type
    /// (`^^x`; `callee` is then the escalated function itself).
    consteval_only: Option<QualType>,
}

/// A use of a default argument or default member initializer whose
/// immediate invocations are checked at the end of the full-expression
/// ([expr.const]/16, P2564: they are evaluated where used).
#[derive(Clone, Copy, Debug)]
pub(crate) struct DefaultUse {
    /// The `CXXDefaultArgExpr` or `CXXDefaultInitExpr`.
    expr: ExprId,
    /// The parameter or field.
    decl: DeclId,
    /// Where it is used ("in the default initializer of").
    loc: Loc,
}

/// The per-full-expression state, saved and cleared where a nested
/// function body or full-expression begins (lambda bodies, statement
/// expressions, implicit definitions, instantiations).
#[derive(Debug, Default)]
pub(crate) struct ImmPending {
    /// The `ConstantExpr` wrappers of the immediate invocations built,
    /// with the function each calls.
    candidates: Vec<(ExprId, DeclId)>,
    /// `DeclRefExpr`s naming an immediate function, not (yet) known to
    /// be the callee of an immediate invocation.
    refs: Vec<ExprId>,
    /// Depth of manifestly constant-evaluated contexts ([expr.const]/22)
    /// entered by the parser.
    const_eval: u32,
    /// The variable whose initializer the next full-expression is.
    var_init: Option<DeclId>,
    /// Its failed invocations are reported after its constant
    /// initialization is checked ([`Sema::report_deferred_immediates`]).
    var_defer: bool,
    /// The `constexpr` or `constinit` variable whose instantiated
    /// initializer is being substituted: its shared failed invocations
    /// ([`Sema::recheck_shared_invocation`]) wait for its check, as
    /// [`ImmPending::var_defer`]'s do.
    shared_var: Option<DeclId>,
    /// The next full-expression is a default argument or default member
    /// initializer: its invocations are not checked here.
    default_init: bool,
    /// The default arguments and default member initializers used.
    default_uses: Vec<DefaultUse>,
    /// The next full-expression is the mem-initializer of this member,
    /// written or not (an escalation names it, at the constructor, as
    /// clang's `DiagnoseImmediateEscalatingReason` does).
    member_init: Option<(DeclId, bool)>,
}

/// [`Sema`]'s immediate-invocation state.
#[derive(Debug, Default)]
pub(crate) struct ImmState {
    pending: ImmPending,
    /// The escalated functions and why.
    reasons: HashMap<DeclId, Escalation>,
    /// The failed immediate invocations of a `constexpr` or `constinit`
    /// variable's initializer, with the variable and the callee, to be
    /// reported after the variable's check; `true` for one of a
    /// template definition reported again ([`Sema::recheck_shared_invocation`]).
    after_var: Vec<(DeclId, ExprId, DeclId, bool)>,
}

/// The pending state of an enclosing full-expression set aside by
/// [`Sema::set_aside_outer_immediates`].
#[derive(Debug, Default)]
pub(crate) struct OuterImm {
    candidates: Vec<(ExprId, DeclId)>,
    refs: Vec<ExprId>,
    default_uses: Vec<DefaultUse>,
}

/// The pending state taken by [`Sema::take_immediate_state`].
#[derive(Debug, Default)]
pub(crate) struct SavedImm {
    pending: ImmPending,
    immediate: Option<u32>,
}

impl Sema<'_, '_> {
    /// Whether `f` is an immediate function: `consteval`, or escalated
    /// ([expr.const]/20).
    pub(crate) fn is_immediate_fn(&self, f: DeclId) -> bool {
        self.ast.decls.function(f).is_some_and(|x| {
            x.spec.constexpr_kind() == ConstexprKind::Consteval || x.spec.escalated()
        })
    }

    /// Whether `f` is an immediate-escalating function ([expr.const]/18):
    /// a lambda call operator, a defaulted special member function, or a
    /// specialization of a templated constexpr function, none declared
    /// `consteval`.
    pub(crate) fn is_immediate_escalating(&self, f: DeclId) -> bool {
        let Some(x) = self.ast.decls.function(f) else { return false };
        match x.spec.constexpr_kind() {
            ConstexprKind::Consteval => false,
            _ if self.is_lambda_call_op(f) => true,
            _ if x.spec.defaulted() != Defaulted::No => true,
            ConstexprKind::Constexpr => {
                self.ast.spec_info(f).is_some_and(|(_, s)| {
                    matches!(
                        s.kind,
                        SpecKind::ImplicitInstantiation
                            | SpecKind::ExplicitInstantiationDeclaration
                            | SpecKind::ExplicitInstantiationDefinition
                    )
                }) || self.ast.member_spec_info(f).is_some()
                    || self.ast.templates.instantiated_from_map(f).is_some()
                    // A function template being defined (its pattern); not
                    // a member of a class template's pattern (clang's
                    // `TK_NonTemplate`), whose instantiations escalate.
                    || self.ast.described_template(self.ast.first(f)).is_some()
            }
            ConstexprKind::None => false,
        }
    }

    /// Whether the expression being built is in an immediate function
    /// context ([expr.const]/16), or otherwise exempt from the checks
    /// here: unevaluated, or in a manifestly constant-evaluated
    /// expression. A template definition is checked like clang does: its
    /// non-dependent invocations now (an instantiation shares them),
    /// the dependent ones at instantiation.
    pub(crate) fn immediate_exempt(&self) -> bool {
        self.unevaluated > 0
            || self.immediate > 0
            || self.imm.pending.const_eval > 0
            || self.func.as_ref().is_some_and(|s| self.is_consteval_kind(s.decl))
    }

    /// Enters a manifestly constant-evaluated context ([expr.const]/22):
    /// a constant-expression (`static_assert`, an array bound, a template
    /// argument, `if constexpr`). Its invocations are not wrapped.
    pub fn enter_constant_evaluated(&mut self) {
        self.imm.pending.const_eval += 1;
        self.cleanup_marks.push(self.cleanup);
    }

    /// Whether the expression being built is in a context entered with
    /// [`Sema::enter_constant_evaluated`].
    pub(crate) fn in_constant_evaluated(&self) -> bool {
        self.imm.pending.const_eval > 0
    }

    /// Leaves a context entered with [`Sema::enter_constant_evaluated`];
    /// `e` is the constant-expression parsed in it, if any.
    pub fn exit_constant_evaluated(&mut self, e: Option<ExprId>) {
        self.imm.pending.const_eval = self.imm.pending.const_eval.saturating_sub(1);
        let outer = self.cleanup_marks.pop().unwrap_or_default();
        if let Some(e) = e {
            self.note_constant_outer_cleanups(e, outer);
        }
    }

    /// Records `outer`, the cleanup state before constant-expression `e`
    /// was built, for [`Sema::act_on_constant_full_expr`] (clang's
    /// `ConstantEvaluated` context saving `ParentCleanup`). Only an outer
    /// full-expression with pending cleanups needs it.
    pub(crate) fn note_constant_outer_cleanups(&mut self, e: ExprId, outer: CleanupState) {
        if outer.needs {
            self.constant_outer_cleanups.insert(e, outer);
        }
    }

    /// The outer cleanup state recorded for a constant-expression inside
    /// `e` (which may be its converted form), removed from the table.
    pub(crate) fn take_constant_outer_cleanups(&mut self, e: ExprId) -> Option<CleanupState> {
        if self.constant_outer_cleanups.is_empty() {
            return None;
        }
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            if let Some(s) = self.constant_outer_cleanups.remove(&x) {
                return Some(s);
            }
            match self.ast.exprs.get(x).kind() {
                ExprKind::ExprWithCleanups | ExprKind::LambdaExpr => {}
                _ => self.ast.exprs.children_into(x, &self.ast.lists, &mut stack),
            }
        }
        None
    }

    /// Takes the pending state where a nested body or full-expression
    /// begins; `reset_context` also leaves an enclosing `if consteval`
    /// (a function body is not in its compound statement).
    pub(crate) fn take_immediate_state(&mut self, reset_context: bool) -> SavedImm {
        let mut pending = std::mem::take(&mut self.imm.pending);
        if !reset_context {
            self.imm.pending.const_eval = pending.const_eval;
            pending.const_eval = 0;
        }
        let immediate = reset_context.then(|| std::mem::take(&mut self.immediate));
        SavedImm { pending, immediate }
    }

    /// Restores the state taken by [`Sema::take_immediate_state`]; what
    /// the nested part left unfinished joins the enclosing
    /// full-expression.
    pub(crate) fn restore_immediate_state(&mut self, saved: SavedImm) {
        let SavedImm { mut pending, immediate } = saved;
        let inner = std::mem::take(&mut self.imm.pending);
        pending.candidates.extend(inner.candidates);
        pending.refs.extend(inner.refs);
        pending.default_uses.extend(inner.default_uses);
        if immediate.is_none() {
            pending.const_eval = inner.const_eval;
        }
        self.imm.pending = pending;
        if let Some(i) = immediate {
            self.immediate = i;
        }
    }

    /// Sets aside the pending immediate invocations, references and
    /// default uses that are not part of constant-expression `e` about
    /// to be finished as a full-expression of its own: they belong to an
    /// enclosing full-expression whose overload resolution substituted
    /// the default template argument `e` (clang pushes a
    /// `ConstantEvaluated` context for it, `SubstDefaultTemplateArgument`).
    pub(crate) fn set_aside_outer_immediates(&mut self, e: ExprId) -> OuterImm {
        let p = &self.imm.pending;
        if p.candidates.is_empty() && p.refs.is_empty() && p.default_uses.is_empty() {
            return OuterImm::default();
        }
        let mut inside = HashSet::new();
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            if inside.len() > 1_000_000 {
                break;
            }
            if inside.insert(x) {
                self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
            }
        }
        let p = &mut self.imm.pending;
        let (keep, candidates) =
            std::mem::take(&mut p.candidates).into_iter().partition(|c| inside.contains(&c.0));
        p.candidates = keep;
        let (keep, refs) =
            std::mem::take(&mut p.refs).into_iter().partition(|r| inside.contains(r));
        p.refs = keep;
        let (keep, default_uses) =
            std::mem::take(&mut p.default_uses).into_iter().partition(|u| inside.contains(&u.expr));
        p.default_uses = keep;
        OuterImm { candidates, refs, default_uses }
    }

    /// Returns what [`Sema::set_aside_outer_immediates`] set aside to the
    /// enclosing full-expression.
    pub(crate) fn restore_outer_immediates(&mut self, out: OuterImm) {
        let p = &mut self.imm.pending;
        p.candidates.extend(out.candidates);
        p.refs.extend(out.refs);
        p.default_uses.extend(out.default_uses);
    }

    /// The next full-expression is the initializer of `v`.
    pub(crate) fn note_var_init_full_expr(&mut self, v: DeclId) {
        self.imm.pending.var_init = Some(v);
    }

    /// [`Sema::note_var_init_full_expr`], with the failed invocations of
    /// a `constexpr` or `constinit` `v` reported only once its constant
    /// initialization has been checked ([`Sema::report_deferred_immediates`]):
    /// clang evaluates the immediate invocations of a declaration's
    /// initializer after `CheckCompleteVariableDeclaration`, so the
    /// variable's own error comes first and a read of `v` in an invocation
    /// sees its failed initializer.
    pub(crate) fn note_checked_var_init_full_expr(&mut self, v: DeclId) {
        self.imm.pending.var_init = Some(v);
        self.imm.pending.var_defer = true;
    }

    /// Reports the failed immediate invocations of `v`'s initializer set
    /// aside by [`Sema::finish_immediate_invocations`], evaluating each
    /// again (`v` now has its initializer and its evaluation state).
    ///
    /// None is reported when the initializer as a whole was just found to
    /// be a constant initializer (`constexpr` `v` has a value, or
    /// `constinit_ok`): that evaluation covered the invocations, whose
    /// results need not be permitted results of their own — the heap a
    /// consteval prvalue owns is freed at the end of the full-expression
    /// (clang's `CheckCompleteVariableDeclaration` evaluates the
    /// initializer whole and leaves the wrappers unvalued; spec/sema.md
    /// "Immediate invocations").
    pub(crate) fn report_deferred_immediates(&mut self, v: DeclId, constinit_ok: bool) {
        if self.imm.after_var.is_empty() {
            return;
        }
        let all = std::mem::take(&mut self.imm.after_var);
        let (mine, rest): (Vec<_>, Vec<_>) = all.into_iter().partition(|&(w, ..)| w == v);
        self.imm.after_var = rest;
        if mine.is_empty() {
            return;
        }
        let has_value = self.ast.decls.var(v).is_some_and(|x| {
            x.spec.is_constexpr() && matches!(x.eval.state(), VarEvalState::Value(_))
        });
        if has_value || constinit_ok {
            return;
        }
        for (_, c, callee, again) in mine {
            match self.eval_immediate(c) {
                // A shared wrapper is the template definition's.
                Ok(val) if !again => {
                    let _ = self.ast.exprs.set_constant_value(c, val);
                }
                Ok(_) => {}
                Err(fail) => self.report_immediate_call(c, callee, fail, None, again),
            }
        }
    }

    /// The initializer of instantiated variable `v` is being substituted
    /// (`Some`) or done (`None`); see [`ImmPending::shared_var`]. Returns
    /// the previous value, which the caller restores when it is done (a
    /// variable template instantiated inside another one's initializer).
    pub(crate) fn note_var_init_subst(&mut self, v: Option<DeclId>) -> Option<DeclId> {
        let v = v.filter(|&v| self.checked_var(v));
        std::mem::replace(&mut self.imm.pending.shared_var, v)
    }

    /// The variable whose initializer the next full-expression is, if
    /// it is one.
    pub(crate) fn pending_var_init(&self) -> Option<DeclId> {
        self.imm.pending.var_init
    }

    /// The next full-expression is the mem-initializer of member `f`,
    /// `written` or implicit.
    pub(crate) fn note_member_init_full_expr(&mut self, f: DeclId, written: bool) {
        self.imm.pending.member_init = Some((f, written));
    }

    /// The next full-expression is a default argument or a default
    /// member initializer: its immediate invocations are checked where it
    /// is used ([`Sema::note_default_use`]), not here.
    pub(crate) fn note_default_init_full_expr(&mut self) {
        self.imm.pending.default_init = true;
    }

    /// The use `e` (a `CXXDefaultArgExpr` or a `CXXDefaultInitExpr` in an
    /// aggregate initialization) of `decl`'s default argument or default
    /// member initializer at `loc`: its immediate invocations are
    /// evaluated at the end of the enclosing full-expression, unless that
    /// is exempt ([expr.const]/16; clang rebuilds the initializer there).
    pub(crate) fn note_default_use(&mut self, e: ExprId, decl: DeclId, loc: Loc) {
        self.queue_default_arg_uses(decl, loc);
        // A default argument's temporaries belong to the full-expression
        // of the call (clang `CheckCXXDefaultArgExpr`'s
        // `setExprNeedsCleanups`; [intro.execution]/5).
        if let Some(d) = self.ast.decls.param(decl).and_then(|p| p.default_arg)
            && self.ast.exprs.get(d).kind() == ExprKind::ExprWithCleanups
        {
            self.cleanup.needs = true;
            self.cleanup.side_effects |= self.ast.exprs.cleanups_have_side_effects(d);
        }
        if self.immediate_exempt() || !self.default_has_invocation(decl) {
            return;
        }
        self.imm.pending.default_uses.push(DefaultUse { expr: e, decl, loc });
    }

    /// Whether `decl`'s default argument or default member initializer
    /// holds an immediate invocation of its own.
    fn default_has_invocation(&self, decl: DeclId) -> bool {
        let mut found = Vec::new();
        self.default_invocations(decl, &mut found);
        !found.is_empty()
    }

    /// The default argument of parameter `decl`, or the default member
    /// initializer of field `decl`.
    fn default_root(&self, decl: DeclId) -> Option<ExprId> {
        self.ast
            .decls
            .param(decl)
            .and_then(|p| p.default_arg)
            .or_else(|| self.ast.decls.field(decl).and_then(|f| f.init))
    }

    /// The outermost immediate invocations of `decl`'s default argument
    /// or default member initializer, not yet evaluated; those in an
    /// unevaluated operand or a lambda body are not, nor those of the
    /// default arguments and initializers it uses itself (checked where
    /// it is defined, like clang).
    fn default_invocations(&self, decl: DeclId, out: &mut Vec<ExprId>) {
        let Some(root) = self.default_root(decl) else { return };
        let mut stack = vec![root];
        let mut seen = 0usize;
        while let Some(x) = stack.pop() {
            seen += 1;
            if seen > 1_000_000 {
                break;
            }
            match self.ast.exprs.get(x).kind() {
                ExprKind::ConstantExpr => {
                    if self.ast.exprs.constant_value(x).is_none()
                        && !self.expr_is_value_dependent(x)
                    {
                        out.push(x);
                    }
                }
                ExprKind::CXXDefaultArgExpr
                | ExprKind::CXXDefaultInitExpr
                | ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::ExpressionTraitExpr
                | ExprKind::RequiresExpr
                | ExprKind::ConceptSpecializationExpr
                | ExprKind::LambdaExpr
                | ExprKind::CXXTypeidExpr => {}
                _ => {
                    let kids = self.ast.exprs.children(x, &self.ast.lists);
                    stack.extend(kids.into_iter().rev());
                }
            }
        }
    }

    /// The immediate invocations of `e`, an expression built before the
    /// full-expression that uses it (an enumerating expansion element,
    /// built with its expansion-init-list before its declaration,
    /// [stmt.expand]/5.1): those not yet evaluated are evaluated at the
    /// end of the current full-expression ([expr.const]/16).
    pub(crate) fn note_deferred_invocations(&mut self, e: ExprId) {
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            if self.ast.exprs.get(x).kind() == ExprKind::ConstantExpr
                && self.ast.exprs.constant_value(x).is_none()
                && !self.ast.exprs.constant_elided(x)
                && !self.imm.pending.candidates.iter().any(|&(c, _)| c == x)
                && let Some(f) = self.invocation_callee(x).filter(|&f| self.is_immediate_fn(f))
            {
                self.imm.pending.candidates.push((x, f));
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
    }

    /// An immediate invocation `c` of a template definition that the
    /// instantiation being performed shares with it
    /// (`templ/subst/shared.rs`): one that failed at the definition is
    /// evaluated again and reported with the instantiation's context, as
    /// clang reports its rebuilt `ConstantExpr` ([expr.const]/16).
    /// One that made the pattern immediate makes an immediate-escalating
    /// instantiation immediate.
    pub(crate) fn recheck_shared_invocation(&mut self, c: ExprId) {
        if self.ast.exprs.constant_value(c).is_some()
            || (self.ast.exprs.constant_elided(c) && !self.marked_escalating(c))
            || self.immediate_exempt()
            || self.has_errors_in(c)
        {
            return;
        }
        let Some(f) = self.invocation_callee(c).filter(|&f| self.is_immediate_fn(f)) else {
            return;
        };
        let at = self.begin_loc(c);
        let r =
            self.eval_const(self.ast.exprs.operand(c), EvalContext::new(EvalKind::Consteval, at));
        let Err(fail) = r else { return };
        let escalating =
            self.func.as_ref().map(|s| s.decl).filter(|&g| self.is_immediate_escalating(g));
        if let Some(v) = self.imm.pending.shared_var
            && escalating.is_none()
        {
            self.imm.after_var.push((v, c, f, true));
            return;
        }
        match escalating {
            Some(g) => {
                let why = Escalation {
                    loc: at,
                    callee: f,
                    call: true,
                    default_init: None,
                    written: false,
                    consteval_only: None,
                };
                self.escalate(g, why);
            }
            None => self.report_immediate_call(c, f, fail, None, true),
        }
    }

    /// A reference `r` to an immediate function of a template definition
    /// that the instantiation being performed shares with it
    /// (`templ/subst/shared.rs`), not the callee of an invocation: one that
    /// made the pattern immediate (its address taken outside an immediate
    /// invocation) makes an immediate-escalating instantiation immediate
    /// at clang 21+ (P2564; clang 19 keeps the specialization
    /// non-immediate and does not diagnose it).
    pub(crate) fn recheck_shared_ref(&mut self, r: ExprId) {
        if self.clang_compat < 22
            || !self.ast.exprs.get(r).has(ExprFlags::IMMEDIATE_ESCALATING)
            || self.immediate_exempt()
        {
            return;
        }
        let d = self.ast.exprs.decl(r);
        if !self.is_immediate_fn(d) {
            return;
        }
        if let Some(g) =
            self.func.as_ref().map(|s| s.decl).filter(|&g| self.is_immediate_escalating(g))
        {
            let why = Escalation {
                loc: self.begin_loc(r),
                callee: d,
                call: false,
                default_init: None,
                written: false,
                consteval_only: None,
            };
            self.escalate(g, why);
        }
    }

    /// The callee `DeclRefExpr` of the immediate invocation `c`, if any.
    pub(crate) fn invocation_callee_ref(&self, c: ExprId) -> Option<ExprId> {
        let e = self.ignore_implicit_for_imm(self.ast.exprs.operand(c));
        match self.ast.exprs.get(e).kind() {
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr | ExprKind::CXXMemberCallExpr => {
                let r = self.ignore_implicit_for_imm(self.ast.exprs.callee(e));
                (self.ast.exprs.get(r).kind() == ExprKind::DeclRefExpr).then_some(r)
            }
            _ => None,
        }
    }

    /// Whether the immediate invocation `c` is marked as the one that
    /// made its function immediate ([`Sema::mark_immediate_escalating`]).
    fn marked_escalating(&self, c: ExprId) -> bool {
        let e = self.ignore_implicit_for_imm(self.ast.exprs.operand(c));
        match self.ast.exprs.get(e).kind() {
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                self.ast.exprs.construct_info(e).immediate_escalating
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr | ExprKind::CXXMemberCallExpr => {
                let r = self.ignore_implicit_for_imm(self.ast.exprs.callee(e));
                self.ast.exprs.get(r).kind() == ExprKind::DeclRefExpr
                    && self.ast.exprs.get(r).has(ExprFlags::IMMEDIATE_ESCALATING)
            }
            _ => false,
        }
    }

    /// The immediate function an immediate invocation `c` calls.
    fn invocation_callee(&self, c: ExprId) -> Option<DeclId> {
        let e = self.ignore_implicit_for_imm(self.ast.exprs.operand(c));
        match self.ast.exprs.get(e).kind() {
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                Some(self.ast.exprs.constructor(e))
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr | ExprKind::CXXMemberCallExpr => {
                let r = self.ignore_implicit_for_imm(self.ast.exprs.callee(e));
                matches!(self.ast.exprs.get(r).kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr)
                    .then(|| self.ast.exprs.decl(r))
            }
            _ => None,
        }
    }

    /// Evaluates the immediate invocations of `decl`'s default argument
    /// or default member initializer used at `loc` ([expr.const]/16):
    /// one that is not a constant expression makes the
    /// immediate-escalating function `escalating` immediate, else it is
    /// reported "in the default initializer of" `decl`. A constructor
    /// using a default member initializer passes itself as `ctor`.
    pub(crate) fn check_default_use(
        &mut self,
        decl: DeclId,
        loc: Loc,
        escalating: Option<DeclId>,
        ctor: bool,
    ) {
        let mut found = Vec::new();
        self.default_invocations(decl, &mut found);
        for c in found {
            let at = self.begin_loc(c);
            let r = self
                .eval_const(self.ast.exprs.operand(c), EvalContext::new(EvalKind::Consteval, at));
            let Err(fail) = r else { continue };
            let Some(callee) = self.invocation_callee(c) else { continue };
            match escalating {
                Some(g) => {
                    let why = if ctor {
                        let loc = self.ast.decls.get(g).loc();
                        Escalation {
                            loc,
                            callee,
                            call: true,
                            default_init: Some(decl),
                            written: false,
                            consteval_only: None,
                        }
                    } else {
                        Escalation {
                            loc: at,
                            callee,
                            call: true,
                            default_init: None,
                            written: false,
                            consteval_only: None,
                        }
                    };
                    self.escalate(g, why);
                    return;
                }
                None => self.report_immediate_call(c, callee, fail, Some((decl, loc)), false),
            }
        }
    }

    /// A reference `e` to function `d`: recorded when `d` is immediate
    /// and `e` is not exempt ([expr.const]/17).
    pub(crate) fn note_immediate_ref(&mut self, e: ExprId, d: DeclId) {
        // A function in a dependent context (a member of a class
        // template, a template pattern) is not recorded: its references
        // are checked when instantiated (clang `MarkDeclRefReferenced`,
        // `!FD->isDependentContext()`), so a dependent call of it in the
        // pattern, never formed as an invocation there, is not mistaken
        // for taking its address.
        if self.is_immediate_fn(d)
            && !self.ast.decls.get(d).has(DeclFlags::DEPENDENT_CTX)
            && self.ast.described_template(d).is_none()
            && !self.immediate_exempt()
            && !self.imm.pending.refs.contains(&e)
        {
            self.imm.pending.refs.push(e);
        }
    }

    /// `e`, recorded by [`Sema::note_immediate_ref`], is the callee of a
    /// call after all: not a reference to check.
    pub(crate) fn forget_immediate_ref(&mut self, e: ExprId) {
        if let Some(i) = self.imm.pending.refs.iter().rposition(|&x| x == e) {
            self.imm.pending.refs.remove(i);
        }
    }

    /// Records `e` for the immediate checks at the end of the current
    /// full-expression (a consteval-only leaf, `consteval_only.rs`; a
    /// fresh node, so not searched for: a full-expression may hold many).
    pub(crate) fn note_pending_ref(&mut self, e: ExprId) {
        self.imm.pending.refs.push(e);
    }

    /// The call `call` of function `f` (named by `callee_ref`, when it
    /// is a `DeclRefExpr`): an immediate invocation outside an immediate
    /// function context is wrapped in a `ConstantExpr` evaluated at the
    /// end of the full-expression ([expr.const]/16).
    pub(crate) fn check_immediate_invocation(
        &mut self,
        call: ExprId,
        f: DeclId,
        callee_ref: Option<ExprId>,
    ) -> ExprId {
        if !self.is_immediate_fn(f) {
            return call;
        }
        if let Some(r) = callee_ref {
            self.forget_immediate_ref(r);
        }
        if self.immediate_exempt() || self.has_errors_in(call) || self.expr_is_value_dependent(call)
        {
            return call;
        }
        let w = self.builder().constant_expr(call, None);
        self.imm.pending.candidates.push((w, f));
        w
    }

    /// The end of a full-expression kept as the separate expressions
    /// `parts` (the arguments of a mem-initializer of a template, which
    /// is not built until instantiation): the immediate invocations of
    /// each part are finished like those of a full-expression.
    pub(crate) fn finish_immediate_parts(&mut self, parts: &[ExprId]) {
        let candidates = std::mem::take(&mut self.imm.pending.candidates);
        let refs = std::mem::take(&mut self.imm.pending.refs);
        let uses = std::mem::take(&mut self.imm.pending.default_uses);
        for &e in parts {
            self.imm.pending.candidates.clone_from(&candidates);
            self.imm.pending.refs.clone_from(&refs);
            self.imm.pending.default_uses.clone_from(&uses);
            self.finish_immediate_invocations(e);
        }
        self.imm.pending.candidates.clear();
        self.imm.pending.refs.clear();
        self.imm.pending.default_uses.clear();
    }

    /// Evaluates immediate invocation `c` ([expr.const]/25).
    fn eval_immediate(&mut self, c: ExprId) -> Result<ConstValueId, EvalFailure> {
        let at = self.begin_loc(c);
        self.eval_const(self.ast.exprs.operand(c), EvalContext::new(EvalKind::Consteval, at))
    }

    /// Whether `v` is constexpr or constinit: its initializer is checked as
    /// a whole before its immediate invocations are reported (WP-465).
    fn checked_var(&self, v: DeclId) -> bool {
        self.ast.decls.var(v).is_some_and(|x| x.spec.is_constexpr())
            || self.constinit_of(v).is_some()
    }

    /// The end of full-expression `e` (from [`Sema::act_on_full_expr`]):
    /// evaluates its immediate invocations and checks the references to
    /// immediate functions ([expr.const]/16-19).
    pub(crate) fn finish_immediate_invocations(&mut self, e: ExprId) {
        let var = self.imm.pending.var_init.take();
        let var_defer = std::mem::take(&mut self.imm.pending.var_defer);
        let member_init = self.imm.pending.member_init.take();
        let default_init = std::mem::take(&mut self.imm.pending.default_init);
        let uses = std::mem::take(&mut self.imm.pending.default_uses);
        if self.imm.pending.candidates.is_empty()
            && self.imm.pending.refs.is_empty()
            && uses.is_empty()
        {
            return;
        }
        let candidates = std::mem::take(&mut self.imm.pending.candidates);
        let refs = std::mem::take(&mut self.imm.pending.refs);
        if default_init {
            // Its invocations are evaluated where it is used; those of the
            // default arguments and initializers it uses, here (clang
            // checks them as it builds it), unless they are part of one.
            let (_, _, _, uses) = self.classify_immediate(e, &candidates, &[], &uses);
            for (c, _) in candidates {
                self.ast.exprs.set_constant_elided(c);
            }
            for u in uses {
                self.check_default_use(u.decl, u.loc, None, false);
            }
            return;
        }
        let (outer, nested, bad_refs, uses) = self.classify_immediate(e, &candidates, &refs, &uses);
        // An escalating function's body is not an immediate function
        // context even once it has escalated: later invocations are still
        // evaluated (and wrapped) individually.
        let escalating =
            self.func.as_ref().map(|s| s.decl).filter(|&f| self.is_immediate_escalating(f));
        let report_after =
            var.filter(|&v| var_defer && escalating.is_none() && self.checked_var(v));
        let results: Vec<_> = outer.iter().map(|&(c, _)| self.eval_immediate(c)).collect();
        // A constant-initialized variable's initializer (clang evaluates
        // it as a whole): its invocations keep their wrappers, unvalued
        // in clang's dump.
        let deferred = var.is_some_and(|v| self.checks_constant_init(v))
            && results.iter().all(Result::is_ok)
            && self.try_fold(e).is_some();
        // One that is not a permitted result of its own (a prvalue owning
        // heap the full-expression frees) is validated by that check
        // instead: a constexpr or constinit variable's after it
        // ([`Sema::report_deferred_immediates`]), another's here.
        let validated = report_after.is_none()
            && escalating.is_none()
            && results.iter().any(Result::is_err)
            && var.is_some_and(|v| self.checks_constant_init(v))
            && self.try_fold(e).is_some();
        for (&(c, callee), r) in outer.iter().zip(results) {
            match r {
                Ok(v) => {
                    let _ = self.ast.exprs.set_constant_value(c, v);
                    if deferred {
                        self.ast.exprs.set_constant_value_hidden(c);
                    }
                }
                Err(fail) => match escalating {
                    Some(g) => {
                        self.ast.exprs.set_constant_elided(c);
                        self.mark_immediate_escalating(self.ast.exprs.operand(c));
                        let (loc, field, written) = match member_init {
                            Some((f, w)) => (self.ast.decls.get(g).loc(), Some(f), w),
                            None => (self.begin_loc(c), None, false),
                        };
                        let why = Escalation {
                            loc,
                            callee,
                            call: true,
                            default_init: field,
                            written,
                            consteval_only: None,
                        };
                        self.escalate(g, why);
                    }
                    None => match report_after {
                        Some(v) => self.imm.after_var.push((v, c, callee, false)),
                        None if validated => {}
                        None => self.report_immediate_call(c, callee, fail, None, false),
                    },
                },
            }
        }
        if !deferred {
            for c in nested {
                self.ast.exprs.set_constant_elided(c);
            }
        }
        // Consteval-only expressions (spec/reflection-repr.md §6).
        let bad_refs = self.check_consteval_only_leaves(bad_refs, e, var, escalating);
        self.check_immediate_refs(bad_refs, escalating);
        for u in uses {
            self.check_default_use(u.decl, u.loc, escalating, false);
        }
    }

    /// The references to immediate functions outside every immediate
    /// invocation ([expr.const]/18): each escalates `escalating`, or is
    /// an error.
    fn check_immediate_refs(&mut self, bad_refs: Vec<ExprId>, escalating: Option<DeclId>) {
        for r in bad_refs {
            // Already immediate-escalating (an instantiation's copy of a
            // pattern's reference): clang skips it.
            if self.ast.exprs.get(r).has(ExprFlags::IMMEDIATE_ESCALATING) {
                continue;
            }
            let d = self.ast.exprs.decl(r);
            let loc = self.begin_loc(r);
            if let Some(g) = escalating {
                self.ast.exprs.set_immediate_escalating(r);
                self.escalate(
                    g,
                    Escalation {
                        loc,
                        callee: d,
                        call: false,
                        default_init: None,
                        written: false,
                        consteval_only: None,
                    },
                );
                continue;
            }
            let consteval = self.is_consteval_kind(d);
            let mut msg = self
                .diag(DiagId::sema_immediate_address, loc)
                .arg(u8::from(consteval))
                .arg(self.unqualified_decl_name(d));
            // A builtin has no declaration to point at.
            if self.ast.decls.builtin_id(d).is_none() {
                msg =
                    msg.with_note(self.diag(DiagId::note_declared_at, self.ast.decls.get(d).loc()));
            }
            if !consteval && let Some(note) = self.escalation_reason_note(d) {
                msg = msg.with_note(note);
            }
            self.emit(msg);
        }
    }

    /// Splits the candidates of `e` into the outermost and the nested
    /// ones, and finds the references and the default uses outside every
    /// candidate (a use inside one is evaluated with it). A candidate,
    /// reference or use not reachable from `e` (built for a discarded
    /// expression) is left alone.
    #[allow(clippy::type_complexity)]
    fn classify_immediate(
        &self,
        e: ExprId,
        candidates: &[(ExprId, DeclId)],
        refs: &[ExprId],
        uses: &[DefaultUse],
    ) -> (Vec<(ExprId, DeclId)>, Vec<ExprId>, Vec<ExprId>, Vec<DefaultUse>) {
        let cset: HashMap<ExprId, DeclId> = candidates.iter().copied().collect();
        let rset: HashSet<ExprId> = refs.iter().copied().collect();
        let use_map: HashMap<ExprId, DefaultUse> = uses.iter().map(|u| (u.expr, *u)).collect();
        let (mut outer, mut nested, mut bad) = (Vec::new(), Vec::new(), Vec::new());
        let mut live = Vec::new();
        let mut stack = vec![(e, false)];
        let mut seen = 0usize;
        while let Some((x, inside)) = stack.pop() {
            seen += 1;
            if seen > 1_000_000 {
                break;
            }
            let callee = cset.get(&x).copied();
            let is_c = callee.is_some();
            if let Some(f) = callee {
                if inside { nested.push(x) } else { outer.push((x, f)) }
            } else if !inside && rset.contains(&x) {
                bad.push(x);
            }
            if !inside && let Some(&u) = use_map.get(&x) {
                live.push(u);
            }
            let kids = self.ast.exprs.children(x, &self.ast.lists);
            stack.extend(kids.into_iter().rev().map(|k| (k, inside || is_c)));
        }
        (outer, nested, bad, live)
    }

    /// Marks the immediate invocation `e` that made the enclosing
    /// function immediate (clang's `MarkExpressionAsImmediateEscalating`):
    /// a construction itself, a call's callee `DeclRefExpr`.
    fn mark_immediate_escalating(&mut self, e: ExprId) {
        let e = self.ignore_implicit_for_imm(e);
        match self.ast.exprs.get(e).kind() {
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                self.ast.exprs.set_immediate_escalating(e);
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr | ExprKind::CXXMemberCallExpr => {
                let callee = self.ast.exprs.callee(e);
                let r = self.ignore_implicit_for_imm(callee);
                if self.ast.exprs.get(r).kind() == ExprKind::DeclRefExpr {
                    self.ast.exprs.set_immediate_escalating(r);
                }
            }
            _ => {}
        }
    }

    /// `e` without the implicit nodes around it (clang's `IgnoreImplicit`
    /// plus parentheses).
    fn ignore_implicit_for_imm(&self, mut e: ExprId) -> ExprId {
        loop {
            let n = self.ast.exprs.get(e);
            match n.kind() {
                ExprKind::ImplicitCastExpr
                | ExprKind::ParenExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ExprWithCleanups => e = self.ast.exprs.operand(e),
                _ => return e,
            }
        }
    }

    pub(crate) fn is_consteval_kind(&self, f: DeclId) -> bool {
        self.ast
            .decls
            .function(f)
            .is_some_and(|x| x.spec.constexpr_kind() == ConstexprKind::Consteval)
    }

    /// Makes the immediate-escalating function `g` an immediate function
    /// ([expr.const]/20) for `why`; the first reason is kept.
    /// The consteval-only expression `r` in immediate-escalating function
    /// `g` makes `g` immediate ([expr.const.imm]/2), with the reason note
    /// `note_immediate_function_reason_consteval_only`
    /// (`consteval_only.rs`, spec/reflection-repr.md §6).
    pub(crate) fn escalate_consteval_only(&mut self, g: DeclId, r: ExprId) {
        let why = Escalation {
            loc: self.begin_loc(r),
            callee: g,
            call: false,
            default_init: None,
            written: false,
            consteval_only: Some(self.ast.exprs.get(r).ty()),
        };
        self.escalate(g, why);
    }

    fn escalate(&mut self, g: DeclId, why: Escalation) {
        self.ast.decls.mark_function(g, FnSpec::new().with_escalated(true));
        self.imm.reasons.entry(g).or_insert(why);
    }

    /// "call to consteval function 'f' is not a constant expression",
    /// with why an escalated function is immediate and the evaluation's
    /// notes (clang's `EvaluateAndDiagnoseImmediateInvocation`).
    /// A default argument or member initializer's invocation names it
    /// and its use `(decl, loc)`.
    fn report_immediate_call(
        &mut self,
        c: ExprId,
        f: DeclId,
        mut fail: EvalFailure,
        default_use: Option<(DeclId, Loc)>,
        again: bool,
    ) {
        let loc = self.begin_loc(c);
        let consteval = self.is_consteval_kind(f);
        let at = Self::fold_lone_invalid(&mut fail, loc);
        let mut msg = self
            .diag(DiagId::sema_immediate_call_non_constant, at)
            .arg(u8::from(consteval))
            .arg(self.qualified_decl_name(f));
        if again {
            msg = msg.allow_duplicate();
        }
        if let Some((d, used)) = default_use {
            let begin = self.ast.decls.cold.begin_loc.get(d).copied();
            let begin = begin.unwrap_or(self.ast.decls.get(d).loc());
            // Reported at the invocation once per use.
            msg = msg
                .allow_duplicate()
                .with_note(
                    self.diag(DiagId::note_in_default_initializer, used)
                        .arg(self.unqualified_decl_name(d)),
                )
                .with_note(self.diag(DiagId::note_declared_at, begin));
        }
        if !consteval && let Some(note) = self.escalation_reason_note(f) {
            msg = msg.with_note(note);
        }
        let msg = Self::with_failure_notes(msg, fail);
        self.emit(msg);
    }

    /// clang `DiagnoseImmediateEscalatingReason`: why the escalated
    /// function `f` is immediate, if it is one.
    fn escalation_reason_note(&self, f: DeclId) -> Option<kiln_diag::Diag> {
        // clang names the reason for a generic lambda's call operator
        // specialization, not for a non-template call operator.
        let plain_call_op = self.is_lambda_call_op(f)
            && !matches!(
                self.ast.templ_link(self.ast.first(f)).get(),
                Some((kiln_ast::decl::TemplLinkKind::Specialization, _))
            );
        if plain_call_op {
            return None;
        }
        let why = self.imm.reasons.get(&f).copied()?;
        // A constructor is named by its class's unqualified name.
        let is_ctor = |d: DeclId| self.ast.decls.get(d).kind() == DeclKind::CXXConstructor;
        let name = |d: DeclId| {
            if is_ctor(d) { self.unqualified_decl_name(d) } else { self.qualified_decl_name(d) }
        };
        if let Some(ty) = why.consteval_only {
            return Some(
                self.diag(DiagId::note_immediate_function_reason_consteval_only, why.loc)
                    .arg(if self.is_lambda_call_op(f) {
                        self.unqualified_decl_name(f)
                    } else {
                        name(f)
                    })
                    .arg(u8::from(is_ctor(f)))
                    .arg(self.ty_arg(ty)),
            );
        }
        Some(match why.default_init {
            Some(field) => self
                .diag(DiagId::note_immediate_default_init_reason, why.loc)
                .arg(self.unqualified_decl_name(f))
                .arg(u8::from(is_ctor(f)))
                .arg(self.unqualified_decl_name(field))
                .arg(u8::from(self.is_consteval_kind(why.callee)))
                .arg(name(why.callee))
                .arg(u8::from(is_ctor(why.callee)))
                .arg(u8::from(!why.written)),
            None => self
                .diag(DiagId::note_immediate_function_reason, why.loc)
                .arg(if self.is_lambda_call_op(f) {
                    self.unqualified_decl_name(f)
                } else {
                    name(f)
                })
                .arg(u8::from(why.call))
                .arg(u8::from(self.is_consteval_kind(why.callee)))
                .arg(name(why.callee))
                .arg(u8::from(is_ctor(f)))
                .arg(u8::from(is_ctor(why.callee))),
        })
    }

    /// Whether `v`'s initializer is checked for constant initialization
    /// when it is declared (clang's `CheckCompleteVariableDeclaration`):
    /// `constexpr`, static or thread storage duration, or a `const`
    /// non-volatile integral or enumeration local.
    pub(crate) fn checks_constant_init(&self, v: DeclId) -> bool {
        let Some(var) = self.ast.decls.var(v) else { return false };
        if var.spec.is_constexpr() || self.has_static_storage(v) {
            return true;
        }
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        let q = self.ast.types.effective_quals(ty);
        q.is_const() && !q.is_volatile() && self.is_integral_or_enum(ty)
    }
}
