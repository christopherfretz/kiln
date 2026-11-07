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

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclKind, LinkageLang};
use kiln_ast::names::DeclName;
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;

impl Sema<'_, '_> {
    /// Whether a declaration in context `ctx` has C language linkage
    /// (the innermost lexically enclosing linkage specification is
    /// `extern "C"`; class members and local entities never do), as
    /// `kiln_abi::mangle::is_extern_c` decides for a declared entity.
    /// Namespaces are walked through their lexical contexts: a namespace
    /// reopened inside `extern "C++"` within `extern "C"` has the
    /// first definition's semantic context but C++ linkage (libstdc++'s
    /// `bits/std_abs.h` overloading `std::abs` after `<stdlib.h>` was
    /// included inside `extern "C"`).
    pub(crate) fn ctx_is_extern_c(&self, ctx: DeclId) -> bool {
        let mut c = Some(ctx);
        for _ in 0..10_000 {
            let Some(x) = c else { return false };
            match self.kind_of(x) {
                DeclKind::LinkageSpec => {
                    return self
                        .ast
                        .decls
                        .scope(x)
                        .is_some_and(|s| s.linkage_lang() == Some(LinkageLang::C));
                }
                DeclKind::Namespace => {}
                _ => return false,
            }
            c = self.ast.decls.lexical_ctx(x);
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
    /// new declaration is a variable, `is_static` whether it is declared
    /// `static` (then it has internal linkage and no language linkage,
    /// [dcl.link]/5: `namespace N { extern "C" { static int x; } }` is
    /// another entity than `::x`).
    pub(crate) fn extern_c_prev(
        &mut self,
        name: Symbol,
        is_var: bool,
        is_static: bool,
        at: Loc,
    ) -> Option<DeclId> {
        if self.in_local_scope() || is_static {
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
            || kiln_abi::mangle::has_internal_linkage(&self.ast, d)
            || !self.ast.pass_object_size_params(d).is_empty()
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

    /// The language of the innermost linkage specification lexically
    /// enclosing context `ctx`, through any kind of context (clang's
    /// `isLinkageSpecContext`).
    fn enclosing_linkage_lang(&self, ctx: DeclId) -> Option<LinkageLang> {
        let mut c = Some(ctx);
        for _ in 0..10_000 {
            let x = c?;
            match self.kind_of(x) {
                DeclKind::LinkageSpec => return self.ast.decls.scope(x)?.linkage_lang(),
                DeclKind::TranslationUnit => return None,
                _ => {}
            }
            // Lexically: a namespace first defined inside `extern "C"` and
            // reopened at file scope encloses no linkage specification there.
            c = self.ast.decls.lexical_ctx(x);
        }
        None
    }

    /// [dcl.link]/6: a redeclaration of namespace-scope function or
    /// variable `old` in the current context gives it a different
    /// language linkage: `old` has C++ language linkage and the new
    /// declaration is in an `extern "C"` specification, or `old` has C
    /// language linkage (its first declaration is in one) and the new
    /// one is in an `extern "C++"` specification (clang's
    /// `haveIncompatibleLanguageLinkages`). An entity with internal
    /// linkage has no language linkage; a redeclaration with no linkage
    /// specification keeps the old one.
    pub(crate) fn has_incompatible_language_linkage(&self, old: DeclId) -> bool {
        let first = self.ast.first(old);
        let in_record = self.ast.decls.get(first).ctx().is_some_and(|c| {
            matches!(
                self.kind_of(c),
                DeclKind::CXXRecord
                    | DeclKind::ClassTemplateSpecialization
                    | DeclKind::ClassTemplatePartialSpecialization
            )
        });
        if in_record || kiln_abi::mangle::has_internal_linkage(&self.ast, first) {
            return false;
        }
        let old_c = kiln_abi::mangle::is_extern_c(&self.ast, first);
        match self.enclosing_linkage_lang(self.ctx) {
            Some(LinkageLang::C) => !old_c,
            Some(LinkageLang::Cxx) => old_c,
            None => false,
        }
    }

    /// Reports [`Self::has_incompatible_language_linkage`] for the
    /// redeclaration of `old` named `name` at `at`
    /// (`err_different_language_linkage`); `def` says whether the note
    /// calls `old` a definition. True if reported.
    pub(crate) fn diagnose_language_linkage_redecl(
        &mut self,
        old: DeclId,
        name: &str,
        at: Loc,
        def: bool,
    ) -> bool {
        if !self.has_incompatible_language_linkage(old) {
            return false;
        }
        let msg = self.diag(DiagId::sema_different_language_linkage, at).arg(name);
        let msg = self.note_prev(msg, old, def);
        self.emit(msg);
        true
    }
}
