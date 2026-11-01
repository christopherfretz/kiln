//! Unused file-scope variables at the end of the translation unit (clang
//! `ShouldWarnIfUnusedFileScopedDecl` / `ActOnEndOfTranslationUnit`,
//! spec/sema.md "Unused variables"): a namespace-scope variable with
//! internal linkage, declared in the main file and never referenced, is
//! `-Wunused-const-variable` when `const`, else `-Wunused-variable`.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits};
use kiln_ast::DeclId;
use kiln_ast::expr::ExprKind;
use kiln_diag::DiagId;

use crate::Sema;

impl Sema<'_, '_> {
    /// clang's unused file-scoped declarations, for variables: diagnosed
    /// in declaration order, none once an error occurred. A variable
    /// referenced but not odr-used (clang
    /// `-Wunneeded-internal-declaration`), a variable template
    /// (`-Wunused-template`) and a static data member are not diagnosed.
    pub(crate) fn diagnose_unused_file_scope_vars(&mut self) {
        if self.diags.error_count() > 0
            || !(self.diags.is_enabled(DiagId::sema_unused_variable)
                || self.diags.is_enabled(DiagId::sema_unused_const_variable))
        {
            return;
        }
        let mut vars: Vec<DeclId> = Vec::new();
        let mut ctxs = vec![self.ast.tu];
        while let Some(c) = ctxs.pop() {
            for m in self.ast.decls.members(c) {
                match self.ast.decls.get(m).kind() {
                    DeclKind::Namespace | DeclKind::LinkageSpec => ctxs.push(m),
                    DeclKind::Var => vars.push(m),
                    _ => {}
                }
            }
        }
        vars.sort_by_key(|&v| self.ast.decls.get(v).loc());
        // Redeclarations count as one variable, diagnosed at its first
        // declaration that is a definition (else its first).
        let mut seen: Vec<DeclId> = Vec::new();
        for v in vars {
            let first = self.ast.first(v);
            if seen.contains(&first) {
                continue;
            }
            seen.push(first);
            let mut redecls: Vec<DeclId> = self.ast.redecls_of(first).collect();
            redecls.reverse();
            if !self.should_warn_unused_file_scope_var(&redecls) {
                continue;
            }
            let at = redecls
                .iter()
                .copied()
                .find(|&r| self.ast.decls.var(r).is_some_and(|x| x.init.is_some()))
                .unwrap_or(first);
            let n = *self.ast.decls.get(at);
            let is_const = n.ty().is_some_and(|t| {
                let e = self.base_element(t);
                self.ast.types.canonical(e).quals().is_const()
            });
            let id = if is_const {
                DiagId::sema_unused_const_variable
            } else {
                DiagId::sema_unused_variable
            };
            let msg = self.diag(id, n.loc()).arg(self.decl_arg(at));
            self.emit(msg);
        }
    }

    /// clang `ShouldWarnIfUnusedFileScopedDecl` / `ShouldRemoveFromUnused`
    /// for the namespace-scope variable declared by `redecls`.
    fn should_warn_unused_file_scope_var(&mut self, redecls: &[DeclId]) -> bool {
        let first = redecls[0];
        let n = *self.ast.decls.get(first);
        if n.name().as_ident().is_none()
            || self.ast.decls.is_decomposition(first)
            || self.ast.spec_info(first).is_some()
            || self.ast.templ_link(first).get().is_some()
        {
            return false;
        }
        for &r in redecls {
            let d = self.ast.decls.get(r);
            if d.has(DeclFlags::INVALID)
                || d.has(DeclFlags::REFERENCED)
                || d.has(DeclFlags::USED)
                || [AttrKind::UNUSED, AttrKind::MAYBE_UNUSED, AttrKind::USED]
                    .iter()
                    .any(|&k| self.find_attr(r, k).is_some())
            {
                return false;
            }
        }
        if !self.in_main_file(n.loc()) || !kiln_abi::mangle::has_internal_linkage(&self.ast, first) {
            return false;
        }
        // clang `DeclMustBeEmitted`: a variable whose initialization or
        // destruction has side effects is needed (and so is an inline one,
        // which the warning leaves alone).
        let Some(var) = redecls.iter().find_map(|&r| self.ast.decls.var(r).copied()) else {
            return false;
        };
        if var.spec.is_inline() {
            return false;
        }
        let Some(ty) = n.ty() else { return false };
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

    /// clang `isMainFileLoc`: `loc` (its expansion) is in the main file,
    /// not in an included one.
    fn in_main_file(&self, loc: kiln_source::Loc) -> bool {
        let sm = self.diags.sm();
        let e = sm.expansion_loc(loc);
        e.is_valid() && e.is_file() && !sm.include_loc(e).is_valid()
    }
}
