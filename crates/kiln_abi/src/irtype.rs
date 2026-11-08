//! LLVM-independent IR type lowering: clang's `CodeGenTypes::ConvertType`
//! and `CGRecordLowering` (CGRecordLayoutBuilder.cpp, clang 19) for the
//! M2 subset, plus the data-layout size/alignment of the lowered types
//! on the supported targets. `kiln_codegen` materialises these shapes as
//! `kiln_llvm` types; `call` classifies arguments against them.
//! See spec/codegen.md §2.
// Mirrors clang's classification functions 1:1 (long functions, arms kept
// separate as in clang, short type/offset names).
#![allow(clippy::too_many_lines, clippy::many_single_char_names, clippy::match_same_arms)]

use kiln_ast::Ast;
use kiln_ast::decl::TagKind;
use kiln_ast::ids::{DeclId, QualType};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_source::fx::FxHashMap;

use crate::layout::Layouter;
use crate::layout::props::{self, FieldDesc};
use crate::target::{Arch, Target};

/// An LLVM IR type shape.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum IrType {
    /// `void`.
    Void,
    /// `iN`.
    Int(u32),
    /// `ptr`.
    Ptr,
    /// `half`.
    Half,
    /// `bfloat`.
    BFloat,
    /// `float`.
    Float,
    /// `double`.
    Double,
    /// `x86_fp80`.
    X86Fp80,
    /// `fp128`.
    Fp128,
    /// `[N x T]`.
    Array(Box<IrType>, u64),
    /// `<N x T>`.
    Vector(Box<IrType>, u64),
    /// `<vscale x N x T>` (the SVE builtins; declarations only).
    ScalableVector(Box<IrType>, u64),
    /// A parameterless target extension type `target("name")`
    /// (`__SVCount_t` is `target("aarch64.svcount")`).
    TargetExt(&'static str),
    /// The named struct of a class (`%struct.S`), by first declaration.
    Record(DeclId),
    /// The base-subobject struct of a class whose non-virtual size differs
    /// from its size (`%struct.S.base`; clang `getBaseSubobjectLLVMType`).
    RecordBase(DeclId),
    /// A literal struct `{ ... }` (packed if the flag is set).
    Struct(Vec<IrType>, bool),
}

impl IrType {
    /// `[n x i8]`, or `i8` when `n == 1` (clang `getByteArrayType`).
    pub fn bytes(n: u64) -> IrType {
        if n == 1 { IrType::Int(8) } else { IrType::Array(Box::new(IrType::Int(8)), n) }
    }
    /// `iN` with `N` rounded up to whole bytes (clang `getIntNType`).
    pub fn int_n(bits: u64) -> IrType {
        IrType::Int(u32::try_from(bits.div_ceil(8) * 8).unwrap_or(u32::MAX))
    }
    /// Is this an integer type?
    pub fn is_int(&self) -> bool {
        matches!(self, IrType::Int(_))
    }
    /// Is this a floating-point type?
    pub fn is_fp(&self) -> bool {
        matches!(
            self,
            IrType::Half
                | IrType::BFloat
                | IrType::Float
                | IrType::Double
                | IrType::X86Fp80
                | IrType::Fp128
        )
    }
}

/// Where a bit-field lives (clang `CGBitFieldInfo`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BitFieldInfo {
    /// Bit offset of the field within its storage unit.
    pub offset: u32,
    /// Width in bits (clamped to the storage size).
    pub size: u32,
    /// Size of the storage unit in bits (its alloc size).
    pub storage_size: u32,
    /// Byte offset of the storage unit in the record.
    pub storage_offset: u64,
    /// The field's type is signed (sign-extend on load).
    pub is_signed: bool,
}

/// The lowered form of one class (clang `CGRecordLayout`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent facts, as clang's CGRecordLayout")]
pub struct IrRecord {
    /// The struct body.
    pub members: Vec<IrType>,
    /// The struct is `<{ ... }>`.
    pub packed: bool,
    /// Field declaration → struct element index (absent for empty fields).
    pub field_index: FxHashMap<DeclId, u32>,
    /// Bit-field declaration → storage description.
    pub bitfields: FxHashMap<DeclId, BitFieldInfo>,
    /// The class is a union.
    pub is_union: bool,
    /// `sizeof` of the class.
    pub size: u64,
    /// `alignof` of the class.
    pub align: u64,
    /// The body of the base-subobject type, when the non-virtual size
    /// differs from the size (clang `.base`), with the same packedness.
    pub base_members: Option<Vec<IrType>>,
    /// Non-virtual base (first declaration) → struct element index
    /// (absent for empty or zero-sized bases).
    pub base_index: FxHashMap<DeclId, u32>,
    /// Virtual base (first declaration) → struct element index of the
    /// complete-object type.
    pub vbase_index: FxHashMap<DeclId, u32>,
    /// A complete object is null-initialized by all-zero bytes (clang
    /// `CGRecordLayout::isZeroInitializable`): false when a member or
    /// base (transitively) holds an Itanium data member pointer, whose
    /// null value is `-1`.
    pub zero_init: bool,
    /// The same for the class as a base subobject (virtual bases do not
    /// count; clang `isZeroInitializableAsBase`).
    pub zero_init_as_base: bool,
}

/// A conversion session (clang `CodeGenTypes` minus the LLVM context).
#[derive(Debug)]
pub struct IrTypes<'a> {
    lay: Layouter<'a>,
    records: FxHashMap<DeclId, IrRecord>,
    /// The clang release whose call lowering to follow where releases
    /// differ (`-fclang-compat`; 22 by default, as the driver).
    clang_compat: u32,
}

struct Member {
    offset: u64,
    data: Option<IrType>,
    field: Option<DeclId>,
    /// A base subobject: (class, virtual).
    base: Option<(DeclId, bool)>,
    /// Ordering key so storage precedes the fields it holds.
    seq: u32,
}

impl<'a> IrTypes<'a> {
    /// A session over `ast` for `target`.
    pub fn new(ast: &'a Ast, target: &'a Target) -> Self {
        IrTypes { lay: Layouter::new(ast, target), records: FxHashMap::default(), clang_compat: 22 }
    }

    /// Follow clang release `major`'s call lowering (`-fclang-compat`):
    /// from 20 on, an `AArch64` C++ empty class goes by value as `i64`
    /// (spec/abi.md §8.21).
    #[must_use]
    pub fn with_clang_compat(mut self, major: u32) -> Self {
        self.clang_compat = major;
        self
    }

    /// The clang release whose call lowering is followed.
    pub fn clang_compat(&self) -> u32 {
        self.clang_compat
    }

    /// The layout session.
    pub fn layouter(&mut self) -> &mut Layouter<'a> {
        &mut self.lay
    }

    /// The AST.
    pub fn ast(&self) -> &'a Ast {
        self.lay.ast()
    }

    /// The target.
    pub fn target(&self) -> &'a Target {
        self.lay.target()
    }

    /// clang `ConvertType`: the type of a value (`bool` is `i1`).
    pub fn convert(&mut self, q: QualType) -> IrType {
        let ast = self.ast();
        match ast.types.canonical_view(q) {
            TypeView::Builtin(BuiltinType::Bool) => return IrType::Int(1),
            // An enumeration whose underlying type is `bool` is a `bool`
            // value too (`ConvertType` of its integer type).
            TypeView::Enum(_) if has_boolean_representation(ast, q) => return IrType::Int(1),
            // A `_BitInt(N)` value is `iN`; in memory it fills its slot.
            TypeView::BitInt { width, .. } => return IrType::Int(width),
            // A vector value is `<N x T>`; an `ext_vector_type` of `bool`
            // is `<N x i1>` (clang `ConvertType`).
            TypeView::Vector { elem, count, .. } => {
                return IrType::Vector(Box::new(self.convert(elem)), u64::from(count));
            }
            _ => {}
        }
        self.convert_mem(q)
    }

    /// clang `convertTypeForLoadStore`: the type a value of type `q` is
    /// loaded and stored as. A `_BitInt(N)` is the integer of its ABI size
    /// even where its in-memory type is a byte array
    /// ([`Self::convert_mem`]); otherwise the in-memory type.
    pub fn convert_load_store(&mut self, q: QualType) -> IrType {
        if let TypeView::BitInt { width, .. } = self.ast().types.canonical_view(q) {
            return IrType::Int(self.target().bit_int_size_align_bits(width).0 as u32);
        }
        self.convert_mem(q)
    }

    /// clang `ConvertTypeForMem`: the type of an object in memory.
    pub fn convert_mem(&mut self, q: QualType) -> IrType {
        let ast = self.ast();
        match ast.types.canonical_view(q) {
            TypeView::Builtin(b) => self.builtin(b),
            // clang `ConvertTypeForMem`: `_BitInt(N)` in memory is the
            // integer of its ABI size (`i8` for `_BitInt(7)`, `i64` for
            // `_BitInt(33)`, `i256` for `_BitInt(200)` on x86_64), unless
            // LLVM's alloc size of `iN` differs from that size, when it is
            // the byte array of the size (`[24 x i8]` for
            // `_BitInt(129)`: `i129` allocates 32 bytes; clang
            // `typeRequiresSplitIntoByteArray`).
            TypeView::BitInt { width, .. } => {
                let bits = self.target().bit_int_size_align_bits(width).0;
                if self.size_align(&IrType::Int(width)).0 == bits / 8 {
                    IrType::Int(bits as u32)
                } else {
                    IrType::bytes(bits / 8)
                }
            }
            TypeView::Pointer(_)
            | TypeView::LValueRef(_)
            | TypeView::RValueRef(_)
            | TypeView::Function(_) => IrType::Ptr,
            TypeView::ConstantArray(a) => {
                let n = ast.types.array_size(q).unwrap_or(0);
                IrType::Array(Box::new(self.convert_mem(a.elem)), n)
            }
            TypeView::IncompleteArray(e) => IrType::Array(Box::new(self.convert_mem(e)), 0),
            // clang `ConvertType`: `_Complex T` is the pair `{ T, T }`.
            TypeView::Complex(e) => {
                let t = self.convert_mem(e);
                IrType::Struct(vec![t.clone(), t], false)
            }
            // clang `ConvertType`: a variable length array is its element.
            TypeView::VariableArray { elem, .. } => self.convert_mem(elem),
            TypeView::Record(d) => {
                let d = ast.first(d);
                self.ensure_record(d);
                IrType::Record(d)
            }
            TypeView::Enum(d) => {
                let int = ast
                    .decls
                    .enumeration(ast.definition(d).unwrap_or(d))
                    .and_then(|e| e.int_type)
                    .unwrap_or(QualType::INT);
                self.convert_mem(int)
            }
            // Itanium: a data member pointer is a `ptrdiff_t` offset, a
            // member function pointer the pair `{ ptr-or-vtable-offset, adj }`
            // (clang `ItaniumCXXABI::ConvertMemberPointerType`).
            TypeView::MemberPointer { pointee, .. } => {
                if ast.types.is_function(pointee) {
                    IrType::Struct(vec![IrType::Int(64), IrType::Int(64)], false)
                } else {
                    IrType::Int(64)
                }
            }
            // clang `ConvertTypeForMem`: a vector of `bool` is stored as
            // an integer of at least 8 bits, one bit per element.
            TypeView::Vector { elem, count, .. } => {
                if let TypeView::Builtin(BuiltinType::Bool) = ast.types.canonical_view(elem) {
                    IrType::Int(count.max(8))
                } else {
                    IrType::Vector(Box::new(self.convert_mem(elem)), u64::from(count))
                }
            }
            // clang `ConvertTypeForMem(_Atomic(T))`: `T`, padded to the
            // promoted atomic size with a byte array.
            TypeView::Atomic(v) => {
                let inner = self.convert_mem(v);
                let full = self.lay.type_layout(q).size;
                let value = self.lay.type_layout(v).size;
                if full > value {
                    IrType::Struct(
                        vec![inner, IrType::Array(Box::new(IrType::Int(8)), full - value)],
                        false,
                    )
                } else {
                    inner
                }
            }
            _ => IrType::Int(8),
        }
    }

    fn builtin(&self, b: BuiltinType) -> IrType {
        let t = self.target();
        match b {
            BuiltinType::Void => IrType::Void,
            BuiltinType::Float16 | BuiltinType::Half => IrType::Half,
            BuiltinType::BFloat16 => IrType::BFloat,
            BuiltinType::Float | BuiltinType::Float32 => IrType::Float,
            BuiltinType::Double | BuiltinType::Float64 | BuiltinType::Float32x => IrType::Double,
            BuiltinType::LongDouble | BuiltinType::Float64x => match t.arch() {
                Arch::X86_64 => IrType::X86Fp80,
                Arch::AArch64 => IrType::Fp128,
            },
            BuiltinType::Float128 | BuiltinType::StdFloat128 => IrType::Fp128,
            BuiltinType::NullPtr => IrType::Ptr,
            b if b.is_sizeless() => sve_type(b),
            // A placeholder with no size (an error or dependent type, which
            // only an undiagnosed instantiation failure lets reach codegen)
            // is `i8` like the other unrepresentable types above, never the
            // invalid `i0` (WP-582).
            _ => match t.builtin_size_align(b).0 {
                0 => IrType::Int(8),
                size => IrType::Int(u32::try_from(size * 8).unwrap_or(8)),
            },
        }
    }

    /// The lowered class `d` (any declaration).
    pub fn record(&mut self, d: DeclId) -> &IrRecord {
        let d = self.ast().first(d);
        self.ensure_record(d);
        &self.records[&d]
    }

    fn ensure_record(&mut self, d: DeclId) {
        if self.records.contains_key(&d) {
            return;
        }
        // Insert a placeholder first: a class can point to itself.
        self.records.insert(d, IrRecord::default());
        let r = self.lower(d);
        self.records.insert(d, r);
    }

    /// (alloc size, ABI alignment) of `t` in bytes on the target.
    pub fn size_align(&mut self, t: &IrType) -> (u64, u64) {
        match t {
            IrType::Void => (0, 1),
            IrType::Int(bits) => {
                let store = u64::from(*bits).div_ceil(8);
                let align = int_align(*bits);
                (store.next_multiple_of(align), align)
            }
            IrType::Ptr => (8, 8),
            IrType::Half | IrType::BFloat => (2, 2),
            IrType::Float => (4, 4),
            IrType::Double => (8, 8),
            IrType::X86Fp80 | IrType::Fp128 => (16, 16),
            IrType::Array(e, n) => {
                let (s, a) = self.size_align(e);
                (s * n, a)
            }
            IrType::Vector(e, n) => {
                let bits = match **e {
                    IrType::Int(b) => u64::from(b),
                    ref e => self.size_align(e).0 * 8,
                };
                let bytes = (bits * n).div_ceil(8).next_power_of_two();
                (bytes, bytes)
            }
            // The minimum (vscale = 1) size of a scalable vector; these
            // types are sizeless in C++ and never laid out.
            IrType::ScalableVector(..) => (16, 16),
            IrType::TargetExt(_) => (2, 2),
            IrType::Record(d) => {
                let r = self.record(*d).clone();
                struct_size_align(self, &r.members, r.packed)
            }
            IrType::RecordBase(d) => {
                let r = self.record(*d).clone();
                let m = r.base_members.unwrap_or(r.members);
                struct_size_align(self, &m, r.packed)
            }
            IrType::Struct(m, packed) => {
                let m = m.clone();
                struct_size_align(self, &m, *packed)
            }
        }
    }

    /// The byte offset of each element of a struct body.
    pub fn struct_offsets(&mut self, members: &[IrType], packed: bool) -> Vec<u64> {
        let mut off = 0u64;
        let mut v = Vec::with_capacity(members.len());
        for m in members {
            let (s, a) = self.size_align(m);
            if !packed {
                off = off.next_multiple_of(a);
            }
            v.push(off);
            off += s;
        }
        v
    }

    fn lower(&mut self, d: DeclId) -> IrRecord {
        let ast = self.ast();
        let def = ast.definition(d).unwrap_or(d);
        let layout = self.lay.record(d).clone();
        let is_union = ast.decls.record(def).is_some_and(|r| r.tag == TagKind::Union);
        let mut out = IrRecord {
            is_union,
            size: layout.size,
            align: u64::from(layout.align),
            zero_init: true,
            zero_init_as_base: true,
            ..IrRecord::default()
        };
        if ast.definition(d).is_none() {
            return out;
        }
        let fields = props::fields(ast, def);
        if is_union {
            self.lower_union(def, &fields, &layout, layout.size, &mut out);
            // clang 19 `lowerUnion(isNonVirtualBaseType)`: a union whose
            // data size is short of its size (a non-POD member's tail
            // padding) has a `.base` type of `dsize` bytes, which a
            // potentially-overlapping member of that union type uses so
            // later members can live in the tail padding.
            if layout.nv_size != layout.size {
                let mut scratch = IrRecord { zero_init: true, ..IrRecord::default() };
                self.lower_union(def, &fields, &layout, layout.data_size, &mut scratch);
                out.base_members = Some(scratch.members);
            }
            return out;
        }
        let (members, packed) = self.lower_struct(def, &fields, &layout, false, false, &mut out);
        out.members = members;
        out.packed = packed;
        self.calculate_zero_init(&fields, &layout, &mut out);
        // clang `ComputeRecordLayout`: the base-subobject type, lowered
        // with the complete type's packedness.
        if layout.nv_size != layout.size {
            let mut scratch = IrRecord::default();
            let (bm, _) = self.lower_struct(def, &fields, &layout, true, packed, &mut scratch);
            out.base_members = Some(bm);
        }
        out
    }

    /// clang `CGRecordLowering::lower` for a non-union class: the struct
    /// body and whether it is packed. `nv_base` lowers the base-subobject
    /// type; `packed` is the initial packedness.
    #[allow(clippy::too_many_arguments)]
    fn lower_struct(
        &mut self,
        def: DeclId,
        fields: &[FieldDesc],
        layout: &kiln_ast::types::RecordLayout,
        nv_base: bool,
        packed: bool,
        out: &mut IrRecord,
    ) -> (Vec<IrType>, bool) {
        let ast = self.ast();
        let size = if nv_base { layout.nv_size } else { layout.size };
        let mut members: Vec<Member> = Vec::new();
        let mut seq = 0u32;
        let discrete = ast.is_ms_struct(def);
        // accumulateFields.
        let mut i = 0;
        while i < fields.len() {
            let f = fields[i];
            if f.bit_width.is_some() && discrete {
                i = self.accumulate_discrete_bitfields(fields, i, layout, &mut members, &mut seq);
            } else if f.bit_width.is_some() {
                i = self.accumulate_bitfields(fields, i, layout, &mut members, &mut seq);
            } else if self.is_empty_field(&f) {
                i += 1;
            } else {
                // A potentially-overlapping member ([[no_unique_address]]
                // of class type) is stored as its base-subobject type, as
                // the record layout places it (clang 19 accumulateFields):
                // later members may live in its tail padding.
                let ty = match self.overlapping_class(&f) {
                    Some(c) => self.base_storage_type(c),
                    None => self.convert_mem(f.ty),
                };
                members.push(Member {
                    offset: layout.fields[i] / 8,
                    data: Some(ty),
                    field: Some(f.decl),
                    base: None,
                    seq,
                });
                seq += 1;
                i += 1;
            }
        }
        // accumulateVPtrs.
        if layout.vptr.is_some() {
            members.push(Member {
                offset: 0,
                data: Some(IrType::Ptr),
                field: None,
                base: None,
                seq,
            });
            seq += 1;
        }
        // accumulateBases.
        if let Some((p, true)) = layout.primary_base {
            let p = ast.first(p);
            let t = self.base_storage_type(p);
            members.push(Member {
                offset: 0,
                data: Some(t),
                field: None,
                base: Some((p, true)),
                seq,
            });
            seq += 1;
        }
        for &(b, off) in &layout.bases {
            let b = ast.first(b);
            if self.is_empty_class(b) || self.lay.record(b).nv_size == 0 {
                continue;
            }
            let t = self.base_storage_type(b);
            members.push(Member {
                offset: off,
                data: Some(t),
                field: None,
                base: Some((b, false)),
                seq,
            });
            seq += 1;
        }
        if members.is_empty() {
            let mut body = Vec::new();
            if size > 0 {
                body.push(IrType::bytes(size));
            }
            return (body, packed);
        }
        // accumulateVBases.
        if !nv_base {
            for &(vb, off) in &layout.vbases {
                let vb = ast.first(vb);
                if self.is_empty_class(vb) {
                    continue;
                }
                let data = if self.is_nearly_empty(vb) && !self.has_own_storage(def, vb) {
                    None
                } else {
                    Some(self.base_storage_type(vb))
                };
                members.push(Member {
                    offset: off,
                    data,
                    field: None,
                    base: Some((vb, true)),
                    seq,
                });
                seq += 1;
            }
        }
        members.sort_by_key(|m| (m.offset, m.seq));
        // Capstone.
        members.push(Member {
            offset: size,
            data: Some(IrType::Int(8)),
            field: None,
            base: None,
            seq: u32::MAX,
        });
        // determinePacked.
        let mut packed = packed;
        if !packed {
            let mut alignment = 1u64;
            let nv_size = if nv_base { 0 } else { layout.nv_size };
            let mut nv_alignment = 1u64;
            for m in &members[..members.len() - 1] {
                if let Some(t) = &m.data {
                    let (_, a) = self.size_align(t);
                    if m.offset % a != 0 {
                        packed = true;
                    }
                    if m.offset < nv_size {
                        nv_alignment = nv_alignment.max(a);
                    }
                    alignment = alignment.max(a);
                }
            }
            if members.last().is_some_and(|m| m.offset % alignment != 0) {
                packed = true;
            }
            if !nv_size.is_multiple_of(nv_alignment) {
                packed = true;
            }
            if !packed && let Some(last) = members.last_mut() {
                last.data = Some(IrType::Int(u32::try_from(alignment * 8).unwrap_or(8)));
            }
        }
        // insertPadding.
        let mut padding = Vec::new();
        let mut cur = 0u64;
        for m in &members {
            let Some(t) = &m.data else { continue };
            let (s, a) = self.size_align(t);
            let aligned = if packed { cur } else { cur.next_multiple_of(a) };
            // Internal invariant (clang's `assert(Offset >= Size)`): the
            // members with storage are disjoint and sorted -- an empty
            // `[[no_unique_address]]` member has none, a potentially
            // overlapping one is stored as its base-subobject type, a
            // bit-field run as its access units. No user input reaches a
            // violation; it would only drop the padding, never wrap.
            let gap = m.offset.checked_sub(cur);
            debug_assert!(gap.is_some(), "overlapping IR members at {} < {cur}", m.offset);
            if let Some(gap) = gap
                && m.offset != aligned
            {
                padding.push((cur, gap));
            }
            cur = m.offset + s;
        }
        members.pop();
        for (off, n) in padding {
            members.push(Member {
                offset: off,
                data: Some(IrType::bytes(n)),
                field: None,
                base: None,
                seq: 0,
            });
        }
        // stable_sort by offset only.
        members.sort_by_key(|m| m.offset);
        // fillOutputFields.
        let mut body = Vec::new();
        for m in &members {
            if let Some(t) = &m.data {
                body.push(t.clone());
            }
            let idx = u32::try_from(body.len().saturating_sub(1)).unwrap_or(0);
            if let Some(fd) = m.field {
                out.field_index.insert(fd, idx);
                if m.data.is_none() {
                    let storage = body.last().cloned().unwrap_or(IrType::Int(8));
                    let info = self.bitfield_info(fields, fd, &layout.fields, m.offset, &storage);
                    out.bitfields.insert(fd, info);
                }
            }
            match m.base {
                Some((b, false)) if m.data.is_some() => {
                    out.base_index.insert(b, idx);
                }
                Some((b, true)) if m.data.is_some() => {
                    out.vbase_index.insert(b, idx);
                }
                _ => {}
            }
        }
        (body, packed)
    }

    /// clang `getStorageType(const CXXRecordDecl *)`: the base-subobject
    /// type of class `b`.
    fn base_storage_type(&mut self, b: DeclId) -> IrType {
        let b = self.ast().first(b);
        self.ensure_record(b);
        if self.records[&b].base_members.is_some() {
            IrType::RecordBase(b)
        } else {
            IrType::Record(b)
        }
    }

    /// clang `isEmptyRecordForLayout(T)`: is `q` a class type with no
    /// storage of its own (no vptr, and only empty bases and fields)?
    pub fn is_empty_record_for_layout(&mut self, q: QualType) -> bool {
        self.is_empty_record_type(q)
    }

    /// clang `isEmptyRecordForLayout` of class `d`.
    fn is_empty_class(&mut self, d: DeclId) -> bool {
        let ast = self.ast();
        let Some(def) = ast.definition(d) else { return false };
        let class = props::class_desc(ast, def);
        if class.dynamic {
            return false;
        }
        for b in ast.decls.bases(def, &ast.lists) {
            if !self.is_empty_record_type(b.ty) {
                return false;
            }
        }
        props::fields(ast, def).iter().all(|f| self.is_empty_field(f))
    }

    /// clang `ASTContext::isNearlyEmpty`: a dynamic class whose non-virtual
    /// part is just the vptr.
    fn is_nearly_empty(&mut self, d: DeclId) -> bool {
        let ast = self.ast();
        let def = ast.definition(d).unwrap_or(d);
        let ptr = self.target().pointer_size;
        props::class_desc(ast, def).dynamic && self.lay.record(d).nv_size == ptr
    }

    /// clang `CGRecordLowering::hasOwnStorage`.
    fn has_own_storage(&mut self, decl: DeclId, query: DeclId) -> bool {
        let ast = self.ast();
        let l = self.lay.record(decl).clone();
        if let Some((p, true)) = l.primary_base
            && ast.first(p) == query
        {
            return false;
        }
        let def = ast.definition(decl).unwrap_or(decl);
        for b in props::bases(ast, def) {
            if let Some(c) = b.class
                && !self.has_own_storage(c, query)
            {
                return false;
            }
        }
        true
    }

    fn bitfield_info(
        &mut self,
        fields: &[FieldDesc],
        fd: DeclId,
        offsets: &[u64],
        storage_offset: u64,
        storage: &IrType,
    ) -> BitFieldInfo {
        let ast = self.ast();
        let i = fields.iter().position(|f| f.decl == fd).unwrap_or(0);
        let f = fields[i];
        let (ssize, _) = self.size_align(storage);
        let storage_size = u32::try_from(ssize * 8).unwrap_or(0);
        let width = u32::try_from(f.bit_width.unwrap_or(0)).unwrap_or(0);
        BitFieldInfo {
            offset: u32::try_from(offsets.get(i).copied().unwrap_or(0) - storage_offset * 8)
                .unwrap_or(0),
            size: width.min(storage_size),
            storage_size,
            storage_offset,
            is_signed: is_signed_or_enum(ast, f.ty, self.target()),
        }
    }

    /// clang `FieldDecl::isPotentiallyOverlapping`: the class of a
    /// `[[no_unique_address]]` member of class type.
    fn overlapping_class(&self, f: &FieldDesc) -> Option<DeclId> {
        if !f.nua {
            return None;
        }
        let ast = self.ast();
        match ast.types.canonical_view(f.ty) {
            TypeView::Record(d) if ast.definition(d).is_some() => Some(d),
            _ => None,
        }
    }

    /// clang `isEmptyFieldForLayout`.
    fn is_empty_field(&mut self, f: &FieldDesc) -> bool {
        if f.bit_width == Some(0) {
            return true;
        }
        if f.bit_width.is_some() && !f.named {
            return false;
        }
        self.is_empty_record_type(f.ty)
    }

    fn is_empty_record_type(&mut self, q: QualType) -> bool {
        let ast = self.ast();
        let TypeView::Record(d) = ast.types.canonical_view(q) else { return false };
        self.is_empty_class(d)
    }

    /// clang `CGRecordLowering::calculateZeroInit` over the lowered
    /// members: a non-empty, non-bit-field member of a type that is not
    /// zero-initializable, or a non-empty base whose base subobject is
    /// not, makes the class not zero-initializable (a virtual base only
    /// the complete object).
    fn calculate_zero_init(
        &mut self,
        fields: &[FieldDesc],
        layout: &kiln_ast::types::RecordLayout,
        out: &mut IrRecord,
    ) {
        let ast = self.ast();
        for f in fields {
            if f.bit_width.is_none()
                && out.field_index.contains_key(&f.decl)
                && !self.is_zero_initializable(f.ty)
            {
                out.zero_init = false;
                out.zero_init_as_base = false;
                return;
            }
        }
        for &(b, _) in &layout.bases {
            let b = ast.first(b);
            if out.base_index.contains_key(&b) && !self.record(b).zero_init_as_base {
                out.zero_init = false;
                out.zero_init_as_base = false;
                return;
            }
        }
        for &(vb, _) in &layout.vbases {
            let vb = ast.first(vb);
            if !self.is_empty_class(vb) && !self.record(vb).zero_init_as_base {
                out.zero_init = false;
            }
        }
    }

    /// clang `CodeGenTypes::isZeroInitializable(QualType)`: the null
    /// value of `q` is all-zero bytes. Under Itanium only a data member
    /// pointer (null `-1`) and classes and arrays containing one are not.
    pub fn is_zero_initializable(&mut self, q: QualType) -> bool {
        let ast = self.ast();
        let mut q = q;
        loop {
            match ast.types.canonical_view(q) {
                TypeView::ConstantArray(a) => {
                    if ast.types.array_size(q) == Some(0) {
                        return true;
                    }
                    q = a.elem;
                }
                TypeView::IncompleteArray(_) => return true,
                TypeView::VariableArray { elem, .. } => q = elem,
                TypeView::MemberPointer { pointee, .. } => return ast.types.is_function(pointee),
                TypeView::Record(d) => return self.record(d).zero_init,
                _ => return true,
            }
        }
    }

    /// clang `RecordDecl::findFirstNamedDataMember` (true when found).
    fn has_named_data_member(&self, d: DeclId) -> bool {
        let ast = self.ast();
        let Some(def) = ast.definition(d) else { return false };
        props::fields(ast, def).iter().any(|f| {
            f.named
                || matches!(ast.types.canonical_view(f.ty), TypeView::Record(r) if self.has_named_data_member(r))
        })
    }

    /// clang `getStorageType(const FieldDecl *)` for a bit-field.
    /// clang `getStorageType` of a bit-field: its type's memory type in
    /// the `discrete` (`ms_struct`) ABI, else an integer of its width.
    fn bitfield_storage_type(&mut self, f: &FieldDesc, discrete: bool) -> IrType {
        let t = self.convert_mem(f.ty);
        if discrete {
            return t;
        }
        let (s, _) = self.size_align(&t);
        IrType::int_n(f.bit_width.unwrap_or(0).min(s * 8))
    }

    /// clang 19 `CGRecordLowering::lowerUnion`: the storage member of a
    /// union padded to `layout_size` bytes (its size, or its data size
    /// for the base-subobject type).
    fn lower_union(
        &mut self,
        def: DeclId,
        fields: &[FieldDesc],
        layout: &kiln_ast::types::RecordLayout,
        layout_size: u64,
        out: &mut IrRecord,
    ) {
        let mut storage: Option<IrType> = None;
        let mut seen_named = false;
        let discrete = self.ast().is_ms_struct(def);
        for f in fields {
            if let Some(w) = f.bit_width {
                if w == 0 {
                    continue;
                }
                let mut ft = self.bitfield_storage_type(f, discrete);
                if layout_size < self.size_align(&ft).0 {
                    ft = IrType::bytes(layout_size);
                }
                let (ssize, _) = self.size_align(&ft);
                let storage_size = u32::try_from(ssize * 8).unwrap_or(0);
                let width = u32::try_from(w).unwrap_or(0);
                out.bitfields.insert(
                    f.decl,
                    BitFieldInfo {
                        offset: 0,
                        size: width.min(storage_size),
                        storage_size,
                        storage_offset: 0,
                        is_signed: is_signed_or_enum(self.ast(), f.ty, self.target()),
                    },
                );
            }
            out.field_index.insert(f.decl, 0);
            let ft = if f.bit_width.is_some() {
                self.bitfield_storage_type(f, discrete)
            } else {
                self.convert_mem(f.ty)
            };
            // clang `lowerUnion`: the first named member decides whether
            // the union is zero-initializable; when it is not, its type is
            // the storage type (no "better" one is looked for).
            if !seen_named {
                seen_named = f.named
                    || matches!(self.ast().types.canonical_view(f.ty), TypeView::Record(r) if self.has_named_data_member(r));
                if seen_named && !self.is_zero_initializable(f.ty) {
                    out.zero_init = false;
                    out.zero_init_as_base = false;
                    storage = Some(ft);
                    continue;
                }
            }
            if !out.zero_init {
                continue;
            }
            let better = match &storage {
                None => true,
                Some(s) => {
                    let (fs, fa) = self.size_align(&ft);
                    let (ss, sa) = self.size_align(s);
                    fa > sa || (fa == sa && fs > ss)
                }
            };
            if better {
                storage = Some(ft);
            }
        }
        let Some(mut st) = storage else {
            if layout_size > 0 {
                out.members.push(IrType::bytes(layout_size));
            }
            return;
        };
        if layout_size < self.size_align(&st).0 {
            st = IrType::bytes(layout_size);
        }
        let (ss, sa) = self.size_align(&st);
        out.members.push(st);
        if layout_size > ss {
            out.members.push(IrType::bytes(layout_size - ss));
        }
        if !layout.data_size.is_multiple_of(sa) {
            out.packed = true;
        }
    }

    /// clang `accumulateBitFields` for the discrete bit-field ABI
    /// (`ms_struct`, WP-809): each run gets a storage member of its first
    /// field's memory type, and a later bit-field joins it while it starts
    /// within that storage; a zero-width bit-field ends the run.
    fn accumulate_discrete_bitfields(
        &mut self,
        fields: &[FieldDesc],
        start: usize,
        layout: &kiln_ast::types::RecordLayout,
        members: &mut Vec<Member>,
        seq: &mut u32,
    ) -> usize {
        let mut field = start;
        let mut run: Option<(u64, u64)> = None;
        while field < fields.len() && fields[field].bit_width.is_some() {
            let f = fields[field];
            field += 1;
            if f.bit_width == Some(0) {
                run = None;
                continue;
            }
            let bit_offset = layout.fields[field - 1];
            let start_bit = match run {
                Some((start_bit, tail)) if bit_offset < tail => start_bit,
                _ => {
                    let ty = self.convert_mem(f.ty);
                    let (size, align) = self.size_align(&ty);
                    let tail = bit_offset + size.next_multiple_of(align.max(1)) * 8;
                    run = Some((bit_offset, tail));
                    members.push(Member {
                        offset: bit_offset / 8,
                        data: Some(ty),
                        field: None,
                        base: None,
                        seq: *seq,
                    });
                    *seq += 1;
                    bit_offset
                }
            };
            members.push(Member {
                offset: start_bit / 8,
                data: None,
                field: Some(f.decl),
                base: None,
                seq: *seq,
            });
            *seq += 1;
        }
        field
    }

    /// clang 19 `accumulateBitFields` (access units, non-discrete ABI,
    /// cheap unaligned access). Returns the index of the first field
    /// after the run.
    fn accumulate_bitfields(
        &mut self,
        fields: &[FieldDesc],
        start: usize,
        layout: &kiln_ast::types::RecordLayout,
        members: &mut Vec<Member>,
        seq: &mut u32,
    ) -> usize {
        let reg_size = 8u64;
        let end = fields.len();
        let mut field = start;
        let mut begin = end;
        let mut begin_offset = 0u64;
        let mut bit_size_since_begin = 0u64;
        let mut best_end = begin;
        let mut best_end_offset = 0u64;
        let mut best_clipped = false;
        let mut scissor = 0u64;
        loop {
            let mut at_aligned_boundary = false;
            let mut barrier = false;
            if field != end && fields[field].bit_width.is_some() {
                let bit_offset = layout.fields[field];
                if begin == end {
                    begin = field;
                    best_end = begin;
                    begin_offset = bit_offset / 8;
                    bit_size_since_begin = 0;
                } else if !bit_offset.is_multiple_of(8) {
                    // Continuing a run.
                } else {
                    if fields[field].bit_width == Some(0) {
                        barrier = true;
                    }
                    at_aligned_boundary = true;
                }
            } else {
                if begin == end {
                    break;
                }
                barrier = true;
                at_aligned_boundary = true;
            }
            let mut install_best = false;
            if at_aligned_boundary {
                let access_size = bit_size_since_begin.div_ceil(8);
                if best_end == begin {
                    best_end = field;
                    best_end_offset = begin_offset + access_size;
                    best_clipped = true;
                    if bit_size_since_begin == 0 {
                        install_best = true;
                    }
                } else if access_size > reg_size {
                    install_best = true;
                }
                if !install_best {
                    let ty = IrType::int_n(access_size * 8);
                    let mut limit = None;
                    for (p, f) in fields.iter().enumerate().take(end).skip(field) {
                        if !self.is_empty_field(f) {
                            limit = Some(layout.fields[p] / 8);
                            break;
                        }
                    }
                    let limit = if let Some(l) = limit {
                        l
                    } else {
                        if scissor == 0 {
                            scissor = layout.nv_size;
                        }
                        scissor
                    };
                    let (type_size, _) = self.size_align(&ty);
                    if begin_offset + type_size <= limit {
                        best_end_offset = begin_offset + type_size;
                        best_end = field;
                        best_clipped = false;
                    }
                    if barrier {
                        install_best = true;
                    } else {
                        bit_size_since_begin = (limit - begin_offset) * 8;
                    }
                }
            }
            if install_best {
                let access_size = best_end_offset - begin_offset;
                if access_size != 0 {
                    let ty = if best_clipped {
                        IrType::bytes(access_size)
                    } else {
                        IrType::int_n(access_size * 8)
                    };
                    members.push(Member {
                        offset: begin_offset,
                        data: Some(ty),
                        field: None,
                        base: None,
                        seq: *seq,
                    });
                    *seq += 1;
                    for f in &fields[begin..best_end] {
                        if f.bit_width != Some(0) {
                            members.push(Member {
                                offset: begin_offset,
                                data: None,
                                field: Some(f.decl),
                                base: None,
                                seq: *seq,
                            });
                            *seq += 1;
                        }
                    }
                }
                field = best_end;
                begin = end;
            } else {
                bit_size_since_begin += fields[field].bit_width.unwrap_or(0);
                field += 1;
            }
        }
        field
    }
}

fn struct_size_align(t: &mut IrTypes<'_>, members: &[IrType], packed: bool) -> (u64, u64) {
    let mut off = 0u64;
    let mut align = 1u64;
    for m in members {
        let (s, a) = t.size_align(m);
        let a = if packed { 1 } else { a };
        off = off.next_multiple_of(a);
        off += s;
        align = align.max(a);
    }
    (off.next_multiple_of(align), align)
}

/// ABI alignment of `iN` on both supported targets (`i64:64-i128:128`).
fn int_align(bits: u32) -> u64 {
    match bits {
        0..=8 => 1,
        9..=16 => 2,
        17..=32 => 4,
        33..=64 => 8,
        _ => 16,
    }
}

/// clang `hasBooleanRepresentation`: `bool`, or an enumeration whose
/// underlying type is `bool` (`enum class E : bool`). Such a value is `i1`
/// and its object an `i8` (`EmitToMemory`/`EmitFromMemory`).
pub fn has_boolean_representation(ast: &Ast, q: QualType) -> bool {
    match ast.types.canonical_view(q) {
        TypeView::Builtin(BuiltinType::Bool) => true,
        TypeView::Enum(d) => ast
            .decls
            .enumeration(ast.definition(d).unwrap_or(d))
            .and_then(|e| e.int_type)
            .is_some_and(|i| {
                matches!(ast.types.canonical_view(i), TypeView::Builtin(BuiltinType::Bool))
            }),
        _ => false,
    }
}

/// clang `isSignedIntegerOrEnumerationType`.
pub fn is_signed_or_enum(ast: &Ast, q: QualType, target: &Target) -> bool {
    match ast.types.canonical_view(q) {
        TypeView::Builtin(b) => match b {
            BuiltinType::Char => target.types.char_signed,
            BuiltinType::WChar => target.types.wchar_signed,
            _ => b.is_signed_integer(),
        },
        TypeView::BitInt { signed, .. } => signed,
        TypeView::Enum(d) => {
            let int = ast
                .decls
                .enumeration(ast.definition(d).unwrap_or(d))
                .and_then(|e| e.int_type)
                .unwrap_or(QualType::INT);
            is_signed_or_enum(ast, int, target)
        }
        _ => false,
    }
}

/// The record kind prefix of a class's IR name (`struct`, `class`, `union`).
pub fn record_kind_prefix(ast: &Ast, d: DeclId) -> &'static str {
    let def = ast.definition(d).unwrap_or(d);
    match ast.decls.record(def).map(|r| r.tag) {
        Some(TagKind::Union) => "union",
        Some(TagKind::Class) => "class",
        _ => "struct",
    }
}

/// The LLVM type of the SVE builtin `b` (clang
/// `CodeGenTypes::ConvertType` with `AArch64SVEACLETypes.def`).
fn sve_type(b: BuiltinType) -> IrType {
    use BuiltinType as B;
    let (e, n) = match b {
        B::SvInt8 | B::SvUint8 => (IrType::Int(8), 16),
        B::SvInt16 | B::SvUint16 => (IrType::Int(16), 8),
        B::SvInt32 | B::SvUint32 => (IrType::Int(32), 4),
        B::SvInt64 | B::SvUint64 => (IrType::Int(64), 2),
        B::SvFloat16 => (IrType::Half, 8),
        B::SvBfloat16 => (IrType::BFloat, 8),
        B::SvFloat32 => (IrType::Float, 4),
        B::SvFloat64 => (IrType::Double, 2),
        B::SvBool => (IrType::Int(1), 16),
        _ => return IrType::TargetExt("aarch64.svcount"),
    };
    IrType::ScalableVector(Box::new(e), n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sve_builtins_lower_to_scalable_vectors() {
        use BuiltinType as B;
        let sv = |e: IrType, n| IrType::ScalableVector(Box::new(e), n);
        assert_eq!(sve_type(B::SvInt8), sv(IrType::Int(8), 16));
        assert_eq!(sve_type(B::SvUint64), sv(IrType::Int(64), 2));
        assert_eq!(sve_type(B::SvFloat32), sv(IrType::Float, 4));
        assert_eq!(sve_type(B::SvBfloat16), sv(IrType::BFloat, 8));
        assert_eq!(sve_type(B::SvBool), sv(IrType::Int(1), 16));
        assert_eq!(sve_type(B::SvCount), IrType::TargetExt("aarch64.svcount"));
        assert_eq!(B::ALL.iter().filter(|b| b.is_sizeless()).count(), 14);
    }
}
