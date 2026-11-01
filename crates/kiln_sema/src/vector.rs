//! Vector types (spec/types.md §1 `Vector` / `DependentVector`, spec/sema.md
//! §5.8): clang's `ext_vector_type(N)` and GNU `vector_size(N)` attributes
//! on a typedef or alias-declaration (libc++ `__algorithm/simd_utils.h`
//! `using __simd_vector __attribute__((__ext_vector_type__(_Np))) = _Tp;`).
//!
//! Both attributes are consumed into the declared type and never attached
//! to the declaration (clang's AST keeps only the type). A value-dependent
//! size, or a dependent element of a `vector_size` vector, gives a
//! `DependentVector` whose operand is the attribute's expression as
//! written (the element count for `ext_vector_type`, the byte size for
//! `vector_size`, as clang's `DependentVectorType` /
//! `DependentSizedExtVectorType`); substitution rebuilds it through
//! [`Sema::build_vector_type`]. Operations on vector values are M4.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::ids::RawId;
use kiln_ast::types::{TypeView, VectorKind};
use kiln_ast::{AttrId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::conv;
use crate::decl::AttrArg;

impl Sema<'_, '_> {
    /// `ext_vector_type(N)` / `vector_size(N)` as written: exactly one
    /// expression argument, kept unevaluated until the attribute is
    /// applied to its declaration.
    pub(crate) fn vector_attribute(
        &mut self,
        kind: AttrKind,
        syntax: AttrSyntax,
        raw: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let n = raw.trim_start_matches("__").trim_end_matches("__").to_owned();
        let e = match args {
            [AttrArg::Expr(e)] => *e,
            [AttrArg::Type(_)] => {
                self.error(DiagId::parse_expected_expression, loc);
                return None;
            }
            _ => {
                let msg = self.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg(n).arg("1");
                self.emit(msg);
                return None;
            }
        };
        let node = AttrNode { kind, syntax, flags: 0, loc, a: e.raw(), b: 0 };
        Some(self.ast.attrs.push(node))
    }

    /// Applies (and removes from `attrs`) the vector attributes of a
    /// declaration of type `ty`. `typedef` is true for a typedef or
    /// alias-declaration, the only declarations `ext_vector_type` applies
    /// to; `vector_size` also applies to the type of a variable, field or
    /// parameter (on a function clang applies it to the return type: not
    /// supported yet). Returns the declared type.
    pub(crate) fn apply_vector_attrs(
        &mut self,
        ty: QualType,
        attrs: &mut Vec<AttrId>,
        typedef: bool,
    ) -> QualType {
        let mut ty = ty;
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if !matches!(node.kind, AttrKind::EXT_VECTOR_TYPE | AttrKind::VECTOR_SIZE) {
                i += 1;
                continue;
            }
            attrs.remove(i);
            let ext = node.kind == AttrKind::EXT_VECTOR_TYPE;
            if !typedef && ext {
                self.error(DiagId::sema_ext_vector_type_on_non_typedef, node.loc);
                continue;
            }
            if !typedef && self.ast.types.is_function(ty) {
                // clang applies it to the return type; not supported yet.
                let msg = self
                    .diag(DiagId::parse_unsupported, node.loc)
                    .arg("'vector_size' on a function declaration");
                self.emit(msg);
                continue;
            }
            if self.is_error_type(ty) {
                continue;
            }
            let kind = if ext { VectorKind::Ext } else { VectorKind::Generic };
            let Some(size) = ExprId::from_raw(node.a) else { continue };
            ty = self.build_vector_type(ty, size, kind, node.loc);
        }
        ty
    }

    /// The vector of `elem` sized by `size` (the element count for
    /// `VectorKind::Ext`, the byte size otherwise), clang's
    /// `BuildExtVectorType` / `BuildVectorType`; `ERROR` after a
    /// diagnostic.
    pub(crate) fn build_vector_type(
        &mut self,
        elem: QualType,
        size: ExprId,
        kind: VectorKind,
        loc: Loc,
    ) -> QualType {
        let ext = kind == VectorKind::Ext;
        let t = &self.ast.types;
        let elem_dep = t.is_dependent(elem);
        if !elem_dep {
            let ok = if ext {
                // An integer (`bool` included) or real floating type.
                t.is_integral(elem) || t.is_floating(elem)
            } else {
                !t.is_array(elem)
                    && t.builtin(elem).is_some()
                    && t.canonical(elem).ty() != QualType::BOOL.ty()
                    && (t.is_integral(elem) || t.is_floating(elem))
            };
            let already = matches!(t.canonical_view(elem), TypeView::Vector { .. });
            if !ok || already {
                let msg =
                    self.diag(DiagId::sema_vector_invalid_element_type, loc).arg(self.ty_arg(elem));
                self.emit(msg);
                return QualType::ERROR;
            }
        }
        if self.expr_is_value_dependent(size) || (!ext && elem_dep) {
            let p = self.dependent_profile(size);
            return self.ast.types.dependent_vector(elem, size, p, kind);
        }
        let name = if ext { "ext_vector_type" } else { "vector_size" };
        let Some(v) = self.vector_size_value(size, name, loc) else {
            return QualType::ERROR;
        };
        if v == 0 {
            self.error(DiagId::sema_vector_zero_size, loc);
            return QualType::ERROR;
        }
        let count = if ext {
            v
        } else {
            let esize = crate::target::size_align(&self.ast, elem, self.abi()).map_or(0, |s| s.0);
            if esize == 0 || v % u128::from(esize) != 0 {
                self.error(DiagId::sema_vector_size_not_multiple, loc);
                return QualType::ERROR;
            }
            v / u128::from(esize)
        };
        let Ok(count) = u32::try_from(count) else {
            self.error(DiagId::sema_vector_size_too_large, loc);
            return QualType::ERROR;
        };
        self.ast.types.vector(elem, count, kind)
    }

    /// The value of the size operand `e` of attribute `name`: an integral
    /// constant expression (clang `err_attribute_argument_type`).
    fn vector_size_value(&mut self, e: ExprId, name: &str, loc: Loc) -> Option<u128> {
        let ty = self.ast.exprs.get(e).ty();
        if self.is_error_type(ty) {
            return None;
        }
        if !self.is_integral_or_enum(ty) {
            let msg = self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(name);
            self.emit(msg);
            return None;
        }
        let c = conv::lvalue_to_rvalue(self, e);
        let c = self.act_on_full_expr(c);
        match self.eval_int(c, EvalContext::new(EvalKind::Other, self.begin_loc(e))) {
            Ok(v) if v < 0 => {
                self.error(DiagId::sema_vector_size_too_large, loc);
                None
            }
            Ok(v) => u128::try_from(v).ok(),
            Err(_) => {
                let msg =
                    self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(name);
                self.emit(msg);
                None
            }
        }
    }
}
