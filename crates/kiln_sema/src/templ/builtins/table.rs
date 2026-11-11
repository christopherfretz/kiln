//! The builtin `TABLE` (spec/sema-templates.md §9, §13.3): every
//! compiler-provided trait, transform, template and `__builtin_*`
//! function the M3 headers use, with its staging status. `__has_builtin`
//! answers from it ([`has_builtin_staged`]): a row kiln has not
//! implemented answers 0, so libc++ falls back to its library
//! implementation instead of hitting
//! `sema_unimplemented_template_feature`.

use kiln_ast::expr::TypeTrait;
use kiln_ast::types::UnaryTransformKind;

/// What kind of builtin a row is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// `__is_*` / `__has_*` / `__reference_*` (§9.1): a `TypeTraitExpr`.
    TypePredicate,
    /// `__array_rank` / `__array_extent` (§9.4): an `ArrayTypeTraitExpr`.
    ArrayTrait,
    /// `__is_lvalue_expr` / `__is_rvalue_expr`: an `ExpressionTraitExpr`.
    ExpressionTrait,
    /// `__remove_cv(T)` etc. (§9.2): a `UnaryTransform` type.
    Transform,
    /// `__make_integer_seq` / `__type_pack_element` (§9.3).
    Template,
    /// A `__builtin_*` function (§9.4-9.5).
    Function,
}

/// How many type operands a trait takes (clang's `getTypeTraitArity`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arity {
    /// Exactly `n`.
    Exact(u8),
    /// `n` or more.
    AtLeast(u8),
}

/// Whether kiln implements a row, or which work package does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// Implemented: `__has_builtin` gives clang's answer.
    Done,
    /// Not implemented: a use reports `sema_unimplemented_template_feature`
    /// and `__has_builtin` gives 0. The string names the owner.
    Owner(&'static str),
}

/// The Oven column of a function row (§9.5): whether the builtin is
/// evaluable in constant expressions, and who implements that.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Oven {
    /// Not a function, or not constant-evaluable by the Oven.
    No,
    /// Evaluated by the Oven. `__has_constexpr_builtin` then gives the
    /// clang table's answer at the `-fclang-compat` level, so a builtin
    /// clang only made `Constexpr` in 22 (the P0533 `abs`, `signbit` and
    /// comparison-macro rows, WP-393) still answers 0 below 22.
    Yes,
    /// Constant-evaluable in clang 19; the Oven side belongs to the named
    /// work package (`__has_constexpr_builtin` answers false until then).
    Stub(&'static str),
}

/// The completeness an operand of a unary trait needs (clang's
/// `CheckUnaryTypeTraitTypeCompleteness`, verified per trait against
/// clang 19 on `I`, `I[]`, `I[2]`, `void`, `I&`, `int()`, `union U`,
/// `enum E : int` and `const I`). A failing check is "incomplete type
/// 'X' used in type trait expression" at the keyword.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Complete {
    /// No requirement.
    None,
    /// The element type (all extents removed) must be complete or `void`.
    Element,
    /// As [`Complete::Element`], but an array of unknown bound is
    /// accepted (the destructible family answers false for it).
    BoundedElement,
    /// A class or union type must be complete (arrays are not looked
    /// through).
    Class,
    /// A non-union class type must be complete.
    NonUnionClass,
}

/// One builtin.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Row {
    /// The spelling.
    pub name: &'static str,
    /// What it is.
    pub kind: Kind,
    /// Implemented, or by whom.
    pub status: Status,
    /// The Oven column (functions only).
    pub oven: Oven,
}

/// The arity of trait `t`.
pub fn arity(t: TypeTrait) -> Arity {
    use TypeTrait as T;
    match t {
        T::IsBaseOf
        | T::IsVirtualBaseOf
        | T::IsSame
        | T::IsSameAs
        | T::IsAssignable
        | T::IsConvertible
        | T::IsConvertibleTo
        | T::IsLayoutCompatible
        | T::IsNothrowAssignable
        | T::IsNothrowConvertible
        | T::IsPointerInterconvertibleBaseOf
        | T::IsTriviallyAssignable
        | T::ReferenceBindsToTemporary
        | T::ReferenceConstructsFromTemporary
        | T::ReferenceConvertsFromTemporary
        | T::TypesCompatible
        | T::ArrayExtent => Arity::Exact(2),
        T::IsConstructible | T::IsNothrowConstructible | T::IsTriviallyConstructible => {
            Arity::AtLeast(1)
        }
        _ => Arity::Exact(1),
    }
}

/// The completeness trait `t` requires of its (first) operand.
pub fn completeness(t: TypeTrait) -> Complete {
    use TypeTrait as T;
    match t {
        T::HasNothrowAssign
        | T::HasNothrowConstructor
        | T::HasNothrowCopy
        | T::HasNothrowMoveAssign
        | T::HasTrivialAssign
        | T::HasTrivialDefaultConstructor
        | T::HasTrivialCopy
        | T::HasTrivialDestructor
        | T::HasTrivialMoveAssign
        | T::HasTrivialMoveConstructor
        | T::HasUniqueObjectRepresentations
        | T::HasVirtualDestructor
        | T::IsBitwiseCloneable
        | T::IsLiteral
        | T::IsLiteralType
        | T::IsPod
        | T::IsStandardLayout
        | T::IsTrivial
        | T::IsTriviallyCopyable
        | T::IsTriviallyEqualityComparable
        | T::IsTriviallyRelocatable
        | T::IsCppTriviallyRelocatable
        | T::CanPassInRegs => Complete::Element,
        T::IsDestructible | T::IsNothrowDestructible | T::IsTriviallyDestructible => {
            Complete::BoundedElement
        }
        T::IsAggregate | T::IsFinal | T::IsImplicitLifetime => Complete::Class,
        T::IsAbstract | T::IsEmpty | T::IsPolymorphic => Complete::NonUnionClass,
        _ => Complete::None,
    }
}

/// The trait clang's `-Wdeprecated-builtins` suggests instead of `t`.
pub fn deprecated_replacement(t: TypeTrait) -> Option<&'static str> {
    use TypeTrait as T;
    Some(match t {
        T::HasNothrowAssign | T::HasNothrowMoveAssign => "__is_nothrow_assignable",
        T::HasNothrowCopy | T::HasNothrowConstructor => "__is_nothrow_constructible",
        T::HasTrivialAssign | T::HasTrivialMoveAssign => "__is_trivially_assignable",
        T::HasTrivialCopy => "__is_trivially_copyable",
        T::HasTrivialDefaultConstructor | T::HasTrivialMoveConstructor => {
            "__is_trivially_constructible"
        }
        T::HasTrivialDestructor => "__is_trivially_destructible",
        _ => return None,
    })
}

/// The status of trait `t`.
pub fn trait_status(_t: TypeTrait) -> Status {
    // Every trait clang 19 knows is implemented (WP-166 finished the
    // `__has_nothrow_*`, layout and register-passing queries).
    Status::Done
}

/// The kind of trait `t`'s row.
pub fn trait_kind(t: TypeTrait) -> Kind {
    match t {
        TypeTrait::ArrayRank | TypeTrait::ArrayExtent => Kind::ArrayTrait,
        TypeTrait::IsLvalueExpr | TypeTrait::IsRvalueExpr => Kind::ExpressionTrait,
        _ => Kind::TypePredicate,
    }
}

/// The status of transform `k` (all of §9.2 is implemented).
pub fn transform_status(_k: UnaryTransformKind) -> Status {
    Status::Done
}

const fn f(name: &'static str, status: Status, oven: Oven) -> Row {
    Row { name, kind: Kind::Function, status, oven }
}

const DONE: Status = Status::Done;
/// The clang 22 builtins WP-263 left for a follow-up.
const LATER: Status = Status::Owner("WP-263 follow-up");

/// The non-trait rows: §9.3 templates, then §9.4-9.5 functions.
pub static TABLE: &[Row] = &[
    Row { name: "__make_integer_seq", kind: Kind::Template, status: Status::Done, oven: Oven::No },
    Row { name: "__type_pack_element", kind: Kind::Template, status: Status::Done, oven: Oven::No },
    // §9.4
    f("__builtin_is_constant_evaluated", DONE, Oven::Yes),
    f("__builtin_constant_p", DONE, Oven::Yes),
    f("__builtin_classify_type", DONE, Oven::Yes),
    // §9.5 object model
    f("__builtin_addressof", DONE, Oven::Yes),
    f("__builtin_launder", DONE, Oven::Yes),
    f("__builtin_complex", DONE, Oven::Yes),
    f("__builtin_bit_cast", DONE, Oven::Yes),
    f("__builtin_unreachable", DONE, Oven::Yes),
    // allocation
    f("__builtin_operator_new", DONE, Oven::Stub("WP-061b")),
    f("__builtin_operator_delete", DONE, Oven::Stub("WP-061b")),
    // bits
    f("__builtin_clz", DONE, Oven::Yes),
    f("__builtin_clzl", DONE, Oven::Yes),
    f("__builtin_clzll", DONE, Oven::Yes),
    f("__builtin_ctz", DONE, Oven::Yes),
    f("__builtin_ctzl", DONE, Oven::Yes),
    f("__builtin_ctzll", DONE, Oven::Yes),
    f("__builtin_popcount", DONE, Oven::Yes),
    f("__builtin_popcountl", DONE, Oven::Yes),
    f("__builtin_popcountll", DONE, Oven::Yes),
    f("__builtin_clzg", DONE, Oven::Yes),
    f("__builtin_ctzg", DONE, Oven::Yes),
    f("__builtin_popcountg", DONE, Oven::Yes),
    f("__builtin_bswap16", DONE, Oven::Yes),
    f("__builtin_bswap32", DONE, Oven::Yes),
    f("__builtin_bswap64", DONE, Oven::Yes),
    // memory and strings
    f("__builtin_memcmp", DONE, Oven::Yes),
    f("__builtin_memcpy", DONE, Oven::Yes),
    f("__builtin_memmove", DONE, Oven::Yes),
    f("__builtin_memchr", DONE, Oven::Yes),
    f("__builtin_char_memchr", DONE, Oven::Yes),
    f("__builtin_strlen", DONE, Oven::Yes),
    f("__builtin_wcslen", DONE, Oven::Yes),
    f("__builtin_wmemchr", DONE, Oven::Yes),
    f("__builtin_wmemcmp", DONE, Oven::Yes),
    f("__builtin_strcmp", DONE, Oven::Yes),
    f("__builtin_strncmp", DONE, Oven::Yes),
    f("__builtin_strchr", DONE, Oven::Yes),
    f("__builtin_strrchr", DONE, Oven::No),
    f("__builtin_strpbrk", DONE, Oven::No),
    f("__builtin_strstr", DONE, Oven::No),
    f("__builtin_wcscmp", DONE, Oven::Yes),
    f("__builtin_wcsncmp", DONE, Oven::Yes),
    f("__builtin_wcschr", DONE, Oven::Yes),
    // float classification
    f("__builtin_isfinite", DONE, Oven::Yes),
    f("__builtin_isinf", DONE, Oven::Yes),
    f("__builtin_isnan", DONE, Oven::Yes),
    f("__builtin_isnormal", DONE, Oven::Yes),
    f("__builtin_signbit", DONE, Oven::Yes),
    f("__builtin_signbitf", DONE, Oven::Yes),
    f("__builtin_signbitl", DONE, Oven::Yes),
    f("__builtin_isgreater", DONE, Oven::Yes),
    f("__builtin_isgreaterequal", DONE, Oven::Yes),
    f("__builtin_isless", DONE, Oven::Yes),
    f("__builtin_islessequal", DONE, Oven::Yes),
    f("__builtin_islessgreater", DONE, Oven::Yes),
    f("__builtin_isunordered", DONE, Oven::Yes),
    // float constants
    f("__builtin_huge_val", DONE, Oven::Yes),
    f("__builtin_huge_valf", DONE, Oven::Yes),
    f("__builtin_huge_vall", DONE, Oven::Yes),
    f("__builtin_nan", DONE, Oven::Yes),
    f("__builtin_nanf", DONE, Oven::Yes),
    f("__builtin_nanl", DONE, Oven::Yes),
    f("__builtin_nans", DONE, Oven::Yes),
    f("__builtin_nansf", DONE, Oven::Yes),
    f("__builtin_nansl", DONE, Oven::Yes),
    // source location ([support.srcloc]; `SourceLocExpr`, values at the
    // call site of a default argument)
    f("__builtin_FILE", DONE, Oven::Yes),
    f("__builtin_FILE_NAME", DONE, Oven::Yes),
    f("__builtin_FUNCTION", DONE, Oven::Yes),
    f("__builtin_LINE", DONE, Oven::Yes),
    f("__builtin_COLUMN", DONE, Oven::Yes),
    f("__builtin_source_location", DONE, Oven::Yes),
    // math (lowered to libm calls / LLVM intrinsics; not constant in
    // clang 19)
    f("__builtin_exp", DONE, Oven::No),
    f("__builtin_expf", DONE, Oven::No),
    f("__builtin_expl", DONE, Oven::No),
    f("__builtin_exp2", DONE, Oven::No),
    f("__builtin_exp2f", DONE, Oven::No),
    f("__builtin_exp2l", DONE, Oven::No),
    f("__builtin_expm1", DONE, Oven::No),
    f("__builtin_expm1f", DONE, Oven::No),
    f("__builtin_expm1l", DONE, Oven::No),
    f("__builtin_frexp", DONE, Oven::No),
    f("__builtin_frexpf", DONE, Oven::No),
    f("__builtin_frexpl", DONE, Oven::No),
    f("__builtin_ldexp", DONE, Oven::No),
    f("__builtin_ldexpf", DONE, Oven::No),
    f("__builtin_ldexpl", DONE, Oven::No),
    f("__builtin_pow", DONE, Oven::No),
    f("__builtin_powf", DONE, Oven::No),
    f("__builtin_powl", DONE, Oven::No),
    f("__builtin_scalbn", DONE, Oven::No),
    f("__builtin_scalbnf", DONE, Oven::No),
    f("__builtin_scalbnl", DONE, Oven::No),
    f("__builtin_scalbln", DONE, Oven::No),
    f("__builtin_scalblnf", DONE, Oven::No),
    f("__builtin_scalblnl", DONE, Oven::No),
    // WP-144: constant-evaluated libm builtins (clang 19 `Constexpr`)
    f("__builtin_fabs", DONE, Oven::Yes),
    f("__builtin_fabsf", DONE, Oven::Yes),
    f("__builtin_fabsl", DONE, Oven::Yes),
    f("__builtin_copysign", DONE, Oven::Yes),
    f("__builtin_copysignf", DONE, Oven::Yes),
    f("__builtin_copysignl", DONE, Oven::Yes),
    f("__builtin_fmax", DONE, Oven::Yes),
    f("__builtin_fmaxf", DONE, Oven::Yes),
    f("__builtin_fmaxl", DONE, Oven::Yes),
    f("__builtin_fmin", DONE, Oven::Yes),
    f("__builtin_fminf", DONE, Oven::Yes),
    f("__builtin_fminl", DONE, Oven::Yes),
    f("__builtin_inf", DONE, Oven::Yes),
    f("__builtin_inff", DONE, Oven::Yes),
    f("__builtin_infl", DONE, Oven::Yes),
    f("__builtin_isinf_sign", DONE, Oven::Yes),
    f("__builtin_fpclassify", DONE, Oven::Yes),
    f("__builtin_issubnormal", DONE, Oven::Yes),
    f("__builtin_iszero", DONE, Oven::Yes),
    f("__builtin_issignaling", DONE, Oven::Yes),
    // WP-144: intrinsic-lowered, not constant in clang 19; the other
    // libm functions (`builtin::LIBM`) fall back to clang's answer
    f("__builtin_ceil", DONE, Oven::No),
    f("__builtin_ceilf", DONE, Oven::No),
    f("__builtin_ceill", DONE, Oven::No),
    f("__builtin_floor", DONE, Oven::No),
    f("__builtin_floorf", DONE, Oven::No),
    f("__builtin_floorl", DONE, Oven::No),
    f("__builtin_trunc", DONE, Oven::No),
    f("__builtin_truncf", DONE, Oven::No),
    f("__builtin_truncl", DONE, Oven::No),
    f("__builtin_round", DONE, Oven::No),
    f("__builtin_roundf", DONE, Oven::No),
    f("__builtin_roundl", DONE, Oven::No),
    f("__builtin_rint", DONE, Oven::No),
    f("__builtin_rintf", DONE, Oven::No),
    f("__builtin_rintl", DONE, Oven::No),
    f("__builtin_nearbyint", DONE, Oven::No),
    f("__builtin_nearbyintf", DONE, Oven::No),
    f("__builtin_nearbyintl", DONE, Oven::No),
    f("__builtin_fma", DONE, Oven::No),
    f("__builtin_fmaf", DONE, Oven::No),
    f("__builtin_fmal", DONE, Oven::No),
    f("__builtin_abs", DONE, Oven::Yes),
    f("__builtin_labs", DONE, Oven::Yes),
    f("__builtin_llabs", DONE, Oven::Yes),
    // WP-156: vectors
    f("__builtin_reduce_add", DONE, Oven::No),
    f("__builtin_reduce_and", DONE, Oven::No),
    f("__builtin_reduce_max", DONE, Oven::No),
    f("__builtin_reduce_min", DONE, Oven::No),
    f("__builtin_reduce_mul", DONE, Oven::No),
    f("__builtin_reduce_or", DONE, Oven::No),
    f("__builtin_reduce_xor", DONE, Oven::No),
    // WP-150: the remaining GCC integer and memory builtins
    // (`builtin::INT_MEM`); `memset`/`mempcpy` are not constant in clang 19
    f("__builtin_ffs", DONE, Oven::Yes),
    f("__builtin_ffsl", DONE, Oven::Yes),
    f("__builtin_ffsll", DONE, Oven::Yes),
    f("__builtin_parity", DONE, Oven::Yes),
    f("__builtin_parityl", DONE, Oven::Yes),
    f("__builtin_parityll", DONE, Oven::Yes),
    f("__builtin_clrsb", DONE, Oven::Yes),
    f("__builtin_clrsbl", DONE, Oven::Yes),
    f("__builtin_clrsbll", DONE, Oven::Yes),
    f("__builtin_bitreverse8", DONE, Oven::Yes),
    f("__builtin_bitreverse16", DONE, Oven::Yes),
    f("__builtin_bitreverse32", DONE, Oven::Yes),
    f("__builtin_bitreverse64", DONE, Oven::Yes),
    f("__builtin_rotateleft8", DONE, Oven::Yes),
    f("__builtin_rotateleft16", DONE, Oven::Yes),
    f("__builtin_rotateleft32", DONE, Oven::Yes),
    f("__builtin_rotateleft64", DONE, Oven::Yes),
    f("__builtin_rotateright8", DONE, Oven::Yes),
    f("__builtin_rotateright16", DONE, Oven::Yes),
    f("__builtin_rotateright32", DONE, Oven::Yes),
    f("__builtin_rotateright64", DONE, Oven::Yes),
    f("__builtin_bcmp", DONE, Oven::Yes),
    f("__builtin_wmemcpy", DONE, Oven::Yes),
    f("__builtin_wmemmove", DONE, Oven::Yes),
    f("__builtin_memset", DONE, Oven::No),
    f("__builtin_mempcpy", DONE, Oven::No),
    // WP-270: checked arithmetic (constant-evaluated), alloca and the
    // printf-family library builtins (not constant in clang 19)
    f("__builtin_add_overflow", DONE, Oven::Yes),
    f("__builtin_sub_overflow", DONE, Oven::Yes),
    f("__builtin_mul_overflow", DONE, Oven::Yes),
    f("__builtin_sadd_overflow", DONE, Oven::Yes),
    f("__builtin_saddl_overflow", DONE, Oven::Yes),
    f("__builtin_saddll_overflow", DONE, Oven::Yes),
    f("__builtin_uadd_overflow", DONE, Oven::Yes),
    f("__builtin_uaddl_overflow", DONE, Oven::Yes),
    f("__builtin_uaddll_overflow", DONE, Oven::Yes),
    f("__builtin_ssub_overflow", DONE, Oven::Yes),
    f("__builtin_ssubl_overflow", DONE, Oven::Yes),
    f("__builtin_ssubll_overflow", DONE, Oven::Yes),
    f("__builtin_usub_overflow", DONE, Oven::Yes),
    f("__builtin_usubl_overflow", DONE, Oven::Yes),
    f("__builtin_usubll_overflow", DONE, Oven::Yes),
    f("__builtin_smul_overflow", DONE, Oven::Yes),
    f("__builtin_smull_overflow", DONE, Oven::Yes),
    f("__builtin_smulll_overflow", DONE, Oven::Yes),
    f("__builtin_umul_overflow", DONE, Oven::Yes),
    f("__builtin_umull_overflow", DONE, Oven::Yes),
    f("__builtin_umulll_overflow", DONE, Oven::Yes),
    // WP-557: the multiprecision carry builtins (constant-evaluated)
    f("__builtin_addcb", DONE, Oven::Yes),
    f("__builtin_addcs", DONE, Oven::Yes),
    f("__builtin_addc", DONE, Oven::Yes),
    f("__builtin_addcl", DONE, Oven::Yes),
    f("__builtin_addcll", DONE, Oven::Yes),
    f("__builtin_subcb", DONE, Oven::Yes),
    f("__builtin_subcs", DONE, Oven::Yes),
    f("__builtin_subc", DONE, Oven::Yes),
    f("__builtin_subcl", DONE, Oven::Yes),
    f("__builtin_subcll", DONE, Oven::Yes),
    f("__builtin_alloca", DONE, Oven::No),
    f("__builtin_alloca_with_align", DONE, Oven::No),
    f("__builtin_vsnprintf", DONE, Oven::No),
    f("__builtin_snprintf", DONE, Oven::No),
    f("__builtin_sprintf", DONE, Oven::No),
    // WP-282: the remaining library builtins the headers probe (the
    // `*f128` family is answered from the kiln_ast builtin table).
    f("__builtin_assume_aligned", DONE, Oven::Yes),
    f("__builtin_object_size", DONE, Oven::Yes),
    f("__builtin_dynamic_object_size", DONE, Oven::Yes),
    f("__builtin_powi", DONE, Oven::No),
    f("__builtin_powif", DONE, Oven::No),
    f("__builtin_powil", DONE, Oven::No),
    f("__builtin_shufflevector", DONE, Oven::No),
    f("__builtin_verbose_trap", DONE, Oven::No),
    f("__builtin_copysignf16", DONE, Oven::No),
    // WP-432.
    f("__builtin_return_address", DONE, Oven::No),
    f("__builtin_elementwise_add_sat", DONE, Oven::Yes),
    f("__builtin_elementwise_sub_sat", DONE, Oven::Yes),
    // Added by clang 20-22 and probed by the C++26 headers; staged off
    // until implemented (spec/sema.md "clang 22 builtins"), so
    // `__has_builtin` answers 0 at every `-fclang-compat` level.
    Row { name: "__builtin_common_type", kind: Kind::Template, status: LATER, oven: Oven::No },
    Row {
        name: "__builtin_structured_binding_size",
        kind: Kind::TypePredicate,
        status: LATER,
        oven: Oven::No,
    },
    Row {
        name: "__builtin_lt_synthesizes_from_spaceship",
        kind: Kind::TypePredicate,
        status: LATER,
        oven: Oven::No,
    },
    Row {
        name: "__builtin_le_synthesizes_from_spaceship",
        kind: Kind::TypePredicate,
        status: LATER,
        oven: Oven::No,
    },
    Row {
        name: "__builtin_gt_synthesizes_from_spaceship",
        kind: Kind::TypePredicate,
        status: LATER,
        oven: Oven::No,
    },
    Row {
        name: "__builtin_ge_synthesizes_from_spaceship",
        kind: Kind::TypePredicate,
        status: LATER,
        oven: Oven::No,
    },
    f("__builtin_invoke", LATER, Oven::No),
    // WP-418: sema `builtin_is_within_lifetime`, the Oven `bi::LIFETIME`.
    f("__builtin_is_within_lifetime", DONE, Oven::Yes),
    // WP-473: kiln extension, the Oven's `Op::IsConstEval` (aux 2).
    f("__kiln_plainly_constant_evaluated", DONE, Oven::Yes),
    // WP-484: GCC's (libstdc++ `<meta>` declares `std::is_string_literal`
    // over it); sema `builtin_is_string_literal`, the Oven `bi::STRING_LIT`.
    f("__builtin_is_string_literal", DONE, Oven::Yes),
    // WP-757: GCC's P1494R5 observable checkpoint (libstdc++ `<utility>`
    // declares `std::observable_checkpoint` over it); codegen lowers it to
    // an opaque memory barrier; the Oven evaluates it as a no-op, as GCC
    // does (spec/codegen.md "Observable checkpoints").
    f("__builtin_observable_checkpoint", DONE, Oven::Yes),
    // WP-773: GCC's P3726R2 `std::start_lifetime` builtin; sema
    // `builtin_start_lifetime`, the Oven `bi::START_LIFETIME`, a no-op in
    // codegen (spec/oven.md "C++26 union lifetime").
    f("__builtin_start_lifetime", DONE, Oven::Yes),
    // WP-774: GCC's (libstdc++'s and libc++ 22's atomics test for it);
    // sema `builtin_clear_padding`, codegen `emit_clear_padding`, a
    // validated no-op in the Oven (spec/codegen.md "Clearing padding").
    f("__builtin_clear_padding", DONE, Oven::Yes),
];

/// The row for builtin `name`, if `name` is in the table.
pub fn lookup(name: &str) -> Option<Row> {
    if let Some(t) = TypeTrait::from_name(name) {
        return Some(Row {
            name: t.spelling(),
            kind: trait_kind(t),
            status: trait_status(t),
            oven: Oven::No,
        });
    }
    if let Some(k) = transform_by_name(name) {
        return Some(Row {
            name: k.spelling(),
            kind: Kind::Transform,
            status: transform_status(k),
            oven: Oven::No,
        });
    }
    TABLE.iter().find(|r| r.name == name).copied()
}

/// The transform spelled `name`.
pub fn transform_by_name(name: &str) -> Option<UnaryTransformKind> {
    (0u8..).map_while(UnaryTransformKind::from_u8).find(|k| k.spelling() == name)
}

/// The staged `__has_builtin(name)` answer: `Some(false)` for a row kiln
/// has not implemented, `Some(true)` for an implemented one, `None` when
/// `name` is not in the table (the caller answers as clang 19 does).
pub fn has_builtin_staged(name: &str) -> Option<bool> {
    lookup(name).map(|r| r.status == Status::Done)
}

/// Whether `name` is a GCC builtin clang does not have, which kiln
/// implements and reports through `__has_builtin` (WP-484, WP-757,
/// WP-773, WP-774).
pub fn gcc_only(name: &str) -> bool {
    matches!(
        name,
        "__builtin_is_string_literal"
            | "__builtin_observable_checkpoint"
            | "__builtin_start_lifetime"
            | "__builtin_clear_padding"
    )
}

/// The staged `__has_constexpr_builtin(name)` answer: `Some(false)` for a
/// function row whose Oven side is not implemented.
pub fn has_constexpr_builtin_staged(name: &str) -> Option<bool> {
    lookup(name).filter(|r| r.kind == Kind::Function).map(|r| r.oven == Oven::Yes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_trait_has_a_row() {
        for &t in TypeTrait::all() {
            let r = lookup(t.spelling()).expect("trait row");
            assert_eq!(r.name, t.spelling());
        }
    }

    #[test]
    fn every_transform_has_a_row() {
        for k in (0u8..).map_while(UnaryTransformKind::from_u8) {
            assert_eq!(lookup(k.spelling()).map(|r| r.kind), Some(Kind::Transform));
        }
    }

    #[test]
    fn overload_traits_are_staged_off() {
        assert_eq!(has_builtin_staged("__is_trivially_constructible"), Some(true));
        assert_eq!(has_builtin_staged("__is_trivially_assignable"), Some(true));
        assert_eq!(has_builtin_staged("__has_nothrow_assign"), Some(true));
        assert_eq!(has_builtin_staged("__is_layout_compatible"), Some(true));
        assert_eq!(has_builtin_staged("__type_pack_element"), Some(true));
        assert_eq!(has_builtin_staged("__is_constructible"), Some(true));
        assert_eq!(has_builtin_staged("__is_convertible"), Some(true));
        assert_eq!(has_builtin_staged("__is_same"), Some(true));
        assert_eq!(has_builtin_staged("__remove_cvref"), Some(true));
        assert_eq!(has_builtin_staged("__builtin_memcpy"), Some(true));
        assert_eq!(has_builtin_staged("__builtin_constant_p"), Some(true));
        assert_eq!(has_constexpr_builtin_staged("__builtin_strlen"), Some(true));
        assert_eq!(has_constexpr_builtin_staged("__builtin_constant_p"), Some(true));
        assert_eq!(has_builtin_staged("__builtin_expect"), None);
        assert_eq!(has_builtin_staged("__make_integer_seq"), Some(true));
    }

    /// WP-270: the checked-arithmetic builtins are constant-evaluated
    /// (clang 19 `Constexpr`); `alloca` and `vsnprintf` are not.
    #[test]
    fn checked_arithmetic_rows() {
        assert_eq!(has_constexpr_builtin_staged("__builtin_mul_overflow"), Some(true));
        assert_eq!(has_constexpr_builtin_staged("__builtin_umulll_overflow"), Some(true));
        assert_eq!(has_constexpr_builtin_staged("__builtin_alloca"), Some(false));
        assert_eq!(has_builtin_staged("__builtin_vsnprintf"), Some(true));
    }

    /// WP-557: every carry builtin is declared, staged on and
    /// constant-evaluated (clang 19 `Constexpr`).
    #[test]
    fn carry_rows() {
        for op in ["add", "sub"] {
            for w in ["b", "s", "", "l", "ll"] {
                let n = format!("__builtin_{op}c{w}");
                assert_eq!(has_builtin_staged(&n), Some(true), "{n}");
                assert_eq!(has_constexpr_builtin_staged(&n), Some(true), "{n}");
                let id = kiln_ast::decl::builtin::by_name(&n).expect("declared");
                assert_eq!(kiln_ast::decl::builtin::carry_op(id), Some(op == "sub"), "{n}");
            }
        }
    }

    /// WP-393 ([c.math]/[library.c], P0533): the Oven folds `abs`, `signbit`
    /// and the comparison macros, so their rows say `Yes`; the clang table
    /// decides the compat-level answer.
    #[test]
    fn p0533_rows() {
        for n in [
            "__builtin_abs",
            "__builtin_labs",
            "__builtin_llabs",
            "__builtin_signbit",
            "__builtin_isgreater",
            "__builtin_isgreaterequal",
            "__builtin_isless",
            "__builtin_islessequal",
            "__builtin_islessgreater",
            "__builtin_isunordered",
        ] {
            assert_eq!(has_constexpr_builtin_staged(n), Some(true), "{n}");
        }
    }

    #[test]
    fn wp282_library_rows() {
        assert_eq!(has_constexpr_builtin_staged("__builtin_object_size"), Some(true));
        assert_eq!(has_constexpr_builtin_staged("__builtin_assume_aligned"), Some(true));
        assert_eq!(has_constexpr_builtin_staged("__builtin_shufflevector"), Some(false));
        assert_eq!(has_builtin_staged("__builtin_verbose_trap"), Some(true));
        assert_eq!(has_builtin_staged("__builtin_powil"), Some(true));
    }

    #[test]
    fn clang22_builtins() {
        for n in [
            "__builtin_is_virtual_base_of",
            "__builtin_is_implicit_lifetime",
            "__builtin_is_cpp_trivially_relocatable",
            "__builtin_is_within_lifetime",
        ] {
            assert_eq!(has_builtin_staged(n), Some(true), "{n}");
        }
        for n in [
            "__builtin_common_type",
            "__builtin_structured_binding_size",
            "__builtin_lt_synthesizes_from_spaceship",
            "__builtin_invoke",
        ] {
            assert_eq!(has_builtin_staged(n), Some(false), "{n}");
        }
    }

    #[test]
    fn table_names_are_unique() {
        let mut names: Vec<&str> = TABLE.iter().map(|r| r.name).collect();
        names.sort_unstable();
        let n = names.len();
        names.dedup();
        assert_eq!(names.len(), n);
    }
}
