//! Variable length arrays (a GNU extension in C++, WP-183): clang's
//! `EmitVariablyModifiedType`, `getVLASize`, the `vla` alloca of
//! `EmitAutoVarAlloca` with its stack save and restore, and `sizeof` of a
//! variable length array (spec/codegen.md).

use kiln_ast::expr::{CastKind, ExprKind, TraitKind};
use kiln_ast::lambda::CaptureKind;
use kiln_ast::types::{TypeFlags, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_llvm::{BinOp, Flags, TypeId as LTy};

use crate::cleanup::Cleanup;
use crate::fir::{Op, V};
use crate::func::Addr;
use crate::module::{Cg, R, err};

impl Cg<'_> {
    /// `q` is a variable length array type.
    pub(crate) fn is_vla(&self, q: QualType) -> bool {
        matches!(self.ast.types.canonical_view(q), TypeView::VariableArray { .. })
    }

    /// `e` is the array-to-pointer decay of a variable length array.
    pub(crate) fn is_vla_decay(&self, e: ExprId) -> bool {
        let ast = self.ast;
        let mut e = e;
        while ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = ast.exprs.operand(e);
        }
        ast.exprs.get(e).kind().is_cast()
            && ast.exprs.cast_kind(e) == CastKind::ArrayToPointerDecay
            && self.is_vla(ast.exprs.get(ast.exprs.operand(e)).ty())
    }

    /// clang `EmitVariablyModifiedType` for the array types of `q`:
    /// evaluates each variable length array bound not evaluated yet,
    /// converted to `size_t` as unsigned (`VLASizeMap`).
    pub(crate) fn emit_vla_bounds(&mut self, q: QualType) -> R<()> {
        if !self.ast.types.qflags(q).contains(TypeFlags::VARIABLY_MODIFIED) {
            return Ok(());
        }
        // clang `EmitVariablyModifiedType` starts with `EnsureInsertPoint`:
        // the declaration may follow a `noreturn` call or a `return`.
        self.ensure_insert_point();
        let mut q = q;
        loop {
            if !self.ast.types.qflags(q).contains(TypeFlags::VARIABLY_MODIFIED) {
                return Ok(());
            }
            match self.ast.types.canonical_view(q) {
                TypeView::VariableArray { elem, size } => {
                    if !self.f.vla_sizes.contains_key(&size) {
                        let v = self.emit_scalar(size)?;
                        let sq = self.ast.exprs.get(size).ty();
                        self.ub_vla_bound(size, v, sq)?;
                        let v = self.int_cast(v, LTy::I64, false);
                        self.f.vla_sizes.insert(size, v);
                    }
                    q = elem;
                }
                TypeView::ConstantArray(a) => q = a.elem,
                TypeView::IncompleteArray(e)
                | TypeView::Pointer(e)
                | TypeView::LValueRef(e)
                | TypeView::RValueRef(e) => q = e,
                _ => return Ok(()),
            }
        }
    }

    /// The `i64` value of the variable length array bound `size`: the one
    /// computed where its array type was declared (`VLASizeMap`), or, for
    /// a bound not evaluated yet in this function, evaluated now and
    /// remembered.
    pub(crate) fn vla_bound_value(&mut self, size: ExprId) -> R<V> {
        if let Some(&v) = self.f.vla_sizes.get(&size) {
            return Ok(v);
        }
        self.ensure_insert_point();
        let v = self.emit_scalar(size)?;
        let sq = self.ast.exprs.get(size).ty();
        self.ub_vla_bound(size, v, sq)?;
        let v = self.int_cast(v, LTy::I64, false);
        self.f.vla_sizes.insert(size, v);
        Ok(v)
    }

    /// clang `StartFunction` in a lambda's call operator: each variable
    /// length array bound the closure `l` captured
    /// ([`CaptureKind::VlaBound`]) is loaded from its member into
    /// `VLASizeMap`, so the body's uses of the array types see the value
    /// of the enclosing function.
    pub(crate) fn load_vla_bound_captures(&mut self, l: kiln_ast::LambdaId) -> R<()> {
        let ast = self.ast;
        let data = ast.exprs.lambdas.get(l);
        if !data.captures.iter().any(|c| c.kind == CaptureKind::VlaBound) {
            return Ok(());
        }
        let inits: Vec<ExprId> =
            data.inits.map(|x| ast.lists.iter::<ExprId>(x).collect()).unwrap_or_default();
        let caps: Vec<_> = data.captures.iter().filter(|c| c.field.is_some()).copied().collect();
        for (c, size) in caps.into_iter().zip(inits) {
            let Some(field) = c.field.filter(|_| c.kind == CaptureKind::VlaBound) else {
                continue;
            };
            let fq = ast.decls.get(field).ty().unwrap_or_else(|| ast.exprs.get(size).ty());
            let lv = self.closure_field_lvalue(field, fq)?;
            let t = self.lty(fq);
            let v = self.load(t, lv.addr.v, lv.addr.align, false);
            let v = self.int_cast(v, LTy::I64, false);
            self.f.vla_sizes.insert(size, v);
        }
        Ok(())
    }

    /// clang `getVLASize`: the total element count of the variable length
    /// array type `q` (its nested variable length dimensions multiplied)
    /// and its first non-variable element type.
    pub(crate) fn vla_size(&mut self, q: QualType) -> R<(V, QualType)> {
        let mut q = q;
        let mut n: Option<V> = None;
        while let TypeView::VariableArray { elem, size } = self.ast.types.canonical_view(q) {
            let Some(&v) = self.f.vla_sizes.get(&size) else {
                return err(self.loc_of(size), "variable length array bound not emitted");
            };
            n = Some(match n {
                Some(p) => self.bin(BinOp::Mul, Flags::NUW, p, v),
                None => v,
            });
            q = elem;
        }
        match n {
            Some(n) => Ok((n, q)),
            None => err(None, "not a variable length array"),
        }
    }

    /// clang `EmitAutoVarAlloca` (and the rest of `EmitAutoVarDecl`) for
    /// the local variable `d` of variable length array type `q`.
    pub(crate) fn emit_vla_var(&mut self, d: DeclId, q: QualType, init: Option<ExprId>) -> R<()> {
        let ast = self.ast;
        let loc = Some(ast.decls.get(d).loc());
        if self.in_split_coroutine() {
            // Sema rejects a variable length array written in a coroutine
            // (`sema_vla_in_coroutine_unsupported`); one an instantiation
            // produced stops here, not in LLVM's `CoroSplit` (WP-1374).
            return err(loc, "variable length arrays in a coroutine are not supported");
        }
        self.emit_vla_bounds(q)?;
        self.ensure_insert_point();
        let mut elem = q;
        while let TypeView::VariableArray { elem: e, .. } = self.ast.types.canonical_view(elem) {
            elem = e;
        }
        // A class element type: `init` is the default construction of the
        // elements; Sema rejects every other initializer of a variable
        // length array in C++ ("variable-sized object may not be
        // initialized", as clang 22 at every `-std=`).
        if init.is_some_and(|i| ast.exprs.get(i).kind() != ExprKind::CXXConstructExpr) {
            // kiln-gap: GAP-091
            return err(loc, "this initializer of a variable length array is not supported yet");
        }
        if !self.f.did_stack_save {
            // The stack is saved once per cleanup scope and restored when
            // the scope is left normally (`CallStackRestore`).
            let al = self.pref_align(LTy::PTR);
            let slot = self.f.fir.alloca(LTy::PTR, al);
            let f = kiln_llvm::intrinsics::stacksave(&mut self.m);
            let v = self.intrinsic_call(f, Vec::new());
            self.store(v, slot, al, false);
            self.f.did_stack_save = true;
            self.push_cleanup_kind(Cleanup::StackRestore { slot }, true, false);
        }
        let (count, _) = self.vla_size(q)?;
        let ty = self.mty(elem);
        // clang `getDeclAlign`: a variable length array always gets the
        // target's large-array alignment (x86's 16).
        let align = self.size_align(elem).1;
        let align = if self.is_x86() { align.max(16) } else { align };
        let v = self.f.fir.push(Op::DynAlloca { ty, count, align }, LTy::PTR);
        // `EmitAndRegisterVariableArrayDimensions`: each non-constant
        // dimension is stored to an `i64` slot (the debug-info
        // `__vla_exprN`), emitted with or without debug info.
        let mut t = q;
        let mut dims: Vec<(ExprId, Result<V, i64>)> = Vec::new();
        while let TypeView::VariableArray { elem, size } = self.ast.types.canonical_view(t) {
            let n = self.f.vla_sizes[&size];
            if let Some((_, c, _)) = self.cint(n) {
                dims.push((size, Err(i64::try_from(c).unwrap_or(-1))));
            } else {
                let al = self.pref_align(LTy::I64);
                let s = self.f.fir.alloca(LTy::I64, al);
                self.store(n, s, al, false);
                dims.push((size, Ok(s)));
            }
            t = elem;
        }
        let addr = Addr { v, ty, align };
        self.f.locals.insert(d, addr);
        if self.dbg_on() {
            // Each dimension's `__vla_exprN`, then the array, whose
            // subranges count by them (WP-1445).
            self.di_declare_vla_dims(&dims);
            self.di_declare_var(
                d,
                crate::debuginfo::func::VarSlot { addr: v, arg: None, deref: false },
            );
        }
        // [basic.indet]/1: the erroneous value (spec/codegen.md §7y).
        self.auto_var_init_vla(d, q, addr)?;
        // clang `EmitCXXAggrConstructorCall` with the run-time element
        // count, then the `emitDestroy` of the whole array (from the end,
        // an empty array checked first) as the variable's cleanup. An
        // element that is itself an array of class objects (`S a[n][3]`)
        // is flattened, as clang's `emitArrayLength` does: the loop runs
        // over `n * 3` objects of the base class type.
        if init.is_none() && self.destructed_class(elem).is_none() {
            return Ok(());
        }
        let mut base = elem;
        let mut count = count;
        while let TypeView::ConstantArray(a) = self.ast.types.canonical_view(base) {
            let k = self.i64c(i128::from(a.size));
            count = self.bin(BinOp::Mul, Flags::NUW, count, k);
            base = a.elem;
        }
        let bty = self.mty(base);
        let elem_size = self.alloc_size(bty);
        let elem_align = crate::func::min_align(align, elem_size);
        if let Some(i) = init {
            self.construct_array_n(i, v, bty, elem_align, count, false, false)?;
        }
        if let Some(record) = self.destructed_class(base) {
            let end = self.gep(bty, v, vec![count], true);
            let c = Cleanup::PartialArray { begin: v, end, elem: bty, record };
            self.push_cleanup_kind(c, true, true);
        }
        Ok(())
    }

    /// clang `EmitArraySubscriptExpr` of an element of variable length
    /// array type `q`: the index scaled (`mul nsw`) by the element count
    /// of `q` (all its variable dimensions), and `q`'s element type (whose
    /// size, zero for a variable length array, scales the alignment like
    /// clang's `getTypeSizeInChars`). `None` when `q` is not a variable
    /// length array.
    pub(crate) fn vla_scaled_index(&mut self, q: QualType, iv: V) -> R<Option<(V, QualType)>> {
        let TypeView::VariableArray { elem, .. } = self.ast.types.canonical_view(q) else {
            return Ok(None);
        };
        let (n, _) = self.vla_size(q)?;
        let fl = self.vla_nsw();
        Ok(Some((self.bin(BinOp::Mul, fl, iv, n), elem)))
    }

    /// The `StackRestore` cleanup (clang `CallStackRestore`).
    pub(crate) fn emit_stack_restore(&mut self, slot: V) {
        let al = self.pref_align(LTy::PTR);
        let v = self.load(LTy::PTR, slot, al, false);
        let f = kiln_llvm::intrinsics::stackrestore(&mut self.m);
        self.intrinsic_call(f, vec![v]);
    }

    /// clang `ScalarExprEmitter::VisitUnaryExprOrTypeTraitExpr` for
    /// `sizeof` of a variable length array: the operand expression is
    /// evaluated (a type operand's bounds are), and the size is the
    /// element count scaled by the element size. `None` for any other
    /// trait expression.
    pub(crate) fn emit_vla_sizeof(&mut self, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        if ast.exprs.trait_kind(e) != TraitKind::SizeOf {
            return Ok(None);
        }
        let ta = ast.exprs.trait_type_arg(e);
        let op = ExprId::new(ast.exprs.get(e).a());
        let q = ta.unwrap_or_else(|| ast.exprs.get(op).ty());
        if !self.is_vla(q) {
            return Ok(None);
        }
        if ta.is_some() {
            self.emit_vla_bounds(q)?;
        } else {
            self.emit_ignored(op)?;
        }
        self.vla_bytes(q).map(Some)
    }

    /// The size in bytes of the variable length array type `q`: its
    /// element count scaled (`mul nuw`) by its non-variable element's
    /// size (clang's `sizeof` and pointer-difference divisor).
    pub(crate) fn vla_bytes(&mut self, q: QualType) -> R<V> {
        let (n, elem) = self.vla_size(q)?;
        let esz = self.size_align(elem).0;
        if esz == 1 {
            return Ok(n);
        }
        let s = self.i64c(i128::from(esz));
        Ok(self.bin(BinOp::Mul, Flags::NUW, s, n))
    }

    /// clang `emitPointerArithmetic` / `EmitScalarPrePostIncDec` on a
    /// pointer to the variable length array type `q`: the `i64` index
    /// `idx` (already negated for a subtraction) scaled (`mul nsw`) by
    /// `q`'s element count, and the IR type of `q`'s non-variable element
    /// the GEP steps over. `None` when `q` is not a variable length array.
    pub(crate) fn vla_pointer_step(&mut self, q: QualType, idx: V) -> R<Option<(V, LTy)>> {
        if !self.is_vla(q) {
            return Ok(None);
        }
        let (n, elem) = self.vla_size(q)?;
        let ty = self.mty(elem);
        let fl = self.vla_nsw();
        Ok(Some((self.bin(BinOp::Mul, fl, idx, n), ty)))
    }

    /// The step of `++`/`--` on a pointer to the variable length array
    /// type `q`: its element count, negated (`sub nsw 0, n`) for `--`,
    /// and the IR element type; `None` when `q` is not a variable length
    /// array (clang `EmitScalarPrePostIncDec`, `vla.negsize`).
    pub(crate) fn vla_inc_step(&mut self, q: QualType, inc: bool) -> R<Option<(V, LTy)>> {
        if !self.is_vla(q) {
            return Ok(None);
        }
        let (mut n, elem) = self.vla_size(q)?;
        if !inc {
            let z = self.i64c(0);
            n = self.bin(BinOp::Sub, Flags::NSW, z, n);
        }
        Ok(Some((n, self.mty(elem))))
    }
}
