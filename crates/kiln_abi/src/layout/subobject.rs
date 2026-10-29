//! The base-subobject table of a most-derived class (spec/abi.md §8.3;
//! spec/sema-class.md §3.5): every base-class subobject with its byte
//! offset in the complete object, numbered by [`SubobjectId`] in
//! preorder — 0 is the class itself, then each direct non-virtual base's
//! subtree in declaration order.
//!
//! Virtual bases are not yet included (they follow the non-virtual
//! subtree in Itanium inheritance-graph order; WP-078b).

use kiln_ast::DeclId;
use kiln_ast::class::{Subobject, SubobjectId};

use super::{Layouter, props};

/// The most subobjects a table holds (`SubobjectId` is a `u16`). A
/// non-virtual hierarchy can have exponentially many base subobjects
/// (each level naming the previous class twice); the table stops there.
pub const MAX_SUBOBJECTS: usize = u16::MAX as usize + 1;

impl Layouter<'_> {
    /// The base-subobject table of `record` (any declaration): the
    /// [`Subobject`] for each [`SubobjectId`] with its byte offset from
    /// the start of the complete object. Truncated at
    /// [`MAX_SUBOBJECTS`] entries.
    pub fn subobjects(&mut self, record: DeclId) -> Vec<(Subobject, u64)> {
        let first = self.ast().first(record);
        self.record(first);
        let mut out: Vec<(Subobject, u64)> = Vec::new();
        // Explicit preorder: a subobject is numbered when popped, then
        // its direct non-virtual bases are pushed in reverse so they pop
        // in declaration order.
        let root = Subobject {
            class: first,
            parent: SubobjectId::COMPLETE,
            base_index: 0,
            is_virtual: false,
        };
        let mut stack = vec![(root, 0u64)];
        while let Some((sub, off)) = stack.pop() {
            if out.len() >= MAX_SUBOBJECTS {
                break;
            }
            let id = SubobjectId(u16::try_from(out.len()).unwrap_or(u16::MAX));
            out.push((sub, off));
            let ast = self.ast();
            let Some(def) = ast.definition(sub.class) else { continue };
            let Some(facts) = self.facts(sub.class) else { continue };
            let offsets = &facts.layout.bases;
            let first_child = stack.len();
            for b in props::bases(ast, def) {
                let Some(base) = b.class else { continue };
                if b.is_virtual {
                    continue;
                }
                let bo = offsets.iter().find(|(d, _)| *d == base).map_or(0, |&(_, o)| o);
                let child =
                    Subobject { class: base, parent: id, base_index: b.index, is_virtual: false };
                stack.push((child, off.saturating_add(bo)));
            }
            stack[first_child..].reverse();
        }
        out
    }
}

/// The base-subobject table of `record` on `target`
/// ([`Layouter::subobjects`]).
pub fn subobjects(
    ast: &kiln_ast::Ast,
    record: DeclId,
    target: &crate::target::Target,
) -> Vec<(Subobject, u64)> {
    Layouter::new(ast, target).subobjects(record)
}
