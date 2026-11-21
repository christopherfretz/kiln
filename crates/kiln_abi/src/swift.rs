//! The Swift calling conventions' argument lowering (clang
//! `swiftcall::computeABIInfo`, `SwiftCallingConv.cpp`; spec/abi.md §8.33,
//! WP-1745): `swiftcall` and `swiftasynccall` bypass the target ABI. A
//! class that cannot be passed in registers goes indirectly; any other
//! class, and a vector, is lowered to a sequence of legal scalar and
//! vector "entries" by byte range, then passed as those entries
//! (`CoerceAndExpand`) unless they need more than four registers (then
//! indirectly, without `byval`). A complex value is returned directly
//! and passed as its two parts; everything else is passed directly, with
//! no integer promotion.
// Mirrors clang's `SwiftAggLowering` 1:1 (short offset names).
#![allow(clippy::many_single_char_names)]

use kiln_ast::ids::{DeclId, QualType};
use kiln_ast::types::{BuiltinType, TypeView};

use crate::call::{ArgInfo, ArgKind, CallInfo, can_pass_in_registers};
use crate::irtype::{IrType, IrTypes};
use crate::layout::props;
use crate::target::Arch;

/// clang `getMaximumVoluntaryIntegerSize`: the pointer size, in bytes.
const CHUNK: u64 = 8;

/// One byte range `[begin, end)` of the lowered value: typed, or opaque
/// (`ty` = `None`).
#[derive(Clone, Debug)]
struct Entry {
    begin: u64,
    end: u64,
    ty: Option<IrType>,
}

/// clang `SwiftAggLowering`.
struct Lowering<'t, 'a> {
    t: &'t mut IrTypes<'a>,
    entries: Vec<Entry>,
}

/// Classify a `swiftcall` / `swiftasynccall` call returning `ret` with
/// argument types `args` (clang `swiftcall::computeABIInfo`).
pub(crate) fn classify(t: &mut IrTypes<'_>, ret: QualType, args: &[QualType]) -> CallInfo {
    let r = classify_type(t, ret, true);
    let a = args
        .iter()
        .map(|&q| ArgInfo { kind: classify_type(t, q, false), noundef: false })
        .collect();
    CallInfo { ret: ArgInfo { kind: r, noundef: false }, args: a, max_vector_width: 0 }
}

/// clang `classifyType` (`for_return`: of the return value).
fn classify_type(t: &mut IrTypes<'_>, q: QualType, for_return: bool) -> ArgKind {
    let ast = t.ast();
    match ast.types.canonical_view(q) {
        TypeView::Record(d) => {
            let def = ast.definition(d).unwrap_or(d);
            let align = u64::from(t.layouter().record(def).align).max(1);
            // clang `mustPassRecordIndirectly`.
            if !can_pass_in_registers(ast, def) {
                return ArgKind::Indirect { byval: false, align };
            }
            let mut l = Lowering { t, entries: Vec::new() };
            l.add_record(def, 0);
            l.finish();
            l.classify_expanded(align)
        }
        // Returned `Direct`; passed `getExpand`: the two parts as
        // separate arguments, written as a `CoerceAndExpand` of them (the
        // parts are values, not memory: codegen passes the real and
        // imaginary values as they are, spec/abi.md §8.33).
        TypeView::Complex(e) => {
            let c = t.convert(q);
            if for_return {
                return direct(c);
            }
            let e = t.convert(e);
            ArgKind::CoerceAndExpand {
                coerce: vec![e.clone(), e.clone()],
                parts: vec![e.clone(), e],
            }
        }
        TypeView::Vector { .. } => {
            let align = t.layouter().type_layout(q).align;
            let mut l = Lowering { t, entries: Vec::new() };
            l.add_qual(q, 0);
            l.finish();
            l.classify_expanded(align)
        }
        TypeView::Builtin(BuiltinType::Void) => ArgKind::Ignore,
        _ => direct(t.convert(q)),
    }
}

fn direct(coerce: IrType) -> ArgKind {
    ArgKind::Direct { coerce, offset: 0, flatten: true, align: None }
}

/// `DataLayout::getTypeStoreSize`, in bytes.
fn store_size(t: &mut IrTypes<'_>, ty: &IrType) -> u64 {
    match ty {
        IrType::Int(n) => u64::from(*n).div_ceil(8),
        IrType::X86Fp80 => 10,
        IrType::Vector(e, n) => (elem_bits(t, e) * n).div_ceil(8),
        _ => t.size_align(ty).0,
    }
}

fn elem_bits(t: &mut IrTypes<'_>, e: &IrType) -> u64 {
    match e {
        IrType::Int(b) => u64::from(*b),
        IrType::X86Fp80 => 80,
        e => t.size_align(e).0 * 8,
    }
}

/// clang `getNaturalAlignment`: the store size rounded up to a power of 2.
fn natural_align(t: &mut IrTypes<'_>, ty: &IrType) -> u64 {
    store_size(t, ty).max(1).next_power_of_two()
}

/// clang `isLegalIntegerType` (both targets have `__int128`).
fn is_legal_int(bits: u32) -> bool {
    matches!(bits, 1 | 8 | 16 | 32 | 64 | 128)
}

/// `SwiftABIInfo::isLegalVectorType` of the target: x86-64 the default
/// (more than 8 and at most 16 bytes), `AArch64SwiftABIInfo` a power-of-2
/// element count in 8 bytes, or in 16 with more than one element.
fn is_legal_vector(t: &IrTypes<'_>, size: u64, n: u64) -> bool {
    if n <= 1 {
        return false;
    }
    match t.target().arch() {
        Arch::X86_64 => size > 8 && size <= 16,
        Arch::AArch64 => n.is_power_of_two() && (size == 8 || size == 16),
    }
}

/// clang `splitLegalVectorType`: halves when that is legal, else the
/// elements.
fn split_legal_vector(t: &IrTypes<'_>, size: u64, e: &IrType, n: u64) -> (IrType, u64) {
    if n >= 4 && n.is_power_of_two() && is_legal_vector(t, size / 2, n / 2) {
        return (IrType::Vector(Box::new(e.clone()), n / 2), 2);
    }
    (e.clone(), n)
}

/// clang `legalizeVectorType`: a vector of `size` bytes as legal
/// subvectors and elements.
fn legalize_vector(t: &IrTypes<'_>, size: u64, e: &IrType, n: u64) -> Vec<IrType> {
    let vec = |k: u64| IrType::Vector(Box::new(e.clone()), k);
    if is_legal_vector(t, size, n) {
        return vec![vec(n)];
    }
    let mut out = Vec::new();
    let mut n_left = n;
    let mut log = 63 - u64::from(n.max(1).leading_zeros());
    let mut cand = 1u64 << log;
    if cand == n {
        log = log.saturating_sub(1);
        cand >>= 1;
    }
    let esize = size / n.max(1);
    let mut csize = esize * cand;
    while log > 0 {
        if !is_legal_vector(t, csize, cand) {
            log -= 1;
            cand /= 2;
            csize /= 2;
            continue;
        }
        let num = n_left >> log;
        out.extend(std::iter::repeat_n(vec(cand), usize::try_from(num).unwrap_or(0)));
        n_left -= num << log;
        if n_left == 0 {
            return out;
        }
        if n_left > 2 && !n_left.is_power_of_two() && is_legal_vector(t, esize * n_left, n_left) {
            out.push(vec(n_left));
            return out;
        }
        loop {
            log -= 1;
            cand /= 2;
            csize /= 2;
            if cand <= n_left {
                break;
            }
        }
    }
    out.extend(std::iter::repeat_n(e.clone(), usize::try_from(n_left).unwrap_or(0)));
    out
}

/// clang `isMergeableEntryType`: opaque memory, integers and pointers.
fn mergeable(ty: Option<&IrType>) -> bool {
    ty.is_none_or(|t| !t.is_fp() && !matches!(t, IrType::Vector(..)))
}

impl Lowering<'_, '_> {
    /// clang `addTypedData(QualType, CharUnits)`.
    fn add_qual(&mut self, q: QualType, begin: u64) {
        let ast = self.t.ast();
        match ast.types.canonical_view(q) {
            TypeView::Record(d) => {
                let def = ast.definition(d).unwrap_or(d);
                self.add_record(def, begin);
            }
            TypeView::ConstantArray(a) => {
                let esize = self.t.layouter().type_layout(a.elem).size;
                for i in 0..a.size {
                    self.add_qual(a.elem, begin + i * esize);
                }
            }
            TypeView::IncompleteArray(_)
            | TypeView::VariableArray { .. }
            | TypeView::DependentSizedArray { .. } => {}
            TypeView::Complex(e) => {
                let esize = self.t.layouter().type_layout(e).size;
                let ty = self.t.convert(e);
                self.add_ir_range(&ty, begin, begin + esize);
                self.add_ir_range(&ty, begin + esize, begin + 2 * esize);
            }
            TypeView::MemberPointer { .. } => {
                let size = self.t.layouter().type_layout(q).size;
                self.add_opaque(begin, begin + size);
            }
            TypeView::Atomic(v) => {
                let size = self.t.layouter().type_layout(q).size;
                let vsize = self.t.layouter().type_layout(v).size;
                self.add_qual(v, begin);
                if size > vsize {
                    self.add_opaque(begin + vsize, begin + size);
                }
            }
            _ => {
                // `ConvertType`, not for memory: a `bool` stays `i1`.
                let ty = self.t.convert(q);
                let size = store_size(self.t, &ty);
                self.add_ir_range(&ty, begin, begin + size);
            }
        }
    }

    /// clang `addTypedData(const RecordDecl *, CharUnits, layout)`.
    fn add_record(&mut self, def: DeclId, begin: u64) {
        let ast = self.t.ast();
        let layout = self.t.layouter().record(def).clone();
        let fields = props::fields(ast, def);
        if props::class_desc(ast, def).is_union {
            for f in &fields {
                match f.bit_width {
                    Some(w) => self.add_bitfield(w, begin, 0),
                    None => self.add_qual(f.ty, begin),
                }
            }
            return;
        }
        if layout.vptr.is_some() {
            // `hasOwnVFPtr`: an `i8*` at the start.
            self.add_ir_range(&IrType::Ptr, begin, begin + 8);
        }
        for &(b, off) in &*layout.bases {
            let bd = ast.definition(b).unwrap_or(b);
            self.add_record(bd, begin + off);
        }
        for (i, f) in fields.iter().enumerate() {
            let bit = layout.fields.get(i).copied().unwrap_or(0);
            match f.bit_width {
                Some(w) => self.add_bitfield(w, begin, bit),
                None => self.add_qual(f.ty, begin + bit / 8),
            }
        }
    }

    /// clang `addBitFieldData`: the bytes the bit-field touches, opaque.
    fn add_bitfield(&mut self, width: u64, record_begin: u64, bit: u64) {
        if width == 0 {
            return;
        }
        let first = bit / 8;
        let last = (bit + width - 1) / 8 + 1;
        self.add_opaque(record_begin + first, record_begin + last);
    }

    fn add_opaque(&mut self, begin: u64, end: u64) {
        self.add_entry(None, begin, end);
    }

    /// clang `addTypedData(llvm::Type *, CharUnits, CharUnits)`.
    fn add_ir_range(&mut self, ty: &IrType, mut begin: u64, end: u64) {
        if let IrType::Vector(e, n) = ty {
            let parts = legalize_vector(self.t, end - begin, e, *n);
            let Some((last, init)) = parts.split_last() else { return };
            for p in init {
                let size = store_size(self.t, p);
                self.add_legal(p, begin, begin + size);
                begin += size;
            }
            self.add_legal(last, begin, end);
            return;
        }
        if let IrType::Int(b) = ty
            && !is_legal_int(*b)
        {
            self.add_opaque(begin, end);
            return;
        }
        self.add_legal(ty, begin, end);
    }

    /// clang `addLegalTypedData`.
    fn add_legal(&mut self, ty: &IrType, mut begin: u64, end: u64) {
        if begin != 0 && !begin.is_multiple_of(natural_align(self.t, ty)) {
            if let IrType::Vector(e, n) = ty {
                let (et, num) = split_legal_vector(self.t, end - begin, e, *n);
                let esize = (end - begin) / num.max(1);
                for _ in 0..num {
                    self.add_legal(&et, begin, begin + esize);
                    begin += esize;
                }
                return;
            }
            self.add_opaque(begin, end);
            return;
        }
        self.add_entry(Some(ty.clone()), begin, end);
    }

    /// clang `getCommonType`: integers win over pointers.
    fn common_type(a: &IrType, b: &IrType) -> Option<IrType> {
        match (a, b) {
            (IrType::Int(_) | IrType::Ptr, IrType::Ptr) => Some(a.clone()),
            (IrType::Ptr, IrType::Int(_)) => Some(b.clone()),
            (IrType::Vector(ea, na), IrType::Vector(eb, nb)) => {
                let e = Self::common_type(ea, eb)?;
                (na == nb).then(|| IrType::Vector(Box::new(e), *na))
            }
            _ => None,
        }
    }

    /// clang `addEntry`.
    fn add_entry(&mut self, ty: Option<IrType>, mut begin: u64, end: u64) {
        if self.entries.last().is_none_or(|e| e.end <= begin) {
            self.entries.push(Entry { begin, end, ty });
            return;
        }
        let mut index = self.entries.len() - 1;
        while index != 0 {
            if self.entries[index - 1].end <= begin {
                break;
            }
            index -= 1;
        }
        if self.entries[index].begin >= end {
            self.entries.insert(index, Entry { begin, end, ty });
            return;
        }
        loop {
            let cur = &mut self.entries[index];
            if cur.begin == begin && cur.end == end {
                match (&cur.ty, &ty) {
                    (Some(a), Some(b)) if a == b => {}
                    (None, _) => {}
                    (_, None) => cur.ty = None,
                    (Some(a), Some(b)) => cur.ty = Self::common_type(a, b),
                }
                return;
            }
            if let Some(IrType::Vector(e, n)) = &ty {
                let esize = (end - begin) / (*n).max(1);
                for _ in 0..*n {
                    self.add_entry(Some((**e).clone()), begin, begin + esize);
                    begin += esize;
                }
                return;
            }
            if matches!(cur.ty, Some(IrType::Vector(..))) {
                self.split_vector_entry(index);
                continue;
            }
            break;
        }
        self.entries[index].ty = None;
        if begin < self.entries[index].begin {
            self.entries[index].begin = begin;
        }
        while end > self.entries[index].end {
            if index == self.entries.len() - 1 || end <= self.entries[index + 1].begin {
                self.entries[index].end = end;
                break;
            }
            self.entries[index].end = self.entries[index + 1].begin;
            index += 1;
            if self.entries[index].ty.is_none() {
                continue;
            }
            if matches!(self.entries[index].ty, Some(IrType::Vector(..)))
                && end < self.entries[index].end
            {
                self.split_vector_entry(index);
            }
            self.entries[index].ty = None;
        }
    }

    /// clang `splitVectorEntry`.
    fn split_vector_entry(&mut self, index: usize) {
        let Entry { begin, end, ty: Some(IrType::Vector(e, n)) } = self.entries[index].clone()
        else {
            return;
        };
        let (et, num) = split_legal_vector(self.t, end - begin, &e, n);
        let esize = store_size(self.t, &et);
        let mut b = begin;
        let parts: Vec<Entry> = (0..num)
            .map(|_| {
                let x = Entry { begin: b, end: b + esize, ty: Some(et.clone()) };
                b += esize;
                x
            })
            .collect();
        self.entries.splice(index..=index, parts);
    }

    /// clang `finish`.
    fn finish(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let unit = |o: u64, size: u64| o & !(size - 1);
        let mut opaque = self.entries[0].ty.is_none();
        for i in 1..self.entries.len() {
            let (a, b) = (&self.entries[i - 1], &self.entries[i]);
            if unit(a.end - 1, CHUNK) == unit(b.begin, CHUNK)
                && mergeable(a.ty.as_ref())
                && mergeable(b.ty.as_ref())
            {
                let nb = self.entries[i].begin;
                self.entries[i - 1].ty = None;
                self.entries[i - 1].end = nb;
                self.entries[i].ty = None;
                opaque = true;
            } else if self.entries[i].ty.is_none() {
                opaque = true;
            }
        }
        if !opaque {
            return;
        }
        let orig = std::mem::take(&mut self.entries);
        let mut i = 0;
        while i < orig.len() {
            if orig[i].ty.is_some() {
                self.entries.push(orig[i].clone());
                i += 1;
                continue;
            }
            let mut begin = orig[i].begin;
            let mut end = orig[i].end;
            while i + 1 < orig.len() && orig[i + 1].ty.is_none() && end == orig[i + 1].begin {
                end = orig[i + 1].end;
                i += 1;
            }
            loop {
                let chunk_end = unit(begin, CHUNK) + CHUNK;
                let local_end = end.min(chunk_end);
                let mut size = 1;
                let (ub, ue) = loop {
                    let ub = unit(begin, size);
                    if ub + size >= local_end {
                        break (ub, ub + size);
                    }
                    size *= 2;
                };
                let bits = u32::try_from(size * 8).unwrap_or(64);
                self.entries.push(Entry { begin: ub, end: ue, ty: Some(IrType::Int(bits)) });
                begin = local_end;
                if begin == end {
                    break;
                }
            }
            i += 1;
        }
    }

    /// clang `classifyExpandedType`.
    fn classify_expanded(&mut self, align: u64) -> ArgKind {
        if self.entries.is_empty() {
            return ArgKind::Ignore;
        }
        if self.occupies_more_than(4) {
            return ArgKind::Indirect { byval: false, align };
        }
        // clang `getCoerceAndExpandTypes`: `[N x i8]` padding between
        // the entries (never packed: every entry is naturally aligned).
        let mut coerce = Vec::with_capacity(self.entries.len());
        let mut parts = Vec::with_capacity(self.entries.len());
        let mut last_end = 0;
        for e in &self.entries {
            let ty = e.ty.clone().unwrap_or(IrType::Int(8));
            if e.begin != last_end {
                coerce.push(IrType::Array(Box::new(IrType::Int(8)), e.begin - last_end));
            }
            last_end = e.begin + self.t.size_align(&ty).0;
            coerce.push(ty.clone());
            parts.push(ty);
        }
        ArgKind::CoerceAndExpand { coerce, parts }
    }

    /// clang `SwiftABIInfo::occupiesMoreThan`: a pointer or each 64 bits
    /// of an integer take an integer register, a vector or a
    /// floating-point value one vector register.
    fn occupies_more_than(&self, max: u64) -> bool {
        let mut n = 0;
        for e in &self.entries {
            n += match &e.ty {
                Some(IrType::Int(b)) => u64::from(*b).div_ceil(64),
                _ => 1,
            };
        }
        n > max
    }
}
