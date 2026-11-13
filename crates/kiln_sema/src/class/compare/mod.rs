//! Three-way comparison and defaulted comparison operators
//! ([expr.spaceship], [cmp.categories], [class.compare];
//! spec/sema-class.md §4.12, spec/sema-overload.md §10.6).
//!
//! - The comparison category types `std::partial_ordering`,
//!   `std::weak_ordering` and `std::strong_ordering` are looked up and
//!   validated on first use (clang's `CheckComparisonCategoryType`) and
//!   recorded in [`kiln_ast::Ast::cmp_categories`] for the Oven and
//!   codegen.
//! - The built-in `<=>` on scalars ([expr.spaceship]/4–8).

use kiln_ast::class::{CmpCategory, CmpCategoryInfo};
use kiln_ast::decl::{DeclKind, DefinitionBits, NamespaceData, VarEvalState};
use kiln_ast::expr::{BinaryOp, CastKind};
use kiln_ast::types::{BuiltinType, Nns, TagKeyword};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use crate::Sema;
use crate::conv::{self, ArithConvKind, ConvCtx, Narrowing};
use crate::lookup::LookupKind;

mod analyze;
mod default;
mod synth;
mod three_way;

pub(crate) use default::CmpKind;

/// Why a comparison category type is needed (clang's
/// `ComparisonCategoryUsage`), selecting the wording of
/// `sema_cmp_category_not_found`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CmpUsage {
    /// A built-in `<=>` in an expression.
    Builtin = 0,
    /// The deduced return type of a defaulted `operator<=>`.
    Defaulted = 1,
}

impl Sema<'_, '_> {
    /// The type of class `r` as clang's bare `RecordType` of it prints:
    /// `'std::strong_ordering'`, `'ns::Outer::B'`, which kiln spells as
    /// `Elaborated` sugar with the enclosing non-inline namespaces and
    /// (each spelled so in turn) classes as the qualifier (spec/types.md
    /// §15 R4). A local class stays bare.
    pub(crate) fn scoped_class_type(&mut self, r: DeclId) -> QualType {
        let t = self.class_type(r);
        let mut chain = Vec::new();
        let mut c = self.ast.decls.get(r).ctx();
        let mut class = None;
        while let Some(d) = c
            && d != self.ast.tu
        {
            if self.kind_of(d) == DeclKind::CXXRecord && chain.is_empty() {
                // The enclosing class's own scoped type is the prefix
                // (WP-1158: the object of `p->m` for a member `m` of a
                // nested base class prints `'ns::Outer::B *'`).
                class = Some(d);
                break;
            }
            if self.kind_of(d) != DeclKind::Namespace {
                return t;
            }
            let first = self.ast.first(d);
            if !self.ast.decls.namespace(first).is_some_and(NamespaceData::is_inline) {
                chain.push(first);
            }
            c = self.ast.decls.get(d).ctx();
        }
        if let Some(d) = class {
            let outer = self.scoped_class_type(d);
            let nns = self.ast.types.nns(Nns::Type { prefix: None, ty: outer, template_kw: false });
            return self.ast.types.elaborated(TagKeyword::None, Some(nns), t);
        }
        if chain.is_empty() {
            return t;
        }
        let mut nns = None;
        for ns in chain.into_iter().rev() {
            nns = Some(self.ast.types.nns(Nns::Namespace { prefix: nns, ns }));
        }
        self.ast.types.elaborated(TagKeyword::None, nns, t)
    }

    /// The comparison category class `std::<kind>` (clang's
    /// `CheckComparisonCategoryType`): looked up in namespace `std`,
    /// required complete and checked to have the form kiln's evaluation
    /// relies on (trivially copyable, no non-empty bases, one integral or
    /// enumeration data member, and each of the category's values a
    /// static data member usable in constant expressions whose value has
    /// that form). The result is recorded once valid; a failure is
    /// diagnosed at `loc` on every use, as clang does.
    pub(crate) fn cmp_category_type(
        &mut self,
        kind: CmpCategory,
        loc: Loc,
        usage: CmpUsage,
    ) -> Option<QualType> {
        if let Some(info) = self.ast.cmp_categories.get(kind) {
            let r = info.record;
            return Some(self.scoped_class_type(r));
        }
        let Some(rec) = self.std_record(kind.name()) else {
            let msg = self
                .diag(DiagId::sema_cmp_category_not_found, loc)
                .arg(format!("std::{}", kind.name()))
                .arg(usage as u8);
            self.emit(msg);
            return None;
        };
        let ty = self.class_type(rec);
        // Diagnostics name it as clang prints a bare RecordType.
        let shown = self.scoped_class_type(rec);
        if !self.require_complete(ty, loc) {
            let msg = self.diag(DiagId::sema_incomplete_type_required, loc).arg(self.ty_arg(shown));
            self.emit(msg);
            return None;
        }
        let def = self.class_def(rec);
        let type_form = |s: &mut Self, which: u8| {
            let msg =
                s.diag(DiagId::sema_std_compare_type_form, loc).arg(s.ty_arg(shown)).arg(which);
            s.emit(msg);
        };
        if !self.defn_bits(def).contains(DefinitionBits::TRIVIALLY_COPYABLE) {
            type_form(self, 0);
            return None;
        }
        let bases: Vec<_> = self.ast.decls.bases(def, &self.ast.lists).map(|b| b.ty).collect();
        let empty_base = |s: &Self, b: QualType| {
            s.ast.types.as_record(b).is_some_and(|d| s.defn_bits(d).contains(DefinitionBits::EMPTY))
        };
        let fields: Vec<DeclId> =
            self.ast.decls.members(def).filter(|&f| self.kind_of(f) == DeclKind::Field).collect();
        let field_ok = |s: &Self, f: DeclId| {
            let t = s.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            s.is_integral_or_enum(t)
        };
        if !bases.iter().all(|&b| empty_base(self, b))
            || fields.len() != 1
            || !field_ok(self, fields[0])
        {
            type_form(self, 1);
            return None;
        }
        let field = fields[0];
        let mut info =
            CmpCategoryInfo { kind, record: def, field, values: [None; 5], ints: [0; 5] };
        let mut used = Vec::with_capacity(4);
        for &r in kind.results() {
            let name = self.intern(r.name());
            let member_form = |s: &mut Self, missing: bool| {
                let msg = s
                    .diag(DiagId::sema_std_compare_member_form, loc)
                    .arg(s.ty_arg(shown))
                    .arg(r.name())
                    .arg(u8::from(missing));
                s.emit(msg);
            };
            let found = self.lookup_qualified(def, name, LookupKind::Ordinary).single();
            let Some(var) = found.filter(|&d| self.kind_of(d) == DeclKind::Var) else {
                member_form(self, true);
                return None;
            };
            let Some(holder) = self.cmp_value_holder(def, var) else {
                member_form(self, false);
                return None;
            };
            let Some(v) = self.cmp_value_int(holder, field) else {
                type_form(self, 1);
                return None;
            };
            info.values[r as usize] = Some(var);
            info.ints[r as usize] = v;
            used.push(var);
        }
        for var in used {
            self.mark_used_at(var, loc);
        }
        self.ast.cmp_categories.set(info);
        Some(self.scoped_class_type(rec))
    }

    /// The class `std::<name>` (through inline namespaces), if declared.
    fn std_record(&mut self, name: &str) -> Option<DeclId> {
        let tu = self.ast.tu;
        let std = self.intern("std");
        let ns = self
            .lookup_in(tu, std, LookupKind::Nested)
            .single()
            .filter(|&d| self.kind_of(d) == DeclKind::Namespace)?;
        let name = self.intern(name);
        let d = self.lookup_qualified(ns, name, LookupKind::Tag).single()?;
        self.ast.types.as_record(self.tag_type(d))
    }

    /// The declaration holding the initializer of the comparison
    /// category value `var`, if `var` is a static data member of `class`
    /// usable in constant expressions ([expr.const]/4: `constexpr` on
    /// some declaration, constant-initialized).
    fn cmp_value_holder(&mut self, class: DeclId, var: DeclId) -> Option<DeclId> {
        let first = self.ast.first(class);
        let in_class =
            self.ast.decls.get(var).ctx().is_some_and(|c| {
                self.kind_of(c) == DeclKind::CXXRecord && self.ast.first(c) == first
            });
        if !in_class {
            return None;
        }
        let redecls: Vec<DeclId> = self.ast.redecls_of(var).collect();
        let constexpr =
            redecls.iter().any(|&d| self.ast.decls.var(d).is_some_and(|v| v.spec.is_constexpr()));
        let holder = redecls
            .into_iter()
            .find(|&d| self.ast.decls.var(d).is_some_and(|v| v.init.is_some()))?;
        (constexpr && self.is_constant_initialized(holder)).then_some(holder)
    }

    /// The value of the data member `field` (at offset 0) of the
    /// constant-initialized category value `holder` (clang's
    /// `ValueInfo::hasValidIntValue`).
    /// A declaration image does not carry an aggregate value (spec/modules.md
    /// §11.11): an imported holder's is folded from its initializer here.
    fn cmp_value_int(&mut self, holder: DeclId, field: DeclId) -> Option<i64> {
        let data = self.ast.decls.var(holder)?;
        let v = match (data.eval.state(), data.init) {
            (VarEvalState::Value(v), _) => v,
            (VarEvalState::NotEvaluated, Some(init)) => self.try_fold(init)?,
            _ => return None,
        };
        let img = self.ast.consts.aggregate(v)?;
        let fty = self.ast.decls.get(field).ty()?;
        let width = conv::int_width(self, fty)?;
        let n = usize::try_from(width.div_ceil(8)).ok()?;
        if n == 0 || n > 8 || img.bytes.len() < n {
            return None;
        }
        let mut raw = [0u8; 8];
        raw[..n].copy_from_slice(&img.bytes[..n]);
        let u = u64::from_le_bytes(raw);
        let bits = u32::try_from(n * 8).ok()?;
        Some(if conv::int_signed(self, fty) && bits < 64 {
            let shift = 64 - bits;
            (i64::from_ne_bytes(u.to_ne_bytes()) << shift) >> shift
        } else {
            i64::from_ne_bytes(u.to_ne_bytes())
        })
    }

    /// The comparison category of a built-in `<=>` on operands of the
    /// (composite) type `t` (clang's `getComparisonCategoryForBuiltinCmp`).
    pub(crate) fn builtin_cmp_category(&self, t: QualType) -> Option<CmpCategory> {
        if self.is_integral_or_enum(t) {
            Some(CmpCategory::Strong)
        } else if self.ast.types.is_floating(t) {
            Some(CmpCategory::Partial)
        } else if self.ast.types.is_pointer(t)
            && self.ast.types.pointee(t).is_some_and(|p| !self.ast.types.is_function(p))
        {
            Some(CmpCategory::Strong)
        } else {
            None
        }
    }

    /// The built-in `<=>` on non-dependent operands ([expr.spaceship];
    /// clang's `CheckCompareOperands` for `BO_Cmp`).
    pub(crate) fn builtin_three_way(&mut self, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        self.diagnose_self_comparison(BinaryOp::Cmp, l, r, loc);
        let any_ptr = |s: &Self, e: ExprId| {
            let t = s.ety(e);
            s.ast.types.is_pointer(t) || s.is_member_pointer(t)
        };
        // [expr.spaceship]/6: array-to-pointer and function-to-pointer
        // conversions only when an operand has pointer type.
        let (l, r) = if any_ptr(self, l) || any_ptr(self, r) {
            (conv::decay(self, l), conv::decay(self, r))
        } else {
            (conv::lvalue_to_rvalue(self, l), conv::lvalue_to_rvalue(self, r))
        };
        let (lt, rt) = (self.ety(l), self.ety(r));
        if self.is_error_type(lt) || self.is_error_type(rt) {
            return self.recover_err(&[l, r], loc);
        }
        let arith_or_enum = |s: &Self, t: QualType| {
            s.ast.types.is_arithmetic(t) || s.ast.types.as_enum(t).is_some()
        };
        if arith_or_enum(self, lt) && arith_or_enum(self, rt) {
            return self.arith_three_way(l, r, loc);
        }
        let (lp, rp) = (self.ast.types.is_pointer(lt), self.ast.types.is_pointer(rt));
        if lp && rp {
            let Some((l2, r2, ct)) = self.composite_pointer(l, r) else {
                let msg = self
                    .diag(DiagId::sema_comparison_distinct_pointers, loc)
                    .arg(self.ty_arg(lt))
                    .arg(self.ty_arg(rt));
                self.emit(msg);
                return self.recover_err(&[l, r], loc);
            };
            let Some(cat) = self.builtin_cmp_category(ct) else {
                return self.invalid_operands(l, r, loc);
            };
            let Some(ty) = self.cmp_category_type(cat, loc, CmpUsage::Builtin) else {
                return self.recover_err(&[l2, r2], loc);
            };
            return self.builder().binary(BinaryOp::Cmp, l2, r2, ty, loc);
        }
        let int = |s: &Self, t: QualType| s.ast.types.is_integral(t);
        if (lp && int(self, rt)) || (rp && int(self, lt)) {
            let null = if lp {
                conv::is_null_pointer_constant(self, r)
            } else {
                conv::is_null_pointer_constant(self, l)
            };
            let id = if null {
                DiagId::sema_ordered_pointer_zero
            } else {
                DiagId::sema_comparison_pointer_integer
            };
            let msg = self.diag(id, loc).arg(self.ty_arg(lt)).arg(self.ty_arg(rt));
            self.emit(msg);
            return self.recover_err(&[l, r], loc);
        }
        self.invalid_operands(l, r, loc)
    }

    /// clang's `checkArithmeticOrEnumeralThreeWayCompare`
    /// ([expr.spaceship]/3–5).
    fn arith_three_way(&mut self, l: ExprId, r: ExprId, loc: Loc) -> ExprId {
        let (ls, rs) = (self.ignore_paren_imp_casts(l), self.ignore_paren_imp_casts(r));
        let (lst, rst) = (self.ety(ls), self.ety(rs));
        let is_bool = |s: &Self, t: QualType| s.ast.types.builtin(t) == Some(BuiltinType::Bool);
        // /3: `bool` only with `bool`.
        if is_bool(self, lst) != is_bool(self, rst) {
            return self.invalid_operands(ls, rs, loc);
        }
        let (le, re) =
            (self.ast.types.as_enum(lst).is_some(), self.ast.types.as_enum(rst).is_some());
        let (mut l, mut r) = (l, r);
        if le != re {
            let other = if le { rst } else { lst };
            if self.ast.types.is_floating(other) {
                return self.invalid_operands(ls, rs, loc);
            }
        } else if le && re {
            // /5: the same enumeration, compared as its underlying type
            // (promoted, as clang does to avoid an `IntegralCast` to
            // `bool`).
            if !self.ast.types.same_unqualified(lst, rst) {
                return self.invalid_operands(l, r, loc);
            }
            let Some(u) = conv::rank::enum_data(self, lst).and_then(|e| e.int_type) else {
                return self.invalid_operands(l, r, loc);
            };
            let it = conv::promoted_type(self, u, None).unwrap_or(u);
            l = conv::cast_to(self, CastKind::IntegralCast, l, it, None);
            r = conv::cast_to(self, CastKind::IntegralCast, r, it, None);
        }
        // /4: the usual arithmetic conversions.
        let (l2, r2, ty) =
            match conv::usual_arithmetic_conversions_kind(self, l, r, ArithConvKind::Comparison) {
                Ok(x) => x,
                Err(err) => return self.conv_failed(&err, l, r, loc),
            };
        self.report_arith_warning(loc);
        let Some(cat) = self.builtin_cmp_category(ty) else {
            return self.invalid_operands(l2, r2, loc);
        };
        let narrowed_l = self.three_way_narrowing(l, ty);
        let narrowed_r = self.three_way_narrowing(r, ty);
        if narrowed_l || narrowed_r {
            return self.recover_err(&[l2, r2], loc);
        }
        let Some(cty) = self.cmp_category_type(cat, loc, CmpUsage::Builtin) else {
            return self.recover_err(&[l2, r2], loc);
        };
        self.builder().binary(BinaryOp::Cmp, l2, r2, cty, loc)
    }

    /// clang's `checkThreeWayNarrowingConversion`: the conversion of the
    /// operand `e` to the common type `to` must not narrow
    /// ([expr.spaceship]/4); diagnosed at the operand.
    fn three_way_narrowing(&mut self, e: ExprId, to: QualType) -> bool {
        let Some(scs) = conv::standard_conversion(self, e, to, ConvCtx::Initialization) else {
            return false;
        };
        let from = conv::strip_cv(self, self.ety(e));
        // "other than from an integral type to a floating-point type"
        // (clang's `IgnoreFloatToIntegralConversion`, which despite its
        // name exempts the integral-to-floating case).
        let floating =
            |s: &Self, q: QualType| s.ast.types.builtin(q).is_some_and(BuiltinType::is_floating);
        if conv::is_integral_or_unscoped_enum(self, from) && floating(self, to) {
            return false;
        }
        let value = self.try_fold(e);
        let n = conv::narrowing_kind(self, &scs, e, value);
        let at = self.begin_loc(e);
        let msg = match n {
            // Floating-to-integral (`Type`): the usual arithmetic
            // conversions never make one.
            Narrowing::NotNarrowing | Narrowing::Dependent | Narrowing::Type => return false,
            Narrowing::Variable => self
                .diag(DiagId::sema_spaceship_narrowing_var, at)
                .arg(self.ty_arg(from))
                .arg(self.ty_arg(to)),
            Narrowing::Constant { value } => self
                .diag(DiagId::sema_spaceship_narrowing_const, at)
                .arg(value)
                .arg(self.ty_arg(to)),
        };
        self.emit(msg);
        true
    }

    /// The comparison category of class type `t`, if it is one (clang's `ComparisonCategories::lookupInfoForType`).
    pub(crate) fn cmp_category_of(&self, t: QualType) -> Option<CmpCategory> {
        let d = self.ast.types.as_record(t)?;
        let def = self.class_def(d);
        if let Some(i) = self.ast.cmp_categories.for_record(def) {
            return Some(i.kind);
        }
        // Not yet looked up: a class of namespace `std` (through inline
        // namespaces) named as a category is one (clang
        // `ComparisonCategories::lookupInfoForType`); its form is checked
        // when the category is used ([`Sema::cmp_category_type`]).
        let name = self.ast.decls.get(def).name().as_ident()?;
        let name = self.ast.symbols().as_str(name);
        let kind = CmpCategory::ALL.into_iter().find(|k| k.name() == name)?;
        let mut c = self.ast.decls.get(def).ctx();
        while let Some(ns) = c
            && self.kind_of(ns) == DeclKind::Namespace
        {
            let first = self.ast.first(ns);
            if !self.ast.decls.namespace(first).is_some_and(NamespaceData::is_inline) {
                let parent = self.ast.decls.get(ns).ctx();
                let is_std = parent == Some(self.ast.tu)
                    && self
                        .ast
                        .decls
                        .get(ns)
                        .name()
                        .as_ident()
                        .is_some_and(|n| self.ast.symbols().as_str(n) == "std");
                return is_std.then_some(kind);
            }
            c = self.ast.decls.get(ns).ctx();
        }
        None
    }
}

/// The defaulted-comparison state of [`Sema`] (spec/sema-class.md §8.3),
/// kept in [`crate::class::SpecialState`].
#[derive(Debug, Default, kiln_snap::Snap)]
pub(crate) struct CompareState {
    /// The defaulted comparison whose subobject comparisons are being
    /// analyzed or synthesized, with the context they are resolved in:
    /// `(first declaration, context)`. It is excluded from their
    /// candidate sets (clang's `DefaultedFn`), and their unqualified
    /// operator lookups are the ones captured at its `= default`.
    pub exclude: Option<(DeclId, DeclId)>,
    /// The unqualified operator lookups captured at each defaulted
    /// comparison's `= default` (clang's `UnresolvedSet` of
    /// `DefaultedOrDeletedFunctionInfo`), keyed by its first declaration:
    /// `(target decl, found decl)` pairs.
    pub lookups: FxHashMap<DeclId, Vec<(DeclId, DeclId)>>,
    /// Where each defaulted comparison's `= default` was written, keyed by
    /// its first declaration: the declaration defaulted there and the
    /// location (the synthesized body is placed at its last character).
    pub default_loc: FxHashMap<DeclId, (DeclId, Loc)>,
    /// Defaulted comparisons found ill-formed by their declaration checks
    /// (clang `setInvalidDecl`; kept out of [`kiln_ast::decl::DeclFlags::INVALID`]
    /// because clang still considers them as candidates). First
    /// declarations.
    pub invalid: kiln_source::fx::FxHashSet<DeclId>,
    /// Defaulted comparisons whose implicit definition is being built,
    /// against recursion. First declarations.
    pub defining: Vec<DeclId>,
    /// The implicit `operator==` declared for each explicitly defaulted
    /// `operator<=>` ([class.compare.default]/4), keyed by the `<=>`: a
    /// class template specialization's implicit `==` is linked to the one
    /// of its pattern (clang instantiates it from the pattern's implicit
    /// declaration).
    pub implicit_eq: FxHashMap<DeclId, DeclId>,
}

impl Sema<'_, '_> {
    /// The defaulted comparison whose body is being analyzed or
    /// synthesized, when the current context is the one it is resolved in.
    pub(crate) fn cmp_active(&self) -> Option<DeclId> {
        self.special.cmp.exclude.filter(|&(_, c)| c == self.ctx).map(|(f, _)| f)
    }
}
