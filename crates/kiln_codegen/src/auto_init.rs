//! The erroneous value of an automatic variable (C++26 [basic.indet]/1,
//! P2795R5; WP-448, WP-452): clang's `-ftrivial-auto-var-init=zero` and
//! `=pattern` (`EmitAutoVarInit`'s `initializeWhatIsTechnicallyUninitialized`,
//! `emitZeroOrPatternForAutoVarInit`, `initializationPatternFor`,
//! `constWithPadding` and `replaceUndef` in `CGDecl.cpp`). Kiln applies
//! `zero` by default in C++26 (spec/codegen.md §7r, §7y).

use kiln_ast::attr::AttrKind;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_llvm::{BinOp, CastOp, Flags, IntPredicate, TypeId as LTy, Value};
use kiln_source::Span;

use crate::AutoVarInit;
use crate::fir::{Blk, Fir, Op, V};
use crate::fold::{is_local, is_static_local};
use crate::func::{Addr, min_align};
use crate::module::{Cg, R, gval};

/// clang's pattern for integers, pointers and padding bytes.
const PATTERN_BYTE: u8 = 0xAA;
const PATTERN_WORD: u64 = 0xAAAA_AAAA_AAAA_AAAA;

impl Cg<'_> {
    /// clang's `trivialAutoVarInit` for the variable `d`: `None` when it
    /// gets no fill — the flag is `uninitialized`, `d` is not an
    /// automatic variable, is `constexpr` (its constant initializes
    /// everything), `[[indeterminate]]` ([dcl.attr.indet]) or
    /// `[[clang::uninitialized]]`.
    pub(crate) fn auto_init_mode(&self, d: DeclId) -> Option<AutoVarInit> {
        let mode = self.opts.auto_var_init;
        if mode == AutoVarInit::Uninitialized {
            return None;
        }
        let ast = self.ast;
        let v = ast.decls.var(d)?;
        if v.spec.is_constexpr() || !is_local(ast, d) || is_static_local(ast, d) {
            return None;
        }
        let marked = ast.redecls_of(d).any(|r| {
            ast.lists.iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r)).any(|a| {
                matches!(ast.attrs.get(a).kind, AttrKind::INDETERMINATE | AttrKind::UNINITIALIZED)
            })
        });
        (!marked).then_some(mode)
    }

    /// clang `initializeWhatIsTechnicallyUninitialized`: writes the
    /// erroneous value of the automatic variable `d` at `addr` — all-zero
    /// bytes or clang's pattern, padding included — before its
    /// initialization (none, a trivial default constructor, or one that
    /// may leave bytes unwritten; [`Cg::auto_init_prefill`]), unless
    /// [`Cg::auto_init_mode`] says no fill or `d` is larger than
    /// `-ftrivial-auto-var-init-max-size` (then a `-Rauto-var-init`
    /// remark). An integer, pointer, floating or vector type gets one
    /// store, anything else clang's `emitStoresForConstant` (a `memset`,
    /// a `memset` and a few stores, or a `memcpy` from a private
    /// `__const.<function>.<variable>`); each store and call carries
    /// clang's `!annotation !{!"auto-init"}`.
    pub(crate) fn auto_var_init(&mut self, d: DeclId, addr: Addr) {
        let Some(mode) = self.auto_init_mode(d) else { return };
        if self.f.fir.cur().is_none() {
            return;
        }
        let ast = self.ast;
        let Some(q) = ast.decls.get(d).ty() else { return };
        let size = self.alloc_size(addr.ty);
        if size == 0 {
            return;
        }
        let max = self.opts.auto_var_init_max_size;
        if max > 0 && size > max {
            let name = ast.decls.get(d).name().as_ident().map_or("", |n| ast.symbols().as_str(n));
            let span = Span::empty(ast.decls.get(d).loc());
            let r = Diag::new(DiagId::cg_auto_var_init_too_large, span)
                .arg(format!("'{name}'"))
                .arg(size)
                .arg(max);
            self.diags.push(r);
            return;
        }
        // A reference's slot is not volatile whatever its referee.
        let vol = self.referee(q).is_none() && self.is_volatile(q);
        let pattern = mode == AutoVarInit::Pattern;
        let t = addr.ty;
        let mark = self.fill_mark();
        if self.single_store_type(t) {
            let k = self.fill_value(pattern, t);
            self.store(V::C(k), addr.v, addr.align, vol);
        } else {
            let k = self.fill_value(pattern, t);
            let k = self.with_padding(pattern, k);
            // Only fails on an unreachable lowering error; the fill is
            // then left out.
            let _ = self.stores_for_constant(d, addr, k, vol);
        }
        self.annotate_fill(mark);
    }

    /// clang `EmitAutoVarInit` for a non-constant initializer `init` of
    /// the automatic variable `d` of type `q`: the erroneous value is
    /// written first ([`Cg::auto_var_init`]) unless `q` is a scalar type
    /// (clang `isScalarType`: arithmetic, enumeration, pointer, member
    /// pointer, complex) whose initializer does not name `d` itself
    /// (`isAccessedBy`), since the initializer then writes every byte.
    pub(crate) fn auto_init_prefill(&mut self, d: DeclId, q: QualType, addr: Addr, init: ExprId) {
        if self.auto_init_mode(d).is_none() {
            return;
        }
        let ast = self.ast;
        let scalar = self.referee(q).is_none()
            && ast.types.as_atomic(q).is_none()
            && !self.is_aggregate(q)
            && self.m.types.vector_info(addr.ty).is_none();
        if scalar && !self.init_names(d, init) {
            return;
        }
        self.auto_var_init(d, addr);
    }

    /// clang `EmitAutoVarInit` for a constant initializer `k` of the
    /// automatic variable `d`: a non-zero constant is written with its
    /// undefined parts (a union's tail) as the erroneous value
    /// (`replaceUndef`) and its padding as zero bytes (`constWithPadding`
    /// with `IsPattern::No`: brace initialization zeroes padding as
    /// static initialization does).
    pub(crate) fn auto_init_constant(&mut self, d: DeclId, k: Value) -> Value {
        let Some(mode) = self.auto_init_mode(d) else { return k };
        if self.const_is_null(k) {
            return k;
        }
        let k = self.replace_undef(mode == AutoVarInit::Pattern, k);
        self.with_padding(false, k)
    }

    /// clang `emitZeroOrPatternForAutoVarInit` for the variable length
    /// array `d` of type `q` at `addr` (whose IR type is the first
    /// non-variable element type): zero is one `memset` of the run-time
    /// size, the pattern a loop copying the element's pattern from a
    /// private constant into each element (an empty array is skipped).
    /// `-ftrivial-auto-var-init-max-size` does not apply.
    pub(crate) fn auto_var_init_vla(&mut self, d: DeclId, q: QualType, addr: Addr) -> R<()> {
        let Some(mode) = self.auto_init_mode(d) else { return Ok(()) };
        if self.f.fir.cur().is_none() {
            return Ok(());
        }
        let (count, elem) = self.vla_size(q)?;
        let (esize, ealign) = self.size_align(elem);
        if esize == 0 {
            return Ok(());
        }
        let vol = self.is_volatile(q);
        let scaled = |s: &mut Self| {
            if esize == 1 {
                count
            } else {
                let n = s.i64c(i128::from(esize));
                s.bin(BinOp::Mul, Flags::NUW, count, n)
            }
        };
        let kind = self.m.md_kind("annotation");
        let node = self.auto_init_md();
        if mode == AutoVarInit::Zero {
            let size = scaled(self);
            self.memset_v(addr, 0, size, vol);
            if let Some(i) = self.last_inst() {
                self.f.fir.add_md(i, kind, node);
            }
            return Ok(());
        }
        let zero = self.i64c(0);
        let empty = self.icmp(IntPredicate::Eq, count, zero);
        let setup = self.f.fir.create_block();
        let body = self.f.fir.create_block();
        let after = self.f.fir.create_block();
        self.cond_br(empty, after, setup);
        self.emit_block(setup, false);
        let size = scaled(self);
        let end = self.gep(LTy::I8, addr.v, vec![size], true);
        let Some(origin) = self.f.fir.cur() else { return Ok(()) };
        self.emit_block(body, false);
        let cur = self.f.fir.push(Op::Phi { ty: LTy::PTR, inc: vec![(addr.v, origin)] }, LTy::PTR);
        let k = self.fill_value(true, addr.ty);
        let k = self.with_padding(true, k);
        let g = self.auto_init_global(d, k, ealign);
        let dst = Addr { v: cur, ty: LTy::I8, align: min_align(addr.align, esize) };
        let c = self.memcpy(dst, V::C(gval(g)), ealign, esize, vol);
        self.f.fir.add_md(Fir::last_inst(c), kind, node);
        let step = self.i64c(i128::from(esize));
        let next = self.gep(LTy::I8, cur, vec![step], true);
        let done = self.icmp(IntPredicate::Eq, next, end);
        self.cond_br(done, after, body);
        self.f.fir.add_incoming(Fir::last_inst(cur), next, body);
        self.emit_block(after, false);
        Ok(())
    }

    /// clang `initializeAlloca` (`CGBuiltin.cpp`) for the memory `p` of a
    /// `__builtin_alloca` or `__builtin_alloca_with_align` (and the
    /// library `alloca`) of `size` bytes at alignment `align`: one
    /// `llvm.memset` of the run-time size to zero or to the `0xAA`
    /// pattern byte, at the alloca's alignment, with `!annotation
    /// !{!"auto-init"}`. The `_uninitialized` builtins never get here.
    /// `-ftrivial-auto-var-init-max-size` does not apply, nor does
    /// `[[clang::uninitialized]]` on a variable the pointer initializes.
    pub(crate) fn auto_init_alloca(&mut self, p: V, size: V, align: u64) {
        let byte = match self.opts.auto_var_init {
            AutoVarInit::Uninitialized => return,
            AutoVarInit::Zero => 0,
            AutoVarInit::Pattern => PATTERN_BYTE,
        };
        if self.f.fir.cur().is_none() {
            return;
        }
        self.memset_v(Addr { v: p, ty: LTy::I8, align }, byte, size, false);
        let kind = self.m.md_kind("annotation");
        let node = self.auto_init_md();
        if let Some(i) = self.last_inst() {
            self.f.fir.add_md(i, kind, node);
        }
    }

    /// The private `__const.<function>.<variable>` holding `k` (clang
    /// `createUnnamedGlobalFrom` for a local).
    pub(crate) fn auto_init_global(
        &mut self,
        d: DeclId,
        k: Value,
        align: u64,
    ) -> kiln_llvm::GlobalId {
        let fname = self.f.func.map(|f| self.m.func(f).name().to_owned()).unwrap_or_default();
        let name = format!("__const.{fname}.{}", kiln_abi::mangle::decl_name(self.ast, d));
        self.private_const(&name, k, align)
    }

    /// clang `emitStoresForConstant`'s `canDoSingleStore`.
    fn single_store_type(&self, t: LTy) -> bool {
        let types = &self.m.types;
        types.is_int_or_int_vector(t)
            || types.is_ptr_or_ptr_vector(t)
            || types.is_float_or_float_vector(t)
    }

    /// The fill of IR type `t`: zero, or clang's `initializationPatternFor`
    /// (a scalable vector, which kiln cannot spell as a constant splat,
    /// is zero-filled).
    fn fill_value(&mut self, pattern: bool, t: LTy) -> Value {
        if !pattern {
            return self.m.const_zero(t);
        }
        self.pattern_for(t)
    }

    /// clang `initializationPatternFor`: `0xAA` bytes for an integer
    /// (any width) and a pointer (`inttoptr`), an all-ones negative quiet
    /// NaN for a floating type, element-wise for vectors, arrays and
    /// structures (padding untouched; [`Cg::with_padding`]).
    fn pattern_for(&mut self, t: LTy) -> Value {
        if let Some(w) = self.m.types.int_width(t) {
            if w <= 128 {
                let v = i128::from_ne_bytes([PATTERN_BYTE; 16]);
                return self.m.const_int(t, v);
            }
            let words = vec![PATTERN_WORD; w.div_ceil(64) as usize];
            return self.m.const_int_words(t, &words);
        }
        if self.m.types.is_ptr(t) {
            let i = self.m.const_int(LTy::I64, i128::from(PATTERN_WORD as i64));
            return self
                .m
                .const_cast(CastOp::IntToPtr, i, t)
                .unwrap_or_else(|_| self.m.const_zero(t));
        }
        if self.m.types.is_float(t) {
            return self.m.const_float_bits(t, u128::MAX);
        }
        if let Some((e, n, scalable)) = self.m.types.vector_info(t) {
            if scalable {
                return self.m.const_zero(t);
            }
            let p = self.pattern_for(e);
            return self
                .m
                .const_vector(&vec![p; n as usize])
                .unwrap_or_else(|_| self.m.const_zero(t));
        }
        if let Some((e, n)) = self.m.types.array_info(t) {
            let p = self.pattern_for(e);
            return self
                .m
                .const_array(e, &vec![p; n as usize])
                .unwrap_or_else(|_| self.m.const_zero(t));
        }
        if let Some(fields) = self.m.types.struct_fields(t).map(<[LTy]>::to_vec) {
            let elems: Vec<Value> = fields.into_iter().map(|f| self.pattern_for(f)).collect();
            return self.m.const_struct(t, &elems).unwrap_or_else(|_| self.m.const_zero(t));
        }
        self.m.const_zero(t)
    }

    /// `n` padding bytes: `0xAA` or zero.
    fn pad_bytes(&mut self, pattern: bool, n: u64) -> Value {
        if pattern {
            self.m.const_bytes(&vec![PATTERN_BYTE; n as usize])
        } else {
            let t = self.m.types.array(LTy::I8, n);
            self.m.const_zero(t)
        }
    }

    /// clang `constWithPadding`: `k` with each structure's padding (between
    /// fields and at the tail) spelled as `[N x i8]` fields of pattern or
    /// zero bytes, in a literal structure type; an array whose element
    /// type changes is rebuilt; anything else is unchanged.
    fn with_padding(&mut self, pattern: bool, k: Value) -> Value {
        let t = self.m.const_type(k);
        if let Some(fields) = self.m.types.struct_fields(t).map(<[LTy]>::to_vec) {
            let elems: Vec<Value> = match self.m.const_of(k).cloned() {
                Some(kiln_llvm::ConstData::Struct { elems, .. }) => elems.to_vec(),
                Some(kiln_llvm::ConstData::Zero(_)) => {
                    fields.iter().map(|&f| self.m.const_zero(f)).collect()
                }
                Some(kiln_llvm::ConstData::Undef(_) | kiln_llvm::ConstData::Poison(_)) => {
                    fields.iter().map(|&f| self.m.const_undef(f)).collect()
                }
                _ => return k,
            };
            if elems.len() != fields.len() {
                return k;
            }
            let packed = self.m.types.struct_is_packed(t);
            let offs = self.struct_field_offsets(t);
            let total = self.alloc_size(t);
            let mut vals = Vec::with_capacity(fields.len());
            let mut so_far = 0;
            let mut intact = true;
            for (i, (&f, &e)) in fields.iter().zip(&elems).enumerate() {
                let off = offs[i];
                if so_far < off {
                    vals.push(self.pad_bytes(pattern, off - so_far));
                }
                let ne = self.with_padding(pattern, e);
                intact &= ne == e;
                vals.push(ne);
                so_far = off + self.alloc_size(f);
            }
            if so_far < total {
                vals.push(self.pad_bytes(pattern, total - so_far));
            }
            if intact && vals.len() == fields.len() {
                return k;
            }
            return self.m.const_anon_struct(&vals, packed).unwrap_or(k);
        }
        if let Some((e, n)) = self.m.types.array_info(t) {
            if n == 0 {
                return k;
            }
            let vals: Vec<Value> = match self.m.const_of(k).cloned() {
                Some(kiln_llvm::ConstData::Array { elems, .. }) => {
                    elems.iter().map(|&x| self.with_padding(pattern, x)).collect()
                }
                Some(kiln_llvm::ConstData::Zero(_)) => {
                    let z = self.m.const_zero(e);
                    vec![self.with_padding(pattern, z); n as usize]
                }
                _ => return k,
            };
            let ne = self.m.const_type(vals[0]);
            if ne == e {
                return k;
            }
            return self.m.const_array(ne, &vals).unwrap_or(k);
        }
        k
    }

    /// clang `replaceUndef`: each `undef` (or `poison`) part of `k` becomes
    /// the pattern or zero.
    fn replace_undef(&mut self, pattern: bool, k: Value) -> Value {
        use kiln_llvm::ConstData as C;
        match self.m.const_of(k).cloned() {
            Some(C::Undef(t) | C::Poison(t)) => self.fill_value(pattern, t),
            Some(C::Struct { ty, elems }) => {
                let n: Vec<Value> = elems.iter().map(|&x| self.replace_undef(pattern, x)).collect();
                if n[..] == elems[..] { k } else { self.m.const_struct(ty, &n).unwrap_or(k) }
            }
            Some(C::Array { ty, elems }) => {
                let n: Vec<Value> = elems.iter().map(|&x| self.replace_undef(pattern, x)).collect();
                let Some((e, _)) = self.m.types.array_info(ty) else { return k };
                if n[..] == elems[..] { k } else { self.m.const_array(e, &n).unwrap_or(k) }
            }
            Some(C::Vector { elems, .. }) => {
                let n: Vec<Value> = elems.iter().map(|&x| self.replace_undef(pattern, x)).collect();
                if n[..] == elems[..] { k } else { self.m.const_vector(&n).unwrap_or(k) }
            }
            _ => k,
        }
    }

    /// clang `isAccessedBy`: the initializer `e` names the variable `d`.
    fn init_names(&self, d: DeclId, e: ExprId) -> bool {
        let ast = self.ast;
        let mut work = vec![e];
        while let Some(x) = work.pop() {
            if ast.exprs.get(x).kind() == kiln_ast::expr::ExprKind::DeclRefExpr
                && ast.exprs.decl(x) == d
            {
                return true;
            }
            ast.exprs.children_into(x, &ast.lists, &mut work);
        }
        false
    }

    /// `!{!"auto-init"}`.
    fn auto_init_md(&mut self) -> kiln_llvm::MetadataId {
        let s = self.m.md_string(b"auto-init");
        self.m.md_tuple(&[Some(s)])
    }

    /// The current block and its length, before a fill.
    fn fill_mark(&self) -> Option<(Blk, usize)> {
        let b = self.f.fir.cur()?;
        Some((b, self.f.fir.block_insts(b).len()))
    }

    /// Marks each store and call emitted since `mark` with clang's
    /// `!annotation !{!"auto-init"}` (`addAutoInitAnnotation`).
    fn annotate_fill(&mut self, mark: Option<(Blk, usize)>) {
        let Some((b, from)) = mark else { return };
        let insts: Vec<u32> = self.f.fir.block_insts(b).get(from..).unwrap_or_default().to_vec();
        let kind = self.m.md_kind("annotation");
        let node = self.auto_init_md();
        for i in insts {
            if matches!(self.f.fir.op(i), Op::Store { .. } | Op::Call { .. }) {
                self.f.fir.add_md(i, kind, node);
            }
        }
    }

    /// The last instruction of the current block.
    fn last_inst(&self) -> Option<u32> {
        self.f.fir.cur().and_then(|b| self.f.fir.block_insts(b).last().copied())
    }
}
