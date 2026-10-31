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

use kiln_ast::DeclId;
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::ids::RawId;
use kiln_ast::types::TemplateArg;

use crate::Sema;
use crate::templ::subst::Substituter;

/// Deepest concept expansion normalized (the rest are atoms).
const MAX_EXPANSION: u32 = 64;

impl Sema<'_, '_> {
    /// The normal form of the constraints `exprs` of entity `key`
    /// (a template or partial specialization), cached (§8.2); `None`
    /// when unconstrained.
    pub(crate) fn normal_form(&mut self, key: DeclId, exprs: &[ExprId]) -> Option<NormId> {
        if let Some(&n) = self.inst.concepts.normal_forms.get(&key) {
            return n;
        }
        let saved = std::mem::take(&mut self.ast.templates.dependent_depth);
        let mut acc: Option<NormId> = None;
        for &e in exprs {
            let n = self.normalize(e, None, 0);
            acc = Some(match acc {
                None => n,
                Some(a) => self.inst.concepts.norm.intern(NormNode::And(a, n)),
            });
        }
        self.ast.templates.dependent_depth = saved;
        self.inst.concepts.normal_forms.insert(key, acc);
        acc
    }

    /// Normalizes `e` ([temp.constr.normal]/1); `ctx` is the concept
    /// (and its parameter mapping) `e` appears in.
    fn normalize(&mut self, e: ExprId, ctx: Option<(DeclId, ArgListId)>, depth: u32) -> NormId {
        let mapping = ctx.map_or(ArgListId::EMPTY, |(_, m)| m);
        let atom = NormNode::Atomic { expr: e, mapping };
        if depth > MAX_EXPANSION {
            return self.inst.concepts.norm.intern(atom);
        }
        let n = *self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr => {
                let o = self.ast.exprs.operand(e);
                return self.normalize(o, ctx, depth + 1);
            }
            ExprKind::BinaryOperator
                if matches!(self.ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
            {
                let [l, r] = self.ast.exprs.operands(e);
                let a = self.normalize(l, ctx, depth + 1);
                let b = self.normalize(r, ctx, depth + 1);
                let node = if self.ast.exprs.binary_op(e) == BinaryOp::LAnd {
                    NormNode::And(a, b)
                } else {
                    NormNode::Or(a, b)
                };
                return self.inst.concepts.norm.intern(node);
            }
            ExprKind::ConceptSpecializationExpr => {
                if let Some(x) = self.normalize_concept_id(e, ctx, depth) {
                    return x;
                }
            }
            _ => {}
        }
        self.inst.concepts.norm.intern(atom)
    }

    /// [temp.constr.normal]/1.4: the normal form of concept-id `e` is the
    /// normal form of its concept's constraint with the arguments as the
    /// parameter mapping.
    fn normalize_concept_id(
        &mut self,
        e: ExprId,
        ctx: Option<(DeclId, ArgListId)>,
        depth: u32,
    ) -> Option<NormId> {
        let n = *self.ast.exprs.get(e);
        let c = self.ast.first(DeclId::from_raw(n.a())?);
        let l = ArgListId::from_raw(n.b())?;
        let args = match ctx {
            None => l,
            Some((cc, m)) => {
                let d = self.template_param_depth(cc);
                let prior: Vec<TemplateArg> = self.ast.types.args(m).to_vec();
                let levels = self.prior_levels(cc, d, &prior);
                let mut s = Substituter::new(self, &levels);
                s.args(l, n.loc()).ok()?
            }
        };
        let canonical = self.ast.types.canonical_args(args);
        if self.inst.concepts.checking.contains(&(c, canonical)) {
            return None;
        }
        let ce = self.ast.template_info(c).and_then(|i| i.constraint)?;
        self.inst.concepts.checking.push((c, canonical));
        let r = self.normalize(ce, Some((c, canonical)), depth + 1);
        self.inst.concepts.checking.pop();
        Some(r)
    }
}
