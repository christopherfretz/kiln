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
    self, ConvCtx, IcsCompare, RefBinding, SecondStep, StandardConversion,
    compare_standard_conversions, standard_conversion_from,
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
    /// The constructor is an initializer-list constructor called with a
    /// braced-init-list ([over.ics.rank]/3.1).
    pub list_ctor: bool,
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

    fn ics_is_error(&self, q: QualType) -> bool {
        let t = &self.ast.types;
        t.canonical(q).without_quals() == QualType::ERROR || t.is_dependent(q)
    }

    /// The implicit conversion sequence converting `a` to `to`
    /// ([over.best.ics]).
    pub(crate) fn try_implicit_conversion(
        &mut self,
        a: &ArgInfo,
        to: QualType,
        opts: IcsOptions,
    ) -> Ics {
        self.ovl.stats.ics += 1;
        if let Some(i) = a.list {
            return self.list_ics_at(i, to, opts);
        }
        if self.ics_is_error(a.ty) || self.ics_is_error(to) {
            return Ics::Standard(StandardConversion::identity(a.ty, to, RefBinding::NONE));
        }
        if a.ty == QualType::OVERLOAD {
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

    /// The conversion sequence of an overload-set argument ([over.ics.scs]
    /// with [over.over]): that of the function the target selects (an
    /// lvalue of function type, or its address for `&f`), else none.
    fn overload_set_ics(&mut self, a: &ArgInfo, to: QualType, opts: IcsOptions) -> Ics {
        let Some(e) = a.expr else { return Ics::bad(BadKind::NoConversion, a.ty, to) };
        let Some(d) = self.address_match(e, to) else {
            return Ics::bad(BadKind::NoConversion, a.ty, to);
        };
        let ft = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let addr = self.overload_operand(e).is_some_and(|(_, addr)| addr);
        let class = self.is_instance_member(d).then(|| self.declaring_class(d)).flatten();
        let f = if let Some(c) = class {
            let ct = self.tag_type(c).ty();
            ArgInfo::of_type(self.ast.types.member_pointer(ft, ct), ValueCategory::PRValue)
        } else if addr {
            ArgInfo::of_type(self.ast.types.pointer_to(ft), ValueCategory::PRValue)
        } else {
            ArgInfo::of_type(ft, ValueCategory::LValue)
        };
        self.try_implicit_conversion(&f, to, opts)
    }

    /// Reference-relatedness of `t1` (the referee) and `t2`, both
    /// canonical and unqualified ([dcl.init.ref]/4): `(related,
    /// derived)`.
    fn ref_related(&self, t1: QualType, t2: QualType) -> (bool, bool) {
        if t1 == t2 {
            return (true, false);
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

    /// The conversion sequence binding reference type `to` to `a`
    /// ([over.ics.ref], clang `TryReferenceInit`).
    #[allow(clippy::too_many_lines, reason = "one step per bullet of [dcl.init.ref]/5")]
    fn ref_ics(&mut self, a: &ArgInfo, to: QualType, opts: IcsOptions) -> Ics {
        let (lref, t1) = match self.ast.types.canonical_view(to) {
            TypeView::LValueRef(p) => (true, p),
            TypeView::RValueRef(p) => (false, p),
            _ => return Ics::bad(BadKind::NoConversion, a.ty, to),
        };
        let types = &self.ast.types;
        let (q1, q2) = (types.effective_quals(t1), types.effective_quals(a.ty));
        let u1 = types.canonical(t1).without_quals();
        let u2 = types.canonical(a.ty).without_quals();
        let is_fn = types.is_function(u1);
        let class_src = types.as_record(u2).is_some();
        let class_or_array = class_src || types.is_array(u2);
        let (related, derived) = self.ref_related(u1, u2);
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
        // /5.1.2: an lvalue result of a conversion function.
        if lref && !related && class_src && !suppress {
            if let Some(x) = self.ref_conversion(a, to, true, opts) {
                return x;
            }
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
        if !related && class_src && !suppress {
            if let Some(x) = self.ref_conversion(a, to, false, opts) {
                return x;
            }
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
        match self.try_implicit_conversion(a, target, opts) {
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
        self.ovl.stats.ics += 1;
        if self.ics_is_error(o.ty) {
            return super::ignored_object(o.ty);
        }
        let Some(cls) = self.declaring_class(m) else { return super::ignored_object(o.ty) };
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
                if !lv && !(q.is_const() && !q.is_volatile()) {
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

    /// The ICS of braced-init-list argument `i` (in `self.ovl.lists`).
    fn list_ics_at(&mut self, i: u32, to: QualType, opts: IcsOptions) -> Ics {
        let Some(slot) = self.ovl.lists.get_mut(i as usize) else {
            return Ics::bad(BadKind::InitList, QualType::ERROR, to);
        };
        let empty = InitList { items: Vec::new(), lbrace: slot.lbrace, rbrace: slot.rbrace };
        let l = std::mem::replace(slot, empty);
        let r = self.list_ics(&l, to, opts);
        self.ovl.lists[i as usize] = l;
        r
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
            let inner = conv::strip_cv(self, r);
            let lref = matches!(self.ast.types.canonical_view(to), TypeView::LValueRef(_));
            let q = self.ast.types.effective_quals(r);
            if lref && (!q.is_const() || q.is_volatile()) {
                // A non-const lvalue reference binds no temporary (an
                // element of the referenced type is not handled here).
                if let [InitItem { value: InitValue::Expr(e), designator }] = l.items.as_slice()
                    && designator.is_empty()
                {
                    let a = self.arg_info(*e);
                    return self.try_implicit_conversion(&a, to, opts);
                }
                return Ics::bad(BadKind::InitList, void_from, to);
            }
            let rb = if lref {
                RefBinding::IS_REF.with(RefBinding::LVALUE_REF).with(RefBinding::BINDS_RVALUE)
            } else {
                RefBinding::IS_REF.with(RefBinding::BINDS_RVALUE)
            };
            return match self.list_ics(l, inner, opts) {
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
                other => other,
            };
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
            return self.worst_element(l, elem, to, opts);
        }
        if let Some(c) = self.ast.types.as_record(to) {
            // /5: std::initializer_list<X>.
            if let Some(elem) = self.initializer_list_element(to) {
                return self.worst_element(l, elem, to, opts);
            }
            if !self.require_complete_silent(to, Loc::INVALID) {
                return Ics::bad(BadKind::InitList, void_from, to);
            }
            // /7: a non-aggregate class: constructors ([over.match.list]).
            if !self.is_aggregate(to) {
                return self.list_ctor_conversion(l, c, to, opts);
            }
            // /8: an aggregate.
            if l.items.len() as u64 <= self.aggregate_capacity(to) {
                let id = StandardConversion::identity(to, to, RefBinding::NONE);
                return Ics::UserDefined(UserConv {
                    before: id,
                    func: None,
                    found: None,
                    after: id,
                    via: UdcVia::Aggregate,
                    list_ctor: false,
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

    /// The most elements a braced-init-list for aggregate `to` can have
    /// (brace elision counts scalar leaves).
    fn aggregate_capacity(&self, to: QualType) -> u64 {
        self.scalar_leaves(to).unwrap_or(u64::MAX)
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
        // [over.best.ics]/4.5: one element converting to the class.
        let suppress = if l.items.len() == 1 { IcsOptions::SUPPRESS_UDC } else { IcsOptions::NONE };
        self.add_ctor_candidates(&mut set, c, suppress, false);
        for _ in 0..pushed {
            self.ovl.lists.pop();
        }
        let _ = opts;
        let r = self.udc_outcome(&mut set, QualType::VOID, to, false);
        self.end_set(set);
        r
    }

    /// Whether `c` has a default constructor (a cheap check for the
    /// `{}` rule of [dcl.init.list]/3.5).
    fn has_default_ctor_cheap(&mut self, c: DeclId) -> bool {
        let c = self.class_def(c);
        self.ensure_implicit(c, crate::class::SpecialKind::DefaultCtor);
        let ctors: Vec<DeclId> = self.ast.constructors(c).iter().collect();
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
        found.extend(self.ast.constructors(c).iter());
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
        for &f in found {
            let k = self.kind_of(f);
            if k == DeclKind::FunctionTemplate {
                set.skipped_templates = true;
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
            self.add_candidate(set, f, f, kind, first_opts);
        }
    }

    /// Whether constructor `f` is an initializer-list constructor
    /// ([dcl.init.list]/2).
    pub(crate) fn is_init_list_ctor(&mut self, f: DeclId) -> bool {
        let params = self.param_decls(f);
        let Some(&p) = params.first() else { return false };
        if params[1..].iter().any(|&q| !self.param_has_default(q)) {
            return false;
        }
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let t = self.non_reference(pty);
        self.initializer_list_element(t).is_some()
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
            let c = self.class_def(t);
            self.ensure_implicit(c, crate::class::SpecialKind::DefaultCtor);
            self.ensure_implicit(c, crate::class::SpecialKind::CopyCtor);
            self.ensure_implicit(c, crate::class::SpecialKind::MoveCtor);
            let mut found = std::mem::take(&mut set.found);
            found.clear();
            found.extend(self.ast.constructors(c).iter());
            if a.list.is_none() {
                let mut params = Vec::new();
                found.retain(|&f| {
                    if self.kind_of(f) != DeclKind::CXXConstructor {
                        return true;
                    }
                    params.clear();
                    let variadic = self.fn_param_types(f, &mut params);
                    if params.is_empty() { variadic } else { self.fn_min_args(f) <= 1 }
                });
            }
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
            let fns = self.conversion_functions(a.ty);
            for (found, f) in fns {
                self.add_conversion_candidate(set, f, found, to, opts);
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
                list_ctor: false,
            });
        }
        let after = StandardConversion::identity(to, to, RefBinding::NONE);
        Ics::UserDefined(UserConv {
            before,
            func: Some(f),
            found: Some(cand.found),
            after,
            via: UdcVia::Ctor,
            list_ctor,
        })
    }

    /// The conversion functions of class type `ty` and its bases
    /// ([class.conv.fct]/9: hidden by one converting to the same type),
    /// as `(found, function)`; templates are skipped (WP-071).
    pub(crate) fn conversion_functions(&self, ty: QualType) -> Vec<(DeclId, DeclId)> {
        let mut out: Vec<(DeclId, DeclId)> = Vec::new();
        let mut seen_types: Vec<QualType> = Vec::new();
        let Some(r) = self.ast.types.as_record(ty) else { return out };
        let mut work = vec![self.ast.first(r)];
        let mut visited: Vec<DeclId> = Vec::new();
        while let Some(c) = work.pop() {
            if visited.contains(&c) {
                continue;
            }
            visited.push(c);
            let Some(def) = self.ast.definition(c) else { continue };
            let mut here: Vec<QualType> = Vec::new();
            for m in self.ast.decls.members(def) {
                if self.kind_of(m) != DeclKind::CXXConversion
                    || self.ast.decls.get(m).has(DeclFlags::INVALID)
                {
                    continue;
                }
                let fty = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
                let ret = self.ast.types.proto(fty).map_or(QualType::ERROR, |p| p.ret());
                let key = self.ast.types.canonical(ret);
                if seen_types.contains(&key) {
                    continue;
                }
                if out.iter().any(|&(_, f)| self.ast.first(f) == self.ast.first(m)) {
                    continue;
                }
                here.push(key);
                out.push((m, m));
            }
            seen_types.extend(here);
            for b in self.ast.decls.bases(def, &self.ast.lists) {
                if let Some(bc) = self.ast.types.as_record(b.ty) {
                    work.push(self.ast.first(bc));
                }
            }
        }
        out
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
        self.ovl.stats.candidates += 1;
        let a = set.args[0];
        let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
        let mut cand = super::Candidate::new(f, found, CandidateKind::ConvFn);
        let obj = self.object_ics(&a, f);
        set.ics.push(obj);
        cand.ics = start..start + 1;
        if obj.is_bad() {
            cand.viable = false;
            cand.failure = CandidateFailure::BadConversion { arg: u16::MAX };
            set.cands.push(cand);
            return;
        }
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let ret = self.ast.types.proto(fty).map_or(QualType::ERROR, |p| p.ret());
        if self.fn_spec(f).is_explicit() {
            // [over.match.conv]/1.2, [over.match.copy]/1.2: an explicit
            // conversion function is a candidate (of a direct-
            // initialization) only when it yields the target type up to
            // a qualification conversion (clang
            // `isAllowableExplicitConversion`); in a copy-initialization
            // it is then "not a candidate".
            if !matches!(set.kind, SetKind::ConvToRef { .. })
                && !self.allowable_explicit_conversion(ret, to)
            {
                self.ovl.stats.candidates -= 1;
                set.ics.truncate(start as usize);
                return;
            }
            if !opts.contains(IcsOptions::ALLOW_EXPLICIT) {
                cand.viable = false;
                cand.failure = CandidateFailure::Explicit;
                set.cands.push(cand);
                return;
            }
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
                    cand.viable = false;
                    cand.failure = CandidateFailure::BadFinalConversion;
                    set.cands.push(cand);
                    return;
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
                let r = ArgInfo::of_type(rty, rvk);
                match self.try_implicit_conversion(&r, to, IcsOptions::SUPPRESS_UDC) {
                    Ics::Standard(s) => Some(s),
                    _ => None,
                }
            }
        };
        match final_conv {
            Some(s) => cand.final_conv = Some(s),
            None => {
                cand.viable = false;
                cand.failure = CandidateFailure::BadFinalConversion;
            }
        }
        set.cands.push(cand);
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
        match conv::standard_conversion_from(
            self,
            a.ty,
            a.vk,
            false,
            None,
            t,
            conv::ConvCtx::Initialization,
        ) {
            Some(s) => {
                s.second == conv::SecondStep::Identity && s.first == conv::FirstStep::Identity
            }
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
        let fns = self.conversion_functions(a.ty);
        if fns.is_empty() {
            return None;
        }
        let mut set = self.begin_set(SetKind::ConvToRef { lvalue }, Loc::INVALID);
        set.target = to;
        set.args.push(*a);
        for (found, f) in fns {
            self.add_conversion_candidate(&mut set, f, found, to, opts);
        }
        let r = if set.viable().next().is_some() {
            Some(self.udc_outcome(&mut set, a.ty, to, false))
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
        match (a, b) {
            (Ics::Standard(x), Ics::Standard(y)) => compare_standard_conversions(self, x, y),
            (Ics::UserDefined(x), Ics::UserDefined(y)) => {
                // /3.1: a list-initialization sequence converting to
                // std::initializer_list is better (approximated by the
                // initializer-list-constructor bit).
                if x.list_ctor != y.list_ctor {
                    return if x.list_ctor { IcsCompare::Better } else { IcsCompare::Worse };
                }
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
