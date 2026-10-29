//! Function bodies (spec/codegen.md §5): clang's `StartFunction`,
//! `FinishFunction`, the return block and the instruction helpers
//! (with `IRBuilder`'s constant folding).

use kiln_abi::call::ArgKind;
use kiln_abi::irtype::IrType;
use kiln_abi::mangle;
use kiln_ast::decl::DeclKind;
use kiln_ast::{DeclId, QualType};
use kiln_llvm::{
    Attr, AttrKind, AttrList, BinOp, CallOpts, CastOp, ConstData, Flags, FloatPredicate, FuncId,
    GepFlags, IntPredicate, IntValue, MemOpts, MetadataId, TypeId as LTy, Value,
};
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use crate::fir::{Blk, Fir, Op, V};
use crate::module::{Cg, R, Sig, err};

/// An address with its element type and alignment (clang `Address`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Addr {
    pub v: V,
    pub ty: LTy,
    pub align: u64,
}

/// A `break`/`continue` target pair.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Jump {
    pub brk: Blk,
    pub cont: Option<Blk>,
}

/// The active `switch`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SwitchCtx {
    pub inst: u32,
    pub cond_ty: LTy,
    pub default: Blk,
}

/// An active loop (clang `LoopInfo`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct LoopFrame {
    pub header: Blk,
    pub md: Option<MetadataId>,
}

/// A parameter's value after clang `EmitFunctionProlog`.
enum ParamVal {
    /// A scalar value, spilled to a `.addr` alloca by `EmitParmDecl`.
    Direct(V),
    /// The parameter's storage; `spill` for a non-`byval` indirect
    /// argument, whose pointer `EmitParmDecl` stores to an
    /// `.indirect_addr` alloca.
    Indirect(Addr, bool),
}

/// An evaluated call argument (clang `CallArg`).
#[derive(Clone, Copy)]
pub(crate) enum CallArg {
    /// A scalar value.
    Scalar(V),
    /// An aggregate in memory (`agg.tmp`).
    Agg(Addr),
}

/// How the function returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RetKind {
    Void,
    /// Direct or extended with the coerce type equal to the value type.
    Scalar,
    /// Coerced from the `retval` memory to the given type.
    Coerced,
    Sret,
}

/// Per-function state.
pub(crate) struct Fx {
    pub fir: Fir,
    pub decl: Option<DeclId>,
    pub func: Option<FuncId>,
    pub locals: FxHashMap<DeclId, Addr>,
    pub ret_block: Option<Blk>,
    pub ret_slot: Option<Addr>,
    pub ret_kind: RetKind,
    pub ret_coerce: Option<IrType>,
    pub sig: Option<Sig>,
    pub is_main: bool,
    pub jumps: Vec<Jump>,
    pub labels: FxHashMap<DeclId, Blk>,
    pub sw: Option<SwitchCtx>,
    pub loops: Vec<LoopFrame>,
    pub mustprogress: bool,
    /// clang `LargestVectorWidth`: the widest vector in the signature or
    /// any call's IR arguments and result (x86 `min-legal-vector-width`).
    pub vector_width: u64,
    /// Calls: (instruction, callee, call-site nounwind).
    pub calls: Vec<(u32, Option<FuncId>, bool)>,
    pub loop_mp: Option<MetadataId>,
    /// The active conditional evaluations (clang `ConditionalEvaluation`),
    /// innermost last: each is the block the condition started in.
    pub conds: Vec<Blk>,
    /// The bound common expressions of the enclosing GNU binary
    /// conditionals (clang `OpaqueValueMapping`).
    pub opaque: FxHashMap<kiln_ast::ExprId, crate::expr::Opaque>,
}

impl Fx {
    pub fn empty() -> Fx {
        Fx {
            fir: Fir::new(Vec::new()),
            decl: None,
            func: None,
            locals: FxHashMap::default(),
            ret_block: None,
            ret_slot: None,
            ret_kind: RetKind::Void,
            ret_coerce: None,
            sig: None,
            is_main: false,
            jumps: Vec::new(),
            labels: FxHashMap::default(),
            sw: None,
            loops: Vec::new(),
            mustprogress: true,
            vector_width: 0,
            calls: Vec::new(),
            loop_mp: None,
            conds: Vec::new(),
            opaque: FxHashMap::default(),
        }
    }
}

fn mem(align: u64, volatile: bool) -> MemOpts {
    let mut m = MemOpts::aligned(align);
    m.volatile = volatile;
    m
}

impl Cg<'_> {
    // ---- function definitions ------------------------------------------

    /// clang `GenerateCode` for the function definition `d`.
    pub fn emit_fn_def(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        let f = self.get_fn(d)?;
        let first = ast.first(d);
        let fty = ast.decls.get(d).ty().expect("function type");
        let extern_c = mangle::is_extern_c(ast, first);
        let sig = self.sig(fty, extern_c);
        let fd = *ast.decls.function(d).expect("function");
        let Some(body) = fd.body else { return Ok(()) };
        let is_main = mangle::decl_name(ast, first) == "main"
            && ast.decls.get(first).ctx().is_some_and(|c| self.is_tu_level(c));

        let params: Vec<LTy> = {
            let func = self.m.func(f);
            func.param_types().to_vec()
        };
        self.f = Fx::empty();
        self.f.fir = Fir::new(params);
        self.f.decl = Some(d);
        self.f.func = Some(f);
        self.f.is_main = is_main;
        self.f.sig = Some(sig.clone());
        self.f.vector_width = sig.info.max_vector_width;
        self.set_def_linkage(f, d);

        // StartFunction.
        let ret_block = self.f.fir.create_block();
        self.f.ret_block = Some(ret_block);
        let mut arg_no = 0u32;
        match &sig.info.ret.kind {
            ArgKind::Ignore => self.f.ret_kind = RetKind::Void,
            ArgKind::Indirect { byval, align } => {
                self.f.ret_kind = RetKind::Sret;
                let ty = self.mty(sig.ret);
                self.f.ret_slot = Some(Addr { v: V::Arg(0), ty, align: *align });
                arg_no = 1;
                if !*byval {
                    let p = self.f.fir.alloca(LTy::PTR, 8);
                    self.store(V::Arg(0), p, 8, false);
                }
            }
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                let vt = self.lty(sig.ret);
                let (_, align) = self.size_align(sig.ret);
                let slot = self.f.fir.alloca(vt, align);
                self.f.ret_slot = Some(Addr { v: slot, ty: vt, align });
                let ct = self.ir(coerce);
                if ct == vt {
                    self.f.ret_kind = RetKind::Scalar;
                } else {
                    self.f.ret_kind = RetKind::Coerced;
                    self.f.ret_coerce = Some(coerce.clone());
                }
            }
        }
        if is_main && let Some(slot) = self.f.ret_slot {
            let z = self.m.const_int(slot.ty, 0);
            self.store(V::C(z), slot.v, slot.align, false);
        }
        // EmitFunctionProlog: every argument first, then EmitParmDecl for
        // each parameter in order.
        let param_decls: Vec<DeclId> = ast.lists.iter(fd.params).collect();
        let mut vals = Vec::with_capacity(param_decls.len());
        for (i, &p) in param_decls.iter().enumerate() {
            let pq = ast.decls.get(p).ty().expect("param type");
            let info = sig.info.args[i].clone();
            let loc = Some(ast.decls.get(p).loc());
            vals.push(self.prolog_arg(&info.kind, pq, &mut arg_no, loc)?);
        }
        for (&p, val) in param_decls.iter().zip(vals) {
            let pq = ast.decls.get(p).ty().expect("param type");
            let addr = match val {
                ParamVal::Indirect(a, spill) => {
                    if spill {
                        let p = self.f.fir.alloca(LTy::PTR, 8);
                        self.store(a.v, p, 8, false);
                    }
                    a
                }
                ParamVal::Direct(v) => {
                    let mt = self.mty(pq);
                    let (_, align) = self.size_align(pq);
                    let a = self.f.fir.alloca(mt, align);
                    let addr = Addr { v: a, ty: mt, align };
                    self.store_scalar(v, addr, pq);
                    addr
                }
            };
            self.f.locals.insert(p, addr);
        }

        self.emit_body(body)?;

        // GenerateCode: falling off the end of a value-returning function.
        if !is_main && self.f.ret_kind != RetKind::Void && self.f.fir.cur().is_some() {
            let trap = kiln_llvm::intrinsics::trap(&mut self.m);
            let fa =
                self.m.attr_set(&[Attr::Flag(AttrKind::NoReturn), Attr::Flag(AttrKind::NoUnwind)]);
            let fn_ty = self.m.func(trap).ty();
            let opts = CallOpts {
                attrs: AttrList { func: fa, ..AttrList::default() },
                ..CallOpts::default()
            };
            let c = self.f.fir.push(
                Op::Call {
                    fn_ty,
                    callee: V::C(Value::func(trap)),
                    args: Vec::new(),
                    opts: Box::new(opts),
                },
                LTy::VOID,
            );
            self.f.calls.push((Fir::last_inst(c), Some(trap), true));
            self.f.fir.push(Op::Unreachable, LTy::VOID);
            self.f.fir.set_cur(None);
        }
        self.emit_return_block();
        self.emit_epilog();

        // Attributes.
        let nothrow_def = sig.nothrow || !self.opts.exceptions || self.no_throw_calls();
        let fa = self.def_fn_attrs(is_main, nothrow_def, self.f.mustprogress, self.f.vector_width);
        let (ret, params) = self.sig_attrs(&sig, sig.params.len(), true);
        let (fa, ret) = self.alloc_fn_attrs(d, fa, ret);
        {
            let func = self.m.func_mut(f);
            func.attrs.func = fa;
            func.attrs.ret = ret;
            func.attrs.params = params;
        }
        self.nounwind.insert(f, nothrow_def);
        if let Err(e) = self.f.fir.lower(&mut self.m, f) {
            return err(Some(ast.decls.get(d).loc()), format!("IR lowering failed: {e:?}"));
        }
        Ok(())
    }

    pub(crate) fn is_tu_level(&self, mut c: DeclId) -> bool {
        loop {
            let n = self.ast.decls.get(c);
            match n.kind() {
                DeclKind::TranslationUnit => return true,
                DeclKind::LinkageSpec => c = n.ctx().expect("ctx"),
                _ => return false,
            }
        }
    }

    /// clang `TryMarkNoThrow`: no call in the body may throw.
    fn no_throw_calls(&self) -> bool {
        self.f.calls.iter().all(|&(i, callee, site)| {
            self.f.fir.inst_block(i).is_none()
                || site
                || callee.is_some_and(|c| self.nounwind.get(&c).copied().unwrap_or(false))
        })
    }

    /// Whether `q` is evaluated as an aggregate (`hasAggregateEvaluationKind`).
    pub fn is_aggregate(&self, q: QualType) -> bool {
        let t = &self.ast.types;
        t.as_record(q).is_some() || t.is_array(q)
    }

    /// clang `EmitReturnBlock`.
    fn emit_return_block(&mut self) {
        let rb = self.f.ret_block.expect("return block");
        if let Some(cur) = self.f.fir.cur() {
            if self.f.fir.block_empty(cur) || self.f.fir.block_uses(rb) == 0 {
                self.f.fir.replace_block_uses(rb, cur);
                self.f.ret_block = None;
            } else {
                self.emit_block(rb, false);
            }
            return;
        }
        if self.f.fir.block_uses(rb) == 1 {
            let preds = self.f.fir.predecessors(rb);
            if let Some(&p) = preds.first()
                && let Some(t) = self.f.fir.terminator(p)
                && matches!(self.f.fir.op(t), Op::Br { .. })
            {
                self.f.fir.erase(t);
                self.f.fir.set_cur(Some(p));
                self.f.ret_block = None;
                return;
            }
        }
        self.emit_block(rb, false);
    }

    /// clang `EmitFunctionEpilog`.
    fn emit_epilog(&mut self) {
        let rv = match self.f.ret_kind {
            RetKind::Void | RetKind::Sret => None,
            RetKind::Scalar => {
                let slot = self.f.ret_slot.expect("retval");
                match self.dominating_store(slot) {
                    Some(st) => {
                        let Op::Store { val, .. } = *self.f.fir.op(st) else { unreachable!() };
                        self.f.fir.erase(st);
                        if self.f.fir.uses(slot.v) == 0 {
                            self.f.fir.erase(Fir::last_inst(slot.v));
                        }
                        Some(val)
                    }
                    None => Some(self.load(slot.ty, slot.v, slot.align, false)),
                }
            }
            RetKind::Coerced => {
                let slot = self.f.ret_slot.expect("retval");
                let ct = self.f.ret_coerce.clone().expect("coerce type");
                Some(self.coerced_load(slot, &ct))
            }
        };
        self.f.fir.push(Op::Ret { v: rv }, LTy::VOID);
    }

    /// clang `findDominatingStoreToReturnValue`.
    fn dominating_store(&self, slot: Addr) -> Option<u32> {
        let fir = &self.f.fir;
        let cur = fir.cur()?;
        let valid = |i: u32| match fir.op(i) {
            Op::Store { ptr, val, .. } if *ptr == slot.v && fir.ty(&self.m, *val) == slot.ty => {
                Some(i)
            }
            _ => None,
        };
        let users = fir.users(slot.v);
        if users.len() != 1 {
            let &last = fir.block_insts(cur).last()?;
            return valid(last);
        }
        let st = valid(users[0])?;
        let sb = fir.inst_block(st)?;
        let mut ip = cur;
        let mut seen = Vec::new();
        while ip != sb {
            if seen.contains(&ip) {
                return None;
            }
            seen.push(ip);
            ip = fir.single_predecessor(ip)?;
        }
        Some(st)
    }

    // ---- blocks --------------------------------------------------------

    /// clang `EmitBranch`.
    pub fn emit_branch(&mut self, target: Blk) {
        if let Some(cur) = self.f.fir.cur()
            && !self.f.fir.is_terminated(cur)
        {
            self.br(target);
        }
        self.f.fir.set_cur(None);
    }

    /// An unconditional branch at the insertion point (loop metadata
    /// attached as clang's `LoopInfoStack::InsertHelper`).
    pub fn br(&mut self, target: Blk) {
        let v = self.f.fir.push(Op::Br { dest: target }, LTy::VOID);
        self.loop_md(Fir::last_inst(v), &[target]);
    }

    /// A conditional branch (constant conditions are kept, as `IRBuilder`
    /// does).
    pub fn cond_br(&mut self, c: V, t: Blk, f: Blk) {
        let v = self.f.fir.push(Op::CondBr { c, t, f }, LTy::VOID);
        self.loop_md(Fir::last_inst(v), &[t, f]);
    }

    fn loop_md(&mut self, i: u32, succ: &[Blk]) {
        if let Some(l) = self.f.loops.last().copied()
            && let Some(md) = l.md
            && succ.contains(&l.header)
        {
            let k = self.m.md_kind("llvm.loop");
            self.f.fir.add_md(i, k, md);
        }
    }

    /// clang `EmitBlock`.
    pub fn emit_block(&mut self, b: Blk, finished: bool) {
        let cur = self.f.fir.cur();
        self.emit_branch(b);
        if finished && self.f.fir.block_uses(b) == 0 {
            return;
        }
        match cur.filter(|&c| self.f.fir.is_placed(c)) {
            Some(c) => self.f.fir.place_after(c, b),
            None => self.f.fir.place_at_end(b),
        }
        self.f.fir.set_cur(Some(b));
    }

    /// clang `EnsureInsertPoint`.
    pub fn ensure_insert_point(&mut self) {
        if self.f.fir.cur().is_none() {
            let b = self.f.fir.create_block();
            self.emit_block(b, false);
        }
    }

    /// clang `SimplifyForwardingBlocks`.
    pub fn simplify_forwarding(&mut self, b: Blk) {
        let insts = self.f.fir.block_insts(b);
        if insts.len() != 1 {
            return;
        }
        let i = insts[0];
        let Op::Br { dest } = *self.f.fir.op(i) else { return };
        if dest == b {
            return;
        }
        self.f.fir.erase(i);
        self.f.fir.replace_block_uses(b, dest);
        self.f.fir.erase_block(b);
    }

    // ---- memory --------------------------------------------------------

    /// A local temporary (`CreateMemTemp`).
    pub fn temp(&mut self, q: QualType) -> Addr {
        let ty = self.mty(q);
        let (_, align) = self.size_align(q);
        let v = self.f.fir.alloca(ty, align);
        Addr { v, ty, align }
    }

    pub fn load(&mut self, ty: LTy, ptr: V, align: u64, volatile: bool) -> V {
        self.f.fir.push(Op::Load { ty, ptr, mem: mem(align, volatile) }, ty)
    }

    pub fn store(&mut self, val: V, ptr: V, align: u64, volatile: bool) {
        self.f.fir.push(Op::Store { val, ptr, mem: mem(align, volatile) }, LTy::VOID);
    }

    /// clang `EmitLoadOfScalar` (bools are truncated to `i1`).
    pub fn load_scalar(&mut self, a: Addr, q: QualType) -> V {
        let vol = self.ast.types.effective_quals(q).is_volatile();
        let v = self.load(a.ty, a.v, a.align, vol);
        if self.is_bool(q) { self.cast(CastOp::Trunc, v, LTy::I1) } else { v }
    }

    /// clang `EmitStoreOfScalar` (bools are zero-extended to memory).
    pub fn store_scalar(&mut self, v: V, a: Addr, q: QualType) {
        let vol = self.ast.types.effective_quals(q).is_volatile();
        let v = if self.is_bool(q) && self.f.fir.ty(&self.m, v) == LTy::I1 {
            self.cast(CastOp::ZExt, v, a.ty)
        } else {
            v
        };
        self.store(v, a.v, a.align, vol);
    }

    /// `memcpy(dst, src, n)`.
    pub fn memcpy(&mut self, dst: Addr, src: V, src_align: u64, n: u64, volatile: bool) {
        let f = kiln_llvm::intrinsics::memcpy(&mut self.m, LTy::I64);
        let fn_ty = self.m.func(f).ty();
        let n = self.m.const_int(LTy::I64, i128::from(n));
        let vb = self.m.const_bool(volatile);
        let pa = self.m.attr_set(&[Attr::Align(dst.align)]);
        let pb = self.m.attr_set(&[Attr::Align(src_align)]);
        let e = self.m.attr_set(&[]);
        let opts = CallOpts {
            attrs: AttrList { params: vec![pa, pb, e, e], ..AttrList::default() },
            ..CallOpts::default()
        };
        let c = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(f)),
                args: vec![dst.v, src, V::C(n), V::C(vb)],
                opts: Box::new(opts),
            },
            LTy::VOID,
        );
        let _ = (c, src);
        self.f.calls.push((Fir::last_inst(c), Some(f), true));
    }

    /// `memcpy(dst, src, n)` with an instruction source.
    pub fn memcpy_v(&mut self, dst: Addr, src: V, src_align: u64, n: u64) {
        let f = kiln_llvm::intrinsics::memcpy(&mut self.m, LTy::I64);
        let fn_ty = self.m.func(f).ty();
        let nv = self.m.const_int(LTy::I64, i128::from(n));
        let vb = self.m.const_bool(false);
        let pa = self.m.attr_set(&[Attr::Align(dst.align)]);
        let pb = self.m.attr_set(&[Attr::Align(src_align)]);
        let e = self.m.attr_set(&[]);
        let opts = CallOpts {
            attrs: AttrList { params: vec![pa, pb, e, e], ..AttrList::default() },
            ..CallOpts::default()
        };
        let c = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(f)),
                args: vec![dst.v, src, V::C(nv), V::C(vb)],
                opts: Box::new(opts),
            },
            LTy::VOID,
        );
        self.f.calls.push((Fir::last_inst(c), Some(f), true));
    }

    /// `memset(dst, byte, n)`.
    pub fn memset(&mut self, dst: Addr, byte: u8, n: u64, volatile: bool) {
        let nv = self.m.const_int(LTy::I64, i128::from(n));
        self.memset_v(dst, byte, V::C(nv), volatile);
    }

    /// `memset(dst, byte, n)` with a computed length.
    pub fn memset_v(&mut self, dst: Addr, byte: u8, n: V, volatile: bool) {
        let f = kiln_llvm::intrinsics::memset(&mut self.m, LTy::I64);
        let fn_ty = self.m.func(f).ty();
        let bv = self.m.const_int(LTy::I8, i128::from(byte));
        let vb = self.m.const_bool(volatile);
        let pa = self.m.attr_set(&[Attr::Align(dst.align)]);
        let e = self.m.attr_set(&[]);
        let opts = CallOpts {
            attrs: AttrList { params: vec![pa, e, e, e], ..AttrList::default() },
            ..CallOpts::default()
        };
        let c = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(f)),
                args: vec![dst.v, V::C(bv), n, V::C(vb)],
                opts: Box::new(opts),
            },
            LTy::VOID,
        );
        self.f.calls.push((Fir::last_inst(c), Some(f), true));
    }

    /// One argument of clang `EmitFunctionProlog`: the parameter's value
    /// (scalars) or address (aggregates and coerced scalars' memory).
    fn prolog_arg(
        &mut self,
        kind: &ArgKind,
        pq: QualType,
        arg_no: &mut u32,
        loc: Option<Loc>,
    ) -> R<ParamVal> {
        let mt = self.mty(pq);
        let agg = self.is_aggregate(pq);
        match kind {
            ArgKind::Ignore => {
                if agg {
                    let (_, align) = self.size_align(pq);
                    let a = self.f.fir.alloca(mt, align);
                    Ok(ParamVal::Indirect(Addr { v: a, ty: mt, align }, false))
                } else {
                    err(loc, "an ignored scalar parameter is not supported yet")
                }
            }
            ArgKind::Indirect { byval, align } => {
                let addr = Addr { v: V::Arg(*arg_no), ty: mt, align: *align };
                *arg_no += 1;
                if agg {
                    Ok(ParamVal::Indirect(addr, !*byval))
                } else {
                    Ok(ParamVal::Direct(self.load_scalar(addr, pq)))
                }
            }
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                let offset = if let ArgKind::Direct { offset, .. } = kind { *offset } else { 0 };
                let ct = self.ir(coerce);
                let is_struct = matches!(coerce, IrType::Struct(..));
                if !agg && !is_struct && ct == self.lty(pq) && offset == 0 {
                    let v = V::Arg(*arg_no);
                    *arg_no += 1;
                    return Ok(ParamVal::Direct(v));
                }
                if offset != 0 {
                    return err(loc, "an argument at a nonzero ABI offset is not supported yet");
                }
                let align = self.global_align(pq);
                let a = self.f.fir.alloca(mt, align);
                let alloca = Addr { v: a, ty: mt, align };
                match kind {
                    ArgKind::Direct {
                        coerce: IrType::Struct(fs, packed), flatten: true, ..
                    } if fs.len() > 1 => {
                        let src_size = self.alloc_size(ct);
                        let dst_size = self.alloc_size(mt);
                        let into = if src_size <= dst_size {
                            alloca.v
                        } else {
                            self.f.fir.alloca(ct, alloca.align)
                        };
                        let offs = self.tys.struct_offsets(fs, *packed);
                        for (j, off) in offs.iter().enumerate() {
                            let p = self.struct_gep(ct, into, j as u32);
                            self.store(V::Arg(*arg_no), p, min_align(alloca.align, *off), false);
                            *arg_no += 1;
                        }
                        if src_size > dst_size {
                            self.memcpy_v(alloca, into, alloca.align, dst_size);
                        }
                    }
                    _ => {
                        let (size, _) = self.size_align(pq);
                        self.coerced_store_sized(V::Arg(*arg_no), coerce, alloca, size);
                        *arg_no += 1;
                    }
                }
                if agg {
                    Ok(ParamVal::Indirect(alloca, false))
                } else {
                    Ok(ParamVal::Direct(self.load_scalar(alloca, pq)))
                }
            }
        }
    }

    /// The IR arguments for one call argument (clang `EmitCall`'s
    /// per-argument lowering), appended to `out`.
    pub(crate) fn call_arg(
        &mut self,
        kind: &ArgKind,
        q: QualType,
        arg: CallArg,
        out: &mut Vec<V>,
        loc: Option<Loc>,
    ) -> R<()> {
        match kind {
            ArgKind::Ignore => Ok(()),
            ArgKind::Indirect { byval, align } => {
                match arg {
                    CallArg::Agg(a) => {
                        if *byval && a.align < *align {
                            let got = self.f.fir.enforce_alloca_align(a.v, *align);
                            if got.is_none_or(|g| g < *align) {
                                return err(
                                    loc,
                                    "copying an under-aligned byval argument is not supported yet",
                                );
                            }
                        }
                        out.push(a.v);
                    }
                    CallArg::Scalar(v) => {
                        let mt = self.mty(q);
                        let t = self.f.fir.alloca(mt, *align);
                        self.store_scalar(v, Addr { v: t, ty: mt, align: *align }, q);
                        out.push(t);
                    }
                }
                Ok(())
            }
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                let offset = if let ArgKind::Direct { offset, .. } = kind { *offset } else { 0 };
                let ct = self.ir(coerce);
                let is_struct = matches!(coerce, IrType::Struct(..));
                let src = match arg {
                    CallArg::Scalar(v) => {
                        if !is_struct && ct == self.lty(q) && offset == 0 {
                            out.push(v);
                            return Ok(());
                        }
                        let t = self.temp(q);
                        self.store_scalar(v, t, q);
                        t
                    }
                    CallArg::Agg(a) => a,
                };
                if offset != 0 {
                    return err(loc, "an argument at a nonzero ABI offset is not supported yet");
                }
                if let ArgKind::Direct {
                    coerce: IrType::Struct(fs, packed), flatten: true, ..
                } = kind
                {
                    let src_size = self.alloc_size(src.ty);
                    let dst_size = self.alloc_size(ct);
                    let from = if src_size < dst_size {
                        let t = self.f.fir.alloca(ct, src.align);
                        self.memcpy_v(
                            Addr { v: t, ty: ct, align: src.align },
                            src.v,
                            src.align,
                            src_size,
                        );
                        t
                    } else {
                        src.v
                    };
                    let offs = self.tys.struct_offsets(fs, *packed);
                    for (j, f) in fs.iter().enumerate() {
                        let ft = self.ir(f);
                        let p = self.struct_gep(ct, from, j as u32);
                        let v = self.load(ft, p, min_align(src.align, offs[j]), false);
                        out.push(v);
                    }
                } else {
                    let v = self.coerced_load(src, coerce);
                    out.push(v);
                }
                Ok(())
            }
        }
    }

    /// clang `CreateCoercedLoad` from `src` to the ABI type `ct`.
    pub fn coerced_load(&mut self, src: Addr, ct: &IrType) -> V {
        let dt = self.ir(ct);
        if src.ty == dt {
            return self.load(dt, src.v, src.align, false);
        }
        let (dsize, _) = self.tys.size_align(ct);
        let src = self.enter_struct_for_coerce(src, dsize);
        let is_ip = |m: &kiln_llvm::Module, t: LTy| m.types.is_int(t) || m.types.is_ptr(t);
        if is_ip(&self.m, dt) && is_ip(&self.m, src.ty) {
            let v = self.load(src.ty, src.v, src.align, false);
            return self.coerce_int_ptr(v, src.ty, dt);
        }
        let ssize = self.alloc_size(src.ty);
        if ssize >= dsize {
            return self.load(dt, src.v, src.align, false);
        }
        let (_, dalign) = self.tys.size_align(ct);
        let tmp_align = dalign.max(src.align);
        let tmp = self.f.fir.alloca(dt, tmp_align);
        self.memcpy_v(Addr { v: tmp, ty: dt, align: tmp_align }, src.v, src.align, ssize);
        self.load(dt, tmp, tmp_align, false)
    }

    /// clang 19 `CreateCoercedStore(Src, Dst, DstSize)` as used by
    /// `EmitFunctionProlog`: `dst_size` is the size of the object, not of
    /// its IR type, and a wider integer is truncated to it.
    pub fn coerced_store_sized(&mut self, v: V, ct: &IrType, dst: Addr, dst_size: u64) {
        if dst_size == 0 {
            return;
        }
        let st = self.ir(ct);
        let (ssize, salign) = self.tys.size_align(ct);
        let mut dst = dst;
        if st != dst.ty {
            dst = self.enter_struct_for_coerce(dst, ssize);
        }
        if ssize <= dst_size {
            if self.m.types.is_int(st)
                && self.m.types.is_ptr(dst.ty)
                && ssize == self.alloc_size(dst.ty)
            {
                let c = self.coerce_int_ptr(v, st, dst.ty);
                self.store(c, dst.v, dst.align, false);
            } else {
                self.aggregate_store(v, ct, dst);
            }
        } else if self.m.types.is_int(st) {
            let it = self.m.types.int(u32::try_from(dst_size * 8).unwrap_or(u32::MAX));
            let c = self.coerce_int_ptr(v, st, it);
            self.store(c, dst.v, dst.align, false);
        } else {
            let a = salign.max(dst.align);
            let tmp = self.f.fir.alloca(st, a);
            self.store(v, tmp, a, false);
            self.memcpy_v(dst, tmp, a, dst_size);
        }
    }

    /// clang `EmitAggregateStore`: element-wise for first-class structs.
    pub(crate) fn aggregate_store(&mut self, v: V, ct: &IrType, dst: Addr) {
        let st = self.ir(ct);
        let Some(fields) = self.struct_fields(st) else {
            self.store(v, dst.v, dst.align, false);
            return;
        };
        let offs = self.struct_field_offsets(st);
        for (i, &ft) in fields.iter().enumerate() {
            let p = self.struct_gep(st, dst.v, i as u32);
            let e = self.f.fir.push(Op::ExtractValue { agg: v, idx: i as u32 }, ft);
            let al = min_align(dst.align, offs[i]);
            self.store(e, p, al, false);
        }
    }

    /// `StructLayout::getElementOffset` for every field of struct type `t`.
    pub fn struct_field_offsets(&self, t: LTy) -> Vec<u64> {
        let ty = &self.m.types;
        let Some(fields) = ty.struct_fields(t) else { return Vec::new() };
        let packed = ty.struct_is_packed(t);
        let mut off = 0u64;
        let mut v = Vec::with_capacity(fields.len());
        for &f in fields {
            let (s, a) = self.ir_layout(f);
            let a = if packed { 1 } else { a };
            off = off.div_ceil(a) * a;
            v.push(off);
            off += s;
        }
        v
    }

    fn coerce_int_ptr(&mut self, v: V, from: LTy, to: LTy) -> V {
        if from == to {
            return v;
        }
        let mut v = v;
        let mut from = from;
        if self.m.types.is_ptr(from) {
            if self.m.types.is_ptr(to) {
                return v;
            }
            v = self.cast(CastOp::PtrToInt, v, LTy::I64);
            from = LTy::I64;
        }
        let dest_int = if self.m.types.is_ptr(to) { LTy::I64 } else { to };
        if from != dest_int {
            let fw = self.m.types.int_width(from).unwrap_or(64);
            let tw = self.m.types.int_width(dest_int).unwrap_or(64);
            v = if fw > tw {
                self.cast(CastOp::Trunc, v, dest_int)
            } else {
                self.cast(CastOp::ZExt, v, dest_int)
            };
        }
        if self.m.types.is_ptr(to) {
            v = self.cast(CastOp::IntToPtr, v, to);
        }
        v
    }

    /// clang `EnterStructPointerForCoercedAccess`.
    fn enter_struct_for_coerce(&mut self, a: Addr, want: u64) -> Addr {
        let Some(fields) = self.struct_fields(a.ty) else { return a };
        if fields.is_empty() {
            return a;
        }
        let first = fields[0];
        let fs = self.store_size(first);
        if fs < want && fs < self.store_size(a.ty) {
            return a;
        }
        let p = self.struct_gep(a.ty, a.v, 0);
        self.enter_struct_for_coerce(Addr { v: p, ty: first, align: a.align }, want)
    }

    /// The field types of a struct type.
    pub fn struct_fields(&self, t: LTy) -> Option<Vec<LTy>> {
        self.m.types.struct_fields(t).map(<[LTy]>::to_vec)
    }

    /// The data-layout (size, ABI alignment) of an IR type. Both
    /// supported targets agree: pointers and `i64`/`double` are 8-aligned,
    /// `i128`, `fp128` and `x86_fp80` 16-aligned.
    pub fn ir_layout(&self, t: LTy) -> (u64, u64) {
        use kiln_llvm::TypeData as D;
        let ty = &self.m.types;
        if let Some(w) = ty.int_width(t) {
            let bytes = u64::from(w).div_ceil(8);
            let align = bytes.next_power_of_two().min(16);
            return (bytes.div_ceil(align) * align, align);
        }
        if let Some((e, n)) = ty.array_info(t) {
            let (s, a) = self.ir_layout(e);
            return (s * n, a);
        }
        if let Some(fields) = ty.struct_fields(t) {
            let packed = ty.struct_is_packed(t);
            let (mut off, mut align) = (0u64, 1u64);
            for &f in fields {
                let (s, a) = self.ir_layout(f);
                let a = if packed { 1 } else { a };
                off = off.div_ceil(a) * a + s;
                align = align.max(a);
            }
            return (off.div_ceil(align) * align, align);
        }
        match ty.data(t) {
            D::Half | D::BFloat => (2, 2),
            D::Float => (4, 4),
            D::Double | D::Ptr(_) => (8, 8),
            D::Fp128 | D::X86Fp80 => (16, 16),
            D::Vector { len, elem, .. } => {
                let (s, _) = self.ir_layout(*elem);
                let bytes = (s * u64::from(*len)).next_power_of_two();
                (bytes, bytes.min(16))
            }
            _ => (0, 1),
        }
    }

    /// `DataLayout::getTypeAllocSize`.
    pub fn alloc_size(&self, t: LTy) -> u64 {
        self.ir_layout(t).0
    }

    /// `DataLayout::getTypeStoreSize`.
    pub fn store_size(&self, t: LTy) -> u64 {
        if let Some(w) = self.m.types.int_width(t) {
            return u64::from(w).div_ceil(8);
        }
        if self.m.types.float_width(t) == Some(80) {
            return 10;
        }
        self.ir_layout(t).0
    }

    /// `getelementptr inbounds %T, ptr %p, i32 0, i32 i`.
    pub fn struct_gep(&mut self, t: LTy, p: V, i: u32) -> V {
        let z = self.m.const_int(LTy::I32, 0);
        let iv = self.m.const_int(LTy::I32, i128::from(i));
        self.gep(t, p, vec![V::C(z), V::C(iv)], true)
    }

    // ---- instructions with IRBuilder constant folding --------------------

    /// The integer constant `v`, if it is one: (type, zero-extended bits, width).
    pub fn cint(&self, v: V) -> Option<(LTy, u128, u32)> {
        let V::C(c) = v else { return None };
        match self.m.const_of(c)? {
            ConstData::Int { ty, value: IntValue::Small(x) } => {
                Some((*ty, *x, self.m.types.int_width(*ty).unwrap_or(0)))
            }
            _ => None,
        }
    }

    fn cfloat(&self, v: V) -> Option<(LTy, f64)> {
        let V::C(c) = v else { return None };
        match self.m.const_of(c)? {
            ConstData::Float { ty, bits } if *ty == LTy::DOUBLE => {
                Some((*ty, f64::from_bits(*bits as u64)))
            }
            ConstData::Float { ty, bits } if *ty == LTy::FLOAT => {
                Some((*ty, f64::from(f32::from_bits(*bits as u32))))
            }
            _ => None,
        }
    }

    fn mk_float(&mut self, ty: LTy, x: f64) -> V {
        let bits = if ty == LTy::FLOAT {
            u128::from((x as f32).to_bits())
        } else {
            u128::from(x.to_bits())
        };
        V::C(self.m.const_float_bits(ty, bits))
    }

    pub fn bin(&mut self, op: BinOp, flags: Flags, l: V, r: V) -> V {
        if let (Some((ty, a, w)), Some((_, b, _))) = (self.cint(l), self.cint(r))
            && let Some(x) = fold_int_bin(op, a, b, w)
        {
            return V::C(self.m.const_int(ty, x as i128));
        }
        if let (Some((ty, a)), Some((_, b))) = (self.cfloat(l), self.cfloat(r)) {
            let single = ty == LTy::FLOAT;
            let x = match op {
                BinOp::FAdd => Some(if single { f64::from(a as f32 + b as f32) } else { a + b }),
                BinOp::FSub => Some(if single { f64::from(a as f32 - b as f32) } else { a - b }),
                BinOp::FMul => Some(if single { f64::from(a as f32 * b as f32) } else { a * b }),
                BinOp::FDiv => Some(if single { f64::from(a as f32 / b as f32) } else { a / b }),
                _ => None,
            };
            if let Some(x) = x {
                return self.mk_float(ty, x);
            }
        }
        let ty = self.f.fir.ty(&self.m, l);
        self.f.fir.push(Op::Bin { op, flags, l, r }, ty)
    }

    pub fn icmp(&mut self, p: IntPredicate, l: V, r: V) -> V {
        if let (Some((_, a, w)), Some((_, b, _))) = (self.cint(l), self.cint(r)) {
            let sa = sext(a, w);
            let sb = sext(b, w);
            let x = match p {
                IntPredicate::Eq => a == b,
                IntPredicate::Ne => a != b,
                IntPredicate::Ugt => a > b,
                IntPredicate::Uge => a >= b,
                IntPredicate::Ult => a < b,
                IntPredicate::Ule => a <= b,
                IntPredicate::Sgt => sa > sb,
                IntPredicate::Sge => sa >= sb,
                IntPredicate::Slt => sa < sb,
                IntPredicate::Sle => sa <= sb,
            };
            return V::C(self.m.const_bool(x));
        }
        self.f.fir.push(Op::ICmp { p, l, r }, LTy::I1)
    }

    #[allow(clippy::float_cmp, reason = "constant folding is exact IEEE comparison")]
    pub fn fcmp(&mut self, p: FloatPredicate, l: V, r: V) -> V {
        if let (Some((_, a)), Some((_, b))) = (self.cfloat(l), self.cfloat(r)) {
            let uno = a.is_nan() || b.is_nan();
            let x = match p {
                FloatPredicate::Oeq => !uno && a == b,
                FloatPredicate::One => !uno && a != b,
                FloatPredicate::Ogt => !uno && a > b,
                FloatPredicate::Oge => !uno && a >= b,
                FloatPredicate::Olt => !uno && a < b,
                FloatPredicate::Ole => !uno && a <= b,
                FloatPredicate::Une => uno || a != b,
                FloatPredicate::Ueq => uno || a == b,
                _ => return self.f.fir.push(Op::FCmp { p, l, r }, LTy::I1),
            };
            return V::C(self.m.const_bool(x));
        }
        self.f.fir.push(Op::FCmp { p, l, r }, LTy::I1)
    }

    pub fn fneg(&mut self, v: V) -> V {
        if let Some((ty, a)) = self.cfloat(v) {
            return self.mk_float(ty, -a);
        }
        let ty = self.f.fir.ty(&self.m, v);
        self.f.fir.push(Op::FNeg { v }, ty)
    }

    #[allow(clippy::cast_precision_loss, reason = "int-to-float folds round like LLVM")]
    pub fn cast(&mut self, op: CastOp, v: V, to: LTy) -> V {
        let from = self.f.fir.ty(&self.m, v);
        if from == to && matches!(op, CastOp::BitCast | CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
        {
            return v;
        }
        if let Some((_, a, w)) = self.cint(v) {
            let tw = self.m.types.int_width(to);
            let r = match (op, tw) {
                (CastOp::Trunc | CastOp::ZExt, Some(_)) => {
                    Some(V::C(self.m.const_int(to, a as i128)))
                }
                (CastOp::SExt, Some(_)) => Some(V::C(self.m.const_int(to, sext(a, w)))),
                // Rust's int-to-float `as` rounds to nearest even, like
                // LLVM's folder; `float` is rounded once, not via `double`.
                (CastOp::SIToFP, None) if to == LTy::DOUBLE => {
                    Some(self.mk_float(to, sext(a, w) as f64))
                }
                (CastOp::UIToFP, None) if to == LTy::DOUBLE => Some(self.mk_float(to, a as f64)),
                (CastOp::SIToFP, None) if to == LTy::FLOAT => {
                    Some(self.mk_float(to, f64::from(sext(a, w) as f32)))
                }
                (CastOp::UIToFP, None) if to == LTy::FLOAT => {
                    Some(self.mk_float(to, f64::from(a as f32)))
                }
                (CastOp::SIToFP | CastOp::UIToFP, None)
                    if self.m.types.float_width(to) == Some(128) =>
                {
                    let neg = op == CastOp::SIToFP && sext(a, w) < 0;
                    let mag = if neg { sext(a, w).unsigned_abs() } else { a };
                    Some(V::C(self.m.const_float_bits(to, int_to_f128_bits(neg, mag))))
                }
                (CastOp::SIToFP | CastOp::UIToFP, None)
                    if self.m.types.float_width(to) == Some(80) =>
                {
                    let neg = op == CastOp::SIToFP && sext(a, w) < 0;
                    let mag = if neg { sext(a, w).unsigned_abs() } else { a };
                    // Exact (64-bit significand) up to 2^64; wider values
                    // would need rounding and are left to LLVM.
                    u64::try_from(mag)
                        .ok()
                        .map(|m| V::C(self.m.const_float_bits(to, int_to_x87_bits(neg, m))))
                }
                _ => None,
            };
            if let Some(r) = r {
                return r;
            }
        }
        if let Some((fty, x)) = self.cfloat(v) {
            let r = match op {
                CastOp::FPExt | CastOp::FPTrunc if to == LTy::DOUBLE || to == LTy::FLOAT => {
                    let _ = fty;
                    Some(self.mk_float(to, x))
                }
                CastOp::FPToSI if x.is_finite() => {
                    Some(V::C(self.m.const_int(to, x.trunc() as i128)))
                }
                CastOp::FPToUI if x.is_finite() && x >= 0.0 => {
                    Some(V::C(self.m.const_int(to, x.trunc() as i128)))
                }
                _ => None,
            };
            if let Some(r) = r {
                return r;
            }
        }
        if let V::C(c) = v
            && matches!(op, CastOp::PtrToInt | CastOp::IntToPtr)
            && let Ok(x) = self.m.const_cast(op, c, to)
        {
            return V::C(x);
        }
        self.f.fir.push(Op::Cast { op, flags: Flags::NONE, v, to }, to)
    }

    /// Integer cast by signedness (`CreateIntCast`).
    pub fn int_cast(&mut self, v: V, to: LTy, signed: bool) -> V {
        let from = self.f.fir.ty(&self.m, v);
        let fw = self.m.types.int_width(from).unwrap_or(0);
        let tw = self.m.types.int_width(to).unwrap_or(0);
        if fw == tw {
            v
        } else if fw > tw {
            self.cast(CastOp::Trunc, v, to)
        } else if signed {
            self.cast(CastOp::SExt, v, to)
        } else {
            self.cast(CastOp::ZExt, v, to)
        }
    }

    pub fn gep(&mut self, src: LTy, base: V, idx: Vec<V>, inbounds: bool) -> V {
        let flags = if inbounds { GepFlags::INBOUNDS } else { GepFlags::NONE };
        if let V::C(b) = base
            && idx.iter().all(|i| matches!(i, V::C(_)))
        {
            if idx.iter().all(|&i| self.cint(i).is_some_and(|x| x.1 == 0)) {
                return base;
            }
            let iv: Vec<Value> =
                idx.iter().map(|i| if let V::C(c) = i { *c } else { unreachable!() }).collect();
            if let Ok(c) = self.m.const_gep(src, b, &iv, flags, None) {
                return V::C(c);
            }
        }
        self.f.fir.push(Op::Gep { src, base, idx, flags }, LTy::PTR)
    }

    pub fn select(&mut self, c: V, t: V, f: V) -> V {
        if let Some((_, x, _)) = self.cint(c) {
            return if x != 0 { t } else { f };
        }
        let ty = self.f.fir.ty(&self.m, t);
        self.f.fir.push(Op::Select { c, t, f }, ty)
    }
}

/// Folds an integer binary operation of width `w` (`None`: not folded).
fn fold_int_bin(op: BinOp, a: u128, b: u128, w: u32) -> Option<u128> {
    let mask = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
    let sa = sext(a, w);
    let sb = sext(b, w);
    let r = match op {
        BinOp::Add => a.wrapping_add(b),
        BinOp::Sub => a.wrapping_sub(b),
        BinOp::Mul => a.wrapping_mul(b),
        BinOp::And => a & b,
        BinOp::Or => a | b,
        BinOp::Xor => a ^ b,
        BinOp::UDiv if b != 0 => a / b,
        BinOp::URem if b != 0 => a % b,
        BinOp::SDiv if sb != 0 && !(sb == -1 && sa == sext(1u128 << (w - 1), w)) => {
            (sa / sb) as u128
        }
        BinOp::SRem if sb != 0 && !(sb == -1 && sa == sext(1u128 << (w - 1), w)) => {
            (sa % sb) as u128
        }
        BinOp::Shl if b < u128::from(w) => a << b,
        BinOp::LShr if b < u128::from(w) => a >> b,
        BinOp::AShr if b < u128::from(w) => (sa >> b) as u128,
        _ => return None,
    };
    Some(r & mask)
}

/// The `x86_fp80` bits (sign and biased exponent in bits 64..80, explicit
/// integer bit at 63) of the exactly representable integer `mag`.
fn int_to_x87_bits(neg: bool, mag: u64) -> u128 {
    if mag == 0 {
        return 0;
    }
    let n = mag.ilog2();
    let exp = 16383 + u128::from(n);
    let sign = if neg { 1u128 << 79 } else { 0 };
    sign | (exp << 64) | u128::from(mag << (63 - n))
}

/// The IEEE binary128 encoding of `±mag` (round to nearest, ties to even).
fn int_to_f128_bits(neg: bool, mag: u128) -> u128 {
    if mag == 0 {
        return 0;
    }
    let sign = u128::from(neg) << 127;
    let e = mag.ilog2();
    let frac_mask = (1u128 << 112) - 1;
    if e <= 112 {
        let m = mag << (112 - e);
        return sign | (u128::from(e + 16383) << 112) | (m & frac_mask);
    }
    let shift = e - 112;
    let mut m = mag >> shift;
    let rem = mag & ((1u128 << shift) - 1);
    let half = 1u128 << (shift - 1);
    let mut e = e;
    if rem > half || (rem == half && m & 1 == 1) {
        m += 1;
        if m >> 113 != 0 {
            m >>= 1;
            e += 1;
        }
    }
    sign | (u128::from(e + 16383) << 112) | (m & frac_mask)
}

/// Sign-extends the low `w` bits of `a`.
pub(crate) fn sext(a: u128, w: u32) -> i128 {
    if w == 0 || w >= 128 {
        return a as i128;
    }
    let s = 128 - w;
    ((a << s) as i128) >> s
}

/// The alignment of `align`-aligned memory at byte offset `off`.
pub(crate) fn min_align(align: u64, off: u64) -> u64 {
    if off == 0 { align } else { align.min(1u64 << off.trailing_zeros()) }
}
