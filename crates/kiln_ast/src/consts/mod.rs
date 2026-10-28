//! Interned constant values: the [`ConstStore`] (spec/ast.md §9).
//!
//! Every result of constant evaluation that the AST retains is a
//! [`ConstValueId`] into the store. Values are **interned**: structurally
//! equal values (same type, same bits, same relocations) get the same id,
//! so template-argument equivalence ([temp.type]) and `ConstantExpr`
//! comparisons are id equality.
//!
//! # Value model (matches the Oven's memory, spec/oven.md)
//! - **Scalars** are inline in a 16-byte node: integers up to 64 bits,
//!   `bool`, floats up to 64 bits (exact IEEE bits), null pointers,
//!   pointers `(base, offset)`, pointers to member, reflections. Wider
//!   integers (`__int128`, `_BitInt(N)`) and floats (x87, binary128) put
//!   their bits in a word pool.
//! - **Aggregates** are a [`ByteImage`]: the object's bytes in target
//!   layout, a relocation table (byte offset → interned pointer-like
//!   value), a padding list (the "shadow summary": which bytes are not
//!   part of any value) and the active members of unions. The Oven freezes
//!   an allocation by copying its bytes and turning its shadow into these
//!   tables, and thaws one by `memcpy` plus one pass over [`ConstStore::shadow`].
//!
//! # Freeze / thaw (the Oven's use)
//! Freeze bottom-up: intern each pointer-like subobject with
//! [`ConstStore::push_scalar`] (a pointer into another frozen object uses
//! [`PtrBase::Const`]), then the aggregate with
//! [`ConstStore::push_aggregate`]. Thaw: [`ConstStore::get`] (or
//! [`ConstStore::bytes`] / [`ConstStore::relocs`] / [`ConstStore::shadow`]
//! / [`ConstStore::unions`]) — copy the bytes into a fresh allocation,
//! mark `Value` runs initialized, `Padding` runs uninitialized, and write
//! the provenance of each `Pointer` run; set each union's active member.
//! No layout is re-derived. The store contains no Oven logic.
//!
//! # Storage
//! A value is a 16-byte node (`ty`, a tag word, two payload words) plus,
//! for wide scalars, member-pointer paths and aggregates, slices of
//! append-only pools. The intern table stores only ids (hashbrown
//! `HashTable`, ~5 bytes per entry); keys are re-read from the pools.
//! Aggregates keep their 64-bit hash so growing the table never rehashes
//! images. All pools are addressed by `u32`: an image, and the sum of all
//! images, is limited to 4 GiB ([`ConstError::Capacity`]).

mod float;
mod print;
mod value;

pub use float::format_float;
pub use print::{IntStyle, Member, NoEnv, PrintEnv, PrintOptions, PrintStyle, Shape};
pub use value::{
    ByteImage, ByteRange, ConstError, ConstValue, FloatKind, IntBits, MemberKind, NullKind,
    PtrBase, Reloc, Scalar, ShadowKind, ShadowRun, UnionMember, ValueKind,
};

use std::fmt;
use std::hash::Hasher;
use std::mem::size_of;

use hashbrown::HashTable;
use kiln_source::fx::FxHasher;

use crate::ids::{ConstValueId, DeclId, QualType, ReflectionId};
use crate::mem::{HeapSize, MemReport, MemRow};

/// A stored value's fixed-size header.
#[derive(Clone, Copy, Debug)]
struct Node {
    ty: QualType,
    /// Bits 0..4: kind; 4..8: flags; 8..32: `extra` (24 bits).
    tag: u32,
    a: u32,
    b: u32,
}

const _: () = assert!(size_of::<Node>() == 16);

mod kind {
    pub const INT: u32 = 1;
    pub const BOOL: u32 = 2;
    pub const FLOAT: u32 = 3;
    pub const NULL: u32 = 4;
    pub const PTR: u32 = 5;
    pub const MEMPTR: u32 = 6;
    pub const REFL: u32 = 7;
    pub const AGG: u32 = 8;
}
/// `INT`: the bits are signed.
const F_SIGNED: u32 = 1;
/// `PTR`: `b` is a word-pool index holding the 64-bit offset.
const F_WIDE: u32 = 1;
/// `PTR`: `b` is a word-pool index holding the 64-bit offset followed by
/// the base's second id ([`PtrBase::aux`]: a string literal's origin).
const F_AUX: u32 = 2;
/// `MEMPTR`: member function.
const F_FUNCTION: u32 = 1;
/// `MEMPTR`: `derived` path.
const F_DERIVED: u32 = 2;

const fn tag(kind: u32, flags: u32, extra: u32) -> u32 {
    kind | (flags << 4) | (extra << 8)
}

impl Node {
    fn kind(self) -> u32 {
        self.tag & 0xf
    }
    fn flags(self) -> u32 {
        (self.tag >> 4) & 0xf
    }
    fn extra(self) -> u32 {
        self.tag >> 8
    }
}

/// Per-aggregate pool starts. `aggs[i + 1]` (always present: the vector
/// ends with a sentinel) gives the ends.
#[derive(Clone, Copy, Debug, Default)]
struct AggRec {
    bytes: u32,
    relocs: u32,
    padding: u32,
    unions: u32,
    hash: u64,
}

/// The pools (everything but the intern table, so the table's rehash
/// closure can borrow them while the table is borrowed mutably).
#[derive(Debug)]
struct Pools {
    pointer_bytes: u8,
    nodes: Vec<Node>,
    words: Vec<u64>,
    paths: Vec<DeclId>,
    aggs: Vec<AggRec>,
    bytes: Vec<u8>,
    relocs: Vec<Reloc>,
    padding: Vec<ByteRange>,
    unions: Vec<UnionMember>,
}

/// The interned, append-only store of constant values (spec/ast.md §9.1).
///
/// `Default` assumes 8-byte pointers; `Ast::new` uses
/// [`ConstStore::with_pointer_bytes`] with `TargetTypes::pointer_bytes`.
pub struct ConstStore {
    pools: Pools,
    table: HashTable<ConstValueId>,
}

impl Default for ConstStore {
    fn default() -> Self {
        Self::with_pointer_bytes(8)
    }
}

impl fmt::Debug for ConstStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConstStore")
            .field("values", &self.pools.nodes.len())
            .field("aggregates", &(self.pools.aggs.len() - 1))
            .field("image_bytes", &self.pools.bytes.len())
            .finish_non_exhaustive()
    }
}

fn fx_finish(h: FxHasher) -> u64 {
    // FxHash's low bits are its weakest; hashbrown indexes buckets by them.
    h.finish().rotate_left(26)
}

fn to_u32(n: usize, what: &'static str) -> Result<u32, ConstError> {
    u32::try_from(n).map_err(|_| ConstError::Capacity(what))
}

impl ConstStore {
    /// Bytes of the fixed per-value header (the node); every value costs
    /// this plus its pool slices plus ~5 bytes of intern table.
    pub const NODE_BYTES: usize = size_of::<Node>();

    /// An empty store for a target with `pointer_bytes`-byte pointers (the
    /// width of pointer-like subobjects in images).
    pub fn with_pointer_bytes(pointer_bytes: u8) -> Self {
        ConstStore {
            pools: Pools {
                pointer_bytes,
                nodes: Vec::new(),
                words: Vec::new(),
                paths: Vec::new(),
                aggs: vec![AggRec::default()],
                bytes: Vec::new(),
                relocs: Vec::new(),
                padding: Vec::new(),
                unions: Vec::new(),
            },
            table: HashTable::new(),
        }
    }

    /// The target pointer width this store was created with.
    pub fn pointer_bytes(&self) -> u8 {
        self.pools.pointer_bytes
    }

    /// Number of distinct values.
    pub fn len(&self) -> usize {
        self.pools.nodes.len()
    }

    /// True if no value has been interned.
    pub fn is_empty(&self) -> bool {
        self.pools.nodes.is_empty()
    }

    /// Every id, in interning order.
    pub fn ids(&self) -> impl Iterator<Item = ConstValueId> + '_ {
        (1..=self.pools.nodes.len() as u32).map(ConstValueId::new)
    }

    /// Interns a scalar.
    ///
    /// # Errors
    /// [`ConstError::Malformed`] if `s` breaks the contract of its variant
    /// (float bits wider than the format, a function pointer with a
    /// non-zero offset, a [`PtrBase::Const`] id not in this store);
    /// [`ConstError::Capacity`] on id or pool exhaustion.
    pub fn push_scalar(&mut self, s: Scalar<'_>) -> Result<ConstValueId, ConstError> {
        self.pools.check_scalar(&s)?;
        let h = hash_scalar(&s);
        if let Some(&id) = self.table.find(h, |&id| self.pools.scalar_eq(id, &s)) {
            return Ok(id);
        }
        let id = self.pools.append_scalar(&s)?;
        let pools = &self.pools;
        self.table.insert_unique(h, id, |&id| pools.hash_of(id));
        Ok(id)
    }

    /// Interns an aggregate image (see [`ByteImage`] for the canonical form
    /// it must be in).
    ///
    /// # Errors
    /// [`ConstError::Malformed`] if the image is not canonical (unsorted or
    /// overlapping tables, non-zero bytes under a relocation or padding, a
    /// relocation to a value that is not pointer-like);
    /// [`ConstError::Capacity`] on id or pool exhaustion.
    pub fn push_aggregate(&mut self, img: ByteImage<'_>) -> Result<ConstValueId, ConstError> {
        self.pools.check_image(&img)?;
        let h = hash_image(&img);
        if let Some(&id) = self.table.find(h, |&id| self.pools.image_eq(id, &img)) {
            return Ok(id);
        }
        let id = self.pools.append_image(&img, h)?;
        let pools = &self.pools;
        self.table.insert_unique(h, id, |&id| pools.hash_of(id));
        Ok(id)
    }

    /// Interns a value of either shape.
    ///
    /// # Errors
    /// As [`ConstStore::push_scalar`] / [`ConstStore::push_aggregate`].
    pub fn push(&mut self, v: ConstValue<'_>) -> Result<ConstValueId, ConstError> {
        match v {
            ConstValue::Scalar(s) => self.push_scalar(s),
            ConstValue::Aggregate(a) => self.push_aggregate(a),
        }
    }

    /// The id of `v` if it is already interned (no insertion; a malformed
    /// value is never found).
    pub fn lookup(&self, v: &ConstValue<'_>) -> Option<ConstValueId> {
        match v {
            ConstValue::Scalar(s) => {
                self.pools.check_scalar(s).ok()?;
                self.table.find(hash_scalar(s), |&id| self.pools.scalar_eq(id, s)).copied()
            }
            ConstValue::Aggregate(a) => {
                self.pools.check_image(a).ok()?;
                self.table.find(hash_image(a), |&id| self.pools.image_eq(id, a)).copied()
            }
        }
    }

    fn node(&self, id: ConstValueId) -> Node {
        self.pools.node(id)
    }

    /// The value.
    ///
    /// # Panics
    /// If `id` is not from this store.
    pub fn get(&self, id: ConstValueId) -> ConstValue<'_> {
        self.pools.get(id)
    }

    /// The value's type.
    pub fn ty(&self, id: ConstValueId) -> QualType {
        self.node(id).ty
    }

    /// The value's kind.
    pub fn kind(&self, id: ConstValueId) -> ValueKind {
        match self.node(id).kind() {
            kind::INT => ValueKind::Int,
            kind::BOOL => ValueKind::Bool,
            kind::FLOAT => ValueKind::Float,
            kind::NULL => ValueKind::Null,
            kind::PTR => ValueKind::Pointer,
            kind::MEMPTR => ValueKind::MemberPtr,
            kind::REFL => ValueKind::Reflection,
            _ => ValueKind::Aggregate,
        }
    }

    /// The value if it is a scalar.
    pub fn scalar(&self, id: ConstValueId) -> Option<Scalar<'_>> {
        match self.get(id) {
            ConstValue::Scalar(s) => Some(s),
            ConstValue::Aggregate(_) => None,
        }
    }

    /// The image if the value is an aggregate.
    pub fn aggregate(&self, id: ConstValueId) -> Option<ByteImage<'_>> {
        match self.get(id) {
            ConstValue::Aggregate(a) => Some(a),
            ConstValue::Scalar(_) => None,
        }
    }

    /// The aggregate's object bytes (empty for a scalar).
    pub fn bytes(&self, id: ConstValueId) -> &[u8] {
        self.aggregate(id).map_or(&[], |a| a.bytes)
    }

    /// The aggregate's relocations (empty for a scalar).
    pub fn relocs(&self, id: ConstValueId) -> &[Reloc] {
        self.aggregate(id).map_or(&[], |a| a.relocs)
    }

    /// The aggregate's padding ranges (empty for a scalar).
    pub fn padding(&self, id: ConstValueId) -> &[ByteRange] {
        self.aggregate(id).map_or(&[], |a| a.padding)
    }

    /// The aggregate's active union members (empty for a scalar).
    pub fn unions(&self, id: ConstValueId) -> &[UnionMember] {
        self.aggregate(id).map_or(&[], |a| a.unions)
    }

    /// The aggregate's shadow summary: its bytes as maximal runs of
    /// [`ShadowKind::Value`] and [`ShadowKind::Padding`], with each
    /// relocation as its own [`ShadowKind::Pointer`] run, in order and
    /// covering the image exactly. Empty for a scalar.
    pub fn shadow(&self, id: ConstValueId) -> Shadow<'_> {
        let img = self.aggregate(id);
        Shadow {
            pools: &self.pools,
            len: img.map_or(0, |a| a.bytes.len() as u64),
            relocs: img.map_or(&[], |a| a.relocs),
            padding: img.map_or(&[], |a| a.padding),
            pos: 0,
        }
    }

    /// The number of image bytes a value occupies as a relocation inside an
    /// aggregate, or `None` if it is not pointer-like: a pointer, null
    /// pointer, null data-member pointer or data-member pointer is one
    /// pointer; a member-function pointer (null or not) is two (Itanium); a
    /// reflection is 8 bytes (spec/oven.md "Reflection runtime").
    pub fn reloc_width(&self, id: ConstValueId) -> Option<u32> {
        self.pools.reloc_width(id)
    }
}

impl Pools {
    fn node(&self, id: ConstValueId) -> Node {
        self.nodes[id.index() as usize - 1]
    }

    fn contains(&self, id: ConstValueId) -> bool {
        id.index() as usize <= self.nodes.len()
    }

    fn reloc_width(&self, id: ConstValueId) -> Option<u32> {
        if !self.contains(id) {
            return None;
        }
        let p = u32::from(self.pointer_bytes);
        let n = self.node(id);
        match n.kind() {
            kind::NULL if n.extra() == NullKind::MemberFunction as u32 => Some(2 * p),
            kind::MEMPTR if n.flags() & F_FUNCTION != 0 => Some(2 * p),
            kind::PTR | kind::NULL | kind::MEMPTR => Some(p),
            kind::REFL => Some(8),
            _ => None,
        }
    }

    fn get(&self, id: ConstValueId) -> ConstValue<'_> {
        let n = self.node(id);
        let ty = n.ty;
        let s = match n.kind() {
            kind::INT => {
                let width = n.extra();
                let bits = if width <= 64 {
                    IntBits::from_u128(width, u128::from(n.a) | (u128::from(n.b) << 32))
                } else {
                    let start = n.a as usize;
                    let words = &self.words[start..start + IntBits::words_for(width)];
                    IntBits::from_words(width, words)
                        .expect("ConstStore: stored int not normalized")
                };
                Scalar::Int { ty, signed: n.flags() & F_SIGNED != 0, bits }
            }
            kind::BOOL => Scalar::Bool { ty, value: n.a != 0 },
            kind::FLOAT => {
                let kind = FloatKind::from_u8(n.extra() as u8).expect("ConstStore: float kind");
                let bits = if kind.bits() <= 64 {
                    u128::from(n.a) | (u128::from(n.b) << 32)
                } else {
                    let w = &self.words[n.a as usize..n.a as usize + 2];
                    u128::from(w[0]) | (u128::from(w[1]) << 64)
                };
                Scalar::Float { ty, kind, bits }
            }
            kind::NULL => Scalar::Null {
                ty,
                kind: match n.extra() {
                    0 => NullKind::Pointer,
                    1 => NullKind::MemberData,
                    _ => NullKind::MemberFunction,
                },
            },
            kind::PTR => {
                let (base, offset) = if n.flags() & F_AUX != 0 {
                    let aux = self.words[n.b as usize + 1] as u32;
                    (PtrBase::from_parts_aux(n.extra(), n.a, aux), self.words[n.b as usize])
                } else if n.flags() & F_WIDE != 0 {
                    (PtrBase::from_parts(n.extra(), n.a), self.words[n.b as usize])
                } else {
                    (PtrBase::from_parts(n.extra(), n.a), u64::from(n.b))
                };
                Scalar::Pointer { ty, base: base.expect("ConstStore: pointer base"), offset }
            }
            kind::MEMPTR => {
                let start = n.b as usize;
                Scalar::MemberPtr {
                    ty,
                    member: DeclId::new(n.a),
                    kind: if n.flags() & F_FUNCTION != 0 {
                        MemberKind::Function
                    } else {
                        MemberKind::Data
                    },
                    derived: n.flags() & F_DERIVED != 0,
                    path: &self.paths[start..start + n.extra() as usize],
                }
            }
            kind::REFL => Scalar::Reflection { ty, id: ReflectionId::new(n.a) },
            _ => {
                let (r, e) = (self.aggs[n.a as usize], self.aggs[n.a as usize + 1]);
                return ConstValue::Aggregate(ByteImage {
                    ty,
                    bytes: &self.bytes[r.bytes as usize..e.bytes as usize],
                    relocs: &self.relocs[r.relocs as usize..e.relocs as usize],
                    padding: &self.padding[r.padding as usize..e.padding as usize],
                    unions: &self.unions[r.unions as usize..e.unions as usize],
                });
            }
        };
        ConstValue::Scalar(s)
    }

    fn hash_of(&self, id: ConstValueId) -> u64 {
        let n = self.node(id);
        if n.kind() == kind::AGG {
            return self.aggs[n.a as usize].hash;
        }
        match self.get(id) {
            ConstValue::Scalar(s) => hash_scalar(&s),
            ConstValue::Aggregate(_) => unreachable!("ConstStore: aggregate node without AGG kind"),
        }
    }

    fn scalar_eq(&self, id: ConstValueId, s: &Scalar<'_>) -> bool {
        matches!(self.get(id), ConstValue::Scalar(t) if t == *s)
    }

    fn image_eq(&self, id: ConstValueId, img: &ByteImage<'_>) -> bool {
        matches!(self.get(id), ConstValue::Aggregate(a) if a == *img)
    }

    fn check_scalar(&self, s: &Scalar<'_>) -> Result<(), ConstError> {
        match *s {
            Scalar::Float { kind, bits, .. } => {
                if kind.bits() < 128 && bits >> kind.bits() != 0 {
                    return Err(ConstError::Malformed("float bits wider than the format"));
                }
            }
            Scalar::Pointer { base, offset, .. } => match base {
                PtrBase::Function(_) if offset != 0 => {
                    return Err(ConstError::Malformed("function pointer with an offset"));
                }
                PtrBase::Const(c) if !self.contains(c) => {
                    return Err(ConstError::Malformed("PtrBase::Const id not in this store"));
                }
                _ => {}
            },
            Scalar::MemberPtr { path, .. } => {
                if path.len() >= 1 << 24 {
                    return Err(ConstError::Capacity("member pointer path longer than 2^24"));
                }
            }
            Scalar::Int { .. }
            | Scalar::Bool { .. }
            | Scalar::Null { .. }
            | Scalar::Reflection { .. } => {}
        }
        Ok(())
    }

    fn new_id(&self) -> Result<ConstValueId, ConstError> {
        let next = to_u32(self.nodes.len() + 1, "ConstValueId")?;
        ConstValueId::try_new(next).ok_or(ConstError::Capacity("ConstValueId"))
    }

    fn push_words(&mut self, w: &[u64]) -> Result<u32, ConstError> {
        let start = to_u32(self.words.len(), "constant word pool")?;
        to_u32(self.words.len() + w.len(), "constant word pool")?;
        self.words.extend_from_slice(w);
        Ok(start)
    }

    fn append_scalar(&mut self, s: &Scalar<'_>) -> Result<ConstValueId, ConstError> {
        let id = self.new_id()?;
        let lo_hi = |v: u64| (v as u32, (v >> 32) as u32);
        let node = match *s {
            Scalar::Int { ty, signed, bits } => {
                let flags = if signed { F_SIGNED } else { 0 };
                let (a, b) = if bits.width() <= 64 {
                    lo_hi(bits.words()[0])
                } else {
                    (self.push_words(bits.words())?, 0)
                };
                Node { ty, tag: tag(kind::INT, flags, bits.width()), a, b }
            }
            Scalar::Bool { ty, value } => {
                Node { ty, tag: tag(kind::BOOL, 0, 0), a: u32::from(value), b: 0 }
            }
            Scalar::Float { ty, kind: k, bits } => {
                let (a, b) = if k.bits() <= 64 {
                    lo_hi(bits as u64)
                } else {
                    (self.push_words(&[bits as u64, (bits >> 64) as u64])?, 0)
                };
                Node { ty, tag: tag(kind::FLOAT, 0, k as u32), a, b }
            }
            Scalar::Null { ty, kind: k } => {
                Node { ty, tag: tag(kind::NULL, 0, k as u32), a: 0, b: 0 }
            }
            Scalar::Pointer { ty, base, offset } => {
                let (flags, b) = match (base.aux(), u32::try_from(offset)) {
                    (Some(aux), _) => (F_AUX, self.push_words(&[offset, u64::from(aux)])?),
                    (None, Ok(b)) => (0, b),
                    (None, Err(_)) => (F_WIDE, self.push_words(&[offset])?),
                };
                Node { ty, tag: tag(kind::PTR, flags, base.code()), a: base.raw(), b }
            }
            Scalar::MemberPtr { ty, member, kind: k, derived, path } => {
                let mut flags = if derived { F_DERIVED } else { 0 };
                if k == MemberKind::Function {
                    flags |= F_FUNCTION;
                }
                let b = to_u32(self.paths.len(), "member pointer path pool")?;
                to_u32(self.paths.len() + path.len(), "member pointer path pool")?;
                self.paths.extend_from_slice(path);
                Node { ty, tag: tag(kind::MEMPTR, flags, path.len() as u32), a: member.index(), b }
            }
            Scalar::Reflection { ty, id } => {
                Node { ty, tag: tag(kind::REFL, 0, 0), a: id.index(), b: 0 }
            }
        };
        self.nodes.push(node);
        Ok(id)
    }

    fn check_image(&self, img: &ByteImage<'_>) -> Result<(), ConstError> {
        use ConstError::Malformed;
        let len = img.bytes.len() as u64;
        if len > u64::from(u32::MAX) {
            return Err(ConstError::Capacity("aggregate image larger than 4 GiB"));
        }
        let zero =
            |start: u64, end: u64| img.bytes[start as usize..end as usize].iter().all(|&b| b == 0);
        let mut prev_end = 0u64;
        for (i, r) in img.relocs.iter().enumerate() {
            let w =
                self.reloc_width(r.value).ok_or(Malformed("relocation to a non-pointer value"))?;
            let (s, e) = (u64::from(r.at), u64::from(r.at) + u64::from(w));
            if i > 0 && s < prev_end {
                return Err(Malformed("relocations unsorted or overlapping"));
            }
            if e > len {
                return Err(Malformed("relocation outside the image"));
            }
            if !zero(s, e) {
                return Err(Malformed("non-zero bytes under a relocation"));
            }
            prev_end = e;
        }
        let mut relocs = img.relocs.iter().peekable();
        let mut prev: Option<ByteRange> = None;
        for p in img.padding {
            if p.len == 0 {
                return Err(Malformed("empty padding range"));
            }
            if prev.is_some_and(|q| u64::from(p.start) <= q.end()) {
                return Err(Malformed("padding ranges unsorted, overlapping or not coalesced"));
            }
            if p.end() > len {
                return Err(Malformed("padding outside the image"));
            }
            if !zero(u64::from(p.start), p.end()) {
                return Err(Malformed("non-zero padding byte"));
            }
            // Relocations ending at or before this range cannot overlap it
            // or any later one.
            while let Some(r) = relocs.peek() {
                let w = u64::from(self.reloc_width(r.value).unwrap_or(0));
                let (rs, re) = (u64::from(r.at), u64::from(r.at) + w);
                if re <= u64::from(p.start) {
                    relocs.next();
                } else if rs < p.end() {
                    return Err(Malformed("padding overlaps a relocation"));
                } else {
                    break;
                }
            }
            prev = Some(*p);
        }
        for w in img.unions.windows(2) {
            if (w[0].at, w[0].field) >= (w[1].at, w[1].field) {
                return Err(Malformed("union members unsorted or duplicated"));
            }
        }
        if img.unions.last().is_some_and(|u| u64::from(u.at) >= len) {
            return Err(Malformed("union member outside the image"));
        }
        Ok(())
    }

    fn append_image(&mut self, img: &ByteImage<'_>, hash: u64) -> Result<ConstValueId, ConstError> {
        let id = self.new_id()?;
        let agg = to_u32(self.aggs.len() - 1, "aggregate count")?;
        let end = AggRec {
            bytes: to_u32(self.bytes.len() + img.bytes.len(), "constant byte pool")?,
            relocs: to_u32(self.relocs.len() + img.relocs.len(), "relocation pool")?,
            padding: to_u32(self.padding.len() + img.padding.len(), "padding pool")?,
            unions: to_u32(self.unions.len() + img.unions.len(), "union pool")?,
            hash: 0,
        };
        self.aggs.last_mut().expect("ConstStore: aggregate sentinel").hash = hash;
        self.aggs.push(end);
        self.bytes.extend_from_slice(img.bytes);
        self.relocs.extend_from_slice(img.relocs);
        self.padding.extend_from_slice(img.padding);
        self.unions.extend_from_slice(img.unions);
        self.nodes.push(Node { ty: img.ty, tag: tag(kind::AGG, 0, 0), a: agg, b: 0 });
        Ok(id)
    }
}

fn hash_scalar(s: &Scalar<'_>) -> u64 {
    let mut h = FxHasher::default();
    h.write_u32(s.ty().to_raw());
    match *s {
        Scalar::Int { signed, bits, .. } => {
            h.write_u32(tag(kind::INT, u32::from(signed), bits.width()));
            for &w in bits.words() {
                h.write_u64(w);
            }
        }
        Scalar::Bool { value, .. } => h.write_u32(tag(kind::BOOL, 0, u32::from(value))),
        Scalar::Float { kind: k, bits, .. } => {
            h.write_u32(tag(kind::FLOAT, 0, k as u32));
            h.write_u64(bits as u64);
            h.write_u64((bits >> 64) as u64);
        }
        Scalar::Null { kind: k, .. } => h.write_u32(tag(kind::NULL, 0, k as u32)),
        Scalar::Pointer { base, offset, .. } => {
            h.write_u32(tag(kind::PTR, 0, base.code()));
            h.write_u32(base.raw());
            if let Some(aux) = base.aux() {
                h.write_u32(aux);
            }
            h.write_u64(offset);
        }
        Scalar::MemberPtr { member, kind: k, derived, path, .. } => {
            h.write_u32(tag(kind::MEMPTR, u32::from(derived), k as u32));
            h.write_u32(member.index());
            for d in path {
                h.write_u32(d.index());
            }
        }
        Scalar::Reflection { id, .. } => {
            h.write_u32(tag(kind::REFL, 0, 0));
            h.write_u32(id.index());
        }
    }
    fx_finish(h)
}

fn hash_image(img: &ByteImage<'_>) -> u64 {
    let mut h = FxHasher::default();
    h.write_u32(img.ty.to_raw());
    h.write_u32(tag(kind::AGG, 0, 0));
    h.write_usize(img.bytes.len());
    h.write(img.bytes);
    for r in img.relocs {
        h.write_u64(u64::from(r.at) | (u64::from(r.value.index()) << 32));
    }
    h.write_usize(img.padding.len());
    for p in img.padding {
        h.write_u64(u64::from(p.start) | (u64::from(p.len) << 32));
    }
    for u in img.unions {
        h.write_u64(u64::from(u.at) | (u64::from(u.field.index()) << 32));
    }
    fx_finish(h)
}

/// Iterator over an aggregate's shadow runs ([`ConstStore::shadow`]).
#[derive(Clone, Debug)]
pub struct Shadow<'a> {
    pools: &'a Pools,
    len: u64,
    relocs: &'a [Reloc],
    padding: &'a [ByteRange],
    pos: u64,
}

impl Iterator for Shadow<'_> {
    type Item = ShadowRun;

    fn next(&mut self) -> Option<ShadowRun> {
        if self.pos >= self.len {
            return None;
        }
        let run = |start: u64, end: u64, kind| ShadowRun {
            range: ByteRange { start: start as u32, len: (end - start) as u32 },
            kind,
        };
        let pos = self.pos;
        if let Some(r) = self.relocs.first().filter(|r| u64::from(r.at) == pos) {
            let w = u64::from(self.pools.reloc_width(r.value).unwrap_or(0));
            self.relocs = &self.relocs[1..];
            self.pos = pos + w;
            return Some(run(pos, pos + w, ShadowKind::Pointer(r.value)));
        }
        if let Some(p) = self.padding.first().filter(|p| u64::from(p.start) == pos) {
            self.padding = &self.padding[1..];
            self.pos = p.end();
            return Some(run(pos, p.end(), ShadowKind::Padding));
        }
        let next_reloc = self.relocs.first().map_or(self.len, |r| u64::from(r.at));
        let next_pad = self.padding.first().map_or(self.len, |p| u64::from(p.start));
        let end = next_reloc.min(next_pad);
        self.pos = end;
        Some(run(pos, end, ShadowKind::Value))
    }
}

impl HeapSize for ConstStore {
    fn heap_bytes(&self) -> MemReport {
        let p = &self.pools;
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("nodes", &p.nodes));
        r.push(MemRow::of_vec("words", &p.words));
        r.push(MemRow::of_vec("paths", &p.paths));
        // `aggs` ends with a sentinel; count aggregates, account all bytes.
        let mut aggs = MemRow::of_vec("aggs", &p.aggs);
        aggs.count -= 1;
        r.push(aggs);
        r.push(MemRow::of_vec("bytes", &p.bytes));
        r.push(MemRow::of_vec("relocs", &p.relocs));
        r.push(MemRow::of_vec("padding", &p.padding));
        r.push(MemRow::of_vec("unions", &p.unions));
        let slot = size_of::<ConstValueId>() as u64 + 1;
        r.push(MemRow::new(
            "intern",
            self.table.len() as u64,
            self.table.len() as u64 * slot,
            self.table.allocation_size() as u64,
        ));
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_and_side_record_sizes() {
        assert_eq!(size_of::<Node>(), 16);
        assert_eq!(size_of::<AggRec>(), 24);
        assert_eq!(size_of::<Reloc>(), 8);
        assert_eq!(size_of::<ByteRange>(), 8);
        assert_eq!(size_of::<UnionMember>(), 8);
    }
}
