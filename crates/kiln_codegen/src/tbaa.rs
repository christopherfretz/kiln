//! Type-based alias analysis metadata (clang `CodeGenTBAA`): the
//! `!tbaa` access tags on scalar loads and stores and the `!tbaa.struct`
//! descriptions of aggregate copies, emitted at `-O1` and up unless
//! `-fno-strict-aliasing` ([basic.lval]/11). spec/codegen.md §3, "TBAA".
//!
//! The node shapes are clang's "old" struct-path format:
//!
//! - root `!{!"Simple C++ TBAA"}`;
//! - scalar `!{!"name", !parent, i64 0}` (`omnipotent char` under the
//!   root, every other scalar under `omnipotent char`, `vtable pointer`
//!   under the root);
//! - struct `!{!"_ZTS...", !member, i64 offset, ...}`;
//! - access tag `!{!base, !access, i64 offset}`.
//!
//! At `-fclang-compat` below 22 every pointer and reference is `any
//! pointer`; from 22 (clang 20+ `-fpointer-tbaa`) a pointer to a builtin
//! or a named class is `p<depth> <pointee>` under `any p<depth> pointer`.

use kiln_abi::mangle;
use kiln_ast::decl::DeclKind;
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, QualType, TypeId};
use kiln_llvm::{MdKindId, MetadataId, TypeId as LTy};
use kiln_source::fx::FxHashMap;

use crate::fir::{Fir, V};
use crate::module::Cg;

/// How an lvalue's accesses are described (clang `TBAAAccessInfo`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tb {
    /// The access type's own tag (`MakeAddrLValue`'s default).
    #[default]
    Type,
    /// `omnipotent char`: a member of a union, or of a may-alias object.
    MayAlias,
    /// A member access path: the outermost class `base` (its definition)
    /// and the byte offset of the accessed object in it.
    Path { base: DeclId, off: u64 },
    /// No tag (bit-field storage units).
    None,
}

impl Tb {
    /// The access info of a subobject that starts a new path (an array
    /// element, a base class subobject): clang `getTBAAInfoForSubobject`.
    pub(crate) fn sub(self) -> Tb {
        if self == Tb::MayAlias { Tb::MayAlias } else { Tb::Type }
    }
}

/// The per-module TBAA node caches.
#[derive(Default)]
pub(crate) struct TbaaCache {
    root: Option<MetadataId>,
    char_node: Option<MetadataId>,
    vtable: Option<MetadataId>,
    /// `any pointer`, `any p2 pointer`, ... by depth - 1.
    any_ptr: Vec<MetadataId>,
    types: FxHashMap<TypeId, MetadataId>,
    structs: FxHashMap<DeclId, Option<MetadataId>>,
    copies: FxHashMap<TypeId, Option<MetadataId>>,
    kinds: Option<(MdKindId, MdKindId)>,
}

impl Cg<'_> {
    /// TBAA is emitted (clang: `OptimizationLevel > 0 && !RelaxedAliasing`).
    pub(crate) fn tbaa_on(&self) -> bool {
        self.opts.optimize && self.opts.strict_aliasing
    }

    fn tbaa_kinds(&mut self) -> (MdKindId, MdKindId) {
        if let Some(k) = self.tbaa.kinds {
            return k;
        }
        let k = (self.m.md_kind("tbaa"), self.m.md_kind("tbaa.struct"));
        self.tbaa.kinds = Some(k);
        k
    }

    fn md_i64(&mut self, v: u64) -> MetadataId {
        let c = self.m.const_int(LTy::I64, i128::from(v));
        let Ok(m) = self.m.md_const(c) else { unreachable!("integer metadata") };
        m
    }

    fn tbaa_root(&mut self) -> MetadataId {
        if let Some(r) = self.tbaa.root {
            return r;
        }
        let s = self.m.md_string(b"Simple C++ TBAA");
        let r = self.m.md_tuple(&[Some(s)]);
        self.tbaa.root = Some(r);
        r
    }

    /// `!{!"name", !parent, i64 0}` (clang `createScalarTypeNode`).
    fn tbaa_scalar(&mut self, name: &[u8], parent: MetadataId) -> MetadataId {
        let s = self.m.md_string(name);
        let z = self.md_i64(0);
        self.m.md_tuple(&[Some(s), Some(parent), Some(z)])
    }

    /// `omnipotent char` (clang `getChar`).
    fn tbaa_char(&mut self) -> MetadataId {
        if let Some(c) = self.tbaa.char_node {
            return c;
        }
        let root = self.tbaa_root();
        let c = self.tbaa_scalar(b"omnipotent char", root);
        self.tbaa.char_node = Some(c);
        c
    }

    /// `any pointer` at depth 1, `any p<depth> pointer` below it.
    fn tbaa_any_ptr(&mut self, depth: usize) -> MetadataId {
        while self.tbaa.any_ptr.len() < depth {
            let d = self.tbaa.any_ptr.len() + 1;
            let n = if d == 1 {
                let c = self.tbaa_char();
                self.tbaa_scalar(b"any pointer", c)
            } else {
                let parent = self.tbaa.any_ptr[d - 2];
                self.tbaa_scalar(format!("any p{d} pointer").as_bytes(), parent)
            };
            self.tbaa.any_ptr.push(n);
        }
        self.tbaa.any_ptr[depth - 1]
    }

    /// The scalar type node of `q` (clang `CodeGenTBAA::getTypeInfo`).
    pub(crate) fn tbaa_type(&mut self, q: QualType) -> MetadataId {
        let t = self.ast.types.canonical(q).ty();
        if let Some(&n) = self.tbaa.types.get(&t) {
            return n;
        }
        let n = self.tbaa_type_uncached(QualType::unqual(t));
        self.tbaa.types.insert(t, n);
        n
    }

    fn tbaa_type_uncached(&mut self, q: QualType) -> MetadataId {
        let ast = self.ast;
        match ast.types.canonical_view(q) {
            TypeView::Builtin(b) => match tbaa_builtin_name(b) {
                Some(name) => {
                    let c = self.tbaa_char();
                    self.tbaa_scalar(name.as_bytes(), c)
                }
                None => self.tbaa_char(),
            },
            // clang names a `_BitInt` by its width alone.
            TypeView::BitInt { width, .. } => {
                let c = self.tbaa_char();
                self.tbaa_scalar(format!("_BitInt({width})").as_bytes(), c)
            }
            // [basic.lval]/11.3: `std::byte` aliases everything; an
            // enumeration without external linkage is `char`.
            TypeView::Enum(d) if !self.in_std(d, "byte") && mangle::type_externally_visible(ast, q) => {
                let name = mangle::mangle_rtti(ast, &mut self.anon, q, "_ZTS");
                let c = self.tbaa_char();
                self.tbaa_scalar(name.as_bytes(), c)
            }
            TypeView::Pointer(_) | TypeView::LValueRef(_) | TypeView::RValueRef(_) => self.tbaa_pointer(q),
            TypeView::Record(rd) if self.tbaa_valid_base(rd).is_some() => {
                self.tbaa_struct(rd).unwrap_or_else(|| self.tbaa_char())
            }
            _ => self.tbaa_char(),
        }
    }

    /// A pointer or reference type's node.
    fn tbaa_pointer(&mut self, q: QualType) -> MetadataId {
        if self.opts.clang_compat < 22 {
            return self.tbaa_any_ptr(1);
        }
        // clang 20+ `-fpointer-tbaa`: count the pointer levels, looking
        // through arrays and qualifiers ([conv.qual]/2 similar types).
        let ast = self.ast;
        let mut depth = 0usize;
        let mut t = q;
        loop {
            let p = match ast.types.canonical_view(t) {
                TypeView::Pointer(p) => p,
                TypeView::LValueRef(p) | TypeView::RValueRef(p) if depth == 0 => p,
                _ => break,
            };
            depth += 1;
            t = p;
            while let Some(e) = self.array_elem(t) {
                t = e;
            }
        }
        self.tbaa_pointer_to(depth, t)
    }

    /// The node of a `depth`-level pointer whose innermost pointee is `t`
    /// (clang 22 `-fpointer-tbaa`).
    fn tbaa_pointer_to(&mut self, depth: usize, t: QualType) -> MetadataId {
        if self.opts.clang_compat < 22 {
            return self.tbaa_any_ptr(1);
        }
        let ast = self.ast;
        let name = match ast.types.canonical_view(t) {
            TypeView::Builtin(BuiltinType::Void) => None,
            TypeView::Builtin(b) => Some(tbaa_builtin_name(b).unwrap_or("omnipotent char").to_owned()),
            TypeView::Record(rd) if !ast.decls.get(rd).name().is_empty() => {
                let u = QualType::unqual(ast.types.canonical(t).ty());
                Some(mangle::mangle_rtti(ast, &mut self.anon, u, "_ZTS"))
            }
            _ => None,
        };
        let any = self.tbaa_any_ptr(depth);
        match name {
            Some(n) => self.tbaa_scalar(format!("p{depth} {n}").as_bytes(), any),
            None => any,
        }
    }

    /// The definition of class `rd` if it can be a struct-path base
    /// (clang `isValidBaseType`): a complete non-union class without a
    /// flexible array member.
    fn tbaa_valid_base(&self, rd: DeclId) -> Option<DeclId> {
        let def = self.class_def(rd)?;
        if self.is_union(def) || self.has_flexible_array(def) {
            return None;
        }
        Some(def)
    }

    fn has_flexible_array(&self, def: DeclId) -> bool {
        let ast = self.ast;
        ast.members(def)
            .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
            .last()
            .and_then(|m| ast.decls.get(m).ty())
            .is_some_and(|q| matches!(ast.types.canonical_view(q), TypeView::IncompleteArray(_)))
    }

    /// The struct type node of class `rd` (clang `getBaseTypeInfo`).
    pub(crate) fn tbaa_struct(&mut self, rd: DeclId) -> Option<MetadataId> {
        let def = self.tbaa_valid_base(rd)?;
        if let Some(&n) = self.tbaa.structs.get(&def) {
            return n;
        }
        // A recursive reference cannot occur: a class cannot contain
        // itself, and pointers are scalars.
        let n = self.tbaa_struct_uncached(def);
        self.tbaa.structs.insert(def, n);
        n
    }

    fn tbaa_member_node(&mut self, q: QualType) -> MetadataId {
        match self.ast.types.canonical_view(q) {
            TypeView::Record(rd) if self.tbaa_valid_base(rd).is_some() => {
                self.tbaa_struct(rd).unwrap_or_else(|| self.tbaa_char())
            }
            _ => self.tbaa_type(q),
        }
    }

    fn tbaa_struct_uncached(&mut self, def: DeclId) -> Option<MetadataId> {
        let ast = self.ast;
        let (bases, offs) = {
            let l = self.tys.layouter().record(def);
            (l.bases.to_vec(), l.fields.to_vec())
        };
        // Non-virtual, non-empty bases by offset, then the fields.
        let mut parts: Vec<(u64, MetadataId)> = Vec::new();
        for (b, off) in bases {
            let Some(bdef) = self.class_def(b) else { continue };
            if self.tys.layouter().is_empty(bdef) {
                continue;
            }
            let node = self.tbaa_member_node(self.class_type(bdef));
            parts.push((off, node));
        }
        parts.sort_by_key(|p| p.0);
        let fields: Vec<DeclId> =
            ast.members(def).filter(|&m| ast.decls.get(m).kind() == DeclKind::Field).collect();
        for (i, f) in fields.into_iter().enumerate() {
            let Some(fq) = ast.decls.get(f).ty() else { continue };
            let data = ast.decls.field(f);
            let bitfield = data.is_some_and(|d| d.bit_width.is_some());
            // `isZeroSize` (a zero-width bit-field is unnamed) and
            // unnamed bit-fields.
            if bitfield && ast.decls.get(f).name().is_empty() {
                continue;
            }
            if self.tbaa_zero_size(f, fq) {
                continue;
            }
            let node = self.tbaa_member_node(fq);
            parts.push((offs.get(i).copied().unwrap_or(0) / 8, node));
        }
        let name = mangle::mangle_class_special(ast, &mut self.anon, "_ZTS", def);
        let mut ops = vec![Some(self.m.md_string(name.as_bytes()))];
        for (off, node) in parts {
            ops.push(Some(node));
            ops.push(Some(self.md_i64(off)));
        }
        Some(self.m.md_tuple(&ops))
    }

    /// clang `FieldDecl::isZeroSize` for a non-bit-field: an empty
    /// `[[no_unique_address]]` member ([intro.object]/8).
    fn tbaa_zero_size(&mut self, f: DeclId, fq: QualType) -> bool {
        let ast = self.ast;
        let nua = ast.decls.field(f).is_some_and(kiln_ast::decl::FieldData::no_unique_address)
            || self.fn_attr(f, kiln_ast::attr::AttrKind::NO_UNIQUE_ADDRESS).is_some();
        nua && ast
            .types
            .as_record(fq)
            .and_then(|r| self.class_def(r))
            .is_some_and(|d| self.tys.layouter().is_empty(d))
    }

    /// The access tag of an access of type `q` described by `tb` (clang
    /// `getAccessTagInfo`); `None` when untagged.
    pub(crate) fn tbaa_tag(&mut self, q: QualType, tb: Tb) -> Option<MetadataId> {
        let (base, access, off) = match tb {
            Tb::None => return None,
            Tb::MayAlias => {
                let c = self.tbaa_char();
                (c, c, 0)
            }
            Tb::Type => {
                let t = self.tbaa_type(q);
                (t, t, 0)
            }
            Tb::Path { base, off } => {
                let t = self.tbaa_type(q);
                match self.tbaa_struct(base) {
                    Some(b) => (b, t, off),
                    None => (t, t, 0),
                }
            }
        };
        let o = self.md_i64(off);
        Some(self.m.md_tuple(&[Some(base), Some(access), Some(o)]))
    }

    /// Attaches the access tag of an access of type `q` to the load or
    /// store `inst` (clang `DecorateInstructionWithTBAA`).
    pub(crate) fn tbaa_mark(&mut self, inst: V, q: QualType, tb: Tb) {
        if !self.tbaa_on() || !matches!(inst, V::I(_)) {
            return;
        }
        if !crate::builtins::is_complete(self.ast, q) {
            return;
        }
        if let Some(tag) = self.tbaa_tag(q, tb) {
            let (k, _) = self.tbaa_kinds();
            self.f.fir.add_md(Fir::last_inst(inst), k, tag);
        }
    }

    /// Tags the prolog store of the implicit `this` parameter of a member
    /// of class `rd` (clang `EmitParmDecl` on the `this` declaration).
    pub(crate) fn tbaa_this_store(&mut self, inst: V, rd: DeclId) {
        if !self.tbaa_on() || !matches!(inst, V::I(_)) {
            return;
        }
        let q = self.class_type(rd);
        let node = self.tbaa_pointer_to(1, q);
        self.tbaa_attach_scalar(inst, node);
    }

    /// Tags the prolog store of the implicit VTT parameter (`void **`).
    pub(crate) fn tbaa_vtt_store(&mut self, inst: V) {
        if !self.tbaa_on() || !matches!(inst, V::I(_)) {
            return;
        }
        let node = if self.opts.clang_compat < 22 { self.tbaa_any_ptr(1) } else { self.tbaa_any_ptr(2) };
        self.tbaa_attach_scalar(inst, node);
    }

    /// Tags a load or store of a vtable pointer (clang
    /// `getTBAAVTablePtrAccessInfo`).
    pub(crate) fn tbaa_vptr(&mut self, inst: V) {
        if !self.tbaa_on() || !matches!(inst, V::I(_)) {
            return;
        }
        let n = match self.tbaa.vtable {
            Some(n) => n,
            None => {
                let root = self.tbaa_root();
                let n = self.tbaa_scalar(b"vtable pointer", root);
                self.tbaa.vtable = Some(n);
                n
            }
        };
        self.tbaa_attach_scalar(inst, n);
    }

    /// Attaches the scalar access tag `{node, node, 0}` to `inst`.
    fn tbaa_attach_scalar(&mut self, inst: V, node: MetadataId) {
        let o = self.md_i64(0);
        let tag = self.m.md_tuple(&[Some(node), Some(node), Some(o)]);
        let (k, _) = self.tbaa_kinds();
        self.f.fir.add_md(Fir::last_inst(inst), k, tag);
    }

    /// The access path of field `field` of an object described by `base`
    /// (clang `EmitLValueForField`): a union member or a vector-typed
    /// member may alias anything, a bit-field is untagged, and any other
    /// member extends the path of its outermost class.
    pub(crate) fn tb_field(&mut self, base: Tb, field: DeclId, fq: QualType) -> Tb {
        if !self.tbaa_on() {
            return Tb::Type;
        }
        let ast = self.ast;
        let Some(rd) = ast.decls.get(field).ctx() else { return Tb::Type };
        if base == Tb::MayAlias
            || self.is_union(rd)
            || matches!(ast.types.canonical_view(fq), TypeView::Vector { .. })
        {
            return Tb::MayAlias;
        }
        let Ok(off) = self.field_byte_offset(field) else { return Tb::Type };
        match base {
            Tb::Path { base, off: o } => Tb::Path { base, off: o + off },
            _ => match self.tbaa_valid_base(rd) {
                Some(def) => Tb::Path { base: def, off },
                None => Tb::Type,
            },
        }
    }

    /// Attaches `!tbaa.struct` to the aggregate copy `call` of an object
    /// of type `q` (clang `getTBAAStructInfo`).
    pub(crate) fn tbaa_struct_copy(&mut self, call: V, q: QualType) {
        if !self.tbaa_on() || !matches!(call, V::I(_)) {
            return;
        }
        let t = self.ast.types.canonical(q).ty();
        let node = match self.tbaa.copies.get(&t) {
            Some(&n) => n,
            None => {
                let mut fields = Vec::new();
                let n = if self.tbaa_collect(0, QualType::unqual(t), &mut fields) {
                    let mut ops = Vec::with_capacity(fields.len() * 3);
                    for (off, size, tag) in fields {
                        ops.push(Some(self.md_i64(off)));
                        ops.push(Some(self.md_i64(size)));
                        ops.push(Some(tag));
                    }
                    Some(self.m.md_tuple(&ops))
                } else {
                    None
                };
                self.tbaa.copies.insert(t, n);
                n
            }
        };
        if let Some(n) = node {
            let (_, k) = self.tbaa_kinds();
            self.f.fir.add_md(Fir::last_inst(call), k, n);
        }
    }

    /// clang `CodeGenTBAA::CollectFields`: `(offset, size, tag)` of each
    /// scalar of `q`; false for a class it does not describe (one with
    /// bases or a flexible array member).
    fn tbaa_collect(&mut self, at: u64, q: QualType, out: &mut Vec<(u64, u64, MetadataId)>) -> bool {
        let ast = self.ast;
        let TypeView::Record(rd) = ast.types.canonical_view(q) else {
            let size = self.size_align(q).0;
            let Some(tag) = self.tbaa_tag(q, Tb::Type) else { return false };
            out.push((at, size, tag));
            return true;
        };
        let Some(def) = self.class_def(rd) else { return false };
        if self.is_union(def) {
            let size = self.size_align(q).0;
            let Some(tag) = self.tbaa_tag(q, Tb::MayAlias) else { return false };
            out.push((at, size, tag));
            return true;
        }
        if self.has_flexible_array(def) {
            return false;
        }
        let has_bases = {
            let l = self.tys.layouter().record(def);
            !l.bases.is_empty() || !l.vbases.is_empty()
        };
        if has_bases {
            return false;
        }
        let offs = self.tys.layouter().record(def).fields.to_vec();
        let fields: Vec<DeclId> =
            ast.members(def).filter(|&m| ast.decls.get(m).kind() == DeclKind::Field).collect();
        for (i, f) in fields.into_iter().enumerate() {
            let Some(fq) = ast.decls.get(f).ty() else { continue };
            let off = at + offs.get(i).copied().unwrap_or(0) / 8;
            let bitfield = ast.decls.field(f).is_some_and(|d| d.bit_width.is_some());
            if bitfield {
                // One `char` entry per storage unit, at the run's first
                // bit-field (little-endian targets).
                let Some(bf) = self.tys.record(def).bitfields.get(&f).copied() else { continue };
                if bf.size == 0 || bf.offset != 0 {
                    continue;
                }
                let size = u64::from(bf.storage_size).div_ceil(8);
                let Some(tag) = self.tbaa_tag(fq, Tb::MayAlias) else { return false };
                out.push((off, size, tag));
                continue;
            }
            if self.tys.is_empty_record_for_layout(fq) {
                continue;
            }
            if !self.tbaa_collect(off, fq, out) {
                return false;
            }
        }
        true
    }

    /// clang 20+: `!llvm.errno.tbaa`, the `int` access tag the optimizer
    /// gives `errno`.
    pub(crate) fn tbaa_errno(&mut self) {
        if self.tbaa_on() && self.opts.clang_compat >= 22 {
            let Some(tag) = self.tbaa_tag(QualType::INT, Tb::Type) else { return };
            self.m.add_named_metadata("llvm.errno.tbaa", &[tag]);
        }
    }
}

/// clang's scalar node name of builtin `b`; `None` for the character
/// types, which are `omnipotent char` ([basic.lval]/11.3). An unsigned
/// type shares its signed counterpart's node ([basic.lval]/11.2).
fn tbaa_builtin_name(b: BuiltinType) -> Option<&'static str> {
    use BuiltinType as B;
    Some(match b {
        B::Char | B::SChar | B::UChar => return None,
        B::UShort => "short",
        B::UInt => "int",
        B::ULong => "long",
        B::ULongLong => "long long",
        B::UInt128 => "__int128",
        _ => b.name(),
    })
}
