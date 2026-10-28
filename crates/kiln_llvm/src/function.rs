//! Functions and basic blocks (`spec/llvm.md` §2.4, §4.1).

use std::collections::HashSet;

use crate::attr::AttrList;
use crate::inst::{CallConv, CallData, Inst, LandingPadData, Opcode};
use crate::module::{DllStorage, Linkage, UnnamedAddr, Visibility};
use crate::types::TypeId;
use crate::value::{BlockId, ComdatId, InstId, MdKindId, MetadataId, Value};

/// Sentinel for "no name" in [`Function::inst_names`].
pub(crate) const NO_NAME: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub(crate) struct BlockData {
    pub(crate) name: Box<str>,
    pub(crate) insts: Vec<InstId>,
    pub(crate) placed: bool,
}

/// A function: declaration (no blocks) or definition.
///
/// Header properties are public fields; the body is built through
/// [`crate::IrBuilder`].
#[derive(Clone, Debug)]
pub struct Function {
    pub(crate) name: Box<str>,
    pub(crate) ty: TypeId,
    pub(crate) ret: TypeId,
    pub(crate) params: Box<[TypeId]>,
    pub(crate) varargs: bool,
    /// Linkage (`external` by default).
    pub linkage: Linkage,
    /// Visibility.
    pub visibility: Visibility,
    /// DLL storage class.
    pub dll_storage: DllStorage,
    /// `dso_local` (not printed when implied by linkage/visibility).
    pub dso_local: bool,
    /// `unnamed_addr` / `local_unnamed_addr`.
    pub unnamed_addr: UnnamedAddr,
    /// Calling convention.
    pub cc: CallConv,
    /// Function, return and parameter attributes.
    pub attrs: AttrList,
    /// `section "..."`.
    pub section: Option<Box<str>>,
    /// `align N` (bytes).
    pub align: Option<u64>,
    /// `comdat` / `comdat($c)`.
    pub comdat: Option<ComdatId>,
    /// `gc "name"`.
    pub gc: Option<Box<str>>,
    /// `personality ptr @f`.
    pub personality: Option<Value>,
    /// Program address space (`addrspace(N)`; 0 is not printed).
    pub addrspace: u32,
    pub(crate) md: Vec<(MdKindId, MetadataId)>,
    pub(crate) arg_names: Vec<Box<str>>,
    pub(crate) insts: Vec<Inst>,
    pub(crate) inst_names: Vec<u32>,
    pub(crate) inst_block: Vec<BlockId>,
    pub(crate) names: Vec<Box<str>>,
    pub(crate) blocks: Vec<BlockData>,
    pub(crate) layout: Vec<BlockId>,
    pub(crate) ops: Vec<Value>,
    pub(crate) idx: Vec<u32>,
    pub(crate) calls: Vec<CallData>,
    pub(crate) pads: Vec<LandingPadData>,
    pub(crate) inst_md: Vec<(InstId, MdKindId, MetadataId)>,
    symtab: HashSet<Box<str>>,
    last_unique: u32,
}

impl Function {
    pub(crate) fn new(
        name: Box<str>,
        ty: TypeId,
        ret: TypeId,
        params: &[TypeId],
        varargs: bool,
    ) -> Function {
        Function {
            name,
            ty,
            ret,
            params: params.into(),
            varargs,
            linkage: Linkage::External,
            visibility: Visibility::Default,
            dll_storage: DllStorage::Default,
            dso_local: false,
            unnamed_addr: UnnamedAddr::None,
            cc: CallConv::C,
            attrs: AttrList::default(),
            section: None,
            align: None,
            comdat: None::<ComdatId>,
            gc: None,
            personality: None,
            addrspace: 0,
            md: Vec::new(),
            arg_names: vec![Box::from(""); params.len()],
            insts: Vec::new(),
            inst_names: Vec::new(),
            inst_block: Vec::new(),
            names: Vec::new(),
            blocks: Vec::new(),
            layout: Vec::new(),
            ops: Vec::new(),
            idx: Vec::new(),
            calls: Vec::new(),
            pads: Vec::new(),
            inst_md: Vec::new(),
            symtab: HashSet::new(),
            last_unique: 0,
        }
    }

    /// The (uniqued) name; empty for an unnamed function.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The function type.
    pub fn ty(&self) -> TypeId {
        self.ty
    }

    /// Return type.
    pub fn ret_type(&self) -> TypeId {
        self.ret
    }

    /// Parameter types.
    pub fn param_types(&self) -> &[TypeId] {
        &self.params
    }

    /// Trailing `...`.
    pub fn is_varargs(&self) -> bool {
        self.varargs
    }

    /// Argument `n` as a value.
    pub fn arg(&self, n: u32) -> Value {
        debug_assert!((n as usize) < self.params.len(), "argument index out of range");
        Value::arg(n)
    }

    /// Name argument `n` (uniqued within the function).
    pub fn set_arg_name(&mut self, n: u32, name: &str) {
        let old = std::mem::take(&mut self.arg_names[n as usize]);
        self.release_name(&old);
        self.arg_names[n as usize] = self.unique_name(name);
    }

    /// The name of argument `n` (empty if unnamed).
    pub fn arg_name(&self, n: u32) -> &str {
        &self.arg_names[n as usize]
    }

    /// True if the function has no body.
    pub fn is_declaration(&self) -> bool {
        self.layout.is_empty()
    }

    /// Blocks in layout order.
    pub fn layout(&self) -> &[BlockId] {
        &self.layout
    }

    /// The entry block (first in layout).
    pub fn entry_block(&self) -> Option<BlockId> {
        self.layout.first().copied()
    }

    /// Instructions of a block, in order.
    pub fn block_insts(&self, bb: BlockId) -> &[InstId] {
        &self.blocks[bb.index()].insts
    }

    /// A block's name (empty if unnamed).
    pub fn block_name(&self, bb: BlockId) -> &str {
        &self.blocks[bb.index()].name
    }

    /// An instruction.
    pub fn inst(&self, id: InstId) -> &Inst {
        &self.insts[id.index()]
    }

    /// An instruction's name (empty if unnamed).
    pub fn inst_name(&self, id: InstId) -> &str {
        match self.inst_names[id.index()] {
            NO_NAME => "",
            n => &self.names[n as usize],
        }
    }

    /// The block's terminator, if its last instruction is one.
    pub fn terminator(&self, bb: BlockId) -> Option<InstId> {
        let last = *self.blocks[bb.index()].insts.last()?;
        self.insts[last.index()].is_terminator().then_some(last)
    }

    /// True if the block ends in a terminator.
    pub fn is_terminated(&self, bb: BlockId) -> bool {
        self.terminator(bb).is_some()
    }

    /// Number of blocks created (placed or not).
    pub fn num_blocks(&self) -> usize {
        self.blocks.len()
    }

    /// Number of instructions created.
    pub fn num_insts(&self) -> usize {
        self.insts.len()
    }

    /// Create a block that is not yet in the layout (see
    /// [`Function::append_block`]). An empty name leaves it unnamed.
    pub fn create_block(&mut self, name: &str) -> BlockId {
        let name = self.unique_name(name);
        let id = BlockId(self.blocks.len() as u32);
        self.blocks.push(BlockData { name, insts: Vec::new(), placed: false });
        id
    }

    /// Place a created block at the end of the layout. Placing a block
    /// twice is a no-op.
    pub fn append_block(&mut self, bb: BlockId) {
        let b = &mut self.blocks[bb.index()];
        if !b.placed {
            b.placed = true;
            self.layout.push(bb);
        }
    }

    /// Place a created block right after `after` in the layout.
    pub fn insert_block_after(&mut self, after: BlockId, bb: BlockId) {
        if self.blocks[bb.index()].placed {
            return;
        }
        self.blocks[bb.index()].placed = true;
        let pos = self.layout.iter().position(|&b| b == after).map_or(self.layout.len(), |p| p + 1);
        self.layout.insert(pos, bb);
    }

    /// Create and place a block at the end of the layout.
    pub fn add_block(&mut self, name: &str) -> BlockId {
        let bb = self.create_block(name);
        self.append_block(bb);
        bb
    }

    /// Remove a block from the layout (it stays allocated; uses of it must
    /// be gone, or the verifier reports them).
    pub fn remove_block(&mut self, bb: BlockId) {
        if let Some(p) = self.layout.iter().position(|&b| b == bb) {
            self.layout.remove(p);
            self.blocks[bb.index()].placed = false;
        }
    }

    /// Rename a value (instruction result, argument or block) — the name is
    /// uniqued as LLVM does (`x`, `x1`, `x2`, ...). An empty name makes it
    /// unnamed.
    pub fn set_value_name(&mut self, v: Value, name: &str) {
        match v.get() {
            crate::value::ValueRef::Inst(i) => {
                let old = self.inst_names[i.index()];
                if old != NO_NAME {
                    let s = std::mem::take(&mut self.names[old as usize]);
                    self.release_name(&s);
                }
                self.inst_names[i.index()] = if name.is_empty() {
                    NO_NAME
                } else {
                    let n = self.unique_name(name);
                    self.names.push(n);
                    (self.names.len() - 1) as u32
                };
            }
            crate::value::ValueRef::Arg(n) => self.set_arg_name(n, name),
            crate::value::ValueRef::Block(b) => {
                let old = std::mem::take(&mut self.blocks[b.index()].name);
                self.release_name(&old);
                self.blocks[b.index()].name = self.unique_name(name);
            }
            _ => {}
        }
    }

    /// Rename a block.
    pub fn set_block_name(&mut self, bb: BlockId, name: &str) {
        self.set_value_name(Value::block(bb), name);
    }

    /// Attach metadata to the function (`!dbg !N` after the header).
    pub fn add_metadata(&mut self, kind: MdKindId, md: MetadataId) {
        crate::module::set_attachment(&mut self.md, kind, md);
    }

    /// Attach metadata to an instruction (`, !kind !N`).
    pub fn add_inst_metadata(&mut self, inst: InstId, kind: MdKindId, md: MetadataId) {
        let key = (inst, kind);
        let pos = self.inst_md.partition_point(|e| (e.0, e.1) < key);
        if self.inst_md.get(pos).is_some_and(|e| (e.0, e.1) == key) {
            self.inst_md[pos].2 = md;
        } else {
            self.inst_md.insert(pos, (inst, kind, md));
        }
    }

    pub(crate) fn inst_metadata(&self, inst: InstId) -> &[(InstId, MdKindId, MetadataId)] {
        let lo = self.inst_md.partition_point(|e| e.0 < inst);
        let hi = self.inst_md.partition_point(|e| e.0 <= inst);
        &self.inst_md[lo..hi]
    }

    /// Operand-pool slice.
    pub(crate) fn ops(&self, r: crate::inst::OpRange) -> &[Value] {
        &self.ops[r.range()]
    }

    /// Index-pool slice.
    pub(crate) fn idx(&self, r: crate::inst::OpRange) -> &[u32] {
        &self.idx[r.range()]
    }

    /// The type of a function-local value (instruction result or argument).
    pub fn local_type(&self, v: Value) -> Option<TypeId> {
        match v.get() {
            crate::value::ValueRef::Inst(i) => self.insts.get(i.index()).map(|x| x.ty),
            crate::value::ValueRef::Arg(n) => self.params.get(n as usize).copied(),
            crate::value::ValueRef::Block(_) => Some(TypeId::LABEL),
            _ => None,
        }
    }

    /// Make a local name unique (LLVM's `ValueSymbolTable::makeUniqueName`
    /// for local values: append a per-function counter).
    pub(crate) fn unique_name(&mut self, name: &str) -> Box<str> {
        if name.is_empty() {
            return Box::from("");
        }
        let b: Box<str> = if self.symtab.contains(name) {
            loop {
                self.last_unique += 1;
                let cand = format!("{name}{}", self.last_unique);
                if !self.symtab.contains(cand.as_str()) {
                    break cand.into_boxed_str();
                }
            }
        } else {
            name.into()
        };
        self.symtab.insert(b.clone());
        b
    }

    fn release_name(&mut self, name: &str) {
        if !name.is_empty() {
            self.symtab.remove(name);
        }
    }

    /// Successor blocks of a terminator instruction (allocates; for the
    /// verifier and analyses).
    pub fn successors(&self, inst: InstId) -> Vec<BlockId> {
        use crate::inst::InstKind as K;
        let i = &self.insts[inst.index()];
        match i.kind {
            K::Br { dest } | K::CatchRet { dest, .. } => vec![dest],
            K::CondBr { t, f, .. } => vec![t, f],
            K::Switch { default, cases, .. } => {
                let mut v = vec![default];
                v.extend(self.ops(cases).chunks(2).map(|c| c[1].as_block()));
                v
            }
            K::IndirectBr { dests, .. } => self.ops(dests).iter().map(|b| b.as_block()).collect(),
            K::Invoke(c) => {
                let c = &self.calls[c as usize];
                c.normal.into_iter().chain(c.unwind).collect()
            }
            K::CatchSwitch { handlers, unwind, .. } => {
                let mut v: Vec<BlockId> = self.ops(handlers).iter().map(|b| b.as_block()).collect();
                v.extend(unwind.opt().map(Value::as_block));
                v
            }
            K::CleanupRet { unwind, .. } => unwind.opt().map(Value::as_block).into_iter().collect(),
            _ => Vec::new(),
        }
    }

    /// Opcode of an instruction.
    pub fn opcode(&self, inst: InstId) -> Opcode {
        self.insts[inst.index()].opcode()
    }
}
