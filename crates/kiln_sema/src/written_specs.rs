//! Stage 4 of the written-reference table (spec/ast.md §18.9, WP-1933):
//! per-specialization entries for the dependent names a template
//! instantiation resolves. The substituter calls these where it rebuilds
//! a dependent name; the pattern's entry is found from the pattern node
//! (`WrittenRefs::pattern_entry`), which the parse-time recording sites
//! remember only while recording is on, so with the switch off every site
//! is one branch on an empty map.

use kiln_ast::decl::{DeclFlags, DeclKind, FriendTarget};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::Nns;
use kiln_ast::written_refs::{
    PatternNode, QualLink, REF_DEPENDENT, REF_MEMBER, REF_UNRESOLVED, RefLookup, SPEC_BUILTIN,
    SpecRef, SpecRefId, WrittenRefId,
};
use kiln_ast::{DeclId, ExprId, NnsId, QualType};
use kiln_source::Loc;

use crate::Sema;
use crate::templ::stack::FrameKind;
use crate::written_refs::Target;

impl Sema<'_, '_> {
    /// The specialization the lookups running now resolve names for
    /// (spec/ast.md §18.9.2): the innermost instantiation frame's entity
    /// (an alias template's or a default template argument's frame: the
    /// template), the specialization a function template's signature is
    /// substituted for, the declaration whose constraints are being
    /// satisfied, or `None` outside an instantiation and inside a check
    /// that is not one (default-argument checking).
    pub(crate) fn written_spec_key(&self) -> Option<DeclId> {
        let over = self.ast.written_refs.spec_key();
        for (i, f) in self.inst.stack.frames.iter().enumerate().rev() {
            if let Some((depth, s)) = over
                && i < depth as usize
            {
                return Some(s);
            }
            match f.kind {
                // Parts of the instantiation around them; a
                // requires-expression's requirements are part of what
                // contains the requires-expression (§18.9.2).
                FrameKind::LambdaSubstitution
                | FrameKind::BindingInit
                | FrameKind::RequirementInstantiation => {}
                FrameKind::AliasTemplate | FrameKind::DefaultTemplateArg => {
                    return Some(f.template);
                }
                k if k.is_constraint_substitution() || k.is_satisfaction_check() => {
                    match self.kind_of(f.entity) {
                        DeclKind::RequiresExprBody => {}
                        // A builtin trait's own check (`__is_invocable`).
                        DeclKind::TranslationUnit => return None,
                        _ => return Some(self.constraint_spec_key(f.entity)),
                    }
                }
                k if k.is_instantiation() => return Some(f.entity),
                _ => return None,
            }
        }
        over.map(|(_, s)| s)
    }

    /// The key of a name in the constraints of `entity` being satisfied
    /// (§18.9.2): the declaration they belong to, a concept, a
    /// constrained declaration or a member of a class template
    /// specialization; a function or class template's templated
    /// declaration, so the template's deduction frame does not drop the
    /// rows as its own (§18.9.3).
    fn constraint_spec_key(&self, entity: DeclId) -> DeclId {
        match self.kind_of(entity) {
            DeclKind::FunctionTemplate | DeclKind::ClassTemplate | DeclKind::VarTemplate => {
                self.pattern_of(entity).unwrap_or(entity)
            }
            _ => entity,
        }
    }

    /// The substituter rebuilt the pattern expression `pattern` as
    /// `rebuilt`: when `pattern` is a dependent name's node, records what
    /// it resolved to in the current specialization (§18.9.4). Callers
    /// test `WrittenRefs::has_pattern_nodes` first.
    pub(crate) fn note_written_spec_expr(&mut self, pattern: ExprId, rebuilt: ExprId) {
        if rebuilt == pattern {
            return;
        }
        let Some(w) = self.ast.written_refs.pattern_entry(PatternNode::Expr(pattern)) else {
            return;
        };
        let r = *self.ast.written_refs.get(w);
        // Resolved at the definition (a non-dependent overload set
        // patched by overload resolution).
        if r.flags & REF_DEPENDENT == 0 {
            return;
        }
        let Some(spec) = self.written_spec_key() else { return };
        match self.written_target(rebuilt) {
            // Still dependent (a generic lambda's body, a member template
            // substituted with the outer arguments only): the rebuilt node
            // stands for the entry in the next instantiation.
            Target::Dependent(x) => {
                self.ast.written_refs.note_pattern_node(PatternNode::Expr(x), w);
                return;
            }
            Target::Overloaded(x) => {
                if self.written_ctx_dependent() {
                    self.ast.written_refs.note_pattern_node(PatternNode::Expr(x), w);
                } else {
                    self.ast.written_refs.defer_spec(x, w, spec);
                }
            }
            Target::Decl(d, f) => {
                self.push_written_spec(w, spec, Some(d), f);
            }
            Target::Concept(c) => {
                self.push_written_spec(w, spec, Some(c), None);
            }
            Target::Type(t) => {
                let e = self.written_type_entity(t, 0);
                self.push_written_spec(w, spec, e.map(|x| x.0), e.and_then(|x| x.1));
            }
            // A substitution failure records nothing (§18.9.3).
            Target::Unresolved if self.inst.stack.is_sfinae() => return,
            Target::Unresolved => {
                self.push_written_spec(w, spec, None, None);
            }
        }
        let nns = self.written_expr_qualifier(rebuilt);
        self.note_written_spec_qualifiers(w, spec, nns);
    }

    /// Before the substituter rebuilds a pattern expression: the errors
    /// reported so far when per-specialization entries are recorded
    /// (`None`, after one branch, when not).
    #[inline]
    pub(crate) fn written_spec_errors(&self) -> Option<u32> {
        if self.ast.written_refs.has_pattern_nodes() {
            Some(self.diags.error_count())
        } else {
            None
        }
    }

    /// The substituter failed to rebuild the pattern expression
    /// `pattern`, `errors` having been reported before it started: when
    /// it is a dependent name's node and the failure is an error
    /// reported since, outside a SFINAE context, its name stays
    /// unresolved in the current specialization (§18.9.3). A substitution
    /// failure, or a rebuild given up without a diagnostic (a trial
    /// substitution), records nothing.
    pub(crate) fn note_written_spec_failed(&mut self, pattern: ExprId, errors: u32) {
        let Some(w) = self.ast.written_refs.pattern_entry(PatternNode::Expr(pattern)) else {
            return;
        };
        if self.inst.stack.is_sfinae() || self.diags.error_count() <= errors {
            return;
        }
        if self.ast.written_refs.get(w).flags & REF_DEPENDENT == 0 {
            return;
        }
        let Some(spec) = self.written_spec_key() else { return };
        self.push_written_spec(w, spec, None, None);
    }

    /// The substituter resolved the dependent type `q`, written with its
    /// name at `loc` (§5.8's occurrence), as naming `entity` (found as
    /// `found`) after the substituted qualifier `nns` (§18.9.4).
    pub(crate) fn note_written_spec_type(
        &mut self,
        q: QualType,
        loc: Loc,
        entity: Option<DeclId>,
        found: Option<DeclId>,
        nns: Option<NnsId>,
    ) {
        let Some(w) =
            self.ast.written_refs.pattern_entry(PatternNode::Type(q.without_quals(), loc))
        else {
            return;
        };
        let Some(spec) = self.written_spec_key() else { return };
        // A substitution failure records nothing (§18.9.3).
        if entity.is_none() && self.inst.stack.is_sfinae() {
            return;
        }
        self.push_written_spec(w, spec, entity, found);
        self.note_written_spec_qualifiers(w, spec, nns);
    }

    /// The class template specialization's instantiation redid the
    /// pattern's dependent using-declarator `pattern` (§18.9.4) with the
    /// substituted qualifier `nns` (naming the class `ctx`) and the name
    /// `name`: `inst` is the using-declaration it made (`None`: it failed
    /// with an error since `errors`), an `UnresolvedUsing*` member while
    /// the qualifier is still dependent. The row's entity is the
    /// declaration the main table would record for the declarator written
    /// with that qualifier (§18.4): the first it introduced, else the
    /// first qualified lookup finds.
    pub(crate) fn note_written_spec_using(
        &mut self,
        pattern: DeclId,
        inst: Option<DeclId>,
        scope: Option<(NnsId, DeclId, kiln_ast::names::DeclName)>,
        errors: u32,
    ) {
        let Some(w) = self.ast.written_refs.pattern_entry(PatternNode::Decl(pattern)) else {
            return;
        };
        let entity = match inst.map(|i| (i, self.kind_of(i))) {
            Some((i, DeclKind::UnresolvedUsingValue | DeclKind::UnresolvedUsingTypename)) => {
                if i != pattern {
                    self.ast.written_refs.note_pattern_node(PatternNode::Decl(i), w);
                }
                return;
            }
            Some((i, _)) => {
                let first = self.ast.using_shadows(i).next().map(|s| self.ast.strip_shadow(s));
                first.or_else(|| {
                    let (_, ctx, name) = scope?;
                    let f = self.lookup_qualified_dn(ctx, name, crate::lookup::LookupKind::All);
                    f.decls.first().map(|&x| self.ast.strip_shadow(x))
                })
            }
            None if self.inst.stack.is_sfinae() || self.diags.error_count() <= errors => return,
            None => None,
        };
        let Some(spec) = self.written_spec_key() else { return };
        self.push_written_spec(w, spec, entity, None);
        self.note_written_spec_qualifiers(w, spec, scope.map(|s| s.0));
    }

    /// The friend function template specialization `pattern` of a class
    /// template pattern (a `FriendDecl` whose entry is `DEPENDENT`,
    /// §18.4) was instantiated as `inst`, `errors` having been reported
    /// before: its entry's row for the class template specialization
    /// names the specialization befriended, found as its template
    /// (§18.9.4). `None` matched no template: unresolved when an error
    /// was reported since (§18.9.3).
    pub(crate) fn note_written_spec_friend(
        &mut self,
        pattern: DeclId,
        inst: Option<DeclId>,
        errors: u32,
    ) {
        let Some(w) = self.ast.written_refs.pattern_entry(PatternNode::Decl(pattern)) else {
            return;
        };
        let (entity, found) = match inst.map(|f| self.ast.decls.friend_target(f)) {
            Some(FriendTarget::Decl(s)) => {
                let t = self.ast.spec_info(s).map(|(_, i)| self.ast.first(i.template));
                (Some(s), t)
            }
            Some(FriendTarget::Type { .. }) => return,
            None if self.inst.stack.is_sfinae() || self.diags.error_count() <= errors => return,
            None => (None, None),
        };
        let Some(spec) = self.written_spec_key() else { return };
        self.push_written_spec(w, spec, entity, found);
    }

    /// Overload resolution chose `d` (found as `found`; `None`: it failed)
    /// for the rebuilt overload set `x` of an instantiation: its
    /// per-specialization entry (§18.9.4).
    pub(crate) fn resolve_written_spec(
        &mut self,
        x: ExprId,
        d: Option<DeclId>,
        found: Option<DeclId>,
    ) -> Option<SpecRefId> {
        let (w, spec) = self.ast.written_refs.take_spec_pending(x)?;
        let id = self.push_written_spec(w, spec, d, found);
        self.ast.written_refs.set_spec_last(x, id);
        Some(id)
    }

    /// The call whose rebuilt callee `x` stands for pattern entry `w` in
    /// `spec` was resolved (or failed), after argument-dependent lookup
    /// saw `facts`: the entry gets `Adl`/`Both` and the associated set
    /// (§18.7.4, §18.9.4).
    pub(crate) fn note_written_spec_call(
        &mut self,
        x: ExprId,
        facts: Option<crate::overload::adl::AdlFacts>,
    ) {
        let id = match self.ast.written_refs.take_spec_last(x) {
            Some(id) => id,
            // Overload resolution failed: the name stays unresolved, but
            // a substitution failure records nothing (§18.9.3).
            None if self.inst.stack.is_sfinae() => {
                let _ = self.ast.written_refs.take_spec_pending(x);
                return;
            }
            None => match self.resolve_written_spec(x, None, None) {
                Some(id) => {
                    let _ = self.ast.written_refs.take_spec_last(x);
                    id
                }
                None => return,
            },
        };
        let Some(f) = facts else { return };
        let r = *self.ast.written_refs.spec(id);
        let Some((how, set)) = self.written_call_lookup(r.entity, r.found, &f) else { return };
        let set = self.ast.written_refs.intern_adl(set);
        self.ast.written_refs.update_spec(id, |r| {
            if let Some(h) = how {
                r.how = h;
                // Found only by argument-dependent lookup, which ignores
                // using-directives ([basic.lookup.argdep]/4).
                if h == RefLookup::Adl {
                    r.udir = None;
                }
            }
            r.adl = Some(set);
        });
    }

    /// The member call of the rebuilt overloaded member access `x`
    /// failed: its entry stays unresolved.
    #[inline]
    pub(crate) fn fail_written_spec(&mut self, x: ExprId) {
        if self.ast.written_refs.has_spec_pending() {
            if self.inst.stack.is_sfinae() {
                let _ = self.ast.written_refs.take_spec_pending(x);
                return;
            }
            let _ = self.resolve_written_spec(x, None, None);
            let _ = self.ast.written_refs.take_spec_last(x);
        }
    }

    /// Records that pattern entry `w` named `entity` (found as `found`)
    /// in `spec`.
    fn push_written_spec(
        &mut self,
        w: WrittenRefId,
        spec: DeclId,
        entity: Option<DeclId>,
        found: Option<DeclId>,
    ) -> SpecRefId {
        self.push_written_spec_with(w, spec, entity, found, 0, None)
    }

    /// [`Self::push_written_spec`] with a dependent operator's facts
    /// (spec/ast.md §18.13.6): `flags` (`REWRITTEN`, `REVERSED`,
    /// `SPEC_BUILTIN`, which makes the row entity-less and `how`
    /// `Builtin`) and the `how` its call's facts give (`Member` for a
    /// member function, `Adl` for a candidate argument-dependent lookup
    /// alone found; `None`: as for a name).
    pub(crate) fn push_written_spec_with(
        &mut self,
        w: WrittenRefId,
        spec: DeclId,
        entity: Option<DeclId>,
        found: Option<DeclId>,
        flags: u8,
        how: Option<RefLookup>,
    ) -> SpecRefId {
        let r = *self.ast.written_refs.get(w);
        let builtin = flags & SPEC_BUILTIN != 0;
        let (entity, found) = if builtin { (None, None) } else { (entity, found) };
        let flags = (r.flags & REF_MEMBER)
            | flags
            | if entity.is_none() && !builtin { REF_UNRESOLVED } else { 0 };
        let mut row = SpecRef {
            entry: w,
            spec,
            entity,
            found: found.filter(|&f| Some(f) != entity),
            adl: None,
            udir: None,
            flags,
            how: RefLookup::None,
        };
        row.how = if builtin { RefLookup::Builtin } else { row.default_how(&r) };
        if let Some(h) = how
            && entity.is_some()
        {
            row.how = h;
        }
        if let Some(e) = entity
            && self.ast.decls.builtin_id(e).is_some()
            && matches!(row.how, RefLookup::Unqualified | RefLookup::Qualified)
        {
            row.how = RefLookup::Builtin;
        }
        // The using-directive ordinary lookup at the definition found the
        // selected declaration through (§18.9.1).
        if let Some(e) = entity
            && matches!(row.how, RefLookup::Unqualified | RefLookup::Qualified)
        {
            let key = self.ast.first(self.ast.strip_shadow(row.found.unwrap_or(e)));
            row.udir = self.ast.written_refs.pattern_udir(w, key);
        }
        self.ast.written_refs.push_spec(row)
    }

    /// The dependent components of entry `w`'s qualifier, paired from the
    /// right with the components of the substituted `nns`: each names
    /// what its substituted component names (§18.9.4).
    fn note_written_spec_qualifiers(&mut self, w: WrittenRefId, spec: DeclId, nns: Option<NnsId>) {
        let mut link = self.ast.written_refs.get(w).qualifier;
        let mut cur = nns;
        for _ in 0..64 {
            let (QualLink::Entry(qid), Some(c)) = (link.link(), cur) else { return };
            let q = *self.ast.written_refs.get(qid);
            if q.entity.is_none() && q.flags & REF_DEPENDENT != 0 {
                let entity = match self.ast.types.nns_view(c) {
                    Nns::Namespace { ns, .. } => Some(ns),
                    Nns::NamespaceAlias { alias, .. } => Some(alias),
                    Nns::Type { ty, .. } => match self.written_type_entity(ty, 0) {
                        Some((d, _)) => Some(d),
                        None if self.ast.types.is_dependent(ty) => return,
                        None => None,
                    },
                    // Still dependent: resolved by a later instantiation.
                    Nns::Identifier { .. } | Nns::Splice(_) => return,
                    Nns::Global => None,
                };
                self.push_written_spec(qid, spec, entity, None);
            }
            link = q.qualifier;
            cur = self.ast.types.nns_prefix(c);
        }
    }

    /// The nested-name-specifier of a rebuilt name expression `e`.
    fn written_expr_qualifier(&self, mut e: ExprId) -> Option<NnsId> {
        let exprs = &self.ast.exprs;
        for _ in 0..16 {
            match exprs.get(e).kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr | ExprKind::ParenExpr => {
                    e = exprs.operand(e);
                }
                ExprKind::DeclRefExpr | ExprKind::MemberExpr => {
                    return exprs
                        .decl_ref_extra(e)
                        .and_then(|x| x.qualifier)
                        .or_else(|| self.ast.templates.expr_nns.get(e).copied());
                }
                _ => return self.ast.templates.expr_nns.get(e).copied(),
            }
        }
        None
    }

    /// True when the context being built is itself a template pattern (a
    /// generic lambda's call operator rebuilt inside an instantiation),
    /// whose overload sets are resolved by its own instantiations.
    fn written_ctx_dependent(&self) -> bool {
        let c = self.ctx;
        self.ast.decls.get(c).flags().contains(DeclFlags::DEPENDENT_CTX)
            || self.ast.described_template(c).is_some()
            || matches!(self.kind_of(c), DeclKind::FunctionTemplate)
    }

    /// The per-specialization table's mark at the start of a SFINAE
    /// frame (`None` when nothing is recorded).
    #[inline]
    pub(crate) fn written_spec_mark(&self) -> Option<u32> {
        self.records_written_refs().then(|| self.ast.written_refs.spec_mark())
    }

    /// A SFINAE frame of template `ft` ended (`failed`: with a
    /// substitution failure): the entries it recorded for `ft` itself
    /// are dropped, and on failure those for the specializations of `ft`
    /// it formed; what nested instantiations recorded stays (§18.9.3).
    pub(crate) fn end_written_spec_sfinae(&mut self, mark: u32, ft: DeclId, failed: bool) {
        let ft = self.ast.first(ft);
        let drop: Vec<DeclId> = self
            .ast
            .written_refs
            .spec_keys_since(mark)
            .into_iter()
            .filter(|&k| {
                self.ast.first(k) == ft
                    || (failed
                        && self
                            .ast
                            .spec_info(k)
                            .is_some_and(|(_, i)| self.ast.first(i.template) == ft))
            })
            .collect();
        if !drop.is_empty() {
            self.ast.written_refs.drop_specs_since(mark, |k| drop.contains(&k));
        }
    }
}
