//! `IrBuilder`: instruction construction with light type checking
//! (`spec/llvm.md` §4).

use crate::attr::AttrList;
use crate::error::{IrError, IrErrorKind, IrResult};
use crate::function::{Function, NO_NAME};
use crate::inst::{
    Align, BinOp, CallConv, CallData, CastOp, Clause, FastMath, Flags, FloatPredicate, GepFlags,
    Inst, InstKind, IntPredicate, LandingPadData, OpRange, OperandBundle, Ordering, RmwOp,
    SyncScope, TailKind,
};
use crate::module::Module;
use crate::types::{TypeData, TypeId, TypeTable};
use crate::value::{BlockId, FuncId, InstId, Value, ValueRef};

/// Shuffle mask element meaning `poison`.
pub const SHUFFLE_POISON: u32 = u32::MAX;

/// Options of a load or store (and the ordering/scope part of atomics).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct MemOpts {
    /// Alignment in bytes (0: none printed; required for atomics).
    pub align: u64,
    /// `volatile`.
    pub volatile: bool,
    /// Atomic ordering (`NotAtomic` for plain accesses).
    pub ordering: Ordering,
    /// Synchronization scope.
    pub scope: SyncScope,
}

impl MemOpts {
    /// A plain access with alignment `align`.
    pub fn aligned(align: u64) -> MemOpts {
        MemOpts { align, ..MemOpts::default() }
    }
}

/// Options of a `cmpxchg`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CmpXchgOpts {
    /// Alignment in bytes (required).
    pub align: u64,
    /// Success ordering.
    pub success: Ordering,
    /// Failure ordering.
    pub failure: Ordering,
    /// `weak`.
    pub weak: bool,
    /// `volatile`.
    pub volatile: bool,
    /// Synchronization scope.
    pub scope: SyncScope,
}

/// Call-site options for `call` / `invoke`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CallOpts {
    /// Call-site attributes (function attrs print as `#N`).
    pub attrs: AttrList,
    /// Calling convention.
    pub cc: CallConv,
    /// Tail-call marker (`call` only).
    pub tail: TailKind,
    /// Fast-math flags (floating-point results only).
    pub fmf: FastMath,
    /// Operand bundles.
    pub bundles: Vec<OperandBundle>,
}

fn err<T>(kind: IrErrorKind, msg: impl Into<String>) -> IrResult<T> {
    Err(IrError::new(kind, msg))
}

/// Builds instructions into one function of a module.
///
/// The builder is positioned at the end of a block ([`IrBuilder::position_at_end`]),
/// before an instruction ([`IrBuilder::position_before`]) or at the start of a
/// block after its phis ([`IrBuilder::position_at_start`]). Every
/// instruction method returns `IrResult<Value>`; instructions without a
/// result (`store`, `br`, a `void` call, ...) return a value of type `void`
/// that is rejected as an operand.
#[derive(Debug)]
pub struct IrBuilder<'m> {
    m: &'m mut Module,
    f: FuncId,
    block: Option<BlockId>,
    /// Insertion index within `block`; `None` = at the end.
    pos: Option<usize>,
    next_name: Option<String>,
}

impl<'m> IrBuilder<'m> {
    /// A builder for function `f` (not yet positioned).
    pub fn new(m: &'m mut Module, f: FuncId) -> IrBuilder<'m> {
        IrBuilder { m, f, block: None, pos: None, next_name: None }
    }

    /// The module.
    pub fn module(&mut self) -> &mut Module {
        self.m
    }

    /// The module (shared).
    pub fn module_ref(&self) -> &Module {
        self.m
    }

    /// The function being built.
    pub fn func(&mut self) -> &mut Function {
        &mut self.m.funcs[self.f.index()]
    }

    /// The function being built (shared).
    pub fn func_ref(&self) -> &Function {
        &self.m.funcs[self.f.index()]
    }

    /// The id of the function being built.
    pub fn func_id(&self) -> FuncId {
        self.f
    }

    /// Switch to building another function of the same module.
    pub fn set_function(&mut self, f: FuncId) {
        self.f = f;
        self.block = None;
        self.pos = None;
        self.next_name = None;
    }

    /// Create and place a block at the end of the layout.
    pub fn append_block(&mut self, name: &str) -> BlockId {
        self.func().add_block(name)
    }

    /// Create a block that is not yet placed (see [`Function::append_block`]).
    pub fn create_block(&mut self, name: &str) -> BlockId {
        self.func().create_block(name)
    }

    /// Place a created block at the end of the layout.
    pub fn place_block(&mut self, bb: BlockId) {
        self.func().append_block(bb);
    }

    /// Insert at the end of `bb`.
    pub fn position_at_end(&mut self, bb: BlockId) {
        self.block = Some(bb);
        self.pos = None;
    }

    /// Insert before `inst` (which must be in a block).
    pub fn position_before(&mut self, inst: InstId) -> IrResult<()> {
        let f = &self.m.funcs[self.f.index()];
        let Some(&bb) = f.inst_block.get(inst.index()) else {
            return err(IrErrorKind::InvalidOperand, "position_before: unknown instruction");
        };
        let Some(p) = f.blocks[bb.index()].insts.iter().position(|&i| i == inst) else {
            return err(
                IrErrorKind::InvalidOperand,
                "position_before: instruction not in its block",
            );
        };
        self.block = Some(bb);
        self.pos = Some(p);
        Ok(())
    }

    /// Insert at the start of `bb`, after any phis (and a landingpad).
    pub fn position_at_start(&mut self, bb: BlockId) {
        let f = &self.m.funcs[self.f.index()];
        let p = f.blocks[bb.index()]
            .insts
            .iter()
            .position(|&i| {
                !matches!(f.insts[i.index()].kind, InstKind::Phi { .. } | InstKind::LandingPad(_))
            })
            .unwrap_or(f.blocks[bb.index()].insts.len());
        self.block = Some(bb);
        self.pos = Some(p);
    }

    /// The block being inserted into.
    pub fn current_block(&self) -> Option<BlockId> {
        self.block
    }

    /// True if the current block ends in a terminator.
    pub fn is_terminated(&self) -> bool {
        self.block.is_some_and(|b| self.func_ref().is_terminated(b))
    }

    /// Name the next value-producing instruction (`%name`, uniqued). The
    /// hint is consumed by the next instruction even if it has no result.
    pub fn name_next(&mut self, name: &str) -> &mut Self {
        self.next_name = Some(name.to_owned());
        self
    }

    /// Rename a local value (instruction, argument, block).
    pub fn set_name(&mut self, v: Value, name: &str) {
        self.func().set_value_name(v, name);
    }

    /// The type of `v` in the current function.
    pub fn type_of(&self, v: Value) -> TypeId {
        self.m.value_type(self.f, v)
    }

    fn types(&self) -> &TypeTable {
        &self.m.types
    }

    /// Type of an operand; errors for void results, out-of-range locals,
    /// blocks and the absent value.
    fn operand_ty(&self, v: Value) -> IrResult<TypeId> {
        let f = &self.m.funcs[self.f.index()];
        let ty = match v.get() {
            ValueRef::Inst(i) if i.index() < f.insts.len() => f.insts[i.index()].ty,
            ValueRef::Arg(n) if (n as usize) < f.params.len() => f.params[n as usize],
            ValueRef::Inst(_) | ValueRef::Arg(_) | ValueRef::Block(_) => {
                return err(IrErrorKind::InvalidOperand, "operand is not a value of this function");
            }
            _ if v.is_none() => return err(IrErrorKind::InvalidOperand, "absent operand"),
            _ => self.m.const_type(v),
        };
        if ty == TypeId::VOID {
            return err(IrErrorKind::InvalidOperand, "operand has type void");
        }
        Ok(ty)
    }

    fn mismatch<T>(&self, what: &str, want: TypeId, got: TypeId) -> IrResult<T> {
        err(
            IrErrorKind::TypeMismatch,
            format!(
                "{what}: expected {}, found {}",
                self.types().display(want),
                self.types().display(got)
            ),
        )
    }

    fn expect_ty(&self, v: Value, want: TypeId, what: &str) -> IrResult<()> {
        let got = self.operand_ty(v)?;
        if got != want {
            return self.mismatch(what, want, got);
        }
        Ok(())
    }

    fn expect_ptr(&self, v: Value, what: &str) -> IrResult<TypeId> {
        let t = self.operand_ty(v)?;
        if !self.types().is_ptr(t) {
            return err(
                IrErrorKind::TypeMismatch,
                format!("{what}: expected a pointer, found {}", self.types().display(t)),
            );
        }
        Ok(t)
    }

    fn check_block(&self, bb: BlockId) -> IrResult<()> {
        if bb.index() >= self.func_ref().blocks.len() {
            return err(IrErrorKind::DetachedBlock, "unknown block");
        }
        Ok(())
    }

    fn push_ops(&mut self, vals: &[Value]) -> OpRange {
        let f = self.func();
        let start = f.ops.len() as u32;
        f.ops.extend_from_slice(vals);
        OpRange { start, len: vals.len() as u32 }
    }

    fn push_idx(&mut self, vals: &[u32]) -> OpRange {
        let f = self.func();
        let start = f.idx.len() as u32;
        f.idx.extend_from_slice(vals);
        OpRange { start, len: vals.len() as u32 }
    }

    fn insert(&mut self, kind: InstKind, ty: TypeId) -> IrResult<Value> {
        let name = self.next_name.take();
        let Some(bb) = self.block else {
            return err(IrErrorKind::DetachedBlock, "builder is not positioned in a block");
        };
        let inst = Inst { kind, ty };
        let is_term = inst.is_terminator();
        let f = &mut self.m.funcs[self.f.index()];
        if bb.index() >= f.blocks.len() {
            return err(IrErrorKind::DetachedBlock, "builder positioned in an unknown block");
        }
        let len = f.blocks[bb.index()].insts.len();
        let pos = self.pos.unwrap_or(len);
        if pos == len && len > 0 {
            let last = f.blocks[bb.index()].insts[len - 1];
            if f.insts[last.index()].is_terminator() {
                return err(IrErrorKind::TerminatorNotLast, "block already has a terminator");
            }
        }
        if is_term && pos != len {
            return err(
                IrErrorKind::TerminatorNotLast,
                "terminator inserted before other instructions",
            );
        }
        let id = InstId(f.insts.len() as u32);
        f.insts.push(inst);
        f.inst_names.push(NO_NAME);
        f.inst_block.push(bb);
        f.blocks[bb.index()].insts.insert(pos, id);
        if let Some(p) = &mut self.pos {
            *p += 1;
        }
        let v = Value::inst(id);
        if let Some(n) = name
            && ty != TypeId::VOID
        {
            f.set_value_name(v, &n);
        }
        Ok(v)
    }

    fn result_bool(&mut self, operand: TypeId) -> TypeId {
        match self.m.types.vector_info(operand) {
            Some((_, n, true)) => self.m.types.scalable_vector(TypeId::I1, n),
            Some((_, n, false)) => self.m.types.vector(TypeId::I1, n),
            None => TypeId::I1,
        }
    }

    fn fmf_ok(&self, ty: TypeId, fmf: FastMath) -> IrResult<()> {
        if fmf != FastMath::NONE && !is_fp_math_type(self.types(), ty) {
            return err(
                IrErrorKind::InvalidFlags,
                "fast-math flags on a non-floating-point operation",
            );
        }
        Ok(())
    }

    // ---- arithmetic -----------------------------------------------------

    /// A binary operator with flags (`nuw`/`nsw`/`exact`/`disjoint`) and
    /// fast-math flags.
    pub fn binop_ex(
        &mut self,
        op: BinOp,
        flags: Flags,
        fmf: FastMath,
        lhs: Value,
        rhs: Value,
    ) -> IrResult<Value> {
        let ty = self.operand_ty(lhs)?;
        self.expect_ty(rhs, ty, op.name())?;
        let ok = if op.is_float() {
            self.types().is_float_or_float_vector(ty)
        } else {
            self.types().is_int_or_int_vector(ty)
        };
        if !ok {
            return err(
                IrErrorKind::TypeMismatch,
                format!("{} on {}", op.name(), self.types().display(ty)),
            );
        }
        if !op.allowed_flags().contains(flags) {
            return err(IrErrorKind::InvalidFlags, format!("invalid flags on {}", op.name()));
        }
        if fmf != FastMath::NONE && !op.is_float() {
            return err(IrErrorKind::InvalidFlags, "fast-math flags on an integer operator");
        }
        self.insert(InstKind::Binary { op, flags, fmf, lhs, rhs }, ty)
    }

    /// A binary operator without flags.
    pub fn binop(&mut self, op: BinOp, lhs: Value, rhs: Value) -> IrResult<Value> {
        self.binop_ex(op, Flags::NONE, FastMath::NONE, lhs, rhs)
    }

    /// `add`.
    pub fn add(&mut self, lhs: Value, rhs: Value) -> IrResult<Value> {
        self.binop(BinOp::Add, lhs, rhs)
    }

    /// `add nsw`.
    pub fn add_nsw(&mut self, lhs: Value, rhs: Value) -> IrResult<Value> {
        self.binop_ex(BinOp::Add, Flags::NSW, FastMath::NONE, lhs, rhs)
    }

    /// `sub`.
    pub fn sub(&mut self, lhs: Value, rhs: Value) -> IrResult<Value> {
        self.binop(BinOp::Sub, lhs, rhs)
    }

    /// `sub nsw`.
    pub fn sub_nsw(&mut self, lhs: Value, rhs: Value) -> IrResult<Value> {
        self.binop_ex(BinOp::Sub, Flags::NSW, FastMath::NONE, lhs, rhs)
    }

    /// `mul`.
    pub fn mul(&mut self, lhs: Value, rhs: Value) -> IrResult<Value> {
        self.binop(BinOp::Mul, lhs, rhs)
    }

    /// `mul nsw`.
    pub fn mul_nsw(&mut self, lhs: Value, rhs: Value) -> IrResult<Value> {
        self.binop_ex(BinOp::Mul, Flags::NSW, FastMath::NONE, lhs, rhs)
    }

    /// `fneg`.
    pub fn fneg(&mut self, val: Value, fmf: FastMath) -> IrResult<Value> {
        let ty = self.operand_ty(val)?;
        if !self.types().is_float_or_float_vector(ty) {
            return err(IrErrorKind::TypeMismatch, "fneg on a non-floating-point value");
        }
        self.insert(InstKind::FNeg { fmf, val }, ty)
    }

    /// `icmp` on integers or pointers (or vectors of them).
    pub fn icmp(&mut self, pred: IntPredicate, lhs: Value, rhs: Value) -> IrResult<Value> {
        let ty = self.operand_ty(lhs)?;
        self.expect_ty(rhs, ty, "icmp")?;
        if !(self.types().is_int_or_int_vector(ty) || self.types().is_ptr_or_ptr_vector(ty)) {
            return err(IrErrorKind::TypeMismatch, "icmp on a non-integer, non-pointer value");
        }
        let rt = self.result_bool(ty);
        self.insert(InstKind::ICmp { pred, lhs, rhs }, rt)
    }

    /// `fcmp` with fast-math flags.
    pub fn fcmp_ex(
        &mut self,
        pred: FloatPredicate,
        fmf: FastMath,
        lhs: Value,
        rhs: Value,
    ) -> IrResult<Value> {
        let ty = self.operand_ty(lhs)?;
        self.expect_ty(rhs, ty, "fcmp")?;
        if !self.types().is_float_or_float_vector(ty) {
            return err(IrErrorKind::TypeMismatch, "fcmp on a non-floating-point value");
        }
        let rt = self.result_bool(ty);
        self.insert(InstKind::FCmp { pred, fmf, lhs, rhs }, rt)
    }

    /// `fcmp`.
    pub fn fcmp(&mut self, pred: FloatPredicate, lhs: Value, rhs: Value) -> IrResult<Value> {
        self.fcmp_ex(pred, FastMath::NONE, lhs, rhs)
    }

    /// A conversion with flags (`trunc nuw nsw`, `zext nneg`, `uitofp nneg`).
    pub fn cast_ex(&mut self, op: CastOp, flags: Flags, val: Value, to: TypeId) -> IrResult<Value> {
        let from = self.operand_ty(val)?;
        check_cast(self.types(), op, from, to)?;
        if !op.allowed_flags().contains(flags) {
            return err(IrErrorKind::InvalidFlags, format!("invalid flags on {}", op.name()));
        }
        self.insert(InstKind::Cast { op, flags, val }, to)
    }

    /// A conversion.
    pub fn cast(&mut self, op: CastOp, val: Value, to: TypeId) -> IrResult<Value> {
        self.cast_ex(op, Flags::NONE, val, to)
    }

    // ---- memory ---------------------------------------------------------

    /// `alloca ty[, count], align N` with optional `inalloca`. A `count` of
    /// constant `i32 1` is the same as none.
    pub fn alloca_ex(
        &mut self,
        ty: TypeId,
        count: Option<Value>,
        align: u64,
        inalloca: bool,
    ) -> IrResult<Value> {
        if !self.types().is_sized(ty) {
            return err(IrErrorKind::InvalidType, "alloca of an unsized type");
        }
        let mut count = count.unwrap_or(Value::NONE);
        if !count.is_none() {
            let ct = self.operand_ty(count)?;
            if !self.types().is_int(ct) {
                return err(IrErrorKind::TypeMismatch, "alloca count must be an integer");
            }
            if ct == TypeId::I32 && self.m.const_index(count) == Some(1) {
                count = Value::NONE;
            }
        }
        let align = to_align(align)?;
        self.insert(InstKind::Alloca { allocated: ty, count, align, inalloca }, TypeId::PTR)
    }

    /// `alloca ty, align N`.
    pub fn alloca(&mut self, ty: TypeId, align: u64) -> IrResult<Value> {
        self.alloca_ex(ty, None, align, false)
    }

    /// `load` with full options.
    pub fn load_ex(&mut self, ty: TypeId, ptr: Value, o: MemOpts) -> IrResult<Value> {
        self.expect_ptr(ptr, "load address")?;
        if !self.types().is_first_class(ty) || !self.types().is_sized(ty) {
            return err(IrErrorKind::InvalidType, "load of a non-first-class or unsized type");
        }
        if o.ordering != Ordering::NotAtomic {
            self.check_atomic_ty(ty, "atomic load")?;
            if o.align == 0 {
                return err(IrErrorKind::InvalidOrdering, "atomic load requires an alignment");
            }
            if matches!(o.ordering, Ordering::Release | Ordering::AcqRel) {
                return err(IrErrorKind::InvalidOrdering, "load cannot have release ordering");
            }
        }
        let align = to_align(o.align)?;
        self.insert(
            InstKind::Load {
                ptr,
                align,
                volatile: o.volatile,
                ordering: o.ordering,
                scope: o.scope,
            },
            ty,
        )
    }

    /// `load ty, ptr %p, align N`.
    pub fn load(&mut self, ty: TypeId, ptr: Value, align: u64) -> IrResult<Value> {
        self.load_ex(ty, ptr, MemOpts::aligned(align))
    }

    /// `store` with full options.
    pub fn store_ex(&mut self, val: Value, ptr: Value, o: MemOpts) -> IrResult<Value> {
        let ty = self.operand_ty(val)?;
        self.expect_ptr(ptr, "store address")?;
        if !self.types().is_sized(ty) {
            return err(IrErrorKind::InvalidType, "store of an unsized type");
        }
        if o.ordering != Ordering::NotAtomic {
            self.check_atomic_ty(ty, "atomic store")?;
            if o.align == 0 {
                return err(IrErrorKind::InvalidOrdering, "atomic store requires an alignment");
            }
            if matches!(o.ordering, Ordering::Acquire | Ordering::AcqRel) {
                return err(IrErrorKind::InvalidOrdering, "store cannot have acquire ordering");
            }
        }
        let align = to_align(o.align)?;
        self.insert(
            InstKind::Store {
                val,
                ptr,
                align,
                volatile: o.volatile,
                ordering: o.ordering,
                scope: o.scope,
            },
            TypeId::VOID,
        )
    }

    /// `store ty %v, ptr %p, align N`.
    pub fn store(&mut self, val: Value, ptr: Value, align: u64) -> IrResult<Value> {
        self.store_ex(val, ptr, MemOpts::aligned(align))
    }

    fn check_atomic_ty(&self, ty: TypeId, what: &str) -> IrResult<()> {
        let t = self.types();
        if !(t.is_int(ty) || t.is_float(ty) || t.is_ptr(ty)) {
            return err(IrErrorKind::InvalidType, format!("{what} of {}", t.display(ty)));
        }
        Ok(())
    }

    /// `fence [syncscope] ordering`.
    pub fn fence(&mut self, ordering: Ordering, scope: SyncScope) -> IrResult<Value> {
        if matches!(ordering, Ordering::NotAtomic | Ordering::Unordered | Ordering::Monotonic) {
            return err(
                IrErrorKind::InvalidOrdering,
                "fence needs acquire, release, acq_rel or seq_cst",
            );
        }
        self.insert(InstKind::Fence { ordering, scope }, TypeId::VOID)
    }

    /// `cmpxchg` (result `{ T, i1 }`).
    pub fn cmpxchg(
        &mut self,
        ptr: Value,
        cmp: Value,
        new: Value,
        o: CmpXchgOpts,
    ) -> IrResult<Value> {
        self.expect_ptr(ptr, "cmpxchg address")?;
        let ty = self.operand_ty(cmp)?;
        self.expect_ty(new, ty, "cmpxchg new value")?;
        if !(self.types().is_int(ty) || self.types().is_ptr(ty)) {
            return err(IrErrorKind::InvalidType, "cmpxchg operand must be an integer or pointer");
        }
        if o.success < Ordering::Monotonic
            || o.failure < Ordering::Monotonic
            || matches!(o.failure, Ordering::Release | Ordering::AcqRel)
        {
            return err(IrErrorKind::InvalidOrdering, "invalid cmpxchg orderings");
        }
        if o.align == 0 {
            return err(IrErrorKind::InvalidOrdering, "cmpxchg requires an alignment");
        }
        let align = to_align(o.align)?;
        let rt = self.m.types.struct_literal(&[ty, TypeId::I1], false);
        self.insert(
            InstKind::CmpXchg {
                ptr,
                cmp,
                new,
                align,
                success: o.success,
                failure: o.failure,
                weak: o.weak,
                volatile: o.volatile,
                scope: o.scope,
            },
            rt,
        )
    }

    /// `atomicrmw op ptr, val ordering, align N` (alignment required).
    pub fn atomicrmw(&mut self, op: RmwOp, ptr: Value, val: Value, o: MemOpts) -> IrResult<Value> {
        self.expect_ptr(ptr, "atomicrmw address")?;
        let ty = self.operand_ty(val)?;
        let t = self.types();
        let ok = if op.is_float() {
            t.is_float(ty)
        } else if op == RmwOp::Xchg {
            t.is_int(ty) || t.is_float(ty) || t.is_ptr(ty)
        } else {
            t.is_int(ty)
        };
        if !ok {
            return err(
                IrErrorKind::InvalidType,
                format!("atomicrmw {} on {}", op.name(), t.display(ty)),
            );
        }
        if o.ordering < Ordering::Monotonic {
            return err(IrErrorKind::InvalidOrdering, "atomicrmw must be at least monotonic");
        }
        if o.align == 0 {
            return err(IrErrorKind::InvalidOrdering, "atomicrmw requires an alignment");
        }
        let align = to_align(o.align)?;
        self.insert(
            InstKind::AtomicRmw {
                op,
                ptr,
                val,
                align,
                ordering: o.ordering,
                volatile: o.volatile,
                scope: o.scope,
            },
            ty,
        )
    }

    /// `getelementptr [flags] source, ptr base, indices...`.
    pub fn gep(
        &mut self,
        source: TypeId,
        base: Value,
        indices: &[Value],
        flags: GepFlags,
    ) -> IrResult<Value> {
        let base_ty = self.operand_ty(base)?;
        let mut idx_tys = Vec::with_capacity(indices.len());
        let mut idx_consts = Vec::with_capacity(indices.len());
        for &i in indices {
            idx_tys.push(self.operand_ty(i)?);
            idx_consts.push(self.m.const_index(i));
        }
        let ty = gep_result_type(&mut self.m.types, source, base_ty, &idx_tys, &idx_consts)?;
        let mut ops = Vec::with_capacity(indices.len() + 1);
        ops.push(base);
        ops.extend_from_slice(indices);
        let ops = self.push_ops(&ops);
        self.insert(InstKind::Gep { source, flags, ops }, ty)
    }

    /// `getelementptr inbounds source, ptr base, i32 0, i32 field` (struct
    /// field address, as clang emits).
    pub fn struct_gep(&mut self, source: TypeId, base: Value, field: u32) -> IrResult<Value> {
        let z = self.m.const_int(TypeId::I32, 0);
        let f = self.m.const_int(TypeId::I32, i128::from(field));
        self.gep(source, base, &[z, f], GepFlags::INBOUNDS)
    }

    // ---- aggregates and vectors -----------------------------------------

    /// `extractvalue agg, idx...`.
    pub fn extract_value(&mut self, agg: Value, indices: &[u32]) -> IrResult<Value> {
        let at = self.operand_ty(agg)?;
        let Some(ty) =
            (!indices.is_empty()).then(|| self.types().indexed_type(at, indices)).flatten()
        else {
            return err(IrErrorKind::BadIndex, "invalid extractvalue indices");
        };
        let idx = self.push_idx(indices);
        self.insert(InstKind::ExtractValue { agg, idx }, ty)
    }

    /// `insertvalue agg, val, idx...`.
    pub fn insert_value(&mut self, agg: Value, val: Value, indices: &[u32]) -> IrResult<Value> {
        let at = self.operand_ty(agg)?;
        let Some(et) =
            (!indices.is_empty()).then(|| self.types().indexed_type(at, indices)).flatten()
        else {
            return err(IrErrorKind::BadIndex, "invalid insertvalue indices");
        };
        self.expect_ty(val, et, "insertvalue element")?;
        let idx = self.push_idx(indices);
        self.insert(InstKind::InsertValue { agg, val, idx }, at)
    }

    /// `extractelement vec, idx`.
    pub fn extract_element(&mut self, vec: Value, idx: Value) -> IrResult<Value> {
        let vt = self.operand_ty(vec)?;
        let Some((et, _, _)) = self.types().vector_info(vt) else {
            return err(IrErrorKind::TypeMismatch, "extractelement on a non-vector");
        };
        let it = self.operand_ty(idx)?;
        if !self.types().is_int(it) {
            return err(IrErrorKind::TypeMismatch, "extractelement index must be an integer");
        }
        self.insert(InstKind::ExtractElement { vec, idx }, et)
    }

    /// `insertelement vec, elt, idx`.
    pub fn insert_element(&mut self, vec: Value, elt: Value, idx: Value) -> IrResult<Value> {
        let vt = self.operand_ty(vec)?;
        let Some((et, _, _)) = self.types().vector_info(vt) else {
            return err(IrErrorKind::TypeMismatch, "insertelement on a non-vector");
        };
        self.expect_ty(elt, et, "insertelement element")?;
        let it = self.operand_ty(idx)?;
        if !self.types().is_int(it) {
            return err(IrErrorKind::TypeMismatch, "insertelement index must be an integer");
        }
        self.insert(InstKind::InsertElement { vec, elt, idx }, vt)
    }

    /// `shufflevector a, b, mask` ([`SHUFFLE_POISON`] for poison lanes).
    pub fn shuffle_vector(&mut self, a: Value, b: Value, mask: &[u32]) -> IrResult<Value> {
        let vt = self.operand_ty(a)?;
        self.expect_ty(b, vt, "shufflevector second operand")?;
        let Some((et, n, scalable)) = self.types().vector_info(vt) else {
            return err(IrErrorKind::TypeMismatch, "shufflevector on a non-vector");
        };
        if mask.is_empty() || mask.iter().any(|&e| e != SHUFFLE_POISON && e >= 2 * n) {
            return err(IrErrorKind::BadIndex, "invalid shuffle mask");
        }
        if scalable && mask.iter().any(|&e| e != 0 && e != SHUFFLE_POISON) {
            return err(IrErrorKind::BadIndex, "scalable shuffle mask must be zero or poison");
        }
        let rt = if scalable {
            self.m.types.scalable_vector(et, mask.len() as u32)
        } else {
            self.m.types.vector(et, mask.len() as u32)
        };
        let mask = self.push_idx(mask);
        self.insert(InstKind::ShuffleVector { a, b, mask }, rt)
    }

    /// `phi ty [v, bb], ...` (more incoming edges with [`IrBuilder::add_incoming`]).
    pub fn phi(&mut self, ty: TypeId, incoming: &[(Value, BlockId)]) -> IrResult<Value> {
        self.phi_ex(ty, FastMath::NONE, incoming)
    }

    /// `phi` with fast-math flags.
    pub fn phi_ex(
        &mut self,
        ty: TypeId,
        fmf: FastMath,
        incoming: &[(Value, BlockId)],
    ) -> IrResult<Value> {
        if !self.types().is_first_class(ty) || ty == TypeId::LABEL || ty == TypeId::METADATA {
            return err(IrErrorKind::InvalidType, "invalid phi type");
        }
        self.fmf_ok(ty, fmf)?;
        let mut ops = Vec::with_capacity(incoming.len() * 2);
        for &(v, bb) in incoming {
            self.expect_ty(v, ty, "phi incoming value")?;
            self.check_block(bb)?;
            ops.push(v);
            ops.push(Value::block(bb));
        }
        let incoming = self.push_ops(&ops);
        self.insert(InstKind::Phi { fmf, incoming }, ty)
    }

    /// Add an incoming edge to a phi.
    pub fn add_incoming(&mut self, phi: Value, v: Value, bb: BlockId) -> IrResult<()> {
        let Some(id) = phi.as_inst().filter(|i| i.index() < self.func_ref().insts.len()) else {
            return err(IrErrorKind::InvalidOperand, "add_incoming on a non-instruction");
        };
        let inst = self.func_ref().insts[id.index()];
        let InstKind::Phi { fmf, incoming } = inst.kind else {
            return err(IrErrorKind::InvalidOperand, "add_incoming on a non-phi");
        };
        self.expect_ty(v, inst.ty, "phi incoming value")?;
        self.check_block(bb)?;
        let f = self.func();
        let new = if (incoming.start + incoming.len) as usize == f.ops.len() {
            f.ops.push(v);
            f.ops.push(Value::block(bb));
            OpRange { start: incoming.start, len: incoming.len + 2 }
        } else {
            let start = f.ops.len() as u32;
            f.ops.extend_from_within(incoming.range());
            f.ops.push(v);
            f.ops.push(Value::block(bb));
            OpRange { start, len: incoming.len + 2 }
        };
        f.insts[id.index()].kind = InstKind::Phi { fmf, incoming: new };
        Ok(())
    }

    /// `select cond, t, f`.
    pub fn select(&mut self, cond: Value, t: Value, f: Value) -> IrResult<Value> {
        self.select_ex(FastMath::NONE, cond, t, f)
    }

    /// `select` with fast-math flags.
    #[allow(clippy::many_single_char_names)]
    pub fn select_ex(&mut self, fmf: FastMath, cond: Value, t: Value, f: Value) -> IrResult<Value> {
        let ty = self.operand_ty(t)?;
        self.expect_ty(f, ty, "select false value")?;
        let ct = self.operand_ty(cond)?;
        let ok = ct == TypeId::I1
            || match (self.types().vector_info(ct), self.types().vector_info(ty)) {
                (Some((TypeId::I1, n, s)), Some((_, m, s2))) => n == m && s == s2,
                _ => false,
            };
        if !ok {
            return err(
                IrErrorKind::TypeMismatch,
                "select condition must be i1 or a matching i1 vector",
            );
        }
        self.fmf_ok(ty, fmf)?;
        self.insert(InstKind::Select { fmf, cond, t, f }, ty)
    }

    /// `freeze v`.
    pub fn freeze(&mut self, val: Value) -> IrResult<Value> {
        let ty = self.operand_ty(val)?;
        self.insert(InstKind::Freeze { val }, ty)
    }

    /// `va_arg ptr %list, ty`.
    pub fn va_arg(&mut self, list: Value, ty: TypeId) -> IrResult<Value> {
        self.expect_ptr(list, "va_arg list")?;
        self.insert(InstKind::VaArg { list }, ty)
    }

    // ---- calls ----------------------------------------------------------

    fn check_call(
        &self,
        fn_ty: TypeId,
        callee: Value,
        args: &[Value],
        o: &CallOpts,
    ) -> IrResult<TypeId> {
        let Some((ret, params, varargs)) = self.types().function_info(fn_ty) else {
            return err(IrErrorKind::BadCall, "call needs a function type");
        };
        let ct = self.operand_ty(callee)?;
        if !self.types().is_ptr(ct) {
            return err(IrErrorKind::BadCall, "callee must be a pointer");
        }
        if args.len() < params.len() || (!varargs && args.len() != params.len()) {
            return err(
                IrErrorKind::BadCall,
                format!("call with {} arguments to a function taking {}", args.len(), params.len()),
            );
        }
        for (i, (&a, &p)) in args.iter().zip(params).enumerate() {
            let at = self.operand_ty(a)?;
            if at != p {
                return self.mismatch(&format!("call argument {i}"), p, at);
            }
        }
        for &a in &args[params.len()..] {
            let at = self.operand_ty(a)?;
            if !self.types().is_first_class(at) {
                return err(IrErrorKind::BadCall, "variadic argument must be first-class");
            }
        }
        if !self.types().is_float_or_float_vector(ret)
            && o.fmf != FastMath::NONE
            && !is_fp_math_type(self.types(), ret)
        {
            return err(
                IrErrorKind::InvalidFlags,
                "fast-math flags on a call without a floating-point result",
            );
        }
        for b in &o.bundles {
            for &v in &b.args {
                self.operand_ty(v)?;
            }
        }
        Ok(ret)
    }

    fn push_call(
        &mut self,
        fn_ty: TypeId,
        callee: Value,
        args: &[Value],
        o: &CallOpts,
        normal: Option<BlockId>,
        unwind: Option<BlockId>,
    ) -> u32 {
        let args = self.push_ops(args);
        let f = self.func();
        f.calls.push(CallData {
            callee,
            fn_ty,
            args,
            attrs: o.attrs.clone(),
            cc: o.cc,
            tail: o.tail,
            fmf: o.fmf,
            normal,
            unwind,
            bundles: o.bundles.clone(),
        });
        (f.calls.len() - 1) as u32
    }

    /// `call` through `callee` (a function, a pointer value or inline asm)
    /// of type `fn_ty`.
    pub fn call_ex(
        &mut self,
        fn_ty: TypeId,
        callee: Value,
        args: &[Value],
        o: &CallOpts,
    ) -> IrResult<Value> {
        let ret = self.check_call(fn_ty, callee, args, o)?;
        if self.block.is_none() {
            return err(IrErrorKind::DetachedBlock, "builder is not positioned in a block");
        }
        let c = self.push_call(fn_ty, callee, args, o, None, None);
        self.insert(InstKind::Call(c), ret)
    }

    /// Plain `call` of a function in this module, with the callee's calling
    /// convention and no call-site attributes.
    pub fn call(&mut self, callee: FuncId, args: &[Value]) -> IrResult<Value> {
        let f = &self.m.funcs[callee.index()];
        let o = CallOpts { cc: f.cc, ..CallOpts::default() };
        let ty = f.ty;
        self.call_ex(ty, Value::func(callee), args, &o)
    }

    /// `invoke ... to label %normal unwind label %unwind`.
    pub fn invoke_ex(
        &mut self,
        fn_ty: TypeId,
        callee: Value,
        args: &[Value],
        normal: BlockId,
        unwind: BlockId,
        o: &CallOpts,
    ) -> IrResult<Value> {
        let ret = self.check_call(fn_ty, callee, args, o)?;
        self.check_block(normal)?;
        self.check_block(unwind)?;
        if o.tail != TailKind::None {
            return err(IrErrorKind::BadCall, "invoke cannot have a tail marker");
        }
        if self.block.is_none() {
            return err(IrErrorKind::DetachedBlock, "builder is not positioned in a block");
        }
        let c = self.push_call(fn_ty, callee, args, o, Some(normal), Some(unwind));
        self.insert(InstKind::Invoke(c), ret)
    }

    /// `invoke` of a function in this module.
    pub fn invoke(
        &mut self,
        callee: FuncId,
        args: &[Value],
        normal: BlockId,
        unwind: BlockId,
    ) -> IrResult<Value> {
        let f = &self.m.funcs[callee.index()];
        let o = CallOpts { cc: f.cc, ..CallOpts::default() };
        let ty = f.ty;
        self.invoke_ex(ty, Value::func(callee), args, normal, unwind, &o)
    }

    // ---- exception handling ---------------------------------------------

    /// `landingpad ty [cleanup] clauses...`.
    pub fn landingpad(&mut self, ty: TypeId, cleanup: bool, clauses: &[Clause]) -> IrResult<Value> {
        if !cleanup && clauses.is_empty() {
            return err(IrErrorKind::InvalidOperand, "landingpad needs cleanup or a clause");
        }
        for c in clauses {
            match *c {
                Clause::Catch(v) => {
                    self.expect_ptr(v, "catch clause")?;
                }
                Clause::Filter(v) => {
                    let t = self.operand_ty(v)?;
                    if self.types().array_info(t).is_none() {
                        return err(IrErrorKind::TypeMismatch, "filter clause must be an array");
                    }
                }
            }
            let v = match *c {
                Clause::Catch(v) | Clause::Filter(v) => v,
            };
            if !v.is_constant() {
                return err(IrErrorKind::InvalidOperand, "landingpad clause must be a constant");
            }
        }
        let f = self.func();
        f.pads.push(LandingPadData { cleanup, clauses: clauses.to_vec() });
        let p = (f.pads.len() - 1) as u32;
        self.insert(InstKind::LandingPad(p), ty)
    }

    /// `resume ty %v`.
    pub fn resume(&mut self, val: Value) -> IrResult<Value> {
        self.operand_ty(val)?;
        self.insert(InstKind::Resume { val }, TypeId::VOID)
    }

    fn expect_token(&self, v: Value, what: &str) -> IrResult<()> {
        self.expect_ty(v, TypeId::TOKEN, what)
    }

    /// `catchswitch within parent [handlers] unwind (label %bb | to caller)`;
    /// `parent` `None` is `none`.
    pub fn catchswitch(
        &mut self,
        parent: Option<Value>,
        handlers: &[BlockId],
        unwind: Option<BlockId>,
    ) -> IrResult<Value> {
        let parent = match parent {
            Some(p) => {
                self.expect_token(p, "catchswitch parent")?;
                p
            }
            None => Value::NONE,
        };
        if handlers.is_empty() {
            return err(IrErrorKind::InvalidOperand, "catchswitch needs a handler");
        }
        for &h in handlers {
            self.check_block(h)?;
        }
        if let Some(u) = unwind {
            self.check_block(u)?;
        }
        let hs: Vec<Value> = handlers.iter().map(|&b| Value::block(b)).collect();
        let handlers = self.push_ops(&hs);
        let unwind = unwind.map_or(Value::NONE, Value::block);
        self.insert(InstKind::CatchSwitch { parent, handlers, unwind }, TypeId::TOKEN)
    }

    /// `catchpad within %cs [args]`.
    pub fn catchpad(&mut self, catchswitch: Value, args: &[Value]) -> IrResult<Value> {
        self.expect_token(catchswitch, "catchpad parent")?;
        for &a in args {
            self.operand_ty(a)?;
        }
        let args = self.push_ops(args);
        self.insert(InstKind::CatchPad { parent: catchswitch, args }, TypeId::TOKEN)
    }

    /// `cleanuppad within (parent | none) [args]`.
    pub fn cleanuppad(&mut self, parent: Option<Value>, args: &[Value]) -> IrResult<Value> {
        let parent = match parent {
            Some(p) => {
                self.expect_token(p, "cleanuppad parent")?;
                p
            }
            None => Value::NONE,
        };
        for &a in args {
            self.operand_ty(a)?;
        }
        let args = self.push_ops(args);
        self.insert(InstKind::CleanupPad { parent, args }, TypeId::TOKEN)
    }

    /// `catchret from %pad to label %dest`.
    pub fn catchret(&mut self, pad: Value, dest: BlockId) -> IrResult<Value> {
        self.expect_token(pad, "catchret pad")?;
        self.check_block(dest)?;
        self.insert(InstKind::CatchRet { pad, dest }, TypeId::VOID)
    }

    /// `cleanupret from %pad unwind (label %bb | to caller)`.
    pub fn cleanupret(&mut self, pad: Value, unwind: Option<BlockId>) -> IrResult<Value> {
        self.expect_token(pad, "cleanupret pad")?;
        if let Some(u) = unwind {
            self.check_block(u)?;
        }
        self.insert(
            InstKind::CleanupRet { pad, unwind: unwind.map_or(Value::NONE, Value::block) },
            TypeId::VOID,
        )
    }

    // ---- terminators ----------------------------------------------------

    /// `br label %dest`.
    pub fn br(&mut self, dest: BlockId) -> IrResult<Value> {
        self.check_block(dest)?;
        self.insert(InstKind::Br { dest }, TypeId::VOID)
    }

    /// `br i1 %c, label %t, label %f`.
    pub fn cond_br(&mut self, cond: Value, t: BlockId, f: BlockId) -> IrResult<Value> {
        self.expect_ty(cond, TypeId::I1, "branch condition")?;
        self.check_block(t)?;
        self.check_block(f)?;
        self.insert(InstKind::CondBr { cond, t, f }, TypeId::VOID)
    }

    /// `switch ty %c, label %default [ ty val, label %bb ... ]`; case values
    /// must be distinct integer constants of the condition's type.
    pub fn switch(
        &mut self,
        cond: Value,
        default: BlockId,
        cases: &[(Value, BlockId)],
    ) -> IrResult<Value> {
        let ty = self.operand_ty(cond)?;
        if !self.types().is_int(ty) {
            return err(IrErrorKind::TypeMismatch, "switch condition must be an integer");
        }
        self.check_block(default)?;
        let mut ops = Vec::with_capacity(cases.len() * 2);
        for (i, &(v, bb)) in cases.iter().enumerate() {
            self.expect_ty(v, ty, "switch case")?;
            if !matches!(self.m.const_of(v), Some(crate::constant::ConstData::Int { .. })) {
                return err(IrErrorKind::InvalidOperand, "switch case must be an integer constant");
            }
            if cases[..i].iter().any(|&(w, _)| w == v) {
                return err(IrErrorKind::InvalidOperand, "duplicate switch case");
            }
            self.check_block(bb)?;
            ops.push(v);
            ops.push(Value::block(bb));
        }
        let cases = self.push_ops(&ops);
        self.insert(InstKind::Switch { cond, default, cases }, TypeId::VOID)
    }

    /// `indirectbr ptr %addr, [label %a, ...]`.
    pub fn indirect_br(&mut self, addr: Value, dests: &[BlockId]) -> IrResult<Value> {
        self.expect_ptr(addr, "indirectbr address")?;
        for &d in dests {
            self.check_block(d)?;
        }
        let ds: Vec<Value> = dests.iter().map(|&b| Value::block(b)).collect();
        let dests = self.push_ops(&ds);
        self.insert(InstKind::IndirectBr { addr, dests }, TypeId::VOID)
    }

    /// `ret ty %v` / `ret void` (`None`); checked against the return type.
    pub fn ret(&mut self, val: Option<Value>) -> IrResult<Value> {
        let want = self.func_ref().ret;
        match val {
            Some(v) => self.expect_ty(v, want, "return value")?,
            None if want != TypeId::VOID => {
                return err(IrErrorKind::TypeMismatch, "ret void in a function returning a value");
            }
            None => {}
        }
        self.insert(InstKind::Ret { val: val.unwrap_or(Value::NONE) }, TypeId::VOID)
    }

    /// `ret void`.
    pub fn ret_void(&mut self) -> IrResult<Value> {
        self.ret(None)
    }

    /// `unreachable`.
    pub fn unreachable(&mut self) -> IrResult<Value> {
        self.insert(InstKind::Unreachable, TypeId::VOID)
    }
}

fn to_align(bytes: u64) -> IrResult<Align> {
    Align::from_bytes(bytes).ok_or_else(|| {
        IrError::new(IrErrorKind::InvalidOperand, format!("invalid alignment {bytes}"))
    })
}

/// Types that may carry fast-math flags on phi/select/call: floating-point
/// scalars and vectors, and arrays of them (LLVM's `FPMathOperator`).
fn is_fp_math_type(t: &TypeTable, ty: TypeId) -> bool {
    let mut ty = ty;
    while let Some((e, _)) = t.array_info(ty) {
        ty = e;
    }
    t.is_float_or_float_vector(ty)
}

/// Bit width of a primitive (int, float, vector of those); `None` for
/// pointers and aggregates.
fn primitive_bits(t: &TypeTable, ty: TypeId) -> Option<u64> {
    match t.data(ty) {
        TypeData::Int(n) => Some(u64::from(*n)),
        TypeData::Vector { len, elem, scalable: false } => {
            Some(u64::from(*len) * primitive_bits(t, *elem)?)
        }
        _ => t.float_width(ty).map(u64::from),
    }
}

/// Validate a conversion (LLVM's `CastInst::castIsValid`).
pub(crate) fn check_cast(t: &TypeTable, op: CastOp, from: TypeId, to: TypeId) -> IrResult<()> {
    let bad = || {
        Err(IrError::new(
            IrErrorKind::TypeMismatch,
            format!("invalid {} from {} to {}", op.name(), t.display(from), t.display(to)),
        ))
    };
    let (fv, tv) = (t.vector_info(from), t.vector_info(to));
    let same_shape = match (fv, tv) {
        (None, None) => true,
        (Some((_, a, s)), Some((_, b, s2))) => a == b && s == s2,
        _ => false,
    };
    let (fs, ts) = (t.scalar(from), t.scalar(to));
    let int_w = |x| t.int_width(x);
    let fl_w = |x| t.float_width(x);
    let ok = match op {
        CastOp::Trunc => {
            same_shape && matches!((int_w(fs), int_w(ts)), (Some(a), Some(b)) if a > b)
        }
        CastOp::ZExt | CastOp::SExt => {
            same_shape && matches!((int_w(fs), int_w(ts)), (Some(a), Some(b)) if a < b)
        }
        CastOp::FPTrunc => {
            same_shape && matches!((fl_w(fs), fl_w(ts)), (Some(a), Some(b)) if a > b)
        }
        CastOp::FPExt => same_shape && matches!((fl_w(fs), fl_w(ts)), (Some(a), Some(b)) if a < b),
        CastOp::FPToUI | CastOp::FPToSI => same_shape && t.is_float(fs) && t.is_int(ts),
        CastOp::UIToFP | CastOp::SIToFP => same_shape && t.is_int(fs) && t.is_float(ts),
        CastOp::PtrToInt => same_shape && t.is_ptr(fs) && t.is_int(ts),
        CastOp::IntToPtr => same_shape && t.is_int(fs) && t.is_ptr(ts),
        CastOp::BitCast => {
            if t.is_ptr(fs) || t.is_ptr(ts) {
                same_shape
                    && t.ptr_addrspace(fs).is_some()
                    && t.ptr_addrspace(fs) == t.ptr_addrspace(ts)
            } else {
                match (primitive_bits(t, from), primitive_bits(t, to)) {
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                }
            }
        }
        CastOp::AddrSpaceCast => {
            same_shape
                && matches!((t.ptr_addrspace(fs), t.ptr_addrspace(ts)), (Some(a), Some(b)) if a != b)
        }
    };
    if ok { Ok(()) } else { bad() }
}

/// Result type of a GEP; validates the indices against `source`.
pub(crate) fn gep_result_type(
    types: &mut TypeTable,
    source: TypeId,
    base_ty: TypeId,
    idx_tys: &[TypeId],
    idx_consts: &[Option<u64>],
) -> IrResult<TypeId> {
    if !types.is_ptr_or_ptr_vector(base_ty) {
        return err(IrErrorKind::TypeMismatch, "getelementptr base must be a pointer");
    }
    if !types.is_sized(source) {
        return err(IrErrorKind::InvalidType, "getelementptr source type must be sized");
    }
    let mut vec_len = types.vector_info(base_ty).map(|(_, n, s)| (n, s));
    let mut cur = source;
    for (i, (&it, &ic)) in idx_tys.iter().zip(idx_consts).enumerate() {
        if !types.is_int_or_int_vector(it) {
            return err(IrErrorKind::BadIndex, "getelementptr index must be an integer");
        }
        if let Some((_, n, s)) = types.vector_info(it) {
            match vec_len {
                Some(v) if v != (n, s) => {
                    return err(IrErrorKind::BadIndex, "getelementptr vector widths differ");
                }
                _ => vec_len = Some((n, s)),
            }
        }
        if i == 0 {
            continue;
        }
        cur = match types.data(cur) {
            TypeData::Array { elem, .. } | TypeData::Vector { elem, .. } => *elem,
            TypeData::Struct { .. } | TypeData::Named(_) => {
                let Some(k) = ic.filter(|_| types.int_width(it) == Some(32)) else {
                    return err(IrErrorKind::BadIndex, "struct index must be a constant i32");
                };
                match types.struct_fields(cur).and_then(|f| f.get(k as usize).copied()) {
                    Some(t) => t,
                    None => return err(IrErrorKind::BadIndex, "struct index out of range"),
                }
            }
            _ => return err(IrErrorKind::BadIndex, "getelementptr indexes into a non-aggregate"),
        };
    }
    let p = types.scalar(base_ty);
    Ok(match vec_len {
        Some((n, false)) => types.vector(p, n),
        Some((n, true)) => types.scalable_vector(p, n),
        None => p,
    })
}
