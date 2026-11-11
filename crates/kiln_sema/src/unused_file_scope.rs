//! Unused file-scope variables at the end of the translation unit (clang
//! `ShouldWarnIfUnusedFileScopedDecl` / `ActOnEndOfTranslationUnit`,
//! spec/sema.md "Unused variables"): a namespace-scope variable or static
//! data member with internal linkage, declared in the main file and never
//! odr-used, is `-Wunneeded-internal-declaration` when referenced,
//! `-Wunused-template` when a variable template, `-Wunused-const-variable`
//! when `const`, else `-Wunused-variable`.

use kiln_ast::DeclId;
use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits};
use kiln_ast::expr::ExprKind;
use kiln_ast::templates::SpecKind;
use kiln_diag::DiagId;

use crate::Sema;

/// What [`Sema::unused_file_scope_kind`] found to say about a variable.
enum Unused {
    /// Referenced but not odr-used: `-Wunneeded-internal-declaration`.
    Unneeded,
    /// A never-referenced variable template: `-Wunused-template`.
    Template,
    /// Never referenced: `-Wunused-variable` / `-Wunused-const-variable`.
    Variable,
}

impl Sema<'_, '_> {
    /// clang's unused file-scoped declarations, for variables (clang
    /// `ActOnEndOfTranslationUnit`), in the order of their first
    /// declarations, none once an error occurred: namespace-scope
    /// variables, variable templates and static data members (of classes
    /// with internal linkage, and out-of-line explicit specializations of
    /// those of class templates). Diagnosed at the definition, else the
    /// first declaration.
    pub(crate) fn diagnose_unused_file_scope_vars(&mut self) {
        if self.diags.error_count() > 0
            || ![
                DiagId::sema_unused_variable,
                DiagId::sema_unused_const_variable,
                DiagId::sema_unneeded_internal_decl,
                DiagId::sema_unused_variable_template,
            ]
            .iter()
            .any(|&id| self.diags.is_enabled(id))
        {
            return;
        }
        let mut vars: Vec<DeclId> = Vec::new();
        let mut ctxs = vec![self.ast.tu];
        while let Some(c) = ctxs.pop() {
            for m in self.ast.decls.members(c) {
                match self.ast.decls.get(m).kind() {
                    DeclKind::Namespace | DeclKind::LinkageSpec => ctxs.push(m),
                    DeclKind::CXXRecord if self.ast.described_template(m).is_none() => {
                        ctxs.push(m);
                    }
                    DeclKind::Var => vars.push(m),
                    DeclKind::VarTemplate => {
                        if let Some(p) = self.ast.template_info(m).and_then(|i| i.pattern) {
                            vars.push(p);
                        }
                    }
                    _ => {}
                }
            }
        }
        vars.sort_by_key(|&v| self.ast.decls.get(v).loc());
        // Redeclarations count as one variable, diagnosed at its first
        // declaration that is a definition (else at its first explicit
        // specialization declaration, [temp.expl.spec]/14; else its first).
        let mut seen: Vec<DeclId> = Vec::new();
        for v in vars {
            let first = self.ast.first(v);
            if seen.contains(&first) {
                continue;
            }
            seen.push(first);
            let mut redecls: Vec<DeclId> = self.ast.redecls_of(first).collect();
            redecls.reverse();
            let Some(kind) = self.unused_file_scope_kind(&redecls) else { continue };
            let at = redecls
                .iter()
                .copied()
                .find(|&r| self.is_var_definition(r))
                .or_else(|| {
                    redecls.iter().copied().find(|&r| {
                        self.is_member_spec_declaration(r) && !self.is_in_class_static(r)
                    })
                })
                .unwrap_or(first);
            let n = *self.ast.decls.get(at);
            let msg = match kind {
                Unused::Unneeded => self
                    .diag(DiagId::sema_unneeded_internal_decl, n.loc())
                    .arg("1")
                    .arg(self.decl_arg(at)),
                Unused::Template => {
                    self.diag(DiagId::sema_unused_variable_template, n.loc()).arg(self.decl_arg(at))
                }
                Unused::Variable => {
                    let is_const = n.ty().is_some_and(|t| {
                        let e = self.base_element(t);
                        self.ast.types.canonical(e).quals().is_const()
                    });
                    let id = if is_const {
                        DiagId::sema_unused_const_variable
                    } else {
                        DiagId::sema_unused_variable
                    };
                    self.diag(id, n.loc()).arg(self.decl_arg(at))
                }
            };
            self.emit(msg);
        }
    }

    /// clang `ShouldWarnIfUnusedFileScopedDecl` / `ShouldRemoveFromUnused`
    /// for the variable declared by `redecls`: what to diagnose, if
    /// anything.
    fn unused_file_scope_kind(&mut self, redecls: &[DeclId]) -> Option<Unused> {
        let first = redecls[0];
        let n = *self.ast.decls.get(first);
        let template = self.ast.described_template(first);
        if n.name().as_ident().is_none()
            || self.ast.decls.is_decomposition(first)
            || self.ast.spec_info(first).is_some()
            || self.ast.partial_spec_info(first).is_some()
            || n.has(DeclFlags::USED)
            || self.in_templated_class(n.ctx())
            || !self.static_member_may_warn(redecls)
        {
            return None;
        }
        let mut referenced = false;
        for &r in redecls {
            let d = self.ast.decls.get(r);
            if d.has(DeclFlags::INVALID)
                || d.has(DeclFlags::USED)
                // A `weakref` declaration defines nothing (clang emits no
                // unused-entity warning for one).
                || [AttrKind::UNUSED, AttrKind::MAYBE_UNUSED, AttrKind::USED, AttrKind::WEAKREF]
                    .iter()
                    .any(|&k| self.find_attr(r, k).is_some())
            {
                return None;
            }
            referenced |= d.has(DeclFlags::REFERENCED);
        }
        if !self.in_main_file(n.loc()) || !kiln_abi::mangle::has_internal_linkage(&self.ast, first)
        {
            return None;
        }
        // A referenced variable whose value a constant expression may
        // need is left alone (clang's approximation).
        if referenced && self.might_be_constant_var(first) {
            return None;
        }
        if let Some(t) = template {
            // A variable template any specialization of which is used (or
            // referenced and possibly constant), or explicitly specialized.
            let t = self.ast.first(t);
            let specs = self.ast.templates.specs_of(t).to_vec();
            let needed = specs.iter().any(|&s| {
                let n = self.ast.decls.get(self.ast.first(s));
                n.has(DeclFlags::USED)
                    || (n.has(DeclFlags::REFERENCED) && self.might_be_constant_var(s))
                    || self
                        .ast
                        .spec_info(s)
                        .is_some_and(|(_, i)| i.kind == SpecKind::ExplicitSpecialization)
            });
            if needed {
                return None;
            }
        } else if !self.may_omit_file_scope_var(redecls) {
            return None;
        }
        Some(if referenced {
            Unused::Unneeded
        } else if template.is_some() {
            Unused::Template
        } else {
            Unused::Variable
        })
    }

    /// clang `DeclMustBeEmitted` (negated) for the non-template variable
    /// declared by `redecls`: one whose initialization or destruction has
    /// side effects is needed.
    fn may_omit_file_scope_var(&mut self, redecls: &[DeclId]) -> bool {
        let first = redecls[0];
        if redecls.iter().all(|&r| self.ast.decls.var(r).is_none()) {
            return false;
        }
        let Some(ty) = self.ast.decls.get(first).ty() else { return false };
        if self.ast.types.is_dependent(ty) || !self.is_complete(ty) {
            return false;
        }
        let elem = self.base_element(ty);
        if let Some(r) = self.ast.types.as_record(elem) {
            let Some(def) = self.ast.definition(r) else { return false };
            if !self.defn_bits(def).contains(DefinitionBits::TRIVIAL_DTOR) {
                return false;
            }
        }
        // An initializer with possible side effects (approximated: one that
        // is not a constant initializer nor a trivial construction).
        for &r in redecls {
            let Some(init) = self.ast.decls.var(r).and_then(|x| x.init) else { continue };
            let trivial = self.ast.exprs.get(init).kind() == ExprKind::CXXConstructExpr
                && self
                    .ast
                    .decls
                    .function(self.ast.exprs.constructor(init))
                    .is_some_and(|f| f.spec.is_trivial());
            if !trivial && !self.is_constant_initialized(r) {
                return false;
            }
        }
        true
    }

    /// clang's static data member cases of
    /// `ShouldWarnIfUnusedFileScopedDecl`: a member of an implicit
    /// instantiation is not diagnosed, nor an explicitly specialized one
    /// without an out-of-line declaration.
    fn static_member_may_warn(&self, redecls: &[DeclId]) -> bool {
        let kinds: Vec<SpecKind> = redecls
            .iter()
            .filter_map(|&r| self.ast.member_spec_info(r).map(|(_, m)| m.kind))
            .collect();
        if kinds.is_empty() {
            return true;
        }
        if kinds.contains(&SpecKind::ExplicitSpecialization) {
            return redecls.iter().any(|&r| !self.is_in_class_static(r));
        }
        false
    }

    /// clang `isMainFileLoc`: `loc` (its expansion) is in the main file,
    /// not in an included one.
    pub(crate) fn in_main_file(&self, loc: kiln_source::Loc) -> bool {
        let sm = self.diags.sm();
        let e = sm.expansion_loc(loc);
        e.is_valid() && e.is_file() && !sm.include_loc(e).is_valid()
    }
}
