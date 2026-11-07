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

mod anon;
mod decls;
mod exprs;
mod lambda;
mod local;
mod shared;
mod stmts;
mod types;

pub(crate) use local::LocalMethod;

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::ids::{ArgListId, ListId, NnsId, QualType};
use kiln_ast::types::{ArgView, Nns, TemplateArg, TemplateName, TypeFlags};
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::templ::args::{ArgLookup, MultiLevelArgs};
use crate::templ::written::WrittenKey;

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
#[allow(clippy::struct_excessive_bools)]
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
    /// The start of the declarator's type (the declaration's
    /// decl-specifiers, clang's `FunctionTypeLoc` begin): where a
    /// substituted function type returning an array or function is
    /// diagnosed ([dcl.fct]/11). Cleared inside function parameters.
    pub(crate) type_begin: Option<Loc>,
    /// The written outermost `&`/`&&` of the declarator whose type is
    /// substituted ([`crate::Sema::ref_locs`]): the first reference
    /// rebuilt takes it (clang's `ReferenceTypeLoc`).
    pub(crate) ref_loc: Option<Loc>,
    /// The written innermost `*`/`[` of the declarator whose type is
    /// substituted ([`crate::Sema::inner_chunk_locs`]): the first pointer
    /// or array rebuilt takes it (clang's `TypeLoc` sigil).
    pub(crate) chunk_loc: Option<Loc>,
    /// A type parameter becomes its (sugared) argument with no
    /// `SubstTemplateTypeParmType` around it (clang's `Final`
    /// substitution of a non-type parameter's type by
    /// `CheckTemplateArgument`).
    pub(crate) final_subst: bool,
    /// How many lambda bodies are being rebuilt (a shared expression in
    /// one still captures `this`).
    pub(crate) lambda_bodies: u32,
    /// A statement of the body failed to substitute after an error
    /// (clang's `TransformCompoundStmt` `SubStmtInvalid`: the body, and
    /// so the function template specialization, is invalid; WP-145).
    pub(crate) stmt_failed: bool,
    /// A `return` of the lambda body being rebuilt failed in a call
    /// operator with a placeholder return type: the lambda-expression
    /// fails (clang marks the call operator invalid; WP-434).
    pub(crate) lambda_return_failed: bool,
    /// Variable initializers that failed to substitute (the variable is
    /// invalid, its declaration statement is not: clang keeps it).
    pub(crate) init_failures: u32,
    /// The instantiated variables whose initializer failed to substitute
    /// with an error of this instantiation.
    pub(crate) failed_inits: Vec<DeclId>,
    /// A statement outside a lambda body named one of `failed_inits`
    /// (clang 22: a constexpr specialization doing so is invalid).
    pub(crate) failed_init_use: bool,
    /// The pattern's value expression of the innermost statement
    /// expression being rebuilt: its expression statement ends the
    /// statement expression (`act_on_stmt_expr_result`).
    pub(crate) stmt_expr_result: Option<kiln_ast::ExprId>,
    /// Block-scope declarations of the pattern whose instantiation
    /// failed (diagnosed, or silent under SFINAE), with the rest of their
    /// declaration statement: their uses are invalid without another
    /// diagnostic (clang keeps the invalid declaration in the
    /// instantiation's scope: an invalid `VarDecl`, `TypedefNameDecl`,
    /// `UsingDecl`, ...).
    pub(crate) failed_locals: Vec<DeclId>,
    /// Set when a declaration statement failed because its pattern
    /// declaration is invalid: the enclosing compound statement stops
    /// there (clang's `TransformCompoundStmt`).
    pub(crate) invalid_decl_stmt: bool,
    /// The explicitly specified pack deduction may still extend
    /// ([temp.arg.explicit]/9; clang's partially substituted pack), by
    /// (depth, index): its length is a lower bound, checked against the
    /// other packs of an expansion.
    pub(crate) partial_pack: Option<(u16, u16)>,
    /// Diagnostics on a dependent name point at where it was written
    /// nearest to the given location (§5.8); off for an alias template,
    /// whose substitution clang reports at the alias declaration.
    pub(crate) written_locs: bool,
    /// The `DependentScopeDeclRefExpr` about to be substituted is a
    /// call's callee: a template-id there is not resolved to its one
    /// specialization (the call deduces). Taken by that node.
    pub(crate) as_callee: bool,
    /// The occurrence rule of the declaration whose type is being
    /// substituted (§5.8; [`Substituter::with_written_scope`]).
    pub(crate) written_scope: Option<crate::templ::written::WrittenScope>,
    /// The occurrences an *after* `written_scope` has used, per dependent
    /// type: a type the declarator writes again (`T::type (*f)(T::type)`)
    /// resolves to its next occurrence (§5.8).
    written_used: std::cell::RefCell<FxHashMap<WrittenKey, Loc>>,
    /// The elements of the enclosing expansion statements being expanded
    /// ([stmt.expand], P1306R5), innermost last: what their
    /// `CXXExpansionSelectExpr`s become.
    pub(crate) expansions: Vec<crate::expansion::ExpansionElem>,
    /// Set when a substituted level's argument (or the instantiated
    /// class of an injected-class-name) was used: the substituted
    /// construct names an enclosing template's parameter
    /// ([temp.friend]/9; clang's `ConstraintRefersToContainingTemplateChecker`).
    pub(crate) outer_used: std::cell::Cell<bool>,
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
            type_begin: None,
            ref_loc: None,
            chunk_loc: None,
            final_subst: false,
            lambda_bodies: 0,
            stmt_failed: false,
            lambda_return_failed: false,
            init_failures: 0,
            failed_inits: Vec::new(),
            failed_init_use: false,
            stmt_expr_result: None,
            failed_locals: Vec::new(),
            invalid_decl_stmt: false,
            partial_pack: None,
            written_locs: true,
            as_callee: false,
            written_scope: None,
            written_used: std::cell::RefCell::default(),
            expansions: Vec::new(),
            outer_used: std::cell::Cell::new(false),
        }
    }

    /// Runs `f` with `scope` as the declaration-level occurrence rule
    /// (§5.8), restoring the previous one.
    pub(crate) fn with_written_scope<T>(
        &mut self,
        scope: crate::templ::written::WrittenScope,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let saved = self.written_scope.replace(scope);
        let used = self.written_used.take();
        let r = f(self);
        self.written_scope = saved;
        let _ = self.written_used.replace(used);
        r
    }

    /// Type operand `t` of the expression at `loc` (`sizeof(T::type)`,
    /// `typeid(T::type)`): its dependent names are the occurrences after
    /// the keyword (§5.8), not the nearest ones, which may belong to the
    /// enclosing declaration's type.
    pub(crate) fn ty_written_after(&mut self, t: QualType, loc: Loc) -> SubstResult<QualType> {
        use crate::templ::written::{WrittenScope, WrittenSide};
        let scope = WrittenScope { anchor: loc, side: WrittenSide::After, bound: loc };
        self.with_written_scope(scope, |s| s.ty(t, loc))
    }

    /// Where the dependent entity `key` was written nearest to `loc`
    /// ([`Substituter::written_locs`]). Under an *after* scope, each
    /// substitution of the same dependent type takes the next occurrence
    /// (types are substituted in written order: a leading return type,
    /// then the parameters). Only types advance: a nested-name-specifier
    /// is looked up more than once per occurrence.
    pub(crate) fn written(&self, key: WrittenKey, loc: Loc) -> Loc {
        use crate::templ::written::WrittenSide;
        if !self.written_locs {
            return loc;
        }
        let Some(sc) = self.written_scope.filter(|sc| {
            sc.side == WrittenSide::After && sc.anchor == loc && matches!(key, WrittenKey::Type(_))
        }) else {
            return self.sema.written_loc_in(key, loc, self.written_scope);
        };
        let prev = self.written_used.borrow().get(&key).copied();
        let found = match prev {
            Some(p) => {
                let next = crate::templ::written::WrittenScope { bound: p.with_offset(1), ..sc };
                let l = self.sema.written_loc_in(key, loc, Some(next));
                // None after the previous one: it stays.
                if l > p { l } else { p }
            }
            None => self.sema.written_loc_in(key, loc, Some(sc)),
        };
        let _ = self.written_used.borrow_mut().insert(key, found);
        found
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
        let r = self.args.lookup(depth, index, |l, i| types.args(l).get(i).copied());
        if !matches!(r, ArgLookup::Retain(_)) {
            self.outer_used.set(true);
        }
        r
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
        use crate::templ::pack::PackName;
        self.pack_element(
            a,
            |s| match parm {
                Some(d) => PackName {
                    name: s.sema.decl_name(d),
                    depth: s.sema.ast.template_parm(d).map(|i| i.depth),
                    outer: false,
                    partial: s.partial_pack.is_some()
                        && s.sema.ast.template_parm(d).map(|i| (i.depth, i.index))
                            == s.partial_pack,
                },
                None => PackName::default(),
            },
            loc,
        )
    }

    /// Substitutes into one template argument (§5.3).
    pub fn template_arg(&mut self, a: TemplateArg, loc: Loc) -> SubstResult<TemplateArg> {
        let flags = self.sema.ast.types.arg_flags(a);
        if !flags.intersects(TypeFlags::DEPENDENT_INSTANTIATION | TypeFlags::UNEXPANDED_PACK) {
            // A kept expression argument still odr-uses what it names in
            // the instantiation (`s.template f<&C::g>()` in a generic
            // lambda converted `&C::g` in a dependent context; clang's
            // `TransformDeclRefExpr` marks a kept node referenced).
            if let ArgView::Expr { expr, .. } = a.view() {
                self.sema.mark_shared_expr_referenced(expr);
            }
            return Ok(a);
        }
        match a.view() {
            ArgView::Type(t) => Ok(TemplateArg::ty(self.ty(t, loc)?)),
            ArgView::Expr { expr, .. } => {
                // A constant-expression ([temp.arg.nontype]/1): converted
                // later, it keeps the enclosing full-expression's cleanups
                // (spec/sema-class.md §5.3).
                let outer = self.sema.cleanup;
                let e = self.expr(expr)?;
                self.sema.note_constant_outer_cleanups(e, outer);
                Ok(self.sema.ast.builder().template_arg_expr(e))
            }
            ArgView::Template(n) => Ok(TemplateArg::template(self.template_name(n, loc)?)),
            // A converted argument pack (a partial specialization's
            // `<U>` against `class... Up` is the pack `{U}`): its
            // elements, expansions expanded (§7.2).
            ArgView::Pack(l) => Ok(TemplateArg::pack(self.args(l, loc)?)),
            // A canonical value-dependent argument: its first expression
            // stands for it (an implicit deduction guide's, WP-134).
            ArgView::Profile(p) if !flags.contains(TypeFlags::UNEXPANDED_PACK) => {
                let e = self.sema.ast.types.profile_expr(p).ok_or(SubstFail)?;
                let e = self.expr(e)?;
                Ok(self.sema.ast.builder().template_arg_expr(e))
            }
            ArgView::TemplateExpansion { .. } | ArgView::Profile(_) => {
                self.unimplemented("pack expansion", loc)
            }
            ArgView::Null | ArgView::Value { .. } => Ok(a),
        }
    }

    /// Substitutes into an argument list, expanding pack expansions
    /// (§7.2).
    pub fn args(&mut self, l: ArgListId, loc: Loc) -> SubstResult<ArgListId> {
        // The list is read in place (the arena only grows) and copied only
        // from its first changed argument on (WP-657).
        let n = self.sema.ast.types.args(l).len();
        let mut out: Option<Vec<TemplateArg>> = None;
        for i in 0..n {
            let a = self.sema.ast.types.args(l)[i];
            let prefix = |s: &Self| {
                let mut v = Vec::with_capacity(n);
                v.extend_from_slice(&s.sema.ast.types.args(l)[..i]);
                v
            };
            if let Some(x) = self.expand_arg(a, loc)? {
                out.get_or_insert_with(|| prefix(self)).extend(x);
                continue;
            }
            let b = self.template_arg(a, loc)?;
            if let Some(o) = out.as_mut() {
                o.push(b);
            } else if b != a {
                let mut v = prefix(self);
                v.push(b);
                out = Some(v);
            }
        }
        Ok(match out {
            Some(v) => self.sema.ast.types.arg_list(&v),
            None => l,
        })
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
        // A canonical argument that is an expansion (`I...` in an implicit
        // deduction guide's canonical signature): its first expression
        // stands for it, as in `template_arg`.
        let a = match a.view() {
            ArgView::Profile(p) => match self.sema.ast.types.profile_expr(p) {
                Some(e) if expr_pattern(self.sema, e).is_some() => {
                    self.sema.ast.builder().template_arg_expr(e)
                }
                _ => return Ok(None),
            },
            _ => a,
        };
        let r = match a.view() {
            ArgView::Type(q) => {
                let Some(p) = type_pattern(self.sema, q) else { return Ok(None) };
                match self.expand_pattern_rewrap(None, |s| s.element_ty(p, loc))? {
                    Expansion::Elements(v) => {
                        let rw = self.take_rewrap();
                        v.into_iter()
                            .enumerate()
                            .map(|(k, t)| {
                                TemplateArg::ty(if rw.get(k) == Some(&true) {
                                    self.sema.ast.types.pack_expansion(t, None)
                                } else {
                                    t
                                })
                            })
                            .collect()
                    }
                    Expansion::Unexpanded(t) => {
                        vec![TemplateArg::ty(self.sema.ast.types.pack_expansion(t, None))]
                    }
                }
            }
            ArgView::Expr { expr, .. } => {
                let Some(p) = expr_pattern(self.sema, expr) else { return Ok(None) };
                // `element_expr`: an element that still names a pack of a
                // level not substituted (`B<P<Ts, Us>::value...>` in a
                // member template, only `Ts` known) keeps the expansion
                // (§7.2).
                match self.expand_pattern_rewrap(None, |s| s.element_expr(p))? {
                    Expansion::Elements(v) => {
                        let rw = self.take_rewrap();
                        v.into_iter()
                            .enumerate()
                            .map(|(k, e)| {
                                let e = if rw.get(k) == Some(&true) {
                                    self.sema.act_pack_expansion(e, loc)
                                } else {
                                    e
                                };
                                self.sema.ast.builder().template_arg_expr(e)
                            })
                            .collect()
                    }
                    Expansion::Unexpanded(e) => {
                        let x = self.sema.act_pack_expansion(e, loc);
                        vec![self.sema.ast.builder().template_arg_expr(x)]
                    }
                }
            }
            ArgView::TemplateExpansion { name, .. } => {
                match self.expand_pattern_rewrap(None, |s| s.template_name(name, loc))? {
                    Expansion::Elements(v) => {
                        let rw = self.take_rewrap();
                        v.into_iter()
                            .enumerate()
                            .map(|(k, n)| {
                                if rw.get(k) == Some(&true) {
                                    TemplateArg::template_expansion(n, None)
                                } else {
                                    TemplateArg::template(n)
                                }
                            })
                            .collect()
                    }
                    Expansion::Unexpanded(n) => vec![TemplateArg::template_expansion(n, None)],
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(r))
    }

    /// The qualifier of the argument template template parameter name `n`
    /// is replaced by, when that argument is a qualified template name
    /// not written with `template` (`X<int>::apply` kept as first written
    /// on a specialization, clang 22): clang 22 prints the substituted
    /// template-id with it (`'X<int>::apply<int>'`).
    pub(crate) fn ttp_arg_qualifier(
        &mut self,
        n: kiln_ast::ids::TemplateNameId,
        loc: Loc,
    ) -> Option<NnsId> {
        let TemplateName::TemplateTemplateParm { depth, index, .. } =
            self.sema.ast.types.template_name_view(n)
        else {
            return None;
        };
        let ArgLookup::Arg(a) = self.lookup(depth as u16, index as u16) else { return None };
        let ArgView::Template(t) = self.element(a, None, loc).ok()?.view() else { return None };
        match self.sema.ast.types.template_name_view(t) {
            TemplateName::Qualified { nns, .. } => Some(nns),
            _ => None,
        }
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
                        // clang prints a substituted template template
                        // parameter through its replacement's declaration
                        // (`apply<int>`, not `template apply<int>`): the
                        // qualifier as written belongs to the argument.
                        ArgView::Template(t) => {
                            Ok(match self.sema.ast.types.template_name_view(t) {
                                TemplateName::Qualified { underlying, .. } => underlying,
                                _ => t,
                            })
                        }
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
            // `[: r :]<args>` with a dependent `r` (P2996 [temp.names]/1):
            // the template the substituted operand designates.
            TemplateName::Splice { expr, .. } => {
                let e = self.expr(expr)?;
                self.sema.splice_template_name(e, true).ok_or(SubstFail)
            }
            TemplateName::SpliceProfile(p) => {
                let e = self.sema.ast.types.profile_expr(p).ok_or(SubstFail)?;
                let e = self.expr(e)?;
                self.sema.splice_template_name(e, true).ok_or(SubstFail)
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
                if let Some((nns, name)) = self.dependent_member_nns(d, loc)? {
                    return Ok(self
                        .sema
                        .ast
                        .types
                        .template_name(TemplateName::DependentIdentifier { nns, name }));
                }
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
            // `A::y` in the pattern of `A` names the specialization being
            // instantiated: `A<int>::` (clang's `TransformNestedNameSpecifierLoc`
            // over the injected-class-name; printed).
            if let Nns::Type { prefix, ty, template_kw } = self.sema.ast.types.nns_view(n)
                && let kiln_ast::types::TypeView::InjectedClassName { pattern, .. }
                | kiln_ast::types::TypeView::Record(pattern) = self.sema.ast.types.view(ty.ty())
                && let Some(&(_, i)) = self.pairs.iter().rev().find(|p| p.0 == pattern)
            {
                let ty = self.sema.tag_type(i);
                return Ok(self.sema.ast.types.nns(Nns::Type { prefix, ty, template_kw }));
            }
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
                let Some(orig) = prefix else { return self.unimplemented("dependent name", loc) };
                let p = self.nns(orig, loc)?;
                if self.nns_still_dependent(p) {
                    // Substituted with dependent arguments: `X::name::`
                    // stays dependent ([temp.dep.type]/6), rebuilt on
                    // the substituted prefix (WP-121).
                    return Ok(self.sema.ast.types.nns(Nns::Identifier { prefix: Some(p), name }));
                }
                let ctx = self.nns_context(p, self.written(WrittenKey::Nns(orig), loc))?;
                let nl = self.written(WrittenKey::Nns(n), loc);
                let Some((c, spelled)) = self.sema.act_on_nested_name_spelled(Some(ctx), name, nl)
                else {
                    return Err(SubstFail);
                };
                return Ok(self.sema.nns_component(Some(p), c, spelled));
            }
            // `[: r :]::` (spec/reflection.md §5): the scope the
            // substituted operand designates.
            Nns::Splice(e) => {
                let e = self.expr(e)?;
                let nl = self.written(WrittenKey::Nns(n), loc);
                let (_, m) = self.sema.act_on_nested_name_splice(e, nl).ok_or(SubstFail)?;
                return Ok(m);
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

    /// Where the pattern's dependent qualifier `n` begins (§5.8): the
    /// written location of its first dependent component (`T` in
    /// `T::B::`), nearest to `loc`. A declaration keeps no location for
    /// its qualifier, so a using-declaration reports a qualifier error
    /// here, at the start of the nested-name-specifier as clang does
    /// (`QualifierLoc.getBeginLoc()`). A non-dependent leading prefix
    /// (`N::X<T>::`) is not recorded: the location is that of `X<T>::`.
    pub(crate) fn qualifier_begin(&self, n: NnsId, loc: Loc) -> Loc {
        let mut first = n;
        let mut cur = Some(n);
        while let Some(c) = cur {
            if self.sema.ast.types.nns_flags(c).intersects(TypeFlags::DEPENDENT_INSTANTIATION) {
                first = c;
            }
            cur = self.sema.ast.types.nns_prefix(c);
        }
        self.written(WrittenKey::Nns(first), loc)
    }

    /// True if the substituted specifier `n` is still dependent (its
    /// arguments were dependent: an alias or member template named in
    /// another template), so names after it are looked up later.
    pub(crate) fn nns_still_dependent(&self, n: NnsId) -> bool {
        self.sema.ast.types.nns_flags(n).intersects(TypeFlags::DEPENDENT_INSTANTIATION)
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

    /// Whether member `d` of a class sits in a dependent class this
    /// substitution does not instantiate and whose levels it does not
    /// keep ([`Substituter::find_instantiated_decl`] has no member to
    /// find for it).
    pub(crate) fn class_not_instantiated(&mut self, d: DeclId, loc: Loc) -> SubstResult<bool> {
        let node = *self.sema.ast.decls.get(d);
        if !node.flags().contains(DeclFlags::DEPENDENT_CTX) || self.pairs.iter().any(|p| p.0 == d) {
            return Ok(false);
        }
        let Some(parent) = node.ctx() else { return Ok(false) };
        let inst_parent = self.find_instantiated_decl(parent, loc)?;
        Ok(inst_parent == parent && !self.levels_retained(self.sema.ctx_template_levels(parent)))
    }

    /// A parameter of a function type written inside the pattern
    /// (`void (*)(T a, decltype(a))`, never adopted by a function), named
    /// in a later part of that function type: a parameter with the
    /// substituted type, made once per substitution (clang
    /// `TransformFunctionTypeParams` instantiating the `ParmVarDecl`s).
    /// It keeps its index and function-scope depth for the mangling of
    /// a still-dependent reference.
    fn nested_prototype_param(&mut self, d: DeclId, loc: Loc) -> SubstResult<DeclId> {
        let node = *self.sema.ast.decls.get(d);
        let Some(pd) = self.sema.ast.decls.param(d).copied() else { return Ok(d) };
        let t = pd.original_ty.unwrap_or_else(|| self.decl_ty(d));
        let ty = self.ty(t, loc)?;
        let ctx = self.sema.ctx;
        let flags = pd.flags & !kiln_ast::decl::ParamData::DEFAULT_ARG_PENDING;
        let np =
            self.sema.builder().prototype_param(ctx, node.name(), node.loc(), ty, pd.index, flags);
        self.sema.unowned_params.insert(np);
        self.locals.insert(d, DeclOrPack::Decl(np));
        Ok(np)
    }

    /// The declaration of the instantiation that pattern declaration `d`
    /// became (clang's `FindInstantiatedDecl`, §5.6): a local of the
    /// pattern, a member of an instantiated class, or `d` itself when it
    /// is not in a dependent context.
    pub fn find_instantiated_decl(&mut self, d: DeclId, loc: Loc) -> SubstResult<DeclId> {
        if let Some(&x) = self.locals.get(&d) {
            return match x {
                DeclOrPack::Decl(x) => {
                    if self.lambda_bodies == 0 && self.failed_inits.contains(&x) {
                        self.failed_init_use = true;
                    }
                    Ok(x)
                }
                DeclOrPack::Pack(l) => {
                    let n = u32::try_from(self.sema.ast.lists.len(l)).unwrap_or(u32::MAX);
                    let i = self.pack_reached(
                        n,
                        |s| crate::templ::pack::PackName {
                            name: s.sema.decl_name(d),
                            depth: None,
                            outer: false,
                            partial: false,
                        },
                        loc,
                    )?;
                    self.sema.ast.lists.at::<DeclId>(l, i as usize).ok_or(SubstFail)
                }
            };
        }
        // A local whose declaration statement failed: an invalid use,
        // already diagnosed ([temp.inst]/3; §5.6).
        if self.failed_locals.contains(&d) {
            return Err(SubstFail);
        }
        let node = *self.sema.ast.decls.get(d);
        if !node.flags().contains(DeclFlags::DEPENDENT_CTX) {
            return Ok(d);
        }
        if let Some(&(_, i)) = self.pairs.iter().rev().find(|p| p.0 == d) {
            return Ok(i);
        }
        if node.kind() == DeclKind::ParmVar && self.sema.unowned_params.contains(&d) {
            return self.nested_prototype_param(d, loc);
        }
        if matches!(
            node.kind(),
            DeclKind::TemplateTypeParm
                | DeclKind::NonTypeTemplateParm
                | DeclKind::TemplateTemplateParm
        ) {
            return Ok(d);
        }
        // An init-capture variable of a generic lambda whose closure this
        // substitution does not rebuild (rebuilt ones are in `locals`):
        // a specialization of its call operator, or of a lambda nested in
        // one, shares the closure and names the variable itself (clang
        // `addInstantiatedCapturesToScope`, [expr.prim.lambda.capture]/6).
        if self.sema.init_capture_owner(d).is_some() {
            return Ok(d);
        }
        // A member of a class: find the instantiated class, then its
        // member instantiated from `d`.
        let Some(parent) = node.ctx() else { return Ok(d) };
        // The pattern class of a class template is the specialization
        // being instantiated (its template is not itself a context).
        let mut inst_parent = self.find_instantiated_decl(parent, loc)?;
        if inst_parent == parent {
            // The parent depends only on levels this substitution keeps
            // (an alias or member template of a class template named
            // inside its pattern): the member stays (clang's
            // `FindInstantiatedDecl`, `ParentDependsOnArgs`).
            if self.levels_retained(self.sema.ctx_template_levels(parent)) {
                return Ok(d);
            }
            // A parameter of the pattern named in its own signature
            // (`decltype(t)`) while only the function type is substituted
            // (deduction against a target type, [temp.deduct.funcaddr]):
            // a substituted parameter, as clang's
            // `TransformFunctionTypeParams` makes.
            if node.kind() == DeclKind::ParmVar {
                return self.nested_prototype_param(d, loc);
            }
            // A member of a class template pattern this substitution does
            // not instantiate (named in the deduction guide of one of its
            // constructor templates, [over.match.class.deduct]/1.2, or of
            // an alias guide built from one, /3): the specialization the
            // substituted `X<P...>` names (clang's `FindInstantiatedDecl`
            // `ParentDC` lookup of the instantiated context).
            inst_parent = self.pattern_specialization(parent, loc)?;
        }
        if self.sema.kind_of(inst_parent) == DeclKind::CXXRecord {
            let ty = self.sema.tag_type(inst_parent);
            let _ = self.sema.require_complete_silent(ty, loc);
        } else {
            self.sema.require_member_enum(inst_parent, loc);
        }
        if let Some(m) = self.find_member_of(inst_parent, d) {
            return Ok(m);
        }
        // The members of an instantiated member class are recorded on the
        // redeclaration that was defined: a pattern class declared before
        // its definition (`struct In; struct In { ... };`) has two
        // instantiated declarations, and the one the pattern's definition
        // maps to (through the enclosing class's member map) is not the
        // one defined (WP-582; clang's `FindInstantiatedDecl` looks the
        // member up in the instantiated context's definition).
        if let Some(def) = self.sema.ast.definition(inst_parent).filter(|&x| x != inst_parent)
            && let Some(m) = self.find_member_of(def, d)
        {
            return Ok(m);
        }
        // A member of the pattern that an explicitly specialized member
        // ([temp.expl.spec]/7) does not have: clang looks the name up
        // again in the specialization.
        let first_parent = self.sema.ast.first(inst_parent);
        if self
            .sema
            .ast
            .member_spec_info(first_parent)
            .is_some_and(|(_, m)| m.kind == kiln_ast::templates::SpecKind::ExplicitSpecialization)
        {
            let name = self.sema.decl_name_str(node.name());
            let e = self
                .sema
                .diag(kiln_diag::DiagId::sema_no_member, loc)
                .arg(name)
                .arg(self.ctx_arg(inst_parent));
            self.sema.emit(e);
        }
        // A member not instantiated (an invalid member of the
        // specialization): already diagnosed.
        Err(SubstFail)
    }
}

impl Substituter<'_, '_, '_> {
    /// The member of instantiated class or enumeration `parent`
    /// instantiated from pattern member `d`, or from its first
    /// declaration: an out-of-line redeclaration of the member
    /// (`T C<T>::z = ...;` found by lookup in a later out-of-line
    /// initializer) names the member instantiated from its first
    /// declaration.
    fn find_member_of(&self, parent: DeclId, d: DeclId) -> Option<DeclId> {
        let templates = &self.sema.ast.templates;
        templates.find_member(parent, d).or_else(|| {
            let first = self.sema.ast.first(d);
            if first == d { None } else { templates.find_member(parent, first) }
        })
    }

    /// The instantiation of the pattern of a class template or class
    /// template partial specialization `parent` that this substitution's
    /// arguments name: `X<P...>` substituted, made complete. Fails (a
    /// substitution failure) when the substituted type is still dependent
    /// or names no class.
    fn pattern_specialization(&mut self, parent: DeclId, loc: Loc) -> SubstResult<DeclId> {
        if !self.is_class_pattern(parent) {
            return self.unimplemented("reference to a member of a dependent context", loc);
        }
        let injected = self.sema.this_class_type(parent);
        let t = self.ty(injected, loc)?;
        if self.sema.ast.types.is_dependent(t) {
            return self.unimplemented("reference to a member of a dependent context", loc);
        }
        match self.sema.ast.types.as_record(t) {
            Some(r) if r != parent => Ok(r),
            _ => Err(SubstFail),
        }
    }

    /// Whether `c` is the pattern of a class template or of a class
    /// template partial specialization.
    pub(crate) fn is_class_pattern(&self, c: DeclId) -> bool {
        let first = self.sema.ast.first(c);
        self.sema.kind_of(c) == DeclKind::CXXRecord
            && (self.sema.ast.described_template(first).is_some()
                || self.sema.ast.partial_spec_info(first).is_some())
    }

    /// For member `d` of a class template pattern this substitution
    /// does not instantiate, when the substituted `X<A...>` is still
    /// dependent (the guide of an alias template built from a
    /// constructor template's guide, [over.match.class.deduct]/3): the
    /// qualifier `X<A...>::` and `d`'s identifier, to name the member as
    /// a member of an unknown specialization ([temp.dep.type]/6), looked
    /// up again when the guide itself is substituted. `None` otherwise.
    pub(crate) fn dependent_member_nns(
        &mut self,
        d: DeclId,
        loc: Loc,
    ) -> SubstResult<Option<(NnsId, Symbol)>> {
        let node = *self.sema.ast.decls.get(d);
        if !node.flags().contains(DeclFlags::DEPENDENT_CTX)
            || self.locals.contains_key(&d)
            || self.pairs.iter().any(|p| p.0 == d)
        {
            return Ok(None);
        }
        let (Some(parent), Some(name)) = (node.ctx(), node.name().as_ident()) else {
            return Ok(None);
        };
        if !self.is_class_pattern(parent)
            || self.find_instantiated_decl(parent, loc)? != parent
            || self.levels_retained(self.sema.ctx_template_levels(parent))
        {
            return Ok(None);
        }
        let injected = self.sema.this_class_type(parent);
        let t = self.ty(injected, loc)?;
        if !self.sema.ast.types.is_dependent(t) {
            return Ok(None);
        }
        let nns = self.sema.ast.types.nns(Nns::Type { prefix: None, ty: t, template_kw: false });
        Ok(Some((nns, name)))
    }

    /// Whether `d` is (nested in) a member of a class template pattern
    /// whose substitution is still dependent ([`Self::dependent_member_nns`]
    /// applies to it or to an enclosing class).
    pub(crate) fn in_dependent_specialization(&mut self, d: DeclId, loc: Loc) -> SubstResult<bool> {
        let mut x = d;
        for _ in 0..64 {
            let Some(parent) = self.sema.ast.decls.get(x).ctx() else { return Ok(false) };
            if self.sema.kind_of(parent) != DeclKind::CXXRecord {
                return Ok(false);
            }
            if self.is_class_pattern(parent) {
                return Ok(self.dependent_member_nns(x, loc)?.is_some());
            }
            x = parent;
        }
        Ok(false)
    }

    /// True if none of the outermost `n` levels is substituted (a level of
    /// undeduced arguments keeps its parameters, as a class template's
    /// does in the guide of a constructor template, WP-134).
    pub(crate) fn levels_retained(&self, n: u16) -> bool {
        (0..n).all(|k| {
            self.args.level(k).is_none_or(|l| {
                l.mode == crate::templ::args::LevelMode::Retain
                    || self
                        .sema
                        .ast
                        .types
                        .args(l.args)
                        .iter()
                        .all(|a| matches!(a.view(), ArgView::Null))
            })
        })
    }
}

impl Sema<'_, '_> {
    /// The number of template parameter levels context `c` is inside
    /// (its own, for a template pattern or partial specialization).
    pub(crate) fn ctx_template_levels(&self, c: DeclId) -> u16 {
        let mut cur = Some(c);
        let mut guard = 0;
        while let Some(d) = cur {
            if let Some(t) = self.ast.described_template(d) {
                return self.template_param_depth(t) + 1;
            }
            if let Some((_, p)) = self.ast.partial_spec_info(d) {
                return self.ast.templates.param_list(p.params).depth + 1;
            }
            guard += 1;
            if guard > 1024 {
                break;
            }
            cur = self.ast.decls.get(d).ctx();
        }
        0
    }

    /// Whether template-argument flags `f` make a node dependent.
    pub(crate) fn dependent_flags(f: TypeFlags) -> bool {
        f.intersects(TypeFlags::DEPENDENT_INSTANTIATION | TypeFlags::UNEXPANDED_PACK)
    }
}
