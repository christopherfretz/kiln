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
            // A dependent namespace alias ([temp.dep.namespace]/1) is the
            // dependent `[: e :]::` it stands for, substituted as one.
            DeclKind::NamespaceAlias if let Some(e) = self.dependent_alias_splice(resolved) => {
                Nns::Splice(e)
            }
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

    /// [temp.variadic]/5, /12: the qualifier or the name (a
    /// conversion-function-id's type) of a using-declarator that is a
    /// pack expansion names a pack; one that is not names none (clang's
    /// `ActOnUsingDeclaration`: without packs the `...` is diagnosed and
    /// dropped). The `...` kept, or `Err` after an unexpanded pack.
    fn using_declarator_packs(
        &mut self,
        q: &Qualifier,
        name: DeclName,
        name_loc: Loc,
        ellipsis: Option<Loc>,
    ) -> Result<Option<Loc>, ()> {
        use crate::templ::pack::PackOperand;
        let name_ty = match self.ast.names.view(name) {
            kiln_ast::names::DeclNameView::Conversion(t) => Some(t),
            _ => None,
        };
        if let Some(l) = ellipsis {
            if !self.operand_has_packs(PackOperand::Nns(q.nns))
                && !name_ty.is_some_and(|t| self.has_unexpanded_pack(t))
            {
                self.error(DiagId::sema_pack_expansion_without_packs, l);
                return Ok(None);
            }
            return Ok(ellipsis);
        }
        let what = "using declaration";
        if self.diagnose_unexpanded_operand_written(PackOperand::Nns(q.nns), what, q.loc)
            || name_ty.is_some_and(|t| {
                self.diagnose_unexpanded_operand_written(PackOperand::Type(t), what, name_loc)
            })
        {
            return Err(());
        }
        Ok(None)
    }

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
        let ellipsis = self.using_declarator_packs(&q, name, name_loc, ellipsis).ok()?;
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
        if self.in_local_scope() && dependent {
            // A dependent qualifier in a function template (`using
            // E<T>::A;`, an enumerator, [namespace.udecl]/8): clang's
            // `UnresolvedUsingValueDecl`, a local name whose uses are
            // dependent; the instantiation redoes the declarator
            // (`subst/stmts.rs` `local_using_decl`). An operator or
            // conversion name (`using T::operator+;`) never names an
            // enumerator, so it is diagnosed there ("using declaration
            // cannot refer to class member"); the local scope records
            // only identifiers (`Scope.local_names`), so it names nothing
            // meanwhile.
            let h = DeclHead::new(self.ctx, name, name_loc);
            let d = self.builder().unresolved_using_value(h, q.nns, false);
            if using_loc != name_loc {
                // The instantiation's notes point at the `using`.
                let _ = self.ast.decls.cold.begin_loc.insert(d, using_loc);
            }
            if let Some(l) = ellipsis {
                // A pack expansion (`using E<Ts>::A...;`): expanded when
                // instantiated, where more than one element is an error.
                let _ = self.ast.decls.cold.declarator_end.insert(d, l);
            }
            if let Some(n) = name.as_ident() {
                self.push_local(n, d);
            }
            return Some(d);
        }
        self.unrepeated_enum_using(q.ctx, name, name_loc)?;
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
                let shadow = self.introduce_shadow(name, name_loc, uuie);
                self.link_using_shadows(using, shadow.as_slice());
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
        let shadows: Vec<DeclId> =
            found.decls.iter().filter_map(|&t| self.introduce_shadow(name, name_loc, t)).collect();
        self.link_using_shadows(using, &shadows);
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
            if let Some(class) = self.member_spec_class()
                && self.repeated_dependent_using(
                    class,
                    DeclKind::UnresolvedUsingTypename,
                    q.nns,
                    name,
                    name_loc,
                )
            {
                return None;
            }
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
    #[allow(clippy::too_many_lines, reason = "one arm per using-declarator form")]
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
        // Inheriting constructors are C++11 ([namespace.udecl]/3; clang
        // `err_using_decl_constructor`).
        if inheriting && self.std < kiln_lex::Std::Cxx11 {
            self.error(DiagId::sema_using_decl_constructor, name_loc);
            return None;
        }
        if self.is_dependent_ctx(q.ctx)
            || self
                .ast
                .types
                .nns_flags(q.nns)
                .intersects(kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION)
                && qk != DeclKind::CXXRecord
            // clang's `isProvablyNotDerivedFrom` cannot prove a class with a
            // dependent base is not derived from itself: that waits too.
            || self.ast.first(q.ctx) == self.ast.first(class) && self.has_dependent_bases(class)
        {
            // The current instantiation is looked up now all the same
            // ([temp.dep.type]/1, clang `LookupQualifiedName`): a name it
            // does not declare (dependent bases aside) is "no member
            // named" at the definition, and the declaration is dropped.
            if !inheriting
                && self.ast.first(q.ctx) == self.ast.first(class)
                && self.has_dependent_bases(class)
                && self.lookup_in_dn(q.ctx, name, LookupKind::All).is_empty()
            {
                let msg = self
                    .diag(DiagId::sema_no_member, name_loc)
                    .arg(self.decl_name_str(name))
                    .arg(self.qualifier_desc(q.ctx));
                self.emit(msg);
                return None;
            }
            return self.dependent_class_using(class, q, name, name_loc, inheriting);
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
                return self.dependent_class_using(class, q, name, name_loc, inheriting);
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
        // [class.mem.general]/22: a member named by a using-declarator has
        // a name different from the class; a non-static data member only
        // when the class has a user-declared constructor, checked as the
        // class completes (`check_field_named_as_class`).
        if let Some(n) = name.as_ident()
            && found.decls.iter().any(|&t| self.kind_of(t) != DeclKind::Field)
            && self.diagnose_class_name_shadow(class, n, name_loc)
        {
            return None;
        }
        // [namespace.udecl]/10 (clang `CheckUsingDeclRedeclaration`): a
        // using-declarator in a class repeats an earlier one only when both
        // name the member through the same class. Through another class
        // (`using A::f; using B::f;`, B derived from A) the members already
        // introduced are not introduced again (`introduce_shadow`).
        if self.repeated_class_using(class, q.ctx, name, name_loc) {
            return None;
        }
        let access = self.current_member_access();
        let h = DeclHead::new(self.ctx, name, name_loc).access(access);
        let using = self.builder().using_decl(h, Some(q.nns));
        if name.as_operator() == Some(kiln_ast::names::OverloadedOperator::Equal) {
            self.note_inherited_assign(class);
        }
        let mut targets = found.decls.clone();
        if self.reverse_using_shadows {
            targets.reverse();
        }
        let shadows: Vec<DeclId> =
            targets.into_iter().filter_map(|t| self.introduce_shadow(name, name_loc, t)).collect();
        self.link_using_shadows(using, &shadows);
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
        self.dependent_class_using(class, q, name, name_loc, inheriting)
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
    ) -> Option<DeclId> {
        if !inheriting
            && self.repeated_dependent_using(
                class,
                DeclKind::UnresolvedUsingValue,
                q.nns,
                name,
                name_loc,
            )
        {
            return None;
        }
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
        Some(d)
    }

    /// [namespace.udecl]/10 for a dependent using-declarator in `class`
    /// (clang `CheckUsingDeclRedeclaration`, WP-1015): an earlier one of
    /// the same kind (`typename` or not) naming `name` through the same
    /// canonical qualifier makes this one a redeclaration, reported at
    /// the name with a note at the earlier one; true when it did.
    fn repeated_dependent_using(
        &mut self,
        class: DeclId,
        kind: DeclKind,
        nns: NnsId,
        name: DeclName,
        name_loc: Loc,
    ) -> bool {
        let def = self.ast.definition(class).unwrap_or(class);
        let prev = self
            .ast
            .decls
            .members(def)
            .filter(|&m| self.kind_of(m) == kind && self.ast.decls.get(m).name() == name)
            .find(|&m| self.ast.using_qualifier(m).is_some_and(|p| self.ast.types.same_nns(p, nns)))
            .map(|m| self.ast.decls.get(m).loc());
        let Some(prev) = prev else { return false };
        let msg = self
            .diag(DiagId::sema_using_decl_redeclaration, name_loc)
            .with_note(self.diag(DiagId::note_previous_using_decl, prev));
        self.emit(msg);
        true
    }

    /// Sets `HAS_INHERITED_CTOR` on the class being defined.
    fn note_inherited_ctor(&mut self, class: DeclId) {
        if let Some(c) = self.classes.last_mut()
            && c.record == class
        {
            c.bits = c.bits.with(kiln_ast::decl::DefinitionBits::HAS_INHERITED_CTOR);
        }
    }

    /// Sets `HAS_INHERITED_ASSIGN` on the class being defined: a
    /// using-declarator names a base `operator=` (clang `addedMember`'s
    /// `HasInheritedAssignment`). The class then declares its implicit
    /// copy and move assignment operators at `}`
    /// (`finish_special_members`), so a shadow one of them hides is marked
    /// when the class is completed, whatever the unit uses later
    /// ([namespace.udecl]/4, /14; WP-3146).
    fn note_inherited_assign(&mut self, class: DeclId) {
        if let Some(c) = self.classes.last_mut()
            && c.record == class
        {
            c.bits = c.bits.with(kiln_ast::decl::DefinitionBits::HAS_INHERITED_ASSIGN);
        }
    }

    /// `using B::B;` naming direct base `q.ctx` of `class`
    /// ([namespace.udecl]/3, [class.inhctor.init]): a `Using` named by the
    /// class's constructor name and one `ConstructorUsingShadow` per
    /// constructor of the base (including the ones it inherits itself);
    /// one a constructor of `class` already declared with the same
    /// signature hides is marked hidden by it ([namespace.udecl]/14;
    /// later ones mark at `}`).
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
        let mut shadows = Vec::with_capacity(inherited.len());
        for (target, nominated) in inherited {
            // A constructor the class declared earlier with the same
            // signature hides the shadow, which is made and marked hidden
            // as by a later one (`hide_inherited_ctors`): one record for
            // both orders (WP-3145).
            let by = own.iter().copied().find(|&c| self.hides_ctor(c, target));
            let h = DeclHead::new(self.ctx, cname, name_loc).access(access);
            let s = self.builder().constructor_using_shadow(h, target, nominated);
            if let Some(by) = by {
                let _ = self.ast.decls.set_hidden_shadow(s, by);
            }
            shadows.push(s);
        }
        self.link_using_shadows(using, &shadows);
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

    /// [namespace.udecl]/10: `using E::x;` repeated in one class through
    /// the enumeration `qual` is a redeclaration (reported); `None` then.
    fn unrepeated_enum_using(&mut self, qual: DeclId, name: DeclName, at: Loc) -> Option<()> {
        let class = self.member_spec_class();
        match class {
            Some(c) if self.repeated_class_using(c, qual, name, at) => None,
            _ => Some(()),
        }
    }

    /// [`Sema::previous_class_using`] reported: "redeclaration of using
    /// declaration" at `name_loc` with a note at the earlier one; true
    /// when there was one.
    fn repeated_class_using(
        &mut self,
        class: DeclId,
        qual: DeclId,
        name: DeclName,
        name_loc: Loc,
    ) -> bool {
        let Some(prev) = self.previous_class_using(class, qual, name) else { return false };
        let msg = self
            .diag(DiagId::sema_using_decl_redeclaration, name_loc)
            .with_note(self.diag(DiagId::note_previous_using_decl, prev));
        self.emit(msg);
        true
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
                // A member enumeration's qualifier too (`using E::x;`
                // twice, [enum.udecl]).
                let t = &self.ast.types;
                t.as_record(ty).or_else(|| t.as_enum(ty)).is_some_and(|r| self.ast.first(r) == qual)
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
        let mut hidden_by = None;
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
                } else if in_class && self.kind_of(es) == DeclKind::EnumConstant {
                    // The enumerator of a member enumeration is already a
                    // member of this class (`enum E { A }; using E::A;` or
                    // `using enum E;` in the class): the using-declarator
                    // names the same entity, so it does not conflict
                    // ([basic.scope.scope]/6: only different entities do),
                    // nor is it a second member declaration
                    // ([class.mem.general]/5: a using-declarator declares
                    // no member; two using-declarators naming one
                    // declaration are [namespace.udecl]/8's case). It
                    // makes no new shadow. clang 22 accepts; GCC 16
                    // rejects (docs/status/deviations.md, WP-1631).
                    return None;
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
            // A dependent member using-declarator (`using B<T>::f;`) is
            // assumed to introduce functions that overload with this one;
            // a conflict is found at instantiation (clang `CheckOverload`,
            // `UnresolvedUsingValueDecl`; Boost.Cobalt's promises, WP-812).
            if in_class
                && self.kind_of(es) == DeclKind::UnresolvedUsingValue
                && fnlike(self, target)
            {
                continue;
            }
            if in_class && e == es && fnlike(self, es) && fnlike(self, target) {
                // [namespace.udecl]/14: a member function (template) of
                // the class hides the one the declarator would introduce
                // when they correspond; otherwise they overload. The
                // shadow is made and marked hidden, as when the member is
                // declared after the using-declarator
                // (`check_using_shadow_conflict`): one record for both
                // orders (WP-3145).
                if hidden_by.is_none() && self.member_hides_using_target(es, target) {
                    hidden_by = Some(es);
                }
                continue;
            }
            if self.using_conflicts(target, es) {
                self.emit_using_conflict(loc, target, es);
                return None;
            }
        }
        let mut h = DeclHead::new(self.ctx, name, loc);
        if in_class {
            h = h.access(self.current_member_access());
        }
        let s = self.builder().using_shadow(h, target, prev);
        if let Some(by) = hidden_by {
            let _ = self.ast.decls.set_hidden_shadow(s, by);
        }
        if self.in_local_scope() {
            match name.as_ident() {
                Some(n) => self.push_local(n, s),
                None => self.push_local_name(name, s),
            }
        }
        Some(s)
    }

    /// Records the shadows using-declaration `using` introduced
    /// (`Ast::using_shadows`, spec/ast.md §4.2; WP-2025).
    pub(crate) fn link_using_shadows(&mut self, using: DeclId, shadows: &[DeclId]) {
        let r = self.builder().set_using_shadows(using, shadows);
        debug_assert!(r.is_ok(), "using-declaration shadows linked twice: {r:?}");
    }

    /// "target of using declaration conflicts with declaration already in
    /// scope" at `loc`, noting `target` and the conflicting `es`.
    fn emit_using_conflict(&mut self, loc: Loc, target: DeclId, es: DeclId) {
        let msg = self
            .diag(DiagId::sema_using_decl_conflict, loc)
            .with_note(self.diag(DiagId::note_using_decl_target, self.decl_loc(target)))
            .with_note(self.diag(DiagId::note_using_decl_conflict, self.decl_loc(es)));
        self.emit(msg);
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
        // A dependent using-declarator (`using A::f;` with `A` a
        // dependent base) names nothing yet: it is checked when the
        // template is instantiated, not against the members and the
        // using-declarators that follow it (clang `CheckUsingShadowDecl`
        // skips an `UnresolvedUsingValueDecl`; WP-811).
        let unresolved =
            |k| matches!(k, DeclKind::UnresolvedUsingIfExists | DeclKind::UnresolvedUsingValue);
        if unresolved(ka) || unresolved(kb) {
            return false;
        }
        let typedef = |k| matches!(k, DeclKind::Typedef | DeclKind::TypeAlias);
        if is_tag_kind(ka) || is_tag_kind(kb) {
            // In a class or a block, a class or enumeration and a
            // typedef-name of another type correspond
            // ([basic.scope.scope]/4 exempts only a variable, data
            // member, enumerator or function), so they conflict
            // ([namespace.udecl]/11; GCC 16; clang 22 lets a tag coexist
            // with any non-tag). At namespace scope clang's rule is kept
            // (GCC 16 accepts that case too; docs/status/deviations.md).
            if typedef(ka) || typedef(kb) {
                let (tag, td) = if typedef(kb) { (a, b) } else { (b, a) };
                return (self.member_spec_class().is_some() || self.in_local_scope())
                    && !self.typedef_names_tag(td, tag);
            }
            return is_tag_kind(ka) && is_tag_kind(kb);
        }
        let ta = self.ast.decls.get(a).ty().unwrap_or(QualType::ERROR);
        let tb = self.ast.decls.get(b).ty().unwrap_or(QualType::ERROR);
        if typedef(ka) && typedef(kb) {
            return !self.ast.types.same_type(ta, tb);
        }
        if ka.is_function() && kb.is_function() {
            // A `pass_object_size` function overloads one without
            // (spec/sema-overload.md §4.3a; WP-803).
            // Functions conflict when their parameter-type-lists (and
            // trailing requires-clauses) correspond; the return type
            // does not take part ([namespace.udecl]/11 per CWG101,
            // [basic.scope.scope]/4; clang `IsOverload`): `int f();`
            // conflicts with a using-declared `void f();`.
            // kiln: not at the instantiation of a block scope, where
            // clang's lookup of the earlier declaration misses a local
            // function declaration (and gcc accepts too; clang's
            // namespace.udecl/p11.cpp FIXME, test5).
            if self.in_local_scope() && !self.inst.stack.is_empty() {
                return false;
            }
            let ps = |me: &Self, d| me.ast.pass_object_size_params(d).is_empty();
            return ps(self, a) == ps(self, b)
                && (self.same_ignoring_exc_spec(ta, tb)
                    || self.signatures_correspond(a, b, false));
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

    /// Whether typedef-name `td` denotes the class or enumeration `tag`
    /// declares (the same entity, so no conflict).
    fn typedef_names_tag(&self, td: DeclId, tag: DeclId) -> bool {
        let Some(u) = self.ast.decls.typedef(td).map(|x| x.underlying) else { return false };
        let t = &self.ast.types;
        if t.is_dependent(u) {
            return true;
        }
        let named = t.as_record(u).or_else(|| t.as_enum(u));
        named.is_some_and(|r| self.ast.first(r) == self.ast.first(tag))
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
    /// comparison is between the instantiated declarations. A member of
    /// a class template specialization keeps its pattern's
    /// requires-clause: clauses equivalent as written correspond only
    /// when the enclosing levels they name have the same arguments
    /// (`requires C<T>` of `A<T*>` and of `B<T>` differ in `B<int>`;
    /// clang `AreConstraintExpressionsEqual` with each declaration's
    /// template arguments).
    pub(crate) fn member_hides_using_target(&mut self, own: DeclId, target: DeclId) -> bool {
        let is_t = |me: &Self, x| me.kind_of(x) == DeclKind::FunctionTemplate;
        if is_t(self, own) != is_t(self, target) {
            return false;
        }
        if self.signatures_correspond(target, own, true)
            && self.kept_constraints_levels_agree(own, target)
        {
            return true;
        }
        self.correspond_substituted(own, target)
    }

    /// For [`Self::member_hides_using_target`]: whether `own` and `target`
    /// correspond with the kept (pattern) constraints of each compared as
    /// substituted with its class's arguments ([temp.over.link]/5-7, clang
    /// `AreConstraintExpressionsEqual`): `requires C<T>` of `B<int>`
    /// corresponds to `requires C<int>` of a non-template base, and to
    /// `requires C<T>` of the partial specialization `A<T*>` matched by
    /// `A<int*>`. False when neither keeps its pattern's constraints.
    fn correspond_substituted(&mut self, own: DeclId, target: DeclId) -> bool {
        let mut cmp = kiln_source::fx::FxHashMap::default();
        for d in [own, target] {
            if let Some((pt, outer)) = self.kept_constraints(d) {
                let m = self.cmp_constraints_of(d, pt, outer);
                cmp.extend(m);
            }
        }
        if cmp.is_empty() {
            return false;
        }
        for (&k, &v) in &self.friend_cmp_constraints {
            cmp.entry(k).or_insert(v);
        }
        let saved = std::mem::replace(&mut self.friend_cmp_constraints, cmp);
        let ok = self.signatures_correspond(target, own, true);
        self.friend_cmp_constraints = saved;
        ok
    }

    /// For [`Self::member_hides_using_target`]: whether the enclosing
    /// levels the kept (pattern) constraints of `a` and `b` name have the
    /// same arguments; true when neither keeps its pattern's.
    fn kept_constraints_levels_agree(&mut self, a: DeclId, b: DeclId) -> bool {
        let (Some((pa, la)), Some((pb, lb))) = (self.kept_constraints(a), self.kept_constraints(b))
        else {
            return true;
        };
        // A templated non-template member's constraint is its trailing
        // requires-clause alone.
        let own = |me: &Self, x| {
            let mut v = me.associated_constraints(x);
            if me.ast.template_info(x).is_none() {
                v.extend(me.ast.templates.trailing_requires.get(x).copied());
            }
            v
        };
        let mut exprs = own(self, pa);
        exprs.extend(own(self, pb));
        for d in 0..la.levels.len().max(lb.levels.len()) {
            let Ok(level) = u16::try_from(d) else { break };
            let mut mask = 0u64;
            for &e in &exprs {
                mask |= self.used_template_params(e, level).unwrap_or(u64::MAX);
            }
            if mask == 0 {
                continue;
            }
            let args = |l: &crate::templ::args::MultiLevelArgs| {
                l.levels.get(d).map_or(kiln_ast::ids::ArgListId::EMPTY, |x| x.args)
            };
            let (x, y) = (args(&la), args(&lb));
            let t = &self.ast.types;
            let (x, y) = (t.args(t.canonical_args(x)), t.args(t.canonical_args(y)));
            let differ = (0..x.len().max(y.len()))
                .any(|i| (i >= 64 || mask & (1 << i) != 0) && x.get(i) != y.get(i));
            if differ {
                return false;
            }
        }
        true
    }

    /// The constraints member `d` of a class template specialization
    /// keeps and the levels they are checked with: its
    /// `member_constraints` entry, or, while it is being declared in an
    /// instantiated class (before that entry is recorded), its own with
    /// the class's arguments.
    fn kept_constraints(&self, d: DeclId) -> Option<(DeclId, crate::templ::args::MultiLevelArgs)> {
        if let Some(k) = self.inst.concepts.member_constraints.get(&d) {
            return Some(k.clone());
        }
        let ctx = self.ast.decls.get(d).ctx()?;
        (self.kind_of(ctx) == DeclKind::CXXRecord).then_some(())?;
        Some((d, self.spec_args(ctx)?.1))
    }

    /// Whether `found`, what a lookup of `name` in the current class
    /// found, is the target of a using-declarator of that class: a new
    /// member of that name is then checked by
    /// [`Self::check_using_shadow_conflict`], not reported as a duplicate.
    pub(crate) fn found_through_using_shadow(&self, name: DeclName, found: DeclId) -> bool {
        self.current_scope_raw_dn(name)
            .into_iter()
            .any(|s| self.kind_of(s) == DeclKind::UsingShadow && self.ast.strip_shadow(s) == found)
    }

    /// Checks a new declaration `d` named `name` against the using-shadows
    /// of the current scope ([namespace.udecl]/11); on a conflict, reports
    /// it, marks `d` invalid and returns true.
    pub(crate) fn check_using_shadow_conflict(&mut self, d: DeclId, name: DeclName) -> bool {
        self.check_using_shadow_conflict_at(d, name, None)
    }

    /// [`Self::check_using_shadow_conflict`] for a declaration whose
    /// conflict is reported at `at` (a class or enumeration declaration:
    /// its class-key, clang's `KWLoc`) instead of at its name.
    pub(crate) fn check_using_shadow_conflict_at(
        &mut self,
        d: DeclId,
        name: DeclName,
        at: Option<Loc>,
    ) -> bool {
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
            if is_tag_kind(self.kind_of(d)) && self.tag_conflicts_with_using_target(t) {
                return self.report_using_shadow_conflict(d, s, t, at);
            }
            if in_class && fnlike(self, d) && fnlike(self, t) {
                // [namespace.udecl]/14: a member function (template)
                // hides a corresponding one the shadow introduced;
                // otherwise they overload.
                if self.member_hides_using_target(d, t) {
                    let _ = self.ast.decls.set_hidden_shadow(s, d);
                }
                continue;
            }
            let typedef =
                |me: &Self, x| matches!(me.kind_of(x), DeclKind::Typedef | DeclKind::TypeAlias);
            if typedef(self, d) && typedef(self, t) {
                // [namespace.udecl]/11 with [basic.scope.scope]/4: a
                // typedef-name redeclaring the typedef-name a
                // using-declarator named conflicts only when it denotes
                // another type; checked as a redeclaration of the target
                // (clang's `MergeTypedefNameDecl` sees through the
                // shadow; in a class, [class.mem.general]/6 makes any
                // redeclaration an error).
                let Some(name) = name.as_ident() else { continue };
                let ty = self.ast.decls.typedef(d).map_or(QualType::ERROR, |x| x.underlying);
                let loc = self.decl_loc(d);
                if self.check_typedef_redefinition(t, ty, false, name, loc).is_some() {
                    continue;
                }
                self.invalidate(d);
                return true;
            }
            if !self.using_conflicts(t, d) {
                continue;
            }
            return self.report_using_shadow_conflict(d, s, t, at);
        }
        false
    }

    /// Reports new declaration `d` conflicting with target `t` of
    /// using-shadow `s` and marks `d` invalid; true.
    fn report_using_shadow_conflict(
        &mut self,
        d: DeclId,
        s: DeclId,
        t: DeclId,
        at: Option<Loc>,
    ) -> bool {
        let msg = self
            .diag(DiagId::sema_using_decl_conflict_reverse, at.unwrap_or(self.decl_loc(d)))
            .with_note(self.diag(DiagId::note_using_decl_target, self.decl_loc(t)))
            .with_note(self.diag(DiagId::note_using_decl, self.decl_loc(s)));
        self.emit(msg);
        self.invalidate(d);
        true
    }

    /// Whether a class or enumeration declared in the scope of a
    /// using-declarator conflicts with the declarator's target `t` that
    /// is no class or enumeration: they correspond unless `t` is a
    /// variable, data member, enumerator, function or function template
    /// ([basic.scope.scope]/4.2), so a typedef-name, an alias, a class,
    /// alias or variable template or a concept conflicts
    /// ([namespace.udecl]/11; clang's `ActOnTag` finds them by tag lookup;
    /// GCC 16 accepts the templates and concepts but for a class
    /// template outside a class). A class or enumeration target is
    /// checked by [`Self::using_conflicts`].
    fn tag_conflicts_with_using_target(&self, t: DeclId) -> bool {
        matches!(
            self.kind_of(t),
            DeclKind::Typedef
                | DeclKind::TypeAlias
                | DeclKind::ClassTemplate
                | DeclKind::TypeAliasTemplate
                | DeclKind::VarTemplate
                | DeclKind::Concept
        )
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
    /// `using enum [: r :];` ([enum.udecl]/1 with a splice-type-specifier,
    /// P2996): the enumeration type `ty` the splice designates, at `loc`.
    pub fn act_on_using_enum_type(&mut self, ty: QualType, loc: Loc) -> Option<DeclId> {
        let written = self.ty_str(ty);
        let Some(e) = self.ast.types.as_enum(ty) else {
            let msg = self.diag(DiagId::sema_using_enum_not_enum, loc).arg(written);
            self.emit(msg);
            return None;
        };
        if self.ast.types.is_dependent(ty) {
            self.error(DiagId::sema_using_enum_is_dependent, loc);
            return None;
        }
        self.mark_type_referenced(e);
        Some(self.using_enum_of(e, &written, loc))
    }

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
            self.invalidate(ud);
            return ud;
        };
        let ud = self.builder().using_enum(h, def);
        let enumerators: Vec<DeclId> = self
            .ast
            .decls
            .members(def)
            .filter(|&m| self.kind_of(m) == DeclKind::EnumConstant)
            .collect();
        let shadows: Vec<DeclId> = enumerators
            .into_iter()
            .filter_map(|c| {
                let n = self.ast.decls.get(c).name().as_ident()?;
                self.introduce_shadow(DeclName::ident(n), name_loc, c)
            })
            .collect();
        self.link_using_shadows(ud, &shadows);
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
            Some(q) => self.lookup_qualified(q, name, LookupKind::Namespace),
            None => self.lookup_name(name, LookupKind::Namespace),
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
        self.act_on_namespace_alias_to(name, name_loc, t)
    }

    /// `namespace name = t;` for the namespace (or alias) `t` the
    /// qualified-namespace-specifier denotes or the splice-specifier
    /// designates ([namespace.alias]/1, /4).
    pub fn act_on_namespace_alias_to(
        &mut self,
        name: Symbol,
        name_loc: Loc,
        t: DeclId,
    ) -> Option<DeclId> {
        // [temp.local]/6 (WP-1042).
        self.check_template_param_shadow(name, name_loc);
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
