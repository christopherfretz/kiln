//! Partial specialization matching and partial ordering
//! ([temp.spec.partial.match], [temp.spec.partial.order];
//! spec/sema-templates.md §7). Partial ordering of function templates
//! ([temp.func.order]) without packs is WP-071; with packs, WP-072.

use kiln_ast::DeclId;
use kiln_ast::QualType;
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::ArgListId;
use kiln_ast::types::{ArgView, TemplateArg, TypeView};
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
            if let Some(args) = self.deduce_partial(p.partial, i, &as_, canonical, poi) {
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
            // A pack deduced from no elements is empty ([temp.deduct.type]/9).
            for i in 0..n {
                let pack = self
                    .ast
                    .lists
                    .at::<DeclId>(list.params, i)
                    .and_then(|x| self.ast.template_parm(x))
                    .is_some_and(|x| x.pack);
                if pack && matches!(d.args.get(i).map(TemplateArg::view), Some(ArgView::Null)) {
                    let l = self.ast.types.arg_list(&[]);
                    d.args[i] = TemplateArg::pack(l);
                }
            }
        }
        ok &= d.first_undeduced().is_none();
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
        let mut constraints = Vec::new();
        self.head_constraints(list.params, list.requires, &mut constraints);
        if !constraints.is_empty() {
            let mut levels = Self::outer_levels(tmpl, list.depth);
            levels.push(deduced, p.partial, LevelMode::Substitute);
            let sat = self.check_constraints(partial, &constraints, &levels, deduced, poi);
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
        let ok = self.deduce_args(&ps, &as_, true, &mut d, loc) == Tdk::Success
            && d.first_undeduced().is_none();
        let failed = self.inst.sfinae.borrow_mut().pop().is_some_and(|s| s.errors > 0);
        let r = ok && !failed;
        let _ = self.templ.partial_order.insert((a, b), r);
        r
    }

    /// Whether function template `a` is more specialized than `b` in the
    /// context of a call with `nargs` arguments ([temp.func.order]/3-5,
    /// [temp.deduct.partial]): `a` is at least as specialized as `b` and
    /// not the reverse. Packs are WP-072; the reference-binding
    /// tie-breaker ([temp.deduct.partial]/9) is not applied.
    pub fn more_specialized(&mut self, a: DeclId, b: DeclId, nargs: usize) -> bool {
        let (a, b) = (self.ast.first(a), self.ast.first(b));
        if a == b {
            return false;
        }
        let ab = self.fn_at_least_as_specialized(a, b, nargs);
        if !ab {
            return false;
        }
        if !self.fn_at_least_as_specialized(b, a, nargs) {
            return true;
        }
        // [temp.func.order]/6: equally specialized, the more constrained.
        self.template_more_constrained(a, b)
    }

    /// The first `nargs` parameter types of function template `ft`'s
    /// templated function, adjusted for partial ordering
    /// ([temp.deduct.partial]/5-7): references and top-level cv dropped.
    fn ordering_types(&mut self, ft: DeclId, nargs: usize) -> Option<Vec<QualType>> {
        let f = self.templated_function(ft)?;
        let (ptys, _) = self.proto_param_types(f);
        let t = &mut self.ast.types;
        Some(
            ptys.into_iter()
                .take(nargs)
                .map(|p| {
                    let p = t.canonical(p);
                    let p = match t.canonical_view(p) {
                        TypeView::LValueRef(x) | TypeView::RValueRef(x) => t.canonical(x),
                        _ => p,
                    };
                    let u = t.unqualified(p);
                    t.canonical(u)
                })
                .collect(),
        )
    }

    /// Whether function template `a` is at least as specialized as `b`:
    /// `b`'s parameters deduce from `a`'s parameter types, `a`'s template
    /// parameters standing for unique synthesized types
    /// ([temp.func.order]/3-4, [temp.deduct.partial]/2-10).
    fn fn_at_least_as_specialized(&mut self, a: DeclId, b: DeclId, nargs: usize) -> bool {
        let (Some(pa), Some(pb)) = (self.ordering_types(a, nargs), self.ordering_types(b, nargs))
        else {
            return false;
        };
        if pa.len() != pb.len() {
            return false;
        }
        let depth = self.template_param_depth(b);
        let n = self.template_params(b).len();
        self.inst.sfinae.borrow_mut().push(SfinaeState::default());
        let mut d = DeducedArgs::new(depth, n);
        let mut ok = true;
        for (&p, &x) in pb.iter().zip(&pa) {
            if self.deduce(p, x, TdfFlags::NONE, &mut d) != Tdk::Success {
                ok = false;
                break;
            }
        }
        let failed = self.inst.sfinae.borrow_mut().pop().is_some_and(|s| s.errors > 0);
        ok && !failed
    }
}
