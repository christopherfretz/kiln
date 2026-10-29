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
    /// Not a function, or not constant-evaluable in clang 19.
    No,
    /// Evaluated by the Oven.
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

/// The owner of the overload-resolution traits (spec §9.1 `init_check`,
/// `conv_check`).
pub const OVERLOAD_WP: &str = "WP-068a";

/// The arity of trait `t`.
pub fn arity(t: TypeTrait) -> Arity {
    use TypeTrait as T;
    match t {
        T::IsBaseOf
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
        | T::IsTriviallyRelocatable => Complete::Element,
        T::IsDestructible | T::IsNothrowDestructible | T::IsTriviallyDestructible => {
            Complete::BoundedElement
        }
        T::IsAggregate | T::IsFinal => Complete::Class,
        T::IsAbstract | T::IsEmpty | T::IsPolymorphic => Complete::NonUnionClass,
        _ => Complete::None,
    }
}

/// The trait clang's `-Wdeprecated-builtins` suggests instead of `t`.
pub fn deprecated_replacement(t: TypeTrait) -> Option<&'static str> {
    use TypeTrait as T;
    Some(match t {
        T::HasNothrowAssign => "__is_nothrow_assignable",
        T::HasNothrowMoveAssign => "__is_nothrow_assignable",
        T::HasNothrowCopy => "__is_nothrow_constructible",
        T::HasNothrowConstructor => "__is_nothrow_constructible",
        T::HasTrivialAssign => "__is_trivially_assignable",
        T::HasTrivialMoveAssign => "__is_trivially_assignable",
        T::HasTrivialCopy => "__is_trivially_copyable",
        T::HasTrivialDefaultConstructor => "__is_trivially_constructible",
        T::HasTrivialMoveConstructor => "__is_trivially_constructible",
        T::HasTrivialDestructor => "__is_trivially_destructible",
        _ => return None,
    })
}

/// The status of trait `t`.
pub fn trait_status(t: TypeTrait) -> Status {
    use TypeTrait as T;
    match t {
        // [meta.unary.prop] initialization and conversion checks: these
        // need the unevaluated `init_check` / `conv_check` of overload
        // resolution.
        T::IsConstructible
        | T::IsNothrowConstructible
        | T::IsTriviallyConstructible
        | T::IsAssignable
        | T::IsNothrowAssignable
        | T::IsTriviallyAssignable
        | T::IsConvertible
        | T::IsConvertibleTo
        | T::IsNothrowConvertible
        | T::ReferenceBindsToTemporary
        | T::ReferenceConstructsFromTemporary
        | T::ReferenceConvertsFromTemporary
        | T::HasNothrowAssign
        | T::HasNothrowConstructor
        | T::HasNothrowCopy
        | T::HasNothrowMoveAssign
        | T::IsTriviallyEqualityComparable => Status::Owner(OVERLOAD_WP),
        // Layout-compatibility and register-passing queries (not used by
        // the M3 headers).
        T::IsLayoutCompatible
        | T::IsPointerInterconvertibleBaseOf
        | T::CanPassInRegs
        | T::IsBitwiseCloneable => Status::Owner("M4 (unassigned)"),
        _ => Status::Done,
    }
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
const SRC_LOC: &str = "unowned: source-location builtins with __builtin_source_location";
const SRC_LOC_WP: Status = Status::Owner(SRC_LOC);
/// The owner of the §9.5 builtin declarations kiln does not declare yet.
/// The spec gives them to WP-063's builtin declaration table, but WP-063
/// shipped with `__builtin_is_constant_evaluated` only, so they need a new
/// package (spec/sema-templates.md §11, "Landed (WP-074)").
const DECL_WP: Status = Status::Owner("unowned: proposed §9.5 builtin-functions WP");
const OVEN_WP: &str = "unowned: proposed §9.5 builtin-functions WP";

/// The non-trait rows: §9.3 templates, then §9.4-9.5 functions.
pub static TABLE: &[Row] = &[
    Row {
        name: "__make_integer_seq",
        kind: Kind::Template,
        status: Status::Done,
        oven: Oven::No,
    },
    Row {
        name: "__type_pack_element",
        kind: Kind::Template,
        status: Status::Owner("WP-072 (pack expansion of Ts...)"),
        oven: Oven::No,
    },
    // §9.4
    f("__builtin_is_constant_evaluated", DONE, Oven::Yes),
    f("__builtin_constant_p", DECL_WP, Oven::Stub(OVEN_WP)),
    // §9.5 object model
    f("__builtin_addressof", DONE, Oven::Yes),
    f("__builtin_launder", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_bit_cast", DONE, Oven::Stub("unowned: Oven bit_cast through abi layout")),
    f("__builtin_unreachable", DECL_WP, Oven::Stub(OVEN_WP)),
    // allocation
    f("__builtin_operator_new", DECL_WP, Oven::Stub("WP-061b")),
    f("__builtin_operator_delete", DECL_WP, Oven::Stub("WP-061b")),
    // bits
    f("__builtin_clz", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_clzl", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_clzll", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_ctz", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_ctzl", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_ctzll", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_popcount", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_popcountl", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_popcountll", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_clzg", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_ctzg", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_popcountg", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_bswap16", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_bswap32", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_bswap64", DECL_WP, Oven::Stub(OVEN_WP)),
    // memory and strings
    f("__builtin_memcmp", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_memcpy", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_memmove", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_memchr", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_char_memchr", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_strlen", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_wcslen", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_wmemchr", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_wmemcmp", DECL_WP, Oven::Stub(OVEN_WP)),
    // float classification
    f("__builtin_isfinite", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_isinf", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_isnan", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_isnormal", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_signbit", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_isgreater", DECL_WP, Oven::No),
    f("__builtin_isgreaterequal", DECL_WP, Oven::No),
    f("__builtin_isless", DECL_WP, Oven::No),
    f("__builtin_islessequal", DECL_WP, Oven::No),
    f("__builtin_islessgreater", DECL_WP, Oven::No),
    f("__builtin_isunordered", DECL_WP, Oven::No),
    // float constants
    f("__builtin_huge_val", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_huge_valf", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_huge_vall", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_nan", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_nanf", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_nanl", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_nans", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_nansf", DECL_WP, Oven::Stub(OVEN_WP)),
    f("__builtin_nansl", DECL_WP, Oven::Stub(OVEN_WP)),
    // source location
    // These need the call-site context of a default argument (clang's
    // `SourceLocExpr` under `CXXDefaultArgExpr`); libc++ 19 uses
    // `__builtin_source_location` for `<source_location>` instead.
    f("__builtin_FILE", SRC_LOC_WP, Oven::Stub(SRC_LOC)),
    f("__builtin_LINE", SRC_LOC_WP, Oven::Stub(SRC_LOC)),
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
        assert_eq!(has_builtin_staged("__is_constructible"), Some(false));
        assert_eq!(has_builtin_staged("__is_convertible"), Some(false));
        assert_eq!(has_builtin_staged("__is_same"), Some(true));
        assert_eq!(has_builtin_staged("__remove_cvref"), Some(true));
        assert_eq!(has_builtin_staged("__builtin_memcpy"), Some(false));
        assert_eq!(has_builtin_staged("__builtin_expect"), None);
        assert_eq!(has_builtin_staged("__make_integer_seq"), Some(true));
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
