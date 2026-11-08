//! The function IR buffer (spec/codegen.md §4).
//!
//! clang emits into a mutable LLVM function and then edits it: it erases
//! blocks nobody branches to, replaces the return block by its single
//! predecessor, folds forwarding blocks and deletes the store into the
//! return-value slot. `kiln_llvm` is append-only, so a function body is
//! first built here — a small mutable mirror of the instructions codegen
//! emits — and lowered into the module once it is final ([`Fir::lower`]).

mod mem2reg;

use kiln_llvm::{
    Attr, BinOp, BlockId, CallOpts, CastOp, Clause, CmpXchgOpts, FastMath, Flags, FloatPredicate,
    FuncId, GepFlags, IntPredicate, IrBuilder, IrResult, MdKindId, MemOpts, MetadataId, ModRef,
    Module, OperandBundle, Ordering, RmwOp, SyncScope, TypeId, Value, ValueRef,
};

/// A block of the buffer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Blk(pub u32);

/// An operand: an instruction result, an argument or a module-level value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum V {
    /// The result of instruction `n`.
    I(u32),
    /// Argument `n`.
    Arg(u32),
    /// A constant, global or function of the module.
    C(Value),
    /// A constructor/destructor symbol resolved at lowering (clang's
    /// complete-object declaration later replaced by the base variant or
    /// an alias; spec/codegen.md §7).
    P(u32),
}

/// One instruction.
#[derive(Clone, Debug)]
pub(crate) enum Op {
    Alloca {
        ty: TypeId,
        align: u64,
    },
    /// A dynamic `alloca ty, i64 count` at the current insertion point
    /// (a variable length array, WP-183).
    DynAlloca {
        ty: TypeId,
        count: V,
        align: u64,
    },
    Load {
        ty: TypeId,
        ptr: V,
        mem: MemOpts,
    },
    Store {
        val: V,
        ptr: V,
        mem: MemOpts,
    },
    Bin {
        op: BinOp,
        flags: Flags,
        l: V,
        r: V,
    },
    FNeg {
        v: V,
    },
    /// `freeze v` (WP-630).
    Freeze {
        v: V,
    },
    ICmp {
        p: IntPredicate,
        l: V,
        r: V,
    },
    FCmp {
        p: FloatPredicate,
        l: V,
        r: V,
    },
    Cast {
        op: CastOp,
        flags: Flags,
        v: V,
        to: TypeId,
    },
    Gep {
        src: TypeId,
        base: V,
        idx: Vec<V>,
        flags: GepFlags,
    },
    Phi {
        ty: TypeId,
        inc: Vec<(V, Blk)>,
    },
    Select {
        c: V,
        t: V,
        f: V,
    },
    Call {
        fn_ty: TypeId,
        callee: V,
        args: Vec<V>,
        opts: Box<CallOpts>,
    },
    /// `call void @llvm.assume(i1 true) [ "<tag>"(<args>) ]` (clang
    /// `CreateAlignmentAssumption`): the bundle operands are FIR values.
    AssumeBundle {
        fn_ty: TypeId,
        callee: Value,
        /// The `i1 true` constant.
        cond: Value,
        tag: &'static str,
        args: Vec<V>,
    },
    /// `invoke ... to label %normal unwind label %unwind` (spec/codegen.md §7b).
    Invoke {
        fn_ty: TypeId,
        callee: V,
        args: Vec<V>,
        opts: Box<CallOpts>,
        normal: Blk,
        unwind: Blk,
    },
    /// `callbr ... to label %normal [label %indirect...]` (`asm goto`).
    CallBr {
        fn_ty: TypeId,
        callee: V,
        args: Vec<V>,
        opts: Box<CallOpts>,
        normal: Blk,
        indirect: Vec<Blk>,
    },
    /// `indirectbr ptr %addr, [label %d...]` (computed `goto`; the
    /// destinations may repeat, as clang's `addDestination` makes them).
    IndirectBr {
        addr: V,
        dests: Vec<Blk>,
    },
    /// `landingpad { ptr, i32 } [cleanup] catch ptr <clause>...`.
    LandingPad {
        cleanup: bool,
        clauses: Vec<V>,
    },
    /// `resume { ptr, i32 } %v`.
    Resume {
        v: V,
    },
    Br {
        dest: Blk,
    },
    CondBr {
        c: V,
        t: Blk,
        f: Blk,
    },
    Switch {
        c: V,
        default: Blk,
        cases: Vec<(V, Blk)>,
    },
    Ret {
        v: Option<V>,
    },
    Unreachable,
    ExtractValue {
        agg: V,
        idx: u32,
    },
    InsertValue {
        agg: V,
        val: V,
        idx: u32,
    },
    /// `extractelement vec, idx` (WP-156).
    ExtractElement {
        vec: V,
        idx: V,
    },
    /// `insertelement vec, elt, idx`.
    InsertElement {
        vec: V,
        elt: V,
        idx: V,
    },
    /// `shufflevector a, b, mask`.
    ShuffleVector {
        a: V,
        b: V,
        mask: Vec<u32>,
    },
    /// `atomicrmw` (the result has `val`'s type).
    AtomicRmw {
        op: RmwOp,
        ptr: V,
        val: V,
        mem: MemOpts,
    },
    /// `cmpxchg` (the result is `{ T, i1 }`).
    CmpXchg {
        ptr: V,
        cmp: V,
        new: V,
        opts: CmpXchgOpts,
    },
    /// `fence`.
    Fence {
        ordering: Ordering,
        scope: SyncScope,
    },
}

/// `fmf` if `ty` may carry fast-math flags (LLVM `FPMathOperator`), else
/// none.
fn fp_math(m: &Module, ty: TypeId, fmf: FastMath) -> FastMath {
    if fmf != FastMath::NONE && kiln_llvm::is_fp_math_type(&m.types, ty, m.ir_version) {
        fmf
    } else {
        FastMath::NONE
    }
}

/// The [`Inst::fmf`] bit (not an LLVM flag) of a call taking the FP
/// scope's fast-math flags ([`Fir::mark_scope_calls`]).
const SCOPE_CALL: u8 = 0x80;

/// Whether `op` calls an LLVM intrinsic (`llvm.*`): its fast-math flags
/// are the FP scope's, a library call's the command line's (WP-806).
fn calls_intrinsic(m: &Module, op: &Op) -> bool {
    let Op::Call { callee: V::C(c), .. } = op else { return false };
    let ValueRef::Func(g) = c.get() else { return false };
    m.func(g).name().starts_with("llvm.")
}

/// Whether `op` calls a function whose attributes say `memory(none)`.
fn calls_memory_none(m: &Module, op: &Op) -> bool {
    let (Op::Call { callee: V::C(c), .. }
    | Op::Invoke { callee: V::C(c), .. }
    | Op::CallBr { callee: V::C(c), .. }) = op
    else {
        return false;
    };
    let ValueRef::Func(g) = c.get() else { return false };
    m.attrs_of(m.func(g).attrs.func).iter().any(|a| {
        matches!(a, Attr::Memory(e) if e.argmem == ModRef::None
            && e.inaccessiblemem == ModRef::None
            && e.other == ModRef::None)
    })
}

impl Op {
    /// LLVM `Instruction::mayReadOrWriteMemory`, calls aside from their
    /// callee's memory effects.
    fn may_access_memory(&self) -> bool {
        matches!(
            self,
            Op::Load { .. }
                | Op::Store { .. }
                | Op::Call { .. }
                | Op::AssumeBundle { .. }
                | Op::Invoke { .. }
                | Op::CallBr { .. }
                | Op::AtomicRmw { .. }
                | Op::CmpXchg { .. }
                | Op::Fence { .. }
        )
    }

    fn is_terminator(&self) -> bool {
        matches!(
            self,
            Op::Br { .. }
                | Op::CondBr { .. }
                | Op::Switch { .. }
                | Op::Ret { .. }
                | Op::Unreachable
                | Op::Invoke { .. }
                | Op::CallBr { .. }
                | Op::IndirectBr { .. }
                | Op::Resume { .. }
        )
    }

    fn successors(&self) -> Vec<Blk> {
        match self {
            Op::Br { dest } => vec![*dest],
            Op::Invoke { normal, unwind, .. } => vec![*normal, *unwind],
            Op::CallBr { normal, indirect, .. } => {
                std::iter::once(*normal).chain(indirect.iter().copied()).collect()
            }
            Op::CondBr { t, f, .. } => vec![*t, *f],
            Op::IndirectBr { dests, .. } => dests.clone(),
            Op::Switch { default, cases, .. } => {
                std::iter::once(*default).chain(cases.iter().map(|c| c.1)).collect()
            }
            _ => Vec::new(),
        }
    }

    fn successors_mut(&mut self) -> Vec<&mut Blk> {
        match self {
            Op::Br { dest } => vec![dest],
            Op::Invoke { normal, unwind, .. } => vec![normal, unwind],
            Op::CallBr { normal, indirect, .. } => {
                std::iter::once(normal).chain(indirect.iter_mut()).collect()
            }
            Op::CondBr { t, f, .. } => vec![t, f],
            Op::IndirectBr { dests, .. } => dests.iter_mut().collect(),
            Op::Switch { default, cases, .. } => {
                std::iter::once(default).chain(cases.iter_mut().map(|c| &mut c.1)).collect()
            }
            _ => Vec::new(),
        }
    }

    fn operands(&self) -> Vec<V> {
        match self {
            Op::Alloca { .. } | Op::Br { .. } | Op::Unreachable | Op::Fence { .. } => Vec::new(),
            Op::AtomicRmw { ptr, val, .. } => vec![*ptr, *val],
            Op::CmpXchg { ptr, cmp, new, .. } => vec![*ptr, *cmp, *new],
            Op::ExtractValue { agg, .. } => vec![*agg],
            Op::InsertValue { agg, val, .. } => vec![*agg, *val],
            Op::ExtractElement { vec, idx } => vec![*vec, *idx],
            Op::InsertElement { vec, elt, idx } => vec![*vec, *elt, *idx],
            Op::ShuffleVector { a, b, .. } => vec![*a, *b],
            Op::Load { ptr, .. } => vec![*ptr],
            Op::DynAlloca { count, .. } => vec![*count],
            Op::Store { val, ptr, .. } => vec![*val, *ptr],
            Op::Bin { l, r, .. } | Op::ICmp { l, r, .. } | Op::FCmp { l, r, .. } => vec![*l, *r],
            Op::FNeg { v } | Op::Freeze { v } | Op::Cast { v, .. } => vec![*v],
            Op::Gep { base, idx, .. } => {
                std::iter::once(*base).chain(idx.iter().copied()).collect()
            }
            Op::Phi { inc, .. } => inc.iter().map(|x| x.0).collect(),
            Op::Select { c, t, f } => vec![*c, *t, *f],
            Op::Call { callee, args, .. }
            | Op::Invoke { callee, args, .. }
            | Op::CallBr { callee, args, .. } => {
                std::iter::once(*callee).chain(args.iter().copied()).collect()
            }
            Op::AssumeBundle { args, .. } => args.clone(),
            Op::LandingPad { clauses, .. } => clauses.clone(),
            Op::Resume { v } | Op::IndirectBr { addr: v, .. } => vec![*v],
            Op::CondBr { c, .. } => vec![*c],
            Op::Switch { c, cases, .. } => {
                std::iter::once(*c).chain(cases.iter().map(|x| x.0)).collect()
            }
            Op::Ret { v } => v.iter().copied().collect(),
        }
    }

    /// The operands of the instruction, mutably (in [`Op::operands`] order).
    fn operands_mut(&mut self) -> Vec<&mut V> {
        match self {
            Op::Alloca { .. } | Op::Br { .. } | Op::Unreachable | Op::Fence { .. } => Vec::new(),
            Op::AtomicRmw { ptr, val, .. } => vec![ptr, val],
            Op::CmpXchg { ptr, cmp, new, .. } => vec![ptr, cmp, new],
            Op::ExtractValue { agg, .. } => vec![agg],
            Op::InsertValue { agg, val, .. } => vec![agg, val],
            Op::ExtractElement { vec, idx } => vec![vec, idx],
            Op::InsertElement { vec, elt, idx } => vec![vec, elt, idx],
            Op::ShuffleVector { a, b, .. } => vec![a, b],
            Op::Load { ptr, .. } => vec![ptr],
            Op::DynAlloca { count, .. } => vec![count],
            Op::Store { val, ptr, .. } => vec![val, ptr],
            Op::Bin { l, r, .. } | Op::ICmp { l, r, .. } | Op::FCmp { l, r, .. } => vec![l, r],
            Op::FNeg { v } | Op::Freeze { v } | Op::Cast { v, .. } => vec![v],
            Op::Gep { base, idx, .. } => std::iter::once(base).chain(idx.iter_mut()).collect(),
            Op::Phi { inc, .. } => inc.iter_mut().map(|x| &mut x.0).collect(),
            Op::Select { c, t, f } => vec![c, t, f],
            Op::Call { callee, args, .. }
            | Op::Invoke { callee, args, .. }
            | Op::CallBr { callee, args, .. } => {
                std::iter::once(callee).chain(args.iter_mut()).collect()
            }
            Op::AssumeBundle { args, .. } => args.iter_mut().collect(),
            Op::LandingPad { clauses, .. } => clauses.iter_mut().collect(),
            Op::Resume { v } | Op::IndirectBr { addr: v, .. } => vec![v],
            Op::CondBr { c, .. } => vec![c],
            Op::Switch { c, cases, .. } => {
                std::iter::once(c).chain(cases.iter_mut().map(|x| &mut x.0)).collect()
            }
            Op::Ret { v } => v.iter_mut().collect(),
        }
    }
}

#[derive(Clone, Debug)]
struct Inst {
    op: Op,
    ty: TypeId,
    block: Option<Blk>,
    md: Vec<(MdKindId, MetadataId)>,
    /// The fast-math flags in effect when it was appended (applied by
    /// [`Fir::lower`] to the ops LLVM's `FPMathOperator` covers).
    fmf: FastMath,
}

#[derive(Clone, Debug, Default)]
struct Block {
    insts: Vec<u32>,
    placed: bool,
    /// Terminator references to the block.
    uses: u32,
    /// The instructions that named the block as a successor or as a phi's
    /// incoming block, oldest first: a superset (an entry may since have
    /// been erased or redirected), filtered by [`Fir::refs`]. Lets the
    /// predecessor queries and [`Fir::replace_block_uses`] avoid scanning
    /// the whole function, which made a function with many cleanups
    /// quadratic.
    refs: Vec<u32>,
    /// The LLVM block made for it ahead of lowering, for a
    /// `blockaddress` ([`Fir::block_id`]).
    llvm: Option<BlockId>,
}

/// The function's indirect `goto` dispatch block (clang
/// `GetIndirectGotoBlock`), made at the first label address or computed
/// `goto`: the block, its `phi` and its `indirectbr`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct IndirectGoto {
    pub blk: Blk,
    pub phi: u32,
    pub br: u32,
}

/// A `#dbg_declare` record before an instruction (WP-814).
#[derive(Clone, Copy, Debug)]
pub(crate) struct DbgRec {
    pub addr: V,
    pub var: MetadataId,
    pub expr: MetadataId,
    pub loc: MetadataId,
}

/// The debug-location state of a function with a `DISubprogram`
/// (spec/codegen.md "Debug information"): clang's builder location and
/// `CGDebugInfo::LexicalBlockStack`. Absent at `-g0`.
#[derive(Clone, Debug)]
pub(crate) struct FirDbg {
    /// The `dbg` kind.
    pub kind: MdKindId,
    /// The subprogram.
    pub sp: MetadataId,
    /// The builder's current location (`None`: no `!dbg`).
    pub loc: Option<MetadataId>,
    /// The lexical scopes, innermost last; the subprogram first.
    pub scopes: Vec<MetadataId>,
    /// clang `LastStopPoint`'s location (the return's line when every
    /// return was simple).
    pub last_stop: kiln_source::Loc,
    /// clang `CGDebugInfo::CurLoc`: the last valid location emitted.
    pub cur_loc: kiln_source::Loc,
    /// clang `CurEHLocation`: where landing pads are.
    pub eh_loc: kiln_source::Loc,
    /// A variable was described (the subprogram gets `retainedNodes`).
    pub tracked: bool,
    /// clang `NumReturnExprs` / `NumSimpleReturnExprs`: when every
    /// `return` was simple, the epilogue takes the last stop point.
    pub returns: u32,
    pub simple_returns: u32,
    /// clang `ReturnValuePointer`: the slot holding the `sret` pointer
    /// (an NRVO variable is described through it).
    pub ret_ptr: Option<V>,
    /// The `ret` takes the location of the store it replaces (clang
    /// `EmitRetDbgLoc`).
    pub ret_dbg: bool,
    /// The function's variables (the subprogram's `retainedNodes` when
    /// optimizing).
    pub vars: Vec<MetadataId>,
    /// Records declared at the end of a block, adopted by the next
    /// instruction appended to it (`DIBuilder::insertDeclare(.., BB)`).
    trailing: Vec<(Blk, DbgRec)>,
    /// Records attached before an instruction, in order.
    recs: Vec<(u32, DbgRec)>,
}

impl FirDbg {
    /// The state of a function with subprogram `sp`.
    pub fn new(kind: MdKindId, sp: MetadataId) -> FirDbg {
        FirDbg {
            kind,
            sp,
            loc: None,
            scopes: vec![sp],
            last_stop: kiln_source::Loc::INVALID,
            cur_loc: kiln_source::Loc::INVALID,
            eh_loc: kiln_source::Loc::INVALID,
            tracked: false,
            returns: 0,
            simple_returns: 0,
            ret_ptr: None,
            ret_dbg: true,
            vars: Vec::new(),
            trailing: Vec::new(),
            recs: Vec::new(),
        }
    }

    /// The innermost scope.
    pub fn scope(&self) -> MetadataId {
        self.scopes.last().copied().unwrap_or(self.sp)
    }
}

/// The function buffer.
#[derive(Debug)]
pub(crate) struct Fir {
    insts: Vec<Inst>,
    blocks: Vec<Block>,
    layout: Vec<Blk>,
    allocas: Vec<u32>,
    params: Vec<TypeId>,
    cur: Option<Blk>,
    igoto: Option<IndirectGoto>,
    /// The `llvm.access.group` of the active parallel loops (clang
    /// `LoopInfoStack::InsertHelper`): attached to every instruction
    /// [`Fir::push`] appends that may access memory.
    access_group: Option<MetadataId>,
    /// The `llvm.access.group` kind, once a parallel loop was entered.
    access_kind: Option<MdKindId>,
    /// The fast-math flags of the FP scope being emitted (clang's
    /// `Builder.getFastMathFlags()`, WP-806): recorded in each
    /// instruction appended.
    fmf: FastMath,
    /// The fast-math flags of calls to functions other than intrinsics:
    /// the command line's (clang keeps a pragma scope's off them).
    call_fmf: FastMath,
    /// Debug locations and records (`None` at `-g0` and for functions
    /// without a subprogram).
    pub dbg: Option<Box<FirDbg>>,
    /// The `!nosanitize !{}` attachment of an open `UBSan` check scope
    /// (clang `SanitizerScope`, WP-675): appended to every instruction
    /// [`Fir::push`] adds.
    nosan: Option<(MdKindId, MetadataId)>,
}

impl Fir {
    /// A buffer for a function with parameters of types `params`; the
    /// entry block is created, placed and current.
    pub fn new(params: Vec<TypeId>) -> Fir {
        let mut f = Fir {
            insts: Vec::new(),
            blocks: Vec::new(),
            layout: Vec::new(),
            allocas: Vec::new(),
            params,
            cur: None,
            igoto: None,
            access_group: None,
            access_kind: None,
            fmf: FastMath::NONE,
            call_fmf: FastMath::NONE,
            dbg: None,
            nosan: None,
        };
        let e = f.create_block();
        f.place_at_end(e);
        f.cur = Some(e);
        f
    }

    // ---- blocks ---------------------------------------------------------

    /// The entry block.
    pub fn entry(&self) -> Blk {
        self.layout[0]
    }

    /// A new, unplaced block.
    pub fn create_block(&mut self) -> Blk {
        self.blocks.push(Block::default());
        Blk(self.blocks.len() as u32 - 1)
    }

    /// Whether `b` is in the layout.
    pub fn is_placed(&self, b: Blk) -> bool {
        self.blocks[b.0 as usize].placed
    }

    /// Number of branches to `b`.
    pub fn block_uses(&self, b: Blk) -> u32 {
        self.blocks[b.0 as usize].uses
    }

    /// Whether `b` has no instructions.
    pub fn block_empty(&self, b: Blk) -> bool {
        self.blocks[b.0 as usize].insts.is_empty()
    }

    /// Appends `b` to the layout.
    pub fn place_at_end(&mut self, b: Blk) {
        debug_assert!(!self.is_placed(b));
        self.blocks[b.0 as usize].placed = true;
        self.layout.push(b);
    }

    /// Inserts `b` into the layout after `after`.
    pub fn place_after(&mut self, after: Blk, b: Blk) {
        debug_assert!(!self.is_placed(b));
        self.blocks[b.0 as usize].placed = true;
        let i = self.layout.iter().position(|&x| x == after).map_or(self.layout.len(), |i| i + 1);
        self.layout.insert(i, b);
    }

    /// Removes `b` (and its instructions) from the function.
    pub fn erase_block(&mut self, b: Blk) {
        let insts = std::mem::take(&mut self.blocks[b.0 as usize].insts);
        for i in insts {
            self.drop_inst(i);
        }
        self.blocks[b.0 as usize].placed = false;
        self.layout.retain(|&x| x != b);
        if self.cur == Some(b) {
            self.cur = None;
        }
    }

    /// The current insertion block.
    pub fn cur(&self) -> Option<Blk> {
        self.cur
    }

    /// Sets (or clears) the insertion block.
    pub fn set_cur(&mut self, b: Option<Blk>) {
        self.cur = b;
    }

    /// Whether the block `b` ends in a terminator.
    pub fn is_terminated(&self, b: Blk) -> bool {
        self.blocks[b.0 as usize]
            .insts
            .last()
            .is_some_and(|&i| self.insts[i as usize].op.is_terminator())
    }

    /// The terminator of `b`.
    pub fn terminator(&self, b: Blk) -> Option<u32> {
        self.blocks[b.0 as usize]
            .insts
            .last()
            .copied()
            .filter(|&i| self.insts[i as usize].op.is_terminator())
    }

    /// The instructions of `b`.
    pub fn block_insts(&self, b: Blk) -> &[u32] {
        &self.blocks[b.0 as usize].insts
    }

    /// The single predecessor of `b`, counting edges as LLVM's
    /// `getSinglePredecessor` does (a `condbr` with both arms to `b` is two;
    /// the indirect `goto` block's `indirectbr` counts before it is placed).
    pub fn single_predecessor(&self, b: Blk) -> Option<Blk> {
        let mut pred = None;
        let unplaced = self.igoto.map(|g| g.blk).filter(|&g| !self.is_placed(g));
        for i in self.refs(b) {
            let Some(bb) = self.insts[i as usize].block else { continue };
            if !(self.is_placed(bb) || Some(bb) == unplaced) || self.terminator(bb) != Some(i) {
                continue;
            }
            for s in self.insts[i as usize].op.successors() {
                if s == b {
                    if pred.is_some() {
                        return None;
                    }
                    pred = Some(bb);
                }
            }
        }
        pred
    }

    /// Replaces every branch to `from` with a branch to `to`.
    pub fn replace_block_uses(&mut self, from: Blk, to: Blk) {
        for i in self.refs(from) {
            let inst = &mut self.insts[i as usize];
            if inst.block.is_none() {
                continue;
            }
            let mut changed = false;
            for s in inst.op.successors_mut() {
                if *s == from {
                    *s = to;
                    changed = true;
                    self.blocks[from.0 as usize].uses -= 1;
                    self.blocks[to.0 as usize].uses += 1;
                }
            }
            if let Op::Phi { inc, .. } = &mut inst.op {
                for x in inc {
                    if x.1 == from {
                        x.1 = to;
                        changed = true;
                    }
                }
            }
            if changed {
                self.note_ref(to, i);
            }
        }
    }

    /// The instructions that may name `b` (see `Block::refs`), ascending
    /// and without duplicates.
    fn refs(&self, b: Blk) -> Vec<u32> {
        let mut v = self.blocks[b.0 as usize].refs.clone();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Records that instruction `i` names block `b`.
    fn note_ref(&mut self, b: Blk, i: u32) {
        let r = &mut self.blocks[b.0 as usize].refs;
        if r.last() != Some(&i) {
            r.push(i);
        }
    }

    /// Records the blocks the new instruction `i` names.
    fn note_refs(&mut self, i: u32) {
        let op = &self.insts[i as usize].op;
        let mut bs = op.successors();
        if let Op::Phi { inc, .. } = op {
            bs.extend(inc.iter().map(|x| x.1));
        }
        for b in bs {
            self.note_ref(b, i);
        }
    }

    // ---- instructions -----------------------------------------------------

    /// The instruction `i`.
    pub fn op(&self, i: u32) -> &Op {
        &self.insts[i as usize].op
    }

    /// Marks call `i` `tail`.
    pub fn set_tail(&mut self, i: u32) {
        if let Op::Call { opts, .. } = &mut self.insts[i as usize].op {
            opts.tail = kiln_llvm::TailKind::Tail;
        }
    }

    /// Marks call `i` `musttail`.
    pub fn set_musttail(&mut self, i: u32) {
        if let Op::Call { opts, .. } = &mut self.insts[i as usize].op {
            opts.tail = kiln_llvm::TailKind::MustTail;
        }
    }

    /// LLVM `getOrEnforceKnownAlignment` on a pointer: raises the
    /// alignment of an `alloca` to at least `align` and returns the
    /// resulting alignment, or `None` if `p` is not an `alloca`.
    pub fn enforce_alloca_align(&mut self, p: V, align: u64) -> Option<u64> {
        let V::I(i) = p else { return None };
        match &mut self.insts[i as usize].op {
            Op::Alloca { align: a, .. } => {
                *a = (*a).max(align);
                Some(*a)
            }
            _ => None,
        }
    }

    /// Replaces argument `idx` of call `i` (clang `setArgOperand`).
    pub fn set_call_arg(&mut self, i: u32, idx: usize, v: V) {
        if let Op::Call { args, .. } = &mut self.insts[i as usize].op {
            args[idx] = v;
        }
    }

    /// The indirect `goto` dispatch block, made (unplaced) on first use:
    /// `phi ptr` (`indirect.goto.dest`) and `indirectbr` on it with no
    /// destinations yet.
    pub fn indirect_goto(&mut self) -> IndirectGoto {
        if let Some(g) = self.igoto {
            return g;
        }
        let blk = self.create_block();
        let V::I(phi) =
            self.push_in(blk, Op::Phi { ty: TypeId::PTR, inc: Vec::new() }, TypeId::PTR)
        else {
            unreachable!()
        };
        let V::I(br) =
            self.push_in(blk, Op::IndirectBr { addr: V::I(phi), dests: Vec::new() }, TypeId::VOID)
        else {
            unreachable!()
        };
        let g = IndirectGoto { blk, phi, br };
        self.igoto = Some(g);
        g
    }

    /// The indirect `goto` block, if one was made.
    pub fn indirect_goto_made(&self) -> Option<IndirectGoto> {
        self.igoto
    }

    /// Adds destination `d` to `indirectbr` `i`.
    pub fn add_indirect_dest(&mut self, i: u32, d: Blk) {
        if let Op::IndirectBr { dests, .. } = &mut self.insts[i as usize].op {
            dests.push(d);
            self.blocks[d.0 as usize].uses += 1;
            self.note_ref(d, i);
        }
    }

    /// The LLVM block `b` will lower to, made now in function `f` (for a
    /// `blockaddress`, which must name it before the body is lowered).
    pub fn block_id(&mut self, m: &mut Module, f: FuncId, b: Blk) -> BlockId {
        if let Some(id) = self.blocks[b.0 as usize].llvm {
            return id;
        }
        let id = m.func_mut(f).create_block("");
        self.blocks[b.0 as usize].llvm = Some(id);
        id
    }

    /// Adds a case to switch `i`.
    pub fn add_switch_case(&mut self, i: u32, v: V, d: Blk) {
        if let Op::Switch { cases, .. } = &mut self.insts[i as usize].op {
            cases.push((v, d));
            self.blocks[d.0 as usize].uses += 1;
            self.note_ref(d, i);
        }
    }

    /// The result type of an operand.
    pub fn ty(&self, m: &Module, v: V) -> TypeId {
        match v {
            V::I(i) => self.insts[i as usize].ty,
            V::Arg(n) => self.params[n as usize],
            V::C(c) => m.const_type(c),
            V::P(_) => TypeId::PTR,
        }
    }

    fn drop_inst(&mut self, i: u32) {
        let inst = &mut self.insts[i as usize];
        inst.block = None;
        let succ = inst.op.successors();
        for s in succ {
            self.blocks[s.0 as usize].uses -= 1;
        }
    }

    /// Erases instruction `i`.
    pub fn erase(&mut self, i: u32) {
        if let Some(b) = self.insts[i as usize].block {
            if self.dbg.as_ref().is_some_and(|d| d.recs.iter().any(|r| r.0 == i)) {
                self.move_records(b, i);
            }
            self.blocks[b.0 as usize].insts.retain(|&x| x != i);
            self.allocas.retain(|&x| x != i);
            self.drop_inst(i);
        }
    }

    /// Number of live uses of `v`.
    pub fn uses(&self, v: V) -> usize {
        self.insts
            .iter()
            .filter(|i| i.block.is_some())
            .map(|i| i.op.operands().iter().filter(|&&o| o == v).count())
            .sum()
    }

    /// The live instructions using `v`.
    pub fn users(&self, v: V) -> Vec<u32> {
        (0..self.insts.len() as u32)
            .filter(|&i| {
                let x = &self.insts[i as usize];
                x.block.is_some() && x.op.operands().contains(&v)
            })
            .collect()
    }

    /// The block holding instruction `i`.
    pub fn inst_block(&self, i: u32) -> Option<Blk> {
        self.insts[i as usize].block
    }

    /// Attaches metadata to instruction `i`.
    pub fn add_md(&mut self, i: u32, kind: MdKindId, md: MetadataId) {
        self.insts[i as usize].md.push((kind, md));
    }

    /// Opens (`Some`) or closes (`None`) a `UBSan` check scope: the
    /// instructions appended meanwhile get the `!nosanitize` attachment
    /// `md` (WP-675). Returns the previous scope.
    pub fn set_nosanitize(
        &mut self,
        md: Option<(MdKindId, MetadataId)>,
    ) -> Option<(MdKindId, MetadataId)> {
        std::mem::replace(&mut self.nosan, md)
    }

    /// Sets the `llvm.access.group` (kind `kind`) of the instructions
    /// appended from now on (WP-462, spec/codegen.md "Loop metadata").
    pub fn set_access_group(&mut self, kind: MdKindId, group: Option<MetadataId>) {
        self.access_kind = Some(kind);
        self.access_group = group;
    }

    /// Sets the fast-math flags of the instructions appended from now on
    /// (WP-806, spec/codegen.md "Floating-point semantics").
    pub fn set_fmf(&mut self, fmf: FastMath) {
        self.fmf = fmf;
    }

    /// Sets the fast-math flags of the function's base FP scope: those of
    /// its calls to non-intrinsic functions, and of what follows.
    pub fn set_base_fmf(&mut self, fmf: FastMath) {
        self.fmf = fmf;
        self.call_fmf = fmf;
    }

    /// The number of instructions appended so far (a mark for
    /// [`Fir::mark_scope_calls`]).
    pub fn inst_count(&self) -> usize {
        self.insts.len()
    }

    /// Gives the calls of `callee` appended since mark `from` the FP
    /// scope's fast-math flags instead of the command line's: a library
    /// builtin's call (clang `EmitBuiltinExpr` enters `CGFPOptionsRAII`).
    pub fn mark_scope_calls(&mut self, from: usize, callee: FuncId) {
        for inst in self.insts.iter_mut().skip(from) {
            if let Op::Call { callee: V::C(c), .. } = &inst.op
                && matches!(c.get(), ValueRef::Func(g) if g == callee)
            {
                inst.fmf = FastMath(inst.fmf.0 | SCOPE_CALL);
            }
        }
    }

    /// Appends `op` with result type `ty` to the current block. Under a
    /// parallel loop an instruction that may access memory (LLVM
    /// `mayReadOrWriteMemory`) gets the loop's access group; a call does
    /// unless its callee is `memory(none)` (checked by [`Fir::lower`]).
    pub fn push(&mut self, op: Op, ty: TypeId) -> V {
        let b = self.cur.unwrap_or_else(|| panic!("kiln_codegen: no insertion point for {op:?}"));
        let n = self.insts.len() as u32;
        for s in op.successors() {
            self.blocks[s.0 as usize].uses += 1;
        }
        let mut md = match (self.access_kind, self.access_group) {
            (Some(k), Some(g)) if op.may_access_memory() => vec![(k, g)],
            _ => Vec::new(),
        };
        if self.dbg.is_some() {
            self.dbg_append(b, n, &mut md);
        }
        md.extend(self.nosan);
        self.insts.push(Inst { op, ty, block: Some(b), md, fmf: self.fmf });
        self.blocks[b.0 as usize].insts.push(n);
        self.note_refs(n);
        V::I(n)
    }

    /// The builder location and the block's trailing debug records for
    /// instruction `n` appended to `b` (`IRBuilder::Insert`).
    fn dbg_append(&mut self, b: Blk, n: u32, md: &mut Vec<(MdKindId, MetadataId)>) {
        let Some(d) = self.dbg.as_deref_mut() else { return };
        if let Some(l) = d.loc {
            md.push((d.kind, l));
        }
        if d.trailing.iter().any(|t| t.0 == b) {
            let (mine, rest): (Vec<_>, Vec<_>) = d.trailing.drain(..).partition(|t| t.0 == b);
            d.trailing = rest;
            d.recs.extend(mine.into_iter().map(|(_, r)| (n, r)));
        }
    }

    /// Moves the records before instruction `i` (being erased) of block
    /// `b` to the instruction after it, or to the block's end (LLVM
    /// `Instruction::eraseFromParent` keeps a marker's records).
    fn move_records(&mut self, b: Blk, i: u32) {
        let insts = &self.blocks[b.0 as usize].insts;
        let next = insts.iter().position(|&x| x == i).and_then(|p| insts.get(p + 1)).copied();
        let Some(d) = self.dbg.as_deref_mut() else { return };
        let moved: Vec<DbgRec> = d.recs.iter().filter(|r| r.0 == i).map(|r| r.1).collect();
        d.recs.retain(|r| r.0 != i);
        match next {
            Some(n) => {
                let at = d.recs.iter().position(|r| r.0 == n).unwrap_or(d.recs.len());
                for (k, r) in moved.into_iter().enumerate() {
                    d.recs.insert(at + k, (n, r));
                }
            }
            None => d.trailing.extend(moved.into_iter().map(|r| (b, r))),
        }
    }

    /// The `!dbg` of instruction `i`.
    pub fn inst_dbg(&self, i: u32) -> Option<MetadataId> {
        let k = self.dbg.as_deref()?.kind;
        self.insts[i as usize].md.iter().find(|m| m.0 == k).map(|m| m.1)
    }

    /// Replaces the `!dbg` of instruction `i`.
    pub fn set_inst_dbg(&mut self, i: u32, loc: Option<MetadataId>) {
        let Some(k) = self.dbg.as_deref().map(|d| d.kind) else { return };
        let md = &mut self.insts[i as usize].md;
        md.retain(|m| m.0 != k);
        if let Some(l) = loc {
            md.push((k, l));
        }
    }

    /// Sets the builder's debug location, returning the previous one.
    pub fn set_dbg_loc(&mut self, loc: Option<MetadataId>) -> Option<MetadataId> {
        match self.dbg.as_deref_mut() {
            Some(d) => std::mem::replace(&mut d.loc, loc),
            None => None,
        }
    }

    /// A `#dbg_declare` at the end of the current block.
    pub fn declare(&mut self, rec: DbgRec) {
        let b = self.cur;
        if let (Some(d), Some(b)) = (self.dbg.as_deref_mut(), b) {
            d.trailing.push((b, rec));
        }
    }

    /// Appends `op` to block `b` (which need not be current or placed).
    pub fn push_in(&mut self, b: Blk, op: Op, ty: TypeId) -> V {
        let n = self.insts.len() as u32;
        for s in op.successors() {
            self.blocks[s.0 as usize].uses += 1;
        }
        let mut md = Vec::new();
        if self.dbg.is_some() {
            self.dbg_append(b, n, &mut md);
        }
        self.insts.push(Inst { op, ty, block: Some(b), md, fmf: self.fmf });
        self.blocks[b.0 as usize].insts.push(n);
        self.note_refs(n);
        V::I(n)
    }

    /// Inserts `op` before the last instruction of `b` (its terminator;
    /// LLVM `new StoreInst(.., block->back())`), or appends it to an
    /// empty block.
    pub fn insert_before_last(&mut self, b: Blk, op: Op, ty: TypeId) -> V {
        let n = self.insts.len() as u32;
        for s in op.successors() {
            self.blocks[s.0 as usize].uses += 1;
        }
        self.insts.push(Inst { op, ty, block: Some(b), md: Vec::new(), fmf: self.fmf });
        let insts = &mut self.blocks[b.0 as usize].insts;
        let at = insts.len().saturating_sub(1);
        insts.insert(at, n);
        self.note_refs(n);
        V::I(n)
    }

    /// [`Fir::insert_before_last`] through the function's builder (clang
    /// `Builder.restoreIP(Block, Block->back())` then `Builder.CreateX`):
    /// `LoopInfoStack::InsertHelper` gives it the active access group,
    /// as [`Fir::push`] does.
    pub fn builder_insert_before_last(&mut self, b: Blk, op: Op, ty: TypeId) -> V {
        let grouped = op.may_access_memory();
        let v = self.insert_before_last(b, op, ty);
        if let (true, Some(k), Some(g), V::I(n)) = (grouped, self.access_kind, self.access_group, v)
        {
            self.insts[n as usize].md.push((k, g));
        }
        if let (Some(d), V::I(n)) = (self.dbg.as_deref(), v)
            && let Some(l) = d.loc
        {
            self.insts[n as usize].md.push((d.kind, l));
        }
        v
    }

    /// Inserts `op` where LLVM's `CGBuilderTy(Inst->getParent(),
    /// std::next(Inst))` would after the instruction `at`: right after it,
    /// or for an `invoke` at the first insertion point of its normal
    /// destination (after its phis and landing pad).
    pub fn insert_after(&mut self, at: u32, op: Op, ty: TypeId) -> V {
        let (b, pos) = if let Op::Invoke { normal, .. } = self.insts[at as usize].op {
            let insts = &self.blocks[normal.0 as usize].insts;
            let pos = insts
                .iter()
                .position(|&x| {
                    !matches!(self.insts[x as usize].op, Op::Phi { .. } | Op::LandingPad { .. })
                })
                .unwrap_or(insts.len());
            (normal, pos)
        } else {
            let b = self.insts[at as usize].block.expect("insertion after a live instruction");
            let insts = &self.blocks[b.0 as usize].insts;
            (b, insts.iter().position(|&x| x == at).expect("instruction in its block") + 1)
        };
        let n = self.insts.len() as u32;
        for s in op.successors() {
            self.blocks[s.0 as usize].uses += 1;
        }
        self.insts.push(Inst { op, ty, block: Some(b), md: Vec::new(), fmf: self.fmf });
        self.blocks[b.0 as usize].insts.insert(pos, n);
        self.note_refs(n);
        V::I(n)
    }

    /// Inserts `op` immediately before instruction `at` (LLVM
    /// `new X(.., InsertBefore)`).
    pub fn insert_before(&mut self, at: u32, op: Op, ty: TypeId) -> V {
        let b = self.insts[at as usize].block.expect("insertion before a live instruction");
        let n = self.insts.len() as u32;
        for s in op.successors() {
            self.blocks[s.0 as usize].uses += 1;
        }
        self.insts.push(Inst { op, ty, block: Some(b), md: Vec::new(), fmf: self.fmf });
        let insts = &mut self.blocks[b.0 as usize].insts;
        let pos = insts.iter().position(|&x| x == at).expect("instruction in its block");
        insts.insert(pos, n);
        self.note_refs(n);
        V::I(n)
    }

    /// A copy of the whole buffer for another function (LLVM
    /// `CloneFunction`), or `None` when a block was already made in the
    /// original function for a `blockaddress` ([`Fir::block_id`]).
    pub fn clone_body(&self) -> Option<Fir> {
        if self.blocks.iter().any(|b| b.llvm.is_some()) {
            return None;
        }
        // The copy has no subprogram (yet): drop the locations, which
        // would point into the original's.
        let mut insts = self.insts.clone();
        if let Some(d) = self.dbg.as_deref() {
            for i in &mut insts {
                i.md.retain(|m| m.0 != d.kind);
            }
        }
        Some(Fir {
            insts,
            blocks: self.blocks.clone(),
            layout: self.layout.clone(),
            allocas: self.allocas.clone(),
            params: self.params.clone(),
            cur: None,
            igoto: self.igoto,
            access_group: None,
            access_kind: self.access_kind,
            fmf: self.fmf,
            call_fmf: self.call_fmf,
            dbg: None,
            nosan: None,
        })
    }

    /// The placed blocks in layout order.
    pub fn layout(&self) -> &[Blk] {
        &self.layout
    }

    /// The first `store` of `v` in block `b`.
    pub fn first_store_of(&self, b: Blk, v: V) -> Option<u32> {
        self.blocks[b.0 as usize]
            .insts
            .iter()
            .copied()
            .find(|&i| matches!(self.insts[i as usize].op, Op::Store { val, .. } if val == v))
    }

    /// Replaces the stored value of `store` `i` (LLVM `setOperand(0)`).
    pub fn set_store_val(&mut self, i: u32, v: V) {
        if let Op::Store { val, .. } = &mut self.insts[i as usize].op {
            *val = v;
        }
    }

    /// Moves instruction `i` immediately before instruction `at` (LLVM
    /// `moveBefore`).
    pub fn move_before(&mut self, i: u32, at: u32) {
        if let Some(b) = self.insts[i as usize].block {
            self.blocks[b.0 as usize].insts.retain(|&x| x != i);
        }
        let b = self.insts[at as usize].block.expect("move before a live instruction");
        let insts = &mut self.blocks[b.0 as usize].insts;
        let pos = insts.iter().position(|&x| x == at).expect("instruction in its block");
        insts.insert(pos, i);
        self.insts[i as usize].block = Some(b);
    }

    /// Redirects every edge of terminator `i` to `from` to `to`
    /// (LLVM `setSuccessor`).
    pub fn set_successor(&mut self, i: u32, from: Blk, to: Blk) {
        let live = self.insts[i as usize].block.is_some();
        let mut n = 0;
        for s in self.insts[i as usize].op.successors_mut() {
            if *s == from {
                *s = to;
                n += 1;
            }
        }
        if live {
            self.blocks[from.0 as usize].uses -= n;
            self.blocks[to.0 as usize].uses += n;
        }
        if n > 0 {
            self.note_ref(to, i);
        }
    }

    /// Moves every instruction of `from` to the end of `to`; `from` is
    /// left empty.
    pub fn splice_block(&mut self, from: Blk, to: Blk) {
        let insts = std::mem::take(&mut self.blocks[from.0 as usize].insts);
        for &i in &insts {
            self.insts[i as usize].block = Some(to);
        }
        self.blocks[to.0 as usize].insts.extend(insts);
        if self.cur == Some(from) {
            self.cur = Some(to);
        }
    }

    /// The live terminators branching to `b`.
    pub fn branches_to(&self, b: Blk) -> Vec<u32> {
        self.refs(b)
            .into_iter()
            .filter(|&i| {
                let x = &self.insts[i as usize];
                x.block.is_some() && x.op.is_terminator() && x.op.successors().contains(&b)
            })
            .collect()
    }

    /// Adds an incoming edge to phi `i`.
    pub fn add_incoming(&mut self, i: u32, v: V, from: Blk) {
        if let Op::Phi { inc, .. } = &mut self.insts[i as usize].op {
            inc.push((v, from));
            self.note_ref(from, i);
        }
    }

    /// The predecessors of `b` in LLVM use-list order: the most recently
    /// created branch first (one entry per edge).
    pub fn predecessors(&self, b: Blk) -> Vec<Blk> {
        let mut out = Vec::new();
        for i in self.refs(b).into_iter().rev() {
            let inst = &self.insts[i as usize];
            let Some(bb) = inst.block else { continue };
            if !inst.op.is_terminator() {
                continue;
            }
            for s in inst.op.successors().into_iter().rev() {
                if s == b {
                    out.push(bb);
                }
            }
        }
        out
    }

    /// Creates an alloca at the alloca insertion point of the entry block.
    pub fn alloca(&mut self, ty: TypeId, align: u64) -> V {
        let n = self.insts.len() as u32;
        let e = self.entry();
        self.insts.push(Inst {
            op: Op::Alloca { ty, align },
            ty: TypeId::PTR,
            block: Some(e),
            md: Vec::new(),
            fmf: FastMath::NONE,
        });
        let pos = self.allocas.len();
        self.blocks[e.0 as usize].insts.insert(pos, n);
        self.allocas.push(n);
        V::I(n)
    }

    /// The index the next instruction will get (a mark for
    /// [`Fir::allocas_from`]).
    pub fn mark(&self) -> u32 {
        self.insts.len() as u32
    }

    /// The live allocas created since `mark` (clang `AllocaTrackerRAII`).
    pub fn allocas_from(&self, mark: u32) -> Vec<V> {
        self.allocas.iter().filter(|&&i| i >= mark).map(|&i| V::I(i)).collect()
    }

    /// Erases alloca `a` and every instruction using it
    /// (`EHCleanupScope::RemoveAuxAllocas`).
    pub fn erase_with_users(&mut self, a: V) {
        for u in self.users(a) {
            self.erase(u);
        }
        self.erase(Self::last_inst(a));
    }

    /// Index of the last instruction pushed.
    pub fn last_inst(v: V) -> u32 {
        match v {
            V::I(i) => i,
            _ => unreachable!("not an instruction"),
        }
    }

    // ---- lowering -----------------------------------------------------------

    /// Lowers the buffer into the (empty) body of `f`; `ph` resolves the
    /// [`V::P`] placeholders.
    pub fn lower(&self, m: &mut Module, f: FuncId, ph: &[Option<Value>]) -> IrResult<()> {
        let phv = |n: u32| ph[n as usize].unwrap_or_else(|| panic!("unresolved placeholder {n}"));
        // A call to a `memory(none)` function accesses no memory, so it
        // gets no access group.
        let no_group: Vec<u32> = match self.access_kind {
            Some(k) => (0..self.insts.len() as u32)
                .filter(|&i| {
                    let inst = &self.insts[i as usize];
                    inst.md.iter().any(|&(x, _)| x == k) && calls_memory_none(m, &inst.op)
                })
                .collect(),
            None => Vec::new(),
        };
        let mut b = IrBuilder::new(m, f);
        let mut bmap: Vec<Option<BlockId>> = vec![None; self.blocks.len()];
        for &bb in &self.layout {
            bmap[bb.0 as usize] = Some(match self.blocks[bb.0 as usize].llvm {
                Some(id) => {
                    b.place_block(id);
                    id
                }
                None => b.append_block(""),
            });
        }
        let mut vmap: Vec<Option<Value>> = vec![None; self.insts.len()];
        let mut phis: Vec<(Value, u32)> = Vec::new();
        let blk =
            |x: Blk| bmap[x.0 as usize].unwrap_or_else(|| panic!("branch to unplaced block {x:?}"));
        for &bb in &self.layout {
            b.position_at_end(blk(bb));
            for &i in &self.blocks[bb.0 as usize].insts {
                let inst = &self.insts[i as usize];
                let val = |v: V| -> Value {
                    match v {
                        V::I(n) => vmap[n as usize]
                            .unwrap_or_else(|| panic!("use of %{n} before definition")),
                        V::Arg(n) => Value::arg(n),
                        V::C(c) => c,
                        V::P(n) => phv(n),
                    }
                };
                let r = match &inst.op {
                    Op::Alloca { ty, align } => b.alloca(*ty, *align)?,
                    Op::DynAlloca { ty, count, align } => {
                        b.alloca_ex(*ty, Some(val(*count)), *align, false)?
                    }
                    Op::Load { ty, ptr, mem } => b.load_ex(*ty, val(*ptr), *mem)?,
                    Op::Store { val: v, ptr, mem } => b.store_ex(val(*v), val(*ptr), *mem)?,
                    Op::Bin { op, flags, l, r } => {
                        let fmf = if op.is_float() { inst.fmf } else { FastMath::NONE };
                        b.binop_ex(*op, *flags, fmf, val(*l), val(*r))?
                    }
                    Op::FNeg { v } => b.fneg(val(*v), inst.fmf)?,
                    Op::Freeze { v } => b.freeze(val(*v))?,
                    Op::ICmp { p, l, r } => b.icmp(*p, val(*l), val(*r))?,
                    Op::FCmp { p, l, r } => b.fcmp_ex(*p, inst.fmf, val(*l), val(*r))?,
                    Op::Cast { op: op @ (CastOp::FPExt | CastOp::FPTrunc), v, to, .. }
                        if inst.fmf != FastMath::NONE =>
                    {
                        b.cast_fmf(*op, inst.fmf, val(*v), *to)?
                    }
                    Op::Cast { op, flags, v, to } => b.cast_ex(*op, *flags, val(*v), *to)?,
                    Op::Gep { src, base, idx, flags } => {
                        let idx: Vec<Value> = idx.iter().map(|&x| val(x)).collect();
                        b.gep(*src, val(*base), &idx, *flags)?
                    }
                    Op::Phi { ty, .. } => {
                        let fmf = fp_math(b.module_ref(), *ty, inst.fmf);
                        let p = b.phi_ex(*ty, fmf, &[])?;
                        phis.push((p, i));
                        p
                    }
                    Op::Select { c, t, f } => {
                        let fmf = fp_math(b.module_ref(), inst.ty, inst.fmf);
                        b.select_ex(fmf, val(*c), val(*t), val(*f))?
                    }
                    Op::Call { fn_ty, callee, args, opts } => {
                        let args: Vec<Value> = args.iter().map(|&x| val(x)).collect();
                        let fmf = if inst.fmf.0 & SCOPE_CALL != 0
                            || calls_intrinsic(b.module_ref(), &inst.op)
                        {
                            FastMath(inst.fmf.0 & !SCOPE_CALL)
                        } else {
                            self.call_fmf
                        };
                        let fmf = fp_math(b.module_ref(), inst.ty, fmf);
                        if fmf == FastMath::NONE || opts.fmf != FastMath::NONE {
                            b.call_ex(*fn_ty, val(*callee), &args, opts)?
                        } else {
                            let opts = CallOpts { fmf, ..(**opts).clone() };
                            b.call_ex(*fn_ty, val(*callee), &args, &opts)?
                        }
                    }
                    Op::AssumeBundle { fn_ty, callee, cond, tag, args } => {
                        let args: Vec<Value> = args.iter().map(|&x| val(x)).collect();
                        let opts = CallOpts {
                            bundles: vec![OperandBundle { tag: (*tag).into(), args }],
                            ..CallOpts::default()
                        };
                        b.call_ex(*fn_ty, *callee, &[*cond], &opts)?
                    }
                    Op::Invoke { fn_ty, callee, args, opts, normal, unwind } => {
                        let args: Vec<Value> = args.iter().map(|&x| val(x)).collect();
                        b.invoke_ex(*fn_ty, val(*callee), &args, blk(*normal), blk(*unwind), opts)?
                    }
                    Op::CallBr { fn_ty, callee, args, opts, normal, indirect } => {
                        let args: Vec<Value> = args.iter().map(|&x| val(x)).collect();
                        let ind: Vec<BlockId> = indirect.iter().map(|&x| blk(x)).collect();
                        b.callbr_ex(*fn_ty, val(*callee), &args, blk(*normal), &ind, opts)?
                    }
                    Op::LandingPad { cleanup, clauses } => {
                        let cl: Vec<Clause> =
                            clauses.iter().map(|&x| Clause::Catch(val(x))).collect();
                        b.landingpad(inst.ty, *cleanup, &cl)?
                    }
                    Op::Resume { v } => b.resume(val(*v))?,
                    Op::IndirectBr { addr, dests } => {
                        let ds: Vec<BlockId> = dests.iter().map(|&x| blk(x)).collect();
                        b.indirect_br(val(*addr), &ds)?
                    }
                    Op::Br { dest } => b.br(blk(*dest))?,
                    Op::CondBr { c, t, f } => b.cond_br(val(*c), blk(*t), blk(*f))?,
                    Op::Switch { c, default, cases } => {
                        let cases: Vec<(Value, BlockId)> =
                            cases.iter().map(|&(v, d)| (val(v), blk(d))).collect();
                        b.switch(val(*c), blk(*default), &cases)?
                    }
                    Op::Ret { v } => b.ret(v.map(val))?,
                    Op::Unreachable => b.unreachable()?,
                    Op::AtomicRmw { op, ptr, val: v, mem } => {
                        b.atomicrmw(*op, val(*ptr), val(*v), *mem)?
                    }
                    Op::CmpXchg { ptr, cmp, new, opts } => {
                        b.cmpxchg(val(*ptr), val(*cmp), val(*new), *opts)?
                    }
                    Op::Fence { ordering, scope } => b.fence(*ordering, *scope)?,
                    Op::ExtractValue { agg, idx } => b.extract_value(val(*agg), &[*idx])?,
                    Op::ExtractElement { vec, idx } => b.extract_element(val(*vec), val(*idx))?,
                    Op::InsertElement { vec, elt, idx } => {
                        b.insert_element(val(*vec), val(*elt), val(*idx))?
                    }
                    Op::ShuffleVector { a, b: y, mask } => {
                        b.shuffle_vector(val(*a), val(*y), mask)?
                    }
                    Op::InsertValue { agg, val: x, idx } => {
                        b.insert_value(val(*agg), val(*x), &[*idx])?
                    }
                };
                if let Some(id) = r.as_inst() {
                    for &(k, md) in &inst.md {
                        if Some(k) == self.access_kind && no_group.binary_search(&i).is_ok() {
                            continue;
                        }
                        b.func().add_inst_metadata(id, k, md);
                    }
                    // The verifier wants a location on every call in a
                    // function with a subprogram: line 0 in it.
                    if let Some(d) = self.dbg.as_deref()
                        && matches!(inst.op, Op::Call { .. } | Op::Invoke { .. })
                        && !inst.md.iter().any(|m| m.0 == d.kind)
                    {
                        let l = b.module().md_specialized(
                            "DILocation",
                            false,
                            vec![
                                ("line", kiln_llvm::MdField::UInt(0)),
                                ("scope", kiln_llvm::MdField::Node(Some(d.sp))),
                            ],
                        );
                        b.func().add_inst_metadata(id, d.kind, l);
                    }
                }
                vmap[i as usize] = Some(r);
            }
        }
        if let Some(d) = self.dbg.as_deref() {
            for (i, r) in &d.recs {
                let addr = match r.addr {
                    V::I(n) => vmap[n as usize],
                    V::Arg(n) => Some(Value::arg(n)),
                    V::C(c) => Some(c),
                    V::P(n) => ph[n as usize],
                };
                if let (Some(Some(id)), Some(addr)) = (vmap[*i as usize].map(Value::as_inst), addr)
                {
                    let rec = kiln_llvm::DbgRecord {
                        kind: kiln_llvm::DbgRecordKind::Declare,
                        addr,
                        var: r.var,
                        expr: r.expr,
                        loc: r.loc,
                    };
                    b.func().add_debug_record(id, rec);
                }
            }
        }
        for (p, i) in phis {
            if let Op::Phi { inc, .. } = &self.insts[i as usize].op {
                for &(v, bb) in inc {
                    let v = match v {
                        V::I(n) => {
                            vmap[n as usize].unwrap_or_else(|| panic!("phi operand %{n} undefined"))
                        }
                        V::Arg(n) => Value::arg(n),
                        V::C(c) => c,
                        V::P(n) => phv(n),
                    };
                    b.add_incoming(p, v, blk(bb))?;
                }
            }
        }
        Ok(())
    }
}
