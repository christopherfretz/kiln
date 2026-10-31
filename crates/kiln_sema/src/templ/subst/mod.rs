//! The substituter (spec/sema-templates.md §5): rebuilds types,
//! expressions, statements and declarations of a template pattern with
//! the template arguments of a [`MultiLevelArgs`] substituted for its
//! parameters. Types go through the type context's constructors;
//! expressions and statements are rebuilt through the same Sema actions
//! the parser calls, so every semantic check runs again on the
//! instantiation ([temp.res.general]/6).
//!
//! A failure is reported (or, in a SFINAE context, captured, §5.7) where
//! it happens and returned as [`SubstFail`].

mod decls;
mod exprs;
mod stmts;
mod types;

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::ids::{ArgListId, ListId, NnsId};
use kiln_ast::types::{ArgView, Nns, TemplateArg, TemplateName, TypeFlags};
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::templ::args::{ArgLookup, MultiLevelArgs};

/// A substitution failure, already diagnosed (or captured, §5.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubstFail;

/// The result of a substitution step.
pub type SubstResult<T> = Result<T, SubstFail>;

/// What a local declaration of the pattern became (§5.6): one
/// declaration, or the expansion of a function parameter pack (WP-072).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclOrPack {
    /// The instantiated declaration.
    Decl(DeclId),
    /// The expanded parameters (a list of `DeclId`).
    Pack(ListId),
}

/// One substitution (clang's `TemplateInstantiator`, §5.2).
pub struct Substituter<'s, 'a, 'sm> {
    /// The semantic analyzer the rebuilt nodes go through.
    pub sema: &'s mut Sema<'a, 'sm>,
    /// The arguments, by depth.
    pub args: &'s MultiLevelArgs,
    /// Local declarations of the pattern and what they became.
    pub locals: FxHashMap<DeclId, DeclOrPack>,
    /// The element of the pack being expanded (WP-072).
    pub pack_index: Option<u32>,
    /// The innermost expansion's state (§7.2).
    pub pack: crate::templ::pack::PackState,
    /// The specialization (or member) being instantiated.
    pub entity: Option<DeclId>,
    /// Pattern contexts and their instantiations (the pattern class and
    /// its specialization), innermost last.
    pub(crate) pairs: Vec<(DeclId, DeclId)>,
    /// The name of the declaration whose type is being substituted
    /// (clang's diagnostics name it: "'a' declared as an array with a
    /// negative size").
    pub(crate) declarator_name: Option<(Symbol, Loc)>,
}

impl std::fmt::Debug for Substituter<'_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Substituter")
            .field("entity", &self.entity)
            .field("pack_index", &self.pack_index)
            .field("pairs", &self.pairs)
            .finish_non_exhaustive()
    }
}

impl<'s, 'a, 'sm> Substituter<'s, 'a, 'sm> {
    /// A substituter for `args`.
    pub fn new(sema: &'s mut Sema<'a, 'sm>, args: &'s MultiLevelArgs) -> Self {
        Substituter {
            sema,
            args,
            locals: FxHashMap::default(),
            pack_index: None,
            pack: crate::templ::pack::PackState::default(),
            entity: None,
            pairs: Vec::new(),
            declarator_name: None,
        }
    }

    /// Records that the pattern context `pattern` is instantiated as
    /// `inst` (member lookups through it go to `inst`'s member map).
    pub fn map_context(&mut self, pattern: DeclId, inst: DeclId) {
        self.pairs.push((pattern, inst));
    }

    /// Records pattern contexts and their instantiations, innermost first
    /// (as [`Sema::inst_context_pairs`] lists them); a pattern's first
    /// declaration maps too.
    pub(crate) fn map_contexts(&mut self, pairs: &[(DeclId, DeclId)]) {
        for &(p, x) in pairs.iter().rev() {
            self.map_context(p, x);
            let first = self.sema.ast.first(p);
            if first != p {
                self.map_context(first, x);
            }
        }
    }

    /// Reports `feature` as not yet implemented and fails.
    /// The stack guard's check at a substitution step (P4 (c)): a
    /// failure, after the fatal error, when the stack is nearly
    /// exhausted. `loc` is computed only then.
    #[inline]
    pub(crate) fn check_stack(&self, loc: impl FnOnce(&Self) -> Loc) -> SubstResult<()> {
        if !self.sema.stack_exhausted() && !self.sema.stack_low() {
            return Ok(());
        }
        let _ = self.sema.stack_ok(crate::stack_guard::StackPhase::Instantiation, loc(self));
        Err(SubstFail)
    }

    pub(crate) fn unimplemented<T>(&mut self, feature: &str, loc: Loc) -> SubstResult<T> {
        self.sema.templ_unimplemented(feature, loc);
        Err(SubstFail)
    }

    /// What template parameter `(depth, index)` becomes.
    pub(crate) fn lookup(&self, depth: u16, index: u16) -> ArgLookup {
        let types = &self.sema.ast.types;
        self.args.lookup(depth, index, |l| types.args(l).to_vec())
    }

    /// The associated declaration of level `depth` (the
    /// `SubstTemplateTypeParmType` sugar records it).
    pub(crate) fn assoc(&self, depth: u16) -> Option<DeclId> {
        self.args.level(depth).map(|l| l.assoc)
    }

    /// The element of a pack argument for the current pack index (§7.2);
    /// `a` itself when it is not a pack. `parm` names the parameter in
    /// the length-mismatch diagnostic.
    pub(crate) fn element(
        &mut self,
        a: TemplateArg,
        parm: Option<DeclId>,
        loc: Loc,
    ) -> SubstResult<TemplateArg> {
        self.pack_element(a, |s| parm.map_or_else(String::new, |d| s.sema.decl_name(d)), loc)
    }

    /// Substitutes into one template argument (§5.3).
    pub fn template_arg(&mut self, a: TemplateArg, loc: Loc) -> SubstResult<TemplateArg> {
        let flags = self.sema.ast.types.arg_flags(a);
        if !flags.intersects(TypeFlags::DEPENDENT_INSTANTIATION | TypeFlags::UNEXPANDED_PACK) {
            return Ok(a);
        }
        match a.view() {
            ArgView::Type(t) => Ok(TemplateArg::ty(self.ty(t, loc)?)),
            ArgView::Expr { expr, .. } => {
                let e = self.expr(expr)?;
                Ok(self.sema.ast.builder().template_arg_expr(e))
            }
            ArgView::Template(n) => Ok(TemplateArg::template(self.template_name(n, loc)?)),
            ArgView::Pack(_) | ArgView::TemplateExpansion { .. } | ArgView::Profile(_) => {
                self.unimplemented("pack expansion", loc)
            }
            ArgView::Null | ArgView::Value { .. } => Ok(a),
        }
    }

    /// Substitutes into an argument list, expanding pack expansions
    /// (§7.2).
    pub fn args(&mut self, l: ArgListId, loc: Loc) -> SubstResult<ArgListId> {
        let v = self.sema.ast.types.args(l).to_vec();
        let mut out = Vec::with_capacity(v.len());
        let mut changed = false;
        for a in v {
            if let Some(x) = self.expand_arg(a, loc)? {
                changed = true;
                out.extend(x);
                continue;
            }
            let b = self.template_arg(a, loc)?;
            changed |= b != a;
            out.push(b);
        }
        Ok(if changed { self.sema.ast.types.arg_list(&out) } else { l })
    }

    /// Expands template argument `a` when it is a pack expansion (`T...`,
    /// `v...`, `TT...`): its elements, or the expansion rebuilt when it
    /// cannot be expanded yet. `None` when `a` is not an expansion.
    pub(crate) fn expand_arg(
        &mut self,
        a: TemplateArg,
        loc: Loc,
    ) -> SubstResult<Option<Vec<TemplateArg>>> {
        use crate::templ::pack::{Expansion, expr_pattern, type_pattern};
        let r = match a.view() {
            ArgView::Type(q) => {
                let Some(p) = type_pattern(self.sema, q) else { return Ok(None) };
                match self.expand_pattern(|s| s.ty(p, loc))? {
                    Expansion::Elements(v) => v.into_iter().map(TemplateArg::ty).collect(),
                    Expansion::Unexpanded(t) => {
                        vec![TemplateArg::ty(self.sema.ast.types.pack_expansion(t, None))]
                    }
                }
            }
            ArgView::Expr { expr, .. } => {
                let Some(p) = expr_pattern(self.sema, expr) else { return Ok(None) };
                match self.expand_pattern(|s| s.expr(p))? {
                    Expansion::Elements(v) => {
                        v.into_iter().map(|e| self.sema.ast.builder().template_arg_expr(e)).collect()
                    }
                    Expansion::Unexpanded(e) => {
                        let x = self.sema.act_pack_expansion(e, loc);
                        vec![self.sema.ast.builder().template_arg_expr(x)]
                    }
                }
            }
            ArgView::TemplateExpansion { name, .. } => {
                match self.expand_pattern(|s| s.template_name(name, loc))? {
                    Expansion::Elements(v) => v.into_iter().map(TemplateArg::template).collect(),
                    Expansion::Unexpanded(n) => vec![TemplateArg::template_expansion(n, None)],
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(r))
    }

    /// Substitutes into a template name (a template template parameter
    /// becomes its argument's template).
    pub fn template_name(
        &mut self,
        n: kiln_ast::ids::TemplateNameId,
        loc: Loc,
    ) -> SubstResult<kiln_ast::ids::TemplateNameId> {
        match self.sema.ast.types.template_name_view(n) {
            TemplateName::TemplateTemplateParm { depth, index, .. }
            | TemplateName::CanonTemplateTemplateParm { depth, index, .. } => {
                let (d, i) = (depth as u16, index as u16);
                match self.lookup(d, i) {
                    ArgLookup::Arg(a) => match self.element(a, None, loc)?.view() {
                        ArgView::Template(t) => Ok(t),
                        _ => Err(SubstFail),
                    },
                    ArgLookup::Retain(nd) if nd == d => Ok(n),
                    ArgLookup::Retain(_) => {
                        let TemplateName::TemplateTemplateParm { decl, .. } =
                            self.sema.ast.types.template_name_view(n)
                        else {
                            return self.unimplemented("member templates", loc);
                        };
                        let np = self.find_instantiated_decl(decl, loc)?;
                        Ok(self.sema.template_name_for(np))
                    }
                    ArgLookup::Missing => Err(SubstFail),
                }
            }
            TemplateName::DependentIdentifier { nns, name } => {
                let q = self.nns(nns, loc)?;
                if let Nns::Type { ty, .. } = self.sema.ast.types.nns_view(q)
                    && self.sema.is_dependent_type(ty)
                {
                    return Ok(self
                        .sema
                        .ast
                        .types
                        .template_name(TemplateName::DependentIdentifier { nns: q, name }));
                }
                let d = self.member_template_named(q, name, loc)?;
                let underlying = self.sema.template_name_for(d);
                Ok(self.sema.ast.types.template_name(TemplateName::Qualified {
                    nns: q,
                    template_kw: true,
                    underlying,
                }))
            }
            // A member template of a class template names the member of
            // the instantiated class ([temp.mem]).
            TemplateName::Template(d)
                if self.sema.ast.decls.get(d).flags().contains(DeclFlags::DEPENDENT_CTX) =>
            {
                let x = self.find_instantiated_decl(d, loc)?;
                if x == d {
                    return Ok(n);
                }
                Ok(self.sema.template_name_for(x))
            }
            TemplateName::Qualified { nns, template_kw, underlying } => {
                let u = self.template_name(underlying, loc)?;
                let q = self.nns(nns, loc)?;
                if u == underlying && q == nns {
                    return Ok(n);
                }
                Ok(self.sema.ast.types.template_name(TemplateName::Qualified {
                    nns: q,
                    template_kw,
                    underlying: u,
                }))
            }
            _ => Ok(n),
        }
    }

    /// Substitutes into a nested-name-specifier (§5.3): the result
    /// names the instantiated context.
    pub fn nns(&mut self, n: NnsId, loc: Loc) -> SubstResult<NnsId> {
        let flags = self.sema.ast.types.nns_flags(n);
        if !flags.intersects(TypeFlags::DEPENDENT_INSTANTIATION | TypeFlags::UNEXPANDED_PACK) {
            return Ok(n);
        }
        let v = self.sema.ast.types.nns_view(n);
        let r = match v {
            Nns::Type { prefix, ty, template_kw } => {
                let prefix = match prefix {
                    Some(p) => Some(self.nns(p, loc)?),
                    None => None,
                };
                let ty = self.ty(ty, loc)?;
                Nns::Type { prefix, ty, template_kw }
            }
            Nns::Identifier { prefix, name } => {
                let Some(p) = prefix else { return self.unimplemented("dependent name", loc) };
                let p = self.nns(p, loc)?;
                let ctx = self.nns_context(p, loc)?;
                let Some((c, spelled)) = self.sema.act_on_nested_name_spelled(Some(ctx), name, loc)
                else {
                    return Err(SubstFail);
                };
                return Ok(self.sema.nns_component(Some(p), c, spelled));
            }
            Nns::Namespace { prefix: Some(p), ns } => {
                Nns::Namespace { prefix: Some(self.nns(p, loc)?), ns }
            }
            Nns::NamespaceAlias { prefix: Some(p), alias, ns } => {
                Nns::NamespaceAlias { prefix: Some(self.nns(p, loc)?), alias, ns }
            }
            other => other,
        };
        Ok(self.sema.ast.types.nns(r))
    }

    /// The context a substituted nested-name-specifier names (a class
    /// type is instantiated, [temp.inst]/2: a class member access).
    pub(crate) fn nns_context(&mut self, n: NnsId, loc: Loc) -> SubstResult<DeclId> {
        match self.sema.ast.types.nns_view(n) {
            Nns::Global => Ok(self.sema.ast.tu),
            Nns::Namespace { ns, .. } | Nns::NamespaceAlias { ns, .. } => Ok(ns),
            Nns::Type { ty, .. } => {
                if self.sema.is_dependent_type(ty) {
                    return self.unimplemented("dependent nested name specifier", loc);
                }
                let t = &self.sema.ast.types;
                let Some(r) = t.as_record(ty).or_else(|| t.as_enum(ty)) else {
                    let d = self
                        .sema
                        .diag(kiln_diag::DiagId::sema_nested_name_spec_non_tag, loc)
                        .arg(self.sema.ty_arg(ty));
                    self.sema.emit(d);
                    return Err(SubstFail);
                };
                if !self.sema.require_complete_nns(ty, loc) {
                    return Err(SubstFail);
                }
                Ok(self.sema.ast.definition(r).unwrap_or(r))
            }
            _ => self.unimplemented("this nested name specifier", loc),
        }
    }

    /// The declaration of the instantiation that pattern declaration `d`
    /// became (clang's `FindInstantiatedDecl`, §5.6): a local of the
    /// pattern, a member of an instantiated class, or `d` itself when it
    /// is not in a dependent context.
    pub fn find_instantiated_decl(&mut self, d: DeclId, loc: Loc) -> SubstResult<DeclId> {
        if let Some(&x) = self.locals.get(&d) {
            return match x {
                DeclOrPack::Decl(x) => Ok(x),
                DeclOrPack::Pack(l) => {
                    let n = u32::try_from(self.sema.ast.lists.len(l)).unwrap_or(u32::MAX);
                    let i = self.pack_reached(n, |s| s.sema.decl_name(d), loc)?;
                    self.sema.ast.lists.at::<DeclId>(l, i as usize).ok_or(SubstFail)
                }
            };
        }
        let node = *self.sema.ast.decls.get(d);
        if !node.flags().contains(DeclFlags::DEPENDENT_CTX) {
            return Ok(d);
        }
        if let Some(&(_, i)) = self.pairs.iter().rev().find(|p| p.0 == d) {
            return Ok(i);
        }
        if matches!(
            node.kind(),
            DeclKind::TemplateTypeParm
                | DeclKind::NonTypeTemplateParm
                | DeclKind::TemplateTemplateParm
        ) {
            return Ok(d);
        }
        // A member of a class: find the instantiated class, then its
        // member instantiated from `d`.
        let Some(parent) = node.ctx() else { return Ok(d) };
        // The pattern class of a class template is the specialization
        // being instantiated (its template is not itself a context).
        let inst_parent = self.find_instantiated_decl(parent, loc)?;
        if inst_parent == parent {
            return self.unimplemented("reference to a member of a dependent context", loc);
        }
        if self.sema.kind_of(inst_parent) == DeclKind::CXXRecord {
            let ty = self.sema.tag_type(inst_parent);
            let _ = self.sema.require_complete_silent(ty, loc);
        }
        if let Some(m) = self.sema.ast.templates.find_member(inst_parent, d) {
            return Ok(m);
        }
        // A member not instantiated (an invalid member of the
        // specialization): already diagnosed.
        Err(SubstFail)
    }
}

impl Sema<'_, '_> {
    /// Whether template-argument flags `f` make a node dependent.
    pub(crate) fn dependent_flags(f: TypeFlags) -> bool {
        f.intersects(TypeFlags::DEPENDENT_INSTANTIATION | TypeFlags::UNEXPANDED_PACK)
    }
}
