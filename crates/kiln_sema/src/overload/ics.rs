//! Implicit conversion sequences ([over.best.ics], spec/sema-overload.md
//! §3.3) and their comparison ([over.ics.rank], §3.4).
//!
//! `try_implicit_conversion` is a query: it builds no expressions and
//! emits no diagnostics (instantiating a class template to look at its
//! constructors is its only side effect, as in clang).

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::ValueCategory;
use kiln_ast::types::{Quals, RefQual, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use super::{CandidateFailure, CandidateKind, OvlOutcome, SetKind};
use crate::Sema;
use crate::conv::bases::{complete_class, is_derived_from, same_class};
use crate::conv::{
    self, ConvCtx, IcsCompare, ListArray, RefBinding, SecondStep, StandardConversion,
    compare_standard_conversions, function_ref_compatible, standard_conversion_from,
};
use crate::init::{InitItem, InitList, InitValue};

/// One argument of a candidate set: what an implicit conversion sequence
/// is computed from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ArgInfo {
    /// The argument's type.
    pub ty: QualType,
    /// Its value category.
    pub vk: ValueCategory,
    /// It is a null pointer constant.
    pub npc: bool,
    /// The width of the bit-field it designates.
    pub bit_field: Option<u32>,
    /// The expression, if there is one (synthetic arguments of special
    /// member selection have none).
    pub expr: Option<ExprId>,
    /// A braced-init-list argument: its index in `Sema::ovl.lists`.
    pub list: Option<u32>,
}

impl ArgInfo {
    /// A synthetic argument of type `ty` and category `vk`.
    pub(crate) fn of_type(ty: QualType, vk: ValueCategory) -> ArgInfo {
        ArgInfo { ty, vk, npc: false, bit_field: None, expr: None, list: None }
    }
}

/// Restrictions on an implicit conversion sequence ([over.best.ics]/4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct IcsOptions(u8);

impl IcsOptions {
    /// None.
    pub const NONE: IcsOptions = IcsOptions(0);
    /// No user-defined conversions ([over.best.ics]/4).
    pub const SUPPRESS_UDC: IcsOptions = IcsOptions(1);
    /// A reference may not bind a temporary (the implicit object
    /// parameter, [over.match.funcs]/5).
    pub const NO_TEMPORARY: IcsOptions = IcsOptions(2);
    /// Explicit constructors and conversion functions are candidates
    /// (direct-initialization, contextual conversion to `bool`).
    pub const ALLOW_EXPLICIT: IcsOptions = IcsOptions(4);
    /// The sequence is a candidate's argument conversion: a reference
    /// binding whose best conversion function ([over.match.ref]) is
    /// deleted is no conversion (clang's `FindConversionForRefInit`),
    /// where an initialization diagnoses the deleted function.
    pub const CANDIDATE_ARG: IcsOptions = IcsOptions(8);
    /// A direct-initialization of a reference: explicit conversion
    /// functions are candidates of [over.match.ref] (/1.1), but not the
    /// constructors of the /5.4.1 temporary (clang
    /// `allowExplicitConversionFunctionsInRefBinding`).
    pub const EXPLICIT_REF_CONV: IcsOptions = IcsOptions(16);
    /// The constructor candidates of a direct-initialization with one
    /// argument: a copy or move constructor's first parameter gets
    /// [`Self::EXPLICIT_REF_CONV`] ([over.match.copy]/1.2,
    /// [over.match.ref]/1.1; clang `hasCopyOrMoveCtorParam`). Consumed by
    /// [`Sema::add_found_candidates`], never seen by a sequence.
    pub const DIRECT_COPY_CTOR: IcsOptions = IcsOptions(32);
    /// The constructor candidates of the second phase of
    /// [over.match.list] for a list whose only element is itself a
    /// braced-init-list, in an implicit conversion sequence: a
    /// constructor whose first parameter is (a reference to) cv the class
    /// gets [`Self::SUPPRESS_UDC`] there ([over.best.ics.general]/4.5;
    /// clang `isFirstArgumentCompatibleWithType`). Consumed by
    /// [`Sema::add_found_candidates`], never seen by a sequence.
    pub const LIST_COPY_CTOR: IcsOptions = IcsOptions(64);

    /// The raw bits.
    pub(crate) fn bits(self) -> u8 {
        self.0
    }

    /// All of `o`'s bits are set.
    pub(crate) fn contains(self, o: IcsOptions) -> bool {
        self.0 & o.0 == o.0
    }

    /// The union.
    #[must_use]
    pub(crate) fn with(self, o: IcsOptions) -> IcsOptions {
        IcsOptions(self.0 | o.0)
    }
}

/// How a user-defined conversion converts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum UdcVia {
    /// A converting constructor ([class.conv.ctor]).
    Ctor,
    /// A conversion function ([class.conv.fct]).
    ConvFn,
    /// Aggregate initialization from a braced-init-list
    /// ([over.ics.list]/8).
    Aggregate,
}

/// A user-defined conversion sequence ([over.ics.user]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct UserConv {
    /// The standard conversion before the user-defined conversion.
    pub before: StandardConversion,
    /// The constructor or conversion function (`None` for aggregates).
    pub func: Option<DeclId>,
    /// The declaration lookup found.
    pub found: Option<DeclId>,
    /// The standard conversion after it ([over.ics.rank]/3.3).
    pub after: StandardConversion,
    /// The kind.
    pub via: UdcVia,
}

/// Why there is no implicit conversion sequence (for the candidate
/// notes, spec/sema-overload.md §9.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BadKind {
    /// "no known conversion from 'X' to 'Y'".
    NoConversion,
    /// A non-const lvalue reference and an rvalue: "expects an lvalue".
    LvalueRequired,
    /// An rvalue reference and an lvalue: "expects an rvalue".
    RvalueRequired,
    /// A reference binding drops these qualifiers.
    LosesQuals(Quals),
    /// The object argument is more qualified than the method.
    ObjectQuals,
    /// A braced-init-list with no conversion to a non-class type.
    InitList,
}

/// A failed implicit conversion sequence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct BadConv {
    /// Why.
    pub kind: BadKind,
    /// The argument's type.
    pub from: QualType,
    /// The parameter's type.
    pub to: QualType,
}

/// An implicit conversion sequence ([over.best.ics]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Ics {
    /// A standard conversion sequence (with its reference binding).
    Standard(StandardConversion),
    /// A user-defined conversion sequence.
    UserDefined(UserConv),
    /// An ellipsis conversion sequence ([over.ics.ellipsis]).
    Ellipsis,
    /// More than one user-defined conversion ([over.best.ics]/10): ranks
    /// as a user-defined conversion indistinguishable from any other.
    Ambiguous {
        /// The argument's type.
        from: QualType,
        /// The target.
        to: QualType,
    },
    /// None.
    Bad(BadConv),
}

const _: () = assert!(size_of::<Ics>() <= 96);

impl Ics {
    /// No conversion sequence.
    pub(crate) fn is_bad(&self) -> bool {
        matches!(self, Ics::Bad(_))
    }

    /// [over.ics.rank]/2: standard < user-defined < ellipsis.
    fn class(&self) -> u8 {
        match self {
            Ics::Standard(_) => 0,
            Ics::UserDefined(_) | Ics::Ambiguous { .. } => 1,
            Ics::Ellipsis => 2,
            Ics::Bad(_) => 3,
        }
    }

    /// A standard sequence never compared (a static member's object).
    pub(crate) fn is_ignored(&self) -> bool {
        matches!(self, Ics::Standard(s) if s.rb.contains(RefBinding::IGNORED))
    }

    fn bad(kind: BadKind, from: QualType, to: QualType) -> Ics {
        Ics::Bad(BadConv { kind, from, to })
    }
}

/// The standard conversion of a class glvalue or prvalue of type `from`
/// to its base `to` ([over.best.ics]/6).
pub(crate) fn derived_to_base(from: QualType, to: QualType, rb: RefBinding) -> StandardConversion {
    let mut s = StandardConversion::identity(from, to, rb);
    s.second = SecondStep::DerivedToBase;
    s.to_types = [from, to, to];
    s
}

impl Sema<'_, '_> {
    /// The [`ArgInfo`] of expression `e`.
    pub(crate) fn arg_info(&self, e: ExprId) -> ArgInfo {
        let n = *self.ast.exprs.get(e);
        ArgInfo {
            ty: n.ty(),
            vk: n.value_category(),
            npc: conv::is_null_pointer_constant(self, e),
            bit_field: conv::bit_field_width(self, e),
            expr: Some(e),
            list: None,
        }
    }

    /// The [`ArgInfo`] of constructor argument `e`: a braced-init-list
    /// placeholder ([`Sema::act_on_braced_arg`], `T x({1, 2})`) is a list
    /// argument ([over.ics.list]); its list stays in `ovl.lists` for the
    /// set's notes.
    pub(crate) fn ctor_arg_info(&mut self, e: ExprId) -> ArgInfo {
        let Some(l) = self.braced_args.iter().find(|(x, _)| *x == e).map(|(_, l)| l.clone()) else {
            return self.arg_info(e);
        };
        let idx = u32::try_from(self.ovl.lists.len()).unwrap_or(u32::MAX);
        self.ovl.lists.push(l);
        ArgInfo { list: Some(idx), ..ArgInfo::of_type(QualType::VOID, ValueCategory::PRValue) }
    }

    fn ics_is_error(&self, q: QualType) -> bool {
        let t = &self.ast.types;
        t.canonical(q).without_quals() == QualType::ERROR || t.is_dependent(q)
    }

    /// Whether `from` and `to` (or the type `to` refers to) are the same
    /// class, one of them the current instantiation in its own template
    /// (the injected-class-name and `X<params...>` are one type,
    /// [temp.dep.type]/1): the conversion is a reference binding or an
    /// identity whatever the template arguments, so it is ranked as one
    /// (a copy constructor over a move constructor for an lvalue).
    fn same_dependent_class(&mut self, from: QualType, to: QualType) -> bool {
        let t = &self.ast.types;
        let referee = match t.canonical_view(to) {
            TypeView::LValueRef(p) | TypeView::RValueRef(p) => p,
            _ => to,
        };
        let err = |q: QualType| t.canonical(q).without_quals() == QualType::ERROR;
        if err(from) || err(referee) {
            return false;
        }
        let (a, b) = (t.canonical(from).without_quals(), t.canonical(referee).without_quals());
        if a == b {
            // The canonical current instantiation is a dependent
            // specialization, a record only through its sugar.
            return t.as_record(a).is_some()
                || t.as_record(from).is_some()
                || matches!(t.canonical_view(a), TypeView::TemplateSpecialization { .. });
        }
        match (t.as_record(a), t.as_record(b)) {
            (Some(x), Some(y)) => same_class(self, x, y),
            (Some(x), None) => {
                let q = self.this_class_type(x);
                self.ast.types.canonical(q).without_quals() == b
            }
            (None, Some(y)) => {
                let q = self.this_class_type(y);
                self.ast.types.canonical(q).without_quals() == a
            }
            (None, None) => false,
        }
    }

    /// `to` (a reference to, or an object of, the class of `from`) spelt
    /// with `from`'s class type, keeping `to`'s qualifiers.
    fn retarget_to_class(&mut self, from: QualType, to: QualType) -> QualType {
        let t = &mut self.ast.types;
        let (r, referee) = match t.canonical_view(to) {
            TypeView::LValueRef(p) => (Some(true), p),
            TypeView::RValueRef(p) => (Some(false), p),
            _ => (None, to),
        };
        let cv = t.effective_quals(referee) & Quals::CV;
        let class = t.canonical(from).without_quals();
        let obj = t.qualify(class, cv);
        match r {
            Some(true) => t.lvalue_ref_to(obj),
            Some(false) => t.rvalue_ref_to(obj),
            None => obj,
        }
    }

    /// The implicit conversion sequence converting `a` to `to`
    /// ([over.best.ics]).
    pub(crate) fn try_implicit_conversion(
        &mut self,
        a: &ArgInfo,
        to: QualType,
        opts: IcsOptions,
    ) -> Ics {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("try_implicit_conversion");
        self.ovl.stats.ics += 1;
        if let Some(i) = a.list {
            return self.list_ics_at(i, to, opts);
        }
        let mut to = to;
        if self.ics_is_error(a.ty) || self.ics_is_error(to) {
            if !self.same_dependent_class(a.ty, to) {
                return Ics::Standard(StandardConversion::identity(a.ty, to, RefBinding::NONE));
            }
            to = self.retarget_to_class(a.ty, to);
        }
        // [over.over]/1: a class type is not a target; an overload set
        // converts to one through a converting constructor taking it.
        if a.ty == QualType::OVERLOAD && self.ast.types.as_record(self.non_reference(to)).is_none()
        {
            return self.overload_set_ics(a, to, opts);
        }
        if self.ast.types.is_reference(to) {
            return self.ref_ics(a, to, opts);
        }
        let (fr, tr) = (self.ast.types.as_record(a.ty), self.ast.types.as_record(to));
        if let Some(t) = tr {
            if let Some(f) = fr {
                if same_class(self, f, t) {
                    return Ics::Standard(StandardConversion::identity(a.ty, to, RefBinding::NONE));
                }
                if complete_class(self, f).is_some() && is_derived_from(self, f, t) {
                    return Ics::Standard(derived_to_base(a.ty, to, RefBinding::NONE));
                }
            }
            if opts.contains(IcsOptions::SUPPRESS_UDC) {
                return Ics::bad(BadKind::NoConversion, a.ty, to);
            }
            return self.user_conversion(a, to, SetKind::CopyInit, opts);
        }
        if fr.is_some() {
            if opts.contains(IcsOptions::SUPPRESS_UDC) {
                return Ics::bad(BadKind::NoConversion, a.ty, to);
            }
            return self.user_conversion(a, to, SetKind::ConvToNonClass, opts);
        }
        match standard_conversion_from(self, a.ty, a.vk, a.npc, a.bit_field, to, ConvCtx::Argument)
        {
            Some(s) => Ics::Standard(s),
            None => Ics::bad(BadKind::NoConversion, a.ty, to),
        }
    }

    /// Whether overload set `e` converts to `to`, a class or reference to
    /// class, through a constructor taking it (the set is the
    /// constructor's argument, not an [over.over] target: folly's
    /// `const dynamic& v = dynamic::object` with `dynamic(ObjectMaker
    /// (*)())`). Otherwise the set is resolved against `to` and fails
    /// with clang's "address of overloaded function" error.
    pub(crate) fn overload_set_converts_to_class(
        &mut self,
        e: ExprId,
        to: QualType,
        opts: IcsOptions,
    ) -> bool {
        self.ety(e) == QualType::OVERLOAD
            && self.ast.types.as_record(self.non_reference(to)).is_some()
            && !self.try_implicit_conversion(&self.arg_info(e), to, opts).is_bad()
    }

    /// The conversion sequence of an overload-set argument ([over.ics.scs]
    /// with [over.over]): that of the function the target selects (an
    /// lvalue of function type, or its address for `&f`), else none.
    fn overload_set_ics(&mut self, a: &ArgInfo, to: QualType, opts: IcsOptions) -> Ics {
        let Some(e) = a.expr else { return Ics::bad(BadKind::NoConversion, a.ty, to) };
        let Some(d) = self.address_match(e, to) else {
            return Ics::bad(BadKind::NoConversion, a.ty, to);
        };
        let ft = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let addr = self
            .overload_operand(e)
            .or_else(|| self.implicit_member_operand(e))
            .is_some_and(|(_, addr)| addr);
        let class = self.is_implicit_object_member(d).then(|| self.declaring_class(d)).flatten();
        let f = if let Some(c) = class {
            let ct = self.tag_type(c).ty();
            ArgInfo::of_type(self.ast.types.member_pointer(ft, ct), ValueCategory::PRValue)
        } else if addr {
            ArgInfo::of_type(self.ast.types.pointer_to(ft), ValueCategory::PRValue)
        } else {
            ArgInfo::of_type(ft, ValueCategory::LValue)
        };
        match self.try_implicit_conversion(&f, to, opts) {
            // The selection does not convert (a specialization deduced
            // ignoring its exception specification, [temp.deduct.funcaddr]):
            // the argument is still the overload set (clang reports it
            // as `no overload of 'f' matching`).
            Ics::Bad(_) => Ics::bad(BadKind::NoConversion, a.ty, to),
            x => x,
        }
    }

    /// Reference-relatedness of `t1` (the referee) and `t2`, both
    /// canonical and unqualified ([dcl.init.ref]/4): `(related,
    /// derived)`. A class template specialization `t2` is instantiated
    /// first (clang `CompareReferenceRelationship`'s `isCompleteType`).
    pub(crate) fn ref_related(&mut self, t1: QualType, t2: QualType) -> (bool, bool) {
        if t1 == t2 || self.unknown_bound_of(t1, t2) {
            return (true, false);
        }
        if let (Some(d), Some(b)) = (self.ast.types.as_record(t2), self.ast.types.as_record(t1))
            && !same_class(self, d, b)
        {
            let _ = self.require_complete_silent(t2, Loc::INVALID);
        }
        let t = &self.ast.types;
        match (t.as_record(t2), t.as_record(t1)) {
            (Some(d), Some(b)) if same_class(self, d, b) => (true, false),
            (Some(d), Some(b))
                if complete_class(self, d).is_some() && is_derived_from(self, d, b) =>
            {
                (true, true)
            }
            _ => (false, false),
        }
    }

    /// True if `t1` is an array of unknown bound of the element type of
    /// the array of known bound `t2`, both canonical and unqualified: a
    /// `T[N]` glvalue binds a `T (&)[]` directly ([dcl.init.ref]/4: the
    /// types are similar and `T (*)[N]` converts to `T (*)[]` by a
    /// qualification conversion, [conv.qual]/3, P0388R4). clang
    /// reports the binding as unrelated when it would drop qualifiers.
    pub(crate) fn unknown_bound_of(&self, t1: QualType, t2: QualType) -> bool {
        let t = &self.ast.types;
        match (t.view(t1.ty()), t.view(t2.ty())) {
            (TypeView::IncompleteArray(e1), TypeView::ConstantArray(a)) => {
                t.same_type(t.canonical(e1), t.canonical(a.elem))
            }
            _ => false,
        }
    }

    /// The canonical type of `q` without its qualifiers, including an
    /// array's element qualifiers (clang `getUnqualifiedArrayType`).
    fn unqualified_canonical(&mut self, q: QualType) -> QualType {
        let c = self.ast.types.canonical(q);
        let u = self.ast.types.unqualified(c);
        self.ast.types.canonical(u)
    }

    /// The conversion sequence binding reference type `to` to `a`
    /// ([over.ics.ref], clang `TryReferenceInit`).
    #[allow(clippy::too_many_lines, reason = "one step per bullet of [dcl.init.ref]/5")]
    fn ref_ics(&mut self, a: &ArgInfo, to: QualType, opts: IcsOptions) -> Ics {
        let (lref, t1) = match self.ast.types.canonical_view(to) {
            TypeView::LValueRef(p) => (true, p),
            TypeView::RValueRef(p) => (false, p),
            _ => return Ics::bad(BadKind::NoConversion, a.ty, to),
        };
        // Unqualified through arrays: `const int[3]` and `int[3]` are
        // reference-related ([dcl.init.ref]/4, [basic.type.qualifier]/3).
        let u1 = self.unqualified_canonical(t1);
        let u2 = self.unqualified_canonical(a.ty);
        let types = &self.ast.types;
        let (q1, q2) = (types.effective_quals(t1), types.effective_quals(a.ty));
        let is_fn = types.is_function(u1);
        let class_src = types.as_record(u2).is_some();
        let class_or_array = class_src || types.is_array(u2);
        let (mut related, derived) = self.ref_related(u1, u2);
        // A function lvalue binds a reference to the same function type
        // without `noexcept` directly, as an identity ([dcl.init.ref]/4,
        // [over.ics.ref]/1): it ranks like the exact match, so
        // `n(void (&)(int) noexcept)` and `n(void (&)(int))` are
        // ambiguous for a `noexcept` function (clang).
        if is_fn && !related {
            related = function_ref_compatible(self, u1, u2);
        }
        if related && !q1.contains(q2) && self.unknown_bound_of(u1, u2) {
            related = false;
        }
        let compatible = related && q1.contains(q2);
        let lv = a.vk == ValueCategory::LValue;
        let base =
            if lref { RefBinding::IS_REF.with(RefBinding::LVALUE_REF) } else { RefBinding::IS_REF };
        let direct = |rb: RefBinding| {
            let mut s = if derived {
                derived_to_base(u2, u1, rb)
            } else {
                StandardConversion::identity(u2, u1, rb)
            };
            s.from = a.ty;
            s.to_types[2] = t1;
            s
        };
        // /5.1.1: an lvalue reference (or a function lvalue) binds a
        // reference-compatible lvalue.
        if lv && compatible && (lref || is_fn) {
            let mut rb = base.with(RefBinding::DIRECT);
            if is_fn {
                rb = rb.with(RefBinding::BINDS_FUNCTION_LVALUE);
            }
            return Ics::Standard(direct(rb));
        }
        let suppress = opts.contains(IcsOptions::SUPPRESS_UDC);
        let ref_opts = if opts.contains(IcsOptions::EXPLICIT_REF_CONV) {
            opts.with(IcsOptions::ALLOW_EXPLICIT)
        } else {
            opts
        };
        // /5.1.2: an lvalue result of a conversion function.
        if lref
            && !related
            && class_src
            && !suppress
            && let Some(x) = self.ref_conversion(a, to, true, ref_opts)
        {
            return x;
        }
        let plain_lref = lref && (!q1.is_const() || q1.is_volatile());
        if plain_lref {
            let kind = match (related, lv) {
                (true, false) => BadKind::LvalueRequired,
                (true, true) => BadKind::LosesQuals(q2 - q1),
                _ => BadKind::NoConversion,
            };
            return Ics::bad(kind, a.ty, to);
        }
        // /5.3.1: a reference-compatible xvalue, class or array prvalue,
        // or function lvalue.
        if !lv && compatible && (a.vk == ValueCategory::XValue || class_or_array) {
            return Ics::Standard(direct(
                base.with(RefBinding::DIRECT).with(RefBinding::BINDS_RVALUE),
            ));
        }
        // /5.3.2: an rvalue result of a conversion function.
        if !related
            && class_src
            && !suppress
            && let Some(x) = self.ref_conversion(a, to, false, ref_opts)
        {
            return x;
        }
        // /5.4.4: an rvalue reference to a related lvalue.
        if !lref && lv && related {
            return Ics::bad(BadKind::RvalueRequired, a.ty, to);
        }
        if related && !q1.contains(q2) {
            return Ics::bad(BadKind::LosesQuals(q2 - q1), a.ty, to);
        }
        if opts.contains(IcsOptions::NO_TEMPORARY) || is_fn {
            return Ics::bad(BadKind::NoConversion, a.ty, to);
        }
        // /5.4.2: a temporary copy-initialized from the argument. A class
        // temporary needs a constructor, a user-defined conversion
        // ([over.best.ics]/4; clang reports the reference type).
        if suppress && self.ast.types.as_record(t1).is_some() {
            return Ics::bad(BadKind::NoConversion, a.ty, to);
        }
        let target = conv::strip_cv(self, t1);
        let temp_rb = base.with(RefBinding::BINDS_RVALUE);
        // A non-class prvalue result of an explicit conversion function
        // binds in a direct-initialization ([over.match.ref]/1.1); a
        // class temporary's explicit constructors stay excluded.
        let temp_opts = if ref_opts != opts && self.ast.types.as_record(t1).is_none() {
            ref_opts
        } else {
            opts
        };
        match self.try_implicit_conversion(a, target, temp_opts) {
            Ics::Standard(mut s) => {
                s.rb = temp_rb;
                s.to_types[2] = t1;
                Ics::Standard(s)
            }
            // [over.ics.ref]/3: an rvalue reference cannot bind to the
            // lvalue a conversion function returns (clang
            // `TryReferenceInit`).
            Ics::UserDefined(u)
                if !lref
                    && u.via == UdcVia::ConvFn
                    && u.func.is_some_and(|f| {
                        let r = self.conversion_result_type(f);
                        matches!(self.ast.types.canonical_view(r), TypeView::LValueRef(_))
                    }) =>
            {
                Ics::bad(BadKind::NoConversion, a.ty, to)
            }
            Ics::UserDefined(mut u) => {
                u.after.rb = temp_rb;
                u.after.to_types[2] = t1;
                Ics::UserDefined(u)
            }
            // clang reports the referent ('const H'), the temporary's type.
            Ics::Bad(b) => Ics::bad(b.kind, a.ty, t1),
            Ics::Ambiguous { .. } => Ics::Ambiguous { from: a.ty, to },
            Ics::Ellipsis => Ics::Ellipsis,
        }
    }

    /// The implicit object parameter's conversion sequence for method
    /// `m` and object `o` ([over.match.funcs]/4–5, clang
    /// `TryObjectArgumentInitialization`).
    pub(crate) fn object_ics(&mut self, o: &ArgInfo, m: DeclId) -> Ics {
        self.object_ics_found(o, m, m)
    }

    /// [`Self::object_ics`] for method `m` found as `found`: a member
    /// nominated by a using-declaration is a member of the class of the
    /// using-declaration for the type of its implicit object parameter
    /// ([over.match.funcs.general]/4; clang's `ActingContext`).
    pub(crate) fn object_ics_found(&mut self, o: &ArgInfo, m: DeclId, found: DeclId) -> Ics {
        self.ovl.stats.ics += 1;
        if self.ics_is_error(o.ty) {
            return super::ignored_object(o.ty);
        }
        let acting = (found != m && self.kind_of(found) == DeclKind::UsingShadow)
            .then(|| self.ast.decls.get(found).ctx())
            .flatten()
            .filter(|&c| self.kind_of(c) == DeclKind::CXXRecord);
        let Some(cls) = acting.or_else(|| self.declaring_class(m)) else {
            return super::ignored_object(o.ty);
        };
        let cty = self.ast.decls.get(cls).ty().unwrap_or(QualType::ERROR);
        let (q, rq) = self.method_object_quals(m);
        let param = self.ast.types.qualify(cty, q);
        let Some(or) = self.ast.types.as_record(o.ty) else {
            return Ics::bad(BadKind::NoConversion, o.ty, param);
        };
        let derived = if same_class(self, or, cls) {
            false
        } else if complete_class(self, or).is_some() && is_derived_from(self, or, cls) {
            true
        } else {
            return Ics::bad(BadKind::NoConversion, o.ty, param);
        };
        let oq = self.ast.types.effective_quals(o.ty);
        if !q.contains(oq) {
            return Ics::bad(BadKind::ObjectQuals, o.ty, param);
        }
        let lv = o.vk == ValueCategory::LValue;
        let mut rb = RefBinding::IS_REF.with(RefBinding::DIRECT);
        if !lv {
            rb = rb.with(RefBinding::BINDS_RVALUE);
        }
        match rq {
            RefQual::None => {
                rb = rb.with(RefBinding::LVALUE_REF).with(RefBinding::OBJECT_NO_REF_QUAL);
            }
            RefQual::LValue => {
                if !lv && (!q.is_const() || q.is_volatile()) {
                    return Ics::bad(BadKind::LvalueRequired, o.ty, param);
                }
                rb = rb.with(RefBinding::LVALUE_REF);
            }
            RefQual::RValue => {
                if lv {
                    return Ics::bad(BadKind::RvalueRequired, o.ty, param);
                }
            }
        }
        let mut s = if derived {
            derived_to_base(o.ty, cty, rb)
        } else {
            StandardConversion::identity(o.ty, cty, rb)
        };
        s.to_types[2] = param;
        Ics::Standard(s)
    }

    /// True for a reference that binds no temporary: an lvalue
    /// reference to non-const or volatile.
    fn nonconst_lref(&self, to: QualType) -> bool {
        matches!(self.ast.types.canonical_view(to), TypeView::LValueRef(_))
            && self.ast.types.pointee(to).is_some_and(|r| {
                let q = self.ast.types.effective_quals(r);
                !q.is_const() || q.is_volatile()
            })
    }

    /// The ICS of braced-init-list argument `i` (in `self.ovl.lists`).
    fn list_ics_at(&mut self, i: u32, to: QualType, opts: IcsOptions) -> Ics {
        let Some(slot) = self.ovl.lists.get_mut(i as usize) else {
            return Ics::bad(BadKind::InitList, QualType::ERROR, to);
        };
        let empty = InitList { items: Vec::new(), lbrace: slot.lbrace, rbrace: slot.rbrace };
        let l = std::mem::replace(slot, empty);
        let r = self.list_ics(&l, to, opts);
        // A reference bound to a reference-related element fails as
        // that binding ("would lose const qualifier").
        let elem = self.ast.types.pointee(to).filter(|_| self.ast.types.is_reference(to));
        let bound = elem.is_some_and(|p| self.list_ref_element(&l, p).is_some());
        self.ovl.lists[i as usize] = l;
        if bound {
            return r;
        }
        // clang's `TryListConversion` fails with the list itself as the
        // source: "cannot convert initializer list argument to 'T'" (a
        // non-const lvalue reference whose temporary would initialize
        // keeps "expects an lvalue").
        match r {
            Ics::Bad(b) if b.kind == BadKind::LvalueRequired && self.nonconst_lref(to) => {
                Ics::Bad(b)
            }
            Ics::Bad(b) if b.kind != BadKind::InitList => {
                Ics::bad(BadKind::InitList, QualType::VOID, b.to)
            }
            r => r,
        }
    }

    /// Records on `r` that it converts list `l` to an array of `elem`
    /// (`to`), for the [over.ics.rank]/3.1.2 tie-breaker.
    fn with_list_array(&self, mut r: Ics, l: &InitList, elem: QualType, to: QualType) -> Ics {
        let len = self.ast.types.array_size(to);
        let la =
            ListArray { elem, len: len.unwrap_or(l.items.len() as u64), unbounded: len.is_none() };
        match &mut r {
            Ics::Standard(s) => s.list_array = Some(la),
            Ics::UserDefined(u) => u.after.list_array = Some(la),
            _ => {}
        }
        r
    }

    /// `l`'s only element, if it is an expression reference-related to
    /// `r` ([dcl.init.ref]/4).
    fn list_ref_element(&mut self, l: &InitList, r: QualType) -> Option<ArgInfo> {
        let [InitItem { value: InitValue::Expr(e), designator }] = l.items.as_slice() else {
            return None;
        };
        if !designator.is_empty() {
            return None;
        }
        let a = self.arg_info(*e);
        let (u1, u2) = (self.unqualified_canonical(r), self.unqualified_canonical(a.ty));
        self.ref_related(u1, u2).0.then_some(a)
    }

    /// [over.ics.list]/9: the ICS of braced-init-list `l` to the
    /// reference type `to` (referring to `r`).
    fn list_ref_ics(&mut self, l: &InitList, to: QualType, r: QualType, opts: IcsOptions) -> Ics {
        let void_from = QualType::VOID;
        let inner = conv::strip_cv(self, r);
        // One element reference-related to the referee: the reference
        // is initialized from it ([dcl.init.list]/3.10; clang's
        // `TryListConversion`), `f({x})` for `f(const X&)`. A failed
        // binding binds no temporary either (`g({x})` for `g(X&&)` with
        // `x` an lvalue is not viable).
        if let Some(a) = self.list_ref_element(l, r) {
            return self.try_implicit_conversion(&a, to, opts);
        }
        let lref = matches!(self.ast.types.canonical_view(to), TypeView::LValueRef(_));
        let q = self.ast.types.effective_quals(r);
        if lref && (!q.is_const() || q.is_volatile()) {
            // A non-const lvalue reference binds no temporary (an
            // element of the referenced type is not handled here).
            if let [InitItem { value: InitValue::Expr(e), designator }] = l.items.as_slice()
                && designator.is_empty()
            {
                let a = self.arg_info(*e);
                let x = self.try_implicit_conversion(&a, to, opts);
                if !x.is_bad() {
                    return x;
                }
            }
            // clang's `TryListConversion`: the temporary would be
            // list-initialized, then fails to bind ("expects an
            // lvalue"); otherwise the list does not convert to `T`.
            return match self.list_ics(l, inner, opts) {
                Ics::Bad(_) => Ics::bad(BadKind::InitList, void_from, r),
                _ => Ics::bad(BadKind::LvalueRequired, void_from, to),
            };
        }
        let rb = if lref {
            RefBinding::IS_REF.with(RefBinding::LVALUE_REF).with(RefBinding::BINDS_RVALUE)
        } else {
            RefBinding::IS_REF.with(RefBinding::BINDS_RVALUE)
        };
        match self.list_ics(l, inner, opts) {
            Ics::Standard(mut s) => {
                s.rb = rb;
                s.to_types[2] = r;
                Ics::Standard(s)
            }
            Ics::UserDefined(mut u) => {
                u.after.rb = rb;
                u.after.to_types[2] = r;
                Ics::UserDefined(u)
            }
            // The failure names the referenced type (clang recurses
            // on `T1`).
            Ics::Bad(_) => Ics::bad(BadKind::InitList, void_from, r),
            other => other,
        }
    }

    /// The ICS of a braced-init-list ([over.ics.list]).
    pub(crate) fn list_ics(&mut self, l: &InitList, to: QualType, opts: IcsOptions) -> Ics {
        let void_from = QualType::VOID;
        if self.ics_is_error(to) {
            return Ics::Standard(StandardConversion::identity(to, to, RefBinding::NONE));
        }
        // /9: a reference: as the referenced type (a temporary).
        if self.ast.types.is_reference(to)
            && let Some(r) = self.ast.types.pointee(to)
        {
            return self.list_ref_ics(l, to, r, opts);
        }
        // /6: an array: the worst element conversion.
        if self.ast.types.is_array(to) {
            let Some(elem) = self.ast.types.element(to) else {
                return Ics::bad(BadKind::InitList, void_from, to);
            };
            if let Some(n) = self.ast.types.array_size(to)
                && l.items.len() as u64 > n
            {
                return Ics::bad(BadKind::InitList, void_from, to);
            }
            let r = self.worst_element(l, elem, to, opts);
            return self.with_list_array(r, l, elem, to);
        }
        if let Some(c) = self.ast.types.as_record(to) {
            // /5: std::initializer_list<X>.
            if let Some(elem) = self.initializer_list_element(to) {
                let mut r = self.worst_element(l, elem, to, opts);
                match &mut r {
                    Ics::Standard(s) => s.std_init_list = true,
                    Ics::UserDefined(u) => u.after.std_init_list = true,
                    _ => {}
                }
                return r;
            }
            if !self.require_complete_silent(to, Loc::INVALID) {
                return Ics::bad(BadKind::InitList, void_from, to);
            }
            // /7: a non-aggregate class: constructors ([over.match.list]).
            if !self.is_aggregate(to) {
                // /2: a designated-initializer-list converts only to an
                // aggregate.
                if l.items.iter().any(|i| !i.designator.is_empty()) {
                    return Ics::bad(BadKind::InitList, void_from, to);
                }
                // [over.best.ics.general]/4: a suppressed user-defined
                // conversion (clang `TryUserDefinedConversion`), unless the
                // only element is the class or derived from it (/2).
                if opts.contains(IcsOptions::SUPPRESS_UDC)
                    && self.single_elem_of_class(l, c).is_none()
                {
                    return Ics::bad(BadKind::InitList, void_from, to);
                }
                return self.list_ctor_conversion(l, c, to, opts);
            }
            // /3: an aggregate from one element of its class or a class
            // derived from it: that element's conversion (CWG2076).
            if let Some(e) = self.single_elem_of_class(l, c) {
                let a = self.arg_info(e);
                return match self.try_implicit_conversion(&a, to, opts) {
                    Ics::Bad(_) => Ics::bad(BadKind::InitList, void_from, to),
                    x => x,
                };
            }
            // /8: an aggregate.
            if self.aggregate_list_fits(l, to) {
                let id = StandardConversion::identity(to, to, RefBinding::NONE);
                return Ics::UserDefined(UserConv {
                    before: id,
                    func: None,
                    found: None,
                    after: id,
                    via: UdcVia::Aggregate,
                });
            }
            return Ics::bad(BadKind::InitList, void_from, to);
        }
        // /10-11: a non-class type.
        match l.items.as_slice() {
            [] => Ics::Standard(StandardConversion::identity(to, to, RefBinding::NONE)),
            [InitItem { value: InitValue::Expr(e), designator }] if designator.is_empty() => {
                let a = self.arg_info(*e);
                match self.try_implicit_conversion(&a, to, opts) {
                    Ics::Bad(_) => Ics::bad(BadKind::InitList, void_from, to),
                    x => x,
                }
            }
            _ => Ics::bad(BadKind::InitList, void_from, to),
        }
    }

    /// `l`'s only element, if it is an expression of class `c` or a
    /// class derived from it ([over.ics.list]/3).
    fn single_elem_of_class(&mut self, l: &InitList, c: DeclId) -> Option<ExprId> {
        let [InitItem { value: InitValue::Expr(e), designator }] = l.items.as_slice() else {
            return None;
        };
        let ty = self.ety(*e);
        let of_class = designator.is_empty()
            && self.ast.types.as_record(ty).is_some_and(|f| {
                same_class(self, f, c)
                    || (complete_class(self, f).is_some() && is_derived_from(self, f, c))
            });
        of_class.then_some(*e)
    }

    /// The worst conversion of the elements of `l` to `elem`
    /// ([over.ics.list]/5-6).
    fn worst_element(
        &mut self,
        l: &InitList,
        elem: QualType,
        to: QualType,
        opts: IcsOptions,
    ) -> Ics {
        let mut worst = Ics::Standard(StandardConversion::identity(to, to, RefBinding::NONE));
        for item in &l.items {
            if !item.designator.is_empty() {
                return Ics::bad(BadKind::InitList, QualType::VOID, to);
            }
            let x = match &item.value {
                InitValue::Expr(e) => {
                    let a = self.arg_info(*e);
                    self.try_implicit_conversion(&a, elem, opts)
                }
                InitValue::List(inner) => self.list_ics(inner, elem, opts),
            };
            if x.is_bad() {
                return Ics::bad(BadKind::InitList, QualType::VOID, to);
            }
            if self.compare_ics(&x, &worst) == IcsCompare::Worse {
                worst = x;
            }
        }
        // The sequence converts to the list type itself.
        match &mut worst {
            Ics::Standard(s) => s.to_types[2] = to,
            Ics::UserDefined(u) => u.after.to_types[2] = to,
            _ => {}
        }
        worst
    }

    /// The element type `E` of `to` = `std::initializer_list<E>`.
    pub(crate) fn initializer_list_element(&mut self, to: QualType) -> Option<QualType> {
        let r = self.ast.types.as_record(to)?;
        let name = kiln_ast::names::DeclName::ident(self.intern("initializer_list"));
        if self.ast.decls.get(r).name() != name {
            return None;
        }
        let args = self.ast.spec_info(r).map(|(_, s)| s.args)?;
        match self.ast.types.args(args).first()?.view() {
            kiln_ast::types::ArgView::Type(t) => Some(t),
            _ => None,
        }
    }

    /// Whether `l` can initialize aggregate `to` ([over.ics.list]/8:
    /// "if the initializer list can initialize `to` by aggregate
    /// initialization"; clang's verify-only `InitListChecker`). An
    /// undesignated list is walked under brace elision
    /// ([`Sema::aggregate_list_walk`]); a designated one, or one the walk
    /// does not model, is bounded by the element count and the
    /// designators ([`Sema::designators_override`]).
    fn aggregate_list_fits(&mut self, l: &InitList, to: QualType) -> bool {
        if l.items.iter().all(|i| i.designator.is_empty()) {
            let mut pos = 0;
            if let Some(ok) = self.aggregate_list_walk(to, &l.items, &mut pos, true) {
                return ok;
            }
        }
        l.items.len() as u64 <= self.aggregate_capacity(to) && !self.designators_override(to, l)
    }

    /// The elements of aggregate `to` in initialization order
    /// ([dcl.init.aggr]/2: bases, then non-static data members without
    /// unnamed bit-fields; a union's first member; `n` copies of an
    /// array's element type): each type and the field it is (for the
    /// default-initialization of the elements a list leaves out).
    /// `None` for an array without a bound or a vector.
    fn aggregate_elements(&mut self, to: QualType) -> Option<Vec<(QualType, Option<DeclId>)>> {
        if self.ast.types.is_array(to) {
            let n = self.ast.types.array_size(to)?;
            let elem = self.ast.types.element(to)?;
            let n = usize::try_from(n).ok().filter(|&n| n <= 1 << 16)?;
            return Some(vec![(elem, None); n]);
        }
        let r = self.ast.types.as_record(to)?;
        if !self.require_complete_silent(to, Loc::INVALID) {
            return None;
        }
        let def = self.ast.definition(r).unwrap_or(r);
        let mut out: Vec<(QualType, Option<DeclId>)> =
            self.init_bases(def).into_iter().map(|b| (b, None)).collect();
        let union =
            self.ast.decls.record(def).is_some_and(|x| x.tag == kiln_ast::decl::TagKind::Union);
        for f in self.init_fields(def) {
            out.push((self.ast.decls.get(f).ty()?, Some(f)));
            if union {
                break;
            }
        }
        Some(out)
    }

    /// Walks the undesignated clauses `items[*pos..]` initializing
    /// aggregate `to` ([dcl.init.aggr]/4-5, /16, verify-only): each
    /// element takes a braced list whole (its own list conversion), or
    /// an expression that converts to it; an expression that does not
    /// convert to a subaggregate starts brace elision, the subaggregate
    /// taking the following clauses for its own elements. `braced` is
    /// false under brace elision (the walk stops at the subaggregate's
    /// last element); a braced list must be used up. The elements left
    /// over are initialized from their default member initializers or
    /// from `{}` (a reference member without one is ill-formed, /5.3).
    /// Brace elision into an aggregate without elements is ill-formed
    /// (clang `err_empty_scalar_initializer`'s aggregate form). `None`
    /// when the walk meets something it does not model (an unbounded
    /// array, a vector, an incomplete class).
    pub(crate) fn aggregate_list_walk(
        &mut self,
        to: QualType,
        items: &[InitItem],
        pos: &mut usize,
        braced: bool,
    ) -> Option<bool> {
        let elems = self.aggregate_elements(to)?;
        if !braced && self.aggregate_has_no_elements(to) {
            return Some(false);
        }
        let opts = IcsOptions::NONE;
        let mut next = 0;
        while next < elems.len() && *pos < items.len() {
            let (ety, _) = elems[next];
            next += 1;
            match &items[*pos].value {
                InitValue::List(sub) => {
                    *pos += 1;
                    if self.list_ics(sub, ety, opts).is_bad() {
                        return Some(false);
                    }
                }
                InitValue::Expr(e) => {
                    let e = *e;
                    if self.vector_parts(ety).is_some() {
                        return None;
                    }
                    let xty = self.ast.exprs.get(e).ty();
                    if self.is_error_type(xty) {
                        return None;
                    }
                    let a = self.arg_info(e);
                    let elide = if self.ast.types.is_array(ety) {
                        let el = self.ast.types.element(ety).unwrap_or(QualType::ERROR);
                        !(self.is_char_type(el) && self.string_lit_of(e).is_some())
                    } else if let Some(r) = self.ast.types.as_record(ety) {
                        self.is_aggregate(ety)
                            && self.ast.types.as_record(xty).is_none_or(|x| {
                                self.ast.first(x) != self.ast.first(r)
                                    && !is_derived_from(self, x, r)
                                    && self.try_implicit_conversion(&a, ety, opts).is_bad()
                            })
                    } else {
                        false
                    };
                    if elide {
                        if !self.aggregate_list_walk(ety, items, pos, false)? {
                            return Some(false);
                        }
                    } else {
                        *pos += 1;
                        if self.try_implicit_conversion(&a, ety, opts).is_bad() {
                            return Some(false);
                        }
                    }
                }
            }
        }
        if braced && *pos < items.len() {
            return Some(false);
        }
        let empty = InitList { items: Vec::new(), lbrace: Loc::INVALID, rbrace: Loc::INVALID };
        for &(ety, f) in &elems[next..] {
            let Some(f) = f else { continue };
            if self.field_has_nsdmi(f) || self.is_anonymous_member(f) {
                continue;
            }
            if self.ast.types.is_reference(ety) {
                return Some(false);
            }
            if self.ast.types.as_record(ety).is_some() && self.list_ics(&empty, ety, opts).is_bad()
            {
                return Some(false);
            }
        }
        Some(true)
    }

    /// The most elements a braced-init-list for aggregate `to` can have
    /// (brace elision reaches subaggregate elements, each taking at
    /// least one).
    fn aggregate_capacity(&self, to: QualType) -> u64 {
        self.aggregate_list_capacity(to).unwrap_or(u64::MAX)
    }

    /// [over.match.list] phase 2 as an ICS: the constructors of `c` with
    /// the elements of `l` as arguments (phase 1, initializer-list
    /// constructors, first).
    fn list_ctor_conversion(
        &mut self,
        l: &InitList,
        c: DeclId,
        to: QualType,
        opts: IcsOptions,
    ) -> Ics {
        let mut set = self.begin_set(SetKind::ListCtor { phase: 1 }, Loc::INVALID);
        // Phase 1: the list as the only argument of an initializer-list
        // constructor.
        if !l.items.is_empty() || !self.has_default_ctor_cheap(c) {
            let idx = u32::try_from(self.ovl.lists.len()).unwrap_or(u32::MAX);
            self.ovl.lists.push(l.clone());
            set.args.push(ArgInfo {
                list: Some(idx),
                ..ArgInfo::of_type(QualType::VOID, ValueCategory::PRValue)
            });
            self.add_ctor_candidates(&mut set, c, IcsOptions::NONE, true);
            self.ovl.lists.pop();
            self.drop_explicit_ctors(&mut set, opts);
            if set.viable().next().is_some() {
                let r = self.udc_outcome(&mut set, QualType::VOID, to, true);
                self.end_set(set);
                return r;
            }
            set.clear();
        }
        // Phase 2: the elements.
        set.kind = SetKind::ListCtor { phase: 2 };
        let mut pushed = 0;
        for item in &l.items {
            match &item.value {
                InitValue::Expr(e) => set.args.push(self.arg_info(*e)),
                InitValue::List(inner) => {
                    let idx = u32::try_from(self.ovl.lists.len()).unwrap_or(u32::MAX);
                    self.ovl.lists.push(inner.clone());
                    pushed += 1;
                    set.args.push(ArgInfo {
                        list: Some(idx),
                        ..ArgInfo::of_type(QualType::VOID, ValueCategory::PRValue)
                    });
                }
            }
        }
        // [over.best.ics.general]/4.5: one element that is itself a
        // braced-init-list, converting to the class through a copy or
        // move constructor, considers no user-defined conversion. Any
        // other element may use one (clang `IsUserDefinedConversion`
        // with `ListInitializing`: `H hh({1})` with `H(E)` finds the copy
        // and move constructors through `{1}` -> `H` too).
        let suppress = match l.items.as_slice() {
            [InitItem { value: InitValue::List(_), .. }] => IcsOptions::LIST_COPY_CTOR,
            _ => IcsOptions::NONE,
        };
        self.add_ctor_candidates(&mut set, c, suppress, false);
        for _ in 0..pushed {
            self.ovl.lists.pop();
        }
        self.drop_explicit_ctors(&mut set, opts);
        let r = self.udc_outcome(&mut set, QualType::VOID, to, false);
        self.end_set(set);
        r
    }

    /// clang's `IsInitializerListConstructorConversion` /
    /// `IsUserDefinedConversion` without `AllowExplicit`: as an implicit
    /// conversion sequence, list-initialization does not use explicit
    /// constructors (the initialization itself still considers them,
    /// [over.match.list]/1, and is then ill-formed).
    fn drop_explicit_ctors(&self, set: &mut super::OverloadCandidateSet, opts: IcsOptions) {
        if opts.contains(IcsOptions::ALLOW_EXPLICIT) {
            return;
        }
        for c in &mut set.cands {
            if c.viable && self.fn_spec(c.func()).is_explicit() {
                c.viable = false;
                c.failure = CandidateFailure::Explicit;
            }
        }
    }

    /// Whether `c` has a default constructor (a cheap check for the
    /// `{}` rule of [dcl.init.list]/3.5).
    pub(crate) fn has_default_ctor_cheap(&mut self, c: DeclId) -> bool {
        let c = self.class_def(c);
        self.ensure_implicit(c, crate::class::SpecialKind::DefaultCtor);
        let ctors: Vec<DeclId> = self.usable_ctors(c);
        ctors
            .into_iter()
            .any(|f| self.kind_of(f) == DeclKind::CXXConstructor && self.fn_min_args(f) == 0)
    }

    /// Adds the constructors of class `c` to `set` (the set's arguments
    /// are the constructor arguments). With `list_only`, only
    /// initializer-list constructors. `opts` applies to the first
    /// parameter.
    pub(crate) fn add_ctor_candidates(
        &mut self,
        set: &mut super::OverloadCandidateSet,
        c: DeclId,
        first_opts: IcsOptions,
        list_only: bool,
    ) {
        let c = self.class_def(c);
        self.ensure_implicit(c, crate::class::SpecialKind::DefaultCtor);
        self.ensure_implicit(c, crate::class::SpecialKind::CopyCtor);
        self.ensure_implicit(c, crate::class::SpecialKind::MoveCtor);
        let mut found = std::mem::take(&mut set.found);
        found.clear();
        found.extend(self.usable_ctors(c));
        found.extend(self.ctor_shadows(c));
        self.add_found_candidates(set, &found, CandidateKind::Ctor, first_opts, list_only);
        set.found = found;
    }

    /// Adds the functions in `found` (constructors or member functions
    /// of one class, of kind `kind`) to `set`, once per entity; function
    /// templates are skipped (WP-071) and a *requires-clause* makes the
    /// set uncertain (WP-073). With `list_only`, only
    /// initializer-list constructors.
    pub(crate) fn add_found_candidates(
        &mut self,
        set: &mut super::OverloadCandidateSet,
        found: &[DeclId],
        kind: CandidateKind,
        first_opts: IcsOptions,
        list_only: bool,
    ) {
        let want = if kind == CandidateKind::Ctor {
            DeclKind::CXXConstructor
        } else {
            DeclKind::CXXMethod
        };
        let mut seen: Vec<DeclId> = Vec::with_capacity(found.len());
        for &fd in found {
            // An inherited constructor is its base constructor found
            // through its shadow ([namespace.udecl]/13,
            // [over.match.funcs.general]/8).
            let mut f = fd;
            if self.kind_of(fd) == DeclKind::ConstructorUsingShadow {
                if kind != CandidateKind::Ctor {
                    continue;
                }
                let Some(t) = self.ast.using_target(fd) else { continue };
                f = t;
            }
            let k = self.kind_of(f);
            let n = set.cands.len();
            let first_opts = self.direct_copy_ctor_opts(fd, f, first_opts);
            if k == DeclKind::FunctionTemplate {
                self.add_found_template(set, f, fd, want, kind, first_opts, list_only, &mut seen);
                if fd != f {
                    self.exclude_inhctor_slice(set, n, fd);
                }
                continue;
            }
            if k != want || self.ast.decls.get(f).has(DeclFlags::INVALID) {
                continue;
            }
            let first = self.ast.first(f);
            if seen.contains(&first) {
                continue;
            }
            seen.push(first);
            if list_only && !self.is_init_list_ctor(f) {
                continue;
            }
            if self.ast.decls.cold.requires_clause.contains(f) {
                set.uncertain = true;
            }
            self.add_candidate(set, f, fd, kind, first_opts);
            if fd != f {
                self.exclude_inhctor_slice(set, n, fd);
            }
        }
    }

    /// The first-parameter options of constructor `f`, found as `fd`,
    /// under `opts`: with
    /// [`IcsOptions::DIRECT_COPY_CTOR`], a first parameter of type
    /// (reference to) cv `C`, the class `fd` is a member of, also allows
    /// explicit conversion functions: the temporary bound to it in a
    /// direct-initialization of a `C` with one argument
    /// ([over.match.copy]/1.2, [over.match.ref]/1.1; clang
    /// `ResolveConstructorOverload` `AllowExplicitConv`). An inherited
    /// constructor's first parameter names its base, never `C`.
    fn direct_copy_ctor_opts(&self, fd: DeclId, f: DeclId, opts: IcsOptions) -> IcsOptions {
        let flags = IcsOptions::DIRECT_COPY_CTOR.with(IcsOptions::LIST_COPY_CTOR);
        if opts.0 & flags.0 == 0 {
            return opts;
        }
        let rest = IcsOptions(opts.0 & !flags.0);
        // clang: not for a constructor template (its FIXME), nor an
        // inherited constructor.
        let copy_or_move = self.kind_of(f) == DeclKind::CXXConstructor
            && self.ast.decls.get(fd).ctx().is_some_and(|c| self.copy_or_move_ctor_param(f, c));
        if !copy_or_move {
            rest
        } else if opts.contains(IcsOptions::LIST_COPY_CTOR) {
            rest.with(IcsOptions::SUPPRESS_UDC)
        } else {
            rest.with(IcsOptions::EXPLICIT_REF_CONV)
        }
    }

    /// clang `hasCopyOrMoveCtorParam`: constructor `f`'s first parameter
    /// is (a reference to) cv `class`.
    pub(crate) fn copy_or_move_ctor_param(&self, f: DeclId, class: DeclId) -> bool {
        let mut params = Vec::new();
        self.fn_param_types(f, &mut params);
        let Some(&p) = params.first() else { return false };
        let p = self.ast.types.canonical(self.non_reference(p));
        self.ast.types.as_record(p).is_some_and(|r| self.ast.first(r) == self.ast.first(class))
    }

    /// [over.match.funcs.general]/8 (clang `AddOverloadCandidate`): an
    /// inherited constructor of class C whose first parameter is a
    /// reference to P is not viable for a single argument when the
    /// constructed class D is reference-related to P and P to C (it
    /// would copy or move a base subobject).
    pub(crate) fn exclude_inhctor_slice(
        &mut self,
        set: &mut super::OverloadCandidateSet,
        n: usize,
        shadow: DeclId,
    ) {
        if set.args.len() != 1 {
            return;
        }
        let Some(cand) = set.cands.get(n) else { return };
        // clang checks the slice before the arity and the conversions, so
        // it overrides those failures (not a failed deduction or an
        // illegal constructor specialization, checked earlier).
        if !cand.viable
            && matches!(
                cand.failure,
                CandidateFailure::BadDeduction | CandidateFailure::IllegalCtor
            )
        {
            return;
        }
        let f = cand.func();
        let params = self.param_decls(f);
        let Some(&p0) = params.first() else { return };
        let pty = self.ast.decls.get(p0).ty().unwrap_or(QualType::ERROR);
        if !self.ast.types.is_reference(pty) {
            return;
        }
        let moving = self.ast.types.canonical_kind(pty) == kiln_ast::types::TypeKind::RValueRef;
        let Some(p) = self.ast.types.pointee(pty) else { return };
        let (Some(c), Some(d)) = (self.ast.decls.get(f).ctx(), self.ast.decls.get(shadow).ctx())
        else {
            return;
        };
        let (cty, dty) = (self.class_type(c), self.class_type(d));
        let related = |s: &Self, a: QualType, b: QualType| {
            s.ast.types.same_unqualified(a, b)
                || match (s.ast.types.as_record(a), s.ast.types.as_record(b)) {
                    (Some(x), Some(y)) => is_derived_from(s, x, y),
                    _ => false,
                }
        };
        if related(self, p, cty) && related(self, dty, p) {
            let c = &mut set.cands[n];
            c.viable = false;
            c.failure = CandidateFailure::InhCtorSlice { moving };
        }
    }

    /// Whether constructor `f` is an initializer-list constructor
    /// ([dcl.init.list]/2).
    /// Adds function template `ft` (a constructor or member function
    /// template of the kind `want`) to `set` through template argument
    /// deduction from the set's arguments ([over.match.funcs.general]/8,
    /// [temp.deduct.call]). A constructor specialization that would copy
    /// its own class by value is not viable ([class.copy.ctor]/5).
    #[allow(clippy::too_many_arguments, reason = "add_found_candidates' parts")]
    fn add_found_template(
        &mut self,
        set: &mut super::OverloadCandidateSet,
        ft: DeclId,
        found: DeclId,
        want: DeclKind,
        kind: CandidateKind,
        first_opts: IcsOptions,
        list_only: bool,
        seen: &mut Vec<DeclId>,
    ) {
        let Some(pattern) = self.templated_function(ft) else { return };
        if self.kind_of(pattern) != want || self.ast.decls.get(ft).has(DeclFlags::INVALID) {
            return;
        }
        let first = self.ast.first(ft);
        if seen.contains(&first) {
            return;
        }
        seen.push(first);
        if list_only && !self.is_init_list_ctor(pattern) {
            return;
        }
        if self.ast.decls.cold.requires_clause.contains(ft) {
            set.uncertain = true;
        }
        if set.two_phase {
            // clang 21+: a constructor template whose explicit-specifier
            // is value-dependent is deduced at once and forbids picking a
            // perfect match first (clang `AddTemplateOverloadCandidate`).
            if kind == CandidateKind::Ctor && self.dependently_explicit(pattern) {
                set.no_perfect = true;
            } else {
                set.deferred.push(super::defer::Deferred::Found { ft, found, kind, first_opts });
                return;
            }
        }
        self.add_found_template_now(set, ft, found, kind, first_opts);
    }

    /// The deduction part of [`Self::add_found_template`], which a
    /// two-phase set defers.
    pub(crate) fn add_found_template_now(
        &mut self,
        set: &mut super::OverloadCandidateSet,
        ft: DeclId,
        found: DeclId,
        kind: CandidateKind,
        first_opts: IcsOptions,
    ) {
        let n = set.cands.len();
        let loc = set.loc;
        self.add_template_candidate(set, ft, found, None, kind, first_opts, loc);
        if kind == CandidateKind::Ctor
            && set.args.len() == 1
            && let Some(c) = set.cands.get(n)
            && c.viable
            && self.specialization_copies_object(c.func(), &set.args[0])
        {
            let c = &mut set.cands[n];
            c.viable = false;
            c.failure = CandidateFailure::IllegalCtor;
        }
    }

    /// clang `isSpecializationCopyingObject` with the argument check of
    /// `AddOverloadCandidate`: constructor `f`'s first parameter is its
    /// class by value, the others defaulted, and `a` is of that class
    /// (or derived from it).
    fn specialization_copies_object(&mut self, f: DeclId, a: &ArgInfo) -> bool {
        let params = self.param_decls(f);
        let Some(&p) = params.first() else { return false };
        if params[1..].iter().any(|&q| !self.param_has_default(q)) {
            return false;
        }
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        if self.ast.types.is_reference(pty) {
            return false;
        }
        let Some(class) = self.ast.decls.get(f).ctx() else { return false };
        let cty = self.class_type(class);
        if !self.ast.types.same_unqualified(pty, cty) || a.list.is_some() {
            return false;
        }
        if self.ast.types.same_unqualified(a.ty, cty) {
            return true;
        }
        self.ast.types.as_record(a.ty).is_some_and(|d| is_derived_from(self, d, class))
    }

    pub(crate) fn is_init_list_ctor(&mut self, f: DeclId) -> bool {
        let params = self.param_decls(f);
        let Some(&p) = params.first() else { return false };
        if params[1..].iter().any(|&q| !self.param_has_default(q)) {
            return false;
        }
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let t = self.non_reference(pty);
        // A constructor template's `std::initializer_list<U>` is
        // dependent ([dcl.init.list]/2 names the template, WP-169).
        self.initializer_list_param(t).is_some()
    }

    /// A user-defined conversion of `a` to `to` ([over.match.copy],
    /// [over.match.conv]): `kind` is `CopyInit` for a class target,
    /// `ConvToNonClass` otherwise.
    fn user_conversion(
        &mut self,
        a: &ArgInfo,
        to: QualType,
        kind: SetKind,
        opts: IcsOptions,
    ) -> Ics {
        let mut set = self.begin_set(kind, Loc::INVALID);
        set.target = to;
        self.build_conversion_set(&mut set, a, to, opts);
        let r = self.udc_outcome(&mut set, a.ty, to, false);
        self.end_set(set);
        r
    }

    /// Fills `set` (kind `CopyInit` or `ConvToNonClass`, target `to`)
    /// with the constructors of a class `to` and the conversion functions
    /// of a class `a`.
    pub(crate) fn build_conversion_set(
        &mut self,
        set: &mut super::OverloadCandidateSet,
        a: &ArgInfo,
        to: QualType,
        opts: IcsOptions,
    ) {
        set.args.push(*a);
        set.target = to;
        if let Some(t) = self.ast.types.as_record(to)
            && self.require_complete_silent(to, Loc::INVALID)
        {
            let first = opts.with(IcsOptions::SUPPRESS_UDC);
            let n = set.cands.len();
            // [over.match.copy]/1.1: converting constructors only (clang
            // `isConvertingConstructor`; an explicit one is noted).
            // Inherited constructors convert too ([namespace.udecl]/13).
            let mut found = std::mem::take(&mut set.found);
            found.clear();
            found.extend(self.converting_ctors(t, a.list.is_none()));
            self.add_found_candidates(set, &found, CandidateKind::Ctor, first, false);
            set.found = found;
            for c in &mut set.cands[n..] {
                if self.fn_spec(c.func()).is_explicit()
                    && !opts.contains(IcsOptions::ALLOW_EXPLICIT)
                {
                    c.viable = false;
                    c.failure = CandidateFailure::Explicit;
                }
            }
        }
        if self.ast.types.as_record(a.ty).is_some()
            && self.require_complete_silent(a.ty, Loc::INVALID)
        {
            let fns = self.conversion_functions_all(a.ty);
            for (found, f) in fns {
                self.add_conversion_candidate(set, f, found, to, opts);
            }
        }
        self.order_conversion_templates(set);
    }

    /// Records in `set.tmpl_order` which viable conversion function
    /// template specializations come from a more specialized template
    /// ([over.match.best.general]/2.5, [temp.deduct.partial]/3.2).
    pub(crate) fn order_conversion_templates(&mut self, set: &mut super::OverloadCandidateSet) {
        let ts: Vec<(u32, DeclId)> = set
            .cands
            .iter()
            .enumerate()
            .filter(|(_, c)| c.viable && c.kind == CandidateKind::ConvFn)
            .filter_map(|(i, c)| Some((u32::try_from(i).ok()?, c.template?)))
            .collect();
        for (i, &(c1, t1)) in ts.iter().enumerate() {
            for &(c2, t2) in &ts[i + 1..] {
                if self.more_specialized_conversion(t1, t2) {
                    set.tmpl_order.push((c1, c2));
                } else if self.more_specialized_conversion(t2, t1) {
                    set.tmpl_order.push((c2, c1));
                }
            }
        }
    }

    /// The outcome of a user-defined conversion set as an ICS.
    fn udc_outcome(
        &mut self,
        set: &mut super::OverloadCandidateSet,
        from: QualType,
        to: QualType,
        list_ctor: bool,
    ) -> Ics {
        self.order_template_candidates(set);
        let c = match self.best_viable(set) {
            OvlOutcome::Success(c) | OvlOutcome::Deleted(c) => c,
            OvlOutcome::Ambiguous => return Ics::Ambiguous { from, to },
            OvlOutcome::NoViable => {
                let k = if list_ctor || set.args.iter().any(|a| a.list.is_some()) {
                    BadKind::InitList
                } else {
                    BadKind::NoConversion
                };
                return Ics::bad(k, from, to);
            }
        };
        let cand = set.cand(c);
        let f = cand.func();
        let first = set.ics_of(c).first().copied();
        let before = match first {
            Some(Ics::Standard(s)) => s,
            _ => StandardConversion::identity(from, from, RefBinding::NONE),
        };
        if cand.kind == CandidateKind::ConvFn {
            let after = cand
                .final_conv
                .unwrap_or_else(|| StandardConversion::identity(to, to, RefBinding::NONE));
            return Ics::UserDefined(UserConv {
                before,
                func: Some(f),
                found: Some(cand.found),
                after,
                via: UdcVia::ConvFn,
            });
        }
        let after = StandardConversion::identity(to, to, RefBinding::NONE);
        Ics::UserDefined(UserConv {
            before,
            func: Some(f),
            found: Some(cand.found),
            after,
            via: UdcVia::Ctor,
        })
    }

    /// The conversion functions of class type `ty` and its bases
    /// ([class.conv.fct]/9: hidden by one converting to the same type),
    /// as `(found, function)`; conversion function templates are
    /// skipped (the built-in candidate types and surrogate calls do not
    /// consider them).
    pub(crate) fn conversion_functions(&self, ty: QualType) -> Vec<(DeclId, DeclId)> {
        let mut v = self.conversion_functions_all(ty);
        v.retain(|&(_, f)| self.kind_of(f) == DeclKind::CXXConversion);
        v
    }

    /// [`Self::conversion_functions`] with the conversion function
    /// templates ([temp.deduct.conv]), which [`Self::add_conversion_candidate`]
    /// deduces.
    ///
    /// The conversion functions visible in class `ty` (clang
    /// `CXXRecordDecl::getVisibleConversionFunctions`): its own, then
    /// each base's in declaration order, depth first. A base's conversion
    /// function is hidden when a class derived from that base along the
    /// same path declares one converting to the same type
    /// ([class.member.lookup], [class.conv.fct]/9), so two sibling bases'
    /// `operator int` are both visible (an ambiguous conversion) and the
    /// one of a virtual base is hidden when it is hidden along any path
    /// (`struct D : B, C` with `B, C : virtual A` and `B` declaring its
    /// own). A conversion function a using-declaration names in a class
    /// is that class's ([namespace.udecl]/1), found as its shadow
    /// ([namespace.udecl]/19 access). Each function once.
    pub(crate) fn conversion_functions_all(&self, ty: QualType) -> Vec<(DeclId, DeclId)> {
        let mut out: Vec<(DeclId, DeclId)> = Vec::new();
        let Some(r) = self.ast.types.as_record(ty) else { return out };
        // Virtual bases' conversion functions, added last unless hidden
        // along some path.
        let mut vout: Vec<(DeclId, DeclId)> = Vec::new();
        let mut hidden_v: Vec<DeclId> = Vec::new();
        // (class, reached through a virtual base, the types its derived
        // classes on this path convert to); the bases are pushed in
        // reverse so they are walked in declaration order.
        type Hidden = std::rc::Rc<Vec<QualType>>;
        let mut work: Vec<(DeclId, bool, Hidden)> =
            vec![(self.ast.first(r), false, Hidden::default())];
        let mut seen: kiln_source::fx::FxHashSet<(DeclId, bool, Hidden)> =
            kiln_source::fx::FxHashSet::default();
        let top = self.ast.first(r);
        while let Some((c, in_virtual, parent_hidden)) = work.pop() {
            // A class reached again with the same state adds nothing.
            if !seen.insert((c, in_virtual, parent_hidden.clone())) {
                continue;
            }
            let Some(def) = self.ast.definition(c) else { continue };
            let mut hidden = parent_hidden.clone();
            for (found, m, key) in self.own_conversion_functions(def) {
                if c == top {
                    std::rc::Rc::make_mut(&mut hidden).push(key);
                    out.push((found, m));
                    continue;
                }
                let is_hidden = parent_hidden.contains(&key);
                if !is_hidden && !hidden.contains(&key) {
                    std::rc::Rc::make_mut(&mut hidden).push(key);
                }
                if is_hidden {
                    if in_virtual {
                        hidden_v.push(self.ast.first(m));
                    }
                } else if in_virtual {
                    vout.push((found, m));
                } else {
                    out.push((found, m));
                }
            }
            let bases: Vec<_> = self.ast.decls.bases(def, &self.ast.lists).collect();
            for b in bases.into_iter().rev() {
                if let Some(bc) = self.ast.types.as_record(b.ty) {
                    work.push((self.ast.first(bc), in_virtual || b.is_virtual, hidden.clone()));
                }
            }
        }
        for (found, m) in vout {
            if !hidden_v.contains(&self.ast.first(m)) {
                out.push((found, m));
            }
        }
        // A function reached along several paths is one candidate (clang
        // `OverloadCandidateSet::isNewCandidate`).
        let mut unique: Vec<(DeclId, DeclId)> = Vec::with_capacity(out.len());
        for (found, m) in out {
            if !unique.iter().any(|&(_, f)| self.ast.first(f) == self.ast.first(m)) {
                unique.push((found, m));
            }
        }
        unique
    }

    /// The conversion functions (and templates) class definition `def`
    /// declares, those using-declarations name included, as `(found,
    /// function or template, canonical converted-to type)`.
    fn own_conversion_functions(&self, def: DeclId) -> Vec<(DeclId, DeclId, QualType)> {
        let mut v = Vec::new();
        for found in self.ast.decls.members(def) {
            // A using-declaration names a base's conversion function in
            // this class ([namespace.udecl]/1): found as its shadow, whose
            // access is the using-declaration's ([namespace.udecl]/19).
            let m = self.ast.strip_shadow(found);
            let f = if self.kind_of(m) == DeclKind::FunctionTemplate {
                match self.templated_function(m) {
                    Some(f) if self.kind_of(f) == DeclKind::CXXConversion => f,
                    _ => continue,
                }
            } else {
                m
            };
            if self.kind_of(f) != DeclKind::CXXConversion
                || self.ast.decls.get(m).has(DeclFlags::INVALID)
            {
                continue;
            }
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            let ret =
                self.ast.types.proto(fty).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
            v.push((found, m, self.ast.types.canonical(ret)));
        }
        v
    }

    /// The implicit object argument's conversion for conversion function
    /// `f`: an explicit object parameter is copy-initialized from the
    /// object without user-defined conversions ([over.best.ics.general]/4,
    /// C++23; clang `TryObjectArgumentInitialization`), else the implicit
    /// object parameter's binding.
    fn conv_object_ics(&mut self, a: &ArgInfo, f: DeclId) -> Ics {
        match self.explicit_object_param_type(f) {
            Some(p) => self.try_implicit_conversion(a, p, IcsOptions::SUPPRESS_UDC),
            None => self.object_ics(a, f),
        }
    }

    /// [class.conv.fct]/1, [over.ics.user]/4: a conversion function never
    /// converts an object of class type `from` to its own class or to a
    /// base of it (that is a copy, or a derived-to-base conversion of
    /// Conversion rank, through a constructor). Such a candidate is not
    /// viable whatever its result type (clang `AddConversionCandidate`,
    /// `ovl_fail_trivial_conversion`), so it never competes with the copy
    /// or move constructor and is never called.
    fn is_trivial_class_conversion(&mut self, from: QualType, to: QualType) -> bool {
        let (Some(fr), Some(tr)) = (self.ast.types.as_record(from), self.ast.types.as_record(to))
        else {
            return false;
        };
        same_class(self, fr, tr)
            || complete_class(self, fr).is_some() && is_derived_from(self, fr, tr)
    }

    /// Adds conversion function `f` (found as `found`) for converting the
    /// set's only argument to `to` ([over.match.copy], [over.match.conv],
    /// [over.match.ref]).
    pub(crate) fn add_conversion_candidate(
        &mut self,
        set: &mut super::OverloadCandidateSet,
        f: DeclId,
        found: DeclId,
        to: QualType,
        opts: IcsOptions,
    ) {
        if self.kind_of(f) == DeclKind::FunctionTemplate {
            self.add_conversion_template_candidate(set, f, found, to, opts);
            return;
        }
        self.ovl.stats.candidates += 1;
        let a = set.args[0];
        let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
        let mut cand = super::Candidate::new(f, found, CandidateKind::ConvFn);
        let obj = self.conv_object_ics(&a, f);
        set.ics.push(obj);
        cand.ics = start..start + 1;
        if obj.is_bad() {
            return set.push_non_viable(cand, CandidateFailure::BadConversion { arg: u16::MAX });
        }
        if let Some(sat) = self.check_function_constraints(f, set.loc).filter(|&s| !self.sat_ok(s))
        {
            // [over.match.viable]/3: a conversion function whose trailing
            // requires-clause is not satisfied is not viable, after the
            // object argument (clang `AddConversionCandidate`).
            return set.push_non_viable(cand, CandidateFailure::ConstraintsNotSatisfied { sat });
        }
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let ret =
            self.ast.types.proto(fty).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
        if self.fn_spec(f).is_explicit() {
            // [over.match.conv]/1.2, [over.match.copy]/1.2: an explicit
            // conversion function is a candidate (of a direct-
            // initialization) only when it yields the target type up to
            // a qualification conversion (clang
            // `isAllowableExplicitConversion`); in a copy-initialization
            // it is then "not a candidate". [over.match.ref]/1.1 has the
            // same rule for the explicit functions a direct-
            // initialization admits.
            if (!matches!(set.kind, SetKind::ConvToRef { .. })
                || opts.contains(IcsOptions::ALLOW_EXPLICIT))
                && !self.allowable_explicit_conversion(ret, to)
            {
                self.ovl.stats.candidates -= 1;
                set.ics.truncate(start as usize);
                return;
            }
            if !opts.contains(IcsOptions::ALLOW_EXPLICIT) {
                return set.push_non_viable(cand, CandidateFailure::Explicit);
            }
        }
        if self.is_trivial_class_conversion(a.ty, to) {
            return set.push_non_viable(cand, CandidateFailure::TrivialConversion);
        }
        // The call's result: a glvalue of the referenced type or a
        // prvalue.
        let (rty, rvk) = match self.ast.types.canonical_view(ret) {
            TypeView::LValueRef(p) => (p, ValueCategory::LValue),
            TypeView::RValueRef(p) => (p, ValueCategory::XValue),
            _ => (conv::strip_cv(self, ret), ValueCategory::PRValue),
        };
        let final_conv = match set.kind {
            SetKind::ConvToRef { lvalue } => {
                let ok = if lvalue {
                    rvk == ValueCategory::LValue
                } else {
                    rvk != ValueCategory::LValue || self.ast.types.is_function(rty)
                };
                if ok {
                    let r = ArgInfo::of_type(rty, rvk);
                    match self.ref_ics(&r, to, opts.with(IcsOptions::SUPPRESS_UDC)) {
                        Ics::Standard(s) if s.rb.contains(RefBinding::DIRECT) => Some(s),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ if self.ast.types.as_record(to).is_some() => {
                // [over.match.copy]/1.2: yielding cv T or a derived class.
                let (Some(tr), Some(rr)) =
                    (self.ast.types.as_record(to), self.ast.types.as_record(rty))
                else {
                    return set.push_non_viable(cand, CandidateFailure::BadFinalConversion);
                };
                if same_class(self, rr, tr) {
                    Some(StandardConversion::identity(rty, to, RefBinding::NONE))
                } else if complete_class(self, rr).is_some() && is_derived_from(self, rr, tr) {
                    Some(derived_to_base(rty, to, RefBinding::NONE))
                } else {
                    None
                }
            }
            _ => {
                // A `std::nullptr_t` prvalue result is a null pointer
                // constant ([conv.ptr]/1).
                let npc = rvk == ValueCategory::PRValue
                    && self.ast.types.canonical(rty).ty() == kiln_ast::TypeId::NULLPTR;
                let r = ArgInfo { npc, ..ArgInfo::of_type(rty, rvk) };
                match self.try_implicit_conversion(&r, to, IcsOptions::SUPPRESS_UDC) {
                    Ics::Standard(s) => Some(s),
                    _ => None,
                }
            }
        };
        if let Some(s) = final_conv {
            cand.final_conv = Some(s);
        } else {
            cand.viable = false;
            cand.failure = CandidateFailure::BadFinalConversion;
        }
        set.cands.push(cand);
    }

    /// Adds conversion function template `ft`: the specialization
    /// deduced for `to` ([temp.deduct.conv]), else a non-viable
    /// candidate keeping the deduction failure for its note. A template
    /// whose pattern is explicit is not a candidate where explicit
    /// conversions are not allowed (clang `isNonDependentlyExplicit`).
    pub(crate) fn add_conversion_template_candidate(
        &mut self,
        set: &mut super::OverloadCandidateSet,
        ft: DeclId,
        found: DeclId,
        to: QualType,
        opts: IcsOptions,
    ) {
        let Some(pattern) = self.templated_function(ft) else { return };
        if self.nondependently_explicit(pattern) && !opts.contains(IcsOptions::ALLOW_EXPLICIT) {
            // clang `AddTemplateConversionCandidate`: noted as "not a
            // candidate" (a reference binding skips it).
            if !matches!(set.kind, SetKind::ConvToRef { .. }) {
                self.ovl.stats.candidates += 1;
                let mut c = super::Candidate::new(pattern, found, CandidateKind::ConvFn);
                let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
                c.ics = start..start;
                c.template = Some(ft);
                c.viable = false;
                c.failure = CandidateFailure::Explicit;
                set.cands.push(c);
            }
            return;
        }
        let loc = set.loc;
        let obj = self.has_explicit_object(pattern).then_some(set.args[0]);
        match self.deduce_conversion(ft, to, obj, loc) {
            Ok(spec) => {
                let n = set.cands.len();
                self.add_conversion_candidate(set, spec, found, to, opts);
                if let Some(c) = set.cands.get_mut(n) {
                    c.template = Some(ft);
                }
            }
            Err(fail) => {
                self.ovl.stats.candidates += 1;
                let mut c = super::Candidate::new(pattern, found, CandidateKind::ConvFn);
                let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
                c.ics = start..start;
                c.template = Some(ft);
                c.viable = false;
                c.failure = CandidateFailure::BadDeduction;
                c.deduction = Some(fail);
                set.cands.push(c);
            }
        }
    }

    /// Whether an explicit conversion function returning `ret` may
    /// convert to `to`: the same type (references and cv dropped), or a
    /// qualification conversion of pointers ([over.match.conv]/1.2).
    fn allowable_explicit_conversion(&mut self, ret: QualType, to: QualType) -> bool {
        let r = conv::strip_cv(self, self.non_reference(ret));
        let t = conv::strip_cv(self, self.non_reference(to));
        let (cr, ct) = (self.ast.types.canonical(r), self.ast.types.canonical(t));
        if self.ast.types.same_type(cr, ct) {
            return true;
        }
        let a = ArgInfo::of_type(r, ValueCategory::PRValue);
        match standard_conversion_from(self, a.ty, a.vk, false, None, t, ConvCtx::Initialization) {
            Some(s) => s.second == SecondStep::Identity && s.first == conv::FirstStep::Identity,
            None => false,
        }
    }

    /// [over.match.ref]: binding reference `to` to the result of a
    /// conversion function of `a`'s class yielding an lvalue (`lvalue`)
    /// or an rvalue. `None` when no candidate is viable.
    fn ref_conversion(
        &mut self,
        a: &ArgInfo,
        to: QualType,
        lvalue: bool,
        opts: IcsOptions,
    ) -> Option<Ics> {
        if !self.require_complete_silent(a.ty, Loc::INVALID) {
            return None;
        }
        let fns = self.conversion_functions_all(a.ty);
        if fns.is_empty() {
            return None;
        }
        let mut set = self.begin_set(SetKind::ConvToRef { lvalue }, Loc::INVALID);
        set.target = to;
        set.args.push(*a);
        for (found, f) in fns {
            self.add_conversion_candidate(&mut set, f, found, to, opts);
        }
        self.order_conversion_templates(&mut set);
        let r = if set.viable().next().is_some() {
            if opts.contains(IcsOptions::CANDIDATE_ARG)
                && matches!(self.best_viable(&mut set), OvlOutcome::Deleted(_))
            {
                None
            } else {
                Some(self.udc_outcome(&mut set, a.ty, to, false))
            }
        } else {
            None
        };
        self.end_set(set);
        r
    }

    /// Compares two implicit conversion sequences ([over.ics.rank]).
    pub(crate) fn compare_ics(&self, a: &Ics, b: &Ics) -> IcsCompare {
        let (ca, cb) = (a.class(), b.class());
        if ca < cb {
            return IcsCompare::Better;
        }
        if ca > cb {
            return IcsCompare::Worse;
        }
        // /3.1.1: a list-initialization sequence converting to
        // std::initializer_list is better than one that does not, even
        // if another rule of /3 would apply (CWG1467).
        let to_list = |i: &Ics| match i {
            Ics::Standard(s) => s.std_init_list,
            Ics::UserDefined(u) => u.after.std_init_list,
            _ => false,
        };
        if to_list(a) != to_list(b) {
            return if to_list(a) { IcsCompare::Better } else { IcsCompare::Worse };
        }
        // /3.1.2: of two list-initialization sequences converting to
        // arrays of the same element type, the one initializing fewer
        // elements is better, and with as many, the one not converting to
        // an array of unknown bound (P0388; clang's
        // `InitializerListContainerType`).
        let to_array = |i: &Ics| match i {
            Ics::Standard(s) => s.list_array,
            Ics::UserDefined(u) => u.after.list_array,
            _ => None,
        };
        if let (Some(x), Some(y)) = (to_array(a), to_array(b)) {
            let t = &self.ast.types;
            if t.canonical(x.elem).without_quals() == t.canonical(y.elem).without_quals() {
                if x.len != y.len {
                    return if x.len < y.len { IcsCompare::Better } else { IcsCompare::Worse };
                }
                if x.unbounded != y.unbounded {
                    return if y.unbounded { IcsCompare::Better } else { IcsCompare::Worse };
                }
            }
        }
        match (a, b) {
            (Ics::Standard(x), Ics::Standard(y)) => compare_standard_conversions(self, x, y),
            (Ics::UserDefined(x), Ics::UserDefined(y)) => {
                // A class initialized through its initializer-list
                // constructor does not convert to std::initializer_list
                // (/3.1.1 names the parameter type): two such sequences
                // through different constructors are indistinguishable.
                // /3.3: the same function (or aggregate initialization of
                // the same class): compare the second standard
                // conversions.
                let same = match (x.func, y.func) {
                    (Some(f), Some(g)) => self.ast.first(f) == self.ast.first(g),
                    (None, None) => x.via == UdcVia::Aggregate && y.via == UdcVia::Aggregate,
                    _ => false,
                };
                if same {
                    compare_standard_conversions(self, &x.after, &y.after)
                } else {
                    IcsCompare::Indistinguishable
                }
            }
            _ => IcsCompare::Indistinguishable,
        }
    }
}
