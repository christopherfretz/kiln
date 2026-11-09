//! LLVM `PromoteMemToReg` for a single alloca (spec/codegen.md §7p).
//!
//! clang's `FinishFunction` promotes the normal-cleanup destination slot
//! of a coroutine to SSA values ("Eliminate `CleanupDestSlot` alloca by
//! replacing it with SSA values and PHIs if the current function is a
//! coroutine"), so the coroutine passes see a `phi` instead of a memory
//! round trip. This is a faithful port of LLVM 19's algorithm for one
//! alloca — the single-store and single-block fast paths, pruned phi
//! placement at the iterated dominance frontier, the renaming walk (its
//! successor order decides each phi's incoming order) and the trivial-phi
//! simplification — so the emitted IR matches clang's.

use kiln_llvm::{MdField, MdNode, MetadataId, Module, TypeId};
use kiln_source::fx::{FxHashMap, FxHashSet};

use super::{Blk, Fir, Op, V};

/// The dominator tree of the blocks reachable from the entry.
struct Dom {
    /// Immediate dominator per block index (`None`: unreachable or entry).
    idom: Vec<Option<Blk>>,
    reachable: Vec<bool>,
    entry: Blk,
}

impl Dom {
    fn new(f: &Fir) -> Dom {
        let n = f.blocks.len();
        let entry = f.entry();
        // Reverse postorder by an iterative DFS.
        let mut post = Vec::new();
        let mut seen = vec![false; n];
        let mut stack: Vec<(Blk, usize)> = vec![(entry, 0)];
        seen[entry.0 as usize] = true;
        while let Some(&mut (b, ref mut k)) = stack.last_mut() {
            let succ = f.succs(b);
            if *k < succ.len() {
                let s = succ[*k];
                *k += 1;
                if !seen[s.0 as usize] {
                    seen[s.0 as usize] = true;
                    stack.push((s, 0));
                }
            } else {
                post.push(b);
                stack.pop();
            }
        }
        let mut po_num = vec![usize::MAX; n];
        for (i, b) in post.iter().enumerate() {
            po_num[b.0 as usize] = i;
        }
        let mut preds: Vec<Vec<Blk>> = vec![Vec::new(); n];
        for &b in &post {
            for s in f.succs(b) {
                preds[s.0 as usize].push(b);
            }
        }
        // Cooper, Harvey and Kennedy's iterative algorithm.
        let mut idom: Vec<Option<Blk>> = vec![None; n];
        idom[entry.0 as usize] = Some(entry);
        let mut changed = true;
        while changed {
            changed = false;
            for &b in post.iter().rev() {
                if b == entry {
                    continue;
                }
                let mut new: Option<Blk> = None;
                for &p in &preds[b.0 as usize] {
                    if idom[p.0 as usize].is_none() {
                        continue;
                    }
                    new = Some(match new {
                        None => p,
                        Some(mut a) => {
                            let mut c = p;
                            while a != c {
                                while po_num[a.0 as usize] < po_num[c.0 as usize] {
                                    a = idom[a.0 as usize].expect("processed");
                                }
                                while po_num[c.0 as usize] < po_num[a.0 as usize] {
                                    c = idom[c.0 as usize].expect("processed");
                                }
                            }
                            a
                        }
                    });
                }
                if new.is_some() && idom[b.0 as usize] != new {
                    idom[b.0 as usize] = new;
                    changed = true;
                }
            }
        }
        idom[entry.0 as usize] = None;
        Dom { idom, reachable: seen, entry }
    }

    fn reachable(&self, b: Blk) -> bool {
        self.reachable[b.0 as usize]
    }

    /// LLVM `DominatorTree::dominates(BasicBlock*, BasicBlock*)`.
    fn dominates(&self, a: Blk, b: Blk) -> bool {
        if !self.reachable(b) {
            return true;
        }
        if !self.reachable(a) {
            return false;
        }
        let mut x = b;
        loop {
            if x == a {
                return true;
            }
            if x == self.entry {
                return false;
            }
            x = self.idom[x.0 as usize].expect("reachable block has an idom");
        }
    }

    /// The dominance frontier of every reachable block.
    fn frontiers(&self, f: &Fir) -> Vec<Vec<Blk>> {
        let n = f.blocks.len();
        let mut preds: Vec<Vec<Blk>> = vec![Vec::new(); n];
        for &b in &f.layout {
            if !self.reachable(b) {
                continue;
            }
            for s in f.succs(b) {
                if !preds[s.0 as usize].contains(&b) {
                    preds[s.0 as usize].push(b);
                }
            }
        }
        let mut df: Vec<Vec<Blk>> = vec![Vec::new(); n];
        for &b in &f.layout {
            let ps = &preds[b.0 as usize];
            if ps.len() < 2 {
                continue;
            }
            let Some(id) = self.idom[b.0 as usize] else { continue };
            for &p in ps {
                let mut r = p;
                while r != id {
                    if !df[r.0 as usize].contains(&b) {
                        df[r.0 as usize].push(b);
                    }
                    match self.idom[r.0 as usize] {
                        Some(x) => r = x,
                        None => break,
                    }
                }
            }
        }
        df
    }
}

impl Fir {
    /// The successors of `b` (its terminator's, in LLVM order).
    fn succs(&self, b: Blk) -> Vec<Blk> {
        self.terminator(b).map(|t| self.insts[t as usize].op.successors()).unwrap_or_default()
    }

    /// LLVM `replaceAllUsesWith`.
    pub(crate) fn replace_all_uses(&mut self, from: V, to: V) {
        for inst in &mut self.insts {
            if inst.block.is_none() {
                continue;
            }
            for o in inst.op.operands_mut() {
                if *o == from {
                    *o = to;
                }
            }
        }
    }

    fn is_alloca_load(&self, i: u32, a: V) -> bool {
        matches!(self.insts[i as usize].op, Op::Load { ptr, .. } if ptr == a)
    }

    fn is_alloca_store(&self, i: u32, a: V) -> bool {
        matches!(self.insts[i as usize].op, Op::Store { ptr, .. } if ptr == a)
    }

    fn position(&self, i: u32) -> usize {
        let b = self.insts[i as usize].block.expect("live instruction");
        self.blocks[b.0 as usize].insts.iter().position(|&x| x == i).expect("in its block")
    }

    fn stored(&self, i: u32) -> V {
        match self.insts[i as usize].op {
            Op::Store { val, .. } => val,
            _ => unreachable!("not a store"),
        }
    }

    /// LLVM `PromoteMemToReg` of the alloca `a`: its loads become the
    /// values stored before them, through phis where control flow joins.
    /// Does nothing unless `a` is a promotable alloca (only loaded from
    /// and stored to).
    pub fn promote_alloca(&mut self, m: &mut Module, a: V) {
        let V::I(ai) = a else { return };
        let Op::Alloca { ty, .. } = self.insts[ai as usize].op else { return };
        if self.insts[ai as usize].block.is_none() {
            return;
        }
        // Users in use-list order (the most recent first).
        let users: Vec<u32> = self.users(a).into_iter().rev().collect();
        for &u in &users {
            match self.insts[u as usize].op {
                Op::Load { ptr, .. } if ptr == a => {}
                Op::Store { ptr, val, .. } if ptr == a && val != a => {}
                _ => return,
            }
        }
        if users.is_empty() {
            self.erase(ai);
            return;
        }
        let undef = V::C(m.const_undef(ty));
        let poison = V::C(m.const_poison(ty));
        let dom = Dom::new(self);

        // `AllocaInfo::AnalyzeAlloca`.
        let mut defining = Vec::new();
        let mut using = Vec::new();
        let mut only_store = None;
        let mut only_block = None;
        let mut one_block = true;
        for &u in &users {
            let b = self.insts[u as usize].block.expect("live user");
            if self.is_alloca_store(u, a) {
                defining.push(b);
                only_store = Some(u);
            } else {
                using.push(b);
            }
            match only_block {
                None => only_block = Some(b),
                Some(x) if x != b => one_block = false,
                _ => {}
            }
        }

        // `rewriteSingleStoreAlloca`.
        if defining.len() == 1 {
            let st = only_store.expect("one store");
            let repl = self.stored(st);
            let require_dom = !self.not_poison(m, repl, ty, 0);
            let sbb = self.insts[st as usize].block.expect("live store");
            using.clear();
            for &u in &users {
                if u == st {
                    continue;
                }
                let lbb = self.insts[u as usize].block.expect("live load");
                if require_dom {
                    if lbb == sbb {
                        if self.position(st) > self.position(u) {
                            using.push(sbb);
                            continue;
                        }
                    } else if !dom.dominates(sbb, lbb) {
                        using.push(lbb);
                        continue;
                    }
                }
                let r = if repl == V::I(u) { poison } else { repl };
                self.replace_all_uses(V::I(u), r);
                self.erase(u);
            }
            if using.is_empty() {
                self.erase(st);
                self.erase(ai);
                return;
            }
        }

        // `promoteSingleBlockAlloca`.
        if one_block && self.promote_single_block(a, undef, poison) {
            return;
        }

        // Phi placement: the iterated dominance frontier of the defining
        // blocks, pruned to the blocks the value is live into.
        let users: Vec<u32> = self.users(a).into_iter().rev().collect();
        let defs: FxHashSet<Blk> = users
            .iter()
            .filter(|&&u| self.is_alloca_store(u, a))
            .map(|&u| self.insts[u as usize].block.expect("live"))
            .collect();
        let live_in = self.live_in_blocks(a, &using, &defs);
        let df = dom.frontiers(self);
        let mut phi_blocks: Vec<Blk> = Vec::new();
        let mut work: Vec<Blk> = defs.iter().copied().filter(|&b| dom.reachable(b)).collect();
        let mut queued: FxHashSet<Blk> = work.iter().copied().collect();
        while let Some(x) = work.pop() {
            for &y in &df[x.0 as usize] {
                if !live_in.contains(&y) || phi_blocks.contains(&y) {
                    continue;
                }
                phi_blocks.push(y);
                if !defs.contains(&y) && queued.insert(y) {
                    work.push(y);
                }
            }
        }
        let num: FxHashMap<Blk, usize> =
            self.layout.iter().enumerate().map(|(i, &b)| (b, i)).collect();
        phi_blocks.sort_by_key(|b| num[b]);
        let mut phi_of: FxHashMap<Blk, u32> = FxHashMap::default();
        let mut phis: Vec<u32> = Vec::new();
        for &b in &phi_blocks {
            let n = self.insts.len() as u32;
            self.insts.push(super::Inst {
                op: Op::Phi { ty, inc: Vec::new() },
                ty,
                block: Some(b),
                md: Vec::new(),
                fmf: kiln_llvm::FastMath::NONE,
            });
            self.blocks[b.0 as usize].insts.insert(0, n);
            phi_of.insert(b, n);
            phis.push(n);
        }

        // The renaming walk (`RenamePass`). `loc` is `IncomingLocs`: the
        // debug location of the store that defined `inc` (none at entry).
        let dbg_kind = self.dbg.as_deref().map(|d| d.kind);
        let mut phi_loc: FxHashMap<u32, Option<MetadataId>> = FxHashMap::default();
        let mut visited: FxHashSet<Blk> = FxHashSet::default();
        let mut rename: Vec<(Blk, Option<Blk>, V, Option<MetadataId>)> =
            vec![(self.entry(), None, undef, None)];
        while let Some((mut bb, mut pred, mut inc, mut loc)) = rename.pop() {
            loop {
                if let (Some(&p), Some(pr)) = (phi_of.get(&bb), pred) {
                    // `updateForIncomingValueLocation`.
                    if dbg_kind.is_some() {
                        let first = matches!(&self.insts[p as usize].op, Op::Phi { inc, .. } if inc.is_empty());
                        let l = if first {
                            loc
                        } else {
                            let cur = phi_loc.get(&p).copied().flatten();
                            merged_location(m, cur, loc)
                        };
                        phi_loc.insert(p, l);
                    }
                    let edges = self.succs(pr).iter().filter(|&&s| s == bb).count();
                    for _ in 0..edges {
                        self.add_incoming(p, inc, pr);
                    }
                    inc = V::I(p);
                }
                if !visited.insert(bb) {
                    break;
                }
                for i in self.blocks[bb.0 as usize].insts.clone() {
                    if self.is_alloca_load(i, a) {
                        self.replace_all_uses(V::I(i), inc);
                        self.erase(i);
                    } else if self.is_alloca_store(i, a) {
                        inc = self.stored(i);
                        loc = dbg_kind.and_then(|k| {
                            self.insts[i as usize].md.iter().find(|x| x.0 == k).map(|x| x.1)
                        });
                        self.erase(i);
                    }
                }
                let succ = self.succs(bb);
                let Some(&first) = succ.first() else { break };
                let mut seen = vec![first];
                for &s in &succ[1..] {
                    if !seen.contains(&s) {
                        seen.push(s);
                        rename.push((s, Some(bb), inc, loc));
                    }
                }
                pred = Some(bb);
                bb = first;
            }
        }
        if let Some(k) = dbg_kind {
            for (&p, &l) in &phi_loc {
                if let Some(l) = l {
                    self.insts[p as usize].md.push((k, l));
                }
            }
        }
        // Uses left in unreachable blocks.
        if !self.users(a).is_empty() {
            let pp = V::C(m.const_poison(TypeId::PTR));
            self.replace_all_uses(a, pp);
        }
        self.erase(ai);

        // Trivial phis.
        let mut changed = true;
        while changed {
            changed = false;
            for &p in &phis {
                if self.insts[p as usize].block.is_none() {
                    continue;
                }
                if let Some(v) = self.simplify_phi(m, &dom, p, ty, undef, poison) {
                    self.replace_all_uses(V::I(p), v);
                    self.erase(p);
                    changed = true;
                }
            }
        }
        // Poison for the edges from blocks the walk never reached.
        for &p in &phis {
            let Some(b) = self.insts[p as usize].block else { continue };
            let mut preds = self.predecessors(b);
            let Op::Phi { inc, .. } = &self.insts[p as usize].op else { continue };
            if inc.len() == preds.len() {
                continue;
            }
            preds.sort_by_key(|x| num[x]);
            for &(_, from) in inc {
                if let Some(k) = preds.iter().position(|&x| x == from) {
                    preds.remove(k);
                }
            }
            for pr in preds {
                self.add_incoming(p, poison, pr);
            }
        }
    }

    /// `promoteSingleBlockAlloca`: every load takes the nearest store
    /// above it; fails (after rewriting the loads it could) when a load
    /// precedes every store.
    fn promote_single_block(&mut self, a: V, undef: V, poison: V) -> bool {
        let users: Vec<u32> = self.users(a).into_iter().rev().collect();
        let mut stores: Vec<(usize, u32)> = users
            .iter()
            .filter(|&&u| self.is_alloca_store(u, a))
            .map(|&u| (self.position(u), u))
            .collect();
        stores.sort_by_key(|x| x.0);
        for &u in &users {
            if !self.is_alloca_load(u, a) {
                continue;
            }
            let at = self.position(u);
            let k = stores.partition_point(|x| x.0 < at);
            let repl = if k == 0 {
                if !stores.is_empty() {
                    return false;
                }
                undef
            } else {
                self.stored(stores[k - 1].1)
            };
            let repl = if repl == V::I(u) { poison } else { repl };
            self.replace_all_uses(V::I(u), repl);
            self.erase(u);
            // Positions shift once the load is gone.
            for s in &mut stores {
                if s.0 > at {
                    s.0 -= 1;
                }
            }
        }
        for u in self.users(a) {
            self.erase(u);
        }
        self.erase(Fir::last_inst(a));
        true
    }

    /// `ComputeLiveInBlocks`.
    fn live_in_blocks(&self, a: V, using: &[Blk], defs: &FxHashSet<Blk>) -> FxHashSet<Blk> {
        let mut work: Vec<Blk> = Vec::new();
        for &b in using {
            // A defining block is live-in only if a load precedes its
            // first store.
            if defs.contains(&b) {
                let first = self.blocks[b.0 as usize]
                    .insts
                    .iter()
                    .find(|&&i| self.is_alloca_store(i, a) || self.is_alloca_load(i, a));
                if first.is_some_and(|&i| self.is_alloca_store(i, a)) {
                    continue;
                }
            }
            work.push(b);
        }
        let mut live = FxHashSet::default();
        while let Some(b) = work.pop() {
            if !live.insert(b) {
                continue;
            }
            for p in self.predecessors(b) {
                if !defs.contains(&p) {
                    work.push(p);
                }
            }
        }
        live
    }

    /// `isGuaranteedNotToBePoison` for the values this pass meets.
    fn not_poison(&self, m: &mut Module, v: V, ty: TypeId, depth: u32) -> bool {
        if depth >= 6 {
            return false;
        }
        match v {
            V::C(c) => c != m.const_undef(ty) && c != m.const_poison(ty),
            V::P(_) => true,
            V::Arg(_) => false,
            V::I(i) => match &self.insts[i as usize].op {
                Op::Phi { inc, .. } => {
                    inc.clone().into_iter().all(|(x, _)| self.not_poison(m, x, ty, depth + 1))
                }
                _ => false,
            },
        }
    }

    /// `simplifyPHINode`.
    fn simplify_phi(
        &self,
        m: &mut Module,
        dom: &Dom,
        p: u32,
        ty: TypeId,
        undef: V,
        poison: V,
    ) -> Option<V> {
        let Op::Phi { inc, .. } = &self.insts[p as usize].op else { return None };
        let mut common = None;
        let (mut has_poison, mut has_undef) = (false, false);
        for &(x, _) in inc {
            if x == V::I(p) {
                continue;
            }
            if x == poison {
                has_poison = true;
                continue;
            }
            if x == undef {
                has_undef = true;
                continue;
            }
            if common.is_some_and(|c| c != x) {
                return None;
            }
            common = Some(x);
        }
        let Some(c) = common else { return Some(if has_undef { undef } else { poison }) };
        if has_poison || has_undef {
            // `valueDominatesPHI`.
            if let V::I(i) = c {
                let pb = self.insts[p as usize].block?;
                let ib = self.insts[i as usize].block?;
                let def_bb = match self.insts[i as usize].op {
                    Op::Invoke { normal, .. } => normal,
                    _ => ib,
                };
                let ok = if !dom.reachable(pb) {
                    true
                } else if !dom.reachable(ib) || def_bb == pb && ib == pb {
                    false
                } else {
                    dom.dominates(def_bb, pb)
                };
                if !ok {
                    return None;
                }
            }
            if has_undef && !self.not_poison(m, c, ty, 0) {
                return None;
            }
        }
        Some(c)
    }
}

/// `DILocation::getMergedLocation` (LLVM 19) of two locations of one
/// function without inlining: none when either is none; else the nearest
/// common scope, the line when both share it (otherwise 0) and the column
/// when both share that too (otherwise 0).
fn merged_location(
    m: &mut Module,
    a: Option<MetadataId>,
    b: Option<MetadataId>,
) -> Option<MetadataId> {
    let (a, b) = (a?, b?);
    if a == b {
        return Some(a);
    }
    let (la, ca, sa) = location_fields(m, a)?;
    let (lb, cb, sb) = location_fields(m, b)?;
    // `GetNearestCommonScope`: the first ancestor of `sb` that is `sa` or
    // one of its ancestors.
    let chain = |m: &Module, mut s: MetadataId| {
        let mut v = vec![s];
        while let Some(p) = parent_scope(m, s) {
            v.push(p);
            s = p;
        }
        v
    };
    let ancestors = chain(m, sa);
    let scope = chain(m, sb).into_iter().find(|s| ancestors.contains(s))?;
    let (line, col) = if la != lb { (0, 0) } else { (la, if ca == cb { ca } else { 0 }) };
    let mut fields = vec![("line", MdField::UInt(line))];
    if col != 0 {
        fields.push(("column", MdField::UInt(col)));
    }
    fields.push(("scope", MdField::Node(Some(scope))));
    Some(m.md_specialized("DILocation", false, fields))
}

/// `(line, column, scope)` of a `DILocation` without `inlinedAt`.
fn location_fields(m: &Module, l: MetadataId) -> Option<(u64, u64, MetadataId)> {
    let MdNode::Specialized { name: "DILocation", fields, .. } = m.md_node(l) else { return None };
    let (mut line, mut col, mut scope) = (0, 0, None);
    for (k, v) in fields.iter() {
        match (*k, v) {
            ("line", MdField::UInt(x)) => line = *x,
            ("column", MdField::UInt(x)) => col = *x,
            ("scope", MdField::Node(s)) => scope = *s,
            ("inlinedAt", _) => return None,
            _ => {}
        }
    }
    Some((line, col, scope?))
}

/// The enclosing scope of a lexical block (`DILexicalBlock[File]`); none
/// for a subprogram.
fn parent_scope(m: &Module, s: MetadataId) -> Option<MetadataId> {
    let MdNode::Specialized { name: "DILexicalBlock" | "DILexicalBlockFile", fields, .. } =
        m.md_node(s)
    else {
        return None;
    };
    fields.iter().find_map(|(k, v)| match (*k, v) {
        ("scope", MdField::Node(p)) => *p,
        _ => None,
    })
}
