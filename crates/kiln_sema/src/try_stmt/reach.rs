//! Reachability of the throw-expressions that escape a non-throwing
//! function (clang's `checkThrowInNonThrowingFunc` walks the reachable
//! blocks of the body's CFG, built with `PruneTriviallyFalseEdges`). kiln
//! has no CFG; this is a structured walk over the body that mirrors the
//! CFG's edges (spec/sema.md §7.5):
//!
//! - `return`, `break`, `continue`, `goto`, a throw-expression and a call
//!   to a `noreturn` function end the live path;
//! - a condition that folds to a constant (clang `tryEvaluateBool`, with
//!   `&&`/`||` decided by one known side) prunes the other edge: `if`,
//!   loops, `?:` and the right operand of `&&`/`||`;
//! - a switch on a constant reaches only the matching case (else the
//!   default); with no default, the code after a switch over an
//!   enumeration whose enumerators all have cases is unreachable;
//! - a handler is reachable only from a reachable throw-expression in its
//!   try-block (or in a nested try-block without a catch-all): calls get
//!   no exception edges;
//! - labels and loop conditions reached by a later `goto` or back edge
//!   are found by iterating to a fixed point;
//! - an `if consteval` anywhere in the body: clang builds no CFG and
//!   checks nothing.

use std::collections::HashSet;

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprKind, UnaryOp};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::{DeclId, ExprId, StmtId};

use crate::Sema;

/// The state of one walk.
#[derive(Default)]
struct Reach {
    /// Labels a live `goto` reaches (grows across passes).
    labels: HashSet<DeclId>,
    /// Loops whose condition a back edge reaches (grows across passes).
    loops: HashSet<StmtId>,
    /// A pass grew `labels` or `loops`.
    changed: bool,
    /// The enclosing try statements, innermost last, and whether a live
    /// throw reaches each one's handlers.
    tries: Vec<(StmtId, bool)>,
    /// The enclosing loops and switches, innermost last.
    jumps: Vec<Jump>,
    /// The enclosing switches, innermost last.
    switches: Vec<SwitchFrame>,
    /// The live throw-expressions no enclosing handler catches.
    found: Vec<ExprId>,
    /// An `if consteval` was seen.
    no_cfg: bool,
}

struct Jump {
    is_loop: bool,
    broke: bool,
    continued: bool,
}

struct SwitchFrame {
    /// The condition is reached.
    live: bool,
    /// The condition's constant value.
    value: Option<i128>,
    /// `value` matches a case.
    matched: bool,
}

impl Sema<'_, '_> {
    /// The reachable throw-expressions of `body` that escape it; `None`
    /// when clang would build no CFG.
    pub(super) fn reachable_escaping_throws(&mut self, body: StmtId) -> Option<Vec<ExprId>> {
        let mut r = Reach::default();
        loop {
            r.changed = false;
            r.found.clear();
            self.reach_stmt(body, true, &mut r);
            if r.no_cfg {
                return None;
            }
            if !r.changed {
                return Some(r.found);
            }
        }
    }

    fn known_truth(&mut self, e: ExprId) -> Option<bool> {
        let n = self.ast.exprs.get(e);
        if n.kind() == ExprKind::BinaryOperator {
            let op = self.ast.exprs.binary_op(e);
            if matches!(op, BinaryOp::LAnd | BinaryOp::LOr) {
                let or = op == BinaryOp::LOr;
                let [l, rr] = self.ast.exprs.operands(e);
                let lv = self.known_truth(l);
                if lv == Some(or) {
                    return lv;
                }
                let rv = self.known_truth(rr);
                return match (lv, rv) {
                    (Some(a), Some(b)) => Some(if or { a || b } else { a && b }),
                    (None, Some(b)) if b == or => Some(b),
                    (None, None) => self.incorrect_logic_op(l, rr, or),
                    _ => None,
                };
            }
        }
        self.try_fold(e).and_then(|v| self.const_truth(v))
    }

    /// clang `checkIncorrectLogicOperator`: `x OP1 c1 && x OP2 c2` (or
    /// `||`, either side flipped) with integer-literal or same-enumeration
    /// constants is decided when it has one value at the minimum, at
    /// `c1`, just past the smaller constant, at `c2` and at the maximum.
    fn incorrect_logic_op(&mut self, l: ExprId, r: ExprId, or: bool) -> Option<bool> {
        let (x1, op1, c1) = self.normalized_comparison(l)?;
        let (x2, op2, c2) = self.normalized_comparison(r)?;
        if x1 != x2 {
            return None;
        }
        let (e1, e2) = (self.enum_constant(c1), self.enum_constant(c2));
        match (e1, e2) {
            (None, None) => {}
            (Some(a), Some(b)) if self.ast.decls.get(a).ctx() == self.ast.decls.get(b).ctx() => {}
            _ => return None,
        }
        let (t1, t2) = (self.ast.exprs.get(c1).ty(), self.ast.exprs.get(c2).ty());
        let (w, signed) =
            (crate::conv::rank::int_width(self, t1)?, crate::conv::rank::int_signed(self, t1));
        if crate::conv::rank::int_width(self, t2) != Some(w)
            || crate::conv::rank::int_signed(self, t2) != signed
            || w >= 127
        {
            return None;
        }
        let (v1, v2) = (self.fold_int(c1)?, self.fold_int(c2)?);
        let (min, max) = if signed {
            (-(1i128 << (w - 1)), (1i128 << (w - 1)) - 1)
        } else {
            (0, (1i128 << w) - 1)
        };
        let (mut always_true, mut always_false) = (true, true);
        for v in [min, v1, v1.min(v2) + 1, v2, max] {
            let a = compare(op1, v, v1)?;
            let b = compare(op2, v, v2)?;
            let res = if or { a || b } else { a && b };
            always_true &= res;
            always_false &= !res;
        }
        if always_true || always_false { Some(always_true) } else { None }
    }

    /// `x OP c` or `c OP x` (flipped) for a comparison `e` of a variable
    /// with an integer literal or enumerator: `(x's decl, OP, c)`.
    fn normalized_comparison(&self, e: ExprId) -> Option<(DeclId, BinaryOp, ExprId)> {
        let e = self.skip_parens(e);
        if self.ast.exprs.get(e).kind() != ExprKind::BinaryOperator {
            return None;
        }
        let op = self.ast.exprs.binary_op(e);
        if !matches!(
            op,
            BinaryOp::EQ | BinaryOp::NE | BinaryOp::LT | BinaryOp::GT | BinaryOp::LE | BinaryOp::GE
        ) {
            return None;
        }
        let [a, b] = self.ast.exprs.operands(e);
        let (x, op, c) = if let Some(c) = self.int_or_enum_constant(b) {
            (a, op, c)
        } else {
            let flipped = match op {
                BinaryOp::LT => BinaryOp::GT,
                BinaryOp::GT => BinaryOp::LT,
                BinaryOp::LE => BinaryOp::GE,
                BinaryOp::GE => BinaryOp::LE,
                o => o,
            };
            (b, flipped, self.int_or_enum_constant(a)?)
        };
        let mut x = x;
        while matches!(
            self.ast.exprs.get(x).kind(),
            ExprKind::ImplicitCastExpr | ExprKind::ParenExpr
        ) {
            x = self.ast.exprs.operand(x);
        }
        if self.ast.exprs.get(x).kind() != ExprKind::DeclRefExpr {
            return None;
        }
        Some((self.ast.exprs.decl(x), op, c))
    }

    fn skip_parens(&self, mut e: ExprId) -> ExprId {
        while self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = self.ast.exprs.operand(e);
        }
        e
    }

    /// clang `tryTransformToIntOrEnumConstant`: an integer literal (under
    /// parens, one integral cast and one unary minus) or an enumerator.
    fn int_or_enum_constant(&self, e: ExprId) -> Option<ExprId> {
        let e = self.skip_parens(e);
        let mut x = e;
        let ex = &self.ast.exprs;
        if ex.get(x).kind() == ExprKind::ImplicitCastExpr
            && ex.cast_kind(x) == kiln_ast::expr::CastKind::IntegralCast
        {
            x = ex.operand(x);
        }
        if ex.get(x).kind() == ExprKind::UnaryOperator && ex.unary_op(x) == UnaryOp::Minus {
            x = ex.operand(x);
        }
        if ex.get(x).kind() == ExprKind::IntegerLiteral {
            return Some(e);
        }
        self.enum_constant(e).map(|_| e)
    }

    /// The enumerator `e` names (under parens and implicit casts).
    fn enum_constant(&self, mut e: ExprId) -> Option<DeclId> {
        while matches!(
            self.ast.exprs.get(e).kind(),
            ExprKind::ImplicitCastExpr | ExprKind::ParenExpr
        ) {
            e = self.ast.exprs.operand(e);
        }
        (self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr)
            .then(|| self.ast.exprs.decl(e))
            .filter(|&d| self.kind_of(d) == DeclKind::EnumConstant)
    }

    fn reach_stmt(&mut self, s: StmtId, mut live: bool, r: &mut Reach) -> bool {
        let n = *self.ast.stmts.get(s);
        match n.kind() {
            StmtKind::CompoundStmt => {
                for c in self.children(s) {
                    live = self.reach_stmt(c, live, r);
                }
                live
            }
            StmtKind::LabelStmt => {
                let l = self.ast.stmts.label_decl(s);
                let live = live || r.labels.contains(&l);
                self.reach_stmt(self.ast.stmts.sub_stmt(s), live, r)
            }
            StmtKind::AttributedStmt => self.reach_stmt(self.ast.stmts.sub_stmt(s), live, r),
            StmtKind::GotoStmt => {
                if live && r.labels.insert(self.ast.stmts.label_decl(s)) {
                    r.changed = true;
                }
                false
            }
            StmtKind::IndirectGotoStmt => false,
            StmtKind::ReturnStmt | StmtKind::CoreturnStmt => {
                self.reach_parts(s, live, r);
                false
            }
            StmtKind::BreakStmt => {
                if live && let Some(j) = r.jumps.last_mut() {
                    j.broke = true;
                }
                false
            }
            StmtKind::ContinueStmt => {
                if live && let Some(j) = r.jumps.iter_mut().rev().find(|j| j.is_loop) {
                    j.continued = true;
                }
                false
            }
            StmtKind::IfStmt => self.reach_if(s, live, r),
            StmtKind::WhileStmt | StmtKind::ForStmt | StmtKind::DoStmt => {
                self.reach_loop(s, live, r)
            }
            StmtKind::CXXForRangeStmt => self.reach_range_for(s, live, r),
            StmtKind::SwitchStmt => self.reach_switch(s, live, r),
            StmtKind::CaseStmt => {
                let d = *self.ast.stmts.case_data(s);
                if let Some(f) = r.switches.last() {
                    let (sl, value) = (f.live, f.value);
                    live = live || (sl && (value.is_none() || self.fold_int(d.lhs) == value));
                }
                self.reach_stmt(d.sub, live, r)
            }
            StmtKind::DefaultStmt => {
                if let Some(f) = r.switches.last() {
                    live = live || (f.live && !f.matched);
                }
                self.reach_stmt(self.ast.stmts.default_sub(s), live, r)
            }
            StmtKind::CXXTryStmt => {
                r.tries.push((s, false));
                let mut out = self.reach_stmt(self.ast.stmts.try_block(s), live, r);
                let (_, reached) = r.tries.pop().expect("try frame");
                let handlers: Vec<StmtId> =
                    self.ast.lists.iter(self.ast.stmts.try_handlers(s)).collect();
                for h in handlers {
                    out |= self.reach_stmt(self.ast.stmts.catch_block(h), reached, r);
                }
                out
            }
            _ => self.reach_parts(s, live, r),
        }
    }

    fn reach_if(&mut self, s: StmtId, mut live: bool, r: &mut Reach) -> bool {
        if matches!(self.ast.stmts.if_kind(s), IfKind::Consteval | IfKind::NotConsteval) {
            r.no_cfg = true;
            return live;
        }
        let d = *self.ast.stmts.if_data(s);
        if let Some(i) = d.init {
            live = self.reach_stmt(i, live, r);
        }
        live = self.reach_var_init(d.cond_var, live, r);
        let mut kv = None;
        if let Some(c) = d.cond {
            live = self.reach_expr(c, live, r);
            if d.cond_var.is_none() {
                kv = self.known_truth(c);
            }
        }
        let (tl, el) = (live && kv != Some(false), live && kv != Some(true));
        let t = d.then.map_or(tl, |t| self.reach_stmt(t, tl, r));
        let e = d.els.map_or(el, |e| self.reach_stmt(e, el, r));
        t || e
    }

    fn reach_loop(&mut self, s: StmtId, mut live: bool, r: &mut Reach) -> bool {
        let d = *self.ast.stmts.loop_data(s);
        if let Some(i) = d.init {
            live = self.reach_stmt(i, live, r);
        }
        let entry = live || r.loops.contains(&s);
        r.jumps.push(Jump { is_loop: true, broke: false, continued: false });
        let (cond_out, kv, back);
        if self.ast.stmts.get(s).kind() == StmtKind::DoStmt {
            let body_out = d.body.is_some_and(|b| self.reach_stmt(b, entry, r));
            let cont = r.jumps.last().is_some_and(|j| j.continued);
            (cond_out, kv) = self.reach_loop_cond(d.cond_var, d.cond, body_out || cont, r);
            back = cond_out && kv != Some(false);
        } else {
            (cond_out, kv) = self.reach_loop_cond(d.cond_var, d.cond, entry, r);
            let body_in = cond_out && kv != Some(false);
            let body_out = d.body.map_or(body_in, |b| self.reach_stmt(b, body_in, r));
            let cont = r.jumps.last().is_some_and(|j| j.continued);
            back = d.inc.map_or(body_out || cont, |i| self.reach_expr(i, body_out || cont, r));
        }
        let j = r.jumps.pop().expect("loop jump frame");
        if back && !entry {
            r.loops.insert(s);
            r.changed = true;
        }
        (cond_out && kv != Some(true)) || j.broke
    }

    fn reach_range_for(&mut self, s: StmtId, mut live: bool, r: &mut Reach) -> bool {
        let d = *self.ast.stmts.range_for_data(s);
        if let Some(i) = d.init {
            live = self.reach_stmt(i, live, r);
        }
        for v in [Some(d.range), d.begin, d.end] {
            live = self.reach_var_init(v, live, r);
        }
        let entry = live || r.loops.contains(&s);
        let cond_out = d.cond.map_or(entry, |c| self.reach_expr(c, entry, r));
        r.jumps.push(Jump { is_loop: true, broke: false, continued: false });
        let body_in = self.reach_var_init(Some(d.loop_var), cond_out, r);
        let body_out = self.reach_stmt(d.body, body_in, r);
        let j = r.jumps.pop().expect("loop jump frame");
        let latch = body_out || j.continued;
        let back = d.inc.map_or(latch, |i| self.reach_expr(i, latch, r));
        if back && !entry {
            r.loops.insert(s);
            r.changed = true;
        }
        cond_out || j.broke
    }

    /// A statement whose expressions and sub-statements run in order.
    fn reach_parts(&mut self, s: StmtId, mut live: bool, r: &mut Reach) -> bool {
        let (mut exprs, mut subs) = (Vec::new(), Vec::new());
        self.stmt_parts(s, &mut exprs, &mut subs);
        for e in exprs {
            live = self.reach_expr(e, live, r);
        }
        for c in subs {
            live = self.reach_stmt(c, live, r);
        }
        live
    }

    fn reach_var_init(&mut self, v: Option<DeclId>, live: bool, r: &mut Reach) -> bool {
        match v.and_then(|v| self.ast.decls.var(v).and_then(|x| x.init)) {
            Some(i) => self.reach_expr(i, live, r),
            None => live,
        }
    }

    /// A loop condition (absent: `true`) and its known truth value.
    fn reach_loop_cond(
        &mut self,
        var: Option<DeclId>,
        cond: Option<ExprId>,
        live: bool,
        r: &mut Reach,
    ) -> (bool, Option<bool>) {
        let live = self.reach_var_init(var, live, r);
        let Some(c) = cond else { return (live, Some(true)) };
        let live = self.reach_expr(c, live, r);
        (live, if var.is_none() { self.known_truth(c) } else { None })
    }

    fn reach_switch(&mut self, s: StmtId, mut live: bool, r: &mut Reach) -> bool {
        let d = *self.ast.stmts.switch_data(s);
        if let Some(i) = d.init {
            live = self.reach_stmt(i, live, r);
        }
        live = self.reach_var_init(d.cond_var, live, r);
        live = self.reach_expr(d.cond, live, r);
        let value = if d.cond_var.is_none() { self.fold_int(d.cond) } else { None };
        let cases: Vec<StmtId> = self.ast.lists.iter(d.cases).collect();
        let mut has_default = false;
        let mut case_values = Vec::new();
        for &c in &cases {
            if self.ast.stmts.get(c).kind() == StmtKind::DefaultStmt {
                has_default = true;
            } else {
                let lhs = self.ast.stmts.case_data(c).lhs;
                case_values.push(self.fold_int(lhs));
            }
        }
        let matched = value.is_some() && case_values.contains(&value);
        r.switches.push(SwitchFrame { live, value, matched });
        r.jumps.push(Jump { is_loop: false, broke: false, continued: false });
        let body_out = self.reach_stmt(d.body, false, r);
        let j = r.jumps.pop().expect("switch jump frame");
        r.switches.pop();
        let skips = live
            && !has_default
            && !matched
            && (value.is_some() || !self.all_enum_cases_covered(d.cond, &case_values));
        body_out || j.broke || skips
    }

    /// clang `SwitchStmt::isAllEnumCasesCovered`: the condition (before
    /// promotion) has a complete enumeration type with enumerators, each
    /// of which some case names.
    fn all_enum_cases_covered(&self, mut cond: ExprId, cases: &[Option<i128>]) -> bool {
        while self.ast.exprs.get(cond).kind() == ExprKind::ImplicitCastExpr {
            cond = self.ast.exprs.operand(cond);
        }
        let t = &self.ast.types;
        let ty = t.canonical(self.ast.exprs.get(cond).ty());
        let Some(def) = t.as_enum(ty).and_then(|e| self.ast.definition(e)) else { return false };
        let mut any = false;
        for m in self.ast.decls.members(def) {
            if self.kind_of(m) != DeclKind::EnumConstant {
                continue;
            }
            any = true;
            let v = self.ast.decls.enumerator(m).and_then(|x| x.value);
            if v.is_none_or(|v| !cases.contains(&self.const_int(v))) {
                return false;
            }
        }
        any
    }

    fn reach_expr(&mut self, e: ExprId, live: bool, r: &mut Reach) -> bool {
        let kind = self.ast.exprs.get(e).kind();
        match kind {
            ExprKind::LambdaExpr => live,
            ExprKind::CXXThrowExpr => {
                let live = self.reach_kids(e, live, r);
                if live {
                    let tries: Vec<StmtId> = r.tries.iter().map(|t| t.0).collect();
                    if !self.throw_caught(e, &tries) {
                        r.found.push(e);
                    }
                    // The dispatch of each enclosing try up to the first
                    // with a catch-all.
                    for i in (0..r.tries.len()).rev() {
                        r.tries[i].1 = true;
                        let tr = r.tries[i].0;
                        if self
                            .ast
                            .lists
                            .iter::<StmtId>(self.ast.stmts.try_handlers(tr))
                            .any(|h| self.ast.stmts.catch_var(h).is_none())
                        {
                            break;
                        }
                    }
                }
                false
            }
            ExprKind::ConditionalOperator => {
                let kids = self.ast.exprs.children(e, &self.ast.lists);
                let live = self.reach_expr(kids[0], live, r);
                let kv = self.known_truth(kids[0]);
                let t = self.reach_expr(kids[1], live && kv != Some(false), r);
                let f = self.reach_expr(kids[2], live && kv != Some(true), r);
                t || f
            }
            ExprKind::BinaryOperator
                if matches!(self.ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
            {
                let decisive = self.ast.exprs.binary_op(e) == BinaryOp::LOr;
                let [l, rr] = self.ast.exprs.operands(e);
                let live = self.reach_expr(l, live, r);
                let kv = self.known_truth(l);
                let rhs = self.reach_expr(rr, live && kv != Some(decisive), r);
                (live && kv != Some(!decisive)) || rhs
            }
            ExprKind::StmtExpr => {
                let body = self.ast.exprs.stmt_expr_body(e);
                self.reach_stmt(body, live, r)
            }
            _ => {
                let live = self.reach_kids(e, live, r);
                live && !self.is_noreturn_call(e)
            }
        }
    }

    fn reach_kids(&mut self, e: ExprId, mut live: bool, r: &mut Reach) -> bool {
        for c in self.ast.exprs.children(e, &self.ast.lists) {
            live = self.reach_expr(c, live, r);
        }
        live
    }
}

/// clang `analyzeLogicOperatorCondition`: `v OP c`.
fn compare(op: BinaryOp, v: i128, c: i128) -> Option<bool> {
    Some(match op {
        BinaryOp::EQ => v == c,
        BinaryOp::NE => v != c,
        BinaryOp::LT => v < c,
        BinaryOp::GT => v > c,
        BinaryOp::LE => v <= c,
        BinaryOp::GE => v >= c,
        _ => return None,
    })
}
