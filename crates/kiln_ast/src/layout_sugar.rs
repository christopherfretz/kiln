//! The type sugar that layout reads: a typedef (or alias) carrying GNU
//! `__attribute__((aligned(N)))` raises the alignment of the type it names
//! (clang `ASTContext::getTypeInfo` of a `TypedefType`), so the alignment
//! of a type is not always its canonical type's. This module is the one
//! rule for which sugar carries that alignment; `kiln_abi`'s layout
//! (`type_info`), the Oven's `alignof` and Sema's array-element check ask
//! it (spec/abi.md "Typedef-carried alignment").
//!
//! The rule is clang's: the alignment walk follows every sugar node
//! (typedef, using, elaborated, paren, alias template-id, `decltype`) and
//! goes through the cv-qualifiers and the element type of an array
//! (cv-qualifiers do not change alignment, [basic.type.qualifier]/3; an
//! array's alignment is its element's). A type substituted for a template
//! type parameter of a function, variable or class template is read
//! canonical: clang instantiates those from the canonical converted
//! arguments, so `f<D>()` with `typedef double D
//! __attribute__((aligned(32)))` sees `alignof(U) == 8`, as does GCC.
//! kiln keeps the first use's sugar in a function specialization's
//! arguments (for diagnostics), which would otherwise make the result
//! depend on how the first call spelled its argument. An alias template's
//! arguments keep their sugar (clang 22 substitutes them as written; at
//! `-fclang-compat=19` they are canonical already).

use crate::Ast;
use crate::decl::{DeclFlags, DeclKind};
use crate::types::TypeView;
use crate::{DeclId, QualType};

impl Ast {
    /// One step of the desugaring layout follows: `desugar_once`, except
    /// that a substituted template type parameter whose template is not an
    /// alias template is its canonical type (see the module docs).
    #[must_use]
    pub fn layout_desugar_once(&self, q: QualType) -> QualType {
        if let TypeView::SubstTemplateTypeParm { associated, .. } = self.types.view(q.ty())
            && !self.subst_keeps_layout_sugar(associated)
        {
            return self.types.canonical(q);
        }
        self.types.desugar_once(q)
    }

    /// Whether a replacement substituted for a parameter of `associated`
    /// (the `SubstTemplateTypeParm`'s template) keeps its typedef sugar for
    /// layout: only an alias template's does.
    #[must_use]
    pub fn subst_keeps_layout_sugar(&self, associated: DeclId) -> bool {
        self.decls.get(associated).kind() == DeclKind::TypeAliasTemplate
    }

    /// Whether the layout of `q` reads a typedef that carries attributes
    /// (a superset of "carries `aligned`": the caller then asks the ABI for
    /// the exact alignment). Walks [`Self::layout_desugar_once`] and,
    /// through cv-qualified arrays and array element types, into the
    /// element (`alignof(const T)` and `alignof(T[2])` for an aligned array
    /// typedef `T`). A canonical type answers at once.
    #[must_use]
    pub fn layout_attributed_typedef(&self, q: QualType) -> bool {
        let mut x = q;
        loop {
            if self.types.is_canonical(x.ty()) {
                // A canonical array's element is canonical too.
                return false;
            }
            match self.types.view(x.ty()) {
                TypeView::Typedef { decl, .. }
                    if self.decls.get(decl).has(DeclFlags::HAS_ATTRS) =>
                {
                    return true;
                }
                // `desugar_once` of a qualified array is its canonical
                // type, which drops the typedef in its base.
                TypeView::QualifiedArray { base, .. } => {
                    x = QualType::unqual(base);
                    continue;
                }
                TypeView::ConstantArray(a) => {
                    x = a.elem;
                    continue;
                }
                TypeView::IncompleteArray(elem) => {
                    x = elem;
                    continue;
                }
                _ => {}
            }
            let d = self.layout_desugar_once(x);
            if d.ty() == x.ty() {
                return false;
            }
            x = d;
        }
    }
}
