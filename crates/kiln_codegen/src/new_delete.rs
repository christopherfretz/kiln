//! `new` and `delete` expressions (spec/codegen.md §3.1): clang's
//! `EmitCXXNewExpr`, `EmitCXXNewAllocSize`, `EmitNewArrayInitializer`,
//! `EmitCXXDeleteExpr` and `EmitDeleteCall` at `-O0`, for non-class and
//! aggregate element types (no array cookies, constructors or
//! destructors).

use kiln_abi::mangle;
use kiln_ast::decl::{DeclKind, DefinitionBits, NamespaceData, TagKind};
use kiln_ast::expr::{CastKind, ExprFlags, ExprKind, NewExtra, UnaryOp};
use kiln_ast::names::OverloadedOperator;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_llvm::intrinsics::OverflowOp;
use kiln_llvm::{
    Attr, AttrKind, AttrList, AttrSetId, BinOp, CallOpts, Flags, IntPredicate, TypeId as LTy, Value,
};
use kiln_source::Loc;

use crate::cleanup::{ArrayDelete, Cleanup, Depth, IrregularPartialArray, NewDelete, Saved};
use crate::expr::LV;
use crate::fir::{Fir, Op, V};
use crate::func::{Addr, CallArg, min_align};
use crate::module::{Cg, R, err};
use crate::tbaa::Tb;
use crate::ubsan::{Tck, TypeCheck};

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

/// The irregular partial-array cleanup of an array new-initializer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct NewPartial {
    depth: Depth,
    /// clang's `dominatingIP` placeholder.
    dominator: V,
    /// `array.init.end`.
    slot: V,
    align: u64,
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
    pub(crate) fn in_std(&self, d: DeclId, name: &str) -> bool {
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

    /// The first attribute of kind `kind` on any declaration of `d`.
    pub(crate) fn fn_attr(
        &self,
        d: DeclId,
        kind: kiln_ast::attr::AttrKind,
    ) -> Option<kiln_ast::attr::AttrNode> {
        let ast = self.ast;
        ast.redecls_of(d).find_map(|r| {
            ast.lists
                .iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r))
                .map(|a| *ast.attrs.get(a))
                .find(|n| n.kind == kind)
        })
    }

    /// Function `d` carries GNU `malloc` (clang `RestrictAttr`) and
    /// returns a pointer: its result is `noalias`.
    pub(crate) fn fn_is_malloc(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let ptr_ret = ast
            .decls
            .get(d)
            .ty()
            .and_then(|t| ast.types.proto(t))
            .is_some_and(|p| ast.types.is_pointer(p.ret()));
        ptr_ret && self.fn_attr(d, kiln_ast::attr::AttrKind::MALLOC).is_some()
    }

    /// The allocation call-site attributes of a direct call of `d` with
    /// arguments `vals` (excluding `this`), added to `extra` (function)
    /// and `ret` (clang `ConstructAttributeList`): a replaceable global
    /// `operator new`'s implicit `alloc_size` and `returns_nonnull`, and
    /// `noalias` ("a sane operator new"); GNU `alloc_size` and `malloc`;
    /// and `alloc_align` with a constant power-of-two alignment, which
    /// raises the result's alignment (clang `AllocAlignAttrEmitter`; any
    /// other alignment is `alloc_align_assume` after the call).
    pub(crate) fn alloc_call_attrs(
        &mut self,
        d: DeclId,
        vals: &[CallArg],
        extra: &mut Vec<Attr>,
        ret: &mut Vec<Attr>,
    ) {
        if let Some(af) = self.alloc_fn(d).filter(|a| a.replaceable && a.is_new) {
            extra.push(Attr::AllocSize(0, None));
            ret.push(Attr::Flag(AttrKind::NoAlias));
            if !af.nothrow_t && !self.opts.null_pointer_is_valid {
                ret.push(Attr::Flag(AttrKind::NonNull));
            }
        } else if let Some(a) = self.fn_alloc_size(d) {
            extra.push(a);
        }
        if self.fn_is_malloc(d) && !ret.contains(&Attr::Flag(AttrKind::NoAlias)) {
            ret.push(Attr::Flag(AttrKind::NoAlias));
        }
        if let Some(i) = self.fn_alloc_align(d)
            && let Some(&CallArg::Scalar(v)) = vals.get(i)
            && let Some((_, n, _)) = self.cint(v)
            && n.is_power_of_two()
        {
            ret.push(Attr::Align(n as u64));
        }
        self.class_b_call_attrs(d, extra, ret);
        self.f.call_decl = Some(d);
    }

    /// The one-based parameter index an `alloc_size` / `alloc_align`
    /// argument word holds (an `IntegerLiteral`).
    fn attr_index(&self, raw: u32) -> Option<u32> {
        let e = <ExprId as kiln_ast::ids::RawId>::from_raw(raw)?;
        self.ast.exprs.int_value(e).and_then(|v| u32::try_from(v).ok())
    }

    /// Whether `d` returns a pointer (`refs`: or a reference).
    fn returns_pointer(&self, d: DeclId, refs: bool) -> bool {
        let t = &self.ast.types;
        self.ast
            .decls
            .get(d)
            .ty()
            .and_then(|q| t.proto(q))
            .is_some_and(|p| t.is_pointer(p.ret()) || (refs && t.is_reference(p.ret())))
    }

    /// The C library allocation functions clang recognizes as builtins
    /// (`extern "C"` at namespace scope, pointer result) and gives an
    /// implicit `alloc_size` and, for `aligned_alloc`, `alloc_align`
    /// (clang `Sema::AddKnownFunctionAttributes`): the zero-based
    /// `allocsize` indices and alignment parameter.
    fn lib_alloc(&self, d: DeclId) -> Option<(Attr, Option<usize>)> {
        let ast = self.ast;
        let t = &ast.types;
        let n = ast.decls.get(d);
        if n.kind() != DeclKind::Function
            || !n.ctx().is_some_and(|c| self.is_tu_level(c))
            || !mangle::is_extern_c(ast, ast.first(d))
        {
            return None;
        }
        let p = *t.proto(n.ty()?)?;
        if p.variadic() || !t.is_pointer(p.ret()) {
            return None;
        }
        let name = ast.symbols().as_str(n.name().as_ident()?);
        match (name, t.params(&p).len()) {
            ("malloc", 1) => Some((Attr::AllocSize(0, None), None)),
            ("calloc", 2) => Some((Attr::AllocSize(0, Some(1)), None)),
            ("realloc", 2) => Some((Attr::AllocSize(1, None), None)),
            ("aligned_alloc", 2) => Some((Attr::AllocSize(1, None), Some(0))),
            _ => None,
        }
    }

    /// GNU `alloc_size(n[, m])` on function `d` (or the implicit one of a
    /// C library allocation function), as the zero-based parameter
    /// indices of `allocsize` (clang `AllocSizeAttr`; the LLVM index
    /// counts `this`, as the one-based source index does).
    pub(crate) fn fn_alloc_size(&self, d: DeclId) -> Option<Attr> {
        if let Some(n) = self.fn_attr(d, kiln_ast::attr::AttrKind::ALLOC_SIZE) {
            if !self.returns_pointer(d, false) {
                return None;
            }
            let b = if n.b == 0 { None } else { Some(self.attr_index(n.b)?.checked_sub(1)?) };
            return Some(Attr::AllocSize(self.attr_index(n.a)?.checked_sub(1)?, b));
        }
        self.lib_alloc(d).map(|x| x.0)
    }

    /// The zero-based argument (excluding `this`) giving the alignment of
    /// `d`'s result: GNU `alloc_align(n)`, the implicit one of
    /// `aligned_alloc`, or the `std::align_val_t` of a
    /// replaceable `operator new` (clang `AllocAlignAttr`).
    pub(crate) fn fn_alloc_align(&self, d: DeclId) -> Option<usize> {
        if let Some(n) = self.fn_attr(d, kiln_ast::attr::AttrKind::ALLOC_ALIGN) {
            if !self.returns_pointer(d, true) {
                return None;
            }
            let f = self.ast.first(d);
            let this = u32::from(
                self.ast.decls.get(f).kind() == DeclKind::CXXMethod && !self.is_static_method(f),
            );
            return self.attr_index(n.a)?.checked_sub(1 + this).map(|i| i as usize);
        }
        if let Some((_, a)) = self.lib_alloc(d) {
            return a;
        }
        self.alloc_fn(d).filter(|a| a.replaceable && a.is_new).and_then(|a| a.align_param)
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
        let af = self.alloc_fn(d).filter(|a| a.replaceable);
        let malloc = self.fn_is_malloc(d);
        let size = self.fn_alloc_size(d);
        if af.is_none() && !malloc && size.is_none() {
            return (func, ret);
        }
        let mut f = self.m.attrs_of(func).to_vec();
        let mut r = self.m.attrs_of(ret).to_vec();
        if let Some(af) = af {
            f.push(Attr::Flag(AttrKind::NoBuiltin));
            if af.is_new {
                f.push(Attr::AllocSize(0, None));
                if !af.nothrow_t && !self.opts.null_pointer_is_valid {
                    r.push(Attr::Flag(AttrKind::NonNull));
                }
            }
        }
        // GNU `malloc` / `alloc_size` (clang `ConstructAttributeList`).
        if malloc {
            r.push(Attr::Flag(AttrKind::NoAlias));
        }
        if let Some(a) = size
            && !f.iter().any(|x| matches!(x, Attr::AllocSize(..)))
        {
            f.push(a);
        }
        (self.m.attr_set(&f), self.m.attr_set(&r))
    }

    /// Emits a call to an allocation or deallocation function (clang
    /// `EmitNewDeleteCall`): `vals` in parameter order.
    pub(crate) fn emit_new_delete_call(
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
        // alignment; anything else is an `llvm.assume` after the call.
        let mut ret_align = None;
        if af.replaceable
            && af.is_new
            && let Some(i) = af.align_param
            && let Some(&CallArg::Scalar(v)) = vals.get(i)
            && let Some((_, n, _)) = self.cint(v)
            && n.is_power_of_two()
        {
            ret_align = Some(n as u64);
        }
        let (ret, params) = self.sig_attrs(&sig, vals.len(), false);
        let mut fa = Vec::new();
        if af.replaceable {
            fa.push(Attr::Flag(AttrKind::Builtin));
        }
        if sig.nothrow {
            fa.push(Attr::Flag(AttrKind::NoUnwind));
        }
        self.call_size_attrs(Some(f), &mut fa);
        self.stmt_call_attrs(Some(d), &mut fa);
        let mut ra = self.m.attrs_of(ret).to_vec();
        if af.replaceable && af.is_new {
            fa.push(Attr::AllocSize(0, None));
            ra.push(Attr::Flag(AttrKind::NoAlias));
            if !af.nothrow_t && !self.opts.null_pointer_is_valid {
                ra.push(Attr::Flag(AttrKind::NonNull));
            }
        }
        if let Some(n) = ret_align {
            ra.push(Attr::Align(n));
        }
        let func = self.m.attr_set(&fa);
        let ret = self.m.attr_set(&ra);
        let opts = CallOpts {
            attrs: AttrList { func, ret, params },
            cc: sig.llvm_cc(),
            ..CallOpts::default()
        };
        // A class-specific allocation function is no global one: its
        // result type comes from its signature.
        let rty = self.m.types.function_info(sig.fn_ty).map_or(LTy::VOID, |(r, _, _)| r);
        let r = self.emit_call_op(
            sig.fn_ty,
            V::C(Value::func(f)),
            ir_args,
            opts,
            rty,
            Some(f),
            sig.nothrow,
        );
        if af.replaceable && af.is_new {
            self.alloc_align_assume(d, vals, r);
        }
        Ok(r)
    }

    /// clang `AllocAlignAttrEmitter::EmitAsAnAssumption`: after a call of
    /// `d` returning `r`, an `alloc_align` argument that is not a constant
    /// power of two becomes `llvm.assume(i1 true) ["align"(ptr r, i64 a)]`
    /// (the argument zero-extended to `i64`, clang `emitAlignmentAssumption`).
    pub(crate) fn alloc_align_assume(&mut self, d: DeclId, vals: &[CallArg], r: V) {
        self.assume_aligned_assume(d, r);
        let Some(i) = self.fn_alloc_align(d) else { return };
        let Some(&CallArg::Scalar(v)) = vals.get(i) else { return };
        if self.cint(v).is_some_and(|(_, n, _)| n.is_power_of_two()) || self.f.fir.cur().is_none() {
            return;
        }
        let a = self.int_cast(v, LTy::I64, false);
        let f = kiln_llvm::intrinsics::assume(&mut self.m);
        let fn_ty = self.m.func(f).ty();
        let cond = self.m.const_bool(true);
        self.f.fir.push(
            Op::AssumeBundle {
                fn_ty,
                callee: Value::func(f),
                cond,
                tag: "align",
                args: vec![r, a],
            },
            LTy::VOID,
        );
    }

    /// The lvalue `e` has clang's `AlignmentSource::Decl`: a named
    /// non-reference variable; a non-reference field of any base (clang
    /// `getFieldAlignmentSource` is always `Decl`, `->` included); an
    /// element of such an array; or `*p`/`p[i]` where the pointer's
    /// alignment is a declaration's (`EmitPointerWithAlignment`).
    pub(crate) fn lvalue_from_decl(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let e = self.skip_parens(e);
        let n = *ast.exprs.get(e);
        let non_ref = |d| ast.decls.get(d).ty().is_some_and(|q| !ast.types.is_reference(q));
        match n.kind() {
            ExprKind::DeclRefExpr => {
                let d = ast.exprs.decl(e);
                matches!(ast.decls.get(d).kind(), DeclKind::Var | DeclKind::ParmVar) && non_ref(d)
            }
            ExprKind::MemberExpr => {
                let d = ast.exprs.decl(e);
                matches!(ast.decls.get(d).kind(), DeclKind::Field | DeclKind::Var) && non_ref(d)
            }
            ExprKind::ArraySubscriptExpr => {
                let [l, r] = ast.exprs.operands(e);
                let base = if ast.types.is_pointer(ast.exprs.get(l).ty()) { l } else { r };
                self.pointer_from_decl(base)
            }
            ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::Deref => {
                self.pointer_from_decl(ast.exprs.operand(e))
            }
            _ => false,
        }
    }

    /// The alignment `pointer_with_alignment(e)` computes is a
    /// declaration's (its `LValueBaseInfo` source is `Decl`), without
    /// emitting anything.
    pub(crate) fn pointer_from_decl(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let e = self.skip_parens(e);
        let n = *ast.exprs.get(e);
        if n.kind().is_cast() {
            let s = ast.exprs.operand(e);
            let sq = ast.exprs.get(s).ty();
            match ast.exprs.cast_kind(e) {
                CastKind::NoOp | CastKind::BitCast
                    if ast.types.is_pointer(sq)
                        && ast.types.pointee(sq).is_some_and(|p| !ast.types.is_void(p)) =>
                {
                    return self.pointer_from_decl(s);
                }
                CastKind::ArrayToPointerDecay => return self.lvalue_from_decl(s),
                CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase
                    if ast.types.pointee(sq).and_then(|t| ast.types.as_record(t)).is_some() =>
                {
                    return self.pointer_from_decl(s);
                }
                _ => return false,
            }
        }
        match n.kind() {
            ExprKind::UnaryOperator if ast.exprs.unary_op(e) == UnaryOp::AddrOf => {
                self.lvalue_from_decl(ast.exprs.operand(e))
            }
            ExprKind::CallExpr => {
                self.addressof_call_arg(e).is_some_and(|s| self.lvalue_from_decl(s))
            }
            ExprKind::BinaryOperator => self.pointer_arith_operands(e).is_some_and(|(lp, _)| {
                let [l, r] = ast.exprs.operands(e);
                self.pointer_from_decl(if lp { l } else { r })
            }),
            _ => false,
        }
    }

    pub(crate) fn skip_parens(&self, mut e: ExprId) -> ExprId {
        while self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = self.ast.exprs.operand(e);
        }
        e
    }

    /// The preferred alignment of `ty` (`DataLayout::getPrefTypeAlign`):
    /// `AArch64` prefers 32-bit alignment for `i8`/`i16`.
    pub(crate) fn pref_align(&self, ty: LTy) -> u64 {
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
        // The array cookie (`GetArrayCookieSize`); none for the reserved
        // placement form (Itanium `requiresArrayCookie`).
        let cookie = if is_array
            && !self.alloc_fn(op_new).is_some_and(|a| a.reserved)
            && self.needs_array_cookie(base, x.operator_delete)
        {
            self.array_cookie_size(base)
        } else {
            0
        };
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
        // A parenthesized list counts the same (clang `EmitCXXNewAllocSize`).
        let ile = x.init.filter(|&i| {
            matches!(
                ast.exprs.get(i).kind(),
                ExprKind::InitListExpr | ExprKind::CXXParenListInitExpr
            )
        });
        let mut min_elems: u64 = 0;
        if is_array && let Some(i) = x.init {
            let lits: Vec<ExprId> = match ile {
                Some(l) => crate::expr::agg_init_parts(ast, l).0,
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
        let (size, size_nc, num_elems) = if is_array {
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
                let (total, ov2) = bytes.overflowing_add(cookie);
                let num = self.i64c(i128::from(c.wrapping_mul(asm)));
                let size = if ov || ov2 || c < min_elems {
                    self.i64c(-1)
                } else {
                    self.i64c(i128::from(total))
                };
                let size_nc = if cookie == 0 { size } else { self.i64c(i128::from(bytes)) };
                (size, size_nc, Some(num))
            } else {
                let mut has_ov = None;
                if min_elems > 0 {
                    let m = self.i64c(i128::from(min_elems));
                    has_ov = Some(self.icmp(IntPredicate::Ult, nv, m));
                }
                let mut size = nv;
                let mut num = nv;
                if tsm != 1 {
                    let tv = self.i64c(i128::from(tsm));
                    let r = self.overflow_call(OverflowOp::UMul, nv, tv);
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
                let mut size_nc = size;
                if cookie != 0 {
                    let cv = self.i64c(i128::from(cookie));
                    let r = self.overflow_call(OverflowOp::UAdd, size, cv);
                    let ov = self.f.fir.push(Op::ExtractValue { agg: r, idx: 1 }, LTy::I1);
                    has_ov = Some(match has_ov {
                        Some(h) => self.bin(BinOp::Or, Flags::NONE, h, ov),
                        None => ov,
                    });
                    size = self.f.fir.push(Op::ExtractValue { agg: r, idx: 0 }, LTy::I64);
                }
                if let Some(h) = has_ov {
                    let m1 = self.i64c(-1);
                    size = self.select(h, m1, size);
                }
                if cookie == 0 {
                    size_nc = size;
                }
                (size, size_nc, Some(num))
            }
        } else {
            let (sz, _) = self.size_align(x.allocated_ty);
            let size = self.i64c(i128::from(sz));
            (size, size, None)
        };

        // The allocation.
        let placement: Vec<ExprId> =
            x.placement.map(|l| ast.lists.iter(l).collect()).unwrap_or_default();
        let mut placement_vals: Vec<CallArg> = Vec::new();
        let (alloc, allocation_align) = if af.reserved {
            let Some(&arg) = placement.first() else {
                return err(loc, "placement new without a placement argument");
            };
            // clang `EmitCXXNewExpr`: the placement pointer's alignment
            // only when it is a declaration's.
            let p = self.pointer_with_alignment(arg)?;
            // The pointer is still the placement argument of a matching
            // class-scope `operator delete(void*, void*)` should the
            // initialization throw ([expr.new]/28; clang adds it to
            // `allocatorArgs`).
            placement_vals.push(CallArg::Scalar(p.v));
            (p.v, if p.decl { p.align } else { alloc_align })
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
            // Default-initializing a class is a (possibly trivial)
            // constructor call in clang's AST, so it counts as an
            // initializer (`CXXNewExpr::hasInitializer`).
            && (x.init.is_some() || t.as_record(base).is_some());
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

        // EnterNewDeleteCleanup: the deallocation function runs if the
        // initialization throws ([expr.new]/28); deactivated once it
        // completes. `dominator` stands for clang's `cleanupDominator`.
        let cleanup = self.opts.exceptions
            && x.operator_delete.is_some_and(|d| !self.alloc_fn(d).is_some_and(|a| a.reserved));
        let mut del_cleanup = None;
        if cleanup && let Some(op_delete) = x.operator_delete {
            let placement = placement_vals
                .iter()
                .zip(&placement)
                .map(|(&v, &a)| (Saved::Direct(v), ast.exprs.get(a).ty()))
                .collect();
            let nd = NewDelete {
                op_delete,
                ptr: Saved::Direct(CallArg::Scalar(alloc)),
                size: Saved::Direct(CallArg::Scalar(size)),
                align: alloc_align,
                pass_align,
                placement,
            };
            self.push_full_expr_cleanup(Cleanup::NewDelete(Box::new(nd)), false, true);
            let c = self.f.cl.depth();
            let z = V::C(self.m.const_bool(false));
            let dominator =
                self.f.fir.push(Op::Bin { op: BinOp::Or, flags: Flags::NONE, l: z, r: z }, LTy::I1);
            del_cleanup = Some((c, dominator));
        }

        // InitializeArrayCookie: the element count is stored just before
        // the elements, which start `cookie` bytes into the allocation.
        let num = num_elems.unwrap_or(size);
        let (data, data_align) = if cookie != 0 {
            // The cookie's `size_t` count makes the allocation at least
            // `size_t`-aligned.
            let allocation_align = allocation_align.max(8);
            let off = cookie - 8;
            let cp = self.byte_offset(alloc, off);
            self.store(num, cp, min_align(allocation_align, off), false);
            (self.byte_offset(alloc, cookie), min_align(allocation_align, cookie))
        } else {
            (alloc, allocation_align)
        };

        // The initializer.
        if is_array {
            self.emit_new_array_init(&x, base, esize, data, data_align, num, size_nc, loc)?;
        } else if let Some(init) = x.init {
            let ty = self.mty(x.allocated_ty);
            let lv = LV {
                addr: Addr { v: alloc, ty, align: allocation_align },
                q: x.allocated_ty,
                bf: None,
                vidx: None,
                tb: Tb::Type,
            };
            self.init_to_lvalue(init, lv, false)?;
        }
        if let Some((c, dominator)) = del_cleanup {
            let d = Fir::last_inst(dominator);
            self.deactivate_cleanup_block(c, d);
            self.f.fir.erase(d);
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
                .push(Op::Phi { ty: LTy::PTR, inc: vec![(data, nn_end), (z, nb)] }, LTy::PTR));
        }
        Ok(data)
    }

    /// A call of the `llvm.{op}.with.overflow.i64` intrinsic on `a`, `b`.
    fn overflow_call(&mut self, op: OverflowOp, a: V, b: V) -> V {
        let f = kiln_llvm::intrinsics::with_overflow(&mut self.m, op, LTy::I64);
        let fn_ty = self.m.func(f).ty();
        let rty = self.m.types.struct_literal(&[LTy::I64, LTy::I1], false);
        let r = self.f.fir.push(
            Op::Call {
                fn_ty,
                callee: V::C(Value::func(f)),
                args: vec![a, b],
                opts: Box::default(),
            },
            rty,
        );
        self.f.calls.push((Fir::last_inst(r), Some(f), true));
        r
    }

    /// Itanium `getArrayCookieSizeImpl`: room for a `size_t` count, padded
    /// to the element type's alignment.
    fn array_cookie_size(&mut self, elem: QualType) -> u64 {
        let (_, al) = self.size_align(elem);
        al.max(8)
    }

    /// Itanium `requiresArrayCookie`: elements of class type with a
    /// non-trivial destructor, or a usual array deallocation function
    /// that takes the size ([expr.new]/16, [expr.delete]/11).
    fn needs_array_cookie(&self, elem: QualType, op_delete: Option<DeclId>) -> bool {
        let ast = self.ast;
        if ast.types.as_record(elem).is_some_and(|r| self.has_nontrivial_dtor(ast.first(r))) {
            return true;
        }
        // Only a class-scope usual deallocation function asks for the
        // size without a non-trivial destructor (`doesUsualArrayDeleteWantSize`).
        let Some(d) = op_delete.filter(|&d| {
            ast.types.as_record(elem).is_some()
                && ast.decls.get(d).ctx().is_some_and(|c| ast.decls.record(c).is_some())
        }) else {
            return false;
        };
        let Some(proto) = ast.decls.get(d).ty().and_then(|f| ast.types.proto(f).copied()) else {
            return false;
        };
        let params = ast.types.params(&proto);
        params.len() >= 2
            && !self.is_std_enum(params[1], "align_val_t")
            && ast.types.canonical(params[1])
                == ast.types.canonical(QualType::unqual(ast.types.target().size_t))
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

    /// clang `EmitNewArrayInitializer` (no constructors, no cookies). Its
    /// `CleanupDeactivationScope` deactivates the irregular partial-array
    /// cleanup of a list initializer on every exit.
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
        let mut partial = None;
        let geom = (base, esize, begin, align, num, size);
        let r = self.new_array_init_body(x, geom, loc, &mut partial);
        if let Some(p) = partial {
            let d = Fir::last_inst(p.dominator);
            self.deactivate_cleanup_block(p.depth, d);
            self.f.fir.erase(d);
        }
        r
    }

    /// [except.ctor]/3 for the explicit elements of `new T[n]{...}`: an
    /// EH cleanup destroying `[begin, *array.init.end)`
    /// (`pushIrregularPartialArrayCleanup`, only when `needsEHCleanup`).
    fn push_new_partial(&mut self, base: QualType, begin: V, record: DeclId) -> NewPartial {
        let z = V::C(self.m.const_bool(false));
        let dominator =
            self.f.fir.push(Op::Bin { op: BinOp::Or, flags: Flags::NONE, l: z, r: z }, LTy::I1);
        let mark = self.f.fir.mark();
        let al = self.pref_align(LTy::PTR);
        let slot = self.f.fir.alloca(LTy::PTR, al);
        // The cleanup walks base elements: clang passes it the base
        // element type of a multidimensional allocation.
        let elem = self.mty(base);
        let p = IrregularPartialArray {
            begin,
            saved: false,
            end_slot: slot,
            outer: elem,
            depth: 0,
            elem,
            record,
        };
        self.push_full_expr_cleanup(Cleanup::IrregularPartialArray(Box::new(p)), false, true);
        self.add_aux_allocas(mark);
        NewPartial { depth: self.f.cl.depth(), dominator, slot, align: al }
    }

    /// The body of [`Cg::emit_new_array_init`]; `geom` is the base element
    /// type, its size, the first element, its alignment, the element count
    /// and the byte size.
    fn new_array_init_body(
        &mut self,
        x: &NewExtra,
        (base, esize, begin, align, num, size): (QualType, u64, V, u64, V, V),
        loc: Option<Loc>,
        partial: &mut Option<NewPartial>,
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
        // clang `EmitNewArrayInitializer`: the explicit elements of a braced
        // or parenthesized list, then its array filler.
        if matches!(
            ast.exprs.get(init0).kind(),
            ExprKind::InitListExpr | ExprKind::CXXParenListInitExpr
        ) {
            let (elems, filler, _) = crate::expr::agg_init_parts(ast, init0);
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
            if let Some(record) = self.destructed_class(base).filter(|_| self.opts.exceptions) {
                *partial = Some(self.push_new_partial(base, begin, record));
            }
            let start_align = cur_align;
            for (k, &ie) in elems.iter().enumerate() {
                if let Some(p) = *partial {
                    self.store(cur, p.slot, p.align, false);
                }
                let q = ast.exprs.get(ie).ty();
                let lv = LV {
                    addr: Addr { v: cur, ty: cur_ty, align: cur_align },
                    q,
                    bf: None,
                    vidx: None,
                    tb: Tb::Type,
                };
                self.init_to_lvalue(ie, lv, false)?;
                let one = self.i64c(1);
                cur = self.gep(cur_ty, cur, vec![one], true);
                cur_align = min_align(start_align, (k as u64 + 1).wrapping_mul(esize));
            }
            init = filler;
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
        // A constructor call loop over the remaining elements
        // (`EmitCXXAggrConstructorCall`).
        if ast.exprs.get(init).kind() == ExprKind::CXXConstructExpr {
            let ctor = ast.exprs.constructor(init);
            let zeroing = ast.exprs.construct_info(init).zeroing;
            let trivial = ast.decls.function(ctor).is_some_and(|f| f.spec.is_trivial());
            let rd = ast.decls.get(ctor).ctx();
            let empty = rd.is_some_and(|r| self.class_bits(r).contains(DefinitionBits::EMPTY));
            if trivial && (!zeroing || empty) {
                return Ok(());
            }
            if trivial && self.zero_init(base) {
                // TryMemsetInitialization.
                let mut rem = size;
                if n_init > 0 {
                    let s = self.i64c(i128::from(esize.wrapping_mul(n_init)));
                    rem = self.bin(BinOp::Sub, Flags::NONE, size, s);
                }
                self.memset_v(Addr { v: cur, ty: base_ty, align: cur_align }, 0, rem, false);
                return Ok(());
            }
            if let Some(p) = *partial {
                self.store(cur, p.slot, p.align, false);
            }
            let mut rem = num;
            if n_init > 0 {
                let k = self.i64c(i128::from(n_init));
                rem = self.bin(BinOp::Sub, Flags::NONE, num, k);
            }
            let ea = min_align(cur_align, esize);
            return self.construct_array_n(init, cur, base_ty, ea, rem, zeroing);
        }
        // TryMemsetInitialization (only for a zero-initializable element).
        if self.zero_init(base) && self.memset_init(init) {
            let mut rem = size;
            if n_init > 0 {
                let s = self.i64c(i128::from(esize.wrapping_mul(n_init)));
                rem = self.bin(BinOp::Sub, Flags::NONE, size, s);
            }
            self.memset_v(Addr { v: cur, ty: base_ty, align: cur_align }, 0, rem, false);
            return Ok(());
        }
        let ivie = ast.exprs.get(init).kind() == ExprKind::ImplicitValueInitExpr;
        if ivie && let Some(rd) = t.as_record(base) {
            // Sema value-initializes the elements of `new T[n]()` with an
            // `ImplicitValueInitExpr` where clang builds a zeroing trivial
            // `CXXConstructExpr`: clang's `EmitCXXAggrConstructorCall` loop.
            if self.class_bits(rd).contains(DefinitionBits::EMPTY) {
                return Ok(());
            }
            let mut rem = num;
            if n_init > 0 {
                let k = self.i64c(i128::from(n_init));
                rem = self.bin(BinOp::Sub, Flags::NONE, num, k);
            }
            let ea = min_align(cur_align, esize);
            return self.null_init_array_n(cur, base_ty, base, ea, rem);
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
        if let Some(p) = *partial {
            self.store(phi, p.slot, p.align, false);
        }
        // clang switches a value-initialization to one of the element
        // type (multidimensional `new` of member pointers).
        let iq = if ivie { base } else { ast.exprs.get(init).ty() };
        let ity = self.mty(iq);
        let lv = LV {
            addr: Addr { v: phi, ty: ity, align: elem_align },
            q: iq,
            bf: None,
            vidx: None,
            tb: Tb::Type,
        };
        self.init_to_lvalue(init, lv, false)?;
        // clang leaves the list's partial-array cleanup in the loop body,
        // after the first element's initializer.
        if let Some(p) = partial.take() {
            let d = Fir::last_inst(p.dominator);
            self.deactivate_cleanup_block(p.depth, d);
            self.f.fir.erase(d);
        }
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

    /// clang `EmitCXXAggrConstructorCall` with `zeroInitialize` for a
    /// trivial default constructor: each of the `num` elements of type `eq`
    /// from `begin` is null-initialized (`EmitNullInitialization`).
    fn null_init_array_n(
        &mut self,
        begin: V,
        elem: LTy,
        eq: QualType,
        elem_align: u64,
        num: V,
    ) -> R<()> {
        let mut out = None;
        if self.cint(num).is_none() {
            let z = self.i64c(0);
            let empty = self.icmp(IntPredicate::Eq, num, z);
            let lp = self.f.fir.create_block();
            let o = self.f.fir.create_block();
            self.cond_br(empty, o, lp);
            self.emit_block(lp, false);
            out = Some(o);
        } else if self.cint(num).is_some_and(|(_, c, _)| c == 0) {
            return Ok(());
        }
        let end = self.gep(elem, begin, vec![num], true);
        let entry = self.f.fir.cur().expect("insertion point");
        let body = self.f.fir.create_block();
        self.emit_block(body, false);
        let phi = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(begin, entry)] }, LTy::PTR);
        let size = self.alloc_size(elem);
        self.emit_null_init(Addr { v: phi, ty: elem, align: elem_align }, eq, size)?;
        let one = self.i64c(1);
        let next = self.gep(elem, phi, vec![one], true);
        let cur = self.f.fir.cur().expect("insertion point");
        self.f.fir.add_incoming(Fir::last_inst(phi), next, cur);
        let done = self.icmp(IntPredicate::Eq, next, end);
        let out = out.unwrap_or_else(|| self.f.fir.create_block());
        self.cond_br(done, out, body);
        self.emit_block(out, false);
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
                let rd = ast.definition(rd).unwrap_or(rd);
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
        let array_form = ast.exprs.get(e).has(ExprFlags::ARRAY_FORM);
        let mut cookie = 0;
        if array_form {
            let mut b = dt;
            while let Some(el) = t.element(b) {
                b = el;
            }
            if self.needs_array_cookie(b, Some(od)) {
                cookie = self.array_cookie_size(b);
            }
        }
        let p = self.emit_scalar(arg)?;
        let notnull = self.f.fir.create_block();
        let end = self.f.fir.create_block();
        let z = V::C(self.m.const_null(LTy::PTR));
        let isnull = self.icmp(IntPredicate::Eq, p, z);
        self.cond_br(isnull, end, notnull);
        self.emit_block(notnull, false);
        let mut ptr = p;
        let const_array = matches!(t.canonical_view(dt), TypeView::ConstantArray(_));
        if const_array {
            let src = self.mty(dt);
            let zero = self.icn(LTy::I32, 0);
            let mut idx = vec![zero];
            while let TypeView::ConstantArray(a) = t.canonical_view(dt) {
                dt = a.elem;
                idx.push(zero);
            }
            ptr = self.gep(src, p, idx, true);
        }
        // clang `EmitObjectDelete`: [expr.delete]/3, the object's static
        // type is its dynamic type or a base with a virtual destructor.
        if !array_form && !ast.is_destroying_operator_delete(od) {
            let loc = self.ub_expr_loc(e);
            self.ub_type_check(ptr, TypeCheck::new(Tck::MemberCall, loc, dt))?;
        }
        if const_array {
            // The element pointer goes to `operator delete` below.
        } else if ast.is_destroying_operator_delete(od) {
            // [expr.delete]/7: a destroying operator delete runs no
            // destructor; it is responsible for destroying the object.
            // clang `EmitDestroyingObjectDelete`: through a virtual
            // destructor, the deleting destructor of the dynamic type
            // (which calls that class's destroying delete) is called
            // virtually, never devirtualized (`emitVirtualObjectDelete`).
            let def = t.as_record(dt).and_then(|r| self.class_def(r));
            if let Some(def) = def
                && let Some(dtor) = ast.destructor(def)
                && ast.decls.function(dtor).is_some_and(|f| f.spec.is_virtual())
            {
                let fty = ast.decls.get(ast.first(dtor)).ty().expect("destructor type");
                let sig = self.call_sig_this(fty, false, &[], Some(def));
                let (_, al) = self.size_align(dt);
                let fp = self.virtual_fn_ptr(dtor, Some(mangle::Structor::Deleting), p, al)?;
                self.call_vals(&sig, fp, None, Some(p), &[], &[], None, &[])?;
                self.emit_block(end, false);
                return Ok(());
            }
        } else if cookie == 0 && self.emit_object_delete(e, arg, p, dt, loc)? {
            self.emit_block(end, false);
            return Ok(());
        }
        if !array_form && self.opts.opt_size > 1 && !ast.is_destroying_operator_delete(od) {
            // clang `EmitObjectDelete` at -Oz: "When optimizing for size,
            // call 'operator delete' unconditionally" — the
            // `CallObjectDelete` cleanup is emitted in the end block.
            self.emit_block(end, false);
            self.emit_delete_call(od, ptr, dt, loc)?;
            return Ok(());
        }
        if cookie != 0 {
            self.emit_array_delete(od, ptr, dt, cookie);
        } else {
            self.emit_delete_call(od, ptr, dt, loc)?;
        }
        self.emit_block(end, false);
        Ok(())
    }

    /// clang `EmitArrayDelete` with an array cookie: reads the element
    /// count back (`ReadArrayCookie`), destroys the elements in reverse
    /// order and deallocates — the latter as a cleanup so that it also
    /// runs if a destructor throws ([expr.delete]/6-7).
    fn emit_array_delete(&mut self, od: DeclId, ptr: V, elem: QualType, cookie: u64) {
        let (_, elem_align) = self.size_align(elem);
        let back = self.i64c(-i128::from(cookie));
        let alloc = self.gep(LTy::I8, ptr, vec![back], true);
        let off = cookie - 8;
        let np = self.byte_offset(alloc, off);
        let num = self.load(LTy::I64, np, min_align(min_align(elem_align, cookie), off), false);
        self.push_cleanup(Cleanup::ArrayDelete(Box::new(ArrayDelete {
            op_delete: od,
            ptr: alloc,
            num,
            elem,
            cookie,
        })));
        if let Some(rd) = self.ast.types.as_record(elem)
            && let Some(def) = self.class_def(rd)
            && self.has_nontrivial_dtor(def)
        {
            let ety = self.mty(elem);
            let end = self.gep(ety, ptr, vec![num], true);
            self.array_destroy_range(ptr, end, ety, def, true, true);
        }
        self.pop_cleanup_block(false);
    }

    /// clang `EmitObjectDelete` (the destructor part): destroys the object
    /// of type `dt` at `p` before its deallocation. Returns `true` when a
    /// virtual call to the deleting destructor (`emitVirtualObjectDelete`)
    /// also deallocated it.
    fn emit_object_delete(
        &mut self,
        e: ExprId,
        arg: ExprId,
        p: V,
        dt: QualType,
        loc: Option<Loc>,
    ) -> R<bool> {
        let ast = self.ast;
        let Some(rd) = ast.types.as_record(dt) else { return Ok(false) };
        let Some(def) = self.class_def(rd) else { return Ok(false) };
        if !self.has_nontrivial_dtor(def) {
            return Ok(false);
        }
        let Some(mut dtor) = ast.destructor(def) else {
            return err(loc, "destructor not declared");
        };
        let mut dclass = def;
        if ast.decls.function(dtor).is_some_and(|f| f.spec.is_virtual()) {
            let mut use_virtual = true;
            if let Some((dd, _)) = self.devirtualize(dtor, arg) {
                let dd_class = ast.decls.get(dd).ctx().map(|c| ast.first(c));
                if dd_class == Some(ast.first(def)) {
                    dtor = dd;
                    use_virtual = false;
                }
            }
            if use_virtual {
                let first = ast.first(dtor);
                let fty = ast.decls.get(first).ty().expect("destructor type");
                let sig = self.call_sig_this(fty, false, &[], Some(def));
                let (_, al) = self.size_align(dt);
                if ast.exprs.get(e).has(ExprFlags::GLOBAL) {
                    // emitVirtualObjectDelete with UseGlobalDelete: the
                    // complete object is `p + vtable[-2]`; the complete
                    // destructor is called virtually, then the global
                    // `operator delete` (the CallObjectDelete cleanup).
                    let Some(od) = ast.exprs.operator_delete(e) else {
                        return err(loc, "delete-expression without a deallocation function");
                    };
                    let vt = self.load(LTy::PTR, p, al, false);
                    self.tbaa_vptr(vt);
                    let m2 = self.i64c(-2);
                    let op = self.gep(LTy::I64, vt, vec![m2], true);
                    let off = self.load(LTy::I64, op, 8, false);
                    let complete = self.gep(LTy::I8, p, vec![off], true);
                    let fp = self.virtual_fn_ptr(dtor, Some(mangle::Structor::Complete), p, al)?;
                    self.call_vals(&sig, fp, None, Some(p), &[], &[], None, &[])?;
                    self.emit_delete_call(od, complete, dt, loc)?;
                    return Ok(true);
                }
                // emitVirtualObjectDelete: the deleting destructor frees it.
                let fp = self.virtual_fn_ptr(dtor, Some(mangle::Structor::Deleting), p, al)?;
                self.call_vals(&sig, fp, None, Some(p), &[], &[], None, &[])?;
                return Ok(true);
            }
            dclass = ast.decls.get(dtor).ctx().map_or(def, |c| ast.first(c));
        }
        // EmitCXXDestructorCall(Dtor_Complete); the CallObjectDelete
        // cleanup then deallocates.
        let (callee, fid) = self.structor_ref(dtor, mangle::Structor::Complete)?;
        let first = ast.first(dtor);
        let fty = ast.decls.get(first).ty().expect("destructor type");
        let sig = self.call_sig_this(fty, false, &[], Some(dclass));
        self.call_vals(&sig, callee, fid, Some(p), &[], &[], None, &[])?;
        Ok(false)
    }

    /// `CallDeleteDuringNew::Emit`: the deallocation function of a
    /// new-expression whose initialization threw, passed the size and
    /// alignment a usual deallocation function takes, or for a placement
    /// form the placement arguments (and the alignment if the allocation
    /// was passed one) ([expr.new]/28).
    pub(crate) fn emit_new_delete_cleanup(&mut self, c: &NewDelete) -> R<()> {
        let ast = self.ast;
        let t = &ast.types;
        let loc = Some(ast.decls.get(c.op_delete).loc());
        let Some(fty) = ast.decls.get(c.op_delete).ty() else {
            return err(loc, "deallocation function without a type");
        };
        let Some(proto) = t.proto(fty).copied() else {
            return err(loc, "deallocation function without a prototype");
        };
        let params: Vec<QualType> = t.params(&proto).to_vec();
        let Some(&p0) = params.first() else {
            return err(loc, "deallocation function without parameters");
        };
        let size_t = QualType::unqual(t.target().size_t);
        let mut vals = vec![self.restore_saved(c.ptr)];
        let mut arg_tys = vec![p0];
        if c.placement.is_empty() {
            let mut i = 1;
            if let Some(&q) = params.get(i)
                && t.is_integral(q)
                && !self.is_std_enum(q, "align_val_t")
            {
                vals.push(self.restore_saved(c.size));
                arg_tys.push(size_t);
                i += 1;
            }
            if let Some(&q) = params.get(i)
                && self.is_std_enum(q, "align_val_t")
            {
                vals.push(CallArg::Scalar(self.i64c(i128::from(c.align))));
                arg_tys.push(size_t);
            }
        } else if c.pass_align {
            vals.push(CallArg::Scalar(self.i64c(i128::from(c.align))));
            arg_tys.push(size_t);
        }
        for &(a, q) in &c.placement {
            vals.push(self.restore_saved(a));
            arg_tys.push(q);
        }
        self.emit_new_delete_call(c.op_delete, &vals, &arg_tys, loc)?;
        Ok(())
    }

    /// `DominatingValue::restore`.
    pub(crate) fn restore_saved(&mut self, s: Saved) -> CallArg {
        match s {
            Saved::Direct(a) => a,
            Saved::Slot { slot, ty, align } => CallArg::Scalar(self.load(ty, slot, align, false)),
            Saved::AggSlot { slot, addr } => {
                let al = self.pref_align(LTy::PTR);
                CallArg::Agg(Addr { v: self.load(LTy::PTR, slot, al, false), ..addr })
            }
        }
    }

    /// clang `EmitDeleteCall`: calls deallocation function `od` for the
    /// object of type `dt` at `ptr` (sized / aligned per its parameters,
    /// `getUsualDeleteParams`).
    pub(crate) fn emit_delete_call(
        &mut self,
        od: DeclId,
        ptr: V,
        dt: QualType,
        loc: Option<Loc>,
    ) -> R<()> {
        self.emit_delete_call_n(od, ptr, dt, None, loc)
    }

    /// [`Self::emit_delete_call`] of an array of `n.0` elements of type
    /// `dt` behind an array cookie of `n.1` bytes (the size passed to a
    /// sized deallocation function covers both).
    pub(crate) fn emit_delete_call_n(
        &mut self,
        od: DeclId,
        ptr: V,
        dt: QualType,
        n: Option<(V, u64)>,
        loc: Option<Loc>,
    ) -> R<()> {
        let ast = self.ast;
        let t = &ast.types;
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
        let mut tag_slot = None;
        // A destroying operator delete ([expr.delete]/10) takes the
        // (empty) `std::destroying_delete_t` tag by value next.
        if ast.is_destroying_operator_delete(od)
            && let Some(&q) = params.get(1)
        {
            let tag = self.temp(q);
            tag_slot = Some(tag.v);
            vals.push(CallArg::Agg(tag));
            arg_tys.push(q);
            i = 2;
        }
        let incomplete = t.is_void(dt)
            || t.as_record(dt).is_some_and(|r| {
                ast.definition(r).is_none_or(|d| {
                    !ast.decls.record(d).is_some_and(|x| x.defn.contains(DefinitionBits::COMPLETE))
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
            let mut size = self.icn(ty, i128::from(sz));
            if let Some((num, cookie)) = n {
                size = self.bin(BinOp::Mul, Flags::NONE, size, num);
                if cookie != 0 {
                    let c = self.icn(ty, i128::from(cookie));
                    size = self.bin(BinOp::Add, Flags::NONE, size, c);
                }
            }
            vals.push(CallArg::Scalar(size));
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
        // clang erases the tag's alloca when the ABI ignored the empty
        // argument (x86-64), keeping it where it was passed (AArch64).
        if let Some(a) = tag_slot
            && self.f.fir.uses(a) == 0
        {
            self.f.fir.erase(Fir::last_inst(a));
        }
        Ok(())
    }
}
