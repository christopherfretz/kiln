//! Constant emission (spec/codegen.md §6): clang's `ConstantEmitter`.
//!
//! The Oven freezes constants as scalars or byte images (the object
//! representation in target layout plus relocations), so an aggregate is
//! rebuilt from its bytes, walking the IR type of the object: integers and
//! floats are read little-endian, pointers come from the relocations, and
//! arrays get clang's `EmitArrayConstant` shape (trailing zero elements
//! folded into a `zeroinitializer` filler).

use kiln_abi::irtype::IrType;
use kiln_ast::consts::{ConstValue, NullKind, PtrBase, Reloc, Scalar};
use kiln_ast::decl::{DeclKind, VarStorage};
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::{ConstValueId, DeclId, ExprId, QualType};
use kiln_llvm::{ConstData, GepFlags, IntValue, TypeId as LTy, Value};

use crate::fold::is_local;
use crate::module::{Cg, R, err, gval};

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
                let it = self.tys.convert_mem(ty);
                let bytes = img.bytes.to_vec();
                let relocs = img.relocs.to_vec();
                let img = Img { bytes, relocs };
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
            ConstValue::Aggregate(_) => err(None, "aggregate constant in scalar context"),
        }
    }

    /// A scalar constant of IR type `t`.
    fn scalar_const(&mut self, s: Scalar<'_>, t: LTy) -> R<Value> {
        match s {
            Scalar::Int { bits, .. } => {
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
            PtrBase::Temporary(_) => {
                err(None, "pointers to lifetime-extended temporaries are not supported yet (M3)")
            }
            PtrBase::TypeInfo(_) => err(None, "typeid constants are not supported yet (M3)"),
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
        let init = self.const_value(c, ty, None, true)?;
        let align = self.global_align(ty);
        let g = self.private_const(".constant", init, align);
        self.const_objects.insert(c, g);
        Ok(g)
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
            DeclKind::Var => {
                let st = ast.decls.var(d).map(|v| v.spec.storage());
                if is_local(ast, d) && st == Some(VarStorage::Static) {
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
                let inits: Vec<ExprId> = match e {
                    Some(x) if self.ast.exprs.get(x).kind() == ExprKind::InitListExpr => {
                        let l = self.ast.exprs.init_list_inits(x);
                        self.ast.lists.get(l).iter().map(|&i| ExprId::new(i)).collect()
                    }
                    _ => Vec::new(),
                };
                let mut elems = Vec::with_capacity(*n as usize);
                for i in 0..*n {
                    let sub = inits.get(i as usize).copied();
                    elems.push(self.image_const(img, off + i * es, et, sub)?);
                }
                if is_string {
                    let elt = self.ir(et);
                    return self.m.const_array(elt, &elems).map_err(|e| ir_err(&e));
                }
                let elt = self.ir(et);
                self.array_const(t, elt, &elems)
            }
            IrType::Record(d) => {
                let rec = self.tys.record(*d).clone();
                if rec.is_union {
                    return self.union_const(img, off, t, rec.size, e);
                }
                if !rec.bitfields.is_empty() {
                    return err(None, "bit-field constants are not supported yet (M3)");
                }
                let offs = self.tys.struct_offsets(&rec.members, rec.packed);
                let mut by_member: Vec<Option<DeclId>> = vec![None; rec.members.len()];
                for (&f, &i) in &rec.field_index {
                    if let Some(s) = by_member.get_mut(i as usize) {
                        *s = Some(f);
                    }
                }
                let e = e.map(|x| self.strip_init(x));
                let inits: Vec<ExprId> = match e {
                    Some(x) if self.ast.exprs.get(x).kind() == ExprKind::InitListExpr => {
                        let l = self.ast.exprs.init_list_inits(x);
                        self.ast.lists.get(l).iter().map(|&i| ExprId::new(i)).collect()
                    }
                    _ => Vec::new(),
                };
                let fields: Vec<DeclId> = self
                    .ast
                    .decls
                    .members(self.ast.first(*d))
                    .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
                    .collect();
                let mut elems = Vec::with_capacity(rec.members.len());
                for (i, m) in rec.members.iter().enumerate() {
                    let sub = by_member[i]
                        .and_then(|f| fields.iter().position(|&x| x == f))
                        .and_then(|p| inits.get(p).copied());
                    if by_member[i].is_none() {
                        let mt = self.ir(m);
                        if matches!(m, IrType::Array(..) | IrType::Int(8)) {
                            elems.push(self.m.const_zero(mt));
                            continue;
                        }
                        return err(
                            None,
                            "base-class subobjects in constants are not supported yet (M3)",
                        );
                    }
                    elems.push(self.image_const(img, off + offs[i], m, sub)?);
                }
                // clang's ConstantAggregateBuilder: the named type when the
                // element types match it, else a literal struct.
                let want = self.m.types.struct_fields(t).map(<[LTy]>::to_vec).unwrap_or_default();
                let got: Vec<LTy> = elems.iter().map(|&v| self.m.const_type(v)).collect();
                if want == got {
                    self.m.const_struct(t, &elems).map_err(|e| ir_err(&e))
                } else {
                    self.m.const_anon_struct(&elems, rec.packed).map_err(|e| ir_err(&e))
                }
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
            IrType::Void | IrType::Vector(..) => {
                err(None, "vector constants are not supported yet (M3)")
            }
        }
    }

    /// A union constant (clang `ConstStructBuilder` for unions): the active
    /// member then `undef` padding, as the union's IR type when the element
    /// types match it, else a literal struct.
    fn union_const(
        &mut self,
        img: &Img,
        off: u64,
        t: LTy,
        size: u64,
        e: Option<ExprId>,
    ) -> R<Value> {
        let e = e.map(|x| self.strip_init(x));
        let ile = e.filter(|&x| self.ast.exprs.get(x).kind() == ExprKind::InitListExpr);
        let active = ile.and_then(|x| self.ast.exprs.init_list(x).union_field);
        let Some(f) = active else {
            if (0..size).all(|i| img.byte(off + i) == 0) && img.relocs.is_empty() {
                return Ok(self.m.const_zero(t));
            }
            return err(None, "union constant without an active member");
        };
        let sub = ile.and_then(|x| {
            let l = self.ast.exprs.init_list_inits(x);
            self.ast.lists.get(l).first().map(|&i| ExprId::new(i))
        });
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

    /// clang `EmitArrayConstant` over fully expanded elements.
    fn array_const(&mut self, t: LTy, elt: LTy, elems: &[Value]) -> R<Value> {
        let bound = elems.len();
        let mut nz = bound;
        while nz > 0 && self.is_null_value(elems[nz - 1]) {
            nz -= 1;
        }
        if nz == 0 {
            return Ok(self.m.const_zero(t));
        }
        let common = elems.iter().all(|&v| self.m.const_type(v) == elt);
        let tz = bound - nz;
        if tz >= 8 {
            let mut out = Vec::new();
            if common && nz >= 8 {
                out.push(self.m.const_array(elt, &elems[..nz]).map_err(|e| ir_err(&e))?);
            } else {
                out.extend_from_slice(&elems[..nz]);
            }
            let ft = self.m.types.array(elt, tz as u64);
            out.push(self.m.const_zero(ft));
            return self.m.const_anon_struct(&out, true).map_err(|e| ir_err(&e));
        }
        if common {
            return self.m.const_array(elt, elems).map_err(|e| ir_err(&e));
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
struct Img {
    bytes: Vec<u8>,
    relocs: Vec<Reloc>,
}

impl Img {
    fn byte(&self, i: u64) -> u8 {
        self.bytes.get(i as usize).copied().unwrap_or(0)
    }

    fn reloc(&self, at: u64) -> Option<ConstValueId> {
        self.relocs.iter().find(|r| u64::from(r.at) == at).map(|r| r.value)
    }
}

pub(crate) fn ir_err(e: &kiln_llvm::IrError) -> crate::CodegenError {
    crate::CodegenError { loc: None, msg: format!("constant emission failed: {e:?}") }
}
