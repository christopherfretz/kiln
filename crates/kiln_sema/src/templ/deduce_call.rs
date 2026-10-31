//! Template argument deduction from a function call
//! ([temp.deduct.call]; spec/sema-templates.md §6.1): explicit
//! arguments first, then each `(P, A)` pair with the call adjustments,
//! then defaults, then the substitution of the deduced arguments into
//! the signature in a SFINAE context. A failure is a
//! [`DeductionFailure`] the overload candidate keeps for its note
//! (§10.4).

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::ids::ArgListId;
use kiln_ast::types::{ArgView, TemplateArg, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;
use crate::overload::ArgInfo;
use crate::templ::args::ConvertedArgs;
use crate::templ::deduce::{DeducedArgs, TdfFlags, Tdk};
use crate::templ::stack::{FrameKind, SfinaeState};
use crate::templ::subst::Substituter;

/// A failed deduction (clang's `DeductionFailureInfo`).
#[derive(Clone, Debug)]
pub struct DeductionFailure {
    /// Why.
    pub tdk: Tdk,
    /// The arguments deduced so far and the failure detail.
    pub deduced: DeducedArgs,
    /// `SubstitutionFailure`: the arguments substituted.
    pub args: Option<ArgListId>,
    /// `SubstitutionFailure`: the first error captured.
    pub diag: Option<Diag>,
}

impl DeductionFailure {
    fn new(tdk: Tdk, deduced: DeducedArgs) -> Box<DeductionFailure> {
        Box::new(DeductionFailure { tdk, deduced, args: None, diag: None })
    }
}

impl Sema<'_, '_> {
    /// Runs `f` in a SFINAE frame of `kind` for template `ft` showing
    /// `args`: its result, and the first error captured (`Some` when
    /// substitution failed). `None` when the instantiation depth limit
    /// stops it.
    fn in_sfinae<R>(
        &mut self,
        kind: FrameKind,
        ft: DeclId,
        args: Option<ArgListId>,
        loc: Loc,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<(R, Result<(), Option<Diag>>)> {
        if !self.push_inst_frame(kind, ft, ft, loc) {
            return None;
        }
        if let Some(fr) = self.inst.stack.frames.last_mut() {
            fr.sfinae = Some(true);
        }
        if let Some(a) = args {
            self.set_frame_args(a);
        }
        self.inst.sfinae.borrow_mut().push(SfinaeState::default());
        let r = f(self);
        let st = self.inst.sfinae.borrow_mut().pop().unwrap_or_default();
        self.pop_inst_frame();
        let failed = if st.errors > 0 { Err(st.first) } else { Ok(()) };
        Some((r, failed))
    }

    /// The function declaration a function template declares (its
    /// pattern, instantiated for a member template of a class template
    /// specialization).
    pub(crate) fn templated_function(&self, ft: DeclId) -> Option<DeclId> {
        self.pattern_of(ft).filter(|&f| self.ast.decls.get(f).kind().is_function())
    }

    /// Deduces the template arguments of function template `ft` for a
    /// call with arguments `args` (`explicit` the explicitly-specified
    /// template arguments, as written) and forms the specialization
    /// (§6.1). `object`: `args` has no slot for an object argument the
    /// template's implicit object parameter takes.
    #[allow(clippy::too_many_lines, reason = "the steps of §6.1 in order")]
    pub(crate) fn deduce_call(
        &mut self,
        ft: DeclId,
        explicit: Option<ArgListId>,
        args: &[ArgInfo],
        loc: Loc,
    ) -> Result<DeclId, Box<DeductionFailure>> {
        let ft = self.ast.first(ft);
        let params = self.template_params(ft);
        let depth = self.template_param_depth(ft);
        let n = params.len();
        let mut d = DeducedArgs::new(depth, n);
        let Some(templated) = self.templated_function(ft) else {
            return Err(DeductionFailure::new(Tdk::MiscellaneousDeductionFailure, d));
        };
        if params.iter().any(|&p| self.ast.template_parm(p).is_some_and(|x| x.pack)) {
            self.templ_unimplemented("deduction of template parameter packs", loc);
            return Err(DeductionFailure::new(Tdk::MiscellaneousDeductionFailure, d));
        }
        // §6.1 step 1: the explicitly-specified arguments.
        let (mut ptys, variadic) = self.proto_param_types(templated);
        if let Some(list) = explicit {
            let written: Vec<TemplateArg> = self.ast.types.args(list).to_vec();
            if written.len() > n {
                d.info.param = u16::try_from(n).ok();
                return Err(DeductionFailure::new(Tdk::InvalidExplicitArguments, d));
            }
            let mut bad = None;
            let r = self.in_sfinae(FrameKind::ExplicitArgSubstitution, ft, None, loc, |s| {
                for (i, &w) in written.iter().enumerate() {
                    let prior: Vec<TemplateArg> = d.args[..i].to_vec();
                    let Ok(a) = s.check_template_arg(ft, depth, params[i], w, loc, &prior) else {
                        bad = Some(i);
                        return;
                    };
                    d.args[i] = a;
                }
            });
            let Some(((), failed)) = r else {
                return Err(DeductionFailure::new(Tdk::InstantiationDepth, d));
            };
            if let Some(i) = bad.or_else(|| failed.is_err().then_some(written.len() - 1)) {
                d.info.param = u16::try_from(i).ok();
                return Err(DeductionFailure::new(Tdk::InvalidExplicitArguments, d));
            }
            // P with the explicit arguments substituted ([temp.deduct]/2).
            let levels = self.prior_levels(ft, depth, &d.args);
            let shown = self.ast.types.arg_list(&d.args[..written.len()]);
            let tys = ptys.clone();
            let r = self.in_sfinae(FrameKind::ExplicitArgSubstitution, ft, Some(shown), loc, |s| {
                let mut sub = Substituter::new(s, &levels);
                tys.iter().map(|&t| sub.ty(t, loc)).collect::<Result<Vec<_>, _>>()
            });
            match r {
                None => return Err(DeductionFailure::new(Tdk::InstantiationDepth, d)),
                Some((Ok(t), Ok(()))) => ptys = t,
                Some((_, failed)) => {
                    let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                    f.args = Some(shown);
                    f.diag = failed.err().flatten();
                    return Err(f);
                }
            }
        }
        // Arity ([temp.deduct.call]/1: a parameter without an argument
        // is not deduced from).
        let nargs = args.len();
        if nargs > ptys.len() && !variadic {
            return Err(DeductionFailure::new(Tdk::TooManyArguments, d));
        }
        if nargs < self.fn_min_args(templated) {
            return Err(DeductionFailure::new(Tdk::TooFewArguments, d));
        }
        // §6.1 step 2: each (P, A).
        for (i, a) in args.iter().enumerate() {
            let Some(&p) = ptys.get(i) else { break };
            if !self.is_dependent_type(p) {
                continue;
            }
            let r = self.deduce_call_arg(p, a, &mut d);
            if r != Tdk::Success {
                return Err(DeductionFailure::new(r, d));
            }
        }
        // §6.1 step 3: defaults, then completeness.
        for (j, &p) in params.iter().enumerate() {
            if !matches!(d.args[j].view(), ArgView::Null) {
                continue;
            }
            let Some(def) = self.template_default(p) else {
                d.info.param = u16::try_from(j).ok();
                return Err(DeductionFailure::new(Tdk::Incomplete, d));
            };
            let prior: Vec<TemplateArg> = d.args[..j].to_vec();
            let shown = self.ast.types.arg_list(&prior);
            let r = self.in_sfinae(FrameKind::DeducedArgSubstitution, ft, Some(shown), loc, |s| {
                s.default_template_arg(ft, depth, p, def, loc, &prior)
            });
            match r {
                None => return Err(DeductionFailure::new(Tdk::InstantiationDepth, d)),
                Some((Ok(a), Ok(()))) => d.args[j] = a,
                Some((_, failed)) => {
                    let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                    f.args = Some(shown);
                    f.diag = failed.err().flatten();
                    return Err(f);
                }
            }
        }
        // §6.1 step 4: the specialization, its signature substituted.
        let sugared = self.ast.types.arg_list(&d.args);
        let canonical = self.ast.types.canonical_args(sugared);
        let conv = ConvertedArgs { sugared, canonical };
        let r = self.in_sfinae(FrameKind::DeducedArgSubstitution, ft, Some(sugared), loc, |s| {
            s.get_function_spec(ft, &conv, loc)
        });
        match r {
            None => Err(DeductionFailure::new(Tdk::InstantiationDepth, d)),
            Some((Ok(spec), Ok(()))) => Ok(spec),
            Some((_, failed)) => {
                let mut diag = failed.err().flatten();
                if let Some(spec) = self.ast.templates.find_spec(ft, canonical) {
                    match &diag {
                        Some(x) => {
                            self.templ.fn_spec_failures.entry(spec).or_insert_with(|| x.clone());
                        }
                        None => diag = self.templ.fn_spec_failures.get(&spec).cloned(),
                    }
                }
                let mut f = DeductionFailure::new(Tdk::SubstitutionFailure, d);
                f.args = Some(sugared);
                f.diag = diag;
                Err(f)
            }
        }
    }

    /// Deduces from one call argument `a` for parameter type `p`
    /// ([temp.deduct.call]/2–5).
    fn deduce_call_arg(&mut self, p: QualType, a: &ArgInfo, d: &mut DeducedArgs) -> Tdk {
        let t = &self.ast.types;
        let mut p = t.canonical(p);
        let mut flags = TdfFlags::NONE;
        // [temp.deduct.call]/1: a braced-init-list.
        if let Some(li) = a.list {
            let inner = match t.canonical_view(p) {
                TypeView::LValueRef(x) | TypeView::RValueRef(x) => t.canonical(x),
                _ => p,
            };
            let Some(elem) = self.initializer_list_param(inner) else {
                // A non-deduced context.
                return Tdk::Success;
            };
            let items: Vec<crate::init::InitValue> = self
                .ovl
                .lists
                .get(li as usize)
                .map(|l| l.items.iter().map(|x| x.value.clone()).collect())
                .unwrap_or_default();
            for v in items {
                let crate::init::InitValue::Expr(e) = v else { continue };
                let info = self.arg_info(e);
                let r = self.deduce_call_arg(elem, &info, d);
                if r != Tdk::Success {
                    return r;
                }
            }
            return Tdk::Success;
        }
        let mut at = a.ty;
        // An overload set ([temp.deduct.call]/6): one non-template
        // function deduces from its type; otherwise non-deduced.
        if let Some(e) = a.expr {
            match self.overload_set_arg(e) {
                OverloadArg::Single(f) => {
                    let ft = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                    let addr =
                        self.ast.exprs.get(self.ignore_parens(e)).kind() == ExprKind::UnaryOperator;
                    at = if addr { self.ast.types.pointer_to(ft) } else { ft };
                }
                OverloadArg::NonDeduced => return Tdk::Success,
                OverloadArg::NotASet => {}
            }
        }
        let t = &mut self.ast.types;
        let mut at = t.canonical(at);
        match t.canonical_view(p) {
            TypeView::RValueRef(inner)
                if a.vk == ValueCategory::LValue
                    && inner.quals().is_empty()
                    && matches!(t.canonical_view(inner),
                        TypeView::CanonTemplateTypeParm { depth, .. } if depth as u16 == d.depth) =>
            {
                // A forwarding reference with an lvalue: `A&`.
                p = t.canonical(inner);
                at = t.lvalue_ref_to(at);
            }
            TypeView::LValueRef(inner) | TypeView::RValueRef(inner) => {
                p = t.canonical(inner);
                flags = flags.union(TdfFlags::PARAM_WITH_REFERENCE_TYPE);
            }
            _ => {
                if t.is_array(at) {
                    let e = t.element(at).unwrap_or(QualType::ERROR);
                    at = t.pointer_to(e);
                } else if t.is_function(at) {
                    at = t.pointer_to(at);
                } else {
                    at = t.unqualified(at);
                }
                at = t.canonical(at);
            }
        }
        if flags.contains(TdfFlags::PARAM_WITH_REFERENCE_TYPE) {
            // clang: P's top-level cv is intersected with A's.
            p = QualType::new(p.ty(), p.quals_bits() & at.quals_bits());
        }
        if matches!(t.canonical_view(at), TypeView::Pointer(_) | TypeView::MemberPointer { .. }) {
            flags = flags.union(TdfFlags::IGNORE_QUALIFIERS);
        }
        flags = flags.union(TdfFlags::DERIVED_CLASS);
        self.deduce(p, at, flags, d)
    }

    /// `E` when `p` is `std::initializer_list<E>` (dependent or not).
    fn initializer_list_param(&mut self, p: QualType) -> Option<QualType> {
        if let TypeView::TemplateSpecialization { name, args, .. } =
            self.ast.types.canonical_view(p)
        {
            let tmpl = self.ast.types.template_decl(name)?;
            if Some(self.ast.first(tmpl)) != self.std_initializer_list().map(|x| self.ast.first(x))
            {
                return None;
            }
            return match self.ast.types.args(args).first()?.view() {
                ArgView::Type(t) => Some(t),
                _ => None,
            };
        }
        self.initializer_list_element(p)
    }

    /// For an argument naming an overload set (possibly `&set`): the one
    /// non-template function it names, or `Some(None)` when it names
    /// several or a template; `None` when it is not an overload set.
    fn overload_set_arg(&self, e: ExprId) -> OverloadArg {
        let mut e = self.ignore_parens(e);
        if self.ast.exprs.get(e).kind() == ExprKind::UnaryOperator {
            if self.ast.exprs.unary_op(e) != kiln_ast::expr::UnaryOp::AddrOf {
                return OverloadArg::NotASet;
            }
            e = self.ignore_parens(self.ast.exprs.operand(e));
        }
        if self.ast.exprs.get(e).kind() != ExprKind::UnresolvedLookupExpr {
            return OverloadArg::NotASet;
        }
        let decls = self.ast.unresolved_decls(e);
        let plain: Vec<DeclId> = decls
            .iter()
            .copied()
            .filter(|&d| self.kind_of(d) != DeclKind::FunctionTemplate)
            .collect();
        if plain.len() == 1 && plain.len() == decls.len() {
            return OverloadArg::Single(plain[0]);
        }
        OverloadArg::NonDeduced
    }

    /// The candidate note of a function template whose deduction failed
    /// (spec/sema-overload.md §10.4, clang `DiagnoseBadDeduction`).
    pub(crate) fn deduction_note(&self, ft: DeclId, f: &DeductionFailure, loc: Loc) -> Diag {
        let ft = self.ast.first(ft);
        if let Some(n) = self.deduction_failure_note(ft, f.tdk, &f.deduced, loc) {
            return n;
        }
        let param_name = |i: u16| {
            self.template_params(ft).get(i as usize).map(|&p| self.decl_name(p)).unwrap_or_default()
        };
        match f.tdk {
            Tdk::SubstitutionFailure => {
                let with = f.args.map(|l| format!(" {}", self.template_args_desc(ft, l)));
                let msg = f.diag.as_ref().map(|x| format!(": {}", x.message()));
                self.diag(DiagId::note_ovl_candidate_substitution_failure, loc)
                    .arg(with.unwrap_or_default())
                    .arg(msg.unwrap_or_default())
            }
            Tdk::InvalidExplicitArguments => {
                let i = f.deduced.info.param.unwrap_or(0);
                let name = param_name(i);
                if name.is_empty() {
                    self.diag(DiagId::note_ovl_candidate_explicit_arg_mismatch_unnamed, loc)
                        .arg(ordinal(u32::from(i) + 1))
                } else {
                    self.diag(DiagId::note_ovl_candidate_explicit_arg_mismatch_named, loc).arg(name)
                }
            }
            Tdk::InstantiationDepth => {
                self.diag(DiagId::note_ovl_candidate_instantiation_depth, loc)
            }
            _ => self.diag(DiagId::note_ovl_candidate, loc).arg("function template"),
        }
    }
}

/// What a call argument naming an overload set deduces from
/// ([temp.deduct.call]/6).
enum OverloadArg {
    /// Not an overload set.
    NotASet,
    /// One non-template function: its type.
    Single(DeclId),
    /// Several, or a template: a non-deduced context.
    NonDeduced,
}

/// `1st`, `2nd`, `3rd`, `4th`, …
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, x) if x != 11 => "st",
        (2, x) if x != 12 => "nd",
        (3, x) if x != 13 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}
