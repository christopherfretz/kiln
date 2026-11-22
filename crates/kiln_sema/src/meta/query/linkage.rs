//! Linkage and storage duration of a reflection ([basic.link],
//! [basic.stc]; the `has_*_linkage` and `has_*_storage_duration`
//! queries of [meta.reflection.queries]).
//!
//! This is the language rule, not the symbol's ABI linkage
//! (`kiln_abi::mangle::has_internal_linkage`): a local entity has no
//! linkage, whatever its function, except a block-scope function or
//! `extern` variable declaration ([basic.link]/6). `Ast::linkage`
//! (spec/ast.md §4.5) computes it.

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclKind, TlsKind, VarStorage};
use kiln_ast::linkage::LanguageLinkage;
pub(super) use kiln_ast::linkage::Linkage;
use kiln_ast::reflect::Reflectee;

use crate::Sema;

impl Sema<'_, '_> {
    /// The linkage of what `x` represents: a variable, function, type,
    /// template or namespace ([meta.reflection.queries] `has_linkage`);
    /// [`Linkage::None`] for anything else (a type alias, a namespace
    /// alias, a data member, an enumerator, a parameter, a value).
    pub(super) fn linkage_of(&self, x: Reflectee) -> Linkage {
        let d = match x {
            Reflectee::Namespace(d) => return self.ast.linkage(d),
            Reflectee::Type(t) if self.is_alias_type(t) => return Linkage::None,
            Reflectee::Type(t) => {
                match self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t)) {
                    Some(d) if t.quals_bits() == 0 => d,
                    _ => return Linkage::None,
                }
            }
            Reflectee::Template(n) => match self.ast.types.template_decl(n) {
                Some(d) => d,
                None => return Linkage::None,
            },
            Reflectee::Decl(d) => d,
            _ => return Linkage::None,
        };
        self.decl_linkage(d)
    }

    /// Whether `d` has external or module linkage (clang
    /// `isExternallyVisible`, by the language rule).
    pub(crate) fn has_external_linkage(&self, d: DeclId) -> bool {
        matches!(self.decl_linkage(d), Linkage::External | Linkage::Module)
    }

    /// Whether `d` has internal linkage (clang `getFormalLinkage() ==
    /// Internal`).
    pub(crate) fn has_internal_linkage(&self, d: DeclId) -> bool {
        self.decl_linkage(d) == Linkage::Internal
    }

    /// [basic.link]/3-4 for declaration `d` (`Ast::linkage`, spec/ast.md
    /// §4.5); a namespace declaration counts as none here, as its own
    /// reflection (`Reflectee::Namespace`) asks for its linkage.
    fn decl_linkage(&self, d: DeclId) -> Linkage {
        if self.kind_of(d) == DeclKind::Namespace {
            return Linkage::None;
        }
        self.ast.linkage(d)
    }

    /// `has_c_language_linkage`: a function or variable with linkage
    /// declared in an `extern "C"` linkage specification ([dcl.link];
    /// `Ast::language_linkage`).
    pub(super) fn has_c_language_linkage(&self, x: Reflectee) -> bool {
        let Reflectee::Decl(d) = x else { return false };
        self.ast.language_linkage(d) == LanguageLinkage::C
    }

    /// `has_static_storage_duration` ([basic.stc.static]): an object, or
    /// a variable that is not `thread_local` and is not a block-scope
    /// variable without `static` or `extern`.
    pub(super) fn has_static_storage_duration(&self, x: Reflectee) -> bool {
        match x {
            Reflectee::Object(_) => true,
            Reflectee::Decl(d) => {
                matches!(self.kind_of(d), DeclKind::Var | DeclKind::VarTemplateSpecialization)
                    && self.has_static_storage(d)
            }
            _ => false,
        }
    }

    /// `has_thread_storage_duration` ([basic.stc.thread]): a
    /// `thread_local` variable.
    pub(super) fn has_thread_storage_duration(&self, x: Reflectee) -> bool {
        let Reflectee::Decl(d) = x else { return false };
        matches!(self.kind_of(d), DeclKind::Var | DeclKind::VarTemplateSpecialization)
            && self.ast.decls.var(d).is_some_and(|v| v.spec.tls() != TlsKind::None)
    }

    /// `has_automatic_storage_duration` ([basic.stc.auto]): a parameter
    /// variable, or a block-scope variable not declared `static`,
    /// `extern` or `thread_local`.
    pub(super) fn has_automatic_storage_duration(&self, x: Reflectee) -> bool {
        let Reflectee::Decl(d) = x else { return false };
        match self.kind_of(d) {
            DeclKind::ParmVar => true,
            DeclKind::Var => {
                let local =
                    self.ast.decls.get(d).ctx().is_some_and(|c| self.kind_of(c).is_function());
                local
                    && self.ast.decls.var(d).is_some_and(|v| {
                        v.spec.tls() == TlsKind::None
                            && !matches!(v.spec.storage(), VarStorage::Static | VarStorage::Extern)
                    })
            }
            _ => false,
        }
    }
}
