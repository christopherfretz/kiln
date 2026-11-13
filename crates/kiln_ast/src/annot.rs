//! Annotations (P3394R4 `[[=constant-expression]]`, [dcl.attr.annotation];
//! spec/annotations.md): the cold side table of every annotation written
//! on a declaration.
//!
//! An annotation is not an attribute node: sema takes it out of the
//! attribute-specifier-seq when the declaration it appertains to is known
//! and appends one [`Annotation`] here. Each declaration's annotations are
//! one immutable list, written once, in source order; an entity's
//! annotations are the lists of its redeclarations, first to latest
//! ([`AnnotArena::of_entity`]). Equal values are not merged: every
//! annotation, and every instantiation of one, is a distinct annotation
//! ([dcl.attr.annotation]/3).
#![warn(missing_docs)]

use kiln_source::Loc;

use crate::Ast;
pub use crate::ids::AnnId;
use crate::ids::{ConstValueId, DeclId, ExprId, ListId, QualType};
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::slot::ColdMap;

/// One annotation (20 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub struct Annotation {
    /// The declaration it was written on (not the first declaration of
    /// the entity; [`AnnotArena::of_entity`] gathers those).
    pub decl: DeclId,
    /// The underlying constant ([dcl.attr.annotation]/2): the frozen
    /// value of `std::meta::reflect_constant(e)`, of type
    /// [`Annotation::written_type`]. `None` while the operand is
    /// dependent (a templated declaration's annotation, substituted with
    /// it).
    pub value: Option<ConstValueId>,
    /// The `=` token.
    pub loc: Loc,
    /// The type `T` of the underlying constant: the operand's type with
    /// references and cv-qualifiers removed and arrays and functions
    /// decayed (the `T` `reflect_constant` deduces). The operand's type as
    /// written while it is dependent.
    pub written_type: QualType,
    /// The operand as written (after a pack expansion, the element).
    pub expr: ExprId,
}

/// All annotations and the declaration → annotations map.
#[derive(Debug, kiln_snap::Snap)]
pub struct AnnotArena {
    nodes: Vec<Annotation>,
    /// The annotations written on each declaration that has any.
    of_decl: ColdMap<DeclId, ListId>,
    /// The annotations of each annotated base-specifier, keyed by the
    /// first declaration of the derived class and the base's index in
    /// its definition's base list ([dcl.attr.annotation]/1).
    of_base: ColdMap<(DeclId, u32), ListId>,
}

impl Default for AnnotArena {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            of_decl: ColdMap::new("annot_decls"),
            of_base: ColdMap::new("annot_bases"),
        }
    }
}

impl AnnotArena {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an annotation (not yet attached to its declaration).
    pub fn push(&mut self, a: Annotation) -> AnnId {
        self.nodes.push(a);
        AnnId::new(self.nodes.len() as u32)
    }

    /// The annotation `a`.
    pub fn get(&self, a: AnnId) -> &Annotation {
        &self.nodes[a.index() as usize - 1]
    }

    /// The number of annotations.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True if there are none.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The annotations written on the declaration `d`, in source order
    /// ([`ListId::EMPTY`] if none).
    pub fn of_decl(&self, d: DeclId) -> ListId {
        self.of_decl.get(d).copied().unwrap_or(ListId::EMPTY)
    }

    /// Whether `d` has a list (possibly empty) already.
    pub fn has_list(&self, d: DeclId) -> bool {
        self.of_decl.contains(d)
    }

    /// Records `list` as the annotations of declaration `d`. A
    /// declaration's list is written once; a second call appends (a class
    /// whose trailing attributes come after its head's).
    pub fn attach(&mut self, d: DeclId, list: ListId, lists: &mut crate::lists::IdLists) {
        if let Some(&old) = self.of_decl.get(d) {
            let mut all: Vec<u32> = lists.get(old).to_vec();
            all.extend_from_slice(lists.get(list));
            let joined = lists.push_raw(&all);
            self.of_decl.refine(d, joined);
        } else {
            let _ = self.of_decl.insert(d, list);
        }
    }

    /// Empties the annotations of declaration `d`, leaving it an (empty)
    /// list so that none is copied onto it later: a function template
    /// specialization declared by an explicit specialization drops those
    /// instantiated from the pattern (spec/annotations.md §5).
    pub fn clear(&mut self, d: DeclId) {
        if !self.of_decl.refine(d, ListId::EMPTY) {
            let _ = self.of_decl.insert(d, ListId::EMPTY);
        }
    }

    /// The annotations of the `index`-th direct base-specifier of the
    /// class whose first declaration is `class`, in source order
    /// ([`ListId::EMPTY`] if none).
    pub fn of_base(&self, class: DeclId, index: u32) -> ListId {
        self.of_base.get((class, index)).copied().unwrap_or(ListId::EMPTY)
    }

    /// Records `list` as the annotations of the `index`-th base-specifier
    /// of the class whose first declaration is `class` (written once,
    /// when the class is defined).
    pub fn attach_base(&mut self, class: DeclId, index: u32, list: ListId) {
        let _ = self.of_base.insert((class, index), list);
    }

    /// The annotations of the entity `d` declares
    /// ([meta.reflection.annotation]): those of each of its
    /// declarations, first declaration to latest, each in source order.
    pub fn of_entity(&self, ast: &Ast, d: DeclId) -> Vec<AnnId> {
        if self.of_decl.is_empty() {
            return Vec::new();
        }
        let mut decls: Vec<DeclId> = ast.redecls_of(d).collect();
        decls.reverse();
        let mut out = Vec::new();
        for x in decls {
            if let Some(&l) = self.of_decl.get(x) {
                out.extend(ast.lists.iter::<AnnId>(l));
            }
        }
        out
    }

    pub(crate) fn live_bytes(&self) -> u64 {
        crate::mem::VecDims::of("nodes", &self.nodes).live_bytes()
            + self.of_decl.live_bytes()
            + self.of_base.live_bytes()
    }
}

impl HeapSize for AnnotArena {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("nodes", &self.nodes));
        r.push(self.of_decl.mem_row());
        r.push(self.of_base.mem_row());
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_is_20_bytes() {
        assert_eq!(size_of::<Annotation>(), 20);
    }
}
