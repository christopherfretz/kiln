//! Calls of overloaded names ([over.match.call]; spec/sema-overload.md
//! §2.1–§2.2): an `UnresolvedLookupExpr` callee is resolved against the
//! call's arguments, and the winner becomes a `DeclRefExpr` that the
//! direct call path then converts the arguments for.

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::{ArgInfo, CandidateKind, IcsOptions, OverloadCandidateSet, OvlOutcome, SetKind, UleFound};
use crate::Sema;
use crate::init::InitList;

impl Sema<'_, '_> {
    /// An `UnresolvedLookupExpr` for the overload set `decls` (found as
    /// `found`, parallel) named `name`: `adl` for an unqualified callee
    /// whose lookup does not suppress argument-dependent lookup.
    #[allow(clippy::too_many_arguments, reason = "the lookup result's parts")]
    pub(crate) fn build_overload_ref(
        &mut self,
        name: kiln_source::symbol::Symbol,
        decls: &[DeclId],
        found: &[DeclId],
        naming: Option<DeclId>,
        adl: bool,
        nns: Option<kiln_ast::NnsId>,
        loc: Loc,
    ) -> ExprId {
        let e = self.builder().unresolved_lookup(name, decls, adl, nns, None, loc);
        if naming.is_some() || found.iter().zip(decls).any(|(f, d)| f != d) {
            self.ovl.ule_found.insert(e, UleFound { found: found.to_vec(), naming });
        }
        e
    }

    /// Resolves the non-dependent call of the overloaded name `callee` (an
    /// `UnresolvedLookupExpr`, possibly parenthesized) with `args`
    /// (`braced` the braced-init-list ones): the `DeclRefExpr` naming the
    /// best viable function, or `None` after diagnosing.
    pub(crate) fn resolve_unresolved_call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        braced: &[Option<InitList>],
    ) -> Option<ExprId> {
        let ule = self.ignore_parens(callee);
        let n = *self.ast.exprs.get(ule);
        let name = self.ast.unresolved_name(ule);
        let decls = self.ast.unresolved_decls(ule);
        let adl = n.aux() != 0;
        let extra = self.ovl.ule_found.remove(&ule);
        let naming = extra.as_ref().and_then(|x| x.naming);
        let mut cands: Vec<(DeclId, DeclId)> = decls
            .iter()
            .enumerate()
            .map(|(i, &d)| (d, extra.as_ref().and_then(|x| x.found.get(i).copied()).unwrap_or(d)))
            .collect();
        if adl {
            self.adl_lookup(name, args, &mut cands);
        }
        cands.retain(|&(d, _)| !self.ast.decls.get(d).has(DeclFlags::INVALID));
        let at = self.begin_loc(callee);
        if cands.is_empty() {
            let text = self.ast.symbols().as_str(name).to_owned();
            let msg = self.diag(DiagId::sema_undeclared_var_use, n.loc()).arg(text);
            self.emit(msg);
            return None;
        }
        if self.ast.templates.expr_targs.get(ule).is_some() {
            // `f<args>(…)`: explicit template arguments (WP-071).
            self.templ_unimplemented("calls to function templates", n.loc());
            return None;
        }
        let templates = cands.iter().any(|&(d, _)| self.kind_of(d) == DeclKind::FunctionTemplate);
        let plain = cands.iter().filter(|&&(d, _)| self.kind_of(d) != DeclKind::FunctionTemplate).count();
        if plain == 0 {
            self.templ_unimplemented("calls to function templates", n.loc());
            return None;
        }
        let nns = self.ast.templates.expr_nns.get(ule).copied();
        if plain == 1 && !templates {
            // One candidate: the direct path's conversions and
            // diagnostics are clang's for a one-candidate set.
            let (d, f) = cands[0];
            return Some(self.finish_overload_ref(d, f, naming, n.loc(), nns, ule, true));
        }
        let mut set = self.begin_set(SetKind::Normal, at);
        let base = self.ovl.lists.len();
        for (i, &a) in args.iter().enumerate() {
            match braced.get(i).and_then(Option::as_ref) {
                Some(l) => {
                    let idx = u32::try_from(self.ovl.lists.len()).unwrap_or(u32::MAX);
                    self.ovl.lists.push(l.clone());
                    set.args.push(ArgInfo {
                        list: Some(idx),
                        ..ArgInfo::of_type(QualType::VOID, ValueCategory::PRValue)
                    });
                }
                None => set.args.push(self.arg_info(a)),
            }
        }
        for (i, &(d, f)) in cands.iter().enumerate() {
            if self.kind_of(d) == DeclKind::FunctionTemplate {
                set.skipped_templates = true;
                continue;
            }
            if self.ast.decls.cold.requires_clause.contains(d) {
                set.uncertain = true;
            }
            self.add_candidate(&mut set, d, f, CandidateKind::Function, IcsOptions::NONE);
            if i >= decls.len()
                && let Some(c) = set.cands.last_mut()
            {
                c.adl_only = true;
            }
        }
        let out = self.best_viable(&mut set);
        self.ovl.lists.truncate(base);
        let r = self.finish_overloaded_call(&set, out, name, at, n.loc(), naming, nns, ule);
        self.end_set(set);
        r
    }

    #[allow(clippy::too_many_arguments, reason = "the call's parts")]
    fn finish_overloaded_call(
        &mut self,
        set: &OverloadCandidateSet,
        out: OvlOutcome,
        name: kiln_source::symbol::Symbol,
        at: Loc,
        loc: Loc,
        naming: Option<DeclId>,
        nns: Option<kiln_ast::NnsId>,
        ule: ExprId,
    ) -> Option<ExprId> {
        let text = self.ast.symbols().as_str(name).to_owned();
        let undecided = set.uncertain
            || (set.skipped_templates && !matches!(out, OvlOutcome::Success(_) | OvlOutcome::Deleted(_)));
        if undecided {
            if set.skipped_templates {
                self.templ_unimplemented("calls to function templates", loc);
            } else {
                self.unsupported("overload resolution with constraints", loc);
            }
            return None;
        }
        match out {
            OvlOutcome::Success(b) => {
                let c = set.cand(b);
                let (d, f) = (c.func(), c.found);
                Some(self.finish_overload_ref(d, f, naming, loc, nns, ule, false))
            }
            OvlOutcome::Deleted(b) => {
                let d = set.cand(b).func();
                let msg = self
                    .diag(DiagId::sema_call_deleted_function, at)
                    .arg(self.decl_arg(d))
                    .arg(self.delete_message_suffix(d));
                let msg = self.with_ovl_notes(msg, set, false);
                self.emit(msg);
                None
            }
            OvlOutcome::NoViable => {
                let msg = self.diag(DiagId::sema_ovl_no_viable_function_in_call, at).arg(text);
                let msg = self.with_ovl_notes(msg, set, false);
                self.emit(msg);
                None
            }
            OvlOutcome::Ambiguous => {
                let msg = self.diag(DiagId::sema_ovl_ambiguous_call, at).arg(text);
                let msg = self.with_ovl_notes(msg, set, true);
                self.emit(msg);
                None
            }
        }
    }

    /// The `DeclRefExpr` naming the selected function `d` (found as
    /// `found`) at `loc`, with the access check of a class member found by
    /// lookup in `naming`. `single`: the only candidate, whose use as a
    /// deleted function the call diagnoses.
    #[allow(clippy::too_many_arguments, reason = "the reference's parts")]
    pub(crate) fn finish_overload_ref(
        &mut self,
        d: DeclId,
        found: DeclId,
        naming: Option<DeclId>,
        loc: Loc,
        nns: Option<kiln_ast::NnsId>,
        ule: ExprId,
        single: bool,
    ) -> ExprId {
        if let Some(naming) = naming {
            self.check_member_access(found, naming, None, loc);
        }
        let shadow = (found != d).then_some(found);
        let e = self.build_decl_ref_qualified(d, loc, shadow, nns);
        if let Some(&b) = self.ast.templates.expr_begin.get(ule)
            && b != loc
            && self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr
        {
            let _ = self.ast.templates.expr_begin.insert(e, b);
        }
        if single {
            self.note_function_name_use(d, e, loc, nns.is_none());
            if nns.is_none() {
                self.act_on_deleted_callee(e, false, false);
            }
        }
        e
    }
}
