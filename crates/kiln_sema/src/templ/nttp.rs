//! Non-type template parameters of class and reference type
//! ([temp.param]/6-7, [temp.arg.nontype]; spec/sema-templates.md §6.10):
//! the structural-type check of a parameter's type, and the template
//! parameter objects ([temp.param]/8) class-type arguments designate.

use kiln_ast::QualType;
use kiln_ast::decl::{Access, DeclKind, DefinitionBits};
use kiln_ast::types::TypeView;
use kiln_ast::DeclId;
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;

/// Recursion bound for the explanation walk (a class cannot contain
/// itself, but a broken one may refer to itself through errors).
const MAX_DEPTH: usize = 256;

impl Sema<'_, '_> {
    /// Checks the type of a non-type template parameter at `loc` (clang
    /// `CheckNonTypeTemplateParameterType`): it must be a structural
    /// type ([temp.param]/6), so a class must be literal with only
    /// public, non-mutable bases and members of structural type
    /// ([temp.param]/7). Dependent and placeholder types are checked
    /// when substituted or deduced. False after a diagnostic.
    pub(crate) fn check_nttp_type(&mut self, ty: QualType, loc: Loc) -> bool {
        if self.is_error_type(ty) || self.is_dependent_type(ty) {
            return true;
        }
        let t = &self.ast.types;
        if matches!(t.canonical_view(ty), TypeView::RValueRef(_)) {
            let d = self.diag(DiagId::sema_nttp_rvalue_ref, loc).arg(self.ty_arg(ty));
            self.emit(d);
            return false;
        }
        let Some(r) = t.as_record(ty) else { return true };
        let Some(def) = self.ast.definition(r) else { return true };
        let bits = self.record_bits(def);
        if !bits.contains(DefinitionBits::COMPLETE) {
            return true;
        }
        if !bits.contains(DefinitionBits::LITERAL) {
            let mut d = self.diag(DiagId::sema_nttp_non_literal, loc).arg(self.ty_arg(ty));
            if let Some(n) = self.non_literal_note(ty) {
                d = d.with_note(n);
            }
            self.emit(d);
            return false;
        }
        if self.is_structural(ty) {
            return true;
        }
        let mut d = self.diag(DiagId::sema_nttp_not_structural, loc).arg(self.ty_arg(ty));
        for n in self.not_structural_notes(ty) {
            d = d.with_note(n);
        }
        self.emit(d);
        false
    }

    fn record_bits(&self, def: DeclId) -> DefinitionBits {
        self.ast.decls.record(def).map(|x| x.defn).unwrap_or_default()
    }

    /// Whether `ty` is a structural type ([temp.param]/7), as clang's
    /// `Type::isStructuralType`, recomputed for classes so a member of
    /// rvalue reference type counts (the `STRUCTURAL` bit misses it).
    fn is_structural(&self, ty: QualType) -> bool {
        self.is_structural_at(ty, 0)
    }

    fn is_structural_at(&self, ty: QualType, depth: usize) -> bool {
        let q = self.base_element(ty);
        let t = &self.ast.types;
        let Some(def) = t.as_record(q).and_then(|r| self.ast.definition(r)) else {
            return !matches!(t.canonical_view(q), TypeView::RValueRef(_));
        };
        let b = self.record_bits(def);
        if !b.contains(DefinitionBits::LITERAL) || !b.contains(DefinitionBits::STRUCTURAL) {
            return false;
        }
        if depth >= MAX_DEPTH {
            return true;
        }
        self.fields_in(def).into_iter().all(|f| {
            self.ast.decls.get(f).ty().is_none_or(|q| self.is_structural_at(q, depth + 1))
        }) && self
            .ast
            .decls
            .bases(def, &self.ast.lists)
            .all(|b| self.is_structural_at(b.ty, depth + 1))
    }

    fn fields_in(&self, def: DeclId) -> Vec<DeclId> {
        self.ast.members(def).filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field).collect()
    }

    /// clang's drill-down into why class type `ty` is not structural
    /// (`RequireStructuralType`): a local reason (a non-public or mutable
    /// member, an rvalue reference member, a non-public base) ends the
    /// walk, else the first non-structural subobject is named and the
    /// walk continues into its type.
    fn not_structural_notes(&self, ty: QualType) -> Vec<Diag> {
        let mut notes = Vec::new();
        let mut ty = ty;
        for _ in 0..MAX_DEPTH {
            let t = &self.ast.types;
            let Some(def) = t.as_record(ty).and_then(|r| self.ast.definition(r)) else { break };
            let shown = self.ty_arg(ty);
            let fields = self.fields_in(def);
            for &f in &fields {
                let n = *self.ast.decls.get(f);
                let note = if n.misc().access() != Access::Public {
                    Some(self.diag(DiagId::note_not_structural_non_public, n.loc()).arg(&shown).arg(0u8))
                } else if self.ast.decls.field(f).is_some_and(|x| x.is_mutable()) {
                    Some(self.diag(DiagId::note_not_structural_mutable_field, n.loc()).arg(&shown))
                } else if n.ty().is_some_and(|q| {
                    matches!(self.ast.types.canonical_view(q), TypeView::RValueRef(_))
                }) {
                    Some(self.diag(DiagId::note_not_structural_rvalue_ref_field, n.loc()).arg(&shown))
                } else {
                    None
                };
                if let Some(note) = note {
                    notes.push(note);
                    return notes;
                }
            }
            let bases: Vec<_> = self.ast.decls.bases(def, &self.ast.lists).copied().collect();
            for b in &bases {
                if self.base_access(def, b.access) != Access::Public {
                    notes.push(
                        self.diag(DiagId::note_not_structural_non_public, b.loc).arg(&shown).arg(1u8),
                    );
                    return notes;
                }
            }
            let sub = fields
                .iter()
                .find_map(|&f| {
                    let n = self.ast.decls.get(f);
                    let q = self.base_element(n.ty()?);
                    (!self.is_structural(q)).then_some((n.loc(), q, 0u8))
                })
                .or_else(|| {
                    bases.iter().find_map(|b| (!self.is_structural(b.ty)).then_some((b.loc, b.ty, 1u8)))
                });
            let Some((at, q, kind)) = sub else { break };
            notes.push(
                self.diag(DiagId::note_not_structural_subobject, at)
                    .arg(&shown)
                    .arg(kind)
                    .arg(self.ty_arg(q)),
            );
            ty = q;
        }
        notes
    }

    /// Why class type `ty` is not literal ([basic.types.general]/10;
    /// clang `RequireLiteralType`'s notes), as far as kiln tracks it.
    fn non_literal_note(&self, ty: QualType) -> Option<Diag> {
        let def = self.ast.types.as_record(ty).and_then(|r| self.ast.definition(r))?;
        let bits = self.record_bits(def);
        let shown = self.ty_arg(ty);
        if !bits.contains(DefinitionBits::AGGREGATE)
            && !bits.contains(DefinitionBits::HAS_CONSTEXPR_CTOR)
            && !bits.contains(DefinitionBits::LAMBDA)
        {
            let loc = self.ast.decls.get(def).loc();
            return Some(self.diag(DiagId::note_non_literal_no_constexpr_ctors, loc).arg(shown));
        }
        for b in self.ast.decls.bases(def, &self.ast.lists) {
            if !self.is_literal_type(b.ty) {
                return Some(
                    self.diag(DiagId::note_non_literal_base_class, b.loc)
                        .arg(&shown)
                        .arg(self.ty_arg(b.ty)),
                );
            }
        }
        for f in self.fields_in(def) {
            let n = self.ast.decls.get(f);
            let q = n.ty()?;
            if !self.is_literal_type(q) {
                return Some(
                    self.diag(DiagId::note_non_literal_field, n.loc())
                        .arg(&shown)
                        .arg(self.decl_arg(f))
                        .arg(self.ty_arg(q)),
                );
            }
        }
        let dtor = self.ast.members(def).find(|&m| self.ast.decls.get(m).kind() == DeclKind::CXXDestructor)?;
        let constexpr = self.ast.decls.function(dtor).is_some_and(|f| f.spec.constexpr_kind() != kiln_ast::decl::ConstexprKind::None);
        (!constexpr).then(|| {
            self.diag(DiagId::note_non_literal_non_constexpr_dtor, self.ast.decls.get(dtor).loc()).arg(shown)
        })
    }
}
