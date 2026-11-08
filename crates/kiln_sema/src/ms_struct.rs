//! GNU `ms_struct` / `gcc_struct` and `#pragma ms_struct` (clang
//! `MSStructAttr`, clang 22 `GCCStructAttr`; GCC "x86 Type Attributes";
//! WP-809).
//!
//! An `ms_struct` record lays out its bit-fields by the Microsoft rules
//! within the Itanium layout (clang `ItaniumRecordLayoutBuilder` with
//! `IsMsStruct`; `kiln_abi::layout`), and its IR uses one storage unit per
//! run of same-sized bit-fields (clang `isDiscreteBitFieldABI`). The
//! attribute is inheritable: a forward declaration's applies to the
//! definition. `#pragma ms_struct on` gives every class definition begun
//! while it is on an implicit one (clang `AddMsStructLayoutForRecord`).
//! clang 22's `gcc_struct` on any declaration asks for the default layout,
//! overriding both; the two written on one declaration conflict, and the
//! later is dropped.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::DefinitionBits;
use kiln_ast::{AttrId, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// `#pragma ms_struct on|off|reset`.
    pub fn act_on_pragma_ms_struct(&mut self, on: bool) {
        self.ms_struct_pragma = on;
    }

    /// `{` of class `r`'s definition: the implicit `ms_struct` of an
    /// active `#pragma ms_struct on`.
    pub(crate) fn ms_struct_from_pragma(&mut self, r: DeclId) {
        if !self.ms_struct_pragma {
            return;
        }
        let loc = self.ast.decls.get(r).loc();
        let node = AttrNode {
            kind: AttrKind::MS_STRUCT,
            syntax: AttrSyntax::Gnu,
            flags: AttrNode::IMPLICIT,
            loc,
            a: 0,
            b: 0,
        };
        let a = self.ast.attrs.push(node);
        self.builder().append_attrs(r, &[a]);
    }

    /// The `ms_struct` / `gcc_struct` attributes among `attrs` of class
    /// `d` (a wrong subject is diagnosed by `check_decl_attr`): one added
    /// after the definition is ignored (clang `checkNewAttributesAfterDef`),
    /// and one written after its opposite on the same declaration
    /// conflicts with it (clang 22 `MutualExclusions`).
    pub(crate) fn check_ms_struct_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        let ours = |k: AttrKind| k == AttrKind::MS_STRUCT || k == AttrKind::GCC_STRUCT;
        if !attrs.iter().any(|&a| ours(self.ast.attrs.get(a).kind))
            || self.ast.decls.record(d).is_none()
        {
            return;
        }
        // Those already on `d` (its class-head ones, for trailing
        // attributes), then those kept from `attrs`.
        let mut seen: Vec<AttrId> = self
            .ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(d))
            .filter(|&a| {
                let n = self.ast.attrs.get(a);
                ours(n.kind) && n.flags & AttrNode::IMPLICIT == 0
            })
            .collect();
        let def = self.ast.definition(d).filter(|&x| {
            self.ast.decls.record(x).is_some_and(|r| r.defn.contains(DefinitionBits::COMPLETE))
        });
        attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            if !ours(node.kind) {
                return true;
            }
            if let Some(def) = def {
                let has = self
                    .ast
                    .lists
                    .iter::<AttrId>(self.ast.attrs.of_decl(def))
                    .any(|x| self.ast.attrs.get(x).kind == node.kind);
                if !has {
                    let prev_def =
                        self.diag(DiagId::note_previous_definition, self.ast.decls.get(def).loc());
                    let msg = self
                        .diag(DiagId::sema_attribute_precede_definition, node.loc)
                        .with_note(prev_def);
                    self.emit(msg);
                }
                return false;
            }
            if let Some(&other) = seen.iter().find(|&&x| self.ast.attrs.get(x).kind != node.kind) {
                let (this, that) = if node.kind == AttrKind::MS_STRUCT {
                    ("ms_struct", "gcc_struct")
                } else {
                    ("gcc_struct", "ms_struct")
                };
                let here =
                    self.diag(DiagId::note_conflicting_attribute, self.ast.attrs.get(other).loc);
                let msg = self
                    .diag(DiagId::sema_attributes_not_compatible, node.loc)
                    .arg(this)
                    .arg(that)
                    .with_note(here);
                self.emit(msg);
                return false;
            }
            seen.push(a);
            true
        });
    }

    /// At the closing brace of class `r` (with definition bits `bits`):
    /// clang `CheckCompletedCXXClass`'s `warn_cxx_ms_struct`, an error by
    /// default, for an `ms_struct` class with bases or virtual functions.
    pub(crate) fn check_ms_struct_class(&mut self, r: DeclId, bits: DefinitionBits) {
        if !self.ast.is_ms_struct(r) {
            return;
        }
        let bases = self.ast.decls.record(r).is_some_and(|x| !self.ast.lists.is_empty(x.bases));
        if bases || bits.contains(DefinitionBits::POLYMORPHIC) {
            // Reported for the pattern and again for each instantiation.
            let msg = self.diag(DiagId::sema_cxx_ms_struct, self.ast.decls.get(r).loc());
            self.emit(msg.per_instantiation());
        }
    }

    /// clang `VerifyBitField`: in a class known to be `ms_struct` when
    /// the bit-field is declared, a width `v` above the storage size of
    /// its type `ty` is an error (reported at `loc`; the width is dropped).
    pub(crate) fn ms_bitfield_too_wide(
        &mut self,
        ty: QualType,
        v: i128,
        name: Option<&str>,
        loc: Loc,
    ) -> bool {
        let Some(r) = self.classes.last().map(|c| c.record) else { return false };
        if !self.ast.is_ms_struct(r) {
            return false;
        }
        let Some((bytes, _)) = crate::target::size_align(&self.ast, ty, self.abi()) else {
            return false;
        };
        let bits = bytes * 8;
        if v <= i128::from(bits) {
            return false;
        }
        let msg = match name {
            Some(n) => self
                .diag(DiagId::sema_bitfield_width_exceeds_size, loc)
                .arg(n)
                .arg(v.to_string())
                .arg(bits.to_string()),
            None => self
                .diag(DiagId::sema_anon_bitfield_width_exceeds_size, loc)
                .arg(v.to_string())
                .arg(bits.to_string()),
        };
        self.emit(msg);
        true
    }
}
