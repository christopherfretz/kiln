//! An array whose element size is not a multiple of the element's
//! alignment ([dcl.array]/1 with [basic.align]/1, [expr.sizeof]/2: the
//! elements of an array are contiguous, so each must be aligned; clang
//! `BuildArrayType`'s `err_array_element_alignment`; spec/sema.md
//! "Arrays of over-aligned elements"; WP-1595).
//!
//! Every complete type's size is a multiple of its alignment except a
//! typedef (or alias) whose `__attribute__((aligned(N)))` raises the
//! alignment above the size: `typedef int I __attribute__((aligned(16)));`
//! is 4 bytes aligned to 16, and an enumeration with the attribute
//! (`enum __attribute__((aligned(8))) E : char` is 1 byte aligned to 8,
//! in clang and GCC; WP-1604). So the check runs only when the element
//! type is sugar (one bit) with a typedef carrying attributes
//! (`DeclFlags::HAS_ATTRS`) on its desugaring chain, or an enumeration
//! whose declaration carries attributes; any other canonical element type
//! costs a flag test. A template argument is canonical, so `T[2]`
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
        let sugar = !self.ast.types.is_canonical(elem.ty()) && self.has_attributed_typedef(elem);
        if !sugar && !self.is_attributed_enum(elem) {
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

    /// Whether `q` is an enumeration with attributes on a declaration
    /// (an `aligned` one raises its alignment above its size).
    fn is_attributed_enum(&self, q: QualType) -> bool {
        let TypeView::Enum(d) = self.ast.types.canonical_view(q) else { return false };
        let def = self.ast.definition(d).unwrap_or(d);
        [self.ast.first(d), def].iter().any(|&x| self.ast.decls.get(x).has(DeclFlags::HAS_ATTRS))
    }

    /// Whether the layout sugar of `q` names a typedef or alias that
    /// carries attributes (`Ast::layout_attributed_typedef`, the rule
    /// layout and the Oven share).
    fn has_attributed_typedef(&self, q: QualType) -> bool {
        self.ast.layout_attributed_typedef(q)
    }
}
