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

    /// Queues explicitly instantiated `decl` at `poi` even when an
    /// implicit use queued it already: clang reports an undefined static
    /// data member at each (§4.5).
    pub(crate) fn queue_explicit_instantiation(&mut self, decl: DeclId, poi: Loc) {
        if !self.inst.queued.insert(decl)
            && self.inst.pending.iter().any(|p| p.decl == decl && p.poi == poi)
        {
            return;
        }
        self.inst.pending.push_back(PendingInst { decl, poi });
    }

    /// Instantiates every queued definition, including the ones queued
    /// while draining ([temp.point]/7: the end of the TU is a point of
    /// instantiation of every specialization).
    pub fn end_of_tu_instantiations(&mut self) {
        self.inst.at_end_of_tu = true;
        // clang `ActOnEndOfTranslationUnit`: the used vtables' virtual
        // members, then the pending instantiations, until neither adds
        // more (class/virtual/used.rs).
        loop {
            self.define_used_vtables();
            if self.instantiation_stopped() {
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
    /// a long chain of instantiations does not recurse natively. As in
    /// clang, whose nested `perform` runs inside the requesting
    /// `InstantiateFunctionDefinition`, the requesting definition's frame
    /// stays on the instantiation stack while its scope drains, so the
    /// nested instantiations carry its "in instantiation of … requested
    /// here" note (and count toward the depth limit). A function
    /// definition is handed to codegen before what it requires (pre-order,
    /// `InstantiateFunctionDefinition`'s `HandleTopLevelDecl`), a variable
    /// definition after it (post-order, `InstantiateVariableDefinition`'s
    /// `PassToConsumerRAII` runs once its nested `perform` is done).
    fn drain_pending_instantiations(&mut self) {
        type Saved =
            (std::collections::VecDeque<PendingInst>, Vec<(DeclId, Loc)>, bool, Option<DeclId>);
        let mut saved: Vec<Saved> = Vec::new();
        loop {
            if self.instantiation_stopped() {
                self.inst.pending.clear();
                for (q, _, resumed, _) in &mut saved {
                    q.clear();
                    if *resumed {
                        let _ = self.inst.stack.pop();
                    }
                }
                return;
            }
            if let Some(p) = self.inst.pending.pop_front() {
                // Enter the instantiation's own scope.
                let outer = std::mem::take(&mut self.inst.pending);
                let outer_vt = std::mem::take(&mut self.vtable_uses);
                let prev = self.inst.drain_capture.replace((self.inst.stack.len(), None));
                let var_def = self.instantiate_pending(p);
                let frame = std::mem::replace(&mut self.inst.drain_capture, prev).and_then(|c| c.1);
                // Keep the definition's frame for what it required (not
                // timed or charged again: the report saw it once).
                let resumed = frame.is_some_and(|mut f| {
                    if self.inst.pending.is_empty() && self.vtable_uses.is_empty() {
                        return false;
                    }
                    f.start = None;
                    f.child_ns = 0;
                    f.trace = None;
                    self.inst.stack.push(f);
                    true
                });
                saved.push((outer, outer_vt, resumed, var_def));
                // `GlobalEagerInstantiationScope::perform`.
                self.define_used_vtables();
                continue;
            }
            if !self.vtable_uses.is_empty() {
                self.define_used_vtables();
                continue;
            }
            // Leave the scope: its queue and vtable uses are exhausted.
            let Some((outer, outer_vt, resumed, var_def)) = saved.pop() else { return };
            if resumed {
                let _ = self.inst.stack.pop();
            }
            if let Some(d) = var_def {
                self.ast.templates.add_end_of_tu_def(d);
            }
            self.inst.pending = outer;
            self.vtable_uses = outer_vt;
        }
    }

    /// Instantiates one queued definition and records it for codegen: a
    /// function at once, a variable returned to be recorded once what it
    /// requires is instantiated.
    fn instantiate_pending(&mut self, p: PendingInst) -> Option<DeclId> {
        if self.ast.templates.is_late_explicit_inst(self.ast.first(p.decl)) {
            // Explicitly instantiated before its template was defined:
            // codegen defines these itself (`emit_late_explicit_insts`).
            if self.is_var_spec(p.decl) {
                self.instantiate_late_var_spec(p.decl, p.poi);
                if self.var_pattern_def(p.decl).is_none() {
                    self.undefined_explicit_inst(p.decl, p.poi);
                }
            } else if self.kind_of(p.decl) == kiln_ast::decl::DeclKind::Var {
                self.instantiate_static_member_def(p.decl, p.poi);
            } else {
                self.instantiate_function_definition(p.decl, p.poi);
                if self.ast.definition(p.decl).is_none() {
                    self.undefined_explicit_inst(p.decl, p.poi);
                }
            }
            return None;
        }
        let is_var = self.kind_of(p.decl) == kiln_ast::decl::DeclKind::Var;
        if is_var
            && self.is_implicit_var_spec(p.decl)
            && !self.inst.member_defs.contains(&p.decl)
            && self.ast.decls.var(p.decl).is_some_and(|v| v.init.is_none())
        {
            // An implicit variable template specialization used before its
            // template's definition: instantiated from the definition seen
            // since (clang `InstantiateVariableDefinition` finds the
            // pattern's definition at the end of the TU), and emitted as
            // its own definition (spec/codegen.md §2a).
            self.instantiate_late_var_spec(p.decl, p.poi);
            if self.ast.decls.var(p.decl).is_some_and(|v| v.init.is_some()) {
                return Some(p.decl);
            }
        }
        if is_var && self.inst.deferred_inits.contains(&p.decl) {
            // A deferred inline static data member (WP-186).
            self.instantiate_static_member_init(p.decl, p.poi);
            return (!self.inst.deferred_inits.contains(&p.decl)).then_some(p.decl);
        }
        let had = self.ast.definition(p.decl).is_some();
        let redecls = self.ast.redecls_of(p.decl).count();
        // A variable template specialization's definition is the
        // specialization itself, its initializer instantiated in place.
        let spec_def =
            is_var && self.is_var_spec(p.decl) && self.inst.member_defs.contains(&p.decl);
        if is_var {
            self.instantiate_static_member_def(p.decl, p.poi);
        } else {
            self.instantiate_function_definition(p.decl, p.poi);
        }
        // clang's `InstantiateFunctionDefinition` hands the new definition
        // to the consumer (`HandleTopLevelDecl`) here, before the
        // instantiations it requires; `InstantiateVariableDefinition`
        // after them (the drain records it). Codegen replays the order
        // (spec/codegen.md §2a), so a variable's dynamic initialization
        // follows the TU's own.
        let defined = if is_var {
            self.ast.redecls_of(p.decl).count() > redecls
                || (spec_def && !self.inst.member_defs.contains(&p.decl))
        } else {
            !had && self.ast.definition(p.decl).is_some()
        };
        if defined {
            if is_var {
                return Some(p.decl);
            }
            self.ast.templates.add_end_of_tu_def(p.decl);
        } else if self.inst.quiet_pending.contains(&p.decl) {
            // A variable that may be usable in constant expressions,
            // queued only for the end-of-TU point of instantiation
            // (`note_var_referenced`): clang never queues it, so never
            // reports it undefined.
        } else if is_var && self.inst.member_defs.contains(&p.decl) {
            self.undefined_explicit_inst(p.decl, p.poi);
            self.undefined_var_template(p.decl, p.poi);
        } else if self.is_var_spec(p.decl) && self.var_pattern_def(p.decl).is_none() {
            self.undefined_var_template(p.decl, p.poi);
        }
        None
    }

    /// Whether a class enclosing member `d` is explicitly instantiated
    /// by a declaration (`extern template struct S<int>;`): clang
    /// `InstantiateClassMembers` gives its static data members that kind
    /// ([temp.explicit]/10), so their definitions are not required here.
    fn in_explicit_inst_declaration(&self, d: DeclId) -> bool {
        use kiln_ast::templates::SpecKind;
        let mut c = self.ast.decls.get(d).ctx();
        while let Some(x) =
            c.filter(|&x| self.kind_of(x).payload() == kiln_ast::decl::PayloadClass::Record)
        {
            let first = self.ast.first(x);
            let kind = self
                .ast
                .spec_info(first)
                .map(|(_, s)| s.kind)
                .or_else(|| self.ast.member_spec_info(first).map(|(_, m)| m.kind));
            if kind == Some(SpecKind::ExplicitInstantiationDeclaration) {
                return true;
            }
            c = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// clang `InstantiateVariableDefinition` at the end of the TU: an
    /// implicitly instantiated variable template specialization or static
    /// data member odr-used without a definition anywhere in the TU is
    /// `-Wundefined-var-template` at its point of instantiation, unless
    /// an error was reported before or the pattern is in a system header
    /// (§4.5).
    fn undefined_var_template(&mut self, d: DeclId, poi: Loc) {
        use kiln_ast::templates::SpecKind;
        let (kind, pattern, name) = if let Some((_, s)) = self.ast.spec_info(d) {
            let (kind, from, template, args) = (s.kind, s.from, s.template, s.args);
            (kind, from, self.spec_decl_name(template, args))
        } else if let Some((_, m)) = self.ast.member_spec_info(d) {
            let (kind, from) = (m.kind, m.instantiated_from);
            (kind, from, self.qualified_decl_name(d))
        } else {
            return;
        };
        let pattern_loc = self.ast.decls.get(self.ast.latest(pattern)).loc();
        if kind != SpecKind::ImplicitInstantiation
            || self.in_explicit_inst_declaration(d)
            || self.inst.fatal
            || self.has_errors()
            || self.diags.sm().is_in_system_header(pattern_loc)
        {
            return;
        }
        let e = self.diag(kiln_diag::DiagId::sema_undefined_var_template, poi).arg(name.clone());
        let fwd = self.diag(kiln_diag::DiagId::note_forward_template_decl, pattern_loc);
        let hint = self.diag(kiln_diag::DiagId::note_inst_declaration_hint, poi).arg(name);
        // Repeated per queued use, as clang (no deduplication).
        self.emit(e.with_note(fwd).with_note(hint).allow_duplicate());
    }

    /// clang `InstantiateFunctionDefinition`/`InstantiateVariableDefinition`
    /// with `DefinitionRequired` ([temp.explicit]/5): an explicit
    /// instantiation definition whose pattern is still undefined at the
    /// end of the TU is an error at its point of instantiation.
    fn undefined_explicit_inst(&mut self, d: DeclId, poi: Loc) {
        use kiln_ast::templates::SpecKind;
        let (kind, pattern, template) = if let Some((_, s)) = self.ast.spec_info(d) {
            (s.kind, self.ast.instantiated_from(d).unwrap_or(s.from), Some(s.template))
        } else if let Some((_, m)) = self.ast.member_spec_info(d) {
            (m.kind, m.instantiated_from, None)
        } else {
            return;
        };
        if kind != SpecKind::ExplicitInstantiationDefinition || self.inst.fatal {
            return;
        }
        let e = if let Some(t) = template
            && self.is_var_spec(d)
        {
            let args = self.ast.spec_info(d).map(|(_, s)| s.args);
            let name = args.map_or_else(|| self.decl_name(t), |a| self.spec_decl_name(t, a));
            self.diag(kiln_diag::DiagId::sema_explicit_inst_undefined_var_template, poi).arg(name)
        } else if let Some(t) = template {
            self.diag(kiln_diag::DiagId::sema_explicit_inst_undefined_func_template, poi)
                .arg(self.decl_name(t))
        } else {
            let Some(class) = self.ast.decls.get(d).ctx() else { return };
            let var = self.kind_of(d) == kiln_ast::decl::DeclKind::Var;
            self.diag(kiln_diag::DiagId::sema_explicit_inst_undefined_member, poi)
                .arg(if var { 2u8 } else { 1u8 })
                .arg(self.decl_name(d))
                .arg(self.decl_ty_str(self.tag_type(class)))
        };
        let note = self.diag(
            kiln_diag::DiagId::note_explicit_instantiation_here,
            self.ast.decls.get(pattern).loc(),
        );
        self.emit(e.with_note(note));
    }
}
