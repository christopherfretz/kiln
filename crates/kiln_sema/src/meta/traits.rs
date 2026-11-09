//! The type-trait family ([meta.reflection.traits];
//! spec/reflection-traits.md). Every function is one query of the builtin
//! trait engine (`templ/builtins/traits*.rs`, `transforms.rs`), run
//! quietly so a failure becomes a `MetaError` note rather than a
//! diagnostic: `__is_X(T)` and `is_X_type(^^T)` share their evaluator and
//! cannot diverge. The traits with no `__trait` builtin are the engine's
//! library delegation (`traits/library.rs`).

use kiln_ast::QualType;
use kiln_ast::ReflectionId;
use kiln_ast::consts::{PtrBase, Scalar};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::TypeTrait;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{Quals, TemplateArg, UnaryTransformKind};
use kiln_oven::{MetaCtx, MetaError, MetaOut, Oven, TraitOp, info_arg};
use kiln_source::Loc;

use super::fail;
use crate::Sema;
use crate::lookup::LookupKind;
use crate::templ::builtins::table::{self, Complete};
use crate::templ::builtins::{LibFailure, TraitFailure};

/// How many range elements a range form evaluates without a heap buffer.
const RANGE_INLINE: usize = 8;

/// The longest range a range form accepts (a bound on a bogus size).
const MAX_RANGE: u64 = 1 << 16;

/// What a trait op computes, once its operands are types.
#[derive(Clone, Copy)]
enum Shape {
    /// `__trait(T...)` on the operands as given.
    Engine(TypeTrait),
    /// `is_{,trivially_,nothrow_}{default,copy,move}_constructible`: the
    /// constructible trait on `T` plus the implied argument.
    Construct(TypeTrait, Implied),
    /// `is_{,trivially_,nothrow_}{copy,move}_assignable`: the assignable
    /// trait on `T&` and the implied argument.
    Assign(TypeTrait, Implied),
    /// A transformation `__transform(T)`.
    Transform(UnaryTransformKind),
    /// `add_const` / `add_volatile` / `add_cv`.
    AddQuals(Quals),
    /// `std::NAME<args>::value` as a `bool`.
    LibBool(&'static str),
    /// `std::NAME<args>::value` as a `size_t`.
    LibSize(&'static str),
    /// `std::NAME<args>::type`.
    LibType(&'static str),
    /// `std::NAME<I, T>::type` (`tuple_element`, `variant_alternative`).
    LibIndexed(&'static str),
    IsReflection,
    IsStructural,
    Rank,
    Extent,
    /// A range form whose `reflection_range` argument is not read yet.
    Range,
    TypeOrder,
}

/// A quiet engine answer: a value, or a type the caller reflects once
/// the quiet evaluation is over (interning can fail, which is fatal and
/// diagnosed: spec/sema.md, WP-471).
enum Answer {
    Out(MetaOut),
    Type(QualType),
}

/// The second argument a copy/move/default trait implies.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Implied {
    None,
    /// `const T&` ([meta.unary.prop]: `add_lvalue_reference_t<const T>`).
    ConstRef,
    /// `T&&` (`add_rvalue_reference_t<T>`).
    RvalueRef,
}

/// The op's [`Shape`] and how many leading `info` type operands it has
/// (a range form's elements follow them).
#[allow(clippy::too_many_lines, reason = "one arm per op")]
fn shape(op: TraitOp) -> (Shape, usize) {
    use Shape as S;
    use TraitOp as O;
    use TypeTrait as T;
    use UnaryTransformKind as K;
    let e1 = |t| (S::Engine(t), 1);
    let e2 = |t| (S::Engine(t), 2);
    match op {
        O::IsVoidType => e1(T::IsVoid),
        O::IsNullPointerType => e1(T::IsNullptr),
        O::IsIntegralType => e1(T::IsIntegral),
        O::IsFloatingPointType => e1(T::IsFloatingPoint),
        O::IsArrayType => e1(T::IsArray),
        O::IsPointerType => e1(T::IsPointer),
        O::IsLvalueReferenceType => e1(T::IsLvalueReference),
        O::IsRvalueReferenceType => e1(T::IsRvalueReference),
        O::IsMemberObjectPointerType => e1(T::IsMemberObjectPointer),
        O::IsMemberFunctionPointerType => e1(T::IsMemberFunctionPointer),
        O::IsEnumType => e1(T::IsEnum),
        O::IsUnionType => e1(T::IsUnion),
        O::IsClassType => e1(T::IsClass),
        O::IsFunctionType => e1(T::IsFunction),
        O::IsReflectionType => (S::IsReflection, 1),
        O::IsReferenceType => e1(T::IsReference),
        O::IsArithmeticType => e1(T::IsArithmetic),
        O::IsFundamentalType => e1(T::IsFundamental),
        O::IsObjectType => e1(T::IsObject),
        O::IsScalarType => e1(T::IsScalar),
        O::IsCompoundType => e1(T::IsCompound),
        O::IsMemberPointerType => e1(T::IsMemberPointer),
        O::IsConstType => e1(T::IsConst),
        O::IsVolatileType => e1(T::IsVolatile),
        O::IsTriviallyCopyableType => e1(T::IsTriviallyCopyable),
        O::IsStandardLayoutType => e1(T::IsStandardLayout),
        O::IsEmptyType => e1(T::IsEmpty),
        O::IsPolymorphicType => e1(T::IsPolymorphic),
        O::IsAbstractType => e1(T::IsAbstract),
        O::IsFinalType => e1(T::IsFinal),
        O::IsAggregateType => e1(T::IsAggregate),
        O::IsStructuralType => (S::IsStructural, 1),
        O::IsSignedType => e1(T::IsSigned),
        O::IsUnsignedType => e1(T::IsUnsigned),
        O::IsBoundedArrayType => e1(T::IsBoundedArray),
        O::IsUnboundedArrayType => e1(T::IsUnboundedArray),
        O::IsScopedEnumType => e1(T::IsScopedEnum),
        O::IsDefaultConstructibleType => (S::Construct(T::IsConstructible, Implied::None), 1),
        O::IsCopyConstructibleType => (S::Construct(T::IsConstructible, Implied::ConstRef), 1),
        O::IsMoveConstructibleType => (S::Construct(T::IsConstructible, Implied::RvalueRef), 1),
        O::IsAssignableType => e2(T::IsAssignable),
        O::IsCopyAssignableType => (S::Assign(T::IsAssignable, Implied::ConstRef), 1),
        O::IsMoveAssignableType => (S::Assign(T::IsAssignable, Implied::RvalueRef), 1),
        O::IsSwappableWithType => (S::LibBool("is_swappable_with"), 2),
        O::IsSwappableType => (S::LibBool("is_swappable"), 1),
        O::IsDestructibleType => e1(T::IsDestructible),
        O::IsTriviallyDefaultConstructibleType => {
            (S::Construct(T::IsTriviallyConstructible, Implied::None), 1)
        }
        O::IsTriviallyCopyConstructibleType => {
            (S::Construct(T::IsTriviallyConstructible, Implied::ConstRef), 1)
        }
        O::IsTriviallyMoveConstructibleType => {
            (S::Construct(T::IsTriviallyConstructible, Implied::RvalueRef), 1)
        }
        O::IsTriviallyAssignableType => e2(T::IsTriviallyAssignable),
        O::IsTriviallyCopyAssignableType => {
            (S::Assign(T::IsTriviallyAssignable, Implied::ConstRef), 1)
        }
        O::IsTriviallyMoveAssignableType => {
            (S::Assign(T::IsTriviallyAssignable, Implied::RvalueRef), 1)
        }
        O::IsTriviallyDestructibleType => e1(T::IsTriviallyDestructible),
        O::IsNothrowDefaultConstructibleType => {
            (S::Construct(T::IsNothrowConstructible, Implied::None), 1)
        }
        O::IsNothrowCopyConstructibleType => {
            (S::Construct(T::IsNothrowConstructible, Implied::ConstRef), 1)
        }
        O::IsNothrowMoveConstructibleType => {
            (S::Construct(T::IsNothrowConstructible, Implied::RvalueRef), 1)
        }
        O::IsNothrowAssignableType => e2(T::IsNothrowAssignable),
        O::IsNothrowCopyAssignableType => (S::Assign(T::IsNothrowAssignable, Implied::ConstRef), 1),
        O::IsNothrowMoveAssignableType => {
            (S::Assign(T::IsNothrowAssignable, Implied::RvalueRef), 1)
        }
        O::IsNothrowSwappableWithType => (S::LibBool("is_nothrow_swappable_with"), 2),
        O::IsNothrowSwappableType => (S::LibBool("is_nothrow_swappable"), 1),
        O::IsNothrowDestructibleType => e1(T::IsNothrowDestructible),
        O::IsImplicitLifetimeType => e1(T::IsImplicitLifetime),
        O::HasVirtualDestructor => e1(T::HasVirtualDestructor),
        O::HasUniqueObjectRepresentations => e1(T::HasUniqueObjectRepresentations),
        O::ReferenceConstructsFromTemporary => e2(T::ReferenceConstructsFromTemporary),
        O::ReferenceConvertsFromTemporary => e2(T::ReferenceConvertsFromTemporary),
        O::Rank => (S::Rank, 1),
        O::Extent => (S::Extent, 1),
        O::IsSameType => e2(T::IsSame),
        O::IsBaseOfType => e2(T::IsBaseOf),
        O::IsVirtualBaseOfType => e2(T::IsVirtualBaseOf),
        O::IsConvertibleType => e2(T::IsConvertible),
        O::IsNothrowConvertibleType => e2(T::IsNothrowConvertible),
        O::IsLayoutCompatibleType => e2(T::IsLayoutCompatible),
        O::IsPointerInterconvertibleBaseOfType => e2(T::IsPointerInterconvertibleBaseOf),
        O::RemoveConst => (S::Transform(K::RemoveConst), 1),
        O::RemoveVolatile => (S::Transform(K::RemoveVolatile), 1),
        O::RemoveCv => (S::Transform(K::RemoveCv), 1),
        O::AddConst => (S::AddQuals(Quals::CONST), 1),
        O::AddVolatile => (S::AddQuals(Quals::VOLATILE), 1),
        O::AddCv => (S::AddQuals(Quals::CV), 1),
        O::RemoveReference => (S::Transform(K::RemoveReference), 1),
        O::AddLvalueReference => (S::Transform(K::AddLvalueReference), 1),
        O::AddRvalueReference => (S::Transform(K::AddRvalueReference), 1),
        O::MakeSigned => (S::Transform(K::MakeSigned), 1),
        O::MakeUnsigned => (S::Transform(K::MakeUnsigned), 1),
        O::RemoveExtent => (S::Transform(K::RemoveExtent), 1),
        O::RemoveAllExtents => (S::Transform(K::RemoveAllExtents), 1),
        O::RemovePointer => (S::Transform(K::RemovePointer), 1),
        O::AddPointer => (S::Transform(K::AddPointer), 1),
        O::RemoveCvref => (S::Transform(K::RemoveCvref), 1),
        O::Decay => (S::Transform(K::Decay), 1),
        O::UnderlyingType => (S::Transform(K::UnderlyingType), 1),
        O::UnwrapReference => (S::LibType("unwrap_reference"), 1),
        O::UnwrapRefDecay => (S::LibType("unwrap_ref_decay"), 1),
        O::TupleSize => (S::LibSize("tuple_size"), 1),
        O::VariantSize => (S::LibSize("variant_size"), 1),
        O::TupleElement => (S::LibIndexed("tuple_element"), 0),
        O::VariantAlternative => (S::LibIndexed("variant_alternative"), 0),
        O::TypeOrder => (S::TypeOrder, 0),
        O::IsConstructibleType
        | O::IsTriviallyConstructibleType
        | O::IsNothrowConstructibleType
        | O::IsInvocableType
        | O::IsNothrowInvocableType
        | O::InvokeResult
        | O::IsInvocableRType
        | O::IsNothrowInvocableRType
        | O::CommonType
        | O::CommonReference => (S::Range, 0),
        O::IsConstructibleTypeN => e1(T::IsConstructible),
        O::IsTriviallyConstructibleTypeN => e1(T::IsTriviallyConstructible),
        O::IsNothrowConstructibleTypeN => e1(T::IsNothrowConstructible),
        O::IsInvocableTypeN => (S::LibBool("is_invocable"), 1),
        O::IsNothrowInvocableTypeN => (S::LibBool("is_nothrow_invocable"), 1),
        O::IsInvocableRTypeN => (S::LibBool("is_invocable_r"), 2),
        O::IsNothrowInvocableRTypeN => (S::LibBool("is_nothrow_invocable_r"), 2),
        O::InvokeResultN => (S::LibType("invoke_result"), 1),
        O::CommonTypeN => (S::LibType("common_type"), 0),
        O::CommonReferenceN => (S::LibType("common_reference"), 0),
    }
}

/// The `(const info*, size_t)` form of `reflection_range` form `op`, and
/// the index of its range argument (also the number of leading `info`s).
pub(super) fn range_n(op: TraitOp) -> Option<(TraitOp, usize)> {
    use TraitOp as O;
    Some(match op {
        O::IsConstructibleType => (O::IsConstructibleTypeN, 1),
        O::IsTriviallyConstructibleType => (O::IsTriviallyConstructibleTypeN, 1),
        O::IsNothrowConstructibleType => (O::IsNothrowConstructibleTypeN, 1),
        O::IsInvocableType => (O::IsInvocableTypeN, 1),
        O::IsNothrowInvocableType => (O::IsNothrowInvocableTypeN, 1),
        O::InvokeResult => (O::InvokeResultN, 1),
        O::IsInvocableRType => (O::IsInvocableRTypeN, 2),
        O::IsNothrowInvocableRType => (O::IsNothrowInvocableRTypeN, 2),
        O::CommonType => (O::CommonTypeN, 0),
        O::CommonReference => (O::CommonReferenceN, 0),
        _ => return None,
    })
}

/// Whether `op` is a `(const info*, size_t)` range form.
fn is_range_n(op: TraitOp) -> bool {
    use TraitOp as O;
    matches!(
        op,
        O::IsConstructibleTypeN
            | O::IsTriviallyConstructibleTypeN
            | O::IsNothrowConstructibleTypeN
            | O::IsInvocableTypeN
            | O::IsNothrowInvocableTypeN
            | O::IsInvocableRTypeN
            | O::IsNothrowInvocableRTypeN
            | O::InvokeResultN
            | O::CommonTypeN
            | O::CommonReferenceN
    )
}

/// The header that declares library trait `std::NAME`, for the note.
fn header_of(name: &str) -> &'static str {
    match name {
        "tuple_size" | "tuple_element" => "<tuple>",
        "variant_size" | "variant_alternative" => "<variant>",
        "unwrap_reference" | "unwrap_ref_decay" => "<type_traits> or <functional>",
        _ => "<type_traits>",
    }
}

impl Sema<'_, '_> {
    /// Runs trait `op` on `args` into `out` (spec/reflection-traits.md §2).
    ///
    /// # Errors
    /// A non-type reflection, an incomplete type where the IS requires a
    /// complete one, an operand outside a transformation's domain, a
    /// library trait the TU does not declare: a note each. A silent
    /// failure in a trial that would have to instantiate. Faults reading
    /// a range.
    pub(super) fn meta_trait(
        &mut self,
        op: TraitOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        let (shape, fixed) = shape(op);
        match shape {
            Shape::Range => {
                // The `reflection_range` form (<meta>): its range argument
                // arrives read as `data | n << 64` (the site's
                // `Shape::Range` recipe); run the `(const info*, size_t)`
                // form on it.
                let (n_op, k) = range_n(op).ok_or_else(MetaError::invalid)?;
                let v = *args.get(k).ok_or_else(MetaError::invalid)?;
                let mut a = [0u128; 4];
                a[..k].copy_from_slice(&args[..k]);
                a[k] = v & u128::from(u64::MAX);
                a[k + 1] = v >> 64;
                return self.meta_trait(n_op, &a[..k + 2], out, ctx, oven);
            }
            Shape::TypeOrder => return self.meta_type_order(args, out, ctx),
            _ => {}
        }
        // The leading type operands (`tuple_element(I, T)` has its type
        // second).
        let mut tys = [QualType::ERROR; 2];
        let lead = if let Shape::LibIndexed(_) = shape {
            tys[0] = self.trait_operand(info_arg(args, 1)?, loc)?;
            1
        } else {
            for (i, slot) in tys.iter_mut().enumerate().take(fixed) {
                *slot = self.trait_operand(info_arg(args, i)?, loc)?;
            }
            fixed
        };
        if is_range_n(op) {
            return self.meta_trait_range(op, shape, &tys[..lead], args, out, ctx, oven);
        }
        let tys = &tys[..lead];
        if self.trait_is_light(shape, tys) {
            return self.run_trait(op, shape, tys, args, out, loc);
        }
        // Anything else may instantiate or evaluate: never in a trial
        // (spec/reflection.md §4.3), and with the Oven lent back. The
        // Transactional checkpoint `substitute` opens around a variable
        // template's instantiation is not a trial (`ctx.trial` is a fold
        // or a speculative checkpoint; WP-538): the initializer evaluated
        // there asks its traits here, as a manifestly constant-evaluated
        // evaluation.
        if ctx.trial || self.inst.eval_loc.is_none() {
            return Err(speculative_refusal(op, loc));
        }
        self.lend_oven(oven, |s| s.run_trait(op, shape, tys, args, out, loc))
    }

    /// A `(const info*, size_t)` range form (spec/reflection-traits.md
    /// §1.3): `lead` are the leading type operands; the range is
    /// `args[lead.len()]` (the pointer) and `args[lead.len() + 1]` (the
    /// count). The elements are read with the caller's Oven before it is
    /// lent; up to [`RANGE_INLINE`] of them need no heap buffer.
    #[allow(clippy::too_many_arguments, reason = "the dispatch context, passed through")]
    fn meta_trait_range(
        &mut self,
        op: TraitOp,
        shape: Shape,
        lead: &[QualType],
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        let k = lead.len();
        let p = args.get(k).copied().unwrap_or(0);
        let Some(n) =
            args.get(k + 1).and_then(|&n| u64::try_from(n).ok()).filter(|&n| n <= MAX_RANGE)
        else {
            return fail(loc, format!("'{}': the range is too long", op.name()));
        };
        let total = k + usize::try_from(n).unwrap_or(usize::MAX);
        let mut inline = [QualType::ERROR; RANGE_INLINE + 2];
        let mut heap = Vec::new();
        let tys: &mut [QualType] = if total <= inline.len() {
            &mut inline[..total]
        } else {
            heap.resize(total, QualType::ERROR);
            &mut heap
        };
        tys[..k].copy_from_slice(lead);
        for (i, slot) in (0..n).zip(tys[k..].iter_mut()) {
            *slot = self.trait_operand(oven.meta_info_at(p, i, ctx)?, loc)?;
        }
        if ctx.trial || self.inst.eval_loc.is_none() {
            return Err(speculative_refusal(op, loc));
        }
        let tys = &*tys;
        self.lend_oven(oven, |s| {
            let (r, failure) = s.quiet_trait(|s| s.eval_trait_range(shape, tys, loc));
            s.trait_result(op, r, failure, out, loc)
        })
    }

    /// One engine query for a range form's `shape` on all of `tys`.
    fn eval_trait_range(
        &mut self,
        shape: Shape,
        tys: &[QualType],
        loc: Loc,
    ) -> Result<Answer, LibFailure> {
        let none = LibFailure::NoMember;
        match shape {
            Shape::Engine(t) => {
                let mut inline = [(QualType::ERROR, loc); RANGE_INLINE + 2];
                let mut heap = Vec::new();
                let ops: &mut [(QualType, Loc)] = if tys.len() <= inline.len() {
                    &mut inline[..tys.len()]
                } else {
                    heap.resize(tys.len(), (QualType::ERROR, loc));
                    &mut heap
                };
                for (o, &t) in ops.iter_mut().zip(tys) {
                    o.0 = t;
                }
                let v = self.eval_type_trait(t, ops, loc, loc).ok_or(none)?;
                Ok(Answer::Out(MetaOut::Scalar(u128::from(v))))
            }
            Shape::LibBool(name) | Shape::LibType(name) => {
                let mut inline = [TemplateArg::NULL; RANGE_INLINE + 2];
                let mut heap = Vec::new();
                let targs: &mut [TemplateArg] = if tys.len() <= inline.len() {
                    &mut inline[..tys.len()]
                } else {
                    heap.resize(tys.len(), TemplateArg::NULL);
                    &mut heap
                };
                for (a, &t) in targs.iter_mut().zip(tys) {
                    *a = TemplateArg::ty(t);
                }
                if let Shape::LibBool(_) = shape {
                    let v = self.library_trait_value(name, targs, loc)?;
                    return Ok(Answer::Out(MetaOut::Scalar(u128::from(v != 0))));
                }
                let t = self.library_trait_type(name, targs, loc)?;
                Ok(Answer::Type(t))
            }
            _ => Err(none),
        }
    }

    /// Whether `shape` on `tys` can neither instantiate nor evaluate, so
    /// it runs without the Oven: a category, a cv test, `is_same`,
    /// `rank`/`extent`, or a transformation not completing an enum; or a
    /// property trait of operands already complete.
    fn trait_is_light(&self, shape: Shape, tys: &[QualType]) -> bool {
        let settled = || tys.iter().all(|&q| self.undefined_spec(q).is_none());
        match shape {
            Shape::Engine(t) => match t {
                TypeTrait::IsSame
                | TypeTrait::IsLayoutCompatible
                | TypeTrait::IsBaseOf
                | TypeTrait::IsVirtualBaseOf
                | TypeTrait::IsPointerInterconvertibleBaseOf => settled(),
                _ if tys.len() == 1 => match table::completeness(t) {
                    Complete::None => true,
                    _ => {
                        settled()
                            && !matches!(
                                t,
                                TypeTrait::IsDestructible
                                    | TypeTrait::IsNothrowDestructible
                                    | TypeTrait::IsTriviallyDestructible
                                    | TypeTrait::IsImplicitLifetime
                            )
                    }
                },
                _ => false,
            },
            Shape::Transform(k) => k != UnaryTransformKind::UnderlyingType || settled(),
            Shape::AddQuals(_) | Shape::IsReflection | Shape::Rank | Shape::Extent => true,
            _ => false,
        }
    }

    /// The type `r` represents ([meta.reflection.traits]/4: a type or a
    /// type alias), else the "does not represent a type" failure (p2).
    /// `type_order(a, b)` (P2830R10, [meta.reflection.traits]): the
    /// `strong_ordering::less`/`equal`/`greater` static member of the
    /// intrinsic's result type, copied whole. The order is GCC 16.1's
    /// (gcc/cp/method.cc `type_order_value`): `strcmp` of the canonical
    /// types' Itanium manglings (`i` < `j`, `N2ns1AE` < `N2ns1BE`), so it is
    /// the same in every TU for every type with linkage, and a library
    /// that sorts types by it iterates them in GCC's order; distinct types
    /// mangled alike are ordered by their interned id.
    fn meta_type_order(
        &mut self,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        let a = self.trait_operand(info_arg(args, 0)?, loc)?;
        let b = self.trait_operand(info_arg(args, 1)?, loc)?;
        let (a, b) = (self.ast.types.canonical(a), self.ast.types.canonical(b));
        let ord = if a == b {
            std::cmp::Ordering::Equal
        } else {
            let mut anon = kiln_abi::mangle::AnonIds::default();
            let ma = kiln_abi::mangle::mangle_type(&self.ast, &mut anon, a);
            let mb = kiln_abi::mangle::mangle_type(&self.ast, &mut anon, b);
            ma.as_bytes().cmp(mb.as_bytes()).then(a.cmp(&b))
        };
        let name = match ord {
            std::cmp::Ordering::Less => "less",
            std::cmp::Ordering::Equal => "equal",
            std::cmp::Ordering::Greater => "greater",
        };
        let ret = ctx
            .callee
            .and_then(|f| self.ast.decls.get(f).ty())
            .and_then(|q| self.ast.types.proto(q))
            .map(|p| self.ast.types.canonical(p.ret()).without_quals());
        let member = ret.and_then(|r| {
            let cls = self.ast.types.as_record(r)?;
            let cls = self.ast.definition(cls)?;
            let n = self.intern(name);
            let m = self.lookup_qualified(cls, n, LookupKind::Ordinary).single()?;
            (self.kind_of(m) == DeclKind::Var).then_some((r, self.ast.first(m)))
        });
        let Some((r, m)) = member else {
            return fail(
                loc,
                format!(
                    "'type_order' needs a result class with a static member '{name}' \
                         (std::strong_ordering)"
                ),
            );
        };
        let ty = self.ast.types.pointer_to(r);
        let p = self
            .ast
            .consts
            .push_scalar(Scalar::Pointer {
                ty,
                base: PtrBase::Decl(m),
                offset: 0,
                designated: None,
                past: false,
            })
            .map_err(|_| MetaError::memory())?;
        *out = MetaOut::Deref(p);
        Ok(())
    }

    fn trait_operand(&self, r: ReflectionId, loc: Loc) -> Result<QualType, MetaError> {
        let t = match self.ast.reflections.get(r) {
            Reflectee::Type(t) => Some(t),
            Reflectee::Decl(d)
                if matches!(self.kind_of(d), DeclKind::Typedef | DeclKind::TypeAlias) =>
            {
                self.ast.decls.get(d).ty()
            }
            _ => None,
        };
        match t {
            Some(t) if t != QualType::ERROR => Ok(self.ast.types.canonical(t)),
            _ => fail(loc, format!("'{}' does not represent a type", self.reflection_str(r))),
        }
    }

    /// Evaluates the non-range `shape` on `tys` quietly into `out`.
    fn run_trait(
        &mut self,
        op: TraitOp,
        shape: Shape,
        tys: &[QualType],
        args: &[u128],
        out: &mut MetaOut,
        loc: Loc,
    ) -> Result<(), MetaError> {
        let (r, failure) = self.quiet_trait(|s| s.eval_trait_shape(shape, tys, args, loc));
        self.trait_result(op, r, failure, out, loc)
    }

    /// One engine query for `shape`.
    fn eval_trait_shape(
        &mut self,
        shape: Shape,
        tys: &[QualType],
        args: &[u128],
        loc: Loc,
    ) -> Result<Answer, LibFailure> {
        let bool_out = |v: Option<bool>| v.map(|b| MetaOut::Scalar(u128::from(b)));
        let none = LibFailure::NoMember;
        Ok(Answer::Out(match shape {
            Shape::Engine(t) => {
                let ops = [(tys[0], loc), (tys.get(1).copied().unwrap_or(QualType::ERROR), loc)];
                bool_out(self.eval_type_trait(t, &ops[..tys.len()], loc, loc)).ok_or(none)?
            }
            Shape::Construct(t, implied) => {
                let a = (tys[0], loc);
                let v = match self.implied_arg(tys[0], implied, loc) {
                    Some(b) => self.eval_type_trait(t, &[a, (b, loc)], loc, loc),
                    None => self.eval_type_trait(t, &[a], loc, loc),
                };
                bool_out(v).ok_or(none)?
            }
            Shape::Assign(t, implied) => {
                let lhs =
                    self.apply_unary_transform(UnaryTransformKind::AddLvalueReference, tys[0], loc);
                let rhs = self.implied_arg(tys[0], implied, loc);
                let (Some(lhs), Some(rhs)) = (lhs, rhs) else { return Err(none) };
                bool_out(self.eval_type_trait(t, &[(lhs, loc), (rhs, loc)], loc, loc))
                    .ok_or(none)?
            }
            Shape::Transform(k) => {
                let t = self.apply_unary_transform(k, tys[0], loc).ok_or(none)?;
                return Ok(Answer::Type(t));
            }
            Shape::AddQuals(q) => {
                let t = self.add_quals_transform(tys[0], q);
                return Ok(Answer::Type(t));
            }
            Shape::IsReflection => MetaOut::Scalar(u128::from(self.is_meta_info_type(tys[0]))),
            Shape::IsStructural => bool_out(self.is_structural_type(tys[0], loc)).ok_or(none)?,
            Shape::Rank => MetaOut::Scalar(u128::from(self.array_rank(tys[0]))),
            Shape::Extent => {
                let i = args.get(1).map_or(0, |&v| u64::try_from(v).unwrap_or(u64::MAX));
                MetaOut::Scalar(u128::from(self.array_extent(tys[0], i)))
            }
            Shape::LibBool(name) | Shape::LibSize(name) => {
                let targs = [
                    TemplateArg::ty(tys[0]),
                    TemplateArg::ty(tys.get(1).copied().unwrap_or(QualType::ERROR)),
                ];
                let v = self.library_trait_value(name, &targs[..tys.len()], loc)?;
                MetaOut::Scalar(u128::try_from(v).map_err(|_| none)?)
            }
            Shape::LibType(name) => {
                let t = self.library_trait_type(name, &[TemplateArg::ty(tys[0])], loc)?;
                return Ok(Answer::Type(t));
            }
            Shape::LibIndexed(name) => {
                let i = args.first().map_or(0, |&v| u64::try_from(v).unwrap_or(u64::MAX));
                let targs = [self.size_t_arg(i), TemplateArg::ty(tys[0])];
                let t = self.library_trait_type(name, &targs, loc)?;
                return Ok(Answer::Type(t));
            }
            Shape::Range | Shape::TypeOrder => return Err(none),
        }))
    }

    /// The argument a copy/move trait of `t` implies: `const T&` or
    /// `T&&`, through the engine's transforms (a non-referenceable `T`,
    /// `void` or an abominable function type, gives `T` itself).
    fn implied_arg(&mut self, t: QualType, implied: Implied, loc: Loc) -> Option<QualType> {
        match implied {
            Implied::None => None,
            Implied::ConstRef => {
                let c = self.add_quals_transform(t, Quals::CONST);
                self.apply_unary_transform(UnaryTransformKind::AddLvalueReference, c, loc)
            }
            Implied::RvalueRef => {
                self.apply_unary_transform(UnaryTransformKind::AddRvalueReference, t, loc)
            }
        }
    }

    /// The reflection of `t`, canonical ([meta.reflection.traits]/5: the
    /// result is never an alias).
    fn type_reflection(&mut self, t: QualType, loc: Loc) -> Result<ReflectionId, MetaError> {
        let t = self.ast.types.canonical(t);
        Ok(self.intern_reflection(Reflectee::Type(t), loc)?)
    }

    /// `out` from a quiet evaluation, or the failure as a note.
    fn trait_result(
        &mut self,
        op: TraitOp,
        r: Result<Answer, LibFailure>,
        failure: Option<TraitFailure>,
        out: &mut MetaOut,
        loc: Loc,
    ) -> Result<(), MetaError> {
        match (failure, r) {
            (Some(TraitFailure::Incomplete(t)), _) => {
                fail(loc, format!("invalid use of incomplete type '{}'", self.ty_str(t)))
            }
            (Some(TraitFailure::BadOperand(t)), _) => {
                let t = self.ty_str(t);
                fail(
                    loc,
                    match op {
                        TraitOp::UnderlyingType => format!("'{t}' is not an enumeration type"),
                        TraitOp::MakeSigned | TraitOp::MakeUnsigned => format!(
                            "'{}' requires a non-bool integral or enumeration type, not '{t}'",
                            op.name()
                        ),
                        _ => format!("'{t}' is not a valid operand of '{}'", op.name()),
                    },
                )
            }
            (Some(TraitFailure::Unimplemented), _) => {
                fail(loc, format!("kiln: '{}' is not implemented yet", op.name()))
            }
            (None, Ok(Answer::Out(v))) => {
                *out = v;
                Ok(())
            }
            (None, Ok(Answer::Type(t))) => {
                *out = MetaOut::Info(self.type_reflection(t, loc)?);
                Ok(())
            }
            (None, Err(f)) => fail(loc, lib_failure_note(op, f)),
        }
    }
}

/// The failure of a trait that would instantiate or evaluate, asked in a
/// fold or a speculative trial: a speculative evaluation's notes are
/// dropped (the manifestly constant-evaluated evaluation, if any, runs
/// the trait itself), but the failure still carries a note, so that if
/// it ever reaches a reported evaluation it is not a bare "not a
/// constant expression" (WP-538).
fn speculative_refusal(op: TraitOp, loc: Loc) -> MetaError {
    MetaError::note(
        loc,
        format!(
            "'{}' is not evaluated during a speculative evaluation (it may instantiate)",
            op.name()
        ),
    )
}

/// The note for library trait failure `f` of `op`.
fn lib_failure_note(op: TraitOp, f: LibFailure) -> String {
    let (Shape::LibBool(name)
    | Shape::LibSize(name)
    | Shape::LibType(name)
    | Shape::LibIndexed(name)) = shape(op).0
    else {
        return format!("'{}' has no result for these operands", op.name());
    };
    match f {
        LibFailure::NotDeclared => format!(
            "'{}' needs 'std::{name}', which is not declared (include {})",
            op.name(),
            header_of(name)
        ),
        LibFailure::Incomplete => {
            format!("'std::{name}' is not defined for these operands")
        }
        LibFailure::NoMember => format!(
            "'std::{name}' has no member '{}' for these operands",
            if matches!(shape(op).0, Shape::LibType(_) | Shape::LibIndexed(_)) {
                "type"
            } else {
                "value"
            }
        ),
    }
}
