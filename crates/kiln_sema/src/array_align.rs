//! An array whose element size is not a multiple of the element's
//! alignment ([dcl.array]/1 with [basic.align]/1, [expr.sizeof]/2: the
//! elements of an array are contiguous, so each must be aligned; clang
//! `BuildArrayType`'s `err_array_element_alignment`; spec/sema.md
//! "Arrays of over-aligned elements"; WP-1595).
//!
//! Every complete type's size is a multiple of its alignment except a
//! typedef (or alias) whose `__attribute__((aligned(N)))` raises the
//! alignment above the size: `typedef int I __attribute__((aligned(16)));`
//! is 4 bytes aligned to 16. So the check runs only when the element type
//! is sugar (one bit) with a typedef carrying attributes
//! (`DeclFlags::HAS_ATTRS`) on its desugaring chain; a canonical element
//! type costs one flag test. A template argument is canonical, so `T[2]`
//! with `T = I` is never checked (as in clang and GCC).

use kiln_ast::QualType;
use kiln_ast::decl::DeclFlags;
use kiln_ast::types::TypeView;
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// Reports `size of array element ... isn't a multiple of its
    /// alignment` at `loc` (the `[` of the bound) when an array of `elem`
    /// would place its elements off their alignment. Returns whether it
    /// did: the array type is then not formed (clang's `BuildArrayType`
    /// returns no type).
    pub(crate) fn check_array_element_alignment(&mut self, elem: QualType, loc: Loc) -> bool {
        if self.ast.types.is_canonical(elem.ty()) || !self.has_attributed_typedef(elem) {
            return false;
        }
        if self.is_dependent_type(elem) || !self.is_complete(elem) {
            return false;
        }
        let Some((size, align)) = crate::target::size_align(&self.ast, elem, &self.abi) else {
            return false;
        };
        if align <= 1 || size % align == 0 {
            return false;
        }
        let msg = self
            .diag(DiagId::sema_array_element_alignment, loc)
            .arg(self.ty_arg(elem))
            .arg(size.to_string())
            .arg(align.to_string());
        self.emit(msg);
        true
    }

    /// Whether the desugaring chain of `q` names a typedef or alias that
    /// carries attributes (bounded).
    fn has_attributed_typedef(&self, mut q: QualType) -> bool {
        for _ in 0..64 {
            if self.ast.types.is_canonical(q.ty()) {
                return false;
            }
            if let TypeView::Typedef { decl, .. } = self.ast.types.view(q.ty())
                && self.ast.decls.get(decl).has(DeclFlags::HAS_ATTRS)
            {
                return true;
            }
            let d = self.ast.types.desugar_once(q);
            if d.ty() == q.ty() {
                return false;
            }
            q = d;
        }
        false
    }
}
