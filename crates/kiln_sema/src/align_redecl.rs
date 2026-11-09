//! Alignment specifiers across redeclarations and under-alignment
//! ([dcl.align]/5, /6; clang `mergeAlignedAttrs`,
//! `checkNewAttributesAfterDef` and `CheckAlignasUnderalignment`;
//! spec/sema.md "alignas").
//!
//! A variable or class declaration's *effective* alignment attributes are
//! the ones it inherits from its previous declaration
//! ([`Sema::inherited_aligned`], in front) followed by its own. They are
//! merged when the declaration is made: an `alignas` added after a
//! definition without one is an error (and dropped), two declarations
//! with `alignas` must request the same alignment, a definition must
//! repeat an earlier `alignas`; then the strictest earlier request and
//! the latest earlier `alignas` are inherited. A complete variable or
//! class definition whose `alignas` (with every other alignment
//! attribute) requests less than its type's alignment is an error.

use kiln_ast::attr::{AttrKind, AttrSyntax};
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

/// The merged view of a list of alignment attributes: `None` when one of
/// them is dependent (clang `isAlignmentDependent`: nothing is checked).
struct AlignSet {
    /// The strictest alignment requested, in bytes (0 if none).
    max: u64,
    /// The first attribute requesting `max`.
    strictest: Option<AttrId>,
    /// The last `alignas` (keyword) attribute.
    alignas: Option<AttrId>,
}

impl Sema<'_, '_> {
    /// The alignment one `ALIGNED` attribute requests, in bytes (`alignas(0)`
    /// requests 0, ignored by [dcl.align]/4); `None` if dependent.
    fn requested_align(&mut self, a: AttrId) -> Option<u64> {
        let node = *self.ast.attrs.get(a);
        if let Some(t) = QualType::from_raw(node.b) {
            if self.ast.types.is_dependent(t) {
                return None;
            }
            return Some(crate::target::size_align(&self.ast, t, &self.abi).map_or(0, |x| x.1));
        }
        let Some(e) = ExprId::from_raw(node.a) else {
            // `__attribute__((aligned))`: the target's biggest alignment.
            return Some(self.abi.biggest_align);
        };
        if self.ast.exprs.get(e).is_value_dependent() {
            return None;
        }
        Some(self.fold_int(e).and_then(|v| u64::try_from(v).ok()).unwrap_or(0))
    }

    fn align_set(&mut self, attrs: &[AttrId]) -> Option<AlignSet> {
        let mut set = AlignSet { max: 0, strictest: None, alignas: None };
        for &a in attrs {
            let node = *self.ast.attrs.get(a);
            if node.kind != AttrKind::ALIGNED {
                continue;
            }
            let v = self.requested_align(a)?;
            if node.syntax == AttrSyntax::Keyword {
                set.alignas = Some(a);
            }
            if v > set.max {
                set.max = v;
                set.strictest = Some(a);
            }
        }
        Some(set)
    }

    /// `d`'s own `ALIGNED` attributes (as attached).
    pub(crate) fn own_aligned(&self, d: DeclId) -> Vec<AttrId> {
        self.ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(d))
            .filter(|&a| self.ast.attrs.get(a).kind == AttrKind::ALIGNED)
            .collect()
    }

    /// `d`'s effective alignment attributes: inherited, then `own` (its
    /// own, attached or about to be).
    fn effective_aligned(&self, d: DeclId, own: &[AttrId]) -> Vec<AttrId> {
        let mut v = self.inherited_aligned.get(&d).cloned().unwrap_or_default();
        v.extend(own.iter().copied().filter(|&a| self.ast.attrs.get(a).kind == AttrKind::ALIGNED));
        v
    }

    /// The type whose alignment `d`'s alignment attributes are measured
    /// against: a variable's or member's type, a class's own type.
    fn aligned_subject_type(&self, d: DeclId) -> Option<QualType> {
        match self.ast.decls.get(d).kind() {
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization => Some(self.tag_type(d)),
            DeclKind::Enum => None,
            _ => self.ast.decls.get(d).ty(),
        }
    }

    fn type_align(&self, t: QualType) -> u64 {
        crate::target::size_align(&self.ast, t, &self.abi).map_or(0, |x| x.1)
    }

    /// Merges the alignment attributes of the new declaration `new` (its
    /// own, not yet attached, are `own`; an `alignas` among them made
    /// after a definition without one is reported and removed) with its
    /// previous declaration's ([dcl.align]/6). `is_def`: `new` is a
    /// definition. Records what `new` inherits.
    pub(crate) fn merge_aligned_redecl(
        &mut self,
        new: DeclId,
        own: &mut Vec<AttrId>,
        is_def: bool,
    ) {
        let Some(old) = self.ast.prev(new) else { return };
        // The earlier definition matters only to an `alignas` of `new`'s
        // own; finding a variable's walks every previous declaration, so
        // a run of plain redeclarations would be quadratic (WP-1076).
        let alignas = own.iter().any(|&a| {
            let n = self.ast.attrs.get(a);
            n.kind == AttrKind::ALIGNED && n.syntax == AttrSyntax::Keyword
        });
        let def = if alignas { self.definition_before(new) } else { None };
        self.merge_aligned(Some(new), new, old, def, own, is_def);
    }

    /// A class redeclaration `struct alignas(N) S;` that kiln folds into
    /// the existing declaration `d` (no declaration of its own): its
    /// alignment attributes `own` are checked against `d`'s (removed when
    /// added after a definition without one), inheriting nothing.
    pub(crate) fn merge_aligned_folded_redecl(&mut self, d: DeclId, own: &mut Vec<AttrId>) {
        let def = self.is_complete_tag_def(d).then_some(d);
        self.merge_aligned(None, d, d, def, own, false);
    }

    /// clang `mergeAlignedAttrs` after `checkNewAttributesAfterDef`: the
    /// new declaration `new` (`None`: folded into `old`) of `subject`'s
    /// entity, with own alignment attributes `own`, against the previous
    /// declaration `old` and the earlier definition `def`.
    fn merge_aligned(
        &mut self,
        new: Option<DeclId>,
        subject: DeclId,
        old: DeclId,
        def: Option<DeclId>,
        own: &mut Vec<AttrId>,
        is_def: bool,
    ) {
        // clang `checkNewAttributesAfterDef`: an `alignas` added after a
        // definition that has no alignment attribute.
        if let Some(def) = def
            && self.effective_aligned(def, &self.own_aligned(def)).is_empty()
        {
            let mut i = 0;
            while i < own.len() {
                let node = *self.ast.attrs.get(own[i]);
                if node.kind == AttrKind::ALIGNED && node.syntax == AttrSyntax::Keyword {
                    let at = self.ast.decls.get(def).loc();
                    let msg =
                        self.diag(DiagId::sema_alignas_missing_on_definition, at).arg("'alignas'");
                    let earlier =
                        self.diag(DiagId::note_alignas_on_declaration, node.loc).arg("'alignas'");
                    self.emit(msg.with_note(earlier));
                    own.remove(i);
                } else {
                    i += 1;
                }
            }
        }
        let old_attrs = self.effective_aligned(old, &self.own_aligned(old));
        if old_attrs.is_empty() {
            return;
        }
        let Some(o) = self.align_set(&old_attrs) else { return };
        let Some(n) = self.align_set(own) else { return };
        if let (Some(oa), Some(na)) = (o.alignas, n.alignas)
            && o.max != n.max
        {
            // `alignas(0)` alone requests the natural alignment.
            let natural = || self.aligned_subject_type(subject).map_or(0, |t| self.type_align(t));
            let om = if o.max == 0 { natural() } else { o.max };
            let nm = if n.max == 0 { natural() } else { n.max };
            if om != nm {
                let at = self.ast.attrs.get(na).loc;
                let msg = self
                    .diag(DiagId::sema_alignas_mismatch, at)
                    .arg(om.to_string())
                    .arg(nm.to_string());
                let note = self.diag(DiagId::note_previous_declaration, self.ast.attrs.get(oa).loc);
                self.emit(msg.with_note(note));
            }
        }
        let Some(new) = new else { return };
        if let Some(oa) = o.alignas
            && n.alignas.is_none()
            && is_def
        {
            let at = self.ast.decls.get(new).loc();
            let msg = self.diag(DiagId::sema_alignas_missing_on_definition, at).arg("'alignas'");
            let loc: Loc = self.ast.attrs.get(oa).loc;
            let note = self.diag(DiagId::note_alignas_on_declaration, loc).arg("'alignas'");
            self.emit(msg.with_note(note));
        }
        let mut inherited = Vec::new();
        if o.max > n.max
            && let Some(s) = o.strictest
        {
            inherited.push(s);
        }
        if let Some(oa) = o.alignas
            && n.alignas.is_none()
            && !(inherited
                .first()
                .is_some_and(|&s| self.ast.attrs.get(s).syntax == AttrSyntax::Keyword))
        {
            inherited.push(oa);
        }
        if !inherited.is_empty() {
            let _ = self.inherited_aligned.insert(new, inherited);
        }
    }

    /// The definition among the declarations before `new`, if any.
    fn definition_before(&self, new: DeclId) -> Option<DeclId> {
        let kind = self.ast.decls.get(new).kind();
        if kind == DeclKind::Var {
            let mut cur = self.ast.prev(new);
            let mut n = 0;
            while let Some(x) = cur {
                if self.is_var_definition(x) {
                    return Some(x);
                }
                n += 1;
                if n > 4096 {
                    break;
                }
                cur = self.ast.prev(x);
            }
            return None;
        }
        self.ast.definition(new).filter(|&d| d != new && self.is_complete_tag_def(d))
    }

    fn is_complete_tag_def(&self, d: DeclId) -> bool {
        self.ast
            .decls
            .record(d)
            .is_some_and(|r| r.defn.contains(kiln_ast::decl::DefinitionBits::COMPLETE))
    }

    /// Merges and checks the alignment attributes of variable or member
    /// `d` before `own` (its own) are attached.
    pub(crate) fn finish_decl_alignment(&mut self, d: DeclId, own: &mut Vec<AttrId>) {
        if !matches!(self.ast.decls.get(d).kind(), DeclKind::Var | DeclKind::Field) {
            return;
        }
        let is_def = self.ast.decls.get(d).kind() == DeclKind::Var && self.is_var_definition(d);
        self.merge_aligned_redecl(d, own, is_def);
        self.check_alignas_underalignment(d, own);
    }

    /// clang `CheckAlignasUnderalignment` ([dcl.align]/5): `d` (with its
    /// own alignment attributes `own`) has an `alignas` and, with every
    /// alignment attribute, requests less than its complete type's
    /// alignment. Reported at the last `alignas`.
    pub(crate) fn check_alignas_underalignment(&mut self, d: DeclId, own: &[AttrId]) {
        let attrs = self.effective_aligned(d, own);
        if attrs.is_empty() {
            return;
        }
        let Some(set) = self.align_set(&attrs) else { return };
        let Some(a) = set.alignas else { return };
        if set.max == 0 {
            return;
        }
        let Some(ty) = self.aligned_subject_type(d) else { return };
        if self.ast.types.is_dependent(ty) {
            return;
        }
        let Some((_, natural)) = crate::target::size_align(&self.ast, ty, &self.abi) else {
            return;
        };
        if natural > set.max {
            let msg = self
                .diag(DiagId::sema_alignas_underaligned, self.ast.attrs.get(a).loc)
                .arg(natural.to_string())
                .arg(self.ty_arg(ty));
            self.emit(msg);
        }
    }
}
