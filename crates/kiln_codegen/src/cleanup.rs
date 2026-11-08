//! The cleanup and EH scope stack (spec/codegen.md §5.3, §7b): a port of
//! clang's `EHScopeStack` (cleanup, catch and terminate scopes) and of
//! `CGCleanup.cpp`'s `PopCleanupBlock`, `EmitBranchThroughCleanup` and the
//! branch-fixup machinery, so that scope exits through destructors produce
//! clang's block shapes (`cleanup.dest.slot`, the exit switch,
//! `cleanup.cont`) and EH cleanups clang's `ehcleanup` blocks (WP-100).

use kiln_ast::{DeclId, QualType};
use kiln_llvm::{IntPredicate, MemOpts, TypeId as LTy};

use crate::fir::{Blk, Op, V};
use crate::func::{Addr, CallArg};
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
    /// (`DestroyObject` with `destroyCXXObject`, `CallBaseDtor`).
    Dtor { addr: V, record: DeclId, base: bool },
    /// `CallDelegatingCtorDtor`: a [`Cleanup::Dtor`] of `*this` whose
    /// base-object variant forwards the current function's VTT.
    DelegatingDtor { addr: V, record: DeclId, base: bool },
    /// Destroys the constant-length array at `addr` of objects of class
    /// `record` (`emitDestroy` + `emitArrayDestroy`).
    ArrayDtor { addr: Addr, record: DeclId },
    /// A [`Cleanup::Dtor`] (complete object) or, with `array`, a
    /// [`Cleanup::ArrayDtor`] pushed inside a conditional branch at an
    /// address that does not dominate the cleanup: the address is
    /// reloaded from the `cond-cleanup.save` slot `slot`
    /// (`DominatingValue<Address>::restore`); `ty`/`align` describe the
    /// object.
    SavedObject { slot: V, ty: LTy, align: u64, record: DeclId, array: bool },
    /// The destructor of an NRVO variable, skipped when the NRVO flag is
    /// set (`DestroyNRVOVariableCXX`).
    NrvoDtor { addr: V, record: DeclId, flag: V },
    /// Destroys a field of `this` in a base destructor (`DestroyField`).
    Field { field: DeclId, record: DeclId },
    /// Destroys a base subobject of `this` (`CallBaseDtor`): a
    /// non-virtual base in a base-object destructor, a virtual base
    /// (`virt`) in a complete-object one.
    BaseOfThis { base: DeclId, offset: u64, virt: bool },
    /// Frees an exception object whose initialization threw
    /// (`FreeException`); `saved` when `exn` is the
    /// `cond-cleanup.save` slot holding it.
    FreeException { exn: V, saved: bool },
    /// Destroys, in reverse order, the elements in `[begin, end)` of an
    /// array of class `record` whose construction threw
    /// (`RegularPartialArrayDestroy`, [except.ctor]/3).
    PartialArray { begin: V, end: V, elem: LTy, record: DeclId },
    /// Destroys, in reverse order, the elements of an array being
    /// initialized from a braced list up to the pointer stored in
    /// `end_slot` (`IrregularPartialArrayDestroy`, [except.ctor]/3).
    /// `outer` is the array's element type; when it is itself an array
    /// (`depth` levels deep), both ends are first drilled down to the
    /// innermost `elem` objects (`emitPartialArrayDestroy`).
    IrregularPartialArray(Box<IrregularPartialArray>),
    /// A [`Cleanup::PartialArray`] pushed inside a conditional branch,
    /// its ends saved where they do not dominate the cleanup.
    SavedPartialArray(Box<SavedPartialArray>),
    /// Calls the deallocation function of a new-expression whose
    /// initialization threw (`CallDeleteDuringNew`, [expr.new]/28).
    NewDelete(Box<NewDelete>),
    /// Calls the deallocation function of an array delete-expression
    /// even if a destructor throws (`CallArrayDelete`, [expr.delete]/7).
    ArrayDelete(Box<ArrayDelete>),
    /// Ends a catch handler (`CallEndCatch`); `might_throw` when the
    /// caught object has a class type whose destructor may throw.
    EndCatch { might_throw: bool },
    /// Releases a static-storage guard variable whose initialization threw
    /// (`CallGuardAbort`, [stmt.dcl]/3).
    GuardAbort { guard: V },
    /// Restores the stack pointer saved in `slot` before the scope's
    /// first variable length array (`CallStackRestore`, WP-183).
    StackRestore { slot: V },
    /// Calls the deallocation function `op_delete` of class type `ty` on
    /// `ptr` (`this`) after the complete-object destructor of a deleting
    /// destructor, even if that destructor throws (`CallDtorDelete`).
    DtorDelete { op_delete: DeclId, ptr: V, ty: QualType },
    /// Frees a coroutine frame: `if (coro.free(id, begin)) deallocate`
    /// (clang `CallCoroDelete`, spec/codegen.md §7o).
    CoroDelete { dealloc: kiln_ast::ExprId },
    /// `coro.end(null, true)` on the exceptions path out of a coroutine
    /// (clang `CallCoroEnd`): the ramp resumes unwinding, a resumed
    /// coroutine continues to the enclosing cleanups.
    CoroEnd,
    /// The destructor of the coroutine's `get_return_object()` result,
    /// run only once `flag` (`gro.active`) is set (clang's active flag
    /// on the `__coro_gro` cleanups).
    FlaggedDtor { addr: V, record: DeclId, flag: V },
    /// `llvm.lifetime.end` of the `size`-byte alloca `addr` (clang
    /// `CallLifetimeEnd`, pushed as `NormalEHLifetimeMarker`;
    /// spec/codegen.md §7p).
    LifetimeEnd { addr: V, size: u64 },
    /// Calls the GNU `cleanup` function `func` with the address `addr`
    /// of a local variable (clang `CallCleanupFunction`, WP-807).
    AttrFn { func: DeclId, addr: V },
}

/// A value a cleanup uses: directly, or reloaded from the
/// `cond-cleanup.save` slot it was saved to inside a conditional branch
/// (`DominatingValue::save` / `restore`).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Saved {
    Direct(CallArg),
    Slot {
        slot: V,
        ty: LTy,
        align: u64,
    },
    /// An aggregate argument whose address was saved to the
    /// `cond-cleanup.save` slot `slot` (`DominatingValue<RValue>`'s
    /// `AggregateAddress`); `addr.v` is unused.
    AggSlot {
        slot: V,
        addr: Addr,
    },
}

/// The operands of a [`Cleanup::IrregularPartialArray`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct IrregularPartialArray {
    pub begin: V,
    /// `begin` is a `cond-cleanup.save` slot holding the first element
    /// (`saveValueInCond`).
    pub saved: bool,
    pub end_slot: V,
    pub outer: LTy,
    pub depth: usize,
    pub elem: LTy,
    pub record: DeclId,
}

/// The operands of a [`Cleanup::SavedPartialArray`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct SavedPartialArray {
    pub begin: Saved,
    pub end: Saved,
    pub elem: LTy,
    pub record: DeclId,
}

/// The operands of a [`Cleanup::NewDelete`].
#[derive(Clone, Debug)]
pub(crate) struct NewDelete {
    pub op_delete: DeclId,
    pub ptr: Saved,
    pub size: Saved,
    /// The allocated type's alignment (`allocAlign`).
    pub align: u64,
    /// The allocation was passed an alignment (`passAlignment`).
    pub pass_align: bool,
    /// The placement arguments and their types.
    pub placement: Vec<(Saved, QualType)>,
}

/// The operands of a [`Cleanup::ArrayDelete`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct ArrayDelete {
    pub op_delete: DeclId,
    /// The start of the allocation (before the array cookie).
    pub ptr: V,
    /// The element count read back from the cookie.
    pub num: V,
    /// The (innermost) element type.
    pub elem: QualType,
    /// The array cookie's size in bytes.
    pub cookie: u64,
}

/// A handler of a catch scope (clang `EHCatchScope::Handler`): the
/// type-info of the caught type, `None` for `catch (...)`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Handler {
    pub rtti: Option<V>,
    pub blk: Blk,
}

/// What a scope is (clang `EHScope::Kind`; filters are C++14-only).
#[derive(Clone, Debug)]
pub(crate) enum Kind {
    Cleanup(Cleanup),
    Catch(Vec<Handler>),
    Terminate,
}

/// One scope (clang `EHCleanupScope`, `EHCatchScope`, `EHTerminateScope`).
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "clang's EHCleanupScope bit-fields")]
pub(crate) struct Scope {
    pub kind: Kind,
    /// A cleanup run on normal exits.
    pub normal: bool,
    /// A cleanup run on unwinding.
    pub eh: bool,
    pub active: bool,
    pub active_flag: Option<Addr>,
    /// `shouldTestFlagInNormalCleanup`.
    pub test_flag: bool,
    /// `shouldTestFlagInEHCleanup`.
    pub test_flag_eh: bool,
    /// `getCachedLandingPad`.
    pub cached_lpad: Option<Blk>,
    /// `getCachedEHDispatchBlock`.
    pub cached_eh: Option<Blk>,
    normal_block: Option<Blk>,
    fixup_depth: usize,
    /// `ExtInfo` exists.
    ext: bool,
    branch_afters: Vec<(Blk, u32)>,
    branches: Vec<Blk>,
    /// `AuxAllocas`: allocas (and their stores) dropped again when the
    /// cleanup is popped without ever being emitted.
    pub aux: Vec<V>,
}

impl Scope {
    /// A normal cleanup.
    pub fn is_normal_cleanup(&self) -> bool {
        matches!(self.kind, Kind::Cleanup(_)) && self.normal
    }
    /// `EHCleanupScope::isLifetimeMarker`.
    pub fn is_lifetime_marker(&self) -> bool {
        matches!(self.kind, Kind::Cleanup(Cleanup::LifetimeEnd { .. }))
    }
    /// An EH scope (`!isNonEHScope`): catch, terminate or an EH cleanup.
    pub fn is_eh_scope(&self) -> bool {
        !matches!(self.kind, Kind::Cleanup(_)) || self.eh
    }
    fn cleanup(&self) -> &Cleanup {
        match &self.kind {
            Kind::Cleanup(c) => c,
            _ => panic!("kiln_codegen: top of the scope stack is not a cleanup"),
        }
    }
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
    pub dest_slot: Option<V>,
    /// `UnreachableBlock`.
    pub unreachable: Option<Blk>,
    /// `NextCleanupDestIndex`.
    pub next_index: u32,
    /// `LifetimeExtendedCleanupStack`.
    pub lifetime_extended: Vec<(Cleanup, Option<Addr>)>,
    /// `DeferredDeactivationCleanupStack`: cleanups deactivated when the
    /// enclosing `RunCleanupsScope` ends, each with the placeholder
    /// instruction standing for its dominating IP.
    pub deferred: Vec<(Depth, V)>,
    /// `ExceptionSlot`.
    pub exn_slot: Option<V>,
    /// `EHSelectorSlot`.
    pub sel_slot: Option<V>,
    /// `EHResumeBlock`.
    pub eh_resume: Option<Blk>,
    /// The `eh.resume` block was emitted (a `resume` may throw).
    pub eh_resume_used: bool,
    /// `TerminateLandingPad`.
    pub terminate_lpad: Option<Blk>,
    /// `TerminateHandler`.
    pub terminate_handler: Option<Blk>,
    /// `CurrentCleanupScopeDepth`.
    pub cur_scope: Depth,
    /// `PrologueCleanupDepth`.
    pub prologue: Depth,
}

impl Cleanups {
    pub fn new() -> Self {
        Cleanups { next_index: 1, ..Cleanups::default() }
    }

    /// `stable_begin()`.
    pub fn depth(&self) -> Depth {
        self.stack.len()
    }

    /// `getEnclosingNormalCleanup` of the scope at stable position `d`.
    pub fn enclosing(&self, d: Depth) -> Depth {
        (1..d).rev().find(|&i| self.stack[i - 1].is_normal_cleanup()).unwrap_or(0)
    }

    /// `getEnclosingEHScope` of the scope at stable position `d`.
    pub fn enclosing_eh(&self, d: Depth) -> Depth {
        (1..d).rev().find(|&i| self.stack[i - 1].is_eh_scope()).unwrap_or(0)
    }

    /// `getInnermostEHScope`.
    pub fn innermost_eh(&self) -> Depth {
        self.enclosing_eh(self.stack.len() + 1)
    }

    /// `getInnermostNormalCleanup`.
    pub fn innermost_normal(&self) -> Depth {
        self.enclosing(self.stack.len() + 1)
    }

    /// `getInnermostActiveNormalCleanup`.
    fn innermost_active(&self) -> Depth {
        (1..=self.stack.len())
            .rev()
            .find(|&i| self.stack[i - 1].is_normal_cleanup() && self.stack[i - 1].active)
            .unwrap_or(0)
    }

    /// `EHScopeStack::requiresLandingPad`: an EH scope other than a
    /// lifetime marker encloses the current point.
    pub fn requires_landing_pad(&self) -> bool {
        let mut d = self.innermost_eh();
        while d != 0 {
            if !self.stack[d - 1].is_lifetime_marker() {
                return true;
            }
            d = self.enclosing_eh(d);
        }
        false
    }

    /// `hasNormalCleanups`.
    pub fn has_normal_cleanups(&self) -> bool {
        self.innermost_normal() != 0
    }

    /// `popNullFixups`.
    fn pop_null_fixups(&mut self) {
        let min = self.stack[self.innermost_normal() - 1].fixup_depth;
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

/// How control leaves a cleanup's normal block.
enum Exit {
    Br(Blk),
    Switch { default: Blk, cases: Vec<(V, Blk)> },
}

impl Cg<'_> {
    /// `getJumpDestInCurrentScope`.
    pub(crate) fn jump_dest_here(&mut self) -> JumpDest {
        let blk = self.f.fir.create_block();
        let index = self.f.cl.next_index;
        self.f.cl.next_index += 1;
        JumpDest { blk, depth: Some(self.f.cl.depth()), index }
    }

    /// `getJumpDestInCurrentScope` for an existing block.
    pub(crate) fn jump_dest_to(&mut self, blk: Blk) -> JumpDest {
        let index = self.f.cl.next_index;
        self.f.cl.next_index += 1;
        JumpDest { blk, depth: Some(self.f.cl.depth()), index }
    }

    /// `EHScopeStack::pushCleanup` (`NormalAndEHCleanup`).
    pub(crate) fn push_cleanup(&mut self, c: Cleanup) {
        self.push_cleanup_kind(c, true, true);
    }

    /// `EHScopeStack::pushCleanup` of kind `normal` / `eh`: no EH cleanup
    /// without exceptions or under a terminate scope.
    pub(crate) fn push_cleanup_kind(&mut self, c: Cleanup, normal: bool, eh: bool) {
        let ie = self.f.cl.innermost_eh();
        let under_terminate = ie != 0 && matches!(self.f.cl.stack[ie - 1].kind, Kind::Terminate);
        let eh = eh && self.opts.exceptions && !under_terminate;
        self.push_scope(Kind::Cleanup(c), normal, eh);
    }

    /// `AddAuxAllocas(AllocaTracker.Take())`: the allocas created since
    /// `mark` belong to the cleanup on top of the stack.
    pub(crate) fn add_aux_allocas(&mut self, mark: u32) {
        let a = self.f.fir.allocas_from(mark);
        self.f.cl.stack.last_mut().expect("cleanup scope").aux.extend(a);
    }

    /// `pushFullExprCleanup`: outside a conditional branch a plain
    /// cleanup; inside one, its values are saved if they do not dominate
    /// the cleanup (`saveValueInCond`) and it runs only if its
    /// `cleanup.cond` flag, cleared before the outermost conditional and
    /// set here, says it was reached (`initFullExprCleanup`).
    pub(crate) fn push_full_expr_cleanup(&mut self, c: Cleanup, normal: bool, eh: bool) {
        if self.f.conds.is_empty() {
            self.push_cleanup_kind(c, normal, eh);
            return;
        }
        let c = self.save_cleanup_in_cond(c);
        self.push_cleanup_kind(c, normal, eh);
        let flag = self.create_cleanup_active_flag();
        self.init_full_expr_cleanup_with_flag(flag);
    }

    /// `initFullExprCleanupWithFlag` on the innermost cleanup.
    pub(crate) fn init_full_expr_cleanup_with_flag(&mut self, flag: Addr) {
        let s = self.f.cl.stack.last_mut().expect("cleanup scope");
        s.active_flag = Some(flag);
        s.test_flag = s.normal;
        s.test_flag_eh = s.eh;
    }

    /// `createCleanupActiveFlag`: a `cleanup.cond` flag cleared before
    /// the outermost conditional and set at the current location.
    pub(crate) fn create_cleanup_active_flag(&mut self) -> Addr {
        let outer = *self.f.conds.first().expect("in a conditional branch");
        let flag = self.f.fir.alloca(LTy::I1, 1);
        let f0 = V::C(self.m.const_bool(false));
        self.f.fir.insert_before_last(
            outer,
            Op::Store { val: f0, ptr: flag, mem: MemOpts::aligned(1) },
            LTy::VOID,
        );
        let t1 = V::C(self.m.const_bool(true));
        self.store(t1, flag, 1, false);
        Addr { v: flag, ty: LTy::I1, align: 1 }
    }

    /// `saveValueInCond` of a value whose instruction is outside the
    /// entry block (`DominatingLLVMValue::needsSaving`).
    fn needs_saving(&self, v: V) -> bool {
        let entry = self.f.fir.entry();
        matches!(v, V::I(i) if self.f.fir.inst_block(i) != Some(entry))
    }

    /// `DominatingLLVMValue::save` of a pointer: a `cond-cleanup.save`
    /// slot when `v` does not dominate the cleanup.
    fn save_ptr(&mut self, v: V) -> Saved {
        if !self.needs_saving(v) {
            return Saved::Direct(CallArg::Scalar(v));
        }
        let al = self.pref_align(LTy::PTR);
        let slot = self.f.fir.alloca(LTy::PTR, al);
        self.store(v, slot, al, false);
        Saved::Slot { slot, ty: LTy::PTR, align: al }
    }

    /// The `saveValueInCond` half of `pushFullExprCleanup` inside a
    /// conditional branch: `c` with the values that do not dominate the
    /// cleanup replaced by `cond-cleanup.save` slots.
    pub(crate) fn save_cleanup_in_cond(&mut self, c: Cleanup) -> Cleanup {
        match c {
            Cleanup::FreeException { exn, saved: false } if self.needs_saving(exn) => {
                let al = self.pref_align(LTy::PTR);
                let slot = self.f.fir.alloca(LTy::PTR, al);
                self.store(exn, slot, al, false);
                Cleanup::FreeException { exn: slot, saved: true }
            }
            Cleanup::PartialArray { begin, end, elem, record }
                if self.needs_saving(begin) || self.needs_saving(end) =>
            {
                // `pushRegularPartialArrayCleanup` inside a conditional
                // branch (e.g. the elements of a nothrow `new[]`).
                let begin = self.save_ptr(begin);
                let end = self.save_ptr(end);
                Cleanup::SavedPartialArray(Box::new(SavedPartialArray { begin, end, elem, record }))
            }
            Cleanup::Dtor { addr, record, base: false } if self.needs_saving(addr) => {
                let ty = self.record_ty(record);
                let al = self.pref_align(LTy::PTR);
                let slot = self.f.fir.alloca(LTy::PTR, al);
                self.store(addr, slot, al, false);
                Cleanup::SavedObject { slot, ty, align: 1, record, array: false }
            }
            Cleanup::ArrayDtor { addr, record } if self.needs_saving(addr.v) => {
                let al = self.pref_align(LTy::PTR);
                let slot = self.f.fir.alloca(LTy::PTR, al);
                self.store(addr.v, slot, al, false);
                Cleanup::SavedObject { slot, ty: addr.ty, align: addr.align, record, array: true }
            }
            Cleanup::IrregularPartialArray(mut p) if !p.saved && self.needs_saving(p.begin) => {
                let al = self.pref_align(LTy::PTR);
                let slot = self.f.fir.alloca(LTy::PTR, al);
                self.store(p.begin, slot, al, false);
                p.begin = slot;
                p.saved = true;
                Cleanup::IrregularPartialArray(p)
            }
            Cleanup::NewDelete(mut nd) => {
                // EnterNewDeleteCleanup: the pointer, the size, then the
                // placement arguments.
                self.save_new_delete_arg(&mut nd.ptr);
                self.save_new_delete_arg(&mut nd.size);
                for (a, _) in &mut nd.placement {
                    self.save_new_delete_arg(a);
                }
                Cleanup::NewDelete(nd)
            }
            c => c,
        }
    }

    /// `DominatingValue<RValue>::save` of an argument of a
    /// [`Cleanup::NewDelete`].
    fn save_new_delete_arg(&mut self, s: &mut Saved) {
        match *s {
            Saved::Direct(CallArg::Scalar(v)) if self.needs_saving(v) => {
                let ty = self.f.fir.ty(&self.m, v);
                let align = self.pref_align(ty);
                let slot = self.f.fir.alloca(ty, align);
                self.store(v, slot, align, false);
                *s = Saved::Slot { slot, ty, align };
            }
            Saved::Direct(CallArg::Agg(addr)) if self.needs_saving(addr.v) => {
                let al = self.pref_align(LTy::PTR);
                let slot = self.f.fir.alloca(LTy::PTR, al);
                self.store(addr.v, slot, al, false);
                *s = Saved::AggSlot { slot, addr };
            }
            _ => {}
        }
    }

    fn push_scope(&mut self, kind: Kind, normal: bool, eh: bool) {
        let fixup_depth = self.f.cl.fixups.len();
        self.f.cl.stack.push(Scope {
            kind,
            normal,
            eh,
            active: true,
            active_flag: None,
            test_flag: false,
            test_flag_eh: false,
            cached_lpad: None,
            cached_eh: None,
            normal_block: None,
            fixup_depth,
            ext: false,
            branch_afters: Vec::new(),
            branches: Vec::new(),
            aux: Vec::new(),
        });
    }

    /// `EHScopeStack::pushTerminate`.
    pub(crate) fn push_terminate(&mut self) {
        self.push_scope(Kind::Terminate, false, false);
    }

    /// `EHScopeStack::popTerminate`.
    pub(crate) fn pop_terminate(&mut self) {
        let s = self.f.cl.stack.pop();
        debug_assert!(matches!(s.map(|s| s.kind), Some(Kind::Terminate)));
    }

    /// `EHScopeStack::pushCatch`.
    pub(crate) fn push_catch(&mut self, handlers: Vec<Handler>) {
        self.push_scope(Kind::Catch(handlers), false, false);
    }

    /// `EHScopeStack::popCatch`.
    pub(crate) fn pop_catch(&mut self) -> Scope {
        let s = self.f.cl.stack.pop().expect("catch scope");
        debug_assert!(matches!(s.kind, Kind::Catch(_)));
        s
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
    pub(crate) fn unreachable_block(&mut self) -> Blk {
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
                let next = self.f.cl.enclosing(i);
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
            let ld = self.f.fir.insert_before(
                t,
                Op::Load { ty: LTy::I32, ptr: slot, mem: mem4() },
                LTy::I32,
            );
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
        self.pop_cleanup_blocks_reload(old, &mut []);
    }

    /// `PopCleanupBlocks(Old, ValuesToReload)`: when a popped cleanup had
    /// branches (its block is shared, so the current block is no longer
    /// dominated by the values computed before it), each instruction in
    /// `reload` is spilled to a `tmp.exprcleanup` temporary right after
    /// its definition and reloaded at the insertion point.
    pub(crate) fn pop_cleanup_blocks_reload(&mut self, old: Depth, reload: &mut [&mut V]) {
        let mut had_branches = false;
        while self.f.cl.depth() != old {
            let d = self.f.cl.depth();
            let fallthrough_is_branch_through = old < self.f.cl.enclosing(d);
            had_branches |= self.f.cl.stack.last().is_some_and(Scope::has_branches);
            self.pop_cleanup_block(fallthrough_is_branch_through);
        }
        if !had_branches || self.f.fir.cur().is_none() {
            return;
        }
        for v in reload.iter_mut() {
            let V::I(i) = **v else { continue };
            if matches!(self.f.fir.op(i), Op::Alloca { .. }) || self.f.fir.inst_block(i).is_none() {
                continue;
            }
            let ty = self.f.fir.ty(&self.m, **v);
            let align = self.pref_align(ty);
            let tmp = self.f.fir.alloca(ty, align);
            self.f.fir.insert_after(
                i,
                Op::Store { val: **v, ptr: tmp, mem: crate::func::mem(align, false) },
                LTy::VOID,
            );
            **v = self.load(ty, tmp, align, false);
        }
    }

    /// `PopCleanupBlocks(Old, OldLifetimeExtendedSize)`: pops to `old`,
    /// then pushes the lifetime-extended cleanups recorded since.
    /// With `ValuesToReload` ([`Cg::pop_cleanup_blocks_reload`]).
    pub(crate) fn pop_cleanup_blocks_le_reload(
        &mut self,
        old: Depth,
        old_le: usize,
        reload: &mut [&mut V],
    ) {
        self.pop_cleanup_blocks_reload(old, reload);
        let pending: Vec<_> = self.f.cl.lifetime_extended.drain(old_le..).collect();
        for (c, flag) in pending {
            self.push_cleanup(c);
            if let Some(f) = flag {
                self.init_full_expr_cleanup_with_flag(f);
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

    /// `PopCleanupBlock`.
    pub(crate) fn pop_cleanup_block(&mut self, fallthrough_is_branch_through: bool) {
        self.pop_cleanup_block_ex(fallthrough_is_branch_through, false);
    }

    /// `PopCleanupBlock(FallthroughIsBranchThrough, ForDeactivation)`.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn pop_cleanup_block_ex(
        &mut self,
        fallthrough_is_branch_through: bool,
        for_deactivation: bool,
    ) {
        let top = self.f.cl.depth();
        let deact_ip = if for_deactivation {
            let ip = self.f.fir.cur();
            self.f.fir.set_cur(None);
            Some(ip)
        } else {
            None
        };
        let (is_active, flag, eh_flag, fixup_depth, has_existing_branches, is_normal, eh_entry) = {
            let s = self.f.cl.stack.last().expect("cleanup scope");
            (
                s.active,
                s.active_flag.filter(|_| s.test_flag),
                s.active_flag.filter(|_| s.test_flag_eh),
                s.fixup_depth,
                s.has_branches(),
                s.normal,
                s.cached_eh,
            )
        };
        let eh_parent = self.f.cl.enclosing_eh(top);
        let has_fixups = self.f.cl.fixups.len() != fixup_depth;
        let fallthrough_source = self.f.fir.cur();
        let has_fallthrough = fallthrough_source.is_some() && (is_active || has_existing_branches);
        let has_prebranched = fallthrough_source.is_some_and(|b| self.f.fir.is_terminated(b));
        let requires_normal = is_normal && (has_fixups || has_existing_branches || has_fallthrough);
        let cleanup = self.f.cl.stack[top - 1].cleanup().clone();
        let aux = std::mem::take(&mut self.f.cl.stack[top - 1].aux);
        if !requires_normal && eh_entry.is_none() {
            for &a in aux.iter().rev() {
                self.f.fir.erase_with_users(a);
            }
        }

        if is_normal && has_prebranched && !requires_normal {
            let pre_dest = if fallthrough_is_branch_through {
                let e = self.f.cl.enclosing(top);
                self.normal_entry(e)
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
        } else if has_fallthrough && !has_prebranched && !has_fixups && !has_existing_branches {
            self.destroy_optimistic_entry(top);
            self.f.cl.pop();
            self.emit_cleanup(&cleanup, flag, false);
        } else {
            self.emit_normal_cleanup_paths(
                top,
                &cleanup,
                flag,
                fallthrough_is_branch_through,
                (fallthrough_source, has_fallthrough, has_prebranched),
                (has_fixups, has_existing_branches, fixup_depth),
            );
        }
        if let Some(ip) = deact_ip {
            self.f.fir.set_cur(ip);
        }
        if let Some(eh_entry) = eh_entry {
            let saved = self.f.fir.cur();
            self.f.fir.set_cur(None);
            self.emit_block(eh_entry, false);
            let next = self.eh_dispatch_block(eh_parent);
            self.push_terminate();
            if eh_flag.is_some() || is_active {
                self.emit_cleanup(&cleanup, eh_flag, true);
            }
            self.br(next);
            self.pop_terminate();
            self.f.fir.set_cur(saved);
            self.simplify_cleanup_entry(eh_entry);
        }
    }

    /// The normal-cleanup emission of `PopCleanupBlock` (steps I-VI) for
    /// a cleanup that needs its own normal entry block.
    #[allow(clippy::too_many_lines)]
    fn emit_normal_cleanup_paths(
        &mut self,
        top: Depth,
        cleanup: &Cleanup,
        flag: Option<Addr>,
        fallthrough_is_branch_through: bool,
        (fallthrough_source, has_fallthrough, has_prebranched): (Option<Blk>, bool, bool),
        (has_fixups, _, fixup_depth): (bool, bool, usize),
    ) {
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

        let enclosing = self.f.cl.enclosing(top);
        let has_enclosing = enclosing != 0;
        let (has_throughs, n_afters, after0) = {
            let s = &self.f.cl.stack[top - 1];
            (s.has_branch_throughs(), s.branch_afters.len(), s.branch_afters.first().copied())
        };
        let mut through_dest = None;
        if has_throughs
            || (fallthrough_source.is_some() && fallthrough_is_branch_through)
            || (has_fixups && has_enclosing)
        {
            through_dest = Some(self.normal_entry(enclosing));
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
            let branch_afters = self.f.cl.stack[top - 1].branch_afters.clone();
            for (b, i) in branch_afters {
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
        self.emit_cleanup(cleanup, flag, false);
        let normal_exit = self.f.fir.cur().expect("cleanup ends with an insertion point");
        match exit {
            Exit::Br(b) => {
                self.f.fir.push(Op::Br { dest: b }, LTy::VOID);
            }
            Exit::Switch { default, cases } => {
                // `createLoadInstBefore(.., nullptr)`, appended to the
                // exit block directly: no access group.
                let slot = self.dest_slot();
                let ld = self.f.fir.push_in(
                    normal_exit,
                    Op::Load { ty: LTy::I32, ptr: slot, mem: mem4() },
                    LTy::I32,
                );
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

    /// `EmitCleanup`: runs `c` (on the EH path if `for_eh`), testing the
    /// active flag if any.
    fn emit_cleanup(&mut self, c: &Cleanup, flag: Option<Addr>, for_eh: bool) {
        let mut cont = None;
        if let Some(f) = flag {
            let done = self.f.fir.create_block();
            let action = self.f.fir.create_block();
            let v = self.load(f.ty, f.v, f.align, false);
            self.cond_br(v, action, done);
            self.emit_block(action, false);
            cont = Some(done);
        }
        self.run_cleanup(c, for_eh);
        if let Some(d) = cont {
            self.emit_block(d, false);
        }
    }

    /// The body of a cleanup (`Cleanup::Emit`).
    fn run_cleanup(&mut self, c: &Cleanup, for_eh: bool) {
        match *c {
            Cleanup::Dtor { addr, record, base } => self.call_dtor(record, addr, base),
            Cleanup::DelegatingDtor { addr, record, base } => {
                self.call_dtor_delegating(record, addr, base);
            }
            // Along the exceptions path the destructor always runs.
            Cleanup::NrvoDtor { addr, record, .. } if for_eh => {
                self.call_dtor(record, addr, false);
            }
            Cleanup::NrvoDtor { addr, record, flag } => {
                let skip = self.f.fir.create_block();
                let run = self.f.fir.create_block();
                let v = self.load(LTy::I1, flag, 1, false);
                self.cond_br(v, skip, run);
                self.emit_block(run, false);
                self.call_dtor(record, addr, false);
                self.emit_block(skip, false);
            }
            // clang `isForNormalCleanup() && useEHCleanupForArray`.
            Cleanup::ArrayDtor { addr, record } => self.array_destroy_at(addr, record, !for_eh),
            Cleanup::Field { field, record } => {
                self.destroy_field_of_this(field, record, !for_eh);
            }
            Cleanup::BaseOfThis { base, offset, virt } => {
                let this = self.this_value();
                let p = self.byte_offset(this, offset);
                self.call_dtor_of(base, p, true, virt);
            }
            Cleanup::FreeException { exn, saved } => {
                let exn = if saved { self.load(LTy::PTR, exn, 8, false) } else { exn };
                let f = self.rt_fn("__cxa_free_exception", LTy::VOID, &[LTy::PTR], &[]);
                self.nounwind_runtime_call(f, LTy::VOID, vec![exn]);
            }
            Cleanup::PartialArray { begin, end, elem, record } => {
                self.array_destroy_range(begin, end, elem, record, true, false);
            }
            Cleanup::SavedPartialArray(ref p) => {
                let SavedPartialArray { begin, end, elem, record } = **p;
                let (CallArg::Scalar(begin), CallArg::Scalar(end)) =
                    (self.restore_saved(begin), self.restore_saved(end))
                else {
                    return;
                };
                self.array_destroy_range(begin, end, elem, record, true, false);
            }
            Cleanup::SavedObject { slot, ty, align, record, array } => {
                let al = self.pref_align(LTy::PTR);
                let v = self.load(LTy::PTR, slot, al, false);
                if array {
                    self.array_destroy_at(Addr { v, ty, align }, record, !for_eh);
                } else {
                    self.call_dtor(record, v, false);
                }
            }
            Cleanup::IrregularPartialArray(ref p) => {
                let IrregularPartialArray { begin, saved, end_slot, outer, depth, elem, record } =
                    **p;
                let al = self.pref_align(LTy::PTR);
                let begin = if saved { self.load(LTy::PTR, begin, al, false) } else { begin };
                let end = self.load(LTy::PTR, end_slot, al, false);
                let (begin, end) = if depth > 0 {
                    let zero = self.i64c(0);
                    let b = self.gep(outer, begin, vec![zero; depth + 1], true);
                    let e = self.gep(outer, end, vec![zero; depth + 1], true);
                    (b, e)
                } else {
                    (begin, end)
                };
                self.array_destroy_range(begin, end, elem, record, true, false);
            }
            Cleanup::NewDelete(ref nd) => {
                if let Err(e) = self.emit_new_delete_cleanup(nd) {
                    self.f.error.get_or_insert(e);
                }
            }
            Cleanup::ArrayDelete(ref ad) => {
                let ArrayDelete { op_delete, ptr, num, elem, cookie } = **ad;
                let loc = Some(self.ast.decls.get(op_delete).loc());
                if let Err(e) =
                    self.emit_delete_call_n(op_delete, ptr, elem, Some((num, cookie)), loc)
                {
                    self.f.error.get_or_insert(e);
                }
            }
            Cleanup::EndCatch { might_throw } => {
                let f = self.rt_fn("__cxa_end_catch", LTy::VOID, &[], &[]);
                if might_throw {
                    self.runtime_call_or_invoke(f, LTy::VOID, Vec::new(), &[]);
                } else {
                    self.nounwind_runtime_call(f, LTy::VOID, Vec::new());
                }
            }
            Cleanup::GuardAbort { guard } => self.emit_guard_abort(guard),
            Cleanup::StackRestore { slot } => self.emit_stack_restore(slot),
            Cleanup::DtorDelete { op_delete, ptr, ty } => {
                let loc = Some(self.ast.decls.get(op_delete).loc());
                if let Err(e) = self.emit_delete_call(op_delete, ptr, ty, loc) {
                    self.f.error.get_or_insert(e);
                }
            }
            Cleanup::CoroDelete { dealloc } => {
                if let Err(e) = self.emit_coro_delete(dealloc) {
                    self.f.error.get_or_insert(e);
                }
            }
            Cleanup::CoroEnd => self.emit_coro_end_cleanup(),
            Cleanup::FlaggedDtor { addr, record, flag } => {
                let skip = self.f.fir.create_block();
                let run = self.f.fir.create_block();
                let v = self.load(LTy::I1, flag, 1, false);
                self.cond_br(v, run, skip);
                self.emit_block(run, false);
                self.call_dtor(record, addr, false);
                self.emit_block(skip, false);
            }
            Cleanup::LifetimeEnd { addr, size } => self.emit_lifetime_end(addr, size),
            Cleanup::AttrFn { func, addr } => {
                if let Err(e) = self.emit_cleanup_fn_call(func, addr) {
                    self.f.error.get_or_insert(e);
                }
            }
        }
    }

    /// `emitArrayDestroy` for a non-empty array of constant length.
    pub(crate) fn array_destroy(&mut self, begin: V, n: u64, elem: LTy, record: DeclId, eh: bool) {
        let nn = self.i64c(i128::from(n));
        let end = self.gep(elem, begin, vec![nn], true);
        self.array_destroy_range(begin, end, elem, record, false, eh);
    }

    /// `emitArrayDestroy` of `[begin, end)`; `check_zero` tests for an
    /// empty range first (`checkZeroLength`); `eh` destroys the elements
    /// not yet destroyed if a destructor throws (`useEHCleanup`,
    /// [except.ctor]/2).
    pub(crate) fn array_destroy_range(
        &mut self,
        begin: V,
        end: V,
        elem: LTy,
        record: DeclId,
        check_zero: bool,
        eh: bool,
    ) {
        let body = self.f.fir.create_block();
        let out = self.f.fir.create_block();
        if check_zero {
            let empty = self.icmp(IntPredicate::Eq, begin, end);
            self.cond_br(empty, out, body);
        }
        let entry = self.f.fir.cur().expect("insertion point");
        self.emit_block(body, false);
        let phi = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(end, entry)] }, LTy::PTR);
        let m1 = self.i64c(-1);
        let el = self.gep(elem, phi, vec![m1], true);
        if eh {
            self.push_cleanup_kind(
                Cleanup::PartialArray { begin, end: el, elem, record },
                false,
                true,
            );
        }
        self.call_dtor(record, el, false);
        if eh {
            self.pop_cleanup_block(false);
        }
        let done = self.icmp(IntPredicate::Eq, el, begin);
        let cur = self.f.fir.cur().expect("insertion point");
        self.f.fir.add_incoming(crate::fir::Fir::last_inst(phi), el, cur);
        self.cond_br(done, out, body);
        self.emit_block(out, false);
    }
}
