//! The Itanium C++ ABI §2.4 class layout algorithm for classes without
//! bases (clang `ItaniumRecordLayoutBuilder`: `InitializeLayout`,
//! `LayoutField`, `LayoutBitField`, `LayoutWideBitField`,
//! `FinishLayout`). All quantities are in bits unless named `_bytes`.

use kiln_ast::types::{BuiltinType, RecordLayout, TypeView};
use kiln_ast::{Ast, DeclId, QualType};

use super::empty::EmptySubobjectMap;
use super::props::{self, ClassDesc, FieldDesc};
use super::{FieldTarget, Info, Layouter, RecordFacts, align_to};

/// What the algorithm needs to know about one field, gathered before
/// placement (so placement only reads the session).
#[derive(Clone, Copy, Debug)]
struct FieldIn {
    desc: FieldDesc,
    /// `getTypeInfo(field type)`.
    info: Info,
    /// `FieldDecl::getMaxAlignment()` (bits, 0 = none).
    max_align: u64,
    /// Incomplete array (flexible array member): occupies no space.
    incomplete_array: bool,
    /// The class the field's type designates directly
    /// (`getAsCXXRecordDecl`), first declaration.
    class: Option<DeclId>,
    target: FieldTarget,
}

/// Running state of the builder (clang member names in comments).
#[derive(Debug)]
struct Builder {
    is_union: bool,
    packed: bool,
    /// `Size`.
    size: u64,
    /// `DataSize`.
    data_size: u64,
    /// `UnfilledBitsInLastUnit`.
    unfilled: u64,
    /// `PaddedFieldSize`.
    padded_field_size: u64,
    /// `Alignment`, in bits (a multiple of 8).
    align: u64,
    offsets: Vec<u64>,
}

impl Builder {
    /// clang `UpdateAlignment` (alignments below a byte are ignored).
    fn update_align(&mut self, bits: u64) {
        let bytes_as_bits = bits / 8 * 8;
        self.align = self.align.max(bytes_as_bits);
    }
}

/// The field target of a field of type `q` in the empty-subobject map
/// (clang `CanPlaceFieldSubobjectAtOffset`: a class, or a constant array
/// whose base element is a class).
fn field_target(ast: &Ast, q: QualType) -> FieldTarget {
    let mut v = ast.types.canonical_view(q);
    let mut count: u64 = 1;
    let mut is_array = false;
    for _ in 0..1024 {
        match v {
            TypeView::ConstantArray(a) => {
                is_array = true;
                count = count.saturating_mul(a.size);
                v = ast.types.canonical_view(a.elem);
            }
            TypeView::QualifiedArray { base, .. } => {
                v = ast.types.canonical_view(QualType::unqual(base));
            }
            TypeView::Record(d) if is_array => return FieldTarget::Array(ast.first(d), count),
            TypeView::Record(d) => return FieldTarget::Record(ast.first(d)),
            _ => return FieldTarget::None,
        }
    }
    FieldTarget::None
}

fn is_incomplete_array(ast: &Ast, q: QualType) -> bool {
    matches!(ast.types.canonical_view(q), TypeView::IncompleteArray(_))
}

/// Lays out the class definition `def` (its contained classes are
/// already in the session).
pub(super) fn layout_record(lay: &mut Layouter<'_>, def: DeclId) -> RecordFacts {
    let ast = lay.ast();
    let target = lay.target();
    let class: ClassDesc = props::class_desc(ast, def);
    let descs = props::fields(ast, def);

    // Gather per-field type facts (may lay out further classes reached
    // only through sugar).
    let mut ins = Vec::with_capacity(descs.len());
    for d in &descs {
        let info = lay.type_info(d.ty);
        let max_align = props::max_align_attr(ast, d.decl, target, &mut |q| lay.type_info(q).align);
        ins.push(FieldIn {
            desc: *d,
            info,
            max_align,
            incomplete_array: is_incomplete_array(ast, d.ty),
            class: props::as_record(ast, d.ty),
            target: if d.bit_width.is_some() { FieldTarget::None } else { field_target(ast, d.ty) },
        });
    }
    let record_align_attr =
        props::max_align_attr(ast, def, target, &mut |q| lay.type_info(q).align);

    let lay: &Layouter<'_> = lay;
    let facts_of = |r: DeclId| lay.facts(r);
    let is_pod = props::is_pod(ast, def, class, &descs, &|r| facts_of(r).is_some_and(|f| f.is_pod));
    let is_empty =
        props::is_empty(ast, def, class, &descs, &|r| facts_of(r).is_some_and(|f| f.is_empty));

    // clang `ComputeEmptySubobjectSizes` (fields only in M2).
    let largest_empty = descs
        .iter()
        .filter_map(|d| props::base_element_record(ast, d.ty))
        .filter_map(facts_of)
        .map(|f| if f.is_empty { f.layout.size } else { f.largest_empty })
        .max()
        .unwrap_or(0);

    // InitializeLayout.
    let mut b = Builder {
        is_union: class.is_union,
        packed: class.packed,
        size: 0,
        data_size: 0,
        unfilled: 0,
        padded_field_size: 0,
        align: 8,
        offsets: Vec::with_capacity(ins.len()),
    };
    b.update_align(record_align_attr);

    let mut empty_map = EmptySubobjectMap::new(largest_empty);
    for f in &ins {
        match f.desc.bit_width {
            Some(w) => layout_bit_field(&mut b, lay, f, w),
            None => layout_field(&mut b, lay, &mut empty_map, f),
        }
    }

    // Layout(): the non-virtual size is taken before FinishLayout.
    let nv_size_bits = align_to(b.size, 8);
    let nv_align_bits = b.align;

    // FinishLayout.
    if b.size == 0 && is_empty {
        b.size = 8;
    }
    b.size = b.size.max(b.padded_field_size);
    b.size = align_to(b.size, b.align);

    let size = b.size / 8;
    let (data_size, nv_size) =
        if is_pod { (size, size) } else { (b.data_size / 8, nv_size_bits / 8) };
    let align = u32::try_from(b.align / 8).unwrap_or(u32::MAX);
    let nv_align = u32::try_from(nv_align_bits / 8).unwrap_or(u32::MAX);
    RecordFacts {
        layout: RecordLayout {
            size,
            align,
            preferred_align: align,
            data_size,
            nv_size,
            nv_align,
            fields: b.offsets.into_boxed_slice(),
            ..RecordLayout::default()
        },
        is_pod,
        is_empty,
        packed: class.packed,
        largest_empty,
        field_targets: ins.iter().map(|f| f.target).collect(),
    }
}

/// clang `LayoutField` for a non-bit-field member.
fn layout_field(b: &mut Builder, lay: &Layouter<'_>, map: &mut EmptySubobjectMap, f: &FieldIn) {
    let class_facts = f.class.and_then(|c| lay.facts(c));
    let potentially_overlapping = f.desc.nua && f.class.is_some();
    let overlapping_empty = potentially_overlapping && class_facts.is_some_and(|c| c.is_empty);

    let mut off = if b.is_union || overlapping_empty { 0 } else { b.data_size };
    b.unfilled = 0;

    let field_size = if f.incomplete_array { 0 } else { f.info.width };
    let mut effective = field_size;
    if potentially_overlapping && !f.incomplete_array {
        effective = class_facts.map_or(0, |c| c.layout.nv_size.max(c.layout.data_size) * 8);
    }

    let field_packed = (b.packed
        && (f.class.is_none() || class_facts.is_some_and(|c| c.is_pod || c.packed)))
        || f.desc.packed;
    let attr = f.max_align / 8 * 8;
    let field_align = if field_packed { attr.max(8) } else { f.info.align.max(attr) };

    off = align_to(off, field_align);
    if !b.is_union {
        // Terminates: past the last empty subobject every offset fits.
        while !map.can_place_field(lay, f.target, off / 8, f.desc.nua) {
            off = if off == 0 && b.data_size != 0 {
                align_to(b.data_size, field_align)
            } else {
                off.saturating_add(field_align)
            };
        }
    }
    b.offsets.push(off);

    if overlapping_empty {
        b.size = b.size.max(off.saturating_add(field_size));
    } else {
        b.data_size =
            if b.is_union { b.data_size.max(effective) } else { off.saturating_add(effective) };
        b.padded_field_size = b.padded_field_size.max(off.saturating_add(field_size));
        b.size = b.size.max(b.data_size);
    }
    b.update_align(field_align);
}

/// clang `LayoutBitField` (System V bit-field allocation, Itanium ABI
/// §2.4 II.3.a).
fn layout_bit_field(b: &mut Builder, lay: &Layouter<'_>, f: &FieldIn, width: u64) {
    let storage_unit = f.info.width;
    if width > storage_unit {
        layout_wide_bit_field(b, lay, width);
        return;
    }
    let field_packed = b.packed || f.desc.packed;
    let mut off = if b.is_union { 0 } else { b.data_size - b.unfilled };
    let mut field_align = f.info.align.max(1);
    if field_packed && width != 0 {
        field_align = 1;
    }
    let explicit = f.max_align;
    if explicit != 0 {
        field_align = field_align.max(explicit);
    }
    if width == 0 || (off % field_align) + width > storage_unit {
        off = align_to(off, field_align);
    } else if explicit != 0 {
        off = align_to(off, explicit);
    }
    b.offsets.push(off);

    if !lay.target().unnamed_bitfield_affects_align && !f.desc.named {
        field_align = 1;
    }
    if b.is_union {
        b.data_size = b.data_size.max(align_to(width, 8));
    } else {
        let end = off.saturating_add(width);
        b.data_size = align_to(end, 8);
        b.unfilled = b.data_size - end;
    }
    b.size = b.size.max(b.data_size);
    b.update_align(field_align);
}

/// clang `LayoutWideBitField`: a bit-field wider than its type is
/// allocated with the alignment of the largest unsigned integer type
/// T' with `sizeof(T') * 8 <= width` (Itanium ABI §2.4 II.3.a.i).
fn layout_wide_bit_field(b: &mut Builder, lay: &Layouter<'_>, width: u64) {
    let target = lay.target();
    let mut t_align = 8;
    for t in [
        BuiltinType::UChar,
        BuiltinType::UShort,
        BuiltinType::UInt,
        BuiltinType::ULong,
        BuiltinType::ULongLong,
    ] {
        let (size, align) = target.builtin_size_align(t);
        if size * 8 > width {
            break;
        }
        t_align = align * 8;
    }
    b.unfilled = 0;
    let off;
    if b.is_union {
        b.data_size = b.data_size.max(align_to(width, 8));
        off = 0;
    } else {
        off = align_to(b.data_size, t_align);
        let end = off.saturating_add(width);
        b.data_size = align_to(end, 8);
        b.unfilled = b.data_size - end;
    }
    b.offsets.push(off);
    b.size = b.size.max(b.data_size);
    b.update_align(t_align);
}
