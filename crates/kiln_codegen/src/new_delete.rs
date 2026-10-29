//! `new` and `delete` expressions (spec/codegen.md §3.1): clang's
//! `EmitCXXNewExpr`, `EmitCXXNewAllocSize`, `EmitNewArrayInitializer`,
//! `EmitCXXDeleteExpr` and `EmitDeleteCall` at `-O0`, for non-class and
//! aggregate element types (no array cookies, constructors or
//! destructors).

use kiln_abi::mangle;
use kiln_ast::decl::{DeclKind, NamespaceData, TagKind};
use kiln_ast::expr::{CastKind, ExprFlags, ExprKind, NewExtra, UnaryOp};
use kiln_ast::names::OverloadedOperator;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_llvm::intrinsics::OverflowOp;
use kiln_llvm::{
    Attr, AttrKind, AttrList, AttrSetId, BinOp, CallOpts, Flags, IntPredicate, MemOpts,
    TypeId as LTy, Value,
};
use kiln_source::Loc;

use crate::expr::LV;
use crate::fir::{Fir, Op, V};
use crate::func::{Addr, CallArg, min_align};
use crate::module::{Cg, R, err};

/// What clang knows about a global allocation or deallocation function
/// (`isReplaceableGlobalAllocationFunction`,
/// `isReservedGlobalPlacementOperator`).
#[derive(Clone, Copy, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent facts, as clang's predicates")]
pub(crate) struct AllocFn {
    /// `operator new` / `operator new[]`.
    pub is_new: bool,
    /// A replaceable global allocation or deallocation function.
    pub replaceable: bool,
    /// The reserved placement form `(size_t, void*)` / `(void*, void*)`.
    pub reserved: bool,
    /// Replaceable, with a `const std::nothrow_t&` parameter.
    pub nothrow_t: bool,
    /// Replaceable, with a `std::align_val_t` parameter at this index.
    pub align_param: Option<usize>,
}

impl Cg<'_> {
    /// The allocation-function facts of `d`, if it is a global
    /// `operator new`/`new[]`/`delete`/`delete[]`.
    pub(crate) fn alloc_fn(&self, d: DeclId) -> Option<AllocFn> {
        let ast = self.ast;
        let t = &ast.types;
        let n = ast.decls.get(d);
        if n.kind() != DeclKind::Function {
            return None;
        }
        let is_new = match n.name().as_operator()? {
            OverloadedOperator::New | OverloadedOperator::ArrayNew => true,
            OverloadedOperator::Delete | OverloadedOperator::ArrayDelete => false,
            _ => return None,
        };
        if !n.ctx().is_some_and(|c| self.is_tu_level(c)) {
            return None;
        }
        let p = *t.proto(n.ty()?)?;
        let params = t.params(&p);
        let variadic = p.variadic();
        let mut a = AllocFn { is_new, ..AllocFn::default() };
        a.reserved = params.len() == 2
            && !variadic
            && t.canonical(params[1]).quals_bits() == 0
            && matches!(t.canonical_view(params[1]),
                TypeView::Pointer(pq) if t.canonical(pq) == QualType::unqual(kiln_ast::TypeId::VOID));
        if variadic || params.is_empty() || params.len() > 4 {
            return Some(a);
        }
        if params.len() == 1 {
            a.replaceable = true;
            return Some(a);
        }
        let at = |i: usize| params.get(i).copied();
        let mut i = 1;
        let mut sized = false;
        let size_t = t.canonical(QualType::unqual(t.target().size_t));
        if !is_new && at(i).is_some_and(|q| t.canonical(q) == size_t) {
            sized = true;
            i += 1;
        }
        if at(i).is_some_and(|q| self.is_std_enum(q, "align_val_t")) {
            a.align_param = Some(i);
            i += 1;
        }
        if !sized
            && let Some(q) = at(i)
            && t.is_reference(q)
        {
            let pt = t.canonical(t.pointee(q)?);
            if pt.quals_bits() != 1 {
                return Some(a);
            }
            if t.as_record(pt).is_some_and(|r| self.in_std(r, "nothrow_t")) {
                a.nothrow_t = true;
                i += 1;
            }
        }
        if !sized
            && let Some(q) = at(i)
            && let Some(ed) = t.as_enum(q)
            && self.decl_named(ed, "__hot_cold_t")
        {
            i += 1;
        }
        a.replaceable = i == params.len();
        if !a.replaceable {
            a.nothrow_t = false;
            a.align_param = None;
        }
        Some(a)
    }

    fn decl_named(&self, d: DeclId, name: &str) -> bool {
        let ast = self.ast;
        ast.decls.get(d).name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == name)
    }

    /// `d` is named `name` and declared in namespace `std` (clang
    /// `isInStdNamespace`: through inline namespaces).
    fn in_std(&self, d: DeclId, name: &str) -> bool {
        let decls = &self.ast.decls;
        if !self.decl_named(d, name) {
            return false;
        }
        let mut ns = decls.get(d).ctx();
        while let Some(n) = ns
            && decls.namespace(n).is_some_and(NamespaceData::is_inline)
        {
            ns = decls.get(n).ctx();
        }
        ns.is_some_and(|n| {
            decls.get(n).kind() == DeclKind::Namespace
                && self.decl_named(n, "std")
                && decls.get(n).ctx().is_some_and(|c| self.is_tu_level(c))
        })
    }

    /// `q` is the enumeration `std::<name>` (clang `isAlignValT`).
    fn is_std_enum(&self, q: QualType, name: &str) -> bool {
        self.ast.types.as_enum(q).is_some_and(|e| self.in_std(e, name))
    }

    /// The attributes of a replaceable allocation function's declaration
    /// or definition: `nobuiltin` (clang `SetFunctionAttributes`), and for
    /// `operator new` the implicit `alloc_size` and `returns_nonnull`
    /// (`AddKnownFunctionAttributesForReplaceableGlobalAllocationFunction`).
    pub(crate) fn alloc_fn_attrs(
        &mut self,
        d: DeclId,
        func: AttrSetId,
        ret: AttrSetId,
    ) -> (AttrSetId, AttrSetId) {
        let Some(af) = self.alloc_fn(d).filter(|a| a.replaceable) else { return (func, ret) };
        let mut f = self.m.attrs_of(func).to_vec();
        f.push(Attr::Flag(AttrKind::NoBuiltin));
        let mut r = self.m.attrs_of(ret).to_vec();
        if af.is_new {
            f.push(Attr::AllocSize(0, None));
            if !af.nothrow_t {
                r.push(Attr::Flag(AttrKind::NonNull));
            }
        }
        (self.m.attr_set(&f), self.m.attr_set(&r))
    }

    /// Emits a call to an allocation or deallocation function (clang
    /// `EmitNewDeleteCall`): `vals` in parameter order.
    fn emit_new_delete_call(
        &mut self,
        d: DeclId,
        vals: &[CallArg],
        arg_tys: &[QualType],
        loc: Option<Loc>,
    ) -> R<V> {
        let ast = self.ast;
        let Some(fty) = ast.decls.get(d).ty() else {
            return err(loc, "allocation function without a type");
        };
        let af = self.alloc_fn(d).unwrap_or_default();
        let f = self.get_fn(d)?;
        let sig = self.call_sig(fty, mangle::is_extern_c(ast, ast.first(d)), arg_tys);
        if sig.sret() {
            return err(loc, "an allocation function returning a class is not supported");
        }
        let mut ir_args = Vec::new();
        for (i, &v) in vals.iter().enumerate() {
            let Some(info) = sig.info.args.get(i) else {
                return err(loc, "allocation call argument without ABI information");
            };
            let kind = info.kind.clone();
            self.call_arg(&kind, sig.params[i], v, &mut ir_args, loc)?;
        }
        // `alloc_align`: a constant power of two raises the return
        // alignment; anything else would be an `llvm.assume`.
        let mut ret_align = None;
        if af.replaceable
            && af.is_new
            && let Some(i) = af.align_param
        {
            let c = match vals.get(i) {
                Some(&CallArg::Scalar(v)) => self.cint(v).map(|x| x.1),
                _ => None,
            };
            match c {
                Some(n) if n.is_power_of_two() => ret_align = Some(n as u64),
                _ => {
                    return err(
                        loc,
                        "a non-constant std::align_val_t allocation argument is not supported yet",
                    );
                }
            }
        }
        let (ret, params) = self.sig_attrs(&sig, vals.len(), false);
        let mut fa = Vec::new();
        if af.replaceable {
            fa.push(Attr::Flag(AttrKind::Builtin));
        }
        if sig.nothrow {
            fa.push(Attr::Flag(AttrKind::NoUnwind));
        }
        let mut ra = self.m.attrs_of(ret).to_vec();
        if af.replaceable && af.is_new {
            fa.push(Attr::AllocSize(0, None));
            ra.push(Attr::Flag(AttrKind::NoAlias));
            if !af.nothrow_t {
                ra.push(Attr::Flag(AttrKind::NonNull));
            }
        }
        if let Some(n) = ret_align {
            ra.push(Attr::Align(n));
        }
        let func = self.m.attr_set(&fa);
        let ret = self.m.attr_set(&ra);
        let opts = CallOpts { attrs: AttrList { func, ret, params }, ..CallOpts::default() };
        let rty = if af.is_new { LTy::PTR } else { LTy::VOID };
        let r = self.f.fir.push(
            Op::Call {
                fn_ty: sig.fn_ty,
                callee: V::C(Value::func(f)),
                args: ir_args,
                opts: Box::new(opts),
            },
            rty,
        );
        self.f.calls.push((Fir::last_inst(r), Some(f), sig.nothrow));
        Ok(r)
    }

    /// The pointer value of `e` and, when its alignment comes from a
    /// declaration, that alignment (clang `EmitPointerWithAlignment` with
    /// `AlignmentSource::Decl`).
    fn pointer_with_align(&mut self, e: ExprId) -> R<(V, Option<u64>)> {
        let ast = self.ast;
        let mut e = e;
        while ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = ast.exprs.operand(e);
        }
        let n = *ast.exprs.get(e);
        if n.kind().is_cast() {
            let s = ast.exprs.operand(e);
            match ast.exprs.cast_kind(e) {
                CastKind::NoOp | CastKind::BitCast => {
                    let sq = ast.exprs.get(s).ty();
                    if ast.types.pointee(sq).is_some_and(|p| !ast.types.is_void(p))
                        && ast.types.is_pointer(sq)
                    {
                        let (v, a) = self.pointer_with_align(s)?;
                        // An explicit cast of an opaque pointer takes the
                        // target type's alignment, which the caller then
                        // discards; only `Decl` survives either way.
                        return Ok((v, a));
                    }
                }
                CastKind::ArrayToPointerDecay => {
                    let lv = self.emit_lvalue(s)?;
                    let decl = self.lvalue_from_decl(s);
                    let v = self.decay(lv.addr);
                    return Ok((v, decl.then_some(lv.addr.align)));
                }
                _ => {}
            }
        }
        if n.kind() == ExprKind::UnaryOperator && ast.exprs.unary_op(e) == UnaryOp::AddrOf {
            let s = ast.exprs.operand(e);
            let lv = self.emit_lvalue(s)?;
            let decl = self.lvalue_from_decl(s);
            return Ok((lv.addr.v, decl.then_some(lv.addr.align)));
        }
        Ok((self.emit_scalar(e)?, None))
    }

    /// The lvalue `e` has `AlignmentSource::Decl`: a named non-reference
    /// variable, or a `.` member or array element of one.
    fn lvalue_from_decl(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr => self.lvalue_from_decl(ast.exprs.operand(e)),
            ExprKind::DeclRefExpr => {
                let d = ast.exprs.decl(e);
                matches!(ast.decls.get(d).kind(), DeclKind::Var | DeclKind::ParmVar)
                    && ast.decls.get(d).ty().is_some_and(|q| !ast.types.is_reference(q))
            }
            ExprKind::MemberExpr if !n.has(ExprFlags::ARROW) => {
                self.lvalue_from_decl(ast.exprs.operand(e))
            }
            ExprKind::ArraySubscriptExpr => {
                let [l, _] = ast.exprs.operands(e);
                let l = self.skip_parens(l);
                ast.exprs.get(l).kind().is_cast()
                    && ast.exprs.cast_kind(l) == CastKind::ArrayToPointerDecay
                    && self.lvalue_from_decl(ast.exprs.operand(l))
            }
            _ => false,
        }
    }

    fn skip_parens(&self, mut e: ExprId) -> ExprId {
        while self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = self.ast.exprs.operand(e);
        }
        e
    }

    /// The preferred alignment of `ty` (`DataLayout::getPrefTypeAlign`):
    /// `AArch64` prefers 32-bit alignment for `i8`/`i16`.
    fn pref_align(&self, ty: LTy) -> u64 {
        let (_, a) = self.ir_layout(ty);
        if !self.is_x86() && self.m.types.int_width(ty).is_some_and(|w| (2..=16).contains(&w)) {
            return a.max(4);
        }
        a
    }

    /// clang `EmitCXXNewExpr`.
    pub(crate) fn emit_cxx_new(&mut self, e: ExprId) -> R<V> {
        let ast = self.ast;
        let t = &ast.types;
        let n = *ast.exprs.get(e);
        let loc = self.loc_of(e);
        let x: NewExtra = *ast.exprs.new_extra(e);
        let Some(op_new) = x.operator_new else {
            return err(loc, "new-expression without an allocation function");
        };
        let is_array = n.has(ExprFlags::ARRAY_FORM);
        let pass_align = n.has(ExprFlags::PASS_ALIGN);
        // The base element type and the constant-dimension multiplier.
        let mut base = x.allocated_ty;
        let mut asm: u64 = 1;
        while let TypeView::ConstantArray(a) = t.canonical_view(base) {
            asm = asm.wrapping_mul(a.size);
            base = a.elem;
        }
        if matches!(t.canonical_view(base), TypeView::MemberPointer { .. }) {
            return err(loc, "new of a member pointer is not supported yet");
        }
        let (esize, alloc_align) = self.size_align(base);
        let Some(fty) = ast.decls.get(op_new).ty() else {
            return err(loc, "allocation function without a type");
        };
        let Some(proto) = t.proto(fty).copied() else {
            return err(loc, "allocation function without a prototype");
        };
        let params: Vec<QualType> = t.params(&proto).to_vec();
        let nothrow = t.exc_spec(&proto).is_nothrow();
        let af = self.alloc_fn(op_new).unwrap_or_default();

        // minElements: a braced initializer's explicit elements.
        let ile = x.init.filter(|&i| ast.exprs.get(i).kind() == ExprKind::InitListExpr);
        let mut min_elems: u64 = 0;
        if is_array && let Some(i) = x.init {
            let lits: Vec<ExprId> = match ile {
                Some(l) => ast.lists.iter(ast.exprs.init_list_inits(l)).collect(),
                None => vec![i],
            };
            if lits.len() == 1
                && t.is_array(ast.exprs.get(i).ty())
                && self.strip_to_string_literal(lits[0])
            {
                return err(
                    loc,
                    "new of an array initialized from a string literal is not supported yet",
                );
            }
            if ile.is_some() {
                min_elems = lits.len() as u64;
            }
        }

        // EmitCXXNewAllocSize.
        let (size, num_elems) = if is_array {
            let Some(ae) = x.array_size else {
                return err(loc, "array new without a size");
            };
            let tsm = esize.wrapping_mul(asm);
            let nv = match self.fold_int(ae) {
                Some(c) => self.i64c(c),
                None => self.emit_scalar(ae)?,
            };
            if let Some((_, c, _)) = self.cint(nv) {
                let c = c as u64;
                let (bytes, ov) = c.overflowing_mul(tsm);
                let num = self.i64c(i128::from(c.wrapping_mul(asm)));
                let size =
                    if ov || c < min_elems { self.i64c(-1) } else { self.i64c(i128::from(bytes)) };
                (size, Some(num))
            } else {
                let mut has_ov = None;
                if min_elems > 0 {
                    let m = self.i64c(i128::from(min_elems));
                    has_ov = Some(self.icmp(IntPredicate::Ult, nv, m));
                }
                let mut size = nv;
                let mut num = nv;
                if tsm != 1 {
                    let f = kiln_llvm::intrinsics::with_overflow(
                        &mut self.m,
                        OverflowOp::UMul,
                        LTy::I64,
                    );
                    let fn_ty = self.m.func(f).ty();
                    let rty = self.m.types.struct_literal(&[LTy::I64, LTy::I1], false);
                    let tv = self.i64c(i128::from(tsm));
                    let r = self.f.fir.push(
                        Op::Call {
                            fn_ty,
                            callee: V::C(Value::func(f)),
                            args: vec![nv, tv],
                            opts: Box::default(),
                        },
                        rty,
                    );
                    self.f.calls.push((Fir::last_inst(r), Some(f), true));
                    let ov = self.f.fir.push(Op::ExtractValue { agg: r, idx: 1 }, LTy::I1);
                    has_ov = Some(match has_ov {
                        Some(h) => self.bin(BinOp::Or, Flags::NONE, h, ov),
                        None => ov,
                    });
                    size = self.f.fir.push(Op::ExtractValue { agg: r, idx: 0 }, LTy::I64);
                    if asm != 1 {
                        num = if esize == 1 {
                            size
                        } else {
                            let a = self.i64c(i128::from(asm));
                            self.bin(BinOp::Mul, Flags::NONE, nv, a)
                        };
                    }
                }
                if let Some(h) = has_ov {
                    let m1 = self.i64c(-1);
                    size = self.select(h, m1, size);
                }
                (size, Some(num))
            }
        } else {
            let (sz, _) = self.size_align(x.allocated_ty);
            (self.i64c(i128::from(sz)), None)
        };

        // The allocation.
        let placement: Vec<ExprId> =
            x.placement.map(|l| ast.lists.iter(l).collect()).unwrap_or_default();
        let mut placement_vals: Vec<CallArg> = Vec::new();
        let (alloc, allocation_align) = if af.reserved {
            let Some(&arg) = placement.first() else {
                return err(loc, "placement new without a placement argument");
            };
            let (p, decl_align) = self.pointer_with_align(arg)?;
            (p, decl_align.unwrap_or(alloc_align))
        } else {
            let size_q = params.first().copied().unwrap_or(QualType::unqual(t.target().size_t));
            let mut vals = vec![CallArg::Scalar(size)];
            let mut arg_tys = vec![size_q];
            if pass_align {
                let al = self.i64c(i128::from(alloc_align));
                vals.push(CallArg::Scalar(al));
                arg_tys.push(params.get(1).copied().unwrap_or(size_q));
            }
            let skip = vals.len();
            for (k, &a) in placement.iter().enumerate() {
                let aq = ast.exprs.get(a).ty();
                let pq = params.get(skip + k).copied().unwrap_or(aq);
                let v = if self.referee(pq).is_some() {
                    CallArg::Scalar(self.emit_lvalue(a)?.addr.v)
                } else if self.is_aggregate(aq) {
                    let tmp = self.temp(aq);
                    self.emit_agg(a, tmp, false)?;
                    CallArg::Agg(tmp)
                } else {
                    CallArg::Scalar(self.emit_scalar(a)?)
                };
                vals.push(v);
                arg_tys.push(aq);
                placement_vals.push(v);
            }
            let p = self.emit_new_delete_call(op_new, &vals, &arg_tys, loc)?;
            let mut align = alloc_align;
            if !pass_align && af.replaceable {
                let (bsize, _) = self.size_align(base);
                let bits = (self.target.default_new_align * 8).min(bsize * 8);
                let floor = if bits == 0 { 0 } else { 1u64 << bits.ilog2() };
                align = align.max(floor / 8);
            }
            (p, align)
        };

        // The null check (an allocation function that may return null).
        let null_check = !af.reserved
            && nothrow
            && !(af.replaceable && af.is_new && !af.nothrow_t)
            && x.init.is_some();
        let mut null_bb = None;
        let mut cont = None;
        if null_check {
            let Some(nb) = self.f.fir.cur() else {
                return err(loc, "new-expression without an insertion point");
            };
            let notnull = self.f.fir.create_block();
            let c = self.f.fir.create_block();
            let z = V::C(self.m.const_null(LTy::PTR));
            let isnull = self.icmp(IntPredicate::Eq, alloc, z);
            self.cond_br(isnull, c, notnull);
            self.emit_block(notnull, false);
            self.f.conds.push(nb);
            null_bb = Some(nb);
            cont = Some(c);
        }

        // The `operator delete` cleanup: only its conditional-evaluation
        // bookkeeping is observable without an exceptional edge.
        let cleanup = self.opts.exceptions
            && x.operator_delete.is_some_and(|d| !self.alloc_fn(d).is_some_and(|a| a.reserved));
        if cleanup && let Some(&outer) = self.f.conds.first() {
            let mut save = vec![alloc, size];
            for v in &placement_vals {
                match *v {
                    CallArg::Scalar(s) => save.push(s),
                    CallArg::Agg(_) => {
                        return err(
                            loc,
                            "a conditional new-expression with a class placement argument is not supported yet",
                        );
                    }
                }
            }
            let entry = self.f.fir.entry();
            for v in save {
                if let V::I(i) = v
                    && self.f.fir.inst_block(i) != Some(entry)
                {
                    let ty = self.f.fir.ty(&self.m, v);
                    let al = self.pref_align(ty);
                    let slot = self.f.fir.alloca(ty, al);
                    self.store(v, slot, al, false);
                }
            }
            let flag = self.f.fir.alloca(LTy::I1, 1);
            let f0 = V::C(self.m.const_bool(false));
            self.f.fir.insert_before_last(
                outer,
                Op::Store { val: f0, ptr: flag, mem: MemOpts { align: 1, ..MemOpts::default() } },
                LTy::VOID,
            );
            let t1 = V::C(self.m.const_bool(true));
            self.store(t1, flag, 1, false);
        }

        // The initializer.
        let calls_before = self.f.calls.len();
        if is_array {
            let num = num_elems.unwrap_or(size);
            self.emit_new_array_init(&x, base, esize, alloc, allocation_align, num, size, loc)?;
        } else if let Some(init) = x.init {
            let ty = self.mty(x.allocated_ty);
            let lv = LV {
                addr: Addr { v: alloc, ty, align: allocation_align },
                q: x.allocated_ty,
                bf: None,
            };
            self.init_to_lvalue(init, lv, false)?;
        }
        if cleanup {
            let may_throw = self.f.calls[calls_before..].iter().any(|&(_, c, site)| {
                !site && !c.is_some_and(|c| self.nounwind.get(&c).copied().unwrap_or(false))
            });
            if may_throw {
                return err(
                    loc,
                    "a new-expression whose initializer may throw needs an exception cleanup, which is not supported yet",
                );
            }
        }

        if let (Some(nb), Some(c)) = (null_bb, cont) {
            self.f.conds.pop();
            let Some(nn_end) = self.f.fir.cur() else {
                return err(loc, "new-expression initializer without a continuation");
            };
            self.emit_block(c, false);
            let z = V::C(self.m.const_null(LTy::PTR));
            return Ok(self
                .f
                .fir
                .push(Op::Phi { ty: LTy::PTR, inc: vec![(alloc, nn_end), (z, nb)] }, LTy::PTR));
        }
        Ok(alloc)
    }

    /// `e` is (through parentheses and implicit casts) a string literal.
    fn strip_to_string_literal(&self, mut e: ExprId) -> bool {
        let ast = self.ast;
        loop {
            let k = ast.exprs.get(e).kind();
            match k {
                ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => e = ast.exprs.operand(e),
                ExprKind::StringLiteral => return true,
                _ => return false,
            }
        }
    }

    /// clang `EmitNewArrayInitializer` (no constructors, no cookies).
    #[allow(clippy::too_many_arguments, reason = "mirrors clang's parameter list")]
    fn emit_new_array_init(
        &mut self,
        x: &NewExtra,
        base: QualType,
        esize: u64,
        begin: V,
        align: u64,
        num: V,
        size: V,
        loc: Option<Loc>,
    ) -> R<()> {
        let ast = self.ast;
        let t = &ast.types;
        let Some(init0) = x.init else { return Ok(()) };
        let base_ty = self.mty(base);
        let elem_align = min_align(align, esize);
        let mut elem_ty = base_ty;
        let mut cur = begin;
        let mut cur_align = align;
        let mut n_init: u64 = 0;
        let mut init = Some(init0);
        if ast.exprs.get(init0).kind() == ExprKind::InitListExpr {
            let elems: Vec<ExprId> = ast.lists.iter(ast.exprs.init_list_inits(init0)).collect();
            n_init = elems.len() as u64;
            let mut cur_ty = base_ty;
            if matches!(t.canonical_view(x.allocated_ty), TypeView::ConstantArray(_)) {
                elem_ty = self.mty(x.allocated_ty);
                cur_ty = elem_ty;
                let mut q = x.allocated_ty;
                let mut count: u64 = 1;
                while let TypeView::ConstantArray(a) = t.canonical_view(q) {
                    count = count.wrapping_mul(a.size);
                    q = a.elem;
                }
                n_init = n_init.wrapping_mul(count);
            }
            let start_align = cur_align;
            for (k, &ie) in elems.iter().enumerate() {
                let q = ast.exprs.get(ie).ty();
                let lv = LV { addr: Addr { v: cur, ty: cur_ty, align: cur_align }, q, bf: None };
                self.init_to_lvalue(ie, lv, false)?;
                let one = self.i64c(1);
                cur = self.gep(cur_ty, cur, vec![one], true);
                cur_align = min_align(start_align, (k as u64 + 1).wrapping_mul(esize));
            }
            init = ast.exprs.init_list(init0).array_filler;
            while let Some(f) = init
                && matches!(t.canonical_view(ast.exprs.get(f).ty()), TypeView::ConstantArray(_))
                && ast.exprs.get(f).kind() == ExprKind::InitListExpr
            {
                init = ast.exprs.init_list(f).array_filler;
            }
        }
        if let Some((_, c, _)) = self.cint(num)
            && (c as u64) <= n_init
        {
            return Ok(());
        }
        let Some(init) = init else {
            return err(loc, "array new with trailing elements but no initializer");
        };
        // TryMemsetInitialization.
        if self.memset_init(init) {
            let mut rem = size;
            if n_init > 0 {
                let s = self.i64c(i128::from(esize.wrapping_mul(n_init)));
                rem = self.bin(BinOp::Sub, Flags::NONE, size, s);
            }
            self.memset_v(Addr { v: cur, ty: base_ty, align: cur_align }, 0, rem, false);
            return Ok(());
        }
        // The element loop.
        let Some(entry) = self.f.fir.cur() else {
            return err(loc, "array new without an insertion point");
        };
        let lp = self.f.fir.create_block();
        let cont = self.f.fir.create_block();
        let end = self.gep(base_ty, begin, vec![num], true);
        if self.cint(num).is_none() {
            let empty = self.icmp(IntPredicate::Eq, cur, end);
            self.cond_br(empty, cont, lp);
        }
        self.emit_block(lp, false);
        let phi = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(cur, entry)] }, LTy::PTR);
        let iq = ast.exprs.get(init).ty();
        let ity = self.mty(iq);
        let lv = LV { addr: Addr { v: phi, ty: ity, align: elem_align }, q: iq, bf: None };
        self.init_to_lvalue(init, lv, false)?;
        let one = self.icn(LTy::I32, 1);
        let next = self.gep(elem_ty, phi, vec![one], true);
        let at_end = self.icmp(IntPredicate::Eq, next, end);
        self.cond_br(at_end, cont, lp);
        let Some(cb) = self.f.fir.cur() else {
            return err(loc, "array new loop without an insertion point");
        };
        self.f.fir.add_incoming(Fir::last_inst(phi), next, cb);
        self.emit_block(cont, false);
        Ok(())
    }

    /// The remaining elements can be zeroed with one `memset`: value
    /// initialization, `{}`, or a `struct` whose every member is
    /// value-initialized.
    fn memset_init(&self, init: ExprId) -> bool {
        let ast = self.ast;
        let n = ast.exprs.get(init);
        match n.kind() {
            ExprKind::ImplicitValueInitExpr => true,
            ExprKind::InitListExpr => {
                let inits: Vec<ExprId> = ast.lists.iter(ast.exprs.init_list_inits(init)).collect();
                if inits.is_empty() {
                    return true;
                }
                let Some(rd) = ast.types.as_record(n.ty()) else { return false };
                if ast.decls.record(rd).map(|r| r.tag) != Some(TagKind::Struct) {
                    return false;
                }
                let mut count = ast.decls.bases(rd, &ast.lists).count();
                for m in ast.decls.members(rd) {
                    if ast.decls.get(m).kind() != DeclKind::Field {
                        continue;
                    }
                    let unnamed_bf = ast.decls.field(m).is_some_and(|f| f.bit_width.is_some())
                        && mangle::decl_name(ast, m).is_empty();
                    if !unnamed_bf {
                        count += 1;
                    }
                }
                inits.len() == count
                    && inits
                        .iter()
                        .all(|&i| ast.exprs.get(i).kind() == ExprKind::ImplicitValueInitExpr)
            }
            _ => false,
        }
    }

    /// clang `EmitCXXDeleteExpr` (trivially destructible types).
    pub(crate) fn emit_cxx_delete(&mut self, e: ExprId) -> R<()> {
        let ast = self.ast;
        let t = &ast.types;
        let loc = self.loc_of(e);
        let arg = ast.exprs.delete_operand(e);
        let Some(od) = ast.exprs.operator_delete(e) else {
            return err(loc, "delete-expression without a deallocation function");
        };
        let Some(mut dt) = t.pointee(ast.exprs.get(arg).ty()) else {
            return err(loc, "delete of a non-pointer");
        };
        let p = self.emit_scalar(arg)?;
        let notnull = self.f.fir.create_block();
        let end = self.f.fir.create_block();
        let z = V::C(self.m.const_null(LTy::PTR));
        let isnull = self.icmp(IntPredicate::Eq, p, z);
        self.cond_br(isnull, end, notnull);
        self.emit_block(notnull, false);
        let mut ptr = p;
        if matches!(t.canonical_view(dt), TypeView::ConstantArray(_)) {
            let src = self.mty(dt);
            let zero = self.icn(LTy::I32, 0);
            let mut idx = vec![zero];
            while let TypeView::ConstantArray(a) = t.canonical_view(dt) {
                dt = a.elem;
                idx.push(zero);
            }
            ptr = self.gep(src, p, idx, true);
        }
        // getUsualDeleteParams.
        let Some(fty) = ast.decls.get(od).ty() else {
            return err(loc, "deallocation function without a type");
        };
        let Some(proto) = t.proto(fty).copied() else {
            return err(loc, "deallocation function without a prototype");
        };
        let params: Vec<QualType> = t.params(&proto).to_vec();
        let Some(&p0) = params.first() else {
            return err(loc, "deallocation function without parameters");
        };
        let mut vals = vec![CallArg::Scalar(ptr)];
        let mut arg_tys = vec![p0];
        let mut i = 1;
        let incomplete = t.is_void(dt)
            || t.as_record(dt).is_some_and(|r| {
                ast.definition(r).is_none_or(|d| {
                    !ast.decls
                        .record(d)
                        .is_some_and(|x| x.defn.contains(kiln_ast::decl::DefinitionBits::COMPLETE))
                })
            });
        if let Some(&q) = params.get(i)
            && t.is_integral(q)
            && !self.is_std_enum(q, "align_val_t")
        {
            if incomplete {
                return err(loc, "sized delete of an incomplete type");
            }
            let (sz, _) = self.size_align(dt);
            let ty = self.lty(q);
            vals.push(CallArg::Scalar(self.icn(ty, i128::from(sz))));
            arg_tys.push(q);
            i += 1;
        }
        if let Some(&q) = params.get(i)
            && self.is_std_enum(q, "align_val_t")
        {
            if incomplete {
                return err(loc, "aligned delete of an incomplete type");
            }
            let (_, al) = self.size_align(dt);
            let ty = self.lty(q);
            vals.push(CallArg::Scalar(self.icn(ty, i128::from(al))));
            arg_tys.push(q);
            i += 1;
        }
        if i != params.len() {
            return err(loc, "this deallocation function is not supported yet");
        }
        self.emit_new_delete_call(od, &vals, &arg_tys, loc)?;
        self.emit_block(end, false);
        Ok(())
    }
}
