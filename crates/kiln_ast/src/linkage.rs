//! Linkage ([basic.link]) and language linkage ([dcl.link]) of a
//! declaration (spec/ast.md §4.5, WP-2025).
//!
//! Two computations live here so that every client asks the same one:
//!
//! - [`Ast::linkage`], the language rule of [basic.link]/3-4 (a local
//!   entity has none, a member takes its class's), which reflection's
//!   `has_*_linkage` queries and sema's redeclaration checks read;
//! - [`Ast::is_extern_c`], whether a function or variable has C language
//!   linkage, which mangling (`kiln_abi::mangle::is_extern_c`), codegen
//!   and sema's [dcl.link]/6 redeclaration checks read.
//!
//! [`Ast::language_linkage`] combines them for tooling clients, which
//! read the `Ast` after the translation unit, when `Sema` is gone.

use crate::Ast;
use crate::decl::{DeclKind, FnStorage, LinkageLang, NamespaceData, TemplLinkKind, VarStorage};
use crate::ids::{DeclId, QualType};

/// The linkage of a name ([basic.link]/2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Linkage {
    /// No linkage: a local entity, a member of a class without linkage,
    /// an unnamed class, or a declaration that is no entity with a name
    /// (a data member, an enumerator, a parameter, an alias, ...).
    None,
    /// Internal linkage.
    Internal,
    /// Module linkage (attached to a named module, not exported).
    Module,
    /// External linkage.
    External,
}

/// The language linkage of a function or variable ([dcl.link]/1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LanguageLinkage {
    /// C++ language linkage (the default, [dcl.link]/2).
    Cxx,
    /// C language linkage (`extern "C"`).
    C,
    /// None: not a function or variable, or one whose name has no
    /// external linkage ([dcl.link]/1 gives a language linkage only to
    /// functions and variables whose names have external linkage).
    None,
}

/// How many context links a walk follows at most.
const MAX_CTX: usize = 1024;

fn is_record_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
    )
}

fn is_function_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion
    )
}

impl Ast {
    /// The language linkage of `d` ([dcl.link]): [`LanguageLinkage::C`]
    /// for a function or variable with C language linkage
    /// ([`Ast::is_extern_c`]), [`LanguageLinkage::Cxx`] for any other
    /// function or variable whose name has external linkage
    /// ([`Ast::linkage`]), [`LanguageLinkage::None`] for everything else:
    /// types, namespaces, templates (their pattern has the linkage),
    /// non-static data members, enumerators, parameters, deduction
    /// guides, local variables, and functions and variables with
    /// internal or module linkage. Answered from any declaration of the
    /// entity: a redeclaration has its first declaration's
    /// ([dcl.link]/6). A member of a class, a static data member
    /// included, has C++ language linkage even in an `extern "C"` block
    /// ([dcl.link]/5); a block-scope `extern` declaration has that of
    /// the linkage specification around its function.
    pub fn language_linkage(&self, d: DeclId) -> LanguageLinkage {
        let k = self.decls.get(d).kind();
        let fn_or_var =
            matches!(k, DeclKind::Var | DeclKind::VarTemplateSpecialization) || is_function_kind(k);
        if !fn_or_var {
            return LanguageLinkage::None;
        }
        let linkage = self.linkage(d);
        if self.is_extern_c(d) && linkage != Linkage::None {
            LanguageLinkage::C
        } else if linkage == Linkage::External {
            LanguageLinkage::Cxx
        } else {
            LanguageLinkage::None
        }
    }

    /// True if the function or variable `d` has C language linkage
    /// ([dcl.link]/4-6; clang `getDeclLanguageLinkage`, `isExternC`): its
    /// first declaration is lexically inside an `extern "C"` linkage
    /// specification, through enclosing functions and classes
    /// ([`Ast::in_extern_c_context`]: a block-scope `extern` declaration,
    /// or a friend declared in a class, in an `extern "C"` block), and it
    /// has external linkage. A class member has C++ language linkage, and
    /// so has a friend function with a trailing requires-clause
    /// ([dcl.link]/5); an entity with internal linkage (`extern "C" {
    /// static int x; }`, a `const` variable in a braced specification) or
    /// a local variable has none, and is mangled (`_ZL1x`). A function or
    /// variable in an unnamed namespace that is first declared in an
    /// `extern "C"` context keeps C language linkage, as in clang
    /// (`getLVForNamespaceScopeDecl`). In C every function and variable
    /// with linkage has it. Meant for functions and variables (callers
    /// ask only for those); [`Ast::language_linkage`] filters the kind.
    pub fn is_extern_c(&self, d: DeclId) -> bool {
        let first = self.first(d);
        let n = self.decls.get(first);
        // In C every entity with external linkage has C language linkage
        // (clang `getDeclLanguageLinkage`): a local variable has none unless
        // it is a block-scope `extern` declaration.
        if self.lang_c {
            if !matches!(n.kind(), DeclKind::Function | DeclKind::Var) {
                return false;
            }
            let local_var = n.kind() == DeclKind::Var
                && self.in_function(first)
                && self.decls.var(first).is_none_or(|v| v.spec.storage() != VarStorage::Extern);
            return !local_var && !self.internal_by_storage(first);
        }
        let Some(c) = n.ctx() else { return false };
        let ck = self.decls.get(c).kind();
        if is_record_kind(ck) {
            return false;
        }
        if is_function_kind(ck)
            && n.kind() != DeclKind::Function
            && self.decls.var(first).is_none_or(|v| v.spec.storage() != VarStorage::Extern)
        {
            return false;
        }
        if !self.in_extern_c_context(first) {
            return false;
        }
        // [dcl.link]/5: a member-like constrained friend (whose semantic
        // context is the namespace) has C++ language linkage.
        if is_function_kind(n.kind())
            && self.templates.trailing_requires.contains(first)
            && self
                .decls
                .lexical_ctx(first)
                .is_some_and(|l| is_record_kind(self.decls.get(l).kind()))
        {
            return false;
        }
        !self.extern_c_internal(first)
    }

    /// Whether namespace-scope or block-scope `extern` function or
    /// variable `first` (a first declaration, in an `extern "C"`
    /// context) has internal linkage, which takes away its language
    /// linkage: `static`, `const`, `internal_linkage`, or an unnamed
    /// namespace that the `extern "C"` exception does not cover.
    fn extern_c_internal(&self, first: DeclId) -> bool {
        if self.internal_by_storage(first) || self.has_internal_linkage_attr(first) {
            return true;
        }
        self.in_anonymous_namespace(first)
            && !(matches!(self.decls.get(first).kind(), DeclKind::Function | DeclKind::Var)
                && self.is_file_context(first))
    }

    /// True if `d` is lexically inside an `extern "C"` linkage
    /// specification, the innermost one deciding, through any enclosing
    /// namespaces, functions and classes (clang `isInExternCContext`).
    pub fn in_extern_c_context(&self, d: DeclId) -> bool {
        let mut c = self.decls.lexical_ctx(d);
        for _ in 0..10_000 {
            let Some(ctx) = c else { return false };
            match self.decls.get(ctx).kind() {
                DeclKind::LinkageSpec => {
                    return self
                        .decls
                        .scope(ctx)
                        .is_some_and(|s| s.linkage_lang() == Some(LinkageLang::C));
                }
                DeclKind::TranslationUnit => return false,
                _ => {}
            }
            c = self.decls.lexical_ctx(ctx);
        }
        false
    }

    /// True if the namespace-scope function or variable `d` has internal
    /// linkage by its own declaration ([basic.link]/3), as the symbol's
    /// linkage counts it: `static`, `internal_linkage`, or a
    /// non-`extern`/non-`inline` non-volatile `const` variable (not in C,
    /// not attached to a named module, not directly in a braceless
    /// linkage specification, not a variable template specialization).
    pub fn internal_by_storage(&self, d: DeclId) -> bool {
        let first = self.first(d);
        let n = self.decls.get(first);
        if !self.is_file_context(first) {
            return false;
        }
        // clang `internal_linkage` (a class's too: clang
        // `isInternalLinkageDecl` mangles it with `L`).
        if self.has_internal_linkage_attr(first) {
            return true;
        }
        match n.kind() {
            DeclKind::Function => {
                self.decls.function(first).is_some_and(|f| f.spec.storage() == FnStorage::Static)
            }
            DeclKind::Var | DeclKind::VarTemplateSpecialization => {
                let Some(v) = self.decls.var(first) else { return false };
                if v.spec.storage() == VarStorage::Static {
                    return true;
                }
                let q = self.types.canonical(n.ty().unwrap_or(QualType::INT));
                // [basic.link]/3.2: only a non-volatile `const` type gives
                // internal linkage (`const volatile int cv = 1;` is
                // external). P2788: a `const` variable attached to a named
                // module has module (or, exported, external) linkage. In C
                // a file-scope `const` object has external linkage
                // ([C17 6.2.2]/5).
                !self.lang_c
                    && self.const_non_volatile(q)
                    && !v.spec.is_inline()
                    && (!self.modules.active || self.module_attachment(first).is_none())
                    && v.spec.storage() != VarStorage::Extern
                    && !self.decls.is_single_line_linkage(first)
                    && !matches!(v.templ.get(), Some((TemplLinkKind::Specialization, _)))
            }
            _ => false,
        }
    }

    /// Whether canonical type `q` is const-qualified and not volatile
    /// ([basic.link]/3.2), the qualifiers of an array being those of its
    /// element type ([basic.type.qualifier]/3).
    fn const_non_volatile(&self, q: QualType) -> bool {
        let mut q = q;
        let mut quals = self.types.effective_quals(q);
        while let Some(e) = self.types.element(q) {
            q = self.types.canonical(e);
            quals |= self.types.effective_quals(q);
        }
        quals.is_const() && !quals.is_volatile()
    }

    /// True if the semantic context of `d` is a namespace (or the
    /// translation unit), looking through linkage specifications and
    /// `export` declarations.
    fn is_file_context(&self, d: DeclId) -> bool {
        let mut c = self.decls.get(d).ctx();
        while let Some(ctx) = c {
            match self.decls.get(ctx).kind() {
                DeclKind::TranslationUnit | DeclKind::Namespace => return true,
                DeclKind::LinkageSpec | DeclKind::Export => c = self.decls.get(ctx).ctx(),
                _ => return false,
            }
        }
        true
    }

    /// True if some semantic context of `d` is a function.
    fn in_function(&self, d: DeclId) -> bool {
        let mut c = self.decls.get(d).ctx();
        for _ in 0..MAX_CTX {
            let Some(ctx) = c else { return false };
            if is_function_kind(self.decls.get(ctx).kind()) {
                return true;
            }
            c = self.decls.get(ctx).ctx();
        }
        false
    }

    /// True if some enclosing namespace of `d` is unnamed.
    pub fn in_anonymous_namespace(&self, d: DeclId) -> bool {
        let mut c = self.decls.get(d).ctx();
        for _ in 0..MAX_CTX {
            let Some(x) = c else { return false };
            if self.is_anonymous_namespace(x) {
                return true;
            }
            c = self.decls.get(x).ctx();
        }
        false
    }

    /// True if `ns` is an unnamed namespace.
    pub fn is_anonymous_namespace(&self, ns: DeclId) -> bool {
        self.decls.namespace(ns).is_some_and(|n| n.flags & NamespaceData::ANONYMOUS != 0)
    }

    /// The linkage of the name of declaration `d` by the language rule
    /// ([basic.link]/3-4), not the symbol's: a variable, function, named
    /// class or enumeration (or one with a typedef name for linkage
    /// purposes), template or namespace; [`Linkage::None`] for anything
    /// else (a type alias, a namespace alias, a data member, an
    /// enumerator, a parameter, a deduction guide, a closure type). A
    /// local entity has none, whatever its function; a block-scope
    /// `extern` declaration has external linkage; a member takes its
    /// class's linkage. A namespace is internal when it is unnamed or in
    /// an unnamed one, else external.
    pub fn linkage(&self, d: DeclId) -> Linkage {
        if self.decls.get(d).kind() == DeclKind::Namespace {
            let internal = self.is_anonymous_namespace(d) || self.in_anonymous_namespace(d);
            return if internal { Linkage::Internal } else { Linkage::External };
        }
        self.decl_linkage(d, 0)
    }

    /// [basic.link]/3-4 for declaration `d` (not a namespace).
    fn decl_linkage(&self, d: DeclId, depth: usize) -> Linkage {
        let d = self.first(d);
        let k = self.decls.get(d).kind();
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
                self.closure_lambda(d).is_none()
                    && (!self.decls.get(d).name().is_empty()
                        || self.decls.typedef_for_anon(d).is_some())
            }
            DeclKind::CXXDeductionGuide => false,
            k => k.is_function(),
        };
        if !entity || depth > MAX_CTX {
            return Linkage::None;
        }
        let Some(c) = self.semantic_parent(d) else { return Linkage::None };
        match self.decls.get(c).kind() {
            // [basic.link]/6: a block-scope function declaration or
            // `extern` variable whose first declaration it is has
            // external linkage (a prior namespace-scope declaration
            // would be `first` and decide). Recognized by its shape:
            // `DeclMisc::local_extern` has no writer.
            k if k.is_function() => {
                let local_extern = match self.decls.get(d).kind() {
                    DeclKind::Function => true,
                    DeclKind::Var => {
                        self.decls.var(d).is_some_and(|v| v.spec.storage() == VarStorage::Extern)
                    }
                    _ => false,
                };
                if local_extern || self.decls.get(d).misc().local_extern() {
                    Linkage::External
                } else {
                    Linkage::None
                }
            }
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
        if self.modules.active && self.module_attachment(d).is_some() && !self.is_exported(d) {
            return Linkage::Module;
        }
        Linkage::External
    }

    /// [basic.link]/3: a `static` variable, function or template thereof,
    /// or a non-template non-volatile `const` variable that is neither
    /// `inline`, `extern`, attached to a named module, nor of C language
    /// linkage.
    fn internal_by_declaration(&self, d: DeclId) -> bool {
        let kind = self.decls.get(d).kind();
        let templated = matches!(kind, DeclKind::FunctionTemplate | DeclKind::VarTemplate);
        let pattern = if templated {
            match self.template_info(d).and_then(|i| i.pattern) {
                Some(p) => p,
                None => return false,
            }
        } else {
            d
        };
        let p = self.first(pattern);
        match self.decls.get(p).kind() {
            fk if fk.is_function() => {
                self.decls.function(p).is_some_and(|f| f.spec.storage() == FnStorage::Static)
            }
            vk @ (DeclKind::Var | DeclKind::VarTemplateSpecialization) => {
                let Some(var) = self.decls.var(p) else { return false };
                if var.spec.storage() == VarStorage::Static {
                    return true;
                }
                if vk != DeclKind::Var || templated {
                    return false;
                }
                let ty = self.decls.get(p).ty().unwrap_or(QualType::ERROR);
                let ty = self.types.canonical(ty);
                let mut elem = ty;
                while let Some(x) = self.types.element(elem) {
                    elem = self.types.canonical(x);
                }
                let q = self.types.effective_quals(elem);
                let exported = q.is_volatile()
                    || var.spec.is_inline()
                    || var.spec.storage() == VarStorage::Extern
                    || (self.modules.active && self.module_attachment(p).is_some())
                    || self.is_extern_c(p);
                q.is_const() && !exported
            }
            _ => false,
        }
    }

    /// Whether `d` is declared in an `export` declaration (any lexical
    /// enclosing one).
    fn is_exported(&self, d: DeclId) -> bool {
        let mut c = self.decls.lexical_ctx(d);
        for _ in 0..MAX_CTX {
            let Some(x) = c else { return false };
            if self.decls.get(x).kind() == DeclKind::Export {
                return true;
            }
            c = self.decls.lexical_ctx(x);
        }
        false
    }

    /// The semantic context of `d`, looking through linkage
    /// specifications and `export` declarations.
    pub fn semantic_parent(&self, d: DeclId) -> Option<DeclId> {
        let mut c = self.decls.get(d).ctx()?;
        for _ in 0..MAX_CTX {
            match self.decls.get(c).kind() {
                DeclKind::LinkageSpec | DeclKind::Export => c = self.decls.get(c).ctx()?,
                _ => return Some(c),
            }
        }
        None
    }
}
