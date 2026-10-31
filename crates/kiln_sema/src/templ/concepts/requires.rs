//! Requires-expressions (spec/sema-templates.md §8.6, [expr.prim.req]):
//! the requirements are substituted in order, each in a SFINAE context,
//! and checking stops at the first that fails
//! ([expr.prim.req.general]/5). The result is a non-dependent
//! `RequiresExpr` whose satisfaction (and the notes explaining a
//! failure) is recorded.

use kiln_ast::expr::ValueCategory;
use kiln_ast::ids::{ArgListId, RawId};
use kiln_ast::templates::Requirement;
use kiln_ast::types::TemplateArg;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use super::{SatRecord, Satisfaction};
use crate::Sema;
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
        if !self.sema.push_inst_frame(FrameKind::ConstraintSubstitution, entity, entity, loc) {
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

    /// Substitutes into requires-expression `e` and evaluates it.
    pub(crate) fn requires_expr(&mut self, e: ExprId, loc: Loc) -> SubstResult<ExprId> {
        let Some(rq) = self.sema.ast.requires_of(e).copied() else { return Err(SubstFail) };
        let items: Vec<Requirement> = self.sema.ast.templates.requirements(&rq).to_vec();
        let params: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(rq.params).collect();
        let at = self.sema.ast.exprs.get(e).loc();
        let rbrace = self.sema.ast.exprs.cold.end_loc.get(e).copied().unwrap_or(at);
        let mut recs = Vec::new();
        let mut new_params = Vec::with_capacity(params.len());
        let mut ok = true;
        for (i, &p) in params.iter().enumerate() {
            let n = *self.sema.ast.decls.get(p);
            let ty = n.ty().unwrap_or(QualType::ERROR);
            let r = self.sfinae(rq.body, n.loc(), |s| s.ty(ty, n.loc()).ok());
            match r {
                Some((Some(t), Ok(()))) => {
                    let np = self.sema.builder().param(
                        rq.body,
                        n.name(),
                        n.loc(),
                        t,
                        u16::try_from(i).unwrap_or(u16::MAX),
                        0,
                    );
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
        if ok {
            for r in items {
                if !self.requirement(rq.body, r, at, &mut recs) {
                    ok = false;
                    break;
                }
            }
        }
        let x = self.sema.builder().requires_expr(
            rq.body,
            &new_params,
            &[],
            QualType::BOOL,
            at,
            rbrace,
        );
        let _ = self.sema.ast.exprs.set_satisfied(x, ok);
        if !ok {
            let id = self.sema.inst.concepts.push(Satisfaction { ok, records: recs });
            self.sema.inst.concepts.expr_sat.insert(x, id);
        }
        let _ = loc;
        Ok(x)
    }

    /// Checks requirement `r` ([expr.prim.req.simple] to
    /// [expr.prim.req.nested]); a failure appends its note.
    fn requirement(
        &mut self,
        body: DeclId,
        r: Requirement,
        at: Loc,
        recs: &mut Vec<SatRecord>,
    ) -> bool {
        match r {
            Requirement::Simple(e) => self.req_expr(body, e, recs).is_some(),
            Requirement::Type(t, loc) => {
                let _ = at;
                match self.sfinae(body, loc, |s| s.ty(t, loc).ok()) {
                    Some((Some(_), Ok(()))) => true,
                    Some((_, st)) => {
                        let first = st.err().flatten();
                        let text = self.sema.type_requirement_text(t);
                        recs.push(invalid_record(self.sema, first.as_ref(), loc, text));
                        false
                    }
                    None => false,
                }
            }
            Requirement::Compound { expr, noexcept, constraint } => {
                let Some(x) = self.req_expr(body, expr, recs) else { return false };
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
                    return false;
                }
                if let Some(c) = constraint {
                    return self.return_type_requirement(c, x, recs);
                }
                true
            }
            Requirement::Nested(e) => {
                let loc = self.sema.begin_loc(e);
                let Some(x) = self.req_subst(body, e, loc) else {
                    // A substitution failure in a nested requirement's
                    // constraint makes it unsatisfied.
                    recs.push(SatRecord {
                        id: DiagId::note_atomic_constraint_false,
                        loc,
                        select: true,
                        args: vec![self.sema.expr_text(e)],
                        nested: None,
                    });
                    return false;
                };
                if self.sema.expr_is_value_dependent(x) || self.sema.has_errors_in(x) {
                    return !self.sema.has_errors_in(x);
                }
                let c = crate::conv::contextually_convert_to_bool(self.sema, x).unwrap_or(x);
                match self.sema.eval_const(c, EvalContext::new(EvalKind::Constraint, loc)) {
                    Ok(v) if self.sema.const_truth(v) == Some(true) => true,
                    Ok(_) => {
                        let r = self.sema.false_atom_records(x);
                        recs.extend(r);
                        false
                    }
                    Err(_) => {
                        self.sema.error(DiagId::sema_non_constant_constraint, loc);
                        false
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
        match self.sfinae(body, loc, |s| s.expr(e).ok()) {
            Some((Some(x), Ok(()))) => Some(x),
            Some((_, st)) => {
                let first = st.err().flatten();
                if let Some(d) = first
                    .as_ref()
                    .filter(|d| d.id() == Some(DiagId::sema_unimplemented_template_feature))
                {
                    // Not substitutable yet: a hard error, never assumed
                    // to hold.
                    self.sema.emit(d.clone());
                    return None;
                }
                let text = self.sema.expr_text(e);
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
        for &a in written.iter().skip(1) {
            match self.template_arg(a, loc) {
                Ok(a) => args.push(a),
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
    let at = first.map_or(loc, Diag::loc);
    let msg = first.map(Diag::message).unwrap_or_default();
    let _ = s;
    SatRecord {
        id: DiagId::note_requirement_would_be_invalid,
        loc: at,
        select: true,
        args: vec![text, msg],
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
