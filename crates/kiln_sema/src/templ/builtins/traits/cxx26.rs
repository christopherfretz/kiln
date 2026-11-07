//! The traits clang 20-22 added for C++26 library features
//! (spec/sema-templates.md §9.1; WP-263): `__builtin_is_virtual_base_of`
//! ([meta.rel], P2985), `__builtin_is_implicit_lifetime`
//! ([meta.unary.prop], P2674) and `__builtin_is_cpp_trivially_relocatable`
//! (clang's `[class.prop]` "trivially relocatable" without the P2786
//! keywords). Each follows clang 22.1 `SemaTypeTraits.cpp`. kiln accepts
//! them at every `-fclang-compat` level (a superset of clang 19, which
//! has none of them); only `__has_builtin` follows the level.

use kiln_ast::decl::DefinitionBits;
use kiln_ast::types::{Quals, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::class::{Selected, SpecialKind};
use crate::conv::bases;

/// The recursion bound of the subobject walk.
const MAX_DEPTH: usize = 256;

impl Sema<'_, '_> {
    /// `__builtin_is_virtual_base_of(Base, Derived)` (clang 22
    /// `BTT_IsVirtualBaseOf`): both non-union classes, `Derived` complete
    /// (else "incomplete type 'X' where a complete type is required"),
    /// and `Base` named by a `virtual` base-specifier somewhere in
    /// `Derived`'s hierarchy (`CXXRecordDecl::isVirtuallyDerivedFrom`).
    pub(super) fn is_virtual_base_of(&mut self, a: QualType, b: QualType, rhs_loc: Loc) -> bool {
        let (Some(base), Some(derived)) =
            (self.ast.types.as_record(a), self.ast.types.as_record(b))
        else {
            return false;
        };
        if self.is_union(base) || self.is_union(derived) {
            return false;
        }
        if !self.trait_complete(b, rhs_loc) {
            self.trait_incomplete(DiagId::sema_incomplete_type_required, rhs_loc, b);
            return false;
        }
        bases::base_paths(self, derived, base)
            .iter()
            .any(|p| p.last_virtual.is_some_and(|v| bases::same_class(self, v, base)))
    }

    /// `__builtin_is_implicit_lifetime(T)` ([basic.types.general]/9,
    /// [class.prop]/9; clang 22 `UTT_IsImplicitLifetime`): scalar, array
    /// and vector types, and implicit-lifetime classes (an aggregate whose
    /// destructor is not user-provided, or a class with a trivial
    /// non-deleted destructor and a trivial, eligible, non-deleted
    /// constructor).
    pub(super) fn is_implicit_lifetime(&mut self, q: QualType) -> bool {
        use SpecialKind as K;
        let tys = &self.ast.types;
        if tys.is_reference(q) {
            return false;
        }
        if self.is_scalar_ext(q)
            || tys.is_array(q)
            || matches!(tys.canonical_view(q), TypeView::Vector { .. })
        {
            return true;
        }
        let Some(r) = self.defined_record(q) else { return false };
        let r = self.class_def(r);
        let dtor = match self.select_special(r, K::Dtor, Quals::default(), Quals::default()) {
            Selected::Found(d) => Some((d, false)),
            Selected::Deleted(d) => Some((d, true)),
            _ => None,
        };
        let bits = self.defn_bits(r);
        if bits.contains(DefinitionBits::AGGREGATE)
            && dtor.is_none_or(|(d, _)| !self.is_user_provided(d))
        {
            return true;
        }
        if !bits.contains(DefinitionBits::TRIVIAL_DTOR) || dtor.is_some_and(|(_, del)| del) {
            return false;
        }
        let declared: Vec<DeclId> = self.ast.constructors(r).iter().collect();
        // clang skips an ineligible constructor ([special]/6: its
        // constraints are not satisfied).
        for c in declared {
            if self.is_deleted_fn(c) || !self.fn_spec(c).is_trivial() {
                continue;
            }
            let loc = self.ast.decls.get(c).loc();
            if self.check_function_constraints(c, loc).is_none_or(|s| self.sat_ok(s)) {
                return true;
            }
        }
        (self.needs_implicit(r, K::DefaultCtor)
            && bits.contains(DefinitionBits::TRIVIAL_DEFAULT_CTOR))
            || (self.needs_implicit(r, K::CopyCtor)
                && bits.contains(DefinitionBits::TRIVIAL_COPY_CTOR)
                && !self.known_deleted(r, K::CopyCtor))
            || (self.needs_implicit(r, K::MoveCtor)
                && self.has_trivial(r, K::MoveCtor)
                && !self.known_deleted(r, K::MoveCtor))
    }

    /// `__builtin_is_cpp_trivially_relocatable(T)` (clang 22
    /// `Sema::IsCXXTriviallyRelocatableType`): a scalar or vector type, or
    /// a class eligible for trivial relocation (no virtual base, every
    /// base and non-reference member trivially relocatable, destructor
    /// not deleted) that is a union without user-declared special members
    /// or is default-movable; arrays of these.
    pub(super) fn is_cxx_trivially_relocatable(&mut self, q: QualType, depth: usize) -> bool {
        let e = self.base_element(q);
        if !self.is_complete(e) || depth > MAX_DEPTH {
            return false;
        }
        if self.is_scalar_or_vector(e) {
            return true;
        }
        match self.defined_record(e) {
            Some(r) => self.cxx_relocatable_class(self.class_def(r), depth),
            None => false,
        }
    }

    pub(crate) fn cxx_relocatable_class(&mut self, r: DeclId, depth: usize) -> bool {
        use SpecialKind as K;
        if self.is_dependent_type(self.class_type(r)) {
            return false;
        }
        let specs: Vec<_> = self.ast.decls.bases(r, &self.ast.lists).copied().collect();
        for spec in specs {
            if self.ast.types.as_record(spec.ty).is_none() {
                continue;
            }
            if spec.is_virtual || !self.is_cxx_trivially_relocatable(spec.ty, depth + 1) {
                return false;
            }
        }
        for f in self.fields_of(r) {
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            if self.is_dependent_type(fty) || self.ast.types.is_reference(fty) {
                continue;
            }
            if !self.is_cxx_trivially_relocatable(fty, depth + 1) {
                return false;
            }
        }
        let dtor = self.select_special(r, K::Dtor, Quals::default(), Quals::default());
        if matches!(dtor, Selected::Deleted(_)) {
            return false;
        }
        let bits = self.defn_bits(r);
        if self.is_union(r)
            && [K::CopyCtor, K::CopyAssign, K::MoveCtor, K::MoveAssign, K::Dtor]
                .iter()
                .all(|k| !bits.contains(k.user_declared_bit()))
        {
            return true;
        }
        // Default-movable ([class.prop]): move construction and move
        // assignment from an xvalue select members that are neither
        // user-provided nor deleted, and the destructor is not
        // user-provided (one defaulted after its first declaration is;
        // clang 22 agrees). The simple copy operation stands in only when
        // the move operation is not user-declared (a user-declared move
        // is what an xvalue selects, and it deletes the implicit copy).
        for (mv, cp) in [(K::MoveCtor, K::CopyCtor), (K::MoveAssign, K::CopyAssign)] {
            let mv_declared = bits.contains(mv.user_declared_bit());
            if self.has_simple(r, mv) || (!mv_declared && self.has_simple(r, cp)) {
                continue;
            }
            match self.select_special(r, mv, Quals::default(), Quals::default()) {
                Selected::Found(d) if !self.is_user_provided(d) => {}
                _ => return false,
            }
        }
        match dtor {
            Selected::Found(d) => !self.is_user_provided(d),
            _ => true,
        }
    }
}
