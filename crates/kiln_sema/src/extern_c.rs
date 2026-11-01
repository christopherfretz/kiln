//! Entities with C language linkage across namespaces ([dcl.link]/6-7;
//! spec/sema.md §3, "Language linkage").
//!
//! Two declarations of a function or variable with C language linkage
//! and the same name declare the same entity, whatever namespace each is
//! in ([dcl.link]/6). Redeclaration lookup only sees the current scope,
//! so sema keeps clang's "extern C context": the latest namespace-scope
//! declaration of each C-linkage name ([`Sema::extern_c`]). A new
//! C-linkage declaration that ordinary redeclaration lookup did not pair
//! with a previous one redeclares the recorded entity (clang's
//! `checkForConflictWithNonVisibleExternC`); a variable in global scope
//! and a C-linkage entity of the same name in another namespace conflict
//! ([dcl.link]/7, clang's `checkGlobalOrExternCConflict`).

use kiln_ast::decl::DeclKind;
use kiln_ast::DeclId;
use kiln_ast::names::DeclName;
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;

impl Sema<'_, '_> {
    /// Whether a declaration in context `ctx` has C language linkage
    /// (the innermost enclosing linkage specification is `extern "C"`;
    /// class members and local entities never do), as
    /// `kiln_abi::mangle::is_extern_c` decides for a declared entity.
    pub(crate) fn ctx_is_extern_c(&self, ctx: DeclId) -> bool {
        let mut c = Some(ctx);
        for _ in 0..10_000 {
            let Some(x) = c else { return false };
            match self.kind_of(x) {
                DeclKind::LinkageSpec => {
                    return self.ast.decls.scope(x).is_some_and(|s| {
                        s.linkage_lang() == Some(kiln_ast::decl::LinkageLang::C)
                    });
                }
                DeclKind::Namespace => {}
                _ => return false,
            }
            c = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// Whether `ctx` is the global scope past linkage specifications.
    fn is_global_ctx(&self, ctx: DeclId) -> bool {
        let mut c = Some(ctx);
        for _ in 0..10_000 {
            match c {
                Some(x) if self.kind_of(x) == DeclKind::LinkageSpec => {
                    c = self.ast.decls.get(x).ctx();
                }
                Some(x) => return self.kind_of(x) == DeclKind::TranslationUnit,
                None => return false,
            }
        }
        false
    }

    /// The previous declaration of the C-linkage entity named `name` a
    /// new unqualified namespace-scope declaration in the current
    /// context redeclares when redeclaration lookup found none it
    /// corresponds to ([dcl.link]/6). Diagnoses a conflict with a global
    /// variable ([dcl.link]/7) at `at` instead; `is_var` says whether the
    /// new declaration is a variable.
    pub(crate) fn extern_c_prev(&mut self, name: Symbol, is_var: bool, at: Loc) -> Option<DeclId> {
        if self.in_local_scope() {
            return None;
        }
        let c = self.ctx_is_extern_c(self.ctx);
        let global = self.is_global_ctx(self.ctx);
        if !c && !global {
            return None;
        }
        if let Some(&prev) = self.extern_c.get(&name) {
            if c {
                // clang's notes name a function's first declaration, a
                // variable's latest.
                return Some(if is_var { prev } else { self.ast.first(prev) });
            }
            // A global, non-C declaration: only a variable conflicts
            // (a function overloads, as in clang).
            if is_var {
                self.extern_c_global_conflict(name, 1, at, prev);
            }
            return None;
        }
        if c && !global {
            let tu = self.ast.tu;
            let var = self
                .ast
                .lookup_local(tu, DeclName::ident(name))
                .iter()
                .find(|&d| self.kind_of(d) == DeclKind::Var);
            if let Some(v) = var {
                self.extern_c_global_conflict(name, 0, at, v);
            }
        }
        None
    }

    /// clang's `err_extern_c_global_conflict`: `global` is 1 when the new
    /// declaration is the global one.
    fn extern_c_global_conflict(&mut self, name: Symbol, global: u8, at: Loc, prev: DeclId) {
        let prev = self.ast.first(prev);
        let ploc = self.ast.decls.get(prev).loc();
        let n = crate::decl::sym_str(self, name);
        let note = self.diag(DiagId::note_extern_c_global_conflict, ploc).arg(global);
        let d = self.diag(DiagId::sema_extern_c_global_conflict, at).arg(global).arg(n);
        self.emit(d.with_note(note));
    }

    /// Records `d`, a namespace-scope declaration named `name`, as the
    /// latest declaration of the C-linkage entity of that name when it has
    /// C language linkage (and redeclares the recorded one, if any).
    pub(crate) fn record_extern_c(&mut self, name: Symbol, d: DeclId) {
        if self.in_local_scope()
            || !matches!(self.kind_of(d), DeclKind::Function | DeclKind::Var)
            || self.ast.described_template(d).is_some()
            || self.ast.decls.get(d).flags().contains(kiln_ast::decl::DeclFlags::INVALID)
            || !self.ctx_is_extern_c(self.ctx)
        {
            return;
        }
        match self.extern_c.get(&name) {
            Some(&old) if self.ast.first(old) != self.ast.first(d) => {}
            _ => {
                self.extern_c.insert(name, d);
            }
        }
    }
}
