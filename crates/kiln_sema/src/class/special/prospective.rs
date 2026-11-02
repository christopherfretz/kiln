//! Prospective destructors ([class.dtor]/4; spec/sema-class.md §4.7.1).
//!
//! A class may declare several destructors that differ in their trailing
//! requires-clauses. At the end of the definition of a non-dependent
//! class, overload resolution with an empty argument list selects the
//! destructor among them (clang `ComputeSelectedDestructor`); the others
//! are marked [`FnSpec::not_selected`] and are not the destructor (not
//! its triviality, `constexpr`-ness or callee).

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind, FnSpec};
use kiln_diag::DiagId;

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
}
