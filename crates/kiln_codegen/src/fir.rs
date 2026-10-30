//! The function IR buffer (spec/codegen.md §4).
//!
//! clang emits into a mutable LLVM function and then edits it: it erases
//! blocks nobody branches to, replaces the return block by its single
//! predecessor, folds forwarding blocks and deletes the store into the
//! return-value slot. `kiln_llvm` is append-only, so a function body is
//! first built here — a small mutable mirror of the instructions codegen
//! emits — and lowered into the module once it is final ([`Fir::lower`]).

use kiln_llvm::{
    BinOp, BlockId, CallOpts, CastOp, FastMath, Flags, FloatPredicate, FuncId, GepFlags,
    IntPredicate, IrBuilder, IrResult, MdKindId, MemOpts, MetadataId, Module, TypeId, Value,
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
    Alloca { ty: TypeId, align: u64 },
    Load { ty: TypeId, ptr: V, mem: MemOpts },
    Store { val: V, ptr: V, mem: MemOpts },
    Bin { op: BinOp, flags: Flags, l: V, r: V },
    FNeg { v: V },
    ICmp { p: IntPredicate, l: V, r: V },
    FCmp { p: FloatPredicate, l: V, r: V },
    Cast { op: CastOp, flags: Flags, v: V, to: TypeId },
    Gep { src: TypeId, base: V, idx: Vec<V>, flags: GepFlags },
    Phi { ty: TypeId, inc: Vec<(V, Blk)> },
    Select { c: V, t: V, f: V },
    Call { fn_ty: TypeId, callee: V, args: Vec<V>, opts: Box<CallOpts> },
    Br { dest: Blk },
    CondBr { c: V, t: Blk, f: Blk },
    Switch { c: V, default: Blk, cases: Vec<(V, Blk)> },
    Ret { v: Option<V> },
    Unreachable,
    ExtractValue { agg: V, idx: u32 },
    InsertValue { agg: V, val: V, idx: u32 },
}

impl Op {
    fn is_terminator(&self) -> bool {
        matches!(
            self,
            Op::Br { .. }
                | Op::CondBr { .. }
                | Op::Switch { .. }
                | Op::Ret { .. }
                | Op::Unreachable
        )
    }

    fn successors(&self) -> Vec<Blk> {
        match self {
            Op::Br { dest } => vec![*dest],
            Op::CondBr { t, f, .. } => vec![*t, *f],
            Op::Switch { default, cases, .. } => {
                std::iter::once(*default).chain(cases.iter().map(|c| c.1)).collect()
            }
            _ => Vec::new(),
        }
    }

    fn successors_mut(&mut self) -> Vec<&mut Blk> {
        match self {
            Op::Br { dest } => vec![dest],
            Op::CondBr { t, f, .. } => vec![t, f],
            Op::Switch { default, cases, .. } => {
                std::iter::once(default).chain(cases.iter_mut().map(|c| &mut c.1)).collect()
            }
            _ => Vec::new(),
        }
    }

    fn operands(&self) -> Vec<V> {
        match self {
            Op::Alloca { .. } | Op::Br { .. } | Op::Unreachable => Vec::new(),
            Op::ExtractValue { agg, .. } => vec![*agg],
            Op::InsertValue { agg, val, .. } => vec![*agg, *val],
            Op::Load { ptr, .. } => vec![*ptr],
            Op::Store { val, ptr, .. } => vec![*val, *ptr],
            Op::Bin { l, r, .. } | Op::ICmp { l, r, .. } | Op::FCmp { l, r, .. } => vec![*l, *r],
            Op::FNeg { v } | Op::Cast { v, .. } => vec![*v],
            Op::Gep { base, idx, .. } => {
                std::iter::once(*base).chain(idx.iter().copied()).collect()
            }
            Op::Phi { inc, .. } => inc.iter().map(|x| x.0).collect(),
            Op::Select { c, t, f } => vec![*c, *t, *f],
            Op::Call { callee, args, .. } => {
                std::iter::once(*callee).chain(args.iter().copied()).collect()
            }
            Op::CondBr { c, .. } => vec![*c],
            Op::Switch { c, cases, .. } => {
                std::iter::once(*c).chain(cases.iter().map(|x| x.0)).collect()
            }
            Op::Ret { v } => v.iter().copied().collect(),
        }
    }
}

#[derive(Clone, Debug)]
struct Inst {
    op: Op,
    ty: TypeId,
    block: Option<Blk>,
    md: Vec<(MdKindId, MetadataId)>,
}

#[derive(Clone, Debug, Default)]
struct Block {
    insts: Vec<u32>,
    placed: bool,
    /// Terminator references to the block.
    uses: u32,
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
    /// `getSinglePredecessor` does (a `condbr` with both arms to `b` is two).
    pub fn single_predecessor(&self, b: Blk) -> Option<Blk> {
        let mut pred = None;
        for &bb in &self.layout {
            if let Some(t) = self.terminator(bb) {
                for s in self.insts[t as usize].op.successors() {
                    if s == b {
                        if pred.is_some() {
                            return None;
                        }
                        pred = Some(bb);
                    }
                }
            }
        }
        pred
    }

    /// Replaces every branch to `from` with a branch to `to`.
    pub fn replace_block_uses(&mut self, from: Blk, to: Blk) {
        for inst in &mut self.insts {
            if inst.block.is_none() {
                continue;
            }
            for s in inst.op.successors_mut() {
                if *s == from {
                    *s = to;
                    self.blocks[from.0 as usize].uses -= 1;
                    self.blocks[to.0 as usize].uses += 1;
                }
            }
            if let Op::Phi { inc, .. } = &mut inst.op {
                for x in inc {
                    if x.1 == from {
                        x.1 = to;
                    }
                }
            }
        }
    }

    // ---- instructions -----------------------------------------------------

    /// The instruction `i`.
    pub fn op(&self, i: u32) -> &Op {
        &self.insts[i as usize].op
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

    /// Adds a case to switch `i`.
    pub fn add_switch_case(&mut self, i: u32, v: V, d: Blk) {
        if let Op::Switch { cases, .. } = &mut self.insts[i as usize].op {
            cases.push((v, d));
            self.blocks[d.0 as usize].uses += 1;
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

    /// Appends `op` with result type `ty` to the current block.
    pub fn push(&mut self, op: Op, ty: TypeId) -> V {
        let b = self.cur.unwrap_or_else(|| panic!("kiln_codegen: no insertion point for {op:?}"));
        let n = self.insts.len() as u32;
        for s in op.successors() {
            self.blocks[s.0 as usize].uses += 1;
        }
        self.insts.push(Inst { op, ty, block: Some(b), md: Vec::new() });
        self.blocks[b.0 as usize].insts.push(n);
        V::I(n)
    }

    /// Appends `op` to block `b` (which need not be current or placed).
    pub fn push_in(&mut self, b: Blk, op: Op, ty: TypeId) -> V {
        let n = self.insts.len() as u32;
        for s in op.successors() {
            self.blocks[s.0 as usize].uses += 1;
        }
        self.insts.push(Inst { op, ty, block: Some(b), md: Vec::new() });
        self.blocks[b.0 as usize].insts.push(n);
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
        self.insts.push(Inst { op, ty, block: Some(b), md: Vec::new() });
        let insts = &mut self.blocks[b.0 as usize].insts;
        let at = insts.len().saturating_sub(1);
        insts.insert(at, n);
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
        self.insts.push(Inst { op, ty, block: Some(b), md: Vec::new() });
        let insts = &mut self.blocks[b.0 as usize].insts;
        let pos = insts.iter().position(|&x| x == at).expect("instruction in its block");
        insts.insert(pos, n);
        V::I(n)
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
        (0..self.insts.len() as u32)
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
        }
    }

    /// The predecessors of `b` in LLVM use-list order: the most recently
    /// created branch first (one entry per edge).
    pub fn predecessors(&self, b: Blk) -> Vec<Blk> {
        let mut out = Vec::new();
        for (i, inst) in self.insts.iter().enumerate().rev() {
            let Some(bb) = inst.block else { continue };
            if !inst.op.is_terminator() {
                continue;
            }
            let _ = i;
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
        });
        let pos = self.allocas.len();
        self.blocks[e.0 as usize].insts.insert(pos, n);
        self.allocas.push(n);
        V::I(n)
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
        let mut b = IrBuilder::new(m, f);
        let mut bmap: Vec<Option<BlockId>> = vec![None; self.blocks.len()];
        for &bb in &self.layout {
            bmap[bb.0 as usize] = Some(b.append_block(""));
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
                    Op::Load { ty, ptr, mem } => b.load_ex(*ty, val(*ptr), *mem)?,
                    Op::Store { val: v, ptr, mem } => b.store_ex(val(*v), val(*ptr), *mem)?,
                    Op::Bin { op, flags, l, r } => {
                        b.binop_ex(*op, *flags, FastMath::NONE, val(*l), val(*r))?
                    }
                    Op::FNeg { v } => b.fneg(val(*v), FastMath::NONE)?,
                    Op::ICmp { p, l, r } => b.icmp(*p, val(*l), val(*r))?,
                    Op::FCmp { p, l, r } => b.fcmp(*p, val(*l), val(*r))?,
                    Op::Cast { op, flags, v, to } => b.cast_ex(*op, *flags, val(*v), *to)?,
                    Op::Gep { src, base, idx, flags } => {
                        let idx: Vec<Value> = idx.iter().map(|&x| val(x)).collect();
                        b.gep(*src, val(*base), &idx, *flags)?
                    }
                    Op::Phi { ty, .. } => {
                        let p = b.phi(*ty, &[])?;
                        phis.push((p, i));
                        p
                    }
                    Op::Select { c, t, f } => b.select(val(*c), val(*t), val(*f))?,
                    Op::Call { fn_ty, callee, args, opts } => {
                        let args: Vec<Value> = args.iter().map(|&x| val(x)).collect();
                        b.call_ex(*fn_ty, val(*callee), &args, opts)?
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
                    Op::ExtractValue { agg, idx } => b.extract_value(val(*agg), &[*idx])?,
                    Op::InsertValue { agg, val: x, idx } => {
                        b.insert_value(val(*agg), val(*x), &[*idx])?
                    }
                };
                if let Some(id) = r.as_inst() {
                    for &(k, md) in &inst.md {
                        b.func().add_inst_metadata(id, k, md);
                    }
                }
                vmap[i as usize] = Some(r);
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
