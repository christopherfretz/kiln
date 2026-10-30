//! The normal-cleanup stack (spec/codegen.md §5.3): a port of clang's
//! `EHScopeStack` restricted to normal cleanups and of `CGCleanup.cpp`'s
//! `PopCleanupBlock`, `EmitBranchThroughCleanup` and the branch-fixup
//! machinery, so that scope exits through destructors produce clang's
//! block shapes (`cleanup.dest.slot`, the exit switch, `cleanup.cont`).
//!
//! Exception-handling cleanups are not modelled: kiln emits the normal
//! path only, which is exact under `-fno-exceptions` (spec gap: landing
//! pads under `-fexceptions`).

use kiln_ast::DeclId;
use kiln_llvm::{IntPredicate, MemOpts, TypeId as LTy};

use crate::fir::{Blk, Op, V};
use crate::func::Addr;
use crate::module::Cg;

/// A stable position in the cleanup stack (clang `stable_iterator`): the
/// number of scopes below and including it; `0` is `stable_end()`.
pub(crate) type Depth = usize;

/// A branch destination with the cleanup depth it lives at (clang
/// `JumpDest`); `depth` is `None` for a label not yet emitted.
#[derive(Clone, Copy, Debug)]
pub(crate) struct JumpDest {
    pub blk: Blk,
    pub depth: Option<Depth>,
    pub index: u32,
}

/// What a cleanup does when it runs (clang's `EHScopeStack::Cleanup`
/// subclasses).
#[derive(Clone, Debug)]
pub(crate) enum Cleanup {
    /// Calls the destructor variant of `record` on the object at `addr`
    /// (`DestroyObject` with `destroyCXXObject`, `CallBaseDtor`,
    /// `CallDtorDelegating`).
    Dtor { addr: V, record: DeclId, base: bool },
    /// Destroys an array of `n` objects of class `record` (element IR type
    /// `elem`) starting at `begin` (`emitArrayDestroy`).
    ArrayDtor { begin: V, n: u64, elem: LTy, record: DeclId },
    /// The destructor of an NRVO variable, skipped when the NRVO flag is
    /// set (`DestroyNRVOVariableCXX`).
    NrvoDtor { addr: V, record: DeclId, flag: V },
    /// Destroys a field of `this` in a base destructor (`DestroyField`).
    Field { field: DeclId, record: DeclId },
    /// Destroys a non-virtual base subobject of `this` in a base
    /// destructor (`CallBaseDtor`).
    BaseOfThis { base: DeclId, offset: u64 },
    /// Calls `operator delete(ptr)` for a new-expression whose initializer
    /// is running (normal path only: never needed without exceptions).
    Nop,
}

/// One scope (clang `EHCleanupScope`, normal cleanups only).
#[derive(Clone, Debug)]
pub(crate) struct Scope {
    pub cleanup: Cleanup,
    pub active: bool,
    pub active_flag: Option<Addr>,
    pub test_flag: bool,
    normal_block: Option<Blk>,
    fixup_depth: usize,
    /// `ExtInfo` exists.
    ext: bool,
    branch_afters: Vec<(Blk, u32)>,
    branches: Vec<Blk>,
}

impl Scope {
    fn has_branches(&self) -> bool {
        self.ext && !self.branches.is_empty()
    }
    fn has_branch_throughs(&self) -> bool {
        self.ext && self.branch_afters.len() != self.branches.len()
    }
    fn add_branch_after(&mut self, index: u32, b: Blk) {
        self.ext = true;
        if !self.branches.contains(&b) {
            self.branches.push(b);
            self.branch_afters.push((b, index));
        }
    }
    fn add_branch_through(&mut self, b: Blk) -> bool {
        self.ext = true;
        if self.branches.contains(&b) {
            false
        } else {
            self.branches.push(b);
            true
        }
    }
}

/// A branch out of cleanups to a label not yet seen (clang `BranchFixup`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fixup {
    destination: Option<Blk>,
    index: u32,
    initial_branch: u32,
    optimistic: Option<Blk>,
}

/// The cleanup state of a function.
#[derive(Debug, Default)]
pub(crate) struct Cleanups {
    pub stack: Vec<Scope>,
    fixups: Vec<Fixup>,
    /// `NormalCleanupDest`.
    dest_slot: Option<V>,
    /// `UnreachableBlock`.
    pub unreachable: Option<Blk>,
    /// `NextCleanupDestIndex`.
    pub next_index: u32,
    /// `LifetimeExtendedCleanupStack`.
    pub lifetime_extended: Vec<(Cleanup, Option<Addr>)>,
}

impl Cleanups {
    pub fn new() -> Self {
        Cleanups { next_index: 1, ..Cleanups::default() }
    }

    /// `stable_begin()`.
    pub fn depth(&self) -> Depth {
        self.stack.len()
    }

    /// `getEnclosingNormalCleanup` of the scope at stable position `d`
    /// (every scope here is a normal cleanup).
    fn enclosing(d: Depth) -> Depth {
        d - 1
    }

    /// `getInnermostActiveNormalCleanup`.
    fn innermost_active(&self) -> Depth {
        let mut d = self.stack.len();
        while d != 0 {
            if self.stack[d - 1].active {
                return d;
            }
            d -= 1;
        }
        0
    }

    fn has_normal_cleanups(&self) -> bool {
        !self.stack.is_empty()
    }

    /// `popNullFixups`.
    fn pop_null_fixups(&mut self) {
        let min = self.stack.last().expect("normal cleanup").fixup_depth;
        while self.fixups.len() > min && self.fixups.last().is_some_and(|f| f.destination.is_none())
        {
            self.fixups.pop();
        }
    }

    /// `EHScopeStack::popCleanup`.
    fn pop(&mut self) {
        self.stack.pop();
        if !self.fixups.is_empty() {
            if self.has_normal_cleanups() {
                self.pop_null_fixups();
            } else {
                self.fixups.clear();
            }
        }
    }
}

fn mem4() -> MemOpts {
    MemOpts::aligned(4)
}

impl Cg<'_> {
    /// `getJumpDestInCurrentScope`.
    pub(crate) fn jump_dest_here(&mut self) -> JumpDest {
        let blk = self.f.fir.create_block();
        let index = self.f.cl.next_index;
        self.f.cl.next_index += 1;
        JumpDest { blk, depth: Some(self.f.cl.depth()), index }
    }

    /// `EHScopeStack::pushCleanup` (a normal cleanup).
    pub(crate) fn push_cleanup(&mut self, c: Cleanup) {
        let fixup_depth = self.f.cl.fixups.len();
        self.f.cl.stack.push(Scope {
            cleanup: c,
            active: true,
            active_flag: None,
            test_flag: false,
            normal_block: None,
            fixup_depth,
            ext: false,
            branch_afters: Vec::new(),
            branches: Vec::new(),
        });
    }

    /// `getNormalCleanupDestSlot`.
    fn dest_slot(&mut self) -> V {
        if let Some(s) = self.f.cl.dest_slot {
            return s;
        }
        let s = self.f.fir.alloca(LTy::I32, 4);
        self.f.cl.dest_slot = Some(s);
        s
    }

    /// `getUnreachableBlock`.
    fn unreachable_block(&mut self) -> Blk {
        if let Some(b) = self.f.cl.unreachable {
            return b;
        }
        let b = self.f.fir.create_block();
        self.f.fir.push_in(b, Op::Unreachable, LTy::VOID);
        self.f.cl.unreachable = Some(b);
        b
    }

    /// `FinishFunction`'s `EmitIfUsed(UnreachableBlock)`.
    pub(crate) fn emit_unreachable_if_used(&mut self) {
        if let Some(b) = self.f.cl.unreachable.take()
            && self.f.fir.block_uses(b) > 0
        {
            self.f.fir.place_at_end(b);
        }
    }

    fn store_index_before(&mut self, index: u32, at: u32) {
        let slot = self.dest_slot();
        let k = V::C(self.m.const_int(LTy::I32, i128::from(index)));
        self.f.fir.insert_before(at, Op::Store { val: k, ptr: slot, mem: mem4() }, LTy::VOID);
    }

    fn i32c(&mut self, v: u32) -> V {
        V::C(self.m.const_int(LTy::I32, i128::from(v)))
    }

    /// `CreateNormalEntry`.
    fn normal_entry(&mut self, d: Depth) -> Blk {
        if let Some(b) = self.f.cl.stack[d - 1].normal_block {
            return b;
        }
        let b = self.f.fir.create_block();
        self.f.cl.stack[d - 1].normal_block = Some(b);
        b
    }

    /// `EmitBranchThroughCleanup`.
    pub(crate) fn branch_through_cleanup(&mut self, dest: JumpDest) {
        if self.f.fir.cur().is_none() {
            return;
        }
        let bi = self.br(dest.blk);
        let top = self.f.cl.innermost_active();
        // `TopCleanup.encloses(Dest.getScopeDepth())` (false for invalid).
        if top == 0 || dest.depth.is_some_and(|d| top <= d) {
            self.f.fir.set_cur(None);
            return;
        }
        let Some(dd) = dest.depth else {
            self.f.cl.fixups.push(Fixup {
                destination: Some(dest.blk),
                index: dest.index,
                initial_branch: bi,
                optimistic: None,
            });
            self.f.fir.set_cur(None);
            return;
        };
        self.store_index_before(dest.index, bi);
        let entry = self.normal_entry(top);
        self.f.fir.set_successor(bi, dest.blk, entry);
        let mut i = top;
        if dd < i {
            loop {
                let next = Cleanups::enclosing(i);
                if dd >= next {
                    self.f.cl.stack[i - 1].add_branch_after(dest.index, dest.blk);
                    break;
                }
                if !self.f.cl.stack[i - 1].add_branch_through(dest.blk) {
                    break;
                }
                i = next;
            }
        }
        self.f.fir.set_cur(None);
    }

    /// `ResolveBranchFixups`.
    pub(crate) fn resolve_branch_fixups(&mut self, b: Blk) {
        if self.f.cl.fixups.is_empty() {
            return;
        }
        let mut modified: Vec<Blk> = Vec::new();
        let mut any = false;
        for i in 0..self.f.cl.fixups.len() {
            let fx = self.f.cl.fixups[i];
            if fx.destination != Some(b) {
                continue;
            }
            self.f.cl.fixups[i].destination = None;
            any = true;
            let Some(bb) = fx.optimistic else { continue };
            if modified.contains(&bb) {
                continue;
            }
            modified.push(bb);
            let sw = self.transition_to_cleanup_switch(bb);
            let k = self.i32c(fx.index);
            self.f.fir.add_switch_case(sw, k, b);
        }
        if any {
            self.f.cl.pop_null_fixups();
        }
    }

    /// `TransitionToCleanupSwitch`.
    fn transition_to_cleanup_switch(&mut self, b: Blk) -> u32 {
        let t = self.f.fir.terminator(b).expect("terminator");
        if let Op::Br { dest } = *self.f.fir.op(t) {
            let slot = self.dest_slot();
            let ld =
                self.f.fir.insert_before(t, Op::Load { ty: LTy::I32, ptr: slot, mem: mem4() }, LTy::I32);
            self.f.fir.erase(t);
            crate::fir::Fir::last_inst(self.f.fir.push_in(
                b,
                Op::Switch { c: ld, default: dest, cases: Vec::new() },
                LTy::VOID,
            ))
        } else {
            t
        }
    }

    /// `PopCleanupBlocks(Old)`.
    pub(crate) fn pop_cleanup_blocks(&mut self, old: Depth) {
        while self.f.cl.depth() != old {
            let d = self.f.cl.depth();
            let fallthrough_is_branch_through = old < Cleanups::enclosing(d);
            self.pop_cleanup_block(fallthrough_is_branch_through);
        }
    }

    /// `PopCleanupBlocks(Old, OldLifetimeExtendedSize)`: pops to `old`,
    /// then pushes the lifetime-extended cleanups recorded since.
    pub(crate) fn pop_cleanup_blocks_le(&mut self, old: Depth, old_le: usize) {
        self.pop_cleanup_blocks(old);
        let pending: Vec<_> = self.f.cl.lifetime_extended.drain(old_le..).collect();
        for (c, flag) in pending {
            self.push_cleanup(c);
            if let Some(f) = flag {
                let s = self.f.cl.stack.last_mut().expect("scope");
                s.active_flag = Some(f);
                s.test_flag = true;
            }
        }
    }

    /// `destroyOptimisticNormalEntry`.
    fn destroy_optimistic_entry(&mut self, d: Depth) {
        let Some(entry) = self.f.cl.stack[d - 1].normal_block else { return };
        if self.f.fir.block_uses(entry) == 0 {
            return;
        }
        let unr = self.unreachable_block();
        for t in self.f.fir.branches_to(entry) {
            self.f.fir.set_successor(t, entry, unr);
            if let Op::Switch { c, default, cases } = self.f.fir.op(t).clone()
                && cases.len() == 1
                && default == unr
            {
                let b = self.f.fir.inst_block(t).expect("live switch");
                let _ = b;
                self.f.fir.insert_before(t, Op::Br { dest: cases[0].1 }, LTy::VOID);
                self.f.fir.erase(t);
                if let V::I(ci) = c {
                    self.f.fir.erase(ci);
                }
            }
        }
    }

    /// `SimplifyCleanupEntry`.
    fn simplify_cleanup_entry(&mut self, entry: Blk) -> Blk {
        let Some(pred) = self.f.fir.single_predecessor(entry) else { return entry };
        let Some(t) = self.f.fir.terminator(pred) else { return entry };
        if !matches!(self.f.fir.op(t), Op::Br { .. }) {
            return entry;
        }
        self.f.fir.erase(t);
        self.f.fir.replace_block_uses(entry, pred);
        self.f.fir.splice_block(entry, pred);
        self.f.fir.erase_block(entry);
        pred
    }

    /// `PopCleanupBlock` (normal cleanups).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn pop_cleanup_block(&mut self, fallthrough_is_branch_through: bool) {
        let top = self.f.cl.depth();
        let (is_active, flag, fixup_depth, has_existing_branches) = {
            let s = self.f.cl.stack.last().expect("cleanup scope");
            (s.active, s.active_flag.filter(|_| s.test_flag), s.fixup_depth, s.has_branches())
        };
        let has_fixups = self.f.cl.fixups.len() != fixup_depth;
        let fallthrough_source = self.f.fir.cur();
        let has_fallthrough = fallthrough_source.is_some() && (is_active || has_existing_branches);
        let has_prebranched = fallthrough_source.is_some_and(|b| self.f.fir.is_terminated(b));
        let requires_normal = has_fixups || has_existing_branches || has_fallthrough;

        if has_prebranched && !requires_normal {
            let pre_dest = if fallthrough_is_branch_through {
                self.normal_entry(Cleanups::enclosing(top))
            } else {
                let b = self.f.fir.create_block();
                self.emit_block(b, false);
                b
            };
            let entry = self.f.cl.stack[top - 1].normal_block.expect("normal entry");
            let src = fallthrough_source.expect("source");
            let t = self.f.fir.terminator(src).expect("terminator");
            self.f.fir.set_successor(t, entry, pre_dest);
        }

        if !requires_normal {
            self.destroy_optimistic_entry(top);
            self.f.cl.pop();
            return;
        }
        let cleanup = self.f.cl.stack[top - 1].cleanup.clone();

        if has_fallthrough && !has_prebranched && !has_fixups && !has_existing_branches {
            self.destroy_optimistic_entry(top);
            self.f.cl.pop();
            self.emit_cleanup(&cleanup, flag);
            return;
        }

        let entry = self.normal_entry(top);
        let mut saved_inactive = None;
        if has_fallthrough {
            if !has_prebranched {
                let slot = self.dest_slot();
                let z = self.i32c(0);
                self.store(z, slot, 4, false);
            }
        } else if fallthrough_source.is_some() {
            saved_inactive = fallthrough_source;
            self.f.fir.set_cur(None);
        }
        self.emit_block(entry, false);

        let has_enclosing = Cleanups::enclosing(top) != 0;
        let (has_throughs, n_afters, after0) = {
            let s = &self.f.cl.stack[top - 1];
            (s.has_branch_throughs(), s.branch_afters.len(), s.branch_afters.first().copied())
        };
        let mut through_dest = None;
        if has_throughs
            || (fallthrough_source.is_some() && fallthrough_is_branch_through)
            || (has_fixups && has_enclosing)
        {
            through_dest = Some(self.normal_entry(Cleanups::enclosing(top)));
        }

        enum Exit {
            Br(Blk),
            Switch { default: Blk, cases: Vec<(V, Blk)> },
        }
        let mut fallthrough_dest = None;
        let exit = if !has_throughs && !has_fixups && !has_fallthrough && n_afters == 1 {
            let slot = self.dest_slot();
            let users = self.f.fir.users(slot);
            if users.len() == 1 {
                self.f.fir.erase(users[0]);
                self.f.fir.erase(crate::fir::Fir::last_inst(slot));
                self.f.cl.dest_slot = None;
            }
            Exit::Br(after0.expect("branch-after").0)
        } else if n_afters > 0
            || (has_fallthrough && !fallthrough_is_branch_through)
            || (has_fixups && !has_enclosing)
        {
            let default = match through_dest {
                Some(b) => b,
                None => self.unreachable_block(),
            };
            let _ = self.dest_slot();
            let mut cases = Vec::new();
            if fallthrough_source.is_some() && !fallthrough_is_branch_through {
                let ft = self.f.fir.create_block();
                fallthrough_dest = Some(ft);
                if has_fallthrough {
                    cases.push((self.i32c(0), ft));
                }
            }
            let afters = self.f.cl.stack[top - 1].branch_afters.clone();
            for (b, i) in afters {
                cases.push((self.i32c(i), b));
            }
            if has_fixups && !has_enclosing {
                // ResolveAllBranchFixups.
                let mut added: Vec<Blk> = Vec::new();
                for i in 0..self.f.cl.fixups.len() {
                    let fx = self.f.cl.fixups[i];
                    let Some(dst) = fx.destination else { continue };
                    if fx.optimistic.is_none() {
                        self.store_index_before(fx.index, fx.initial_branch);
                        self.f.fir.set_successor(fx.initial_branch, dst, entry);
                    }
                    if added.contains(&dst) {
                        continue;
                    }
                    added.push(dst);
                    cases.push((self.i32c(fx.index), dst));
                }
                self.f.cl.fixups.clear();
            }
            Exit::Switch { default, cases }
        } else {
            Exit::Br(through_dest.expect("branch-through destination"))
        };

        self.f.cl.pop();
        self.emit_cleanup(&cleanup, flag);
        let normal_exit = self.f.fir.cur().expect("cleanup ends with an insertion point");
        match exit {
            Exit::Br(b) => {
                self.f.fir.push(Op::Br { dest: b }, LTy::VOID);
            }
            Exit::Switch { default, cases } => {
                let slot = self.dest_slot();
                let ld = self.f.fir.push(Op::Load { ty: LTy::I32, ptr: slot, mem: mem4() }, LTy::I32);
                self.f.fir.push(Op::Switch { c: ld, default, cases }, LTy::VOID);
            }
        }

        for i in fixup_depth..self.f.cl.fixups.len() {
            let fx = self.f.cl.fixups[i];
            let Some(dst) = fx.destination else { continue };
            if fx.optimistic.is_none() {
                self.store_index_before(fx.index, fx.initial_branch);
                self.f.fir.set_successor(fx.initial_branch, dst, entry);
            }
            self.f.cl.fixups[i].optimistic = Some(normal_exit);
        }

        if !has_fallthrough && fallthrough_source.is_some() {
            self.f.fir.set_cur(saved_inactive);
        } else if has_fallthrough && let Some(ft) = fallthrough_dest {
            self.emit_block(ft, false);
        } else if has_fallthrough {
        } else {
            self.f.fir.set_cur(None);
        }

        let new_entry = self.simplify_cleanup_entry(entry);
        if new_entry != entry && entry == normal_exit {
            for i in fixup_depth..self.f.cl.fixups.len() {
                self.f.cl.fixups[i].optimistic = Some(new_entry);
            }
        }
    }

    /// `EmitCleanup`: runs `c`, testing the active flag if any.
    fn emit_cleanup(&mut self, c: &Cleanup, flag: Option<Addr>) {
        let mut cont = None;
        if let Some(f) = flag {
            let done = self.f.fir.create_block();
            let action = self.f.fir.create_block();
            let v = self.load(f.ty, f.v, f.align, false);
            self.cond_br(v, action, done);
            self.emit_block(action, false);
            cont = Some(done);
        }
        self.run_cleanup(c);
        if let Some(d) = cont {
            self.emit_block(d, false);
        }
    }

    /// The body of a cleanup (`Cleanup::Emit`).
    fn run_cleanup(&mut self, c: &Cleanup) {
        match *c {
            Cleanup::Dtor { addr, record, base } => self.call_dtor(record, addr, base),
            Cleanup::NrvoDtor { addr, record, flag } => {
                let skip = self.f.fir.create_block();
                let run = self.f.fir.create_block();
                let v = self.load(LTy::I1, flag, 1, false);
                self.cond_br(v, skip, run);
                self.emit_block(run, false);
                self.call_dtor(record, addr, false);
                self.emit_block(skip, false);
            }
            Cleanup::ArrayDtor { begin, n, elem, record } => {
                self.array_destroy(begin, n, elem, record);
            }
            Cleanup::Field { field, record } => self.destroy_field_of_this(field, record),
            Cleanup::BaseOfThis { base, offset } => {
                let this = self.this_value();
                let p = self.byte_offset(this, offset);
                self.call_dtor(base, p, true);
            }
            Cleanup::Nop => {}
        }
    }

    /// `emitArrayDestroy` for a non-empty array of constant length.
    pub(crate) fn array_destroy(&mut self, begin: V, n: u64, elem: LTy, record: DeclId) {
        let nn = self.i64c(i128::from(n));
        let end = self.gep(elem, begin, vec![nn], true);
        let body = self.f.fir.create_block();
        let entry = self.f.fir.cur().expect("insertion point");
        self.emit_block(body, false);
        let phi = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(end, entry)] }, LTy::PTR);
        let m1 = self.i64c(-1);
        let el = self.gep(elem, phi, vec![m1], true);
        self.call_dtor(record, el, false);
        let done = self.icmp(IntPredicate::Eq, el, begin);
        let out = self.f.fir.create_block();
        let cur = self.f.fir.cur().expect("insertion point");
        self.f.fir.add_incoming(crate::fir::Fir::last_inst(phi), el, cur);
        self.cond_br(done, out, body);
        self.emit_block(out, false);
    }
}
