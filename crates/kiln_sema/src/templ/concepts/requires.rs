//! Requires-expressions (spec/sema-templates.md §8.6, [expr.prim.req]):
//! the requirements are substituted in order, each in a SFINAE context,
//! and checking stops at the first that fails
//! ([expr.prim.req.general]/5). The result is a non-dependent
//! `RequiresExpr` whose satisfaction (and the notes explaining a
//! failure) is recorded.

use kiln_ast::decl::ParamData;
use kiln_ast::expr::{ExprDeps, ValueCategory};
use kiln_ast::ids::{ArgListId, RawId};
use kiln_ast::templates::Requirement;
use kiln_ast::types::{TemplateArg, TypeFlags};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use super::DeferredRequires;
use super::{SatRecord, Satisfaction};
use crate::Sema;
use crate::templ::args::LevelMode;
use crate::templ::pack::Expansion;
use crate::templ::stack::{FrameKind, SfinaeState};
use crate::templ::subst::{DeclOrPack, SubstFail, SubstResult, Substituter};

impl Substituter<'_, '_, '_> {
    /// Runs `f` in a SFINAE context (§5.7): `None` when the frame could
    /// not be pushed, else `f`'s result and the first error captured.
    fn sfinae<R>(
        &mut self,
        entity: DeclId,
        loc: Loc,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<(R, Result<(), Option<Diag>>)> {
        self.sfinae_as(FrameKind::ConstraintSubstitution, entity, loc, f)
    }

    /// [`Self::sfinae`] under a frame of `kind`.
    fn sfinae_as<R>(
        &mut self,
        kind: FrameKind,
        entity: DeclId,
        loc: Loc,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<(R, Result<(), Option<Diag>>)> {
        if !self.sema.push_inst_frame(kind, entity, entity, loc) {
            return None;
        }
        if let Some(fr) = self.sema.inst.stack.frames.last_mut() {
            fr.sfinae = Some(true);
        }
        self.sema.inst.sfinae.borrow_mut().push(SfinaeState::default());
        let r = f(self);
        let st = self.sema.inst.sfinae.borrow_mut().pop().unwrap_or_default();
        self.sema.pop_inst_frame();
        let failed = if st.errors > 0 { Err(st.first) } else { Ok(()) };
        Some((r, failed))
    }

    /// Substitutes the parameters `params` of a requires-expression with
    /// body `body`, recording a failure in `recs`: the new parameters and
    /// whether every one substituted.
    fn requires_params(
        &mut self,
        body: DeclId,
        params: &[DeclId],
        recs: &mut Vec<SatRecord>,
    ) -> (Vec<DeclId>, bool) {
        let mut new_params = Vec::with_capacity(params.len());
        let mut ok = true;
        for &p in params {
            let n = *self.sema.ast.decls.get(p);
            let ty = n.ty().unwrap_or(QualType::ERROR);
            // The function-scope depth stays (`ParamData::scope_depth`).
            let depth =
                self.sema.ast.decls.param(p).map_or(0, |x| x.flags & ParamData::SCOPE_DEPTH_MASK);
            let pack = self
                .sema
                .ast
                .decls
                .param(p)
                .is_some_and(|x| x.flags & ParamData::PACK != 0)
                .then(|| crate::templ::pack::type_pattern(self.sema, ty))
                .flatten();
            if let Some(pat) = pack {
                // A parameter pack ([temp.variadic]/5): one parameter per
                // element, or still a pack when its expansion names a
                // retained level (§7.2, `requires(T... t, U... u)` in a
                // member template with only `T` known).
                let r = self.sfinae(body, n.loc(), |s| {
                    s.expand_pattern(|s| s.element_ty(pat, n.loc())).ok()
                });
                match r {
                    Some((Some(Expansion::Elements(v)), Ok(()))) => {
                        let mut decls = Vec::with_capacity(v.len());
                        for t in v {
                            let idx = u16::try_from(new_params.len()).unwrap_or(u16::MAX);
                            let np =
                                self.sema.builder().param(body, n.name(), n.loc(), t, idx, depth);
                            decls.push(np);
                            new_params.push(np);
                        }
                        let l = self.sema.ast.lists.push(&decls);
                        self.locals.insert(p, DeclOrPack::Pack(l));
                        continue;
                    }
                    Some((Some(Expansion::Unexpanded(x)), Ok(()))) => {
                        let t = self.sema.ast.types.pack_expansion(x, None);
                        let idx = u16::try_from(new_params.len()).unwrap_or(u16::MAX);
                        let np = self.sema.builder().param(
                            body,
                            n.name(),
                            n.loc(),
                            t,
                            idx,
                            ParamData::PACK | depth,
                        );
                        self.locals.insert(p, DeclOrPack::Decl(np));
                        new_params.push(np);
                        continue;
                    }
                    Some((_, st)) => {
                        let first = st.err().flatten();
                        recs.push(invalid_record(
                            self.sema,
                            first.as_ref(),
                            n.loc(),
                            String::new(),
                        ));
                        ok = false;
                        break;
                    }
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            let r = self.sfinae(body, n.loc(), |s| s.ty(ty, n.loc()).ok());
            match r {
                Some((Some(t), Ok(()))) => {
                    let idx = u16::try_from(new_params.len()).unwrap_or(u16::MAX);
                    let np = self.sema.builder().param(body, n.name(), n.loc(), t, idx, depth);
                    self.locals.insert(p, DeclOrPack::Decl(np));
                    new_params.push(np);
                }
                Some((_, st)) => {
                    let first = st.err().flatten();
                    recs.push(invalid_record(self.sema, first.as_ref(), n.loc(), String::new()));
                    ok = false;
                    break;
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        (new_params, ok)
    }

    /// Substitutes into requires-expression `e` and evaluates it.
    pub(crate) fn requires_expr(&mut self, e: ExprId, loc: Loc) -> SubstResult<ExprId> {
        if let Some(d) = self.sema.inst.concepts.deferred_requires.get(&e).cloned() {
            return self.deferred_requires_expr(&d, loc);
        }
        let Some(rq) = self.sema.ast.requires_of(e).copied() else { return Err(SubstFail) };
        let items: Vec<Requirement> = self.sema.ast.templates.requirements(&rq).to_vec();
        let params: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(rq.params).collect();
        let at = self.sema.ast.exprs.get(e).loc();
        let rbrace = self.sema.ast.exprs.cold.end_loc.get(e).copied().unwrap_or(at);
        let mut recs = Vec::new();
        // The requirements are unevaluated operands
        // ([expr.prim.req.general]/2), as in `act_on_requires_expr_start`.
        self.sema.enter_unevaluated();
        // Checking a function's constraints from the function, the
        // requirements' source-location builtins are in the body
        // ([`Sema::srcloc_ctx`]).
        let srcloc = self.sema.srcloc.satisfying;
        if let Some((f, _)) = srcloc
            && f == self.sema.ctx
        {
            self.sema.srcloc.satisfying = Some((f, Some(rq.body)));
        }
        let (new_params, mut ok) = self.requires_params(rq.body, &params, &mut recs);
        // A requirement left dependent (it names a retained level, §7.2)
        // is not checked: the requires-expression stays dependent with
        // the substituted requirements (clang's `TransformRequiresExpr`).
        let mut dependent = false;
        let mut new_reqs = Vec::with_capacity(items.len());
        if ok {
            let n = items.len();
            for (i, &r) in items.iter().enumerate() {
                let was = dependent;
                if let Some(r) = self.requirement(rq.body, r, at, &mut recs, &mut dependent) {
                    new_reqs.push(r);
                } else {
                    ok = false;
                    break;
                }
                // The first requirement left dependent decides nothing
                // yet: the ones after it are substituted only once it is
                // known ([expr.prim.req.general]/5), with the final
                // arguments (`typename U::type; typename S<T>::type;` in a
                // generic lambda of `f<int>`: `S<int>` is not formed when
                // `U::type` fails).
                if !was && dependent && i + 1 < n && self.can_defer_requires(e) {
                    self.sema.exit_unevaluated();
                    self.sema.srcloc.satisfying = srcloc;
                    return Ok(self.defer_requires(e, &rq, at, rbrace));
                }
            }
        }
        // The pattern's requirements that passed an access check only
        // speculatively are checked again here, as substitution failures
        // with no note (clang `TransformRequiresExpr`: the body's
        // `PerformDependentDiagnostics` under a SFINAE trap, then
        // `setSatisfied(false)`; spec/sema-templates.md §4.8). Still
        // speculative (a partial substitution): left dependent.
        let mut pending = false;
        if ok && !dependent && self.sema.requires_access_pending(rq.body) {
            match self.sfinae(rq.body, at, |s| s.sema.replay_requires_access(rq.body)) {
                Some((Some(true), Ok(()))) => {}
                Some((None, Ok(()))) => pending = true,
                _ => ok = false,
            }
        }
        self.sema.exit_unevaluated();
        self.sema.srcloc.satisfying = srcloc;
        if ok && (dependent || pending) {
            let x = self.sema.builder().requires_expr(
                rq.body,
                &new_params,
                &new_reqs,
                QualType::BOOL,
                at,
                rbrace,
            );
            if pending {
                self.sema.ast.exprs.mark_value_dependent(x);
            }
            return Ok(x);
        }
        // An unsatisfied requirement settles the value even when a local
        // parameter's type still names a retained level
        // (`requires(T... t, decltype(u)... v)` in a generic lambda): the
        // result keeps no such parameter, which would make it dependent
        // and re-substituted to `true` with no requirement left.
        let dep_param = |s: &Self, p: DeclId| {
            s.sema.ast.decls.get(p).ty().is_some_and(|t| {
                s.sema.ast.types.qflags(t).intersects(TypeFlags::DEPENDENT_INSTANTIATION)
            })
        };
        let kept: &[DeclId] =
            if !ok && new_params.iter().any(|&p| dep_param(self, p)) { &[] } else { &new_params };
        let x = self.sema.builder().requires_expr(rq.body, kept, &[], QualType::BOOL, at, rbrace);
        let _ = self.sema.ast.exprs.set_satisfied(x, ok);
        if !ok {
            let id = self.sema.inst.concepts.push(Satisfaction { ok, records: recs });
            self.sema.inst.concepts.expr_sat.insert(x, id);
        }
        let _ = loc;
        Ok(x)
    }

    /// Whether requires-expression `e`, substituted partially (some level
    /// retained), may be left as its pattern for the final substitution
    /// ([`DeferredRequires`]; a requirement stayed dependent, so some
    /// level is still to come). Under an expansion substituted element by
    /// element (a generic lambda in a fold or a pack-expanded
    /// initializer), each element defers by itself with its index
    /// ([`DeferredRequires::pack`]); not while it names a pack of a
    /// partially substituted expansion that is kept unexpanded (§7.2).
    fn can_defer_requires(&self, e: ExprId) -> bool {
        self.pack_index.is_some()
            || !self.sema.ast.exprs.get(e).deps().contains(ExprDeps::UNEXPANDED_PACK)
    }

    /// A copy of pattern `e` (requirements `rq`), value-dependent, recorded
    /// with this substitution's levels, locals and context pairs.
    fn defer_requires(
        &mut self,
        e: ExprId,
        rq: &kiln_ast::templates::RequiresData,
        at: Loc,
        rbrace: Loc,
    ) -> ExprId {
        let items: Vec<Requirement> = self.sema.ast.templates.requirements(rq).to_vec();
        let params: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(rq.params).collect();
        let x =
            self.sema.builder().requires_expr(rq.body, &params, &items, QualType::BOOL, at, rbrace);
        self.sema.ast.exprs.mark_value_dependent(x);
        if self.pack_index.is_some() {
            // The packs its requirements name are this element's
            // ([`DeferredRequires::pack`]): the expansion is complete.
            self.sema.ast.exprs.clear_unexpanded_pack(x);
        }
        let mut locals: Vec<(DeclId, DeclOrPack)> =
            self.locals.iter().map(|(&k, &v)| (k, v)).collect();
        locals.sort_unstable_by_key(|&(k, _)| k.raw());
        let d = DeferredRequires {
            pattern: e,
            levels: self.args.clone(),
            locals,
            pairs: self.pairs.clone(),
            pack: self.pack_index.map(|i| (i, self.pack.expanding.as_ref().map_or(0, |e| e.0))),
        };
        self.sema.inst.concepts.deferred_requires.insert(x, d);
        x
    }

    /// The final substitution of a requires-expression left as its pattern
    /// by a partial one ([`DeferredRequires`]): the pattern with the saved
    /// levels, their retained levels and the deeper ones this
    /// substitution's, the saved locals mapped on through this
    /// substitution's. When the levels do not line up (more retained
    /// levels than this substitution has), the two substitutions run one
    /// after the other, as without the deferral.
    fn deferred_requires_expr(&mut self, d: &DeferredRequires, loc: Loc) -> SubstResult<ExprId> {
        let retained = d.levels.levels.iter().filter(|l| l.mode == LevelMode::Retain).count();
        let pairs: Vec<(DeclId, DeclId)> = d.pairs.iter().chain(&self.pairs).copied().collect();
        if retained > self.args.levels.len() {
            let mid = {
                let mut s = Substituter::new(&mut *self.sema, &d.levels);
                s.locals = d.locals.iter().copied().collect();
                s.pairs.clone_from(&pairs);
                s.entity = self.entity;
                s.set_deferred_pack(d.pack);
                s.requires_expr(d.pattern, loc)?
            };
            return self.expr(mid);
        }
        // The levels deeper than the saved ones (a generic lambda's own,
        // one level shallower once the enclosing levels are substituted)
        // follow them.
        let mut levels = d.levels.clone();
        let mut now = self.args.levels.iter();
        for l in &mut levels.levels {
            if l.mode == LevelMode::Retain
                && let Some(&n) = now.next()
            {
                *l = n;
            }
        }
        levels.levels.extend(now.copied());
        let mut locals = self.locals.clone();
        for &(k, v) in &d.locals {
            let v = match v {
                DeclOrPack::Decl(m) => self.locals.get(&m).copied().unwrap_or(v),
                DeclOrPack::Pack(l) => {
                    let elems: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(l).collect();
                    let mapped: Vec<DeclId> = elems
                        .iter()
                        .map(|m| match self.locals.get(m) {
                            Some(&DeclOrPack::Decl(f)) => f,
                            _ => *m,
                        })
                        .collect();
                    if mapped == elems {
                        v
                    } else {
                        DeclOrPack::Pack(self.sema.ast.lists.push(&mapped))
                    }
                }
            };
            locals.insert(k, v);
        }
        let entity = self.entity;
        let final_subst = self.final_subst;
        let lambda_bodies = self.lambda_bodies;
        let mut s = Substituter::new(&mut *self.sema, &levels);
        s.locals = locals;
        s.pairs.clone_from(&pairs);
        s.entity = entity;
        s.final_subst = final_subst;
        s.lambda_bodies = lambda_bodies;
        s.set_deferred_pack(d.pack);
        s.requires_expr(d.pattern, loc)
    }

    /// The element of the enclosing expansion a deferred
    /// requires-expression was left at ([`DeferredRequires::pack`]).
    fn set_deferred_pack(&mut self, pack: Option<(u32, u32)>) {
        if let Some((i, len)) = pack {
            self.pack_index = Some(i);
            self.pack.expanding = (len > 0).then(|| (len, crate::templ::pack::PackName::outer()));
        }
    }

    /// Checks requirement `r` ([expr.prim.req.simple] to
    /// [expr.prim.req.nested]); a failure appends its note and gives
    /// `None`. A requirement whose substitution is still dependent sets
    /// `dependent` and is returned unchecked.
    fn requirement(
        &mut self,
        body: DeclId,
        r: Requirement,
        at: Loc,
        recs: &mut Vec<SatRecord>,
        dependent: &mut bool,
    ) -> Option<Requirement> {
        // With every level substituted by concrete arguments, a result
        // that is only instantiation-dependent names a local of a context
        // that stays a template (an init-capture of a generic lambda,
        // P2036): it is checked now, as clang checks the call operator's
        // specialization.
        let deps = if self.sema.levels_dependent(self.args) {
            ExprDeps::TYPE_VALUE_INST.with(ExprDeps::UNEXPANDED_PACK)
        } else {
            ExprDeps::TYPE.with(ExprDeps::VALUE).with(ExprDeps::UNEXPANDED_PACK)
        };
        let dep_expr = |s: &Self, x: ExprId| s.sema.ast.exprs.get(x).deps().intersects(deps);
        match r {
            Requirement::Simple(e) => {
                let x = self.req_expr(body, e, recs)?;
                *dependent |= dep_expr(self, x);
                Some(Requirement::Simple(x))
            }
            Requirement::Type(t, loc) => {
                let _ = at;
                let kind = FrameKind::RequirementInstantiation;
                match self.sfinae_as(kind, body, loc, |s| s.ty(t, loc).ok()) {
                    Some((Some(x), Ok(()))) => {
                        *dependent |= self.sema.ast.types.qflags(x).intersects(
                            TypeFlags::DEPENDENT_INSTANTIATION.union(TypeFlags::UNEXPANDED_PACK),
                        );
                        Some(Requirement::Type(x, loc))
                    }
                    Some((_, st)) => {
                        let first = st.err().flatten();
                        let text = self.sema.type_requirement_text(t);
                        recs.push(invalid_record(self.sema, first.as_ref(), loc, text));
                        None
                    }
                    None => None,
                }
            }
            Requirement::Compound { expr, noexcept, constraint } => {
                let x = self.req_expr(body, expr, recs)?;
                if dep_expr(self, x) {
                    *dependent = true;
                    let c = match constraint {
                        Some(c) => Some(self.req_subst(body, c, at)?),
                        None => None,
                    };
                    return Some(Requirement::Compound { expr: x, noexcept, constraint: c });
                }
                let done = Some(Requirement::Compound { expr: x, noexcept, constraint });
                if let Some(at) = noexcept
                    && self.sema.can_throw(x)
                {
                    recs.push(SatRecord {
                        id: DiagId::note_requirement_may_throw,
                        loc: at,
                        select: true,
                        args: vec![self.sema.expr_text(expr)],
                        nested: None,
                    });
                    return None;
                }
                if let Some(c) = constraint {
                    return self.return_type_requirement(c, x, recs).then_some(done).flatten();
                }
                done
            }
            Requirement::Nested(e) => {
                let loc = self.sema.begin_loc(e);
                let Some(x) = self.req_subst(body, e, loc) else {
                    // A substitution failure in a nested requirement's
                    // constraint makes it unsatisfied, unless its atoms
                    // checked one by one are satisfied (an argument no
                    // atom's parameter mapping names failed).
                    // The atoms name the substituted local parameters.
                    let locals = || self.locals.iter().map(|(&k, &v)| (k, v)).collect();
                    if !self.sema.levels_dependent(self.args)
                        && self.sema.sat_nested_requirement(body, e, self.args, locals())
                    {
                        let t = self.sema.builder().bool_literal(QualType::BOOL, true, loc);
                        return Some(Requirement::Nested(t));
                    }
                    recs.push(SatRecord {
                        id: DiagId::note_atomic_constraint_false,
                        loc,
                        select: true,
                        args: vec![self.sema.expr_text(e)],
                        nested: None,
                    });
                    return None;
                };
                if self.sema.has_errors_in(x) {
                    return None;
                }
                if self.sema.expr_is_value_dependent(x) || dep_expr(self, x) {
                    *dependent = true;
                    return Some(Requirement::Nested(x));
                }
                let c = crate::conv::contextually_convert_to_bool(self.sema, x).unwrap_or(x);
                match self.sema.eval_const(c, EvalContext::new(EvalKind::Constraint, loc)) {
                    Ok(v) if self.sema.const_truth(v) == Some(true) => Some(Requirement::Nested(x)),
                    Ok(_) => {
                        let r = self.sema.false_atom_records(x);
                        recs.extend(r);
                        None
                    }
                    Err(_) => {
                        self.sema.error(DiagId::sema_non_constant_constraint, loc);
                        None
                    }
                }
            }
        }
    }

    /// Substitutes `e` in a SFINAE context.
    fn req_subst(&mut self, body: DeclId, e: ExprId, loc: Loc) -> Option<ExprId> {
        match self.sfinae(body, loc, |s| s.expr(e).ok()) {
            Some((Some(x), Ok(()))) => Some(x),
            _ => None,
        }
    }

    /// Substitutes the expression of a simple or compound requirement;
    /// `None` (with the "would be invalid" note) when it fails.
    fn req_expr(&mut self, body: DeclId, e: ExprId, recs: &mut Vec<SatRecord>) -> Option<ExprId> {
        let loc = self.sema.begin_loc(e);
        // An expression of overloaded-function type with no target
        // (`&X::operator()` naming a template) is invalid
        // ([over.over]/1; clang's `CheckPlaceholderExpr` on the
        // requirement). So is a bound member function that is not
        // called (`t.*pmf`, `t.f`, [expr.mptr.oper]/6, [expr.ref]/7.3.2):
        // the requirement is not satisfied, a substitution failure, not
        // the error at the end of a full-expression.
        let checked = |s: &mut Self| {
            let x = s.expr(e).ok()?;
            let dependent = s.sema.ast.exprs.get(x).deps().intersects(ExprDeps::TYPE_VALUE_INST);
            if dependent {
                return Some(x);
            }
            if s.sema.ast.exprs.get(x).ty() == QualType::BOUND_MEMBER {
                s.sema.diagnose_bound_member(x);
                return None;
            }
            (!s.sema.check_overload_use(x)).then_some(x)
        };
        match self.sfinae_as(FrameKind::RequirementInstantiation, body, loc, checked) {
            Some((Some(x), Ok(()))) => Some(x),
            Some((_, st)) => {
                let first = st.err().flatten();
                if let Some(d) = first
                    .as_ref()
                    // kiln-gap: check
                    .filter(|d| d.id() == Some(DiagId::sema_unimplemented_template_feature))
                {
                    // Not substitutable yet: a hard error, never assumed
                    // to hold.
                    self.sema.emit(d.clone());
                    return None;
                }
                let text = self.sema.expr_text_as_written(e);
                recs.push(invalid_record(self.sema, first.as_ref(), loc, text));
                None
            }
            None => None,
        }
    }

    /// `{ x } -> C<args>` ([expr.prim.req.compound]/1.3): `C<decltype((x)),
    /// args>` must be satisfied. `c` is the pattern's
    /// immediately-declared constraint.
    fn return_type_requirement(&mut self, c: ExprId, x: ExprId, recs: &mut Vec<SatRecord>) -> bool {
        let n = *self.sema.ast.exprs.get(c);
        let (Some(concept), Some(l)) = (DeclId::from_raw(n.a()), ArgListId::from_raw(n.b())) else {
            return true;
        };
        let loc = n.loc();
        let written: Vec<TemplateArg> = self.sema.ast.types.args(l).to_vec();
        let xn = *self.sema.ast.exprs.get(x);
        let ty = match xn.value_category() {
            ValueCategory::LValue => self.sema.ast.types.lvalue_ref_to(xn.ty()),
            ValueCategory::XValue => self.sema.ast.types.rvalue_ref_to(xn.ty()),
            ValueCategory::PRValue => xn.ty(),
        };
        let mut args = Vec::with_capacity(written.len());
        args.push(TemplateArg::ty(ty));
        // A pack expansion among them (`-> C<P...>`) expands to its
        // elements (§7.2).
        for &a in written.iter().skip(1) {
            match self.expand_arg(a, loc) {
                Ok(Some(xs)) => args.extend(xs),
                Ok(None) => match self.template_arg(a, loc) {
                    Ok(a) => args.push(a),
                    Err(SubstFail) => return false,
                },
                Err(SubstFail) => return false,
            }
        }
        let list = self.sema.ast.types.arg_list(&args);
        let Ok(e) = self.sema.build_concept_id(concept, list, &[], loc) else { return false };
        if self.sema.ast.exprs.satisfied(e) != Some(false) {
            return true;
        }
        let nested = self.sema.inst.concepts.expr_sat.get(&e).copied();
        recs.push(SatRecord {
            id: DiagId::note_requirement_type_constraint,
            loc,
            select: true,
            args: vec![self.sema.expr_text(e)],
            nested,
        });
        false
    }
}

/// The "'X' would be invalid: msg" note for a failed substitution.
fn invalid_record(s: &Sema<'_, '_>, first: Option<&Diag>, loc: Loc, text: String) -> SatRecord {
    let _ = s;
    // clang's `*_unknown_substitution_error` when no diagnostic was
    // captured (a hard error inside the substitution).
    let Some(first) = first else {
        return SatRecord {
            id: DiagId::note_requirement_would_be_invalid_unknown,
            loc,
            select: true,
            args: vec![text],
            nested: None,
        };
    };
    SatRecord {
        id: DiagId::note_requirement_would_be_invalid,
        loc: first.loc(),
        select: true,
        args: vec![text, first.message()],
        nested: None,
    }
}

impl Sema<'_, '_> {
    /// A type requirement's type as clang prints it (`typename T::type`).
    pub(crate) fn type_requirement_text(&self, t: QualType) -> String {
        let s = self.ty_str(t);
        if s.contains("::") && !s.starts_with("typename ") { format!("typename {s}") } else { s }
    }
}
