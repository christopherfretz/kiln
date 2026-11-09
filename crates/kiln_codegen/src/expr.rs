//! Expression emission (spec/codegen.md §3): clang's `ScalarExprEmitter`,
//! `EmitLValue`, `AggExprEmitter` and `EmitBranchOnBoolExpr` at `-O0`.

use kiln_abi::call::ArgKind;
use kiln_abi::irtype::BitFieldInfo;
use kiln_abi::mangle;
use kiln_ast::decl::{DeclKind, VarStorage};
use kiln_ast::expr::{
    BinaryOp, CastKind, ExprFlags, ExprKind, NonOdrUse, StorageDuration, UnaryOp, ValueCategory,
};
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_llvm::{
    Attr, AttrKind, AttrList, BinOp, CallOpts, CastOp, ConstData, Flags, FloatPredicate, FuncId,
    IntPredicate, TypeId as LTy, Value,
};

use crate::cleanup::{Cleanup, IrregularPartialArray};
use crate::coverage::pgo::{self, Key};
use crate::debuginfo::func::VarSlot;
use crate::fir::{Fir, Op, V};
use crate::fold::{is_local, is_static_local};
use crate::func::{Addr, CallArg, min_align};
use crate::module::{Cg, R, Sig, err, gval};
use crate::tbaa::Tb;
use crate::ubsan::{Tck, TypeCheck, UbDesc};
use kiln_ast::sanitize::SanitizerSet as San;

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
    /// An element, or a swizzle, of the vector at `addr` (clang
    /// `LValue::MakeVectorElt` / `MakeExtVectorElt`).
    pub vidx: Option<VecLane>,
    /// Its TBAA access path (clang `LValue::getTBAAInfo`).
    pub tb: Tb,
}

/// The part of a vector an [`LV`] designates.
#[derive(Clone, Copy, Debug)]
pub(crate) enum VecLane {
    /// One element, by a (possibly non-constant) index.
    Index(V),
    /// The lanes the `ExtVectorElementExpr` names (`v.xz`).
    Swizzle(ExprId),
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

    /// The integer constant `v` of type `ty`, splat across the lanes of a
    /// (fixed or scalable) vector type.
    pub(crate) fn icn(&mut self, ty: LTy, v: i128) -> V {
        if let Some((et, _, _)) = self.m.types.vector_info(ty) {
            let c = self.m.const_int(et, v);
            return self.const_splat_of(ty, c);
        }
        V::C(self.m.const_int(ty, v))
    }

    /// A floating constant `x` (±1.0 and 0.0 only) of type `ty`.
    pub(crate) fn fconst(&mut self, ty: LTy, x: f64) -> V {
        if let Some((et, _, _)) = self.m.types.vector_info(ty) {
            let V::C(c) = self.fconst(et, x) else {
                unreachable!("fconst of a scalar is a constant")
            };
            return self.const_splat_of(ty, c);
        }
        V::C(self.m.const_float(ty, x))
    }

    /// The vector constant of type `ty` with every lane `c`
    /// (`ConstantVector::getSplat`): a `<N x T>` vector, or a scalable
    /// `splat` for an SVE `<vscale x N x T>`.
    pub(crate) fn const_splat_of(&mut self, ty: LTy, c: Value) -> V {
        let Some((_, n, scalable)) = self.m.types.vector_info(ty) else { return V::C(c) };
        V::C(if scalable {
            self.m.const_splat(ty, c).expect("lane of the element type")
        } else {
            self.m.const_vector(&vec![c; n as usize]).expect("lanes of the element type")
        })
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
    /// An SVE result is not a `VectorType` there: the comparison's `bool`
    /// is converted to it (`EmitScalarConversion`), a zero extension
    /// (none for `svbool_t`).
    fn vec_cmp_result(&mut self, c: V, q: QualType) -> V {
        match self.vec_of(q) {
            Some((elem, _)) if !self.is_bool(elem) => {
                let t = self.lty(q);
                self.cast(CastOp::SExt, c, t)
            }
            None if self.ast.types.is_sizeless(q) => {
                let t = self.lty(q);
                self.zext_or_same(c, t)
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
        let dl = self.dbg_apply_expr(e);
        self.di_explicit_cast(e);
        let r = self.emit_scalar_x(e);
        self.dbg_restore(dl);
        r
    }

    /// `ScalarExprEmitter::Visit` without its debug location.
    fn emit_scalar_x(&mut self, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        Ok(Some(match n.kind() {
            ExprKind::ParenExpr => return self.emit_scalar_opt(ast.exprs.operand(e)),
            // GNU `__extension__` of a possibly `void` operand (arm_neon.h
            // `__extension__ ({ ... })` statement macros).
            ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::Extension => {
                return self.emit_scalar_opt(ast.exprs.operand(e));
            }
            // clang `getChosenSubExpr`: only the chosen operand is emitted.
            ExprKind::ChooseExpr if ast.exprs.choose_chosen(e).is_some() => {
                return self
                    .emit_scalar_opt(ast.exprs.choose_chosen(e).expect("match guard checked it"));
            }
            // clang `ScalarExprEmitter::VisitCompoundLiteralExpr`: a load
            // of the literal's temporary.
            ExprKind::CompoundLiteralExpr => {
                let lv = self.compound_literal_lvalue(e)?;
                self.load_lv(lv)
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
            // clang `ScalarExprEmitter::VisitExprWithCleanups`: the value
            // survives a shared cleanup block (`ForceCleanup({&V})`).
            ExprKind::ExprWithCleanups => {
                let sc = self.begin_scope(false);
                let mut r = self.emit_scalar_opt(self.sub(e));
                match &mut r {
                    Ok(Some(v)) => self.end_scope_reload(sc, &mut [v]),
                    _ => self.end_scope(sc),
                }
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
                let saved = self.enter_default_init(e);
                let r = self.emit_scalar_opt(init);
                self.exit_default_init(saved);
                return r;
            }
            // A glvalue's value is the address it designates (WP-640).
            ExprKind::ConstantExpr if self.is_glvalue(e) && self.immediate_value(e).is_some() => {
                let lv = self.emit_lvalue(e)?;
                self.load_lv(lv)
            }
            ExprKind::ConstantExpr => match self.immediate_value(e) {
                Some(c) if !ast.types.is_void(q) => V::C(self.const_scalar_value(c, q)?),
                // clang `ScalarExprEmitter::VisitConstantExpr`: a `void`
                // immediate invocation generates no code.
                Some(_) => return Ok(None),
                None => return self.emit_scalar_opt(ast.exprs.operand(e)),
            },
            ExprKind::IntegerLiteral => {
                if let Some(v) = ast.exprs.int_value(e) {
                    let t = self.lty(q);
                    self.icn(t, i128::from(v))
                } else {
                    let c = ast.exprs.literal_const(e).expect("Sema stored the value");
                    V::C(self.const_scalar_value(c, q)?)
                }
            }
            ExprKind::FloatingLiteral => {
                let c = ast.exprs.literal_const(e).expect("Sema stored the value");
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
            ExprKind::GNUNullExpr => {
                let t = self.lty(q);
                V::C(self.m.const_int(t, 0))
            }
            ExprKind::ImplicitValueInitExpr | ExprKind::CXXScalarValueInitExpr => {
                if ast.types.is_void(q) {
                    return Ok(None);
                }
                if self.zero_init(q) {
                    let t = self.lty(q);
                    V::C(self.m.const_zero(t))
                } else {
                    V::C(self.null_const(q)?)
                }
            }
            ExprKind::OffsetOfExpr => {
                let t = self.lty(q);
                V::C(self.m.const_int(t, i128::from(ast.exprs.offsetof_value(e))))
            }
            ExprKind::UnaryExprOrTypeTraitExpr => match self.folds.get(e) {
                Some(c) => V::C(self.const_scalar_value(c, q)?),
                None => match self.emit_vla_sizeof(e)? {
                    Some(v) => v,
                    None => return err(self.loc_of(e), "unfolded type trait expression"),
                },
            },
            ExprKind::SubstNonTypeTemplateParmExpr => {
                return self.emit_scalar_opt(ExprId::new(n.b()));
            }
            ExprKind::DeclRefExpr
                if ast.decls.get(ast.exprs.decl(e)).kind() == DeclKind::EnumConstant =>
            {
                let d = ast.exprs.decl(e);
                self.di_const_ref(d, None);
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
                self.di_const_ref(d, self.folds.get(e));
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
                return self.emit_conditional(e, self.sub(e), a, b);
            }
            ExprKind::BinaryConditionalOperator => {
                let [common, c, a, b] = self.bind_common(e)?;
                let r = self.emit_conditional(e, c, a, b);
                self.f.opaque.remove(&common);
                return r;
            }
            ExprKind::CoawaitExpr | ExprKind::CoyieldExpr => {
                return match self.emit_coawait(e, crate::coro::Want::Scalar)? {
                    crate::coro::Got::Scalar(v) => Ok(v),
                    _ => Ok(None),
                };
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
            ExprKind::PseudoObjectExpr => {
                let r = self.bind_pseudo(e)?;
                let v = self.emit_arm(r);
                self.unbind_pseudo(e);
                return v;
            }
            ExprKind::CXXNewExpr => self.emit_cxx_new(e)?,
            ExprKind::CXXDeleteExpr => {
                self.emit_cxx_delete(e)?;
                return Ok(None);
            }
            ExprKind::StmtExpr => return self.emit_stmt_expr(e),
            ExprKind::AtomicExpr => return self.emit_atomic_expr(e, None),
            ExprKind::VAArgExpr => return self.emit_va_arg(e, None),
            // A scalar initializer list (clang `VisitInitListExpr`): its
            // single element, or zero.
            // clang `VisitInitListExpr` on a vector: `insertelement` of
            // each element into `poison` (i32 indices), then zeros; a
            // vector element is two `shufflevector`s.
            ExprKind::InitListExpr if self.vec_of(q).is_some() => {
                let inits: Vec<u32> = ast.lists.get(ast.exprs.init_list_inits(e)).to_vec();
                let t = self.lty(q);
                if inits.is_empty() {
                    return Ok(Some(V::C(self.m.const_zero(t))));
                }
                let (et, n, _) = self.m.types.vector_info(t).expect("match checked: vector type");
                let mut v = V::C(self.m.const_poison(t));
                let mut cur = 0u32;
                for x in inits {
                    let x = self.emit_scalar(ExprId::new(x))?;
                    let xt = self.f.fir.ty(&self.m, x);
                    if let Some((_, k, _)) = self.m.types.vector_info(xt) {
                        // A vector element (sema keeps one only as the
                        // whole initializer of a vector of its type): clang
                        // widens it to the result's length, then shuffles
                        // its lanes in after the `cur` already set.
                        let p = kiln_llvm::SHUFFLE_POISON;
                        let pad = |m: &mut Vec<u32>| m.resize(n as usize, p);
                        let mut m: Vec<u32> = (0..k).collect();
                        pad(&mut m);
                        let wide = self.shuffle(x, None, m);
                        let off = if cur == 0 { 0 } else { n };
                        let mut m: Vec<u32> = (0..cur).chain((0..k).map(|j| j + off)).collect();
                        pad(&mut m);
                        v = if cur == 0 {
                            self.shuffle(wide, Some(v), m)
                        } else {
                            self.shuffle(v, Some(wide), m)
                        };
                        cur += k;
                        continue;
                    }
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
                if self.zero_init(q) {
                    let t = self.lty(q);
                    V::C(self.m.const_zero(t))
                } else {
                    V::C(self.null_const(q)?)
                }
            }
            // clang `ScalarExprEmitter::VisitArraySubscriptExpr`: a vector
            // subscript is the vector's value and `extractelement`.
            ExprKind::ArraySubscriptExpr if self.vec_subscript(e).is_some() => {
                let (base, idx) = self.vec_subscript(e).expect("match guard checked it");
                // A `const` vector variable folds (`tryEmitAsConstant`,
                // WP-453): no load, the lane is extracted from the constant.
                let v = match self.folds.get(base) {
                    Some(c) => {
                        let bq = ast.exprs.get(base).ty();
                        V::C(self.const_scalar_value(c, bq)?)
                    }
                    // A glvalue base is loaded from its address (a call
                    // returning `V&` yields the address as its scalar).
                    None if self.is_glvalue(base) => {
                        let lv = self.emit_lvalue(base)?;
                        self.load_lv(lv)
                    }
                    None => self.emit_scalar(base)?,
                };
                let i = self.emit_scalar(idx)?;
                self.ub_bounds(e, base, i, ast.exprs.get(idx).ty(), true)?;
                self.extract_elt(v, i)
            }
            ExprKind::ShuffleVectorExpr => {
                // clang `ScalarExprEmitter::VisitShuffleVectorExpr` with
                // scalar indices (a `-1` is a poison lane).
                let subs: Vec<ExprId> = ast.lists.iter(ast.exprs.shuffle_subs(e)).collect();
                let Some(mask) = ast.exprs.shuffle_mask(e, &ast.lists) else {
                    return err(self.loc_of(e), "a dependent __builtin_shufflevector");
                };
                let mask: Vec<u32> = mask
                    .iter()
                    .map(|&w| if w == u32::MAX { kiln_llvm::SHUFFLE_POISON } else { w })
                    .collect();
                let (Some(&x), Some(&y)) = (subs.first(), subs.get(1)) else {
                    return err(self.loc_of(e), "malformed ShuffleVectorExpr");
                };
                let a = self.emit_scalar(x)?;
                let b = self.emit_scalar(y)?;
                if subs.len() == 2 {
                    return Ok(Some(self.shuffle_by_mask(a, b)));
                }
                self.shuffle(a, Some(b), mask)
            }
            ExprKind::ConvertVectorExpr => {
                let src = ast.exprs.operand(e);
                let sq = ast.exprs.get(src).ty();
                // A `const` vector variable folds (WP-490), as a subscript base.
                let v = match self.folds.get(src) {
                    Some(c) => V::C(self.const_scalar_value(c, sq)?),
                    None => self.emit_scalar(src)?,
                };
                self.convert_vector(v, sq, q)
            }
            // clang `EmitExtVectorElementExpr` of a prvalue vector (`(v +
            // v).x`): the vector is stored to a temporary, then loaded as
            // an element (`extractelement`) or swizzle (`shufflevector`).
            ExprKind::ExtVectorElementExpr if !self.is_glvalue(e) => {
                let base = ast.exprs.operand(e);
                let lanes = self.swizzle_lanes(e, false)?;
                let bq = ast.exprs.get(base).ty();
                let v = self.emit_scalar(base)?;
                let tmp = self.temp(bq);
                self.store(v, tmp.v, tmp.align, false);
                let vidx = match lanes.as_slice() {
                    [l] => VecLane::Index(self.icn(LTy::I64, i128::from(*l))),
                    _ => VecLane::Swizzle(e),
                };
                self.load_lv(LV { addr: tmp, q, bf: None, vidx: Some(vidx), tb: Tb::Type })
            }
            // clang `ScalarExprEmitter::VisitDeclRefExpr` ->
            // `tryEmitAsConstant`: a variable named without being odr-used
            // ([basic.def.odr]/5), reached by the scalar visitor through a
            // glvalue conditional's arm, is its constant, never a load of a
            // global that need not be defined.
            ExprKind::DeclRefExpr if let Some(c) = self.non_odr_use_constant(e) => {
                self.di_const_ref(ast.exprs.decl(e), Some(c));
                V::C(self.const_scalar_value(c, q)?)
            }
            ExprKind::DeclRefExpr
            | ExprKind::MemberExpr
            | ExprKind::ArraySubscriptExpr
            | ExprKind::ExtVectorElementExpr
            | ExprKind::StringLiteral
            | ExprKind::PredefinedExpr
            | ExprKind::MaterializeTemporaryExpr => {
                let lv = self.emit_checked_lvalue(e, Tck::Load)?;
                self.load_lv_at(lv, self.ub_expr_loc(e))?
            }
            // clang `VisitAddrLabelExpr`.
            ExprKind::AddrLabelExpr => {
                V::C(self.addr_of_label(ast.exprs.addr_label(e), self.loc_of(e))?)
            }
            k => return err(self.loc_of(e), format!("expression {k:?} is not supported yet")),
        }))
    }

    /// clang `EmitLoadOfLValue` for scalars.
    pub(crate) fn load_lv(&mut self, lv: LV) -> V {
        match lv.vidx {
            Some(VecLane::Index(idx)) => {
                let vec = self.vec_elt_load(lv);
                return self.extract_elt(vec, idx);
            }
            Some(VecLane::Swizzle(e)) => {
                let vec = self.vec_elt_load(lv);
                // Sema accepted the accessor; a malformed one reads poison.
                let lanes = self.swizzle_lanes(e, true).unwrap_or_default();
                return self.swizzle_value(vec, &lanes);
            }
            None => {}
        }
        let Some(bf) = lv.bf else {
            if let Some(vq) = self.ast.types.as_atomic(lv.q) {
                return self.atomic_load_lv(lv, vq);
            }
            return self.load_scalar_tb(lv.addr, lv.q, lv.tb);
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
        if let Some(lane) = lv.vidx {
            let vec = self.vec_elt_load(lv);
            let ins = match lane {
                VecLane::Index(idx) => self.insert_elt(vec, v, idx),
                VecLane::Swizzle(e) => {
                    let lanes = self.swizzle_lanes(e, true).unwrap_or_default();
                    self.store_swizzle(v, vec, &lanes)
                }
            };
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
                self.store_scalar_tb(v, lv.addr, lv.q, lv.tb);
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
            if !self.has_bool_rep(lv.q) {
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
        // clang casts the stored value to the load/store type and takes it
        // `EmitFromMemory` (a `bool` or `bool`-based enumeration: `i8`,
        // then `i1`).
        let rt = self.lsty(lv.q);
        let r = self.int_cast(r, rt, bf.is_signed);
        self.emit_from_memory(r, lv.q)
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
        // An integer converted to an enumeration whose underlying type is
        // `bool` is first converted to `bool` ([expr.static.cast]/10;
        // clang's `CK_IntegralToBoolean` to the enumeration): `!= 0`, not
        // a truncation to `i1`.
        if self.has_bool_rep(to) && self.m.types.int_width(ft).is_some() {
            return self.conv_to_bool(v, from);
        }
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
            let op = if signed { CastOp::SIToFP } else { CastOp::UIToFP };
            if self.is_fp16(to) {
                // clang: integer to non-native `__fp16` goes through float.
                let f = self.cast(op, v, LTy::FLOAT);
                return self.cast(CastOp::FPTrunc, f, tt);
            }
            return self.cast(op, v, tt);
        }
        if ti {
            let v = if self.is_fp16(from) { self.cast(CastOp::FPExt, v, LTy::FLOAT) } else { v };
            return self.cast(
                if self.is_signed(to) { CastOp::FPToSI } else { CastOp::FPToUI },
                v,
                tt,
            );
        }
        let fw = self.m.types.float_width(ft).unwrap_or(0);
        let tw = self.m.types.float_width(tt).unwrap_or(0);
        if tw == fw {
            // `half` ↔ `bfloat` (P1467 explicit conversion): through
            // `float`, exact one way and singly rounded the other, as
            // clang lowers `(T)(float)x`.
            let w = self.cast(CastOp::FPExt, v, LTy::FLOAT);
            return self.cast(CastOp::FPTrunc, w, tt);
        }
        if tw < fw { self.cast(CastOp::FPTrunc, v, tt) } else { self.cast(CastOp::FPExt, v, tt) }
    }

    /// clang `EmitConversionToBool`.
    fn conv_to_bool(&mut self, v: V, from: QualType) -> V {
        let ft = self.f.fir.ty(&self.m, v);
        if ft == LTy::I1 {
            // clang `EmitIntToBoolConversion` of an `i1`: the value of an
            // enumeration with underlying type `bool` (or a `bool` cast
            // to one) is tested `!= false` (clang's `CK_IntegralToBoolean`).
            if self.has_bool_rep(from) {
                let z = self.icn(LTy::I1, 0);
                return self.icmp(IntPredicate::Ne, v, z);
            }
            return v;
        }
        if self.is_float_ty(ft) {
            let z = self.fconst(ft, 0.0);
            return self.fcmp(FloatPredicate::Une, v, z);
        }
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
        if !self.is_bool(q) {
            return Ok(self.conv_to_bool(v, q));
        }
        // A post-increment of `_Atomic(bool)` yields its old byte in memory
        // form (`i8`, `atomic_incdec`); clang 19 branches on it as is
        // (invalid IR), kiln truncates it (spec/codegen.md, WP-183).
        Ok(if self.f.fir.ty(&self.m, v) == LTy::I8 {
            self.cast(CastOp::Trunc, v, LTy::I1)
        } else {
            v
        })
    }

    fn zext_or_same(&mut self, v: V, to: LTy) -> V {
        let ft = self.f.fir.ty(&self.m, v);
        if ft == to { v } else { self.cast(CastOp::ZExt, v, to) }
    }

    /// clang `CK_BitCast` of a vector `v` to `t`: a plain
    /// bitcast, except between fixed-length and scalable vectors (WP-810):
    /// a vector insert or extract when the elements pair up, else through
    /// a `saved-value` temporary.
    fn vector_bitcast(&mut self, v: V, t: LTy) -> V {
        let from = self.f.fir.ty(&self.m, v);
        let scal = |m: &kiln_llvm::Module, x: LTy| m.types.vector_info(x).is_some_and(|i| i.2);
        let (fs, ts) = (scal(&self.m, from), scal(&self.m, t));
        if fs == ts {
            return self.cast(CastOp::BitCast, v, t);
        }
        let r = if ts { self.sve_to_scalable(v, t) } else { self.sve_to_fixed(v, t) };
        if let Some(r) = r {
            return r;
        }
        // `CreateDefaultAlignTempAlloca`: the preferred alignment, natural
        // (the size) for a fixed vector, 16 for a scalable one.
        let align = if fs { 16 } else { self.alloc_size(from).next_power_of_two() };
        let a = self.f.fir.alloca(from, align);
        self.store(v, a, align, false);
        self.load(t, a, align, false)
    }

    fn emit_cast(&mut self, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        let s = ast.exprs.operand(e);
        let sq = ast.exprs.get(s).ty();
        let ck = ast.exprs.cast_kind(e);
        Ok(Some(match ck {
            CastKind::LValueToRValue => return self.emit_load_cast(e, q, s),
            // clang `ScalarExprEmitter::VisitCastExpr`: a user-defined or
            // constructor conversion is its (call) operand.
            // A vector bitcast reinterprets the value (clang `CK_BitCast`
            // `CreateBitCast`).
            CastKind::BitCast if self.vec_of(q).is_some() || self.vec_of(sq).is_some() => {
                let v = self.emit_scalar(s)?;
                let t = self.lty(q);
                self.vector_bitcast(v, t)
            }
            // clang `CK_VectorSplat`: `CreateVectorSplat`.
            CastKind::VectorSplat => {
                let v = self.emit_scalar(s)?;
                let t = self.lty(q);
                self.splat(v, t)
            }
            // A glvalue `NoOp` or `LValueBitCast` (a cast to a reference,
            // [expr.static.cast]/4, [expr.reinterpret.cast]/11) is its
            // operand's object as the cast's type, loaded through the
            // cast's lvalue (clang `EmitLoadOfLValue(EmitCastLValue)`):
            // `reinterpret_cast<const float&>(u)` reads a `float`, and
            // `static_cast<U&&>(f())` the referee of a reference-returning
            // `f`, not the address `emit_scalar` yields for the call.
            CastKind::NoOp | CastKind::LValueBitCast
                if self.is_glvalue(e) && !ast.types.is_function(q) =>
            {
                // A `NoOp` visits its operand (`EmitCheckedLValue` +
                // `EmitLoadOfScalar` of the innermost non-`NoOp` glvalue,
                // so `return p;`'s implicit-move xvalue of a parameter is
                // a declaration reference, unchecked); an `LValueBitCast`
                // loads unchecked.
                if ck == CastKind::NoOp {
                    let mut inner = s;
                    while ast.exprs.get(inner).kind().is_cast()
                        && ast.exprs.cast_kind(inner) == CastKind::NoOp
                        && self.is_glvalue(inner)
                    {
                        inner = self.sub(inner);
                    }
                    if ast.exprs.get(inner).kind() == ExprKind::ArraySubscriptExpr {
                        self.f.ub_accessed = Some(inner);
                    }
                    let lv = self.emit_lvalue(e);
                    self.f.ub_accessed = None;
                    let lv = lv?;
                    self.ub_check_lv(inner, lv, Tck::Load)?;
                    self.load_lv_at(lv, self.ub_expr_loc(inner))?
                } else {
                    let lv = self.emit_lvalue(e)?;
                    self.load_lv_at(lv, self.ub_expr_loc(e))?
                }
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
            CastKind::DerivedToBase
            | CastKind::UncheckedDerivedToBase
            | CastKind::BaseToDerived => self.emit_class_ptr_cast(e, ck, q, sq, s)?,
            // `__builtin_bit_cast` ([bit.cast]): the operand's bytes
            // loaded as the destination type (clang `EmitLoadOfLValue` of
            // the reinterpreted address).
            CastKind::LValueToRValueBitCast => {
                let lv = self.emit_lvalue(s)?;
                let a = Addr { ty: self.mty(q), ..lv.addr };
                return Ok(Some(self.load_scalar_tb(a, q, Tb::MayAlias)));
            }
            CastKind::ToVoid => {
                self.emit_ignored(s)?;
                return Ok(None);
            }
            CastKind::Dynamic => {
                // clang `VisitCastExpr` `CK_Dynamic`: the operand with
                // `EmitPointerWithAlignment`'s alignment.
                let (p, align) = self.ptr_align(s)?;
                self.emit_dynamic_cast(e, p, align)?
            }
            CastKind::ArrayToPointerDecay => {
                let lv = self.emit_lvalue(s)?;
                self.decay(lv.addr, lv.q)
            }
            CastKind::FunctionToPointerDecay | CastKind::BuiltinFnToFnPtr => {
                self.emit_lvalue(s)?.addr.v
            }
            CastKind::NullToPointer => {
                self.visit_null_operand(s, sq)?;
                V::C(self.m.const_null(LTy::PTR))
            }
            CastKind::NullToMemberPointer
            | CastKind::BaseToDerivedMemberPointer
            | CastKind::DerivedToBaseMemberPointer
            | CastKind::MemberPointerToBoolean => self.emit_member_ptr_cast(e, ck, q, sq, s)?,
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
                if self.vec_of(sq).is_some() {
                    // clang 22: int → bool ext vector, lane-wise `icmp ne 0`.
                    self.convert_vector(v, sq, q)
                } else {
                    let loc = self.ub_expr_loc(e);
                    let r = self.conv_at(v, sq, q, loc)?;
                    // clang `VisitCastExpr` `CK_IntegralCast`: an implicit
                    // conversion not written as part of a cast is checked.
                    if ck == CastKind::IntegralCast
                        && ast.exprs.get(e).kind() == ExprKind::ImplicitCastExpr
                        && !ast.exprs.get(e).has(ExprFlags::PART_OF_EXPLICIT_CAST)
                    {
                        self.ub_int_conv(v, sq, r, q, loc)?;
                    }
                    r
                }
            }
            CastKind::FloatingComplexToReal
            | CastKind::IntegralComplexToReal
            | CastKind::FloatingComplexToBoolean
            | CastKind::IntegralComplexToBoolean => {
                self.emit_complex_to_scalar_cast(e, ck, q, sq, s)?
            }
            k => return err(self.loc_of(e), format!("cast {k:?} is not supported yet")),
        }))
    }

    /// `CK_LValueToRValue` in clang's `ScalarExprEmitter::VisitCastExpr`:
    /// a folded constant, the load of a reference-returning call, or the
    /// operand's scalar value.
    fn emit_load_cast(&mut self, e: ExprId, q: QualType, s: ExprId) -> R<Option<V>> {
        if let Some(c) = self.folds.get(e)
            && !self.is_aggregate(q)
            && !self.is_complex(q)
        {
            if self.dbg_on() {
                let mut s = s;
                loop {
                    match self.ast.exprs.get(s).kind() {
                        ExprKind::ParenExpr => s = self.ast.exprs.operand(s),
                        ExprKind::SubstNonTypeTemplateParmExpr => {
                            s = self.ast.exprs.subst_replacement(s);
                        }
                        _ => break,
                    }
                }
                if matches!(
                    self.ast.exprs.get(s).kind(),
                    ExprKind::DeclRefExpr | ExprKind::MemberExpr
                ) {
                    self.di_const_ref(self.ast.exprs.decl(s), Some(c));
                }
            }
            return Ok(Some(V::C(self.const_scalar_value(c, q)?)));
        }
        self.emit_glvalue_load(s)
    }

    /// The loaded scalar value of the glvalue `s`: a reference-returning
    /// call yields its referee's address from `emit_scalar`, so it is
    /// loaded through its lvalue (clang `ScalarExprEmitter::VisitCallExpr`),
    /// also beneath the glvalues clang's scalar visitor passes through to
    /// that call (WP-632); a glvalue `CK_LValueBitCast` is loaded as its
    /// own type (clang `VisitCastExpr` `CK_LValueBitCast`:
    /// `EmitLoadOfLValue`); anything else is its scalar value. Also each
    /// arm of a glvalue conditional, whose arms clang visits as scalars
    /// (WP-418).
    fn emit_glvalue_load(&mut self, s: ExprId) -> R<Option<V>> {
        // A call returning a reference yields the referee's address
        // (clang `ScalarExprEmitter::VisitCallExpr`: a reference
        // result is `EmitLoadOfLValue`), also through the lvalue
        // `CK_UserDefinedConversion` of a reference-returning conversion
        // function (`operator bool&()`), any implicit or explicit glvalue
        // `CK_NoOp` cast (`static_cast<const T&>(f())`, `(const T&)f()`,
        // `const_cast<T&>(f())`; `VisitCastExpr` visits the operand), the
        // right operand of a glvalue comma (`VisitBinComma`) and the
        // chosen operand of `__builtin_choose_expr` (`VisitChooseExpr`).
        // A glvalue `co_await`/`co_yield` is its `await_resume` call
        // (clang `emitSuspendExpression` visits the resume call), so it
        // is loaded the same way (WP-789).
        let ast = self.ast;
        let mut inner = self.skip_parens(s);
        let mut bit_cast = false;
        while self.is_glvalue(inner) {
            let k = ast.exprs.get(inner).kind();
            inner = if k.is_cast() {
                match ast.exprs.cast_kind(inner) {
                    CastKind::NoOp | CastKind::UserDefinedConversion => {}
                    CastKind::LValueBitCast => bit_cast = true,
                    _ => break,
                }
                ast.exprs.operand(inner)
            } else if k == ExprKind::BinaryOperator && ast.exprs.binary_op(inner) == BinaryOp::Comma
            {
                ast.exprs.operands(inner)[1]
            } else if k == ExprKind::ChooseExpr
                && let Some(c) = ast.exprs.choose_chosen(inner)
            {
                c
            } else {
                break;
            };
            inner = self.skip_parens(inner);
        }
        if bit_cast
            || matches!(
                self.ast.exprs.get(inner).kind(),
                ExprKind::CallExpr
                    | ExprKind::CXXMemberCallExpr
                    | ExprKind::CXXOperatorCallExpr
                    | ExprKind::CoawaitExpr
                    | ExprKind::CoyieldExpr
            ) && self.is_glvalue(inner)
        {
            let lv = self.emit_lvalue(s)?;
            if !bit_cast {
                self.ub_check_lv(inner, lv, Tck::Load)?;
            }
            return Ok(Some(self.load_lv_at(lv, self.ub_expr_loc(inner))?));
        }
        self.emit_scalar_opt(s)
    }

    /// The class pointer casts `CK_DerivedToBase`, `CK_UncheckedDerivedToBase`
    /// (clang `GetAddressOfBaseClass`) and `CK_BaseToDerived`
    /// (`GetAddressOfDerivedClass`), null-checked unless unchecked or the
    /// operand is `this` (`ShouldNullCheckClassCastValue`).
    fn emit_class_ptr_cast(
        &mut self,
        e: ExprId,
        ck: CastKind,
        q: QualType,
        sq: QualType,
        s: ExprId,
    ) -> R<V> {
        let ast = self.ast;
        if ck == CastKind::BaseToDerived {
            let p = self.emit_scalar(s)?;
            let Some(derived) = ast.types.pointee(q).and_then(|t| ast.types.as_record(t)) else {
                return err(self.loc_of(e), "base-to-derived cast of a non-class pointer");
            };
            let off = self.derived_cast_offset(e, derived)?;
            let null_check = !self.is_this_expr(s);
            let v = self.derived_ptr(p, off, null_check);
            // [expr.static.cast]/11: the object is of the derived type.
            if self.ub_type_check_on() {
                let loc = self.ub_expr_loc(e);
                let dq = ast.types.pointee(q).expect("Sema: pointer type");
                self.ub_type_check(v, TypeCheck::new(Tck::DowncastPointer, loc, dq))?;
            }
            return Ok(v);
        }
        if ast.types.pointee(sq).and_then(|t| ast.types.as_record(t)).is_none() {
            return err(self.loc_of(e), "derived-to-base cast of a non-class pointer");
        }
        // clang `VisitCastExpr` `CK_DerivedToBase`: the whole cast through
        // `EmitPointerWithAlignment` (`GetAddressOfBaseClass`), so a
        // virtual step loads the vtable pointer at the operand's known
        // alignment.
        Ok(self.pointer_with_alignment(e)?.v)
    }

    /// clang `MustVisitNullValue`: a `nullptr_t` operand of a null pointer
    /// conversion is evaluated for its side effects.
    fn visit_null_operand(&mut self, s: ExprId, sq: QualType) -> R<()> {
        if matches!(
            self.ast.types.canonical_view(sq),
            kiln_ast::types::TypeView::Builtin(kiln_ast::types::BuiltinType::NullPtr)
        ) {
            self.emit_ignored(s)?;
        }
        Ok(())
    }

    /// The member pointer casts: `CK_NullToMemberPointer`, the base/derived
    /// conversions (clang `EmitMemberPointerConversion`) and
    /// `CK_MemberPointerToBoolean`.
    fn emit_member_ptr_cast(
        &mut self,
        e: ExprId,
        ck: CastKind,
        q: QualType,
        sq: QualType,
        s: ExprId,
    ) -> R<V> {
        if ck == CastKind::NullToMemberPointer {
            self.visit_null_operand(s, sq)?;
            return Ok(self.null_mp(q));
        }
        let v = self.emit_scalar(s)?;
        if ck == CastKind::MemberPointerToBoolean {
            Ok(self.mp_is_not_null(v, sq))
        } else {
            self.mp_convert(e, v)
        }
    }

    /// clang `ScalarExprEmitter::VisitCastExpr` for a complex operand:
    /// `CK_*ComplexToReal` takes the real part only, `CK_*ComplexToBoolean`
    /// tests both parts against zero.
    fn emit_complex_to_scalar_cast(
        &mut self,
        e: ExprId,
        ck: CastKind,
        q: QualType,
        sq: QualType,
        s: ExprId,
    ) -> R<V> {
        if matches!(ck, CastKind::FloatingComplexToReal | CastKind::IntegralComplexToReal) {
            return match self.emit_cx(s, (false, true))?.0 {
                Some(v) => Ok(v),
                None => err(self.loc_of(e), "a complex value without a real part"),
            };
        }
        let v = self.emit_complex(s)?;
        self.cx_to_scalar(v, sq, q, self.ub_expr_loc(e))
    }

    /// `gep inbounds [N x T], p, i64 0, i64 0`.
    pub(crate) fn decay(&mut self, a: Addr, q: QualType) -> V {
        // clang `EmitArrayToPointerDecay`: a variable length array's
        // address already points to its first element.
        if self.is_vla(q) {
            return a.v;
        }
        let z = self.i64c(0);
        self.gep(a.ty, a.v, vec![z, z], true)
    }

    fn emit_unary(&mut self, e: ExprId) -> R<V> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        let s = ast.exprs.operand(e);
        match ast.exprs.unary_op(e) {
            UnaryOp::Minus | UnaryOp::Plus if self.f16_excess(q) => {
                let v = self.emit_promoted(e)?;
                let t = self.lty(q);
                Ok(self.cast(CastOp::FPTrunc, v, t))
            }
            UnaryOp::Plus | UnaryOp::Extension => self.emit_scalar(s),
            UnaryOp::Minus => {
                let v = self.emit_scalar(s)?;
                let t = self.f.fir.ty(&self.m, v);
                if self.is_float_ty(t) {
                    return Ok(self.fneg(v));
                }
                if let Some(r) = self.ub_negate(e, v, q)? {
                    return Ok(r);
                }
                let z = self.icn(t, 0);
                let fl = self.ub_nsw(q, self.is_signed(q));
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
                let lv = self.emit_checked_lvalue(e, Tck::Load)?;
                self.load_lv_at(lv, self.ub_expr_loc(e))
            }
            op @ (UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec) => {
                let lv = self.emit_lvalue(s)?;
                self.inc_dec(e, lv, op)
            }
            // clang `VisitUnaryReal`/`VisitUnaryImag`: a part of a complex
            // operand; `__real__` of a real operand is the operand and
            // `__imag__` zero (after the operand's side effects).
            op @ (UnaryOp::Real | UnaryOp::Imag) => {
                let imag = op == UnaryOp::Imag;
                if let Some(v) = self.emit_real_imag(e, imag)? {
                    return Ok(v);
                }
                if !imag {
                    return self.emit_scalar(s);
                }
                if self.is_glvalue(s) {
                    self.emit_lvalue(s)?;
                } else {
                    self.emit_scalar(s)?;
                }
                let t = self.lty(q);
                Ok(V::C(self.m.const_zero(t)))
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
        let old = self.load_lv_at(lv, self.ub_expr_loc(e))?;
        let promoted = match lv.bf {
            Some(_) => self.ub_inc_dec_promoted(e, lv.q, old, inc, true)?,
            None => None,
        };
        let (new, src, sq) = match promoted {
            Some((r, v, pq)) if self.ub_on(San::IMPLICIT_BITFIELD_CONVERSION) => (r, v, pq),
            Some((r, ..)) => (r, r, self.ast.exprs.get(e).ty()),
            None => {
                let r = self.inc_dec_value(e, lv.q, old, inc, lv.bf.is_some())?;
                (r, r, self.ast.exprs.get(e).ty())
            }
        };
        let stored = self.store_lv(new, lv, lv.bf.is_some());
        if let Some(bf) = lv.bf {
            let to = self.ast.exprs.get(e).ty();
            self.ub_bf_conv(src, sq, stored, to, bf.size, self.ub_expr_loc(e))?;
        }
        Ok(if pre { stored } else { old })
    }

    /// The incremented (`inc`) or decremented value `old` of type `q`.
    /// `bf`: the operand is a bit-field.
    pub(crate) fn inc_dec_value(
        &mut self,
        e: ExprId,
        q: QualType,
        old: V,
        inc: bool,
        bf: bool,
    ) -> R<V> {
        let ast = self.ast;
        let amount: i128 = if inc { 1 } else { -1 };
        Ok(if self.is_bool(q) && inc {
            V::C(self.m.const_bool(true))
        } else if ast.types.is_pointer(q) {
            let pq = ast.types.pointee(q).expect("Sema: pointer type");
            if let Some((n, et)) = self.vla_inc_step(pq, inc)? {
                let loc = self.ub_expr_loc(e);
                return self.gep_checked(et, old, vec![n], inc, false, !inc, loc);
            }
            let et = if ast.types.is_void(pq) || ast.types.is_function(pq) {
                LTy::I8
            } else {
                self.mty(pq)
            };
            let c = self.icn(LTy::I32, amount);
            let loc = self.ub_expr_loc(e);
            self.gep_checked(et, old, vec![c], inc, false, !inc, loc)?
        } else {
            let t = self.f.fir.ty(&self.m, old);
            if self.is_float_ty(t) && self.is_fp16(q) {
                // clang: non-native `__fp16` increments in float.
                let w = self.cast(CastOp::FPExt, old, LTy::FLOAT);
                let c = self.fconst(LTy::FLOAT, if inc { 1.0 } else { -1.0 });
                let r = self.bin(BinOp::FAdd, Flags::NONE, w, c);
                self.cast(CastOp::FPTrunc, r, t)
            } else if self.is_float_ty(t) {
                let c = self.fconst(t, if inc { 1.0 } else { -1.0 });
                self.bin(BinOp::FAdd, Flags::NONE, old, c)
            } else {
                if let Some((r, ..)) = self.ub_inc_dec_promoted(e, q, old, inc, bf)? {
                    return Ok(r);
                }
                if let Some(r) = self.ub_inc_dec(e, old, inc)? {
                    return Ok(r);
                }
                let c = self.icn(t, amount);
                let can_overflow = !ast.exprs.get(e).has(ExprFlags::CANNOT_OVERFLOW);
                let fl = self.ub_nsw(q, can_overflow && self.is_signed(q));
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
                let q = ast.exprs.get(e).ty();
                if self.is_aggregate(q) || self.is_atomic_aggregate(q) {
                    let lv = self.emit_lvalue(e)?;
                    return Ok(Some(lv.addr.v));
                }
                let (v, prev) = self.ub_bf_rhs(l, r)?;
                let lv = self.emit_checked_lvalue(l, Tck::Store)?;
                let res = self.store_lv(v, lv, true);
                self.ub_bf_assign(e, lv, v, prev, res)?;
                if self.is_volatile(lv.q) { self.load_lv(lv) } else { res }
            }
            BinaryOp::Comma => {
                self.emit_ignored(l)?;
                self.ensure_insert_point();
                return self.emit_scalar_opt(r);
            }
            BinaryOp::LAnd | BinaryOp::LOr => self.emit_logical(e, op == BinaryOp::LAnd)?,
            BinaryOp::PtrMemD | BinaryOp::PtrMemI => {
                let lv = self.emit_checked_lvalue(e, Tck::Load)?;
                self.load_lv_at(lv, self.ub_expr_loc(e))?
            }
            BinaryOp::EQ | BinaryOp::NE
                if self.is_complex(ast.exprs.get(l).ty())
                    || self.is_complex(ast.exprs.get(r).ty()) =>
            {
                self.cx_compare(e)?
            }
            BinaryOp::EQ | BinaryOp::NE if self.is_mp(ast.exprs.get(l).ty()) => {
                let lv = self.emit_scalar(l)?;
                let rv = self.emit_scalar(r)?;
                let lq = ast.exprs.get(l).ty();
                self.mp_compare(lv, rv, lq, op == BinaryOp::NE)
            }
            _ if self.f16_excess(ast.exprs.get(e).ty()) && is_promotable_op(op) => {
                let v = self.emit_promoted(e)?;
                let t = self.lty(ast.exprs.get(e).ty());
                self.cast(CastOp::FPTrunc, v, t)
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

    /// Whether `q` is the storage-only `__fp16` (clang `isHalfType`; kiln
    /// never enables a native half type, so it converts through float).
    pub(crate) fn is_fp16(&self, q: QualType) -> bool {
        self.ast.types.builtin(self.ast.types.canonical(q))
            == Some(kiln_ast::types::BuiltinType::Half)
    }

    /// clang `TargetInfo::shouldEmitFloat16WithExcessPrecision` and
    /// `ScalarExprEmitter::getPromotionType`: on x86-64 `_Float16`
    /// arithmetic is evaluated in `float`
    /// (`-ffloat16-excess-precision=standard`); an aarch64 target
    /// operates on `half`. `__fp16` is promoted by sema. `__bf16`
    /// arithmetic is evaluated in `float` on both targets (clang 19/22
    /// `shouldEmitBFloat16WithExcessPrecision`,
    /// `-fbfloat16-excess-precision=standard`; spec/codegen.md).
    pub(crate) fn f16_excess(&self, q: QualType) -> bool {
        match self.ast.types.builtin(self.ast.types.canonical(q)) {
            Some(kiln_ast::types::BuiltinType::Float16) => self.is_x86(),
            Some(kiln_ast::types::BuiltinType::BFloat16) => true,
            _ => false,
        }
    }

    /// clang `ScalarExprEmitter::EmitPromoted` to `float`: `+ - * /` and
    /// unary `-`/`+` of an excess-precision `_Float16` keep their operands
    /// in `float`; anything else is emitted and extended.
    pub(crate) fn emit_promoted(&mut self, e: ExprId) -> R<V> {
        let ast = self.ast;
        let e = self.skip_parens(e);
        let n = *ast.exprs.get(e);
        if self.f16_excess(n.ty()) {
            match n.kind() {
                ExprKind::BinaryOperator if is_promotable_op(ast.exprs.binary_op(e)) => {
                    let [l, r] = ast.exprs.operands(e);
                    let lv = self.emit_promoted(l)?;
                    let rv = self.emit_promoted(r)?;
                    let f = QualType::FLOAT;
                    return self.arith(e, ast.exprs.binary_op(e), lv, rv, f, f, f);
                }
                ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::Minus => {
                    let v = self.emit_promoted(ast.exprs.operand(e))?;
                    return Ok(self.fneg(v));
                }
                ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::Plus => {
                    return self.emit_promoted(ast.exprs.operand(e));
                }
                _ => {}
            }
        }
        let v = self.emit_scalar(e)?;
        Ok(self.cast(CastOp::FPExt, v, LTy::FLOAT))
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
        let nsw = self.ub_nsw(q, self.is_signed(q));
        Ok(match op {
            BinaryOp::Add | BinaryOp::Sub if lp && rp => {
                let a = self.cast(CastOp::PtrToInt, l, LTy::I64);
                let b = self.cast(CastOp::PtrToInt, r, LTy::I64);
                let d = self.bin(BinOp::Sub, Flags::NONE, a, b);
                let pq = ast.types.pointee(lq).expect("Sema: pointer type");
                if self.is_vla(pq) {
                    let c = self.vla_bytes(pq)?;
                    return Ok(self.bin(BinOp::SDiv, Flags::EXACT, d, c));
                }
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
                if let Some((le, re)) = self.ub_operands(e) {
                    let (pe, ie) = if lp || sub { (le, re) } else { (re, le) };
                    self.ub_bounds(e, pe, idx, ast.exprs.get(ie).ty(), false)?;
                }
                let et = ast.types.pointee(pq).expect("Sema: pointer type");
                if let Some((idx, ety)) = self.vla_pointer_step(et, idx)? {
                    let loc = self.ub_expr_loc(e);
                    return self.gep_checked(ety, p, vec![idx], !is && !sub, is, sub, loc);
                }
                let ety = if ast.types.is_void(et) || ast.types.is_function(et) {
                    LTy::I8
                } else {
                    self.mty(et)
                };
                let loc = self.ub_expr_loc(e);
                self.gep_checked(ety, p, vec![idx], !is && !sub, is, sub, loc)?
            }
            BinaryOp::Add => {
                if float {
                    match self.try_fmuladd(l, r, false) {
                        Some(v) => v,
                        None => self.bin(BinOp::FAdd, Flags::NONE, l, r),
                    }
                } else if let Some(v) = self.ub_binary_overflow(e, op, l, r, q)? {
                    v
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
                } else if let Some(v) = self.ub_binary_overflow(e, op, l, r, q)? {
                    v
                } else {
                    self.bin(BinOp::Sub, nsw, l, r)
                }
            }
            BinaryOp::Mul => {
                if float {
                    self.bin(BinOp::FMul, Flags::NONE, l, r)
                } else if let Some(v) = self.ub_binary_overflow(e, op, l, r, q)? {
                    v
                } else {
                    self.bin(BinOp::Mul, nsw, l, r)
                }
            }
            BinaryOp::Div => {
                if float {
                    self.bin(BinOp::FDiv, Flags::NONE, l, r)
                } else {
                    self.ub_divrem(e, op, l, r, q)?;
                    self.bin(if signed { BinOp::SDiv } else { BinOp::UDiv }, Flags::NONE, l, r)
                }
            }
            BinaryOp::Rem => {
                if float {
                    self.bin(BinOp::FRem, Flags::NONE, l, r)
                } else {
                    self.ub_divrem(e, op, l, r, q)?;
                    self.bin(if signed { BinOp::SRem } else { BinOp::URem }, Flags::NONE, l, r)
                }
            }
            BinaryOp::Shl | BinaryOp::Shr => {
                let rt = self.f.fir.ty(&self.m, r);
                let rp = if rt == lt { r } else { self.int_cast(r, lt, false) };
                if !self.f.san.is_empty() {
                    self.ub_shift(e, op == BinaryOp::Shl, l, r, rp, q, rq)?;
                }
                let r = rp;
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
        if !self.contract_on() {
            return None;
        }
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
            return Some(self.build_fmuladd(rm, l, sub ^ nr.is_some(), false));
        }
        None
    }

    fn build_fmuladd(&mut self, mul: V, addend: V, neg_mul: bool, neg_add: bool) -> V {
        let mi = Fir::last_inst(mul);
        let Op::Bin { l: a, r: b, .. } = *self.f.fir.op(mi) else {
            unreachable!("try_fmuladd checked: an fmul")
        };
        let a = if neg_mul { self.fneg(a) } else { a };
        let addend = if neg_add { self.fneg(addend) } else { addend };
        let ty = self.f.fir.ty(&self.m, addend);
        let name = kiln_llvm::intrinsics::overloaded_name(&self.m, "llvm.fmuladd", &[ty]);
        let fn_ty = self.m.types.function(ty, &[ty, ty, ty], false);
        let f =
            kiln_llvm::intrinsics::intrinsic(&mut self.m, &name, fn_ty).expect("known intrinsic");
        let mut attrs = vec![
            Attr::Flag(AttrKind::NoCallback),
            Attr::Flag(AttrKind::NoFree),
            Attr::Flag(AttrKind::NoSync),
            Attr::Flag(AttrKind::NoUnwind),
            Attr::Flag(AttrKind::Speculatable),
            Attr::Flag(AttrKind::WillReturn),
            Attr::Memory(kiln_llvm::MemoryEffects::all(kiln_llvm::ModRef::None)),
        ];
        if kiln_llvm::intrinsics::creates_no_undef_or_poison(&self.m, &name) {
            attrs.push(Attr::Flag(AttrKind::NoCreateUndefOrPoison));
        }
        let fa = self.m.attr_set(&attrs);
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
        let q = ast.exprs.get(e).ty();
        let rt = self.lty(q);
        // clang `VisitBinLAnd`/`VisitBinLOr` on vectors: both operands
        // compared with zero, lane-wise `and`/`or`, sign-extended.
        if self.vec_of(q).is_some() {
            self.prof_inc(Key::Expr(e));
            let x = self.emit_scalar(l)?;
            let y = self.emit_scalar(r)?;
            let t = self.f.fir.ty(&self.m, x);
            let zero = V::C(self.m.const_zero(t));
            let (x, y) = if self.is_float_ty(t) {
                (self.fcmp(FloatPredicate::Une, x, zero), self.fcmp(FloatPredicate::Une, y, zero))
            } else {
                (self.icmp(IntPredicate::Ne, x, zero), self.icmp(IntPredicate::Ne, y, zero))
            };
            let op = if and { BinOp::And } else { BinOp::Or };
            let v = self.bin(op, Flags::NONE, x, y);
            return Ok(self.cast(CastOp::SExt, v, rt));
        }
        if let Some(b) = self.fold_bool(l) {
            if b == and {
                self.prof_inc(Key::Expr(e));
                let v = self.eval_bool(r)?;
                if self.instr_regions() && pgo::is_instrumented_condition(ast, r) {
                    let fb = self.f.fir.create_block();
                    let cnt = self.f.fir.create_block();
                    if and {
                        self.cond_br(v, cnt, fb);
                    } else {
                        self.cond_br(v, fb, cnt);
                    }
                    self.emit_block(cnt, false);
                    self.prof_inc(Key::Expr(r));
                    self.emit_branch(fb);
                    self.emit_block(fb, false);
                }
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
        self.prof_inc(Key::Expr(e));
        let v = self.cond_eval(start, |cg| cg.eval_bool(r))?;
        let rb = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
        if self.instr_regions() && pgo::is_instrumented_condition(ast, r) {
            let cnt = self.f.fir.create_block();
            if and {
                self.cond_br(v, cnt, end);
            } else {
                self.cond_br(v, end, cnt);
            }
            self.emit_block(cnt, false);
            self.prof_inc(Key::Expr(r));
            self.emit_branch(end);
            self.f.fir.add_incoming(Fir::last_inst(phi), v, cnt);
        }
        // clang `VisitBinLAnd`: the fall-through branch into the merge
        // block has no line (gcov would count the operator's line again per
        // right-hand evaluation) and the phi an artificial line-0 location.
        // `VisitBinLOr` keeps the branch's location and leaves the phi
        // (created outside the builder) without one.
        let saved = if and { self.dbg_apply_empty() } else { None };
        self.emit_block(end, false);
        self.dbg_restore(saved);
        self.f.fir.add_incoming(Fir::last_inst(phi), v, rb);
        if self.dbg_on() {
            let l = match self.f.fir.dbg.as_deref().map(crate::fir::FirDbg::scope) {
                Some(scope) if and => Some(self.di_location_at(0, 0, scope)),
                _ => None,
            };
            self.f.fir.set_inst_dbg(Fir::last_inst(phi), l);
        }
        Ok(self.zext_or_same(phi, rt))
    }

    /// Runs `f` as conditionally evaluated code (clang
    /// `ConditionalEvaluation::begin`/`end`); `start` is the block the
    /// condition was branched from.
    pub(crate) fn cond_eval<T>(
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
                    self.prof_inc(Key::Expr(e));
                    return self.branch_to_counter_block(r, and, t, f, None);
                }
                if self.fold_bool(r) == Some(and) {
                    return self.branch_to_counter_block(l, and, t, f, Some(e));
                }
                let mid = self.f.fir.create_block();
                let start = self.f.fir.cur();
                if and {
                    self.branch_on_bool(l, mid, f)?;
                } else {
                    self.branch_on_bool(l, t, mid)?;
                }
                self.emit_block(mid, false);
                self.prof_inc(Key::Expr(e));
                self.cond_eval(start, |cg| cg.branch_to_counter_block(r, and, t, f, None))
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
                self.prof_inc(Key::Expr(e));
                self.cond_eval(start, |cg| cg.branch_on_bool(a, t, f))?;
                self.emit_block(rb, false);
                self.cond_eval(start, |cg| cg.branch_on_bool(b, t, f))
            }
            // `br(c ? throw x : y, t, f)` folds to `br(c, throw x, br(y, t, f))`.
            ExprKind::CXXThrowExpr => self.emit_throw(e, false),
            _ => {
                let v = self.eval_bool(e)?;
                let br = self.cond_br(v, t, f);
                // A condition that is a call of `__builtin_unpredictable`
                // (implicit conversions stripped, not parentheses) marks
                // the branch `!unpredictable` when optimizing.
                let mut c = e;
                while matches!(
                    ast.exprs.get(c).kind(),
                    ExprKind::ImplicitCastExpr
                        | ExprKind::ExprWithCleanups
                        | ExprKind::ConstantExpr
                ) {
                    c = ast.exprs.operand(c);
                }
                self.mark_unpredictable(br, c);
                Ok(())
            }
        }
    }

    /// clang `EmitBranchToCounterBlock`: [`Self::branch_on_bool`] of the
    /// operand `c` of a `&&` (`and`) or `||`, through a block counting
    /// its true (false) outcomes when `InstrProf` counters are on (the
    /// counter of `c`, or of `cntr`).
    fn branch_to_counter_block(
        &mut self,
        c: ExprId,
        and: bool,
        t: crate::fir::Blk,
        f: crate::fir::Blk,
        cntr: Option<ExprId>,
    ) -> R<()> {
        if !self.instr_regions() || !pgo::is_instrumented_condition(self.ast, c) {
            return self.branch_on_bool(c, t, f);
        }
        let cnt = self.f.fir.create_block();
        let (then, els, next) = if and { (cnt, f, t) } else { (t, cnt, f) };
        self.branch_on_bool(c, then, els)?;
        self.emit_block(cnt, false);
        self.prof_inc(Key::Expr(cntr.unwrap_or(c)));
        self.emit_branch(next);
        Ok(())
    }

    /// clang `EmitBranchOnBoolExpr` / `EmitSwitchStmt`: when optimizing,
    /// a branch or `switch` (instruction `i`) whose condition `c` is a
    /// call of `__builtin_unpredictable` carries `!unpredictable !{}`.
    pub(crate) fn mark_unpredictable(&mut self, i: u32, c: ExprId) {
        if self.opts.optimize()
            && self.ast.exprs.get(c).kind() == ExprKind::CallExpr
            && crate::builtins::builtin_callee(self.ast, c)
                .is_some_and(|(_, b)| b == kiln_ast::decl::builtin::UNPREDICTABLE)
        {
            let empty = self.m.md_tuple(&[]);
            let k = self.m.md_kind("unpredictable");
            self.f.fir.add_md(i, k, empty);
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

    /// clang `emitPseudoObjectExpr`: evaluates the semantic expressions
    /// of `PseudoObjectExpr` `e` before its result in order, binding each
    /// `OpaqueValueExpr`'s source once for its uses and ignoring the
    /// rest; returns the result. [`Self::unbind_pseudo`] drops the
    /// bindings.
    pub(crate) fn bind_pseudo(&mut self, e: ExprId) -> R<ExprId> {
        let ast = self.ast;
        let result = ast.exprs.pseudo_result(e, &ast.lists);
        for s in ast.exprs.pseudo_semantics(e, &ast.lists) {
            if s == result {
                break;
            }
            match ast.exprs.opaque_source(s) {
                Some(src) if ast.exprs.get(s).kind() == ExprKind::OpaqueValueExpr => {
                    let v = if self.is_glvalue(src) {
                        Opaque::Lv(self.emit_lvalue(src)?)
                    } else {
                        Opaque::Rv(self.emit_scalar_opt(src)?)
                    };
                    self.f.opaque.insert(src, v);
                }
                _ => self.emit_ignored(s)?,
            }
        }
        Ok(result)
    }

    /// Drops the bindings [`Self::bind_pseudo`] made for `e`.
    pub(crate) fn unbind_pseudo(&mut self, e: ExprId) {
        let ast = self.ast;
        for s in ast.exprs.pseudo_semantics(e, &ast.lists) {
            if ast.exprs.get(s).kind() == ExprKind::OpaqueValueExpr
                && let Some(src) = ast.exprs.opaque_source(s)
            {
                self.f.opaque.remove(&src);
            }
        }
    }

    pub(crate) fn opaque_of(&self, e: ExprId) -> Option<Opaque> {
        // A handler's copy-initialization source has no source
        // expression: it is bound by itself.
        let src = self.ast.exprs.opaque_source(e).unwrap_or(e);
        self.f.opaque.get(&src).copied()
    }

    /// The scalar value of a conditional's arm: the arms of a glvalue
    /// conditional are glvalues and are loaded (`c ? f() : x` with a
    /// reference-returning `f`, WP-418).
    fn emit_arm(&mut self, a: ExprId) -> R<Option<V>> {
        if self.is_glvalue(a) { self.emit_glvalue_load(a) } else { self.emit_scalar_opt(a) }
    }

    /// clang `VisitAbstractConditionalOperator`.
    fn emit_conditional(&mut self, e: ExprId, c: ExprId, a: ExprId, b: ExprId) -> R<Option<V>> {
        if let kiln_ast::types::TypeView::Vector { kind, .. } =
            self.ast.types.canonical_view(self.ast.exprs.get(c).ty())
        {
            self.prof_inc(Key::Expr(e));
            return self.emit_vector_conditional(c, a, b, kind).map(Some);
        }
        // An SVE vector condition (clang `isSveVLSBuiltinType`) selects as
        // a GNU vector condition does (an `svbool_t` by its lanes).
        if self.ast.types.is_sizeless(self.ast.exprs.get(c).ty()) {
            self.prof_inc(Key::Expr(e));
            let kind = kiln_ast::types::VectorKind::Generic;
            return self.emit_vector_conditional(c, a, b, kind).map(Some);
        }
        if let Some(x) = self.fold_bool(c) {
            if x {
                self.prof_inc(Key::Expr(e));
            }
            return self.emit_arm(if x { a } else { b });
        }
        if self.folds.get(a).is_some() && self.folds.get(b).is_some() {
            let cv = self.eval_bool(c)?;
            let step = self.zext_or_same(cv, LTy::I64);
            self.prof_inc_step(Key::Expr(e), Some(step));
            let x = self.emit_arm(a)?;
            let y = self.emit_arm(b)?;
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
        self.prof_inc(Key::Expr(e));
        let x = self.cond_eval(start, |cg| cg.emit_arm(a))?;
        let lb = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
        self.br(cont);
        self.emit_block(rb, false);
        let mut y = self.cond_eval(start, |cg| cg.emit_arm(b))?;
        // A `bool` arm that is a post-increment of `_Atomic(bool)` is an
        // `i8` (`atomic_incdec`); clang 19 feeds it to a phi of the other
        // arm's type (invalid IR). Kiln converts the second arm to the
        // first's type, which stays the phi's type as in clang
        // (spec/codegen.md, WP-183).
        if let (Some(xv), Some(yv)) = (x, y) {
            let (xt, yt) = (self.f.fir.ty(&self.m, xv), self.f.fir.ty(&self.m, yv));
            if xt != yt && matches!((xt, yt), (LTy::I1, LTy::I8) | (LTy::I8, LTy::I1)) {
                let op = if xt == LTy::I1 { CastOp::Trunc } else { CastOp::ZExt };
                y = Some(self.cast(op, yv, xt));
            }
        }
        let rb = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
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

    /// clang `VisitAbstractConditionalOperator` with a vector condition:
    /// all three operands are evaluated; an ext vector condition selects
    /// by each lane's sign bit through masks (`(f & ~m) | (t & m)`, float
    /// lanes bit-cast to the condition's type), a GNU vector condition by
    /// lane `!= 0` and a bool ext vector condition by its lanes, both
    /// through a `select`. An SVE condition comes here as `Generic`.
    fn emit_vector_conditional(
        &mut self,
        c: ExprId,
        a: ExprId,
        b: ExprId,
        kind: kiln_ast::types::VectorKind,
    ) -> R<V> {
        let cv = self.emit_scalar(c)?;
        let x = self.emit_scalar(a)?;
        let y = self.emit_scalar(b)?;
        let ct = self.f.fir.ty(&self.m, cv);
        let zero = V::C(self.m.const_zero(ct));
        // A bool ext vector condition (clang 20+) or an `svbool_t` (clang
        // 22; clang 19 compares it `ne zeroinitializer` like any other
        // SVE condition) is its `<N x i1>` lanes.
        if self.m.types.vector_info(ct).is_some_and(|(et, _, _)| et == LTy::I1)
            && (kind == kiln_ast::types::VectorKind::Ext || self.opts.clang_compat >= 22)
        {
            return Ok(self.select(cv, x, y));
        }
        if kind != kiln_ast::types::VectorKind::Ext {
            let m = self.icmp(IntPredicate::Ne, cv, zero);
            return Ok(self.select(m, x, y));
        }
        let msb = self.icmp(IntPredicate::Slt, cv, zero);
        let mask = self.cast(CastOp::SExt, msb, ct);
        let ones = self.icn(ct, -1);
        let inv = self.bin(BinOp::Xor, Flags::NONE, mask, ones);
        let rt = self.f.fir.ty(&self.m, y);
        let float = self.is_float_ty(rt);
        let (xt, yt) = if float {
            let yt = self.bitcast_fold(y, ct);
            (self.bitcast_fold(x, ct), yt)
        } else {
            (x, y)
        };
        let lo = self.bin(BinOp::And, Flags::NONE, yt, inv);
        let hi = self.bin(BinOp::And, Flags::NONE, xt, mask);
        let r = self.bin(BinOp::Or, Flags::NONE, lo, hi);
        Ok(if float { self.cast(CastOp::BitCast, r, rt) } else { r })
    }

    /// A bitcast of `v` to the integer vector type `to`, folded (as by
    /// LLVM's `ConstantFolder`) when `v` is a constant float vector.
    fn bitcast_fold(&mut self, v: V, to: LTy) -> V {
        if let V::C(c) = v
            && let Some((et, _, _)) = self.m.types.vector_info(to)
        {
            match self.m.const_of(c) {
                Some(ConstData::Zero(_)) => return V::C(self.m.const_zero(to)),
                Some(ConstData::Vector { elems, .. }) => {
                    let bits: Option<Vec<u128>> = elems
                        .iter()
                        .map(|&x| match self.m.const_of(x) {
                            Some(ConstData::Float { bits, .. }) => Some(*bits),
                            _ => None,
                        })
                        .collect();
                    if let Some(bits) = bits {
                        let ints: Vec<Value> =
                            bits.iter().map(|&b| self.m.const_int(et, b as i128)).collect();
                        if let Ok(r) = self.m.const_vector(&ints) {
                            return V::C(r);
                        }
                    }
                }
                _ => {}
            }
        }
        self.cast(CastOp::BitCast, v, to)
    }

    /// clang `EmitCompoundAssignLValue`: the lvalue and the stored value.
    fn emit_compound_assign(&mut self, e: ExprId) -> R<(LV, V)> {
        let ast = self.ast;
        let ca = *ast.exprs.compound_assign(e);
        if self.is_complex(ca.result_ty) {
            // clang `EmitScalarCompoundAssignWithComplex` (and
            // `EmitComplexCompoundAssignmentLValue` for an lvalue).
            let (lv, (v, _)) = self.cx_compound_assign(e)?;
            let v = v.unwrap_or_else(|| V::C(self.m.const_zero(LTy::I1)));
            let v =
                if !self.is_complex(lv.q) && self.is_volatile(lv.q) { self.load_lv(lv) } else { v };
            return Ok((lv, v));
        }
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
        // clang `EmitCompoundAssignLValue`: an excess-precision operand or
        // computation is promoted to `float`.
        let (rv, rq) = if self.f16_excess(rq) {
            (self.emit_promoted(ca.rhs)?, QualType::FLOAT)
        } else {
            (self.emit_scalar(ca.rhs)?, rq)
        };
        let lv = self.emit_checked_lvalue(l, Tck::Store)?;
        let lq = lv.q;
        if lv.bf.is_none()
            && let Some(vq) = ast.types.as_atomic(lq)
        {
            let v = self.atomic_compound_assign(e, op, lv, vq, rv, rq, ca.lhs_ty, ca.result_ty)?;
            return Ok((lv, v));
        }
        let old = self.load_lv_at(lv, self.ub_expr_loc(e))?;
        let promote = |c: &Self, q: QualType| if c.f16_excess(q) { QualType::FLOAT } else { q };
        let (lhs_ty, result_ty) = (promote(self, ca.lhs_ty), promote(self, ca.result_ty));
        let lhs = if ast.types.is_pointer(lq) { old } else { self.conv(old, lq, lhs_ty) };
        let res = self.arith(e, op, lhs, rv, lhs_ty, rq, result_ty)?;
        let res = if ast.types.is_pointer(lq) {
            (res, None)
        } else {
            let loc = self.ub_expr_loc(e);
            let r = self.conv_at(res, result_ty, lq, loc)?;
            if lv.bf.is_none() {
                self.ub_int_conv(res, result_ty, r, ast.exprs.get(l).ty(), loc)?;
            }
            (r, Some(res))
        };
        let (res, prev) = res;
        let v = self.store_lv(res, lv, true);
        if let (Some(bf), Some(p)) = (lv.bf, prev) {
            let to = ast.exprs.get(l).ty();
            self.ub_bf_conv(p, rq, v, to, bf.size, self.ub_expr_loc(e))?;
        }
        Ok((lv, v))
    }

    // ---- lvalues -------------------------------------------------------

    /// The address of a variable or function named by a `DeclRefExpr`.
    fn decl_lvalue(&mut self, e: ExprId, d: DeclId, q: QualType) -> R<LV> {
        let ast = self.ast;
        match ast.decls.get(d).kind() {
            // An explicit object conversion function is named like a
            // free function ([over.match.funcs.general]/4).
            DeclKind::Function | DeclKind::CXXMethod | DeclKind::CXXConversion => {
                let f = self.get_fn(d)?;
                Ok(LV {
                    addr: Addr { v: V::C(Value::func(f)), ty: LTy::PTR, align: 1 },
                    q,
                    bf: None,
                    vidx: None,
                    tb: Tb::Type,
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
                if let Some(lv) = self.constant_ref_lvalue(e, d, q)? {
                    return Ok(lv);
                }
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
                        let mut v = V::C(gval(g));
                        if self.var_is_tls(d) {
                            if is_local(ast, d) {
                                v = self.threadlocal_address(g);
                            } else {
                                let (p, wrapped) = self.tls_var_addr(d, g)?;
                                if wrapped {
                                    // The wrapper returns the referee.
                                    return Ok(self.ref_lvalue(p, q));
                                }
                                v = p;
                            }
                        }
                        Addr { v, ty: LTy::PTR, align: self.global_align(dq) }
                    };
                    let p = self.load_reference(slot.v, slot.align, dq);
                    self.tbaa_mark(p, dq, Tb::Type);
                    return Ok(self.ref_lvalue(p, q));
                }
                if let Some(&a) = self.f.locals.get(&d) {
                    return Ok(LV { addr: a, q, bf: None, vidx: None, tb: Tb::Type });
                }
                let ty = self.mty(q);
                let st = ast.decls.var(d).map(|v| v.spec.storage());
                let g = if is_static_local(ast, d) {
                    self.static_local(d)?
                } else if is_local(ast, d) && st != Some(VarStorage::Extern) {
                    return err(self.loc_of(e), "reference to an unemitted local variable");
                } else {
                    self.get_gvar(d)?
                };
                let align = self.var_align(d);
                let mut v = V::C(gval(g));
                if self.var_is_tls(d) {
                    // [basic.stc.thread]: this thread's object.
                    v = if is_local(ast, d) {
                        self.threadlocal_address(g)
                    } else {
                        self.tls_var_addr(d, g)?.0
                    };
                }
                Ok(LV { addr: Addr { v, ty, align }, q, bf: None, vidx: None, tb: Tb::Type })
            }
            // A postcondition's result name ([dcl.contract.res]): the
            // return slot, bound by `emit_postconditions`.
            DeclKind::ContractResultName => {
                // Named in a lambda of the postcondition that captured it
                // ([basic.pre]/9: a local entity).
                if ast.exprs.get(e).has(ExprFlags::REFERS_TO_ENCLOSING)
                    && let Some(lv) = self.captured_var_lvalue(d, q)?
                {
                    return Ok(lv);
                }
                let Some(&slot) = self.f.locals.get(&d) else {
                    return err(self.loc_of(e), "result name outside a postcondition");
                };
                let dq = ast.decls.get(d).ty().unwrap_or(q);
                if self.referee(dq).is_some() {
                    let p = self.load_reference(slot.v, slot.align, dq);
                    return Ok(self.ref_lvalue(p, q));
                }
                Ok(LV { addr: slot, q, bf: None, vidx: None, tb: Tb::Type })
            }
            DeclKind::TemplateParamObject => {
                let g = self.param_object_global(d)?;
                let dq = ast.decls.get(d).ty().unwrap_or(q);
                let ty = self.mty(dq);
                let align = self.global_align(dq);
                Ok(LV {
                    addr: Addr { v: V::C(gval(g)), ty, align },
                    q,
                    bf: None,
                    vidx: None,
                    tb: Tb::Type,
                })
            }
            k => err(self.loc_of(e), format!("reference to {k:?} is not supported yet")),
        }
    }

    /// The folded constant initializer of the scalar, non-reference
    /// variable `e` names without odr-using it (sema's
    /// `non_odr_use_constant`); `None` otherwise.
    fn non_odr_use_constant(&self, e: ExprId) -> Option<kiln_ast::ConstValueId> {
        let ast = self.ast;
        if ast.exprs.non_odr_use(e) != NonOdrUse::Constant {
            return None;
        }
        let q = ast.exprs.get(e).ty();
        if self.is_aggregate(q) || self.is_complex(q) || self.is_volatile(q) {
            return None;
        }
        let d = ast.exprs.decl(e);
        let dq = ast.decls.get(d).ty()?;
        if self.referee(dq).is_some() {
            return None;
        }
        let init = std::iter::once(d)
            .chain(ast.redecls_of(d))
            .find_map(|x| ast.decls.var(x).and_then(|v| v.init))?;
        self.folds.get(init)
    }

    /// clang `EmitDeclRefLValue` on a reference variable named without
    /// being odr-used ([basic.def.odr]/5: it is usable in constant
    /// expressions, sema's `non_odr_use_constant`): the address its
    /// constant initializer designates, read through without loading the
    /// reference (WP-231). `None` when the initializer did not fold.
    fn constant_ref_lvalue(&mut self, e: ExprId, d: DeclId, q: QualType) -> R<Option<LV>> {
        let ast = self.ast;
        if ast.exprs.non_odr_use(e) != NonOdrUse::Constant {
            return Ok(None);
        }
        let Some(dq) = ast.decls.get(d).ty() else { return Ok(None) };
        if self.referee(dq).is_none() {
            return Ok(None);
        }
        let Some(init) = std::iter::once(d)
            .chain(ast.redecls_of(d))
            .find_map(|x| ast.decls.var(x).and_then(|v| v.init))
        else {
            return Ok(None);
        };
        let Some(c) = self.folds.get(init) else { return Ok(None) };
        let k = self.const_value(c, dq, Some(init), true)?;
        Ok(Some(self.ref_lvalue(V::C(k), q)))
    }

    /// The global holding a namespace-scope or static reference variable.
    fn ref_var_global(&mut self, e: ExprId, d: DeclId) -> R<kiln_llvm::GlobalId> {
        let ast = self.ast;
        let st = ast.decls.var(d).map(|v| v.spec.storage());
        if is_static_local(ast, d) {
            self.static_local(d)
        } else if is_local(ast, d) && st != Some(VarStorage::Extern) {
            err(self.loc_of(e), "reference to an unemitted local variable")
        } else {
            self.get_gvar(d)
        }
    }

    /// clang `EmitLoadOfReference`: the address a reference of type `rq`
    /// stored at `slot` is bound to. At `-fclang-compat=22` (clang 20+) the
    /// load carries `!nonnull !{}` and, for an object referee whose natural
    /// alignment (as a pointee: `getNaturalTypeAlignment`, a non-final
    /// class's non-virtual alignment) exceeds 1, `!align !{i64 A}`; neither
    /// when the referee is incomplete (spec/codegen.md §3, "Reference
    /// loads").
    pub(crate) fn load_reference(&mut self, slot: V, align: u64, rq: QualType) -> V {
        let p = self.load(LTy::PTR, slot, align, false);
        if self.opts.clang_compat < 22 {
            return p;
        }
        let Some(referee) = self.referee(rq) else { return p };
        let types = &self.ast.types;
        if types.is_void(referee)
            || matches!(
                types.canonical_view(referee),
                kiln_ast::types::TypeView::IncompleteArray(_)
            )
            || !crate::builtins::is_complete(self.ast, referee)
        {
            return p;
        }
        let i = Fir::last_inst(p);
        if !self.opts.null_pointer_is_valid {
            let k = self.m.md_kind("nonnull");
            let empty = self.m.md_tuple(&[]);
            self.f.fir.add_md(i, k, empty);
        }
        if !self.ast.types.is_function(referee) {
            let (_, a) = self.pointee_layout(referee);
            if a > 1 {
                let c = self.m.const_int(LTy::I64, i128::from(a));
                let Ok(m) = self.m.md_const(c) else {
                    unreachable!("an integer is valid metadata")
                };
                let md = self.m.md_tuple(&[Some(m)]);
                let k = self.m.md_kind("align");
                self.f.fir.add_md(i, k, md);
            }
        }
        p
    }

    /// The lvalue of type `q` at the address `p` a reference holds.
    pub(crate) fn ref_lvalue(&mut self, p: V, q: QualType) -> LV {
        let ty = if self.ast.types.is_function(q) { LTy::PTR } else { self.mty(q) };
        let (_, align) = self.pointee_layout(q);
        LV { addr: Addr { v: p, ty, align }, q, bf: None, vidx: None, tb: Tb::Type }
    }

    /// clang `EmitLValue`.
    pub(crate) fn emit_lvalue(&mut self, e: ExprId) -> R<LV> {
        let dl = self.dbg_apply_expr(e);
        let r = self.emit_lvalue_x(e);
        self.dbg_restore(dl);
        r
    }

    fn emit_lvalue_x(&mut self, e: ExprId) -> R<LV> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        match n.kind() {
            ExprKind::ParenExpr => self.emit_lvalue(ast.exprs.operand(e)),
            ExprKind::ChooseExpr if ast.exprs.choose_chosen(e).is_some() => {
                self.emit_lvalue(ast.exprs.choose_chosen(e).expect("match guard checked it"))
            }
            ExprKind::CXXDefaultArgExpr => match default_arg_of(ast, e) {
                Some(d) => {
                    let saved = self.enter_default_arg(e);
                    let r = self.emit_lvalue(d);
                    self.srcloc = saved;
                    r
                }
                None => err(self.loc_of(e), "default argument not yet parsed"),
            },
            // clang `EmitLValue` of an `ExprWithCleanups`: the address
            // survives a shared cleanup block (`ForceCleanup({&V})`).
            ExprKind::ExprWithCleanups => {
                let sc = self.begin_scope(false);
                let mut r = self.emit_lvalue(self.sub(e));
                match &mut r {
                    Ok(lv) => self.end_scope_reload(sc, &mut [&mut lv.addr.v]),
                    Err(_) => self.end_scope(sc),
                }
                r
            }
            ExprKind::CXXDefaultInitExpr => match self.default_member_init(e) {
                Some(init) => {
                    let saved = self.enter_default_init(e);
                    let r = self.emit_lvalue(init);
                    self.exit_default_init(saved);
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
            ExprKind::ConstantExpr => match self.immediate_address(e)? {
                Some(p) if self.is_glvalue(e) => Ok(self.ref_lvalue(V::C(p), q)),
                _ => self.emit_lvalue(ast.exprs.operand(e)),
            },
            ExprKind::CXXTypeidExpr => {
                let p = self.emit_typeid(e)?;
                Ok(self.ref_lvalue(p, q))
            }
            ExprKind::SubstNonTypeTemplateParmExpr => self.emit_lvalue(ExprId::new(n.b())),
            ExprKind::DeclRefExpr => self.decl_lvalue(e, ast.exprs.decl(e), q),
            // clang `EmitPredefinedLValue`: the literal's global, named
            // `<identifier>.<current function>` when first emitted.
            ExprKind::PredefinedExpr => self.predefined_lvalue(e, q),
            ExprKind::StringLiteral => {
                let g = self.lit_global(ast.exprs.string_lit(e));
                let ty = self.m.global(g).value_type();
                let align = self.m.global(g).align.unwrap_or(1);
                Ok(LV {
                    addr: Addr { v: V::C(gval(g)), ty, align },
                    q,
                    bf: None,
                    vidx: None,
                    tb: Tb::Type,
                })
            }
            ExprKind::MemberExpr => self.member_lvalue(e),
            ExprKind::ExtVectorElementExpr => self.ext_vector_lvalue(e, q),
            ExprKind::ArraySubscriptExpr => self.subscript_lvalue(e),
            ExprKind::UnaryOperator => self.unary_lvalue(e, q),
            ExprKind::BinaryOperator => self.binary_lvalue(e, q),
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
            ExprKind::CoawaitExpr | ExprKind::CoyieldExpr => {
                match self.emit_coawait(e, crate::coro::Want::Lvalue)? {
                    crate::coro::Got::Lv(lv) => Ok(lv),
                    _ => err(self.loc_of(e), "co_await glvalue without a value"),
                }
            }
            ExprKind::OpaqueValueExpr => match self.opaque_of(e) {
                Some(Opaque::Lv(lv)) => Ok(lv),
                _ => err(self.loc_of(e), "unbound glvalue OpaqueValueExpr"),
            },
            ExprKind::CXXRewrittenBinaryOperator => {
                self.emit_lvalue(ast.exprs.rewritten_semantic(e))
            }
            ExprKind::PseudoObjectExpr => {
                let r = self.bind_pseudo(e)?;
                let lv = self.emit_lvalue(r);
                self.unbind_pseudo(e);
                lv
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr if self.is_glvalue(e) => {
                // A call returning a reference.
                let Some(p) = self.emit_call(e, None)? else {
                    return err(self.loc_of(e), "reference-returning call produced no value");
                };
                Ok(self.ref_lvalue(p, q))
            }
            // clang `EmitCompoundLiteralLValue` of a file-scope literal:
            // its `.compoundliteral` global (WP-466).
            ExprKind::CompoundLiteralExpr if let Some(d) = ast.exprs.compound_literal_var(e) => {
                let g = self.get_gvar(d)?;
                let (ty, align) = (self.mty(q), self.var_align(d));
                let addr = Addr { v: V::C(gval(g)), ty, align };
                Ok(LV { addr, q, bf: None, vidx: None, tb: Tb::Type })
            }
            ExprKind::CompoundLiteralExpr if !self.is_aggregate(q) => {
                self.compound_literal_lvalue(e)
            }
            ExprKind::MaterializeTemporaryExpr
            | ExprKind::CallExpr
            | ExprKind::CXXOperatorCallExpr
            | ExprKind::InitListExpr
            | ExprKind::CXXParenListInitExpr => self.temporary_lvalue(e),
            k if k.is_cast() => self.cast_lvalue(e, q),
            k => err(self.loc_of(e), format!("lvalue {k:?} is not supported yet")),
        }
    }

    /// clang `EmitPredefinedLValue`: the literal's global, named
    /// `<identifier>.<current function>` when first emitted.
    fn predefined_lvalue(&mut self, e: ExprId, q: QualType) -> R<LV> {
        let ast = self.ast;
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
        Ok(LV { addr: Addr { v: V::C(gval(g)), ty, align }, q, bf: None, vidx: None, tb: Tb::Type })
    }

    /// clang `EmitUnaryOpLValue`: `*p`, `++x`/`--x`, `__real`/`__imag`
    /// and `__extension__`.
    fn unary_lvalue(&mut self, e: ExprId, q: QualType) -> R<LV> {
        let ast = self.ast;
        match ast.exprs.unary_op(e) {
            UnaryOp::Deref => {
                // clang `EmitUnaryOpLValue`: the pointer's alignment from
                // `EmitPointerWithAlignment` (spec/codegen.md §3.3).
                let s = ast.exprs.operand(e);
                let (p, align) = if ast.types.is_pointer(ast.exprs.get(s).ty()) {
                    self.ptr_align(s)?
                } else {
                    (self.emit_scalar(s)?, self.natural_align(q))
                };
                let ty = if ast.types.is_function(q) { LTy::PTR } else { self.mty(q) };
                Ok(LV { addr: Addr { v: p, ty, align }, q, bf: None, vidx: None, tb: Tb::Type })
            }
            op @ (UnaryOp::PreInc | UnaryOp::PreDec) => {
                let lv = self.emit_lvalue(ast.exprs.operand(e))?;
                if self.is_complex(lv.q) {
                    self.cx_inc_dec(lv, op == UnaryOp::PreInc)?;
                } else {
                    self.inc_dec(e, lv, op)?;
                }
                Ok(lv)
            }
            op @ (UnaryOp::Real | UnaryOp::Imag) => self.real_imag_lvalue(e, op == UnaryOp::Imag),
            UnaryOp::Extension => self.emit_lvalue(ast.exprs.operand(e)),
            op => err(self.loc_of(e), format!("lvalue unary {op:?} is not supported yet")),
        }
    }

    /// clang `EmitBinaryOperatorLValue`: `.*`/`->*`, assignment (scalar,
    /// aggregate, `_Atomic` aggregate, complex) and the comma operator.
    fn binary_lvalue(&mut self, e: ExprId, q: QualType) -> R<LV> {
        let ast = self.ast;
        let [l, r] = ast.exprs.operands(e);
        match ast.exprs.binary_op(e) {
            BinaryOp::PtrMemD | BinaryOp::PtrMemI => self.mp_data_lvalue(e),
            BinaryOp::Assign => {
                if self.is_aggregate(q) {
                    let lv = self.emit_lvalue(l)?;
                    self.emit_agg(r, lv.addr, false)?;
                    return Ok(lv);
                }
                if self.is_atomic_aggregate(q) {
                    // clang `AggExprEmitter::VisitBinAssign` on an
                    // `_Atomic` left side (WP-165).
                    // The right side goes into a temporary of the
                    // assignment's type (`EmitAggExprToLValue`),
                    // which is the result.
                    let lv = self.emit_lvalue(l)?;
                    let t = self.temp(q);
                    self.f.slot_volatile = self.is_volatile(q);
                    self.emit_agg(r, t, false)?;
                    self.f.slot_volatile = false;
                    self.atomic_agg_store(t, lv);
                    return Ok(LV { addr: t, q, bf: None, vidx: None, tb: Tb::Type });
                }
                if self.is_complex(q) {
                    return Ok(self.cx_assign(e)?.0);
                }
                let (v, prev) = self.ub_bf_rhs(l, r)?;
                let lv = self.emit_checked_lvalue(l, Tck::Store)?;
                // clang computes the stored bit-field value only for the
                // implicit-bitfield-conversion check.
                let want = lv.bf.is_some() && self.ub_on(San::IMPLICIT_BITFIELD_CONVERSION);
                let res = self.store_lv(v, lv, want);
                self.ub_bf_assign(e, lv, v, prev, res)?;
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

    /// A materialized temporary, or a prvalue call / init list used as an
    /// lvalue (clang `EmitMaterializeTemporaryExpr`, `EmitAnyExprToMem`).
    fn temporary_lvalue(&mut self, e: ExprId) -> R<LV> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let s = if n.kind() == ExprKind::MaterializeTemporaryExpr { self.sub(e) } else { e };
        let sq = ast.exprs.get(s).ty();
        // [class.temporary]/6: a temporary bound to a reference
        // with static or thread storage duration is a `_ZGR` global.
        let static_tmp = n.kind() == ExprKind::MaterializeTemporaryExpr
            && matches!(
                ast.exprs.temporary_storage(e),
                StorageDuration::Static | StorageDuration::Thread
            );
        let (tmp, needs_init) =
            if static_tmp { self.static_temporary(e, sq)? } else { (self.temp(sq), true) };
        if !needs_init {
            return Ok(LV { addr: tmp, q: sq, bf: None, vidx: None, tb: Tb::Type });
        }
        if n.kind() == ExprKind::MaterializeTemporaryExpr && !static_tmp {
            self.temporary_lifetime(e, s, tmp);
        }
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
        } else if self.is_complex(sq) {
            let v = self.emit_complex(s)?;
            self.cx_store(v, tmp, sq);
        } else {
            let v = self.emit_scalar(s)?;
            self.store_scalar(v, tmp, sq);
        }
        if static_tmp {
            self.register_temporary_dtor(e, tmp)?;
        } else if n.kind() == ExprKind::MaterializeTemporaryExpr {
            self.push_temporary_cleanup(e, tmp);
        }
        Ok(LV { addr: tmp, q: sq, bf: None, vidx: None, tb: Tb::Type })
    }

    /// clang `EmitCompoundLiteralLValue` for a scalar, vector or complex
    /// literal in a function: its initializer stored into a fresh
    /// temporary (`EmitAnyExprToMem`; a C++ literal is not destroyed here).
    pub(crate) fn compound_literal_lvalue(&mut self, e: ExprId) -> R<LV> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let init = ast.exprs.compound_literal_init(e);
        let tmp = self.temp(q);
        if self.is_complex(q) {
            let v = self.emit_complex(init)?;
            self.cx_store(v, tmp, q);
        } else {
            let v = self.emit_scalar(init)?;
            self.store_scalar(v, tmp, q);
        }
        Ok(LV { addr: tmp, q, bf: None, vidx: None, tb: Tb::Type })
    }

    /// clang `EmitCastLValue`.
    fn cast_lvalue(&mut self, e: ExprId, q: QualType) -> R<LV> {
        let ast = self.ast;
        match ast.exprs.cast_kind(e) {
            CastKind::NoOp
            | CastKind::LValueBitCast
            | CastKind::LValueToRValue
            | CastKind::UserDefinedConversion => {
                let mut lv = self.emit_lvalue(ast.exprs.operand(e))?;
                if lv.bf.is_none() && lv.vidx.is_none() && !ast.types.is_function(q) {
                    lv.addr.ty = self.mty(q);
                }
                if ast.exprs.cast_kind(e) == CastKind::LValueBitCast {
                    lv.tb = lv.tb.sub();
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
                let loc = self.ub_expr_loc(e);
                let (v, align) = self.base_address(lv.addr, derived, vb, off, false, Some(loc))?;
                let ty = self.mty(q);
                Ok(LV { addr: Addr { v, ty, align }, q, bf: None, vidx: None, tb: lv.tb.sub() })
            }
            // clang `EmitCastLValue` `CK_BaseToDerived`: never null-checked.
            CastKind::BaseToDerived => {
                let lv = self.emit_lvalue(ast.exprs.operand(e))?;
                let Some(derived) = ast.types.as_record(q) else {
                    return err(self.loc_of(e), "base-to-derived cast of a non-class");
                };
                let off = self.derived_cast_offset(e, derived)?;
                let v = self.derived_ptr(lv.addr.v, off, false);
                // [expr.static.cast]/2: the object is of the derived type.
                let loc = self.ub_expr_loc(e);
                self.ub_type_check(v, TypeCheck::new(Tck::DowncastReference, loc, q))?;
                Ok(self.ref_lvalue(v, q))
            }
            CastKind::Dynamic => {
                let lv = self.emit_lvalue(ast.exprs.operand(e))?;
                let p = self.emit_dynamic_cast(e, lv.addr.v, lv.addr.align)?;
                Ok(self.ref_lvalue(p, q))
            }
            k => err(self.loc_of(e), format!("lvalue cast {k:?} is not supported yet")),
        }
    }

    fn cond_lvalue(&mut self, e: ExprId, c: ExprId, a: ExprId, b: ExprId) -> R<LV> {
        let ast = self.ast;
        if let Some(x) = self.fold_bool(c) {
            let (live, dead) = if x { (a, b) } else { (b, a) };
            if x {
                self.prof_inc(Key::Expr(e));
            }
            // A live throw-expression: emitted, with an undefined lvalue
            // (`HandleConditionalOperatorLValueSimpleCase`).
            if let Some(t) = self.throw_of(live) {
                self.emit_throw(t, true)?;
                let dq = ast.exprs.get(dead).ty();
                let ty = self.mty(dq);
                let v = V::C(self.m.const_undef(LTy::PTR));
                return Ok(LV {
                    addr: Addr { v, ty, align: 1 },
                    q: dq,
                    bf: None,
                    vidx: None,
                    tb: Tb::Type,
                });
            }
            return self.emit_lvalue(live);
        }
        let lb = self.f.fir.create_block();
        let rb = self.f.fir.create_block();
        let cont = self.f.fir.create_block();
        let start = self.f.fir.cur();
        self.branch_on_bool(c, lb, rb)?;
        self.emit_block(lb, false);
        self.prof_inc(Key::Expr(e));
        let x = self.cond_eval(start, |cg| cg.lvalue_or_throw(a))?;
        let lb = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
        if x.is_some() {
            self.br(cont);
        }
        self.emit_block(rb, false);
        let y = self.cond_eval(start, |cg| cg.lvalue_or_throw(b))?;
        let rb = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
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
        // clang `mergeTBAAInfoForConditionalOperator`.
        let tb = if x.tb == y.tb { x.tb } else { Tb::MayAlias };
        Ok(LV {
            addr: Addr { v: p, ty: x.addr.ty, align },
            q: ast.exprs.get(e).ty(),
            bf: None,
            vidx: None,
            tb,
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
        if matches!(ast.decls.get(field).kind(), DeclKind::Var | DeclKind::CXXMethod) {
            // clang `tryToConvertMemberExprToDeclRefExpr` and
            // `EmitMemberExpr`'s function case: a static data member or a
            // static member function named with `.`/`->` is its base,
            // evaluated and ignored ([expr.ref]/6.1, 6.3.1), and the
            // variable or function.
            self.emit_ignored(base)?;
            return self.decl_lvalue(e, field, q);
        }
        let (base_addr, btb) = if n.has(ExprFlags::ARROW) {
            // clang `EmitMemberExpr`: `EmitPointerWithAlignment` of the
            // base (`this` included: a class's pointer alignment).
            let pq = ast.types.pointee(ast.exprs.get(base).ty()).expect("Sema: pointer type");
            let (p, align) = self.ptr_align(base)?;
            if self.ub_type_check_on() {
                let skip = self.ub_member_skips(base);
                // Through `this`, clang's type is `getRecordType`'s.
                let desc =
                    if skip.has_any(San::ALIGNMENT) { UbDesc::Record } else { UbDesc::Written };
                let tc =
                    TypeCheck::new(Tck::MemberAccess, self.loc_of(e), pq).skip(skip).desc(desc);
                self.ub_type_check(p, tc)?;
            }
            let ty = self.mty(pq);
            (Addr { v: p, ty, align }, Tb::Type)
        } else {
            let lv = self.emit_checked_lvalue(base, Tck::MemberAccess)?;
            (lv.addr, lv.tb)
        };
        if ast.decls.get(field).kind() != DeclKind::Field {
            return err(self.loc_of(e), "member is not a field (M3)");
        }
        self.field_lvalue_tb(base_addr, btb, field, q)
    }

    /// clang `EmitLValueForField` on an object with the default access
    /// path.
    pub(crate) fn field_lvalue(&mut self, base: Addr, field: DeclId, q: QualType) -> R<LV> {
        self.field_lvalue_tb(base, Tb::Type, field, q)
    }

    /// clang `EmitLValueForField` on an object whose access path is `btb`.
    pub(crate) fn field_lvalue_tb(
        &mut self,
        base: Addr,
        btb: Tb,
        field: DeclId,
        q: QualType,
    ) -> R<LV> {
        let ast = self.ast;
        let rd = ast.decls.get(field).ctx().expect("Sema: a field is a member");
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
                v = self.field_gep(rty, base.v, i);
                align = min_align(base.align, self.elem_offset(rty, u64::from(i)));
            }
            let st = self.m.types.int(bf.storage_size);
            return Ok(LV {
                addr: Addr { v, ty: st, align },
                q,
                bf: Some(bf),
                vidx: None,
                tb: Tb::None,
            });
        }
        let fq = ast.decls.get(field).ty().unwrap_or(q);
        let tb = self.tb_field(btb, field, fq);
        if let Some(referee) = self.referee(fq) {
            // clang `EmitLValueForField` on a reference member: load the
            // bound address (tagged with the member's path).
            let slot = self.field_slot(base, field, fq)?;
            let p = self.load_reference(slot.addr.v, slot.addr.align, fq);
            self.tbaa_mark(p, fq, tb);
            let (_, align) = self.pointee_layout(referee);
            let ty = self.mty(referee);
            return Ok(LV {
                addr: Addr { v: p, ty, align },
                q: referee,
                bf: None,
                vidx: None,
                tb: Tb::Type,
            });
        }
        let mut lv = self.field_slot(base, field, q)?;
        lv.tb = tb;
        Ok(lv)
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
        self.field_lvalue_init_tb(base, Tb::Type, field, q)
    }

    /// `field_lvalue_init` on an object whose access path is `btb`: a
    /// reference member's slot is accessed as its own type (clang
    /// `getTBAAInfoForSubobject`); any other field, bit-fields included
    /// (a constructor's default member initializer `bool b : 1 {false};`,
    /// [class.base.init]/9), is `field_lvalue_tb`.
    pub(crate) fn field_lvalue_init_tb(
        &mut self,
        base: Addr,
        btb: Tb,
        field: DeclId,
        q: QualType,
    ) -> R<LV> {
        if self.referee(q).is_none() {
            return self.field_lvalue_tb(base, btb, field, q);
        }
        let mut lv = self.field_slot(base, field, q)?;
        lv.tb = btb.sub();
        Ok(lv)
    }

    /// The address of field `field` of the object at `base` (a reference
    /// member's own slot), with the default access path.
    fn field_slot(&mut self, base: Addr, field: DeclId, q: QualType) -> R<LV> {
        let ast = self.ast;
        let rd = ast.decls.get(field).ctx().expect("Sema: a field is a member");
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
                tb: Tb::Type,
            });
        }
        let Some(i) = idx else {
            // An empty field with no IR element (clang addresses it by
            // byte offset).
            let off = self.field_byte_offset(field)?;
            let v = if off == 0 { base.v } else { self.byte_offset(base.v, off) };
            let align = min_align(base.align, off);
            return Ok(LV { addr: Addr { v, ty, align }, q, bf: None, vidx: None, tb: Tb::Type });
        };
        let v = self.field_gep(rty, base.v, i);
        let align = min_align(base.align, self.elem_offset(rty, u64::from(i)));
        Ok(LV { addr: Addr { v, ty, align }, q, bf: None, vidx: None, tb: Tb::Type })
    }

    /// The array operand of a simple array-to-pointer decay.
    fn simple_decay_operand(&self, e: ExprId) -> Option<ExprId> {
        let ast = self.ast;
        let n = ast.exprs.get(e);
        if n.kind() == ExprKind::ImplicitCastExpr
            && ast.exprs.cast_kind(e) == CastKind::ArrayToPointerDecay
        {
            let a = ast.exprs.operand(e);
            // clang `isSimpleArrayDecayOperand`: not a variable length
            // array.
            if self.is_vla(ast.exprs.get(a).ty()) {
                return None;
            }
            return Some(a);
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
        // An SVE vector (sema rejects `svbool_t` and `__SVCount_t`) is
        // subscripted as a fixed one, its base always on the left.
        let lt = ast.exprs.get(l).ty();
        if self.vec_of(lt).is_some() || ast.types.is_sizeless(lt) {
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

    /// clang `EmitExtVectorElementExpr`: the base vector's address (the
    /// pointer's pointee for `->`) and the named lanes.
    fn ext_vector_lvalue(&mut self, e: ExprId, q: QualType) -> R<LV> {
        let ast = self.ast;
        // `v.wzyx.x`: a swizzle of a swizzle lvalue is one swizzle of
        // the innermost vector (the lanes compose in `swizzle_lanes`).
        let mut inner = e;
        while let Some(b) = self.swizzle_lvalue_base(inner) {
            inner = b;
        }
        let base = ast.exprs.operand(inner);
        let addr = if ast.exprs.get(inner).has(ExprFlags::ARROW) {
            let Some(vq) = ast.types.pointee(ast.exprs.get(base).ty()) else {
                return err(self.loc_of(inner), "malformed ExtVectorElementExpr");
            };
            let (p, align) = self.ptr_align(base)?;
            Addr { v: p, ty: self.mty(vq), align }
        } else {
            self.emit_lvalue(base)?.addr
        };
        let lanes = self.swizzle_lanes(e, true)?;
        let vidx = match lanes.as_slice() {
            // clang's `SizeTy` index.
            [l] => VecLane::Index(self.icn(LTy::I64, i128::from(*l))),
            _ => VecLane::Swizzle(e),
        };
        Ok(LV { addr, q, bf: None, vidx: Some(vidx), tb: Tb::Type })
    }

    /// The `ExtVectorElementExpr` lvalue (through parentheses) that the
    /// `.` access `e` swizzles further, if any.
    fn swizzle_lvalue_base(&self, e: ExprId) -> Option<ExprId> {
        let ast = self.ast;
        if ast.exprs.get(e).has(ExprFlags::ARROW) || !self.is_glvalue(e) {
            return None;
        }
        let mut b = ast.exprs.operand(e);
        while ast.exprs.get(b).kind() == ExprKind::ParenExpr {
            b = ast.exprs.operand(b);
        }
        (ast.exprs.get(b).kind() == ExprKind::ExtVectorElementExpr).then_some(b)
    }

    /// The lanes of its base vector an `ExtVectorElementExpr` selects;
    /// with `compose`, of the innermost vector of a swizzle-of-swizzle
    /// lvalue (a lane past the end stays past it).
    fn swizzle_lanes(&self, e: ExprId, compose: bool) -> R<Vec<u32>> {
        let ast = self.ast;
        let base = ast.exprs.operand(e);
        let mut bq = ast.exprs.get(base).ty();
        if ast.exprs.get(e).has(ExprFlags::ARROW) {
            bq = ast.types.pointee(bq).unwrap_or(bq);
        }
        let Some((_, count)) = self.vec_of(bq) else {
            return err(self.loc_of(e), "ExtVectorElementExpr of a non-vector");
        };
        let name = ast.symbols().as_str(ast.exprs.ext_vector_accessor(e));
        let kiln_ast::expr::ExtVectorAccess::Lanes { lanes, .. } =
            kiln_ast::expr::ext_vector_access(name, count)
        else {
            return err(self.loc_of(e), "invalid vector component name");
        };
        match self.swizzle_lvalue_base(e) {
            Some(b) if compose => {
                let outer = self.swizzle_lanes(b, true)?;
                Ok(lanes
                    .iter()
                    .map(|&l| outer.get(l as usize).copied().unwrap_or(u32::MAX))
                    .collect())
            }
            _ => Ok(lanes),
        }
    }

    /// `lanes` of the vector value `v`: one lane is `extractelement`,
    /// several a one-operand `shufflevector` (a lane past the end is
    /// poison).
    fn swizzle_value(&mut self, v: V, lanes: &[u32]) -> V {
        let count = self.vec_lane_count(v);
        if let [l] = lanes {
            let i = self.icn(LTy::I32, i128::from(*l));
            return self.extract_elt(v, i);
        }
        let mask =
            lanes.iter().map(|&l| if l < count { l } else { kiln_llvm::SHUFFLE_POISON }).collect();
        self.shuffle(v, None, mask)
    }

    /// clang `EmitStoreThroughExtVectorComponentLValue`: `v` shuffled
    /// into `lanes` of the loaded vector `vec`. A source as long as the
    /// vector is one shuffle; a shorter one is widened first, then
    /// shuffled into `vec` (an odd vector's `.hi`/`.odd` names one lane
    /// past its end, which is dropped).
    fn store_swizzle(&mut self, v: V, vec: V, lanes: &[u32]) -> V {
        let src_n = u32::try_from(lanes.len()).unwrap_or(0);
        let dst_n = self.vec_lane_count(vec);
        if dst_n == src_n {
            let mut mask = vec![0; dst_n as usize];
            for (i, &l) in lanes.iter().enumerate() {
                if let Some(m) = mask.get_mut(l as usize) {
                    *m = i as u32;
                }
            }
            return self.shuffle(v, None, mask);
        }
        let mut ext: Vec<u32> = (0..src_n).collect();
        ext.resize(dst_n as usize, kiln_llvm::SHUFFLE_POISON);
        let wide = self.shuffle(v, None, ext);
        let mut mask: Vec<u32> = (0..dst_n).collect();
        let used = if lanes.last() == Some(&dst_n) { lanes.len() - 1 } else { lanes.len() };
        for (i, &l) in lanes.iter().enumerate().take(used) {
            if let Some(m) = mask.get_mut(l as usize) {
                *m = i as u32 + dst_n;
            }
        }
        self.shuffle(vec, Some(wide), mask)
    }

    /// The element count of the vector value `v`.
    fn vec_lane_count(&mut self, v: V) -> u32 {
        let t = self.f.fir.ty(&self.m, v);
        self.m.types.vector_info(t).map_or(0, |x| x.1)
    }

    /// clang `EmitArraySubscriptExpr`.
    fn subscript_lvalue(&mut self, e: ExprId) -> R<LV> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        // `LValue::MakeVectorElt`: the vector's address and the
        // unpromoted index.
        let accessed = self.f.ub_accessed.take() == Some(e);
        if let Some((base, idx)) = self.vec_subscript(e) {
            let blv = self.emit_lvalue(base)?;
            let i = self.emit_scalar(idx)?;
            self.ub_bounds(e, base, i, ast.exprs.get(idx).ty(), accessed)?;
            return Ok(LV {
                addr: blv.addr,
                q,
                bf: None,
                vidx: Some(VecLane::Index(i)),
                tb: Tb::Type,
            });
        }
        let [l, r] = ast.exprs.operands(e);
        let swapped =
            !ast.types.is_pointer(ast.exprs.get(l).ty()) && self.simple_decay_operand(l).is_none();
        let (base, idx) = if swapped { (r, l) } else { (l, r) };
        let pre = if swapped { Some(self.emit_scalar(idx)?) } else { None };
        let iq = ast.exprs.get(idx).ty();
        let mut esz = self.size_align(q).0;
        let mut ety = self.mty(q);
        // clang `getTBAAInfoForSubobject`: an element of an array lvalue
        // starts a new access path.
        let mut tb = Tb::Type;
        let (addr, indices) = if let Some(arr) = self.simple_decay_operand(base) {
            let alv = if ast.exprs.get(arr).kind() == ExprKind::ArraySubscriptExpr {
                // clang: the inner subscript of `a[i][j]` is accessed.
                self.f.ub_accessed = Some(arr);
                self.subscript_lvalue(arr)?
            } else {
                self.emit_lvalue(arr)?
            };
            tb = alv.tb.sub();
            let iv = match pre {
                Some(v) => v,
                None => self.emit_scalar(idx)?,
            };
            self.ub_bounds(e, base, iv, iq, accessed)?;
            let iv = self.int_cast(iv, LTy::I64, self.is_signed(iq));
            let z = self.i64c(0);
            (alv.addr, vec![z, iv])
        } else if self.is_vla_decay(base) || self.is_vla(q) {
            // clang `EmitPointerWithAlignment`: the decayed variable
            // length array's alignment; an element of variable length
            // array type scales the index by its element count.
            let (p, align) = self.ptr_align(base)?;
            let iv = match pre {
                Some(v) => v,
                None => self.emit_scalar(idx)?,
            };
            self.ub_bounds(e, base, iv, iq, accessed)?;
            let iv = self.int_cast(iv, LTy::I64, self.is_signed(iq));
            if let Some((iv, elem)) = self.vla_scaled_index(q, iv)? {
                esz = self.size_align(elem).0;
                ety = self.mty(elem);
                (Addr { v: p, ty: ety, align }, vec![iv])
            } else {
                (Addr { v: p, ty: ety, align }, vec![iv])
            }
        } else {
            // The base pointer with `EmitPointerWithAlignment`'s alignment.
            let (p, align) = self.ptr_align(base)?;
            let iv = match pre {
                Some(v) => v,
                None => self.emit_scalar(idx)?,
            };
            self.ub_bounds(e, base, iv, iq, accessed)?;
            let iv = self.int_cast(iv, LTy::I64, self.is_signed(iq));
            (Addr { v: p, ty: ety, align }, vec![iv])
        };
        let last = *indices.last().expect("a GEP has an index");
        let align = match self.cint(last) {
            Some((_, x, _)) => min_align(addr.align, (x as u64).wrapping_mul(esz)),
            None => min_align(addr.align, esz),
        };
        let is = self.is_signed(iq);
        let v = self.gep_checked(addr.ty, addr.v, indices, !is, is, false, self.ub_expr_loc(e))?;
        Ok(LV { addr: Addr { v, ty: ety, align }, q, bf: None, vidx: None, tb })
    }

    // ---- statement expressions -----------------------------------------

    /// A GNU statement expression with a scalar or `void` value (clang
    /// `EmitCompoundStmt` with `GetLast`): the statements in order; the
    /// result statement (the last non-null one, through labels) is
    /// evaluated as the value.
    pub(crate) fn emit_stmt_expr(&mut self, e: ExprId) -> R<Option<V>> {
        let fp = self.enter_fp(self.ast.exprs.stmt_expr_body(e));
        let r = self.emit_stmt_expr_in(e);
        self.leave_fp(fp);
        r
    }

    fn emit_stmt_expr_in(&mut self, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        let body = ast.exprs.stmt_expr_body(e);
        if ast.types.is_void(ast.exprs.get(e).ty()) {
            self.emit_stmt(body)?;
            return Ok(None);
        }
        let (scope, r) = self.stmt_expr_prefix(e)?;
        let q = ast.exprs.get(r).ty();
        let tmp = self.temp(q);
        if self.is_complex(q) {
            // A complex result is loaded into `cx_ret`.
            let v = self.emit_complex(r)?;
            self.cx_store(v, tmp, q);
            self.end_scope(scope);
            let (re, im) = self.cx_load(tmp, q, (false, false));
            self.f.cx_ret = re.zip(im);
            return Ok(None);
        }
        let v = self.emit_scalar(r)?;
        self.store_scalar(v, tmp, q);
        self.end_scope(scope);
        Ok(Some(self.load_scalar(tmp, q)))
    }

    /// clang `AggExprEmitter::VisitStmtExpr`: a statement expression of
    /// class (or array) type initializes `dest` from its result
    /// expression inside the compound statement's scope, so the copy of a
    /// local is made before the scope's cleanups destroy it.
    fn agg_stmt_expr(&mut self, e: ExprId, dest: Addr, zeroed: bool) -> R<()> {
        let (scope, r) = self.stmt_expr_prefix(e)?;
        self.emit_agg(r, dest, zeroed)?;
        self.end_scope(scope);
        Ok(())
    }

    /// `EmitCompoundStmt(GetLast)` up to the result: opens the lexical
    /// scope, emits every statement before the last one that is not a
    /// null statement, and its labels; returns the scope (for the caller
    /// to end once the result is stored) and the result expression.
    fn stmt_expr_prefix(&mut self, e: ExprId) -> R<(crate::stmt::Scope, ExprId)> {
        let ast = self.ast;
        let body = ast.exprs.stmt_expr_body(e);
        let children: Vec<StmtId> = ast.lists.iter(ast.stmts.compound_body(body)).collect();
        let Some(result) =
            children.iter().rposition(|&c| ast.stmts.get(c).kind() != StmtKind::NullStmt)
        else {
            return err(self.loc_of(e), "statement expression without a result");
        };
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
        Ok((scope, ast.stmts.expr_stmt_expr(s)))
    }

    // ---- ignored expressions -------------------------------------------

    /// clang `EmitIgnoredExpr`.
    pub(crate) fn emit_ignored(&mut self, e: ExprId) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        if matches!(ast.exprs.get(e).kind(), ExprKind::CoawaitExpr | ExprKind::CoyieldExpr) {
            self.emit_coawait(e, crate::coro::Want::Ignored)?;
            return Ok(());
        }
        // GNU `__extension__` is transparent: its operand is the ignored
        // expression (`__extension__ ({ ... });`, arm_neon.h store macros).
        if ast.exprs.get(e).kind() == ExprKind::UnaryOperator
            && ast.exprs.unary_op(e) == UnaryOp::Extension
        {
            return self.emit_ignored(ast.exprs.operand(e));
        }
        if !self.is_glvalue(e) {
            if self.is_aggregate(q) {
                if ast.exprs.get(e).kind() == ExprKind::VAArgExpr {
                    // An ignored slot: the `va_list` advances, nothing is
                    // copied (clang `EmitAggFinalDestCopy`).
                    return self.emit_va_arg(e, None).map(|_| ());
                }
                if ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups {
                    // clang `AggExprEmitter::VisitExprWithCleanups` keeps
                    // the ignored slot: a discarded call on a temporary
                    // (`M{}.e(E{}, 3);`) gets only `EmitCall`'s result
                    // temporary, after the object's and the arguments'.
                    let sc = self.begin_scope(false);
                    let r = self.emit_ignored(self.sub(e));
                    self.end_scope(sc);
                    return r;
                }
                // clang `AggExprEmitter::withReturnValueSlot`: an ignored
                // slot gives the call no address, so `EmitCall` makes its
                // `sret`/coerce temporary after the arguments' `agg.tmp`s.
                let mut c = e;
                while ast.exprs.get(c).kind() == ExprKind::ParenExpr {
                    c = ast.exprs.operand(c);
                }
                match ast.exprs.get(c).kind() {
                    // clang `AggExprEmitter::VisitBinComma`: the left
                    // operand is ignored, the right one keeps the ignored
                    // slot, so its result temporary is made in order
                    // (`M{}.b(), M{}.b();`, WP-774).
                    ExprKind::BinaryOperator if ast.exprs.binary_op(c) == BinaryOp::Comma => {
                        let [l, r] = ast.exprs.operands(c);
                        self.emit_ignored(l)?;
                        return self.emit_ignored(r);
                    }
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
            if self.is_complex(q) {
                self.emit_cx(e, (true, true))?;
                return Ok(());
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
        let outer = self.f.call_loc.replace(self.ast.begin_loc(e));
        let r = self.emit_call_inner(e, dest);
        self.f.call_loc = outer;
        r
    }

    fn emit_call_inner(&mut self, e: ExprId, dest: Option<Addr>) -> R<Option<V>> {
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
        // WP-810: a call of a builtin alias (`arm_sve.h`'s intrinsics) lowers
        // as the builtin it names, or is a hard error; never an external call.
        if let Some(d) = direct
            && let Some((b, _)) = ast.builtin_alias(d)
        {
            return self.emit_sve_alias(d, b, e);
        }
        // A builtin is never emitted as a function (spec/codegen.md §3, "Calls"):
        // it folds, lowers inline, or calls the library function it names.
        // `std::move(x)` and its kin fold to the address of `x` (also in
        // a function with the wildcard `no_builtin`, unlike clang 19,
        // spec/codegen.md "no-builtin").
        if let Some(d) = direct
            && self.is_std_builtin(d)
            && let Some(a) = ast.lists.iter::<ExprId>(ast.exprs.call_args(e)).next()
        {
            return Ok(Some(self.emit_lvalue(a)?.addr.v));
        }
        // WP-940: clang `EmitBuiltinExpr` first folds a library builtin
        // call its evaluator evaluates (`strlen("abc")` is `i64 3`); the
        // arguments are not emitted. Not where `no_builtin` names it.
        if let Some(d) = direct
            && crate::builtins::lib_fold_callee(ast, e).is_some()
            && let Some(c) = self.folds.get(e)
            && !self.no_builtin_here(d)
        {
            let q = ast.exprs.get(e).ty();
            if ast.types.is_integral(q) || ast.types.is_floating(q) {
                return Ok(Some(V::C(self.const_scalar_value(c, q)?)));
            }
        }
        let mut lib = None;
        // WP-419: a recognized C library function clang lowers as its
        // `__builtin_` twin (`ceil(x)` -> `llvm.ceil`).
        // WP-808: not in a function whose `no_builtin` attribute names it
        // (clang `EmitDirectCallee`, `HasAttributeNoBuiltin`).
        if let Some(d) = direct
            && let Some(id) = ast.decls.builtin_id(d).or_else(|| {
                crate::builtins::lib_intrinsic(self, d).filter(|_| !self.no_builtin_here(d))
            })
        {
            match self.emit_builtin(id, e, d)? {
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
            let fty = ast.decls.get(d).ty().expect("Sema: a function has a type");
            (V::C(Value::func(f)), fty, mangle::is_extern_c(ast, ast.first(d)), Some(f))
        } else {
            let p = self.emit_scalar(callee_e)?;
            let pq = ast.exprs.get(callee_e).ty();
            // The pointee as written: before C++17 it alone keeps the
            // exception specification (spec/types.md §5.4).
            let fty = ast.types.exc_pointee(pq).expect("Sema: function pointer");
            // A callee that folded to a function (a constant `int (&)(int)`
            // naming one) is that function, as clang's `CalleePtr` is
            // (`EmitCall` / `TryMarkNoThrow` consult its `nounwind`).
            let fid = match p {
                V::C(v) => match v.get() {
                    kiln_llvm::ValueRef::Func(f) => Some(f),
                    _ => None,
                },
                _ => None,
            };
            (p, fty, false, fid)
        };
        let mut sig = self.call_sig(fty, extern_c, &arg_tys);
        let mut extra: Vec<Attr> = lib.map(|(_, x)| x).unwrap_or_default();
        // A callee with a `NoThrowAttr` (a recognized C library function)
        // or a `const` / `pure` attribute.
        if let Some(d) = direct {
            let const_pure = crate::builtins::const_pure_attrs(ast, d, &sig, &mut extra);
            sig.nothrow |= const_pure || ast.has_nothrow_attr(d);
        }
        let mut noreturn = extra.contains(&Attr::Flag(AttrKind::NoReturn));
        if !noreturn {
            let (site, unreachable) = self.noreturn_call(direct, fty);
            if site {
                extra.push(Attr::Flag(AttrKind::NoReturn));
            }
            noreturn = unreachable;
        }
        // clang `EmitCall` checks target features before the arguments.
        if let Some(d) = direct {
            self.target_feature_call_check(e, d);
        }
        let vals = self.emit_call_args_for(&sig, &args, direct)?;
        let mut ret_extra = Vec::new();
        if let Some(d) = direct {
            self.psabi_call_check(e, d, &args, false);
            self.alloc_call_attrs(d, &vals, &mut extra, &mut ret_extra);
        }
        // WP-803: a hidden `size_t` argument after each `pass_object_size`
        // argument (clang `EmitCallArgs`).
        let sized =
            direct.and_then(|d| self.object_size_args(d, fty, &sig, &vals, &args, extern_c));
        let (cvals, cargs);
        let (cv, ca) = match sized {
            Some((s, v, a)) => {
                (sig, cvals, cargs) = (s, v, a);
                (&cvals, &cargs)
            }
            None => (&vals, &args),
        };
        let mark = self.f.fir.inst_count();
        let from = self.f.calls.len();
        self.arm_musttail(e);
        let r = self.call_vals_ret(&sig, callee, fid, None, cv, ca, dest, &extra, &ret_extra)?;
        // WP-806: a library builtin's call takes the FP scope's fast-math
        // flags (clang `EmitBuiltinExpr`'s `CGFPOptionsRAII`).
        if let (Some(d), Some(f)) = (direct, fid)
            && (ast.decls.builtin_id(d).is_some() || ast.decls.lib_builtin(d).is_some())
        {
            self.f.fir.mark_scope_calls(mark, f);
        }
        if let (Some(d), Some(p)) = (direct, r) {
            self.alloc_align_assume(d, &vals, p);
        }
        if noreturn {
            self.finish_noreturn_call(from, fid, Some(e))?;
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

    /// The returned call `e` of a `[[clang::musttail]]` return is about
    /// to be emitted, its arguments evaluated (clang `EmitCall`'s
    /// `IsMustTail`).
    pub(crate) fn arm_musttail(&mut self, e: ExprId) {
        if self.f.musttail == Some(e) {
            self.f.musttail = None;
            self.f.musttail_armed = true;
        }
    }

    /// clang `EmitCall` of a `musttail` call `r` (WP-809): the call is
    /// marked `musttail` and returned at once (`ret` of its raw result,
    /// or `ret void`), and what follows goes to a fresh unreachable block
    /// with an undefined result of type `rq` (`GetUndefRValue`; an
    /// aggregate gets its unused temporary). A call that became an
    /// `invoke` skips over cleanups, which LLVM cannot tail-call.
    fn finish_musttail(
        &mut self,
        r: V,
        rq: QualType,
        rty: LTy,
        loc: Option<kiln_source::Loc>,
    ) -> R<Option<V>> {
        let i = Fir::last_inst(r);
        if !matches!(self.f.fir.op(i), Op::Call { .. }) {
            return err(loc, "cannot compile this tail call skipping over cleanups yet");
        }
        self.f.fir.set_musttail(i);
        let v = (rty != LTy::VOID).then_some(r);
        self.f.fir.push(Op::Ret { v }, LTy::VOID);
        self.f.fir.set_cur(None);
        let cont = self.f.fir.create_block();
        self.emit_block(cont, false);
        if self.ast.types.is_void(rq) {
            return Ok(None);
        }
        if self.is_aggregate(rq) || self.is_atomic_aggregate(rq) {
            self.temp(rq);
            return Ok(None);
        }
        if let Some(eq) = self.cx_elem(rq) {
            let et = self.lty(eq);
            let u = V::C(self.m.const_undef(et));
            self.f.cx_ret = Some((u, u));
            return Ok(None);
        }
        let t = self.lty(rq);
        Ok(Some(V::C(self.m.const_undef(t))))
    }

    /// clang `EmitCall` of a call that does not return (of `fid`, the
    /// first one recorded in `self.f.calls` from `from`; `e` its
    /// expression): `unreachable` ([`Cg::ub_noreturn_call`] first), then
    /// a dummy block for the unreachable code that follows.
    pub(crate) fn finish_noreturn_call(
        &mut self,
        from: usize,
        fid: Option<FuncId>,
        e: Option<ExprId>,
    ) -> R<()> {
        if self.f.fir.cur().is_none() {
            return Ok(());
        }
        self.ub_noreturn_call(from, fid, e)?;
        self.f.fir.push(Op::Unreachable, LTy::VOID);
        self.f.fir.set_cur(None);
        let cont = self.f.fir.create_block();
        self.emit_block(cont, false);
        Ok(())
    }

    /// WP-803: a call of `d` (type `fty`, signature `sig`) with a hidden
    /// `size_t` argument after each `pass_object_size` argument (clang
    /// `EmitCallArgs`, `MaybeEmitImplicitObjectSize`): the signature, the
    /// values and the argument expressions with them; `None` if `d` has
    /// no such parameter. `vals` are `args` emitted (no object argument).
    pub(crate) fn object_size_args(
        &mut self,
        d: DeclId,
        fty: QualType,
        sig: &Sig,
        vals: &[CallArg],
        args: &[ExprId],
        extern_c: bool,
    ) -> Option<(Sig, Vec<CallArg>, Vec<ExprId>)> {
        let ps = self.ast.pass_object_size_params(d);
        if ps.is_empty() {
            return None;
        }
        let (mut cvals, mut cargs) = (vals.to_vec(), args.to_vec());
        for &(i, t, dynamic) in ps.iter().rev() {
            if let (Some(&a), Some(&CallArg::Scalar(v))) = (args.get(i), vals.get(i)) {
                let s = self.pass_object_size_arg(a, v, t, dynamic);
                cvals.insert(i + 1, CallArg::Scalar(s));
                cargs.insert(i + 1, a);
            }
        }
        let sig = self.with_object_sizes(sig.clone(), fty, &ps, extern_c);
        Some((sig, cvals, cargs))
    }

    /// clang `EmitCallArgs`: evaluates every argument, aggregates into
    /// `agg.tmp`.
    pub(crate) fn emit_call_args(&mut self, sig: &Sig, args: &[ExprId]) -> R<Vec<CallArg>> {
        self.emit_call_args_for(sig, args, None)
    }

    /// [`Self::emit_call_args`] of a call whose direct callee is `callee`
    /// (clang's `AbstractCallee`): each scalar argument gets its
    /// `nonnull` check after it is evaluated.
    pub(crate) fn emit_call_args_for(
        &mut self,
        sig: &Sig,
        args: &[ExprId],
        callee: Option<DeclId>,
    ) -> R<Vec<CallArg>> {
        let ast = self.ast;
        let arg_tys: Vec<QualType> = args.iter().map(|&a| ast.exprs.get(a).ty()).collect();
        let mut vals = Vec::with_capacity(args.len());
        for (i, &a) in args.iter().enumerate() {
            // clang `EmitCallArg`'s `DisableDebugLocationUpdates`.
            let off = self.dbg_disable(ast.exprs.get(a).kind() == ExprKind::CXXDefaultArgExpr);
            let v = self.emit_call_arg(sig.params[i], a, arg_tys[i]);
            self.dbg_enable(off);
            let v = v?;
            // clang `EmitNonNullArgCheck`, after `EmitCallArg`, of an
            // argument that is not a reference binding.
            if let (Some(d), CallArg::Scalar(x)) = (callee, &v)
                && self.referee(sig.params[i]).is_none()
            {
                self.ub_nonnull_arg(d, i, a, *x)?;
            }
            vals.push(v);
        }
        Ok(vals)
    }

    /// clang `EmitCallArg` of `a` (of type `ty`) for parameter type
    /// `param`.
    fn emit_call_arg(&mut self, param: QualType, a: ExprId, ty: QualType) -> R<CallArg> {
        if self.referee(param).is_some() {
            // A reference parameter binds the argument glvalue.
            let v = self.emit_lvalue(a)?.addr.v;
            self.ub_ref_binding(a, v)?;
            return Ok(CallArg::Scalar(v));
        }
        if self.is_aggregate(ty) || self.is_atomic_aggregate(ty) {
            let t = self.temp(ty);
            // clang `EmitCallArg`: an argument the callee destroys is
            // externally destructed, with an EH-only cleanup until
            // the call (`DestroyUnpassedArg`).
            let callee_destroyed = kiln_abi::call::is_destroyed_in_callee(self.ast, ty);
            let saved = std::mem::replace(&mut self.f.dest_destructed, callee_destroyed);
            let r = self.emit_agg(a, t, false);
            self.f.dest_destructed = saved;
            r?;
            if callee_destroyed && let Some(rd) = self.destructed_class(ty) {
                self.push_arg_cleanup(t.v, rd);
            }
            return Ok(CallArg::Agg(t));
        }
        if self.is_complex(ty) {
            let (re, im) = self.emit_complex(a)?;
            return Ok(CallArg::Complex(re, im));
        }
        Ok(CallArg::Scalar(self.emit_scalar(a)?))
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
        self.call_vals_to(sig, Callee::Value(callee), fid, this, vals, args, dest, extra, &[])
    }

    /// [`Self::call_vals`] with extra return attributes `ret_extra`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call_vals_ret(
        &mut self,
        sig: &Sig,
        callee: V,
        fid: Option<FuncId>,
        this: Option<V>,
        vals: &[CallArg],
        args: &[ExprId],
        dest: Option<Addr>,
        extra: &[Attr],
        ret_extra: &[Attr],
    ) -> R<Option<V>> {
        self.call_vals_to(sig, Callee::Value(callee), fid, this, vals, args, dest, extra, ret_extra)
    }

    /// [`Self::call_vals_ret`] with a callee that may be computed only after
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
        ret_extra: &[Attr],
    ) -> R<Option<V>> {
        let musttail = std::mem::take(&mut self.f.musttail_armed);
        self.f.vector_width = self.f.vector_width.max(sig.info.max_vector_width);
        // The direct callee whose class B call-site attributes apply
        // (set by `alloc_call_attrs`, WP-813).
        let callee_decl = self.f.call_decl.take();
        self.check_pure_scalable(sig, self.f.call_loc)?;
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
            let at = ir_args.len();
            self.call_arg(&kind, sig.params[i], v, &mut ir_args, loc)?;
            // clang `EmitCall`: a `maybe_undef` argument is frozen.
            if callee_decl.is_some_and(|d| self.maybe_undef_param(d, i)) {
                for a in &mut ir_args[at..] {
                    let ty = self.f.fir.ty(&self.m, *a);
                    *a = self.f.fir.push(Op::Freeze { v: *a }, ty);
                }
            }
        }
        let callee = match callee {
            Callee::Value(v) => v,
            Callee::Virtual { md, this, align } => self.virtual_fn_ptr(md, None, this, align)?,
        };
        self.deactivate_arg_cleanups(vals);
        let rty = match &sig.info.ret.kind {
            ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. } => {
                let c = coerce.clone();
                self.ir(&c)
            }
            _ => LTy::VOID,
        };
        let (mut ret, mut params) = self.sig_attrs(sig, vals.len(), false);
        if let Some(d) = callee_decl {
            self.class_b_params(d, sig, &mut params);
        }
        if !ret_extra.is_empty() {
            let mut ra = self.m.attrs_of(ret).to_vec();
            ra.extend(ret_extra.iter().cloned());
            ret = self.m.attr_set(&ra);
        }
        let mut fa = Vec::new();
        if sig.nothrow {
            fa.push(Attr::Flag(AttrKind::NoUnwind));
        }
        if sig.no_csr {
            fa.push(Attr::Str("no_caller_saved_registers".into(), "".into()));
        }
        fa.extend(extra.iter().cloned());
        self.call_size_attrs(fid, &mut fa);
        self.stmt_call_attrs(callee_decl, &mut fa);
        let func = if fa.is_empty() { AttrList::default().func } else { self.m.attr_set(&fa) };
        let tail = if callee_decl.is_some_and(|d| self.not_tail_called(d)) {
            kiln_llvm::TailKind::NoTail
        } else {
            kiln_llvm::TailKind::None
        };
        let opts = CallOpts {
            attrs: AttrList { func, ret, params },
            cc: sig.llvm_cc(),
            tail,
            ..CallOpts::default()
        };
        let r = self.emit_call_op(sig.fn_ty, callee, ir_args, opts, rty, fid, sig.nothrow);
        if musttail {
            let loc = args.first().and_then(|&a| self.loc_of(a));
            return self.finish_musttail(r, rq, rty, loc);
        }
        if let Some(a) = sret_addr {
            // clang `convertTempToRValue`: a scalar returned indirectly
            // (a vector wider than the registers) is loaded back.
            if !self.is_aggregate(rq)
                && !self.is_atomic_aggregate(rq)
                && !matches!(
                    self.ast.types.canonical_view(rq),
                    kiln_ast::types::TypeView::Complex(_)
                )
            {
                return Ok(Some(self.load_scalar(a, rq)));
            }
            if self.is_complex(rq) {
                self.f.cx_ret = match self.cx_load(a, rq, (false, false)) {
                    (Some(re), Some(im)) => Some((re, im)),
                    _ => None,
                };
            }
            return Ok(None);
        }
        if rty == LTy::VOID {
            if matches!(sig.info.ret.kind, ArgKind::Ignore) && self.is_aggregate(rq) {
                // clang `EmitCall` -> `GetUndefRValue`: an ignored
                // aggregate result still gets its (unused) temporary.
                self.temp(rq);
            }
            return Ok(None);
        }
        if self.is_complex(rq) {
            let kind = sig.info.ret.kind.clone();
            self.f.cx_ret = Some(self.cx_call_result(&kind, r, rq));
            return Ok(None);
        }
        if self.is_aggregate(rq) || self.is_atomic_aggregate(rq) {
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
            let kind = sig.info.ret.kind.clone();
            let (coerce, offset) = match &kind {
                ArgKind::Direct { coerce, offset, .. } => (coerce.clone(), *offset),
                ArgKind::Extend { coerce, .. } => (coerce.clone(), 0),
                _ => unreachable!("sret, Ignore handled above"),
            };
            if offset != 0 {
                // clang `EmitCall`: the result's bytes start past the
                // empty eightbyte (`emitAddressAtOffset`).
                let at = self.at_abi_offset(d, &kind);
                self.coerced_store_sized(r, &coerce, at, size.saturating_sub(offset));
            } else if self.ir(&coerce) == self.mty(rq) {
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
                unreachable!("sret, Ignore handled above")
            };
            // clang `EmitCall`: a fixed-length SVE result of the scalable
            // type's element type is extracted (WP-810); a predicate goes
            // through memory.
            let lt = self.lty(rq);
            if matches!(coerce, kiln_abi::irtype::IrType::ScalableVector(..))
                && self.m.types.vector_info(rty).map(|x| x.0)
                    == self.m.types.vector_info(lt).map(|x| x.0)
                && let Some(v) = self.sve_to_fixed(r, lt)
            {
                return Ok(Some(v));
            }
            let t = self.temp(rq);
            let (size, _) = self.size_align(rq);
            self.coerced_store_sized(r, &coerce, t, size);
            return Ok(Some(self.load_scalar(t, rq)));
        }
        if rty == LTy::BFLOAT && self.is_x86() && !self.opts.optimize() && self.opts.llvm_major < 22
        {
            // spec/abi.md §8.18: LLVM 19's x86-64 FastISel (`-O0`) takes
            // a `bfloat` call result for the already widened `float`, so a
            // result passed straight on reaches `__truncsfbf2` as raw
            // bits. Through memory the result is extended correctly. LLVM
            // 22 and the optimizing selector are not affected.
            let t = self.temp(rq);
            self.store(r, t.v, t.align, false);
            return Ok(Some(self.load(LTy::BFLOAT, t.v, t.align, false)));
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
    fn is_simple_zero(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        let mut e = e;
        let k = ast.exprs.get(e).kind();
        if matches!(k, ExprKind::ImplicitValueInitExpr | ExprKind::CXXScalarValueInitExpr) {
            return self.zero_init(ast.exprs.get(e).ty());
        }
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
        if ast.exprs.get(e).kind() != ExprKind::InitListExpr || !self.zero_init(q) {
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

    /// clang `CheckAggExprForMemSetUse`'s first test: objects of (arrays
    /// of) a class with a user-declared constructor are never zeroed by
    /// a `memset` first.
    fn base_class_has_user_ctor(&self, q: QualType) -> bool {
        let ast = self.ast;
        let mut q = q;
        while let Some(e) = ast.types.element(q) {
            q = e;
        }
        ast.types.as_record(q).is_some_and(|rd| {
            self.class_bits_of(rd).contains(kiln_ast::decl::DefinitionBits::USER_DECLARED_CTOR)
        })
    }

    /// clang `EmitAggExpr` (with `CheckAggExprForMemSetUse`).
    pub(crate) fn emit_agg(&mut self, e: ExprId, dest: Addr, zeroed: bool) -> R<()> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let mut zeroed = zeroed;
        if !zeroed && !self.is_volatile(q) && !self.base_class_has_user_ctor(q) {
            let size = self.size_align(q).0;
            if size > 16 && self.non_zero_bytes(e) * 4 <= size {
                self.memset(dest, 0, size, false);
                zeroed = true;
            }
        }
        self.agg_visit(e, dest, zeroed)
    }

    fn agg_visit(&mut self, e: ExprId, dest: Addr, zeroed: bool) -> R<()> {
        if self.f.ub_checked
            && !matches!(
                self.ast.exprs.get(e).kind(),
                ExprKind::ExprWithCleanups
                    | ExprKind::CXXBindTemporaryExpr
                    | ExprKind::ParenExpr
                    | ExprKind::CXXDefaultInitExpr
                    | ExprKind::MaterializeTemporaryExpr
                    | ExprKind::CXXConstructExpr
                    | ExprKind::CXXTemporaryObjectExpr
            )
        {
            self.f.ub_checked = false;
        }
        let dl = self.dbg_apply_expr(e);
        self.di_explicit_cast(e);
        let r = self.agg_visit_x(e, dest, zeroed);
        self.dbg_restore(dl);
        r
    }

    fn agg_visit_x(&mut self, e: ExprId, dest: Addr, zeroed: bool) -> R<()> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        match n.kind() {
            // A glvalue's value is the address it designates (WP-640).
            ExprKind::ConstantExpr if self.is_glvalue(e) && self.immediate_value(e).is_some() => {
                let lv = self.emit_lvalue(e)?;
                let size = self.size_align(q).0;
                self.agg_copy(q, dest, lv.addr, size, false);
                Ok(())
            }
            ExprKind::ConstantExpr if self.immediate_value(e).is_some() => {
                // clang `AggExprEmitter::VisitConstantExpr`: the value of
                // an immediate invocation, stored by `CreateCoercedStore`.
                // The destination size is `AggValueSlot::getPreferredSize`:
                // the data size when the slot may overlap.
                let overlap = std::mem::take(&mut self.f.overlap);
                let c = self.immediate_value(e).expect("match guard checked it");
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
            // GNU `__extension__` (clang `AggExprEmitter::VisitUnaryExtension`).
            ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::Extension => {
                self.agg_visit(ast.exprs.operand(e), dest, zeroed)
            }
            ExprKind::CoawaitExpr | ExprKind::CoyieldExpr => {
                self.emit_coawait(e, crate::coro::Want::Agg(dest, zeroed))?;
                Ok(())
            }
            ExprKind::ChooseExpr if ast.exprs.choose_chosen(e).is_some() => self.agg_visit(
                ast.exprs.choose_chosen(e).expect("match guard checked it"),
                dest,
                zeroed,
            ),
            // clang `AggExprEmitter::VisitCXXRewrittenBinaryOperator`: the
            // semantic form (`(y <=> x)` reversed, or `0 <=> (y <=> x)`).
            ExprKind::CXXRewrittenBinaryOperator => {
                self.agg_visit(ast.exprs.rewritten_semantic(e), dest, zeroed)
            }
            ExprKind::PseudoObjectExpr => {
                let r = self.bind_pseudo(e)?;
                let v = self.emit_agg(r, dest, zeroed);
                self.unbind_pseudo(e);
                v
            }
            // clang `VisitCompoundLiteralExpr`: `EmitAggExpr` of the
            // initializer into the slot (C++: not destroyed here).
            ExprKind::CompoundLiteralExpr => {
                self.emit_agg(ast.exprs.compound_literal_init(e), dest, zeroed)
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
            ExprKind::VAArgExpr => self.emit_va_arg(e, Some(dest)).map(|_| ()),
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
                    // clang `EmitCXXTemporary`: `pushDestroy` of the
                    // temporary's type, an array destroyed element-wise.
                    let c = if self.array_elem(q).is_some() {
                        Cleanup::ArrayDtor { addr: dest, record: rd }
                    } else {
                        Cleanup::Dtor { addr: dest.v, record: rd, base: false }
                    };
                    self.push_full_expr_cleanup(c, true, true);
                }
                Ok(())
            }
            ExprKind::CXXDefaultInitExpr => match self.default_member_init(e) {
                Some(init) => {
                    let saved = self.enter_default_init(e);
                    let r = self.agg_visit(init, dest, zeroed);
                    self.exit_default_init(saved);
                    r
                }
                None => err(self.loc_of(e), "default member initializer not yet parsed"),
            },
            ExprKind::CXXMemberCallExpr if self.is_glvalue(e) => {
                let lv = self.emit_lvalue(e)?;
                let size = self.size_align(q).0;
                self.agg_copy(q, dest, lv.addr, size, false);
                Ok(())
            }
            ExprKind::CXXMemberCallExpr => {
                self.emit_member_call(e, Some(dest))?;
                Ok(())
            }
            // A braced `_Atomic` initializer holds its one converted
            // element (clang `InitListExpr::isTransparent`).
            ExprKind::InitListExpr if ast.types.as_atomic(q).is_some() => {
                match ast.lists.iter::<ExprId>(ast.exprs.init_list_inits(e)).next() {
                    Some(x) => self.agg_visit(x, dest, zeroed),
                    None => err(self.loc_of(e), "empty `_Atomic` initializer list"),
                }
            }
            // clang `VisitCXXParenListOrInitListExpr`: a parenthesized
            // aggregate initialization ([dcl.init.general]/16.5, /16.6.2.2)
            // is emitted as the braced list with the same elements.
            ExprKind::InitListExpr | ExprKind::CXXParenListInitExpr => {
                if ast.types.is_array(q) {
                    self.array_init(e, dest, zeroed)
                } else {
                    self.record_init(e, dest, zeroed)
                }
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr if self.is_glvalue(e) => {
                let lv = self.emit_lvalue(e)?;
                let size = self.size_align(q).0;
                self.agg_copy(q, dest, lv.addr, size, false);
                Ok(())
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr => {
                self.emit_call(e, Some(dest))?;
                Ok(())
            }
            ExprKind::DesignatedInitUpdateExpr => {
                // clang `AggExprEmitter::VisitDesignatedInitUpdateExpr`:
                // the prior initialization, then the updater's clauses
                // over it; its `NoInitExpr` slots leave the bytes alone
                // ([dcl.init.aggr]/4, C99 6.7.9p19).
                let [base, updater] = ast.exprs.designated_init_update(e);
                self.emit_agg(base, dest, zeroed)?;
                self.agg_visit(updater, dest, false)
            }
            ExprKind::NoInitExpr => Ok(()),
            ExprKind::ImplicitValueInitExpr | ExprKind::CXXScalarValueInitExpr => {
                if !(zeroed && self.zero_init(q)) {
                    let size = self.size_align(q).0;
                    self.emit_null_init(dest, q, size)?;
                }
                Ok(())
            }
            k if k.is_cast()
                && matches!(
                    ast.exprs.cast_kind(e),
                    CastKind::NonAtomicToAtomic | CastKind::AtomicToNonAtomic
                ) =>
            {
                self.agg_atomic_cast(e, dest, zeroed)
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
                    if self.is_atomic_aggregate(lv.q) {
                        self.atomic_agg_load(lv, dest);
                        return Ok(());
                    }
                    let size = self.size_align(q).0;
                    let vol = self.is_volatile(q) || self.is_volatile(lv.q);
                    self.agg_copy(q, dest, lv.addr, size, vol);
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
                if self.is_atomic_aggregate(lv.q) {
                    self.atomic_agg_load(lv, dest);
                    return Ok(());
                }
                let size = self.size_align(q).0;
                self.agg_copy(q, dest, lv.addr, size, false);
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
                    if x {
                        self.prof_inc(Key::Expr(e));
                    }
                    return self.agg_visit(if x { a } else { b }, dest, zeroed);
                }
                let lb = self.f.fir.create_block();
                let rb = self.f.fir.create_block();
                let cont = self.f.fir.create_block();
                let start = self.f.fir.cur();
                self.branch_on_bool(c, lb, rb)?;
                // Both arms see the same slot (`IsDestructed` included).
                let destructed = self.f.dest_destructed;
                self.emit_block(lb, false);
                self.prof_inc(Key::Expr(e));
                self.cond_eval(start, |cg| cg.agg_visit(a, dest, zeroed))?;
                self.br(cont);
                self.emit_block(rb, false);
                self.f.dest_destructed = destructed;
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
                let deferred = self.f.cl.deferred.len();
                for (f, init) in fields.into_iter().zip(inits) {
                    let fq = ast.decls.get(f).ty().expect("Sema: a capture has a type");
                    let lv = self.field_lvalue_init(base, f, fq)?;
                    if self.referee(fq).is_some() {
                        // clang `EmitInitializationToLValue` on a
                        // reference: bind, then store the address.
                        let p = self.emit_ref_binding(init)?;
                        let st = self.store(p, lv.addr.v, lv.addr.align, false);
                        self.tbaa_mark(st, fq, lv.tb);
                    } else {
                        self.init_to_lvalue(init, lv, false)?;
                    }
                    if let Some(frd) = self.destructed_class(fq) {
                        let c = if self.array_elem(fq).is_some() {
                            Cleanup::ArrayDtor { addr: lv.addr, record: frd }
                        } else {
                            Cleanup::Dtor { addr: lv.addr.v, record: frd, base: false }
                        };
                        self.push_deferred_destroy(c);
                    }
                }
                self.deactivate_deferred(deferred);
                Ok(())
            }
            ExprKind::BinaryOperator if ast.exprs.binary_op(e) == BinaryOp::Cmp => {
                self.agg_three_way(e, dest)
            }
            ExprKind::StmtExpr => self.agg_stmt_expr(e, dest, zeroed),
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
        if k == ExprKind::NoInitExpr {
            return Ok(());
        }
        if lv.bf.is_none() && self.referee(lv.q).is_some() {
            // A reference member binds to its initializer's address
            // (clang `EmitInitializationToLValue`, reference case).
            let p = self.emit_ref_binding(e)?;
            let st = self.store(p, lv.addr.v, lv.addr.align, false);
            self.tbaa_mark(st, lv.q, lv.tb);
            return Ok(());
        }
        if lv.bf.is_none() && self.ast.types.as_atomic(lv.q).is_some() {
            // clang `EmitAtomicInit` (WP-165).
            return self.atomic_init(e, lv);
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
        if self.is_complex(lv.q) {
            let v = self.emit_complex(e)?;
            self.cx_store(v, lv.addr, lv.q);
            return Ok(());
        }
        let v = self.emit_scalar(e)?;
        self.store_lv(v, lv, false);
        Ok(())
    }

    /// clang `EmitNullInitializationToLValue`.
    pub(crate) fn null_init_lvalue(&mut self, lv: LV, zeroed: bool) -> R<()> {
        if zeroed && self.zero_init(lv.q) {
            return Ok(());
        }
        if self.is_aggregate(lv.q) || self.is_complex(lv.q) {
            let size = self.size_align(lv.q).0;
            return self.emit_null_init(lv.addr, lv.q, size);
        }
        let z = V::C(self.null_const(lv.q)?);
        if lv.bf.is_some() {
            self.store_lv(z, lv, false);
        } else {
            let vol = self.is_volatile(lv.q);
            // clang `EmitStoreThroughLValue`: the lvalue's access tag.
            let st = self.store(z, lv.addr.v, lv.addr.align, vol);
            self.tbaa_mark(st, lv.q, lv.tb);
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
        let q0 = ast.decls.get(f0).ty().expect("Sema: a field has a type");
        let lv0 = self.field_lvalue(base, f0, q0)?;
        self.store_scalar(a, lv0.addr, q0);
        let q1 = ast.decls.get(f1).ty().expect("Sema: a field has a type");
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
        let Some(rd) = self.ast.types.as_record(self.ast.exprs.get(e).ty()) else {
            return err(self.loc_of(e), "initializer list of non-aggregate type");
        };
        // clang `VisitCXXParenListOrInitListExpr`'s `FieldConstructionScope`.
        let saved = self.f.default_init_this.replace((dest.v, rd));
        let r = self.record_init_fields(e, rd, dest, zeroed);
        self.f.default_init_this = saved;
        r
    }

    /// [`Self::record_init`] of class `rd`.
    fn record_init_fields(&mut self, e: ExprId, rd: DeclId, dest: Addr, zeroed: bool) -> R<()> {
        let ast = self.ast;
        let (inits, _, union_field) = agg_init_parts(ast, e);
        let is_union = self.tys.record(rd).is_union;
        let rty = self.record_ty(rd);
        let base = Addr { v: dest.v, ty: rty, align: dest.align };
        if is_union {
            let Some(f) = union_field else { return Ok(()) };
            let fq = ast.decls.get(f).ty().expect("Sema: a field has a type");
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
        let deferred = self.f.cl.deferred.len();
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
                self.push_deferred_destroy(c);
            }
        }
        let fields: Vec<DeclId> = ast
            .decls
            .members(def)
            .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
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
            let fq = ast.decls.get(f).ty().expect("Sema: a field has a type");
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
                self.push_deferred_destroy(c);
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
        let eq = ast.types.element(q).expect("caller checked: array type");
        let esz = self.size_align(eq).0;
        let ety = self.mty(eq);
        let ealign = min_align(dest.align, esz);
        let zero = self.i64c(0);
        let begin = self.gep(dest.ty, dest.v, vec![zero, zero], true);
        let entry = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
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
        let lv = LV {
            addr: Addr { v: el, ty: ety, align: ealign },
            q: eq,
            bf: None,
            vidx: None,
            tb: Tb::Type,
        };
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
        let cur = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
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
    fn push_deferred_destroy(&mut self, c: Cleanup) {
        if self.f.fir.cur().is_none() {
            return;
        }
        let z = V::C(self.m.const_bool(false));
        let dominator =
            self.f.fir.push(Op::Bin { op: BinOp::Or, flags: Flags::NONE, l: z, r: z }, LTy::I1);
        self.push_full_expr_cleanup(c, true, true);
        let depth = self.f.cl.depth();
        self.f.cl.deferred.push((depth, dominator));
    }

    /// clang `EmitCallArg`'s `pushFullExprCleanup<DestroyUnpassedArg>`
    /// and `addArgCleanupDeactivation`: the EH-only destructor cleanup of
    /// the callee-destroyed argument at `addr` (class `rd`), deactivated
    /// by [`Self::deactivate_arg_cleanups`] at the call.
    fn push_arg_cleanup(&mut self, addr: V, rd: DeclId) {
        if self.f.fir.cur().is_none() {
            return;
        }
        self.push_full_expr_cleanup(Cleanup::Dtor { addr, record: rd, base: false }, false, true);
        let z = V::C(self.m.const_bool(false));
        let marker =
            self.f.fir.push(Op::Bin { op: BinOp::Or, flags: Flags::NONE, l: z, r: z }, LTy::I1);
        let depth = self.f.cl.depth();
        self.f.arg_cleanups.push((addr, depth, marker));
    }

    /// clang `EmitDelegateCallArg`: forwarding parameter `a`, which this
    /// function destroys, hands its destruction to the callee.
    pub(crate) fn forward_param_cleanup(&mut self, a: V) {
        if self.f.fir.cur().is_none() {
            return;
        }
        let Some(&(_, depth)) = self.f.param_cleanups.iter().find(|x| x.0 == a) else { return };
        let z = V::C(self.m.const_bool(false));
        let marker =
            self.f.fir.push(Op::Bin { op: BinOp::Or, flags: Flags::NONE, l: z, r: z }, LTy::I1);
        self.f.arg_cleanups.push((a, depth, marker));
    }

    /// clang `deactivateArgCleanupsBeforeCall`: deactivates the pending
    /// argument cleanups of the aggregate arguments in `vals`, innermost
    /// first, and drops their placeholders.
    fn deactivate_arg_cleanups(&mut self, vals: &[CallArg]) {
        if self.f.arg_cleanups.is_empty() {
            return;
        }
        let mut mine = Vec::new();
        for v in vals {
            if let CallArg::Agg(a) = v
                && let Some(i) = self.f.arg_cleanups.iter().rposition(|x| x.0 == a.v)
            {
                mine.push(self.f.arg_cleanups.remove(i));
            }
        }
        mine.sort_by_key(|x| x.1);
        for (_, depth, marker) in mine.into_iter().rev() {
            let d = Fir::last_inst(marker);
            self.deactivate_cleanup_block(depth, d);
            self.f.fir.erase(d);
        }
    }

    /// `CleanupDeactivationScope::ForceDeactivate`: deactivates the
    /// cleanups deferred since `mark` (a length of `f.cl.deferred`),
    /// innermost first, and drops their placeholders.
    pub(crate) fn deactivate_deferred(&mut self, mark: usize) {
        let deferred: Vec<_> = self.f.cl.deferred.drain(mark..).collect();
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
        let eq = ast.types.element(q).expect("caller checked: array type");
        let n_elems = ast.types.array_size(q).unwrap_or(0);
        let (inits, filler, _) = agg_init_parts(ast, e);
        let esz = self.size_align(eq).0;
        if self.array_init_from_constant(e, q, eq, inits.len() as u64 * esz, dest) {
            return Ok(());
        }
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
            let mark = self.f.fir.mark();
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
            let p = IrregularPartialArray {
                begin,
                saved: false,
                end_slot: slot,
                outer: ety,
                depth,
                elem,
                record,
            };
            self.push_full_expr_cleanup(Cleanup::IrregularPartialArray(Box::new(p)), false, true);
            self.add_aux_allocas(mark);
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
            let lv = LV {
                addr: Addr { v: el, ty: ety, align: ealign },
                q: eq,
                bf: None,
                vidx: None,
                tb: Tb::Type,
            };
            self.init_to_lvalue(init, lv, zeroed)?;
        }
        let trivial = match filler {
            None => true,
            Some(f) => self.is_trivial_filler(f),
        };
        if inits.len() as u64 != n_elems && !(zeroed && trivial && self.zero_init(eq)) {
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
            let entry = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
            let body = self.f.fir.create_block();
            self.emit_block(body, false);
            let cur = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(el, entry)] }, LTy::PTR);
            let lv = LV {
                addr: Addr { v: cur, ty: ety, align: ealign },
                q: eq,
                bf: None,
                vidx: None,
                tb: Tb::Type,
            };
            // [class.temporary]/5: the temporaries of each element's
            // initialization are destroyed before the next one starts.
            // The array's zeroed state reaches each element (clang
            // `EmitInitializationToLValue` with `Dest.isZeroed()`), so a
            // zeroed array gets no per-element `memset` or zero stores.
            let sc = self.begin_scope(false);
            let r = match filler {
                Some(f) => self.init_to_lvalue(f, lv, zeroed),
                None => self.null_init_lvalue(lv, zeroed),
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
            let here = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
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

    /// clang `EmitArrayInit` (WP-1008, spec/codegen.md "Constant
    /// emission of initializers"): when the explicit elements take more
    /// than 16 bytes, the elements are trivially copyable and the list
    /// folded ([`crate::fold`]), the array is copied from a private
    /// `constinit` global holding its value (clang
    /// `ConstantEmitter::tryEmitForInitializer`, `EmitFinalDestCopy`).
    /// Whether it was.
    fn array_init_from_constant(
        &mut self,
        e: ExprId,
        q: QualType,
        eq: QualType,
        explicit_bytes: u64,
        dest: Addr,
    ) -> bool {
        if explicit_bytes <= 16 || !crate::class::is_trivially_copyable(self.ast, eq) {
            return false;
        }
        let Some(c) = self.folds.get(e) else { return false };
        let Ok(k) = self.const_value(c, q, Some(e), true) else { return false };
        let (size, align) = self.size_align(q);
        let g = self.private_const("constinit", k, align);
        // A `new GlobalVariable` without `unnamed_addr`.
        self.m.global_mut(g).unnamed_addr = kiln_llvm::UnnamedAddr::None;
        let ty = self.mty(q);
        self.agg_copy(q, dest, Addr { v: V::C(gval(g)), ty, align }, size, false);
        true
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
        self.emit_binding_vars(d)
    }

    /// A condition variable ([stmt.pre]/4): only the variable itself. A
    /// decomposition's tuple-like bindings are initialized after the
    /// condition is evaluated (P0963; clang `EmitBranchOnBoolExpr` with
    /// its `ConditionalDecl`), by [`Self::emit_binding_vars`]; whether
    /// that is still to do is returned.
    pub(crate) fn emit_cond_var_decl(&mut self, d: DeclId) -> R<bool> {
        self.emit_one_var_decl(d)?;
        Ok(self.ast.decls.decomp_bindings(d).is_some())
    }

    /// The holding variables of decomposition `d`'s tuple-like bindings
    /// (clang `MaybeEmitDeferredVarDeclInit`).
    pub(crate) fn emit_binding_vars(&mut self, d: DeclId) -> R<()> {
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
        // A consteval-only variable (`constexpr std::meta::info r`, an
        // expansion statement's range and iterators over `info`s) exists
        // only for constant evaluation: sema made it `constexpr` and every
        // use of it constant ([basic.types.general], P2996R13), so it has
        // no storage (spec/codegen.md "Consteval-only variables").
        if let Some(q) = ast.decls.get(d).ty()
            && self.is_consteval_only(q)
        {
            return Ok(());
        }
        match v.spec.storage() {
            _ if is_static_local(ast, d) => {
                let g = self.static_local(d)?;
                self.static_local_init(d, g)?;
                self.di_global_var(d, g);
                return Ok(());
            }
            VarStorage::Extern => return Ok(()),
            _ => {}
        }
        let q = ast.decls.get(d).ty().expect("Sema: a variable has a type");
        if self.is_vla(q) {
            let init = v.init.filter(|&i| !ast.is_trivial_default_construction(i));
            self.emit_vla_var(d, q, init)?;
            self.push_cleanup_attr(d);
            return Ok(());
        }
        // clang `EmitAutoVarAlloca`: `EmitVariablyModifiedType` (a pointer
        // or reference to a variable length array).
        self.emit_vla_bounds(q)?;
        let ty = self.mty(q);
        let dtor_class = self.var_destructed_class(d, q);
        // NRVO: the variable lives in the return slot (clang
        // `EmitAutoVarAlloca`), with a flag telling its cleanup whether
        // the return happened.
        let nrvo_slot =
            self.f.ret_slot.filter(|_| v.spec.is_nrvo() && ast.types.as_record(q).is_some());
        let mut lifetime = None;
        let (addr, flag) = if let Some(slot) = nrvo_slot {
            let addr = Addr { v: slot.v, ty, align: slot.align };
            let flag = if dtor_class.is_some() {
                let f = self.f.fir.alloca(LTy::I1, 1);
                // clang `EmitAutoVarAlloca` calls `EnsureInsertPoint`
                // before the flag store: the variable may follow a
                // `noreturn` call or a `return` ([stmt.return]/1,
                // [class.copy.elision]/1).
                self.ensure_insert_point();
                let fls = V::C(self.m.const_bool(false));
                self.store(fls, f, 1, false);
                self.f.nrvo_flags.insert(d, f);
                Some(f)
            } else {
                None
            };
            (addr, flag)
        } else {
            let align = self.var_align(d);
            let a = self.f.fir.alloca(ty, align);
            lifetime = Some(a);
            (Addr { v: a, ty, align }, None)
        };
        self.f.locals.insert(d, addr);
        if let Some(a) = lifetime {
            self.var_lifetime(d, a, ty);
        }
        if self.dbg_on() {
            let ret_ptr = self.f.fir.dbg.as_deref().and_then(|x| x.ret_ptr);
            let slot = match ret_ptr {
                Some(p) if nrvo_slot.is_some() => VarSlot { addr: p, arg: None, deref: true },
                _ => VarSlot { addr: addr.v, arg: None, deref: false },
            };
            self.di_declare_var(d, slot);
        }
        // Default-initialization by a trivial constructor emits nothing
        // (but a non-trivial destructor still gets its cleanup).
        let init = v.init.filter(|&i| {
            !ast.is_trivial_default_construction(i) || ast.exprs.construct_info(i).zeroing
        });
        let dl = self.dbg_apply_default_artificial(ast.decls.get(d).loc());
        let r = if let Some(init) = init
            && self.f.fir.cur().is_some()
        {
            self.emit_var_init(d, q, addr, init)
        } else {
            if init.is_none() {
                // [basic.indet]/1: the erroneous value (spec/codegen.md §7r).
                self.auto_var_init(d, addr);
            }
            Ok(())
        };
        self.dbg_restore(dl);
        r?;
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
        self.push_cleanup_attr(d);
        Ok(())
    }

    /// clang `EmitAutoVarInit` for variable `d` of type `q` at `addr`.
    pub(crate) fn emit_var_init(
        &mut self,
        d: DeclId,
        q: QualType,
        addr: Addr,
        init: ExprId,
    ) -> R<()> {
        let ast = self.ast;
        if self.referee(q).is_some() {
            // A reference binds the address of its initializer glvalue.
            // clang `EmitAutoVarInit`: a reference might be usable in
            // constant expressions, so a constant initializer is stored as
            // its constant (`ConstantLValueEmitter`, a byte-offset GEP)
            // rather than re-evaluated (WP-189).
            let k = match self.folds.get(init) {
                Some(c) => self.const_value(c, q, Some(init), true).ok(),
                None => None,
            };
            let p = if let Some(k) = k {
                V::C(k)
            } else {
                self.auto_init_prefill(d, q, addr, init);
                self.emit_ref_binding(init)?
            };
            let slot = self.scalar_tls_addr(addr.v);
            let st = self.store(p, slot, addr.align, false);
            self.tbaa_mark(st, q, Tb::Type);
            return Ok(());
        }
        if ast.types.as_atomic(q).is_some() {
            // clang `EmitExprAsInit` -> `EmitAtomicInit` (WP-165).
            self.auto_init_prefill(d, q, addr, init);
            return self.atomic_init(init, LV { addr, q, bf: None, vidx: None, tb: Tb::Type });
        }
        let fold = self.folds.get(init);
        if self.is_aggregate(q) {
            // clang `IsConstantAggregate`: constexpr, or POD with a
            // constant initializer.
            let constexpr = ast.decls.var(d).is_some_and(|v| v.spec.is_constexpr());
            let constant_init = || self.is_pod(q) && !self.empty_copy_of_nonconstant(init);
            if let Some(c) = fold.filter(|_| constexpr || constant_init()) {
                // A constant the lowering cannot spell yet (bit-fields,
                // base subobjects) is initialized as written instead,
                // which has the same effect for a local, its padding
                // filled first (clang stores the constant with it).
                if let Ok(k) = self.const_value(c, q, Some(init), true) {
                    let k = self.auto_init_constant(d, k);
                    return self.stores_for_constant(d, addr, k, self.is_volatile(q));
                }
            }
            self.auto_init_prefill(d, q, addr, init);
            return self.emit_agg_into(init, addr, false);
        }
        if self.is_complex(q) {
            // clang `EmitAutoVarInit`: a constexpr variable stores its
            // constant pair whole, otherwise `EmitComplexExprIntoLValue`.
            let constexpr = ast.decls.var(d).is_some_and(|v| v.spec.is_constexpr());
            if let Some(c) = fold.filter(|_| constexpr) {
                let k = self.const_value(c, q, Some(init), true)?;
                let vol = self.is_volatile(q);
                self.store(V::C(k), addr.v, addr.align, vol);
                return Ok(());
            }
            self.auto_init_prefill(d, q, addr, init);
            let v = self.emit_complex(init)?;
            self.cx_store(v, addr, q);
            return Ok(());
        }
        // clang `mightBeUsableInConstantExpressions`: a const scalar with a
        // constant initializer is stored as that constant, unfilled.
        if fold.is_none()
            || !self.ast.types.effective_quals(q).is_const()
            || self.m.types.vector_info(addr.ty).is_some()
        {
            self.auto_init_prefill(d, q, addr, init);
        }
        let v = match fold {
            Some(c) => V::C(self.const_scalar_value(c, q)?),
            None => self.emit_scalar(init)?,
        };
        self.store_scalar(v, addr, q);
        Ok(())
    }

    pub(crate) fn const_elems(&mut self, k: Value) -> Option<Vec<Value>> {
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
    pub(crate) fn const_is_null(&mut self, k: Value) -> bool {
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
            // `isBytewiseValue` takes `half`, `float` and `double` only.
            ConstData::Float { ty, bits } if matches!(ty, LTy::HALF | LTy::FLOAT | LTy::DOUBLE) => {
                let w = self.m.types.float_width(ty)?;
                let b = (bits & 0xff) as u8;
                (0..w / 8).all(|i| ((bits >> (8 * i)) & 0xff) as u8 == b).then_some(Some(b))
            }
            ConstData::Float { .. } => None,
            ConstData::Cast { op: CastOp::IntToPtr, val, .. } => self.bytewise(val),
            _ => {
                let es = match self.m.const_of(k)?.clone() {
                    ConstData::Vector { elems, .. } => elems.to_vec(),
                    _ => self.const_elems(k)?,
                };
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

    /// clang `Expr::isConstantInitializer` on a trivial copy or move of an
    /// empty class asks whether the source is constant. The copy reads
    /// nothing ([class.copy.ctor]/14), so it folds whatever the source is,
    /// and the fold does not answer that question: `E e2 = std::move(e)`
    /// of a non-constexpr `e` is copied as written. The source counts as
    /// constant only when it names a constexpr variable.
    fn empty_copy_of_nonconstant(&self, init: ExprId) -> bool {
        let ast = self.ast;
        let mut e = init;
        while matches!(
            ast.exprs.get(e).kind(),
            ExprKind::ExprWithCleanups | ExprKind::CXXBindTemporaryExpr | ExprKind::ParenExpr
        ) {
            e = self.sub(e);
        }
        if ast.exprs.get(e).kind() != ExprKind::CXXConstructExpr {
            return false;
        }
        let ctor = ast.exprs.constructor(e);
        let trivial = ast.decls.function(ctor).is_some_and(|f| f.spec.is_trivial());
        let empty =
            ast.decls.get(ctor).ctx().is_some_and(|rd| {
                self.class_bits(rd).contains(kiln_ast::decl::DefinitionBits::EMPTY)
            });
        let mut args = ast.lists.iter::<ExprId>(ast.exprs.construct_args(e));
        let (Some(src), true, true) = (args.next(), trivial, empty) else {
            return false;
        };
        let mut src = self.ignore_paren_base_casts(src);
        // `std::move(ce)` and its kin are the lvalue of their argument
        // (`is_std_builtin`); clang evaluates through them.
        while let Some(a) = self.std_builtin_call_arg(src) {
            src = self.ignore_paren_base_casts(a);
        }
        !(ast.exprs.get(src).kind() == ExprKind::DeclRefExpr
            && ast.decls.var(ast.exprs.decl(src)).is_some_and(|v| v.spec.is_constexpr()))
    }

    /// The argument of `e` when `e` is a call of a standard function that
    /// folds to its argument's lvalue (`std::move`, `forward`, `as_const`,
    /// ...; [`Cg::is_std_builtin`]).
    fn std_builtin_call_arg(&self, e: ExprId) -> Option<ExprId> {
        let ast = self.ast;
        if ast.exprs.get(e).kind() != ExprKind::CallExpr {
            return None;
        }
        let mut c = ast.exprs.callee(e);
        loop {
            match ast.exprs.get(c).kind() {
                ExprKind::ParenExpr => c = ast.exprs.operand(c),
                ExprKind::ImplicitCastExpr
                    if matches!(
                        ast.exprs.cast_kind(c),
                        CastKind::FunctionToPointerDecay | CastKind::BuiltinFnToFnPtr
                    ) =>
                {
                    c = ast.exprs.operand(c);
                }
                ExprKind::DeclRefExpr => break,
                _ => return None,
            }
        }
        let d = ast.exprs.decl(c);
        let mut args = ast.lists.iter::<ExprId>(ast.exprs.call_args(e));
        let a = args.next()?;
        // `addressof` returns a pointer: not an lvalue of the argument.
        let ret = ast.types.proto(ast.decls.get(d).ty()?)?.ret();
        (args.next().is_none() && self.is_std_builtin(d) && ast.types.is_reference(ret))
            .then_some(a)
    }

    pub(crate) fn stores_for_constant(
        &mut self,
        d: DeclId,
        at: Addr,
        k: Value,
        vol: bool,
    ) -> R<()> {
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
    ast.default_arg_expr(e)
}

/// The elements, array filler and initialized union member of an
/// aggregate initialization: an `InitListExpr` or a
/// `CXXParenListInitExpr` (clang `VisitCXXParenListOrInitListExpr`).
pub(crate) fn agg_init_parts(
    ast: &kiln_ast::Ast,
    e: ExprId,
) -> (Vec<ExprId>, Option<ExprId>, Option<DeclId>) {
    if ast.exprs.get(e).kind() == ExprKind::CXXParenListInitExpr {
        let inits = ast.lists.iter(ast.exprs.paren_list_inits(e)).collect();
        let x = &ast.exprs;
        return (inits, x.paren_list_filler(e), x.paren_list_union_field(e));
    }
    let inits = ast.lists.iter(ast.exprs.init_list_inits(e)).collect();
    let l = ast.exprs.init_list(e);
    (inits, l.array_filler, l.union_field)
}

/// The operators clang's `EmitPromoted` keeps in the promoted type.
fn is_promotable_op(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div)
}
