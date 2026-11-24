//! Pack expansion (spec/sema-templates.md §7): the expansion of a
//! pattern into its elements ([temp.variadic]/5–8), the length check,
//! and the per-list helpers the substituter's expansion sites call.
//!
//! The packs of a pattern and their lengths are found by a *probe*: the
//! pattern is substituted with no pack index; the first pack argument (or
//! instantiated function parameter pack) the substitution reaches records
//! its length and name and stops the probe. Each element is then
//! substituted with its index; a pack of another length reached while
//! expanding is the length-mismatch error. A probe that reaches no pack
//! (every pack belongs to a retained level) yields the substituted
//! pattern, still an expansion.

use kiln_ast::QualType;
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::RawId;
use kiln_ast::templates::Requirement;
use kiln_ast::types::{ArgView, TemplateArg, TypeKind, TypeView};
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::templ::subst::{SubstFail, SubstResult, Substituter};

/// The state of the innermost expansion in progress.
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent flags of one expansion")]
pub struct PackState {
    /// Probing for the length: the first pack reached stops the probe.
    pub probing: bool,
    /// What the probe found: the length and the pack's name.
    pub found: Option<(u32, PackName)>,
    /// The length of the expansion and the first pack's name.
    pub expanding: Option<(u32, PackName)>,
    /// The `...` of the expansion, where a length mismatch is reported.
    pub ellipsis: Option<Loc>,
    /// A pack element substituted since the flag was last cleared is a
    /// type that itself names an unexpanded pack of an enclosing
    /// template (`alias<T, U>...` with `alias`'s pack bound to `{T, U}`
    /// inside a template over the packs `T` and `U`).
    pub element_has_packs: bool,
    /// An element still names an unexpanded pack of a retained level
    /// (`is_convertible<_Up, _Tp>...` in a member template of
    /// `tuple<_Tp...>` with only `_Tp` known): the expansion is rebuilt
    /// with [`PackState::keep`] instead (§7.2).
    pub partial: bool,
    /// Rebuilding a partially substituted expansion: a substituted type
    /// parameter pack becomes a `SubstTemplateTypeParmPackType` naming
    /// its arguments, expanded with the retained packs later (clang's
    /// `TransformTemplateTypeParmType` with no pack index).
    pub keep: bool,
    /// The element substituted since the flag was last cleared came from
    /// a pack element that is itself a pack expansion (`Is...` bound to
    /// `index_sequence`'s pack `I`): the element is its pattern and the
    /// substituted element is re-formed as an expansion (clang's
    /// `getPackSubstitutedTemplateArgument` and the
    /// `containsUnexpandedParameterPack` re-wrap in `TransformTypes`).
    pub element_expansion: bool,
    /// Set on the enclosing state when an expansion finishes: per
    /// element, whether it must be re-formed as an expansion. Taken by
    /// the expansion site with [`Substituter::take_rewrap`].
    pub rewrap: Vec<bool>,
    /// The expansion site re-forms elements flagged in `rewrap`; only
    /// then is an expansion element stripped to its pattern.
    pub rewrap_ok: bool,
    /// The length sweep of an expansion whose first pack is empty
    /// ([temp.variadic]/7; §7.1): the pattern is substituted once more
    /// in keep mode so every other pack it names is reached; a pack of
    /// another length is the mismatch, a retained one makes the
    /// expansion partially substituted.
    pub sweep: bool,
    /// The sweep reached a pack of a level not substituted.
    pub sweep_retained: bool,
    /// The sweep reached a pack of another length: its length, name and
    /// where, diagnosed once the sweep ends.
    pub sweep_mismatch: Option<(u32, PackName, Loc)>,
    /// The length of the `__integer_pack(N)` node (WP-1667) the probe
    /// evaluated, so the elements do not evaluate `N` again.
    pub integer_pack: Option<(ExprId, u32)>,
}

/// A pack as the length-mismatch diagnostic names it: its name and,
/// for a template parameter pack, its depth.
#[derive(Clone, Debug, Default)]
pub struct PackName {
    /// The pack's name.
    pub name: String,
    /// The template depth of a template parameter pack.
    pub depth: Option<u16>,
    /// A pack substituted earlier (§7.2): one of the "outer parameter
    /// packs" of the diagnostic.
    pub outer: bool,
    /// The partially substituted pack ([`Substituter::partial_pack`]).
    pub partial: bool,
}

impl PackName {
    /// A pack substituted earlier, unnamed.
    pub fn outer() -> Self {
        PackName { outer: true, ..PackName::default() }
    }
}

/// The result of expanding one pattern.
#[derive(Debug)]
pub enum Expansion<T> {
    /// The elements, in order.
    Elements(Vec<T>),
    /// The pattern substituted but not expandable yet (its packs belong
    /// to a retained level).
    Unexpanded(T),
}

/// The pattern of a pack-expansion type.
pub(crate) fn type_pattern(s: &Sema<'_, '_>, q: QualType) -> Option<QualType> {
    match s.ast.types.view_if(q.ty(), TypeKind::PackExpansion) {
        Some(TypeView::PackExpansion { pattern, .. }) => Some(pattern),
        _ => None,
    }
}

/// The pattern of a `PackExpansionExpr`.
pub(crate) fn expr_pattern(s: &Sema<'_, '_>, e: ExprId) -> Option<ExprId> {
    let n = s.ast.exprs.get(e);
    (n.kind() == ExprKind::PackExpansionExpr).then(|| ExprId::new(n.a()))
}

impl Substituter<'_, '_, '_> {
    /// True while probing once the probe has found its pack: what remains
    /// of the pattern (the rest of a lambda body) is not substituted.
    pub(crate) fn probe_done(&self) -> bool {
        (self.pack.probing && self.pack.found.is_some()) || self.pack.sweep
    }

    /// The length sweep ([`PackState::sweep`]) reached a substituted pack
    /// of length `n` that it keeps: a non-empty one is the length
    /// mismatch with the empty first pack ([temp.variadic]/7).
    pub(crate) fn sweep_kept(
        &mut self,
        n: usize,
        name: impl FnOnce(&Self) -> PackName,
        loc: Loc,
    ) -> SubstResult<()> {
        if self.pack.sweep && n != 0 {
            return self.pack_reached(u32::try_from(n).unwrap_or(u32::MAX), name, loc).map(|_| ());
        }
        Ok(())
    }

    /// The length sweep substituted a pattern element `op`: a pack it
    /// still names by declaration is of a level not substituted (the
    /// substituted packs are kept as `SubstTemplateTypeParmPackType` and
    /// the like, which name none).
    fn sweep_note_retained(&mut self, op: PackOperand) {
        if self.pack.sweep && !self.sema.unexpanded_packs(op).is_empty() {
            self.pack.sweep_retained = true;
        }
    }

    /// A pack of length `n` named `name` reached during substitution:
    /// the index of the element to use. Probing records the pack and
    /// stops; expanding checks the length ([temp.variadic]/8).
    pub(crate) fn pack_reached(
        &mut self,
        n: u32,
        name: impl FnOnce(&Self) -> PackName,
        loc: Loc,
    ) -> SubstResult<u32> {
        if self.pack.probing {
            self.pack.found = Some((n, name(self)));
            return Err(SubstFail);
        }
        // The length sweep cannot substitute an element: an empty pack
        // ends it; another length is the mismatch below.
        if self.pack.sweep {
            if n != 0 && self.pack.sweep_mismatch.is_none() {
                self.pack.sweep_mismatch = Some((n, name(self), loc));
            }
            return Err(SubstFail);
        }
        let Some(i) = self.pack_index else {
            // [temp.variadic]/12: a pack reached outside every expansion
            // is named where no `...` expands it. Each such context is
            // diagnosed when the template is parsed (a base-specifier, an
            // enumerator value, a bit-field size, a fixed underlying
            // type, a using-declarator, `explicit` and `noexcept`
            // operands, a lambda's body); this reports one those checks
            // do not see, as they would, rather than leaving the
            // instantiation silently invalid.
            let n = name(self).name;
            let list =
                if n.is_empty() { " '__integer_pack'".to_owned() } else { format!(" '{n}'") };
            let d = self.sema.diag(DiagId::sema_unexpanded_pack, loc).arg("expression").arg(list);
            self.sema.emit(d);
            return Err(SubstFail);
        };
        if let Some((len, first)) = self.pack.expanding.clone()
            && len != n
        {
            let second = name(self);
            let at = self.pack.ellipsis.unwrap_or(loc);
            if first.partial || second.partial {
                // [temp.arg.explicit]/9: deduction may extend the
                // explicitly specified pack, so only a pack shorter than
                // it conflicts (clang's
                // `err_pack_expansion_length_conflict_partial`).
                let (named, at_least, other) =
                    if first.partial { (first.name, len, n) } else { (second.name, n, len) };
                if at_least > other {
                    let d = self
                        .sema
                        .diag(DiagId::sema_pack_length_mismatch_partial, at)
                        .arg(named)
                        .arg(at_least)
                        .arg(other);
                    self.sema.emit(d);
                } else {
                    // Deduction may yet extend it to the other's length:
                    // clang retains the expansion, partially substituted
                    // (§7.2), rather than failing.
                    self.pack.partial = true;
                }
                return Err(SubstFail);
            }
            // A pack of a shallower level than the other, or one
            // substituted earlier (§7.2), is an outer pack: clang
            // substitutes it first and reports
            // `err_pack_expansion_length_conflict_multilevel`.
            let first_outer =
                first.outer || matches!((first.depth, second.depth), (Some(a), Some(b)) if a < b);
            let second_outer =
                second.outer || matches!((first.depth, second.depth), (Some(a), Some(b)) if b < a);
            let d = if first_outer == second_outer {
                self.sema
                    .diag(DiagId::sema_pack_length_mismatch, at)
                    .arg(first.name)
                    .arg(second.name)
                    .arg(len)
                    .arg(n)
            } else {
                let (named, outer, inner) =
                    if first_outer { (second.name, len, n) } else { (first.name, n, len) };
                self.sema
                    .diag(DiagId::sema_pack_length_mismatch_outer, at)
                    .arg(named)
                    .arg(outer)
                    .arg(inner)
            };
            self.sema.emit(d);
            return Err(SubstFail);
        }
        if i >= n {
            return Err(SubstFail);
        }
        Ok(i)
    }

    /// The element of pack argument `a` for the current index (`a`
    /// itself when it is not a pack).
    pub(crate) fn pack_element(
        &mut self,
        a: TemplateArg,
        name: impl FnOnce(&Self) -> PackName,
        loc: Loc,
    ) -> SubstResult<TemplateArg> {
        match a.view() {
            ArgView::Pack(l) => {
                let n = u32::try_from(self.sema.ast.types.args(l).len()).unwrap_or(u32::MAX);
                let i = self.pack_reached(n, name, loc)?;
                let mut e = self.sema.ast.types.args(l)[i as usize];
                if self.pack.rewrap_ok
                    && let Some(p) = self.expansion_pattern_arg(e)
                {
                    e = p;
                    self.pack.element_expansion = true;
                    self.pack.element_has_packs = true;
                }
                if let ArgView::Type(t) = e.view()
                    && self.sema.has_unexpanded_pack(t)
                {
                    self.pack.element_has_packs = true;
                }
                Ok(e)
            }
            _ => Ok(a),
        }
    }

    /// The pattern of a pack-expansion argument (`T...`, `e...`, `TT...`).
    fn expansion_pattern_arg(&mut self, a: TemplateArg) -> Option<TemplateArg> {
        match a.view() {
            ArgView::Type(q) => type_pattern(self.sema, q).map(TemplateArg::ty),
            ArgView::Expr { expr, .. } => {
                expr_pattern(self.sema, expr).map(|p| self.sema.ast.builder().template_arg_expr(p))
            }
            ArgView::TemplateExpansion { name, .. } => Some(TemplateArg::template(name)),
            _ => None,
        }
    }

    /// Which elements of the expansion that just finished must be
    /// re-formed as expansions (their pack element was one); empty when
    /// none.
    pub(crate) fn take_rewrap(&mut self) -> Vec<bool> {
        std::mem::take(&mut self.pack.rewrap)
    }

    /// Expands a pattern (§7.2): `f` substitutes the pattern once, under
    /// the current pack index.
    pub fn expand_pattern<T>(
        &mut self,
        f: impl FnMut(&mut Self) -> SubstResult<T>,
    ) -> SubstResult<Expansion<T>> {
        self.expand_pattern_at(None, f)
    }

    /// [`Self::expand_pattern`] for the expansion whose `...` is at
    /// `ellipsis`.
    pub fn expand_pattern_at<T>(
        &mut self,
        ellipsis: Option<Loc>,
        f: impl FnMut(&mut Self) -> SubstResult<T>,
    ) -> SubstResult<Expansion<T>> {
        self.expand_pattern_impl(ellipsis, false, f)
    }

    /// [`Self::expand_pattern_at`] for a site that re-forms the elements
    /// [`Self::take_rewrap`] flags as expansions: a pack element that is
    /// itself an expansion is substituted as its pattern.
    pub(crate) fn expand_pattern_rewrap<T>(
        &mut self,
        ellipsis: Option<Loc>,
        f: impl FnMut(&mut Self) -> SubstResult<T>,
    ) -> SubstResult<Expansion<T>> {
        self.expand_pattern_impl(ellipsis, true, f)
    }

    fn expand_pattern_impl<T>(
        &mut self,
        ellipsis: Option<Loc>,
        rewrap_ok: bool,
        mut f: impl FnMut(&mut Self) -> SubstResult<T>,
    ) -> SubstResult<Expansion<T>> {
        let mut rewrap = Vec::new();
        let saved_state = std::mem::take(&mut self.pack);
        self.pack.ellipsis = ellipsis;
        self.pack.rewrap_ok = rewrap_ok;
        let saved_index = self.pack_index.take();
        self.pack.probing = true;
        let outer = self.sema.lambda_mangle.begin_probe();
        let scope = self.sema.open_discard();
        let probe = f(self);
        let found = self.pack.found.take();
        // A probe that found a pack is thrown away, with what it made
        // invalid (spec/sema-templates.md §5.7.3).
        self.sema.close_discard(scope, found.is_some());
        // A probe that found a pack is discarded: so are the mangling
        // numbers of the closure types it created (clang substitutes the
        // pattern only per element).
        self.sema.lambda_mangle.end_probe(outer, found.is_some());
        self.pack.probing = false;
        let r = match (probe, found) {
            (Ok(x), None) => Ok(Expansion::Unexpanded(x)),
            (Err(e), None) => Err(e),
            // A probe that reached a pack but still succeeded: the failure
            // was recovered from inside a statement of a lambda body in
            // the pattern (`[] { return sizeof(T); }()...`).
            (_, Some((0, name))) => self.sweep_empty(ellipsis, rewrap_ok, name, &mut f),
            (_, Some((n, name))) => {
                // The explicitly given pack deduction may still extend
                // ([`Substituter::partial_pack`]): its elements are each
                // substituted (and so checked), but the expansion is
                // retained, partially substituted, as clang retains it,
                // so `X<Ts...>` is not formed with the given elements
                // alone (WP-3380).
                let retain = name.partial;
                self.pack.expanding = Some((n, name));
                let mut out = Vec::with_capacity(n as usize);
                let mut res = Ok(());
                for k in 0..n {
                    self.pack_index = Some(k);
                    self.pack.element_expansion = false;
                    match f(self) {
                        Ok(x) => {
                            out.push(x);
                            if self.pack.element_expansion {
                                rewrap.resize(out.len() - 1, false);
                                rewrap.push(true);
                            }
                        }
                        Err(e) => {
                            res = Err(e);
                            break;
                        }
                    }
                }
                self.sema.templ.pack_elements += u64::from(n);
                if (res.is_err() && self.pack.partial) || (res.is_ok() && retain) {
                    // A partially substituted expansion (§7.2): the
                    // pattern rebuilt once, still an expansion.
                    self.pack = PackState { ellipsis, keep: true, ..PackState::default() };
                    self.pack_index = None;
                    f(self).map(Expansion::Unexpanded)
                } else {
                    res.map(|()| Expansion::Elements(out))
                }
            }
        };
        self.pack = saved_state;
        self.pack_index = saved_index;
        self.pack.rewrap =
            if matches!(r, Ok(Expansion::Elements(_))) { rewrap } else { Vec::new() };
        r
    }

    /// An expansion whose probe reached an empty pack first
    /// ([temp.variadic]/7; §7.1): no elements, unless the length sweep
    /// finds another pack of a different length (the mismatch) or a pack
    /// of a level not substituted (the expansion stays, partially
    /// substituted, as one whose element failed would).
    fn sweep_empty<T>(
        &mut self,
        ellipsis: Option<Loc>,
        rewrap_ok: bool,
        name: PackName,
        f: &mut impl FnMut(&mut Self) -> SubstResult<T>,
    ) -> SubstResult<Expansion<T>> {
        self.pack = PackState {
            ellipsis,
            rewrap_ok,
            keep: true,
            sweep: true,
            expanding: Some((0, name.clone())),
            ..PackState::default()
        };
        // What the sweep substitutes is discarded, and so are its
        // diagnostics, also those of the checking frames it pushes (the
        // prior-argument substitution of `ic<T, v>` with `T` = void and an
        // empty `v`, which clang never forms; WP-1222): only the mismatch
        // is reported, as expanding reports it.
        let held = self.sema.open_capture(true);
        let swept = f(self);
        drop(self.sema.close_capture(held));
        if let Some((n, second, loc)) = self.pack.sweep_mismatch.take() {
            self.pack = PackState { ellipsis, expanding: Some((0, name)), ..PackState::default() };
            let saved_index = self.pack_index.replace(0);
            let r = self.pack_reached(n, |_| second, loc);
            self.pack_index = saved_index;
            return r.and(Err(SubstFail));
        }
        if swept.is_ok() && self.pack.sweep_retained {
            self.pack = PackState { ellipsis, keep: true, ..PackState::default() };
            return f(self).map(Expansion::Unexpanded);
        }
        Ok(Expansion::Elements(Vec::new()))
    }

    /// Expands a list of pattern elements (§12 rule 2): an element that
    /// is a pack expansion (`pattern_of` gives its pattern) is replaced
    /// by its elements, each substituted by `each`; an expansion that
    /// cannot be expanded yet is rebuilt by `rewrap`.
    pub fn expand_list<T: Copy>(
        &mut self,
        items: &[T],
        pattern_of: impl Fn(&Sema<'_, '_>, T) -> Option<T>,
        mut each: impl FnMut(&mut Self, T) -> SubstResult<T>,
        rewrap: impl Fn(&mut Self, T) -> T,
    ) -> SubstResult<Vec<T>> {
        let mut out = Vec::with_capacity(items.len());
        for &x in items {
            if let Some(p) = pattern_of(self.sema, x) {
                match self.expand_pattern_rewrap(None, |s| each(s, p))? {
                    Expansion::Elements(v) => {
                        let rw = self.take_rewrap();
                        for (k, y) in v.into_iter().enumerate() {
                            out.push(if rw.get(k) == Some(&true) { rewrap(self, y) } else { y });
                        }
                    }
                    Expansion::Unexpanded(y) => out.push(rewrap(self, y)),
                }
            } else {
                out.push(each(self, x)?);
            }
        }
        Ok(out)
    }

    /// Substitutes into a list of types that may contain pack
    /// expansions.
    pub fn expand_types(&mut self, tys: &[QualType], loc: Loc) -> SubstResult<Vec<QualType>> {
        self.expand_list(
            tys,
            type_pattern,
            |s, q| s.element_ty(q, loc),
            |s, q| s.sema.ast.types.pack_expansion(q, None),
        )
    }

    /// One element of a type pack expansion: an element that still names
    /// an unexpanded pack (of a level not substituted, `tl<Ts, Us>...`
    /// with only `Ts` known) fails and marks the expansion partially
    /// substituted, rebuilt by [`Self::expand_pattern_at`] (§7.2). An
    /// element whose packs came from the pack's own elements (the
    /// arguments name packs of an enclosing template) is complete.
    pub(crate) fn element_ty(&mut self, q: QualType, loc: Loc) -> SubstResult<QualType> {
        let saved = std::mem::take(&mut self.pack.element_has_packs);
        let t = self.ty(q, loc);
        let from_elements = std::mem::replace(&mut self.pack.element_has_packs, saved);
        let t = t?;
        self.sweep_note_retained(PackOperand::Type(t));
        if self.pack_index.is_some()
            && !from_elements
            && self.sema.ast.types.qflags(t).contains(kiln_ast::types::TypeFlags::UNEXPANDED_PACK)
        {
            self.pack.partial = true;
            return Err(SubstFail);
        }
        Ok(t)
    }

    /// One element of an expression pack expansion: an element that
    /// still names an unexpanded pack of a level not substituted
    /// (`(idx * s[P]) + ...` in a generic lambda of a function template
    /// over the pack `idx`, with only `idx` known) fails and marks the
    /// expansion partially substituted, rebuilt by
    /// [`Self::expand_pattern_at`] with the substituted pack kept (§7.2).
    pub(crate) fn element_expr(&mut self, e: ExprId) -> SubstResult<ExprId> {
        let saved = std::mem::take(&mut self.pack.element_has_packs);
        let r = self.expr(e);
        let from_elements = std::mem::replace(&mut self.pack.element_has_packs, saved);
        let r = r?;
        self.sweep_note_retained(PackOperand::Expr(r));
        if self.pack_index.is_some()
            && !from_elements
            && self.sema.ast.exprs.get(r).deps().contains(kiln_ast::expr::ExprDeps::UNEXPANDED_PACK)
        {
            self.pack.partial = true;
            return Err(SubstFail);
        }
        Ok(r)
    }

    /// Substitutes into a list of expressions that may contain pack
    /// expansions.
    pub fn expand_exprs(&mut self, es: &[ExprId]) -> SubstResult<Vec<ExprId>> {
        let mut out = Vec::with_capacity(es.len());
        for &x in es {
            let Some(p) = expr_pattern(self.sema, x) else {
                out.push(self.expr(x)?);
                continue;
            };
            // A `PackExpansionExpr` is located at its `...`.
            let ellipsis = self.sema.ast.exprs.get(x).loc();
            match self.expand_pattern_rewrap(Some(ellipsis), |s| s.element_expr(p))? {
                Expansion::Elements(v) => {
                    let rw = self.take_rewrap();
                    for (k, y) in v.into_iter().enumerate() {
                        let y = if rw.get(k) == Some(&true) {
                            self.sema.act_pack_expansion(y, ellipsis)
                        } else {
                            y
                        };
                        out.push(y);
                    }
                }
                Expansion::Unexpanded(y) => out.push(self.sema.act_pack_expansion(y, ellipsis)),
            }
        }
        Ok(out)
    }

    /// Maps the parameters `pp` of a pattern function to the parameters
    /// `fp` of its instantiation: a function parameter pack takes the
    /// parameters its expansion produced (§7.1).
    pub(crate) fn map_params(&mut self, pp: &[DeclId], fp: &[DeclId]) {
        use crate::templ::subst::DeclOrPack;
        let is_pack = |s: &Self, p: DeclId| {
            s.sema
                .ast
                .decls
                .param(p)
                .is_some_and(|x| x.flags & kiln_ast::decl::ParamData::PACK != 0)
        };
        let packs = pp.iter().filter(|&&p| is_pack(self, p)).count();
        let extra = (fp.len() + packs).saturating_sub(pp.len());
        // With several packs (`Ts&&... ts, Us&&... us`, `Ts` the enclosing
        // class template's and `Us` deduced; libstdc++'s
        // `__tuple_concater::_S_do`) the parameter count does not say
        // where one ends: each takes the length of its own expansion.
        let lens: Vec<Option<usize>> = if packs > 1 {
            pp.iter()
                .map(|&p| if is_pack(self, p) { self.param_pack_len(p) } else { None })
                .collect()
        } else {
            Vec::new()
        };
        let mut j = 0;
        for (k, &a) in pp.iter().enumerate() {
            // A pack whose new parameter is still a pack (a generic
            // lambda's, rebuilt with its levels retained) maps one to one.
            let retained = fp.get(j).is_some_and(|&b| is_pack(self, b));
            let len = match lens.get(k) {
                Some(&n) => n,
                None if is_pack(self, a) && packs == 1 && !retained => Some(extra),
                None => None,
            };
            if let Some(n) = len {
                let end = (j + n).min(fp.len());
                let l = self.sema.ast.lists.push(&fp[j..end]);
                self.locals.insert(a, DeclOrPack::Pack(l));
                j = end;
            } else if let Some(&b) = fp.get(j) {
                self.locals.insert(a, DeclOrPack::Decl(b));
                j += 1;
            }
        }
    }

    /// The number of parameters pattern function parameter pack `p`
    /// expands to under the current arguments: its type's pattern probed
    /// for the pack it names, as [`Self::expand_pattern`] does. `None`
    /// when no pack is substituted (one retained, or `p` no expansion).
    pub(crate) fn param_pack_len(&mut self, p: DeclId) -> Option<usize> {
        let pd = self.sema.ast.decls.param(p).copied();
        let t = pd.and_then(|x| x.original_ty).unwrap_or_else(|| self.decl_ty(p));
        let pat = type_pattern(self.sema, t)?;
        let loc = self.sema.ast.decls.get(p).loc();
        let saved_state = std::mem::take(&mut self.pack);
        let saved_index = self.pack_index.take();
        self.pack.probing = true;
        let scope = self.sema.open_discard();
        let _ = self.element_ty(pat, loc);
        self.sema.close_discard(scope, true);
        let found = self.pack.found.take();
        self.pack = saved_state;
        self.pack_index = saved_index;
        found.map(|(n, _)| n as usize)
    }

    /// Gives the parameters [`Self::map_params`] mapped the names of
    /// their pattern parameters `pp` (an expanded pack's elements all
    /// take the pack's name), as clang `addInstantiatedParametersToScope`
    /// does when it defines a function from a definition whose
    /// parameters are named differently from the declaration's.
    pub(crate) fn rename_params_from_pattern(&mut self, pp: &[DeclId]) {
        use crate::templ::subst::DeclOrPack;
        for &a in pp {
            let name = self.sema.ast.decls.get(a).name();
            match self.locals.get(&a).copied() {
                Some(DeclOrPack::Decl(b)) => self.sema.ast.decls.set_name(b, name),
                Some(DeclOrPack::Pack(l)) => {
                    let elems: Vec<DeclId> = self.sema.ast.lists.iter(l).collect();
                    for b in elems {
                        self.sema.ast.decls.set_name(b, name);
                    }
                }
                _ => {}
            }
        }
    }

    /// A fold-expression (§7.3, [temp.variadic]/11): the elements of its
    /// pattern combined by `act_on_binary` in a loop (no recursion per
    /// element, R-WALK), wrapped in a `ParenExpr`. Each combination, and a
    /// fold rebuilt over an unexpanded slice, takes the non-member
    /// candidates the fold's definition found ([`Self::fold_combine`],
    /// [temp.dep.candidate]/1), not a lookup at the instantiation.
    pub(crate) fn fold(&mut self, e: ExprId, loc: Loc, end: Loc) -> SubstResult<ExprId> {
        let n = *self.sema.ast.exprs.get(e);
        let op = kiln_ast::expr::BinaryOp::from_u8(n.aux()).ok_or(SubstFail)?;
        let lhs = ExprId::from_raw(n.a());
        let rhs = ExprId::from_raw(n.b());
        let has_pack = |s: &Self, x: Option<ExprId>| {
            x.is_some_and(|x| {
                s.sema.ast.exprs.get(x).deps().contains(kiln_ast::expr::ExprDeps::UNEXPANDED_PACK)
            })
        };
        // A right fold has its pattern on the left: `(E op ...)`.
        let right = has_pack(self, lhs);
        let (pattern, init) = if right { (lhs, rhs) } else { (rhs, lhs) };
        let pattern = pattern.ok_or(SubstFail)?;
        let ellipsis = self.sema.templ.fold_ellipsis.get(&e).copied().unwrap_or(loc);
        let init = self.opt_expr(init)?;
        // An element substituted from a pack element that is itself an
        // expansion (an alias guide's `Ts` bound to `{Ts'...}`) still
        // names its pack: it stays a fold over that slice, combined with
        // the rest (clang `TransformCXXFoldExpr`, "retain a pack
        // expansion for this slice").
        let elems = match self.expand_pattern_rewrap(Some(ellipsis), |s| s.element_expr(pattern))? {
            Expansion::Elements(v) => v,
            Expansion::Unexpanded(p) => {
                let (l, r) = if right { (Some(p), init) } else { (init, Some(p)) };
                return Ok(self.fold_combine(e, |s| s.act_fold(l, op, r, loc, ellipsis, end)));
            }
        };
        let rw = self.take_rewrap();
        let mut it: Box<dyn Iterator<Item = (ExprId, bool)>> = {
            let tagged =
                elems.into_iter().enumerate().map(move |(k, x)| (x, rw.get(k) == Some(&true)));
            if right {
                Box::new(tagged.collect::<Vec<_>>().into_iter().rev())
            } else {
                Box::new(tagged)
            }
        };
        let mut acc = init;
        // A fold-expression prints its own parentheses.
        let mut folded = false;
        // Whether an operator was applied (or a slice kept as a fold): a
        // specialization whose expansion applies none still reached the
        // fold's operator tokens (spec/ast.md §18.13.6).
        let mut applied = false;
        if acc.is_none() {
            match it.next() {
                Some((x, false)) => acc = Some(x),
                Some((x, true)) => {
                    let (l, r) = if right { (Some(x), None) } else { (None, Some(x)) };
                    acc = Some(self.fold_combine(e, |s| s.act_fold(l, op, r, loc, ellipsis, end)));
                    folded = true;
                    applied = true;
                }
                None => {}
            }
        }
        let Some(mut acc) = acc else {
            use kiln_ast::expr::BinaryOp;
            if matches!(op, BinaryOp::LAnd | BinaryOp::LOr | BinaryOp::Comma) {
                self.fold_applied_none(e);
            }
            return match op {
                BinaryOp::LAnd => Ok(self.sema.act_on_bool_literal(true, ellipsis)),
                BinaryOp::LOr => Ok(self.sema.act_on_bool_literal(false, ellipsis)),
                BinaryOp::Comma => {
                    Ok(self.sema.act_on_value_init_cast(QualType::VOID, ellipsis, ellipsis))
                }
                _ => {
                    let d = self.sema.diag(DiagId::sema_fold_empty, ellipsis).arg(op.spelling());
                    self.sema.emit(d);
                    Err(SubstFail)
                }
            };
        };
        for (x, slice) in it {
            self.check_stack(|_| loc)?;
            folded = slice;
            applied = true;
            acc = self.fold_combine(e, |s| match (slice, right) {
                (true, true) => s.act_fold(Some(x), op, Some(acc), loc, ellipsis, end),
                (true, false) => s.act_fold(Some(acc), op, Some(x), loc, ellipsis, end),
                (false, true) => s.act_on_binary(op, x, acc, ellipsis),
                (false, false) => s.act_on_binary(op, acc, x, ellipsis),
            });
            if self.sema.is_err_expr(acc) {
                return Err(SubstFail);
            }
        }
        if !applied {
            // One element and no init: the element itself
            // ([temp.variadic]/13).
            self.fold_applied_none(e);
        }
        if folded {
            return Ok(acc);
        }
        Ok(self.sema.act_on_paren(acc, loc, end))
    }

    /// Runs `f`, one combination of the elements of the fold-expression
    /// `e` of the pattern, with the operator candidates `e`'s definition
    /// recorded (`act_fold`, spec/ast.md §8.5) as the override an
    /// operator expression rebuilt by a substitution uses.
    /// The combination's resolution is a per-specialization row of the
    /// fold's operator entries (spec/ast.md §18.13.6).
    fn fold_combine(&mut self, e: ExprId, f: impl FnOnce(&mut Sema) -> ExprId) -> ExprId {
        self.sema.oper_override = self.sema.oper_rebuild_set(e);
        let r = f(self.sema);
        self.sema.oper_override = None;
        if self.sema.ast.written_refs.has_pattern_nodes() {
            self.sema.note_written_spec_fold(e, r);
        }
        r
    }

    /// The expansion of fold-expression `e` applied no operator (an empty
    /// pack's `&&`, `||` or `,`, one element without an init, an init
    /// alone): its operator entries get a `SPEC_BUILTIN` row
    /// (spec/ast.md §18.13.6).
    fn fold_applied_none(&mut self, e: ExprId) {
        if self.sema.ast.written_refs.has_pattern_nodes() {
            self.sema.note_written_spec_fold_none(e);
        }
    }

    /// `pack...[index]` ([expr.prim.pack.index]): the selected element.
    /// clang keeps a `PackIndexingExpr` around it; kiln records the
    /// element in `pack_index_origin` (the pattern's node, for mangling)
    /// and in `pack_index_written` (the substituted index, its written
    /// form; spec/sema-templates.md §7.4, WP-3582).
    pub(crate) fn pack_indexing_expr(
        &mut self,
        e: ExprId,
        loc: Loc,
        end: Loc,
    ) -> SubstResult<ExprId> {
        let n = *self.sema.ast.exprs.get(e);
        let pattern = ExprId::from_raw(n.a()).ok_or(SubstFail)?;
        let index = ExprId::from_raw(n.b()).ok_or(SubstFail)?;
        let index = self.expr(index)?;
        if self.sema.expr_is_value_dependent(index) {
            let p = self.kept_pack_pattern(|s| s.expr(pattern))?;
            return Ok(self.sema.act_pack_indexing(p, loc, index, end));
        }
        match self.expand_pattern(|s| s.expr(pattern))? {
            Expansion::Unexpanded(p) => Ok(self.sema.act_pack_indexing(p, loc, index, end)),
            Expansion::Elements(v) => {
                // clang 19 names the pack expression unquoted, clang 22
                // quoted.
                let mut name = if self.sema.ast.exprs.get(pattern).kind() == ExprKind::DeclRefExpr {
                    let d = self.sema.ast.exprs.decl(pattern);
                    self.sema.decl_name_str(self.sema.ast.decls.get(d).name())
                } else {
                    "<expression>".to_owned()
                };
                if self.sema.clang_compat >= 22 {
                    name = format!("'{name}'");
                }
                let i = self.pack_index_value(index, v.len(), &name, loc)?;
                // The element was built for this pack index alone; it
                // stands for clang's non-dependent `PackIndexingExpr`,
                // which a dependent signature still mangles as nothing
                // (`Y<1, 2>::g`'s `decltype(u + I...[1])` is `DTplfp_E`).
                let _ = self.sema.ast.templates.pack_index_origin.insert(v[i], e);
                let _ = self.sema.ast.templates.pack_index_written.insert(v[i], index);
                Ok(v[i])
            }
        }
    }

    /// `T...[index]` ([dcl.type.pack.index]): the selected element, kept
    /// as `PackIndexing` sugar over it with the pattern substituted in
    /// keep mode (its packs named by their elements, clang's
    /// `SubstTemplateTypeParmPackType`) and the substituted index, as
    /// clang's `TransformPackIndexingType` builds it (spec/sema-templates.md
    /// §7.4, WP-3582).
    pub(crate) fn pack_indexing_type(
        &mut self,
        pattern: QualType,
        index: ExprId,
        loc: Loc,
    ) -> SubstResult<QualType> {
        let index = self.expr(index)?;
        if self.sema.expr_is_value_dependent(index) {
            let p = self.kept_pack_pattern(|s| s.element_ty(pattern, loc))?;
            let at = self.sema.begin_loc(index);
            return Ok(self.sema.act_pack_indexing_type(p, at, index));
        }
        match self.expand_pattern(|s| s.element_ty(pattern, loc))? {
            Expansion::Unexpanded(p) => {
                let at = self.sema.begin_loc(index);
                Ok(self.sema.act_pack_indexing_type(p, at, index))
            }
            Expansion::Elements(v) => {
                let name = format!("'{}'", self.sema.ty_str(pattern));
                let at = self.sema.begin_loc(index);
                let i = self.pack_index_value(index, v.len(), &name, at)?;
                let p = self.kept_pack_pattern(|s| s.element_ty(pattern, loc)).unwrap_or(pattern);
                Ok(self.sema.ast.types.pack_indexing(p, index, None, Some(v[i])))
            }
        }
    }

    /// The pattern of a pack index that stays value-dependent (a generic
    /// lambda's `xs...[I]` with `I` its own parameter and `xs` already
    /// known, §7.4): substituted once in keep mode (§7.2), so a known
    /// pack becomes a `FunctionParmPackExpr` /
    /// `SubstNonTypeTemplateParmPackExpr` / `SubstTemplateTypeParmPackType`
    /// naming its elements, and the index selects among them when the
    /// retained level is instantiated.
    pub(crate) fn kept_pack_pattern<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> SubstResult<T>,
    ) -> SubstResult<T> {
        let saved =
            std::mem::replace(&mut self.pack, PackState { keep: true, ..PackState::default() });
        let saved_index = self.pack_index.take();
        let r = f(self);
        self.pack = saved;
        self.pack_index = saved_index;
        r
    }

    /// The value of a substituted pack index, checked against the pack's
    /// length `len` (clang: "invalid index 3 for pack 'T' of size 2").
    fn pack_index_value(
        &mut self,
        index: ExprId,
        len: usize,
        name: &str,
        at: Loc,
    ) -> SubstResult<usize> {
        use kiln_oven::{EvalContext, EvalKind};
        let c = self.sema.pack_index_operand(index).ok_or(SubstFail)?;
        let loc = self.sema.begin_loc(index);
        // clang checks the index as a converted constant expression of
        // the array-size kind ("array size is not a constant expression").
        let v = match self.sema.eval_int(c, EvalContext::new(EvalKind::Other, loc)) {
            Ok(v) => v,
            Err(f) => {
                let d = self.sema.diag(DiagId::sema_expr_not_cce, loc).arg("array size");
                let d = Sema::with_failure_notes(d, f);
                self.sema.emit(d);
                return Err(SubstFail);
            }
        };
        match usize::try_from(v) {
            Ok(i) if i < len => Ok(i),
            _ => {
                let d =
                    self.sema.diag(DiagId::sema_pack_index_invalid, at).arg(v).arg(name).arg(len);
                self.sema.emit(d);
                Err(SubstFail)
            }
        }
    }

    /// `sizeof...(P)` (§7.4): the length of the substituted pack, a
    /// `SizeOfPackExpr` that is no longer dependent.
    pub(crate) fn size_of_pack(&mut self, e: ExprId, loc: Loc, end: Loc) -> SubstResult<ExprId> {
        use crate::templ::args::ArgLookup;
        use crate::templ::subst::DeclOrPack;
        let n = *self.sema.ast.exprs.get(e);
        let d = DeclId::from_raw(n.a()).ok_or(SubstFail)?;
        // A structured binding pack whose decomposition failed.
        if self.failed_locals.contains(&d) {
            return Err(SubstFail);
        }
        let len = if let Some(&DeclOrPack::Pack(l)) = self.locals.get(&d) {
            self.sema.ast.lists.len(l)
        } else if let Some(&DeclOrPack::Decl(x)) = self.locals.get(&d) {
            // A function parameter pack that stays one (a generic
            // lambda's, its own levels retained).
            let st = self.sema.size_type();
            return Ok(self.sema.builder().size_of_pack(x, st, loc, end));
        } else if let Some(info) = self.sema.ast.template_parm(d).copied() {
            match self.lookup(info.depth, info.index) {
                ArgLookup::Arg(a) => match a.view() {
                    ArgView::Pack(l) => {
                        let elems = self.sema.ast.types.args(l).to_vec();
                        if elems.iter().any(|&x| self.sema.is_pack_expansion_arg(x)) {
                            return self.size_of_partial_pack(&elems, e, loc, end);
                        }
                        elems.len()
                    }
                    // An argument not grouped into a pack stands for a
                    // pack of itself, as `pack_element` substitutes it;
                    // an expansion for the pack it expands.
                    _ if self.sema.is_pack_expansion_arg(a) => {
                        return self.size_of_partial_pack(&[a], e, loc, end);
                    }
                    _ => 1,
                },
                ArgLookup::Retain(_) => return Ok(e),
                ArgLookup::Missing => return Err(SubstFail),
            }
        } else {
            return Ok(e);
        };
        // Gap (spec §7.4): an `IntegerLiteral` of type `size_t` stands
        // for the substituted `SizeOfPackExpr` (its dump differs).
        let _ = end;
        let st = self.sema.size_type();
        let lit = self.int_literal(st, len as i128, loc);
        let _ = self.sema.ast.templates.size_of_pack_origin.insert(lit, d);
        Ok(lit)
    }
}

impl Substituter<'_, '_, '_> {
    /// `sizeof...` of a pack bound to elements some of which are pack
    /// expansions (`_Args` := `{int, Ts...}`): still value-dependent, the
    /// number of the other elements plus `sizeof...` of the first pack
    /// each expansion names (clang's partially substituted
    /// `SizeOfPackExpr`).
    fn size_of_partial_pack(
        &mut self,
        elems: &[TemplateArg],
        e: ExprId,
        loc: Loc,
        end: Loc,
    ) -> SubstResult<ExprId> {
        let st = self.sema.size_type();
        let mut fixed = 0i128;
        let mut terms = Vec::new();
        for &x in elems {
            if !self.sema.is_pack_expansion_arg(x) {
                fixed += 1;
                continue;
            }
            let packs = match x.view() {
                ArgView::Type(q) => type_pattern(self.sema, q)
                    .map(|t| self.sema.unexpanded_packs(PackOperand::Type(t))),
                ArgView::Expr { expr, .. } => expr_pattern(self.sema, expr)
                    .map(|p| self.sema.unexpanded_packs(PackOperand::Expr(p))),
                _ => None,
            };
            // An expansion whose pack cannot be named keeps the
            // expression as written.
            let Some(&pk) = packs.as_ref().and_then(|v| v.first()) else { return Ok(e) };
            terms.push(self.sema.builder().size_of_pack(pk, st, loc, end));
        }
        let mut acc = if fixed > 0 { Some(self.int_literal(st, fixed, loc)) } else { None };
        for t in terms {
            acc = Some(match acc {
                None => t,
                Some(l) => self.sema.builtin_binary(kiln_ast::expr::BinaryOp::Add, l, t, loc),
            });
        }
        let acc = acc.ok_or(SubstFail)?;
        // The partial arguments, for the mangling `sP <template-arg>* E`
        // (clang's `isPartiallySubstituted`, spec/abi.md §8.15).
        let partial = self.sema.ast.types.arg_list(elems);
        let _ = self.sema.ast.templates.size_of_pack_partial.insert(acc, partial);
        Ok(acc)
    }
}

/// An operand checked for unexpanded packs ([temp.variadic]/12).
#[derive(Clone, Copy)]
pub(crate) enum PackOperand {
    Expr(ExprId),
    Type(QualType),
    /// A nested-name-specifier (`Ts::` of a using-declarator).
    Nns(kiln_ast::NnsId),
}

impl Sema<'_, '_> {
    /// [temp.variadic]/12: an operand that names a pack outside any
    /// expansion is ill-formed. Reports "`what` contains unexpanded
    /// parameter pack 'Ts'" at `at` (clang's `DiagnoseUnexpandedParameterPack`,
    /// reported at the operand's begin) and returns true.
    pub(crate) fn diagnose_unexpanded_pack(
        &mut self,
        op: PackOperand,
        what: &'static str,
        at: Loc,
    ) -> bool {
        if !self.operand_has_packs(op) {
            return false;
        }
        let packs = self.unexpanded_packs(op);
        self.report_unexpanded_packs(&packs, what, at);
        true
    }

    /// True if `op` is marked as naming an unexpanded pack.
    pub(crate) fn operand_has_packs(&self, op: PackOperand) -> bool {
        match op {
            PackOperand::Expr(e) => {
                self.ast.exprs.get(e).deps().contains(kiln_ast::expr::ExprDeps::UNEXPANDED_PACK)
            }
            PackOperand::Type(t) => self.has_unexpanded_pack(t),
            PackOperand::Nns(n) => {
                self.ast.types.nns_flags(n).contains(kiln_ast::types::TypeFlags::UNEXPANDED_PACK)
            }
        }
    }

    /// The packs `op` names outside any expansion, in order (a
    /// lambda-expression's are those its body and captures name,
    /// [`Sema::lambda_unexpanded`]).
    pub(crate) fn unexpanded_packs(&self, op: PackOperand) -> Vec<DeclId> {
        let mut packs = Vec::new();
        if self.operand_has_packs(op) {
            match op {
                PackOperand::Expr(e) => self.expr_packs(e, &mut packs, 0),
                PackOperand::Type(t) => self.type_packs(t, &mut packs, 0),
                PackOperand::Nns(n) => self.nns_packs(n, &mut packs, 0),
            }
        }
        packs
    }

    /// "`what` contains unexpanded parameter pack[s] 'P'..." at `at`.
    pub(crate) fn report_unexpanded_packs(
        &mut self,
        packs: &[DeclId],
        what: &'static str,
        at: Loc,
    ) {
        let names: Vec<String> = packs.iter().map(|&d| self.decl_name(d)).collect();
        let quoted: Vec<String> = names.iter().map(|n| format!("'{n}'")).collect();
        let list = match quoted.as_slice() {
            // The one unexpanded pack that is no declaration: GCC's
            // built-in `__integer_pack(N)` (WP-1667).
            [] => " '__integer_pack'".to_owned(),
            [a] => format!(" {a}"),
            [a, b] => format!("s {a} and {b}"),
            [a, b, ..] => format!("s {a}, {b}, ..."),
        };
        let d = self.diag(DiagId::sema_unexpanded_pack, at).arg(what).arg(list);
        self.emit(d);
    }

    /// [temp.variadic]/12 for the type of a declarator: `Ts x;`,
    /// `Ts *p;`, `Ts f();`, `void g(Ts);` (`what` = "declaration type";
    /// a non-static data member's, "data member type"; a non-type
    /// template parameter's, "non-type template parameter type"),
    /// reported at the declarator-id (clang's `HandleDeclarator`). The
    /// declaration is then invalid, and the flag
    /// [`Sema::take_declarator_type_invalid`] reads is set. True after
    /// the diagnostic.
    pub(crate) fn diagnose_unexpanded_declarator(
        &mut self,
        ty: QualType,
        what: &'static str,
        at: Loc,
    ) -> bool {
        // In a lambda body the lambda-expression carries an enclosing
        // scope's pack (a lambda in a pack expansion, `[] { Ts x{}; }()...`);
        // the lambda's own packs are diagnosed here.
        if !self.lambdas.is_empty() {
            if !self.inst.stack.is_empty() {
                return false;
            }
            let local = self.lambda_unexpanded(PackOperand::Type(ty));
            if local.is_empty() {
                return false;
            }
            self.report_unexpanded_packs(&local, what, at);
        } else if !self.diagnose_unexpanded_pack(PackOperand::Type(ty), what, at) {
            return false;
        }
        self.declarator_type_invalid = true;
        true
    }

    /// True (once) if the last declarator's type was diagnosed as naming
    /// an unexpanded pack: the parser then parses, but does not declare,
    /// the rest of a simple-declaration's group (clang's sticky
    /// `Declarator::InvalidType` in `ParseDeclGroup`).
    pub fn take_declarator_type_invalid(&mut self) -> bool {
        std::mem::take(&mut self.declarator_type_invalid)
    }

    /// [temp.variadic]/12 for an expression written in a template (an
    /// expression statement, `return` operand or condition, `what` =
    /// "expression"; a variable's initializer, "initializer"): naming a
    /// pack outside an expansion is diagnosed when the template is
    /// parsed (clang's `DiagnoseUnexpandedParameterPack` in
    /// `ActOnFinishFullExpr` / `AddInitializerToDecl`). Inside a lambda
    /// body the lambda-expression carries the pack instead
    /// ([`Sema::note_unexpanded_in_lambda`]); a substituted expression is
    /// not checked (a pack it keeps belongs to an enclosing expansion
    /// being rebuilt, §7.2). True after the diagnostic.
    pub(crate) fn diagnose_unexpanded_written(
        &mut self,
        e: ExprId,
        what: &'static str,
        at: Option<Loc>,
    ) -> bool {
        if !self.inst.stack.is_empty() {
            return false;
        }
        if !self.lambdas.is_empty() {
            // Only the innermost lambda's own packs are diagnosed in its
            // body; an enclosing scope's make the lambda unexpanded.
            let local = self.lambda_unexpanded(PackOperand::Expr(e));
            if local.is_empty() {
                return false;
            }
            let at = at.unwrap_or_else(|| self.begin_loc(e));
            self.report_unexpanded_packs(&local, what, at);
            return true;
        }
        if !self.in_template() {
            return false;
        }
        let at = at.unwrap_or_else(|| self.begin_loc(e));
        self.diagnose_unexpanded_pack(PackOperand::Expr(e), what, at)
    }

    /// [temp.variadic]/12 for a default argument written in a function
    /// declarator (clang's `ActOnParamDefaultArgument`, at the
    /// argument's begin). In a lambda (its body, or its own declarator)
    /// an enclosing scope's packs make the lambda-expression contain an
    /// unexpanded pack instead: `[](auto... xs) { [](int = sizeof(xs))
    /// {}; }` is then diagnosed at the inner lambda's full-expression.
    /// True after the diagnostic.
    pub(crate) fn diagnose_unexpanded_default_arg(&mut self, e: ExprId, at: Loc) -> bool {
        if !self.inst.stack.is_empty() {
            return false;
        }
        if self.lambdas.is_empty() {
            return self.diagnose_unexpanded_pack(PackOperand::Expr(e), "default argument", at);
        }
        self.diagnose_lambda_local_packs(PackOperand::Expr(e), "default argument", at)
    }

    /// [temp.variadic]/5, /12 for expression `e` written in the innermost
    /// lambda's declarator before its body opens (a requires-clause of
    /// its template-head or its trailing one, `what` = "requires
    /// clause"), at `e`'s begin: the packs the lambda declares (its
    /// template parameter packs, its function parameter packs) are
    /// diagnosed; an enclosing scope's make the lambda-expression contain
    /// an unexpanded pack ([`Sema::split_lambda_packs`]), so
    /// `[]<class U>() requires C<U, Ts> {}...` expands `Ts`. A
    /// substituted expression is not checked (§7.2). True after the
    /// diagnostic.
    pub(crate) fn diagnose_unexpanded_lambda_declarator(
        &mut self,
        e: ExprId,
        what: &'static str,
    ) -> bool {
        if !self.inst.stack.is_empty() {
            return false;
        }
        let at = self.begin_loc(e);
        self.diagnose_lambda_local_packs(PackOperand::Expr(e), what, at)
    }

    /// The lambda's own packs `op` names diagnosed at `at`, the enclosing
    /// scope's recorded on it. True after the diagnostic.
    pub(crate) fn diagnose_lambda_local_packs(
        &mut self,
        op: PackOperand,
        what: &'static str,
        at: Loc,
    ) -> bool {
        if !self.operand_has_packs(op) {
            return false;
        }
        let local = self.split_lambda_packs(op);
        if local.is_empty() {
            return false;
        }
        self.report_unexpanded_packs(&local, what, at);
        true
    }

    /// [temp.variadic]/12 for a type written in a template: an
    /// exception-declaration's (`what` = "exception type", clang's
    /// `ActOnExceptionDeclarator` at the declarator-id's location). In a
    /// lambda body only the lambda's own packs are diagnosed
    /// ([`Sema::lambda_unexpanded`]). True after the diagnostic.
    pub(crate) fn diagnose_unexpanded_type_written(
        &mut self,
        ty: QualType,
        what: &'static str,
        at: Loc,
    ) -> bool {
        self.diagnose_unexpanded_operand_written(PackOperand::Type(ty), what, at)
    }

    /// [temp.variadic]/12 for an operand written in a template: a type,
    /// expression or nested-name-specifier of a declaration (a
    /// base-specifier's "base type", an "enumerator value", a
    /// using-declarator's "using declaration"; clang's
    /// `DiagnoseUnexpandedParameterPack` with the matching
    /// `UnexpandedParameterPackContext`). In a lambda body only the
    /// lambda's own packs are diagnosed; an enclosing scope's make the
    /// lambda-expression contain an unexpanded pack
    /// ([`Sema::lambda_unexpanded`]), diagnosed at its full-expression
    /// unless an expansion encloses it. A substituted operand is not
    /// checked (§7.2). True after the diagnostic.
    pub(crate) fn diagnose_unexpanded_operand_written(
        &mut self,
        op: PackOperand,
        what: &'static str,
        at: Loc,
    ) -> bool {
        if !self.inst.stack.is_empty() || !self.operand_has_packs(op) {
            return false;
        }
        if !self.lambdas.is_empty() {
            let local = self.lambda_unexpanded(op);
            if local.is_empty() {
                return false;
            }
            self.report_unexpanded_packs(&local, what, at);
            return true;
        }
        self.diagnose_unexpanded_pack(op, what, at)
    }

    /// An expression argument of an attribute, at its parse
    /// ([temp.variadic]/12): no attribute but `annotate` takes a pack
    /// expansion, so a pack it names is reported ("expression contains
    /// unexpanded parameter pack 'T'"); returns false then, and the
    /// attribute is dropped.
    pub fn check_attribute_arg_packs(&mut self, e: ExprId) -> bool {
        let at = self.begin_loc(e);
        !self.diagnose_unexpanded_operand_written(PackOperand::Expr(e), "expression", at)
    }

    /// The packs expression `e` names outside expansions, in order.
    #[allow(clippy::too_many_lines)]
    fn expr_packs(&self, e: ExprId, out: &mut Vec<DeclId>, depth: u32) {
        use kiln_ast::expr::ExprDeps;
        let n = self.ast.exprs.get(e);
        if depth > 256 || !n.deps().contains(ExprDeps::UNEXPANDED_PACK) {
            return;
        }
        match n.kind() {
            // A substituted function parameter pack names its elements,
            // not a pack of a level still to substitute (as a
            // `SubstNonTypeTemplateParmPackExpr` names none); its type is
            // the pattern parameter's, so it must not fall through to the
            // type walk below (WP-980: an empty one in a generic lambda's
            // fold kept the instantiated expansion unexpanded).
            ExprKind::PackExpansionExpr
            | ExprKind::CXXFoldExpr
            | ExprKind::SizeOfPackExpr
            | ExprKind::PackIndexingExpr
            | ExprKind::FunctionParmPackExpr => return,
            ExprKind::DeclRefExpr => {
                let d = self.ast.exprs.decl(e);
                if self.is_pack_decl(d) && !out.contains(&d) {
                    out.push(d);
                }
                return;
            }
            // `Cs<T>` / `Vs<T>` naming a concept or variable template
            // template parameter pack (P2841R7, §2.1.1): the pack, then
            // the arguments' packs below.
            ExprKind::ConceptSpecializationExpr | ExprKind::UnresolvedLookupExpr => {
                let d = if n.kind() == ExprKind::ConceptSpecializationExpr {
                    DeclId::from_raw(n.a()).filter(|&d| {
                        self.kind_of(d) == kiln_ast::decl::DeclKind::TemplateTemplateParm
                    })
                } else {
                    self.var_ttp_ref(e)
                };
                if let Some(d) = d
                    && self.is_pack_decl(d)
                    && !out.contains(&d)
                {
                    out.push(d);
                }
                let args = if n.kind() == ExprKind::ConceptSpecializationExpr {
                    kiln_ast::ids::ArgListId::from_raw(n.b())
                } else {
                    self.ast.templates.expr_targs.get(e).copied()
                };
                for &a in args.map_or(&[][..], |l| self.ast.types.args(l)) {
                    match a.view() {
                        ArgView::Type(x) => self.type_packs(x, out, depth + 1),
                        ArgView::Expr { expr, .. } => self.expr_packs(expr, out, depth + 1),
                        _ => {}
                    }
                }
            }
            ExprKind::LambdaExpr => {
                // What the body names (recorded when it was analysed), and
                // the captures that do not expand their own packs (as
                // `AstBuilder::lambda_expr`'s dependence).
                let id = self.ast.exprs.lambda_id(e);
                for &d in self.lambda_packs.get(&id).map_or(&[][..], Vec::as_slice) {
                    if !out.contains(&d) {
                        out.push(d);
                    }
                }
                let l = self.ast.exprs.lambdas.get(id);
                // The call operator's parameter and return types (clang's
                // `TraverseLambdaExpr` visits the lambda's type-loc).
                if let Some(op) = l.call_op
                    && let Some(t) = self.ast.decls.get(op).ty()
                {
                    self.type_packs(t, out, depth + 1);
                }
                if let Some(inits) = l.inits {
                    for (i, k) in self.ast.lists.iter::<ExprId>(inits).enumerate() {
                        if !l.captures.get(i).is_some_and(|c| c.ellipsis.is_valid() || c.implicit) {
                            self.expr_packs(k, out, depth + 1);
                        }
                    }
                }
                return;
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                if let Some(t) = self.ast.exprs.trait_type_arg(e) {
                    self.type_packs(t, out, depth + 1);
                }
            }
            k if k.is_cast() => {
                if let Some(t) = self.ast.exprs.cast_written_type(e) {
                    self.type_packs(t, out, depth + 1);
                }
            }
            // The local parameters' types, then the requirements
            // ([temp.variadic]/5; clang's `TraverseRequiresExpr`).
            ExprKind::RequiresExpr => {
                let Some(rq) = self.ast.requires_of(e) else { return };
                for p in self.ast.lists.iter::<DeclId>(rq.params) {
                    if let Some(t) = self.ast.decls.get(p).ty() {
                        self.type_packs(t, out, depth + 1);
                    }
                }
                for r in self.ast.templates.requirements(rq) {
                    match *r {
                        Requirement::Simple(x) | Requirement::Nested(x) => {
                            self.expr_packs(x, out, depth + 1);
                        }
                        Requirement::Compound { expr, .. } => self.expr_packs(expr, out, depth + 1),
                        Requirement::Type(t, _) => self.type_packs(t, out, depth + 1),
                    }
                }
                return;
            }
            _ => {}
        }
        let before = out.len();
        for k in self.ast.exprs.children(e, &self.ast.lists) {
            self.expr_packs(k, out, depth + 1);
        }
        if out.len() == before {
            // A written type the node keeps as its own (`Ts()`).
            self.type_packs(n.ty(), out, depth + 1);
        }
    }

    /// The packs type `q` names outside expansions, in order.
    fn type_packs(&self, q: QualType, out: &mut Vec<DeclId>, depth: u32) {
        if depth > 256 || !self.has_unexpanded_pack(q) {
            return;
        }
        let t = &self.ast.types;
        match t.view(q.ty()) {
            TypeView::TemplateTypeParm(d) => {
                if !out.contains(&d) {
                    out.push(d);
                }
            }
            TypeView::Pointer(x)
            | TypeView::LValueRef(x)
            | TypeView::RValueRef(x)
            | TypeView::IncompleteArray(x)
            | TypeView::Paren(x)
            | TypeView::Complex(x)
            | TypeView::Atomic(x)
            | TypeView::Elaborated { named: x, .. }
            | TypeView::ExtQual { base: x, .. }
            | TypeView::Attributed { modified: x, .. }
            | TypeView::Vector { elem: x, .. }
            | TypeView::Decayed { original: x, .. } => {
                self.type_packs(x, out, depth + 1);
            }
            // A kept bound (`char[sizeof(sizeof(Ts))]`, WP-2720); a bound
            // kept only as written names no pack (WP-3015).
            TypeView::ConstantArray(a) => {
                self.type_packs(a.elem, out, depth + 1);
                if let Some(size) = self.ast.types.inst_bound(&a) {
                    self.expr_packs(size, out, depth + 1);
                }
            }
            TypeView::VariableArray { elem, size } => {
                self.type_packs(elem, out, depth + 1);
                self.expr_packs(size, out, depth + 1);
            }
            TypeView::MemberPointer { pointee, class } => {
                self.type_packs(QualType::unqual(class), out, depth + 1);
                self.type_packs(pointee, out, depth + 1);
            }
            TypeView::DependentBitInt { width, .. } => {
                if let Some(e) = t.profile_expr(width) {
                    self.expr_packs(e, out, depth + 1);
                }
            }
            TypeView::DependentSizedArray { elem, size }
            | TypeView::DependentVector { elem, size, .. }
            | TypeView::DependentAddrSpace { pointee: elem, space: size } => {
                self.type_packs(elem, out, depth + 1);
                let size = match size {
                    kiln_ast::types::ExprOperand::Expr(e) => Some(e),
                    kiln_ast::types::ExprOperand::Profile(p) => t.profile_expr(p),
                };
                if let Some(e) = size {
                    self.expr_packs(e, out, depth + 1);
                }
            }
            TypeView::Function(p) => {
                self.type_packs(p.ret(), out, depth + 1);
                for &x in t.params(&p) {
                    self.type_packs(x, out, depth + 1);
                }
                let noexcept = match t.exc_spec(&p) {
                    kiln_ast::types::ExcSpec::NoexceptExpr { expr, .. } => Some(expr),
                    kiln_ast::types::ExcSpec::DependentNoexcept(pr) => t.profile_expr(pr),
                    _ => None,
                };
                if let Some(e) = noexcept {
                    self.expr_packs(e, out, depth + 1);
                }
            }
            // `C<Ts> auto`: the type-constraint's arguments.
            TypeView::Auto { args: Some(args), .. } => {
                for &a in t.args(args) {
                    match a.view() {
                        ArgView::Type(x) => self.type_packs(x, out, depth + 1),
                        ArgView::Expr { expr, .. } => self.expr_packs(expr, out, depth + 1),
                        _ => {}
                    }
                }
            }
            TypeView::DependentName { nns, .. } => self.nns_packs(nns, out, depth + 1),
            TypeView::TemplateSpecialization { args, .. }
            | TypeView::DependentTemplateSpecialization { args, .. } => {
                if let TypeView::DependentTemplateSpecialization { nns, .. } = t.view(q.ty()) {
                    self.nns_packs(nns, out, depth + 1);
                }
                for &a in t.args(args) {
                    match a.view() {
                        ArgView::Type(x) => self.type_packs(x, out, depth + 1),
                        ArgView::Expr { expr, .. } => self.expr_packs(expr, out, depth + 1),
                        _ => {}
                    }
                }
            }
            TypeView::Decltype { expr, .. } => self.expr_packs(expr, out, depth + 1),
            _ => {}
        }
    }

    /// The packs a nested-name-specifier names (`Ts::` in
    /// `typename Ts::type`), outermost prefix first.
    fn nns_packs(&self, n: kiln_ast::NnsId, out: &mut Vec<DeclId>, depth: u32) {
        use kiln_ast::types::{Nns, TypeFlags};
        let t = &self.ast.types;
        if depth > 256 || !t.nns_flags(n).contains(TypeFlags::UNEXPANDED_PACK) {
            return;
        }
        let (prefix, ty) = match t.nns_view(n) {
            Nns::Type { prefix, ty, .. } => (prefix, Some(ty)),
            Nns::Identifier { prefix, .. } => (prefix, None),
            _ => (None, None),
        };
        if let Some(p) = prefix {
            self.nns_packs(p, out, depth + 1);
        }
        if let Some(ty) = ty {
            self.type_packs(ty, out, depth + 1);
        }
    }
}
