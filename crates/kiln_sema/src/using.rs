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
use kiln_ast::names::DeclName;
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
    fn member_spec_class(&self) -> Option<DeclId> {
        let s = self.scopes.iter().rev().find(|s| !s.is_transparent())?;
        if s.kind == ScopeKind::Record { s.entity } else { None }
    }

    // -- using-declarations ------------------------------------------------------------

    /// One using-declarator `using qual name;` ([namespace.udecl]).
    /// `using_loc` is the `using` keyword; `if_exists` is set by the
    /// `using_if_exists` attribute. Returns the `Using` declaration, or
    /// `None` after an error.
    pub fn act_on_using_declarator(
        &mut self,
        using_loc: Loc,
        qual: Option<Qualifier>,
        name: Symbol,
        name_loc: Loc,
        if_exists: bool,
    ) -> Option<DeclId> {
        // [namespace.udecl]/1: the declarator's name is qualified.
        let Some(q) = qual else {
            self.error(DiagId::sema_using_requires_qualname, name_loc);
            return None;
        };
        let qk = self.kind_of(q.ctx);
        if let Some(class) = self.member_spec_class()
            && qk != DeclKind::Enum
        {
            return self.class_using_declarator(class, q, name, name_loc, using_loc);
        }
        let found = self.lookup_qualified(q.ctx, name, LookupKind::All);
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
                let h = DeclHead::new(self.ctx, DeclName::ident(name), using_loc);
                let uuie = self.builder().unresolved_using_if_exists(h);
                let _ = self.introduce_shadow(name, name_loc, uuie);
                return Some(using);
            }
            let msg = self
                .diag(DiagId::sema_no_member, name_loc)
                .arg(sym_str(self, name))
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

    fn new_using_decl(&mut self, name: Symbol, name_loc: Loc, nns: NnsId) -> DeclId {
        let h = DeclHead::new(self.ctx, DeclName::ident(name), name_loc);
        self.builder().using_decl(h, Some(nns))
    }

    /// A using-declarator in a class ([namespace.udecl]/3): it names a
    /// member of a base class (`using B::f;`), or, when the name is the
    /// qualifier's last component (`using B::B;`, [class.qual]/2), the
    /// constructors of a direct base (WP-114). A dependent qualifier makes
    /// an `UnresolvedUsingValue` redone at instantiation.
    fn class_using_declarator(
        &mut self,
        class: DeclId,
        q: Qualifier,
        name: Symbol,
        name_loc: Loc,
        using_loc: Loc,
    ) -> Option<DeclId> {
        let qk = self.kind_of(q.ctx);
        if matches!(qk, DeclKind::Namespace | DeclKind::TranslationUnit) {
            let msg = self
                .diag(DiagId::sema_using_decl_nested_name_specifier_is_not_class, q.loc)
                .arg(self.nns_str(q.nns));
            self.emit(msg);
            return None;
        }
        let inheriting = self.nns_last_name(q.nns) == Some(name);
        if self.is_dependent_ctx(q.ctx) || self.ast.types.nns_flags(q.nns).intersects(
            kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION,
        ) && qk != DeclKind::CXXRecord
        {
            return Some(self.dependent_class_using(class, q, name, name_loc, inheriting));
        }
        if self.ast.first(q.ctx) == self.ast.first(class) {
            self.error(DiagId::sema_using_decl_refers_to_own_class, q.loc);
            if self.lookup_in(q.ctx, name, LookupKind::All).is_empty() {
                let msg = self
                    .diag(DiagId::sema_no_member, name_loc)
                    .arg(sym_str(self, name))
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
            let msg = if inheriting && crate::conv::bases::is_derived_from(self, class, q.ctx) {
                self.diag(DiagId::sema_using_decl_constructor_not_in_direct_base, using_loc)
                    .arg(self.ty_arg(self.class_type(q.ctx)))
                    .arg(self.ty_arg(self.class_type(class)))
            } else {
                self.diag(DiagId::sema_using_decl_not_base_class, q.loc)
                    .arg(self.nns_str(q.nns))
                    .arg(self.decl_name(class))
            };
            self.emit(msg);
            return None;
        }
        if inheriting {
            return Some(self.inheriting_using(class, q, name_loc));
        }
        let found = self.lookup_qualified(q.ctx, name, LookupKind::All);
        if found.is_empty() {
            let msg = self
                .diag(DiagId::sema_no_member, name_loc)
                .arg(sym_str(self, name))
                .arg(self.qualifier_desc(q.ctx));
            self.emit(msg);
            return None;
        }
        let access = self.current_member_access();
        let h = DeclHead::new(self.ctx, DeclName::ident(name), name_loc).access(access);
        let using = self.builder().using_decl(h, Some(q.nns));
        for &t in &found.decls {
            let _ = self.introduce_shadow(name, name_loc, t);
        }
        Some(using)
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
    fn dependent_class_using(
        &mut self,
        class: DeclId,
        q: Qualifier,
        name: Symbol,
        name_loc: Loc,
        inheriting: bool,
    ) -> DeclId {
        let access = self.current_member_access();
        let h = DeclHead::new(self.ctx, DeclName::ident(name), name_loc).access(access);
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
        for (target, nominated) in self.inheritable_ctors(base) {
            if own.iter().any(|&c| self.hides_ctor(c, target)) {
                continue;
            }
            let h = DeclHead::new(self.ctx, cname, name_loc).access(access);
            let _ = self.builder().constructor_using_shadow(h, target, nominated);
        }
        using
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
                c.record == def && c.bits.contains(kiln_ast::decl::DefinitionBits::HAS_INHERITED_CTOR)
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
    fn introduce_shadow(&mut self, name: Symbol, loc: Loc, target: DeclId) -> Option<DeclId> {
        let in_class = self.member_spec_class().is_some();
        let fnlike = |me: &Self, d| {
            let k = me.kind_of(d);
            k.is_function() || k == DeclKind::FunctionTemplate
        };
        let mut prev = None;
        for e in self.current_scope_raw(name) {
            let es = self.ast.strip_shadow(e);
            if self.ast.first(es) == self.ast.first(target) {
                if e != es {
                    if in_class && self.ast.decls.hidden_shadow(e).is_none() {
                        // [namespace.udecl]/10: a using-declarator in a
                        // class cannot be repeated.
                        let msg = self
                            .diag(DiagId::sema_using_decl_redeclaration, loc)
                            .with_note(self.diag(DiagId::note_previous_using_decl, self.decl_loc(e)));
                        self.emit(msg);
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
            if self.using_conflicts(target, es) {
                if in_class && e == es && fnlike(self, es) && fnlike(self, target) {
                    // [namespace.udecl]/14: a member function of the class
                    // hides the one the declarator would introduce.
                    return None;
                }
                let msg = self
                    .diag(DiagId::sema_using_decl_conflict, loc)
                    .with_note(self.diag(DiagId::note_using_decl_target, self.decl_loc(target)))
                    .with_note(self.diag(DiagId::note_using_decl_conflict, self.decl_loc(es)));
                self.emit(msg);
                return None;
            }
        }
        let mut h = DeclHead::new(self.ctx, DeclName::ident(name), loc);
        if in_class {
            h = h.access(self.current_member_access());
        }
        let s = self.builder().using_shadow(h, target, prev);
        if self.in_local_scope() {
            self.push_local(name, s);
        }
        Some(s)
    }

    fn decl_loc(&self, d: DeclId) -> Loc {
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
            return self.same_ignoring_exc_spec(ta, tb);
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

    /// Checks a new declaration `d` named `name` against the using-shadows
    /// of the current scope ([namespace.udecl]/11); on a conflict, reports
    /// it, marks `d` invalid and returns true.
    pub(crate) fn check_using_shadow_conflict(&mut self, d: DeclId, name: Symbol) -> bool {
        let shadows = self.current_scope_shadows(name);
        for s in shadows.into_iter().rev() {
            let t = self.ast.strip_shadow(s);
            if self.ast.first(t) == self.ast.first(d) || !self.using_conflicts(t, d) {
                continue;
            }
            if self.member_spec_class().is_some()
                && self.kind_of(d).is_function() | (self.kind_of(d) == DeclKind::FunctionTemplate)
            {
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
        self.mark_type_referenced(td);
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
            return Some(ud);
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
                let _ = self.introduce_shadow(n, name_loc, c);
            }
        }
        Some(ud)
    }

    // -- namespace aliases -------------------------------------------------------------

    /// `namespace name = qual target;` ([namespace.alias]).
    pub fn act_on_namespace_alias(
        &mut self,
        name: Symbol,
        name_loc: Loc,
        qual: Option<Qualifier>,
        target: Symbol,
        target_loc: Loc,
    ) -> Option<DeclId> {
        let found = match qual {
            Some(q) => self.lookup_qualified(q.ctx, target, LookupKind::Nested),
            None => self.lookup_name(target, LookupKind::Nested),
        };
        let Some(t) =
            found.decls.iter().copied().find(|&d| {
                matches!(self.kind_of(d), DeclKind::Namespace | DeclKind::NamespaceAlias)
            })
        else {
            self.error(DiagId::parse_expected_namespace_name, target_loc);
            return None;
        };
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
