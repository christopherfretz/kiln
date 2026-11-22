//! GNU `_Complex` code generation (spec/codegen.md §3.12): clang 19's
//! `ComplexExprEmitter` at `-O0` with the full complex range
//! (`CX_Full`: Annex G multiplication with the NaN check and library
//! call, library division).
//!
//! A complex value is the pair `(real, imag)` of element values; in
//! memory it is the IR struct `{ T, T }`. A part is `None` when the
//! consumer ignores it (clang's `IgnoreReal`/`IgnoreImag`) or when the
//! operand of an arithmetic operator is real.

use kiln_abi::call::ArgKind;
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, UnaryOp};
use kiln_ast::{ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_llvm::{BinOp, CastOp, Flags, FloatPredicate, IntPredicate, TypeId as LTy, Value};
use kiln_source::{Loc, Span};

use crate::ComplexRange;
use crate::coverage::pgo::Key;
use crate::expr::{Callee, LV, Opaque};
use crate::fir::{Fir, Op, V};
use crate::func::{Addr, CallArg, min_align};
use crate::module::{Cg, R, err};
use crate::tbaa::Tb;

/// A complex value with possibly absent parts.
pub(crate) type Cx = (Option<V>, Option<V>);

/// Both parts wanted.
const ALL: (bool, bool) = (false, false);

/// The operands of a complex binary operator (clang `BinOpInfo`): `ty`
/// is the complex computation type.
struct Ops {
    lhs: Cx,
    rhs: Cx,
    ty: QualType,
}

impl Cg<'_> {
    /// The element type of `q` if it is `_Complex`.
    pub(crate) fn cx_elem(&self, q: QualType) -> Option<QualType> {
        self.ast.types.as_complex(q)
    }

    pub(crate) fn is_complex(&self, q: QualType) -> bool {
        self.cx_elem(q).is_some()
    }

    /// Whether `q` is `_Atomic(T)` with `T` complex.
    pub(crate) fn is_atomic_complex(&self, q: QualType) -> bool {
        self.ast.types.as_atomic(q).is_some_and(|v| self.is_complex(v))
    }

    /// clang `emitAddrOfRealComponent` / `emitAddrOfImagComponent`: the
    /// address of part `i` of the complex object of type `q` at `a`.
    pub(crate) fn cx_part_addr(&mut self, a: Addr, q: QualType, i: u32) -> Addr {
        let st = self.mty(q);
        let eq = self.cx_elem(q).expect("Sema typed it _Complex");
        let et = self.mty(eq);
        let v = self.struct_gep(st, a.v, i);
        let align = if i == 0 { a.align } else { min_align(a.align, self.alloc_size(et)) };
        Addr { v, ty: et, align }
    }

    /// clang `ComplexExprEmitter::EmitLoadOfLValue`: the wanted parts
    /// (both when volatile).
    /// An `_Atomic` complex object is loaded whole, atomically (clang
    /// `EmitLoadOfComplex` → `EmitAtomicLoad`, C17 6.2.6.1p9).
    pub(crate) fn cx_load(&mut self, a: Addr, q: QualType, ign: (bool, bool)) -> Cx {
        if let Some(vq) = self.ast.types.as_atomic(q) {
            let lv = LV { addr: a, q, bf: None, vidx: None, sel: None, tb: Tb::Type };
            let (r, i) = self.atomic_cx_load(lv, vq);
            return (Some(r), Some(i));
        }
        let vol = self.is_volatile(q);
        let re = (!ign.0 || vol).then(|| {
            let p = self.cx_part_addr(a, q, 0);
            self.load(p.ty, p.v, p.align, vol)
        });
        let im = (!ign.1 || vol).then(|| {
            let p = self.cx_part_addr(a, q, 1);
            self.load(p.ty, p.v, p.align, vol)
        });
        (re, im)
    }

    /// clang `EmitStoreOfComplex`.
    /// An `_Atomic` complex object is stored whole, atomically (clang
    /// `EmitStoreOfComplex` → `EmitAtomicStore`).
    pub(crate) fn cx_store(&mut self, (re, im): (V, V), a: Addr, q: QualType) {
        if let Some(vq) = self.ast.types.as_atomic(q) {
            let lv = LV { addr: a, q, bf: None, vidx: None, sel: None, tb: Tb::Type };
            self.atomic_cx_store((re, im), lv, vq);
            return;
        }
        let vol = self.is_volatile(q);
        let rp = self.cx_part_addr(a, q, 0);
        let ip = self.cx_part_addr(a, q, 1);
        self.store(re, rp.v, rp.align, vol);
        self.store(im, ip.v, ip.align, vol);
    }

    /// clang `EmitComplexExpr` with both parts wanted (a new
    /// `ComplexExprEmitter`, [`Self::cx_emitter`]).
    pub(crate) fn emit_complex(&mut self, e: ExprId) -> R<(V, V)> {
        self.cx_emitter(|cg| cg.visit_complex(e))
    }

    /// clang `ComplexExprEmitter::Visit` with both parts wanted, in the
    /// current emitter.
    fn visit_complex(&mut self, e: ExprId) -> R<(V, V)> {
        let c = self.emit_cx(e, ALL)?;
        self.both(c, e)
    }

    /// `f` in a new clang `ComplexExprEmitter`: its
    /// `FPHasBeenPromoted` starts clear, and the caller's comes back after
    /// (clang starts one in `EmitComplexExpr`, `EmitPromotedComplexExpr`,
    /// `EmitComplexAssignmentLValue` and
    /// `EmitComplexCompoundAssignmentLValue`).
    pub(crate) fn cx_emitter<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::replace(&mut self.f.cx_promoted, false);
        let r = f(self);
        self.f.cx_promoted = saved;
        r
    }

    fn both(&self, c: Cx, e: ExprId) -> R<(V, V)> {
        match c {
            (Some(r), Some(i)) => Ok((r, i)),
            _ => err(self.loc_of(e), "a complex value without both parts"),
        }
    }

    /// The zero pair of the complex type `q`.
    fn cx_zero(&mut self, q: QualType) -> (V, V) {
        let eq = self.cx_elem(q).expect("Sema typed it _Complex");
        let t = self.lty(eq);
        let z = V::C(self.m.const_zero(t));
        (z, z)
    }

    /// The parts of the folded complex constant `c` of type `q` (clang
    /// `tryEmitAsConstant`: `extractvalue` of the constant pair).
    fn cx_const(&mut self, c: kiln_ast::ConstValueId, q: QualType) -> R<(V, V)> {
        let k = self.const_value(c, q, None, true)?;
        match self.const_elems(k).as_deref() {
            Some(&[r, i]) => Ok((V::C(r), V::C(i))),
            _ => Ok(self.cx_zero(q)),
        }
    }

    /// clang `ComplexExprEmitter::Visit` with the ignore flags `ign`.
    pub(crate) fn emit_cx(&mut self, e: ExprId, ign: (bool, bool)) -> R<Cx> {
        let dl = self.dbg_apply_expr(e);
        self.di_explicit_cast(e);
        let r = self.emit_cx_x(e, ign);
        self.dbg_restore(dl);
        r
    }

    fn emit_cx_x(&mut self, e: ExprId, ign: (bool, bool)) -> R<Cx> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        let q = n.ty();
        match n.kind() {
            ExprKind::ParenExpr => self.emit_cx(ast.exprs.operand(e), ign),
            ExprKind::ChooseExpr | ExprKind::GenericSelectionExpr
                if ast.exprs.selected_operand(e).is_some() =>
            {
                self.emit_cx(ast.exprs.selected_operand(e).expect("match guard checked it"), ign)
            }
            // clang `VisitExprWithCleanups`: the parts survive a shared
            // cleanup block (`ForceCleanup({&Vals.first, &Vals.second})`;
            // a `co_await` of an awaiter returning `_Complex`).
            ExprKind::ExprWithCleanups => {
                let sc = self.begin_scope(false);
                let mut r = self.emit_cx(self.sub(e), ign);
                match &mut r {
                    Ok((Some(re), Some(im))) => self.end_scope_reload(sc, &mut [re, im]),
                    Ok((Some(v), None) | (None, Some(v))) => self.end_scope_reload(sc, &mut [v]),
                    _ => self.end_scope(sc),
                }
                r
            }
            // A glvalue's value is the address it designates (WP-640).
            ExprKind::ConstantExpr if self.is_glvalue(e) && self.immediate_value(e).is_some() => {
                let lv = self.emit_lvalue(e)?;
                Ok(self.cx_load(lv.addr, lv.q, ign))
            }
            ExprKind::ConstantExpr => match self.immediate_value(e) {
                Some(c) => self.cx_const(c, q).map(|(r, i)| (Some(r), Some(i))),
                None => self.emit_cx(ast.exprs.operand(e), ign),
            },
            // clang `ComplexExprEmitter::VisitCompoundLiteralExpr`.
            ExprKind::CompoundLiteralExpr => {
                let lv = self.compound_literal_lvalue(e)?;
                Ok(self.cx_load(lv.addr, lv.q, ign))
            }
            ExprKind::CXXDefaultArgExpr => match ast.default_arg_expr(e) {
                Some(d) => {
                    let saved = self.enter_call_default_arg(e);
                    let r = self.emit_cx(d, ign);
                    self.exit_call_default_arg(saved);
                    r
                }
                None => err(self.loc_of(e), "default argument not yet parsed"),
            },
            ExprKind::CXXDefaultInitExpr => {
                let Some(init) = self.default_member_init(e) else {
                    return err(self.loc_of(e), "default member initializer not yet parsed");
                };
                let saved = self.enter_default_init(e);
                let r = self.emit_cx(init, ign);
                self.exit_default_init(saved);
                r
            }
            ExprKind::SubstNonTypeTemplateParmExpr => self.emit_cx(ExprId::new(n.b()), ign),
            ExprKind::ImplicitCastExpr
            | ExprKind::CStyleCastExpr
            | ExprKind::CXXFunctionalCastExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::CXXConstCastExpr
            | ExprKind::CXXReinterpretCastExpr
            | ExprKind::CXXDynamicCastExpr
            | ExprKind::BuiltinBitCastExpr => self.emit_cx_cast(e, ign),
            ExprKind::UnaryOperator => self.emit_cx_unary(e, ign),
            ExprKind::BinaryOperator => self.emit_cx_binary(e, ign),
            ExprKind::CompoundAssignOperator => {
                let (lv, v) = self.cx_compound_assign(e)?;
                if self.is_volatile(lv.q) { Ok(self.cx_load(lv.addr, lv.q, ALL)) } else { Ok(v) }
            }
            ExprKind::ConditionalOperator => {
                let [a, b] = ast.exprs.cond_arms(e);
                self.emit_cx_conditional(e, self.sub(e), a, b)
            }
            ExprKind::CallExpr | ExprKind::CXXOperatorCallExpr | ExprKind::CXXMemberCallExpr
                if !self.is_glvalue(e) =>
            {
                self.f.cx_ret = None;
                if n.kind() == ExprKind::CXXMemberCallExpr {
                    self.emit_member_call(e, None)?;
                } else {
                    self.emit_call(e, None)?;
                }
                match self.f.cx_ret.take() {
                    Some((r, i)) => Ok((Some(r), Some(i))),
                    None => err(self.loc_of(e), "a complex call without a complex result"),
                }
            }
            // clang `ComplexExprEmitter::VisitCoawaitExpr` /
            // `VisitCoyieldExpr`: the `await_resume()` call's value
            // ([expr.await]/5.4, an `await_resume` returning `_Complex`).
            ExprKind::CoawaitExpr | ExprKind::CoyieldExpr => {
                match self.emit_coawait(e, crate::coro::Want::Complex)? {
                    crate::coro::Got::Cx(r, i) => Ok((Some(r), Some(i))),
                    _ => unreachable!("emit_coawait(Want::Complex) yields Got::Cx"),
                }
            }
            // clang `ComplexExprEmitter::VisitAtomicExpr`: the result
            // temporary's parts (`__c11_atomic_load` of an `_Atomic
            // _Complex`).
            ExprKind::AtomicExpr => {
                self.f.cx_ret = None;
                self.emit_atomic_expr(e, None)?;
                match self.f.cx_ret.take() {
                    Some((r, i)) => Ok((Some(r), Some(i))),
                    None => unreachable!("emit_atomic_expr leaves a complex result in cx_ret"),
                }
            }
            ExprKind::StmtExpr => {
                self.f.cx_ret = None;
                self.emit_stmt_expr(e)?;
                match self.f.cx_ret.take() {
                    Some((r, i)) => Ok((Some(r), Some(i))),
                    None => err(self.loc_of(e), "a complex statement expression without a value"),
                }
            }
            ExprKind::InitListExpr => {
                let inits: Vec<u32> = ast.lists.get(ast.exprs.init_list_inits(e)).to_vec();
                match inits[..] {
                    [r, i] => {
                        let r = self.emit_scalar(ExprId::new(r))?;
                        let i = self.emit_scalar(ExprId::new(i))?;
                        Ok((Some(r), Some(i)))
                    }
                    [x] => self.emit_cx(ExprId::new(x), ALL),
                    _ => {
                        let (r, i) = self.cx_zero(q);
                        Ok((Some(r), Some(i)))
                    }
                }
            }
            ExprKind::ImaginaryLiteral => {
                // clang `VisitImaginaryLiteral`: a null real part.
                let i = self.emit_scalar(ast.exprs.operand(e))?;
                let (r, _) = self.cx_zero(q);
                Ok((Some(r), Some(i)))
            }
            ExprKind::ImplicitValueInitExpr | ExprKind::CXXScalarValueInitExpr => {
                let (r, i) = self.cx_zero(q);
                Ok((Some(r), Some(i)))
            }
            // clang `ComplexExprEmitter::VisitOpaqueValueExpr`: the bound
            // lvalue, loaded, or the bound pair (GNU `c ?: d`).
            ExprKind::OpaqueValueExpr => match self.opaque_of(e) {
                Some(Opaque::Lv(lv)) => Ok(self.cx_load(lv.addr, lv.q, ign)),
                Some(Opaque::Cx(r, i)) => Ok((Some(r), Some(i))),
                Some(Opaque::Rv(_)) => {
                    unreachable!("bind_opaque_source binds a complex prvalue as Opaque::Cx")
                }
                None => err(self.loc_of(e), "unbound OpaqueValueExpr"),
            },
            // GNU `x ?: y` of complex type (clang
            // `VisitAbstractConditionalOperator`): the common operand is
            // evaluated once, then each arm in its own block.
            ExprKind::BinaryConditionalOperator => {
                let [common, c, a, b] = self.bind_common(e)?;
                let r = self.emit_cx_conditional(e, c, a, b);
                self.f.opaque.remove(&common);
                r
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                self.emit_cx(ast.exprs.rewritten_semantic(e), ign)
            }
            ExprKind::PseudoObjectExpr => {
                let r = self.bind_pseudo(e)?;
                let v = self.emit_cx(r, ign);
                self.unbind_pseudo(e);
                v
            }
            ExprKind::VAArgExpr => {
                // clang 19 `VisitVAArgExpr`: the target walk's address,
                // loaded as a whole (`EmitLoadOfAnyValue`), whatever is
                // ignored.
                let Some(a) = self.va_arg_addr(e)? else {
                    return err(self.loc_of(e), "va_arg of an empty complex type");
                };
                Ok(self.cx_load(a, q, ALL))
            }
            _ if self.is_glvalue(e) => {
                let lv = self.emit_lvalue(e)?;
                Ok(self.cx_load(lv.addr, lv.q, ign))
            }
            k @ (ExprKind::IntegerLiteral
            | ExprKind::FloatingLiteral
            | ExprKind::CharacterLiteral
            | ExprKind::StringLiteral
            | ExprKind::CXXBoolLiteralExpr
            | ExprKind::CXXNullPtrLiteralExpr
            | ExprKind::UserDefinedLiteral
            | ExprKind::EmbedExpr
            | ExprKind::GNUNullExpr
            | ExprKind::DeclRefExpr
            | ExprKind::MemberExpr
            | ExprKind::CXXThisExpr
            | ExprKind::PredefinedExpr
            | ExprKind::SourceLocExpr
            | ExprKind::UnresolvedLookupExpr
            | ExprKind::UnresolvedMemberExpr
            | ExprKind::DependentScopeDeclRefExpr
            | ExprKind::CXXDependentScopeMemberExpr
            | ExprKind::SubstNonTypeTemplateParmPackExpr
            | ExprKind::FunctionParmPackExpr
            | ExprKind::ArraySubscriptExpr
            | ExprKind::CallExpr
            | ExprKind::CXXMemberCallExpr
            | ExprKind::CXXOperatorCallExpr
            | ExprKind::ParenListExpr
            | ExprKind::DesignatedInitExpr
            | ExprKind::CXXConstructExpr
            | ExprKind::CXXTemporaryObjectExpr
            | ExprKind::CXXInheritedCtorInitExpr
            | ExprKind::CXXParenListInitExpr
            | ExprKind::ArrayInitLoopExpr
            | ExprKind::ArrayInitIndexExpr
            | ExprKind::CXXStdInitializerListExpr
            | ExprKind::NoInitExpr
            | ExprKind::DesignatedInitUpdateExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::CXXBindTemporaryExpr
            | ExprKind::CXXNewExpr
            | ExprKind::CXXDeleteExpr
            | ExprKind::CXXPseudoDestructorExpr
            | ExprKind::CXXThrowExpr
            | ExprKind::CXXNoexceptExpr
            | ExprKind::CXXTypeidExpr
            | ExprKind::UnaryExprOrTypeTraitExpr
            | ExprKind::TypeTraitExpr
            | ExprKind::ArrayTypeTraitExpr
            | ExprKind::ExpressionTraitExpr
            | ExprKind::OffsetOfExpr
            | ExprKind::SizeOfPackExpr
            | ExprKind::RequiresExpr
            | ExprKind::ConceptSpecializationExpr
            | ExprKind::LambdaExpr
            | ExprKind::PackExpansionExpr
            | ExprKind::CXXFoldExpr
            | ExprKind::PackIndexingExpr
            | ExprKind::CXXUnresolvedConstructExpr
            | ExprKind::DependentCoawaitExpr
            | ExprKind::AddrLabelExpr
            | ExprKind::ChooseExpr
            | ExprKind::ShuffleVectorExpr
            | ExprKind::ConvertVectorExpr
            | ExprKind::ExtVectorElementExpr
            | ExprKind::CXXReflectExpr
            | ExprKind::CXXSpliceExpr
            | ExprKind::RecoveryExpr
            | ExprKind::CXXExpansionSelectExpr
            | ExprKind::GenericSelectionExpr
            | ExprKind::IntegerPackExpr) => {
                // Never a complex prvalue (WP-1739): every glvalue takes
                // the arm above, a complex call/init list/literal/opaque
                // value has its own arm, and the rest are of another type
                // or dependent kinds and `RecoveryExpr` that never reach
                // codegen.
                unreachable!("complex {k:?} (WP-1739)")
            }
        }
    }

    /// clang `ComplexExprEmitter::EmitCast`.
    fn emit_cx_cast(&mut self, e: ExprId, ign: (bool, bool)) -> R<Cx> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        let s = ast.exprs.operand(e);
        let sq = ast.exprs.get(s).ty();
        match ast.exprs.cast_kind(e) {
            CastKind::LValueToRValue => {
                if let Some(c) = self.folds.get(e) {
                    return self.cx_const(c, q).map(|(r, i)| (Some(r), Some(i)));
                }
                self.emit_cx(s, ign)
            }
            CastKind::NoOp
            | CastKind::AtomicToNonAtomic
            | CastKind::NonAtomicToAtomic
            | CastKind::UserDefinedConversion
            | CastKind::ConstructorConversion => self.emit_cx(s, ign),
            CastKind::LValueBitCast | CastKind::LValueToRValueBitCast => {
                let lv = self.emit_lvalue(s)?;
                let a = Addr { ty: self.mty(q), ..lv.addr };
                Ok(self.cx_load(a, q, ign))
            }
            CastKind::FloatingRealToComplex | CastKind::IntegralRealToComplex => {
                let v = self.emit_scalar(s)?;
                let eq = self.cx_elem(q).expect("Sema typed it _Complex");
                let v = self.conv(v, sq, eq);
                let t = self.f.fir.ty(&self.m, v);
                Ok((Some(v), Some(V::C(self.m.const_zero(t)))))
            }
            CastKind::FloatingComplexCast
            | CastKind::FloatingComplexToIntegralComplex
            | CastKind::IntegralComplexCast
            | CastKind::IntegralComplexToFloatingComplex => {
                let c = self.emit_cx(s, ign)?;
                self.cx_convert(c, sq, q, self.ub_expr_loc(e))
            }
            k @ (CastKind::Dependent
            | CastKind::BitCast
            | CastKind::ArrayToPointerDecay
            | CastKind::FunctionToPointerDecay
            | CastKind::BuiltinFnToFnPtr
            | CastKind::NullToPointer
            | CastKind::NullToMemberPointer
            | CastKind::IntegralToPointer
            | CastKind::PointerToIntegral
            | CastKind::PointerToBoolean
            | CastKind::IntegralCast
            | CastKind::IntegralToBoolean
            | CastKind::BooleanToSignedIntegral
            | CastKind::IntegralToFloating
            | CastKind::FloatingToIntegral
            | CastKind::FloatingToBoolean
            | CastKind::FloatingCast
            | CastKind::DerivedToBase
            | CastKind::UncheckedDerivedToBase
            | CastKind::BaseToDerived
            | CastKind::Dynamic
            | CastKind::DerivedToBaseMemberPointer
            | CastKind::BaseToDerivedMemberPointer
            | CastKind::MemberPointerToBoolean
            | CastKind::ReinterpretMemberPointer
            | CastKind::ToVoid
            | CastKind::ToUnion
            | CastKind::VectorSplat
            | CastKind::IntegralComplexToReal
            | CastKind::FloatingComplexToReal
            | CastKind::IntegralComplexToBoolean
            | CastKind::FloatingComplexToBoolean
            | CastKind::AddressSpaceConversion) => {
                // Never a complex result (WP-1739): these casts yield a
                // scalar, pointer, class, union, vector or `void`, so the
                // scalar, aggregate or ignored emitters lower them;
                // `Dependent` only in a template pattern.
                unreachable!("complex cast {k:?}: not of complex type (WP-1739)")
            }
        }
    }

    /// clang `EmitComplexToComplexCast`: each present part converted.
    /// `loc`: the float-cast-overflow check location.
    fn cx_convert(&mut self, (r, i): Cx, from: QualType, to: QualType, loc: Option<Loc>) -> R<Cx> {
        let fe = self.cx_elem(from).expect("Sema typed it _Complex");
        let te = self.cx_elem(to).expect("Sema typed it _Complex");
        self.cx_convert_elem((r, i), fe, te, loc)
    }

    /// [`Self::cx_convert`] between element types `fe` and `te`.
    fn cx_convert_elem(
        &mut self,
        (r, i): Cx,
        fe: QualType,
        te: QualType,
        loc: Option<Loc>,
    ) -> R<Cx> {
        let r = r.map(|v| self.conv_at(v, fe, te, loc)).transpose()?;
        let i = i.map(|v| self.conv_at(v, fe, te, loc)).transpose()?;
        Ok((r, i))
    }

    /// clang `VisitUnaryPlus`/`Minus`/`Not`/`Extension` and the
    /// increments.
    fn emit_cx_unary(&mut self, e: ExprId, ign: (bool, bool)) -> R<Cx> {
        let ast = self.ast;
        let s = ast.exprs.operand(e);
        let op = ast.exprs.unary_op(e);
        match op {
            UnaryOp::Extension => self.emit_cx(s, ign),
            UnaryOp::Plus | UnaryOp::Minus
                if let Some(pe) = self.cx_promotion(ast.exprs.get(s).ty(), false) =>
            {
                // clang `VisitUnaryPlus`/`VisitUnaryMinus` of an
                // excess-precision operand: emitted promoted, negated in
                // `float`, truncated once.
                let eq = self.cx_elem(ast.exprs.get(s).ty()).expect("Sema typed it _Complex");
                let (r, i) = self.cx_emitter(|cg| cg.emit_cx_promoted_expr(s, pe))?;
                let (r, i) =
                    if op == UnaryOp::Minus { (self.fneg(r), self.fneg(i)) } else { (r, i) };
                let t = self.lty(eq);
                Ok((Some(self.unpromote(r, t)), Some(self.unpromote(i, t))))
            }
            UnaryOp::Plus => self.emit_cx(s, ALL),
            UnaryOp::Minus | UnaryOp::Not => {
                let (r, i) = self.visit_complex(s)?;
                let neg = |cg: &mut Self, v: V| {
                    let t = cg.f.fir.ty(&cg.m, v);
                    if cg.is_float_ty(t) {
                        cg.fneg(v)
                    } else {
                        let z = cg.icn(t, 0);
                        cg.bin(BinOp::Sub, Flags::NONE, z, v)
                    }
                };
                let r = if op == UnaryOp::Minus { neg(self, r) } else { r };
                let i = neg(self, i);
                Ok((Some(r), Some(i)))
            }
            UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec => {
                let lv = self.emit_lvalue(s)?;
                let inc = matches!(op, UnaryOp::PreInc | UnaryOp::PostInc);
                let pre = matches!(op, UnaryOp::PreInc | UnaryOp::PreDec);
                let (old, new) = self.cx_inc_dec(lv, inc)?;
                let v = if pre { new } else { old };
                Ok((Some(v.0), Some(v.1)))
            }
            _ if self.is_glvalue(e) => {
                let lv = self.emit_lvalue(e)?;
                Ok(self.cx_load(lv.addr, lv.q, ign))
            }
            // Never a complex prvalue (WP-1739): `&` is a pointer, `!` an
            // `int`/`bool`, `__real__`/`__imag__` the element type, `*`
            // a glvalue (the arm above); sema never builds a built-in
            // unary `co_await`.
            op @ (UnaryOp::AddrOf
            | UnaryOp::Deref
            | UnaryOp::LNot
            | UnaryOp::Real
            | UnaryOp::Imag
            | UnaryOp::Coawait) => unreachable!("complex unary {op:?} (WP-1739)"),
        }
    }

    /// clang `EmitComplexPrePostIncDec`: the real part steps by one; the
    /// old and new values.
    pub(crate) fn cx_inc_dec(&mut self, lv: LV, inc: bool) -> R<((V, V), (V, V))> {
        let old = self.cx_load(lv.addr, lv.q, ALL);
        let (Some(r), Some(i)) = old else { unreachable!("cx_load(ALL) loads both parts") };
        let t = self.f.fir.ty(&self.m, r);
        let step = if inc { 1.0 } else { -1.0 };
        let next = if self.is_float_ty(t) {
            let one = self.fconst(t, step);
            self.bin(BinOp::FAdd, Flags::NONE, r, one)
        } else {
            let one = self.icn(t, if inc { 1 } else { -1 });
            self.bin(BinOp::Add, Flags::NONE, r, one)
        };
        self.cx_store((next, i), lv.addr, lv.q);
        Ok(((r, i), (next, i)))
    }

    fn emit_cx_binary(&mut self, e: ExprId, ign: (bool, bool)) -> R<Cx> {
        let ast = self.ast;
        let [l, r] = ast.exprs.operands(e);
        let op = ast.exprs.binary_op(e);
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                let ty = ast.exprs.get(e).ty();
                // clang 22 promotes a quotient by a complex divisor;
                // clang 19 every quotient (`IsDivOpCode`).
                let divisor = op == BinaryOp::Div
                    && (self.opts.clang_compat < 22 || self.is_complex(ast.exprs.get(r).ty()));
                if let Some(pe) = self.cx_promotion(ty, divisor) {
                    let eq = self.cx_elem(ty).expect("Sema typed it _Complex");
                    let (r, i) = self.emit_cx_promoted(e, pe)?;
                    let t = self.lty(eq);
                    return Ok((Some(self.unpromote(r, t)), Some(self.unpromote(i, t))));
                }
                let lhs = self.cx_operand(l)?;
                let rhs = self.cx_operand(r)?;
                let (r, i) = self.cx_arith(op, &Ops { lhs, rhs, ty })?;
                Ok((Some(r), Some(i)))
            }
            BinaryOp::Assign => {
                let (lv, v) = self.cx_assign(e)?;
                if self.is_volatile(lv.q) {
                    Ok(self.cx_load(lv.addr, lv.q, ALL))
                } else {
                    Ok((Some(v.0), Some(v.1)))
                }
            }
            BinaryOp::Comma => {
                self.emit_ignored(l)?;
                self.ensure_insert_point();
                self.emit_cx(r, ign)
            }
            _ if self.is_glvalue(e) => {
                let lv = self.emit_lvalue(e)?;
                Ok(self.cx_load(lv.addr, lv.q, ign))
            }
            op @ (BinaryOp::PtrMemD
            | BinaryOp::PtrMemI
            | BinaryOp::Rem
            | BinaryOp::Shl
            | BinaryOp::Shr
            | BinaryOp::Cmp
            | BinaryOp::LT
            | BinaryOp::GT
            | BinaryOp::LE
            | BinaryOp::GE
            | BinaryOp::EQ
            | BinaryOp::NE
            | BinaryOp::And
            | BinaryOp::Xor
            | BinaryOp::Or
            | BinaryOp::LAnd
            | BinaryOp::LOr
            | BinaryOp::MulAssign
            | BinaryOp::DivAssign
            | BinaryOp::RemAssign
            | BinaryOp::AddAssign
            | BinaryOp::SubAssign
            | BinaryOp::ShlAssign
            | BinaryOp::ShrAssign
            | BinaryOp::AndAssign
            | BinaryOp::XorAssign
            | BinaryOp::OrAssign) => {
                // Never a complex prvalue (WP-1739): `%`, shifts and the
                // bitwise operators are rejected for complex operands as
                // clang does ("invalid operands to binary expression",
                // GNU C too), comparisons and `&&`/`||` are `int`/`bool`,
                // `<=>` is rejected, `.*`/`->*` are glvalues (the arm
                // above), and compound assignments are
                // `CompoundAssignOperator`s.
                unreachable!("complex binary {op:?} (WP-1739)")
            }
        }
    }

    /// clang `EmitPromotedComplexOperand` without promotion: a complex
    /// operand's pair, or a real operand alone.
    fn cx_operand(&mut self, x: ExprId) -> R<Cx> {
        if self.is_complex(self.ast.exprs.get(x).ty()) {
            self.emit_cx(x, ALL)
        } else {
            Ok((Some(self.emit_scalar(x)?), None))
        }
    }

    /// clang `EmitBinAssignLValue`: the right side, then the left lvalue
    /// and the store.
    pub(crate) fn cx_assign(&mut self, e: ExprId) -> R<(LV, (V, V))> {
        let [l, r] = self.ast.exprs.operands(e);
        let v = self.visit_complex(r)?;
        let lv = self.emit_lvalue(l)?;
        self.cx_store(v, lv.addr, lv.q);
        Ok((lv, v))
    }

    /// clang `ComplexExprEmitter::EmitCompoundAssignLValue`: the right
    /// side first, then the left lvalue and its value converted to the
    /// (promoted) computation type; the result converted back and stored.
    /// A real left side yields `(value, None)`. clang 22 promotes a `/=`
    /// by a complex divisor under `-fcomplex-arithmetic=promoted`; clang
    /// 19 never range-promotes a compound assignment.
    pub(crate) fn cx_compound_assign(&mut self, e: ExprId) -> R<(LV, Cx)> {
        let ast = self.ast;
        let ca = *ast.exprs.compound_assign(e);
        let l = self.sub(e);
        let op = match ast.exprs.binary_op(e) {
            BinaryOp::MulAssign => BinaryOp::Mul,
            BinaryOp::DivAssign => BinaryOp::Div,
            BinaryOp::SubAssign => BinaryOp::Sub,
            _ => BinaryOp::Add,
        };
        let ty = ca.result_ty;
        let rq = ast.exprs.get(ca.rhs).ty();
        let divisor = op == BinaryOp::Div && self.opts.clang_compat >= 22 && self.is_complex(rq);
        // `PromotionTypeCR`'s element: the computation's.
        let ce = match self.cx_promotion(ty, divisor) {
            Some(pe) => pe,
            None => self.cx_elem(ty).expect("Sema: _Complex computation type"),
        };
        let op_ty = self.promoted_cx_type(ce, ty);
        let rhs = if ast.types.is_floating(rq) && !self.is_complex(rq) {
            match self.cx_promotion(rq, divisor) {
                Some(p) => (Some(self.emit_promoted_as(ca.rhs, p)?), None),
                None => (Some(self.emit_scalar(ca.rhs)?), None),
            }
        } else {
            match self.cx_promotion(rq, divisor) {
                Some(pe) => {
                    let (r, i) = self.cx_emitter(|cg| cg.emit_cx_promoted_expr(ca.rhs, pe))?;
                    (Some(r), Some(i))
                }
                None => self.emit_cx(ca.rhs, ALL)?,
            }
        };
        let lv = self.emit_lvalue(l)?;
        let lq = ast.types.as_atomic(lv.q).unwrap_or(lv.q);
        let le = self.cx_promotion(ca.lhs_ty, divisor).unwrap_or(ce);
        let loc = self.ub_expr_loc(e);
        let lhs = if let Some(lqe) = self.cx_elem(lq) {
            let v = self.cx_load(lv.addr, lv.q, ALL);
            self.cx_convert_elem(v, lqe, le, loc)?
        } else {
            let v = self.load_lv(lv);
            if ast.types.is_floating(lq) {
                (Some(self.conv(v, lq, le)), None)
            } else {
                let v = self.conv(v, lq, ce);
                let t = self.f.fir.ty(&self.m, v);
                (Some(v), Some(V::C(self.m.const_zero(t))))
            }
        };
        let res = self.cx_arith(op, &Ops { lhs, rhs, ty: op_ty })?;
        if let Some(lqe) = self.cx_elem(lq) {
            let (r, i) = self.cx_convert_elem((Some(res.0), Some(res.1)), ce, lqe, loc)?;
            let v = self.both((r, i), e)?;
            self.cx_store(v, lv.addr, lv.q);
            Ok((lv, (Some(v.0), Some(v.1))))
        } else {
            let v = self.cx_to_scalar_elem(res, ce, lq, loc)?;
            self.store_lv(v, lv, false);
            Ok((lv, (Some(v), None)))
        }
    }

    /// clang `EmitComplexToScalarConversion`: to `bool`, `re != 0 | im !=
    /// 0`; otherwise the converted real part.
    pub(crate) fn cx_to_scalar(
        &mut self,
        (r, i): (V, V),
        from: QualType,
        to: QualType,
        loc: Option<Loc>,
    ) -> R<V> {
        let fe = self.cx_elem(from).expect("Sema typed it _Complex");
        self.cx_to_scalar_elem((r, i), fe, to, loc)
    }

    /// [`Self::cx_to_scalar`] from a pair of element type `fe`.
    fn cx_to_scalar_elem(
        &mut self,
        (r, i): (V, V),
        fe: QualType,
        to: QualType,
        loc: Option<Loc>,
    ) -> R<V> {
        if self.is_bool(to) {
            let r = self.conv(r, fe, to);
            let i = self.conv(i, fe, to);
            return Ok(self.bin(BinOp::Or, Flags::NONE, r, i));
        }
        self.conv_at(r, fe, to, loc)
    }

    /// clang `getPromotionType`: the element type a complex `ty`'s
    /// arithmetic runs in (the promoted type itself for a real `ty`), or
    /// `None` when it is not promoted. A `divisor` (clang 22: a quotient
    /// by a complex value; clang 19: any quotient) of a floating element
    /// under `-fcomplex-arithmetic=promoted` without a `#pragma STDC
    /// CX_LIMITED_RANGE` override runs in the next wider type
    /// ([`Self::cx_higher_precision`]). Otherwise an excess-precision
    /// element (`__bf16`; `_Float16` on x86-64, `f16_excess`) runs in
    /// `float` (`__mulsc3`, `__divsc3`) and is truncated once at the end,
    /// as clang does for `_Complex _Float16` and GCC 14 for `_Complex
    /// __bf16` (spec/abi.md).
    fn cx_promotion(&mut self, ty: QualType, divisor: bool) -> Option<QualType> {
        let Some(eq) = self.cx_elem(ty) else {
            return self.f16_excess(ty).then_some(QualType::FLOAT);
        };
        if divisor
            && self.complex_range() == ComplexRange::Promoted
            && let Some(p) = self.ast.types.complex_division_promotion(eq)
        {
            return self.cx_higher_precision(p);
        }
        self.f16_excess(eq).then_some(QualType::FLOAT)
    }

    /// clang `HigherPrecisionTypeForComplexArithmetic` on
    /// [`kiln_ast::types::TypeCtx::complex_division_promotion`]'s
    /// verdict: a wider type that holds twice the element's exponent range
    /// sets the emitter's `FPHasBeenPromoted` (the quotient is then
    /// algebraic); none (`long double`, `__float128`) leaves the quotient
    /// unpromoted, to Smith's algorithm, and clang 19 warns
    /// `-Whigher-precision-fp` without a location.
    fn cx_higher_precision(&mut self, p: Result<QualType, QualType>) -> Option<QualType> {
        if let Ok(w) = p {
            self.f.cx_promoted = true;
            return Some(w);
        }
        if self.opts.clang_compat < 22 {
            let id = DiagId::cg_next_larger_fp_type_same_size;
            self.diags.push(Diag::new(id, Span::INVALID));
        }
        None
    }

    /// The `_Complex` type of the promoted element `pe` for
    /// [`Ops::ty`] (`orig` when it is not interned: its only readers
    /// are the library calls and integer quotients, which a range
    /// promotion never reaches).
    fn promoted_cx_type(&self, pe: QualType, orig: QualType) -> QualType {
        if self.cx_elem(orig) == Some(pe) {
            return orig;
        }
        self.ast.types.find_complex(pe).unwrap_or(orig)
    }

    /// clang `EmitPromoted` on a complex `+ - * /` whose arithmetic runs
    /// in the element type `pe`: both operands are emitted promoted
    /// (`EmitPromotedComplexOperand`).
    fn emit_cx_promoted(&mut self, e: ExprId, pe: QualType) -> R<(V, V)> {
        let ast = self.ast;
        let [l, r] = ast.exprs.operands(e);
        let op = ast.exprs.binary_op(e);
        let lhs = self.cx_operand_promoted(l, pe)?;
        let rhs = self.cx_operand_promoted(r, pe)?;
        let ty = self.promoted_cx_type(pe, ast.exprs.get(e).ty());
        self.cx_arith(op, &Ops { lhs, rhs, ty })
    }

    /// clang `EmitPromotedComplexOperand`: a complex operand in `pe`
    /// parts (`EmitPromotedComplexExpr`, a new emitter), or a real
    /// operand alone (`EmitPromotedScalarExpr`).
    fn cx_operand_promoted(&mut self, x: ExprId, pe: QualType) -> R<Cx> {
        if !self.is_complex(self.ast.exprs.get(x).ty()) {
            return Ok((Some(self.emit_promoted_as(x, pe)?), None));
        }
        let (r, i) = self.cx_emitter(|cg| cg.emit_cx_promoted_expr(x, pe))?;
        Ok((Some(r), Some(i)))
    }

    /// clang `ComplexExprEmitter::EmitPromoted` (in the caller's new
    /// emitter): `+ - * /` recurse in `pe`, unary `-`/`+` emit their
    /// operand promoted; anything else is visited and extended
    /// (`EmitPromotedValue`).
    fn emit_cx_promoted_expr(&mut self, x: ExprId, pe: QualType) -> R<(V, V)> {
        let ast = self.ast;
        let y = self.skip_parens(x);
        match ast.exprs.get(y).kind() {
            ExprKind::BinaryOperator
                if matches!(
                    ast.exprs.binary_op(y),
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
                ) =>
            {
                return self.emit_cx_promoted(y, pe);
            }
            ExprKind::UnaryOperator
                if matches!(ast.exprs.unary_op(y), UnaryOp::Minus | UnaryOp::Plus) =>
            {
                let s = ast.exprs.operand(y);
                let (r, i) = self.cx_emitter(|cg| cg.emit_cx_promoted_expr(s, pe))?;
                if ast.exprs.unary_op(y) == UnaryOp::Plus {
                    return Ok((r, i));
                }
                return Ok((self.fneg(r), self.fneg(i)));
            }
            _ => {}
        }
        let (r, i) = self.visit_complex(y)?;
        let t = self.lty(pe);
        Ok((self.cast_unscoped(CastOp::FPExt, r, t), self.cast_unscoped(CastOp::FPExt, i, t)))
    }

    /// clang `EmitBinAdd`/`Sub`/`Mul`/`Div`.
    fn cx_arith(&mut self, op: BinaryOp, o: &Ops) -> R<(V, V)> {
        let (Some(a), b) = o.lhs else { unreachable!("complex operand has a real part") };
        let (Some(c), d) = o.rhs else { unreachable!("complex operand has a real part") };
        let t = self.f.fir.ty(&self.m, a);
        let float = self.is_float_ty(t);
        let missing = || err(None, "integer complex arithmetic on a real operand");
        Ok(match op {
            BinaryOp::Add | BinaryOp::Sub => {
                let add = op == BinaryOp::Add;
                let (fop, iop) =
                    if add { (BinOp::FAdd, BinOp::Add) } else { (BinOp::FSub, BinOp::Sub) };
                if float {
                    let re = self.bin(fop, Flags::NONE, a, c);
                    let im = match (b, d) {
                        (Some(b), Some(d)) => self.bin(fop, Flags::NONE, b, d),
                        (Some(b), None) => b,
                        (None, Some(d)) => {
                            if add {
                                d
                            } else {
                                self.fneg(d)
                            }
                        }
                        (None, None) => return missing(),
                    };
                    (re, im)
                } else {
                    let (Some(b), Some(d)) = (b, d) else { return missing() };
                    let re = self.bin(iop, Flags::NONE, a, c);
                    (re, self.bin(iop, Flags::NONE, b, d))
                }
            }
            BinaryOp::Mul => self.cx_mul(o, a, b, c, d)?,
            _ => self.cx_div(o, a, b, c, d)?,
        })
    }

    /// clang `EmitBinMul`.
    fn cx_mul(&mut self, o: &Ops, a: V, b: Option<V>, c: V, d: Option<V>) -> R<(V, V)> {
        let t = self.f.fir.ty(&self.m, a);
        let fl = Flags::NONE;
        if !self.is_float_ty(t) {
            let (Some(b), Some(d)) = (b, d) else {
                return err(None, "integer complex arithmetic on a real operand");
            };
            let rl = self.bin(BinOp::Mul, fl, a, c);
            let rr = self.bin(BinOp::Mul, fl, b, d);
            let re = self.bin(BinOp::Sub, fl, rl, rr);
            let il = self.bin(BinOp::Mul, fl, b, c);
            let ir = self.bin(BinOp::Mul, fl, a, d);
            return Ok((re, self.bin(BinOp::Add, fl, il, ir)));
        }
        let (b, d) = match (b, d) {
            (Some(b), Some(d)) => (b, d),
            (b, d) => {
                let re = self.bin(BinOp::FMul, fl, a, c);
                let im = match (b, d) {
                    (Some(b), _) => self.bin(BinOp::FMul, fl, b, c),
                    (None, Some(d)) => self.bin(BinOp::FMul, fl, a, d),
                    (None, None) => return err(None, "complex product of two reals"),
                };
                return Ok((re, im));
            }
        };
        let ac = self.bin(BinOp::FMul, fl, a, c);
        let bd = self.bin(BinOp::FMul, fl, b, d);
        let ad = self.bin(BinOp::FMul, fl, a, d);
        let bc = self.bin(BinOp::FMul, fl, b, c);
        let re = self.bin(BinOp::FSub, fl, ac, bd);
        let im = self.bin(BinOp::FAdd, fl, ad, bc);
        if self.complex_range() != ComplexRange::Full {
            return Ok((re, im));
        }
        let cont = self.f.fir.create_block();
        let inan = self.f.fir.create_block();
        let lib_blk = self.f.fir.create_block();
        let orig = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
        let rnan = self.fcmp(FloatPredicate::Uno, re, re);
        self.unlikely_br(rnan, inan, cont)?;
        self.emit_block(inan, false);
        let inan_b = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
        let inanv = self.fcmp(FloatPredicate::Uno, im, im);
        self.unlikely_br(inanv, lib_blk, cont)?;
        self.emit_block(lib_blk, false);
        let name = match self.m.types.float_width(t) {
            Some(16) => "__mulhc3",
            Some(32) => "__mulsc3",
            Some(64) => "__muldc3",
            Some(80) => "__mulxc3",
            _ => "__multc3",
        };
        let (lr, li) = self.cx_libcall(name, [a, b, c, d], o.ty)?;
        let lib_end = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
        self.br(cont);
        self.emit_block(cont, false);
        let rp = self
            .f
            .fir
            .push(Op::Phi { ty: t, inc: vec![(re, orig), (re, inan_b), (lr, lib_end)] }, t);
        let ip = self
            .f
            .fir
            .push(Op::Phi { ty: t, inc: vec![(im, orig), (im, inan_b), (li, lib_end)] }, t);
        Ok((rp, ip))
    }

    /// The complex range in the current FP scope: `#pragma STDC
    /// CX_LIMITED_RANGE ON` is `basic`, `OFF` `full` (clang
    /// `ActOnPragmaCXLimitedRange`), else the command line's.
    fn complex_range(&self) -> ComplexRange {
        match self.f.fp_ovr.cx_limited {
            Some(true) => ComplexRange::Basic,
            Some(false) => ComplexRange::Full,
            None => self.opts.complex_range,
        }
    }

    /// A conditional branch with clang's unlikely branch weights
    /// (`MDBuilder::createUnlikelyBranchWeights`: 1 to 2^20 - 1).
    fn unlikely_br(&mut self, c: V, t: crate::fir::Blk, f: crate::fir::Blk) -> R<()> {
        let br = self.f.fir.push(Op::CondBr { c, t, f }, LTy::VOID);
        let s = self.m.md_string(b"branch_weights");
        let w1 = self.m.const_int(LTy::I32, 1);
        let w2 = self.m.const_int(LTy::I32, 1024 * 1024 - 1);
        let (Ok(m1), Ok(m2)) = (self.m.md_const(w1), self.m.md_const(w2)) else {
            return err(None, "bad branch weights");
        };
        let md = self.m.md_tuple(&[Some(s), Some(m1), Some(m2)]);
        let k = self.m.md_kind("prof");
        self.f.fir.add_md(Fir::last_inst(br), k, md);
        Ok(())
    }

    /// clang `EmitBinDiv`: a real divisor divides each part; otherwise
    /// by [`ComplexRange`] a library call (full), Smith's algorithm
    /// (improved) or the algebraic quotient (basic).
    fn cx_div(&mut self, o: &Ops, a: V, b: Option<V>, c: V, d: Option<V>) -> R<(V, V)> {
        let t = self.f.fir.ty(&self.m, a);
        let fl = Flags::NONE;
        if self.is_float_ty(t) {
            let Some(d) = d else {
                let Some(b) = b else { return err(None, "complex quotient of two reals") };
                let re = self.bin(BinOp::FDiv, fl, a, c);
                return Ok((re, self.bin(BinOp::FDiv, fl, b, c)));
            };
            let b = b.unwrap_or_else(|| V::C(self.m.const_zero(t)));
            match self.complex_range() {
                ComplexRange::Full => {}
                ComplexRange::Improved => return Ok(self.cx_div_smith(t, a, b, c, d)),
                ComplexRange::Basic => return Ok(self.cx_div_algebraic(true, a, b, c, d)),
                // clang `EmitBinDiv`: algebraic in the wider type, Smith's
                // algorithm when no wider type exists (or the quotient's
                // emitter never promoted).
                ComplexRange::Promoted if self.f.cx_promoted => {
                    return Ok(self.cx_div_algebraic(true, a, b, c, d));
                }
                ComplexRange::Promoted => return Ok(self.cx_div_smith(t, a, b, c, d)),
            }
            let name = match self.m.types.float_width(t) {
                Some(16) => "__divhc3",
                Some(32) => "__divsc3",
                Some(64) => "__divdc3",
                Some(80) => "__divxc3",
                _ => "__divtc3",
            };
            return self.cx_libcall(name, [a, b, c, d], o.ty);
        }
        let (Some(b), Some(d)) = (b, d) else {
            return err(None, "integer complex arithmetic on a real operand");
        };
        let eq = self.cx_elem(o.ty).expect("Sema typed it _Complex");
        Ok(self.cx_div_algebraic(self.is_signed(eq), a, b, c, d))
    }

    /// `(a + ib) / (c + id)` as `((ac + bd) + i(bc - ad)) / (cc + dd)`:
    /// clang `EmitAlgebraicDiv` on floats (`signed` ignored), the integer
    /// quotient of `EmitBinDiv` otherwise.
    fn cx_div_algebraic(&mut self, signed: bool, a: V, b: V, c: V, d: V) -> (V, V) {
        let float = {
            let t = self.f.fir.ty(&self.m, a);
            self.is_float_ty(t)
        };
        let (mul, add, sub, div) = if float {
            (BinOp::FMul, BinOp::FAdd, BinOp::FSub, BinOp::FDiv)
        } else {
            (BinOp::Mul, BinOp::Add, BinOp::Sub, if signed { BinOp::SDiv } else { BinOp::UDiv })
        };
        let fl = Flags::NONE;
        let t1 = self.bin(mul, fl, a, c);
        let t2 = self.bin(mul, fl, b, d);
        let t3 = self.bin(add, fl, t1, t2);
        let t4 = self.bin(mul, fl, c, c);
        let t5 = self.bin(mul, fl, d, d);
        let t6 = self.bin(add, fl, t4, t5);
        let t7 = self.bin(mul, fl, b, c);
        let t8 = self.bin(mul, fl, a, d);
        let t9 = self.bin(sub, fl, t7, t8);
        let re = self.bin(div, fl, t3, t6);
        (re, self.bin(div, fl, t9, t6))
    }

    /// clang `EmitRangeReductionDiv` (Smith's algorithm): branch on
    /// `|c| > |d|` (`fcmp ugt` of `llvm.fabs`), scale by `d/c` or `c/d`,
    /// and join the two quotients in a `phi`.
    fn cx_div_smith(&mut self, t: LTy, a: V, b: V, c: V, d: V) -> (V, V) {
        let fl = Flags::NONE;
        let cont = self.f.fir.create_block();
        let ge = self.f.fir.create_block();
        let lt = self.f.fir.create_block();
        let fabs = kiln_llvm::intrinsics::fabs(&mut self.m, t);
        let ac = self.intrinsic_call(fabs, vec![c]);
        let ad = self.intrinsic_call(fabs, vec![d]);
        let is_r = self.fcmp(FloatPredicate::Ugt, ac, ad);
        self.cond_br(is_r, ge, lt);
        self.emit_block(ge, false);
        let r = self.bin(BinOp::FDiv, fl, d, c);
        let rd = self.bin(BinOp::FMul, fl, r, d);
        let den = self.bin(BinOp::FAdd, fl, c, rd);
        let br = self.bin(BinOp::FMul, fl, b, r);
        let num = self.bin(BinOp::FAdd, fl, a, br);
        let tr = self.bin(BinOp::FDiv, fl, num, den);
        let ar = self.bin(BinOp::FMul, fl, a, r);
        let num = self.bin(BinOp::FSub, fl, b, ar);
        let ti = self.bin(BinOp::FDiv, fl, num, den);
        let ge_end = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
        self.br(cont);
        self.emit_block(lt, false);
        let r = self.bin(BinOp::FDiv, fl, c, d);
        let rc = self.bin(BinOp::FMul, fl, r, c);
        let den = self.bin(BinOp::FAdd, fl, d, rc);
        let ar = self.bin(BinOp::FMul, fl, a, r);
        let num = self.bin(BinOp::FAdd, fl, ar, b);
        let fr = self.bin(BinOp::FDiv, fl, num, den);
        let br = self.bin(BinOp::FMul, fl, b, r);
        let num = self.bin(BinOp::FSub, fl, br, a);
        let fi = self.bin(BinOp::FDiv, fl, num, den);
        let lt_end = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
        self.br(cont);
        self.emit_block(cont, false);
        let re = self.f.fir.push(Op::Phi { ty: t, inc: vec![(tr, ge_end), (fr, lt_end)] }, t);
        let im = self.f.fir.push(Op::Phi { ty: t, inc: vec![(ti, ge_end), (fi, lt_end)] }, t);
        (re, im)
    }

    /// clang `EmitComplexBinOpLibCall`: `name(a, b, c, d)` returning the
    /// complex computation type `cq`, declared without attributes and
    /// called `nounwind`.
    fn cx_libcall(&mut self, name: &str, ops: [V; 4], cq: QualType) -> R<(V, V)> {
        let eq = self.cx_elem(cq).expect("Sema typed it _Complex");
        let sig = self.libcall_sig(cq, &[eq; 4]);
        let f = match self.m.get_function(name) {
            Some(f) => f,
            None => match self.m.add_function(name, sig.fn_ty) {
                Ok(f) => f,
                Err(_) => return err(None, format!("cannot declare {name}")),
            },
        };
        let vals = ops.map(CallArg::Scalar);
        self.f.cx_ret = None;
        let callee = Callee::Value(V::C(Value::func(f)));
        self.call_vals_to(&sig, callee, Some(f), None, &vals, &[], None, &[], &[])?;
        match self.f.cx_ret.take() {
            Some(p) => Ok(p),
            None => err(None, format!("{name} returned no complex value")),
        }
    }

    /// clang `VisitAbstractConditionalOperator` on complex arms.
    fn emit_cx_conditional(&mut self, e: ExprId, c: ExprId, a: ExprId, b: ExprId) -> R<Cx> {
        let lb = self.f.fir.create_block();
        let rb = self.f.fir.create_block();
        let cont = self.f.fir.create_block();
        let start = self.f.fir.cur();
        self.branch_on_bool(c, lb, rb)?;
        self.emit_block(lb, false);
        self.prof_inc(Key::Expr(e));
        let x = self.cond_eval(start, |cg| cg.emit_cx_arm(a))?;
        let lb = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
        self.emit_branch(cont);
        self.emit_block(rb, false);
        let y = self.cond_eval(start, |cg| cg.emit_cx_arm(b))?;
        let rb = self.f.fir.cur().expect(crate::fir::OPEN_BLOCK);
        self.emit_block(cont, false);
        let (x, y) = match (x, y) {
            (Some(x), Some(y)) => (x, y),
            // A `throw` arm ([expr.cond]/2.1) leaves no value: the result
            // is the other arm's, as for a scalar conditional.
            (Some(v), None) | (None, Some(v)) => return Ok((Some(v.0), Some(v.1))),
            (None, None) => unreachable!("Sema: a complex conditional has a complex arm"),
        };
        let t = self.f.fir.ty(&self.m, x.0);
        let r = self.f.fir.push(Op::Phi { ty: t, inc: vec![(x.0, lb), (y.0, rb)] }, t);
        let i = self.f.fir.push(Op::Phi { ty: t, inc: vec![(x.1, lb), (y.1, rb)] }, t);
        Ok((Some(r), Some(i)))
    }

    /// An arm of a complex conditional: its parts, or `None` for a
    /// `void` arm (a throw-expression, [expr.cond]/2.1), which is
    /// emitted for its effect. clang 22 rejects a throw arm here ("cannot
    /// compile this complex expression yet"); GCC accepts it.
    fn emit_cx_arm(&mut self, a: ExprId) -> R<Option<(V, V)>> {
        if self.ast.types.is_void(self.ast.exprs.get(a).ty()) {
            self.emit_ignored(a)?;
            return Ok(None);
        }
        self.visit_complex(a).map(Some)
    }

    /// The complex result of a call returning `rq` directly as `r`
    /// (clang `EmitCall`, `TEK_Complex`): `extractvalue` when the ABI type
    /// is the pair itself, else through a `coerce` temporary.
    pub(crate) fn cx_call_result(&mut self, sig_ret: &ArgKind, r: V, rq: QualType) -> (V, V) {
        let (ArgKind::Direct { coerce, .. } | ArgKind::Extend { coerce, .. }) = sig_ret.clone()
        else {
            unreachable!("complex return is Direct or Extend")
        };
        let ct = self.ir(&coerce);
        if ct == self.mty(rq) {
            let eq = self.cx_elem(rq).expect("Sema typed it _Complex");
            let et = self.lty(eq);
            let re = self.f.fir.push(Op::ExtractValue { agg: r, idx: 0 }, et);
            let im = self.f.fir.push(Op::ExtractValue { agg: r, idx: 1 }, et);
            return (re, im);
        }
        let t = self.temp(rq);
        let (size, _) = self.size_align(rq);
        self.coerced_store_sized(r, &coerce, t, size);
        match self.cx_load(t, rq, ALL) {
            (Some(re), Some(im)) => (re, im),
            _ => unreachable!("cx_load(ALL) loads both parts"),
        }
    }

    /// clang `ComplexExprEmitter`'s scalar projections: `__real__` and
    /// `__imag__` of a complex operand (`None` when `e`'s operand is not
    /// complex).
    pub(crate) fn emit_real_imag(&mut self, e: ExprId, imag: bool) -> R<Option<V>> {
        let s = self.ast.exprs.operand(e);
        if !self.is_complex(self.ast.exprs.get(s).ty()) {
            return Ok(None);
        }
        if self.is_glvalue(e) {
            let lv = self.emit_lvalue(e)?;
            return Ok(Some(self.load_lv(lv)));
        }
        let (r, i) = self.cx_emitter(|cg| cg.emit_cx(s, (imag, !imag)))?;
        let v = if imag { i } else { r };
        match v {
            Some(v) => Ok(Some(v)),
            None => err(self.loc_of(e), "a missing complex part"),
        }
    }

    /// clang `EmitLValue` of `__real__`/`__imag__` on a complex lvalue.
    pub(crate) fn real_imag_lvalue(&mut self, e: ExprId, imag: bool) -> R<LV> {
        let s = self.ast.exprs.operand(e);
        let lv = self.emit_lvalue(s)?;
        let q = self.ast.exprs.get(e).ty();
        if !self.is_complex(lv.q) {
            return Ok(lv);
        }
        let addr = self.cx_part_addr(lv.addr, lv.q, u32::from(imag));
        Ok(LV { addr, q, bf: None, vidx: None, sel: None, tb: Tb::Type })
    }

    /// clang `EmitCompare` on complex operands (`==` and `!=` only).
    pub(crate) fn cx_compare(&mut self, e: ExprId) -> R<V> {
        let ast = self.ast;
        let [l, r] = ast.exprs.operands(e);
        let ne = ast.exprs.binary_op(e) == BinaryOp::NE;
        let side = |cg: &mut Self, x: ExprId| -> R<(V, V, QualType)> {
            let xq = cg.ast.exprs.get(x).ty();
            if let Some(eq) = cg.cx_elem(xq) {
                let (a, b) = cg.emit_complex(x)?;
                Ok((a, b, eq))
            } else {
                let a = cg.emit_scalar(x)?;
                let t = cg.f.fir.ty(&cg.m, a);
                Ok((a, V::C(cg.m.const_zero(t)), xq))
            }
        };
        let (a, b, eq) = side(self, l)?;
        let (c, d, _) = side(self, r)?;
        let (cr, ci) = if ast.types.is_floating(eq) {
            let p = if ne { FloatPredicate::Une } else { FloatPredicate::Oeq };
            (self.fcmp(p, a, c), self.fcmp(p, b, d))
        } else {
            let p = if ne { IntPredicate::Ne } else { IntPredicate::Eq };
            (self.icmp(p, a, c), self.icmp(p, b, d))
        };
        let op = if ne { BinOp::Or } else { BinOp::And };
        Ok(self.bin(op, Flags::NONE, cr, ci))
    }

    /// clang `EmitDelegateCallArg`: parameter `q`'s value at `a`
    /// forwarded to the delegated-to function. A reference parameter is
    /// a plain `load ptr` of its slot with no `!nonnull` / `!align`
    /// (clang's `CreateLoad` here bypasses `EmitLoadOfReference`, at 19
    /// and 22 alike; spec/codegen.md "Reference loads").
    pub(crate) fn delegate_call_arg(&mut self, a: Addr, q: QualType) -> CallArg {
        if self.referee(q).is_some() {
            CallArg::Scalar(self.load(LTy::PTR, a.v, a.align, false))
        } else if self.is_aggregate(q) {
            self.forward_param_cleanup(a.v);
            CallArg::Agg(a)
        } else if self.is_complex(q) {
            match self.cx_load(a, q, ALL) {
                (Some(re), Some(im)) => CallArg::Complex(re, im),
                _ => unreachable!("cx_load(ALL) loads both parts"),
            }
        } else {
            CallArg::Scalar(self.load_scalar(a, q))
        }
    }

    /// clang `EmitReturnOfRValue` of a delegated call's complex result
    /// (left in `cx_ret` by the call) into the return slot, unless the
    /// call wrote the slot itself (`sret`).
    pub(crate) fn delegate_cx_return(&mut self, rq: QualType, sret: bool) {
        if let Some(p) = self.f.cx_ret.take()
            && !sret
            && let Some(slot) = self.f.ret_slot
            && self.is_complex(rq)
        {
            self.cx_store(p, slot, rq);
        }
    }
}
