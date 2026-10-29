//! The type-trait predicates (spec/sema-templates.md §9.1, §9.4):
//! `__is_*` / `__has_*` (`TypeTraitExpr`), `__array_rank` /
//! `__array_extent` (`ArrayTypeTraitExpr`) and `__is_lvalue_expr` /
//! `__is_rvalue_expr` (`ExpressionTraitExpr`). Each is a `bool` (or
//! `size_t`) prvalue over unevaluated operands; a dependent operand
//! gives a value-dependent node that substitution rebuilds through
//! [`Sema::build_type_trait`]. The evaluators follow clang 19's
//! `EvaluateUnaryTypeTrait` / `EvaluateBinaryTypeTrait`, including its
//! extensions (complex types count as arithmetic and scalar, a
//! zero-length array is not an array).

use kiln_ast::decl::{Access, DefinitionBits, TagKind};
use kiln_ast::expr::{ExprDeps, TypeTrait, ValueCategory};
use kiln_ast::types::{BuiltinType, TypeFlags, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::table::{self, Arity, Complete, Status};
use crate::Sema;
use crate::class::{Selected, SpecialKind};
use crate::conv::rank::builtin_signed;
use crate::target;

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
        if let Some(rep) = table::deprecated_replacement(t) {
            let d = self.diag(DiagId::sema_deprecated_builtin, loc).arg(t.spelling()).arg(rep);
            self.emit(d);
        }
        self.build_type_trait(t, args, loc, rparen)
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
        let tys: Vec<QualType> = args.iter().map(|a| a.0).collect();
        if tys.iter().any(|&q| self.ast.types.flags(q.ty()).intersects(TypeFlags::CONTAINS_ERROR))
            || tys.contains(&QualType::ERROR)
        {
            return self.recover(QualType::BOOL, &[], loc);
        }
        let has_pack = tys.iter().any(|&q| self.is_pack_expansion_type(q));
        if !has_pack && !self.check_trait_arity(t, tys.len(), loc) {
            return self.recover(QualType::BOOL, &[], loc);
        }
        let rhs_loc = (tys.len() == 2).then(|| args[1].1);
        let dependent = has_pack
            || tys.iter().any(|&q| {
                self.is_dependent_type(q)
                    || self.ast.types.flags(q.ty()).intersects(TypeFlags::UNEXPANDED_PACK)
            });
        if dependent {
            return self.builder().type_trait_expr(
                t,
                &tys,
                rhs_loc,
                false,
                QualType::BOOL,
                loc,
                rparen,
            );
        }
        if let Status::Owner(_) = table::trait_status(t) {
            let what = format!("type trait '{}'", t.spelling());
            let d = self.diag(DiagId::sema_unimplemented_template_feature, loc).arg(what);
            self.emit(d);
            return self.recover(QualType::BOOL, &[], loc);
        }
        let value = if tys.len() == 1 {
            if !self.check_trait_completeness(t, tys[0], loc) {
                return self.recover(QualType::BOOL, &[], loc);
            }
            self.eval_unary_trait(t, tys[0])
        } else {
            self.eval_binary_trait(t, tys[0], tys[1], args[1].1)
        };
        self.builder().type_trait_expr(t, &tys, rhs_loc, value, QualType::BOOL, loc, rparen)
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
            let lv = n.value_category() == ValueCategory::LValue;
            if t == TypeTrait::IsLvalueExpr { lv } else { !lv }
        };
        self.builder().expression_trait_expr(t, e, value, QualType::BOOL, loc, rparen)
    }

    fn expr_has_error(&self, e: ExprId) -> bool {
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
                let e = self.ast.types.unqualified(e);
                e
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
        if self.require_complete(checked, loc) {
            return true;
        }
        let msg = self.diag(DiagId::sema_incomplete_type_trait, loc).arg(self.ty_arg(checked));
        let msg = self.incomplete_note(msg, checked);
        self.emit(msg);
        false
    }

    fn array_rank(&self, q: QualType) -> u64 {
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

    fn array_extent(&self, q: QualType, dim: u64) -> u64 {
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
                if !self.require_complete(b, rhs_loc) {
                    let msg =
                        self.diag(DiagId::sema_incomplete_type_trait, rhs_loc).arg(self.ty_arg(b));
                    let msg = self.incomplete_note(msg, b);
                    self.emit(msg);
                    return false;
                }
                crate::conv::bases::is_derived_from(self, derived, base)
            }
            _ => false,
        }
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

    /// clang's `EvaluateUnaryTypeTrait`.
    #[allow(clippy::too_many_lines, reason = "one arm per trait")]
    fn eval_unary_trait(&mut self, t: TypeTrait, q: QualType) -> bool {
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
            T::IsCompound => !self.is_fundamental(q),
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
            T::IsEmpty => self
                .defined_record(q)
                .is_some_and(|r| !self.is_union(r) && self.bits_of(r).contains(DefinitionBits::EMPTY)),
            T::IsAggregate => {
                tys.is_array(q) || self.record_bit(q, DefinitionBits::AGGREGATE)
            }
            T::IsTriviallyCopyable => self.is_trivially_copyable_type(q),
            T::IsTrivial => self.is_trivial_type(q),
            T::IsPod => self.is_pod_type(q),
            T::IsStandardLayout => self.is_standard_layout_type(q),
            T::IsLiteral | T::IsLiteralType => self.trait_literal(q),
            T::IsTriviallyRelocatable => self.is_trivially_relocatable(q),
            T::HasUniqueObjectRepresentations => self.has_unique_object_reps(q),
            T::IsDestructible | T::IsNothrowDestructible | T::IsTriviallyDestructible => {
                self.is_destructible(t, q)
            }
            T::HasTrivialDefaultConstructor => {
                self.is_pod_type(q)
                    || self.element_record(q).is_some_and(|r| {
                        self.has_special(r, SpecialKind::DefaultCtor)
                            && self.trivial_default_ctor(r)
                    })
            }
            T::HasTrivialMoveConstructor => {
                self.is_pod_type(q)
                    || self.element_record(q).is_some_and(|r| {
                        self.has_special(r, SpecialKind::MoveCtor)
                            && self.bits_of(r).contains(DefinitionBits::TRIVIAL_MOVE_CTOR)
                    })
            }
            T::HasTrivialCopy => {
                self.is_pod_type(q)
                    || tys.is_reference(q)
                    || self
                        .defined_record(q)
                        .is_some_and(|r| self.bits_of(r).contains(DefinitionBits::TRIVIAL_COPY_CTOR))
            }
            T::HasTrivialMoveAssign => {
                self.is_pod_type(q)
                    || self.element_record(q).is_some_and(|r| {
                        self.has_special(r, SpecialKind::MoveAssign)
                            && self.bits_of(r).contains(DefinitionBits::TRIVIAL_MOVE_ASSIGN)
                    })
            }
            T::HasTrivialAssign => {
                if !tys.is_reference(q) && tys.effective_quals(q).is_const() {
                    return false;
                }
                self.is_pod_type(q)
                    || self
                        .defined_record(q)
                        .is_some_and(|r| self.bits_of(r).contains(DefinitionBits::TRIVIAL_COPY_ASSIGN))
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
                match self.select_special(r, SpecialKind::Dtor, Default::default(), Default::default()) {
                    Selected::Found(d) | Selected::Deleted(d) => self
                        .ast
                        .decls
                        .function(self.ast.first(d))
                        .is_some_and(|f| f.spec.is_virtual()),
                    _ => false,
                }
            }
            // Binary, n-ary, array and expression traits, and the
            // unimplemented rows, never reach here.
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
        self.ast.types.is_void(q) || self.is_nullptr_type(q) || self.is_arithmetic_ext(q)
    }

    /// [basic.types.general]/8: not a function, reference or `void`.
    fn is_object_type(&self, q: QualType) -> bool {
        let t = &self.ast.types;
        !t.is_function(q) && !t.is_reference(q) && !t.is_void(q)
    }

    /// clang's `isScalarType`: arithmetic (complex included),
    /// enumeration, pointer, member pointer and `std::nullptr_t`.
    fn is_scalar_ext(&self, q: QualType) -> bool {
        matches!(
            self.ast.types.canonical_view(q),
            TypeView::Pointer(_) | TypeView::MemberPointer { .. } | TypeView::Enum(_)
        ) || self.is_arithmetic_ext(q)
            || self.is_nullptr_type(q)
    }

    /// Scalar or vector (clang treats vectors as scalars here).
    fn is_scalar_or_vector(&self, q: QualType) -> bool {
        self.is_scalar_ext(q) || matches!(self.ast.types.canonical_view(q), TypeView::Vector { .. })
    }

    /// [basic.types.general]/9 (clang `isTriviallyCopyableType`).
    pub(crate) fn is_trivially_copyable_type(&self, q: QualType) -> bool {
        let e = self.base_element(q);
        if self.is_dependent_type(e) || !self.is_complete(e) {
            return false;
        }
        if self.is_scalar_or_vector(e) {
            return true;
        }
        match self.defined_record(e) {
            Some(r) => self.bits_of(r).contains(DefinitionBits::TRIVIALLY_COPYABLE),
            None => false,
        }
    }

    /// The trivial default constructor half of a trivial class
    /// ([class.prop]/2).
    fn trivial_default_ctor(&self, r: DeclId) -> bool {
        let b = self.bits_of(r);
        b.contains(DefinitionBits::TRIVIAL_DEFAULT_CTOR)
            && !b.contains(DefinitionBits::USER_PROVIDED_DEFAULT_CTOR)
    }

    /// [basic.types.general]/9 trivial types (clang `isTrivialType`).
    fn is_trivial_type(&self, q: QualType) -> bool {
        let e = self.base_element(q);
        if self.is_dependent_type(e) || !self.is_complete(e) {
            return false;
        }
        if self.is_scalar_or_vector(e) {
            return true;
        }
        match self.defined_record(e) {
            Some(r) => {
                self.has_special(r, SpecialKind::DefaultCtor)
                    && self.trivial_default_ctor(r)
                    && self.bits_of(r).contains(DefinitionBits::TRIVIALLY_COPYABLE)
            }
            None => false,
        }
    }

    /// C++11 POD types (clang `isCXX11PODType`): trivial and
    /// standard-layout.
    fn is_pod_type(&self, q: QualType) -> bool {
        let e = self.base_element(q);
        if self.is_dependent_type(e) || !self.is_complete(e) {
            return false;
        }
        if self.is_scalar_or_vector(e) {
            return true;
        }
        match self.defined_record(e) {
            Some(r) => {
                let b = self.bits_of(r);
                self.has_special(r, SpecialKind::DefaultCtor)
                    && self.trivial_default_ctor(r)
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
            return self.bits_of(r).contains(DefinitionBits::TRIVIALLY_RELOCATABLE);
        }
        self.is_trivially_copyable_type(e)
    }

    /// [meta.unary.prop] `is_destructible` and its trivially / nothrow
    /// variants (clang's `UTT_Is*Destructible`).
    fn is_destructible(&mut self, t: TypeTrait, q: QualType) -> bool {
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
        let d = match self.select_special(r, SpecialKind::Dtor, Default::default(), Default::default())
        {
            Selected::Found(d) => d,
            _ => return false,
        };
        let access = self.ast.decls.get(d).misc().access();
        if !matches!(access, Access::Public | Access::None) {
            return false;
        }
        if t == TypeTrait::IsNothrowDestructible {
            let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
            let t = &self.ast.types;
            return t.proto(ty).is_some_and(|p| t.exc_spec(p).is_nothrow());
        }
        true
    }

    /// [meta.unary.prop] `has_unique_object_representations` (clang's
    /// `ASTContext::hasUniqueObjectRepresentations`).
    fn has_unique_object_reps(&self, q: QualType) -> bool {
        let e = self.base_element(q);
        if !self.is_trivially_copyable_type(e) {
            return false;
        }
        self.unique_reps_tc(e)
    }

    /// [`Sema::has_unique_object_reps`] of a non-array, trivially
    /// copyable type.
    fn unique_reps_tc(&self, q: QualType) -> bool {
        let tys = &self.ast.types;
        match tys.canonical_view(q) {
            TypeView::Builtin(b) if b.is_integral() => true,
            TypeView::Enum(_) | TypeView::Pointer(_) | TypeView::MemberPointer { .. } => true,
            TypeView::BitInt { width, .. } => {
                self.size_bits(q).is_some_and(|s| s == u64::from(width))
            }
            TypeView::Record(r) => {
                if self.is_union(r) {
                    self.union_unique_reps(r, q)
                } else {
                    let size = self.size_bits(q);
                    self.struct_unique_bits(r).is_some_and(|b| Some(b) == size)
                }
            }
            _ => false,
        }
    }

    fn unique_reps(&self, q: QualType) -> bool {
        if self.ast.types.is_array(q) {
            return self.unique_reps(self.base_element(q));
        }
        self.is_trivially_copyable_type(q) && self.unique_reps_tc(q)
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
    fn union_unique_reps(&self, r: DeclId, q: QualType) -> bool {
        let Some(us) = self.size_bits(q) else { return false };
        let fields = self.fields_of(r);
        !fields.is_empty()
            && fields.iter().all(|&f| {
                let ft = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                self.unique_reps(ft) && self.size_bits(ft) == Some(us)
            })
    }

    /// clang's `structHasUniqueObjectRepresentations`: the number of
    /// value bits of class `r` when its bases and fields are laid out
    /// contiguously, else `None`.
    fn struct_unique_bits(&self, r: DeclId) -> Option<u64> {
        let def = self.class_def(r);
        let bits = self.bits_of(def);
        if bits.contains(DefinitionBits::POLYMORPHIC) || bits.contains(DefinitionBits::HAS_VIRTUAL_BASES) {
            return None;
        }
        let layout = kiln_abi::record_layout(&self.ast, def, &self.abi);
        let mut cur: u64 = 0;
        let mut bases: Vec<(DeclId, u64)> = layout.bases.to_vec();
        bases.sort_by_key(|b| b.1);
        for (b, off) in bases {
            let size = self.struct_unique_bits(b)?;
            if size != 0 {
                if off.saturating_mul(8) != cur {
                    return None;
                }
                cur += size;
            }
        }
        for (i, f) in self.fields_of(def).into_iter().enumerate() {
            let size = self.field_unique_bits(f)?;
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
    fn field_unique_bits(&self, f: DeclId) -> Option<u64> {
        let ft = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let tys = &self.ast.types;
        if let Some(r) = tys.as_record(ft) {
            if !self.is_union(r) && !tys.is_array(ft) {
                return self.struct_unique_bits(r);
            }
        }
        let bit_int = matches!(tys.canonical_view(ft), TypeView::BitInt { .. });
        if !tys.is_reference(ft) && !bit_int && !self.unique_reps(ft) {
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
        if bit_int && !self.unique_reps(ft) {
            return None;
        }
        Some(size)
    }
}

#[allow(dead_code, reason = "kept for the n-ary traits of WP-068a")]
fn _unused(_: BuiltinType, _: TagKind) {}
