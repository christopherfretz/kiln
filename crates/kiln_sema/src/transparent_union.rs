//! GNU `transparent_union` in C++ (clang `TransparentUnionAttr`; GCC
//! "Common Type Attributes"; WP-809).
//!
//! The attribute is C-only (`LangOpts = [COnly]`), so clang ignores every
//! occurrence in C++ with `'transparent_union' attribute ignored`. But the
//! occurrences after the class-key of a tag also reach clang's delayed
//! handler (`ProcessDeclAttributeDelayed` -> `handleTransparentUnionAttr`),
//! which does not check the language: it diagnoses a non-union subject
//! (including a class template), a non-definition, and a union whose
//! fields do not qualify, and otherwise keeps the attribute. A kept
//! attribute makes no argument conversions in C++ (overload resolution
//! never looks at it), but the calling convention passes such a union by
//! value as its first field (clang `useFirstFieldIfTransparentUnion`,
//! `kiln_abi::call`).

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclKind, TagKind};
use kiln_ast::{AttrId, DeclId};
use kiln_diag::DiagId;

use crate::Sema;
use crate::decl::TagUse;

impl Sema<'_, '_> {
    /// The `transparent_union` attributes among the class-head
    /// attributes `attrs` of tag `d`: each is ignored with a warning, then
    /// put through clang's delayed subject check. All are removed from
    /// `attrs`; those on a union definition are returned, to be attached
    /// for [`Sema::check_transparent_union`] at the closing brace.
    pub(crate) fn transparent_union_head(
        &mut self,
        d: DeclId,
        attrs: &mut Vec<AttrId>,
        use_: TagUse,
    ) -> Vec<AttrId> {
        let mut found = Vec::new();
        let mut kept = Vec::new();
        attrs.retain(|&a| {
            let keep = self.ast.attrs.get(a).kind != AttrKind::TRANSPARENT_UNION;
            if !keep {
                found.push(a);
            }
            keep
        });
        // clang's delayed handler only sees class-heads, not enum-heads.
        let enum_ = self.ast.decls.get(d).kind() == DeclKind::Enum;
        let union = self.ast.decls.get(d).kind() == DeclKind::CXXRecord
            && self.ast.decls.record(d).is_some_and(|r| r.tag == TagKind::Union)
            && self.ast.described_template(d).is_none();
        for a in found {
            let node = *self.ast.attrs.get(a);
            let shown = self.attr_shown(&node, "transparent_union");
            let msg = self.diag(DiagId::parse_attribute_ignored, node.loc).arg(&shown);
            self.emit(msg);
            if enum_ {
                continue;
            }
            if !union {
                let msg = self
                    .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                    .arg(format!("'{shown}'"))
                    .arg("unions");
                self.emit(msg);
            } else if use_ != TagUse::Definition {
                let msg = self.diag(DiagId::sema_transparent_union_not_definition, node.loc);
                self.emit(msg);
            } else {
                kept.push(a);
            }
        }
        kept
    }

    /// At the closing brace of union `r`: clang `handleTransparentUnionAttr`'s
    /// field checks for a kept class-head `transparent_union`, which is
    /// dropped (with a warning) when they fail. A field of incomplete or
    /// dependent type drops it silently, as in clang.
    pub(crate) fn check_transparent_union(&mut self, r: DeclId) {
        let Some(a) = self
            .ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(r))
            .find(|&a| self.ast.attrs.get(a).kind == AttrKind::TRANSPARENT_UNION)
        else {
            return;
        };
        if !self.transparent_union_fields_ok(r, a) {
            self.builder().remove_attr(r, a);
        }
    }

    fn transparent_union_fields_ok(&mut self, r: DeclId, a: AttrId) -> bool {
        let fields: Vec<DeclId> = self
            .ast
            .decls
            .members(r)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        let Some(&first) = fields.first() else {
            let msg =
                self.diag(DiagId::sema_transparent_union_zero_fields, self.ast.attrs.get(a).loc);
            self.emit(msg);
            return false;
        };
        let Some(first_ty) = self.ast.decls.get(first).ty() else { return false };
        let vector = self.is_vector_type(first_ty);
        let floating = self.ast.types.is_floating(first_ty)
            || matches!(self.ast.types.canonical_view(first_ty),
                kiln_ast::types::TypeView::Complex(e) if self.ast.types.is_floating(e));
        if floating || vector {
            let ty = self.ty_arg(first_ty);
            let msg = self
                .diag(DiagId::sema_transparent_union_floating, self.ast.decls.get(first).loc())
                .arg(u32::from(vector))
                .arg(ty);
            self.emit(msg);
            return false;
        }
        let Some((size, align)) = crate::target::size_align(&self.ast, first_ty, self.abi()) else {
            return false;
        };
        for &f in &fields {
            let Some(ty) = self.ast.decls.get(f).ty() else { return false };
            let Some((fs, fa)) = crate::target::size_align(&self.ast, ty, self.abi()) else {
                return false;
            };
            if fs != size || fa > align {
                let is_size = fs != size;
                let (bits, first_bits) = if is_size { (fs, size) } else { (fa, align) };
                let note = self
                    .diag(
                        DiagId::note_transparent_union_first_field,
                        self.ast.decls.get(first).loc(),
                    )
                    .arg(u32::from(is_size))
                    .arg(first_bits * 8);
                let name = self.decl_name(f);
                let msg = self
                    .diag(
                        DiagId::sema_transparent_union_field_mismatch,
                        self.ast.decls.get(f).loc(),
                    )
                    .arg(u32::from(is_size))
                    .arg(name)
                    .arg(bits * 8)
                    .with_note(note);
                self.emit(msg);
                return false;
            }
        }
        true
    }
}
