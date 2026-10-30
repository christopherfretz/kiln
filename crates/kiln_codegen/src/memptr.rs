//! Pointers to members (spec/abi.md, Itanium C++ ABI §2.3; clang
//! `ItaniumCXXABI`, with `UseARMMethodPtrABI` on AArch64).
//!
//! A data member pointer is a `ptrdiff_t` offset with null `-1`; a member
//! function pointer is `{ ptr-as-int, adj }` with null `ptr == 0`. On
//! x86-64 `adj` is the `this` adjustment and the low bit of `ptr` marks a
//! virtual function (a vtable offset plus one); the ARM variant stores
//! `2 * adjustment + virtual` in `adj` and the plain vtable offset in
//! `ptr`. Virtual member functions themselves are WP-080.

use kiln_ast::consts::MemberKind;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_llvm::{BinOp, CastOp, ConstData, Flags, IntPredicate, TypeId as LTy, Value};

use crate::expr::LV;
use crate::fir::{Fir, Op, V};
use crate::func::Addr;
use crate::module::{Cg, R, err};

impl Cg<'_> {
    /// The pointee type and class of member pointer type `q`.
    pub(crate) fn mp_parts(&self, q: QualType) -> Option<(QualType, DeclId)> {
        let ast = self.ast;
        match ast.types.canonical_view(q) {
            TypeView::MemberPointer { pointee, class } => {
                Some((pointee, ast.types.as_record(QualType::unqual(class))?))
            }
            _ => None,
        }
    }

    /// `q` is a pointer to member function.
    pub(crate) fn is_mfp(&self, q: QualType) -> bool {
        self.mp_parts(q).is_some_and(|(p, _)| self.ast.types.is_function(p))
    }

    /// `q` is a pointer to member.
    pub(crate) fn is_mp(&self, q: QualType) -> bool {
        matches!(self.ast.types.canonical_view(q), TypeView::MemberPointer { .. })
    }

    fn arm_mp(&self) -> bool {
        !self.is_x86()
    }

    /// `extractvalue agg, idx` of a `{ i64, i64 }`, folded on constants
    /// (`IRBuilder`'s `ConstantFolder`).
    pub(crate) fn extract_i64(&mut self, agg: V, idx: u32) -> V {
        if let V::C(c) = agg {
            match self.m.const_of(c) {
                Some(ConstData::Struct { elems, .. }) => {
                    if let Some(&x) = elems.get(idx as usize) {
                        return V::C(x);
                    }
                }
                Some(ConstData::Zero(_)) => return self.i64c(0),
                _ => {}
            }
        }
        self.f.fir.push(Op::ExtractValue { agg, idx }, LTy::I64)
    }

    /// `insertvalue agg, val, idx` of a `{ i64, i64 }`, folded on
    /// constants.
    fn insert_i64(&mut self, agg: V, val: V, idx: u32) -> V {
        let ty = self.f.fir.ty(&self.m, agg);
        if let (V::C(c), V::C(x)) = (agg, val) {
            let elems: Option<Vec<Value>> = match self.m.const_of(c) {
                Some(ConstData::Struct { elems, .. }) => Some(elems.to_vec()),
                Some(ConstData::Zero(_)) => {
                    let z = self.m.const_int(LTy::I64, 0);
                    Some(vec![z, z])
                }
                _ => None,
            };
            if let Some(mut elems) = elems
                && (idx as usize) < elems.len()
            {
                elems[idx as usize] = x;
                if let Ok(s) = self.m.const_struct(ty, &elems) {
                    return V::C(s);
                }
            }
        }
        self.f.fir.push(Op::InsertValue { agg, val, idx }, ty)
    }

    /// clang `EmitNullMemberPointer`.
    pub(crate) fn null_mp(&mut self, q: QualType) -> V {
        if self.is_mfp(q) {
            let t = self.lty(q);
            V::C(self.m.const_zero(t))
        } else {
            self.i64c(-1)
        }
    }

    /// The byte offset of field `f` in its class.
    fn field_byte_offset(&mut self, f: DeclId) -> R<u64> {
        let ast = self.ast;
        let Some(rd) = ast.decls.get(f).ctx() else {
            return err(Some(ast.decls.get(f).loc()), "field outside a class");
        };
        let Some(def) = self.class_def(rd) else {
            return err(Some(ast.decls.get(f).loc()), "field of an incomplete class");
        };
        let Some(i) = self.field_number(def, f) else {
            return err(Some(ast.decls.get(f).loc()), "field not in its class");
        };
        Ok(self.tys.layouter().record(def).fields[i] / 8)
    }

    /// clang `EmitMemberFunctionPointer`: `{ ptrtoint @f, adj }` (`adj`
    /// already in the target's encoding).
    fn mfp_const(&mut self, md: DeclId, adj: i128, t: LTy) -> R<Value> {
        let ast = self.ast;
        if ast.decls.function(md).is_some_and(|f| f.spec.is_virtual()) {
            return err(
                Some(ast.decls.get(md).loc()),
                "pointers to virtual member functions are WP-080",
            );
        }
        let f = self.get_fn(md)?;
        let p = self
            .m
            .const_cast(CastOp::PtrToInt, Value::func(f), LTy::I64)
            .map_err(|e| crate::consts::ir_err(&e))?;
        let a = self.m.const_int(LTy::I64, adj);
        self.m.const_struct(t, &[p, a]).map_err(|e| crate::consts::ir_err(&e))
    }

    /// clang `getMemberPointerConstant` for `&C::m` (`e` the address-of
    /// expression, of member pointer type `q`).
    pub(crate) fn mp_addr_of(&mut self, e: ExprId, q: QualType) -> R<V> {
        let ast = self.ast;
        let mut s = ast.exprs.operand(e);
        while ast.exprs.get(s).kind() == ExprKind::ParenExpr {
            s = ast.exprs.operand(s);
        }
        let d = ast.exprs.decl(s);
        match ast.decls.get(d).kind() {
            DeclKind::Field => {
                let off = self.field_byte_offset(d)?;
                Ok(self.i64c(i128::from(off)))
            }
            DeclKind::CXXMethod | DeclKind::CXXConversion => {
                let t = self.lty(q);
                Ok(V::C(self.mfp_const(d, 0, t)?))
            }
            k => err(self.loc_of(e), format!("pointer to member {k:?} is not supported yet")),
        }
    }

    /// clang `EmitMemberPointer(APValue)`: a constant member pointer.
    pub(crate) fn mp_value(
        &mut self,
        member: DeclId,
        kind: MemberKind,
        derived: bool,
        path: &[DeclId],
        t: LTy,
    ) -> R<Value> {
        let ast = self.ast;
        // clang `getMemberPointerPathAdjustment`.
        let mut rd = ast.decls.get(member).ctx().unwrap_or(member);
        let mut adj: i128 = 0;
        for &p in path {
            let (base, der) = if derived { (p, rd) } else { (rd, p) };
            let Some(o) = self.base_offset(der, base) else {
                return err(Some(ast.decls.get(member).loc()), "member pointer path not in the layout");
            };
            adj += i128::from(o);
            rd = p;
        }
        if derived {
            adj = -adj;
        }
        match kind {
            MemberKind::Data => {
                let off = self.field_byte_offset(member)?;
                Ok(self.m.const_int(LTy::I64, i128::from(off) + adj))
            }
            MemberKind::Function => {
                let a = if self.arm_mp() { adj * 2 } else { adj };
                self.mfp_const(member, a, t)
            }
        }
    }

    /// clang `ItaniumCXXABI::EmitMemberPointerConversion` for cast `e`
    /// (`BaseToDerivedMemberPointer` / `DerivedToBaseMemberPointer`) of
    /// `src`; the constant forms fall out of the folding builders.
    pub(crate) fn mp_convert(&mut self, e: ExprId, src: V) -> R<V> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let sq = ast.exprs.get(ast.exprs.operand(e)).ty();
        let to_base = ast.exprs.cast_kind(e) == CastKind::DerivedToBaseMemberPointer;
        let Some((_, derived)) = self.mp_parts(if to_base { sq } else { q }) else {
            return err(self.loc_of(e), "member pointer conversion of a non-member pointer");
        };
        let off = self.base_path_offset(e, derived)?;
        if off == 0 {
            return Ok(src);
        }
        let op = if to_base { BinOp::Sub } else { BinOp::Add };
        if !self.is_mfp(q) {
            let a = self.i64c(i128::from(off));
            let dst = self.bin(op, Flags::NSW, src, a);
            let null = self.i64c(-1);
            let is_null = self.icmp(IntPredicate::Eq, src, null);
            return Ok(self.select(is_null, src, dst));
        }
        let off = if self.arm_mp() { off * 2 } else { off };
        let a = self.i64c(i128::from(off));
        let sa = self.extract_i64(src, 1);
        let da = self.bin(op, Flags::NSW, sa, a);
        Ok(self.insert_i64(src, da, 1))
    }

    /// clang `EmitMemberPointerIsNotNull`.
    pub(crate) fn mp_is_not_null(&mut self, v: V, q: QualType) -> V {
        if !self.is_mfp(q) {
            let n = self.i64c(-1);
            return self.icmp(IntPredicate::Ne, v, n);
        }
        let p = self.extract_i64(v, 0);
        let z = self.i64c(0);
        let r = self.icmp(IntPredicate::Ne, p, z);
        if !self.arm_mp() {
            return r;
        }
        let one = self.i64c(1);
        let adj = self.extract_i64(v, 1);
        let bit = self.bin(BinOp::And, Flags::NONE, adj, one);
        let virt = self.icmp(IntPredicate::Ne, bit, z);
        self.bin(BinOp::Or, Flags::NONE, r, virt)
    }

    /// clang `EmitMemberPointerComparison` (`ne`: `!=`).
    pub(crate) fn mp_compare(&mut self, l: V, r: V, q: QualType, ne: bool) -> V {
        let (eq, and, or) = if ne {
            (IntPredicate::Ne, BinOp::Or, BinOp::And)
        } else {
            (IntPredicate::Eq, BinOp::And, BinOp::Or)
        };
        if !self.is_mfp(q) {
            return self.icmp(eq, l, r);
        }
        let l0 = self.extract_i64(l, 0);
        let r0 = self.extract_i64(r, 0);
        let ptr_eq = self.icmp(eq, l0, r0);
        let z = self.i64c(0);
        let mut eq_zero = self.icmp(eq, l0, z);
        let l1 = self.extract_i64(l, 1);
        let r1 = self.extract_i64(r, 1);
        let adj_eq = self.icmp(eq, l1, r1);
        if self.arm_mp() {
            let one = self.i64c(1);
            let or_adj = self.bin(BinOp::Or, Flags::NONE, l1, r1);
            let bit = self.bin(BinOp::And, Flags::NONE, or_adj, one);
            let bit_z = self.icmp(eq, bit, z);
            eq_zero = self.bin(and, Flags::NONE, eq_zero, bit_z);
        }
        let res = self.bin(or, Flags::NONE, eq_zero, adj_eq);
        self.bin(and, Flags::NONE, ptr_eq, res)
    }

    /// clang `getDynamicOffsetAlignment`.
    fn dyn_offset_align(&mut self, actual: u64, class: DeclId, expected: u64) -> u64 {
        let Some(def) = self.class_def(class) else { return actual.min(expected) };
        let nv = u64::from(self.tys.layouter().record(def).nv_align);
        if actual >= nv { expected } else { actual.min(expected) }
    }

    /// The object operand of `.*` / `->*` `e` (clang: the pointer with its
    /// alignment for `->*`, the lvalue's address for `.*`).
    fn mp_object(&mut self, e: ExprId) -> R<Addr> {
        let ast = self.ast;
        let [l, _] = ast.exprs.operands(e);
        if ast.exprs.binary_op(e) == BinaryOp::PtrMemI {
            let p = self.emit_scalar(l)?;
            let lq = ast.exprs.get(l).ty();
            let Some(pq) = ast.types.pointee(lq) else {
                return err(self.loc_of(e), "->* on a non-pointer");
            };
            let (_, align) = self.pointee_layout(pq);
            let ty = self.mty(pq);
            Ok(Addr { v: p, ty, align })
        } else {
            Ok(self.emit_lvalue(l)?.addr)
        }
    }

    /// clang `EmitPointerToDataMemberBinaryExpr`: the lvalue of `.*` /
    /// `->*` `e` with a data member pointer.
    pub(crate) fn mp_data_lvalue(&mut self, e: ExprId) -> R<LV> {
        let ast = self.ast;
        let [_, r] = ast.exprs.operands(e);
        let base = self.mp_object(e)?;
        let off = self.emit_scalar(r)?;
        let rq = ast.exprs.get(r).ty();
        let Some((pointee, class)) = self.mp_parts(rq) else {
            return err(self.loc_of(e), "pointer-to-member operand of a non-member pointer");
        };
        let p = self.gep(LTy::I8, base.v, vec![off], true);
        let natural = self.natural_align(pointee);
        let align = self.dyn_offset_align(base.align, class, natural);
        let q = ast.exprs.get(e).ty();
        let ty = self.mty(q);
        Ok(LV { addr: Addr { v: p, ty, align }, q, bf: None })
    }

    /// clang `EmitCXXMemberPointerCallExpr` with
    /// `ItaniumCXXABI::EmitLoadOfMemberFunctionPointer`: call `e` whose
    /// callee is `.*` / `->*` `bo`.
    pub(crate) fn mp_call(&mut self, e: ExprId, bo: ExprId, dest: Option<Addr>) -> R<Option<V>> {
        let ast = self.ast;
        let [_, r] = ast.exprs.operands(bo);
        let rq = ast.exprs.get(r).ty();
        let Some((fty, rd)) = self.mp_parts(rq) else {
            return err(self.loc_of(e), "call through a non-member pointer");
        };
        let this = self.mp_object(bo)?;
        let mfp = self.emit_scalar(r)?;
        let arm = self.arm_mp();
        let one = self.i64c(1);
        let raw_adj = self.extract_i64(mfp, 1);
        let adj = if arm { self.bin(BinOp::AShr, Flags::NONE, raw_adj, one) } else { raw_adj };
        let this_v = self.gep(LTy::I8, this.v, vec![adj], true);
        let fn_int = self.extract_i64(mfp, 0);
        let bit = self.bin(BinOp::And, Flags::NONE, if arm { raw_adj } else { fn_int }, one);
        let z = self.i64c(0);
        let is_virtual = self.icmp(IntPredicate::Ne, bit, z);
        let virt = self.f.fir.create_block();
        let nonvirt = self.f.fir.create_block();
        let end = self.f.fir.create_block();
        self.cond_br(is_virtual, virt, nonvirt);
        self.emit_block(virt, false);
        let vt_align = self.dyn_offset_align(this.align, rd, 8);
        let vtable = self.load(LTy::PTR, this_v, vt_align, false);
        let off = if arm { fn_int } else { self.bin(BinOp::Sub, Flags::NONE, fn_int, one) };
        let slot = self.gep(LTy::I8, vtable, vec![off], false);
        let k = self.m.md_kind("nosanitize");
        let md = self.m.md_tuple(&[]);
        if let V::I(_) = slot {
            self.f.fir.add_md(Fir::last_inst(slot), k, md);
        }
        let vfn = self.load(LTy::PTR, slot, 8, false);
        self.f.fir.add_md(Fir::last_inst(vfn), k, md);
        let vfrom = self.f.fir.cur().expect("insertion point");
        self.emit_branch(end);
        self.emit_block(nonvirt, false);
        let nvfn = self.cast(CastOp::IntToPtr, fn_int, LTy::PTR);
        let nvfrom = self.f.fir.cur().expect("insertion point");
        self.emit_block(end, false);
        let callee = self
            .f
            .fir
            .push(Op::Phi { ty: LTy::PTR, inc: vec![(vfn, vfrom), (nvfn, nvfrom)] }, LTy::PTR);
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        let arg_tys: Vec<QualType> = args.iter().map(|&a| ast.exprs.get(a).ty()).collect();
        let sig = self.call_sig_this(fty, false, &arg_tys, Some(rd));
        self.call_with(&sig, callee, None, Some(this_v), &args, dest, &[])
    }
}
