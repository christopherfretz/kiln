//! An imported inline `used` function named by the importer (WP-1825;
//! spec/modules.md §7 "Imported `used` functions").
//!
//! An inline function that an imported named-module unit owns is not
//! emitted by the importer until the importer names it, as any imported
//! inline function. clang 22 loads such a declaration when a name lookup
//! of the importer finds it, and hands it to codegen at once; GNU `used`
//! (an x86 `interrupt` handler is implicitly `used`) then makes it
//! `DeclMustBeEmitted`, so its `linkonce_odr` definition is emitted right
//! before the namespace-scope declaration that named it, with every
//! other `used` function of that name lookup found (its overloads), named
//! in an evaluated operand or not. kiln records them where a reference is
//! built, at the anchor `TemplateTables::ref_var_insts` gives a variable
//! instantiated at a reference.

use kiln_ast::DeclId;
use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{ConstexprKind, DeclKind};

use crate::Sema;
use crate::lookup::LookupKind;

impl Sema<'_, '_> {
    /// Function `d`, just named by a reference: when it is an imported
    /// inline `used` function, it and the `used` overloads of its name in
    /// its context are handed to codegen before the declaration being
    /// parsed (once each).
    pub(crate) fn note_imported_used_fn(&mut self, d: DeclId) {
        if !self.ast.attrs.has_kind(AttrKind::USED) || !self.imported_used_candidate(d) {
            return;
        }
        let first = self.ast.first(d);
        if self.handed(first) {
            return;
        }
        let Some((anchor, after)) = self.ref_anchor() else { return };
        // A reference while an imported unit's own declarations are
        // replayed is that unit's, not this one's.
        if self.ast.modules.is_imported_named_unit_decl(anchor) {
            return;
        }
        let n = *self.ast.decls.get(d);
        let mut found = vec![first];
        if let Some(ctx) = n.ctx() {
            let r = self.lookup_in_dn(ctx, n.name(), LookupKind::Ordinary);
            for &o in &r.decls {
                let o = self.ast.first(o);
                if o != first && self.imported_used_candidate(o) && !found.contains(&o) {
                    found.push(o);
                }
            }
        }
        // In declaration order, as clang loads a lookup's results.
        found.sort_unstable();
        for f in found {
            if !self.handed(f) {
                self.ast.templates.add_ref_var_inst(anchor, after, f);
            }
        }
    }

    /// Whether function `f` was handed over already (a short scan: the
    /// table holds the few definitions handed over at a reference).
    fn handed(&self, f: DeclId) -> bool {
        self.ast.templates.ref_var_insts().iter().any(|&(_, _, v)| v == f)
    }

    /// Whether `d` is a `used` inline function definition (not a
    /// template, not `consteval`) owned by an imported named-module unit.
    fn imported_used_candidate(&self, d: DeclId) -> bool {
        let ast = &self.ast;
        if !matches!(ast.decls.get(d).kind(), DeclKind::Function | DeclKind::CXXMethod)
            || !ast.modules.is_imported_named_unit_decl(d)
            || ast.inherited_attr(d, AttrKind::USED).is_none()
        {
            return false;
        }
        let Some(def) = ast.definition(d) else { return false };
        ast.decls.function(def).is_some_and(|f| f.body.is_some() && f.templ.get().is_none())
            && ast.redecls_of(d).any(|r| {
                ast.decls.function(r).is_some_and(|f| {
                    f.spec.is_inline()
                        || f.spec.implicit_inline()
                        || f.spec.constexpr_kind() == ConstexprKind::Constexpr
                })
            })
    }
}
