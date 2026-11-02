//! C++20 module initializers (spec/modules.md §7, [basic.start.dynamic]/5,
//! Itanium ABI §3.2.6 / clang `EmitCXXModuleInitFunc`).
//!
//! A module interface or partition unit defines `_ZGIW<module>`: guarded
//! by an internal `i8` `<name>__in_chrg` (when it does anything), it
//! calls the initializers of the named-module units it imports, then
//! runs its own ordered initializers, and is itself a
//! `@llvm.global_ctors` entry. Any other unit's `_GLOBAL__sub_I_<file>`
//! calls its imports' initializers before its own. An imported unit's
//! initializer is called only when it does something (clang
//! `NamedModuleHasInit`): the unit defines a variable clang must emit
//! (`DeclMustBeEmitted`), or imports a unit that does.

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind, TemplLinkKind, VarStorage};
use kiln_ast::modules::UnitKind;
use kiln_llvm::{FuncId, TypeId as LTy};

use crate::module::Cg;

impl Cg<'_> {
    /// The named-module units the main unit imports, in clang's order:
    /// an interface or partition unit lists its `export import`s first
    /// (clang `Primary->Exports`, then `Imports`); any other unit its
    /// imports in source order (`ImportedModules`).
    fn module_import_units(&self) -> Vec<u16> {
        let mt = &self.ast.modules;
        let Some(main) = mt.units.first() else { return Vec::new() };
        let mut keys: Vec<&str> = Vec::new();
        if main.kind.is_interface() || main.kind == UnitKind::PartitionImplementation {
            keys.extend(main.imports.iter().filter(|i| i.1).map(|i| i.0.as_str()));
        }
        keys.extend(main.imports.iter().map(|i| i.0.as_str()));
        let mut out: Vec<u16> = Vec::new();
        for k in keys {
            let Some(u) = mt.unit_by_key(k) else { continue };
            if u != 0
                && mt.units.get(usize::from(u)).is_some_and(|x| x.kind.is_named())
                && !out.contains(&u)
            {
                out.push(u);
            }
        }
        out
    }

    /// Per unit: it defines a variable clang adds to its module's
    /// initializers (`DeclMustBeEmitted`, from any fragment of the unit).
    fn units_with_own_inits(&self) -> Vec<bool> {
        let ast = self.ast;
        let mut own = vec![false; ast.modules.units.len()];
        let mut stack = vec![ast.tu];
        while let Some(ctx) = stack.pop() {
            for d in ast.decls.members(ctx) {
                let n = ast.decls.get(d);
                if n.has(DeclFlags::INVALID) || n.has(DeclFlags::DEPENDENT_CTX) {
                    continue;
                }
                match n.kind() {
                    DeclKind::Namespace | DeclKind::LinkageSpec | DeclKind::Export => {
                        stack.push(d);
                    }
                    DeclKind::Var => {
                        let u = usize::from(ast.modules.owner(d).unit);
                        if u != 0 && !own[u] && self.var_is_module_initializer(d) {
                            own[u] = true;
                        }
                    }
                    _ => {}
                }
            }
        }
        own
    }

    /// clang `DeclMustBeEmitted` for namespace-scope variable `d`: a
    /// definition that is not discardable, or whose initialization or
    /// destruction has side effects.
    fn var_is_module_initializer(&self, d: DeclId) -> bool {
        let ast = self.ast;
        let Some(v) = ast.decls.var(d) else { return false };
        let ext = v.spec.storage() == VarStorage::Extern || ast.decls.is_single_line_linkage(d);
        if (ext && v.init.is_none())
            || matches!(v.templ.get(), Some((TemplLinkKind::Pattern, _)))
            || self.is_templated(d)
            || self.is_explicit_inst_declaration(ast.first(d))
        {
            return false;
        }
        !self.discardable_var(d) || self.var_must_emit(d)
    }

    /// Per unit: its initializer does something (clang
    /// `NamedModuleHasInit`): own initializers, or a named-module import
    /// whose initializer does.
    fn units_needing_init(&self) -> Vec<bool> {
        let mt = &self.ast.modules;
        let own = self.units_with_own_inits();
        let n = mt.units.len();
        // Imports precede importers in unit order except through cycles
        // (diagnosed); iterate to a fixpoint.
        let mut has = own;
        loop {
            let mut changed = false;
            for u in 1..n {
                if has[u] {
                    continue;
                }
                let any = mt.units[u].imports.iter().any(|(k, _)| {
                    mt.unit_by_key(k).is_some_and(|v| {
                        let v = usize::from(v);
                        v != 0 && v < n && mt.units[v].kind.is_named() && has[v]
                    })
                });
                if any {
                    has[u] = true;
                    changed = true;
                }
            }
            if !changed {
                return has;
            }
        }
    }

    /// Declarations of the `_ZGIW` initializers of the imported units
    /// that do something, in call order (clang creates them right
    /// before the calling function).
    pub(crate) fn imported_module_inits(&mut self) -> Vec<FuncId> {
        if !self.ast.modules.active {
            return Vec::new();
        }
        let units = self.module_import_units();
        if units.is_empty() {
            return Vec::new();
        }
        let has = self.units_needing_init();
        let mut out = Vec::new();
        for u in units {
            if !has.get(usize::from(u)).copied().unwrap_or(false) {
                continue;
            }
            let info = &self.ast.modules.units[usize::from(u)];
            let Some(m) = info.module.as_deref() else { continue };
            let name =
                kiln_abi::mangle::mangle_module_initializer(self.ast, m, info.partition.as_deref());
            if let Some(f) = self.m.get_function(&name) {
                out.push(f);
                continue;
            }
            let ty = self.m.types.function(LTy::VOID, &[], false);
            if let Ok(f) = self.m.add_function(&name, ty) {
                out.push(f);
            }
        }
        out
    }

    /// The name of the main unit's own `_ZGIW` initializer, when it is a
    /// module interface or partition unit (clang `isInterfaceOrPartition`).
    pub(crate) fn own_module_init_name(&self) -> Option<String> {
        let mt = &self.ast.modules;
        if !mt.active {
            return None;
        }
        let main = mt.units.first()?;
        if !(main.kind.is_interface() || main.kind == UnitKind::PartitionImplementation) {
            return None;
        }
        let m = main.module.as_deref()?;
        Some(kiln_abi::mangle::mangle_module_initializer(self.ast, m, main.partition.as_deref()))
    }
}
