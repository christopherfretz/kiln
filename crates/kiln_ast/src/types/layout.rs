//! Layout hooks (spec/types.md §11). Layout is target knowledge and
//! belongs to `kiln_abi`; the type layer only stores the target's builtin
//! sizes and a write-once cache of computed record layouts.

use super::kinds::BuiltinType;
use crate::ids::{DeclId, TypeId};
use crate::mem::{HeapSize, MemReport, MemRow};
use kiln_source::fx::FxHashMap;

/// A floating-point representation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum FloatFormat {
    /// IEEE 754 binary16.
    IeeeHalf,
    /// bfloat16.
    BFloat,
    /// IEEE 754 binary32.
    IeeeSingle,
    /// IEEE 754 binary64.
    #[default]
    IeeeDouble,
    /// x87 80-bit extended precision.
    X87Extended,
    /// IEEE 754 binary128.
    IeeeQuad,
    /// PowerPC double-double.
    PpcDoubleDouble,
}

/// Target facts about the builtin types, filled by
/// `kiln_abi::target(triple)`. The `Default` value describes an unknown
/// target (all sizes 0); it is only for tests and for ASTs that are never
/// laid out.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TargetTypes {
    /// `(size in bytes, alignment in bytes)` per builtin id 1..=90 (index
    /// 0 unused).
    pub size_align: [(u16, u16); BuiltinType::COUNT + 1],
    /// `char` is signed.
    pub char_signed: bool,
    /// `wchar_t` is signed.
    pub wchar_signed: bool,
    /// The format of `long double`.
    pub long_double: FloatFormat,
    /// The builtin `std::size_t` is.
    pub size_t: TypeId,
    /// The builtin `std::ptrdiff_t` is.
    pub ptrdiff_t: TypeId,
    /// The builtin `std::intptr_t` is.
    pub intptr_t: TypeId,
    /// Size of a data pointer in bytes.
    pub pointer_bytes: u8,
    /// The widest atomic operation, in bytes, the target inlines (clang
    /// `MaxAtomicInlineWidth` / 8; 0 for an unknown target).
    pub max_atomic_inline_bytes: u8,
}

impl Default for TargetTypes {
    fn default() -> Self {
        TargetTypes {
            size_align: [(0, 0); BuiltinType::COUNT + 1],
            char_signed: true,
            wchar_signed: true,
            long_double: FloatFormat::IeeeDouble,
            size_t: TypeId::ULONG,
            ptrdiff_t: TypeId::LONG,
            intptr_t: TypeId::LONG,
            pointer_bytes: 0,
            max_atomic_inline_bytes: 0,
        }
    }
}

impl TargetTypes {
    /// `(size, align)` in bytes of builtin `b` (0 for an unknown target
    /// and for the placeholder builtins).
    pub fn builtin_size_align(&self, b: BuiltinType) -> (u16, u16) {
        self.size_align[b as usize]
    }
}

/// The Itanium layout of one class; all offsets are from the start of
/// the object.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RecordLayout {
    /// `sizeof`.
    pub size: u64,
    /// `alignof`.
    pub align: u32,
    /// Preferred alignment (may exceed `align`, e.g. i386 `double`).
    pub preferred_align: u32,
    /// dsize: size without tail padding (reusable by derived classes).
    pub data_size: u64,
    /// Size as a base subobject (no virtual bases).
    pub nv_size: u64,
    /// Alignment as a base subobject.
    pub nv_align: u32,
    /// Unadjusted (natural) alignment, clang 21+ `getUnadjustedAlignment`:
    /// the largest alignment of a field (after `packed` / `#pragma pack`
    /// and the field's own `alignas`/`aligned`) or of a base not placed
    /// as an empty base at offset 0 (its `nv_align`, capped by `#pragma
    /// pack`); at least 1. The class's own alignment attribute does not
    /// count. AAPCS64 argument classification reads it (spec/abi.md §5,
    /// WP-583).
    pub unadjusted_align: u32,
    /// The same over the fields only (clang 19/20, before bases counted).
    pub unadjusted_field_align: u32,
    /// Bit offset of each field, in declaration order.
    pub fields: Box<[u64]>,
    /// Direct non-virtual bases with their byte offsets.
    pub bases: Box<[(DeclId, u64)]>,
    /// All virtual bases with their complete-object byte offsets.
    pub vbases: Box<[(DeclId, u64)]>,
    /// Byte offset of the vtable pointer, if the class has its own.
    pub vptr: Option<u64>,
    /// The primary base and whether it is virtual.
    pub primary_base: Option<(DeclId, bool)>,
}

impl RecordLayout {
    fn heap_bytes(&self) -> u64 {
        (self.fields.len() * 8 + (self.bases.len() + self.vbases.len()) * 16) as u64
    }
}

/// Write-once cache of record layouts keyed by the class's first
/// declaration (`Ast::layouts`).
#[derive(Debug, Default)]
pub struct LayoutCache {
    map: FxHashMap<DeclId, Box<RecordLayout>>,
}

impl LayoutCache {
    /// Stores the layout of `record`. Inserting twice is a logic error
    /// (`debug_assert!`); in release builds the first layout is kept.
    pub fn insert(&mut self, record: DeclId, layout: RecordLayout) {
        let prev = self.map.insert(record, Box::new(layout));
        debug_assert!(prev.is_none(), "layout of {record:?} computed twice");
        if let Some(p) = prev {
            self.map.insert(record, p);
        }
    }

    /// The cached layout of `record`.
    pub fn get(&self, record: DeclId) -> Option<&RecordLayout> {
        self.map.get(&record).map(|b| &**b)
    }

    /// Number of cached layouts.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True if no layout is cached.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl HeapSize for LayoutCache {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_map("map", &self.map));
        let boxed: u64 =
            self.map.values().map(|l| size_of::<RecordLayout>() as u64 + l.heap_bytes()).sum();
        r.push(MemRow::new("records", self.map.len() as u64, boxed, boxed));
        r
    }
}
