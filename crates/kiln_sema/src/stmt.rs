//! Statements ([stmt]): compound statements and their scopes, selection
//! and iteration statements, `case`/`default`, jumps, labels and `goto`,
//! `return` (spec/sema.md §7).

use kiln_ast::decl::{DeclFlags, DeclKind, DeclMisc, NewDecl, Payload};
use kiln_ast::expr::{CastKind, ExprDeps, ExprKind, ValueCategory};
use kiln_ast::names::DeclName;
use kiln_ast::stmt::{CaseData, IfData, IfKind, LoopData, StmtKind, SwitchData};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv::{self, ConvCtx};
use crate::scope::{Scope, ScopeKind};

/// A label of the current function.
#[derive(Clone, Debug)]
pub(crate) struct LabelInfo {
    pub name: Symbol,
    pub decl: DeclId,
    /// Where it was defined (`None` while only used by `goto`).
    pub defined: Option<Loc>,
    /// The first `goto` naming it.
    pub first_use: Option<Loc>,
}

/// One enclosing `switch`.
#[derive(Clone, Debug)]
pub(crate) struct SwitchState {
    /// The promoted condition type.
    pub cond_ty: QualType,
    /// Case values seen, with their locations.
    pub values: Vec<(i128, Loc)>,
    /// The `CaseStmt`/`DefaultStmt`s, in source order.
    pub cases: Vec<StmtId>,
    /// The first `default:`.
    pub default: Option<Loc>,
    /// Set when the condition was invalid (suppresses case checks).
    pub invalid: bool,
}

/// Per-function state while its body is analysed.
#[derive(Clone, Debug)]
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
    /// Enclosing loops and switches (innermost last; `true` = loop).
    pub(crate) breakables: Vec<bool>,
    /// The type of `this` in this function (`cv C*`), `None` outside a
    /// non-static member function (spec/sema-class.md §2.5).
    pub(crate) this_ty: Option<QualType>,
    /// A `this` type that overrides `this_ty` while parsing an NSDMI or a
    /// trailing return type (clang `CXXThisScopeRAII`).
    pub(crate) this_override: Option<QualType>,
}

impl FnState {
    pub(crate) fn new(decl: DeclId, ret: QualType) -> Self {
        FnState {
            decl,
            ret,
            auto_ret: None,
            deduced: false,
            labels: Vec::new(),
            switches: Vec::new(),
            breakables: Vec::new(),
            this_ty: None,
            this_override: None,
        }
    }
}

impl Sema<'_, '_> {
    // -- scopes -----------------------------------------------------------

    /// Enters a scope of `kind` (block scopes for compound statements and
    /// the implicit scopes of selection/iteration statements).
    pub fn push_scope(&mut self, kind: ScopeKind) {
        self.scopes.push(Scope::new(kind, None));
    }

    /// Leaves the innermost scope.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    // -- simple statements ------------------------------------------------

    /// `{ stmts }` ([stmt.block]).
    pub fn act_on_compound(&mut self, stmts: &[StmtId], lbrace: Loc, rbrace: Loc) -> StmtId {
        self.builder().compound(stmts, lbrace, rbrace)
    }

    /// `;`.
    pub fn act_on_null_stmt(&mut self, loc: Loc) -> StmtId {
        self.builder().null_stmt(loc)
    }

    /// A declaration statement ([stmt.dcl]).
    pub fn act_on_decl_stmt(&mut self, decls: &[DeclId], loc: Loc) -> StmtId {
        self.builder().decl_stmt(decls, loc)
    }

    /// An expression statement ([stmt.expr]): `e` is a discarded-value
    /// full-expression.
    pub fn act_on_expr_stmt(&mut self, e: ExprId) -> StmtId {
        if !self.check_builtin_fn_use(e) {
            self.diagnose_unused(e);
        }
        let e = self.act_on_full_expr(e);
        self.builder().expr_stmt(e)
    }

    /// A condition ([stmt.pre]/4): contextually converted to `bool`.
    pub fn act_on_condition(&mut self, e: ExprId) -> ExprId {
        let e = match conv::contextually_convert_to_bool(self, e) {
            Ok(c) => c,
            Err(err) => {
                let loc = self.ast.exprs.get(e).loc();
                let mut d = self.diag(err.id, loc);
                for a in err.args {
                    d = d.arg(a);
                }
                self.emit(d);
                e
            }
        };
        self.act_on_full_expr(e)
    }

    /// A condition declaration ([stmt.pre]/4): the variable `v` (already
    /// initialized) yields the condition value, a reference to `v`.
    pub fn act_on_condition_var(&mut self, v: DeclId, loc: Loc) -> (Option<DeclId>, ExprId) {
        (Some(v), self.build_decl_ref(v, loc))
    }

    /// The condition of `if constexpr` ([stmt.if]/2): contextually
    /// converted to `bool` (narrowing allowed since P1401), then
    /// evaluated by the Oven (`EvalKind::IfConstexpr`) and wrapped in a
    /// `ConstantExpr` holding the value. A condition that is not a
    /// constant expression is diagnosed and kept unwrapped.
    pub fn act_on_constexpr_condition(&mut self, e: ExprId) -> ExprId {
        let at = self.begin_loc(e);
        let c = self.act_on_condition(e);
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
            Err(f) => {
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
        let data = IfData { init, cond_var, cond, then: Some(then), els };
        self.builder().if_stmt(kind, data, loc)
    }

    /// Enters an immediate function context ([expr.const]/16): the
    /// compound statement of `if consteval` (or the `else` of
    /// `if !consteval`). M2 has no consteval functions, so nothing
    /// consults it yet beyond [`Sema::in_immediate_context`].
    pub fn enter_immediate_context(&mut self) {
        self.immediate += 1;
    }

    /// Leaves an immediate function context.
    pub fn exit_immediate_context(&mut self) {
        self.immediate = self.immediate.saturating_sub(1);
    }

    /// True inside an immediate function context entered with
    /// [`Sema::enter_immediate_context`].
    pub fn in_immediate_context(&self) -> bool {
        self.immediate > 0
    }

    /// Marks the start of a loop body (for `break`/`continue`).
    pub fn act_on_start_loop(&mut self) {
        if let Some(f) = self.func.as_mut() {
            f.breakables.push(true);
        }
    }

    /// Marks the end of a loop body.
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
    pub fn act_on_do(&mut self, body: StmtId, cond: ExprId, loc: Loc) -> StmtId {
        self.builder().do_stmt(body, cond, loc)
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
            self.act_on_full_expr(e)
        });
        let data = LoopData { init, cond_var, cond, inc, body: Some(body) };
        self.builder().for_stmt(data, loc)
    }

    // -- switch -------------------------------------------------------------

    /// The condition of a `switch` ([stmt.switch]/2): converted to an
    /// integral type and promoted. Starts the switch body. `loc` is the
    /// `switch` keyword, where clang reports a non-integral condition.
    pub fn act_on_start_switch(&mut self, cond: ExprId, loc: Loc) -> ExprId {
        let ty0 = self.ast.exprs.get(cond).ty();
        let t = &self.ast.types;
        let ok = t.is_integral(ty0)
            || t.as_enum(ty0).is_some()
            || t.builtin(ty0) == Some(BuiltinType::Error);
        let (cond, cond_ty, invalid) = if ok {
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
            f.switches.push(SwitchState {
                cond_ty,
                values: Vec::new(),
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
                f.switches.pop().map(|s| s.cases).unwrap_or_default()
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
        let Some(sw) = self.func.as_ref().and_then(|f| f.switches.last()) else {
            self.error(DiagId::sema_case_not_in_switch, case_loc);
            return e;
        };
        let (cond_ty, invalid) = (sw.cond_ty, sw.invalid);
        if self.expr_is_value_dependent(e) || self.is_dependent_type(cond_ty) {
            return e;
        }
        let loc = self.ast.exprs.get(e).loc();
        let conv = match conv::implicit_convert(self, e, cond_ty, ConvCtx::Initialization) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, loc);
                return e;
            }
        };
        let at = self.begin_loc(e);
        match self.eval_const(conv, kiln_oven::EvalContext::converted(at)) {
            Ok(v) => {
                let iv = self.const_int(v).unwrap_or(0);
                let cv = self.int_const(cond_ty, iv);
                let ce = self.builder().constant_expr(conv, Some(cv));
                let ce = self.act_on_full_expr(ce);
                if !invalid {
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
                }
                ce
            }
            Err(f) => {
                // [stmt.switch]/2: a converted constant expression (clang's
                // `CheckConvertedConstantExpression`, CCEK_CaseValue).
                let msg = self.diag(DiagId::sema_expr_not_cce, loc).arg("case value");
                let d = Self::with_failure_notes(msg, f);
                self.emit(d);
                self.act_on_full_expr(conv)
            }
        }
    }

    /// `case value: sub`.
    pub fn act_on_case(&mut self, value: ExprId, sub: StmtId, loc: Loc) -> StmtId {
        let s = self.builder().case_stmt(CaseData { lhs: value, rhs: None, sub }, loc);
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

    /// `continue;` ([stmt.cont]).
    pub fn act_on_continue(&mut self, loc: Loc) -> StmtId {
        if self.func.as_ref().is_none_or(|f| !f.breakables.contains(&true)) {
            self.error(DiagId::sema_continue_not_in_loop, loc);
        }
        self.builder().continue_stmt(loc)
    }

    fn label_decl(&mut self, name: Symbol, loc: Loc) -> DeclId {
        if let Some(l) = self.func.as_ref().and_then(|f| f.labels.iter().find(|l| l.name == name)) {
            return l.decl;
        }
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
            f.labels.push(LabelInfo { name, decl: d, defined: None, first_use: None });
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
            .and_then(|l| l.defined);
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

    /// `return e;` ([stmt.return]).
    pub fn act_on_return(&mut self, value: Option<ExprId>, loc: Loc) -> StmtId {
        let Some((fdecl, mut ret)) = self.func.as_ref().map(|f| (f.decl, f.ret)) else {
            return self.builder().return_stmt(value, None, loc);
        };
        if self.func.as_ref().is_some_and(|f| f.auto_ret.is_some()) {
            ret = self.deduce_return(value, loc);
        }
        let ret_void = self.ast.types.is_void(ret);
        let fname = self.decl_name(fdecl);
        let value = match value {
            None => {
                if !ret_void && !self.is_error_type(ret) {
                    let d = self.diag(DiagId::sema_return_missing_expr, loc).arg(fname);
                    self.emit(d);
                }
                None
            }
            Some(e) => {
                let ety = self.ast.exprs.get(e).ty();
                if ret_void {
                    if !self.ast.types.is_void(ety) && !self.is_error_type(ety) {
                        let d = self.diag(DiagId::sema_return_has_expr, loc).arg(fname);
                        self.emit(d);
                    }
                    Some(self.act_on_full_expr(e))
                } else if self.ast.types.is_void(ety) {
                    let d = self.diag(DiagId::sema_return_void_expr_in_nonvoid, loc).arg(fname);
                    self.emit(d);
                    Some(self.act_on_full_expr(e))
                } else {
                    let e = self.implicit_move(e);
                    let c = self.convert_for_init(e, ret, ConvCtx::Return);
                    Some(self.act_on_full_expr(c))
                }
            }
        };
        self.builder().return_stmt(value, None, loc)
    }

    /// C++23 implicit move ([expr.prim.id.unqual]/4, [class.copy.elision]/3):
    /// an id-expression naming a non-volatile object of automatic storage
    /// duration declared in the function becomes an xvalue.
    pub(crate) fn implicit_move(&mut self, e: ExprId) -> ExprId {
        // A possibly parenthesized id-expression.
        let mut id = e;
        while self.ast.exprs.get(id).kind() == ExprKind::ParenExpr {
            id = self.ast.exprs.operand(id);
        }
        let n = *self.ast.exprs.get(id);
        if n.kind() != ExprKind::DeclRefExpr {
            return e;
        }
        let d = self.ast.exprs.decl(id);
        let dn = *self.ast.decls.get(d);
        let local = match dn.kind() {
            DeclKind::ParmVar => true,
            DeclKind::Var => {
                self.ast.decls.var(d).is_some_and(|v| {
                    v.spec.storage() == kiln_ast::decl::VarStorage::None
                        || v.spec.storage() == kiln_ast::decl::VarStorage::Register
                }) && dn.ctx().is_some_and(|c| self.ast.decls.get(c).kind().is_function())
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
        if self.contains_undeduced_auto(ret) {
            state.auto_ret = Some(ret);
        }
        self.func = Some(state);
    }

    /// Ends the body: undefined labels and fall-off-the-end checks.
    pub(crate) fn end_function_body(&mut self, body: StmtId, rbrace: Loc) {
        // No return statement: the placeholder deduces `void`
        // ([dcl.spec.auto.general]/10).
        if let Some(st) = self.func.as_ref()
            && st.auto_ret.is_some()
            && !st.deduced
        {
            let f = st.decl;
            self.set_deduced_return(f, QualType::VOID);
        }
        let Some(state) = self.func.take() else { return };
        for l in &state.labels {
            if l.defined.is_none() {
                let msg = self
                    .diag(DiagId::sema_undeclared_label, l.first_use.unwrap_or(rbrace))
                    .arg(self.ast.symbols().as_str(l.name).to_string());
                self.emit(msg);
            }
        }
        let ret = state.ret;
        if self.ast.types.is_void(ret) || self.is_error_type(ret) {
            return;
        }
        let is_main = self.decl_name(state.decl) == "main"
            && self
                .ast
                .decls
                .get(state.decl)
                .ctx()
                .is_some_and(|c| self.ast.redecl_context(c) == self.ast.tu);
        // clang skips its analysis-based warnings once an error occurred
        // (`hasUncompilableErrorOccurred`).
        if is_main || self.diags.error_count() > 0 {
            return;
        }
        match self.falls_off(body) {
            Falloff::Never => {}
            Falloff::Always => self.error(DiagId::sema_falloff_nonvoid, rbrace),
            Falloff::Maybe => self.error(DiagId::sema_falloff_nonvoid_maybe, rbrace),
        }
    }

    /// Whether control can reach the end of `s` (a structural
    /// approximation of clang's CFG-based `-Wreturn-type`).
    fn falls_off(&mut self, s: StmtId) -> Falloff {
        let n = *self.ast.stmts.get(s);
        match n.kind() {
            StmtKind::ReturnStmt | StmtKind::GotoStmt => Falloff::Never,
            StmtKind::CompoundStmt => {
                let list = self.ast.stmts.compound_body(s);
                let items: Vec<StmtId> = self.ast.lists.iter(list).collect();
                let mut result = Falloff::Always;
                for &c in &items {
                    match self.falls_off(c) {
                        Falloff::Never => return Falloff::Never,
                        Falloff::Maybe => result = Falloff::Maybe,
                        Falloff::Always => {}
                    }
                }
                result
            }
            StmtKind::IfStmt => {
                let d = *self.ast.stmts.if_data(s);
                let then = d.then.map_or(Falloff::Always, |t| self.falls_off(t));
                let els = d.els.map_or(Falloff::Always, |e| self.falls_off(e));
                // A constant condition selects one branch.
                if let Some(c) = d.cond
                    && let Some(v) = self.try_fold(c).and_then(|v| self.const_truth(v))
                {
                    return if v { then } else { els };
                }
                match (then, els) {
                    (Falloff::Never, Falloff::Never) => Falloff::Never,
                    (Falloff::Always, Falloff::Always) => Falloff::Always,
                    _ => Falloff::Maybe,
                }
            }
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                let d = *self.ast.stmts.loop_data(s);
                let infinite = match d.cond {
                    None => true,
                    Some(c) => self.try_fold(c).and_then(|v| self.const_truth(v)) == Some(true),
                };
                let breaks = d.body.is_some_and(|b| self.contains_break(b, 0));
                let body_never = n.kind() == StmtKind::DoStmt
                    && d.body.is_some_and(|b| self.falls_off(b) == Falloff::Never);
                if (infinite || body_never) && !breaks {
                    Falloff::Never
                } else {
                    Falloff::Maybe.min_with(Falloff::Always, d.body.map(|b| self.has_return(b)))
                }
            }
            StmtKind::LabelStmt => self.falls_off(StmtId::new(n.b())),
            StmtKind::CaseStmt => self.falls_off(self.ast.stmts.case_data(s).sub),
            StmtKind::DefaultStmt => self.falls_off(self.ast.stmts.default_sub(s)),
            StmtKind::SwitchStmt => {
                let d = *self.ast.stmts.switch_data(s);
                let has_default = self
                    .ast
                    .lists
                    .iter::<StmtId>(d.cases)
                    .any(|c| self.ast.stmts.get(c).kind() == StmtKind::DefaultStmt);
                let body = self.falls_off(d.body);
                if has_default && body == Falloff::Never && !self.contains_break(d.body, 0) {
                    Falloff::Never
                } else if self.has_return(d.body) {
                    Falloff::Maybe
                } else {
                    Falloff::Always
                }
            }
            StmtKind::ExprStmt => {
                let e = self.ast.stmts.expr_stmt_expr(s);
                if self.is_noreturn_call(e) { Falloff::Never } else { Falloff::Always }
            }
            _ => Falloff::Always,
        }
    }

    fn is_noreturn_call(&self, e: ExprId) -> bool {
        let n = self.ast.exprs.get(e);
        if n.kind() != ExprKind::CallExpr {
            return false;
        }
        let mut c = self.ast.exprs.callee(e);
        loop {
            let cn = self.ast.exprs.get(c);
            match cn.kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ParenExpr => c = ExprId::new(cn.a()),
                ExprKind::DeclRefExpr => {
                    let d = self.ast.exprs.decl(c);
                    let attrs = self.ast.attrs.of_decl(d);
                    return self
                        .ast
                        .lists
                        .iter::<kiln_ast::AttrId>(attrs)
                        .any(|a| self.ast.attrs.get(a).kind == kiln_ast::attr::AttrKind::NORETURN);
                }
                _ => return false,
            }
        }
    }

    fn children(&self, s: StmtId) -> Vec<StmtId> {
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
            StmtKind::SwitchStmt => vec![st.switch_data(s).body],
            StmtKind::CaseStmt => vec![st.case_data(s).sub],
            StmtKind::DefaultStmt => vec![st.default_sub(s)],
            StmtKind::LabelStmt => vec![StmtId::new(n.b())],
            _ => Vec::new(),
        }
    }

    /// True if `s` contains a `break` that leaves the statement it is
    /// nested in at `depth` 0.
    fn contains_break(&self, s: StmtId, depth: u32) -> bool {
        let k = self.ast.stmts.get(s).kind();
        if k == StmtKind::BreakStmt {
            return depth == 0;
        }
        let nested = matches!(
            k,
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt | StmtKind::SwitchStmt
        );
        let d = if nested { depth + 1 } else { depth };
        self.children(s).into_iter().any(|c| self.contains_break(c, d))
    }

    fn has_return(&self, s: StmtId) -> bool {
        self.ast.stmts.get(s).kind() == StmtKind::ReturnStmt
            || self.children(s).into_iter().any(|c| self.has_return(c))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Falloff {
    Never,
    Maybe,
    Always,
}

impl Falloff {
    /// A loop body: `Maybe` when it contains a `return`, else `Always`.
    fn min_with(self, other: Falloff, has_return: Option<bool>) -> Falloff {
        if has_return == Some(true) { self } else { other }
    }
}
