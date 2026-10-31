//! Constraint satisfaction (spec/sema-templates.md §8.3, §8.7;
//! [temp.constr.constr], [temp.constr.atomic]).
//!
//! Satisfaction is expression-based, as clang's: the top-level `&&`
//! and `||` of a constraint (through parentheses) are conjunctions and
//! disjunctions; every other expression is an atomic constraint,
//! substituted under a SFINAE frame and evaluated by the Oven with
//! [`EvalKind::Constraint`]. A concept-id is checked once per
//! `(concept, canonical arguments)` and memoized.

use kiln_ast::expr::{BinaryOp, ExprKind};
use kiln_ast::ids::{ArgListId, RawId};
use kiln_ast::types::{ArgView, BuiltinType, TemplateArg};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use super::{SatId, SatRecord, Satisfaction};
use crate::Sema;
use crate::templ::args::MultiLevelArgs;
use crate::templ::stack::FrameKind;
use crate::templ::subst::Substituter;

/// Deepest note chain rendered (the chains are acyclic; this bounds
/// the recursion).
const MAX_NOTE_DEPTH: u32 = 64;

impl Sema<'_, '_> {
    /// The associated constraints of template `tmpl` in order
    /// ([temp.constr.decl]/3): the template-head's requires-clause, each
    /// parameter's immediately-declared constraint, the trailing
    /// requires-clause.
    pub(crate) fn associated_constraints(&self, tmpl: DeclId) -> Vec<ExprId> {
        let mut out = Vec::new();
        let Some(info) = self.ast.template_info(tmpl).copied() else { return out };
        let pl = *self.ast.templates.param_list(info.params);
        self.head_constraints(pl.params, pl.requires, &mut out);
        if let Some(f) = self.templated_function(tmpl)
            && let Some(&r) = self.ast.templates.trailing_requires.get(f)
        {
            out.push(r);
        }
        out
    }

    /// A template-head's constraints: its requires-clause, then each
    /// parameter's immediately-declared constraint.
    pub(crate) fn head_constraints(
        &self,
        params: kiln_ast::ids::ListId,
        requires: Option<ExprId>,
        out: &mut Vec<ExprId>,
    ) {
        if let Some(r) = requires {
            out.push(r);
        }
        for p in self.ast.lists.iter::<DeclId>(params) {
            if let Some(c) = self.ast.template_parm(p).and_then(|i| i.constraint) {
                out.push(c);
            }
        }
    }

    /// Checks the satisfaction of concept-id `c<args>` (`args` converted,
    /// sugared), memoized on the canonical arguments (§8.3).
    pub(crate) fn check_concept(&mut self, c: DeclId, args: ArgListId, loc: Loc) -> SatId {
        let c = self.ast.first(c);
        let key = (c, self.ast.types.canonical_args(args));
        if let Some(&id) = self.inst.concepts.sat_cache.get(&key) {
            self.inst.concepts.counters.cache_hits += 1;
            return id;
        }
        if self.inst.concepts.checking.contains(&key) {
            let d = self
                .diag(DiagId::sema_constraint_depends_on_self, loc)
                .arg(self.spec_name(c, args));
            self.emit(d);
            return self.inst.concepts.push(Satisfaction { ok: false, records: Vec::new() });
        }
        self.inst.concepts.counters.count += 1;
        let Some(e) = self.ast.template_info(c).and_then(|i| i.constraint) else {
            let id = self.inst.concepts.push(Satisfaction { ok: true, records: Vec::new() });
            self.inst.concepts.sat_cache.insert(key, id);
            return id;
        };
        let depth = self.template_param_depth(c);
        let a: Vec<TemplateArg> = self.ast.types.args(args).to_vec();
        let levels = self.prior_levels(c, depth, &a);
        self.inst.concepts.checking.push(key);
        let s = self.satisfy(c, &[e], &levels, args, loc);
        self.inst.concepts.checking.pop();
        let id = self.inst.concepts.push(s);
        self.inst.concepts.sat_cache.insert(key, id);
        id
    }

    /// Checks the associated constraints of template `tmpl` (or of a
    /// partial specialization, `exprs` its constraints) for `levels`,
    /// `shown` the arguments the frame prints. Memoized on
    /// `(tmpl, canonical shown)` when `levels` has a single level.
    pub(crate) fn check_constraints(
        &mut self,
        tmpl: DeclId,
        exprs: &[ExprId],
        levels: &MultiLevelArgs,
        shown: ArgListId,
        loc: Loc,
    ) -> SatId {
        if exprs.is_empty() {
            return self.satisfied_id();
        }
        let key = (tmpl, self.ast.types.canonical_args(shown));
        let cacheable = levels.levels.len() == 1;
        if cacheable && let Some(&id) = self.inst.concepts.sat_cache.get(&key) {
            self.inst.concepts.counters.cache_hits += 1;
            return id;
        }
        self.inst.concepts.counters.count += 1;
        let s = self.satisfy(tmpl, exprs, levels, shown, loc);
        let id = self.inst.concepts.push(s);
        if cacheable {
            self.inst.concepts.sat_cache.insert(key, id);
        }
        id
    }

    /// [temp.names]/9 (clang's `EnsureTemplateArgumentListConstraints`):
    /// a template-id naming a class, variable or alias template
    /// specialization requires its associated constraints satisfied.
    /// `sel` picks the kind in the error (0 class, 2 variable, 3 alias).
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub(crate) fn ensure_template_constraints(
        &mut self,
        tmpl: DeclId,
        args: ArgListId,
        sel: u32,
        loc: Loc,
    ) -> Result<(), ()> {
        let constraints = self.associated_constraints(tmpl);
        if constraints.is_empty() {
            return Ok(());
        }
        let depth = self.template_param_depth(tmpl);
        let a: Vec<TemplateArg> = self.ast.types.args(args).to_vec();
        let levels = self.prior_levels(tmpl, depth, &a);
        let sat = self.check_constraints(tmpl, &constraints, &levels, args, loc);
        if self.sat_ok(sat) {
            return Ok(());
        }
        let d = self
            .diag(DiagId::sema_template_constraints_not_satisfied, loc)
            .arg(sel)
            .arg(self.qualified_decl_name(tmpl))
            .arg(self.template_args_desc(tmpl, args));
        let d = self.with_sat_notes(d, sat);
        self.emit(d);
        Err(())
    }

    /// The partial specialization of `tmpl` a partial specialization
    /// declaration with arguments `canonical` and template-head `head`
    /// redeclares: same arguments and equivalent constraints
    /// ([temp.spec.partial.general]/2, [temp.over.link]/6; constraints
    /// compared by their printed form).
    pub(crate) fn find_partial_decl(
        &self,
        tmpl: DeclId,
        canonical: ArgListId,
        head: kiln_ast::ids::TemplateParamListId,
    ) -> Option<u32> {
        let i = self.ast.templates.find_partial(tmpl, canonical)?;
        let sig = self.head_signature(head);
        let rows = self.ast.templates.partials_of(tmpl);
        std::iter::once(i).chain(rows.iter().copied()).find(|&r| {
            self.ast
                .templates
                .partial(r)
                .is_some_and(|p| p.canonical == canonical && self.head_signature(p.params) == sig)
        })
    }

    /// The constraints of template-head `head`, printed.
    fn head_signature(&self, head: kiln_ast::ids::TemplateParamListId) -> Vec<String> {
        let l = *self.ast.templates.param_list(head);
        let mut c = Vec::new();
        self.head_constraints(l.params, l.requires, &mut c);
        c.into_iter().map(|e| self.expr_text(e)).collect()
    }

    /// A satisfied result.
    pub(crate) fn satisfied_id(&mut self) -> SatId {
        self.inst.concepts.push(Satisfaction { ok: true, records: Vec::new() })
    }

    /// Whether satisfaction `id` holds.
    pub(crate) fn sat_ok(&self, id: SatId) -> bool {
        self.inst.concepts.sat(id).ok
    }

    /// Checks `exprs` in order under a `ConstraintsCheck` frame.
    fn satisfy(
        &mut self,
        entity: DeclId,
        exprs: &[ExprId],
        levels: &MultiLevelArgs,
        shown: ArgListId,
        loc: Loc,
    ) -> Satisfaction {
        let kind = if self.ast.decls.get(entity).kind() == kiln_ast::decl::DeclKind::Concept {
            FrameKind::ConceptCheck
        } else {
            FrameKind::ConstraintsCheck
        };
        if !self.push_inst_frame(kind, entity, entity, loc) {
            return Satisfaction { ok: false, records: Vec::new() };
        }
        self.set_frame_args(shown);
        let saved = std::mem::take(&mut self.ast.templates.dependent_depth);
        let mut records = Vec::new();
        let mut ok = true;
        for &e in exprs {
            if !self.sat_expr(entity, e, levels, &mut records, 0) {
                ok = false;
                break;
            }
        }
        self.ast.templates.dependent_depth = saved;
        self.pop_inst_frame();
        if ok {
            records.clear();
        }
        Satisfaction { ok, records }
    }

    /// Whether constraint expression `e` is satisfied
    /// ([temp.constr.op]); failures append their notes to `recs`.
    fn sat_expr(
        &mut self,
        entity: DeclId,
        e: ExprId,
        levels: &MultiLevelArgs,
        recs: &mut Vec<SatRecord>,
        depth: u32,
    ) -> bool {
        if depth > 256 {
            return self.sat_atom(entity, e, levels, recs);
        }
        let n = *self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr => {
                let o = self.ast.exprs.operand(e);
                self.sat_expr(entity, o, levels, recs, depth + 1)
            }
            ExprKind::BinaryOperator
                if matches!(self.ast.exprs.binary_op(e), BinaryOp::LAnd | BinaryOp::LOr) =>
            {
                let [l, r] = self.ast.exprs.operands(e);
                if self.ast.exprs.binary_op(e) == BinaryOp::LAnd {
                    self.sat_expr(entity, l, levels, recs, depth + 1)
                        && self.sat_expr(entity, r, levels, recs, depth + 1)
                } else {
                    let mark = recs.len();
                    if self.sat_expr(entity, l, levels, recs, depth + 1) {
                        return true;
                    }
                    if self.sat_expr(entity, r, levels, recs, depth + 1) {
                        recs.truncate(mark);
                        return true;
                    }
                    false
                }
            }
            ExprKind::ImplicitCastExpr if self.ast.exprs.get(e).ty() == QualType::BOOL => {
                let o = self.ast.exprs.operand(e);
                let k = self.ast.exprs.get(self.ignore_parens(o)).kind();
                if k == ExprKind::BinaryOperator || k == ExprKind::ParenExpr {
                    self.sat_expr(entity, o, levels, recs, depth + 1)
                } else {
                    self.sat_atom(entity, e, levels, recs)
                }
            }
            _ => self.sat_atom(entity, e, levels, recs),
        }
    }

    /// Whether atomic constraint `e` is satisfied
    /// ([temp.constr.atomic]/3).
    fn sat_atom(
        &mut self,
        entity: DeclId,
        e: ExprId,
        levels: &MultiLevelArgs,
        recs: &mut Vec<SatRecord>,
    ) -> bool {
        let loc = self.begin_loc(e);
        let Some((r, st)) =
            self.in_sfinae(FrameKind::ConstraintSubstitution, entity, None, loc, |s| {
                Substituter::new(s, levels).expr(e).ok()
            })
        else {
            return false;
        };
        let x = match (r, st) {
            (Some(x), Ok(())) => x,
            (_, st) => {
                let first = st.err().flatten();
                // A construct the substituter does not handle yet: assume
                // satisfied rather than reject a valid program.
                if first
                    .as_ref()
                    .is_some_and(|d| d.id() == Some(DiagId::sema_unimplemented_template_feature))
                {
                    return true;
                }
                let at = first.as_ref().map_or(loc, Diag::loc);
                let msg = first.map(|d| format!(": {}", d.message())).unwrap_or_default();
                recs.push(SatRecord {
                    id: DiagId::note_substituted_constraint_ill_formed,
                    loc: at,
                    select: false,
                    args: vec![msg],
                    nested: None,
                });
                return false;
            }
        };
        if self.has_errors_in(x) {
            return false;
        }
        if self.expr_is_value_dependent(x) {
            // Still dependent (an enclosing template's parameters are
            // retained): checked again when instantiated.
            return true;
        }
        let ty = self.ety(x);
        if self.ast.types.builtin(ty) != Some(BuiltinType::Bool) {
            let d = self.diag(DiagId::sema_non_bool_atomic_constraint, loc).arg(self.ty_arg(ty));
            self.emit(d);
            return false;
        }
        let c = crate::conv::contextually_convert_to_bool(self, x).unwrap_or(x);
        match self.eval_const(c, EvalContext::new(EvalKind::Constraint, loc)) {
            Ok(v) if self.const_truth(v) == Some(true) => true,
            Ok(_) => {
                let r = self.false_atom_records(x);
                recs.extend(r);
                false
            }
            Err(_) => {
                self.error(DiagId::sema_non_constant_constraint, loc);
                false
            }
        }
    }

    /// The notes explaining why the (substituted, non-dependent) atomic
    /// constraint `x` evaluated to false (§8.7).
    pub(crate) fn false_atom_records(&mut self, x: ExprId) -> Vec<SatRecord> {
        let inner = self.ignore_paren_imp_casts(x);
        let n = *self.ast.exprs.get(inner);
        let at = self.begin_loc(inner);
        match n.kind() {
            ExprKind::ConceptSpecializationExpr => {
                let nested = self.inst.concepts.expr_sat.get(&inner).copied();
                let c = DeclId::from_raw(n.a());
                let written = ArgListId::from_raw(n.b());
                if let (Some(c), Some(w)) = (c, written) {
                    let args = self.ast.types.args(w).to_vec();
                    if let [a] = args[..]
                        && !matches!(a.view(), ArgView::Pack(_))
                    {
                        return vec![SatRecord {
                            id: DiagId::note_single_arg_concept_false,
                            loc: self.eloc(inner),
                            select: true,
                            args: vec![self.arg_str(a), self.decl_name(c)],
                            nested,
                        }];
                    }
                }
                vec![SatRecord {
                    id: DiagId::note_atomic_constraint_false,
                    loc: self.eloc(inner),
                    select: true,
                    args: vec![self.expr_text(inner)],
                    nested,
                }]
            }
            ExprKind::RequiresExpr => {
                if let Some(&id) = self.inst.concepts.expr_sat.get(&inner) {
                    let r = self.inst.concepts.sat(id).records.clone();
                    if !r.is_empty() {
                        return r;
                    }
                }
                vec![SatRecord {
                    id: DiagId::note_atomic_constraint_false,
                    loc: at,
                    select: true,
                    args: vec![self.expr_text(inner)],
                    nested: None,
                }]
            }
            _ => {
                if let Some((op, l, r)) = self.comparison_parts(inner)
                    && matches!(op, "<" | ">" | "<=" | ">=" | "==" | "!=")
                    && self.ast.types.is_integral(self.ety(l))
                    && self.ast.types.is_integral(self.ety(r))
                    && let (Some(lv), Some(rv)) = (self.fold_int(l), self.fold_int(r))
                {
                    return vec![SatRecord {
                        id: DiagId::note_atomic_constraint_false_elaborated,
                        loc: at,
                        select: true,
                        args: vec![
                            self.expr_text(inner),
                            lv.to_string(),
                            op.to_string(),
                            rv.to_string(),
                        ],
                        nested: None,
                    }];
                }
                vec![SatRecord {
                    id: DiagId::note_atomic_constraint_false,
                    loc: at,
                    select: true,
                    args: vec![self.expr_text(inner)],
                    nested: None,
                }]
            }
        }
    }

    /// `e` as source text (clang's `printPretty`, canonical types).
    pub(crate) fn expr_text(&self, e: ExprId) -> String {
        let n = *self.ast.exprs.get(e);
        if n.kind() == ExprKind::ConceptSpecializationExpr
            && let (Some(c), Some(l)) = (DeclId::from_raw(n.a()), ArgListId::from_raw(n.b()))
        {
            let args: Vec<String> =
                self.ast.types.args(l).iter().map(|&a| self.arg_str(a)).collect();
            return format!("{}<{}>", self.decl_name(c), args.join(", "));
        }
        let mut s = String::new();
        kiln_ast::types::TypeNames::write_expr(
            &kiln_ast::dump::AstNames::new(&self.ast).canonical_expr_types(),
            e,
            &mut s,
        );
        s
    }

    /// The notes of satisfaction `id`, as a chain starting "because".
    pub(crate) fn sat_notes(&self, id: SatId) -> Vec<Diag> {
        let mut out = Vec::new();
        self.render_records(&self.inst.concepts.sat(id).records, &mut out, 0);
        out
    }

    /// The notes of `recs`, as a chain starting "because".
    pub(crate) fn render_records(&self, recs: &[SatRecord], out: &mut Vec<Diag>, depth: u32) {
        if depth > MAX_NOTE_DEPTH {
            return;
        }
        for (i, r) in recs.iter().enumerate() {
            let mut d = self.diag(r.id, r.loc);
            if r.select {
                d = d.arg(u32::from(i == 0));
            }
            for a in &r.args {
                d = d.arg(a);
            }
            out.push(d);
            if let Some(n) = r.nested {
                self.render_records(&self.inst.concepts.sat(n).records, out, depth + 1);
            }
        }
    }

    /// `d` with the notes of satisfaction `id` attached.
    pub(crate) fn with_sat_notes(&self, mut d: Diag, id: SatId) -> Diag {
        for n in self.sat_notes(id) {
            d = d.with_note(n);
        }
        d
    }

    /// Forms the concept-id `c<written>`: a non-dependent one is checked
    /// now ([temp.names]/9), its satisfaction recorded on the expression.
    pub(crate) fn build_concept_id(
        &mut self,
        c: DeclId,
        written: ArgListId,
        arg_locs: &[Loc],
        loc: Loc,
    ) -> Result<ExprId, ()> {
        let c = self.ast.first(c);
        let args: Vec<TemplateArg> = self.ast.types.args(written).to_vec();
        let dep = self
            .ast
            .types
            .arg_list_flags(written)
            .intersects(kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION)
            || args.iter().any(|&a| self.is_pack_expansion_arg(a));
        if dep {
            return Ok(self.builder().concept_spec_expr(c, written, QualType::BOOL, loc));
        }
        let conv = self.check_template_args(c, &args, arg_locs, loc)?;
        let e = self.builder().concept_spec_expr(c, written, QualType::BOOL, loc);
        let sat = self.check_concept(c, conv.sugared, loc);
        let ok = self.sat_ok(sat);
        let _ = self.ast.exprs.set_satisfied(e, ok);
        self.inst.concepts.expr_sat.insert(e, sat);
        Ok(e)
    }
}

impl Substituter<'_, '_, '_> {
    /// Substitutes into concept-id `e` and checks it once non-dependent.
    pub(crate) fn concept_id_expr(
        &mut self,
        e: ExprId,
        loc: Loc,
    ) -> crate::templ::subst::SubstResult<ExprId> {
        let n = *self.sema.ast.exprs.get(e);
        let (Some(c), Some(l)) = (DeclId::from_raw(n.a()), ArgListId::from_raw(n.b())) else {
            return Err(crate::templ::subst::SubstFail);
        };
        let nl = self.args(l, loc)?;
        let at = self.sema.ast.exprs.get(e).loc();
        self.sema.build_concept_id(c, nl, &[], at).map_err(|()| crate::templ::subst::SubstFail)
    }
}
