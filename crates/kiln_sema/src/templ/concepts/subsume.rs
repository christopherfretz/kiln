//! Subsumption (spec/sema-templates.md §8.5, [temp.constr.order]).

use super::{NormId, NormNode};
use crate::Sema;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::{Diag, DiagId};

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

    /// The associated constraints of an overload candidate: its
    /// template's, or a non-template function's trailing requires-clause.
    fn candidate_constraints(&self, f: DeclId, tmpl: Option<DeclId>) -> Vec<ExprId> {
        match tmpl {
            Some(t) => self.associated_constraints(t),
            None => self.ast.templates.trailing_requires.get(f).copied().into_iter().collect(),
        }
    }

    /// clang's `MaybeDiagnoseAmbiguousConstraints` after an ambiguous
    /// call: when exactly two candidates are constrained and treating
    /// textually identical atoms ([temp.over.link]/5 equivalence, same
    /// parameter mapping) as identical would change either subsumption
    /// direction, the notes naming the last such pair
    /// (`note_ambiguous_atomic_constraints` and its "similar constraint
    /// expression here"). `cands` are (function, template) in lookup
    /// order.
    pub(crate) fn ambiguous_constraint_notes(
        &mut self,
        cands: &[(DeclId, Option<DeclId>)],
    ) -> Vec<Diag> {
        let mut constrained = Vec::new();
        for &(f, t) in cands {
            let ac = self.candidate_constraints(f, t);
            if ac.is_empty() {
                continue;
            }
            if constrained.len() == 2 {
                return Vec::new();
            }
            constrained.push((t.unwrap_or(f), ac));
        }
        if constrained.len() != 2 {
            return Vec::new();
        }
        // clang's lookup lists the later declaration first.
        constrained.sort_by_key(|(d, _)| std::cmp::Reverse(*d));
        let (d1, ac1) = constrained.swap_remove(0);
        let (d2, ac2) = constrained.swap_remove(0);
        let (Some(n1), Some(n2)) = (self.normal_form(d1, &ac1), self.normal_form(d2, &ac2)) else {
            return Vec::new();
        };
        let (Some(dnf1), Some(cnf1), Some(dnf2), Some(cnf2)) = (
            self.clauses(n1, true),
            self.clauses(n1, false),
            self.clauses(n2, true),
            self.clauses(n2, false),
        ) else {
            return Vec::new();
        };
        let normal12 = self.clauses_subsume(&dnf1, &cnf2, &mut |_, a, b| a == b);
        let normal21 = self.clauses_subsume(&dnf2, &cnf1, &mut |_, a, b| a == b);
        let mut pair: Option<(ExprId, ExprId)> = None;
        let mut identical = |s: &Self, a: NormId, b: NormId| -> bool {
            if a == b {
                return true;
            }
            let (
                NormNode::Atomic { expr: ea, mapping: ma },
                NormNode::Atomic { expr: eb, mapping: mb },
            ) = (s.inst.concepts.norm.get(a), s.inst.concepts.norm.get(b))
            else {
                return false;
            };
            if !s.mappings_match(ma, mb) {
                return false;
            }
            if ea == eb {
                return true;
            }
            if !s.exprs_equivalent(ea, eb, 0) {
                return false;
            }
            pair = Some((ea, eb));
            true
        };
        let ident12 = self.clauses_subsume(&dnf1, &cnf2, &mut identical);
        let ident21 = self.clauses_subsume(&dnf2, &cnf1, &mut identical);
        if ident12 == normal12 && ident21 == normal21 {
            return Vec::new();
        }
        let Some((a, b)) = pair else { return Vec::new() };
        vec![
            self.diag(DiagId::note_ambiguous_atomic_constraints, self.begin_loc(a)),
            self.diag(
                DiagId::note_ambiguous_atomic_constraints_similar_expression,
                self.begin_loc(b),
            ),
        ]
    }

    /// clang's `subsumes(PDNF, QCNF, E)` over precomputed clauses, with
    /// atom evaluator `eq`.
    fn clauses_subsume(
        &self,
        p: &Clauses,
        q: &Clauses,
        eq: &mut dyn FnMut(&Self, NormId, NormId) -> bool,
    ) -> bool {
        p.iter().all(|pi| q.iter().all(|qj| pi.iter().any(|&a| qj.iter().any(|&b| eq(self, a, b)))))
    }

    /// clang's `AtomicConstraint::hasMatchingParameterMapping`: equal
    /// canonical arguments.
    fn mappings_match(&self, a: kiln_ast::ids::ArgListId, b: kiln_ast::ids::ArgListId) -> bool {
        if a == b {
            return true;
        }
        let t = &self.ast.types;
        let (xa, xb) = (t.args(a), t.args(b));
        xa.len() == xb.len()
            && xa.iter().zip(xb).all(|(x, y)| t.canonical_arg(*x) == t.canonical_arg(*y))
    }
}
