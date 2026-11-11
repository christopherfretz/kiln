//! Statements ([stmt]): compound statements and their scopes, selection
//! and iteration statements, `case`/`default`, jumps, labels and `goto`,
//! `return` (spec/sema.md §7).

use kiln_ast::consts::Scalar;
use kiln_ast::decl::{DeclFlags, DeclKind, DeclMisc, NewDecl, Payload};
use kiln_ast::expr::{BinaryOp, CastKind, ExprDeps, ExprKind, UnaryOp, ValueCategory};
use kiln_ast::ids::RawId;
use kiln_ast::names::DeclName;
use kiln_ast::stmt::{CaseData, IfData, IfKind, LoopData, StmtKind, SwitchData};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use std::collections::HashSet;

use crate::Sema;
use crate::conv::{self, ConvCtx, SecondStep, StandardConversion};
use crate::overload::Ics;
use crate::scope::{Scope, ScopeKind};
use crate::templ::stack::FrameKind;

/// A label of the current function.
#[derive(Clone, Debug)]
pub(crate) struct LabelInfo {
    pub name: Symbol,
    pub decl: DeclId,
    /// Where it was defined (`None` while only used by `goto`).
    pub defined: Option<Loc>,
    /// The first `goto` naming it.
    pub first_use: Option<Loc>,
    /// A GNU local label (`__label__ name;`), declared at this location:
    /// found only from its block ([`Scope::labels`]).
    pub local: Option<Loc>,
}

/// One enclosing `switch`.
#[derive(Clone, Debug)]
pub(crate) struct SwitchState {
    /// The promoted condition type.
    pub cond_ty: QualType,
    /// Case values seen, with their locations.
    pub values: Vec<(i128, Loc)>,
    /// GNU case ranges `lo ... hi` seen, with their `lo` and `hi`
    /// expressions (checked when the switch ends).
    pub ranges: Vec<(i128, i128, ExprId, ExprId)>,
    /// Case values outside `i128` (an `unsigned __int128` above
    /// `i128::MAX`, a wide `_BitInt`): their normalized words, decimal
    /// spelling and location, for the duplicate check only.
    pub wide_values: Vec<(Vec<u64>, String, Loc)>,
    /// The `CaseStmt`/`DefaultStmt`s, in source order.
    pub cases: Vec<StmtId>,
    /// The first `default:`.
    pub default: Option<Loc>,
    /// Set when the condition was invalid (suppresses case checks).
    pub invalid: bool,
}

/// Per-function state while its body is analysed.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent facts of one body")]
pub struct FnState {
    pub(crate) decl: DeclId,
    pub(crate) ret: QualType,
    /// The declared placeholder return type (`auto`, `const auto`,
    /// `decltype(auto)`) of a function with a deduced return type.
    pub(crate) auto_ret: Option<QualType>,
    /// The placeholder was deduced from a return statement.
    pub(crate) deduced: bool,
    pub(crate) labels: Vec<LabelInfo>,
    pub(crate) switches: Vec<SwitchState>,
    /// Enclosing loops and switches (innermost last; `true` = loop). A
    /// loop is open from the start of its condition (`while`, `for`) or
    /// body (`do`, range-based `for`) to the end of the statement (clang
    /// `BreakScope`/`ContinueScope`; spec/sema.md §7.4a).
    pub(crate) breakables: Vec<bool>,
    /// The index in `breakables` of the `for` loop whose condition
    /// variable's declaration is being parsed: a `continue` to it is an
    /// error (clang `isConditionVarScope`).
    pub(crate) for_cond_var: Option<usize>,
    /// The type of `this` in this function (`cv C*`), `None` outside a
    /// non-static member function (spec/sema-class.md §2.5).
    pub(crate) this_ty: Option<QualType>,
    /// A `this` type that overrides `this_ty` while parsing an NSDMI or a
    /// trailing return type (clang `CXXThisScopeRAII`).
    pub(crate) this_override: Option<QualType>,
    /// The `return`s with an NRVO candidate, re-checked at the end of
    /// the body (clang `FunctionScopeInfo::Returns`, §5.5).
    pub(crate) returns: Vec<StmtId>,
    /// The body has a `switch` (a goto or `asm goto` shows in `labels`):
    /// the jump-scope check runs (clang `NeedsScopeChecking`).
    pub(crate) has_switch: bool,
    /// The body has a GNU `goto *e;` (clang `setFunctionHasIndirectGoto`):
    /// the jump-scope check runs.
    pub(crate) has_indirect_goto: bool,
    /// A `[[fallthrough]]` statement was built for the body (not by an
    /// instantiation, which copies it): its placement is checked at the
    /// end (clang `FunctionScopeInfo::HasFallthroughStmt`).
    pub(crate) has_fallthrough: bool,
    /// The body has a `[[clang::musttail]]` return (clang
    /// `setFunctionHasMustTail`): the jump-scope check runs.
    pub(crate) has_musttail: bool,
    /// Runtime-behavior diagnostics of the body (clang
    /// `FunctionScopeInfo::PossiblyUnreachableDiags`), emitted at its
    /// end (`diag_runtime_behavior`, `diag_if_reachable`), each with
    /// the expression whose reachability decides it.
    pub(crate) runtime_diags: Vec<(kiln_diag::Diag, Option<ExprId>)>,
    /// The quiet expression-evaluation contexts entered since the
    /// innermost potentially-evaluated one (clang's
    /// `ExprEvalContexts.back()` being `DiscardedStatement` or
    /// `ImmediateFunctionContext`): the body of a `consteval` function,
    /// the compound statement of `if consteval`, a discarded
    /// `if constexpr` branch. A variable's initializer is potentially
    /// evaluated again ([`Sema::enter_var_initializer`]).
    pub(crate) rt_quiet: u32,
    /// How many of `rt_quiet`'s contexts are discarded statements:
    /// clang 22 keeps a variable's initializer in a discarded statement
    /// quiet (its evaluation context inherits `InDiscardedStatement`).
    /// While nonzero, what is named is not odr-used
    /// ([`Sema::in_discarded_statement`]).
    pub(crate) discarded: u32,
    /// The coroutine state, once a coroutine keyword was seen
    /// (spec/sema.md §coroutines).
    pub(crate) coro: Option<Box<crate::coroutine::CoroState>>,
    /// The first `return` statement (an error in a coroutine,
    /// [stmt.return.coroutine]/1).
    pub(crate) first_return: Option<Loc>,
    /// The parameters assigned with `=` or whose address was taken so
    /// far (clang `ModifiedNonNullParams`): a `nonnull` one is no longer
    /// known non-null (spec/sema.md §5.27).
    pub(crate) modified_params: Vec<DeclId>,
    /// The left operands of the volatile simple assignments of the body
    /// not (yet) found discarded (clang
    /// `ExpressionEvaluationContextRecord::VolatileAssignmentLHSs`,
    /// [`Sema::note_volatile_assign`]), warned at the end of the body.
    pub(crate) volatile_assigns: Vec<ExprId>,
    /// The expression count when the body began: the body's calls are
    /// among the expressions built after it (the `-Winfinite-recursion`
    /// prefilter, [`Sema::check_infinite_recursion`]).
    pub(crate) expr_start: u32,
    /// A local variable without an initializer was declared, or a local
    /// variable was named in its own initializer: the uninitialized-values
    /// analysis has something to look at (clang's fast path).
    pub(crate) uninit_vars: bool,
    /// The body gets clang's `CheckConstexprFunctionDefinition` rules at
    /// its end if the function is `constexpr`/`consteval`: not for the
    /// call operator of a lambda that is not declared so explicitly
    /// (`constexpr_body.rs`).
    pub(crate) constexpr_rules: bool,
}

impl FnState {
    /// The body has a construct the jump-scope checker verifies (clang
    /// `NeedsScopeChecking`).
    fn needs_jump_scope_check(&self) -> bool {
        self.has_switch || self.has_indirect_goto || self.has_musttail || !self.labels.is_empty()
    }

    pub(crate) fn new(decl: DeclId, ret: QualType) -> Self {
        FnState {
            decl,
            ret,
            auto_ret: None,
            deduced: false,
            labels: Vec::new(),
            switches: Vec::new(),
            breakables: Vec::new(),
            for_cond_var: None,
            this_ty: None,
            this_override: None,
            returns: Vec::new(),
            has_switch: false,
            has_indirect_goto: false,
            has_fallthrough: false,
            has_musttail: false,
            runtime_diags: Vec::new(),
            rt_quiet: 0,
            discarded: 0,
            coro: None,
            first_return: None,
            modified_params: Vec::new(),
            volatile_assigns: Vec::new(),
            expr_start: 0,
            uninit_vars: false,
            constexpr_rules: true,
        }
    }
}

impl Sema<'_, '_> {
    // -- scopes -----------------------------------------------------------

    /// Enters a scope of `kind` (block scopes for compound statements and
    /// the implicit scopes of selection/iteration statements).
    pub fn push_scope(&mut self, kind: ScopeKind) {
        let mut s = Scope::new(kind, None);
        s.errors = self.diags.unrecoverable_error_count();
        self.scopes.push(s);
    }

    /// Leaves the innermost scope. A function scope's unused variables
    /// were diagnosed at the end of its body.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1
            && let Some(s) = self.scopes.pop()
        {
            self.apply_nrvo(&s);
            if s.kind != ScopeKind::Function {
                self.check_popped_labels(&s);
                self.diagnose_unused_locals(&s);
            }
        }
    }

    /// clang `CheckPoppedLabel`: a GNU local label of the block being left
    /// that was never defined is "use of undeclared label" at its
    /// declaration.
    fn check_popped_labels(&mut self, s: &Scope) {
        for &(name, d) in &s.labels {
            let undefined = self
                .func
                .as_ref()
                .and_then(|f| f.labels.iter().find(|l| l.decl == d))
                .and_then(|l| if l.defined.is_none() { l.local } else { None });
            if let Some(at) = undefined {
                let msg = self
                    .diag(DiagId::sema_undeclared_label, at)
                    .arg(self.ast.symbols().as_str(name).to_string());
                self.emit(msg);
            }
        }
    }

    /// clang `Scope::applyNRVO` on scope exit (spec/sema-class.md §5.5):
    /// the candidate declared in this scope becomes the named return
    /// value; a block passes its state (candidate or disabled) up to the
    /// enclosing scope, overwriting it.
    fn apply_nrvo(&mut self, s: &Scope) {
        let Some(nrvo) = s.nrvo.nrvo else { return };
        if let Some(v) = nrvo
            && s.locals.iter().any(|l| l.1 == v)
        {
            self.builder().set_nrvo(v);
        }
        if matches!(s.kind, ScopeKind::Block | ScopeKind::Control)
            && let Some(parent) = self.scopes.last_mut()
        {
            parent.nrvo.nrvo = Some(nrvo);
        }
    }

    /// clang `Scope::updateNRVOCandidate` at a `return` returning the
    /// copy-elision candidate `v` (`None` for any other operand): the
    /// candidate keeps its return slot in each scope out to the function
    /// scope that declared it, every other variable loses it.
    pub(crate) fn update_nrvo_candidate(&mut self, v: Option<DeclId>) {
        let mut can_be_put = false;
        for s in self.scopes.iter_mut().rev() {
            let found = v.is_some_and(|v| s.nrvo.slots.contains(&v));
            s.nrvo.slots.clear();
            if found {
                s.nrvo.slots.extend(v);
            }
            can_be_put |= found;
            if !matches!(s.kind, ScopeKind::Block | ScopeKind::Control) {
                break;
            }
        }
        if let Some(s) = self.scopes.last_mut() {
            s.nrvo.nrvo = Some(if can_be_put { v } else { None });
        }
    }

    /// The copy-elision candidate of `return e;` ([class.copy.elision]/1.1,
    /// clang `getNamedReturnInfo` + `getCopyElisionCandidate`): a possibly
    /// parenthesized id-expression naming a non-volatile automatic
    /// variable of the function (not a parameter or exception variable)
    /// whose type is the class return type ignoring cv.
    fn copy_elision_candidate(&self, e: ExprId, ret: QualType) -> Option<DeclId> {
        // A return type still to be deduced at instantiation has no
        // candidate (clang `getCopyElisionCandidate`: a canonical `auto` or
        // the dependent builtin).
        if ret == QualType::DEPENDENT
            || matches!(self.ast.types.canonical_view(ret), TypeView::Auto { .. })
        {
            return None;
        }
        // clang `IgnoreParens`: also through `__extension__` and a resolved
        // `_Generic` or `__builtin_choose_expr`.
        let id = self.ignore_parens(e);
        if self.ast.exprs.get(id).kind() != ExprKind::DeclRefExpr
            || self.ast.exprs.get(id).has(kiln_ast::expr::ExprFlags::REFERS_TO_ENCLOSING)
        {
            return None;
        }
        let d = self.ast.exprs.decl(id);
        let dn = *self.ast.decls.get(d);
        if dn.kind() != DeclKind::Var || !self.in_innermost_function(dn.ctx()) {
            return None;
        }
        let v = self.ast.decls.var(d)?;
        let automatic = matches!(
            v.spec.storage(),
            kiln_ast::decl::VarStorage::None | kiln_ast::decl::VarStorage::Register
        ) && v.spec.tls() == kiln_ast::decl::TlsKind::None
            && dn.ctx().is_some_and(|c| self.ast.decls.get(c).kind().is_function());
        if !automatic || v.spec.is_exception_var() {
            return None;
        }
        let vty = dn.ty()?;
        if self.ast.types.is_reference(vty) || self.ast.types.effective_quals(vty).is_volatile() {
            return None;
        }
        let dependent = self.ast.types.is_dependent(ret) || self.ast.types.is_dependent(vty);
        if !self.ast.types.is_dependent(ret) && self.ast.types.as_record(ret).is_none() {
            return None;
        }
        if !dependent && !self.ast.types.same_unqualified(ret, vty) {
            return None;
        }
        Some(d)
    }

    // -- simple statements ------------------------------------------------

    /// `{ stmts }` ([stmt.block]).
    pub fn act_on_compound(&mut self, stmts: &[StmtId], lbrace: Loc, rbrace: Loc) -> StmtId {
        self.builder().compound(stmts, lbrace, rbrace)
    }

    /// `{ stmts }` whose FP state (after its leading FP pragmas) is `fp`.
    pub fn act_on_compound_fp(
        &mut self,
        stmts: &[StmtId],
        lbrace: Loc,
        rbrace: Loc,
        fp: kiln_ast::fp::FpOverride,
    ) -> StmtId {
        self.builder().compound_fp(stmts, lbrace, rbrace, fp)
    }

    /// The FP pragma state in effect (spec/codegen.md "Floating-point
    /// semantics").
    pub fn fp_pragma_state(&self) -> kiln_ast::fp::FpOverride {
        self.fp_pragma.0
    }

    /// Sets the FP pragma state (a pragma, or the end of a compound
    /// statement restoring its enclosing state).
    pub fn set_fp_pragma_state(&mut self, fp: kiln_ast::fp::FpOverride) {
        self.fp_pragma.0 = fp;
    }

    /// `#pragma float_control(push)`: saves the state.
    pub fn fp_pragma_push(&mut self) {
        let cur = self.fp_pragma.0;
        self.fp_pragma.1.push(cur);
    }

    /// `#pragma float_control(pop)`: restores the last saved state; false
    /// if none.
    pub fn fp_pragma_pop(&mut self) -> bool {
        match self.fp_pragma.1.pop() {
            Some(s) => {
                self.fp_pragma.0 = s;
                true
            }
            None => false,
        }
    }

    /// `;`.
    pub fn act_on_null_stmt(&mut self, loc: Loc) -> StmtId {
        self.builder().null_stmt(loc)
    }

    /// A declaration statement ([stmt.dcl]).
    pub fn act_on_decl_stmt(&mut self, decls: &[DeclId], loc: Loc) -> StmtId {
        if !self.lambdas.is_empty() {
            for &d in decls {
                if let Some(t) = self.ast.decls.get(d).ty()
                    && self.has_unexpanded_pack(t)
                {
                    self.note_unexpanded_in_lambda(crate::templ::pack::PackOperand::Type(t));
                }
            }
        }
        self.builder().decl_stmt(decls, loc)
    }

    /// An expression statement ([stmt.expr]): `e` is a discarded-value
    /// full-expression.
    pub fn act_on_expr_stmt(&mut self, e: ExprId) -> StmtId {
        let e = if self.diagnose_unexpanded_written(e, "expression", None) {
            let at = self.begin_loc(e);
            self.recover_err(&[], at)
        } else {
            e
        };
        let e = self.resolve_single_overload_candidate(e);
        if !self.check_builtin_fn_use(e) && !self.check_overload_use(e) {
            self.diagnose_unused(e);
        }
        self.discard_assignment_ref(e);
        let e = self.ignored_value_conversions(e);
        let e = self.act_on_full_expr(e);
        self.builder().expr_stmt(e)
    }

    /// clang `IgnoredValueConversions` for a discarded-value expression
    /// ([expr.context]/2): from C++11, a glvalue of volatile-qualified
    /// type of one of the forms [`Sema::is_read_if_discarded`] accepts
    /// undergoes the lvalue-to-rvalue conversion (so the object is read).
    pub(crate) fn ignored_value_conversions(&mut self, e: ExprId) -> ExprId {
        self.discard_volatile_assign(e);
        if self.std >= kiln_lex::Std::Cxx11 && self.is_read_if_discarded(e) {
            conv::lvalue_to_rvalue(self, e)
        } else {
            e
        }
    }

    /// clang `Expr::isReadIfDiscardedInCPlusPlus11` ([expr.context]/2): a
    /// glvalue of volatile-qualified (non-class, non-array) type that is,
    /// ignoring parentheses, an id-expression, a subscript, a class member
    /// access, an indirection, a pointer-to-member operation, a comma
    /// expression whose right operand is one of these, or a conditional
    /// expression both of whose arms are.
    fn is_read_if_discarded(&self, e: ExprId) -> bool {
        let n = self.ast.exprs.get(e);
        let t = n.ty();
        if !n.value_category().is_glvalue()
            || self.ast.types.is_dependent(t)
            || !self.ast.types.effective_quals(t).is_volatile()
        {
            return false;
        }
        let mut x = e;
        while self.ast.exprs.get(x).kind() == ExprKind::ParenExpr {
            x = self.ast.exprs.operand(x);
        }
        match self.ast.exprs.get(x).kind() {
            ExprKind::DeclRefExpr | ExprKind::ArraySubscriptExpr | ExprKind::MemberExpr => true,
            ExprKind::UnaryOperator => self.ast.exprs.unary_op(x) == UnaryOp::Deref,
            ExprKind::BinaryOperator => match self.ast.exprs.binary_op(x) {
                BinaryOp::PtrMemD | BinaryOp::PtrMemI => true,
                BinaryOp::Comma => self.is_read_if_discarded(self.ast.exprs.operands(x)[1]),
                _ => false,
            },
            ExprKind::ConditionalOperator => {
                let [a, b] = self.ast.exprs.cond_arms(x);
                self.is_read_if_discarded(a) && self.is_read_if_discarded(b)
            }
            ExprKind::BinaryConditionalOperator => {
                let ops = self.ast.exprs.binary_conditional_operands(x, &self.ast.lists);
                self.is_read_if_discarded(ops[0]) && self.is_read_if_discarded(ops[3])
            }
            _ => false,
        }
    }

    /// A condition ([stmt.pre]/4): contextually converted to `bool`.
    pub fn act_on_condition(&mut self, e: ExprId) -> ExprId {
        self.act_on_condition_at(e, Loc::INVALID)
    }

    /// [`Sema::act_on_condition`] of the statement whose keyword (`if`,
    /// `while`, `do`, `for`) is at `kw`: the conversion context of its
    /// implicit-conversion warnings (clang `CheckBooleanCondition`'s
    /// `Loc`); invalid for the full-expression's own location.
    pub fn act_on_condition_at(&mut self, e: ExprId, kw: Loc) -> ExprId {
        if self.diagnose_unexpanded_written(e, "expression", None) {
            let at = self.begin_loc(e);
            return self.recover_err(&[], at);
        }
        let at = self.expr_loc(self.ignore_parens(e));
        let typo = self.take_condition_typo(at);
        let e = self.resolve_single_overload_candidate(e);
        let e = match conv::contextually_convert_to_bool(self, e) {
            Ok(c) => c,
            Err(err) => {
                let loc = self.ast.exprs.get(e).loc();
                self.emit(err.diag_at(self, loc));
                e
            }
        };
        let saved = std::mem::replace(&mut self.cond_typo_at, if typo { at } else { Loc::INVALID });
        let r =
            if kw.is_valid() { self.act_on_full_expr_at(e, kw) } else { self.act_on_full_expr(e) };
        self.cond_typo_at = saved;
        r
    }

    /// The statement attributes `attrs` written before `sub`, at `loc`
    /// ([dcl.attr.grammar]/5; clang's `ProcessStmtAttributes`), under
    /// language standard `std`. `[[assume]]`, `[[likely]]`,
    /// `[[unlikely]]` and `[[fallthrough]]` are kept (in an
    /// `AttributedStmt`); none applies to a declaration statement.
    /// `[[assume]]` and `[[fallthrough]]` apply to a null statement only
    /// ([dcl.attr.assume]/1, [dcl.attr.fallthrough]/1), a fallthrough
    /// statement only inside a switch (clang
    /// `err_fallthrough_attr_outside_switch`; whether it directly precedes
    /// a case label is checked at the end of the body, `fallthrough.rs`).
    /// `likely` and
    /// `unlikely` exclude each other ([dcl.attr.likelihood]/1). The
    /// assumption is contextually converted to `bool` unless dependent.
    /// Each attribute newer than `std` is an extension warning
    /// (`-Wc++17-attribute-extensions`, `-Wc++20-…`, `-Wc++23-…` under
    /// `-pedantic`).
    // One arm per statement attribute kind, then the cross-attribute
    // checks (compat, likelihood, musttail) in clang's order.
    #[allow(clippy::too_many_lines)]
    pub fn act_on_attributed_stmt(
        &mut self,
        attrs: &[kiln_ast::AttrId],
        sub: StmtId,
        loc: Loc,
        std: kiln_lex::Std,
    ) -> StmtId {
        use kiln_ast::attr::AttrKind;
        use kiln_lex::Std;
        let sub_kind = self.ast.stmts.get(sub).kind();
        let mut kept = Vec::new();
        let mut likelihood: Option<kiln_ast::attr::AttrNode> = None;
        let mut musttail = Vec::new();
        // An annotation appertains only to a declaration
        // ([dcl.attr.annotation]/1).
        self.reject_annotations_in(attrs, None);
        for &a in attrs {
            let node = *self.ast.attrs.get(a);
            if node.kind == AttrKind::LOOP_HINT {
                kept.extend(self.handle_loop_hint(a, sub));
                continue;
            }
            if let Some(keep) = self.class_b_stmt_attr(a, sub, &kept) {
                if keep {
                    kept.push(a);
                }
                continue;
            }
            let name = match node.kind {
                AttrKind::ASSUME => "assume",
                AttrKind::LIKELY => "likely",
                AttrKind::UNLIKELY => "unlikely",
                AttrKind::FALLTHROUGH => "fallthrough",
                AttrKind::MUSTTAIL => "musttail",
                // Any other attribute does not apply to a statement
                // (annotations were rejected above).
                AttrKind::ANNOTATION => continue,
                _ => {
                    if sub_kind != StmtKind::DeclStmt {
                        self.act_on_stmt_decl_attrs(&[a], false);
                    }
                    continue;
                }
            };
            let name = self.attr_shown(&node, name);
            if sub_kind == StmtKind::DeclStmt {
                let msg =
                    self.diag(DiagId::sema_stmt_attribute_on_decl, node.loc).arg(name.clone());
                self.emit(msg);
                continue;
            }
            if node.kind == AttrKind::MUSTTAIL {
                if sub_kind == StmtKind::ReturnStmt {
                    musttail.push(node);
                    kept.push(a);
                } else {
                    let msg = self
                        .diag(DiagId::sema_attribute_wrong_stmt_type, node.loc)
                        .arg(name)
                        .arg("return statements");
                    self.emit(msg);
                }
                continue;
            }
            if matches!(node.kind, AttrKind::ASSUME | AttrKind::FALLTHROUGH)
                && sub_kind != StmtKind::NullStmt
            {
                let msg = self
                    .diag(DiagId::sema_attribute_wrong_stmt_type, node.loc)
                    .arg(name.clone())
                    .arg("empty statements");
                self.emit(msg);
                continue;
            }
            match node.kind {
                AttrKind::FALLTHROUGH => {
                    if self.func.as_ref().is_none_or(|f| f.switches.is_empty()) {
                        let msg = self.diag(DiagId::sema_fallthrough_outside_switch, node.loc);
                        self.emit(msg);
                        continue;
                    }
                    if std < Std::Cxx17
                        && node.syntax == kiln_ast::attr::AttrSyntax::Cxx11
                        && node.flags & kiln_ast::attr::AttrNode::FALLTHROUGH_SCOPED == 0
                    {
                        let msg = self.diag(DiagId::ext_cxx17_attr, node.loc).arg(name);
                        self.emit(msg);
                    }
                    if let Some(f) = self.func.as_mut() {
                        f.has_fallthrough = true;
                    }
                    kept.push(a);
                }
                AttrKind::LIKELY | AttrKind::UNLIKELY => {
                    if std < Std::Cxx20 && node.syntax == kiln_ast::attr::AttrSyntax::Cxx11 {
                        let msg = self.diag(DiagId::ext_cxx20_attr, node.loc).arg(name.clone());
                        self.emit(msg);
                    }
                    if let Some(prev) = likelihood
                        && prev.kind != node.kind
                    {
                        let other =
                            if prev.kind == AttrKind::LIKELY { "likely" } else { "unlikely" };
                        let msg = self
                            .diag(DiagId::sema_attributes_not_compatible, node.loc)
                            .arg(name)
                            .arg(other)
                            .with_note(self.diag(DiagId::note_conflicting_attribute, prev.loc));
                        self.emit(msg);
                        continue;
                    }
                    likelihood = Some(node);
                    kept.push(a);
                }
                _ => {
                    let Some(e) = ExprId::from_raw(node.a) else { continue };
                    let Some(e) = self.build_cxx_assume_expr(e) else { continue };
                    if std < Std::Cxx23 {
                        let msg = self.diag(DiagId::ext_cxx23_attr, node.loc).arg(name);
                        self.emit(msg);
                    }
                    kept.push(self.assume_attr(node, e));
                }
            }
        }
        self.check_loop_hint_compat(&kept);
        if kept.is_empty() {
            return sub;
        }
        // clang `BuildAttributedStmt`: a failed `musttail` check drops
        // the attributed statement (WP-809).
        for node in musttail {
            if !self.check_musttail(sub, &node) {
                return sub;
            }
            if let Some(f) = self.func.as_mut() {
                f.has_musttail = true;
            }
        }
        self.builder().attributed_stmt(&kept, sub, loc)
    }

    /// The likelihood attribute (`[[likely]]`/`[[unlikely]]`) of `s`, if
    /// it is an `AttributedStmt` with one (clang `Stmt::getLikelihood`).
    fn likelihood_attr(&self, s: StmtId) -> Option<kiln_ast::attr::AttrNode> {
        use kiln_ast::attr::AttrKind;
        if self.ast.stmts.get(s).kind() != StmtKind::AttributedStmt {
            return None;
        }
        self.ast
            .lists
            .iter::<kiln_ast::AttrId>(self.ast.stmts.stmt_attrs(s))
            .map(|a| *self.ast.attrs.get(a))
            .find(|n| matches!(n.kind, AttrKind::LIKELY | AttrKind::UNLIKELY))
    }

    /// The `[[assume]]` attribute `node` with built assumption `e`
    /// (`AttrNode::ASSUME_IGNORED` when it has possible side effects).
    pub(crate) fn assume_attr(
        &mut self,
        node: kiln_ast::attr::AttrNode,
        e: ExprId,
    ) -> kiln_ast::AttrId {
        let dependent = self
            .ast
            .exprs
            .get(e)
            .deps()
            .intersects(ExprDeps::TYPE | ExprDeps::VALUE | ExprDeps::UNEXPANDED_PACK);
        let mut flags = node.flags & !kiln_ast::attr::AttrNode::ASSUME_IGNORED;
        if dependent || self.has_possible_side_effects(e) {
            flags |= kiln_ast::attr::AttrNode::ASSUME_IGNORED;
        }
        let b = self.expr_loc(e).to_raw();
        self.ast.attrs.push(kiln_ast::attr::AttrNode { flags, a: e.raw(), b, ..node })
    }

    /// clang's `BuildCXXAssumeExpr`: assumption `e` contextually converted
    /// to `bool` ([dcl.attr.assume]/1), with `-Wassume` when it has
    /// possible side effects (it is then not assumed; spec/codegen.md).
    /// A dependent assumption is kept as written until instantiation;
    /// `None` after an error.
    pub(crate) fn build_cxx_assume_expr(&mut self, e: ExprId) -> Option<ExprId> {
        let n = self.ast.exprs.get(e);
        if n.deps().contains(ExprDeps::ERROR) || n.kind() == ExprKind::RecoveryExpr {
            return None;
        }
        if n.deps().intersects(ExprDeps::TYPE | ExprDeps::VALUE | ExprDeps::UNEXPANDED_PACK) {
            return Some(e);
        }
        let c = match conv::contextually_convert_to_bool(self, e) {
            Ok(c) => c,
            Err(err) => {
                let loc = self.ast.exprs.get(e).loc();
                self.emit(err.diag_at(self, loc));
                return None;
            }
        };
        let c = self.act_on_full_expr(c);
        if self.has_possible_side_effects(c) {
            let msg = self.diag(DiagId::sema_assume_side_effects, self.begin_loc(c));
            self.emit(msg);
        }
        Some(c)
    }

    /// A condition declaration ([stmt.pre]/4): the variable `v` (already
    /// initialized) yields the condition value, a reference to `v`.
    pub fn act_on_condition_var(&mut self, v: DeclId, loc: Loc) -> (Option<DeclId>, ExprId) {
        // [stmt.pre]/5: not a function (clang `CheckConditionVariable`,
        // reached through `decltype(auto)`).
        if self.ast.decls.get(v).ty().is_some_and(|t| self.ast.types.is_function(t)) {
            let at = self.decl_loc(v);
            self.error(DiagId::sema_invalid_use_of_function_type, at);
            return (Some(v), self.recover_err(&[], loc));
        }
        let r = self.build_decl_ref(v, loc);
        if let Some(c) = self.refs_minus_assign.get_mut(&v) {
            c.1 = true;
        }
        (Some(v), r)
    }

    /// The condition of `if constexpr` ([stmt.if]/2): contextually
    /// converted to `bool` (narrowing allowed since P1401), then
    /// evaluated by the Oven (`EvalKind::IfConstexpr`) and wrapped in a
    /// `ConstantExpr` holding the value. A condition that is not a
    /// constant expression is diagnosed and kept unwrapped.
    pub fn act_on_constexpr_condition(&mut self, e: ExprId, kw: Loc) -> ExprId {
        let at = self.begin_loc(e);
        // clang `ActOnCondition`: its full-expression is completed as
        // constant-evaluated (`IsConstexpr`; no `-Winteger-overflow`).
        self.constant_full_expr += 1;
        let c = self.act_on_condition_at(e, kw);
        self.constant_full_expr -= 1;
        if self.is_err_expr(c) || self.ast.exprs.get(c).deps().intersects(ExprDeps::VALUE) {
            return c;
        }
        match self.eval_const(c, kiln_oven::EvalContext::new(kiln_oven::EvalKind::IfConstexpr, at))
        {
            Ok(v) => {
                let value = self.const_truth(v).unwrap_or(false);
                let cv = self.int_const(QualType::BOOL, i128::from(value));
                self.builder().constant_expr(c, Some(cv))
            }
            Err(mut f) => {
                let at = Self::fold_lone_invalid(&mut f, at);
                let d = self.diag(DiagId::sema_expr_not_cce, at).arg("constexpr if condition");
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                c
            }
        }
    }

    /// An if-statement ([stmt.if]) of `kind`: `if (init; cond) then else
    /// els`, `cond_var` being the condition variable if declared. `cond`
    /// is `None` exactly for `if consteval` / `if !consteval`, whose
    /// sub-statements are compound statements (checked by the parser).
    /// M2 keeps both branches of `if constexpr`; discarding the other
    /// branch matters only inside templates ([stmt.if]/2, M3).
    #[allow(clippy::too_many_arguments, reason = "mirrors the grammar")]
    pub fn act_on_if(
        &mut self,
        kind: IfKind,
        init: Option<StmtId>,
        cond_var: Option<DeclId>,
        cond: Option<ExprId>,
        then: StmtId,
        els: Option<StmtId>,
        loc: Loc,
    ) -> StmtId {
        // [dcl.attr.likelihood]: both branches equally likely (clang
        // `ActOnIfStmt`, `Stmt::determineLikelihoodConflict`). clang
        // reports it straight to the engine (`Diags.Report`): again for
        // each instantiation, without instantiation context notes.
        if let (Some(t), Some(e)) =
            (self.likelihood_attr(then), els.and_then(|e| self.likelihood_attr(e)))
            && t.kind == e.kind
        {
            let name =
                if t.kind == kiln_ast::attr::AttrKind::LIKELY { "likely" } else { "unlikely" };
            self.diag(DiagId::sema_likelihood_if_conflict, t.loc)
                .arg(name)
                .with_note(self.diag(DiagId::note_conflicting_attribute, e.loc))
                .allow_duplicate()
                .emit(self.diags);
        }
        let data = IfData { init, cond_var, cond, then: Some(then), els };
        self.builder().if_stmt(kind, data, loc)
    }

    /// Enters an immediate function context ([expr.const]/16): the
    /// compound statement of `if consteval` (or the `else` of
    /// `if !consteval`). M2 has no consteval functions, so nothing
    /// consults it yet beyond [`Sema::in_immediate_context`].
    pub fn enter_immediate_context(&mut self) {
        self.immediate += 1;
        self.enter_quiet_context();
    }

    /// Leaves an immediate function context.
    pub fn exit_immediate_context(&mut self) {
        self.immediate = self.immediate.saturating_sub(1);
        self.exit_quiet_context();
    }

    fn enter_quiet_context(&mut self) {
        if let Some(f) = self.func.as_mut() {
            f.rt_quiet += 1;
        }
    }

    fn exit_quiet_context(&mut self) {
        if let Some(f) = self.func.as_mut() {
            f.rt_quiet = f.rt_quiet.saturating_sub(1);
        }
    }

    /// Enters the discarded sub-statement of an `if constexpr` whose
    /// condition is not value-dependent ([stmt.if]/2; clang's
    /// `DiscardedStatement` evaluation context).
    pub fn enter_discarded_statement(&mut self) {
        self.enter_quiet_context();
        if let Some(f) = self.func.as_mut() {
            f.discarded += 1;
        }
    }

    /// Leaves a context entered with [`Sema::enter_discarded_statement`].
    pub fn exit_discarded_statement(&mut self) {
        self.exit_quiet_context();
        if let Some(f) = self.func.as_mut() {
            f.discarded = f.discarded.saturating_sub(1);
        }
    }

    /// Whether what an expression built now names is only referenced,
    /// not odr-used, being in a discarded statement of the function
    /// being defined ([stmt.if]/2 note, [basic.def.odr]/5; clang's
    /// `isOdrUseContext` giving `FormallyUsed`): nothing is then defined
    /// or instantiated for it but what constant evaluation or a deduced
    /// type needs ([temp.inst]/8). As in clang 21, the
    /// constant-expressions, `if consteval` blocks and nested
    /// `if constexpr` branches inside a discarded statement are
    /// discarded too; the body of a lambda or of a local class's member
    /// function is not (it has its own [`FnState`]).
    pub(crate) fn in_discarded_statement(&self) -> bool {
        self.func.as_ref().is_some_and(|f| f.discarded > 0)
    }

    /// Before a variable's initializer: it is potentially evaluated even
    /// inside a quiet context (clang's `ActOnCXXEnterDeclInitializer`
    /// pushes `PotentiallyEvaluated`). Returns what
    /// [`Sema::exit_var_initializer`] restores. A use of a deprecated
    /// entity in the initializer of `v` is in `v`'s context
    /// ([dcl.attr.deprecated]; clang delays it to the declarator's
    /// completion). At compat 22 it stays quiet in a discarded statement.
    /// Before, an initializer written (`has_init`) is no part of the
    /// discarded statement for odr-use either (clang 19/20's
    /// `PotentiallyEvaluated` context without `InDiscardedStatement`,
    /// [`Sema::in_discarded_statement`]); a default-initialization is.
    pub fn enter_var_initializer(
        &mut self,
        v: Option<DeclId>,
        has_init: bool,
    ) -> (u32, u32, Option<DeclId>) {
        let keep_discarded = self.clang_compat >= 22;
        let (quiet, discarded) = self.func.as_mut().map_or((0, 0), |f| {
            let keep = if keep_discarded { f.discarded } else { 0 };
            let odr_keep = if keep_discarded || !has_init { f.discarded } else { 0 };
            let quiet = std::mem::replace(&mut f.rt_quiet, keep);
            (quiet, std::mem::replace(&mut f.discarded, odr_keep))
        });
        let depr = std::mem::replace(&mut self.depr_init, v);
        (quiet, discarded, depr)
    }

    /// After a variable's initializer.
    pub fn exit_var_initializer(&mut self, (quiet, discarded, depr): (u32, u32, Option<DeclId>)) {
        if let Some(f) = self.func.as_mut() {
            f.rt_quiet = quiet;
            f.discarded = discarded;
        } else {
            let pending = std::mem::take(&mut self.volatile_assigns);
            self.warn_volatile_assigns(pending);
        }
        self.depr_init = depr;
    }

    /// The value of an `if constexpr` condition `c` as returned by
    /// [`Sema::act_on_constexpr_condition`], when it was evaluated
    /// (`None` when value-dependent or invalid).
    pub fn constexpr_condition_value(&self, c: ExprId) -> Option<bool> {
        if self.ast.exprs.get(c).kind() != ExprKind::ConstantExpr {
            return None;
        }
        self.const_truth(self.ast.exprs.constant_value(c)?)
    }

    /// Whether a run-time behavior diagnostic is dropped where the
    /// expression being built is (clang `DiagRuntimeBehavior`): in an
    /// unevaluated operand, a discarded statement, a manifestly
    /// constant-evaluated expression (whose problems the Oven reports),
    /// or an immediate function context.
    pub(crate) fn runtime_diag_quiet(&self) -> bool {
        self.unevaluated > 0
            || self.constant_full_expr > 0
            || self.in_constant_evaluated()
            || self.func.as_ref().is_some_and(|f| f.rt_quiet > 0)
    }

    /// True inside an immediate function context entered with
    /// [`Sema::enter_immediate_context`].
    pub fn in_immediate_context(&self) -> bool {
        self.immediate > 0
    }

    /// Opens a loop for `break`/`continue` ([stmt.break], [stmt.cont]):
    /// before the condition of a `while` and a `for` (after its
    /// init-statement), before the body of a `do` and a range-based `for`
    /// (clang's `BreakScope | ContinueScope`; spec/sema.md §7.4a). A jump
    /// in a GNU statement expression in the header then targets the loop.
    pub fn act_on_start_loop(&mut self) {
        if let Some(f) = self.func.as_mut() {
            f.breakables.push(true);
        }
    }

    /// Closes the loop opened by [`Sema::act_on_start_loop`].
    pub fn act_on_end_loop(&mut self) {
        if let Some(f) = self.func.as_mut() {
            f.breakables.pop();
        }
    }

    /// `while (cond) body` ([stmt.while]).
    pub fn act_on_while(
        &mut self,
        cond_var: Option<DeclId>,
        cond: ExprId,
        body: StmtId,
        loc: Loc,
    ) -> StmtId {
        let data = LoopData { cond_var, cond: Some(cond), body: Some(body), ..LoopData::default() };
        self.builder().while_stmt(data, loc)
    }

    /// `do body while (cond);` ([stmt.do]).
    pub fn act_on_do(&mut self, body: StmtId, cond: ExprId, loc: Loc, rparen: Loc) -> StmtId {
        self.builder().do_stmt(body, cond, loc, rparen)
    }

    /// `for (init; cond; inc) body` ([stmt.for]).
    pub fn act_on_for(
        &mut self,
        init: Option<StmtId>,
        cond_var: Option<DeclId>,
        cond: Option<ExprId>,
        inc: Option<ExprId>,
        body: StmtId,
        loc: Loc,
    ) -> StmtId {
        let inc = inc.map(|e| {
            self.diagnose_unused(e);
            self.discard_assignment_ref(e);
            let e = self.ignored_value_conversions(e);
            self.act_on_full_expr(e)
        });
        self.check_for_loop_analysis(cond_var, cond, inc, body);
        let data = LoopData { init, cond_var, cond, inc, body: Some(body) };
        self.builder().for_stmt(data, loc)
    }

    // -- switch -------------------------------------------------------------

    /// The condition of a `switch` ([stmt.switch]/2): converted to an
    /// integral type and promoted. Starts the switch body. `loc` is the
    /// `switch` keyword, where clang reports a non-integral condition.
    pub fn act_on_start_switch(&mut self, cond: ExprId, loc: Loc) -> ExprId {
        // [temp.variadic]/12, as a condition ([`Sema::act_on_condition`]).
        let cond = if self.diagnose_unexpanded_written(cond, "expression", None) {
            let at = self.begin_loc(cond);
            self.recover_err(&[], at)
        } else {
            cond
        };
        let ty0 = self.ast.exprs.get(cond).ty();
        let t = &self.ast.types;
        let tv = t.as_atomic(ty0).unwrap_or(ty0);
        let ok = t.is_integral(tv)
            || t.as_enum(tv).is_some()
            || t.builtin(ty0) == Some(BuiltinType::Error);
        // [temp.res.general]/6: a condition of dependent type is checked
        // (converted, promoted) when the template is instantiated; the
        // case values are left unevaluated until then.
        // [stmt.switch]/2: a condition of class type is contextually
        // implicitly converted to an integral or enumeration type
        // ([conv.general]/5, `switch_conv.rs`).
        let class = if ok || self.is_dependent_type(ty0) || t.as_record(ty0).is_none() {
            None
        } else {
            self.contextual_class_conversion(crate::switch_conv::ContextualConv::Switch, cond, loc)
        };
        let (cond, cond_ty, invalid) = if self.is_dependent_type(ty0) {
            (cond, ty0, false)
        } else if let Some(c) = class {
            if self.is_err_expr(c) {
                (c, QualType::INT, true)
            } else {
                self.check_switch_bool(c, loc);
                let c = conv::integral_promotion(self, c);
                let ty = self.ast.exprs.get(c).ty();
                (c, ty, false)
            }
        } else if ok {
            self.check_switch_bool(cond, loc);
            let c = conv::integral_promotion(self, cond);
            let ty = self.ast.exprs.get(c).ty();
            (c, ty, false)
        } else {
            let d = self.diag(DiagId::sema_statement_requires_integer, loc).arg(self.ty_arg(ty0));
            self.emit(d);
            let c = conv::decay(self, cond);
            (c, QualType::INT, true)
        };
        let cond = self.act_on_full_expr(cond);
        if let Some(f) = self.func.as_mut() {
            f.has_switch = true;
            f.switches.push(SwitchState {
                cond_ty,
                values: Vec::new(),
                ranges: Vec::new(),
                wide_values: Vec::new(),
                cases: Vec::new(),
                default: None,
                invalid,
            });
            f.breakables.push(false);
        }
        cond
    }

    /// Finishes `switch (init; cond) body`.
    pub fn act_on_finish_switch(
        &mut self,
        init: Option<StmtId>,
        cond_var: Option<DeclId>,
        cond: ExprId,
        body: StmtId,
        loc: Loc,
    ) -> StmtId {
        let cases = match self.func.as_mut() {
            Some(f) => {
                f.breakables.pop();
                f.switches.pop()
            }
            None => None,
        };
        let cases = match cases {
            Some(sw) => {
                self.check_case_ranges(&sw);
                self.check_switch_cases(&sw, cond, loc);
                sw.cases
            }
            None => Vec::new(),
        };
        let cases = self.builder().list(&cases);
        self.builder().switch_stmt(SwitchData { init, cond_var, cond, body, cases }, loc)
    }

    /// The value of `case value:` ([stmt.switch]/2): a converted constant
    /// expression of the promoted condition type. Returns the
    /// `ConstantExpr` (or `e` when invalid).
    pub fn act_on_case_value(&mut self, e: ExprId, case_loc: Loc) -> ExprId {
        let (ce, value) = self.convert_case_value(e, case_loc);
        let Some(iv) = value else {
            self.wide_duplicate_case(e, ce);
            return ce;
        };
        // clang reports (and remembers) a case at its value's begin.
        let loc = self.begin_loc(e);
        let dup = self
            .func
            .as_ref()
            .and_then(|f| f.switches.last())
            .and_then(|s| s.values.iter().find(|(x, _)| *x == iv).copied());
        if let Some((_, prev)) = dup {
            let d = self
                .diag(DiagId::sema_duplicate_case, loc)
                .arg(iv.to_string())
                .with_note(self.diag(DiagId::note_duplicate_case_prev, prev));
            self.emit(d);
        } else if let Some(s) = self.func.as_mut().and_then(|f| f.switches.last_mut()) {
            s.values.push((iv, loc));
        }
        ce
    }

    /// The duplicate check of a case value `e` (converted: `ce`) outside
    /// `i128`, by its exact bits.
    fn wide_duplicate_case(&mut self, e: ExprId, ce: ExprId) {
        if self.ast.exprs.get(ce).kind() != ExprKind::ConstantExpr {
            return;
        }
        let Some(v) = self.ast.exprs.constant_value(ce) else { return };
        let Some(Scalar::Int { bits, signed, .. }) = self.ast.consts.scalar(v) else { return };
        let words = bits.words().to_vec();
        let loc = self.begin_loc(e);
        let Some(sw) = self.func.as_ref().and_then(|f| f.switches.last()) else { return };
        if sw.invalid {
            return;
        }
        if let Some((_, text, prev)) = sw.wide_values.iter().find(|w| w.0 == words).cloned() {
            let d = self
                .diag(DiagId::sema_duplicate_case, loc)
                .arg(text)
                .with_note(self.diag(DiagId::note_duplicate_case_prev, prev));
            self.emit(d);
            return;
        }
        let mut text = String::new();
        bits.write_decimal(signed, &mut text);
        if let Some(s) = self.func.as_mut().and_then(|f| f.switches.last_mut()) {
            s.wide_values.push((words, text, loc));
        }
    }

    /// The bounds of a GNU case range `case lo ... hi:`, each converted
    /// like a case value; the range is checked when the switch ends
    /// (clang `ActOnFinishSwitchStmt`).
    pub fn act_on_case_range(&mut self, lo: ExprId, hi: ExprId, case_loc: Loc) -> (ExprId, ExprId) {
        let (lo_ce, lo_v) = self.convert_case_value(lo, case_loc);
        if self.func.as_ref().is_none_or(|f| f.switches.is_empty()) {
            return (lo_ce, hi);
        }
        let (hi_ce, hi_v) = self.convert_case_value(hi, case_loc);
        if let (Some(l), Some(h)) = (lo_v, hi_v)
            && let Some(s) = self.func.as_mut().and_then(|f| f.switches.last_mut())
        {
            s.ranges.push((l, h, lo, hi));
        }
        (lo_ce, hi_ce)
    }

    /// The case ranges of the finished switch `sw`: an empty range is
    /// dropped with a warning, then each range is checked against the
    /// case values and the previous range (clang `ActOnFinishSwitchStmt`).
    fn check_case_ranges(&mut self, sw: &SwitchState) {
        let mut ranges = sw.ranges.clone();
        ranges.sort_by_key(|r| r.0);
        let mut kept = Vec::new();
        for (lo, hi, le, he) in ranges {
            if lo > hi {
                let span = kiln_source::Span::new(self.begin_loc(le), self.expr_range(he).hi);
                let d = self.diag(DiagId::sema_case_empty_range, self.begin_loc(le)).range(span);
                self.emit(d);
            } else {
                kept.push((lo, hi, le));
            }
        }
        let mut values = sw.values.clone();
        values.sort_by_key(|v| v.0);
        for (i, &(lo, hi, le)) in kept.iter().enumerate() {
            // The largest case value in the range, else an overlap with
            // the previous range.
            let mut overlap =
                values.iter().rev().find(|v| v.0 >= lo && v.0 <= hi).map(|&(v, l)| (v, l));
            if i > 0 && lo <= kept[i - 1].1 {
                overlap = Some((kept[i - 1].1, self.begin_loc(kept[i - 1].2)));
            }
            if let Some((v, prev)) = overlap {
                let d = self
                    .diag(DiagId::sema_duplicate_case, self.begin_loc(le))
                    .arg(v.to_string())
                    .with_note(self.diag(DiagId::note_duplicate_case_prev, prev));
                self.emit(d);
            }
        }
    }

    /// Converts the case value `e` to the promoted condition type and
    /// evaluates it: the `ConstantExpr` (or `e`) and its value, `None`
    /// when dependent, invalid or outside a valid switch.
    fn convert_case_value(&mut self, e: ExprId, case_loc: Loc) -> (ExprId, Option<i128>) {
        // [temp.variadic]/12: clang's `ActOnCaseExpr` ends the value as a
        // full-expression ("expression", at its begin).
        if self.diagnose_unexpanded_written(e, "expression", None) {
            let at = self.begin_loc(e);
            return (self.recover_err(&[], at), None);
        }
        let Some(sw) = self.func.as_ref().and_then(|f| f.switches.last()) else {
            self.error(DiagId::sema_case_not_in_switch, case_loc);
            return (e, None);
        };
        let (cond_ty, invalid) = (sw.cond_ty, sw.invalid);
        if self.expr_is_value_dependent(e) || self.is_dependent_type(cond_ty) {
            return (e, None);
        }
        let loc = self.ast.exprs.get(e).loc();
        let narrowed = match self.case_cce_check(e, cond_ty) {
            Ok(n) => n,
            Err(r) => return r,
        };
        let conv = match conv::implicit_convert(self, e, cond_ty, ConvCtx::Initialization) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, loc);
                return (e, None);
            }
        };
        let at = self.begin_loc(e);
        match self.eval_const(conv, kiln_oven::EvalContext::converted(at)) {
            Ok(v) => {
                // A value outside `i128` keeps its own (exact) constant
                // and has no `i128` key.
                let iv = self.const_int(v);
                let cv = match iv {
                    Some(iv) => self.int_const(cond_ty, iv),
                    None => v,
                };
                let ce = self.builder().constant_expr(conv, Some(cv));
                let ce = self.act_on_constant_full_expr(ce);
                self.case_narrowing(narrowed, at, cond_ty);
                (ce, iv.filter(|_| !invalid))
            }
            Err(mut f) => {
                // [stmt.switch]/2: a converted constant expression (clang's
                // `CheckConvertedConstantExpression`, CCEK_CaseValue),
                // reported at the value's first token.
                let full = self.act_on_constant_full_expr(conv);
                self.case_narrowing(narrowed, at, cond_ty);
                let at = Self::fold_lone_invalid(&mut f, at);
                let msg = self.diag(DiagId::sema_expr_not_cce, at).arg("case value");
                let d = Self::with_failure_notes(msg, f);
                self.emit(d);
                (full, None)
            }
        }
    }

    /// [stmt.switch]/2, [expr.const]/12 (clang
    /// `BuildConvertedConstantExpression`, `CCEK_CaseValue`): a case value is
    /// a converted constant expression of the promoted condition type. With
    /// no implicit conversion sequence to it, or one whose (last) standard
    /// conversion is not allowed there (a floating-integral conversion, a
    /// boolean conversion from a pointer, ...), it is an error at the
    /// value's first token naming its type as written, and the case is
    /// dropped (`Err`, the recovered value). Otherwise the value a
    /// narrowing integral conversion changes, if any (`Ok`), reported by
    /// `case_narrowing` after the full-expression's conversion warnings, as
    /// in clang.
    fn case_cce_check(
        &mut self,
        e: ExprId,
        cond_ty: QualType,
    ) -> Result<Option<i128>, (ExprId, Option<i128>)> {
        let written = self.ast.exprs.get(e).ty();
        let at = self.begin_loc(e);
        let pty = conv::strip_cv(self, cond_ty);
        if self.is_c() {
            return self.c_case_check(e, written, at);
        }
        if written != QualType::OVERLOAD && !self.ast.types.is_dependent(written) {
            let info = self.arg_info(e);
            let ics = self.try_implicit_conversion(&info, pty, crate::overload::IcsOptions::NONE);
            let scs = match ics {
                Ics::Standard(s) => Some(s),
                Ics::UserDefined(u) => Some(u.after),
                _ => None,
            };
            let id = if ics.is_bad() {
                Some(DiagId::sema_cce_not_implicitly_convertible)
            } else if scs.is_some_and(|s| !self.case_second_allowed(&s)) {
                Some(DiagId::sema_cce_conversion_not_allowed)
            } else {
                None
            };
            if let Some(id) = id {
                let d = self
                    .diag(id, at)
                    .arg(self.ty_arg(written))
                    .arg(self.ty_arg(pty))
                    .range(self.expr_range(e));
                self.emit(d);
                return Err((self.recover_err(&[e], at), None));
            }
        }
        let c = conv::lvalue_to_rvalue(self, e);
        let cty = conv::strip_cv(self, self.ast.exprs.get(c).ty());
        let t = &self.ast.types;
        if !(t.is_integral(pty) && (t.is_integral(cty) || t.as_enum(cty).is_some())) {
            return Ok(None);
        }
        // C23 6.8.5.3p5: a C case constant is converted to the promoted
        // condition type with no narrowing rule (`case 0xca093fa0:` on an
        // `int`).
        if self.is_c() {
            return Ok(None);
        }
        Ok(self.fold_int(c).filter(|&raw| !self.int_fits(pty, raw)))
    }

    /// C23 6.8.5.3p3, p5: a case value is an integer constant expression,
    /// converted to the promoted type of the controlling expression with
    /// no narrowing rule (clang's `ActOnCaseExpr` outside C++11:
    /// `VerifyIntegerConstantExpression`, `err_ice_not_integral`).
    fn c_case_check(
        &mut self,
        e: ExprId,
        written: QualType,
        at: Loc,
    ) -> Result<Option<i128>, (ExprId, Option<i128>)> {
        let t = &self.ast.types;
        let w = conv::strip_cv(self, written);
        if self.is_error_type(w) || t.is_integral(w) || t.as_enum(w).is_some() {
            return Ok(None);
        }
        let d = self
            .diag(DiagId::sema_ice_not_integral_c, at)
            .arg(self.ty_arg(written))
            .range(self.expr_range(e));
        self.emit(d);
        Err((self.recover_err(&[e], at), None))
    }

    /// clang `CheckConvertedConstantConversions`: the second standard
    /// conversions allowed in a converted constant expression of integral
    /// or enumeration type ([expr.const]/12; narrowing is checked apart).
    fn case_second_allowed(&self, scs: &StandardConversion) -> bool {
        match scs.second {
            SecondStep::Identity
            | SecondStep::IntegralPromotion
            | SecondStep::IntegralConversion => true,
            SecondStep::BooleanConversion => {
                self.ast.types.is_integral(scs.from) || conv::is_unscoped_enum(self, scs.from)
            }
            SecondStep::PointerConversion | SecondStep::MemberPointerConversion => {
                self.is_nullptr_type(scs.from)
            }
            _ => false,
        }
    }

    /// The narrowing of a case value `raw` to the condition type (clang
    /// `ext_cce_narrowing`: a `-Wc++11-narrowing` error by default; clang
    /// goes on with the converted value), at the value's first token `at`.
    fn case_narrowing(&mut self, raw: Option<i128>, at: Loc, cond_ty: QualType) {
        if let Some(raw) = raw {
            let pty = conv::strip_cv(self, cond_ty);
            let d = self
                .diag(DiagId::sema_cce_narrowing_value, at)
                .arg("case value")
                .arg(raw.to_string())
                .arg(self.ty_arg(pty));
            self.emit(d);
        }
    }

    /// `case value: sub`, or the GNU range `case value ... rhs: sub`.
    pub fn act_on_case(
        &mut self,
        value: ExprId,
        rhs: Option<ExprId>,
        sub: StmtId,
        loc: Loc,
    ) -> StmtId {
        let s = self.builder().case_stmt(CaseData { lhs: value, rhs, sub }, loc);
        self.record_case(s);
        s
    }

    fn record_case(&mut self, s: StmtId) {
        if let Some(sw) = self.func.as_mut().and_then(|f| f.switches.last_mut()) {
            sw.cases.push(s);
        }
    }

    /// Checks a `default:` label before its sub-statement is parsed.
    pub fn act_on_default_label(&mut self, loc: Loc) {
        let Some(sw) = self.func.as_mut().and_then(|f| f.switches.last_mut()) else {
            self.error(DiagId::sema_default_not_in_switch, loc);
            return;
        };
        match sw.default {
            Some(prev) => {
                let d = self
                    .diag(DiagId::sema_multiple_default_labels, loc)
                    .with_note(self.diag(DiagId::note_duplicate_case_prev, prev));
                self.emit(d);
            }
            None => sw.default = Some(loc),
        }
    }

    /// `default: sub`.
    pub fn act_on_default(&mut self, sub: StmtId, loc: Loc) -> StmtId {
        let s = self.builder().default_stmt(sub, loc);
        self.record_case(s);
        s
    }

    // -- jumps --------------------------------------------------------------

    /// `break;` ([stmt.break]).
    pub fn act_on_break(&mut self, loc: Loc) -> StmtId {
        if self.func.as_ref().is_none_or(|f| f.breakables.is_empty()) {
            self.error(DiagId::sema_break_not_in_loop_or_switch, loc);
        }
        self.builder().break_stmt(loc)
    }

    /// `continue;` ([stmt.cont]). A `continue` in the initializer of a
    /// `for` loop's condition variable (in a GNU statement expression)
    /// would jump to the increment past that variable's initialization
    /// (clang `err_continue_from_cond_var_init`).
    pub fn act_on_continue(&mut self, loc: Loc) -> StmtId {
        let target =
            self.func.as_ref().map(|f| (f.breakables.iter().rposition(|&b| b), f.for_cond_var));
        match target {
            Some((Some(i), cv)) if cv == Some(i) => {
                self.error(DiagId::sema_continue_from_cond_var_init, loc);
            }
            Some((Some(_), _)) => {}
            _ => self.error(DiagId::sema_continue_not_in_loop, loc),
        }
        self.builder().continue_stmt(loc)
    }

    /// Starts the declaration of a `for` loop's condition variable (the
    /// loop is the innermost open one); returns the state to restore
    /// with [`Sema::end_for_cond_var`].
    pub fn begin_for_cond_var(&mut self) -> Option<usize> {
        let f = self.func.as_mut()?;
        let i = f.breakables.len().checked_sub(1);
        std::mem::replace(&mut f.for_cond_var, i)
    }

    /// Ends what [`Sema::begin_for_cond_var`] started.
    pub fn end_for_cond_var(&mut self, saved: Option<usize>) {
        if let Some(f) = self.func.as_mut() {
            f.for_cond_var = saved;
        }
    }

    /// The label `name` names here (clang `LookupOrCreateLabel`): the
    /// innermost GNU local label of that name in an enclosing block of
    /// this function, else the function's label, created at its first
    /// mention.
    fn label_decl(&mut self, name: Symbol, loc: Loc) -> DeclId {
        for s in self.scopes.iter().rev() {
            if let Some(&(_, d)) = s.labels.iter().rev().find(|l| l.0 == name) {
                return d;
            }
            if s.kind == ScopeKind::Function {
                break;
            }
        }
        if let Some(l) = self
            .func
            .as_ref()
            .and_then(|f| f.labels.iter().find(|l| l.name == name && l.local.is_none()))
        {
            return l.decl;
        }
        self.new_label(name, loc, None)
    }

    /// GNU `__label__ name;` at the start of a block (clang
    /// `LookupOrCreateLabel` with a `GnuLabelLoc`): a fresh label, visible
    /// in the current block, that shadows any label of the same name.
    pub fn act_on_local_label(&mut self, name: Symbol, loc: Loc) -> DeclId {
        let d = self.new_label(name, loc, Some(loc));
        if let Some(s) = self.scopes.last_mut() {
            s.labels.push((name, d));
        }
        d
    }

    fn new_label(&mut self, name: Symbol, loc: Loc, local: Option<Loc>) -> DeclId {
        let ctx = self.func.as_ref().map_or(self.ctx, |f| f.decl);
        let d = self.ast.decls.push(
            NewDecl {
                kind: DeclKind::Label,
                flags: DeclFlags::empty(),
                misc: DeclMisc::default(),
                name: DeclName::ident(name),
                ctx: Some(ctx),
                loc,
                ty: None,
            },
            // Label payload: unused (0); see spec/sema.md §7.4.
            Payload::None(0),
        );
        if let Some(f) = self.func.as_mut() {
            f.labels.push(LabelInfo { name, decl: d, defined: None, first_use: None, local });
        }
        d
    }

    /// `name:` before its sub-statement ([stmt.label]). Returns the label.
    pub fn act_on_label(&mut self, name: Symbol, loc: Loc) -> DeclId {
        let d = self.label_decl(name, loc);
        let prev = self
            .func
            .as_ref()
            .and_then(|f| f.labels.iter().find(|l| l.decl == d))
            .and_then(|l| l.defined.map(|at| l.local.unwrap_or(at)));
        if let Some(prev) = prev {
            let msg = self
                .diag(DiagId::sema_redefinition_of_label, loc)
                .arg(self.ast.symbols().as_str(name))
                .with_note(self.diag(DiagId::note_previous_definition, prev));
            self.emit(msg);
        } else if let Some(l) =
            self.func.as_mut().and_then(|f| f.labels.iter_mut().find(|l| l.decl == d))
        {
            l.defined = Some(loc);
        }
        d
    }

    /// `name: sub`.
    pub fn act_on_label_stmt(&mut self, label: DeclId, sub: StmtId, loc: Loc) -> StmtId {
        self.builder().label_stmt(label, sub, loc)
    }

    /// `goto name;` ([stmt.goto]).
    pub fn act_on_goto(&mut self, name: Symbol, name_loc: Loc, loc: Loc) -> StmtId {
        let d = self.label_decl(name, name_loc);
        self.ast.decls.set_flags(d, DeclFlags::USED);
        if let Some(l) = self.func.as_mut().and_then(|f| f.labels.iter_mut().find(|l| l.decl == d))
        {
            l.first_use.get_or_insert(name_loc);
        }
        self.builder().goto_stmt(d, loc)
    }

    /// GNU `goto *e;` (clang `ActOnIndirectGotoStmt`): `e` is a
    /// full-expression, so an unexpanded pack in it is diagnosed
    /// ([temp.variadic]/12); unless type-dependent it is converted to
    /// `const void*` as by assignment (clang `CheckSingleAssignment
    /// Constraints`, whose C++ path is `PerformImplicitConversion` with
    /// the assignment wording at the operand). The function then needs the
    /// jump-scope check (`setFunctionHasIndirectGoto`). `None` (the
    /// statement is dropped) when the operand is invalid.
    pub fn act_on_indirect_goto(&mut self, e: ExprId, loc: Loc, star: Loc) -> Option<StmtId> {
        if self.diagnose_unexpanded_written(e, "expression", None) || self.is_err_expr(e) {
            return None;
        }
        let e = if self.expr_is_type_dependent(e) {
            e
        } else {
            let cv = self.ast.types.qualify(QualType::VOID, kiln_ast::types::Quals::CONST);
            let to = self.ast.types.pointer_to(cv);
            match conv::implicit_convert(self, e, to, ConvCtx::Assignment) {
                Ok(x) => x,
                Err(err) => {
                    let d = err.diag(self, e);
                    self.emit(d);
                    return None;
                }
            }
        };
        let e = self.act_on_full_expr(e);
        if self.is_err_expr(e) {
            return None;
        }
        if let Some(f) = self.func.as_mut() {
            f.has_indirect_goto = true;
        }
        Some(self.builder().indirect_goto_stmt(e, loc, star))
    }

    /// GNU `&&label` (clang `LookupOrCreateLabel` then `ActOnAddrLabel`):
    /// a prvalue of type `void*` naming the label, which is marked used.
    /// Outside a function body clang rejects it at the identifier
    /// (`err_address_of_label_outside_fn`) without the extension warning;
    /// otherwise "use of GNU address-of-label extension" at the `&&`.
    pub fn act_on_addr_label(&mut self, name: Symbol, name_loc: Loc, amp: Loc) -> Option<ExprId> {
        if self.func.is_none() {
            let d = self.diag(DiagId::parse_address_of_label_outside_fn, name_loc);
            self.emit(d);
            return None;
        }
        let d = self.diag(DiagId::parse_gnu_address_of_label, amp);
        self.emit(d);
        let l = self.act_on_asm_goto_label(name, name_loc);
        let ty = self.ast.types.pointer_to(QualType::VOID);
        let e = self.builder().addr_label(l, ty, amp, name_loc);
        // A label of a template's body is a declaration of a dependent
        // context: instantiation rebuilds the expression for the
        // specialization's label (clang `TransformAddrLabelExpr`).
        if let Some(f) = self.func.as_ref().map(|f| f.decl)
            && self.ctx_template_levels(f) > 0
        {
            self.ast.exprs.mark_instantiation_dependent(e);
        }
        Some(e)
    }

    /// An `asm goto` label operand (clang `LookupOrCreateLabel` then
    /// `ActOnAddrLabel`, which marks it used).
    pub fn act_on_asm_goto_label(&mut self, name: Symbol, name_loc: Loc) -> DeclId {
        let d = self.label_decl(name, name_loc);
        self.ast.decls.set_flags(d, DeclFlags::USED);
        if let Some(l) = self.func.as_mut().and_then(|f| f.labels.iter_mut().find(|l| l.decl == d))
        {
            l.first_use.get_or_insert(name_loc);
        }
        d
    }

    /// `return e;` ([stmt.return]).
    pub fn act_on_return(&mut self, value: Option<ExprId>, loc: Loc) -> StmtId {
        let value = match value {
            Some(e) if self.diagnose_unexpanded_written(e, "expression", None) => {
                let at = self.begin_loc(e);
                Some(self.recover_err(&[], at))
            }
            v => v,
        };
        self.note_return_stmt(loc);
        let Some((fdecl, mut ret)) = self.func.as_ref().map(|f| (f.decl, f.ret)) else {
            return self.builder().return_stmt(value, None, loc);
        };
        if let Some(e) = value {
            self.warn_main_returns_bool(fdecl, e, loc);
        }
        if self.func.as_ref().is_some_and(|f| f.auto_ret.is_some()) {
            // A return in a discarded statement of a non-templated
            // function takes no part in return type deduction
            // ([dcl.spec.auto.general]/9, [stmt.if]/2) and, as in clang,
            // its operand is not converted to the deduced type.
            if self.in_discarded_statement() && !self.in_template() {
                let value = value.map(|e| self.act_on_full_expr(e));
                return self.builder().return_stmt(value, None, loc);
            }
            ret = self.deduce_return(value, loc);
        }
        // A `return` in a lambda whose call operator has a GNU
        // `noreturn` function type (spec/types.md §5.4; not
        // `[[noreturn]]`, a declaration attribute) is an error (clang
        // `ActOnCapScopeReturnStmt`, `err_noreturn_lambda_has_return_expr`;
        // WP-752).
        if self.is_lambda_call_op(fdecl) {
            let t = &self.ast.types;
            let fty = self.ast.decls.get(fdecl).ty();
            if fty.and_then(|q| t.proto(q)).is_some_and(|p| p.info().noreturn()) {
                self.error(DiagId::sema_noreturn_lambda_has_return_expr, loc);
            }
        }
        let ret_void = self.ast.types.is_void(ret);
        let fname = self.decl_name(fdecl);
        let mut candidate = None;
        // A dependent return type checks the operand at instantiation
        // (clang `BuildReturnStmt`'s `HasDependentReturnType`).
        let ret_dep = self.ast.types.is_dependent(ret);
        let value = match value {
            None => {
                if !ret_void && !self.is_error_type(ret) && !ret_dep {
                    let d = self.diag(DiagId::sema_return_missing_expr, loc).arg(fname);
                    self.emit(d);
                }
                None
            }
            Some(e) => {
                let ety = self.ast.exprs.get(e).ty();
                if ret_void {
                    // [stmt.return]/2: an operand of type `void` is fine;
                    // a type-dependent one is checked at instantiation.
                    if !self.ast.types.is_void(ety)
                        && !self.is_error_type(ety)
                        && !self.expr_is_type_dependent(e)
                    {
                        let d = self.diag(DiagId::sema_return_has_expr, loc).arg(fname);
                        self.emit(d);
                    }
                    Some(self.act_on_full_expr(e))
                } else {
                    // A `void` operand of a non-void function is an
                    // ill-formed copy-initialization ([stmt.return]/2,
                    // [dcl.init.general]/16), as in clang.

                    candidate = self.copy_elision_candidate(e, ret);
                    let e = self.return_operand(e, ret, fdecl, loc);
                    let c = self.convert_for_init(e, ret, ConvCtx::Return);
                    if !self.is_err_expr(c) {
                        self.check_return_lifetime(c, ret);
                    }
                    Some(self.act_on_full_expr_at(c, loc))
                }
            }
        };
        self.update_nrvo_candidate(candidate);
        let r = self.builder().return_stmt(value, candidate, loc);
        // clang `computeNRVO`: a candidate that does not end up as the
        // named return value is dropped from its return at the function's
        // end (only class-returning non-template functions are tracked).
        if candidate.is_some()
            && !self.in_template()
            && self.ast.types.as_record(ret).is_some()
            && let Some(f) = self.func.as_mut()
        {
            f.returns.push(r);
        }
        r
    }

    /// Whether a local whose semantic context is `ctx` is declared in the
    /// innermost enclosing function or lambda ([class.copy.elision]/3): a
    /// local of an enclosing function named from a lambda body is not.
    fn in_innermost_function(&self, ctx: Option<DeclId>) -> bool {
        let (Some(c), Some(f)) = (ctx, self.func.as_ref().map(|f| f.decl)) else {
            return true;
        };
        !self.ast.decls.get(c).kind().is_function() || self.ast.first(c) == self.ast.first(f)
    }

    /// The operand a `return` copy-initializes its result from. C++23
    /// (P2266, [expr.prim.id.unqual]/4): an implicitly movable entity is
    /// an xvalue. Before C++23 ([class.copy.elision]/3 of C++20, clang's
    /// `PerformMoveOrCopyInitialization`): the initialization is first
    /// tried with the operand as an rvalue; when that fails (other than by
    /// selecting a deleted function), it is performed again with the
    /// operand as the lvalue it is, so `T& f(T&& t) { return t; }` is
    /// well-formed in C++20.
    fn return_operand(&mut self, e: ExprId, ret: QualType, f: DeclId, loc: Loc) -> ExprId {
        let moved = self.implicit_move(e);
        if moved == e || self.std >= kiln_lex::Std::Cxx23 {
            return moved;
        }
        // Between non-class, non-reference types the two agree.
        let ety = self.ast.exprs.get(e).ty();
        if !self.ast.types.is_reference(ret)
            && self.ast.types.as_record(ret).is_none()
            && self.ast.types.as_record(ety).is_none()
        {
            return moved;
        }
        if self.ast.types.is_dependent(ret) || self.expr_is_type_dependent(e) {
            return moved;
        }
        let r = self.in_sfinae(FrameKind::ConstraintSubstitution, f, None, loc, |s| {
            let c = s.convert_for_init(moved, ret, ConvCtx::Return);
            !s.is_err_expr(c)
        });
        match r {
            Some((true, Ok(()))) => moved,
            Some((_, Err(Some(d))))
                if matches!(
                    d.id(),
                    Some(DiagId::sema_call_deleted_ctor | DiagId::sema_call_deleted_function)
                ) =>
            {
                moved
            }
            _ => e,
        }
    }

    /// C++23 implicit move ([expr.prim.id.unqual]/4, [class.copy.elision]/3):
    /// an id-expression naming a non-volatile object of automatic storage
    /// duration declared in the function becomes an xvalue.
    pub(crate) fn implicit_move(&mut self, e: ExprId) -> ExprId {
        // A possibly parenthesized id-expression; clang `getNamedReturnInfo`
        // strips with `IgnoreParens`, so also `__extension__` and a resolved
        // `_Generic` or `__builtin_choose_expr`. The xvalue cast wraps the
        // whole operand.
        let id = self.ignore_parens(e);
        let n = *self.ast.exprs.get(id);
        // A captured variable is not declared in the innermost function
        // or lambda ([class.copy.elision]/3).
        if n.kind() != ExprKind::DeclRefExpr
            || n.has(kiln_ast::expr::ExprFlags::REFERS_TO_ENCLOSING)
        {
            return e;
        }
        let d = self.ast.exprs.decl(id);
        let dn = *self.ast.decls.get(d);
        if !self.in_innermost_function(dn.ctx()) {
            return e;
        }
        let local = match dn.kind() {
            DeclKind::ParmVar => true,
            // A block-scope `thread_local` has thread storage duration
            // ([basic.stc.thread]): not implicitly movable.
            DeclKind::Var => {
                self.has_automatic_storage(d)
                    && dn.ctx().is_some_and(|c| self.ast.decls.get(c).kind().is_function())
            }
            _ => false,
        };
        let ty = n.ty();
        // The entity is an object or an rvalue reference to one: the
        // expression's type is the referee's, so ask the declaration.
        let lref = dn
            .ty()
            .is_some_and(|t| matches!(self.ast.types.canonical_view(t), TypeView::LValueRef(_)));
        if !local || lref || self.ast.types.effective_quals(ty).is_volatile() {
            return e;
        }
        self.builder().implicit_cast(CastKind::NoOp, e, ty, ValueCategory::XValue)
    }

    /// Starts the body of function `f` (its parameters are already in
    /// the function scope).
    pub(crate) fn begin_function_body(&mut self, f: DeclId, ret: QualType) {
        let mut state = FnState::new(f, ret);
        state.expr_start = u32::try_from(self.ast.exprs.len()).unwrap_or(0);
        // An immediate function's body is an immediate function context
        // ([expr.const]/16).
        state.rt_quiet = u32::from(self.is_consteval_kind(f));
        let auto = self.contains_undeduced_auto(ret);
        if auto {
            state.auto_ret = Some(ret);
        }
        self.func = Some(state);
        // A templated function deduces its return type when it is
        // instantiated ([dcl.spec.auto.general]/12): until then the
        // placeholder is deduced as dependent (clang's dependent
        // `AutoType`, printed `auto`), so calls of it in the template are
        // type-dependent rather than uses of an undeduced function.
        if auto && self.in_template() {
            let dep = self.dependent_auto(ret);
            self.set_deduced_return(f, dep);
        }
    }

    /// Ends the body: undefined labels and fall-off-the-end checks.
    /// Leaves the function-body scope ahead of the body's end checks:
    /// marks its NRVO candidate and diagnoses its unused variables.
    fn leave_function_scope(&mut self) {
        // The function scope's own NRVO candidate is marked before the
        // returns are re-checked (clang exits the body scope first).
        if let Some(s) = self.scopes.last_mut()
            && s.kind == ScopeKind::Function
        {
            let nrvo = std::mem::take(&mut s.nrvo);
            if let Some(Some(v)) = nrvo.nrvo
                && s.locals.iter().any(|l| l.1 == v)
            {
                self.builder().set_nrvo(v);
            }
        }
        // clang leaves the body scope (`ActOnPopScope`: the unused
        // variables) before `ActOnFinishFunctionBody`.
        if let Some(s) = self.scopes.last()
            && s.kind == ScopeKind::Function
        {
            let s = std::mem::replace(
                self.scopes.last_mut().expect("function scope"),
                Scope::new(ScopeKind::Function, None),
            );
            self.check_popped_labels(&s);
            self.diagnose_unused_locals(&s);
            *self.scopes.last_mut().expect("function scope") = s;
        }
    }

    /// Finishes a function body; returns the body to attach (a
    /// coroutine's is wrapped in its `CoroutineBodyStmt`).
    /// Plans the body's graph, built at its first query (spec/sema.md
    /// §7.5.1), and emits the deferred runtime-behavior diagnostics,
    /// dropping those anchored to an expression the graph proves
    /// unreachable.
    fn flush_runtime_diags(
        &mut self,
        f: DeclId,
        body: StmtId,
        runtime_diags: Vec<(kiln_diag::Diag, Option<ExprId>)>,
        has_fallthrough: bool,
        uninit: bool,
    ) {
        let dead = if self.diags.uncompilable_error_count() > 0 || self.in_template() {
            None
        } else {
            let wanted: HashSet<ExprId> = runtime_diags.iter().filter_map(|(_, e)| *e).collect();
            let probe = !wanted.is_empty();
            self.cfg_plan(f, body, wanted, has_fallthrough, uninit);
            if probe { self.with_cfg(body, |_, c| c.unreachable_probes()) } else { None }
        };
        for (msg, anchor) in runtime_diags {
            if let (Some(dead), Some(e)) = (&dead, anchor)
                && dead.contains(&e)
            {
                continue;
            }
            self.emit(msg);
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn end_function_body(&mut self, body: StmtId, rbrace: Loc) -> StmtId {
        self.leave_function_scope();
        let coroutine = self.coro_state().is_some_and(|c| c.first.is_some());
        let wrapped = self.complete_coroutine_body(body);
        // No return statement: the placeholder deduces `void`
        // ([dcl.spec.auto.general]/10); a templated function's when it
        // is instantiated (clang skips a dependent context).
        if let Some(st) = self.func.as_ref()
            && st.auto_ret.is_some()
            && !self.in_template()
            && !st.deduced
        {
            let f = st.decl;
            // A specialization whose body failed to instantiate (a
            // dropped `return`) deduces nothing (clang leaves the type
            // undeduced): the error type, so uses are silent.
            let ret = if self.ast.decls.get(f).has(DeclFlags::INVALID) {
                QualType::ERROR
            } else {
                QualType::VOID
            };
            self.set_deduced_return(f, ret);
        }
        let Some(mut state) = self.func.take() else { return wrapped };
        self.warn_volatile_assigns(std::mem::take(&mut state.volatile_assigns));
        for &r in &state.returns {
            if let Some(v) = self.ast.stmts.return_nrvo(r)
                && !self.ast.decls.var(v).is_some_and(|x| x.spec.is_nrvo())
            {
                self.builder().clear_return_nrvo(r);
            }
        }
        for l in &state.labels {
            if l.defined.is_none() && l.local.is_none() {
                let msg = self
                    .diag(DiagId::sema_undeclared_label, l.first_use.unwrap_or(rbrace))
                    .arg(self.ast.symbols().as_str(l.name).to_string());
                self.emit(msg);
            }
        }
        // [stmt.dcl]/3, [except.handle]/2: jumps into protected scopes
        // (clang `DiagnoseInvalidJumps`, ahead of the flow warnings).
        if state.needs_jump_scope_check() {
            self.check_jump_scopes(body);
        }
        // clang `CheckConstexprFunctionDefinition`'s rules on the
        // declaration and the statements, ahead of the analysis-based
        // warnings, which an error there suppresses (constexpr_body.rs).
        if state.constexpr_rules {
            self.check_constexpr_definition(state.decl, body);
        }
        // clang 22 `inferNoReturnAttr` (`PopFunctionScopeInfo`, ahead of
        // the analysis-based warnings; also in a dependent context).
        if self.diags.uncompilable_error_count() == 0 && !coroutine {
            self.infer_noreturn(state.decl, body);
        }
        // The deferred runtime-behavior diagnostics, ahead of the flow
        // warnings (clang `PopFunctionScopeInfo` / `IssueWarnings`). A
        // function template's are flushed unfiltered (clang finishes its
        // `FunctionTemplateDecl` without an analysis policy); any other
        // function of a dependent context (a member of a class template,
        // a lambda or local class in a template) drops the anchored ones
        // (clang's `IssueWarnings` returns for a dependent context): its
        // instantiations warn instead (rebuilt expressions re-check,
        // shared null dereferences replay, `reissue_null_derefs`). The
        // unanchored ones are kept: an instantiation does not re-check
        // them. With no error and outside a template, those anchored to
        // an expression that is unreachable in the body are dropped.
        if self.in_template() && !self.is_named_function_template(state.decl) {
            state.runtime_diags.retain(|(_, anchor)| anchor.is_none());
        }
        let runtime_diags = std::mem::take(&mut state.runtime_diags);
        let uninit = state.uninit_vars && self.uninit_enabled(state.decl);
        self.flush_runtime_diags(state.decl, body, runtime_diags, state.has_fallthrough, uninit);
        let ret = state.ret;
        // clang skips its analysis-based warnings once an error occurred
        // (`hasUncompilableErrorOccurred`: not a warning promoted by
        // `-Werror`), and in dependent contexts.
        if self.diags.uncompilable_error_count() > 0 || self.in_template() {
            return wrapped;
        }
        if self.cfg_stats {
            self.stats_body_cfg();
        }
        if self.cfg_dump {
            self.dump_body_cfg();
        }
        // clang `ActOnFinishFunctionBody`: no fall-off check for a
        // `naked` function (nor for `main`, below).
        let naked = self.naked_attr(state.decl).is_some();
        if self.decl_is_noreturn(state.decl) {
            // [dcl.attr.noreturn]/2: control reaching the end of a
            // `noreturn` function (clang `warn_falloff_noreturn_function`,
            // ahead of the return-type check).
            // A lambda's is an error (clang `MakeForLambda`'s
            // `err_noreturn_lambda_has_return_expr`, WP-752).
            if !naked && self.falls_off(body) != Falloff::Never {
                let id = if self.is_lambda_call_op(state.decl) {
                    DiagId::sema_noreturn_lambda_has_return_expr
                } else {
                    DiagId::sema_falloff_noreturn
                };
                self.error(id, rbrace);
            }
            self.late_analysis(state.decl, body, state.has_fallthrough, state.expr_start, uninit);
            return wrapped;
        }
        // A coroutine that falls off its end calls `return_void()` when
        // the promise has one ([stmt.return.coroutine]/3).
        let coro_void = coroutine
            && wrapped != body
            && self.ast.stmts.coroutine_body(wrapped).on_fallthrough.is_some();
        if self.ast.types.is_void(ret) || self.is_error_type(ret) || coro_void {
            if !naked && !coroutine && self.ast.types.is_void(ret) {
                self.suggest_noreturn(state.decl, body);
            }
            self.late_analysis(state.decl, body, state.has_fallthrough, state.expr_start, uninit);
            return wrapped;
        }
        if !naked && !self.is_main_function(state.decl) {
            let lambda = self.is_lambda_call_op(state.decl);
            match self.falls_off(body) {
                Falloff::Never => {}
                // clang 22: the body ends in a throw or a call of a
                // function inferred `noreturn`.
                _ if !coroutine && self.ends_in_inferred_noreturn(body) => {}
                Falloff::Always if lambda => {
                    self.error(DiagId::sema_falloff_nonvoid_lambda, rbrace);
                }
                Falloff::Maybe if lambda => {
                    self.error(DiagId::sema_falloff_nonvoid_lambda_maybe, rbrace);
                }
                Falloff::Always if coroutine => {
                    self.error(DiagId::sema_falloff_nonvoid_coroutine, rbrace);
                }
                Falloff::Maybe if coroutine => {
                    self.error(DiagId::sema_falloff_nonvoid_coroutine_maybe, rbrace);
                }
                Falloff::Always => self.error(DiagId::sema_falloff_nonvoid, rbrace),
                Falloff::Maybe => self.error(DiagId::sema_falloff_nonvoid_maybe, rbrace),
            }
        }
        self.late_analysis(state.decl, body, state.has_fallthrough, state.expr_start, uninit);
        wrapped
    }

    /// A diagnostic about behavior at run time (clang
    /// `DiagRuntimeBehavior`): dropped where
    /// [`Sema::runtime_diag_quiet`], deferred to the end of the enclosing
    /// function body, emitted at once outside a function. clang drops
    /// the deferred ones whose statement is unreachable in the body's
    /// CFG when no error occurred; kiln has no CFG and emits them all
    /// (spec/sema-stmt.md).
    pub(crate) fn diag_runtime_behavior(&mut self, msg: kiln_diag::Diag) {
        if self.runtime_diag_quiet() {
            return;
        }
        match self.func.as_mut() {
            Some(st) => st.runtime_diags.push((msg, None)),
            None => self.emit(msg),
        }
    }

    /// clang `DiagIfReachable`: in a function body, deferred to its end
    /// and dropped there when `anchor` is unreachable; outside one, not
    /// emitted in the initializer of a constexpr variable or of a static
    /// data member's first, non-inline declaration (clang reads its
    /// `ManglingContextDecl`).
    pub(crate) fn diag_if_reachable(&mut self, msg: kiln_diag::Diag, anchor: ExprId) {
        if let Some(st) = self.func.as_mut() {
            st.runtime_diags.push((msg, Some(anchor)));
            return;
        }
        self.emit_unless_constant_init(msg);
    }

    /// clang `DiagRuntimeBehavior` with no statement (`DiagIfReachable`
    /// with an empty list): dropped where [`Sema::runtime_diag_quiet`]
    /// and in the initializer of a constexpr variable (a local one too)
    /// or of a static data member's first, non-inline declaration;
    /// otherwise emitted at once, in a function body too.
    pub(crate) fn diag_runtime_behavior_now(&mut self, msg: kiln_diag::Diag) {
        if !self.runtime_diag_quiet() {
            self.emit_unless_constant_init(msg);
        }
    }

    /// Emits `msg` unless it is in the initializer of a constexpr variable
    /// or of a static data member's first, non-inline declaration (clang
    /// `DiagIfReachable` reading the `ManglingContextDecl`: that
    /// initializer is required to be a constant expression anyway).
    fn emit_unless_constant_init(&mut self, msg: kiln_diag::Diag) {
        if let Some(v) = self.lambda_mangle.var
            && let Some(var) = self.ast.decls.var(v)
        {
            let constexpr = var.spec.is_constexpr();
            let ctx = self.ast.decls.get(v).ctx();
            let static_member = ctx.is_some_and(|c| self.kind_of(c) == DeclKind::CXXRecord)
                && self.ast.prev(v).is_none()
                && !var.spec.is_inline();
            if constexpr || static_member {
                return;
            }
        }
        self.emit(msg);
    }

    /// Whether `f` is the pattern of a function template declared by
    /// name (a generic lambda's call operator is not: clang finishes it
    /// as a function of a dependent context).
    pub(crate) fn is_named_function_template(&self, f: DeclId) -> bool {
        !self.is_lambda_call_op(f)
            && (self.ast.described_template(f).is_some()
                || self.ast.described_template(self.ast.first(f)).is_some())
    }

    /// Whether function `f` is declared `[[noreturn]]`.
    fn decl_is_noreturn(&self, f: DeclId) -> bool {
        self.ast.is_noreturn_fn(f)
    }

    /// How control reaches the end of the function body `s` (clang
    /// `CheckFallThrough` over the body's graph; spec/sema.md §7.5.1):
    /// `Never` when it does not (or there is no graph), `Maybe` when the
    /// body also returns, throws out or calls a `noreturn` function on a
    /// live path, else `Always`.
    ///
    /// A body whose last statement is a `return` never does: control
    /// reaches the closing brace only through its last statement (no
    /// label follows it), so no graph is built for the question (WP-798:
    /// a graph is 8 bytes per expression, 24 MB for the 12,000 member
    /// accesses of bench/overload/arrow-chain.cpp).
    fn falls_off(&mut self, s: StmtId) -> Falloff {
        let st = &self.ast.stmts;
        if st.get(s).kind() == StmtKind::CompoundStmt
            && self
                .ast
                .lists
                .iter::<StmtId>(st.compound_body(s))
                .last()
                .is_some_and(|l| st.get(l).kind() == StmtKind::ReturnStmt)
        {
            return Falloff::Never;
        }
        match self.with_cfg(s, |_, c| c.exit_kinds()) {
            Some(k) if k.plain => {
                if k.ret || k.fake || k.abnormal {
                    Falloff::Maybe
                } else {
                    Falloff::Always
                }
            }
            _ => Falloff::Never,
        }
    }

    /// Whether a call of function `d` ends its block in the control-flow
    /// graph (clang 22 `CFGBuilder::VisitCallExpr`: `isNoReturn()` or GNU
    /// `analyzer_noreturn`, so -Wreturn-type does not fire after the call;
    /// clang 19 ignored the attribute outside the static analyzer). The
    /// attribute on a function-pointer variable does not count.
    fn cfg_noreturn_fn(&self, d: DeclId) -> bool {
        self.ast.is_noreturn_fn(d)
            || self.find_attr(d, kiln_ast::attr::AttrKind::ANALYZER_NORETURN).is_some()
    }

    pub(crate) fn is_noreturn_call(&self, e: ExprId) -> bool {
        let n = self.ast.exprs.get(e);
        if !matches!(
            n.kind(),
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr | ExprKind::CXXMemberCallExpr
        ) {
            return false;
        }
        let mut c = self.ast.exprs.callee(e);
        // clang `getFunctionExtInfo` of the callee's type: a call through
        // a `noreturn` function type (spec/types.md §5.4; a GNU `noreturn`
        // function pointer or reference) does not return.
        let ct = self.ast.exprs.get(c).ty();
        let t = &self.ast.types;
        let fnty = if t.is_function(ct) { Some(ct) } else { t.pointee(ct) };
        if fnty.and_then(|f| t.proto(f)).is_some_and(|p| p.info().noreturn()) {
            return true;
        }
        loop {
            let cn = self.ast.exprs.get(c);
            match cn.kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ParenExpr => c = ExprId::new(cn.a()),
                ExprKind::DeclRefExpr => {
                    let d = self.ast.exprs.decl(c);
                    // A `noreturn` builtin (`__builtin_unreachable`).
                    if let Some(b) = self.ast.decls.builtin_id(d) {
                        use kiln_ast::decl::builtin;
                        return builtin::info(b)
                            .is_some_and(|i| i.attrs & builtin::attr::NORETURN != 0);
                    }
                    return self.ast.decls.get(d).kind().is_function() && self.cfg_noreturn_fn(d);
                }
                // `s.m()`: clang's CFG uses the direct callee, virtual or
                // not.
                ExprKind::MemberExpr => {
                    let d = self.ast.exprs.decl(c);
                    return self.ast.decls.get(d).kind().is_function() && self.cfg_noreturn_fn(d);
                }
                _ => return false,
            }
        }
    }

    pub(crate) fn children(&self, s: StmtId) -> Vec<StmtId> {
        let st = &self.ast.stmts;
        let n = st.get(s);
        match n.kind() {
            StmtKind::CompoundStmt => self.ast.lists.iter(st.compound_body(s)).collect(),
            StmtKind::IfStmt => {
                let d = st.if_data(s);
                d.then.into_iter().chain(d.els).collect()
            }
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                st.loop_data(s).body.into_iter().collect()
            }
            StmtKind::CXXForRangeStmt => vec![st.range_for_data(s).body],
            StmtKind::CXXExpansionStmt => {
                let d = st.expansion_data(s);
                let l = &self.ast.lists;
                if st.expansion_expanded(s) {
                    d.init
                        .into_iter()
                        .chain(l.iter::<StmtId>(d.prelude))
                        .chain(l.iter::<StmtId>(d.expansions))
                        .collect()
                } else {
                    d.init.into_iter().chain([d.body]).collect()
                }
            }
            StmtKind::SwitchStmt => vec![st.switch_data(s).body],
            StmtKind::CaseStmt => vec![st.case_data(s).sub],
            StmtKind::DefaultStmt => vec![st.default_sub(s)],
            StmtKind::LabelStmt | StmtKind::AttributedStmt => vec![StmtId::new(n.b())],
            StmtKind::CXXTryStmt => std::iter::once(st.try_block(s))
                .chain(self.ast.lists.iter::<StmtId>(st.try_handlers(s)))
                .collect(),
            StmtKind::CXXCatchStmt => vec![st.catch_block(s)],
            _ => Vec::new(),
        }
    }
}

/// How control reaches the end of a function body (clang
/// `ControlFlowKind`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Falloff {
    /// Never (or clang builds no CFG).
    Never,
    /// On some path, others returning, throwing or ending in a call to a
    /// `noreturn` function.
    Maybe,
    /// On every path that leaves the body.
    Always,
}
