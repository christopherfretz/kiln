//! Imported entities (clang `EmitUsingDirective`, `EmitUsingDecl`,
//! `EmitUsingShadowDecl`, `EmitNamespaceAlias`; WP-1365): the
//! `DIImportedEntity` nodes gdb and lldb use for lookup through
//! `using namespace N;`, `using N::x;` and `namespace A = N;`.
//!
//! Only with reduced debug information (clang `hasReducedDebugInfo`):
//! nothing at `-gline-tables-only`. A namespace-scope entity goes to the
//! compile unit's `imports:`; a block-scope one is scoped to the innermost
//! lexical block and kept in its subprogram's `retainedNodes` (LLVM's
//! `DIBuilder` tracks local imported entities per subprogram).

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::types::TypeView;
use kiln_llvm::{MdField, MdNode, MetadataId};

use super::{Fields, PendingImport};
use crate::module::Cg;

/// `DW_TAG_imported_module` (a using-directive).
const MODULE: &str = "DW_TAG_imported_module";
/// `DW_TAG_imported_declaration` (a using-declaration, a namespace alias).
const DECLARATION: &str = "DW_TAG_imported_declaration";

/// What a using-declaration's shadow makes the imported entity's
/// `entity:` (clang `getDeclarationOrDefinition`).
enum Target {
    /// A type, known now (clang `getOrCreateType`).
    Node(MetadataId),
    /// A function or variable: its definition if the unit emits one by
    /// its end, else a declaration (clang's forward declarations replaced
    /// in `finalize`).
    Deferred(DeclId),
}

impl Cg<'_> {
    /// clang `CodeGenModule::EmitTopLevelDecl` for a using-directive,
    /// using-declaration or namespace alias `d` at namespace scope.
    pub(crate) fn di_using_top(&mut self, d: DeclId) {
        if !self.di_reduced() || self.ast.modules.is_imported_named_unit_decl(d) {
            return;
        }
        // clang `getCurrentContextDescriptor` -> `getContextDescriptor`:
        // a namespace, else the compile unit (a linkage specification or
        // an export block is no scope of its own).
        let ast = self.ast;
        let scope = match ast.decls.get(d).ctx() {
            Some(c) if ast.decls.get(c).kind() == DeclKind::Namespace => {
                self.di_namespace(ast.first(c))
            }
            _ => match self.di.as_deref() {
                Some(x) => x.cu,
                None => return,
            },
        };
        self.di_using(d, scope, false);
    }

    /// clang `CodeGenFunction::EmitDecl` for a using-directive,
    /// using-declaration or namespace alias `d` in a function body being
    /// emitted: scoped to the innermost lexical block.
    pub(crate) fn di_using_local(&mut self, d: DeclId) {
        if !self.di_reduced() {
            return;
        }
        let Some(scope) = self.f.fir.dbg.as_deref().map(crate::fir::FirDbg::scope) else {
            return;
        };
        self.di_using(d, scope, true);
    }

    fn di_using(&mut self, d: DeclId, scope: MetadataId, local: bool) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        if n.has(DeclFlags::INVALID) {
            return;
        }
        match n.kind() {
            DeclKind::UsingDirective => {
                // clang `EmitUsingDirective`: not for an anonymous
                // namespace (its implicit using-directive).
                let Some(ns) = ast.using_target(d).map(|t| ast.resolve_alias(t)) else { return };
                if ast.decls.get(ns).kind() != DeclKind::Namespace || self.di_ident(ns).is_empty() {
                    return;
                }
                let entity = self.di_namespace(ast.first(ns));
                self.di_import(MODULE, None, scope, entity, n.loc(), local);
            }
            DeclKind::NamespaceAlias => {
                let _ = self.di_namespace_alias(d, scope, local);
            }
            DeclKind::Using => self.di_using_decl(d, scope, local),
            // clang `EmitUsingEnumDecl`: each shadow names an enumerator,
            // which has no entity (`getDeclarationOrDefinition` is null).
            _ => {}
        }
    }

    /// clang `EmitNamespaceAlias` (cached per alias): an alias of an
    /// alias imports the underlying alias's entity.
    fn di_namespace_alias(
        &mut self,
        d: DeclId,
        scope: MetadataId,
        local: bool,
    ) -> Option<MetadataId> {
        if let Some(&e) = self.di.as_deref().and_then(|x| x.ns_aliases.get(&d)) {
            return Some(e);
        }
        let ast = self.ast;
        let target = ast.using_target(d)?;
        let entity = match ast.decls.get(target).kind() {
            DeclKind::NamespaceAlias => self.di_namespace_alias(target, scope, local)?,
            DeclKind::Namespace => self.di_namespace(ast.first(target)),
            _ => return None,
        };
        let name = self.di_ident(d);
        let e =
            self.di_import(DECLARATION, Some(&name), scope, entity, ast.decls.get(d).loc(), local);
        if let Some(x) = self.di.as_deref_mut() {
            x.ns_aliases.insert(d, e);
        }
        Some(e)
    }

    /// clang `EmitUsingDecl`: one entity, for the first shadow in clang's
    /// order (the most recently declared entity first) that is not a
    /// function whose return type is still to be deduced; "debuggers can
    /// detect that this is an overloaded name". A shadow naming a
    /// template, an enumerator or anything else without an entity emits
    /// nothing, and ends the search all the same.
    fn di_using_decl(&mut self, d: DeclId, scope: MetadataId, local: bool) {
        let ast = self.ast;
        let mut targets: Vec<DeclId> =
            self.using_shadows(d).map(|s| self.shadow_target(s)).collect();
        targets.sort_by_key(|&t| std::cmp::Reverse(ast.first(t)));
        let Some(t) = targets.into_iter().find(|&t| !self.undeduced_at(t, d, local)) else {
            return;
        };
        let target = match ast.decls.get(t).kind() {
            k if k.is_function() => Target::Deferred(t),
            DeclKind::Var => Target::Deferred(t),
            DeclKind::CXXRecord
            | DeclKind::Enum
            | DeclKind::Typedef
            | DeclKind::TypeAlias
            | DeclKind::ClassTemplateSpecialization => {
                match ast.decls.get(t).ty().and_then(|q| self.di_type(q)) {
                    Some(m) => Target::Node(m),
                    None => return,
                }
            }
            _ => return,
        };
        let loc = ast.decls.get(d).loc();
        match target {
            Target::Node(entity) => {
                let _ = self.di_import(DECLARATION, None, scope, entity, loc, local);
            }
            Target::Deferred(t) => self.di_import_deferred(scope, t, loc, local),
        }
    }

    /// The shadows using-declaration `d` introduced: sema creates them
    /// right after it, in its context (`act_on_using_declarator`).
    fn using_shadows(&self, d: DeclId) -> impl Iterator<Item = DeclId> + '_ {
        let ast = self.ast;
        let ctx = ast.decls.get(d).ctx();
        let last = u32::try_from(ast.decls.len()).unwrap_or(u32::MAX);
        (d.index() + 1..=last).map_while(DeclId::try_new).take_while(move |&s| {
            let n = ast.decls.get(s);
            n.kind() == DeclKind::UsingShadow && n.ctx() == ctx
        })
    }

    /// The declaration shadow `s` names, through shadows of shadows
    /// (clang `getUnderlyingDecl`).
    fn shadow_target(&self, s: DeclId) -> DeclId {
        let ast = self.ast;
        let mut t = s;
        for _ in 0..64 {
            let next = ast.strip_shadow(t);
            if next == t {
                break;
            }
            t = next;
        }
        t
    }

    /// clang `EmitUsingDecl`'s skip: function `t` has a return type to
    /// deduce that is not deduced yet where clang emits the
    /// using-declaration `using` (when its top-level declaration ends, or
    /// when its function body is emitted). Approximated by declaration
    /// order: the definition is after the using-declaration; a body that
    /// clang defers (an inline or internal function, a template
    /// instantiation) sees every definition of the unit.
    fn undeduced_at(&self, t: DeclId, using: DeclId, local: bool) -> bool {
        let ast = self.ast;
        if !ast.decls.get(t).kind().is_function() || !self.declared_auto_return(t) {
            return false;
        }
        let Some(def) = ast.definition(t) else { return true };
        if def == t {
            return false;
        }
        if local && self.f.decl.is_some_and(|f| self.body_deferred(f)) {
            return false;
        }
        def > using
    }

    /// The return type as declared by `t` is a placeholder (`auto f();`;
    /// sema records the deduced type on the earlier declarations too).
    fn declared_auto_return(&self, t: DeclId) -> bool {
        let ast = self.ast;
        if ast.decls.function(t).is_some_and(|x| x.spec.return_deduced()) {
            return true;
        }
        let Some(q) = ast.decls.get(t).ty() else { return false };
        let mut ty = q.ty();
        for _ in 0..64 {
            match ast.types.view(ty) {
                TypeView::Paren(i) | TypeView::Attributed { modified: i, .. } => ty = i.ty(),
                TypeView::Function(fp) => {
                    return matches!(
                        ast.types.view(fp.ret().ty()),
                        TypeView::Auto { .. } | TypeView::DeducedTemplateSpecialization { .. }
                    );
                }
                _ => return false,
            }
        }
        false
    }

    /// clang `EmitGlobal` defers function `f`'s body to the end of the
    /// unit (or its first use): not an external non-inline function.
    fn body_deferred(&self, f: DeclId) -> bool {
        let ast = self.ast;
        let first = ast.first(f);
        kiln_abi::mangle::has_internal_linkage(ast, first)
            || ast.spec_info(f).is_some()
            || ast
                .redecls_of(first)
                .any(|r| ast.decls.function(r).is_some_and(|x| x.spec.is_inline()))
            || ast.decls.get(f).has(DeclFlags::IMPLICIT)
    }

    /// `DIBuilder::createImportedModule` / `createImportedDeclaration`:
    /// a new entity is kept by the compile unit (namespace scope) or by
    /// the current subprogram (`local`); an identical one is not kept
    /// twice.
    fn di_import(
        &mut self,
        tag: &'static str,
        name: Option<&str>,
        scope: MetadataId,
        entity: MetadataId,
        loc: kiln_source::Loc,
        local: bool,
    ) -> MetadataId {
        let fields = self.di_import_fields(tag, name, scope, Some(entity), loc);
        let e = self.m.md_specialized("DIImportedEntity", false, fields.0);
        self.di_keep_import(e, local, None);
        e
    }

    /// A using-declaration of function or variable `t`: a placeholder
    /// whose entity [`Self::di_resolve_imports`] fills in at the end.
    fn di_import_deferred(
        &mut self,
        scope: MetadataId,
        t: DeclId,
        loc: kiln_source::Loc,
        local: bool,
    ) {
        let (file, line) = (self.di_file(loc), self.di_line(loc));
        let key = (scope, self.ast.first(t), file, line);
        if self.di.as_deref().is_some_and(|x| x.import_keys.contains(&key)) {
            return;
        }
        let fields = self.di_import_fields(DECLARATION, None, scope, None, loc);
        let p = self.m.md_specialized("DIImportedEntity", true, fields.0.clone());
        if let Some(x) = self.di.as_deref_mut() {
            x.import_keys.insert(key);
            x.pending_imports.push(PendingImport { node: p, target: t, fields: fields.0 });
        }
        self.di_keep_import(p, local, Some(()));
    }

    fn di_import_fields(
        &mut self,
        tag: &'static str,
        name: Option<&str>,
        scope: MetadataId,
        entity: Option<MetadataId>,
        loc: kiln_source::Loc,
    ) -> Fields {
        let (file, line) = (self.di_file(loc), self.di_line(loc));
        let mut f = Fields::default().raw("tag", tag);
        if let Some(n) = name.filter(|n| !n.is_empty()) {
            f = f.s("name", n);
        }
        f.n("scope", Some(scope))
            .nn("entity", entity)
            .n("file", Some(file))
            .u("line", u64::from(line))
    }

    /// Adds entity `e` to its list unless already there (`fresh`: known
    /// to be new).
    fn di_keep_import(&mut self, e: MetadataId, local: bool, fresh: Option<()>) {
        if local {
            if let Some(x) = self.f.fir.dbg.as_deref_mut()
                && (fresh.is_some() || !x.vars.contains(&e))
            {
                x.tracked = true;
                x.vars.push(e);
            }
        } else if let Some(x) = self.di.as_deref_mut()
            && (fresh.is_some() || !x.imported.contains(&e))
        {
            x.imported.push(e);
        }
    }

    /// clang `finalize`'s `FwdDeclReplaceMap`: each deferred entity names
    /// its function's or variable's definition when the unit emitted one,
    /// else a declaration (`getFunctionForwardDeclaration`,
    /// `getGlobalVariableForwardDeclaration`).
    pub(crate) fn di_resolve_imports(&mut self) {
        let Some(pending) = self.di.as_deref_mut().map(|x| std::mem::take(&mut x.pending_imports))
        else {
            return;
        };
        for PendingImport { node, target, mut fields } in pending {
            let first = self.ast.first(target);
            let entity = if self.ast.decls.get(target).kind() == DeclKind::Var {
                match self.di.as_deref().and_then(|x| x.gves.get(&first)).copied() {
                    Some(gve) => self.gve_var(gve),
                    None => self.di_var_fwd_decl(target),
                }
            } else {
                match self.di.as_deref().and_then(|x| x.regions.get(&first)).copied() {
                    Some(sp) => Some(sp),
                    None => self.di_fn_fwd_decl(target),
                }
            };
            let Some(entity) = entity else { continue };
            if let Some(f) = fields.iter_mut().find(|f| f.0 == "entity") {
                f.1 = MdField::Node(Some(entity));
            }
            let new = MdNode::Specialized {
                distinct: false,
                name: "DIImportedEntity",
                fields: fields.into_boxed_slice(),
            };
            let _ = self.m.md_replace(node, new);
        }
    }

    /// The variable of global variable expression `gve`.
    fn gve_var(&self, gve: MetadataId) -> Option<MetadataId> {
        match self.m.md_node(gve) {
            MdNode::Specialized { fields, .. } => fields.iter().find_map(|f| match f {
                ("var", MdField::Node(v)) => *v,
                _ => None,
            }),
            _ => None,
        }
    }

    /// clang `getFunctionFwdDeclOrStub(GD, false)`: a declaration-only
    /// subprogram of function `t` (no unit, no scope line).
    fn di_fn_fwd_decl(&mut self, t: DeclId) -> Option<MetadataId> {
        let ast = self.ast;
        let n = ast.decls.get(t);
        let loc = n.loc();
        let unit = self.di_file(loc);
        let name = self.di_fn_name(t);
        let link = kiln_abi::mangle::mangle_decl(ast, &mut self.anon, ast.first(t));
        let scope = match n.ctx() {
            Some(c) if ast.decls.get(c).kind() == DeclKind::Namespace => {
                self.di_namespace(ast.first(c))
            }
            _ => unit,
        };
        let ty = if let Some(t) = n.ty().and_then(|q| self.di_type(q)) {
            t
        } else {
            let e = self.m.md_tuple(&[]);
            self.m.md_specialized(
                "DISubroutineType",
                false,
                Fields::default().n("types", Some(e)).0,
            )
        };
        let mut fl = vec!["DIFlagPrototyped"];
        let noreturn = self.has_fn_attr(t, kiln_ast::attr::AttrKind::NORETURN)
            || matches!(n.ty().map(|q| ast.types.view(q.ty())), Some(TypeView::Function(fp)) if fp.info().noreturn());
        if noreturn {
            fl.push("DIFlagNoReturn");
        }
        let mut sp: Vec<&str> = Vec::new();
        if self.di_internal(t) {
            sp.push("DISPFlagLocalToUnit");
        }
        if self.opts.opt_level > 0 {
            sp.push("DISPFlagOptimized");
        }
        let tparams = self.di_template_params(t);
        let mut f = Fields::default().s("name", &name);
        if link != name {
            f = f.s("linkageName", &link);
        }
        f = f.n("scope", Some(scope)).n("file", Some(unit)).u("line", u64::from(self.di_line(loc)));
        f = f.n("type", Some(ty)).flags("flags", &fl).sp_flags(&sp).n("templateParams", tparams);
        Some(self.m.md_specialized("DISubprogram", false, f.0))
    }

    /// clang `getGlobalVariableForwardDeclaration`: a declaration of
    /// variable `t` (`isDefinition: false`).
    fn di_var_fwd_decl(&mut self, t: DeclId) -> Option<MetadataId> {
        let ast = self.ast;
        let n = ast.decls.get(t);
        let loc = n.loc();
        let q = n.ty()?;
        // clang `collectVarDeclProps`: `T[]` is described as `T[1]`.
        let ty = if matches!(ast.types.view(q.ty()), TypeView::IncompleteArray(_)) {
            self.di_array_counted(q, true)?
        } else {
            self.di_type(q)?
        };
        let (file, line) = (self.di_file(loc), self.di_line(loc));
        let name = self.di_ident(t);
        let mangled = kiln_abi::mangle::mangle_decl(ast, &mut self.anon, ast.first(t));
        let scope = self.di_scope(t);
        let tparams = self.di_template_params(t);
        let mut f = Fields::default().s("name", &name);
        if mangled != name {
            f = f.s("linkageName", &mangled);
        }
        f = f
            .n("scope", scope)
            .n("file", Some(file))
            .u("line", u64::from(line))
            .n("type", Some(ty));
        f = f
            .b("isLocal", self.di_internal(t))
            .b("isDefinition", false)
            .n("templateParams", tparams);
        Some(self.m.md_specialized("DIGlobalVariable", false, f.0))
    }
}
