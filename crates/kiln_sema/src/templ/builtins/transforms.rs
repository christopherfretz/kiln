//! The unary type transforms (spec/sema-templates.md §9.2): `__remove_cv(T)`
//! and friends, written as type specifiers and represented as
//! `UnaryTransform` sugar over the transformed type (clang's
//! `BuildUnaryTransformType`). A dependent operand gives an unresolved
//! `UnaryTransform` node that substitution rebuilds through
//! [`Sema::build_unary_transform`].

use kiln_ast::QualType;
use kiln_ast::types::{BuiltinType, Quals, RefQual, TypeFlags, TypeView, UnaryTransformKind};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::TraitFailure;
use crate::Sema;

impl Sema<'_, '_> {
    /// `__transform(T)` written at `loc`: the `UnaryTransform` type, or
    /// `None` after a diagnostic ([meta.trans.sign]/1, [meta.trans.other]
    /// `underlying_type`).
    pub fn build_unary_transform(
        &mut self,
        kind: UnaryTransformKind,
        base: QualType,
        loc: Loc,
    ) -> Option<QualType> {
        if base == QualType::ERROR {
            return None;
        }
        // Only a dependent type defers the transform (clang
        // `BuildUnaryTransformType` tests `isDependentType`); a merely
        // instantiation-dependent one (`TC<N>*`, `N` a constant local of a
        // function template, reaching an alias substituted with its
        // arguments as written at compat 22) is transformed now.
        let dep = TypeFlags::DEPENDENT.union(TypeFlags::UNEXPANDED_PACK);
        if self.ast.types.flags(base.ty()).intersects(dep) {
            return Some(self.ast.types.unary_transform(kind, base, None));
        }
        let result = self.apply_unary_transform(kind, base, loc)?;
        Some(self.ast.types.unary_transform(kind, base, Some(result)))
    }

    /// The transformed type of non-dependent `t`: the one evaluator behind
    /// both `__transform(T)` and the [meta.trans] reflection functions
    /// (spec/reflection-traits.md). `None` after an error, diagnosed or,
    /// while a reflection trait evaluates quietly, recorded.
    pub(crate) fn apply_unary_transform(
        &mut self,
        kind: UnaryTransformKind,
        t: QualType,
        loc: Loc,
    ) -> Option<QualType> {
        use UnaryTransformKind as K;
        Some(match kind {
            K::AddLvalueReference => {
                if self.referenceable(t) {
                    self.ast.types.lvalue_ref_to(t)
                } else {
                    t
                }
            }
            K::AddRvalueReference => {
                if self.referenceable(t) {
                    self.ast.types.rvalue_ref_to(t)
                } else {
                    t
                }
            }
            K::AddPointer => {
                let p = self.strip_reference(t);
                if self.ast.types.is_void(p) || self.referenceable(p) {
                    self.ast.types.pointer_to(p)
                } else {
                    t
                }
            }
            K::RemovePointer => match self.ast.types.canonical_view(t) {
                TypeView::Pointer(p) => p,
                _ => t,
            },
            K::RemoveReference => self.strip_reference(t),
            K::RemoveConst => self.remove_quals(t, Quals::CONST),
            K::RemoveVolatile => self.remove_quals(t, Quals::VOLATILE),
            K::RemoveRestrict => self.remove_quals(t, Quals::RESTRICT),
            K::RemoveCv => self.remove_quals(t, Quals::CV),
            K::RemoveCvref => {
                let r = self.strip_reference(t);
                self.remove_quals(r, Quals::CV)
            }
            K::RemoveExtent => {
                if self.ast.types.is_array(t) {
                    self.ast.types.element(t).unwrap_or(t)
                } else {
                    t
                }
            }
            K::RemoveAllExtents => self.base_element(t),
            K::Decay => {
                let r = self.strip_reference(t);
                if self.ast.types.is_array(r) {
                    let e = self.ast.types.element(r).unwrap_or(r);
                    self.ast.types.pointer_to(e)
                } else if self.ast.types.is_function(r) {
                    // clang `BuiltinDecay` defers to `BuiltinAddPointer`:
                    // an abominable function type (cv- or ref-qualified)
                    // has no pointer type and decays to itself.
                    if self.referenceable(r) { self.ast.types.pointer_to(r) } else { t }
                } else {
                    self.remove_quals(r, Quals::CV)
                }
            }
            K::MakeSigned | K::MakeUnsigned => {
                return self.change_signedness(t, kind == K::MakeSigned, loc);
            }
            K::UnderlyingType => {
                if self.ast.types.as_enum(t).is_none() {
                    if !self.transform_quiet(TraitFailure::BadOperand(t)) {
                        self.error(DiagId::sema_only_enums_have_underlying, loc);
                    }
                    return None;
                }
                // [meta.trans.other]: an enumeration without a fixed
                // underlying type is incomplete until its closing brace
                // ([dcl.enum]/6), and its underlying type is not yet
                // determined (clang `BuiltinEnumUnderlyingType`).
                if self.traits.quiet && !self.require_complete_silent(t, loc) {
                    self.transform_quiet(TraitFailure::Incomplete(t));
                    return None;
                }
                if !self.require_complete(t, loc) {
                    let ty = self.ty_arg(t);
                    let d = self.diag(DiagId::sema_underlying_type_of_incomplete_enum, loc).arg(ty);
                    let d = match self.ast.types.as_enum(t) {
                        Some(e) => {
                            let at = self.ast.latest(e);
                            let note = self
                                .diag(
                                    DiagId::note_forward_declaration,
                                    self.ast.decls.get(at).loc(),
                                )
                                .arg(format!("'{}'", self.qualified_decl_name(e)));
                            d.with_note(note)
                        }
                        None => d,
                    };
                    self.emit(d);
                    return None;
                }
                crate::conv::rank::enum_data(self, t).and_then(|d| d.int_type)?
            }
        })
    }

    /// Records `f` while a reflection trait evaluates quietly (true), else
    /// false and the caller diagnoses.
    fn transform_quiet(&mut self, f: TraitFailure) -> bool {
        self.traits.record(f)
    }

    /// `add_const` / `add_volatile` / `add_cv` ([meta.trans.cv]): `t` with
    /// `add` added at the top level; a reference or function type is
    /// unchanged. (No `__add_*` builtin exists: libc++ spells these as
    /// `T const`, which is the same rule.)
    pub(crate) fn add_quals_transform(&mut self, t: QualType, add: Quals) -> QualType {
        // `qualify` leaves references and functions alone and puts an
        // array's qualifiers on its element ([basic.type.qualifier]/3).
        self.ast.types.qualify(t, add)
    }

    /// [defns.referenceable]: an object type, a reference, or a function
    /// type without cv-qualifiers or a ref-qualifier.
    pub(crate) fn referenceable(&self, t: QualType) -> bool {
        let tys = &self.ast.types;
        match tys.canonical_view(t) {
            TypeView::Function(p) => {
                p.info().this_quals().is_empty() && p.info().ref_qual() == RefQual::None
            }
            _ => !tys.is_void(t),
        }
    }

    fn strip_reference(&self, t: QualType) -> QualType {
        match self.ast.types.canonical_view(t) {
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => p,
            _ => t,
        }
    }

    /// `t` without the qualifiers `drop` (through array element types;
    /// references and functions are unchanged).
    fn remove_quals(&mut self, t: QualType, drop: Quals) -> QualType {
        let tys = &mut self.ast.types;
        if tys.is_reference(t) || tys.is_function(t) {
            return t;
        }
        let have = tys.effective_quals(t);
        if (have.bits() & drop.bits()) == 0 {
            return t;
        }
        let keep = Quals::from_bits(have.bits() & !drop.bits());
        let u = tys.unqualified(t);
        tys.qualify(u, keep)
    }

    /// clang's `BuiltinChangeSignedness`: [meta.trans.sign].
    fn change_signedness(&mut self, t: QualType, signed: bool, loc: Loc) -> Option<QualType> {
        let quals = self.ast.types.effective_quals(t);
        let view = self.ast.types.canonical_view(t);
        let is_enum = matches!(view, TypeView::Enum(_));
        let ok = match view {
            TypeView::Builtin(b) => b.is_integral() && b != BuiltinType::Bool,
            TypeView::BitInt { width, .. } => width >= 2,
            TypeView::Enum(_) => true,
            _ => false,
        };
        if !ok {
            if !self.transform_quiet(TraitFailure::BadOperand(t)) {
                let d = self
                    .diag(DiagId::sema_make_signed_bad_type, loc)
                    .arg(u8::from(!signed))
                    .arg(self.ty_arg(t));
                self.emit(d);
            }
            return None;
        }
        let result = match view {
            TypeView::BitInt { width, .. } => self.ast.types.bit_int(signed, width),
            TypeView::Builtin(b)
                if !is_enum
                    && !matches!(
                        b,
                        BuiltinType::Char16 | BuiltinType::Char32 | BuiltinType::WChar
                    ) =>
            {
                corresponding(b, signed).qual_type()
            }
            _ => {
                if is_enum {
                    let int = crate::conv::rank::enum_data(self, t).and_then(|d| d.int_type);
                    match int.map(|i| self.ast.types.canonical_view(i)) {
                        Some(TypeView::BitInt { width, .. }) => {
                            let r = self.ast.types.bit_int(signed, width);
                            return Some(self.ast.types.qualify(r, quals));
                        }
                        Some(TypeView::Builtin(BuiltinType::Bool)) | None => {
                            if !self.transform_quiet(TraitFailure::BadOperand(t)) {
                                let d = self
                                    .diag(DiagId::sema_make_signed_bad_type, loc)
                                    .arg(u8::from(!signed))
                                    .arg(self.ty_arg(t));
                                self.emit(d);
                            }
                            return None;
                        }
                        _ => {}
                    }
                }
                let size = crate::target::size_align(&self.ast, t, &self.abi).map(|s| s.0)?;
                let cands: [BuiltinType; 6] = if signed {
                    [
                        BuiltinType::SChar,
                        BuiltinType::Short,
                        BuiltinType::Int,
                        BuiltinType::Long,
                        BuiltinType::LongLong,
                        BuiltinType::Int128,
                    ]
                } else {
                    [
                        BuiltinType::UChar,
                        BuiltinType::UShort,
                        BuiltinType::UInt,
                        BuiltinType::ULong,
                        BuiltinType::ULongLong,
                        BuiltinType::UInt128,
                    ]
                };
                let found = cands.into_iter().find(|&c| {
                    crate::target::size_align(&self.ast, c.qual_type(), &self.abi)
                        .is_some_and(|s| s.0 == size)
                })?;
                found.qual_type()
            }
        };
        Some(self.ast.types.qualify(result, quals))
    }
}

/// clang's `getCorresponding{Signed,Unsigned}Type` for the standard and
/// character integer types.
fn corresponding(b: BuiltinType, signed: bool) -> BuiltinType {
    use BuiltinType as B;
    if signed {
        match b {
            B::Char | B::UChar | B::Char8 => B::SChar,
            B::UShort => B::Short,
            B::UInt => B::Int,
            B::ULong => B::Long,
            B::ULongLong => B::LongLong,
            B::UInt128 => B::Int128,
            other => other,
        }
    } else {
        match b {
            B::Char | B::SChar | B::Char8 => B::UChar,
            B::Short => B::UShort,
            B::Int => B::UInt,
            B::Long => B::ULong,
            B::LongLong => B::ULongLong,
            B::Int128 => B::UInt128,
            other => other,
        }
    }
}
