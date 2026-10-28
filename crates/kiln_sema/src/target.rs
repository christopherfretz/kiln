//! Target queries used by `sizeof`/`alignof`, bit-field checks and the
//! constant folder (spec/sema.md §9). Layout is `kiln_abi`'s Itanium
//! implementation (spec/abi.md); this module only picks the target.

use kiln_abi::{Arch, Target};
use kiln_ast::types::{FloatFormat, TypeView};
use kiln_ast::{Ast, QualType};

/// The M2 default target triple (docs/ARCHITECTURE.md).
pub const DEFAULT_TRIPLE: &str = "aarch64-unknown-linux-gnu";

/// The M2 default target, aarch64-unknown-linux-gnu.
pub fn default_target() -> Target {
    Target::for_arch(Arch::AArch64)
}

/// The `kiln_abi` target whose builtin table `ast` was built with (for
/// callers that hold only an [`Ast`]): the two supported targets differ
/// in their `long double` format.
pub fn target_of(ast: &Ast) -> Target {
    match ast.types.target().long_double {
        FloatFormat::X87Extended => Target::for_arch(Arch::X86_64),
        _ => Target::for_arch(Arch::AArch64),
    }
}

/// `(sizeof, alignof)` of `ty` in bytes, or `None` for incomplete,
/// function, `void` and dependent types (which `sizeof` rejects before
/// asking).
pub fn size_align(ast: &Ast, ty: QualType, target: &Target) -> Option<(u64, u64)> {
    if !is_complete_object(ast, ty) {
        return None;
    }
    let l = kiln_abi::type_size_align(ast, ty, target);
    Some((l.size, l.align))
}

/// True if `ty` is a complete object type (its size is known).
pub fn is_complete_object(ast: &Ast, ty: QualType) -> bool {
    let t = &ast.types;
    let c = t.canonical(ty);
    match t.view(c.ty()) {
        TypeView::Builtin(b) => t.target().builtin_size_align(b).0 != 0,
        TypeView::LValueRef(r) | TypeView::RValueRef(r) => is_complete_object(ast, r),
        TypeView::ConstantArray(a) => is_complete_object(ast, a.elem),
        TypeView::Enum(d) => {
            let def = ast.definition(d).unwrap_or(d);
            ast.decls.enumeration(def).is_some_and(|e| e.int_type.is_some())
        }
        TypeView::Record(d) => ast.definition(d).is_some(),
        TypeView::QualifiedArray { base, .. } => is_complete_object(ast, QualType::unqual(base)),
        TypeView::Pointer(_)
        | TypeView::MemberPointer { .. }
        | TypeView::BitInt { .. }
        | TypeView::Complex(_)
        | TypeView::Atomic(_)
        | TypeView::Vector { .. } => true,
        _ => false,
    }
}
