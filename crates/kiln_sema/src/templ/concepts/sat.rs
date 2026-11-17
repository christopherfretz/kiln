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
use kiln_ast::types::{ArgView, BuiltinType, TemplateArg, TypeFlags};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use super::{AtomKey, SatId, SatRecord, Satisfaction};
use crate::Sema;
use crate::templ::args::{LevelMode, MultiLevelArgs};
use crate::templ::stack::FrameKind;
use crate::templ::subst::Substituter;

/// Deepest note chain rendered (the chains are acyclic; this bounds
/// the recursion).
const MAX_NOTE_DEPTH: u32 = 64;

/// What a constraint substitution gives: `f`'s result and the SFINAE
/// frame's outcome (the first error, if one was kept).
type SubstOutcome<R> = (Option<R>, Result<(), Option<Diag>>);

impl Sema<'_, '_> {
    /// The associated constraints of template `tmpl` in order
    /// ([temp.constr.decl]/3, as clang 19 orders them): each template
    /// parameter's immediately-declared constraint, the template-head's
    /// requires-clause, the trailing requires-clause.
    pub(crate) fn associated_constraints(&self, tmpl: DeclId) -> Vec<ExprId> {
        if let Some(&(pattern, _)) = self.inst.concepts.member_constraints.get(&tmpl)
            && pattern != tmpl
            && self.ast.template_info(tmpl).is_some()
        {
            return self.associated_constraints(pattern);
        }
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

    /// A template-head's constraints: each template parameter's
    /// immediately-declared constraint, then its requires-clause
    /// ([temp.constr.decl]/3.1, /3.2). Satisfaction checks them in this
    /// order and stops at the first unsatisfied one, so a type-constraint
    /// guards the substitution into the requires-clause. An abbreviated
    /// function template's invented parameters ([dcl.fct]/22) are template
    /// parameters too and come before the requires-clause, as in clang 19
    /// (the standard lists them after it, /3.3; GCC follows that).
    pub(crate) fn head_constraints(
        &self,
        params: kiln_ast::ids::ListId,
        requires: Option<ExprId>,
        out: &mut Vec<ExprId>,
    ) {
        for p in self.ast.lists.iter::<DeclId>(params) {
            if let Some(c) = self.ast.template_parm(p).and_then(|i| i.constraint) {
                out.push(c);
            }
        }
        if let Some(r) = requires {
            out.push(r);
        }
    }

    /// Checks the satisfaction of concept-id `c<args>` (`args` converted,
    /// sugared), memoized on the canonical arguments (§8.3).
    pub(crate) fn check_concept(&mut self, c: DeclId, args: ArgListId, loc: Loc) -> SatId {
        let _t = self.trace_satisfaction(loc);
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
        let _t = self.trace_satisfaction(loc);
        // A redeclared member template: the pattern's constraints with
        // the specialization's levels outside the template's own.
        let remapped;
        let levels = if let Some((_, outer)) = self.inst.concepts.member_constraints.get(&tmpl)
            && self.ast.template_info(tmpl).is_some()
            && let Some(own) = levels.levels.last()
        {
            let mut m = outer.clone();
            m.levels.push(*own);
            remapped = m;
            &remapped
        } else {
            levels
        };
        let key = (tmpl, self.ast.types.canonical_args(shown));
        let cacheable = levels.levels.len() == 1;
        if cacheable && let Some(&id) = self.inst.concepts.sat_cache.get(&key) {
            self.inst.concepts.counters.cache_hits += 1;
            return id;
        }
        // A constraint whose satisfaction needs itself (a constructor
        // template constrained on `__is_constructible` of its own class,
        // asked for the same arguments) is unsatisfied, as in clang's
        // `err_constraint_depends_on_self`, instead of recursing forever.
        if cacheable && self.inst.concepts.checking.contains(&key) {
            let d = self
                .diag(DiagId::sema_constraint_depends_on_self, self.expr_loc(exprs[0]))
                .arg(self.expr_text_as_written(exprs[0]));
            self.emit(d);
            return self.inst.concepts.push(Satisfaction { ok: false, records: Vec::new() });
        }
        self.inst.concepts.counters.count += 1;
        if cacheable {
            self.inst.concepts.checking.push(key);
        }
        let s = self.satisfy(tmpl, exprs, levels, shown, loc);
        if cacheable {
            self.inst.concepts.checking.pop();
        }
        let id = self.inst.concepts.push(s);
        if cacheable {
            self.inst.concepts.sat_cache.insert(key, id);
        }
        id
    }

    /// clang's `CheckConstraintSatisfaction` time-trace event (spec/driver.md
    /// "Time trace"; the detail is the template-id's location, cache hits
    /// included). A concept-id met while the enclosing check substitutes
    /// its atoms (the innermost frame past atom substitutions is that
    /// check's) records none:
    /// clang 22 normalizes it into the enclosing check.
    fn trace_satisfaction(&self, loc: Loc) -> Option<kiln_source::trace::Scope> {
        if !kiln_source::trace::enabled()
            || self
                .inst
                .stack
                .frames
                .iter()
                .rev()
                .find(|f| f.kind != FrameKind::ConstraintSubstitution)
                .is_some_and(|f| {
                    matches!(f.kind, FrameKind::ConstraintsCheck | FrameKind::ConceptCheck)
                })
        {
            return None;
        }
        Some(kiln_source::trace::Scope::with_detail("CheckConstraintSatisfaction", || {
            kiln_source::trace::range_detail(self.diags.sm(), loc, loc)
        }))
    }

    /// The satisfaction of the trailing requires-clause of non-template
    /// function `f` ([over.match.viable]/3), when `f` is a member of a
    /// class template specialization redeclared from a constrained
    /// member of the pattern, or a non-templated function whose clause
    /// was diagnosed but is still checked with no arguments, as in clang
    /// ([dcl.decl.general]/5; else `None`: nothing to check, or the
    /// arguments are dependent).
    pub(crate) fn check_function_constraints(&mut self, f: DeclId, loc: Loc) -> Option<SatId> {
        let &r = self.ast.templates.trailing_requires.get(f)?;
        if self.inst.concepts.untemplated_requires.contains(&f) {
            let none = MultiLevelArgs::new();
            return Some(self.check_constraints(f, &[r], &none, ArgListId::EMPTY, loc));
        }
        let (_, outer) = self.inst.concepts.member_constraints.get(&f)?;
        if self.levels_dependent(outer) {
            return None;
        }
        let outer = outer.clone();
        let shown = outer.levels.last().map_or(ArgListId::EMPTY, |l| l.args);
        Some(self.check_constraints(f, &[r], &outer, shown, loc))
    }

    /// A reference to non-template function `f` outside overload
    /// resolution requires its constraints satisfied
    /// ([expr.prim.id.general]/4, clang `DiagnoseUseOfDecl`'s
    /// `err_reference_to_function_with_unsatisfied_constraints`);
    /// whether it diagnosed.
    pub(crate) fn diagnose_unsatisfied_use(&mut self, f: DeclId, loc: Loc) -> bool {
        if self.ast.templates.trailing_requires.get(f).is_none() {
            return false;
        }
        let Some(sat) = self.check_function_constraints(f, loc) else { return false };
        if self.sat_ok(sat) {
            return false;
        }
        // The check itself was ill-formed (a hard error, already
        // diagnosed): clang's `CheckFunctionConstraints` fails and the
        // use is not diagnosed again (WP-1448).
        if self.sat_hard_error(sat) {
            return true;
        }
        let d =
            self.diag(DiagId::sema_reference_to_unsatisfied_function, loc).arg(self.decl_arg(f));
        let d = self.with_sat_notes(d, sat);
        self.emit(d);
        true
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
            // clang names the underlying template decl unqualified (the
            // written qualifier is not part of the checked template name).
            .arg(self.decl_name_str(self.ast.decls.get(tmpl).name()))
            .arg(self.template_args_desc(tmpl, args));
        let d = self.with_sat_notes(d, sat);
        self.emit(d);
        Err(())
    }

    /// The partial specialization of `tmpl` a partial specialization
    /// declaration with arguments `canonical` and template-head `head`
    /// redeclares: same arguments and equivalent constraints
    /// ([temp.spec.partial.general]/2, [temp.over.link]/6; constraints
    /// compared by equivalence, WP-116).
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
            self.ast.templates.partial(r).is_some_and(|p| {
                p.canonical == canonical && {
                    let other = self.head_signature(p.params);
                    other.len() == sig.len()
                        && other.iter().zip(&sig).all(|(&a, &b)| self.exprs_equivalent(a, b))
                }
            })
        })
    }

    /// Whether template-heads `a` and `b` have equivalent associated
    /// constraints ([temp.over.link]/6-7).
    pub(crate) fn head_signatures_equivalent(
        &self,
        a: kiln_ast::ids::TemplateParamListId,
        b: kiln_ast::ids::TemplateParamListId,
    ) -> bool {
        let (x, y) = (self.head_signature(a), self.head_signature(b));
        x.len() == y.len() && x.iter().zip(&y).all(|(&p, &q)| self.exprs_equivalent(p, q))
    }

    /// The associated constraints of template-head `head`
    /// ([temp.constr.decl]/3), in order.
    fn head_signature(&self, head: kiln_ast::ids::TemplateParamListId) -> Vec<ExprId> {
        let l = *self.ast.templates.param_list(head);
        let mut c = Vec::new();
        self.head_constraints(l.params, l.requires, &mut c);
        c
    }

    /// A satisfied result.
    pub(crate) fn satisfied_id(&mut self) -> SatId {
        self.inst.concepts.push(Satisfaction { ok: true, records: Vec::new() })
    }

    /// Whether unsatisfied `id` failed by a hard error diagnosed while
    /// checking it ([temp.constr.atomic]/3: an atomic constraint not of
    /// type `bool`, or not a constant expression): such a failure leaves
    /// no record to explain it.
    pub(crate) fn sat_hard_error(&self, id: SatId) -> bool {
        let s = self.inst.concepts.sat(id);
        !s.ok && s.records.is_empty()
    }

    /// Whether satisfaction `id` holds.
    pub(crate) fn sat_ok(&self, id: SatId) -> bool {
        self.inst.concepts.sat(id).ok
    }

    /// The function whose constraints `entity` holds: `entity` itself, or a
    /// function template's pattern. Its constraints are checked from the
    /// function (clang `CheckFunctionConstraints`), so its access as a
    /// member or friend applies: a hidden friend's requires-clause may name
    /// a private member (an iterator adaptor's hidden-friend
    /// `operator-`). Lookup is unaffected: a function adds no scope.
    fn constrained_function(&self, entity: DeclId) -> Option<DeclId> {
        match self.ast.template_info(entity) {
            Some(info) => info.pattern.filter(|&p| self.ast.decls.function(p).is_some()),
            None => self.ast.decls.function(entity).is_some().then_some(entity),
        }
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
        // Names in the constraints are looked up from the entity's
        // definition context, not from where satisfaction is checked
        // ([temp.constr.atomic]/3 substitutes into the template's
        // expression; [temp.dep.candidate]): libc++'s
        // `__unqualified_swappable_with` must see its poison-pill `swap`
        // when `swappable<T>` is checked from inside namespace std. A
        // function's are checked from the function itself
        // ([`Sema::constrained_function`]).
        let func = self.constrained_function(entity);
        let ctx = func.unwrap_or_else(|| self.ast.decls.get(entity).ctx().unwrap_or(self.ast.tu));
        // With concrete arguments that substitution is final: the
        // function is no dependent context for the source-location
        // builtins in it ([`Sema::srcloc_ctx`]).
        let final_fn = func.filter(|_| !self.levels_dependent(levels));
        let srcloc = std::mem::replace(&mut self.srcloc.satisfying, final_fn.map(|f| (f, None)));
        // Satisfaction checks access also while an explicit
        // specialization's candidates are deduced (clang 22, §6.9).
        let suppress = self.inst.suppress_access.replace(false);
        let (ok, mut records) = self.in_inst_context(ctx, |s| {
            let mut records = Vec::new();
            let mut ok = true;
            for &e in exprs {
                if !s.sat_expr(entity, e, levels, &mut records, 0) {
                    ok = false;
                    break;
                }
            }
            (ok, records)
        });
        self.inst.suppress_access.set(suppress);
        self.srcloc.satisfying = srcloc;
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
    /// ([temp.constr.atomic]/3), through the §8.3 `atom_cache`: an atom
    /// with the same canonical arguments is evaluated once across every
    /// concept and constrained template that reaches it (a change in
    /// result is ill-formed NDR, [temp.constr.atomic]/3). An evaluation
    /// that emitted an error, or whose arguments are dependent, is not
    /// cached, so its diagnostics are not lost on a later hit.
    fn sat_atom(
        &mut self,
        entity: DeclId,
        e: ExprId,
        levels: &MultiLevelArgs,
        recs: &mut Vec<SatRecord>,
    ) -> bool {
        if let Some(&(c, mask)) = self.inst.concepts.poisoned.last()
            && c == entity
        {
            // An atom of a concept checked with a partly failed parameter
            // mapping: one naming a failed parameter is not satisfied
            // (its substitution failed, [temp.constr.atomic]/3; the
            // failure's note is the concept-id's); a concept-id passes
            // the mapping on argument by argument. A concept-id of a form
            // that does not (a parameter pack: `C<T, T>` of
            // `template<class T, class... A> concept C`) is one atom for
            // this: naming a failed parameter, it is not satisfied, rather
            // than substituted with the failed argument (which left it
            // value-dependent; WP-1571).
            if self.is_concept_id_atom(e)
                && let Some(ok) = self.sat_concept_id_partial(entity, e, levels, None, recs)
            {
                return ok;
            }
            let level = self.template_param_depth(c);
            if self.used_template_params(e, level).is_none_or(|u| u & mask != 0) {
                return false;
            }
        }
        let key: Option<AtomKey> = (!self.levels_dependent(levels)).then(|| {
            let args =
                levels.levels.iter().map(|l| self.ast.types.canonical_args(l.args)).collect();
            (e, args)
        });
        if let Some(k) = &key
            && let Some((ok, r)) = self.inst.concepts.atom_cache.get(k)
        {
            self.inst.concepts.counters.atom_hits += 1;
            recs.extend(r.iter().cloned());
            return *ok;
        }
        self.inst.concepts.counters.atoms += 1;
        let errors = self.errors_so_far();
        let mut mine = Vec::new();
        let ok = self.sat_atom_uncached(entity, e, levels, &mut mine);
        if let Some(k) = key
            && self.errors_so_far() == errors
        {
            self.inst.concepts.atom_cache.insert(k, (ok, mine.clone()));
        }
        recs.extend(mine);
        ok
    }

    /// What a trailing requires-clause of `entity` may name besides
    /// template parameters: the pattern function whose parameters are in
    /// scope ([basic.scope.param]/1), if it has any, and the type of
    /// `this` in a non-static member function ([expr.prim.this]/2).
    /// Both `None` without a trailing requires-clause.
    fn constraint_function_scope(&mut self, entity: DeclId) -> (Option<DeclId>, Option<QualType>) {
        let pattern = match self.inst.concepts.member_constraints.get(&entity) {
            Some(&(p, _)) => p,
            None => entity,
        };
        let function_of = |s: &Self, d: DeclId| match s.kind_of(d) {
            kiln_ast::decl::DeclKind::FunctionTemplate => s.templated_function(d),
            k if k.is_function() => Some(d),
            _ => None,
        };
        let Some(f) = function_of(self, pattern) else { return (None, None) };
        if self.ast.templates.trailing_requires.get(f).is_none() {
            return (None, None);
        }
        let params = self.ast.decls.function(f).map_or(0, |x| self.ast.lists.len(x.params));
        let this_ty = function_of(self, entity).and_then(|g| self.method_this_type(g));
        ((params > 0).then_some(f), this_ty)
    }

    /// The context pairs a constraint of `entity` is substituted with. A
    /// member of a class template specialization: names of the pattern
    /// class are looked up in the specialization. A friend defined in one
    /// (a hidden friend of a member class such as `view<V>::iterator`,
    /// [temp.friend]/1) maps the classes of the class that declared it,
    /// its lexical context, so the members of the enclosing
    /// specializations it names resolve ([temp.dep.type]).
    fn constraint_context_pairs(&mut self, entity: DeclId) -> Vec<(DeclId, DeclId)> {
        let class = self.inst_lexical_class(entity);
        if self.kind_of(class) == kiln_ast::decl::DeclKind::CXXRecord {
            self.inst_context_pairs(class)
        } else {
            Vec::new()
        }
    }

    /// The pattern call operator and `entity`'s, when `entity` is (the
    /// template of) a call operator substituted from a lambda with
    /// init-captures, which its constraints may name (P2036).
    pub(crate) fn constraint_lambda_captures(&self, entity: DeclId) -> Option<(DeclId, DeclId)> {
        let &(pattern, _) = self.inst.concepts.member_constraints.get(&entity)?;
        let function_of = |d: DeclId| match self.kind_of(d) {
            kiln_ast::decl::DeclKind::FunctionTemplate => self.templated_function(d),
            k if k.is_function() => Some(d),
            _ => None,
        };
        let (p, n) = (function_of(pattern)?, function_of(entity)?);
        let l = self.lambda_of_call_op(p)?;
        self.ast.exprs.lambdas.get(l).captures.iter().any(|c| c.init).then_some((p, n))
    }

    /// Errors emitted or captured by the enclosing SFINAE trap so far.
    fn errors_so_far(&self) -> u64 {
        let trapped = self.inst.sfinae.borrow().last().map_or(0, |s| s.errors);
        u64::from(self.diags.error_count()) + u64::from(trapped)
    }

    fn sat_atom_uncached(
        &mut self,
        entity: DeclId,
        e: ExprId,
        levels: &MultiLevelArgs,
        recs: &mut Vec<SatRecord>,
    ) -> bool {
        // An atom diagnosed where it was written (a non-`bool` atom in a
        // requires-clause) is not satisfied, silently.
        if self.has_errors_in(e) {
            return false;
        }
        let loc = self.begin_loc(e);
        let r = self.constraint_subst(entity, levels, loc, |sub| sub.expr(e).ok());
        let Some((r, st)) = r else {
            return false;
        };
        let x = match (r, st) {
            (Some(x), Ok(())) => x,
            (_, st) => {
                let first = st.err().flatten();
                // A construct the substituter does not handle yet is a
                // hard error: assuming either answer could accept an
                // invalid program or pick the wrong overload.
                if let Some(d) = first
                    .as_ref()
                    .filter(|d| d.id() == Some(DiagId::sema_unimplemented_template_feature))
                {
                    self.emit(d.clone());
                    return false;
                }
                if let Some(ok) = self.sat_concept_id_partial(entity, e, levels, Some(loc), recs) {
                    return ok;
                }
                let at = first.as_ref().map_or(loc, Diag::loc);
                let level = self.diags.clang_compat();
                let msg = first.map(|d| format!(": {}", d.message_at(level))).unwrap_or_default();
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
            // Dependent arguments (an enclosing template's parameters are
            // retained): checked again when instantiated, as clang treats
            // constraints with dependent arguments as satisfied.
            if self.levels_dependent(levels) {
                return true;
            }
            // Concrete arguments but a dependent result: a substitution
            // the substituter does not complete.
            let d = self
                .diag(DiagId::sema_unimplemented_template_feature, loc)
                .arg("an atomic constraint that stays value-dependent after substitution");
            self.emit(d);
            return false;
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

    /// Whether the constraint-expression `e` of a nested requirement
    /// ([expr.prim.req.nested]/1) is satisfied for `levels` when it did not
    /// substitute as a whole: it is checked by the rules of satisfaction
    /// ([temp.constr.constr]), atom by atom, so a concept-id whose failed
    /// argument no atom names is satisfied (`requires C<T>;` with an
    /// invalid default argument of `C` its atoms do not use; clang 22
    /// and GCC 16.1 agree). `body` is the requires-expression's body.
    pub(crate) fn sat_nested_requirement(
        &mut self,
        body: DeclId,
        e: ExprId,
        levels: &MultiLevelArgs,
    ) -> bool {
        let mut recs = Vec::new();
        self.sat_expr(body, e, levels, &mut recs, 0)
    }

    /// Whether atom `e` is a concept-id of a concept (not of a concept
    /// template parameter).
    fn is_concept_id_atom(&self, e: ExprId) -> bool {
        let x = self.ignore_paren_imp_casts(e);
        let n = *self.ast.exprs.get(x);
        n.kind() == ExprKind::ConceptSpecializationExpr
            && DeclId::from_raw(n.a()).is_some_and(|c| {
                self.kind_of(self.ast.first(c)) == kiln_ast::decl::DeclKind::Concept
            })
    }

    /// The atom `e`, a concept-id `C<A...>` of `entity`'s constraints,
    /// whose arguments do not all substitute ([temp.constr.normal]/1.4,
    /// [temp.constr.atomic]/3). The normal form of the concept-id is
    /// `C`'s constraint with `A...` substituted into the parameter
    /// mapping of each atom, which holds only the parameters that atom
    /// names ([temp.constr.atomic]/1), so a failed argument makes only
    /// the atoms naming its parameter unsatisfied: `Void<T> || A42<U>`
    /// with `U := void&` is satisfied by its left operand (clang 22 and
    /// GCC 16.1 check the mapping atom by atom as well). Each argument is
    /// substituted on its own (one naming a parameter that already failed
    /// fails too, unsubstituted), then `C`'s constraint is checked with
    /// the failed parameters marked. On failure the substitution note
    /// comes first, then the notes of the other false atoms, as clang's.
    /// A defaulted argument is substituted from the concept's default
    /// with the arguments before it ([temp.arg.general]/9), so an invalid
    /// default makes only the atoms naming its parameter unsatisfied
    /// (`template<class T, class U = typename T::type> concept C = true`
    /// is satisfied by `C auto` for `int`, GCC 16.1; clang 22 rejects).
    /// A trailing parameter pack takes the remaining arguments as one
    /// group: when one fails, or names a failed parameter of the
    /// enclosing concept, the atoms naming the pack are unsatisfied and
    /// the others are evaluated (WP-1579).
    /// `None` when no argument fails on its own, or the concept-id has a
    /// form this does not cover (a non-type parameter, a pack expansion
    /// before the pack's position): the caller keeps the whole-atom
    /// answer. Only reached after a substitution failure, so the
    /// satisfied path is unchanged.
    fn sat_concept_id_partial(
        &mut self,
        entity: DeclId,
        e: ExprId,
        levels: &MultiLevelArgs,
        loc: Option<Loc>,
        recs: &mut Vec<SatRecord>,
    ) -> Option<bool> {
        use kiln_ast::decl::DeclKind;
        if !self.is_concept_id_atom(e) {
            return None;
        }
        let x = self.ignore_paren_imp_casts(e);
        let n = *self.ast.exprs.get(x);
        let c = self.ast.first(DeclId::from_raw(n.a())?);
        let written: Vec<TemplateArg> = self.ast.types.args(ArgListId::from_raw(n.b())?).to_vec();
        let params = self.template_params(c).clone();
        // The parameters before a trailing pack take one argument each,
        // written or defaulted; the pack takes the rest, substituted as
        // one group (expanding a pack expansion among them).
        let pack =
            params.last().is_some_and(|&p| self.ast.template_parm(p).is_some_and(|i| i.pack));
        let fixed = params.len() - usize::from(pack);
        if params.len() > 64
            || written.len() > 64
            || (!pack && written.len() > params.len())
            || params.iter().any(|&p| self.kind_of(p) == DeclKind::NonTypeTemplateParm)
            || params[..fixed].iter().any(|&p| self.ast.template_parm(p).is_some_and(|i| i.pack))
            || written.iter().take(fixed).any(|&a| self.is_pack_expansion_arg(a))
            || params[written.len().min(fixed)..fixed]
                .iter()
                .any(|&p| self.template_default(p).is_none())
        {
            return None;
        }
        let ce = self.ast.template_info(c).and_then(|i| i.constraint)?;
        let at = loc.unwrap_or_else(|| self.begin_loc(e));
        let outer = self.inst.concepts.poisoned.last().copied().filter(|&(p, _)| p == entity);
        let outer_level = outer.map(|(p, _)| self.template_param_depth(p));
        let poisoned = |s: &Self, a: TemplateArg| match (outer, outer_level) {
            (Some((_, m)), Some(level)) => {
                s.used_template_params_in_arg(a, level).is_none_or(|u| u & m != 0)
            }
            _ => false,
        };
        let depth = self.template_param_depth(c);
        let mut mask = 0u64;
        let mut first: Option<Diag> = None;
        let mut args = Vec::with_capacity(params.len());
        for (i, &p) in params.iter().enumerate() {
            let group: &[TemplateArg] = if i < fixed {
                written.get(i).map_or(&[], std::slice::from_ref)
            } else {
                written.get(fixed..).unwrap_or(&[])
            };
            let r = if i < fixed && group.is_empty() {
                // A default argument ([temp.arg.general]/9) is substituted
                // with the concept's arguments before it; one naming a
                // failed parameter fails too.
                let def = self.template_default(p)?;
                if self.used_template_params_in_arg(def, depth).is_none_or(|u| u & mask != 0) {
                    mask |= 1 << i;
                    args.push(TemplateArg::NULL);
                    continue;
                }
                let prior = args.clone();
                self.in_sfinae(FrameKind::ConstraintSubstitution, c, None, at, |s| {
                    s.default_template_arg(c, depth, p, def, at, &prior).ok()
                })
            } else {
                if group.iter().any(|&a| poisoned(self, a)) {
                    mask |= 1 << i;
                    args.push(TemplateArg::NULL);
                    continue;
                }
                self.subst_concept_arg_group(entity, levels, at, group, i >= fixed)
            };
            match r {
                Some((Some(a), Ok(()))) => args.push(a),
                Some((_, st)) => {
                    if first.is_none() {
                        first = st.err().flatten();
                    }
                    mask |= 1 << i;
                    args.push(TemplateArg::NULL);
                }
                None => return Some(false),
            }
        }
        if mask == 0 {
            return None;
        }
        let c_levels = self.prior_levels(c, depth, &args);
        let shown = self.ast.types.arg_list(&args);
        self.inst.concepts.poisoned.push((c, mask));
        let s = self.satisfy(c, &[ce], &c_levels, shown, at);
        self.inst.concepts.poisoned.pop();
        if s.ok {
            return Some(true);
        }
        if let Some(d) = first {
            let level = self.diags.clang_compat();
            recs.push(SatRecord {
                id: DiagId::note_substituted_constraint_ill_formed,
                loc: d.loc(),
                select: false,
                args: vec![format!(": {}", d.message_at(level))],
                nested: None,
            });
        }
        recs.extend(s.records);
        Some(false)
    }

    /// The written arguments `group` of one parameter of a concept-id in
    /// [`Self::sat_concept_id_partial`], substituted with `levels`: one
    /// argument, or (`pack`) the trailing pack's group as one pack
    /// argument (a group written as a single pack stays that pack).
    fn subst_concept_arg_group(
        &mut self,
        entity: DeclId,
        levels: &MultiLevelArgs,
        at: Loc,
        group: &[TemplateArg],
        pack: bool,
    ) -> Option<SubstOutcome<TemplateArg>> {
        let list = self.ast.types.arg_list(group);
        let r = self.constraint_subst(entity, levels, at, |sub| sub.args(list, at).ok());
        r.map(|(l, st)| {
            let l = l.and_then(|l| {
                let xs = self.ast.types.args(l);
                if pack {
                    match (group, xs) {
                        ([g], [x])
                            if matches!(g.view(), ArgView::Pack(_))
                                && matches!(x.view(), ArgView::Pack(_)) =>
                        {
                            Some(*x)
                        }
                        _ => Some(TemplateArg::pack(l)),
                    }
                } else {
                    (xs.len() == 1).then(|| xs[0])
                }
            });
            (l, st)
        })
    }

    /// Runs `f` on a [`Substituter`] for the constraints of `entity`
    /// under `levels`, in a SFINAE frame (the setup an atom's substitution
    /// needs: the function parameters and `this` a trailing
    /// requires-clause may name, the specialization's context pairs, the
    /// lookup context and access of the template).
    fn constraint_subst<R>(
        &mut self,
        entity: DeclId,
        levels: &MultiLevelArgs,
        loc: Loc,
        f: impl FnOnce(&mut Substituter<'_, '_, '_>) -> Option<R>,
    ) -> Option<SubstOutcome<R>> {
        let (params_of, this_ty) = self.constraint_function_scope(entity);
        let pairs = self.constraint_context_pairs(entity);
        if let Some(t) = this_ty {
            self.nsdmi_this.push((entity, t));
        }
        let access =
            self.inst.concepts.partial_access.filter(|&(p, _)| p == entity).map(|(_, c)| c);
        let lambdas: Vec<_> = self
            .constraint_lambda_captures(entity)
            .into_iter()
            .flat_map(|(p, n)| [p, n])
            .filter_map(|f| self.lambda_of_call_op(f))
            .collect();
        let pushed = lambdas.len();
        self.inst.concepts.lambda_constraint.extend(lambdas);
        let run = |s: &mut Self| {
            let saved = s.access_ctx;
            s.access_ctx = access.or(saved);
            let r = s.in_sfinae(FrameKind::ConstraintSubstitution, entity, None, loc, |s| {
                let mut sub = Substituter::new(s, levels);
                sub.map_contexts(&pairs);
                if let Some(f) = params_of {
                    sub.seed_function_params(f).ok()?;
                }
                sub.seed_init_captures(entity);
                f(&mut sub)
            });
            s.access_ctx = saved;
            r
        };
        // The names of a concept or a namespace-scope template's
        // constraints are looked up again from where they were written,
        // not from where satisfaction is checked ([temp.point]/1,
        // [temp.dep.candidate]): `swap(a, b)` in libc++'s
        // `ranges::__swap` concepts must find the deleted poison pill, not
        // `std::swap`, when checked from namespace `std`. A function's are
        // looked up there too, but from the function itself (its access).
        let r = match self.ast.decls.get(entity).ctx() {
            Some(c)
                if this_ty.is_none() && self.kind_of(c) == kiln_ast::decl::DeclKind::Namespace =>
            {
                self.in_inst_context(self.constrained_function(entity).unwrap_or(c), run)
            }
            _ => run(self),
        };
        let kept = self.inst.concepts.lambda_constraint.len() - pushed;
        self.inst.concepts.lambda_constraint.truncate(kept);
        if this_ty.is_some() {
            self.nsdmi_this.pop();
        }
        r
    }

    /// True if substituting `levels` leaves template parameters behind: a
    /// retained level or a dependent argument.
    pub(crate) fn levels_dependent(&self, levels: &MultiLevelArgs) -> bool {
        levels.levels.iter().any(|l| {
            l.mode == LevelMode::Retain
                || self.ast.types.args(l.args).iter().any(|&a| {
                    self.ast.types.arg_flags(a).intersects(
                        TypeFlags::DEPENDENT_INSTANTIATION | TypeFlags::INSTANTIATION_DEPENDENT,
                    )
                })
        })
    }

    /// The records of a `&&` / `||` chain left in a substituted atom
    /// (`false_atom_records`).
    fn logical_chain_records(&mut self, inner: ExprId) -> Vec<SatRecord> {
        // Clang recurses into both sides of a false `&&` (into the
        // right one only when the left is true) and of a false
        // `||`: every false operand of the chain, in order. The
        // chain of one operator is walked iteratively (a fold
        // over a long pack nests deeply).
        let op = self.ast.exprs.binary_op(inner);
        let mut leaves = Vec::new();
        let mut stack = vec![inner];
        while let Some(e) = stack.pop() {
            let e = self.ignore_paren_imp_casts(e);
            if self.ast.exprs.get(e).kind() == ExprKind::BinaryOperator
                && self.ast.exprs.binary_op(e) == op
            {
                let [l, r] = self.ast.exprs.operands(e);
                stack.push(r);
                stack.push(l);
            } else {
                leaves.push(e);
            }
        }
        let mut recs = Vec::new();
        for e in leaves {
            if op == BinaryOp::LOr || self.fold_int(e).is_none_or(|v| v == 0) {
                recs.extend(self.false_atom_records(e));
            }
        }
        recs
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
                            // clang 19 quotes a type argument, not a
                            // value; clang 22 quotes both.
                            args: vec![
                                // A type is quoted with its `aka` (`'decltype(1)'
                                // (aka 'int')`, [temp.param]/5).
                                if let ArgView::Type(t) = a.view() {
                                    self.ty_arg(t)
                                } else if self.clang_compat >= 22
                                    || matches!(
                                        a.view(),
                                        ArgView::Type(_)
                                            | ArgView::Template(_)
                                            | ArgView::TemplateExpansion { .. }
                                    )
                                {
                                    format!("'{}'", self.arg_str(a))
                                } else {
                                    self.arg_str(a)
                                },
                                self.decl_name(c),
                            ],
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
            // A `&&` or `||` left in the substituted atom (an expanded fold
            // over a pack the normal form does not split, such as an
            // enclosing template's): the operands that made it false, as
            // clang's `diagnoseWellFormedUnsatisfiedConstraintExpr` ("and"
            // after the first).
            ExprKind::BinaryOperator
                if matches!(self.ast.exprs.binary_op(inner), BinaryOp::LAnd | BinaryOp::LOr) =>
            {
                self.logical_chain_records(inner)
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
                let mut recs = vec![SatRecord {
                    id: DiagId::note_atomic_constraint_false,
                    loc: at,
                    select: true,
                    args: vec![self.expr_text(inner)],
                    nested: None,
                }];
                recs.extend(self.trait_explanation(inner, at));
                recs
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

    /// `e` as written (clang's `printPretty` with the default policy,
    /// as an expression requirement's substitution failure prints it).
    pub(crate) fn expr_text_as_written(&self, e: ExprId) -> String {
        let mut s = String::new();
        kiln_ast::types::TypeNames::write_expr(
            &kiln_ast::dump::AstNames::new(&self.ast),
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
        // Dependent, not merely instantiation-dependent, arguments defer
        // the check (clang `anyDependentTemplateArguments`): `C<TC<N>>`
        // with `N` a constant local of a function template is checked at
        // the definition (spec/sema-templates.md §5.3).
        let dep = self.ast.types.arg_list_flags(written).intersects(TypeFlags::DEPENDENT)
            || args.iter().any(|&a| self.is_pack_expansion_arg(a));
        if dep {
            // Checked against the parameters even so (clang
            // `CheckTemplateArgumentList`; spec/sema-templates.md §3.2).
            self.check_dependent_template_args_at(c, &args, arg_locs, loc)?;
            return Ok(self.builder().concept_spec_expr(c, written, QualType::BOOL, loc));
        }
        let conv = self.check_template_args(c, &args, arg_locs, loc)?;
        // A concept whose definition was diagnosed (a non-`bool` atom) is
        // invalid: naming it is an error expression, already diagnosed
        // (clang's invalid `ConceptDecl`).
        if self
            .ast
            .template_info(c)
            .and_then(|i| i.constraint)
            .is_some_and(|x| self.has_errors_in(x))
        {
            return Ok(self.act_on_error_expr(&[], loc));
        }
        let e = self.builder().concept_spec_expr(c, written, QualType::BOOL, loc);
        // Satisfaction depends on the canonical arguments only: sugar
        // naming a function template's block-scope constants
        // (instantiation-dependent, not dependent) is dropped, so the
        // atoms see concrete arguments ([temp.constr.atomic]/3).
        let args = self.settle_inst_sugar(conv.sugared);
        let sat = self.check_concept(c, args, loc);
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
        // `C<args>` with `C` a concept template parameter (P2841R7).
        if let Some(r) = self.ttp_template_id(e, c, loc) {
            return r;
        }
        let nl = self.args(l, loc)?;
        let at = self.sema.ast.exprs.get(e).loc();
        let r = self
            .sema
            .build_concept_id(c, nl, &[], at)
            .map_err(|()| crate::templ::subst::SubstFail)?;
        // The written qualifier stays with the concept-id (it prints in
        // a failed `static_assert`'s requirement).
        if let Some(&q) = self.sema.ast.templates.expr_nns.get(e) {
            let q = self.nns(q, loc)?;
            self.sema.set_template_id_qualifier(r, q);
        }
        Ok(r)
    }
}
