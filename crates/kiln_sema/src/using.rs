//! Using-declarations ([namespace.udecl]), using-enum-declarations
//! ([enum.udecl]) and namespace aliases ([namespace.alias]); spec/sema.md
//! §4.8.
//!
//! A using-declarator creates a `Using` declaration (not entered in
//! lookup) and one implicit `UsingShadow` per declaration it names; the
//! shadows are entered in the current scope and lookup looks through
//! them ([`Sema::lookup_name`] strips them, recording the shadow as the
//! found declaration). A namespace alias is entered in lookup and resolved
//! transparently wherever a namespace is expected.

use kiln_ast::decl::DeclKind;
use kiln_ast::names::{DeclName, DeclNameKind};
use kiln_ast::types::Nns;
use kiln_ast::{DeclHead, DeclId, NnsId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::decl::{NameClass, sym_str};
use crate::lookup::{LookupKind, is_tag_kind, is_type_kind};
use crate::scope::ScopeKind;

/// A nested-name-specifier as the parser resolved it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Qualifier {
    /// The context it names (aliases resolved; a class or enumeration's
    /// definition when it has one).
    pub ctx: DeclId,
    /// The specifier as written.
    pub nns: NnsId,
    /// Where it starts.
    pub loc: Loc,
}

impl Sema<'_, '_> {
    // -- nested-name-specifiers ------------------------------------------------------

    /// The nested-name-specifier `::`.
    pub fn nns_global(&mut self) -> NnsId {
        self.ast.types.nns(Nns::Global)
    }

    /// The nested-name-specifier `prefix spelled::`, where `spelled` is the
    /// declaration the component's name found and `resolved` the context
    /// it denotes ([`Sema::act_on_nested_name_spelled`]).
    pub fn nns_component(
        &mut self,
        prefix: Option<NnsId>,
        resolved: DeclId,
        spelled: DeclId,
    ) -> NnsId {
        let n = match self.kind_of(spelled) {
            DeclKind::Namespace => Nns::Namespace { prefix, ns: self.ast.first(spelled) },
            DeclKind::NamespaceAlias => {
                Nns::NamespaceAlias { prefix, alias: spelled, ns: resolved }
            }
            k => {
                let ty = if matches!(
                    k,
                    DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::TemplateTypeParm
                ) {
                    self.ast.decls.get(spelled).ty().unwrap_or(QualType::ERROR)
                } else if k == DeclKind::CXXRecord {
                    // [temp.dep.type]/1: the injected-class-name of a class
                    // template (or partial specialization) inside it names
                    // the current instantiation `X<P...>` (clang's
                    // `InjectedClassNameType` specifier).
                    self.this_class_type(spelled)
                } else {
                    self.tag_type(spelled)
                };
                Nns::Type { prefix, ty, template_kw: false }
            }
        };
        self.ast.types.nns(n)
    }

    /// The class whose member-specification is being parsed, if the
    /// innermost scope is a class scope.
    pub(crate) fn member_spec_class(&self) -> Option<DeclId> {
        let s = self.scopes.iter().rev().find(|s| !s.is_transparent())?;
        if s.kind == ScopeKind::Record { s.entity } else { None }
    }

    // -- using-declarations ------------------------------------------------------------

    /// One using-declarator `using qual name;` ([namespace.udecl]).
    /// `using_loc` is the `using` keyword; `if_exists` is set by the
    /// `using_if_exists` attribute; `ellipsis` is the `...` of a pack
    /// expansion (`using Bases::operator()...;`, [temp.variadic]/5), whose
    /// qualifier names an unexpanded pack: the declarator stays an
    /// `UnresolvedUsingValue` and instantiation expands it over the pack.
    /// Returns the `Using` declaration, or `None` after an error.
    pub fn act_on_using_declarator(
        &mut self,
        using_loc: Loc,
        qual: Option<Qualifier>,
        name: DeclName,
        name_loc: Loc,
        if_exists: bool,
        ellipsis: Option<Loc>,
    ) -> Option<DeclId> {
        // [namespace.udecl]/1: the declarator's name is qualified.
        let Some(q) = qual else {
            self.error(DiagId::sema_using_requires_qualname, name_loc);
            return None;
        };
        if let Some(l) = ellipsis {
            let packs = self
                .ast
                .types
                .nns_flags(q.nns)
                .contains(kiln_ast::types::TypeFlags::UNEXPANDED_PACK);
            if !packs {
                self.error(DiagId::sema_pack_expansion_without_packs, l);
                return None;
            }
            if self.member_spec_class().is_none() {
                self.unsupported("a pack expansion in a non-member using-declaration", l);
                return None;
            }
        }
        let qk = self.kind_of(q.ctx);
        if self.literal_operator_qualifier_error(q, name, name_loc) {
            return None;
        }
        if let Some(class) = self.member_spec_class()
            && qk != DeclKind::Enum
        {
            return self.class_using_declarator(class, q, name, name_loc, using_loc, false);
        }
        let dependent = self.dependent_qualifier(q);
        if name.as_ident().is_none() && self.in_local_scope() && dependent {
            // A block-scope `using T::operator+;` is an
            // `UnresolvedUsingValue` in clang, diagnosed when instantiated
            // (the qualifier is a class, whose only nameable members here
            // are enumerators, [namespace.udecl]/8); the local scope
            // records non-identifier names only as resolved shadows
            // (`Scope.local_names`).
            self.unsupported(
                "an operator name with a dependent qualifier in a block-scope using-declarator",
                name_loc,
            );
            return None;
        }
        if let Some(n) = name.as_ident()
            && self.in_local_scope()
            && dependent
        {
            // A dependent qualifier in a function template (`using
            // E<T>::A;`, an enumerator, [namespace.udecl]/8): clang's
            // `UnresolvedUsingValueDecl`, a local name whose uses are
            // dependent; the instantiation redoes the declarator
            // (`subst/stmts.rs` `local_using_decl`).
            let h = DeclHead::new(self.ctx, name, name_loc);
            let d = self.builder().unresolved_using_value(h, q.nns, false);
            if using_loc != name_loc {
                // The instantiation's notes point at the `using`.
                let _ = self.ast.decls.cold.begin_loc.insert(d, using_loc);
            }
            self.push_local(n, d);
            return Some(d);
        }
        let found = self.lookup_qualified_dn(q.ctx, name, LookupKind::All);
        // [namespace.udecl]/8: at namespace or block scope a
        // using-declarator may name a class member only if it is an
        // enumerator.
        if qk == DeclKind::CXXRecord
            && (found.is_empty()
                || found.decls.iter().any(|&d| self.kind_of(d) != DeclKind::EnumConstant))
        {
            let mut msg =
                self.diag(DiagId::sema_using_decl_can_not_refer_to_class_member, name_loc);
            if let Some(&t) = found.decls.first() {
                let k = self.kind_of(t);
                if is_type_kind(k) {
                    msg =
                        msg.with_note(self.diag(DiagId::note_using_decl_class_member_alias, q.loc));
                } else if k == DeclKind::Var {
                    msg = msg.with_note(
                        self.diag(DiagId::note_using_decl_class_member_reference, using_loc),
                    );
                }
            }
            self.emit(msg);
            return None;
        }
        if found.is_empty() {
            if if_exists {
                // clang's UnresolvedUsingIfExistsDecl: lookup ignores it
                // while anything else is found; a use is an error.
                let using = self.new_using_decl(name, name_loc, q.nns);
                let h = DeclHead::new(self.ctx, name, using_loc);
                let uuie = self.builder().unresolved_using_if_exists(h);
                let _ = self.introduce_shadow(name, name_loc, uuie);
                return Some(using);
            }
            let msg = self
                .diag(DiagId::sema_no_member, name_loc)
                .arg(self.decl_name_str(name))
                .arg(self.qualifier_desc(q.ctx));
            self.emit(msg);
            return None;
        }
        // [namespace.udecl]/3: a using-declarator cannot name a namespace.
        if found
            .decls
            .iter()
            .any(|&d| matches!(self.kind_of(d), DeclKind::Namespace | DeclKind::NamespaceAlias))
        {
            let msg = self
                .diag(DiagId::sema_using_decl_can_not_refer_to_namespace, name_loc)
                .with_note(self.diag(DiagId::note_namespace_using_decl, q.loc));
            self.emit(msg);
            return None;
        }
        let using = self.new_using_decl(name, name_loc, q.nns);
        for &t in &found.decls {
            let _ = self.introduce_shadow(name, name_loc, t);
        }
        Some(using)
    }

    /// True if qualifier `q` is dependent (or instantiation-dependent).
    fn dependent_qualifier(&self, q: Qualifier) -> bool {
        self.is_dependent_ctx(q.ctx)
            || self
                .ast
                .types
                .nns_flags(q.nns)
                .intersects(kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION)
    }

    /// [over.literal]/2: a literal operator is a namespace member, so a
    /// class, enumeration or dependent qualifier never names one (clang
    /// `err_literal_operator_id_outside_namespace`, at any scope).
    /// Reports it and returns true for such a declarator.
    fn literal_operator_qualifier_error(
        &mut self,
        q: Qualifier,
        name: DeclName,
        name_loc: Loc,
    ) -> bool {
        if name.kind() != DeclNameKind::LiteralOperator
            || matches!(self.kind_of(q.ctx), DeclKind::Namespace | DeclKind::TranslationUnit)
        {
            return false;
        }
        let msg = self
            .diag(DiagId::sema_literal_operator_id_outside_namespace, name_loc)
            .arg(self.refers_into_str(q.nns));
        self.emit(msg);
        true
    }

    /// One using-declarator `using typename qual name;`
    /// ([namespace.udecl]/1, [temp.res.general]/5): the name is a type.
    /// In a class with a dependent qualifier (`using typename
    /// B<T>::type;`) the member is an `UnresolvedUsingTypename` naming
    /// its own dependent `UnresolvedUsing` type (as clang), redone when
    /// the class is instantiated. Otherwise the declarator is an ordinary one whose
    /// target must be a type. Returns the declaration, or `None` after an
    /// error.
    pub fn act_on_using_typename_declarator(
        &mut self,
        using_loc: Loc,
        qual: Option<Qualifier>,
        name: DeclName,
        name_loc: Loc,
        if_exists: bool,
        ellipsis: Option<Loc>,
    ) -> Option<DeclId> {
        if let (Some(q), Some(_), None) = (qual, name.as_ident(), ellipsis)
            && self.member_spec_class().is_some()
            && (self.is_dependent_ctx(q.ctx)
                || self
                    .ast
                    .types
                    .nns_flags(q.nns)
                    .intersects(kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION)
                    && self.kind_of(q.ctx) != DeclKind::CXXRecord)
        {
            let access = self.current_member_access();
            let h = DeclHead::new(self.ctx, name, name_loc).access(access);
            return Some(self.builder().unresolved_using_typename(h, q.nns));
        }
        if let Some(q) = qual
            && !self.is_dependent_ctx(q.ctx)
        {
            let found = self.lookup_qualified_dn(q.ctx, name, LookupKind::All);
            if let Some(&t) = found.decls.iter().find(|&&d| {
                let k = self.kind_of(self.ast.using_target(d).unwrap_or(d));
                !is_type_kind(k) && !is_tag_kind(k)
            }) && !found.decls.iter().any(|&d| {
                let k = self.kind_of(self.ast.using_target(d).unwrap_or(d));
                is_type_kind(k) || is_tag_kind(k)
            }) {
                let msg = self.diag(DiagId::sema_using_typename_non_type, name_loc).with_note(
                    self.diag(DiagId::note_using_decl_target, self.ast.decls.get(t).loc()),
                );
                self.emit(msg);
                return None;
            }
        }
        self.act_on_using_declarator(using_loc, qual, name, name_loc, if_exists, ellipsis)
    }

    /// The qualifier a "using declaration refers into" diagnostic names:
    /// clang 19 prints the specifier (`C::`), clang 22 the scope (`C`).
    fn refers_into_str(&self, nns: NnsId) -> String {
        let s = self.nns_str(nns);
        match s.strip_suffix("::") {
            Some(scope) if self.clang_compat >= 22 && !scope.is_empty() => scope.to_owned(),
            _ => s,
        }
    }

    fn new_using_decl(&mut self, name: DeclName, name_loc: Loc, nns: NnsId) -> DeclId {
        let h = DeclHead::new(self.ctx, name, name_loc);
        self.builder().using_decl(h, Some(nns))
    }

    /// A using-declarator in a class ([namespace.udecl]/3): it names a
    /// member of a base class (`using B::f;`), or, when the name is the
    /// qualifier's last component (`using B::B;`, [class.qual]/2) or the
    /// nominated class's own name (WP-724), the constructors of a direct
    /// base (WP-114). A dependent qualifier makes
    /// an `UnresolvedUsingValue` redone at instantiation.
    fn class_using_declarator(
        &mut self,
        class: DeclId,
        q: Qualifier,
        name: DeclName,
        name_loc: Loc,
        using_loc: Loc,
        force_inheriting: bool,
    ) -> Option<DeclId> {
        let qk = self.kind_of(q.ctx);
        if matches!(qk, DeclKind::Namespace | DeclKind::TranslationUnit) {
            let msg = self
                .diag(DiagId::sema_using_decl_nested_name_specifier_is_not_class, q.loc)
                .arg(self.refers_into_str(q.nns));
            self.emit(msg);
            return None;
        }
        // [class.qual]/2.2: the name is the last spelled component
        // (`using B::B;`), or the injected-class-name of the class the
        // qualifier nominates however it is spelled (`using
        // std::regex::basic_regex;`).
        let inheriting = force_inheriting
            || name.as_ident().is_some_and(|n| {
                self.nns_last_name(q.nns) == Some(n)
                    || qk == DeclKind::CXXRecord
                        && self.ast.decls.get(q.ctx).name().as_ident() == Some(n)
            });
        if self.is_dependent_ctx(q.ctx)
            || self
                .ast
                .types
                .nns_flags(q.nns)
                .intersects(kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION)
                && qk != DeclKind::CXXRecord
        {
            return Some(self.dependent_class_using(class, q, name, name_loc, inheriting));
        }
        if self.ast.first(q.ctx) == self.ast.first(class) {
            self.error(DiagId::sema_using_decl_refers_to_own_class, q.loc);
            // clang 22 stops there; clang 19 also looks the name up.
            if self.clang_compat < 22 && self.lookup_in_dn(q.ctx, name, LookupKind::All).is_empty()
            {
                let msg = self
                    .diag(DiagId::sema_no_member, name_loc)
                    .arg(self.decl_name_str(name))
                    .arg(self.qualifier_desc(q.ctx));
                self.emit(msg);
            }
            return None;
        }
        let is_base = qk == DeclKind::CXXRecord
            && if inheriting {
                self.is_direct_base(class, q.ctx)
            } else {
                crate::conv::bases::is_derived_from(self, class, q.ctx)
            };
        if !is_base {
            if self.has_dependent_bases(class) {
                // The qualifier may name a member of a dependent base
                // (clang: checked at instantiation).
                return Some(self.dependent_class_using(class, q, name, name_loc, inheriting));
            }
            // clang: a qualifier naming an invalid class (one whose
            // instantiation failed, so it may have been dropped as a
            // base) is not diagnosed again.
            if self.ast.decls.get(q.ctx).has(kiln_ast::decl::DeclFlags::INVALID) {
                return None;
            }
            let msg = if inheriting && crate::conv::bases::is_derived_from(self, class, q.ctx) {
                self.diag(DiagId::sema_using_decl_constructor_not_in_direct_base, using_loc)
                    .arg(self.ty_arg(self.class_type(q.ctx)))
                    .arg(self.ty_arg(self.class_type(class)))
            } else {
                self.diag(DiagId::sema_using_decl_not_base_class, q.loc)
                    .arg(self.refers_into_str(q.nns))
                    // A `NamedDecl` argument: `C<X>` for a specialization.
                    .arg(self.name_for_diagnostic(class))
            };
            self.emit(msg);
            return None;
        }
        if inheriting {
            // [namespace.udecl]/10: `using B::B;` cannot be repeated.
            if let Some(prev) = self.previous_inheriting_using(class, q.ctx) {
                let msg = self
                    .diag(DiagId::sema_using_decl_redeclaration, name_loc)
                    .with_note(self.diag(DiagId::note_previous_using_decl, prev));
                self.emit(msg);
                return None;
            }
            return Some(self.inheriting_using(class, q, name_loc));
        }
        self.declare_implicit_named(q.ctx, name);
        let found = self.lookup_qualified_dn(q.ctx, name, LookupKind::All);
        if found.is_empty() {
            let msg = self
                .diag(DiagId::sema_no_member, name_loc)
                .arg(self.decl_name_str(name))
                .arg(self.qualifier_desc(q.ctx));
            self.emit(msg);
            return None;
        }
        // [namespace.udecl]/10 (clang `CheckUsingDeclRedeclaration`): a
        // using-declarator in a class repeats an earlier one only when both
        // name the member through the same class. Through another class
        // (`using A::f; using B::f;`, B derived from A) the members already
        // introduced are not introduced again (`introduce_shadow`).
        if let Some(prev) = self.previous_class_using(class, q.ctx, name) {
            let msg = self
                .diag(DiagId::sema_using_decl_redeclaration, name_loc)
                .with_note(self.diag(DiagId::note_previous_using_decl, prev));
            self.emit(msg);
            return None;
        }
        let access = self.current_member_access();
        let h = DeclHead::new(self.ctx, name, name_loc).access(access);
        let using = self.builder().using_decl(h, Some(q.nns));
        let mut targets = found.decls.clone();
        if self.reverse_using_shadows {
            targets.reverse();
        }
        for t in targets {
            let _ = self.introduce_shadow(name, name_loc, t);
        }
        Some(using)
    }

    /// A class-scope using-declaration of a class template's pattern,
    /// replayed in the instantiation being defined ([temp.inst]/3):
    /// `inheriting` when the pattern's declarator named the base's
    /// constructors (`using B<T>::B;`, `using T::T;`, `using __base::__base;`),
    /// which the substituted qualifier may no longer spell.
    pub(crate) fn instantiate_class_using(
        &mut self,
        q: Qualifier,
        name: DeclName,
        name_loc: Loc,
        inheriting: bool,
    ) -> Option<DeclId> {
        let class = self.member_spec_class()?;
        self.class_using_declarator(class, q, name, name_loc, name_loc, inheriting)
    }

    /// Whether `base` is a direct base class of `class`.
    pub(crate) fn is_direct_base(&self, class: DeclId, base: DeclId) -> bool {
        let Some(def) = self.ast.definition(class) else { return false };
        let want = self.ast.first(base);
        self.ast
            .decls
            .bases(def, &self.ast.lists)
            .any(|b| self.ast.types.as_record(b.ty).is_some_and(|r| self.ast.first(r) == want))
    }

    /// The access of the next member of the class being defined.
    fn current_member_access(&self) -> kiln_ast::decl::Access {
        self.classes.last().map_or(kiln_ast::decl::Access::Public, |c| c.access)
    }

    /// The identifier spelling the last component of `nns` (a typedef,
    /// class, template or template parameter name), for [class.qual]/2.
    pub(crate) fn nns_last_name(&self, nns: NnsId) -> Option<Symbol> {
        match self.ast.types.nns_view(nns) {
            Nns::Type { ty, .. } => self.type_spelled_name(ty, 0),
            Nns::Identifier { name, .. } => Some(name),
            _ => None,
        }
    }

    fn type_spelled_name(&self, ty: QualType, depth: u32) -> Option<Symbol> {
        use kiln_ast::types::{TemplateName, TypeView};
        if depth > 64 {
            return None;
        }
        let ident = |d: DeclId| self.ast.decls.get(d).name().as_ident();
        match self.ast.types.view(ty.ty()) {
            TypeView::Typedef { decl, .. } => ident(decl),
            TypeView::Using { shadow, .. } => ident(shadow),
            TypeView::Record(d) | TypeView::Enum(d) | TypeView::TemplateTypeParm(d) => ident(d),
            TypeView::InjectedClassName { pattern, .. } => ident(pattern),
            TypeView::Elaborated { named, .. } => self.type_spelled_name(named, depth + 1),
            TypeView::SubstTemplateTypeParm { replacement, .. } => {
                self.type_spelled_name(replacement, depth + 1)
            }
            TypeView::DependentName { name, .. }
            | TypeView::DependentTemplateSpecialization { name, .. } => Some(name),
            TypeView::TemplateSpecialization { name, .. } => {
                let mut n = name;
                for _ in 0..64 {
                    match self.ast.types.template_name_view(n) {
                        TemplateName::Qualified { underlying, .. } => n = underlying,
                        TemplateName::Template(d) => return ident(d),
                        TemplateName::TemplateTemplateParm { decl, .. } => return ident(decl),
                        TemplateName::DependentIdentifier { name, .. } => return Some(name),
                        _ => return None,
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// A class-scope using-declarator with a dependent qualifier (or one
    /// that may name a member of a dependent base): an
    /// `UnresolvedUsingValue` member, entered in lookup so uses of the
    /// name are dependent; instantiation redoes the declarator.
    /// A class-scope using-declarator whose substituted qualifier is
    /// still dependent (a local class of a generic lambda in a template
    /// specialization): it stays unresolved in the class being defined.
    pub(crate) fn still_dependent_class_using(
        &mut self,
        nns: NnsId,
        name: DeclName,
        name_loc: Loc,
        inheriting: bool,
    ) -> Option<DeclId> {
        let class = self.member_spec_class()?;
        let q = Qualifier { ctx: class, nns, loc: name_loc };
        Some(self.dependent_class_using(class, q, name, name_loc, inheriting))
    }

    /// `using typename N::name;` whose substituted qualifier is still
    /// dependent: an `UnresolvedUsingTypename` in the class being defined.
    pub(crate) fn still_dependent_using_typename(
        &mut self,
        nns: NnsId,
        name: DeclName,
        name_loc: Loc,
    ) -> DeclId {
        let access = self.current_member_access();
        let h = DeclHead::new(self.ctx, name, name_loc).access(access);
        self.builder().unresolved_using_typename(h, nns)
    }

    fn dependent_class_using(
        &mut self,
        class: DeclId,
        q: Qualifier,
        name: DeclName,
        name_loc: Loc,
        inheriting: bool,
    ) -> DeclId {
        let access = self.current_member_access();
        // An inheriting declarator is named by the constructor name of the
        // canonical nominated type (clang: `T::type-parameter-0-0`).
        let dn = match self.ast.types.nns_view(q.nns) {
            Nns::Type { ty, .. } if inheriting => {
                let canon = self.ast.types.canonical(ty).ty();
                self.ast.names.constructor(canon)
            }
            _ => name,
        };
        let h = DeclHead::new(self.ctx, dn, name_loc).access(access);
        let d = self.builder().unresolved_using_value(h, q.nns, !inheriting);
        if inheriting {
            self.note_inherited_ctor(class);
        }
        d
    }

    /// Sets `HAS_INHERITED_CTOR` on the class being defined.
    fn note_inherited_ctor(&mut self, class: DeclId) {
        if let Some(c) = self.classes.last_mut()
            && c.record == class
        {
            c.bits = c.bits.with(kiln_ast::decl::DefinitionBits::HAS_INHERITED_CTOR);
        }
    }

    /// `using B::B;` naming direct base `q.ctx` of `class`
    /// ([namespace.udecl]/3, [class.inhctor.init]): a `Using` named by the
    /// class's constructor name and one `ConstructorUsingShadow` per
    /// constructor of the base (including the ones it inherits itself),
    /// except those hidden by a constructor of `class` already declared
    /// with the same signature ([namespace.udecl]/14; later ones hide at
    /// `}`).
    fn inheriting_using(&mut self, class: DeclId, q: Qualifier, name_loc: Loc) -> DeclId {
        let access = self.current_member_access();
        let canon = self.ast.types.canonical(self.class_type(class)).ty();
        let cname = self.ast.names.constructor(canon);
        let h = DeclHead::new(self.ctx, cname, name_loc).access(access);
        let using = self.builder().using_decl(h, Some(q.nns));
        self.note_inherited_ctor(class);
        let base = self.ast.definition(q.ctx).unwrap_or(q.ctx);
        for k in [
            crate::class::SpecialKind::DefaultCtor,
            crate::class::SpecialKind::CopyCtor,
            crate::class::SpecialKind::MoveCtor,
        ] {
            self.ensure_implicit(base, k);
        }
        let own: Vec<DeclId> = self.ast.constructors(class).iter().collect();
        let mut inherited = self.inheritable_ctors(base);
        if self.reverse_using_shadows {
            inherited.reverse();
        }
        for (target, nominated) in inherited {
            if own.iter().any(|&c| self.hides_ctor(c, target)) {
                continue;
            }
            let h = DeclHead::new(self.ctx, cname, name_loc).access(access);
            let _ = self.builder().constructor_using_shadow(h, target, nominated);
        }
        using
    }

    /// The location of an earlier `using B::B;` of class `class` naming
    /// base `base` (found by the `ConstructorUsingShadow`s it made).
    fn previous_inheriting_using(&self, class: DeclId, base: DeclId) -> Option<Loc> {
        let def = self.ast.definition(class).unwrap_or(class);
        let base = self.ast.first(base);
        self.ast
            .decls
            .members(def)
            .filter(|&m| self.kind_of(m) == DeclKind::ConstructorUsingShadow)
            .find(|&m| {
                let ns = self.ast.decls.ctor_shadow_nominated(m);
                self.ast.shadow_base_class(ns, m) == Some(base)
            })
            .map(|m| self.ast.decls.get(m).loc())
    }

    /// The location of an earlier using-declarator of `name` in class
    /// `class` whose nested-name-specifier names class `qual`.
    fn previous_class_using(&self, class: DeclId, qual: DeclId, name: DeclName) -> Option<Loc> {
        let def = self.ast.definition(class).unwrap_or(class);
        let qual = self.ast.first(qual);
        self.ast
            .decls
            .members(def)
            .filter(|&m| self.kind_of(m) == DeclKind::Using && self.ast.decls.get(m).name() == name)
            .find(|&m| {
                let Some(nns) = self.ast.using_qualifier(m) else { return false };
                let Nns::Type { ty, .. } = self.ast.types.nns_view(nns) else { return false };
                let ty = self.ast.types.canonical(ty);
                self.ast.types.as_record(ty).is_some_and(|r| self.ast.first(r) == qual)
            })
            .map(|m| self.ast.decls.get(m).loc())
    }

    /// The constructors class `base` offers to a class inheriting them:
    /// its own and the ones it inherits (not hidden), each with the
    /// base's shadow for an inherited one.
    pub(crate) fn inheritable_ctors(&self, base: DeclId) -> Vec<(DeclId, Option<DeclId>)> {
        let mut out: Vec<(DeclId, Option<DeclId>)> =
            self.ast.constructors(base).iter().map(|c| (c, None)).collect();
        for s in self.ctor_shadows(base) {
            if let Some(t) = self.ast.using_target(s) {
                out.push((t, Some(s)));
            }
        }
        out
    }

    /// The `ConstructorUsingShadow`s of class `class` not hidden by one of
    /// its constructors.
    pub(crate) fn ctor_shadows(&self, class: DeclId) -> Vec<DeclId> {
        let Some(def) = self.ast.definition(class) else { return Vec::new() };
        let bits = self.ast.decls.record(def).map(|r| r.defn).unwrap_or_default();
        if !bits.contains(kiln_ast::decl::DefinitionBits::HAS_INHERITED_CTOR)
            && !self.classes.iter().any(|c| {
                c.record == def
                    && c.bits.contains(kiln_ast::decl::DefinitionBits::HAS_INHERITED_CTOR)
            })
        {
            return Vec::new();
        }
        self.ast
            .decls
            .members(def)
            .filter(|&m| {
                self.kind_of(m) == DeclKind::ConstructorUsingShadow
                    && self.ast.decls.hidden_shadow(m).is_none()
            })
            .collect()
    }

    /// Whether constructor `own` (of the derived class) hides inherited
    /// constructor `target`: same parameter-type-list (and template head)
    /// ([namespace.udecl]/14, [over.dcl]).
    pub(crate) fn hides_ctor(&mut self, own: DeclId, target: DeclId) -> bool {
        let (ko, kt) = (self.kind_of(own), self.kind_of(target));
        if ko != kt {
            return false;
        }
        if ko == DeclKind::FunctionTemplate {
            return self.equivalent_signatures(own, target);
        }
        let to = self.ast.decls.get(own).ty().unwrap_or(QualType::ERROR);
        let tt = self.ast.decls.get(target).ty().unwrap_or(QualType::ERROR);
        self.same_ignoring_exc_spec(to, tt)
    }

    /// At the `}` of class `r`: records the `ConstructorUsingShadow`s
    /// hidden by a constructor of `r` declared after the using-declaration
    /// (including the implicit ones declared eagerly).
    pub(crate) fn hide_inherited_ctors(&mut self, r: DeclId) {
        let shadows = self.ctor_shadows(r);
        if shadows.is_empty() {
            return;
        }
        let own: Vec<DeclId> = self.ast.constructors(r).iter().collect();
        for s in shadows {
            let Some(t) = self.ast.using_target(s) else { continue };
            if let Some(&c) = own.iter().find(|&&c| self.hides_ctor(c, t)) {
                let _ = self.ast.decls.set_hidden_shadow(s, c);
            }
        }
    }

    /// Introduces a `UsingShadow` of `target` named `name` into the
    /// current scope, unless it conflicts with a declaration already
    /// there ([namespace.udecl]/11, [basic.scope.scope]/4). A repeated
    /// using-declaration of the same entity replaces the earlier shadow.
    fn introduce_shadow(&mut self, name: DeclName, loc: Loc, target: DeclId) -> Option<DeclId> {
        let in_class = self.member_spec_class().is_some();
        let fnlike = |me: &Self, d| {
            let k = me.kind_of(d);
            k.is_function() || k == DeclKind::FunctionTemplate
        };
        let mut prev = None;
        for e in self.current_scope_raw_dn(name) {
            let es = self.ast.strip_shadow(e);
            if self.ast.first(es) == self.ast.first(target) {
                if e != es {
                    if in_class && self.ast.decls.hidden_shadow(e).is_none() {
                        // The member is already introduced into the class
                        // by an earlier using-declarator through another
                        // class (clang `CheckUsingShadowDecl`: an
                        // equivalent declaration makes no new shadow). The
                        // same class twice was diagnosed by
                        // `previous_class_using`.
                        return None;
                    }
                    prev = Some(e);
                }
                continue;
            }
            // [namespace.udecl]/11 (example): two using-declarators may
            // introduce functions with the same parameter-type-list (a
            // call is then ambiguous); only a function declared in the
            // scope conflicts.
            if e != es && fnlike(self, es) && fnlike(self, target) {
                continue;
            }
            if in_class && e == es && fnlike(self, es) && fnlike(self, target) {
                // [namespace.udecl]/14: a member function (template) of
                // the class hides the one the declarator would introduce
                // when they correspond; otherwise they overload.
                if self.member_hides_using_target(es, target) {
                    return None;
                }
                continue;
            }
            if self.using_conflicts(target, es) {
                let msg = self
                    .diag(DiagId::sema_using_decl_conflict, loc)
                    .with_note(self.diag(DiagId::note_using_decl_target, self.decl_loc(target)))
                    .with_note(self.diag(DiagId::note_using_decl_conflict, self.decl_loc(es)));
                self.emit(msg);
                return None;
            }
        }
        let mut h = DeclHead::new(self.ctx, name, loc);
        if in_class {
            h = h.access(self.current_member_access());
        }
        let s = self.builder().using_shadow(h, target, prev);
        if self.in_local_scope() {
            match name.as_ident() {
                Some(n) => self.push_local(n, s),
                None => self.push_local_name(name, s),
            }
        }
        Some(s)
    }

    /// Where declaration `d` was declared (its name).
    pub(crate) fn decl_loc(&self, d: DeclId) -> Loc {
        self.ast.decls.get(d).loc()
    }

    /// Whether `a` and `b` (distinct entities with the same name in one
    /// scope) cannot coexist when one is introduced by a using-declarator
    /// (clang's `CheckUsingShadowDecl`): a tag conflicts only with a tag;
    /// functions overload unless their parameter lists match; typedefs of
    /// the same type agree; anything else conflicts.
    pub(crate) fn using_conflicts(&mut self, a: DeclId, b: DeclId) -> bool {
        let (ka, kb) = (self.kind_of(a), self.kind_of(b));
        if ka == DeclKind::UnresolvedUsingIfExists || kb == DeclKind::UnresolvedUsingIfExists {
            return false;
        }
        if is_tag_kind(ka) || is_tag_kind(kb) {
            return is_tag_kind(ka) && is_tag_kind(kb);
        }
        let ta = self.ast.decls.get(a).ty().unwrap_or(QualType::ERROR);
        let tb = self.ast.decls.get(b).ty().unwrap_or(QualType::ERROR);
        let typedef = |k| matches!(k, DeclKind::Typedef | DeclKind::TypeAlias);
        if typedef(ka) && typedef(kb) {
            return !self.ast.types.same_type(ta, tb);
        }
        if ka.is_function() && kb.is_function() {
            // A `pass_object_size` function overloads one without
            // (spec/sema-overload.md §4.3a; WP-803).
            let ps = |me: &Self, d| me.ast.pass_object_size_params(d).is_empty();
            return ps(self, a) == ps(self, b) && self.same_ignoring_exc_spec(ta, tb);
        }
        // A function template overloads with functions and with other
        // function templates unless the signatures, including the
        // template heads, are equivalent ([basic.scope.scope]/4,
        // [temp.over.link]/7; clang `CheckOverload`).
        let fnlike = |k: DeclKind| k.is_function() || k == DeclKind::FunctionTemplate;
        if fnlike(ka) && fnlike(kb) {
            return ka == kb && self.equivalent_signatures(a, b);
        }
        true
    }

    /// [namespace.udecl]/14: whether member function (template) `own` of
    /// a class hides `target`, a base member function (template) a
    /// using-declarator of the class names: both templates or both not,
    /// with the same parameter-type-list, cv- and ref-qualifiers and
    /// trailing requires-clause, and for templates equivalent template
    /// heads ([temp.over.link]/6). The return type does not take part,
    /// for templates either (clang's `UseMemberUsingDeclRules`; gcc
    /// compares template return types): `M& operator=(int)` hides a
    /// using-declared `B& operator=(int)`. In an instantiated class the
    /// comparison is between the instantiated declarations.
    pub(crate) fn member_hides_using_target(&mut self, own: DeclId, target: DeclId) -> bool {
        let is_t = |me: &Self, x| me.kind_of(x) == DeclKind::FunctionTemplate;
        is_t(self, own) == is_t(self, target) && self.signatures_correspond(target, own, true)
    }

    /// Checks a new declaration `d` named `name` against the using-shadows
    /// of the current scope ([namespace.udecl]/11); on a conflict, reports
    /// it, marks `d` invalid and returns true.
    pub(crate) fn check_using_shadow_conflict(&mut self, d: DeclId, name: DeclName) -> bool {
        let mut shadows = self.current_scope_raw_dn(name);
        shadows.retain(|&s| self.kind_of(s) == DeclKind::UsingShadow);
        for s in shadows.into_iter().rev() {
            let t = self.ast.strip_shadow(s);
            if self.ast.first(t) == self.ast.first(d) {
                continue;
            }
            let fnlike = |me: &Self, x| {
                let k = me.kind_of(x);
                k.is_function() || k == DeclKind::FunctionTemplate
            };
            let in_class = self.member_spec_class().is_some();
            if in_class && fnlike(self, d) && fnlike(self, t) {
                // [namespace.udecl]/14: a member function (template)
                // hides a corresponding one the shadow introduced;
                // otherwise they overload.
                if self.member_hides_using_target(d, t) {
                    let _ = self.ast.decls.set_hidden_shadow(s, d);
                }
                continue;
            }
            if !self.using_conflicts(t, d) {
                continue;
            }
            if in_class && fnlike(self, d) {
                // [namespace.udecl]/14: in a class the member hides the
                // shadow instead of conflicting with it.
                let _ = self.ast.decls.set_hidden_shadow(s, d);
                continue;
            }
            let msg = self
                .diag(DiagId::sema_using_decl_conflict_reverse, self.decl_loc(d))
                .with_note(self.diag(DiagId::note_using_decl_target, self.decl_loc(t)))
                .with_note(self.diag(DiagId::note_using_decl, self.decl_loc(s)));
            self.emit(msg);
            self.ast.decls.set_flags(d, kiln_ast::decl::DeclFlags::INVALID);
            return true;
        }
        false
    }

    /// A use of a name whose lookup found only an unresolved
    /// `using_if_exists` target: "reference to unresolved using
    /// declaration".
    pub(crate) fn diag_unresolved_using(&self, uuie: DeclId, loc: Loc) {
        let msg = self
            .diag(DiagId::sema_use_of_empty_using_if_exists, loc)
            .with_note(self.diag(DiagId::note_empty_using_if_exists_here, self.decl_loc(uuie)));
        self.emit(msg);
    }

    // -- using enum --------------------------------------------------------------------

    /// `using enum name;` ([enum.udecl]): `class` is what the (possibly
    /// qualified) name classified as. Introduces a shadow of each
    /// enumerator.
    pub fn act_on_using_enum(
        &mut self,
        qual: Option<Qualifier>,
        class: NameClass,
        name: Symbol,
        name_loc: Loc,
    ) -> Option<DeclId> {
        let mut written = qual.map(|q| self.nns_str(q.nns)).unwrap_or_default();
        written.push_str(&sym_str(self, name));
        let NameClass::Type(ty, td) = class else {
            let msg = self
                .diag(DiagId::sema_using_enum_unknown_typename, name_loc)
                .arg(sym_str(self, name));
            self.emit(msg);
            return None;
        };
        let Some(e) = self.ast.types.as_enum(ty) else {
            let msg = self.diag(DiagId::sema_using_enum_not_enum, name_loc).arg(written);
            self.emit(msg);
            return None;
        };
        // [enum.udecl]/1: the using-enum-declarator designates a
        // non-dependent type (a member enumeration of the current
        // instantiation, or a local one of a template, is dependent).
        if self.ast.types.is_dependent(ty) {
            self.error(DiagId::sema_using_enum_is_dependent, name_loc);
            return None;
        }
        self.mark_type_referenced(td);
        Some(self.using_enum_of(e, &written, name_loc))
    }

    /// The using-enum-declaration for enumeration `e` (`written` is how
    /// the declarator spelled it): checks completeness and introduces a
    /// shadow of each enumerator in the current scope. Also the
    /// instantiation of a block-scope `using enum` in a template.
    pub(crate) fn using_enum_of(&mut self, e: DeclId, written: &str, name_loc: Loc) -> DeclId {
        // A scoped member enumeration of a class template specialization
        // is defined when first required ([temp.inst]/3).
        self.require_member_enum(e, name_loc);
        let ename = self.ast.decls.get(e).name();
        let h = DeclHead::new(self.ctx, ename, name_loc);
        let Some(def) = self.ast.definition(e) else {
            // [enum.udecl]/2: the enumeration must be complete.
            let msg = self
                .diag(DiagId::sema_using_enum_incomplete, name_loc)
                .arg(written)
                .with_note(self.diag(DiagId::note_declared_at, self.decl_loc(e)));
            self.emit(msg);
            let ud = self.builder().using_enum(h, e);
            self.ast.decls.set_flags(ud, kiln_ast::decl::DeclFlags::INVALID);
            return ud;
        };
        let ud = self.builder().using_enum(h, def);
        let enumerators: Vec<DeclId> = self
            .ast
            .decls
            .members(def)
            .filter(|&m| self.kind_of(m) == DeclKind::EnumConstant)
            .collect();
        for c in enumerators {
            if let Some(n) = self.ast.decls.get(c).name().as_ident() {
                let _ = self.introduce_shadow(DeclName::ident(n), name_loc, c);
            }
        }
        ud
    }

    // -- namespace aliases -------------------------------------------------------------

    /// The namespace or namespace alias a namespace-name `name` (in
    /// `qual`, or unqualified) finds. The lookup considers only
    /// namespaces ([namespace.udir]/1, [namespace.alias]/1;
    /// [basic.lookup.qual]/1 kinds), so a function or variable of the
    /// same name does not hide one: `using namespace index;` after
    /// `using namespace clang;` with `::index` from <strings.h> nominates
    /// `clang::index`.
    pub fn lookup_namespace_name(&self, qual: Option<DeclId>, name: Symbol) -> Option<DeclId> {
        let found = match qual {
            Some(q) => self.lookup_qualified(q, name, LookupKind::Nested),
            None => self.lookup_name(name, LookupKind::Nested),
        };
        found
            .decls
            .iter()
            .copied()
            .find(|&d| matches!(self.kind_of(d), DeclKind::Namespace | DeclKind::NamespaceAlias))
    }

    /// `namespace name = qual target;` ([namespace.alias]).
    pub fn act_on_namespace_alias(
        &mut self,
        name: Symbol,
        name_loc: Loc,
        qual: Option<Qualifier>,
        target: Symbol,
        target_loc: Loc,
    ) -> Option<DeclId> {
        let Some(t) = self.lookup_namespace_name(qual.map(|q| q.ctx), target) else {
            self.error(DiagId::parse_expected_namespace_name, target_loc);
            return None;
        };
        self.act_on_namespace_name_use(t, target_loc);
        let ns = self.ast.resolve_alias(t);
        let mut prev = None;
        for p in self.lookup_current_scope(name) {
            match self.kind_of(p) {
                DeclKind::NamespaceAlias if self.ast.resolve_alias(p) == ns => prev = Some(p),
                DeclKind::NamespaceAlias => {
                    let was = self.decl_name(self.ast.resolve_alias(p));
                    let msg = self
                        .diag(DiagId::sema_redefinition_different_namespace_alias, name_loc)
                        .arg(sym_str(self, name))
                        .with_note(
                            self.diag(DiagId::note_previous_namespace_alias, self.decl_loc(p))
                                .arg(was),
                        );
                    self.emit(msg);
                    return None;
                }
                k => {
                    let id = if k == DeclKind::Namespace {
                        DiagId::sema_redefinition
                    } else {
                        DiagId::sema_redefinition_different_kind
                    };
                    let msg = self.diag(id, name_loc).arg(sym_str(self, name));
                    let msg = self.note_prev(msg, p, true);
                    self.emit(msg);
                    return None;
                }
            }
        }
        let h = DeclHead::new(self.ctx, DeclName::ident(name), name_loc);
        let d = self.builder().namespace_alias(h, t, prev);
        if self.in_local_scope() {
            self.push_local(name, d);
        }
        Some(d)
    }
}
