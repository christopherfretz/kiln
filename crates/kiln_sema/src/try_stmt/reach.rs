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
//!   default); the default edge of a switch over an enumeration whose
//!   enumerators all have cases is unreachable (clang
//!   `isAllEnumCasesCovered`), so with no default the code after it is
//!   unreachable;
//! - a handler is reachable only from a reachable throw-expression in its
//!   try-block (or in a nested try-block without a catch-all): calls get
//!   no exception edges;
//! - labels and loop conditions reached by a later `goto` or back edge
//!   are found by iterating to a fixed point;
//! - an `if consteval` anywhere in the body: clang builds no CFG and
//!   checks nothing.
//!
//! The same walk, in fall-off mode (`fall_off_kind`), classifies the
//! live predecessors of the exit as clang `CheckFallThrough` does: a
//! `return`, a throw leaving the body (fake edge), a `noreturn` call or a
//! dispatch without a catch-all leaving the body (abnormal edge), and the
//! end of the body (plain edge).

use std::collections::{HashMap, HashSet};

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprKind, UnaryOp};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::{DeclId, ExprId, StmtId};

use crate::Sema;
use crate::stmt::Falloff;

/// The state of one walk.
#[derive(Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent facts of one walk")]
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
    /// The expressions whose reachability is asked for.
    wanted: HashSet<ExprId>,
    /// The wanted expressions the walk met (in any pass).
    visited: HashSet<ExprId>,
    /// The wanted expressions met on a live path (in any pass).
    reached: HashSet<ExprId>,
    /// Every case label is reached (clang `FallthroughMapper::
    /// fillReachableBlocks`).
    cases_live: bool,
    /// The (attributed) statements whose reachability is asked for.
    stmts_wanted: HashSet<StmtId>,
    /// The wanted statements met on a live path (in any pass).
    stmts_reached: HashSet<StmtId>,
    /// The fall-off walk (clang `CheckFallThrough`): a try's handlers
    /// are also reached when its dispatch has no predecessor.
    falloff: bool,
    /// A live `return` (this pass).
    live_return: bool,
    /// A live throw leaves the body (this pass).
    fake_edge: bool,
    /// A live call to a `noreturn` function, or a live dispatch without
    /// a catch-all leaving the body (this pass).
    abnormal_edge: bool,
    /// Whether each statement is reached, and whether control leaves it
    /// normally (in any pass), when recorded ([`Sema::stmt_flow`]).
    flow: Option<StmtFlow>,
    /// The unreachable-code scan ([`Sema::dead_scan`]).
    scan: Option<DeadScan>,
}

/// An element of the unreachable-code scan (a clang `CFGStmt`): an
/// expression, or a statement the CFG holds as an element or a
/// terminator (a declaration of a variable, `return`, `break`,
/// `continue`, `goto`, a `[[fallthrough]]` statement).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Unit {
    Stmt(StmtId),
    Expr(ExprId),
}

/// What the unreachable-code walk learns about the elements of a body
/// (clang `reachable_code::FindUnreachableCode`; spec/sema.md §7.10).
/// An expression counts after its operands (its own CFG position), a
/// declaration and a `return` after their expressions, a jump on entry.
#[derive(Default)]
pub(crate) struct DeadScan {
    /// The elements treated as reached whatever flows into them (clang
    /// scans forward from each reported dead block).
    pub(crate) roots: HashSet<Unit>,
    /// Every element, in evaluation order (an assignment's right operand
    /// first, as clang's CFG).
    pub(crate) order: Vec<Unit>,
    /// The elements reached.
    pub(crate) live: HashSet<Unit>,
    /// The elements a reached element (or root) flows into.
    pub(crate) has_pred: HashSet<Unit>,
    /// The first element of a branch a constant condition prunes, with
    /// that condition (the silencing note's source).
    pub(crate) pruned: HashMap<Unit, ExprId>,
    /// The elements of a loop increment, with the loop.
    pub(crate) inc: HashMap<Unit, StmtId>,
    /// The elements of a `return` statement, with the statement.
    pub(crate) ret: HashMap<Unit, StmtId>,
    /// The conditions of the `do` loops, parentheses and casts skipped.
    pub(crate) do_conds: HashSet<ExprId>,
    /// The condition pruning the edge into the next element.
    pending: Option<ExprId>,
    cur_inc: Option<StmtId>,
    cur_ret: Option<StmtId>,
    /// The first pass (records `order`, `inc` and `ret`).
    recording: bool,
}

struct Jump {
    is_loop: bool,
    broke: bool,
    continued: bool,
}

/// Whether each statement is reached, and whether control leaves it
/// normally.
pub(crate) type StmtFlow = (HashMap<StmtId, bool>, HashMap<StmtId, bool>);

/// The folded `lo` and `hi` of a case label.
type CaseBounds = (Option<i128>, Option<i128>);

struct SwitchFrame {
    /// The condition is reached.
    live: bool,
    /// The condition's constant value.
    value: Option<i128>,
    /// The switch reaches its `default:` label (clang CFG: not when the
    /// value matched a case, nor when every enumerator is covered).
    default_edge: bool,
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

    /// The expressions of `wanted` that are unreachable in `body` (clang
    /// `IssueWarnings` filtering `PossiblyUnreachableDiags` through the
    /// CFG); `None` when clang would build no CFG. An expression the walk
    /// does not meet counts as reachable, as an unmapped statement does
    /// in clang.
    pub(crate) fn unreachable_exprs(
        &mut self,
        body: StmtId,
        wanted: HashSet<ExprId>,
    ) -> Option<HashSet<ExprId>> {
        let mut r = Reach { wanted, ..Reach::default() };
        loop {
            r.changed = false;
            r.found.clear();
            self.reach_stmt(body, true, &mut r);
            if r.no_cfg {
                return None;
            }
            if !r.changed {
                return Some(r.visited.difference(&r.reached).copied().collect());
            }
        }
    }

    /// How control reaches the end of the function body `body` (clang
    /// `CheckFallThrough`): `Never` when it does not (or clang builds no
    /// CFG), `Maybe` when the body also returns, throws out or calls a
    /// `noreturn` function on a live path, else `Always`.
    pub(crate) fn fall_off_kind(&mut self, body: StmtId) -> Falloff {
        let mut r = Reach { falloff: true, ..Reach::default() };
        loop {
            r.changed = false;
            r.found.clear();
            (r.live_return, r.fake_edge, r.abnormal_edge) = (false, false, false);
            let end = self.reach_stmt(body, true, &mut r);
            if r.no_cfg {
                return Falloff::Never;
            }
            if !r.changed {
                return if !end {
                    Falloff::Never
                } else if r.live_return || r.fake_edge || r.abnormal_edge {
                    Falloff::Maybe
                } else {
                    Falloff::Always
                };
            }
        }
    }

    /// Whether the dispatch block of try statement `s` has predecessors
    /// in clang's CFG: a throw-expression in its try-block (outside a
    /// nested try-block and a lambda), or a nested try without a
    /// catch-all, whose dispatch chains to it.
    fn dispatch_has_preds(&self, s: StmtId) -> bool {
        let (mut stmts, mut exprs) = (vec![self.ast.stmts.try_block(s)], Vec::new());
        while let Some(s) = stmts.pop() {
            if self.ast.stmts.get(s).kind() == StmtKind::CXXTryStmt {
                if !self.has_catch_all(s) {
                    return true;
                }
                let hs = self.ast.lists.iter::<StmtId>(self.ast.stmts.try_handlers(s));
                stmts.extend(hs.map(|h| self.ast.stmts.catch_block(h)));
                continue;
            }
            self.stmt_parts(s, &mut exprs, &mut stmts);
            while let Some(e) = exprs.pop() {
                match self.ast.exprs.get(e).kind() {
                    ExprKind::LambdaExpr | ExprKind::UnaryExprOrTypeTraitExpr => {}
                    ExprKind::CXXThrowExpr => return true,
                    ExprKind::StmtExpr => stmts.push(self.ast.exprs.stmt_expr_body(e)),
                    _ => exprs.extend(self.ast.exprs.children(e, &self.ast.lists)),
                }
            }
        }
        false
    }

    fn has_catch_all(&self, s: StmtId) -> bool {
        self.ast
            .lists
            .iter::<StmtId>(self.ast.stmts.try_handlers(s))
            .any(|h| self.ast.stmts.catch_var(h).is_none())
    }

    pub(crate) fn known_truth(&mut self, e: ExprId) -> Option<bool> {
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

    /// The attributed statements of `wanted` that are unreachable in
    /// `body`, every case label counting as reached (clang
    /// `FallthroughMapper::fillReachableBlocks`); `None` when clang would
    /// build no CFG.
    pub(crate) fn unreachable_attributed_stmts(
        &mut self,
        body: StmtId,
        wanted: HashSet<StmtId>,
    ) -> Option<HashSet<StmtId>> {
        let mut r = Reach { stmts_wanted: wanted, cases_live: true, ..Reach::default() };
        loop {
            r.changed = false;
            r.found.clear();
            self.reach_stmt(body, true, &mut r);
            if r.no_cfg {
                return None;
            }
            if !r.changed {
                return Some(r.stmts_wanted.difference(&r.stmts_reached).copied().collect());
            }
        }
    }

    /// Whether each statement of `body` is reached, and whether control
    /// leaves it normally, every case label counting as reached (clang
    /// `FallthroughMapper::fillReachableBlocks`); `None` when clang would
    /// build no CFG.
    pub(crate) fn stmt_flow(&mut self, body: StmtId) -> Option<StmtFlow> {
        let mut r = Reach { cases_live: true, flow: Some(Default::default()), ..Reach::default() };
        loop {
            r.changed = false;
            r.found.clear();
            self.reach_stmt(body, true, &mut r);
            if r.no_cfg {
                return None;
            }
            if !r.changed {
                return r.flow;
            }
        }
    }

    /// The unreachable-code walk of `body` with `roots` treated as
    /// reached ([`DeadScan`]): constant conditions of `if`, `?:`, `&&`
    /// and `||` (and of a loop whose condition is `&&` or `||`) prune
    /// unless they are configuration values, an `if constexpr` prunes
    /// nothing, every label of a reached switch is reached and every
    /// handler is reached (clang's CFG has no exception edges and scans
    /// the dispatch blocks as roots). `None` when clang would build no
    /// CFG.
    pub(crate) fn dead_scan(&mut self, body: StmtId, roots: HashSet<Unit>) -> Option<DeadScan> {
        let scan = DeadScan { roots, recording: true, ..DeadScan::default() };
        let mut r = Reach { scan: Some(scan), ..Reach::default() };
        loop {
            r.changed = false;
            r.found.clear();
            self.reach_stmt(body, true, &mut r);
            if r.no_cfg {
                return None;
            }
            if let Some(sc) = r.scan.as_mut() {
                sc.recording = false;
            }
            if !r.changed {
                return r.scan;
            }
        }
    }

    /// Records element `u`, which control reaches when `at`; whether it
    /// is reached (it may be a root).
    fn unit(u: Unit, at: bool, r: &mut Reach) -> bool {
        let Some(sc) = r.scan.as_mut() else { return at };
        if sc.recording {
            sc.order.push(u);
            if let Some(l) = sc.cur_inc {
                sc.inc.insert(u, l);
            }
            if let Some(rs) = sc.cur_ret {
                sc.ret.insert(u, rs);
            }
        }
        if let Some(c) = sc.pending.take()
            && !at
        {
            sc.pruned.entry(u).or_insert(c);
        }
        if at {
            sc.has_pred.insert(u);
        }
        let live = at || sc.roots.contains(&u);
        if live {
            sc.live.insert(u);
        }
        live
    }

    /// Notes (scan only) that the edge into the next element is pruned
    /// by condition `c` when `pruned` (and control reaches the branch).
    fn prune_edge(live: bool, pruned: bool, c: ExprId, r: &mut Reach) {
        if let Some(sc) = r.scan.as_mut() {
            sc.pending = (live && pruned).then_some(c);
        }
    }

    fn clear_pending(r: &mut Reach) {
        if let Some(sc) = r.scan.as_mut() {
            sc.pending = None;
        }
    }

    /// The known truth value of `c` as a branch condition: in the scan, a
    /// configuration value (`whole`, clang `isConfigurationValue`) does
    /// not prune.
    fn branch_truth(&mut self, c: ExprId, whole: ExprId, r: &Reach) -> Option<bool> {
        let kv = self.known_truth(c);
        if kv.is_some() && r.scan.is_some() && self.is_configuration_value(whole, &mut None) {
            None
        } else {
            kv
        }
    }

    /// A branch statement, pruned by `c` when `pruned`.
    fn reach_arm_stmt(
        &mut self,
        s: StmtId,
        live: bool,
        pruned: bool,
        c: ExprId,
        r: &mut Reach,
    ) -> bool {
        Self::prune_edge(live, pruned, c, r);
        let out = self.reach_stmt(s, live && !pruned, r);
        Self::clear_pending(r);
        out
    }

    /// A branch expression, pruned by `c` when `pruned`.
    fn reach_arm_expr(
        &mut self,
        e: ExprId,
        live: bool,
        pruned: bool,
        c: ExprId,
        r: &mut Reach,
    ) -> bool {
        Self::prune_edge(live, pruned, c, r);
        let out = self.reach_expr(e, live && !pruned, r);
        Self::clear_pending(r);
        out
    }

    fn reach_stmt(&mut self, s: StmtId, live: bool, r: &mut Reach) -> bool {
        let out = self.reach_stmt_inner(s, live, r);
        if let Some((before, after)) = r.flow.as_mut() {
            *before.entry(s).or_default() |= live;
            *after.entry(s).or_default() |= out;
        }
        out
    }

    /// A GNU asm statement: its operands, then, for a reachable `asm goto`,
    /// an edge to each of its labels besides the fall-through.
    fn reach_asm(&mut self, s: StmtId, live: bool, r: &mut Reach) -> bool {
        let out = self.reach_parts(s, live, r);
        if out && self.ast.stmts.asm_bits(s) & kiln_ast::stmt::asm_bits::GOTO != 0 {
            let labels = self.ast.stmts.asm_data(s).labels;
            for l in self.ast.lists.iter::<DeclId>(labels) {
                if r.labels.insert(l) {
                    r.changed = true;
                }
            }
        }
        out
    }

    fn reach_stmt_inner(&mut self, s: StmtId, mut live: bool, r: &mut Reach) -> bool {
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
            StmtKind::AttributedStmt => {
                if live && r.stmts_wanted.contains(&s) {
                    r.stmts_reached.insert(s);
                }
                if r.scan.is_some() && self.ift_is_fallthrough(s) {
                    live = Self::unit(Unit::Stmt(s), live, r);
                }
                self.reach_stmt(self.ast.stmts.sub_stmt(s), live, r)
            }
            StmtKind::GotoStmt => {
                let live = Self::unit(Unit::Stmt(s), live, r);
                if live && r.labels.insert(self.ast.stmts.label_decl(s)) {
                    r.changed = true;
                }
                false
            }
            StmtKind::IndirectGotoStmt => {
                let live = self.reach_parts(s, live, r);
                Self::unit(Unit::Stmt(s), live, r);
                false
            }
            // [dcl.asm], GNU `asm goto`: an edge to each label as well as
            // to the next statement (clang's CFG `GCCAsmStmt` successors).
            StmtKind::GCCAsmStmt => self.reach_asm(s, live, r),
            StmtKind::ReturnStmt | StmtKind::CoreturnStmt => {
                let outer = r.scan.as_mut().and_then(|sc| sc.cur_ret.replace(s));
                let at = self.reach_parts(s, live, r);
                let at = Self::unit(Unit::Stmt(s), at, r);
                if let Some(sc) = r.scan.as_mut() {
                    sc.cur_ret = outer;
                }
                if at {
                    r.live_return = true;
                }
                false
            }
            StmtKind::BreakStmt => {
                let live = Self::unit(Unit::Stmt(s), live, r);
                if live && let Some(j) = r.jumps.last_mut() {
                    j.broke = true;
                }
                false
            }
            StmtKind::ContinueStmt => {
                let live = Self::unit(Unit::Stmt(s), live, r);
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
                live |= r.cases_live;
                if let Some(f) = r.switches.last() {
                    let (sl, value) = (f.live, f.value);
                    let hit = r.scan.is_some()
                        || value.is_none_or(|v| Self::in_case(self.case_bounds(s), v));
                    live = live || (sl && hit);
                }
                self.reach_stmt(d.sub, live, r)
            }
            StmtKind::DefaultStmt => {
                live |= r.cases_live;
                if let Some(f) = r.switches.last() {
                    live = live || f.default_edge;
                }
                self.reach_stmt(self.ast.stmts.default_sub(s), live, r)
            }
            StmtKind::CXXTryStmt => self.reach_try(s, live, r),
            StmtKind::DeclStmt => {
                let at = self.reach_parts(s, live, r);
                let has_var = r.scan.is_some()
                    && self
                        .ast
                        .lists
                        .iter::<DeclId>(self.ast.stmts.decl_stmt_decls(s))
                        .any(|d| self.ast.decls.var(d).is_some());
                if has_var { Self::unit(Unit::Stmt(s), at, r) } else { at }
            }
            _ => self.reach_parts(s, live, r),
        }
    }

    fn reach_try(&mut self, s: StmtId, live: bool, r: &mut Reach) -> bool {
        r.tries.push((s, false));
        let mut out = self.reach_stmt(self.ast.stmts.try_block(s), live, r);
        let (_, mut reached) = r.tries.pop().expect("try frame");
        if r.falloff {
            // clang `CheckFallThrough` rescans a dispatch block
            // with no predecessor as live; a live one without a
            // catch-all chains to the enclosing dispatch, or
            // leaves the body.
            reached = reached || !self.dispatch_has_preds(s);
            if reached && !self.has_catch_all(s) {
                match r.tries.last_mut() {
                    Some(t) => t.1 = true,
                    None => r.abnormal_edge = true,
                }
            }
        }
        // The scan treats every dispatch block as a root.
        reached |= r.scan.is_some();
        let handlers: Vec<StmtId> = self.ast.lists.iter(self.ast.stmts.try_handlers(s)).collect();
        for h in handlers {
            out |= self.reach_stmt(self.ast.stmts.catch_block(h), reached, r);
        }
        out
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
        let constexpr = self.ast.stmts.if_kind(s) == IfKind::Constexpr;
        if let Some(c) = d.cond {
            live = self.reach_expr(c, live, r);
            // The scan prunes no edge of an `if constexpr` (clang
            // `shouldTreatSuccessorsAsReachable`).
            if d.cond_var.is_none() && !(constexpr && r.scan.is_some()) {
                kv = self.branch_truth(c, c, r);
            }
        }
        let (tl, el) = (live && kv != Some(false), live && kv != Some(true));
        let Some(c) = d.cond else {
            let t = d.then.map_or(tl, |t| self.reach_stmt(t, tl, r));
            let e = d.els.map_or(el, |e| self.reach_stmt(e, el, r));
            return t || e;
        };
        let t = d.then.map_or(tl, |t| self.reach_arm_stmt(t, live, kv == Some(false), c, r));
        let e = match d.els {
            Some(e) => self.reach_arm_stmt(e, live, kv == Some(true), c, r),
            None => el,
        };
        if d.els.is_none() {
            // The pruned false edge leads to the statement after the if.
            Self::prune_edge(live, kv == Some(true), c, r);
        }
        t || e
    }

    fn reach_loop(&mut self, s: StmtId, mut live: bool, r: &mut Reach) -> bool {
        let d = *self.ast.stmts.loop_data(s);
        if let Some(i) = d.init {
            live = self.reach_stmt(i, live, r);
        }
        let entry = live || r.loops.contains(&s);
        r.jumps.push(Jump { is_loop: true, broke: false, continued: false });
        let (cond_out, mut kv, back);
        // clang's CFG prunes a loop's constant condition outright, but
        // one of `&&` or `||` goes through its logical-operator edges.
        let logical = r.scan.is_some() && d.cond.is_some_and(|c| self.is_logical_op(c));
        if self.ast.stmts.get(s).kind() == StmtKind::DoStmt {
            let body_out = d.body.is_some_and(|b| self.reach_stmt(b, entry, r));
            let cont = r.jumps.last().is_some_and(|j| j.continued);
            if let (Some(sc), Some(c)) = (r.scan.as_mut(), d.cond) {
                sc.do_conds.insert(self.ignore_paren_imp_casts(c));
            }
            (cond_out, kv) = self.reach_loop_cond(d.cond_var, d.cond, body_out || cont, r);
            if logical
                && kv.is_some()
                && self.is_configuration_value(d.cond.expect("cond"), &mut None)
            {
                kv = None;
            }
            back = cond_out && kv != Some(false);
        } else {
            (cond_out, kv) = self.reach_loop_cond(d.cond_var, d.cond, entry, r);
            if logical
                && kv.is_some()
                && self.is_configuration_value(d.cond.expect("cond"), &mut None)
            {
                kv = None;
            }
            let body_in = cond_out && kv != Some(false);
            let body_out = match (d.body, d.cond) {
                (Some(b), Some(c)) if logical => {
                    self.reach_arm_stmt(b, cond_out, kv == Some(false), c, r)
                }
                (Some(b), _) => self.reach_stmt(b, body_in, r),
                (None, _) => body_in,
            };
            let cont = r.jumps.last().is_some_and(|j| j.continued);
            back = match d.inc {
                Some(i) => self.reach_inc(s, i, body_out || cont, r),
                None => body_out || cont,
            };
        }
        let j = r.jumps.pop().expect("loop jump frame");
        if back && !entry {
            r.loops.insert(s);
            r.changed = true;
        }
        if logical && let Some(c) = d.cond {
            Self::prune_edge(cond_out, kv == Some(true), c, r);
        }
        (cond_out && kv != Some(true)) || j.broke
    }

    /// The increment `i` of loop `s`.
    fn reach_inc(&mut self, s: StmtId, i: ExprId, live: bool, r: &mut Reach) -> bool {
        let outer = r.scan.as_mut().and_then(|sc| sc.cur_inc.replace(s));
        let out = self.reach_expr(i, live, r);
        if let Some(sc) = r.scan.as_mut() {
            sc.cur_inc = outer;
        }
        out
    }

    /// `e` is `&&` or `||`, parentheses skipped.
    fn is_logical_op(&self, e: ExprId) -> bool {
        let e = self.skip_parens(e);
        self.ast.exprs.get(e).kind() == ExprKind::BinaryOperator
            && matches!(self.ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr)
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
        let back = d.inc.map_or(latch, |i| self.reach_inc(s, i, latch, r));
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
                case_values.push(self.case_bounds(c));
            }
        }
        let matched = value.is_some_and(|v| case_values.iter().any(|&b| Self::in_case(b, v)));
        // The scan reaches every edge of a reached switch.
        let default_edge = live
            && (r.scan.is_some()
                || (!matched && !self.switch_covers_enum(d.cond, &cases, has_default)));
        r.switches.push(SwitchFrame { live, value, default_edge });
        r.jumps.push(Jump { is_loop: false, broke: false, continued: false });
        let body_out = self.reach_stmt(d.body, false, r);
        let j = r.jumps.pop().expect("switch jump frame");
        r.switches.pop();
        body_out || j.broke || (default_edge && !has_default)
    }

    /// The folded bounds of case `c`: `lo` and `hi` of a GNU range
    /// `case lo ... hi`, else the value twice.
    fn case_bounds(&mut self, c: StmtId) -> CaseBounds {
        let d = *self.ast.stmts.case_data(c);
        let lo = self.fold_int(d.lhs);
        let hi = match d.rhs {
            Some(h) => self.fold_int(h),
            None => lo,
        };
        (lo, hi)
    }

    /// The case of bounds `b` matches the value `v` (clang CFG
    /// `shouldAddCase`).
    fn in_case(b: CaseBounds, v: i128) -> bool {
        matches!(b, (Some(lo), Some(hi)) if lo <= v && v <= hi)
    }

    fn reach_expr(&mut self, e: ExprId, live: bool, r: &mut Reach) -> bool {
        if r.wanted.contains(&e) {
            r.visited.insert(e);
            if live {
                r.reached.insert(e);
            }
        }
        let kind = self.ast.exprs.get(e).kind();
        match kind {
            // clang `CFGBuilder::VisitUnaryExprOrTypeTraitExpr` adds the
            // node but walks only a VLA bound: the operand of `sizeof`
            // and `alignof` is unevaluated.
            ExprKind::LambdaExpr | ExprKind::UnaryExprOrTypeTraitExpr => {
                Self::unit(Unit::Expr(e), live, r)
            }
            // Implicit initializers have no CFG element of their own.
            ExprKind::ImplicitValueInitExpr | ExprKind::NoInitExpr => live,
            ExprKind::CXXThrowExpr => {
                let live = self.reach_kids(e, live, r);
                let live = Self::unit(Unit::Expr(e), live, r);
                if live {
                    r.fake_edge |= r.tries.is_empty();
                    let tries: Vec<StmtId> = r.tries.iter().map(|t| t.0).collect();
                    if !self.throw_caught(e, &tries) {
                        r.found.push(e);
                    }
                    // The dispatch of each enclosing try up to the first
                    // with a catch-all.
                    for i in (0..r.tries.len()).rev() {
                        r.tries[i].1 = true;
                        if self.has_catch_all(r.tries[i].0) {
                            break;
                        }
                    }
                }
                false
            }
            ExprKind::ConditionalOperator => {
                let kids = self.ast.exprs.children(e, &self.ast.lists);
                let live = self.reach_expr(kids[0], live, r);
                let kv = self.branch_truth(kids[0], kids[0], r);
                let t = self.reach_arm_expr(kids[1], live, kv == Some(false), kids[0], r);
                let f = self.reach_arm_expr(kids[2], live, kv == Some(true), kids[0], r);
                Self::unit(Unit::Expr(e), t || f, r)
            }
            ExprKind::BinaryOperator
                if matches!(self.ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
            {
                let decisive = self.ast.exprs.binary_op(e) == BinaryOp::LOr;
                let [l, rr] = self.ast.exprs.operands(e);
                let live = self.reach_expr(l, live, r);
                let kv = self.branch_truth(l, e, r);
                let rhs = self.reach_arm_expr(rr, live, kv == Some(decisive), e, r);
                Self::unit(Unit::Expr(e), (live && kv != Some(!decisive)) || rhs, r)
            }
            ExprKind::StmtExpr => {
                let body = self.ast.exprs.stmt_expr_body(e);
                let live = self.reach_stmt(body, live, r);
                Self::unit(Unit::Expr(e), live, r)
            }
            _ => {
                let live = self.reach_kids(e, live, r);
                // clang `isValidDeadStmt`: a comma is never a dead root.
                let comma = kind == ExprKind::BinaryOperator
                    && self.ast.exprs.binary_op(e) == BinaryOp::Comma;
                let live = if comma { live } else { Self::unit(Unit::Expr(e), live, r) };
                if live && self.is_noreturn_call(e) {
                    r.abnormal_edge = true;
                    return false;
                }
                live
            }
        }
    }

    fn reach_kids(&mut self, e: ExprId, mut live: bool, r: &mut Reach) -> bool {
        let mut kids = self.ast.exprs.children(e, &self.ast.lists);
        // clang's CFG evaluates the right operand of `=` first.
        if r.scan.is_some()
            && self.ast.exprs.get(e).kind() == ExprKind::BinaryOperator
            && self.ast.exprs.binary_op(e) == BinaryOp::Assign
        {
            kids.reverse();
        }
        for c in kids {
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
