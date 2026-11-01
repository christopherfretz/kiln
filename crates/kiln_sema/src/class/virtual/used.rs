//! Vtable uses ([basic.def.odr]/8, WP-137): a constructor or destructor
//! definition, or the definition of a class's key function, uses the
//! class's vtable, which odr-uses every non-pure final overrider in it
//! (clang `MarkVTableUsed`, `DefineUsedVTables` and
//! `MarkVirtualMembersReferenced`). The overriders are marked at the end
//! of the translation unit, interleaved with the pending instantiations
//! (an instantiated definition can use further vtables); a local class's
//! at once.

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DefinitionBits};
use kiln_source::Loc;
use kiln_source::fx::FxHashSet;

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
            self.mark_virtual_members_referenced(loc, def, 0);
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
            {
                continue;
            }
            any = true;
            self.mark_virtual_members_referenced(loc, class, 0);
        }
        any
    }

    /// clang `MarkVirtualMembersReferenced`: odr-uses each non-pure final
    /// overrider in `class`'s vtable at `loc`, then (for the VTT) the
    /// virtual members of each direct base that has virtual bases.
    fn mark_virtual_members_referenced(&mut self, loc: Loc, class: DeclId, depth: usize) {
        if depth > 256 {
            return;
        }
        let Some(v) = graph::ensure_virtuals(&mut self.ast, class) else { return };
        let entries = v.overriders.clone();
        // In clang's `CXXFinalOverriderMap` order (by method).
        let mut seen: FxHashSet<DeclId> = FxHashSet::default();
        let mut finals: Vec<DeclId> = Vec::new();
        for m in graph::method_order(&self.ast, class) {
            for e in entries.iter().filter(|e| self.ast.first(e.1) == m) {
                if seen.insert(e.2) {
                    finals.push(e.2);
                }
            }
        }
        for o in finals {
            if !is_pure(&self.ast, o) {
                self.mark_function_referenced(o, loc);
            }
        }
        let has_vbases = |s: &Self, c: DeclId| {
            s.ast
                .definition(c)
                .and_then(|d| s.ast.decls.record(d))
                .is_some_and(|r| r.defn.contains(DefinitionBits::HAS_VIRTUAL_BASES))
        };
        if !has_vbases(self, class) {
            return;
        }
        let bases: Vec<DeclId> =
            graph::direct_bases(&self.ast, class).into_iter().map(|b| b.class).collect();
        for b in bases {
            if has_vbases(self, b) {
                self.mark_virtual_members_referenced(loc, b, depth + 1);
            }
        }
    }
}
