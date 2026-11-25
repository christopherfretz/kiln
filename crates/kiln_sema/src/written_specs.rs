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
    ScopeRef, SpecRef, SpecRefId, SubstRef, WrittenRefId,
};
use kiln_ast::{DeclId, ExprId, NnsId, QualType};
use kiln_source::Loc;

use crate::Sema;
use crate::templ::stack::FrameKind;
use crate::written_keys::RowKeys;
use crate::written_refs::Target;

/// What [`Sema::push_written_spec_with`] recorded: the per-specialization
/// row and the key scope's buffered row (scope, index), either or both.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct WrittenPush {
    pub(crate) spec: Option<SpecRefId>,
    pub(crate) key: Option<(ScopeRef, u32)>,
}

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
        let node = PatternNode::Expr(pattern);
        let Some(keys) = self.written_row_keys(w, Some(node)) else { return };
        match self.written_target(rebuilt) {
            // Still dependent (a generic lambda's body, a member template
            // substituted with the outer arguments only): the rebuilt node
            // stands for the entry in the next instantiation.
            Target::Dependent(x) => {
                self.ast.written_refs.note_pattern_node(PatternNode::Expr(x), w);
                self.note_keyed_still_dependent(w, keys.key, node, PatternNode::Expr(x));
                return;
            }
            // An overload set awaits its use: the call that resolves it
            // records the row, whether by ordinary or by argument-dependent
            // lookup, also in a context that is itself a pattern (a
            // constraint satisfied with the template as context, a generic
            // lambda's body). There the set stands for the entry in the
            // next instantiation until it resolves, and does so for good
            // when its call stays dependent
            // ([`Sema::written_spec_call_dependent`]).
            Target::Overloaded(x) => {
                if self.written_ctx_dependent(keys) {
                    self.ast.written_refs.note_pattern_node(PatternNode::Expr(x), w);
                } else if let Some(k) = keys.key {
                    self.ast.written_refs.defer_subst(x, w, k);
                }
                if let Some(spec) = keys.spec {
                    self.ast.written_refs.defer_spec(x, w, spec);
                }
            }
            Target::Decl(d, f) => {
                self.push_written_spec(w, keys, Some(d), f);
            }
            Target::Concept(c) => {
                self.push_written_spec(w, keys, Some(c), None);
            }
            Target::Type(t) => {
                let e = self.written_type_entity(t, 0);
                self.push_written_spec(w, keys, e.map(|x| x.0), e.and_then(|x| x.1));
            }
            // A substitution failure records nothing (§18.9.3); the
            // substitution a key scope records fails.
            Target::Unresolved if self.inst.stack.is_sfinae() => {
                if let Some(k) = keys.key {
                    self.key_scope_poison(k);
                }
                return;
            }
            Target::Unresolved => {
                self.push_written_spec(w, keys, None, None);
            }
        }
        let nns = self.written_expr_qualifier(rebuilt);
        self.note_written_spec_qualifiers(w, keys, nns);
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
        let Some(keys) = self.written_row_keys(w, Some(PatternNode::Expr(pattern))) else {
            return;
        };
        self.push_written_spec(w, keys, None, None);
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
        let node = PatternNode::Type(q.without_quals(), loc);
        let Some(w) = self.ast.written_refs.pattern_entry(node) else {
            return;
        };
        let Some(keys) = self.written_row_keys(w, Some(node)) else { return };
        // A substitution failure records nothing (§18.9.3).
        if entity.is_none() && self.inst.stack.is_sfinae() {
            if let Some(k) = keys.key {
                self.key_scope_poison(k);
            }
            return;
        }
        self.push_written_spec(w, keys, entity, found);
        self.note_written_spec_qualifiers(w, keys, nns);
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
        let Some(keys) = self.written_row_keys(w, Some(PatternNode::Decl(pattern))) else {
            return;
        };
        self.push_written_spec(w, keys, entity, None);
        self.note_written_spec_qualifiers(w, keys, scope.map(|s| s.0));
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
        let Some(keys) = self.written_row_keys(w, Some(PatternNode::Decl(pattern))) else {
            return;
        };
        self.push_written_spec(w, keys, entity, found);
    }

    /// Overload resolution chose `d` (found as `found`; `None`: it failed)
    /// for the rebuilt overload set `x` of an instantiation: its
    /// per-specialization entry (§18.9.4) and its key scope's row
    /// (§18.14.3; dropped when the scope closed meanwhile).
    pub(crate) fn resolve_written_spec(
        &mut self,
        x: ExprId,
        d: Option<DeclId>,
        found: Option<DeclId>,
    ) -> Option<WrittenPush> {
        let spec = self.ast.written_refs.take_spec_pending(x);
        let key = self.ast.written_refs.take_subst_pending(x);
        let w = spec.map(|p| p.0).or(key.map(|p| p.0))?;
        // Resolved in a context that is itself a pattern: a row, not a
        // further node.
        self.ast.written_refs.forget_pattern_node(PatternNode::Expr(x), w);
        let keys = RowKeys {
            spec: spec.map(|p| p.1),
            key: key.map(|p| p.1).filter(|&k| self.key_scopes.records(k)),
        };
        let p = self.push_written_spec(w, keys, d, found);
        if let Some(id) = p.spec {
            self.ast.written_refs.set_spec_last(x, id);
        }
        if let Some((k, i)) = p.key {
            self.ast.written_refs.set_subst_last(x, k, i);
        }
        Some(p)
    }

    /// The pending rows of the rebuilt overload set `x` are dropped: a
    /// substitution failure records nothing (§18.9.3), and fails the
    /// substitution a key scope records.
    fn drop_written_spec_pending(&mut self, x: ExprId) {
        let _ = self.ast.written_refs.take_spec_pending(x);
        if let Some((_, k)) = self.ast.written_refs.take_subst_pending(x) {
            self.key_scope_poison(k);
        }
    }

    /// The call of the rebuilt overload set `callee` (possibly
    /// parenthesized) stays dependent, in a context that is itself a
    /// pattern: the set stands for its entry in the next instantiation (a
    /// further node), and records no row here (§18.9.4).
    pub(crate) fn written_spec_call_dependent(&mut self, callee: ExprId) {
        let x = self.ignore_parens(callee);
        if let Some((w, _)) = self.ast.written_refs.take_spec_pending(x) {
            self.ast.written_refs.note_pattern_node(PatternNode::Expr(x), w);
        }
    }

    /// The call whose rebuilt callee `x` stands for pattern entry `w` in
    /// `spec` was resolved (or failed), after argument-dependent lookup
    /// saw `facts`: the entry gets `Adl`/`Both` and the associated set
    /// (§18.7.4, §18.9.4), in its key scope's row as well.
    pub(crate) fn note_written_spec_call(
        &mut self,
        x: ExprId,
        facts: Option<crate::overload::adl::AdlFacts>,
    ) {
        let mut p = WrittenPush {
            spec: self.ast.written_refs.take_spec_last(x),
            key: self.ast.written_refs.take_subst_last(x),
        };
        if p.spec.is_none() && p.key.is_none() {
            // Overload resolution failed: the name stays unresolved, but
            // a substitution failure records nothing (§18.9.3).
            if self.inst.stack.is_sfinae() {
                self.drop_written_spec_pending(x);
                return;
            }
            let Some(q) = self.resolve_written_spec(x, None, None) else { return };
            let _ = self.ast.written_refs.take_spec_last(x);
            let _ = self.ast.written_refs.take_subst_last(x);
            p = q;
        }
        let Some(f) = facts else { return };
        let (entity, found) = match (p.spec, p.key) {
            (Some(id), _) => {
                let r = self.ast.written_refs.spec(id);
                (r.entity, r.found)
            }
            (None, Some((k, i))) => match self.key_scopes.row(k, i) {
                Some(r) => (r.entity, r.found),
                None => return,
            },
            (None, None) => return,
        };
        let Some((how, set)) = self.written_call_lookup(entity, found, &f) else { return };
        let set = self.ast.written_refs.intern_adl(set);
        if let Some(id) = p.spec {
            self.ast.written_refs.update_spec(id, |r| {
                if let Some(h) = how {
                    r.how = h;
                    // Found only by argument-dependent lookup, which
                    // ignores using-directives ([basic.lookup.argdep]/4).
                    if h == RefLookup::Adl {
                        r.udir = None;
                    }
                }
                r.adl = Some(set);
            });
        }
        if let Some((k, i)) = p.key {
            let _ = self.key_scope_update(k, i, |r| {
                if let Some(h) = how {
                    r.how = h;
                    if h == RefLookup::Adl {
                        r.udir = None;
                    }
                }
                r.adl = Some(set);
            });
        }
    }

    /// The one-candidate call whose callee became `callee` was checked:
    /// `viable`, its arguments converted to the candidate's parameters,
    /// and the row parked for it names the candidate; otherwise the call
    /// failed as a candidate set would, so its name stays unresolved, and
    /// a substitution failure records nothing (§18.9.3, §18.9.4).
    pub(crate) fn finish_single_written_spec(&mut self, callee: ExprId, viable: bool) {
        let Some(i) = self.ovl.single_specs.iter().rposition(|s| s.callee == callee) else {
            return;
        };
        let s = self.ovl.single_specs.remove(i);
        self.ast.written_refs.defer_spec(s.set, s.entry, s.spec);
        if viable {
            let _ = self.resolve_written_spec(s.set, Some(s.func), Some(s.found));
        }
        self.note_written_spec_call(s.set, s.facts);
    }

    /// The calls built since the parked rows numbered `mark` are done:
    /// a row still parked (its call ended before its arguments were
    /// converted, a builtin's or an implicit member call's) names its
    /// candidate (§18.9.4).
    pub(crate) fn finish_single_written_specs(&mut self, mark: usize) {
        while self.ovl.single_specs.len() > mark {
            let Some(s) = self.ovl.single_specs.last() else { return };
            let callee = s.callee;
            self.finish_single_written_spec(callee, true);
        }
    }

    /// The member call of the rebuilt overloaded member access `x`
    /// failed: its entry stays unresolved.
    #[inline]
    pub(crate) fn fail_written_spec(&mut self, x: ExprId) {
        if self.ast.written_refs.has_spec_pending() {
            if self.inst.stack.is_sfinae() {
                self.drop_written_spec_pending(x);
                return;
            }
            let _ = self.resolve_written_spec(x, None, None);
            let _ = self.ast.written_refs.take_spec_last(x);
            let _ = self.ast.written_refs.take_subst_last(x);
        }
    }

    /// Records that pattern entry `w` named `entity` (found as `found`)
    /// for `keys`.
    fn push_written_spec(
        &mut self,
        w: WrittenRefId,
        keys: RowKeys,
        entity: Option<DeclId>,
        found: Option<DeclId>,
    ) -> WrittenPush {
        self.push_written_spec_with(w, keys, entity, found, 0, None)
    }

    /// [`Self::push_written_spec`] with a dependent operator's facts
    /// (spec/ast.md §18.13.6): `flags` (`REWRITTEN`, `REVERSED`,
    /// `SPEC_BUILTIN`, which makes the row entity-less and `how`
    /// `Builtin`) and the `how` its call's facts give (`Member` for a
    /// member function, `Adl` for a candidate argument-dependent lookup
    /// alone found; `None`: as for a name). The row goes to the
    /// per-specialization table under `keys.spec` and to the key scope
    /// `keys.key` (spec/ast.md §18.14.3), where an unresolved name fails
    /// the substitution instead.
    pub(crate) fn push_written_spec_with(
        &mut self,
        w: WrittenRefId,
        keys: RowKeys,
        entity: Option<DeclId>,
        found: Option<DeclId>,
        flags: u8,
        how: Option<RefLookup>,
    ) -> WrittenPush {
        let r = *self.ast.written_refs.get(w);
        let builtin = flags & SPEC_BUILTIN != 0;
        let (entity, found) = if builtin { (None, None) } else { (entity, found) };
        let flags = (r.flags & REF_MEMBER)
            | flags
            | if entity.is_none() && !builtin { REF_UNRESOLVED } else { 0 };
        let mut row = SpecRef {
            entry: w,
            // A keyed row alone drops the specialization (`SubstRef::of_spec`).
            spec: keys.spec.unwrap_or(self.ast.tu),
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
        let key = match keys.key {
            Some(k) if entity.is_none() && !builtin => {
                self.key_scope_poison(k);
                None
            }
            Some(k) => self.key_scope_push(k, SubstRef::of_spec(&row)).map(|i| (k, i)),
            None => None,
        };
        let spec = keys.spec.map(|_| self.ast.written_refs.push_spec(row));
        WrittenPush { spec, key }
    }

    /// The dependent components of entry `w`'s qualifier, paired from the
    /// right with the components of the substituted `nns`: each names
    /// what its substituted component names (§18.9.4).
    fn note_written_spec_qualifiers(&mut self, w: WrittenRefId, keys: RowKeys, nns: Option<NnsId>) {
        let mut link = self.ast.written_refs.get(w).qualifier;
        let mut cur = nns;
        for _ in 0..64 {
            let (QualLink::Entry(qid), Some(c)) = (link.link(), cur) else { return };
            let q = *self.ast.written_refs.get(qid);
            // A component formed again from the object's class (`acme` of
            // `t.acme::B::f()`, resolved at the definition): a row when it
            // names something else now (§18.9.1).
            let relooked = self.templ.relooked_nns.contains(&c);
            if (q.entity.is_none() && q.flags & REF_DEPENDENT != 0) || relooked {
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
                if !relooked || q.flags & REF_DEPENDENT != 0 || entity != q.entity {
                    self.push_written_spec(qid, keys, entity, None);
                }
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
    /// whose overload sets are resolved by its own instantiations. For a
    /// key scope alone (no per-specialization key) the context the scope
    /// opened in is the substitution's own (spec/ast.md §18.14.3).
    pub(crate) fn written_ctx_dependent(&self, keys: RowKeys) -> bool {
        let c = self.ctx;
        if keys.spec.is_none()
            && let Some(k) = keys.key
            && self.key_scopes.home_ctx(k) == Some(c)
        {
            return false;
        }
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
    /// A deduction's default template argument keeps its own on success
    /// until the deduction ends ([`Self::end_written_spec_deduction`]).
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

    /// The deduction of function template `ft` that began at `mark`
    /// ended, forming `spec` (`None`: it failed). The rows its default
    /// template arguments recorded, keyed by `ft` while the
    /// specialization did not exist yet, become `spec`'s; a failed
    /// deduction's are dropped, as its substitution failures record
    /// nothing (§18.9.3).
    pub(crate) fn end_written_spec_deduction(
        &mut self,
        mark: u32,
        ft: DeclId,
        spec: Option<DeclId>,
    ) {
        let ft = self.ast.first(ft);
        let own: Vec<DeclId> = self
            .ast
            .written_refs
            .spec_keys_since(mark)
            .into_iter()
            .filter(|&k| self.ast.first(k) == ft)
            .collect();
        if own.is_empty() {
            return;
        }
        match spec {
            Some(s) => {
                for k in own {
                    self.ast.written_refs.rekey_specs_since(mark, k, s);
                }
            }
            None => self.ast.written_refs.drop_specs_since(mark, |k| own.contains(&k)),
        }
    }
}
