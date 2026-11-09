//! Subsumption (spec/sema-templates.md §8.5, [temp.constr.order]).

use super::{NormId, NormNode};
use crate::Sema;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::{Diag, DiagId};

/// Most clauses a DNF/CNF conversion may produce (§8.5); when both forms
/// a subsumption check could use are past it, the check answers "does not
/// subsume".
const MAX_CLAUSES: usize = 65_536;

type Clauses = Vec<Vec<NormId>>;

impl Sema<'_, '_> {
    /// Whether normal form `p` subsumes `q` ([temp.constr.order]/1): every
    /// disjunctive clause of `p`'s DNF has a leaf subsuming a leaf of
    /// every conjunctive clause of `q`'s CNF ([`Self::leaf_subsumes`]).
    /// Cached.
    pub(crate) fn subsumes(&mut self, p: NormId, q: NormId) -> bool {
        if p == q {
            return true;
        }
        if let Some(&r) = self.inst.concepts.subsume_cache.get(&(p, q)) {
            return r;
        }
        self.inst.concepts.counters.subsume += 1;
        let r = self.subsumes_by(p, q, &mut |s: &mut Self, a, b| s.leaf_subsumes(a, b));
        self.inst.concepts.subsume_cache.insert((p, q), r);
        r
    }

    /// Whether leaf `a` of a DNF subsumes leaf `b` of a CNF
    /// ([temp.constr.order]/1.1): identical atomic constraints
    /// ([temp.constr.atomic]/2), or fold expanded constraints with the
    /// same fold-operator that are compatible for subsumption (their
    /// patterns expand an equivalent pack, [temp.constr.fold]/2) where
    /// `a`'s constraint subsumes `b`'s (P2963R3).
    fn leaf_subsumes(&mut self, a: NormId, b: NormId) -> bool {
        if a == b {
            return true;
        }
        match (self.inst.concepts.norm.get(a), self.inst.concepts.norm.get(b)) {
            (
                NormNode::Fold { and: x, pattern: pa, packs: sa },
                NormNode::Fold { and: y, pattern: pb, packs: sb },
            ) if x == y && self.packs_compatible(sa, sb, false) => self.subsumes(pa, pb),
            _ => false,
        }
    }

    /// Whether pack sets `a` and `b` share a pack (by depth and index, or
    /// by index alone when `positional`).
    fn packs_compatible(&self, a: u32, b: u32, positional: bool) -> bool {
        let t = &self.inst.concepts.norm;
        let key = |x: u32| if positional { x & 0xffff } else { x };
        let (pa, pb) = (t.pack_set(a), t.pack_set(b));
        pa.iter().any(|&x| pb.iter().any(|&y| key(x) == key(y)))
    }

    /// The clauses of `n`: its DNF (`dnf`, each clause a conjunction) or
    /// CNF (each a disjunction); `None` past [`MAX_CLAUSES`].
    fn clauses(&self, n: NormId, dnf: bool) -> Option<Clauses> {
        match self.inst.concepts.norm.get(n) {
            NormNode::Atomic { .. } | NormNode::Fold { .. } => Some(vec![vec![n]]),
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

    /// Whether normal form `n` is eligible for subsumption
    /// ([temp.constr.order]/3): it contains no concept-dependent
    /// constraint, the constraints of fold expanded constraints
    /// included.
    fn eligible_for_subsumption(&self, n: NormId) -> bool {
        let mut seen = kiln_source::fx::FxHashSet::default();
        let mut work = vec![n];
        while let Some(x) = work.pop() {
            if !seen.insert(x) {
                continue;
            }
            match self.inst.concepts.norm.get(x) {
                NormNode::Atomic { .. } => {
                    if self.is_concept_dependent(x) {
                        return false;
                    }
                }
                NormNode::And(a, b) | NormNode::Or(a, b) => work.extend([a, b]),
                NormNode::Fold { pattern, .. } => work.push(pattern),
            }
        }
        true
    }

    /// Whether the entity with constraints `ca` (key `a`) is more
    /// constrained than the one with `cb` (key `b`)
    /// ([temp.constr.order]/4, /5): it is at least as constrained (its
    /// normal form is eligible for subsumption and subsumes the other's)
    /// and not conversely. No constraints is the weakest.
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
            (Some(p), Some(q)) => {
                self.eligible_for_subsumption(p)
                    && self.subsumes(p, q)
                    && !(self.eligible_for_subsumption(q) && self.subsumes(q, p))
            }
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

    /// Whether constraints `ca` of `a` subsume `cb` of `b` when the two
    /// are written over different template parameter lists of the same
    /// shape (a template template parameter and its argument,
    /// [temp.arg.template]/3): atoms are identical when they are the same
    /// expression with mappings equal up to each type parameter's depth.
    /// Unconstrained `cb` is always subsumed.
    pub(crate) fn at_least_as_constrained_positional(
        &mut self,
        a: DeclId,
        ca: &[ExprId],
        b: DeclId,
        cb: &[ExprId],
    ) -> bool {
        let Some(q) = self.normal_form(b, cb) else { return true };
        let Some(p) = self.normal_form(a, ca) else { return false };
        self.subsumes_positional(p, q, 0)
    }

    /// [`Self::subsumes`] with atoms compared positionally (same
    /// expression, [`Self::mappings_match_positional`]) and fold
    /// expanded constraints' packs by index.
    fn subsumes_positional(&mut self, p: NormId, q: NormId, depth: u32) -> bool {
        if depth > 64 {
            return false;
        }
        self.subsumes_by(p, q, &mut |s: &mut Self, x, y| {
            if x == y {
                return true;
            }
            match (s.inst.concepts.norm.get(x), s.inst.concepts.norm.get(y)) {
                (
                    NormNode::Atomic { expr: ex, mapping: mx },
                    NormNode::Atomic { expr: ey, mapping: my },
                ) => ex == ey && s.mappings_match_positional(mx, my),
                (
                    NormNode::Fold { and: ax, pattern: px, packs: sx },
                    NormNode::Fold { and: ay, pattern: py, packs: sy },
                ) => {
                    ax == ay
                        && s.packs_compatible(sx, sy, true)
                        && s.subsumes_positional(px, py, depth + 1)
                }
                _ => false,
            }
        })
    }

    /// [`Sema::mappings_match`] with type parameters compared by index
    /// and pack-ness, not depth.
    fn mappings_match_positional(
        &self,
        a: kiln_ast::ids::ArgListId,
        b: kiln_ast::ids::ArgListId,
    ) -> bool {
        let t = &self.ast.types;
        let (xa, xb) = (t.args(a), t.args(b));
        let parm = |x: kiln_ast::types::TemplateArg| match x.view() {
            kiln_ast::types::ArgView::Type(q) => match t.canonical_view(q) {
                kiln_ast::types::TypeView::CanonTemplateTypeParm { index, pack, .. } => {
                    Some((index, pack))
                }
                _ => None,
            },
            _ => None,
        };
        xa.len() == xb.len()
            && xa.iter().zip(xb).all(|(x, y)| {
                t.canonical_arg(*x) == t.canonical_arg(*y)
                    || parm(*x).is_some_and(|i| parm(*y) == Some(i))
            })
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
        self.similar_atom_notes(d1, &ac1, d2, &ac2)
    }

    /// clang's `MaybeEmitAmbiguousAtomicConstraintsDiagnostic`: when
    /// treating textually identical atoms of constraints `ac1` (keyed by
    /// `d1`) and `ac2` (keyed by `d2`) as identical would change either
    /// subsumption direction, the "similar constraint" note pair naming the
    /// last such pair of atoms; otherwise none.
    pub(crate) fn similar_atom_notes(
        &mut self,
        d1: DeclId,
        ac1: &[ExprId],
        d2: DeclId,
        ac2: &[ExprId],
    ) -> Vec<Diag> {
        if ac1.is_empty() || ac2.is_empty() {
            return Vec::new();
        }
        let (Some(n1), Some(n2)) = (self.normal_form(d1, ac1), self.normal_form(d2, ac2)) else {
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
        let normal12 = self.clauses_subsume(&dnf1, &cnf2, &mut |s, a, b| s.leaf_subsumes(a, b));
        let normal21 = self.clauses_subsume(&dnf2, &cnf1, &mut |s, a, b| s.leaf_subsumes(a, b));
        let mut pair: Option<(ExprId, ExprId)> = None;
        let mut identical = |s: &mut Self, a: NormId, b: NormId| -> bool {
            if s.leaf_subsumes(a, b) {
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
            if !s.exprs_equivalent(ea, eb) {
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
        let (la, lb) = (self.atom_note_loc(a), self.atom_note_loc(b));
        vec![
            self.diag(DiagId::note_ambiguous_atomic_constraints, la),
            self.diag(DiagId::note_ambiguous_atomic_constraints_similar_expression, lb),
        ]
    }

    /// Where a "similar constraint" note points for atom `e`: its begin,
    /// except that a fold atom (a concept-dependent fold, kept whole)
    /// points at its pattern, as clang's fold-expanded constraint does.
    fn atom_note_loc(&self, e: ExprId) -> kiln_source::Loc {
        use kiln_ast::expr::{ExprDeps, ExprKind};
        use kiln_ast::ids::RawId;
        let n = *self.ast.exprs.get(e);
        if n.kind() == ExprKind::CXXFoldExpr {
            let (l, r) = (ExprId::from_raw(n.a()), ExprId::from_raw(n.b()));
            let pat = [l, r]
                .into_iter()
                .flatten()
                .find(|&x| self.ast.exprs.get(x).deps().contains(ExprDeps::UNEXPANDED_PACK));
            if let Some(p) = pat {
                return self.begin_loc(p);
            }
        }
        self.begin_loc(e)
    }

    /// Whether `p` subsumes `q` with leaf relation `eq` ([temp.constr.order]/1),
    /// building only one clause form (§8.5). Leaves are never negated, so
    /// a normal form is a monotone function of its leaves: a DNF clause
    /// `Pi` of `p` subsumes every CNF clause of `q` exactly when `q` is
    /// true with the leaves some leaf of `Pi` subsumes true and the rest
    /// false; dually, `p` subsumes the CNF clause `Qj` of `q` exactly when
    /// `p` is false with the leaves subsuming a leaf of `Qj` false. So a
    /// four-of-eight majority, whose CNF before absorption has `4^70`
    /// clauses, is compared through its 70-clause DNF. `false` when
    /// neither form fits [`MAX_CLAUSES`].
    fn subsumes_by(
        &mut self,
        p: NormId,
        q: NormId,
        eq: &mut dyn FnMut(&mut Self, NormId, NormId) -> bool,
    ) -> bool {
        if let Some(dnf) = self.clauses(p, true) {
            return dnf.iter().all(|pi| {
                let mut memo = kiln_source::fx::FxHashMap::default();
                self.eval_norm(q, &mut |s, b| pi.iter().any(|&a| eq(s, a, b)), &mut memo)
            });
        }
        let Some(cnf) = self.clauses(q, false) else { return false };
        cnf.iter().all(|qj| {
            let mut memo = kiln_source::fx::FxHashMap::default();
            !self.eval_norm(p, &mut |s, a| !qj.iter().any(|&b| eq(s, a, b)), &mut memo)
        })
    }

    /// The truth of normal form `n` with each leaf's value given by
    /// `leaf` (memoized per node in `memo`: normal forms share subtrees).
    fn eval_norm(
        &mut self,
        n: NormId,
        leaf: &mut dyn FnMut(&mut Self, NormId) -> bool,
        memo: &mut kiln_source::fx::FxHashMap<NormId, bool>,
    ) -> bool {
        if let Some(&v) = memo.get(&n) {
            return v;
        }
        let v = match self.inst.concepts.norm.get(n) {
            NormNode::And(a, b) => self.eval_norm(a, leaf, memo) && self.eval_norm(b, leaf, memo),
            NormNode::Or(a, b) => self.eval_norm(a, leaf, memo) || self.eval_norm(b, leaf, memo),
            NormNode::Atomic { .. } | NormNode::Fold { .. } => leaf(self, n),
        };
        memo.insert(n, v);
        v
    }

    /// clang's `subsumes(PDNF, QCNF, E)` over precomputed clauses, with
    /// atom evaluator `eq`.
    fn clauses_subsume(
        &mut self,
        p: &Clauses,
        q: &Clauses,
        eq: &mut dyn FnMut(&mut Self, NormId, NormId) -> bool,
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
