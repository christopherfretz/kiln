//! Variable length arrays (a GNU extension in C++, WP-183): clang's
//! `EmitVariablyModifiedType`, `getVLASize`, the `vla` alloca of
//! `EmitAutoVarAlloca` with its stack save and restore, and `sizeof` of a
//! variable length array (spec/codegen.md).

use kiln_ast::expr::{CastKind, ExprKind, TraitKind};
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
        if self.destructed_class(elem).is_some() || init.is_some() {
            return err(
                loc,
                "variable length arrays of objects needing construction or destruction \
                 are not supported yet (M3)",
            );
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
        while let TypeView::VariableArray { elem, size } = self.ast.types.canonical_view(t) {
            let n = self.f.vla_sizes[&size];
            if self.cint(n).is_none() {
                let al = self.pref_align(LTy::I64);
                let s = self.f.fir.alloca(LTy::I64, al);
                self.store(n, s, al, false);
            }
            t = elem;
        }
        let addr = Addr { v, ty, align };
        self.f.locals.insert(d, addr);
        // [basic.indet]/1: the erroneous value (spec/codegen.md §7y).
        self.auto_var_init_vla(d, q, addr)
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
