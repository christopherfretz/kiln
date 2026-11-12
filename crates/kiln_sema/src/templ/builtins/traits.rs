//! The type-trait predicates (spec/sema-templates.md §9.1, §9.4):
//! `__is_*` / `__has_*` (`TypeTraitExpr`), `__array_rank` /
//! `__array_extent` (`ArrayTypeTraitExpr`) and `__is_lvalue_expr` /
//! `__is_rvalue_expr` (`ExpressionTraitExpr`). Each is a `bool` (`int`
//! in C, clang `getLogicalOperationType`; or `size_t`) prvalue over unevaluated operands; a dependent operand
//! gives a value-dependent node that substitution rebuilds through
//! [`Sema::build_type_trait`]. The evaluators follow clang 19's
//! `EvaluateUnaryTypeTrait` / `EvaluateBinaryTypeTrait`, including its
//! extensions (complex types count as arithmetic and scalar, a
//! zero-length array is not an array). The traits clang 20-22 added
//! follow clang 22.1 (`cxx26`).

use kiln_ast::decl::{Access, DefinitionBits};
use kiln_ast::expr::{BinaryOp, ExprDeps, ExprKind, TypeTrait, ValueCategory};
use kiln_ast::types::{ExcSpec, Quals, TypeFlags, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::table::{self, Arity, Complete, Status};
use crate::Sema;
use crate::class::{Selected, SpecialKind};
use crate::conv::rank::builtin_signed;
use crate::target;
use crate::templ::stack::FrameKind;

mod cxx26;
mod library;
pub(crate) use library::LibFailure;
mod explain;
mod special;

/// Why a quietly evaluated trait failed ([`Sema::quiet_trait`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TraitFailure {
    /// An operand that must be complete is not (the type named).
    Incomplete(QualType),
    /// A transformation's operand is outside its domain
    /// (`make_signed<bool>`, `underlying_type<int>`): the type named.
    BadOperand(QualType),
    /// The trait is not implemented ([`table::trait_status`]).
    Unimplemented,
}

/// The engine's per-TU state: whether trait diagnostics are being
/// recorded instead of emitted, and the first failure recorded so far.
#[derive(Debug, Default)]
pub(crate) struct TraitState {
    pub(crate) quiet: bool,
    pub(crate) failure: Option<TraitFailure>,
}

impl TraitState {
    /// Records `f` (the first failure wins) when quiet; false otherwise.
    pub(crate) fn record(&mut self, f: TraitFailure) -> bool {
        if self.quiet {
            self.failure.get_or_insert(f);
        }
        self.quiet
    }
}

impl Sema<'_, '_> {
    /// `__trait(T, ...)` as parsed at `loc` (the keyword) up to `rparen`:
    /// clang's `ActOnTypeTrait`. Warns about the deprecated `__has_*`
    /// spellings ([depr]-style `-Wdeprecated-builtins`), then
    /// [`Sema::build_type_trait`]. `args` are the type operands with
    /// their locations (pack expansions included).
    pub fn act_type_trait(
        &mut self,
        t: TypeTrait,
        args: &[(QualType, Loc)],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        // clang 22 also deprecates `__is_trivially_relocatable` in favour
        // of the P2786 `__builtin_is_cpp_trivially_relocatable`, and
        // `__reference_binds_to_temporary` in favour of
        // `__reference_constructs_from_temporary`.
        let rep = table::deprecated_replacement(t).or(match t {
            TypeTrait::IsTriviallyRelocatable if self.clang_compat >= 22 => {
                Some("__builtin_is_cpp_trivially_relocatable")
            }
            TypeTrait::ReferenceBindsToTemporary if self.clang_compat >= 22 => {
                Some("__reference_constructs_from_temporary")
            }
            _ => None,
        });
        if let Some(rep) = rep {
            // Repeated by every instantiation that rebuilds the trait.
            let d = self.diag(DiagId::sema_deprecated_builtin, loc).arg(t.spelling()).arg(rep);
            self.emit(d.per_instantiation());
        }
        self.build_type_trait(t, args, loc, rparen)
    }

    /// The arguments a non-dependent trait is evaluated on: canonical
    /// when one is only instantiation-dependent (an alias naming a
    /// concrete type through a dependent alias specialization; clang
    /// evaluates on the canonical types), `None` to use `args`.
    fn trait_eval_args(&self, args: &[(QualType, Loc)]) -> Option<Vec<(QualType, Loc)>> {
        args.iter()
            .any(|&(q, _)| self.is_dependent_type(q))
            .then(|| args.iter().map(|&(q, l)| (self.ast.types.canonical(q), l)).collect())
    }

    /// Builds `__trait(T, ...)` (clang's `BuildTypeTrait`; also the
    /// substituter's rebuild): checks the arity and the operands'
    /// completeness, then evaluates. Not-yet-implemented traits report
    /// `sema_unimplemented_template_feature` (their `__has_builtin` is
    /// 0, spec §13.3).
    pub fn build_type_trait(
        &mut self,
        t: TypeTrait,
        args: &[(QualType, Loc)],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let t = alias_target(t);
        let tys: Vec<QualType> = args.iter().map(|a| a.0).collect();
        if tys.iter().any(|&q| self.ast.types.flags(q.ty()).intersects(TypeFlags::CONTAINS_ERROR))
            || tys.contains(&QualType::ERROR)
        {
            return self.recover(self.truth_ty(), &[], loc);
        }
        let has_pack = tys.iter().any(|&q| self.is_pack_expansion_type(q));
        if !has_pack && !self.check_trait_arity(t, tys.len(), loc) {
            return self.recover(self.truth_ty(), &[], loc);
        }
        let rhs_loc = (tys.len() == 2).then(|| args[1].1);
        // Only a dependent type defers the trait (clang `BuildTypeTrait`
        // tests `isDependentType`); a merely instantiation-dependent one
        // (`TC<N>`, `N` a constant local of a function template) is
        // evaluated now, on its canonical type, so a specialization
        // instantiated with it does not cache an unevaluated `false`.
        let dep = TypeFlags::DEPENDENT.union(TypeFlags::UNEXPANDED_PACK);
        let dependent =
            has_pack || tys.iter().any(|&q| self.ast.types.flags(q.ty()).intersects(dep));
        if dependent {
            let ty = self.truth_ty();
            return self.builder().type_trait_expr(t, &tys, rhs_loc, false, ty, loc, rparen);
        }
        let Some(value) = self.eval_type_trait(t, args, loc, rparen) else {
            return self.recover(self.truth_ty(), &[], loc);
        };
        let ty = self.truth_ty();
        self.builder().type_trait_expr(t, &tys, rhs_loc, value, ty, loc, rparen)
    }

    /// Evaluates the non-dependent trait `t` on `args` (written at `loc`
    /// .. `rparen`, arity already checked): the one evaluator behind both
    /// `__trait(T, ...)` and the [meta.reflection.traits] functions
    /// (spec/reflection-traits.md), so the two cannot diverge. `None`
    /// after an error: diagnosed, or recorded in [`TraitState`] while a
    /// reflection trait evaluates quietly.
    pub(crate) fn eval_type_trait(
        &mut self,
        t: TypeTrait,
        args: &[(QualType, Loc)],
        loc: Loc,
        rparen: Loc,
    ) -> Option<bool> {
        if matches!(
            t,
            TypeTrait::ReferenceBindsToTemporary
                | TypeTrait::ReferenceConstructsFromTemporary
                | TypeTrait::ReferenceConvertsFromTemporary
        ) {
            return Some(self.eval_ref_from_temporary(t, args[0], args[1], loc, rparen));
        }
        let canon = self.trait_eval_args(args);
        let args = canon.as_deref().unwrap_or(args);
        if let Status::Owner(_) = table::trait_status(t) {
            if self.traits.record(TraitFailure::Unimplemented) {
                return None;
            }
            let what = format!("type trait '{}'", t.spelling());
            let d = self.diag(DiagId::sema_unimplemented_template_feature, loc).arg(what);
            self.emit(d);
            return None;
        }
        let value = if matches!(
            t,
            TypeTrait::IsConstructible
                | TypeTrait::IsNothrowConstructible
                | TypeTrait::IsTriviallyConstructible
        ) {
            self.eval_constructible(t, args, loc, rparen)?
        } else if let [(q, _)] = *args {
            if !self.check_trait_completeness(t, q, loc) {
                return None;
            }
            self.eval_unary_trait(t, q, loc)
        } else if t == TypeTrait::IsLayoutCompatible {
            self.eval_layout_compatible(args[0], args[1])
        } else if t == TypeTrait::IsPointerInterconvertibleBaseOf {
            self.eval_pointer_interconvertible_base(args[0], args[1])
        } else if matches!(
            t,
            TypeTrait::IsConvertible | TypeTrait::IsConvertibleTo | TypeTrait::IsNothrowConvertible
        ) {
            self.eval_convertible(t, args[0], args[1], loc)?
        } else if matches!(
            t,
            TypeTrait::IsAssignable
                | TypeTrait::IsNothrowAssignable
                | TypeTrait::IsTriviallyAssignable
        ) {
            let check = match t {
                TypeTrait::IsNothrowAssignable => InitCheck::Nothrow,
                TypeTrait::IsTriviallyAssignable => InitCheck::Trivial,
                _ => InitCheck::WellFormed,
            };
            self.eval_assignable(args[0], args[1], check, loc)?
        } else {
            self.eval_binary_trait(t, args[0].0, args[1].0, args[1].1)
        };
        Some(value)
    }

    /// `__array_rank(T)` / `__array_extent(T, I)` ([meta.unary.prop.query]):
    /// a `size_t` prvalue. The dimension must be an integral constant
    /// expression; an out-of-range dimension gives 0.
    pub fn build_array_type_trait(
        &mut self,
        t: TypeTrait,
        ty: QualType,
        dim: Option<ExprId>,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        if ty == QualType::ERROR || dim.is_some_and(|d| self.expr_has_error(d)) {
            return self.recover(self.size_type(), &[], loc);
        }
        let size_t = self.size_type();
        let dim_dependent = dim.is_some_and(|d| {
            self.ast.exprs.get(d).deps().intersects(ExprDeps::VALUE | ExprDeps::TYPE)
        });
        if self.is_dependent_type(ty) || dim_dependent {
            return self.builder().array_type_trait_expr(t, ty, dim, 0, size_t, loc, rparen);
        }
        let value = match (t, dim) {
            (TypeTrait::ArrayExtent, Some(d)) => {
                let v = self.fold_int(d).filter(|&v| (0..=i128::from(u32::MAX)).contains(&v));
                let Some(v) = v else {
                    let loc = self.eloc(d);
                    self.error(DiagId::sema_dimension_expr_not_const, loc);
                    return self.recover(size_t, &[d], loc);
                };
                self.array_extent(ty, u64::try_from(v).unwrap_or(0))
            }
            _ => self.array_rank(ty),
        };
        self.builder().array_type_trait_expr(t, ty, dim, value, size_t, loc, rparen)
    }

    /// `__is_lvalue_expr(e)` / `__is_rvalue_expr(e)`: the value category
    /// of the unevaluated operand.
    pub fn build_expression_trait(
        &mut self,
        t: TypeTrait,
        e: ExprId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let n = self.ast.exprs.get(e);
        let value = if n.deps().contains(ExprDeps::TYPE) {
            false
        } else {
            // clang: `isLValue()` / `isPRValue()`; an xvalue is neither.
            let vc = n.value_category();
            if t == TypeTrait::IsLvalueExpr {
                vc == ValueCategory::LValue
            } else {
                vc == ValueCategory::PRValue
            }
        };
        let ty = self.truth_ty();
        self.builder().expression_trait_expr(t, e, value, ty, loc, rparen)
    }

    pub(crate) fn expr_has_error(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).deps().contains(ExprDeps::ERROR)
    }

    fn is_pack_expansion_type(&self, q: QualType) -> bool {
        matches!(self.ast.types.view(q.ty()), TypeView::PackExpansion { .. })
    }

    /// clang's `CheckTypeTraitArity`: "type trait requires N argument(s);
    /// have M argument(s)" at the keyword.
    fn check_trait_arity(&mut self, t: TypeTrait, n: usize, loc: Loc) -> bool {
        let (want, more) = match table::arity(t) {
            Arity::Exact(k) => (k, false),
            Arity::AtLeast(k) => (k, true),
        };
        let ok = if more { n >= usize::from(want) } else { n == usize::from(want) };
        if !ok {
            let d = self
                .diag(DiagId::sema_type_trait_arity, loc)
                .arg(want)
                .arg(u8::from(more))
                .arg(u8::from(more || want != 1))
                .arg(n)
                .arg(u8::from(n != 1));
            self.emit(d);
        }
        ok
    }

    /// clang's `CheckUnaryTypeTraitTypeCompleteness` (classes in
    /// [`table::completeness`]): "incomplete type 'X' used in type trait
    /// expression" at the keyword, with the forward-declaration note.
    fn check_trait_completeness(&mut self, t: TypeTrait, q: QualType, loc: Loc) -> bool {
        let tys = &self.ast.types;
        let checked = match table::completeness(t) {
            Complete::None => return true,
            Complete::Element => {
                let e = self.base_element(q);
                if self.ast.types.is_void(e) {
                    return true;
                }
                self.ast.types.unqualified(e)
            }
            Complete::BoundedElement => {
                if tys.is_void(q)
                    || tys.is_reference(q)
                    || tys.is_function(q)
                    || matches!(tys.canonical_view(q), TypeView::IncompleteArray(_))
                {
                    return true;
                }
                q
            }
            Complete::Class => {
                let Some(r) = tys.as_record(q) else { return true };
                let _ = r;
                q
            }
            Complete::NonUnionClass => {
                let Some(r) = tys.as_record(q) else { return true };
                if self.is_union(r) {
                    return true;
                }
                q
            }
        };
        if self.ast.types.is_reference(checked) || self.ast.types.is_function(checked) {
            return true;
        }
        if self.trait_complete(checked, loc) {
            return true;
        }
        self.trait_incomplete(DiagId::sema_incomplete_type_trait, loc, checked);
        false
    }

    /// Completes `q` for a trait at `at` (instantiating a class template
    /// specialization). While a reflection trait evaluates quietly an
    /// undefined template is not diagnosed: the failure is the caller's.
    fn trait_complete(&mut self, q: QualType, at: Loc) -> bool {
        if self.traits.quiet {
            self.require_complete_silent(q, at)
        } else {
            self.require_complete(q, at)
        }
    }

    /// The trait's operand `q` (written at `at`) is incomplete where a
    /// complete type is required: diagnosed as `id` with the
    /// forward-declaration note, or recorded while evaluating quietly.
    fn trait_incomplete(&mut self, id: DiagId, at: Loc, q: QualType) {
        if self.traits.record(TraitFailure::Incomplete(q)) {
            return;
        }
        let msg = self.diag(id, at).arg(self.ty_arg(q));
        let msg = self.incomplete_note(msg, q);
        self.emit(msg);
    }

    /// Runs `f` with trait diagnostics recorded instead of emitted (the
    /// reflection traits: a failure is a `MetaError`, spec/reflection-traits.md
    /// §2). Errors from a class template instantiation the trait triggers
    /// are still diagnosed ([temp.inst]: they are not in the immediate
    /// context). Returns `f`'s result and the first recorded failure.
    pub(crate) fn quiet_trait<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> (R, Option<TraitFailure>) {
        let quiet = std::mem::replace(&mut self.traits.quiet, true);
        let outer = self.traits.failure.take();
        let r = f(self);
        self.traits.quiet = quiet;
        let failure = std::mem::replace(&mut self.traits.failure, outer);
        (r, failure)
    }

    pub(crate) fn array_rank(&self, q: QualType) -> u64 {
        let mut t = q;
        let mut n = 0;
        while self.ast.types.is_array(t) {
            n += 1;
            match self.ast.types.element(t) {
                Some(e) => t = e,
                None => break,
            }
        }
        n
    }

    pub(crate) fn array_extent(&self, q: QualType, dim: u64) -> u64 {
        let mut t = q;
        let mut i = 0;
        loop {
            if !self.ast.types.is_array(t) {
                return 0;
            }
            if i == dim {
                return self.ast.types.array_size(t).unwrap_or(0);
            }
            match self.ast.types.element(t) {
                Some(e) => t = e,
                None => return 0,
            }
            i += 1;
        }
    }

    /// clang's `EvaluateBinaryTypeTrait` for the implemented binary
    /// traits. `rhs_loc` is where the derived type of `__is_base_of` is
    /// written (its incomplete-type error points there).
    fn eval_binary_trait(&mut self, t: TypeTrait, a: QualType, b: QualType, rhs_loc: Loc) -> bool {
        match t {
            TypeTrait::IsSame | TypeTrait::IsSameAs => self.ast.types.same_type(a, b),
            TypeTrait::TypesCompatible => crate::conv::c_assign::compatible_unqualified(self, a, b),
            TypeTrait::IsVirtualBaseOf => self.is_virtual_base_of(a, b, rhs_loc),
            TypeTrait::IsBaseOf => {
                let (Some(base), Some(derived)) =
                    (self.ast.types.as_record(a), self.ast.types.as_record(b))
                else {
                    return false;
                };
                if self.is_union(base) || self.is_union(derived) {
                    return false;
                }
                if self.ast.first(base) == self.ast.first(derived) {
                    return true;
                }
                // [meta.rel]/2: Derived shall be complete.
                if !self.trait_complete(b, rhs_loc) {
                    self.trait_incomplete(DiagId::sema_incomplete_type_trait, rhs_loc, b);
                    return false;
                }
                crate::conv::bases::is_derived_from(self, derived, base)
            }
            _ => false,
        }
    }

    /// `__is_assignable(T, U)` ([meta.unary.prop]/3, clang's
    /// `EvaluateBinaryTypeTrait`): whether `declval<T>() = declval<U>()` is
    /// well-formed as an unevaluated operand. The assignment is built
    /// on opaque operands in a SFINAE context, with access checked from
    /// the translation unit; `__is_nothrow_assignable` also requires
    /// that it cannot throw. `None` when an operand is incomplete (not
    /// `void` or an array of unknown bound), which is an error.
    fn eval_assignable(
        &mut self,
        a: (QualType, Loc),
        b: (QualType, Loc),
        check: InitCheck,
        loc: Loc,
    ) -> Option<bool> {
        for (q, at) in [a, b] {
            let tys = &self.ast.types;
            if tys.is_void(q)
                || matches!(tys.canonical_view(q), TypeView::IncompleteArray(_))
                || tys.is_reference(q)
                || tys.is_function(q)
                || self.trait_complete(q, at)
            {
                continue;
            }
            self.trait_incomplete(DiagId::sema_incomplete_type_trait, at, q);
            return None;
        }
        if self.ast.types.is_void(a.0) || self.ast.types.is_void(b.0) {
            return Some(false);
        }
        // `declval<T>()`: an lvalue for an lvalue reference (or a
        // reference to function), else an xvalue.
        let operand = |s: &mut Self, q: QualType| {
            let (ty, vc) = match s.ast.types.canonical_view(q) {
                TypeView::LValueRef(p) => (p, ValueCategory::LValue),
                TypeView::RValueRef(p) if s.ast.types.is_function(p) => (p, ValueCategory::LValue),
                TypeView::RValueRef(p) => (p, ValueCategory::XValue),
                _ if s.ast.types.is_function(q) => (q, ValueCategory::LValue),
                _ => (q, ValueCategory::XValue),
            };
            s.builder().opaque_value_of(ty, vc, loc)
        };
        let tu = self.ast.tu;
        let saved = std::mem::replace(&mut self.ctx, tu);
        let r = self.in_sfinae(FrameKind::ConstraintSubstitution, tu, None, loc, |s| {
            s.enter_unevaluated();
            let l = operand(s, a.0);
            let r = operand(s, b.0);
            let e = s.binary_operator(BinaryOp::Assign, l, r, None, loc);
            s.exit_unevaluated();
            !s.is_err_expr(e) && s.init_check_holds(check, e)
        });
        self.ctx = saved;
        Some(matches!(r, Some((true, Ok(())))))
    }

    /// `__is_constructible(T, Args...)` / `__is_nothrow_constructible`
    /// ([meta.unary.prop]/9, clang's `EvaluateBooleanTypeTrait`): whether
    /// the direct-initialization `T t(declval<Args>()...);` of a
    /// temporary is well-formed as an unevaluated operand (and, for the
    /// nothrow form, cannot throw). Built in a SFINAE context with
    /// access checked from the translation unit. `None` when an operand
    /// is incomplete (not `void` or an array of unknown bound), an error.
    fn eval_constructible(
        &mut self,
        t: TypeTrait,
        args: &[(QualType, Loc)],
        loc: Loc,
        rparen: Loc,
    ) -> Option<bool> {
        for &(q, at) in args {
            let tys = &self.ast.types;
            if tys.is_void(q)
                || matches!(tys.canonical_view(q), TypeView::IncompleteArray(_))
                || tys.is_reference(q)
                || tys.is_function(q)
                || self.trait_complete(q, at)
            {
                continue;
            }
            self.trait_incomplete(DiagId::sema_incomplete_type_trait, at, q);
            return None;
        }
        let ty = args[0].0;
        let tys = &self.ast.types;
        if tys.is_void(ty)
            || matches!(tys.canonical_view(ty), TypeView::IncompleteArray(_))
            || tys.is_function(ty)
            || self.record_bit(ty, DefinitionBits::ABSTRACT)
            || args[1..].iter().any(|a| self.ast.types.is_void(a.0))
        {
            return Some(false);
        }
        // A reference is bound by exactly one argument; anything else
        // but a class or an array takes at most one.
        let n = args.len() - 1;
        let is_ref = self.ast.types.is_reference(ty);
        if (is_ref && n != 1)
            || (n > 1 && self.ast.types.as_record(ty).is_none() && !self.ast.types.is_array(ty))
        {
            return Some(false);
        }
        // `declval<A>()`: an lvalue for an lvalue reference (or a function
        // type), else an xvalue.
        let operand = |s: &mut Self, q: QualType, at: Loc| {
            let (ty, vc) = match s.ast.types.canonical_view(q) {
                TypeView::LValueRef(p) => (p, ValueCategory::LValue),
                TypeView::RValueRef(p) if s.ast.types.is_function(p) => (p, ValueCategory::LValue),
                TypeView::RValueRef(p) => (p, ValueCategory::XValue),
                _ if s.ast.types.is_function(q) => (q, ValueCategory::LValue),
                _ => (q, ValueCategory::XValue),
            };
            s.builder().opaque_value_of(ty, vc, at)
        };
        let check = match t {
            TypeTrait::IsNothrowConstructible => InitCheck::Nothrow,
            TypeTrait::IsTriviallyConstructible => InitCheck::Trivial,
            _ => InitCheck::WellFormed,
        };
        let tu = self.ast.tu;
        let saved = std::mem::replace(&mut self.ctx, tu);
        let r = self.in_sfinae(FrameKind::ConstraintSubstitution, tu, None, loc, |s| {
            s.enter_unevaluated();
            let ops: Vec<ExprId> = args[1..].iter().map(|&(q, at)| operand(s, q, at)).collect();
            let e = if ops.is_empty() {
                s.paren_value_init(ty, loc)
            } else {
                let init = crate::init::Initializer::Paren(ops, loc, rparen);
                s.perform_init(ty, init, loc, false)
            };
            // clang initializes `InitializedEntity::InitializeTemporary`:
            // a class temporary is bound (`MaybeBindToTemporary`), so its
            // destructor must be accessible and not deleted, and counts
            // for the nothrow form.
            let e = if s.is_err_expr(e) { e } else { s.maybe_bind_temporary(e) };
            s.exit_unevaluated();
            !s.is_err_expr(e) && s.init_check_holds(check, e)
        });
        self.ctx = saved;
        let ok = matches!(r, Some((true, Ok(()))));
        // The temporary is destroyed: clang's `CXXBindTemporaryExpr` of a
        // class with a non-trivial destructor is a non-trivial call.
        if ok && check == InitCheck::Trivial && !self.ast.types.is_reference(ty) {
            return Some(self.is_destructible(TypeTrait::IsTriviallyDestructible, ty, loc));
        }
        Some(ok)
    }

    /// The extra requirement of the nothrow / trivially form of an
    /// initialization or assignment trait on its well-formed operation `e`.
    fn init_check_holds(&self, check: InitCheck, e: ExprId) -> bool {
        match check {
            InitCheck::WellFormed => true,
            InitCheck::Nothrow => !self.can_throw(e),
            InitCheck::Trivial => !self.has_nontrivial_call(e),
        }
    }

    /// Whether evaluating `e` calls a function that is not a trivial
    /// special member ([meta.unary.prop]/9 "known to call no operation
    /// that is not trivial"; clang's `Expr::hasNonTrivialCall`): a call
    /// of a non-trivial function (a user-provided or non-trivial special
    /// member, a conversion function, any non-member function), the
    /// construction by a non-trivial constructor, or a temporary with a
    /// non-trivial destructor. Unevaluated operands are not entered.
    pub(crate) fn has_nontrivial_call(&self, e: ExprId) -> bool {
        let exprs = &self.ast.exprs;
        let trivial = |d: DeclId| self.ast.decls.function(d).is_some_and(|f| f.spec.is_trivial());
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let n = exprs.get(x);
            match n.kind() {
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::TypeTraitExpr
                | ExprKind::ArrayTypeTraitExpr
                | ExprKind::ExpressionTraitExpr
                | ExprKind::CXXTypeidExpr => continue,
                ExprKind::CallExpr
                | ExprKind::CXXOperatorCallExpr
                | ExprKind::CXXMemberCallExpr => {
                    let c = self.ignore_parens(exprs.callee(x));
                    let c = if exprs.get(c).kind().is_cast() { exprs.operand(c) } else { c };
                    let c = self.ignore_parens(c);
                    let named =
                        matches!(exprs.get(c).kind(), ExprKind::DeclRefExpr | ExprKind::MemberExpr);
                    if !named || !trivial(exprs.decl(c)) {
                        return true;
                    }
                }
                ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr
                    if !trivial(exprs.constructor(x)) =>
                {
                    return true;
                }
                ExprKind::CXXBindTemporaryExpr
                    if self
                        .ast
                        .types
                        .as_record(n.ty())
                        .and_then(|r| self.ast.definition(r))
                        .and_then(|r| self.ast.destructor(r))
                        .is_none_or(|d| !trivial(d)) =>
                {
                    return true;
                }
                _ => {}
            }
            exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        false
    }

    /// `__is_convertible(From, To)` / `__is_convertible_to` /
    /// `__is_nothrow_convertible` ([meta.rel]/5, clang's
    /// `EvaluateBinaryTypeTrait`): whether `To test() { return
    /// declval<From>(); }` is well-formed, i.e. the copy-initialization of
    /// a `To` temporary from a `declval<From>()` operand, built
    /// unevaluated under a SFINAE frame with access from the TU (and,
    /// for the nothrow form, cannot throw). `None` when an operand is
    /// incomplete, an error.
    fn eval_convertible(
        &mut self,
        t: TypeTrait,
        from: (QualType, Loc),
        to: (QualType, Loc),
        loc: Loc,
    ) -> Option<bool> {
        let tys = &self.ast.types;
        if tys.is_void(to.0) {
            return Some(tys.is_void(from.0));
        }
        if tys.is_function(to.0) || tys.is_array(to.0) || tys.is_void(from.0) {
            return Some(false);
        }
        for (q, at) in [from, to] {
            let tys = &self.ast.types;
            if matches!(tys.canonical_view(q), TypeView::IncompleteArray(_))
                || tys.is_reference(q)
                || tys.is_function(q)
                || self.trait_complete(q, at)
            {
                continue;
            }
            self.trait_incomplete(DiagId::sema_incomplete_type_trait, at, q);
            return None;
        }
        // clang `CheckConvertibilityForTypeTraits`: `To test()` needs a
        // non-abstract return type ([class.abstract]/3).
        if self.abstract_class(to.0).is_some() {
            return Some(false);
        }
        let (fty, vc) = match self.ast.types.canonical_view(from.0) {
            TypeView::LValueRef(p) => (p, ValueCategory::LValue),
            TypeView::RValueRef(p) if self.ast.types.is_function(p) => (p, ValueCategory::LValue),
            TypeView::RValueRef(p) => (p, ValueCategory::XValue),
            _ if self.ast.types.is_function(from.0) => (from.0, ValueCategory::LValue),
            _ => (from.0, ValueCategory::XValue),
        };
        let nothrow = t == TypeTrait::IsNothrowConvertible;
        let tu = self.ast.tu;
        let saved = std::mem::replace(&mut self.ctx, tu);
        let r = self.in_sfinae(FrameKind::ConstraintSubstitution, tu, None, loc, |s| {
            s.enter_unevaluated();
            let op = s.builder().opaque_value_of(fty, vc, from.1);
            let e = s.perform_init(to.0, crate::init::Initializer::Expr(op), loc, false);
            s.exit_unevaluated();
            !(s.is_err_expr(e) || (nothrow && s.can_throw(e)))
        });
        self.ctx = saved;
        Some(matches!(r, Some((true, Ok(())))))
    }

    /// `__reference_binds_to_temporary(T, U)` and the C++23
    /// `__reference_constructs_from_temporary` /
    /// `__reference_converts_from_temporary` ([meta.unary.prop], clang's
    /// `EvaluateBinaryTypeTrait`): `T` is a reference and its
    /// initialization from `declval<U>()` (direct for the first two, copy
    /// for the last) is well-formed and binds it to a materialized
    /// temporary rather than directly. The C++23 forms take a non-reference
    /// `U` as a prvalue.
    fn eval_ref_from_temporary(
        &mut self,
        t: TypeTrait,
        to: (QualType, Loc),
        from: (QualType, Loc),
        loc: Loc,
        rparen: Loc,
    ) -> bool {
        if !self.ast.types.is_reference(to.0) || self.ast.types.is_void(from.0) {
            return false;
        }
        if !self.ast.types.is_reference(from.0)
            && !self.ast.types.is_function(from.0)
            && !matches!(self.ast.types.canonical_view(from.0), TypeView::IncompleteArray(_))
            && !self.trait_complete(from.0, from.1)
        {
            return false;
        }
        // clang 22 evaluates `__reference_binds_to_temporary` as
        // `__reference_constructs_from_temporary` (a non-reference `U` is
        // a prvalue).
        let prvalue = t != TypeTrait::ReferenceBindsToTemporary || self.clang_compat >= 22;
        let (fty, vc) = match self.ast.types.canonical_view(from.0) {
            TypeView::LValueRef(p) => (p, ValueCategory::LValue),
            TypeView::RValueRef(p) if self.ast.types.is_function(p) => (p, ValueCategory::LValue),
            TypeView::RValueRef(p) => (p, ValueCategory::XValue),
            _ if self.ast.types.is_function(from.0) => (from.0, ValueCategory::LValue),
            // [expr.type]/2: a prvalue of a non-class, non-array type has
            // its cv-qualifiers dropped (`VAL<const int>` is an `int`), at
            // every -fclang-compat level: clang 19 keeps them, a bug clang
            // 22 and GCC 16.1 do not have.
            _ if prvalue
                && self.ast.types.as_record(from.0).is_none()
                && !self.ast.types.is_array(from.0) =>
            {
                (self.ast.types.unqualified(from.0), ValueCategory::PRValue)
            }
            _ if prvalue => (from.0, ValueCategory::PRValue),
            _ => (from.0, ValueCategory::XValue),
        };
        let tu = self.ast.tu;
        let saved = std::mem::replace(&mut self.ctx, tu);
        let r = self.in_sfinae(FrameKind::ConstraintSubstitution, tu, None, loc, |s| {
            s.enter_unevaluated();
            let op = s.builder().opaque_value_of(fty, vc, from.1);
            let init = if t == TypeTrait::ReferenceConvertsFromTemporary {
                crate::init::Initializer::Expr(op)
            } else {
                crate::init::Initializer::Paren(vec![op], loc, rparen)
            };
            let e = s.perform_init(to.0, init, loc, false);
            s.exit_unevaluated();
            (!s.is_err_expr(e)).then(|| s.binds_temporary(e))
        });
        self.ctx = saved;
        matches!(r, Some((Some(true), Ok(()))))
    }

    /// Whether the reference initialization `e` binds to a materialized
    /// temporary (clang's `!isDirectReferenceBinding()`).
    fn binds_temporary(&self, mut e: ExprId) -> bool {
        for _ in 0..64 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::MaterializeTemporaryExpr => return true,
                ExprKind::ParenExpr
                | ExprKind::ImplicitCastExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::ConstantExpr => e = self.ast.exprs.operand(e),
                _ => return false,
            }
        }
        false
    }

    /// The record of `q` (not looking through arrays), if it is a class
    /// or union with a definition.
    fn defined_record(&self, q: QualType) -> Option<DeclId> {
        let r = self.ast.types.as_record(q)?;
        self.ast.definition(r).is_some().then_some(r)
    }

    fn bits_of(&self, r: DeclId) -> DefinitionBits {
        self.defn_bits(r)
    }

    /// `__is_aggregate`: clang also reports vector and complex types as
    /// aggregates (they support aggregate initialization).
    fn is_aggregate_trait(&self, q: QualType) -> bool {
        let tys = &self.ast.types;
        tys.is_array(q)
            || tys.as_complex(q).is_some()
            || self.is_vector_type(q)
            || self.record_bit(q, DefinitionBits::AGGREGATE)
    }

    /// clang's `EvaluateUnaryTypeTrait`.
    #[allow(clippy::too_many_lines, reason = "one arm per trait")]
    fn eval_unary_trait(&mut self, t: TypeTrait, q: QualType, loc: Loc) -> bool {
        use TypeTrait as T;
        let tys = &self.ast.types;
        let view = tys.canonical_view(q);
        match t {
            T::IsVoid => tys.is_void(q),
            T::IsNullptr => self.is_nullptr_type(q),
            T::IsIntegral => tys.is_integral(q),
            T::IsFloatingPoint => self.is_floating_ext(q),
            T::IsArray => match view {
                TypeView::ConstantArray(a) => a.size != 0,
                _ => tys.is_array(q),
            },
            T::IsBoundedArray => match view {
                TypeView::ConstantArray(a) => a.size != 0,
                TypeView::IncompleteArray(_) => false,
                _ => tys.is_array(q),
            },
            T::IsUnboundedArray => matches!(view, TypeView::IncompleteArray(_)),
            T::IsPointer => matches!(view, TypeView::Pointer(_)),
            T::IsLvalueReference => matches!(view, TypeView::LValueRef(_)),
            T::IsRvalueReference => matches!(view, TypeView::RValueRef(_)),
            T::IsReference => tys.is_reference(q),
            T::IsMemberPointer => matches!(view, TypeView::MemberPointer { .. }),
            T::IsMemberFunctionPointer => {
                matches!(view, TypeView::MemberPointer { pointee, .. } if tys.is_function(pointee))
            }
            T::IsMemberObjectPointer => {
                matches!(view, TypeView::MemberPointer { pointee, .. } if !tys.is_function(pointee))
            }
            T::IsEnum => matches!(view, TypeView::Enum(_)),
            T::IsScopedEnum => crate::conv::rank::is_scoped_enum(self, q),
            T::IsUnion => tys.as_record(q).is_some_and(|r| self.is_union(r)),
            T::IsClass => tys.as_record(q).is_some_and(|r| !self.is_union(r)),
            T::IsFunction => tys.is_function(q),
            T::IsArithmetic => self.is_arithmetic_ext(q),
            T::IsFundamental => self.is_fundamental(q),
            // clang `isCompoundType`: the sizeless SVE builtins are neither.
            T::IsCompound => !self.is_fundamental(q) && !self.ast.types.is_sizeless(q),
            T::IsObject => self.is_object_type(q),
            T::IsScalar => self.is_scalar_ext(q),
            T::IsConst => !tys.is_reference(q) && tys.effective_quals(q).is_const(),
            T::IsVolatile => !tys.is_reference(q) && tys.effective_quals(q).is_volatile(),
            T::IsSigned => {
                self.is_floating_ext(q)
                    || match view {
                        TypeView::Builtin(b) => b.is_integral() && builtin_signed(self, b),
                        TypeView::BitInt { signed, .. } => signed,
                        _ => false,
                    }
            }
            T::IsUnsigned => match view {
                TypeView::Builtin(b) => b.is_integral() && !builtin_signed(self, b),
                TypeView::BitInt { signed, .. } => !signed,
                _ => false,
            },
            T::IsCompleteType => tys.is_function(q) || self.is_complete(q),
            T::IsReferenceable => match view {
                TypeView::Function(p) => {
                    p.info().this_quals().is_empty()
                        && p.info().ref_qual() == kiln_ast::types::RefQual::None
                }
                _ => tys.is_reference(q) || self.is_object_type(q),
            },
            T::IsPolymorphic => self.record_bit(q, DefinitionBits::POLYMORPHIC),
            T::IsAbstract => self.record_bit(q, DefinitionBits::ABSTRACT),
            T::IsFinal => self.defined_record(q).is_some_and(|r| self.is_final_class(r)),
            T::IsEmpty => self.defined_record(q).is_some_and(|r| {
                !self.is_union(r) && self.bits_of(r).contains(DefinitionBits::EMPTY)
            }),
            T::IsAggregate => self.is_aggregate_trait(q),
            T::IsTriviallyCopyable => self.is_trivially_copyable_type(q),
            T::IsTrivial => self.is_trivial_type(q),
            T::IsPod => self.is_pod_type(q),
            T::IsStandardLayout => self.is_standard_layout_type(q),
            T::IsLiteral | T::IsLiteralType => self.trait_literal(q),
            T::IsTriviallyRelocatable => self.is_trivially_relocatable(q),
            T::HasUniqueObjectRepresentations => self.has_unique_object_reps(q),
            T::HasNothrowAssign
            | T::HasNothrowMoveAssign
            | T::HasNothrowCopy
            | T::HasNothrowConstructor => self.has_nothrow(t, q, loc),
            T::IsTriviallyEqualityComparable => self.trivially_equality_comparable(q, loc),
            T::CanPassInRegs => self.can_pass_in_regs_trait(q, loc),
            T::IsBitwiseCloneable => self.is_bitwise_cloneable(q, 0),
            T::IsImplicitLifetime => self.is_implicit_lifetime(q),
            T::IsCppTriviallyRelocatable => self.is_cxx_trivially_relocatable(q, 0),
            T::IsDestructible | T::IsNothrowDestructible | T::IsTriviallyDestructible => {
                self.is_destructible(t, q, loc)
            }
            // clang `hasTrivial*() && !hasNonTrivial*()`: every eligible
            // member of the kind is trivial ([special]/6; WP-1109).
            T::HasTrivialDefaultConstructor => {
                self.is_pod_type(q)
                    || self
                        .element_record(q)
                        .is_some_and(|r| self.all_trivial(r, SpecialKind::DefaultCtor))
            }
            T::HasTrivialMoveConstructor => {
                self.is_pod_type(q)
                    || self
                        .element_record(q)
                        .is_some_and(|r| self.all_trivial(r, SpecialKind::MoveCtor))
            }
            T::HasTrivialCopy => {
                self.is_pod_type(q)
                    || tys.is_reference(q)
                    || self
                        .defined_record(q)
                        .is_some_and(|r| self.all_trivial(r, SpecialKind::CopyCtor))
            }
            T::HasTrivialMoveAssign => {
                self.is_pod_type(q)
                    || self
                        .element_record(q)
                        .is_some_and(|r| self.all_trivial(r, SpecialKind::MoveAssign))
            }
            T::HasTrivialAssign => {
                if !tys.is_reference(q) && tys.effective_quals(q).is_const() {
                    return false;
                }
                self.is_pod_type(q)
                    || self
                        .defined_record(q)
                        .is_some_and(|r| self.all_trivial(r, SpecialKind::CopyAssign))
            }
            T::HasTrivialDestructor => {
                self.is_pod_type(q)
                    || tys.is_reference(q)
                    || self
                        .element_record(q)
                        .is_some_and(|r| self.bits_of(r).contains(DefinitionBits::TRIVIAL_DTOR))
            }
            T::HasVirtualDestructor => {
                let Some(r) = self.defined_record(q) else { return false };
                match self.select_special(r, SpecialKind::Dtor, Quals::default(), Quals::default())
                {
                    Selected::Found(d) | Selected::Deleted(d) => self
                        .ast
                        .decls
                        .function(self.ast.first(d))
                        .is_some_and(|f| f.spec.is_virtual()),
                    _ => false,
                }
            }
            // Binary, n-ary, array and expression traits never reach
            // here.
            _ => false,
        }
    }

    fn record_bit(&self, q: QualType, bit: DefinitionBits) -> bool {
        self.defined_record(q).is_some_and(|r| self.bits_of(r).contains(bit))
    }

    fn is_final_class(&self, r: DeclId) -> bool {
        self.bits_of(r).contains(DefinitionBits::FINAL)
    }

    /// The defined class of `q`'s base element type.
    fn element_record(&self, q: QualType) -> Option<DeclId> {
        self.defined_record(self.base_element(q))
    }

    /// clang's `isFloatingType`: the floating builtins and complex
    /// floating types.
    fn is_floating_ext(&self, q: QualType) -> bool {
        match self.ast.types.canonical_view(q) {
            TypeView::Complex(e) => self.ast.types.is_floating(e),
            _ => self.ast.types.is_floating(q),
        }
    }

    /// clang's `isArithmeticType() && !isEnumeralType()` (complex types
    /// included).
    fn is_arithmetic_ext(&self, q: QualType) -> bool {
        matches!(self.ast.types.canonical_view(q), TypeView::Complex(_))
            || self.ast.types.is_arithmetic(q)
    }

    /// [basic.fundamental]/17 (clang `isFundamentalType`).
    fn is_fundamental(&self, q: QualType) -> bool {
        self.ast.types.is_void(q)
            || self.is_nullptr_type(q)
            || self.is_arithmetic_ext(q)
            || self.is_meta_info_type(q)
    }

    /// `std::meta::info`, a fundamental ([basic.fundamental]/17) and
    /// scalar ([basic.types.general]/9) type.
    pub(crate) fn is_meta_info_type(&self, q: QualType) -> bool {
        self.ast.types.builtin(q) == Some(kiln_ast::types::BuiltinType::MetaInfo)
    }

    /// [basic.types.general]/8: not a function, reference or `void`.
    pub(crate) fn is_object_type(&self, q: QualType) -> bool {
        let t = &self.ast.types;
        !t.is_function(q) && !t.is_reference(q) && !t.is_void(q)
    }

    /// clang's `isScalarType`: arithmetic (complex included),
    /// enumeration, pointer, member pointer, `std::nullptr_t` and
    /// `std::meta::info`.
    fn is_scalar_ext(&self, q: QualType) -> bool {
        matches!(
            self.ast.types.canonical_view(q),
            TypeView::Pointer(_) | TypeView::MemberPointer { .. } | TypeView::Enum(_)
        ) || self.is_arithmetic_ext(q)
            || self.is_nullptr_type(q)
            || self.is_meta_info(q)
    }

    /// Scalar or vector (clang treats vectors as scalars here).
    fn is_scalar_or_vector(&self, q: QualType) -> bool {
        self.is_scalar_ext(q) || matches!(self.ast.types.canonical_view(q), TypeView::Vector { .. })
    }

    /// [basic.types.general]/9 (clang `isTriviallyCopyableType`).
    pub(crate) fn is_trivially_copyable_type(&self, q: QualType) -> bool {
        let e = self.base_element(q);
        // `std::meta::info` is scalar: trivially copyable, trivial and
        // POD ([basic.types.general]/9; GCC 16.1 agrees). A constant
        // evaluation still copies it only as an `info` (spec/reflection.md
        // §2.5, typed copy).
        if self.is_dependent_type(e) || !self.is_complete(e) {
            return false;
        }
        // clang: the sizeless SVE builtins are trivial, but not
        // standard-layout or literal.
        if self.is_scalar_or_vector(e) || self.ast.types.is_sizeless(e) {
            return true;
        }
        match self.defined_record(e) {
            Some(r) => self.bits_of(r).contains(DefinitionBits::TRIVIALLY_COPYABLE),
            None => false,
        }
    }

    /// [basic.types.general]/9 trivial types (clang `isTrivialType`).
    fn is_trivial_type(&self, q: QualType) -> bool {
        let e = self.base_element(q);
        // `std::meta::info` is scalar: trivially copyable, trivial and
        // POD ([basic.types.general]/9; GCC 16.1 agrees). A constant
        // evaluation still copies it only as an `info` (spec/reflection.md
        // §2.5, typed copy).
        if self.is_dependent_type(e) || !self.is_complete(e) {
            return false;
        }
        // clang: the sizeless SVE builtins are trivial, but not
        // standard-layout or literal.
        if self.is_scalar_or_vector(e) || self.ast.types.is_sizeless(e) {
            return true;
        }
        match self.defined_record(e) {
            // [class.prop]/2: one or more eligible default constructors,
            // all trivial (clang `isTrivialType`).
            Some(r) => {
                self.has_special(r, SpecialKind::DefaultCtor)
                    && self.all_trivial(r, SpecialKind::DefaultCtor)
                    && self.bits_of(r).contains(DefinitionBits::TRIVIALLY_COPYABLE)
            }
            None => false,
        }
    }

    /// C++11 POD types (clang `isCXX11PODType`): trivial and
    /// standard-layout.
    pub(crate) fn is_pod_type(&self, q: QualType) -> bool {
        let e = self.base_element(q);
        // `std::meta::info` is scalar: trivially copyable, trivial and
        // POD ([basic.types.general]/9; GCC 16.1 agrees). A constant
        // evaluation still copies it only as an `info` (spec/reflection.md
        // §2.5, typed copy).
        if self.is_dependent_type(e) || !self.is_complete(e) {
            return false;
        }
        // clang: the sizeless SVE builtins are trivial, but not
        // standard-layout or literal.
        if self.is_scalar_or_vector(e) || self.ast.types.is_sizeless(e) {
            return true;
        }
        match self.defined_record(e) {
            Some(r) => {
                // clang `CXXRecordDecl::isTrivial`: a trivial default
                // constructor (`hasTrivialDefaultConstructor`; a
                // non-trivial one beside it does not count).
                let b = self.bits_of(r);
                self.has_special(r, SpecialKind::DefaultCtor)
                    && self.has_trivial(r, SpecialKind::DefaultCtor)
                    && b.contains(DefinitionBits::TRIVIALLY_COPYABLE)
                    && b.contains(DefinitionBits::STANDARD_LAYOUT)
            }
            None => false,
        }
    }

    /// [basic.types.general]/9 standard-layout types.
    fn is_standard_layout_type(&self, q: QualType) -> bool {
        let e = self.base_element(q);
        if self.is_dependent_type(e) || !self.is_complete(e) {
            return false;
        }
        if self.is_scalar_or_vector(e) {
            return true;
        }
        self.record_bit(e, DefinitionBits::STANDARD_LAYOUT)
    }

    /// [basic.types.general]/10 literal types (clang `isLiteralType`).
    fn trait_literal(&self, q: QualType) -> bool {
        let t = &self.ast.types;
        if t.is_void(q) {
            return true;
        }
        if matches!(t.canonical_view(q), TypeView::VariableArray { .. }) {
            return false;
        }
        let e = self.base_element(q);
        if t.is_reference(e) {
            return true;
        }
        if t.is_function(e) || !self.is_complete(e) {
            return false;
        }
        if self.is_scalar_or_vector(e) || matches!(t.canonical_view(e), TypeView::Complex(_)) {
            return true;
        }
        if let Some(r) = self.defined_record(e) {
            return self.bits_of(r).contains(DefinitionBits::LITERAL);
        }
        match t.canonical_view(e) {
            TypeView::Atomic(v) => self.trait_literal(v),
            _ => false,
        }
    }

    /// P2786 / clang 19 `isTriviallyRelocatableType`.
    fn is_trivially_relocatable(&self, q: QualType) -> bool {
        let e = self.base_element(q);
        if !self.is_complete(e) || !self.is_object_type(e) {
            return false;
        }
        if let Some(r) = self.defined_record(e) {
            return self.can_pass_in_registers(r);
        }
        // clang: any other complete object type of C++ (none is destructed
        // or non-trivial to move primitively).
        true
    }

    /// [meta.unary.prop] `is_destructible` and its trivially / nothrow
    /// variants (clang's `UTT_Is*Destructible`).
    fn is_destructible(&mut self, t: TypeTrait, q: QualType, loc: Loc) -> bool {
        let tys = &self.ast.types;
        if tys.is_reference(q) {
            return true;
        }
        if tys.is_function(q) || !self.is_complete(q) {
            return false;
        }
        let Some(r) = self.element_record(q) else { return true };
        if t == TypeTrait::IsTriviallyDestructible
            && !self.bits_of(r).contains(DefinitionBits::TRIVIAL_DTOR)
        {
            return false;
        }
        let Selected::Found(d) =
            self.select_special(r, SpecialKind::Dtor, Quals::default(), Quals::default())
        else {
            return false;
        };
        let access = self.ast.decls.get(d).misc().access();
        if !matches!(access, Access::Public | Access::None) {
            return false;
        }
        if t == TypeTrait::IsNothrowDestructible {
            return self.dtor_nothrow(r, d, loc);
        }
        true
    }

    /// Whether destructor `d` of class `r` is non-throwing. An unwritten
    /// specification (`ExcSpec::Deferred`) is the implicit one of
    /// [except.spec]/8: non-throwing unless the destructor of a base or
    /// of a non-static data member is potentially throwing. Virtual bases
    /// are direct bases of a complete object, so walking the direct bases
    /// covers them.
    fn dtor_nothrow(&mut self, r: DeclId, d: DeclId, loc: Loc) -> bool {
        // A destructor of a class template specialization keeps its
        // `noexcept(expr)` uninstantiated until needed ([temp.inst]/14):
        // `is_nothrow_destructible` needs it.
        let first = self.ast.first(d);
        if self.uninstantiated_exc.contains_key(&first) {
            self.instantiate_exception_spec(first, loc);
        }
        let d = first;
        let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let t = &self.ast.types;
        let Some(p) = t.proto(ty) else { return false };
        match t.exc_spec(p) {
            ExcSpec::Deferred(_) => {}
            e => return e.is_nothrow(),
        }
        let def = self.class_def(r);
        let mut subs: Vec<QualType> =
            self.ast.decls.bases(def, &self.ast.lists).map(|b| b.ty).collect();
        subs.extend(
            self.fields_of(r)
                .into_iter()
                .map(|f| self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR)),
        );
        // The subobject's destructor itself, whatever its access: a
        // protected base destructor (libc++'s `__list_imp`) is usable by
        // the derived class's ([except.spec]/8 names the invoked
        // destructor, not `is_nothrow_destructible`).
        subs.into_iter().all(|q| {
            if self.ast.types.is_reference(q) {
                return true;
            }
            let Some(sr) = self.element_record(q) else { return true };
            match self.select_special(sr, SpecialKind::Dtor, Quals::default(), Quals::default()) {
                Selected::Found(sd) => self.dtor_nothrow(sr, sd, loc),
                // A deleted or ambiguous one deletes this destructor
                // instead ([class.dtor]/7).
                _ => true,
            }
        })
    }

    /// [meta.unary.prop] `has_unique_object_representations` (clang's
    /// `ASTContext::hasUniqueObjectRepresentations`).
    fn has_unique_object_reps(&self, q: QualType) -> bool {
        let e = self.base_element(q);
        if !self.is_trivially_copyable_type(e) {
            return false;
        }
        self.unique_reps_tc(e, true)
    }

    /// [`Sema::has_unique_object_reps`] of a non-array type (trivially
    /// copyable, or not checked when `tc` is false: clang's
    /// `CheckIfTriviallyCopyable`).
    fn unique_reps_tc(&self, q: QualType, tc: bool) -> bool {
        let tys = &self.ast.types;
        match tys.canonical_view(q) {
            TypeView::Builtin(b) if b.is_integral() => true,
            TypeView::Enum(_) | TypeView::Pointer(_) | TypeView::MemberPointer { .. } => true,
            TypeView::BitInt { width, .. } => {
                self.size_bits(q).is_some_and(|s| s == u64::from(width))
            }
            TypeView::Record(r) => {
                if self.is_union(r) {
                    self.union_unique_reps(r, q, tc)
                } else {
                    let size = self.size_bits(q);
                    self.struct_unique_bits(r, tc).is_some_and(|b| Some(b) == size)
                }
            }
            _ => false,
        }
    }

    /// clang's `hasUniqueObjectRepresentations(q, tc)`.
    fn unique_reps(&self, q: QualType, tc: bool) -> bool {
        if self.ast.types.is_array(q) {
            return self.unique_reps(self.base_element(q), tc);
        }
        (!tc || self.is_trivially_copyable_type(q)) && self.unique_reps_tc(q, tc)
    }

    fn size_bits(&self, q: QualType) -> Option<u64> {
        target::size_align(&self.ast, q, &self.abi).map(|(s, _)| s.saturating_mul(8))
    }

    fn fields_of(&self, r: DeclId) -> Vec<DeclId> {
        let def = self.class_def(r);
        self.ast
            .members(def)
            .filter(|&m| self.ast.decls.get(m).kind() == kiln_ast::decl::DeclKind::Field)
            .collect()
    }

    /// clang's `unionHasUniqueObjectRepresentations`.
    fn union_unique_reps(&self, r: DeclId, q: QualType, tc: bool) -> bool {
        let Some(us) = self.size_bits(q) else { return false };
        let fields = self.fields_of(r);
        !fields.is_empty()
            && fields.iter().all(|&f| {
                let ft = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                self.unique_reps(ft, tc) && self.size_bits(ft) == Some(us)
            })
    }

    /// clang's `structHasUniqueObjectRepresentations`: the number of
    /// value bits of class `r` when its bases and fields are laid out
    /// contiguously, else `None`.
    fn struct_unique_bits(&self, r: DeclId, tc: bool) -> Option<u64> {
        let def = self.class_def(r);
        let bits = self.bits_of(def);
        if bits.contains(DefinitionBits::POLYMORPHIC)
            || bits.contains(DefinitionBits::HAS_VIRTUAL_BASES)
        {
            return None;
        }
        let layout = kiln_abi::record_layout(&self.ast, def, &self.abi);
        let mut cur: u64 = 0;
        let mut bases: Vec<(DeclId, u64)> = layout.bases.to_vec();
        bases.sort_by_key(|b| b.1);
        for (b, off) in bases {
            let size = self.struct_unique_bits(b, tc)?;
            if size != 0 {
                if off.saturating_mul(8) != cur {
                    return None;
                }
                cur += size;
            }
        }
        for (i, f) in self.fields_of(def).into_iter().enumerate() {
            let size = self.field_unique_bits(f, tc)?;
            if size != 0 {
                if layout.fields.get(i).copied() != Some(cur) {
                    return None;
                }
                cur += size;
            }
        }
        Some(cur)
    }

    /// clang's `getSubobjectSizeInBits` for a field.
    fn field_unique_bits(&self, f: DeclId, tc: bool) -> Option<u64> {
        let ft = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let tys = &self.ast.types;
        if let Some(r) = tys.as_record(ft)
            && !self.is_union(r)
            && !tys.is_array(ft)
        {
            return self.struct_unique_bits(r, tc);
        }
        let bit_int = matches!(tys.canonical_view(ft), TypeView::BitInt { .. });
        if !tys.is_reference(ft) && !bit_int && !self.unique_reps(ft, tc) {
            return None;
        }
        let size = self.size_bits(ft)?;
        if let Some(w) = self.ast.decls.field(f).and_then(|x| x.bit_width) {
            if self.ast.decls.get(f).name().is_empty() {
                return Some(0);
            }
            let w = u64::from(crate::conv::eval_width(self, w)?);
            if let TypeView::BitInt { width, .. } = tys.canonical_view(ft) {
                if w > u64::from(width) {
                    return None;
                }
            } else if w > size {
                return None;
            }
            return Some(w);
        }
        if bit_int && !self.unique_reps(ft, tc) {
            return None;
        }
        Some(size)
    }
}

/// Which form of an initialization or assignment trait is evaluated
/// (`__is_X`, `__is_nothrow_X`, `__is_trivially_X`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InitCheck {
    WellFormed,
    Nothrow,
    Trivial,
}

/// clang `TypeTraits.def` `ALIAS`es: the keyword names the aliased trait,
/// which is what diagnostics and dumps spell.
fn alias_target(t: TypeTrait) -> TypeTrait {
    match t {
        TypeTrait::IsSameAs => TypeTrait::IsSame,
        TypeTrait::IsLiteralType => TypeTrait::IsLiteral,
        _ => t,
    }
}
