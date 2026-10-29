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
            self.class_using_declarator(class, q, name, name_loc);
            return None;
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

    /// A using-declarator in a class ([namespace.udecl]/3): it must name
    /// a member of a base class. kiln has no base classes yet, so every
    /// such declarator is ill-formed; this reports clang's diagnostic for
    /// the qualifier.
    fn class_using_declarator(&mut self, class: DeclId, q: Qualifier, name: Symbol, name_loc: Loc) {
        let qk = self.kind_of(q.ctx);
        if matches!(qk, DeclKind::Namespace | DeclKind::TranslationUnit) {
            let msg = self
                .diag(DiagId::sema_using_decl_nested_name_specifier_is_not_class, q.loc)
                .arg(self.nns_str(q.nns));
            self.emit(msg);
            return;
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
            return;
        }
        let msg = self
            .diag(DiagId::sema_using_decl_not_base_class, q.loc)
            .arg(self.nns_str(q.nns))
            .arg(self.decl_name(class));
        self.emit(msg);
    }

    /// Introduces a `UsingShadow` of `target` named `name` into the
    /// current scope, unless it conflicts with a declaration already
    /// there ([namespace.udecl]/11, [basic.scope.scope]/4). A repeated
    /// using-declaration of the same entity replaces the earlier shadow.
    fn introduce_shadow(&mut self, name: Symbol, loc: Loc, target: DeclId) -> Option<DeclId> {
        let mut prev = None;
        for e in self.current_scope_raw(name) {
            let es = self.ast.strip_shadow(e);
            if self.ast.first(es) == self.ast.first(target) {
                if e != es {
                    prev = Some(e);
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
        let h = DeclHead::new(self.ctx, DeclName::ident(name), loc);
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
        if ka == DeclKind::Function && kb == DeclKind::Function {
            return self.same_ignoring_exc_spec(ta, tb);
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
