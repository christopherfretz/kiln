//! Target queries used by `sizeof`/`alignof`, bit-field checks and the
//! constant folder (spec/sema.md §9). Layout is `kiln_abi`'s Itanium
//! implementation (spec/abi.md); this module only picks the target.

pub use kiln_abi::{Arch, Target};
use kiln_ast::types::{FloatFormat, TargetTypes, TypeView};
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
    target_for_types(ast.types.target())
}

/// The `kiln_abi` target with builtin table `types` (the two supported
/// targets differ in their `long double` format); its builtin table is
/// `types` itself.
pub fn target_for_types(types: &TargetTypes) -> Target {
    let mut t = match types.long_double {
        FloatFormat::X87Extended => Target::for_arch(Arch::X86_64),
        _ => Target::for_arch(Arch::AArch64),
    };
    t.types = types.clone();
    t
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

impl crate::Sema<'_, '_> {
    /// `(sizeof, alignof)` of `ty` in bytes, as [`size_align`], laid out
    /// over the per-Sema [`kiln_abi::LayoutCache`] (spec/sema-class.md
    /// §6.5): a complete class, and every complete class it contains, is
    /// laid out once per TU. A class still being defined is laid out over
    /// the cached layouts of its complete bases and members and then
    /// dropped (`Layouter::into_complete_cache`; WP-428, WP-515).
    pub(crate) fn size_align(&self, ty: QualType) -> Option<(u64, u64)> {
        if !is_complete_object(&self.ast, ty) {
            return None;
        }
        let l = self.with_layouter(|lay| lay.type_layout(ty));
        Some((l.size, l.align))
    }

    /// Runs `f` on a layout session resuming the per-Sema cache, then
    /// keeps the layouts of the complete classes it laid out (§6.5). The
    /// session cannot call back into sema, so the cache is never borrowed
    /// twice.
    pub(crate) fn with_layouter<R>(&self, f: impl FnOnce(&mut kiln_abi::Layouter<'_>) -> R) -> R {
        let cache = self.layout_cache.take();
        let mut lay = kiln_abi::Layouter::with_cache(&self.ast, &self.abi, cache);
        let r = f(&mut lay);
        *self.layout_cache.borrow_mut() = lay.into_complete_cache();
        r
    }
}

/// True if `ty` is a complete object type (its size is known).
pub fn is_complete_object(ast: &Ast, ty: QualType) -> bool {
    let t = &ast.types;
    let c = t.canonical(ty);
    match t.view(c.ty()) {
        TypeView::Builtin(b) => b.is_sizeless() || t.target().builtin_size_align(b).0 != 0,
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
