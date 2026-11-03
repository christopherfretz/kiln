//! The consumers' views of a [`Cfg`] (spec/sema.md §7.5): reachability
//! of the deferred diagnostics' expressions, fall-off-the-end, the
//! throw-expressions escaping the body, the statement flow of the
//! fallthrough checks, the unreachable-code scan and the paths of the
//! infinite-recursion check.

use std::collections::{HashMap, HashSet};

use kiln_ast::{ExprId, StmtId};

use super::{BlockId, Cfg, Elem, edge};
use crate::Sema;

/// An element of the unreachable-code scan (a clang `CFGStmt`): an
/// expression, or a statement the CFG holds as an element or a
/// terminator (a declaration of a variable, `return`, `break`,
/// `continue`, `goto`, a `[[fallthrough]]` statement).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Unit {
    Stmt(StmtId),
    Expr(ExprId),
}

/// What the unreachable-code scan learns about the elements of a body
/// (clang `reachable_code::FindUnreachableCode`; spec/sema.md §7.10).
#[derive(Default, Debug)]
pub(crate) struct DeadScan {
    /// Every element, in evaluation order (an assignment's right operand
    /// first, as clang's CFG).
    pub(crate) order: Vec<Unit>,
    /// The elements reached.
    pub(crate) live: HashSet<Unit>,
    /// The elements a reached element (or root) flows into.
    pub(crate) has_pred: HashSet<Unit>,
}

/// Whether each statement is reached, and whether control leaves it
/// normally.
pub(crate) type StmtFlow = (HashMap<StmtId, bool>, HashMap<StmtId, bool>);

/// The live edges into the exit, by kind (clang `CheckFallThrough`).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools, reason = "independent edge kinds")]
pub(crate) struct ExitKinds {
    /// The end of the body (or a dispatch's plain chain).
    pub(crate) plain: bool,
    /// A `return`.
    pub(crate) ret: bool,
    /// A throw leaving the body.
    pub(crate) fake: bool,
    /// A `noreturn` call or destructor, a dispatch without a catch-all.
    pub(crate) abnormal: bool,
}

impl Cfg {
    /// The probed expressions met only where control does not reach
    /// (clang `IssueWarnings` filtering `PossiblyUnreachableDiags`).
    pub(crate) fn unreachable_probes(&self) -> HashSet<ExprId> {
        let r = self.reach(edge::LIVE, &[BlockId::ENTRY]);
        let mut reached = HashSet::new();
        for &(e, p) in &self.probes {
            if self.pos_live(p, &r, edge::LIVE) {
                reached.insert(e);
            }
        }
        self.probes.iter().map(|p| p.0).filter(|e| !reached.contains(e)).collect()
    }

    /// The live edges into the exit (clang `CheckFallThrough`, which
    /// also scans a dispatch block with no predecessor as live).
    pub(crate) fn exit_kinds(&self) -> ExitKinds {
        let mut seeds = vec![BlockId::ENTRY];
        seeds.extend(self.dispatch_blocks().filter(|&d| self.preds(d).is_empty()));
        let r = self.reach(edge::LIVE, &seeds);
        let mut k = ExitKinds::default();
        for p in self.preds(BlockId::EXIT) {
            if p.flags & edge::LIVE == 0 || !r[p.block.idx()] {
                continue;
            }
            if p.flags & edge::RETURN != 0 {
                k.ret = true;
            } else if p.flags & edge::FAKE != 0 {
                k.fake = true;
            } else if p.flags & edge::ABNORMAL != 0 {
                k.abnormal = true;
            } else {
                k.plain = true;
            }
        }
        k
    }

    /// Whether each statement is reached, and whether control leaves it
    /// normally, every case label counting as reached (clang
    /// `FallthroughMapper::fillReachableBlocks`).
    pub(crate) fn stmt_flow(&self) -> StmtFlow {
        let mut seeds = vec![BlockId::ENTRY];
        seeds.extend(self.case_blocks.iter().copied());
        let r = self.reach(edge::LIVE, &seeds);
        let (mut before, mut after) = (HashMap::new(), HashMap::new());
        for &(s, entry, exit) in &self.flow {
            *before.entry(s).or_default() |= self.pos_live(entry, &r, edge::LIVE);
            *after.entry(s).or_default() |= self.pos_live(exit, &r, edge::LIVE);
        }
        (before, after)
    }

    /// Whether every live path from the entry meets an expression
    /// element for which `hit` holds before it reaches the exit, and one
    /// does (clang `checkForRecursiveFunctionCall`: a path stops at the
    /// first block holding a hit). `false` when no edge enters the exit.
    pub(crate) fn every_path_hits(&self, hit: impl Fn(ExprId) -> bool) -> bool {
        if self.preds(BlockId::EXIT).iter().all(|a| a.flags & edge::NULL != 0) {
            return false;
        }
        let mut seen = vec![false; self.blocks.len()];
        let mut work = vec![BlockId::ENTRY];
        let mut found = false;
        while let Some(b) = work.pop() {
            for a in self.succs(b) {
                if a.flags & edge::LIVE == 0 || std::mem::replace(&mut seen[a.block.idx()], true) {
                    continue;
                }
                if a.block == BlockId::EXIT {
                    return false;
                }
                if self.elems(a.block).iter().any(|el| matches!(*el, Elem::Expr(e) if hit(e))) {
                    found = true;
                    continue;
                }
                work.push(a.block);
            }
        }
        found
    }

    /// The block holding element `u`.
    pub(crate) fn block_of(&self, u: Unit) -> Option<BlockId> {
        let i = self.elems.iter().position(|e| e.unit() == Some(u))? as u32;
        (0..self.blocks.len() as u32).map(BlockId).find(|b| self.block(*b).elems.contains(&i))
    }

    /// The unreachable-code scan with `roots` treated as reached: from
    /// the entry and every dispatch block (clang's CFG has no exception
    /// edges and scans the dispatch blocks as roots) over the scan's
    /// edges, element by element.
    pub(crate) fn dead_scan(&self, roots: &HashSet<Unit>) -> DeadScan {
        let n = self.blocks.len();
        let (mut live_in, mut live_out) = (vec![false; n], vec![false; n]);
        // 0: not visited; 1: visited with no live predecessor; 2: visited live.
        let mut state = vec![0u8; n];
        live_in[BlockId::ENTRY.idx()] = true;
        for d in self.dispatch_blocks() {
            live_in[d.idx()] = true;
        }
        let mut sc = DeadScan::default();
        let mut work: Vec<BlockId> = Vec::new();
        let mut visit = |b: BlockId,
                         work: &mut Vec<BlockId>,
                         sc: &mut DeadScan,
                         live_in: &mut Vec<bool>,
                         live_out: &mut Vec<bool>| {
            let mut live = live_in[b.idx()];
            state[b.idx()] = if live { 2 } else { 1 };
            for el in self.elems(b) {
                let Some(u) = el.unit() else { continue };
                if live {
                    sc.has_pred.insert(u);
                }
                live |= roots.contains(&u);
                if live {
                    sc.live.insert(u);
                }
            }
            if live && !live_out[b.idx()] {
                live_out[b.idx()] = true;
                for a in self.succs(b) {
                    let t = a.block.idx();
                    if a.flags & edge::SCAN != 0 && !live_in[t] {
                        live_in[t] = true;
                        if state[t] == 1 {
                            work.push(a.block);
                        }
                    }
                }
            }
        };
        for i in 0..n {
            visit(BlockId(i as u32), &mut work, &mut sc, &mut live_in, &mut live_out);
            while let Some(b) = work.pop() {
                visit(b, &mut work, &mut sc, &mut live_in, &mut live_out);
            }
        }
        sc.order = self.elems.iter().filter_map(|e| e.unit()).collect();
        sc
    }
}

impl Sema<'_, '_> {
    /// The reachable throw-expressions of the body that no enclosing
    /// handler catches (clang `checkThrowInNonThrowingFunc`).
    pub(crate) fn cfg_escaping_throws(&self, cfg: &Cfg) -> Vec<ExprId> {
        let r = cfg.reach(edge::LIVE, &[BlockId::ENTRY]);
        let mut out = Vec::new();
        let mut chain = Vec::new();
        for t in &cfg.throws {
            if !r[t.block.idx()] {
                continue;
            }
            chain.clear();
            let mut at = t.try_;
            while let Some(i) = at {
                let ti = cfg.tries[i as usize];
                chain.push(ti.stmt);
                at = ti.outer;
            }
            chain.reverse();
            if !self.throw_caught(t.expr, &chain) {
                out.push(t.expr);
            }
        }
        out
    }
}
