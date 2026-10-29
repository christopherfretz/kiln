//! Expression emission (spec/codegen.md §3): clang's `ScalarExprEmitter`,
//! `EmitLValue`, `AggExprEmitter` and `EmitBranchOnBoolExpr` at `-O0`.

use kiln_abi::call::ArgKind;
use kiln_abi::irtype::BitFieldInfo;
use kiln_abi::mangle;
use kiln_ast::decl::{DeclKind, VarStorage};
use kiln_ast::expr::{BinaryOp, CastKind, ExprFlags, ExprKind, UnaryOp, ValueCategory};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_llvm::{
    Attr, AttrKind, AttrList, BinOp, CallOpts, CastOp, ConstData, Flags, FloatPredicate,
    IntPredicate, TypeId as LTy, Value,
};

use crate::fir::{Fir, Op, V};
use crate::fold::is_local;
use crate::func::{Addr, CallArg, min_align};
use crate::module::{Cg, R, err, gval};

/// The value an `OpaqueValueExpr` stands for.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Opaque {
    Rv(Option<V>),
    Lv(LV),
}

/// An lvalue: an address, its source type and the bit-field access.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LV {
    pub addr: Addr,
    pub q: QualType,
    pub bf: Option<BitFieldInfo>,
}

fn low_mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

impl Cg<'_> {
    #[allow(clippy::unnecessary_wraps, reason = "diagnostic location slot")]
    fn loc_of(&self, e: ExprId) -> Option<kiln_source::Loc> {
        Some(self.ast.exprs.get(e).loc())
    }

    fn sub(&self, e: ExprId) -> ExprId {
        ExprId::new(self.ast.exprs.get(e).a())
    }

    fn is_glvalue(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).value_category() != ValueCategory::PRValue
    }

    fn is_volatile(&self, q: QualType) -> bool {
        self.ast.types.effective_quals(q).is_volatile()
    }

    /// The natural alignment of objects of type `q`.
    fn natural_align(&mut self, q: QualType) -> u64 {
        if self.ast.types.is_void(q) || self.ast.types.is_function(q) {
            return 1;
        }
        self.size_align(q).1
    }

    fn i64c(&mut self, v: i128) -> V {
        V::C(self.m.const_int(LTy::I64, v))
    }

    fn icn(&mut self, ty: LTy, v: i128) -> V {
        V::C(self.m.const_int(ty, v))
    }

    /// A floating constant `x` (±1.0 and 0.0 only) of type `ty`.
    fn fconst(&mut self, ty: LTy, x: f64) -> V {
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

    fn is_float_ty(&self, t: LTy) -> bool {
        self.m.types.float_width(t).is_some()
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
            ExprKind::CXXDefaultArgExpr => match default_arg_of(ast, e) {
                Some(d) => return self.emit_scalar_opt(d),
                None => return err(self.loc_of(e), "default argument not yet parsed"),
            },
            ExprKind::ExprWithCleanups => return self.emit_scalar_opt(self.sub(e)),
            ExprKind::ConstantExpr => match ast.exprs.constant_value(e) {
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
            ExprKind::CXXNullPtrLiteralExpr => V::C(self.m.const_null(LTy::PTR)),
            ExprKind::ImplicitValueInitExpr | ExprKind::CXXScalarValueInitExpr => {
                if ast.types.is_void(q) {
                    return Ok(None);
                }
                let t = self.lty(q);
                V::C(self.m.const_zero(t))
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
            ExprKind::CallExpr => return self.emit_call(e, None),
            // A scalar initializer list (clang `VisitInitListExpr`): its
            // single element, or zero.
            ExprKind::InitListExpr if !self.is_aggregate(q) => {
                match ast.lists.get(ast.exprs.init_list_inits(e)).first() {
                    Some(&x) => return self.emit_scalar_opt(ExprId::new(x)),
                    None => {
                        let t = self.lty(q);
                        V::C(self.m.const_zero(t))
                    }
                }
            }
            ExprKind::DeclRefExpr
            | ExprKind::MemberExpr
            | ExprKind::ArraySubscriptExpr
            | ExprKind::StringLiteral
            | ExprKind::MaterializeTemporaryExpr => {
                let lv = self.emit_lvalue(e)?;
                self.load_lv(lv)
            }
            k => return err(self.loc_of(e), format!("expression {k:?} is not supported yet")),
        }))
    }

    /// clang `EmitLoadOfLValue` for scalars.
    pub(crate) fn load_lv(&mut self, lv: LV) -> V {
        let Some(bf) = lv.bf else { return self.load_scalar(lv.addr, lv.q) };
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
        let Some(bf) = lv.bf else {
            self.store_scalar(v, lv.addr, lv.q);
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
                return self.emit_scalar_opt(s);
            }
            CastKind::NoOp | CastKind::BitCast | CastKind::LValueBitCast => {
                return self.emit_scalar_opt(s);
            }
            CastKind::ToVoid => {
                self.emit_ignored(s)?;
                return Ok(None);
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
    fn decay(&mut self, a: Addr) -> V {
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
            UnaryOp::LNot => {
                let b = self.eval_bool(s)?;
                let t = V::C(self.m.const_bool(true));
                let x = self.bin(BinOp::Xor, Flags::NONE, b, t);
                let rt = self.lty(q);
                Ok(self.zext_or_same(x, rt))
            }
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
        let ast = self.ast;
        let inc = matches!(op, UnaryOp::PreInc | UnaryOp::PostInc);
        let pre = matches!(op, UnaryOp::PreInc | UnaryOp::PreDec);
        let q = lv.q;
        let old = self.load_lv(lv);
        let amount: i128 = if inc { 1 } else { -1 };
        let new = if self.is_bool(q) && inc {
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
        };
        let stored = self.store_lv(new, lv, lv.bf.is_some());
        Ok(if pre { stored } else { old })
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
    fn arith(
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
        let signed = self.is_signed(q);
        let nsw = if signed { Flags::NSW } else { Flags::NONE };
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
                    self.fcmp(p, l, r)
                } else {
                    let s = !lp && self.is_signed(lq);
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
                    self.icmp(p, l, r)
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
        let v = self.eval_bool(r)?;
        let rb = self.f.fir.cur().expect("insert point");
        self.emit_block(end, false);
        self.f.fir.add_incoming(Fir::last_inst(phi), v, rb);
        Ok(self.zext_or_same(phi, rt))
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
                if and {
                    self.branch_on_bool(l, mid, f)?;
                } else {
                    self.branch_on_bool(l, t, mid)?;
                }
                self.emit_block(mid, false);
                self.branch_on_bool(r, t, f)
            }
            ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::LNot => {
                self.branch_on_bool(ast.exprs.operand(e), f, t)
            }
            ExprKind::ConditionalOperator => {
                let c = self.sub(e);
                let [a, b] = ast.exprs.cond_arms(e);
                let lb = self.f.fir.create_block();
                let rb = self.f.fir.create_block();
                self.branch_on_bool(c, lb, rb)?;
                self.emit_block(lb, false);
                self.branch_on_bool(a, t, f)?;
                self.emit_block(rb, false);
                self.branch_on_bool(b, t, f)
            }
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
        let src = self.ast.exprs.opaque_source(e)?;
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
        self.branch_on_bool(c, lb, rb)?;
        self.emit_block(lb, false);
        let x = self.emit_scalar_opt(a)?;
        let lb = self.f.fir.cur().expect("insert point");
        self.br(cont);
        self.emit_block(rb, false);
        let y = self.emit_scalar_opt(b)?;
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
                })
            }
            DeclKind::Var | DeclKind::ParmVar => {
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
                    return Ok(LV { addr: a, q, bf: None });
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
                Ok(LV { addr: Addr { v: V::C(gval(g)), ty, align }, q, bf: None })
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

    /// The lvalue of type `q` at the address `p` a reference holds.
    pub(crate) fn ref_lvalue(&mut self, p: V, q: QualType) -> LV {
        let ty = if self.ast.types.is_function(q) { LTy::PTR } else { self.mty(q) };
        let (_, align) = self.pointee_layout(q);
        LV { addr: Addr { v: p, ty, align }, q, bf: None }
    }

    /// clang `EmitLValue`.
    pub(crate) fn emit_lvalue(&mut self, e: ExprId) -> R<LV> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        match n.kind() {
            ExprKind::ParenExpr => self.emit_lvalue(ast.exprs.operand(e)),
            ExprKind::CXXDefaultArgExpr => match default_arg_of(ast, e) {
                Some(d) => self.emit_lvalue(d),
                None => err(self.loc_of(e), "default argument not yet parsed"),
            },
            ExprKind::ExprWithCleanups => self.emit_lvalue(self.sub(e)),
            ExprKind::ConstantExpr => self.emit_lvalue(ast.exprs.operand(e)),
            ExprKind::SubstNonTypeTemplateParmExpr => self.emit_lvalue(ExprId::new(n.b())),
            ExprKind::DeclRefExpr => self.decl_lvalue(e, ast.exprs.decl(e), q),
            ExprKind::StringLiteral => {
                let g = self.lit_global(ast.exprs.string_lit(e));
                let ty = self.m.global(g).value_type();
                let align = self.m.global(g).align.unwrap_or(1);
                Ok(LV { addr: Addr { v: V::C(gval(g)), ty, align }, q, bf: None })
            }
            ExprKind::MemberExpr => self.member_lvalue(e),
            ExprKind::ArraySubscriptExpr => self.subscript_lvalue(e),
            ExprKind::UnaryOperator => match ast.exprs.unary_op(e) {
                UnaryOp::Deref => {
                    let p = self.emit_scalar(ast.exprs.operand(e))?;
                    let ty = if ast.types.is_function(q) { LTy::PTR } else { self.mty(q) };
                    let align = self.natural_align(q);
                    Ok(LV { addr: Addr { v: p, ty, align }, q, bf: None })
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
            ExprKind::CallExpr if self.is_glvalue(e) => {
                // A call returning a reference.
                let Some(p) = self.emit_call(e, None)? else {
                    return err(self.loc_of(e), "reference-returning call produced no value");
                };
                Ok(self.ref_lvalue(p, q))
            }
            ExprKind::MaterializeTemporaryExpr | ExprKind::CallExpr | ExprKind::InitListExpr => {
                let s =
                    if n.kind() == ExprKind::MaterializeTemporaryExpr { self.sub(e) } else { e };
                let sq = ast.exprs.get(s).ty();
                let tmp = self.temp(sq);
                if self.is_aggregate(sq) {
                    self.emit_agg(s, tmp, false)?;
                } else {
                    let v = self.emit_scalar(s)?;
                    self.store_scalar(v, tmp, sq);
                }
                Ok(LV { addr: tmp, q: sq, bf: None })
            }
            k if k.is_cast() => match ast.exprs.cast_kind(e) {
                CastKind::NoOp | CastKind::LValueBitCast | CastKind::LValueToRValue => {
                    let mut lv = self.emit_lvalue(ast.exprs.operand(e))?;
                    if lv.bf.is_none() && !ast.types.is_function(q) {
                        lv.addr.ty = self.mty(q);
                    }
                    lv.q = q;
                    Ok(lv)
                }
                k => err(self.loc_of(e), format!("lvalue cast {k:?} is not supported yet")),
            },
            k => err(self.loc_of(e), format!("lvalue {k:?} is not supported yet")),
        }
    }

    fn cond_lvalue(&mut self, e: ExprId, c: ExprId, a: ExprId, b: ExprId) -> R<LV> {
        let ast = self.ast;
        if let Some(x) = self.fold_bool(c) {
            return self.emit_lvalue(if x { a } else { b });
        }
        let lb = self.f.fir.create_block();
        let rb = self.f.fir.create_block();
        let cont = self.f.fir.create_block();
        self.branch_on_bool(c, lb, rb)?;
        self.emit_block(lb, false);
        let x = self.emit_lvalue(a)?;
        let lb = self.f.fir.cur().expect("insert point");
        self.br(cont);
        self.emit_block(rb, false);
        let y = self.emit_lvalue(b)?;
        let rb = self.f.fir.cur().expect("insert point");
        self.emit_block(cont, false);
        let p = self
            .f
            .fir
            .push(Op::Phi { ty: LTy::PTR, inc: vec![(x.addr.v, lb), (y.addr.v, rb)] }, LTy::PTR);
        let align = x.addr.align.min(y.addr.align);
        Ok(LV { addr: Addr { v: p, ty: x.addr.ty, align }, q: ast.exprs.get(e).ty(), bf: None })
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
            return Ok(LV { addr: Addr { v, ty: st, align }, q, bf: Some(bf) });
        }
        if ast.decls.get(field).ty().is_some_and(|t| self.referee(t).is_some()) {
            return err(
                Some(ast.decls.get(field).loc()),
                "a reference member is not supported yet",
            );
        }
        let ty = self.mty(q);
        if is_union {
            return Ok(LV { addr: Addr { v: base.v, ty, align: base.align }, q, bf: None });
        }
        let Some(i) = idx else {
            return Ok(LV { addr: Addr { v: base.v, ty, align: base.align }, q, bf: None });
        };
        let v = self.struct_gep(rty, base.v, i);
        let align = min_align(base.align, self.elem_offset(rty, u64::from(i)));
        Ok(LV { addr: Addr { v, ty, align }, q, bf: None })
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

    /// clang `EmitArraySubscriptExpr`.
    fn subscript_lvalue(&mut self, e: ExprId) -> R<LV> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
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
        Ok(LV { addr: Addr { v, ty: ety, align }, q, bf: None })
    }

    // ---- ignored expressions -------------------------------------------

    /// clang `EmitIgnoredExpr`.
    pub(crate) fn emit_ignored(&mut self, e: ExprId) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        if !self.is_glvalue(e) {
            if self.is_aggregate(q) {
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
                    ExprKind::ImplicitCastExpr
                        if ast.exprs.cast_kind(c) == CastKind::FunctionToPointerDecay =>
                    {
                        c = ast.exprs.operand(c);
                    }
                    ExprKind::DeclRefExpr => {
                        let d = ast.exprs.decl(c);
                        if matches!(ast.decls.get(d).kind(), DeclKind::Function) {
                            direct = Some(d);
                        }
                        break;
                    }
                    _ => break,
                }
            }
        }
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        let arg_tys: Vec<QualType> = args.iter().map(|&a| ast.exprs.get(a).ty()).collect();
        let (callee, fty, extern_c, fid) = if let Some(d) = direct {
            let f = self.get_fn(d)?;
            let fty = ast.decls.get(d).ty().expect("function type");
            (V::C(Value::func(f)), fty, mangle::is_extern_c(ast, ast.first(d)), Some(f))
        } else {
            let p = self.emit_scalar(callee_e)?;
            let pq = ast.exprs.get(callee_e).ty();
            let fty = ast.types.pointee(pq).expect("function pointer");
            (p, fty, false, None)
        };
        let sig = self.call_sig(fty, extern_c, &arg_tys);
        self.f.vector_width = self.f.vector_width.max(sig.info.max_vector_width);
        let rq = sig.ret;
        // EmitCallArgs: evaluate every argument, aggregates into `agg.tmp`.
        let mut vals = Vec::with_capacity(args.len());
        for (i, &a) in args.iter().enumerate() {
            let v = if self.referee(sig.params[i]).is_some() {
                // A reference parameter binds the argument glvalue.
                CallArg::Scalar(self.emit_lvalue(a)?.addr.v)
            } else if self.is_aggregate(arg_tys[i]) {
                let t = self.temp(arg_tys[i]);
                self.emit_agg(a, t, false)?;
                CallArg::Agg(t)
            } else {
                CallArg::Scalar(self.emit_scalar(a)?)
            };
            vals.push(v);
        }
        // EmitCall.
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
        for (i, &v) in vals.iter().enumerate() {
            let Some(info) = sig.info.args.get(i) else {
                return err(self.loc_of(args[i]), "call argument without ABI information");
            };
            let kind = info.kind.clone();
            let loc = self.loc_of(args[i]);
            self.call_arg(&kind, sig.params[i], v, &mut ir_args, loc)?;
        }
        let rty = match &sig.info.ret.kind {
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                let c = coerce.clone();
                self.ir(&c)
            }
            _ => LTy::VOID,
        };
        let (ret, params) = self.sig_attrs(&sig, args.len(), false);
        let func = if sig.nothrow {
            self.m.attr_set(&[Attr::Flag(AttrKind::NoUnwind)])
        } else {
            AttrList::default().func
        };
        let opts = CallOpts { attrs: AttrList { func, ret, params }, ..CallOpts::default() };
        let r = self
            .f
            .fir
            .push(Op::Call { fn_ty: sig.fn_ty, callee, args: ir_args, opts: Box::new(opts) }, rty);
        self.f.calls.push((Fir::last_inst(r), fid, sig.nothrow));
        if let Some(a) = sret_addr {
            let _ = a;
            return Ok(None);
        }
        if rty == LTy::VOID {
            return Ok(None);
        }
        if self.is_aggregate(rq) {
            let d = match dest {
                Some(d) => d,
                None => self.temp(rq),
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
                let (size, _) = self.size_align(rq);
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
            ExprKind::ParenExpr | ExprKind::ConstantExpr => {
                self.agg_visit(ast.exprs.operand(e), dest, zeroed)
            }
            ExprKind::CXXDefaultArgExpr => match default_arg_of(ast, e) {
                Some(d) => self.agg_visit(d, dest, zeroed),
                None => err(self.loc_of(e), "default argument not yet parsed"),
            },
            ExprKind::ExprWithCleanups | ExprKind::MaterializeTemporaryExpr => {
                self.agg_visit(self.sub(e), dest, zeroed)
            }
            ExprKind::InitListExpr => {
                if ast.types.is_array(q) {
                    self.array_init(e, dest, zeroed)
                } else {
                    self.record_init(e, dest, zeroed)
                }
            }
            ExprKind::CallExpr if self.is_glvalue(e) => {
                let lv = self.emit_lvalue(e)?;
                let size = self.size_align(q).0;
                self.memcpy(dest, lv.addr.v, lv.addr.align, size, false);
                Ok(())
            }
            ExprKind::CallExpr => {
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
                && matches!(ast.exprs.cast_kind(e), CastKind::NoOp | CastKind::LValueToRValue) =>
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
            ExprKind::StringLiteral => {
                // A literal initializing a (possibly longer) char array:
                // copy from the literal's constant padded to the array.
                let size = self.size_align(q).0;
                let g = self.lit_global_sized(ast.exprs.string_lit(e), size);
                let align = self.m.global(g).align.unwrap_or(1);
                self.memcpy(dest, V::C(gval(g)), align, size, false);
                Ok(())
            }
            k => err(self.loc_of(e), format!("aggregate expression {k:?} is not supported yet")),
        }
    }

    /// clang `AggExprEmitter::EmitInitializationToLValue`.
    fn init_to_lvalue(&mut self, e: ExprId, lv: LV, zeroed: bool) -> R<()> {
        let ast = self.ast;
        if zeroed && self.is_simple_zero(e) {
            return Ok(());
        }
        let k = ast.exprs.get(e).kind();
        if matches!(k, ExprKind::ImplicitValueInitExpr | ExprKind::CXXScalarValueInitExpr) {
            return self.null_init_lvalue(lv, zeroed);
        }
        if self.is_aggregate(lv.q) {
            return self.emit_agg(e, lv.addr, zeroed);
        }
        let v = self.emit_scalar(e)?;
        self.store_lv(v, lv, false);
        Ok(())
    }

    /// clang `EmitNullInitializationToLValue`.
    fn null_init_lvalue(&mut self, lv: LV, zeroed: bool) -> R<()> {
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
            let lv = self.field_lvalue(base, f, fq)?;
            return match inits.first() {
                Some(&i) => self.init_to_lvalue(i, lv, zeroed),
                None => self.null_init_lvalue(lv, zeroed),
            };
        }
        let fields: Vec<DeclId> =
            ast.decls.members(rd).filter(|&m| ast.decls.get(m).kind() == DeclKind::Field).collect();
        let mut k = 0usize;
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
            let lv = self.field_lvalue(base, f, fq)?;
            if k < inits.len() {
                self.init_to_lvalue(inits[k], lv, zeroed)?;
                k += 1;
            } else {
                self.null_init_lvalue(lv, zeroed)?;
            }
        }
        Ok(())
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
        for (i, &init) in inits.iter().enumerate() {
            let el = if i == 0 {
                begin
            } else {
                let c = self.i64c(i as i128);
                self.gep(ety, begin, vec![c], true)
            };
            let lv = LV { addr: Addr { v: el, ty: ety, align: ealign }, q: eq, bf: None };
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
            }
            let c = self.i64c(i128::from(n_elems));
            let end = self.gep(ety, begin, vec![c], true);
            let entry = self.f.fir.cur().expect("insert point");
            let body = self.f.fir.create_block();
            self.emit_block(body, false);
            let cur = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(el, entry)] }, LTy::PTR);
            let lv = LV { addr: Addr { v: cur, ty: ety, align: ealign }, q: eq, bf: None };
            match filler {
                Some(f) => self.init_to_lvalue(f, lv, false)?,
                None => self.null_init_lvalue(lv, false)?,
            }
            let one = self.i64c(1);
            let next = self.gep(ety, cur, vec![one], true);
            let done = self.icmp(IntPredicate::Eq, next, end);
            let endb = self.f.fir.create_block();
            self.cond_br(done, endb, body);
            let here = self.f.fir.cur().expect("insert point");
            self.f.fir.add_incoming(Fir::last_inst(cur), next, here);
            self.emit_block(endb, false);
        }
        Ok(())
    }

    fn is_trivial_filler(&self, e: ExprId) -> bool {
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

    /// clang `EmitVarDecl` for a block-scope variable.
    pub(crate) fn emit_var_decl(&mut self, d: DeclId) -> R<()> {
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
        let align = self.global_align(q);
        let a = self.f.fir.alloca(ty, align);
        let addr = Addr { v: a, ty, align };
        self.f.locals.insert(d, addr);
        let Some(init) = v.init else { return Ok(()) };
        if self.f.fir.cur().is_none() {
            return Ok(());
        }
        if self.referee(q).is_some() {
            // A reference binds the address of its initializer glvalue.
            if !self.is_glvalue(init) {
                return err(
                    self.loc_of(init),
                    "binding a reference to a temporary is not supported yet",
                );
            }
            let lv = self.emit_lvalue(init)?;
            self.store(lv.addr.v, a, align, false);
            return Ok(());
        }
        let fold = self.folds.get(init);
        if self.is_aggregate(q) {
            if let Some(c) = fold {
                let k = self.const_value(c, q, Some(init), true)?;
                return self.stores_for_constant(d, addr, k, self.is_volatile(q));
            }
            return self.emit_agg(init, addr, false);
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
