//! Builtin function calls (spec/codegen.md §3, "Calls"): clang `EmitBuiltinExpr`
//! for the builtins kiln declares (`kiln_ast::decl::builtin`).
//!
//! A builtin is never emitted as a function. A call whose value folds to an
//! integer or floating constant is that constant (clang tries
//! `EvaluateAsRValue` before anything else; the fold prepass collects these
//! calls). The rest lower to LLVM intrinsics, inline IR, or a call to the
//! library function the builtin names (`__builtin_strlen` → `strlen`).

use kiln_ast::decl::builtin as id;
use kiln_ast::decl::{DeclKind, builtin::attr};
use kiln_ast::expr::{CastKind, ExprKind, UnaryOp};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_llvm::{
    Attr, AttrKind, AttrList, CallOpts, CastOp, FloatPredicate, FuncId, IntPredicate,
    MemoryEffects, ModRef, TypeId as LTy, Value,
};

use crate::fir::{Fir, Op, V};
use crate::func::{Addr, CallArg};
use crate::module::{Cg, R, err};

/// The builtin `e` calls directly, if it is a call of one: its declaration
/// and id.
pub(crate) fn builtin_callee(ast: &Ast, e: ExprId) -> Option<(DeclId, u32)> {
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
            ExprKind::DeclRefExpr => {
                let d = ast.exprs.decl(c);
                if ast.decls.get(d).kind() != DeclKind::Function {
                    return None;
                }
                return ast.decls.builtin_id(d).map(|i| (d, i));
            }
            _ => return None,
        }
    }
}

/// Whether the call `e` of a builtin is folded when it can be (clang
/// `EmitBuiltinExpr` keeps an `EvaluateAsRValue` result that is an integer
/// or a floating value).
pub(crate) fn is_fold_candidate(ast: &Ast, e: ExprId) -> bool {
    let q = ast.exprs.get(e).ty();
    // `__builtin_is_constant_evaluated()` is false outside a manifestly
    // constant-evaluated context, which a fold is not.
    builtin_callee(ast, e).is_some_and(|(_, b)| b != id::IS_CONSTANT_EVALUATED)
        && (ast.types.is_integral(q) || ast.types.is_floating(q))
}

/// The name of the library function a library builtin calls (clang
/// `getBuiltinLibFunction`), `None` for a builtin that is not one.
fn lib_name(b: u32) -> Option<&'static str> {
    let b = if b == id::CHAR_MEMCHR { id::MEMCHR } else { b };
    match b {
        id::MEMCMP
        | id::MEMCHR
        | id::STRLEN
        | id::WCSLEN
        | id::WMEMCHR
        | id::WMEMCMP
        | id::NAN
        | id::NANF
        | id::NANL
        | id::NANS
        | id::NANSF
        | id::NANSL
        | id::EXP
        | id::EXPF
        | id::EXPL
        | id::EXP2
        | id::EXP2F
        | id::EXP2L
        | id::EXPM1
        | id::EXPM1F
        | id::EXPM1L
        | id::LDEXP
        | id::LDEXPF
        | id::LDEXPL
        | id::POW
        | id::POWF
        | id::POWL
        | id::SCALBN
        | id::SCALBNF
        | id::SCALBNL
        | id::SCALBLN
        | id::SCALBLNF
        | id::SCALBLNL => id::info(b).and_then(|i| i.name.strip_prefix("__builtin_")),
        _ => None,
    }
}

/// The `llvm.is.fpclass` test of a classification builtin
/// (`llvm::FPClassTest`).
fn fp_class_test(b: u32) -> Option<i128> {
    Some(match b {
        id::ISNAN => 0x003,
        id::ISINF => 0x204,
        id::ISNORMAL => 0x108,
        id::ISFINITE => 0x1f8,
        _ => return None,
    })
}

/// The ordered (or unordered) comparison of an `isgreater`-family builtin.
fn fp_compare(b: u32) -> Option<FloatPredicate> {
    Some(match b {
        id::ISGREATER => FloatPredicate::Ogt,
        id::ISGREATEREQUAL => FloatPredicate::Oge,
        id::ISLESS => FloatPredicate::Olt,
        id::ISLESSEQUAL => FloatPredicate::Ole,
        id::ISLESSGREATER => FloatPredicate::One,
        id::ISUNORDERED => FloatPredicate::Uno,
        _ => return None,
    })
}

/// The extra function attributes of a library builtin's declaration and
/// call sites: `Pure` and `Const` (clang `ConstructAttributeList`).
fn lib_attrs(b: u32, out: &mut Vec<Attr>) {
    let bits = id::info(b).map_or(0, |i| i.attrs);
    let mem = if bits & attr::CONST != 0 {
        ModRef::None
    } else if bits & attr::PURE != 0 {
        ModRef::Read
    } else {
        return;
    };
    out.push(Attr::Flag(AttrKind::WillReturn));
    out.push(Attr::Memory(MemoryEffects::all(mem)));
}

/// Whether `q` is a complete object type (an incomplete class has no
/// natural alignment to speak of).
fn is_complete(ast: &Ast, q: QualType) -> bool {
    ast.types.as_record(q).is_none_or(|r| {
        ast.definition(r).is_some_and(|d| {
            ast.decls
                .record(d)
                .is_some_and(|x| x.defn.contains(kiln_ast::decl::DefinitionBits::COMPLETE))
        })
    })
}

impl Cg<'_> {
    /// clang `EmitBuiltinExpr` for the call `e` of builtin `b`: `Ok(None)`
    /// for a library builtin, which the caller emits as an ordinary call
    /// of [`Cg::lib_fn`].
    pub(crate) fn emit_builtin(&mut self, b: u32, e: ExprId) -> R<Option<Option<V>>> {
        let ast = self.ast;
        let q = ast.exprs.get(e).ty();
        if let Some(c) = self.folds.get(e)
            && (ast.types.is_integral(q) || ast.types.is_floating(q))
        {
            return Ok(Some(Some(V::C(self.const_scalar_value(c, q)?))));
        }
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        let arg = |i: usize| args.get(i).copied();
        let Some(a0) = arg(0).or(Some(e)) else { unreachable!() };
        let v = match b {
            id::IS_CONSTANT_EVALUATED => V::C(self.m.const_bool(false)),
            // [specialized.addressof]: `&x` without an overloaded `&`.
            id::ADDRESSOF => self.emit_lvalue(a0)?.addr.v,
            id::CLZ
            | id::CLZL
            | id::CLZLL
            | id::CLZG
            | id::CTZ
            | id::CTZL
            | id::CTZLL
            | id::CTZG => {
                let ctz = matches!(b, id::CTZ | id::CTZL | id::CTZLL | id::CTZG);
                let fallback = if matches!(b, id::CLZG | id::CTZG) { arg(1) } else { None };
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = if ctz {
                    kiln_llvm::intrinsics::cttz(&mut self.m, ty)
                } else {
                    kiln_llvm::intrinsics::ctlz(&mut self.m, ty)
                };
                // `TargetInfo::isCLZForZeroUndef`: false only on AArch64.
                let zero_undef = self.m.const_bool(fallback.is_some() || self.is_x86());
                let r = self.intrinsic_call(f, vec![x, V::C(zero_undef)]);
                let rt = self.lty(q);
                let r = self.int_cast(r, rt, false);
                match fallback {
                    None => r,
                    Some(fb) => {
                        let zero = V::C(self.m.const_int(ty, 0));
                        let is_zero = self.icmp(IntPredicate::Eq, x, zero);
                        let fv = self.emit_scalar(fb)?;
                        self.select(is_zero, fv, r)
                    }
                }
            }
            id::POPCOUNT | id::POPCOUNTL | id::POPCOUNTLL | id::POPCOUNTG => {
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::ctpop(&mut self.m, ty);
                let r = self.intrinsic_call(f, vec![x]);
                let rt = self.lty(q);
                self.int_cast(r, rt, false)
            }
            id::BSWAP16 | id::BSWAP32 | id::BSWAP64 => {
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::bswap(&mut self.m, ty);
                self.intrinsic_call(f, vec![x])
            }
            id::ISNAN | id::ISINF | id::ISNORMAL | id::ISFINITE => {
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let f = kiln_llvm::intrinsics::is_fpclass(&mut self.m, ty);
                let test = fp_class_test(b).unwrap_or_default();
                let t = V::C(self.m.const_int(LTy::I32, test));
                let r = self.intrinsic_call(f, vec![x, t]);
                let rt = self.lty(q);
                self.cast(CastOp::ZExt, r, rt)
            }
            id::SIGNBIT => {
                // clang `EmitSignBit`: the sign of the bits as an integer.
                let x = self.emit_scalar(a0)?;
                let ty = self.f.fir.ty(&self.m, x);
                let w = self.m.types.float_width(ty).unwrap_or(64);
                let it = self.m.types.int(w);
                let bits = self.cast(CastOp::BitCast, x, it);
                let zero = V::C(self.m.const_int(it, 0));
                let r = self.icmp(IntPredicate::Slt, bits, zero);
                let rt = self.lty(q);
                self.cast(CastOp::ZExt, r, rt)
            }
            id::ISGREATER
            | id::ISGREATEREQUAL
            | id::ISLESS
            | id::ISLESSEQUAL
            | id::ISLESSGREATER
            | id::ISUNORDERED => {
                let (Some(l), Some(r)) = (arg(0), arg(1)) else {
                    return err(self.loc_of(e), "comparison builtin without two arguments");
                };
                let l = self.emit_scalar(l)?;
                let r = self.emit_scalar(r)?;
                let p = fp_compare(b).unwrap_or(FloatPredicate::Uno);
                let c = self.fcmp(p, l, r);
                let rt = self.lty(q);
                self.cast(CastOp::ZExt, c, rt)
            }
            id::FREXP | id::FREXPF | id::FREXPL => {
                // clang `emitFrexpBuiltin`: `llvm.frexp`, the exponent
                // stored through the pointer at its natural alignment.
                let (Some(x), Some(p)) = (arg(0), arg(1)) else {
                    return err(self.loc_of(e), "frexp without two arguments");
                };
                let x = self.emit_scalar(x)?;
                let pq = ast.exprs.get(p).ty();
                let p = self.emit_scalar(p)?;
                let iq = ast.types.pointee(pq).unwrap_or(QualType::ERROR);
                let ty = self.f.fir.ty(&self.m, x);
                let it = self.lty(iq);
                let f = kiln_llvm::intrinsics::frexp(&mut self.m, ty, it);
                let rty = self.m.types.struct_literal(&[ty, it], false);
                let call = self.intrinsic_call_ty(f, vec![x], rty);
                let exp = self.f.fir.push(Op::ExtractValue { agg: call, idx: 1 }, it);
                let align = self.size_align(iq).1;
                let mt = self.mty(iq);
                self.store_scalar(exp, Addr { v: p, ty: mt, align }, iq);
                self.f.fir.push(Op::ExtractValue { agg: call, idx: 0 }, ty)
            }
            id::MEMCPY | id::MEMMOVE => {
                let (Some(d), Some(s), Some(n)) = (arg(0), arg(1), arg(2)) else {
                    return err(self.loc_of(e), "memory builtin without three arguments");
                };
                let (dv, da) = self.ptr_align(d)?;
                let (sv, sa) = self.ptr_align(s)?;
                let nv = self.emit_scalar(n)?;
                let f = if b == id::MEMCPY {
                    kiln_llvm::intrinsics::memcpy(&mut self.m, LTy::I64)
                } else {
                    kiln_llvm::intrinsics::memmove(&mut self.m, LTy::I64)
                };
                let fn_ty = self.m.func(f).ty();
                let vb = self.m.const_bool(false);
                let pa = self.m.attr_set(&[Attr::Align(da)]);
                let pb = self.m.attr_set(&[Attr::Align(sa)]);
                let none = self.m.attr_set(&[]);
                let opts = CallOpts {
                    attrs: AttrList { params: vec![pa, pb, none, none], ..AttrList::default() },
                    ..CallOpts::default()
                };
                let c = self.f.fir.push(
                    Op::Call {
                        fn_ty,
                        callee: V::C(Value::func(f)),
                        args: vec![dv, sv, nv, V::C(vb)],
                        opts: Box::new(opts),
                    },
                    LTy::VOID,
                );
                self.f.calls.push((Fir::last_inst(c), Some(f), true));
                dv
            }
            // [ptr.launder]: the pointer itself (no `llvm.launder.invariant.group`
            // without `-fstrict-vtable-pointers`).
            id::LAUNDER => self.emit_scalar(a0)?,
            id::OPERATOR_NEW | id::OPERATOR_DELETE => {
                return self.builtin_new_delete(b, e, &args).map(Some);
            }
            id::UNREACHABLE => {
                // clang `EmitUnreachable`, then an insertion point.
                self.f.fir.push(Op::Unreachable, LTy::VOID);
                self.f.fir.set_cur(None);
                let cont = self.f.fir.create_block();
                self.emit_block(cont, false);
                return Ok(Some(None));
            }
            _ if lib_name(b).is_some() => return Ok(None),
            _ => {
                let name = id::info(b).map_or("?", |i| i.name);
                return err(
                    self.loc_of(e),
                    format!("code generation for {name} is not supported yet"),
                );
            }
        };
        Ok(Some(Some(v)))
    }

    /// A call of intrinsic `f` with no call-site attributes.
    fn intrinsic_call(&mut self, f: FuncId, args: Vec<V>) -> V {
        let fty = self.m.func(f).ty();
        let rty = self.m.types.function_info(fty).map_or(LTy::VOID, |i| i.0);
        self.intrinsic_call_ty(f, args, rty)
    }

    fn intrinsic_call_ty(&mut self, f: FuncId, args: Vec<V>, rty: LTy) -> V {
        let fn_ty = self.m.func(f).ty();
        let r = self.f.fir.push(
            Op::Call { fn_ty, callee: V::C(Value::func(f)), args, opts: Box::default() },
            rty,
        );
        self.f.calls.push((Fir::last_inst(r), Some(f), true));
        r
    }

    /// The pointer value of `e` and the alignment it is known to have
    /// (clang `EmitPointerWithAlignment`).
    fn ptr_align(&mut self, e: ExprId) -> R<(V, u64)> {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        if n.kind() == ExprKind::ParenExpr {
            return self.ptr_align(ast.exprs.operand(e));
        }
        if n.kind().is_cast() {
            let s = ast.exprs.operand(e);
            match ast.exprs.cast_kind(e) {
                CastKind::NoOp | CastKind::BitCast => {
                    let sq = ast.exprs.get(s).ty();
                    if let Some(p) = ast.types.pointee(sq)
                        && ast.types.is_pointer(sq)
                        && !ast.types.is_void(p)
                    {
                        let (v, a) = self.ptr_align(s)?;
                        if n.kind() == ExprKind::ImplicitCastExpr || self.decl_aligned(s) {
                            return Ok((v, a));
                        }
                        // An explicit cast honors the target type's
                        // alignment unless the source is a declaration.
                        let a = self.pointee_align(n.ty());
                        return Ok((v, a));
                    }
                }
                CastKind::ArrayToPointerDecay => {
                    let lv = self.emit_lvalue(s)?;
                    let v = self.decay(lv.addr);
                    return Ok((v, lv.addr.align));
                }
                _ => {}
            }
        }
        if n.kind() == ExprKind::UnaryOperator && ast.exprs.unary_op(e) == UnaryOp::AddrOf {
            let lv = self.emit_lvalue(ast.exprs.operand(e))?;
            return Ok((lv.addr.v, lv.addr.align));
        }
        let v = self.emit_scalar(e)?;
        let a = self.pointee_align(n.ty());
        Ok((v, a))
    }

    /// Whether the pointer `e` (through implicit casts) has its alignment
    /// from a declaration (`AlignmentSource::Decl`).
    fn decl_aligned(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr => self.decl_aligned(ast.exprs.operand(e)),
            ExprKind::ImplicitCastExpr => match ast.exprs.cast_kind(e) {
                CastKind::NoOp | CastKind::BitCast => self.decl_aligned(ast.exprs.operand(e)),
                CastKind::ArrayToPointerDecay => self.lvalue_from_decl(ast.exprs.operand(e)),
                _ => false,
            },
            ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::AddrOf => {
                self.lvalue_from_decl(ast.exprs.operand(e))
            }
            _ => false,
        }
    }

    /// clang `getNaturalPointeeTypeAlignment`: an incomplete or `void`
    /// pointee has alignment 1.
    fn pointee_align(&mut self, pq: QualType) -> u64 {
        let ast = self.ast;
        match ast.types.pointee(pq) {
            Some(p)
                if !ast.types.is_void(p) && !ast.types.is_function(p) && is_complete(ast, p) =>
            {
                self.size_align(p).1
            }
            _ => 1,
        }
    }

    /// The library function a library builtin `d` calls (clang
    /// `getBuiltinLibFunction`): declared with the builtin's type and
    /// attributes under the library name.
    pub(crate) fn lib_fn(&mut self, d: DeclId, b: u32) -> R<(FuncId, Vec<Attr>)> {
        let ast = self.ast;
        let Some(name) = lib_name(b) else {
            return err(Some(ast.decls.get(d).loc()), "not a library builtin");
        };
        let mut extra = Vec::new();
        lib_attrs(b, &mut extra);
        if let Some(f) = self.m.get_function(name) {
            return Ok((f, extra));
        }
        let Some(fty) = ast.decls.get(d).ty() else {
            return err(None, "builtin without a type");
        };
        let sig = self.sig(fty, true);
        let f = match self.m.add_function(name, sig.fn_ty) {
            Ok(f) => f,
            Err(x) => {
                return err(Some(ast.decls.get(d).loc()), format!("cannot declare {name}: {x:?}"));
            }
        };
        let (ret, params) = self.sig_attrs(&sig, sig.params.len(), false);
        let mut fa = Vec::new();
        if sig.nothrow {
            fa.push(Attr::Flag(AttrKind::NoUnwind));
        }
        fa.extend(extra.iter().cloned());
        self.target_attrs(&mut fa);
        let fa = self.m.attr_set(&fa);
        let func = self.m.func_mut(f);
        func.attrs.func = fa;
        func.attrs.ret = ret;
        func.attrs.params = params;
        self.nounwind.insert(f, sig.nothrow);
        Ok((f, extra))
    }

    /// clang `EmitBuiltinNewDeleteCall`: a call of the global replaceable
    /// `operator new` / `operator delete` whose type sema gave the callee.
    fn builtin_new_delete(&mut self, b: u32, e: ExprId, args: &[ExprId]) -> R<Option<V>> {
        let ast = self.ast;
        let callee = ast.exprs.callee(e);
        let fty = ast.exprs.get(callee).ty();
        let fty = ast.types.pointee(fty).unwrap_or(fty);
        let op = if b == id::OPERATOR_DELETE {
            OverloadedOperator::Delete
        } else {
            OverloadedOperator::New
        };
        let canon = ast.types.canonical(fty);
        let found = ast.lookup_local(ast.tu, DeclName::operator(op)).iter().find(|&f| {
            ast.decls.get(f).kind() == DeclKind::Function
                && ast.decls.get(f).ty().is_some_and(|t| ast.types.canonical(t) == canon)
        });
        let Some(f) = found else {
            return err(self.loc_of(e), "no global allocation function for the builtin");
        };
        let params: Vec<QualType> =
            ast.types.proto(fty).map(|p| ast.types.params(p).to_vec()).unwrap_or_default();
        let mut vals = Vec::with_capacity(args.len());
        let mut tys = Vec::with_capacity(args.len());
        for (i, &a) in args.iter().enumerate() {
            let pq = params.get(i).copied().unwrap_or(ast.exprs.get(a).ty());
            tys.push(ast.exprs.get(a).ty());
            vals.push(if ast.types.is_reference(pq) {
                CallArg::Scalar(self.emit_lvalue(a)?.addr.v)
            } else {
                CallArg::Scalar(self.emit_scalar(a)?)
            });
        }
        let loc = self.loc_of(e);
        let r = self.emit_new_delete_call(f, &vals, &tys, loc)?;
        Ok((b == id::OPERATOR_NEW).then_some(r))
    }
}
