//! Prospective destructors ([class.dtor]/4; spec/sema-class.md §4.7.1)
//! and eligible special member functions ([special]/6; §4.7.2).
//!
//! A class may declare several destructors that differ in their trailing
//! requires-clauses. At the end of the definition of a non-dependent
//! class, overload resolution with an empty argument list selects the
//! destructor among them (clang `ComputeSelectedDestructor`); the others
//! are marked [`FnSpec::not_selected`] and are not the destructor (not
//! its triviality, `constexpr`-ness or callee).
//!
//! The other special members are not selected but filtered: one whose
//! constraints are not satisfied, or for which another of the same kind
//! is at least as constrained, is not eligible (clang
//! `ComputeSpecialMemberFunctionsEligiblity`); it is marked
//! [`FnSpec::not_selected`] too and does not count for the class's
//! triviality.

use kiln_ast::decl::{DeclFlags, DeclKind, FnSpec};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;

use super::SpecialKind;
use crate::Sema;
use crate::overload::{CandidateKind, IcsOptions, OvlOutcome, SetKind};

impl Sema<'_, '_> {
    /// Selects the destructor of class `r` at its `}` among its
    /// prospective destructors ([class.dtor]/4). Nothing to do with one
    /// unconstrained destructor (the common case: no allocation beyond
    /// the lookup). No viable or an ambiguous selection is diagnosed
    /// (`no viable destructor found for class` / `destructor of class
    /// ... is ambiguous`, with the candidates); the first declared one
    /// then stands in, as in clang.
    pub(crate) fn select_destructor(&mut self, r: DeclId) {
        let flags = self.ast.decls.get(r).flags();
        if flags.contains(DeclFlags::DEPENDENT_CTX) || flags.contains(DeclFlags::INVALID) {
            return;
        }
        let Some(ty) = self.ast.decls.get(r).ty() else { return };
        let canon = self.ast.types.canonical(ty).ty();
        let Some(name) = self.ast.names.find_destructor(canon) else { return };
        let found: Vec<DeclId> = self
            .ast
            .lookup_local(r, name)
            .iter()
            .filter(|&d| {
                self.kind_of(d) == DeclKind::CXXDestructor
                    && !self.ast.decls.get(d).has(DeclFlags::INVALID)
            })
            .collect();
        if !found.iter().any(|&d| self.ast.templates.trailing_requires.get(d).is_some()) {
            return;
        }
        let loc = self.ast.decls.get(r).loc();
        let mut set = self.begin_set(SetKind::Normal, loc);
        for &d in &found {
            self.add_candidate(&mut set, d, d, CandidateKind::Method, IcsOptions::NONE);
        }
        self.order_template_candidates(&mut set);
        let out = self.best_viable(&mut set);
        let (selected, msg) = match out {
            OvlOutcome::Success(b) | OvlOutcome::Deleted(b) => (set.cand(b).func(), None),
            OvlOutcome::Ambiguous => (found[0], Some((DiagId::sema_ambiguous_destructor, true))),
            OvlOutcome::NoViable => (found[0], Some((DiagId::sema_no_viable_destructor, false))),
        };
        if let Some((id, ambiguous)) = msg {
            let d = self.diag(id, loc).arg(self.ty_arg(self.class_type(r)));
            let d = self.with_ovl_notes(d, &set, ambiguous);
            self.emit(d);
        }
        self.end_set(set);
        for d in found {
            if d != selected {
                self.ast.decls.mark_function(d, FnSpec::new().with_not_selected(true));
            }
        }
    }

    /// Marks the special members of class `r` other than destructors
    /// that are not eligible ([special]/6) at its `}`: constraints not
    /// satisfied, or another member of the same kind, its constraints
    /// satisfied, at least as constrained (clang `SetEligibleMethods`:
    /// same kind is the same first non-object parameter type and object
    /// parameter for a copy/move member; default constructors compare
    /// with default constructors, templates never). A deleted member
    /// stays eligible, as in clang 19-22 (which defers CWG1496/CWG1734):
    /// `struct N { N(...) = delete; N() = default; }` has a non-trivial
    /// default constructor besides the trivial one. Nothing to do when no
    /// candidate has a trailing requires-clause (the common case).
    pub(crate) fn select_eligible_special_members(&mut self, r: DeclId) {
        let flags = self.ast.decls.get(r).flags();
        if flags.contains(DeclFlags::DEPENDENT_CTX) || flags.contains(DeclFlags::INVALID) {
            return;
        }
        let mut cands: Vec<(DeclId, SpecialKind, Option<ExprId>)> = Vec::new();
        let mut constrained = false;
        for m in self.ast.members(r) {
            if !matches!(self.kind_of(m), DeclKind::CXXConstructor | DeclKind::CXXMethod)
                || self.ast.decls.get(m).has(DeclFlags::INVALID)
                || !self.is_user_declared(m)
            {
                continue;
            }
            let Some(k) = self.special_kind_of(m) else { continue };
            if k == SpecialKind::Dtor {
                continue;
            }
            let req = self.ast.templates.trailing_requires.get(m).copied();
            constrained |= req.is_some();
            cands.push((m, k, req));
        }
        if !constrained {
            return;
        }
        let loc = self.ast.decls.get(r).loc();
        let satisfied: Vec<bool> = cands
            .iter()
            .map(|&(m, _, req)| {
                req.is_none()
                    || self.check_function_constraints(m, loc).is_none_or(|s| self.sat_ok(s))
            })
            .collect();
        let mut ineligible = Vec::new();
        for (i, &(m, k, req)) in cands.iter().enumerate() {
            if !satisfied[i] {
                ineligible.push(m);
                continue;
            }
            let mine: Vec<ExprId> = req.into_iter().collect();
            for (j, &(o, ok, oreq)) in cands.iter().enumerate() {
                let Some(oreq) = oreq else { continue };
                if i == j || !satisfied[j] || ok != k || !self.same_special_kind(m, o, k) {
                    continue;
                }
                if req.is_none() || self.at_least_as_constrained(o, &[oreq], m, &mine) {
                    ineligible.push(m);
                    break;
                }
            }
        }
        for m in ineligible {
            self.ast.decls.mark_function(m, FnSpec::new().with_not_selected(true));
        }
    }

    /// clang `AreSpecialMemberFunctionsSameKind` for two members of kind
    /// `k`: any two (non-template) default constructors; copy/move
    /// members with the same first non-object parameter type and the same
    /// object parameter (cv-, ref-qualifiers, explicit object type).
    fn same_special_kind(&self, a: DeclId, b: DeclId, k: SpecialKind) -> bool {
        if k == SpecialKind::DefaultCtor {
            return true;
        }
        let first = |s: &Self, f: DeclId| {
            s.special_params(f).first().and_then(|&p| s.ast.decls.get(p).ty())
        };
        let (Some(pa), Some(pb)) = (first(self, a), first(self, b)) else { return false };
        if !self.ast.types.same_type(pa, pb) {
            return false;
        }
        let object = |s: &Self, f: DeclId| {
            if let Some(t) = s.explicit_object_param_type(f) {
                return (Some(s.ast.types.canonical(t)), None);
            }
            let q = s.ast.decls.get(f).ty().and_then(|t| match s.ast.types.view(t.ty()) {
                TypeView::Function(p) => Some((p.info().this_quals(), p.info().ref_qual())),
                _ => None,
            });
            (None, q)
        };
        object(self, a) == object(self, b)
    }
}
