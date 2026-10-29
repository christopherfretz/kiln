//! The base-subobject table of a most-derived class (spec/abi.md §8.3;
//! spec/sema-class.md §3.5): every base-class subobject with its byte
//! offset in the complete object, numbered by [`SubobjectId`] in
//! preorder — 0 is the class itself, then each direct non-virtual base's
//! subtree in declaration order, then each virtual base once, in
//! inheritance-graph order (a depth-first, left-to-right walk of the
//! base graph numbering a virtual base at its first encounter), each
//! followed by its own non-virtual subtree. A virtual base's `parent` is
//! the first subobject whose class names it as a direct virtual base.
//! The numbering is sema's (`kiln_sema::class::virtual::subobject_graph`).

use kiln_ast::DeclId;
use kiln_ast::class::{Subobject, SubobjectId};
use kiln_source::fx::FxHashSet;

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
        let root = Subobject {
            class: first,
            parent: SubobjectId::COMPLETE,
            base_index: 0,
            is_virtual: false,
        };
        self.number_subtree(root, 0, &mut out);
        let vbases: Vec<(DeclId, u64)> =
            self.facts(first).map(|f| f.layout.vbases.to_vec()).unwrap_or_default();
        for v in self.vbase_order(first) {
            if out.len() >= MAX_SUBOBJECTS {
                break;
            }
            let (parent, base_index) = out
                .iter()
                .enumerate()
                .find_map(|(i, (s, _))| {
                    let def = self.ast().definition(s.class)?;
                    props::bases(self.ast(), def)
                        .into_iter()
                        .find(|b| b.is_virtual && b.class == Some(v))
                        .map(|b| (SubobjectId(u16::try_from(i).unwrap_or(u16::MAX)), b.index))
                })
                .unwrap_or((SubobjectId::COMPLETE, 0));
            let off = vbases.iter().find(|(d, _)| *d == v).map_or(0, |&(_, o)| o);
            let top = Subobject { class: v, parent, base_index, is_virtual: true };
            self.number_subtree(top, off, &mut out);
        }
        out
    }

    /// The virtual bases of `first` in inheritance-graph order: a
    /// depth-first walk over base-specifiers in declaration order, each
    /// class expanded once, collecting each virtual base at its first
    /// encounter.
    pub(crate) fn vbase_order(&self, first: DeclId) -> Vec<DeclId> {
        let ast = self.ast();
        let mut out: Vec<DeclId> = Vec::new();
        let mut expanded: FxHashSet<DeclId> = FxHashSet::default();
        expanded.insert(first);
        let direct =
            |c: DeclId| ast.definition(c).map(|def| props::bases(ast, def)).unwrap_or_default();
        let mut stack: Vec<props::BaseDesc> = direct(first).into_iter().rev().collect();
        while let Some(b) = stack.pop() {
            let Some(c) = b.class else { continue };
            if b.is_virtual && !out.contains(&c) {
                out.push(c);
            }
            if expanded.insert(c) {
                stack.extend(direct(c).into_iter().rev());
            }
        }
        out
    }

    /// Numbers `top` (at byte offset `off`) and its non-virtual base
    /// subtree in preorder: a subobject is numbered when popped, then its
    /// direct non-virtual bases are pushed in reverse so they pop in
    /// declaration order.
    fn number_subtree(&self, top: Subobject, off: u64, out: &mut Vec<(Subobject, u64)>) {
        let mut stack = vec![(top, off)];
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
