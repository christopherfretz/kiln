//! Integer helpers shared by sema's checks: the width and signedness of
//! integral types and two's-complement wrapping.
//!
//! Constant evaluation itself is the Oven's (`kiln_oven`, spec/oven.md),
//! reached through [`Sema::eval_const`](crate::Sema::eval_const) and
//! [`Sema::try_fold`](crate::Sema::try_fold) (spec/sema.md §8). The M2
//! tree-walking folder that used to live here was removed by WP-054.

use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{Ast, QualType};

/// `(width, signed)` of an integral or enumeration type.
pub fn int_info(ast: &Ast, ty: QualType) -> Option<(u32, bool)> {
    let t = &ast.types;
    let c = t.canonical(ty);
    match t.view(c.ty()) {
        TypeView::Builtin(b) => {
            if b == BuiltinType::Bool {
                return Some((1, false));
            }
            if !b.is_integral() {
                return None;
            }
            let bytes = t.target().builtin_size_align(b).0;
            let signed = match b {
                BuiltinType::Char => t.target().char_signed,
                BuiltinType::WChar => t.target().wchar_signed,
                _ => b.is_signed_integer(),
            };
            Some((u32::from(bytes) * 8, signed))
        }
        TypeView::Enum(d) => {
            let e = ast.decls.enumeration(ast.definition(d).unwrap_or(d))?;
            int_info(ast, e.int_type?)
        }
        _ => None,
    }
}

/// Truncates `v` to `(width, signed)`, wrapping.
pub fn wrap(v: i128, width: u32, signed: bool) -> i128 {
    if width >= 128 {
        return v;
    }
    let mask = (1i128 << width) - 1;
    let u = v & mask;
    if signed && width > 0 && (u >> (width - 1)) & 1 == 1 { u - (1i128 << width) } else { u }
}

/// True if `v` is representable in `(width, signed)`.
pub fn fits(v: i128, width: u32, signed: bool) -> bool {
    wrap(v, width, signed) == v
}
