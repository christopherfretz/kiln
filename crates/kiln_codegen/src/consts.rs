//! Constant emission (spec/codegen.md §6): clang's `ConstantEmitter`.
//!
//! The Oven freezes constants as scalars or byte images (the object
//! representation in target layout plus relocations), so an aggregate is
//! rebuilt from its bytes, walking the IR type of the object: integers and
//! floats are read little-endian, pointers come from the relocations, and
//! arrays get clang's `EmitArrayConstant` shape (trailing zero elements
//! folded into a `zeroinitializer` filler).

use kiln_abi::irtype::IrType;
use kiln_ast::consts::{ConstValue, NullKind, PtrBase, Reloc, Scalar, UnionMember};
use kiln_ast::decl::{DeclKind, VarStorage};
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::{ConstValueId, DeclId, ExprId, QualType};
use kiln_llvm::{ConstData, GepFlags, IntValue, TypeId as LTy, Value};

use crate::fold::{is_local, is_static_local};
use crate::module::{Cg, R, err, gval};

/// What [`Cg::enter_default_init`] replaced.
pub(crate) type DefaultInitSave =
    (Option<ExprId>, Option<(Option<crate::fir::V>, Option<DeclId>)>);

impl Cg<'_> {
    /// Lowers the folded constant `c` of type `ty` in its in-memory form
    /// (clang `tryEmitPrivateForMemory`); `e` is the expression it was
    /// folded from, which decides the shape of string-literal arrays.
    pub(crate) fn const_value(
        &mut self,
        c: ConstValueId,
        ty: QualType,
        e: Option<ExprId>,
        top: bool,
    ) -> R<Value> {
        let _ = top;
        match self.ast.consts.get(c) {
            ConstValue::Scalar(s) => {
                let mt = self.mty(ty);
                self.scalar_const(s, mt)
            }
            ConstValue::Aggregate(img) => {
                let it = self.const_ir_type(ty);
                let bytes = img.bytes.to_vec();
                let relocs = img.relocs.to_vec();
                let unions = img.unions.to_vec();
                let img = Img::new(bytes, relocs, unions);
                self.image_const(&img, 0, &it, e)
            }
        }
    }

    /// Lowers the folded constant `c` of type `ty` as a scalar value
    /// (clang `emitAbstract`; `bool` is `i1`).
    pub(crate) fn const_scalar_value(&mut self, c: ConstValueId, ty: QualType) -> R<Value> {
        match self.ast.consts.get(c) {
            ConstValue::Scalar(Scalar::Bool { value, .. }) => Ok(self.m.const_bool(value)),
            ConstValue::Scalar(s) => {
                let t = self.lty(ty);
                self.scalar_const(s, t)
            }
            // A vector value (scalar in IR, an aggregate image in the
            // Oven): its `<N x T>` constant (clang `emitAbstract`).
            ConstValue::Aggregate(_) if self.is_vector_ty(ty) => {
                self.const_value(c, ty, None, true)
            }
            ConstValue::Aggregate(_) => err(None, "aggregate constant in scalar context"),
        }
    }

    /// The IR type of a constant of type `ty`: its memory type, except
    /// that a `bool` ext vector (also as an array element) is its value
    /// type `<N x i1>` (clang
    /// `ConstantEmitter::emitForMemory` keeps the vector constant; its
    /// memory type is an integer).
    fn const_ir_type(&mut self, ty: QualType) -> IrType {
        match self.ast.types.canonical_view(ty) {
            kiln_ast::types::TypeView::Vector { elem, .. }
                if matches!(
                    self.ast.types.canonical_view(elem),
                    kiln_ast::types::TypeView::Builtin(kiln_ast::types::BuiltinType::Bool)
                ) =>
            {
                self.tys.convert(ty)
            }
            kiln_ast::types::TypeView::ConstantArray(a) => match self.const_ir_type(a.elem) {
                et if matches!(et, IrType::Vector(..)) || has_bool_vec(&et) => {
                    IrType::Array(Box::new(et), self.ast.types.array_size(ty).unwrap_or(0))
                }
                _ => self.tys.convert_mem(ty),
            },
            _ => self.tys.convert_mem(ty),
        }
    }

    /// The memory form of constant type `it`: a `bool` vector is the
    /// integer of its size (clang `ConvertTypeForMem`, `<4 x i1>` is
    /// `i8`), in arrays of any depth.
    fn bool_vec_mem_form(&mut self, it: &IrType) -> IrType {
        match it {
            IrType::Vector(et, _) if matches!(**et, IrType::Int(1)) => {
                IrType::Int((self.tys.size_align(it).0 * 8) as u32)
            }
            IrType::Array(et, n) => IrType::Array(Box::new(self.bool_vec_mem_form(et)), *n),
            _ => it.clone(),
        }
    }

    /// True if `ty` is a vector type (a `bool` ext vector's value form
    /// is `<N x i1>`).
    fn is_vector_ty(&mut self, ty: QualType) -> bool {
        matches!(self.tys.convert(ty), IrType::Vector(..))
    }

    /// An integer narrower than its memory type `t` (a `_BitInt(N)` in
    /// its ABI-sized `iM`, or the byte array of one whose `iN` allocates
    /// a different size): extended by its signedness to the load/store
    /// width and, for a byte array, split into its bytes (clang
    /// `ConstantEmitter::emitForMemory`; the targets are little-endian).
    /// `None` when `t` is an integer of the value's own width.
    fn extended_int_const(
        &mut self,
        bits: &kiln_ast::consts::IntBits<'_>,
        signed: bool,
        t: LTy,
    ) -> Option<R<Value>> {
        let width = bits.width();
        let total = match (self.m.types.int_width(t), self.m.types.array_info(t)) {
            (Some(w), _) if w > width => w,
            (None, Some((LTy::I8, n))) => u32::try_from(n * 8).ok()?,
            _ => return None,
        };
        let n = total.div_ceil(64) as usize;
        let mut w = vec![0u64; n];
        for (d, s) in w.iter_mut().zip(bits.words()) {
            *d = *s;
        }
        let top = width.checked_sub(1)?;
        let neg = signed && w.get((top / 64) as usize).is_some_and(|x| (x >> (top % 64)) & 1 == 1);
        for i in 0..total {
            let (wi, b) = ((i / 64) as usize, i % 64);
            if i >= width && neg {
                w[wi] |= 1 << b;
            } else if i >= width {
                w[wi] &= !(1 << b);
            }
        }
        if self.m.types.int_width(t).is_some() {
            return Some(Ok(self.m.const_int_words(t, &w)));
        }
        // clang's `ConstantAggregateBuilder` shape for the bytes
        // (`<{ i8 3, [23 x i8] zeroinitializer }>`).
        let elems: Vec<Value> = (0..(total / 8) as usize)
            .map(|i| self.m.const_int(LTy::I8, i128::from((w[i / 8] >> ((i % 8) * 8)) as u8)))
            .collect();
        Some(self.array_const(t, LTy::I8, &elems, elems.len()))
    }

    /// A scalar constant of IR type `t`.
    pub(crate) fn scalar_const(&mut self, s: Scalar<'_>, t: LTy) -> R<Value> {
        match s {
            Scalar::Int { bits, signed, .. } => {
                if let Some(v) = self.extended_int_const(&bits, signed, t) {
                    return v;
                }
                if let Some(v) = bits.to_u128() {
                    if self.m.types.int_width(t).is_some() {
                        return Ok(self.m.const_int(t, v as i128));
                    }
                    if self.m.types.is_ptr(t) {
                        let i = self.m.const_int(LTy::I64, v as i128);
                        return self
                            .m
                            .const_cast(kiln_llvm::CastOp::IntToPtr, i, t)
                            .map_err(|e| ir_err(&e));
                    }
                    return err(None, "integer constant of non-integer type");
                }
                let w = bits.words().to_vec();
                Ok(self.m.const_int_words(t, &w))
            }
            Scalar::Bool { value, .. } => {
                if t == LTy::I1 {
                    Ok(self.m.const_bool(value))
                } else {
                    Ok(self.m.const_int(t, i128::from(value)))
                }
            }
            Scalar::Float { bits, .. } => Ok(self.m.const_float_bits(t, bits)),
            Scalar::Null { kind: NullKind::Pointer, .. } => Ok(self.m.const_null(LTy::PTR)),
            Scalar::Null { kind: NullKind::MemberData, .. } => Ok(self.m.const_int(t, -1)),
            Scalar::Null { kind: NullKind::MemberFunction, .. } => Ok(self.m.const_zero(t)),
            Scalar::MemberPtr { member, kind, derived, path, .. } => {
                self.mp_value(member, kind, derived, path, t)
            }
            Scalar::Pointer { base, offset, .. } => {
                let b = self.ptr_base(base)?;
                if offset == 0 {
                    return Ok(b);
                }
                let off = self.m.const_int(LTy::I64, i128::from(offset));
                self.m.const_gep(LTy::I8, b, &[off], GepFlags::NONE, None).map_err(|e| ir_err(&e))
            }
            Scalar::Reflection { .. } => err(None, "reflection values cannot be emitted"),
        }
    }

    /// The address a constant pointer is based on.
    fn ptr_base(&mut self, base: PtrBase) -> R<Value> {
        match base {
            PtrBase::Decl(d) => self.decl_addr_const(d),
            PtrBase::Function(d) => Ok(Value::func(self.get_fn(d)?)),
            PtrBase::StringLit { lit, .. } => Ok(gval(self.lit_global(lit))),
            PtrBase::Const(c) => Ok(gval(self.const_object(c)?)),
            PtrBase::Temporary(t) => {
                let ty = self.ast.decls.get(t).ty().unwrap_or(QualType::INT);
                Ok(gval(self.let_global(t, ty)?))
            }
            PtrBase::TypeInfo(t) => self.rtti_descriptor(t),
            PtrBase::Label { label, origin } => {
                let at = self.loc_of(origin);
                self.addr_of_label(label, at)
            }
        }
    }

    /// The private `.constant` global holding frozen constant object
    /// `c` (clang `GetAddrOfUnnamedGlobalConstantDecl`: the object of a
    /// `__builtin_source_location()`); one per value. Its initializer is
    /// built first, so the strings it points to are named before it.
    fn const_object(&mut self, c: ConstValueId) -> R<kiln_llvm::GlobalId> {
        if let Some(&g) = self.const_objects.get(&c) {
            return Ok(g);
        }
        let ty = self.ast.consts.ty(c);
        if let Some(g) = self.const_chars(c, ty) {
            self.const_objects.insert(c, g);
            return Ok(g);
        }
        let init = self.const_value(c, ty, None, true)?;
        let align = self.global_align(ty);
        let g = self.private_const(".constant", init, align);
        self.const_objects.insert(c, g);
        Ok(g)
    }

    /// A constant character array (the object of P3491's
    /// `reflect_constant_string`/`define_static_string`, or a
    /// `reflect_constant_array` of characters): a string-pool global
    /// (`.str.N`, internal; one per content, shared with an equal string
    /// literal's, as either object may be potentially non-unique,
    /// [intro.object]/9), not a `.constant`. WP-479.
    fn const_chars(&mut self, c: ConstValueId, ty: QualType) -> Option<kiln_llvm::GlobalId> {
        use kiln_ast::types::{BuiltinType, TypeView};
        let ast = self.ast;
        let TypeView::ConstantArray(a) = ast.types.canonical_view(ty) else { return None };
        let unit = match ast.types.builtin(ast.types.canonical(a.elem))? {
            BuiltinType::Char | BuiltinType::Char8 => 1,
            BuiltinType::Char16 => 2,
            BuiltinType::Char32 => 4,
            BuiltinType::WChar => self.size_align(a.elem).0,
            _ => return None,
        };
        let img = ast.consts.aggregate(c)?;
        if !img.relocs.is_empty() || unit == 0 || !(img.bytes.len() as u64).is_multiple_of(unit) {
            return None;
        }
        let n = img.bytes.len() as u64 / unit;
        Some(self.string_global_named(img.bytes, unit, n, ".str"))
    }

    /// The value of source-location builtin `e` ([support.srcloc.cons]):
    /// the values sema recorded for it, or for the default argument use
    /// enclosing it.
    pub(crate) fn source_loc_value(&mut self, e: ExprId) -> R<Value> {
        use kiln_ast::expr::SourceLocKind as K;
        let ast = self.ast;
        let key = self.srcloc.unwrap_or(e);
        let Some(v) = ast.exprs.source_loc_values(key).copied() else {
            return err(None, "source-location builtin without recorded values");
        };
        let cv = match ast.exprs.source_loc_kind(e) {
            K::Line | K::Column => {
                let n = if ast.exprs.source_loc_kind(e) == K::Line { v.line } else { v.column };
                let t = self.lty(ast.exprs.get(e).ty());
                return Ok(self.m.const_int(t, i128::from(n)));
            }
            K::File => v.file,
            K::FileName => v.file_name,
            K::Function => v.function,
            K::SourceLocStruct => match v.location {
                Some(c) => c,
                None => return err(None, "source-location object without recorded value"),
            },
        };
        match ast.consts.scalar(cv) {
            Some(s) => self.scalar_const(s, LTy::PTR),
            None => err(None, "source-location value is not a pointer"),
        }
    }

    /// The value of `ConstantExpr` `e`: sema's, or for an immediate
    /// invocation inside a default argument the value the prepass folded
    /// for the current use ([`crate::fold::Folds::imm`]).
    pub(crate) fn immediate_value(&self, e: ExprId) -> Option<ConstValueId> {
        self.ast.exprs.constant_value(e).or_else(|| self.folds.imm.get(&(e, self.srcloc)).copied())
    }

    /// The address glvalue `ConstantExpr` `e` designates, when it has a
    /// value: the Oven freezes a glvalue immediate invocation as that
    /// address (`Oven::evaluate`, WP-640; clang `EmitLValue` of a
    /// `ConstantExpr` with an `APValue` result).
    pub(crate) fn immediate_address(&mut self, e: ExprId) -> R<Option<Value>> {
        let ast = self.ast;
        let Some(c) = self.immediate_value(e) else { return Ok(None) };
        match ast.consts.scalar(c) {
            Some(s @ Scalar::Pointer { .. }) => self.scalar_const(s, LTy::PTR).map(Some),
            _ => Ok(None),
        }
    }

    /// Enters default argument `e` for emission: the source-location
    /// builtins in it take the values of this use unless an enclosing
    /// default argument already set them. Returns the scope to restore.
    pub(crate) fn enter_default_arg(&mut self, e: ExprId) -> Option<ExprId> {
        let saved = self.srcloc;
        if saved.is_none() && self.ast.exprs.source_loc_values(e).is_some() {
            self.srcloc = Some(e);
        }
        saved
    }

    /// [`Self::enter_default_arg`] for a `CXXDefaultInitExpr`, whose
    /// `this` is the aggregate being initialized when one is
    /// (`FuncState::default_init_this`; clang `CXXDefaultInitExprScope`).
    pub(crate) fn enter_default_init(&mut self, e: ExprId) -> DefaultInitSave {
        let src = self.enter_default_arg(e);
        let this = self.f.default_init_this.map(|(v, rd)| {
            let old = (self.f.this, self.f.this_class);
            self.f.this = Some(v);
            self.f.this_class = Some(rd);
            old
        });
        (src, this)
    }

    /// Ends [`Self::enter_default_init`].
    pub(crate) fn exit_default_init(&mut self, s: DefaultInitSave) {
        self.srcloc = s.0;
        if let Some((this, class)) = s.1 {
            self.f.this = this;
            self.f.this_class = class;
        }
    }

    /// The global holding string literal `lit`, terminator included.
    pub(crate) fn lit_global(&mut self, lit: kiln_ast::StringLitId) -> kiln_llvm::GlobalId {
        self.lit_global_sized(lit, 0)
    }

    /// The string literal's constant zero-padded to at least `min_bytes`
    /// (clang gives a literal initializing a longer array that array's
    /// type).
    pub(crate) fn lit_global_sized(
        &mut self,
        lit: kiln_ast::StringLitId,
        min_bytes: u64,
    ) -> kiln_llvm::GlobalId {
        let unit = u64::from(self.ast.strings.encoding(lit).unit_bytes());
        let mut bytes = self.ast.strings.bytes(lit).to_vec();
        let total = self.ast.strings.object_bytes(lit).max(min_bytes);
        bytes.resize(total as usize, 0);
        self.string_global(&bytes, unit, total / unit)
    }

    /// The constant address of a variable or function.
    pub(crate) fn decl_addr_const(&mut self, d: DeclId) -> R<Value> {
        let ast = self.ast;
        match ast.decls.get(d).kind() {
            DeclKind::Function => Ok(Value::func(self.get_fn(d)?)),
            DeclKind::TemplateParamObject => Ok(gval(self.param_object_global(d)?)),
            DeclKind::Var => {
                let st = ast.decls.var(d).map(|v| v.spec.storage());
                if is_static_local(ast, d) {
                    Ok(gval(self.static_local(d)?))
                } else if is_local(ast, d) && st != Some(VarStorage::Extern) {
                    err(Some(ast.decls.get(d).loc()), "constant address of a local variable")
                } else {
                    Ok(gval(self.get_gvar(d)?))
                }
            }
            _ => {
                err(Some(ast.decls.get(d).loc()), "constant address of an unsupported declaration")
            }
        }
    }

    /// The constant of IR type `it` stored at byte `off` of `img`.
    fn image_const(&mut self, img: &Img, off: u64, it: &IrType, e: Option<ExprId>) -> R<Value> {
        let t = self.ir(it);
        if let Some(v) = self.member_ptr_reloc(img, off, it, t)? {
            return Ok(v);
        }
        // clang `VisitImplicitValueInitExpr`: a value-initialized `bool`
        // vector, array of them or class is `EmitNullConstant`, the null
        // of its memory type (`i8` for `<4 x i1>`, `%struct.P` for a class
        // whose explicit constant is a literal struct of `<N x i1>`).
        if let Some(x) = e
            && self.ast.exprs.get(self.strip_init(x)).kind() == ExprKind::ImplicitValueInitExpr
            && (has_bool_vec(it) || matches!(it, IrType::Record(_)))
        {
            let m = self.bool_vec_mem_form(it);
            let v = self.image_const(img, off, &m, None)?;
            if self.is_null_value(v) {
                let mt = self.ir(&m);
                return Ok(self.m.const_zero(mt));
            }
            return Ok(v);
        }
        match it {
            IrType::Int(w) => {
                let n = u64::from(*w).div_ceil(8);
                if *w <= 128 {
                    let mut v: u128 = 0;
                    for i in 0..n {
                        v |= u128::from(img.byte(off + i)) << (8 * i);
                    }
                    Ok(self.m.const_int(t, v as i128))
                } else {
                    let mut words = vec![0u64; n.div_ceil(8) as usize];
                    for i in 0..n {
                        words[(i / 8) as usize] |= u64::from(img.byte(off + i)) << (8 * (i % 8));
                    }
                    Ok(self.m.const_int_words(t, &words))
                }
            }
            IrType::Half
            | IrType::BFloat
            | IrType::Float
            | IrType::Double
            | IrType::X86Fp80
            | IrType::Fp128 => {
                let n = match it {
                    IrType::Half | IrType::BFloat => 2,
                    IrType::Float => 4,
                    IrType::Double => 8,
                    IrType::X86Fp80 => 10,
                    _ => 16,
                };
                let mut v: u128 = 0;
                for i in 0..n {
                    v |= u128::from(img.byte(off + i)) << (8 * i);
                }
                Ok(self.m.const_float_bits(t, v))
            }
            IrType::Ptr => match img.reloc(off) {
                Some(r) => match self.ast.consts.get(r) {
                    ConstValue::Scalar(s) => self.scalar_const(s, LTy::PTR),
                    ConstValue::Aggregate(_) => err(None, "malformed pointer relocation"),
                },
                None => Ok(self.m.const_null(LTy::PTR)),
            },
            IrType::Array(et, n) => {
                let (es, _) = self.tys.size_align(et);
                let e = e.map(|x| self.strip_init(x));
                let is_string =
                    e.is_some_and(|x| self.ast.exprs.get(x).kind() == ExprKind::StringLiteral);
                // clang `EmitArrayInitialization`: an array initialized from
                // a list without explicit elements has only a filler, so
                // its constant has no common element type
                // (`EmitArrayConstant`); a value-initialized one is
                // `EmitNullConstant`'s plain array.
                let (inits, filler, no_elts, list) = match e {
                    Some(x)
                        if matches!(
                            self.ast.exprs.get(x).kind(),
                            ExprKind::InitListExpr | ExprKind::CXXParenListInitExpr
                        ) =>
                    {
                        let (inits, filler, _) = crate::expr::agg_init_parts(self.ast, x);
                        let none = inits.is_empty();
                        (inits, filler, none, true)
                    }
                    _ => (Vec::new(), None, false, false),
                };
                // The elements clang's `CommonElementType` looks at: the
                // list's explicit ones (all of them without a list).
                let ninit = if list { inits.len().min(*n as usize) } else { *n as usize };
                // `EmitArrayConstant`'s desired type is the memory form
                // (`ConvertType` of the array): a `bool` vector element is
                // the integer of its size (`<4 x i1>` is `i8`), so the
                // trailing zero array and an all-zero array of `bool`
                // vectors are of it (a value-initialized filler is too:
                // `VisitImplicitValueInitExpr` above), and the array beside
                // explicit `<N x i1>` elements is a packed struct.
                let mem = has_bool_vec(et)
                    .then(|| (self.bool_vec_mem_form(it), self.bool_vec_mem_form(et)));
                let mut elems = Vec::with_capacity(*n as usize);
                for i in 0..*n {
                    let sub = inits.get(i as usize).copied().or(filler);
                    elems.push(self.image_const(img, off + i * es, et, sub)?);
                }
                if is_string {
                    let elt = self.ir(et);
                    return self.m.const_array(elt, &elems).map_err(|e| ir_err(&e));
                }
                let (t, elt) = match &mem {
                    Some((mt, met)) => (self.ir(mt), self.ir(met)),
                    None => (t, self.ir(et)),
                };
                self.array_const(t, elt, &elems, if no_elts { 0 } else { ninit })
            }
            IrType::Record(d) => {
                let rec = self.tys.record(*d).clone();
                if rec.is_union {
                    return self.union_const(img, off, t, (*d, rec.size), e);
                }
                let mut elems: Vec<(u64, Value)> = Vec::with_capacity(rec.members.len());
                self.record_elems(img, off, 0, *d, None, e, &mut elems)?;
                self.build_struct_const(t, rec.size, &elems)
            }
            IrType::Struct(ms, packed) => {
                let offs = self.tys.struct_offsets(ms, *packed);
                let mut elems = Vec::with_capacity(ms.len());
                for (i, m) in ms.iter().enumerate() {
                    elems.push(self.image_const(img, off + offs[i], m, None)?);
                }
                self.m.const_struct(t, &elems).map_err(|e| ir_err(&e))
            }
            IrType::RecordBase(_) => {
                err(None, "base-class subobjects in constants are not supported yet (M3)")
            }
            // clang `ConstExprEmitter`/`APValue::Vector`: one constant per
            // lane, lane `i` at byte `i * sizeof(elem)` of the image; an
            // `i1` lane (a `bool` vector) is bit `i % 8` of byte `i / 8`.
            IrType::Vector(et, n) if matches!(**et, IrType::Int(1)) => {
                let t1 = self.ir(et);
                let elems: Vec<Value> = (0..*n)
                    .map(|i| {
                        let b = img.byte(off + i / 8) >> (i % 8) & 1;
                        self.m.const_int(t1, i128::from(b))
                    })
                    .collect();
                self.m.const_vector(&elems).map_err(|e| ir_err(&e))
            }
            IrType::Vector(et, n) => {
                let (es, _) = self.tys.size_align(et);
                let mut elems = Vec::with_capacity(*n as usize);
                for i in 0..*n {
                    elems.push(self.image_const(img, off + i * es, et, None)?);
                }
                self.m.const_vector(&elems).map_err(|e| ir_err(&e))
            }
            IrType::Void | IrType::ScalableVector(..) | IrType::TargetExt(_) => {
                err(None, "scalable-vector and target-type constants are not supported yet (M3)")
            }
        }
    }

    /// clang `ConstStructBuilder::Build(APValue)`: the field constants of
    /// class `d` at byte `rel` of the object at `off`, its non-virtual
    /// bases flattened in at their offsets (address order); padding (and
    /// the byte of an empty class) is left to `build_struct_const`.
    /// `complete`: the most derived class when `d` is laid out as a base
    /// (its non-virtual part at `rel` of that object). A vtable pointer
    /// slot is the address point of subobject (`d`, `rel`) in the most
    /// derived class's vtable group (clang `ConstStructBuilder` with
    /// `getVTableAddressPoint`; the virtual bases are members of the
    /// complete object).
    #[allow(clippy::too_many_arguments)]
    fn record_elems(
        &mut self,
        img: &Img,
        off: u64,
        rel: u64,
        d: DeclId,
        complete: Option<DeclId>,
        e: Option<ExprId>,
        out: &mut Vec<(u64, Value)>,
    ) -> R<()> {
        let ast = self.ast;
        let first = ast.first(d);
        let rec = self.tys.record(first).clone();
        let as_base = complete.is_some();
        let vt_class = complete.unwrap_or(first);
        if rec.is_union && as_base {
            return err(None, "union base-class constants are not supported yet (M3)");
        }
        let (members, packed) = match (&rec.base_members, as_base) {
            (Some(b), true) => {
                let lt = self.ir(&IrType::RecordBase(first));
                (b.clone(), self.m.types.struct_is_packed(lt))
            }
            _ => (rec.members.clone(), rec.packed),
        };
        let offs = self.tys.struct_offsets(&members, packed);
        let mut by_member: Vec<Option<DeclId>> = vec![None; members.len()];
        for (&f, &i) in &rec.field_index {
            if let Some(s) = by_member.get_mut(i as usize) {
                *s = Some(f);
            }
        }
        let mut by_base: Vec<Option<DeclId>> = vec![None; members.len()];
        for (&b, &i) in &rec.base_index {
            if let Some(s) = by_base.get_mut(i as usize) {
                *s = Some(b);
            }
        }
        // The virtual bases are members of the complete object only
        // (P3533: kiln's constant evaluation makes such objects).
        let mut virt_slot: Vec<Option<DeclId>> = vec![None; members.len()];
        if !as_base {
            for (&b, &i) in &rec.vbase_index {
                if let Some(s) = virt_slot.get_mut(i as usize) {
                    *s = Some(b);
                }
            }
        }
        let e = e.map(|x| self.strip_init(x));
        let inits: Vec<ExprId> = match e {
            Some(x)
                if matches!(
                    ast.exprs.get(x).kind(),
                    ExprKind::InitListExpr | ExprKind::CXXParenListInitExpr
                ) =>
            {
                crate::expr::agg_init_parts(ast, x).0
            }
            _ => Vec::new(),
        };
        // An aggregate's list initializes its bases first ([dcl.init.aggr]
        // /7).
        let bases: Vec<DeclId> = ast.definition(d).map_or_else(Vec::new, |def| {
            ast.decls
                .bases(def, &ast.lists)
                .filter_map(|b| ast.types.as_record(b.ty).map(|r| ast.first(r)))
                .collect()
        });
        let fields: Vec<DeclId> = ast
            .decls
            .members(first)
            .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        for (i, m) in members.iter().enumerate() {
            let at = rel + offs[i];
            if let Some(f) = by_member[i] {
                // A bit-field storage unit is rebuilt from its bytes (the
                // object representation already packs the fields, in
                // target bit order), with no initializer.
                let sub = Some(f)
                    .filter(|f| !rec.bitfields.contains_key(f))
                    .and_then(|f| fields.iter().position(|&x| x == f))
                    .and_then(|p| inits.get(p + bases.len()).copied());
                // A `bool` vector member (or array of them) is its
                // `<N x i1>` constant.
                let fm = ast.decls.get(f).ty().map(|q| self.const_ir_type(q));
                let m = fm
                    .as_ref()
                    .filter(|t| matches!(t, IrType::Vector(..)) || has_bool_vec(t))
                    .unwrap_or(m);
                out.push((at, self.image_const(img, off + at, m, sub)?));
            } else if let Some(b) = by_base[i] {
                let sub = bases.iter().position(|&x| x == b).and_then(|p| inits.get(p).copied());
                self.record_elems(img, off, at, b, Some(vt_class), sub, out)?;
            } else if let Some(b) = virt_slot[i] {
                self.record_elems(img, off, at, b, Some(vt_class), None, out)?;
            } else if i == 0 && matches!(m, IrType::Ptr) && self.is_dynamic(first) {
                out.push((at, self.vtable_address_point(vt_class, first, rel)?));
            } else if !matches!(m, IrType::Array(..) | IrType::Int(8)) {
                return err(None, "class constants with this member are not supported yet");
            }
        }
        Ok(())
    }

    /// A member pointer stored in an image: the Oven records it as a
    /// relocation (a `MemberPtr` or member `Null` scalar) over its `i64`
    /// (data) or `{ i64, i64 }` (function) storage, whose bytes are 0; a
    /// null data member pointer is `-1` (Itanium).
    fn member_ptr_reloc(&mut self, img: &Img, off: u64, it: &IrType, t: LTy) -> R<Option<Value>> {
        if !matches!(it, IrType::Int(64) | IrType::Struct(..)) {
            return Ok(None);
        }
        let Some(r) = img.reloc(off) else { return Ok(None) };
        match self.ast.consts.get(r) {
            ConstValue::Scalar(
                s @ (Scalar::MemberPtr { .. }
                | Scalar::Null { kind: NullKind::MemberData | NullKind::MemberFunction, .. }),
            ) => self.scalar_const(s, t).map(Some),
            _ => Ok(None),
        }
    }

    /// clang 19 `ConstantAggregateBuilder::buildFrom` for a struct of
    /// `elems` (offset, constant) whose IR type `t` has alloc size `size`:
    /// the natural unpacked layout when it fits (tail-padded with `undef`
    /// bytes), else a packed one with explicit padding; the named type
    /// when layout-identical, else a literal struct.
    fn build_struct_const(&mut self, t: LTy, size: u64, fields: &[(u64, Value)]) -> R<Value> {
        // `ConstantAggregateBuilder::add`, appending: padding fills a gap
        // the natural layout would leave; a misaligned element makes the
        // layout unnatural.
        let mut natural = true;
        let mut elems: Vec<(u64, Value)> = Vec::with_capacity(fields.len());
        let mut end = 0u64;
        for &(o, v) in fields {
            let (s, a) = self.ir_layout(self.m.const_type(v));
            let aligned = end.next_multiple_of(a);
            if aligned > o || !o.is_multiple_of(a) {
                natural = false;
            } else if aligned < o {
                let pad = self.padding_const(o - end);
                elems.push((end, pad));
            }
            elems.push((o, v));
            end = o + s;
        }
        // `buildFrom`.
        let desired = size.max(end);
        let align =
            elems.iter().map(|&(_, v)| self.ir_layout(self.m.const_type(v)).1).max().unwrap_or(1);
        let aligned = end.next_multiple_of(align);
        let mut packed = false;
        let mut unpacked: Vec<Value> = elems.iter().map(|&(_, v)| v).collect();
        if desired < aligned || !desired.is_multiple_of(align) {
            natural = false;
            packed = true;
        } else if desired > aligned {
            let pad = self.padding_const(desired - end);
            unpacked.push(pad);
        }
        let mut packed_elems = Vec::new();
        if !natural {
            let mut so_far = 0u64;
            for &(o, v) in &elems {
                let (s, a) = self.ir_layout(self.m.const_type(v));
                if so_far.next_multiple_of(a) != o {
                    packed = true;
                }
                if o != so_far {
                    let pad = self.padding_const(o - so_far);
                    packed_elems.push(pad);
                }
                packed_elems.push(v);
                so_far = o + s;
            }
            if packed && so_far < desired {
                let pad = self.padding_const(desired - so_far);
                packed_elems.push(pad);
            }
        }
        let out = if packed { packed_elems } else { unpacked };
        let got: Vec<LTy> = out.iter().map(|&v| self.m.const_type(v)).collect();
        let want = self.m.types.struct_fields(t).map(<[LTy]>::to_vec);
        if want.as_deref() == Some(&got[..]) && self.m.types.struct_is_packed(t) == packed {
            self.m.const_struct(t, &out).map_err(|e| ir_err(&e))
        } else {
            self.m.const_anon_struct(&out, packed).map_err(|e| ir_err(&e))
        }
    }

    /// The active member of the union `rd` at `off` of `img`, as the Oven
    /// recorded it (nested unions can share an offset: the member whose
    /// context is `rd`'s definition).
    fn image_union_member(&self, img: &Img, off: u64, rd: DeclId) -> Option<DeclId> {
        let def = self.ast.definition(rd).unwrap_or(rd);
        img.unions_at(off).find(|&f| {
            let ctx = self.ast.decls.get(f).ctx();
            ctx == Some(def) || ctx == Some(rd)
        })
    }

    /// clang `ConstantAggregateBuilderUtils::getPadding`.
    fn padding_const(&mut self, n: u64) -> Value {
        let t = if n > 1 { self.m.types.array(LTy::I8, n) } else { LTy::I8 };
        self.m.const_undef(t)
    }

    /// A union constant (clang `ConstStructBuilder` for unions): the active
    /// member then `undef` padding, as the union's IR type when the element
    /// types match it, else a literal struct.
    fn union_const(
        &mut self,
        img: &Img,
        off: u64,
        t: LTy,
        (rd, size): (DeclId, u64),
        e: Option<ExprId>,
    ) -> R<Value> {
        let e = e.map(|x| self.strip_init(x));
        let ile = e.filter(|&x| {
            matches!(
                self.ast.exprs.get(x).kind(),
                ExprKind::InitListExpr | ExprKind::CXXParenListInitExpr
            )
        });
        // The member the initializer list names, else the one the
        // evaluation left active (a constructor body's assignment, or a
        // union copied by its trivial `operator=`), as clang's `APValue`.
        let active = ile.and_then(|x| crate::expr::agg_init_parts(self.ast, x).2);
        let active = active.or_else(|| self.image_union_member(img, off, rd));
        let Some(f) = active else {
            if (0..size).all(|i| img.byte(off + i) == 0) && img.relocs.is_empty() {
                return Ok(self.m.const_zero(t));
            }
            return err(None, "union constant without an active member");
        };
        let sub = ile.and_then(|x| crate::expr::agg_init_parts(self.ast, x).0.first().copied());
        let fq = self.ast.decls.get(f).ty().expect("field type");
        let fit = self.tys.convert(fq);
        let c = self.image_const(img, off, &fit, sub)?;
        let (fs, _) = self.tys.size_align(&fit);
        let mut elems = vec![c];
        if size > fs {
            let pt = self.m.types.array(LTy::I8, size - fs);
            elems.push(self.m.const_undef(pt));
        }
        let want = self.m.types.struct_fields(t).map(<[LTy]>::to_vec).unwrap_or_default();
        let got: Vec<LTy> = elems.iter().map(|&v| self.m.const_type(v)).collect();
        if want == got {
            self.m.const_struct(t, &elems).map_err(|e| ir_err(&e))
        } else {
            self.m.const_anon_struct(&elems, false).map_err(|e| ir_err(&e))
        }
    }

    /// clang `EmitArrayConstant` over fully expanded elements, the first
    /// `ninit` of them initialized explicitly (0 when the value has only a
    /// filler); `t` and `elt` are the desired array and element types.
    fn array_const(&mut self, t: LTy, elt: LTy, elems: &[Value], ninit: usize) -> R<Value> {
        let bound = elems.len();
        let mut nz = bound;
        while nz > 0 && self.is_null_value(elems[nz - 1]) {
            nz -= 1;
        }
        if nz == 0 {
            return Ok(self.m.const_zero(t));
        }
        // clang's `CommonElementType`: the first element's type when each
        // of the `ninit` initialized elements has it (a polymorphic class
        // element is a literal struct); the filler must match it too
        // unless it is folded into a trailing zero array.
        let first = self.m.const_type(elems[0]);
        let init_common =
            ninit > 0 && elems[..ninit.min(bound)].iter().all(|&v| self.m.const_type(v) == first);
        let tz = bound - nz;
        if tz >= 8 {
            let mut out = Vec::new();
            if init_common && nz >= 8 && elems[..nz].iter().all(|&v| self.m.const_type(v) == first)
            {
                out.push(self.m.const_array(first, &elems[..nz]).map_err(|e| ir_err(&e))?);
            } else {
                out.extend_from_slice(&elems[..nz]);
            }
            let ft = self.m.types.array(if init_common { first } else { elt }, tz as u64);
            out.push(self.m.const_zero(ft));
            return self.m.const_anon_struct(&out, true).map_err(|e| ir_err(&e));
        }
        if init_common && elems.iter().all(|&v| self.m.const_type(v) == first) {
            return self.m.const_array(first, elems).map_err(|e| ir_err(&e));
        }
        self.m.const_anon_struct(elems, true).map_err(|e| ir_err(&e))
    }

    /// `Constant::isNullValue`.
    pub(crate) fn is_null_value(&self, v: Value) -> bool {
        match self.m.const_of(v) {
            Some(ConstData::Int { value: IntValue::Small(x), .. }) => *x == 0,
            Some(ConstData::Int { value: IntValue::Big(w), .. }) => w.iter().all(|&x| x == 0),
            Some(ConstData::Float { bits, .. }) => *bits == 0,
            Some(ConstData::Null(_) | ConstData::Zero(_)) => true,
            _ => false,
        }
    }

    /// Strips parentheses and no-op wrappers around an initializer.
    pub(crate) fn strip_init(&self, mut e: ExprId) -> ExprId {
        loop {
            let n = self.ast.exprs.get(e);
            match n.kind() {
                ExprKind::ParenExpr | ExprKind::ConstantExpr => e = self.ast.exprs.operand(e),
                ExprKind::ExprWithCleanups => e = ExprId::new(n.a()),
                ExprKind::ImplicitCastExpr if self.ast.exprs.cast_kind(e) == CastKind::NoOp => {
                    e = self.ast.exprs.operand(e);
                }
                _ => return e,
            }
        }
    }
}

/// A byte image being decomposed.
///
/// Its tables are in the canonical order `ConstStore::push_aggregate`
/// checks (`ByteImage`): `relocs` strictly increasing in `at` (they do not
/// overlap and have a non-zero width, so one starts at each offset at
/// most), `unions` strictly increasing in (`at`, `field`). The lookups
/// binary-search them, so lowering an image with F fields and R
/// relocations is O(F log R), not O(F x R) (WP-665).
struct Img {
    bytes: Vec<u8>,
    relocs: Vec<Reloc>,
    /// The active members the evaluation recorded (`UnionMember::at` is
    /// the union's offset in the image).
    unions: Vec<UnionMember>,
}

impl Img {
    fn new(bytes: Vec<u8>, relocs: Vec<Reloc>, unions: Vec<UnionMember>) -> Img {
        debug_assert!(
            relocs.windows(2).all(|w| w[0].at < w[1].at),
            "constant image relocations not sorted by offset"
        );
        debug_assert!(
            unions.windows(2).all(|w| (w[0].at, w[0].field) < (w[1].at, w[1].field)),
            "constant image union members not sorted by offset"
        );
        Img { bytes, relocs, unions }
    }

    fn byte(&self, i: u64) -> u8 {
        self.bytes.get(i as usize).copied().unwrap_or(0)
    }

    /// The relocation starting at byte `at`.
    fn reloc(&self, at: u64) -> Option<ConstValueId> {
        let i = self.relocs.binary_search_by_key(&at, |r| u64::from(r.at)).ok()?;
        Some(self.relocs[i].value)
    }

    /// The active members recorded for the unions at byte `at`, in table
    /// order.
    fn unions_at(&self, at: u64) -> impl Iterator<Item = DeclId> + '_ {
        let first = self.unions.partition_point(|u| u64::from(u.at) < at);
        self.unions[first..].iter().take_while(move |u| u64::from(u.at) == at).map(|u| u.field)
    }
}

/// Whether constant type `it` is a `bool` vector (`<N x i1>`) or an
/// array of them, at any depth.
fn has_bool_vec(it: &IrType) -> bool {
    match it {
        IrType::Vector(et, _) => matches!(**et, IrType::Int(1)),
        IrType::Array(et, _) => has_bool_vec(et),
        _ => false,
    }
}

pub(crate) fn ir_err(e: &kiln_llvm::IrError) -> crate::CodegenError {
    crate::CodegenError { loc: None, msg: format!("constant emission failed: {e:?}") }
}
