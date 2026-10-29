//! The base-subobject tree of a class being laid out (clang
//! `BaseSubobjectInfo`, `ComputeBaseSubobjectInfo`): one node per
//! non-virtual base subobject and one per virtual base (shared by every
//! path that reaches it). A node whose class has a *virtual* primary base
//! may *claim* that virtual base (Itanium §2.4 II.1, "indirect primary
//! base"): the claimed virtual base is then placed at the claimant's
//! offset and its empty subobjects are checked there.
//!
//! The tree is built with an explicit stack. Like clang's, it has one
//! node per non-virtual path, so it is capped at [`MAX_NODES`]; past the
//! cap nodes are created without children.

use kiln_ast::DeclId;
use kiln_source::fx::FxHashMap;

use super::Layouter;
use super::props::{self, BaseDesc};

/// The most nodes a tree holds.
pub(super) const MAX_NODES: usize = 1 << 16;

/// One base subobject (clang `BaseSubobjectInfo`).
#[derive(Clone, Debug)]
pub(super) struct BaseInfo {
    /// The base class (first declaration).
    pub class: DeclId,
    /// A virtual base.
    pub is_virtual: bool,
    /// The node that claimed this virtual base as its primary base
    /// (clang `Derived`).
    pub derived: Option<u32>,
    /// The virtual primary base this node claimed (clang
    /// `PrimaryVirtualBaseInfo`).
    pub primary_vbase: Option<u32>,
    /// Direct bases in declaration order (virtual ones are shared nodes).
    pub bases: Vec<u32>,
}

/// The tree of one class.
#[derive(Debug, Default)]
pub(super) struct BaseInfos {
    pub nodes: Vec<BaseInfo>,
    /// clang `VirtualBaseInfo`.
    pub virtuals: FxHashMap<DeclId, u32>,
    /// clang `NonVirtualBaseInfo`: the direct non-virtual bases.
    pub non_virtuals: FxHashMap<DeclId, u32>,
}

impl BaseInfos {
    pub fn node(&self, i: u32) -> &BaseInfo {
        &self.nodes[i as usize]
    }

    pub fn node_mut(&mut self, i: u32) -> &mut BaseInfo {
        &mut self.nodes[i as usize]
    }
}

enum Step {
    Enter {
        class: DeclId,
        is_virtual: bool,
        parent: Option<u32>,
    },
    /// Claim the primary virtual base `class` after the children were
    /// computed (it was first reached through them).
    Exit {
        node: u32,
        pending: Option<DeclId>,
    },
}

/// The direct class bases of `class` (any declaration).
pub(super) fn class_bases(lay: &Layouter<'_>, class: DeclId) -> Vec<BaseDesc> {
    let ast = lay.ast();
    ast.definition(class)
        .map(|def| props::bases(ast, def).into_iter().filter(|b| b.class.is_some()).collect())
        .unwrap_or_default()
}

/// clang `ComputeBaseSubobjectInfo(RD)` over the direct bases `bases` of
/// the class being laid out (their layouts are in `lay`).
pub(super) fn compute(lay: &Layouter<'_>, bases: &[BaseDesc]) -> BaseInfos {
    let mut t = BaseInfos::default();
    let mut stack: Vec<Step> = bases
        .iter()
        .rev()
        .filter_map(|b| {
            Some(Step::Enter { class: b.class?, is_virtual: b.is_virtual, parent: None })
        })
        .collect();
    while let Some(step) = stack.pop() {
        match step {
            Step::Exit { node, pending } => {
                if let Some(p) = pending.and_then(|p| t.virtuals.get(&p).copied()) {
                    t.node_mut(node).primary_vbase = Some(p);
                    t.node_mut(p).derived = Some(node);
                }
            }
            Step::Enter { class, is_virtual, parent } => {
                if is_virtual && let Some(&existing) = t.virtuals.get(&class) {
                    if let Some(p) = parent {
                        t.node_mut(p).bases.push(existing);
                    }
                    continue;
                }
                let idx = u32::try_from(t.nodes.len()).unwrap_or(u32::MAX);
                t.nodes.push(BaseInfo {
                    class,
                    is_virtual,
                    derived: None,
                    primary_vbase: None,
                    bases: Vec::new(),
                });
                if is_virtual {
                    t.virtuals.insert(class, idx);
                }
                match parent {
                    Some(p) => t.node_mut(p).bases.push(idx),
                    None if !is_virtual => {
                        t.non_virtuals.insert(class, idx);
                    }
                    None => {}
                }
                // A primary virtual base of this base: claim it now if it
                // already has a node no one claimed, later if it has none.
                let mut pending = None;
                if let Some(f) = lay.facts(class)
                    && !f.layout.vbases.is_empty()
                    && let Some((pb, true)) = f.layout.primary_base
                {
                    match t.virtuals.get(&pb).copied() {
                        Some(p) if t.node(p).derived.is_none() => {
                            t.node_mut(idx).primary_vbase = Some(p);
                            t.node_mut(p).derived = Some(idx);
                        }
                        Some(_) => {}
                        None => pending = Some(pb),
                    }
                }
                stack.push(Step::Exit { node: idx, pending });
                if t.nodes.len() >= MAX_NODES {
                    continue;
                }
                let first_child = stack.len();
                for b in class_bases(lay, class) {
                    if let Some(c) = b.class {
                        stack.push(Step::Enter {
                            class: c,
                            is_virtual: b.is_virtual,
                            parent: Some(idx),
                        });
                    }
                }
                stack[first_child..].reverse();
            }
        }
    }
    t
}
