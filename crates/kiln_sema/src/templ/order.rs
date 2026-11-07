//! Partial specialization matching and partial ordering
//! ([temp.spec.partial.match], [temp.spec.partial.order];
//! spec/sema-templates.md §7). Partial ordering of function templates
//! ([temp.func.order]) is WP-071; with function parameter packs, WP-131.

use kiln_ast::DeclId;
use kiln_ast::QualType;
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::ArgListId;
use kiln_ast::types::{ArgView, Quals, TemplateArg, TypeView};
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
            // [temp.arg.template]/3: a deduced template template argument
            // must match its parameter.
            let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
            ok = self
                .check_deduced_template_template_args(partial, list.depth, &params, &d.args, poi);
        }
        let r = if ok {
            let deduced = self.ast.types.arg_list(&d.args);
            self.set_frame_args(deduced);
            self.partial_args_match(&p, list.depth, deduced, canonical, poi).then_some(deduced)
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

    /// Whether the arguments of partial specialization `p`, with
    /// `deduced` substituted, are the specialization's arguments
    /// `canonical` ([temp.spec.partial.match]/2: non-deduced contexts
    /// must match too).
    fn partial_args_match(
        &mut self,
        p: &kiln_ast::templates::PartialSpecInfo,
        depth: u16,
        deduced: ArgListId,
        canonical: ArgListId,
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
        if !has_expr {
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
    fn empty_undeduced_packs(&mut self, params: kiln_ast::ids::ListId, d: &mut DeducedArgs) {
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
        let ok = ok && d.first_undeduced().is_none();
        let failed = self.inst.sfinae.borrow_mut().pop().is_some_and(|s| s.errors > 0);
        let r = ok && !failed;
        let _ = self.templ.partial_order.insert((a, b), r);
        r
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
                    (Some(tx), Some(ty)) => self.ast.types.same_type(tx, ty),
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
    /// a non-deduced context included (clang `MarkUsedTemplateParameters`
    /// without `OnlyDeduced`; fmt's `to_pointer(OutputIt, size_t)` against
    /// `to_pointer(buffer_appender<T>, size_t)` with `T` explicit). Probed
    /// by substituting `int` for each undeduced type parameter and every
    /// other parameter for itself: a type that changes or fails names one.
    /// With a parameter pack in `b`, or an undeduced non-type or template
    /// parameter, the parameters count as unused (WP-726).
    fn undeduced_params_unused(&mut self, b: DeclId, d: &DeducedArgs, pb: &[OrderingType]) -> bool {
        let params = self.template_params(b);
        if params.iter().any(|&p| self.ast.template_parm(p).is_some_and(|i| i.pack)) {
            return true;
        }
        let mut args = Vec::with_capacity(params.len());
        for (i, &p) in params.iter().enumerate() {
            let undeduced = d.args.get(i).is_none_or(|a| matches!(a.view(), ArgView::Null));
            if !undeduced {
                args.push(self.template_param_arg(p));
            } else if self.kind_of(p) == DeclKind::TemplateTypeParm {
                args.push(TemplateArg::ty(QualType::INT));
            } else {
                return true;
            }
        }
        let list = self.ast.types.arg_list(&args);
        let mut levels = Self::outer_levels(b, d.depth);
        levels.push(list, b, LevelMode::Substitute);
        let saved = std::mem::take(&mut self.ast.templates.dependent_depth);
        // The probe is a SFINAE context: `int` in place of a parameter a
        // qualifier names (`typename O::pointer`) fails the substitution
        // quietly (LLVM's `NodeAccess::getNodePtr<O>` overloads).
        let r = self.in_sfinae(FrameKind::DeducedArgSubstitution, b, Some(list), Loc::INVALID, |s| {
            pb.iter().all(|x| {
                let r = Substituter::new(s, &levels).ty(x.ty, Loc::INVALID);
                r.is_ok_and(|t| s.ast.types.canonical(t) == s.ast.types.canonical(x.ty))
            })
        });
        self.ast.templates.dependent_depth = saved;
        matches!(r, Some((true, Ok(()))))
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
