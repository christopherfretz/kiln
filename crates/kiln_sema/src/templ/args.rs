//! Template argument checking and conversion ([temp.arg]; spec/sema-
//! templates.md §3): kind checks, converted constant expressions for
//! non-type parameters, default arguments, packs, and the multi-level
//! argument lists the substituter consults (§5.1).

use kiln_ast::decl::DeclKind;
use kiln_ast::ids::ArgListId;
use kiln_ast::types::{ArgView, TemplateArg, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::EvalContext;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::conv::{self, ConvCtx};
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
            Some(l) if l.mode == LevelMode::Substitute => args(l.args)
                .get(usize::from(index))
                .copied()
                .map_or(ArgLookup::Missing, ArgLookup::Arg),
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
        let tmpl = self.ast.first(tmpl);
        let params = self.template_params(tmpl);
        let depth = self.template_param_depth(tmpl);
        let mut out: Vec<TemplateArg> = Vec::with_capacity(params.len());
        let mut ok = true;
        let arg_loc = |i: usize| arg_locs.get(i).copied().unwrap_or(loc);
        let mut wi = 0usize;
        for (pi, &p) in params.iter().enumerate() {
            let pack = self.ast.template_parm(p).is_some_and(|x| x.pack);
            if pack {
                let mut elems = Vec::new();
                while wi < written.len() {
                    match self.check_template_arg(tmpl, depth, p, written[wi], arg_loc(wi), &out) {
                        Ok(a) => elems.push(a),
                        Err(()) => ok = false,
                    }
                    wi += 1;
                }
                let l = self.ast.types.arg_list(&elems);
                out.push(TemplateArg::pack(l));
                continue;
            }
            if wi < written.len() {
                match self.check_template_arg(tmpl, depth, p, written[wi], arg_loc(wi), &out) {
                    Ok(a) => out.push(a),
                    Err(()) => {
                        ok = false;
                        out.push(TemplateArg::NULL);
                    }
                }
                wi += 1;
                continue;
            }
            match self.template_default(p) {
                Some(def) => {
                    let _ = pi;
                    match self.default_template_arg(tmpl, depth, p, def, loc, &out) {
                        Ok(a) => out.push(a),
                        Err(()) => return Err(()),
                    }
                }
                None => {
                    self.diag_template_arity(tmpl, false, loc);
                    return Err(());
                }
            }
        }
        if wi < written.len() {
            self.diag_template_arity(tmpl, true, loc);
            return Err(());
        }
        if !ok {
            return Err(());
        }
        let sugared = self.ast.types.arg_list(&out);
        let canonical = self.ast.types.canonical_args(sugared);
        Ok(ConvertedArgs { sugared, canonical })
    }

    fn diag_template_arity(&self, tmpl: DeclId, too_many: bool, loc: Loc) {
        let name = self.qualified_decl_name(tmpl);
        let at = self
            .pattern_of(tmpl)
            .map_or_else(|| self.ast.decls.get(tmpl).loc(), |p| self.ast.decls.get(p).loc());
        let d = self
            .diag(DiagId::sema_template_arg_list_different_arity, loc)
            .arg(u32::from(too_many))
            .arg(template_kind_select(self.kind_of(tmpl)))
            .arg(name)
            .note(DiagId::note_template_decl_here, Span::empty(at));
        self.emit(d);
    }

    fn note_param_here(&self, d: kiln_diag::Diag, p: DeclId) -> kiln_diag::Diag {
        d.note(DiagId::note_template_param_here, Span::empty(self.ast.decls.get(p).loc()))
    }

    /// Checks and converts one written argument for parameter `p`
    /// (§3.2). `prior` holds the converted arguments of the parameters
    /// before `p` (a non-type parameter's type may name them).
    fn check_template_arg(
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
                match self.ast.types.template_decl(n).map(|d| self.kind_of(d)) {
                    Some(
                        DeclKind::ClassTemplate
                        | DeclKind::TypeAliasTemplate
                        | DeclKind::TemplateTemplateParm,
                    )
                    | None => Ok(a),
                    Some(_) => {
                        self.error(DiagId::sema_template_arg_must_be_template, at);
                        Err(())
                    }
                }
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
        let mut s = Substituter::new(self, &args);
        s.ty(ty, at).map_err(|_| ())
    }

    /// The levels for substituting into parameter `depth` of `tmpl`:
    /// outer levels retained, this level's `prior` arguments.
    fn prior_levels(&mut self, tmpl: DeclId, depth: u16, prior: &[TemplateArg]) -> MultiLevelArgs {
        let mut m = self.outer_levels(tmpl, depth);
        let l = self.ast.types.arg_list(prior);
        m.push(l, tmpl, LevelMode::Substitute);
        m
    }

    /// The levels enclosing `tmpl`'s own parameters (`depth` of them):
    /// the arguments of the class template specialization `tmpl` is a
    /// member of, else retained.
    pub(crate) fn outer_levels(&self, tmpl: DeclId, depth: u16) -> MultiLevelArgs {
        let mut m = MultiLevelArgs::new();
        for _ in 0..depth {
            m.push(ArgListId::EMPTY, tmpl, LevelMode::Retain);
        }
        m
    }

    /// `e` converted to the type of non-type parameter `p` as a
    /// converted constant expression ([temp.arg.nontype]/2) and
    /// evaluated: a `Value` argument.
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
        let mut pty = self.nttp_type(tmpl, depth, p, prior, at)?;
        if self.is_error_type(pty) {
            return Err(());
        }
        if self.ast.types.is_dependent(pty) {
            // `template<class T, T V>` with a dependent `T`: converted
            // when substituted.
            return Ok(self.builder().template_arg_expr(e));
        }
        let ety = self.ast.exprs.get(e).ty();
        if matches!(self.ast.types.canonical_view(pty), TypeView::Auto { .. }) {
            // `template<auto V>` ([temp.arg.nontype]/1): the type is
            // deduced from the argument (decayed, unqualified).
            let d = conv::decay(self, e);
            pty = conv::strip_cv(self, self.ast.exprs.get(d).ty());
        }
        let t = &self.ast.types;
        if t.is_reference(pty)
            || t.as_record(pty).is_some()
            || t.is_array(ety) && !t.is_pointer(pty)
        {
            self.templ_unimplemented("non-type template parameter of class or reference type", at);
            return Err(());
        }
        let pty = conv::strip_cv(self, pty);
        let c = conv::lvalue_to_rvalue(self, e);
        let cty = conv::strip_cv(self, self.ast.exprs.get(c).ty());
        let t = &self.ast.types;
        let int_like = |q| t.is_integral(q) || t.as_enum(q).is_some();
        if (int_like(pty) && t.is_floating(cty)) || (t.is_floating(pty) && int_like(cty)) {
            let d = self
                .diag(DiagId::sema_cce_conversion_not_allowed, at)
                .arg(self.ty_arg(ety))
                .arg(self.ty_arg(pty));
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
            self.emit(d);
            return Err(());
        }
        let c = match conv::implicit_convert(self, c, pty, ConvCtx::Initialization) {
            Ok(c) => c,
            Err(err) => {
                self.emit_conv_error(err, at);
                return Err(());
            }
        };
        let c = self.act_on_full_expr(c);
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
                    _ => v,
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

    /// The default argument of parameter `p`, substituted with the
    /// arguments before it ([temp.arg.general]/9) and converted.
    pub(crate) fn default_template_arg(
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
        let r = {
            let mut s = Substituter::new(self, &args);
            s.template_arg(def, at)
        };
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
