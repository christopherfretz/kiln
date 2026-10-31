//! Template argument deduction ([temp.deduct]; spec/sema-templates.md
//! §6). WP-070b lands the deduction core (§6.2): [`Sema::deduce`] over
//! types and [`Sema::deduce_args`] over template argument lists, shared
//! by partial-specialization matching (§4.4) and partial ordering
//! (§6.7). Deduction from a call (§6.1) is WP-071; packs are WP-072.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::ArgListId;
use kiln_ast::types::{
    ArgView, ExcSpec, ExprOperand, NoexceptValue, TemplateArg, TypeFlags, TypeView,
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
    /// A function type `A` may differ in `noexcept` ([temp.deduct.call]/4.2).
    pub const ALLOW_COMPATIBLE_FUNCTION_TYPE: TdfFlags = TdfFlags(128);

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
}

/// The arguments deduced so far for the parameters of one template
/// (one depth, §6.2). An undeduced parameter holds [`TemplateArg::NULL`].
#[derive(Clone, Debug, PartialEq, Eq)]
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
}

impl DeducedArgs {
    /// Nothing deduced yet for `n` parameters at `depth`.
    pub fn new(depth: u16, n: usize) -> Self {
        DeducedArgs {
            depth,
            args: vec![TemplateArg::NULL; n],
            info: DeductionInfo::default(),
            pack_cur: None,
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
        let as_ = self.flatten_args(as_);
        for (k, &p) in ps.iter().enumerate() {
            if let Some(pat) = self.arg_expansion_pattern(p) {
                if k + 1 != ps.len() {
                    // Not the last: a non-deduced context ([temp.deduct.type]/9).
                    return Tdk::Success;
                }
                let rest = as_.get(k..).unwrap_or(&[]);
                if rest.iter().any(|&a| self.is_pack_arg(a)) {
                    // `A` has an expansion too: equal only as written.
                    let same = rest.len() == 1
                        && self.ast.types.canonical_arg(p) == self.ast.types.canonical_arg(rest[0]);
                    return if same { Tdk::Success } else { Tdk::MiscellaneousDeductionFailure };
                }
                return self.deduce_pack_elements(rest.len(), d, |s, i, d| {
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

    /// The parameter types `ps` of a function type against `as_`: a
    /// trailing pack expansion matches the remaining types.
    fn deduce_type_list(
        &mut self,
        ps: &[QualType],
        as_: &[QualType],
        d: &mut DeducedArgs,
        level: u32,
    ) -> Tdk {
        for (k, &p) in ps.iter().enumerate() {
            if let TypeView::PackExpansion { pattern, .. } = self.ast.types.view(p.ty()) {
                if k + 1 != ps.len() {
                    return Tdk::Success;
                }
                let rest = as_.get(k..).unwrap_or(&[]).to_vec();
                return self.deduce_pack_elements(rest.len(), d, |s, i, d| {
                    s.deduce_ty(pattern, rest[i], TdfFlags::NONE, d, level)
                });
            }
            let Some(&a) = as_.get(k) else { return Tdk::MiscellaneousDeductionFailure };
            let r = self.deduce_ty(p, a, TdfFlags::NONE, d, level);
            if r != Tdk::Success {
                return r;
            }
        }
        if as_.len() == ps.len() { Tdk::Success } else { Tdk::MiscellaneousDeductionFailure }
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
        self.deduce_pack_elements_from(0, n, d, f)
    }

    /// The length of the explicitly given pack among `params`, if any
    /// ([temp.arg.explicit]/9).
    pub(crate) fn explicit_pack_len(&self, params: &[DeclId], d: &DeducedArgs) -> usize {
        params
            .iter()
            .enumerate()
            .filter(|&(_, &p)| self.ast.template_parm(p).is_some_and(|x| x.pack))
            .find_map(|(i, _)| match d.args.get(i).map(TemplateArg::view) {
                Some(ArgView::Pack(l)) => Some(self.ast.types.args(l).len()),
                _ => None,
            })
            .unwrap_or(0)
    }

    /// [`Self::deduce_pack_elements`] where elements `0..skip` were given
    /// explicitly: only `skip..n` are deduced, appended to the given
    /// prefix of each pack.
    pub(crate) fn deduce_pack_elements_from(
        &mut self,
        skip: usize,
        n: usize,
        d: &mut DeducedArgs,
        mut f: impl FnMut(&mut Self, usize, &mut DeducedArgs) -> Tdk,
    ) -> Tdk {
        let saved = d.pack_cur.take();
        let mut per: Vec<(u16, Vec<TemplateArg>)> = Vec::new();
        let skip = skip.min(n);
        for i in skip..n {
            d.pack_cur = Some(Vec::new());
            let r = f(self, i, d);
            let cur = d.pack_cur.take().unwrap_or_default();
            if r != Tdk::Success {
                d.pack_cur = saved;
                return r;
            }
            for (idx, a) in cur {
                if let Some(x) = per.iter_mut().find(|x| x.0 == idx) {
                    x.1.push(a);
                } else {
                    let mut v = Vec::with_capacity(n);
                    if let Some(ArgView::Pack(l)) = d.args.get(idx as usize).map(TemplateArg::view)
                    {
                        v.extend(self.ast.types.args(l).iter().take(skip));
                    }
                    v.push(a);
                    per.push((idx, v));
                }
            }
        }
        d.pack_cur = saved;
        for (idx, v) in per {
            if v.len() != n {
                continue;
            }
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
    fn arg_expansion_pattern(&mut self, a: TemplateArg) -> Option<TemplateArg> {
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
            ArgView::Type(q) => matches!(t.canonical_view(q), TypeView::PackExpansion { .. }),
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
                self.templ_unimplemented("template argument deduction for a pack", loc);
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
            return Tdk::Success;
        };
        if parm.depth != d.depth {
            return Tdk::Success;
        }
        match a.view() {
            ArgView::Value { .. } | ArgView::Expr { .. } | ArgView::Profile(_) => {
                self.record_deduction(parm.index, parm.pack, a, d)
            }
            _ => {
                d.info.first = Some(p);
                d.info.second = Some(a);
                Tdk::NonDeducedMismatch
            }
        }
    }

    /// The index of the non-type template parameter of depth `depth`
    /// that argument `a` names as a bare (possibly converted) reference.
    pub(crate) fn bare_nttp_index(&self, a: TemplateArg, depth: u16) -> Option<u16> {
        let ArgView::Expr { expr, .. } = a.view() else { return None };
        self.nttp_of_expr(expr).filter(|r| r.depth == depth && !r.pack).map(|r| r.index)
    }

    /// The non-type template parameter expression `e` names (through
    /// implicit conversions and parentheses).
    fn nttp_of_expr(&self, mut e: ExprId) -> Option<ParmRef> {
        let ex = &self.ast.exprs;
        loop {
            match ex.get(e).kind() {
                ExprKind::ImplicitCastExpr | ExprKind::ParenExpr | ExprKind::ConstantExpr => {
                    e = ex.operand(e);
                }
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
        let p = t.canonical(p);
        let a = t.canonical(a);
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
            return if same { Tdk::Success } else { mismatch(d) };
        }
        let pv = t.view(p.ty());
        // `cv T` ([temp.deduct.type]/8: the forms `T` and `cv T`).
        if let TypeView::CanonTemplateTypeParm { depth, index, pack } = pv {
            if depth != u32::from(d.depth) {
                return Tdk::Success;
            }
            let (pq, aq) = (p.quals_bits(), a.quals_bits());
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
            let deduced = QualType::new(a.ty(), aq & !pq);
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
                let sub =
                    flags.intersect(TdfFlags::IGNORE_QUALIFIERS.union(TdfFlags::DERIVED_CLASS));
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
                let sub = flags.intersect(TdfFlags::IGNORE_QUALIFIERS);
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
            TypeView::Function(pf) => {
                let TypeView::Function(af) = av else { return mismatch(d) };
                let t = &self.ast.types;
                let p_pack = t
                    .params(&pf)
                    .last()
                    .is_some_and(|&x| matches!(t.view(x.ty()), TypeView::PackExpansion { .. }));
                if (pf.num_params() != af.num_params() && !p_pack)
                    || pf.variadic() != af.variadic()
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
                let r = self.deduce_type_list(&pps, &aps, d, next);
                if r != Tdk::Success {
                    return r;
                }
                self.deduce_noexcept(pexc, aexc, flags, d, p, a)
            }
            TypeView::TemplateSpecialization { name, args, .. } => {
                let r = self.deduce_template_id(name, args, a, d, next);
                if r == Tdk::Success || !flags.contains(TdfFlags::DERIVED_CLASS) {
                    if r == Tdk::NonDeducedMismatch {
                        return mismatch(d);
                    }
                    return r;
                }
                // No base matches: the result for `A` itself (clang's
                // `DeduceTemplateSpecArguments`).
                match self.deduce_from_bases(name, args, a, d, next) {
                    Some(b) => b,
                    None if r == Tdk::NonDeducedMismatch => mismatch(d),
                    None => r,
                }
            }
            TypeView::Complex(pe) => {
                let TypeView::Complex(ae) = av else { return mismatch(d) };
                self.deduce_ty(pe, ae, TdfFlags::NONE, d, next)
            }
            TypeView::Atomic(pe) => {
                let TypeView::Atomic(ae) = av else { return mismatch(d) };
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
            TypeView::PackExpansion { .. } => {
                self.templ_unimplemented("template argument deduction for a pack", Loc::INVALID);
                Tdk::MiscellaneousDeductionFailure
            }
            // Non-deduced contexts ([temp.deduct.type]/5): the
            // qualifier of a qualified-id, `decltype`, pack indexing,
            // type transforms.
            TypeView::DependentName { .. }
            | TypeView::DependentTemplateSpecialization { .. }
            | TypeView::DependentDecltype(_)
            | TypeView::Decltype { .. }
            | TypeView::PackIndexing { .. }
            | TypeView::TypeOf { .. }
            | TypeView::UnaryTransform { .. }
            | TypeView::DependentBitInt { .. }
            | TypeView::Splice { .. } => Tdk::Success,
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
    fn deduce_array_bound(
        &mut self,
        size: ExprOperand,
        asize: Option<Result<u64, ExprOperand>>,
        d: &mut DeducedArgs,
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
                let ty = self.nttp_param_type(e).unwrap_or_else(|| self.size_type());
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
        while matches!(
            ex.get(e).kind(),
            ExprKind::ImplicitCastExpr | ExprKind::ParenExpr | ExprKind::ConstantExpr
        ) {
            e = ex.operand(e);
        }
        let ty = self.ast.decls.get(ex.decl(e)).ty()?;
        (!self.ast.types.is_dependent(ty)).then_some(ty)
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
        if pexc.is_nothrow() == a_nothrow
            || (flags.contains(TdfFlags::ALLOW_COMPATIBLE_FUNCTION_TYPE) && a_nothrow)
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
            return Tdk::NonDeducedMismatch;
        };
        let aname = match aname {
            Some(n) => n,
            None => self.ast.types.template_name_of(atmpl),
        };
        let r = self.deduce_template_name(name, aname, d);
        if r != Tdk::Success {
            return r;
        }
        let ps = self.ast.types.args(args).to_vec();
        let as_ = self.ast.types.args(aargs).to_vec();
        self.deduce_arg_list(&ps, &as_, true, d, level, Loc::INVALID)
    }

    /// [temp.deduct.call]/4.3: `P` against the bases of `A`; the
    /// successful base must be unique. `None` when none matches.
    fn deduce_from_bases(
        &mut self,
        name: kiln_ast::ids::TemplateNameId,
        args: ArgListId,
        a: QualType,
        d: &mut DeducedArgs,
        level: u32,
    ) -> Option<Tdk> {
        let TypeView::Record(r) = self.ast.types.canonical_view(a) else { return None };
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
                let mut trial = d.clone();
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
                Some(
                    self.diag(DiagId::note_deduction_non_deduced_mismatch, loc)
                        .arg(self.arg_str(p))
                        .arg(self.arg_str(a)),
                )
            }
            _ => None,
        }
    }

    /// One template argument as clang prints it in a note (types without
    /// quotes; used inside a quoted context).
    pub(crate) fn arg_str(&self, a: TemplateArg) -> String {
        if let ArgView::Type(q) = a.view() {
            return self.ty_str(q);
        }
        let names = crate::names::SemaNames::new(self);
        let mut s = String::new();
        kiln_ast::types::write_template_arg(&self.ast.types, &names, a, &mut s);
        s
    }

    /// A conflicting deduction: a type in quotes, a value or template bare.
    fn deduced_arg_str(&self, a: TemplateArg) -> String {
        match a.view() {
            ArgView::Type(q) => format!("'{}'", self.ty_str(q)),
            ArgView::Pack(l) => {
                let v: Vec<String> =
                    self.ast.types.args(l).iter().map(|&x| self.arg_str(x)).collect();
                format!("<{}>", v.join(", "))
            }
            _ => self.arg_str(a),
        }
    }
}
