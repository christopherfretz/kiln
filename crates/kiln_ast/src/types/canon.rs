//! Type constructors: hash-consing plus the canonical-type rules of
//! spec/types.md §5 and §7. Every constructor returns the existing node
//! for an equal key, so `TypeId` equality is structural equality and
//! canonical equality is type identity.

use super::context::{
    AUX_PROFILE, TYPEOF_TYPE, TYPEOF_UNQUAL, TypeContext, TypeOfOperand, TypeView,
};
use super::func::{ExcSpec, FuncInfo, NoexceptValue};
use super::kinds::{
    AutoKeyword, Quals, TagKeyword, TypeAttrKind, TypeFlags, TypeKind, UnaryTransformKind,
    VectorKind,
};
use super::names::TemplateName;
use super::node::{ConstArray, ExcKey, Key};
use crate::ids::{
    ArgListId, DeclId, ExprId, NnsId, ProfileId, QualType, RawId, TemplateNameId, TypeId,
};
use kiln_source::symbol::Symbol;

/// clang's `toSemanticDependence`: an instantiation-dependent expression
/// inside a type makes the type dependent.
fn sem(f: TypeFlags) -> TypeFlags {
    let f = f.without(TypeFlags::SUGAR);
    if f.intersects(TypeFlags::DEPENDENT_INSTANTIATION) {
        f | TypeFlags::DEPENDENT_INSTANTIATION
    } else {
        f
    }
}

#[inline]
fn raw(q: QualType) -> u32 {
    q.to_raw()
}

#[inline]
fn raw_opt(q: Option<QualType>) -> u32 {
    q.map_or(0, QualType::to_raw)
}

impl TypeContext {
    fn uq(t: impl Into<QualType>) -> QualType {
        t.into()
    }

    /// `canonical(q)` if it differs from `q`, for canonical closures.
    fn canon_if(&self, q: QualType) -> Option<QualType> {
        let c = self.canonical(q);
        (c != q).then_some(c)
    }

    /// A node with one `QualType` operand stored inline whose canonical
    /// form is the same kind over the canonical operand.
    fn derived1(&mut self, kind: TypeKind, aux: u16, op: QualType, extra: TypeFlags) -> TypeId {
        let flags = self.qflags(op) | extra;
        self.intern_with(Key::inline(kind, aux, raw(op)), flags, |ctx| {
            let c = ctx.canon_if(op)?;
            Some(Self::uq(ctx.derived1(kind, aux, c, extra)))
        })
    }

    // ----- qualifiers ---------------------------------------------------

    /// Adds `cv` to `q` (§2.1): ignored on function and reference types,
    /// pushed into the element type of arrays (through `QualifiedArray`
    /// when `q` is array sugar).
    pub fn qualify(&mut self, q: QualType, cv: Quals) -> QualType {
        if cv.is_empty() {
            return q;
        }
        let k = self.canonical_kind(q);
        if k == TypeKind::Function || k.is_reference() {
            q
        } else if k.is_array() {
            Self::uq(self.qualify_array(q, cv | q.quals()))
        } else {
            QualType::new(q.ty(), q.quals_bits() | cv.bits())
        }
    }

    fn qualify_array(&mut self, q: QualType, cv: Quals) -> TypeId {
        match self.view(q.ty()) {
            TypeView::ConstantArray(a) => {
                let e = self.qualify(a.elem, cv);
                self.constant_array(e, a.size, a.size_expr).ty()
            }
            TypeView::IncompleteArray(e) => {
                let e = self.qualify(e, cv);
                self.incomplete_array(e).ty()
            }
            TypeView::VariableArray { elem, size } => {
                let e = self.qualify(elem, cv);
                self.variable_array(e, size).ty()
            }
            TypeView::DependentSizedArray { elem, size } => {
                let e = self.qualify(elem, cv);
                self.dsa_rebuild(q.ty(), e, size)
            }
            TypeView::QualifiedArray { quals, base } => self.qualified_array(base, quals | cv),
            _ => self.qualified_array(q.ty(), cv),
        }
    }

    fn dsa_rebuild(
        &mut self,
        t: TypeId,
        elem: QualType,
        size: super::context::ExprOperand,
    ) -> TypeId {
        use super::context::ExprOperand;
        match size {
            ExprOperand::Profile(p) => self.dsa_canonical(elem, p),
            ExprOperand::Expr(e) => match self.canonical_view(Self::uq(t)) {
                TypeView::DependentSizedArray { size: ExprOperand::Profile(p), .. } => {
                    self.dependent_sized_array(elem, e, p).ty()
                }
                _ => self.exhaust(),
            },
        }
    }

    fn qualified_array(&mut self, base: TypeId, cv: Quals) -> TypeId {
        let flags = self.qflags(Self::uq(base));
        self.intern_with(
            Key::inline(TypeKind::QualifiedArray, u16::from(cv.bits()), base.index()),
            flags,
            |ctx| {
                let c = ctx.canonical(Self::uq(base));
                Some(Self::uq(ctx.qualify_array(c, cv)))
            },
        )
    }

    /// `q` without its top-level qualifiers, including those an array
    /// type carries on its element type (§2.1).
    pub fn unqualified(&mut self, q: QualType) -> QualType {
        let q = q.without_quals();
        if !self.canonical_kind(q).is_array() || self.effective_quals(q).is_empty() {
            return q;
        }
        let t = match self.view(q.ty()) {
            TypeView::ConstantArray(a) => {
                let e = self.unqualified(a.elem);
                self.constant_array(e, a.size, a.size_expr).ty()
            }
            TypeView::IncompleteArray(e) => {
                let e = self.unqualified(e);
                self.incomplete_array(e).ty()
            }
            TypeView::VariableArray { elem, size } => {
                let e = self.unqualified(elem);
                self.variable_array(e, size).ty()
            }
            TypeView::DependentSizedArray { elem, size } => {
                let e = self.unqualified(elem);
                self.dsa_rebuild(q.ty(), e, size)
            }
            TypeView::QualifiedArray { base, .. } => return self.unqualified(Self::uq(base)),
            _ => {
                let c = self.canonical(q);
                return self.unqualified(c);
            }
        };
        Self::uq(t)
    }

    // ----- pointers and references ------------------------------------

    /// `T*`.
    pub fn pointer_to(&mut self, pointee: QualType) -> QualType {
        let t: TypeId = { self.derived1(TypeKind::Pointer, 0, pointee, TypeFlags::NONE) };
        t.into()
    }

    /// `T&`, with reference collapsing ([dcl.ref]/6).
    pub fn lvalue_ref_to(&mut self, t: QualType) -> QualType {
        let t: TypeId = {
            match self.ref_node(t) {
                Some((TypeKind::LValueRef, _)) => t.without_quals().ty(),
                Some((_, p)) => self.derived1(TypeKind::LValueRef, 0, p, TypeFlags::NONE),
                None => self.derived1(TypeKind::LValueRef, 0, t, TypeFlags::NONE),
            }
        };
        t.into()
    }

    /// `T&&`, with reference collapsing ([dcl.ref]/6).
    pub fn rvalue_ref_to(&mut self, t: QualType) -> QualType {
        let t: TypeId = {
            match self.ref_node(t) {
                Some(_) => t.ty(),
                None => self.derived1(TypeKind::RValueRef, 0, t, TypeFlags::NONE),
            }
        };
        t.into()
    }

    /// If `t` is a reference type: the kind of the reference and its
    /// referenced type (through sugar).
    fn ref_node(&self, t: QualType) -> Option<(TypeKind, QualType)> {
        if !self.canonical_kind(t).is_reference() {
            return None;
        }
        let mut q = t;
        loop {
            match self.view(q.ty()) {
                TypeView::LValueRef(p) => return Some((TypeKind::LValueRef, p)),
                TypeView::RValueRef(p) => return Some((TypeKind::RValueRef, p)),
                _ => {
                    let n = self.desugar_once(q);
                    if n == q {
                        q = self.canonical(q);
                    } else {
                        q = n;
                    }
                }
            }
        }
    }

    /// `T C::*`.
    pub fn member_pointer(&mut self, pointee: QualType, class: TypeId) -> QualType {
        let t: TypeId = {
            let flags = self.qflags(pointee) | self.flags(class).without(TypeFlags::SUGAR);
            let key = Key::pair(TypeKind::MemberPointer, 0, raw(pointee), class.index());
            self.intern_with(key, flags, |ctx| {
                let cp = ctx.canonical(pointee);
                let cc = ctx.canonical(Self::uq(class)).ty();
                (cp != pointee || cc != class).then(|| Self::uq(ctx.member_pointer(cp, cc)))
            })
        };
        t.into()
    }

    // ----- arrays --------------------------------------------------------

    /// `T[N]`; `size_expr` is the bound as written (sugar).
    pub fn constant_array(
        &mut self,
        elem: QualType,
        size: u64,
        size_expr: Option<ExprId>,
    ) -> QualType {
        let t: TypeId = {
            let flags = self.qflags(elem);
            let a = ConstArray { elem, size, size_expr };
            self.intern_with(Key::Array(a), flags, |ctx| {
                let ce = ctx.canonical(elem);
                (ce != elem || size_expr.is_some())
                    .then(|| Self::uq(ctx.constant_array(ce, size, None)))
            })
        };
        t.into()
    }

    /// `T[]`.
    pub fn incomplete_array(&mut self, elem: QualType) -> QualType {
        let t: TypeId = { self.derived1(TypeKind::IncompleteArray, 0, elem, TypeFlags::NONE) };
        t.into()
    }

    /// `T[n]` with a runtime bound (a VLA, GNU extension): a `D/O` node,
    /// never interned.
    pub fn variable_array(&mut self, elem: QualType, size: ExprId) -> QualType {
        let t: TypeId = {
            let flags = self.qflags(elem) | TypeFlags::VARIABLY_MODIFIED;
            let key = Key::pair(TypeKind::VariableArray, 0, raw(elem), size.raw());
            let canon = self.canon_if(elem).map(|ce| Self::uq(self.variable_array(ce, size)));
            self.create(&key, flags, canon, None)
        };
        t.into()
    }

    /// `T[expr]` with a value-dependent bound whose profile is `size`.
    pub fn dependent_sized_array(
        &mut self,
        elem: QualType,
        expr: ExprId,
        size: ProfileId,
    ) -> QualType {
        let t: TypeId = {
            let flags = self.qflags(elem)
                | sem(self.profile_flags(size))
                | TypeFlags::DEPENDENT_INSTANTIATION;
            let key = Key::pair(TypeKind::DependentSizedArray, 0, raw(elem), expr.raw());
            self.intern_with(key, flags, |ctx| {
                let ce = ctx.canonical(elem);
                Some(Self::uq(ctx.dsa_canonical(ce, size)))
            })
        };
        t.into()
    }

    fn dsa_canonical(&mut self, elem: QualType, size: ProfileId) -> TypeId {
        let flags =
            self.qflags(elem) | sem(self.profile_flags(size)) | TypeFlags::DEPENDENT_INSTANTIATION;
        let key = Key::pair(TypeKind::DependentSizedArray, AUX_PROFILE, raw(elem), size.raw());
        self.intern_with(key, flags, |ctx| {
            let ce = ctx.canon_if(elem)?;
            Some(Self::uq(ctx.dsa_canonical(ce, size)))
        })
    }

    // ----- other builtin-like compounds ------------------------------------

    /// `_BitInt(N)` / `unsigned _BitInt(N)`.
    pub fn bit_int(&mut self, signed: bool, width: u32) -> QualType {
        let t: TypeId = {
            self.intern_with(
                Key::inline(TypeKind::BitInt, u16::from(signed), width),
                TypeFlags::NONE,
                |_| None,
            )
        };
        t.into()
    }

    /// `_BitInt(expr)` with a value-dependent width.
    pub fn dependent_bit_int(&mut self, signed: bool, width: ProfileId) -> QualType {
        let t: TypeId = {
            let flags = sem(self.profile_flags(width)) | TypeFlags::DEPENDENT_INSTANTIATION;
            let key = Key::inline(TypeKind::DependentBitInt, u16::from(signed), width.raw());
            self.intern_with(key, flags, |_| None)
        };
        t.into()
    }

    /// `_Complex T`.
    pub fn complex(&mut self, elem: QualType) -> QualType {
        let t: TypeId = { self.derived1(TypeKind::Complex, 0, elem, TypeFlags::NONE) };
        t.into()
    }

    /// `_Atomic(T)`.
    pub fn atomic(&mut self, value: QualType) -> QualType {
        let t: TypeId = { self.derived1(TypeKind::Atomic, 0, value, TypeFlags::NONE) };
        t.into()
    }

    /// A vector of `count` elements.
    pub fn vector(&mut self, elem: QualType, count: u32, kind: VectorKind) -> QualType {
        let t: TypeId = {
            let key = Key::pair(TypeKind::Vector, kind as u16, raw(elem), count);
            self.intern_with(key, self.qflags(elem), |ctx| {
                let ce = ctx.canon_if(elem)?;
                Some(Self::uq(ctx.vector(ce, count, kind)))
            })
        };
        t.into()
    }

    /// A vector whose element count is the value-dependent `expr`.
    pub fn dependent_vector(
        &mut self,
        elem: QualType,
        expr: ExprId,
        size: ProfileId,
        kind: VectorKind,
    ) -> QualType {
        let t: TypeId = {
            let flags = self.qflags(elem)
                | sem(self.profile_flags(size))
                | TypeFlags::DEPENDENT_INSTANTIATION;
            let key = Key::pair(TypeKind::DependentVector, kind as u16, raw(elem), expr.raw());
            self.intern_with(key, flags, |ctx| {
                let ce = ctx.canonical(elem);
                Some(Self::uq(ctx.dependent_vector_canonical(ce, size, kind)))
            })
        };
        t.into()
    }

    fn dependent_vector_canonical(
        &mut self,
        elem: QualType,
        size: ProfileId,
        kind: VectorKind,
    ) -> TypeId {
        let flags =
            self.qflags(elem) | sem(self.profile_flags(size)) | TypeFlags::DEPENDENT_INSTANTIATION;
        let key =
            Key::pair(TypeKind::DependentVector, kind as u16 | AUX_PROFILE, raw(elem), size.raw());
        self.intern_with(key, flags, |ctx| {
            let ce = ctx.canon_if(elem)?;
            Some(Self::uq(ctx.dependent_vector_canonical(ce, size, kind)))
        })
    }

    /// A type with extended qualifiers (address space, GC, ...) packed in
    /// `word`.
    pub fn ext_qual(&mut self, base: QualType, word: u32) -> QualType {
        let t: TypeId = {
            let key = Key::pair(TypeKind::ExtQual, 0, raw(base), word);
            self.intern_with(key, self.qflags(base), |ctx| {
                let cb = ctx.canon_if(base)?;
                Some(Self::uq(ctx.ext_qual(cb, word)))
            })
        };
        t.into()
    }

    // ----- functions -------------------------------------------------------

    /// A function type. `params` are the adjusted parameter types
    /// ([dcl.fct]/5) as written; the canonical type drops their top-level
    /// cv, the trailing-return flag, and normalizes the exception
    /// specification (§5.4).
    pub fn function(
        &mut self,
        ret: QualType,
        params: &[QualType],
        info: FuncInfo,
        exc: ExcSpec,
    ) -> QualType {
        let t: TypeId = {
            let dyn_types: Vec<QualType> = match exc {
                ExcSpec::Dynamic(l) => self.type_list_types(l).to_vec(),
                _ => Vec::new(),
            };
            let Some(exc_key) = Self::exc_key(exc, &dyn_types) else {
                return self.exhaust().into();
            };
            let mut flags = self.qflags(ret);
            for &p in params {
                flags |= self.qflags(p);
            }
            flags |= match exc {
                ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(p), .. }
                | ExcSpec::DependentNoexcept(p) => {
                    sem(self.profile_flags(p)) | TypeFlags::DEPENDENT_INSTANTIATION
                }
                _ => TypeFlags::NONE,
            };
            for &t in &dyn_types {
                flags |= self.qflags(t);
            }
            let key = Key::Func { ret, params, info, exc: exc_key };
            self.intern_with(key, flags, |ctx| {
                let cret = ctx.canonical(ret);
                let cparams: Vec<QualType> =
                    params.iter().map(|&p| ctx.canonical(p).without_quals()).collect();
                let cinfo = info.with_trailing_return(false);
                let cexc = ctx.canonical_exc(exc);
                let same =
                    cret == ret && cparams.as_slice() == params && cinfo == info && cexc == exc;
                (!same).then(|| Self::uq(ctx.function(cret, &cparams, cinfo, cexc)))
            })
        };
        t.into()
    }

    fn exc_key(exc: ExcSpec, dyn_types: &[QualType]) -> Option<ExcKey<'_>> {
        let simple = |tag: u32, op: u32| (op >> 29 == 0).then_some(ExcKey::Simple(tag | op << 3));
        match exc {
            ExcSpec::None => simple(ExcSpec::TAG_NONE, 0),
            ExcSpec::Nothrow => simple(ExcSpec::TAG_NOTHROW, 0),
            ExcSpec::DynamicNone => simple(ExcSpec::TAG_DYNAMIC_NONE, 0),
            ExcSpec::BasicNoexcept => simple(ExcSpec::TAG_BASIC_NOEXCEPT, 0),
            ExcSpec::NoexceptExpr { expr, value } => {
                let (tag, p) = match value {
                    NoexceptValue::False => (0, 0),
                    NoexceptValue::True => (1, 0),
                    NoexceptValue::Dependent(p) => (2, p.raw()),
                };
                Some(ExcKey::Expr([expr.raw(), tag, p]))
            }
            ExcSpec::Dynamic(_) => Some(ExcKey::Dynamic(dyn_types)),
            ExcSpec::DependentNoexcept(p) => simple(ExcSpec::TAG_DEPENDENT, p.raw()),
            ExcSpec::Deferred(d) => simple(ExcSpec::TAG_DEFERRED, d.raw()),
        }
    }

    /// The canonical form of an exception specification (§5.4): before
    /// C++17 it is not part of the type; from C++17 only "non-throwing"
    /// and "potentially-throwing" remain (plus the unresolved forms).
    pub fn canonical_exc(&self, exc: ExcSpec) -> ExcSpec {
        if !self.opts.noexcept_in_type {
            return ExcSpec::None;
        }
        match exc {
            ExcSpec::Nothrow
            | ExcSpec::DynamicNone
            | ExcSpec::BasicNoexcept
            | ExcSpec::NoexceptExpr { value: NoexceptValue::True, .. } => ExcSpec::Nothrow,
            ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(p), .. } => {
                ExcSpec::DependentNoexcept(p)
            }
            ExcSpec::DependentNoexcept(_) | ExcSpec::Deferred(_) => exc,
            ExcSpec::None | ExcSpec::NoexceptExpr { .. } | ExcSpec::Dynamic(_) => ExcSpec::None,
        }
    }

    // ----- declaration types ---------------------------------------------

    /// The type of class `decl` (one node per declaration).
    pub fn record_type(&mut self, decl: DeclId) -> TypeId {
        self.record_type_dependent(decl, false)
    }

    /// The type of class `decl`; `dependent` for a class nested in a
    /// template ([temp.dep.type]/9: a member of the current instantiation).
    pub fn record_type_dependent(&mut self, decl: DeclId, dependent: bool) -> TypeId {
        self.decl_type(TypeKind::Record, decl, dependent)
    }

    /// The type of enumeration `decl`.
    pub fn enum_type(&mut self, decl: DeclId) -> TypeId {
        self.enum_type_dependent(decl, false)
    }

    /// The type of enumeration `decl`; `dependent` as for
    /// [`Self::record_type_dependent`].
    pub fn enum_type_dependent(&mut self, decl: DeclId, dependent: bool) -> TypeId {
        self.decl_type(TypeKind::Enum, decl, dependent)
    }

    fn decl_type(&mut self, kind: TypeKind, decl: DeclId, dependent: bool) -> TypeId {
        let flags = if dependent { TypeFlags::DEPENDENT_INSTANTIATION } else { TypeFlags::NONE };
        self.intern_with(Key::inline(kind, u16::from(dependent), decl.raw()), flags, |_| None)
    }

    /// The type named by typedef or alias declaration `decl`, sugar over
    /// `underlying`.
    pub fn typedef_type(&mut self, decl: DeclId, underlying: QualType) -> TypeId {
        self.sugar2(TypeKind::Typedef, 0, decl.raw(), underlying, self.qflags(underlying))
    }

    /// A type named through using-declaration `shadow`.
    pub fn using_type(&mut self, shadow: DeclId, underlying: QualType) -> TypeId {
        self.sugar2(TypeKind::Using, 0, shadow.raw(), underlying, self.qflags(underlying))
    }

    /// A type named with an elaborated-type-specifier and/or a
    /// nested-name-specifier: `struct ns::S`.
    pub fn elaborated(
        &mut self,
        keyword: TagKeyword,
        nns: Option<NnsId>,
        named: QualType,
    ) -> QualType {
        let t: TypeId = {
            let flags = self.qflags(named) | nns.map_or(TypeFlags::NONE, |n| self.nns_flags(n));
            self.sugar2(TypeKind::Elaborated, keyword as u16, NnsId::raw_opt(nns), named, flags)
        };
        t.into()
    }

    /// A pair sugar node `[a, of]` whose canonical type is `canonical(of)`.
    fn sugar2(
        &mut self,
        kind: TypeKind,
        aux: u16,
        a: u32,
        of: QualType,
        flags: TypeFlags,
    ) -> TypeId {
        let key = Key::pair(kind, aux, a, raw(of));
        self.intern_with(key, flags, |ctx| Some(ctx.canonical(of)))
    }

    // ----- template parameters -------------------------------------------

    /// The type of template type parameter `decl` (sugar for the
    /// canonical parameter at `(depth, index)`; not desugared by
    /// [`Self::desugar_once`], like clang).
    pub fn template_type_parm(
        &mut self,
        decl: DeclId,
        depth: u32,
        index: u32,
        pack: bool,
    ) -> TypeId {
        let flags = Self::parm_flags(pack);
        self.intern_with(Key::inline(TypeKind::TemplateTypeParm, 0, decl.raw()), flags, |ctx| {
            Some(Self::uq(ctx.canon_template_type_parm(depth, index, pack)))
        })
    }

    fn parm_flags(pack: bool) -> TypeFlags {
        if pack {
            TypeFlags::DEPENDENT_INSTANTIATION | TypeFlags::UNEXPANDED_PACK
        } else {
            TypeFlags::DEPENDENT_INSTANTIATION
        }
    }

    /// The canonical template type parameter at `(depth, index)`.
    /// `depth` is limited to 32767 (exhaustion beyond).
    pub fn canon_template_type_parm(&mut self, depth: u32, index: u32, pack: bool) -> TypeId {
        if depth > 0x7fff {
            return self.exhaust();
        }
        let aux = depth as u16 | (u16::from(pack) << 15);
        self.intern_with(
            Key::inline(TypeKind::CanonTemplateTypeParm, aux, index),
            Self::parm_flags(pack),
            |_| None,
        )
    }

    /// A substituted template type parameter: sugar over `replacement`
    /// recording that it replaced parameter `index` of `associated`
    /// (element `pack_index` of a pack). Pack indices ≥ 65535 are not
    /// recorded.
    pub fn subst_template_type_parm(
        &mut self,
        replacement: QualType,
        associated: DeclId,
        index: u32,
        pack_index: Option<u32>,
    ) -> QualType {
        let t: TypeId = {
            let aux = pack_index.filter(|&i| i < 0xffff).map_or(0, |i| i as u16 + 1);
            let key = Key::triple(
                TypeKind::SubstTemplateTypeParm,
                aux,
                raw(replacement),
                associated.raw(),
                index,
            );
            self.intern_with(key, self.qflags(replacement), |ctx| Some(ctx.canonical(replacement)))
        };
        t.into()
    }

    /// A template type parameter pack being substituted by `pack`.
    pub fn subst_template_type_parm_pack(
        &mut self,
        associated: DeclId,
        index: u32,
        pack: ArgListId,
    ) -> QualType {
        let t: TypeId = {
            let key = Key::triple(
                TypeKind::SubstTemplateTypeParmPack,
                0,
                associated.raw(),
                index,
                pack.raw(),
            );
            let flags = TypeFlags::DEPENDENT_INSTANTIATION | TypeFlags::UNEXPANDED_PACK;
            self.intern_with(key, flags, |ctx| {
                let c = ctx.canonical_args(pack);
                (c != pack)
                    .then(|| Self::uq(ctx.subst_template_type_parm_pack(associated, index, c)))
            })
        };
        t.into()
    }

    // ----- template specializations ---------------------------------------

    /// `name<args>`. For a class template specialization `underlying` is
    /// the specialization's record type (`None` when dependent); for an
    /// alias template (`alias`) it is the substituted aliased type.
    pub fn template_specialization(
        &mut self,
        name: TemplateNameId,
        args: ArgListId,
        underlying: Option<QualType>,
        alias: bool,
    ) -> QualType {
        let t: TypeId = {
            let nf = self.template_name_flags(name);
            let af = self.arg_list_flags(args);
            let mut flags = (nf & TypeFlags::UNEXPANDED_PACK) | af.without(TypeFlags::DEPENDENT);
            if underlying.is_none() && (nf | af).intersects(TypeFlags::DEPENDENT_INSTANTIATION) {
                flags |= TypeFlags::DEPENDENT_INSTANTIATION;
            }
            let key = Key::triple(
                TypeKind::TemplateSpecialization,
                u16::from(alias),
                name.raw(),
                args.raw(),
                raw_opt(underlying),
            );
            self.intern_with(key, flags, |ctx| {
                if let Some(u) = underlying {
                    return Some(ctx.canonical(u));
                }
                let cn = ctx.canonical_template_name(name);
                let ca = ctx.canonical_args(args);
                (cn != name || ca != args || alias)
                    .then(|| Self::uq(ctx.template_specialization(cn, ca, None, false)))
            })
        };
        t.into()
    }

    /// The injected-class-name of class template `pattern`: sugar over
    /// `injected`, the dependent specialization `X<T...>` (§17 T3).
    pub fn injected_class_name(&mut self, pattern: DeclId, injected: TypeId) -> TypeId {
        let inj = Self::uq(injected);
        self.sugar2(TypeKind::InjectedClassName, 0, pattern.raw(), inj, self.qflags(inj))
    }

    /// `[typename] nns::name` with a dependent `nns`.
    pub fn dependent_name(&mut self, keyword: TagKeyword, nns: NnsId, name: Symbol) -> QualType {
        let t: TypeId = {
            let flags = TypeFlags::DEPENDENT_INSTANTIATION | self.nns_flags(nns);
            let key = Key::pair(TypeKind::DependentName, keyword as u16, nns.raw(), name.index());
            self.intern_with(key, flags, |ctx| {
                let ck = Self::canonical_keyword(keyword);
                let cn = ctx.canonical_nns(nns);
                (ck != keyword || cn != nns).then(|| Self::uq(ctx.dependent_name(ck, cn, name)))
            })
        };
        t.into()
    }

    fn canonical_keyword(k: TagKeyword) -> TagKeyword {
        if k == TagKeyword::None { TagKeyword::Typename } else { k }
    }

    /// `[typename] nns::template name<args>` with a dependent `nns`.
    pub fn dependent_template_specialization(
        &mut self,
        keyword: TagKeyword,
        nns: NnsId,
        name: Symbol,
        args: ArgListId,
    ) -> QualType {
        let t: TypeId = {
            let flags = TypeFlags::DEPENDENT_INSTANTIATION
                | self.nns_flags(nns)
                | self.arg_list_flags(args);
            let key = Key::triple(
                TypeKind::DependentTemplateSpecialization,
                keyword as u16,
                nns.raw(),
                name.index(),
                args.raw(),
            );
            self.intern_with(key, flags, |ctx| {
                let ck = Self::canonical_keyword(keyword);
                let cn = ctx.canonical_nns(nns);
                let ca = ctx.canonical_args(args);
                (ck != keyword || cn != nns || ca != args)
                    .then(|| Self::uq(ctx.dependent_template_specialization(ck, cn, name, ca)))
            })
        };
        t.into()
    }

    /// Convenience: the template name for template `decl`.
    pub fn template_name_of(&mut self, decl: DeclId) -> TemplateNameId {
        self.template_name(TemplateName::Template(decl))
    }

    // ----- packs --------------------------------------------------------------

    /// `pattern...`.
    pub fn pack_expansion(&mut self, pattern: QualType, num_expansions: Option<u32>) -> QualType {
        let t: TypeId = {
            let Some(n) = num_expansions.map_or(Some(0), |n| n.checked_add(1)) else {
                return self.exhaust().into();
            };
            let flags = self.qflags(pattern).without(TypeFlags::UNEXPANDED_PACK)
                | TypeFlags::DEPENDENT_INSTANTIATION;
            self.intern_with(Key::pair(TypeKind::PackExpansion, 0, raw(pattern), n), flags, |ctx| {
                let c = ctx.canon_if(pattern)?;
                Some(Self::uq(ctx.pack_expansion(c, num_expansions)))
            })
        };
        t.into()
    }

    /// `pattern...[index]` (C++26). `selected` is the chosen element once
    /// the index is known; otherwise `profile` is the index's profile.
    pub fn pack_indexing(
        &mut self,
        pattern: QualType,
        index: ExprId,
        profile: Option<ProfileId>,
        selected: Option<QualType>,
    ) -> QualType {
        let t: TypeId = {
            let flags = match selected {
                Some(s) => self.qflags(s) | (self.qflags(pattern) & TypeFlags::CONTAINS_ERROR),
                None => {
                    self.qflags(pattern).without(TypeFlags::UNEXPANDED_PACK)
                        | profile.map_or(TypeFlags::NONE, |p| sem(self.profile_flags(p)))
                        | TypeFlags::DEPENDENT_INSTANTIATION
                }
            };
            let key = Key::triple(
                TypeKind::PackIndexing,
                0,
                raw(pattern),
                index.raw(),
                raw_opt(selected),
            );
            self.intern_with(key, flags, |ctx| match (selected, profile) {
                (Some(s), _) => Some(ctx.canonical(s)),
                (None, Some(p)) => Some(Self::uq(ctx.pack_indexing_canonical(pattern, p, flags))),
                (None, None) => None,
            })
        };
        t.into()
    }

    fn pack_indexing_canonical(
        &mut self,
        pattern: QualType,
        p: ProfileId,
        flags: TypeFlags,
    ) -> TypeId {
        let cp = self.canonical(pattern);
        let key = Key::triple(TypeKind::PackIndexing, AUX_PROFILE, raw(cp), p.raw(), 0);
        self.intern_with(key, flags, |_| None)
    }

    // ----- expression-based types -------------------------------------------

    /// `decltype(expr)`. `underlying` is the computed type (the
    /// `Dependent` builtin when it cannot be computed); `profile` is the
    /// expression's profile when it is instantiation-dependent, and then
    /// the canonical type is `DependentDecltype(profile)`.
    pub fn decltype_type(
        &mut self,
        expr: ExprId,
        underlying: QualType,
        profile: Option<ProfileId>,
    ) -> QualType {
        let t: TypeId = {
            let key = Key::pair(TypeKind::Decltype, 0, expr.raw(), raw(underlying));
            match profile {
                Some(p) => {
                    let flags = sem(self.profile_flags(p)) | TypeFlags::DEPENDENT_INSTANTIATION;
                    self.intern_with(key, flags, |ctx| Some(Self::uq(ctx.dependent_decltype(p))))
                }
                None => self.intern_with(key, self.qflags(underlying), |ctx| {
                    Some(ctx.canonical(underlying))
                }),
            }
        };
        t.into()
    }

    /// The canonical type of every `decltype(e)` with profile `p`.
    pub fn dependent_decltype(&mut self, p: ProfileId) -> QualType {
        let t: TypeId = {
            let flags = sem(self.profile_flags(p)) | TypeFlags::DEPENDENT_INSTANTIATION;
            self.intern_with(Key::inline(TypeKind::DependentDecltype, 0, p.raw()), flags, |_| None)
        };
        t.into()
    }

    /// `typeof(expr)` / `typeof_unqual(expr)` (C23/GNU).
    pub fn typeof_expr(
        &mut self,
        expr: ExprId,
        underlying: QualType,
        unqual: bool,
        profile: Option<ProfileId>,
    ) -> QualType {
        let t: TypeId = {
            let aux = if unqual { TYPEOF_UNQUAL } else { 0 };
            let key = Key::pair(TypeKind::TypeOf, aux, expr.raw(), raw(underlying));
            match profile {
                Some(p) => {
                    let flags = sem(self.profile_flags(p)) | TypeFlags::DEPENDENT_INSTANTIATION;
                    self.intern_with(key, flags, |ctx| {
                        let k = Key::pair(TypeKind::TypeOf, aux | AUX_PROFILE, p.raw(), 0);
                        Some(Self::uq(ctx.intern_with(k, flags, |_| None)))
                    })
                }
                None => self.intern_with(key, self.qflags(underlying), |ctx| {
                    let c = ctx.canonical(underlying);
                    Some(if unqual { ctx.unqualified(c) } else { c })
                }),
            }
        };
        t.into()
    }

    /// `typeof(type)` / `typeof_unqual(type)`.
    pub fn typeof_type(&mut self, ty: QualType, unqual: bool) -> QualType {
        let t: TypeId = {
            let aux = TYPEOF_TYPE | if unqual { TYPEOF_UNQUAL } else { 0 };
            self.intern_with(Key::pair(TypeKind::TypeOf, aux, raw(ty), 0), self.qflags(ty), |ctx| {
                let c = ctx.canonical(ty);
                Some(if unqual { ctx.unqualified(c) } else { c })
            })
        };
        t.into()
    }

    /// A type-trait transform such as `__remove_cv(T)`; `result` is the
    /// transformed type once `base` is not dependent.
    pub fn unary_transform(
        &mut self,
        kind: UnaryTransformKind,
        base: QualType,
        result: Option<QualType>,
    ) -> QualType {
        let t: TypeId = {
            let key = Key::pair(TypeKind::UnaryTransform, kind as u16, raw(base), raw_opt(result));
            self.intern_with(key, self.qflags(base), |ctx| {
                if let Some(r) = result {
                    Some(ctx.canonical(r))
                } else {
                    let cb = ctx.canon_if(base)?;
                    Some(Self::uq(ctx.unary_transform(kind, cb, None)))
                }
            })
        };
        t.into()
    }

    // ----- placeholders ------------------------------------------------------

    /// `auto`, `decltype(auto)` or `C auto` (constrained by concept
    /// `concept` with extra arguments `args`), deduced as `deduced` once
    /// known.
    pub fn auto_type(
        &mut self,
        keyword: AutoKeyword,
        deduced: Option<QualType>,
        concept: Option<DeclId>,
        args: Option<ArgListId>,
        pack: bool,
    ) -> QualType {
        let t: TypeId = {
            let aux = keyword as u16 | (u16::from(pack) << 2);
            let flags = if let Some(d) = deduced {
                self.qflags(d).without(TypeFlags::VARIABLY_MODIFIED)
            } else {
                let mut f = TypeFlags::UNDEDUCED;
                if pack {
                    f |= TypeFlags::UNEXPANDED_PACK | TypeFlags::DEPENDENT_INSTANTIATION;
                }
                f | args.map_or(TypeFlags::NONE, |a| self.arg_list_flags(a))
            };
            let key = Key::triple(
                TypeKind::Auto,
                aux,
                raw_opt(deduced),
                DeclId::raw_opt(concept),
                ArgListId::raw_opt(args),
            );
            self.intern_with(key, flags, |ctx| {
                if let Some(d) = deduced {
                    Some(ctx.canonical(d))
                } else {
                    let ca = args.map(|a| ctx.canonical_args(a));
                    (ca != args).then(|| Self::uq(ctx.auto_type(keyword, None, concept, ca, pack)))
                }
            })
        };
        t.into()
    }

    /// A class template argument deduction placeholder `X` (C++17).
    pub fn deduced_template_specialization(
        &mut self,
        name: TemplateNameId,
        deduced: Option<QualType>,
    ) -> QualType {
        let t: TypeId = {
            let flags = match deduced {
                Some(d) => self.qflags(d),
                None => TypeFlags::UNDEDUCED | self.template_name_flags(name),
            };
            let key =
                Key::pair(TypeKind::DeducedTemplateSpecialization, 0, name.raw(), raw_opt(deduced));
            self.intern_with(key, flags, |ctx| {
                if let Some(d) = deduced {
                    Some(ctx.canonical(d))
                } else {
                    let cn = ctx.canonical_template_name(name);
                    (cn != name).then(|| Self::uq(ctx.deduced_template_specialization(cn, None)))
                }
            })
        };
        t.into()
    }

    // ----- misc sugar -----------------------------------------------------------

    /// A type with a type attribute: sugar over `equivalent`.
    pub fn attributed(
        &mut self,
        kind: TypeAttrKind,
        modified: QualType,
        equivalent: QualType,
    ) -> QualType {
        let t: TypeId = {
            let flags = self.qflags(modified);
            let key = Key::pair(TypeKind::Attributed, kind as u16, raw(modified), raw(equivalent));
            self.intern_with(key, flags, |ctx| Some(ctx.canonical(equivalent)))
        };
        t.into()
    }

    /// `typename [: expr :]` (P2996, M6). `designated` is the reflected
    /// type once known; otherwise `profile` is the expression's profile.
    pub fn splice_type(
        &mut self,
        expr: ExprId,
        profile: Option<ProfileId>,
        designated: Option<QualType>,
    ) -> QualType {
        let t: TypeId = {
            let flags = match (designated, profile) {
                (Some(d), _) => self.qflags(d),
                (None, p) => {
                    p.map_or(TypeFlags::NONE, |p| sem(self.profile_flags(p)))
                        | TypeFlags::DEPENDENT_INSTANTIATION
                }
            };
            let key = Key::pair(TypeKind::Splice, 0, expr.raw(), raw_opt(designated));
            self.intern_with(key, flags, |ctx| match (designated, profile) {
                (Some(d), _) => Some(ctx.canonical(d)),
                (None, Some(p)) => {
                    let k = Key::pair(TypeKind::Splice, AUX_PROFILE, p.raw(), 0);
                    Some(Self::uq(ctx.intern_with(k, flags, |_| None)))
                }
                (None, None) => None,
            })
        };
        t.into()
    }

    /// The operand of a `typeof` node as a [`TypeOfOperand`] (helper for
    /// printers).
    pub fn typeof_operand(&self, t: TypeId) -> Option<TypeOfOperand> {
        match self.view(t) {
            TypeView::TypeOf { operand, .. } => Some(operand),
            _ => None,
        }
    }
}
