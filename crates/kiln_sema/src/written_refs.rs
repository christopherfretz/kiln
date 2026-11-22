//! Recording into the written-reference table (spec/ast.md §18,
//! WP-1930). The parser calls these at the sites where it consumed a
//! name; sema itself only patches overloaded names after overload
//! resolution and records lambda captures. Every entry point is one
//! branch on the table's flag when recording is off.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::types::{ArgView, Nns, TemplateName, TypeView};
use kiln_ast::written_refs::{
    REF_DEPENDENT, REF_UNRESOLVED, RefMark, RefQualifier, RefRole, WrittenRef, WrittenRefId,
};
use kiln_ast::{DeclId, ExprId, NnsId, QualType};
use kiln_source::Span;

use crate::Sema;

/// What a built name expression resolved to.
enum Target {
    /// The entity, and what lookup found when it differs.
    Decl(DeclId, Option<DeclId>),
    /// An overload set, resolved later.
    Overloaded(ExprId),
    /// A concept-id's concept.
    Concept(DeclId),
    /// A pseudo-destructor's destroyed scalar type (its type-name).
    Type(QualType),
    /// A dependent name.
    Dependent,
    /// Nothing (an error).
    Unresolved,
}

impl Sema<'_, '_> {
    /// True when names are recorded (one branch; spec/ast.md §18.1).
    #[inline]
    pub fn records_written_refs(&self) -> bool {
        self.ast.written_refs.is_on()
    }

    /// Records the expression or member name written at `range`, which
    /// built `e` (`None`: nothing was built), with qualifier `q`.
    pub fn note_written_name(
        &mut self,
        e: Option<ExprId>,
        range: Span,
        role: RefRole,
        q: RefQualifier,
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
            Target::Dependent => (None, None, REF_DEPENDENT, None),
            Target::Unresolved => (None, None, REF_UNRESOLVED, None),
        };
        let id = self.push_written_ref(range, entity, found, role, q, flags)?;
        if let Some(x) = pending {
            self.ast.written_refs.defer(id, x);
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
        self.push_written_ref(range, entity, found, role, q, flags)
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
        if self.ast.written_refs.has_pending() && self.inst.stack.is_empty() {
            self.ast.written_refs.resolve(x, d, found);
        }
    }

    /// The overload set `from` goes on as the set `to`.
    #[inline]
    pub(crate) fn retarget_written_ref(&mut self, from: ExprId, to: ExprId) {
        if self.ast.written_refs.has_pending() && self.inst.stack.is_empty() {
            self.ast.written_refs.retarget(from, to);
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
        self.ast.written_refs.push(WrittenRef {
            range,
            entity,
            found,
            enclosing,
            qualifier,
            role,
            flags,
        })
    }

    /// What the name expression `e` (implicit conversions peeled)
    /// resolved to.
    fn written_target(&self, mut e: ExprId) -> Target {
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
                    return Target::Dependent;
                }
                _ if n.is_type_dependent() || n.is_value_dependent() => return Target::Dependent,
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
    fn written_type_entity(&self, ty: QualType, depth: u32) -> Option<(DeclId, Option<DeclId>)> {
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
