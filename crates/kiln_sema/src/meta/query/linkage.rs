//! Linkage and storage duration of a reflection ([basic.link],
//! [basic.stc]; the `has_*_linkage` and `has_*_storage_duration`
//! queries of [meta.reflection.queries]).
//!
//! This is the language rule, not the symbol's ABI linkage
//! (`kiln_abi::mangle::has_internal_linkage`): a local entity has no
//! linkage, whatever its function.

use kiln_ast::decl::{DeclKind, NamespaceData, TlsKind, VarStorage};
use kiln_ast::reflect::Reflectee;
use kiln_ast::{DeclId, QualType};

use crate::Sema;

/// The linkage of a name ([basic.link]/2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Linkage {
    None,
    Internal,
    Module,
    External,
}

/// How many context links a walk follows at most.
const MAX_CTX: usize = 1024;

impl Sema<'_, '_> {
    /// The linkage of what `x` represents: a variable, function, type,
    /// template or namespace ([meta.reflection.queries] `has_linkage`);
    /// [`Linkage::None`] for anything else (a type alias, a namespace
    /// alias, a data member, an enumerator, a parameter, a value).
    pub(super) fn linkage_of(&self, x: Reflectee) -> Linkage {
        let d = match x {
            Reflectee::Namespace(d) => return self.namespace_linkage(d),
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
        self.decl_linkage(d, 0)
    }

    /// Whether `d` has external or module linkage (clang
    /// `isExternallyVisible`, by the language rule).
    pub(crate) fn has_external_linkage(&self, d: DeclId) -> bool {
        matches!(self.decl_linkage(d, 0), Linkage::External | Linkage::Module)
    }

    /// Whether `d` has internal linkage (clang `getFormalLinkage() ==
    /// Internal`).
    pub(crate) fn has_internal_linkage(&self, d: DeclId) -> bool {
        self.decl_linkage(d, 0) == Linkage::Internal
    }

    /// [basic.link]/3-4 for declaration `d` (a variable, function, class,
    /// enumeration or template; anything else has none).
    fn decl_linkage(&self, d: DeclId, depth: usize) -> Linkage {
        let d = self.ast.first(d);
        let k = self.kind_of(d);
        let entity = match k {
            DeclKind::Var
            | DeclKind::VarTemplateSpecialization
            | DeclKind::ClassTemplate
            | DeclKind::FunctionTemplate
            | DeclKind::VarTemplate
            | DeclKind::TypeAliasTemplate
            | DeclKind::Concept => true,
            DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization | DeclKind::Enum => {
                // An unnamed class or enumeration without a typedef name
                // for linkage purposes has none, a closure type included
                // ([basic.link]/4.3-4.4; GCC 16.1 gives a namespace-scope
                // lambda's closure type linkage).
                self.ast.closure_lambda(d).is_none()
                    && (!self.decl_name(d).is_empty()
                        || self.ast.decls.typedef_for_anon(d).is_some())
            }
            DeclKind::CXXDeductionGuide => false,
            k => k.is_function(),
        };
        if !entity || depth > MAX_CTX {
            return Linkage::None;
        }
        if self.ast.decls.get(d).misc().local_extern() {
            return Linkage::External;
        }
        let Some(c) = self.semantic_parent(d) else { return Linkage::None };
        match self.kind_of(c) {
            k if k.is_function() => Linkage::None,
            // [basic.link]/4: a member takes its class's linkage.
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization => self.decl_linkage(c, depth + 1),
            DeclKind::TranslationUnit | DeclKind::Namespace => self.namespace_scope_linkage(d),
            _ => Linkage::None,
        }
    }

    /// [basic.link]/3-4 for `d` (first declaration) at namespace scope.
    fn namespace_scope_linkage(&self, d: DeclId) -> Linkage {
        if self.in_anonymous_namespace(d) || self.internal_by_declaration(d) {
            return Linkage::Internal;
        }
        if self.ast.modules.active
            && self.ast.module_attachment(d).is_some()
            && !self.is_exported(d)
        {
            return Linkage::Module;
        }
        Linkage::External
    }

    /// [basic.link]/3: a `static` variable, function or template thereof,
    /// or a non-template non-volatile `const` variable that is neither
    /// `inline`, `extern`, attached to a named module, nor of C language
    /// linkage.
    fn internal_by_declaration(&self, d: DeclId) -> bool {
        let templated =
            matches!(self.kind_of(d), DeclKind::FunctionTemplate | DeclKind::VarTemplate);
        let pattern = match self.kind_of(d) {
            DeclKind::FunctionTemplate | DeclKind::VarTemplate => {
                match self.ast.template_info(d).and_then(|i| i.pattern) {
                    Some(p) => p,
                    None => return false,
                }
            }
            _ => d,
        };
        let p = self.ast.first(pattern);
        match self.kind_of(p) {
            k if k.is_function() => self.fn_spec(p).storage() == kiln_ast::decl::FnStorage::Static,
            k @ (DeclKind::Var | DeclKind::VarTemplateSpecialization) => {
                let Some(v) = self.ast.decls.var(p) else { return false };
                if v.spec.storage() == VarStorage::Static {
                    return true;
                }
                if k != DeclKind::Var || templated {
                    return false;
                }
                let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                let ty = self.ast.types.canonical(ty);
                let mut e = ty;
                while let Some(x) = self.ast.types.element(e) {
                    e = self.ast.types.canonical(x);
                }
                let q = self.ast.types.effective_quals(e);
                let exported = q.is_volatile()
                    || v.spec.is_inline()
                    || v.spec.storage() == VarStorage::Extern
                    || (self.ast.modules.active && self.ast.module_attachment(p).is_some())
                    || kiln_abi::mangle::is_extern_c(&self.ast, p);
                q.is_const() && !exported
            }
            _ => false,
        }
    }

    /// A namespace has internal linkage when it is unnamed or inside an
    /// unnamed one, else external ([basic.link]/4); the global namespace
    /// is external (GCC 16.1 agrees).
    fn namespace_linkage(&self, d: DeclId) -> Linkage {
        let anon =
            self.ast.decls.namespace(d).is_some_and(|n| n.flags & NamespaceData::ANONYMOUS != 0);
        if anon || self.in_anonymous_namespace(d) { Linkage::Internal } else { Linkage::External }
    }

    /// Whether an enclosing namespace of `d` is unnamed.
    fn in_anonymous_namespace(&self, d: DeclId) -> bool {
        let mut c = self.ast.decls.get(d).ctx();
        for _ in 0..MAX_CTX {
            let Some(x) = c else { return false };
            if self.ast.decls.namespace(x).is_some_and(|n| n.flags & NamespaceData::ANONYMOUS != 0)
            {
                return true;
            }
            c = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// Whether `d` is declared in an `export` declaration (any lexical
    /// enclosing one).
    fn is_exported(&self, d: DeclId) -> bool {
        let mut c = self.ast.decls.lexical_ctx(d);
        for _ in 0..MAX_CTX {
            let Some(x) = c else { return false };
            if self.kind_of(x) == DeclKind::Export {
                return true;
            }
            c = self.ast.decls.lexical_ctx(x);
        }
        false
    }

    /// The semantic context of `d`, looking through linkage
    /// specifications and `export` declarations.
    pub(super) fn semantic_parent(&self, d: DeclId) -> Option<DeclId> {
        let mut c = self.ast.decls.get(d).ctx()?;
        for _ in 0..MAX_CTX {
            match self.kind_of(c) {
                DeclKind::LinkageSpec | DeclKind::Export => c = self.ast.decls.get(c).ctx()?,
                _ => return Some(c),
            }
        }
        None
    }

    /// `has_c_language_linkage`: a function or variable with linkage
    /// declared in an `extern "C"` linkage specification ([dcl.link]).
    pub(super) fn has_c_language_linkage(&self, x: Reflectee) -> bool {
        let Reflectee::Decl(d) = x else { return false };
        let k = self.kind_of(d);
        (k == DeclKind::Var || (k.is_function() && k != DeclKind::CXXDeductionGuide))
            && kiln_abi::mangle::is_extern_c(&self.ast, self.ast.first(d))
            && self.linkage_of(x) != Linkage::None
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
