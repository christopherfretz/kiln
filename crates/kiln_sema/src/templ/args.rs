//! Template argument checking and conversion ([temp.arg]; spec/sema-
//! templates.md §3): kind checks, converted constant expressions for
//! non-type parameters, default arguments, packs, and the multi-level
//! argument lists the substituter consults (§5.1).

use kiln_ast::decl::DeclKind;
use kiln_ast::ids::{ArgListId, TemplateNameId};
use kiln_ast::types::{ArgView, TemplateArg, TemplateName, TypeFlags, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::EvalContext;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::conv::{self, ConvCtx, SecondStep, StandardConversion};
use crate::overload::Ics;
use crate::overload::address::AddrCtx;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LevelMode {
    /// The level's parameters are replaced by `args`.
    Substitute,
    /// The level's parameters stay parameters, renumbered to the depth
    /// they have once the substituted levels are gone (a member template
    /// of a class template specialization).
    Retain,
}

/// One template parameter level (§5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
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

    /// What the parameter at `(depth, index)` becomes, reading element
    /// `i` of an argument list `l` through `arg(l, i)`.
    pub fn lookup(
        &self,
        depth: u16,
        index: u16,
        arg: impl Fn(ArgListId, usize) -> Option<TemplateArg>,
    ) -> ArgLookup {
        match self.level(depth) {
            Some(l) if l.mode == LevelMode::Substitute => {
                match arg(l.args, usize::from(index)) {
                    // Not yet deduced (explicit arguments substituted before
                    // deduction, §6.1 step 1): the parameter stays.
                    Some(a) if matches!(a.view(), ArgView::Null) => {
                        ArgLookup::Retain(self.retained_depth(depth))
                    }
                    Some(a) => ArgLookup::Arg(a),
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
    /// `args` with each merely instantiation-dependent argument (not
    /// dependent: `TC<N>` with `N` a constant local of a function
    /// template) replaced by its canonical form: a specialization is
    /// shared by every use with the same canonical arguments (§3.1), so
    /// sugar naming one function template's locals must not reach the
    /// declaration another use finds (clang builds a specialization from
    /// its canonical converted arguments).
    pub(crate) fn settle_inst_sugar(&mut self, args: ArgListId) -> ArgListId {
        let only_inst = |f: TypeFlags| {
            f.intersects(TypeFlags::INSTANTIATION_DEPENDENT) && !f.intersects(TypeFlags::DEPENDENT)
        };
        if !only_inst(self.ast.types.arg_list_flags(args))
            && !self.ast.types.args(args).iter().any(|&a| only_inst(self.ast.types.arg_flags(a)))
        {
            return args;
        }
        let out: Vec<TemplateArg> = self
            .ast
            .types
            .args(args)
            .iter()
            .map(|&a| {
                if only_inst(self.ast.types.arg_flags(a)) {
                    self.ast.types.canonical_arg(a)
                } else {
                    a
                }
            })
            .collect();
        self.ast.types.arg_list(&out)
    }

    /// The parameters of template `tmpl`, in order.
    pub(crate) fn template_params(&self, tmpl: DeclId) -> Vec<DeclId> {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("template_params");
        let Some(info) = self.ast.template_info(tmpl).copied() else { return Vec::new() };
        let l = *self.ast.templates.param_list(info.params);
        self.ast.lists.iter::<DeclId>(l.params).collect()
    }

    /// [`Self::template_params`] into `out` (cleared first).
    pub(crate) fn template_params_into(&self, tmpl: DeclId, out: &mut Vec<DeclId>) {
        out.clear();
        if let Some(info) = self.ast.template_info(tmpl) {
            let l = *self.ast.templates.param_list(info.params);
            out.extend(self.ast.lists.iter::<DeclId>(l.params));
        }
    }

    /// The number of `tmpl`'s parameters ([`Self::template_params`]'s
    /// length, without collecting them).
    pub(crate) fn template_param_count(&self, tmpl: DeclId) -> usize {
        self.ast
            .template_info(tmpl)
            .map_or(0, |i| self.ast.lists.get(self.ast.templates.param_list(i.params).params).len())
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
        // The `>` of the template-id as written, when this is its check
        // (clang `TemplateArgumentListInfo::getRAngleLoc`); the template
        // name otherwise.
        let r_angle = match self.templ.r_angle.take() {
            Some((first, r)) if first == arg_locs.first().copied() => r,
            _ => loc,
        };
        let mut wi = 0usize;
        let mut open_expansion = false;
        // `out` with each argument kept as written only for its
        // instantiation-dependence converted, once there is one:
        // the canonical list, and the prior arguments a later default is
        // substituted with (`A = mx<L>::value` after `L = sizeof(member)`
        // names `mx<32>::value`, not a dependent `mx<sizeof(member)>`).
        let mut canon: Option<Vec<TemplateArg>> = None;
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
                push_arg(&mut out, &mut canon, TemplateArg::pack(l));
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
                push_arg(&mut out, &mut canon, TemplateArg::pack(l));
                continue;
            }
            if wi < written.len() {
                let a = self.check_template_arg(tmpl, depth, p, written[wi], arg_loc(wi), &out)?;
                if let (true, ArgView::Expr { expr, .. }) =
                    (self.inst_dependent_nttp_arg(p, a), a.view())
                {
                    let c = canon.get_or_insert_with(|| out.clone());
                    let v = self.convert_nttp_arg(tmpl, depth, p, expr, arg_loc(wi), c)?;
                    c.push(v);
                } else if let Some(c) = canon.as_mut() {
                    c.push(a);
                }
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
            let prior = canon.as_deref().unwrap_or(&out);
            let a = self.checked_default_template_arg(tmpl, depth, p, def, loc, r_angle, prior)?;
            push_arg(&mut out, &mut canon, a);
        }
        if wi < written.len() {
            self.diag_template_arity(tmpl, true, loc);
            return Err(());
        }
        let sugared = self.ast.types.arg_list(&out);
        let Some(canon) = canon else {
            let canonical = self.ast.types.canonical_args(sugared);
            return Ok(ConvertedArgs { sugared, canonical });
        };
        // The canonical argument is the value ([temp.arg.nontype]/2; clang
        // converts an argument that is not value-dependent), so
        // `enable_if<sizeof(first_t<int, decltype(T::x)>) == 4>` names
        // `enable_if<true>`; the written form is what substitution
        // re-checks ([temp.deduct.general]/8).
        let canon = self.ast.types.arg_list(&canon);
        let canonical = self.ast.types.canonical_args(canon);
        Ok(ConvertedArgs { sugared, canonical })
    }

    /// Whether checked argument `a` for parameter `p` is a non-type
    /// argument kept as written although not value-dependent: it is
    /// instantiation-dependent through a type or qualifier.
    fn inst_dependent_nttp_arg(&self, p: DeclId, a: TemplateArg) -> bool {
        let ArgView::Expr { expr, .. } = a.view() else { return false };
        self.kind_of(p) == DeclKind::NonTypeTemplateParm
            && !self.is_err_expr(expr)
            && !self.expr_is_value_dependent(expr)
            && !self.ast.types.is_dependent(self.ast.exprs.get(expr).ty())
            && self.type_instantiation_dependent(expr)
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
    /// expression for a non-type parameter). A non-dependent non-type
    /// argument is converted, a value-dependent one only checked for a
    /// conversion its type cannot make; a missing argument takes its
    /// default, substituted with the prior arguments and checked as
    /// they are. A dependent `T::x` for a type
    /// parameter is "did you forget 'typename'?" and the check goes on,
    /// as clang's does; any other failure ends it. Only the arguments
    /// before a pack expansion are checked, and not the arity; a
    /// template-id naming an expanded parameter pack is checked only for
    /// that pack's arity. Reads the pending argument locations and leaves
    /// them for the caller. `Err` after a diagnostic.
    pub(crate) fn check_dependent_template_args(
        &mut self,
        tmpl: DeclId,
        args: &[TemplateArg],
        loc: Loc,
    ) -> Result<(), ()> {
        let locs = std::mem::take(&mut self.templ.arg_locs);
        let r = self.check_dependent_template_args_at(tmpl, args, &locs, loc);
        self.templ.arg_locs = locs;
        r
    }

    pub(crate) fn check_dependent_template_args_at(
        &mut self,
        tmpl: DeclId,
        args: &[TemplateArg],
        locs: &[Loc],
        loc: Loc,
    ) -> Result<(), ()> {
        let saved = self.inst.template_id_loc.replace(loc);
        let r = self.check_dependent_template_args_in(tmpl, args, locs, loc);
        self.inst.template_id_loc = saved;
        r
    }

    fn check_dependent_template_args_in(
        &mut self,
        tmpl: DeclId,
        args: &[TemplateArg],
        locs: &[Loc],
        loc: Loc,
    ) -> Result<(), ()> {
        self.check_expanded_pack_arity(tmpl, args, loc)?;
        let head = self.head_template(tmpl);
        let params = self.template_params(head);
        if params.iter().any(|p| self.templ.expanded_packs.contains_key(p)) {
            return Ok(());
        }
        // The `>` of the template-id as written (the "while checking a
        // default template argument" frame), as the full check finds it.
        let r_angle = match self.templ.r_angle {
            Some((first, r)) if first == locs.first().copied() => r,
            _ => loc,
        };
        // A pack expansion may cover any number of parameters: the
        // arguments before it are checked, the arity is not.
        let expansion = args.iter().position(|&a| self.is_pack_expansion_arg(a));
        let checked = expansion.unwrap_or(args.len());
        let depth = self.template_param_depth(head);
        let tmpl = self.ast.first(tmpl);
        let arg_loc = |i: usize| locs.get(i).copied().unwrap_or(loc);
        let mut prior: Vec<TemplateArg> = Vec::with_capacity(params.len());
        let mut failed = false;
        let mut wi = 0usize;
        let mut open = false;
        // A pack expansion is checked against the parameters only as
        // written: substitution forms the template-id of an element that
        // is itself an expansion (`remove_const_t<Ts...>` for `_Types` :=
        // `{Ts...}` in a fold) on the way to re-forming the expansion.
        let written = self.inst.stack.is_empty();
        for &p in &params {
            if self.ast.template_parm(p).is_some_and(|x| x.pack) {
                while wi < checked {
                    let (f, _) =
                        self.check_dependent_arg(tmpl, depth, p, args[wi], arg_loc(wi), &prior)?;
                    failed |= f;
                    wi += 1;
                }
                open = true;
                break;
            }
            if wi >= checked {
                // After a pack expansion the rest are known once it is
                // substituted; an alias template or a concept takes no
                // pack expansion for a parameter that is not a pack
                // ([temp.alias]/2, CWG1430; clang's
                // `err_template_expansion_into_fixed_list`).
                if expansion.is_some() {
                    let alias = match self.kind_of(tmpl) {
                        _ if !written => None,
                        DeclKind::TypeAliasTemplate => Some(0u32),
                        DeclKind::Concept => Some(1),
                        _ => None,
                    };
                    if let Some(k) = alias {
                        let d = self
                            .diag(DiagId::sema_template_expansion_into_fixed_list, arg_loc(wi))
                            .arg(k);
                        let d = self.note_param_here(d, p);
                        self.emit(d);
                        return Err(());
                    }
                    open = true;
                    break;
                }
                // A default is substituted with the arguments before it
                // and checked, as for a non-dependent template-id.
                let Some(def) = self.template_default(p) else {
                    self.diag_template_arity(tmpl, false, loc);
                    return Err(());
                };
                let a =
                    self.checked_default_template_arg(tmpl, depth, p, def, loc, r_angle, &prior)?;
                prior.push(a);
                continue;
            }
            let (f, a) = self.check_dependent_arg(tmpl, depth, p, args[wi], arg_loc(wi), &prior)?;
            failed |= f;
            prior.push(a);
            wi += 1;
        }
        // Too many even before a pack expansion (`R<int, int, int, T...>`
        // for two parameters), or a pack expansion left over.
        if wi < checked || (written && !open && wi < args.len()) {
            self.diag_template_arity(tmpl, true, loc);
            return Err(());
        }
        if failed { Err(()) } else { Ok(()) }
    }

    /// One argument of [`Self::check_dependent_template_args`], with the
    /// argument it converts to: `Ok((true, a))` after the recoverable
    /// missing-`typename` error, `Err` after any other.
    fn check_dependent_arg(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        a: TemplateArg,
        at: Loc,
        prior: &[TemplateArg],
    ) -> Result<(bool, TemplateArg), ()> {
        match (self.kind_of(p), a.view()) {
            (DeclKind::TemplateTypeParm, ArgView::Expr { expr, .. })
                if self.ast.exprs.get(expr).kind()
                    == kiln_ast::expr::ExprKind::DependentScopeDeclRefExpr =>
            {
                let d = self.diag(DiagId::sema_template_arg_must_be_type_suggest, at);
                let d = self.note_param_here(d, p);
                self.emit(d);
                Ok((true, a))
            }
            // A parameter whose type needs dependent prior arguments, or
            // the pattern of a parameter pack's types (`template<Ts...
            // a>`), takes its argument once substituted.
            (DeclKind::NonTypeTemplateParm, ArgView::Expr { .. })
                if self.nttp_type_pending(p, prior) =>
            {
                Ok((false, a))
            }
            // A non-type argument is converted, a value-dependent one
            // only checked for its conversion (the dependent template-id
            // keeps its arguments as written).
            (_, ArgView::Type(_) | ArgView::Template(_) | ArgView::Expr { .. }) => {
                self.check_template_arg(tmpl, depth, p, a, at, prior).map(|a| (false, a))
            }
            // A profiled or substituted value is checked when the
            // template-id is.
            _ => Ok((false, a)),
        }
    }

    /// Whether the type of non-type parameter `p` is dependent and cannot
    /// be substituted with `prior` yet: they are dependent, or `p` is a
    /// parameter pack (not an expanded one, §3.2).
    fn nttp_type_pending(&self, p: DeclId, prior: &[TemplateArg]) -> bool {
        if self.templ.nttp_elem.is_some() {
            return false;
        }
        let declared = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        self.ast.types.is_dependent(declared)
            && (self.ast.template_parm(p).is_some_and(|x| x.pack)
                || prior.iter().any(|&a| {
                    self.ast
                        .types
                        .arg_flags(a)
                        .intersects(TypeFlags::DEPENDENT | TypeFlags::UNEXPANDED_PACK)
                }))
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
    pub(crate) fn arg_template_decl(&self, name: TemplateNameId) -> Option<DeclId> {
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("check_template_arg");
        match (self.kind_of(p), a.view()) {
            (DeclKind::TemplateTypeParm, ArgView::Type(t)) => {
                // Before C++17 an argument's exception specification is
                // not part of it (spec/types.md §5.4).
                Ok(TemplateArg::ty(self.ast.types.without_exc_sugar(t)))
            }
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
            // A value-dependent argument is evaluated when it is
            // substituted ([temp.res.general]/6), and so is an
            // instantiation-dependent one (`is_same<void,
            // void_t<decltype(e)>>::value`): folding it now would drop the
            // substitution failure of `e` ([temp.deduct.general]/8; clang
            // keeps it as written). One of a known type is checked for its
            // conversion now (§3.2).
            (DeclKind::NonTypeTemplateParm, ArgView::Expr { expr, .. })
                if self.expr_is_value_dependent(expr)
                    || self.type_instantiation_dependent(expr) =>
            {
                self.check_value_dependent_nttp_arg(tmpl, depth, p, expr, at, prior).map(|()| a)
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
                let Some(d) = self.arg_template_decl(n) else { return Ok(a) };
                // [temp.arg.template]/1, /3: the argument's kind is the
                // parameter's (a variable template or concept for a type
                // template template parameter is rejected as clang 22
                // does, P2841R7).
                match self.template_arg_kind(d) {
                    Some(k) if k == self.ttp_kind(p) => {
                        let args = self.prior_levels(tmpl, depth, prior);
                        if !self.check_template_template_arg(p, d, at, &args) {
                            return Err(());
                        }
                        Ok(a)
                    }
                    Some(_) => {
                        let want = self.ttp_kind(p);
                        self.reject_ttp_arg_kind(want, d, at);
                        Err(())
                    }
                    None => {
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("in_template_context");
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("prior_levels");
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

    /// Whether `e` is instantiation-dependent through a type or a
    /// qualifier (`is_same<void, void_t<decltype(x)>>::value`), whose
    /// substitution can fail; a type operand counts
    /// (`sizeof(void_t<decltype(x)>*)`, `__is_same(U, first_t<int,
    /// decltype(x)>)`; WP-465). A reference to a local declaration of a
    /// template (`constexpr int N = 3; TC<N>`) is instantiation-dependent
    /// in kiln only so that it is remapped; clang folds it, and so does
    /// kiln.
    fn type_instantiation_dependent(&self, e: ExprId) -> bool {
        use kiln_ast::expr::{ExprDeps, ExprKind};
        let (x, t) = (&self.ast.exprs, &self.ast.types);
        let inst = |q: QualType| t.flags(q.ty()).contains(TypeFlags::INSTANTIATION_DEPENDENT);
        let mut stack = vec![e];
        while let Some(e) = stack.pop() {
            if !x.get(e).deps().contains(ExprDeps::INSTANTIATION) {
                continue;
            }
            let operand = match x.get(e).kind() {
                ExprKind::UnaryExprOrTypeTraitExpr => x.trait_type_arg(e).is_some_and(inst),
                ExprKind::TypeTraitExpr => {
                    self.ast.lists.iter::<QualType>(x.type_trait_args(e)).any(inst)
                }
                _ => false,
            };
            if operand
                || inst(x.get(e).ty())
                || x.decl_ref_extra(e)
                    .and_then(|d| d.qualifier)
                    .is_some_and(|q| t.nns_flags(q).contains(TypeFlags::INSTANTIATION_DEPENDENT))
            {
                return true;
            }
            x.children_into(e, &self.ast.lists, &mut stack);
        }
        false
    }

    /// The type of non-type parameter `p` for an argument at `at`: its
    /// declared type (an expanded pack's element type), or that type
    /// substituted with the `prior` arguments and checked. `Err` after a
    /// diagnostic or for an erroneous type.
    fn nttp_arg_param_type(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        at: Loc,
        prior: &[TemplateArg],
    ) -> Result<QualType, ()> {
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
        let pty = r?;
        if self.is_error_type(pty) {
            return Err(());
        }
        Ok(pty)
    }

    /// The definition-time check of value-dependent argument `e` for
    /// non-type parameter `p` (clang `CheckConvertedConstantExpression`,
    /// which runs on a value-dependent argument whose type is known,
    /// [temp.arg.nontype]/2, [expr.const]/12; §3.2): with no implicit
    /// conversion sequence to the parameter's type it is "not implicitly
    /// convertible"; a sequence a converted constant expression may not
    /// use is "not allowed". A value-dependent integral argument may
    /// still be a null pointer constant, so it converts to a pointer,
    /// member pointer or `std::nullptr_t` only by a disallowed
    /// conversion. Narrowing is checked once the value is known; a
    /// type-dependent argument, a parameter of dependent, deduced or
    /// class type are checked once substituted. `Err` after a
    /// diagnostic.
    fn check_value_dependent_nttp_arg(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        e: ExprId,
        at: Loc,
        prior: &[TemplateArg],
    ) -> Result<(), ()> {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("check_value_dependent_nttp_arg");
        let ety = self.ast.exprs.get(e).ty();
        if self.is_err_expr(e) || ety == QualType::OVERLOAD || self.ast.types.is_dependent(ety) {
            return Ok(());
        }
        // A type substituted from dependent prior arguments stays
        // dependent: checked once they are substituted.
        if self.nttp_type_pending(p, prior) {
            return Ok(());
        }
        let pty = self.nttp_arg_param_type(tmpl, depth, p, at, prior)?;
        let t = &self.ast.types;
        if t.is_dependent(pty)
            || t.as_record(pty).is_some()
            || matches!(t.canonical_view(pty), TypeView::Auto { .. })
            || self.is_ctad_placeholder(pty)
            || self.placeholder_keyword(pty).is_some()
        {
            return Ok(());
        }
        let from = self.begin_loc(e);
        let from = if from.is_valid() { from } else { at };
        let pdecl = Some(self.ast.decls.get(p).loc());
        let t = &self.ast.types;
        let src = conv::strip_cv(self, ety);
        let nullish = t.is_integral(src)
            && (t.is_pointer(pty) || self.is_member_pointer(pty) || self.is_nullptr_type(pty));
        let disallowed = if nullish {
            true
        } else {
            let info = self.arg_info(e);
            let ics = self.try_implicit_conversion(&info, pty, crate::overload::IcsOptions::NONE);
            if ics.is_bad() {
                let d = self
                    .diag(DiagId::sema_cce_not_implicitly_convertible, from)
                    .arg(self.ty_arg(ety))
                    .arg(self.ty_arg(pty))
                    .range(self.expr_range(e));
                let d = self.note_nttp_param(d, pdecl);
                self.emit(d);
                return Err(());
            }
            let scs = match ics {
                Ics::Standard(s) => Some(s),
                Ics::UserDefined(u) => Some(u.after),
                _ => None,
            };
            let to = conv::strip_cv(self, pty);
            let t = &self.ast.types;
            let int_like = |q| t.is_integral(q) || t.as_enum(q).is_some();
            scs.is_some_and(|scs| !self.cce_second_allowed(&scs))
                || (int_like(to) && t.is_floating(src))
                || (t.is_floating(to) && int_like(src))
        };
        if disallowed {
            let d = self
                .diag(DiagId::sema_cce_conversion_not_allowed, from)
                .arg(self.ty_arg(ety))
                .arg(self.ty_arg(pty));
            let d = self.note_nttp_param(d, pdecl);
            self.emit(d);
            return Err(());
        }
        Ok(())
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("convert_nttp_arg");
        if self.is_err_expr(e) {
            return Err(());
        }
        let mut pty = self.nttp_arg_param_type(tmpl, depth, p, at, prior)?;
        if self.ast.types.is_dependent(pty) {
            // `template<class T, T V>` with a dependent `T`: converted
            // when substituted.
            return Ok(self.builder().template_arg_expr(e));
        }
        if let Some(arg) = self.substituted_int_nttp_arg(e, pty) {
            return Ok(arg);
        }
        // P2308R1 ([temp.arg.nontype]/1): a braced-init-list
        // copy-list-initializes the parameter (its type deduced first for
        // a class template placeholder).
        let e = match self.braced_template_arg(e) {
            Some(l) => {
                if !self.is_ctad_placeholder(pty) && self.placeholder_keyword(pty).is_some() {
                    // `template<auto V>`: deduced as for `auto v = {..};`,
                    // a `std::initializer_list`, never structural (clang
                    // words it as for a missing <initializer_list>).
                    let d = self.diag(DiagId::sema_auto_init_list_deduction, l.lbrace);
                    self.emit(d);
                    return Err(());
                }
                if self.is_ctad_placeholder(pty) {
                    let init = crate::init::Initializer::List(l.clone());
                    pty = self.ctad_for_initializer(pty, &init, false, at).ok_or(())?;
                }
                let note = Some(self.param_decl_note(p));
                let to = conv::strip_cv(self, pty);
                let c = self.init_list_entity(to, &l, l.lbrace, ConvCtx::Argument, note);
                if self.is_err_expr(c) || self.has_errors_in(c) {
                    return Err(());
                }
                c
            }
            None => e,
        };
        let ety = self.ast.exprs.get(e).ty();
        #[cfg(feature = "op-stats")]
        crate::templ::deduce_stats::tally(
            "nttp",
            format!(
                "{:?} {} <- {}",
                self.ast.exprs.get(e).kind(),
                self.ty_str(pty),
                self.ty_str(ety)
            ),
        );
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
        // An overload set resolves against the parameter's type
        // ([temp.arg.nontype]/2, [over.over]/1.5); with no match it has no
        // implicit conversion (`A<f>` for an `int`, a non-template
        // `g<int>` recovered from a typo; WP-420).
        let (e, ety) = if ety == QualType::OVERLOAD && !t.is_dependent(pty) {
            let r = self.address_match(e, pty).and_then(|_| {
                self.resolve_address_of_overloaded(e, pty, false, AddrCtx::Convert, at)
            });
            let Some(r) = r else {
                let d = self
                    .diag(DiagId::sema_cce_not_implicitly_convertible, self.begin_loc(e))
                    .arg(self.ty_arg(ety))
                    .arg(self.ty_arg(pty))
                    .range(self.expr_range(e));
                let d = self.note_nttp_param(d, Some(self.ast.decls.get(p).loc()));
                self.emit(d);
                return Err(());
            };
            (r, self.ast.exprs.get(r).ty())
        } else {
            (e, ety)
        };
        let t = &self.ast.types;
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
                // A function template-id resolved early is still clang's
                // overload set (`A<tf<int>>`, `A<&tf<int>>`).
                let shown =
                    if self.overload_set_of(e).is_some() { QualType::OVERLOAD } else { ety };
                let d = self
                    .diag(DiagId::sema_cce_not_implicitly_convertible, from)
                    .arg(self.ty_arg(shown))
                    .arg(self.ty_arg(pty))
                    .range(self.expr_range(e));
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
        let narrowed = if t.is_integral(pty) && int_like(cty) {
            self.fold_int(c).filter(|&raw| !self.int_fits(pty, raw))
        } else {
            None
        };
        let c = match self.in_cce(|s| conv::implicit_convert(s, c, pty, ConvCtx::Initialization)) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, at);
                return Err(());
            }
        };
        // After a user-defined conversion the standard conversion that
        // follows it narrows from the conversion function's result
        // (`Z<CI{}>` with `operator int` to `bool`; clang
        // `getNarrowingKind` on `ICS.UserDefined.After`).
        if self.ast.types.as_record(cty).is_some() && self.ast.types.is_integral(pty) {
            let from = self.ignore_paren_imp_casts(c);
            let t = &self.ast.types;
            let fty = conv::strip_cv(self, self.ast.exprs.get(from).ty());
            if from != c
                && (t.is_integral(fty) || t.as_enum(fty).is_some())
                && let Some(raw) = self.fold_int(from)
                && !self.int_fits(pty, raw)
            {
                let d = self
                    .diag(DiagId::sema_cce_narrowing_value, at)
                    .arg("non-type template argument")
                    .arg(raw.to_string())
                    .arg(self.ty_arg(pty));
                self.emit(d);
            }
        }
        let c = self.act_on_constant_full_expr(c);
        // After the full-expression's conversion warnings, as in clang
        // (`BuildConvertedConstantExpression`): `ext_cce_narrowing` (a
        // `-Wc++11-narrowing` error by default) recovers with the
        // converted value.
        if let Some(raw) = narrowed {
            let d = self
                .diag(DiagId::sema_cce_narrowing_value, at)
                .arg("non-type template argument")
                .arg(raw.to_string())
                .arg(self.ty_arg(pty));
            self.emit(d);
        }
        if self.is_err_expr(c) {
            return Err(());
        }
        if let Some(v) = self.defining_class_member_ptr(c, pty) {
            return Ok(TemplateArg::value(self.ast.types.canonical(pty), v));
        }
        match self.eval_const(c, EvalContext::converted(at)) {
            Ok(v) if self.invalid_template_arg_base(v, false, at) => Err(()),
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
            // clang does not evaluate an argument that contains errors: a
            // variable whose initializer names an invalid declaration (a
            // variable template specialization that failed to
            // instantiate) was already diagnosed.
            Err(_) if self.refers_to_invalid_var(c) => Err(()),
            Err(f) => {
                let d = self.diag(DiagId::sema_expr_not_cce, at).arg("non-type template argument");
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                Err(())
            }
        }
    }

    /// [temp.arg.nontype]/6: a pointer argument `v` (or the address a
    /// `reference` argument binds) must not point to a
    /// string literal, the result of a `typeid` expression or a temporary
    /// object (or a subobject of one).
    /// Diagnosed at `at` as clang's `err_constexpr_invalid_template_arg`
    /// ("pointer to subobject of string literal is not allowed in a
    /// template argument"); in a deduction it is the substitution
    /// failure of the explicitly specified argument.
    pub(crate) fn invalid_template_arg_base(
        &mut self,
        v: kiln_ast::ids::ConstValueId,
        reference: bool,
        at: Loc,
    ) -> bool {
        use kiln_ast::consts::{PtrBase, Scalar};
        let Some(Scalar::Pointer { ty, base, offset, .. }) = self.ast.consts.scalar(v) else {
            return false;
        };
        let t = &self.ast.types;
        let to_array = t.pointee(ty).is_some_and(|p| t.is_array(p));
        let (what, sub) = match base {
            // Gap: `__func__` designates its string literal, so it is
            // named "string literal" here, not "predefined '__func__'
            // variable" (clang).
            PtrBase::StringLit { .. } => ("string literal", !to_array),
            PtrBase::TypeInfo(_) => ("type_info object", offset != 0),
            PtrBase::Temporary(_) => ("temporary object", offset != 0),
            _ => return false,
        };
        let d = self
            .diag(DiagId::sema_constexpr_invalid_template_arg, at)
            .arg(if reference { "reference" } else { "pointer" })
            .arg(if sub { "subobject of " } else { "" })
            .arg(what);
        self.emit(d);
        true
    }

    /// The value of `c`, a converted argument of member pointer type
    /// `pty`, when it is `&C::m` (of exactly that type) and `C` is still
    /// being defined (boost.intrusive's `member_hook<C, H, &C::hook>` in
    /// `C`'s own body): the member itself, as clang's
    /// `APValue::MemberPointer` holds it. The Oven would need `C`'s
    /// layout, which it does not have yet (WP-726). `None` otherwise.
    fn defining_class_member_ptr(
        &mut self,
        c: ExprId,
        pty: QualType,
    ) -> Option<kiln_ast::ids::ConstValueId> {
        use kiln_ast::consts::{MemberKind, Scalar};
        use kiln_ast::expr::{ExprKind, UnaryOp};
        let canon = self.ast.types.canonical(pty);
        let TypeView::MemberPointer { class, .. } = self.ast.types.view(canon.ty()) else {
            return None;
        };
        if self.is_complete(QualType::unqual(class)) {
            return None;
        }
        let x = self.ignore_paren_imp_casts(c);
        if self.ast.types.canonical(self.ety(x)) != canon
            || self.ast.exprs.get(x).kind() != ExprKind::UnaryOperator
            || self.ast.exprs.unary_op(x) != UnaryOp::AddrOf
        {
            return None;
        }
        let r = self.ast.exprs.operand(x);
        if self.ast.exprs.get(r).kind() != ExprKind::DeclRefExpr {
            return None;
        }
        let d = self.ast.exprs.decl(r);
        let kind = match self.kind_of(d) {
            DeclKind::Field => MemberKind::Data,
            DeclKind::CXXMethod => MemberKind::Function,
            _ => return None,
        };
        let member = self.ast.first(d);
        self.ast
            .consts
            .push_scalar(Scalar::MemberPtr { ty: canon, member, kind, derived: false, path: &[] })
            .ok()
    }

    /// The converted argument of an integral parameter of type `pty` for
    /// `e`, an already-substituted parameter whose value is a plain
    /// integer literal of exactly that type (`get<I>` with `I` from an
    /// enclosing specialization): the value the full conversion below
    /// computes, without building and evaluating the converted constant
    /// expression (WP-657). `None` falls back to the full path, as does
    /// anything the full path would act on beyond `e` itself (pending
    /// member references, deleted-function notes, misaligned members,
    /// potential captures) and [`Sema::full_deduction`].
    fn substituted_int_nttp_arg(&mut self, e: ExprId, pty: QualType) -> Option<TemplateArg> {
        use kiln_ast::expr::ExprKind;
        if self.full_deduction
            || self.ast.exprs.get(e).kind() != ExprKind::SubstNonTypeTemplateParmExpr
            || !self.member_refs.is_empty()
            || self.special.has_pending()
            || !self.misaligned.is_empty()
            || !self.potential_caps.is_empty()
        {
            return None;
        }
        let r = self.ast.exprs.subst_replacement(e);
        if self.ast.exprs.get(r).kind() != ExprKind::IntegerLiteral {
            return None;
        }
        let v = i128::from(self.ast.exprs.int_value(r)?);
        let canon = self.ast.types.canonical(conv::strip_cv(self, pty));
        let same = |s: &Self, x: ExprId| {
            let q = conv::strip_cv(s, s.ast.exprs.get(x).ty());
            s.ast.types.canonical(q) == canon
        };
        let t = &self.ast.types;
        if !t.is_integral(canon)
            || t.as_enum(canon).is_some()
            || t.builtin(canon).is_none()
            || !same(self, e)
            || !same(self, r)
            || !self.int_fits(canon, v)
        {
            return None;
        }
        Some(TemplateArg::value(canon, self.int_const(canon, v)))
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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("default_template_arg");
        self.default_template_arg_impl(tmpl, depth, p, def, loc, None, prior)
    }

    /// [`Self::default_template_arg`] for a template-id's argument list
    /// (clang `CheckTemplateArgumentList`): the substituted default is
    /// checked under a "while checking a default template argument used
    /// here" frame at the template-id's `>`, `r_angle`
    /// (`DefaultTemplateArgumentChecking`); substituting it is the
    /// "in instantiation of default argument" frame at the name, `loc`.
    #[allow(clippy::too_many_arguments, reason = "mirrors default_template_arg")]
    fn checked_default_template_arg(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        def: TemplateArg,
        loc: Loc,
        r_angle: Loc,
        prior: &[TemplateArg],
    ) -> Result<TemplateArg, ()> {
        self.default_template_arg_impl(tmpl, depth, p, def, loc, Some(r_angle), prior)
    }

    #[allow(clippy::too_many_arguments, reason = "mirrors default_template_arg")]
    fn default_template_arg_impl(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        def: TemplateArg,
        loc: Loc,
        checking: Option<Loc>,
        prior: &[TemplateArg],
    ) -> Result<TemplateArg, ()> {
        self.push_cleanups();
        // Nor are the pending odr-use checks of that full-expression its
        // own: a name the call's arguments use, such as a constexpr local
        // named in a lambda, is resolved at the end of the call's
        // full-expression ([basic.def.odr]/5), not here.
        let outer_odr = std::mem::take(&mut self.pending_odr);
        let r = self.default_template_arg_in(tmpl, depth, p, def, loc, checking, prior);
        let inner = std::mem::replace(&mut self.pending_odr, outer_odr);
        self.pending_odr.extend(inner);
        self.pop_cleanups();
        r
    }

    #[allow(clippy::too_many_arguments, reason = "mirrors default_template_arg")]
    fn default_template_arg_in(
        &mut self,
        tmpl: DeclId,
        depth: u16,
        p: DeclId,
        def: TemplateArg,
        loc: Loc,
        checking: Option<Loc>,
        prior: &[TemplateArg],
    ) -> Result<TemplateArg, ()> {
        let dependent = self
            .ast
            .types
            .arg_flags(def)
            .intersects(TypeFlags::DEPENDENT_INSTANTIATION | TypeFlags::UNEXPANDED_PACK);
        // Errors point into the default argument, as clang's do.
        let at = self.templ.default_locs.get(&p).copied().unwrap_or(loc);
        let a = if dependent {
            if !self.push_inst_frame(FrameKind::DefaultTemplateArg, p, tmpl, loc) {
                return Err(());
            }
            let so_far = self.ast.types.arg_list(prior);
            self.set_frame_args(so_far);
            let args = self.prior_levels(tmpl, depth, prior);
            // A declarator error names the parameter, at the default's
            // innermost written `*`/`[` (clang's `TypeLoc` of the default).
            let name = self.ast.decls.get(p).name().as_ident().map(|n| (n, self.decl_loc(p)));
            let chunk = self.inner_chunk_locs.get(&p).copied();
            // In the template's scope, not the use's (`n::f<int>()`
            // outside `n`).
            let r = self
                .in_template_context(tmpl, |s| {
                    let mut sub = Substituter::new(s, &args);
                    sub.declarator_name = name;
                    sub.chunk_loc = chunk;
                    sub.template_arg(def, at)
                })
                .map_err(|_| ());
            if checking.is_none() {
                let r = r.and_then(|a| self.check_template_arg(tmpl, depth, p, a, at, prior));
                self.pop_inst_frame();
                return r;
            }
            // Substituted: the instantiation frame ends here (clang
            // `SubstDefaultTemplateArgument`).
            self.pop_inst_frame();
            r?
        } else {
            def
        };
        match checking {
            Some(r_angle) => {
                if !self.push_inst_frame(FrameKind::DefaultArgChecking, p, tmpl, r_angle) {
                    return Err(());
                }
                let r = self.check_template_arg(tmpl, depth, p, a, at, prior);
                self.pop_inst_frame();
                r
            }
            _ => self.check_template_arg(tmpl, depth, p, a, at, prior),
        }
    }

    /// Whether integer `v` is representable in integral type `ty`.
    pub(crate) fn int_fits(&self, ty: QualType, v: i128) -> bool {
        crate::constfold::int_info(&self.ast, ty)
            .is_none_or(|(bits, signed)| crate::constfold::fits(v, bits, signed))
    }
}

/// Appends checked argument `a` to `out` and, once there is one, to the
/// list with the instantiation-dependent arguments converted.
fn push_arg(out: &mut Vec<TemplateArg>, canon: &mut Option<Vec<TemplateArg>>, a: TemplateArg) {
    out.push(a);
    if let Some(c) = canon.as_mut() {
        c.push(a);
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
        let get = |_, i: usize| [a].get(i).copied();
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
