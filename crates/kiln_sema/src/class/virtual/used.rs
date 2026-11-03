//! Vtable uses ([basic.def.odr]/8, WP-137): a constructor or destructor
//! definition, or the definition of a class's key function, uses the
//! class's vtable, which odr-uses every non-pure final overrider in it
//! (clang `MarkVTableUsed`, `DefineUsedVTables` and
//! `MarkVirtualMembersReferenced`). The overriders are marked at the end
//! of the translation unit, interleaved with the pending instantiations
//! (an instantiated definition can use further vtables); a local class's
//! at once.

use std::rc::Rc;

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DefinitionBits};
use kiln_ast::modules::UnitKind;
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use super::graph::{self, is_pure};
use crate::Sema;

impl Sema<'_, '_> {
    /// clang `MarkVTableUsed`: the vtable of `class` is used at `loc`.
    pub(crate) fn mark_vtable_used(&mut self, class: DeclId, loc: Loc) {
        if self.unevaluated > 0 || self.in_template() {
            return;
        }
        let Some(def) = self.ast.definition(class) else { return };
        let dynamic = self.ast.decls.record(def).is_some_and(|r| {
            r.defn.contains(DefinitionBits::POLYMORPHIC)
                || r.defn.contains(DefinitionBits::HAS_VIRTUAL_BASES)
        });
        if !dynamic || self.ast.decls.get(def).flags().contains(DeclFlags::DEPENDENT_CTX) {
            return;
        }
        if !self.vtables_used.insert(self.ast.first(def)) {
            return;
        }
        if self.is_local_class(def) {
            self.mark_virtual_members_referenced(loc, def);
        } else {
            self.vtable_uses.push((def, loc));
        }
    }

    /// The key function of a just-defined method `f` makes its class's
    /// vtable used (Itanium C++ ABI §5.2.3: the vtable is emitted with
    /// it).
    pub(crate) fn mark_key_function_vtable_used(&mut self, f: DeclId) {
        let Some(class) = self.ast.decls.get(f).ctx() else { return };
        let Some(def) = self.ast.definition(class) else { return };
        let Some(&k) = self.ast.decls.cold.key_function.get(def) else { return };
        if self.ast.first(k) == self.ast.first(f) {
            let loc = self.ast.decls.get(f).loc();
            self.mark_vtable_used(def, loc);
        }
    }

    /// A dynamic class defined in the purview of a module interface or
    /// partition unit has its vtable emitted there even if unused (clang
    /// `ActOnTagFinishDefinition`; spec/modules.md §7): its importers
    /// declare it external, so the owning unit must define it and its
    /// virtual members.
    pub(crate) fn mark_module_vtable_used(&mut self, r: DeclId) {
        let o = self.ast.modules.owner(r);
        let interface = self.ast.modules.units.get(usize::from(o.unit)).is_some_and(|u| {
            matches!(
                u.kind,
                UnitKind::Interface
                    | UnitKind::PartitionInterface
                    | UnitKind::PartitionImplementation
            )
        });
        if o.unit != 0 || !interface || self.ast.module_attachment(r).is_none() {
            return;
        }
        let loc = self.ast.decls.get(r).loc();
        self.mark_vtable_used(r, loc);
        self.module_vtables.insert(self.ast.first(r));
    }

    /// clang `DefineUsedVTables`: marks the virtual members of every used
    /// vtable whose class's key function is not declared-but-undefined
    /// here (that vtable is emitted with the key function elsewhere).
    /// Returns whether anything was marked.
    pub(crate) fn define_used_vtables(&mut self) -> bool {
        let mut any = false;
        let uses = std::mem::take(&mut self.vtable_uses);
        for (class, loc) in uses {
            if let Some(&k) = self.ast.decls.cold.key_function.get(class)
                && self.ast.definition(k).is_none()
                && !self.module_vtables.contains(&self.ast.first(class))
            {
                continue;
            }
            any = true;
            self.mark_virtual_members_referenced(loc, class);
        }
        any
    }

    /// clang `MarkVirtualMembersReferenced`: odr-uses each non-pure final
    /// overrider in `class`'s vtable at `loc`, then (for the VTT) the
    /// virtual members of each direct base that has virtual bases. clang
    /// recurses into every such base, which is exponential in the depth
    /// of a virtual lattice; kiln walks the same preorder once, visiting
    /// each class once (a second visit only re-marks referenced
    /// functions at the same `loc`, a no-op; WP-400).
    fn mark_virtual_members_referenced(&mut self, loc: Loc, class: DeclId) {
        let has_vbases = |s: &Self, c: DeclId| {
            s.ast
                .definition(c)
                .and_then(|d| s.ast.decls.record(d))
                .is_some_and(|r| r.defn.contains(DefinitionBits::HAS_VIRTUAL_BASES))
        };
        let mut visited: FxHashSet<DeclId> = FxHashSet::default();
        let mut stack = vec![class];
        while let Some(c) = stack.pop() {
            if !visited.insert(self.ast.first(c)) {
                continue;
            }
            for &o in self.vtable_finals(c).iter() {
                if !is_pure(&self.ast, o) {
                    self.mark_function_referenced(o, loc);
                }
            }
            if !has_vbases(self, c) {
                continue;
            }
            let at = stack.len();
            for b in graph::direct_bases(&self.ast, c) {
                if has_vbases(self, b.class) {
                    stack.push(b.class);
                }
            }
            stack[at..].reverse();
        }
    }

    /// The final overriders in `class`'s vtable in clang's
    /// `CXXFinalOverriderMap` order (by method), each once (cached per
    /// class once it is complete).
    fn vtable_finals(&mut self, class: DeclId) -> Rc<[DeclId]> {
        let first = self.ast.first(class);
        if let Some(f) = self.vtable_finals.get(&first) {
            return f.clone();
        }
        if graph::ensure_virtuals(&mut self.ast, class).is_none() {
            return Rc::from([]);
        }
        let Some(v) = self.ast.virtuals.get(self.ast.first(class)) else { return Rc::from([]) };
        let mut by_method: FxHashMap<DeclId, Vec<DeclId>> = FxHashMap::default();
        for e in &v.overriders {
            by_method.entry(self.ast.first(e.1)).or_default().push(e.2);
        }
        let mut seen: FxHashSet<DeclId> = FxHashSet::default();
        let mut finals: Vec<DeclId> = Vec::new();
        for m in graph::method_order(&self.ast, class) {
            for &o in by_method.get(&m).into_iter().flatten() {
                if seen.insert(o) {
                    finals.push(o);
                }
            }
        }
        let finals: Rc<[DeclId]> = finals.into();
        self.vtable_finals.insert(first, finals.clone());
        finals
    }
}
