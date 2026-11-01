//! Expression emission (spec/codegen.md §3): clang's `ScalarExprEmitter`,
//! `EmitLValue`, `AggExprEmitter` and `EmitBranchOnBoolExpr` at `-O0`.

use kiln_abi::call::ArgKind;
use kiln_abi::irtype::BitFieldInfo;
use kiln_abi::mangle;
use kiln_ast::decl::{DeclKind, VarStorage};
use kiln_ast::expr::{BinaryOp, CastKind, ExprFlags, ExprKind, UnaryOp, ValueCategory};
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_llvm::{
    Attr, AttrKind, AttrList, BinOp, CallOpts, CastOp, ConstData, Flags, FloatPredicate, FuncId,
    IntPredicate, TypeId as LTy, Value,
};

use crate::cleanup::{Cleanup, IrregularPartialArray};
use crate::fir::{Fir, Op, V};
use crate::fold::is_local;
use crate::func::{Addr, CallArg, min_align};
use crate::module::{Cg, R, Sig, err, gval};

/// The value an `OpaqueValueExpr` stands for.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Opaque {
    Rv(Option<V>),
    Lv(LV),
}

/// The callee of [`Cg::call_vals_to`] (clang `CGCallee`).
#[derive(Clone, Copy, Debug)]
pub(crate) enum Callee {
    /// A function pointer already computed.
    Value(V),
    /// Virtual method `md` of the object at `this` (alignment `align`):
    /// its vtable slot is loaded after the arguments are lowered
    /// (clang `CGCallee::forVirtual`).
    Virtual { md: DeclId, this: V, align: u64 },
}

/// An lvalue: an address, its source type and the bit-field access.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LV {
    pub addr: Addr,
    pub q: QualType,
    pub bf: Option<BitFieldInfo>,
    /// An element of the vector at `addr` (clang `LValue::MakeVectorElt`).
    pub vidx: Option<V>,
}

fn low_mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

impl Cg<'_> {
    #[allow(clippy::unnecessary_wraps, reason = "diagnostic location slot")]
    pub(crate) fn loc_of(&self, e: ExprId) -> Option<kiln_source::Loc> {
        Some(self.ast.exprs.get(e).loc())
    }

    pub(crate) fn sub(&self, e: ExprId) -> ExprId {
        ExprId::new(self.ast.exprs.get(e).a())
    }

    pub(crate) fn is_glvalue(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).value_category() != ValueCategory::PRValue
    }

    pub(crate) fn is_volatile(&self, q: QualType) -> bool {
        self.ast.types.effective_quals(q).is_volatile()
    }

    /// The natural alignment of objects of type `q`.
    pub(crate) fn natural_align(&mut self, q: QualType) -> u64 {
        if self.ast.types.is_void(q) || self.ast.types.is_function(q) {
            return 1;
        }
        self.size_align(q).1
    }

    pub(crate) fn i64c(&mut self, v: i128) -> V {
        V::C(self.m.const_int(LTy::I64, v))
    }

    pub(crate) fn icn(&mut self, ty: LTy, v: i128) -> V {
        if let Some((et, n, _)) = self.m.types.vector_info(ty) {
            let c = self.m.const_int(et, v);
            return V::C(self.m.const_vector(&vec![c; n as usize]).expect("int vector"));
        }
        V::C(self.m.const_int(ty, v))
    }

    /// A floating constant `x` (±1.0 and 0.0 only) of type `ty`.
    pub(crate) fn fconst(&mut self, ty: LTy, x: f64) -> V {
        if let Some((et, n, _)) = self.m.types.vector_info(ty) {
            let V::C(c) = self.fconst(et, x) else { unreachable!("constant") };
            return V::C(self.m.const_vector(&vec![c; n as usize]).expect("float vector"));
        }
        let bits: u128 = match self.m.types.float_width(ty) {
            Some(32) => u128::from((x as f32).to_bits()),
            Some(64) => u128::from(x.to_bits()),
            Some(128) => {
                if x == 0.0 {
                    0
                } else {
                    let s: u128 = if x < 0.0 { 1 << 127 } else { 0 };
                    s | (0x3fffu128 << 112)
                }
            }
            Some(80) => {
                if x == 0.0 {
                    0
                } else {
                    let s: u128 = if x < 0.0 { 1 << 79 } else { 0 };
                    s | (0x3fffu128 << 64) | (1u128 << 63)
                }
            }
            _ => u128::from(x.to_bits()),
        };
        V::C(self.m.const_float_bits(ty, bits))
    }

    pub(crate) fn is_float_ty(&self, t: LTy) -> bool {
        self.m.types.is_float_or_float_vector(t)
    }

    /// The element type of a vector type, else `q` itself.
    pub(crate) fn scalar_of(&self, q: QualType) -> QualType {
        self.vec_of(q).map_or(q, |x| x.0)
    }

    /// clang `EmitCompare` on vectors: the `<N x i1>` result sign-extended
    /// to the result type unless that is an `ext_vector_type` of `bool`.
    fn vec_cmp_result(&mut self, c: V, q: QualType) -> V {
        match self.vec_of(q) {
            Some((elem, _)) if !self.is_bool(elem) => {
                let t = self.lty(q);
                self.cast(CastOp::SExt, c, t)
            }
            _ => c,
        }
    }

    /// Folded integer value of `e` (clang `ConstantFoldsToSimpleInteger`).
    pub(crate) fn fold_int(&mut self, e: ExprId) -> Option<i128> {
        let c = self.folds.get(e)?;
        let q = self.ast.exprs.get(e).ty();
        let v = self.const_scalar_value(c, q).ok()?;
        let (_, x, w) = self.cint(V::C(v))?;
        Some(if self.is_signed(q) && w > 1 { crate::func::sext(x, w) } else { x as i128 })
    }

    pub(crate) fn fold_bool(&mut self, e: ExprId) -> Option<bool> {
        self.fold_int(e).map(|x| x != 0)
    }

    // ---- scalars -------------------------------------------------------

    /// clang `EmitScalarExpr`.
    pub(crate) fn emit_scalar(&mut self, e: ExprId) -> R<V> {
        match self.emit_scalar_opt(e)? {
            Some(v) => Ok(v),
            None => err(self.loc_of(e), "void expression used as a value"),
        }
    }

    /// `EmitScalarExpr` where the expression may be `void`.
    pub(crate) fn emit_scalar_opt(&mut self, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        Ok(Some(match n.kind() {
            ExprKind::ParenExpr => return self.emit_scalar_opt(ast.exprs.operand(e)),
            ExprKind::CompoundLiteralExpr => {
                return self.emit_scalar_opt(ast.exprs.compound_literal_init(e));
            }
            ExprKind::CXXDefaultArgExpr => match default_arg_of(ast, e) {
                Some(d) => {
                    let saved = self.enter_default_arg(e);
                    let r = self.emit_scalar_opt(d);
                    self.srcloc = saved;
                    return r;
                }
                None => return err(self.loc_of(e), "default argument not yet parsed"),
            },
            ExprKind::SourceLocExpr => V::C(self.source_loc_value(e)?),
            ExprKind::CXXThrowExpr => {
                self.emit_throw(e, true)?;
                return Ok(None);
            }
            ExprKind::ExprWithCleanups => {
                let sc = self.begin_scope(false);
                let r = self.emit_scalar_opt(self.sub(e));
                self.end_scope(sc);
                return r;
            }
            ExprKind::CXXThisExpr => self.this_value(),
            ExprKind::ArrayInitIndexExpr => match self.f.array_index.last() {
                Some(&v) => v,
                None => return err(self.loc_of(e), "ArrayInitIndexExpr outside its loop"),
            },
            ExprKind::CXXMemberCallExpr => return self.emit_member_call(e, None),
            ExprKind::CXXDefaultInitExpr => {
                let Some(init) = self.default_member_init(e) else {
                    return err(self.loc_of(e), "default member initializer not yet parsed");
                };
                let saved = self.enter_default_arg(e);
                let r = self.emit_scalar_opt(init);
                self.srcloc = saved;
                return r;
            }
            ExprKind::ConstantExpr => match self.immediate_value(e) {
                Some(c) if !ast.types.is_void(q) => V::C(self.const_scalar_value(c, q)?),
                _ => return self.emit_scalar_opt(ast.exprs.operand(e)),
            },
            ExprKind::IntegerLiteral => {
                if let Some(v) = ast.exprs.int_value(e) {
                    let t = self.lty(q);
                    self.icn(t, i128::from(v))
                } else {
                    let c = ast.exprs.literal_const(e).expect("wide literal");
                    V::C(self.const_scalar_value(c, q)?)
                }
            }
            ExprKind::FloatingLiteral => {
                let c = ast.exprs.literal_const(e).expect("float literal");
                V::C(self.const_scalar_value(c, q)?)
            }
            ExprKind::CharacterLiteral => {
                let t = self.lty(q);
                self.icn(t, i128::from(ast.exprs.char_value(e)))
            }
            ExprKind::CXXBoolLiteralExpr => V::C(self.m.const_bool(ast.exprs.bool_value(e))),
            ExprKind::CXXNoexceptExpr => V::C(self.m.const_bool(ast.exprs.noexcept_value(e))),
            ExprKind::TypeTraitExpr | ExprKind::ExpressionTraitExpr => {
                V::C(self.m.const_bool(ast.exprs.type_trait_value(e)))
            }
            // spec/sema-templates.md §8.3, §8.5: Sema checked satisfaction;
            // a concept-id or requires-expression is a prvalue constant.
            ExprKind::ConceptSpecializationExpr | ExprKind::RequiresExpr => {
                match ast.exprs.satisfied(e) {
                    Some(v) => V::C(self.m.const_bool(v)),
                    None => return err(self.loc_of(e), "unchecked constraint expression"),
                }
            }
            ExprKind::ArrayTypeTraitExpr => {
                let t = self.lty(q);
                V::C(self.m.const_int(t, i128::from(ast.exprs.array_trait(e).value)))
            }
            ExprKind::CXXNullPtrLiteralExpr => V::C(self.m.const_null(LTy::PTR)),
            ExprKind::ImplicitValueInitExpr | ExprKind::CXXScalarValueInitExpr => {
                if ast.types.is_void(q) {
                    return Ok(None);
                }
                let t = self.lty(q);
                V::C(self.m.const_zero(t))
            }
            ExprKind::OffsetOfExpr => {
                let t = self.lty(q);
                V::C(self.m.const_int(t, i128::from(ast.exprs.offsetof_value(e))))
            }
            ExprKind::UnaryExprOrTypeTraitExpr => match self.folds.get(e) {
                Some(c) => V::C(self.const_scalar_value(c, q)?),
                None => return err(self.loc_of(e), "unfolded type trait expression"),
            },
            ExprKind::SubstNonTypeTemplateParmExpr => {
                return self.emit_scalar_opt(ExprId::new(n.b()));
            }
            ExprKind::DeclRefExpr
                if ast.decls.get(ast.exprs.decl(e)).kind() == DeclKind::EnumConstant =>
            {
                let d = ast.exprs.decl(e);
                match ast.decls.enumerator(d).and_then(|x| x.value) {
                    Some(c) => V::C(self.const_scalar_value(c, q)?),
                    None => return err(self.loc_of(e), "enumerator without a value"),
                }
            }
            // clang `ScalarExprEmitter::VisitMemberExpr`: an integral
            // member access that folds (an enumerator, a constant member)
            // is its base, ignored, and the constant.
            ExprKind::MemberExpr
                if self.folds.get(e).is_some()
                    || ast.decls.get(ast.exprs.decl(e)).kind() == DeclKind::EnumConstant =>
            {
                let d = ast.exprs.decl(e);
                let c = match self.folds.get(e) {
                    Some(c) => c,
                    None => match ast.decls.enumerator(d).and_then(|x| x.value) {
                        Some(c) => c,
                        None => return err(self.loc_of(e), "enumerator without a value"),
                    },
                };
                let v = self.const_scalar_value(c, q)?;
                self.emit_ignored(ast.exprs.operand(e))?;
                V::C(v)
            }
            k if k.is_cast() => return self.emit_cast(e),
            ExprKind::UnaryOperator => return self.emit_unary(e).map(Some),
            ExprKind::BinaryOperator => return self.emit_binary(e),
            ExprKind::CompoundAssignOperator => {
                let (lv, v) = self.emit_compound_assign(e)?;
                if self.is_volatile(lv.q) { self.load_lv(lv) } else { v }
            }
            ExprKind::ConditionalOperator => {
                let [a, b] = ast.exprs.cond_arms(e);
                return self.emit_conditional(self.sub(e), a, b);
            }
            ExprKind::BinaryConditionalOperator => {
                let [common, c, a, b] = self.bind_common(e)?;
                let r = self.emit_conditional(c, a, b);
                self.f.opaque.remove(&common);
                return r;
            }
            ExprKind::OpaqueValueExpr => match self.opaque_of(e) {
                Some(Opaque::Rv(v)) => return Ok(v),
                Some(Opaque::Lv(lv)) => self.load_lv(lv),
                None => return err(self.loc_of(e), "unbound OpaqueValueExpr"),
            },
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr => return self.emit_call(e, None),
            ExprKind::CXXRewrittenBinaryOperator => {
                return self.emit_scalar_opt(ast.exprs.rewritten_semantic(e));
            }
            ExprKind::CXXNewExpr => self.emit_cxx_new(e)?,
            ExprKind::CXXDeleteExpr => {
                self.emit_cxx_delete(e)?;
                return Ok(None);
            }
            ExprKind::StmtExpr => return self.emit_stmt_expr(e),
            ExprKind::AtomicExpr => return self.emit_atomic_expr(e, None),
            // A scalar initializer list (clang `VisitInitListExpr`): its
            // single element, or zero.
            // clang `VisitInitListExpr` on a vector: `insertelement` of
            // each element into `poison` (i32 indices), then zeros.
            ExprKind::InitListExpr if self.vec_of(q).is_some() => {
                let inits: Vec<u32> = ast.lists.get(ast.exprs.init_list_inits(e)).to_vec();
                let t = self.lty(q);
                if inits.is_empty() {
                    return Ok(Some(V::C(self.m.const_zero(t))));
                }
                let (et, n, _) = self.m.types.vector_info(t).expect("vector");
                let mut v = V::C(self.m.const_poison(t));
                let mut cur = 0u32;
                for x in inits {
                    let x = self.emit_scalar(ExprId::new(x))?;
                    let idx = self.icn(LTy::I32, i128::from(cur));
                    v = self.insert_elt(v, x, idx);
                    cur += 1;
                }
                while cur < n {
                    let idx = self.icn(LTy::I32, i128::from(cur));
                    let z = V::C(self.m.const_zero(et));
                    v = self.insert_elt(v, z, idx);
                    cur += 1;
                }
                v
            }
            ExprKind::InitListExpr if !self.is_aggregate(q) => {
                if let Some(&x) = ast.lists.get(ast.exprs.init_list_inits(e)).first() {
                    return self.emit_scalar_opt(ExprId::new(x));
                }
                let t = self.lty(q);
                V::C(self.m.const_zero(t))
            }
            // clang `ScalarExprEmitter::VisitArraySubscriptExpr`: a vector
            // subscript is the vector's value and `extractelement`.
            ExprKind::ArraySubscriptExpr if self.vec_subscript(e).is_some() => {
                let (base, idx) = self.vec_subscript(e).expect("checked");
                let v = self.emit_scalar(base)?;
                let i = self.emit_scalar(idx)?;
                self.extract_elt(v, i)
            }
            ExprKind::ConvertVectorExpr => {
                let src = ast.exprs.operand(e);
                let v = self.emit_scalar(src)?;
                let sq = ast.exprs.get(src).ty();
                self.convert_vector(v, sq, q)
            }
            ExprKind::DeclRefExpr
            | ExprKind::MemberExpr
            | ExprKind::ArraySubscriptExpr
            | ExprKind::StringLiteral
            | ExprKind::PredefinedExpr
            | ExprKind::MaterializeTemporaryExpr => {
                let lv = self.emit_lvalue(e)?;
                self.load_lv(lv)
            }
            k => return err(self.loc_of(e), format!("expression {k:?} is not supported yet")),
        }))
    }

    /// clang `EmitLoadOfLValue` for scalars.
    pub(crate) fn load_lv(&mut self, lv: LV) -> V {
        if let Some(idx) = lv.vidx {
            let vec = self.vec_elt_load(lv);
            return self.extract_elt(vec, idx);
        }
        let Some(bf) = lv.bf else {
            if let Some(vq) = self.ast.types.as_atomic(lv.q) {
                return self.atomic_load_lv(lv, vq);
            }
            return self.load_scalar(lv.addr, lv.q);
        };
        let vol = self.is_volatile(lv.q);
        let st = lv.addr.ty;
        let mut v = self.load(st, lv.addr.v, lv.addr.align, vol);
        let ss = bf.storage_size;
        if bf.is_signed {
            let high = ss - bf.offset - bf.size;
            if high != 0 {
                let c = self.icn(st, i128::from(high));
                v = self.bin(BinOp::Shl, Flags::NONE, v, c);
            }
            if bf.offset + high != 0 {
                let c = self.icn(st, i128::from(bf.offset + high));
                v = self.bin(BinOp::AShr, Flags::NONE, v, c);
            }
        } else {
            if bf.offset != 0 {
                let c = self.icn(st, i128::from(bf.offset));
                v = self.bin(BinOp::LShr, Flags::NONE, v, c);
            }
            if bf.offset + bf.size < ss {
                let c = self.icn(st, low_mask(bf.size) as i128);
                v = self.bin(BinOp::And, Flags::NONE, v, c);
            }
        }
        let rt = self.lty(lv.q);
        self.int_cast(v, rt, bf.is_signed)
    }

    /// clang `EmitStoreThroughLValue`; with `want`, also the value of the
    /// bit-field after the store.
    pub(crate) fn store_lv(&mut self, v: V, lv: LV, want: bool) -> V {
        // clang `EmitStoreThroughLValue` of a vector element: read,
        // `insertelement`, write (a `bool` vector through `<P x i1>`).
        if let Some(idx) = lv.vidx {
            let vec = self.vec_elt_load(lv);
            let ins = self.insert_elt(vec, v, idx);
            let raw = if self.m.types.int_width(lv.addr.ty).is_some() {
                self.cast(CastOp::BitCast, ins, lv.addr.ty)
            } else {
                ins
            };
            let vol = self.is_volatile(lv.q);
            self.store(raw, lv.addr.v, lv.addr.align, vol);
            return v;
        }
        let Some(bf) = lv.bf else {
            if let Some(vq) = self.ast.types.as_atomic(lv.q) {
                self.atomic_store_lv(v, lv, vq);
            } else {
                self.store_scalar(v, lv.addr, lv.q);
            }
            return v;
        };
        let vol = self.is_volatile(lv.q);
        let st = lv.addr.ty;
        let ss = bf.storage_size;
        let mut src = self.int_cast(v, st, false);
        let mut masked = src;
        if ss != bf.size {
            let old = self.load(st, lv.addr.v, lv.addr.align, vol);
            if !self.is_bool(lv.q) {
                let m = self.icn(st, low_mask(bf.size) as i128);
                src = self.bin(BinOp::And, Flags::NONE, src, m);
            }
            masked = src;
            if bf.offset != 0 {
                let c = self.icn(st, i128::from(bf.offset));
                src = self.bin(BinOp::Shl, Flags::NONE, src, c);
            }
            let clear = !(low_mask(bf.size) << bf.offset) & low_mask(ss);
            let c = self.icn(st, clear as i128);
            let old = self.bin(BinOp::And, Flags::NONE, old, c);
            src = self.bin(BinOp::Or, Flags::NONE, old, src);
        }
        self.store(src, lv.addr.v, lv.addr.align, vol);
        if !want {
            return v;
        }
        let mut r = masked;
        if bf.is_signed {
            let high = ss - bf.size;
            if high != 0 {
                let c = self.icn(st, i128::from(high));
                r = self.bin(BinOp::Shl, Flags::NONE, r, c);
                r = self.bin(BinOp::AShr, Flags::NONE, r, c);
            }
        }
        let rt = self.lty(lv.q);
        self.int_cast(r, rt, bf.is_signed)
    }

    /// clang `EmitScalarConversion`.
    pub(crate) fn conv(&mut self, v: V, from: QualType, to: QualType) -> V {
        let ast = self.ast;
        let tt = self.lty(to);
        if self.is_bool(to) {
            if self.is_bool(from) {
                return v;
            }
            return self.conv_to_bool(v, from);
        }
        let ft = self.f.fir.ty(&self.m, v);
        if ft == tt {
            return v;
        }
        let fi = self.m.types.int_width(ft).is_some();
        let ti = self.m.types.int_width(tt).is_some();
        if ast.types.is_pointer(to) && fi {
            let w = self.int_cast(v, LTy::I64, self.is_signed(from));
            return self.cast(CastOp::IntToPtr, w, tt);
        }
        if ast.types.is_pointer(from) && ti {
            return self.cast(CastOp::PtrToInt, v, tt);
        }
        if fi {
            let signed = self.is_signed(from) && !self.is_bool(from);
            if ti {
                return self.int_cast(v, tt, signed);
            }
            return self.cast(if signed { CastOp::SIToFP } else { CastOp::UIToFP }, v, tt);
        }
        if ti {
            return self.cast(
                if self.is_signed(to) { CastOp::FPToSI } else { CastOp::FPToUI },
                v,
                tt,
            );
        }
        let fw = self.m.types.float_width(ft).unwrap_or(0);
        let tw = self.m.types.float_width(tt).unwrap_or(0);
        if tw < fw { self.cast(CastOp::FPTrunc, v, tt) } else { self.cast(CastOp::FPExt, v, tt) }
    }

    /// clang `EmitConversionToBool`.
    fn conv_to_bool(&mut self, v: V, from: QualType) -> V {
        let ft = self.f.fir.ty(&self.m, v);
        if ft == LTy::I1 {
            return v;
        }
        if self.is_float_ty(ft) {
            let z = self.fconst(ft, 0.0);
            return self.fcmp(FloatPredicate::Une, v, z);
        }
        let _ = from;
        // clang `EmitIntToBoolConversion`: a logical value zero-extended
        // and wanted as a logical value again is the `i1` itself (the
        // `zext` is zapped when nothing else uses it).
        if let V::I(_) = v {
            let i = Fir::last_inst(v);
            if let Op::Cast { op: CastOp::ZExt, v: inner, .. } = *self.f.fir.op(i)
                && self.f.fir.ty(&self.m, inner) == LTy::I1
            {
                let last = self.f.fir.cur().and_then(|b| self.f.fir.block_insts(b).last().copied());
                if last == Some(i) || self.f.fir.uses(v) == 0 {
                    self.f.fir.erase(i);
                }
                return inner;
            }
        }
        let z = if ft == LTy::PTR { V::C(self.m.const_null(LTy::PTR)) } else { self.icn(ft, 0) };
        self.icmp(IntPredicate::Ne, v, z)
    }

    /// clang `EvaluateExprAsBool`.
    pub(crate) fn eval_bool(&mut self, e: ExprId) -> R<V> {
        let q = self.ast.exprs.get(e).ty();
        let v = self.emit_scalar(e)?;
        Ok(if self.is_bool(q) { v } else { self.conv_to_bool(v, q) })
    }

    fn zext_or_same(&mut self, v: V, to: LTy) -> V {
        let ft = self.f.fir.ty(&self.m, v);
        if ft == to { v } else { self.cast(CastOp::ZExt, v, to) }
    }

    fn emit_cast(&mut self, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        let s = ast.exprs.operand(e);
        let sq = ast.exprs.get(s).ty();
        let ck = ast.exprs.cast_kind(e);
        Ok(Some(match ck {
            CastKind::LValueToRValue => {
                if let Some(c) = self.folds.get(e)
                    && !self.is_aggregate(q)
                {
                    return Ok(Some(V::C(self.const_scalar_value(c, q)?)));
                }
                // A call returning a reference yields the referee's address
                // (clang `ScalarExprEmitter::VisitCallExpr`: a reference
                // result is `EmitLoadOfLValue`).
                let inner = self.skip_parens(s);
                if matches!(
                    ast.exprs.get(inner).kind(),
                    ExprKind::CallExpr
                        | ExprKind::CXXMemberCallExpr
                        | ExprKind::CXXOperatorCallExpr
                ) && self.is_glvalue(inner)
                {
                    let lv = self.emit_lvalue(s)?;
                    return Ok(Some(self.load_lv(lv)));
                }
                return self.emit_scalar_opt(s);
            }
            // clang `ScalarExprEmitter::VisitCastExpr`: a user-defined or
            // constructor conversion is its (call) operand.
            // A vector bitcast reinterprets the value (clang `CK_BitCast`
            // `CreateBitCast`).
            CastKind::BitCast if self.vec_of(q).is_some() || self.vec_of(sq).is_some() => {
                let v = self.emit_scalar(s)?;
                let t = self.lty(q);
                self.cast(CastOp::BitCast, v, t)
            }
            // clang `CK_VectorSplat`: `CreateVectorSplat`.
            CastKind::VectorSplat => {
                let v = self.emit_scalar(s)?;
                let n = self.vec_of(q).map_or(1, |x| x.1);
                self.splat(v, n)
            }
            CastKind::NoOp
            | CastKind::BitCast
            | CastKind::LValueBitCast
            | CastKind::AtomicToNonAtomic
            | CastKind::NonAtomicToAtomic
            | CastKind::UserDefinedConversion
            | CastKind::ConstructorConversion => {
                return self.emit_scalar_opt(s);
            }
            CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase => {
                let p = self.emit_scalar(s)?;
                let Some(derived) = ast.types.pointee(sq).and_then(|t| ast.types.as_record(t))
                else {
                    return err(self.loc_of(e), "derived-to-base cast of a non-class pointer");
                };
                let (vb, off) = self.base_path(e, derived)?;
                let null_check = ck == CastKind::DerivedToBase && !self.is_this_expr(s);
                if vb.is_none() {
                    self.base_ptr(p, off, null_check)
                } else {
                    let ty = self.mty(ast.types.pointee(sq).expect("checked"));
                    let align = self.record_align_of(sq);
                    self.base_address(Addr { v: p, ty, align }, derived, vb, off, null_check)?.0
                }
            }
            // `__builtin_bit_cast` ([bit.cast]): the operand's bytes
            // loaded as the destination type (clang `EmitLoadOfLValue` of
            // the reinterpreted address).
            CastKind::LValueToRValueBitCast => {
                let lv = self.emit_lvalue(s)?;
                let a = Addr { ty: self.mty(q), ..lv.addr };
                return Ok(Some(self.load_scalar(a, q)));
            }
            CastKind::ToVoid => {
                self.emit_ignored(s)?;
                return Ok(None);
            }
            CastKind::Dynamic => {
                let p = self.emit_scalar(s)?;
                let align = self.record_align_of(sq);
                self.emit_dynamic_cast(e, p, align)?
            }
            CastKind::ArrayToPointerDecay => {
                let lv = self.emit_lvalue(s)?;
                self.decay(lv.addr)
            }
            CastKind::FunctionToPointerDecay | CastKind::BuiltinFnToFnPtr => {
                self.emit_lvalue(s)?.addr.v
            }
            CastKind::NullToPointer => {
                // clang `MustVisitNullValue`: a `nullptr_t` operand is
                // evaluated for its side effects.
                let sq = ast.exprs.get(s).ty();
                if matches!(
                    ast.types.canonical_view(sq),
                    kiln_ast::types::TypeView::Builtin(kiln_ast::types::BuiltinType::NullPtr)
                ) {
                    self.emit_ignored(s)?;
                }
                V::C(self.m.const_null(LTy::PTR))
            }
            CastKind::NullToMemberPointer => {
                if matches!(
                    ast.types.canonical_view(sq),
                    kiln_ast::types::TypeView::Builtin(kiln_ast::types::BuiltinType::NullPtr)
                ) {
                    self.emit_ignored(s)?;
                }
                self.null_mp(q)
            }
            CastKind::BaseToDerivedMemberPointer | CastKind::DerivedToBaseMemberPointer => {
                let v = self.emit_scalar(s)?;
                self.mp_convert(e, v)?
            }
            CastKind::MemberPointerToBoolean => {
                let v = self.emit_scalar(s)?;
                self.mp_is_not_null(v, sq)
            }
            CastKind::IntegralToPointer => {
                let v = self.emit_scalar(s)?;
                let w = self.int_cast(v, LTy::I64, self.is_signed(sq));
                self.cast(CastOp::IntToPtr, w, LTy::PTR)
            }
            CastKind::PointerToIntegral => {
                let v = self.emit_scalar(s)?;
                let t = self.lty(q);
                self.cast(CastOp::PtrToInt, v, t)
            }
            CastKind::BooleanToSignedIntegral => {
                let v = self.emit_scalar(s)?;
                let t = self.lty(q);
                self.cast(CastOp::SExt, v, t)
            }
            CastKind::IntegralCast
            | CastKind::IntegralToBoolean
            | CastKind::IntegralToFloating
            | CastKind::FloatingToIntegral
            | CastKind::FloatingToBoolean
            | CastKind::FloatingCast
            | CastKind::PointerToBoolean => {
                let v = self.emit_scalar(s)?;
                self.conv(v, sq, q)
            }
            k => return err(self.loc_of(e), format!("cast {k:?} is not supported yet")),
        }))
    }

    /// `gep inbounds [N x T], p, i64 0, i64 0`.
    pub(crate) fn decay(&mut self, a: Addr) -> V {
        let z = self.i64c(0);
        self.gep(a.ty, a.v, vec![z, z], true)
    }

    fn emit_unary(&mut self, e: ExprId) -> R<V> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        let s = ast.exprs.operand(e);
        match ast.exprs.unary_op(e) {
            UnaryOp::Plus | UnaryOp::Extension => self.emit_scalar(s),
            UnaryOp::Minus => {
                let v = self.emit_scalar(s)?;
                let t = self.f.fir.ty(&self.m, v);
                if self.is_float_ty(t) {
                    return Ok(self.fneg(v));
                }
                let z = self.icn(t, 0);
                let fl = if self.is_signed(q) { Flags::NSW } else { Flags::NONE };
                Ok(self.bin(BinOp::Sub, fl, z, v))
            }
            UnaryOp::Not => {
                let v = self.emit_scalar(s)?;
                let t = self.f.fir.ty(&self.m, v);
                let m = self.icn(t, -1);
                Ok(self.bin(BinOp::Xor, Flags::NONE, v, m))
            }
            // clang `VisitUnaryLNot` on a vector: a comparison with zero.
            UnaryOp::LNot if self.vec_of(q).is_some() => {
                let v = self.emit_scalar(s)?;
                let t = self.f.fir.ty(&self.m, v);
                let z = V::C(self.m.const_zero(t));
                let c = if self.is_float_ty(t) {
                    self.fcmp(FloatPredicate::Oeq, v, z)
                } else {
                    self.icmp(IntPredicate::Eq, v, z)
                };
                Ok(self.vec_cmp_result(c, q))
            }
            UnaryOp::LNot => {
                let b = self.eval_bool(s)?;
                let t = V::C(self.m.const_bool(true));
                let x = self.bin(BinOp::Xor, Flags::NONE, b, t);
                let rt = self.lty(q);
                Ok(self.zext_or_same(x, rt))
            }
            UnaryOp::AddrOf if self.is_mp(q) => self.mp_addr_of(e, q),
            UnaryOp::AddrOf => Ok(self.emit_lvalue(s)?.addr.v),
            UnaryOp::Deref => {
                let lv = self.emit_lvalue(e)?;
                Ok(self.load_lv(lv))
            }
            op @ (UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec) => {
                let lv = self.emit_lvalue(s)?;
                self.inc_dec(e, lv, op)
            }
            op => err(self.loc_of(e), format!("unary {op:?} is not supported yet")),
        }
    }

    /// clang `EmitScalarPrePostIncDec`.
    fn inc_dec(&mut self, e: ExprId, lv: LV, op: UnaryOp) -> R<V> {
        let inc = matches!(op, UnaryOp::PreInc | UnaryOp::PostInc);
        let pre = matches!(op, UnaryOp::PreInc | UnaryOp::PreDec);
        if lv.bf.is_none()
            && let Some(vq) = self.ast.types.as_atomic(lv.q)
        {
            return self.atomic_inc_dec(e, lv, vq, inc, pre);
        }
        let old = self.load_lv(lv);
        let new = self.inc_dec_value(e, lv.q, old, inc)?;
        let stored = self.store_lv(new, lv, lv.bf.is_some());
        Ok(if pre { stored } else { old })
    }

    /// The incremented (`inc`) or decremented value `old` of type `q`.
    pub(crate) fn inc_dec_value(&mut self, e: ExprId, q: QualType, old: V, inc: bool) -> R<V> {
        let ast = self.ast;
        let amount: i128 = if inc { 1 } else { -1 };
        Ok(if self.is_bool(q) && inc {
            V::C(self.m.const_bool(true))
        } else if ast.types.is_pointer(q) {
            let pq = ast.types.pointee(q).expect("pointee");
            let et = if ast.types.is_void(pq) || ast.types.is_function(pq) {
                LTy::I8
            } else {
                self.mty(pq)
            };
            let c = self.icn(LTy::I32, amount);
            self.gep(et, old, vec![c], true)
        } else {
            let t = self.f.fir.ty(&self.m, old);
            if self.is_float_ty(t) {
                let c = self.fconst(t, if inc { 1.0 } else { -1.0 });
                self.bin(BinOp::FAdd, Flags::NONE, old, c)
            } else {
                let c = self.icn(t, amount);
                let can_overflow = !ast.exprs.get(e).has(ExprFlags::CANNOT_OVERFLOW);
                let fl = if can_overflow && self.is_signed(q) { Flags::NSW } else { Flags::NONE };
                self.bin(BinOp::Add, fl, old, c)
            }
        })
    }

    fn emit_binary(&mut self, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        let op = ast.exprs.binary_op(e);
        let [l, r] = ast.exprs.operands(e);
        Ok(Some(match op {
            BinaryOp::Assign => {
                if self.is_aggregate(ast.exprs.get(e).ty()) {
                    let lv = self.emit_lvalue(e)?;
                    return Ok(Some(lv.addr.v));
                }
                let v = self.emit_scalar(r)?;
                let lv = self.emit_lvalue(l)?;
                let res = self.store_lv(v, lv, true);
                if self.is_volatile(lv.q) { self.load_lv(lv) } else { res }
            }
            BinaryOp::Comma => {
                self.emit_ignored(l)?;
                self.ensure_insert_point();
                return self.emit_scalar_opt(r);
            }
            BinaryOp::LAnd | BinaryOp::LOr => self.emit_logical(e, op == BinaryOp::LAnd)?,
            BinaryOp::PtrMemD | BinaryOp::PtrMemI => {
                let lv = self.emit_lvalue(e)?;
                self.load_lv(lv)
            }
            BinaryOp::EQ | BinaryOp::NE if self.is_mp(ast.exprs.get(l).ty()) => {
                let lv = self.emit_scalar(l)?;
                let rv = self.emit_scalar(r)?;
                let lq = ast.exprs.get(l).ty();
                self.mp_compare(lv, rv, lq, op == BinaryOp::NE)
            }
            _ => {
                let lv = self.emit_scalar(l)?;
                let rv = self.emit_scalar(r)?;
                let lq = ast.exprs.get(l).ty();
                let rq = ast.exprs.get(r).ty();
                let q = ast.exprs.get(e).ty();
                self.arith(e, op, lv, rv, lq, rq, q)?
            }
        }))
    }

    /// Binary arithmetic, comparison and pointer arithmetic on emitted
    /// operands (`q` is the computation type).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn arith(
        &mut self,
        e: ExprId,
        op: BinaryOp,
        l: V,
        r: V,
        lq: QualType,
        rq: QualType,
        q: QualType,
    ) -> R<V> {
        let ast = self.ast;
        let lp = ast.types.is_pointer(lq);
        let rp = ast.types.is_pointer(rq);
        let lt = self.f.fir.ty(&self.m, l);
        let float = self.is_float_ty(lt);
        // clang `hasSignedIntegerRepresentation`: a vector's elements;
        // `nsw` only for scalars (`isSignedIntegerOrEnumerationType`).
        let signed = self.is_signed(self.scalar_of(q));
        let nsw = if self.is_signed(q) { Flags::NSW } else { Flags::NONE };
        Ok(match op {
            BinaryOp::Add | BinaryOp::Sub if lp && rp => {
                let a = self.cast(CastOp::PtrToInt, l, LTy::I64);
                let b = self.cast(CastOp::PtrToInt, r, LTy::I64);
                let d = self.bin(BinOp::Sub, Flags::NONE, a, b);
                let pq = ast.types.pointee(lq).expect("pointee");
                let sz = if ast.types.is_void(pq) || ast.types.is_function(pq) {
                    1
                } else {
                    self.size_align(pq).0
                };
                if sz == 1 {
                    d
                } else {
                    let c = self.i64c(i128::from(sz));
                    self.bin(BinOp::SDiv, Flags::EXACT, d, c)
                }
            }
            BinaryOp::Add | BinaryOp::Sub if lp || rp => {
                let sub = op == BinaryOp::Sub;
                let (p, pq, idx, iq) = if lp || sub { (l, lq, r, rq) } else { (r, rq, l, lq) };
                let is = self.is_signed(iq);
                let mut idx = self.int_cast(idx, LTy::I64, is);
                if sub {
                    let z = self.i64c(0);
                    idx = self.bin(BinOp::Sub, Flags::NONE, z, idx);
                }
                let et = ast.types.pointee(pq).expect("pointee");
                let ety = if ast.types.is_void(et) || ast.types.is_function(et) {
                    LTy::I8
                } else {
                    self.mty(et)
                };
                self.gep(ety, p, vec![idx], true)
            }
            BinaryOp::Add => {
                if float {
                    match self.try_fmuladd(l, r, false) {
                        Some(v) => v,
                        None => self.bin(BinOp::FAdd, Flags::NONE, l, r),
                    }
                } else {
                    self.bin(BinOp::Add, nsw, l, r)
                }
            }
            BinaryOp::Sub => {
                if float {
                    match self.try_fmuladd(l, r, true) {
                        Some(v) => v,
                        None => self.bin(BinOp::FSub, Flags::NONE, l, r),
                    }
                } else {
                    self.bin(BinOp::Sub, nsw, l, r)
                }
            }
            BinaryOp::Mul => {
                if float {
                    self.bin(BinOp::FMul, Flags::NONE, l, r)
                } else {
                    self.bin(BinOp::Mul, nsw, l, r)
                }
            }
            BinaryOp::Div => {
                if float {
                    self.bin(BinOp::FDiv, Flags::NONE, l, r)
                } else {
                    self.bin(if signed { BinOp::SDiv } else { BinOp::UDiv }, Flags::NONE, l, r)
                }
            }
            BinaryOp::Rem => {
                if float {
                    self.bin(BinOp::FRem, Flags::NONE, l, r)
                } else {
                    self.bin(if signed { BinOp::SRem } else { BinOp::URem }, Flags::NONE, l, r)
                }
            }
            BinaryOp::Shl | BinaryOp::Shr => {
                let rt = self.f.fir.ty(&self.m, r);
                let r = if rt == lt { r } else { self.int_cast(r, lt, false) };
                let o = if op == BinaryOp::Shl {
                    BinOp::Shl
                } else if signed {
                    BinOp::AShr
                } else {
                    BinOp::LShr
                };
                self.bin(o, Flags::NONE, l, r)
            }
            BinaryOp::And => self.bin(BinOp::And, Flags::NONE, l, r),
            BinaryOp::Or => self.bin(BinOp::Or, Flags::NONE, l, r),
            BinaryOp::Xor => self.bin(BinOp::Xor, Flags::NONE, l, r),
            BinaryOp::LT
            | BinaryOp::GT
            | BinaryOp::LE
            | BinaryOp::GE
            | BinaryOp::EQ
            | BinaryOp::NE => {
                if float {
                    let p = match op {
                        BinaryOp::LT => FloatPredicate::Olt,
                        BinaryOp::GT => FloatPredicate::Ogt,
                        BinaryOp::LE => FloatPredicate::Ole,
                        BinaryOp::GE => FloatPredicate::Oge,
                        BinaryOp::EQ => FloatPredicate::Oeq,
                        _ => FloatPredicate::Une,
                    };
                    let c = self.fcmp(p, l, r);
                    self.vec_cmp_result(c, q)
                } else {
                    let s = !lp && self.is_signed(self.scalar_of(lq));
                    let p = match (op, s) {
                        (BinaryOp::LT, true) => IntPredicate::Slt,
                        (BinaryOp::LT, false) => IntPredicate::Ult,
                        (BinaryOp::GT, true) => IntPredicate::Sgt,
                        (BinaryOp::GT, false) => IntPredicate::Ugt,
                        (BinaryOp::LE, true) => IntPredicate::Sle,
                        (BinaryOp::LE, false) => IntPredicate::Ule,
                        (BinaryOp::GE, true) => IntPredicate::Sge,
                        (BinaryOp::GE, false) => IntPredicate::Uge,
                        (BinaryOp::EQ, _) => IntPredicate::Eq,
                        _ => IntPredicate::Ne,
                    };
                    let c = self.icmp(p, l, r);
                    self.vec_cmp_result(c, q)
                }
            }
            op => return err(self.loc_of(e), format!("binary {op:?} is not supported yet")),
        })
    }

    /// The `fneg` instruction `v` negates, if unused with a single-use
    /// operand (clang `tryEmitFMulAdd` peeking through `fneg`).
    fn peek_fneg(&self, v: V) -> Option<V> {
        let V::I(i) = v else { return None };
        let Op::FNeg { v: x } = *self.f.fir.op(i) else { return None };
        (self.f.fir.uses(v) == 0 && self.f.fir.uses(x) == 1).then_some(x)
    }

    fn is_fmul(&self, v: V) -> bool {
        let V::I(i) = v else { return false };
        matches!(self.f.fir.op(i), Op::Bin { op: BinOp::FMul, .. })
    }

    /// clang `tryEmitFMulAdd` (`-ffp-contract=on`): fuse an unused `fmul`
    /// operand of an `fadd`/`fsub` into `llvm.fmuladd`.
    fn try_fmuladd(&mut self, l: V, r: V, sub: bool) -> Option<V> {
        let nl = self.peek_fneg(l);
        let nr = self.peek_fneg(r);
        let lm = nl.unwrap_or(l);
        let rm = nr.unwrap_or(r);
        if self.is_fmul(lm) && (nl.is_some() || self.f.fir.uses(lm) == 0) {
            if nl.is_some() {
                self.f.fir.erase(Fir::last_inst(l));
            }
            return Some(self.build_fmuladd(lm, r, nl.is_some(), sub));
        }
        if self.is_fmul(rm) && (nr.is_some() || self.f.fir.uses(rm) == 0) {
            if nr.is_some() {
                self.f.fir.erase(Fir::last_inst(r));
            }
            return Some(self.build_fmuladd(rm, l, nr.is_some(), !sub));
        }
        None
    }

    fn build_fmuladd(&mut self, mul: V, addend: V, neg_mul: bool, neg_add: bool) -> V {
        let mi = Fir::last_inst(mul);
        let Op::Bin { l: a, r: b, .. } = *self.f.fir.op(mi) else { unreachable!("fmul") };
        let a = if neg_mul { self.fneg(a) } else { a };
        let addend = if neg_add { self.fneg(addend) } else { addend };
        let ty = self.f.fir.ty(&self.m, addend);
        let name = kiln_llvm::intrinsics::overloaded_name(&self.m, "llvm.fmuladd", &[ty]);
        let fn_ty = self.m.types.function(ty, &[ty, ty, ty], false);
        let f = kiln_llvm::intrinsics::intrinsic(&mut self.m, &name, fn_ty)
            .expect("fmuladd declaration");
        let fa = self.m.attr_set(&[
            Attr::Flag(AttrKind::NoCallback),
            Attr::Flag(AttrKind::NoFree),
            Attr::Flag(AttrKind::NoSync),
            Attr::Flag(AttrKind::NoUnwind),
            Attr::Flag(AttrKind::Speculatable),
            Attr::Flag(AttrKind::WillReturn),
            Attr::Memory(kiln_llvm::MemoryEffects::all(kiln_llvm::ModRef::None)),
        ]);
        self.m.func_mut(f).attrs.func = fa;
        let c = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(f)),
                args: vec![a, b, addend],
                opts: Box::default(),
            },
            ty,
        );
        self.f.calls.push((Fir::last_inst(c), Some(f), true));
        self.f.fir.erase(mi);
        c
    }

    /// clang `VisitBinLAnd` / `VisitBinLOr`.
    fn emit_logical(&mut self, e: ExprId, and: bool) -> R<V> {
        let ast = self.ast;
        let [l, r] = ast.exprs.operands(e);
        let rt = self.lty(ast.exprs.get(e).ty());
        if let Some(b) = self.fold_bool(l) {
            if b == and {
                let v = self.eval_bool(r)?;
                return Ok(self.zext_or_same(v, rt));
            }
            return Ok(V::C(self.m.const_int(rt, i128::from(!and))));
        }
        let end = self.f.fir.create_block();
        let rhs = self.f.fir.create_block();
        let start = self.f.fir.cur();
        if and {
            self.branch_on_bool(l, rhs, end)?;
        } else {
            self.branch_on_bool(l, end, rhs)?;
        }
        let preds = self.f.fir.predecessors(end);
        let k = V::C(self.m.const_bool(!and));
        let phi = self.f.fir.push_in(
            end,
            Op::Phi { ty: LTy::I1, inc: preds.iter().map(|&p| (k, p)).collect() },
            LTy::I1,
        );
        self.emit_block(rhs, false);
        let v = self.cond_eval(start, |cg| cg.eval_bool(r))?;
        let rb = self.f.fir.cur().expect("insert point");
        self.emit_block(end, false);
        self.f.fir.add_incoming(Fir::last_inst(phi), v, rb);
        Ok(self.zext_or_same(phi, rt))
    }

    /// Runs `f` as conditionally evaluated code (clang
    /// `ConditionalEvaluation::begin`/`end`); `start` is the block the
    /// condition was branched from.
    fn cond_eval<T>(
        &mut self,
        start: Option<crate::fir::Blk>,
        f: impl FnOnce(&mut Self) -> R<T>,
    ) -> R<T> {
        let Some(b) = start else { return f(self) };
        self.f.conds.push(b);
        let r = f(self);
        self.f.conds.pop();
        r
    }

    /// clang `EmitBranchOnBoolExpr`.
    pub(crate) fn branch_on_bool(
        &mut self,
        e: ExprId,
        t: crate::fir::Blk,
        f: crate::fir::Blk,
    ) -> R<()> {
        let ast = self.ast;
        let mut e = e;
        while ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = ast.exprs.operand(e);
        }
        let n = *ast.exprs.get(e);
        match n.kind() {
            ExprKind::BinaryOperator
                if matches!(ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
            {
                let and = ast.exprs.binary_op(e) == BinaryOp::LAnd;
                let [l, r] = ast.exprs.operands(e);
                if self.fold_bool(l) == Some(and) {
                    return self.branch_on_bool(r, t, f);
                }
                if self.fold_bool(r) == Some(and) {
                    return self.branch_on_bool(l, t, f);
                }
                let mid = self.f.fir.create_block();
                let start = self.f.fir.cur();
                if and {
                    self.branch_on_bool(l, mid, f)?;
                } else {
                    self.branch_on_bool(l, t, mid)?;
                }
                self.emit_block(mid, false);
                self.cond_eval(start, |cg| cg.branch_on_bool(r, t, f))
            }
            ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::LNot => {
                self.branch_on_bool(ast.exprs.operand(e), f, t)
            }
            ExprKind::ConditionalOperator => {
                let c = self.sub(e);
                let [a, b] = ast.exprs.cond_arms(e);
                let lb = self.f.fir.create_block();
                let rb = self.f.fir.create_block();
                let start = self.f.fir.cur();
                self.branch_on_bool(c, lb, rb)?;
                self.emit_block(lb, false);
                self.cond_eval(start, |cg| cg.branch_on_bool(a, t, f))?;
                self.emit_block(rb, false);
                self.cond_eval(start, |cg| cg.branch_on_bool(b, t, f))
            }
            // `br(c ? throw x : y, t, f)` folds to `br(c, throw x, br(y, t, f))`.
            ExprKind::CXXThrowExpr => self.emit_throw(e, false),
            _ => {
                let v = self.eval_bool(e)?;
                self.cond_br(v, t, f);
                Ok(())
            }
        }
    }

    /// clang `OpaqueValueMapping` for a `BinaryConditionalOperator`:
    /// evaluates the common expression once and binds it; returns the
    /// operands `[common, cond, then, else]`.
    fn bind_common(&mut self, e: ExprId) -> R<[ExprId; 4]> {
        let ast = self.ast;
        let ops = ast.exprs.binary_conditional_operands(e, &ast.lists);
        let v = if self.is_glvalue(ops[0]) {
            Opaque::Lv(self.emit_lvalue(ops[0])?)
        } else {
            Opaque::Rv(self.emit_scalar_opt(ops[0])?)
        };
        self.f.opaque.insert(ops[0], v);
        Ok(ops)
    }

    fn opaque_of(&self, e: ExprId) -> Option<Opaque> {
        // A handler's copy-initialization source has no source
        // expression: it is bound by itself.
        let src = self.ast.exprs.opaque_source(e).unwrap_or(e);
        self.f.opaque.get(&src).copied()
    }

    /// clang `VisitAbstractConditionalOperator`.
    fn emit_conditional(&mut self, c: ExprId, a: ExprId, b: ExprId) -> R<Option<V>> {
        if let Some(x) = self.fold_bool(c) {
            return self.emit_scalar_opt(if x { a } else { b });
        }
        if self.folds.get(a).is_some() && self.folds.get(b).is_some() {
            let cv = self.eval_bool(c)?;
            let _ = self.zext_or_same(cv, LTy::I64);
            let x = self.emit_scalar_opt(a)?;
            let y = self.emit_scalar_opt(b)?;
            return Ok(match (x, y) {
                (Some(x), Some(y)) => Some(self.select(cv, x, y)),
                _ => None,
            });
        }
        let lb = self.f.fir.create_block();
        let rb = self.f.fir.create_block();
        let cont = self.f.fir.create_block();
        let start = self.f.fir.cur();
        self.branch_on_bool(c, lb, rb)?;
        self.emit_block(lb, false);
        let x = self.cond_eval(start, |cg| cg.emit_scalar_opt(a))?;
        let lb = self.f.fir.cur().expect("insert point");
        self.br(cont);
        self.emit_block(rb, false);
        let y = self.cond_eval(start, |cg| cg.emit_scalar_opt(b))?;
        let rb = self.f.fir.cur().expect("insert point");
        self.emit_block(cont, false);
        Ok(match (x, y) {
            (Some(x), Some(y)) => {
                let ty = self.f.fir.ty(&self.m, x);
                Some(self.f.fir.push(Op::Phi { ty, inc: vec![(x, lb), (y, rb)] }, ty))
            }
            (x, None) => x,
            (None, y) => y,
        })
    }

    /// clang `EmitCompoundAssignLValue`: the lvalue and the stored value.
    fn emit_compound_assign(&mut self, e: ExprId) -> R<(LV, V)> {
        let ast = self.ast;
        let ca = *ast.exprs.compound_assign(e);
        let l = self.sub(e);
        let op = match ast.exprs.binary_op(e) {
            BinaryOp::MulAssign => BinaryOp::Mul,
            BinaryOp::DivAssign => BinaryOp::Div,
            BinaryOp::RemAssign => BinaryOp::Rem,
            BinaryOp::AddAssign => BinaryOp::Add,
            BinaryOp::SubAssign => BinaryOp::Sub,
            BinaryOp::ShlAssign => BinaryOp::Shl,
            BinaryOp::ShrAssign => BinaryOp::Shr,
            BinaryOp::AndAssign => BinaryOp::And,
            BinaryOp::XorAssign => BinaryOp::Xor,
            _ => BinaryOp::Or,
        };
        let rq = ast.exprs.get(ca.rhs).ty();
        let rv = self.emit_scalar(ca.rhs)?;
        let lv = self.emit_lvalue(l)?;
        let lq = lv.q;
        if lv.bf.is_none()
            && let Some(vq) = ast.types.as_atomic(lq)
        {
            let v = self.atomic_compound_assign(e, op, lv, vq, rv, rq, ca.lhs_ty, ca.result_ty)?;
            return Ok((lv, v));
        }
        let old = self.load_lv(lv);
        let lhs = if ast.types.is_pointer(lq) { old } else { self.conv(old, lq, ca.lhs_ty) };
        let res = self.arith(e, op, lhs, rv, ca.lhs_ty, rq, ca.result_ty)?;
        let res = if ast.types.is_pointer(lq) { res } else { self.conv(res, ca.result_ty, lq) };
        let v = self.store_lv(res, lv, true);
        Ok((lv, v))
    }

    // ---- lvalues -------------------------------------------------------

    /// The address of a variable or function named by a `DeclRefExpr`.
    fn decl_lvalue(&mut self, e: ExprId, d: DeclId, q: QualType) -> R<LV> {
        let ast = self.ast;
        match ast.decls.get(d).kind() {
            DeclKind::Function | DeclKind::CXXMethod => {
                let f = self.get_fn(d)?;
                Ok(LV {
                    addr: Addr { v: V::C(Value::func(f)), ty: LTy::PTR, align: 1 },
                    q,
                    bf: None,
                    vidx: None,
                })
            }
            // clang `EmitDeclRefLValue`: a structured binding is its
            // binding expression ([dcl.struct.bind]), or its capture.
            DeclKind::Binding => {
                if ast.exprs.get(e).has(ExprFlags::REFERS_TO_ENCLOSING)
                    && let Some(lv) = self.captured_var_lvalue(d, q)?
                {
                    return Ok(lv);
                }
                let Some(b) = ast.decls.binding_expr(d) else {
                    return err(Some(ast.decls.get(d).loc()), "unbound binding");
                };
                let lv = self.emit_lvalue(b)?;
                Ok(LV { q, ..lv })
            }
            DeclKind::Var | DeclKind::ParmVar => {
                if ast.exprs.get(e).has(ExprFlags::REFERS_TO_ENCLOSING)
                    && let Some(lv) = self.captured_var_lvalue(d, q)?
                {
                    return Ok(lv);
                }
                let dq = ast.decls.get(d).ty().unwrap_or(q);
                if self.referee(dq).is_some() {
                    // clang `EmitLoadOfReferenceLValue`: the slot holds the
                    // address of the referee.
                    let slot = if let Some(&a) = self.f.locals.get(&d) {
                        a
                    } else {
                        let g = self.ref_var_global(e, d)?;
                        Addr { v: V::C(gval(g)), ty: LTy::PTR, align: self.global_align(dq) }
                    };
                    let p = self.load(LTy::PTR, slot.v, slot.align, false);
                    return Ok(self.ref_lvalue(p, q));
                }
                if let Some(&a) = self.f.locals.get(&d) {
                    return Ok(LV { addr: a, q, bf: None, vidx: None });
                }
                let ty = self.mty(q);
                let st = ast.decls.var(d).map(|v| v.spec.storage());
                let g = if is_local(ast, d) && st == Some(VarStorage::Static) {
                    self.static_local(d)?
                } else if is_local(ast, d) && st != Some(VarStorage::Extern) {
                    return err(self.loc_of(e), "reference to an unemitted local variable");
                } else {
                    self.get_gvar(d)?
                };
                let dq = ast.decls.get(d).ty().unwrap_or(q);
                let align = self.global_align(dq);
                Ok(LV { addr: Addr { v: V::C(gval(g)), ty, align }, q, bf: None, vidx: None })
            }
            DeclKind::TemplateParamObject => {
                let g = self.param_object_global(d)?;
                let dq = ast.decls.get(d).ty().unwrap_or(q);
                let ty = self.mty(dq);
                let align = self.global_align(dq);
                Ok(LV { addr: Addr { v: V::C(gval(g)), ty, align }, q, bf: None, vidx: None })
            }
            k => err(self.loc_of(e), format!("reference to {k:?} is not supported yet")),
        }
    }

    /// The global holding a namespace-scope or static reference variable.
    fn ref_var_global(&mut self, e: ExprId, d: DeclId) -> R<kiln_llvm::GlobalId> {
        let ast = self.ast;
        let st = ast.decls.var(d).map(|v| v.spec.storage());
        if is_local(ast, d) && st == Some(VarStorage::Static) {
            self.static_local(d)
        } else if is_local(ast, d) && st != Some(VarStorage::Extern) {
            err(self.loc_of(e), "reference to an unemitted local variable")
        } else {
            self.get_gvar(d)
        }
    }

    /// The natural alignment of the object pointer type `q` points to.
    pub(crate) fn record_align_of(&mut self, q: QualType) -> u64 {
        let p = self.ast.types.pointee(q).unwrap_or(q);
        self.pointee_layout(p).1
    }

    /// The lvalue of type `q` at the address `p` a reference holds.
    pub(crate) fn ref_lvalue(&mut self, p: V, q: QualType) -> LV {
        let ty = if self.ast.types.is_function(q) { LTy::PTR } else { self.mty(q) };
        let (_, align) = self.pointee_layout(q);
        LV { addr: Addr { v: p, ty, align }, q, bf: None, vidx: None }
    }

    /// clang `EmitLValue`.
    pub(crate) fn emit_lvalue(&mut self, e: ExprId) -> R<LV> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        match n.kind() {
            ExprKind::ParenExpr => self.emit_lvalue(ast.exprs.operand(e)),
            ExprKind::CXXDefaultArgExpr => match default_arg_of(ast, e) {
                Some(d) => {
                    let saved = self.enter_default_arg(e);
                    let r = self.emit_lvalue(d);
                    self.srcloc = saved;
                    r
                }
                None => err(self.loc_of(e), "default argument not yet parsed"),
            },
            ExprKind::ExprWithCleanups => {
                let sc = self.begin_scope(false);
                let r = self.emit_lvalue(self.sub(e));
                self.end_scope(sc);
                r
            }
            ExprKind::CXXDefaultInitExpr => match self.default_member_init(e) {
                Some(init) => {
                    let saved = self.enter_default_arg(e);
                    let r = self.emit_lvalue(init);
                    self.srcloc = saved;
                    r
                }
                None => err(self.loc_of(e), "default member initializer not yet parsed"),
            },
            ExprKind::CXXMemberCallExpr if self.is_glvalue(e) => {
                let Some(p) = self.emit_member_call(e, None)? else {
                    return err(self.loc_of(e), "reference-returning call produced no value");
                };
                Ok(self.ref_lvalue(p, q))
            }
            ExprKind::ConstantExpr => self.emit_lvalue(ast.exprs.operand(e)),
            ExprKind::CXXTypeidExpr => {
                let p = self.emit_typeid(e)?;
                Ok(self.ref_lvalue(p, q))
            }
            ExprKind::SubstNonTypeTemplateParmExpr => self.emit_lvalue(ExprId::new(n.b())),
            ExprKind::DeclRefExpr => self.decl_lvalue(e, ast.exprs.decl(e), q),
            // clang `EmitPredefinedLValue`: the literal's global, named
            // `<identifier>.<current function>` when first emitted.
            ExprKind::PredefinedExpr => {
                let Some(sl) = ast.exprs.predefined_name(e) else {
                    return err(self.loc_of(e), "dependent predefined identifier");
                };
                let lit = ast.exprs.string_lit(sl);
                let fname = self.f.func.map(|f| self.m.func(f).name().to_owned());
                let g = match fname {
                    Some(fname) => {
                        let name = format!("{}.{}", ast.exprs.predefined_kind(e).spelling(), fname);
                        let bytes = ast.strings.bytes(lit).to_vec();
                        let mut bytes = bytes;
                        let total = ast.strings.object_bytes(lit);
                        bytes.resize(total as usize, 0);
                        self.string_global_named(&bytes, 1, total, &name)
                    }
                    None => self.lit_global(lit),
                };
                let ty = self.m.global(g).value_type();
                let align = self.m.global(g).align.unwrap_or(1);
                Ok(LV { addr: Addr { v: V::C(gval(g)), ty, align }, q, bf: None, vidx: None })
            }
            ExprKind::StringLiteral => {
                let g = self.lit_global(ast.exprs.string_lit(e));
                let ty = self.m.global(g).value_type();
                let align = self.m.global(g).align.unwrap_or(1);
                Ok(LV { addr: Addr { v: V::C(gval(g)), ty, align }, q, bf: None, vidx: None })
            }
            ExprKind::MemberExpr => self.member_lvalue(e),
            ExprKind::ArraySubscriptExpr => self.subscript_lvalue(e),
            ExprKind::UnaryOperator => match ast.exprs.unary_op(e) {
                UnaryOp::Deref => {
                    let p = self.emit_scalar(ast.exprs.operand(e))?;
                    let ty = if ast.types.is_function(q) { LTy::PTR } else { self.mty(q) };
                    let align = self.natural_align(q);
                    Ok(LV { addr: Addr { v: p, ty, align }, q, bf: None, vidx: None })
                }
                op @ (UnaryOp::PreInc | UnaryOp::PreDec) => {
                    let lv = self.emit_lvalue(ast.exprs.operand(e))?;
                    self.inc_dec(e, lv, op)?;
                    Ok(lv)
                }
                UnaryOp::Extension => self.emit_lvalue(ast.exprs.operand(e)),
                op => err(self.loc_of(e), format!("lvalue unary {op:?} is not supported yet")),
            },
            ExprKind::BinaryOperator => {
                let [l, r] = ast.exprs.operands(e);
                match ast.exprs.binary_op(e) {
                    BinaryOp::PtrMemD | BinaryOp::PtrMemI => self.mp_data_lvalue(e),
                    BinaryOp::Assign => {
                        if self.is_aggregate(q) {
                            let lv = self.emit_lvalue(l)?;
                            self.emit_agg(r, lv.addr, false)?;
                            return Ok(lv);
                        }
                        let v = self.emit_scalar(r)?;
                        let lv = self.emit_lvalue(l)?;
                        self.store_lv(v, lv, false);
                        Ok(lv)
                    }
                    BinaryOp::Comma => {
                        self.emit_ignored(l)?;
                        self.ensure_insert_point();
                        self.emit_lvalue(r)
                    }
                    op => err(self.loc_of(e), format!("lvalue binary {op:?} is not supported yet")),
                }
            }
            ExprKind::CompoundAssignOperator => Ok(self.emit_compound_assign(e)?.0),
            ExprKind::ConditionalOperator => {
                let [a, b] = ast.exprs.cond_arms(e);
                self.cond_lvalue(e, self.sub(e), a, b)
            }
            ExprKind::BinaryConditionalOperator => {
                let [common, c, a, b] = self.bind_common(e)?;
                let r = self.cond_lvalue(e, c, a, b);
                self.f.opaque.remove(&common);
                r
            }
            ExprKind::OpaqueValueExpr => match self.opaque_of(e) {
                Some(Opaque::Lv(lv)) => Ok(lv),
                _ => err(self.loc_of(e), "unbound glvalue OpaqueValueExpr"),
            },
            ExprKind::CXXRewrittenBinaryOperator => {
                self.emit_lvalue(ast.exprs.rewritten_semantic(e))
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr if self.is_glvalue(e) => {
                // A call returning a reference.
                let Some(p) = self.emit_call(e, None)? else {
                    return err(self.loc_of(e), "reference-returning call produced no value");
                };
                Ok(self.ref_lvalue(p, q))
            }
            ExprKind::MaterializeTemporaryExpr
            | ExprKind::CallExpr
            | ExprKind::CXXOperatorCallExpr
            | ExprKind::InitListExpr => {
                let s =
                    if n.kind() == ExprKind::MaterializeTemporaryExpr { self.sub(e) } else { e };
                let sq = ast.exprs.get(s).ty();
                let tmp = self.temp(sq);
                if self.is_aggregate(sq) {
                    // clang `EmitAnyExprToMem`: a materialized temporary is
                    // a `MayOverlap` slot, which only a constructor call or
                    // an immediate invocation consumes.
                    let mut inner = s;
                    loop {
                        let k = ast.exprs.get(inner).kind();
                        if k == ExprKind::ParenExpr
                            || (k.is_cast() && ast.exprs.cast_kind(inner) == CastKind::NoOp)
                        {
                            inner = ast.exprs.operand(inner);
                        } else {
                            break;
                        }
                    }
                    let overlap = n.kind() == ExprKind::MaterializeTemporaryExpr
                        && matches!(
                            ast.exprs.get(inner).kind(),
                            ExprKind::ConstantExpr
                                | ExprKind::CXXConstructExpr
                                | ExprKind::CXXTemporaryObjectExpr
                        );
                    self.emit_agg_into(s, tmp, overlap)?;
                } else {
                    let v = self.emit_scalar(s)?;
                    self.store_scalar(v, tmp, sq);
                }
                if n.kind() == ExprKind::MaterializeTemporaryExpr {
                    self.push_temporary_cleanup(e, tmp);
                }
                Ok(LV { addr: tmp, q: sq, bf: None, vidx: None })
            }
            k if k.is_cast() => match ast.exprs.cast_kind(e) {
                CastKind::NoOp
                | CastKind::LValueBitCast
                | CastKind::LValueToRValue
                | CastKind::UserDefinedConversion => {
                    let mut lv = self.emit_lvalue(ast.exprs.operand(e))?;
                    if lv.bf.is_none() && !ast.types.is_function(q) {
                        lv.addr.ty = self.mty(q);
                    }
                    lv.q = q;
                    Ok(lv)
                }
                CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase => {
                    let s = ast.exprs.operand(e);
                    let lv = self.emit_lvalue(s)?;
                    let Some(derived) = ast.types.as_record(ast.exprs.get(s).ty()) else {
                        return err(self.loc_of(e), "derived-to-base cast of a non-class");
                    };
                    let (vb, off) = self.base_path(e, derived)?;
                    let (v, align) = self.base_address(lv.addr, derived, vb, off, false)?;
                    let ty = self.mty(q);
                    Ok(LV { addr: Addr { v, ty, align }, q, bf: None, vidx: None })
                }
                CastKind::Dynamic => {
                    let lv = self.emit_lvalue(ast.exprs.operand(e))?;
                    let p = self.emit_dynamic_cast(e, lv.addr.v, lv.addr.align)?;
                    Ok(self.ref_lvalue(p, q))
                }
                k => err(self.loc_of(e), format!("lvalue cast {k:?} is not supported yet")),
            },
            k => err(self.loc_of(e), format!("lvalue {k:?} is not supported yet")),
        }
    }

    fn cond_lvalue(&mut self, e: ExprId, c: ExprId, a: ExprId, b: ExprId) -> R<LV> {
        let ast = self.ast;
        if let Some(x) = self.fold_bool(c) {
            let (live, dead) = if x { (a, b) } else { (b, a) };
            // A live throw-expression: emitted, with an undefined lvalue
            // (`HandleConditionalOperatorLValueSimpleCase`).
            if let Some(t) = self.throw_of(live) {
                self.emit_throw(t, true)?;
                let dq = ast.exprs.get(dead).ty();
                let ty = self.mty(dq);
                let v = V::C(self.m.const_undef(LTy::PTR));
                return Ok(LV { addr: Addr { v, ty, align: 1 }, q: dq, bf: None, vidx: None });
            }
            return self.emit_lvalue(live);
        }
        let lb = self.f.fir.create_block();
        let rb = self.f.fir.create_block();
        let cont = self.f.fir.create_block();
        let start = self.f.fir.cur();
        self.branch_on_bool(c, lb, rb)?;
        self.emit_block(lb, false);
        let x = self.cond_eval(start, |cg| cg.lvalue_or_throw(a))?;
        let lb = self.f.fir.cur().expect("insert point");
        if x.is_some() {
            self.br(cont);
        }
        self.emit_block(rb, false);
        let y = self.cond_eval(start, |cg| cg.lvalue_or_throw(b))?;
        let rb = self.f.fir.cur().expect("insert point");
        self.emit_block(cont, false);
        let (x, y) = match (x, y) {
            (Some(x), Some(y)) => (x, y),
            (Some(lv), None) | (None, Some(lv)) => return Ok(lv),
            (None, None) => {
                return err(self.loc_of(e), "both operands of a glvalue conditional throw");
            }
        };
        let p = self
            .f
            .fir
            .push(Op::Phi { ty: LTy::PTR, inc: vec![(x.addr.v, lb), (y.addr.v, rb)] }, LTy::PTR);
        let align = x.addr.align.min(y.addr.align);
        Ok(LV {
            addr: Addr { v: p, ty: x.addr.ty, align },
            q: ast.exprs.get(e).ty(),
            bf: None,
            vidx: None,
        })
    }

    /// `e` through parentheses, if it is a throw-expression.
    pub(crate) fn throw_of(&self, mut e: ExprId) -> Option<ExprId> {
        let ast = self.ast;
        while ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = ast.exprs.operand(e);
        }
        (ast.exprs.get(e).kind() == ExprKind::CXXThrowExpr).then_some(e)
    }

    /// clang `EmitLValueOrThrowExpression`.
    fn lvalue_or_throw(&mut self, e: ExprId) -> R<Option<LV>> {
        if let Some(t) = self.throw_of(e) {
            self.emit_throw(t, false)?;
            return Ok(None);
        }
        Ok(Some(self.emit_lvalue(e)?))
    }

    /// The byte offset of element `i` of the IR aggregate `t`.
    pub(crate) fn elem_offset(&self, t: LTy, i: u64) -> u64 {
        if let Some((e, _)) = self.m.types.array_info(t) {
            return self.alloc_size(e) * i;
        }
        let fields = self.m.types.struct_fields(t).map(<[LTy]>::to_vec).unwrap_or_default();
        let packed = self.m.types.struct_is_packed(t);
        let mut off = 0u64;
        for (k, &f) in fields.iter().enumerate() {
            let (s, a) = self.ir_layout(f);
            let a = if packed { 1 } else { a };
            off = off.div_ceil(a) * a;
            if k as u64 == i {
                return off;
            }
            off += s;
        }
        off
    }

    fn member_lvalue(&mut self, e: ExprId) -> R<LV> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        let base = ast.exprs.operand(e);
        let field = ast.exprs.decl(e);
        if ast.decls.get(field).kind() == DeclKind::Var {
            // clang `tryToConvertMemberExprToDeclRefExpr`: a static data
            // member named with `.`/`->` is its base, ignored, and the
            // variable.
            self.emit_ignored(base)?;
            return self.decl_lvalue(e, field, q);
        }
        let base_addr = if n.has(ExprFlags::ARROW) {
            let p = self.emit_scalar(base)?;
            let pq = ast.types.pointee(ast.exprs.get(base).ty()).expect("pointee");
            let ty = self.mty(pq);
            let align = self.natural_align(pq);
            Addr { v: p, ty, align }
        } else {
            self.emit_lvalue(base)?.addr
        };
        if ast.decls.get(field).kind() != DeclKind::Field {
            return err(self.loc_of(e), "member is not a field (M3)");
        }
        self.field_lvalue(base_addr, field, q)
    }

    /// clang `EmitLValueForField`.
    pub(crate) fn field_lvalue(&mut self, base: Addr, field: DeclId, q: QualType) -> R<LV> {
        let ast = self.ast;
        let rd = ast.decls.get(field).ctx().expect("field parent");
        let rty = self.record_ty(rd);
        let (is_union, idx, bf) = {
            let r = self.tys.record(rd);
            (r.is_union, r.field_index.get(&field).copied(), r.bitfields.get(&field).copied())
        };
        if let Some(bf) = bf {
            let mut v = base.v;
            let mut align = base.align;
            let i = idx.unwrap_or(0);
            if i != 0 && !is_union {
                v = self.struct_gep(rty, base.v, i);
                align = min_align(base.align, self.elem_offset(rty, u64::from(i)));
            }
            let st = self.m.types.int(bf.storage_size);
            return Ok(LV { addr: Addr { v, ty: st, align }, q, bf: Some(bf), vidx: None });
        }
        let fq = ast.decls.get(field).ty().unwrap_or(q);
        if let Some(referee) = self.referee(fq) {
            // clang `EmitLValueForField` on a reference member: load the
            // bound address.
            let slot = self.field_lvalue_init(base, field, fq)?;
            let p = self.load(LTy::PTR, slot.addr.v, slot.addr.align, false);
            let (_, align) = self.pointee_layout(referee);
            let ty = self.mty(referee);
            return Ok(LV { addr: Addr { v: p, ty, align }, q: referee, bf: None, vidx: None });
        }
        self.field_lvalue_init(base, field, q)
    }

    /// The lvalue an aggregate initializer stores to: a reference member's
    /// own slot (to bind), any other field as `field_lvalue` (bit-fields).
    fn init_field_lvalue(&mut self, base: Addr, field: DeclId, q: QualType) -> R<LV> {
        if self.referee(q).is_some() {
            self.field_lvalue_init(base, field, q)
        } else {
            self.field_lvalue(base, field, q)
        }
    }

    /// The storage of field `field` of the object at `base` (a reference
    /// member's own slot, not its referee): clang `EmitLValueForFieldInitialization`.
    pub(crate) fn field_lvalue_init(&mut self, base: Addr, field: DeclId, q: QualType) -> R<LV> {
        let ast = self.ast;
        let rd = ast.decls.get(field).ctx().expect("field parent");
        let rty = self.record_ty(rd);
        let (is_union, idx) = {
            let r = self.tys.record(rd);
            (r.is_union, r.field_index.get(&field).copied())
        };
        let ty = self.mty(q);
        if is_union {
            return Ok(LV {
                addr: Addr { v: base.v, ty, align: base.align },
                q,
                bf: None,
                vidx: None,
            });
        }
        let Some(i) = idx else {
            // An empty field with no IR element (clang addresses it by
            // byte offset).
            let off = self.field_byte_offset(field)?;
            let v = if off == 0 { base.v } else { self.byte_offset(base.v, off) };
            let align = min_align(base.align, off);
            return Ok(LV { addr: Addr { v, ty, align }, q, bf: None, vidx: None });
        };
        let v = self.struct_gep(rty, base.v, i);
        let align = min_align(base.align, self.elem_offset(rty, u64::from(i)));
        Ok(LV { addr: Addr { v, ty, align }, q, bf: None, vidx: None })
    }

    /// The array operand of a simple array-to-pointer decay.
    fn simple_decay_operand(&self, e: ExprId) -> Option<ExprId> {
        let ast = self.ast;
        let n = ast.exprs.get(e);
        if n.kind() == ExprKind::ImplicitCastExpr
            && ast.exprs.cast_kind(e) == CastKind::ArrayToPointerDecay
        {
            return Some(ast.exprs.operand(e));
        }
        None
    }

    /// clang `ScalarExprEmitter::VisitConvertVectorExpr`.
    fn convert_vector(&mut self, v: V, sq: QualType, q: QualType) -> V {
        let st = self.f.fir.ty(&self.m, v);
        let dt = self.lty(q);
        if st == dt {
            return v;
        }
        let (Some((se, _)), Some((de, _))) = (self.vec_of(sq), self.vec_of(q)) else {
            return v;
        };
        let set = self.m.types.vector_info(st).map_or(st, |x| x.0);
        let det = self.m.types.vector_info(dt).map_or(dt, |x| x.0);
        let s_int = self.m.types.int_width(set);
        let d_int = self.m.types.int_width(det);
        if self.is_bool(de) {
            let z = V::C(self.m.const_zero(st));
            return if s_int.is_some() {
                self.icmp(IntPredicate::Ne, v, z)
            } else {
                self.fcmp(FloatPredicate::Une, v, z)
            };
        }
        let op = match (s_int, d_int) {
            (Some(sw), Some(dw)) => {
                let signed = self.is_signed(se) && !self.is_bool(se);
                match sw.cmp(&dw) {
                    std::cmp::Ordering::Equal => return v,
                    std::cmp::Ordering::Greater => CastOp::Trunc,
                    std::cmp::Ordering::Less if signed => CastOp::SExt,
                    std::cmp::Ordering::Less => CastOp::ZExt,
                }
            }
            (Some(_), None) if self.is_signed(se) && !self.is_bool(se) => CastOp::SIToFP,
            (Some(_), None) => CastOp::UIToFP,
            (None, Some(_)) if self.is_signed(de) => CastOp::FPToSI,
            (None, Some(_)) => CastOp::FPToUI,
            (None, None) => {
                let sw = self.m.types.float_width(set).unwrap_or(0);
                let dw = self.m.types.float_width(det).unwrap_or(0);
                if dw < sw { CastOp::FPTrunc } else { CastOp::FPExt }
            }
        };
        self.cast(op, v, dt)
    }

    /// `(vector, index)` operands of a vector subscript.
    fn vec_subscript(&self, e: ExprId) -> Option<(ExprId, ExprId)> {
        let ast = self.ast;
        let [l, r] = ast.exprs.operands(e);
        if self.vec_of(ast.exprs.get(l).ty()).is_some() {
            Some((l, r))
        } else if self.vec_of(ast.exprs.get(r).ty()).is_some() {
            Some((r, l))
        } else {
            None
        }
    }

    /// The vector holding the element `lv` (a `bool` vector's `iP`
    /// storage as `<P x i1>`).
    fn vec_elt_load(&mut self, lv: LV) -> V {
        let vol = self.is_volatile(lv.q);
        let raw = self.load(lv.addr.ty, lv.addr.v, lv.addr.align, vol);
        match self.m.types.int_width(lv.addr.ty) {
            Some(p) => {
                let vt = self.m.types.vector(LTy::I1, p);
                self.cast(CastOp::BitCast, raw, vt)
            }
            None => raw,
        }
    }

    /// clang `EmitArraySubscriptExpr`.
    fn subscript_lvalue(&mut self, e: ExprId) -> R<LV> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        // `LValue::MakeVectorElt`: the vector's address and the
        // unpromoted index.
        if let Some((base, idx)) = self.vec_subscript(e) {
            let blv = self.emit_lvalue(base)?;
            let i = self.emit_scalar(idx)?;
            return Ok(LV { addr: blv.addr, q, bf: None, vidx: Some(i) });
        }
        let [l, r] = ast.exprs.operands(e);
        let swapped =
            !ast.types.is_pointer(ast.exprs.get(l).ty()) && self.simple_decay_operand(l).is_none();
        let (base, idx) = if swapped { (r, l) } else { (l, r) };
        let pre = if swapped { Some(self.emit_scalar(idx)?) } else { None };
        let iq = ast.exprs.get(idx).ty();
        let esz = self.size_align(q).0;
        let ety = self.mty(q);
        let (addr, indices) = if let Some(arr) = self.simple_decay_operand(base) {
            let alv = if ast.exprs.get(arr).kind() == ExprKind::ArraySubscriptExpr {
                self.subscript_lvalue(arr)?
            } else {
                self.emit_lvalue(arr)?
            };
            let iv = match pre {
                Some(v) => v,
                None => self.emit_scalar(idx)?,
            };
            let iv = self.int_cast(iv, LTy::I64, self.is_signed(iq));
            let z = self.i64c(0);
            (alv.addr, vec![z, iv])
        } else {
            let p = self.emit_scalar(base)?;
            let align = self.natural_align(q);
            let iv = match pre {
                Some(v) => v,
                None => self.emit_scalar(idx)?,
            };
            let iv = self.int_cast(iv, LTy::I64, self.is_signed(iq));
            (Addr { v: p, ty: ety, align }, vec![iv])
        };
        let last = *indices.last().expect("index");
        let align = match self.cint(last) {
            Some((_, x, _)) => min_align(addr.align, (x as u64).wrapping_mul(esz)),
            None => min_align(addr.align, esz),
        };
        let v = self.gep(addr.ty, addr.v, indices, true);
        Ok(LV { addr: Addr { v, ty: ety, align }, q, bf: None, vidx: None })
    }

    // ---- statement expressions -----------------------------------------

    /// A GNU statement expression with a scalar or `void` value (clang
    /// `EmitCompoundStmt` with `GetLast`): the statements in order; the
    /// result statement (the last non-null one, through labels) is
    /// evaluated as the value.
    fn emit_stmt_expr(&mut self, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        let body = ast.exprs.stmt_expr_body(e);
        if ast.types.is_void(ast.exprs.get(e).ty()) {
            self.emit_stmt(body)?;
            return Ok(None);
        }
        let children: Vec<StmtId> = ast.lists.iter(ast.stmts.compound_body(body)).collect();
        let Some(result) =
            children.iter().rposition(|&c| ast.stmts.get(c).kind() != StmtKind::NullStmt)
        else {
            return err(self.loc_of(e), "statement expression without a result");
        };
        // `EmitCompoundStmt(GetLast)`: a lexical scope, the result stored
        // to a `tmp` so the scope's cleanups can run before it is read.
        let scope = self.begin_scope(true);
        for &c in &children[..result] {
            self.emit_stmt(c)?;
        }
        let mut s = children[result];
        loop {
            let n = *ast.stmts.get(s);
            match n.kind() {
                StmtKind::LabelStmt => {
                    self.emit_label(DeclId::new(n.a()));
                    s = StmtId::new(n.b());
                }
                StmtKind::AttributedStmt => s = StmtId::new(n.b()),
                StmtKind::ExprStmt => break,
                _ => return err(self.loc_of(e), "statement expression without a result"),
            }
        }
        self.ensure_insert_point();
        let r = ast.stmts.expr_stmt_expr(s);
        let q = ast.exprs.get(r).ty();
        let tmp = self.temp(q);
        let v = self.emit_scalar(r)?;
        self.store_scalar(v, tmp, q);
        self.end_scope(scope);
        Ok(Some(self.load_scalar(tmp, q)))
    }

    // ---- ignored expressions -------------------------------------------

    /// clang `EmitIgnoredExpr`.
    pub(crate) fn emit_ignored(&mut self, e: ExprId) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        if !self.is_glvalue(e) {
            if self.is_aggregate(q) {
                // clang `AggExprEmitter::withReturnValueSlot`: an ignored
                // slot gives the call no address, so `EmitCall` makes its
                // `sret`/coerce temporary after the arguments' `agg.tmp`s.
                let mut c = e;
                while ast.exprs.get(c).kind() == ExprKind::ParenExpr {
                    c = ast.exprs.operand(c);
                }
                match ast.exprs.get(c).kind() {
                    ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr => {
                        self.emit_call(c, None)?;
                        return Ok(());
                    }
                    ExprKind::CXXMemberCallExpr => {
                        self.emit_member_call(c, None)?;
                        return Ok(());
                    }
                    _ => {}
                }
                let tmp = self.temp(q);
                return self.emit_agg(e, tmp, false);
            }
            self.emit_scalar_opt(e)?;
            return Ok(());
        }
        self.emit_lvalue(e)?;
        Ok(())
    }

    // ---- calls ---------------------------------------------------------

    /// clang `EmitCallExpr`; an aggregate result is written to `dest`.
    pub(crate) fn emit_call(&mut self, e: ExprId, dest: Option<Addr>) -> R<Option<V>> {
        let ast = self.ast;
        let callee_e = ast.exprs.callee(e);
        let mut direct = None;
        {
            let mut c = callee_e;
            loop {
                let n = ast.exprs.get(c);
                match n.kind() {
                    ExprKind::ParenExpr => c = ast.exprs.operand(c),
                    // `f()` with `f` a function-reference template argument.
                    ExprKind::SubstNonTypeTemplateParmExpr => c = ast.exprs.subst_replacement(c),
                    ExprKind::ImplicitCastExpr
                        if matches!(
                            ast.exprs.cast_kind(c),
                            CastKind::FunctionToPointerDecay | CastKind::BuiltinFnToFnPtr
                        ) =>
                    {
                        c = ast.exprs.operand(c);
                    }
                    ExprKind::DeclRefExpr => {
                        let d = ast.exprs.decl(c);
                        // An operator call whose callee is a member: the
                        // first argument is the object.
                        if ast.exprs.get(e).kind() == ExprKind::CXXOperatorCallExpr
                            && ast.decls.get(d).kind() == DeclKind::CXXMethod
                            && !self.is_explicit_object_method(d)
                        {
                            let args: Vec<ExprId> =
                                ast.lists.iter(ast.exprs.call_args(e)).collect();
                            if let Some((&obj, rest)) = args.split_first() {
                                return self.emit_method_call(e, d, obj, false, rest, dest, false);
                            }
                        }
                        if matches!(ast.decls.get(d).kind(), DeclKind::Function)
                            || self.is_static_method(d)
                            || self.is_explicit_object_method(d)
                        {
                            direct = Some(d);
                        }
                        break;
                    }
                    // `p->~T()` on a scalar ([expr.prim.id.dtor]/2): the
                    // object expression is evaluated; there is no call.
                    ExprKind::CXXPseudoDestructorExpr => {
                        self.emit_ignored(ast.exprs.pseudo_destructor(c).0)?;
                        return Ok(None);
                    }
                    // `obj.f()` naming a static member function: the object
                    // is evaluated and ignored.
                    ExprKind::MemberExpr if self.is_static_method(ast.exprs.decl(c)) => {
                        self.emit_ignored(ast.exprs.operand(c))?;
                        direct = Some(ast.exprs.decl(c));
                        break;
                    }
                    _ => break,
                }
            }
        }
        // A builtin is never emitted as a function (spec/codegen.md §3, "Calls"):
        // it folds, lowers inline, or calls the library function it names.
        let mut lib = None;
        if let Some(d) = direct
            && let Some(id) = ast.decls.builtin_id(d)
        {
            match self.emit_builtin(id, e)? {
                crate::builtins::BuiltinCall::Emitted(v) => return Ok(v),
                crate::builtins::BuiltinCall::Library => lib = Some(self.lib_fn(d, id)?),
            }
        }
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        let arg_tys: Vec<QualType> = args.iter().map(|&a| ast.exprs.get(a).ty()).collect();
        let (callee, fty, extern_c, fid) = if let Some(d) = direct {
            let f = match &lib {
                Some((f, _)) => *f,
                None => self.get_fn(d)?,
            };
            let fty = ast.decls.get(d).ty().expect("function type");
            (V::C(Value::func(f)), fty, mangle::is_extern_c(ast, ast.first(d)), Some(f))
        } else {
            let p = self.emit_scalar(callee_e)?;
            let pq = ast.exprs.get(callee_e).ty();
            let fty = ast.types.pointee(pq).expect("function pointer");
            (p, fty, false, None)
        };
        let sig = self.call_sig(fty, extern_c, &arg_tys);
        let mut extra: Vec<Attr> = lib.map(|(_, x)| x).unwrap_or_default();
        let mut noreturn = extra.contains(&Attr::Flag(AttrKind::NoReturn));
        if !noreturn {
            let (site, unreachable) = self.noreturn_call(direct, fty);
            if site {
                extra.push(Attr::Flag(AttrKind::NoReturn));
            }
            noreturn = unreachable;
        }
        let r = self.call_with(&sig, callee, fid, None, &args, dest, &extra)?;
        if noreturn {
            self.finish_noreturn_call();
        }
        Ok(r)
    }

    /// Whether a call of function `d` (`None`: through a pointer) of type
    /// `fty` gets the `noreturn` call-site attribute, and whether it is
    /// followed by `unreachable` (clang `ConstructAttributeList`: a
    /// `[[noreturn]]` virtual function's overriders may return, so only
    /// the GNU spelling and a `noreturn` function type mark its call
    /// site; `EmitCall` still ends the block).
    pub(crate) fn noreturn_call(&self, d: Option<DeclId>, fty: QualType) -> (bool, bool) {
        let ast = self.ast;
        let ty_nr = ast.types.proto(fty).is_some_and(|p| p.info().noreturn());
        let Some(d) = d else { return (ty_nr, ty_nr) };
        if !ast.is_noreturn_fn(d) {
            return (ty_nr, ty_nr);
        }
        let virt =
            ast.redecls_of(d).any(|r| ast.decls.function(r).is_some_and(|f| f.spec.is_virtual()));
        let gnu = ast.redecls_of(d).any(|r| {
            ast.lists.iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r)).any(|a| {
                let n = ast.attrs.get(a);
                n.kind == kiln_ast::attr::AttrKind::NORETURN
                    && n.syntax == kiln_ast::attr::AttrSyntax::Gnu
            })
        });
        (ty_nr || gnu || !virt, true)
    }

    /// clang `EmitCall` of a call that does not return: `unreachable`,
    /// then a dummy block for the unreachable code that follows.
    pub(crate) fn finish_noreturn_call(&mut self) {
        if self.f.fir.cur().is_none() {
            return;
        }
        self.f.fir.push(Op::Unreachable, LTy::VOID);
        self.f.fir.set_cur(None);
        let cont = self.f.fir.create_block();
        self.emit_block(cont, false);
    }

    /// clang `EmitCallArgs` + `EmitCall`: evaluates `args` and calls
    /// `callee` with signature `sig` (`this`: the implicit object
    /// argument; `extra`: call-site attributes of a library builtin).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call_with(
        &mut self,
        sig: &Sig,
        callee: V,
        fid: Option<FuncId>,
        this: Option<V>,
        args: &[ExprId],
        dest: Option<Addr>,
        extra: &[Attr],
    ) -> R<Option<V>> {
        let vals = self.emit_call_args(sig, args)?;
        self.call_vals(sig, callee, fid, this, &vals, args, dest, extra)
    }

    /// clang `EmitCallArgs`: evaluates every argument, aggregates into
    /// `agg.tmp`.
    pub(crate) fn emit_call_args(&mut self, sig: &Sig, args: &[ExprId]) -> R<Vec<CallArg>> {
        let ast = self.ast;
        let arg_tys: Vec<QualType> = args.iter().map(|&a| ast.exprs.get(a).ty()).collect();
        let mut vals = Vec::with_capacity(args.len());
        for (i, &a) in args.iter().enumerate() {
            let v = if self.referee(sig.params[i]).is_some() {
                // A reference parameter binds the argument glvalue.
                CallArg::Scalar(self.emit_lvalue(a)?.addr.v)
            } else if self.is_aggregate(arg_tys[i]) {
                let t = self.temp(arg_tys[i]);
                let saved = std::mem::take(&mut self.f.dest_destructed);
                let r = self.emit_agg(a, t, false);
                self.f.dest_destructed = saved;
                r?;
                CallArg::Agg(t)
            } else {
                CallArg::Scalar(self.emit_scalar(a)?)
            };
            vals.push(v);
        }
        Ok(vals)
    }

    /// clang `EmitCall` with evaluated arguments `vals` (of the
    /// expressions `args`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call_vals(
        &mut self,
        sig: &Sig,
        callee: V,
        fid: Option<FuncId>,
        this: Option<V>,
        vals: &[CallArg],
        args: &[ExprId],
        dest: Option<Addr>,
        extra: &[Attr],
    ) -> R<Option<V>> {
        self.call_vals_to(sig, Callee::Value(callee), fid, this, vals, args, dest, extra)
    }

    /// [`Self::call_vals`] with a callee that may be computed only after
    /// the arguments are lowered (clang `EmitCall` step 2,
    /// `prepareConcreteCallee`: a virtual call loads its vtable slot after
    /// the argument coercions).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call_vals_to(
        &mut self,
        sig: &Sig,
        callee: Callee,
        fid: Option<FuncId>,
        this: Option<V>,
        vals: &[CallArg],
        args: &[ExprId],
        dest: Option<Addr>,
        extra: &[Attr],
    ) -> R<Option<V>> {
        self.f.vector_width = self.f.vector_width.max(sig.info.max_vector_width);
        let rq = sig.ret;
        let mut ir_args = Vec::new();
        let mut sret_addr = None;
        if sig.sret() {
            let a = match dest {
                Some(d) => d,
                None => self.temp(rq),
            };
            ir_args.push(a.v);
            sret_addr = Some(a);
        }
        if let Some(t) = this {
            ir_args.push(t);
        }
        if sig.vtt {
            let Some(v) = self.f.call_vtt.take() else {
                return err(
                    args.first().and_then(|&a| self.loc_of(a)),
                    "structor call without a VTT",
                );
            };
            ir_args.push(v);
        }
        for (i, &v) in vals.iter().enumerate() {
            let Some(info) = sig.info.args.get(i) else {
                return err(
                    args.get(i).and_then(|&a| self.loc_of(a)),
                    "call argument without ABI information",
                );
            };
            let kind = info.kind.clone();
            let loc = args.get(i).and_then(|&a| self.loc_of(a));
            self.call_arg(&kind, sig.params[i], v, &mut ir_args, loc)?;
        }
        let callee = match callee {
            Callee::Value(v) => v,
            Callee::Virtual { md, this, align } => self.virtual_fn_ptr(md, None, this, align)?,
        };
        let rty = match &sig.info.ret.kind {
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                let c = coerce.clone();
                self.ir(&c)
            }
            _ => LTy::VOID,
        };
        let (ret, params) = self.sig_attrs(sig, vals.len(), false);
        let func = if sig.nothrow {
            let mut fa = vec![Attr::Flag(AttrKind::NoUnwind)];
            fa.extend(extra.iter().cloned());
            self.m.attr_set(&fa)
        } else if !extra.is_empty() {
            self.m.attr_set(extra)
        } else {
            AttrList::default().func
        };
        let opts = CallOpts { attrs: AttrList { func, ret, params }, ..CallOpts::default() };
        let r = self.emit_call_op(sig.fn_ty, callee, ir_args, opts, rty, fid, sig.nothrow);
        if let Some(a) = sret_addr {
            // clang `convertTempToRValue`: a scalar returned indirectly
            // (a vector wider than the registers) is loaded back.
            if !self.is_aggregate(rq)
                && !matches!(
                    self.ast.types.canonical_view(rq),
                    kiln_ast::types::TypeView::Complex(_)
                )
            {
                return Ok(Some(self.load_scalar(a, rq)));
            }
            return Ok(None);
        }
        if rty == LTy::VOID {
            return Ok(None);
        }
        if self.is_aggregate(rq) {
            // clang `EmitCall`: a given destination takes the data size
            // (`getTypeInfoDataSizeInChars`; it may be a base subobject or
            // a `[[no_unique_address]]` member whose tail padding holds
            // other members), a fresh temporary the whole object.
            let (size, _) = self.size_align(rq);
            let (d, size) = match dest {
                Some(d) => {
                    let ds = self.ast.types.as_record(rq).and_then(|r| self.class_def(r));
                    (d, ds.map_or(size, |r| self.tys.layouter().record(r).data_size))
                }
                None => (self.temp(rq), size),
            };
            let (ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. }) =
                sig.info.ret.kind.clone()
            else {
                unreachable!()
            };
            if self.ir(&coerce) == self.mty(rq) {
                // EmitAggregateStore: element-wise for a first-class struct.
                self.aggregate_store(r, &coerce, d);
            } else {
                // (Empty records are returned `Ignore` on both targets, so
                // clang's empty-record skip cannot arise here.)
                self.coerced_store_sized(r, &coerce, d, size);
            }
            return Ok(None);
        }
        if rty != self.lty(rq) {
            // A coerced scalar (x86-64 `__int128` as `{ i64, i64 }`):
            // through a `coerce` temporary.
            let (ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. }) =
                sig.info.ret.kind.clone()
            else {
                unreachable!()
            };
            let t = self.temp(rq);
            let (size, _) = self.size_align(rq);
            self.coerced_store_sized(r, &coerce, t, size);
            return Ok(Some(self.load_scalar(t, rq)));
        }
        Ok(Some(r))
    }

    // ---- aggregates ----------------------------------------------------

    /// Strips parentheses, cleanups and no-op casts.
    fn strip_noop(&self, mut e: ExprId) -> ExprId {
        let ast = self.ast;
        loop {
            let n = ast.exprs.get(e);
            match n.kind() {
                ExprKind::ParenExpr | ExprKind::ConstantExpr => e = ast.exprs.operand(e),
                ExprKind::ExprWithCleanups | ExprKind::MaterializeTemporaryExpr => e = self.sub(e),
                k if k.is_cast() && ast.exprs.cast_kind(e) == CastKind::NoOp => {
                    e = ast.exprs.operand(e);
                }
                _ => return e,
            }
        }
    }

    /// clang `isSimpleZero`.
    fn is_simple_zero(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let mut e = e;
        loop {
            let n = ast.exprs.get(e);
            match n.kind() {
                ExprKind::ParenExpr => e = ast.exprs.operand(e),
                k if k.is_cast() => match ast.exprs.cast_kind(e) {
                    CastKind::NoOp
                    | CastKind::BitCast
                    | CastKind::ToVoid
                    | CastKind::IntegralCast
                    | CastKind::IntegralToBoolean
                    | CastKind::IntegralToFloating
                    | CastKind::FloatingCast
                    | CastKind::FloatingToBoolean
                    | CastKind::FloatingToIntegral
                    | CastKind::BooleanToSignedIntegral
                    | CastKind::IntegralToPointer
                    | CastKind::PointerToIntegral => e = ast.exprs.operand(e),
                    CastKind::NullToPointer => return true,
                    _ => return false,
                },
                ExprKind::IntegerLiteral => return ast.exprs.int_value(e) == Some(0),
                ExprKind::FloatingLiteral => {
                    let c = ast.exprs.literal_const(e);
                    return c.is_some_and(|c| match ast.consts.get(c) {
                        kiln_ast::consts::ConstValue::Scalar(kiln_ast::consts::Scalar::Float {
                            bits,
                            ..
                        }) => bits == 0,
                        _ => false,
                    });
                }
                ExprKind::ImplicitValueInitExpr | ExprKind::CXXScalarValueInitExpr => return true,
                ExprKind::CharacterLiteral => return ast.exprs.char_value(e) == 0,
                _ => return false,
            }
        }
    }

    /// clang `GetNumNonZeroBytesInInit`.
    fn non_zero_bytes(&mut self, e: ExprId) -> u64 {
        let ast = self.ast;
        let e = self.strip_noop(e);
        if self.is_simple_zero(e) {
            return 0;
        }
        let q = ast.exprs.get(e).ty();
        if ast.exprs.get(e).kind() != ExprKind::InitListExpr {
            return self.size_align(q).0;
        }
        let inits: Vec<ExprId> = ast.lists.iter(ast.exprs.init_list_inits(e)).collect();
        if let Some(rd) = ast.types.as_record(q) {
            let is_union = self.tys.record(rd).is_union;
            if !is_union {
                let fields: Vec<DeclId> = ast
                    .decls
                    .members(rd)
                    .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
                    .collect();
                let mut total = 0;
                for (k, _) in fields.iter().enumerate() {
                    if k >= inits.len() {
                        break;
                    }
                    total += self.non_zero_bytes(inits[k]);
                }
                return total;
            }
        }
        inits.iter().map(|&i| self.non_zero_bytes(i)).sum()
    }

    /// clang `EmitAggExpr` (with `CheckAggExprForMemSetUse`).
    pub(crate) fn emit_agg(&mut self, e: ExprId, dest: Addr, zeroed: bool) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let mut zeroed = zeroed;
        if !zeroed && !self.is_volatile(q) {
            let size = self.size_align(q).0;
            if size > 16 && self.non_zero_bytes(e) * 4 <= size {
                self.memset(dest, 0, size, false);
                zeroed = true;
            }
        }
        self.agg_visit(e, dest, zeroed)
    }

    fn agg_visit(&mut self, e: ExprId, dest: Addr, zeroed: bool) -> R<()> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        match n.kind() {
            ExprKind::ConstantExpr if self.immediate_value(e).is_some() => {
                // clang `AggExprEmitter::VisitConstantExpr`: the value of
                // an immediate invocation, stored by `CreateCoercedStore`.
                // The destination size is `AggValueSlot::getPreferredSize`:
                // the data size when the slot may overlap.
                let overlap = std::mem::take(&mut self.f.overlap);
                let c = self.immediate_value(e).expect("guarded");
                let k = self.const_value(c, q, Some(e), true)?;
                let size = match ast.types.as_record(q) {
                    Some(rd) if overlap && self.array_elem(q).is_none() => {
                        Some(self.copy_size(rd, true))
                    }
                    _ => None,
                };
                self.coerced_store_const(k, dest, size, self.is_volatile(q));
                Ok(())
            }
            ExprKind::ParenExpr | ExprKind::ConstantExpr => {
                self.agg_visit(ast.exprs.operand(e), dest, zeroed)
            }
            ExprKind::CXXDefaultArgExpr => match default_arg_of(ast, e) {
                Some(d) => {
                    let saved = self.enter_default_arg(e);
                    let r = self.agg_visit(d, dest, zeroed);
                    self.srcloc = saved;
                    r
                }
                None => err(self.loc_of(e), "default argument not yet parsed"),
            },
            ExprKind::ExprWithCleanups => {
                let sc = self.begin_scope(false);
                let r = self.agg_visit(self.sub(e), dest, zeroed);
                self.end_scope(sc);
                r
            }
            ExprKind::MaterializeTemporaryExpr => self.agg_visit(self.sub(e), dest, zeroed),
            ExprKind::AtomicExpr => self.emit_atomic_expr(e, Some(dest)).map(|_| ()),
            ExprKind::CXXThrowExpr => self.emit_throw(e, true),
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                self.emit_construct(e, dest, zeroed)
            }
            ExprKind::CXXInheritedCtorInitExpr => self.emit_inherited_ctor_init(e, dest),
            ExprKind::CXXStdInitializerListExpr => self.std_initializer_list(e, dest),
            ExprKind::CXXBindTemporaryExpr => {
                let destructed = std::mem::take(&mut self.f.dest_destructed);
                self.agg_visit(self.sub(e), dest, zeroed)?;
                if !destructed && let Some(rd) = self.destructed_class(q) {
                    let c = Cleanup::Dtor { addr: dest.v, record: rd, base: false };
                    self.push_full_expr_cleanup(c, true, true);
                }
                Ok(())
            }
            ExprKind::CXXDefaultInitExpr => match self.default_member_init(e) {
                Some(init) => {
                    let saved = self.enter_default_arg(e);
                    let r = self.agg_visit(init, dest, zeroed);
                    self.srcloc = saved;
                    r
                }
                None => err(self.loc_of(e), "default member initializer not yet parsed"),
            },
            ExprKind::CXXMemberCallExpr if self.is_glvalue(e) => {
                let lv = self.emit_lvalue(e)?;
                let size = self.size_align(q).0;
                self.memcpy(dest, lv.addr.v, lv.addr.align, size, false);
                Ok(())
            }
            ExprKind::CXXMemberCallExpr => {
                self.emit_member_call(e, Some(dest))?;
                Ok(())
            }
            ExprKind::InitListExpr => {
                if ast.types.is_array(q) {
                    self.array_init(e, dest, zeroed)
                } else {
                    self.record_init(e, dest, zeroed)
                }
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr if self.is_glvalue(e) => {
                let lv = self.emit_lvalue(e)?;
                let size = self.size_align(q).0;
                self.memcpy(dest, lv.addr.v, lv.addr.align, size, false);
                Ok(())
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr => {
                self.emit_call(e, Some(dest))?;
                Ok(())
            }
            ExprKind::ImplicitValueInitExpr | ExprKind::CXXScalarValueInitExpr => {
                if !zeroed {
                    let size = self.size_align(q).0;
                    self.memset(dest, 0, size, false);
                }
                Ok(())
            }
            k if k.is_cast()
                && matches!(
                    ast.exprs.cast_kind(e),
                    CastKind::NoOp
                        | CastKind::LValueToRValue
                        | CastKind::LValueToRValueBitCast
                        | CastKind::ConstructorConversion
                        | CastKind::UserDefinedConversion
                ) =>
            {
                let s = ast.exprs.operand(e);
                if self.is_glvalue(s) {
                    let lv = self.emit_lvalue(s)?;
                    let size = self.size_align(q).0;
                    let vol = self.is_volatile(q) || self.is_volatile(lv.q);
                    self.memcpy(dest, lv.addr.v, lv.addr.align, size, vol);
                    Ok(())
                } else {
                    self.agg_visit(s, dest, zeroed)
                }
            }
            ExprKind::DeclRefExpr
            | ExprKind::MemberExpr
            | ExprKind::ArraySubscriptExpr
            | ExprKind::UnaryOperator => {
                let lv = self.emit_lvalue(e)?;
                let size = self.size_align(q).0;
                self.memcpy(dest, lv.addr.v, lv.addr.align, size, false);
                Ok(())
            }
            ExprKind::BinaryOperator if ast.exprs.binary_op(e) == BinaryOp::Comma => {
                let [l, r] = ast.exprs.operands(e);
                self.emit_ignored(l)?;
                self.ensure_insert_point();
                self.agg_visit(r, dest, zeroed)
            }
            // clang `AggExprEmitter::VisitAbstractConditionalOperator`: each
            // arm initializes the destination in its own block.
            ExprKind::ConditionalOperator => {
                let [a, b] = ast.exprs.cond_arms(e);
                let c = self.sub(e);
                if let Some(x) = self.fold_bool(c) {
                    return self.agg_visit(if x { a } else { b }, dest, zeroed);
                }
                let lb = self.f.fir.create_block();
                let rb = self.f.fir.create_block();
                let cont = self.f.fir.create_block();
                let start = self.f.fir.cur();
                self.branch_on_bool(c, lb, rb)?;
                self.emit_block(lb, false);
                self.cond_eval(start, |cg| cg.agg_visit(a, dest, zeroed))?;
                self.br(cont);
                self.emit_block(rb, false);
                self.cond_eval(start, |cg| cg.agg_visit(b, dest, zeroed))?;
                self.emit_block(cont, false);
                Ok(())
            }
            ExprKind::StringLiteral => {
                // A literal initializing a (possibly longer) char array:
                // copy from the literal's constant padded to the array.
                let size = self.size_align(q).0;
                let g = self.lit_global_sized(ast.exprs.string_lit(e), size);
                let align = self.m.global(g).align.unwrap_or(1);
                self.memcpy(dest, V::C(gval(g)), align, size, false);
                Ok(())
            }
            ExprKind::ArrayInitLoopExpr => self.array_init_loop(e, dest, None),
            // clang `AggExprEmitter::VisitLambdaExpr`: each capture
            // initializes its field; a captureless closure has nothing to
            // initialize.
            ExprKind::LambdaExpr => {
                let l = ast.exprs.lambda(e);
                let inits: Vec<ExprId> =
                    l.inits.map(|x| ast.lists.iter::<ExprId>(x).collect()).unwrap_or_default();
                let fields: Vec<DeclId> = l.captures.iter().filter_map(|c| c.field).collect();
                let rty = self.record_ty(l.closure);
                let base = Addr { v: dest.v, ty: rty, align: dest.align };
                // [except.ctor]/3: an initialized capture is destroyed if a
                // later one throws (`pushDestroyAndDeferDeactivation`).
                let mut deferred = Vec::new();
                for (f, init) in fields.into_iter().zip(inits) {
                    let fq = ast.decls.get(f).ty().expect("capture type");
                    let lv = self.field_lvalue_init(base, f, fq)?;
                    if self.referee(fq).is_some() {
                        // clang `EmitInitializationToLValue` on a
                        // reference: bind, then store the address.
                        let p = self.emit_ref_binding(init)?;
                        self.store(p, lv.addr.v, lv.addr.align, false);
                    } else {
                        self.init_to_lvalue(init, lv, false)?;
                    }
                    if let Some(frd) = self.destructed_class(fq) {
                        let c = if self.array_elem(fq).is_some() {
                            Cleanup::ArrayDtor { addr: lv.addr, record: frd }
                        } else {
                            Cleanup::Dtor { addr: lv.addr.v, record: frd, base: false }
                        };
                        self.push_deferred_destroy(c, &mut deferred);
                    }
                }
                self.deactivate_deferred(deferred);
                Ok(())
            }
            ExprKind::BinaryOperator if ast.exprs.binary_op(e) == BinaryOp::Cmp => {
                self.agg_three_way(e, dest)
            }
            k => err(self.loc_of(e), format!("aggregate expression {k:?} is not supported yet")),
        }
    }

    /// The builtin three-way comparison `e` into the comparison-category
    /// object at `dest`: clang `AggExprEmitter::VisitBinCmp`, a chain of
    /// selects over the category's integer values stored into its only
    /// field.
    fn agg_three_way(&mut self, e: ExprId, dest: Addr) -> R<()> {
        use kiln_ast::class::{CmpCategory, CmpResult};
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let rd = ast.types.as_record(q).and_then(|d| ast.definition(d));
        let Some(info) = rd.and_then(|d| ast.cmp_categories.for_record(d)).copied() else {
            return err(self.loc_of(e), "comparison category type not recorded");
        };
        let [l, r] = ast.exprs.operands(e);
        let lq = ast.exprs.get(l).ty();
        let lv = self.emit_scalar(l)?;
        let rv = self.emit_scalar(r)?;
        let fq = ast.decls.get(info.field).ty().unwrap_or(q);
        let fty = self.mty(fq);
        let lt = self.f.fir.ty(&self.m, lv);
        let float = self.is_float_ty(lt);
        let signed = !ast.types.is_pointer(lq) && self.is_signed(lq);
        let cmp = |cg: &mut Self, k: CmpResult| -> V {
            if float {
                let p = match k {
                    CmpResult::Less => FloatPredicate::Olt,
                    CmpResult::Greater => FloatPredicate::Ogt,
                    _ => FloatPredicate::Oeq,
                };
                cg.fcmp(p, lv, rv)
            } else {
                let p = match (k, signed) {
                    (CmpResult::Less, true) => IntPredicate::Slt,
                    (CmpResult::Less, false) => IntPredicate::Ult,
                    (CmpResult::Greater, true) => IntPredicate::Sgt,
                    (CmpResult::Greater, false) => IntPredicate::Ugt,
                    _ => IntPredicate::Eq,
                };
                cg.icmp(p, lv, rv)
            }
        };
        let val = |cg: &mut Self, x: i64| cg.icn(fty, i128::from(x));
        let select = if info.kind == CmpCategory::Partial {
            let c = cmp(self, CmpResult::Equal);
            let (a, b) =
                (val(self, info.equal_or_equiv_int()), val(self, info.int(CmpResult::Unordered)));
            let sel_eq = self.select(c, a, b);
            let c = cmp(self, CmpResult::Greater);
            let a = val(self, info.int(CmpResult::Greater));
            let sel_gt = self.select(c, a, sel_eq);
            let c = cmp(self, CmpResult::Less);
            let a = val(self, info.int(CmpResult::Less));
            self.select(c, a, sel_gt)
        } else {
            let c = cmp(self, CmpResult::Less);
            let (a, b) =
                (val(self, info.int(CmpResult::Less)), val(self, info.int(CmpResult::Greater)));
            let sel_lt = self.select(c, a, b);
            let c = cmp(self, CmpResult::Equal);
            let a = val(self, info.equal_or_equiv_int());
            self.select(c, a, sel_lt)
        };
        let flv = self.field_lvalue_init(dest, info.field, fq)?;
        self.store_lv(select, flv, false);
        Ok(())
    }

    /// clang `AggExprEmitter::EmitInitializationToLValue`.
    pub(crate) fn init_to_lvalue(&mut self, e: ExprId, lv: LV, zeroed: bool) -> R<()> {
        let ast = self.ast;
        if zeroed && self.is_simple_zero(e) {
            return Ok(());
        }
        let k = ast.exprs.get(e).kind();
        if matches!(k, ExprKind::ImplicitValueInitExpr | ExprKind::CXXScalarValueInitExpr) {
            return self.null_init_lvalue(lv, zeroed);
        }
        if lv.bf.is_none() && self.referee(lv.q).is_some() {
            // A reference member binds to its initializer's address
            // (clang `EmitInitializationToLValue`, reference case).
            let p = self.emit_ref_binding(e)?;
            self.store(p, lv.addr.v, lv.addr.align, false);
            return Ok(());
        }
        if self.is_aggregate(lv.q) {
            // `AggValueSlot::IsDestructed`: the enclosing object owns the
            // destruction, so a bound temporary initializing it in place
            // gets no cleanup of its own.
            let saved = std::mem::replace(&mut self.f.dest_destructed, true);
            let r = self.emit_agg(e, lv.addr, zeroed);
            self.f.dest_destructed = saved;
            return r;
        }
        let v = self.emit_scalar(e)?;
        if lv.bf.is_none() && self.ast.types.as_atomic(lv.q).is_some() {
            // An initialization of an `_Atomic` object is a plain store.
            self.store_scalar(v, lv.addr, lv.q);
        } else {
            self.store_lv(v, lv, false);
        }
        Ok(())
    }

    /// clang `EmitNullInitializationToLValue`.
    pub(crate) fn null_init_lvalue(&mut self, lv: LV, zeroed: bool) -> R<()> {
        if zeroed {
            return Ok(());
        }
        if self.is_aggregate(lv.q) {
            let size = self.size_align(lv.q).0;
            self.memset(lv.addr, 0, size, false);
            return Ok(());
        }
        let t = self.mty(lv.q);
        let z = V::C(self.m.const_zero(t));
        if lv.bf.is_some() {
            self.store_lv(z, lv, false);
        } else {
            let vol = self.is_volatile(lv.q);
            self.store(z, lv.addr.v, lv.addr.align, vol);
        }
        Ok(())
    }

    /// clang `AggExprEmitter::VisitCXXStdInitializerListExpr`: the
    /// object points at its backing array, then holds the length (or a
    /// pointer past the end) ([dcl.init.list]/5).
    fn std_initializer_list(&mut self, e: ExprId, dest: Addr) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let arr = ast.exprs.operand(e);
        let Some(rd) = ast.types.as_record(q) else {
            return err(self.loc_of(e), "std::initializer_list of a non-class type");
        };
        let def = ast.definition(rd).unwrap_or(rd);
        let fields: Vec<DeclId> = ast
            .decls
            .members(def)
            .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        let n = match ast.types.canonical_view(ast.exprs.get(arr).ty()) {
            kiln_ast::types::TypeView::ConstantArray(a) => a.size,
            _ => return err(self.loc_of(e), "std::initializer_list without a constant array"),
        };
        let [f0, f1] = fields.as_slice() else {
            return err(self.loc_of(e), "std::initializer_list of an unexpected layout");
        };
        let (f0, f1) = (*f0, *f1);
        let a = self.emit_lvalue(arr)?.addr.v;
        let rty = self.record_ty(rd);
        let base = Addr { v: dest.v, ty: rty, align: dest.align };
        let q0 = ast.decls.get(f0).ty().expect("field type");
        let lv0 = self.field_lvalue(base, f0, q0)?;
        self.store_scalar(a, lv0.addr, q0);
        let q1 = ast.decls.get(f1).ty().expect("field type");
        let lv1 = self.field_lvalue(base, f1, q1)?;
        let len = i128::from(n);
        let v = if ast.types.is_pointer(q1) {
            let elem = ast.types.pointee(q1).unwrap_or(q1);
            let size = self.size_align(elem).0;
            self.byte_offset(a, n * size)
        } else {
            let t = self.lty(q1);
            self.icn(t, len)
        };
        self.store_scalar(v, lv1.addr, q1);
        Ok(())
    }

    fn record_init(&mut self, e: ExprId, dest: Addr, zeroed: bool) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let Some(rd) = ast.types.as_record(q) else {
            return err(self.loc_of(e), "initializer list of non-aggregate type");
        };
        let inits: Vec<ExprId> = ast.lists.iter(ast.exprs.init_list_inits(e)).collect();
        let is_union = self.tys.record(rd).is_union;
        let rty = self.record_ty(rd);
        let base = Addr { v: dest.v, ty: rty, align: dest.align };
        if is_union {
            let Some(f) = ast.exprs.init_list(e).union_field else { return Ok(()) };
            let fq = ast.decls.get(f).ty().expect("field type");
            let lv = self.init_field_lvalue(base, f, fq)?;
            return match inits.first() {
                Some(&i) => self.init_to_lvalue(i, lv, zeroed),
                None => self.null_init_lvalue(lv, zeroed),
            };
        }
        // [dcl.init.aggr]/2: the bases are the list's first elements
        // (clang `VisitCXXParenListOrInitListExpr`).
        let def = ast.definition(rd).unwrap_or(rd);
        let bases: Vec<_> = ast.decls.bases(def, &ast.lists).collect();
        let mut k = 0usize;
        // [except.ctor]/3: each initialized base and member is destroyed
        // if a later initialization throws; the cleanups are deactivated
        // once the object is complete (`pushDestroyAndDeferDeactivation`,
        // `CleanupDeactivationScope`).
        let mut deferred = Vec::new();
        for b in bases {
            if b.is_virtual {
                return err(self.loc_of(e), "aggregate with a virtual base");
            }
            let Some(brd) = ast.types.as_record(b.ty) else {
                return err(self.loc_of(e), "base of a non-class type");
            };
            let Some(off) = self.base_offset(rd, brd) else {
                return err(self.loc_of(e), "base class not in the layout");
            };
            let Some(&init) = inits.get(k) else { break };
            k += 1;
            let v = self.byte_offset(dest.v, off);
            let ty = self.record_ty(brd);
            let bdest = Addr { v, ty, align: min_align(dest.align, off) };
            let overlap = self.base_may_overlap(rd, brd, off);
            self.emit_agg_into(init, bdest, overlap)?;
            if self.has_nontrivial_dtor(brd) {
                let c = Cleanup::Dtor { addr: v, record: brd, base: false };
                self.push_deferred_destroy(c, &mut deferred);
            }
        }
        let fields: Vec<DeclId> =
            ast.decls.members(rd).filter(|&m| ast.decls.get(m).kind() == DeclKind::Field).collect();
        for f in fields {
            let fd = ast.decls.field(f);
            let unnamed_bf =
                fd.is_some_and(|x| x.bit_width.is_some()) && mangle::decl_name(ast, f).is_empty();
            if unnamed_bf {
                continue;
            }
            if k >= inits.len() && zeroed {
                break;
            }
            let fq = ast.decls.get(f).ty().expect("field type");
            let lv = self.init_field_lvalue(base, f, fq)?;
            if k < inits.len() {
                self.init_to_lvalue(inits[k], lv, zeroed)?;
                k += 1;
            } else {
                self.null_init_lvalue(lv, zeroed)?;
            }
            if let Some(frd) = self.destructed_class(fq) {
                let c = if self.array_elem(fq).is_some() {
                    Cleanup::ArrayDtor { addr: lv.addr, record: frd }
                } else {
                    Cleanup::Dtor { addr: lv.addr.v, record: frd, base: false }
                };
                self.push_deferred_destroy(c, &mut deferred);
            }
        }
        self.deactivate_deferred(deferred);
        Ok(())
    }

    /// clang `AggExprEmitter::VisitArrayInitLoopExpr`: binds the common
    /// expression, then initializes each element of `dest` in a loop.
    /// `outer` is the outermost loop's first element (a nested loop shares
    /// its partial-array cleanup).
    fn array_init_loop(&mut self, e: ExprId, dest: Addr, outer: Option<V>) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let [ove, sub] = ast.exprs.array_init_loop(e);
        let src = ast.exprs.opaque_source(ove);
        if let Some(src) = src {
            let v = if self.is_glvalue(src) {
                Opaque::Lv(self.emit_lvalue(src)?)
            } else {
                Opaque::Rv(self.emit_scalar_opt(src)?)
            };
            self.f.opaque.insert(src, v);
        }
        let n = ast.types.array_size(q).unwrap_or(0);
        if n == 0 {
            if let Some(src) = src {
                self.f.opaque.remove(&src);
            }
            return Ok(());
        }
        let eq = ast.types.element(q).expect("element type");
        let esz = self.size_align(eq).0;
        let ety = self.mty(eq);
        let ealign = min_align(dest.align, esz);
        let zero = self.i64c(0);
        let begin = self.gep(dest.ty, dest.v, vec![zero, zero], true);
        let entry = self.f.fir.cur().expect("insertion point");
        let body = self.f.fir.create_block();
        self.emit_block(body, false);
        let phi = self.f.fir.push(Op::Phi { ty: LTy::I64, inc: vec![(zero, entry)] }, LTy::I64);
        let el = self.gep(ety, begin, vec![phi], true);
        let outer = outer.unwrap_or(begin);
        let inner = ast.exprs.get(sub).kind() == ExprKind::ArrayInitLoopExpr;
        // The elements initialized so far are destroyed if an
        // initialization throws ([except.ctor]/3,
        // `pushRegularPartialArrayCleanup`).
        let mut cleanup = None;
        if self.opts.exceptions
            && !inner
            && let Some(record) = self.destructed_class(eq)
        {
            let c = Cleanup::PartialArray { begin: outer, end: el, elem: ety, record };
            self.push_full_expr_cleanup(c, false, true);
            cleanup = Some(self.f.cl.depth());
        }
        self.f.array_index.push(phi);
        let sc = self.begin_scope(false);
        let lv = LV { addr: Addr { v: el, ty: ety, align: ealign }, q: eq, bf: None, vidx: None };
        let r = if inner {
            self.array_init_loop(sub, lv.addr, Some(outer))
        } else {
            self.init_to_lvalue(sub, lv, false)
        };
        self.end_scope(sc);
        self.f.array_index.pop();
        r?;
        let one = self.i64c(1);
        let next = self.bin(BinOp::Add, Flags::NUW, phi, one);
        let cur = self.f.fir.cur().expect("insertion point");
        self.f.fir.add_incoming(Fir::last_inst(phi), next, cur);
        let nn = self.i64c(i128::from(n));
        let done = self.icmp(IntPredicate::Eq, next, nn);
        let end = self.f.fir.create_block();
        self.cond_br(done, end, body);
        self.emit_block(end, false);
        if let Some(c) = cleanup {
            self.deactivate_cleanup_block(c, Fir::last_inst(phi));
        }
        if let Some(src) = src {
            self.f.opaque.remove(&src);
        }
        Ok(())
    }

    /// `pushDestroyAndDeferDeactivation`: pushes the normal and EH
    /// cleanup `c` and records it, with its dominating placeholder, for
    /// [`Self::deactivate_deferred`].
    fn push_deferred_destroy(&mut self, c: Cleanup, deferred: &mut Vec<(usize, V)>) {
        if self.f.fir.cur().is_none() {
            return;
        }
        let z = V::C(self.m.const_bool(false));
        let dominator =
            self.f.fir.push(Op::Bin { op: BinOp::Or, flags: Flags::NONE, l: z, r: z }, LTy::I1);
        self.push_full_expr_cleanup(c, true, true);
        deferred.push((self.f.cl.depth(), dominator));
    }

    /// `CleanupDeactivationScope::ForceDeactivate`: deactivates the
    /// deferred cleanups, innermost first, and drops their placeholders.
    fn deactivate_deferred(&mut self, deferred: Vec<(usize, V)>) {
        for (c, dominator) in deferred.into_iter().rev() {
            let d = Fir::last_inst(dominator);
            self.deactivate_cleanup_block(c, d);
            self.f.fir.erase(d);
        }
    }

    /// clang `AggExprEmitter::EmitArrayInit`.
    fn array_init(&mut self, e: ExprId, dest: Addr, zeroed: bool) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let eq = ast.types.element(q).expect("element type");
        let n_elems = ast.types.array_size(q).unwrap_or(0);
        let inits: Vec<ExprId> = ast.lists.iter(ast.exprs.init_list_inits(e)).collect();
        let esz = self.size_align(eq).0;
        let ety = self.mty(eq);
        let ealign = min_align(dest.align, esz);
        let begin = dest.v;
        // [except.ctor]/3: the elements initialized so far are destroyed
        // if a later initializer throws. The EH cleanup reads how far the
        // initialization got from `arrayinit.endOfInit` and is
        // deactivated once the array is complete (clang `EmitArrayInit`,
        // `pushIrregularPartialArrayCleanup`, only when `needsEHCleanup`);
        // `dominator` stands for clang's `dominatingIP`.
        let mut partial = None;
        if let Some(record) = self.destructed_class(eq).filter(|_| self.opts.exceptions) {
            let z = V::C(self.m.const_bool(false));
            let dominator =
                self.f.fir.push(Op::Bin { op: BinOp::Or, flags: Flags::NONE, l: z, r: z }, LTy::I1);
            let al = self.pref_align(LTy::PTR);
            let slot = self.f.fir.alloca(LTy::PTR, al);
            self.store(begin, slot, al, false);
            let mut depth = 0;
            let mut inner = eq;
            while let Some(e) = self.array_elem(inner) {
                inner = e;
                depth += 1;
            }
            let elem = self.mty(inner);
            let p =
                IrregularPartialArray { begin, end_slot: slot, outer: ety, depth, elem, record };
            self.push_full_expr_cleanup(Cleanup::IrregularPartialArray(Box::new(p)), false, true);
            partial = Some((self.f.cl.depth(), dominator, slot, al));
        }
        for (i, &init) in inits.iter().enumerate() {
            let el = if i == 0 {
                begin
            } else {
                let c = self.i64c(i as i128);
                let el = self.gep(ety, begin, vec![c], true);
                if let Some((_, _, slot, al)) = partial {
                    self.store(el, slot, al, false);
                }
                el
            };
            let lv =
                LV { addr: Addr { v: el, ty: ety, align: ealign }, q: eq, bf: None, vidx: None };
            self.init_to_lvalue(init, lv, zeroed)?;
        }
        let filler = ast.exprs.init_list(e).array_filler;
        let trivial = match filler {
            None => true,
            Some(f) => self.is_trivial_filler(f),
        };
        if inits.len() as u64 != n_elems && !(zeroed && trivial) {
            let mut el = begin;
            if !inits.is_empty() {
                let c = self.i64c(inits.len() as i128);
                el = self.gep(ety, el, vec![c], true);
                if let Some((_, _, slot, al)) = partial {
                    self.store(el, slot, al, false);
                }
            }
            let c = self.i64c(i128::from(n_elems));
            let end = self.gep(ety, begin, vec![c], true);
            let entry = self.f.fir.cur().expect("insert point");
            let body = self.f.fir.create_block();
            self.emit_block(body, false);
            let cur = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(el, entry)] }, LTy::PTR);
            let lv =
                LV { addr: Addr { v: cur, ty: ety, align: ealign }, q: eq, bf: None, vidx: None };
            // [class.temporary]/5: the temporaries of each element's
            // initialization are destroyed before the next one starts.
            let sc = self.begin_scope(false);
            let r = match filler {
                Some(f) => self.init_to_lvalue(f, lv, false),
                None => self.null_init_lvalue(lv, false),
            };
            self.end_scope(sc);
            r?;
            let one = self.i64c(1);
            let next = self.gep(ety, cur, vec![one], true);
            if let Some((_, _, slot, al)) = partial {
                self.store(next, slot, al, false);
            }
            let done = self.icmp(IntPredicate::Eq, next, end);
            let endb = self.f.fir.create_block();
            self.cond_br(done, endb, body);
            let here = self.f.fir.cur().expect("insert point");
            self.f.fir.add_incoming(Fir::last_inst(cur), next, here);
            self.emit_block(endb, false);
        }
        if let Some((c, dominator, _, _)) = partial {
            let d = Fir::last_inst(dominator);
            self.deactivate_cleanup_block(c, d);
            self.f.fir.erase(d);
        }
        Ok(())
    }

    pub(crate) fn is_trivial_filler(&self, e: ExprId) -> bool {
        let ast = self.ast;
        match ast.exprs.get(e).kind() {
            ExprKind::ImplicitValueInitExpr => true,
            ExprKind::InitListExpr => {
                ast.lists.get(ast.exprs.init_list_inits(e)).is_empty()
                    && ast.exprs.init_list(e).array_filler.is_none_or(|f| self.is_trivial_filler(f))
            }
            _ => false,
        }
    }

    // ---- local variables -----------------------------------------------

    /// clang `EmitVarDecl` for a block-scope variable; a decomposition's
    /// tuple-like bindings' holding variables follow it (clang
    /// `EmitDecl` of a `DecompositionDecl`).
    pub(crate) fn emit_var_decl(&mut self, d: DeclId) -> R<()> {
        self.emit_one_var_decl(d)?;
        let ast = self.ast;
        if let Some(l) = ast.decls.decomp_bindings(d) {
            for b in ast.lists.iter::<DeclId>(l).collect::<Vec<_>>() {
                if let Some(h) = ast.decls.binding_var(b) {
                    self.emit_one_var_decl(h)?;
                }
            }
        }
        Ok(())
    }

    fn emit_one_var_decl(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        let Some(v) = ast.decls.var(d) else { return Ok(()) };
        match v.spec.storage() {
            VarStorage::Static => {
                self.static_local(d)?;
                return Ok(());
            }
            VarStorage::Extern => return Ok(()),
            _ => {}
        }
        let q = ast.decls.get(d).ty().expect("var type");
        let ty = self.mty(q);
        let dtor_class = self.destructed_class(q);
        // NRVO: the variable lives in the return slot (clang
        // `EmitAutoVarAlloca`), with a flag telling its cleanup whether
        // the return happened.
        let nrvo_slot =
            self.f.ret_slot.filter(|_| v.spec.is_nrvo() && ast.types.as_record(q).is_some());
        let (addr, flag) = if let Some(slot) = nrvo_slot {
            let addr = Addr { v: slot.v, ty, align: slot.align };
            let flag = if dtor_class.is_some() {
                let f = self.f.fir.alloca(LTy::I1, 1);
                let fls = V::C(self.m.const_bool(false));
                self.store(fls, f, 1, false);
                self.f.nrvo_flags.insert(d, f);
                Some(f)
            } else {
                None
            };
            (addr, flag)
        } else {
            let align = self.global_align(q);
            let a = self.f.fir.alloca(ty, align);
            (Addr { v: a, ty, align }, None)
        };
        self.f.locals.insert(d, addr);
        // Default-initialization by a trivial constructor emits nothing
        // (but a non-trivial destructor still gets its cleanup).
        let init = v.init.filter(|&i| {
            !ast.is_trivial_default_construction(i) || ast.exprs.construct_info(i).zeroing
        });
        if let Some(init) = init
            && self.f.fir.cur().is_some()
        {
            self.emit_var_init(d, q, addr, init)?;
        }
        // clang `EmitAutoVarCleanups`: no insertion point, no cleanup
        // (sema rejects jumps into the variable's scope).
        if self.f.fir.cur().is_none() {
            return Ok(());
        }
        if let Some(rd) = dtor_class {
            let c = if self.array_elem(q).is_some() {
                Cleanup::ArrayDtor { addr, record: rd }
            } else if let Some(flag) = flag {
                Cleanup::NrvoDtor { addr: addr.v, record: rd, flag }
            } else {
                Cleanup::Dtor { addr: addr.v, record: rd, base: false }
            };
            self.push_cleanup(c);
        }
        Ok(())
    }

    /// clang `EmitAutoVarInit` for variable `d` of type `q` at `addr`.
    fn emit_var_init(&mut self, d: DeclId, q: QualType, addr: Addr, init: ExprId) -> R<()> {
        let ast = self.ast;
        if self.referee(q).is_some() {
            // A reference binds the address of its initializer glvalue.
            let p = self.emit_ref_binding(init)?;
            self.store(p, addr.v, addr.align, false);
            return Ok(());
        }
        let fold = self.folds.get(init);
        if self.is_aggregate(q) {
            // clang `IsConstantAggregate`: constexpr, or POD with a
            // constant initializer.
            let constexpr = ast.decls.var(d).is_some_and(|v| v.spec.is_constexpr());
            if let Some(c) = fold.filter(|_| constexpr || self.is_pod(q)) {
                let k = self.const_value(c, q, Some(init), true)?;
                return self.stores_for_constant(d, addr, k, self.is_volatile(q));
            }
            return self.emit_agg_into(init, addr, false);
        }
        let v = match fold {
            Some(c) => V::C(self.const_scalar_value(c, q)?),
            None => self.emit_scalar(init)?,
        };
        self.store_scalar(v, addr, q);
        Ok(())
    }

    fn const_elems(&mut self, k: Value) -> Option<Vec<Value>> {
        match self.m.const_of(k)?.clone() {
            ConstData::Array { elems, .. } | ConstData::Struct { elems, .. } => {
                Some(elems.to_vec())
            }
            ConstData::Bytes { bytes, .. } => {
                Some(bytes.iter().map(|&b| self.m.const_int(LTy::I8, i128::from(b))).collect())
            }
            _ => None,
        }
    }

    fn is_scalar_const(&self, k: Value) -> bool {
        matches!(
            self.m.const_of(k),
            Some(
                ConstData::Int { .. }
                    | ConstData::Float { .. }
                    | ConstData::Gep { .. }
                    | ConstData::Cast { .. }
                    | ConstData::Binary { .. }
            )
        ) || matches!(self.m.const_of(k), Some(ConstData::Null(_)))
    }

    fn is_undef_const(&self, k: Value) -> bool {
        matches!(self.m.const_of(k), Some(ConstData::Undef(_) | ConstData::Poison(_)))
    }

    /// `Constant::isNullValue` (deep for aggregates of zeros).
    fn const_is_null(&mut self, k: Value) -> bool {
        if self.is_null_value(k) {
            return true;
        }
        match self.const_elems(k) {
            Some(es) => !es.is_empty() && es.into_iter().all(|x| self.const_is_null(x)),
            None => false,
        }
    }

    /// clang `canEmitInitWithFewStoresAfterBZero`.
    fn few_stores(&mut self, k: Value, budget: &mut u32) -> bool {
        if self.const_is_null(k) || self.is_undef_const(k) {
            return true;
        }
        if self.is_scalar_const(k) {
            if *budget == 0 {
                return false;
            }
            *budget -= 1;
            return true;
        }
        match self.const_elems(k) {
            Some(es) => es.into_iter().all(|x| self.few_stores(x, budget)),
            None => false,
        }
    }

    /// `llvm::isBytewiseValue`: `Some(None)` is undef (any byte).
    #[allow(clippy::option_option, reason = "None / undef / byte, as in LLVM")]
    fn bytewise(&mut self, k: Value) -> Option<Option<u8>> {
        if self.is_undef_const(k) {
            return Some(None);
        }
        if self.const_is_null(k) {
            return Some(Some(0));
        }
        match self.m.const_of(k)?.clone() {
            ConstData::Int { ty, value: kiln_llvm::IntValue::Small(x) } => {
                let w = self.m.types.int_width(ty)?;
                if w % 8 != 0 {
                    return None;
                }
                let b = (x & 0xff) as u8;
                (0..w / 8).all(|i| ((x >> (8 * i)) & 0xff) as u8 == b).then_some(Some(b))
            }
            ConstData::Float { ty, bits } => {
                let w = self.m.types.float_width(ty)?;
                let b = (bits & 0xff) as u8;
                (0..w / 8).all(|i| ((bits >> (8 * i)) & 0xff) as u8 == b).then_some(Some(b))
            }
            _ => {
                let es = self.const_elems(k)?;
                let mut acc: Option<u8> = None;
                for x in es {
                    match self.bytewise(x)? {
                        None => {}
                        Some(b) => match acc {
                            None => acc = Some(b),
                            Some(a) if a == b => {}
                            Some(_) => return None,
                        },
                    }
                }
                Some(acc)
            }
        }
    }

    /// clang `emitStoresForInitAfterBZero`.
    fn stores_after_bzero(&mut self, k: Value, at: Addr, vol: bool) {
        if self.is_scalar_const(k) {
            self.store(V::C(k), at.v, at.align, vol);
            return;
        }
        let Some(es) = self.const_elems(k) else { return };
        for (i, x) in es.into_iter().enumerate() {
            if self.const_is_null(x) || self.is_undef_const(x) {
                continue;
            }
            let off = self.elem_offset(at.ty, i as u64);
            let et = self.m.const_type(x);
            let z = self.icn(LTy::I32, 0);
            let iv = self.icn(LTy::I32, i as i128);
            let p = self.f.fir.push(
                Op::Gep {
                    src: at.ty,
                    base: at.v,
                    idx: vec![z, iv],
                    flags: kiln_llvm::GepFlags::INBOUNDS,
                },
                LTy::PTR,
            );
            self.stores_after_bzero(x, Addr { v: p, ty: et, align: min_align(at.align, off) }, vol);
        }
    }

    /// clang `emitStoresForConstant`.
    /// clang 19 `CreateCoercedStore` of constant `k` into `dst` (whose
    /// size is that of its IR type), with `BuildAggStore` splitting a
    /// first-class struct into one store per element.
    fn coerced_store_const(&mut self, k: Value, dst: Addr, size: Option<u64>, vol: bool) {
        let dst_size = size.unwrap_or_else(|| self.alloc_size(dst.ty));
        if dst_size == 0 {
            return;
        }
        let st = self.m.const_type(k);
        let src_size = self.alloc_size(st);
        let mut dst = dst;
        if st != dst.ty {
            dst = self.enter_struct_for_coerce(dst, src_size);
        }
        if src_size > dst_size {
            // `CreateTempAllocaForCoercion`: at least the preferred
            // alignment (8 for a struct, `a:0:64` on both targets).
            let pref = if self.m.types.struct_fields(st).is_some() {
                let abi = if self.m.types.struct_is_packed(st) { 1 } else { self.ir_layout(st).1 };
                abi.max(8)
            } else {
                self.ir_layout(st).1
            };
            let a = pref.max(dst.align);
            let tmp = self.f.fir.alloca(st, a);
            self.store(V::C(k), tmp, a, vol);
            self.memcpy_v(dst, tmp, a, dst_size);
            return;
        }
        let Some(fields) = self.struct_fields(st) else {
            self.store(V::C(k), dst.v, dst.align, vol);
            return;
        };
        let elems = match self.const_elems(k) {
            Some(es) if es.len() == fields.len() => es,
            _ => match self.m.const_of(k) {
                Some(ConstData::Zero(_)) => fields.iter().map(|&f| self.m.const_zero(f)).collect(),
                Some(ConstData::Undef(_)) => {
                    fields.iter().map(|&f| self.m.const_undef(f)).collect()
                }
                _ => {
                    self.store(V::C(k), dst.v, dst.align, vol);
                    return;
                }
            },
        };
        let offs = self.struct_field_offsets(st);
        for (i, ek) in elems.into_iter().enumerate() {
            let p = self.struct_gep(st, dst.v, i as u32);
            self.store(V::C(ek), p, min_align(dst.align, offs[i]), vol);
        }
    }

    fn stores_for_constant(&mut self, d: DeclId, at: Addr, k: Value, vol: bool) -> R<()> {
        let ty = self.m.const_type(k);
        let size = self.alloc_size(ty);
        if size == 0 {
            return Ok(());
        }
        if self.is_scalar_const(k) {
            self.store(V::C(k), at.v, at.align, vol);
            return Ok(());
        }
        let zero_agg = matches!(self.m.const_of(k), Some(ConstData::Zero(_)));
        let bzero = zero_agg || (size > 32 && self.few_stores(k, &mut 6));
        if bzero {
            self.memset(at, 0, size, vol);
            if !self.const_is_null(k) && !self.is_undef_const(k) {
                self.stores_after_bzero(k, Addr { v: at.v, ty, align: at.align }, vol);
            }
            return Ok(());
        }
        if size > 32
            && let Some(b) = self.bytewise(k)
        {
            self.memset(at, b.unwrap_or(0), size, vol);
            return Ok(());
        }
        let fname = self.f.func.map(|f| self.m.func(f).name().to_owned()).unwrap_or_default();
        let name = format!("__const.{fname}.{}", mangle::decl_name(self.ast, d));
        let g = self.private_const(&name, k, at.align);
        self.memcpy(at, V::C(gval(g)), at.align, size, vol);
        Ok(())
    }
}

/// The expression a `CXXDefaultArgExpr` stands for: its parameter's
/// default argument, evaluated afresh at each call ([dcl.fct.default]/9).
fn default_arg_of(ast: &kiln_ast::Ast, e: ExprId) -> Option<ExprId> {
    let p = ast.exprs.default_arg_param(e);
    ast.decls.param(p).and_then(|x| x.default_arg)
}
