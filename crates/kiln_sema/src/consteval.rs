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

use kiln_ast::decl::{ConstexprKind, DeclKind, Defaulted, FnSpec};
use kiln_ast::expr::{ExprFlags, ExprKind};
use kiln_ast::templates::SpecKind;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;

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
    /// The field whose default member initializer holds the call (a
    /// constructor using it; `loc` is the constructor).
    default_init: Option<DeclId>,
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
    /// The next full-expression is a default argument or default member
    /// initializer: its invocations are not checked here.
    default_init: bool,
    /// The default arguments and default member initializers used.
    default_uses: Vec<DefaultUse>,
}

/// [`Sema`]'s immediate-invocation state.
#[derive(Debug, Default)]
pub(crate) struct ImmState {
    pending: ImmPending,
    /// The escalated functions and why.
    reasons: HashMap<DeclId, Escalation>,
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
                    // A templated function being defined (its pattern).
                    || self.in_template()
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
    fn immediate_exempt(&self) -> bool {
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
    }

    /// Leaves a context entered with [`Sema::enter_constant_evaluated`].
    pub fn exit_constant_evaluated(&mut self) {
        self.imm.pending.const_eval = self.imm.pending.const_eval.saturating_sub(1);
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

    /// The next full-expression is the initializer of `v`.
    pub(crate) fn note_var_init_full_expr(&mut self, v: DeclId) {
        self.imm.pending.var_init = Some(v);
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
                        Escalation { loc, callee, call: true, default_init: Some(decl) }
                    } else {
                        Escalation { loc: at, callee, call: true, default_init: None }
                    };
                    self.escalate(g, why);
                    return;
                }
                None => self.report_immediate_call(c, callee, fail, Some((decl, loc))),
            }
        }
    }

    /// A reference `e` to function `d`: recorded when `d` is immediate
    /// and `e` is not exempt ([expr.const]/17).
    pub(crate) fn note_immediate_ref(&mut self, e: ExprId, d: DeclId) {
        if self.is_immediate_fn(d)
            && !self.immediate_exempt()
            && !self.imm.pending.refs.contains(&e)
        {
            self.imm.pending.refs.push(e);
        }
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
        if let Some(r) = callee_ref
            && let Some(i) = self.imm.pending.refs.iter().rposition(|&x| x == r)
        {
            self.imm.pending.refs.remove(i);
        }
        if self.immediate_exempt() || self.has_errors_in(call) || self.expr_is_value_dependent(call)
        {
            return call;
        }
        let w = self.builder().constant_expr(call, None);
        self.imm.pending.candidates.push((w, f));
        w
    }

    /// The end of full-expression `e` (from [`Sema::act_on_full_expr`]):
    /// evaluates its immediate invocations and checks the references to
    /// immediate functions ([expr.const]/16-19).
    pub(crate) fn finish_immediate_invocations(&mut self, e: ExprId) {
        let var = self.imm.pending.var_init.take();
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
        let mut results = Vec::with_capacity(outer.len());
        for &(c, _) in &outer {
            let at = self.begin_loc(c);
            let r = self
                .eval_const(self.ast.exprs.operand(c), EvalContext::new(EvalKind::Consteval, at));
            results.push(r);
        }
        // A constant-initialized variable's initializer (clang evaluates
        // it as a whole): its invocations keep their wrappers, unvalued
        // in clang's dump.
        let deferred = var.is_some_and(|v| self.checks_constant_init(v))
            && results.iter().all(Result::is_ok)
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
                        let loc = self.begin_loc(c);
                        self.escalate(
                            g,
                            Escalation { loc, callee, call: true, default_init: None },
                        );
                    }
                    None => self.report_immediate_call(c, callee, fail, None),
                },
            }
        }
        if !deferred {
            for c in nested {
                self.ast.exprs.set_constant_elided(c);
            }
        }
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
                self.escalate(g, Escalation { loc, callee: d, call: false, default_init: None });
                continue;
            }
            let consteval = self.is_consteval_kind(d);
            let msg = self
                .diag(DiagId::sema_immediate_address, loc)
                .arg(u8::from(consteval))
                .arg(self.unqualified_decl_name(d))
                .with_note(self.diag(DiagId::note_declared_at, self.ast.decls.get(d).loc()));
            self.emit(msg);
        }
        for u in uses {
            self.check_default_use(u.decl, u.loc, escalating, false);
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
        mut fail: kiln_oven::EvalFailure,
        default_use: Option<(DeclId, Loc)>,
    ) {
        let loc = self.begin_loc(c);
        let consteval = self.is_consteval_kind(f);
        let at = Self::fold_lone_invalid(&mut fail, loc);
        let mut msg = self
            .diag(DiagId::sema_immediate_call_non_constant, at)
            .arg(u8::from(consteval))
            .arg(self.qualified_decl_name(f));
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
        if !consteval
            && !self.is_lambda_call_op(f)
            && let Some(why) = self.imm.reasons.get(&f).copied()
        {
            let note = match why.default_init {
                Some(field) => self
                    .diag(DiagId::note_immediate_default_init_reason, why.loc)
                    .arg(self.unqualified_decl_name(f))
                    .arg(u8::from(self.ast.decls.get(f).kind() == DeclKind::CXXConstructor))
                    .arg(self.unqualified_decl_name(field))
                    .arg(u8::from(self.is_consteval_kind(why.callee)))
                    .arg(self.qualified_decl_name(why.callee)),
                None => self
                    .diag(DiagId::note_immediate_function_reason, why.loc)
                    .arg(self.qualified_decl_name(f))
                    .arg(u8::from(why.call))
                    .arg(u8::from(self.is_consteval_kind(why.callee)))
                    .arg(self.qualified_decl_name(why.callee)),
            };
            msg = msg.with_note(note);
        }
        let msg = Self::with_failure_notes(msg, fail);
        self.emit(msg);
    }

    /// Whether `v`'s initializer is checked for constant initialization
    /// when it is declared (clang's `CheckCompleteVariableDeclaration`):
    /// `constexpr`, static or thread storage duration, or a `const`
    /// non-volatile integral or enumeration local.
    fn checks_constant_init(&self, v: DeclId) -> bool {
        let Some(var) = self.ast.decls.var(v) else { return false };
        if var.spec.is_constexpr() || self.has_static_storage(v) {
            return true;
        }
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        let q = self.ast.types.effective_quals(ty);
        q.is_const() && !q.is_volatile() && self.is_integral_or_enum(ty)
    }
}
