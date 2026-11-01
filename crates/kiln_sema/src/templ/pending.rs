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

    /// clang `PerformPendingInstantiations` at the end of the TU, where
    /// every instantiation is recursive (`GlobalEagerInstantiationScope`):
    /// the definitions and vtables a body requires are instantiated right
    /// after it, before the rest of the enclosing queue (a depth-first,
    /// pre-order walk). The walk keeps its own stack of saved queues, so
    /// a long chain of instantiations does not recurse natively.
    fn drain_pending_instantiations(&mut self) {
        type Saved = (std::collections::VecDeque<PendingInst>, Vec<(DeclId, Loc)>);
        let mut saved: Vec<Saved> = Vec::new();
        loop {
            if self.inst.fatal {
                self.inst.pending.clear();
                for (q, _) in &mut saved {
                    q.clear();
                }
                return;
            }
            if let Some(p) = self.inst.pending.pop_front() {
                // Enter the instantiation's own scope.
                let outer = std::mem::take(&mut self.inst.pending);
                let outer_vt = std::mem::take(&mut self.vtable_uses);
                saved.push((outer, outer_vt));
                self.instantiate_pending(p);
                // `GlobalEagerInstantiationScope::perform`.
                self.define_used_vtables();
                continue;
            }
            if !self.vtable_uses.is_empty() {
                self.define_used_vtables();
                continue;
            }
            // Leave the scope: its queue and vtable uses are exhausted.
            let Some((outer, outer_vt)) = saved.pop() else { return };
            self.inst.pending = outer;
            self.vtable_uses = outer_vt;
        }
    }

    /// Instantiates one queued definition and records it for codegen.
    fn instantiate_pending(&mut self, p: PendingInst) {
        let had = self.ast.definition(p.decl).is_some();
        let is_var = self.kind_of(p.decl) == kiln_ast::decl::DeclKind::Var;
        let redecls = self.ast.redecls_of(p.decl).count();
        if is_var {
            self.instantiate_static_member_def(p.decl, p.poi);
        } else {
            self.instantiate_function_definition(p.decl, p.poi);
        }
        // clang's `InstantiateFunctionDefinition` hands the new
        // definition to the consumer (`HandleTopLevelDecl`) here, before
        // the instantiations it requires; codegen replays the order
        // (spec/codegen.md §2a).
        let defined = if is_var {
            self.ast.redecls_of(p.decl).count() > redecls
        } else {
            !had && self.ast.definition(p.decl).is_some()
        };
        if defined {
            self.ast.templates.add_end_of_tu_def(p.decl);
        }
    }
}
