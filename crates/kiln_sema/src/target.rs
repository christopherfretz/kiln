//! Target layout queries used by `sizeof`/`alignof`, bit-field checks and
//! the constant folder (spec/sema.md §9).
//!
//! STUB(WP-050): [`StubLayout`] is a naive C layout (fields in order,
//! bit-fields packed into their declared type's storage unit). The real
//! Itanium layout lives in `kiln_abi` (WP-050); swapping it in is a change
//! to [`layout`] alone.

use kiln_ast::decl::DeclKind;
use kiln_ast::types::{BuiltinType, FloatFormat, TargetTypes, TypeView};
use kiln_ast::{Ast, QualType, TypeId};

/// Size and alignment queries (spec/sema.md §9).
pub trait TargetLayout {
    /// `(sizeof, alignof)` of `ty` in bytes, or `None` for incomplete,
    /// function and dependent types.
    fn size_align(&self, ast: &Ast, ty: QualType) -> Option<(u64, u64)>;
}

/// The layout implementation sema uses.
pub fn layout() -> impl TargetLayout {
    StubLayout
}

/// The aarch64-unknown-linux-gnu builtin type table (the M2 default
/// target, docs/ARCHITECTURE.md).
pub fn aarch64_linux() -> TargetTypes {
    let mut size_align = [(0u16, 0u16); 34];
    let table: [(BuiltinType, u16, u16); 28] = [
        (BuiltinType::Void, 1, 1),
        (BuiltinType::Bool, 1, 1),
        (BuiltinType::Char, 1, 1),
        (BuiltinType::SChar, 1, 1),
        (BuiltinType::UChar, 1, 1),
        (BuiltinType::WChar, 4, 4),
        (BuiltinType::Char8, 1, 1),
        (BuiltinType::Char16, 2, 2),
        (BuiltinType::Char32, 4, 4),
        (BuiltinType::Short, 2, 2),
        (BuiltinType::UShort, 2, 2),
        (BuiltinType::Int, 4, 4),
        (BuiltinType::UInt, 4, 4),
        (BuiltinType::Long, 8, 8),
        (BuiltinType::ULong, 8, 8),
        (BuiltinType::LongLong, 8, 8),
        (BuiltinType::ULongLong, 8, 8),
        (BuiltinType::Int128, 16, 16),
        (BuiltinType::UInt128, 16, 16),
        (BuiltinType::Float16, 2, 2),
        (BuiltinType::BFloat16, 2, 2),
        (BuiltinType::Float, 4, 4),
        (BuiltinType::Double, 8, 8),
        (BuiltinType::LongDouble, 16, 16),
        (BuiltinType::Float128, 16, 16),
        (BuiltinType::Half, 2, 2),
        (BuiltinType::NullPtr, 8, 8),
        (BuiltinType::MetaInfo, 8, 8),
    ];
    for (b, s, a) in table {
        size_align[b as usize] = (s, a);
    }
    TargetTypes {
        size_align,
        char_signed: false,
        wchar_signed: false,
        long_double: FloatFormat::IeeeQuad,
        size_t: TypeId::ULONG,
        ptrdiff_t: TypeId::LONG,
        intptr_t: TypeId::LONG,
        pointer_bytes: 8,
    }
}

/// STUB(WP-050): naive C layout.
#[derive(Clone, Copy, Debug, Default)]
pub struct StubLayout;

impl TargetLayout for StubLayout {
    fn size_align(&self, ast: &Ast, ty: QualType) -> Option<(u64, u64)> {
        let t = &ast.types;
        let c = t.canonical(ty);
        match t.view(c.ty()) {
            TypeView::Builtin(b) => {
                let (s, a) = t.target().builtin_size_align(b);
                (s != 0).then_some((u64::from(s), u64::from(a)))
            }
            TypeView::Pointer(_) | TypeView::MemberPointer { .. } => {
                let p = u64::from(t.target().pointer_bytes);
                Some((p, p))
            }
            TypeView::LValueRef(r) | TypeView::RValueRef(r) => self.size_align(ast, r),
            TypeView::ConstantArray(a) => {
                let (s, al) = self.size_align(ast, a.elem)?;
                Some((s.checked_mul(a.size)?, al))
            }
            TypeView::Enum(d) => {
                let e = ast.decls.enumeration(ast.definition(d).unwrap_or(d))?;
                self.size_align(ast, e.int_type?)
            }
            TypeView::Record(d) => self.record(ast, ast.definition(d)?),
            _ => None,
        }
    }
}

impl StubLayout {
    fn record(&self, ast: &Ast, r: DeclId) -> Option<(u64, u64)> {
        let union = ast.decls.record(r).is_some_and(|d| d.tag == kiln_ast::decl::TagKind::Union);
        let (mut size_bits, mut align) = (0u64, 1u64);
        for m in ast.members(r) {
            let n = ast.decls.get(m);
            if n.kind() != DeclKind::Field {
                continue;
            }
            let fty = n.ty()?;
            let (fs, fa) = self.size_align(ast, fty)?;
            align = align.max(fa);
            let width = ast.decls.field(m).and_then(|f| f.bit_width).and_then(|w| {
                crate::constfold::fold_int_expr(ast, w).map(|v| v as u64)
            });
            if union {
                size_bits = size_bits.max(width.unwrap_or(fs * 8));
                continue;
            }
            match width {
                Some(0) => size_bits = size_bits.div_ceil(fa * 8) * (fa * 8),
                Some(w) => {
                    // Itanium: a bit-field does not straddle a boundary of
                    // its type's alignment.
                    let unit = fa * 8;
                    if size_bits / unit != (size_bits + w - 1) / unit {
                        size_bits = size_bits.div_ceil(unit) * unit;
                    }
                    size_bits += w;
                }
                None => {
                    let off = size_bits.div_ceil(fa * 8) * (fa * 8);
                    size_bits = off + fs * 8;
                }
            }
        }
        let bytes = size_bits.div_ceil(8);
        let size = bytes.div_ceil(align) * align;
        Some((size.max(1), align))
    }
}

use kiln_ast::DeclId;
