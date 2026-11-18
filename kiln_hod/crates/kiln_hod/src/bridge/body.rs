//! Function bodies: each instruction as `hod_ll`'s body reader builds it
//! from the text kiln prints (its `body.rs`, `inst.rs`, `call.rs`).
//!
//! The reader numbers values in the order it meets them: a value used
//! before its definition is reserved where the text names it first. The
//! walk below visits operands in the order kiln prints them, so that the
//! reservations, and with them the printed numbers, are the reader's.

use std::borrow::Cow;

use hod_ir::kit::{
    AllocaDesc, AllocaError, CALL_SITE_FLAGS, CALL_SITE_NOINLINE, Gep, gep_index_value,
    lower_alloca, set_call_site_flags,
};
use hod_ir::{
    AtomicOrdering, BlockId, Body, Bundle, CallDesc, Clause, Const, FloatPredicate,
    FunctionBuilder, InlineAsm, IntPredicate, IrErrorKind, MemFlags, NoCode, Opcode, ParamAttrs,
    RmwOp, SHUFFLE_UNDEF, SymId, SyncScope, TailKind, TypeId, TypeKind, Value, ValueKind,
    aggregate_field, no_code_intrinsic, resolve_intrinsic, sext64,
};
use kiln_llvm as k;

use super::{Bridge, Fallback, R, call_conv, ir};

/// What the body walk built for a function.
#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "one per function, moved straight into the session; boxing would allocate per body"
)]
pub enum Built {
    /// The body.
    Body(Body),
    /// No body: the constructs that made it unsupported, the first the
    /// reason (recorded in the module as well, as the reader does).
    Unsupported(Vec<Cow<'static, str>>),
}

/// What is known of a symbol as a callee (the reader's `CalleeInfo`).
#[derive(Clone, Copy)]
enum Callee {
    Plain,
    NoCode(NoCode),
    Known(hod_ir::Intrinsic<'static>, [TypeId; 3], u8),
    Other,
}

/// What a call calls.
#[derive(Clone, Copy)]
enum Target {
    Sym(SymId),
    Value(Value),
    Asm(k::AsmId),
}

/// The walk of one body.
struct Fx<'b, 'k> {
    br: &'b mut Bridge<'k>,
    fid: k::FuncId,
    f: &'k k::Function,
    b: FunctionBuilder,
    /// Each instruction's value, once defined.
    vals: Vec<Value>,
    /// Each instruction's reservation, if it was used first.
    forward: Vec<Value>,
    /// Each kiln block's ordinal in the layout.
    ordinal: Vec<u32>,
    /// The ordinal of the block being built.
    current: u32,
}

impl Bridge<'_> {
    /// The body of `fid`, the function of `sym` (`hod_ll`'s body pass).
    ///
    /// # Errors
    /// What the bridge does not map, or where the reader would fail.
    pub fn body(&mut self, fid: k::FuncId, sym: SymId) -> R<Built> {
        let k = self.k;
        let f = k.func(fid);
        let fn_ty = self.module.symbol(sym).ty();
        let b = ir(FunctionBuilder::new(&self.module, sym, fn_ty))?;
        let mut ordinal = vec![u32::MAX; f.num_blocks()];
        for (n, &bb) in f.layout().iter().enumerate() {
            ordinal[bb.index()] = n as u32;
        }
        self.pending.clear();
        let mut fx = Fx {
            br: self,
            fid,
            f,
            b,
            vals: vec![Value::NONE; f.num_insts()],
            forward: vec![Value::NONE; f.num_insts()],
            ordinal,
            current: 0,
        };
        let walked = fx.walk();
        let Fx { br, b, .. } = fx;
        walked?;
        if br.pending.is_empty() {
            match b.finish() {
                Ok(body) => return Ok(Built::Body(body)),
                Err(e) if e.kind == IrErrorKind::UndefinedValue => br.note("no-code-cycle"),
                Err(e) => return ir(Err(e)),
            }
        } else {
            b.abandon();
        }
        let constructs: Vec<Cow<'static, str>> = br.pending.drain(..).collect();
        for construct in &constructs {
            br.module.record_unsupported(construct, Some(sym));
        }
        Ok(Built::Unsupported(constructs))
    }
}

type Done = (TypeId, Value, bool);

impl Fx<'_, '_> {
    fn walk(&mut self) -> R<()> {
        let f = self.f;
        for (n, &bb) in f.layout().iter().enumerate() {
            self.current = n as u32;
            let id = self.block_id(n as u32)?;
            ir(self.b.begin_block(id))?;
            for &i in f.block_insts(bb) {
                if f.debug_records(i).next().is_some() {
                    return Err(Fallback::new("debug"));
                }
                let (ty, value, _) = self.inst(i)?;
                let mut outside_frame = false;
                for &(_, kind, _) in f.inst_metadata(i) {
                    match self.br.k.md_kind_name(kind) {
                        "dbg" => return Err(Fallback::new("debug")),
                        "coro.outside.frame" => outside_frame = true,
                        _ => {}
                    }
                }
                if outside_frame && let ValueKind::Slot(slot) = value.kind() {
                    ir(self.b.set_slot_outside_frame(slot))?;
                }
                self.define(i, ty, value)?;
            }
        }
        Ok(())
    }

    fn define(&mut self, i: k::InstId, ty: TypeId, value: Value) -> R<()> {
        if ty == TypeId::VOID {
            return Ok(());
        }
        let placeholder = self.forward[i.index()];
        if placeholder != Value::NONE {
            ir(self.b.resolve(placeholder, value))?;
        }
        self.vals[i.index()] = value;
        Ok(())
    }

    fn block_id(&mut self, ordinal: u32) -> R<BlockId> {
        while self.b.block_count() <= ordinal as usize {
            ir(self.b.create_block())?;
        }
        Ok(BlockId::new(ordinal))
    }

    fn block(&mut self, bb: k::BlockId) -> R<BlockId> {
        let ordinal = self.ordinal[bb.index()];
        if ordinal == u32::MAX {
            return Err(Fallback::new("block-not-in-layout"));
        }
        self.block_id(ordinal)
    }

    fn block_of(&mut self, v: k::Value) -> R<BlockId> {
        match v.get() {
            k::ValueRef::Block(bb) => self.block(bb),
            _ => Err(Fallback::new("block-operand")),
        }
    }

    /// The type kiln prints `v` with.
    fn type_of(&mut self, v: k::Value) -> R<TypeId> {
        let ty = self.br.k.value_type(self.fid, v);
        self.br.ty(ty)
    }

    /// `v` as an operand of type `ty` (the reader's `value`).
    fn operand(&mut self, v: k::Value, ty: TypeId) -> R<Value> {
        let v = self.br.k.resolve(v);
        match v.get() {
            k::ValueRef::Inst(i) => {
                let at = i.index();
                if self.vals[at] != Value::NONE {
                    return Ok(self.vals[at]);
                }
                if self.forward[at] == Value::NONE {
                    self.forward[at] = ir(self.b.reserve(ty))?;
                }
                Ok(self.forward[at])
            }
            k::ValueRef::Arg(n) => ir(self.b.arg(n)),
            k::ValueRef::Func(_) | k::ValueRef::Global(_) | k::ValueRef::Alias(_)
                if ty == TypeId::PTR =>
            {
                let sym = self.br.sym_of(v).ok_or_else(|| Fallback::new("symbol"))?;
                ir(Value::sym(sym))
            }
            k::ValueRef::Func(_)
            | k::ValueRef::Global(_)
            | k::ValueRef::Alias(_)
            | k::ValueRef::Const(_) => {
                let id = self.br.constant(v)?;
                if ty == TypeId::PTR
                    && let Const::Addr { sym, addend: 0 } = self.br.module.consts.get(id)
                {
                    return ir(Value::sym(sym));
                }
                ir(Value::constant(id))
            }
            _ => Err(Fallback::new("operand")),
        }
    }

    /// `type value`.
    fn typed(&mut self, v: k::Value) -> R<(TypeId, Value)> {
        let ty = self.type_of(v)?;
        Ok((ty, self.operand(v, ty)?))
    }

    fn stand_in(&mut self, ty: TypeId) -> R<Value> {
        let module = &mut self.br.module;
        let id = match ty {
            TypeId::VOID => return Ok(Value::NONE),
            TypeId::TOKEN => module.consts.none_token(),
            _ => module.consts.poison(&module.types, ty),
        };
        ir(Value::constant(ir(id)?))
    }

    fn scope(scope: k::SyncScope) -> R<SyncScope> {
        if scope == k::SyncScope::SYSTEM {
            Ok(SyncScope::System)
        } else if scope == k::SyncScope::SINGLE_THREAD {
            Ok(SyncScope::SingleThread)
        } else {
            // The module does not expose the name of a target's own
            // scope: the text path records it.
            Err(Fallback::new("syncscope"))
        }
    }

    fn mem(align: u64, volatile: bool, ordering: k::Ordering, scope: k::SyncScope) -> R<MemFlags> {
        let mut mem = ir(MemFlags::aligned(align))?;
        if volatile {
            mem = mem.with_volatile();
        }
        if ordering != k::Ordering::NotAtomic {
            let scope = Self::scope(scope)?;
            mem = mem.with_atomic(ordering_of(ordering)?, scope);
        }
        Ok(mem)
    }

    // ---- instructions ----

    fn inst(&mut self, i: k::InstId) -> R<Done> {
        let inst = self.f.inst(i);
        let result = inst.ty();
        let (ty, value) = match *inst.kind() {
            k::InstKind::Binary { op, lhs, rhs, .. } => {
                let ty = self.type_of(lhs)?;
                let lhs = self.operand(lhs, ty)?;
                let rhs = self.operand(rhs, ty)?;
                (ty, ir(self.b.binary(binary_op(op)?, ty, lhs, rhs))?)
            }
            k::InstKind::FNeg { val, .. } => {
                let (ty, arg) = self.typed(val)?;
                (ty, ir(self.b.fneg(ty, arg))?)
            }
            k::InstKind::Freeze { val } => {
                let (ty, arg) = self.typed(val)?;
                (ty, ir(self.b.freeze(ty, arg))?)
            }
            k::InstKind::ICmp { pred, lhs, rhs } => {
                let ty = self.type_of(lhs)?;
                let lhs = self.operand(lhs, ty)?;
                let rhs = self.operand(rhs, ty)?;
                let result = self.br.ty(result)?;
                (result, ir(self.b.icmp(int_predicate(pred)?, result, lhs, rhs))?)
            }
            k::InstKind::FCmp { pred, lhs, rhs, .. } => {
                let ty = self.type_of(lhs)?;
                let lhs = self.operand(lhs, ty)?;
                let rhs = self.operand(rhs, ty)?;
                let result = self.br.ty(result)?;
                let pred = FloatPredicate::ALL
                    .iter()
                    .copied()
                    .find(|p| p.name() == pred.name())
                    .ok_or_else(|| Fallback::new("fcmp-predicate"))?;
                (result, ir(self.b.fcmp(pred, result, lhs, rhs))?)
            }
            k::InstKind::Cast { op, val, .. } => {
                let (_, arg) = self.typed(val)?;
                let to = self.br.ty(result)?;
                (to, ir(self.b.cast(cast_op(op)?, to, arg))?)
            }
            k::InstKind::Select { cond, t, f, .. } => {
                let (_, cond) = self.typed(cond)?;
                let (ty, if_true) = self.typed(t)?;
                let (_, if_false) = self.typed(f)?;
                (ty, ir(self.b.select(ty, cond, if_true, if_false))?)
            }
            k::InstKind::Phi { incoming, .. } => {
                let ty = self.br.ty(result)?;
                let pairs = self.f.ops(incoming);
                let mut list = Vec::with_capacity(pairs.len() / 2);
                for pair in pairs.chunks(2) {
                    let value = self.operand(pair[0], ty)?;
                    let block = self.block_of(pair[1])?;
                    list.push((value, block));
                }
                (ty, ir(self.b.phi(ty, &list))?)
            }
            k::InstKind::VaArg { list } => {
                let (_, list) = self.typed(list)?;
                let ty = self.br.ty(result)?;
                (ty, ir(self.b.va_arg(ty, list))?)
            }
            k::InstKind::Load { ptr, align, volatile, ordering, scope } => {
                let ty = self.br.ty(result)?;
                let (_, ptr) = self.typed(ptr)?;
                let align = align.bytes().unwrap_or_else(|| self.br.module.types.align_of(ty));
                let mem = Self::mem(align, volatile, ordering, scope)?;
                (ty, ir(self.b.load(ty, ptr, mem))?)
            }
            k::InstKind::Store { val, ptr, align, volatile, ordering, scope } => {
                let (ty, value) = self.typed(val)?;
                let (_, ptr) = self.typed(ptr)?;
                let align = align.bytes().unwrap_or_else(|| self.br.module.types.align_of(ty));
                let mem = Self::mem(align, volatile, ordering, scope)?;
                ir(self.b.store(value, ptr, mem))?;
                (TypeId::VOID, Value::NONE)
            }
            k::InstKind::Fence { ordering, scope } => {
                let scope = Self::scope(scope)?;
                ir(self.b.fence(ordering_of(ordering)?, scope))?;
                (TypeId::VOID, Value::NONE)
            }
            k::InstKind::CmpXchg {
                ptr,
                cmp,
                new,
                align,
                success,
                failure,
                weak,
                volatile,
                scope,
            } => {
                let (_, ptr) = self.typed(ptr)?;
                let (ty, expected) = self.typed(cmp)?;
                let (_, replacement) = self.typed(new)?;
                let types = &self.br.module.types;
                let align = align.bytes().unwrap_or_else(|| types.size_of(ty).next_power_of_two());
                let mem = Self::mem(align, volatile, success, scope)?;
                let failure = ordering_of(failure)?;
                let result = ir(self.br.module.types.struct_type(&[ty, TypeId::I1], false))?;
                let value = self.b.cmpxchg(result, ptr, expected, replacement, mem, failure, weak);
                (result, ir(value)?)
            }
            k::InstKind::AtomicRmw { op, ptr, val, align, ordering, volatile, scope } => {
                let op = RmwOp::ALL
                    .iter()
                    .copied()
                    .find(|o| o.name() == op.name())
                    .ok_or_else(|| Fallback::new("atomicrmw-op"))?;
                let (_, ptr) = self.typed(ptr)?;
                let (ty, value) = self.typed(val)?;
                let types = &self.br.module.types;
                let align = align.bytes().unwrap_or_else(|| types.size_of(ty).next_power_of_two());
                let mem = Self::mem(align, volatile, ordering, scope)?;
                (ty, ir(self.b.atomic_rmw(op, ty, ptr, value, mem))?)
            }
            k::InstKind::Alloca { allocated, count, align, inalloca } => {
                let ty = self.br.ty(allocated)?;
                let count = if count.is_none() { None } else { Some(self.typed(count)?.1) };
                let module = &mut self.br.module;
                let desc = AllocaDesc {
                    ty,
                    count,
                    align: align.bytes(),
                    addr_space: module.types.data_layout().alloca_addr_space,
                    in_entry_block: self.current == 0,
                    in_alloca: inalloca,
                    swift_error: false,
                };
                let (result, lowered) = match lower_alloca(module, &mut self.b, &desc) {
                    Ok(done) => done,
                    Err(AllocaError::InvalidType) => return Err(Fallback::new("alloca-type")),
                    Err(AllocaError::Ir(e)) => return ir(Err(e)),
                };
                if let Some(construct) = lowered.refused() {
                    self.br.note(construct);
                }
                (result, lowered.value())
            }
            k::InstKind::Gep { source, ops, .. } => self.gep(source, ops)?,
            k::InstKind::ExtractValue { agg, idx } => {
                let (agg_ty, agg) = self.typed(agg)?;
                let field = self.br.ty(result)?;
                let path = self.f.idx(idx);
                if let Ok((offset, _)) = aggregate_field(&self.br.module.types, agg_ty, path) {
                    (field, ir(self.b.extract_value(field, agg, offset))?)
                } else {
                    self.br.note("aggregate:no-fixed-offset");
                    (field, self.stand_in(field)?)
                }
            }
            k::InstKind::InsertValue { agg, val, idx } => {
                let (agg_ty, agg) = self.typed(agg)?;
                let (_, value) = self.typed(val)?;
                let path = self.f.idx(idx);
                match aggregate_field(&self.br.module.types, agg_ty, path) {
                    Ok((offset, _)) if u32::try_from(offset).is_ok() => {
                        (agg_ty, ir(self.b.insert_value(agg_ty, agg, value, offset))?)
                    }
                    Ok(_) => {
                        self.br.note("insertvalue:offset-over-4GiB");
                        (agg_ty, self.stand_in(agg_ty)?)
                    }
                    Err(_) => {
                        self.br.note("aggregate:no-fixed-offset");
                        (agg_ty, self.stand_in(agg_ty)?)
                    }
                }
            }
            k::InstKind::ExtractElement { vec, idx } => {
                let (_, vector) = self.typed(vec)?;
                let (_, index) = self.typed(idx)?;
                let elem = self.br.ty(result)?;
                (elem, ir(self.b.extract_element(elem, vector, index))?)
            }
            k::InstKind::InsertElement { vec, elt, idx } => {
                let (ty, vector) = self.typed(vec)?;
                let (_, value) = self.typed(elt)?;
                let (_, index) = self.typed(idx)?;
                (ty, ir(self.b.insert_element(ty, vector, value, index))?)
            }
            k::InstKind::ShuffleVector { a, b, mask } => {
                let (_, first) = self.typed(a)?;
                let (_, second) = self.typed(b)?;
                let mask: Vec<u32> = self
                    .f
                    .idx(mask)
                    .iter()
                    .map(|&m| if m == k::SHUFFLE_POISON { SHUFFLE_UNDEF } else { m })
                    .collect();
                let result = self.br.ty(result)?;
                (result, ir(self.b.shuffle_vector(result, first, second, &mask))?)
            }
            k::InstKind::Call(c) => return self.call(c, Opcode::Call),
            k::InstKind::Invoke(c) => return self.call(c, Opcode::Invoke),
            k::InstKind::CallBr(c) => return self.call(c, Opcode::CallBr),
            k::InstKind::LandingPad(p) => {
                let ty = self.br.ty(result)?;
                let pad = self.f.landing_pad(p);
                let mut clauses = Vec::with_capacity(pad.clauses().len());
                for &clause in pad.clauses() {
                    clauses.push(match clause {
                        k::Clause::Catch(v) => Clause::Catch(self.br.constant(v)?),
                        k::Clause::Filter(v) => Clause::Filter(self.br.constant(v)?),
                    });
                }
                (ty, ir(self.b.landing_pad(ty, pad.cleanup(), &clauses))?)
            }
            k::InstKind::Resume { val } => {
                let (_, value) = self.typed(val)?;
                ir(self.b.resume(value))?;
                return Ok((TypeId::VOID, Value::NONE, true));
            }
            k::InstKind::CatchSwitch { .. }
            | k::InstKind::CatchPad { .. }
            | k::InstKind::CleanupPad { .. }
            | k::InstKind::CatchRet { .. }
            | k::InstKind::CleanupRet { .. } => return Err(Fallback::new("funclet")),
            k::InstKind::Br { dest } => {
                let target = self.block(dest)?;
                ir(self.b.br(target))?;
                return Ok((TypeId::VOID, Value::NONE, true));
            }
            k::InstKind::CondBr { cond, t, f } => {
                let cond = self.operand(cond, TypeId::I1)?;
                let if_true = self.block(t)?;
                let if_false = self.block(f)?;
                ir(self.b.cond_br(cond, if_true, if_false))?;
                return Ok((TypeId::VOID, Value::NONE, true));
            }
            k::InstKind::Switch { cond, default, cases } => {
                let (_, cond) = self.typed(cond)?;
                let default = self.block(default)?;
                let pairs = self.f.ops(cases);
                let mut list = Vec::with_capacity(pairs.len() / 2);
                for pair in pairs.chunks(2) {
                    let constant = self.br.constant(pair[0])?;
                    let target = self.block_of(pair[1])?;
                    list.push((constant, target));
                }
                ir(self.b.switch(cond, default, &list))?;
                return Ok((TypeId::VOID, Value::NONE, true));
            }
            k::InstKind::IndirectBr { addr, dests } => {
                let (_, address) = self.typed(addr)?;
                let mut targets = Vec::new();
                for &d in self.f.ops(dests) {
                    targets.push(self.block_of(d)?);
                }
                ir(self.b.indirect_br(address, &targets))?;
                return Ok((TypeId::VOID, Value::NONE, true));
            }
            k::InstKind::Ret { val } => {
                let value = if val.is_none() { None } else { Some(self.typed(val)?.1) };
                ir(self.b.ret(value))?;
                return Ok((TypeId::VOID, Value::NONE, true));
            }
            k::InstKind::Unreachable => {
                ir(self.b.unreachable())?;
                return Ok((TypeId::VOID, Value::NONE, true));
            }
        };
        Ok((ty, value, false))
    }

    /// `getelementptr`: a constant index the text writes as a short
    /// literal is read as one (`sext64` of its value).
    fn gep(&mut self, source: k::TypeId, ops: k::OpRange) -> R<(TypeId, Value)> {
        let ops = self.f.ops(ops);
        let (&base, indices) = ops.split_first().ok_or_else(|| Fallback::new("gep"))?;
        let source = self.br.ty(source)?;
        let (base_ty, base) = self.typed(base)?;
        let mut gep = Gep::new(&self.br.module.types, source, base_ty)
            .map_err(|_| Fallback::new("gep-shape"))?;
        let mut lowered_indices = Vec::new();
        for &index in indices {
            let index_ty = self.type_of(index)?;
            let bits = gep
                .index_type(&self.br.module.types, index_ty)
                .map_err(|_| Fallback::new("gep-index"))?;
            let (value, constant) = if let Some(literal) = self.literal(index, index_ty) {
                (Value::NONE, Some(sext64(literal as u64, bits)))
            } else {
                let value = self.operand(index, index_ty)?;
                let module = &self.br.module;
                let constant = value
                    .as_const()
                    .and_then(|id| gep_index_value(&module.types, &module.consts, id));
                (value, constant)
            };
            gep.index(&self.br.module.types, value, constant, &mut lowered_indices)
                .map_err(|_| Fallback::new("gep-index"))?;
        }
        let (result, lowered) =
            ir(gep.build(&mut self.br.module, &mut self.b, base, &lowered_indices))?;
        if let Some(construct) = lowered.refused() {
            self.br.note(construct);
        }
        Ok((result, lowered.value()))
    }

    /// The value of an index the text writes as an integer literal of at
    /// most 18 characters (not `true`/`false`, not a vector).
    fn literal(&self, index: k::Value, ty: TypeId) -> Option<i64> {
        let types = &self.br.module.types;
        if !types.is_int(ty) || ty == TypeId::I1 {
            return None;
        }
        let k::ConstData::Int { value: k::IntValue::Small(v), .. } = self.br.k.const_of(index)?
        else {
            return None;
        };
        let bits = types.int_bits(ty)?;
        let signed = if bits >= 128 {
            *v as i128
        } else {
            let shift = 128 - bits;
            ((*v << shift) as i128) >> shift
        };
        let literal = i64::try_from(signed).ok()?;
        (literal.to_string().len() <= 18).then_some(literal)
    }

    // ---- calls ----

    fn callee_info(&mut self, sym: SymId) -> Callee {
        let module = &self.br.module;
        let symbol = module.symbol(sym);
        match std::str::from_utf8(symbol.name()) {
            Ok(name) if name.starts_with("llvm.") && symbol.is_function() => {
                if let Some(rule) = no_code_intrinsic(name) {
                    return Callee::NoCode(rule);
                }
                let found = resolve_intrinsic(&module.types, name, symbol.ty());
                let known = match found.intrinsic {
                    hod_ir::Intrinsic::Other(_) => None,
                    intrinsic => hod_ir::Intrinsic::ALL
                        .iter()
                        .copied()
                        .find(|member| member.name() == intrinsic.name()),
                };
                match known {
                    Some(member) => {
                        let mut overloads = [TypeId::VOID; 3];
                        let given = found.overloads();
                        overloads[..given.len()].copy_from_slice(given);
                        Callee::Known(member, overloads, given.len() as u8)
                    }
                    None => Callee::Other,
                }
            }
            _ => Callee::Plain,
        }
    }

    /// A `metadata` argument: a string or a node of strings is a metadata
    /// operand of the body; anything else is not (`None`).
    fn metadata_arg(&mut self, v: k::Value) -> R<Option<Value>> {
        let k = self.br.k;
        let k::ValueRef::Metadata(md) = v.get() else {
            return Err(Fallback::new("metadata-arg"));
        };
        let utf8 = |s: &[u8]| std::str::from_utf8(s).ok().map(str::to_owned);
        match k.md_node(md) {
            k::MdNode::String(s) => match utf8(s) {
                Some(s) => Ok(Some(ir(self.b.metadata(false, &[&s]))?)),
                None => Ok(None),
            },
            k::MdNode::Tuple { distinct: false, ops } => {
                let mut strings = Vec::with_capacity(ops.len());
                for op in ops {
                    match op.map(|op| k.md_node(op)) {
                        Some(k::MdNode::String(s)) => match utf8(s) {
                            Some(s) => strings.push(s),
                            None => return Ok(None),
                        },
                        _ => return Ok(None),
                    }
                }
                let strings: Vec<&str> = strings.iter().map(String::as_str).collect();
                Ok(Some(ir(self.b.metadata(true, &strings))?))
            }
            &k::MdNode::Value(ty, value) => {
                let ty = self.br.ty(ty)?;
                self.operand(value, ty)?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn successors(
        &mut self,
        call: &k::CallData,
        op: Opcode,
    ) -> R<(BlockId, BlockId, Vec<BlockId>)> {
        let missing = || Fallback::new("call-successor");
        match op {
            Opcode::Invoke => {
                let normal = self.block(call.normal().ok_or_else(missing)?)?;
                let unwind = self.block(call.unwind().ok_or_else(missing)?)?;
                Ok((normal, unwind, Vec::new()))
            }
            Opcode::CallBr => {
                let normal = self.block(call.normal().ok_or_else(missing)?)?;
                let mut targets = Vec::with_capacity(call.indirect().len());
                for &bb in call.indirect() {
                    targets.push(self.block(bb)?);
                }
                Ok((normal, normal, targets))
            }
            _ => Ok((BlockId::new(0), BlockId::new(0), Vec::new())),
        }
    }

    fn stand_in_call(&mut self, call: &k::CallData, op: Opcode, ret: TypeId) -> R<Done> {
        let value = self.stand_in(ret)?;
        if op == Opcode::Call {
            return Ok((ret, value, false));
        }
        let (normal, ..) = self.successors(call, op)?;
        ir(self.b.br(normal))?;
        Ok((ret, value, true))
    }

    fn call(&mut self, c: u32, op: Opcode) -> R<Done> {
        let k = self.br.k;
        let f = self.f;
        let call = f.call_data(c);
        let cc = call_conv(call.cc())?;
        let ret_attrs = self.br.value_attrs(call.attrs().ret)?;
        let (ret, _, varargs) =
            k.types.function_info(call.fn_ty()).ok_or_else(|| Fallback::new("call-type"))?;
        let written = if varargs { self.br.ty(call.fn_ty())? } else { self.br.ty(ret)? };
        let ret = self.br.ty(ret)?;
        let callee = k.resolve(call.callee());
        let callee_ty = k.value_type(self.fid, callee);
        let ptr_ty = match k.types.ptr_addrspace(callee_ty) {
            Some(space) if space != 0 => ir(self.br.module.types.ptr(space))?,
            _ => TypeId::PTR,
        };
        let target = match callee.get() {
            k::ValueRef::Func(_) | k::ValueRef::Global(_) | k::ValueRef::Alias(_) => {
                let sym = self.br.sym_of(callee).ok_or_else(|| Fallback::new("symbol"))?;
                if ptr_ty == TypeId::PTR {
                    Target::Sym(sym)
                } else {
                    Target::Value(ir(Value::sym(sym))?)
                }
            }
            k::ValueRef::Inst(_) | k::ValueRef::Arg(_) => {
                Target::Value(self.operand(callee, ptr_ty)?)
            }
            k::ValueRef::Asm(a) => Target::Asm(a),
            k::ValueRef::Const(_) => {
                let id = self.br.constant(callee)?;
                match self.br.module.consts.get(id) {
                    Const::Addr { sym, addend: 0 } => Target::Sym(sym),
                    _ => Target::Value(ir(Value::constant(id))?),
                }
            }
            _ => return Err(Fallback::new("callee")),
        };
        let info = match target {
            Target::Sym(sym) => self.callee_info(sym),
            _ => Callee::Plain,
        };
        if let Callee::NoCode(NoCode::DebugRecord) = info {
            return Err(Fallback::new("debug"));
        }

        // The arguments.
        let args = f.ops(call.args());
        let (mut vals, mut arg_tys, mut arg_attrs) =
            (Vec::with_capacity(args.len()), Vec::with_capacity(args.len()), Vec::new());
        let (mut any_attrs, mut metadata) = (false, false);
        for (n, &arg) in args.iter().enumerate() {
            let ty = self.type_of(arg)?;
            let attrs = self.br.value_attrs(call.attrs().param(n))?;
            any_attrs |= attrs != ParamAttrs::NONE;
            let value = if ty == TypeId::METADATA {
                let meta = self.metadata_arg(arg)?;
                metadata |= meta.is_none();
                meta.unwrap_or(Value::NONE)
            } else {
                self.operand(arg, ty)?
            };
            vals.push(value);
            arg_tys.push(ty);
            arg_attrs.push(attrs);
        }

        // The function type.
        let types = &self.br.module.types;
        let fn_ty = if let TypeKind::Function { .. } = types.kind(written) {
            written
        } else {
            let own = match target {
                Target::Sym(sym) => {
                    let sym_ty = self.br.module.symbol(sym).ty();
                    match types.kind(sym_ty) {
                        TypeKind::Function { ret: r, params, variadic: false }
                            if r == ret && params == arg_tys.as_slice() =>
                        {
                            Some(sym_ty)
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            match own {
                Some(ty) => ty,
                None => ir(self.br.module.types.function(ret, &arg_tys, false))?,
            }
        };

        // The call-site function attributes.
        let mut flags = 0;
        let set = call.attrs().func;
        if set != k::AttrSetId::EMPTY {
            let group =
                self.br.groups[set.index()].clone().ok_or_else(|| Fallback::new("group"))?;
            flags |= self.br.module.fn_attrs(group.attrs).flags();
            if group.noinline {
                flags |= CALL_SITE_NOINLINE;
            }
            for name in &group.rejected {
                let known = self
                    .br
                    .pending
                    .iter()
                    .any(|noted| noted.strip_prefix("attr:") == Some(&**name));
                if !known {
                    self.br.note(format!("attr:{name}"));
                }
            }
        }
        let flags = flags & CALL_SITE_FLAGS;

        // The operand bundles.
        let mut bundle_vals: Vec<Vec<Value>> = Vec::with_capacity(call.bundles().len());
        for bundle in call.bundles() {
            let mut inputs = Vec::with_capacity(bundle.args.len());
            for &arg in &bundle.args {
                let ty = self.type_of(arg)?;
                if ty == TypeId::METADATA {
                    self.br.note("bundle:metadata-input");
                    continue;
                }
                inputs.push(self.operand(arg, ty)?);
            }
            bundle_vals.push(inputs);
        }

        let llvm = matches!(info, Callee::NoCode(_) | Callee::Known(..) | Callee::Other);
        if llvm
            && let Target::Sym(sym) = target
            && self.br.module.symbol(sym).ty() != fn_ty
        {
            return Err(Fallback::new("intrinsic-signature"));
        }

        if let Callee::NoCode(rule) = info {
            if op != Opcode::Call {
                self.br.note("invoke:no-code-intrinsic");
                return self.stand_in_call(call, op, ret);
            }
            if rule == NoCode::FirstArg {
                if arg_tys.first() != Some(&ret) {
                    return Err(Fallback::new("intrinsic-first-arg"));
                }
                return Ok((ret, vals[0], false));
            }
            if let Target::Sym(sym) = target
                && let Ok(name) = std::str::from_utf8(self.br.module.symbol(sym).name())
                && let Some(start) = hod_ir::lifetime_intrinsic(name)
                && let Some(&ptr) = vals.last()
            {
                ir(self.b.lifetime_marker(start, ptr))?;
            }
            return self.stand_in_call(call, op, ret);
        }

        let metadata =
            metadata || (!matches!(info, Callee::Known(..)) && arg_tys.contains(&TypeId::METADATA));
        if metadata {
            let construct = match target {
                Target::Sym(sym) => format!(
                    "metadata-arg:{}",
                    String::from_utf8_lossy(self.br.module.symbol(sym).name())
                ),
                _ => "metadata-arg:indirect".to_owned(),
            };
            self.br.note(construct);
            return self.stand_in_call(call, op, ret);
        }

        let lowered = match target {
            Target::Sym(sym) => match info {
                Callee::Known(..) | Callee::Other => None,
                _ => Some(ir(Value::sym(sym))?),
            },
            Target::Value(value) => Some(value),
            Target::Asm(a) => {
                let asm = k.asm(a);
                let mut flags = 0;
                if asm.side_effect {
                    flags |= InlineAsm::SIDE_EFFECT;
                }
                if asm.align_stack {
                    flags |= InlineAsm::ALIGN_STACK;
                }
                if asm.intel_dialect {
                    flags |= InlineAsm::INTEL_DIALECT;
                }
                if asm.can_throw {
                    flags |= InlineAsm::UNWIND;
                }
                let desc = InlineAsm {
                    template: asm.asm.as_bytes(),
                    constraints: asm.constraints.as_bytes(),
                    flags,
                    ty: fn_ty,
                };
                let id = ir(self.br.module.inline_asm(&desc))?;
                Some(ir(Value::asm(id))?)
            }
        };
        let (normal, unwind, targets) = self.successors(call, op)?;
        let bundles: Vec<Bundle<'_>> = call
            .bundles()
            .iter()
            .zip(&bundle_vals)
            .map(|(bundle, inputs)| Bundle { tag: &bundle.tag, inputs })
            .collect();
        let overloads;
        let mut desc = match (lowered, info, target) {
            (Some(value), _, _) => CallDesc::new(value, fn_ty, &vals),
            (None, Callee::Known(intrinsic, types, len), _) => {
                overloads = types;
                CallDesc::intrinsic(intrinsic, &overloads[..usize::from(len)], fn_ty, &vals)
            }
            (None, _, Target::Sym(sym)) => {
                let name = std::str::from_utf8(self.br.module.symbol(sym).name())
                    .map_err(|_| Fallback::new("intrinsic-name"))?
                    .to_owned();
                return self.emit_other(
                    call,
                    op,
                    cc,
                    flags,
                    ret_attrs,
                    any_attrs,
                    &arg_attrs,
                    &bundles,
                    (fn_ty, ret),
                    &name,
                    &vals,
                    (normal, unwind, &targets),
                );
            }
            (None, ..) => return Err(Fallback::new("callee")),
        };
        desc.cc = cc;
        desc.tail = tail_kind(call.tail());
        set_call_site_flags(&mut desc, flags);
        desc.ret_attrs = ret_attrs;
        desc.arg_attrs = if any_attrs { &arg_attrs } else { &[] };
        desc.bundles = &bundles;
        let value = match op {
            Opcode::Invoke => ir(self.b.invoke(ret, &desc, normal, unwind))?,
            Opcode::CallBr => ir(self.b.callbr(ret, &desc, normal, &targets))?,
            _ => ir(self.b.call(ret, &desc))?,
        };
        Ok((ret, value, op != Opcode::Call))
    }

    /// A call of an intrinsic `hod_ir` does not know by name.
    #[allow(clippy::too_many_arguments, reason = "the reader's `emit_call`, split")]
    fn emit_other(
        &mut self,
        call: &k::CallData,
        op: Opcode,
        cc: hod_ir::CallConv,
        flags: u32,
        ret_attrs: ParamAttrs,
        any_attrs: bool,
        arg_attrs: &[ParamAttrs],
        bundles: &[Bundle<'_>],
        (fn_ty, ret): (TypeId, TypeId),
        name: &str,
        vals: &[Value],
        (normal, unwind, targets): (BlockId, BlockId, &[BlockId]),
    ) -> R<Done> {
        let mut desc = CallDesc::intrinsic(hod_ir::Intrinsic::Other(name), &[], fn_ty, vals);
        desc.cc = cc;
        desc.tail = tail_kind(call.tail());
        set_call_site_flags(&mut desc, flags);
        desc.ret_attrs = ret_attrs;
        desc.arg_attrs = if any_attrs { arg_attrs } else { &[] };
        desc.bundles = bundles;
        let value = match op {
            Opcode::Invoke => ir(self.b.invoke(ret, &desc, normal, unwind))?,
            Opcode::CallBr => ir(self.b.callbr(ret, &desc, normal, targets))?,
            _ => ir(self.b.call(ret, &desc))?,
        };
        Ok((ret, value, op != Opcode::Call))
    }
}

fn tail_kind(t: k::TailKind) -> TailKind {
    match t {
        k::TailKind::None => TailKind::None,
        k::TailKind::Tail => TailKind::Tail,
        k::TailKind::MustTail => TailKind::MustTail,
        k::TailKind::NoTail => TailKind::NoTail,
    }
}

fn ordering_of(o: k::Ordering) -> R<AtomicOrdering> {
    AtomicOrdering::ALL
        .iter()
        .copied()
        .find(|a| a.name() == o.name())
        .ok_or_else(|| Fallback::new("atomic-ordering"))
}

fn int_predicate(p: k::IntPredicate) -> R<IntPredicate> {
    Ok(match p.name() {
        "eq" => IntPredicate::Eq,
        "ne" => IntPredicate::Ne,
        "ugt" => IntPredicate::Ugt,
        "uge" => IntPredicate::Uge,
        "ult" => IntPredicate::Ult,
        "ule" => IntPredicate::Ule,
        "sgt" => IntPredicate::Sgt,
        "sge" => IntPredicate::Sge,
        "slt" => IntPredicate::Slt,
        "sle" => IntPredicate::Sle,
        _ => return Err(Fallback::new("icmp-predicate")),
    })
}

fn binary_op(op: k::BinOp) -> R<Opcode> {
    Ok(match op.name() {
        "add" => Opcode::Add,
        "sub" => Opcode::Sub,
        "mul" => Opcode::Mul,
        "udiv" => Opcode::UDiv,
        "sdiv" => Opcode::SDiv,
        "urem" => Opcode::URem,
        "srem" => Opcode::SRem,
        "shl" => Opcode::Shl,
        "lshr" => Opcode::LShr,
        "ashr" => Opcode::AShr,
        "and" => Opcode::And,
        "or" => Opcode::Or,
        "xor" => Opcode::Xor,
        "fadd" => Opcode::FAdd,
        "fsub" => Opcode::FSub,
        "fmul" => Opcode::FMul,
        "fdiv" => Opcode::FDiv,
        "frem" => Opcode::FRem,
        other => return Err(Fallback(format!("binary:{other}").into())),
    })
}

fn cast_op(op: k::CastOp) -> R<Opcode> {
    Ok(match op.name() {
        "trunc" => Opcode::Trunc,
        "zext" => Opcode::ZExt,
        "sext" => Opcode::SExt,
        "fptrunc" => Opcode::FPTrunc,
        "fpext" => Opcode::FPExt,
        "fptoui" => Opcode::FPToUI,
        "fptosi" => Opcode::FPToSI,
        "uitofp" => Opcode::UIToFP,
        "sitofp" => Opcode::SIToFP,
        "ptrtoint" => Opcode::PtrToInt,
        "ptrtoaddr" => Opcode::PtrToAddr,
        "inttoptr" => Opcode::IntToPtr,
        "bitcast" => Opcode::BitCast,
        "addrspacecast" => Opcode::AddrSpaceCast,
        other => return Err(Fallback(format!("cast:{other}").into())),
    })
}
