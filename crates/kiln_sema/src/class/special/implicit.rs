//! Which special members a class has, their triviality and `constexpr`,
//! and their implicit declarations (spec/sema-class.md §4.1, §4.3, §4.6;
//! clang `CXXRecordDecl::addedMember` / `addedClassSubobject`,
//! `AddImplicitlyDeclaredMembersToClass`, `DeclareImplicit*`).

use kiln_ast::decl::{
    BaseSpec, ConstexprKind, DeclFlags, DeclKind, Defaulted, DefinitionBits, FieldData, FnSpec,
    TagKind,
};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{ExcSpec, FuncInfo, Quals, TypeView};
use kiln_ast::{DeclHead, DeclId, QualType};

use super::SpecialKind;
use crate::Sema;
use crate::class::r#virtual::is_virtual;

/// How deep the walks over bases and member classes go before giving up
/// (a cycle is impossible for complete classes; this bounds the stack).
const MAX_DEPTH: usize = 256;

/// Whether `k` is declared, given the definition bits and the
/// implicitly-declared set (clang `DeclaredSpecialMembers`).
fn declared(bits: DefinitionBits, implicit: u8, k: SpecialKind) -> bool {
    implicit & k.bit() != 0 || bits.contains(k.user_declared_bit())
}

/// clang `needsImplicit*` ([class.default.ctor]/1, [class.copy.ctor]/6
/// and /8, [class.copy.assign]/2 and /4, [class.dtor]/2): `k` would be
/// implicitly declared and has not been yet.
pub(crate) fn needs_from(bits: DefinitionBits, implicit: u8, k: SpecialKind) -> bool {
    if declared(bits, implicit, k) {
        return false;
    }
    let ud = |k: SpecialKind| bits.contains(k.user_declared_bit());
    match k {
        SpecialKind::MoveCtor => {
            !ud(SpecialKind::CopyCtor)
                && !ud(SpecialKind::CopyAssign)
                && !ud(SpecialKind::MoveAssign)
                && !ud(SpecialKind::Dtor)
        }
        SpecialKind::MoveAssign => {
            !ud(SpecialKind::CopyCtor)
                && !ud(SpecialKind::CopyAssign)
                && !ud(SpecialKind::MoveCtor)
                && !ud(SpecialKind::Dtor)
        }
        _ => true,
    }
}

/// clang `needsOverloadResolutionFor*`: whether the implicit `k` of a
/// class with `bits` must be declared to know its properties.
pub(crate) fn needs_ovl_from(bits: DefinitionBits, k: SpecialKind) -> bool {
    let moves = bits.contains(DefinitionBits::USER_DECLARED_MOVE_CTOR)
        || bits.contains(DefinitionBits::USER_DECLARED_MOVE_ASSIGN);
    match k {
        SpecialKind::CopyCtor | SpecialKind::CopyAssign if moves => true,
        _ => k.needs_ovl_bit().is_some_and(|b| bits.contains(b)),
    }
}

/// What the scan of a class's declared special members found.
#[derive(Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent facts of the scan")]
struct Declared {
    /// Kinds with a user-declared (non-implicit) member.
    user: u8,
    /// Kinds with a declared member that is not trivial.
    non_trivial: u8,
    /// Some user-declared constructor (template included) is `constexpr`
    /// and not a copy/move constructor.
    constexpr_ctor: bool,
    /// An in-class defaulted default constructor.
    defaulted_default_ctor: bool,
    /// A user-provided default constructor.
    provided_default_ctor: bool,
    /// Any user-declared constructor (template included).
    any_ctor: bool,
    /// The user-declared destructor is `constexpr` (or defaulted in
    /// class, decided by [`Sema::defaulted_is_constexpr`]).
    dtor_constexpr: Option<bool>,
}

impl Sema<'_, '_> {
    /// The definition of class `r` (or `r`).
    pub(crate) fn class_def(&self, r: DeclId) -> DeclId {
        self.ast.definition(r).unwrap_or(r)
    }

    /// The definition bits of class `r`: those being computed at its `}`
    /// while it is finishing, else the stored ones.
    pub(crate) fn defn_bits(&self, r: DeclId) -> DefinitionBits {
        let d = self.class_def(r);
        if let Some(&(_, b)) = self.special.finishing.iter().rev().find(|x| x.0 == d) {
            return b;
        }
        self.ast.decls.record(d).map(|x| x.defn).unwrap_or_default()
    }

    /// Updates the bits of finishing class `d` (no-op otherwise).
    fn set_finishing_bits(&mut self, d: DeclId, f: impl FnOnce(DefinitionBits) -> DefinitionBits) {
        if let Some(e) = self.special.finishing.iter_mut().rev().find(|x| x.0 == d) {
            e.1 = f(e.1);
        }
    }

    /// The implicitly declared special members of class `r`.
    fn implicit_bits(&self, r: DeclId) -> u8 {
        self.implicit_declared.get(&self.class_def(r)).copied().unwrap_or(0)
    }

    /// Whether the special member `k` of class `r` would be implicitly
    /// declared and has not been yet.
    pub(crate) fn needs_implicit(&self, r: DeclId, k: SpecialKind) -> bool {
        needs_from(self.defn_bits(r), self.implicit_bits(r), k)
    }

    /// Whether class `r` has a special member `k` (declared, or to be
    /// implicitly declared; clang `hasDefaultConstructor`,
    /// `hasMoveConstructor`, `hasMoveAssignment`).
    pub(crate) fn has_special(&self, r: DeclId, k: SpecialKind) -> bool {
        let bits = self.defn_bits(r);
        let implicit = self.implicit_bits(r);
        (declared(bits, implicit, k)
            && (k != SpecialKind::DefaultCtor || self.has_default_ctor_decl(r)))
            || needs_from(bits, implicit, k)
    }

    /// Whether class `r` declares a default constructor.
    fn has_default_ctor_decl(&self, r: DeclId) -> bool {
        let d = self.class_def(r);
        self.ast
            .constructors(d)
            .iter()
            .any(|c| self.special_kind_of(c) == Some(SpecialKind::DefaultCtor))
    }

    /// Whether the defaulted `k` of class `r` is known deleted (clang
    /// `Defaulted*IsDeleted`).
    pub(crate) fn known_deleted(&self, r: DeclId, k: SpecialKind) -> bool {
        let d = self.class_def(r);
        k.defaulted_deleted_bit().is_some_and(|b| self.defn_bits(d).contains(b))
            || self.special.deleted_implicit.get(&d).is_some_and(|m| m & k.bit() != 0)
    }

    /// clang `hasSimple*`: whether the `k` of class `r` is known to be the
    /// non-deleted implicit (or defaulted) one without overload
    /// resolution.
    fn has_simple(&self, r: DeclId, k: SpecialKind) -> bool {
        let bits = self.defn_bits(r);
        if bits.contains(k.user_declared_bit()) || self.known_deleted(r, k) {
            return false;
        }
        match k {
            SpecialKind::MoveCtor | SpecialKind::MoveAssign => self.has_special(r, k),
            _ => true,
        }
    }

    /// clang `hasTrivial*`: whether class `r` has a trivial `k`.
    fn has_trivial(&self, r: DeclId, k: SpecialKind) -> bool {
        let bits = self.defn_bits(r);
        match k {
            SpecialKind::MoveCtor | SpecialKind::MoveAssign => {
                self.has_special(r, k) && bits.contains(k.trivial_bit())
            }
            _ => bits.contains(k.trivial_bit()),
        }
    }

    /// clang `hasCopyConstructorWithConstParam` /
    /// `hasCopyAssignmentWithConstParam` of class `r`.
    fn copy_has_const_param(&self, r: DeclId, k: SpecialKind) -> bool {
        let d = self.class_def(r);
        let bits = self.defn_bits(d);
        let implicit_bit = match k {
            SpecialKind::CopyCtor => DefinitionBits::IMPLICIT_COPY_CTOR_CONST_PARAM,
            _ => DefinitionBits::IMPLICIT_COPY_ASSIGN_CONST_PARAM,
        };
        if self.needs_implicit(d, k) && bits.contains(implicit_bit) {
            return true;
        }
        let cands: Vec<DeclId> = if k == SpecialKind::CopyCtor {
            self.ast.constructors(d).iter().collect()
        } else {
            self.ast.lookup_local(d, DeclName::operator(OverloadedOperator::Equal)).iter().collect()
        };
        cands.into_iter().any(|m| self.special_kind_of(m) == Some(k) && self.copy_param_is_const(m))
    }

    /// Whether the first parameter of copy constructor / assignment `m`
    /// accepts a `const` argument (a reference to `const`, or by value).
    pub(crate) fn copy_param_is_const(&self, m: DeclId) -> bool {
        let Some(&p) = self.param_decls(m).first() else { return false };
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        match self.ast.types.canonical_view(ty) {
            TypeView::LValueRef(t) | TypeView::RValueRef(t) => {
                self.ast.types.effective_quals(t).is_const()
            }
            _ => true,
        }
    }

    /// Whether class `r` can be const-default-initialized (clang
    /// `allowConstDefaultInit`, [dcl.init.general]/8).
    fn can_const_default_init(&self, r: DeclId) -> bool {
        self.defn_bits(r).contains(DefinitionBits::CAN_CONST_DEFAULT_INIT)
    }

    /// The array element type of `q` (arrays of arrays stripped).
    pub(crate) fn base_element(&self, q: QualType) -> QualType {
        let mut t = q;
        for _ in 0..MAX_DEPTH {
            match self.ast.types.element(t) {
                Some(e) if self.ast.types.is_array(t) => t = e,
                _ => break,
            }
        }
        t
    }

    /// The complete class of the base-element type of `q`, if any.
    pub(crate) fn element_class(&self, q: QualType) -> Option<DeclId> {
        let d = self.ast.types.as_record(self.base_element(q))?;
        let d = self.ast.definition(d)?;
        self.defn_bits(d).contains(DefinitionBits::COMPLETE).then_some(d)
    }

    /// The class of base specifier `b`, if complete.
    pub(crate) fn base_class(&self, b: &BaseSpec) -> Option<DeclId> {
        let d = self.ast.types.as_record(b.ty)?;
        let d = self.ast.definition(d)?;
        self.defn_bits(d).contains(DefinitionBits::COMPLETE).then_some(d)
    }

    /// The virtual base classes of class `r`, in clang's `vbases()` order:
    /// per direct base, its virtual bases, then itself if virtual; each
    /// class once. The specifier is the one naming it (in `r` or in the
    /// base that introduced it).
    pub(crate) fn virtual_bases(&self, r: DeclId) -> Vec<(DeclId, BaseSpec)> {
        let mut out: Vec<(DeclId, BaseSpec)> = Vec::new();
        self.collect_vbases(self.class_def(r), &mut out, 0);
        out
    }

    fn collect_vbases(&self, r: DeclId, out: &mut Vec<(DeclId, BaseSpec)>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let bases: Vec<BaseSpec> = self.ast.decls.bases(r, &self.ast.lists).copied().collect();
        for b in bases {
            let Some(bd) = self.base_class(&b) else { continue };
            if self.defn_bits(bd).contains(DefinitionBits::HAS_VIRTUAL_BASES) {
                let mut inner = Vec::new();
                self.collect_vbases(bd, &mut inner, depth + 1);
                for (c, s) in inner {
                    if !out.iter().any(|x| self.ast.first(x.0) == self.ast.first(c)) {
                        out.push((c, s));
                    }
                }
            }
            if b.is_virtual && !out.iter().any(|x| self.ast.first(x.0) == self.ast.first(bd)) {
                out.push((bd, b));
            }
        }
    }

    /// Whether field `f` has a default member initializer.
    pub(crate) fn field_has_nsdmi(&self, f: DeclId) -> bool {
        self.special.nsdmi.contains(&f) || self.ast.decls.field(f).is_some_and(|x| x.init.is_some())
    }

    /// Whether field `f` is `mutable`.
    pub(crate) fn field_is_mutable(&self, f: DeclId) -> bool {
        self.ast.decls.field(f).is_some_and(|x| x.flags & FieldData::MUTABLE != 0)
    }

    /// Whether field `f` is an unnamed bit-field (not a member for the
    /// special-member rules, [class.bit]/2).
    pub(crate) fn is_unnamed_bit_field(&self, f: DeclId) -> bool {
        self.ast.decls.field(f).is_some_and(|x| x.bit_width.is_some())
            && self.ast.decls.get(f).name().is_empty()
    }

    /// Whether field `f` is an anonymous struct or union member.
    pub(crate) fn is_anonymous_member(&self, f: DeclId) -> bool {
        let Some(t) = self.ast.decls.get(f).ty() else { return false };
        self.ast
            .types
            .as_record(t)
            .and_then(|d| self.ast.decls.record(self.class_def(d)))
            .is_some_and(|x| x.defn.contains(DefinitionBits::ANONYMOUS))
    }

    /// The fields of class `r` the special-member rules consider (no
    /// unnamed bit-fields).
    pub(crate) fn special_fields(&self, r: DeclId) -> Vec<DeclId> {
        self.ast
            .decls
            .members(self.class_def(r))
            .filter(|&f| self.kind_of(f) == DeclKind::Field && !self.is_unnamed_bit_field(f))
            .collect()
    }

    /// Whether class `r` is a union.
    pub(crate) fn is_union(&self, r: DeclId) -> bool {
        self.ast.decls.record(self.class_def(r)).is_some_and(|x| x.tag == TagKind::Union)
    }

    /// Whether an implicit or defaulted `k` of a class with `bits` is
    /// `constexpr` (spec/sema-class.md §4.6, C++23 and later: unless a
    /// constructor or destructor of a class with a virtual base).
    pub(crate) fn defaulted_is_constexpr(bits: DefinitionBits, k: SpecialKind) -> bool {
        k.is_assign() || !bits.contains(DefinitionBits::HAS_VIRTUAL_BASES)
    }

    /// Whether `m`, a defaulted or deleted `k` of a class whose subobjects
    /// allow a trivial `k` (`class_trivial`), is trivial
    /// ([class.default.ctor]/3, [class.copy.ctor]/11, [class.copy.assign]/9,
    /// [class.dtor]/8; clang `SpecialMemberIsTrivial`'s signature checks).
    fn defaulted_is_trivial(&self, m: DeclId, k: SpecialKind, class_trivial: bool) -> bool {
        if !class_trivial {
            return false;
        }
        let params = self.param_decls(m);
        match k {
            SpecialKind::DefaultCtor => {
                if !params.is_empty() {
                    return false;
                }
            }
            SpecialKind::Dtor => {
                if is_virtual(&self.ast, m) {
                    return false;
                }
            }
            _ => {
                if params.len() != 1 {
                    return false;
                }
                let ty = self.ast.decls.get(params[0]).ty().unwrap_or(QualType::ERROR);
                let ok = match self.ast.types.canonical_view(ty) {
                    TypeView::LValueRef(_) => !k.is_move(),
                    TypeView::RValueRef(t) => {
                        k.is_move() && self.ast.types.effective_quals(t).is_empty()
                    }
                    _ => false,
                };
                if !ok {
                    return false;
                }
            }
        }
        !params.iter().any(|&p| self.param_has_default(p))
    }

    /// The scan of class `r`'s own special members at its `}`.
    fn scan_declared(&mut self, r: DeclId, class_trivial: DefinitionBits) -> Declared {
        let mut out = Declared::default();
        let members: Vec<DeclId> = self.ast.members(r).collect();
        let vbases = class_trivial.contains(DefinitionBits::HAS_VIRTUAL_BASES);
        for m in members {
            let f = if self.kind_of(m) == DeclKind::FunctionTemplate {
                match self.pattern_of(m) {
                    Some(p) => p,
                    None => continue,
                }
            } else {
                m
            };
            let fkind = self.kind_of(f);
            if !matches!(
                fkind,
                DeclKind::CXXConstructor | DeclKind::CXXDestructor | DeclKind::CXXMethod
            ) || !self.is_user_declared(f)
            {
                continue;
            }
            let spec = self.ast.decls.function(f).map(|x| x.spec).unwrap_or_default();
            if fkind == DeclKind::CXXConstructor {
                out.any_ctor = true;
            }
            let kind = self.special_kind_of(f);
            let in_class_default = spec.defaulted() == Defaulted::Explicit;
            if fkind == DeclKind::CXXConstructor
                && !matches!(kind, Some(SpecialKind::CopyCtor | SpecialKind::MoveCtor))
                && (spec.constexpr_kind() != ConstexprKind::None || in_class_default && !vbases)
            {
                out.constexpr_ctor = true;
            }
            let Some(k) = kind else { continue };
            out.user |= k.bit();
            match k {
                SpecialKind::DefaultCtor => {
                    if in_class_default {
                        out.defaulted_default_ctor = true;
                    }
                    if self.is_user_provided(f) {
                        out.provided_default_ctor = true;
                    }
                }
                SpecialKind::Dtor => {
                    out.dtor_constexpr = Some(
                        spec.constexpr_kind() != ConstexprKind::None || in_class_default && !vbases,
                    );
                }
                _ => {}
            }
            if self.is_user_provided(f) {
                out.non_trivial |= k.bit();
            } else if self.defaulted_is_trivial(f, k, class_trivial.contains(k.trivial_bit())) {
                self.ast.decls.mark_function(f, FnSpec::new().with_trivial(true));
            } else {
                out.non_trivial |= k.bit();
            }
        }
        out
    }

    /// The special-member definition bits of class `r` at its `}`, on top
    /// of `bits` from [`Sema::class_bits`] and
    /// [`Sema::virtual_class_bits`] (spec/sema-class.md §4.1–4.3, §4.6;
    /// clang `addedMember` / `addedClassSubobject`).
    #[allow(clippy::too_many_lines, reason = "one pass, as clang's addedMember")]
    pub(crate) fn special_member_bits(
        &mut self,
        r: DeclId,
        mut bits: DefinitionBits,
    ) -> DefinitionBits {
        use SpecialKind as K;
        let union = self.is_union(r);
        // The trivial bits so far say whether every subobject's
        // corresponding member is trivial (clang `HasTrivialSpecialMembers`);
        // a subobject without a move constructor / assignment has no
        // trivial one.
        let bases: Vec<BaseSpec> = self.ast.decls.bases(r, &self.ast.lists).copied().collect();
        let base_classes: Vec<(DeclId, bool)> =
            bases.iter().filter_map(|b| self.base_class(b).map(|d| (d, b.is_virtual))).collect();
        let fields = self.special_fields(r);
        let field_classes: Vec<(DeclId, DeclId)> = fields
            .iter()
            .filter_map(|&f| {
                let t = self.ast.decls.get(f).ty()?;
                if self.ast.types.is_reference(t) {
                    return None;
                }
                self.element_class(t).map(|c| (f, c))
            })
            .collect();
        let subobjects: Vec<DeclId> =
            base_classes.iter().map(|x| x.0).chain(field_classes.iter().map(|x| x.1)).collect();
        for &s in &subobjects {
            for k in [K::MoveCtor, K::MoveAssign] {
                if !self.has_trivial(s, k) {
                    bits = bits.without(k.trivial_bit());
                }
            }
        }
        // Subobjects whose members need overload resolution
        // (`addedClassSubobject`): direct bases, virtual bases, fields.
        let vbases = self.virtual_bases(r);
        let ovl_subobjects: Vec<DeclId> = base_classes
            .iter()
            .map(|x| x.0)
            .chain(vbases.iter().map(|x| x.0))
            .chain(field_classes.iter().map(|x| x.1))
            .collect();
        for s in ovl_subobjects {
            for k in [K::CopyCtor, K::MoveCtor, K::CopyAssign, K::MoveAssign] {
                if let Some(b) = k.needs_ovl_bit()
                    && !self.has_simple(s, k)
                {
                    bits = bits.with(b);
                }
            }
            if !self.has_simple(s, K::Dtor) {
                bits = bits
                    .with(DefinitionBits::NEEDS_OVL_COPY_CTOR)
                    .with(DefinitionBits::NEEDS_OVL_MOVE_CTOR)
                    .with(DefinitionBits::NEEDS_OVL_DTOR);
            }
        }
        // Fields ([class.copy.ctor]/10, [class.copy.assign]/7,
        // [class.union.general]/3).
        let d = self.class_def(r);
        let mut assign_deleted = 0u8;
        let mut uninit_fields = false;
        for &f in &fields {
            let Some(fty) = self.ast.decls.get(f).ty() else { continue };
            let has_init = self.field_has_nsdmi(f);
            let mutable = self.field_is_mutable(f);
            match self.ast.types.canonical_view(fty) {
                TypeView::RValueRef(_) => {
                    bits = bits.with(DefinitionBits::DEFAULTED_COPY_CTOR_DELETED);
                    assign_deleted |= K::CopyAssign.bit() | K::MoveAssign.bit();
                }
                TypeView::LValueRef(_) => {
                    assign_deleted |= K::CopyAssign.bit() | K::MoveAssign.bit();
                }
                _ => {}
            }
            let is_ref = self.ast.types.is_reference(fty);
            let quals = self.ast.types.effective_quals(fty);
            let elem = self.base_element(fty);
            let class = if is_ref { None } else { self.element_class(fty) };
            if is_ref && !has_init {
                bits = bits.with(DefinitionBits::HAS_UNINIT_REF_OR_CONST);
            }
            if !is_ref
                && quals.is_const()
                && !has_init
                && class.is_none_or(|c| !self.can_const_default_init(c))
            {
                bits = bits.with(DefinitionBits::HAS_UNINIT_REF_OR_CONST);
            }
            if !has_init && !mutable && !is_ref {
                match class {
                    Some(c) => {
                        if !self.can_const_default_init(c) {
                            uninit_fields = true;
                        }
                    }
                    None => {
                        if self.ast.types.as_record(elem).is_none() {
                            uninit_fields = true;
                        }
                    }
                }
            } else if is_ref && !has_init {
                uninit_fields = true;
            }
            let Some(c) = class else {
                if !is_ref && quals.is_const() {
                    assign_deleted |= K::CopyAssign.bit() | K::MoveAssign.bit();
                }
                continue;
            };
            if quals.contains(Quals::CONST) || quals.contains(Quals::VOLATILE) {
                bits = bits
                    .with(DefinitionBits::NEEDS_OVL_COPY_CTOR)
                    .with(DefinitionBits::NEEDS_OVL_MOVE_CTOR)
                    .with(DefinitionBits::NEEDS_OVL_COPY_ASSIGN)
                    .with(DefinitionBits::NEEDS_OVL_MOVE_ASSIGN);
            }
            let cb = self.defn_bits(c);
            if union {
                if !cb.contains(DefinitionBits::TRIVIAL_COPY_CTOR) {
                    bits = bits.with(DefinitionBits::DEFAULTED_COPY_CTOR_DELETED);
                }
                if !cb.contains(DefinitionBits::TRIVIAL_MOVE_CTOR) {
                    bits = bits.with(DefinitionBits::DEFAULTED_MOVE_CTOR_DELETED);
                }
                if !cb.contains(DefinitionBits::TRIVIAL_MOVE_ASSIGN) {
                    assign_deleted |= K::MoveAssign.bit();
                }
                if !cb.contains(DefinitionBits::TRIVIAL_DTOR) {
                    bits = bits.with(DefinitionBits::DEFAULTED_DTOR_DELETED);
                }
            }
            if self.is_anonymous_member(f) {
                for k in [K::CopyCtor, K::MoveCtor, K::CopyAssign, K::MoveAssign, K::Dtor] {
                    if let Some(b) = k.needs_ovl_bit()
                        && cb.contains(b)
                    {
                        bits = bits.with(b);
                    }
                    if let Some(b) = k.defaulted_deleted_bit()
                        && cb.contains(b)
                    {
                        bits = bits.with(b);
                    }
                }
                if let Some(m) = self.special.deleted_implicit.get(&c) {
                    assign_deleted |= m & (K::CopyAssign.bit() | K::MoveAssign.bit());
                }
            }
        }
        if assign_deleted != 0 {
            *self.special.deleted_implicit.entry(d).or_default() |= assign_deleted;
        }
        // Bases: const-default-constructibility.
        for &(b, _) in &base_classes {
            if !self.can_const_default_init(b) {
                uninit_fields = true;
            }
        }
        // Own members.
        let decl = self.scan_declared(r, bits);
        let ud_bits = [
            (K::CopyCtor, DefinitionBits::USER_DECLARED_COPY_CTOR),
            (K::MoveCtor, DefinitionBits::USER_DECLARED_MOVE_CTOR),
            (K::CopyAssign, DefinitionBits::USER_DECLARED_COPY_ASSIGN),
            (K::MoveAssign, DefinitionBits::USER_DECLARED_MOVE_ASSIGN),
            (K::Dtor, DefinitionBits::USER_DECLARED_DTOR),
        ];
        for (k, b) in ud_bits {
            if decl.user & k.bit() != 0 {
                bits = bits.with(b);
            }
        }
        if decl.any_ctor {
            bits = bits.with(DefinitionBits::USER_DECLARED_CTOR).without(DefinitionBits::AGGREGATE);
        }
        if bits.contains(DefinitionBits::HAS_INHERITED_CTOR) {
            bits = bits.without(DefinitionBits::AGGREGATE);
        }
        if decl.provided_default_ctor {
            bits = bits.with(DefinitionBits::USER_PROVIDED_DEFAULT_CTOR);
        }
        let implicit = self.implicit_bits(d);
        // Triviality (clang `hasNonTrivial*` for copy/move/dtor,
        // `hasTrivialDefaultConstructor` for the default ctor).
        for k in K::ALL {
            let ht = bits.contains(k.trivial_bit());
            let trivial = if decl.user & k.bit() != 0 {
                decl.non_trivial & k.bit() == 0
            } else if needs_from(bits, implicit, k) {
                ht
            } else {
                // Not declared and never will be: a move (no non-trivial
                // one) or the default ctor (none at all).
                k != K::DefaultCtor
            };
            bits = if trivial { bits.with(k.trivial_bit()) } else { bits.without(k.trivial_bit()) };
        }
        let copyable = [K::CopyCtor, K::MoveCtor, K::CopyAssign, K::MoveAssign, K::Dtor]
            .iter()
            .all(|k| bits.contains(k.trivial_bit()));
        bits = if copyable {
            bits.with(DefinitionBits::TRIVIALLY_COPYABLE)
        } else {
            bits.without(DefinitionBits::TRIVIALLY_COPYABLE)
        };
        // Parameter forms of the implicit copy members
        // ([class.copy.ctor]/7, [class.copy.assign]/2).
        let mut ctor_const_direct = true;
        let mut ctor_const_vbase = true;
        let mut assign_const = true;
        for &(b, virt) in &base_classes {
            if !virt && !self.copy_has_const_param(b, K::CopyCtor) {
                ctor_const_direct = false;
            }
            if !self.copy_has_const_param(b, K::CopyAssign) {
                assign_const = false;
            }
        }
        for (v, _) in &vbases {
            if !self.copy_has_const_param(*v, K::CopyCtor) {
                ctor_const_vbase = false;
            }
        }
        for &(_, c) in &field_classes {
            if !self.copy_has_const_param(c, K::CopyCtor) {
                ctor_const_direct = false;
            }
            if !self.copy_has_const_param(c, K::CopyAssign) {
                assign_const = false;
            }
        }
        if ctor_const_direct && (bits.contains(DefinitionBits::ABSTRACT) || ctor_const_vbase) {
            bits = bits.with(DefinitionBits::IMPLICIT_COPY_CTOR_CONST_PARAM);
        }
        if assign_const {
            bits = bits.with(DefinitionBits::IMPLICIT_COPY_ASSIGN_CONST_PARAM);
        }
        // `constexpr` and literal types (§4.6, [basic.types.general]/10).
        let no_vbases = !bits.contains(DefinitionBits::HAS_VIRTUAL_BASES);
        if no_vbases {
            bits = bits.with(DefinitionBits::DEFAULTED_DEFAULT_CTOR_CONSTEXPR);
        }
        let needs_default = needs_from(bits, implicit, K::DefaultCtor);
        if decl.constexpr_ctor || needs_default && no_vbases {
            bits = bits.with(DefinitionBits::HAS_CONSTEXPR_CTOR);
        }
        let constexpr_dtor = decl.dtor_constexpr.unwrap_or(no_vbases);
        let literal = constexpr_dtor
            && (bits.contains(DefinitionBits::AGGREGATE)
                || bits.contains(DefinitionBits::HAS_CONSTEXPR_CTOR)
                || bits.contains(DefinitionBits::TRIVIAL_DEFAULT_CTOR));
        if !literal {
            bits = bits.without(DefinitionBits::LITERAL).without(DefinitionBits::STRUCTURAL);
        }
        // [dcl.init.general]/8: const-default-constructible.
        if !uninit_fields || !(decl.defaulted_default_ctor || needs_default) {
            bits = bits.with(DefinitionBits::CAN_CONST_DEFAULT_INIT);
        }
        bits
    }

    /// The implicit declarations made at class `r`'s `}` (clang
    /// `AddImplicitlyDeclaredMembersToClass`; spec/sema-class.md §4.1),
    /// with its definition bits `bits` not yet stored. Returns the bits,
    /// updated by the declarations.
    pub(crate) fn finish_special_members(
        &mut self,
        r: DeclId,
        bits: DefinitionBits,
    ) -> DefinitionBits {
        use SpecialKind as K;
        self.special.finishing.push((r, bits));
        let flags = self.ast.decls.get(r).flags();
        if !flags.contains(DeclFlags::DEPENDENT_CTX) && !flags.contains(DeclFlags::INVALID) {
            let dynamic = bits.contains(DefinitionBits::POLYMORPHIC)
                || bits.contains(DefinitionBits::HAS_VIRTUAL_BASES);
            let inh_ctor = bits.contains(DefinitionBits::HAS_INHERITED_CTOR);
            let inh_assign = bits.contains(DefinitionBits::HAS_INHERITED_ASSIGN);
            for k in K::ALL {
                if !self.needs_implicit(r, k) {
                    continue;
                }
                let eager = match k {
                    K::DefaultCtor => inh_ctor,
                    K::CopyCtor | K::MoveCtor => needs_ovl_from(bits, k) || inh_ctor,
                    K::CopyAssign | K::MoveAssign => {
                        dynamic || needs_ovl_from(bits, k) || inh_assign
                    }
                    K::Dtor => dynamic || needs_ovl_from(bits, k),
                };
                if eager {
                    self.declare_implicit(r, k);
                }
            }
        }
        self.special.finishing.pop().map_or(bits, |x| x.1)
    }

    /// Declares the implicit special member `k` of class `r` if it needs
    /// one and it can be declared now (complete, not dependent; clang
    /// `LookupSpecialMember` / `ForceDeclarationOfImplicitMembers`).
    pub(crate) fn ensure_implicit(&mut self, class: DeclId, k: SpecialKind) {
        let Some(d) = self.ast.definition(class) else { return };
        let stored = self.ast.decls.record(d).map(|x| x.defn).unwrap_or_default();
        if !stored.contains(DefinitionBits::COMPLETE) {
            return;
        }
        let flags = self.ast.decls.get(d).flags();
        if flags.contains(DeclFlags::DEPENDENT_CTX) || flags.contains(DeclFlags::INVALID) {
            return;
        }
        if self.needs_implicit(d, k) {
            self.declare_implicit(d, k);
        }
    }

    /// Whether the implicit copy/move `k` of class `d`, which needs
    /// overload resolution, is trivial ([class.copy.ctor]/11,
    /// [class.copy.assign]/9; clang `SpecialMemberIsTrivial`): no virtual
    /// functions or bases, and the member selected for each direct base
    /// and class-type field is trivial. A failed selection does not make
    /// it non-trivial (the member is deleted then, as clang).
    fn selected_all_trivial(&mut self, d: DeclId, k: SpecialKind, const_arg: bool) -> bool {
        let bits = self.defn_bits(d);
        if bits.contains(DefinitionBits::POLYMORPHIC)
            || bits.contains(DefinitionBits::HAS_VIRTUAL_BASES)
        {
            return false;
        }
        let arg = if const_arg { Quals::CONST } else { Quals::NONE };
        let bases: Vec<BaseSpec> = self.ast.decls.bases(d, &self.ast.lists).copied().collect();
        // (class, argument quals, object quals), as `walk_subobjects`.
        let mut subs: Vec<(DeclId, Quals, Quals)> = bases
            .iter()
            .filter_map(|b| self.base_class(b))
            .map(|c| (c, arg, Quals::NONE))
            .collect();
        for f in self.special_fields(d) {
            let Some(t) = self.ast.decls.get(f).ty() else { continue };
            if self.ast.types.is_reference(t) {
                continue;
            }
            let elem = self.base_element(t);
            let Some(c) = self.element_class(elem) else { continue };
            let q = self.ast.types.effective_quals(elem) & Quals::CV;
            let rhs = if const_arg && !self.field_is_mutable(f) { q | Quals::CONST } else { q };
            subs.push((c, rhs, if k.is_assign() { q } else { Quals::NONE }));
        }
        subs.into_iter().all(|(c, rhs, lhs)| match self.select_special(c, k, rhs, lhs) {
            super::deletion::Selected::Found(m) | super::deletion::Selected::Deleted(m) => {
                self.ast.decls.function(m).is_some_and(|x| x.spec.is_trivial())
            }
            super::deletion::Selected::Unsupported => self.has_trivial(c, k),
            _ => true,
        })
    }

    /// Declares the implicit special member `k` of class `r` (clang
    /// `DeclareImplicit*`): public, inline, `Defaulted::Implicit`,
    /// `constexpr` per §4.6, trivial per the class bits, deleted per
    /// [`Sema::should_delete`], and the deferred exception specification.
    #[allow(clippy::too_many_lines, reason = "one step per property of the declaration")]
    pub(crate) fn declare_implicit(&mut self, r: DeclId, k: SpecialKind) -> DeclId {
        use SpecialKind as K;
        let d = self.class_def(r);
        let bits = self.defn_bits(d);
        *self.implicit_declared.entry(d).or_default() |= k.bit();
        let class_ty = self.class_type(d);
        let canon = self.ast.types.canonical(class_ty).ty();
        let copy_param = |s: &mut Self, konst: bool| {
            let t = if konst { s.ast.types.qualify(class_ty, Quals::CONST) } else { class_ty };
            s.ast.types.lvalue_ref_to(t)
        };
        let (kind, name, ret, params): (DeclKind, DeclName, QualType, Vec<QualType>) = match k {
            K::DefaultCtor => (
                DeclKind::CXXConstructor,
                self.ast.names.constructor(canon),
                QualType::VOID,
                vec![],
            ),
            K::CopyCtor => {
                let p =
                    copy_param(self, bits.contains(DefinitionBits::IMPLICIT_COPY_CTOR_CONST_PARAM));
                (
                    DeclKind::CXXConstructor,
                    self.ast.names.constructor(canon),
                    QualType::VOID,
                    vec![p],
                )
            }
            K::MoveCtor => {
                let p = self.ast.types.rvalue_ref_to(class_ty);
                (
                    DeclKind::CXXConstructor,
                    self.ast.names.constructor(canon),
                    QualType::VOID,
                    vec![p],
                )
            }
            K::CopyAssign | K::MoveAssign => {
                let p = if k == K::CopyAssign {
                    copy_param(
                        self,
                        bits.contains(DefinitionBits::IMPLICIT_COPY_ASSIGN_CONST_PARAM),
                    )
                } else {
                    self.ast.types.rvalue_ref_to(class_ty)
                };
                let ret = self.ast.types.lvalue_ref_to(class_ty);
                (DeclKind::CXXMethod, DeclName::operator(OverloadedOperator::Equal), ret, vec![p])
            }
            K::Dtor => {
                (DeclKind::CXXDestructor, self.ast.names.destructor(canon), QualType::VOID, vec![])
            }
        };
        let fty = self.ast.types.function(ret, &params, FuncInfo::new(), ExcSpec::None);
        let mut spec = FnSpec::new().with_inline(true).with_defaulted(Defaulted::Implicit);
        if Self::defaulted_is_constexpr(bits, k) {
            spec = spec.with_constexpr_kind(ConstexprKind::Constexpr);
        }
        let trivial = match k.needs_ovl_bit() {
            Some(n) if k != K::Dtor && bits.contains(n) => {
                let const_arg = k == K::CopyCtor
                    && bits.contains(DefinitionBits::IMPLICIT_COPY_CTOR_CONST_PARAM)
                    || k == K::CopyAssign
                        && bits.contains(DefinitionBits::IMPLICIT_COPY_ASSIGN_CONST_PARAM);
                self.selected_all_trivial(d, k, const_arg)
            }
            _ => bits.contains(k.trivial_bit()),
        };
        if trivial {
            spec = spec.with_trivial(true);
        }
        let overridden = if k == K::Dtor { self.overridden_destructors(d) } else { Vec::new() };
        if !overridden.is_empty() {
            spec = spec.with_virtual(true);
        }
        let loc = self.ast.decls.get(d).loc();
        let h = DeclHead::new(d, name, loc)
            .flags(DeclFlags::IMPLICIT)
            .access(kiln_ast::decl::Access::Public);
        let f = self.builder().append_implicit_member(h, kind, fty, spec);
        let ps: Vec<DeclId> = params
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                let i = u16::try_from(i).unwrap_or(u16::MAX);
                self.builder().param(f, DeclName::EMPTY, loc, t, i, 0)
            })
            .collect();
        if !ps.is_empty() {
            let _ = self.builder().set_params(f, &ps);
        }
        let _ = self.builder().set_overrides(f, &overridden);
        let t = self.with_exc_spec(fty, ExcSpec::Deferred(f));
        self.ast.decls.set_ty(f, t);
        if self.should_delete(f, k, false) {
            self.ast.decls.mark_function(f, FnSpec::new().with_deleted(true));
            if let Some(b) = k.defaulted_deleted_bit() {
                if self.special.finishing.iter().any(|x| x.0 == d) {
                    self.set_finishing_bits(d, |x| x.with(b));
                } else {
                    *self.special.deleted_implicit.entry(d).or_default() |= k.bit();
                }
            }
        }
        f
    }

    /// The explicitly defaulted members of class `r` declared in it,
    /// checked once `r` is complete (clang `CheckCompletedCXXClass` →
    /// `CheckExplicitlyDefaultedSpecialMember`).
    pub(crate) fn check_pending_defaulted(&mut self, r: DeclId) {
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.special.defaulted)
            .into_iter()
            .partition(|&(f, _)| self.ast.decls.get(f).ctx().map(|c| self.class_def(c)) == Some(r));
        self.special.defaulted = rest;
        for (f, loc) in mine {
            if let Some(k) = self.special_kind_of(f) {
                self.check_defaulted(f, k, loc);
            }
        }
    }
}
