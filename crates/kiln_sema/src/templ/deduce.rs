//! Template argument deduction ([temp.deduct]; spec/sema-templates.md
//! §6). WP-070b lands the deduction core (§6.2): [`Sema::deduce`] over
//! types and [`Sema::deduce_args`] over template argument lists, shared
//! by partial-specialization matching (§4.4) and partial ordering
//! (§6.7). Deduction from a call (§6.1) is WP-071; packs are WP-072.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::ArgListId;
use kiln_ast::types::{
    ArgView, ExcSpec, ExprOperand, NoexceptValue, TemplateArg, TypeFlags, TypeKind, TypeView,
};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;

/// The flags of one deduction step (clang's `TDF_*`, §6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TdfFlags(u16);

impl TdfFlags {
    /// No flags: `P` and `A` must match exactly where not deduced.
    pub const NONE: TdfFlags = TdfFlags(0);
    /// `P` was a reference type: `A` may be more cv-qualified.
    pub const PARAM_WITH_REFERENCE_TYPE: TdfFlags = TdfFlags(1);
    /// Top-level cv-qualifiers of `P` and `A` are ignored.
    pub const IGNORE_QUALIFIERS: TdfFlags = TdfFlags(2);
    /// A class template-id `P` may match a base class of `A`
    /// ([temp.deduct.call]/4.3).
    pub const DERIVED_CLASS: TdfFlags = TdfFlags(4);
    /// A non-dependent `P` matches anything (checked elsewhere).
    pub const SKIP_NON_DEPENDENT: TdfFlags = TdfFlags(8);
    /// `P` is the parameter type list of a function type (pack rules).
    pub const TOP_LEVEL_PARAMETER_TYPE_LIST: TdfFlags = TdfFlags(16);
    /// Deduction for overload resolution (no deduced-type checks).
    pub const IN_OVERLOAD_RESOLUTION: TdfFlags = TdfFlags(32);
    /// `A` came from a reference type.
    pub const ARG_WITH_REFERENCE_TYPE: TdfFlags = TdfFlags(64);
    /// A function type `A` may differ in `noexcept` ([temp.deduct.call]/4.2;
    /// set for every call argument, kept through pointers and member
    /// pointers).
    pub const ALLOW_COMPATIBLE_FUNCTION_TYPE: TdfFlags = TdfFlags(128);
    /// A function type `A` may differ in a non-deduced `noexcept`
    /// (the address of a function template, [temp.deduct.funcaddr]:
    /// clang's `isSameOrCompatibleFunctionType` ignores the exception
    /// specification; the conversion that follows checks it).
    pub const IGNORE_NONDEDUCED_NOEXCEPT: TdfFlags = TdfFlags(256);

    /// Whether every flag of `o` is set.
    pub const fn contains(self, o: TdfFlags) -> bool {
        self.0 & o.0 == o.0
    }

    /// The flags of both.
    #[must_use]
    pub const fn union(self, o: TdfFlags) -> TdfFlags {
        TdfFlags(self.0 | o.0)
    }

    /// The flags of `self` that are also in `o`.
    #[must_use]
    pub const fn intersect(self, o: TdfFlags) -> TdfFlags {
        TdfFlags(self.0 & o.0)
    }
}

/// The result of a deduction (clang's `TemplateDeductionResult`, §10.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tdk {
    /// Deduction succeeded.
    Success,
    /// A parameter was deduced two different ways ([temp.deduct.type]/2).
    Inconsistent,
    /// A parameter was not deduced.
    Incomplete,
    /// A pack expansion was deduced too few elements (WP-072).
    IncompletePack,
    /// `cv T` against an `A` that lacks `cv`.
    Underqualified,
    /// Substituting the deduced arguments failed ([temp.deduct]/8).
    SubstitutionFailure,
    /// The substituted `P` does not match `A` (function call, WP-071).
    DeducedMismatch,
    /// A non-deduced part of `P` does not match `A`.
    NonDeducedMismatch,
    /// Too many call arguments (WP-071).
    TooManyArguments,
    /// Too few call arguments (WP-071).
    TooFewArguments,
    /// An explicitly-specified argument is invalid (WP-071).
    InvalidExplicitArguments,
    /// A non-dependent call argument does not convert (WP-071).
    NonDependentConversionFailure,
    /// The associated constraints are not satisfied (WP-073).
    ConstraintsNotSatisfied,
    /// The instantiation depth limit was reached.
    InstantiationDepth,
    /// Any other failure (no note).
    MiscellaneousDeductionFailure,
    /// The template's declaration is invalid (a parameter or return type
    /// that failed to resolve, `template <class T> void f(T, u);` with
    /// undeclared `u`): never viable and never noted (clang's
    /// `TemplateDeductionResult::Invalid`, WP-308).
    Invalid,
}

/// What a failed deduction reports (clang's `TemplateDeductionInfo`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DeductionInfo {
    /// The template parameter the failure is about (its index at the
    /// deduced depth).
    pub param: Option<u16>,
    /// `Inconsistent`: the first deduction; `NonDeducedMismatch`: `P`.
    pub first: Option<TemplateArg>,
    /// `Inconsistent`: the second deduction; `NonDeducedMismatch`: `A`.
    pub second: Option<TemplateArg>,
    /// `NonDeducedMismatch` of two template names: `A` named a class
    /// template specialization without template-id sugar, so its
    /// template (clang's `TemplateName::Template` kind) prints fully
    /// qualified; otherwise both names print as written.
    pub second_from_decl: bool,
}

/// The arguments deduced so far for the parameters of one template
/// (one depth, §6.2). An undeduced parameter holds [`TemplateArg::NULL`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools, reason = "independent deduction modes")]
pub struct DeducedArgs {
    /// The depth of the parameters being deduced.
    pub depth: u16,
    /// The deduced argument of each parameter, by index.
    pub args: Vec<TemplateArg>,
    /// The failure detail.
    pub info: DeductionInfo,
    /// While deducing one element of a pack expansion: the deductions
    /// of the pack parameters it names (§6.2, §7.2).
    pub pack_cur: Option<Vec<(u16, TemplateArg)>>,
    /// A trailing pack expansion in `P` is deduced from `A` elements
    /// that are pack expansions too, each taken as a whole (clang; the
    /// alias template guides of [over.match.class.deduct]/3).
    pub expansions: bool,
    /// While deducing one element of a pack expansion: its index, which
    /// selects the element of a pack substituted earlier
    /// (`SubstTemplateTypeParmPackType` in a partially substituted
    /// pattern, §7.2).
    pub pack_elem: Option<usize>,
    /// The parameters (by index) named by the pattern of a pack expansion
    /// in `P` that matched no `A` element: a pack among them is deduced
    /// as empty ([temp.deduct.type]/9; clang `PackDeductionScope::finish`).
    /// Partial ordering reads it ([temp.deduct.partial]/12).
    pub empty_expanded: Vec<u16>,
    /// Deducing for the partial ordering of function templates
    /// ([temp.func.order]/3): `A` is the other template's transformed
    /// parameter type, so a template template parameter's template-id
    /// takes `A`'s extra arguments as its defaults (`deduce_ttp_id_args`).
    pub ordering: bool,
    /// A non-type parameter typed by another template parameter
    /// (`template<class T, T N>`) does not deduce its type from the
    /// argument's (`deduce_nttp_type`): clang 19's check that a partial
    /// specialization is more specialized than its primary saw the
    /// argument already converted to that type (`-fclang-compat=19`;
    /// `not_more_specialized_than_primary`).
    pub converted_values: bool,
    /// Deducing a partial specialization's arguments against themselves
    /// ([temp.spec.partial.general]/8, `check_partial_deducible`): a
    /// function parameter pack that is not last is skipped (a non-deduced
    /// context, [temp.deduct.type]/5.7) and the parameters after it are
    /// still deduced (clang `MarkUsedTemplateParameters`, `OnlyDeduced`).
    pub self_deduce: bool,
    /// Where the deduction was requested (the call, the declaration, the
    /// alias template): the location of a diagnostic issued from a step
    /// that has none of its own.
    pub loc: Loc,
}

impl DeducedArgs {
    /// Nothing deduced yet for `n` parameters at `depth`.
    pub fn new(depth: u16, n: usize) -> Self {
        DeducedArgs {
            depth,
            args: vec![TemplateArg::NULL; n],
            info: DeductionInfo::default(),
            pack_cur: None,
            expansions: false,
            pack_elem: None,
            empty_expanded: Vec::new(),
            ordering: false,
            converted_values: false,
            self_deduce: false,
            loc: Loc::INVALID,
        }
    }

    /// The first parameter left undeduced.
    pub fn first_undeduced(&self) -> Option<u16> {
        self.args
            .iter()
            .position(|a| matches!(a.view(), ArgView::Null))
            .and_then(|i| u16::try_from(i).ok())
    }
}

/// Recursion limit of the structural deduction (types nested deeper
/// fail instead of exhausting the stack).
const DEDUCE_DEPTH_LIMIT: u32 = 512;

/// A template parameter named by a type, expression or template name.
#[derive(Clone, Copy, Debug)]
struct ParmRef {
    depth: u16,
    index: u16,
    pack: bool,
}

impl Sema<'_, '_> {
    /// Deduces the template parameters of depth `d.depth` in `p` from
    /// `a` ([temp.deduct.type]; §6.2). Both are compared canonically.
    pub fn deduce(
        &mut self,
        p: QualType,
        a: QualType,
        flags: TdfFlags,
        d: &mut DeducedArgs,
    ) -> Tdk {
        self.deduce_ty(p, a, flags, d, 0)
    }

    /// Deduces from template argument lists `ps` (the parameter side,
    /// as written) and `as_` ([temp.deduct.type]/9). Packs are WP-072:
    /// a pack on either side reports and fails. `as_` may be longer than
    /// `ps` only when `must_match` is false.
    pub fn deduce_args(
        &mut self,
        ps: &[TemplateArg],
        as_: &[TemplateArg],
        must_match: bool,
        d: &mut DeducedArgs,
        loc: Loc,
    ) -> Tdk {
        if loc.is_valid() {
            d.loc = loc;
        }
        self.deduce_arg_list(ps, as_, must_match, d, 0, loc)
    }

    fn deduce_arg_list(
        &mut self,
        ps: &[TemplateArg],
        as_: &[TemplateArg],
        must_match: bool,
        d: &mut DeducedArgs,
        level: u32,
        loc: Loc,
    ) -> Tdk {
        if ps.iter().chain(as_).any(|a| self.is_pack_arg(*a)) {
            return self.deduce_arg_list_packs(ps, as_, must_match, d, level, loc);
        }
        if ps.len() != as_.len() && (must_match || as_.len() < ps.len()) {
            return Tdk::MiscellaneousDeductionFailure;
        }
        for (&p, &a) in ps.iter().zip(as_) {
            let r = self.deduce_arg(p, a, d, level, loc);
            if r != Tdk::Success {
                return r;
            }
        }
        Tdk::Success
    }

    /// [`Self::deduce_arg_list`] with packs ([temp.deduct.type]/9): both
    /// lists are flattened (an argument pack contributes its elements); a
    /// pack expansion in `P` matches every remaining `A` element.
    fn deduce_arg_list_packs(
        &mut self,
        ps: &[TemplateArg],
        as_: &[TemplateArg],
        must_match: bool,
        d: &mut DeducedArgs,
        level: u32,
        loc: Loc,
    ) -> Tdk {
        let ps = self.flatten_args(ps);
        // [temp.deduct.type]/9: a pack expansion that is not the last
        // argument makes the entire list a non-deduced context, so nothing
        // is deduced from it, not even the arguments before it (clang
        // `hasPackExpansionBeforeEnd`): `template<class T, class... U>
        // X<T, U..., int>` deduces neither `T` nor `U`.
        if ps.iter().rev().skip(1).any(|&p| self.is_pack_arg(p)) {
            return Tdk::Success;
        }
        let as_ = self.flatten_args(as_);
        for (k, &p) in ps.iter().enumerate() {
            if let Some(pat) = self.arg_expansion_pattern(p) {
                let rest = as_.get(k..).unwrap_or(&[]);
                if !d.expansions && rest.iter().any(|&a| self.is_pack_arg(a)) {
                    // `A` has an expansion too: equal only as written.
                    let same = rest.len() == 1
                        && self.ast.types.canonical_arg(p) == self.ast.types.canonical_arg(rest[0]);
                    return if same { Tdk::Success } else { Tdk::MiscellaneousDeductionFailure };
                }
                if rest.is_empty() && d.pack_cur.is_none() {
                    let mut used = vec![false; d.args.len()];
                    self.mark_used_arg(pat, u32::from(d.depth), &mut used, 0);
                    for (i, _) in used.iter().enumerate().filter(|x| *x.1) {
                        let i = i as u16;
                        if !d.empty_expanded.contains(&i) {
                            d.empty_expanded.push(i);
                        }
                    }
                }
                return self.deduce_pack_elements(rest.len(), d, |s, i, d| {
                    if let Some((ap, n)) = s.type_expansion_parts(pat, rest[i], d) {
                        return s.deduce_pattern_from_expansion(pat, ap, n, d, level, loc);
                    }
                    if let Some((an, n)) = Self::template_expansion_parts(pat, rest[i], d) {
                        return s.deduce_template_from_expansion(pat, an, n, d, level, loc);
                    }
                    s.deduce_arg(pat, rest[i], d, level, loc)
                });
            }
            let Some(&a) = as_.get(k) else { return Tdk::MiscellaneousDeductionFailure };
            if self.is_pack_arg(a) {
                return Tdk::MiscellaneousDeductionFailure;
            }
            let r = self.deduce_arg(p, a, d, level, loc);
            if r != Tdk::Success {
                return r;
            }
        }
        if as_.len() != ps.len() && (must_match || as_.len() < ps.len()) {
            return Tdk::MiscellaneousDeductionFailure;
        }
        Tdk::Success
    }

    /// With [`DeducedArgs::expansions`], a type pattern `pat` of `P`
    /// against an `A` element that is a type pack expansion: that
    /// expansion's pattern and expansion count.
    fn type_expansion_parts(
        &self,
        pat: TemplateArg,
        a: TemplateArg,
        d: &DeducedArgs,
    ) -> Option<(QualType, Option<u32>)> {
        if !d.expansions || !matches!(pat.view(), ArgView::Type(_)) {
            return None;
        }
        let ArgView::Type(q) = a.view() else { return None };
        match self.ast.types.canonical_view(q) {
            TypeView::PackExpansion { pattern, num_expansions } => Some((pattern, num_expansions)),
            _ => None,
        }
    }

    /// [`Self::type_expansion_parts`] for a template template pattern
    /// `pat` against a template pack expansion `B'...`: its template and
    /// expansion count.
    fn template_expansion_parts(
        pat: TemplateArg,
        a: TemplateArg,
        d: &DeducedArgs,
    ) -> Option<(kiln_ast::ids::TemplateNameId, Option<u32>)> {
        if !d.expansions || !matches!(pat.view(), ArgView::Template(_)) {
            return None;
        }
        match a.view() {
            ArgView::TemplateExpansion { name, num_expansions } => Some((name, num_expansions)),
            _ => None,
        }
    }

    /// [`Self::deduce_pattern_from_expansion`] for a template template
    /// pattern: `B` from `B'...` is `B'...`.
    fn deduce_template_from_expansion(
        &mut self,
        pat: TemplateArg,
        an: kiln_ast::ids::TemplateNameId,
        n: Option<u32>,
        d: &mut DeducedArgs,
        level: u32,
        loc: Loc,
    ) -> Tdk {
        let r = self.deduce_arg(pat, TemplateArg::template(an), d, level, loc);
        if r != Tdk::Success {
            return r;
        }
        if let Some(cur) = d.pack_cur.as_mut() {
            for e in cur.iter_mut() {
                if let ArgView::Template(t) = e.1.view() {
                    e.1 = TemplateArg::template_expansion(t, n);
                }
            }
        }
        Tdk::Success
    }

    /// [temp.deduct.type]/9 in partial ordering ([temp.deduct.partial]/8;
    /// clang deduces the pattern of `P` from the pattern of `A`): the
    /// pattern `pat` of a `P` expansion against an `A` element that is a
    /// pack expansion of pattern `ap`. Each pack element deduced from it
    /// is that expansion again (`Tail` from `Tail'*...` against `Tail*...`
    /// is `Tail'...`), so substituting `P` gives `A` back.
    fn deduce_pattern_from_expansion(
        &mut self,
        pat: TemplateArg,
        ap: QualType,
        n: Option<u32>,
        d: &mut DeducedArgs,
        level: u32,
        loc: Loc,
    ) -> Tdk {
        let r = self.deduce_arg(pat, TemplateArg::ty(ap), d, level, loc);
        if r != Tdk::Success {
            return r;
        }
        if let Some(cur) = d.pack_cur.as_mut() {
            for e in cur.iter_mut() {
                if let ArgView::Type(t) = e.1.view() {
                    e.1 = TemplateArg::ty(self.ast.types.pack_expansion(t, n));
                }
            }
        }
        Tdk::Success
    }

    /// The parameter types `ps` of a function type against `as_`: a
    /// trailing pack expansion matches the remaining types.
    fn deduce_type_list(
        &mut self,
        ps: &[QualType],
        as_: &[QualType],
        d: &mut DeducedArgs,
        level: u32,
        top: bool,
    ) -> Tdk {
        for (k, &p) in ps.iter().enumerate() {
            if let TypeView::PackExpansion { pattern, .. } = self.ast.types.view(p.ty()) {
                if k + 1 != ps.len() {
                    if d.self_deduce {
                        // `A` is `P`: its element `k` is the same pack.
                        continue;
                    }
                    return Tdk::Success;
                }
                let rest = as_.get(k..).unwrap_or(&[]).to_vec();
                return self.deduce_pack_elements(rest.len(), d, |s, i, d| {
                    let pattern =
                        if top { s.forwarding_to_lvalue(pattern, rest[i], d) } else { pattern };
                    s.deduce_ty(pattern, rest[i], TdfFlags::NONE, d, level)
                });
            }
            let Some(&a) = as_.get(k) else { return Tdk::MiscellaneousDeductionFailure };
            // [temp.deduct.type]/10: an `A` that is a function parameter
            // pack (partial ordering) against a `P` that is not one fails
            // (clang's `PackExpansionType` check in the type-list case).
            if matches!(self.ast.types.view(a.ty()), TypeView::PackExpansion { .. }) {
                return Tdk::MiscellaneousDeductionFailure;
            }
            let p = if top { self.forwarding_to_lvalue(p, a, d) } else { p };
            let r = self.deduce_ty(p, a, TdfFlags::NONE, d, level);
            if r != Tdk::Success {
                return r;
            }
        }
        if as_.len() == ps.len() { Tdk::Success } else { Tdk::MiscellaneousDeductionFailure }
    }

    /// [temp.deduct.type]/10: in the top-level parameter-type-list of a
    /// function type deduced against the address target or a declaration
    /// ([temp.deduct.funcaddr], [temp.deduct.decl]), a forwarding
    /// reference `T&&` paired with an lvalue reference `A` is adjusted to
    /// `T` (clang `TDF_TopLevelParameterTypeList`).
    fn forwarding_to_lvalue(&self, p: QualType, a: QualType, d: &DeducedArgs) -> QualType {
        let t = &self.ast.types;
        match t.canonical_view(p) {
            TypeView::RValueRef(inner)
                if inner.quals().is_empty()
                    && matches!(t.canonical_view(a), TypeView::LValueRef(_))
                    && matches!(t.canonical_view(inner),
                        TypeView::CanonTemplateTypeParm { depth, .. } if depth as u16 == d.depth) =>
            {
                inner
            }
            _ => p,
        }
    }

    /// Deduces a pack expansion from `n` elements (§6.2): `f(i)`
    /// deduces element `i`; the pack parameters it names collect one
    /// argument each, then become `Pack` arguments (agreeing with any
    /// already deduced or given explicitly).
    pub(crate) fn deduce_pack_elements(
        &mut self,
        n: usize,
        d: &mut DeducedArgs,
        f: impl FnMut(&mut Self, usize, &mut DeducedArgs) -> Tdk,
    ) -> Tdk {
        self.deduce_pack_elements_from(None, n, d, f)
    }

    /// The index and length of the explicitly given pack among `params`,
    /// if any ([temp.arg.explicit]/9).
    pub(crate) fn explicit_pack(&self, params: &[DeclId], d: &DeducedArgs) -> Option<(u16, usize)> {
        params
            .iter()
            .enumerate()
            .filter(|&(_, &p)| self.ast.template_parm(p).is_some_and(|x| x.pack))
            .find_map(|(i, _)| match d.args.get(i).map(TemplateArg::view) {
                Some(ArgView::Pack(l)) => {
                    Some((u16::try_from(i).ok()?, self.ast.types.args(l).len()))
                }
                _ => None,
            })
    }

    /// [`Self::deduce_pack_elements`] where pack `explicit = (index, m)`
    /// was given `m` elements explicitly ([temp.arg.explicit]/9). If the
    /// pattern expands only that pack, elements `0..m` are fixed and
    /// `m..n` extend it (a failure in a fixed element is ignored). If it
    /// expands other packs too, they are deduced from all `n` elements
    /// and the explicit pack keeps its given elements (clang: the
    /// partially substituted pack is not extended then).
    pub(crate) fn deduce_pack_elements_from(
        &mut self,
        explicit: Option<(u16, usize)>,
        n: usize,
        d: &mut DeducedArgs,
        mut f: impl FnMut(&mut Self, usize, &mut DeducedArgs) -> Tdk,
    ) -> Tdk {
        let saved = d.pack_cur.take();
        let (ep, m) = explicit.map_or((None, 0), |(i, m)| (Some(i), m.min(n)));
        // Each pack's deductions, by element index.
        let mut per: Vec<(u16, Vec<(usize, TemplateArg)>)> = Vec::new();
        let mut fixed_fail: Option<(Tdk, DeductionInfo)> = None;
        let saved_elem = d.pack_elem;
        for i in 0..n {
            d.pack_cur = Some(Vec::new());
            d.pack_elem = Some(i);
            let r = f(self, i, d);
            d.pack_elem = saved_elem;
            let cur = d.pack_cur.take().unwrap_or_default();
            if r != Tdk::Success {
                if i < m {
                    if fixed_fail.is_none() {
                        fixed_fail = Some((r, d.info));
                    }
                    continue;
                }
                d.pack_cur = saved;
                return r;
            }
            for (idx, a) in cur {
                if let Some(x) = per.iter_mut().find(|x| x.0 == idx) {
                    x.1.push((i, a));
                } else {
                    let mut v = Vec::with_capacity(n);
                    v.push((i, a));
                    per.push((idx, v));
                }
            }
        }
        d.pack_cur = saved;
        let multi = ep.is_some() && per.iter().any(|x| Some(x.0) != ep);
        if multi && let Some((r, info)) = fixed_fail {
            d.info = info;
            return r;
        }
        for (idx, v) in per {
            let v: Vec<TemplateArg> = if Some(idx) != ep {
                if v.len() != n {
                    continue;
                }
                v.into_iter().map(|x| x.1).collect()
            } else if multi {
                continue;
            } else {
                let mut w = Vec::with_capacity(n);
                if let Some(ArgView::Pack(l)) = d.args.get(idx as usize).map(TemplateArg::view) {
                    w.extend(self.ast.types.args(l).iter().take(m));
                }
                w.extend(v.into_iter().filter(|x| x.0 >= m).map(|x| x.1));
                if w.len() != n {
                    continue;
                }
                w
            };
            let l = self.ast.types.arg_list(&v);
            let r = self.record_pack(idx, TemplateArg::pack(l), d);
            if r != Tdk::Success {
                return r;
            }
        }
        Tdk::Success
    }

    /// Records a deduced pack for parameter `index` (a pack already
    /// deduced must agree element by element; an explicitly given prefix
    /// is extended, [temp.arg.explicit]/9).
    pub(crate) fn record_pack(&mut self, index: u16, a: TemplateArg, d: &mut DeducedArgs) -> Tdk {
        let i = index as usize;
        if i >= d.args.len() {
            d.args.resize(i + 1, TemplateArg::NULL);
        }
        let prev = d.args[i];
        let ArgView::Pack(pl) = prev.view() else {
            if matches!(prev.view(), ArgView::Null) {
                d.args[i] = a;
                return Tdk::Success;
            }
            return Tdk::Inconsistent;
        };
        let ArgView::Pack(al) = a.view() else { return Tdk::Inconsistent };
        let (pv, av) = (self.ast.types.args(pl).to_vec(), self.ast.types.args(al).to_vec());
        if pv.len() <= av.len() && pv.iter().zip(&av).all(|(&x, &y)| self.same_deduced(x, y)) {
            d.args[i] = a;
            return Tdk::Success;
        }
        d.info.param = Some(index);
        d.info.first = Some(prev);
        d.info.second = Some(a);
        Tdk::Inconsistent
    }

    /// `as_` with every argument pack replaced by its elements.
    fn flatten_args(&self, as_: &[TemplateArg]) -> Vec<TemplateArg> {
        let mut out = Vec::with_capacity(as_.len());
        for &a in as_ {
            match a.view() {
                ArgView::Pack(l) => out.extend_from_slice(self.ast.types.args(l)),
                _ => out.push(a),
            }
        }
        out
    }

    /// The pattern of a pack-expansion template argument `P`, as an
    /// argument.
    pub(crate) fn arg_expansion_pattern(&mut self, a: TemplateArg) -> Option<TemplateArg> {
        match a.view() {
            ArgView::Type(q) => match self.ast.types.view(q.ty()) {
                TypeView::PackExpansion { pattern, .. } => Some(TemplateArg::ty(pattern)),
                _ => match self.ast.types.canonical_view(q) {
                    TypeView::PackExpansion { pattern, .. } => Some(TemplateArg::ty(pattern)),
                    _ => None,
                },
            },
            ArgView::Expr { expr, .. } => {
                let n = self.ast.exprs.get(expr);
                (n.kind() == ExprKind::PackExpansionExpr)
                    .then(|| ExprId::new(n.a()))
                    .map(|p| self.ast.builder().template_arg_expr(p))
            }
            ArgView::Profile(p) => {
                let e = self.ast.types.profile_expr(p)?;
                let n = self.ast.exprs.get(e);
                (n.kind() == ExprKind::PackExpansionExpr)
                    .then(|| ExprId::new(n.a()))
                    .map(|p| self.ast.builder().template_arg_expr(p))
            }
            ArgView::TemplateExpansion { name, .. } => Some(TemplateArg::template(name)),
            _ => None,
        }
    }

    /// Whether `a` is a pack or a pack expansion (WP-072).
    pub(crate) fn is_pack_arg(&self, a: TemplateArg) -> bool {
        let t = &self.ast.types;
        match a.view() {
            ArgView::Pack(_) | ArgView::TemplateExpansion { .. } => true,
            ArgView::Type(q) => t.canonical_kind(q) == TypeKind::PackExpansion,
            ArgView::Expr { expr, .. } => {
                self.ast.exprs.get(expr).kind() == ExprKind::PackExpansionExpr
            }
            ArgView::Profile(p) => t
                .profile_expr(p)
                .is_some_and(|e| self.ast.exprs.get(e).kind() == ExprKind::PackExpansionExpr),
            _ => false,
        }
    }

    /// One template argument pair ([temp.deduct.type]/8).
    fn deduce_arg(
        &mut self,
        p: TemplateArg,
        a: TemplateArg,
        d: &mut DeducedArgs,
        level: u32,
        loc: Loc,
    ) -> Tdk {
        let mismatch = |d: &mut DeducedArgs| {
            d.info.first = Some(p);
            d.info.second = Some(a);
            Tdk::NonDeducedMismatch
        };
        match p.view() {
            ArgView::Type(pt) => match a.view() {
                ArgView::Type(at) => self.deduce_ty(pt, at, TdfFlags::NONE, d, level + 1),
                _ => mismatch(d),
            },
            ArgView::Template(pn) => {
                let ArgView::Template(an) = a.view() else { return mismatch(d) };
                self.deduce_template_name(pn, an, d)
            }
            ArgView::Value { .. } => {
                let t = &self.ast.types;
                if t.canonical_arg(p) == t.canonical_arg(a) { Tdk::Success } else { mismatch(d) }
            }
            ArgView::Expr { expr, .. } => self.deduce_expr_arg(Some(expr), p, a, d),
            ArgView::Profile(pr) => {
                let e = self.ast.types.profile_expr(pr);
                self.deduce_expr_arg(e, p, a, d)
            }
            ArgView::Null => Tdk::Success,
            ArgView::Pack(_) | ArgView::TemplateExpansion { .. } => {
                let at = if loc.is_valid() { loc } else { d.loc };
                self.templ_unimplemented("template argument deduction for a pack", at);
                Tdk::MiscellaneousDeductionFailure
            }
        }
    }

    /// A non-type argument `P` given as an expression: deduces when it
    /// names a parameter of the deduced depth; any other expression is a
    /// non-deduced context ([temp.deduct.type]/5.3), checked when the
    /// deduction is finished.
    fn deduce_expr_arg(
        &mut self,
        e: Option<ExprId>,
        p: TemplateArg,
        a: TemplateArg,
        d: &mut DeducedArgs,
    ) -> Tdk {
        let Some(parm) = e.and_then(|e| self.nttp_of_expr(e)) else {
            return self.check_nondeduced_value(e, p, a, d);
        };
        if parm.depth != d.depth {
            return Tdk::Success;
        }
        match a.view() {
            ArgView::Value { .. } | ArgView::Expr { .. } | ArgView::Profile(_) => {
                let r = self.record_deduction(parm.index, parm.pack, a, d);
                if r != Tdk::Success || parm.pack {
                    return r;
                }
                self.deduce_nttp_type(e, a, d)
            }
            _ => {
                d.info.first = Some(p);
                d.info.second = Some(a);
                Tdk::NonDeducedMismatch
            }
        }
    }

    /// [temp.deduct.type]/20 (C++17): a deduced non-type parameter whose
    /// type is dependent, `template<class T, T *Ptr>`, deduces its type
    /// from the type of the argument `a` (clang's
    /// `DeduceNonTypeTemplateArgument`, `TDF_SkipNonDependent`; a
    /// reference parameter matches the argument's type as that
    /// reference). `e` names the parameter.
    fn deduce_nttp_type(&mut self, e: Option<ExprId>, a: TemplateArg, d: &mut DeducedArgs) -> Tdk {
        if self.std < kiln_lex::Std::Cxx17 {
            return Tdk::Success;
        }
        let Some(decl) = e.and_then(|e| self.nttp_decl_of_expr(e)) else { return Tdk::Success };
        let Some(pt) = self.ast.decls.get(decl).ty() else { return Tdk::Success };
        if !self.ast.types.is_dependent(pt)
            || (d.converted_values && !self.nttp_type_has_placeholder(pt))
        {
            return Tdk::Success;
        }
        let at = match a.view() {
            ArgView::Value { ty, .. } => ty,
            ArgView::Expr { expr, .. } => self.ast.exprs.get(expr).ty(),
            ArgView::Profile(pr) => match self.ast.types.profile_expr(pr) {
                Some(x) => self.ast.exprs.get(x).ty(),
                None => return Tdk::Success,
            },
            _ => return Tdk::Success,
        };
        if at == QualType::ERROR {
            return Tdk::Success;
        }
        let at = match self.ast.types.canonical_view(pt) {
            TypeView::LValueRef(_) if !self.ast.types.is_reference(at) => {
                self.ast.types.lvalue_ref_to(at)
            }
            TypeView::RValueRef(_) if !self.ast.types.is_reference(at) => {
                self.ast.types.rvalue_ref_to(at)
            }
            _ => at,
        };
        self.deduce_ty(pt, at, TdfFlags::SKIP_NON_DEPENDENT, d, 0)
    }

    /// The non-type template parameter declaration `e` names (as
    /// [`Self::nttp_of_expr`]).
    fn nttp_decl_of_expr(&self, mut e: ExprId) -> Option<DeclId> {
        let ex = &self.ast.exprs;
        loop {
            match ex.get(e).kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr => {
                    e = ex.operand(e);
                }
                ExprKind::SubstNonTypeTemplateParmExpr => e = ex.subst_replacement(e),
                ExprKind::DeclRefExpr => {
                    let decl = ex.decl(e);
                    return (self.ast.decls.get(decl).kind() == DeclKind::NonTypeTemplateParm)
                        .then_some(decl);
                }
                _ => return None,
            }
        }
    }

    /// A non-deduced non-type `P` that is a value already (a written
    /// constant, or a parameter given explicitly and substituted,
    /// [temp.deduct]/2) against a value `A`: unequal values are a
    /// mismatch now, so a derived-to-base deduction tries the next base
    /// ([temp.deduct.call]/4.3; clang converts the substituted argument
    /// to an integral one). A constant `P` against a value-dependent `A`
    /// (a synthesized unique value in partial ordering,
    /// [temp.deduct.partial]/3) is a mismatch too, as clang's integral
    /// `P` against an expression `A`: `f(ie<I, 1>)` is more specialized
    /// than `f(ie<I, N>)`. Anything else is checked when the deduction
    /// is finished.
    fn check_nondeduced_value(
        &mut self,
        e: Option<ExprId>,
        p: TemplateArg,
        a: TemplateArg,
        d: &mut DeducedArgs,
    ) -> Tdk {
        let Some(e) = e else { return Tdk::Success };
        if self.expr_is_value_dependent(e) {
            return Tdk::Success;
        }
        let av = match a.view() {
            ArgView::Value { value, ty } => match self.const_int(value) {
                Some(av) if !self.is_class_typed(e) => Some(av),
                _ => return self.check_nondeduced_class_value(e, ty, value, p, a, d),
            },
            ArgView::Expr { .. } | ArgView::Profile(_) => {
                if self.ast.types.canonical_arg(p) == self.ast.types.canonical_arg(a) {
                    return Tdk::Success;
                }
                let ae = match a.view() {
                    ArgView::Expr { expr, .. } => Some(expr),
                    ArgView::Profile(pr) => self.ast.types.profile_expr(pr),
                    _ => None,
                };
                match ae {
                    Some(x) if self.expr_is_value_dependent(x) => None,
                    _ => return Tdk::Success,
                }
            }
            _ => return Tdk::Success,
        };
        let Some(pv) = self.fold_int(e) else { return Tdk::Success };
        if av.is_some_and(|av| av == pv) {
            return Tdk::Success;
        }
        d.info.first = Some(p);
        d.info.second = Some(a);
        Tdk::NonDeducedMismatch
    }

    /// [`Self::check_nondeduced_value`] for an argument `A` whose value
    /// `value` of type `ty` is not an integer: a `P` of another class
    /// type, or of class type `ty` with another value, does not match
    /// (clang's `DeduceTemplateArguments` compares the substituted
    /// `P`'s structural value with `A`'s; [temp.type]/2). Base classes
    /// tried for `P` ([temp.deduct.call]/4.3) need it: `get<Tag>(map)`
    /// with `Tag` given picks the one base whose tag equals it.
    fn check_nondeduced_class_value(
        &mut self,
        e: ExprId,
        ty: QualType,
        value: kiln_ast::ConstValueId,
        p: TemplateArg,
        a: TemplateArg,
        d: &mut DeducedArgs,
    ) -> Tdk {
        let et = self.ast.exprs.get(e).ty();
        if self.ast.types.is_dependent(et) {
            return Tdk::Success;
        }
        let et = crate::conv::strip_cv(self, self.ast.types.canonical(et));
        let at = crate::conv::strip_cv(self, self.ast.types.canonical(ty));
        let class = |s: &Self, t: QualType| s.ast.types.as_record(t).is_some();
        if !class(self, et) && !class(self, at) {
            return Tdk::Success;
        }
        let same = et == at
            && match self.param_object_of(e) {
                Some(pv) => pv == value,
                None => self.try_fold(e).is_none_or(|v| v == value),
            };
        if same {
            return Tdk::Success;
        }
        d.info.first = Some(p);
        d.info.second = Some(a);
        Tdk::NonDeducedMismatch
    }

    /// Whether the non-dependent type of `e` is a class type.
    fn is_class_typed(&self, e: ExprId) -> bool {
        let t = self.ast.exprs.get(e).ty();
        !self.ast.types.is_dependent(t)
            && self.ast.types.as_record(self.ast.types.canonical(t)).is_some()
    }

    /// The interned value of the template parameter object (§13.2
    /// [temp.param]/8) that `e` names once wrappers are peeled, if any.
    fn param_object_of(&self, e: ExprId) -> Option<kiln_ast::ConstValueId> {
        let mut e = e;
        for _ in 0..64 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ParenExpr | ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr => {
                    e = self.ast.exprs.operand(e);
                }
                ExprKind::SubstNonTypeTemplateParmExpr => e = self.ast.exprs.subst_replacement(e),
                ExprKind::DeclRefExpr => {
                    return self.ast.templates.param_object_value(self.ast.exprs.decl(e));
                }
                _ => return None,
            }
        }
        None
    }

    /// The index of the non-type template parameter of depth `depth`
    /// that argument `a` names as a bare (possibly converted) reference.
    pub(crate) fn bare_nttp_index(&self, a: TemplateArg, depth: u16) -> Option<u16> {
        let ArgView::Expr { expr, .. } = a.view() else { return None };
        self.nttp_of_expr(expr).filter(|r| r.depth == depth && !r.pack).map(|r| r.index)
    }

    /// Whether `e` names a non-type template parameter of `depth` as a
    /// bare (possibly converted) reference: a deduced context.
    pub(crate) fn nttp_of_expr_at(&self, e: Option<ExprId>, depth: u16) -> bool {
        e.and_then(|e| self.nttp_of_expr(e)).is_some_and(|r| r.depth == depth)
    }

    /// The non-type template parameter expression `e` names (through
    /// implicit conversions, not parentheses).
    fn nttp_of_expr(&self, mut e: ExprId) -> Option<ParmRef> {
        let ex = &self.ast.exprs;
        loop {
            match ex.get(e).kind() {
                // Not through parentheses: `S<(N)>` is a non-deduced
                // context (CWG526, [temp.deduct.type]/5.3; clang
                // `getDeducedParameterFromExpr`).
                ExprKind::ImplicitCastExpr | ExprKind::ConstantExpr => {
                    e = ex.operand(e);
                }
                // clang `getDeducedParameterFromExpr` looks through the
                // substitution of an alias template's parameter (WP-156:
                // `__simd_vector<_Tp, _Np>`).
                ExprKind::SubstNonTypeTemplateParmExpr => e = ex.subst_replacement(e),
                ExprKind::DeclRefExpr => {
                    let decl = ex.decl(e);
                    if self.ast.decls.get(decl).kind() != DeclKind::NonTypeTemplateParm {
                        return None;
                    }
                    let i = self.ast.template_parm(decl)?;
                    return Some(ParmRef { depth: i.depth, index: i.index, pack: i.pack });
                }
                _ => return None,
            }
        }
    }

    /// The template parameter a template name names.
    /// Packs are not deduced here (WP-072).
    fn template_parm_of_name(&self, n: kiln_ast::ids::TemplateNameId) -> Option<ParmRef> {
        let t = &self.ast.types;
        match t.template_name_view(t.canonical_template_name(n)) {
            kiln_ast::types::TemplateName::CanonTemplateTemplateParm { depth, index, pack } => {
                Some(ParmRef {
                    depth: u16::try_from(depth).ok()?,
                    index: u16::try_from(index).ok()?,
                    pack,
                })
            }
            _ => None,
        }
    }

    fn deduce_template_name(
        &mut self,
        pn: kiln_ast::ids::TemplateNameId,
        an: kiln_ast::ids::TemplateNameId,
        d: &mut DeducedArgs,
    ) -> Tdk {
        let t = &self.ast.types;
        if let Some(parm) = self.template_parm_of_name(pn)
            && parm.depth == d.depth
        {
            let a = TemplateArg::template(t.canonical_template_name(an));
            return self.record_deduction(parm.index, parm.pack, a, d);
        }
        if t.same_template_name(pn, an) {
            return Tdk::Success;
        }
        let same_decl = match (t.template_decl(pn), t.template_decl(an)) {
            (Some(x), Some(y)) => self.ast.first(x) == self.ast.first(y),
            _ => false,
        };
        if same_decl {
            Tdk::Success
        } else {
            d.info.first = Some(TemplateArg::template(pn));
            d.info.second = Some(TemplateArg::template(an));
            Tdk::NonDeducedMismatch
        }
    }

    /// Records `a` for parameter `index`, checking consistency with an
    /// earlier deduction ([temp.deduct.type]/2, /17).
    fn record_deduction(
        &mut self,
        index: u16,
        pack: bool,
        a: TemplateArg,
        d: &mut DeducedArgs,
    ) -> Tdk {
        // Before C++17 a deduced type keeps no exception specification
        // (spec/types.md §5.4).
        let a = match a.view() {
            ArgView::Type(q) => TemplateArg::ty(self.ast.types.without_exc_sugar(q)),
            _ => a,
        };
        if pack {
            // One element of a pack (§6.2): collected by the expansion.
            let Some(cur) = d.pack_cur.as_mut() else { return Tdk::Success };
            if let Some(&(_, prev)) = cur.iter().find(|x| x.0 == index) {
                if self.same_deduced(prev, a) {
                    return Tdk::Success;
                }
                d.info.param = Some(index);
                d.info.first = Some(prev);
                d.info.second = Some(a);
                return Tdk::Inconsistent;
            }
            cur.push((index, a));
            return Tdk::Success;
        }
        let i = index as usize;
        if i >= d.args.len() {
            d.args.resize(i + 1, TemplateArg::NULL);
        }
        let prev = d.args[i];
        if matches!(prev.view(), ArgView::Null) {
            d.args[i] = a;
            return Tdk::Success;
        }
        if self.same_deduced(prev, a) {
            return Tdk::Success;
        }
        d.info.param = Some(index);
        d.info.first = Some(prev);
        d.info.second = Some(a);
        Tdk::Inconsistent
    }

    /// Whether two deductions of one parameter agree: canonically equal,
    /// or (non-type) equal values ([temp.deduct.type]/17).
    fn same_deduced(&self, x: TemplateArg, y: TemplateArg) -> bool {
        let t = &self.ast.types;
        if t.canonical_arg(x) == t.canonical_arg(y) {
            return true;
        }
        match (x.view(), y.view()) {
            (ArgView::Value { value: v1, .. }, ArgView::Value { value: v2, .. }) => {
                v1 == v2
                    || matches!((self.const_int(v1), self.const_int(v2)), (Some(a), Some(b)) if a == b)
            }
            _ => false,
        }
    }

    /// The structural match of `P` against `A` (clang's
    /// `DeduceTemplateArgumentsByTypeMatch`).
    fn deduce_ty(
        &mut self,
        p: QualType,
        a: QualType,
        flags: TdfFlags,
        d: &mut DeducedArgs,
        level: u32,
    ) -> Tdk {
        if level > DEDUCE_DEPTH_LIMIT {
            return Tdk::MiscellaneousDeductionFailure;
        }
        let t = &self.ast.types;
        let mut p = t.canonical(p);
        let a = t.canonical(a);
        // [temp.deduct.call]/4.1: the type a reference parameter refers to
        // may be more cv-qualified than `A`; its qualifiers that `A` lacks
        // drop out first (clang `DeduceTemplateArgumentsByTypeMatch`, with
        // `getUnqualifiedArrayType`). An array's are its element's, out of
        // reach of the `cv T` case below once the elements are matched
        // (`const T (&)[N]` from `int[3]`).
        if flags.contains(TdfFlags::PARAM_WITH_REFERENCE_TYPE) && t.is_array(p) {
            let (pq, aq) = (t.effective_quals(p), t.effective_quals(a));
            if pq.bits() & !aq.bits() != 0 {
                let t = &mut self.ast.types;
                let u = t.unqualified(p);
                p = t.qualify(u, kiln_ast::types::Quals::from_bits(pq.bits() & aq.bits()));
            }
        }
        let t = &self.ast.types;
        let mismatch = |d: &mut DeducedArgs| {
            d.info.first = Some(TemplateArg::ty(p));
            d.info.second = Some(TemplateArg::ty(a));
            Tdk::NonDeducedMismatch
        };
        // A non-dependent `P` must match exactly ([temp.deduct.type]/1).
        if !t.qflags(p).intersects(TypeFlags::DEPENDENT) {
            if flags.contains(TdfFlags::SKIP_NON_DEPENDENT) {
                return Tdk::Success;
            }
            let (pq, aq) = (p.quals_bits(), a.quals_bits());
            let same = if flags.contains(TdfFlags::IGNORE_QUALIFIERS) {
                p.without_quals() == a.without_quals()
            } else if flags.contains(TdfFlags::PARAM_WITH_REFERENCE_TYPE) {
                p.without_quals() == a.without_quals() && aq & pq == pq
            } else {
                p == a
            };
            // clang `isSameOrCompatibleFunctionType`: a non-dependent
            // function type (the pointee of `void (C::*)(T)` once the
            // explicit arguments are substituted) matches regardless of
            // `noexcept`; the deduced `A` is checked afterwards
            // ([temp.deduct.call]/4.2).
            if !same
                && flags.contains(TdfFlags::ALLOW_COMPATIBLE_FUNCTION_TYPE)
                && t.is_function(p)
                && t.is_function(a)
            {
                let pn = self.with_exc_spec(p.without_quals(), ExcSpec::None);
                let an = self.with_exc_spec(a.without_quals(), ExcSpec::None);
                let t = &self.ast.types;
                if t.canonical(pn) == t.canonical(an) {
                    return Tdk::Success;
                }
            }
            return if same { Tdk::Success } else { mismatch(d) };
        }
        let pv = t.view(p.ty());
        // `cv T` ([temp.deduct.type]/8: the forms `T` and `cv T`).
        if let TypeView::CanonTemplateTypeParm { depth, index, pack } = pv {
            if depth != u32::from(d.depth) {
                return Tdk::Success;
            }
            // An array carries its element's qualifiers (clang's
            // canonical array type is qualified).
            let a_array = t.is_array(a);
            let aq = if a_array { t.effective_quals(a).bits() } else { a.quals_bits() };
            let pq = p.quals_bits();
            if !flags.contains(TdfFlags::IGNORE_QUALIFIERS) {
                // The argument must carry at least the parameter's
                // qualifiers; a function or reference type carries none.
                let a_noqual = matches!(
                    t.canonical_view(a),
                    TypeView::Function(_) | TypeView::LValueRef(_) | TypeView::RValueRef(_)
                );
                if aq & pq != pq || (pq != 0 && a_noqual) {
                    d.info.param = u16::try_from(index).ok();
                    d.info.first = Some(TemplateArg::ty(p));
                    d.info.second = Some(TemplateArg::ty(a));
                    return Tdk::Underqualified;
                }
            }
            // CWG1584: `cv T` never matches a function type, even where
            // qualifiers are ignored (`const T*` from `void (*)()`).
            if pq != 0 && matches!(t.canonical_view(a), TypeView::Function(_)) {
                return mismatch(d);
            }
            let deduced = if a_array && pq == 0 {
                a
            } else if a_array {
                // `const T&` from `const char[3]`: `T = char[3]`.
                let t = &mut self.ast.types;
                let u = t.unqualified(a);
                t.qualify(u, kiln_ast::types::Quals::from_bits(aq & !pq))
            } else {
                QualType::new(a.ty(), aq & !pq)
            };
            return self.record_deduction(
                u16::try_from(index).unwrap_or(u16::MAX),
                pack,
                TemplateArg::ty(deduced),
                d,
            );
        }
        // Outside of `T`, qualifiers must match ([temp.deduct.type]/1).
        if flags.contains(TdfFlags::PARAM_WITH_REFERENCE_TYPE) {
            if a.quals_bits() & p.quals_bits() != p.quals_bits() {
                return mismatch(d);
            }
        } else if !flags.contains(TdfFlags::IGNORE_QUALIFIERS) && p.quals_bits() != a.quals_bits() {
            return mismatch(d);
        }
        self.deduce_structural(p, a, flags, d, level + 1)
    }

    /// The structural cases of [`Self::deduce_ty`] once `P` is neither
    /// non-dependent nor `cv T`: `P`'s outer type constructor must match
    /// `A`'s ([temp.deduct.type]/8).
    #[allow(clippy::too_many_lines, reason = "one arm per form of [temp.deduct.type]/8")]
    fn deduce_structural(
        &mut self,
        p: QualType,
        a: QualType,
        flags: TdfFlags,
        d: &mut DeducedArgs,
        next: u32,
    ) -> Tdk {
        let mismatch = |d: &mut DeducedArgs| {
            d.info.first = Some(TemplateArg::ty(p));
            d.info.second = Some(TemplateArg::ty(a));
            Tdk::NonDeducedMismatch
        };
        let t = &self.ast.types;
        let pv = t.view(p.ty());
        let av = t.view(a.ty());
        match pv {
            TypeView::Pointer(pp) => {
                let TypeView::Pointer(ap) = av else { return mismatch(d) };
                let sub = flags.intersect(
                    TdfFlags::IGNORE_QUALIFIERS
                        .union(TdfFlags::DERIVED_CLASS)
                        .union(TdfFlags::ALLOW_COMPATIBLE_FUNCTION_TYPE),
                );
                self.deduce_ty(pp, ap, sub, d, next)
            }
            TypeView::LValueRef(pp) => {
                let TypeView::LValueRef(ap) = av else { return mismatch(d) };
                self.deduce_ty(pp, ap, TdfFlags::NONE, d, next)
            }
            TypeView::RValueRef(pp) => {
                let TypeView::RValueRef(ap) = av else { return mismatch(d) };
                self.deduce_ty(pp, ap, TdfFlags::NONE, d, next)
            }
            TypeView::MemberPointer { pointee: pp, class: pc } => {
                let TypeView::MemberPointer { pointee: ap, class: ac } = av else {
                    return mismatch(d);
                };
                let sub = flags.intersect(
                    TdfFlags::IGNORE_QUALIFIERS.union(TdfFlags::ALLOW_COMPATIBLE_FUNCTION_TYPE),
                );
                let r = self.deduce_ty(pp, ap, sub, d, next);
                if r != Tdk::Success {
                    return r;
                }
                self.deduce_ty(QualType::unqual(pc), QualType::unqual(ac), TdfFlags::NONE, d, next)
            }
            TypeView::IncompleteArray(pe) => {
                let TypeView::IncompleteArray(ae) = av else { return mismatch(d) };
                self.deduce_ty(pe, ae, flags.intersect(TdfFlags::IGNORE_QUALIFIERS), d, next)
            }
            TypeView::ConstantArray(pa) => {
                let TypeView::ConstantArray(aa) = av else { return mismatch(d) };
                if pa.size != aa.size {
                    return mismatch(d);
                }
                self.deduce_ty(
                    pa.elem,
                    aa.elem,
                    flags.intersect(TdfFlags::IGNORE_QUALIFIERS),
                    d,
                    next,
                )
            }
            TypeView::DependentSizedArray { elem: pe, size } => {
                let (ae, asize) = match av {
                    TypeView::ConstantArray(aa) => (aa.elem, Some(Ok(aa.size))),
                    TypeView::DependentSizedArray { elem, size } => (elem, Some(Err(size))),
                    _ => return mismatch(d),
                };
                let r =
                    self.deduce_ty(pe, ae, flags.intersect(TdfFlags::IGNORE_QUALIFIERS), d, next);
                if r != Tdk::Success {
                    return r;
                }
                self.deduce_array_bound(size, asize, d)
            }
            // clang `DeduceTemplateArgumentsByTypeMatch`: `_BitInt(N)`
            // deduces `N` from the argument's width as an `int`
            // (spec/types.md §21); the signedness must match. A dependent
            // width deduces as a dependent array bound does (clang only
            // marks it used; kiln's partial-specialization deducibility
            // check deduces the arguments against themselves).
            TypeView::DependentBitInt { signed, width } => {
                let w = match av {
                    TypeView::BitInt { signed: s, width: n } if s == signed => Ok(u64::from(n)),
                    TypeView::DependentBitInt { signed: s, width: aw } if s == signed => {
                        Err(ExprOperand::Profile(aw))
                    }
                    _ => return mismatch(d),
                };
                self.deduce_bound(ExprOperand::Profile(width), Some(w), d, true)
            }
            TypeView::Function(pf) => {
                let TypeView::Function(af) = av else { return mismatch(d) };
                let t = &self.ast.types;
                // The parameter counts are compared pair by pair after
                // the return type (clang: "failed template argument
                // deduction" for a count mismatch).
                if pf.variadic() != af.variadic()
                    || pf.info().ref_qual() != af.info().ref_qual()
                    || pf.info().this_quals() != af.info().this_quals()
                {
                    return mismatch(d);
                }
                let pps = t.params(&pf).to_vec();
                let aps = t.params(&af).to_vec();
                let (pexc, aexc) = (t.exc_spec(&pf), t.exc_spec(&af));
                let r = self.deduce_ty(pf.ret(), af.ret(), TdfFlags::NONE, d, next);
                if r != Tdk::Success {
                    return r;
                }
                let top = flags.contains(TdfFlags::TOP_LEVEL_PARAMETER_TYPE_LIST);
                let r = self.deduce_type_list(&pps, &aps, d, next, top);
                if r != Tdk::Success {
                    return r;
                }
                self.deduce_noexcept(pexc, aexc, flags, d, p, a)
            }
            TypeView::TemplateSpecialization { name, args, .. } => {
                // The bases are tried from the deductions made before `A`
                // itself was (clang's `DeducedOrig`): a failed match of
                // `I<B>` against `ref<iface>` deduced `I = ref` already.
                let orig = flags.contains(TdfFlags::DERIVED_CLASS).then(|| d.clone());
                let r = self.deduce_template_id(name, args, a, d, next);
                // A mismatch inside the arguments keeps its own `P`/`A`
                // (clang reports the innermost pair); only an `A` that is
                // no template-id at all reports the whole types.
                let not_id = r == Tdk::NonDeducedMismatch && d.info.first.is_none();
                if r == Tdk::Success || !flags.contains(TdfFlags::DERIVED_CLASS) {
                    if not_id {
                        return mismatch(d);
                    }
                    return r;
                }
                // No base matches: the result for `A` itself (clang's
                // `DeduceTemplateSpecArguments`).
                let orig = orig.unwrap_or_else(|| d.clone());
                match self.deduce_from_bases(name, args, a, &orig, d, next) {
                    Some(b) => b,
                    None if not_id => mismatch(d),
                    None => r,
                }
            }
            TypeView::Complex(pe) => {
                let TypeView::Complex(ae) = av else { return mismatch(d) };
                self.deduce_ty(pe, ae, TdfFlags::NONE, d, next)
            }
            TypeView::Vector { elem: pe, count: pc, kind: pk } => {
                let TypeView::Vector { elem: ae, count: ac, kind: ak } = av else {
                    return mismatch(d);
                };
                if pc != ac || pk != ak {
                    return mismatch(d);
                }
                self.deduce_ty(pe, ae, TdfFlags::NONE, d, next)
            }
            TypeView::DependentVector { elem: pe, size, kind: pk } => {
                let (ae, asize) = match av {
                    TypeView::Vector { elem, count, kind } if kind == pk => {
                        (elem, Some(Ok(u64::from(count))))
                    }
                    TypeView::DependentVector { elem, size, kind } if kind == pk => {
                        (elem, Some(Err(size)))
                    }
                    _ => return mismatch(d),
                };
                let r = self.deduce_ty(pe, ae, TdfFlags::NONE, d, next);
                if r != Tdk::Success {
                    return r;
                }
                self.deduce_array_bound(size, asize, d)
            }
            // A pack substituted earlier into a partially substituted
            // pattern (§7.2): its element for the element being deduced
            // must match; outside an element it is not deduced from.
            TypeView::SubstTemplateTypeParmPack { pack, .. } => {
                let elem = d.pack_elem.and_then(|i| t.args(pack).get(i).copied());
                match elem.map(|x| x.view()) {
                    Some(ArgView::Type(e)) => self.deduce_ty(e, a, flags, d, next),
                    _ => Tdk::Success,
                }
            }
            TypeView::PackExpansion { .. } => {
                self.templ_unimplemented("template argument deduction for a pack", d.loc);
                Tdk::MiscellaneousDeductionFailure
            }
            // Non-deduced contexts ([temp.deduct.type]/5): the
            // qualifier of a qualified-id, `decltype`, pack indexing,
            // type transforms. clang 19 also does not deduce through
            // `_Atomic(T)` (its `DeduceTemplateArgumentsByTypeMatch`
            // treats it as non-deduced: "couldn't infer template argument").
            TypeView::DependentName { .. }
            | TypeView::DependentTemplateSpecialization { .. }
            | TypeView::DependentDecltype(_)
            | TypeView::Decltype { .. }
            | TypeView::PackIndexing { .. }
            | TypeView::TypeOf { .. }
            | TypeView::UnaryTransform { .. }
            | TypeView::Splice { .. }
            | TypeView::Atomic(_) => Tdk::Success,
            // A dependent record (a member of the current instantiation)
            // and anything else: exact match.
            _ => {
                if p == a {
                    Tdk::Success
                } else {
                    mismatch(d)
                }
            }
        }
    }

    /// An array bound or vector size `size` against `A`'s (a value or a
    /// dependent expression) ([temp.deduct.type]/8: `T[i]`).
    pub(super) fn deduce_array_bound(
        &mut self,
        size: ExprOperand,
        asize: Option<Result<u64, ExprOperand>>,
        d: &mut DeducedArgs,
    ) -> Tdk {
        self.deduce_bound(size, asize, d, false)
    }

    /// [`Self::deduce_array_bound`]; an undeduced parameter type takes
    /// `int` (a `_BitInt` width) if `int_bound`, else `size_t`.
    fn deduce_bound(
        &mut self,
        size: ExprOperand,
        asize: Option<Result<u64, ExprOperand>>,
        d: &mut DeducedArgs,
        int_bound: bool,
    ) -> Tdk {
        let e = match size {
            ExprOperand::Expr(e) => Some(e),
            ExprOperand::Profile(p) => self.ast.types.profile_expr(p),
        };
        let Some(parm) = e.and_then(|e| self.nttp_of_expr(e)) else { return Tdk::Success };
        if parm.depth != d.depth {
            return Tdk::Success;
        }
        let a = match asize {
            Some(Ok(n)) => {
                // The value has the parameter's type ([temp.deduct.type]/17);
                // an undeduced type uses `size_t`.
                let ty = self
                    .nttp_param_type(e)
                    .unwrap_or_else(|| if int_bound { QualType::INT } else { self.size_type() });
                let v = self.int_const(ty, i128::from(n));
                TemplateArg::value(self.ast.types.canonical(ty), v)
            }
            Some(Err(ExprOperand::Profile(p))) => TemplateArg::profile(p),
            Some(Err(ExprOperand::Expr(e))) => self.ast.builder().template_arg_expr(e),
            None => return Tdk::Success,
        };
        self.record_deduction(parm.index, parm.pack, a, d)
    }

    /// The declared type of the non-type parameter `e` names, when it is
    /// not dependent.
    fn nttp_param_type(&self, e: Option<ExprId>) -> Option<QualType> {
        let mut e = e?;
        let ex = &self.ast.exprs;
        loop {
            match ex.get(e).kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ParenExpr | ExprKind::ConstantExpr => {
                    e = ex.operand(e);
                }
                ExprKind::SubstNonTypeTemplateParmExpr => e = ex.subst_replacement(e),
                _ => break,
            }
        }
        let ty = self.ast.decls.get(ex.decl(e)).ty()?;
        // A placeholder parameter (`template <auto N>`) takes the bound's
        // own type: `std::size_t` for an array bound, `int` for a
        // `_BitInt` width ([temp.deduct.type]/20).
        (!self.ast.types.is_dependent(ty) && !self.contains_undeduced_auto(ty)).then_some(ty)
    }

    /// `noexcept(b)` of a function type `P` ([temp.deduct.type]/8).
    fn deduce_noexcept(
        &mut self,
        pexc: ExcSpec,
        aexc: ExcSpec,
        flags: TdfFlags,
        d: &mut DeducedArgs,
        p: QualType,
        a: QualType,
    ) -> Tdk {
        let a_nothrow = aexc.is_nothrow();
        if let ExcSpec::DependentNoexcept(pr)
        | ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(pr), .. } = pexc
        {
            let e = self.ast.types.profile_expr(pr);
            if let Some(parm) = e.and_then(|e| self.nttp_of_expr(e))
                && parm.depth == d.depth
            {
                let v = self.int_const(QualType::BOOL, i128::from(a_nothrow));
                let a = TemplateArg::value(QualType::BOOL, v);
                return self.record_deduction(parm.index, parm.pack, a, d);
            }
            return Tdk::Success;
        }
        // Not yet instantiated (a member template of a class template
        // specialization, [temp.inst]/14): compared once known, as a
        // dependent one.
        if matches!(pexc, ExcSpec::Deferred(_)) {
            return Tdk::Success;
        }
        // A call argument's (clang ignores a non-deduced mismatch there and
        // checks the deduced `A`, [temp.deduct.call]/4.2).
        if pexc.is_nothrow() == a_nothrow
            || flags.contains(TdfFlags::ALLOW_COMPATIBLE_FUNCTION_TYPE)
            || flags.contains(TdfFlags::IGNORE_NONDEDUCED_NOEXCEPT)
        {
            Tdk::Success
        } else {
            d.info.first = Some(TemplateArg::ty(p));
            d.info.second = Some(TemplateArg::ty(a));
            Tdk::NonDeducedMismatch
        }
    }

    /// The template and arguments of a class type `a`: a specialization
    /// record, or a dependent template-id.
    fn template_id_of(
        &self,
        a: QualType,
    ) -> Option<(DeclId, Option<kiln_ast::ids::TemplateNameId>, ArgListId)> {
        let t = &self.ast.types;
        match t.canonical_view(a) {
            TypeView::Record(r) => {
                let (_, s) = self.ast.spec_info(r)?;
                Some((s.template, None, t.canonical_args(s.args)))
            }
            TypeView::TemplateSpecialization { name, args, .. } => {
                let tmpl = t.template_decl(name);
                Some((tmpl.map_or(DeclId::new(1), |x| self.ast.first(x)), Some(name), args))
            }
            _ => None,
        }
    }

    /// The template name of the first class template-id in `a`'s sugar
    /// (clang's `getAs<TemplateSpecializationType>`), as written. An alias
    /// template-id is looked through to the type it names
    /// ([temp.alias]/2; clang's `getLastTemplateSpecType`): `A<int>` for
    /// `template<class T> using A = S<T>` is a template-id of `S`, so a
    /// base written `A<T>` deduces against `S` ([temp.deduct.call]/4.3).
    fn written_template_name(&self, a: QualType) -> Option<kiln_ast::ids::TemplateNameId> {
        let t = &self.ast.types;
        let mut q = a;
        loop {
            if let TypeView::TemplateSpecialization { name, alias: false, .. } = t.view(q.ty()) {
                return Some(name);
            }
            let n = t.desugar_once(q);
            if n == q {
                return None;
            }
            q = n;
        }
    }

    /// `TT<args>` or a class template-id against `A` (§6.2).
    fn deduce_template_id(
        &mut self,
        name: kiln_ast::ids::TemplateNameId,
        args: ArgListId,
        a: QualType,
        d: &mut DeducedArgs,
        level: u32,
    ) -> Tdk {
        let Some((atmpl, aname, aargs)) = self.template_id_of(a) else {
            // The caller reports `P`/`A` themselves.
            d.info.first = None;
            return Tdk::NonDeducedMismatch;
        };
        let written = self.written_template_name(a);
        let aname = match (written, aname) {
            (Some(n), _) | (None, Some(n)) => n,
            (None, None) => self.ast.types.template_name_of(atmpl),
        };
        let r = self.deduce_template_name(name, aname, d);
        if r != Tdk::Success {
            d.info.second_from_decl = written.is_none();
            return r;
        }
        let mut ps = self.ast.types.args(args).to_vec();
        let as_ = self.ast.types.args(aargs).to_vec();
        if ps.len() < as_.len() {
            self.complete_dependent_args(name, &mut ps);
        }
        let n = ps.len();
        let packs = ps.iter().chain(&as_).any(|x| self.is_pack_arg(*x));
        // `A` a template template parameter's template-id: a unique type
        // of partial ordering ([temp.func.order]/3).
        let unique = self.template_parm_of_name(aname).is_some();
        let r = if n != as_.len() && self.template_parm_of_name(name).is_some() && !packs {
            self.deduce_ttp_id_args(&ps, &as_, (atmpl, unique), d, level)
        } else {
            self.deduce_arg_list(&ps, &as_, true, d, level, d.loc)
        };
        if r == Tdk::NonDeducedMismatch {
            // clang deduces against `P`'s canonical arguments, so an inner
            // `P` prints canonically (`pr<type-parameter-0-0, int>`).
            d.info.first = d.info.first.map(|x| self.ast.types.canonical_arg(x));
        }
        r
    }

    /// `U<Ps...>` for a template template parameter `U` against `A`'s
    /// arguments `as_` of another length (template `atmpl`; `unique` when
    /// `A` names a template template parameter), packs aside.
    ///
    /// [temp.arg.template]/3 (P0522): `U<T>` for `template<class> class U`
    /// matches `rv<double, 1>` of `template<class, size_t = 1> struct rv`.
    /// `P`'s arguments are deduced from the first of `A`'s, and the rest
    /// must be the defaults those give (CWG2398 as clang deduces it;
    /// `rv<double, 2>` does not match). Against a unique type, and in the
    /// partial ordering of function templates, the rest is not checked:
    /// `U` is deduced as `A`'s template with the extra arguments as its
    /// defaults (clang's `DeducedTemplateName`, P3310R5), so
    /// `f(const num<B, E>&)` is more specialized than `f(const U<T>)`
    /// for `template<class, int = 0> struct num` (Boost.Math `sinhc_pi`).
    /// A shorter unique `A` matches under clang 19, which ignores `P`'s
    /// extra arguments there, and fails under clang 22 (`A`'s template
    /// has too few parameters for `U`), so `f(TT<int, int>)` is more
    /// specialized than `f(TT<int>)` only under clang 22.
    fn deduce_ttp_id_args(
        &mut self,
        ps: &[TemplateArg],
        as_: &[TemplateArg],
        a: (DeclId, bool),
        d: &mut DeducedArgs,
        level: u32,
    ) -> Tdk {
        let (atmpl, unique) = a;
        let n = ps.len();
        if n > as_.len() {
            if unique && self.clang_compat < 22 {
                return self.deduce_arg_list(&ps[..as_.len()], as_, true, d, level, d.loc);
            }
            return self.deduce_arg_list(ps, as_, true, d, level, d.loc);
        }
        let r = self.deduce_arg_list(ps, &as_[..n], true, d, level, d.loc);
        if r != Tdk::Success || unique || d.ordering {
            return r;
        }
        let mut full = as_[..n].to_vec();
        let an = self.ast.types.template_name_of(atmpl);
        self.complete_dependent_args(an, &mut full);
        let same = full.len() == as_.len()
            && full.iter().zip(as_).all(|(x, y)| self.same_default_arg(*x, *y));
        if same { r } else { Tdk::MiscellaneousDeductionFailure }
    }

    /// Whether the default `x` that [`Self::deduce_ttp_id_args`] completed
    /// is `A`'s argument `y`. A dependent `A` (the partial specialization
    /// `lambda<protect<T>, Tag>` of Boost.MPL, ordered against
    /// `lambda<F<T1>, Tag>`) keeps a defaulted non-type argument as an
    /// expression, which is compared by its value.
    fn same_default_arg(&mut self, x: TemplateArg, y: TemplateArg) -> bool {
        let t = &self.ast.types;
        if t.canonical_arg(x) == t.canonical_arg(y) {
            return true;
        }
        let (Some(a), Some(b)) = (self.arg_int_value(x), self.arg_int_value(y)) else {
            return false;
        };
        a == b
    }

    /// The integer value of a non-type template argument: a converted
    /// value, or a value-independent expression folded.
    fn arg_int_value(&mut self, a: TemplateArg) -> Option<i128> {
        let e = match a.view() {
            ArgView::Value { value, .. } => return self.const_int(value),
            ArgView::Expr { expr, .. } => expr,
            ArgView::Profile(pr) => self.ast.types.profile_expr(pr)?,
            _ => return None,
        };
        if self.expr_is_value_dependent(e) {
            return None;
        }
        self.fold_int(e)
    }

    /// The arguments of a dependent class template-id `P` completed with
    /// its parameters' defaults ([temp.arg.general]/9), as `A`'s were:
    /// `B<T>` is `B<T, int>` for `template<class, class = int> struct B`
    /// (clang converts a dependent template-id's arguments up front). A
    /// default that cannot be formed leaves `ps` short (the deduction
    /// then fails as before).
    fn complete_dependent_args(
        &mut self,
        name: kiln_ast::ids::TemplateNameId,
        ps: &mut Vec<TemplateArg>,
    ) {
        let Some(t) = self.ast.types.template_decl(name) else { return };
        if self.kind_of(t) != DeclKind::ClassTemplate || ps.iter().any(|a| self.is_pack_arg(*a)) {
            return;
        }
        // The parameters (and their depth) are the head declaration's, as
        // in `check_template_args`: the first declaration may be a friend
        // inside a class template, whose parameters sit one level deeper
        // (libc++'s `friend class basic_string_view` in `__wrap_iter`),
        // and substituting at that depth would leave the default unchanged.
        let head = self.head_template(t);
        let t = self.ast.first(t);
        let params = self.template_params(head);
        let depth = self.template_param_depth(head);
        for &p in params.iter().skip(ps.len()) {
            if self.ast.template_parm(p).is_none_or(|x| x.pack) {
                return;
            }
            let Some(def) = self.template_default(p) else { return };
            self.inst.sfinae.borrow_mut().push(crate::templ::stack::SfinaeState::default());
            let r = self.default_template_arg(t, depth, p, def, Loc::INVALID, ps);
            let failed = self.inst.sfinae.borrow_mut().pop().is_some_and(|s| s.errors > 0);
            match r {
                Ok(a) if !failed => ps.push(a),
                _ => return,
            }
        }
    }

    /// [temp.deduct.call]/4.3: `P` against the bases of `A`, each from
    /// the deductions `orig` made before `A` was tried; the successful
    /// base must be unique, its deductions then replace `d`. `None` when
    /// none matches. A class
    /// template specialization `A` not yet instantiated is instantiated
    /// first, at the deduction's location, so that its bases exist
    /// ([temp.inst]/2; clang's `isCompleteType` in
    /// `DeduceTemplateBases`): libstdc++'s `std::get` on a `tuple&` whose
    /// class nothing else completed (WP-682).
    fn deduce_from_bases(
        &mut self,
        name: kiln_ast::ids::TemplateNameId,
        args: ArgListId,
        a: QualType,
        orig: &DeducedArgs,
        d: &mut DeducedArgs,
        level: u32,
    ) -> Option<Tdk> {
        let TypeView::Record(r) = self.ast.types.canonical_view(a) else { return None };
        if self.ast.definition(r).is_none()
            && (self.ast.spec_info(r).is_some() || self.ast.instantiated_from(r).is_some())
        {
            let _ = self.instantiate_class(r, d.loc, false);
        }
        let mut seen = vec![self.ast.first(r)];
        let mut work = vec![r];
        let mut found: Option<(QualType, DeducedArgs)> = None;
        while let Some(c) = work.pop() {
            let Some(def) = self.ast.definition(c) else { continue };
            let bases: Vec<QualType> =
                self.ast.decls.bases(def, &self.ast.lists).map(|b| b.ty).collect();
            for b in bases {
                let TypeView::Record(br) = self.ast.types.canonical_view(b) else { continue };
                let first = self.ast.first(br);
                if seen.contains(&first) {
                    continue;
                }
                seen.push(first);
                let mut trial = orig.clone();
                if self.deduce_template_id(name, args, b, &mut trial, level) == Tdk::Success {
                    let cb = self.ast.types.canonical(b);
                    match &found {
                        Some((prev, _)) if *prev != cb => {
                            return Some(Tdk::MiscellaneousDeductionFailure);
                        }
                        Some(_) => {}
                        None => found = Some((cb, trial)),
                    }
                } else {
                    work.push(br);
                }
            }
        }
        let (_, trial) = found?;
        *d = trial;
        Some(Tdk::Success)
    }

    /// The clang-worded note for a failed deduction of `tmpl` (§10.4),
    /// or `None` for a failure without one.
    pub fn deduction_failure_note(
        &self,
        tmpl: DeclId,
        tdk: Tdk,
        d: &DeducedArgs,
        loc: Loc,
    ) -> Option<Diag> {
        let param_name = |i: u16| {
            self.template_params(tmpl)
                .get(i as usize)
                .map(|&p| self.decl_name(p))
                .unwrap_or_default()
        };
        match tdk {
            Tdk::Inconsistent => {
                let (x, y) = (d.info.first?, d.info.second?);
                // A pack: the kind of its elements (clang prints `<int>`).
                let first = match x.view() {
                    ArgView::Pack(l) => self.ast.types.args(l).first().copied().unwrap_or(x),
                    _ => x,
                };
                let kind = match first.view() {
                    ArgView::Type(_) | ArgView::Pack(_) => 0,
                    ArgView::Template(_) => 2,
                    _ => 1,
                };
                Some(
                    self.diag(DiagId::note_deduced_conflicting, loc)
                        .arg(kind)
                        .arg(param_name(d.info.param?))
                        .arg(self.deduced_arg_str(x))
                        .arg(self.deduced_arg_str(y)),
                )
            }
            Tdk::Incomplete => {
                let i = d.info.param.or_else(|| d.first_undeduced())?;
                Some(self.diag(DiagId::note_deduction_incomplete, loc).arg(param_name(i)))
            }
            // An expanded parameter pack deduced short ([temp.variadic]/5;
            // clang's `TDK_IncompletePack`): `first` is the deduced pack.
            Tdk::IncompletePack => {
                let pack = d.info.first?;
                let n = match pack.view() {
                    ArgView::Pack(l) => self.ast.types.args(l).len(),
                    _ => 0,
                };
                Some(
                    self.diag(DiagId::note_deduction_incomplete_pack, loc)
                        .arg(param_name(d.info.param?))
                        .arg(crate::expr::ordinal(n + 1))
                        .arg(self.deduced_arg_str(pack)),
                )
            }
            Tdk::Underqualified => {
                let (ArgView::Type(p), ArgView::Type(a)) =
                    (d.info.first?.view(), d.info.second?.view())
                else {
                    return None;
                };
                Some(
                    self.diag(DiagId::note_deduction_underqualified, loc)
                        .arg(param_name(d.info.param?))
                        .arg(self.ty_str(p))
                        .arg(self.ty_str(a)),
                )
            }
            Tdk::NonDeducedMismatch => {
                let (p, a) = (d.info.first?, d.info.second?);
                // `%diff{$ against $|types}`: two types are template-diffed.
                let (p, a) = match (p.view(), a.view()) {
                    (ArgView::Type(pt), ArgView::Type(at)) => self.ty_diff(pt, at),
                    // clang streams the two `TemplateName`s as written: an
                    // unqualified name bare, `N::A` with its qualifier; a
                    // template taken from a class (no sugar) fully qualified.
                    // clang 22 prints `A`'s template fully qualified always.
                    (ArgView::Template(pn), ArgView::Template(an)) => {
                        let a = if d.info.second_from_decl || self.clang_compat >= 22 {
                            self.arg_str(TemplateArg::template(an))
                        } else {
                            self.written_name_str(an)
                        };
                        (format!("'{}'", self.written_name_str(pn)), format!("'{a}'"))
                    }
                    _ => (format!("'{}'", self.arg_str(p)), format!("'{}'", self.arg_str(a))),
                };
                Some(self.diag(DiagId::note_deduction_non_deduced_mismatch, loc).arg(p).arg(a))
            }
            _ => None,
        }
    }

    /// One template argument as clang prints it in a note (types without
    /// quotes; used inside a quoted context).
    /// Template name `n` as written (clang's `TemplateName::print` with
    /// `Qualified::AsWritten` over a written name).
    fn written_name_str(&self, n: kiln_ast::ids::TemplateNameId) -> String {
        let names = crate::names::sema_names(self);
        let mut s = String::new();
        kiln_ast::types::write_template_name(&self.ast.types, &names, n, &mut s);
        s
    }

    pub(crate) fn arg_str(&self, a: TemplateArg) -> String {
        if let ArgView::Type(q) = a.view() {
            return self.ty_str(q);
        }
        let names = crate::names::sema_names(self);
        let mut s = String::new();
        kiln_ast::types::write_template_arg(&self.ast.types, &names, a, &mut s);
        s
    }

    /// Argument `a` as a binding in `[with T = int]` prints it: a pack
    /// in angle brackets (`U = <long, short>`, clang's
    /// `getTemplateArgumentBindingsText`).
    pub(crate) fn binding_arg_str(&self, a: TemplateArg) -> String {
        if matches!(a.view(), ArgView::Pack(_)) {
            return format!("<{}>", self.arg_str(a));
        }
        self.arg_str(a)
    }

    /// A conflicting deduction: a type in quotes with its `aka`, a value
    /// or template bare.
    fn deduced_arg_str(&self, a: TemplateArg) -> String {
        match a.view() {
            ArgView::Type(q) => self.ty_arg(q),
            ArgView::Pack(l) => {
                let v: Vec<String> =
                    self.ast.types.args(l).iter().map(|&x| self.arg_str(x)).collect();
                format!("<{}>", v.join(", "))
            }
            _ => self.arg_str(a),
        }
    }
}
