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

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::ids::{ArgListId, ListId, NnsId};
use kiln_ast::types::{ArgView, Nns, TemplateArg, TemplateName, TypeFlags};
use kiln_ast::DeclId;
use kiln_source::fx::FxHashMap;
use kiln_source::Loc;
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

    /// Reports `feature` as not yet implemented and fails.
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

    /// A pack argument found where one element was expected: pack
    /// expansion is WP-072.
    pub(crate) fn element(&mut self, a: TemplateArg, loc: Loc) -> SubstResult<TemplateArg> {
        match a.view() {
            ArgView::Pack(l) => match self.pack_index {
                Some(i) => Ok(self.sema.ast.types.args(l).get(i as usize).copied().unwrap_or(a)),
                None => self.unimplemented("pack expansion", loc),
            },
            _ => Ok(a),
        }
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

    /// Substitutes into an argument list (no pack expansions, WP-072).
    pub fn args(&mut self, l: ArgListId, loc: Loc) -> SubstResult<ArgListId> {
        let v = self.sema.ast.types.args(l).to_vec();
        let mut out = Vec::with_capacity(v.len());
        let mut changed = false;
        for a in v {
            let b = self.template_arg(a, loc)?;
            changed |= b != a;
            out.push(b);
        }
        Ok(if changed { self.sema.ast.types.arg_list(&out) } else { l })
    }

    /// Expands a list of pattern elements that may contain pack
    /// expansions: lists without expansions map one to one. Expansion is
    /// WP-072.
    pub fn expand_list<T: Copy>(
        &mut self,
        items: &[T],
        is_expansion: impl Fn(&Sema<'_, '_>, T) -> bool,
        mut each: impl FnMut(&mut Self, T) -> SubstResult<T>,
        loc: Loc,
    ) -> SubstResult<Vec<T>> {
        let mut out = Vec::with_capacity(items.len());
        for &x in items {
            if is_expansion(self.sema, x) {
                return self.unimplemented("pack expansion", loc);
            }
            out.push(each(self, x)?);
        }
        Ok(out)
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
                    ArgLookup::Arg(a) => match self.element(a, loc)?.view() {
                        ArgView::Template(t) => Ok(t),
                        _ => Err(SubstFail),
                    },
                    ArgLookup::Retain(nd) if nd == d => Ok(n),
                    ArgLookup::Retain(_) => self.unimplemented("member templates", loc),
                    ArgLookup::Missing => Err(SubstFail),
                }
            }
            TemplateName::DependentIdentifier { .. } => {
                self.unimplemented("dependent template name", loc)
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
            Nns::Namespace { prefix: Some(p), ns } => Nns::Namespace { prefix: Some(self.nns(p, loc)?), ns },
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
                DeclOrPack::Pack(_) => self.unimplemented("pack expansion", loc),
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
            DeclKind::TemplateTypeParm | DeclKind::NonTypeTemplateParm | DeclKind::TemplateTemplateParm
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
