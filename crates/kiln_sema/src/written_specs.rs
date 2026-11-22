//! Stage 4 of the written-reference table (spec/ast.md §18.9, WP-1933):
//! per-specialization entries for the dependent names a template
//! instantiation resolves. The substituter calls these where it rebuilds
//! a dependent name; the pattern's entry is found from the pattern node
//! (`WrittenRefs::pattern_entry`), which the parse-time recording sites
//! remember only while recording is on, so with the switch off every site
//! is one branch on an empty map.

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::Nns;
use kiln_ast::written_refs::{
    PatternNode, QualLink, REF_DEPENDENT, REF_MEMBER, REF_UNRESOLVED, RefLookup, SpecRef,
    SpecRefId, WrittenRefId,
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
    /// substituted for, or `None` outside an instantiation and inside a
    /// check that is not one (constraint satisfaction, default-argument
    /// checking).
    pub(crate) fn written_spec_key(&self) -> Option<DeclId> {
        let over = self.ast.written_refs.spec_key();
        for (i, f) in self.inst.stack.frames.iter().enumerate().rev() {
            if let Some((depth, s)) = over
                && i < depth as usize
            {
                return Some(s);
            }
            match f.kind {
                // Parts of the instantiation around them.
                FrameKind::LambdaSubstitution | FrameKind::BindingInit => {}
                FrameKind::AliasTemplate | FrameKind::DefaultTemplateArg => {
                    return Some(f.template);
                }
                k if k.is_instantiation() => return Some(f.entity),
                _ => return None,
            }
        }
        over.map(|(_, s)| s)
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
        let r = *self.ast.written_refs.get(w);
        let flags = (r.flags & REF_MEMBER) | if entity.is_none() { REF_UNRESOLVED } else { 0 };
        let mut row = SpecRef {
            entry: w,
            spec,
            entity,
            found: found.filter(|&f| Some(f) != entity),
            adl: None,
            flags,
            how: RefLookup::None,
        };
        row.how = row.default_how(&r);
        if let Some(e) = entity
            && self.ast.decls.builtin_id(e).is_some()
            && matches!(row.how, RefLookup::Unqualified | RefLookup::Qualified)
        {
            row.how = RefLookup::Builtin;
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
