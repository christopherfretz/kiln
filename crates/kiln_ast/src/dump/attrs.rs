//! Attribute class names (clang's `Attr` subclasses).

use crate::attr::{AttrKind, AttrNode, AttrSyntax};

/// The clang class name of `a` without the `Attr` suffix, or `None` for
/// a kind kiln does not name (printed `UnknownAttr kind=N`).
pub(super) fn class_name(a: &AttrNode) -> Option<&'static str> {
    Some(match a.kind {
        AttrKind::ALIGNED => "Aligned",
        AttrKind::ASSUME => "CXXAssume",
        AttrKind::CARRIES_DEPENDENCY => "CarriesDependency",
        AttrKind::DEPRECATED => "Deprecated",
        AttrKind::FALLTHROUGH => "FallThrough",
        AttrKind::INDETERMINATE => "Indeterminate",
        AttrKind::UNINITIALIZED => "Uninitialized",
        AttrKind::LIKELY => "Likely",
        AttrKind::UNLIKELY => "Unlikely",
        AttrKind::MAYBE_UNUSED | AttrKind::UNUSED => "Unused",
        AttrKind::NODISCARD => "WarnUnusedResult",
        AttrKind::NORETURN => {
            if a.syntax == AttrSyntax::Cxx11 {
                "CXX11NoReturn"
            } else {
                "NoReturn"
            }
        }
        AttrKind::NO_UNIQUE_ADDRESS => "NoUniqueAddress",
        AttrKind::ANNOTATION => "Annotation",
        AttrKind::ALWAYS_INLINE => "AlwaysInline",
        AttrKind::NOINLINE => "NoInline",
        AttrKind::PACKED => "Packed",
        AttrKind::USED => "Used",
        AttrKind::WEAK => "Weak",
        AttrKind::VISIBILITY => "Visibility",
        AttrKind::SECTION => "Section",
        AttrKind::FORMAT => "Format",
        AttrKind::CONST => "Const",
        AttrKind::PURE => "Pure",
        AttrKind::COLD => "Cold",
        AttrKind::MALLOC => "Restrict",
        AttrKind::ALLOC_SIZE => "AllocSize",
        AttrKind::ALLOC_ALIGN => "AllocAlign",
        AttrKind::NOTHROW => "NoThrow",
        AttrKind::ENABLE_IF => "EnableIf",
        AttrKind::DIAGNOSE_IF => "DiagnoseIf",
        AttrKind::TRIVIAL_ABI => "TrivialABI",
        AttrKind::PREFERRED_NAME => "PreferredName",
        AttrKind::EXCLUDE_FROM_EXPLICIT_INSTANTIATION => "ExcludeFromExplicitInstantiation",
        AttrKind::ABI_TAG => "AbiTag",
        AttrKind::INTERNAL_LINKAGE => "InternalLinkage",
        AttrKind::LIFETIMEBOUND => "LifetimeBound",
        AttrKind::NO_SANITIZE => "NoSanitize",
        AttrKind::MODE => "Mode",
        AttrKind::VECTOR_SIZE => "VectorSize",
        AttrKind::EXT_VECTOR_TYPE => "ExtVectorType",
        AttrKind::ASM_LABEL => "AsmLabel",
        AttrKind::BUILTIN => "Builtin",
        AttrKind::USING_IF_EXISTS => "UsingIfExists",
        AttrKind::HOT => "Hot",
        AttrKind::FLAG_ENUM => "FlagEnum",
        AttrKind::ENUM_EXTENSIBILITY => "EnumExtensibility",
        AttrKind::CORO_ONLY_DESTROY_WHEN_COMPLETE => "CoroOnlyDestroyWhenComplete",
        AttrKind::OPTNONE => "OptimizeNone",
        AttrKind::MINSIZE => "MinSize",
        AttrKind::NAKED => "Naked",
        AttrKind::INIT_PRIORITY => "InitPriority",
        AttrKind::NONNULL => "NonNull",
        AttrKind::RETURNS_NONNULL => "ReturnsNonNull",
        AttrKind::LOOP_HINT => "LoopHint",
        _ => return None,
    })
}
