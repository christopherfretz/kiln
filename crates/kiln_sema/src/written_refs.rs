//! Recording into the written-reference table (spec/ast.md §18,
//! WP-1930). The parser calls these at the sites where it consumed a
//! name; sema itself only patches overloaded names after overload
//! resolution and records lambda captures. Every entry point is one
//! branch on the table's flag when recording is off.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::types::{ArgView, Nns, TemplateName, TypeView};
use kiln_ast::written_refs::{
    AdlSet, PatternNode, QualLink, REF_DEPENDENT, REF_MEMBER, REF_UNRESOLVED, RefLookup, RefMark,
    RefQualifier, RefRole, WrittenRef, WrittenRefId,
};
use kiln_ast::{DeclId, ExprId, NnsId, QualType};
use kiln_source::Span;

use crate::Sema;

/// What a built name expression resolved to.
#[derive(Clone, Copy)]
pub(crate) enum Target {
    /// The entity, and what lookup found when it differs.
    Decl(DeclId, Option<DeclId>),
    /// An overload set, resolved later.
    Overloaded(ExprId),
    /// A concept-id's concept.
    Concept(DeclId),
    /// A pseudo-destructor's destroyed scalar type (its type-name).
    Type(QualType),
    /// A dependent name (its node).
    Dependent(ExprId),
    /// Nothing (an error).
    Unresolved,
}

impl Sema<'_, '_> {
    /// True when names are recorded (one branch; spec/ast.md §18.1).
    #[inline]
    pub fn records_written_refs(&self) -> bool {
        self.ast.written_refs.is_on()
    }

    /// The explicit instantiation whose declarator-id is at `loc` names
    /// `target` (a function template, or a member of a class template
    /// specialization): the parser records the name with it after the
    /// declaration (stage 3, spec/ast.md §18.4).
    pub(crate) fn note_written_inst_target(&mut self, loc: kiln_source::Loc, target: DeclId) {
        if self.records_written_refs() {
            self.ast.written_refs.set_inst_target(loc, target);
        }
    }

    /// Records the expression or member name written at `range`, which
    /// built `e` (`None`: nothing was built), with qualifier `q`; `member`
    /// when it was written after `.` or `->`.
    pub fn note_written_name(
        &mut self,
        e: Option<ExprId>,
        range: Span,
        role: RefRole,
        q: RefQualifier,
        member: bool,
    ) -> Option<WrittenRefId> {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return None;
        }
        let target = e.map_or(Target::Unresolved, |e| self.written_target(e));
        let role = match target {
            Target::Concept(_) => RefRole::Concept,
            Target::Type(_) => RefRole::Type,
            _ => role,
        };
        let (entity, found, flags, pending) = match target {
            // `p->~T()` of a scalar type names the type `T` (a typedef or
            // template parameter), not a destructor (spec/ast.md §18.4).
            Target::Type(t) => match self.written_type_entity(t, 0) {
                Some((d, f)) => (Some(d), f.filter(|&f| f != d), 0, None),
                None => (None, None, REF_DEPENDENT, None),
            },
            Target::Concept(c) => (Some(c), None, 0, None),
            Target::Decl(d, f) => (Some(d), f.filter(|&f| f != d), 0, None),
            Target::Overloaded(x) => {
                let flags = if self.in_template() { REF_DEPENDENT } else { REF_UNRESOLVED };
                (None, None, flags, Some(x))
            }
            Target::Dependent(_) => (None, None, REF_DEPENDENT, None),
            Target::Unresolved => (None, None, REF_UNRESOLVED, None),
        };
        // The pattern node a dependent name is rebuilt from, for its
        // per-specialization entries (stage 4, spec/ast.md §18.9.4).
        let node = match target {
            Target::Dependent(x) => Some(x),
            Target::Overloaded(x) if flags & REF_DEPENDENT != 0 => Some(x),
            _ => None,
        };
        // A member (or member template) name after `.`/`->` is found by
        // class member lookup (spec/ast.md §18.7.2); a pseudo-destructor's
        // type name is not.
        let flags = if member && matches!(role, RefRole::Member | RefRole::Template) {
            flags | REF_MEMBER
        } else {
            flags
        };
        let id = self.push_written_ref(range, entity, found, role, q, flags)?;
        if let Some(x) = pending {
            self.ast.written_refs.defer(id, x);
        }
        if let Some(x) = node {
            self.ast.written_refs.note_pattern_node(PatternNode::Expr(x), id);
        }
        Some(id)
    }

    /// Records the nested-name-specifier component written at `range`
    /// that produced `n` (a template-id component names template
    /// `tmpl`), with qualifier `q`; returns the link for the next
    /// component (`q` unchanged when off).
    pub fn note_written_nns(
        &mut self,
        range: Span,
        n: NnsId,
        tmpl: Option<DeclId>,
        q: RefQualifier,
    ) -> RefQualifier {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return q;
        }
        let (entity, found, flags) = match self.ast.types.nns_view(n) {
            Nns::Namespace { ns, .. } => (Some(ns), None, 0),
            Nns::NamespaceAlias { alias, .. } => (Some(alias), None, 0),
            Nns::Type { ty, .. } => match tmpl {
                Some(t) => (Some(t), None, 0),
                None => match self.written_type_entity(ty, 0) {
                    Some((d, f)) => (Some(d), f, 0),
                    None if !self.ast.types.is_error(ty) && self.ast.types.is_dependent(ty) => {
                        (None, None, REF_DEPENDENT)
                    }
                    None => (None, None, REF_UNRESOLVED),
                },
            },
            Nns::Identifier { .. } | Nns::Splice(_) => (None, None, REF_DEPENDENT),
            Nns::Global => (None, None, REF_UNRESOLVED),
        };
        match self.push_written_ref(range, entity, found, RefRole::Nns, q, flags) {
            Some(id) => RefQualifier::entry(id),
            None => q,
        }
    }

    /// Records the type or template name written at `range` (stage 2,
    /// spec/ast.md §18.4) with qualifier `q`: `ty` is the type the
    /// specifier built (its sugar gives the entity: a typedef, the
    /// using-shadow it was found through, the class, the template of a
    /// specialization), `tmpl` the template a template name denotes when
    /// the caller knows it. No entity: dependent when `ty` is, else
    /// unresolved.
    pub fn note_written_type(
        &mut self,
        range: Span,
        ty: Option<QualType>,
        tmpl: Option<DeclId>,
        role: RefRole,
        q: RefQualifier,
    ) -> Option<WrittenRefId> {
        self.note_written_type_found(range, ty, tmpl, role, q, None)
    }

    /// [`Sema::note_written_type`] for a name spelled `name` (`lookup`:
    /// the name and its qualifier's scope), whose `found` is the
    /// using-shadow lookup finds for it ([`Sema::written_template_shadow`]).
    pub fn note_written_type_found(
        &mut self,
        range: Span,
        ty: Option<QualType>,
        tmpl: Option<DeclId>,
        role: RefRole,
        q: RefQualifier,
        lookup: Option<(kiln_source::symbol::Symbol, Option<DeclId>)>,
    ) -> Option<WrittenRefId> {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return None;
        }
        let target = match tmpl {
            Some(t) => Some((t, None)),
            None => ty.and_then(|t| self.written_type_entity(t, 0)),
        };
        let target = match (target, lookup) {
            (Some((d, None)), Some((name, qual))) => {
                Some((d, self.written_template_shadow(name, qual, d)))
            }
            (t, _) => t,
        };
        let (entity, found, flags) = match target {
            Some((d, f)) => (Some(d), f.filter(|&f| f != d), 0),
            None if ty
                .is_some_and(|t| !self.ast.types.is_error(t) && self.ast.types.is_dependent(t)) =>
            {
                (None, None, REF_DEPENDENT)
            }
            None => (None, None, REF_UNRESOLVED),
        };
        let id = self.push_written_ref(range, entity, found, role, q, flags)?;
        // A dependent typename-specifier or template-id is rebuilt from
        // its interned type, found again by where its name is written
        // (stage 4, spec/ast.md §18.9.4).
        if flags == REF_DEPENDENT
            && let Some(t) = ty
        {
            self.ast
                .written_refs
                .note_pattern_node(PatternNode::Type(t.without_quals(), range.lo), id);
        }
        Some(id)
    }

    /// The mem-initializer-id recorded as `w` resolved as `id`
    /// (spec/ast.md §18.4): patches its entity to the non-static data
    /// member, or the class named (a base, or the constructor's own
    /// class for a delegating constructor).
    pub fn resolve_written_mem_init(&mut self, w: WrittenRefId, id: &crate::class::MemInitId) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let (entity, found, flags) = match *id {
            crate::class::MemInitId::Type(ty) => match self.written_type_entity(ty, 0) {
                Some((d, f)) => (Some(d), f.filter(|&f| f != d), 0),
                None if !self.ast.types.is_error(ty) && self.ast.types.is_dependent(ty) => {
                    (None, None, REF_DEPENDENT)
                }
                None => (None, None, REF_UNRESOLVED),
            },
            crate::class::MemInitId::Ident(name) => {
                // Looked up from the constructor's scope: its class's
                // members first ([class.base.init]/2).
                let f = self.lookup_name(name, crate::lookup::LookupKind::Ordinary);
                match f.decls.first().copied().zip(f.found.first().copied()) {
                    Some((d, f)) => {
                        // A class (a base's injected-class-name) names
                        // the class.
                        let d = if crate::lookup::is_tag_kind(self.kind_of(d)) {
                            self.written_type_entity(self.tag_type(d), 0).map_or(d, |x| x.0)
                        } else {
                            d
                        };
                        let shadow = self.kind_of(f) == DeclKind::UsingShadow;
                        (Some(d), Some(f).filter(|&f| f != d && shadow), 0)
                    }
                    None if self.in_template() => (None, None, REF_DEPENDENT),
                    None => (None, None, REF_UNRESOLVED),
                }
            }
        };
        self.ast.written_refs.set_target(w, entity, found, flags);
        self.fill_written_provenance(w);
    }

    /// The declaration lookup found for the type or template name `name`
    /// (unqualified, or in the scope `qual`) when it is a using-shadow of
    /// `entity` (`using std::vector; vector<int>`): the `found` of a
    /// name whose type keeps no shadow.
    pub fn written_template_shadow(
        &self,
        name: kiln_source::symbol::Symbol,
        qual: Option<DeclId>,
        entity: DeclId,
    ) -> Option<DeclId> {
        let f = match qual {
            Some(q) => self.lookup_qualified(q, name, crate::lookup::LookupKind::Ordinary),
            None => self.lookup_name(name, crate::lookup::LookupKind::Ordinary),
        };
        f.decls
            .iter()
            .zip(&f.found)
            .find(|&(&d, &s)| d != s && self.ast.first(d) == self.ast.first(entity))
            .map(|(_, &s)| s)
    }

    /// Records the nested-name-specifier component written at `range`
    /// that names `d` (a namespace or type the lookahead classified; no
    /// `NnsId` was built for it), after qualifier `q`; returns the link
    /// for the next component.
    pub fn note_written_nns_decl(
        &mut self,
        range: Span,
        d: Option<DeclId>,
        q: RefQualifier,
    ) -> RefQualifier {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return q;
        }
        let flags = if d.is_some() { 0 } else { REF_UNRESOLVED };
        match self.push_written_ref(range, d, None, RefRole::Nns, q, flags) {
            Some(id) => RefQualifier::entry(id),
            None => q,
        }
    }

    /// Records the template template argument written at `range` that
    /// parsed as `arg` (spec/ast.md §18.4): a template name (`X<vector>`),
    /// or a dependent member named without `template`, an expression.
    /// `lookup` is the name and its qualifier's scope, for `found`.
    pub fn note_written_template_arg(
        &mut self,
        range: Span,
        arg: kiln_ast::types::TemplateArg,
        q: RefQualifier,
        lookup: (kiln_source::symbol::Symbol, Option<DeclId>),
    ) -> Option<WrittenRefId> {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return None;
        }
        let (entity, role) = match arg.view() {
            ArgView::Template(tn) | ArgView::TemplateExpansion { name: tn, .. } => {
                (self.written_template_decl(tn).map(|x| x.0), RefRole::Template)
            }
            ArgView::Expr { .. } => (None, RefRole::Expr),
            _ => return None,
        };
        let (found, flags) = match entity {
            Some(d) => (self.written_template_shadow(lookup.0, lookup.1, d), 0),
            None => (None, REF_DEPENDENT),
        };
        self.push_written_ref(range, entity, found, role, q, flags)
    }

    /// Records the pack named by `sizeof...(name)` at `range` (role
    /// `expr`, spec/ast.md §18.4): the pack lookup finds
    /// (`act_on_sizeof_pack_name`'s), `UNRESOLVED` when none.
    pub fn note_written_pack_name(&mut self, range: Span, name: kiln_source::symbol::Symbol) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let found = self.lookup_name(name, crate::lookup::LookupKind::Ordinary);
        let pack = found.decls.first().copied().filter(|&d| self.is_pack_decl(d));
        self.note_written_decl_name(range, pack, RefRole::Expr, RefQualifier::NONE, false);
    }

    /// Records a declarative name (stage 3, spec/ast.md §18.4) written at
    /// `range` with qualifier `q`, naming `entity`: `None` is a dependent
    /// name when `dependent`, else an unresolved one.
    pub fn note_written_decl_name(
        &mut self,
        range: Span,
        entity: Option<DeclId>,
        role: RefRole,
        q: RefQualifier,
        dependent: bool,
    ) -> Option<WrittenRefId> {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return None;
        }
        let flags = match entity {
            Some(_) => 0,
            None if dependent => REF_DEPENDENT,
            None => REF_UNRESOLVED,
        };
        self.push_written_ref(range, entity, None, role, q, flags)
    }

    /// Records the name of the using-declarator at `range` that declared
    /// `d` (spec/ast.md §18.4): one entry whose entity is the first
    /// declaration it introduced (an inherited constructor for `using
    /// B::B;`), or, when it introduced none (all were introduced before),
    /// the first that qualified lookup of `name` in `qual` finds; a
    /// dependent using-declarator has none. `found` stays empty: the
    /// using-declaration is the declaration.
    pub fn note_written_using_declarator(
        &mut self,
        range: Span,
        d: Option<DeclId>,
        qual: Option<DeclId>,
        name: kiln_ast::names::DeclName,
        q: RefQualifier,
    ) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let (entity, dependent) = match d.map(|d| (d, self.kind_of(d))) {
            Some((_, DeclKind::UnresolvedUsingValue | DeclKind::UnresolvedUsingTypename)) => {
                (None, true)
            }
            Some((d, _)) => {
                let first = self.ast.using_shadows(d).next().map(|s| self.ast.strip_shadow(s));
                let e = first.or_else(|| {
                    let f = self.lookup_qualified_dn(qual?, name, crate::lookup::LookupKind::All);
                    f.decls.first().map(|&x| self.ast.strip_shadow(x))
                });
                (e, false)
            }
            None => (None, qual.is_some_and(|c| self.is_dependent_ctx(c))),
        };
        self.note_written_decl_name(range, entity, RefRole::UsingDecl, q, dependent);
    }

    /// Records the field designator `.name` whose name is at `range`
    /// (spec/ast.md §18.4): unresolved (dependent in a template) until the
    /// initialization finds the member
    /// ([`resolve_written_designator`](Self::resolve_written_designator)).
    pub fn note_written_designator(&mut self, range: Span) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let flags = REF_MEMBER | if self.in_template() { REF_DEPENDENT } else { REF_UNRESOLVED };
        if let Some(id) =
            self.push_written_ref(range, None, None, RefRole::Designator, RefQualifier::NONE, flags)
        {
            self.ast.written_refs.defer_designator(range.lo, id);
        }
    }

    /// Records the name at `loc` of an offsetof member-designator
    /// component ([support.types.layout]/1, role `designator`, spec/ast.md
    /// §18.4): the member `field` lookup found (the member written, not
    /// the anonymous members on its way), dependent when the designator
    /// waits for instantiation, else unresolved when lookup found none.
    pub(crate) fn note_written_offsetof_member(
        &mut self,
        loc: kiln_source::Loc,
        field: Option<DeclId>,
        dependent: bool,
    ) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let hi = self.token_end(loc).unwrap_or(loc);
        let flags = REF_MEMBER
            | match field {
                Some(_) => 0,
                None if dependent => REF_DEPENDENT,
                None => REF_UNRESOLVED,
            };
        let range = Span::new(loc, hi);
        let _ = self.push_written_ref(
            range,
            field,
            None,
            RefRole::Designator,
            RefQualifier::NONE,
            flags,
        );
    }

    /// The field designator whose name is at `loc` designates `field` (a
    /// member of an anonymous member is designated after the anonymous
    /// member itself, so the last call names the member written).
    #[inline]
    pub(crate) fn resolve_written_designator(&mut self, loc: kiln_source::Loc, field: DeclId) {
        if self.inst.stack.is_empty()
            && let Some(id) = self.ast.written_refs.designator_at(loc)
        {
            self.ast.written_refs.set_target(id, Some(field), None, REF_MEMBER);
            self.fill_written_provenance(id);
        }
    }

    /// The end of the token starting at `at` (for a name whose token
    /// the parser no longer holds).
    pub fn token_end_of(&self, at: kiln_source::Loc) -> Option<kiln_source::Loc> {
        self.token_end(at)
    }

    /// Records a lambda capture's name `[x]` at `loc` naming `var`.
    pub(crate) fn note_written_capture(&mut self, loc: kiln_source::Loc, var: DeclId) {
        if !self.records_written_refs() || !self.inst.stack.is_empty() {
            return;
        }
        let hi = self.token_end(loc).unwrap_or(loc);
        self.push_written_ref(
            Span::new(loc, hi),
            Some(var),
            None,
            RefRole::Expr,
            RefQualifier::NONE,
            0,
        );
    }

    /// Overload resolution chose `d` (found as `found`) for the overload
    /// set `x`: patches the entry naming `x` (spec/ast.md §18.4).
    #[inline]
    pub(crate) fn resolve_written_ref(&mut self, x: ExprId, d: DeclId, found: Option<DeclId>) {
        if self.ast.written_refs.has_pending()
            && self.inst.stack.is_empty()
            && let Some(id) = self.ast.written_refs.resolve(x, d, found)
        {
            self.fill_written_provenance(id);
        } else if self.ast.written_refs.has_spec_pending() {
            self.resolve_written_spec(x, Some(d), found);
        }
    }

    /// The entry of the overloaded name `x` while it awaits resolution
    /// (one branch when nothing is pending).
    #[inline]
    pub(crate) fn pending_written_ref(&self, x: ExprId) -> Option<WrittenRefId> {
        if self.ast.written_refs.has_pending() && self.inst.stack.is_empty() {
            self.ast.written_refs.pending_for(x)
        } else {
            None
        }
    }

    /// The pattern entry and specialization of the rebuilt overload set
    /// `x` while it awaits resolution in an instantiation (stage 4; one
    /// branch when nothing is pending).
    #[inline]
    pub(crate) fn pending_written_spec(&self, x: ExprId) -> Option<(WrittenRefId, DeclId)> {
        if self.ast.written_refs.has_spec_pending() {
            self.ast.written_refs.spec_pending_for(x)
        } else {
            None
        }
    }

    /// The call whose callee's entry is `w` was resolved, after
    /// argument-dependent lookup saw `facts` (`None`: it was not
    /// performed): `how` becomes `Adl` or `Both` when it found the winner,
    /// and the entry keeps the associated set (spec/ast.md §18.7.4).
    pub(crate) fn note_written_call_lookup(
        &mut self,
        w: WrittenRefId,
        facts: Option<crate::overload::adl::AdlFacts>,
    ) {
        let Some(f) = facts else { return };
        let r = *self.ast.written_refs.get(w);
        let Some((how, set)) = self.written_call_lookup(r.entity, r.found, &f) else { return };
        if let Some(how) = how {
            self.ast.written_refs.set_how(w, how);
            if how == RefLookup::Adl {
                self.ast.written_refs.set_using_directive(w, None);
            }
        }
        self.ast.written_refs.set_adl(w, set);
    }

    /// What argument-dependent lookup's `f` says about a call that
    /// selected `entity` (found as `found`): the refined `how` (`Adl`,
    /// `Both`, or none to keep) and the associated set; `None` when a
    /// class member was selected (spec/ast.md §18.7.4).
    pub(crate) fn written_call_lookup(
        &self,
        entity: Option<DeclId>,
        found: Option<DeclId>,
        f: &crate::overload::adl::AdlFacts,
    ) -> Option<(Option<RefLookup>, AdlSet)> {
        // A class member was selected (a template-id that named member
        // function templates, called on the implicit object): ordinary
        // lookup found a class member, which suppresses argument-dependent
        // lookup ([basic.lookup.argdep]/1), so no set is kept.
        if let Some(e) = entity
            && self
                .ast
                .decls
                .get(e)
                .ctx()
                .is_some_and(|c| crate::overload::adl::is_class_kind(self.kind_of(c)))
        {
            return None;
        }
        let how = entity.and_then(|e| {
            let key = self.ast.first(self.ast.strip_shadow(found.unwrap_or(e)));
            if f.only.contains(&key) {
                Some(RefLookup::Adl)
            } else if f.also.contains(&key) {
                Some(RefLookup::Both)
            } else {
                None
            }
        });
        let a = &f.assoc;
        Some((how, AdlSet::new(&a.namespaces, &a.classes, &a.enums)))
    }

    /// Sets `how` and the using-directive of entry `id`, which just got
    /// its entity (spec/ast.md §18.7.2, §18.7.3).
    fn fill_written_provenance(&mut self, id: WrittenRefId) {
        let r = *self.ast.written_refs.get(id);
        let mut how = r.default_how();
        let mut udir = None;
        if let Some(e) = r.entity
            && matches!(how, RefLookup::Unqualified | RefLookup::Qualified)
        {
            if self.ast.decls.builtin_id(e).is_some() {
                how = RefLookup::Builtin;
            } else if let Some((injected, u)) = self.written_lookup_provenance(&r) {
                if injected {
                    how = RefLookup::Injected;
                }
                udir = u;
            }
        }
        if how != r.how {
            self.ast.written_refs.set_how(id, how);
        }
        if udir.is_some() || self.ast.written_refs.using_directive(id).is_some() {
            self.ast.written_refs.set_using_directive(id, udir);
        }
    }

    /// Whether the name of entry `r` was found as an injected-class-name,
    /// and the using-directive it was found through: the lookup re-run in
    /// its provenance form over the current scopes, believed only when it
    /// finds `r`'s entity (spec/ast.md §18.7.1). `None` when neither can
    /// apply or the lookup disagrees.
    fn written_lookup_provenance(&mut self, r: &WrittenRef) -> Option<(bool, Option<DeclId>)> {
        let entity = r.entity?;
        let target = r.found.unwrap_or(entity);
        let key = self.ast.first(self.ast.strip_shadow(target));
        let class = self.written_class_of(key);
        let may_inject = class.is_some()
            && matches!(
                r.role,
                RefRole::Type | RefRole::Template | RefRole::Nns | RefRole::Base | RefRole::MemInit
            );
        let may_udir = self.any_using_directive() && self.in_namespace_scope(target);
        if !may_inject && !may_udir {
            return None;
        }
        let name = self.ast.decls.get(target).name();
        if name == kiln_ast::names::DeclName::EMPTY {
            return None;
        }
        let kind = match r.role {
            RefRole::Nns => crate::lookup::LookupKind::Nested,
            RefRole::Type | RefRole::Base => crate::lookup::LookupKind::Type,
            RefRole::UsingDirective | RefRole::NamespaceAlias => {
                crate::lookup::LookupKind::Namespace
            }
            RefRole::UsingDecl => crate::lookup::LookupKind::All,
            RefRole::Elaborated if crate::lookup::is_tag_kind(self.kind_of(key)) => {
                crate::lookup::LookupKind::Tag
            }
            _ => crate::lookup::LookupKind::Ordinary,
        };
        let (found, p) = match r.qualifier.link() {
            QualLink::None => self.lookup_name_provenance(name, kind),
            QualLink::Global => self.lookup_qualified_walk(self.ast.tu, name, kind, true),
            QualLink::Entry(q) => {
                let ctx = self.written_qualifier_scope(self.ast.written_refs.get(q).entity?)?;
                self.lookup_qualified_walk(ctx, name, kind, true)
            }
        };
        // A class template's injected-class-name is found as its pattern
        // ([temp.local]/1).
        let class_key = class.map(|c| self.ast.first(c));
        if !found.decls.iter().any(|&d| {
            let f = self.ast.first(d);
            f == key || (p.in_class && Some(f) == class_key)
        }) {
            return None;
        }
        let injected = may_inject
            && p.in_class
            && match (class, p.scope) {
                (Some(c), Some(s)) => {
                    let c = self.ast.first(c);
                    self.ast.first(s) == c
                        || crate::conv::bases::is_derived_from(self, self.ast.primary_context(s), c)
                }
                _ => false,
            };
        if p.in_class || p.udirs.is_empty() {
            return Some((injected, None));
        }
        let key_of = |s: &Self, d: DeclId| s.ast.first(s.ast.strip_shadow(d));
        if let Some(ns) = p.scope
            && self.ast.lookup_local(ns, name).iter_ordinary().any(|d| key_of(self, d) == key)
        {
            // The scope's own declaration: no directive (§18.7.3).
            return Some((injected, None));
        }
        let hit = p.udirs.iter().find(|&&(d, _, _)| key_of(self, d) == key).copied();
        let udir = match hit {
            Some((_, _, crate::lookup::UdirVia::Directive(d))) => Some(d),
            Some((_, n, crate::lookup::UdirVia::Namespace(h))) => self.find_using_directive(h, n),
            None => None,
        };
        Some((injected, udir))
    }

    /// The class a class, class template or specialization `d` names.
    fn written_class_of(&self, d: DeclId) -> Option<DeclId> {
        match self.kind_of(d) {
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization => Some(d),
            DeclKind::ClassTemplate => self.pattern_of(d),
            _ => None,
        }
    }

    /// True when some using-directive exists that a lookup could use.
    fn any_using_directive(&self) -> bool {
        !self.ast.lookup.has_no_using_directives()
            || self.scopes.iter().any(|s| s.is_local() && !s.using_dirs.is_empty())
    }

    /// True when `d` is declared in a namespace scope (through linkage
    /// specifications, `export` and unscoped enumerations), where a
    /// using-directive can make it visible.
    fn in_namespace_scope(&self, d: DeclId) -> bool {
        let mut cur = self.ast.decls.get(d).ctx();
        for _ in 0..64 {
            let Some(c) = cur else { return false };
            if self.ast.is_transparent(c) {
                cur = self.ast.decls.get(c).ctx();
                continue;
            }
            return matches!(self.kind_of(c), DeclKind::Namespace | DeclKind::TranslationUnit);
        }
        false
    }

    /// The scope a qualifier component naming `q` designates: a namespace
    /// (an alias's target), or a class's definition.
    fn written_qualifier_scope(&self, mut q: DeclId) -> Option<DeclId> {
        for _ in 0..16 {
            match self.kind_of(q) {
                DeclKind::Namespace => return Some(q),
                DeclKind::NamespaceAlias => q = self.ast.using_target(q)?,
                DeclKind::CXXRecord
                | DeclKind::ClassTemplateSpecialization
                | DeclKind::ClassTemplatePartialSpecialization => {
                    return Some(self.ast.primary_context(q));
                }
                DeclKind::Typedef | DeclKind::TypeAlias => {
                    let t = self.ast.decls.get(q).ty()?;
                    let c = self.ast.types.as_record(t)?;
                    return Some(self.ast.primary_context(c));
                }
                _ => return None,
            }
        }
        None
    }

    /// The using-directive in namespace `holder` (its reopenings, linkage
    /// specifications and inline namespaces) that nominates `nominated`,
    /// earliest first; cached (spec/ast.md §18.7.3).
    fn find_using_directive(&mut self, holder: DeclId, nominated: DeclId) -> Option<DeclId> {
        if let Some(c) = self.ast.written_refs.cached_directive(holder, nominated) {
            return c;
        }
        let d = self.scan_using_directive(holder, self.ast.first(nominated), 0);
        self.ast.written_refs.cache_directive(holder, nominated, d);
        d
    }

    fn scan_using_directive(&self, ns: DeclId, nominated: DeclId, depth: u32) -> Option<DeclId> {
        if depth > 32 {
            return None;
        }
        let mut opened: Vec<DeclId> = self.ast.redecls_of(ns).collect();
        opened.reverse();
        for r in opened {
            if let Some(d) = self.scan_members_for_directive(r, nominated, 0) {
                return Some(d);
            }
        }
        let inline: Vec<DeclId> = self.ast.lookup.inline_children(self.ast.first(ns)).to_vec();
        inline.into_iter().find_map(|c| self.scan_using_directive(c, nominated, depth + 1))
    }

    fn scan_members_for_directive(
        &self,
        ctx: DeclId,
        nominated: DeclId,
        depth: u32,
    ) -> Option<DeclId> {
        if depth > 32 {
            return None;
        }
        for m in self.ast.decls.members(ctx) {
            match self.kind_of(m) {
                DeclKind::UsingDirective
                    if self.ast.using_target(m).map(|n| self.ast.first(n)) == Some(nominated) =>
                {
                    return Some(m);
                }
                DeclKind::LinkageSpec | DeclKind::Export => {
                    if let Some(d) = self.scan_members_for_directive(m, nominated, depth + 1) {
                        return Some(d);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The overload set `from` goes on as the set `to`.
    #[inline]
    pub(crate) fn retarget_written_ref(&mut self, from: ExprId, to: ExprId) {
        if self.ast.written_refs.has_pending() && self.inst.stack.is_empty() {
            self.ast.written_refs.retarget(from, to);
        } else if self.ast.written_refs.has_spec_pending() {
            self.ast.written_refs.retarget_spec(from, to);
        }
    }

    /// The table position and declaration count, for
    /// [`claim_written_refs`](Self::claim_written_refs). Taken whether or
    /// not names are recorded: the same claims decide closure owners
    /// (spec/ast.md §5.9).
    #[inline]
    pub fn written_ref_mark(&self) -> Option<RefMark> {
        Some(self.ast.written_ref_mark())
    }

    /// Declaration `d` claims the names recorded since `m` (spec/ast.md
    /// §18.3) and the closures created since (§5.9).
    #[inline]
    pub fn claim_written_refs(&mut self, m: Option<RefMark>, d: Option<DeclId>) {
        if let (Some(m), Some(d)) = (m, d) {
            if self.records_written_refs() {
                self.ast.written_refs.claim(m, d);
            }
            self.claim_closures(m.decls, d);
        }
    }

    /// [`claim_written_refs`](Self::claim_written_refs) by the template
    /// parameter just parsed since `m` (closures only: template parameters
    /// do not claim names yet, spec/ast.md §18.3).
    #[inline]
    pub fn claim_template_param(&mut self, m: Option<RefMark>) {
        if let Some(m) = m {
            self.claim_template_param_closures(m.decls);
        }
    }

    /// The alias-declaration `d`, whose type-id was parsed since `m`,
    /// claims its closures (spec/ast.md §5.9; its names stay enclosed by
    /// the context, §18.3).
    #[inline]
    pub fn claim_alias_closures(&mut self, m: Option<RefMark>, d: DeclId) {
        if let Some(m) = m {
            self.claim_closures(m.decls, d);
        }
    }

    /// The closures of the innermost template head opened since `m`
    /// (its requires-clause) go to the templated declaration it declared.
    #[inline]
    pub fn claim_template_head(&mut self, m: Option<RefMark>) {
        if let Some(m) = m {
            self.claim_template_head_closures(m.decls);
        }
    }

    fn push_written_ref(
        &mut self,
        range: Span,
        entity: Option<DeclId>,
        found: Option<DeclId>,
        role: RefRole,
        qualifier: RefQualifier,
        flags: u8,
    ) -> Option<WrittenRefId> {
        // A requires-expression's parameter scope is not a declaration
        // a reader names: its names belong to the context around it.
        let mut ctx = self.ctx;
        while self.kind_of(ctx) == DeclKind::RequiresExprBody
            && let Some(c) = self.ast.decls.get(ctx).ctx()
        {
            ctx = c;
        }
        let enclosing = Some(ctx);
        let mut r = WrittenRef {
            range,
            entity,
            found,
            enclosing,
            qualifier,
            role,
            flags,
            how: RefLookup::None,
        };
        r.how = r.default_how();
        let id = self.ast.written_refs.push(r)?;
        if entity.is_some() {
            self.fill_written_provenance(id);
        }
        Some(id)
    }

    /// What the name expression `e` (implicit conversions peeled)
    /// resolved to.
    pub(crate) fn written_target(&self, mut e: ExprId) -> Target {
        let exprs = &self.ast.exprs;
        loop {
            let n = exprs.get(e);
            match n.kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr | ExprKind::ParenExpr => {
                    e = exprs.operand(e);
                }
                // An overloaded member access is a `MemberExpr` of the
                // overload type with its set on the side (`member_overload`).
                ExprKind::MemberExpr
                    if n.ty() == QualType::OVERLOAD && self.ovl.members.contains_key(&e) =>
                {
                    return Target::Overloaded(e);
                }
                ExprKind::DeclRefExpr | ExprKind::MemberExpr => {
                    let found = exprs.decl_ref_extra(e).and_then(|x| x.found_decl);
                    return Target::Decl(exprs.decl(e), found);
                }
                ExprKind::UnresolvedLookupExpr | ExprKind::UnresolvedMemberExpr => {
                    return Target::Overloaded(e);
                }
                ExprKind::CXXPseudoDestructorExpr => {
                    return Target::Type(exprs.pseudo_destructor(e).2);
                }
                ExprKind::ConceptSpecializationExpr => {
                    return DeclId::try_new(n.a()).map_or(Target::Unresolved, Target::Concept);
                }
                ExprKind::DependentScopeDeclRefExpr | ExprKind::CXXDependentScopeMemberExpr => {
                    return Target::Dependent(e);
                }
                _ if n.is_type_dependent() || n.is_value_dependent() => {
                    return Target::Dependent(e);
                }
                _ => return Target::Unresolved,
            }
        }
    }

    /// True when `ty` is a placeholder for a deduced class type (a class
    /// template named for argument deduction, [dcl.type.class.deduct]).
    pub fn written_is_ctad(&self, ty: QualType) -> bool {
        let mut ty = ty;
        for _ in 0..8 {
            match self.ast.types.view(ty.ty()) {
                TypeView::Elaborated { named, .. } => ty = named,
                TypeView::DeducedTemplateSpecialization { .. } => return true,
                _ => return false,
            }
        }
        false
    }

    /// The declaration a type named in a nested-name-specifier denotes
    /// (and the using-shadow it was found through).
    pub(crate) fn written_type_entity(
        &self,
        ty: QualType,
        depth: u32,
    ) -> Option<(DeclId, Option<DeclId>)> {
        if depth > 8 {
            return None;
        }
        let types = &self.ast.types;
        match types.view(ty.ty()) {
            TypeView::Typedef { decl: d, .. }
            | TypeView::TemplateTypeParm(d)
            | TypeView::InjectedClassName { pattern: d, .. }
            | TypeView::Record(d)
            | TypeView::Enum(d) => Some((d, None)),
            TypeView::Using { shadow, underlying } => {
                self.written_type_entity(underlying, depth + 1).map(|(d, _)| (d, Some(shadow)))
            }
            TypeView::Elaborated { named, .. } => self.written_type_entity(named, depth + 1),
            TypeView::TemplateSpecialization { name, .. }
            | TypeView::DeducedTemplateSpecialization { name, .. } => {
                self.written_template_decl(name)
            }
            // `Ts...[i]` names the pack `Ts`.
            TypeView::PackIndexing { pattern, .. } => self.written_type_entity(pattern, depth + 1),
            _ => types.as_record(ty).or_else(|| types.as_enum(ty)).map(|d| (d, None)),
        }
    }

    fn written_template_decl(
        &self,
        mut n: kiln_ast::TemplateNameId,
    ) -> Option<(DeclId, Option<DeclId>)> {
        for _ in 0..8 {
            match self.ast.types.template_name_view(n) {
                TemplateName::Template(d) | TemplateName::TemplateTemplateParm { decl: d, .. } => {
                    return Some((d, None));
                }
                TemplateName::Qualified { underlying, .. } => n = underlying,
                _ => return None,
            }
        }
        None
    }
}
