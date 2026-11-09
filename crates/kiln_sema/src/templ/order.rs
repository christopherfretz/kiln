//! Partial specialization matching and partial ordering
//! ([temp.spec.partial.match], [temp.spec.partial.order];
//! spec/sema-templates.md §7). Partial ordering of function templates
//! ([temp.func.order]) is WP-071; with function parameter packs, WP-131.

use kiln_ast::DeclId;
use kiln_ast::ExprId;
use kiln_ast::QualType;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprDeps, ExprKind};
use kiln_ast::ids::{ArgListId, ListId};
use kiln_ast::ids::{RawId, TemplateNameId};
use kiln_ast::templates::Requirement;
use kiln_ast::types::{
    ArgView, ExprOperand, Nns, Quals, TemplateArg, TemplateName, TypeFlags, TypeOfOperand, TypeView,
};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::templ::args::LevelMode;
use crate::templ::deduce::{DeducedArgs, TdfFlags, Tdk};
use crate::templ::stack::{FrameKind, SfinaeState};
use crate::templ::subst::Substituter;

/// A partial specialization that matched, with its deduced arguments.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PartialMatch {
    /// The partial specialization (its first declaration).
    pub partial: DeclId,
    /// Its row in the partial table.
    pub index: u32,
    /// The arguments deduced for its parameters.
    pub args: ArgListId,
}

impl Sema<'_, '_> {
    /// The partial specialization of class template `tmpl` that
    /// specialization `spec` is instantiated from
    /// ([temp.spec.partial.match]/1): `Ok(None)` for the primary
    /// template, `Err` after the ambiguity diagnostic
    /// ([temp.spec.partial.match]/1.3). `poi` is the point of
    /// instantiation.
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub(crate) fn match_partial_specs(
        &mut self,
        tmpl: DeclId,
        spec: DeclId,
        poi: Loc,
    ) -> Result<Option<PartialMatch>, ()> {
        let Some((_, info)) = self.ast.spec_info(spec) else { return Ok(None) };
        let args = info.args;
        self.match_partial_args(tmpl, args, spec, poi)
    }

    /// [`Self::match_partial_specs`] for the specialization of `tmpl`
    /// with converted arguments `args`; `who` names it in the ambiguity
    /// diagnostic (the class specialization, else the template).
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub(crate) fn match_partial_args(
        &mut self,
        tmpl: DeclId,
        args: ArgListId,
        who: DeclId,
        poi: Loc,
    ) -> Result<Option<PartialMatch>, ()> {
        let tmpl = self.ast.first(tmpl);
        let candidates: Vec<u32> = self.ast.templates.partials_of(tmpl).to_vec();
        if candidates.is_empty() {
            return Ok(None);
        }
        let canonical = self.ast.types.canonical_args(args);
        let as_: Vec<TemplateArg> = self.ast.types.args(canonical).to_vec();
        let mut matched: Vec<PartialMatch> = Vec::new();
        for i in candidates {
            let Some(p) = self.ast.templates.partial(i).copied() else { continue };
            // The prefilter (§7.1): the argument shapes reject most
            // partial specializations without deduction.
            let shapes = self.ast.templates.partial_shapes(&p);
            if shapes.len() != as_.len()
                || !shapes.iter().zip(&as_).all(|(s, &a)| s.admits(&self.ast.types, a))
            {
                continue;
            }
            if let Some(args) = self.deduce_partial(p.partial, i, &as_, canonical, who, poi) {
                matched.push(PartialMatch { partial: p.partial, index: i, args });
            }
            if self.inst.fatal {
                return Err(());
            }
        }
        match matched.len() {
            0 => Ok(None),
            1 => Ok(Some(matched[0])),
            _ => self.most_specialized_partial(who, args, &matched, poi).map(Some),
        }
    }

    /// Deduces the arguments of partial specialization `partial` (row
    /// `index`) from specialization arguments `as_` (canonical list
    /// `canonical`) in a SFINAE context ([temp.spec.partial.match]/2,
    /// [temp.deduct.type]): `None` when it does not match.
    fn deduce_partial(
        &mut self,
        partial: DeclId,
        index: u32,
        as_: &[TemplateArg],
        canonical: ArgListId,
        who: DeclId,
        poi: Loc,
    ) -> Option<ArgListId> {
        let p = *self.ast.templates.partial(index)?;
        let tmpl = p.template;
        let list = *self.ast.templates.param_list(p.params);
        let n = self.ast.lists.len(list.params);
        let ps: Vec<TemplateArg> = self.ast.types.args(p.canonical).to_vec();
        if !self.push_inst_frame(FrameKind::PartialSpecDeduction, partial, tmpl, poi) {
            return None;
        }
        if let Some(f) = self.inst.stack.frames.last_mut() {
            f.sfinae = Some(true);
        }
        self.inst.sfinae.borrow_mut().push(SfinaeState::default());
        let mut d = DeducedArgs::new(list.depth, n);
        let mut ok = self.deduce_args(&ps, as_, true, &mut d, poi) == Tdk::Success;
        if ok && p.has_pack {
            self.empty_undeduced_packs(list.params, &mut d);
        }
        ok &= d.first_undeduced().is_none();
        if ok {
            let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
            // [temp.deduct.type]/17, [temp.arg.nontype]/1: a deduced
            // integral value is converted to its parameter's type as a
            // converted constant expression, so one that does not fit
            // (narrows) fails the match (clang
            // `ConvertDeducedTemplateArgument`): `256` of an `int`
            // primary parameter does not bind `unsigned char C`.
            ok = params.iter().zip(&d.args).all(|(&pd, &a)| self.deduced_int_fits(pd, a));
            // [temp.arg.template]/3: a deduced template template argument
            // must match its parameter.
            ok = ok
                && self.check_deduced_template_template_args(
                    partial, list.depth, &params, &d.args, poi,
                );
        }
        let r = if ok {
            let deduced = self.ast.types.arg_list(&d.args);
            self.set_frame_args(deduced);
            self.partial_args_match(&p, list.depth, deduced, canonical, false, poi)
                .then_some(deduced)
        } else {
            None
        };
        let failed = self.inst.sfinae.borrow_mut().pop().is_some_and(|s| s.errors > 0);
        self.pop_inst_frame();
        let deduced = r.filter(|_| !failed)?;
        // [temp.spec.partial.match]/2: the partial specialization's
        // associated constraints must be satisfied (§8.3).
        // A partial specialization of a member template redeclared in a
        // class template specialization keeps the original's constraints,
        // checked with that specialization's levels outside its own
        // ([temp.spec.partial.member]/2).
        let member = self.inst.concepts.member_constraints.get(&partial).cloned();
        let head = match member {
            Some((m, _)) => self
                .ast
                .partial_spec_info(m)
                .map_or(list, |(_, i)| *self.ast.templates.param_list(i.params)),
            None => list,
        };
        let mut constraints = Vec::new();
        self.head_constraints(head.params, head.requires, &mut constraints);
        if !constraints.is_empty() {
            let mut levels = match member {
                Some((_, outer)) => outer,
                None => Self::outer_levels(tmpl, list.depth),
            };
            levels.push(deduced, p.partial, LevelMode::Substitute);
            let spec = matches!(
                self.kind_of(who),
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
            )
            .then_some((p.partial, who));
            let saved = std::mem::replace(&mut self.inst.concepts.partial_access, spec);
            let sat = self.check_constraints(partial, &constraints, &levels, deduced, poi);
            self.inst.concepts.partial_access = saved;
            if !self.sat_ok(sat) {
                return None;
            }
        }
        Some(deduced)
    }

    /// Whether deduced argument `a` of template parameter `pd` is not an
    /// integer that the parameter's (non-dependent, integral) type cannot
    /// represent ([temp.arg.nontype]/1: no narrowing).
    fn deduced_int_fits(&self, pd: DeclId, a: TemplateArg) -> bool {
        if self.kind_of(pd) != DeclKind::NonTypeTemplateParm {
            return true;
        }
        let ArgView::Value { ty, value } = a.view() else { return true };
        let Some(pty) = self.ast.decls.get(pd).ty() else { return true };
        let t = &self.ast.types;
        let canon = t.canonical(crate::conv::strip_cv(self, pty));
        if self.is_dependent_type(canon)
            || !t.is_integral(canon)
            || t.as_enum(canon).is_some()
            || t.canonical(crate::conv::strip_cv(self, ty)) == canon
        {
            return true;
        }
        self.const_int(value).is_none_or(|v| self.int_fits(canon, v))
    }

    /// Whether the arguments of partial specialization `p`, with
    /// `deduced` substituted, are the specialization's arguments
    /// `canonical` ([temp.spec.partial.match]/2: non-deduced contexts
    /// must match too). With `ordering`, `deduced` and `canonical` name
    /// another partial specialization's synthesized parameters
    /// ([temp.deduct.partial]/12) and are compared without conversion.
    fn partial_args_match(
        &mut self,
        p: &kiln_ast::templates::PartialSpecInfo,
        depth: u16,
        deduced: ArgListId,
        canonical: ArgListId,
        ordering: bool,
        loc: Loc,
    ) -> bool {
        let mut written: Vec<TemplateArg> = Vec::new();
        let mut packed = false;
        for &a in self.ast.types.args(p.args) {
            match a.view() {
                // A converted pack (`X<T, Ts...>` of a variadic primary):
                // its elements are substituted, then converted again.
                ArgView::Pack(l) => {
                    packed = true;
                    written.extend_from_slice(self.ast.types.args(l));
                }
                _ => written.push(a),
            }
        }
        let deduced_elems: Vec<TemplateArg> = self.ast.types.args(deduced).to_vec();
        let mut args = Self::outer_levels(p.template, depth);
        args.push(deduced, p.partial, LevelMode::Substitute);
        let saved = std::mem::take(&mut self.ast.templates.dependent_depth);
        let mut out = Vec::with_capacity(written.len());
        let mut has_expr = packed;
        let mut expanded = false;
        let mut failed = false;
        {
            let mut s = Substituter::new(self, &args);
            for &a in &written {
                if let Some(i) = s.sema.bare_nttp_index(a, depth) {
                    out.push(deduced_elems.get(usize::from(i)).copied().unwrap_or(a));
                    continue;
                }
                match s.expand_arg(a, loc) {
                    Ok(Some(v)) => {
                        // An expanded pack: compare after conversion.
                        has_expr = true;
                        expanded = true;
                        out.extend(v);
                        continue;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
                let Ok(x) = s.template_arg(a, loc) else {
                    failed = true;
                    break;
                };
                has_expr |= matches!(x.view(), ArgView::Expr { .. } | ArgView::Pack(_));
                out.push(x);
            }
        }
        self.ast.templates.dependent_depth = saved;
        if failed {
            return false;
        }
        let types = &self.ast.types;
        let want = types.args(canonical);
        if out.len() == want.len()
            && out.iter().zip(want).all(|(&x, &y)| types.canonical_arg(x) == y)
        {
            return true;
        }
        // A converted pack of either side compares element-wise.
        let flat = |l: &[TemplateArg]| -> Vec<TemplateArg> {
            let mut v = Vec::with_capacity(l.len());
            for &x in l {
                match x.view() {
                    ArgView::Pack(p) => {
                        v.extend(types.args(p).iter().map(|&e| types.canonical_arg(e)));
                    }
                    _ => v.push(types.canonical_arg(x)),
                }
            }
            v
        };
        if flat(&out) == flat(want) {
            return true;
        }
        // In partial ordering both sides name `a`'s synthesized
        // parameters: nothing to convert, except the elements of an
        // expanded pack, which come back as expressions (`P<0, D...>`
        // deducing `D` = `<1, E...>` from `P<0, 1, E...>`; libstdc++'s
        // `_Parse_int<'0', 'x', D...>` against `<'0', D...>`, WP-811).
        if !has_expr || (ordering && !expanded) {
            return false;
        }
        // A substituted expression argument (`N + 1`) is converted to
        // its value before comparing.
        match self.check_template_args(p.template, &out, &[], loc) {
            Ok(conv) => conv.canonical == canonical,
            Err(()) => false,
        }
    }

    /// The most specialized of the matching partial specializations
    /// `matched` ([temp.spec.partial.match]/1.3,
    /// [temp.spec.partial.order]), else the ambiguity diagnostic.
    fn most_specialized_partial(
        &mut self,
        who: DeclId,
        args: ArgListId,
        matched: &[PartialMatch],
        poi: Loc,
    ) -> Result<PartialMatch, ()> {
        let mut best = matched[0];
        for &m in &matched[1..] {
            if self.partial_more_specialized(m.index, best.index, poi) {
                best = m;
            }
        }
        let unique = matched.iter().all(|m| {
            m.index == best.index || self.partial_more_specialized(best.index, m.index, poi)
        });
        if unique {
            return Ok(best);
        }
        let name = if self.kind_of(who) == DeclKind::CXXRecord {
            self.ty_str(self.tag_type(who))
        } else {
            self.spec_name(who, args)
        };
        let mut d = self.diag(DiagId::sema_partial_spec_ambiguous, poi).arg(name);
        for m in matched {
            let note = self
                .diag(DiagId::note_partial_spec_match, self.ast.decls.get(m.partial).loc())
                .arg(self.partial_spec_bindings(m.partial, m.args));
            d = d.with_note(note);
        }
        self.emit(d);
        Err(())
    }

    /// A pack parameter of `params` deduced from no elements is empty
    /// ([temp.deduct.type]/9).
    fn empty_undeduced_packs(&mut self, params: ListId, d: &mut DeducedArgs) {
        for i in 0..self.ast.lists.len(params) {
            let pack = self
                .ast
                .lists
                .at::<DeclId>(params, i)
                .and_then(|x| self.ast.template_parm(x))
                .is_some_and(|x| x.pack);
            if pack && matches!(d.args.get(i).map(TemplateArg::view), Some(ArgView::Null)) {
                let l = self.ast.types.arg_list(&[]);
                d.args[i] = TemplateArg::pack(l);
            }
        }
    }

    /// Whether partial specialization row `a` is more specialized than
    /// row `b` ([temp.spec.partial.order]/1, [temp.func.order]/5).
    fn partial_more_specialized(&mut self, a: u32, b: u32, loc: Loc) -> bool {
        if !self.partial_at_least_as_specialized(a, b, loc) {
            return false;
        }
        if !self.partial_at_least_as_specialized(b, a, loc) {
            return true;
        }
        // [temp.spec.partial.order]/1, [temp.func.order]/6: equally
        // specialized, the more constrained.
        let (Some(pa), Some(pb)) =
            (self.ast.templates.partial(a).copied(), self.ast.templates.partial(b).copied())
        else {
            return false;
        };
        let (la, lb) =
            (*self.ast.templates.param_list(pa.params), *self.ast.templates.param_list(pb.params));
        let (mut ca, mut cb) = (Vec::new(), Vec::new());
        self.head_constraints(la.params, la.requires, &mut ca);
        self.head_constraints(lb.params, lb.requires, &mut cb);
        if ca.is_empty() {
            return false;
        }
        self.more_constrained(pa.partial, &ca, pb.partial, &cb)
    }

    /// Whether partial specialization row `a` is at least as specialized
    /// as row `b`: `b`'s parameters deduce from `a`'s arguments, with
    /// `a`'s parameters as unique synthesized types
    /// ([temp.spec.partial.order]/1.2, [temp.deduct.partial]). Cached.
    fn partial_at_least_as_specialized(&mut self, a: u32, b: u32, loc: Loc) -> bool {
        if let Some(&r) = self.templ.partial_order.get(&(a, b)) {
            return r;
        }
        let (Some(pa), Some(pb)) =
            (self.ast.templates.partial(a).copied(), self.ast.templates.partial(b).copied())
        else {
            return false;
        };
        let lb = *self.ast.templates.param_list(pb.params);
        let ps: Vec<TemplateArg> = self.ast.types.args(pb.canonical).to_vec();
        let as_: Vec<TemplateArg> = self.ast.types.args(pa.canonical).to_vec();
        self.inst.sfinae.borrow_mut().push(SfinaeState::default());
        let mut d = DeducedArgs::new(lb.depth, self.ast.lists.len(lb.params));
        // [temp.deduct.type]/9: a pack expansion of `a`'s (a synthesized
        // pack) is deduced as a whole by a pack expansion of `b`'s, as
        // clang does (`template <class... P> C<0, T<P...>>` against
        // `template <int R, class... P> C<R, T<P...>>`).
        d.expansions = true;
        let ok = self.deduce_args(&ps, &as_, true, &mut d, loc) == Tdk::Success;
        if ok && pb.has_pack {
            // [temp.deduct.partial]: `C<A, B...>` deduces from `C<A'>`
            // with `B` empty, as in matching.
            self.empty_undeduced_packs(lb.params, &mut d);
        }
        let mut ok = ok && d.first_undeduced().is_none();
        if ok {
            // [temp.deduct.partial]/12 via [temp.deduct.type]/1 (clang
            // `FinishTemplateArgumentDeduction` with `IsPartialOrdering`):
            // `b`'s arguments with the deduced values substituted must be
            // `a`'s, its non-deduced contexts included, so
            // `X<T, when<f(T)>>` is not deduced from `X<M, when<c>>`
            // (Boost.Hana's `when<condition>` dispatch; WP-787).
            let deduced = self.ast.types.arg_list(&d.args);
            ok = self.partial_args_match(&pb, lb.depth, deduced, pa.canonical, true, loc);
        }
        let failed = self.inst.sfinae.borrow_mut().pop().is_some_and(|s| s.errors > 0);
        let r = ok && !failed;
        let _ = self.templ.partial_order.insert((a, b), r);
        r
    }

    /// Whether partial specialization `p` is not more specialized than
    /// its primary template ([temp.spec.partial.general]/9.2, DR1495;
    /// clang `isMoreSpecializedThanPrimary`, which asks
    /// `getMoreSpecialized` in both directions):
    /// - the primary is at least as specialized as `p`: `p`'s parameters
    ///   deduce from the primary's own parameters as unique synthesized
    ///   arguments, and substituting them gives those arguments back
    ///   (`A<U, T>`, `B<Ts...>` of `B<class, class...>`); or
    /// - with a pack on either side, `p` is not at least as specialized
    ///   as the primary: the primary's parameters do not deduce from
    ///   `p`'s arguments (`C<0, Ts...>` of `C<int, class, class...>`:
    ///   the expansion `Ts...` cannot match the non-pack `class`).
    ///
    /// Conservative: false whenever either side has constraints (ordered
    /// by constraints, WP-1053), when a primary non-type parameter has a
    /// placeholder or dependent type, and for a member template of a
    /// templated class (checked per instantiation by clang).
    pub(crate) fn partial_not_more_specialized(&mut self, p: DeclId, loc: Loc) -> bool {
        let Some((_, info)) = self.ast.partial_spec_info(p) else { return false };
        let info = *info;
        let lp = *self.ast.templates.param_list(info.params);
        if lp.depth > 0 {
            return false;
        }
        let head = self.head_template(info.template);
        let Some(ti) = self.ast.template_info(head).copied() else { return false };
        let lt = *self.ast.templates.param_list(ti.params);
        if lt.depth != lp.depth {
            return false;
        }
        let (mut cp, mut ct) = (Vec::new(), Vec::new());
        self.head_constraints(lp.params, lp.requires, &mut cp);
        self.head_constraints(lt.params, lt.requires, &mut ct);
        if !cp.is_empty() || !ct.is_empty() {
            return false;
        }
        let primary: Vec<DeclId> = self.ast.lists.iter::<DeclId>(lt.params).collect();
        if primary.iter().any(|&x| self.ast.template_parm(x).is_none()) {
            return false;
        }
        // A primary non-type parameter typed `auto` or by another
        // parameter (`template <auto V>`, `template <class T, T V>`) needs
        // the parameter type itself deduced; not modelled, so no check.
        let typed_by_param = primary.iter().any(|&x| {
            self.kind_of(x) == DeclKind::NonTypeTemplateParm
                && self
                    .ast
                    .decls
                    .get(x)
                    .ty()
                    .is_none_or(|t| self.contains_deduced(t) || self.is_dependent_type(t))
        });
        if typed_by_param {
            return false;
        }
        let packs = info.has_pack
            || primary.iter().any(|&x| self.ast.template_parm(x).is_some_and(|i| i.pack));
        let ps: Vec<TemplateArg> = self.ast.types.args(info.canonical).to_vec();
        if !packs && (ps.len() != primary.len() || ps.iter().any(|&a| self.is_pack_arg(a))) {
            return false;
        }
        let as_: Vec<TemplateArg> = primary
            .iter()
            .map(|&x| {
                let a = self.template_param_arg(x);
                self.ast.types.canonical_arg(a)
            })
            .collect();
        self.inst.sfinae.borrow_mut().push(SfinaeState::default());
        // The primary at least as specialized as `p`.
        let mut d = DeducedArgs::new(lp.depth, self.ast.lists.len(lp.params));
        // [temp.deduct.type]/9: the primary's pack expansion is deduced
        // as a whole by one of `p`'s (as in partial ordering).
        d.expansions = true;
        let mut ok = self.deduce_args(&ps, &as_, true, &mut d, loc) == Tdk::Success;
        if ok && info.has_pack {
            self.empty_undeduced_packs(lp.params, &mut d);
        }
        ok = ok
            && d.first_undeduced().is_none()
            && !self.deduced_nttp_type_mismatch(lp.params, &d.args);
        if ok {
            let deduced = self.ast.types.arg_list(&d.args);
            let injected = self.ast.types.arg_list(&as_);
            ok = self.partial_args_match(&info, lp.depth, deduced, injected, true, loc);
        }
        // `p` at least as specialized as the primary: only a pack can
        // make this fail (a non-pack argument always deduces a primary
        // parameter; clang 22's type check of a deduced non-type
        // argument is not modelled).
        let mut not_reverse = false;
        if !ok && packs {
            let mut r = DeducedArgs::new(lt.depth, primary.len());
            r.expansions = true;
            let fwd = self.deduce_args(&as_, &ps, true, &mut r, loc) == Tdk::Success;
            if fwd {
                self.empty_undeduced_packs(lt.params, &mut r);
            }
            not_reverse = !fwd || r.first_undeduced().is_some();
        }
        let failed = self.inst.sfinae.borrow_mut().pop().is_some_and(|s| s.errors > 0);
        (ok && !failed) || not_reverse
    }

    /// [`Self::partial_not_more_specialized`]: a non-type parameter
    /// deduced from a synthesized argument of another type (`signed char C`
    /// from `long N`) fails deduction ([temp.deduct.type]/20, clang
    /// `err_deduced_non_type_template_arg_type_mismatch`), so the primary
    /// is not at least as specialized.
    fn deduced_nttp_type_mismatch(&self, params: ListId, deduced: &[TemplateArg]) -> bool {
        let t = &self.ast.types;
        self.ast.lists.iter::<DeclId>(params).zip(deduced).any(|(x, a)| {
            let Some(pt) = self.ast.decls.get(x).ty() else { return false };
            if self.kind_of(x) != DeclKind::NonTypeTemplateParm {
                return false;
            }
            let at = match a.view() {
                ArgView::Expr { expr, .. } => self.ast.exprs.get(expr).ty(),
                ArgView::Profile(pr) => match t.profile_expr(pr) {
                    Some(e) => self.ast.exprs.get(e).ty(),
                    None => return false,
                },
                ArgView::Value { ty, .. } => ty,
                _ => return false,
            };
            !t.same_unqualified(t.canonical(pt), t.canonical(at))
        })
    }

    /// Whether function template `a` is more specialized than `b` in the
    /// context of a call with `nargs` arguments ([temp.func.order]/3-5,
    /// [temp.deduct.partial]): `a` is at least as specialized as `b` and
    /// not the reverse. The reference-binding tie-breaker
    /// ([temp.deduct.partial]/9) is not applied.
    pub fn more_specialized(&mut self, a: DeclId, b: DeclId, nargs: usize) -> bool {
        let side = |n| Side::Call { nargs: n, object: false, reverse: false };
        self.more_specialized_in(a, side(nargs), b, side(nargs))
    }

    /// [`Self::more_specialized`] for two conversion function templates
    /// ([temp.deduct.partial]/3.2: their return types are compared).
    pub fn more_specialized_conversion(&mut self, a: DeclId, b: DeclId) -> bool {
        self.more_specialized_in(a, Side::Conversion, b, Side::Conversion)
    }

    /// [`Self::more_specialized`] for two operator candidates, each
    /// with its own count of explicit call arguments (the object operand
    /// of a member is not one): [temp.func.order]/3 inserts an object
    /// parameter for a member compared with a non-member, and reverses
    /// the parameters of `b` when exactly one candidate is a reversed
    /// rewritten candidate (`reversed`, clang `getMoreSpecializedTemplate`).
    pub fn more_specialized_operator(
        &mut self,
        a: DeclId,
        nargs_a: usize,
        b: DeclId,
        nargs_b: usize,
        reversed: bool,
    ) -> bool {
        let member = |s: &Self, t: DeclId| {
            s.templated_function(t)
                .is_some_and(|f| s.kind_of(f) == DeclKind::CXXMethod && !s.has_explicit_object(f))
        };
        let (ma, mb) = (member(self, a), member(self, b));
        let sa = Side::Call { nargs: nargs_a, object: ma && !mb, reverse: false };
        let sb = Side::Call { nargs: nargs_b, object: mb && !ma, reverse: reversed };
        self.more_specialized_in(a, sa, b, sb)
    }

    fn more_specialized_in(&mut self, a: DeclId, sa: Side, b: DeclId, sb: Side) -> bool {
        let (a, b) = (self.ast.first(a), self.ast.first(b));
        if a == b {
            return false;
        }
        let (Some(pa), Some(pb)) = (self.ordering_types(a, sa), self.ordering_types(b, sb)) else {
            return false;
        };
        if !self.fn_at_least_as_specialized(&pa, b, &pb) {
            return false;
        }
        if !self.fn_at_least_as_specialized(&pb, a, &pa) {
            return true;
        }
        // [temp.deduct.partial]/9-10: deduction succeeded both ways; a
        // reference parameter is more specialized when it is an lvalue
        // reference against an rvalue one, or else more cv-qualified.
        let (mut a_wins, mut b_wins) = (false, false);
        for (x, y) in pa.iter().zip(&pb) {
            if let (Some((xl, xq)), Some((yl, yq))) = (x.refq, y.refq) {
                if xl != yl {
                    *(if xl { &mut a_wins } else { &mut b_wins }) = true;
                } else if xq != yq && xq.contains(yq) {
                    a_wins = true;
                } else if xq != yq && yq.contains(xq) {
                    b_wins = true;
                }
            }
        }
        if a_wins != b_wins {
            return a_wins;
        }
        // [temp.deduct.partial]/11 (CWG1395): equally specialized, a
        // template without a trailing function parameter pack beats one
        // whose trailing pack has no corresponding parameter (clang
        // `getMoreSpecializedTemplate` compares the parameter counts).
        if matches!(sa, Side::Call { .. }) && matches!(sb, Side::Call { .. }) {
            let shape = |s: &Self, t: DeclId| {
                let f = s.templated_function(t)?;
                let ps = s.proto_param_types(f).0;
                let variadic =
                    ps.last().is_some_and(|&q| crate::templ::pack::type_pattern(s, q).is_some());
                Some((ps.len(), variadic))
            };
            if let (Some((na, va)), Some((nb, vb))) = (shape(self, a), shape(self, b))
                && va != vb
            {
                if va && na > nb {
                    return false;
                }
                if vb && nb > na {
                    return true;
                }
            }
        }
        // [temp.func.order]/6: equally specialized, the more constrained,
        // provided /6.1-6.2.2 make the constraints comparable.
        self.constraints_comparable(a, sa, b, sb) && self.template_more_constrained(a, b)
    }

    /// [temp.func.order]/6.1-6.2.2 (clang `getMoreSpecializedTemplate`):
    /// constraints decide between equally specialized `a` and `b` only
    /// when their template-parameter-lists and function parameter lists
    /// have the same length, the corresponding template parameters are
    /// equivalent ignoring constraints ([temp.over.link]/6), the
    /// positionally corresponding function parameters have the same type
    /// ignoring top-level cv ([dcl.fct]/5), and, for conversion
    /// functions, the conversion types are the same.
    fn constraints_comparable(&mut self, a: DeclId, sa: Side, b: DeclId, sb: Side) -> bool {
        let heads = |s: &Self, t: DeclId| {
            let i = s.ast.template_info(t)?;
            let l = *s.ast.templates.param_list(i.params);
            Some(s.ast.lists.iter::<DeclId>(l.params).collect::<Vec<DeclId>>())
        };
        let (Some(ha), Some(hb)) = (heads(self, a), heads(self, b)) else { return false };
        if ha.len() != hb.len()
            || !ha.iter().zip(&hb).all(|(&x, &y)| self.template_params_equivalent(x, y))
        {
            return false;
        }
        let (Some(pa), Some(pb)) =
            (self.declared_param_types(a, sa), self.declared_param_types(b, sb))
        else {
            return false;
        };
        pa.len() == pb.len()
            && pa.iter().zip(&pb).all(|(&x, &y)| self.ast.types.same_unqualified(x, y))
    }

    /// The function parameter types of `ft` compared by
    /// [`Self::constraints_comparable`]: all declared parameters, after
    /// an inserted object parameter and reversed as `side` says
    /// ([temp.func.order]/3), or the conversion type.
    fn declared_param_types(&mut self, ft: DeclId, side: Side) -> Option<Vec<QualType>> {
        let f = self.templated_function(ft)?;
        match side {
            Side::Call { object, reverse, .. } => {
                let mut v = self.proto_param_types(f).0;
                if object {
                    let c = self.ast.decls.get(f).ctx()?;
                    v.insert(0, self.class_type(c));
                }
                if reverse {
                    v.reverse();
                }
                Some(v)
            }
            Side::Conversion => Some(vec![self.conversion_result_type(f)]),
        }
    }

    /// [temp.over.link]/6 equivalence of template parameters `x` and `y`
    /// ignoring type-constraints (clang `TPL_TemplateParamsEquivalent`):
    /// same kind and packness, same non-type parameter type, and
    /// equivalent template template parameter heads.
    fn template_params_equivalent(&self, x: DeclId, y: DeclId) -> bool {
        let (kx, ky) = (self.kind_of(x), self.kind_of(y));
        if kx != ky || self.ttp_kind(x) != self.ttp_kind(y) {
            return false;
        }
        let (ix, iy) = (self.ast.template_parm(x).copied(), self.ast.template_parm(y).copied());
        let (Some(ix), Some(iy)) = (ix, iy) else { return ix.is_none() && iy.is_none() };
        if ix.pack != iy.pack {
            return false;
        }
        match kx {
            DeclKind::NonTypeTemplateParm => {
                match (self.ast.decls.get(x).ty(), self.ast.decls.get(y).ty()) {
                    (Some(tx), Some(ty)) => self.same_type_ignoring_constraint(tx, ty),
                    _ => false,
                }
            }
            DeclKind::TemplateTemplateParm => match (ix.params, iy.params) {
                (Some(a), Some(b)) => {
                    let (la, lb) =
                        (*self.ast.templates.param_list(a), *self.ast.templates.param_list(b));
                    let pa: Vec<DeclId> = self.ast.lists.iter::<DeclId>(la.params).collect();
                    let pb: Vec<DeclId> = self.ast.lists.iter::<DeclId>(lb.params).collect();
                    pa.len() == pb.len()
                        && pa.iter().zip(&pb).all(|(&p, &q)| self.template_params_equivalent(p, q))
                }
                (a, b) => a.is_none() && b.is_none(),
            },
            _ => true,
        }
    }

    /// Whether non-type template parameter types `x` and `y` are the same
    /// ignoring the type-constraint of a top-level placeholder
    /// ([temp.over.link]/6.2, clang `getUnconstrainedType`): `Int auto`
    /// and `SmallInt auto` are both `auto`.
    fn same_type_ignoring_constraint(&self, x: QualType, y: QualType) -> bool {
        let t = &self.ast.types;
        let (cx, cy) = (t.canonical(x), t.canonical(y));
        match (t.view(cx.ty()), t.view(cy.ty())) {
            (
                TypeView::Auto { keyword: kx, deduced: None, pack: px, .. },
                TypeView::Auto { keyword: ky, deduced: None, pack: py, .. },
            ) => kx == ky && px == py && cx.quals_bits() == cy.quals_bits(),
            _ => t.same_type(x, y),
        }
    }

    /// The parameter types of function template `ft`'s templated
    /// function that partial ordering compares (`side`), adjusted
    /// ([temp.deduct.partial]/5-7): references and top-level cv dropped.
    fn ordering_types(&mut self, ft: DeclId, side: Side) -> Option<Vec<OrderingType>> {
        let f = self.templated_function(ft)?;
        let mut ptys = match side {
            Side::Call { nargs, object, .. } => {
                let mut v = self.proto_param_types(f).0;
                v.truncate(nargs);
                if object {
                    // The class of the member stands for its object
                    // parameter (cv and reference are dropped below).
                    let c = self.ast.decls.get(f).ctx()?;
                    v.insert(0, self.class_type(c));
                }
                v
            }
            Side::Conversion => vec![self.conversion_result_type(f)],
        };
        if matches!(side, Side::Call { reverse: true, .. }) {
            ptys.reverse();
        }
        let t = &mut self.ast.types;
        Some(
            ptys.into_iter()
                .map(|p| {
                    let p = t.canonical(p);
                    // A function parameter pack is compared through its
                    // pattern ([temp.deduct.partial]/8).
                    let (p, pack) = match t.view(p.ty()) {
                        TypeView::PackExpansion { pattern, .. } => (t.canonical(pattern), true),
                        _ => (p, false),
                    };
                    let (p, refq) = match t.canonical_view(p) {
                        TypeView::LValueRef(x) => {
                            (t.canonical(x), Some((true, t.effective_quals(x))))
                        }
                        TypeView::RValueRef(x) => {
                            (t.canonical(x), Some((false, t.effective_quals(x))))
                        }
                        _ => (p, None),
                    };
                    let u = t.unqualified(p);
                    OrderingType { ty: t.canonical(u), pack, refq }
                })
                .collect(),
        )
    }

    /// Whether the template with ordering types `pa` is at least as
    /// specialized as function template `b` with ordering types `pb`:
    /// `b`'s parameters deduce from `pa`, whose template parameters
    /// stand for unique synthesized types ([temp.func.order]/3-4,
    /// [temp.deduct.partial]/2-10).
    ///
    /// With function parameter packs ([temp.deduct.partial]/8,
    /// [temp.deduct.type]/10, clang's list deduction with
    /// `PartialOrdering`): a trailing pack of `b` deduces from each
    /// remaining type of `pa`; a pack of `pa` matched by a parameter of
    /// `b` that is not a pack fails; a pack of `b` that is not trailing
    /// is a non-deduced context; a trailing pack of `pa` left over
    /// matches nothing and succeeds.
    fn fn_at_least_as_specialized(
        &mut self,
        pa: &[OrderingType],
        b: DeclId,
        pb: &[OrderingType],
    ) -> bool {
        let depth = self.template_param_depth(b);
        let n = self.template_params(b).len();
        self.inst.sfinae.borrow_mut().push(SfinaeState::default());
        let mut d = DeducedArgs::new(depth, n);
        // [temp.deduct.type]/9: a pack expansion in `pa` (`L<I...>`, of
        // the other template's own pack) is deduced as a whole by one in
        // `pb`, as for class template partial specializations; the two
        // packs need not share an index.
        d.expansions = true;
        d.ordering = true;
        let mut ok = true;
        let mut ai = 0;
        for (pi, p) in pb.iter().enumerate() {
            if !p.pack {
                let Some(x) = pa.get(ai) else {
                    ok = false;
                    break;
                };
                if x.pack || self.deduce(p.ty, x.ty, TdfFlags::NONE, &mut d) != Tdk::Success {
                    ok = false;
                    break;
                }
                ai += 1;
                continue;
            }
            if pi + 1 != pb.len() {
                continue;
            }
            let rest: Vec<QualType> = pa[ai.min(pa.len())..].iter().map(|x| x.ty).collect();
            ai = pa.len();
            let r = self.deduce_pack_elements_from(None, rest.len(), &mut d, |s, k, d| {
                s.deduce(p.ty, rest[k], TdfFlags::NONE, d)
            });
            if r != Tdk::Success {
                ok = false;
                break;
            }
        }
        if ok && ai < pa.len() && !(ai + 1 == pa.len() && pa[ai].pack) {
            ok = false;
        }
        // A pack named only by an expansion that matched nothing has the
        // empty deduction ([temp.deduct.type]/9): `J` of
        // `f(L<int, J...>)` from `f(L<int>)` (mp11 `mp_for_each_impl`).
        let params = self.template_params(b);
        for &i in &d.empty_expanded {
            let at = usize::from(i);
            let pack =
                params.get(at).and_then(|&p| self.ast.template_parm(p)).is_some_and(|x| x.pack);
            if pack && matches!(d.args.get(at).map(TemplateArg::view), Some(ArgView::Null)) {
                d.args[at] = TemplateArg::pack(self.ast.types.arg_list(&[]));
            }
        }
        if ok && d.first_undeduced().is_some() && !self.undeduced_params_unused(b, &d, pb) {
            ok = false;
        }
        let failed = self.inst.sfinae.borrow_mut().pop().is_some_and(|s| s.errors > 0);
        ok && !failed
    }
}

impl Sema<'_, '_> {
    /// [temp.deduct.partial]/12: a template parameter of `b` left without
    /// a value fails the deduction when the types `pb` compared name it,
    /// a non-deduced context included, whatever its kind (type, non-type,
    /// template, or a pack): clang `isAtLeastAsSpecializedAs` with
    /// `MarkUsedTemplateParameters(OnlyDeduced = false)` over the compared
    /// parameter types. A parameter named only outside them (the return
    /// type of a call ordering) may stay without a value (WP-726, WP-779).
    fn undeduced_params_unused(&mut self, b: DeclId, d: &DeducedArgs, pb: &[OrderingType]) -> bool {
        let n = self.template_params(b).len();
        let mut used = vec![false; n];
        let depth = u32::from(d.depth);
        for x in pb {
            self.mark_used_type(x.ty, depth, &mut used, 0);
        }
        used.iter()
            .enumerate()
            .all(|(i, &u)| !u || d.args.get(i).is_some_and(|a| !matches!(a.view(), ArgView::Null)))
    }

    /// Marks in `used` the template parameters of depth `depth` canonical
    /// type `q` names anywhere (clang `MarkUsedTemplateParameters` without
    /// `OnlyDeduced`): sugar is looked through, so an alias template that
    /// drops its parameter names nothing, and a non-dependent `decltype`
    /// is its type.
    pub(crate) fn mark_used_type(&self, q: QualType, depth: u32, used: &mut [bool], lvl: u32) {
        let t = &self.ast.types;
        if lvl > 256
            || !t.is_dependent(q) && !t.flags(q.ty()).intersects(TypeFlags::INSTANTIATION_DEPENDENT)
        {
            return;
        }
        let lvl = lvl + 1;
        let expr_op = |o: ExprOperand| match o {
            ExprOperand::Expr(e) => Some(e),
            ExprOperand::Profile(p) => t.profile_expr(p),
        };
        match t.canonical_view(q) {
            TypeView::CanonTemplateTypeParm { depth: dd, index, .. } => {
                Self::mark_at(used, depth, dd, index);
            }
            TypeView::TemplateTypeParm(p) => self.mark_parm(p, depth, used),
            TypeView::Pointer(x)
            | TypeView::LValueRef(x)
            | TypeView::RValueRef(x)
            | TypeView::IncompleteArray(x)
            | TypeView::Paren(x)
            | TypeView::Complex(x)
            | TypeView::Atomic(x)
            | TypeView::ExtQual { base: x, .. }
            | TypeView::Attributed { modified: x, .. }
            | TypeView::Vector { elem: x, .. }
            | TypeView::PackExpansion { pattern: x, .. }
            | TypeView::SubstTemplateTypeParm { replacement: x, .. }
            | TypeView::UnaryTransform { base: x, .. }
            | TypeView::ConstantArray(kiln_ast::types::ConstArray { elem: x, .. }) => {
                self.mark_used_type(x, depth, used, lvl);
            }
            TypeView::QualifiedArray { base, .. } => {
                self.mark_used_type(QualType::unqual(base), depth, used, lvl);
            }
            TypeView::VariableArray { elem, size } => {
                self.mark_used_type(elem, depth, used, lvl);
                self.mark_used_expr(size, depth, used, lvl);
            }
            TypeView::DependentSizedArray { elem, size }
            | TypeView::DependentVector { elem, size, .. }
            | TypeView::PackIndexing { pattern: elem, index: size, .. } => {
                self.mark_used_type(elem, depth, used, lvl);
                if let Some(e) = expr_op(size) {
                    self.mark_used_expr(e, depth, used, lvl);
                }
            }
            TypeView::DependentBitInt { width, .. } | TypeView::DependentDecltype(width) => {
                if let Some(e) = t.profile_expr(width) {
                    self.mark_used_expr(e, depth, used, lvl);
                }
            }
            TypeView::Decltype { expr, .. } => self.mark_used_expr(expr, depth, used, lvl),
            TypeView::TypeOf { operand, .. } => match operand {
                TypeOfOperand::Expr(e) => self.mark_used_expr(e, depth, used, lvl),
                TypeOfOperand::Type(x) => self.mark_used_type(x, depth, used, lvl),
                TypeOfOperand::Profile(p) => {
                    if let Some(e) = t.profile_expr(p) {
                        self.mark_used_expr(e, depth, used, lvl);
                    }
                }
            },
            TypeView::Splice { operand, .. } => {
                if let Some(e) = expr_op(operand) {
                    self.mark_used_expr(e, depth, used, lvl);
                }
            }
            TypeView::MemberPointer { pointee, class } => {
                self.mark_used_type(QualType::unqual(class), depth, used, lvl);
                self.mark_used_type(pointee, depth, used, lvl);
            }
            TypeView::Function(p) => {
                self.mark_used_type(p.ret(), depth, used, lvl);
                for &x in t.params(&p) {
                    self.mark_used_type(x, depth, used, lvl);
                }
            }
            TypeView::InjectedClassName { injected, .. } => {
                self.mark_used_type(injected, depth, used, lvl);
            }
            TypeView::DependentName { nns, .. } => self.mark_used_nns(nns, depth, used, lvl),
            TypeView::DependentTemplateSpecialization { nns, args, .. } => {
                self.mark_used_nns(nns, depth, used, lvl);
                self.mark_used_args(args, depth, used, lvl);
            }
            TypeView::TemplateSpecialization { name, args, .. } => {
                self.mark_used_name(name, depth, used, lvl);
                self.mark_used_args(args, depth, used, lvl);
            }
            _ => {}
        }
    }

    fn mark_at(used: &mut [bool], depth: u32, at: u32, index: u32) {
        if at == depth
            && let Some(u) = used.get_mut(index as usize)
        {
            *u = true;
        }
    }

    fn mark_parm(&self, p: DeclId, depth: u32, used: &mut [bool]) {
        if let Some(i) = self.ast.template_parm(p) {
            Self::mark_at(used, depth, u32::from(i.depth), u32::from(i.index));
        }
    }

    fn mark_used_args(&self, args: ArgListId, depth: u32, used: &mut [bool], lvl: u32) {
        for &a in self.ast.types.args(args) {
            self.mark_used_arg(a, depth, used, lvl);
        }
    }

    pub(crate) fn mark_used_arg(&self, a: TemplateArg, depth: u32, used: &mut [bool], lvl: u32) {
        match a.view() {
            ArgView::Type(x) => self.mark_used_type(x, depth, used, lvl),
            ArgView::Expr { expr, .. } => self.mark_used_expr(expr, depth, used, lvl),
            ArgView::Profile(p) => {
                if let Some(e) = self.ast.types.profile_expr(p) {
                    self.mark_used_expr(e, depth, used, lvl);
                }
            }
            ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. } => {
                self.mark_used_name(n, depth, used, lvl);
            }
            ArgView::Pack(l) => self.mark_used_args(l, depth, used, lvl),
            _ => {}
        }
    }

    fn mark_used_name(&self, n: TemplateNameId, depth: u32, used: &mut [bool], lvl: u32) {
        match self.ast.types.template_name_view(n) {
            TemplateName::TemplateTemplateParm { depth: dd, index, .. }
            | TemplateName::CanonTemplateTemplateParm { depth: dd, index, .. } => {
                Self::mark_at(used, depth, dd, index);
            }
            TemplateName::Qualified { nns, underlying, .. } => {
                self.mark_used_nns(nns, depth, used, lvl + 1);
                self.mark_used_name(underlying, depth, used, lvl + 1);
            }
            TemplateName::DependentIdentifier { nns, .. }
            | TemplateName::DependentOperator { nns, .. } => {
                self.mark_used_nns(nns, depth, used, lvl + 1);
            }
            _ => {}
        }
    }

    fn mark_used_nns(&self, n: kiln_ast::NnsId, depth: u32, used: &mut [bool], lvl: u32) {
        let t = &self.ast.types;
        if lvl > 256 || !t.nns_flags(n).intersects(TypeFlags::DEPENDENT_INSTANTIATION) {
            return;
        }
        let (prefix, ty) = match t.nns_view(n) {
            Nns::Type { prefix, ty, .. } => (prefix, Some(ty)),
            Nns::Identifier { prefix, .. } => (prefix, None),
            _ => (None, None),
        };
        if let Some(p) = prefix {
            self.mark_used_nns(p, depth, used, lvl + 1);
        }
        if let Some(ty) = ty {
            self.mark_used_type(ty, depth, used, lvl + 1);
        }
    }

    /// [`Self::mark_used_type`] for an expression: a non-type or template
    /// template parameter it names, and every type it holds.
    fn mark_used_expr(&self, e: ExprId, depth: u32, used: &mut [bool], lvl: u32) {
        let n = self.ast.exprs.get(e);
        if lvl > 256 || !n.deps().intersects(ExprDeps::TYPE_VALUE_INST) {
            return;
        }
        let lvl = lvl + 1;
        match n.kind() {
            ExprKind::DeclRefExpr => self.mark_parm(self.ast.exprs.decl(e), depth, used),
            ExprKind::SizeOfPackExpr => {
                if let Some(d) = DeclId::from_raw(n.a()) {
                    self.mark_parm(d, depth, used);
                }
            }
            ExprKind::ConceptSpecializationExpr => {
                if let Some(d) = DeclId::from_raw(n.a()) {
                    self.mark_parm(d, depth, used);
                }
                if let Some(l) = ArgListId::from_raw(n.b()) {
                    self.mark_used_args(l, depth, used, lvl);
                }
            }
            ExprKind::UnresolvedLookupExpr | ExprKind::DependentScopeDeclRefExpr => {
                if n.kind() == ExprKind::DependentScopeDeclRefExpr {
                    if let Some(q) = kiln_ast::NnsId::from_raw(n.a()) {
                        self.mark_used_nns(q, depth, used, lvl);
                    }
                } else if let Some(d) = self.var_ttp_ref(e) {
                    self.mark_parm(d, depth, used);
                }
                if let Some(&l) = self.ast.templates.expr_targs.get(e) {
                    self.mark_used_args(l, depth, used, lvl);
                }
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                if let Some(t) = self.ast.exprs.trait_type_arg(e) {
                    self.mark_used_type(t, depth, used, lvl);
                }
            }
            k if k.is_cast() => {
                if let Some(t) = self.ast.exprs.cast_written_type(e) {
                    self.mark_used_type(t, depth, used, lvl);
                }
            }
            ExprKind::RequiresExpr => {
                let Some(rq) = self.ast.requires_of(e) else { return };
                for p in self.ast.lists.iter::<DeclId>(rq.params) {
                    if let Some(t) = self.ast.decls.get(p).ty() {
                        self.mark_used_type(t, depth, used, lvl);
                    }
                }
                for r in self.ast.templates.requirements(rq) {
                    match *r {
                        Requirement::Simple(x)
                        | Requirement::Nested(x)
                        | Requirement::Compound { expr: x, .. } => {
                            self.mark_used_expr(x, depth, used, lvl);
                        }
                        Requirement::Type(t, _) => self.mark_used_type(t, depth, used, lvl),
                    }
                }
                return;
            }
            _ => {}
        }
        for k in self.ast.exprs.children(e, &self.ast.lists) {
            self.mark_used_expr(k, depth, used, lvl);
        }
        // A type the node keeps as its own (`T()`, `T{}`).
        self.mark_used_type(n.ty(), depth, used, lvl);
    }
}

/// One parameter type compared by partial ordering of function
/// templates: adjusted per [temp.deduct.partial]/5-7, and for a function
/// parameter pack, its pattern.
#[derive(Clone, Copy, Debug)]
struct OrderingType {
    ty: QualType,
    pack: bool,
    /// A reference parameter: lvalue reference, and the referred-to
    /// type's cv-qualifiers ([temp.deduct.partial]/9).
    refq: Option<(bool, Quals)>,
}

/// One side of a partial ordering of function templates
/// ([temp.deduct.partial]/3).
#[derive(Clone, Copy, Debug)]
enum Side {
    /// A call: the first `nargs` parameter types, after an inserted
    /// object parameter (`object`), reversed for a reversed rewritten
    /// candidate (`reverse`).
    Call { nargs: usize, object: bool, reverse: bool },
    /// A conversion: the conversion type.
    Conversion,
}
