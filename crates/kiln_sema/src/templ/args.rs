//! Template argument checking and conversion ([temp.arg]; spec/sema-
//! templates.md §3): kind checks, converted constant expressions for
//! non-type parameters, default arguments, packs, and the multi-level
//! argument lists the substituter consults (§5.1).

use kiln_ast::decl::DeclKind;
use kiln_ast::ids::{ArgListId, TemplateNameId};
use kiln_ast::types::{ArgView, TemplateArg, TemplateName, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::EvalContext;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::conv::{self, ConvCtx, SecondStep, StandardConversion};
use crate::overload::Ics;
use crate::templ::stack::FrameKind;
use crate::templ::subst::Substituter;

/// The converted arguments of a template-id (§3.1): `sugared` keeps the
/// arguments as written (types with their sugar; non-type arguments as
/// values), `canonical` is its canonical list (the specialization key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConvertedArgs {
    /// One argument per parameter (a trailing pack is one `Pack`).
    pub sugared: ArgListId,
    /// `types.canonical_args(sugared)`.
    pub canonical: ArgListId,
}

/// What a template parameter level does under substitution (§5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelMode {
    /// The level's parameters are replaced by `args`.
    Substitute,
    /// The level's parameters stay parameters, renumbered to the depth
    /// they have once the substituted levels are gone (a member template
    /// of a class template specialization).
    Retain,
}

/// One template parameter level (§5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    /// The level's arguments ([`ArgListId::EMPTY`] for a retained level).
    pub args: ArgListId,
    /// The template (or specialization) the level belongs to; the
    /// `associated` decl of the `SubstTemplateTypeParmType`s it produces.
    pub assoc: DeclId,
    /// Substitute or retain.
    pub mode: LevelMode,
}

/// What a parameter at `(depth, index)` becomes (§5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgLookup {
    /// Its argument.
    Arg(TemplateArg),
    /// It stays a parameter, at this new depth.
    Retain(u16),
    /// The level is substituted but has no argument at `index` (an
    /// error already diagnosed while checking the arguments).
    Missing,
}

/// The template arguments of every enclosing level, outermost first,
/// indexed by depth (clang's `MultiLevelTemplateArgumentList`, §5.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MultiLevelArgs {
    /// `levels[d]` is depth `d`.
    pub levels: Vec<Level>,
}

impl MultiLevelArgs {
    /// No levels: substitution is the identity.
    pub fn new() -> Self {
        MultiLevelArgs::default()
    }

    /// A single substituted level at depth 0.
    pub fn single(args: ArgListId, assoc: DeclId) -> Self {
        MultiLevelArgs { levels: vec![Level { args, assoc, mode: LevelMode::Substitute }] }
    }

    /// Adds the next (inner) level.
    pub fn push(&mut self, args: ArgListId, assoc: DeclId, mode: LevelMode) {
        self.levels.push(Level { args, assoc, mode });
    }

    /// The number of levels.
    pub fn depth(&self) -> usize {
        self.levels.len()
    }

    /// The number of substituted levels.
    pub fn substituted(&self) -> usize {
        self.levels.iter().filter(|l| l.mode == LevelMode::Substitute).count()
    }

    /// The level at `depth`, if any.
    pub fn level(&self, depth: u16) -> Option<&Level> {
        self.levels.get(usize::from(depth))
    }

    /// The new depth of a parameter at `depth` that is not substituted:
    /// `depth` minus the substituted levels outside it.
    pub fn retained_depth(&self, depth: u16) -> u16 {
        let k = self
            .levels
            .iter()
            .take(usize::from(depth))
            .filter(|l| l.mode == LevelMode::Substitute)
            .count();
        depth.saturating_sub(u16::try_from(k).unwrap_or(u16::MAX))
    }

    /// What the parameter at `(depth, index)` becomes, reading the
    /// argument list through `args`.
    pub fn lookup(
        &self,
        depth: u16,
        index: u16,
        args: impl Fn(ArgListId) -> Vec<TemplateArg>,
    ) -> ArgLookup {
        match self.level(depth) {
            Some(l) if l.mode == LevelMode::Substitute => {
                match args(l.args).get(usize::from(index)) {
                    // Not yet deduced (explicit arguments substituted before
                    // deduction, §6.1 step 1): the parameter stays.
                    Some(a) if matches!(a.view(), ArgView::Null) => {
                        ArgLookup::Retain(self.retained_depth(depth))
                    }
                    Some(&a) => ArgLookup::Arg(a),
                    None => ArgLookup::Missing,
                }
            }
            _ => ArgLookup::Retain(self.retained_depth(depth)),
        }
    }
}

/// The `%select` index of a template's kind in
/// `sema_template_arg_list_different_arity`.
fn template_kind_select(k: DeclKind) -> u32 {
    match k {
        DeclKind::ClassTemplate => 0,
        DeclKind::FunctionTemplate => 1,
        DeclKind::VarTemplate => 2,
        DeclKind::TypeAliasTemplate => 3,
        DeclKind::TemplateTemplateParm => 4,
        DeclKind::Concept => 5,
        _ => 6,
    }
}

impl Sema<'_, '_> {
    /// The parameters of template `tmpl`, in order.
    pub(crate) fn template_params(&self, tmpl: DeclId) -> Vec<DeclId> {
        let Some(info) = self.ast.template_info(tmpl).copied() else { return Vec::new() };
        let l = *self.ast.templates.param_list(info.params);
        self.ast.lists.iter::<DeclId>(l.params).collect()
    }

    /// The depth of `tmpl`'s parameters.
    pub(crate) fn template_param_depth(&self, tmpl: DeclId) -> u16 {
        self.ast.template_info(tmpl).map_or(0, |i| self.ast.templates.param_list(i.params).depth)
    }

    /// Checks the written arguments of the template-id `tmpl<written>`
    /// against its parameters and converts them ([temp.arg]/1, §3.2):
    /// arity, argument kinds, non-type arguments as converted constant
    /// expressions ([temp.arg.nontype]/2), default arguments
    /// ([temp.arg.general]/9) and the trailing pack. `arg_locs[i]` is
    /// where written argument `i` starts (diagnostics point there, as
    /// clang's do); `loc` is the template name. `Err` after a
    /// diagnostic.
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub fn check_template_args(
        &mut self,
        tmpl: DeclId,
        written: &[TemplateArg],
        arg_locs: &[Loc],
        loc: Loc,
    ) -> Result<ConvertedArgs, ()> {
        let saved = self.inst.template_id_loc.replace(loc);
        let r = self.check_template_args_at(tmpl, written, arg_locs, loc);
        self.inst.template_id_loc = saved;
        r
    }

    fn check_template_args_at(
        &mut self,
        tmpl: DeclId,
        written: &[TemplateArg],
        arg_locs: &[Loc],
        loc: Loc,
    ) -> Result<ConvertedArgs, ()> {
        let head = self.head_template(tmpl);
        let tmpl = self.ast.first(tmpl);
        let params = self.template_params(head);
        let depth = self.template_param_depth(head);
        let mut out: Vec<TemplateArg> = Vec::with_capacity(params.len());
        let arg_loc = |i: usize| arg_locs.get(i).copied().unwrap_or(loc);
        let mut wi = 0usize;
        let mut open_expansion = false;
        for (pi, &p) in params.iter().enumerate() {
            if let Some(tys) = self.expanded_pack_types(p) {
                // An expanded parameter pack takes one argument per
                // element type ([temp.variadic]/5); a pack expansion
                // argument takes the rest, checked once substituted.
                let mut elems = Vec::new();
                let mut open = false;
                let mut taken = 0usize;
                while wi < written.len() && (open || taken < tys.len()) {
                    open |= self.is_pack_expansion_arg(written[wi]);
                    let at = arg_loc(wi);
                    let ty = tys.get(taken).copied().filter(|_| !open);
                    taken += 1;
                    // The first failing argument ends the check (clang's
                    // `CheckTemplateArgumentList`).
                    elems.push(self.check_expanded_pack_arg(
                        tmpl,
                        depth,
                        p,
                        written[wi],
                        at,
                        &out,
                        ty,
                    )?);
                    wi += 1;
                }
                open_expansion |= open;
                if !open && taken < tys.len() {
                    self.diag_template_arity(tmpl, false, loc);
                    return Err(());
                }
                let l = self.ast.types.arg_list(&elems);
                out.push(TemplateArg::pack(l));
                continue;
            }
            let pack = self.ast.template_parm(p).is_some_and(|x| x.pack);
            if pack {
                let mut elems = Vec::new();
                while wi < written.len() {
                    elems.push(self.check_template_arg(
                        tmpl,
                        depth,
                        p,
                        written[wi],
                        arg_loc(wi),
                        &out,
                    )?);
                    wi += 1;
                }
                let l = self.ast.types.arg_list(&elems);
                out.push(TemplateArg::pack(l));
                continue;
            }
            if wi < written.len() {
                let a = self.check_template_arg(tmpl, depth, p, written[wi], arg_loc(wi), &out)?;
                out.push(a);
                wi += 1;
                continue;
            }
            let _ = pi;
            // After a pack expansion argument the remaining parameters'
            // arguments are known only once it is substituted.
            if open_expansion {
                break;
            }
            let Some(def) = self.template_default(p) else {
                self.diag_template_arity(tmpl, false, loc);
                return Err(());
            };
            let a = self.default_template_arg(tmpl, depth, p, def, loc, &out)?;
            out.push(a);
        }
        if wi < written.len() {
            self.diag_template_arity(tmpl, true, loc);
            return Err(());
        }
        let sugared = self.ast.types.arg_list(&out);
        let canonical = self.ast.types.canonical_args(sugared);
        Ok(ConvertedArgs { sugared, canonical })
    }

    /// The element types of expanded non-type parameter pack `p`
    /// ([temp.variadic]/5; §3.2), or `None` for any other parameter.
    pub(crate) fn expanded_pack_types(&self, p: DeclId) -> Option<Vec<QualType>> {
        let l = *self.templ.expanded_packs.get(&p)?;
        Some(
            self.ast
                .types
                .args(l)
                .iter()
                .map(|a| match a.view() {
                    ArgView::Type(t) => t,
                    _ => QualType::ERROR,
                })
                .collect(),
        )
    }

    /// Checks argument `a` for an element of expanded parameter pack `p`
    /// whose type is `ty` (`None` for an argument whose element is not
    /// known yet: after a pack expansion argument, or past the end).
    #[allow(clippy::too_many_arguments, reason = "check_template_arg plus the element type")]
    pub(crate) fn check_expanded_pack_arg(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        a: TemplateArg,
        at: Loc,
        prior: &[TemplateArg],
        ty: Option<QualType>,
    ) -> Result<TemplateArg, ()> {
        let Some(ty) = ty else {
            return match a.view() {
                ArgView::Expr { expr, .. } if !self.is_err_expr(expr) => Ok(a),
                _ => self.check_template_arg(tmpl, depth, p, a, at, prior),
            };
        };
        let saved = self.templ.nttp_elem.replace(ty);
        let r = self.check_template_arg(tmpl, depth, p, a, at, prior);
        self.templ.nttp_elem = saved;
        r
    }

    /// The definition-time check of the class template-id `tmpl<args>`
    /// with dependent arguments (clang's `CheckTemplateArgumentList`,
    /// which runs on dependent arguments too; §3.2): the arity, and the
    /// kind of each argument against its parameter (a type for a type
    /// parameter, a template for a template template parameter, whose
    /// parameters are matched as [temp.arg.template]/3 says, an
    /// expression for a non-type parameter). Non-type arguments are
    /// converted once substituted. A dependent `T::x` for a type
    /// parameter is "did you forget 'typename'?" and the check goes on,
    /// as clang's does; any other failure ends it. A template-id with a
    /// pack expansion argument is checked only for an expanded pack's
    /// arity. Consumes the pending argument locations. `Err` after a
    /// diagnostic.
    pub(crate) fn check_dependent_template_args(
        &mut self,
        tmpl: DeclId,
        args: &[TemplateArg],
        loc: Loc,
    ) -> Result<(), ()> {
        let locs = std::mem::take(&mut self.templ.arg_locs);
        self.check_expanded_pack_arity(tmpl, args, loc)?;
        let head = self.head_template(tmpl);
        let params = self.template_params(head);
        if args.iter().any(|&a| self.is_pack_expansion_arg(a))
            || params.iter().any(|p| self.templ.expanded_packs.contains_key(p))
        {
            return Ok(());
        }
        let depth = self.template_param_depth(head);
        let tmpl = self.ast.first(tmpl);
        let arg_loc = |i: usize| locs.get(i).copied().unwrap_or(loc);
        let mut prior: Vec<TemplateArg> = Vec::with_capacity(params.len());
        let mut failed = false;
        let mut wi = 0usize;
        for &p in &params {
            if self.ast.template_parm(p).is_some_and(|x| x.pack) {
                while wi < args.len() {
                    failed |=
                        self.check_dependent_arg(tmpl, depth, p, args[wi], arg_loc(wi), &prior)?;
                    wi += 1;
                }
                break;
            }
            if wi >= args.len() {
                // The rest have defaults, checked once substituted.
                if self.template_default(p).is_none() {
                    self.diag_template_arity(tmpl, false, loc);
                    return Err(());
                }
                break;
            }
            failed |= self.check_dependent_arg(tmpl, depth, p, args[wi], arg_loc(wi), &prior)?;
            prior.push(args[wi]);
            wi += 1;
        }
        if wi < args.len() {
            self.diag_template_arity(tmpl, true, loc);
            return Err(());
        }
        if failed { Err(()) } else { Ok(()) }
    }

    /// One argument of [`Self::check_dependent_template_args`]: `Ok(true)`
    /// after the recoverable missing-`typename` error, `Err` after any
    /// other.
    fn check_dependent_arg(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        a: TemplateArg,
        at: Loc,
        prior: &[TemplateArg],
    ) -> Result<bool, ()> {
        match (self.kind_of(p), a.view()) {
            (DeclKind::TemplateTypeParm, ArgView::Expr { expr, .. })
                if self.ast.exprs.get(expr).kind()
                    == kiln_ast::expr::ExprKind::DependentScopeDeclRefExpr =>
            {
                let d = self.diag(DiagId::sema_template_arg_must_be_type_suggest, at);
                let d = self.note_param_here(d, p);
                self.emit(d);
                Ok(true)
            }
            // Non-type arguments are converted once substituted.
            (DeclKind::NonTypeTemplateParm, ArgView::Expr { .. } | ArgView::Value { .. }) => {
                Ok(false)
            }
            (_, ArgView::Type(_) | ArgView::Template(_) | ArgView::Expr { .. }) => {
                self.check_template_arg(tmpl, depth, p, a, at, prior).map(|_| false)
            }
            // A profiled or substituted value is checked when the
            // template-id is.
            _ => Ok(false),
        }
    }

    /// The arity check of the dependent template-id `tmpl<args>` whose
    /// parameters include an expanded parameter pack (a member template
    /// `template<Ts... a>` of a specialization, §3.2): clang's
    /// `CheckTemplateArgumentList` checks it even with dependent
    /// arguments. An expanded pack of `n` types takes at most `n`
    /// arguments, a pack expansion counting as one; it may take fewer
    /// once a pack expansion was among them. `O<>::A<a...>` and
    /// `O<int>::A<1, a...>` are "too many template arguments". `Err`
    /// after the diagnostic.
    pub(crate) fn check_expanded_pack_arity(
        &self,
        tmpl: DeclId,
        args: &[TemplateArg],
        loc: Loc,
    ) -> Result<(), ()> {
        let params = self.template_params(self.head_template(tmpl));
        if !params.iter().any(|p| self.templ.expanded_packs.contains_key(p)) {
            return Ok(());
        }
        let mut wi = 0usize;
        for &p in &params {
            if let Some(tys) = self.expanded_pack_types(p) {
                let mut open = false;
                let mut taken = 0usize;
                while wi < args.len() && taken < tys.len() {
                    open |= self.is_pack_expansion_arg(args[wi]);
                    taken += 1;
                    wi += 1;
                }
                if !open && taken < tys.len() {
                    self.diag_template_arity(tmpl, false, loc);
                    return Err(());
                }
                continue;
            }
            // A parameter pack takes the rest; a pack expansion for a
            // single parameter leaves the rest unknown; a missing
            // argument is the full check's (defaults).
            if self.ast.template_parm(p).is_some_and(|x| x.pack)
                || wi >= args.len()
                || self.is_pack_expansion_arg(args[wi])
            {
                return Ok(());
            }
            wi += 1;
        }
        if wi < args.len() {
            self.diag_template_arity(tmpl, true, loc);
            return Err(());
        }
        Ok(())
    }

    pub(crate) fn diag_template_arity(&self, tmpl: DeclId, too_many: bool, loc: Loc) {
        // clang prints the `TemplateDecl` unqualified: `A`, also for a
        // member template of a specialization (`O<int, char>::A`).
        let name = self.unqualified_decl_name(tmpl);
        // The most recent (non-friend) declaration, as name lookup finds
        // it: a redeclaration after the definition, not the definition
        // (clang notes the found `TemplateDecl`).
        let at = self
            .pattern_of(self.visible_template(tmpl))
            .and_then(|p| {
                self.ast.redecls_of(p).find(|&x| {
                    self.ast.decls.get(x).misc().friend() == kiln_ast::decl::FriendKind::None
                        && self.ast.described_template(x).is_some()
                })
            })
            // An alias template is located at its `using` (clang's
            // `TypeAliasTemplateDecl` location).
            .map_or_else(
                || self.ast.decls.get(tmpl).loc(),
                |p| {
                    self.templ
                        .alias_using_loc
                        .get(&p)
                        .copied()
                        .unwrap_or_else(|| self.ast.decls.get(p).loc())
                },
            );
        let d = self
            .diag(DiagId::sema_template_arg_list_different_arity, loc)
            .arg(u32::from(too_many))
            .arg(template_kind_select(self.kind_of(tmpl)))
            .arg(name)
            .note(DiagId::note_template_decl_here, Span::empty(at));
        self.emit(d);
    }

    /// The template a template argument names: also a template template
    /// parameter's declaration (`B2<TT>` inside `TT`'s template).
    fn arg_template_decl(&self, name: TemplateNameId) -> Option<DeclId> {
        self.ast.types.template_decl(name).or_else(|| {
            match self.ast.types.template_name_view(name) {
                TemplateName::TemplateTemplateParm { decl, .. } => Some(decl),
                _ => None,
            }
        })
    }

    /// A concept named alone as a template-argument before clang 22
    /// (which parses it as a concept-id with no arguments): "too few
    /// template arguments for concept 'C'" at `loc`; an erroneous
    /// expression argument.
    pub fn act_bare_concept_template_arg(&mut self, c: DeclId, loc: Loc) -> TemplateArg {
        self.diag_template_arity(c, false, loc);
        let e = self.recover_err(&[], loc);
        self.builder().template_arg_expr(e)
    }

    /// A variable template, concept (clang 22 parses either as a
    /// template name, P2841) or template template parameter named
    /// without arguments where a type is expected: "use of variable
    /// template 'v' requires template arguments" with the template's
    /// note (clang's `diagnoseMissingTemplateArguments`). False for any
    /// other template name.
    fn var_or_concept_arg_missing_args(&mut self, name: TemplateNameId, at: Loc) -> bool {
        let Some(d) = self.arg_template_decl(name) else { return false };
        let what = match self.kind_of(d) {
            DeclKind::VarTemplate => "variable template",
            DeclKind::Concept => "concept",
            DeclKind::TemplateTemplateParm => "template template parameter",
            _ => return false,
        };
        let qual = match self.ast.types.template_name_view(name) {
            TemplateName::Qualified { nns, .. } => Some(nns),
            _ => None,
        };
        self.missing_template_args(d, what, qual, at);
        true
    }

    /// "use of <what> 'name' requires template arguments" at `at` with
    /// "template is declared here" at template `d` (clang's
    /// `diagnoseMissingTemplateArguments`); the name is printed with
    /// qualifier `qual` as written.
    pub(crate) fn missing_template_args(
        &mut self,
        d: DeclId,
        what: &str,
        qual: Option<kiln_ast::ids::NnsId>,
        at: Loc,
    ) {
        let note = self.diag(DiagId::note_template_decl_here, self.ast.decls.get(d).loc());
        let q = qual.map(|n| self.nns_str(n)).unwrap_or_default();
        let msg = self
            .diag(DiagId::sema_template_requires_args, at)
            .arg(what.to_owned())
            .arg(format!("'{q}{}'", self.decl_name(d)))
            .with_note(note);
        self.emit(msg);
    }

    fn note_param_here(&self, d: kiln_diag::Diag, p: DeclId) -> kiln_diag::Diag {
        d.note(DiagId::note_template_param_here, Span::empty(self.ast.decls.get(p).loc()))
    }

    /// Checks and converts one written argument for parameter `p`
    /// (§3.2). `prior` holds the converted arguments of the parameters
    /// before `p` (a non-type parameter's type may name them).
    pub(crate) fn check_template_arg(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        a: TemplateArg,
        at: Loc,
        prior: &[TemplateArg],
    ) -> Result<TemplateArg, ()> {
        match (self.kind_of(p), a.view()) {
            (DeclKind::TemplateTypeParm, ArgView::Type(t)) => Ok(TemplateArg::ty(t)),
            (DeclKind::TemplateTypeParm, ArgView::Template(name))
                if self.var_or_concept_arg_missing_args(name, at) =>
            {
                Err(())
            }
            (DeclKind::TemplateTypeParm, ArgView::Template(name))
                if self.template_arg_missing_args(name, at) =>
            {
                Err(())
            }
            // An erroneous argument (`T<undeclared>`) was diagnosed when
            // it was formed (clang's `RecoveryExpr` makes the template-id
            // invalid without a second error).
            (DeclKind::TemplateTypeParm, ArgView::Expr { expr, .. }) if self.is_err_expr(expr) => {
                Err(())
            }
            (DeclKind::TemplateTypeParm, _) => {
                let d = self.diag(DiagId::sema_template_arg_must_be_type, at);
                let d = self.note_param_here(d, p);
                self.emit(d);
                Err(())
            }
            // A value-dependent argument (of an alias template named
            // with dependent arguments) is checked when it is substituted
            // ([temp.res.general]/6).
            (DeclKind::NonTypeTemplateParm, ArgView::Expr { expr, .. })
                if self.expr_is_value_dependent(expr) =>
            {
                Ok(a)
            }
            (DeclKind::NonTypeTemplateParm, ArgView::Expr { expr, .. }) => {
                self.convert_nttp_arg(tmpl, depth, p, expr, at, prior)
            }
            (DeclKind::NonTypeTemplateParm, ArgView::Value { .. }) => Ok(a),
            (DeclKind::NonTypeTemplateParm, _) => {
                let d = self.diag(DiagId::sema_template_arg_must_be_expr, at);
                let d = self.note_param_here(d, p);
                self.emit(d);
                Err(())
            }
            (DeclKind::TemplateTemplateParm, ArgView::Template(n)) => {
                match self.arg_template_decl(n).map(|d| self.kind_of(d)) {
                    Some(
                        DeclKind::ClassTemplate
                        | DeclKind::TypeAliasTemplate
                        | DeclKind::TemplateTemplateParm,
                    ) => {
                        let d = self.arg_template_decl(n);
                        if let Some(d) = d {
                            let args = self.prior_levels(tmpl, depth, prior);
                            if !self.check_template_template_arg(p, d, at, &args) {
                                return Err(());
                            }
                        }
                        Ok(a)
                    }
                    None => Ok(a),
                    // A variable template or concept (parsed as a
                    // template name by clang 22, P2841).
                    Some(DeclKind::VarTemplate | DeclKind::Concept) => {
                        if let Some(d) = self.arg_template_decl(n) {
                            self.reject_keyless_ttp_arg(d, at);
                        }
                        Err(())
                    }
                    Some(_) => {
                        self.error(DiagId::sema_template_arg_must_be_template, at);
                        Err(())
                    }
                }
            }
            // An erroneous argument was diagnosed when it was formed.
            (DeclKind::TemplateTemplateParm, ArgView::Expr { expr, .. })
                if self.is_err_expr(expr) =>
            {
                Err(())
            }
            (DeclKind::TemplateTemplateParm, _) => {
                self.error(DiagId::sema_template_arg_must_be_template, at);
                Err(())
            }
            _ => Err(()),
        }
    }

    /// The type of non-type parameter `p` given the arguments before it
    /// (its declared type with those substituted).
    fn nttp_type(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        prior: &[TemplateArg],
        at: Loc,
    ) -> Result<QualType, ()> {
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        if !self.ast.types.is_dependent(ty) {
            return Ok(ty);
        }
        let args = self.prior_levels(tmpl, depth, prior);
        self.in_template_context(tmpl, |s| {
            let mut s = Substituter::new(s, &args);
            s.final_subst = true;
            s.ty(ty, at).map_err(|_| ())
        })
    }

    /// Runs `f` with the declaration context of template `tmpl` current
    /// ([temp.dep.candidate], [temp.res.general]): the names its
    /// parameter types, function parameter types and default template
    /// arguments use were looked up from the template's scope, and a
    /// name substitution looks up again (`mk<T>()` with explicit
    /// arguments) is too, not from the scope of the use (`n::f<int>()`
    /// outside `n`; clang `SubstDefaultTemplateArgument` and
    /// `SubstituteExplicitTemplateArguments` enter the template's
    /// context), as for its signature and body. A template in a local
    /// scope keeps the use's scopes: they are the only ones that hold its
    /// function's names.
    pub(crate) fn in_template_context<R>(
        &mut self,
        tmpl: DeclId,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        if self.decl_in_local_scope(tmpl) {
            return f(self);
        }
        let ctx = self.ast.decls.get(tmpl).ctx().unwrap_or(self.ast.tu);
        self.in_inst_context(ctx, f)
    }

    /// The levels for substituting into parameter `depth` of `tmpl`:
    /// outer levels retained, this level's `prior` arguments.
    pub(crate) fn prior_levels(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        prior: &[TemplateArg],
    ) -> MultiLevelArgs {
        let mut m = Self::outer_levels(tmpl, depth);
        let l = self.ast.types.arg_list(prior);
        m.push(l, tmpl, LevelMode::Substitute);
        m
    }

    /// The levels enclosing `tmpl`'s own parameters (`depth` of them):
    /// the arguments of the class template specialization `tmpl` is a
    /// member of, else retained.
    pub(crate) fn outer_levels(tmpl: DeclId, depth: u16) -> MultiLevelArgs {
        let mut m = MultiLevelArgs::new();
        for _ in 0..depth {
            m.push(ArgListId::EMPTY, tmpl, LevelMode::Retain);
        }
        m
    }

    /// Runs `f`, a conversion of a converted constant expression, as part
    /// of that manifestly constant-evaluated expression: no run-time
    /// behavior warnings (`-Wnull-dereference` on the load).
    fn in_cce<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.constant_full_expr += 1;
        let r = f(self);
        self.constant_full_expr -= 1;
        r
    }

    /// `e` converted to the type of non-type parameter `p` as a
    /// converted constant expression ([temp.arg.nontype]/2) and
    /// evaluated: a `Value` argument.
    #[allow(clippy::too_many_lines)]
    fn convert_nttp_arg(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        e: ExprId,
        at: Loc,
        prior: &[TemplateArg],
    ) -> Result<TemplateArg, ()> {
        if self.is_err_expr(e) {
            return Err(());
        }
        // A type substituted from prior arguments is substituted under a
        // frame and checked at the parameter (clang `CheckTemplateArgument`:
        // `CheckNonTypeTemplateParameterType(NTTPType, NTTP->getLocation())`).
        // An element of an expanded pack has its own, known type.
        let elem = self.templ.nttp_elem.take();
        let declared =
            elem.unwrap_or_else(|| self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR));
        let substituted = elem.is_none() && self.ast.types.is_dependent(declared);
        if substituted {
            let poi = self.inst.template_id_loc.unwrap_or(at);
            if !self.push_inst_frame(FrameKind::PriorArgSubstitution, p, tmpl, poi) {
                return Err(());
            }
            let l = self.ast.types.arg_list(prior);
            self.set_frame_args(l);
        }
        let r = elem.map_or_else(|| self.nttp_type(tmpl, depth, p, prior, at), Ok).and_then(|t| {
            if substituted && !self.is_error_type(t) && !self.ast.types.is_dependent(t) {
                let ploc = self.ast.decls.get(p).loc();
                let _ = self.require_complete_silent(t, ploc);
                if !self.check_nttp_type(t, ploc) {
                    return Err(());
                }
            }
            Ok(t)
        });
        if substituted {
            self.pop_inst_frame();
        }
        let mut pty = r?;
        if self.is_error_type(pty) {
            return Err(());
        }
        if self.ast.types.is_dependent(pty) {
            // `template<class T, T V>` with a dependent `T`: converted
            // when substituted.
            return Ok(self.builder().template_arg_expr(e));
        }
        let ety = self.ast.exprs.get(e).ty();
        let placeholder = pty;
        if matches!(
            self.ast.types.canonical_view(pty),
            TypeView::Auto { keyword: kiln_ast::types::AutoKeyword::Auto, .. }
        ) {
            // `template<auto V>` ([temp.arg.nontype]/1): the type is
            // deduced from the argument (decayed, unqualified).
            let d = conv::decay(self, e);
            pty = conv::strip_cv(self, self.ast.exprs.get(d).ty());
        } else if self.is_ctad_placeholder(pty) {
            // `template<C V>` of a class template `C`
            // ([temp.param]/6.3, [dcl.type.class.deduct]/1): deduced as
            // for a variable `C x = e;`.
            let init = crate::init::Initializer::Expr(e);
            pty = self.ctad_for_initializer(pty, &init, false, at).ok_or(())?;
        } else if self.placeholder_keyword(pty).is_some() {
            // `auto&`, `auto*`, `decltype(auto)`, `const auto&`
            // ([temp.param]/6.3): deduced as for a variable
            // `T x = e;` ([dcl.type.auto.deduct]).
            if let Ok(t) = self.deduce_auto(pty, &[Some(e)], super::auto::AutoInit::Expr) {
                pty = t;
            } else {
                let d = self
                    .diag(DiagId::sema_nttp_deduction_failed, at)
                    .arg(self.decl_name(p))
                    .arg(self.ty_arg(pty))
                    .arg(self.ty_arg(ety))
                    .note(
                        DiagId::note_template_param_here,
                        Span::empty(self.ast.decls.get(p).loc()),
                    );
                self.emit(d);
                return Err(());
            }
        }
        // A deduced type is checked at the argument ([temp.param]/6.3),
        // naming the parameter (clang `CheckTemplateArgument`).
        if pty != placeholder
            && !self.check_nttp_type_noting(pty, at, Some(self.ast.decls.get(p).loc()))
        {
            return Err(());
        }
        let t = &self.ast.types;
        if t.as_record(pty).is_some() {
            // [temp.param]/8: a template parameter object (§6.10).
            return self.convert_class_nttp_arg(pty, e, at);
        }
        if t.is_reference(pty) {
            return self.convert_ref_nttp_arg(pty, e, at, Some(self.ast.decls.get(p).loc()));
        }
        // clang `CheckConvertedConstantExpression`: an argument with no
        // implicit conversion sequence to the parameter's type
        // ([temp.arg.nontype]/2, [expr.const]/12) is diagnosed as such.
        if ety != QualType::OVERLOAD && !t.is_dependent(ety) {
            let info = self.arg_info(e);
            let ics = self.try_implicit_conversion(&info, pty, crate::overload::IcsOptions::NONE);
            if ics.is_bad() {
                // At the argument (`From->getBeginLoc()`), also when the
                // template-id is substituted.
                let from = self.begin_loc(e);
                let from = if from.is_valid() { from } else { at };
                let d = self
                    .diag(DiagId::sema_cce_not_implicitly_convertible, from)
                    .arg(self.ty_arg(ety))
                    .arg(self.ty_arg(pty));
                let d = self.note_nttp_param(d, Some(self.ast.decls.get(p).loc()));
                self.emit(d);
                return Err(());
            }
            // clang `CheckConvertedConstantConversions` ([expr.const]/12):
            // a pointer or member pointer conversion only from
            // `std::nullptr_t` (so not `S*` to `const void*` or to a base
            // pointer), a boolean conversion only from an integral or
            // unscoped enumeration type. The sequence after a user-defined
            // conversion is the one checked.
            let scs = match ics {
                Ics::Standard(s) => Some(s),
                Ics::UserDefined(u) => Some(u.after),
                _ => None,
            };
            if let Some(scs) = scs
                && !self.cce_second_allowed(&scs)
            {
                let from = self.begin_loc(e);
                let from = if from.is_valid() { from } else { at };
                let d = self
                    .diag(DiagId::sema_cce_conversion_not_allowed, from)
                    .arg(self.ty_arg(ety))
                    .arg(self.ty_arg(pty));
                let d = self.note_nttp_param(d, Some(self.ast.decls.get(p).loc()));
                self.emit(d);
                return Err(());
            }
        }
        let t = &self.ast.types;
        if t.is_array(ety) && !t.is_pointer(pty) {
            self.templ_unimplemented("non-type template parameter of class or reference type", at);
            return Err(());
        }
        let pty = conv::strip_cv(self, pty);
        let c = self.in_cce(|s| conv::lvalue_to_rvalue(s, e));
        let cty = conv::strip_cv(self, self.ast.exprs.get(c).ty());
        let t = &self.ast.types;
        let int_like = |q| t.is_integral(q) || t.as_enum(q).is_some();
        if (int_like(pty) && t.is_floating(cty)) || (t.is_floating(pty) && int_like(cty)) {
            let d = self
                .diag(DiagId::sema_cce_conversion_not_allowed, at)
                .arg(self.ty_arg(ety))
                .arg(self.ty_arg(pty));
            let d = self.note_nttp_param(d, Some(self.ast.decls.get(p).loc()));
            self.emit(d);
            return Err(());
        }
        if t.is_integral(pty)
            && int_like(cty)
            && let Some(raw) = self.fold_int(c)
            && !self.int_fits(pty, raw)
        {
            let d = self
                .diag(DiagId::sema_cce_narrowing_value, at)
                .arg("non-type template argument")
                .arg(raw.to_string())
                .arg(self.ty_arg(pty));
            // clang `ext_cce_narrowing` (a `-Wc++11-narrowing` error by
            // default) recovers with the converted value.
            self.emit(d);
        }
        let c = match self.in_cce(|s| conv::implicit_convert(s, c, pty, ConvCtx::Initialization)) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, at);
                return Err(());
            }
        };
        let c = self.act_on_constant_full_expr(c);
        match self.eval_const(c, EvalContext::converted(at)) {
            Ok(v) => {
                // Integral values are re-interned at the canonical type so
                // equal arguments have equal ids (the specialization key).
                let canon = self.ast.types.canonical(pty);
                let t = &self.ast.types;
                let v = match self.const_int(v) {
                    Some(i) if t.is_integral(canon) || t.as_enum(canon).is_some() => {
                        self.int_const(canon, i)
                    }
                    _ => self.undecayed_void_ptr_arg(v),
                };
                Ok(TemplateArg::value(canon, v))
            }
            Err(f) => {
                let d = self.diag(DiagId::sema_expr_not_cce, at).arg("non-type template argument");
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                Err(())
            }
        }
    }

    /// Whether the second conversion of `scs` may appear in a converted
    /// constant expression of pointer, member pointer or `bool` type
    /// ([expr.const]/12; clang `CheckConvertedConstantConversions`, its
    /// pointer and boolean rows: the integral and floating rows are
    /// checked by the caller).
    fn cce_second_allowed(&self, scs: &StandardConversion) -> bool {
        match scs.second {
            SecondStep::PointerConversion | SecondStep::MemberPointerConversion => {
                self.is_nullptr_type(scs.from)
            }
            SecondStep::BooleanConversion => {
                self.ast.types.is_integral(scs.from) || conv::is_unscoped_enum(self, scs.from)
            }
            _ => true,
        }
    }

    /// The default argument of parameter `p`, substituted with the
    /// arguments before it ([temp.arg.general]/9) and converted.
    ///
    /// In a cleanup context of its own: the temporaries of the default are
    /// not those of the full-expression whose overload resolution uses it
    /// (clang `SubstDefaultTemplateArgument` enters a `ConstantEvaluated`
    /// context; spec/sema-class.md §5.3), with its own pending odr-uses.
    pub(crate) fn default_template_arg(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        def: TemplateArg,
        loc: Loc,
        prior: &[TemplateArg],
    ) -> Result<TemplateArg, ()> {
        self.push_cleanups();
        // Nor are the pending odr-use checks of that full-expression its
        // own: a name the call's arguments use, such as a constexpr local
        // named in a lambda, is resolved at the end of the call's
        // full-expression ([basic.def.odr]/5), not here.
        let outer_odr = std::mem::take(&mut self.pending_odr);
        let r = self.default_template_arg_in(tmpl, depth, p, def, loc, prior);
        let inner = std::mem::replace(&mut self.pending_odr, outer_odr);
        self.pending_odr.extend(inner);
        self.pop_cleanups();
        r
    }

    fn default_template_arg_in(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        def: TemplateArg,
        loc: Loc,
        prior: &[TemplateArg],
    ) -> Result<TemplateArg, ()> {
        let dependent = self.ast.types.arg_flags(def).intersects(
            kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION
                | kiln_ast::types::TypeFlags::UNEXPANDED_PACK,
        );
        if !dependent {
            let at = self.templ.default_locs.get(&p).copied().unwrap_or(loc);
            return self.check_template_arg(tmpl, depth, p, def, at, prior);
        }
        if !self.push_inst_frame(FrameKind::DefaultTemplateArg, p, tmpl, loc) {
            return Err(());
        }
        let so_far = self.ast.types.arg_list(prior);
        self.set_frame_args(so_far);
        let args = self.prior_levels(tmpl, depth, prior);
        // Errors point into the default argument, as clang's do.
        let at = self.templ.default_locs.get(&p).copied().unwrap_or(loc);
        // In the template's scope, not the use's (`n::f<int>()` outside
        // `n`).
        let r =
            self.in_template_context(tmpl, |s| Substituter::new(s, &args).template_arg(def, at));
        let r = match r {
            Ok(a) => self.check_template_arg(tmpl, depth, p, a, at, prior),
            Err(_) => Err(()),
        };
        self.pop_inst_frame();
        r
    }

    /// Whether integer `v` is representable in integral type `ty`.
    pub(crate) fn int_fits(&self, ty: QualType, v: i128) -> bool {
        crate::constfold::int_info(&self.ast, ty)
            .is_none_or(|(bits, signed)| crate::constfold::fits(v, bits, signed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(n: u32) -> DeclId {
        DeclId::new(n)
    }

    #[test]
    fn retained_depth_skips_substituted_levels() {
        let mut m = MultiLevelArgs::new();
        m.push(ArgListId::EMPTY, d(1), LevelMode::Substitute);
        m.push(ArgListId::EMPTY, d(2), LevelMode::Retain);
        m.push(ArgListId::EMPTY, d(3), LevelMode::Substitute);
        assert_eq!(m.retained_depth(0), 0);
        assert_eq!(m.retained_depth(1), 0);
        assert_eq!(m.retained_depth(2), 1);
        // Below the last level: every substituted level is outside.
        assert_eq!(m.retained_depth(4), 2);
        assert_eq!(m.substituted(), 2);
    }

    #[test]
    fn lookup_reads_substituted_levels_only() {
        let mut m = MultiLevelArgs::new();
        m.push(ArgListId::EMPTY, d(1), LevelMode::Substitute);
        m.push(ArgListId::EMPTY, d(2), LevelMode::Retain);
        let a = TemplateArg::ty(QualType::INT);
        let get = |_| vec![a];
        assert_eq!(m.lookup(0, 0, get), ArgLookup::Arg(a));
        assert_eq!(m.lookup(0, 1, get), ArgLookup::Missing);
        assert_eq!(m.lookup(1, 0, get), ArgLookup::Retain(0));
        assert_eq!(m.lookup(5, 0, get), ArgLookup::Retain(4));
    }

    #[test]
    fn single_level() {
        let m = MultiLevelArgs::single(ArgListId::EMPTY, d(7));
        assert_eq!(m.depth(), 1);
        assert_eq!(m.level(0).map(|l| l.assoc), Some(d(7)));
        assert!(m.level(1).is_none());
    }
}
