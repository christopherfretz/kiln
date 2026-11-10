//! Debug information types and scopes (clang `CGDebugInfo::getOrCreateType`,
//! `getContextDescriptor`, `CreateTypeDefinition`, `CreateCXXMemberFunction`).

use kiln_abi::mangle;
use kiln_ast::attr::AttrKind;
use kiln_ast::consts::Scalar;
use kiln_ast::decl::{
    Access, DeclFlags, DeclKind, DefinitionBits as B, FnSpec, FnStorage, TagKind, VarEvalState,
};
use kiln_ast::dump::AstNames;
use kiln_ast::lambda::CaptureKind;
use kiln_ast::names::DeclNameView;
use kiln_ast::types::{
    ArgView, BuiltinType as T, FuncProto, RefQual, TemplateArg, TypeView, write_arg_list_debug,
};
use kiln_ast::{ConstValueId, DeclId, QualType};
use kiln_llvm::{MdField, MdNode, MetadataId};

use super::{DebugInfoKind, Fields};
use crate::module::Cg;

const PTR: &str = "DW_TAG_pointer_type";

impl Cg<'_> {
    fn di_cu(&self) -> MetadataId {
        self.di.as_deref().map_or_else(|| super::unreachable_md(), |d| d.cu)
    }

    /// The identifier naming `d` (clang `getName`; empty when unnamed).
    pub(crate) fn di_ident(&self, d: DeclId) -> String {
        let ast = self.ast;
        match ast.names.view(ast.decls.get(d).name()) {
            DeclNameView::Identifier(s) => ast.symbols().as_str(s).to_owned(),
            _ => String::new(),
        }
    }

    /// `DIFlagPublic`... unless `a` is the default of the enclosing class
    /// (clang `getAccessFlag`).
    fn di_access(&self, a: Access, parent: Option<DeclId>) -> Option<&'static str> {
        let tag = parent.and_then(|p| self.ast.decls.record(p)).map(|r| r.tag);
        let default = match tag {
            Some(TagKind::Class) => Access::Private,
            Some(_) => Access::Public,
            None => Access::None,
        };
        match a {
            _ if a == default => None,
            Access::Private => Some("DIFlagPrivate"),
            Access::Protected => Some("DIFlagProtected"),
            Access::Public => Some("DIFlagPublic"),
            Access::None => None,
        }
    }

    /// The record `d` is a member of, if any.
    pub(crate) fn di_parent_record(&self, d: DeclId) -> Option<DeclId> {
        let c = self.ast.decls.get(d).ctx()?;
        matches!(
            self.ast.decls.get(c).kind(),
            DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
        )
        .then_some(c)
    }

    fn di_member_access(&self, d: DeclId) -> Option<&'static str> {
        self.di_access(self.ast.decls.get(d).misc().access(), self.di_parent_record(d))
    }

    /// clang `getDeclContextDescriptor`: the scope of `d` (the compile
    /// unit by default; `None` for an implicit declaration at file scope).
    pub(crate) fn di_scope(&mut self, d: DeclId) -> Option<MetadataId> {
        let ast = self.ast;
        let n = ast.decls.get(d);
        let default = (!n.has(DeclFlags::IMPLICIT)).then(|| self.di_cu());
        let Some(c) = n.ctx() else { return default };
        let cf = ast.first(c);
        if let Some(&s) = self.di.as_deref().and_then(|di| di.regions.get(&cf)) {
            return Some(s);
        }
        match ast.decls.get(c).kind() {
            DeclKind::Namespace => Some(self.di_namespace(cf)),
            DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization => {
                // clang `getContextDescriptor`: a dependent class (a
                // template pattern) is no scope. Sema keeps the pattern's
                // typedef sugar on a non-dependent type of an instantiated
                // member (`size_type` of `std::array<int, 3>::size`), as
                // clang does, and its typedef lands in the unit.
                let cn = ast.decls.get(c);
                if cn.has(DeclFlags::DEPENDENT_CTX)
                    || ast.described_template(c).is_some()
                    || ast.partial_spec_info(c).is_some()
                {
                    return default;
                }
                match cn.ty() {
                    Some(q) => self.di_type(q),
                    None => default,
                }
            }
            _ => default,
        }
    }

    /// [`Self::di_scope`] with the compile unit dropped (`DIBuilder`'s
    /// `getNonCompileUnitScope`).
    pub(crate) fn di_scope_nocu(&mut self, d: DeclId) -> Option<MetadataId> {
        let cu = self.di_cu();
        self.di_scope(d).filter(|&s| s != cu)
    }

    fn di_namespace(&mut self, first: DeclId) -> MetadataId {
        if let Some(&n) = self.di.as_deref().and_then(|d| d.namespaces.get(&first)) {
            return n;
        }
        let scope = self.di_scope_nocu(first);
        let inline =
            self.ast.decls.namespace(first).is_some_and(kiln_ast::decl::NamespaceData::is_inline);
        let mut f = Fields::default().nn("scope", scope);
        let name = self.di_ident(first);
        if !name.is_empty() {
            f = f.s("name", &name);
        }
        if inline {
            f = f.b("exportSymbols", true);
        }
        let n = self.m.md_specialized("DINamespace", false, f.0);
        if let Some(d) = self.di.as_deref_mut() {
            d.namespaces.insert(first, n);
        }
        n
    }

    /// clang `UnwrapTypeForDebugInfo`: sugar other than typedefs is
    /// dropped, its qualifiers kept. Alias templates are unwrapped too
    /// (clang names them as typedefs; docs/status/debuginfo.md).
    fn di_unwrap(&self, q: QualType) -> QualType {
        let ty = &self.ast.types;
        let mut quals = q.quals_bits();
        let mut t = q.ty();
        loop {
            let next = match ty.view(t) {
                TypeView::Paren(i)
                | TypeView::Elaborated { named: i, .. }
                | TypeView::Using { underlying: i, .. }
                | TypeView::SubstTemplateTypeParm { replacement: i, .. }
                | TypeView::Attributed { equivalent: i, .. }
                | TypeView::Decltype { underlying: i, .. }
                | TypeView::ExtQual { base: i, .. }
                | TypeView::TemplateSpecialization { underlying: Some(i), .. }
                | TypeView::TypeOf { underlying: Some(i), .. }
                | TypeView::UnaryTransform { result: Some(i), .. }
                | TypeView::Auto { deduced: Some(i), .. }
                | TypeView::DeducedTemplateSpecialization { deduced: Some(i), .. }
                | TypeView::PackIndexing { selected: Some(i), .. }
                | TypeView::Splice { designated: Some(i), .. } => i,
                TypeView::InjectedClassName { injected, .. } => ty.canonical(injected),
                _ => break,
            };
            quals |= next.quals_bits();
            t = next.ty();
        }
        QualType::new(t, quals)
    }

    /// clang `getOrCreateType` (`None` for `void`).
    pub(crate) fn di_type(&mut self, q: QualType) -> Option<MetadataId> {
        // clang's `DebugType` time-trace event, cache hits included; the
        // detail is the type as written (spec/driver.md "Time trace").
        let _t = kiln_source::trace::Scope::with_detail("DebugType", || {
            kiln_ast::types::print_type(
                &self.ast.types,
                self.ast,
                q,
                kiln_ast::types::PrintOptions::SUGARED,
            )
        });
        let q = self.di_unwrap(q);
        if let Some(&t) = self.di.as_deref()?.types.get(&q) {
            return t;
        }
        let t = self.di_create_type(q);
        if let Some(d) = self.di.as_deref_mut() {
            d.types.insert(q, t);
        }
        t
    }

    fn di_derived(&mut self, tag: &'static str, base: Option<MetadataId>, f: Fields) -> MetadataId {
        let mut all = Vec::with_capacity(f.0.len() + 2);
        all.push(("tag", MdField::Raw(tag.into())));
        all.push(("baseType", MdField::Node(base)));
        all.extend(f.0);
        self.m.md_specialized("DIDerivedType", false, all)
    }

    fn di_create_type(&mut self, q: QualType) -> Option<MetadataId> {
        let bits = q.quals_bits();
        if bits != 0 {
            // clang `CreateQualifiedType`: const outermost, then volatile.
            let (tag, rest) = match bits {
                b if b & 1 != 0 => ("DW_TAG_const_type", b & !1),
                b if b & 2 != 0 => ("DW_TAG_volatile_type", b & !2),
                b => ("DW_TAG_restrict_type", b & !4),
            };
            let base = self.di_type(QualType::new(q.ty(), rest));
            return Some(self.di_derived(tag, base, Fields::default()));
        }
        let bits_of = |cg: &mut Self, q: QualType| cg.size_align(q).0 * 8;
        match self.ast.types.view(q.ty()) {
            TypeView::Builtin(b) => self.di_basic(b, q),
            TypeView::BitInt { signed, .. } => {
                let name = if signed { "_BitInt" } else { "unsigned _BitInt" };
                let enc = if signed { "DW_ATE_signed" } else { "DW_ATE_unsigned" };
                let size = bits_of(self, q);
                Some(self.di_basic_node(name, size, enc))
            }
            TypeView::Pointer(p) => {
                let base = self.di_type(p);
                Some(self.di_derived(PTR, base, Fields::default().u("size", self.ptr_bits())))
            }
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                let tag = if matches!(self.ast.types.view(q.ty()), TypeView::LValueRef(_)) {
                    "DW_TAG_reference_type"
                } else {
                    "DW_TAG_rvalue_reference_type"
                };
                let base = self.di_type(p);
                Some(self.di_derived(tag, base, Fields::default().u("size", self.ptr_bits())))
            }
            TypeView::MemberPointer { pointee, class } => {
                let cls = self.di_type(QualType::unqual(class));
                let base = match self.ast.types.view(self.ast.types.canonical(pointee).ty()) {
                    TypeView::Function(fp) => {
                        let rd = self.ast.types.as_record(QualType::unqual(class));
                        self.di_instance_fn_type(&fp, rd, false)
                    }
                    _ => self.di_type(pointee),
                };
                let size = bits_of(self, q);
                let f = Fields::default().u("size", size).n("extraData", cls);
                Some(self.di_derived("DW_TAG_ptr_to_member_type", base, f))
            }
            TypeView::ConstantArray(_)
            | TypeView::IncompleteArray(_)
            | TypeView::VariableArray { .. } => self.di_array(q),
            TypeView::Vector { elem, count, .. } => {
                let base = self.di_type(elem);
                let c = if count == 0 { -1 } else { i64::from(count) };
                let sub =
                    self.m.md_specialized("DISubrange", false, Fields::default().i("count", c).0);
                let el = self.m.md_tuple(&[Some(sub)]);
                let size = bits_of(self, q);
                let f = Fields::default()
                    .raw("tag", "DW_TAG_array_type")
                    .nn("baseType", base)
                    .u("size", size)
                    .raw("flags", "DIFlagVector")
                    .n("elements", Some(el));
                Some(self.m.md_specialized("DICompositeType", false, f.0))
            }
            TypeView::Complex(e) => {
                let int = self.ast.types.is_integral(e);
                // clang's `DW_ATE_lo_user` (0x80) for an integer element
                // prints as a number: LLParser has no name for it.
                let enc = if int { "128" } else { "DW_ATE_complex_float" };
                let size = bits_of(self, q);
                Some(self.di_basic_node("complex", size, enc))
            }
            TypeView::Atomic(i) => {
                let base = self.di_type(i);
                Some(self.di_derived("DW_TAG_atomic_type", base, Fields::default()))
            }
            TypeView::Function(fp) => Some(self.di_fn_type(&fp, None)),
            TypeView::Typedef { decl, underlying } => {
                let u = self.ast.decls.typedef(decl).map_or(underlying, |t| t.underlying);
                let base = self.di_type(u);
                if self.has_fn_attr(decl, AttrKind::NODEBUG) {
                    return base;
                }
                let loc = self.ast.decls.get(decl).loc();
                let (file, line) = (self.di_file(loc), self.di_line(loc));
                let scope = self.di_scope_nocu(decl);
                let name = self.di_ident(decl);
                let mut f =
                    Fields::default().s("name", &name).n("scope", scope).n("file", Some(file));
                // An implicit typedef (`__SVInt32_t`) has no line.
                if line != 0 {
                    f = f.u("line", u64::from(line));
                }
                f = f.flags("flags", self.di_member_access(decl).as_slice());
                Some(self.di_derived("DW_TAG_typedef", base, f))
            }
            TypeView::Enum(d) => self.di_enum(d, q),
            TypeView::Record(d) => Some(self.di_record(d, q)),
            TypeView::QualifiedArray { quals, base } => {
                self.di_type(QualType::new(base, quals.bits()))
            }
            _ => None,
        }
    }

    fn ptr_bits(&self) -> u64 {
        u64::from(self.ast.types.target().pointer_bytes) * 8
    }

    fn di_basic_node(&mut self, name: &str, size: u64, enc: &'static str) -> MetadataId {
        let f = Fields::default().s("name", name).u("size", size).raw("encoding", enc);
        self.m.md_specialized("DIBasicType", false, f.0)
    }

    fn di_basic(&mut self, b: T, q: QualType) -> Option<MetadataId> {
        let tt = self.ast.types.target();
        let (signed, unsigned) = ("DW_ATE_signed", "DW_ATE_unsigned");
        let enc = match b {
            T::NullPtr => {
                let f = Fields::default()
                    .raw("tag", "DW_TAG_unspecified_type")
                    .s("name", "decltype(nullptr)");
                return Some(self.m.md_specialized("DIBasicType", false, f.0));
            }
            T::Bool => "DW_ATE_boolean",
            T::Char if tt.char_signed => "DW_ATE_signed_char",
            T::Char | T::UChar => "DW_ATE_unsigned_char",
            T::SChar => "DW_ATE_signed_char",
            T::Char8 | T::Char16 | T::Char32 => "DW_ATE_UTF",
            T::WChar => {
                if tt.wchar_signed {
                    signed
                } else {
                    unsigned
                }
            }
            _ if b.is_floating() => "DW_ATE_float",
            _ if b.is_signed_integer() => signed,
            _ if b.is_integral() => unsigned,
            // `std::meta::info` (WP-910): clang has no such type. It is a
            // builtin (kiln and GCC mangle it as a builtin, `u6__info` /
            // `Dm`, not as a class in `std::meta`), so it is a base type of
            // its spelling, pointer-sized as the handle it stands for. A
            // `DW_TAG_unspecified_type` would lose the size: LLVM's DWARF
            // writer emits only the name of one.
            T::MetaInfo => {
                let size = self.ptr_bits();
                return Some(self.di_basic_node(b.name(), size, unsigned));
            }
            _ if b.is_sizeless() => return Some(self.di_sve(b)),
            _ => return None,
        };
        let size = self.size_align(q).0 * 8;
        Some(self.di_basic_node(b.name(), size, enc))
    }

    /// clang `CreateType(const BuiltinType *)` for an SVE ACLE type
    /// (WP-910): a vector array of its element type whose upper bound is
    /// a DWARF expression of the vector granule register `VG` (`AArch64`
    /// DWARF register 46), `elements per granule * VG - 1`. A predicate
    /// counts bytes (8 lanes each); `svcount_t` is a fixed 2 bytes. The
    /// name reaches it through the implicit typedef of the builtin's
    /// spelling (`__SVInt32_t`, in the main file, no line), as in clang.
    fn di_sve(&mut self, b: T) -> MetadataId {
        let (vec, tuple) = b.sve_tuple_parts().unwrap_or((b, 1));
        // The element type and its lanes in a 128-bit granule pair.
        let (elem, lanes) = match vec {
            T::SvInt8 => (T::SChar, 16),
            T::SvInt16 => (T::Short, 8),
            T::SvInt32 => (T::Int, 4),
            T::SvInt64 => (T::Long, 2),
            T::SvUint8 => (T::UChar, 16),
            T::SvUint16 => (T::UShort, 8),
            T::SvUint32 => (T::UInt, 4),
            T::SvUint64 => (T::ULong, 2),
            T::SvFloat16 => (T::Half, 8),
            T::SvFloat32 => (T::Float, 4),
            T::SvFloat64 => (T::Double, 2),
            T::SvBfloat16 => (T::BFloat16, 8),
            // A predicate: 16 one-bit lanes per 128 bits, as bytes.
            _ => (T::UChar, 2),
        };
        let n = lanes * u64::from(tuple);
        let upper = if vec == T::SvCount {
            MdField::Int(1)
        } else {
            let ops = format!(
                "DW_OP_constu, {}, DW_OP_bregx, 46, 0, DW_OP_mul, DW_OP_constu, 1, DW_OP_minus",
                n / 2
            );
            let e =
                self.m.md_specialized("DIExpression", false, vec![("", MdField::Raw(ops.into()))]);
            MdField::Node(Some(e))
        };
        let mut sf = Fields::default().i("lowerBound", 0);
        sf.0.push(("upperBound", upper));
        let sub = self.m.md_specialized("DISubrange", false, sf.0);
        let subs = self.m.md_tuple(&[Some(sub)]);
        let base = self.di_type(elem.qual_type());
        let f = Fields::default()
            .raw("tag", "DW_TAG_array_type")
            .n("baseType", base)
            .raw("flags", "DIFlagVector")
            .n("elements", Some(subs));
        self.m.md_specialized("DICompositeType", false, f.0)
    }

    /// clang `CreateType(const ArrayType *)`: nested array types are one
    /// composite with a subrange each.
    fn di_array(&mut self, q: QualType) -> Option<MetadataId> {
        let ty = &self.ast.types;
        let size = match ty.view(q.ty()) {
            TypeView::ConstantArray(_) => self.size_align(q).0 * 8,
            _ => 0,
        };
        let mut subs = Vec::new();
        let mut el = q;
        loop {
            let (count, next) = match self.ast.types.view(el.ty()) {
                TypeView::ConstantArray(c) => (i64::try_from(c.size).unwrap_or(-1), c.elem),
                TypeView::IncompleteArray(e) | TypeView::VariableArray { elem: e, .. } => (-1, e),
                _ => break,
            };
            let s =
                self.m.md_specialized("DISubrange", false, Fields::default().i("count", count).0);
            subs.push(Some(s));
            el = next;
        }
        let base = self.di_type(el);
        let elements = self.m.md_tuple(&subs);
        let mut f = Fields::default().raw("tag", "DW_TAG_array_type").n("baseType", base);
        if size != 0 {
            f = f.u("size", size);
        }
        Some(self.m.md_specialized("DICompositeType", false, f.n("elements", Some(elements)).0))
    }

    /// clang `CreateType(const FunctionType *)`; `this_ty` is the object
    /// pointer of an instance method.
    fn di_fn_type(&mut self, fp: &FuncProto, this_ty: Option<MetadataId>) -> MetadataId {
        let mut ops = vec![self.di_type(fp.ret())];
        ops.extend(this_ty.map(Some));
        let params: Vec<QualType> = self.ast.types.params(fp).to_vec();
        for p in params {
            ops.push(self.di_type(p));
        }
        if fp.variadic() {
            ops.push(None);
        }
        let types = self.m.md_tuple(&ops);
        let flags: &[&str] = match fp.info().ref_qual() {
            RefQual::LValue => &["DIFlagLValueReference"],
            RefQual::RValue => &["DIFlagRValueReference"],
            RefQual::None => &[],
        };
        let f = Fields::default().flags("flags", flags).n("types", Some(types));
        self.m.md_specialized("DISubroutineType", false, f.0)
    }

    /// The plain `this` pointer type of a member of `rd` with
    /// method qualifiers `quals` (clang `getThisType`, cached by the
    /// pointee).
    pub(crate) fn di_this_ptr(&mut self, rd: DeclId, quals: u8) -> Option<MetadataId> {
        let rq = self.ast.decls.get(rd).ty()?;
        let pointee = self.di_type(QualType::new(rq.ty(), quals & 3));
        Some(self.di_derived(PTR, pointee, Fields::default().u("size", self.ptr_bits())))
    }

    /// clang `getOrCreateInstanceMethodType`: the function type with the
    /// artificial object pointer after the return type.
    fn di_instance_fn_type(
        &mut self,
        fp: &FuncProto,
        rd: Option<DeclId>,
        _static: bool,
    ) -> Option<MetadataId> {
        let this = match rd {
            Some(rd) => {
                let quals = fp.info().this_quals().bits();
                let p = self.di_this_ptr(rd, quals)?;
                let pointee = match &self.m.md_node(p) {
                    MdNode::Specialized { fields, .. } => {
                        fields.iter().find(|f| f.0 == "baseType").and_then(|f| match f.1 {
                            MdField::Node(n) => n,
                            _ => None,
                        })
                    }
                    _ => None,
                };
                let f = Fields::default()
                    .u("size", self.ptr_bits())
                    .raw("flags", "DIFlagArtificial | DIFlagObjectPointer");
                Some(self.di_derived(PTR, pointee, f))
            }
            None => None,
        };
        Some(self.di_fn_type(fp, this))
    }

    fn di_type_ident(&mut self, rd: Option<DeclId>, q: QualType) -> String {
        let ast = self.ast;
        let q = QualType::unqual(q.ty());
        if !mangle::type_externally_visible(ast, q) {
            return String::new();
        }
        if let Some(rd) = rd
            && self.class_def(rd).is_some()
            && self.is_dynamic(rd)
            && self.vtable_linkage(rd) == crate::vtable::VtLinkage::External
        {
            return String::new();
        }
        mangle::mangle_rtti(ast, &mut self.anon, q, "_ZTS")
    }

    fn di_enum(&mut self, d: DeclId, q: QualType) -> Option<MetadataId> {
        let ast = self.ast;
        let def = ast.definition(d);
        let decl = def.unwrap_or(d);
        let loc = ast.decls.get(decl).loc();
        let (file, line) = (self.di_file(loc), self.di_line(loc));
        let scope = self.di_scope_nocu(decl);
        let name = self.di_ident(decl);
        let ident = self.di_type_ident(None, q);
        let data = ast.decls.enumeration(decl);
        let complete = data.and_then(|e| e.int_type).is_some();
        let mut f = Fields::default()
            .raw("tag", "DW_TAG_enumeration_type")
            .s("name", &name)
            .n("scope", scope)
            .n("file", Some(file))
            .u("line", u64::from(line));
        if complete {
            f = f.u("size", self.size_align(q).0 * 8);
        }
        let (Some(_), Some(e), Some(int)) = (def, data, data.and_then(|e| e.int_type)) else {
            let f = f.raw("flags", "DIFlagFwdDecl").s("identifier", &ident);
            return Some(self.m.md_specialized("DICompositeType", false, f.0));
        };
        let unsigned = !self.is_signed(int);
        let mut els = Vec::new();
        for m in ast.decls.members(decl) {
            let v = ast.decls.enumerator(m).and_then(|x| x.value).and_then(|v| self.di_int(v));
            let Some((_, v)) = v else { continue };
            let mut ef = Fields::default().s("name", &self.di_ident(m));
            ef = if unsigned { ef.u("value", v as u64) } else { ef.i("value", v as i64) };
            if unsigned {
                ef = ef.b("isUnsigned", true);
            }
            els.push(Some(self.m.md_specialized("DIEnumerator", false, ef.0)));
        }
        let base = self.di_type(int);
        let elements = self.m.md_tuple(&els);
        let scoped = e.flags & kiln_ast::decl::EnumData::SCOPED != 0;
        f = f.n("baseType", base).flags("flags", if scoped { &["DIFlagEnumClass"] } else { &[] });
        f = f.n("elements", Some(elements)).s("identifier", &ident);
        let n = self.m.md_specialized("DICompositeType", false, f.0);
        if let Some(d) = self.di.as_deref_mut()
            && !d.enums.contains(&n)
        {
            d.enums.push(n);
        }
        Some(n)
    }

    /// An integral or `bool` constant as (bit width, value).
    fn di_int(&self, v: ConstValueId) -> Option<(u32, i128)> {
        match self.ast.consts.scalar(v)? {
            Scalar::Int { bits, signed, .. } => {
                let w = bits.width();
                let v =
                    if signed { bits.to_i128()? } else { i128::try_from(bits.to_u128()?).ok()? };
                Some((w, v))
            }
            Scalar::Bool { value, .. } => Some((1, i128::from(value))),
            _ => None,
        }
    }

    /// `iN v` as a constant operand prints.
    fn di_int_raw((w, v): (u32, i128)) -> String {
        if w == 1 {
            return format!("i1 {}", v != 0);
        }
        let v = if w < 128 && v >= 0 && (v >> (w - 1)) & 1 == 1 { v - (1i128 << w) } else { v };
        format!("i{w} {v}")
    }

    // ---- classes ------------------------------------------------------

    /// clang `getClassName`: the identifier, with the template arguments
    /// of a specialization (split closers, canonical types).
    fn di_class_name(&self, rd: DeclId) -> String {
        let ast = self.ast;
        let mut s = self.di_ident(rd);
        if let Some(l) = ast.spec_print_args(rd) {
            write_arg_list_debug(&ast.types, &AstNames::new(ast), l, &mut s);
        }
        s
    }

    /// The record type's descriptor: a distinct placeholder completed as
    /// a definition (now, or when homed) or as a declaration at the end
    /// of the unit (clang's `ReplaceMap`).
    fn di_record(&mut self, d: DeclId, q: QualType) -> MetadataId {
        let first = self.ast.first(d);
        if let Some(&p) = self.di.as_deref().and_then(|di| di.classes.get(&first)) {
            return p;
        }
        let f = Fields::default().raw("tag", "DW_TAG_structure_type").raw("flags", "DIFlagFwdDecl");
        let p = self.m.md_specialized("DICompositeType", true, f.0);
        if let Some(di) = self.di.as_deref_mut() {
            di.classes.insert(first, p);
            di.class_order.push(first);
            di.types.insert(q, Some(p));
        }
        if !self.di_omit(first) {
            self.di_complete_class(first);
        }
        p
    }

    /// clang `shouldOmitDefinition` (a definition is always taken as
    /// required: kiln does not track `isCompleteDefinitionRequired`).
    /// `[[clang::standalone_debug]]` on any redeclaration keeps the
    /// definition, as `-fstandalone-debug` does for every class.
    fn di_omit(&self, rd: DeclId) -> bool {
        let k = self.opts.debug.kind;
        if k >= DebugInfoKind::Full || self.has_fn_attr(rd, AttrKind::STANDALONE_DEBUG) {
            return false;
        }
        if self.class_def(rd).is_none() || self.is_dynamic(rd) {
            return true;
        }
        k == DebugInfoKind::Constructor && self.di_can_home(rd)
    }

    /// clang `canUseCtorHoming`.
    fn di_can_home(&self, rd: DeclId) -> bool {
        let bits = self.class_bits(rd);
        if bits.contains(B::LAMBDA)
            || bits.contains(B::AGGREGATE)
            || bits.contains(B::TRIVIAL_DEFAULT_CTOR)
            || bits.contains(B::HAS_CONSTEXPR_CTOR)
        {
            return false;
        }
        let ast = self.ast;
        let Some(def) = self.class_def(rd) else { return false };
        ast.decls.members(def).any(|m| {
            ast.decls.get(m).kind() == DeclKind::CXXConstructor
                && self.special_of(m).is_none()
                && !ast.decls.function(m).is_some_and(|f| f.spec.is_deleted())
        })
    }

    /// clang `completeClassData` (`retain`: `completeUnusedClass`, which
    /// also keeps the type in the unit's retained types).
    pub(crate) fn di_home_class(&mut self, rd: DeclId, retain: bool) {
        let ast = self.ast;
        let rd = ast.first(rd);
        if !self.opts.debug.reduced() || self.class_def(rd).is_none() {
            return;
        }
        if retain && self.is_dynamic(rd) {
            return;
        }
        if self.is_dynamic(rd)
            && self.vtable_linkage(rd) == crate::vtable::VtLinkage::AvailableExternally
        {
            return;
        }
        let Some(q) = ast.decls.get(rd).ty() else { return };
        self.di_record(rd, q);
        self.di_complete_class(rd);
        if retain && let Some(d) = self.di.as_deref_mut() {
            d.retained_classes.push(rd);
        }
    }

    /// clang `CreateTypeDefinition(const RecordType *)` into the
    /// placeholder of `rd` (once).
    fn di_complete_class(&mut self, rd: DeclId) {
        let ast = self.ast;
        let Some(def) = self.class_def(rd) else { return };
        let Some(p) = self
            .di
            .as_deref_mut()
            .and_then(|d| d.completed.insert(rd).then(|| d.classes.get(&rd).copied()))
            .flatten()
        else {
            return;
        };
        let Some(r) = ast.decls.record(def) else { return };
        let Some(rq) = ast.decls.get(def).ty() else { return };
        let loc = ast.decls.get(def).loc();
        let (file, line) = (self.di_file(loc), self.di_line(loc));
        let scope = self.di_scope_nocu(def);
        let name = self.di_class_name(def);
        let size = self.size_align(rq).0 * 8;
        let ident = self.di_type_ident(Some(rd), rq);
        let bits = r.defn;
        let tag = match r.tag {
            TagKind::Union => "DW_TAG_union_type",
            TagKind::Class => "DW_TAG_class_type",
            TagKind::Struct if bits.contains(B::LAMBDA) => "DW_TAG_class_type",
            TagKind::Struct => "DW_TAG_structure_type",
        };
        let mut flags = vec![if kiln_abi::call::can_pass_in_registers(ast, def) {
            "DIFlagTypePassByValue"
        } else {
            "DIFlagTypePassByReference"
        }];
        if !(bits.contains(B::TRIVIAL_DEFAULT_CTOR) && bits.contains(B::TRIVIALLY_COPYABLE)) {
            flags.push("DIFlagNonTrivial");
        }
        if bits.contains(B::ANONYMOUS) {
            flags.push("DIFlagExportSymbols");
        }
        flags.extend(self.di_member_access(def));
        // clang `CollectContainingType`: the root of the primary bases.
        let layout = self.tys.layouter().record(def).clone();
        let holder = match layout.primary_base {
            Some((mut pb, _)) => {
                while let Some(d) = self.class_def(pb) {
                    match self.tys.layouter().record(d).primary_base {
                        Some((b, false)) => pb = b,
                        _ => break,
                    }
                }
                ast.decls.get(pb).ty().and_then(|q| self.di_type(q))
            }
            None => self.is_dynamic(rd).then_some(p),
        };
        let mut els = Vec::new();
        // Bases (clang `CollectCXXBases`).
        let mut seen = Vec::new();
        let bases: Vec<_> = ast.decls.bases(def, &ast.lists).copied().collect();
        for b in bases {
            let Some(brd) = ast.types.as_record(ast.types.canonical(b.ty)) else { continue };
            let brd = ast.first(brd);
            if seen.contains(&brd) {
                continue;
            }
            seen.push(brd);
            let bt = self.di_type(b.ty);
            let mut bf = Vec::new();
            let off = if b.is_virtual {
                bf.push("DIFlagVirtual");
                let oo = self.vt_layout(def).ok().and_then(|l| {
                    l.vbase_offset_offsets
                        .iter()
                        .find(|&&(x, _)| ast.first(x) == brd)
                        .map(|&(_, o)| o)
                });
                oo.map_or(0, |o| -o)
            } else {
                let o =
                    layout.bases.iter().find(|&&(x, _)| ast.first(x) == brd).map_or(0, |&(_, o)| o);
                i64::try_from(o * 8).unwrap_or(0)
            };
            let acc = if b.access == Access::None { Access::None } else { b.access };
            bf.extend(self.di_access(acc, Some(def)));
            let f = Fields::default()
                .n("scope", Some(p))
                .i("offset", off)
                .flags("flags", &bf)
                .raw("extraData", "i32 0");
            els.push(Some(self.di_derived("DW_TAG_inheritance", bt, f)));
        }
        // The vtable pointer (clang `CollectVTableInfo`).
        if layout.primary_base.is_none() && self.is_dynamic(rd) {
            let vt = self.di_vtbl_ptr();
            let pb = self.ptr_bits();
            let f = Fields::default()
                .s("name", &format!("_vptr${}", self.di_ident(def)))
                .n("scope", Some(file))
                .n("file", Some(file))
                .u("size", pb)
                .raw("flags", "DIFlagArtificial");
            els.push(Some(self.di_derived("DW_TAG_member", Some(vt), f)));
        }
        // Fields and static members (clang `CollectRecordFields`).
        let lambda = r.lambda.map(|l| ast.exprs.lambdas.get(l));
        let mut fno = 0usize;
        let members: Vec<DeclId> = ast.decls.members(def).collect();
        for m in members {
            let n = ast.decls.get(m);
            match n.kind() {
                DeclKind::Var if lambda.is_none() => {
                    els.push(self.di_static_member(m, p));
                }
                DeclKind::Field => {
                    let off = layout.fields.get(fno).copied().unwrap_or(0);
                    fno += 1;
                    let name = match lambda {
                        Some(l) => match l.capture_of_field(m) {
                            Some(c) if matches!(c.kind, CaptureKind::This) => "this".to_owned(),
                            Some(c) => c.var.map(|v| self.di_ident(v)).unwrap_or_default(),
                            None => continue,
                        },
                        None => self.di_ident(m),
                    };
                    let Some(fq) = n.ty() else { continue };
                    let is_rec = ast.types.as_record(ast.types.canonical(fq)).is_some();
                    if name.is_empty() && !is_rec {
                        continue;
                    }
                    let floc = lambda
                        .and_then(|l| l.capture_of_field(m))
                        .map_or(n.loc(), |c| if c.var.is_some() { c.loc } else { n.loc() });
                    els.push(Some(self.di_field(m, &name, fq, floc, off, def, p)));
                }
                _ => {}
            }
        }
        // Member functions (clang `CollectCXXMemberFunctions`).
        let members: Vec<DeclId> = ast.decls.members(def).collect();
        for m in members {
            let n = ast.decls.get(m);
            let k = n.kind();
            if !matches!(
                k,
                DeclKind::CXXMethod
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
                    | DeclKind::CXXConversion
            ) || n.has(DeclFlags::IMPLICIT)
                || self.has_fn_attr(m, AttrKind::NODEBUG)
                || self.di_auto_return(m)
            {
                continue;
            }
            els.push(self.di_method_decl(m));
        }
        let elements = self.m.md_tuple(&els);
        let tparams = self.di_template_params(def);
        let mut f = Fields::default().raw("tag", tag);
        if !name.is_empty() {
            f = f.s("name", &name);
        }
        f = f.n("scope", scope).n("file", Some(file)).u("line", u64::from(line)).u("size", size);
        f = f.flags("flags", &flags).n("elements", Some(elements)).n("vtableHolder", holder);
        f = f.n("templateParams", tparams);
        if !ident.is_empty() {
            f = f.s("identifier", &ident);
        }
        let node =
            MdNode::Specialized { distinct: true, name: "DICompositeType", fields: f.0.into() };
        let _ = self.m.md_replace(p, node);
    }

    /// The declaration of a never-completed class (clang
    /// `getOrCreateRecordFwdDecl`), at the end of the unit.
    pub(crate) fn di_class_decl(&mut self, rd: DeclId) -> MdNode {
        let ast = self.ast;
        let decl = self.class_def(rd).unwrap_or(rd);
        let n = ast.decls.get(decl);
        let (file, line) = (self.di_file(n.loc()), self.di_line(n.loc()));
        let scope = self.di_scope_nocu(decl);
        let name = self.di_class_name(decl);
        let tag = match ast.decls.record(decl).map(|r| r.tag) {
            Some(TagKind::Union) => "DW_TAG_union_type",
            Some(TagKind::Class) => "DW_TAG_class_type",
            _ if self.class_bits(rd).contains(B::LAMBDA) => "DW_TAG_class_type",
            _ => "DW_TAG_structure_type",
        };
        let mut flags = vec!["DIFlagFwdDecl"];
        let bits = self.class_bits(rd);
        let def = self.class_def(rd);
        if def.is_none()
            || !(bits.contains(B::TRIVIAL_DEFAULT_CTOR) && bits.contains(B::TRIVIALLY_COPYABLE))
        {
            flags.push("DIFlagNonTrivial");
        }
        let mut f = Fields::default().raw("tag", tag);
        if !name.is_empty() {
            f = f.s("name", &name);
        }
        f = f.n("scope", scope).n("file", Some(file)).u("line", u64::from(line));
        if def.is_some()
            && let Some(q) = n.ty()
        {
            f = f.u("size", self.size_align(q).0 * 8);
        }
        f = f.flags("flags", &flags);
        let ident = n.ty().map(|q| self.di_type_ident(Some(rd), q)).unwrap_or_default();
        if !ident.is_empty() {
            f = f.s("identifier", &ident);
        }
        MdNode::Specialized { distinct: false, name: "DICompositeType", fields: f.0.into() }
    }

    /// `__vtbl_ptr_type` (clang `getOrCreateVTablePtrType`).
    fn di_vtbl_ptr(&mut self) -> MetadataId {
        if let Some(v) = self.di.as_deref().and_then(|d| d.vtbl_ptr) {
            return v;
        }
        let int = self.di_type(QualType::INT);
        let types = self.m.md_tuple(&[int]);
        let sub = self.m.md_specialized(
            "DISubroutineType",
            false,
            Fields::default().n("types", Some(types)).0,
        );
        let pb = self.ptr_bits();
        let inner = self.di_derived(
            PTR,
            Some(sub),
            Fields::default().s("name", "__vtbl_ptr_type").u("size", pb),
        );
        let v = self.di_derived(PTR, Some(inner), Fields::default().u("size", pb));
        if let Some(d) = self.di.as_deref_mut() {
            d.vtbl_ptr = Some(v);
        }
        v
    }

    /// clang `createFieldType` / `createBitFieldType`.
    #[allow(clippy::too_many_arguments)]
    fn di_field(
        &mut self,
        m: DeclId,
        name: &str,
        fq: QualType,
        loc: kiln_source::Loc,
        off: u64,
        def: DeclId,
        p: MetadataId,
    ) -> MetadataId {
        let ast = self.ast;
        let t = self.di_type(fq);
        let (file, line) = (self.di_file(loc), self.di_line(loc));
        let mut fl: Vec<&str> =
            self.di_access(ast.decls.get(m).misc().access(), Some(def)).into_iter().collect();
        let mut f = Fields::default().s("name", name).n("scope", Some(p)).n("file", Some(file));
        f = f.u("line", u64::from(line));
        if let Some(bf) = self.tys.record(def).bitfields.get(&m).copied() {
            let storage = bf.storage_offset * 8;
            fl.push("DIFlagBitField");
            f = f.u("size", u64::from(bf.size)).u("offset", storage + u64::from(bf.offset));
            f = f.flags("flags", &fl).raw("extraData", format!("i64 {storage}"));
        } else {
            let canon = ast.types.canonical(fq);
            let size = match ast.types.view(canon.ty()) {
                TypeView::IncompleteArray(_) => None,
                TypeView::LValueRef(_) | TypeView::RValueRef(_) => Some(self.ptr_bits()),
                _ => Some(self.size_align(canon).0 * 8),
            };
            if let Some(s) = size {
                f = f.u("size", s);
            }
            f = f.u("offset", off).flags("flags", &fl);
        }
        self.di_derived("DW_TAG_member", t, f)
    }

    /// clang `CreateRecordStaticField` (cached by first declaration).
    pub(crate) fn di_static_member(&mut self, v: DeclId, rec: MetadataId) -> Option<MetadataId> {
        let ast = self.ast;
        let first = ast.first(v);
        if let Some(&s) = self.di.as_deref().and_then(|d| d.static_members.get(&first)) {
            return Some(s);
        }
        let n = ast.decls.get(first);
        let t = self.di_type(n.ty()?);
        let (file, line) = (self.di_file(n.loc()), self.di_line(n.loc()));
        let tag = if self.opts.debug.dwarf_version.unwrap_or(5) >= 5 {
            "DW_TAG_variable"
        } else {
            "DW_TAG_member"
        };
        let mut fl: Vec<&str> = self.di_member_access(first).into_iter().collect();
        fl.push("DIFlagStaticMember");
        let mut f = Fields::default()
            .s("name", &self.di_ident(first))
            .n("scope", Some(rec))
            .n("file", Some(file));
        f = f.u("line", u64::from(line)).flags("flags", &fl);
        let c =
            ast.decls.var(first).filter(|d| d.init.is_some()).and_then(|d| match d.eval.state() {
                VarEvalState::Value(cv) => self.di_int(cv),
                _ => None,
            });
        if let Some(c) = c {
            f = f.raw("extraData", Self::di_int_raw(c));
        }
        let s = self.di_derived(tag, t, f);
        if let Some(d) = self.di.as_deref_mut() {
            d.static_members.insert(first, s);
        }
        Some(s)
    }

    /// The method's declared return type contains `auto`.
    fn di_auto_return(&self, m: DeclId) -> bool {
        let ty = &self.ast.types;
        if self.ast.decls.function(m).is_some_and(|x| x.spec.return_deduced()) {
            return true;
        }
        let Some(q) = self.ast.decls.get(m).ty() else { return false };
        let mut t = q.ty();
        loop {
            match ty.view(t) {
                TypeView::Paren(i) | TypeView::Attributed { modified: i, .. } => t = i.ty(),
                TypeView::Function(fp) => {
                    return matches!(
                        ty.view(fp.ret().ty()),
                        TypeView::Auto { .. } | TypeView::DeducedTemplateSpecialization { .. }
                    );
                }
                _ => return false,
            }
        }
    }

    /// clang `getFunctionName`: the name with the template arguments of
    /// a function template specialization.
    pub(crate) fn di_fn_name(&self, f: DeclId) -> String {
        let ast = self.ast;
        let names = AstNames::new(ast);
        let mut s = String::new();
        names.write_name(f, &mut s);
        if let Some((_, si)) = ast.spec_info(f)
            && ast.decls.get(f).kind() != DeclKind::CXXRecord
        {
            write_arg_list_debug(&ast.types, &names, si.args, &mut s);
        }
        s
    }

    /// The type of member function `m` (clang `getOrCreateMethodType`).
    pub(crate) fn di_method_type(&mut self, m: DeclId) -> Option<MetadataId> {
        let ast = self.ast;
        let q = self.di_unwrap(ast.decls.get(m).ty()?);
        let TypeView::Function(fp) = ast.types.view(q.ty()) else { return self.di_type(q) };
        if self.is_static_method(m) || self.is_explicit_object_method(m) {
            return Some(self.di_fn_type(&fp, None));
        }
        let rd = self.di_parent_record(ast.first(m));
        self.di_instance_fn_type(&fp, rd, false)
    }

    /// The declaration of member function `m` (clang
    /// `CreateCXXMemberFunction`, cached by first declaration).
    pub(crate) fn di_method_decl(&mut self, m: DeclId) -> Option<MetadataId> {
        let ast = self.ast;
        let first = ast.first(m);
        if let Some(&s) = self.di.as_deref().and_then(|d| d.decl_sps.get(&first)) {
            return Some(s);
        }
        let rd = self.di_parent_record(first)?;
        let rec = self.di_type(ast.decls.get(rd).ty()?)?;
        let n = ast.decls.get(first);
        let kind = n.kind();
        let structor = matches!(kind, DeclKind::CXXConstructor | DeclKind::CXXDestructor);
        let name = self.di_fn_name(first);
        let ty = self.di_method_type(first);
        let local = self.di_in_function(rd);
        let linkage =
            (!structor && !local).then(|| mangle::mangle_decl(ast, &mut self.anon, first));
        let implicit = n.has(DeclFlags::IMPLICIT);
        let (file, line) =
            if implicit { (None, 0) } else { (Some(self.di_file(n.loc())), self.di_line(n.loc())) };
        let spec = |p: fn(FnSpec) -> bool| {
            ast.redecls_of(first).any(|r| ast.decls.function(r).is_some_and(|f| p(f.spec)))
        };
        let mut sp: Vec<&str> = Vec::new();
        let mut fl: Vec<&str> = Vec::new();
        let mut vindex = None;
        if spec(FnSpec::is_virtual) {
            sp.push(if spec(FnSpec::is_pure) { "DISPFlagPureVirtual" } else { "DISPFlagVirtual" });
            vindex = Some(if kind == DeclKind::CXXDestructor {
                0
            } else {
                self.method_vtable_index(first, None).unwrap_or(0)
            });
        }
        if spec(FnSpec::is_deleted) {
            sp.push("DISPFlagDeleted");
        }
        let fp = match ast.types.view(self.di_unwrap(n.ty()?).ty()) {
            TypeView::Function(fp) => Some(fp),
            _ => None,
        };
        if self.has_fn_attr(first, AttrKind::NORETURN) || fp.is_some_and(|f| f.info().noreturn()) {
            fl.push("DIFlagNoReturn");
        }
        if ast.decls.function(first).is_some_and(|f| f.spec.storage() == FnStorage::Static) {
            fl.push("DIFlagStaticMember");
        }
        if implicit {
            fl.push("DIFlagArtificial");
        }
        fl.extend(self.di_member_access(first));
        if matches!(kind, DeclKind::CXXConstructor | DeclKind::CXXConversion)
            && spec(FnSpec::is_explicit)
        {
            fl.push("DIFlagExplicit");
        }
        fl.push("DIFlagPrototyped");
        match fp.map(|f| f.info().ref_qual()) {
            Some(RefQual::LValue) => fl.push("DIFlagLValueReference"),
            Some(RefQual::RValue) => fl.push("DIFlagRValueReference"),
            _ => {}
        }
        if self.di_internal(first) {
            sp.push("DISPFlagLocalToUnit");
        }
        if self.opts.opt_level > 0 {
            sp.push("DISPFlagOptimized");
        }
        if self.opts.debug.kind == DebugInfoKind::Constructor && kind == DeclKind::CXXConstructor {
            self.di_home_class(rd, true);
            if let Some(&s) = self.di.as_deref().and_then(|d| d.decl_sps.get(&first)) {
                return Some(s);
            }
        }
        let tparams = self.di_template_params(first);
        let mut f = Fields::default().s("name", &name);
        if let Some(l) = &linkage {
            f = f.s("linkageName", l);
        }
        f = f.n("scope", Some(rec)).n("file", file).u("line", u64::from(line)).n("type", ty);
        f = f.u("scopeLine", u64::from(line));
        if let Some(v) = vindex {
            f = f.n("containingType", Some(rec)).u("virtualIndex", u64::from(v));
        }
        f = f.flags("flags", &fl).sp_flags(&sp).n("templateParams", tparams);
        let s = self.m.md_specialized("DISubprogram", false, f.0);
        if let Some(d) = self.di.as_deref_mut() {
            d.decl_sps.insert(first, s);
        }
        Some(s)
    }

    /// `d` is not externally visible (clang `isExternallyVisible`).
    pub(crate) fn di_internal(&self, d: DeclId) -> bool {
        let ast = self.ast;
        mangle::has_internal_linkage(ast, d)
            || self
                .di_parent_record(d)
                .is_some_and(|r| mangle::class_is_internal(ast, ast.first(r)))
    }

    /// clang `isFunctionLocalClass`.
    fn di_in_function(&self, rd: DeclId) -> bool {
        let ast = self.ast;
        let mut c = ast.decls.get(rd).ctx();
        while let Some(x) = c {
            match ast.decls.get(x).kind() {
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization => {
                    c = ast.decls.get(x).ctx();
                }
                DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion => return true,
                _ => return false,
            }
        }
        false
    }

    /// clang `CollectCXXTemplateParams` / `CollectFunctionTemplateParams`
    /// for the specialization `d` (`None` otherwise).
    pub(crate) fn di_template_params(&mut self, d: DeclId) -> Option<MetadataId> {
        let ast = self.ast;
        let (_, si) = ast.spec_info(d)?;
        let info = ast.template_info(si.template)?;
        let params: Vec<DeclId> =
            ast.lists.iter::<DeclId>(ast.templates.param_list(info.params).params).collect();
        let args: Vec<ArgView> = ast.types.args(si.args).iter().map(TemplateArg::view).collect();
        let mut ops = Vec::new();
        for (i, a) in args.into_iter().enumerate() {
            let name = params.get(i).map(|&p| self.di_ident(p)).unwrap_or_default();
            if let Some(p) = self.di_template_arg(&name, a) {
                ops.push(Some(p));
            }
        }
        Some(self.m.md_tuple(&ops))
    }

    fn di_template_arg(&mut self, name: &str, a: ArgView) -> Option<MetadataId> {
        let mut f = Fields::default();
        if !name.is_empty() {
            f = f.s("name", name);
        }
        match a {
            ArgView::Type(t) => {
                let t = self.di_type(t);
                Some(self.m.md_specialized("DITemplateTypeParameter", false, f.nn("type", t).0))
            }
            // clang emits a null pointer argument as `i8 0`, a null
            // member function pointer included; a null data member
            // pointer is the ABI's `-1`, below.
            ArgView::Value { ty, value }
                if matches!(self.ast.consts.scalar(value), Some(Scalar::Null { .. }))
                    && (!self.is_mp(ty) || self.is_mfp(ty)) =>
            {
                let t = self.di_type(ty);
                Some(self.m.md_specialized(
                    "DITemplateValueParameter",
                    false,
                    f.n("type", t).raw("value", "i8 0").0,
                ))
            }
            ArgView::Value { ty, value } => {
                let t = self.di_type(ty);
                if let Some(c) = self.di_int(value) {
                    let f = f.n("type", t).raw("value", Self::di_int_raw(c));
                    return Some(self.m.md_specialized("DITemplateValueParameter", false, f.0));
                }
                // clang: the address of a variable or function, a member
                // pointer, or `emitAbstract` of a floating-point or
                // class-type value.
                let record = matches!(
                    self.ast.types.view(self.ast.types.canonical(ty).ty()),
                    TypeView::Record(_)
                );
                let v = if record {
                    self.const_value(value, ty, None, true)
                } else {
                    self.const_scalar_value(value, ty)
                };
                // A value with no IR constant (a `std::meta::info`, or one
                // kiln cannot lower) keeps its name and type with `value:
                // null`, as clang's parameter does when its `ConstantEmitter`
                // gives none (`CollectTemplateParams`; WP-910).
                let v = v.ok().and_then(|v| self.m.md_const(v).ok());
                let f = f.n("type", t).nn("value", v);
                Some(self.m.md_specialized("DITemplateValueParameter", false, f.0))
            }
            ArgView::Pack(l) => {
                let args: Vec<ArgView> =
                    self.ast.types.args(l).iter().map(TemplateArg::view).collect();
                let ops: Vec<_> = args
                    .into_iter()
                    .filter_map(|a| self.di_template_arg("", a))
                    .map(Some)
                    .collect();
                let v = self.m.md_tuple(&ops);
                let f = Fields::default().raw("tag", "DW_TAG_GNU_template_parameter_pack").0;
                let f = Fields(f).s("name", name).n("value", Some(v));
                Some(self.m.md_specialized("DITemplateValueParameter", false, f.0))
            }
            // clang: the template's `printQualifiedName`.
            ArgView::Template(n) if let Some(d) = self.ast.types.template_decl(n) => {
                let q = self.qualified_name(self.ast.first(d));
                let v = self.m.md_string(q.as_bytes());
                let f = Fields::default().raw("tag", "DW_TAG_GNU_template_template_param");
                let f = if name.is_empty() { f } else { f.s("name", name) };
                Some(self.m.md_specialized(
                    "DITemplateValueParameter",
                    false,
                    f.n("value", Some(v)).0,
                ))
            }
            // A template name that resolves to no template declaration
            // (nothing a concrete specialization should hold): clang's
            // `createTemplateTemplateParameter` needs the template's name,
            // so the parameter is left out (WP-910).
            _ => None,
        }
    }
}
