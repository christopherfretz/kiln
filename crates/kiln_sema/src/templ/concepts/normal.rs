//! Constraint normalization (spec/sema-templates.md §8.2,
//! [temp.constr.normal]): normal forms are hash-consed in a
//! [`NormTable`], atomic constraints identified by `(expression,
//! parameter mapping)`.

use kiln_ast::ExprId;
use kiln_ast::ids::ArgListId;
use kiln_source::fx::FxHashMap;

/// A hash-consed normal-form node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NormId(u32);

/// A normal-form node ([temp.constr.normal]/1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NormNode {
    /// An atomic constraint: its expression (as written, in the concept
    /// or template that declares it) and its parameter mapping (the
    /// canonical arguments, in terms of the outermost template's
    /// parameters).
    Atomic {
        /// The expression.
        expr: ExprId,
        /// The parameter mapping.
        mapping: ArgListId,
    },
    /// A conjunction.
    And(NormId, NormId),
    /// A disjunction.
    Or(NormId, NormId),
}

/// The interned normal forms: equal nodes share an id, so atomic
/// constraint identity is an id comparison (§8.5).
#[derive(Debug, Default)]
pub struct NormTable {
    nodes: Vec<NormNode>,
    map: FxHashMap<NormNode, NormId>,
}

impl NormTable {
    /// The id of `n`, interning it.
    pub fn intern(&mut self, n: NormNode) -> NormId {
        if let Some(&id) = self.map.get(&n) {
            return id;
        }
        let id = NormId(u32::try_from(self.nodes.len()).unwrap_or(u32::MAX));
        self.nodes.push(n);
        self.map.insert(n, id);
        id
    }

    /// Node `id`.
    pub fn get(&self, id: NormId) -> NormNode {
        self.nodes[id.0 as usize]
    }

    /// The number of interned nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// No node is interned.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
