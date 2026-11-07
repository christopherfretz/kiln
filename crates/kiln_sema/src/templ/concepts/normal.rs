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
    /// A fold expanded constraint ([temp.constr.fold]/1, P2963R3): the
    /// normal form of a fold `(E && ...)`/`(E || ...)` whose pattern
    /// expands packs the normalization cannot fix. `pattern` is `E`'s
    /// normal form (with the packs' parameters standing for one
    /// element); `packs` the identities of the packs `E` expands
    /// ([`NormTable::pack_set`]), compared by [temp.constr.fold]/2's
    /// "equivalent unexpanded pack". A fold is a leaf of the DNF/CNF
    /// (§8.5).
    Fold {
        /// `&&` (a conjunction of the elements) or `||`.
        and: bool,
        /// The normal form of the pattern.
        pattern: NormId,
        /// The interned pack set.
        packs: u32,
    },
}

/// The interned normal forms: equal nodes share an id, so atomic
/// constraint identity is an id comparison (§8.5).
#[derive(Debug, Default)]
pub struct NormTable {
    nodes: Vec<NormNode>,
    map: FxHashMap<NormNode, NormId>,
    pack_sets: Vec<Box<[u32]>>,
    pack_map: FxHashMap<Box<[u32]>, u32>,
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

    /// The id of the pack set `packs` (each pack `depth << 16 | index`,
    /// sorted and deduplicated here), interning it.
    pub fn intern_packs(&mut self, mut packs: Vec<u32>) -> u32 {
        packs.sort_unstable();
        packs.dedup();
        let key = packs.into_boxed_slice();
        if let Some(&id) = self.pack_map.get(&key) {
            return id;
        }
        let id = u32::try_from(self.pack_sets.len()).unwrap_or(u32::MAX);
        self.pack_sets.push(key.clone());
        self.pack_map.insert(key, id);
        id
    }

    /// The packs of pack set `id`.
    pub fn pack_set(&self, id: u32) -> &[u32] {
        self.pack_sets.get(id as usize).map_or(&[], |b| b)
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
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::ids::RawId;
use kiln_ast::templates::TtpKind;
use kiln_ast::types::{ArgView, TemplateArg};

use kiln_source::Loc;

use crate::Sema;
use crate::templ::pack::PackOperand;
use crate::templ::stack::FrameKind;
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
        if depth > MAX_EXPANSION {
            return self.intern_atom(e, ctx);
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
            ExprKind::CXXFoldExpr => {
                if let Some(x) = self.normalize_concept_fold(e, ctx, depth) {
                    return x;
                }
                if let Some(x) = self.normalize_fold_expanded(e, ctx, depth) {
                    return x;
                }
            }
            _ => {}
        }
        self.intern_atom(e, ctx)
    }

    /// The atomic constraint `e` with the parameter mapping `ctx` gives
    /// it, restricted to the parameters `e` names ([temp.constr.atomic]/1):
    /// the others' arguments are null, so `sizeof(T) > 1` of `C<T, int>`
    /// and of `C<T, long>` is one atom. Unknown uses keep the whole
    /// mapping.
    fn intern_atom(&mut self, e: ExprId, ctx: Option<(DeclId, ArgListId)>) -> NormId {
        let mut mapping = ctx.map_or(ArgListId::EMPTY, |(_, m)| m);
        if let Some((cc, m)) = ctx {
            let level = self.template_param_depth(cc);
            if let Some(used) = self.used_template_params(e, level) {
                let args = self.ast.types.args(m);
                if args.iter().enumerate().any(|(i, a)| {
                    (i >= 64 || used & (1 << i) == 0) && a.kind() != kiln_ast::types::ArgKind::Null
                }) {
                    let v: Vec<TemplateArg> = args
                        .iter()
                        .enumerate()
                        .map(
                            |(i, &a)| {
                                if i < 64 && used & (1 << i) != 0 { a } else { TemplateArg::NULL }
                            },
                        )
                        .collect();
                    mapping = self.ast.types.arg_list(&v);
                }
            }
        }
        self.inst.concepts.norm.intern(NormNode::Atomic { expr: e, mapping })
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
        let ttp = self.kind_of(c) == DeclKind::TemplateTemplateParm;
        let (c, args) = match ctx {
            // [temp.constr.normal]/1.4.1: `C<A...>` naming a concept
            // template parameter is a concept-dependent constraint.
            None if ttp => return None,
            None => (c, l),
            Some((cc, m)) => {
                let d = self.template_param_depth(cc);
                let prior: Vec<TemplateArg> = self.ast.types.args(m).to_vec();
                let levels = self.prior_levels(cc, d, &prior);
                let name = ttp.then(|| self.template_name_for(c));
                let mut s = Substituter::new(self, &levels);
                let args = s.args(l, n.loc()).ok()?;
                // /1.4.2: the concept argument the mapping gives a
                // concept template parameter is substituted; one that is
                // itself a concept template parameter leaves the
                // constraint concept-dependent.
                let c = match name {
                    Some(t) => {
                        let t = s.template_name(t, n.loc()).ok()?;
                        let c2 = self.ast.types.template_decl(t)?;
                        if self.kind_of(c2) != DeclKind::Concept {
                            return None;
                        }
                        self.ast.first(c2)
                    }
                    None => c,
                };
                (c, args)
            }
        };
        let args = self.concept_default_args(c, args, n.loc());
        let args = self.concept_mapping_args(c, args);
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

    /// [temp.constr.normal]/1.5.3: a fold `(E op ...)` (or `(E op ... op
    /// I)`) with `op` `&&` or `||` whose pattern expands only concept
    /// template parameter packs of the concept `ctx` names, all given
    /// `N > 0` concept arguments by its parameter mapping, normalizes as
    /// `E0 op ... op EN-1`, each `Ei` normalized with the packs' `i`th
    /// arguments in the mapping (/1.5.3.1). Anything else (`None`) stays
    /// one atom: a fold over other packs, or over concept packs the
    /// mapping does not fix (/1.5.3.2's fold expanded constraint, which
    /// is concept-dependent and so never subsumes, §8.5).
    fn normalize_concept_fold(
        &mut self,
        e: ExprId,
        ctx: Option<(DeclId, ArgListId)>,
        depth: u32,
    ) -> Option<NormId> {
        use kiln_ast::expr::ExprDeps;
        let (cc, m) = ctx?;
        let n = *self.ast.exprs.get(e);
        let op = BinaryOp::from_u8(n.aux())?;
        if !matches!(op, BinaryOp::LAnd | BinaryOp::LOr) {
            return None;
        }
        let (lhs, rhs) = (ExprId::from_raw(n.a()), ExprId::from_raw(n.b()));
        let right =
            lhs.is_some_and(|x| self.ast.exprs.get(x).deps().contains(ExprDeps::UNEXPANDED_PACK));
        let (pattern, init) = if right { (lhs?, rhs) } else { (rhs?, lhs) };
        let level = self.template_param_depth(cc);
        let mut slots = Vec::new();
        for p in self.unexpanded_packs(PackOperand::Expr(pattern)) {
            let info = self.ast.template_parm(p)?;
            if self.ttp_kind(p) != TtpKind::Concept || info.depth != level {
                return None;
            }
            slots.push(usize::from(info.index));
        }
        let margs: Vec<TemplateArg> = self.ast.types.args(m).to_vec();
        let mut elems: Vec<Vec<TemplateArg>> = Vec::with_capacity(slots.len());
        for &i in &slots {
            let ArgView::Pack(l) = margs.get(i)?.view() else { return None };
            let v = self.ast.types.args(l).to_vec();
            let known = v.iter().all(|a| {
                matches!(a.view(), ArgView::Template(t)
                    if self.ast.types.template_decl(t)
                        .is_some_and(|d| self.kind_of(d) == DeclKind::Concept))
            });
            if !known || elems.first().is_some_and(|f| f.len() != v.len()) {
                return None;
            }
            elems.push(v);
        }
        let len = elems.first().map_or(0, Vec::len);
        if len == 0 {
            return None;
        }
        let mut acc: Option<NormId> = None;
        for i in 0..len {
            let mut mi = margs.clone();
            for (&s, e) in slots.iter().zip(&elems) {
                mi[s] = e[i];
            }
            let mi = self.ast.types.arg_list(&mi);
            let x = self.normalize(pattern, Some((cc, mi)), depth + 1);
            acc = Some(acc.map_or(x, |a| self.norm_join(op, a, x)));
        }
        let mut acc = acc?;
        if let Some(init) = init {
            let y = self.normalize(init, ctx, depth + 1);
            acc = if right { self.norm_join(op, acc, y) } else { self.norm_join(op, y, acc) };
        }
        Some(acc)
    }

    /// [temp.constr.normal]/1.5.2, [temp.constr.fold]/1 (P2963R3): a fold
    /// `(E op ...)` or `(... op E)` with `op` `&&` or `||` normalizes to
    /// a fold expanded constraint whose constraint is `E`'s normal form;
    /// `(E op ... op I)` to `(E op ...) op I` and `(I op ... op E)` to
    /// `I op (... op E)`. The packs `E` expands are identified by their
    /// parameters' (depth, index): the constrained template's own packs,
    /// or, inside a concept, the packs its mapping gives the concept's
    /// pack parameters (`AllA<Ts...>`: the concept's pack stands for
    /// `Ts`, and its pattern is normalized with `Ts` in its place).
    /// `None` (the fold stays one atom) for a fold over a concept
    /// template parameter pack (concept-dependent, handled by
    /// [`Self::normalize_concept_fold`] or kept whole) and for a mapping
    /// that gives a pack anything but one plain expansion of a pack.
    fn normalize_fold_expanded(
        &mut self,
        e: ExprId,
        ctx: Option<(DeclId, ArgListId)>,
        depth: u32,
    ) -> Option<NormId> {
        use kiln_ast::expr::ExprDeps;
        let n = *self.ast.exprs.get(e);
        let op = BinaryOp::from_u8(n.aux())?;
        if !matches!(op, BinaryOp::LAnd | BinaryOp::LOr) {
            return None;
        }
        let (lhs, rhs) = (ExprId::from_raw(n.a()), ExprId::from_raw(n.b()));
        let right =
            lhs.is_some_and(|x| self.ast.exprs.get(x).deps().contains(ExprDeps::UNEXPANDED_PACK));
        let (pattern, init) = if right { (lhs?, rhs) } else { (rhs?, lhs) };
        let packs = self.unexpanded_packs(PackOperand::Expr(pattern));
        if packs.is_empty() {
            return None;
        }
        let mut ids = Vec::with_capacity(packs.len());
        let mut pctx = ctx;
        match ctx {
            None => {
                for p in packs {
                    if self.ttp_kind(p) == TtpKind::Concept {
                        return None;
                    }
                    let info = self.ast.template_parm(p)?;
                    ids.push(u32::from(info.depth) << 16 | u32::from(info.index));
                }
            }
            Some((cc, m)) => {
                let level = self.template_param_depth(cc);
                let mut margs: Vec<TemplateArg> = self.ast.types.args(m).to_vec();
                for p in packs {
                    let info = *self.ast.template_parm(p)?;
                    if self.ttp_kind(p) == TtpKind::Concept || info.depth != level {
                        return None;
                    }
                    let slot = margs.get_mut(usize::from(info.index))?;
                    let ArgView::Pack(l) = slot.view() else { return None };
                    let &[x] = self.ast.types.args(l) else { return None };
                    let (arg, id) = self.fold_pack_element(x)?;
                    *slot = arg;
                    ids.push(id);
                }
                let mi = self.ast.types.arg_list(&margs);
                pctx = Some((cc, self.ast.types.canonical_args(mi)));
            }
        }
        let inner = self.normalize(pattern, pctx, depth + 1);
        let set = self.inst.concepts.norm.intern_packs(ids);
        let node = NormNode::Fold { and: op == BinaryOp::LAnd, pattern: inner, packs: set };
        let mut acc = self.inst.concepts.norm.intern(node);
        if let Some(init) = init {
            let y = self.normalize(init, ctx, depth + 1);
            acc = if right { self.norm_join(op, acc, y) } else { self.norm_join(op, y, acc) };
        }
        Some(acc)
    }

    /// The one element `x` of a pack argument in a concept's parameter
    /// mapping, when it is a plain pack expansion of a pack parameter
    /// (`Ts...`, `Ns...`): that parameter as the element standing for
    /// each of its elements, and its identity (`depth << 16 | index`).
    fn fold_pack_element(&mut self, x: TemplateArg) -> Option<(TemplateArg, u32)> {
        use kiln_ast::types::TypeView;
        match x.view() {
            ArgView::Type(q) => {
                let p = crate::templ::pack::type_pattern(self, q)?;
                let id = match self.ast.types.canonical_view(p) {
                    TypeView::CanonTemplateTypeParm { depth, index, pack: true } => {
                        depth << 16 | index
                    }
                    _ => return None,
                };
                Some((TemplateArg::ty(p), id))
            }
            ArgView::Expr { expr, .. } => {
                let p = crate::templ::pack::expr_pattern(self, expr)?;
                let pn = *self.ast.exprs.get(p);
                if pn.kind() != ExprKind::DeclRefExpr {
                    return None;
                }
                let d = self.ast.exprs.decl(p);
                let info = self.ast.template_parm(d)?;
                if !info.pack {
                    return None;
                }
                let id = u32::from(info.depth) << 16 | u32::from(info.index);
                Some((self.ast.builder().template_arg_expr(p), id))
            }
            _ => None,
        }
    }

    /// `a op b` for `op` `&&` (a conjunction) or `||`.
    fn norm_join(&mut self, op: BinaryOp, a: NormId, b: NormId) -> NormId {
        let node = if op == BinaryOp::LAnd { NormNode::And(a, b) } else { NormNode::Or(a, b) };
        self.inst.concepts.norm.intern(node)
    }

    /// Whether atom `n` is a concept-dependent constraint
    /// ([temp.constr.normal]/1.4.1): a concept-id naming a concept
    /// template parameter, or a fold over a concept template parameter
    /// pack the normalization left unexpanded (/1.5.3.2).
    pub(crate) fn is_concept_dependent(&self, n: NormId) -> bool {
        let NormNode::Atomic { expr, .. } = self.inst.concepts.norm.get(n) else { return false };
        let x = self.ast.exprs.get(expr);
        match x.kind() {
            ExprKind::ConceptSpecializationExpr => DeclId::from_raw(x.a())
                .is_some_and(|d| self.kind_of(d) == DeclKind::TemplateTemplateParm),
            ExprKind::CXXFoldExpr => {
                [x.a(), x.b()].into_iter().filter_map(ExprId::from_raw).any(|k| {
                    self.unexpanded_packs(PackOperand::Expr(k))
                        .iter()
                        .any(|&p| self.ttp_kind(p) == TtpKind::Concept)
                })
            }
            _ => false,
        }
    }

    /// The arguments `args` of a concept-id of `c` completed with the
    /// concept's default template arguments ([temp.arg.general]/9): the
    /// parameter mapping is formed from the converted arguments, so
    /// `K<F, char*>` and `K<F, char*, int>` name the same atomic
    /// constraints when `int` is the third parameter's default
    /// ([temp.constr.normal]/1.4, [temp.constr.atomic]/2). The defaults
    /// are substituted in a SFINAE context; on failure the written
    /// arguments are kept (the concept-id is then an atom of its own).
    pub(crate) fn concept_default_args(
        &mut self,
        c: DeclId,
        args: ArgListId,
        loc: Loc,
    ) -> ArgListId {
        let params = self.template_params(c);
        let written: Vec<TemplateArg> = self.ast.types.args(args).to_vec();
        if written.len() >= params.len() || written.iter().any(|&a| self.is_pack_expansion_arg(a)) {
            return args;
        }
        let depth = self.template_param_depth(c);
        let r = self.in_sfinae(FrameKind::ConstraintSubstitution, c, None, loc, |s| {
            let mut out = written.clone();
            for &p in &params[written.len()..] {
                if s.ast.template_parm(p).is_some_and(|x| x.pack) {
                    break;
                }
                let def = s.template_default(p)?;
                out.push(s.default_template_arg(c, depth, p, def, loc, &out).ok()?);
            }
            Some(out)
        });
        match r {
            Some((Some(out), Ok(()))) => self.ast.types.arg_list(&out),
            _ => args,
        }
    }

    /// The written arguments `args` of a concept-id of `c`, shaped as its
    /// parameter mapping: the arguments for a trailing parameter pack are
    /// grouped into one pack argument ([temp.arg.general]/4), so the
    /// concept's own constraint substitutes `A...` element-wise and the
    /// mapping is the same whichever route named the concept.
    fn concept_mapping_args(&mut self, c: DeclId, args: ArgListId) -> ArgListId {
        let params = self.template_params(c);
        let Some((&last, fixed)) = params.split_last() else { return args };
        if !self.ast.template_parm(last).is_some_and(|x| x.pack) {
            return args;
        }
        let written: Vec<TemplateArg> = self.ast.types.args(args).to_vec();
        if written.len() < fixed.len()
            || written[..fixed.len()].iter().any(|&a| self.is_pack_expansion_arg(a))
            || (written.len() == params.len()
                && matches!(written[fixed.len()].view(), ArgView::Pack(_)))
        {
            return args;
        }
        let tail = self.ast.types.arg_list(&written[fixed.len()..]);
        let mut out = written[..fixed.len()].to_vec();
        out.push(TemplateArg::pack(tail));
        self.ast.types.arg_list(&out)
    }
}
