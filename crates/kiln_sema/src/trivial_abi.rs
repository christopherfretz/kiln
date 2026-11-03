//! `[[clang::trivial_abi]]` (WP-460; spec/sema.md "The `trivial_abi`
//! attribute", spec/abi.md §8.25): a class with the attribute in effect is
//! trivial for the purposes of calls, so it is passed and returned in
//! registers when its layout allows, and the callee destroys a by-value
//! parameter. Triviality for calls propagates to the implicit and
//! defaulted members of enclosing classes (clang `addedClassSubobject`'s
//! `HasTrivialSpecialMembersForCall`).

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DefinitionBits};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::Quals;
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;

use crate::Sema;
use crate::class::{Selected, SpecialKind};

/// The for-call bit of special member kind `k` (copy/move constructor,
/// destructor), if it has one.
pub(crate) const fn for_call_bit(k: SpecialKind) -> Option<DefinitionBits> {
    match k {
        SpecialKind::CopyCtor => Some(DefinitionBits::COPY_CTOR_FOR_CALL),
        SpecialKind::MoveCtor => Some(DefinitionBits::MOVE_CTOR_FOR_CALL),
        SpecialKind::Dtor => Some(DefinitionBits::DTOR_FOR_CALL),
        _ => None,
    }
}

const FOR_CALL: [(SpecialKind, DefinitionBits); 3] = [
    (SpecialKind::CopyCtor, DefinitionBits::COPY_CTOR_FOR_CALL),
    (SpecialKind::MoveCtor, DefinitionBits::MOVE_CTOR_FOR_CALL),
    (SpecialKind::Dtor, DefinitionBits::DTOR_FOR_CALL),
];

/// clang's `note_cannot_use_trivial_abi_reason` selector.
#[derive(Clone, Copy)]
#[repr(u8)]
enum Reason {
    AllDeleted = 0,
    Polymorphic = 1,
    NonTrivialBase = 2,
    VirtualBase = 3,
    NonTrivialField = 5,
}

impl Sema<'_, '_> {
    /// Whether member `k` of complete class `s` is trivial for the
    /// purposes of calls (clang `hasTrivial*ForCall`).
    pub(crate) fn trivial_for_call(&self, s: DeclId, k: SpecialKind) -> bool {
        self.has_trivial(s, k) || for_call_bit(k).is_some_and(|b| self.defn_bits(s).contains(b))
    }

    /// Whether declared special member `m` (kind `k`) of class `r`,
    /// neither deleted nor trivial, is trivial for the purposes of calls
    /// (clang `isTrivialForCall`): `r` has `trivial_abi` in effect, or
    /// `m` is defaulted and every subobject's corresponding member is.
    pub(crate) fn member_trivial_for_call(&self, r: DeclId, m: DeclId, k: SpecialKind) -> bool {
        let bits = self.defn_bits(r);
        bits.contains(DefinitionBits::TRIVIAL_ABI)
            || (!self.is_user_provided(m) && for_call_bit(k).is_some_and(|b| bits.contains(b)))
    }

    /// At the `}` of class `r`, once its bits are stored: whether its
    /// `trivial_abi` attribute is in effect (clang
    /// `checkIllFormedTrivialABIStruct`, then
    /// `setHasTrivialSpecialMemberForCall`), else which of its copy and
    /// move constructors and destructor are trivial for calls only
    /// because a subobject's are (clang `addedClassSubobject`,
    /// `CheckCompletedCXXClass`'s `setTrivialForCall`).
    pub(crate) fn trivial_abi_bits(&mut self, r: DeclId, bits: DefinitionBits) -> DefinitionBits {
        let flags = self.ast.decls.get(r).flags();
        if flags.contains(DeclFlags::DEPENDENT_CTX) || flags.contains(DeclFlags::INVALID) {
            return bits;
        }
        let attr = self.find_attr(r, AttrKind::TRIVIAL_ABI);
        let subobjects = self.call_subobjects(r);
        let any = DefinitionBits::COPY_CTOR_FOR_CALL
            .with(DefinitionBits::MOVE_CTOR_FOR_CALL)
            .with(DefinitionBits::DTOR_FOR_CALL);
        if attr.is_none() && !subobjects.iter().any(|&s| self.defn_bits(s).bits() & any.bits() != 0)
        {
            return bits;
        }
        self.special.finishing.push((r, bits));
        let add = self.for_call_bits(r, attr, &subobjects, any);
        self.special.finishing.pop();
        bits.with(add)
    }

    fn for_call_bits(
        &mut self,
        r: DeclId,
        attr: Option<kiln_ast::AttrId>,
        subobjects: &[DeclId],
        any: DefinitionBits,
    ) -> DefinitionBits {
        let mut add = DefinitionBits::empty();
        if let Some(a) = attr {
            if self.trivial_abi_in_effect(r, a) {
                return any.with(DefinitionBits::TRIVIAL_ABI);
            }
            // clang declares an implicit member eagerly at `}` (before
            // `checkIllFormedTrivialABIStruct` drops the attribute) as
            // trivial for calls because the attribute is still there
            // (`DeclareImplicitCopyConstructor`'s `setTrivialForCall`), and
            // the dropped attribute does not undo it: such a class can
            // still pass in registers. Modelled for ABI compatibility.
            let eager = self.implicit_bits(r);
            for (k, b) in FOR_CALL {
                if eager & k.bit() != 0 {
                    add = add.with(b);
                }
            }
        }
        let bits = self.defn_bits(r);
        let dynamic = bits.contains(DefinitionBits::POLYMORPHIC)
            || bits.contains(DefinitionBits::HAS_VIRTUAL_BASES);
        for (k, b) in FOR_CALL {
            if bits.contains(k.trivial_bit()) {
                continue;
            }
            let blocked = if k == SpecialKind::Dtor {
                self.ast.destructor(r).is_some_and(|d| crate::class::is_virtual(&self.ast, d))
            } else {
                dynamic
            };
            if blocked || self.has_user_provided(r, k) {
                continue;
            }
            if subobjects.iter().all(|&s| self.trivial_for_call(s, k)) {
                add = add.with(b);
            }
        }
        add
    }

    /// The complete classes of the direct and virtual bases of `r` and of
    /// its non-reference fields (array elements included).
    fn call_subobjects(&self, r: DeclId) -> Vec<DeclId> {
        let mut out: Vec<DeclId> =
            self.ast.decls.bases(r, &self.ast.lists).filter_map(|b| self.base_class(b)).collect();
        out.extend(self.virtual_bases(r).iter().map(|x| x.0));
        for f in self.special_fields(r) {
            let Some(t) = self.ast.decls.get(f).ty() else { continue };
            if self.ast.types.is_reference(t) {
                continue;
            }
            out.extend(self.element_class(t));
        }
        out
    }

    /// Whether `r` declares a user-provided special member `k`.
    fn has_user_provided(&self, r: DeclId, k: SpecialKind) -> bool {
        self.ast.members(r).any(|m| self.special_kind_of(m) == Some(k) && self.is_user_provided(m))
    }

    /// clang `checkIllFormedTrivialABIStruct`: false (with
    /// `-Wignored-attributes`, not for a template instantiation) when the
    /// attribute `a` on `r` cannot take effect.
    fn trivial_abi_in_effect(&mut self, r: DeclId, a: kiln_ast::AttrId) -> bool {
        let reason = self.trivial_abi_ill_formed(r);
        let Some(reason) = reason else { return true };
        let instantiation = self.inst.stack.depth() > 0
            || self.ast.spec_info(r).is_some_and(|(_, s)| {
                matches!(
                    s.kind,
                    SpecKind::ImplicitInstantiation
                        | SpecKind::ExplicitInstantiationDeclaration
                        | SpecKind::ExplicitInstantiationDefinition
                )
            });
        if !instantiation {
            let loc = self.ast.attrs.get(a).loc;
            let note = self
                .diag(DiagId::note_cannot_use_trivial_abi_reason, loc)
                .arg(self.decl_arg(r))
                .arg(reason as u8);
            let msg = self
                .diag(DiagId::sema_cannot_use_trivial_abi, loc)
                .arg(self.decl_arg(r))
                .with_note(note);
            self.emit(msg);
        }
        false
    }

    fn trivial_abi_ill_formed(&mut self, r: DeclId) -> Option<Reason> {
        use SpecialKind as K;
        if self.defn_bits(r).contains(DefinitionBits::POLYMORPHIC) {
            return Some(Reason::Polymorphic);
        }
        let bases: Vec<_> = self.ast.decls.bases(r, &self.ast.lists).copied().collect();
        for b in &bases {
            if let Some(c) = self.base_class(b)
                && !self.can_pass_in_registers(c)
            {
                return Some(Reason::NonTrivialBase);
            }
            if b.is_virtual {
                return Some(Reason::VirtualBase);
            }
        }
        for f in self.special_fields(r) {
            let Some(t) = self.ast.decls.get(f).ty() else { continue };
            if let Some(c) = self.element_class(t)
                && !self.ast.types.is_reference(t)
                && !self.can_pass_in_registers(c)
            {
                return Some(Reason::NonTrivialField);
            }
        }
        // `r` is not complete yet: ask the class rule directly.
        if self.cxx_relocatable_class(r, 0) {
            return None;
        }
        let ctor_alive = |s: &Self, k: SpecialKind| {
            (s.needs_implicit(r, k) && !s.known_deleted(r, k))
                || s.ast.members(r).any(|m| s.special_kind_of(m) == Some(k) && !s.is_deleted_fn(m))
        };
        if !ctor_alive(self, K::CopyCtor) && !ctor_alive(self, K::MoveCtor) {
            return Some(Reason::AllDeleted);
        }
        None
    }

    /// clang `CheckParmsForFunctionDef`: the destructor of a parameter
    /// `p` (of type `pty`) that the function destroys
    /// (`isParamDestroyedInCallee`) is referenced by its definition.
    pub(crate) fn mark_callee_destroyed_param(&mut self, p: DeclId, pty: QualType) {
        if self.in_template()
            || self.ast.decls.get(p).has(DeclFlags::INVALID)
            || self.is_dependent_type(pty)
            || !kiln_abi::call::is_destroyed_in_callee(&self.ast, pty)
        {
            return;
        }
        let Some(class) = self.element_class(pty) else { return };
        let class = self.class_def(class);
        if let Selected::Found(m) =
            self.select_special(class, SpecialKind::Dtor, Quals::NONE, Quals::NONE)
        {
            let loc = self.ast.decls.get(p).loc();
            self.mark_function_referenced(m, loc);
        }
    }
}
