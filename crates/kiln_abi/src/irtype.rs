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
    /// The named struct of a class (`%struct.S`), by first declaration.
    Record(DeclId),
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
}

/// A conversion session (clang `CodeGenTypes` minus the LLVM context).
#[derive(Debug)]
pub struct IrTypes<'a> {
    lay: Layouter<'a>,
    records: FxHashMap<DeclId, IrRecord>,
}

struct Member {
    offset: u64,
    data: Option<IrType>,
    field: Option<DeclId>,
    /// Ordering key so storage precedes the fields it holds.
    seq: u32,
}

impl<'a> IrTypes<'a> {
    /// A session over `ast` for `target`.
    pub fn new(ast: &'a Ast, target: &'a Target) -> Self {
        IrTypes { lay: Layouter::new(ast, target), records: FxHashMap::default() }
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
        if let TypeView::Builtin(BuiltinType::Bool) = ast.types.canonical_view(q) {
            return IrType::Int(1);
        }
        self.convert_mem(q)
    }

    /// clang `ConvertTypeForMem`: the type of an object in memory.
    pub fn convert_mem(&mut self, q: QualType) -> IrType {
        let ast = self.ast();
        match ast.types.canonical_view(q) {
            TypeView::Builtin(b) => self.builtin(b),
            TypeView::BitInt { width, .. } => IrType::Int(width),
            TypeView::Pointer(_)
            | TypeView::LValueRef(_)
            | TypeView::RValueRef(_)
            | TypeView::Function(_) => IrType::Ptr,
            TypeView::ConstantArray(a) => {
                let n = ast.types.array_size(q).unwrap_or(0);
                IrType::Array(Box::new(self.convert_mem(a.elem)), n)
            }
            TypeView::IncompleteArray(e) => IrType::Array(Box::new(self.convert_mem(e)), 0),
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
            TypeView::MemberPointer { .. } => IrType::Int(64),
            _ => IrType::Int(8),
        }
    }

    fn builtin(&self, b: BuiltinType) -> IrType {
        let t = self.target();
        match b {
            BuiltinType::Void => IrType::Void,
            BuiltinType::Float16 | BuiltinType::Half => IrType::Half,
            BuiltinType::BFloat16 => IrType::BFloat,
            BuiltinType::Float => IrType::Float,
            BuiltinType::Double => IrType::Double,
            BuiltinType::LongDouble => match t.arch() {
                Arch::X86_64 => IrType::X86Fp80,
                Arch::AArch64 => IrType::Fp128,
            },
            BuiltinType::Float128 => IrType::Fp128,
            BuiltinType::NullPtr => IrType::Ptr,
            _ => {
                let (size, _) = t.builtin_size_align(b);
                IrType::Int(u32::try_from(size * 8).unwrap_or(8))
            }
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
                let (s, _) = self.size_align(e);
                let bytes = (s * n).next_power_of_two();
                (bytes, bytes)
            }
            IrType::Record(d) => {
                let r = self.record(*d).clone();
                struct_size_align(self, &r.members, r.packed)
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
        let fields = if ast.definition(d).is_some() { props::fields(ast, def) } else { Vec::new() };
        let mut out = IrRecord {
            is_union,
            size: layout.size,
            align: u64::from(layout.align),
            ..IrRecord::default()
        };
        if ast.definition(d).is_none() {
            return out;
        }
        if is_union {
            self.lower_union(&fields, &layout, &mut out);
            return out;
        }
        let mut members: Vec<Member> = Vec::new();
        let mut seq = 0u32;
        let mut i = 0;
        while i < fields.len() {
            let f = fields[i];
            if f.bit_width.is_some() {
                i = self.accumulate_bitfields(&fields, i, &layout, &mut members, &mut seq);
            } else if self.is_empty_field(&f) {
                i += 1;
            } else {
                let ty = self.convert_mem(f.ty);
                members.push(Member {
                    offset: layout.fields[i] / 8,
                    data: Some(ty),
                    field: Some(f.decl),
                    seq,
                });
                seq += 1;
                i += 1;
            }
        }
        if members.is_empty() {
            if layout.size > 0 {
                out.members.push(IrType::bytes(layout.size));
            }
            return out;
        }
        members.sort_by_key(|m| (m.offset, m.seq));
        // Capstone.
        members.push(Member {
            offset: layout.size,
            data: Some(IrType::Int(8)),
            field: None,
            seq: u32::MAX,
        });
        // determinePacked.
        let mut packed = false;
        let mut alignment = 1u64;
        let nv_size = layout.nv_size;
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
        // insertPadding.
        let mut padding = Vec::new();
        let mut size = 0u64;
        for m in &members {
            let Some(t) = &m.data else { continue };
            let (s, a) = self.size_align(t);
            let aligned = if packed { size } else { size.next_multiple_of(a) };
            if m.offset != aligned {
                padding.push((size, m.offset - size));
            }
            size = m.offset + s;
        }
        members.pop();
        for (off, n) in padding {
            members.push(Member { offset: off, data: Some(IrType::bytes(n)), field: None, seq: 0 });
        }
        // stable_sort by offset only.
        members.sort_by_key(|m| m.offset);
        // fillOutputFields.
        for m in &members {
            if let Some(t) = &m.data {
                out.members.push(t.clone());
            }
            if let Some(fd) = m.field {
                let idx = u32::try_from(out.members.len().saturating_sub(1)).unwrap_or(0);
                out.field_index.insert(fd, idx);
                if m.data.is_none() {
                    let storage = out.members.last().cloned().unwrap_or(IrType::Int(8));
                    let info = self.bitfield_info(&fields, fd, &layout.fields, m.offset, &storage);
                    out.bitfields.insert(fd, info);
                }
            }
        }
        out.packed = packed;
        out
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

    /// clang `getStorageType(const FieldDecl *)` for a bit-field.
    fn bitfield_storage_type(&mut self, f: &FieldDesc) -> IrType {
        let t = self.convert_mem(f.ty);
        let (s, _) = self.size_align(&t);
        IrType::int_n(f.bit_width.unwrap_or(0).min(s * 8))
    }

    fn lower_union(
        &mut self,
        fields: &[FieldDesc],
        layout: &kiln_ast::types::RecordLayout,
        out: &mut IrRecord,
    ) {
        let layout_size = layout.size;
        let mut storage: Option<IrType> = None;
        for f in fields {
            if let Some(w) = f.bit_width {
                if w == 0 {
                    continue;
                }
                let mut ft = self.bitfield_storage_type(f);
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
                self.bitfield_storage_type(f)
            } else {
                self.convert_mem(f.ty)
            };
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
                        seq: *seq,
                    });
                    *seq += 1;
                    for f in &fields[begin..best_end] {
                        if f.bit_width != Some(0) {
                            members.push(Member {
                                offset: begin_offset,
                                data: None,
                                field: Some(f.decl),
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
