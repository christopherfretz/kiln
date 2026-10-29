//! Constructor selection for a call with arbitrary arguments
//! ([over.match.ctor], [over.match.list]; spec/sema-class.md §5.2, Q7):
//! the unique-candidate approximation of overload resolution until
//! WP-068a. Each candidate's arguments are classified by the implicit
//! conversion sequence they need ([over.best.ics]); a candidate wins
//! only when it is better than every other candidate that is or may be
//! viable ([over.match.best]/2). Anything the approximation cannot
//! decide (function templates, constraints, inherited constructors,
//! user-defined conversions that must be ranked, derived-to-base
//! ranking between different bases) answers [`Selected::Unsupported`].

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits};
use kiln_ast::expr::ValueCategory;
use kiln_ast::names::DeclName;
use kiln_ast::types::{Quals, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};

use super::special::{Selected, SpecialKind};
use crate::Sema;
use crate::conv::{
    self, ConvCtx, FirstStep, FixedEnumPromotion, IcsCompare, SecondStep, StandardConversion,
    ThirdStep,
};

/// The form of an initialization by constructor (clang's
/// `InitializationKind`), which decides the candidates
/// ([over.match.ctor]/1, [over.match.list]/1) and whether an argument
/// may need a user-defined conversion ([over.best.ics]/4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InitStyle {
    /// `T x = e;`, argument passing, `return e;`: converting
    /// constructors only, arguments without user-defined conversions.
    Copy,
    /// `T x(a, ...)`, `T(a, ...)`, `static_cast<T>(e)`.
    Direct,
    /// `T x = {a, ...}`: an `explicit` constructor may be chosen but
    /// makes the initialization ill-formed ([over.match.list]/1).
    CopyList,
    /// `T x{a, ...}`, `T{a, ...}`.
    DirectList,
}

impl InitStyle {
    pub(crate) fn is_list(self) -> bool {
        matches!(self, InitStyle::CopyList | InitStyle::DirectList)
    }
}

/// The implicit conversion sequence of one argument ([over.best.ics]).
#[derive(Clone, Copy, Debug)]
enum Fit {
    /// A standard conversion sequence; for a reference binding, whether
    /// the reference is an rvalue reference and its referent's
    /// qualifiers.
    Std(StandardConversion, Option<(bool, Quals)>),
    /// A class argument for a parameter of the same class (rank 0) or a
    /// base (rank 1, `base`) ([over.best.ics]/6).
    Class { rank: u8, base: Option<DeclId>, binding: Option<(bool, Quals)> },
    /// A user-defined conversion sequence; `known` when it certainly
    /// exists (a unique converting constructor of the parameter's class
    /// from a non-class argument), otherwise the candidate may not be
    /// viable.
    User { known: bool },
    /// A variadic argument ([over.ics.ellipsis]).
    Ellipsis,
}

struct Candidate {
    f: DeclId,
    fits: Vec<Fit>,
    /// Some argument's viability is unknown.
    maybe: bool,
}

impl Sema<'_, '_> {
    /// Selects the constructor of complete class `class` for `args` in an
    /// initialization of form `style` (Q7). `Missing` when no constructor
    /// is viable.
    pub(crate) fn resolve_ctor(
        &mut self,
        class: DeclId,
        args: &[ExprId],
        style: InitStyle,
    ) -> Selected {
        let c = self.class_def(class);
        self.ensure_implicit(c, SpecialKind::DefaultCtor);
        self.ensure_implicit(c, SpecialKind::CopyCtor);
        self.ensure_implicit(c, SpecialKind::MoveCtor);
        if self.defn_bits(c).contains(DefinitionBits::HAS_INHERITED_CTOR) {
            return Selected::Unsupported;
        }
        let mut templates = false;
        let mut cands: Vec<DeclId> = Vec::new();
        for f in self.ast.constructors(c).iter().collect::<Vec<_>>() {
            match self.kind_of(f) {
                DeclKind::FunctionTemplate => {
                    // todo(WP-068a): constructor templates.
                    templates = true;
                    continue;
                }
                DeclKind::CXXConstructor => {}
                _ => continue,
            }
            if self.ast.decls.get(f).has(DeclFlags::INVALID) {
                continue;
            }
            if self.ast.decls.cold.requires_clause.contains(f) {
                return Selected::Unsupported;
            }
            if !cands.iter().any(|&x| self.ast.first(x) == self.ast.first(f)) {
                cands.push(f);
            }
        }
        let mut viable: Vec<Candidate> = Vec::new();
        for f in cands {
            if style == InitStyle::Copy && self.fn_spec(f).is_explicit() {
                continue;
            }
            let params = self.param_decls(f);
            if args.len() > params.len() && !self.fn_variadic(f) {
                continue;
            }
            if params.iter().skip(args.len()).any(|&p| !self.param_has_default(p)) {
                continue;
            }
            // todo(WP-067b follow-up): initializer-list constructors
            // ([over.match.list]/1.1) are found by the callers first.
            let mut fits = Vec::with_capacity(args.len());
            let mut ok = true;
            for (i, &a) in args.iter().enumerate() {
                let fit = match params.get(i) {
                    None => Some(Fit::Ellipsis),
                    Some(&p) => {
                        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                        self.arg_fit(a, pty, style)
                    }
                };
                let Some(x) = fit else {
                    ok = false;
                    break;
                };
                fits.push(x);
            }
            if !ok {
                continue;
            }
            let maybe = fits.iter().any(|x| matches!(x, Fit::User { known: false }));
            viable.push(Candidate { f, fits, maybe });
        }
        if viable.is_empty() {
            return if templates { Selected::Unsupported } else { Selected::Missing };
        }
        let mut best = None;
        'outer: for (i, a) in viable.iter().enumerate() {
            for (j, b) in viable.iter().enumerate() {
                if i == j {
                    continue;
                }
                match self.better_candidate(a, b) {
                    None => return Selected::Unsupported,
                    Some(true) => {}
                    Some(false) => continue 'outer,
                }
            }
            best = Some(i);
            break;
        }
        match best {
            None if templates || viable.iter().any(|c| c.maybe) => Selected::Unsupported,
            None => Selected::Ambiguous,
            Some(i) if templates || viable[i].maybe => Selected::Unsupported,
            Some(i) => {
                let f = viable[i].f;
                if self.fn_spec(f).is_deleted() { Selected::Deleted(f) } else { Selected::Found(f) }
            }
        }
    }

    /// Whether some constructor of `class` is an initializer-list
    /// constructor ([dcl.init.list]/2): its first parameter is
    /// `std::initializer_list<E>` (or a reference to it) and the others
    /// have default arguments.
    pub(crate) fn has_init_list_ctor(&mut self, class: DeclId) -> bool {
        let c = self.class_def(class);
        let name = DeclName::ident(self.intern("initializer_list"));
        for f in self.ast.constructors(c).iter().collect::<Vec<_>>() {
            let f = match self.kind_of(f) {
                DeclKind::CXXConstructor => f,
                DeclKind::FunctionTemplate => {
                    match self.ast.template_info(f).and_then(|t| t.pattern) {
                        Some(x) => x,
                        None => continue,
                    }
                }
                _ => continue,
            };
            let params = self.param_decls(f);
            let Some(&p) = params.first() else { continue };
            if params[1..].iter().any(|&q| !self.param_has_default(q)) {
                continue;
            }
            let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
            let t = self.non_reference(pty);
            let named = match self.ast.types.canonical_view(t) {
                TypeView::Record(d) => self.ast.decls.get(d).name() == name,
                TypeView::TemplateSpecialization { .. } => self
                    .ast
                    .types
                    .as_record(t)
                    .is_some_and(|d| self.ast.decls.get(d).name() == name),
                _ => false,
            };
            if named {
                return true;
            }
        }
        false
    }

    /// The implicit conversion sequence from `a` to a parameter of type
    /// `pty`, or `None` when there is none.
    fn arg_fit(&mut self, a: ExprId, pty: QualType, style: InitStyle) -> Option<Fit> {
        let aty = self.ety(a);
        if self.is_error_type(aty) || self.is_error_type(pty) {
            return Some(Fit::User { known: false });
        }
        let vk = self.ast.exprs.get(a).value_category();
        let (referent, rref) = match self.ast.types.canonical_view(pty) {
            TypeView::LValueRef(t) => (t, Some(false)),
            TypeView::RValueRef(t) => (t, Some(true)),
            _ => (pty, None),
        };
        let rq = self.ast.types.effective_quals(referent);
        let aq = self.ast.types.effective_quals(aty);
        // A non-const (or volatile) lvalue reference binds no temporary.
        let plain_lref = rref == Some(false) && (!rq.is_const() || rq.is_volatile());
        let user_ok = style != InitStyle::Copy;
        let ar = self.ast.types.as_record(aty);
        let pr = self.ast.types.as_record(referent);
        if let Some(pr) = pr {
            let pc = self.class_def(pr);
            if let Some(ar) = ar
                && let Some(ac) = conv::bases::complete_class(self, ar)
            {
                let (rank, base) = if conv::bases::same_class(self, ac, pc) {
                    (0, None)
                } else if conv::is_derived_from(self, ac, pc) {
                    (1, Some(pc))
                } else {
                    return self.user_fit(a, Some(pc), user_ok, plain_lref);
                };
                let Some(r) = rref else {
                    return Some(Fit::Class { rank, base, binding: None });
                };
                let ok = rq.contains(aq)
                    && match vk {
                        ValueCategory::LValue => !r,
                        _ => !plain_lref,
                    };
                return ok.then_some(Fit::Class { rank, base, binding: Some((r, rq)) });
            }
            if plain_lref || !user_ok {
                // todo(WP-068a): a conversion function yielding an lvalue.
                return if ar.is_some() && conv::has_conversion_functions(self, aty) {
                    Some(Fit::User { known: false })
                } else {
                    None
                };
            }
            if ar.is_some() {
                return Some(Fit::User { known: false });
            }
            // A converting constructor of the parameter's class from the
            // non-class argument.
            return self.converting_ctor_fit(a, pc);
        }
        if ar.is_some() {
            return self.user_fit(a, None, user_ok, false);
        }
        let Some(r) = rref else {
            let to = conv::strip_cv(self, pty);
            return conv::standard_conversion(self, a, to, ConvCtx::Initialization)
                .map(|s| Fit::Std(s, None));
        };
        let related = self.ast.types.same_unqualified(aty, referent);
        if related && vk != ValueCategory::PRValue {
            // [dcl.init.ref]/5.1, /5.3.1: a direct binding.
            let ok = rq.contains(aq)
                && match vk {
                    ValueCategory::LValue => !r,
                    _ => !plain_lref,
                };
            if !ok {
                return None;
            }
            let from = aty;
            let scs = StandardConversion {
                first: FirstStep::Identity,
                second: SecondStep::Identity,
                third: ThirdStep::Identity,
                from,
                to_types: [from; 3],
                null_pointer: false,
                fixed_enum: FixedEnumPromotion::None,
            };
            return Some(Fit::Std(scs, Some((r, rq))));
        }
        if plain_lref {
            return None;
        }
        if self.ast.types.is_function(referent) || self.ast.types.is_array(referent) {
            // todo(WP-068a): references to functions and arrays.
            return Some(Fit::User { known: false });
        }
        // [dcl.init.ref]/5.4.2: a temporary copy-initialized from `a`.
        let to = conv::strip_cv(self, referent);
        conv::standard_conversion(self, a, to, ConvCtx::Initialization)
            .map(|s| Fit::Std(s, Some((r, rq))))
    }

    /// A class argument converted by a conversion function, or an
    /// argument for an unrelated class parameter.
    fn user_fit(
        &mut self,
        a: ExprId,
        pc: Option<DeclId>,
        user_ok: bool,
        plain_lref: bool,
    ) -> Option<Fit> {
        let aty = self.ety(a);
        if self.ast.types.as_record(aty).is_some() && conv::has_conversion_functions(self, aty) {
            // todo(WP-068a): conversion functions of the argument's class.
            return Some(Fit::User { known: false });
        }
        if !user_ok || plain_lref {
            return None;
        }
        let Some(pc) = pc else {
            return Some(Fit::User { known: false });
        };
        self.converting_ctor_fit(a, pc)
    }

    /// A user-defined conversion of `a` by a converting constructor of
    /// the class `pc` ([over.ics.user]/1): none means no conversion
    /// sequence; a deleted one still forms one.
    fn converting_ctor_fit(&mut self, a: ExprId, pc: DeclId) -> Option<Fit> {
        let Some(pc) = conv::bases::complete_class(self, pc) else {
            return Some(Fit::User { known: false });
        };
        match self.resolve_ctor(pc, &[a], InitStyle::Copy) {
            Selected::Missing => None,
            Selected::Found(_) | Selected::Deleted(_) => Some(Fit::User { known: true }),
            _ => Some(Fit::User { known: false }),
        }
    }

    /// [over.match.best]/2.1 for two candidates (no templates):
    /// `Some(true)` if `a` is better than `b`, `None` if the
    /// approximation cannot tell.
    fn better_candidate(&self, a: &Candidate, b: &Candidate) -> Option<bool> {
        let (mut better, mut worse) = (false, false);
        for (x, y) in a.fits.iter().zip(&b.fits) {
            match self.compare_fits(*x, *y)? {
                IcsCompare::Better => better = true,
                IcsCompare::Worse => worse = true,
                IcsCompare::Indistinguishable => {}
            }
        }
        Some(better && !worse)
    }

    fn compare_fits(&self, x: Fit, y: Fit) -> Option<IcsCompare> {
        use IcsCompare::{Better, Indistinguishable, Worse};
        Some(match (x, y) {
            (Fit::Ellipsis, Fit::Ellipsis) => Indistinguishable,
            (Fit::Ellipsis, _) => Worse,
            (_, Fit::Ellipsis) => Better,
            // todo(WP-068a): ranking user-defined conversion sequences
            // ([over.ics.rank]/3.3).
            (Fit::User { .. }, Fit::User { .. }) => return None,
            (Fit::User { .. }, _) => Worse,
            (_, Fit::User { .. }) => Better,
            (Fit::Std(a, ab), Fit::Std(b, bb)) => {
                match conv::compare_standard_conversions(self, &a, &b) {
                    Indistinguishable => binding_tie_break(ab, bb),
                    r => r,
                }
            }
            (
                Fit::Class { rank: ar, base: abase, binding: ab },
                Fit::Class { rank: br, base: bbase, binding: bb },
            ) => {
                if ar != br {
                    if ar < br { Better } else { Worse }
                } else if ar == 1 && abase != bbase {
                    // todo(WP-068a): [over.ics.rank]/4.4 between bases.
                    return None;
                } else {
                    binding_tie_break(ab, bb)
                }
            }
            _ => return None,
        })
    }
}

/// [over.ics.rank]/3.2.3 and /3.2.6 for two reference bindings of the
/// same argument: an rvalue reference binding an rvalue beats an lvalue
/// reference (only an rvalue binds both); the less cv-qualified
/// referent is better.
fn binding_tie_break(a: Option<(bool, Quals)>, b: Option<(bool, Quals)>) -> IcsCompare {
    let (Some((ar, aq)), Some((br, bq))) = (a, b) else { return IcsCompare::Indistinguishable };
    if ar != br {
        return if ar { IcsCompare::Better } else { IcsCompare::Worse };
    }
    if aq == bq {
        IcsCompare::Indistinguishable
    } else if bq.contains(aq) {
        IcsCompare::Better
    } else if aq.contains(bq) {
        IcsCompare::Worse
    } else {
        IcsCompare::Indistinguishable
    }
}
