//! Lazy member instantiation (spec/sema-templates.md §4.4): member
//! function bodies of class template specializations are instantiated
//! when odr-used; the definitions that cannot be instantiated at the
//! point of use are queued here and drained at the end of the
//! translation unit ([temp.point]/7).

use kiln_ast::DeclId;
use kiln_source::Loc;

use crate::Sema;

/// A definition whose instantiation is required (§4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingInst {
    /// The member declaration of the specialization.
    pub decl: DeclId,
    /// Its point of instantiation.
    pub poi: Loc,
}

impl Sema<'_, '_> {
    /// Queues `decl`'s definition for instantiation at end of TU.
    pub(crate) fn queue_instantiation(&mut self, decl: DeclId, poi: Loc) {
        if self.inst.queued.insert(decl) {
            self.inst.pending.push_back(PendingInst { decl, poi });
        }
    }

    /// Instantiates every queued definition, including the ones queued
    /// while draining ([temp.point]/7: the end of the TU is a point of
    /// instantiation of every specialization).
    pub fn end_of_tu_instantiations(&mut self) {
        // clang `ActOnEndOfTranslationUnit`: the used vtables' virtual
        // members, then the pending instantiations, until neither adds
        // more (class/virtual/used.rs).
        loop {
            self.define_used_vtables();
            if self.inst.fatal {
                break;
            }
            if self.inst.pending.is_empty() {
                if self.vtable_uses.is_empty() {
                    break;
                }
                continue;
            }
            self.drain_pending_instantiations();
        }
    }

    fn drain_pending_instantiations(&mut self) {
        while let Some(p) = self.inst.pending.pop_front() {
            if self.inst.fatal {
                self.inst.pending.clear();
                return;
            }
            if self.is_var_spec(p.decl)
                && self.ast.templates.is_late_explicit_inst(self.ast.first(p.decl))
            {
                self.instantiate_late_var_spec(p.decl, p.poi);
            } else if self.kind_of(p.decl) == kiln_ast::decl::DeclKind::Var {
                self.instantiate_static_member_def(p.decl, p.poi);
            } else {
                self.instantiate_function_definition(p.decl, p.poi);
            }
        }
    }
}
