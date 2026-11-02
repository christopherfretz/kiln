//! The checks of a finished `switch` over its case list (clang
//! `ActOnFinishSwitchStmt`, spec/sema.md §7): a constant condition no
//! case matches, case values outside the condition's enumeration, the
//! enumerators no case handles (-Wswitch, -Wswitch-enum), a `default:`
//! of a switch that covers every enumerator (-Wcovered-switch-default),
//! a missing `default:` (-Wswitch-default) and a condition known to be
//! boolean (-Wswitch-bool). The enumeration coverage computed here also
//! decides the CFG edge to `default:` (clang `isAllEnumCasesCovered`) in
//! the reachability walk.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, CastKind, ExprDeps, ExprKind, UnaryOp};
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::stmt::SwitchState;

/// The folded case labels of one switch.
pub(crate) struct CaseList {
    /// `case v:` values with their label expressions, sorted by value.
    pub values: Vec<(i128, ExprId)>,
    /// `case lo ... hi:` ranges (non-empty ones) with their bound
    /// expressions, sorted by `lo`.
    pub ranges: Vec<(i128, i128, ExprId, ExprId)>,
    /// A label did not fold (clang `CaseListIsIncomplete`) or two
    /// labels overlap (`CaseListIsErroneous`).
    pub broken: bool,
}

impl CaseList {
    /// Whether some label handles the value `v`.
    pub fn handles(&self, v: i128) -> bool {
        self.values.binary_search_by_key(&v, |c| c.0).is_ok()
            || self.ranges.iter().any(|&(lo, hi, ..)| lo <= v && v <= hi)
    }
}

impl Sema<'_, '_> {
    /// The value of the converted case label `e` (a `ConstantExpr`).
    fn case_label_value(&mut self, e: ExprId) -> Option<i128> {
        if self.ast.exprs.get(e).kind() != ExprKind::ConstantExpr {
            return None;
        }
        self.fold_int(e)
    }

    /// The case labels among `cases` (`CaseStmt`s and `DefaultStmt`s).
    pub(crate) fn case_list(&mut self, cases: &[StmtId]) -> CaseList {
        let mut l = CaseList { values: Vec::new(), ranges: Vec::new(), broken: false };
        for &c in cases {
            if self.ast.stmts.get(c).kind() != StmtKind::CaseStmt {
                continue;
            }
            let d = *self.ast.stmts.case_data(c);
            let Some(lo) = self.case_label_value(d.lhs) else {
                l.broken = true;
                continue;
            };
            match d.rhs {
                None => l.values.push((lo, d.lhs)),
                Some(h) => match self.case_label_value(h) {
                    // An empty range is dropped (with a warning).
                    Some(hi) if lo > hi => {}
                    Some(hi) => l.ranges.push((lo, hi, d.lhs, h)),
                    None => l.broken = true,
                },
            }
        }
        l.values.sort_by_key(|v| v.0);
        l.ranges.sort_by_key(|r| r.0);
        if l.values.windows(2).any(|w| w[0].0 == w[1].0)
            || l.ranges.windows(2).any(|w| w[1].0 <= w[0].1)
            || l.ranges.iter().any(|&(lo, hi, ..)| l.values.iter().any(|v| lo <= v.0 && v.0 <= hi))
        {
            l.broken = true;
        }
        l
    }

    /// The complete enumeration of the switch condition `cond` before
    /// its integral promotion (clang `GetTypeBeforeIntegralPromotion`:
    /// full-expressions and implicit integral casts stripped), when it
    /// has enumerators.
    pub(crate) fn switch_cond_enum(&self, cond: ExprId) -> Option<DeclId> {
        let ex = &self.ast.exprs;
        let mut e = cond;
        while matches!(ex.get(e).kind(), ExprKind::ExprWithCleanups | ExprKind::ConstantExpr) {
            e = ex.operand(e);
        }
        while ex.get(e).kind() == ExprKind::ImplicitCastExpr
            && ex.cast_kind(e) == CastKind::IntegralCast
        {
            e = ex.operand(e);
        }
        let t = &self.ast.types;
        let def = t.as_enum(t.canonical(ex.get(e).ty())).and_then(|d| self.ast.definition(d))?;
        let any = self.ast.decls.members(def).any(|m| self.kind_of(m) == DeclKind::EnumConstant);
        any.then_some(def)
    }

    /// The enumerators of `def` with their values, sorted by value, the
    /// first of equal values kept (clang `EnumVals`).
    fn enum_vals(&self, def: DeclId) -> Vec<(i128, DeclId)> {
        let mut vals: Vec<(i128, DeclId)> = self
            .ast
            .decls
            .members(def)
            .filter(|&m| self.kind_of(m) == DeclKind::EnumConstant)
            .filter_map(|m| {
                let v = self.ast.decls.enumerator(m).and_then(|x| x.value)?;
                Some((self.const_int(v)?, m))
            })
            .collect();
        vals.sort_by_key(|v| v.0);
        vals.dedup_by_key(|v| v.0);
        vals
    }

    /// The enumerators of `def` no label of `cases` handles, in value
    /// order; a `[[maybe_unused]]` enumerator may be omitted, and below
    /// `-fclang-compat=22` a deprecated one too (clang 22 warns about an
    /// unhandled deprecated enumerator).
    pub(crate) fn unhandled_enumerators(&self, def: DeclId, cases: &CaseList) -> Vec<DeclId> {
        let deprecated_exempt = self.clang_compat < 22;
        self.enum_vals(def)
            .into_iter()
            .filter(|&(_, m)| {
                !(deprecated_exempt && self.find_attr(m, AttrKind::DEPRECATED).is_some())
                    && self.find_attr(m, AttrKind::UNUSED).is_none()
                    && self.find_attr(m, AttrKind::MAYBE_UNUSED).is_none()
            })
            .filter(|&(v, _)| !cases.handles(v))
            .map(|(_, m)| m)
            .collect()
    }

    /// clang `SwitchStmt::isAllEnumCasesCovered` (and a non-empty case
    /// list) for the switch `cond` with labels `cases`: the enumeration
    /// checks ran, finding every enumerator handled. They are skipped for
    /// a broken case list and for a constant condition without a
    /// `default:`.
    pub(crate) fn switch_covers_enum(
        &mut self,
        cond: ExprId,
        cases: &[StmtId],
        has_default: bool,
    ) -> bool {
        if !cases.iter().any(|&c| self.ast.stmts.get(c).kind() == StmtKind::CaseStmt) {
            return false;
        }
        let l = self.case_list(cases);
        if l.broken || (!has_default && self.fold_rvalue_int(cond).is_some()) {
            return false;
        }
        self.switch_cond_enum(cond)
            .is_some_and(|def| self.unhandled_enumerators(def, &l).is_empty())
    }

    /// The checks of the finished switch `sw` on `cond` (clang
    /// `ActOnFinishSwitchStmt`), `loc` the `switch` keyword.
    pub(crate) fn check_switch_cases(&mut self, sw: &SwitchState, cond: ExprId, loc: Loc) {
        let deps = self.ast.exprs.get(cond).deps();
        if sw.invalid
            || deps.intersects(ExprDeps::TYPE | ExprDeps::VALUE | ExprDeps::ERROR)
            || self.has_dependent_case_label(&sw.cases)
        {
            return;
        }
        let default = sw.default;
        if default.is_none() {
            self.emit_switch(self.diag(DiagId::sema_switch_missing_default, loc));
        }
        let cases = self.case_list(&sw.cases);
        if cases.broken {
            return;
        }
        let cond_loc = self.expr_loc(cond);
        if default.is_none()
            && let Some(v) = self.fold_rvalue_int(cond)
        {
            if !cases.handles(v) {
                let d = self.diag(DiagId::sema_switch_missing_case_for_condition, cond_loc);
                self.emit_switch(d.arg(v.to_string()));
            }
            return;
        }
        let Some(def) = self.switch_cond_enum(cond) else { return };
        let vals = self.enum_vals(def);
        let ty = self.ast.types.canonical(self.tag_type(def));
        let ty_arg = self.ty_arg(ty);
        let mut not_in_enum = Vec::new();
        for &(v, e) in &cases.values {
            not_in_enum.push((v, e));
        }
        for &(lo, hi, le, he) in &cases.ranges {
            not_in_enum.push((lo, le));
            not_in_enum.push((hi, he));
        }
        // clang `ShouldDiagnoseSwitchCaseNotInEnum`: an open enumeration
        // takes any value; a flag enumeration any combination of the bits
        // of its single-bit enumerators.
        let closed = self.enum_is_closed(def);
        let flag_bits = self.find_attr(def, AttrKind::FLAG_ENUM).is_some().then(|| {
            vals.iter().filter(|x| x.0 > 0 && x.0 & (x.0 - 1) == 0).fold(0, |b, x| b | x.0)
        });
        for (v, e) in not_in_enum {
            let in_enum = match flag_bits {
                Some(bits) => v & !bits == 0,
                None => vals.binary_search_by_key(&v, |x| x.0).is_ok(),
            };
            if closed && !in_enum && !self.names_const_enum_var(e, def) {
                let d = self.diag(DiagId::sema_switch_not_in_enum, self.begin_loc(e));
                self.emit_switch(d.arg(ty_arg.clone()));
            }
        }
        let unhandled = self.unhandled_enumerators(def, &cases);
        if unhandled.is_empty() {
            // clang `isClosedNonFlag`.
            if let Some(dl) = default
                && closed
                && flag_bits.is_none()
            {
                self.emit_switch(self.diag(DiagId::sema_switch_unreachable_default, dl));
            }
            return;
        }
        let id = if default.is_some() {
            DiagId::sema_switch_def_missing_case
        } else {
            DiagId::sema_switch_missing_case
        };
        let n = unhandled.len();
        let mut d = self.diag(id, cond_loc).arg((n.min(4) - 1).to_string());
        for i in 0..3 {
            d = d.arg(unhandled.get(i).map_or_else(String::new, |&m| self.decl_arg(m)));
        }
        self.emit_switch(d.arg(n.to_string()));
    }

    /// `def` is not `enum_extensibility(open)` (clang `EnumDecl::isClosed`).
    fn enum_is_closed(&self, def: DeclId) -> bool {
        self.find_attr(def, AttrKind::ENUM_EXTENSIBILITY)
            .is_none_or(|a| self.ast.attrs.get(a).a == 0)
    }

    /// A `case` label among `cases` is value-dependent (clang
    /// `HasDependentValue`): none of the checks run, -Wswitch-default
    /// included, until the instantiation.
    fn has_dependent_case_label(&self, cases: &[StmtId]) -> bool {
        let dependent =
            |e: ExprId| self.ast.exprs.get(e).deps().intersects(ExprDeps::TYPE | ExprDeps::VALUE);
        cases.iter().any(|&c| {
            self.ast.stmts.get(c).kind() == StmtKind::CaseStmt && {
                let d = *self.ast.stmts.case_data(c);
                dependent(d.lhs) || d.rhs.is_some_and(dependent)
            }
        })
    }

    /// clang checks the case list again in each instantiation: its
    /// warnings repeat there.
    fn emit_switch(&self, d: kiln_diag::Diag) {
        self.emit(d.allow_duplicate());
    }

    /// The case label `e` names a `const` variable with static storage
    /// of the enumeration type `def` (exempt from -Wswitch).
    fn names_const_enum_var(&self, e: ExprId, def: DeclId) -> bool {
        let e = self.ignore_paren_imp_casts(e);
        if self.ast.exprs.get(e).kind() != ExprKind::DeclRefExpr {
            return false;
        }
        let v = self.ast.exprs.decl(e);
        let Some(var) = self.ast.decls.var(v) else { return false };
        let t = &self.ast.types;
        let ty = self.ast.decls.get(v).ty().unwrap_or(kiln_ast::QualType::ERROR);
        let global = self.has_static_storage(v) || var.spec.tls() != kiln_ast::decl::TlsKind::None;
        global
            && t.effective_quals(ty).contains(kiln_ast::types::Quals::CONST)
            && t.as_enum(t.canonical(ty)).and_then(|d| self.ast.definition(d)) == Some(def)
    }

    /// -Wswitch-bool: the switch condition `cond` (before promotion) is
    /// known to have a boolean value, `loc` the `switch` keyword.
    pub(crate) fn check_switch_bool(&mut self, cond: ExprId, loc: Loc) {
        if self.known_boolean(cond) {
            self.emit_switch(self.diag(DiagId::sema_switch_bool_condition, loc));
        }
    }

    /// clang `Expr::isKnownToHaveBooleanValue` (semantic).
    fn known_boolean(&self, e: ExprId) -> bool {
        let e = self.ignore_parens(e);
        let ex = &self.ast.exprs;
        let t = &self.ast.types;
        let ty = t.canonical(ex.get(e).ty());
        if t.builtin(ty) == Some(kiln_ast::types::BuiltinType::Bool) {
            return true;
        }
        if !(t.is_integral(ty) || t.as_enum(ty).is_some()) {
            return false;
        }
        match ex.get(e).kind() {
            ExprKind::UnaryOperator => match ex.unary_op(e) {
                UnaryOp::Plus => self.known_boolean(ex.operand(e)),
                UnaryOp::LNot => true,
                _ => false,
            },
            ExprKind::ImplicitCastExpr => self.known_boolean(ex.operand(e)),
            ExprKind::BinaryOperator => {
                let [l, r] = ex.operands(e);
                match ex.binary_op(e) {
                    BinaryOp::LT
                    | BinaryOp::GT
                    | BinaryOp::LE
                    | BinaryOp::GE
                    | BinaryOp::EQ
                    | BinaryOp::NE
                    | BinaryOp::LAnd
                    | BinaryOp::LOr => true,
                    BinaryOp::And | BinaryOp::Xor | BinaryOp::Or => {
                        self.known_boolean(l) && self.known_boolean(r)
                    }
                    BinaryOp::Comma | BinaryOp::Assign => self.known_boolean(r),
                    _ => false,
                }
            }
            ExprKind::ConditionalOperator => {
                let kids = ex.children(e, &self.ast.lists);
                self.known_boolean(kids[1]) && self.known_boolean(kids[2])
            }
            _ => false,
        }
    }
}
