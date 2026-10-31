//! `dynamic_cast` and `typeid` (spec/codegen.md §7.6, WP-080): clang
//! `CodeGenFunction::EmitDynamicCast` / `EmitCXXTypeidExpr` with the
//! Itanium `emitDynamicCastCall`, `emitDynamicCastToVoid`, `EmitTypeid`,
//! `EmitBadCastCall` and `EmitBadTypeidCall`.

use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, TypeidOperand, UnaryOp};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_llvm::{
    Attr, AttrKind, AttrList, CallOpts, FuncId, IntPredicate, MemoryEffects, ModRef, TypeId as LTy,
    Value,
};

use crate::fir::{Fir, Op, V};
use crate::module::{Cg, R, err};

impl Cg<'_> {
    /// The runtime function `name` (`ret(params)`) with function
    /// attributes `fa` (clang `CreateRuntimeFunction`).
    fn runtime_fn(&mut self, name: &str, ret: LTy, params: &[LTy], fa: &[Attr]) -> R<FuncId> {
        if let Some(f) = self.m.get_function(name) {
            return Ok(f);
        }
        let ty = self.m.types.function(ret, params, false);
        let f = match self.m.add_function(name, ty) {
            Ok(f) => f,
            Err(e) => return err(None, format!("cannot declare `{name}`: {e:?}")),
        };
        let a = self.m.attr_set(fa);
        self.m.func_mut(f).attrs.func = a;
        Ok(f)
    }

    /// A call of runtime function `f` with call-site function attribute
    /// `site` (`NoUnwind` for clang `EmitNounwindRuntimeCall`, `NoReturn`
    /// for `setDoesNotReturn`).
    fn runtime_call(&mut self, f: FuncId, ret: LTy, args: Vec<V>, site: AttrKind) -> V {
        let fn_ty = self.m.func(f).ty();
        let fa = self.m.attr_set(&[Attr::Flag(site)]);
        let opts =
            CallOpts { attrs: AttrList { func: fa, ..AttrList::default() }, ..CallOpts::default() };
        let c = self.f.fir.push(
            Op::Call { fn_ty, callee: V::C(Value::func(f)), args, opts: Box::new(opts) },
            ret,
        );
        self.f.calls.push((Fir::last_inst(c), Some(f), site == AttrKind::NoUnwind));
        c
    }

    /// `call void @name() noreturn; unreachable` (`__cxa_bad_cast`,
    /// `__cxa_bad_typeid`); leaves no insertion point.
    fn emit_noreturn_runtime_call(&mut self, name: &str) -> R<()> {
        let f = self.runtime_fn(name, LTy::VOID, &[], &[])?;
        self.runtime_call(f, LTy::VOID, Vec::new(), AttrKind::NoReturn);
        self.f.fir.push(Op::Unreachable, LTy::VOID);
        self.f.fir.set_cur(None);
        Ok(())
    }

    /// The class of `q` (or of what pointer `q` points to).
    fn record_of(&self, q: QualType) -> Option<DeclId> {
        let ast = self.ast;
        let q = ast.types.pointee(q).unwrap_or(q);
        ast.types.as_record(q).map(|r| ast.first(r))
    }

    /// clang `CXXRecordDecl::isDerivedFrom` with all paths recorded
    /// (`CXXBasePaths(FindAmbiguities, RecordPaths)`): every path from
    /// `dst` down to a `src` base as (class, base, virtual, public) steps.
    fn base_paths(&self, dst: DeclId, src: DeclId) -> Vec<Vec<(DeclId, DeclId, bool, bool)>> {
        let mut out = Vec::new();
        let mut seen_virtual = Vec::new();
        let mut path = Vec::new();
        self.base_paths_rec(dst, src, &mut path, &mut seen_virtual, &mut out, 0);
        out
    }

    fn base_paths_rec(
        &self,
        class: DeclId,
        src: DeclId,
        path: &mut Vec<(DeclId, DeclId, bool, bool)>,
        seen_virtual: &mut Vec<DeclId>,
        out: &mut Vec<Vec<(DeclId, DeclId, bool, bool)>>,
        depth: usize,
    ) {
        let ast = self.ast;
        let Some(def) = self.class_def(class) else { return };
        if depth > 256 {
            return;
        }
        let tag = ast.decls.record(def).map(|r| r.tag);
        let bases: Vec<_> = ast.decls.bases(def, &ast.lists).copied().collect();
        for b in bases {
            let Some(bd) = ast.types.as_record(b.ty).map(|r| ast.first(r)) else { continue };
            let public = match b.access {
                kiln_ast::decl::Access::Public => true,
                kiln_ast::decl::Access::None => tag != Some(kiln_ast::decl::TagKind::Class),
                _ => false,
            };
            let mut visit = true;
            if b.is_virtual {
                visit = !seen_virtual.contains(&bd);
                seen_virtual.push(bd);
            }
            path.push((ast.first(class), bd, b.is_virtual, public));
            if bd == src {
                out.push(path.clone());
            } else if visit {
                self.base_paths_rec(bd, src, path, seen_virtual, out, depth + 1);
            }
            path.pop();
        }
    }

    /// clang `computeOffsetHint` (`src2dst_offset` of `__dynamic_cast`).
    fn offset_hint(&mut self, src: DeclId, dst: DeclId) -> i64 {
        let paths = self.base_paths(dst, src);
        if paths.is_empty() {
            return -2;
        }
        let mut public_paths = 0;
        let mut offset = 0i64;
        for p in &paths {
            if !p.iter().all(|s| s.3) {
                continue;
            }
            public_paths += 1;
            for &(class, base, virt, _) in p {
                if virt {
                    return -1;
                }
                if public_paths > 1 {
                    continue;
                }
                let o = self.base_offset(class, base).unwrap_or(0);
                offset += i64::try_from(o).unwrap_or(0);
            }
        }
        match public_paths {
            0 => -2,
            1 => offset,
            _ => -3,
        }
    }

    /// clang `CXXDynamicCastExpr::isAlwaysNull`.
    fn dynamic_cast_always_null(&self, src: DeclId, dst: Option<DeclId>) -> bool {
        let Some(dst) = dst else { return false };
        if self.effectively_final(src) {
            return true;
        }
        self.effectively_final(dst) && dst != src && self.base_paths(dst, src).is_empty()
    }

    /// clang `EmitDynamicCast` of the `CXXDynamicCastExpr` `e` (a `Dynamic`
    /// cast) applied to `this` (aligned `align`): the pointer result, or
    /// the address of the result of a cast to reference type.
    pub(crate) fn emit_dynamic_cast(&mut self, e: ExprId, this: V, align: u64) -> R<V> {
        let ast = self.ast;
        let loc = self.loc_of(e);
        let dq = ast.exprs.get(e).ty();
        let sq = ast.exprs.get(ast.exprs.operand(e)).ty();
        let is_ref = self.is_glvalue(e);
        let to_void = !is_ref && ast.types.pointee(dq).is_some_and(|p| ast.types.is_void(p));
        let Some(src) = self.record_of(sq) else {
            return err(loc, "dynamic_cast of a non-class operand");
        };
        let dst = if to_void { None } else { self.record_of(dq) };
        if !to_void && dst.is_none() {
            return err(loc, "dynamic_cast to a non-class type");
        }
        if self.dynamic_cast_always_null(src, dst) {
            if !is_ref {
                return Ok(V::C(self.m.const_null(LTy::PTR)));
            }
            self.emit_noreturn_runtime_call("__cxa_bad_cast")?;
            let b = self.f.fir.create_block();
            self.emit_block(b, false);
            return Ok(V::C(self.m.const_poison(LTy::PTR)));
        }
        let null_check = !is_ref;
        let end = self.f.fir.create_block();
        let mut null_blocks = None;
        if null_check {
            let null = self.f.fir.create_block();
            let notnull = self.f.fir.create_block();
            let z = V::C(self.m.const_null(LTy::PTR));
            let isnull = self.icmp(IntPredicate::Eq, this, z);
            self.cond_br(isnull, null, notnull);
            self.emit_block(notnull, false);
            null_blocks = Some(null);
        }
        let value = if let Some(dst) = dst.filter(|_| !to_void) {
            let st = self.class_type(src);
            let dt = self.class_type(dst);
            let src_rtti = self.rtti(st)?;
            let dst_rtti = self.rtti(dt)?;
            let hint = self.offset_hint(src, dst);
            let f = self.runtime_fn(
                "__dynamic_cast",
                LTy::PTR,
                &[LTy::PTR, LTy::PTR, LTy::PTR, LTy::I64],
                &[
                    Attr::Flag(AttrKind::NoUnwind),
                    Attr::Flag(AttrKind::WillReturn),
                    Attr::Memory(MemoryEffects::all(ModRef::Read)),
                ],
            )?;
            let h = self.i64c(i128::from(hint));
            let v = self.runtime_call(
                f,
                LTy::PTR,
                vec![this, V::C(src_rtti), V::C(dst_rtti), h],
                AttrKind::NoUnwind,
            );
            if is_ref {
                let bad = self.f.fir.create_block();
                let z = V::C(self.m.const_null(LTy::PTR));
                let isnull = self.icmp(IntPredicate::Eq, v, z);
                self.cond_br(isnull, bad, end);
                self.emit_block(bad, false);
                self.emit_noreturn_runtime_call("__cxa_bad_cast")?;
            }
            v
        } else {
            // emitDynamicCastToVoid: this + vtable[-2] (offset-to-top).
            let vt = self.load(LTy::PTR, this, align, false);
            let m2 = self.i64c(-2);
            let p = self.gep(LTy::I64, vt, vec![m2], true);
            let off = self.load(LTy::I64, p, 8, false);
            self.gep(LTy::I8, this, vec![off], true)
        };
        let from = self.f.fir.cur();
        let Some(null) = null_blocks else {
            self.emit_block(end, false);
            return Ok(value);
        };
        self.emit_branch(end);
        self.emit_block(null, false);
        self.emit_branch(end);
        self.emit_block(end, false);
        let Some(from) = from else { return Ok(value) };
        let z = V::C(self.m.const_null(LTy::PTR));
        Ok(self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(value, from), (z, null)] }, LTy::PTR))
    }

    /// clang `isGLValueFromPointerDeref`.
    fn glvalue_from_pointer_deref(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let n = *ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr => self.glvalue_from_pointer_deref(ast.exprs.operand(e)),
            k if k.is_cast() => {
                let s = ast.exprs.operand(e);
                self.is_glvalue(s) && self.glvalue_from_pointer_deref(s)
            }
            ExprKind::OpaqueValueExpr => {
                ast.exprs.opaque_source(e).is_some_and(|s| self.glvalue_from_pointer_deref(s))
            }
            ExprKind::BinaryOperator if ast.exprs.binary_op(e) == BinaryOp::Comma => {
                self.glvalue_from_pointer_deref(ast.exprs.operands(e)[1])
            }
            ExprKind::ConditionalOperator => {
                let [a, b] = ast.exprs.cond_arms(e);
                self.glvalue_from_pointer_deref(a) || self.glvalue_from_pointer_deref(b)
            }
            ExprKind::ArraySubscriptExpr => true,
            ExprKind::UnaryOperator => ast.exprs.unary_op(e) == UnaryOp::Deref,
            _ => false,
        }
    }

    /// clang `CXXTypeidExpr::isMostDerived`: the operand names a variable
    /// of class (not pointer or reference) type.
    fn typeid_most_derived(&self, mut e: ExprId) -> bool {
        let ast = self.ast;
        loop {
            let n = *ast.exprs.get(e);
            match n.kind() {
                ExprKind::ParenExpr => e = ast.exprs.operand(e),
                k if k.is_cast() && ast.exprs.cast_kind(e) == CastKind::NoOp => {
                    e = ast.exprs.operand(e);
                }
                ExprKind::DeclRefExpr => {
                    let d = ast.exprs.decl(e);
                    return ast.decls.get(d).ty().is_some_and(|q| {
                        !ast.types.is_pointer(q)
                            && !matches!(
                                ast.types.canonical_view(q),
                                TypeView::LValueRef(_) | TypeView::RValueRef(_)
                            )
                    });
                }
                _ => return false,
            }
        }
    }

    /// clang `EmitCXXTypeidExpr`: the address of the `std::type_info`.
    pub(crate) fn emit_typeid(&mut self, e: ExprId) -> R<V> {
        let ast = self.ast;
        let x = match ast.exprs.typeid_operand(e) {
            TypeidOperand::Type(t) => {
                let t = self.referee(t).unwrap_or(t);
                return Ok(V::C(self.rtti(t)?));
            }
            TypeidOperand::Expr(x) => x,
        };
        let xq = ast.exprs.get(x).ty();
        if !ast.exprs.typeid_potentially_evaluated(e) || self.typeid_most_derived(x) {
            return Ok(V::C(self.rtti(xq)?));
        }
        // EmitTypeidFromVTable.
        let lv = self.emit_lvalue(x)?;
        if self.glvalue_from_pointer_deref(x) {
            let bad = self.f.fir.create_block();
            let end = self.f.fir.create_block();
            let z = V::C(self.m.const_null(LTy::PTR));
            let isnull = self.icmp(IntPredicate::Eq, lv.addr.v, z);
            self.cond_br(isnull, bad, end);
            self.emit_block(bad, false);
            self.emit_noreturn_runtime_call("__cxa_bad_typeid")?;
            self.emit_block(end, false);
        }
        let vt = self.load(LTy::PTR, lv.addr.v, lv.addr.align, false);
        let m1 = self.i64c(-1);
        let p = self.gep(LTy::PTR, vt, vec![m1], true);
        Ok(self.load(LTy::PTR, p, 8, false))
    }
}
