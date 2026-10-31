//! Subsumption (spec/sema-templates.md §8.5, [temp.constr.order]).

use super::{NormId, NormNode};
use crate::Sema;
use kiln_ast::{DeclId, ExprId};

/// Most clauses a DNF/CNF conversion may produce (§8.5); beyond it the
/// check answers "does not subsume".
const MAX_CLAUSES: usize = 65_536;

type Clauses = Vec<Vec<NormId>>;

impl Sema<'_, '_> {
    /// Whether normal form `p` subsumes `q` ([temp.constr.order]/1): every
    /// disjunctive clause of `p`'s DNF shares an atom with every
    /// conjunctive clause of `q`'s CNF. Cached.
    pub(crate) fn subsumes(&mut self, p: NormId, q: NormId) -> bool {
        if p == q {
            return true;
        }
        if let Some(&r) = self.inst.concepts.subsume_cache.get(&(p, q)) {
            return r;
        }
        self.inst.concepts.counters.subsume += 1;
        let r = match (self.clauses(p, true), self.clauses(q, false)) {
            (Some(dnf), Some(cnf)) => {
                dnf.iter().all(|d| cnf.iter().all(|c| d.iter().any(|a| c.contains(a))))
            }
            _ => false,
        };
        self.inst.concepts.subsume_cache.insert((p, q), r);
        r
    }

    /// The clauses of `n`: its DNF (`dnf`, each clause a conjunction) or
    /// CNF (each a disjunction); `None` past [`MAX_CLAUSES`].
    fn clauses(&self, n: NormId, dnf: bool) -> Option<Clauses> {
        match self.inst.concepts.norm.get(n) {
            NormNode::Atomic { .. } => Some(vec![vec![n]]),
            NormNode::And(a, b) | NormNode::Or(a, b) => {
                let join = matches!(self.inst.concepts.norm.get(n), NormNode::Or(..)) == dnf;
                let (x, y) = (self.clauses(a, dnf)?, self.clauses(b, dnf)?);
                if join {
                    if x.len() + y.len() > MAX_CLAUSES {
                        return None;
                    }
                    let mut x = x;
                    x.extend(y);
                    Some(x)
                } else {
                    if x.len().saturating_mul(y.len()) > MAX_CLAUSES {
                        return None;
                    }
                    let mut out = Vec::with_capacity(x.len() * y.len());
                    for cx in &x {
                        for cy in &y {
                            let mut c = cx.clone();
                            for &a in cy {
                                if !c.contains(&a) {
                                    c.push(a);
                                }
                            }
                            out.push(c);
                        }
                    }
                    Some(out)
                }
            }
        }
    }

    /// Whether the entity with constraints `ca` (key `a`) is more
    /// constrained than the one with `cb` (key `b`)
    /// ([temp.constr.order]/3): its normal form subsumes the other's and
    /// not conversely. No constraints is the weakest.
    pub(crate) fn more_constrained(
        &mut self,
        a: DeclId,
        ca: &[ExprId],
        b: DeclId,
        cb: &[ExprId],
    ) -> bool {
        let p = self.normal_form(a, ca);
        let q = self.normal_form(b, cb);
        match (p, q) {
            (Some(_), None) => true,
            (None, _) => false,
            (Some(p), Some(q)) => self.subsumes(p, q) && !self.subsumes(q, p),
        }
    }

    /// [temp.func.order]/6.2.2: of two function templates equally
    /// specialized, the more constrained.
    pub(crate) fn template_more_constrained(&mut self, a: DeclId, b: DeclId) -> bool {
        let ca = self.associated_constraints(a);
        let cb = self.associated_constraints(b);
        if ca.is_empty() {
            return false;
        }
        self.more_constrained(a, &ca, b, &cb)
    }
}
